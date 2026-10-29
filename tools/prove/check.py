"""Check every stored hit and deterministic variant, recording bounded evidence."""
import csv
import hashlib
import json
import struct
import time
from pathlib import Path

from .common import OUT, PROVEN, name_of, owner_of, abi_args
from .store import PageStore, captures_of, read_capture
from .channel import channel, describe
from .compare import difference, context_result
from .pe import Port
from .replay import Replay
from .variants import variants
from .calls import CALL_INPUTS


def same(replay, left, right, parts):
    return difference(left[0], right[0], parts, left[1], right[1], replay.m.entry, replay.rec['stack'])


def twice(replay, port, parts, collect=False):
    first = replay.run(port=port, collect=collect)
    reads = dict(replay.inputs)
    replay.m.restore()
    second = replay.run(port=port)
    diff = deterministic_difference(first, second)
    replay.m.restore()
    return first, reads, diff


def deterministic_difference(first, second):
    a = {k: v for k, v in first[0].items() if k != 'seconds'}
    b = {k: v for k, v in second[0].items() if k != 'seconds'}
    if a != b or first[1] != second[1]:
        return dict(field='replay outcomes differ', original=a, ours=b,
                    changed_pages_equal=first[1] == second[1])
    return None


def validate_replay(rec, replay, original, store):
    """Recheck the CPU anchor with the current emulator before accepting a port."""
    cpu = context_result(rec['exit'])
    cpu['trace'] = original[0]['trace']  # only the emulator supplies the call trace
    actual = {int(p): store.get(h) for p, h in rec['exit_pages'].items()}
    emulated = dict(original[1])
    for rule in rec['validation'].get('clock_rules', []):
        a = int(rule['address'], 16)
        p, off = a & -4096, a & 4095
        # Only the exact byte exceptions logged by the live validator apply.
        entry = store.get(rec['pages'][str(p)])
        for pages in (actual, emulated):
            b = bytearray(pages.get(p, entry))
            b[off] = entry[off]
            pages[p] = bytes(b)
    initial = {int(p): store.get(h) for p, h in rec['pages'].items()}
    return difference(cpu, original[0], ('al', 'ah', 'eaxhi', 'edx', 'ecx'),
                      actual, emulated, initial, rec['stack'])


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
    keep_going = getattr(args, 'keep_going', False)
    # Copy the input once: a concurrent engine build must not change our image.
    raw = args.dll.read_bytes()
    sha = hashlib.sha256(raw).hexdigest()
    dll = OUT / 'dlls' / f'{sha}.dll'
    dll.parent.mkdir(parents=True, exist_ok=True)
    dll.write_bytes(raw)
    records = {a: [read_capture(p) for p in captures_of(a)] for a in targets}
    flat = [c for cs in records.values() for c in cs]
    if not flat:
        raise ValueError('no captures for the requested functions; run capture first')
    avoid = [(c['gdt'], c['gdt']+4096) for c in flat]
    port = Port(dll, [c['regions'] for c in flat], avoid=avoid)
    store = PageStore()
    results = []
    all_start = time.perf_counter()
    for a in targets:
        start = time.perf_counter()
        cs = [c for c in records[a] if c['validation']['valid']]
        parts, evidence = channel(a, [c['ret'] for c in cs])
        result = dict(address=f'{a:08x}', name=name_of(a), owner=owner_of(a), verdict='EXACT',
                      captures=0, variants=0, skipped=0, scenarios='', dll_sha256=sha,
                      return_channel=describe(parts), channel_evidence=evidence, difference=None,
                      invalid_captures=[c['_path'] for c in records[a] if not c['validation']['valid']],
                      skips=[], seed=args.seed, requested_variants=args.variants,
                      capture_failures=0, variant_failures=0, unattempted_variants=0, unknown_call_targets=[])
        result['available_captures'] = len(cs)
        scenarios = set()
        if a not in port.exports:
            result['difference'] = dict(field='missing export', export=f'fc4p_{a:08x}')
            result['unattempted_variants'] = args.variants
        if not cs:
            result['difference'] = dict(field='no captures')
            result['unattempted_variants'] = args.variants
        next_variant = 0
        unknown_calls = set()
        progress_time = time.perf_counter()
        for capture_index, rec in enumerate(cs):
            if result['difference'] and not keep_going:
                break
            if a not in port.exports:
                break
            ident = str(Path(rec['_path']).relative_to(OUT))
            quota = args.variants // len(cs) + (capture_index < args.variants % len(cs))
            next_variant = (args.variants // len(cs))*capture_index + min(capture_index, args.variants % len(cs))
            if not rec['validation']['valid']:
                result['invalid_captures'].append(ident)
                result['difference'] = dict(capture=ident, field='invalid capture', detail=rec['validation'])
                break
            result['captures'] += 1
            scenarios.add(rec['scenario'])
            r = Replay(rec, store, port, targets)
            original, reads, nondet = twice(r, False, parts, collect=True)
            unknown_calls.update(c['target'] for c in original[0]['trace'] if c['arguments'] is None)
            if nondet:
                result['difference'] = result['difference'] or dict(capture=ident, field='original replay nondeterminism', detail=nondet)
                result['capture_failures'] += 1
                if keep_going:
                    result['unattempted_variants'] += quota
                    continue
                break
            if original[0]['err']:
                result['difference'] = result['difference'] or dict(capture=ident, field='original replay failed', detail=original[0]['err'])
                result['capture_failures'] += 1
                if keep_going:
                    result['unattempted_variants'] += quota
                    continue
                break
            invalid = validate_replay(rec, r, original, store)
            if invalid:
                result['difference'] = result['difference'] or dict(capture=ident, field='CPU validation failed with current emulator', detail=invalid)
                result['capture_failures'] += 1
                if keep_going:
                    result['unattempted_variants'] += quota
                    continue
                break
            ours, _, nondet = twice(r, True, parts)
            diff = nondet or same(r, original, ours, parts)
            if diff:
                n = CALL_INPUTS.entry_arity(rec)
                esp = rec['entry']['regs']['esp']
                words = [] if n == 0 else None
                if n and r.m.ensure(esp+4, n*4):
                    words = [f'{v:08x}' for v in struct.unpack('<'+'I'*n, r.m.read(esp+4, n*4))]
                result['capture_failures'] += 1
                result['difference'] = result['difference'] or dict(capture=ident, input=dict(kind='captured', stack_arguments=words,
                    registers={k: f'{v:08x}' for k, v in rec['entry']['regs'].items()}), **diff)
                if not keep_going:
                    break
            for inp, state, writes in variants(rec, r, reads, quota, args.seed, start=next_variant):
                if time.perf_counter()-progress_time >= 20:
                    print(f'{a:08x}: capture {capture_index+1}/{len(cs)}, input {inp["id"]}/{args.variants}; '
                          f'{result["variants"]} passing, {result["variant_failures"]} different, {result["skipped"]} skipped', flush=True)
                    progress_time = time.perf_counter()
                r.m.state = state
                try:
                    for addr, data in writes:
                        r.m.ensure(addr, len(data))
                        r.m.write(addr, data)
                    # restore() returns to the capture. Reapply variant memory
                    # before every replay of both implementations.
                    def run_variant(is_port):
                        for addr, data in writes:
                            r.m.write(addr, data)
                        out = r.run(port=is_port, timeout=2)
                        r.m.restore()
                        return out
                    one = run_variant(False)
                    if one[0]['err']:
                        result['skipped'] += 1
                        result['skips'].append(dict(capture=ident, input=inp, reason=one[0]['err']))
                        continue
                    two = run_variant(False)
                    diff = deterministic_difference(one, two)
                    if diff:
                        result['difference'] = result['difference'] or dict(capture=ident, input=inp, field='original variant nondeterminism', detail=diff)
                        result['variant_failures'] += 1
                        if keep_going:
                            continue
                        break
                    ours = run_variant(True)
                    repeated = run_variant(True)
                    diff = deterministic_difference(ours, repeated) or same(r, one, ours, parts)
                    if diff:
                        result['difference'] = result['difference'] or dict(capture=ident, input=inp, **diff)
                        result['variant_failures'] += 1
                        if keep_going:
                            continue
                        break
                    result['variants'] += 1
                finally:
                    r.m.restore()
                    r.m.state = rec['entry']
            if result['difference'] and not keep_going:
                break
        if result['difference']:
            result['verdict'] = 'DIFFERENT'
        result['scenarios'] = ','.join(sorted(scenarios))
        result['unknown_call_targets'] = sorted(unknown_calls)
        result['seconds'] = time.perf_counter()-start
        results.append(result)
        print(json.dumps({k: v for k, v in result.items() if k != 'skips'}), flush=True)
    store.close()
    report = args.report or OUT / 'checks' / f'{time.strftime("%Y%m%d-%H%M%S")}-{sha[:8]}.json'
    report.parent.mkdir(parents=True, exist_ok=True)
    report.write_text(json.dumps(dict(dll=str(args.dll), sha256=sha, mapped_base=hex(port.base),
                                     seconds=time.perf_counter()-all_start, results=results,
                                     call_input_decisions=CALL_INPUTS.decisions), indent=2))
    if not args.no_proven:
        update_proven(results)
    print(f'report: {report}', flush=True)
    return int(any(r['verdict'] != 'EXACT' for r in results))
