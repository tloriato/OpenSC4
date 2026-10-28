#!/usr/bin/env python3
"""Our owners in replace mode against the fixed-schedule references (docs/NEXT_STEPS.md step 2).

  python tools/check.py record [--scenarios SEL] [--jobs 12]
      the pinned reference DLL's result for each selected scenario that has none (tools/golden.py record)
  python tools/check.py run --dll <worktree or DLL> --replace <keys> [--scenarios SEL] [--jobs 12] [--no-bisect]
      each scenario with those owners switched on: exact, or the first differing delivery, day, record and
      owner; on a difference with several owners, each owner alone, to name the ones that differ
  python tools/check.py run --dll <worktree or DLL> --replace each [--jobs 12]
      every owner alone on each scenario of its cover; per owner, how many are exact; the table is written to
      build-runs/golden/owner-check.tsv
  python tools/check.py credit --dll <worktree or DLL> --replace <keys or exact> [--jobs 12]
      every scenario (all) with those owners switched on together, written to build-runs/golden/credit.json, the
      input of Exact-replaced in tools/coverage.py; `exact` is the owners exact on their whole cover in
      owner-check.tsv (written with the same DLL) and the owners they need

SEL is `cover` (the default: for each owner, the scenarios of its greedy reach cover in
build-runs/golden/owner-scenarios.tsv, written by `tools/coverage.py --owner-scenarios`), `all`, a set from
engine/tests/scenarios/suite.tsv, or scenario names separated by commas. `--replace none` switches on no owner of
ours: the candidate must then match the references exactly (a drift check of the harness and the schedule).
Lua and clockless scenarios are not run. Exit code: 0 all exact, 1 a difference, 2 an error. Standard library only.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
import csv
import datetime as dt
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import subprocess
import sys

import golden_relabel
import golden as golden_api
from suite_coverage import reached

REPO = Path(__file__).resolve().parents[1]
GOLDEN = Path(__file__).with_name('golden.py')
STORE = REPO / 'build-runs/golden'
SECONDS = 900  # a run's watchdog; not part of a fixed-schedule key
# Owners that switch on only together with others (engine/plugin/FC4Subsystems.cpp, Resolve): "alone" includes them.
REQUIRES = {'mysims': ['advisors'], 'netrules': ['netmgr', 'nettool']}


def scenarios():
    rows = {}
    for name in ('suite.tsv', 'draw.tsv'):
        with (REPO / 'engine/tests/scenarios' / name).open(encoding='utf-8-sig', newline='') as stream:
            for row in csv.DictReader(stream, delimiter='\t'):
                if row['name'] in rows:
                    raise ValueError(f'duplicate scenario: {row["name"]}')
                row['kind'] = 'draw' if name == 'draw.tsv' else 'save'
                rows[row['name']] = row
    return rows


def replacement_code():
    # Load our sibling explicitly: the installed pytest coverage package has the same module name.
    path = Path(__file__).with_name('coverage.py')
    spec = importlib.util.spec_from_file_location('fc4_coverage', path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.replacement_code()


def draw_references(rows):
    """Resolve current input keys, never infer draw reach from the save-only census or an old pin."""
    references, code = {}, None
    for name, row in rows.items():
        if row.get('kind') != 'draw':
            continue
        status, out = golden('key', row)
        match = re.search(r'^key: ([0-9a-f]{64})$', out, re.M)
        key = match[1] if match else None
        reference = {'reference': key, 'keys': None}
        if status == 0 and key and (STORE / key).is_dir():
            metadata = golden_api._baseline_intact(STORE / key)
            if 'reach.tsv' in metadata['artifacts']:
                if code is None:
                    code = replacement_code()
                entries, _ = reached(STORE / key / 'reach.tsv')
                reference['keys'] = sorted(k for k, addresses in code.items() if entries & addresses)
        references[name] = reference
    return references


def draw_names(keys, references):
    # Missing evidence fails closed: run the row, which reports an error rather than granting exactness.
    switched = {dependency for key in keys for dependency in alone(key)}
    return {name for name, entry in references.items()
            if not keys or entry['keys'] is None or switched & set(entry['keys'])}


def owner_covers():
    path = STORE / 'owner-scenarios.tsv'
    if not path.is_file():
        raise ValueError(f'no {path}; run: python tools/coverage.py --owner-scenarios {path}')
    covers = {}
    with path.open(encoding='utf-8', newline='') as stream:
        for row in csv.DictReader((line for line in stream if not line.startswith('#')), delimiter='\t'):
            covers.setdefault(row['key'], []).append(row['scenario'])
    return covers


def select(text, keys, rows):
    if text == 'cover':
        covers = owner_covers()
        names = {name for key in (keys or covers) for name in covers.get(key, [])}
    elif text == 'all':
        names = set(rows)
    elif any(text in row['sets'].split(',') for row in rows.values()):
        names = {name for name, row in rows.items() if text in row['sets'].split(',')}
    else:
        names = set(filter(None, text.split(',')))
    unknown = names - set(rows)
    if unknown:
        raise ValueError(f'unknown scenarios: {sorted(unknown)}')
    # Not run: Lua scripts, and clockless cities (no day comes, so no run reaches a dated save).
    skipped = {name for name in names if rows[name]['script'].lower().endswith('.lua')
               or 'clockless' in rows[name]['sets'].split(',')}
    return sorted(names - skipped), sorted(skipped)


def golden(command, row, *extra):
    args = [sys.executable, str(GOLDEN), command, '--schedule', 'fixed', '--seconds', str(SECONDS),
            '--region', row['region'], '--city', row['city'], '--until', row['end'], '--store', str(STORE)]
    if row.get('kind') == 'draw':
        if row['draw_capture'] != '1' or row['minimize'] != '0' or row['draw_capture_scenarios'] != '1':
            raise ValueError(f'invalid draw scenario controls: {row["name"]}')
        if not 30 <= int(row['seconds']) < 180:
            raise ValueError(f'draw scenario must be under 3 minutes: {row["name"]}')
        args += ['--draw-capture', '--draw-frames', row['draw_capture_frames'],
                 '--draw-warmup', row['draw_capture_warmup'], '--seconds', row['seconds']]
    if row['script'] != '-':
        args += ['--script', row['script']]
    process = subprocess.run(args + list(extra), cwd=REPO, capture_output=True, text=True)
    return process.returncode, process.stdout + process.stderr


def record(args):
    rows = scenarios()
    names, lua = select(args.scenarios, None, rows)
    if args.scenarios == 'cover':
        names = sorted(set(names) | {n for n, r in rows.items() if r.get('kind') == 'draw'})
    print(f'{len(names)} scenarios' + (f'; not run (Lua or clockless): {", ".join(lua)}' if lua else ''), flush=True)
    failed = 0

    def one(name):
        code, out = golden('record', rows[name], *(['--rerun'] if args.rerun else []))
        state = ('error: ' + (re.findall(r'golden: (.*)', out) or [out.strip()[-200:]])[-1] if code else
                 'cached' if re.search(r'^cached:', out, re.M) else 'recorded')
        if rows[name].get('kind') == 'draw':
            detail = '; '.join(line for line in out.splitlines()
                              if line.startswith(('key:', 'recorded:', 'reproduced:', 'nondeterministic:',
                                                  'cached:', 'first DRAW', 'baseline repeatability:')))
            state = '; '.join(filter(None, [detail, state if code else ''])) or state
        return name, code, state

    with ThreadPoolExecutor(args.jobs) as pool:
        for name, code, state in pool.map(one, names):
            failed += code != 0
            print(f'{name}: {state}', flush=True)
    print(f'{len(names) - failed} of {len(names)} scenarios have a reference')
    if any(rows[n].get('kind') == 'draw' for n in names):
        golden_api.write_json(STORE / 'draw-references.json', draw_references(rows))
    return 2 if failed else 0


def record_owners():
    """Save record type (a class ID) -> (replace key, class name), from the subsystem registry."""
    plugin = REPO / 'engine/plugin'
    constants = {}
    for header in plugin.glob('*.h'):
        for name, value in re.findall(r'\b(kFC4_CLSID_\w+)\s*=\s*(0x[0-9A-Fa-f]+)', header.read_text(encoding='utf-8')):
            constants[name] = int(value, 16)
    text = (plugin / 'FC4Subsystems.cpp').read_text(encoding='utf-8')
    arrays = {name: re.findall(r'\{\s*(0x[0-9A-Fa-f]+|kFC4_CLSID_\w+)\s*,\s*"(\w+)"', body)
              for name, body in re.findall(r'const ClassEntry (k\w+Classes)\[\] = \{(.*?)\n\};', text, re.S)}
    owners = {}
    for key, array in re.findall(r'\{"([a-z][a-z0-9_]*)",\s*[^,]+,\s*(k\w+Classes)\b', text):
        for clsid, name in arrays.get(array, []):
            value = constants.get(clsid) if clsid.startswith('kFC4') else int(clsid, 16)
            if value is not None:
                owners.setdefault(value, (key, name))
    return owners


def digests(log):
    """The schedule's per-delivery digest lines: (julian, delivery) -> (rand, generators, target vtable, message)."""
    found = {}
    for match in re.finditer(r'schedule: digest (\d+)\.(\d+) rand (\S+) generators (\S+) heap \S+ \S+ target vt (\S+) '
                             r'msg (\S+)', Path(log).read_text(encoding='utf-8-sig', errors='replace')):
        found[(int(match[1]), int(match[2]))] = match.group(3, 4, 5, 6)
    return found


