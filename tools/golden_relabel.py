"""Compare two city saves modulo a relabelling of object IDs (diagnostics for tools/check.py).

The fixed schedule numbers saved objects 1, 2, 3, ... in the order their references are written (simulator.md 5.8),
so one reference written in another order or not at all shifts every later ID, and every record that holds a
shifted ID differs. This maps the IDs from aligned references and splits the differing records into those equal
once the IDs are mapped ("relabel") and those that still differ ("content"). The smallest shifted ID is where the
two saves' reference order forks. References are found as (id, class) pairs at any byte offset, class being one of
the save's classes (foundations 2.7, 3.3).

    python tools/golden_relabel.py REFERENCE.sc4 CANDIDATE.sc4
"""
import collections
import functools
import re
import struct
import sys

try:
    from .golden_dbpf import read_records, tgi_text
except ImportError:
    from golden_dbpf import read_records, tgi_text

STORE_GROUP = 0x299B2D1B
INDEX = 0x499B23FE


def classes_of(recs):
    for (t, g, _), v in recs.items():
        if (t, g) == (INDEX, STORE_GROUP):
            n, = struct.unpack_from('<I', v, 4)
            return {struct.unpack_from('<I', v, 12 + 8 * i)[0] for i in range(n)}
    return set()


def entries(data):
    out, at = [], 0
    while at + 12 <= len(data):
        size, _, oid = struct.unpack_from('<III', data, at)
        if size < 12 or at + size > len(data):
            return None
        out.append((oid, data[at + 12:at + size]))
        at += size
    return out if at == len(data) else None


@functools.lru_cache(maxsize=8)
def _reference_pattern(classes):
    return re.compile(b'|'.join(re.escape(struct.pack('<I', cls)) for cls in sorted(classes)))


def refs(data, classes):
    """(offset, id, class) of every plausible reference in data."""
    out = []
    if not classes:
        return out
    pattern = _reference_pattern(frozenset(classes))
    at = 4
    while True:
        match = pattern.search(data, at)
        if match is None:
            break
        i = match.start() - 4
        oid, cls = struct.unpack_from('<II', data, i)
        if 0 < oid < 0x01000000:
            out.append((i, oid, cls))
        at = match.start() + 1  # retain overlapping, unaligned matches
    return out


def learn(a, b, classes, fmap):
    if len(a) != len(b):
        return
    for i, oid, cls in refs(a, classes):
        if b[i + 4:i + 8] == a[i + 4:i + 8]:
            fmap[cls].setdefault(oid, int.from_bytes(b[i:i + 4], 'little'))


def substitute(data, classes, fmap):
    out = bytearray(data)
    for i, oid, cls in refs(data, classes):
        if oid in fmap[cls]:
            struct.pack_into('<I', out, i, fmap[cls][oid])
    return bytes(out)


def store(t, g):
    return g == STORE_GROUP and t != INDEX


def compare(reference, candidate):
    """{'records': {tgi: 'relabel' | 'content' | 'missing'}, 'first_shift': (id, class) or None} for the records that
    differ between two saves (paths or record dicts)."""
    A = reference if isinstance(reference, dict) else read_records(reference)
    B = candidate if isinstance(candidate, dict) else read_records(candidate)
    classes = classes_of(A) | classes_of(B)
    fmap = collections.defaultdict(dict)
    # Ordinary records and single-entry class records first, then entries matched by mapped ID.
    for k in sorted(set(A) & set(B)):
        if A[k] == B[k]:
            continue
        if store(k[0], k[1]):
            ea, eb = entries(A[k]), entries(B[k])
            if ea and eb and len(ea) == 1 and len(eb) == 1:
                learn(ea[0][1], eb[0][1], classes, fmap)
                fmap[k[0]].setdefault(ea[0][0], eb[0][0])
        else:
            learn(A[k], B[k], classes, fmap)
    for _ in range(3):
        for k in sorted(set(A) & set(B)):
            if A[k] == B[k] or not store(k[0], k[1]):
                continue
            ea, eb = entries(A[k]), entries(B[k])
            if not ea or not eb:
                continue
            bmap = dict(eb)
            for oid, payload in ea:
                other = bmap.get(fmap[k[0]].get(oid, oid))
                if other is not None:
                    learn(payload, other, classes, fmap)
    verdicts = {}
    for k in sorted(set(A) | set(B)):
        a, b = A.get(k), B.get(k)
        if a == b:
            continue
        if a is None or b is None:
            verdict = 'missing'
        elif store(k[0], k[1]):
            ea, eb = entries(a), entries(b)
            if ea is None or eb is None:
                verdict = 'content'
            else:
                sa = sorted((fmap[k[0]].get(oid, oid), substitute(p, classes, fmap)) for oid, p in ea)
                verdict = 'relabel' if sa == sorted(eb) else 'content'
        else:
            verdict = 'relabel' if substitute(a, classes, fmap) == b else 'content'
        verdicts[tgi_text(k)] = verdict
    shifted = sorted((x, cls) for cls, m in fmap.items() for x, y in m.items() if x != y)
    return {'records': verdicts, 'first_shift': shifted[0] if shifted else None}


if __name__ == '__main__':
    result = compare(sys.argv[1], sys.argv[2])
    for tgi, verdict in result['records'].items():
        print(f'{tgi}  {verdict}')
    print(dict(collections.Counter(result['records'].values())))
    shift = result['first_shift']
    print(f'IDs shift from {shift[0]} (class {shift[1]:08X})' if shift else 'no IDs shift')
