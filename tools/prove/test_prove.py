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
from .variants import variants, float80, typed_values
from .compare import call_values
from .calls import CallInputs

ADDR, BASE, ESP, RET = 0x6bd540, 0x51000000, 0x300800, 0x401000
CODE = bytes.fromhex('8b44240440c20400')  # return argument + 1


def concurrent_proven_fixture(path,base,barrier):
    from pathlib import Path
    from . import check as checker
    checker.PROVEN=Path(path)
    barrier.wait(timeout=30)
    for i in range(20):
        checker.update_proven([dict(address=f'{base+i:08x}',name='fixture',owner='synthetic',
            verdict='EXACT',captures=1,variants=1,skipped=0,scenarios='synthetic',dll_sha256='fixture')])


class ReplayTests(unittest.TestCase):
    def test_kernel_shared_fields_are_named_and_byte_logged(self):
        from .validation import normalize_kernel, SHARED
        initial = {SHARED: bytes(4096)}
        actual = bytearray(4096)
        actual[0x36a] = 8
        actual[0x360] = 42  # QpcSystemTimeIncrementShift is not a changing counter
        a, b, rules = normalize_kernel({SHARED: bytes(actual)}, {}, initial)
        self.assertEqual(a[SHARED][0x36a], b[SHARED][0x36a])
        self.assertNotEqual(a[SHARED][0x360], b[SHARED][0x360])
        self.assertEqual(rules, [dict(rule='kernel maintained shared input', field='UnparkedProcessorCount',
                                    address='7ffe036a', cpu=8, replay=0)])

    def test_debugger_paused_work_does_not_consume_thread_hold(self):
        from .debugger import Debugger
        dbg = Debugger.__new__(Debugger)
        dbg.owner, dbg.running_seconds, dbg.running_since = 12, 0.0, 100.0
        dbg.pause_clock(100.2)
        dbg.pause_clock(125.0)  # 24.8 seconds of entry/exit callbacks
        self.assertAlmostEqual(dbg.running_seconds, .2)
        dbg.running_since = 125.0
        dbg.pause_clock(125.3)
        self.assertAlmostEqual(dbg.running_seconds, .5)

    def test_failed_thread_suspension_never_claims_isolation(self):
        from .debugger import Debugger,k32
        dbg=Debugger.__new__(Debugger)
        dbg.owner=None;dbg.exits=[{}];dbg.suspended=[]
        dbg.proc=SimpleNamespace(threads={1:123,2:456},cpu_seconds=lambda tid:0)
        with patch.object(k32,'Wow64SuspendThread',return_value=0xffffffff), \
             patch.object(k32,'WaitForSingleObject',return_value=0x102):
            dbg.suspend_others(1)
        self.assertFalse(dbg.exits[0]['isolation']['held'])
        self.assertEqual(dbg.exits[0]['isolation']['thread'],2)

    def test_kernel_shared_flag_exception_is_only_the_error_port_bit(self):
        from .validation import normalize_kernel,shared_page_audit,SHARED
        initial={SHARED:bytes(4096)}
        changed=bytearray(4096);changed[0x2f0]=3
        actual={SHARED:bytes(changed)}
        a,b,rules=normalize_kernel(actual,{},initial)
        self.assertEqual(a[SHARED][0x2f0],2)
        self.assertEqual(b[SHARED][0x2f0],0)
        self.assertEqual(rules[0]['mask'],1)
        self.assertEqual(len(shared_page_audit(actual,{},initial)),1)

    def test_file_position_syscall_only_services_supported_class(self):
        from unicorn.x86_const import UC_X86_REG_ESP, UC_X86_REG_EAX
        r = self.replay()
        m = r.m
        m.ensure(ESP, 512)
        m.uc.reg_write(UC_X86_REG_ESP, ESP)
        m.uc.reg_write(UC_X86_REG_EAX, 0x27)
        m.write(ESP, struct.pack('<7I', RET, 0, 100, ESP+100, ESP+120, 8, 14))
        m.write(ESP+100, b'\xff'*8)
        m.write(ESP+120, struct.pack('<q', 98765))
        m._syscall(m.uc, 0, 0, None)
        self.assertEqual(m.read(ESP+100, 8), bytes(8))
        self.assertEqual(m.uc.reg_read(UC_X86_REG_EAX), 0)
        self.assertEqual(m.syscalls, [('NtSetInformationFile position', 100, 98765)])
        m.uc.reg_write(UC_X86_REG_ESP, ESP)
        m.uc.reg_write(UC_X86_REG_EAX, 0x27)
        m.write(ESP+24, struct.pack('<I', 20))
        m._syscall(m.uc, 0, 0, None)
        self.assertEqual(m.syscalls[-1][0], 'unhandled NtSetInformationFile arguments')

    def test_revalidation_repairs_gdt_but_preserves_lost_isolation(self):
        from .revalidate import validate_record
        r = self.replay()
        out = r.run()
        rec = copy.deepcopy(self.rec)
        rec['exit'] = copy.deepcopy(rec['entry'])
        rec['exit']['regs'].update(out[0]['regs'])
        rec['exit_pages'] = {str(p):self.store.put_many([b])[0] for p,b in out[1].items()}
        rec['validation'] = dict(valid=False, difference=dict(field='page 50000000'))
        result = validate_record(rec, self.store)
        self.assertTrue(result['valid'], result)
        rec['validation']['difference'] = dict(field='other threads resumed while function was running')
        result = validate_record(rec, self.store)
        self.assertFalse(result['valid'])
        self.assertEqual(result['difference'], rec['validation']['difference'])

    def test_recursive_return_stops_survive_machine_reuse(self):
        # The captured activation returns into the same function as its
        # deeper recursive calls. Each deeper return must execute INC EBX.
        code = bytes.fromhex('8b44240485c074084850e8f1ffffff43c20400')
        after = ADDR+15
        from .machine import Machine
        from .replay import Provider
        m = Machine(self.rec['entry'], Provider(self.rec, self.store), gdt=self.rec['gdt'])
        m.ensure(ADDR, len(code))
        m.ensure(ESP, 8)
        m.entry[ADDR & -4096] = m.entry[ADDR & -4096][:ADDR % 4096]+code+m.entry[ADDR & -4096][ADDR % 4096+len(code):]
        m.uc.mem_write(ADDR, code)
        stack = bytearray(m.entry[ESP & -4096])
        struct.pack_into('<II', stack, ESP % 4096, after, 3)
        m.entry[ESP & -4096] = bytes(stack)
        m.uc.mem_write(ESP & -4096, bytes(stack))
        outcomes=[]
        for _ in range(3):
            out=m.run(ADDR, after, ESP, instruction_budget=10000)
            outcomes.append({k:v for k,v in out.items() if k!='seconds'})
            self.assertIsNone(out['err'], out)
            self.assertEqual(out['regs']['ebx'], self.rec['entry']['regs']['ebx']+3)
            self.assertEqual(out['regs']['esp'], ESP+8)
            m.restore()
        self.assertEqual(outcomes[0], outcomes[1])
        self.assertEqual(outcomes[0], outcomes[2])

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

    def observer_replay(self, foreign=None, pin=True, readonly=False, clobber=False, game_call=False):
        from .observer import PIN
        observer, data = 0x5c001000, 0x5d8b9000
        # The observer remembers the caller's stack address. The two tested
        # implementations deliberately use different amounts of local storage.
        helper = b'\x89\x25'+struct.pack('<I', data)+b'\xc3'
        def body(base, locals_size):
            start = b'\x83\xec'+bytes([locals_size])
            if game_call:
                start += b'\x6a\x11'
                start += b'\xe8'+struct.pack('<i',0x5f4250-(base+len(start)+5))
            call_pc = base+len(start)
            call = b'\xe8'+struct.pack('<i', observer-call_pc-5)
            # Foreign writes remain observable even if the observer overwrites
            # them afterwards; append another observer call for that case.
            extra = b''
            if base == BASE and foreign:
                extra = b'\xc6\x05'+struct.pack('<I', data)+b'\x42'
                if foreign == 'straddle':
                    extra = b'\xc7\x05'+struct.pack('<I', data-1)+bytes(4)
                if foreign in ('before', 'straddle'):
                    pc = base+len(start)+len(call)+len(extra)
                    extra += b'\xe8'+struct.pack('<i', observer-pc-5)
            if base == BASE and clobber:
                extra += b'\x31\xff'
            return start+call+extra+b'\x83\xc4'+bytes([locals_size])+CODE
        original, port = body(ADDR, 8), body(BASE, 24)
        rec = copy.deepcopy(self.rec)
        if pin:
            rec['pinned'] = dict(dll_sha256=PIN)
        rec['regions'] += [[observer, 4096, 0x1000, 0x20, 0x1000000, r'C:\Plugins\FreeCity4.dll'],
                           [data-4096, 4096, 0x1000, 2, 0x1000000, r'C:\Plugins\FreeCity4.dll'],
                           [data, 4096, 0x1000, 2 if readonly else 4, 0x1000000, r'C:\Plugins\FreeCity4.dll']]
        game = bytearray(4096)
        game[ADDR % 4096:ADDR % 4096+len(original)] = original
        for addr, contents in ((ADDR & -4096, bytes(game)), (observer, helper+bytes(4096-len(helper))),
                               (data-4096, bytes(4096)), (data, bytes(4096))):
            rec['pages'][str(addr)] = self.store.put_many([contents])[0]
        if game_call:
            p=bytearray(4096);callee=bytes.fromhex('b812000000c20400')
            p[0x250:0x250+len(callee)]=callee
            rec['pages'][str(0x5f4000)]=self.store.put_many([bytes(p)])[0]
        return Replay(rec, self.store, SimpleNamespace(base=BASE, image=port+bytes(4096-len(port)),
                                                      exports={ADDR:BASE}), [ADDR])

    def test_observer_private_frame_write_is_exact_and_logged(self):
        r = self.observer_replay()
        self.assertIsNone(self.difference(r))
        rules = list(r.observer_exclusions.values())
        self.assertEqual([v['address'] for v in rules], ['5d8b9000'])
        self.assertEqual(rules[0]['writers'], ['5c001000'])
        self.assertEqual(rules[0]['rule'], 'pinned observer private write')
        self.assertEqual(r.observer_excluded_byte_events, 1)

    def test_port_write_to_observer_page_still_differs(self):
        for order in ('after', 'before', 'straddle'):
            with self.subTest(order=order):
                r = self.observer_replay(foreign=order)
                self.assertEqual(self.difference(r)['field'], 'memory 5d8b9000')
                self.assertFalse(r.observer_exclusions)

    def test_observer_rule_requires_pin_and_writable_region(self):
        for options in (dict(pin=False), dict(readonly=True)):
            with self.subTest(options=options):
                r = self.observer_replay(**options)
                self.assertEqual(self.difference(r)['field'], 'memory 5d8b9000')
                self.assertFalse(r.observer_exclusions)

    def test_substituted_timer_entry_is_a_reference_limitation(self):
        r=self.observer_replay()
        # An observer-owned replacement may return a schedule constant instead
        # of running the actual timer arithmetic. Do not blame a native port
        # for reading an input that this replacement never reads.
        data=bytearray(r.m.provider.get(ADDR & -4096))
        data[ADDR % 4096:ADDR % 4096+5]=b'\xe9'+struct.pack('<i',0x5c001000-ADDR-5)
        r.m.provider.pages[ADDR & -4096]=self.store.put_many([bytes(data)])[0]
        with patch('prove.observer.SUBSTITUTED_ENTRIES',{ADDR:'test schedule replacement'}):
            d=r.observer.reference_limitation(r.rec,r.m.provider)
            self.assertEqual(d['field'],'observer replaces tested entry')
            self.assertEqual(d['target'],'5c001000')
        self.assertIsNone(r.observer.reference_limitation(r.rec,r.m.provider))

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

    def test_image_boundary_write_is_guarded_and_restored(self):
        code=b'\x66\xc7\x05'+struct.pack('<I',BASE-1)+b'\x34\x12'+CODE
        r=self.replay(code)
        r.m.map_page(BASE-4096, bytes(4096))
        self.assertIn('write into port DLL', self.difference(r)['ours'])
        self.assertEqual(r.m.read(BASE, len(code)), code)

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

    def test_operator_new_constructor_initializes_both_reserved_words(self):
        inputs=CallInputs()
        self.assertEqual(inputs.callee(0x9133da)['registers'],[])
        for addr in (0x9133dd,0x9133de):
            self.assertTrue(inputs.dead_allocation_push(inputs.instruction(addr)))
        # The initializer's two stores precede its first outgoing call and
        # cover precisely the two words at EBP-8 and EBP-4 in operator new.
        for addr,offset in ((0x48c9f7,0),(0x48c9fd,4)):
            ins=inputs.instruction(addr)
            self.assertEqual(ins.mnemonic,'mov')
            self.assertEqual(ins.reg_name(ins.operands[0].mem.base),'esi')
            self.assertEqual(ins.operands[0].mem.disp,offset)

    def test_partial_register_writes_preserve_only_unwritten_input_bits(self):
        cases=(('b2000fb6c2c3',0),             # DL is written before reading DL
               ('66ba00000fb6c2c3',0),         # DX is written before reading DL
               ('b6000fb6c2c3',0xff),          # writing DH preserves incoming DL
               ('b2008bc2c3',0xffffff00))      # upper EDX survives writing DL
        for code,expected in cases:
            with self.subTest(code=code):
                inputs=CallInputs();base=0x1000
                ins={i.address:i for i in inputs.md.disasm(bytes.fromhex(code),base)}
                inputs.instruction=ins.get
                regs,_,unknown=inputs.scan(base)
                self.assertFalse(unknown)
                self.assertEqual(inputs.input_masks[base,0]['edx'],expected)
                self.assertEqual('edx' in regs,bool(expected))

    def test_partial_register_masks_propagate_through_direct_calls(self):
        inputs=CallInputs();base=0x1000
        # Parent writes DL, child reads DL and DH: only incoming DH is observed.
        parent=bytes.fromhex('b200e8f9000000c3')
        child=bytes.fromhex('0fb6c20fb6c6c3')
        ins={i.address:i for b,data in ((base,parent),(0x1100,child)) for i in inputs.md.disasm(data,b)}
        inputs.instruction=ins.get
        regs,_,unknown=inputs.scan(base)
        self.assertFalse(unknown)
        self.assertEqual(regs,{'edx'})
        self.assertEqual(inputs.input_masks[base,0]['edx'],0xff00)

    def test_input_preserved_through_child_is_still_read_after_call(self):
        for child,expected in (('b801000000c3',0xffffffff),
                               ('ba01000000c3',0),('b200c3',0xffffff00)):
            with self.subTest(child=child):
                inputs=CallInputs();base=0x1000
                parent=bytes.fromhex('e8fb0000008bc2c3')
                ins={i.address:i for start,data in ((base,parent),(0x1100,bytes.fromhex(child)))
                     for i in inputs.md.disasm(data,start)}
                inputs.instruction=ins.get
                inputs.scan(base)
                self.assertEqual(inputs.input_masks[base,0]['edx'],expected)

    def test_network_setter_does_not_take_incoming_edx(self):
        inputs=CallInputs();d=inputs.callee(0x61b480)
        self.assertEqual(d['registers'],['ecx'])
        self.assertEqual(d['entry_read_masks']['edx'],'00000000')

    def test_typed_sweeps_include_width_boundaries_and_binary64_specials(self):
        self.assertTrue({0,0xff,0x7f,0x80} <= set(typed_values(1)))
        self.assertTrue({0,0xffff,0x7fff,0x8000} <= set(typed_values(2)))
        self.assertTrue({0,0xffffffffffffffff,0x7fffffffffffffff,0x8000000000000000,
                         0x7ff0000000000000,0xfff0000000000000,0x7ff8000000000000,
                         1,0x43e0000000000000} <= set(typed_values(8)))

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

    def test_jump_quantum_does_not_invent_entry_calls(self):
        target=0x5f4250
        r=self.call_replay()
        # Replace the callee by a captured detour that forwards to its body.
        # Its entry can be revisited when an instruction quantum ends there.
        helper=0x5c001000
        page=bytearray(4096)
        page[target % 4096:target % 4096+5]=b'\xe9'+struct.pack('<i',helper-target-5)
        body=b'\xb8\x12\x00\x00\x00\xc2\x04\x00'
        page[target % 4096+5:target % 4096+5+len(body)]=body
        trampoline=b'\xe9'+struct.pack('<i',target+5-helper-5)
        for addr,data in ((target & -4096,bytes(page)),(helper,trampoline+bytes(4096-len(trampoline)))):
            r.m.provider.pages[addr]=self.store.put_many([data])[0]
        for quantum in (1,2,3,5,7):
            with self.subTest(quantum=quantum):
                r.m.quantum=quantum
                self.assertIsNone(self.difference(r))
                self.assertEqual(len(r.trace),1)

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
        for convention, expected in (('unknown', []), ('fastcall', ['ecx'])):
            inputs = CallInputs()
            inputs.instruction = instructions.get
            row = dict(convention=convention, reg_args='edx')
            with patch('prove.calls.abi', return_value={base: row}), patch('prove.calls.abi_args', return_value=([], 0)):
                self.assertEqual(inputs.callee(base)['registers'], expected)

    def test_timer_edx_is_proven_dead_even_with_fastcall_label(self):
        for target in (0x68bf80, 0x8905c4):
            d = CallInputs().callee(target)
            self.assertEqual(d['registers'], ['ecx'])
            self.assertFalse(d['entry_scan_incomplete'])
            self.assertEqual(d['proven_dead_declared_registers'], ['edx'])

    def test_unknown_fastcall_path_retains_edx(self):
        inputs, base = CallInputs(), 0x12340000
        instructions = {i.address:i for i in inputs.md.disasm(bytes.fromhex('ffe0'), base)}
        inputs.instruction = instructions.get
        with patch('prove.calls.abi', return_value={base:dict(convention='fastcall')}), \
             patch('prove.calls.abi_args', return_value=([],0)):
            self.assertEqual(inputs.callee(base)['registers'], ['ecx','edx'])

    def test_thread_context_helper_has_no_stack_inputs(self):
        d = CallInputs().callee(0x9f7604)
        self.assertEqual(d['stack_dwords'], 0)
        self.assertEqual(d['registers'], [])

    def test_callee_cleanup_corrects_batched_abi_argument_count(self):
        inputs=CallInputs()
        d=inputs.callee(0x91286f)  # Date.Month uses three local output slots, no caller arguments.
        self.assertEqual(d['stack_dwords'], 0)
        self.assertIn('verified Date.Month',d['stack_source'])
        base=0x12340000
        inputs.instruction={i.address:i for i in inputs.md.disasm(bytes.fromhex('c20800'),base)}.get
        # A convention label alone does not establish whether a nonstandard
        # caller cleans its arguments. Preserve other known ABI counts.
        for convention,expected in (('thiscall',1),('cdecl',1)):
            inputs.decisions.clear()
            with patch('prove.calls.abi',return_value={base:dict(convention=convention)}), \
                 patch('prove.calls.abi_args',return_value=([],1)):
                self.assertEqual(inputs.callee(base)['stack_dwords'],expected)

    def test_push_ret_forwarding_entry_is_an_external_tail_call(self):
        target = 0x5f4250
        r = self.call_replay()
        # Both tested functions forward the incoming argument/return address.
        original = b'\xe9'+struct.pack('<i', target-ADDR-5)
        page = bytearray(4096)
        page[ADDR % 4096:ADDR % 4096+len(original)] = original
        r.m.provider.pages[ADDR & -4096] = self.store.put_many([bytes(page)])[0]
        code = b'\x68'+struct.pack('<I', target)+b'\xc3'
        image = code+bytes(4096-len(code))
        r.m.uc.mem_write(BASE, image)
        r.m.entry[BASE] = image
        self.assertIsNone(self.difference(r))
        self.assertEqual(len(r.trace), 1)
        self.assertEqual(r.trace[0]['target'], f'{target:08x}')

    def test_return_channel_ignores_overwritten_allocation_push(self):
        from .channel import reads_after
        inputs, base = CallInputs(), 0x12340000
        # Saving an output register to reserve a local does not observe it
        # when the local is overwritten before any read.
        ins = {i.address:i for i in inputs.md.disasm(bytes.fromhex('5189042483c404c3'), base)}
        inputs.instruction = ins.get
        with patch('prove.channel._insn', side_effect=ins.get), \
             patch('prove.channel.EXE_LO', 0), patch('prove.channel.EXE_HI', 0xffffffff), \
             patch('prove.channel._ret_reads', return_value=set()), \
             patch('prove.calls.CALL_INPUTS', inputs):
            self.assertNotIn('ecx', reads_after(base))

    def test_stack_only_import_does_not_consume_returned_ecx(self):
        from .channel import reads_after
        from .common import exe
        slot=next(a for a,(dll,name) in exe().imports.items() if dll=='kernel32.dll' and name=='GetLastError')
        inputs=CallInputs();base=0x12340000
        code=b'\xff\x15'+struct.pack('<I',slot)+b'\xc3'
        ins={i.address:i for i in inputs.md.disasm(code,base)}
        with patch('prove.channel._insn',side_effect=ins.get), \
             patch('prove.channel.EXE_LO',0),patch('prove.channel.EXE_HI',0xffffffff), \
             patch('prove.channel._ret_reads',return_value=set()):
            self.assertNotIn('ecx',reads_after(base))

    def test_kernel_model_cannot_write_hidden_state(self):
        from unicorn import UC_HOOK_CODE
        r = self.replay()
        def kernel(uc, addr, size, user):
            r.m.write(BASE+0x800, b'\x01\x00\x00\x00')
        r.m.uc.hook_add(UC_HOOK_CODE, kernel, None, BASE, BASE)
        self.assertIn('write into port DLL', self.difference(r)['ours'])

    def test_worker_count_preserves_verdict_first_difference_and_proven(self):
        import contextlib
        import gzip
        import io
        import json
        from argparse import Namespace
        from . import check as checker
        root = OUT/'tests/parallel'
        root.mkdir(parents=True, exist_ok=True)
        r = self.observer_replay(clobber=True,game_call=True)
        out, pages = r.run()
        rec = copy.deepcopy(r.rec)
        rec.update(scenario='synthetic', validation=dict(valid=True, clock_rules=[]),
                   exit_pages={str(p):self.store.put_many([data])[0] for p,data in pages.items()})
        rec['exit'] = dict(regs={**out['regs'], 'eflags':out['eflags']},
                           fpu=copy.deepcopy(rec['entry']['fpu']))
        paths = []
        for i in range(2):
            path = root/f'capture-{i}.json.gz'
            with gzip.open(path, 'wt') as f:
                json.dump(rec, f)
            paths.append(path)
        dummy = root/'fixture.dll'
        dummy.write_bytes(b'synthetic test image supplied by the loader fixture')
        reports, tables, decisions = [], [], []
        for keep_going,jobs in ((True,1),(True,2),(False,1),(False,2)):
            table, report = root/f'proven-{jobs}-{keep_going}.tsv', root/f'check-{jobs}-{keep_going}.json'
            table.unlink(missing_ok=True)
            args = Namespace(dll=dummy, variants=64, seed=0xfc4, jobs=jobs, variant_batch=8,
                             keep_going=keep_going, no_proven=False, report=report)
            with patch.object(checker, 'Port', return_value=r.port), \
                 patch.object(checker, 'captures_of', return_value=paths), \
                 patch.object(checker, 'STORE_DB', self.store.path), \
                 patch.object(checker, 'PROVEN', table), contextlib.redirect_stdout(io.StringIO()):
                checker.check(args, [ADDR])
            saved=json.loads(report.read_text())
            result = saved['results']
            decisions.append(saved['call_input_decisions'])
            for row in result:
                row.pop('seconds')
            reports.append(result)
            tables.append(table.read_bytes())
        for i in (0,2):
            self.assertEqual(reports[i], reports[i+1])
            self.assertEqual(tables[i], tables[i+1])
            self.assertEqual(decisions[i],decisions[i+1])
            self.assertTrue(decisions[i])
        self.assertEqual(reports[0][0]['verdict'], 'DIFFERENT')
        self.assertEqual(reports[0][0]['difference']['field'], 'edi')
        self.assertEqual(reports[0][0]['difference']['input']['id'], 1)

    def test_instruction_deadline_is_deterministic(self):
        r = self.replay(b'\xeb\xfe')
        first, _ = r.run(port=True, instruction_budget=1000)
        r.m.restore()
        second, _ = r.run(port=True, instruction_budget=1000)
        self.assertEqual(first['kind'], 'instruction_limit')
        self.assertEqual({k:v for k,v in first.items() if k!='seconds'},
                         {k:v for k,v in second.items() if k!='seconds'})

    def test_port_ceiling_can_allow_equivalent_compiler_overhead(self):
        import gzip,json,contextlib,io
        from argparse import Namespace
        from . import check as checker
        r=self.replay(b'\x90'*1000+CODE)
        out,pages=r.run()
        rec=copy.deepcopy(r.rec)
        rec.update(scenario='synthetic',validation=dict(valid=True),
                   exit=dict(regs={**out['regs'],'eflags':out['eflags']},fpu=rec['entry']['fpu']),
                   exit_pages={str(p):self.store.put_many([v])[0] for p,v in pages.items()})
        folder=OUT/'tests/port-ceiling';folder.mkdir(parents=True,exist_ok=True)
        path=folder/'capture.json.gz'
        with gzip.open(path,'wt') as f:json.dump(rec,f)
        dll=folder/'fixture.dll';dll.write_bytes(b'test loader fixture')
        for ceiling,expected in ((None,'DIFFERENT'),(5000,'EXACT')):
            report=folder/f'{ceiling}.json'
            args=Namespace(dll=dll,variants=2,seed=0xfc4,jobs=1,variant_batch=2,
                           variant_instructions=128,port_variant_instructions=ceiling,
                           keep_going=False,no_proven=True,report=report)
            with patch.object(checker,'Port',return_value=r.port), \
                 patch.object(checker,'captures_of',return_value=[path]), \
                 patch.object(checker,'STORE_DB',self.store.path),contextlib.redirect_stdout(io.StringIO()):
                checker.check(args,[ADDR])
            result=json.loads(report.read_text())['results'][0]
            self.assertEqual(result['verdict'],expected)
            if ceiling:self.assertEqual(result['variants'],2)

    def test_concurrent_owner_proof_rows_are_not_lost(self):
        import multiprocessing,csv
        root=OUT/'tests/concurrent-proven';root.mkdir(parents=True,exist_ok=True)
        table=root/'proven.tsv';table.unlink(missing_ok=True)
        ctx=multiprocessing.get_context('spawn');barrier=ctx.Barrier(2)
        children=[ctx.Process(target=concurrent_proven_fixture,args=(str(table),base,barrier))
                  for base in (0x600000,0x700000)]
        try:
            for child in children:child.start()
            for child in children:
                child.join(60)
                self.assertEqual(child.exitcode,0)
            with table.open(newline='') as f:rows=list(csv.DictReader(f,delimiter='\t'))
            self.assertEqual({r['address'] for r in rows},
                             {f'{base+i:08x}' for base in (0x600000,0x700000) for i in range(20)})
        finally:
            for child in children:
                if child.is_alive():child.terminate();child.join()


if __name__ == '__main__':
    unittest.main()
