"""Measured original-run envelopes. Unknown save fields are byte-offset ranges.

No volatile masks, RNG repair, or guessed binary field types. Stable bytes and
decoded fields remain exact. Ranges describe observations, not correctness.
Only original recordings enter this module; comparisons never learn.
"""
import base64
import hashlib
import json
import math
from pathlib import Path
import re
import shutil
import statistics
import struct
import time
import uuid
import zlib

try:
    from .golden_dbpf import read_records, tgi_text, _fields
except ImportError:
    from golden_dbpf import read_records, tgi_text, _fields


def _json(path):
    return json.loads(path.read_text(encoding='utf-8'))


def _hash(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def snapshot(entry, run):
    """Keep each independent original immutable, including its logs and verdict."""
    directory = entry / 'samples'
    directory.mkdir(exist_ok=True)
    identity = uuid.uuid4().hex
    stage = directory / ('.pending-' + identity)
    stage.mkdir()
    try:
        names = ['end.sc4', 'digests.json', 'verdict.json', 'summary.json', 'game.log', 'harness.txt']
        if (run / 'reach.tsv').is_file():
            names.append('reach.tsv')
        if (run / 'monthly.json').is_file():
            names.append('monthly.json')
        for name in names:
            shutil.copyfile(run / name, stage / name)
        metadata = {'source_run': str(run.resolve()), 'recorded_ns': time.time_ns(),
                    'artifacts': {n: _hash(stage / n) for n in names}}
        (stage / 'sample.json').write_text(json.dumps(metadata, sort_keys=True), encoding='utf-8')
        # A scanner can still hold a just-written file (WinError 5); retry briefly.
        for attempt in range(20):
            try:
                stage.rename(directory / identity)
                break
            except PermissionError:
                if attempt == 19:
                    raise
                time.sleep(0.5)
    finally:
        if stage.exists():
            shutil.rmtree(stage)
    return identity


def sample_paths(entry):
    repeats = [p.parent for p in (entry / 'samples').glob('*/sample.json')]
    paths = [entry] + sorted(repeats, key=lambda p: (_json(p / 'sample.json')['recorded_ns'], p.name))
    for path in paths:
        metadata = _json(path / ('baseline.json' if path == entry else 'sample.json'))
        for name, expected in metadata['artifacts'].items():
            nested_snapshot = re.fullmatch(r'months/\d{4}-\d{2}-\d{2}\.sc4', name)
            if ((Path(name).name != name and not nested_snapshot)
                    or not (path / name).is_file() or _hash(path / name) != expected):
                raise ValueError(f'corrupt golden sample artifact: {path / name}')
    return paths


def flatten(value, prefix=''):
    if isinstance(value, dict):
        return {k: v for name, item in value.items() for k, v in flatten(item, prefix + str(name) + '.').items()}
    if isinstance(value, list):
        return {k: v for i, item in enumerate(value) for k, v in flatten(item, prefix + str(i) + '.').items()}
    return {prefix.rstrip('.'): value}


def digest_fields(row):
    result = {}
    for key, value in row.get('fields', {}).items():
        parts = value.split('/')
        for i, part in enumerate(parts):
            name = key if len(parts) == 1 else f'{key}.{i}'
            try:
                result[name] = int(part)
            except ValueError:
                try:
                    value = float(part)
                    result[name] = value if math.isfinite(value) else part
                except ValueError:
                    result[name] = part
    return result


def domain(values):
    if all(isinstance(v, (int, float)) and not isinstance(v, bool) for v in values):
        return {'min': min(values), 'max': max(values)}
    # Preserve types, absence and structured values without string coercion.
    return {'values': [json.loads(v) for v in sorted({json.dumps(v, sort_keys=True) for v in values})]}


def contains(rule, value):
    if 'values' in rule:
        return any(type(value) is type(v) and value == v for v in rule['values'])
    return isinstance(value, (int, float)) and not isinstance(value, bool) and rule['min'] <= value <= rule['max']


def varying(rule):
    return len(rule['values']) > 1 if 'values' in rule else rule['min'] != rule['max']


def packed(data):
    return base64.b64encode(zlib.compress(bytes(data))).decode('ascii')


def unpacked(data):
    return zlib.decompress(base64.b64decode(data))


def decoded(key, data):
    try:
        return flatten(_fields(key, data))
    except (ValueError, IndexError, KeyError, UnicodeError, struct.error):
        return {}


def decoded_spans(key, data):
    # Only fields with proven offsets may replace raw-byte checks. For other
    # decoders enforce BOTH decoded values and the opaque byte ranges.
    if key[0] == 0x2990C1E5 and len(data) == 91 and data[12:14] == b'\x04\x00':
        return set(range(8, 91))
    return set()


def build(entry, limit=None):
    """Rebuild from hash-verified samples; never trust an edited derived envelope."""
    paths = sample_paths(Path(entry))[:limit]
    saves = [read_records(p / 'end.sc4') for p in paths]
    digests = [_json(p / 'digests.json') for p in paths]
    n = len(paths)
    records = {}
    byte_count = field_count = record_count = digest_count = 0
    numeric_spreads = []
    byte_spreads = []
    digest_spreads = {}
    for key in sorted(set().union(*(s.keys() for s in saves))):
        payloads = [s[key] for s in saves if key in s]
        size = domain([len(p) for p in payloads])
        low, high = bytearray(payloads[0]), bytearray(payloads[0])
        for payload in payloads[1:]:
            for i, value in enumerate(payload):
                if i >= len(low):
                    low.append(value)
                    high.append(value)
                else:
                    low[i] = min(low[i], value)
                    high[i] = max(high[i], value)
        decoded_rows = [decoded(key, p) for p in payloads]
        fields = {f: domain([row.get(f) for row in decoded_rows])
                  for f in sorted(set().union(*(row.keys() for row in decoded_rows)))}
        skip = decoded_spans(key, payloads[0])
        changed_bytes = sum(a != b for i, (a, b) in enumerate(zip(low, high)) if i not in skip)
        byte_spreads.extend(b - a for i, (a, b) in enumerate(zip(low, high)) if i not in skip and a != b)
        changed_fields = sum(varying(rule) for rule in fields.values())
        structure = int(len(payloads) != n) + int(varying(size))
        record_count += bool(changed_bytes or changed_fields or structure)
        byte_count += changed_bytes
        field_count += changed_fields + structure
        numeric_spreads.extend(r['max'] - r['min'] for r in fields.values() if 'min' in r and varying(r))
        records[tgi_text(key)] = {'present': len(payloads), 'size': size, 'low': packed(low), 'high': packed(high),
                                   'fields': fields, 'decoded_offsets': sorted(skip)}
    monthly = []
    for i in range(max(map(len, digests), default=0)):
        rows = [rows[i] if i < len(rows) else None for rows in digests]
        maps = [digest_fields(row) if row else {} for row in rows]
        fields = {f: domain([row.get(f) for row in maps]) for f in sorted(set().union(*(r.keys() for r in maps)))}
        date_rule = domain([row['date'] if row else None for row in rows])
        digest_count += sum(varying(r) for r in fields.values()) + int(varying(date_rule))
        for f, rule in fields.items():
            if 'min' in rule and varying(rule):
                digest_spreads[f'{i}:{rows[0]["date"] if rows[0] else "missing"}:{f}'] = rule['max'] - rule['min']
        monthly.append({'date': date_rule, 'fields': fields})
    return {'schema': 1, 'samples': n, 'records': records, 'digests': monthly,
            'stats': {'records': len(records), 'varying_records': record_count,
                      'varying_decoded_fields': field_count, 'varying_opaque_bytes': byte_count,
                      'varying_digest_fields': digest_count,
                      'varying_count': field_count + byte_count + digest_count,
                      'median_decoded_numeric_spread': statistics.median(numeric_spreads) if numeric_spreads else 0,
                      'median_opaque_byte_spread': statistics.median(byte_spreads) if byte_spreads else 0,
                      'digest_spreads': digest_spreads},
            'limits': 'Observed marginal ranges; correlations and unseen variance are not inferred. No gameplay masks.'}


def status(entry, envelope):
    if envelope['samples'] < 3:
        return 'unqualified'
    if not envelope['stats']['varying_count']:
        return 'qualified'
    return 'enveloped'


def compare(envelope, actual):
    actual = Path(actual)
    saved = read_records(actual / 'end.sc4')
    keyed = {tgi_text(k): (k, v) for k, v in saved.items()}
    changes = []
    exact = 0
    for tgi in sorted(envelope['records'].keys() | keyed.keys()):
        rule, item = envelope['records'].get(tgi), keyed.get(tgi)
        fields = {}
        if rule is None or item is None:
            if rule is not None and rule['present'] < envelope['samples']:
                exact += 1
                continue
            changes.append({'tgi': tgi, 'status': 'added' if rule is None else 'removed',
                            'expected_size': None, 'actual_size': len(item[1]) if item else None})
            continue
        key, payload = item
        if not contains(rule['size'], len(payload)):
            fields['record_size'] = {'expected': rule['size'], 'actual': len(payload)}
        low, high = unpacked(rule['low']), unpacked(rule['high'])
        skip = set(rule['decoded_offsets'])
        outside = [i for i, b in enumerate(payload) if i not in skip and (i >= len(low) or not low[i] <= b <= high[i])]
        values = decoded(key, payload)
        for name in rule['fields'].keys() | values.keys():
            bound = rule['fields'].get(name)
            if bound is None or not contains(bound, values.get(name)):
                fields[name] = {'expected': bound, 'actual': values.get(name)}
        if outside or fields:
            changes.append({'tgi': tgi, 'status': 'outside-envelope', 'expected_size': rule['size'],
                            'actual_size': len(payload), 'first_byte': outside[0] if outside else None,
                            'bytes_different': len(outside), 'fields': fields})
        else:
            exact += 1  # compatible record count; may vary within the envelope
    rows = _json(actual / 'digests.json')
    first = None
    for i in range(max(len(rows), len(envelope['digests']))):
        row = rows[i] if i < len(rows) else None
        rule = envelope['digests'][i] if i < len(envelope['digests']) else None
        values = digest_fields(row) if row else {}
        fields = {}
        if row is None or rule is None or not contains(rule['date'], row['date']):
            fields['occurrence/date'] = {'expected': rule['date'] if rule else None, 'actual': row['date'] if row else None}
        if rule:
            for name in rule['fields'].keys() | values.keys():
                bound = rule['fields'].get(name)
                if bound is None or not contains(bound, values.get(name)):
                    fields[name] = {'expected': bound, 'actual': values.get(name)}
        if fields:
            expected_dates = (rule or {}).get('date', {}).get('values', [])
            first = {'date': row['date'] if row else next((v for v in expected_dates if v), 'unknown'),
                     'expected': rule, 'actual': row, 'fields': fields}
            break
    return {'first_digest_difference': first,
            'save': {'exact_records': exact, 'different_records': len(changes), 'records': changes,
                     'ignored_gameplay_fields': []},
            'comparison_rule': 'observed-envelope', 'envelope_stats': envelope['stats']}
