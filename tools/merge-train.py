#!/usr/bin/env python3
"""FreeCity4 merge train: the single owner of `main`.

Why this exists (docs/merge-train.md "Why"; the original plan is in docs/history/handoff-2026-10.md): each lane
used to take a shared merge lock, merge main, re-gate and renumber `FC4_ABI_VERSION` / the HANDOFF rows / the
manual-checks run numbers itself. N waiting lanes therefore cost about N^2 gates, nearly every merge conflicted on the same five files, and about 25 agents polled
the lock so docs-only lanes waited hours.

The train instead:
  1. a finished lane writes `build-runs/merge-queue/<lane>.json` in its own worktree and stops;
  2. the train takes up to K ready lanes (oldest first), merges them one after another onto `integration/<n>`
     cut from main, resolving the known conflict hotspots once for the whole batch;
  3. it runs the gate and the regress once for the whole batch;
  4. on a pass main fast-forwards to the integration branch and each lane gets a merged notice;
  5. on a fail it bisects the batch (halves, gated in parallel) until the culprit lane is isolated, sends the
     failure back to that lane and lands the rest;
  6. docs-only and `test`/`tools` lanes skip the regress (and, when `engine/`, `tools/` and `tests/` are all
     unchanged, the gate result from the last engine batch stands).

Usage (run from the repository root, on `main`):
  python tools/merge-train.py status
  python tools/merge-train.py plan [--size 8]
  python tools/merge-train.py run   [--size 8] [--no-regress] [--land]
  python tools/merge-train.py merge-batch [--size 8]
  python tools/merge-train.py gate
  python tools/merge-train.py regress [--add-replace occmgr,propmgr,lotcfg]
  python tools/merge-train.py land
  python tools/merge-train.py bisect
  python tools/merge-train.py enqueue --lane L --branch B --kind engine|tools|docs|test [--abi]
                                      [--game-runs a,b] [--summary "..."]

State lives in `build-runs/merge-train/state.json` (gitignored). Nothing here pushes, and nothing here deletes a
worktree.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
NL = chr(10)
MAIN = REPO
QUEUE_DIRS = [REPO / "build-runs" / "merge-queue"]
TRAIN_DIR = REPO / "build-runs" / "merge-train"
STATE_PATH = TRAIN_DIR / "state.json"
NOTICE_DIR = REPO / "build-runs" / "merge-notices"
WT_ROOT = REPO / ".claude" / "worktrees"
TRAIN_WT = WT_ROOT / "train-integration"
TRAIN_BRANCH = "train-integration"
LOG_DIR = REPO / "build-runs"

KINDS = ("engine", "tools", "docs", "test")
# "engine" touches engine/ and wants both a gate and a regress. The rest are batched freely and, for
# docs, skip the regress.
DOCS_KINDS = ("docs",)

# The one build/test implementation, shared with the lanes: every verdict is parsed from the tools' output.
sys.path.insert(0, str(REPO / ".claude" / "lane-bin"))
import fc4verify  # noqa: E402

VERSION_RE_C = re.compile(r"^(\s*#define\s+FC4_ABI_VERSION\s+)(\d+)(u?)\s*$", re.M)
VERSION_RE_RS = re.compile(r"^(\s*pub const FC4_ABI_VERSION: u32 = )(\d+)(;.*)$", re.M)


# --------------------------------------------------------------------------- helpers


def sh(args, cwd=REPO, check=True, capture=True, env=None, timeout=None):
    """Run a command; never uses the shell, so paths with spaces are safe."""
    e = dict(os.environ)
    if env:
        e.update(env)
    p = subprocess.run(
        [str(a) for a in args],
        cwd=str(cwd),
        env=e,
        capture_output=capture,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=timeout,
    )
    if check and p.returncode != 0:
        out = (p.stdout or "") + (p.stderr or "")
        raise RuntimeError(f"command failed ({p.returncode}): {' '.join(str(a) for a in args)}\n{out[-4000:]}")
    return p


def git(args, cwd=MAIN, check=True):
    return sh(["git"] + list(args), cwd=cwd, check=check)


def log(msg):
    print(f"[{time.strftime('%H:%M:%S')}] {msg}", flush=True)


def die(msg):
    print(f"merge-train: {msg}", file=sys.stderr)
    sys.exit(1)


def load_state():
    if STATE_PATH.exists():
        return json.loads(STATE_PATH.read_text(encoding="utf-8"))
    return {"batch_seq": 0, "current": None, "history": []}


def save_state(st):
    TRAIN_DIR.mkdir(parents=True, exist_ok=True)
    STATE_PATH.write_text(json.dumps(st, indent=2) + "\n", encoding="utf-8")


def main_is_clean():
    p = git(["status", "--porcelain"], check=False)
    return not (p.stdout or "").strip()


def current_branch():
    return git(["rev-parse", "--abbrev-ref", "HEAD"]).stdout.strip()


# --------------------------------------------------------------------------- queue


def queue_files():
    """Every merge-queue file: the coordinator's own build-runs/ plus each lane worktree's.

    Files renamed to `<lane>.merged.json` when a batch lands are done and must never be picked up again.
    """
    files = []

    def collect(d):
        if d.is_dir():
            files.extend(sorted(f for f in d.glob("*.json") if not f.name.endswith(".merged.json")))

    # Lane worktrees first: a lane writes its own queue file there, and it must win over a stale copy the
    # coordinator seeded in the main checkout before that lane finished.
    if WT_ROOT.is_dir():
        for wt in sorted(WT_ROOT.iterdir()):
            collect(wt / "build-runs" / "merge-queue")
    for d in QUEUE_DIRS:
        collect(d)
    return files


def read_queue():
    out = []
    seen = set()
    for f in queue_files():
        try:
            rec = json.loads(f.read_text(encoding="utf-8-sig"))
        except Exception as exc:  # noqa: BLE001 - a malformed queue file must not stop the train
            print(f"  ! ignoring {f}: {exc}")
            continue
        lane = rec.get("lane") or f.stem
        if lane in seen:
            continue
        seen.add(lane)
        rec.setdefault("lane", lane)
        rec.setdefault("kind", "engine")
        rec.setdefault("abi", False)
        rec.setdefault("game_runs", [])
        rec.setdefault("state", "ready")
        rec["_file"] = str(f)
        out.append(rec)
    return out


def lane_worktree(lane):
    p = WT_ROOT / lane
    return p if p.is_dir() else None


def lane_branch_exists(branch):
    p = git(["rev-parse", "--verify", "--quiet", f"refs/heads/{branch}"], check=False)
    return p.returncode == 0


def lane_tip(branch):
    return git(["rev-parse", branch]).stdout.strip()


def describe_lane(rec):
    branch = rec.get("branch", rec["lane"])
    problems = []
    if not lane_branch_exists(branch):
        problems.append("branch missing")
    else:
        tip = lane_tip(branch)
        if rec.get("tip") and rec["tip"][:12] != tip[:12]:
            problems.append(f"tip moved ({rec['tip'][:8]} -> {tip[:8]})")
        wt = lane_worktree(rec["lane"])
        if wt is not None:
            dirty = git(["status", "--porcelain"], cwd=wt, check=False).stdout.strip()
            if dirty:
                problems.append(f"worktree dirty ({len(dirty.splitlines())} paths)")
        ahead = git(["rev-list", "--count", f"main..{branch}"], check=False).stdout.strip()
        rec["_ahead"] = ahead or "?"
    rec["_problems"] = problems
    rec["_ready"] = rec.get("state") == "ready" and not problems
    return rec


# --------------------------------------------------------------------------- commands


def cmd_status(_args):
    recs = [describe_lane(r) for r in read_queue()]
    st = load_state()
    print(f"main: {git(['rev-parse', '--short', 'HEAD']).stdout.strip()} on {current_branch()}")
    print(f"batch seq: {st['batch_seq']}   current: {st.get('current')}")
    stop = REPO / "build-runs" / "STOP"
    if stop.exists():
        # lanes.py stop: no lane starts, but what is queued still lands (a wind-down lands what is on disk)
        try:
            why = json.loads(stop.read_text(encoding="utf-8")).get("reason", "")
        except (OSError, ValueError, AttributeError):
            why = ""
        print(f"STOP flag set ({why or 'build-runs/STOP'}): no new lane rounds; landing the queue continues")
    if not recs:
        print("queue: empty")
        return 0
    print(f"queue: {len(recs)} lane(s)")
    for r in sorted(recs, key=lambda x: x.get("tip", "")):
        flag = "READY" if r["_ready"] else "hold "
        print(
            f"  {flag}  {r['lane']:<18} {r['kind']:<7} abi={str(r['abi']):<5} "
            f"ahead={r.get('_ahead', '?'):>3}  {'; '.join(r['_problems']) or r.get('summary', '')[:70]}"
        )
    return 0


def cmd_enqueue(args):
    rec = {
        "lane": args.lane,
        "branch": args.branch,
        "tip": args.tip or (lane_tip(args.branch) if lane_branch_exists(args.branch) else ""),
        "kind": args.kind,
        "abi": bool(args.abi),
        "game_runs": [x for x in (args.game_runs or "").split(",") if x],
        "summary": args.summary or "",
        "state": "ready",
    }
    out = QUEUE_DIRS[0] / f"{args.lane}.json"
    QUEUE_DIRS[0].mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(rec, indent=2) + "\n", encoding="utf-8")
    log(f"enqueued {args.lane} -> {out}")
    return 0


def pick_batch(size, lanes=None):
    recs = [describe_lane(r) for r in read_queue()]
    ready = [r for r in recs if r["_ready"]]
    if lanes:  # an explicit batch (--lanes a,b,c): only these, and say which are not ready
        want = [x.strip() for x in lanes.split(",") if x.strip()]
        missing = [x for x in want if x not in {r["lane"] for r in ready}]
        if missing:
            log("not queued or not ready, left out: " + ", ".join(missing))
        ready = [r for r in ready if r["lane"] in want]
    # Docs-only first (they never need a regress and they clear the way), then by tip commit date = age.
    ready.sort(key=lambda r: (0 if r["kind"] in DOCS_KINDS else 1, r.get("tip", "")))
    return ready[:size], recs


def cmd_plan(args):
    batch, recs = pick_batch(args.size, getattr(args, "lanes", None))
    print(f"batch of {len(batch)} (of {len(recs)} queued):")
    for r in batch:
        print(f"  {r['lane']:<18} {r['kind']:<7} abi={r['abi']}  branch={r['branch']}  tip={r['tip'][:10]}")
    if not batch:
        print("nothing ready")
    engine = [r for r in batch if r["kind"] not in DOCS_KINDS]
    print(f"gate needed: {bool(engine)}   regress needed: {bool(engine)}")
    return 0


def fix_sdk_gitlink(wt):
    """Point the gzcom-dll submodule at the main checkout's module store.

    A git worktree's `.git` is a file, so the submodule's relative `gitdir: ../../../.git/modules/...` does
    not resolve from `.claude/worktrees/<name>/engine/third_party/gzcom-dll` and every git command in the
    worktree then fails with "not a git repository". Lanes hit this too (flakyfix, 2026-10-16).
    """
    dot = Path(wt) / "engine" / "third_party" / "gzcom-dll" / ".git"
    if not dot.is_file():
        return
    want = (REPO / ".git" / "modules" / "engine" / "third_party" / "gzcom-dll").as_posix()
    try:
        text = dot.read_text(encoding="utf-8", errors="replace").strip()
        if text.startswith("gitdir:") and not Path(text.split(":", 1)[1].strip()).exists():
            dot.write_text(f"gitdir: {want}\n", encoding="utf-8")
            log(f"repointed the gzcom-dll gitlink in {Path(wt).name}")
    except OSError as exc:
        log(f"could not repoint the gzcom-dll gitlink in {Path(wt).name}: {exc}")


def ensure_train_worktree():
    if not TRAIN_WT.is_dir():
        TRAIN_WT.parent.mkdir(parents=True, exist_ok=True)
        log(f"creating integration worktree {TRAIN_WT}")
        git(["worktree", "add", str(TRAIN_WT), "-b", TRAIN_BRANCH, "main"])
    fix_sdk_gitlink(TRAIN_WT)
    return TRAIN_WT


def reset_integration(name):
    """Point the long-lived integration worktree at a fresh branch cut from main."""
    wt = ensure_train_worktree()
    git(["fetch", "origin"], check=False)  # no-op on a local-only repo; harmless
    git(["-C", str(wt), "reset", "--hard"], check=False)
    git(["-C", str(wt), "clean", "-fd"], check=False)
    git(["-C", str(wt), "checkout", "--force", "-B", name, "main"])
    return wt


# --- conflict hotspots -------------------------------------------------------
# These five files conflicted in nearly every merge because every lane edited them. The train resolves the
# mechanical ones once for the whole batch and regenerates the derived ones.

ABI_HEADER = "engine/capi/include/fc4.h"
ABI_RS = "engine/capi/src/lib.rs"
ABI_INC = "engine/tests/abi_entries.inc"
HANDOFF = "docs/HANDOFF.md"
LANES_INDEX = "docs/lanes/INDEX.md"
MANUAL = "docs/manual-checks.md"
QUESTIONS = "docs/specs/QUESTIONS.md"  # generated index; the questions live in the per-spec files below
QUESTIONS_DIR = "docs/specs/questions/"
PROGRESS = "docs/PROGRESS.md"
# Derived docs: lanes change them only by running tools/gen_handoff_tables.py (the lane table in INDEX.md, the
# latest-run line in HANDOFF.md), and the HANDOFF's prose is the coordinator's. On a conflict take ours (the
# integration side) and let regenerate() rebuild the generated regions from docs/lanes/ and docs/runs/. A union
# would paste a lane's stale copy back in: HANDOFF.md was cut from 134 KB to one page on 2026-10-20, and every
# lane cut before that carries the old file with its lane table.
DERIVED = (HANDOFF, LANES_INDEX)
# Append-only docs: both sides only ever add rows or sections, so a union always keeps everything.
APPEND_ONLY = (MANUAL, PROGRESS)
# Shared source files where every lane only appends its own registration. These union ONLY when the hunks
# are disjoint additions (see can_union) — blindly unioning FC4Config.cpp once produced a function body with
# `Log` and `kCount` out of scope, which the batch gate then rejected.
UNION_PATHS = (
    "engine/conformance/README.md",
    "engine/conformance/src/runner.rs",
    "engine/plugin/FC4Config.h",
    "engine/plugin/FC4Config.cpp",
    "engine/plugin/FC4Subsystems.cpp",
    "engine/CMakeLists.txt",
    ABI_HEADER,
    ABI_RS,
)


def conflicted_paths(wt):
    out = git(["-C", str(wt), "diff", "--name-only", "--diff-filter=U"], check=False).stdout.split()
    return [p for p in out if p]


def split_terminator(x):
    """Split a trailing block close off a line: `  "pacing"};` -> (`  "pacing"`, `};`).

    A lane's last table entry carries the array's own close on the same line, so a line-level union keeps
    one close per lane. Only the last line of each side is split — a close mid-side is real structure.
    """
    s = x.rstrip()
    # Never treat a comment's own trailing `}` or `*/` as a block close: that changes the brace
    # arithmetic and the union is then refused (or, worse, accepted wrong).
    ls = s.lstrip()
    if ls.startswith("*") or ls.startswith("//") or ls.startswith("/*"):
        return x, None
    m = re.search(r"(\}\;?,?|\*/)\s*$", s)
    if not m:
        return x, None
    head = s[:m.start()].rstrip()
    return (head, m.group(1)) if head else ("", m.group(1))


def union_hunks(wt, rel):
    """Union every conflict hunk, keeping each structural terminator exactly once.

    Each lane's addition to a shared table ends with that table's own `};`, glued to its last entry
    (`"pacing"};`), so a plain union leaves one `};` per lane and the declarations after the array fall
    apart (`kCount` / `kSwitches` undeclared in FC4Config.cpp). Same for fc4.h, where every history entry
    ends with `*/`. Lines are never deduplicated otherwise — initialiser runs like `1, 1, 1,` are
    legitimately repeated.
    """
    path = Path(wt) / rel
    text = path.read_text(encoding="utf-8", errors="replace")
    if "<<<<<<<" not in text:
        return False
    out, i, n = [], 0, len(text)
    while i < n:
        s = text.find("<<<<<<<", i)
        if s < 0:
            out.append(text[i:])
            break
        out.append(text[i:s])
        m = text.find("=======", s)
        e = text.find(">>>>>>>", m) if m >= 0 else -1
        if m < 0 or e < 0:
            return False
        body_o = text[s + 7:m]
        body_t = text[m + 7:e]
        body_o = body_o.split(NL, 1)[1] if NL in body_o else ""
        body_t = body_t.split(NL, 1)[1] if NL in body_t else ""
        sides = (body_o, body_t)
        merged, terms = [], []
        for raw in (body_o, body_t):
            side = raw.splitlines()
            while side and not side[-1].strip():
                side.pop()
            if not side:
                continue
            last_head, last_term = split_terminator(side[-1])
            content = list(side[:-1])
            if last_head.strip():
                content.append(last_head)
            if content:
                # In a union this side's last entry is never the merged list's last entry, so a *list entry*
                # needs its trailing comma now. Only entries: a statement (`...;`) or a comment is left alone.
                for j in range(len(content) - 1, -1, -1):
                    s = content[j].strip()
                    if not s or s.startswith("//") or s.startswith("/*") or s.startswith("*"):
                        continue
                    if s.endswith(";") or s.endswith("{") or s.endswith("}") or s.endswith(","):
                        break
                    if s.startswith(('"', "{", "'", "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "-")):
                        content[j] = content[j].rstrip() + ","
                    break
            if last_term and last_term.rstrip() not in [t.rstrip() for t in terms]:
                terms.append(last_term)
            merged.extend(content)
        if any(t.rstrip().endswith("*/") for t in terms):
            merged = close_comment(merged)
        else:
            merged = merged + terms
        out.append(NL.join(merged))
        j = text.find(NL, e)
        i = n if j < 0 else j + 1
    text = "".join(out)
    if "<<<<<<<" in text or ">>>>>>>" in text:
        return False
    # A bad join shows up as a `fn` signature glued onto the end of a doc line.
    for ln in text.split(NL):
        s = ln.strip()
        import re as _re
        if _re.search(r"(pub(\(crate\))? )?fn [A-Za-z_][A-Za-z0-9_]*\s*\(", ln) and                 not s.startswith(("fn ", "pub fn ", "pub(crate) fn ")):
            return False
    # Structural check: for an additive union the merged brace balance must equal ours + theirs - base.
    # A duplicated (or dropped) `};` breaks that arithmetic, which is exactly the failure mode that
    # otherwise only surfaces as `kCount undeclared` twenty minutes later in the batch gate.
    st = {}
    for num, name in ((1, "base"), (2, "ours"), (3, "theirs")):
        got = git(["-C", str(wt), "show", f":{num}:{rel}"], check=False)
        if got.returncode != 0:
            st = None
            break
        st[name] = got.stdout
    if st:
        want = _brace_balance(st["ours"]) + _brace_balance(st["theirs"]) - _brace_balance(st["base"])
        if _brace_balance(text) != want:
            return False
    path.write_text(text, encoding="utf-8")
    git(["-C", str(wt), "add", rel], check=False)
    return True


def _brace_balance(s):
    return s.count("{") - s.count("}")


def union_merge(wt, rel):
    """Resolve a conflict by unioning ours and theirs over their common base.

    Used for the append-style docs files, where each lane only adds rows or sections. Taking either side
    alone silently drops the other side's additions; a union keeps both. Stage 1 is the base, 2 is ours
    (integration), 3 is theirs (the lane).
    """
    wtp = Path(wt)
    tmp = wtp / "build-runs" / "train-union"
    tmp.mkdir(parents=True, exist_ok=True)
    stages = {}
    for n, name in ((1, "base"), (2, "ours"), (3, "theirs")):
        p = git(["-C", str(wt), "show", f":{n}:{rel}"], check=False)
        body = p.stdout if p.returncode == 0 else None
        if body is None:
            # An add/add conflict has no base stage; an empty base makes this a pure union.
            if n != 1:
                return False
            body = ""
        stages[name] = tmp / f"{name}-{Path(rel).name}"
        stages[name].write_text(body, encoding="utf-8", newline="")
    out = sh(["git", "merge-file", "--union", "-L", "ours", "-L", "base", "-L", "theirs",
              str(stages["ours"]), str(stages["base"]), str(stages["theirs"])],
             check=False)
    # merge-file exits with the number of conflicts; --union resolves them all, so a non-zero exit with no
    # conflict markers left still means success.
    merged = stages["ours"].read_text(encoding="utf-8", errors="replace")
    if "<<<<<<<" in merged:
        return False
    (wtp / rel).write_text(merged, encoding="utf-8", newline="")
    git(["-C", str(wt), "add", rel], check=False)
    _ = out
    return True


def dedupe_version(path, regex):
    """After a union merge the version constant can appear twice (both sides bumped it). Keep one, at the max."""
    text = path.read_text(encoding="utf-8", errors="replace")
    lines = text.split(NL)
    hits = [i for i, ln in enumerate(lines) if regex.match(ln)]
    if len(hits) <= 1:
        return True
    best = max(int(regex.match(lines[i]).group(2)) for i in hits)
    keep = hits[0]
    for i in reversed(hits):
        if i != keep:
            del lines[i]
    lines[keep] = regex.sub(lambda m: f"{m.group(1)}{best}{m.group(3)}", lines[keep])
    path.write_text(NL.join(lines), encoding="utf-8")
    return True


def hunk_bodies(text):
    """Yield (ours_lines, theirs_lines) for every conflict hunk in a file."""
    i, n = 0, len(text)
    while i < n:
        s = text.find("<<<<<<<", i)
        if s < 0:
            return
        m = text.find("=======", s)
        e = text.find(">>>>>>>", m) if m >= 0 else -1
        if m < 0 or e < 0:
            return
        body_o = text[s + 7:m]
        body_t = text[m + 7:e]
        body_o = body_o.split(NL, 1)[1] if NL in body_o else ""
        body_t = body_t.split(NL, 1)[1] if NL in body_t else ""
        yield body_o.splitlines(), body_t.splitlines()
        i = e + 1


def can_union(path):
    """True when every hunk is two sides adding different lines at one anchor.

    That is the shape of nearly every shared-file conflict here: one lane adds `#include "FC4Seed.h"` and
    another `#include "FC4Perf.h"` where the same blank line was, or two lanes append rows to the same table.
    A hunk whose sides share a line is a real edit of shared text, and that goes to the operator.
    """
    text = path.read_text(encoding="utf-8", errors="replace")
    if "<<<<<<<" not in text:
        return False
    saw = False
    for ours, theirs in hunk_bodies(text):
        saw = True
        so = {x.strip() for x in ours if x.strip()}
        st = {x.strip() for x in theirs if x.strip()}
        if so & st:
            return False
    return saw


def commentish_line(x):
    """Blank, a ` * ` continuation, a complete `/* ... */` line, or the ABI version macro."""
    s = x.strip()
    return ((not s) or x.lstrip().startswith("*")
            or (s.startswith("/*") and s.endswith("*/")) or s == "*/"
            or bool(VERSION_RE_C.search(x)) or bool(VERSION_RE_RS.search(x)))


def comments_only(path):
    """True when every conflicted hunk is history comment and/or the version macro — fc4.h's usual shape.

    Two lanes' history entries plus the macro collide here, and the macro line is shared, so can_union
    rejects it; this is the case that must always union.
    """
    text = path.read_text(encoding="utf-8", errors="replace")
    if "<<<<<<<" not in text:
        return False
    saw = False
    for ours, theirs in hunk_bodies(text):
        saw = True
        if [x for x in ours + theirs if not commentish_line(x)]:
            return False
    return saw


def resolve_conflicts(wt, lanes):
    """Return (resolved, still_conflicted)."""
    paths = conflicted_paths(wt)
    if not paths:
        return [], []
    resolved, left = [], []
    for p in paths:
        if p == ABI_INC:
            if not (Path(wt) / p).exists():
                # modify/delete: abi-hygiene generates it at build time and removed it from git. Keep the
                # deletion — never restore, regenerate or commit the file.
                git(["-C", str(wt), "rm", "--cached", "-f", "--", p], check=False)
                resolved.append(p + " (kept deleted: generated at build time)")
                continue
            # Pre-abi-hygiene lanes still track it: take ours and regenerate.
            git(["-C", str(wt), "checkout", "--ours", "--", p], check=False)
            git(["-C", str(wt), "add", p], check=False)
            regen = Path(wt) / "engine" / "tests" / "gen_abi_entries.py"
            if regen.exists():
                sh([sys.executable, str(regen)], cwd=Path(wt) / "engine", check=False)
            resolved.append(p)
            continue
        if p in (ABI_HEADER, ABI_RS):
            # Union both sides (git's own union never drops or glues lines), then collapse the version
            # constant to a single line. consolidate_abi_history rewrites the history block afterwards, so
            # the unioned block does not have to be pretty.
            fp = Path(wt) / p
            if fp.exists() and (can_union(fp) or comments_only(fp)) and union_hunks(wt, p) and \
                    dedupe_version(fp, VERSION_RE_C if p.endswith(".h") else VERSION_RE_RS):
                resolved.append(p + " (union + version)")
                continue
            left.append(p)
            continue
        if p == QUESTIONS:
            # Generated by tools/questions.py from the per-spec files: either side will do, because
            # regenerate() rebuilds it from the merged per-spec files before the batch commit.
            git(["-C", str(wt), "checkout", "--ours", "--", p], check=False)
            git(["-C", str(wt), "add", p], check=False)
            resolved.append(p + " (generated: regenerated by tools/questions.py index)")
            continue
        if p.startswith(QUESTIONS_DIR) and p.endswith(".md"):
            # A per-spec question file is append-at-end and normally merges cleanly. When two lanes both
            # appended to the same spec's file the hunks conflict only because they share an anchor, and
            # the union keeps both appends.
            if union_merge(wt, p):
                resolved.append(p + " (union: both appends kept)")
                continue
            left.append(p)
            continue
        if p in DERIVED:
            if git(["-C", str(wt), "checkout", "--ours", "--", p], check=False).returncode == 0:
                git(["-C", str(wt), "add", p], check=False)
                resolved.append(p + " (ours; regenerated after the merges)")
                continue
            left.append(p)
            continue
        if p in APPEND_ONLY or p.endswith(".md"):
            # Docs and spec tables: both sides only add sections. Two new sections can begin with the same
            # markdown table header, so a strict disjoint test rejects them; a duplicated header is a
            # formatting nit and a dropped section is not.
            if union_merge(wt, p):
                resolved.append(p + " (union)")
                continue
            left.append(p)
            continue
        # Additive shared source, or any other file whose hunks are two sides adding different lines at one
        # anchor. A hunk that edits shared text goes to the operator.
        # union_hunks refuses a non-additive result via its three-way brace check, so no separate
        # can_union gate is needed here.
        fp = Path(wt) / p
        if fp.exists() and union_hunks(wt, p):
            resolved.append(p + " (union)")
            continue
        left.append(p)
    return resolved, left


def _resolve_abi_version(wt, rel):
    """Resolve an fc4.h / lib.rs conflict.

    Every lane appends its own ` * <n> = ...` history entry and bumps the constant just before it, so the
    contested hunk holds both sides' history lines *and* the version macro. Keep the comment lines from both
    sides (ours first, duplicates dropped) and exactly one macro line, at the higher number.
    `consolidate_abi_history` then turns the batch's entries into the single per-batch history line.
    """
    path = Path(wt) / rel
    raw = path.read_text(encoding="utf-8", errors="replace")
    if "<<<<<<<" not in raw:
        return False
    is_h = rel.endswith(".h")
    regex = VERSION_RE_C if is_h else VERSION_RE_RS
    out, i, n = [], 0, len(raw)
    while i < n:
        start = raw.find("<<<<<<<", i)
        if start < 0:
            out.append(raw[i:])
            break
        out.append(raw[i:start])
        mid = raw.find("=======", start)
        end = raw.find(">>>>>>>", mid) if mid >= 0 else -1
        if mid < 0 or end < 0:
            return False
        body_o = raw[start + 7:mid]
        body_t = raw[mid + 7:end]
        nl = chr(10)
        body_o = body_o.split(nl, 1)[1] if nl in body_o else ""
        body_t = body_t.split(nl, 1)[1] if nl in body_t else ""
        lines_o, lines_t = body_o.splitlines(), body_t.splitlines()
        vo, vt = regex.search(body_o), regex.search(body_t)

        def commentish(x):
            s = x.strip()
            return (not s) or x.lstrip().startswith("*") or (s.startswith("/*") and s.endswith("*/")) \
                or s == "*/" or bool(regex.search(x))

        if [x for x in lines_o + lines_t if not commentish(x)]:
            return False
        hist, seen = [], set()
        for x in lines_o + lines_t:
            if x not in seen and not regex.search(x):
                seen.add(x)
                hist.append(x)
        hist = close_comment(hist)
        # The version macro is often NOT in the hunk: lanes that pick the same number merge cleanly and only
        # their history entries collide. Comment-only hunks union; a hunk that carries the macro keeps one
        # copy at the higher number.
        if (vo or vt):
            best = max(int(m.group(2)) for m in (vo, vt) if m)
            if is_h:
                out.append(nl.join(hist + [f"#define FC4_ABI_VERSION {best}u"]))
            else:
                out.append(f"pub const FC4_ABI_VERSION: u32 = {best};")
        else:
            if not hist:
                return False
            out.append(nl.join(hist))
        i = raw.find(nl, end)
        i = n if i < 0 else i + 1
    text = "".join(out)
    if "<<<<<<<" in text or ">>>>>>>" in text:
        return False
    path.write_text(text, encoding="utf-8")
    git(["-C", str(wt), "add", rel], check=False)
    return True


def history_span(text, regex):
    """Line range of the ABI history block: from the first ` * <n> = ` through the `#define FC4_ABI_VERSION`.

    `parse_history` stops at the first non-comment line, which is wrong for a unioned file where the macro
    can sit in the middle of the entry list. Anchoring the end on the macro itself makes the span robust to
    whatever a union leaves behind.
    """
    lines = text.split(NL)
    start = None
    for i, ln in enumerate(lines):
        if re.match(r"^ \* \d+ = ", ln):
            start = i
            break
    if start is None:
        return None, None
    end = None
    for i in range(start, len(lines)):
        if regex.match(lines[i]):
            end = i
            break
    if end is None:
        _s, e, _en = parse_history(text)
        end = (e - 1) if e else start
    return start, end + 1


def close_comment(lines):
    """Strip any trailing `*/` from comment lines and close the block exactly once, on the last line.

    Every lane's history entry ends with `*/` because it is the final entry in the block. Unioning two of
    them, or keeping main's last entry above a new one, would close the comment in the middle and turn the
    rest into stray source.
    """
    closed = False
    out = []
    for x in lines:
        if x.rstrip().endswith("*/"):
            closed = True
            x = x.rstrip()[:-2].rstrip()
        out.append(x)
    if closed and out:
        out[-1] = (out[-1] + " */") if out[-1].strip() else " */"
    return out


def parse_history(text):
    """Split fc4.h's version history comment into whole entries.

    An entry starts at ` * <n> = ` and continues over the ` *`-prefixed lines that follow it. Returns
    (start, end, [(number, lines), ...]) over the region that holds the entries, or (None, None, []).
    """
    lines = text.splitlines()
    first = None
    for i, ln in enumerate(lines):
        if re.match(r"^ \* \d+ = ", ln):
            first = i
            break
    if first is None:
        return None, None, []
    last = first
    entries, cur = [], None
    for i in range(first, len(lines)):
        ln = lines[i]
        m = re.match(r"^ \* (\d+) = (.*)$", ln)
        if m:
            if cur is not None:
                entries.append(cur)
            cur = (int(m.group(1)), [ln])
            last = i
            continue
        if ln.startswith(" *"):
            if cur is not None:
                cur[1].append(ln)
                last = i
            continue
        break
    if cur is not None:
        entries.append(cur)
    return first, last + 1, entries


def consolidate_abi_history(wt, lanes, batch_id, force=False):
    """Write ONE fc4.h history line for the batch and set the constant.

    Each lane appends its own ` * <n> = ...` entry and bumps FC4_ABI_VERSION. Rather than parse the merged
    block (a union of lanes with different line wrapping is not reconstructible), this rebuilds the block
    from main's copy verbatim, then appends one entry describing the batch, with each lane's own entry text
    taken from that lane's original fc4.h. Returns the new number.
    """
    main_header = git(["show", f"main:{ABI_HEADER}"], check=False).stdout
    _s, _e, main_entries = parse_history(main_header or "")
    if not main_entries:
        return None
    main_ver = 0
    mm = VERSION_RE_C.search(main_header or "")
    if mm:
        main_ver = int(mm.group(2))

    def key(lines):
        s = " ".join(" ".join(lines).split())
        # main's last entry carries the comment close; a lane's copy of the same entry does not
        return s[:-2].rstrip() if s.endswith("*/") else s

    main_keys = {key(ls) for _n, ls in main_entries}
    descs, numbers = [], [main_ver]
    for lane, branch in lanes:
        src = git(["show", f"{branch}:{ABI_HEADER}"], check=False)
        if src.returncode != 0:
            continue
        _a, _b, entries = parse_history(src.stdout)
        mine = []
        for num, ls in entries:
            if num > main_ver and key(ls) not in main_keys:
                parts = []
                for ln in ls:
                    s = ln.strip()
                    s = s[1:].strip() if s.startswith("*") else s
                    parts.append(s)
                desc = " ".join(parts)
                desc = re.sub(r"^\d+ = ", "", desc).split(" */")[0].strip()
                mine.append(desc)
                numbers.append(num)
        if mine:
            descs.append("%s: %s" % (lane, " ".join(mine)))
    if not descs and not force:
        return None
    if not descs:
        descs = ["merged (no C ABI history entry of its own)"]
    new = max([main_ver + 1] + numbers)
    body = []
    for _n, ls in main_entries:
        body.extend(ls)
    body.append(" * %d = merge-train batch %s (%s): %s"
                % (new, batch_id, ", ".join(l for l, _b in lanes), " | ".join(descs) or "no C ABI change"))
    body = close_comment(body)
    path = Path(wt) / ABI_HEADER
    text = path.read_text(encoding="utf-8", errors="replace")
    _as, _ae = history_span(text, VERSION_RE_C)
    if _as is None:
        return None
    lines = text.split(NL)
    # The rewrite must touch nothing outside the history block. A bug here deleted every
    # FC4_ADV_SRC_* define and 300 declarations from the header once; refuse rather than repeat it.
    outside_before = [l for k, l in enumerate(text.split(NL))
                      if not (_as <= k < _ae) and not l.lstrip().startswith("*")]
    body.append("#define FC4_ABI_VERSION %du" % new)
    rest = lines[_ae:]
    # A union can leave one lane's history entries *after* the macro. They start with ` *` and belong to the
    # block we just rebuilt, so drop them; stop at anything else (blank lines and the next real comment).
    while rest and rest[0].strip() == "":
        rest.pop(0)
    while rest and rest[0].lstrip().startswith("*") and not rest[0].lstrip().startswith("/*"):
        rest.pop(0)
    while rest and rest[0].strip() == "":
        rest.pop(0)
    rebuilt = lines[:_as] + body + rest
    outside_after = [l for l in (lines[:_as] + rest) if not l.lstrip().startswith("*")]
    if len(outside_after) != len(outside_before):
        raise RuntimeError(
            f"consolidate_abi_history would change {abs(len(outside_after) - len(outside_before))} "
            f"line(s) outside the ABI history block of {ABI_HEADER}; refusing")
    text = NL.join(rebuilt)
    # A union can leave the version macro twice (one per side); the C macro would then be the
    # second value while the Rust constant is the first, and capi_smoke fails on the mismatch.
    defs = [i for i, l in enumerate(text.split(NL)) if VERSION_RE_C.match(l)]
    if len(defs) > 1:
        tl = text.split(NL)
        for i in reversed(defs[1:]):
            if "/*" in tl[i]:
                tl[i] = tl[i][tl[i].index("/*"):]
            else:
                del tl[i]
        text = NL.join(tl)
    text = VERSION_RE_C.sub(lambda x: "%s%d%s" % (x.group(1), new, x.group(3)), text, count=1)
    path.write_text(text, encoding="utf-8")
    git(["-C", str(wt), "add", ABI_HEADER], check=False)
    rs = Path(wt) / ABI_RS
    if rs.exists():
        rt = rs.read_text(encoding="utf-8", errors="replace")
        if VERSION_RE_RS.search(rt):
            rs.write_text(VERSION_RE_RS.sub(lambda x: "%s%d%s" % (x.group(1), new, x.group(3)), rt, count=1),
                          encoding="utf-8")
            git(["-C", str(wt), "add", ABI_RS], check=False)
    log("ABI: one history line for %s at %d (%d lane entries folded in)"
        % (batch_id, new, len(descs)))
    return new


def bump_abi(wt, lanes, batch_id):
    """One ABI bump per batch. Prefers tools/bump_abi.py (abi-hygiene); falls back to a local bump."""
    script = REPO / "tools" / "bump_abi.py"
    if script.exists():
        p = sh([sys.executable, str(script), "--batch", batch_id, "--lanes", ",".join(lanes)], cwd=Path(wt), check=False)
        sys.stdout.write(p.stdout or "")
        if p.returncode == 0:
            return True
        log("tools/bump_abi.py failed; falling back to the built-in bump")
    changed = False
    for rel, regex in ((ABI_HEADER, VERSION_RE_C), (ABI_RS, VERSION_RE_RS)):
        path = Path(wt) / rel
        if not path.exists():
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        m = regex.search(text)
        if not m:
            continue
        old = int(m.group(2))
        new = old + 1
        text = regex.sub(lambda mm: f"{mm.group(1)}{new}{mm.group(3)}", text, count=1)
        if rel.endswith(".h"):
            # one history line per batch, naming the lanes
            line = f" * {new} = merge-train batch {batch_id}: {', '.join(lanes)}.\n"
            close = text.find(" */\n#define FC4_ABI_VERSION")
            if close > 0:
                text = text[:close] + line + text[close:]
        path.write_text(text, encoding="utf-8")
        git(["-C", str(wt), "add", rel], check=False)
        changed = True
        log(f"ABI {old} -> {new} ({rel})")
    return changed


def regenerate(wt):
    """Rebuild every derived file after the merges, so no lane has to hand-merge them."""
    out = []
    gen = Path(wt) / "engine" / "tests" / "gen_abi_entries.py"
    tracked = git(["-C", str(wt), "ls-files", "--error-unmatch", ABI_INC], check=False).returncode == 0
    if gen.exists() and tracked:
        p = sh([sys.executable, str(gen)], cwd=Path(wt) / "engine", check=False)
        out.append(f"gen_abi_entries: exit {p.returncode}")
    tables = Path(wt) / "tools" / "gen_handoff_tables.py"
    if tables.exists():
        p = sh([sys.executable, str(tables)], cwd=Path(wt), check=False)
        out.append(f"gen_handoff_tables: exit {p.returncode}")
        if p.returncode != 0:
            out.append((p.stdout or "")[-1500:])
    questions = Path(wt) / "tools" / "questions.py"
    if questions.exists():
        # QUESTIONS.md is an index of the per-spec files: always rebuild it after the lanes' merges.
        p = sh([sys.executable, str(questions), "index"], cwd=Path(wt), check=False)
        out.append(f"questions index: exit {p.returncode}")
        c = sh([sys.executable, str(questions), "check"], cwd=Path(wt), check=False)
        if c.returncode != 0:  # a duplicated ID or an entry without one: report it, do not block the batch
            out.append("questions check: " + ((c.stdout or "") + (c.stderr or ""))[-1500:])
    return out


def commit_integration(wt, lanes, batch_id, note):
    git(["-C", str(wt), "add", "-A"])
    st = git(["-C", str(wt), "status", "--porcelain"], check=False).stdout.strip()
    if not st:
        return None
    msg = f"merge-train: resolve batch {batch_id} hotspots ({', '.join(lanes)})\n\n{note}\n\n" \
          "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n"
    msgfile = Path(wt) / "build-runs" / f"train-{batch_id}-resolve.txt"
    msgfile.parent.mkdir(parents=True, exist_ok=True)
    msgfile.write_text(msg, encoding="utf-8")
    git(["-C", str(wt), "commit", "-F", str(msgfile)])
    return git(["-C", str(wt), "rev-parse", "HEAD"]).stdout.strip()


# --- gate / regress ----------------------------------------------------------


BUILD_PREFIXES = ("engine/", "tools/", "tests/", ".claude/lane-bin/")


def build_relevant(paths):
    return [p for p in paths if p.startswith(BUILD_PREFIXES)]


def batch_touches_engine(wt):
    base = git(["-C", str(wt), "merge-base", "main", "HEAD"], check=False).stdout.strip()
    if not base:
        return True
    diff = git(["-C", str(wt), "diff", "--name-only", base, "HEAD"], check=False).stdout.split()
    return bool(build_relevant(diff))


def main_moved_under(head):
    """Build-relevant files main changed since the batch was cut from it. Landing then needs a real merge, and the
    merged tree is not the tree the gate built."""
    base = git(["merge-base", "main", head], check=False).stdout.strip()
    if not base:
        return ["(no merge base with main)"]
    return build_relevant(git(["diff", "--name-only", base, "main"], check=False).stdout.split())


def leftover_markers(root):
    """Conflict markers still in the tree. A gate on these only wastes twenty minutes."""
    skip = {".git", "node_modules", "target", "build-x86", "__pycache__", "build-runs"}
    exts = {".rs", ".cpp", ".h", ".hpp", ".c", ".py", ".md", ".json", ".txt", ".inc",
            ".toml", ".tsv", ".cmake", ".yml"}
    bad = []
    for path in root.rglob("*"):
        if not path.is_file() or path.suffix not in exts:
            continue
        parts = set(path.parts)
        if parts & skip:
            continue
        try:
            text = path.read_text(encoding="utf-8", errors="ignore")
        except OSError:
            continue
        # tools/merge-train.py itself carries these as string literals
        if path.name == "merge-train.py":
            continue
        if "<<<<<<<" in text or ">>>>>>>" in text:
            bad.append(str(path.relative_to(root)))
    return bad


GATE_STEPS = fc4verify.GATE_STEPS


def gate_record_path(tag):
    return LOG_DIR / f"gate-{tag}.json"


def load_gate_record(tag):
    try:
        return json.loads(gate_record_path(tag).read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None


def judge_gate(rec, head):
    """(ok, reasons) for a gate record against the integration head about to be used.

    The record must come from fc4verify, be for exactly `head`, have built a clean tree (no uncommitted or untracked
    file the commit would not carry) and pass when its step logs are parsed again. Nothing else counts: not the
    state file's `gate.ok` (train10 landed on a hand-set `ok: true` after its gate had failed in `cargo test`, before
    the C++ build ever ran), not a wrapper's exit code (coord-verify-lane.ps1 exits 0 when cmake fails)."""
    if not rec:
        return False, ["no gate record"]
    why = []
    if rec.get("tool") != "fc4verify":
        why.append("not an fc4verify gate record")
    if rec.get("head") != head:
        why.append(f"the gate built {str(rec.get('head'))[:10]}, the integration branch is at {head[:10]}")
    if rec.get("dirty"):
        why.append(f"the gate built a tree with {len(rec['dirty'])} uncommitted/untracked file(s): "
                   + ", ".join(rec["dirty"][:5]))
    if rec.get("changed_during_run"):
        why.append("the tree changed while the gate ran")
    ok, reasons = fc4verify.evaluate_record(rec, GATE_STEPS)
    why.extend(reasons)
    return not why, why


