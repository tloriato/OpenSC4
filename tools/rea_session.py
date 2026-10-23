"""Owned, exact-target REA admission and a two-tool MCP transport (stdlib only).

No REA tool names/response shapes are inferred. A coordinator-reviewed contract
pins the installed schemas, exact open arguments and typed identity response.
The process admitted BEFORE reservation is the process used by the worker.
"""
import argparse
import hashlib
import json
import ntpath
import os
from pathlib import Path
import queue
import secrets
import socket
import subprocess
import sys
import threading
import time

RESEARCH = ('procedure_pseudo_code', 'batch_decompile')
NOWIN = getattr(subprocess, 'CREATE_NO_WINDOW', 0)
MAX_LINE = 16 * 1024 * 1024


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':')).encode()).hexdigest()


def same_path(a, b):
    return isinstance(a, str) and isinstance(b, str) and Path(a).is_absolute() and Path(b).is_absolute() \
        and os.path.normcase(str(Path(a).resolve())) == os.path.normcase(str(Path(b).resolve()))


def validate_contract(c):
    if c.get('version') != 1 or c.get('reviewed') is not True or c.get('label') != 'Windows 1.1.641':
        raise ValueError('REA target contract requires reviewed Windows 1.1.641 identity')
    binary, project = Path(c['binary_path']), Path(c['project_path'])
    if not binary.is_absolute() or not binary.is_file() or not project.is_absolute() or not project.exists():
        raise ValueError('REA target binary/project missing or not absolute')
    with binary.open('rb') as f:
        actual = hashlib.file_digest(f, 'sha256').hexdigest()
    if actual != c['binary_sha256']:
        raise ValueError('REA target binary SHA256 mismatch')
    op = c['open']
    if op['name'] != 'open_binary' or not same_path(op['arguments'][op['binary_argument']], str(binary)):
        raise ValueError('REA open must bind open_binary to the reviewed exact binary')
    if not same_path(op['arguments'][op['project_argument']], str(project)):
        raise ValueError('REA open must bind the reviewed exact project')
    if op['arguments'].get(op['read_only_argument']) is not True:
        raise ValueError('REA open requires an explicit reviewed read-only option')
    ident = c['identity']
    if not isinstance(ident['name'], str) or ident['name'] in (*RESEARCH, 'open_binary'):
        raise ValueError('REA identity must be a separate reviewed read-only metadata tool')
    # Bootstrap identity takes no caller arguments. The coordinator must review
    # this tool's effect as well as its pinned schema; no worker can choose it.
    for key in ('ready_field', 'state_field', 'binary_field', 'project_field'):
        if not isinstance(ident[key], str) or not ident[key]:
            raise ValueError('REA identity fields must be explicit')
    for name in (*RESEARCH, op['name'], ident['name']):
        pin = c['schemas'][name]
        if not isinstance(pin, str) or len(pin) != 64:
            raise ValueError('REA requires reviewed schema SHA256 for ' + name)
    timeout = c.get('startup_timeout', 45)
    if type(timeout) not in (int, float) or not 0 < timeout <= 120:
        raise ValueError('REA startup timeout must be in (0,120] seconds')
    return c


def payload(reply):
    if not isinstance(reply, dict) or 'error' in reply:
        raise RuntimeError('REA RPC error: ' + str(reply.get('error') if isinstance(reply, dict) else reply)[:500])
    r = reply.get('result')
    if not isinstance(r, dict) or r.get('isError', False) is not False:
        raise RuntimeError('REA tool error: ' + str(r)[:500])
    value = r.get('structuredContent')
    if value is None:
        content = r.get('content', [])
        if len(content) != 1 or content[0].get('type') != 'text':
            raise RuntimeError('REA bootstrap requires one typed JSON object')
        try:
            value = json.loads(content[0]['text'])
        except (ValueError, KeyError) as e:
            raise RuntimeError('REA bootstrap returned non-JSON text') from e
    if not isinstance(value, dict) or 'error' in value:
        raise RuntimeError('REA bootstrap negative/error reply: ' + str(value)[:500])
    return value


