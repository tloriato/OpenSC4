"""Check every stored hit and deterministic variant, recording bounded evidence."""
import csv
import hashlib
import json
import struct
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from .common import OUT, PROVEN, name_of, owner_of, abi_args, STORE_DB
from .store import PageStore, captures_of, read_capture
from .channel import channel, describe
from .compare import difference, context_result
from .pe import Port
from .replay import Replay
from .variants import variants
from .calls import CALL_INPUTS


def same(replay, left, right, parts):
    return difference(left[0], right[0], parts, left[1], right[1], replay.m.entry, replay.rec['stack'])


def twice(replay, port, parts, collect=False, timeout=30):
    first = replay.run(port=port, collect=collect, timeout=timeout)
    reads = dict(replay.inputs)
    replay.m.restore()
    second = replay.run(port=port, timeout=timeout)
    if any(v[0].get('kind') == 'timeout' for v in (first, second)):
        raise RuntimeError('replay wall watchdog expired; no proof rows written')
    diff = deterministic_difference(first, second)
    replay.m.restore()
    return first, reads, diff


def deterministic_difference(first, second):
    from .validation import repeat_difference
    return repeat_difference(first, second)


def validate_replay(rec, replay, original, store):
    """Recheck the CPU anchor with the current emulator before accepting a port."""
    from .validation import cpu_difference
    actual = {int(p): store.get(h) for p, h in rec['exit_pages'].items()}
    initial = {int(p): store.get(h) for p, h in rec['pages'].items()}
    return cpu_difference(rec, original[0], original[1], initial, actual)[0]


def update_proven(rows):
    fields = ['address', 'name', 'owner', 'verdict', 'captures', 'variants', 'skipped', 'scenarios', 'dll_sha256']
    old = {}
    if PROVEN.exists():
        with PROVEN.open(encoding='utf-8', newline='') as f:
            old = {r['address']: r for r in csv.DictReader(f, delimiter='\t')}
    for r in rows:
        old[r['address']] = {k: r[k] for k in fields}
    tmp = PROVEN.with_suffix('.tmp')
    with tmp.open('w', encoding='utf-8', newline='') as f:
        writer = csv.DictWriter(f, fields, delimiter='\t', extrasaction='ignore')
        writer.writeheader()
        writer.writerows(old[a] for a in sorted(old))
    tmp.replace(PROVEN)


