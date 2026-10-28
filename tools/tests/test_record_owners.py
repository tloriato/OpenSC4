"""Every reference-save record should give a lane a useful ownership lead."""
import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import check
import gen_record_owners as generator
import golden
import golden_dbpf
from test_golden import package


class RecordOwnersTests(unittest.TestCase):
    def test_generated_table_is_current(self):
        self.assertEqual(generator.OUTPUT.read_text(encoding='utf-8'), generator.generate())

    def test_registry_wins_and_fallbacks_name_the_example_records(self):
        owners = check.record_owners()
        for clsid, answer in check.registry_record_owners().items():
            self.assertEqual(owners[clsid], answer)
        expected = {
            0x096E75F5: ('automata?', 'cSC4Watercraft'),
            0x2977AA47: ('occupants and props?', 'cSC4PropOccupant'),
            0x499B23FE: ('class-store index', 'class-store index'),
            0x4A1628C5: ('automata?', 'cSC4AutomataGenerator'),
            0x4A222B98: ('automata?', 'cSC4PatrolCar'),
            0x4C0DD224: ('automata?', 'cSC4MayorLimo'),
        }
        for clsid, answer in expected.items():
            self.assertEqual(owners[clsid], answer)

    def test_first_coverage_pattern_wins_and_ambiguous_keys_are_not_guessed(self):
        rows = [dict(pattern='cSC4Specific', subsystem='group', replace='one'),
                dict(pattern='cSC4*', subsystem='group', replace='two')]
        self.assertEqual(generator.inferred_owner('cSC4Specific', rows), 'one?')
        rows.insert(0, dict(pattern='cSC4Other', subsystem='group', replace='-'))
        self.assertEqual(generator.inferred_owner('cSC4Other', rows), 'group?')
        rows.pop()
        self.assertEqual(generator.inferred_owner('cSC4Other', rows), 'one?')
        self.assertIsNone(generator.inferred_owner('unrecognized', rows))

    def test_inferred_differences_cannot_be_credited_exact_or_unobserved(self):
        with tempfile.TemporaryDirectory() as directory:
            tgi = '4C0DD224/299B2D1B/0000000000000000'
            Path(directory, 'monthly.json').write_text(json.dumps({
                'months': {'2112-06-30': {'records': {tgi: 'hash'}}}}), encoding='utf-8')
            report = {'run': directory, 'monthly': {'complete': True, 'differences': [
                {'date': '2112-06-30', 'records': [{'tgi': tgi}]}]}}
            check.attribute_owners(report, ['automata'], check.record_owners())
            self.assertEqual(report['owners']['automata']['verdict'], 'different')
            self.assertEqual(report['monthly']['differences'][0]['records'][0]['owner'], 'automata?')
            report['monthly']['differences'] = []
            check.attribute_owners(report, ['automata'], check.record_owners())
            self.assertEqual(report['owners']['automata']['verdict'], 'exact')

    def test_inventory_reads_compressed_keys_without_decoding_payload(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory, 'end.sc4')
            tgi = (0x4C0DD224, 0x299B2D1B, 0)
            compressed = b'\x10\xfb\x00\x00\x04\xe0abcd\xfc'
            package(path, [((golden_dbpf.DIR, 0, 0), struct.pack('<IIII', *tgi, 4)),
                           (tgi, compressed)])
            self.assertEqual(golden_dbpf.read_records(path, decompress=False), {tgi: compressed})
            self.assertEqual(golden_dbpf.read_records(path), {tgi: b'abcd'})

    def test_every_type_in_current_reference_end_saves_resolves(self):
        store = golden.main_checkout(check.REPO) / 'build-runs/golden'
        saves = sorted(store.glob('*/end.sc4'))
        if not saves:
            self.skipTest(f'local reference store not present: {store}')
        types = set()
        for path in saves:
            # DBPF index and DIR are checked; type coverage does not require QFS payloads.
            types.update(t for t, _, _ in golden_dbpf.read_records(path, decompress=False))
        owners = check.record_owners()
        missing = {f'{t:08X}' for t in types
                   if t not in owners or owners[t][0] in ('', '-', 'unowned') or owners[t][1] == '-'}
        self.assertFalse(missing, f'unresolved record types: {sorted(missing)}')


if __name__ == '__main__':
    unittest.main()