class Client:
    """One owned actual procedure-capable child; strict newline JSON-RPC."""
    def __init__(self, cfg, diagnostics=False):
        self.diagnostics = diagnostics
        self.p = subprocess.Popen([cfg['command'], *cfg.get('args', [])], stdin=subprocess.PIPE,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=dict(os.environ, **cfg.get('env', {})),
            creationflags=NOWIN)
        self.q, self.errors, self.seq = queue.Queue(), bytearray(), 0
        self.lock = threading.RLock()
        self.threads = [threading.Thread(target=self._read, daemon=True),
                        threading.Thread(target=self._stderr, daemon=True)]
        for t in self.threads:
            t.start()

    def _stderr(self):
        while True:
            data = os.read(self.p.stderr.fileno(), 4096)
            if not data:
                return
            self.errors.extend(data)
            if len(self.errors) > 4096:
                del self.errors[:-4096]
            if self.diagnostics:
                sys.stderr.buffer.write(data)
                sys.stderr.buffer.flush()

    def _read(self):
        try:
            while True:
                line = self.p.stdout.readline(MAX_LINE + 1)
                if not line:
                    raise RuntimeError('REA EOF')
                if len(line) > MAX_LINE or not line.endswith(b'\n'):
                    raise RuntimeError('REA malformed/oversize protocol frame')
                r = json.loads(line)
                if not isinstance(r, dict) or r.get('jsonrpc') != '2.0':
                    raise RuntimeError('REA malformed JSON-RPC')
                if 'id' in r and ('result' in r) == ('error' in r):
                    raise RuntimeError('REA malformed response envelope')
                self.q.put(r)
        except Exception as e:
            self.q.put(e)

    def send(self, message):
        self.p.stdin.write((json.dumps(message)+'\n').encode())
        self.p.stdin.flush()

    def rpc(self, method, params, deadline):
        with self.lock:
            self.seq += 1
            self.send(dict(jsonrpc='2.0', id=self.seq, method=method, params=params))
            while True:
                if time.monotonic() >= deadline:
                    raise RuntimeError('REA timeout waiting for ' + method)
                try:
                    r = self.q.get(timeout=max(.001, deadline-time.monotonic()))
                except queue.Empty as e:
                    raise RuntimeError('REA timeout waiting for ' + method) from e
                if isinstance(r, Exception):
                    self.threads[1].join(timeout=.1)
                    raise RuntimeError(str(r) + '; stderr: ' + self.errors.decode(errors='replace')[-1000:]) from r
                if r.get('id') == self.seq:
                    return r
                if 'id' in r:
                    raise RuntimeError('REA unexpected RPC id/server request')

    def call(self, name, args, deadline):
        return payload(self.rpc('tools/call', dict(name=name, arguments=args), deadline))

    def close(self):
        if self.p.poll() is None:
            self.p.terminate()
        try:
            self.p.wait(timeout=3)
        except subprocess.TimeoutExpired:
            self.p.kill()
            self.p.wait(timeout=3)
        for t in self.threads:
            t.join(timeout=1)
        for stream in (self.p.stdin, self.p.stdout, self.p.stderr):
            stream.close()


