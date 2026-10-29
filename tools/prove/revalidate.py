"""Revalidate stored CPU anchors without starting the game or adding pages."""
import collections
import gc
import gzip
import json
import time
from concurrent.futures import ProcessPoolExecutor, ThreadPoolExecutor

from .common import OUT, STORE_DB, name_of, owner_of
from .store import PageStore, all_functions, captures_of, read_capture, replace_capture
from .machine import Machine
from .replay import Provider
from .validation import cpu_difference, repeat_difference, shared_page_audit

_STORE = None


def initialize():
    global _STORE
    _STORE = PageStore(STORE_DB, readonly=True)


def classify(validation):
    if validation['valid']:
        return 'valid'
    diff = validation.get('difference') or {}
    field = diff.get('field', 'unknown')
    if field == 'execution':
        field += ': '+str(diff.get('ours') or diff.get('original'))
    return field


def validate_record(rec, store, timeout=30):
    old = rec.get('validation', {})
    prior = old.get('difference') or {}
    if not rec.get('exit'):
        return dict(valid=False, difference=dict(field='CPU return was not captured'), clock_rules=[], version=2)
    isolation = rec.get('isolation', {})
    if isolation.get('held') is False or isolation.get('field') or prior.get('field') in (
            'other threads resumed while function was running', 'function waits with other threads held',
            'thread isolation runtime limit', 'CPU exit page unavailable'):
        return {**old, 'valid':False, 'difference':prior or isolation, 'version':2,
                'replay_skipped':'capture provenance prevents validation'}
    m = Machine(rec['entry'], Provider(rec, store), gdt=rec['gdt'], syscall_range=rec['syscall_range'])
    first = m.run(rec['function'], rec['ret'], rec['entry']['regs']['esp'], timeout=timeout)
    changed = {p:b for p,b in m.dirty().items() if p not in m.static}
    m.restore()
    second = m.run(rec['function'], rec['ret'], rec['entry']['regs']['esp'], timeout=timeout)
    repeated = {p:b for p,b in m.dirty().items() if p not in m.static}
    repeat = repeat_difference((first, changed), (second, repeated))
    initial = {int(p):store.get(h) for p,h in rec['pages'].items()}
    actual = {int(p):store.get(h) for p,h in rec['exit_pages'].items()}
    diff, rules = cpu_difference(rec, first, changed, initial, actual)
    if repeat:
        diff = dict(field='nondeterministic replay', detail=repeat)
    if first.get('kind') == 'timeout' or second.get('kind') == 'timeout':
        diff = dict(field='replay watchdog limit', original=first.get('err'), ours=second.get('err'),
                    detail=repeat)
    # Lost isolation is capture provenance, not an emulator error. No amount
    # of offline replay proves that the other process threads were held.
    result = dict(valid=diff is None, difference=diff, clock_rules=rules,
                  compared_pages=len(initial), clock_reads=first['clock_reads'], syscalls=first['syscalls'],
                  repeat_difference=repeat, version=2,
                  shared_page_unexcused=shared_page_audit(actual, changed, initial))
    m = None
    gc.collect()
    return result


def execute(item):
    path, timeout = item
    rec = read_capture(path)
    old = rec['validation']
    validation = validate_record(rec, _STORE, timeout)
    return dict(path=path, address=f"{rec['function']:08x}", scenario=rec['scenario'],
                before=classify(old), after=classify(validation), previous=old, validation=validation)


def metadata(path):
    return dict(path=path, validation=read_capture(path)['validation'])


def revalidate(args, targets=None):
    start = time.perf_counter()
    paths = [str(p) for a in (all_functions() if targets is None else targets) for p in captures_of(a)]
    with ThreadPoolExecutor(max_workers=min(args.jobs,8)) as pool:
        manifest = [m for m in pool.map(metadata, paths)
                    if not args.only_stale or m['validation'].get('version',0) < 2]
    paths = [m['path'] for m in manifest]
    report = args.report or OUT/'reports'/f'revalidation-{time.strftime("%Y%m%d-%H%M%S")}.json'
    report.parent.mkdir(parents=True, exist_ok=True)
    with gzip.open(report.with_suffix('.before.json.gz'), 'wt', encoding='utf-8') as f:
        json.dump(manifest, f, separators=(',', ':'))
    before, after = collections.Counter(), collections.Counter()
    results = []
    with report.with_suffix('.work.jsonl').open('w', encoding='utf-8') as journal, \
         ProcessPoolExecutor(max_workers=args.jobs, initializer=initialize) as pool:
        for result in pool.map(execute, ((p, args.replay_timeout) for p in paths), chunksize=4):
            journal.write(json.dumps(result)+'\n')
            journal.flush()
            if not args.dry_run:
                rec = read_capture(result['path'])
                if rec['validation'] != result['previous']:
                    raise ValueError(f"capture changed during revalidation: {result['path']}")
                rec['validation'] = result['validation']
                replace_capture(result['path'], rec)
            before[result['before']] += 1
            after[result['after']] += 1
            results.append(result)
            if len(results) % 100 == 0:
                print(f'revalidate: {len(results)}/{len(paths)}; valid {after["valid"]}', flush=True)
    report.write_text(json.dumps(dict(seconds=time.perf_counter()-start, jobs=args.jobs,
        updated=not args.dry_run, before=dict(before), after=dict(after), results=results), indent=2))
    print(json.dumps(dict(before=before, after=after, seconds=time.perf_counter()-start, report=str(report))), flush=True)
    return 0
