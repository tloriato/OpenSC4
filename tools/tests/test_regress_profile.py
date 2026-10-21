"""Candidate replacements must be tested with the full default union and the gated DLL."""
import importlib.util
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / ".claude" / "lane-bin"))
spec = importlib.util.spec_from_file_location("regress_profile_train", REPO / "tools/merge-train.py")
mt = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mt)


class ReplacementProfileTests(unittest.TestCase):
    def test_default_does_not_override_the_house_list(self):
        self.assertIsNone(mt.replacement_union(None, Path("missing")))

    def test_union_preserves_defaults_and_deduplicates_candidates(self):
        with tempfile.TemporaryDirectory() as td:
            p = Path(td) / "regress.ps1"
            p.write_text('    [string]$Replace = "aura, netmgr",\n', encoding="utf-8")
            self.assertEqual(mt.replacement_union("occmgr, aura, lotcfg", p),
                             "aura,netmgr,occmgr,lotcfg")

    def test_invalid_or_unreadable_profiles_fail_closed(self):
        for value in ("", ",", "occmgr; exit", "occmgr -Dll other"):
            with self.subTest(value=value), self.assertRaises(ValueError):
                mt.replacement_union(value, Path("missing"))
        with tempfile.TemporaryDirectory() as td:
            p = Path(td) / "regress.ps1"
            p.write_text("no default list", encoding="utf-8")
            with self.assertRaises(ValueError):
                mt.replacement_union("occmgr", p)

    def test_runner_passes_union_as_one_argument(self):
        for value in (None, "aura,netmgr,occmgr"):
            with self.subTest(value=value), tempfile.TemporaryDirectory() as td:
                result = subprocess.CompletedProcess([], 0, "REGRESS trial: PASS\n", "")
                with patch.object(mt, "LOG_DIR", Path(td)), patch.object(mt, "sh", return_value=result) as sh:
                    verdict, _ = mt.run_regress(Path(td), "trial", Path(td) / "a dll.dll", value)
                self.assertEqual(verdict, "PASS")
                argv = sh.call_args.args[0]
                if value is None:
                    self.assertNotIn("-Replace", argv)
                else:
                    self.assertEqual(argv[argv.index("-Replace") + 1], value)

    def invoke(self, *, gate_error="", profile_error=None, interrupt=False):
        state = {"current": {"id": "trial", "worktree": "wt",
                             "regress": {"ok": True, "verdict": "PASS"}}}
        with patch.object(mt, "load_state", return_value=state), \
             patch.object(mt, "save_state"), patch.object(mt, "integration_head", return_value="head"), \
             patch.object(mt, "batch_touches_engine", return_value=True), \
             patch.object(mt, "gated_dll", return_value=(None, gate_error) if gate_error else (Path("gated.dll"), "")), \
             patch.object(mt, "replacement_union", side_effect=profile_error, return_value="aura,occmgr"), \
             patch.object(mt, "run_regress", side_effect=RuntimeError("interrupted") if interrupt else None,
                          return_value=("PASS", "")) as run:
            args = SimpleNamespace(no_regress=False, add_replace="occmgr")
            if interrupt:
                with self.assertRaises(RuntimeError):
                    mt.cmd_regress(args)
            else:
                mt.cmd_regress(args)
        return state["current"]["regress"], run

    def test_candidate_profile_still_requires_the_gated_dll(self):
        rec, run = self.invoke(gate_error="hash mismatch")
        self.assertFalse(rec["ok"])
        self.assertEqual(rec["verdict"], "REFUSED")
        run.assert_not_called()

    def test_actual_union_is_recorded_and_used(self):
        rec, run = self.invoke()
        self.assertTrue(rec["ok"])
        self.assertEqual(rec["replace"], "aura,occmgr")
        run.assert_called_once_with(Path("wt"), "trial", Path("gated.dll"), "aura,occmgr")

    def test_invalid_retry_cannot_retain_an_earlier_pass(self):
        rec, run = self.invoke(profile_error=ValueError("invalid defaults"))
        self.assertFalse(rec["ok"])
        self.assertEqual(rec["verdict"], "REFUSED")
        run.assert_not_called()

    def test_interrupted_retry_cannot_retain_an_earlier_pass(self):
        rec, _ = self.invoke(interrupt=True)
        self.assertFalse(rec["ok"])
        self.assertEqual(rec["verdict"], "RUNNING")
