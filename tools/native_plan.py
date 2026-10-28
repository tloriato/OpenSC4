"""Split an owner's functions into lane groups for the native port.

    python tools/native_plan.py <owner> [--lanes N] [--all]

Reads build-runs/coverage-owners.tsv (python tools/coverage.py --owners build-runs/coverage-owners.tsv) for the
owner's functions and build-runs/next-owners-reached.txt (one hex address per line, the functions the corpus
reaches) when present. Functions the corpus never reaches can't be captured, so they are left out unless --all.
Groups are runs of consecutive addresses (a class's related functions sit together), balanced by bytes.
Prints one line per group: bytes, count, then `address name` pairs.
"""
import argparse
import csv
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent


def names():
    out = {}
    with open(REPO / 'ghidra/export/windows/functions.tsv', encoding='utf-8') as f:
        for r in csv.DictReader(f, delimiter='\t'):
            out[int(r['address'], 16)] = r['name']
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('owner')
    ap.add_argument('--lanes', type=int, default=0, help='number of groups (default: about 8 KB of code each)')
    ap.add_argument('--all', action='store_true', help='include functions the corpus does not reach')
    a = ap.parse_args()
    reached_file = REPO / 'build-runs/next-owners-reached.txt'
    reached = {int(x, 16) for x in reached_file.read_text().split()} if reached_file.is_file() else None
    fns = []
    with open(REPO / 'build-runs/coverage-owners.tsv', encoding='utf-8') as f:
        for row in csv.reader(f, delimiter='\t'):
            try:
                va, size = int(row[0], 16), int(row[1])
            except (ValueError, IndexError):
                continue
            if row[2] == a.owner and (a.all or reached is None or va in reached):
                fns.append((va, size))
    fns.sort()
    total = sum(s for _, s in fns)
    lanes = a.lanes or max(1, round(total / 8192))
    target = total / lanes
    nm = names()
    groups, cur, acc = [], [], 0
    for va, size in fns:
        if cur and acc + size / 2 > target and len(groups) < lanes - 1:
            groups.append(cur)
            cur, acc = [], 0
        cur.append((va, size))
        acc += size
    if cur:
        groups.append(cur)
    print(f'{a.owner}: {len(fns)} functions, {total} bytes, {len(groups)} groups')
    for g in groups:
        print(f'{sum(s for _, s in g)}\t{len(g)}\t' + ', '.join(f'{va:08x} {nm.get(va, "?")} ({s})' for va, s in g))


if __name__ == '__main__':
    main()
