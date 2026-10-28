"""Checkpoint completeness, class-aware ID normalization, and bounded snapshots."""
import struct
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import golden_monthly as monthly


def objects(ids, payloads=None):
    cls = 0x12345678
    index = struct.pack('<IIII', 0, 1, 0, cls)
    payloads = payloads or [b'first', b'second']
    frames = b''.join(struct.pack('<III', 12 + len(p), 123, oid) + p for oid, p in zip(ids, payloads))
    return {(monthly.golden_relabel.INDEX, monthly.golden_relabel.STORE_GROUP, 0): index,
            (cls, monthly.golden_relabel.STORE_GROUP, 0): frames,
            (99, 0, 0): b'root' + struct.pack('<II', ids[1], cls)}


def test_normalization_keeps_content_sharing_and_class_but_not_object_ids():
    a, b = objects([1, 2]), objects([700, 900])
    assert monthly.normalized_records(a) == monthly.normalized_records(b)
    assert monthly.normalized_records(a) != monthly.normalized_records(objects([700, 900], [b'first', b'changed']))
    b[99, 0, 0] += struct.pack('<II', 700, 0x12345678)
    assert monthly.normalized_records(a) != monthly.normalized_records(b)


def test_normalization_does_not_shift_other_classes_when_one_is_added():
    a = objects([1, 2])
    b = dict(a)
    index = (monthly.golden_relabel.INDEX, monthly.golden_relabel.STORE_GROUP, 0)
    b[index] = struct.pack('<IIIIII', 0, 2, 0, 0x87654321, 0, 0x12345678)
    b[0x87654321, monthly.golden_relabel.STORE_GROUP, 0] = struct.pack('<III', 13, 0, 5) + b'x'
    b[1, 0, 0] = struct.pack('<II', 5, 0x87654321)
    before, after = monthly.normalized_records(a), monthly.normalized_records(b)
    assert before[99, 0, 0] == after[99, 0, 0]
    assert before[0x12345678, monthly.golden_relabel.STORE_GROUP, 0] == after[0x12345678, monthly.golden_relabel.STORE_GROUP, 0]


def test_fast_reference_search_preserves_unaligned_overlapping_and_bounded_ids():
    import random
    data = bytearray(random.Random(17).randbytes(20000))
    classes = {0x12345678, 0x00010001, 0x00000001}
    for at, oid, cls in ((3, 17, 0x12345678), (21, 0, 0x00000001),
                         (39, 0x01000000, 0x12345678), (101, 1, 0x00010001)):
        struct.pack_into('<II', data, at, oid, cls)
    expected = []
    for at in range(len(data) - 7):
        oid, cls = struct.unpack_from('<II', data, at)
        if cls in classes and 0 < oid < 0x01000000:
            expected.append((at, oid, cls))
    assert monthly.golden_relabel.refs(bytes(data), classes) == expected


def test_calendar_and_actions_at_boundary():
    assert monthly.dates('2000-01-31', '2000-03-02') == ['2000-02-29', '2000-03-02']
    text = monthly.instrument('2000-01-31 mark OLD\n2000-02-29 funds 100\n', '2000-01-31', '2000-03-02',
                              reloaded=True)
    assert 'mark OLD' not in text
    assert text.index('funds 100') < text.index('2000-02-29 digest') < text.index('2000-02-29 save')
    assert text.count('GOLDEN_MONTH') == 2
    assert '2000-02-29 resume' not in text
    assert 'uiwait' not in text
    assert 'pause' not in text


def test_initial_save_setup_is_not_discarded_as_if_it_were_a_checkpoint():
    script = '1999-12-31 funds 100\n2000-01-01 establish Example\n2000-02-01 mark future\n'
    assert '1999-12-31 funds 100' in monthly.instrument(script, '2000-01-01', '2000-01-31')
    assert '2000-01-01 establish Example' in monthly.instrument(script, '2000-01-01', '2000-01-31')
    assert 'mark future' not in monthly.instrument(script, '2000-01-01', '2000-01-31')
    assert 'establish Example' not in monthly.instrument(script, '2000-01-01', '2000-01-31', reloaded=True)


def timeline(values):
    return {'normalizer': monthly.NORMALIZER, 'start': '2000-01-01',
            'months': {f'2000-0{i + 2}-28': {'records': value} for i, value in enumerate(values)}}


def test_complete_comparison_keeps_later_owners_first_differences():
    a = timeline([{'A': '1', 'B': '2'}] * 3)
    b = timeline([{'A': '1', 'B': '2'}, {'A': 'x', 'B': '2'}, {'A': 'x', 'B': 'y'}])
    report = monthly.compare(a, b)
    assert report['first_difference']['date'] == '2000-03-28'
    assert report['last_matching_month'] == '2000-02-28'
    assert report['differences'][-1]['records'] == [
        {'tgi': 'A', 'status': 'different'}, {'tgi': 'B', 'status': 'different'}]


def test_missing_month_fails_closed_except_after_controlled_difference():
    a, b = timeline([{'A': '1'}] * 3), timeline([{'A': 'x'}])
    with pytest.raises(ValueError, match='missing monthly'):
        monthly.compare(a, b)
    b['stopped_early'] = True
    assert not monthly.compare(a, b)['complete']
    b['months']['2000-02-28']['records']['A'] = '1'
    with pytest.raises(ValueError, match='missing monthly'):
        monthly.compare(a, b)


def test_snapshots_thin_by_bytes_and_never_exceed_budget():
    sizes = {str(n): 40 for n in range(8)}
    chosen = monthly.snapshot_dates(sizes, budget=100)
    assert len(chosen) == 2
    assert sum(sizes[d] for d in chosen) <= 100
    assert monthly.snapshot_dates({'large': 101}, budget=100) == []
