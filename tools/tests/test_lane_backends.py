"""Provider parity: shared policy and budgets, correct auth/transport, no unguarded fallback."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import tomllib
import unittest
from unittest.mock import patch

from test_laneguard import Fixture, LANE, git

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO/'tools'))
sys.path.insert(0, str(REPO/'.claude/lane-bin'))
import codexguard
import lane_backend
import lane_launch
import lane_refill
import laneguard


class CodexGuard(unittest.TestCase):
    def setUp(self):
        self.f = Fixture(calls=2)
        self.manifest = self.f.main/'build-runs/codex-manifest.json'
        self.transcript = self.f.main/'build-runs/codex.jsonl'
        self.m = dict(main=str(self.f.main), worktree=str(self.f.wt), lane=LANE, kind='tool', provider='codex',
                      guard_id='codex-'+'a'*32, budget=150000, max_calls=2, round=2,
                      status_path=str(self.f.main/'build-runs/status.json'))
        self.manifest.write_text(json.dumps(self.m))
        self.set_context(1000)
        self.start()

    def tearDown(self):
        self.f.close()

    def set_context(self, n):
        self.transcript.write_text(json.dumps(dict(type='event_msg', payload=dict(type='token_count', info=dict(
            last_token_usage=dict(input_tokens=n, cached_input_tokens=n-1),
            total_token_usage=dict(input_tokens=n*100)))))+'\n')

    def data(self, event, **kw):
        return dict(hook_event_name=event, session_id='session1', cwd=str(self.f.wt),
                    transcript_path=str(self.transcript), model='fixture-model', **kw)

    def start(self):
        return codexguard.handle(self.data('SessionStart'), self.m)

    def call(self, tool='Bash', command='Get-Date', identity=None):
        return codexguard.handle(self.data('PreToolUse', tool_name=tool,
                                          tool_input={'command': command}, tool_use_id=identity), self.m)

    def test_context_is_last_input_not_cumulative_or_cache_sum(self):
        self.set_context(100000)
        self.assertEqual(codexguard.context_tokens(self.transcript), 100000)
        self.assertEqual(self.call()[0], 'allow')

    def test_missing_context_denies_instead_of_silently_disabling_budget(self):
        status = laneguard.read_json(self.m['status_path'])
        status['bootstrap_done'] = True
        laneguard.write_json(self.m['status_path'], status)
        self.transcript.write_text('{}\n')
        p = subprocess.run([sys.executable, str(REPO/'.claude/lane-bin/codexguard.py'), 'hook',
                            '--manifest', str(self.manifest)],
                           input=json.dumps(self.data('PreToolUse', tool_name='Bash', tool_input={'command':'whoami'})),
                           capture_output=True, text=True)
        self.assertEqual(p.returncode, 0)
        self.assertEqual(json.loads(p.stdout)['hookSpecificOutput']['permissionDecision'], 'deny')
        self.assertTrue(self.manifest.with_suffix('.fatal.json').exists())

    def test_first_call_without_usage_only_allows_identity_probe_once(self):
        self.transcript.write_text('{}\n')
        self.assertEqual(self.call(command='Write-Output work')[0], 'deny')
        self.assertEqual(self.call(command=codexguard.bootstrap_command(self.m))[0], 'allow')
        with self.assertRaisesRegex(ValueError, 'telemetry missing'):
            self.call(command=codexguard.bootstrap_command(self.m))
        self.set_context(1000)
        self.assertEqual(self.call(command='Write-Output work')[0], 'allow')

    def test_stop_allows_only_handoff_patch_and_own_commit(self):
        laneguard.write_stop(str(self.f.main), 'test')
        self.assertEqual(self.call()[0], 'deny')
        good = f'*** Begin Patch\n*** Update File: docs/lanes/{LANE}.md\n@@\n-old\n+new\n*** End Patch'
        self.assertEqual(self.call('apply_patch', good)[0], 'allow')
        self.assertEqual(self.call('apply_patch', good.replace(f'docs/lanes/{LANE}.md', 'engine/source.rs'))[0], 'deny')
        self.assertEqual(self.call(command=self.f.commit_cmd()['command'])[0], 'allow')
        self.assertEqual(self.call(command=f'git -C {self.f.main.as_posix()} commit -m bad')[0], 'deny')

    def test_patch_moves_cannot_escape_handoff(self):
        patch_text = f'*** Begin Patch\n*** Update File: docs/lanes/{LANE}.md\n*** Move to: ../../bad\n*** End Patch'
        self.assertFalse(codexguard.handoff_allowed('apply_patch', {'command':patch_text}, str(self.f.wt), LANE))

    def test_watchdog_persists_after_commit_and_duplicates_do_not_count(self):
        self.assertEqual(self.call(identity='a')[0], 'allow')
        self.assertEqual(self.call(identity='a')[0], 'allow')
        self.assertEqual(self.call(identity='b')[0], 'allow')
        self.assertEqual(self.call(identity='c')[0], 'deny')
        self.f.make_commit()
        self.assertEqual(self.call(identity='d')[0], 'deny')

    def test_claude_calls_carry_across_to_codex(self):
        self.f.agent('claude1')
        self.assertEqual(self.f.call('claude1')[0], 'allow')
        self.assertEqual(self.f.call('claude1')[0], 'allow')
        self.assertEqual(self.call()[0], 'deny')

    def test_budget_requires_continue_and_records_shared_round(self):
        self.set_context(150001)
        self.assertEqual(self.call()[0], 'deny')
        result = dict(lane=LANE, state='ready', tip='abcd123', summary='done')
        codexguard.record_result(self.m, result)
        codexguard.record_result(self.m, result)
        self.assertEqual(result['state'], 'continue')
        rows = laneguard.read_rounds(str(self.f.main), LANE)
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]['provider'], 'codex')

    def test_parking_combines_provider_results(self):
        laneguard.append_line(laneguard.rounds_path(str(self.f.main), LANE), json.dumps(
            dict(agent_id='claude1', state='blocked', questions='TR33', label='lane:test#1')))
        codexguard.record_result(self.m, dict(state='blocked', questions='TR33', summary='gap'))
        self.assertIn('TR33', laneguard.park_reason(str(self.f.main), LANE))
        self.assertEqual(self.call()[0], 'deny')

    def test_unregistered_session_and_nested_agent_refused(self):
        with self.assertRaisesRegex(ValueError, 'second Codex'):
            codexguard.handle(self.data('SessionStart') | {'session_id':'other'}, self.m)
        self.assertEqual(self.call('spawn_agent')[0], 'deny')

    def test_rea_role_and_capability_boundary(self):
        self.assertEqual(self.call('mcp__rea__procedure_pseudo_code')[0], 'deny')
        self.m['kind'] = 'engine'
        self.assertEqual(self.call('mcp__rea__procedure_pseudo_code')[0], 'deny')
        self.m['kind'] = 're'
        self.assertEqual(self.call('mcp__rea__open_binary')[0], 'deny')
        self.assertEqual(self.call('mcp__rea__procedure_pseudo_code')[0], 'deny')


class Launcher(unittest.TestCase):
    def setUp(self):
        self.state = dict(lanes={'sample':dict(rounds=2, role='core', touches=['alpha'])},
                          rounds=225, per_lane_limit=3, concurrency=7, glue_limit=3)
        self.spec = dict(args=dict(name='sample', kind='engine'), touches=['alpha'], provider='codex')

    def test_provider_switch_retains_round_cap(self):
        entry, _ = lane_launch.validate_capacity(self.state, self.spec, [], .75)
        self.assertEqual(entry['rounds'], 2)
        entry['rounds'] = 3
        with self.assertRaisesRegex(ValueError, 'round cap'):
            lane_launch.validate_capacity(self.state, self.spec, [], .75)

    def test_parked_waiting_disk_duplicate_and_glue_caps(self):
        for changes, live, fraction, message in [
            ({'parked':True}, [], .75, 'parked'),
            ({'waiting_for':'AD12'}, [], .75, 'blocker'),
            ({}, [], .86, 'exceeds85'),
            ({}, ['sample'], .75, 'already running')]:
            with self.subTest(message=message):
                state = json.loads(json.dumps(self.state))
                state['lanes']['sample'].update(changes)
                with self.assertRaisesRegex(ValueError, message):
                    lane_launch.validate_capacity(state, self.spec, live, fraction)
        self.state['lanes']['other'] = dict(rounds=1, touches=['alpha'], role='re')
        with self.assertRaisesRegex(ValueError, 'reservation'):
            lane_launch.validate_capacity(self.state, self.spec, ['other'], .75)
        self.spec['lane_role'] = 'glue'
        self.state['lanes'].update({f'g{i}':dict(role='glue', touches=[f'g{i}']) for i in range(3)})
        with self.assertRaisesRegex(ValueError, 'Glue lane cap'):
            lane_launch.validate_capacity(self.state, self.spec, ['g0','g1','g2'], .75)

    def test_legacy_pause_is_claude_and_no_automatic_fallback(self):
        f = Fixture()
        try:
            (f.main/'build-runs/run4-refill-paused.json').write_text('{"reason":"weekly limit"}')
            self.assertIsNotNone(lane_launch.provider_pause(f.main, 'claude'))
            self.assertIsNone(lane_launch.provider_pause(f.main, 'codex'))
        finally:
            f.close()


class PromptAndTransport(unittest.TestCase):
    def test_prompt_reuses_rules_and_stable_prefix(self):
        args = dict(kind='engine', name='test-a', task='only task', maxRounds=1)
        a = lane_backend.render(args, 'codex', 'C:/manifest.json')
        b = lane_backend.render(args | {'name':'test-b'}, 'codex', 'C:/manifest2.json')
        self.assertEqual(a['prompt'].split('## Your lane')[0], b['prompt'].split('## Your lane')[0])
        self.assertIn('Never open, list, grep or read: decomp/', a['prompt'])
        self.assertIn('exec_command uses PowerShell', a['prompt'])
        self.assertNotIn('Co-Authored-By: Claude', a['prompt'])
        self.assertEqual(set(a['schema']['required']), set(a['schema']['properties']))
        c = lane_backend.render(args, 'claude', 'unused')
        self.assertIn('Bash tool is Git Bash', c['prompt'])
        self.assertIn('Co-Authored-By: Claude', c['prompt'])

    def test_subscription_auth_and_hooks_are_required(self):
        m = dict(main='C:/main', worktree='C:/main/.claude/worktrees/test', manifest='C:/manifest.json', kind='tool',
                 git_common='C:/main/.git', git_dir='C:/main/.git/worktrees/test')
        with patch('lane_backend.shutil.which', return_value='codex.exe'), patch('lane_backend.codex_defaults', return_value={}):
            with patch('lane_backend.subprocess.run', return_value=subprocess.CompletedProcess([],0,'Logged in using ChatGPT','')):
                cmd = lane_backend.codex_command(m, 'schema', 'final', {})
                self.assertIn('forced_login_method="chatgpt"', cmd)
                self.assertIn('--ignore-user-config', cmd)
                self.assertIn('--no-daemon', cmd)
                self.assertNotIn('--sandbox', cmd)
                self.assertNotIn('danger-full-access', cmd)
                self.assertIn('default_permissions="fc4-lane"', cmd)
                self.assertTrue(any(x.startswith('shell_environment_policy.set=') and 'maintenance.auto' in x
                                    for x in cmd))
                self.assertTrue(any(c.startswith('hooks.PreToolUse=') for c in cmd))
            with patch('lane_backend.subprocess.run', return_value=subprocess.CompletedProcess([],0,'Logged in using an API key','')):
                with self.assertRaisesRegex(RuntimeError, 'subscription login'):
                    lane_backend.codex_command(m, 'schema', 'final', {})

    def test_environment_excludes_api_keys(self):
        with patch.dict('os.environ', {'OPENAI_API_KEY':'secret','CODEX_API_KEY':'secret','CODEX_HOME':'C:/auth'}):
            env = lane_backend.codex_environment()
            self.assertNotIn('OPENAI_API_KEY', env)
            self.assertNotIn('CODEX_API_KEY', env)
            self.assertEqual(env['CODEX_HOME'], 'C:/auth')


class CodexGitTransport(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='fc4-git-transport-')
        self.main = Path(self.tmp.name)/'main'
        self.main.mkdir()
        git(self.main, 'init', '-q', '-b', 'main')
        git(self.main, 'config', 'user.name', 'fixture')
        git(self.main, 'config', 'user.email', 'fixture@example.invalid')
        (self.main/'seed').write_text('seed')
        git(self.main, 'add', 'seed')
        git(self.main, 'commit', '-qm', 'seed')
        self.wt = self.main/'.claude/worktrees/sample'
        git(self.main, 'worktree', 'add', '-qb', 'sample', str(self.wt))
        self.m = dict(main=str(self.main), worktree=str(self.wt), lane='sample',
                      manifest=str(self.main/'build-runs/manifest.json'))

    def tearDown(self):
        self.tmp.cleanup()

    def commit(self):
        (self.wt/'proof').write_text('proof')
        git(self.wt, 'add', 'proof')
        git(self.wt, 'commit', '-qm', 'proof')
        return lane_backend.git_output(self.m, 'rev-parse', 'HEAD')

    def test_detached_commit_published_only_to_own_ref_and_reattached(self):
        main = lane_backend.git_output(self.m, 'rev-parse', 'main')
        lane_backend.prepare_codex_git(self.m)
        self.assertEqual(subprocess.run(['git','-C',str(self.wt),'symbolic-ref','-q','HEAD'],
                                       capture_output=True).returncode, 1)
        tip = self.commit()
        self.assertEqual(lane_backend.git_output(self.m, 'rev-parse', 'sample'), main)
        lane_backend.publish_codex_git(self.m)
        self.assertEqual(lane_backend.git_output(self.m, 'rev-parse', 'sample'), tip)
        self.assertEqual(lane_backend.git_output(self.m, 'rev-parse', 'main'), main)
        self.assertEqual(lane_backend.git_output(self.m, 'symbolic-ref', 'HEAD'), 'refs/heads/sample')
        self.assertEqual(json.loads(Path(self.m['manifest']).read_text())['git_published'], tip)

    def test_concurrent_branch_move_is_preserved_and_recovery_keeps_detached_tip(self):
        lane_backend.prepare_codex_git(self.m)
        tip = self.commit()
        git(self.main, 'commit', '--allow-empty', '-qm', 'concurrent')
        other = lane_backend.git_output(self.m, 'rev-parse', 'main')
        git(self.main, 'update-ref', 'refs/heads/sample', other, self.m['git_base'])
        with self.assertRaisesRegex(ValueError, 'moved concurrently'):
            lane_backend.publish_codex_git(self.m)
        self.assertEqual(lane_backend.git_output(self.m, 'rev-parse', 'sample'), other)
        self.assertEqual(lane_backend.git_output(self.m, 'rev-parse', 'HEAD'), tip)

    def test_initial_branch_identity_and_permission_roots_fail_closed(self):
        self.m['lane'] = 'different'
        with self.assertRaisesRegex(ValueError, 'own lane branch'):
            lane_backend.prepare_codex_git(self.m)
        self.m['lane'] = 'sample'
        lane_backend.prepare_codex_git(self.m)
        bad = self.m | {'git_dir': str(self.main/'.git')}
        with self.assertRaisesRegex(ValueError, 'permission roots'):
            lane_backend.codex_git_permissions(bad)

    def test_permission_profile_has_no_shared_ref_or_config_write(self):
        lane_backend.prepare_codex_git(self.m)
        args = lane_backend.codex_git_permissions(self.m)
        value = next(x.split('=', 1)[1] for x in args if x.startswith('permissions.fc4-lane.filesystem='))
        rules = tomllib.loads('rules=' + value)['rules']
        common = (self.main/'.git').resolve()
        self.assertNotIn(common.as_posix(), rules)
        self.assertEqual({k for k,v in rules.items() if v == 'write'},
                         {Path(self.m['git_dir']).as_posix(), (common/'objects').as_posix()})
        self.assertNotIn((common/'refs').as_posix(), rules)

    def test_repeated_publication_after_success_is_idempotent(self):
        lane_backend.prepare_codex_git(self.m)
        tip = self.commit()
        lane_backend.publish_codex_git(self.m)
        lane_backend.publish_codex_git(self.m)
        self.assertEqual(lane_backend.git_output(self.m, 'rev-parse', 'sample'), tip)

    def test_claude_still_invokes_saved_workflow_once(self):
        f = Fixture()
        try:
            m = dict(kind='tool', main=str(f.main), manifest=str(f.main/'build-runs/manifest.json'),
                     claude_cli='claude.exe', args=dict(name=LANE,kind='tool',maxRounds=1,task='test'))
            def fake_run(cmd, **kw):
                self.assertEqual(cmd[0], 'claude.exe')
                self.assertIn('--allowedTools', cmd)
                text = kw['input'].decode()
                self.assertIn('saved guarded Workflow fc4-lane exactly ONCE', text)
                self.assertIn('Never direct Agent', text)
                kw['stdout'].write(b'{"is_error":false,"result":"ready","modelUsage":{"test-model":{}}}')
                return subprocess.CompletedProcess(cmd, 0)
            with patch('lane_backend.subprocess.run', side_effect=fake_run):
                result = lane_backend.run_claude(m)
            self.assertFalse(result['is_error'])
            self.assertEqual(result['provider'], 'claude')
        finally:
            f.close()

    def test_refiller_recognizes_both_provider_limits_and_errors(self):
        self.assertTrue(lane_refill.provider_error(dict(is_error=True,result='network failed')))
        self.assertTrue(lane_refill.provider_error(dict(result="You've hit your weekly limit")))
        self.assertTrue(lane_refill.provider_error(dict(result='usage_limit reached')))
        self.assertFalse(lane_refill.provider_error(dict(is_error=False,result='ready: tests passed')))


if __name__ == '__main__':
    unittest.main()