class Session:
    def __init__(self, cfg, contract):
        self.contract = validate_contract(contract)
        self.client = Client(cfg, diagnostics=True)
        try:
            deadline = time.monotonic() + contract.get('startup_timeout', 45)
            self.initial = self.client.rpc('initialize', dict(protocolVersion='2025-06-18', capabilities={},
                clientInfo=dict(name='fc4-exact-target', version='1')), deadline)
            init = self.initial.get('result', {})
            if 'error' in self.initial or init.get('protocolVersion') != '2025-06-18' or \
                    not isinstance(init.get('capabilities', {}).get('tools'), dict):
                raise RuntimeError('REA initialize error/unsupported protocol or tools capability')
            self.client.send(dict(jsonrpc='2.0', method='notifications/initialized'))
            listing = self.client.rpc('tools/list', {}, deadline)
            if 'error' in listing:
                raise RuntimeError('REA tools/list error: ' + str(listing['error'])[:500])
            tools = listing.get('result', {}).get('tools', [])
            self.tools = {t['name']: t for t in tools}
            if len(self.tools) != len(tools) or listing.get('result', {}).get('nextCursor'):
                raise RuntimeError('REA duplicate/paginated schemas unsupported')
            for name, pin in contract['schemas'].items():
                if name not in self.tools or digest(self.tools[name]['inputSchema']) != pin:
                    raise RuntimeError('REA missing/unsupported reviewed schema: ' + name)
            op = contract['open']
            schema = self.tools[op['name']]['inputSchema']
            properties = schema.get('properties', {})
            for key, kind in [(op['binary_argument'], 'string'), (op['project_argument'], 'string'),
                              (op['read_only_argument'], 'boolean')]:
                if properties.get(key, {}).get('type') != kind:
                    raise RuntimeError('REA open schema lacks explicit reviewed path/read-only option: ' + key)
            if set(op['arguments']) - set(properties) or set(schema.get('required', [])) - set(op['arguments']):
                raise RuntimeError('REA open arguments do not match reviewed schema')
            if self.tools[contract['identity']['name']]['inputSchema'].get('required'):
                raise RuntimeError('REA identity schema requires unsupported arguments')
            identity = self.identity(deadline, allow_empty=True)
            if identity is None:
                opened = self.client.call(contract['open']['name'], contract['open']['arguments'], deadline)
                if opened.get(contract['open']['success_field']) is not True:
                    raise RuntimeError('REA open_binary negative or untyped success reply: ' + str(opened)[:500])
                self.identity(deadline)
        except BaseException:
            self.client.close()
            raise

    def identity(self, deadline, allow_empty=False):
        c, i = self.contract, self.contract['identity']
        r = self.client.call(i['name'], {}, deadline)
        if allow_empty and r.get(i['state_field']) == 'empty' and r.get(i['ready_field']) is False:
            # An occupied target must NEVER be overwritten, even if not ready.
            if r.get(i['binary_field']) or r.get(i['project_field']):
                raise RuntimeError('REA empty state carries a preexisting target')
            return None
        if r.get(i['state_field']) != 'ready' or r.get(i['ready_field']) is not True:
            raise RuntimeError('REA target not typed ready: ' + str(r)[:500])
        if not same_path(r.get(i['binary_field']), c['binary_path']) or \
                not same_path(r.get(i['project_field']), c['project_path']):
            raise RuntimeError('REA wrong/preexisting target identity')
        return dict(label=c['label'], binary_path=c['binary_path'], binary_sha256=c['binary_sha256'],
                    project_path=c['project_path'], ready=True, child_pid=self.client.p.pid)

    def request(self, r):
        if not isinstance(r, dict) or r.get('jsonrpc') != '2.0':
            raise ValueError('malformed worker JSON-RPC')
        method, params = r.get('method'), r.get('params', {})
        if 'id' not in r:
            if method == 'notifications/initialized':
                return None
            raise ValueError('unsupported worker notification')
        result = None
        if method == 'initialize':
            if params.get('protocolVersion') != '2025-06-18':
                raise ValueError('unsupported worker protocol version')
            result = self.initial['result'] | {'capabilities': {'tools': {}}}
        elif method == 'ping':
            result = {}
        elif method == 'tools/list':
            result = dict(tools=[self.tools[n] for n in RESEARCH])
        elif method == 'tools/call' and params.get('name') in RESEARCH:
            if set(params) - {'name', 'arguments'}:
                raise ValueError('unreviewed research call parameter')
            name, args = params['name'], params.get('arguments', {})
            properties = self.tools[name]['inputSchema'].get('properties', {})
            if not isinstance(args, dict) or set(args) - set(properties):
                raise ValueError('unreviewed research argument')
            def check(value):
                if isinstance(value, dict):
                    for k, v in value.items():
                        if any(w in k.lower() for w in ('path', 'file', 'binary', 'project', 'target', 'app', 'command')):
                            raise ValueError('target/path/command override denied')
                        check(v)
                elif isinstance(value, list):
                    for v in value:
                        check(v)
                elif isinstance(value, str) and (ntpath.isabs(value) or Path(value).is_absolute()):
                    raise ValueError('arbitrary path argument denied')
            check(args)
            with self.client.lock:
                self.identity(time.monotonic()+self.contract.get('startup_timeout', 45))
                reply = self.client.rpc('tools/call', params, time.monotonic()+600)
            return reply | {'id': r['id']}
        else:
            raise ValueError('REA capability denied: ' + str(method))
        return dict(jsonrpc='2.0', id=r['id'], result=result)


