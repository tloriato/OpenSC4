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


def test_credit_always_captures_all_months_for_combined_owners():
    options = check.run_options(argparse.Namespace(command='credit'))
    assert '--monthly' in options
    assert '--no-early-stop' in options


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
