"""Entry evidence, byte accounting and overlap in marginal corpus coverage."""
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import reach_gain


class ReachGainTests(unittest.TestCase):
    def test_baseline_only_and_remaining_gaps_after_overlapping_candidates(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            ref, candidate = root / 'ref.tsv', root / 'candidate.tsv'
            ref.write_text('address\tentries\n00400000\t1\n', encoding='utf-8')
            candidate.write_text('address\tentries\n00400010\t2\n', encoding='utf-8')
            functions = {0x400000: dict(bytes=10, owner='City', subsystem='save'),
                         0x400010: dict(bytes=20, owner='Panel', subsystem='UI'),
                         0x400020: dict(bytes=30, owner='Panel', subsystem='UI')}
            baseline = reach_gain.measure([], [ref], functions)
            self.assertEqual(baseline['gain'], dict(functions=0, bytes=0))
            self.assertEqual(baseline['gaps'], {'UI': dict(functions=2, bytes=50)})
            measured = reach_gain.measure([candidate, candidate], [ref], functions)
            self.assertEqual(measured['baseline'], baseline['baseline'])
            self.assertEqual(measured['gaps'], {'UI': dict(functions=1, bytes=30)})
            self.assertEqual(measured['combined']['bytes'] + sum(g['bytes'] for g in measured['gaps'].values()),
                             measured['denominator']['bytes'])

    def test_reference_union_positive_entries_libraries_and_overlap(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            owners = root / 'owners.tsv'
            owners.write_text('address\tbytes\towner\tsubsystem\n'
                              '00400000\t10\tCity\tsave\n'
                              '00400010\t20\tTool\ttools\n'
                              '00400020\t30\tPanel\tUI\n'
                              '00400030\t99\tlib:crt\tlibrary crt\n', encoding='utf-8')

            def trace(name, body):
                path = root / name
                path.write_text('# lower bounds\naddress\tentries\tskipped\n' + body, encoding='utf-8')
                return path

            a = trace('a.tsv', '00400000\t1\t\n00400010\t0\t\n00400020\t3\tunsafe\n')
            b = trace('b.tsv', '00400010\t2\t\n')
            c = trace('c.tsv', '00400020\t2\t\n00400030\t2\t\n00500000\t8\t\n')
            report = reach_gain.measure([c, c], [a, b], reach_gain.inventory(owners))
            self.assertEqual(report['baseline'], dict(functions=2, bytes=30))
            self.assertEqual(report['denominator'], dict(functions=3, bytes=60))
            self.assertEqual(report['gain'], dict(functions=1, bytes=30))
            self.assertEqual([r['bytes'] for r in report['candidates']], [30, 30])
            self.assertEqual(report['candidates'][0]['subsystems']['UI']['entries'][0]['owner'], 'Panel')
            json.dumps(report)

    def test_truncated_reference_retains_complete_rows_and_reports_warning(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'reach.tsv'
            path.write_text('address\tentries\n00400000\t2\n008f', encoding='utf-8')
            warnings = []
            self.assertEqual(reach_gain.entered(path, warnings), {0x400000})
            self.assertEqual(len(warnings), 1)
            self.assertIn('lower bound', warnings[0])

    def test_empty_and_duplicate_inventory_refused(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'owners.tsv'
            path.write_text('address\tbytes\towner\tsubsystem\n', encoding='utf-8')
            with self.assertRaisesRegex(ValueError, 'empty inventory'):
                reach_gain.inventory(path)
            path.write_text('address\tbytes\towner\tsubsystem\n'
                            '00400000\t10\tCity\tsave\n' * 2, encoding='utf-8')
            with self.assertRaises(ValueError):
                reach_gain.inventory(path)
