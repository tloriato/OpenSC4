#!/usr/bin/env python3
"""Original-game cache and candidate-only comparisons (stdlib only).

record / compare share the original-input key; the candidate DLL and replacement
keys are deliberately excluded. record --rerun verifies an existing key without
overwriting its artifacts. A differing repeat is retained and marked NONDETERMINISTIC.
Identical originals require exact results; varying originals supply observed
envelopes by default. --strict requires exact results. No gameplay mask is applied.

--schedule fixed (docs/specs/simulator.md 5.8): references are recorded only by the
pinned reference DLL (`pin --dll <dll>`, build-runs/golden/reference.json); the key is
that DLL, its schedule code and the game inputs (not the harness sources), comparisons
are exact, and a candidate DLL whose schedule code differs is refused.
"""
import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time
import uuid

try:
    from .golden_dbpf import compare_saves, read_records
    from . import golden_envelope, golden_monthly
except ImportError:
    from golden_dbpf import compare_saves, read_records
    import golden_envelope
    import golden_monthly

REPO = Path(__file__).resolve().parents[1]
SEED = 0x5EED1234
SAVE_GRACE_MS = 60000
SCHEMA = 1
PIN = 'reference.json'  # in the store: the reference DLL for --schedule fixed


def sha256(path):
    h = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            h.update(block)
    return h.hexdigest()


def stage_dll(output, source):
    """Pin the bytes actually launched, including concurrent candidate builds."""
    digest = sha256(source)
    staged = Path(output) / 'golden-inputs' / (digest + '.dll')
    staged.parent.mkdir(parents=True, exist_ok=True)
    if not staged.exists():
        temporary = staged.with_suffix('.' + uuid.uuid4().hex + '.tmp')
        try:
            shutil.copyfile(source, temporary)
            if sha256(temporary) != digest:
                raise ValueError('DLL changed while staging; retry after the build finishes')
            try:
                temporary.rename(staged)
            except OSError:
                if not staged.exists():
                    raise
        finally:
            temporary.unlink(missing_ok=True)
    if sha256(staged) != digest:
        raise ValueError('corrupt staged DLL; immutable input was not overwritten')
    return staged


def schedule_code(dll):
    """The fixed schedule's code hash that the DLL carries (engine/CMakeLists.txt)."""
    match = re.search(rb'FC4_SCHEDULE_CODE=([0-9a-f]{16})\0', Path(dll).read_bytes())
    if not match:
        raise ValueError(f'{dll} carries no schedule code; build it from a checkout that stamps one')
    return match[1].decode()


def pinned(output):
    """(pin, staged DLL) of the reference DLL for --schedule fixed."""
    path = Path(output) / 'golden' / PIN
    if not path.is_file():
        raise ValueError(f'no reference DLL pinned ({path}); run: golden.py pin --dll <dll>')
    pin = load_json(path)
    dll = Path(output) / 'golden-inputs' / (pin['dll_sha256'] + '.dll')
    if not dll.is_file() or sha256(dll) != pin['dll_sha256']:
        raise ValueError(f'pinned reference DLL missing or corrupt: {dll}')
    return pin, dll


def pin_reference(args):
    path = args.output / 'golden' / PIN
    if path.exists() and not args.repin:
        print(f'already pinned: {load_json(path)}; --repin starts new references (old keys stay in the store)')
        return 2
    staged = stage_dll(args.output, args.dll.resolve())
    pin = {'dll_sha256': sha256(staged), 'schedule_code': schedule_code(staged), 'source': str(args.dll.resolve()),
           'pinned_at': dt.datetime.now().isoformat(timespec='seconds')}
    path.parent.mkdir(parents=True, exist_ok=True)
    write_json(path, pin)
    print(f'pinned: {pin}')
    return 0


def write_json(path, data):
    Path(path).write_text(json.dumps(data, indent=2, sort_keys=True, allow_nan=False) + '\n', encoding='utf-8')


def load_json(path):
    return json.loads(Path(path).read_text(encoding='utf-8'))


def date(value):
    parsed = dt.date.fromisoformat(value)
    if str(parsed) != value:
        raise ValueError('dates must be YYYY-MM-DD')
    return parsed


def safe_name(value):
    if not re.fullmatch(r"[A-Za-z0-9 ._()'-]+", value) or value in ('.', '..'):
        raise ValueError(f'not a single city/region name: {value!r}')
    return value


def main_checkout(repo):
    text = repo.resolve().as_posix()
    return Path(text.split('/.claude/worktrees/')[0])


def documents():
    if os.name == 'nt':
        import winreg
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER,
                            r'Software\Microsoft\Windows\CurrentVersion\Explorer\User Shell Folders') as key:
            return Path(os.path.expandvars(winreg.QueryValueEx(key, 'Personal')[0]))
    return Path.home() / 'Documents'


def fingerprint(files, settings):
    """files maps stable logical names to paths; no mtime-based hash shortcuts."""
    manifest = {'schema': SCHEMA, 'settings': settings,
                'files': {name: {'sha256': sha256(path), 'size': Path(path).stat().st_size}
                          for name, path in sorted(files.items())}}
    raw = json.dumps(manifest, sort_keys=True, separators=(',', ':')).encode('utf-8')
    return hashlib.sha256(raw).hexdigest(), manifest


def parse_digests(text):
    rows = []
    for number, line in enumerate(text.splitlines(), 1):
        match = re.search(r'DIGEST julian=\d+ \((\d{4}-\d\d-\d\d)\)(.*)', line)
        if match:
            # Prefix wall-clock time, process and thread IDs are log metadata.
            rows.append({'date': match[1], 'text': match[0], 'line': number,
                         'fields': dict(re.findall(r'(\S+)=([^\s]+)', match[0]))})
    return rows