def wire_send(f, obj):
    f.write((json.dumps(obj)+'\n').encode())
    f.flush()


def wire_read(f):
    line = f.readline(MAX_LINE+1)
    if not line:
        raise RuntimeError('transport EOF')
    if len(line) > MAX_LINE or not line.endswith(b'\n'):
        raise ValueError('malformed transport frame')
    return json.loads(line)


def exchange(ticket, op):
    with socket.create_connection(('127.0.0.1', ticket['port']), timeout=125) as sock:
        with sock.makefile('rwb') as f:
            wire_send(f, dict(token=ticket['token'], op=op, session_id=ticket['session_id']))
            r = wire_read(f)
            if r.get('error'):
                raise RuntimeError(r['error'])
            if r.get('session_id') != ticket['session_id']:
                raise RuntimeError('REA session identity mismatch')
            return r


def require_ready(ticket, lane=None):
    if not ticket or (lane is not None and ticket.get('lane') != lane):
        raise ValueError('REA requires an admitted session for this lane')
    r = exchange(ticket, 'status')
    if r.get('ready') is not True or r.get('identity') != ticket.get('identity'):
        raise RuntimeError('REA admitted target identity changed')
    return r


def shutdown(ticket):
    if ticket:
        try:
            exchange(ticket, 'shutdown')
        except (OSError, RuntimeError):
            pass  # already closed by EOF/cancellation/attach timeout


def owned_windows_job():
    """Put this broker and future descendants in a kill-on-close job.

    Must run BEFORE launching REA. Closing/killing the broker then cannot leave
    an owned JVM/server descendant behind, and cannot affect a keeper/other lane.
    Failure to establish ownership fails admission. Handle stays alive to exit.
    """
    if os.name != 'nt':
        return None  # portable fixtures; live Windows target support is Windows-only
    import ctypes
    from ctypes import wintypes as w
    class Basic(ctypes.Structure):
        _fields_ = [('process_time', ctypes.c_longlong), ('job_time', ctypes.c_longlong),
                    ('flags', w.DWORD), ('min_ws', ctypes.c_size_t), ('max_ws', ctypes.c_size_t),
                    ('active', w.DWORD), ('affinity', ctypes.c_size_t), ('priority', w.DWORD), ('scheduling', w.DWORD)]
    class IO(ctypes.Structure):
        _fields_ = [(n, ctypes.c_ulonglong) for n in ('read_ops', 'write_ops', 'other_ops', 'read', 'write', 'other')]
    class Limits(ctypes.Structure):
        _fields_ = [('basic', Basic), ('io', IO), ('process_memory', ctypes.c_size_t),
                    ('job_memory', ctypes.c_size_t), ('peak_process', ctypes.c_size_t), ('peak_job', ctypes.c_size_t)]
    k = ctypes.WinDLL('kernel32', use_last_error=True)
    k.CreateJobObjectW.argtypes, k.CreateJobObjectW.restype = [ctypes.c_void_p, w.LPCWSTR], w.HANDLE
    k.SetInformationJobObject.argtypes = [w.HANDLE, ctypes.c_int, ctypes.c_void_p, w.DWORD]
    k.AssignProcessToJobObject.argtypes = [w.HANDLE, w.HANDLE]
    k.GetCurrentProcess.restype = w.HANDLE
    k.CloseHandle.argtypes = [w.HANDLE]
    job = k.CreateJobObjectW(None, None)
    limits = Limits()
    limits.basic.flags = 0x2000  # JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE; no breakaway
    if not job or not k.SetInformationJobObject(job, 9, ctypes.byref(limits), ctypes.sizeof(limits)) \
            or not k.AssignProcessToJobObject(job, k.GetCurrentProcess()):
        error = ctypes.get_last_error()
        if job:
            k.CloseHandle(job)
        raise RuntimeError('REA cannot establish owned descendant cleanup job: ' + str(error))
    return job


