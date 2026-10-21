"""Exact verification requires positive evidence that the shadow did not reset."""
import importlib.util
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("coverage_resync", ROOT / "tools/coverage.py")
cov = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cov)


class ResyncEvidenceTests(unittest.TestCase):
    def status(self, summary, log=None):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "regress-probe.out"
            path.write_text("=== regress-probe-shadows (fixture)\n" + summary, encoding="utf-8")
            if log is not None:
                path.with_name("regress-probe-shadows.log").write_text(log, encoding="utf-8")
            reg = cov.read_regress(path)
        reg["runs"]["replace"] = ("clean", 0)
        row = {"pattern": "Fixture", "spec": "-", "core": "-",
               "shadow": "netmgr_shadow", "replace": "-"}
        cov.map_statuses([row], [], ["netmgr_shadow"], {}, reg, {}, "probe")
        return row, reg

    def test_matching_after_reset_is_not_exact(self):
        row, reg = self.status("  netmgr 73 lines, 73 with every mismatch count 0\n"
                               "  netmgr resynced lines: 4\n")
        self.assertEqual(reg["shadows"]["netmgr"], (73, 73))
        self.assertFalse(row["verified"])
        self.assertIn("4 resynced lines", row["shadow_note"])

    def test_zero_resets_and_all_matching_are_exact(self):
        row, _ = self.status("  netmgr 73 lines, 73 with every mismatch count 0\n"
                             "  netmgr resynced lines: 0\n")
        self.assertTrue(row["verified"])

    def test_legacy_missing_reset_evidence_fails_closed(self):
        row, _ = self.status("  netmgr 73 lines, 73 with every mismatch count 0\n")
        self.assertFalse(row["verified"])
        self.assertIn("evidence missing", row["shadow_note"])

    def test_zero_metadata_cannot_erase_a_positive_reset(self):
        row, _ = self.status("  netmgr 73 lines, 73 with every mismatch count 0\n"
                             "  netmgr resynced lines: 4\n  netmgr resynced lines: 0\n")
        self.assertFalse(row["verified"])

    def test_old_summary_can_use_the_complete_recorded_log(self):
        summary = "  netmgr 1 lines, 1 with every mismatch count 0\n"
        line = "2026-10-21 [pid 1 tid 2] netmgr shadow (month) 2026-04-01: mismatches=0"
        clean, reg = self.status(summary, line + "; resyncs=0\n")
        self.assertTrue(clean["verified"])
        self.assertIn("resync_source", reg)
        reset, _ = self.status(summary, line + "; resynced: lot\n")
        self.assertFalse(reset["verified"])

    def test_truncated_legacy_log_cannot_prove_zero_resets(self):
        row, _ = self.status("  netmgr 2 lines, 2 with every mismatch count 0\n",
                             "2026-10-21 [pid 1 tid 2] netmgr shadow (month) 2026-04-01: mismatches=0\n")
        self.assertFalse(row["verified"])

    @unittest.skipUnless(shutil.which("powershell"), "requires Windows PowerShell")
    def test_actual_summary_carries_reset_evidence_into_coverage(self):
        with tempfile.TemporaryDirectory() as tmp:
            log = Path(tmp) / "game.log"
            log.write_text(
                "2026-10-21 [pid 1 tid 2] netmgr shadow (month) 2026-04-01: mismatches=0; resynced: lot\n"
                "2026-10-21 [pid 1 tid 2] netmgr shadow (month) 2026-05-01: mismatches=0; resyncs=0\n",
                encoding="utf-8")
            result = subprocess.run(["powershell", "-NoProfile", "-File",
                                     str(ROOT / "tools/run_summary.ps1"), "-Log", str(log)],
                                    capture_output=True, text=True, check=True)
        row, reg = self.status(result.stdout)
        self.assertEqual(reg["shadows"]["netmgr"], (2, 2))
        self.assertEqual(reg["resynced"]["netmgr"], 1)
        self.assertFalse(row["verified"])
