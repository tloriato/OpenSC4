"""Monthly reporting, controlled stops and per-owner evidence across the pipeline."""
import argparse
import json
from pathlib import Path
import sys
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import check
import golden
import golden_envelope
import golden_monthly as monthly


def test_monthly_report_prints_hash_only_records(capsys):
    report = {'save': {'exact_records': 4, 'different_records': 1,
                       'records': [{'tgi': '12345678/00000000/00000000', 'status': 'different'}]},
              'first_digest_difference': None,
              'monthly': {'first_difference': {'date': '2010-05-31'},
                          'last_matching_month': '2010-04-30', 'complete': False}}
    args = argparse.Namespace(region='Timbuktu', city='Rush Hour Tutorial', until='2010-07-31',
                              dll=Path('candidate.dll'), script=None)
    golden.print_comparison(report, args)
    out = capsys.readouterr().out
    assert '4 exact, 1 different' in out
    assert 'monthly: first difference 2010-05-31; last matched 2010-04-30' in out
    assert 'later owners incomplete' in out


def test_owner_attribution_records_later_differences_without_reruns(tmp_path):
    records = {'0000000A/0/0': 'a', '0000000B/0/0': 'b'}
    (tmp_path / 'monthly.json').write_text(json.dumps({'months': {'2010-05-31': {'records': records}}}))
    report = {'run': str(tmp_path), 'monthly': {'complete': True, 'differences': [
        {'date': '2010-05-31', 'records': [{'tgi': '0000000A/0/0'}]},
        {'date': '2010-06-30', 'records': [{'tgi': '0000000A/0/0'}, {'tgi': '0000000B/0/0'}]}]}}
    check.attribute_owners(report, ['a', 'b', 'c'], {10: ('a', 'A'), 11: ('b', 'B')})
    assert report['owners']['a']['first_difference']['date'] == '2010-05-31'
    assert report['owners']['b']['first_difference']['date'] == '2010-06-30'
    assert report['owners']['c']['verdict'] == 'unobserved'
    report['monthly']['differences'] = report['monthly']['differences'][:1]
    report['monthly']['complete'] = False
    check.attribute_owners(report, ['a', 'b'], {10: ('a', 'A'), 11: ('b', 'B')})
    assert report['owners']['a']['verdict'] == 'different'
    assert report['owners']['b']['verdict'] == 'incomplete'


def test_live_watcher_captures_evidence_then_stops_only_its_game(tmp_path):
    slot = tmp_path / 'slot'
    (slot / 'Plugins').mkdir(parents=True)
    city = slot / 'Regions/FC4 Test/City - Example.sc4'
    city.parent.mkdir(parents=True)
    city.write_bytes(b'city snapshot')
    script = tmp_path / 'actions.script'
    script.write_text('unique script')
    log = f'[pid 123 tid 456] DLL_PROCESS_ATTACH\nscript {script.resolve()}\nGOLDEN_MONTH 2010-05-31\n'
    (slot / 'Plugins/FreeCity4.log').write_text(log)
    reference = {'months': {'2010-05-31': {'records': {'A': 'original'}}}}
    result = tmp_path / 'result'
    watcher = monthly.Watcher(slot, script, result, '2010-04-30', '2010-07-31', reference)
    watcher.city, watcher.expected_pid = 'Example', 123
    process = Mock()
    with patch.object(monthly, 'GameProcess', return_value=process) as launch, \
            patch.object(monthly, 'record_hashes', return_value={
                'date': '2010-05-31', 'records': {'A': 'candidate'}}):
        watcher.tick()
    launch.assert_called_once_with(123)
    process.suspend.assert_called_once()
    process.stop.assert_called_once()
    process.resume.assert_not_called()
    evidence = json.loads((result / 'monthly.json').read_text())
    assert evidence['stop_date'] == '2010-05-31'
    assert evidence['stopped_early']
    assert (result / 'end.sc4').read_bytes() == b'city snapshot'
    assert (result / 'game.log').read_text() == log


