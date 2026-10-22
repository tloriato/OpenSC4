#!/usr/bin/env python3
"""Lane tooling: spawn cap, spawn checks, blocking waits, event monitor, token ledger, lane briefs.

Standard library only. The goal is fewer wasted agent tokens: one tool call instead of polling turns (wait), one
line per event instead of re-reading status (monitor), no spawn when the machine is full or the dependencies have
not landed (slots, check), measured cost per lane (ledger) and a ready-made context pack per lane (brief).

  python tools/lanes.py slots   [--game-slots N] [-v]
  python tools/lanes.py check   <lane> [--deps a,b] [--touches k1,k2] [--kind engine|re|tool|mech]
  python tools/lanes.py wait    (--verdict <label> | --file <path> [--pattern <regex>]) [--timeout 540]
  python tools/lanes.py monitor [--interval 30]
  python tools/lanes.py ledger  [--out build-runs/lane-ledger.tsv] [--md docs/token-ledger.md]
  python tools/lanes.py brief   <subsystem-or-key> [--kind engine|re]
  python tools/lanes.py stop    "<reason>"      (the STOP flag: no lane starts or restarts, running lanes hand off)
  python tools/lanes.py go                       (clears it)
  python tools/lanes.py unpark  <lane>           (release a lane the guard parked)
  python tools/lanes.py guard   [<lane>]         (lane-guard state: calls since commit, trips, parked lanes)

Global option --repo <main checkout> reads another checkout's build-runs, worktrees and docs (read-only use from a
lane or from a test); without it the checkout that holds this file is used, and the main checkout is found through
git when that is a lane worktree. See docs/lane-tools.md.
"""
import argparse
import collections
import concurrent.futures
import datetime
import fnmatch
import glob
import json
import os
import re
import shutil
import statistics
import subprocess
import sys
import time
from pathlib import Path

SCRIPT_ROOT = Path(__file__).resolve().parents[1]
NOWIN = getattr(subprocess, "CREATE_NO_WINDOW", 0)
GIT_ENV = dict(os.environ, GIT_OPTIONAL_LOCKS="0")
# directories that hold build caches or other bulk output: never part of "did this lane touch a file"
PRUNE = {".git", "target", "node_modules", "__pycache__", "dlls", "sc4slots"}
STALL_MIN = 60          # monitor: minutes without a file change before a lane is called stalled
ACTIVE_MIN = 30         # slots: a worktree with a file changed within this many minutes is working


def laneguard():
    """.claude/lane-bin/laneguard.py (the PreToolUse lane guard): the STOP flag, parking and the REA pre-flight live
    there so the hook, the workflow's gate agent and this tool read the same files."""
    import importlib.util
    spec = importlib.util.spec_from_file_location("laneguard", SCRIPT_ROOT / ".claude" / "lane-bin" / "laneguard.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


# ---------------------------------------------------------------------------------------------------------------
# small helpers
# ---------------------------------------------------------------------------------------------------------------

def out(line=""):
    sys.stdout.write(line + "\n")
    sys.stdout.flush()


def git(cwd, *args, timeout=120):
    try:
        p = subprocess.run(["git", *args], cwd=str(cwd), capture_output=True, text=True, encoding="utf-8",
                           errors="replace", timeout=timeout, creationflags=NOWIN, env=GIT_ENV)
        return p.returncode, p.stdout
    except (OSError, subprocess.SubprocessError):
        return 127, ""


def is_ancestor(cwd, rev, of="main"):
    rc, _ = git(cwd, "merge-base", "--is-ancestor", rev, of)
    return rc == 0


def decode_bytes(b):
    """Text of a file as written by PowerShell (UTF-16 with BOM, or UTF-16 without one) or by anything else."""
    if b[:3] == b"\xef\xbb\xbf":
        return b[3:].decode("utf-8", "replace")
    if b[:2] in (b"\xff\xfe", b"\xfe\xff"):
        return b.decode("utf-16", "replace")
    if b"\x00" in b[:4096]:
        even = sum(1 for x in b[1:4096:2] if x == 0)
        odd = sum(1 for x in b[0:4096:2] if x == 0)
        return b.decode("utf-16-be" if odd > even else "utf-16-le", "replace")
    return b.decode("utf-8", "replace")


def read_text(path):
    try:
        return decode_bytes(Path(path).read_bytes())
    except OSError:
        return None


def find_main_root(src):
    """The main checkout of the repository `src` belongs to (src itself when it is the main checkout)."""
    rc, o = git(src, "rev-parse", "--git-common-dir")
    if rc == 0 and o.strip():
        p = Path(o.strip())
        if not p.is_absolute():
            p = Path(src) / p
        p = p.resolve()
        if p.name == ".git":
            return p.parent
    return Path(src).resolve()


class Ctx:
    """src: where to read docs and tools from; main: the main checkout (build-runs, worktrees, the main branch);
    here: the checkout that holds this file (where generated files are written)."""

    def __init__(self, repo):
        self.here = SCRIPT_ROOT
        self.src = Path(repo).resolve() if repo else SCRIPT_ROOT
        self.main = find_main_root(self.src)
        self.wt_dir = self.main / ".claude" / "worktrees"
        self.branch = "main"


def list_worktrees(ctx):
    """[{path, head, branch}] of the worktrees under .claude/worktrees (not the main checkout)."""
    rc, o = git(ctx.main, "worktree", "list", "--porcelain")
    res, cur = [], None
    for line in o.splitlines() + [""]:
        if not line.strip():
            if cur and cur.get("path"):
                res.append(cur)
            cur = None
            continue
        k, _, v = line.partition(" ")
        if k == "worktree":
            cur = {"path": Path(v), "head": "", "branch": None}
        elif cur is not None and k == "HEAD":
            cur["head"] = v
        elif cur is not None and k == "branch":
            cur["branch"] = v[len("refs/heads/"):] if v.startswith("refs/heads/") else v
    wt = str(ctx.wt_dir.resolve()).lower().replace("\\", "/")
    keep = []
    for w in res:
        p = str(w["path"].resolve()).lower().replace("\\", "/")
        if p.startswith(wt + "/"):
            keep.append(w)
    return keep


def branch_sets(ctx):
    """(all local branches, those merged into main)."""
    rc, a = git(ctx.main, "for-each-ref", "--format=%(refname:short)", "refs/heads")
    rc2, m = git(ctx.main, "for-each-ref", "--format=%(refname:short)", f"--merged={ctx.branch}", "refs/heads")
    return set(a.split()), set(m.split())


def _prune(name):
    return name in PRUNE or (name.startswith("build") and name != "build-runs")


def newest_mtime(path, since=None):
    """Newest modification time (seconds) of any file or folder under path, skipping build caches. With `since`,
    return as soon as one entry is at least that new (so a busy tree costs almost nothing)."""
    best = 0.0
    stack = [str(path)]
    while stack:
        d = stack.pop()
        try:
            it = os.scandir(d)
        except OSError:
            continue
        with it:
            for e in it:
                try:
                    m = e.stat(follow_symlinks=False).st_mtime
                    isdir = e.is_dir(follow_symlinks=False) and not getattr(e, "is_junction", lambda: False)()
                except OSError:
                    continue
                if m > best:
                    best = m
                    if since is not None and best >= since:
                        return best
                if isdir and not _prune(e.name):
                    stack.append(e.path)
    return best


def gitdir_mtime(wt_path):
    """Newest mtime of the worktree's private git files (HEAD, index, reflog): commits and checkouts touch them."""
    best = 0.0
    try:
        txt = (Path(wt_path) / ".git").read_text(encoding="utf-8", errors="replace")
        m = re.match(r"gitdir:\s*(.+)", txt.strip())
        if m:
            g = Path(m.group(1).strip())
            for rel in ("HEAD", "index", "logs/HEAD"):
                try:
                    best = max(best, (g / rel).stat().st_mtime)
                except OSError:
                    pass
    except OSError:
        pass
    return best


def worktree_activity(path, since=None):
    return max(gitdir_mtime(path), newest_mtime(path, since))


def parallel(fn, items, workers=8):
    with concurrent.futures.ThreadPoolExecutor(max_workers=workers) as ex:
        return list(ex.map(fn, items))


def is_dirty(path):
    rc, o = git(path, "-c", "submodule.recurse=false", "status", "--porcelain", "--ignore-submodules=all", timeout=40)
    return rc == 0 and bool(o.strip())


def read_json(path):
    t = read_text(path)
    if t is None:
        return None
    try:
        return json.loads(t.lstrip("﻿"))
    except ValueError:
        return None


def fmt_age(sec):
    sec = int(sec)
    if sec < 3600:
        return f"{sec // 60}m"
    if sec < 86400:
        return f"{sec // 3600}h{(sec % 3600) // 60:02d}m"
    return f"{sec // 86400}d"


# ---------------------------------------------------------------------------------------------------------------
# slots
# ---------------------------------------------------------------------------------------------------------------

def powershell(script, timeout=25):
    try:
        p = subprocess.run(["powershell", "-NoProfile", "-NonInteractive", "-Command", script], capture_output=True,
                           text=True, errors="replace", timeout=timeout, creationflags=NOWIN)
        return p.stdout
    except (OSError, subprocess.SubprocessError):
        return ""


def cpu_load():
    """Current CPU load in percent, or None (best effort, Windows)."""
    if os.name != "nt":
        try:
            return min(100.0, os.getloadavg()[0] / (os.cpu_count() or 1) * 100)
        except (OSError, AttributeError):
            return None
    try:
        p = subprocess.run(["wmic", "cpu", "get", "loadpercentage"], capture_output=True, text=True, timeout=15,
                           creationflags=NOWIN)
        v = [int(x) for x in re.findall(r"^\s*(\d+)\s*$", p.stdout, re.M)]
        if v:
            return sum(v) / len(v)
    except (OSError, subprocess.SubprocessError):
        pass
    o = powershell("(Get-CimInstance Win32_Processor | Measure-Object -Property LoadPercentage -Average).Average")
    try:
        return float(o.strip().replace(",", "."))
    except ValueError:
        return None


def running_queue_slots():
    """Slots of the game queues that are running now (from their command lines), or None if none is running."""
    if os.name != "nt":
        return None
    o = powershell("Get-CimInstance Win32_Process -Filter \"Name='powershell.exe' OR Name='pwsh.exe'\" | "
                   "ForEach-Object { $_.CommandLine }")
    total, found = 0, False
    for line in o.splitlines():
        if "game_queue.ps1" not in line or "-Status" in line or "-Check" in line:
            continue
        m = re.search(r"-SlotList\s+['\"]?([\d,\s]+)", line)
        if m:
            total += len([x for x in re.split(r"[,\s]+", m.group(1)) if x])
            found = True
            continue
        m = re.search(r"-Slots\s+(\d+)", line)
        total += int(m.group(1)) if m else 3
        found = True
    return total if found else None


def game_slots(ctx, arg):
    if arg:
        return arg, "--game-slots"
    q = running_queue_slots()
    if q:
        return q, "running queue"
    t = read_text(ctx.src / "tools" / "game_queue.ps1")
    if t:
        m = re.search(r"\[int\]\$Slots\s*=\s*(\d+)", t)
        if m:
            return int(m.group(1)), "game_queue.ps1 default"
    return 6, "default"


def worker_pid_live(pid):
    """Unknown process status retains the conservative activity proxy."""
    if not isinstance(pid, int) or isinstance(pid, bool) or pid <= 0:
        return None
    try:
        if sys.platform != "win32":
            os.kill(pid, 0)
            return True
        import ctypes
        k = ctypes.WinDLL("kernel32", use_last_error=True)
        k.OpenProcess.restype = ctypes.c_void_p
        k.GetExitCodeProcess.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_ulong)]
        k.CloseHandle.argtypes = [ctypes.c_void_p]
        h = k.OpenProcess(0x1000, False, pid)
        if not h:
            return False if ctypes.get_last_error() == 87 else None
        code = ctypes.c_ulong()
        try:
            return code.value == 259 if k.GetExitCodeProcess(h, ctypes.byref(code)) else None
        finally:
            k.CloseHandle(h)
    except ProcessLookupError:
        return False
    except (OSError, ValueError, OverflowError):
        return None


