"""Host provenance survives isolated refs; it never substitutes for runtime guard."""
import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT/'tools'))
sys.path.insert(0, str(ROOT/'.claude/lane-bin'))
import lane_admission as admission
import codexguard


class HostAdmission(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(dir=ROOT/'build-runs', prefix='host-proof-')
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.wt = self.root/'.claude/worktrees/re-fixture'
        self.wt.mkdir(parents=True)
        (self.root/'build-runs').mkdir()
        self.m = dict(main=str(self.root), worktree=str(self.wt), lane='re-fixture', round=2,
                      kind='re', provider='codex', guard_id='codex-'+'a'*32, budget=150000, max_calls=100,
                      manifest=str(self.root/'build-runs/manifest.json'), source={'tip':'a'*40},
                      status_path=str(self.root/'build-runs/status.json'),
                      args={'kind':'re', 'deps':['landed-but-invisible'], 'task':'fixture'},
                      rea_session={'session_id':'fixture', 'identity':{'ready':True, 'binary_sha256':'b'*64}})
        self.checks = [dict(name=n, verdict='GO') for n in ('dispatcher','lanes.check','supervisor')]
        self.m['host_admission'] = admission.issue(self.m, self.checks)
        self.pin = admission.pin(self.m['host_admission'])

    def test_inherited_admission_without_global_npm_or_git_refs(self):
        # A real isolated directory contains neither global installation nor
        # dependency/own refs. Any rediscovery/preflight invocation is an error.
        self.assertFalse((self.wt/'.git').exists())
        self.assertFalse((self.wt/'global-npm/rea.mjs').is_file())
        with patch('subprocess.run', side_effect=AssertionError('worker discovery forbidden')), \
                patch('laneguard.preflight', side_effect=AssertionError('worker preflight forbidden')):
            codexguard.validate_manifest(self.m, self.pin)
            result = admission.verify(self.m, self.pin)
        self.assertEqual(result['subject']['round'], 2)
        # Editing a worker copy grants no admission authority.
        forged = copy.deepcopy(self.m)
        forged['host_admission']['subject']['lane'] = 'forged'
        with self.assertRaisesRegex(ValueError, 'provenance'):
            admission.verify(forged, self.pin)

    def test_forged_lane_round_source_target_session_and_expired_fail_closed(self):
        changes = [dict(lane='re-other'), dict(round=3), dict(source={'tip':'c'*40}), dict(max_calls=10000),
                   dict(args={'task':'other'}), dict(rea_session={'session_id':'other'}),
                   dict(rea_session={'session_id':'fixture','identity':{'binary_sha256':'c'*64}})]
        for change in changes:
            with self.subTest(change=change), self.assertRaisesRegex(ValueError, 'provenance'):
                admission.verify(self.m | change, self.pin)
        for key in ('version', 'checks'):
            forged = copy.deepcopy(self.m)
            if key == 'version':
                forged['host_admission']['version'] = 0
            else:
                forged['host_admission']['checks'][0]['verdict'] = 'GO-fake'
            with self.assertRaisesRegex(ValueError, 'provenance'):
                admission.verify(forged, self.pin)
        expires = self.m['host_admission']['expires']
        with self.assertRaisesRegex(ValueError, 'provenance'):
            admission.verify(self.m, self.pin, now=expires)
        with self.assertRaisesRegex(ValueError, 'provenance'):
            admission.verify({k:v for k,v in self.m.items() if k != 'host_admission'}, self.pin)
        with self.assertRaisesRegex(ValueError, 'provenance'):
            admission.verify(self.m)  # a digest from the manifest is never authority

    def test_runtime_guard_stop_cap_and_role_remain_enforced(self):
        transcript = self.root/'transcript.jsonl'
        transcript.write_text(json.dumps({'type':'event_msg', 'payload':{'type':'token_count',
            'info':{'total_token_usage':{'total_tokens':1}}}})+'\n')
        self.m['status_path'] = str(self.root/'build-runs/status.json')
        manifest = Path(self.m['manifest'])
        manifest.write_text(json.dumps(self.m))
        start = dict(hook_event_name='SessionStart', cwd=str(self.wt), session_id='client',
                     transcript_path=str(transcript))
        self.assertEqual(codexguard.handle(start, self.m, self.pin)[0], 'allow')
        request = dict(hook_event_name='PreToolUse', session_id='client', transcript_path=str(transcript),
                       tool_name='Bash', tool_input={'command':'git status'}, tool_use_id='stop')
        stop = self.root/'build-runs/STOP'
        stop.write_text(json.dumps({'reason':'fixture stop', 'since':'now'}))
        with patch('codexguard.context_tokens', return_value=1):
            self.assertEqual(codexguard.handle(request, self.m, self.pin)[0], 'deny')
        stop.unlink()
        with patch('codexguard.context_tokens', return_value=150001):
            decision = codexguard.handle(request | {'tool_use_id':'budget'}, self.m, self.pin)
        self.assertEqual(decision[0], 'deny')
        decision = codexguard.handle(request | {'tool_use_id':'role', 'tool_name':'mcp__rea__open_binary'}, self.m, self.pin)
        self.assertEqual(decision[0], 'deny')