def first_delivery(reference_log, candidate_log, replaced):
    """The first delivery whose rand() state differs, or (with no owner replaced) its file-local generators' digest.
    A replaced owner draws from our core's copy of its generators, so the original's copy stays behind by design;
    rand() is shared with our cores, so a difference there is behaviour."""
    expected, actual = digests(reference_log), digests(candidate_log)
    fields = 1 if replaced else 2
    for key in sorted(expected.keys() | actual.keys()):
        a, b = expected.get(key), actual.get(key)
        if a is None or b is None or a[:fields] != b[:fields]:
            day = str(dt.date(2000, 1, 1) + dt.timedelta(days=key[0] - 2451545)) if key[0] > 2451545 else 'load'
            return {'day': day, 'delivery': key[1], 'expected': a, 'actual': b}
    return None


def compare(row, dll, keys):
    code, out = golden('compare', row, '--dll', str(dll), '--replace', ','.join(keys) or 'simulator')
    path = re.search(r'^comparison: (.+)$', out, re.M)
    if code not in (0, 1, 3) or not path:
        error = {'verdict': 'error', 'error': (re.findall(r'golden: (.*)', out) or [out.strip()[-300:]])[-1]}
        # golden prints the key before the run: an errored run still names its reference (coverage reads its reach)
        key = re.search(r'^key: ([0-9a-f]{64})$', out, re.M)
        if key and (STORE / key[1]).is_dir():
            error['key'] = key[1]
        return error
    report = json.loads(Path(path[1]).read_text(encoding='utf-8'))
    if code == 3:
        report['verdict'] = 'nondeterministic'
    return report