def dispatcher_workers(ctx):
    """Prefer recorded workers; unknown/malformed records keep the old proxy."""
    root = (ctx.main / "build-runs").resolve()
    workers = {}
    for path in sorted(root.glob("*-coordinator-state.json"), key=lambda p: p.stat().st_mtime):
        data = read_json(path)
        if not isinstance(data, dict) or not isinstance(data.get("lanes"), dict):
            continue
        for name, entry in data["lanes"].items():
            if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]{0,127}", name) or not isinstance(entry, dict):
                continue
            guard = root / "lane-guard" / (name + ".json")
            gm = guard.stat().st_mtime if guard.is_file() else 0
            complete = False
            raw = entry.get("result")
            if isinstance(raw, str) and raw:
                result = Path(raw)
                if not result.is_absolute():
                    result = ctx.main / result
                result = result.resolve()
                if result.is_relative_to(root) and result.is_file():
                    d = read_json(result)
                    complete = (isinstance(d, dict) and ("result" in d or "subtype" in d)
                                and gm <= result.stat().st_mtime)
            # Complete output retires a worker even if Windows reused its PID.
            live = False if complete else worker_pid_live(entry.get("pid"))
            workers[name] = (live if live is True or complete else None, gm)
    return workers


def active_lanes(ctx):
    now = time.time()
    since = now - ACTIVE_MIN * 60
    wts = [w for w in list_worktrees(ctx)]
    workers = dispatcher_workers(ctx)

    def probe(w):
        try:
            m = worktree_activity(w["path"], since)
        except OSError:
            m = 0.0
        return w, m

    res = []
    for w, m in parallel(probe, wts):
        worker, gm = workers.get(w["branch"], (None, 0))
        if worker is False:
            continue
        m = max(m, gm)
        if worker is not True and m < since:
            continue
        merged = False if worker is True else is_ancestor(ctx.main, w["head"], ctx.branch) if w["head"] else True
        if merged:
            # a lane that has not committed yet sits on an old main commit: count it while it is new or dirty
            try:
                fresh = now - w["path"].stat().st_ctime < ACTIVE_MIN * 60
            except OSError:
                fresh = False
            if not (fresh or is_dirty(w["path"])):
                continue
        res.append({"name": w["path"].name, "branch": w["branch"], "path": w["path"], "age": now - m})
    res.sort(key=lambda r: r["age"])
    return res


