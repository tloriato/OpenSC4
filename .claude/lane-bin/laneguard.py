#!/usr/bin/env python3
"""Lane guard: the harness-side enforcement of the lane rules that used to be prompt text only.

Usage:
  python laneguard.py hook                 PreToolUse hook (stdin: the hook JSON). Wired in .claude/settings.json.
  python laneguard.py preflight <lane> [--kind engine|re|tool|mech]
                                           one line for the fc4-lane workflow's gate agent: GO, STOP ..., PARK ...
                                           or REA-DOWN ...; exit 0 on GO, 1 otherwise
  python laneguard.py status [<lane>]      guard state of every lane (or one): calls since commit, trips, rounds

What the hook enforces, for agents of type fc4-lane / fc4-lane-re whose workflow label is "lane:<lane>[#n]" (every
other session, subagent and agent type passes straight through):
  1. Context budget: the input of the agent's latest model call (read from its own transcript, as ctx.py does) past
     the budget denies every tool call except committing the lane file; the agent commits its Handoff and returns
     state "continue".
  2. Progress watchdog: more than <calls> tool calls since the worktree's last commit denies the call; the agent
     commits its findings so far and returns state "blocked" with the reason. The trip lasts for the agent's life.
  3. Stop flag: while <main>/build-runs/STOP exists (python tools/lanes.py stop "<reason>"), every lane is told to
     commit its Handoff and return "blocked".
  4. Parked lane: when the lane's last two results were "blocked" on the same question (or both tripped the
     watchdog), a new agent of that lane is told to return at once (python tools/lanes.py unpark <lane> releases it).
The budget and the call limit come from the lane prompt's "lane guard: budget=N calls=M" line, which the workflow
writes from args.ctxBudget / args.maxCalls. While tripped, the only calls allowed are Read/Write/Edit of
<WT>/docs/lanes/<lane>.md and <WT>/build-runs/msg-*.txt, Bash made only of `git -C <WT> add|commit|status|log|
rev-parse|diff` segments (optionally piped to head/tail), and StructuredOutput (the return). Every StructuredOutput
is also recorded in build-runs/lane-guard/<lane>.rounds.jsonl: that ledger is what parking reads.

State lives in <main>/build-runs/lane-guard/ (gitignored): <lane>.json (head, calls since commit), <lane>.log (one
line per deny), agents/<agent_id>.json (lane, transcript, limits, trip), errors.log. A guard error never blocks a
call (it is logged, and `lanes.py slots -v` shows it): a broken guard must not kill every lane. Standard library
only; environment overrides for tests: FC4_LANEGUARD_MAIN (the main checkout).
"""
import json
import os
import re
import subprocess
import sys
import time

LANE_TYPES = ("fc4-lane", "fc4-lane-re")
DEFAULT_BUDGET = 150000
DEFAULT_CALLS = 150
HERE = os.path.dirname(os.path.abspath(__file__))
NOWIN = getattr(subprocess, "CREATE_NO_WINDOW", 0)


# ----------------------------------------------------------------------------------------------- paths and files

def main_root():
    """The main checkout: FC4_LANEGUARD_MAIN, else the repo that holds this file (its main checkout when that repo
    is a worktree, read from the .git file without running git)."""
    env = os.environ.get("FC4_LANEGUARD_MAIN")
    if env:
        return os.path.abspath(env)
    root = os.path.dirname(os.path.dirname(HERE))
    dotgit = os.path.join(root, ".git")
    if os.path.isfile(dotgit):
        try:
            with open(dotgit, encoding="utf-8") as f:
                line = f.read().strip()
            if line.startswith("gitdir:"):
                gd = os.path.normpath(os.path.join(root, line[7:].strip()))
                # <main>/.git/worktrees/<name>
                if os.path.basename(os.path.dirname(gd)) == "worktrees":
                    return os.path.dirname(os.path.dirname(os.path.dirname(gd)))
        except OSError:
            pass
    return root


def guard_dir(main):
    return os.path.join(main, "build-runs", "lane-guard")


