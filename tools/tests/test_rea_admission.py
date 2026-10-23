"""Process-local startup regression; only harmless MCP fixtures, never real REA."""
import json
import os
from pathlib import Path
import subprocess
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT/'.claude/lane-bin'))
sys.path.insert(0, str(ROOT/'tools'))
import laneguard

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
                           params=dict(name='procedure_pseudo_code', arguments={'address': 'fixture'}))
            out, err = p.communicate(json.dumps(request)+'\n', timeout=3)
            self.assertEqual(json.loads(out)['result']['structuredContent']['error'], 'No app is open')
            print('REPRODUCED: tools/list succeeds; independent research client: No app is open')
        finally:
            if p.poll() is None:
                p.kill()
            p.wait()


if __name__ == '__main__':
    unittest.main()