def describe(name, report, owners, reference, replaced):
    if report['verdict'] == 'error':
        return f'{name}: ERROR {report["error"]}'
    if report['verdict'] == 'exact':
        return f'{name}: exact'
    parts = [f'{name}: {report["verdict"]}']
    if 'draw' in report:
        difference = report['draw']['difference']
        parts.append('first DRAW ' + golden_api.draw_description(difference) if difference else
                     'candidate matches one sample; original draw streams vary')
        parts.append(f'run {report["run"]}')
        return '; '.join(parts)
    delivery = first_delivery(reference / 'game.log', Path(report['run']) / 'game.log', replaced)
    if delivery:
        target = delivery['expected'][2] if delivery['expected'] else '-'
        parts.append(f'first delivery {delivery["day"]} #{delivery["delivery"]} (reference target vt {target})')
    first = report.get('first_digest_difference')
    if first:
        parts.append(f'first DIGEST {first["date"]} ({", ".join(first["fields"])})')
    records = report['save']['records']
    if records:
        first, note = records[0]['tgi'], ''
        saves = reference / 'end.sc4', Path(report['run']) / 'end.sc4'
        if all(path.exists() for path in saves):
            labels = golden_relabel.compare(*saves)
            content = [tgi for tgi, verdict in labels['records'].items() if verdict != 'relabel']
            relabelled = len(labels['records']) - len(content)
            if relabelled:
                shift = labels['first_shift']
                note = f' ({relabelled} only relabelled: object IDs shift from {shift[0]}, class {shift[1]:08X})'
                if content:
                    first = content[0]
        kind = int(first.split('/')[0], 16)
        key, cls = owners.get(kind, ('-', '-'))
        parts.append(f'{report["save"]["different_records"]} records differ{note}, first '
                     f'{"content " if note else ""}{first} (owner {key}, {cls})')
    parts.append(f'run {report["run"]}')
    return '; '.join(parts)