def run_gate(wt, tag):
    """Post-merge clippy + CMake build + full ctest (cargo_test included) of the integration tree. Returns (ok, tail).

    fc4verify runs the steps and parses each one (ninja's progress and FAILED edges, the ctest summary against the
    registered test count, clippy's error lines); the record goes to build-runs/gate-<tag>.json and `land`/`regress`
    judge that record again for the exact head they use. Every step runs even after a failure, so a red gate shows
    the whole picture (train10's stopped at `cargo test` and nobody saw that the C++ did not compile).
    """
    engine = Path(wt) / "engine"
    if not engine.is_dir():
        return True, "no engine/ in this batch"
    bad = leftover_markers(Path(wt))
    if bad:
        why = "leftover conflict markers in " + ", ".join(bad[:12])
        _write_gate_log(tag, [f"=== gate verdict: FAIL ({why})"])
        return False, why
    log(f"gate {tag}: clippy, CMake build, full ctest (fc4verify; this takes a while)")
    rec = fc4verify.verify(wt, ["gate"], log_dir=LOG_DIR / "gate-logs" / tag, tag=f"fc4gate-{tag}", quiet=True)
    gate_record_path(tag).write_text(json.dumps(rec, indent=2) + "\n", encoding="utf-8")
    ok, why = judge_gate(rec, rec["head"])
    lines = [f"gate {tag} at {rec['head'][:12]} ({rec['seconds']} s, record {gate_record_path(tag)})"]
    for name in GATE_STEPS:
        st = rec["steps"].get(name) or {}
        lines.append(f"=== {name}: {'PASS' if st.get('ok') else 'FAIL'} {st.get('summary', 'not run')}"
                     f"  ({st.get('seconds', 0)} s; log {st.get('log')})")
        for e in (st.get("errors") or [])[:15] + [f"FAILED {t}" for t in (st.get("failed_tests") or [])[:15]]:
            lines.append(f"    {e[:220]}")
    lines.append(f"=== gate verdict: {'PASS' if ok else 'FAIL'} ({'; '.join(why) or 'clippy, build, ctest clean'})")
    _write_gate_log(tag, lines)
    tail = "\n".join(lines)
    return ok, tail


