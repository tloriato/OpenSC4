"""Canonical coverage subsystem names must remain intact in lane overlap checks."""
import importlib.util
import sys
import unittest
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


if __name__ == "__main__":
    unittest.main()