def cmd_slots(ctx, a):
    lg = laneguard()
    stop = lg.read_stop(str(ctx.main))
    cpus = os.cpu_count() or 4
    gs, gsrc = game_slots(ctx, a.game_slots)
    cap = min(16, cpus * 3 // 4, gs * 2 + 4)
    free_gb = shutil.disk_usage(str(ctx.main)).free / 2 ** 30
    load = cpu_load()
    lanes = active_lanes(ctx)
    n = len(lanes)
    if stop:
        word = f"STOP: {stop['reason']} (since {stop['since']}; lanes.py go clears it)"
    elif free_gb < a.min_free_gb:
        word = "low disk"
    elif n >= cap:
        word = "at cap"
    elif load is not None and load >= a.busy_pct:
        word = "cpu busy"
    else:
        word = "spawn ok"
    out(f"active {n} / cap {cap} - {word}")
    if a.verbose:
        out(f"  cpus {cpus}, game slots {gs} ({gsrc}), free disk {free_gb:.0f} GB (min {a.min_free_gb:.0f}), "
            f"cpu load {'?' if load is None else f'{load:.0f}%'} (busy at {a.busy_pct:.0f}%)")
        for r in lanes:
            out(f"  {r['name']}  branch {r['branch']}  last change {fmt_age(r['age'])} ago")
        for line in lg.status_lines(str(ctx.main))[1:]:
            out(f"  guard: {line}")
    return 0 if word == "spawn ok" else 1


def cmd_stop(ctx, a):
    lg = laneguard()
    lg.write_stop(str(ctx.main), a.reason)
    out(f"STOP set: {a.reason}")
    out("  the fc4-lane workflow refuses to start or restart a round; running lanes are told by the lane guard to "
        "commit their Handoff and return. The merge train still lands what is queued. Clear with: lanes.py go")
    return 0


def cmd_go(ctx, a):
    lg = laneguard()
    st = lg.read_stop(str(ctx.main))
    if lg.clear_stop(str(ctx.main)):
        out(f"STOP cleared (was: {st['reason']}, since {st['since']})")
    else:
        out("no STOP flag was set")
    return 0


def cmd_unpark(ctx, a):
    lg = laneguard()
    pr = lg.park_reason(str(ctx.main), a.lane)
    lg.unpark(str(ctx.main), a.lane, a.note or "")
    out(f"{a.lane}: released" + (f" (was parked: {pr})" if pr else " (it was not parked)"))
    return 0


def cmd_guard(ctx, a):
    for line in laneguard().status_lines(str(ctx.main), a.lane):
        out(line)
    return 0


# ---------------------------------------------------------------------------------------------------------------
# check
# ---------------------------------------------------------------------------------------------------------------

def queue_json_files(ctx, include_merged=False):
    files = []
    dirs = [ctx.main / "build-runs" / "merge-queue"] + [Path(p) for p in
                                                        glob.glob(str(ctx.wt_dir / "*" / "build-runs" / "merge-queue"))]
    for d in dirs:
        if d.is_dir():
            for f in sorted(d.glob("*.json")):
                if include_merged or not f.name.endswith(".merged.json"):
                    files.append(f)
    return files


def split_touch_keys(value):
    """Separate keys without splitting canonical names such as automata (managers, records)."""
    keys, start, depth = [], 0, 0
    value = value or ""
    for i, char in enumerate(value):
        if char == "(":
            depth += 1
        elif char == ")":
            depth -= 1
            if depth < 0:
                raise ValueError("unbalanced parentheses in --touches")
        elif char == "," and depth == 0:
            keys.append(value[start:i].strip().lower())
            start = i + 1
    if depth:
        raise ValueError("unbalanced parentheses in --touches")
    keys.append(value[start:].strip().lower())
    return [key for key in keys if key]


def declared_touch_keys(texts, lane, canonical):
    """Use validated claims from this lane's queue records; legacy/invalid claims fail closed.

    More than one record can exist (worktree and coordinator); union their claims so a stale
    copy cannot hide an overlap. Never borrow a different lane's declarations from its queue.
    """
    claims = set()
    found = False
    for path, _ in texts:
        if path.suffix != ".json" or path.stem != lane:
            continue
        rec = read_json(path)
        if not isinstance(rec, dict) or rec.get("lane") != lane or rec.get("branch", lane) != lane:
            return None
        keys = rec.get("touches")
        if not isinstance(keys, list) or not keys:
            return None
        if any(not isinstance(k, str) or not k.strip() or k.strip().lower() not in canonical for k in keys):
            return None
        found = True
        claims.update(k.strip().lower() for k in keys)
    return claims if found else None


def cmd_check(ctx, a):
    lane = a.lane
    reasons = []
    # 0. the lane guard's gates: the STOP flag, a parked lane, and for RE lanes a reachable REA server
    lg = laneguard()
    stop = lg.read_stop(str(ctx.main))
    if stop:
        reasons.append(f"STOP flag set: {stop['reason']} (since {stop['since']}; lanes.py go clears it)")
    pr = lg.park_reason(str(ctx.main), lane)
    if pr:
        reasons.append(f"lane parked: {pr} (lanes.py unpark {lane})")
    kind = a.kind or ("re" if lane.startswith("re-") else "engine")
    if kind == "re":
        ok, line = lg.rea_check(handshake=not a.no_rea_handshake)
        if not ok:
            reasons.append(line + " (an RE lane without REA explores without output: re-answers35, 2,789 calls)")
    allb, merged = branch_sets(ctx)
    # 1. dependencies must be on main
    for dep in [d.strip() for d in (a.deps or "").split(",") if d.strip()]:
        if dep not in allb:
            rc, _ = git(ctx.main, "rev-parse", "--verify", "--quiet", dep + "^{commit}")
            if rc != 0:
                reasons.append(f"dep {dep}: no such branch or commit")
                continue
            ok = is_ancestor(ctx.main, dep)
        else:
            ok = dep in merged or is_ancestor(ctx.main, dep)
        if not ok:
            reasons.append(f"dep {dep}: not on {ctx.branch} yet (git merge-base --is-ancestor {dep} {ctx.branch} fails)")
    # 2. new lanes must not exist; resumes must retain their branch/worktree identity.
    # This is read-only: never unpark a lane or reset its watchdog/round ledger here.
    if a.resume:
        if lane not in allb:
            reasons.append(f"resume: branch {lane} does not exist")
        elif lane in merged:
            reasons.append(f"resume: branch {lane} is already merged")
        expected = (ctx.wt_dir / lane).resolve()
        matching = [w for w in list_worktrees(ctx)
                    if w["path"].resolve() == expected and w["branch"] == lane]
        if not matching:
            reasons.append(f"resume: expected worktree .claude/worktrees/{lane} on branch {lane}")
    else:
        if lane in allb:
            reasons.append(f"branch {lane} already exists" + (" (merged)" if lane in merged else " (unmerged)"))
        if (ctx.wt_dir / lane).exists():
            reasons.append(f"worktree .claude/worktrees/{lane} already exists")
        for base in {ctx.main, ctx.src}:
            if (base / "docs" / "lanes" / f"{lane}.md").exists():
                reasons.append(f"docs/lanes/{lane}.md already exists")
                break
    # 2b. work assigned to an outside contributor (docs/assignments.tsv) until its date
    res_file = ctx.main / "docs" / "assignments.tsv"
    if res_file.exists():
        today = datetime.date.today().isoformat()
        keys_l = {k.strip().lower() for k in (a.touches or "").split(",") if k.strip()}
        for row in res_file.read_text(encoding="utf-8").splitlines():
            if not row.strip() or row.startswith("#"):
                continue
            spec, who, until = (row.split("\t") + ["", "", ""])[:3]
            spec = spec.strip().lower()
            if until.strip() < today:
                continue
            if spec in keys_l or spec in lane.lower():
                reasons.append(f"{spec} is assigned to {who.strip()} until {until.strip()} "
                               f"(docs/assignments.tsv): pick other work")
    # 3. explicit subsystem claims; use prose only for legacy/invalid declarations.
    try:
        keys = split_touch_keys(a.touches)
    except ValueError as exc:
        reasons.append(str(exc))
        keys = []
    if keys:
        canonical = set()
        map_path = ctx.main / "tools" / "coverage-map.tsv"
        if map_path.is_file():
            for row in read_text(map_path).splitlines():
                columns = row.split("\t")
                if not row.startswith("#") and len(columns) >= 2:
                    canonical.add(columns[1].strip().lower())
        wts = {w["branch"]: w["path"] for w in list_worktrees(ctx) if w["branch"]}
        now = time.time()
        hits = {}
        for b in sorted(allb - merged):
            if b == lane or b == ctx.branch:
                continue
            wt = wts.get(b)
            sources = []
            for base in ([wt] if wt else []) + [ctx.main]:
                sources.append(base / "docs" / "lanes" / f"{b}.md")
                if base is wt:
                    sources += [f for f in (base / "build-runs" / "merge-queue").glob("*.json")
                                if not f.name.endswith(".merged.json")]
                else:
                    sources.append(base / "build-runs" / "merge-queue" / f"{b}.json")
            texts = [(s, read_text(s)) for s in sources if s.exists()]
            if not texts:
                continue
            pending = any((read_json(s) or {}).get("state") in ("ready", "wip") for s, _ in texts if s.suffix == ".json")
            if not pending and wt:
                if now - worktree_activity(wt) > a.stale_days * 86400:
                    continue    # abandoned: not "active"
            elif not pending and not wt:
                continue
            declared = declared_touch_keys(texts, b, canonical)
            for k in keys:
                if declared is not None:
                    if k in declared:
                        hits.setdefault(k, []).append(f"{b} (declared subsystem)")
                    continue
                for s, t in texts:
                    if t and k in t.lower():
                        hits.setdefault(k, []).append(f"{b} ({s.name})")
                        break
        for k, lst in hits.items():
            more = f" (+{len(lst) - 4} more)" if len(lst) > 4 else ""
            reasons.append(f"touches '{k}': possible duplicate, mentioned by unmerged lane(s) {', '.join(lst[:4])}{more}")
    if reasons:
        for r in reasons:
            out(r)
        return 1
    out("ok")
    return 0


# ---------------------------------------------------------------------------------------------------------------
# wait
# ---------------------------------------------------------------------------------------------------------------

def verdict_key_lines(text, limit=10):
    lines = [l.rstrip() for l in text.splitlines()]
    picked = []

    def add(l):
        l = l.strip()
        if l and l not in picked:
            picked.append(l[:220])

    for pre in ("verdict:", "line:", "request:", "city:", "dll:"):
        for l in lines:
            if l.startswith(pre):
                add(l)
                break
    # problem counters that are not zero
    for l in lines:
        m = re.match(r"^\s+(.+?)\s{2,}(\d+)\s*$", l)
        if m and int(m.group(2)) != 0:
            add(f"problem: {m.group(1)} {m.group(2)}")
    for l in lines:
        s = l.strip()
        if s.startswith("simulator:") or s.startswith("game:"):
            add(s)
    return picked[:limit]


def cmd_wait(ctx, a):
    if not a.verdict and not a.file:
        out("wait: give --verdict <label> or --file <path>")
        return 3
    start = time.time()
    deadline = start + a.timeout
    roots = [ctx.src]
    if a.lane:
        roots.insert(0, ctx.wt_dir / a.lane)
    if a.verdict:
        label = a.verdict
        cands = [r / "build-runs" / "game-results" / f"{label}.verdict" for r in roots]
    else:
        p = Path(a.file)
        cands = [p if p.is_absolute() else r / p for r in roots]
    rx = re.compile(a.pattern) if a.pattern else None
    while True:
        for c in cands:
            try:
                st = c.stat()
            except OSError:
                continue
            if a.fresh and st.st_mtime < start - 2:
                continue
            if a.verdict:
                if time.time() - st.st_mtime < 3:
                    continue    # the writer may still be at it
                text = read_text(c) or ""
                if not text.startswith("verdict:"):
                    continue
                for l in verdict_key_lines(text):
                    out(l)
                return 0
            text = read_text(c)
            if text is None:
                continue
            if rx:
                hits = [l.rstrip()[:220] for l in text.splitlines() if rx.search(l)]
                if not hits:
                    continue
                if time.time() - st.st_mtime < 2:
                    continue    # matched, but the writer may still be at it
                for l in hits[-10:]:
                    out(l)
                return 0
            lines = [l.rstrip()[:220] for l in text.splitlines() if l.strip()]
            for l in lines[-10:]:
                out(l)
            return 0
        left = deadline - time.time()
        if left <= 0:
            out("timeout")
            return 2
        time.sleep(min(a.poll, left))


# ---------------------------------------------------------------------------------------------------------------
# monitor
# ---------------------------------------------------------------------------------------------------------------

TRAIN_RX = re.compile(r"^(=== gate verdict:.*|REGRESS \S+: (?:PASS|FAIL).*)$")


def train_verdict(path):
    try:
        if path.stat().st_size > 16 * 2 ** 20:
            return None
        t = read_text(path)
    except OSError:
        return None
    last = None
    for l in (t or "").splitlines():
        s = l.strip()
        if TRAIN_RX.match(s):
            last = s
    return last


def scan_queue(ctx):
    """{(lane, merged?): (sig, mtime, where)} over main and every worktree."""
    res = {}
    for f in queue_json_files(ctx, include_merged=True):
        merged = f.name.endswith(".merged.json")
        lane = f.name[:-len(".merged.json")] if merged else f.name[:-len(".json")]
        try:
            mt = f.stat().st_mtime
        except OSError:
            continue
        d = read_json(f) or {}
        sig = (d.get("state", "?"), str(d.get("tip", ""))[:8], d.get("kind", ""))
        key = (lane, merged)
        if key not in res or mt > res[key][1]:
            res[key] = (sig, mt, f)
    return res


def scan_verdicts(ctx):
    res = {}
    dirs = [ctx.main / "build-runs" / "game-results"] + [Path(p) for p in
                                                         glob.glob(str(ctx.wt_dir / "*" / "build-runs" / "game-results"))]
    for d in dirs:
        owner = "main" if d == dirs[0] else d.parent.parent.name
        if not d.is_dir():
            continue
        try:
            for e in os.scandir(d):
                if e.name.endswith(".verdict"):
                    res[(owner, e.name[:-len(".verdict")])] = e.stat().st_mtime
        except OSError:
            continue
    return res


def verdict_event(ctx, owner, label):
    d = ctx.main if owner == "main" else ctx.wt_dir / owner
    text = read_text(d / "build-runs" / "game-results" / f"{label}.verdict")
    if not text or not text.startswith("verdict:"):
        return None, False
    word = text.splitlines()[0].split(":", 1)[1].strip()
    bad = [l for l in verdict_key_lines(text) if l.startswith("problem:")]
    if word == "clean" and not bad:
        return None, True
    extra = (" " + "; ".join(bad)) if bad else ""
    return f"game: {label} verdict={word}{extra} [{owner}]", True


def stalled_scan(ctx, now, horizon_h):
    """Worktrees with an unmerged branch whose newest file change is 60 min .. horizon old: {name: (age_sec)}."""
    allb, merged = branch_sets(ctx)
    pending = {k[0] for k in scan_queue(ctx) if True}
    wts = [w for w in list_worktrees(ctx) if w["branch"] and w["branch"] not in merged and w["branch"] not in pending
           and w["path"].name not in pending]

    def probe(w):
        return w, worktree_activity(w["path"])

    res = {}
    for w, m in parallel(probe, wts):
        age = now - m
        if STALL_MIN * 60 <= age <= horizon_h * 3600 and not is_ancestor(ctx.main, w["head"]):
            res[w["path"].name] = (age, m)
    return res


def cmd_monitor(ctx, a):
    q = scan_queue(ctx)
    v = scan_verdicts(ctx)
    trains = {}
    for f in glob.glob(str(ctx.main / "build-runs" / "gate-train*.out")) + \
            glob.glob(str(ctx.main / "build-runs" / "regress-train*.out")):
        trains[Path(f).name] = train_verdict(Path(f))
    rc, head = git(ctx.main, "rev-parse", ctx.branch)
    head = head.strip()
    stalled_seen = set()
    next_stall = 0.0
    pending_verdicts = {}
    first = True
    while True:
        now = time.time()
        # merge queue
        nq = scan_queue(ctx)
        for key, (sig, mt, f) in sorted(nq.items()):
            lane, merged = key
            old = q.get(key)
            if old is not None and old[0] == sig:
                continue
            if merged:
                out(f"queue: {lane} merged")
            elif old is None:
                out(f"queue: {lane} new, state={sig[0]} kind={sig[2] or '?'} tip={sig[1] or '?'}")
            else:
                out(f"queue: {lane} state {old[0][0]} -> {sig[0]} (tip {sig[1] or '?'})")
        q = nq
        # game verdicts
        nv = scan_verdicts(ctx)
        for key, mt in sorted(nv.items()):
            if key in v and v[key] >= mt:
                continue
            if now - mt < 3:
                continue    # still being written
            line, ok = verdict_event(ctx, *key)
            v[key] = mt
            if line:
                out(line)
        # gate / regress outputs of a train
        for f in sorted(glob.glob(str(ctx.main / "build-runs" / "gate-train*.out")) +
                        glob.glob(str(ctx.main / "build-runs" / "regress-train*.out"))):
            name = Path(f).name
            tv = train_verdict(Path(f))
            if tv and tv != trains.get(name):
                out(f"train: {name[:-4]}: {tv[:200]}")
            if tv or name not in trains:
                trains[name] = tv
        # main moved
        rc, h2 = git(ctx.main, "rev-parse", ctx.branch)
        h2 = h2.strip()
        if h2 and head and h2 != head:
            rc, n = git(ctx.main, "rev-list", "--count", f"{head}..{h2}")
            rc, s = git(ctx.main, "log", "-1", "--format=%s", h2)
            out(f"main: {head[:8]} -> {h2[:8]} ({n.strip() or '?'} commits) {s.strip()[:120]}")
        if h2:
            head = h2
        # stalled lanes (a full scan of every worktree is not cheap: every few minutes only)
        if now >= next_stall:
            next_stall = now + max(a.interval, a.stall_every)
            for name, (age, m) in sorted(stalled_scan(ctx, now, a.stall_horizon).items()):
                if name not in stalled_seen:
                    stalled_seen.add(name)
                    out(f"stalled: {name} no file change for {fmt_age(age)} (branch unmerged, no queue file)")
        first = False
        time.sleep(a.interval)


# ---------------------------------------------------------------------------------------------------------------
# ledger
# ---------------------------------------------------------------------------------------------------------------

def journal_dirs(ctx):
    slug = re.sub(r"[:\\/]", "-", str(ctx.main))
    home = Path(os.path.expanduser("~")) / ".claude" / "projects"
    return sorted(p for p in home.glob(slug + "*") if p.is_dir())


def load_journals(ctx):
    runs = {}
    for d in journal_dirs(ctx):
        for f in glob.glob(str(d / "*" / "workflows" / "wf_*.json")):
            j = read_json(f)
            if isinstance(j, dict) and j.get("runId"):
                runs[j["runId"]] = j
    return sorted(runs.values(), key=lambda j: j.get("startTime") or 0)


def agent_lane(label):
    pre, _, rest = (label or "").partition(":")
    return pre, re.sub(r"#.*$", "", rest)


def merge_commit_of(agent):
    m = re.search(r'"merge_commit"\s*:\s*"([0-9a-f]{6,40})"', agent.get("resultPreview") or "")
    return m.group(1) if m else ""


def analyse_run(j):
    ag = [e for e in j.get("workflowProgress", []) if e.get("type") == "workflow_agent"]
    args = j.get("args") if isinstance(j.get("args"), dict) else {}
    r = {"id": j["runId"], "start": j.get("startTime") or 0, "workflow": j.get("workflowName") or "?",
         "name": args.get("name"), "kind": args.get("kind"), "status": j.get("status") or "?",
         "lane_tok": 0, "merge_tok": 0, "other_tok": 0, "agents": ag, "is_lane_run": False, "lanes": {}, "mc": []}
    for e in ag:
        pre, nm = agent_lane(e.get("label"))
        tok = e.get("tokens") or 0
        if pre == "merge":
            r["merge_tok"] += tok
            r["is_lane_run"] = True
        elif pre in ("lane", "re", "gate"):     # gate: the fc4-lane pre-flight agent, part of the lane's cost
            r["lane_tok"] += tok
            r["is_lane_run"] = True
        else:
            r["other_tok"] += tok
        if pre in ("lane", "re", "merge", "gate") and nm:
            L = r["lanes"].setdefault(nm, {"lane_tok": 0, "merge_tok": 0, "calls": 0, "ms": 0,
                                           "pre": "lane" if pre == "gate" else pre, "mc": ""})
            L["merge_tok" if pre == "merge" else "lane_tok"] += tok
            L["calls"] += e.get("toolCalls") or 0
            L["ms"] += e.get("durationMs") or 0
            if pre in ("lane", "re"):
                L["pre"] = pre
            if pre == "merge":
                L["mc"] = merge_commit_of(e) or L["mc"]
    s = sum(e.get("tokens") or 0 for e in ag)
    r["total"] = j.get("totalTokens") or s
    r["calls"] = j.get("totalToolCalls") or sum(e.get("toolCalls") or 0 for e in ag)
    r["ms"] = j.get("durationMs") or 0
    res = j.get("result")
    if isinstance(res, dict):
        mc = (res.get("merge") or {}).get("merge_commit") if isinstance(res.get("merge"), dict) else None
        if mc:
            r["mc"] = [mc]
    if not r["mc"]:
        r["mc"] = [L["mc"] for L in r["lanes"].values() if L["mc"]]
    return r


def queue_kinds(ctx):
    kinds = {}
    for f in queue_json_files(ctx, include_merged=True):
        d = read_json(f) or {}
        nm = f.name.split(".")[0]
        if d.get("kind"):
            kinds.setdefault(nm, d["kind"])
    return kinds


KIND_MAP = {"tool": "tools", "tools": "tools", "engine": "engine", "re": "re", "docs": "docs", "test": "test"}


def parse_headline(text):
    """{'specified': (pct_bytes, pct_funcs), ...} from the Headline table of docs/coverage-report.md."""
    res = {}
    for m in re.finditer(r"\|\s*\*\*(specified|implemented|verified|replaced)\*\*\s*\|[^|]*\|\s*([\d,]+)\s*\|\s*"
                         r"\*\*([\d.]+)%\*\*\s*\|\s*([\d,]+)\s*\|\s*([\d.]+)%", text):
        res[m.group(1)] = (float(m.group(3)), float(m.group(5)))
    return res if len(res) == 4 else None


def coverage_history(ctx):
    """[(position on main's first-parent line, commit, subject, headline)] oldest first."""
    rc, o = git(ctx.main, "log", "--format=%H", ctx.branch, "--", "docs/coverage-report.md")
    commits = o.split()
    rc, fp = git(ctx.main, "rev-list", "--first-parent", ctx.branch)
    fpl = list(reversed(fp.split()))
    index = {c: i for i, c in enumerate(fpl)}
    res = {}
    for c in commits:
        if c in index:
            pos = index[c]
        else:
            lo, hi = 0, len(fpl) - 1     # earliest first-parent commit that contains c
            if not fpl or not is_ancestor(ctx.main, c, fpl[hi]):
                continue
            while lo < hi:
                mid = (lo + hi) // 2
                if is_ancestor(ctx.main, c, fpl[mid]):
                    hi = mid
                else:
                    lo = mid + 1
            pos = lo
        rc, t = git(ctx.main, "show", f"{c}:docs/coverage-report.md")
        h = parse_headline(t) if rc == 0 else None
        rc, s = git(ctx.main, "log", "-1", "--format=%s", c)
        prev = res.get(pos)
        if prev is None or index.get(c, -1) >= index.get(prev[1], -1) and c != prev[1]:
            # several report commits map to one position: keep the newest one that parses
            if h or prev is None:
                res[pos] = (pos, c, s.strip(), h)
    return [res[p] for p in sorted(res)], fpl


def lanes_in_interval(ctx, fpl, p0, p1, known, journal_mc):
    """Lane names merged by the first-parent commits fpl[p0+1 .. p1]."""
    found = set()
    if p1 <= p0:
        return found
    rc, o = git(ctx.main, "log", "--first-parent", "--format=%H%x1f%P%x1f%s%x1f%b%x1e", f"{fpl[p0]}..{fpl[p1]}")
    commits = []
    for rec in o.split("\x1e"):
        rec = rec.strip("\n")
        if not rec.strip():
            continue
        h, parents, subj, body = (rec.split("\x1f") + ["", "", "", ""])[:4]
        commits.append(h.strip())
        merge = len(parents.split()) >= 2
        if merge or subj.startswith("Merge") or subj.startswith("merge-train:"):
            for tok in re.findall(r"[A-Za-z0-9][A-Za-z0-9_.\-]*", subj + " " + body):
                tok = tok.rstrip(".-")
                if tok in known:
                    found.add(tok)
    rc, allc = git(ctx.main, "rev-list", f"{fpl[p0]}..{fpl[p1]}")
    allset = allc.split()
    for lane, mcs in journal_mc.items():
        for mc in mcs:
            if mc and any(c.startswith(mc) for c in allset):
                found.add(lane)
    return found


def cmd_ledger(ctx, a):
    journals = load_journals(ctx)
    if not journals:
        out(f"ledger: no workflow journals under {journal_dirs(ctx) or '~/.claude/projects/<slug>'}")
        return 1
    runs = [analyse_run(j) for j in journals]
    allb, merged = branch_sets(ctx)
    qk = queue_kinds(ctx)

    # per run rows
    lane_kind = {}
    for r in runs:
        if r["is_lane_run"] and r["name"] and r["kind"]:
            lane_kind.setdefault(r["name"], KIND_MAP.get(r["kind"], r["kind"]))

    def doc_kind(name):
        for base in (ctx.main, ctx.wt_dir / name):
            t = read_text(base / "docs" / "lanes" / f"{name}.md") or ""
            m = re.search(r"\((engine|tooling|re|docs|test)\s+lane\)", t, re.I)
            if m:
                return {"tooling": "tools"}.get(m.group(1).lower(), m.group(1).lower())
        return None

    def kind_of(name, pre):
        return (lane_kind.get(name) or KIND_MAP.get(qk.get(name, ""), qk.get(name)) or doc_kind(name)
                or ("re" if pre == "re" or name.startswith("re-") else "engine?"))

    ancestor_cache = {}

    def on_main(c):
        if not c:
            return "-"
        if c not in ancestor_cache:
            rc, _ = git(ctx.main, "rev-parse", "--verify", "--quiet", c + "^{commit}")
            ancestor_cache[c] = "no-such-commit" if rc != 0 else ("yes" if is_ancestor(ctx.main, c) else "no")
        return ancestor_cache[c]

    def landed(name, mcs=()):
        """yes: the branch is an ancestor of main. yes-commit: the branch is gone but the journal's merge commit is
        on main. no: the branch exists and is not merged. no-branch: neither can be shown."""
        if name in allb:
            return "yes" if name in merged else "no"
        if any(on_main(c) == "yes" for c in mcs):
            return "yes-commit"
        return "no-branch"

    rows = []
    for r in runs:
        if r["is_lane_run"]:
            names = list(r["lanes"])
            if r["name"]:
                kind, lane = KIND_MAP.get(r["kind"], r["kind"]) or "?", r["name"]
                land = landed(lane, r["mc"])
            else:
                kind, lane = "wave", f"{r['workflow']} [{len(names)} lanes]"
                st = [landed(n, [r["lanes"][n]["mc"]]) for n in names]
                land = f"{sum(1 for s in st if s.startswith('yes'))}/{len(st)}"
        else:
            kind, lane, land = "non-lane", r["workflow"], "-"
        mcs = r["mc"]
        cm = ",".join(mcs)
        cmain = "-" if not mcs else ("yes" if all(on_main(c) == "yes" for c in mcs) else
                                      ("partial" if any(on_main(c) == "yes" for c in mcs) else "no"))
        rows.append({"run": r["id"], "date": datetime.datetime.fromtimestamp(r["start"] / 1000).strftime("%Y-%m-%d %H:%M")
                     if r["start"] else "", "lane": lane, "kind": kind, "status": r["status"], "lane_tok": r["lane_tok"],
                     "merge_tok": r["merge_tok"], "other_tok": r["other_tok"], "total": r["total"], "calls": r["calls"],
                     "hours": r["ms"] / 3.6e6, "landed": land, "commit": cm, "commit_on_main": cmain,
                     "is_lane": r["is_lane_run"]})

    out_path = Path(a.out) if os.path.isabs(a.out) else ctx.here / a.out
    out_path.parent.mkdir(parents=True, exist_ok=True)
    cols = ["run", "date", "lane", "kind", "status", "lane_tok", "merge_tok", "other_tok", "total", "calls", "hours",
            "landed", "commit", "commit_on_main"]
    with open(out_path, "w", encoding="utf-8", newline="\n") as fh:
        fh.write("\t".join(cols) + "\n")
        for x in rows:
            fh.write("\t".join(f"{x[c]:.2f}" if c == "hours" else str(x[c]) for c in cols) + "\n")

    # per lane (a lane re-run or resumed is one lane: its runs are summed)
    lanes = {}
    for r in runs:
        if not r["is_lane_run"]:
            continue
        for nm, L in r["lanes"].items():
            E = lanes.setdefault(nm, {"lane_tok": 0, "merge_tok": 0, "calls": 0, "ms": 0, "runs": 0, "pre": L["pre"],
                                      "mc": [], "statuses": set()})
            E["lane_tok"] += L["lane_tok"]
            E["merge_tok"] += L["merge_tok"]
            E["calls"] += L["calls"]
            E["ms"] += L["ms"]
            E["runs"] += 1
            E["pre"] = L["pre"] if L["lane_tok"] else E["pre"]
            E["statuses"].add(r["status"])
            if L["mc"]:
                E["mc"].append(L["mc"])
    for nm, E in lanes.items():
        E["kind"] = kind_of(nm, E["pre"])
        E["total"] = E["lane_tok"] + E["merge_tok"]
        E["commit_on_main"] = on_main(E["mc"][-1]) if E["mc"] else "-"
        E["landed"] = landed(nm, E["mc"])
    lane_path = out_path.with_name(out_path.stem + "-by-lane.tsv")
    with open(lane_path, "w", encoding="utf-8", newline="\n") as fh:
        fh.write("lane\tkind\truns\tlane_tok\tmerge_tok\ttotal\tcalls\thours\tlanded\tmerge_commit\tcommit_on_main\n")
        for nm, E in sorted(lanes.items(), key=lambda kv: -kv[1]["total"]):
            fh.write(f"{nm}\t{E['kind']}\t{E['runs']}\t{E['lane_tok']}\t{E['merge_tok']}\t{E['total']}\t{E['calls']}\t"
                     f"{E['ms'] / 3.6e6:.2f}\t{E['landed']}\t{(E['mc'] or [''])[-1]}\t{E['commit_on_main']}\n")

    # coverage movement
    hist, fpl = coverage_history(ctx)
    known = (set(lanes) | allb) - {"main"}
    journal_mc = {nm: E["mc"] for nm, E in lanes.items()}
    cov = []
    for (p0, c0, s0, h0), (p1, c1, s1, h1) in zip(hist, hist[1:]):
        found = lanes_in_interval(ctx, fpl, p0, p1, known, journal_mc)
        toks = sum(lanes[n]["total"] for n in found if n in lanes)
        eng = sum(lanes[n]["total"] for n in found if n in lanes and lanes[n]["kind"].startswith("engine"))
        d = {k: (h1[k][0] - h0[k][0]) for k in h1} if (h0 and h1) else None
        cov.append({"from": c0[:8], "to": c1[:8], "subject": s1, "lanes": sorted(found), "tokens": toks, "eng_tokens": eng,
                    "delta": d})

    summary = build_summary(ctx, runs, rows, lanes, hist, cov)
    if a.md:
        md_path = Path(a.md) if os.path.isabs(a.md) else ctx.here / a.md
        md_path.parent.mkdir(parents=True, exist_ok=True)
        md_path.write_text(summary["md"], encoding="utf-8", newline="\n")
    out(summary["short"])
    out(f"wrote {out_path} ({len(rows)} runs), {lane_path.name} ({len(lanes)} lanes)" +
        (f", {a.md}" if a.md else ""))
    return 0


def k(n):
    return f"{n / 1000:,.0f}k"


def build_summary(ctx, runs, rows, lanes, hist, cov):
    lane_rows = [x for x in rows if x["is_lane"]]
    nonlane = [x for x in rows if not x["is_lane"]]
    tot = sum(x["total"] for x in rows)
    lane_total = sum(x["lane_tok"] for x in rows)
    merge_total = sum(x["merge_tok"] for x in rows)
    other_total = sum(x["other_tok"] for x in rows)
    lane_sys_total = lane_total + merge_total + sum(x["other_tok"] for x in lane_rows)
    bykind = collections.defaultdict(list)
    for nm, E in lanes.items():
        bykind[E["kind"]].append(E)
    kinds = sorted(bykind, key=lambda x: -len(bykind[x]))
    date = datetime.date.today().isoformat()
    first = min((x["date"] for x in rows if x["date"]), default="?")
    last = max((x["date"] for x in rows if x["date"]), default="?")
    landed_n = sum(1 for E in lanes.values() if E["landed"].startswith("yes"))
    cm_yes = sum(1 for E in lanes.values() if E["commit_on_main"] == "yes")
    cm_no = sum(1 for E in lanes.values() if E["commit_on_main"] in ("no", "no-such-commit"))

    md = []
    md.append("# Token ledger")
    md.append("")
    md.append(f"Generated {date} by `python tools/lanes.py ledger` from {len(rows)} workflow run journals "
              f"({first[:10]} to {last[:10]}) in `%USERPROFILE%\\.claude\\projects\\<slug>\\*\\workflows\\`. "
              "Rerun it to refresh; the per-run and per-lane tables are `build-runs/lane-ledger.tsv` and "
              "`build-runs/lane-ledger-by-lane.tsv` (gitignored).")
    md.append("")
    md.append("Tokens are the journal's own `tokens` per agent (what the run reports; not split into input, output "
              "and cache). A lane is one lane name: runs that were re-run or resumed are summed. `merge` is the agent "
              "that merged the lane into main under the old merge-lock protocol (the merge train has no such agent).")
    md.append("")
    md.append("## Totals")
    md.append("")
    md.append(f"- {len(rows)} runs: {len(lane_rows)} lane runs, {len(nonlane)} other (audits, doc cleanups); "
              f"{len(lanes)} distinct lanes.")
    md.append(f"- {k(tot)} tokens in all; {sum(x['calls'] for x in rows):,} tool calls; "
              f"{sum(x['hours'] for x in rows):.0f} agent-run hours (wall time of each run, summed).")
    md.append(f"- Lane agents {k(lane_total)}, merge agents {k(merge_total)}, other agents {k(other_total)}.")
    md.append(f"- **Merge share: {100 * merge_total / max(1, lane_sys_total):.1f}% of the tokens of lane runs** "
              f"({k(merge_total)} of {k(lane_sys_total)}) went to merging, not to the lane's work.")
    md.append(f"- Landed: {landed_n} of {len(lanes)} lanes are on main (branch an ancestor of main, or "
              f"the branch deleted and the journal's merge commit an ancestor); {cm_yes} lanes have a journal merge "
              f"commit that is on main, {cm_no} have one that is not (reported merged, reverted or rewritten).")
    md.append(f"- Status of the runs: " + ", ".join(f"{n} {s}" for s, n in
                                                      collections.Counter(x['status'] for x in rows).most_common()) + ".")
    killed_tok = sum(x["total"] for x in rows if x["status"] == "killed")
    md.append(f"- {k(killed_tok)} tokens ({100 * killed_tok / max(1, tot):.0f}%) were spent in runs that were killed "
              "before they finished.")
    md.append("")
    md.append("## Per kind")
    md.append("")
    md.append("Per lane (all runs of the lane summed). `engine?` = a wave lane whose kind no journal or queue file names.")
    md.append("")
    md.append("| Kind | Lanes | Avg tokens | Median | Avg tool calls | Avg hours | Merge share | Landed |")
    md.append("|---|---:|---:|---:|---:|---:|---:|---:|")
    for kd in kinds:
        es = bykind[kd]
        t = [e["total"] for e in es]
        ms = sum(e["merge_tok"] for e in es)
        md.append(f"| {kd} | {len(es)} | {k(sum(t) / len(t))} | {k(statistics.median(t))} | "
                  f"{sum(e['calls'] for e in es) / len(es):.0f} | {sum(e['ms'] for e in es) / len(es) / 3.6e6:.1f} | "
                  f"{100 * ms / max(1, sum(t)):.0f}% | {sum(1 for e in es if e['landed'].startswith('yes'))}/{len(es)} |")
    allt = [e["total"] for e in lanes.values()]
    md.append(f"| **all** | {len(allt)} | {k(sum(allt) / len(allt))} | {k(statistics.median(allt))} | "
              f"{sum(e['calls'] for e in lanes.values()) / len(allt):.0f} | "
              f"{sum(e['ms'] for e in lanes.values()) / len(allt) / 3.6e6:.1f} | "
              f"{100 * sum(e['merge_tok'] for e in lanes.values()) / max(1, sum(allt)):.0f}% | {landed_n}/{len(allt)} |")
    md.append("")
    md.append("## Ten most expensive lanes")
    md.append("")
    md.append("| Lane | Kind | Runs | Tokens | of which merge | Tool calls | Hours | Landed |")
    md.append("|---|---|---:|---:|---:|---:|---:|---|")
    for nm, E in sorted(lanes.items(), key=lambda kv: -kv[1]["total"])[:10]:
        md.append(f"| {nm} | {E['kind']} | {E['runs']} | {k(E['total'])} | {k(E['merge_tok'])} | {E['calls']} | "
                  f"{E['ms'] / 3.6e6:.1f} | {E['landed']} |")
    md.append("")
    md.append("## Tokens per coverage point")
    md.append("")
    short_cov = "tokens per coverage point: not computable"
    if len(hist) < 2 or not all(h[3] for h in hist):
        md.append("Not computable: fewer than two parsable `docs/coverage-report.md` headlines in the history.")
    else:
        h0, h1 = hist[0][3], hist[-1][3]
        md.append("Headline of `docs/coverage-report.md` (% of game code bytes) at the oldest and newest report "
                  f"commit on main's first-parent line: " + ", ".join(
                      f"{kk} {h0[kk][0]:.1f}% -> {h1[kk][0]:.1f}%" for kk in ("specified", "implemented", "verified",
                                                                                  "replaced")) + ".")
        md.append("")
        md.append("| Report | Subject | Lanes merged since the previous report | Lane tokens | d specified | d implemented | "
                  "d verified | d replaced |")
        md.append("|---|---|---|---:|---:|---:|---:|---:|")
        for c in cov:
            d = c["delta"] or {}
            names = [n for n in c["lanes"]]
            ln = (", ".join(names[:7]) + (f" (+{len(names) - 7})" if len(names) > 7 else "")) if names else "none found"
            nojr = sum(1 for n in names if n not in lanes)
            if nojr:
                ln += f" [{nojr} without a journal]"
            md.append(f"| {c['to']} | {c['subject'][:60]} | {ln} | {k(c['tokens'])} | "
                      + " | ".join(f"{d.get(kk, 0):+.1f}" for kk in ("specified", "implemented", "verified", "replaced"))
                      + " |")
        md.append("")
        usable = [c for c in cov if c["lanes"] and c["delta"]]
        tok_all = sum(c["tokens"] for c in usable)
        tok_eng = sum(c["eng_tokens"] for c in usable)
        di = sum(c["delta"]["implemented"] for c in usable)
        dr = sum(c["delta"]["replaced"] for c in usable)
        dv = sum(c["delta"]["verified"] for c in usable)
        ds = sum(c["delta"]["specified"] for c in usable)
        md.append(f"Over the {len(usable)} report intervals in which a merged lane could be identified "
                  f"({len(usable) and sum(len(c['lanes']) for c in usable)} lanes, {k(tok_all)} lane tokens, "
                  f"{k(tok_eng)} of them in engine lanes):")
        md.append("")
        pts = []
        for nm, dd, tk in (("implemented", di, tok_eng), ("replaced", dr, tok_eng)):
            if dd > 0:
                pts.append(f"- **{nm}**: {dd:+.1f} points; engine-lane tokens per point {k(tk / dd)}; "
                           f"all-lane tokens per point {k(tok_all / dd)}.")
            else:
                pts.append(f"- **{nm}**: {dd:+.1f} points; not computable (no gain).")
        for nm, dd in (("specified", ds), ("verified", dv)):
            pts.append(f"- {nm}: {dd:+.1f} points; all-lane tokens per point "
                       f"{k(tok_all / dd) if dd > 0 else 'not computable (no gain)'}.")
        md.extend(pts)
        md.append("")
        md.append("How far to trust it: coverage points also move when the coverage tool or its map changes "
                  "(`coverage:` commits re-split or re-credit code without any lane), so some of the movement is not "
                  "lane work; lanes are attributed by name found in the merge commit messages and by the merge "
                  "commit in the journals, so a lane that landed some other way is missed; a lane's tokens count in "
                  "the interval in which it merged, however long it ran before; tools, docs and RE lanes cost tokens "
                  "but do not move implemented or replaced, hence the engine-lane column. Treat the numbers as an "
                  "order of magnitude, not a price list.")
        short_cov = ("tokens per point: implemented "
                     + (f"{k(tok_eng / di)} (engine lanes)" if di > 0 else "n/a") + ", replaced "
                     + (f"{k(tok_eng / dr)} (engine lanes)" if dr > 0 else "n/a")
                     + f" over {len(usable)} intervals")
    md.append("")
    per = " | ".join(f"{kd} {k(sum(e['total'] for e in bykind[kd]) / len(bykind[kd]))} ({len(bykind[kd])})"
                     for kd in kinds)
    short = (f"lanes {len(lanes)} in {len(rows)} runs, {k(tot)} tokens; avg per lane by kind: {per}; merge share "
             f"{100 * merge_total / max(1, lane_sys_total):.1f}%; {short_cov}")
    return {"md": "\n".join(md) + "\n", "short": short}


# ---------------------------------------------------------------------------------------------------------------
# brief
# ---------------------------------------------------------------------------------------------------------------

# clean-room rules (CONTRIBUTING.md): an engine implementer must not see these
ENGINE_FORBIDDEN = ("re/", "tools/", "decomp", "ghidra/", "external/", "docs/subsystems/", "docs/sim-engine.md")


def read_map(ctx):
    rows, header = [], None
    t = read_text(ctx.src / "tools" / "coverage-map.tsv") or ""
    for line in t.splitlines():
        if not line.strip() or line.startswith("#"):
            continue
        cells = line.split("\t")
        if header is None:
            header = [c.strip() for c in cells]
            continue
        cells += [""] * (len(header) - len(cells))
        rows.append({h: (c.strip() or "-") for h, c in zip(header, cells)})
    return rows


def split_list(v):
    return [] if v in ("", "-") else [x.strip() for x in v.split(",") if x.strip()]


def report_rows(ctx):
    """{subsystem name: {Specified, Implemented, Shadow, Verified, Replaced}} from the By subsystem table."""
    t = read_text(ctx.src / "docs" / "coverage-report.md") or ""
    res, cols = {}, None
    for line in t.splitlines():
        if not line.startswith("|"):
            continue
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if "Subsystem" in cells and "Shadow" in cells:
            cols = cells
            continue
        if cols and len(cells) == len(cols) and not set(cells[0]) <= set("-: "):
            res[cells[cols.index("Subsystem")].lower()] = dict(zip(cols, cells))
        elif cols and not line.startswith("|"):
            cols = None
    return res


def owner_names(patterns):
    names = []
    for p in patterns:
        if p.startswith("fn:"):
            continue
        n = re.sub(r"(::)?\*+$", "", p)
        n = n.split("::")[0] if n.startswith("n") and "::" in n and n[1:2].isupper() else n
        if n and n not in names:
            names.append(n)
    return names


def human(n):
    return f"{n / 1024:.0f} KB" if n >= 1024 else f"{n} B"


def spec_outline(path, focus=None, limit=24):
    """The spec's '## ' sections with line ranges and sizes, so a lane reads only the sections its task needs
    (Read with offset/limit) instead of the whole spec. With focus (a regex), sections whose heading or body
    match are marked '>>' and their '### ' subsections are listed too."""
    lines = (read_text(path) or "").splitlines()
    heads = [(i, l) for i, l in enumerate(lines) if re.match(r"#{2,4} ", l)]
    tops = [(i, l) for i, l in heads if l.startswith("## ")]
    rx = re.compile(focus, re.I) if focus else None
    out_lines = [f"  - outline (read sections with offset/limit, not the whole file; line numbers are 1-based):"]
    shown = 0
    for n, (i, l) in enumerate(tops):
        end = tops[n + 1][0] if n + 1 < len(tops) else len(lines)
        size = sum(len(x) + 1 for x in lines[i:end])
        hit = bool(rx and rx.search("\n".join(lines[i:end])))
        if shown < limit or hit:
            out_lines.append(f"    {'>>' if hit else '  '} {i + 1}-{end}  {human(size):>7}  {l[3:][:70]}")
            shown += 1
        if hit:  # the subsections (### / ####) whose own text matches, with their line ranges
            subs = [(j, sl) for j, sl in heads if i < j < end]
            for k, (j, sl) in enumerate(subs):
                send = subs[k + 1][0] if k + 1 < len(subs) else end
                if rx.search("\n".join(lines[j:send])):
                    out_lines.append(f"         {j + 1}-{send}  {sl.lstrip('#').strip()[:70]}")
    if len(tops) > shown:
        out_lines.append(f"       (+{len(tops) - shown} more sections; pass --focus <regex> to pick)")
    return out_lines


def cmd_brief(ctx, a):
    key = a.key.strip()
    kl = key.lower()
    rows = read_map(ctx)
    if not rows:
        out("brief: tools/coverage-map.tsv not found or empty")
        return 1

    def fields(r):
        return {"subsystem": r["subsystem"].lower(), "pattern": r["pattern"].lower().rstrip("*"),
                "shadow": r["shadow"].lower(), "replace": r["replace"].lower(), "area": r["area"].lower(),
                "specs": [Path(s).stem.lower() for s in split_list(r["spec"])]}

    exact = [r for r in rows if kl in (fields(r)["subsystem"], fields(r)["pattern"], fields(r)["shadow"],
                                       fields(r)["replace"]) or kl in fields(r)["specs"]]
    if not exact:
        exact = [r for r in rows if any(kl in str(v) for v in (fields(r)["subsystem"], fields(r)["pattern"],
                                                                fields(r)["shadow"], fields(r)["replace"],
                                                                " ".join(fields(r)["specs"]), r["core"].lower()))]
    if not exact:
        exact = [r for r in rows if kl == fields(r)["area"]]
    if not exact:
        subs = sorted({r["subsystem"] for r in rows})
        out(f"brief: no coverage-map row matches '{key}'. Subsystems: " + ", ".join(subs)[:600])
        return 1
    groups = collections.OrderedDict()
    for r in exact:
        g = groups.setdefault(r["subsystem"], {"area": r["area"], "patterns": [], "specs": [], "cores": [],
                                               "shadows": [], "replaces": []})
        g["patterns"].append(r["pattern"])
        for fld, lst in (("spec", "specs"), ("core", "cores"), ("shadow", "shadows"), ("replace", "replaces")):
            for v in split_list(r[fld]):
                if v not in g[lst]:
                    g[lst].append(v)

    rep = report_rows(ctx)
    eng = a.kind == "engine"
    abi_files = sorted(glob.glob(str(ctx.src / "docs" / "abi" / "*")))
    abi_text = {Path(f).name: (read_text(f) or "").splitlines() for f in abi_files if Path(f).suffix in (".tsv", ".md")}
    L = [f"# Lane brief: {key} ({a.kind})", ""]
    if eng:
        L.append("Engine lane, clean-room rules (CONTRIBUTING.md): read only docs/specs, docs/abi, the public SDK "
                 "headers and the game's data files; nothing from the analysis side. Questions for the spec writers "
                 "go at the end of the spec's docs/specs/questions/<spec>.md (listed below).")
        L.append("Checks: while you work, `python <MAIN>/.claude/lane-bin/fc4verify.py --wt <WT> quick` (cargo test of "
                 "the crates you touched + the CMake build, sccache-backed); before you enqueue, `... --wt <WT> build "
                 "ctest` (the full ctest). Read its PASS/FAIL lines and the logs it names; exit codes are not verdicts.")
    else:
        L.append("RE lane: write behaviour specs in docs/specs/ (no engine/ edits); answer open questions per spec.")
    L.append("")
    shown = list(groups.items())
    detail = shown[:4]
    outlined = set()
    for name, g in detail:
        L.append(f"## {name} ({g['area']})")
        for s in g["specs"]:
            sp = ctx.src / "docs" / "specs" / s
            stem = Path(s).stem
            size = human(sp.stat().st_size) if sp.exists() else "MISSING"
            qp = ctx.src / "docs" / "specs" / "questions" / f"{stem}.md"
            L.append(f"- spec: docs/specs/{s} ({size}); questions: docs/specs/questions/{stem}.md "
                     f"({'exists' if qp.exists() else 'not there yet'})")
            if sp.exists() and s not in outlined:
                outlined.add(s)
                L.extend(spec_outline(sp, a.focus))
        if not g["specs"]:
            L.append("- spec: none yet" + ("" if eng else " (an RE lane writes it)"))
        for c in g["cores"]:
            base = ctx.src / "engine" / c
            if base.is_dir():
                n = sum(1 for _ in base.rglob("*") if _.is_file())
                L.append(f"- core: engine/{c}/ ({n} files)")
            elif base.with_suffix(".rs").exists():
                L.append(f"- core: engine/{c}.rs ({human(base.with_suffix('.rs').stat().st_size)})")
            else:
                L.append(f"- core: engine/{c} (not written yet)")
        if not g["cores"]:
            L.append("- core: none yet")
        L.append(f"- shadow key: {', '.join(g['shadows']) or '-'}; replace key: {', '.join(g['replaces']) or '-'}")
        names = owner_names(g["patterns"])
        if names:
            L.append(f"- owner classes: {', '.join(names[:8])}" + (f" (+{len(names) - 8})" if len(names) > 8 else ""))
            rx = re.compile("|".join(re.escape(n) for n in names[:12]))
            per_file = {fn: sum(1 for l in lines if rx.search(l)) for fn, lines in abi_text.items()}
            per_file = {f: n for f, n in per_file.items() if n}
            if per_file:
                top = ", ".join(f"{f} {n}" for f, n in sorted(per_file.items(), key=lambda kv: -kv[1])[:4])
                L.append(f"- ABI: {sum(per_file.values())} rows in docs/abi ({top}); "
                         f"`grep -nE \"{'|'.join(names[:6])}\" docs/abi/*`")
            else:
                L.append("- ABI: no rows in docs/abi mention these classes")
        # conformance vectors: those whose "spec" field starts with one of this subsystem's spec stems
        stems = {Path(s).stem for s in g["specs"]}
        if stems:
            vec_dir = ctx.src / "engine" / "conformance" / "vectors"
            hits = []
            for vf in sorted(vec_dir.glob("*.json")) if vec_dir.is_dir() else []:
                try:
                    head = vf.read_text(encoding="utf-8", errors="replace")[:1500]
                except OSError:
                    continue
                m = re.search(r'"spec"\s*:\s*\[([^\]]*)\]', head)
                if m and any(re.match(rf'\s*"{re.escape(st)}[ "]', e) for st in stems for e in m.group(1).split(",")):
                    hits.append(vf.name)
            runner = [f"engine/conformance/src/{st}.rs" for st in stems if (ctx.src / "engine" / "conformance" / "src" / f"{st}.rs").exists()]
            L.append(f"- conformance: {len(hits)} vectors in engine/conformance/vectors/"
                     + (f" (e.g. {', '.join(hits[:3])})" if hits else "")
                     + (f"; runner {', '.join(runner)}" if runner else "") + "; format: engine/conformance/README.md")
        r = rep.get(name.lower())
        if r:
            L.append("- coverage now: " + ", ".join(f"{c.lower()} {r.get(c, '?')}" for c in
                                                       ("Specified", "Implemented", "Shadow", "Verified", "Replaced")))
        if not eng:
            subs = [Path(f).name for f in glob.glob(str(ctx.src / "docs" / "subsystems" / "*"))
                    if any(n.lower() in Path(f).name.lower() for n in [name.split()[0]] + sorted(stems))]
            if subs:
                L.append("- RE notes: " + ", ".join(f"docs/subsystems/{s}" for s in subs[:4]))
        L.append("")
    if len(shown) > len(detail):
        L.append("## Other matching subsystems")
        for name, g in shown[len(detail):]:
            L.append(f"- {name} ({g['area']}): spec {', '.join(g['specs']) or '-'}; core "
                     f"{', '.join('engine/' + c for c in g['cores']) or '-'}; shadow {', '.join(g['shadows']) or '-'}; "
                     f"replace {', '.join(g['replaces']) or '-'}")
        L.append("")
    while L and L[-1] == "":
        L.pop()
    if eng:
        # last line of defence for the clean-room wall: nothing from the analysis side, no executable addresses
        bad = re.compile(r"(^|[\s`(])(re|tools|decomp[\w-]*|ghidra|external)/|docs/subsystems|docs/sim-engine\.md|"
                         r"\b0x[0-9a-fA-F]{6,8}\b|\b00[0-9a-f]{6}\b")
        L = [l for l in L if not bad.search(l)]
    if len(L) > 60:
        L = L[:59] + [f"... ({len(L) - 59} more lines cut; narrow the key)"]
    out("\n".join(L))
    return 0


# ---------------------------------------------------------------------------------------------------------------

def main(argv=None):
    ap = argparse.ArgumentParser(prog="lanes.py", description=__doc__.split("\n")[0])
    ap.add_argument("--repo", help="main checkout to read build-runs, worktrees and docs from (default: this checkout)")
    sub = ap.add_subparsers(dest="cmd", required=True)

    p = sub.add_parser("slots", help="recommended max concurrent lanes, and whether a spawn is ok now")
    p.add_argument("--game-slots", type=int, help="game-queue slots (default: running queue, game_queue.ps1, else 6)")
    p.add_argument("--min-free-gb", type=float, default=40)
    p.add_argument("--busy-pct", type=float, default=85)
    p.add_argument("-v", "--verbose", action="store_true")

    p = sub.add_parser("check", help="refuse a spawn whose dependencies have not landed or that duplicates a lane")
    p.add_argument("lane")
    p.add_argument("--resume", action="store_true",
                   help="check an existing unmerged lane in its original worktree; retain all guard state")
    p.add_argument("--deps", help="comma-separated branches that must already be on main")
    p.add_argument("--touches", help="comma-separated keys; commas inside parentheses stay in the key")
    p.add_argument("--stale-days", type=float, default=3, help="an unmerged lane idle longer than this is ignored")
    p.add_argument("--kind", choices=("engine", "re", "tool", "mech"),
                   help="lane kind (default: re for lanes named re-*, else engine); re adds the REA pre-flight")
    p.add_argument("--no-rea-handshake", action="store_true", help="re: skip starting a probe REA server")

    p = sub.add_parser("stop", help="set the STOP flag: no lane starts or restarts until 'go'")
    p.add_argument("reason")
    sub.add_parser("go", help="clear the STOP flag")
    p = sub.add_parser("unpark", help="release a lane the guard parked (two rounds blocked on the same thing)")
    p.add_argument("lane")
    p.add_argument("--note", default="")
    p = sub.add_parser("guard", help="lane-guard state: STOP, calls since commit, trips, parked lanes, guard errors")
    p.add_argument("lane", nargs="?")

    p = sub.add_parser("wait", help="block until a game verdict or a file appears (one call instead of polling)")
    p.add_argument("--verdict", help="game label: waits for build-runs/game-results/<label>.verdict")
    p.add_argument("--file", help="file to wait for")
    p.add_argument("--pattern", help="with --file: regex a line of the file must match")
    p.add_argument("--timeout", type=float, default=540, help="seconds (default 540; exit 2 on timeout, then re-invoke)")
    p.add_argument("--poll", type=float, default=5)
    p.add_argument("--lane", help="also look in .claude/worktrees/<lane> (for the coordinator)")
    p.add_argument("--fresh", action="store_true", help="ignore a verdict/file older than this call")

    p = sub.add_parser("monitor", help="one line per event: queue changes, bad verdicts, trains, main moves, stalls")
    p.add_argument("--interval", type=float, default=30)
    p.add_argument("--stall-every", type=float, default=600, help="seconds between stalled-lane scans")
    p.add_argument("--stall-horizon", type=float, default=8, help="hours: older idle lanes are not reported as stalled")

    p = sub.add_parser("ledger", help="token accounting from the workflow journals, and tokens per coverage point")
    p.add_argument("--out", default="build-runs/lane-ledger.tsv")
    p.add_argument("--md", default=None)

    p = sub.add_parser("brief", help="a <= 60-line context pack for a lane")
    p.add_argument("key")
    p.add_argument("--kind", choices=("engine", "re"), default="engine")
    p.add_argument("--focus", default=None, help="regex: mark the spec sections the task is about")

    a = ap.parse_args(argv)
    try:
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    except (AttributeError, ValueError):
        pass
    ctx = Ctx(a.repo)
    fn = {"slots": cmd_slots, "check": cmd_check, "wait": cmd_wait, "monitor": cmd_monitor, "ledger": cmd_ledger,
          "brief": cmd_brief, "stop": cmd_stop, "go": cmd_go, "unpark": cmd_unpark, "guard": cmd_guard}[a.cmd]
    try:
        return fn(ctx, a)
    except KeyboardInterrupt:
        return 130


if __name__ == "__main__":
    sys.exit(main())