def test_terminal_checkpoint_does_not_open_an_already_exited_game(tmp_path):
    slot = tmp_path / 'slot'
    (slot / 'Plugins').mkdir(parents=True)
    city = slot / 'Regions/FC4 Test/City - Example.sc4'
    city.parent.mkdir(parents=True)
    city.write_bytes(b'final save')
    script = tmp_path / 'actions.script'
    (slot / 'Plugins/FreeCity4.log').write_text(
        f'[pid 123 tid 456] DLL_PROCESS_ATTACH\n{script.resolve()}\nGOLDEN_MONTH 2010-05-31\n')
    result = tmp_path / 'result'
    watcher = monthly.Watcher(slot, script, result, '2010-04-30', '2010-05-31')
    watcher.city = 'Example'
    with patch.object(monthly, 'GameProcess', side_effect=AssertionError('game already exited')), \
            patch.object(monthly, 'record_hashes', return_value={'date': '2010-05-31', 'records': {}}):
        watcher.tick()
    assert '2010-05-31' in json.loads((result / 'monthly.json').read_text())['months']


def test_credit_always_captures_all_months_for_combined_owners():
    options = check.run_options(argparse.Namespace(command='credit'))
    assert '--monthly' in options
    assert '--no-early-stop' in options


def test_draw_runs_do_not_receive_monthly_save_or_window_flags():
    row = dict(name='draw-city', kind='draw', region='Test', city='Example', end='2000-01-01', script='-',
               draw_capture='1', minimize='0', draw_capture_scenarios='1', seconds='45',
               draw_capture_frames='2', draw_capture_warmup='1')
    with patch.object(check.subprocess, 'run', return_value=Mock(returncode=0, stdout='', stderr='')) as run:
        check.golden('compare', row, '--monthly', '--from', 'auto', '--months', '1', '--no-early-stop')
    command = run.call_args.args[0]
    assert '--draw-capture' in command
    assert not {'--monthly', '--from', '--months', '--no-early-stop'} & set(command)


def test_combined_credit_distinguishes_unobserved_scenarios_from_failed_runs():
    table = {'present': {'verdict': 'outside', 'owners': {'zones': {'verdict': 'exact'}}},
             'absent': {'verdict': 'exact', 'owners': {}}}
    summary = check.owner_summary(table, 'zones')
    assert summary['verdict'] == 'exact'
    assert summary['exact_scenarios'] == ['present']
    assert summary['unobserved_scenarios'] == ['absent']
    table['failed'] = {'verdict': 'error', 'owners': {}}
    summary = check.owner_summary(table, 'zones')
    assert summary['verdict'] == 'incomplete'
    assert summary['incomplete_scenarios'] == ['failed']


def test_exact_keys_can_come_from_combined_credit_without_each(tmp_path, monkeypatch):
    monkeypatch.setattr(check, 'REPO', tmp_path)
    monkeypatch.setattr(check, 'STORE', tmp_path / 'build-runs/golden')
    monkeypatch.setattr(check, 'owner_covers', lambda: {'zones': ['a', 'b']})
    monkeypatch.setattr(check, 'scenarios', lambda: {})
    monkeypatch.setattr(check, 'select', lambda *args: ([], []))
    dll = tmp_path / 'candidate.dll'
    dll.write_bytes(b'candidate')
    path = tmp_path / 'build-runs/golden/credit.json'
    path.parent.mkdir(parents=True)
    report = {'monthly': {'complete': True}, 'owners': {'zones': {'verdict': 'exact'}}}
    data = {'dll_sha256': check.sha256(dll), 'keys': ['zones'],
            'owner_verdicts': {'zones': {'verdict': 'exact'}}, 'scenarios': {'a': report}}
    path.write_text(json.dumps(data))
    assert check.exact_owners(dll) == []  # a short selection cannot qualify a whole cover
    data['scenarios']['b'] = report
    path.write_text(json.dumps(data))
    assert check.exact_owners(dll) == ['zones']
    data['scenarios']['b'] = {**report, 'monthly': {'complete': False}}
    path.write_text(json.dumps(data))
    assert check.exact_owners(dll) == []
    dll.write_bytes(b'changed build')
    import pytest
    with pytest.raises(ValueError, match='no full-cover owner evidence'):
        check.exact_owners(dll)


