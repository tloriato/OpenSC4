"""Native credit joins independent function proofs and whole-game checks on a DLL's exported owners."""
import csv
import hashlib
import importlib.util
import json
from pathlib import Path
import struct
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("coverage_native", ROOT / "tools/coverage.py")
cov = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cov)


def fake_dll(ports, version=1, entry_size=16, magic=b"FC4P", export_name=b"fc4native_ports"):
    """Small PE32: raw section offsets differ from RVAs, and entries live apart from the table header."""
    raw = bytearray(0x1200)
    raw[:2] = b"MZ"
    struct.pack_into("<I", raw, 0x3c, 0x80)
    raw[0x80:0x84] = b"PE\0\0"
    struct.pack_into("<HH", raw, 0x84, 0x14c, 1)
    struct.pack_into("<H", raw, 0x94, 224)
    opt, base = 0x98, 0x10000000
    struct.pack_into("<H", raw, opt, 0x10b)
    struct.pack_into("<I", raw, opt + 28, base)
    struct.pack_into("<I", raw, opt + 60, 0x200)
    struct.pack_into("<II", raw, opt + 96, 0x1000, 0x80)
    struct.pack_into("<IIII", raw, opt + 224 + 8, 0x1000, 0x1000, 0x1000, 0x200)

    def put(rva, fmt, *values):
        struct.pack_into(fmt, raw, rva - 0xe00, *values)

    def string(rva, value):
        data = value + b"\0"
        start = rva - 0xe00
        raw[start:start + len(data)] = data

    put(0x1014, "<IIIII", 1, 1, 0x1050, 0x1054, 0x1058)
    put(0x1050, "<IIH", 0x1100, 0x1060, 0)
    string(0x1060, export_name)
    put(0x1100, "<4sIIII", magic, version, len(ports), entry_size, base + 0x1200)
    for i, (address, owner) in enumerate(ports):
        owner_rva = 0x1400 + 0x80 * i
        put(0x1200 + 16 * i, "<IIII", address, base + 0x1f00, base + owner_rva, base + owner_rva + 0x40)
        string(owner_rva, owner.encode())
        string(owner_rva + 0x40, b"FixtureFunction")
    return bytes(raw)


class NativeCoverageTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.dll_dir = self.root / "dlls"
        self.dll_dir.mkdir()
        self.sha = self.dll([(0x401000, "nativeowner"), (0x402000, "nativeowner"), (0x403000, "other")])
        self.proven, self.checks = self.root / "proven.tsv", self.root / "native-checks.tsv"
        self.functions = {0x401000: 17, 0x402000: 31, 0x403000: 29, 0x404000: 41}

    def dll(self, ports, **kwargs):
        raw = fake_dll(ports, **kwargs)
        sha = hashlib.sha256(raw).hexdigest()
        (self.dll_dir / f"{sha}.dll").write_bytes(raw)
        return sha

    def proof(self, address="00401000"):
        return dict(address=address, name="Fixture::Function", owner="unrelated proof class", verdict="EXACT",
                    captures="3", variants="10", skipped="2", scenarios="a,b,c", dll_sha256=self.sha)

    def check(self, sha=None, owner="nativeowner", scenarios="62", identical="62", checked="2026-10-29"):
        return dict(owner=owner, dll_sha256=sha or self.sha, scenarios=scenarios, identical=identical,
                    checked=checked, notes="fixture")

    def measure(self, proofs, checks):
        for path, rows, fields in (
                (self.proven, proofs, "address name owner verdict captures variants skipped scenarios dll_sha256"),
                (self.checks, checks, "owner dll_sha256 scenarios identical checked notes")):
            with path.open("w", encoding="utf-8", newline="") as stream:
                writer = csv.DictWriter(stream, fieldnames=fields.split(), delimiter="\t")
                writer.writeheader()
                writer.writerows(rows)
        return cov.native_evidence(self.proven, self.checks, self.dll_dir, self.functions)

    def test_credit_uses_dll_owner_and_denominator_bytes(self):
        result = self.measure([self.proof(), self.proof("00402000")], [self.check()])
        self.assertEqual(result["credited"], {0x401000, 0x402000})
        row = result["owners"][("nativeowner", self.sha)]
        self.assertEqual((len(row["exact"]), row["ports"], row["bytes"]), (2, 2, 48))

    def test_exact_needs_three_distinct_nonempty_scenarios(self):
        for verdict, scenarios in (("DIFFERENT", "a,b,c"), ("EXACT", "a,b"), ("EXACT", "a,a,b"),
                                   ("EXACT", "a,b, ,"), ("EXACT", "")):
            with self.subTest(verdict=verdict, scenarios=scenarios):
                row = self.proof()
                row.update(verdict=verdict, scenarios=scenarios)
                self.assertEqual(self.measure([row], [self.check()])["credited"], set())

    def test_exact_needs_matching_dll_and_port_owner_and_complete_check(self):
        another = self.dll([(0x401000, "nativeowner")])
        for checks in ([], [self.check(sha=another)], [self.check(owner="unrelated proof class")],
                       [self.check(identical="61")], [self.check(scenarios="0", identical="0")]):
            with self.subTest(checks=checks):
                self.assertEqual(self.measure([self.proof()], checks)["credited"], set())

    def test_owner_check_does_not_credit_unproven_ports(self):
        result = self.measure([self.proof()], [self.check(), self.check(owner="other")])
        self.assertEqual(result["credited"], {0x401000})
        self.assertEqual(result["owners"][("other", self.sha)]["bytes"], 0)

    def test_each_owner_needs_its_own_check_on_the_dll(self):
        proofs = [self.proof(), self.proof("00403000")]
        result = self.measure(proofs, [self.check()])
        self.assertEqual(result["credited"], {0x401000})
        result = self.measure(proofs, [self.check(), self.check(owner="other")])
        self.assertEqual(result["credited"], {0x401000, 0x403000})
        self.assertEqual(result["owners"][("other", self.sha)]["bytes"], 29)

    def test_invalid_check_counts_fail(self):
        for n, identical in (("2", "3"), ("-1", "-1"), ("2", "-1")):
            with self.subTest(n=n, identical=identical), self.assertRaises(ValueError):
                self.measure([self.proof()], [self.check(scenarios=n, identical=identical)])

    def test_function_absent_from_ports_or_denominator_earns_nothing(self):
        self.assertEqual(self.measure([self.proof("00404000")], [self.check()])["credited"], set())
        del self.functions[0x401000]
        self.assertEqual(self.measure([self.proof()], [self.check()])["credited"], set())

    def test_checks_are_individual_runs_not_summed(self):
        result = self.measure([self.proof()], [self.check(scenarios="2", identical="1"),
                                              self.check(scenarios="1", identical="1", checked="2026-10-28")])
        self.assertEqual(result["credited"], {0x401000})
        self.assertEqual(result["owners"][("nativeowner", self.sha)]["check"]["scenarios"], "1")

    def test_duplicate_proofs_fail_instead_of_double_counting(self):
        with self.assertRaisesRegex(ValueError, "duplicate proven"):
            self.measure([self.proof(), self.proof()], [self.check()])

    def test_dll_hash_and_existence_are_required(self):
        path = self.dll_dir / f"{self.sha}.dll"
        path.write_bytes(path.read_bytes() + b"changed")
        with self.assertRaisesRegex(ValueError, "sha256 mismatch"):
            self.measure([self.proof()], [self.check()])
        path.unlink()
        with self.assertRaises(FileNotFoundError):
            self.measure([self.proof()], [self.check()])

    def test_bad_export_or_table_fails_closed(self):
        for kwargs in (dict(version=2), dict(entry_size=20), dict(magic=b"BAD!"), dict(export_name=b"other")):
            with self.subTest(kwargs=kwargs):
                sha = self.dll([(0x401000, "nativeowner")], **kwargs)
                proof = self.proof()
                proof["dll_sha256"] = sha
                with self.assertRaises(ValueError):
                    self.measure([proof], [self.check(sha=sha)])

    def test_reference_manifest_verdicts_do_not_affect_reach(self):
        reference = self.root / "reference"
        reference.mkdir()
        trace = reference / "reach.tsv"
        trace.write_text("fixture", encoding="utf-8")
        manifest = self.root / "references.json"
        manifest.write_text(json.dumps({"scenarios": {"city": {"reference": "reference", "verdict": "different"}}}))
        evidence = cov.reference_evidence(manifest, lambda path: ({0x401000}, {}))
        self.assertEqual(set(evidence["by"]), {0x401000})
        self.assertEqual((evidence["n"], evidence["missing"], evidence["files"]), (1, [], [trace]))

    def test_historical_reference_selection_uses_pinned_dll(self):
        for name, sha, address in (("old-a", self.sha, 0x401000), ("old-b", self.sha, 0x402000),
                                   ("new", "f" * 64, 0x403000)):
            directory = self.root / name
            directory.mkdir()
            (directory / "baseline.json").write_text(json.dumps({"inputs": {"files": {"reference.dll": {"sha256": sha}}}}))
            (directory / "reach.tsv").write_text(str(address))
        result = cov.stored_reference_evidence(self.root, self.sha, lambda path: ({int(path.read_text())}, {}))
        self.assertEqual(set(result["by"]), {0x401000, 0x402000})
        self.assertEqual((result["n"], result["missing"]), (2, []))


if __name__ == "__main__":
    unittest.main()
