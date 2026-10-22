"""Actual launch-boundary rejection; no claimed service-ownership repair.

Fixture payloads are harmless Python. Real wrapper dispatch, canonical OS lease,
delegation and log/verdict publication execute without replacing the guard.
"""
import os
from pathlib import Path
import subprocess
import sys
import unittest

from tools.tests import test_build_singleflight as base

lease = base.lease
from build_cache import CacheOwnershipError, require_output_ownership


@unittest.skipUnless(base.SUPPORTED, "requires actual lease peer authentication")
class CacheBoundaryTests(unittest.TestCase):
    setUp = base.SingleflightTests.setUp
    cleanup_processes = base.SingleflightTests.cleanup_processes
    worktree = base.SingleflightTests.worktree
    start = base.SingleflightTests.start
    event = base.SingleflightTests.event
    go = base.SingleflightTests.go
    finish = base.SingleflightTests.finish
    owner = base.SingleflightTests.owner
    sentinels = base.SingleflightTests.sentinels
    assert_unchanged = base.SingleflightTests.assert_unchanged
    busy = base.SingleflightTests.busy

    def boundary_case(self, kind, steps, *, file_log=False):
        files = self.sentinels()
        child = self.start(kind, steps, env={'FC4_TEST_CACHE_WRAPPER': 'unverified-cache.exe',
                                            'FC4_TEST_FILE_LOG': '1' if file_log else ''})
        if kind in ('fc4verify', 'imported') or steps == ('quick',):
            held, _ = self.event('context')
            self.go(held)
        held, _ = self.event('cache-entry')
        self.assertEqual(self.owner()['status'], 'active')
        self.busy()
        self.busy('imported', ())
        self.assert_unchanged(files)
        self.go(held)
        text = self.finish(child, ok=False)
        self.assertIn('service output ownership unverified', text)
        self.assertNotIn('PASS', text)
        self.assert_unchanged(files)
        self.assertEqual(self.owner()['status'], 'released')
        # Nothing was launched at the guarded boundary. A real uncached fixture
        # can acquire the same lease without archiving metadata or deleting locks.
        recovery = self.start('qbuild', ('test',))
        self.finish(recovery, ('test',))
        self.assertEqual(self.owner()['status'], 'released')

    def test_actual_qbuild_log_and_contenders(self):
        self.boundary_case('qbuild', ('test',))

    def test_actual_cli_verify_preserves_verdict(self):
        self.boundary_case('fc4verify', ('quick',), file_log=True)

    def test_actual_imported_verify_preserves_verdict(self):
        self.boundary_case('imported', ())

    def test_actual_qbuild_nested_delegation(self):
        self.boundary_case('qbuild', ('quick',))

    def test_actual_direct_lease_capture_and_file(self):
        for file_log in (False, True):
            with self.subTest(file_log=file_log):
                self.boundary_case('lease-cache', (), file_log=file_log)

    def test_explicit_empty_environment_is_not_inherited(self):
        old = os.environ.get('RUSTC_WRAPPER')
        os.environ['RUSTC_WRAPPER'] = 'unverified-cache.exe'
        try:
            with lease.Lease(self.wt) as owned:
                with self.assertRaisesRegex(lease.LeaseError, 'service output ownership unverified'):
                    owned.run([sys.executable, '-c', 'raise SystemExit(0)'], capture_output=True)
                # Passing env={} must not restore the inherited wrapper.
                result = owned.run([sys.executable, '-c', 'print("EMPTY_ENV_OK")'], env={},
                                   capture_output=True, timeout=10)
                self.assertEqual(result.returncode, 0)
                self.assertIn(b'EMPTY_ENV_OK', result.stdout)
        finally:
            if old is None:
                os.environ.pop('RUSTC_WRAPPER', None)
            else:
                os.environ['RUSTC_WRAPPER'] = old


class QuarantinePolicyTests(unittest.TestCase):
    def test_all_intents_reject_without_service_proof(self):
        for value in ('true', 'on', '1', 'false', 'off', '0', '', 'invalid'):
            with self.subTest(value=value), self.assertRaisesRegex(CacheOwnershipError, 'no legacy'):
                require_output_ownership(['cargo', 'test'],
                                         {'RUSTC_WRAPPER': 'sccache', 'SCCACHE_CLIENT_SIDE': value})

    def test_logging_settings_are_distinct(self):
        for value in ('', 'debug'):
            with self.assertRaisesRegex(CacheOwnershipError, 'SCCACHE_LOG is present'):
                require_output_ownership(['cargo', 'test'],
                                         {'RUSTC_WRAPPER': 'sccache', 'SCCACHE_LOG': value})
        with self.assertRaises(CacheOwnershipError) as result:
            require_output_ownership(['cargo', 'test'], {'RUSTC_WRAPPER': 'sccache',
                                     'SCCACHE_CLIENT_SIDE': '1', 'SCCACHE_ERROR_LOG': 'kept.log'})
        self.assertNotIn('SCCACHE_LOG is present', str(result.exception))

    def test_direct_executable_cmake_and_inherited_config_environment(self):
        for cmd, env in ((['sccache', 'rustc'], {}), (['sccache.exe', 'cl.exe'], {}),
                         (['cmake', '-DFC4_COMPILER_CACHE=ON'], {}),
                         (['cmake', '-DCMAKE_CXX_COMPILER_LAUNCHER=untrusted'], {}),
                         (['cargo', 'test'], {'CARGO_BUILD_RUSTC_WRAPPER': 'untrusted'}),
                         (['cargo', 'test'], {'RUSTC_WORKSPACE_WRAPPER': 'untrusted'})):
            with self.subTest(cmd=cmd, env=env), self.assertRaises(CacheOwnershipError):
                require_output_ownership(cmd, env)

    def test_uncached_and_clippy_wrapper_removal_stays_uncached(self):
        env = {'SCCACHE_SERVER_PORT': '4300', 'SCCACHE_IDLE_TIMEOUT': '900',
               'SCCACHE_CLIENT_SIDE': 'false', 'RUSTC_WRAPPER': ''}
        require_output_ownership(['cargo', 'clippy'], env)
        require_output_ownership(['cmake', '-DFC4_COMPILER_CACHE=OFF'], env)
        self.assertEqual(env['SCCACHE_CLIENT_SIDE'], 'false')


if __name__ == '__main__':
    unittest.main()