def test_legacy_credit_keeps_the_each_table_working(tmp_path, monkeypatch):
    monkeypatch.setattr(check, 'REPO', tmp_path)
    monkeypatch.setattr(check, 'STORE', tmp_path / 'build-runs/golden')
    monkeypatch.setattr(check, 'scenarios', lambda: {})
    dll = tmp_path / 'candidate.dll'
    dll.write_bytes(b'candidate')
    cache = tmp_path / 'build-runs/golden'
    cache.mkdir(parents=True)
    (cache / 'credit.json').write_text(json.dumps({
        'dll_sha256': check.sha256(dll), 'owner_verdicts': {'zones': {'verdict': 'unobserved'}},
        'scenarios': {'city': {'monthly': None}}}))
    (cache / 'owner-check.tsv').write_text(
        f'# sha256 {check.sha256(dll)}\nkey\tscenario\tverdict\nzones\tcity\texact\n')
    assert check.exact_owners(dll) == ['zones']


def test_window_results_cannot_be_published_as_full_scenario_credit():
    import pytest
    with pytest.raises(ValueError, match='credit requires full scenarios'):
        check.credit(argparse.Namespace(from_date='auto'))


def test_auto_window_cannot_use_another_lanes_failure(tmp_path):
    entry = tmp_path / 'reference'
    entry.mkdir()
    other = tmp_path / 'other-lane/candidate.dll'
    mine = tmp_path / 'my-lane/candidate.dll'
    golden.write_json(golden.comparison_history(entry, ['zones'], other), {
        'first_difference': {'date': '2000-03-31'}, 'last_matching_month': '2000-02-29'})
    args = argparse.Namespace(from_date='auto', dll=mine)
    assert golden.window_inputs(args, entry, 'parent', ['zones']) is False
    (entry / 'months').mkdir()
    (entry / 'months/2000-02-29.sc4').write_bytes(b'reference checkpoint')
    golden.write_json(entry / 'monthly.json', {'start': '2000-01-01'})
    golden.write_json(golden.comparison_history(entry, ['zones'], mine), {
        'first_difference': {'date': '2000-03-31'}, 'last_matching_month': '2000-02-29'})
    args.until, args.months, args.output = '2000-03-31', 1, tmp_path / 'output'
    args.regions_root, args.region, args.city = tmp_path, 'Example', 'City'
    assert golden.window_inputs(args, entry, 'parent', ['zones']) is True
    assert args.prepared_city.read_bytes() == b'reference checkpoint'
    assert args.until == '2000-03-31'


def test_envelope_verifies_monthly_snapshots_and_rejects_path_traversal(tmp_path):
    import hashlib
    import pytest
    snapshot = tmp_path / 'months/2010-05-31.sc4'
    snapshot.parent.mkdir()
    snapshot.write_bytes(b'saved city')
    digest = hashlib.sha256(snapshot.read_bytes()).hexdigest()
    metadata = tmp_path / 'baseline.json'
    metadata.write_text(json.dumps({'artifacts': {'months/2010-05-31.sc4': digest}}))
    assert golden_envelope.sample_paths(tmp_path) == [tmp_path]
    snapshot.write_bytes(b'corrupted city')
    with pytest.raises(ValueError, match='corrupt golden sample artifact'):
        golden_envelope.sample_paths(tmp_path)
    metadata.write_text(json.dumps({'artifacts': {'months/../baseline.json': digest}}))
    with pytest.raises(ValueError, match='corrupt golden sample artifact'):
        golden_envelope.sample_paths(tmp_path)
