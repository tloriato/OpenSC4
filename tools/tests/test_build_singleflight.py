"""Actual OS lock/process tests. No Cargo, CMake, SDK or game runs.

Run on Windows to exercise msvcrt locks + named-pipe peer identity, and Linux for
flock + SO_PEERCRED. Other POSIX systems can run root checks; verified delegation
fails closed where the platform has no implemented peer-credential API.
"""
import json
import os
from pathlib import Path
import select
import signal
import socket
import subprocess
import sys
import tempfile
import threading
import unittest
from multiprocessing.connection import Listener

ROOT = Path(__file__).resolve().parents[2]
BIN = ROOT / ".claude" / "lane-bin"
DRIVER = Path(__file__).parent / "fixtures" / "singleflight_driver.py"
sys.path.insert(0, str(BIN))
import build_lease as lease

SUPPORTED = os.name == "nt" or hasattr(socket, "SO_PEERCRED")


def wait_process(pid):
    """Bounded OS process-object wait, not a PID-existence polling loop."""
    if os.name == "nt":
        import ctypes
        from ctypes import wintypes
        k = ctypes.WinDLL("kernel32", use_last_error=True)
        k.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
        k.OpenProcess.restype = wintypes.HANDLE
        k.WaitForSingleObject.argtypes = [wintypes.HANDLE, wintypes.DWORD]
        k.CloseHandle.argtypes = [wintypes.HANDLE]
        handle = k.OpenProcess(0x100000, False, pid)
        if not handle:
            return  # already gone
        try:
            if k.WaitForSingleObject(handle, 15000) != 0:
                raise AssertionError(f"process {pid} did not finish")
        finally:
            k.CloseHandle(handle)
    elif hasattr(os, "pidfd_open"):
        try:
            fd = os.pidfd_open(pid)
        except ProcessLookupError:
            return
        try:
            if not select.select([fd], [], [], 15)[0]:
                raise AssertionError(f"process {pid} did not finish")
        finally:
            os.close(fd)
    else:
        raise unittest.SkipTest("bounded orphan-process wait needs Windows handles or Linux pidfd")