def compare_digests(left, right):
    # Dates and occurrence order matter; repeated script digests are not discarded.
    for index in range(max(len(left), len(right))):
        a = left[index] if index < len(left) else None
        b = right[index] if index < len(right) else None
        if a is None or b is None or a['text'] != b['text']:
            af, bf = (a or {}).get('fields', {}), (b or {}).get('fields', {})
            return {'date': min(row['date'] for row in (a, b) if row),
                    'expected': a, 'actual': b,
                    'fields': {key: {'expected': af.get(key), 'actual': bf.get(key)}
                               for key in sorted(af.keys() | bf.keys()) if af.get(key) != bf.get(key)}}
    return None


def compare_artifacts(expected, actual):
    report = {'first_digest_difference': compare_digests(load_json(expected / 'digests.json'),
                                                       load_json(actual / 'digests.json')),
              'save': compare_saves(expected / 'end.sc4', actual / 'end.sc4')}
    if (expected / 'monthly.json').is_file():
        monthly = golden_monthly.compare(load_json(expected / 'monthly.json'), load_json(actual / 'monthly.json'))
        report['monthly'] = monthly
        # A controlled stop has no terminal counterpart: compare the captured
        # month, never the reference's later end save/digests against that month.
        if not monthly['complete']:
            first = monthly['first_difference']
            report['first_digest_difference'] = None
            a = load_json(expected / 'monthly.json')['months'][first['date']]['records']
            b = load_json(actual / 'monthly.json')['months'][first['date']]['records']
            report['save'] = {'exact_records': sum(a[k] == b[k] for k in a.keys() & b.keys()),
                              'different_records': len(first['records']), 'records': first['records']}
        else:
            a = golden_monthly.normalized_records(read_records(expected / 'end.sc4'))
            b = golden_monthly.normalized_records(read_records(actual / 'end.sc4'))
            changed = [{'tgi': golden_monthly.tgi_text(k), 'status': 'different'}
                       for k in sorted(a.keys() | b.keys()) if a.get(k) != b.get(k)]
            report['save']['records'] = changed
            report['save']['different_records'] = len(changed)
            report['save']['exact_records'] = sum(a[k] == b[k] for k in a.keys() & b.keys())
    return report


def artifacts_exact(report):
    return (report['first_digest_difference'] is None and report['save']['different_records'] == 0
            and not report.get('monthly', {}).get('first_difference'))


def shadow_command(region, city, until, dll, script=None):
    """Replay from the SAME start save through the failing month, retaining history."""
    command = ['python', 'tools/play.py', '--region', region, '--city', city,
               '--until', until, '--profile', 'shadows', '--dll', str(dll)]
    if script:
        command += ['--script', str(script)]
    return subprocess.list2cmdline(command)


def _baseline_intact(entry):
    metadata = load_json(entry / 'baseline.json')
    for filename, digest in metadata['artifacts'].items():
        if sha256(entry / filename) != digest:
            raise ValueError(f'corrupt golden artifact: {entry / filename}')
    return metadata


def cache_record(cache, key, manifest, run):
    if (Path(run) / 'monthly.json').is_file():
        with golden_monthly.store_lock(cache):
            return _cache_record(cache, key, manifest, run)
    return _cache_record(cache, key, manifest, run)


def _cache_record(cache, key, manifest, run):
    """Publish immutable baseline artifacts. Atomic directory rename wins races.

    Return (status, comparison). Repeated different results are never overwritten.
    Qualification is a separate audit sidecar, not an alteration of the baseline.
    """
    cache, run = Path(cache), Path(run)
    cache.mkdir(parents=True, exist_ok=True)
    entry = cache / key
    if entry.exists():
        metadata = _baseline_intact(entry)
        if metadata['inputs'] != manifest:
            raise ValueError('cache key collision / input manifest differs')
        report = compare_artifacts(entry, run)
        sample = golden_envelope.snapshot(entry, run)
        audit_path = entry / ('repeat-' + uuid.uuid4().hex + '.json')
        write_json(audit_path, {'exact': artifacts_exact(report), 'run': str(run.resolve()),
                                'sample': sample, 'comparison': report})
        return ('reproduced' if artifacts_exact(report) else 'nondeterministic'), report
    stage = cache / ('.pending-' + uuid.uuid4().hex)
    stage.mkdir()
    try:
        artifacts = ['end.sc4', 'digests.json', 'verdict.json', 'summary.json', 'game.log', 'harness.txt']
        if (run / 'reach.tsv').is_file():
            artifacts.append('reach.tsv')
        if (run / 'monthly.json').is_file():
            artifacts.append('monthly.json')
            used = sum(p.stat().st_size for p in cache.glob('*/months/*.sc4'))
            kept = golden_monthly.publish_snapshots(run, stage, max(0, golden_monthly.STORE_BUDGET - used))
            artifacts.extend('months/' + day + '.sc4' for day in kept)
        for filename in artifacts:
            shutil.copyfile(run / filename, stage / filename)
        write_json(stage / 'baseline.json', {'key': key, 'inputs': manifest, 'source_run': str(run.resolve()),
                                             'artifacts': {name: sha256(stage / name) for name in artifacts}})
        for attempt in range(20):
            try:
                stage.rename(entry)
                break
            except PermissionError:
                # A scanner can hold a just-written file open for a moment (WinError 5); retry.
                if entry.exists():
                    return _cache_record(cache, key, manifest, run)
                if attempt == 19:
                    raise
                time.sleep(0.5)
            except OSError:
                if not entry.exists():
                    raise
                return _cache_record(cache, key, manifest, run)
    finally:
        if stage.exists():
            shutil.rmtree(stage)
    return 'recorded', None


def qualification(entry):
    audits = [load_json(p) for p in entry.glob('repeat-*.json')]
    if any(not row['exact'] for row in audits):
        return 'nondeterministic'
    return 'reproduced' if audits else 'unverified'