def _write_gate_log(tag, results):
    LOG_DIR.mkdir(parents=True, exist_ok=True)
    (LOG_DIR / f"gate-{tag}.out").write_text("\n".join(results), encoding="utf-8")


def gated_dll(cur, wt):
    """The DLL the passing gate built for the current head, or (None, why)."""
    head = git(["-C", str(wt), "rev-parse", "HEAD"]).stdout.strip()
    rec = load_gate_record(cur["id"])
    ok, why = judge_gate(rec, head)
    if not ok:
        return None, "; ".join(why)
    dll = ((rec["steps"].get("build") or {}).get("dll") or {})
    path = Path(dll.get("path", ""))
    if not path.is_file():
        return None, f"no DLL at {path}"
    if fc4verify.sha256(path) != dll.get("sha256"):
        return None, f"{path} is not the DLL the gate built (rebuilt or replaced since)"
    return path, ""


def replacement_union(extra, script=None):
    """Append candidate keys to every default replacement, before promoting the default list."""
    if extra is None:
        return None
    keys = [k.strip().lower() for k in extra.split(",") if k.strip()]
    if not keys or any(not re.fullmatch(r"[a-z][a-z0-9_]*", k) for k in keys):
        raise ValueError("--add-replace needs comma-separated subsystem keys")
    try:
        text = (script or REPO / "tools" / "regress.ps1").read_text(encoding="utf-8-sig")
    except (OSError, UnicodeError) as exc:
        raise ValueError("cannot read the default replacement list") from exc
    match = re.search(r'(?m)^\s*\[string\]\$Replace\s*=\s*"([^"]+)"', text)
    if not match:
        raise ValueError("cannot find the default replacement list")
    defaults = [k.strip().lower() for k in match.group(1).split(",") if k.strip()]
    if not defaults or any(not re.fullmatch(r"[a-z][a-z0-9_]*", k) for k in defaults):
        raise ValueError("invalid default replacement list")
    return ",".join(dict.fromkeys(defaults + keys))