def serve(start):
    if start.get('kind') != 're' or start.get('provider') != 'codex':
        raise ValueError('REA bootstrap supports only reserved Codex RE sessions; Claude RE unsupported')
    owned_job = owned_windows_job()  # retained until broker exit, including startup failure
    session = Session(start['config'], start['contract'])
    stop, attached = threading.Event(), threading.Event()
    attach_lock = threading.Lock()
    token, sid = secrets.token_hex(32), secrets.token_hex(16)
    listener = socket.socket()
    listener.bind(('127.0.0.1', 0))
    listener.listen(4)
    listener.settimeout(.2)

    def handle(sock):
        is_worker = False
        current_id = None
        with sock, sock.makefile('rwb') as f:
            try:
                sock.settimeout(5)
                hello = wire_read(f)
                if not secrets.compare_digest(str(hello.get('token', '')), token) or hello.get('session_id') != sid:
                    raise ValueError('REA session authorization failed')
                op = hello.get('op')
                if op == 'shutdown':
                    wire_send(f, dict(session_id=sid, closed=True))
                    stop.set()
                    return
                if op == 'status':
                    identity = session.identity(time.monotonic()+session.contract.get('startup_timeout', 45))
                    wire_send(f, dict(session_id=sid, ready=True, identity=identity))
                    return
                with attach_lock:
                    if op != 'attach' or attached.is_set():
                        raise ValueError('REA session already attached or unsupported operation')
                    attached.set()
                is_worker = True
                sock.settimeout(None)
                wire_send(f, dict(session_id=sid, attached=True))
                requests = queue.Queue()
                def read_worker():
                    try:
                        while True:
                            r = wire_read(f)
                            if not isinstance(r, dict):
                                raise ValueError('malformed worker JSON-RPC object')
                            if r.get('method') == 'notifications/cancelled':
                                raise RuntimeError('REA worker cancelled')
                            requests.put(r)
                    except Exception as e:
                        requests.put(e)
                        stop.set()  # closes owned child even during a blocked RPC
                threading.Thread(target=read_worker, daemon=True).start()
                while not stop.is_set():
                    r = requests.get()
                    if isinstance(r, Exception):
                        raise r
                    current_id = r.get('id')
                    try:
                        reply = session.request(r)
                    except ValueError as e:
                        reply = dict(jsonrpc='2.0', id=r.get('id'), error=dict(code=-32602, message=str(e)))
                    if reply is not None:
                        wire_send(f, reply)
            except Exception as e:
                try:
                    error = str(e)[:1500]
                    if is_worker:
                        wire_send(f, dict(jsonrpc='2.0', id=current_id, error=dict(code=-32000, message=error)))
                    else:
                        wire_send(f, dict(error=error, session_id=sid))
                except OSError:
                    pass
                if is_worker:
                    stop.set()

    try:
        identity = session.identity(time.monotonic()+session.contract.get('startup_timeout', 45))
        wire_send(sys.stdout.buffer, dict(port=listener.getsockname()[1], token=token, session_id=sid,
            lane=start['lane'], identity=identity, broker_pid=os.getpid(), contract_sha256=digest(start['contract'])))
        deadline = time.monotonic()+120
        while not stop.is_set():
            if not attached.is_set() and time.monotonic() > deadline:
                break
            if session.client.p.poll() is not None:
                break
            try:
                sock, _ = listener.accept()
            except socket.timeout:
                continue
            threading.Thread(target=handle, args=(sock,), daemon=True).start()
    finally:
        listener.close()
        session.client.close()


