"""Reserve and launch one lane round, or show live workers. Supports Claude and Codex.

Existing run4 briefs/ledger are unchanged; provider selection never resets round counts.
"""
import argparse
import ctypes
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import uuid

import lane_backend as backend
import rea_session


def alive(pid):
    if not pid:
        return False
    if os.name != 'nt':
        try:
            os.kill(int(pid), 0)
            return True
        except OSError:
            return False
    k = ctypes.WinDLL('kernel32', use_last_error=True)
    k.OpenProcess.restype = ctypes.c_void_p
    k.GetExitCodeProcess.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_ulong)]
    k.CloseHandle.argtypes = [ctypes.c_void_p]
    h = k.OpenProcess(0x1000, False, int(pid))
    if not h:
        return False
    value = ctypes.c_ulong()
    try:
        return bool(k.GetExitCodeProcess(h, ctypes.byref(value))) and value.value == 259
    finally:
        k.CloseHandle(h)


def provider_pause(root, provider):
    path = root/'build-runs/provider-pauses'/f'{provider}.json'
    if path.exists():
        return path
    legacy = root/'build-runs/run4-refill-paused.json'
    if legacy.exists():
        value = backend.laneguard.read_json(str(legacy), {})
        if value.get('provider', 'claude') == provider:
            return legacy
    return None


def retire_completed(state, root, write_queues=True):
    for name, entry in state['lanes'].items():
        p = Path(entry.get('result', ''))
        completed = backend.laneguard.read_json(str(p), None) if p.is_file() else None
        if entry.get('pid') and isinstance(completed, dict) and ('result' in completed or 'subtype' in completed):
            entry['finished_pid'] = entry.pop('pid')
            models = [completed['model']] if completed.get('model') else list(completed.get('modelUsage', {}))
            entry['models'] = models
            for record in reversed(state.get('history', [])):
                if record.get('lane') == name and record.get('lane_round') == entry['rounds']:
                    record.update(provider=entry.get('provider', completed.get('provider', 'claude')), models=models)
                    break
        for qp in [root/'build-runs/merge-queue'/f'{name}.json',
                   root/'.claude/worktrees'/name/'build-runs/merge-queue'/f'{name}.json']:
            q = backend.laneguard.read_json(str(qp), None)
            if write_queues and isinstance(q, dict) and entry.get('touches') and q.get('touches') != entry['touches']:
                q['touches'] = entry['touches']
                backend.write_json(qp, q)


def validate_capacity(state, spec, live, disk_fraction):
    name = spec['args']['name']
    entry = state['lanes'].get(name, {'rounds': 0})
    if not backend.re.fullmatch(r'[a-z0-9][a-z0-9_-]*', name):
        raise ValueError('Invalid lane name')
    if state.get('limit') is not None and state['rounds'] >= state['limit']:
        raise ValueError('Run total round cap')
    if disk_fraction > .85:
        raise ValueError('C: exceeds85%; clean landed worktrees before spawning')
    if entry['rounds'] >= entry.get('round_limit', state['per_lane_limit']) or entry.get('parked'):
        raise ValueError('Lane round cap / parked')
    if entry.get('waiting_for'):
        raise ValueError('Lane is waiting for a documented blocker to be fixed')
    if name in live:
        raise ValueError('Lane already running')
    touches = {k.lower() for k in spec.get('touches', [])}
    if not touches:
        raise ValueError('Every lane must declare touches')
    overlaps = [n for n in live if touches.intersection(k.lower() for k in state['lanes'][n].get('touches', []))]
    if overlaps:
        raise ValueError('Active subsystem reservation: ' + ','.join(overlaps))
    if len(live) >= state['concurrency']:
        raise ValueError('Live lane cap reached')
    role = spec.get('lane_role', entry.get('role', 'glue' if name.startswith('glue') else
                                         ('re' if spec['args']['kind'] == 're' else 'verify')))
    if role == 'glue' and sum(state['lanes'][n].get('role') == 'glue' for n in live) >= state.get('glue_limit', 3):
        raise ValueError('Glue lane cap reached')
    if spec['args'].get('unguarded'):
        raise ValueError('Unguarded workflow fallback is forbidden')
    return entry, role


def checked(command, root):
    p = subprocess.run(command, cwd=root, capture_output=True, text=True, encoding='utf-8',
                       errors='replace', creationflags=backend.NOWIN)
    print(p.stdout[:2500].strip(), flush=True)
    if p.returncode:
        raise RuntimeError((p.stdout + p.stderr)[-2000:].strip())


def launch(root, state_path, spec_path=None, provider=None):
    sessions = []
    try:
        return _launch(root, state_path, spec_path, provider, sessions)
    finally:
        for ticket, process in sessions:
            rea_session.shutdown(ticket)
            process.wait(timeout=10)


