"""Run saved-city scenarios through game_queue; Python standard library only.

Examples: python tools/suite.py run --set smoke --profile shadows --tag smoke1
          python tools/suite.py report smoke1 [--against smoke2]
The queue must include this checkout in its roots. Requests opt into isolated
parallel slots. No file in Documents is written by this tool.
The known original-game null access at SimCity 4.exe+0x1D0F94 after the target
and an accepted quit is clean, retaining its raw verdict and exception evidence.
Other quit exceptions are clean-exit-crash and do not pass successful().
"""
import argparse
import csv
from datetime import date, datetime, timezone
import hashlib
import io
import itertools
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time
import uuid

ROOT = Path(__file__).resolve().parents[1]
COLUMNS = ('scenario profile verdict end_reached days_per_second actions_ok '
           'actions_failed shadow_lines shadow_exact_lines worst_shadow '
           'compare_exact compare_different reach_functions log dll_sha256 exception_address').split()
KEY_COLUMNS = ['key', 'scenarios_run', 'scenarios_exact', 'first_mismatch']
SCENARIO_COLUMNS = ['name', 'region', 'city', 'script', 'end', 'sets', 'notes']
SHADOW = re.compile(r'^([a-z0-9_]+(?:[ -][a-z0-9_]+)*?)(?: shadow\b|-shadow:)')
ENVELOPE = re.compile(r'^.*?\[pid \d+ tid \d+\]\s*')
NAME = re.compile(r'^[A-Za-z0-9][A-Za-z0-9._-]{0,39}$')


def game_date(value):
    if not re.fullmatch(r'\d{4}-\d\d-\d\d', value):
        raise ValueError(f'expected YYYY-MM-DD: {value}')
    date.fromisoformat(value)
    return value


def folder_name(value):
    if (not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9 ._()'-]*", value)
            or value.endswith(('.', ' '))):
        raise ValueError(f'expected one region/city name: {value!r}')
    return value


def inside(root, path):
    resolved = path.resolve()
    if not resolved.is_relative_to(root.resolve()):
        raise ValueError(f'path outside checkout: {path}')
    return resolved


def read_scenarios(root, selected):
    path = root / 'engine/tests/scenarios/suite.tsv'
    with path.open(encoding='utf-8-sig', newline='') as stream:
        reader = csv.DictReader(stream, delimiter='\t')
        if reader.fieldnames != SCENARIO_COLUMNS:
            raise ValueError(f'{path}: expected columns {SCENARIO_COLUMNS}')
        rows = list(reader)
    seen = set()
    for number, row in enumerate(rows, 2):
        if None in row or any(v is None for v in row.values()):
            raise ValueError(f'{path}:{number}: wrong number of cells')
        if not NAME.fullmatch(row['name']) or row['name'] in seen:
            raise ValueError(f'{path}:{number}: invalid/duplicate scenario name')
        seen.add(row['name'])
        folder_name(row['region']); folder_name(row['city']); game_date(row['end'])
        if row['script'] != '-':
            script_file(root, row['script'])
    result = [r for r in rows if selected == 'all' or selected in r['sets'].split(',')]
    if not result:
        raise ValueError(f'no scenarios in set {selected}')
    return result


def script_file(root, value):
    path = inside(root, root / value)
    if path.suffix.lower() not in ('.script', '.lua') or not path.is_file():
        raise ValueError(f'no .script or .lua file: {path}')
    return path


def profile_ini(root, profile):
    """Read the house profiles rather than maintaining another replacement list."""
    if profile == 'plain':
        # Simulator and budget are on by default in the plugin. Also override
        # FC4_ENABLE so a native control cannot inherit opt-in replacements.
        return {'disable': 'simulator,budget', 'enable': 'none'}
    source = (root / 'tools/regress.ps1').read_text(encoding='utf-8-sig')
    if profile == 'replace':
        match = re.search(r'\[string\]\$Replace\s*=\s*"([^"]+)"', source)
        if not match:
            raise ValueError('cannot read regress.ps1 default -Replace list')
        return {'enable': match[1]}
    match = re.search(r'^\s*shadows\s*=\s*"([^"\r\n]+)"\s*$', source, re.M)
    if not match or '$' in match[1]:
        raise ValueError('cannot read regress.ps1 shadows profile')
    return dict(line.split(' = ', 1) for line in match[1].split('`n'))


def main_checkout(root):
    result = subprocess.run(['git', '-C', str(root), 'rev-parse', '--git-common-dir'],
                            capture_output=True, text=True, check=True)
    return (root / result.stdout.strip()).resolve().parent


def resolve_dll(root, supplied):
    path = Path(supplied).resolve() if supplied else main_checkout(root)
    if path.is_dir():
        path = path / 'engine/build-x86/FreeCity4.dll'
    if path.suffix.lower() != '.dll' or not path.is_file():
        raise ValueError(f'no DLL: {path}')
    return path


def extras(values):
    result = {}
    for value in values:
        match = re.fullmatch(r'\s*([a-z][a-z0-9_]*)\s*=\s*([^\r\n\x00]+)', value)
        if not match or any(ord(c) < 32 for c in value):
            raise ValueError(f'expected a single key = value: {value!r}')
        key, setting = match[1], match[2].strip()
        if key in ('label', 'dll', 'region', 'city', 'end_date', 'autoload_city', 'script', 'lua_script', 'fixture_city', 'close_dialogs', 'prepared_city', 'export_city'):
            raise ValueError(f'{key} is selected by the scenario/runner')
        result[key] = setting
    return result


def label_for(tag, name, profile):
    label = f'suite-{tag}-{name}-{profile}'
    if len(label) > 64:
        label = label[:55] + '-' + hashlib.sha256(label.encode()).hexdigest()[:8]
    return label


