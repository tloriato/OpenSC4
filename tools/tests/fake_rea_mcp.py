"""Harmless process-local MCP fixture. No original binary/code is read."""
import json
import hashlib
import os
from pathlib import Path
import subprocess
import sys
import time

TOOLS = json.loads((Path(__file__).parent/'fixtures/rea-installed-schemas.json').read_text())


def sample(s, root=None):
    root = s if root is None else root
    if '$ref' in s:
        return sample(root['$defs'][s['$ref'][8:]], root)
    for union in ('anyOf', 'oneOf'):
        if union in s:
            return sample(s[union][0], root)
    if 'const' in s:
        return s['const']
    if 'enum' in s:
        return s['enum'][0]
    kind = s.get('type')
    if kind == 'object':
        return {k: sample(s['properties'][k], root) for k in s.get('required', [])}
    if kind == 'array':
        return []
    if kind == 'string':
        return '0'*64 if '64' in s.get('pattern', '') else 'fixture'
    return {'boolean': False, 'integer': 1, 'number': 1, 'null': None}.get(kind)


def identity(target):
    schema = next(t['outputSchema'] for t in TOOLS if t['name'] == 'binary_session')
    value = sample(schema)['result']
    value['server_identity']['negotiated_protocol_version'] = '2025-06-18'
    # Actual server-side witness, distinct from a Windows venv redirector PID.
    value['server_identity']['server']['started_at'] = str(os.getpid())
    value['analysis_provider_binding']['selection_source'] = 'auto-single-candidate'
    availability = schema['$defs']['__schema19']['items']
    value['tool_availability'] = [sample(availability, schema) | {'name': n}
                                 for n in ('procedure_pseudo_code', 'batch_decompile')]
    if target is not None:
        value.update(target)
    return value


def main():
    mode = os.environ.get('FAKE_REA_MODE', '')
    target = json.loads(os.environ.get('FAKE_REA_EXISTING', 'null'))
    if os.environ.get('FAKE_REA_SPAWN_CHILD'):
        child = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(120)'])
        with open(os.environ['FAKE_REA_AUDIT'], 'a', encoding='utf-8') as f:
            f.write(json.dumps(dict(name='spawn_child', pid=os.getpid(), arguments={}, grandchild_pid=child.pid))+'\n')
    for line in sys.stdin:
        r = json.loads(line)
        if 'id' not in r:
            continue
        if mode == 'timeout':
            time.sleep(10)
        if mode == 'notifications':
            while True:
                print(json.dumps(dict(jsonrpc='2.0', method='notifications/progress')), flush=True)
        if mode == 'eof':
            return
        if mode == 'stderr_eof':
            print('fixture server startup diagnostic', file=sys.stderr, flush=True)
            return
        if mode == 'malformed':
            print('not JSON', flush=True)
            continue
        method, params = r['method'], r.get('params', {})
        if method == 'initialize':
            result = dict(protocolVersion='2025-06-18', capabilities={'tools': {}},
                          serverInfo=dict(name='fake-process-local', version='1'))
            if mode == 'wrong_protocol':
                result['protocolVersion'] = 'unsupported'
        elif method == 'tools/list':
            result = {'tools': [t for t in TOOLS if mode != 'unsupported' or t['name'] != 'open_binary']}
        elif method == 'tools/call':
            name, args = params['name'], params.get('arguments', {})
            audit = os.environ.get('FAKE_REA_AUDIT')
            if audit:
                with open(audit, 'a', encoding='utf-8') as f:
                    f.write(json.dumps(dict(pid=os.getpid(), name=name, arguments=args))+'\n')
            if name == 'binary_session':
                value = identity(target)
                if mode == 'identity_error':
                    value = {'error': 'identity unavailable'}
                if mode == 'untyped_ready' and target:
                    value = value | {'open': 'true'}
                if mode == 'provider_missing' and target:
                    value['analysis_provider_binding'] = None
                drift = os.environ.get('FAKE_REA_DRIFT_FILE')
                if drift and os.path.exists(drift) and target:
                    value = value | {'path': target['path']+'-wrong'}
            elif name == 'open_binary':
                value = dict(path=args['path'], format='pe', kind='executable', architecture='x86',
                             sha256=hashlib.sha256(Path(args['path']).read_bytes()).hexdigest())
                if mode == 'open_error':
                    print(json.dumps(dict(jsonrpc='2.0', id=r['id'], result=dict(isError=True,
                          content=[dict(type='text', text='deliberate open failure')]))), flush=True)
                    continue
                if mode == 'open_rpc_error':
                    print(json.dumps(dict(jsonrpc='2.0', id=r['id'], error=dict(code=-32000,
                          message='deliberate RPC open failure'))), flush=True)
                    continue
                if mode == 'string_success':
                    value['architecture'] = True
                if mode == 'negative':
                    value = {'error': 'negative open result'}
                else:
                    target = dict(open=True, **value)
                    if mode == 'wrong_after_open':
                        target['path'] += '-wrong'
            else:
                value = {'fixture': 'research', 'pid': os.getpid()} if target else {'error': 'No app is open'}
            if name in ('open_binary', 'binary_session') and 'error' not in value:
                value = {'result': value}
            result = dict(structuredContent=value, content=[])
        else:
            print(json.dumps(dict(jsonrpc='2.0', id=r['id'], error={'code': -32601, 'message': method})), flush=True)
            continue
        print(json.dumps(dict(jsonrpc='2.0', id=r['id'], result=result)), flush=True)


if __name__ == '__main__':
    main()