def compare_cached(entry, run, *, rule='auto'):
    """Suite API: exact for identical originals, observed ranges otherwise.

    Only the candidate runs. Rebuild derived ranges from verified originals;
    a candidate result never changes the cache or widens its envelope.
    """
    if rule not in ('auto', 'exact', 'envelope'):
        raise ValueError('comparison rule must be auto, exact or envelope')
    _baseline_intact(entry)
    envelope = golden_envelope.build(entry)
    mode = golden_envelope.status(entry, envelope)
    exact_report = compare_artifacts(entry, run)
    is_exact = artifacts_exact(exact_report)
    use_exact = ('monthly' in exact_report or rule == 'exact'
                 or not envelope['stats']['varying_count'] or is_exact)
    report = (exact_report if use_exact
              else golden_envelope.compare(envelope, run))
    report.update(comparison_rule='exact' if use_exact else 'observed-envelope',
                  envelope_status=mode, envelope_samples=envelope['samples'], envelope_stats=envelope['stats'])
    report['verdict'] = 'exact' if is_exact else ('within-envelope' if artifacts_exact(report) else 'outside')
    first = report['first_digest_difference']
    records = report['save']['records']
    report['first_outside'] = ({'date': first['date'], 'digest_fields': first['fields']} if first else
                               {'date': load_json(Path(run) / 'verdict.json').get('save_date'),
                                'record': records[0]} if records else None)
    return report


def subsystem_keys(root):
    text = (root / 'engine/plugin/FC4Subsystems.cpp').read_text(encoding='utf-8')
    registry = text.split('const Subsystem kSubsystems[] = {', 1)[1].split('};', 1)[0]
    keys = re.findall(r'^\s*\{"([a-z][a-z0-9_]*)",', registry, flags=re.M)
    if 'simulator' not in keys or 'budget' not in keys:
        raise ValueError('cannot read subsystem registry for selective replacement')
    return sorted(set(keys))


def input_script(path, until, *, truncate=False):
    text = Path(path).read_text(encoding='utf-8-sig') if path else ''
    if path and Path(path).suffix.lower() == '.lua':
        raise ValueError('Lua actions need the luacmd harness interface; use a .script file for now')
    for number, line in enumerate(text.splitlines(), 1):
        line = line.split('#', 1)[0].strip()
        if not line:
            continue
        words = line.split()
        if len(words) < 2 or (date(words[0]) > date(until) and not truncate):
            raise ValueError(f'script line {number} is invalid or after --until')
        # An early pause or speed 0 can be intentional, but cannot silently pass:
        # the completed terminal save and exact end date are validated in the log.
    return text + f'\n{until} digest\n{until} save\n{until} mark GOLDEN_FULL_SAVE_COMPLETE\n'


def autotest_end(args):
    # [choice] Under the fixed schedule a run ends at the end date's save (terminate): the original's quit
    # after it can crash in the teardown (a construction occupant's Shutdown at 0x005D0F94, the terrain's
    # edge reconcile at 0x0074D290), and nothing after the save is compared.
    return 'terminate' if args.schedule == 'fixed' else 'quit'


def prepare(args):
    region, city = safe_name(args.region), safe_name(args.city)
    date(args.until)
    root = args.harness_root.resolve()
    save = getattr(args, 'prepared_city', None) or args.regions_root / region / f'City - {city}.sc4'
    if not save.is_file():
        raise ValueError(f'no start save: {save}')
    settings = {'region': region, 'city': city, 'until': args.until, 'speed': 2,
                'turbo_requested': args.turbo, 'turbo_frame_ms': 250, 'pacing': 'original',
                'clock': args.clock, 'fixed_deliveries_per_frame': 64 if args.clock == 'fixed' else None,
                'schedule': args.schedule,
                'session_seed': SEED, 'slot': args.slot if args.backend == 'direct' else 'queue-selected',
                'timeout_seconds': None if args.schedule == 'fixed' else args.seconds, 'cpu_count': 1,
                'affinity': 2 ** (2 * args.slot) if args.backend == 'direct' else '2*slot CPU index', 'popup_dialogs': 0,
                'full_save': True, 'script_save_grace_ms': SAVE_GRACE_MS,
                'environment': 'explicit switches; FC4_* removed in direct backend',
                'backend': args.backend, 'autotest_end': autotest_end(args), 'window': 'minimized after load',
                'reach': reach_supported(root) if args.backend == 'queue' else False}
    settings['queue_parallel'] = args.backend == 'queue'
    generated = input_script(args.script, args.until, truncate=bool(getattr(args, 'prepared_city', None)))
    if getattr(args, 'monthly', False):
        args.checkpoint_start = golden_monthly.save_date(read_records(save))
        original = args.script.read_text(encoding='utf-8-sig') if args.script else ''
        generated = golden_monthly.instrument(original, args.checkpoint_start, args.until,
                                               reloaded=getattr(args, 'window_reloaded', False))
        settings['monthly_records'] = golden_monthly.NORMALIZER
    if getattr(args, 'prepared_city', None):
        settings['window_parent'] = args.window_parent
    settings['effective_script_sha256'] = hashlib.sha256(generated.encode('utf-8')).hexdigest()
    files = {'exe': args.game_root / 'Apps/SimCity 4.exe', 'start.sc4': save,
             'user.cfg': args.regions_root.parent / 'SimCity 4.cfg'}
    if args.schedule == 'fixed':
        # The pinned DLL and its schedule code; a harness edit that changes results shows up as a
        # difference with no owner switched on (tools/check.py), not as new keys.
        files['reference.dll'] = args.original_dll
        settings['schedule_code'] = schedule_code(args.original_dll)
    else:
        files['observer.dll'] = args.original_dll
        for name in ('ingame_run.ps1', 'city_fixture.ps1', 'sc4mutex.ps1', 'sc4ctl.ps1', 'game_queue.ps1'):
            files['harness/' + name] = root / 'tools' / name
        for name in ('golden.py', 'golden_run.ps1', 'golden_snapshot.ps1'):
            files['golden/' + name] = Path(__file__).with_name(name)
    # Neighbour cities and region layout are simulation inputs too.
    for path in sorted((args.regions_root / region).rglob('*')):
        if path.is_file():
            files['region/' + path.relative_to(args.regions_root / region).as_posix()] = path
    for path in sorted(args.game_root.rglob('*')):
        if path.is_file() and path.suffix.lower() in ('.dat', '.dll', '.sgr', '.ini', '.cfg'):
            files['game/' + path.relative_to(args.game_root).as_posix()] = path
    if args.script:
        files['script'] = args.script
    key, manifest = fingerprint(files, settings)
    return key, manifest, generated, files