def _launch(root, state_path, spec_path, provider, sessions):
    # Reject bad sources before even retiring completed lanes or writing coordinator state.
    import lane_evidence
    spec = json.loads(spec_path.read_text(encoding='utf-8')) if spec_path is not None else None
    if spec is not None:
        lane_evidence.preflight(root, spec, spec_path)
    with backend.codexguard.locked(root/'build-runs/run4-coordinator.lock'):
        state = json.loads(state_path.read_text(encoding='utf-8'))
        retire_completed(state, root, write_queues=spec_path is None)
        live = [n for n, e in state['lanes'].items() if alive(e.get('pid'))]
        if spec_path is None:
            backend.write_json(state_path, state)
            print(f"rounds {state['rounds']}; live {len(live)}/{state['concurrency']}; "
                  f"glue {sum(state['lanes'][n].get('role') == 'glue' for n in live)}/{state.get('glue_limit',3)}")
            for n in live:
                print(n, state['lanes'][n].get('provider', 'claude'), 'round', state['lanes'][n]['rounds'])
            return
        provider = provider or spec.get('provider') or state.get('provider', 'claude')
        if provider not in ('claude', 'codex'):
            raise ValueError('Unknown provider')
        if provider_pause(root, provider):
            raise ValueError(provider + ' provider paused; audit its recorded limit/error before resuming')
        disk = shutil.disk_usage(root)
        entry, role = validate_capacity(state, spec, live, disk.used / disk.total)
        args = dict(spec['args'])
        name = args['name']
        checked([sys.executable, str(root/'tools/lanes.py'), 'slots', '--game-slots', '6'], root)
        check = [sys.executable, str(root/'tools/lanes.py'), 'check', name, '--kind', args['kind'],
                 '--touches', ','.join(spec['touches'])]
        if spec.get('resume'):
            check.append('--resume')
        if args.get('deps'):
            check += ['--deps', ','.join(args['deps']) if isinstance(args['deps'], list) else args['deps']]
        ticket = None
        if args['kind'] == 're':
            # STOP/parking/capacity first. No ledger/queue/worktree/round mutation
            # until the exact actual worker client has a typed target identity.
            code, line = backend.laneguard.preflight(str(root), name, 'tool')
            if code:
                raise ValueError(line)
            ticket, owned = rea_session.acquire(root, name, provider, 're', backend.laneguard.rea_config())
            sessions.append((ticket, owned))
            admission = root/'build-runs'/f'rea-admission-{ticket["session_id"]}.json'
            backend.write_json(admission, ticket)
            check += ['--rea-session', str(admission)]
        checked(check, root)
        retire_completed(state, root)
        sources = lane_evidence.preflight(root, spec, spec_path)
        wt = root/'.claude/worktrees'/name
        if not spec.get('resume'):
            checked(['git', 'worktree', 'add', str(wt), '-b', name, 'main'], root)
            sdk = wt/'engine/third_party/gzcom-dll'
            sdk.mkdir(parents=True, exist_ok=True)
            data = subprocess.check_output(['git', '-C', str(root/'engine/third_party/gzcom-dll'), 'archive', 'HEAD'])
            with tarfile.open(fileobj=io.BytesIO(data)) as tf:
                tf.extractall(sdk, filter='data')
            (sdk/'.git').write_text('gitdir: ' + (root/'.git/modules/engine/third_party/gzcom-dll').as_posix() + '\n')
        evidence = wt/'build-runs/evidence-run4'
        evidence.mkdir(parents=True, exist_ok=True)
        for index, p in enumerate(sources):
            data = lane_evidence.read_source(p, spec_path, index, spec['evidence'][index])
            (evidence/p.name).write_bytes(data)
        args.update(maxRounds=1, main=root.as_posix())
        args.setdefault('ctxBudget', 150000)
        args.setdefault('maxCalls', 150)
        if args['kind'] == 'engine':
            args['task'] += (' For every new game request choose a unique label and wait without qwait --fresh; '
                             'an already completed verdict for that unique label is valid and must not be discarded.')
        number = entry['rounds'] + 1
        stem = root/'build-runs'/f'run4-{name}-r{number}'
        manifest = Path(str(stem)+'-manifest.json')
        result = Path(str(stem)+'-result.json')
        stderr = Path(str(stem)+'-stderr.txt')
        if result.exists() or manifest.exists():
            raise ValueError('Round artifact already exists; reconcile ledger before launching')
        m = dict(provider=provider, main=str(root), worktree=str(wt), state=str(state_path), lane=name,
                 kind=args['kind'], round=number, args=args, budget=args['ctxBudget'], max_calls=args['maxCalls'],
                 guard_id='codex-'+uuid.uuid4().hex, manifest=str(manifest), result=str(result),
                 status_path=str(stem)+'-codex-status.json', codex=spec.get('codex', {}))
        if ticket:
            m['rea_session'] = ticket
        backend.write_json(manifest, m)
        # Supervisor stays alive until the CLI ends; it writes one atomic, compatible completion result.
        with stderr.open('wb') as err:
            p = subprocess.Popen([sys.executable, str(root/'tools/lane_backend.py'), '--manifest', str(manifest)],
                                 cwd=root, stdin=subprocess.DEVNULL, stdout=err, stderr=err,
                                 creationflags=backend.NOWIN)
        entry.update(role=role, touches=spec['touches'], rounds=number, pid=p.pid, result=str(result),
                     stderr=str(stderr), provider=provider, manifest=str(manifest))
        state['rounds'] += 1
        state['lanes'][name] = entry
        state['history'].append(dict(round=state['rounds'], lane=name, lane_round=number, pid=p.pid, provider=provider))
        backend.write_json(state_path, state)
        # The supervisor now owns cleanup. Failed admission never reaches here.
        sessions.clear()
        print(f"LAUNCHED run4 round {state['rounds']}; {name} round {number}/{entry.get('round_limit',3)}; "
              f"provider {provider}; pid {p.pid}", flush=True)


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('spec', type=Path, nargs='?')
    ap.add_argument('--provider', choices=('claude', 'codex'))
    ap.add_argument('--root', type=Path, default=Path(backend.laneguard.main_root()))
    ap.add_argument('--state', type=Path)
    a = ap.parse_args()
    try:
        root = a.root.resolve()
        launch(root, (a.state or root/'build-runs/run4-coordinator-state.json').resolve(), a.spec, a.provider)
        return 0
    except Exception as e:
        print(f'{type(e).__name__}: {e}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())
