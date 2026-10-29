"""Executable checks of the replay guards and observable-state comparison.

These small synthetic machines test the tool itself; they are never proof rows.
Run: python -m unittest discover -s tools/prove -t tools -p test_prove.py
"""
import copy
import struct
import unittest
from unittest.mock import patch
from types import SimpleNamespace

from .common import OUT
from .store import PageStore
from .replay import Replay
from .check import same, twice
from .variants import variants, float80
from .compare import call_values
from .calls import CallInputs

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
        target = 0x5f4250
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

    def test_frame_tokens_apply_to_all_call_inputs(self):
        a = dict(target='005f4250', frame=[100, 200], registers=dict(ecx=120, edx=130), arguments=[140, 900])
        b = dict(target='005f4250', frame=[80, 200], registers=dict(ecx=85, edx=95), arguments=[110, 900])
        self.assertEqual(*call_values(a, b))
        b['arguments'][1] = 901
        self.assertNotEqual(*call_values(a, b))

    def test_callee_writes_into_different_local_frames(self):
        target = 0x5f4250
        def caller(base, frame, local):
            b = b'\x83\xec'+bytes([frame])+b'\x8d\x44\x24'+bytes([local])+b'\x50'
            b += b'\xe8'+struct.pack('<I', (target-(base+len(b)+5)) & 0xffffffff)
            return b+b'\x83\xc4'+bytes([frame])+b'\xb8\x12\x00\x00\x00\xc2\x04\x00'
        code = caller(BASE, 24, 12)
        r = self.replay(code)
        original = bytearray(4096)
        data = caller(ADDR, 8, 4)
        original[ADDR & 4095:(ADDR & 4095)+len(data)] = data
        callee = bytearray(4096)
        data = b'\x8b\x54\x24\x04\xc7\x02\x7b\x00\x00\x00\xc2\x04\x00'
        callee[target & 4095:(target & 4095)+len(data)] = data
        a, b = self.store.put_many([bytes(original), bytes(callee)])
        r.m.provider.pages[ADDR & -4096] = a
        r.m.provider.pages[target & -4096] = b
        self.assertIsNone(self.difference(r))

    def test_dead_allocation_push_is_not_an_input(self):
        inputs = CallInputs()
        self.assertNotIn('ecx', inputs.callee(0x5d99a0)['registers'])

    def test_unknown_arity_is_not_padded(self):
        inputs = CallInputs()
        decision = inputs.callee(0x90cf54)
        self.assertIsNone(decision['stack_dwords'])
        self.assertEqual(decision['stack_source'], 'unknown')

    def test_other_enabled_port_replaces_cached_original(self):
        r = self.call_replay()
        target = 0x5f4250
        replacement = b'\xb8\x13\x00\x00\x00\xc2\x04\x00'
        r.m.uc.mem_write(BASE+0x800, replacement)
        initial = bytearray(r.m.entry[BASE])
        initial[0x800:0x800+len(replacement)] = replacement
        r.m.entry[BASE] = bytes(initial)
        r.port.exports[target] = BASE+0x800
        r.enabled[target] = BASE+0x800
        diff = self.difference(r)
        self.assertEqual(diff['field'], 'al')
        self.assertEqual(diff['original'], '0x12')
        self.assertEqual(diff['ours'], '0x13')

    def test_quantum_does_not_invent_calls(self):
        target = 0x5f4250
        def body(base, padding):
            b = b'\x90'*padding+b'\xb8\x80\x00\x00\x00\x48\x75\xfd'
            b += b'\xb9\x00\x00\x00\x00\x68\x11\x00\x00\x00'
            b += b'\xe8'+struct.pack('<I', (target-(base+len(b)+5)) & 0xffffffff)
            return b+b'\xc2\x04\x00'
        r = self.call_replay()
        original = bytearray(4096)
        code = body(ADDR, 0)
        original[ADDR & 4095:(ADDR & 4095)+len(code)] = code
        r.m.provider.pages[ADDR & -4096] = self.store.put_many([bytes(original)])[0]
        code = body(BASE, 13)
        image = code+bytes(4096-len(code))
        r.m.uc.mem_write(BASE, image)
        r.m.entry[BASE] = image
        r.m.quantum = 5
        self.assertIsNone(self.difference(r))
        self.assertEqual(len(r.trace), 1)

    def test_return_pop_derives_unknown_arity(self):
        import capstone
        inputs, base = CallInputs(), 0x12340000
        md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)
        md.detail = True
        code = b'\x8b\x01\xba\x01\x00\x00\x00\x8b\xc2\xc2\x08\x00'
        instructions = {i.address: i for i in md.disasm(code, base)}
        inputs.instruction = instructions.get
        with patch('prove.calls.abi', return_value={}), patch('prove.calls.abi_args', return_value=None):
            decision = inputs.callee(base)
        self.assertEqual(decision['stack_dwords'], 2)
        self.assertEqual(decision['stack_source'], 'callee ret immediate')
        self.assertEqual(decision['registers'], ['ecx'])

    def test_original_caller_cleanup_derives_arity(self):
        import capstone
        inputs, base = CallInputs(), 0x12340000
        md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)
        md.detail = True
        code = b'\x6a\x01\x6a\x02\xe8\x00\x00\x00\x00\x83\xc4\x08\xc3'
        instructions = {i.address: i for i in md.disasm(code, base)}
        inputs.instruction = instructions.get
        with patch('prove.channel.function_at', return_value=None):
            self.assertEqual(inputs.cleanup(base+4), 2)

    def test_batched_cleanup_is_not_one_callee_arity(self):
        # Read releases three calls' arguments together. The last call cannot
        # observe twelve words just because that combined cleanup follows it.
        self.assertIsNone(CallInputs().cleanup(0x6bd9a3))

    def test_register_read_on_either_entry_path_is_observable(self):
        import capstone
        inputs, base = CallInputs(), 0x12340000
        md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)
        md.detail = True
        # One path overwrites EDX; the other returns its incoming value in EAX.
        code = bytes.fromhex('85c07407ba01000000eb028bc2c3')
        inputs.instruction = {i.address: i for i in md.disasm(code, base)}.get
        self.assertEqual(inputs.scan(base)[0], {'edx'})

    def test_edx_requires_fastcall_or_entry_read(self):
        import capstone
        base = 0x12340000
        md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)
        md.detail = True
        instructions = {i.address: i for i in md.disasm(bytes.fromhex('ba01000000c3'), base)}
        for convention, expected in (('unknown', []), ('fastcall', ['ecx', 'edx'])):
            inputs = CallInputs()
            inputs.instruction = instructions.get
            row = dict(convention=convention, reg_args='edx')
            with patch('prove.calls.abi', return_value={base: row}), patch('prove.calls.abi_args', return_value=([], 0)):
                self.assertEqual(inputs.callee(base)['registers'], expected)

    def test_kernel_model_cannot_write_hidden_state(self):
        from unicorn import UC_HOOK_CODE
        r = self.replay()
        def kernel(uc, addr, size, user):
            r.m.write(BASE+0x800, b'\x01\x00\x00\x00')
        r.m.uc.hook_add(UC_HOOK_CODE, kernel, None, BASE, BASE)
        self.assertIn('write into port DLL', self.difference(r)['ours'])


if __name__ == '__main__':
    unittest.main()