def stop_path(main):
    return os.path.join(main, "build-runs", "STOP")


def worktree(main, lane):
    return os.path.join(main, ".claude", "worktrees", lane)


def read_json(path, default=None):
    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except (OSError, ValueError):
        return default


def write_json(path, obj):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    tmp = f"{path}.{os.getpid()}.tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(obj, f)
    for _ in range(5):
        try:
            os.replace(tmp, path)
            return
        except PermissionError:     # Windows: another hook has the file open; retry briefly
            time.sleep(0.02)
    try:
        os.remove(tmp)
    except OSError:
        pass


def append_line(path, text):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "a", encoding="utf-8") as f:
        f.write(text.rstrip("\n") + "\n")


def now_iso():
    return time.strftime("%Y-%m-%dT%H:%M:%S")


def norm_path(p):
    return os.path.normcase(os.path.normpath(str(p).replace("\\", "/"))) if p else ""


# ----------------------------------------------------------------------------------------------- stop flag

def read_stop(main):
    """None when no STOP flag, else {reason, since, by}."""
    p = stop_path(main)
    if not os.path.exists(p):
        return None
    d = read_json(p)
    if not isinstance(d, dict):
        try:
            with open(p, encoding="utf-8", errors="replace") as f:
                d = {"reason": f.read().strip()[:300]}
        except OSError:
            d = {}
    d.setdefault("reason", "no reason given")
    d.setdefault("since", time.strftime("%Y-%m-%dT%H:%M:%S", time.localtime(os.path.getmtime(p))))
    return d


def write_stop(main, reason, by="coordinator"):
    write_json(stop_path(main), {"reason": reason, "since": now_iso(), "by": by})


def clear_stop(main):
    try:
        os.remove(stop_path(main))
        return True
    except FileNotFoundError:
        return False


# ----------------------------------------------------------------------------------------------- rounds and parking

QID = re.compile(r"\b[A-Z]{1,4}-?Q?\d{1,4}\b")


def rounds_path(main, lane):
    return os.path.join(guard_dir(main), f"{lane}.rounds.jsonl")


def read_rounds(main, lane):
    """Recorded results since the last unpark marker, one per agent (its last StructuredOutput), oldest first."""
    res = []
    try:
        with open(rounds_path(main, lane), encoding="utf-8") as f:
            lines = f.read().splitlines()
    except OSError:
        return res
    for line in lines:
        try:
            r = json.loads(line)
        except ValueError:
            continue
        if r.get("unpark"):
            res = []
            continue
        if res and r.get("agent_id") and res[-1].get("agent_id") == r.get("agent_id"):
            res[-1] = r     # a schema retry of the same agent: keep the last
        else:
            res.append(r)
    return res


def blocked_key(r):
    """What a blocked result was blocked on: the question IDs it names, else the guard trip, else its text."""
    if r.get("guard") in ("watchdog", "park"):
        return "guard:" + r["guard"]
    q = str(r.get("questions") or "")
    ids = sorted(set(QID.findall(q)))
    if ids:
        return "q:" + ",".join(ids)
    qn = re.sub(r"[^a-z0-9]+", " ", q.lower()).strip()
    if qn and qn not in ("none", "n a", "na"):
        return "qt:" + qn[:120]
    s = re.sub(r"[^a-z0-9]+", " ", str(r.get("summary") or "").lower()).strip()
    return "s:" + s[:80]


def park_reason(main, lane):
    """A reason string when the lane's last two results ended blocked on the same thing, else None. A STOP-caused
    block does not count."""
    rs = [r for r in read_rounds(main, lane) if r.get("guard") != "stop"]
    if len(rs) < 2:
        return None
    a, b = rs[-2], rs[-1]
    if a.get("state") != "blocked" or b.get("state") != "blocked":
        return None
    ka, kb = blocked_key(a), blocked_key(b)
    if ka != kb:
        return None
    what = kb.split(":", 1)[1]
    if kb.startswith("guard:"):
        what = "a watchdog trip (no commit within the call limit)"
    return f"the last two rounds ({a.get('label') or '?'}, {b.get('label') or '?'}) both ended blocked on {what[:120]}"


