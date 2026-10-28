"""Marginal game-code entry coverage over the recorded golden corpus (stdlib).

Use the clean-room inventory emitted by coverage.py --owners (body bytes and
owners attributed through coverage-ranges.tsv / coverage-map.tsv). This tool
never reads the analysis export, nor guesses function sizes from address gaps.

python tools/reach_gain.py --owners build-runs/coverage-owners.tsv RUN.reach.tsv
Omit RUN.reach.tsv for the baseline alone; --gaps ranks the remaining subsystems.
"""
import argparse
import csv
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def main_checkout(root):
    common = subprocess.run(['git', '-C', str(root), 'rev-parse', '--git-common-dir'],
                            capture_output=True, text=True, check=True).stdout.strip()
    return (root / common).resolve().parent


def rows(path):
    with Path(path).open(encoding='utf-8-sig', newline='') as stream:
        yield from csv.DictReader((line for line in stream if line.strip()
                                   and not line.lstrip().startswith('#')), delimiter='\t')


def entered(path, warnings=None):
    result = set()
    for row in rows(path):
        if row.get('entries') is None:
            message = f'{path}: incomplete reach row {row["address"]!r}; retained complete rows only (lower bound)'
            if warnings is not None:
                warnings.append(message)
            else:
                print('reach_gain: ' + message, file=sys.stderr)
            continue
        if int(row['entries']) > 0 and not row.get('skipped', '').strip():
            result.add(int(row['address'], 16))
    return result


def inventory(path):
    result = {}
    for row in rows(path):
        address = int(row['address'], 16)
        if address in result:
            raise ValueError(f'duplicate inventory address: {address:08x}')
        size = int(row['bytes'])
        if size < 0:
            raise ValueError(f'negative body size: {address:08x}')
        result[address] = dict(address=f'{address:08x}', bytes=size,
                               owner=row['owner'], subsystem=row['subsystem'])
    if not result:
        raise ValueError(f'empty inventory: {path}')
    return {va: row for va, row in result.items()
            if not row['owner'].startswith('lib:')
            and not row['subsystem'].startswith('library ')}


def measure(candidates, references, functions):
    warnings = []
    baseline = set().union(*(entered(path, warnings) for path in references)) & functions.keys()
    combined = set(baseline)
    measured = []
    for path in candidates:
        hits = entered(path, warnings) & functions.keys()
        gain = hits - baseline
        groups = {}
        for va in sorted(gain):
            row = functions[va]
            group = groups.setdefault(row['subsystem'], dict(functions=0, bytes=0, entries=[]))
            group['functions'] += 1
            group['bytes'] += row['bytes']
            group['entries'].append(row)
        measured.append(dict(path=str(path), functions=len(gain),
                             bytes=sum(functions[va]['bytes'] for va in gain),
                             subsystems=groups))
        combined.update(hits)

    def total(addresses):
        return dict(functions=len(addresses), bytes=sum(functions[va]['bytes'] for va in addresses))

    gaps = {}
    for va in functions.keys() - combined:
        row = functions[va]
        group = gaps.setdefault(row['subsystem'], dict(functions=0, bytes=0))
        group['functions'] += 1
        group['bytes'] += row['bytes']
    return dict(reference_files=len(references), denominator=total(functions), warnings=warnings,
                baseline=total(baseline), candidates=measured,
                gain=total(combined - baseline), combined=total(combined), gaps=gaps)


def render(report, details=True, gaps=False):
    for warning in report['warnings']:
        print('reach_gain: ' + warning, file=sys.stderr)
    base, den = report['baseline'], report['denominator']
    print(f"current: {report['reference_files']} references; {base['functions']} functions; "
          f"{base['bytes']} / {den['bytes']} game-code bytes ({100 * base['bytes'] / den['bytes']:.3f}%)")
    for candidate in report['candidates']:
        print(f"candidate: {candidate['path']}; +{candidate['functions']} functions; +{candidate['bytes']} bytes")
        for name, group in sorted(candidate['subsystems'].items(), key=lambda item: (-item[1]['bytes'], item[0])):
            print(f"  {name}: +{group['functions']} functions; +{group['bytes']} bytes")
            if details:
                for row in group['entries']:
                    print(f"    {row['address']}\t{row['bytes']}\t{row['owner']}")
    gain = report['gain']
    print(f"union gain: +{gain['functions']} functions; +{gain['bytes']} bytes "
          '(overlap counted once; each candidate above is against references)')
    if gaps:
        print('remaining unreached game code (after candidate union):')
        for name, group in sorted(report['gaps'].items(), key=lambda item: (-item[1]['bytes'], item[0])):
            print(f"  {name}: {group['functions']} functions; {group['bytes']} bytes")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('reach', nargs='*', type=Path)
    parser.add_argument('--references', type=Path, help='golden store; default main checkout/build-runs/golden')
    parser.add_argument('--owners', type=Path, help='coverage.py --owners TSV; default main/build-runs/coverage-owners.tsv')
    parser.add_argument('--summary', action='store_true', help='omit individual function lines')
    parser.add_argument('--gaps', action='store_true', help='rank remaining unreached subsystems by bytes')
    parser.add_argument('--json', type=Path, help='also save the complete machine-readable report')
    args = parser.parse_args(argv)
    main_root = main_checkout(ROOT)
    references = args.references or main_root / 'build-runs/golden'
    owners = args.owners or main_root / 'build-runs/coverage-owners.tsv'
    try:
        paths = sorted(references.glob('*/reach.tsv'))
        if not paths:
            raise ValueError(f'no reference reach.tsv files in {references}')
        functions = inventory(owners)
        if not functions or not sum(row['bytes'] for row in functions.values()):
            raise ValueError('inventory has no game-code bytes')
        report = measure(args.reach, paths, functions)
        report.update(owners=str(owners), references=str(references))
        render(report, not args.summary, args.gaps)
        if args.json:
            args.json.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    except (OSError, ValueError, KeyError, csv.Error) as exc:
        parser.exit(2, f'reach_gain: {exc}\n')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
