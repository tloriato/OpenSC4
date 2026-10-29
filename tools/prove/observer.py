"""Narrow write-provenance rule for the pinned capture observer's private image."""
import struct

from .common import PAGE, Ranges

# This rule belongs to this observer build, not to arbitrary loaded DLLs.
PIN = 'ee75f849f82f1fd3127865594bb0b8f67aae101b4a3f33dae6b31b9091f564a4'
SUBSTITUTED_ENTRIES = {0x0068bf50: 'fixed-schedule timer start',
                       0x0068bf80: 'fixed-schedule timer expiry',
                       0x0070e2b0: 'fixed-schedule remaining-time constant'}


class ObserverWrites:
    def __init__(self, rec):
        regions = [r for r in rec['regions'] if len(r) >= 6 and r[2] == 0x1000
                   and r[4] == 0x1000000 and r[5].replace('/', '\\').lower().endswith('\\freecity4.dll')]
        if rec.get('pinned', {}).get('dll_sha256') != PIN:
            regions = []
        # Adjacent VirtualQuery regions often differ only in copy-on-write
        # protection. Merge them to keep the number of narrow hooks small.
        self.image = Ranges((r[0], r[0]+r[1]) for r in regions)
        self.writable = []
        for lo, hi in sorted((r[0], r[0]+r[1]) for r in regions if r[3] & 0xff in (4, 8, 0x40, 0x80)):
            if self.writable and self.writable[-1][1] == lo:
                self.writable[-1] = (self.writable[-1][0], hi)
            else:
                self.writable.append((lo, hi))
        self.reset()

    def reset(self):
        self.writes = set()

    def write(self, addr, size, eip):
        for lo, hi in self.writable:
            start, end = max(addr, lo), min(addr+size, hi)
            if start < end:
                self.writes.add((start, end, eip, eip in self.image))

    def outcome(self):
        return [list(w) for w in sorted(self.writes)]

    def reference_limitation(self, rec, provider):
        addr = rec['function']
        if addr not in SUBSTITUTED_ENTRIES:
            return None
        page = provider.get(addr & -PAGE)
        if page is None or addr % PAGE+5 > PAGE:
            return None
        code = page[addr % PAGE:addr % PAGE+5]
        if code[0] != 0xe9:
            return None
        target = (addr+5+struct.unpack('<i', code[1:])[0]) & 0xffffffff
        if target in self.image:
            return dict(field='observer replaces tested entry', target=f'{target:08x}',
                        detail=SUBSTITUTED_ENTRIES[addr]+'; an unmodified CPU reference is required')
        return None


def normalize(left, right, initial):
    """Exclude only differing bytes with observer writers and no foreign writer.

    Remember every writer, rather than just the last one: an observer write
    cannot hide an earlier game or port write to that byte in either replay.
    """
    own, foreign = {}, set()
    for out in (left[0], right[0]):
        for lo, hi, eip, private in out.get('observer_writes', []):
            if private:
                for addr in range(lo, hi):
                    own.setdefault(addr, set()).add(eip)
            else:
                foreign.update(range(lo, hi))
    a_pages, b_pages, rules = dict(left[1]), dict(right[1]), []
    edited = {}
    for addr in sorted(own.keys()-foreign):
        p, i = addr & -PAGE, addr % PAGE
        a, b = a_pages.get(p, initial.get(p)), b_pages.get(p, initial.get(p))
        if a is None or b is None or a[i] == b[i]:
            continue
        rules.append(dict(rule='pinned observer private write', address=f'{addr:08x}',
                          original=a[i], ours=b[i], writers=[f'{eip:08x}' for eip in sorted(own[addr])],
                          observer_sha256=PIN))
        if p not in edited:
            edited[p] = bytearray(a), bytearray(b)
        edited[p][0][i] = edited[p][1][i] = 0
    for p, (a, b) in edited.items():
        a_pages[p], b_pages[p] = bytes(a), bytes(b)
    return a_pages, b_pages, rules
