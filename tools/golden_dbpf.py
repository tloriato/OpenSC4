"""Read-only, bounded DBPF/QFS record comparison for golden city saves.

No gameplay bytes are ignored. The DBPF header/index and DIR are container
metadata: records are matched by full TGI, then compared after decompression.
Field decoders cover the exemplar/cohort and LTEXT formats in engine/formats.
Unknown records retain exact byte comparison, sizes and the first byte offset.
"""
import hashlib
import struct
from pathlib import Path

try:
    from . import dbpf
except ImportError:
    import dbpf

DIR = 0xE86B1EEF
MAX_RECORD = 256 * 1024 * 1024


def tgi_text(key):
    t, g, i = key
    return f'{t:08X}/{g:08X}/{i:016X}'


def _slice(data, offset, size):
    if offset < 0 or size < 0 or offset + size > len(data):
        raise ValueError(f'DBPF range {offset}+{size} exceeds {len(data)} bytes')
    return data[offset:offset + size]


def _qfs(data, expected):
    """Bounded RefPack decoder; output size is checked against DIR and header."""
    pos = 4 if len(data) >= 9 and data[5] == 0xFB and (data[4] & 0xFE) == 0x10 else 0
    head = _slice(data, pos, 5)
    if head[1] != 0xFB or head[0] & 0xFE != 0x10:
        raise ValueError('invalid QFS header')
    declared = int.from_bytes(head[2:5], 'big')
    if declared != expected or declared > MAX_RECORD:
        raise ValueError(f'QFS size {declared} differs from DIR {expected} or exceeds limit')
    pos += 5 + (3 if head[0] & 1 else 0)
    out = bytearray()
    while True:
        c = _slice(data, pos, 1)[0]
        pos += 1
        stop = False
        if c < 0x80:
            b = _slice(data, pos, 1)[0]
            pos += 1
            plain, count, distance = c & 3, ((c >> 2) & 7) + 3, ((c & 0x60) << 3) + b + 1
        elif c < 0xC0:
            b, d = _slice(data, pos, 2)
            pos += 2
            plain, count, distance = b >> 6, (c & 0x3F) + 4, ((b & 0x3F) << 8) + d + 1
        elif c < 0xE0:
            b, d, e = _slice(data, pos, 3)
            pos += 3
            plain, count, distance = c & 3, ((c & 0x0C) << 6) + e + 5, ((c & 0x10) << 12) + (b << 8) + d + 1
        else:
            plain, count, distance = (((c & 0x1F) << 2) + 4 if c < 0xFC else c & 3), 0, 0
            stop = c >= 0xFC
        if len(out) + plain + count > declared:
            raise ValueError('QFS command exceeds declared output size')
        out.extend(_slice(data, pos, plain))
        pos += plain
        if count and distance > len(out):
            raise ValueError('QFS back reference precedes output')
        for _ in range(count):
            out.append(out[-distance])
        if stop:
            break
    if len(out) != declared:
        raise ValueError(f'QFS output size {len(out)} != {declared}')
    return bytes(out)