def check(args, targets):
    from .work import run_items, merge_decisions
    keep_going = getattr(args, 'keep_going', False)
    jobs = getattr(args, 'jobs', 8)
    all_start = time.perf_counter()
    batch_size = getattr(args, 'variant_batch', 32)
    limits = dict(variant_instructions=getattr(args, 'variant_instructions', 200_000),
                  replay_timeout=getattr(args, 'replay_timeout', 30))
    raw = args.dll.read_bytes()
    sha = hashlib.sha256(raw).hexdigest()
    dll = OUT / 'dlls' / f'{sha}.dll'
    dll.parent.mkdir(parents=True, exist_ok=True)
    dll.write_bytes(raw)
    report = args.report or OUT / 'checks' / f'{time.strftime("%Y%m%d-%H%M%S")}-{sha[:8]}.json'
    report.parent.mkdir(parents=True, exist_ok=True)
    tool_files = {p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in Path(__file__).parent.glob('*.py')}
    records = {a:[] for a in targets}
    paths = [(a,p) for a in targets for p in captures_of(a)]
    progress = time.perf_counter()
    with ThreadPoolExecutor(max_workers=min(jobs,8)) as pool:
        for i,((a,p),rec) in enumerate(zip(paths, pool.map(read_capture, (p for a,p in paths))),1):
            records[a].append(rec)
            if time.perf_counter()-progress >= 15:
                print(f'check: loaded {i}/{len(paths)} capture records', flush=True)
                progress = time.perf_counter()
    flat = [c for cs in records.values() for c in cs]
    if not flat:
        raise ValueError('no captures for the requested functions; run capture first')
    avoid = [(c['gdt'], c['gdt']+4096) for c in flat]
    port = Port(dll, [c['regions'] for c in flat], avoid=avoid)
    results, items = [], []
    prepare_seconds = time.perf_counter()-all_start
    for a in targets:
        cs = [c for c in records[a] if c['validation']['valid']]
        parts, evidence = channel(a, [c['ret'] for c in cs])
        result = dict(address=f'{a:08x}', name=name_of(a), owner=owner_of(a), verdict='EXACT',
                      captures=0, variants=0, skipped=0, deadline_skips=0, scenarios='', dll_sha256=sha,
                      return_channel=describe(parts), channel_evidence=evidence, difference=None,
                      invalid_captures=[c['_path'] for c in records[a] if not c['validation']['valid']],
                      skips=[], seed=args.seed, requested_variants=args.variants,
                      capture_failures=0, variant_failures=0, unattempted_variants=0,
                      unknown_call_targets=[], available_captures=len(cs), seconds=0)
        if a not in port.exports:
            result['difference'] = dict(field='missing export', export=f'fc4p_{a:08x}')
        if not cs:
            result['difference'] = dict(field='no captures')
        results.append(result)
        if result['difference']:
            result['unattempted_variants'] = args.variants
            continue
        for capture_index, rec in enumerate(cs):
            quota = args.variants // len(cs) + (capture_index < args.variants % len(cs))
            start = (args.variants // len(cs))*capture_index + min(capture_index, args.variants % len(cs))
            for lo in range(0, max(1, quota), batch_size):
                items.append(dict(result=len(results)-1, capture_index=capture_index,
                    path=rec['_path'], parts=parts, start=start+lo,
                    count=min(batch_size, quota-lo), first=lo == 0))
    config = dict(port=port, dll_sha256=sha, enabled=tuple(targets), seed=args.seed, keep_going=keep_going,
                  store_path=str(STORE_DB), timeout=limits['replay_timeout'],
                  variant_instructions=limits['variant_instructions'],
                  journal=str(report.with_suffix('.work.jsonl')), tool_files=tool_files)
    pieces = run_items(items, config, jobs)
    timings = {}
    for item, piece in zip(items, pieces):
        if piece is None:
            continue
        for key in (results[item['result']]['address'], results[item['result']]['owner']):
            span = timings.setdefault(key, dict(first=float('inf'), last=0, worker_seconds=0))
            span['first'] = min(span['first'], piece['_timing'][0])
            span['last'] = max(span['last'], piece['_timing'][1])
            span['worker_seconds'] += piece['seconds']
    timings = {k:dict(wall_seconds=v['last']-v['first'], worker_seconds=v['worker_seconds']) for k,v in timings.items()}
    decisions, stopped = {}, set()
    scenarios = [set() for _ in results]
    unknowns = [set() for _ in results]
    counts = ('captures', 'variants', 'skipped', 'deadline_skips', 'capture_failures',
              'variant_failures', 'unattempted_variants', 'seconds')
    for item, piece in zip(items, pieces):
        i = item['result']
        if i in stopped:
            continue
        if piece is None:
            raise ValueError('cancelled work before a recorded difference')
        result = results[i]
        for key in counts:
            result[key] += piece[key]
        result['skips'].extend(piece['skips'])
        scenarios[i].update(piece['scenarios'])
        unknowns[i].update(piece['unknown_call_targets'])
        merge_decisions(decisions, piece['call_input_decisions'])
        result['difference'] = result['difference'] or piece['difference']
        if result['difference'] and not keep_going:
            stopped.add(i)
    for i, result in enumerate(results):
        if result['difference']:
            result['verdict'] = 'DIFFERENT'
        if i in stopped:
            result['unattempted_variants'] = args.variants - result['variants'] - result['skipped'] - result['variant_failures']
        result['scenarios'] = ','.join(sorted(scenarios[i]))
        result['unknown_call_targets'] = sorted(unknowns[i])
        print(json.dumps({k:v for k,v in result.items() if k != 'skips'}), flush=True)
    report.write_text(json.dumps(dict(dll=str(args.dll), sha256=sha, mapped_base=hex(port.base),
        jobs=jobs, variant_batch=batch_size, limits=limits,
        prepare_seconds=prepare_seconds,
        timings=timings,
        tool_files=tool_files,
        seconds=time.perf_counter()-all_start, results=results,
        call_input_decisions=dict(sorted(decisions.items()))), indent=2))
    if not args.no_proven:
        update_proven(results)
    print(f'report: {report}', flush=True)
    return int(any(r['verdict'] != 'EXACT' for r in results))