def unpark(main, lane, note=""):
    """Release a parked lane: a marker in its rounds ledger, and its call counter back to zero (a lane blocked by
    the watchdog that did not commit would otherwise trip again on its first call)."""
    append_line(rounds_path(main, lane), json.dumps({"unpark": True, "ts": now_iso(), "note": note}))
    lp = os.path.join(guard_dir(main), f"{lane}.json")
    ls = read_json(lp, None)
    if ls:
        ls["calls"] = 0
        write_json(lp, ls)


# ----------------------------------------------------------------------------------------------- REA pre-flight

def rea_config():
    try:
        with open(os.path.join(os.path.expanduser("~"), ".claude.json"), encoding="utf-8") as f:
            d = json.load(f)
    except (OSError, ValueError):
        return None
    return (d.get("mcpServers") or {}).get("rea")


def rea_processes():
    """Running `rea.mjs mcp` servers (one per Claude Code process that connected REA); None when unknown."""
    if os.name != "nt":
        try:
            o = subprocess.run(["ps", "-eo", "args"], capture_output=True, text=True, timeout=10).stdout
        except (OSError, subprocess.SubprocessError):
            return None
        return sum(1 for l in o.splitlines() if "rea.mjs" in l and " mcp" in l)
    ps = ("Get-CimInstance Win32_Process -Filter \"Name='node.exe'\" | Where-Object { $_.CommandLine -match "
          "'rea\\.mjs\\S*\\s+mcp' } | Measure-Object | ForEach-Object { $_.Count }")
    try:
        o = subprocess.run(["powershell", "-NoProfile", "-Command", ps], capture_output=True, text=True,
                           timeout=25, creationflags=NOWIN).stdout.strip()
        return int(o.splitlines()[-1]) if o else 0
    except (OSError, subprocess.SubprocessError, ValueError, IndexError):
        return None


def rea_handshake(cfg, timeout=45):
    """Start our own `rea.mjs mcp` over stdio, initialize and list tools. (ok, detail). Does not open a binary or
    start Ghidra (~2 s)."""
    import threading
    env = dict(os.environ, **(cfg.get("env") or {}))
    try:
        p = subprocess.Popen([cfg["command"], *(cfg.get("args") or [])], stdin=subprocess.PIPE,
                             stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, env=env, creationflags=NOWIN)
    except (OSError, KeyError) as e:
        return False, f"cannot start the REA server: {e}"
    box = {}

    def talk():
        try:
            def send(o):
                p.stdin.write((json.dumps(o) + "\n").encode())
                p.stdin.flush()

            def recv(want):
                while True:
                    line = p.stdout.readline()
                    if not line:
                        return None
                    try:
                        r = json.loads(line)
                    except ValueError:
                        continue
                    if r.get("id") == want:
                        return r
            send({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
                "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "laneguard", "version": "1"}}})
            if not recv(1):
                box["err"] = "no initialize reply"
                return
            send({"jsonrpc": "2.0", "method": "notifications/initialized"})
            send({"jsonrpc": "2.0", "id": 2, "method": "tools/list"})
            r = recv(2)
            box["tools"] = [t.get("name") for t in ((r or {}).get("result") or {}).get("tools", [])]
        except Exception as e:  # noqa: BLE001 - report, do not crash the pre-flight
            box["err"] = str(e)

    th = threading.Thread(target=talk, daemon=True)
    th.start()
    th.join(timeout)
    try:
        p.kill()
    except OSError:
        pass
    p.wait(timeout=5)
    th.join(timeout=2)
    p.stdin.close()
    p.stdout.close()
    if th.is_alive():
        return False, f"no MCP reply within {timeout} s"
    if "err" in box:
        return False, box["err"]
    need = {"procedure_pseudo_code", "batch_decompile"}
    missing = need - set(box.get("tools") or [])
    if missing:
        return False, f"server answers but lacks {', '.join(sorted(missing))}"
    return True, f"{len(box['tools'])} tools; metadata only, target readiness UNKNOWN"


def rea_check(handshake=True, session=None, lane=None):
    """Only a retained actual research client can establish target admission.

    Legacy handshake=False and keeper/process counts cannot bypass this gate.
    """
    if not session:
        return False, "REA-DOWN no admitted same-client target; use lane_launch with a reviewed target contract (Claude RE unsupported)"
    sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(HERE)), 'tools'))
    try:
        import rea_session
        result = rea_session.require_ready(session, lane)
        return True, f"REA target ready (same client {result['session_id']}; typed identity verified)"
    except Exception as e:
        return False, f"REA-DOWN target admission failed: {e}"


