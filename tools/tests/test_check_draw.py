"""Draw evidence must participate in reference qualification, selection and exact credit."""
import contextlib
import csv
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import check
import golden
from test_drawcheck import capture


class DrawStoreTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.manifest = {'settings': {'draw_capture': True}}

    def run_dir(self, name, change=False):
        run = self.root / name
        run.mkdir()
        stream = capture()
        if change:
            stream[3]['args'][2] = 6
        (run / 'draw.jsonl').write_text(''.join(json.dumps(r) + '\n' for r in stream), encoding='utf-8')
        for name in ('verdict.json', 'summary.json'):
            golden.write_json(run / name, {})
        for name in ('game.log', 'harness.txt'):
            (run / name).write_text('evidence', encoding='utf-8')
        (run / 'reach.tsv').write_text('address\tentries\tfirst_window\tskipped\n00401000\t2\t0\t0\n',
                                     encoding='utf-8')
        return run

    def test_draw_repeats_and_perturb_audits_preserve_immutable_reference(self):
        original = self.run_dir('original')
        store, key = self.root / 'store', 'a' * 64
        self.assertEqual(golden.cache_record(store, key, self.manifest, original)[0], 'recorded')
        entry = store / key
        before = golden.sha256(entry / 'draw.jsonl')
        for perturb in ('', 'heap perturb = holes', 'heap perturb = reverse'):
            with patch.dict(golden.os.environ, {'GOLDEN_INI_EXTRA': perturb}):
                self.assertEqual(golden.cache_record(store, key, self.manifest, original)[0], 'reproduced')
        self.assertEqual(golden.qualification(entry), 'reproduced')
        self.assertEqual({golden.load_json(p)['ini_extra'] for p in entry.glob('repeat-*.json')},
                         {'', 'heap perturb = holes', 'heap perturb = reverse'})
        changed = self.run_dir('changed', change=True)
        status, report = golden.cache_record(store, key, self.manifest, changed)
        self.assertEqual(status, 'nondeterministic')
        self.assertEqual(report['draw']['difference']['field'], 'args[2]')
        self.assertEqual(golden.qualification(entry), 'nondeterministic')
        self.assertEqual(golden.sha256(entry / 'draw.jsonl'), before)
        self.assertIn('reach.tsv', golden.load_json(entry / 'baseline.json')['artifacts'])

    def test_draw_difference_cannot_use_save_envelope_and_names_first_call(self):
        original, changed = self.run_dir('original'), self.run_dir('changed', change=True)
        store, key = self.root / 'store', 'a' * 64
        golden.cache_record(store, key, self.manifest, original)
        with patch.object(golden.golden_envelope, 'build', side_effect=AssertionError('save envelope')):
            report = golden.compare_cached(store / key, changed, rule='envelope')
        self.assertEqual(report['verdict'], 'outside')
        self.assertFalse(golden.artifacts_exact(report))
        report['run'] = str(changed)
        detail = check.describe('draw-city', report, {}, store / key, True)
        self.assertIn('frame=0 call=0 method=DrawArrays field=args[2]', detail)
        self.assertIn('left=3 right=6', detail)

    def test_missing_or_truncated_draw_never_establishes_equality(self):
        original = self.run_dir('original')
        store, key = self.root / 'store', 'a' * 64
        golden.cache_record(store, key, self.manifest, original)
        changed = self.run_dir('changed')
        (changed / 'draw.jsonl').write_text('{}\n', encoding='utf-8')
        with self.assertRaises(ValueError):
            golden.compare_cached(store / key, changed)
        (store / key / 'draw.jsonl').unlink()
        with self.assertRaises(OSError):
            golden.compare_cached(store / key, original)

    def test_pin_store_is_independent_of_run_output(self):
        dll = self.root / 'reference.dll'
        dll.write_bytes(b'reference')
        staged = golden.stage_dll(self.root, dll)
        store = self.root / 'golden'
        store.mkdir()
        golden.write_json(store / golden.PIN, {'dll_sha256': golden.sha256(dll)})
        _, reference = golden.pinned(self.root / 'other-output', store)
        self.assertEqual(reference, staged)

    def test_candidate_draw_without_counters_compares_but_cannot_be_a_reference(self):
        dll = self.root / 'candidate.dll'
        dll.write_bytes(b'candidate')
        config = self.root / 'engine/plugin/FC4Config.cpp'
        config.parent.mkdir(parents=True)
        config.write_text('', encoding='utf-8')
        args = golden.parser().parse_args(['compare', '--city', 'City', '--until', '2000-01-01',
            '--schedule', 'fixed', '--draw-capture', '--draw-frames', '1'])
        args.output, args.harness_root, args.clock = self.root / 'output', self.root, 'fixed'

        def queued(args, dll, ini, result, work):
            result.mkdir()
            base = capture()
            stream = [dict(base[0], frames=10, scenarios=10)]
            for i in range(10):
                for row in json.loads(json.dumps(base[1:-1])):
                    row['frame'] = i
                    if row['type'] == 'frame':
                        row.update(scenario=i, zoom=i // 2 + 1, rotation=i % 2)
                    stream.append(row)
            stream.append(dict(base[-1], frames=10, calls=10, draw_calls=10, ui_frames=10))
            (result / 'draw.jsonl').write_text(''.join(json.dumps(r) + '\n' for r in stream), encoding='utf-8')
            (result / 'game.log').write_text("subsystem 'simulator': enabled\nAUTOTEST VERDICT: clean\n"
                'sim_fixed_step ON: 64 agent deliveries/frame\nschedule: fixed (code fixture, test)\n',
                encoding='utf-8')
            golden.write_json(result / 'run.json', {'label': 'candidate'})

        with patch.object(golden, 'validate_harness_sources'), patch.object(golden, 'reach_supported', return_value=True), \
                patch.object(golden, 'subsystem_keys', return_value=['simulator']), \
                patch.object(golden, 'schedule_code', return_value='fixture'), patch.object(golden, 'run_queued', side_effect=queued):
            result = golden.run_game(args, '', [], dll, 'compare')
            self.assertTrue(golden.load_json(result / 'verdict.json')['draw_complete'])
            self.assertFalse((result / 'reach.tsv').exists())
            with self.assertRaisesRegex(ValueError, 'reach.tsv'):
                golden.cache_record(self.root / 'store', 'a' * 64, self.manifest, result)
            with self.assertRaisesRegex(ValueError, 'missing draw reach counters'):
                golden.run_game(args, '', [], dll, 'original')

    def test_exact_candidate_draw_without_counters_withholds_coverage(self):
        reference, candidate = self.run_dir('reference'), self.run_dir('candidate')
        (candidate / 'reach.tsv').unlink()
        path = self.root / 'credit.json'
        golden.write_json(path, {'keys': ['ports'], 'dll_sha256': 'candidate', 'scenarios': {
            'draw-city': {'kind': 'draw', 'verdict': 'exact', 'reference': reference.name,
                          'run': str(candidate), 'detail': 'exact'}}})
        spec = check.importlib.util.spec_from_file_location('fc4_test_coverage', check.REPO / 'tools/coverage.py')
        coverage = check.importlib.util.module_from_spec(spec)
        spec.loader.exec_module(coverage)
        evidence = coverage.credit_evidence(path, lambda _: ({0x401000}, set()))
        self.assertEqual(evidence['missing'], ['draw-city (run)'])


class DrawSelectionTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.addCleanup(patch.stopall)
        patch.object(check, 'STORE', self.root).start()
        self.rows = {
            'save-city': dict(name='save-city', kind='save', script='-', sets='full'),
            'draw-city': dict(name='draw-city', kind='draw', script='-', sets='draw,full'),
            'draw-ui': dict(name='draw-ui', kind='draw', script='-', sets='draw,ui'),
        }
        self.references = {'draw-city': {'reference': 'a' * 64, 'keys': ['span565', 'gzfw']},
                           'draw-ui': {'reference': 'b' * 64, 'keys': ['gzfw']}}
        patch.object(check, 'scenarios', return_value=self.rows).start()
        patch.object(check, 'draw_references', return_value=self.references).start()
        patch.object(check, 'record_owners', return_value={}).start()
        patch.object(check, 'owner_covers', return_value={'span565': ['save-city'], 'gzfw': ['save-city']}).start()
        self.dll = self.root / 'candidate.dll'
        self.dll.write_bytes(b'candidate')

    def test_run_adds_only_draws_reaching_switched_key_even_for_explicit_save(self):
        calls = []
        def compare(row, dll, keys, options=()):
            calls.append(row['name'])
            return {'verdict': 'exact'}
        with patch.object(check, 'compare', side_effect=compare), contextlib.redirect_stdout(io.StringIO()):
            code = check.main(['run', '--store', str(self.root), '--dll', str(self.dll),
                               '--replace', 'span565', '--scenarios', 'save-city', '--jobs', '1'])
        self.assertEqual(code, 0)
        self.assertEqual(set(calls), {'save-city', 'draw-city'})
        self.assertEqual(check.draw_names(['unknown'], {'draw-missing': {'keys': None}}), {'draw-missing'})

    def test_draw_selection_includes_required_owners_for_run_and_credit(self):
        references = {'draw-network': {'keys': ['netmgr']},
                      'draw-unrelated': {'keys': ['span565']}}
        self.assertEqual(check.draw_names(['netrules'], references), {'draw-network'})
        self.assertEqual(check.draw_names(['mysims'], {'draw-advisors': {'keys': ['advisors']}}),
                         {'draw-advisors'})

    def test_each_draw_failure_changes_owner_verdict_and_preserves_save_table(self):
        (self.root / 'owner-check.tsv').write_text('save-only evidence', encoding='utf-8')
        def compare(row, dll, keys, options=()):
            return {'verdict': 'error', 'error': 'incomplete draw'}
        with patch.object(check, 'compare', side_effect=compare), contextlib.redirect_stdout(io.StringIO()):
            code = check.main(['run', '--store', str(self.root), '--dll', str(self.dll), '--replace', 'each',
                               '--keys', 'span565', '--scenarios', 'draw'])
        self.assertEqual(code, 2)
        with (self.root / 'draw-owner-check.tsv').open() as stream:
            rows = list(csv.DictReader(list(stream)[1:], delimiter='\t'))
        self.assertEqual([(r['key'], r['scenario'], r['verdict']) for r in rows],
                         [('span565', 'draw-city', 'error')])
        self.assertEqual((self.root / 'owner-check.tsv').read_text(), 'save-only evidence')

    def test_credit_includes_draw_failure_as_scenario(self):
        def compare(row, dll, keys, options=()):
            return {'verdict': 'error', 'error': 'draw differs', 'key': 'a' * 64} if row['kind'] == 'draw' else {
                'verdict': 'exact', 'key': 'c' * 64}
        with patch.object(check, 'compare', side_effect=compare), contextlib.redirect_stdout(io.StringIO()):
            code = check.main(['credit', '--store', str(self.root), '--dll', str(self.dll), '--replace', 'span565'])
        self.assertEqual(code, 2)
        scenarios = golden.load_json(self.root / 'credit.json')['scenarios']
        self.assertEqual(set(scenarios), {'save-city', 'draw-city'})
        self.assertEqual(scenarios['draw-city']['kind'], 'draw')
        self.assertEqual(scenarios['draw-city']['verdict'], 'error')

    def test_missing_draw_reference_withholds_coverage_measurement(self):
        path = self.root / 'credit.json'
        golden.write_json(path, {'keys': ['span565'], 'dll_sha256': 'candidate', 'scenarios': {
            'draw-city': {'kind': 'draw', 'verdict': 'error', 'reference': None, 'run': None,
                          'detail': 'ERROR no golden baseline for this key; run record first'},
            'save-unrecorded': {'kind': 'save', 'verdict': 'error', 'reference': None, 'run': None,
                                'detail': 'ERROR no golden baseline for this key; run record first'}}})
        spec = check.importlib.util.spec_from_file_location('fc4_test_coverage', check.REPO / 'tools/coverage.py')
        coverage = check.importlib.util.module_from_spec(spec)
        spec.loader.exec_module(coverage)
        evidence = coverage.credit_evidence(path, lambda _: ({}, {}))
        self.assertEqual(evidence['missing'], ['draw-city (reference)'])
        self.assertEqual(evidence['unrecorded'], ['save-unrecorded'])

    def test_old_save_only_owner_table_cannot_supply_exact_owners(self):
        path = self.root / 'owner-check.tsv'
        header = f'# candidate sha256 {check.sha256(self.dll)}\nkey\tscenario\tverdict\treference\n'
        path.write_text(header + 'span565\tsave-city\texact\tc\n', encoding='utf-8')
        self.assertEqual(check.exact_owners(self.dll), [])
        path.write_text(header + 'span565\tsave-city\texact\tc\nspan565\tdraw-city\texact\t' + 'a' * 64 + '\n',
                        encoding='utf-8')
        self.assertEqual(check.exact_owners(self.dll), ['span565'])
        self.references['draw-city']['reference'] = 'd' * 64
        self.assertEqual(check.exact_owners(self.dll), [])


if __name__ == '__main__':
    unittest.main()
