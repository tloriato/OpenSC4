"""Independent capture/variant work, with ordered reduction in the parent process."""
import copy
import gc
import json
import struct
import time
from concurrent.futures import ProcessPoolExecutor, as_completed
from pathlib import Path

from .common import OUT
from .store import PageStore, read_capture
from .replay import Replay
from .variants import variants
from .calls import CALL_INPUTS
from .check import twice, same, validate_replay, deterministic_difference

_STORE = _CONFIG = None
_CAPTURE_CACHE = None


def initialize(config):
    global _STORE, _CONFIG, _CAPTURE_CACHE
    _CONFIG = config
    _STORE = PageStore(config['store_path'], readonly=True)
    _CAPTURE_CACHE = None


def operational(out, item):
    if out[0].get('kind') == 'timeout':
        raise RuntimeError(f"worker watchdog expired: {item['path']}, batch {item['start']}; "
                           'no proof rows were written; increase --replay-timeout or reduce --jobs')


def execute(item):
    """A capture and a bounded slice of its globally numbered variants."""
    global _CAPTURE_CACHE
    started = time.perf_counter()
    config, store = _CONFIG, _STORE
    rec = read_capture(item['path'])
    ident = str(Path(rec['_path']).relative_to(OUT))
    first = item['first']
    result = dict(captures=int(first), variants=0, skipped=0, deadline_skips=0,
                  capture_failures=0, variant_failures=0, unattempted_variants=0,
                  difference=None, skips=[], unknown_call_targets=[], scenarios=[rec['scenario']] if first else [])
    result.update(observer_exclusions=[], observer_excluded_byte_events=0)
    CALL_INPUTS.observed = {}
    r = None
    try:
        cache_key = (item['path'], Path(item['path']).stat().st_mtime_ns)
        cached = _CAPTURE_CACHE if _CAPTURE_CACHE and _CAPTURE_CACHE[0] == cache_key else None
        if cached:
            _,r,original,reads,decisions,sites = cached
            CALL_INPUTS.observed = copy.deepcopy(decisions)
            r.original_sites = list(sites)
            r.observer_exclusions = {}
            r.observer_excluded_byte_events = 0
        else:
            _CAPTURE_CACHE = None  # keep at most one machine per worker
            r = Replay(rec, store, config['port'], config['enabled'])
        limitation = r.observer.reference_limitation(rec, r.m.provider)
        if limitation:
            result['unattempted_variants'] = item['count']
            if first:
                result['difference'] = dict(capture=ident, **limitation)
            return finish(result, started, r)
        parts = item['parts']
        if cached:
            nondet = None
        else:
            original, reads, nondet = twice(r, False, parts, collect=True,
                                            timeout=config['timeout'])
        operational(original, item)
        result['unknown_call_targets'] = sorted({c['target'] for c in original[0]['trace'] if c['arguments'] is None})
        error = (dict(field='original replay nondeterminism', detail=nondet) if nondet else
                 dict(field='original replay failed', detail=original[0]['err']) if original[0]['err'] else None)
        if error is None and not cached:
            invalid = validate_replay(rec, r, original, store)
            if invalid:
                error = dict(field='CPU validation failed with current emulator', detail=invalid)
        if error:
            result['capture_failures'] = int(first)
            result['unattempted_variants'] = item['count']
            if first:
                result['difference'] = dict(capture=ident, **error)
            return finish(result, started, r)
        if not cached:
            # A scarce, expensive capture can have many variant batches. Its
            # frozen CPU-validated baseline need only run twice per worker,
            # not twice again for every slice of the same captured inputs.
            _CAPTURE_CACHE = (cache_key,r,original,reads,copy.deepcopy(CALL_INPUTS.observed),list(r.original_sites))
        if first:
            ours, _, nondet = twice(r, True, parts, timeout=config['timeout'])
            operational(ours, item)
            diff = nondet or same(r, original, ours, parts)
            if diff:
                n = CALL_INPUTS.entry_arity(rec)
                esp = rec['entry']['regs']['esp']
                words = [] if n == 0 else None
                if n and r.m.ensure(esp+4, n*4):
                    words = [f'{v:08x}' for v in struct.unpack('<'+'I'*n, r.m.read(esp+4, n*4))]
                result['capture_failures'] = 1
                result['difference'] = dict(capture=ident, input=dict(kind='captured', stack_arguments=words,
                    registers={k:f'{v:08x}' for k,v in rec['entry']['regs'].items()}), **diff)
                if not config['keep_going']:
                    return finish(result, started, r)
        for inp, state, writes in variants(rec, r, reads, item['count'], config['seed'], start=item['start']):
            r.m.state = state
            try:
                def run_variant(is_port):
                    for addr, data in writes:
                        r.m.ensure(addr, len(data))
                        r.m.write(addr, data)
                    out = r.run(port=is_port, timeout=config['timeout'],
                                instruction_budget=(config.get('port_variant_instructions',config['variant_instructions'])
                                                    if is_port else config['variant_instructions']))
                    r.m.restore()
                    operational(out, item)
                    return out
                one = run_variant(False)
                if one[0]['err']:
                    result['skipped'] += 1
                    result['deadline_skips'] += one[0].get('kind') == 'instruction_limit'
                    result['skips'].append(dict(capture=ident, input=inp, reason=one[0]['err'], kind=one[0].get('kind')))
                    continue
                two = run_variant(False)
                diff = deterministic_difference(one, two)
                if diff:
                    detail = dict(field='original variant nondeterminism', detail=diff)
                else:
                    ours, repeated = run_variant(True), run_variant(True)
                    detail = deterministic_difference(ours, repeated) or same(r, one, ours, parts)
                if detail:
                    result['difference'] = result['difference'] or dict(capture=ident, input=inp, **detail)
                    result['variant_failures'] += 1
                    if not config['keep_going']:
                        break
                else:
                    result['variants'] += 1
            finally:
                r.m.restore()
                r.m.state = rec['entry']
        return finish(result, started, r)
    finally:
        CALL_INPUTS.observed = None
        r = None
        gc.collect()