def _validate_log(text, until, selected, run, *, stopped=False):
    verdicts = re.findall(r'AUTOTEST VERDICT: ([a-z-]+)[^\r\n]*', text)
    enabled = set(re.findall(r"subsystem '([^']+)': enabled", text))
    # netrules is hosted by the replacement network manager/tool, with no class
    # registration of its own (FC4Subsystems startup explicitly reports this).
    if 'netrules: enabled through netmgr/nettool;' in text:
        enabled.add('netrules')
    enabled = sorted(enabled)
    if enabled != sorted(selected):
        raise ValueError(f'wrong replacements: expected {sorted(selected)}, logged {enabled}; see {run}')
    unexpected = [line for line in text.splitlines()
                  if re.search(r'\b(?:shadow|compare)[^\r\n]*:\s*(?:ON|enabled)', line, re.I)]
    if unexpected:
        raise ValueError(f'shadow/compare unexpectedly enabled: {unexpected[0]}')
    if stopped:
        if 'first-chance exception' in text or 'AUTOTEST VERDICT:' in text:
            raise ValueError(f'plugin failed before controlled monthly stop; see {run}')
        return 'monthly-difference'
    if not verdicts or verdicts[-1] != 'clean':
        raise ValueError(f'not a clean plugin verdict: {verdicts}; see {run}')
    reached = re.search(rf'autotest: end date {re.escape(until)} reached at the new day {re.escape(until)}', text)
    saved = re.search(rf'script: line \d+ {re.escape(until)} save\s*-> ok=1 \(SaveCity\(full\)', text)
    if not reached or not saved or 'GOLDEN_FULL_SAVE_COMPLETE' not in text:
        raise ValueError(f'window / terminal full save incomplete; see {run}')
    return verdicts[-1]


def run_game(args, generated, selected, dll, tag, *, allow_window_drift=False):
    # Golden originals use only our scheduler; every simulated subsystem stays
    # original. Candidates must use the same scheduler and window settings.
    selected = effective_replacements(args.clock, selected)
    validate_harness_sources(args)
    source_dll = dll.resolve()
    dll = stage_dll(args.output, source_dll)
    run_id = f'{tag}-{time.strftime("%Y%m%d-%H%M%S")}-{uuid.uuid4().hex[:8]}'
    work = args.output / 'golden-runs' / run_id
    work.mkdir(parents=True)
    script = work / 'actions.script'
    script.write_text(generated, encoding='utf-8')
    keys = subsystem_keys(args.harness_root) if selected else []
    unknown = set(selected) - set(keys)
    if unknown:
        raise ValueError(f'unknown replacement keys: {sorted(unknown)}')
    disable = ','.join(k for k in keys if k not in selected) if selected else 'all'
    ini = work / 'FreeCity4.ini'
    ini.write_text(f'disable = {disable}\nenable = {",".join(selected)}\nobserve = 1\n'
                   f'session_seed = {SEED}\npacing = original\nsim_turbo = {int(args.turbo)}\n'
                   f'sim_fixed_step = {int(args.clock == "fixed")}\n'
                   f'schedule = {args.schedule}\n'
                   f'script_save_grace_ms = {SAVE_GRACE_MS}\n'
                   f'sim_turbo_frame_ms = 250\npopup_dialogs = 0\nautotest_unpause = 2\n'
                   f'autotest_end = {autotest_end(args)}\nautotest_seconds = {args.seconds}\nscript = {script.resolve()}\n', encoding='utf-8')
    # Diagnostics only (for example `heap trace = <dir>`): GOLDEN_INI_EXTRA lines, separated by ';',
    # are appended; a run that uses it is not a reference.
    if os.environ.get('GOLDEN_INI_EXTRA'):
        with ini.open('a', encoding='utf-8') as stream:
            stream.write(os.environ['GOLDEN_INI_EXTRA'].replace(';', '\n') + '\n')
    if args.backend == 'queue' and reach_supported(args.harness_root):
        with ini.open('a', encoding='utf-8') as stream:
            stream.write('reach = 1\n')
    if getattr(args, 'prepared_city', None):
        with ini.open('a', encoding='utf-8') as stream:
            stream.write(f'prepared_city = {args.prepared_city.resolve()}\n')
    if getattr(args, 'monthly', False):
        runner = (args.harness_root / 'tools/ingame_run.ps1').read_text(encoding='utf-8')
        if 'golden_monthly' not in runner:
            raise ValueError('monthly capture needs the new ingame_run.ps1 in the harness checkout')
        config_path = work / 'monthly-config.json'
        write_json(config_path, {'script': str(script.resolve()), 'result': str((work / 'result').resolve()),
                                 'start': args.checkpoint_start, 'until': args.until, 'city': args.city,
                                 'reference': getattr(args, 'monthly_reference', None),
                                 'early_stop': not args.no_early_stop, 'seconds': args.seconds})
        with ini.open('a', encoding='utf-8') as stream:
            stream.write(f'golden_monthly = {config_path.resolve()}\n')
    # Explicitly turn every known shadow/compare off, also for a queue worker
    # whose environment may contain developer overrides.
    config = (args.harness_root / 'engine/plugin/FC4Config.cpp').read_text(encoding='utf-8')
    verification = sorted(set(re.findall(r'"([a-z0-9_]*(?:shadow|compare)[a-z0-9_]*)"', config)))
    with ini.open('a', encoding='utf-8') as stream:
        stream.write(''.join(f'{key} = 0\n' for key in verification))
    result = work / 'result'
    command = ['powershell', '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File',
               str(Path(__file__).with_name('golden_run.ps1')), '-HarnessRoot', str(args.harness_root.resolve()),
               '-Dll', str(dll.resolve()), '-IniFile', str(ini.resolve()), '-Output', str(result.resolve()),
               '-Region', args.region, '-City', args.city, '-Until', args.until,
               '-Slot', str(args.slot), '-Seconds', str(args.seconds)]
    command += ['-SlotRoot', str(main_checkout(REPO) / 'build-runs/sc4slots')]
    env = {k: v for k, v in os.environ.items() if not k.upper().startswith('FC4_')}
    start = time.perf_counter()
    if args.backend == 'queue':
        run_queued(args, dll, ini, result, work)
    else:
        with (work / 'transport.txt').open('w', encoding='utf-8') as transport:
            process = subprocess.Popen(command, env=env, stdout=transport, stderr=subprocess.STDOUT)
            try:
                deadline = time.monotonic() + args.seconds + 300
                while process.poll() is None:
                    if time.monotonic() > deadline:
                        raise ValueError(f'game harness timed out; see {work}')
                    time.sleep(0.05)
            finally:
                if process.poll() is None:
                    process.kill()
        stopped = (result / 'monthly.json').is_file() and load_json(result / 'monthly.json').get('stopped_early')
        if process.returncode and not stopped:
            raise ValueError(f'game harness failed ({process.returncode}); see {work / "transport.txt"}')
    text = (result / 'game.log').read_text(encoding='utf-8-sig', errors='replace')
    stopped = (load_json(result / 'monthly.json').get('stopped_early', False)
               if (result / 'monthly.json').is_file() else False)
    stop_date = load_json(result / 'monthly.json')['stop_date'] if stopped else args.until
    verdict = _validate_log(text, args.until, selected, result, stopped=stopped)
    if getattr(args, 'monthly', False):
        monthly = load_json(result / 'monthly.json')
        required = golden_monthly.dates(args.checkpoint_start, stop_date)
        if sorted(monthly['months']) != required:
            raise ValueError(f'incomplete monthly captures; not cached: {result}')
    if f'script: full-save watchdog grace {SAVE_GRACE_MS} ms (script_save_grace_ms)' not in text:
        raise ValueError(f'DLL did not acknowledge script_save_grace_ms; not cached: {result}')
    if args.clock == 'fixed' and 'sim_fixed_step ON: 64 agent deliveries/frame' not in text:
        raise ValueError(f'DLL did not acknowledge sim_fixed_step; not cached: {result}')
    if args.schedule == 'fixed' and (f'schedule: fixed (code {schedule_code(dll)},' not in text
                                     or 'schedule: WARNING' in text):
        raise ValueError(f'DLL did not apply schedule = fixed cleanly; not cached: {result}')
    digests = parse_digests(text)
    if not digests:
        raise ValueError(f'no DIGEST observations; see {result}')
    saved_records = read_records(result / 'end.sc4')
    saved_date = save_date(saved_records)
    if saved_date != stop_date and not allow_window_drift:
        raise ValueError(f'end save is dated {saved_date}, expected {args.until}; not cached: {result}')
    write_json(result / 'digests.json', digests)
    write_json(result / 'verdict.json', {'verdict': verdict, 'end_reached': not stopped, 'full_save': True,
                                       'stopped_early': stopped,
                                       'window_exact': saved_date == stop_date, 'save_date': saved_date})
    figures = re.findall(r'days_per_second=([\d.]+)', text)
    warnings = [line for line in text.splitlines() if 'WARNING' in line or 'session seed:' in line]
    summary = {'wall_seconds': time.perf_counter() - start, 'game': load_json(result / 'run.json'),
               'days_per_second': figures[-1] if figures else None, 'digest_lines': len(digests),
               'replacements': selected, 'dll_sha256': sha256(dll), 'staged_dll': str(dll.resolve()),
               'source_dll': str(source_dll), 'warnings': warnings}
    write_json(result / 'summary.json', summary)
    return result