def read_records(path, *, decompress=True):
    """Read records, optionally retaining compressed bytes for an inventory-only census."""
    data = Path(path).read_bytes()
    head = _slice(data, 0, 96)
    if head[:4] != b'DBPF':
        raise ValueError('not a DBPF file')
    if struct.unpack_from('<II', head, 4) not in ((1, 0), (1, 1)):
        raise ValueError('unsupported DBPF version')
    major, count, offset, size = struct.unpack_from('<IIII', head, 0x20)
    minor = struct.unpack_from('<I', head, 0x3C)[0]
    if major != 7 or minor not in (0, 1, 2):
        raise ValueError(f'unsupported DBPF index {major}.{minor}')
    width = 24 if minor >= 1 else 20
    # SC4 files often leave the minor word zero with a 24-byte index.
    if count and size == count * 24:
        width = 24
    if size != count * width:
        raise ValueError('DBPF index size/count mismatch')
    index = _slice(data, offset, size)
    entries = {}
    for at in range(0, size, width):
        words = struct.unpack_from('<' + 'I' * (width // 4), index, at)
        t, g, low = words[:3]
        key = (t, g, (words[3] << 32 | low) if width == 24 else low)
        if key in entries:
            raise ValueError(f'duplicate TGI {tgi_text(key)}')
        entries[key] = _slice(data, words[-2], words[-1])
    compressed = {}
    # engine/formats/src/dbpf.rs: DIR uses 16-byte entries regardless of the
    # index width. A low-instance collision in a 7.1 extension is ambiguous.
    dir_width = 16
    for key, raw in entries.items():
        if key[0] != DIR:
            continue
        if len(raw) % dir_width:
            raise ValueError('truncated DIR record')
        for at in range(0, len(raw), dir_width):
            words = struct.unpack_from('<' + 'I' * (dir_width // 4), raw, at)
            t, g, low = words[:3]
            matches = [candidate for candidate in entries
                       if candidate[0] == t and candidate[1] == g and candidate[2] & 0xFFFFFFFF == low]
            if len(matches) != 1:
                raise ValueError('DIR entry does not identify a unique full TGI')
            ck = matches[0]
            if ck in compressed or ck not in entries or ck[0] == DIR:
                raise ValueError('invalid or duplicate DIR entry')
            compressed[ck] = words[-1]
    return {key: _qfs(raw, compressed[key]) if decompress and key in compressed else raw
            for key, raw in entries.items() if key[0] != DIR}


def _fields(key, data):
    if key[0] == 0x2990C1E5 and len(data) == 91 and data[12:14] == b'\x04\x00':
        # Class-object framing (12 bytes), then core/src/save.rs v4.
        result = {'object_id': struct.unpack_from('<I', data, 8)[0], 'version': 4,
                  'in_tick': data[14], 'city_established': data[15]}
        result.update(zip(('pause_count', 'sim_speed', 'sim_time', 'frame_budget_ms'),
                          struct.unpack_from('<iiiI', data, 16)))
        for level in range(11):
            result[f'level.{level}.counter'] = struct.unpack_from('<I', data, 32 + level * 5)[0]
            result[f'level.{level}.fired'] = data[36 + level * 5]
        result['julian_day'] = struct.unpack_from('<I', data, 87)[0]
        return result
    if key[0] in (0x6534284A, 0x05342861):
        parent, properties = dbpf.parse_exemplar(data)
        return {'parent': list(parent), **{f'property.{pid:08X}': [kind, values]
                                         for pid, (kind, values) in properties.items()}}
    if key[0] == 0x2026960B:  # engine/formats/src/ltext.rs / reader.rs gzstr
        word = struct.unpack_from('<I', data)[0]
        count, encoding = word & 0xFFFFFF, word >> 24
        if encoding == 0x10:
            text = _slice(data, 4, count * 2).decode('utf-16-le', errors='replace')
        elif encoding == 0:
            raw = _slice(data, 4, count)
            try:
                text = raw.decode('utf-8')
            except UnicodeError:
                text = raw.decode('latin-1')
        else:
            raise ValueError('unsupported LTEXT encoding')
        return {'text': text}
    return {}


def compare_saves(expected, actual):
    left, right = read_records(expected), read_records(actual)
    changes = []
    exact = 0
    for key in sorted(left.keys() | right.keys()):
        a, b = left.get(key), right.get(key)
        if a == b:
            exact += 1
            continue
        item = {'tgi': tgi_text(key), 'status': 'added' if a is None else 'removed' if b is None else 'different',
                'expected_size': len(a) if a is not None else None,
                'actual_size': len(b) if b is not None else None}
        if a is not None and b is not None:
            item['first_byte'] = next((i for i, (x, y) in enumerate(zip(a, b)) if x != y), min(len(a), len(b)))
            item['bytes_different'] = sum(x != y for x, y in zip(a, b)) + abs(len(a) - len(b))
            for label, value in (('expected', a), ('actual', b)):
                item[label + '_sha256'] = hashlib.sha256(value).hexdigest()
            try:
                af, bf = _fields(key, a), _fields(key, b)
                item['fields'] = {field: {'expected': af.get(field), 'actual': bf.get(field)}
                                  for field in sorted(af.keys() | bf.keys()) if af.get(field) != bf.get(field)}
            except (ValueError, IndexError, KeyError, struct.error, UnicodeError) as exc:
                item['decode_error'] = str(exc)
        changes.append(item)
    return {'exact_records': exact, 'different_records': len(changes), 'records': changes,
            'ignored': ['DBPF header/index/layout', 'DIR compression metadata'], 'ignored_gameplay_fields': []}
