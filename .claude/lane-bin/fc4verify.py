#!/usr/bin/env python3
"""fc4verify: build and test one FreeCity4 worktree; every verdict is parsed from the tools' output, never taken
from an exit code.

One implementation for the lanes and for the merge train (tools/merge-train.py imports it for its gate):

  python .claude/lane-bin/fc4verify.py --wt <WT> quick         lane check: cargo test -p <touched crates> (+ the
                                                                crates that depend on them, compiled with --no-run),
                                                                then the CMake build (FreeCity4.dll and the C++ tests)
  python .claude/lane-bin/fc4verify.py --wt <WT> build ctest   the CMake build, then the full ctest (cargo_test too)
  python .claude/lane-bin/fc4verify.py --wt <WT> gate          the merge train's gate: clippy, build, full ctest, and a
                                                                verdict record (<log-dir>/gate-<tag>.json)

Steps: quick, test (cargo test --workspace), clippy, build (configure + build), ctest (all of it), cxxtest (ctest
without cargo_test: the C++ tests only), gate (= clippy build ctest). Options: --crates a,b (quick: test these
instead of the touched ones), --cxx-tests (quick: also run cxxtest), --no-cache, --log-dir, --tag, --json.

Compiler cache: when sccache is installed (`cargo install sccache --version 0.18.0 --locked`) the C++ compiler
(through CMake's FC4_COMPILER_CACHE) and rustc (RUSTC_WRAPPER) go through it. Each worktree gets its own sccache
server port with SCCACHE_BASEDIRS set to the worktree root, so C++ objects hit across worktrees; all servers share
one on-disk cache (%LOCALAPPDATA%\\fc4\\sccache, 20G). Rust hits only within a worktree (sccache hashes rustc's
working directory), e.g. between `cargo test` and ctest's cargo_test. Cargo always runs with CARGO_INCREMENTAL=0
and no debug info (disk is shared; incremental builds are not cacheable), matching the CMake build's cargo env.

Logs: <log-dir>/fc4verify-<step>.log (default <WT>/build-runs). The console gets one line per step, the first
errors (capped) and `fc4verify: PASS|FAIL`. Exit 0 only if every step passed.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
import zlib
from pathlib import Path

from build_lease import Lease, LeaseError, canonical_worktree, ownership, run_process

VS = Path(r"C:\Program Files\Microsoft Visual Studio\2022\Community")
VCVARS = VS / "VC" / "Auxiliary" / "Build" / "vcvars32.bat"
CMAKE_BIN = VS / "Common7" / "IDE" / "CommonExtensions" / "Microsoft" / "CMake" / "CMake" / "bin"
NINJA_DIR = VS / "Common7" / "IDE" / "CommonExtensions" / "Microsoft" / "CMake" / "Ninja"
RUST_TARGET = "i686-pc-windows-msvc"
BUILD_DIR = "build-x86"
# The cargo env of every cargo run here and of the CMake build's cargo commands (engine/CMakeLists.txt,
# FC4_COMPILER_CACHE): sccache hashes every CARGO_* variable, so the two must match to share cache entries.
CARGO_ENV = {"CARGO_INCREMENTAL": "0", "CARGO_PROFILE_DEV_DEBUG": "0", "CARGO_PROFILE_TEST_DEBUG": "0"}
LOCAL = Path(os.environ.get("LOCALAPPDATA") or tempfile.gettempdir()) / "fc4"
# Generated at build time but tracked again since a lane merge re-added it; never a reason to call a tree dirty.
GENERATED = {"engine/tests/abi_entries.inc"}
SDK_REL = Path("engine") / "third_party" / "gzcom-dll"
GATE_STEPS = ("clippy", "build", "ctest")
CTEST_JOBS = 4


# --------------------------------------------------------------------------- parsers (pure; tested)

CARGO_RESULT_RE = re.compile(r"test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored")
RUST_ERROR_RE = re.compile(r"^error(\[E\d+\])?:", re.M)
MSVC_ERROR_RE = re.compile(r"(?:\):|^\s*\S*\s*:)\s*(?:fatal |Command line )?error [A-Z]+\d+\s*:", re.M | re.I)
NINJA_PROGRESS_RE = re.compile(r"^\[(\d+)/(\d+)\]", re.M)
CTEST_SUMMARY_RE = re.compile(r"(\d+)% tests passed, (\d+) tests? failed out of (\d+)")
CTEST_TOTAL_RE = re.compile(r"^Total Tests: (\d+)", re.M)


def parse_cargo_test(text):
    """Sum every `test result:` line. Passes only with at least one result line, no failed test, no FAILED
    result and no error line (a compile error or `error: test failed` stops cargo before the later binaries)."""
    passed = failed = ignored = 0
    results = 0
    bad_result = False
    for m in CARGO_RESULT_RE.finditer(text):
        results += 1
        passed += int(m.group(2))
        failed += int(m.group(3))
        ignored += int(m.group(4))
        bad_result |= m.group(1) != "ok"
    errors = [l.strip() for l in text.splitlines() if RUST_ERROR_RE.match(l)]
    failed_tests = re.findall(r"^test (\S+) \.\.\. FAILED", text, re.M)
    for block in re.findall(r"^failures:\r?\n((?:    \S+\r?\n)+)", text, re.M):  # libtest's closing list
        failed_tests += [n for n in block.split() if n not in failed_tests]
    why = []
    if results == 0:
        why.append("no `test result:` line")
    if failed or bad_result:
        why.append(f"{failed} failed")
    if errors:
        why.append(f"{len(errors)} error line(s)")
    return {"ok": not why, "passed": passed, "failed": failed, "ignored": ignored, "results": results,
            "errors": errors[:20], "failed_tests": failed_tests[:20],
            "summary": f"{passed} passed / {failed} failed / {ignored} ignored in {results} result lines"
            + (f" ({'; '.join(why)})" if why else "")}


def parse_clippy(text):
    """Clean only when cargo reached `Finished` and printed no error line (-D warnings turns lints into errors)."""
    errors = [l.strip() for l in text.splitlines() if RUST_ERROR_RE.match(l)]
    finished = re.search(r"^\s*Finished ", text, re.M) is not None
    why = []
    if errors:
        why.append(f"{len(errors)} error line(s)")
    if not finished:
        why.append("no `Finished` line")
    return {"ok": not why, "errors": errors[:20],
            "summary": ("clean" if not why else "; ".join(why))}


def parse_configure(text):
    ok = "-- Build files have been written to" in text and "CMake Error" not in text
    errors = [l.strip() for l in text.splitlines() if "CMake Error" in l]
    return {"ok": ok, "errors": errors[:20],
            "summary": "configured" if ok else (errors[0] if errors else "no `Build files have been written` line")}


def parse_ninja(text):
    """The CMake/Ninja build passed only when ninja finished its last edge (`[N/N]`, or `no work to do`) with no
    FAILED edge, no `build stopped` and no compiler/linker error line. Ninja's progress lines are parsed, not the
    exit code of whatever wrapped it: train10 landed a tree whose C++ never compiled."""
    lines = text.splitlines()
    failed_edges = [l.strip() for l in lines if l.startswith("FAILED:")]
    stopped = "ninja: build stopped" in text
    errors = [l.strip() for l in lines if MSVC_ERROR_RE.search(l)]
    errors += [l.strip() for l in lines if RUST_ERROR_RE.match(l)]
    no_work = "ninja: no work to do" in text
    prog = NINJA_PROGRESS_RE.findall(text)
    done, total = (int(prog[-1][0]), int(prog[-1][1])) if prog else (0, 0)
    finished = no_work or (prog and done == total)
    why = []
    if failed_edges:
        why.append(f"{len(failed_edges)} FAILED edge(s)")
    if stopped:
        why.append("ninja: build stopped")
    if errors:
        why.append(f"{len(errors)} error line(s)")
    if not finished:
        why.append(f"ninja did not finish ({done}/{total})" if prog else "no ninja progress lines")
    return {"ok": not why, "edges": total, "failed_edges": failed_edges[:20], "errors": errors[:20],
            "summary": ("no work to do" if no_work and not prog else f"{done}/{total} edges")
            + (f" ({'; '.join(why)})" if why else "")}


def parse_ctest_total(text):
    m = CTEST_TOTAL_RE.search(text)
    return int(m.group(1)) if m else None


def parse_ctest(text, expected=None):
    """ctest passed only on `100% tests passed, 0 tests failed out of N` with N the registered count (`ctest -N`,
    less the excluded tests) and nothing Not Run. A build failure that leaves the previous build's binaries in place
    can still print a clean summary: the build step is judged separately and ctest is never run after a red build."""
    m = None
    for m in CTEST_SUMMARY_RE.finditer(text):
        pass
    not_run = re.findall(r"\*\*\*Not Run|\(Not Run\)", text)
    failed = re.findall(r"^\s*\d+ - (\S+) \((?:Failed|Not Run|Timeout|SEGFAULT|Exception|Subprocess aborted)",
                        text, re.M)
    why = []
    if not m:
        why.append("no ctest summary")
        pct, nfail, total = 0, None, None
    else:
        pct, nfail, total = int(m.group(1)), int(m.group(2)), int(m.group(3))
        if nfail:
            why.append(f"{nfail} failed")
        if total == 0:
            why.append("no tests ran")
        if expected is not None and total != expected:
            why.append(f"ran {total} of {expected} registered")
    if not_run:
        why.append(f"{len(not_run)} Not Run")
    if "No tests were found" in text:
        why.append("no tests were found")
    return {"ok": not why, "total": total, "failed": nfail, "expected": expected, "failed_tests": failed[:30],
            "summary": (f"{pct}% passed, {nfail} failed of {total}" if m else "no summary")
            + (f" ({'; '.join(why)})" if why else "")}


PARSERS = {"test": parse_cargo_test, "quick-test": parse_cargo_test, "quick-deps": parse_cargo_test,
           "clippy": parse_clippy, "configure": parse_configure, "build": parse_ninja}


def evaluate_record(rec, required=GATE_STEPS, reparse=True):
    """Judge a verdict record (fc4verify gate's JSON) from its step logs, not from its stored `ok`.

    Returns (ok, reasons). With reparse, every required step's log is parsed again: a record whose `ok` was edited
    by hand, whose log is gone, or whose log no longer says what the record says, fails."""
    why = []
    steps = (rec or {}).get("steps") or {}
    for name in required:
        st = steps.get(name)
        if not st:
            why.append(f"{name}: not run")
            continue
        res = st
        if reparse:
            log = st.get("log")
            try:
                text = Path(log).read_bytes().replace(b"\x00", b"").decode("utf-8", "replace")
            except (OSError, TypeError):
                why.append(f"{name}: log missing ({log})")
                continue
            if name in ("ctest", "cxxtest"):
                res = parse_ctest(text, st.get("expected"))
            elif name == "build":
                res = parse_ninja(text)
                cfg = (steps.get("configure") or {}).get("ok", True)
                if not cfg:
                    res = {"ok": False, "summary": "configure failed"}
            else:
                res = PARSERS[name](text)
        if not res.get("ok"):
            why.append(f"{name}: {res.get('summary')}")
    return (not why), why


# --------------------------------------------------------------------------- environment


def git(wt, *args, check=False, strip=True):
    p = run_process(["git", "-C", str(wt), *args], capture_output=True, text=True, encoding="utf-8",
                       errors="replace")
    if check and p.returncode != 0:
        raise RuntimeError(f"git {' '.join(args)}: {p.stderr.strip()}")
    return p.stdout.strip() if strip else p.stdout


def main_root(wt):
    common = git(wt, "rev-parse", "--path-format=absolute", "--git-common-dir")
    return Path(common).parent if common else Path(wt)


def tree_state(wt):
    """HEAD, its tree, and the files that make the working tree differ from it (tracked edits and untracked,
    non-ignored files; submodules and the generated abi_entries.inc excluded)."""
    head = git(wt, "rev-parse", "HEAD")
    tree = git(wt, "rev-parse", "HEAD^{tree}")
    st = git(wt, "-c", "core.quotepath=off", "status", "--porcelain", "--ignore-submodules=all",
             "--untracked-files=all", strip=False)  # porcelain's first column may be a space
    dirty = []
    for l in st.splitlines():
        if len(l) < 4:
            continue
        path = l[3:].strip().strip('"')
        if " -> " in path:
            path = path.split(" -> ", 1)[1]
        if path in GENERATED or path.startswith(SDK_REL.as_posix() + "/"):
            continue
        dirty.append(l.strip())
    return {"head": head, "tree": tree, "dirty": dirty}


def vcvars_env():
    """The variables vcvars32.bat sets, captured once and cached (PATH as the entries it prepends)."""
    if not VCVARS.exists():
        raise SystemExit(f"fc4verify: no vcvars32.bat at {VCVARS}")
    cache = LOCAL / "vcvars32.json"
    key = f"{VCVARS}|{VCVARS.stat().st_mtime_ns}"
    try:
        d = json.loads(cache.read_text(encoding="utf-8"))
        if d.get("key") == key:
            return d["env"], d["path"]
    except (OSError, ValueError, KeyError):
        pass
    p = run_process(f'cmd /d /s /c ""{VCVARS}" >nul 2>&1 && set"', capture_output=True, text=True,
                       errors="replace")
    after = {}
    for l in p.stdout.splitlines():
        if "=" in l and not l.startswith("="):
            k, v = l.split("=", 1)
            after[k.upper()] = v
    before = {k.upper(): v for k, v in os.environ.items()}
    env = {k: v for k, v in after.items() if k != "PATH" and before.get(k) != v}
    old = {e.lower() for e in before.get("PATH", "").split(";")}
    path = [e for e in after.get("PATH", "").split(";") if e and e.lower() not in old]
    if "INCLUDE" not in env:
        raise SystemExit("fc4verify: vcvars32.bat set no INCLUDE (is the x86 toolset installed?)")
    LOCAL.mkdir(parents=True, exist_ok=True)
    cache.write_text(json.dumps({"key": key, "env": env, "path": path}), encoding="utf-8")
    return env, path


def sccache_exe():
    exe = shutil.which("sccache")
    if exe:
        return exe
    cand = Path.home() / ".cargo" / "bin" / "sccache.exe"
    return str(cand) if cand.exists() else None


def cache_env(wt):
    """sccache settings for this worktree: its own server (port from the path) whose base dir is the worktree."""
    exe = sccache_exe()
    if not exe:
        return {}
    root = str(Path(wt).resolve())
    port = 4300 + zlib.crc32(root.lower().encode("utf-8")) % 600
    return {"RUSTC_WRAPPER": exe, "SCCACHE_SERVER_PORT": str(port), "SCCACHE_BASEDIRS": root,
            "SCCACHE_DIR": os.environ.get("SCCACHE_DIR") or str(LOCAL / "sccache"),
            "SCCACHE_CACHE_SIZE": os.environ.get("SCCACHE_CACHE_SIZE") or "20G",
            "SCCACHE_IGNORE_SERVER_IO_ERROR": "1", "SCCACHE_IDLE_TIMEOUT": "900"}


def build_env(wt, tag, use_cache=True):
    vc, vc_path = vcvars_env()
    env = {k.upper(): v for k, v in os.environ.items()}
    env.update(vc)
    cargo_bin = str(Path.home() / ".cargo" / "bin")
    env["PATH"] = ";".join([str(CMAKE_BIN), str(NINJA_DIR), *vc_path, cargo_bin, env.get("PATH", "")])
    env.update(CARGO_ENV)
    # A private PDB server per worktree/gate: the shared 32-bit mspdbsrv ran out of address space under many
    # parallel lanes (2026-10-14 10:31) and every cl/link hung or failed with LNK1318. /Z7 (engine/CMakeLists.txt)
    # keeps the compiler off it; the linker still writes a PDB.
    env["_MSPDBSRV_ENDPOINT_"] = tag
    env.pop("RUSTC_WRAPPER", None)
    if use_cache:
        env.update(cache_env(wt))
    return env


def ensure_sdk(wt):
    """Worktrees made by the agent tool have no submodule checkout: copy the SDK from the main checkout."""
    sdk = Path(wt) / SDK_REL
    if (sdk / "gzcom-dll" / "include").is_dir():
        return None
    src = main_root(wt) / SDK_REL
    if not (src / "gzcom-dll" / "include").is_dir():
        return f"no gzcom-dll SDK in {sdk} or {src}"
    shutil.copytree(src, sdk, dirs_exist_ok=True, ignore=shutil.ignore_patterns(".git"))
    return None


# --------------------------------------------------------------------------- steps


def run(cmd, cwd, env, log, timeout=None):
    t0 = time.time()
    # CreateProcess looks the program up on this process's PATH, not the child's: resolve it on the child's
    cmd = [shutil.which(str(cmd[0]), path=env.get("PATH")) or cmd[0], *cmd[1:]]
    with open(log, "wb") as f:
        f.write(("$ " + " ".join(str(c) for c in cmd) + "\n").encode("utf-8"))
    try:
        p = run_process([str(c) for c in cmd], cwd=str(cwd), env=env, log=log, append=True,
                        timeout=timeout)
        rc = p.returncode
    except subprocess.TimeoutExpired:
        rc = None
    if rc is None:
        with open(log, "ab") as f:
            f.write(f"\nfc4verify: TIMEOUT after {timeout} s\n".encode("utf-8"))
    text = Path(log).read_bytes().replace(b"\x00", b"").decode("utf-8", "replace")
    return rc, text, round(time.time() - t0, 1)


class Ctx:
    def __init__(self, wt, log_dir, tag, use_cache):
        self.wt = Path(wt).resolve()
        self.engine = self.wt / "engine"
        self.build = self.engine / BUILD_DIR
        self.log_dir = Path(log_dir)
        self.log_dir.mkdir(parents=True, exist_ok=True)
        self.tag = tag
        self.env = build_env(self.wt, tag, use_cache)
        self.cached = "SCCACHE_SERVER_PORT" in self.env
        self.results = {}

    def log(self, step):
        return self.log_dir / f"fc4verify-{step}.log"

    def cargo(self, *args):
        return ["cargo", *args, "--target", RUST_TARGET, "--target-dir", str(self.build / "cargo")]


def step_cargo(ctx, name, args):
    rc, text, secs = run(ctx.cargo(*args), ctx.engine, ctx.env, ctx.log(name), timeout=5400)
    res = parse_cargo_test(text)
    return {**res, "seconds": secs, "log": str(ctx.log(name)), "cmd": "cargo " + " ".join(args)}


def step_test(ctx, _a):
    return step_cargo(ctx, "test", ["test", "--workspace"])


def step_clippy(ctx, _a):
    env = dict(ctx.env)
    env.pop("RUSTC_WRAPPER", None)  # clippy-driver runs as a workspace wrapper; keep lints out of the cache
    cmd = ["cargo", "clippy", "--workspace", "--all-targets", "--target-dir", str(ctx.build / "clippy"),
           "--", "-D", "warnings"]
    rc, text, secs = run(cmd, ctx.engine, env, ctx.log("clippy"), timeout=5400)
    return {**parse_clippy(text), "seconds": secs, "log": str(ctx.log("clippy"))}


def step_build(ctx, _a):
    err = ensure_sdk(ctx.wt)
    if err:
        return {"ok": False, "summary": err, "seconds": 0, "log": None}
    cache = "ON" if ctx.cached else "OFF"
    rc, text, s1 = run(["cmake", "-S", ctx.engine, "-B", ctx.build, "-G", "Ninja", f"-DFC4_COMPILER_CACHE={cache}"],
                       ctx.engine, ctx.env, ctx.log("configure"), timeout=900)
    cfg = parse_configure(text)
    ctx.results["configure"] = {**cfg, "seconds": s1, "log": str(ctx.log("configure"))}
    if not cfg["ok"]:
        return {"ok": False, "summary": f"configure: {cfg['summary']}", "seconds": s1, "log": str(ctx.log("configure")),
                "errors": cfg["errors"]}
    if ctx.cached:
        run_process([ctx.env["RUSTC_WRAPPER"], "--zero-stats"], env=ctx.env, capture_output=True)
    rc, text, s2 = run(["cmake", "--build", ctx.build], ctx.engine, ctx.env, ctx.log("build"), timeout=7200)
    res = parse_ninja(text)
    dll = ctx.build / "FreeCity4.dll"
    out = {**res, "seconds": round(s1 + s2, 1), "log": str(ctx.log("build"))}
    if dll.exists():
        out["dll"] = {"path": str(dll), "sha256": sha256(dll), "mtime": dll.stat().st_mtime}
    elif res["ok"]:
        out.update(ok=False, summary=res["summary"] + " (no FreeCity4.dll)")
    if ctx.cached:
        out["cache"] = cache_stats(ctx.env)
    return out


def ctest_run(ctx, name, exclude=None):
    if not (ctx.results.get("build") or {}).get("ok"):
        why = "skipped: the build did not pass" if "build" in ctx.results else "skipped: run `build` first"
        if "build" not in ctx.results and (ctx.build / "build.ninja").exists():
            why = None  # a standalone ctest/cxxtest on a tree built earlier: allowed, judged as usual
        if why:
            return {"ok": False, "summary": why, "seconds": 0, "log": None}
    sel = ["-E", exclude] if exclude else []
    rc, text, s1 = run(["ctest", "--test-dir", ctx.build, "-N", *sel], ctx.engine, ctx.env, ctx.log(name + "-list"),
                       timeout=300)
    expected = parse_ctest_total(text)
    # -j: the tests that load FreeCity4.dll are RUN_SERIAL (they share FreeCity4.log), so ctest runs them alone;
    # cargo_test and abi_conformance_test (~75 s and ~85 s) overlap instead of queueing.
    rc, text, s2 = run(["ctest", "--test-dir", ctx.build, "--output-on-failure", "-j", str(CTEST_JOBS), *sel],
                       ctx.engine, ctx.env, ctx.log(name), timeout=7200)
    res = parse_ctest(text, expected)
    return {**res, "seconds": round(s1 + s2, 1), "log": str(ctx.log(name))}


def step_ctest(ctx, _a):
    return ctest_run(ctx, "ctest")


def step_cxxtest(ctx, _a):
    return ctest_run(ctx, "cxxtest", exclude="^cargo_test$")


def crate_graph(engine):
    """{dir: (package name, [dirs it depends on, dev-deps included])} for the workspace members."""
    g = {}
    for toml in sorted(Path(engine).glob("*/Cargo.toml")):
        text = toml.read_text(encoding="utf-8", errors="replace")
        m = re.search(r'^\s*name\s*=\s*"([^"]+)"', text, re.M)
        if not m:
            continue
        deps = re.findall(r'path\s*=\s*"\.\./([^"/]+)"', text)
        g[toml.parent.name] = (m.group(1), deps)
    return g


def touched_crates(wt, files, graph):
    """(touched, dependents): workspace members with changed files, and the members that depend on them."""
    touched = set()
    for f in files:
        parts = f.replace("\\", "/").split("/")
        if parts[0] != "engine" or len(parts) < 2:
            continue
        if parts[1] in graph and len(parts) > 2:
            touched.add(parts[1])
        elif parts[1] in ("Cargo.toml", "Cargo.lock", ".cargo", "rustfmt.toml"):
            touched.update(graph)
    deps = set()
    changed = True
    while changed:
        changed = False
        for d, (_n, uses) in graph.items():
            if d not in touched and d not in deps and any(u in touched or u in deps for u in uses):
                deps.add(d)
                changed = True
    return sorted(touched), sorted(deps)


def changed_files(wt):
    base = git(wt, "merge-base", "main", "HEAD")
    files = set(git(wt, "-c", "core.quotepath=off", "diff", "--name-only", base, "--").splitlines()) if base else set()
    files |= set(git(wt, "-c", "core.quotepath=off", "ls-files", "--others", "--exclude-standard").splitlines())
    return sorted(f for f in files if f)


def step_quick(ctx, a):
    graph = crate_graph(ctx.engine)
    if a.crates:
        names = {n: d for d, (n, _u) in graph.items()}
        touched = sorted(names.get(c, c) for c in a.crates.split(","))
        deps = []
        files = []
    else:
        files = changed_files(ctx.wt)
        touched, deps = touched_crates(ctx.wt, files, graph)
    out = {"ok": True, "crates": [graph[d][0] for d in touched if d in graph], "dependents": [], "seconds": 0.0}
    parts = []
    if touched:
        args = ["test"] + [x for d in touched for x in ("-p", graph[d][0] if d in graph else d)]
        r = step_cargo(ctx, "quick-test", args)
        ctx.results["quick-test"] = r
        out["seconds"] += r["seconds"]
        out["ok"] &= r["ok"]
        parts.append(f"cargo test -p {','.join(out['crates'])}: {r['summary']}")
    else:
        parts.append("no Rust crate touched" + (f" ({len(files)} changed files)" if files else ""))
    if deps:
        out["dependents"] = [graph[d][0] for d in deps]
        args = ["test", "--no-run"] + [x for d in deps for x in ("-p", graph[d][0])]
        r = step_cargo(ctx, "quick-deps", args)
        # --no-run prints no `test result:` lines: it passes when it compiled
        r["ok"] = not r["errors"] and "Finished" in Path(r["log"]).read_text(encoding="utf-8", errors="replace")
        r["summary"] = "compiled" if r["ok"] else f"{len(r['errors'])} error line(s)"
        ctx.results["quick-deps"] = r
        out["seconds"] += r["seconds"]
        out["ok"] &= r["ok"]
        parts.append(f"dependents {','.join(out['dependents'])} --no-run: {r['summary']}")
    b = step_build(ctx, a)
    ctx.results["build"] = b
    out["seconds"] = round(out["seconds"] + b["seconds"], 1)
    out["ok"] &= b["ok"]
    parts.append(f"build: {b['summary']}")
    if a.cxx_tests:
        c = step_cxxtest(ctx, a)
        ctx.results["cxxtest"] = c
        out["seconds"] = round(out["seconds"] + c["seconds"], 1)
        out["ok"] &= c["ok"]
        parts.append(f"cxxtest: {c['summary']}")
    out["summary"] = "; ".join(parts)
    out["errors"] = [e for k in ("quick-test", "quick-deps", "build") for e in (ctx.results.get(k) or {}).get("errors", [])][:20]
    return out


STEPS = {"quick": step_quick, "test": step_test, "clippy": step_clippy, "build": step_build, "ctest": step_ctest,
         "cxxtest": step_cxxtest}


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def cache_stats(env):
    p = run_process([env["RUSTC_WRAPPER"], "--show-stats", "--stats-format", "json"], env=env,
                       capture_output=True, text=True, errors="replace")
    try:
        st = json.loads(p.stdout)["stats"]
    except (ValueError, KeyError, TypeError):
        return None

    def counts(k):
        return {lang: n for lang, n in ((st.get(k) or {}).get("counts") or {}).items()}

    return {"hits": counts("cache_hits"), "misses": counts("cache_misses"),
            "non_cacheable": st.get("non_cacheable_compilations")}


def fmt_cache(c):
    if not c:
        return ""
    langs = sorted(set(c["hits"]) | set(c["misses"]))
    if not langs:
        return ""
    return "; sccache " + ", ".join(f"{l} {c['hits'].get(l, 0)} hit / {c['misses'].get(l, 0)} miss" for l in langs)


# --------------------------------------------------------------------------- driver


def verify(wt, steps, log_dir=None, tag=None, use_cache=True, crates=None, cxx_tests=False, quiet=False,
           errors_cap=15, json_path=None):
    """Run `steps` in `wt`; returns the verdict record (also what `gate` writes)."""
    wt = canonical_worktree(wt)
    log_dir = Path(log_dir).resolve() if log_dir else wt / "build-runs"
    if log_dir.exists() and not log_dir.is_dir():
        raise ValueError(f"fc4verify: log directory is not a directory: {log_dir}")
    if json_path:
        json_path = Path(json_path).resolve()
        if json_path.is_dir() or json_path in (wt / "build-runs" / ".build-check.lock",
                                               wt / "build-runs" / ".build-check-owner.json"):
            raise ValueError(f"fc4verify: invalid verdict path: {json_path}")
    steps = list(steps)
    if "gate" in steps:
        steps = [s for s in steps if s != "gate"] + [s for s in GATE_STEPS if s not in steps]
    if not steps or any(s not in STEPS for s in steps):
        raise ValueError(f"fc4verify: unknown/empty steps {steps!r} ({', '.join(STEPS)}, gate)")
    with Lease(wt):
        rec = _verify_owned(wt, steps, log_dir, tag or ("fc4lane-" + wt.name), use_cache, crates,
                            cxx_tests, quiet, errors_cap)
        if json_path:
            json_path.parent.mkdir(parents=True, exist_ok=True)
            json_path.write_text(json.dumps(rec, indent=2) + "\n", encoding="utf-8")
        return rec


def _verify_owned(wt, steps, log_dir, tag, use_cache, crates, cxx_tests, quiet, errors_cap):
    started = time.strftime("%Y-%m-%d %H:%M:%S")
    ctx = Ctx(wt, log_dir, tag, use_cache)
    a = argparse.Namespace(crates=crates, cxx_tests=cxx_tests)
    rec = {"tool": "fc4verify", "version": 1, "worktree": str(wt), "tag": tag, "steps": ctx.results,
           "started": started, "cache": ctx.cached, "ownership": ownership()}
    rec.update(tree_state(wt))
    t0 = time.time()
    for s in steps:
        r = STEPS[s](ctx, a)
        ctx.results[s] = r
        if not quiet:
            extra = fmt_cache(r.get("cache"))
            print(f"fc4verify {s}: {'PASS' if r['ok'] else 'FAIL'} {r['summary']} ({r['seconds']} s{extra})"
                  + (f"  log {r['log']}" if r.get("log") else ""), flush=True)
            if not r["ok"]:
                for e in (r.get("errors") or [])[:errors_cap]:
                    print(f"  {e[:220]}")
                for t in (r.get("failed_tests") or [])[:errors_cap]:
                    print(f"  FAILED {t}")
    after = tree_state(wt)
    if after["head"] != rec["head"] or after["dirty"] != rec["dirty"]:
        rec["changed_during_run"] = True
    rec["seconds"] = round(time.time() - t0, 1)
    rec["finished"] = time.strftime("%Y-%m-%d %H:%M:%S")
    # a tree edited mid-run is flagged; the merge train's judge_gate refuses such a record, a lane only sees the note
    rec["ok"] = all(ctx.results[s]["ok"] for s in steps)
    if not quiet:
        print(f"fc4verify: {'PASS' if rec['ok'] else 'FAIL'} ({rec['seconds']} s)"
              + ("  [the tree changed during the run]" if rec.get("changed_during_run") else ""))
    return rec


def main(argv=None):
    ap = argparse.ArgumentParser(prog="fc4verify", description=__doc__.splitlines()[0],
                                 formatter_class=argparse.RawDescriptionHelpFormatter, epilog=__doc__)
    ap.add_argument("--wt", default=".", help="the worktree (default: the current directory)")
    ap.add_argument("--log-dir", help="where the logs go (default <WT>/build-runs)")
    ap.add_argument("--tag", help="_MSPDBSRV_ENDPOINT_ and the gate record's name (default fc4lane-<WT name>)")
    ap.add_argument("--json", help="also write the verdict record here")
    ap.add_argument("--crates", help="quick: comma-separated crates to test instead of the touched ones")
    ap.add_argument("--cxx-tests", action="store_true", help="quick: also run the C++ tests (ctest -E cargo_test)")
    ap.add_argument("--no-cache", action="store_true", help="do not use sccache")
    ap.add_argument("--errors", type=int, default=15, help="max error lines per step")
    ap.add_argument("steps", nargs="+")
    a = ap.parse_args(argv)
    rec = verify(a.wt, a.steps, a.log_dir, a.tag, not a.no_cache, a.crates, a.cxx_tests,
                 errors_cap=a.errors, json_path=a.json)
    return 0 if rec["ok"] else 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (LeaseError, OSError, ValueError) as exc:
        sys.exit(str(exc))