def city_path(row):
    return (Path.home() / 'Documents/SimCity 4/Regions' / row['region'] /
            ('City - ' + row['city'] + '.sc4'))


def city_limits(row, script=None):
    """Bound slow native work using tile hints, save bytes and scripted duration.

    These are explicit suite INI overrides; plugin defaults remain off. File
    size is a conservative fallback when the catalogue has no tile-size hint.
    """
    source = city_path(row)
    size = source.stat().st_size if source.is_file() else 0
    text = script.read_text(encoding='utf-8-sig') if script else ''
    hints = row['notes'].lower() + '\n' + text[:200]
    scale = (4 if re.search(r'\blarge\b|\b256\b', hints) or size >= 4_000_000 else
             2 if re.search(r'\bmedium\b|\b128\b', hints) or size >= 1_000_000 else 1)
    dates = re.findall(r'^(\d{4}-\d\d-\d\d)\s', text, re.M)
    days = max(0, (date.fromisoformat(row['end']) - date.fromisoformat(min(dates))).days) if dates else 0
    seconds = max(750, min(7200, 120 + ((days + 29) // 30) * 45 * scale))
    grace = 30000 * scale
    return dict(seconds=str(seconds), timeout=str(seconds + 150 * scale + grace // 1000)), {
        'autotest_stall_grace_ms': str(grace)}


def make_batch(root, scenarios, profile, dll, tag, extra=()):
    if not NAME.fullmatch(tag):
        raise ValueError('tag: use 1..40 letters, digits, dots, underscores or dashes')
    base_ini = profile_ini(root, profile)
    changes = extras(extra)
    source_dll = resolve_dll(root, dll)
    if changes.get('reach') == '1' and b'reach prepared functions=' not in source_dll.read_bytes():
        raise ValueError(f'{source_dll}: DLL lacks reach tracer; supply a current build with --dll')
    directory = root / 'build-runs/suite' / tag
    if directory.exists():
        raise ValueError(f'tag already exists: {directory}; use report or a new tag')
    queue = root / 'build-runs/game-queue'
    results = root / 'build-runs/game-results'
    prepared = []
    for row in scenarios:
        label = label_for(tag, row['name'], profile)
        for path in (queue / f'{label}.req', results / f'{label}.running', results / f'{label}.verdict'):
            if path.exists():
                raise ValueError(f'request/result already exists: {path}')
        script = script_file(root, row['script']) if row['script'] != '-' else None
        prepared.append((row, label, script))
    directory.mkdir(parents=True)
    staged = directory / 'dll/FreeCity4.dll'
    staged.parent.mkdir()
    shutil.copy2(source_dll, staged)
    if source_dll.with_suffix('.pdb').is_file():
        shutil.copy2(source_dll.with_suffix('.pdb'), staged.with_suffix('.pdb'))
    requests = []
    for row, label, source_script in prepared:
        request = dict(label=label, dll=staged.relative_to(root).as_posix(),
                       region=row['region'], city=row['city'], seconds='750',
                       timeout='900', end_date=row['end'], turbo='1', parallel='1')
        ini = dict(base_ini, autotest_unpause='2', autotest_end='quit', popup_dialogs='0', close_dialogs='1')
        limits, allowances = city_limits(row, source_script)
        request.update(limits)
        ini.update(allowances)
        for field, key in (('_prepared_city', 'prepared_city'), ('_export_city', 'export_city')):
            if row.get(field):
                ini[key] = row[field]
        if load_only(row):
            # PostCityInit shadows and reach arming can still be running well
            # after LoadCity returned. A twelve-second fallback killed these
            # god-mode runs before they could emit their reach summary.
            load_seconds = int(allowances['autotest_stall_grace_ms']) // 1000 * 2
            request.update(seconds=str(load_seconds), timeout=str(load_seconds + 180), turbo='0')
            del request['end_date']
            # Either archive shadow starts the background whole-install scan.
            # Autotest waits for that scan before RequestQuit. These short
            # god-mode load probes cover city/load shadows; advancing scenarios
            # retain both archive shadows and their full evidence.
            for key in ('dbpf_shadow', 'dbpf_bounds_shadow'):
                if key in ini:
                    ini[key] = '0'
        if source_script:
            script = directory / 'scripts' / (row['name'] + source_script.suffix)
            script.parent.mkdir(exist_ok=True)
            shutil.copy2(source_script, script)
            if script.suffix.lower() == '.lua':
                ini['lua_script'] = script.relative_to(root).as_posix()
                ini['lua_extensions'] = '1'  # opt-in log backend, only for Lua scenarios
                ini['autotest_unpause'] = '0'  # the Lua script owns pause/resume during construction
                setup = source_script.with_suffix('.script')
                if setup.is_file():
                    setup_copy = script.with_suffix('.script')
                    shutil.copy2(setup, setup_copy)
                    ini['script'] = setup_copy.relative_to(root).as_posix()
            else:
                ini['script'] = script.relative_to(root).as_posix()
        if ini.get('script'):
            # Inspect the staged dated script (also a Lua setup companion).
            # Native full SaveCity can exceed the ordinary ten-second watchdog;
            # the plugin applies this bounded opt-in only inside full saves.
            dated = (root / ini['script']).read_text(encoding='utf-8-sig')
            if re.search(r'(?m)^\s*\d{4}-\d\d-\d\d\s+save(?:\s|$)', dated):
                ini['script_save_grace_ms'] = str(max(60000, int(allowances['autotest_stall_grace_ms'])))
        for key, value in changes.items():
            (request if key in ('seconds', 'timeout', 'turbo', 'turbo_from', 'minimize', 'priority', 'parallel') else ini)[key] = value
        if ini.get('reach') == '1':
            # A cached/native DLL can carry another worktree's source prefix.
            # Entry contracts belong to the requesting checkout, not that path.
            ini.setdefault('reach_contracts', (root / 'docs/abi/functions.tsv').as_posix())
            ini['reach_output'] = (directory / (row['name'] + '.reach.tsv')).as_posix()
        text = '\n'.join(f'{k} = {v}' for k, v in request.items()) + '\n[ini]\n'
        text += '\n'.join(f'{k} = {v}' for k, v in ini.items()) + '\n'
        (directory / f'{row["name"]}.req').write_text(text, encoding='utf-8')
        requests.append(dict(scenario=row, label=label, request=text))
    manifest = dict(tag=tag, profile=profile, dll=str(source_dll),
                    dll_sha256=hashlib.sha256(staged.read_bytes()).hexdigest(),
                    extra=list(extra), requests=requests,
                    shadow_keys=[k[:-7] for k in dict(base_ini, **changes) if k.endswith('_shadow')])
    (directory / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
    return directory, manifest


def mismatch(body):
    totals = re.findall(r'; mismatches=(\d+)', body)
    if totals:
        return int(totals[-1]) != 0
    return bool(re.search(r'mismatch(?:es|ed)?[= ][1-9]\d*|differences [1-9]\d*|\b(?:diff|different|refusal|rejected)=[1-9]\d*|\b[1-9]\d* (?:differ|rejected)\b|\bINCOMPLETE\b', body))


def resynced(body):
    return bool(re.search(r'\bresynced:\s*(?!none\b|0\b)\S|\bresyncs?\s*[=:]\s*[1-9]\d*\b', body))


def shadow_sample(body, key):
    if key == 'formats':
        return (not re.search(r'shadow: (?:store )?total:', body)
                and bool(re.search(r'\b[1-9]\d* compared,', body)))
    measured = re.search(r'mismatch(?:es|ed)?[= ]\d|: clean\b', body)
    dated = re.search(r'\b\d{4}-\d\d-\d\d\b|\bjulian=\d+', body)
    return bool(measured and (dated or '-shadow:' in body))


def aggregate_counts(body):
    match = re.search(r'(\d+) (?:month )?lines, (\d+) with mismatches=0', body)
    if match:
        return int(match[1]), int(match[2])
    match = re.search(r'(?:months|lines)=(\d+) clean=(\d+)', body)
    return (int(match[1]), int(match[2])) if match else None


def load_only(row):
    return bool(re.search(r'\bload-only\b', row.get('notes', ''), re.I))


def parse_log(text, end, verdict_text='', *, only_load=False):
    """Count comparison evidence, excluding switches, empty files and duplicate totals.

    Keep *all* shadow summaries verbatim in the JSON evidence. Aggregate-only
    keys supply counts if there were no sample lines, never additional counts.
    """
    result = dict(verdict='missing', end_reached='0', days_per_second='-',
                  actions_ok=0, actions_failed=0, actions_expected=0, actions_pending=0,
                  shadow_lines=0, shadow_exact_lines=0, worst_shadow='-',
                  compare_exact=0, compare_different=0, reach_functions='-',
                  per_key={}, action_lines=[], shadow_records=[], shadow_summaries=[], shadow_payloads=[],
                  inplace_lines=[], reach_lines=[], memory_lines=[], invalid_reason='',
                  only_load=only_load, real_seconds=None, wall_seconds=None,
                  load_seconds=None, seconds_per_month=None, reach_windows=[],
                  raw_verdict='missing', exception_address='-', exceptions=[], known_quit_faults=0,
                  calendar_stall=None)
    keys, aggregate, actions = {}, {}, {}
    verdict_line = ''
    reached_date = ''
    parse_errors = 0
    load_started = None
    attached = None
    verdict_at = None
    end_line = None
    quit_line = None
    quit_dispatch_failed = False
    emergency_first = emergency_last = None
    emergency_tick = 0
    for line_no, line in enumerate(io.StringIO(text), 1):
        body = ENVELOPE.sub('', line.rstrip('\r\n'))
        try:
            timestamp = datetime.fromisoformat(line[:23])
        except ValueError:
            timestamp = None
        status = re.search(r'status @tick (\d+): julian=(\d+).*?\bpause=(\d+).*?\bemergency=(\d+)', body)
        if status and timestamp:
            tick = int(status[1])
            if status[3] == status[4] == '1':
                if not emergency_first or status[2] != emergency_first[1] or tick <= emergency_tick:
                    emergency_first = (timestamp, status[2])
                emergency_tick = tick
                emergency_last = (timestamp, line_no, body)
            else:
                emergency_first = emergency_last = None
        if 'DLL_PROCESS_ATTACH:' in body:
            attached = timestamp
        if 'autoload: calling LoadCity(' in body:
            load_started = timestamp
        if 'autoload: LoadCity(' in body and '-> true' in body and load_started and timestamp:
            result['load_seconds'] = (timestamp - load_started).total_seconds()
        if re.search(r'\bmemory\b|address.space|largest.free|out.of.memory', body, re.I):
            result['memory_lines'].append(dict(line=line_no, text=body))
        if 'AUTOTEST VERDICT:' in body:
            verdict_line = body
            verdict_at = timestamp
        window = re.match(r'reach window (\d+) date=(\d{4}-\d\d-\d\d) new=(\d+) total=(\d+)', body)
        if window:
            result['reach_windows'].append(dict(window=int(window[1]), date=window[2], new=int(window[3]), total=int(window[4])))
        reached = re.search(r'end date (\d{4}-\d\d-\d\d) reached', body)
        if reached and reached[1] == end:
            result['end_reached'] = '1'
            end_line = line_no
        if end_line and re.match(
                r'autotest: (?:cISC4App::RequestQuit\([^)]*\)|'
                r'native quit command 0xC7AA61C6 queued) -> 1\b', body):
            quit_line = line_no
        if re.match(r'autotest: native quit command 0xC7AA61C6 queued -> 0\b', body):
            quit_dispatch_failed = True
        if body.startswith('first-chance exception '):
            address = re.search(r'\bat (0x[0-9a-fA-F]+)\b', body)
            if address:
                location = re.search(r'\bin (.+?) \(report ', body)
                result['exceptions'].append(dict(
                    line=line_no, address=address[1],
                    location=location[1] if location else '-', text=body,
                    after_end=end_line is not None, after_quit=quit_line is not None))
                if result['exception_address'] == '-':
                    result['exception_address'] = address[1]
        if body.startswith('script:'):
            loaded = re.search(r': (\d+) action\(s\)', body)
            if loaded:
                result['actions_expected'] = int(loaded[1])
            if 'ERROR line ' in body:
                parse_errors += 1
            if 'REFUSED ' in body:
                parse_errors = max(parse_errors, 1)
            action = re.search(r'^script: line (\d+) .*? -> (.*)', body)
            if action:
                failed = bool(re.search(r'\bok=0\b|\berror=[1-9]\d*|\b(?:skipped|failed|refused|exception)\b|\baccepted=0\b|\bno (?:candidate|city|manager)\b', action[2], re.I))
                actions[action[1]] = (not failed, body)
        if 'inplace summary ' in body:
            result['inplace_lines'].append(dict(line=line_no, text=body))
            fields = dict(re.findall(r'(\w+)=([^ ]+)', body))
            result['compare_exact'] += int(fields.get('exact', 0))
            result['compare_different'] += int(fields.get('different', 0))
        if body.startswith('reach summary '):
            result['reach_lines'].append(dict(line=line_no, text=body))
            fields = dict(re.findall(r'(\w+)=(\d+)', body))
            result['reach_functions'] = fields.get('reached', '-')
        match = SHADOW.match(body)
        if not match:
            continue
        key = match[1].replace(' ', '_')
        evidence = dict(line=line_no, text=body)
        result['shadow_payloads'].append(evidence)
        if 'summary' in body or 'shutdown' in body.lower() or 'totals' in body or 'total:' in body:
            result['shadow_summaries'].append(dict(key=key, **evidence))
        if shadow_sample(body, key):
            count = keys.setdefault(key, dict(lines=0, exact=0, first_mismatch=''))
            exact = not mismatch(body) and not resynced(body)
            count['lines'] += 1
            count['exact'] += int(exact)
            if not exact and not count['first_mismatch']:
                count['first_mismatch'] = f'line {line_no}: {body}'
            result['shadow_records'].append(dict(key=key, **evidence))
        else:
            counts = aggregate_counts(body)
            if counts and counts[0]:
                # Ports has separate airport/seaport totals. Retain each scope.
                scope = re.search(r'shutdown: (airport|seaport) ', body)
                aggregate[(key, scope[1] if scope else '')] = (counts, evidence)
    for (key, _), ((total, exact), evidence) in aggregate.items():
        if any(sample['key'] == key for sample in result['shadow_records']):
            continue
        count = keys.setdefault(key, dict(lines=0, exact=0, first_mismatch=''))
        if resynced(evidence['text']):
            exact = 0
        count['lines'] += total
        count['exact'] += exact
        if total != exact and not count['first_mismatch']:
            count['first_mismatch'] = f'line {evidence["line"]}: {evidence["text"]}'
    if not verdict_line:
        verdict_line = verdict_text
    verdict = re.search(r'AUTOTEST VERDICT:\s*(\w+)|^verdict:\s*([\w-]+)', verdict_line, re.M)
    if verdict:
        result['verdict'] = verdict[1] or verdict[2]
    found_date = re.search(r'\bdate=(\d{4}-\d\d-\d\d)', verdict_line)
    if found_date:
        reached_date = found_date[1]
        if reached_date >= end:
            result['end_reached'] = '1'
    speed = re.search(r'\bdays_per_second=([0-9.]+)', verdict_line)
    if speed:
        result['days_per_second'] = speed[1]
        if float(speed[1]) > 0:
            result['seconds_per_month'] = round(30.4375 / float(speed[1]), 3)
    elapsed = re.search(r'\breal_seconds=([0-9.]+)', verdict_line)
    if elapsed:
        result['real_seconds'] = float(elapsed[1])
    wall = re.search(r'\bwall (\d+) s', verdict_text)
    if wall:
        result['wall_seconds'] = float(wall[1])
    elif attached and verdict_at:
        result['wall_seconds'] = (verdict_at - attached).total_seconds()
    harness_verdict = re.search(r'^verdict:\s*([\w-]+)', verdict_text, re.M)
    if harness_verdict and harness_verdict[1] != 'clean':
        result['verdict'] = harness_verdict[1]
        result['invalid_reason'] = 'harness verdict overrides plugin verdict'
    result['raw_verdict'] = result['verdict']
    # A forced exit can end a calendar stall while the main thread keeps ticking.
    # Preserve the launcher verdict; require a sustained final emergency episode,
    # never a brief/recovered disaster pause or an unexplained process crash.
    if (result['verdict'] == 'exited' and 'exited (code -1) without a verdict' in verdict_text
            and result['end_reached'] != '1' and not result['exceptions']
            and not re.search(r'WATCHDOG:.*(?:stalled|stuck)', text)
            and emergency_first and emergency_last
            and (emergency_last[0] - emergency_first[0]).total_seconds() >= 60):
        result['verdict'] = 'stalled'
        result['invalid_reason'] = 'emergency pause: calendar stopped while OnTick continued; forced exit'
        result['calendar_stall'] = dict(line=emergency_last[1], text=emergency_last[2],
            seconds=(emergency_last[0] - emergency_first[0]).total_seconds(), julian=emergency_first[1])
    if quit_dispatch_failed and result['verdict'] == 'clean':
        result['verdict'] = 'harness-error'
        result['invalid_reason'] = 'native quit command could not be queued; shutdown untested'
    if only_load:
        result['end_reached'] = '1' if result['verdict'] == 'clean' else '0'
    elif speed and float(speed[1]) == 0:
        result['invalid_reason'] = 'game date did not advance (days_per_second=0)'
        if result['verdict'] == 'clean':
            result['verdict'] = 'invalid'
        result['end_reached'] = '0'
    elif result['verdict'] == 'clean' and result['end_reached'] != '1':
        result['invalid_reason'] = 'end date not reached'
        result['verdict'] = 'invalid'
    if (not only_load and result['end_reached'] == '1'
            and result['verdict'] in ('corrupt', 'crash') and result['exceptions']
            and all(e['after_end'] and e['after_quit'] for e in result['exceptions'])):
        result['verdict'] = 'clean-exit-crash'
        result['invalid_reason'] = 'exception after end date during quit'
        if (all(e['location'].lower() == 'simcity 4.exe+0x1d0f94'
                and 'code 0xC0000005 ACCESS_VIOLATION (reading address 0x00000000)' in e['text']
                for e in result['exceptions'])
                and not re.search(r'WATCHDOG:.*(?:stalled|stuck)', text)):
            result['verdict'] = 'clean'
            result['known_quit_faults'] = len(result['exceptions'])
            result['invalid_reason'] = ''
    result['action_lines'] = [a[1] for a in actions.values()]
    result['actions_ok'] = sum(a[0] for a in actions.values())
    result['actions_failed'] = len(actions) - result['actions_ok'] + parse_errors
    result['actions_pending'] = max(0, result['actions_expected'] - len(actions))
    result['per_key'] = keys
    result['shadow_lines'] = sum(v['lines'] for v in keys.values())
    result['shadow_exact_lines'] = sum(v['exact'] for v in keys.values())
    if keys:
        worst = max(keys, key=lambda k: ((keys[k]['lines'] - keys[k]['exact']) / keys[k]['lines'], k))
        count = keys[worst]
        result['worst_shadow'] = (f'{worst}:{count["lines"] - count["exact"]}/{count["lines"]}'
                                  if count['lines'] != count['exact'] else 'exact')
    return result


def write_tsv(path, columns, rows):
    with path.open('w', encoding='utf-8', newline='') as stream:
        writer = csv.DictWriter(stream, fieldnames=columns, delimiter='\t', lineterminator='\n', extrasaction='ignore')
        writer.writeheader(); writer.writerows(rows)


def add_lua_evidence(result, text):
    """Count actual Maxis command returns; a clean game alone cannot pass a Lua script."""
    pending, seen = [], set()
    complete = False
    for line in text.splitlines():
        match = re.search(r'\bFC4_LUA (\d+) ([\w.-]+) (.*)', line)
        if not match or match[0] in seen:
            continue
        seen.add(match[0])
        body = match[3]
        if body.startswith(('unavailable ', 'wrong-state ', 'timeout ')):
            result['actions_failed'] += 1
        if body.startswith('Before '):
            pending.append(body.split()[1])
            result['actions_expected'] += 1
        returned = re.match(r'Return (\w+) status=(\S+)', body)
        if returned:
            command, status = returned.groups()
            if pending and pending[-1] == command:
                pending.pop()
                result['actions_ok' if status == '0' else 'actions_failed'] += 1
            else:
                result['actions_failed'] += 1  # orphan/misordered evidence cannot pass
            result['action_lines'].append(match[0])
        complete |= body == 'scenario-complete'
    result['actions_pending'] += len(pending) + int(not complete)
    result['lua_complete'] = complete
    return result


def write_results(directory, manifest, results, *, reach_cache=None):
    rows = []
    findings = []
    per_key = {key: dict(key=key, scenarios_run=0, scenarios_exact=0, first_mismatch='-')
               for key in manifest['shadow_keys']}
    for request in manifest['requests']:
        name = request['scenario']['name']
        row = dict(results.get(name, parse_log('', request['scenario']['end'])),
                   scenario=name, profile=manifest['profile'],
                   dll_sha256=request.get('dll_sha256', manifest['dll_sha256']))
        row.setdefault('log', '-')
        rows.append(row)
        for key, value in row['per_key'].items():
            entry = per_key.setdefault(key, dict(key=key, scenarios_run=0, scenarios_exact=0, first_mismatch='-'))
            entry['scenarios_run'] += 1
            entry['scenarios_exact'] += int(value['lines'] == value['exact'])
            if value['first_mismatch'] and entry['first_mismatch'] == '-':
                entry['first_mismatch'] = f'{name} {row["log"]} {value["first_mismatch"]}'
            if value['first_mismatch']:
                findings.append(dict(key=key, scenario=name, log=row['log'],
                                     first_mismatch=value['first_mismatch']))
    write_tsv(directory / 'summary.tsv', COLUMNS, rows)
    write_tsv(directory / 'per_key.tsv', KEY_COLUMNS, [per_key[k] for k in sorted(per_key)])
    write_tsv(directory / 'findings.tsv', ['key', 'scenario', 'log', 'first_mismatch'], findings)
    import suite_select
    from collections import Counter
    # Completed request traces are immutable. Keep the cache local to this
    # harvest operation; never cache a live/incomplete trace across resumes.
    if reach_cache is None:
        reach_cache = {}
    for name in results:
        if name not in reach_cache:
            reach_cache[name] = suite_select.reach(directory / (name + '.reach.tsv'))[0]
    reached = {name: reach_cache[name] for name in results}
    scenarios = {r['scenario']['name']: r['scenario'] for r in manifest['requests']}
    counts = Counter(address for addresses in reached.values() for address in addresses)
    metrics, windows = [], []
    for name, parsed in results.items():
        metrics.append(dict(scenario=name,
            **{key: parsed.get(key) for key in ('wall_seconds', 'load_seconds', 'seconds_per_month', 'city_bytes')},
            reach_functions=len(reached[name]) if reached[name] else '-',
            marginal_functions=sum(counts[a] == 1 for a in reached[name]) if reached[name] else '-',
            prepared_until=scenarios[name].get('_prepared_until', '-'),
            setup_actions=scenarios[name].get('_setup_actions', 0),
            knee_months=suite_select.knee(parsed.get('reach_windows', []))))
        windows.extend(dict(scenario=name, **window) for window in parsed.get('reach_windows', []))
    write_tsv(directory / 'metrics.tsv', ['scenario', 'wall_seconds', 'load_seconds', 'seconds_per_month',
              'city_bytes', 'reach_functions', 'marginal_functions', 'knee_months', 'prepared_until', 'setup_actions'], metrics)
    write_tsv(directory / 'saturation.tsv', ['scenario', 'window', 'date', 'new', 'total'], windows)


def wait_for_batch(root, directory, manifest, wait_seconds, *, submit=False, slots=6, stop_file=None):
    slots = min(slots, 6)  # Include waiting requests in the shared queue footprint.
    deadline = time.monotonic() + wait_seconds
    pending = {r['scenario']['name']: r for r in manifest['requests']}
    active = set(pending) if not submit else {
        name for name, request in pending.items()
        if any((root / relative / (request['label'] + suffix)).exists()
               for relative, suffix in (('build-runs/game-queue', '.req'),
                                        ('build-runs/game-queue/done', '.req'),
                                        ('build-runs/game-results', '.running'),
                                        ('build-runs/game-results', '.verdict')))}
    waiting = [name for name in pending if name not in active] if submit else []
    results, reach_cache = {}, {}
    try:
        while pending:
            stopping = stop_file is not None and stop_file.exists()
            # Include queued requests in this lane's shared footprint.
            while waiting and not stopping and len(active) < slots and time.monotonic() < deadline:
                name = waiting.pop(0)
                request = pending[name]
                path = root / 'build-runs/game-queue' / (request['label'] + '.req')
                temporary = path.with_suffix('.req.tmp')
                temporary.write_text(request['request'], encoding='utf-8')
                temporary.replace(path)
                active.add(name)
            changed = False
            for name, request in list(pending.items()):
                if name not in active:
                    continue
                source = root / 'build-runs/game-results' / request['label']
                # Labels can contain dots: append the extension, do not replace it.
                verdict = Path(str(source) + '.verdict')
                if not verdict.is_file():
                    continue
                verdict_text = verdict.read_text(encoding='utf-8-sig', errors='replace')
                source_log = Path(str(source) + '.log')
                log = directory / f'{name}.log'
                if source_log.is_file():
                    shutil.copy2(source_log, log)
                text = log.read_text(encoding='utf-8-sig', errors='replace') if log.exists() else ''
                parsed = parse_log(text, request['scenario']['end'], verdict_text,
                                   only_load=load_only(request['scenario']))
                if Path(request['scenario']['script']).suffix.lower() == '.lua':
                    lua_source = Path(str(source) + '.lua.log')
                    lua_log = directory / f'{name}.lua.log'
                    lua_text = ''
                    if lua_source.is_file():
                        shutil.copy2(lua_source, lua_log)
                        lua_text = lua_log.read_text(encoding='utf-8-sig', errors='replace')
                    add_lua_evidence(parsed, lua_text)
                elif request['scenario']['script'] != '-' and not parsed['actions_expected'] and not parsed['action_lines']:
                    parsed['actions_pending'] = 1  # requested script produced no execution evidence
                parsed['log'] = str(log) if log.exists() else '-'
                source_city = city_path(request['scenario'])
                parsed['city_bytes'] = source_city.stat().st_size if source_city.is_file() else None
                (directory / f'{name}.verdict').write_text(verdict_text, encoding='utf-8')
                (directory / f'{name}.json').write_text(json.dumps(parsed, indent=2) + '\n', encoding='utf-8')
                (directory / f'{name}.shadows.txt').write_text(
                    '\n'.join(r['text'] for r in parsed['shadow_payloads']) + '\n', encoding='utf-8')
                results[name] = parsed
                del pending[name]
                active.remove(name)
                print(f'{name}: {parsed["verdict"]}; end={parsed["end_reached"]}; '
                      f'actions={parsed["actions_ok"]} ok/{parsed["actions_failed"]} failed; '
                      f'shadow={parsed["shadow_exact_lines"]}/{parsed["shadow_lines"]}', flush=True)
                changed = True
            if changed:
                write_results(directory, manifest, results, reach_cache=reach_cache)
            if stopping and not active:
                for name, request in pending.items():
                    results[name] = parse_log('', request['scenario']['end'], 'verdict: not-submitted')
                break
            if pending and time.monotonic() >= deadline:
                print('queue wait expired; queued/running: ' + ', '.join(sorted(active))
                      + '; not submitted: ' + ', '.join(waiting), file=sys.stderr)
                for name, request in pending.items():
                    verdict = 'queue-timeout' if name in active else 'not-submitted'
                    results[name] = parse_log('', request['scenario']['end'], 'verdict: ' + verdict)
                break
            if pending:
                time.sleep(min(2, max(0, deadline - time.monotonic())))
    finally:
        write_results(directory, manifest, results, reach_cache=reach_cache)
    return results


def run_scenarios(root, scenarios, *, profile, dll, tag, extra=(), wait_seconds=7200, dry_run=False, slots=6):
    slots = min(slots, 6)
    directory, manifest = make_batch(root, scenarios, profile, dll, tag, extra)
    if dry_run:
        print(f'prepared {len(scenarios)} requests (not submitted): {directory}')
        return directory, {}
    queue = root / 'build-runs/game-queue'
    queue.mkdir(parents=True, exist_ok=True)
    print(f'staged {len(scenarios)} requests; at most {slots} in flight; results: {directory}', flush=True)
    started = time.monotonic()
    manifest['timing'] = dict(started_utc=datetime.now(timezone.utc).isoformat(), slots=slots, complete=False)
    try:
        results = wait_for_batch(root, directory, manifest, wait_seconds, submit=True, slots=slots)
        manifest['timing']['complete'] = all(r['verdict'] not in ('queue-timeout', 'not-submitted') for r in results.values())
        return directory, results
    finally:
        manifest['timing'].update(finished_utc=datetime.now(timezone.utc).isoformat(),
                                  wall_seconds=round(time.monotonic() - started, 3))
        (directory / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')


def report(root, tag, against=None):
    if not NAME.fullmatch(tag):
        raise ValueError('invalid tag')
    directory = root / 'build-runs/suite' / tag
    for filename in ('summary.tsv', 'per_key.tsv'):
        print(filename)
        print((directory / filename).read_text(encoding='utf-8').rstrip())
    if against:
        if not NAME.fullmatch(against):
            raise ValueError('invalid comparison tag')
        baseline = directory.parent / against
        manifest = json.loads((directory / 'manifest.json').read_text(encoding='utf-8'))
        other = json.loads((baseline / 'manifest.json').read_text(encoding='utf-8'))
        other_rows = {r['scenario']['name']: r['scenario'] for r in other['requests']}
        comparison = []
        print('scenario\tshadow_determinism\tfirst_difference')
        for request in manifest['requests']:
            row = request['scenario']
            name = row['name']
            paths = [folder / f'{name}.shadows.txt' for folder in (directory, baseline)]
            difference = None
            previous = other_rows.get(name)
            scripts = [folder / 'scripts' / (name + Path(row['script']).suffix) for folder in (directory, baseline)]
            same_script = row['script'] == '-' or (all(p.is_file() for p in scripts)
                          and scripts[0].read_bytes() == scripts[1].read_bytes())
            if previous is None or not all(p.exists() for p in paths):
                status = 'missing'
            elif (manifest['dll_sha256'] != other['dll_sha256'] or manifest['profile'] != other['profile']
                    or any(row.get(k) != previous.get(k) for k in ('region', 'city', 'end', '_prepared_until'))
                    or manifest.get('extra') != other.get('extra') or not same_script):
                status = 'input-different'
            else:
                lines = [p.read_text(encoding='utf-8').splitlines() for p in paths]
                if not all(lines):
                    status = 'no-evidence'
                else:
                    difference = next((i for i, (a, b) in enumerate(itertools.zip_longest(*lines), 1) if a != b), None)
                    status = 'different' if difference else 'exact'
            comparison.append(dict(scenario=name, shadow_determinism=status, first_difference=difference or '-',
                                   log=directory / f'{name}.log', against_log=baseline / f'{name}.log'))
            print(f'{name}\t{status}\t{difference or "-"}')
        write_tsv(directory / 'determinism.tsv', ['scenario', 'shadow_determinism', 'first_difference', 'log', 'against_log'], comparison)


def new_tag():
    return datetime.now().strftime('%Y%m%d-%H%M%S-') + uuid.uuid4().hex[:6]


def successful(results):
    return bool(results) and all(r['verdict'] == 'clean' and r['end_reached'] == '1'
                                and not r['actions_failed'] and not r['actions_pending']
                                for r in results.values())


def main(argv=None, root=ROOT):
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    run = commands.add_parser('run')
    run.add_argument('--set', choices=['tier1', 'smoke', 'full', 'gen', 'findings', 'reach29', 'corpus34', 'corpus35', 'all'], default='tier1')
    run.add_argument('--profile', choices=['shadows', 'replace', 'plain'], default='shadows')
    run.add_argument('--dll', help='worktree or DLL; default main checkout DLL')
    run.add_argument('--tag', default=None)
    run.add_argument('--extra', action='append', default=[])
    run.add_argument('--wait-seconds', type=int, default=7200)
    run.add_argument('--dry-run', action='store_true', help='stage reproducible inputs without submitting')
    run.add_argument('--slice', help='nightly rotation, zero-based INDEX/COUNT; e.g. 0/6')
    run.add_argument('--reach', action='store_true', help='request reach27 per-scenario address/window evidence')
    run.add_argument('--states', help='reuse a completed, hash-checked prepared-state tag')
    prepare = commands.add_parser('prepare', help='run declared setup and export saves outside Documents')
    prepare.add_argument('--set', default='smoke')
    prepare.add_argument('--tag', required=True)
    prepare.add_argument('--dll')
    prepare.add_argument('--warmup-months', type=int, default=1)
    prepare.add_argument('--wait-seconds', type=int, default=7200)
    prepare.add_argument('--dry-run', action='store_true', help='stage setup requests without submitting games')
    select = commands.add_parser('select', help='greedy measured tier 1 or change-targeted cover')
    for command in (run, select):
        command.add_argument('--from', dest='sources', help='comma-separated completed calibration tags; default all')
        command.add_argument('--keys', default='', help='comma-separated shadow/replace keys or owners')
        command.add_argument('--budget-seconds', type=float, default=900)
        command.add_argument('--slots', type=int, choices=range(1, 13), default=6)
    select.add_argument('--output', help='optional JSON selection evidence under build-runs/suite')
    run.add_argument('--scenario', action='append', help='select named rows within --set; repeatable')
    show = commands.add_parser('report')
    show.add_argument('tag'); show.add_argument('--against')
    resume = commands.add_parser('resume', help='harvest/restart a staged batch without resubmitting claimed labels')
    resume.add_argument('tag')
    resume.add_argument('--slots', type=int, choices=range(1, 13), default=6)
    resume.add_argument('--wait-seconds', type=int, default=7200)
    resume.add_argument('--seconds', type=int, help='bound only requests not yet claimed by the shared queue')
    resume.add_argument('--timeout', type=int)
    args = parser.parse_args(argv)
    try:
        if args.command == 'resume':
            if not NAME.fullmatch(args.tag):
                raise ValueError('invalid suite tag')
            directory = root / 'build-runs/suite' / args.tag
            manifest_path = directory / 'manifest.json'
            manifest = json.loads(manifest_path.read_text(encoding='utf-8'))
            for request in manifest['requests']:
                label = request['label']
                claimed = any((root / rel / (label + suffix)).exists() for rel, suffix in (
                    ('build-runs/game-queue', '.req'), ('build-runs/game-queue/done', '.req'),
                    ('build-runs/game-results', '.running'), ('build-runs/game-results', '.verdict')))
                if not claimed:
                    for key in ('seconds', 'timeout'):
                        value = getattr(args, key)
                        if value is not None:
                            if value <= 0:
                                raise ValueError(f'--{key} must be positive')
                            request['request'] = re.sub(rf'^{key} = \d+$', f'{key} = {value}', request['request'], flags=re.M)
                    (directory / (request['scenario']['name'] + '.req')).write_text(request['request'], encoding='utf-8')
            manifest_path.write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
            results = wait_for_batch(root, directory, manifest, args.wait_seconds, submit=True, slots=args.slots)
            return 0 if successful(results) else 1
        if args.command == 'report':
            report(root, args.tag, args.against)
            return 0
        import suite_select
        if args.command == 'prepare':
            import suite_states
            rows = suite_states.prepare(root, read_scenarios(root, args.set), args.tag, args.warmup_months)
            directory, results = run_scenarios(root, rows, profile='plain', dll=args.dll,
                                              tag='setup-' + args.tag, wait_seconds=args.wait_seconds,
                                              dry_run=args.dry_run)
            if args.dry_run:
                return 0
            manifest = json.loads((directory / 'manifest.json').read_text(encoding='utf-8'))
            return 0 if suite_states.finalize(root, args.tag, manifest, results) else 1
        marked_tier = False
        # A checked-in tier is reproducible without local calibration artifacts.
        if args.command == 'run' and args.set == 'tier1':
            with (root / 'engine/tests/scenarios/suite.tsv').open(encoding='utf-8-sig', newline='') as stream:
                marked_tier = any('tier1' in r['sets'].split(',') for r in csv.DictReader(stream, delimiter='\t'))
        rows = read_scenarios(root, 'full' if args.command == 'select' or (args.set == 'tier1' and not marked_tier) else args.set)
        if getattr(args, 'scenario', None):
            missing = set(args.scenario) - {row['name'] for row in rows}
            if missing:
                raise ValueError('unknown scenario in selected set: ' + ', '.join(sorted(missing)))
            rows = [row for row in rows if row['name'] in args.scenario]
        if args.budget_seconds <= 0:
            raise ValueError('--budget-seconds must be positive')
        if args.command == 'select' or (args.set == 'tier1' and not marked_tier) or args.keys:
            tags = args.sources.split(',') if args.sources else None
            if tags and any(not NAME.fullmatch(tag) for tag in tags):
                raise ValueError('--from expects comma-separated suite tags')
            items = suite_select.measurements(root, rows, tags)
            addresses, keys = suite_select.targets(root, args.keys)
            plan = suite_select.choose(items, slots=args.slots, budget=args.budget_seconds,
                                       target_addresses=addresses, target_keys=keys)
            if not plan['selected']:
                raise ValueError('no measured passing scenarios cover the requested keys within the wall budget')
            print(f'tier1 objective={plan["objective"]} wall={plan["wall_seconds"]:.1f}s union={plan["union_reach"]}; '
                  f'full measured wall={plan["full_wall_seconds"]:.1f}s union={plan["full_union_reach"]}')
            print('scenario\twall_seconds\tmarginal_functions\tknee_months\tsource')
            for item in plan['selected']:
                print('\t'.join(str(item.get(k, '-')) for k in ('name', 'wall_seconds', 'marginal_functions', 'knee_months', 'source')))
            if args.command == 'select':
                if args.output:
                    output = inside(root / 'build-runs/suite', root / args.output)
                    output.parent.mkdir(parents=True, exist_ok=True)
                    output.write_text(json.dumps(suite_select.serializable(plan), indent=2) + '\n', encoding='utf-8')
                return 0
            names = {item['name'] for item in plan['selected']}
            rows = [r for r in rows if r['name'] in names]
        if args.slice:
            match = re.fullmatch(r'(\d+)/(\d+)', args.slice)
            if not match or not 0 <= int(match[1]) < int(match[2]):
                raise ValueError('--slice expects zero-based INDEX/COUNT')
            rows = rows[int(match[1])::int(match[2])]
            if not rows:
                raise ValueError('empty slice')
        extra = list(args.extra)
        if args.states:
            import suite_states
            rows = suite_states.reuse(root, rows, args.states, resolve_dll(root, args.dll))
        if args.reach:
            # These are defaults: a reduced explicit roster must survive --reach.
            extra = ['reach = 1', 'reach_window = month',
                     'reach_contracts = ' + (root / 'docs/abi/functions.tsv').as_posix()] + extra
        directory, results = run_scenarios(root, rows,
            profile=args.profile, dll=args.dll, tag=args.tag or new_tag(), extra=extra,
            wait_seconds=args.wait_seconds, dry_run=args.dry_run, slots=args.slots)
        if not args.dry_run:
            print(f'summary: {directory / "summary.tsv"}')
        return 0 if args.dry_run or successful(results) else 1
    except (ValueError, OSError, subprocess.SubprocessError) as exc:
        parser.error(str(exc))


if __name__ == '__main__':
    sys.exit(main())
