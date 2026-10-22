"""Second-round regressions against the actual helper and wrapper drivers.

All descendants are test-owned DEVNULL writers. Release sockets are signalled
BEFORE waiting for the wrapper; process handles are retained across contenders.
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import time
import unittest
from multiprocessing.connection import Client

from tools.tests import test_build_singleflight as base

lease = base.lease


@unittest.skipUnless(os.name == "nt", "Windows Job/process-handle regressions")
class RepairTests(unittest.TestCase):
    # Share fixture mechanics, without collecting the original tests twice.
    setUp = base.SingleflightTests.setUp
    cleanup_processes = base.SingleflightTests.cleanup_processes
    worktree = base.SingleflightTests.worktree
    start = base.SingleflightTests.start
    event = base.SingleflightTests.event
    go = base.SingleflightTests.go
    finish = base.SingleflightTests.finish
    owner = base.SingleflightTests.owner
    sentinels = base.SingleflightTests.sentinels
    assert_unchanged = base.SingleflightTests.assert_unchanged
    busy = base.SingleflightTests.busy

    def next_event(self):
        conn, _ = self.server.accept()
        conn.settimeout(15)
        self.connections.append(conn)
        with conn.makefile("rb") as stream:
            return conn, json.loads(stream.readline())

    def tree_case(self, mode, capture, depth=0, crash=None, expire=False):
        output = self.wt / "owned-output.bin"
        env = {"FC4_TEST_TREE_STAGE": "tree" if capture else "test", "FC4_TEST_TREE_MODE": mode,
               "FC4_TEST_DEPTH": str(depth), "FC4_TEST_WRITER_PATH": str(output)}
        first = self.start("tree-capture" if capture else "qbuild", () if capture else ("test",), env=env)
        held, _ = self.event(env["FC4_TEST_TREE_STAGE"])
        self.go(held)
        events = {}
        while len(events) < 2:
            conn, data = self.next_event()
            self.assertIn(data["stage"], ("parent-spawned", "writer"))
            events[data["stage"]] = (conn, data)
        writer, data = events["writer"]
        job = lease._WinJob(self.owner()["jobs"][-1], existing=True)
        self.addCleanup(job.close)
        handle, created, image = job.identity(data["pid"])
        self.addCleanup(job.k.CloseHandle, handle)
        self.assertGreater(created, 0)
        self.assertEqual(image, os.path.normcase(sys._base_executable))
        parent_conn, parent = events["parent-spawned"]
        parent_handle, _, _ = job.identity(parent["pid"])
        self.addCleanup(job.k.CloseHandle, parent_handle)
        self.go(parent_conn)
        if mode == "timeout":
            held, _ = self.event("timeout-held")
            self.assertEqual(job.k.WaitForSingleObject(parent_handle, 5000), 0)
            # Parent was terminated by its own real command deadline, not by test cleanup.
            held.close()
            self.connections.remove(held)
        else:
            self.assertEqual(job.k.WaitForSingleObject(parent_handle, 5000), 0)
        # Retain PROCESS handles, never an observer JOB handle across a crash.
        # Such an observer would accidentally keep the production evidence alive.
        members = []
        for pid in job.pids():
            member, birth, executable = job.identity(pid)
            members.append((pid, member, birth, executable))
            self.addCleanup(job.k.CloseHandle, member)
        job.close()
        if crash == "wrapper":
            first.kill()
        elif crash == "guardian":
            os.kill(self.owner()["guardian_pid"], base.signal.SIGTERM)
            base.wait_process(self.owner()["guardian_pid"])
        files = self.sentinels()
        self.assertEqual(job.k.WaitForSingleObject(handle, 0), 258)
        evidence = {"test": self.id(), "mode": mode, "capture": capture, "depth": depth, "crash": crash,
                    "owner": self.owner(), "writer_pid": data["pid"], "writer_created": created,
                    "writer_image": image, "direct_pid": parent["pid"], "writer_alive_at_contender": True}
        if crash == "guardian":
            self.assertIn("ownership uncertain", self.finish(self.start(steps=("quick",)), ok=False))
        else:
            self.busy(steps=("quick",))
        self.assert_unchanged(files)
        self.assertEqual(job.k.WaitForSingleObject(handle, 0), 258)
        self.assertFalse(output.exists())
        if expire:
            # Bounded supervision may fail closed, but must never publish released.
            self.assertIn("descendants still live", self.finish(first, ok=False))
            self.assertEqual(self.owner()["status"], "active")
            self.assertIn("ownership uncertain", self.finish(self.start(steps=("quick",)), ok=False))
            self.assert_unchanged(files)
        self.go(writer)
        done, same = self.event("writer-done")
        self.assertEqual(same["pid"], data["pid"])
        self.assertEqual(output.read_bytes(), b"owned output\n")
        self.go(done)
        self.assertEqual(job.k.WaitForSingleObject(handle, 5000), 0)
        for pid, member, birth, executable in members:
            self.assertEqual(job.k.WaitForSingleObject(member, 5000), 0, (pid, birth, executable))
        evidence["writer_exited_after_release"] = True
        if not expire:
            text = self.finish(first, ok=mode == "normal" and crash is None)
            if mode == "timeout":
                self.assertIn("timed out", text)
        if expire or crash == "guardian":
            self.assertIn("ownership uncertain", self.finish(self.start(steps=("quick",)), ok=False))
            # The owned fixture cannot spawn after its writer event. Every live
            # member was retained, then signalled exited, before explicit archive.
            (self.wt / "build-runs" / lease.OWNER).rename(self.wt / "archived-owner.json")
        self.finish(self.start(steps=("quick",)), ("context", "quick"))
        self.assertEqual(self.owner()["status"], "released")
        evidence["recovery_status"] = self.owner()["status"]
        with (base.ROOT / "build-runs" / f"r2-process-evidence-{os.getpid()}.jsonl").open("a", encoding="utf-8") as stream:
            stream.write(json.dumps(evidence) + "\n")
        output.unlink()

    def test_normal_file(self):
        self.tree_case("normal", False)

    def test_normal_capture(self):
        self.tree_case("normal", True)

    def test_abrupt_file(self):
        self.tree_case("abrupt", False)

    def test_abrupt_capture(self):
        self.tree_case("abrupt", True)

    def test_nonzero_file(self):
        self.tree_case("nonzero", False)

    def test_nonzero_capture(self):
        self.tree_case("nonzero", True)

    def test_timeout_file(self):
        self.tree_case("timeout", False)

    def test_timeout_capture(self):
        self.tree_case("timeout", True)

    def test_global_and_supplied_venv_launch_and_peer_identity(self):
        for exe in (sys._base_executable, "C:/Users/tlori/FreeCity4/build-runs/pytest-venv/Scripts/python.exe"):
            with self.subTest(executable=exe):
                self.assertTrue(Path(exe).is_file())
                env = dict(self.env, FC4_TEST_DOUBLE_JOIN="1")
                first = subprocess.Popen([exe, str(base.DRIVER), "qbuild", "--wt", str(self.wt), "quick"],
                                         env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
                self.children.append(first)
                held, data = self.event("context")
                identity = self.owner()["delegations"][-1]
                self.assertEqual(identity["peer_pid"], int(data["wrapper"]))
                if Path(exe).resolve() == Path(sys._base_executable).resolve():
                    self.assertEqual(identity["peer_pid"], identity["launch_pid"])
                else:
                    self.assertNotEqual(identity["peer_pid"], identity["launch_pid"])
                self.assertTrue((self.wt / "second-branch-rejected").is_file())
                self.go(held)
                text = self.finish(first, ("quick",))
                with (base.ROOT / "build-runs" / f"r2-peer-evidence-{os.getpid()}.jsonl").open("a", encoding="utf-8") as stream:
                    stream.write(json.dumps({"executable": exe, "identity": identity, "stdout_stderr": text}) + "\n")

    def test_multigeneration_early_exit(self):
        self.tree_case("normal", True, depth=3)

    def test_guardian_crash_retains_kernel_tree_identity_and_recovers(self):
        self.tree_case("normal", True, depth=2, crash="guardian")

    def test_wrapper_crash_with_live_descendant(self):
        self.tree_case("abrupt", False, crash="wrapper")

    def test_bounded_failclosed_then_kernel_proven_recovery(self):
        self.tree_case("normal", True, expire=True)

    def test_halfopen_malformed_abandoned_auth_does_not_block_real_join_or_close(self):
        self.halfopen_case("normal")

    def test_halfopen_cleanup_with_delegate_exception(self):
        self.halfopen_case("exception")

    def test_halfopen_cleanup_after_parent_crash(self):
        self.halfopen_case("parent-crash")

    def halfopen_case(self, mode):
        env = {"FC4_TEST_PRE_JOIN": "1"}
        if mode == "exception":
            env["FC4_TEST_CONTEXT_ERROR"] = "1"
        first = self.start(steps=("quick",), env=env)
        held, data = self.event("before-join")
        invite = json.loads(data["invite"])
        peer = Client(invite["address"], authkey=None)
        self.addCleanup(peer.close)
        # A real pipe connection that never answers the challenge remains open.
        abandoned = Client(invite["address"], authkey=None)
        abandoned.close()
        malformed = Client(invite["address"], authkey=None)
        malformed.send_bytes(b"invalid authentication answer")
        malformed.close()
        started = time.monotonic()
        self.go(held)
        held, _ = self.event("context")
        guardian = self.owner()["guardian_pid"]
        if mode == "parent-crash":
            first.kill()
        self.busy(steps=("quick",))
        self.go(held)
        self.finish(first, () if mode == "exception" else ("quick",), ok=mode == "normal")
        base.wait_process(guardian)
        self.assertLess(time.monotonic() - started, 10)
        self.assertEqual(self.owner()["status"], "released")


if __name__ == "__main__":
    unittest.main()