# ----------------------------------------------------------------------------------------------- preflight / status

def preflight(main, lane, kind, handshake=True, session=None):
    """(code, line): the single verdict line the workflow's gate agent returns."""
    st = read_stop(main)
    if st:
        return 1, f"STOP {st['reason']} (since {st['since']}; python tools/lanes.py go clears it)"
    pr = park_reason(main, lane)
    if pr:
        return 1, f"PARK {pr} (python tools/lanes.py unpark {lane} releases it)"
    if kind == "re":
        ok, line = rea_check(handshake, session=session, lane=lane)
        if not ok:
            return 1, line
    return 0, "GO"


def lane_state(main, lane):
    return read_json(os.path.join(guard_dir(main), f"{lane}.json"), {}) or {}


def status_lines(main, lane=None):
    gd = guard_dir(main)
    out = []
    st = read_stop(main)
    out.append(f"STOP: {st['reason']} (since {st['since']})" if st else "STOP: not set")
    names = []
    if lane:
        names = [lane]
    elif os.path.isdir(gd):
        names = sorted({f.split(".")[0] for f in os.listdir(gd) if f.endswith(".json") or f.endswith(".rounds.jsonl")})
    for n in names:
        s = lane_state(main, n)
        rs = read_rounds(main, n)
        last = rs[-1] if rs else {}
        pr = park_reason(main, n)
        out.append(f"{n}: {s.get('calls', 0)} calls since commit {str(s.get('head') or '-')[:9]}, "
                   f"{s.get('total', 0)} total, last trip {s.get('last_trip') or '-'}; "
                   f"last result {last.get('state') or '-'}" + (f"; PARKED: {pr}" if pr else ""))
    errs = os.path.join(gd, "errors.log")
    if os.path.exists(errs):
        try:
            with open(errs, encoding="utf-8", errors="replace") as f:
                tail = f.read().splitlines()[-3:]
            out += [f"guard error: {t[:200]}" for t in tail]
        except OSError:
            pass
    return out


# ----------------------------------------------------------------------------------------------- the hook

def find_agent_meta(transcript_path, session_id, agent_id):
    """(meta dict, agent transcript path) of the subagent, found next to the session transcript the hook names
    (hook input transcript_path is the PARENT session's; measured 2026-10-21, Claude Code 2.1.304)."""
    if not (transcript_path and agent_id):
        return None, None
    base = os.path.dirname(transcript_path)
    sess = session_id or os.path.splitext(os.path.basename(transcript_path))[0]
    sub = os.path.join(base, sess, "subagents")
    cands = [os.path.join(sub, f"agent-{agent_id}.meta.json")]
    try:
        cands += [os.path.join(sub, "workflows", d, f"agent-{agent_id}.meta.json")
                  for d in os.listdir(os.path.join(sub, "workflows"))]
    except OSError:
        pass
    for m in cands:
        if os.path.exists(m):
            return read_json(m, {}) or {}, m[: -len(".meta.json")] + ".jsonl"
    return None, None


LIMITS = re.compile(r"lane guard: budget=(\d+) calls=(\d+)")