def acquire(root, lane, provider, kind, cfg):
    if kind != 're' or provider != 'codex':
        raise ValueError('REA bootstrap supports only Codex RE; Claude RE unsupported')
    path = Path(root)/'build-runs/rea-target.json'
    if not path.is_file():
        raise ValueError('REA reviewed exact-target contract missing: ' + str(path))
    contract = validate_contract(json.loads(path.read_text(encoding='utf-8')))
    # Keep full server stderr privately, including startup failures. Never mix it
    # with MCP stdout or discard it when the launcher transfers ownership.
    diagnostic_path = Path(root)/'build-runs'/('rea-session-'+secrets.token_hex(12)+'-stderr.log')
    with diagnostic_path.open('wb') as diagnostics:
        p = subprocess.Popen([sys.executable, str(Path(__file__).resolve()), 'serve'], stdin=subprocess.PIPE,
                             stdout=subprocess.PIPE, stderr=diagnostics, creationflags=NOWIN)
    q = queue.Queue()
    def receive():
        try:
            q.put(wire_read(p.stdout))
        except Exception as e:
            q.put(e)
    threading.Thread(target=receive, daemon=True).start()
    try:
        wire_send(p.stdin, dict(config=cfg, contract=contract, lane=lane, provider=provider, kind=kind))
        p.stdin.close()
        try:
            ticket = q.get(timeout=contract.get('startup_timeout', 45)+10)
        except queue.Empty as e:
            raise RuntimeError('REA admission timeout') from e
        if isinstance(ticket, Exception) or 'error' in ticket:
            raise RuntimeError('REA admission failed: ' + str(ticket))
        require_ready(ticket, lane)
        ticket['diagnostics_path'] = str(diagnostic_path)
        return ticket, p
    except BaseException:
        if p.poll() is None:
            p.terminate()
        p.wait(timeout=5)
        raise
    finally:
        p.stdout.close()


def connect(ticket):
    with socket.create_connection(('127.0.0.1', ticket['port']), timeout=125) as sock:
        sock.settimeout(None)
        with sock.makefile('rwb') as f:
            wire_send(f, dict(token=ticket['token'], op='attach', session_id=ticket['session_id']))
            reply = wire_read(f)
            if reply.get('attached') is not True or reply.get('session_id') != ticket['session_id']:
                raise RuntimeError('REA attach rejected: ' + str(reply))
            def send_input():
                try:
                    while True:
                        # Raw reads avoid a daemon holding BufferedReader's stdin
                        # lock during cancellation/interpreter shutdown on Windows.
                        line = os.read(sys.stdin.fileno(), 65536)
                        if not line:
                            break
                        sock.sendall(line)
                finally:
                    try:
                        sock.shutdown(socket.SHUT_WR)
                    except OSError:
                        pass
            threading.Thread(target=send_input, daemon=True).start()
            while True:
                line = f.readline(MAX_LINE+1)
                if not line:
                    return
                sys.stdout.buffer.write(line)
                sys.stdout.buffer.flush()


def configured_server():
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]/'.claude/lane-bin'))
    import laneguard
    cfg = laneguard.rea_config()
    if not cfg:
        raise ValueError('REA configuration missing')
    return cfg