def reach_supported(root):
    """Main's labelled queue export is the capability, never a shared slot file."""
    return 'reach_output =' in (Path(root) / 'tools/game_queue.ps1').read_text(encoding='utf-8')


def validate_harness_sources(args):
    """The existing runner chooses these paths itself; never hash other inputs."""
    expected_regions = documents() / 'SimCity 4/Regions'
    if args.regions_root.resolve() != expected_regions.resolve():
        raise ValueError('--regions-root differs from the harness Documents region source')
    runner = (args.harness_root / 'tools/ingame_run.ps1').read_text(encoding='utf-8')
    installed = re.search(r'^\$GameDir = "([^"]+)"', runner, re.M)
    if not installed or (args.game_root / 'Apps').resolve() != Path(installed[1]).resolve():
        raise ValueError('--game-root differs from the executable the harness launches')
    if args.backend == 'queue' and args.harness_root.resolve() != main_checkout(REPO).resolve():
        raise ValueError('queue backend must fingerprint the main checkout harness actually run by the queue')


def save_date(saved_records):
    """Simulator v4: class-object framing + engine/core/src/save.rs RECORD_LAYOUT_V4."""
    import struct
    values = [value for key, value in saved_records.items() if key[0] == 0x2990C1E5]
    if len(values) != 1 or len(values[0]) != 91 or values[0][12:14] != b'\x04\x00':
        raise ValueError('end save has no unique supported simulator v4 record')
    julian = struct.unpack_from('<I', values[0], 87)[0]
    return str(dt.date(2000, 1, 1) + dt.timedelta(days=julian - 2451545))


def comparison_history(entry, selected):
    identity = hashlib.sha256(','.join(selected).encode('ascii')).hexdigest()[:16]
    return entry / ('latest-' + identity + '.json')