def run_regress(wt, tag, dll, replace=None):
    """Submit the two regress runs through the game queue and wait for the verdict."""
    log(f"regress {tag}: submitting to the game queue (about 10-20 min)")
    command = [
        "powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", str(REPO / "tools" / "regress.ps1"),
        "-Tag", tag, "-Dll", str(dll), "-Queue", "-Turbo", "-TurboTimeout", "1800", "-QueueWait", "5400",
    ]
    if replace is not None:
        command += ["-Replace", replace]
    p = sh(command, check=False, timeout=7200)
    out = (p.stdout or "") + (p.stderr or "")
    (LOG_DIR / f"regress-{tag}.out").write_text(out, encoding="utf-8")
    m = re.search(r"REGRESS\s+\S+:\s*(PASS|FAIL)", out)
    verdict = m.group(1) if m else "FAIL"  # no REGRESS line: never a pass, whatever the exit code
    return verdict, out[-4000:]


# --- batch lifecycle ---------------------------------------------------------


def cmd_merge_batch(args):
    if current_branch() != "main":
        die(f"run this from main (currently on {current_branch()})")
    if not main_is_clean():
        die("main has uncommitted changes; commit or stash them first")
    batch, _all = pick_batch(args.size, getattr(args, "lanes", None))
    if not batch:
        log("nothing ready")
        return 0
    st = load_state()
    n = st["batch_seq"] + 1
    batch_id = f"train{n}"
    integ = f"integration/{n}"
    lanes = [r["lane"] for r in batch]
    log(f"batch {batch_id}: {', '.join(lanes)}")
    wt = reset_integration(integ)
    # main's copy of fc4.h before the batch: consolidate_abi_history diffs the history block against it
    pre = git(["show", f"main:{ABI_HEADER}"], check=False)
    main_header = pre.stdout if pre.returncode == 0 else ""
    merged, skipped = [], []
    for r in batch:
        branch = r["branch"]
        log(f"  merge {r['lane']} ({branch} @ {r['tip'][:10]})")
        p = git(["-C", str(wt), "merge", "--no-ff", "--no-commit", branch],
                check=False, cwd=wt)
        if p.returncode != 0:
            resolved, left = resolve_conflicts(wt, [r["lane"]])
            if left:
                log(f"  ! {r['lane']}: unresolved conflicts in {left}")
                skipped.append((r["lane"], left))
                # `merge --abort` refuses when a build artifact the lane still tracks (abi_entries.inc, untracked
                # on main since 46f3b7cb) differs on disk; a failed abort left the conflict in the index and every
                # later lane of train6 then "conflicted" in the same files. The integration branch is ours: reset.
                if git(["-C", str(wt), "merge", "--abort"], check=False).returncode != 0:
                    git(["-C", str(wt), "reset", "--hard", "HEAD"], check=False)
                continue
            if resolved:
                log(f"  resolved hotspots: {', '.join(resolved)}")
        msgfile = Path(wt) / "build-runs" / f"train-{batch_id}-{r['lane']}-merge.txt"
        msgfile.parent.mkdir(parents=True, exist_ok=True)
        msgfile.write_text(
            f"Merge {r['lane']} into {integ} (merge-train batch {batch_id})\n\n"
            f"{r.get('summary', '')}\n\n"
            f"branch: {branch}\nleaf: {r['tip'][:12]}\nkind: {r['kind']}\n\n"
            "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n",
            encoding="utf-8",
        )
        git(["-C", str(wt), "commit", "-F", str(msgfile)], check=False)
        merged.append(r["lane"])
    if not merged:
        die("no lane merged; aborting the batch")
    need_abi = any(r["abi"] for r in batch if r["lane"] in merged)
    note = []
    lane_pairs = [(r["lane"], r["branch"]) for r in batch if r["lane"] in merged]
    abi_n = consolidate_abi_history(wt, lane_pairs, batch_id, force=need_abi)
    if abi_n is not None:
        note.append(f"ABI: one history line for the batch at {abi_n}")
    elif need_abi:
        bump_abi(wt, merged, batch_id)
        note.append("ABI bumped once for the batch")
    note.extend(regenerate(wt))
    commit_integration(wt, merged, batch_id, "\n".join(note) or "derived files regenerated")
    head = git(["-C", str(wt), "rev-parse", "HEAD"]).stdout.strip()
    st["batch_seq"] = n
    st["current"] = {
        "id": batch_id,
        "integration": integ,
        "worktree": str(TRAIN_WT),
        "lanes": merged,
        "skipped": [{"lane": a, "conflicts": b} for a, b in skipped],
        "head": head,
        "gate": None,
        "regress": None,
    }
    st.setdefault("history", [])
    save_state(st)
    log(f"batch {batch_id} merged at {head[:10]} on {integ} ({len(merged)} lanes)")
    for a, b in skipped:
        log(f"  SKIPPED {a}: conflicts in {b}")
    print(json.dumps({"batch": batch_id, "integration": integ, "merged": merged,
                      "skipped": [{"lane": a, "conflicts": b} for a, b in skipped], "head": head}, indent=2))
    return 0


