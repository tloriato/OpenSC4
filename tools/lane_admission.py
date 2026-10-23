"""Host-issued admission provenance. Never reruns host discovery in a worker.

The registered host hook verifies the MAC using a host-only key; no key or
authority is exported to the isolated worktree. Normal per-call guard remains
mandatory. This assertion attests admission, not successful live research.
"""
import hashlib
import hmac
import json
from pathlib import Path
import secrets
import time


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':')).encode()


def subject(m):
    ticket = m.get('rea_session')
    return dict(lane=m['lane'], round=m['round'], kind=m['kind'], provider=m['provider'],
                worktree=str(Path(m['worktree']).resolve()), guard_id=m['guard_id'],
                manifest=str(Path(m['manifest']).resolve()), source=m['source'],
                args_sha256=hashlib.sha256(canonical(m['args'])).hexdigest(),
                session_sha256=hashlib.sha256(canonical(ticket)).hexdigest())


def key_path(m):
    return Path(m['main'])/'build-runs/lane-guard/host-admission.key'


def issue(m, checks, now=None):
    """Host only, after full dispatch checks and before isolated execution."""
    if not checks or any(c.get('verdict') != 'GO' for c in checks):
        raise ValueError('Full host GO evidence required')
    p = key_path(m)
    p.parent.mkdir(parents=True, exist_ok=True)
    if not p.exists():
        try:
            with p.open('xb') as f:
                f.write(secrets.token_bytes(32))
        except FileExistsError:
            pass
    at = time.time() if now is None else now
    body = dict(version=1, subject=subject(m), checks=checks, issued=at, expires=at+24*3600)
    return dict(body=body, mac=hmac.new(p.read_bytes(), canonical(body), hashlib.sha256).hexdigest())


def verify(m, now=None):
    """Registered host guard only; no npm checks, Git refs or new preflight."""
    proof = m.get('host_admission', {})
    try:
        body = proof['body']
        expected = hmac.new(key_path(m).read_bytes(), canonical(body), hashlib.sha256).hexdigest()
        at = time.time() if now is None else now
        if not hmac.compare_digest(expected, proof['mac']) or body['version'] != 1 or \
                body['subject'] != subject(m) or not body['issued'] <= at < body['expires'] or \
                not 0 < body['expires']-body['issued'] <= 24*3600 or \
                {c['name'] for c in body['checks']} != {'dispatcher', 'lanes.check', 'supervisor'} or \
                any(c['verdict'] != 'GO' for c in body['checks']):
            raise ValueError('mismatch')
    except (KeyError, TypeError, ValueError, OSError) as e:
        raise ValueError('Invalid/expired inherited host admission provenance') from e
    return body