def window_inputs(args, entry, parent, selected):
    """Choose a retained reference snapshot; original and candidate reload it alike."""
    requested = args.from_date
    if requested == 'auto':
        history = comparison_history(entry, selected)
        if not history.is_file():
            return False
        previous = load_json(history)
        if not previous.get('first_difference'):
            return False
        requested = previous['last_matching_month']
    monthly = load_json(entry / 'monthly.json')
    if requested < monthly['start'] or requested >= args.until:
        raise ValueError(f'--from must be {monthly["start"]} <= date < {args.until}')
    date(requested)
    available = {p.stem: p for p in (entry / 'months').glob('*.sc4') if p.stem <= requested}
    start = max(available, default=monthly['start'])
    source = available.get(start, args.regions_root / args.region / f'City - {args.city}.sc4')
    if start != requested:
        print(f'snapshot {requested} not retained; using {start} and retaining the requested end', flush=True)
    requested_day = date(requested)
    # Month ends stay month ends (June 30 -> July 31); otherwise preserve day.
    import calendar
    month = requested_day.month - 1 + args.months
    year, month = requested_day.year + month // 12, month % 12 + 1
    end_day = calendar.monthrange(year, month)[1]
    day = end_day if requested_day.day == calendar.monthrange(requested_day.year, requested_day.month)[1] else min(requested_day.day, end_day)
    until = min(args.until, str(dt.date(year, month, day)))
    staged = args.output / 'suite/states' / (sha256(source) + '.sc4')
    staged.parent.mkdir(parents=True, exist_ok=True)
    if not staged.exists():
        shutil.copyfile(source, staged)
    if sha256(staged) != sha256(source):
        raise ValueError('staged window snapshot is corrupt')
    args.prepared_city, args.window_parent, args.until = staged, parent, until
    args.window_reloaded = start in available
    print(f'window: {start}..{until} (original reload cached separately)', flush=True)
    return True


def run_queued(args, dll, ini, result, work):
    """Use the already-running coordinator queue (no CIM privileges needed here).

    Only submit from a checkout's build-runs. The snapshot verifies that the
    slot has not been reused; export races fail closed with retained evidence.
    """
    if args.output.resolve() != (REPO / 'build-runs').resolve():
        raise ValueError('queue backend requires --output <this checkout>/build-runs')
    queue_text = (args.harness_root / 'tools/game_queue.ps1').read_text(encoding='utf-8')
    region_ready = bool(re.search(r'\bregion\s*=', queue_text, re.I))
    if args.region != 'FC4 Test' and not region_ready:
        raise ValueError('queue region key is not available yet; refusing an FC4 Test fallback')
    label = 'golden-' + uuid.uuid4().hex
    staged = stage_dll(args.output, dll)
    queue = args.output / 'game-queue'
    queue.mkdir(parents=True, exist_ok=True)
    request = (f'label = {label}\ndll = {staged.relative_to(REPO).as_posix()}\ncity = {args.city}\n'
               f'parallel = 1\nseconds = {args.seconds}\ntimeout = {args.seconds + 150}\nend_date = {args.until}\n')
    if region_ready:
        request += f'region = {args.region}\n'
    request += '[ini]\n' + ini.read_text(encoding='utf-8')
    temporary = queue / (label + '.tmp')
    temporary.write_text(request, encoding='utf-8')
    result.mkdir()
    temporary.rename(queue / (label + '.req'))
    verdict_file = args.output / 'game-results' / (label + '.verdict')
    try:
        verdict = wait_snapshot(args, verdict_file, result, work)
    finally:
        # Preserve labelled diagnostics even when validation/export fails.
        for suffix, name in (('.log', 'game.log'), ('.reach.tsv', 'reach.tsv')):
            source = verdict_file.with_suffix(suffix)
            if source.is_file():
                shutil.copyfile(source, result / name)
    (result / 'harness.txt').write_text(verdict, encoding='utf-8')
    stopped = (result / 'monthly.json').is_file() and load_json(result / 'monthly.json').get('stopped_early')
    if not verdict.startswith('verdict: clean') and not stopped:
        raise ValueError(f'queue did not finish cleanly; see {result / "harness.txt"}')
    slot_match = re.search(r'in slot (\d+) after', verdict)
    if not slot_match:
        raise ValueError('queue result does not identify its slot')
    slot = int(slot_match[1])
    wall = re.search(r'wall (\d+) s', verdict)
    write_json(result / 'run.json', {'slot': slot, 'label': label,
                                    'wall_seconds': int(wall[1]) if wall else None, 'backend': 'queue'})


def wait_snapshot(args, verdict_file, result, work):
    """Queue for UserDir while the game owns it, before another job can reuse it."""
    deadline = time.monotonic() + args.queue_timeout
    process = None
    try:
        while not verdict_file.exists():
            marker = verdict_file.with_suffix('.running')
            if process is None and marker.is_file():
                match = re.search(r'running in slot (\d+)', marker.read_text(encoding='utf-8-sig'))
                if match:
                    command = ['powershell', '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass',
                               '-File', str(Path(__file__).with_name('golden_snapshot.ps1')),
                               '-HarnessRoot', str(args.harness_root.resolve()), '-Output', str(result.resolve()),
                               '-City', args.city, '-Region', args.region, '-Slot', match[1],
                               '-ScriptPath', str((work / 'actions.script').resolve()),
                               '-ExpectedLog', str(verdict_file.with_suffix('.log').resolve()),
                               '-VerdictFile', str(verdict_file.resolve()), '-WaitSeconds', str(args.seconds + 150)]
                    process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
            if time.monotonic() > deadline:
                raise ValueError(f'queue wait timed out; request may still run: {verdict_file.stem}')
            time.sleep(1)
        verdict = verdict_file.read_text(encoding='utf-8-sig')
        stopped = (result / 'monthly.json').is_file() and load_json(result / 'monthly.json').get('stopped_early')
        if not verdict.startswith('verdict: clean') and not stopped:
            (result / 'harness.txt').write_text(verdict, encoding='utf-8')
            raise ValueError(f'queue did not finish cleanly; see {result / "harness.txt"}')
        if process is None:
            raise ValueError('missed active slot marker; no safe save export, retry the run')
        output, _ = process.communicate(timeout=150)
        (work / 'transport.txt').write_text(output, encoding='utf-8')
        if process.returncode:
            raise ValueError(f'completed run could not be exported; see {work / "transport.txt"}')
        return verdict
    finally:
        if process is not None and process.poll() is None:
            process.kill()
            process.communicate(timeout=30)