def integration_head(wt):
    return git(["-C", str(wt), "rev-parse", "HEAD"]).stdout.strip()


def cmd_gate(_args):
    st = load_state()
    cur = st.get("current")
    if not cur:
        die("no current batch; run merge-batch first")
    wt = Path(cur["worktree"])
    head = integration_head(wt)
    if head != cur.get("head"):
        log(f"integration moved since merge-batch ({str(cur.get('head'))[:10]} -> {head[:10]}); gating {head[:10]}")
        cur["head"] = head
    if not batch_touches_engine(wt):
        log("docs-only batch: no gate needed")
        cur["gate"] = {"ok": True, "skipped": "docs-only", "head": head}
    else:
        ok, tail = run_gate(wt, cur["id"])
        # informational only: land and regress judge build-runs/gate-<id>.json again, never this flag
        cur["gate"] = {"ok": ok, "head": head, "record": str(gate_record_path(cur["id"]))}
        log(f"gate {cur['id']}: {'PASS' if ok else 'FAIL'}")
        if not ok:
            log(tail[-1500:])
    save_state(st)
    return 0 if cur["gate"]["ok"] else 1


def cmd_regress(args):
    st = load_state()
    cur = st.get("current")
    if not cur:
        die("no current batch; run merge-batch first")
    wt = Path(cur["worktree"])
    head = integration_head(wt)
    if args.no_regress or not batch_touches_engine(wt):
        log("no regress for this batch")
        cur["regress"] = {"ok": True, "skipped": "docs/tools-only" if not args.no_regress else "--no-regress",
                          "head": head}
        save_state(st)
        return 0
    # Only the DLL the passing gate built for this head: train10's regress "passed" on a FreeCity4.dll left in
    # build-x86 from an earlier batch (built 15:21, the merge was 17:08), because its gate had never built one.
    dll, why = gated_dll(cur, wt)
    if dll is None:
        log(f"regress {cur['id']}: refused, no gated DLL for {head[:10]}: {why}")
        cur["regress"] = {"ok": False, "verdict": "REFUSED", "why": why, "head": head}
        save_state(st)
        return 1
    try:
        replace = replacement_union(getattr(args, "add_replace", None))
    except ValueError as exc:
        cur["regress"] = {"ok": False, "verdict": "REFUSED", "why": str(exc), "head": head}
        save_state(st)
        log(f"regress {cur['id']}: refused: {exc}")
        return 1
    # An interrupted retry must not leave a previous PASS in place.
    cur["regress"] = {"ok": False, "verdict": "RUNNING", "head": head}
    if replace is not None:
        cur["regress"]["replace"] = replace
    save_state(st)
    verdict, tail = run_regress(wt, cur["id"], dll, replace)
    cur["regress"].update(ok=verdict == "PASS", verdict=verdict)
    save_state(st)
    log(f"regress {cur['id']}: {verdict}")
    if verdict != "PASS":
        log(tail[-1500:])
    return 0 if verdict == "PASS" else 1


