"""Host-issued admission, pinned in the host-launched hook command.

No signing secret exists on disk or in worker metadata. The trust anchor is the
expected SHA256 literal in the host's per-process CLI hook configuration, never
a hash taken from the worker-readable manifest. Normal guard remains mandatory.
"""
import hashlib
import json
from pathlib import Path
import time


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':')).encode()


def subject(m):
    ticket = m.get('rea_session')
    return dict(lane=m['lane'], round=m['round'], kind=m['kind'], provider=m['provider'],
                main=str(Path(m['main']).resolve()),
                worktree=str(Path(m['worktree']).resolve()), guard_id=m['guard_id'],
                manifest=str(Path(m['manifest']).resolve()), source=m['source'],
                runtime={k:m.get(k) for k in ('budget','max_calls','status_path','state','git_common',
                                             'git_dir','git_base','git_ref','codex')},
                args_sha256=hashlib.sha256(canonical(m['args'])).hexdigest(),
                session_sha256=hashlib.sha256(canonical(ticket)).hexdigest())


def registration_path(m):
    return Path(m['main'])/'.claude/lane-admissions'/(m['guard_id']+'.json')


def pin(proof):
    return hashlib.sha256(canonical(proof)).hexdigest()


def issue(m, checks, now=None):
    """Host only, after full dispatch checks and before isolated execution."""
    if not checks or any(c.get('verdict') != 'GO' for c in checks):
        raise ValueError('Full host GO evidence required')
    at = time.time() if now is None else now
    return dict(version=2, subject=subject(m), checks=checks, issued=at, expires=at+24*3600)


def verify(m, expected_digest=None, now=None):
    """Registered host guard only; no npm checks, Git refs or new preflight."""
    proof = m.get('host_admission', {})
    try:
        body = proof
        at = time.time() if now is None else now
        if not isinstance(expected_digest, str) or pin(body) != expected_digest or body['version'] != 2 or \
                body['subject'] != subject(m) or not body['issued'] <= at < body['expires'] or \
                not 0 < body['expires']-body['issued'] <= 24*3600 or \
                {c['name'] for c in body['checks']} != {'dispatcher', 'lanes.check', 'supervisor'} or \
                any(c['verdict'] != 'GO' for c in body['checks']):
            raise ValueError('mismatch')
    except (KeyError, TypeError, ValueError, OSError) as e:
        raise ValueError('Invalid/expired inherited host admission provenance') from e
    return body