@unittest.skipUnless(SUPPORTED, "verified delegation implemented for Windows and Linux")
class SingleflightTests(unittest.TestCase):
    def setUp(self):
        runs = ROOT / "build-runs"
        runs.mkdir(exist_ok=True)
        self.tmp = tempfile.TemporaryDirectory(prefix="singleflight-", dir=runs)
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name)
        self.server = socket.socket()
        self.server.bind(("127.0.0.1", 0))
        self.server.listen(16)
        self.server.settimeout(15)
        self.addCleanup(self.server.close)
        self.env = dict(os.environ)
        self.env.pop(lease.INVITE, None)
        self.env["FC4_TEST_SYNC"] = "127.0.0.1:" + str(self.server.getsockname()[1])
        self.children = []
        self.connections = []
        self.addCleanup(self.cleanup_processes)
        self.wt = self.worktree("one")

    def cleanup_processes(self):
        for conn in self.connections:
            try:
                conn.sendall(b"go\n")
            except OSError:
                pass
            conn.close()
        for p in self.children:
            if p.poll() is None:
                p.kill()
            try:
                p.communicate(timeout=30)
            except subprocess.TimeoutExpired:
                pass

    def worktree(self, name):
        wt = self.base / name
        (wt / "engine").mkdir(parents=True)
        shim = wt / "build-runs" / "driver" / "fc4verify.py"
        shim.parent.mkdir(parents=True)
        shim.write_text("import runpy, sys\nsys.argv.insert(1, 'fc4verify')\n"
                        + f"runpy.run_path({str(DRIVER)!r}, run_name='__main__')\n", encoding="utf-8")
        (wt / "source-state.json").write_text(json.dumps({"head": "HEAD1", "tree": "TREE1", "dirty": []}),
                                               encoding="utf-8")
        return wt

    def start(self, kind="qbuild", steps=("all",), wt=None, extra=(), env=None):
        child_env = dict(self.env)
        if env:
            child_env.update(env)
        p = subprocess.Popen([sys.executable, str(DRIVER), kind, "--wt", str(wt or self.wt), *extra, *steps],
                             env=child_env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
                             encoding="utf-8")
        self.children.append(p)
        return p

    def event(self, stage, identity=None):
        conn, _ = self.server.accept()
        conn.settimeout(15)
        self.connections.append(conn)
        with conn.makefile("rb") as stream:
            data = json.loads(stream.readline())
        self.assertEqual(stage, data["stage"], data)
        if identity:
            self.assertEqual(identity, data["id"], data)
        return conn, data

    def go(self, conn):
        conn.sendall(b"go\n")
        conn.close()
        self.connections.remove(conn)

    def finish(self, p, stages=(), ok=True):
        for stage in stages:
            conn, _ = self.event(stage)
            self.go(conn)
        stdout, stderr = p.communicate(timeout=20)
        if ok:
            self.assertEqual(p.returncode, 0, stdout + stderr)
            self.assertNotIn("ownership uncertain", stderr)
        else:
            self.assertNotEqual(p.returncode, 0, stdout + stderr)
        return stdout + stderr

    def owner(self, wt=None):
        return json.loads(((wt or self.wt) / "build-runs" / lease.OWNER).read_text(encoding="utf-8"))

    def sentinels(self):
        names = ["qbuild-test.log", "qbuild-clippy.log", "fc4verify-quick.log", "fc4verify-build.log",
                 "fc4verify-ctest.log", "verdict.json"]
        files = {self.wt / "build-runs" / name: ("sentinel " + name).encode() for name in names}
        for path, data in files.items():
            path.write_bytes(data)
        return files

    def assert_unchanged(self, files):
        self.assertEqual({str(p): p.read_bytes() for p in files}, {str(p): b for p, b in files.items()})

    def busy(self, kind="qbuild", steps=("all",), wt=None, extra=(), env=None):
        p = self.start(kind, steps, wt, extra, env)
        text = self.finish(p, ok=False)
        self.assertIn("busy", text)
        for needle in ("worktree=", "owner PID=", "guardian PID=", "started=", "source=", "command="):
            self.assertIn(needle, text)
        self.assertNotIn("PASS", text)
        return text

    def test_all_all_and_all_quick_leave_logs_verdict_and_environment_untouched(self):
        first = self.start()
        held, _ = self.event("test")
        files = self.sentinels()  # first command already opened its log; no more writes until released
        self.busy()
        self.busy(steps=("quick",))
        self.assert_unchanged(files)
        self.assertFalse((self.wt / "context-entered").exists())
        self.go(held)
        text = self.finish(first, ("clippy", "context", "build", "ctest"))
        self.assertIn("qbuild: PASS", text)
        self.assertEqual(self.owner()["status"], "released")

    def test_qbuild_competes_with_cli_and_imported_verify_even_with_other_log_dir(self):
        first = self.start()
        held, _ = self.event("test")
        files = self.sentinels()
        other = self.wt / "other-logs"
        self.busy("fc4verify", ("gate",), extra=("--log-dir", str(other), "--json", str(self.wt / "build-runs/verdict.json")))
        self.busy("imported", (), env={"FC4_TEST_LOG_DIR": str(other)})
        self.assertFalse(other.exists())
        self.assertFalse((self.wt / "context-entered").exists())
        self.assert_unchanged(files)
        self.go(held)
        self.finish(first, ("clippy", "context", "build", "ctest"))

    def test_cli_and_imported_owners_block_qbuild_and_hold_verdict_publication(self):
        for kind in ("fc4verify", "imported"):
            with self.subTest(kind=kind):
                first = self.start(kind, ("quick",) if kind == "fc4verify" else (),
                                   extra=("--json", str(self.wt / "build-runs/verdict.json")) if kind == "fc4verify" else (),
                                   env={"FC4_TEST_HOLD_JSON": "1"})
                held, _ = self.event("context")
                self.busy(steps=("verify",))
                self.go(held)
                held, _ = self.event("quick")
                self.go(held)
                held, _ = self.event("publication")
                self.busy(steps=("quick",))
                self.go(held)
                self.finish(first)
                rec = json.loads((self.wt / "build-runs/verdict.json").read_text())
                self.assertTrue(rec["ok"])
                self.assertEqual((rec["head"], rec["tree"], rec["dirty"]), ("HEAD1", "TREE1", []))
                self.assertEqual(rec["ownership"]["invocation"], self.owner()["invocation"])
                self.assertTrue(rec["started"] and rec["finished"] and rec["steps"]["quick"]["log"])

    def test_intended_delegation_and_copied_inherited_stale_forged_tokens(self):
        first = self.start(steps=("quick",), env={"FC4_TEST_DOUBLE_JOIN": "1"})
        held, data = self.event("context")
        token = data["invite"]
        self.assertTrue(token)
        self.assertIsNone(data["raw_invite"], "ordinary commands must not inherit delegation")
        self.assertTrue((self.wt / "second-branch-rejected").is_file())
        files = self.sentinels()
        bad_key = json.loads(token)
        bad_key["key"] = "00" * 32
        unauthenticated = self.start(steps=("quick",), env={lease.INVITE: json.dumps(bad_key)})
        self.assertIn("failed delegation authentication", self.finish(unauthenticated, ok=False))
        sibling = self.start(steps=("quick",), env={lease.INVITE: token})
        self.assertIn("delegation rejected", self.finish(sibling, ok=False))
        other = self.worktree("different")
        different = self.start(steps=("quick",), wt=other, env={lease.INVITE: token})
        self.assertIn("different canonical worktree", self.finish(different, ok=False))
        forged = self.start(steps=("quick",), env={lease.INVITE: "{}"})
        self.assertIn("invalid/stale delegation", self.finish(forged, ok=False))
        self.assert_unchanged(files)
        self.go(held)
        self.finish(first, ("quick",))
        stale = self.start(steps=("quick",), env={lease.INVITE: token})
        self.assertIn("stale/forged delegation", self.finish(stale, ok=False))
        retry = self.start(steps=("quick",))
        self.finish(retry, ("context", "quick"))

    def test_forged_server_with_copied_live_token_is_not_a_guardian(self):
        first = self.start(steps=("quick",))
        held, data = self.event("context")
        invite = json.loads(data["invite"])
        address = (r"\\.\pipe\fc4-forged-" + str(os.getpid()) if os.name == "nt"
                   else str(self.base / "forged.sock"))
        server = Listener(address, authkey=bytes.fromhex(invite["key"]))
        self.addCleanup(server.close)
        errors = []

        def fake_guardian():
            try:
                with server.accept() as conn:
                    conn.recv()  # client must close before sending any ownership request
                    errors.append("client trusted the forged endpoint")
            except EOFError:
                pass

        thread = threading.Thread(target=fake_guardian, daemon=True)
        thread.start()
        invite["address"] = address
        forged = self.start(steps=("quick",), env={lease.INVITE: json.dumps(invite)})
        self.assertIn("server is not the live guardian", self.finish(forged, ok=False))
        thread.join(10)
        self.assertFalse(thread.is_alive())
        self.assertEqual(errors, [])
        self.go(held)
        self.finish(first, ("quick",))

    def test_original_cli_entrypoints_reject_before_discovery_or_log_writes(self):
        first = self.start("lease", ())
        held, _ = self.event("lease")
        files = self.sentinels()
        for tool in ("qbuild.py", "fc4verify.py"):
            p = subprocess.run([sys.executable, str(BIN / tool), "--wt", str(self.wt), "quick"],
                               env=self.env, capture_output=True, text=True, timeout=10)
            self.assertNotEqual(p.returncode, 0)
            self.assertIn("build/check busy", p.stdout + p.stderr)
            self.assertNotIn("PASS", p.stdout + p.stderr)
        self.assert_unchanged(files)
        self.assertFalse((self.wt / "context-entered").exists())
        self.go(held)
        self.finish(first)

    def test_separate_worktrees_and_log_dirs_retain_parallelism(self):
        other = self.worktree("two")
        first = self.start("fc4verify", ("quick",), env={"FC4_TEST_ID": "one"})
        one, _ = self.event("context", "one")
        second = self.start("fc4verify", ("quick",), wt=other,
                            extra=("--log-dir", str(other / "elsewhere")), env={"FC4_TEST_ID": "two"})
        two, _ = self.event("context", "two")
        self.assertNotEqual(self.owner()["guardian_pid"], self.owner(other)["guardian_pid"])
        self.go(two)
        self.finish(second, ("quick",))
        self.go(one)
        self.finish(first, ("quick",))

    def test_relative_absolute_dot_case_and_symlink_or_junction_alias(self):
        first = self.start("lease", ())
        held, _ = self.event("lease")
        aliases = [self.wt.resolve(), self.wt / "engine" / "..", Path(os.path.relpath(self.wt))]
        if os.name == "nt":
            aliases.append(Path(str(self.wt).upper()))
        alias = self.base / "alias"
        if os.name == "nt":
            p = subprocess.run(["cmd", "/d", "/c", "mklink", "/J", str(alias), str(self.wt)],
                               capture_output=True, text=True, timeout=10)
            self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
        else:
            alias.symlink_to(self.wt, target_is_directory=True)
        try:
            aliases.append(alias)
            for wt in aliases:
                with self.subTest(wt=str(wt)):
                    self.busy(steps=("quick",), wt=wt)
        finally:
            if os.name == "nt":
                os.rmdir(alias)  # remove the junction alone, never its target
            else:
                alias.unlink()
        self.go(held)
        self.finish(first)

    def test_fmt_is_independent_and_does_not_create_build_logs(self):
        first = self.start("lease", ())
        held, _ = self.event("lease")
        files = self.sentinels()
        fmt = self.start(steps=("fmt", "fake.rs"))
        self.finish(fmt, ("fmt",))
        self.assert_unchanged(files)
        self.go(held)
        self.finish(first)

    def test_unknown_steps_and_invalid_paths_fail_before_any_context_or_log(self):
        files = self.sentinels()
        for kind in ("qbuild", "fc4verify"):
            text = self.finish(self.start(kind, ("test", "invalid")), ok=False)
            self.assertIn("unknown", text)
        notdir = self.wt / "file"
        notdir.write_text("file")
        self.assertIn("not a directory", self.finish(self.start("fc4verify", ("quick",),
                          extra=("--log-dir", str(notdir))), ok=False))
        self.assertIn("invalid verdict path", self.finish(self.start("fc4verify", ("quick",),
                          extra=("--json", str(self.wt / "build-runs" / lease.LOCK))), ok=False))
        self.assertFalse((self.wt / "context-entered").exists())
        self.assertFalse((self.wt / "build-runs" / lease.OWNER).exists())
        self.assert_unchanged(files)

    def test_exception_cleanup_and_changed_source_remains_untrusted(self):
        first = self.start("fc4verify", ("quick",), env={"FC4_TEST_CONTEXT_ERROR": "1"})
        self.assertIn("intentional context exception", self.finish(first, ("context",), ok=False))
        self.assertEqual(self.owner()["status"], "released")
        second = self.start("imported", ())
        held, _ = self.event("context")
        self.go(held)
        held, _ = self.event("quick")
        (self.wt / "source-state.json").write_text(json.dumps({"head": "HEAD2", "tree": "TREE2", "dirty": ["M x"]}))
        self.go(held)
        self.finish(second)
        rec = json.loads((self.wt / "build-runs/verdict.json").read_text())
        self.assertTrue(rec["changed_during_run"])
        self.assertEqual(rec["head"], "HEAD1")

    def test_real_failed_command_publishes_failure_and_releases_for_retry(self):
        first = self.start("imported", (), env={"FC4_TEST_FAIL_STAGE": "quick"})
        self.assertIn("FAIL", self.finish(first, ("context", "quick"), ok=False))
        rec = json.loads((self.wt / "build-runs/verdict.json").read_text())
        self.assertFalse(rec["ok"])
        self.assertFalse(rec["steps"]["quick"]["ok"])
        self.assertIn("intentional harmless command failure", Path(rec["steps"]["quick"]["log"]).read_text())
        self.assertEqual(self.owner()["status"], "released")
        self.finish(self.start(steps=("quick",)), ("context", "quick"))

    def test_wrapper_crash_keeps_raw_command_protected_then_recovers(self):
        first = self.start()
        held, _ = self.event("test")
        guardian = self.owner()["guardian_pid"]
        first.kill()
        files = self.sentinels()
        self.busy(steps=("quick",))
        self.assert_unchanged(files)
        self.go(held)
        wait_process(guardian)
        self.finish(first, ok=False)
        self.assertEqual(self.owner()["status"], "released")
        retry = self.start(steps=("quick",))
        self.finish(retry, ("context", "quick"))

    def test_parent_crash_during_live_delegate_preserves_exclusion(self):
        first = self.start(steps=("quick",))
        held, _ = self.event("context")
        guardian = self.owner()["guardian_pid"]
        first.kill()
        self.busy(steps=("quick",))
        self.go(held)
        held, _ = self.event("quick")
        self.busy("fc4verify", ("quick",))
        self.go(held)
        wait_process(guardian)
        self.finish(first, ok=False)
        self.assertEqual(self.owner()["status"], "released")
        self.finish(self.start(steps=("quick",)), ("context", "quick"))

    def test_delegated_child_crash_does_not_release_its_live_command(self):
        first = self.start(steps=("verify",))
        held, _ = self.event("context")
        self.go(held)
        held, data = self.event("build")
        os.kill(int(data["wrapper"]), signal.SIGTERM)
        self.busy(steps=("all",))
        self.go(held)
        self.assertIn("qbuild: FAIL", self.finish(first, ok=False))
        self.assertEqual(self.owner()["status"], "released")
        self.finish(self.start(steps=("quick",)), ("context", "quick"))

    def test_guardian_crash_fails_closed_instead_of_using_pid_or_age(self):
        first = self.start("lease", ())
        held, data = self.event("lease")
        owner = self.owner()
        os.kill(owner["guardian_pid"], signal.SIGTERM)
        wait_process(owner["guardian_pid"])
        files = self.sentinels()
        text = self.finish(self.start(steps=("quick",)), ok=False)
        self.assertIn("ownership uncertain after guardian failure", text)
        self.assertIn("manually archiving", text)
        self.assert_unchanged(files)
        self.assertEqual(self.owner()["status"], "active")
        self.go(held)
        wait_process(data["pid"])
        self.finish(first, ok=False)
        # Explicit recovery only after the supervised raw command has really exited.
        (self.wt / "build-runs" / lease.OWNER).rename(self.wt / "archived-owner.json")
        self.finish(self.start(steps=("quick",)), ("context", "quick"))


if __name__ == "__main__":
    unittest.main()
