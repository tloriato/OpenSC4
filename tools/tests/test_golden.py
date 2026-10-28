"""Golden cache invariants, full-TGI/QFS comparison and monthly diagnostics."""
import hashlib
import contextlib
import io
import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import golden
import golden_dbpf as records
import golden_monthly


def package(path, entries, *, width=20, time=0):
    head = bytearray(96)
    head[:4] = b'DBPF'
    struct.pack_into('<II', head, 4, 1, 0)
    struct.pack_into('<I', head, 24, time)
    index, payload = bytearray(), bytearray()
    for (t, g, i), data in entries:
        words = [t, g, i & 0xFFFFFFFF]
        if width == 24:
            words.append(i >> 32)
        words += [96 + len(payload), len(data)]
        index.extend(struct.pack('<' + 'I' * len(words), *words))
        payload.extend(data)
    struct.pack_into('<IIII', head, 0x20, 7, len(entries), 96 + len(payload), len(index))
    struct.pack_into('<I', head, 0x3C, int(width == 24))
    path.write_bytes(head + payload + index)


class GoldenTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)

    def test_compression_layout_and_save_timestamp_are_not_gameplay(self):
        key = (1, 2, 3)
        original, changed = self.root / 'a.sc4', self.root / 'b.sc4'
        package(original, [(key, b'abcd')])
        qfs = b'\x10\xfb\x00\x00\x04\xe0abcd\xfc'
        package(changed, [((records.DIR, 0, 0), struct.pack('<IIII', *key, 4)),
                          (key, qfs)], time=1234)
        diff = records.compare_saves(original, changed)
        self.assertEqual(diff['exact_records'], 1)
        self.assertEqual(diff['different_records'], 0)
        self.assertEqual(diff['ignored_gameplay_fields'], [])

    def test_qfs_backreferences_and_size_bounds(self):
        # Four literals, then distance 1 count 3, then a terminal marker.
        qfs = b'\x10\xfb\x00\x00\x07\xe0aaaa\x00\x00\xfc'
        self.assertEqual(records._qfs(qfs, 7), b'a' * 7)
        self.assertEqual(records._qfs(struct.pack('<I', len(qfs)) + qfs, 7), b'a' * 7)
        for data, size in ((qfs, 8), (qfs[:-1], 7),
                           (b'\x10\xfb\x00\x00\x03\x00\x00\xfc', 3)):
            with self.assertRaises(ValueError):
                records._qfs(data, size)

    def test_64_bit_instances_are_not_aliased(self):
        path = self.root / 'high.sc4'
        entries = [((1, 2, 3), b'one'), ((1, 2, 0x100000003), b'two')]
        package(path, entries, width=24)
        self.assertEqual(records.read_records(path), dict(entries))

    def test_dir_stays_16_bytes_with_24_byte_index(self):
        path = self.root / 'compressed71.sc4'
        qfs = b'\x10\xfb\x00\x00\x04\xe0abcd\xfc'
        key = (1, 2, 0x100000003)
        package(path, [((records.DIR, 0, 0), struct.pack('<IIII', 1, 2, 3, 4)),
                       (key, qfs)], width=24)
        self.assertEqual(records.read_records(path), {key: b'abcd'})
        package(path, [((records.DIR, 0, 0), struct.pack('<IIII', 1, 2, 3, 4)),
                       (key, qfs), ((1, 2, 3), b'collision')], width=24)
        with self.assertRaisesRegex(ValueError, 'unique full TGI'):
            records.read_records(path)

    def test_added_removed_byte_and_decoded_fields(self):
        a, b = self.root / 'a.sc4', self.root / 'b.sc4'
        text_key = (0x2026960B, 0, 0)
        package(a, [(text_key, struct.pack('<I', 0x10000001) + b'A\0'), ((1, 0, 0), b'gone')])
        package(b, [(text_key, struct.pack('<I', 0x10000001) + b'B\0'), ((2, 0, 0), b'new')])
        diff = records.compare_saves(a, b)
        self.assertEqual(diff['different_records'], 3)
        self.assertEqual(diff['records'][-1]['fields']['text'], {'expected': 'A', 'actual': 'B'})
        self.assertEqual(diff['records'][-1]['first_byte'], 4)

    def test_bad_index_duplicate_and_truncation_fail(self):
        path = self.root / 'broken.sc4'
        for entries in ([((1, 0, 0), b'a'), ((1, 0, 0), b'b')],):
            package(path, entries)
            with self.assertRaises(ValueError):
                records.read_records(path)
        package(path, [((1, 0, 0), b'abc')])
        path.write_bytes(path.read_bytes()[:-1])
        with self.assertRaises(ValueError):
            records.read_records(path)

    def test_digest_first_month_fields_missing_and_prefix(self):
        a = golden.parse_digests('2026 x DIGEST julian=1 (2026-02-01) funds=10 R/C/I=2/3/4\n'
                                 '2026 x DIGEST julian=2 (2026-03-01) funds=20')
        b = golden.parse_digests('other pid DIGEST julian=1 (2026-02-01) funds=10 R/C/I=2/3/4\n'
                                 'other pid DIGEST julian=2 (2026-03-01) funds=19')
        self.assertIsNone(golden.compare_digests(a[:1], b[:1]))
        diff = golden.compare_digests(a, b)
        self.assertEqual(diff['date'], '2026-03-01')
        self.assertEqual(diff['fields']['funds'], {'expected': '20', 'actual': '19'})
        self.assertEqual(golden.compare_digests(a, b[:1])['date'], '2026-03-01')
        self.assertIsNone(golden.compare_digests([], []))

    def test_fingerprint_uses_bytes_and_all_settings(self):
        path = self.root / 'input'
        path.write_bytes(b'one')
        first, manifest = golden.fingerprint({'save': path}, {'until': '2026-03-01'})
        self.assertEqual(first, golden.fingerprint({'save': path}, {'until': '2026-03-01'})[0])
        self.assertNotEqual(first, golden.fingerprint({'save': path}, {'until': '2026-04-01'})[0])
        path.write_bytes(b'two')
        self.assertNotEqual(first, golden.fingerprint({'save': path}, manifest['settings'])[0])

    def make_run(self, name, payload=b'original'):
        run = self.root / name
        run.mkdir()
        package(run / 'end.sc4', [((1, 0, 0), payload)])
        golden.write_json(run / 'digests.json', [{'date': '2026-03-01', 'text': payload.decode(), 'fields': {}}])
        for file in ('verdict.json', 'summary.json'):
            golden.write_json(run / file, {})
        for file in ('game.log', 'harness.txt'):
            (run / file).write_text('evidence', encoding='utf-8')
        return run

    def test_pinned_candidate_dll_reports_the_bytes_actually_launched(self):
        source = self.root / 'build.dll'
        source.write_bytes(b'first build')
        first = golden.stage_dll(self.root, source)
        source.write_bytes(b'second build')
        second = golden.stage_dll(self.root, source)
        self.assertNotEqual(first, second)
        self.assertEqual(first.read_bytes(), b'first build')
        self.assertEqual(first.stem, golden.sha256(first))
        second.write_bytes(b'corrupt')
        with self.assertRaisesRegex(ValueError, 'corrupt staged DLL'):
            golden.stage_dll(self.root, source)
        self.assertEqual(second.read_bytes(), b'corrupt')

    def test_build_race_cannot_publish_a_dll_under_the_wrong_hash(self):
        source = self.root / 'build.dll'
        source.write_bytes(b'initial build')
        with patch.object(golden.shutil, 'copyfile',
                          side_effect=lambda src, dst: Path(dst).write_bytes(b'changed during copy')):
            with self.assertRaisesRegex(ValueError, 'DLL changed while staging'):
                golden.stage_dll(self.root, source)
        self.assertEqual(list((self.root / 'golden-inputs').iterdir()), [])

    def test_cache_never_overwrites_a_different_result(self):
        run = self.make_run('first')
        key = 'f' * 64
        manifest = {'settings': {'until': '2026-03-01'}}
        cache = self.root / 'cache'
        self.assertEqual(golden.cache_record(cache, key, manifest, run)[0], 'recorded')
        self.assertEqual(golden.qualification(cache / key), 'unverified')
        before = golden.sha256(cache / key / 'end.sc4')
        self.assertEqual(golden.cache_record(cache, key, manifest, run)[0], 'reproduced')
        self.assertEqual(golden.qualification(cache / key), 'reproduced')
        other = self.make_run('second', b'different')
        status, report = golden.cache_record(cache, key, manifest, other)
        self.assertEqual(status, 'nondeterministic')
        self.assertEqual(report['save']['different_records'], 1)
        self.assertEqual(golden.sha256(cache / key / 'end.sc4'), before)
        self.assertEqual(golden.qualification(cache / key), 'nondeterministic')
        golden.cache_record(cache, key, manifest, run)
        self.assertEqual(golden.qualification(cache / key), 'nondeterministic')

    def test_corrupt_cache_is_not_a_comparison_or_cache_hit(self):
        run = self.make_run('run')
        cache, key = self.root / 'cache', 'a' * 64
        golden.cache_record(cache, key, {}, run)
        (cache / key / 'end.sc4').write_bytes(b'corrupt')
        with self.assertRaisesRegex(ValueError, 'corrupt golden artifact'):
            golden.cache_record(cache, key, {}, run)

    def test_cli_records_once_reuses_cache_and_compares_only_the_candidate(self):
        run = self.make_run('original')
        golden.write_json(run / 'verdict.json', {'window_exact': True, 'save_date': '2026-03-01'})
        key, manifest = 'c' * 64, {'settings': {}}
        common = ['--city', 'Test', '--until', '2026-03-01', '--output', str(self.root)]
        defaults = golden.parser().parse_args(['record'] + common)
        self.assertEqual((defaults.clock, defaults.comparison, defaults.strict), ('original', 'auto', False))
        with patch.object(golden, 'prepare', return_value=(key, manifest, '', {})), \
                patch.object(golden, 'fingerprint', return_value=(key, manifest)), \
                patch.object(golden, 'run_game', return_value=run) as game, \
                contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(golden.main(['record'] + common), 0)
            self.assertEqual(game.call_count, 1)
            self.assertEqual(game.call_args.args[2], [])
            self.assertEqual(golden.main(['record'] + common), 0)
            self.assertEqual(game.call_count, 1)
            candidate = ['compare'] + common + ['--dll', str(self.root / 'candidate.dll')]
            self.assertEqual(golden.main(candidate), 0)
            self.assertEqual(game.call_count, 2)
            self.assertEqual(game.call_args.args[2], ['budget', 'simulator'])
            golden.cache_record(self.root / 'golden', key, manifest, self.make_run('drift', b'different'))
            self.assertEqual(golden.main(candidate), 0)
            self.assertEqual(game.call_count, 3)
            self.assertEqual(golden.main(candidate + ['--strict']), 3)
            self.assertEqual(game.call_count, 4)

    def test_script_has_a_full_save_and_replay_preserves_history(self):
        generated = golden.input_script(None, '2026-04-01')
        self.assertIn('2026-04-01 digest\n2026-04-01 save\n2026-04-01 mark GOLDEN_FULL_SAVE_COMPLETE\n', generated)
        self.assertNotIn('pause', generated)
        command = golden.shadow_command('FC4 Test', 'Konradshohe', '2026-03-01', Path('candidate'))
        self.assertIn('--profile shadows', command)
        self.assertIn('--until 2026-03-01', command)
        with self.assertRaises(ValueError):
            golden.safe_name('../Documents')

    def test_subsystem_selection_includes_nullptr_environment_entries(self):
        keys = golden.subsystem_keys(Path(__file__).resolve().parents[2])
        self.assertIn('simulator', keys)
        self.assertIn('budget', keys)
        self.assertIn('traffic', keys)

    def test_fixed_original_uses_only_scheduler_and_candidate_always_keeps_it(self):
        self.assertEqual(golden.effective_replacements('fixed', []), ['simulator'])
        self.assertEqual(golden.effective_replacements('fixed', ['traffic']), ['simulator', 'traffic'])
        self.assertEqual(golden.effective_replacements('original', []), [])

    def test_simulator_save_date_and_fields(self):
        payload = bytearray(91)
        struct.pack_into('<I', payload, 0, 91)
        struct.pack_into('<H', payload, 12, 4)
        struct.pack_into('<I', payload, 87, 2451545)
        key = (0x2990C1E5, 0, 0)
        self.assertEqual(golden_monthly.save_date({key: bytes(payload)}), '2000-01-01')
        self.assertEqual(records._fields(key, payload)['julian_day'], 2451545)
        # Version 3 (shipped unfounded cities) carries a 10-byte timer before julianDay.
        old = bytearray(101)
        struct.pack_into('<H', old, 12, 3)
        struct.pack_into('<I', old, 97, 2451552)
        self.assertEqual(golden_monthly.save_date({key: bytes(old)}), '2000-01-08')

    def test_log_requires_completed_window_and_exact_subsystems(self):
        text = ("subsystem 'netmgr': enabled\nsubsystem 'nettool': enabled\n"
                'netrules: enabled through netmgr/nettool; Rust tile-rule solver\n'
                'autotest: end date 2026-04-01 reached at the new day 2026-04-01\n'
                'script: line 4 2026-04-01 save -> ok=1 (SaveCity(full) in 10 ms)\n'
                'GOLDEN_FULL_SAVE_COMPLETE\nAUTOTEST VERDICT: clean\n')
        selected = ['netrules', 'netmgr', 'nettool']
        self.assertEqual(golden._validate_log(text, '2026-04-01', selected, self.root), 'clean')
        for wrong in (text.replace('ok=1', 'ok=0'), text.replace('clean', 'stalled'),
                      text + 'traffic shadow: ON\n'):
            with self.assertRaises(ValueError):
                golden._validate_log(wrong, '2026-04-01', selected, self.root)
        with self.assertRaises(ValueError):
            golden._validate_log(text, '2026-04-01', [], self.root)


if __name__ == '__main__':
    unittest.main()