def write_notices(cur, main_sha):
    NOTICE_DIR.mkdir(parents=True, exist_ok=True)
    for lane in cur["lanes"]:
        f = NOTICE_DIR / f"{lane}.md"
        f.write_text(
            f"# merged: {lane}\n\n"
            f"- branch `{lane}` merged into `main` as `{main_sha}` in merge-train batch `{cur['id']}`.\n"
            f"- integration: `{cur['integration']}` at `{cur['head'][:12]}`\n"
            f"- gate: {cur.get('gate')}   regress: {cur.get('regress')}\n\n"
            "Nothing more to do on this lane. Any `docs/lanes/<lane>.md` and `docs/runs/<tag>.md` you wrote are now\n"
            "the source of the generated docs/lanes/INDEX.md / HANDOFF / manual-checks tables.\n",
            encoding="utf-8",
        )
    # one copy in the main tree as well, so a later session can see what landed
    summary = NOTICE_DIR / f"{cur['id']}.md"
    summary.write_text(
        f"# merge-train batch {cur['id']}\n\n"
        f"- lanes: {', '.join(cur['lanes'])}\n"
        f"- skipped: {cur.get('skipped')}\n"
        f"- gate: {cur.get('gate')}\n- regress: {cur.get('regress')}\n"
        f"- main now at `{main_sha}`\n",
        encoding="utf-8",
    )


