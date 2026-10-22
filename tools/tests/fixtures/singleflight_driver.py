"""Harmless subprocess fixture for real wrapper/lease tests; never invokes a build tool.

Only the build context and command payloads are substituted. The wrappers' actual
drivers, log writers, parsers, delegation, verify and JSON publication run unchanged.
Each command/publication waits on a test-owned socket, not a duration-based sleep.
"""
import json
import os
from pathlib import Path
import socket
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / ".claude" / "lane-bin"))
import build_lease as lease
import fc4verify as f
import qbuild as q

DRIVER = str(Path(__file__).resolve())


def event(stage):
    host, port = os.environ["FC4_TEST_SYNC"].split(":")
    with socket.create_connection((host, int(port)), timeout=25) as sock:
        data = {"stage": stage, "pid": os.getpid(), "parent_pid": os.getppid(),
                "executable": sys.executable, "base_executable": sys._base_executable,
                "wrapper": os.environ.get("FC4_TEST_WRAPPER_PID"),
                "invite": os.environ.get("FC4_TEST_CAPTURED_INVITE"),
                "raw_invite": os.environ.get(lease.INVITE), "id": os.environ.get("FC4_TEST_ID")}
        sock.sendall(json.dumps(data).encode() + b"\n")
        with sock.makefile("rb") as stream:
            if stream.readline() != b"go\n":
                raise RuntimeError("test synchronization channel closed")


def command(stage):
    event(stage)
    if os.environ.get("FC4_TEST_TREE_STAGE") == stage:
        subprocess.Popen([sys.executable, DRIVER, "descendant", os.environ.get("FC4_TEST_DEPTH", "0")],
                         stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        event("parent-spawned")
        mode = os.environ["FC4_TEST_TREE_MODE"]
        if mode == "abrupt":
            os._exit(73)
        if mode == "nonzero":
            return 3
        if mode == "timeout":
            event("timeout-held")
    if os.environ.get("FC4_TEST_FAIL_STAGE") == stage:
        print("error: intentional harmless command failure")
        return 3
    print("    Finished `test` profile [unoptimized] target(s) in 0.01s")
    print("test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out")
    print("[1/1] harmless Python command")
    print("100% tests passed, 0 tests failed out of 1")
    print("Total Test time (real) = 0.01 sec")
    return 0


class FakeCtx:
    def __init__(self, wt, log_dir, tag, use_cache):
        self.wt = Path(wt)
        self.engine = self.wt / "engine"
        self.build = self.engine / "fake-build"
        self.log_dir = Path(log_dir)
        self.log_dir.mkdir(parents=True, exist_ok=True)
        self.env = dict(os.environ)
        self.cached = False
        self.results = {}
        if os.environ.get("FC4_TEST_DOUBLE_JOIN") and os.environ.get("FC4_TEST_CAPTURED_INVITE"):
            os.environ[lease.INVITE] = os.environ["FC4_TEST_CAPTURED_INVITE"]
            try:
                with lease.Lease(wt):
                    raise AssertionError("second child branch joined the lease")
            except lease.LeaseError:
                (self.wt / "second-branch-rejected").write_text("rejected", encoding="utf-8")
        (self.wt / "context-entered").write_text("entered", encoding="utf-8")
        lease.run_process([sys.executable, DRIVER, "command", "context"], env=self.env, capture_output=True)
        if os.environ.get("FC4_TEST_CONTEXT_ERROR"):
            raise RuntimeError("intentional context exception")

    def log(self, stage):
        return self.log_dir / f"fc4verify-{stage}.log"


def fake_step(ctx, args, name):
    rc, text, secs = f.run([sys.executable, DRIVER, "command", name], ctx.engine, ctx.env, ctx.log(name))
    parser = {"test": f.parse_cargo_test, "clippy": f.parse_clippy, "ctest": f.parse_ctest}.get(name, f.parse_ninja)
    return {**parser(text), "seconds": secs, "log": str(ctx.log(name))}


def install(wt):
    original_run = q.run

    def harmless_run(cmd, cwd, log, env=None):
        if os.environ.get("FC4_TEST_TREE_MODE") == "timeout":
            p = lease.run_process([sys.executable, DRIVER, "command", cmd[1]], cwd=cwd, log=log,
                                  env=env, timeout=1)
            return p.returncode, Path(log).read_text().splitlines()
        return original_run([sys.executable, DRIVER, "command", cmd[1]], cwd, log, env)

    q.run = harmless_run
    original_process = q.run_process

    def harmless_fmt(cmd, **kwargs):
        if cmd[0] == "rustfmt":
            cmd = [sys.executable, DRIVER, "command", "fmt"]
        return original_process(cmd, **kwargs)

    q.run_process = harmless_fmt
    # qbuild's actual delegation helper launches this isolated fixture shim.
    q.__file__ = str(Path(wt) / "build-runs" / "driver" / "qbuild.py")
    f.Ctx = FakeCtx
    for name in f.STEPS:
        f.STEPS[name] = lambda ctx, a, name=name: fake_step(ctx, a, name)

    def state(wt):
        return json.loads((Path(wt) / "source-state.json").read_text(encoding="utf-8"))

    f.tree_state = state
    original_write = Path.write_text

    def held_write(path, data, *args, **kwargs):
        if path.name == "verdict.json" and os.environ.get("FC4_TEST_HOLD_JSON"):
            event("publication")
        return original_write(path, data, *args, **kwargs)

    Path.write_text = held_write


def main():
    kind = sys.argv.pop(1)
    if kind == "command":
        return command(sys.argv[1])
    if kind == "descendant":
        depth = int(sys.argv[1])
        if depth:
            subprocess.Popen([sys.executable, DRIVER, "descendant", str(depth - 1)],
                             stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            os._exit(0)
        event("writer")
        with open(os.environ["FC4_TEST_WRITER_PATH"], "ab") as stream:
            stream.write(b"owned output\n")
        event("writer-done")
        return 0
    wt = sys.argv[sys.argv.index("--wt") + 1]
    os.environ["FC4_TEST_WRAPPER_PID"] = str(os.getpid())
    if lease.INVITE in os.environ:
        os.environ["FC4_TEST_CAPTURED_INVITE"] = os.environ[lease.INVITE]
        if os.environ.get("FC4_TEST_PRE_JOIN"):
            event("before-join")
    install(wt)
    if kind == "qbuild":
        return q.main()
    if kind == "fc4verify":
        return f.main(sys.argv[1:])
    if kind == "imported":
        rec = f.verify(wt, ["quick"], log_dir=os.environ.get("FC4_TEST_LOG_DIR"),
                       json_path=Path(wt) / "build-runs" / "verdict.json")
        return 0 if rec["ok"] else 1
    if kind == "lease":
        with lease.Lease(wt):
            lease.run_process([sys.executable, DRIVER, "command", "lease"], capture_output=True)
        return 0
    if kind == "tree-capture":
        with lease.Lease(wt):
            p = lease.run_process([sys.executable, DRIVER, "command", "tree"], capture_output=True,
                                  timeout=1 if os.environ["FC4_TEST_TREE_MODE"] == "timeout" else None)
            print(p.stdout.decode() if isinstance(p.stdout, bytes) else p.stdout)
            return p.returncode
    raise ValueError(kind)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as exc:
        print(str(exc), file=sys.stderr, flush=True)
        sys.exit(2)
