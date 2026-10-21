"""Exercise source unions through real, successive Git conflicts."""
import importlib.util
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / ".claude" / "lane-bin"))
spec = importlib.util.spec_from_file_location("train_union_tests", REPO / "tools/merge-train.py")
mt = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mt)


class SourceUnionTests(unittest.TestCase):
    def git(self, root, *args, check=True):
        return subprocess.run(["git", "-C", str(root), *args], check=check,
                              capture_output=True, text=True)

    def setup_branches(self, root, rel, base, variants):
        self.git(root, "init", "-q")
        self.git(root, "config", "user.name", "Union test")
        self.git(root, "config", "user.email", "union@example.invalid")
        p = root / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(base, encoding="utf-8")
        self.git(root, "add", ".")
        self.git(root, "commit", "-qm", "base")
        base_sha = self.git(root, "rev-parse", "HEAD").stdout.strip()
        for name, body in variants:
            self.git(root, "checkout", "-qb", name, base_sha)
            p.write_text(body, encoding="utf-8")
            self.git(root, "add", rel)
            self.git(root, "commit", "-qm", name)
        self.git(root, "checkout", "-q", variants[0][0])
        return p

    def merge_union(self, root, rel, branch):
        merged = self.git(root, "merge", "--no-ff", "--no-commit", branch, check=False)
        self.assertEqual(merged.returncode, 1, merged.stdout + merged.stderr)
        self.assertIn("<" * 7, (root / rel).read_text(encoding="utf-8"))
        self.assertTrue(mt.union_hunks(root, rel))
        self.git(root, "commit", "-qm", "resolved")

    def test_three_core_lanes_keep_one_module_anchor_and_compile(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            rel = "engine/core/src/lib.rs"
            src = root / "engine/core/src"
            src.mkdir(parents=True)
            for name in ["simgrid", "stl", "store", "alpha", "beta", "gamma"]:
                (src / (name + ".rs")).write_text("", encoding="utf-8")
            base = "pub mod simgrid;\npub mod stl;\npub mod store;\n"
            variants = [(name, base.replace("pub mod stl;", f"pub mod {name};\npub mod stl;"))
                        for name in ["alpha", "beta", "gamma"]]
            p = self.setup_branches(root, rel, base, variants)
            self.merge_union(root, rel, "beta")
            self.merge_union(root, rel, "gamma")
            text = p.read_text(encoding="utf-8")
            self.assertEqual(text.count("pub mod stl;"), 1)
            self.assertEqual(len(text.splitlines()), 6)
            self.assertTrue(all(line.startswith("pub mod ") and line.count(";") == 1
                                for line in text.splitlines()))
            if rustc := shutil.which("rustc"):
                compiled = subprocess.run([rustc, "--crate-type", "lib", "--crate-name", "union_fixture",
                                           "--emit=metadata", str(p), "-o", str(root / "out.rmeta")],
                                          capture_output=True, text=True)
                self.assertEqual(compiled.returncode, 0, compiled.stderr)

    def test_array_union_preserves_repeated_entries_and_common_close(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            rel = "engine/plugin/FC4Config.cpp"
            base = "constexpr int values[] = {\n  0,\n};\nstatic_assert(sizeof(values) > 0);\n"
            p = self.setup_branches(root, rel, base, [
                ("ours", base.replace("};", "  1,\n  1,\n};")),
                ("theirs", base.replace("};", "  2,\n};")),
            ])
            self.merge_union(root, rel, "theirs")
            text = p.read_text(encoding="utf-8")
            self.assertEqual(text.count("  1,"), 2)
            self.assertIn("  2,\n};\nstatic_assert", text)


if __name__ == "__main__":
    unittest.main()