def read_limits(transcript):
    """(budget, calls) from the lane prompt (the first user record of the agent's transcript)."""
    try:
        with open(transcript, "rb") as f:
            head = f.read(256 * 1024).decode("utf-8", errors="replace")
    except OSError:
        return DEFAULT_BUDGET, DEFAULT_CALLS
    m = LIMITS.search(head)
    if m:
        return int(m.group(1)), int(m.group(2))
    return DEFAULT_BUDGET, DEFAULT_CALLS


def context_tokens(transcript):
    sys.path.insert(0, HERE)
    try:
        import ctx  # .claude/lane-bin/ctx.py
        return ctx.call_context(transcript)
    except (OSError, ImportError):
        return None


def worktree_head(wt):
    """HEAD commit of a worktree from its files (no git process); '' when there is no worktree yet."""
    dotgit = os.path.join(wt, ".git")
    try:
        if os.path.isfile(dotgit):
            with open(dotgit, encoding="utf-8") as f:
                gd = f.read().strip()[len("gitdir:"):].strip()
            gd = os.path.normpath(os.path.join(wt, gd))
        elif os.path.isdir(dotgit):
            gd = dotgit
        else:
            return ""
        with open(os.path.join(gd, "HEAD"), encoding="utf-8") as f:
            head = f.read().strip()
        if not head.startswith("ref:"):
            return head
        ref = head[4:].strip()
        common = gd
        cd = os.path.join(gd, "commondir")
        if os.path.exists(cd):
            with open(cd, encoding="utf-8") as f:
                common = os.path.normpath(os.path.join(gd, f.read().strip()))
        for base in (gd, common):
            rp = os.path.join(base, *ref.split("/"))
            if os.path.exists(rp):
                with open(rp, encoding="utf-8") as f:
                    return f.read().strip()
        with open(os.path.join(common, "packed-refs"), encoding="utf-8") as f:
            for line in f:
                if line.rstrip().endswith(" " + ref):
                    return line.split()[0]
    except OSError:
        pass
    # unusual layout: ask git
    try:
        p = subprocess.run(["git", "-C", wt, "rev-parse", "HEAD"], capture_output=True, text=True, timeout=10,
                           creationflags=NOWIN)
        return p.stdout.strip() if p.returncode == 0 else ""
    except (OSError, subprocess.SubprocessError):
        return ""


GIT_OK = {"add", "commit", "status", "log", "rev-parse", "diff"}


def bash_allowed(cmd, wt):
    """True when a Bash command only stages/commits/inspects the lane's own worktree."""
    if not cmd or "`" in cmd or "$(" in cmd or "<<" in cmd:
        return False
    w = norm_path(wt)
    for seg in re.split(r"&&|\|\||;|\n", cmd):
        seg = seg.strip()
        if not seg:
            continue
        parts = [p.strip() for p in seg.split("|")]
        for tail in parts[1:]:
            if not re.fullmatch(r"(head|tail)(\s+-n\s*\d+|\s+-\d+)?", tail):
                return False
        main_part = re.sub(r"\s+2>(&1|/dev/null)\s*$", "", parts[0])
        if re.search(r"[<>]", main_part):
            return False
        m = re.match(r"""git\s+-C\s+("[^"]+"|'[^']+'|\S+)((?:\s+-c\s+\S+)*)\s+(\S+)""", main_part)
        if not m or m.group(3) not in GIT_OK:
            return False
        if norm_path(m.group(1).strip("\"'")) != w:
            return False
    return True


def file_allowed(path, wt, lane):
    p = norm_path(path)
    if p == norm_path(os.path.join(wt, "docs", "lanes", f"{lane}.md")):
        return True
    d = norm_path(os.path.join(wt, "build-runs"))
    return os.path.dirname(p) == d and re.fullmatch(r"msg-.*\.txt", os.path.basename(p), re.I) is not None


def call_allowed_when_tripped(tool, tin, wt, lane):
    if tool in ("Read", "Write", "Edit"):
        return file_allowed(tin.get("file_path"), wt, lane)
    if tool == "Bash":
        return bash_allowed(tin.get("command") or "", wt)
    return False