def inspect_metadata(identity_tool):
    """No target open and no procedure calls. Prints only protocol/schema metadata."""
    child = Client(configured_server())
    try:
        deadline = time.monotonic()+45
        init = child.rpc('initialize', dict(protocolVersion='2025-06-18', capabilities={},
            clientInfo=dict(name='fc4-schema-inspect', version='1')), deadline)
        if 'error' in init:
            raise RuntimeError('REA initialize error: ' + str(init['error']))
        child.send(dict(jsonrpc='2.0', method='notifications/initialized'))
        listing = child.rpc('tools/list', {}, deadline)
        if 'error' in listing:
            raise RuntimeError('REA tools/list error: ' + str(listing['error']))
        for t in listing['result']['tools']:
            if t['name'] in (*RESEARCH, 'open_binary', identity_tool):
                print(json.dumps(dict(name=t['name'], schema=t['inputSchema'], sha256=digest(t['inputSchema']))))
        print(json.dumps(dict(tool_names=[t['name'] for t in listing['result']['tools']], target_readiness='UNKNOWN')))
    finally:
        child.close()


def smoke(root, lane):
    """Two independent owned clients; metadata/typed identity only, zero research."""
    sessions = []
    try:
        for number in range(2):
            ticket, p = acquire(root, lane+'-'+str(number+1), 'codex', 're', configured_server())
            sessions.append((ticket, p))
        if len({t['identity']['child_pid'] for t, _ in sessions}) != 2:
            raise RuntimeError('REA smoke clients are not independent')
        for ticket, p in sessions:
            with socket.create_connection(('127.0.0.1', ticket['port']), timeout=125) as sock, sock.makefile('rwb') as f:
                wire_send(f, dict(token=ticket['token'], op='attach', session_id=ticket['session_id']))
                ack = wire_read(f)
                if ack.get('attached') is not True:
                    raise RuntimeError('REA smoke attach failed: ' + str(ack))
                for i, method, params in [(1, 'initialize', dict(protocolVersion='2025-06-18')),
                                           (2, 'tools/list', {})]:
                    wire_send(f, dict(jsonrpc='2.0', id=i, method=method, params=params))
                    reply = wire_read(f)
                    if 'error' in reply or reply.get('id') != i:
                        raise RuntimeError('REA smoke protocol failed: ' + str(reply))
                names = [t['name'] for t in reply['result']['tools']]
                if names != list(RESEARCH):
                    raise RuntimeError('REA smoke exposed unexpected capabilities')
                evidence = require_ready(ticket, ticket['lane'])
                print(json.dumps(evidence | dict(lane=ticket['lane'], tools=names, research_calls=0)), flush=True)
            p.wait(timeout=10)
    finally:
        for ticket, p in sessions:
            shutdown(ticket)
            p.wait(timeout=10)


def main():
    a = argparse.ArgumentParser(description=__doc__)
    a.add_argument('mode', choices=('serve', 'connect', 'inspect', 'smoke'))
    a.add_argument('--manifest')
    a.add_argument('--root', type=Path)
    a.add_argument('--lane', default='re-readiness-smoke')
    a.add_argument('--identity-tool', default='')
    args = a.parse_args()
    try:
        if args.mode == 'serve':
            serve(wire_read(sys.stdin.buffer))
        elif args.mode == 'connect':
            m = json.loads(Path(args.manifest).read_text(encoding='utf-8'))
            if m['kind'] != 're' or m['provider'] != 'codex':
                raise ValueError('non-RE/unsupported provider transport denied')
            require_ready(m['rea_session'], m['lane'])
            connect(m['rea_session'])
        elif args.mode == 'inspect':
            owned_job = owned_windows_job()
            inspect_metadata(args.identity_tool)
        else:
            if not args.root:
                raise ValueError('smoke requires explicit --root')
            smoke(args.root, args.lane)
    except Exception as e:
        if args.mode == 'serve':
            wire_send(sys.stdout.buffer, {'error': str(e)[:2000]})
        print('REA-DOWN ' + str(e)[:2000], file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