def finish(result, started, replay=None):
    ended = time.perf_counter()
    result['seconds'] = ended-started
    result['_timing'] = [started, ended]
    result['call_input_decisions'] = copy.deepcopy(CALL_INPUTS.observed)
    if replay is not None:
        result['observer_exclusions'] = [dict(capture=str(Path(replay.rec['_path']).relative_to(OUT)), **rule)
                                         for rule in replay.observer_exclusions.values()]
        result['observer_excluded_byte_events'] = replay.observer_excluded_byte_events
    return result


def run_items(items, config, jobs):
    """Completion order affects progress only; return values retain input order."""
    path = config.get('journal')
    journal = open(path, 'w', encoding='utf-8') if path else None
    if journal:
        journal.write(json.dumps(dict(items=items, jobs=jobs, dll_sha256=config['dll_sha256'],
                                     tool_files=config.get('tool_files'))) + '\n')
        journal.flush()
    def completed(index, result):
        if journal:
            journal.write(json.dumps(dict(index=index, result=result))+'\n')
            journal.flush()
    if jobs == 1:
        initialize(config)
        try:
            results, stopped = [], set()
            for i,item in enumerate(items):
                if item['result'] in stopped:
                    results.append(None)
                    continue
                results.append(execute(item))
                completed(i, results[-1])
                if results[-1]['difference'] and not config['keep_going']:
                    stopped.add(item['result'])
            return results
        finally:
            _STORE.close()
            if journal:
                journal.close()
    results = [None]*len(items)
    progress = time.perf_counter()
    pool = ProcessPoolExecutor(max_workers=jobs, initializer=initialize, initargs=(config,))
    try:
        pending = {pool.submit(execute, item): i for i,item in enumerate(items)}
        by_result = {}
        for future,i in pending.items():
            by_result.setdefault(items[i]['result'], []).append((i,future))
        done = 0
        for future in as_completed(pending):
            i = pending[future]
            if future.cancelled():
                done += 1
                continue
            results[i] = future.result()
            completed(i, results[i])
            if results[i]['difference'] and not config['keep_going']:
                # Earlier work still determines the first difference. Only
                # strictly later items of this function can be discarded.
                for j,later in by_result[items[i]['result']]:
                    if j > i:
                        later.cancel()
            done += 1
            if time.perf_counter()-progress >= 15:
                print(f'check: {done}/{len(items)} capture batches complete ({jobs} workers)', flush=True)
                progress = time.perf_counter()
    except BaseException:
        for future in pending:
            future.cancel()
        # A failed operational watchdog must not wait for every queued batch
        # before reporting the failure. Workers own no capture/proof writes.
        for process in list(pool._processes.values()):
            process.terminate()
        pool.shutdown(wait=True, cancel_futures=True)
        raise
    else:
        pool.shutdown(wait=True)
    finally:
        if journal:
            journal.close()
    return results


def merge_decisions(destination, source):
    for target, decision in source.items():
        if target not in destination:
            destination[target] = {**decision, 'caller_sites': dict(decision['caller_sites'])}
        else:
            old = destination[target]
            if {k:v for k,v in old.items() if k!='caller_sites'} != {k:v for k,v in decision.items() if k!='caller_sites'}:
                raise ValueError(f'inconsistent call-input decision for {target}')
            old['caller_sites'].update(decision['caller_sites'])
        destination[target]['caller_sites'] = dict(sorted(destination[target]['caller_sites'].items()))