def commit_how(wt, lane):
    return (f"Allowed now: Read/Edit/Write of {wt}/docs/lanes/{lane}.md and {wt}/build-runs/msg-<lane>-<n>.txt "
            f"(write the commit message with the Write tool), Bash made only of `git -C {wt} add <files>`, "
            f"`git -C {wt} commit -F <msgfile>`, `git -C {wt} status|log|rev-parse|diff` (| head/tail allowed), "
            f"and your final structured result. Everything else is denied by the harness, whatever you do.")


def deny_message(reason, info, wt, lane):
    lf = f"{wt}/docs/lanes/{lane}.md"
    if reason == "stop":
        return (f"LANE GUARD - STOP: the coordinator has stopped all lanes ({info}). Do not start new work. Update the "
                f"## Handoff in {lf} with where you are, commit it, and return state \"blocked\" with summary "
                f"\"STOP: <one line on where you stopped>\". Write no merge-queue file. " + commit_how(wt, lane))
    if reason == "park":
        return (f"LANE GUARD - PARKED: {info}. This lane may not run another round until the coordinator releases "
                f"it. Return state \"blocked\" now, summary \"parked: <the open question>\". " + commit_how(wt, lane))
    if reason == "budget":
        return (f"LANE GUARD - CONTEXT BUDGET: {info}. Update the ## Handoff in {lf} (goal, done with hashes, the exact "
                f"next step, facts a successor needs), commit it, and return state \"continue\" with the tip. A fresh "
                f"agent continues from that file. Write no merge-queue file. " + commit_how(wt, lane))
    return (f"LANE GUARD - NO PROGRESS: {info}. Stop exploring. Write what you found so far into {lf} (what you "
            f"tried, what blocks you, the exact question), commit it, and return state \"blocked\" with the reason in "
            f"summary (and the question in questions). " + commit_how(wt, lane))


def emit_deny(msg):
    sys.stdout.write(json.dumps({"hookSpecificOutput": {
        "hookEventName": "PreToolUse", "permissionDecision": "deny", "permissionDecisionReason": msg}}))
    sys.stdout.flush()


def hook(data, main=None):
    """Decide one PreToolUse call. Returns (decision, message) with decision 'allow' or 'deny'; 'pass' for calls
    that are none of the guard's business."""
    if data.get("agent_type") not in LANE_TYPES:
        return "pass", ""
    main = main or main_root()
    meta, transcript = find_agent_meta(data.get("transcript_path"), data.get("session_id"), data.get("agent_id"))
    label = (meta or {}).get("description") or ""
    if not label.startswith("lane:"):
        return "pass", ""   # the workflow's gate agent, or an fc4-lane agent spawned by hand
    lane = label[5:].split("#")[0]
    agent_id = data["agent_id"]
    return guard_call(main, lane, agent_id, label, data.get("tool_name") or "",
                      data.get("tool_input") or {}, transcript)


