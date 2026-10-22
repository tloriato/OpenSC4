"""Pinned installed-binary config evidence with a test-owned loopback wire sink.

The sink never services Compile, never starts a daemon and never runs a compiler.
These are capability/config tests, not a service identity or ownership protocol.
"""
import hashlib
import os
from pathlib import Path
import secrets
import shutil
import socket
import struct
import subprocess
import sys
import tempfile
import threading
import unittest

from tools.tests import test_build_singleflight as base

PIN = '7600fb817625313850b8a0525c7743c5d44f53494141390223187e57f394ccb0'
EXE = Path(shutil.which('sccache') or Path.home() / '.cargo/bin/sccache.exe')


@unittest.skipUnless(os.name == 'nt', 'pinned Windows binary capability fixture')
class InstalledModeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        # Other installations are not silently treated as the inspected binary.
        if not EXE.is_file() or hashlib.sha256(EXE.read_bytes()).hexdigest() != PIN:
            raise unittest.SkipTest('exact inspected sccache binary not installed; capability unproved')

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='sccache-wire-', dir=base.ROOT / 'build-runs')
        self.addCleanup(self.tmp.cleanup)
        self.folder = Path(self.tmp.name)

    def probe(self, overrides, config='', reply=None):
        conf = self.folder / 'config.toml'
        conf.write_text(config, encoding='utf-8')
        env = {k: v for k, v in os.environ.items() if not k.upper().startswith('SCCACHE_')}
        env.update(SCCACHE_CONF=str(conf), SCCACHE_DIR=str(self.folder / 'cache'),
                   SCCACHE_IGNORE_SERVER_IO_ERROR='0')
        env.update(overrides)
        requests, errors = [], []
        finished = threading.Event()
        with socket.socket() as server:
            server.bind(('127.0.0.1', 0))
            server.listen()
            server.settimeout(0.1)
            env['SCCACHE_SERVER_PORT'] = str(server.getsockname()[1])

            def sink():
                try:
                    while not finished.is_set():
                        try:
                            peer, _ = server.accept()
                        except socket.timeout:
                            continue
                        with peer:
                            peer.settimeout(2)

                            def read(size):
                                data = b''
                                while len(data) < size:
                                    chunk = peer.recv(size - len(data))
                                    if not chunk:
                                        raise EOFError('partial client frame')
                                    data += chunk
                                return data

                            size = struct.unpack('>I', read(4))[0]
                            if not 4 <= size <= 1024 * 1024:
                                raise ValueError('unbounded frame')
                            data = read(size)
                            requests.append(struct.unpack_from('<I', data)[0])
                            if reply is not None:
                                peer.sendall(reply)
                            # Deliberately disconnect, rather than serve any request.
                except Exception as exc:
                    errors.append(repr(exc))

            thread = threading.Thread(target=sink)
            thread.start()
            job = base.lease._WinJob('Local\\FC4-wire-test-' + secrets.token_hex(16))
            self.addCleanup(job.close)
            try:
                child = job.launch([str(EXE), sys.executable, '--version'],
                                   {'env': env, 'cwd': str(self.folder), 'stdout': subprocess.PIPE,
                                    'stderr': subprocess.PIPE})
                try:
                    out, err = child.communicate(timeout=6)
                except subprocess.TimeoutExpired:
                    # The exact test-owned CLI only; the sink cannot spawn work.
                    child.kill()
                    child.wait()
                    raise
                self.assertTrue(job.drain(3))
            finally:
                finished.set()
                thread.join(3)
            self.assertFalse(thread.is_alive())
            self.assertFalse(errors, errors)
            self.assertNotEqual(child.returncode, 0, (out, err))
            return requests, err.decode(errors='replace')

    def test_actual_effective_mode_config_matrix(self):
        cases = []
        for value in ('true', 'TRUE', 'on', '1'):
            cases.append((value, {'SCCACHE_CLIENT_SIDE': value}, '', 5))
        for value in ('false', 'off', '0', ''):
            cases.append((value, {'SCCACHE_CLIENT_SIDE': value}, '', 4))
        cases.extend([
            ('unset', {}, '', 4),
            ('invalid', {'SCCACHE_CLIENT_SIDE': 'invalid'}, '', None),
            ('file-true', {}, 'client_side_mode = true\n', 5),
            ('file-false-env-true', {'SCCACHE_CLIENT_SIDE': '1'}, 'client_side_mode = false\n', 5),
            ('file-true-env-false', {'SCCACHE_CLIENT_SIDE': '0'}, 'client_side_mode = true\n', 4),
            ('file-true-env-empty', {'SCCACHE_CLIENT_SIDE': ''}, 'client_side_mode = true\n', 5),
            ('file-invalid', {'SCCACHE_CLIENT_SIDE': '1'}, 'client_side_mode = "invalid"\n', None),
            ('log-empty', {'SCCACHE_CLIENT_SIDE': '1', 'SCCACHE_LOG': ''}, '', 4),
            ('log-debug', {'SCCACHE_CLIENT_SIDE': '1', 'SCCACHE_LOG': 'debug'}, '', 4),
            ('error-log-empty', {'SCCACHE_CLIENT_SIDE': '1', 'SCCACHE_ERROR_LOG': ''}, '', 5),
            ('error-log-file', {'SCCACHE_CLIENT_SIDE': '1',
                                'SCCACHE_ERROR_LOG': str(self.folder / 'errors.log')}, '', 5),
            ('distributed', {'SCCACHE_CLIENT_SIDE': '1'},
             '[dist]\nscheduler_url = "http://127.0.0.1:9"\n', 4),
        ])
        for label, env, config, expected in cases:
            with self.subTest(label=label):
                requests, error = self.probe(env, config)
                self.assertEqual(requests, [] if expected is None else [expected], error)

    def test_old_partial_and_malformed_handshake_never_issue_legacy_compile(self):
        for label, reply in (
                ('old-response', struct.pack('>I', 4) + struct.pack('<I', 1)),
                ('partial-header', b'\x00\x00'),
                ('partial-body', struct.pack('>I', 12) + struct.pack('<I', 6)),
                ('malformed-enum', struct.pack('>I', 4) + struct.pack('<I', 0xffffffff))):
            with self.subTest(label=label):
                requests, error = self.probe({'SCCACHE_CLIENT_SIDE': '1'}, reply=reply)
                self.assertEqual(requests, [5], error)
                self.assertNotIn(4, requests)


if __name__ == '__main__':
    unittest.main()