def alone(key):
    return [key] + REQUIRES.get(key, [])


def sha256(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def exact_owners(dll):
    """The owners exact on every run of their cover in owner-check.tsv (which must come from this DLL), and the
    owners they need."""
    path = STORE / 'owner-check.tsv'
    lines = path.read_text(encoding='utf-8').splitlines() if path.is_file() else []
    found = re.search(r'sha256 ([0-9a-f]{64})', lines[0]) if lines else None
    if not found or found[1] != sha256(dll):
        raise ValueError(f'{path} is not from this DLL; run: python tools/check.py run --dll <it> --replace each')
    flags, checked = {}, {}
    for row in csv.DictReader(lines[1:], delimiter='\t'):
        flags.setdefault(row['key'], []).append(row['verdict'] == 'exact')
        checked.setdefault(row['key'], {})[row['scenario']] = row.get('reference')
    references = draw_references(scenarios())
    eligible = {key for key, f in flags.items() if all(f) and all(
        checked[key].get(name) == references[name]['reference'] and references[name]['reference'] is not None
        for name in draw_names(alone(key), references))}
    return sorted({k for key in eligible if set(alone(key)) <= eligible for k in alone(key)})


def run_each(args, rows, dll):
    covers = owner_covers()
    skipped = set(select('all', None, rows)[1])
    references = draw_references(rows)
    keys = sorted(set(covers) | {k for r in references.values() for k in (r['keys'] or [])})
    if args.keys:
        requested = set(filter(None, args.keys.split(',')))
        unknown = requested - set(golden_api.subsystem_keys(REPO))
        if unknown:
            raise ValueError(f'unknown replacement keys: {sorted(unknown)}')
        keys = sorted(requested)
    selected = set(select(args.scenarios, None, rows)[0]) if args.scenarios != 'cover' else set(rows)
    jobs = [(key, name) for key in keys if key != 'simulator'
            for name in sorted((set(covers.get(key, [])) | draw_names(alone(key), references)) & selected - skipped)]
    print(f'{len(jobs)} runs: {len(keys)} owners, each alone on its save cover and draw reach', flush=True)
    owners = record_owners()
    with ThreadPoolExecutor(args.jobs) as pool:
        reports = list(pool.map(lambda job: compare(rows[job[1]], dll, alone(job[0])), jobs))
    table, exact = [], {}
    for (key, name), report in zip(jobs, reports):
        line = describe(f'{key} on {name}', report, owners, STORE / report.get('key', '-'), True)
        print(line, flush=True)
        table.append((key, name, report['verdict'], line.split(': ', 1)[1], report.get('key', '')))
        exact.setdefault(key, []).append(report['verdict'] == 'exact')
    draw_only = args.scenarios != 'cover' and all(rows[n].get('kind') == 'draw' for n in selected)
    path = STORE / ('draw-owner-check.tsv' if draw_only else 'owner-check.tsv')
    with path.open('w', encoding='utf-8', newline='\n') as stream:
        stream.write(f'# {dll} sha256 {sha256(dll)} ({len(jobs)} runs)\nkey\tscenario\tverdict\tdetail\treference\n')
        stream.writelines('\t'.join(row) + '\n' for row in table)
    whole = sorted(key for key, flags in exact.items() if all(flags))
    print(f'owners exact on their whole cover: {len(whole)} of {len(exact)}' + (f': {", ".join(whole)}' if whole else ''))
    print(f'table: {path}')
    errors = sum(report['verdict'] == 'error' for report in reports)
    return 2 if errors else 0 if len(whole) == len(exact) else 1


def run(args):
    rows = scenarios()
    keys = [] if args.replace == 'none' else sorted(set(filter(None, re.split(r'[,\s]+', args.replace))))
    dll = args.dll / 'engine/build-x86/FreeCity4.dll' if args.dll.is_dir() else args.dll
    if args.replace == 'each':
        return run_each(args, rows, dll)
    names, lua = select(args.scenarios, keys, rows)
    names = sorted(set(names) | draw_names(keys, draw_references(rows)))
    print(f'{len(names)} scenarios, owners {", ".join(keys) or "none"}' +
          (f'; not run (Lua or clockless): {", ".join(lua)}' if lua else ''), flush=True)
    owners = record_owners()
    with ThreadPoolExecutor(args.jobs) as pool:
        reports = dict(zip(names, pool.map(lambda n: compare(rows[n], dll, keys), names)))
    for name in names:
        entry = STORE / reports[name].get('key', '-')
        print(describe(name, reports[name], owners, entry, bool(keys)), flush=True)
    failing = [n for n in names if reports[n]['verdict'] != 'exact']
    errors = [n for n in failing if reports[n]['verdict'] == 'error']
    print(f'exact: {len(names) - len(failing)} of {len(names)}')
    if failing and len(keys) > 1 and not args.no_bisect:
        culprits = [n for n in failing if n not in errors][:2] or failing[:1]
        print(f'each owner alone on {", ".join(culprits)}:', flush=True)
        jobs = [(n, k) for n in culprits for k in keys]
        with ThreadPoolExecutor(args.jobs) as pool:
            results = list(pool.map(lambda job: compare(rows[job[0]], dll, alone(job[1])), jobs))
        for (name, key), report in zip(jobs, results):
            print(f'  {key} on {name}: {report["verdict"]}' + (f' ({report["error"]})' if 'error' in report else ''))
    return 2 if errors else 1 if failing else 0


def credit(args):
    rows = scenarios()
    dll = args.dll / 'engine/build-x86/FreeCity4.dll' if args.dll.is_dir() else args.dll
    keys = (exact_owners(dll) if args.replace == 'exact' else
            sorted(set(filter(None, re.split(r'[,\s]+', args.replace)))))
    names, _ = select('all', None, rows)
    references = draw_references(rows)
    names = sorted({n for n in names if rows[n].get('kind') != 'draw'} | draw_names(keys, references))
    print(f'{len(names)} scenarios, owners {", ".join(keys)}', flush=True)
    owners = record_owners()
    with ThreadPoolExecutor(args.jobs) as pool:
        reports = dict(zip(names, pool.map(lambda n: compare(rows[n], dll, keys), names)))
    table = {}
    for name in names:
        report = reports[name]
        line = describe(name, report, owners, STORE / report.get('key', '-'), True)
        print(line, flush=True)
        table[name] = {'verdict': report['verdict'], 'reference': report.get('key'), 'run': report.get('run'),
                       'detail': line.split(': ', 1)[1], 'kind': rows[name].get('kind', 'save')}
    path = STORE / 'credit.json'
    path.write_text(json.dumps({'dll': str(dll), 'dll_sha256': sha256(dll), 'keys': keys,
                                'written': dt.datetime.now().isoformat(timespec='seconds'), 'scenarios': table},
                               indent=1), encoding='utf-8')
    exact = sum(entry['verdict'] == 'exact' for entry in table.values())
    print(f'exact: {exact} of {len(names)}; written to {path} (tools/coverage.py reads it)')
    return 2 if any(entry['verdict'] == 'error' for entry in table.values()) else 0 if exact == len(names) else 1


def main(argv=None):
    global STORE
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = p.add_subparsers(dest='command', required=True)
    rec = commands.add_parser('record')
    chk = commands.add_parser('run')
    cre = commands.add_parser('credit')
    for sub in (chk, cre):
        sub.add_argument('--dll', type=Path, required=True)
        sub.add_argument('--replace', required=True)
    chk.add_argument('--no-bisect', action='store_true')
    chk.add_argument('--keys', help='with --replace each, measure only these keys (comma separated)')
    rec.add_argument('--rerun', action='store_true', help='independently audit existing references')
    for sub in (rec, chk):
        sub.add_argument('--scenarios', default='cover')
    for sub in (rec, chk, cre):
        sub.add_argument('--jobs', type=int, default=12)
        sub.add_argument('--store', type=Path, default=REPO / 'build-runs/golden')
    args = p.parse_args(argv)
    STORE = args.store.resolve()
    try:
        return {'record': record, 'run': run, 'credit': credit}[args.command](args)
    except (ValueError, OSError) as exc:
        print('check: ' + str(exc), file=sys.stderr)
        return 2


if __name__ == '__main__':
    sys.exit(main())
