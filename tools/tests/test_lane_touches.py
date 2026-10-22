"""Canonical coverage subsystem names must remain intact in lane overlap checks."""
import importlib.util
import json
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("lane_touches_tests", REPO / "tools/lanes.py")
lanes = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = lanes
spec.loader.exec_module(lanes)


class TouchKeyTests(unittest.TestCase):
    def test_empty(self):
        self.assertEqual(lanes.split_touch_keys(None), [])
        self.assertEqual(lanes.split_touch_keys(" , "), [])

    def test_simple_keys_keep_existing_behavior(self):
        self.assertEqual(lanes.split_touch_keys(" Terrain ,TRAFFIC, "), ["terrain", "traffic"])

    def test_canonical_comma_names_are_not_generic_fragments(self):
        self.assertEqual(lanes.split_touch_keys(
            "automata (managers, records),automata (vehicles, animation),data formats (DBPF, exemplars)"),
            ["automata (managers, records)", "automata (vehicles, animation)",
             "data formats (dbpf, exemplars)"])

    def test_nested_names_and_malformed_input(self):
        self.assertEqual(lanes.split_touch_keys("a (b (c,d),e), f"), ["a (b (c,d),e)", "f"])
        for invalid in ["automata (managers, records", "terrain),traffic"]:
            with self.assertRaisesRegex(ValueError, "unbalanced"):
                lanes.split_touch_keys(invalid)


class OverlapGuardTests(unittest.TestCase):
    """Exercise the actual launch check with independent lane and legacy evidence."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        root = Path(self.tmp.name)
        self.ctx = SimpleNamespace(main=root, src=root, branch="main", wt_dir=root / "worktrees")
        self.wt = self.ctx.wt_dir / "existing"
        self.queue = self.wt / "build-runs/merge-queue"
        self.queue.mkdir(parents=True)
        (root / "tools").mkdir()
        (root / "tools/coverage-map.tsv").write_text(
            "A\ttraffic\tx\nB\tnetwork tools\tx\nC\tdispatch\tx\n"
            "D\tdata formats (DBPF, exemplars)\tx\n", encoding="utf-8")
        report = self.wt / "docs/lanes/existing.md"
        report.parent.mkdir(parents=True)
        report.write_text("regress traffic and dispatch; data formats (DBPF, exemplars)", encoding="utf-8")
        self.args = SimpleNamespace(lane="new", kind="engine", deps="", resume=False,
                                    touches="traffic", stale_days=7)
        self.messages = []
        for name, value in [
            ("laneguard", SimpleNamespace(read_stop=lambda _: None, park_reason=lambda *_: None)),
            ("branch_sets", ({"main", "existing"}, {"main"})),
            ("list_worktrees", [{"branch": "existing", "path": self.wt}]),
        ]:
            p = patch.object(lanes, name, return_value=value)
            p.start()
            self.addCleanup(p.stop)
        p = patch.object(lanes, "out", side_effect=self.messages.append)
        p.start()
        self.addCleanup(p.stop)

    def record(self, touches, lane="existing", directory=None):
        directory = directory or self.queue
        directory.mkdir(parents=True, exist_ok=True)
        (directory / f"{lane}.json").write_text(json.dumps(
            {"lane": lane, "branch": lane, "state": "wip", "touches": touches}), encoding="utf-8")

    def test_incidental_regress_words_do_not_claim_other_subsystems(self):
        self.record(["network tools"])
        self.assertEqual(lanes.cmd_check(self.ctx, self.args), 0)

    def test_declared_same_subsystem_still_denies(self):
        self.record(["traffic"])
        self.assertEqual(lanes.cmd_check(self.ctx, self.args), 1)
        self.assertTrue(any("declared subsystem" in m for m in self.messages))

    def test_legacy_or_malformed_claims_keep_conservative_prose_denial(self):
        for value in [None, [], "network tools", ["unknown"], [None]]:
            with self.subTest(value=value):
                self.record(value)
                self.assertEqual(lanes.cmd_check(self.ctx, self.args), 1)

    def test_other_lane_record_cannot_release_legacy_claim(self):
        self.record(["network tools"], lane="someone-else")
        with patch.object(lanes, "worktree_activity", return_value=10**20):
            self.assertEqual(lanes.cmd_check(self.ctx, self.args), 1)

    def test_worktree_and_coordinator_claims_are_unioned(self):
        self.record(["network tools"])
        self.record(["traffic"], directory=self.ctx.main / "build-runs/merge-queue")
        self.assertEqual(lanes.cmd_check(self.ctx, self.args), 1)

    def test_canonical_key_with_comma_is_a_single_claim(self):
        self.record(["data formats (DBPF, exemplars)"])
        self.args.touches = "data formats (DBPF, exemplars)"
        self.assertEqual(lanes.cmd_check(self.ctx, self.args), 1)


if __name__ == "__main__":
    unittest.main()