def print_comparison(report, args):
    envelope_mode = report.get('comparison_rule') == 'observed-envelope'
    if 'verdict' in report:
        print('outcome: ' + report['verdict'])
    if 'envelope_stats' in report:
        print('comparison rule: ' + report['comparison_rule'] + '; sampling: ' + report['envelope_status'])
        print_envelope({'samples': report['envelope_samples'], 'stats': report['envelope_stats']})
    first = report['first_digest_difference']
    if first:
        print(f'first DIGEST difference: {first["date"]} (month {first["date"][:7]})')
        for key, value in first['fields'].items():
            print(f'  {key}: {value["expected"]} -> {value["actual"]}')
    else:
        print('DIGEST: within envelope' if envelope_mode else 'DIGEST: exact')
    save = report['save']
    label = 'within envelope' if envelope_mode else 'exact'
    print(f'save records: {save["exact_records"]} {label}, {save["different_records"]} different')
    for record in save['records'][:20]:
        detail = (f', {record["expected_size"]} -> {record["actual_size"]} bytes; '
                  f'first byte {record.get("first_byte", "-")}' if 'expected_size' in record else '')
        print(f'  {record["tgi"]}: {record["status"]}' + detail)
        for field, value in record.get('fields', {}).items():
            print(f'    {field}: {value["expected"]!r} -> {value["actual"]!r}')
    if save['different_records'] > 20:
        print('  remaining records are in comparison.json')
    if 'monthly' in report:
        monthly = report['monthly']
        first_month = monthly['first_difference']
        print('monthly: ' + (f'first difference {first_month["date"]}; '
                            f'last matched {monthly["last_matching_month"]}' if first_month else 'exact'))
        if not monthly['complete']:
            print('monthly: stopped at first difference; later owners incomplete')
    outside = report.get('first_outside')
    if outside and 'record' in outside:
        record = outside['record']
        print(f'first outside save record: {record["tgi"]} on {outside["date"] or args.until}; '
              f'field {next(iter(record.get("fields", {})), "byte " + str(record.get("first_byte", "-")))}')
    if not artifacts_exact(report):
        until = first['date'] if first else args.until
        print('localize (replay from original start save through failing month): ' +
              shadow_command(args.region, args.city, until, args.dll or args.original_dll, args.script))


def effective_replacements(clock, selected):
    return sorted(set(selected) | ({'simulator'} if clock == 'fixed' else set()))


def print_envelope(envelope):
    st = envelope['stats']
    print(f'envelope: {envelope["samples"]} originals; {st["varying_records"]}/{st["records"]} records, '
          f'{st["varying_decoded_fields"]} decoded/structure fields, {st["varying_opaque_bytes"]} opaque byte fields, '
          f'{st["varying_digest_fields"]} digest fields vary; median decoded numeric spread '
          f'{st["median_decoded_numeric_spread"]}', flush=True)
    print(f'typical opaque byte spread: {st["median_opaque_byte_spread"]}; '
          f'digest numeric spreads: {json.dumps(st["digest_spreads"], sort_keys=True)}', flush=True)
    print('Observed ranges do not prove determinism or preserve correlations.', flush=True)


def parser():
    p = argparse.ArgumentParser(description=__doc__)
    commands = p.add_subparsers(dest='command', required=True)
    main = main_checkout(REPO)
    for name in ('record', 'compare', 'determinism', 'key'):
        sub = commands.add_parser(name)
        sub.add_argument('--region', default='FC4 Test')
        sub.add_argument('--city', required=True)
        sub.add_argument('--script', type=Path)
        sub.add_argument('--until', required=True)
        sub.add_argument('--harness-root', type=Path, default=main)
        sub.add_argument('--original-dll', type=Path,
                         help="default: the pinned reference DLL with --schedule fixed, else main's build")
        sub.add_argument('--game-root', type=Path, default=Path(r'C:\Program Files (x86)\GOG Galaxy\Games\SimCity 4 Deluxe Edition'))
        sub.add_argument('--regions-root', type=Path, default=documents() / 'SimCity 4/Regions')
        sub.add_argument('--output', type=Path, default=REPO / 'build-runs')
        sub.add_argument('--slot', type=int, choices=range(1, 17), default=6)
        sub.add_argument('--backend', choices=('queue', 'direct'), default='queue')
        sub.add_argument('--queue-timeout', type=int, default=1800)
        sub.add_argument('--seconds', type=int, default=180)
        sub.add_argument('--turbo', action='store_true', help='request turbo (only our simulator supports it)')
        sub.add_argument('--clock', choices=('fixed', 'original'), default='original',
                         help='original scheduler (default); fixed replaces only the scheduler for an explicit experiment')
        sub.add_argument('--schedule', choices=('original', 'fixed'), default='original',
                         help='fixed: the fixed schedule of docs/specs/simulator.md §5.8 (implies --clock fixed)')
        sub.add_argument('--comparison', choices=('auto', 'exact', 'envelope'), default='auto',
                         help='auto (default): exact for identical originals, envelope for varying originals')
        sub.add_argument('--strict', action='store_true',
                         help='require exact comparison; known original nondeterminism returns 3')
        sub.add_argument('--dll', type=Path, help='candidate worktree or DLL (compare only)')
        sub.add_argument('--replace', default='simulator,budget')
        sub.add_argument('--rerun', action='store_true', help='record: independently repeat an existing key')
        sub.add_argument('--monthly', action='store_true', help='capture and compare normalized records every month')
        sub.add_argument('--no-early-stop', action='store_true', help='retain the complete owner timeline')
        sub.add_argument('--from', dest='from_date', nargs='?', const='auto',
                         help='reload a reference snapshot; auto uses the last matched month of the previous failure')
        sub.add_argument('--months', type=int, default=1, help='window length (default one month)')
        sub.add_argument('--repeats', type=int, default=3, help='determinism: independent original runs')
        sub.add_argument('--max-repeats', type=int, default=9,
                         help='upper bound for --repeats (default 9); no implicit extra games')
    pin = commands.add_parser('pin', help='pin the reference DLL for --schedule fixed')
    pin.add_argument('--dll', type=Path, required=True)
    pin.add_argument('--output', type=Path, default=REPO / 'build-runs')
    pin.add_argument('--repin', action='store_true')
    return p