def land_blockers(cur, head, touches_build, record, main_moved):
    """Why `land` must refuse (empty: it may land). Pure, so the train10 case is a unit test.

    `touches_build` and `main_moved` are recomputed from git at land time and `record` is read from
    build-runs/gate-<id>.json; the state file's gate flag is never consulted."""
    why = []
    if touches_build:
        ok, reasons = judge_gate(record, head)
        if not ok:
            why.append("no passing post-merge gate for this head: " + "; ".join(reasons))
    if main_moved:
        why.append("main changed build files since the batch was cut (" + ", ".join(main_moved[:5])
                   + "): the merged tree would not be the gated one; run merge-batch again")
    return why


def cmd_land(args):
    if current_branch() != "main":
        die("run this from main")
    if not main_is_clean():
        die("main has uncommitted changes")
    st = load_state()
    cur = st.get("current")
    if not cur:
        die("no current batch")
    wt = Path(cur["worktree"])
    head = integration_head(wt)
    touches = batch_touches_engine(wt)
    blockers = land_blockers(cur, head, touches, load_gate_record(cur["id"]) if touches else None,
                             main_moved_under(head))
    if blockers:
        die("refusing to land " + cur["id"] + ":\n  - " + "\n  - ".join(blockers)
            + "\nThere is no override: fix the tree on the integration branch, commit, `gate` again.")
    reg = cur.get("regress") or {}
    if not (reg.get("ok") and reg.get("head") == head):
        if not args.force:
            die("regress did not pass for this head; use bisect, or land with --force")
        log(f"landing without a passing regress for {head[:10]} (--force)")
    log(f"landing {cur['id']} ({head[:10]}) onto main")
    # fast-forward main to the integration branch
    p = git(["merge", "--ff-only", head], check=False)
    if p.returncode != 0:
        log("fast-forward refused; merging with --no-ff instead")
        git(["merge", "--no-ff", "-m",
             f"Merge merge-train batch {cur['id']}: {', '.join(cur['lanes'])}\n\n"
             "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\n", head])
    main_sha = git(["rev-parse", "HEAD"]).stdout.strip()
    write_notices(cur, main_sha)
    st.setdefault("history", []).append({**cur, "main": main_sha, "landed_at": time.strftime("%Y-%m-%d %H:%M:%S")})
    st["current"] = None
    save_state(st)
    # the queue files for landed lanes go away so they are not merged twice
    for lane in cur["lanes"]:
        for f in queue_files():
            if f.stem == lane:
                dst = f.with_suffix(".merged.json")
                if dst.exists():  # a stale marker from a batch that crashed mid-land
                    dst.unlink()
                f.rename(dst)
    log(f"main is now {main_sha[:10]}; batch {cur['id']} landed")
    print(json.dumps({"main": main_sha, "lanes": cur["lanes"], "batch": cur["id"]}, indent=2))
    return 0