def guard_call(main, lane, agent_id, label, tool, tin, transcript, *, limits=None,
               context_reader=context_tokens, allowed_when_tripped=call_allowed_when_tripped):
    """Provider-independent policy; adapters supply identity, context usage and tool syntax."""
    gd = guard_dir(main)
    wt = worktree(main, lane).replace("\\", "/")
    apath = os.path.join(gd, "agents", f"{agent_id}.json")
    ag = read_json(apath, None)
    if ag is None:
        budget, calls = limits or read_limits(transcript)
        ag = {"lane": lane, "label": label, "transcript": transcript, "budget": budget, "max_calls": calls,
              "started": now_iso(), "trip": None}
    if tool == "StructuredOutput":
        append_line(rounds_path(main, lane), json.dumps({
            "ts": now_iso(), "agent_id": agent_id, "label": label, "state": tin.get("state"),
            "tip": tin.get("tip"), "questions": tin.get("questions"), "summary": str(tin.get("summary") or "")[:300],
            "guard": ag.get("trip")}))
        return "allow", ""

    ls_path = os.path.join(gd, f"{lane}.json")
    ls = read_json(ls_path, {}) or {}
    head = worktree_head(wt)
    if ls.get("head") != head:
        ls["head"], ls["calls"] = head, 0
    reason, info = None, ""
    st = read_stop(main)
    if st:
        reason, info = "stop", f"{st['reason']}, since {st['since']}"
    if not reason:
        if ag.get("trip") == "park" or (not ag.get("calls") and park_reason(main, lane)):
            reason, info = "park", park_reason(main, lane) or ag.get("trip_info") or "parked"
    if not reason and ag.get("trip") == "watchdog":
        # a watchdog trip lasts for the agent's life (committing does not buy another 150 calls of exploring) and
        # outranks the budget: the lane must end "blocked", not "continue" into a fresh round
        reason, info = "watchdog", ag.get("trip_info") or "watchdog tripped"
    if not reason:
        t = context_reader(transcript) if transcript else None
        if t and t > ag["budget"]:
            reason, info = "budget", f"context {t // 1000}k is over the lane budget {ag['budget'] // 1000}k"
    if not reason:
        if ls["calls"] + 1 > ag["max_calls"]:
            reason = "watchdog"
            info = (f"{ls['calls']} tool calls since this worktree's last commit"
                    f"{' ' + head[:9] if head else ' (no worktree commit yet)'} (limit {ag['max_calls']})")
    ag["calls"] = ag.get("calls", 0) + 1
    if reason:
        allowed = allowed_when_tripped(tool, tin, wt, lane)
        if not ag.get("trip") or ag["trip"] in ("budget", "stop"):
            if ag.get("trip") != reason:
                ag["trip"], ag["trip_info"] = reason, info
        ls["last_trip"] = f"{reason} {now_iso()}"
        if not allowed:
            append_line(os.path.join(gd, f"{lane}.log"), f"{now_iso()} deny {reason} {agent_id} {tool}: {info}")
            write_json(apath, ag)
            write_json(ls_path, ls)
            return "deny", deny_message(reason, info, wt, lane)
        write_json(apath, ag)
        write_json(ls_path, ls)
        return "allow", ""
    ls["calls"] = ls.get("calls", 0) + 1
    ls["total"] = ls.get("total", 0) + 1
    ls["agent"] = agent_id
    write_json(apath, ag)
    write_json(ls_path, ls)
    return "allow", ""


def run_hook():
    raw = sys.stdin.buffer.read().decode("utf-8", errors="replace")
    try:
        data = json.loads(raw)
    except ValueError:
        return 0
    if data.get("agent_type") not in LANE_TYPES:    # fast path: every other session and agent
        return 0
    try:
        decision, msg = hook(data)
    except Exception as e:  # noqa: BLE001 - a guard bug must not block lanes; it is logged and shown by lanes.py
        try:
            append_line(os.path.join(guard_dir(main_root()), "errors.log"),
                        f"{now_iso()} {type(e).__name__}: {e} (tool {data.get('tool_name')}, agent {data.get('agent_id')})")
        except OSError:
            pass
        return 0
    if decision == "deny":
        emit_deny(msg)
    return 0


def main(argv=None):
    import argparse
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("hook")
    p = sub.add_parser("preflight")
    p.add_argument("lane")
    p.add_argument("--kind", default="engine", choices=("engine", "re", "tool", "mech"))
    p.add_argument("--no-handshake", action="store_true")
    p = sub.add_parser("status")
    p.add_argument("lane", nargs="?")
    a = ap.parse_args(argv)
    if a.cmd == "hook":
        return run_hook()
    main_dir = main_root()
    if a.cmd == "preflight":
        code, line = preflight(main_dir, a.lane, a.kind, not a.no_handshake)
        print(line)
        return code
    for line in status_lines(main_dir, a.lane):
        print(line)
    return 0


if __name__ == "__main__":
    sys.exit(main())
