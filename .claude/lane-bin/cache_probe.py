#!/usr/bin/env python3
"""Real train278 regression, usable by pytest and by the shared native worker."""
import json
import os
from pathlib import Path
import re
import shutil
import socket
import subprocess
import sys
import tempfile
import time

import cache_compile
from cache_server import foreground_server
import fc4verify


def queued_probe(root):
    """Use the native worker when present; inspect this request's fresh CTest proof.

    Ninja's Windows process pipes cannot run in the lane sandbox. This runs the
    same probe via CTest, rather than skipping it or trusting an earlier result.
    The CLI below always runs directly, so CTest cannot recursively queue itself.
    """
    import qbuild
    root = Path(root)
    alive = fc4verify.main_root(root) / "build-runs/build-queue-worker.alive"
    if not alive.exists() or time.time() - alive.stat().st_mtime > 60:
        return probe(root)
    started = time.time_ns()
    gate_passed = qbuild.queued_fc4verify(str(root), ["build", "ctest"])
    log = root / "engine/build-x86/Testing/Temporary/LastTest.log"
    if log.stat().st_mtime_ns < started:
        raise AssertionError("native worker returned an old CTest log")
    text = log.read_text(encoding="utf-8", errors="replace")
    block = re.search(r'^\d+/\d+ Testing: compiler_cache_dependency_regression\n'
                      r'(.*?)(?=^\d+/\d+ Testing:|\Z)', text, re.M | re.S)
    if not block or not re.search(r'^Test Passed\.$', block[1], re.M):
        raise AssertionError("fresh CTest log has no passing compiler cache regression")
    for line in block[1].splitlines():
        if line.startswith('{"cache":'):
            result = json.loads(line)
            if "mapped_header_rebuilt_object_and_test" in result:
                if not gate_passed:
                    # The native worker runs all CTest tests, including Cargo.
                    # This tools test owns only the fresh, passing cache proof.
                    # Keep the full gate's failure in its logs and console output.
                    print("native full gate failed; fresh compiler cache regression passed")
                return result
    raise AssertionError("fresh CTest log contains no real cache regression proof")


def probe(root):
    root = Path(root)
    (root / "build-runs").mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="cache-regression-", dir=root / "build-runs") as tmp:
        tmp = Path(tmp)
        a, b = tmp / "a", tmp / "b"
        a.mkdir()
        b.mkdir()
        with socket.socket() as sock:
            sock.bind(("127.0.0.1", 0))
            port = sock.getsockname()[1]
        env = fc4verify.build_env(root, "cache-regression")
        env.update(SCCACHE_SERVER_PORT=str(port), SCCACHE_DIR=str(tmp / "cache"),
                   SCCACHE_BASEDIRS=os.pathsep.join(map(str, (a, b))), SCCACHE_LOG="debug")
        cache = fc4verify.sccache_exe()
        compiler = shutil.which("cl", path=env["PATH"])

        def run(command, cwd=tmp):
            command = [shutil.which(command[0], path=env["PATH"]) or command[0], *command[1:]]
            try:
                result = subprocess.run(command, cwd=cwd, env=env, capture_output=True,
                                        text=True, errors="replace", timeout=30)
            except subprocess.TimeoutExpired as exc:
                raise RuntimeError(f"compiler timed out: {exc.stdout!r}; {''.join(debug_lines)[-3000:]}") from exc
            if result.returncode:
                raise RuntimeError(result.stdout + result.stderr)
            return result.stdout

        def fixture(directory, mapped):
            launcher = f'"{cache}"'
            if mapped:
                launcher = (f'"{sys.executable}" "{cache_compile.__file__}" --root "{directory}" '
                            f'--from-root "{a}" --from-root "{b}" -- ' + launcher)
            rules = (f'rule cc\n  command = {launcher} "{compiler}" /nologo /showIncludes '
                     '/c main.cpp /Foobject.obj\n  deps = msvc\n'
                     f'rule link\n  command = "{compiler}" /nologo object.obj /Fesmoke.exe\n'
                     'build object.obj: cc main.cpp\nbuild smoke.exe: link object.obj\n'
                     'default smoke.exe\n')
            (directory / "build.ninja").write_text(rules, encoding="utf-8", newline="\n")
            (directory / "value.h").write_text("#define VALUE 1\n", encoding="utf-8")
            (directory / "main.cpp").write_text(
                '#include "value.h"\nint main() { return VALUE == 1 ? 0 : 1; }\n', encoding="utf-8")

        def require(condition, message):
            if not condition:
                raise AssertionError(message)

        def has_header(deps, header):
            # Ninja canonicalizes absolute dependencies below its working drive
            # into relative paths. Resolve them before comparing worktree roots.
            return any((b / line.strip()).resolve() == header.resolve()
                       for line in deps.splitlines() if line.strip().endswith("value.h"))

        with foreground_server(cache, env) as debug_lines:
            fixture(a, False)
            run(["ninja"], a)
            fixture(b, False)
            run(["ninja"], b)
            stats = json.loads(run([cache, "--show-stats", "--stats-format", "json"]))["stats"]
            require(sum(stats["cache_hits"]["counts"].values()) >= 1, "must exercise a cache hit")
            deps = run(["ninja", "-t", "deps"], b).replace("\\", "/").lower()
            require(has_header(deps, a / "value.h"), "reproduce foreign dependency: " + deps)
            (b / "value.h").write_text("#define VALUE 2\n", encoding="utf-8")
            require("no work to do" in run(["ninja"], b), "reproduce missing rebuild")
            require(subprocess.run([str(b / "smoke.exe")]).returncode == 0, "reproduce stale success")
            for name in ("object.obj", "smoke.exe", ".ninja_deps", ".ninja_log"):
                (b / name).unlink(missing_ok=True)
            fixture(b, True)
            before = json.loads(run([cache, "--show-stats", "--stats-format", "json"]))["stats"]
            run(["ninja"], b)
            after = json.loads(run([cache, "--show-stats", "--stats-format", "json"]))["stats"]
            require(after["cache_hits"]["counts"].get("C/C++", 0)
                    > before["cache_hits"]["counts"].get("C/C++", 0),
                    "mapped B must reuse A's cached object")
            deps = run(["ninja", "-t", "deps"], b).replace("\\", "/").lower()
            require(has_header(deps, b / "value.h"), "mapped dependency must name B: " + deps)
            require(not has_header(deps, a / "value.h"), "no foreign dependency remains: " + deps)
            obj_time = (b / "object.obj").stat().st_mtime_ns
            exe_time = (b / "smoke.exe").stat().st_mtime_ns
            (b / "value.h").write_text("#define VALUE 2\n", encoding="utf-8")
            require("no work to do" not in run(["ninja"], b), "header edit must trigger rebuild")
            require((b / "object.obj").stat().st_mtime_ns > obj_time, "object must rebuild")
            require((b / "smoke.exe").stat().st_mtime_ns > exe_time, "test must relink")
            require(subprocess.run([str(b / "smoke.exe")]).returncode == 1, "rebuilt test must see new header")
            return {"unsafe_stale_success": True, "mapped_header_rebuilt_object_and_test": True,
                    "cache": json.loads(run([cache, "--show-stats", "--stats-format", "json"]))["stats"]}


if __name__ == "__main__":
    print(json.dumps(probe(Path(__file__).resolve().parents[2]), sort_keys=True))
