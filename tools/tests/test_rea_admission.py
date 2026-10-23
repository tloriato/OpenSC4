"""Process-local startup regression; only harmless MCP fixtures, never real REA."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT/'.claude/lane-bin'))
sys.path.insert(0, str(ROOT/'tools'))
import laneguard
import rea_session as rs
import lane_backend
import lane_launch
from fake_rea_mcp import TOOLS

FAKE = Path(__file__).with_name('fake_rea_mcp.py')


def config(**env):
    return dict(command=sys.executable, args=[str(FAKE)], env=env)


class Reproduce(unittest.TestCase):
    def test_tools_list_cannot_establish_another_process_target(self):
        ok, detail = laneguard.rea_handshake(config(), timeout=3)
        self.assertTrue(ok, detail)
        p = subprocess.Popen([sys.executable, str(FAKE)], stdin=subprocess.PIPE,
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        try:
            request = dict(jsonrpc='2.0', id=1, method='tools/call',
                           params=dict(name='procedure_pseudo_code', arguments={'procedure': 'fixture'}))
            out, err = p.communicate(json.dumps(request)+'\n', timeout=3)
            self.assertEqual(json.loads(out)['result']['structuredContent']['error'], 'No app is open')
            print('REPRODUCED: tools/list succeeds; independent research client: No app is open')
        finally:
            if p.poll() is None:
                p.kill()
            p.wait()


class Admission(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='rea-fixture-', dir=ROOT/'build-runs')
        self.root = Path(self.tmp.name)
        (self.root/'build-runs').mkdir()
        self.binary, self.project = self.root/'fixture.bin', self.root/'project'
        self.binary.write_bytes(b'Harmless fixture, not a game file')
        self.project.mkdir()
        self.audit = self.root/'calls.jsonl'
        self.c = dict(version=2, reviewed=True, label='Windows 1.1.641', binary_path=str(self.binary),
            binary_sha256=__import__('hashlib').sha256(self.binary.read_bytes()).hexdigest(),
            format='pe', architecture='x86', kind='executable', startup_timeout=2,
            open_arguments=dict(path=str(self.binary)), provider_selection='auto-single-candidate',
            schemas={t['name']:{k:rs.digest(t[k+'Schema']) for k in ('input', 'output')} for t in TOOLS})
        self.cfg = config(FAKE_REA_AUDIT=str(self.audit))
        self.owned = []
        self.clients = []
        self.fixture_pids = {}

    def tearDown(self):
        for p in self.clients:
            if p.poll() is None:
                if not p.stdin.closed:
                    p.stdin.close()
                try:
                    p.wait(timeout=8)
                except subprocess.TimeoutExpired:
                    p.kill()  # only this test's owned connector, never another process
                    p.wait(timeout=5)
            for s in (p.stdin, p.stdout, p.stderr):
                if s and not s.closed:
                    s.close()
        for ticket, p in self.owned:
            rs.shutdown(ticket)
            p.wait(timeout=8)
        self.tmp.cleanup()

    def contract(self):
        (self.root/'build-runs/rea-target.json').write_text(json.dumps(self.c), encoding='utf-8')

    def acquire(self, mode='', **env):
        self.contract()
        before = len(self.calls())
        ticket, p = rs.acquire(self.root, 're-fixture', 'codex', 're',
                               self.cfg | {'env': self.cfg['env'] | {'FAKE_REA_MODE': mode} | env})
        self.owned.append((ticket, p))
        # Windows venv executables can be redirector processes. Use the actual
        # fixture audit witness, never Popen.pid, to prove same-server research.
        witnesses = {a['pid'] for a in self.calls()[before:]}
        self.assertEqual(len(witnesses), 1)
        self.fixture_pids[ticket['session_id']] = witnesses.pop()
        return ticket

    def calls(self):
        return [json.loads(l) for l in self.audit.read_text().splitlines()] if self.audit.exists() else []

    def bridge(self, ticket):
        m = dict(main=str(self.root), worktree=str(self.root), manifest=str(self.root/'manifest.json'),
                 kind='re', provider='codex', lane='re-fixture', rea_session=ticket,
                 git_common=str(self.root/'.git'), git_dir=str(self.root/'.git/worktrees/re-fixture'))
        Path(m['manifest']).write_text(json.dumps(m))
        with patch('lane_backend.shutil.which', return_value='codex.exe'), \
                patch('lane_backend.codex_defaults', return_value={}), \
                patch('lane_backend.subprocess.run', return_value=subprocess.CompletedProcess([],0,'Logged in using ChatGPT','')):
            command = lane_backend.codex_command(m, 'schema', 'final', {})
        values = {a.split('=',1)[0]: a.split('=',1)[1] for a in command if a.startswith('mcp_servers.')}
        self.assertEqual(json.loads(values['mcp_servers.rea.enabled_tools']), list(rs.RESEARCH))
        self.assertEqual(values['mcp_servers.rea.required'], 'true')
        self.assertNotIn('open_binary', str(values))
        p = subprocess.Popen([json.loads(values['mcp_servers.rea.command']),
                              *json.loads(values['mcp_servers.rea.args'])],
                             stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        self.clients.append(p)
        return p

    def rpc(self, p, method, params=None):
        rs.wire_send(p.stdin, dict(jsonrpc='2.0', id=10, method=method, params=params or {}))
        return rs.wire_read(p.stdout)

    def test_two_independent_actual_clients_bootstrap_and_research_in_same_process(self):
        tickets = [self.acquire(), self.acquire()]
        self.assertNotEqual(self.fixture_pids[tickets[0]['session_id']], self.fixture_pids[tickets[1]['session_id']])
        for ticket in tickets:
            p = self.bridge(ticket)
            init = self.rpc(p, 'initialize', dict(protocolVersion='2025-06-18'))
            self.assertNotIn('error', init)
            tools = self.rpc(p, 'tools/list')['result']['tools']
            self.assertEqual([t['name'] for t in tools], list(rs.RESEARCH))
            for name, args in [('procedure_pseudo_code', {'procedure':'fixture'}), ('batch_decompile', {'addresses':['fixture']})]:
                reply = self.rpc(p, 'tools/call', dict(name=name, arguments=args))
                self.assertEqual(reply['result']['structuredContent']['pid'], self.fixture_pids[ticket['session_id']])
            opens = [a for a in self.calls() if a['pid'] == self.fixture_pids[ticket['session_id']] and a['name']=='open_binary']
            self.assertEqual(len(opens), 1)
            self.assertEqual(opens[0]['arguments'], self.c['open_arguments'])
        print('FIXTURE VERIFIED: two independent admitted children; exact open and both research calls share each PID')

    def test_already_correct_is_idempotent_and_keeper_does_not_contaminate_new_client(self):
        existing = self.existing()
        keeper = self.acquire(FAKE_REA_EXISTING=json.dumps(existing))
        other = self.acquire()
        opens = [a for a in self.calls() if a['name']=='open_binary']
        self.assertEqual([a['pid'] for a in opens], [self.fixture_pids[other['session_id']]])
        self.assertNotEqual(keeper['session_id'], other['session_id'])

    def test_wrong_preexisting_never_opened_or_reset(self):
        existing = self.existing() | {'path': str(self.binary)+'-wrong'}
        with self.assertRaisesRegex(RuntimeError, 'wrong/preexisting'):
            self.acquire(FAKE_REA_EXISTING=json.dumps(existing))
        self.assertEqual([a['name'] for a in self.calls()], ['binary_session'])

    def existing(self):
        return dict(open=True, path=str(self.binary), sha256=self.c['binary_sha256'],
                    architecture='x86', format='pe', kind='executable')

    def test_failures_preserve_actual_causes(self):
        for mode, cause in [('open_error','deliberate open failure'), ('negative','negative'),
                            ('open_rpc_error','deliberate RPC open failure'), ('wrong_protocol','unsupported protocol'),
                            ('untyped_ready','typed metadata'), ('provider_missing','binding UNKNOWN'),
                            ('string_success','typed metadata'), ('unsupported','schema'), ('identity_error','identity unavailable'),
                            ('wrong_after_open','wrong/preexisting'), ('malformed','Expecting value'),
                            ('stderr_eof','fixture server startup diagnostic'),
                            ('timeout','timeout'), ('notifications','timeout'), ('eof','EOF')]:
            with self.subTest(mode=mode), self.assertRaisesRegex(RuntimeError, cause):
                self.acquire(mode)

    def test_missing_path_wrong_hash_unreviewed_and_wrong_open_are_denied_before_process(self):
        original = json.loads(json.dumps(self.c))
        for update in [dict(binary_path=str(self.binary)+'-absent'), dict(binary_sha256='0'*64), dict(reviewed=False),
                       dict(open_arguments={'path':'C:/arbitrary.exe'}),
                       dict(open_arguments=original['open_arguments'] | {'provider_id':'other'}),
                       dict(open_arguments=original['open_arguments'] | {'snapshot_path':str(self.project)}),
                       dict(open_arguments=original['open_arguments'] | {'read_only':False})]:
            self.c = original | update
            with self.subTest(update=update), self.assertRaises(ValueError):
                self.acquire()
        self.assertFalse(self.audit.exists())

    def test_unreviewed_schema_never_opens_and_nonready_preexisting_never_replaces(self):
        original = self.c['schemas']['open_binary']
        self.c['schemas']['open_binary'] = original | {'input': '0'*64}
        with self.assertRaisesRegex(RuntimeError, 'schema'):
            self.acquire()
        self.assertFalse(self.audit.exists())
        self.c['schemas']['open_binary'] = original
        existing = self.existing() | {'open': 'loading'}
        with self.assertRaisesRegex(RuntimeError, 'typed metadata'):
            self.acquire(FAKE_REA_EXISTING=json.dumps(existing))
        self.assertEqual([a['name'] for a in self.calls()], ['binary_session'])

    def test_identity_drift_fails_before_research_with_framed_error(self):
        drift = self.root/'drift'
        ticket = self.acquire(FAKE_REA_DRIFT_FILE=str(drift))
        p = self.bridge(ticket)
        self.rpc(p, 'tools/list')
        drift.write_text('fixture drift')
        reply = self.rpc(p, 'tools/call', dict(name='procedure_pseudo_code', arguments={'procedure':'fixture'}))
        self.assertEqual(reply['jsonrpc'], '2.0')
        self.assertEqual(reply['id'], 10)
        self.assertIn('wrong/preexisting', reply['error']['message'])
        self.assertFalse(any(a['name'] in rs.RESEARCH for a in self.calls()))

    def test_non_re_and_claude_unsupported_without_start(self):
        self.contract()
        for provider, kind in [('codex','tool'), ('codex','engine'), ('claude','re')]:
            with self.subTest(provider=provider,kind=kind), self.assertRaisesRegex(ValueError, 'only Codex RE'):
                rs.acquire(self.root, 're-fixture', provider, kind, self.cfg)
        with self.assertRaisesRegex(RuntimeError, 'unsupported'):
            lane_backend.run_claude(dict(kind='re'))
        self.assertFalse(self.audit.exists())

    def test_exact_capability_boundary_in_actual_configured_transport(self):
        ticket = self.acquire()
        p = self.bridge(ticket)
        for name, args in [('open_binary', self.c['open_arguments']), ('close_app', {}), ('binary_session', {}),
                           ('execute', {}), ('procedure_pseudo_code', {'path':'C:/arbitrary'}),
                           ('procedure_pseudo_code', {'address':'C:/arbitrary'}),
                           ('procedure_pseudo_code', {'procedure':1}),
                           ('procedure_pseudo_code', {'procedure':'fixture', 'document':'other'}),
                           ('procedure_pseudo_code', {}),
                           ('batch_decompile', {'addresses':[True]}),
                           ('batch_decompile', {'addresses':[], 'target':'other'})]:
            with self.subTest(name=name):
                self.assertIn('error', self.rpc(p, 'tools/call', dict(name=name, arguments=args)))
        self.assertEqual([a['name'] for a in self.calls() if a['name'] in rs.RESEARCH], [])

    def test_admission_guard_no_handshake_bypass_and_lane_identity(self):
        self.assertFalse(laneguard.rea_check(handshake=False)[0])
        ticket = self.acquire()
        self.assertEqual(laneguard.preflight(str(self.root), 're-fixture', 're', session=ticket), (0, 'GO'))
        code, line = laneguard.preflight(str(self.root), 're-other', 're', session=ticket)
        self.assertEqual(code, 1)
        self.assertIn('this lane', line)
        rs.shutdown(ticket)
        self.owned[-1][1].wait(timeout=8)
        self.assertFalse(laneguard.rea_check(session=ticket)[0])

    def test_failed_launch_preflight_changes_no_queue_state_round_or_worktree(self):
        self.contract()
        state = dict(lanes={'re-fixture':dict(rounds=1, role='re', touches=['alpha'])}, rounds=2,
                     per_lane_limit=3, concurrency=7, glue_limit=3, history=[])
        state_path = self.root/'build-runs/state.json'
        state_path.write_text(json.dumps(state))
        queue_dir = self.root/'build-runs/merge-queue'
        queue_dir.mkdir()
        queue_path = queue_dir/'re-fixture.json'
        queue_path.write_text('{"touches":["old"]}')
        spec_path = self.root/'spec.json'
        spec_path.write_text(json.dumps(dict(args=dict(name='re-fixture',kind='re'), touches=['alpha'], resume=True)))
        (self.root/'build-runs/run4-coordinator.lock').write_bytes(b'0')
        before = {p.relative_to(self.root):p.read_bytes() for p in self.root.rglob('*') if p.is_file()}
        with patch('lane_launch.backend.laneguard.rea_config', return_value=self.cfg | {'env':{'FAKE_REA_MODE':'open_error'}}), \
                patch('lane_launch.checked', return_value=None), \
                patch('lane_launch.shutil.disk_usage', return_value=__import__('collections').namedtuple('Disk','total used free')(100,50,50)):
            with self.assertRaisesRegex(RuntimeError, 'deliberate open failure'):
                lane_launch.launch(self.root, state_path, spec_path, 'codex')
        # Private diagnostic logs are the only admitted failure artifact, never a
        # ledger, queue, result/manifest, round or worktree mutation.
        after = {p.relative_to(self.root):p.read_bytes() for p in self.root.rglob('*') if p.is_file()
                 and not (p.name.startswith('rea-session-') and p.name.endswith('-stderr.log'))}
        self.assertEqual(before, after)

    @unittest.skipUnless(os.name == 'nt', 'Windows live target ownership uses nested job objects')
    def test_broker_exit_cleans_owned_descendant_without_killing_other_sessions(self):
        import ctypes
        from ctypes import wintypes as w
        first = self.acquire(FAKE_REA_SPAWN_CHILD='1')
        grandchild = next(a['grandchild_pid'] for a in self.calls() if a['name'] == 'spawn_child')
        second = self.acquire()
        k = ctypes.WinDLL('kernel32', use_last_error=True)
        k.OpenProcess.argtypes, k.OpenProcess.restype = [w.DWORD, w.BOOL, w.DWORD], w.HANDLE
        k.WaitForSingleObject.argtypes = [w.HANDLE, w.DWORD]
        k.CloseHandle.argtypes = [w.HANDLE]
        handle = k.OpenProcess(0x100000, False, grandchild)  # SYNCHRONIZE only
        self.assertTrue(handle)
        try:
            broker = self.owned[0][1]
            broker.terminate()  # this test's own broker: exercises job-on-crash cleanup
            broker.wait(timeout=8)
            self.assertEqual(k.WaitForSingleObject(handle, 5000), 0)
            self.assertTrue(rs.require_ready(second)['ready'])
        finally:
            k.CloseHandle(handle)

    def test_disconnect_cancellation_and_single_attachment_cleanup(self):
        for cancellation in (False, True):
            ticket = self.acquire()
            p = self.bridge(ticket)
            self.rpc(p, 'tools/list')
            second = self.bridge(ticket)
            out, err = second.communicate(timeout=5)
            self.assertEqual(out, b'')
            self.assertIn(b'already attached', err)
            if cancellation:
                rs.wire_send(p.stdin, dict(jsonrpc='2.0', method='notifications/cancelled', params={'requestId':10}))
            else:
                p.stdin.close()
            self.owned[-1][1].wait(timeout=8)
            p.wait(timeout=8)

    def test_supervisor_lifetime_loss_closes_attached_server_without_affecting_another(self):
        ticket = self.acquire()
        other = self.acquire()
        lifetime = rs.lease(ticket)
        try:
            p = self.bridge(ticket)
            self.rpc(p, 'tools/list')
            with self.assertRaisesRegex(RuntimeError, 'already leased'):
                rs.lease(ticket)
        finally:
            lifetime.close()  # same EOF as supervisor death; no child/process-name kills
        self.owned[0][1].wait(timeout=8)
        p.wait(timeout=8)
        self.assertTrue(rs.require_ready(other)['ready'])

    def test_backend_supervisor_establishes_real_lease_before_provider_and_cleans_it(self):
        ticket = self.acquire()
        manifest = self.root/'backend-manifest.json'
        state = self.root/'build-runs/state.json'
        result = self.root/'build-runs/result.json'
        state.write_text(json.dumps(dict(lanes={'re-fixture':dict(pid=os.getpid(),rounds=1)})))
        before = state.read_bytes()
        manifest.write_text(json.dumps(dict(kind='re',provider='codex',lane='re-fixture',round=1,
            main=str(self.root),worktree=str(self.root),state=str(state),result=str(result),rea_session=ticket,
            manifest=str(manifest), guard_id='codex-'+'a'*32, args={}, source={'tip':'fixture'},
            host_checks=[dict(name=n, verdict='GO') for n in ('dispatcher','lanes.check')])) )
        def provider(m):
            # Only model execution/Git preparation are stubbed. Actual typed target
            # admission, backend lifetime ownership and cleanup all run for real.
            self.assertTrue(rs.require_ready(m['rea_session'], m['lane'])['ready'])
            with self.assertRaisesRegex(RuntimeError, 'already leased'):
                rs.lease(m['rea_session'])
            return dict(type='result', is_error=False, result='fixture provider completed')
        with patch.object(sys, 'argv', ['lane_backend.py','--manifest',str(manifest)]), \
                patch('lane_backend.prepare_codex_git'), patch('lane_backend.run_codex', side_effect=provider):
            self.assertEqual(lane_backend.main(), 0)
        self.owned[0][1].wait(timeout=8)
        self.assertEqual(state.read_bytes(), before)
        self.assertEqual(json.loads(result.read_text())['result'], 'fixture provider completed')
        self.assertFalse(laneguard.rea_check(session=ticket)[0])

    def test_prompt_explains_boundary_and_preserves_unknown(self):
        rendered = lane_backend.render(dict(kind='re',name='re-fixture',task='fixture',maxRounds=1), 'codex', 'manifest')
        self.assertIn('retained, admitted exact-target client', rendered['prompt'])
        self.assertIn('keeper process is not target readiness', rendered['prompt'])


if __name__ == '__main__':
    unittest.main()