def main(argv=None):
    args = parser().parse_args(argv)
    if args.command == 'pin':
        try:
            return pin_reference(args)
        except (ValueError, OSError) as exc:
            print('golden: ' + str(exc), file=sys.stderr)
            return 2
    if args.schedule == 'fixed':
        args.clock = 'fixed'
        args.comparison = 'exact'
    if args.strict:
        args.comparison = 'exact'
    try:
        if args.schedule == 'fixed':
            pin, reference = pinned(args.output)
            if args.original_dll and sha256(args.original_dll) != pin['dll_sha256']:
                raise ValueError('--schedule fixed records only with the pinned reference DLL; see `pin`')
            args.original_dll = reference
        elif not args.original_dll:
            args.original_dll = main_checkout(REPO) / 'engine/build-x86/FreeCity4.dll'
        if args.seconds < 30 or args.seconds > 3600 or not 3 <= args.repeats <= args.max_repeats <= 50:
            raise ValueError('--seconds must be 30..3600 and 3 <= --repeats <= --max-repeats <= 50')
        key, manifest, generated, files = prepare(args)
        entry = args.output / 'golden' / key
        print('key: ' + key, flush=True)
        if args.command == 'key':
            print(json.dumps(manifest, indent=2, sort_keys=True))
            return 0
        if args.command == 'compare':
            if not args.dll:
                raise ValueError('compare requires --dll <worktree-or-file>')
            if not entry.exists():
                raise ValueError('no golden baseline for this key; run record with the same inputs first')
            _baseline_intact(entry)
            selected = sorted(set(filter(None, re.split(r'[,\s]+', args.replace))))
            if not selected:
                raise ValueError('compare needs at least one --replace key')
            dll = args.dll / 'engine/build-x86/FreeCity4.dll' if args.dll.is_dir() else args.dll
            if args.schedule == 'fixed' and schedule_code(dll) != manifest['settings']['schedule_code']:
                raise ValueError(f"candidate schedule code {schedule_code(dll)} differs from the references' "
                                 f"{manifest['settings']['schedule_code']}: take the schedule from main or re-pin")
            full_entry = entry
            if args.from_date:
                if not args.monthly or not 1 <= args.months <= 120:
                    raise ValueError('--from requires --monthly and --months 1..120')
                if window_inputs(args, entry, key, selected):
                    key, manifest, generated, files = prepare(args)
                    entry = args.output / 'golden' / key
                    print('window key: ' + key, flush=True)
                    if not entry.exists():
                        original = run_game(args, generated, [], args.original_dll, 'window-original')
                        if fingerprint(files, manifest['settings'])[0] != key:
                            raise ValueError('window input changed while recording original')
                        cache_record(args.output / 'golden', key, manifest, original)
                        print('window original: recorded', flush=True)
                    else:
                        _baseline_intact(entry)
                        print('window original: cached', flush=True)
            if args.monthly:
                args.monthly_reference = load_json(entry / 'monthly.json')
            run = run_game(args, generated, selected, dll, 'compare', allow_window_drift=True)
            if fingerprint(files, manifest['settings'])[0] != key:
                raise ValueError(f'input changed during comparison; no verdict: {run}')
            report = compare_cached(entry, run, rule=args.comparison)
            report.update(key=key, baseline_repeatability=qualification(entry), run=str(run))
            if 'monthly' in report:
                report['full_reference'] = full_entry.name
                report['window_from'] = getattr(args, 'checkpoint_start', None) if args.from_date else None
                # A successful short re-check must not erase the full-run anchor:
                # the next invocation should re-check the same failing window.
                if entry == full_entry:
                    write_json(comparison_history(full_entry, selected), report['monthly'])
            window = load_json(run / 'verdict.json')
            report['window_exact'] = window['window_exact']
            report['save_date'] = window['save_date']
            if not window['window_exact']:
                report['verdict'] = 'invalid-window'
                print(f'window mismatch: save dated {window["save_date"]}, expected {args.until}; diagnostic differences only')
            write_json(run / 'comparison.json', report)
            print('baseline repeatability: ' + report['baseline_repeatability'])
            print_comparison(report, args)
            print('comparison: ' + str(run / 'comparison.json'))
            if not window['window_exact']:
                return 2
            if args.comparison == 'exact' and report['baseline_repeatability'] == 'nondeterministic':
                print('originals are nondeterministic; differences are diagnostic, no regression verdict')
                return 3
            return 0 if artifacts_exact(report) else 1
        if entry.exists() and args.command == 'record' and not args.rerun:
            _baseline_intact(entry)
            envelope = golden_envelope.build(entry)
            mode = golden_envelope.status(entry, envelope)
            print(f'cached: {entry}; repeatability: {qualification(entry)}; sampling: {mode}; no game launched')
            print_envelope(envelope)
            if args.comparison == 'exact':
                return 3 if qualification(entry) == 'nondeterministic' else 0
            return 0
        count = args.repeats if args.command == 'determinism' else 1
        for attempt in range(count):
            run = run_game(args, generated, [], args.original_dll, 'original')
            # Reject runs if a source changed underneath the slot mirror or DLL copy.
            if fingerprint(files, manifest['settings'])[0] != key:
                raise ValueError(f'input changed during run; not cached: {run}')
            status, report = cache_record(args.output / 'golden', key, manifest, run)
            print(f'{status}: {run}', flush=True)
            if report:
                write_json(run / 'comparison.json', report)
                print_comparison(report, args)
            envelope = golden_envelope.build(entry)
            mode = golden_envelope.status(entry, envelope)
            write_json(entry / 'envelope.json', envelope)
            print_envelope(envelope)
            print('sampling: ' + mode, flush=True)
        print('baseline repeatability: ' + qualification(entry))
        if args.comparison == 'exact':
            return 3 if qualification(entry) == 'nondeterministic' else 0
        return 0
    except (ValueError, OSError, subprocess.SubprocessError) as exc:
        print('golden: ' + str(exc), file=sys.stderr)
        return 2


if __name__ == '__main__':
    sys.exit(main())
