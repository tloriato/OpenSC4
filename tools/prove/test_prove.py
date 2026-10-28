"""Executable checks of the replay guards and observable-state comparison.

These small synthetic machines test the tool itself; they are never proof rows.
Run: python -m unittest discover -s tools/prove -t tools -p test_prove.py
"""
import copy
import struct
import unittest
from types import SimpleNamespace

from .common import OUT
from .store import PageStore
from .replay import Replay
from .check import same, twice
from .variants import variants, float80

ADDR, BASE, ESP, RET = 0x6bd540, 0x51000000, 0x300800, 0x401000
CODE = bytes.fromhex('8b44240440c20400')  # return argument + 1


class ReplayTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.store = PageStore(OUT / 'tests/pages.db')
        stack = bytearray(4096)
        struct.pack_into('<II', stack, ESP & 4095, RET, 17)
        code = bytearray(4096)
        code[ADDR & 4095:(ADDR & 4095)+len(CODE)] = CODE
        hashes = cls.store.put_many([bytes(code), bytes(stack)])
        cls.rec = dict(function=ADDR, pages=dict(zip(map(str, (ADDR & -4096, ESP & -4096)), hashes)),
                       ret=RET, stack=[0x300000, 0x301000], gdt=0x50000000, syscall_range=None,
                       regions=[[0x300000, 4096, 4096], [ADDR & -4096, 4096, 4096]],
                       entry=dict(fs_base=0, regs=dict(eax=0, ebx=3, ecx=0, edx=0, edi=0, esi=5,
                                                      ebp=0, esp=ESP, eip=ADDR, eflags=2),
                                  fpu=dict(cw=127, sw=0, tw=65535, st='00'*80, mxcsr=0x1f80)))

    @classmethod
    def tearDownClass(cls):
        cls.store.close()

    def replay(self, code=CODE):
        image = code + bytes(4096-len(code))
        return Replay(copy.deepcopy(self.rec), self.store,
                      SimpleNamespace(base=BASE, image=image, exports={ADDR: BASE}), [ADDR])

    def difference(self, r):
        left, _, nd = twice(r, False, ('al', 'ah', 'eaxhi'), collect=True)
        self.assertIsNone(nd)
        right, _, nd = twice(r, True, ('al', 'ah', 'eaxhi'))
        self.assertIsNone(nd)
        return same(r, left, right, ('al', 'ah', 'eaxhi'))

    def test_equivalent_and_deterministic(self):
        self.assertIsNone(self.difference(self.replay()))

    def test_arithmetic(self):
        self.assertEqual(self.difference(self.replay(CODE.replace(b'\x40', b'\x48')))['field'], 'al')

    def test_x87_extra(self):
        self.assertEqual(self.difference(self.replay(b'\xd9\xe8'+CODE))['field'], 'top')

    def test_control_word(self):
        code = b'\x68\x7f\x04\x00\x00\xd9\x2c\x24\x83\xc4\x04'+CODE
        self.assertEqual(self.difference(self.replay(code))['field'], 'cw')

    def test_mxcsr(self):
        code = b'\x68\x80\x3f\x00\x00\x0f\xae\x14\x24\x83\xc4\x04'+CODE
        self.assertEqual(self.difference(self.replay(code))['field'], 'mxcsr')

    def test_direction_flag(self):
        self.assertEqual(self.difference(self.replay(b'\xfd'+CODE))['field'], 'eflags')

    def test_register_variant(self):
        r = self.replay(b'\x31\xff'+CODE)
        self.assertIsNone(self.difference(r))
        inp, state, _ = list(variants(r.rec, r, {}, 2, 4036))[1]
        self.assertEqual(inp['target'], 'edi')
        r.m.state = state
        self.assertEqual(self.difference(r)['field'], 'edi')

    def test_hidden_state(self):
        code = b'\xc7\x05'+struct.pack('<I', BASE+0x800)+b'\x01\x00\x00\x00'+CODE
        self.assertIn('write into port DLL', self.difference(self.replay(code))['ours'])

    def test_original_entry(self):
        code = b'\xb8'+struct.pack('<I', ADDR)+b'\xff\xe0'
        self.assertIn('original body', self.difference(self.replay(code))['ours'])

    def test_original_interior(self):
        code = b'\xb8'+struct.pack('<I', ADDR+5)+b'\xff\xe0'
        self.assertIn('original body', self.difference(self.replay(code))['ours'])

    def test_uncaptured_page(self):
        code = b'\xa1\x00\x00\x00\x22'+CODE
        self.assertIn('uncaptured memory', self.difference(self.replay(code))['ours'])

    def test_live_stack_write(self):
        code = b'\xc7\x44\x24\x08\x01\x00\x00\x00'+CODE
        self.assertTrue(self.difference(self.replay(code))['field'].startswith('memory'))

    def test_dead_stack_write(self):
        code = b'\xc7\x44\x24\xfc\x01\x00\x00\x00'+CODE
        self.assertIsNone(self.difference(self.replay(code)))

    def test_gdt_is_not_input(self):
        r = self.replay()
        _, reads, _ = twice(r, False, ('al',), collect=True)
        self.assertFalse(reads)

    def test_float80(self):
        self.assertEqual(float80(0x3f800000)[0].hex(), '0000000000000080ff3f')
        self.assertEqual(float80(0x80000000)[1], 1)
        self.assertEqual(float80(0x7f800000)[1], 2)

    def call_replay(self, port_calls=1, ecx=0, arg=17):
        target = 0x808620
        def caller(base, count, cx=0, argument=17):
            b = b'\xb9'+struct.pack('<I', cx)
            for _ in range(count):
                b += b'\x68'+struct.pack('<I', argument)
                b += b'\xe8'+struct.pack('<I', (target-(base+len(b)+5)) & 0xffffffff)
            if count == 0:
                b += b'\xb8\x12\x00\x00\x00'
            return b+b'\xc2\x04\x00'
        r = self.replay(caller(BASE, port_calls, ecx, arg))
        original = bytearray(4096)
        b = caller(ADDR, 1)
        original[ADDR & 4095:(ADDR & 4095)+len(b)] = b
        callee = bytearray(4096)
        callee[target & 4095:(target & 4095)+8] = b'\xb8\x12\x00\x00\x00\xc2\x04\x00'
        hashes = self.store.put_many([bytes(original), bytes(callee)])
        r.m.provider.pages[ADDR & -4096] = hashes[0]
        r.m.provider.pages[target & -4096] = hashes[1]
        return r

    def test_equivalent_external_call(self):
        self.assertIsNone(self.difference(self.call_replay()))

    def test_missing_call(self):
        self.assertTrue(self.difference(self.call_replay(0))['field'].startswith('call['))

    def test_extra_call(self):
        self.assertTrue(self.difference(self.call_replay(2))['field'].startswith('call['))

    def test_call_ecx(self):
        self.assertTrue(self.difference(self.call_replay(ecx=4))['field'].startswith('call['))

    def test_call_argument(self):
        self.assertTrue(self.difference(self.call_replay(arg=18))['field'].startswith('call['))

    def test_clock_at_new_address(self):
        r = self.replay(b'\x0f\x31\xc2\x04\x00')
        original = bytearray(4096)
        original[ADDR & 4095:(ADDR & 4095)+5] = b'\x0f\x31\xc2\x04\x00'
        h = self.store.put_many([bytes(original)])[0]
        r.m.provider.pages[ADDR & -4096] = h
        self.assertIsNone(self.difference(r))

    def test_loop_is_bounded(self):
        r = self.replay(b'\xeb\xfe')
        right, _ = r.run(port=True, timeout=.01)
        self.assertEqual(right['kind'], 'timeout')

    def test_captured_user_segments(self):
        from unicorn.x86_const import UC_X86_REG_EAX
        for instruction, expected in ((b'\x66\x8c\xc8', 0x23), (b'\x66\x8c\xd0', 0x2b),
                                      (b'\x66\x8c\xe0', 0x53)):
            code = b'\x31\xc0'+instruction+b'\xc2\x04\x00'
            rec = copy.deepcopy(self.rec)
            rec['entry']['regs'].update(cs=0x23, ss=0x2b, ds=0x2b, es=0x2b, gs=0x2b, fs=0x53)
            original = bytearray(4096)
            original[ADDR & 4095:(ADDR & 4095)+len(code)] = code
            rec['pages'][str(ADDR & -4096)] = self.store.put_many([bytes(original)])[0]
            r = Replay(rec, self.store, SimpleNamespace(base=BASE, image=code+bytes(4096-len(code)),
                                                       exports={ADDR: BASE}), [ADDR])
            self.assertIsNone(self.difference(r))
            self.assertEqual(r.m.uc.reg_read(UC_X86_REG_EAX), expected)


if __name__ == '__main__':
    unittest.main()
