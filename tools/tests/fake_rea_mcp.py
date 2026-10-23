"""Harmless process-local MCP fixture. No original binary/code is read."""
import json
import os
import sys
import time

OPEN_SCHEMA = {'type': 'object', 'properties': {'path': {'type': 'string'},
               'project': {'type': 'string'}, 'read_only': {'type': 'boolean'}},
               'required': ['path', 'project', 'read_only']}
TOOLS = [dict(name='open_binary', inputSchema=OPEN_SCHEMA),
         dict(name='app_identity', inputSchema={'type': 'object', 'properties': {}}),
         dict(name='procedure_pseudo_code', inputSchema={'type': 'object',
              'properties': {'address': {'type': 'string'}}}),
         dict(name='batch_decompile', inputSchema={'type': 'object',
              'properties': {'addresses': {'type': 'array', 'items': {'type': 'string'}}}}),
         dict(name='close_app', inputSchema={'type': 'object'})]


def main():
    mode = os.environ.get('FAKE_REA_MODE', '')
    target = json.loads(os.environ.get('FAKE_REA_EXISTING', 'null'))
    for line in sys.stdin:
        r = json.loads(line)
        if 'id' not in r:
            continue
        if mode == 'timeout':
            time.sleep(10)
        if mode == 'eof':
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
            if name == 'app_identity':
                value = dict(state='empty', ready=False) if target is None else target
                if mode == 'identity_error':
                    value = {'error': 'identity unavailable'}
                if mode == 'untyped_ready' and target:
                    value = value | {'ready': 'true'}
                drift = os.environ.get('FAKE_REA_DRIFT_FILE')
                if drift and os.path.exists(drift) and target:
                    value = value | {'binary': target['binary']+'-wrong'}
            elif name == 'open_binary':
                value = {'opened': mode not in ('open_error', 'negative', 'string_success')}
                if mode == 'open_error':
                    print(json.dumps(dict(jsonrpc='2.0', id=r['id'], result=dict(isError=True,
                          content=[dict(type='text', text='deliberate open failure')]))), flush=True)
                    continue
                if mode == 'open_rpc_error':
                    print(json.dumps(dict(jsonrpc='2.0', id=r['id'], error=dict(code=-32000,
                          message='deliberate RPC open failure'))), flush=True)
                    continue
                if mode == 'string_success':
                    value['opened'] = 'true'
                if value['opened'] is True:
                    target = dict(state='ready', ready=True, binary=args['path'], project=args['project'])
                    if mode == 'wrong_after_open':
                        target['binary'] += '-wrong'
            else:
                value = {'fixture': 'research', 'pid': os.getpid()} if target else {'error': 'No app is open'}
            result = dict(structuredContent=value, content=[])
        else:
            print(json.dumps(dict(jsonrpc='2.0', id=r['id'], error={'code': -32601, 'message': method})), flush=True)
            continue
        print(json.dumps(dict(jsonrpc='2.0', id=r['id'], result=result)), flush=True)


if __name__ == '__main__':
    main()