def cmd_bisect(_args):
    """Split the current batch and gate the halves until one culprit lane is isolated.

    The halves are gated one after the other in the integration worktree (incremental builds), each judged by its
    parsed fc4verify record. The earlier version gated both halves in parallel in the same worktree, so the second
    reset wiped the first half's tree under its running build, and took coord-verify-lane.ps1's exit code (always 0)
    as the verdict: every half "passed"."""
    st = load_state()
    cur = st.get("current")
    if not cur:
        die("no current batch")
    lanes = cur["lanes"]
    if len(lanes) < 2:
        die(f"batch has a single lane ({lanes}); the culprit is already isolated")
    log(f"bisecting {cur['id']}: {lanes}")
    halves = [lanes[: len(lanes) // 2], lanes[len(lanes) // 2 :]]
    bad = None
    wt = None
    for i, half in enumerate(halves):
        name = f"{cur['id']}-b{i}"
        wt = reset_integration(f"integration/{name}")
        for lane in half:
            rec = next((r for r in read_queue() if r["lane"] == lane), None)
            branch = (rec or {}).get("branch", lane)
            git(["-C", str(wt), "merge", "--no-ff", "--no-commit", branch], check=False)
            resolve_conflicts(wt, [lane])
            git(["-C", str(wt), "commit", "--no-edit"], check=False)
        log(f"  half {i}: {half} -> gating as {name}")
        ok, _tail = run_gate(wt, name)
        log(f"  half {name}: {'PASS' if ok else 'FAIL'} ({half}; build-runs/gate-{name}.out)")
        if not ok:
            bad = half
            break
    # put the batch's own branch back, so `gate`/`land` never act on a bisect half by accident
    if wt is not None:
        git(["-C", str(wt), "checkout", "--force", cur["integration"]], check=False)
    if bad is None:
        log("both halves pass: the failure is in how they interact, or it was flaky. Rerun the full batch.")
        return 1
    cur["lanes"] = bad
    cur["bisect"] = cur.get("bisect", []) + [{"half": bad, "count": len(bad)}]
    save_state(st)
    log(f"culprit is in {bad}; run 'bisect' again (or 'fail --lane' when it is one lane)")
    return 0


def cmd_fail(args):
    st = load_state()
    cur = st.get("current")
    if not cur:
        die("no current batch")
    lane = args.lane
    if lane not in cur["lanes"]:
        die(f"{lane} is not in the current batch")
    wt = lane_worktree(lane)
    body = (
        f"# merge-train: batch {cur['id']} failed on {lane}\n\n"
        f"The gate/regress for the batch failed and bisection isolated {lane}. The failure output is in\n"
        f"`build-runs/gate-{cur['id']}.out` and `build-runs/regress-{cur['id']}.out` in the main checkout.\n\n"
        "Fix it on your branch and write a fresh `build-runs/merge-queue/<lane>.json` when it is green.\n"
    )
    if wt:
        (wt / "build-runs").mkdir(parents=True, exist_ok=True)
        (wt / "build-runs" / f"merge-failed-{cur['id']}.md").write_text(body, encoding="utf-8")
        log(f"wrote {wt}/build-runs/merge-failed-{cur['id']}.md")
    cur["lanes"] = [x for x in cur["lanes"] if x != lane]
    cur.setdefault("rejected", []).append(lane)
    save_state(st)
    # The integration branch still holds the culprit's merge, and its gate record failed: `land` refuses it.
    log(f"{lane} sent back; {len(cur['lanes'])} lane(s) remain in {cur['id']}. Land them as a new batch: "
        f"merge-batch --lanes {','.join(cur['lanes'])}, then gate")
    return 0


def cmd_run(args):
    """merge-batch -> gate -> regress -> land, with bisection on failure."""
    for step, fn in (("merge-batch", lambda: cmd_merge_batch(args)),
                     ("gate", lambda: cmd_gate(args)),
                     ("regress", lambda: cmd_regress(args))):
        log(f"--- {step}")
        rc = fn()
        if rc != 0:
            if step == "merge-batch":
                return rc
            log(f"{step} failed; bisecting")
            if cmd_bisect(args) == 0:
                return 2  # caller (the operator) continues with the isolated half
            return rc
    if args.land:
        return cmd_land(args)
    log("batch is green; run 'land' to put it on main")
    return 0


# --------------------------------------------------------------------------- cli


def main():
    ap = argparse.ArgumentParser(prog="merge-train", description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("status")
    p = sub.add_parser("plan")
    p.add_argument("--size", type=int, default=8)
    p.add_argument("--lanes", default=None, help="comma-separated: batch exactly these queued lanes")
    p = sub.add_parser("enqueue")
    p.add_argument("--lane", required=True)
    p.add_argument("--branch", required=True)
    p.add_argument("--tip", default="")
    p.add_argument("--kind", choices=KINDS, default="engine")
    p.add_argument("--abi", action="store_true")
    p.add_argument("--game-runs", default="")
    p.add_argument("--summary", default="")
    p = sub.add_parser("merge-batch")
    p.add_argument("--size", type=int, default=8)
    p.add_argument("--lanes", default=None, help="comma-separated: batch exactly these queued lanes")
    sub.add_parser("gate")
    p = sub.add_parser("regress")
    profile = p.add_mutually_exclusive_group()
    profile.add_argument("--no-regress", action="store_true")
    profile.add_argument("--add-replace", help="test candidate keys in addition to every default replacement")
    p = sub.add_parser("land")
    p.add_argument("--force", action="store_true")
    sub.add_parser("bisect")
    p = sub.add_parser("fail")
    p.add_argument("--lane", required=True)
    p = sub.add_parser("run")
    p.add_argument("--size", type=int, default=8)
    p.add_argument("--lanes", default=None, help="comma-separated: batch exactly these queued lanes")
    p.add_argument("--land", action="store_true")
    p.add_argument("--no-regress", action="store_true")
    args = ap.parse_args()
    return {"status": cmd_status, "plan": cmd_plan, "enqueue": cmd_enqueue,
            "merge-batch": cmd_merge_batch, "gate": cmd_gate, "regress": cmd_regress,
            "land": cmd_land, "bisect": cmd_bisect, "fail": cmd_fail, "run": cmd_run}[args.cmd](args)


if __name__ == "__main__":
    sys.exit(main())
