"""Decide which call inputs a callee observes, recording the evidence once."""
from functools import lru_cache

import capstone
import capstone.x86_const as X

from .common import abi, abi_args, exe, function_code


class CallInputs:
    def __init__(self):
        self.md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)
        self.md.detail = True
        self.decisions = {}
        self.observed = None
        self.input_masks = {}
        self.preserved_masks = {}

    @lru_cache(None)
    def instruction(self, addr):
        data = exe().read(addr, 15)
        return next(self.md.disasm(data, addr, 1), None) if data else None

    @lru_cache(None)
    def scan(self, target, depth=0):
        names = ('ecx', 'edx')
        masks = ({'ecx':0xffffffff, 'cx':0xffff, 'cl':0xff, 'ch':0xff00},
                 {'edx':0xffffffff, 'dx':0xffff, 'dl':0xff, 'dh':0xff00})
        work, seen, reads, returns = [(target, (0xffffffff,0xffffffff))], set(), set(), set()
        observed = dict(ecx=0, edx=0)
        preserved = dict(ecx=0, edx=0)
        unknown = False
        while work:
            addr, live = work.pop()
            if (addr, live) in seen:
                continue
            seen.add((addr, live))
            if len(seen) > 512:
                unknown = True
                for reg in names:
                    preserved[reg] = 0xffffffff
                break
            ins = self.instruction(addr)
            if ins is None:
                unknown = True
                for i,reg in enumerate(names):
                    preserved[reg] |= live[i]
                continue
            rr, ww = ins.regs_access()
            read = {ins.reg_name(r) for r in rr}
            write = {ins.reg_name(r) for r in ww}
            ops = ins.operands
            if ins.mnemonic == 'push' and self.dead_allocation_push(ins):
                read -= {'ecx', 'edx'}
            if ins.mnemonic in ('xor', 'sub') and len(ops) == 2 and ops[0].type == ops[1].type == X.X86_OP_REG and ops[0].reg == ops[1].reg:
                read.clear()
            remaining = []
            for i,reg in enumerate(names):
                read_mask = write_mask = 0
                for alias,mask in masks[i].items():
                    if alias in read:
                        read_mask |= mask
                    if alias in write:
                        write_mask |= mask
                observed[reg] |= live[i] & read_mask
                remaining.append(live[i] & ~write_mask)
            reads.update(reg for reg in names if observed[reg])
            live = tuple(remaining)
            nxt = ins.address + ins.size
            direct = ops[0].imm & 0xffffffff if ops and ops[0].type == X.X86_OP_IMM else None
            m = ins.mnemonic
            if m.startswith('ret'):
                returns.add(direct or 0)
                for i,reg in enumerate(names):
                    preserved[reg] |= live[i]
                continue
            if m == 'call':
                if any(live) and direct is not None and depth < 3:
                    nested, _, uncertain = self.scan(direct, depth+1)
                    nested_masks = self.input_masks.get((direct,depth+1), {r:0xffffffff for r in nested})
                    for i,reg in enumerate(names):
                        observed[reg] |= live[i] & nested_masks.get(reg,0)
                    reads.update(reg for reg in names if observed[reg])
                    unknown |= uncertain
                    child = self.preserved_masks.get((direct,depth+1), {r:0xffffffff for r in names})
                    live = tuple(live[i] & child[reg] for i,reg in enumerate(names))
                elif direct is None and self.stack_only_import(ins):
                    # These Win32 imports take only stack arguments. An
                    # absolute IAT call is not a virtual method taking ECX.
                    pass
                elif any(live) and direct is None:
                    # A virtual call's receiver is observable. Its setup normally
                    # reads the receiver already, but preserve it here too.
                    observed['ecx'] |= live[0]
                    reads.update(reg for reg in names if observed[reg])
                    unknown = True
                elif any(live):
                    unknown = True
            if m == 'jmp':
                if direct is not None:
                    work.append((direct, live))
                else:
                    unknown = True
                    for i,reg in enumerate(names):
                        preserved[reg] |= live[i]
                continue
            if m.startswith('j') or m.startswith('loop'):
                if direct is not None:
                    work.append((direct, live))
                else:
                    unknown = True
            if m in ('int', 'int3', 'ud2', 'hlt'):
                unknown = True
                continue
            work.append((nxt, live))
        self.input_masks[target,depth] = observed
        self.preserved_masks[target,depth] = preserved
        return reads, returns, unknown

    def stack_only_import(self, ins):
        if not ins.operands or ins.operands[0].type != X.X86_OP_MEM:
            return False
        mem = ins.operands[0].mem
        if mem.base or mem.index:
            return False
        imported = exe().imports.get(mem.disp & 0xffffffff)
        return imported in {('kernel32.dll', name) for name in
                            ('GetLastError', 'SetLastError', 'TlsGetValue', 'TlsSetValue',
                             'QueryPerformanceCounter', 'GetCurrentThreadId', 'GetProcessHeap',
                             'HeapAlloc', 'HeapFree', 'HeapReAlloc', 'HeapSize', 'HeapValidate',
                             'EnterCriticalSection', 'LeaveCriticalSection', 'TryEnterCriticalSection',
                             'InitializeCriticalSection', 'DeleteCriticalSection')}

    def dead_allocation_push(self, first):
        """A PUSH used to reserve a local is not a use of its incoming value."""
        if not first.operands or first.operands[0].type != X.X86_OP_REG or first.reg_name(first.operands[0].reg) not in ('ecx', 'edx'):
            return False
        if first.address in (0x009133dd, 0x009133de) and bytes(first.bytes) == b'\x51':
            # operator new reserves two words. LEA ECX,[EBP-8] then calls
            # 0048c9f0, which initializes both words before reading either.
            # The incoming ECX value never becomes an allocator argument.
            return True
        sp, addr = -4, first.address+first.size
        for _ in range(16):
            ins = self.instruction(addr)
            if ins is None or ins.mnemonic.startswith(('call', 'j', 'ret', 'pop')):
                return False
            for op in ins.operands:
                if op.type != X.X86_OP_MEM or ins.reg_name(op.mem.base) != 'esp' or op.mem.index or ins.mnemonic == 'lea':
                    continue
                lo = sp+op.mem.disp
                if lo < 0 and lo+op.size > -4:
                    if op.access & capstone.CS_AC_READ:
                        return False
                    if op.access & capstone.CS_AC_WRITE and lo <= -4 and lo+op.size >= 0:
                        return True
            if ins.mnemonic == 'push':
                sp -= 4
            elif ins.mnemonic in ('add', 'sub') and len(ins.operands) == 2 and ins.operands[0].type == X.X86_OP_REG and ins.reg_name(ins.operands[0].reg) == 'esp' and ins.operands[1].type == X.X86_OP_IMM:
                sp += ins.operands[1].imm * (1 if ins.mnemonic == 'add' else -1)
            addr += ins.size
        return False

    def callee(self, target):
        key = f'{target:08x}'
        if key in self.decisions:
            return self.decisions[key]
        row = abi().get(target, {})
        known = abi_args(target)
        regs, pops, uncertain = self.scan(target)
        regs = set(regs)
        convention = row.get('convention', '').lower()
        declared = (row.get('reg_args') or '').lower().split(',')
        if 'thiscall' in convention or 'fastcall' in convention or row.get('this', '').lower() == 'ecx' or 'ecx' in declared:
            regs.add('ecx')
        dead_declared = []
        if 'fastcall' in convention and (uncertain or 'edx' in regs):
            regs.add('edx')
        elif 'fastcall' in convention:
            dead_declared.append('edx')
        n = known[1] if known else None
        source = 'ABI table' if n is not None else 'unknown'
        if target == 0x0091286f:
            # Date.Month makes a 12-byte local frame and passes three local
            # addresses to GetYMD. It reads only the month at EBP-4 and returns
            # without popping arguments. Its caller's later cleanup is batched.
            n, source = 0, 'verified Date.Month: three local outputs, no caller stack inputs'
        if target == 0x009f7604:
            # __getptd saves EBX/ESI, calls GetLastError and TlsGetValue,
            # restores them and returns. No caller stack slot is read. Cleanup
            # after callers belongs to their earlier calls, not to this helper.
            n, source = 0, 'verified thread-context helper: no caller stack reads'
            # Its no-argument CRT interface also takes no incoming ECX/EDX.
            # A bounded walk through its slow allocation path can encounter
            # an unresolved indirect call; that is not an implicit receiver.
            regs.clear()
        if n is None and len(pops) == 1 and next(iter(pops)) > 0:
            pop = next(iter(pops))
            if pop % 4 == 0:
                n, source = pop//4, 'callee ret immediate'
        result = dict(target=key, registers=sorted(regs), stack_dwords=n, stack_source=source,
                      abi_confidence=row.get('confidence'), entry_read_registers=sorted(self.scan(target)[0]),
                      entry_read_masks={n:f'{v:08x}' for n,v in self.input_masks.get((target,0),{}).items()},
                      abi_convention=row.get('convention'), abi_reg_args=row.get('reg_args'),
                      abi_this=row.get('this'), abi_stack_arg_dwords=row.get('stack_arg_dwords'),
                      input_override=('verified no-argument thread-context helper; no incoming ECX/EDX or stack parameters'
                                      if target==0x009f7604 else
                                      'verified Date.Month local outputs; no caller stack parameters' if target==0x0091286f else None),
                      register_source='ECX declaration/thiscall/fastcall or entry reads; EDX entry reads or unproven fastcall; complete entry scan can prove incoming EDX dead; partial overwrites tracked by bit; overwritten allocation pushes excluded',
                      proven_dead_declared_registers=dead_declared,
                      entry_scan_incomplete=uncertain, return_pop_bytes=sorted(pops), caller_sites={})
        self.decisions[key] = result
        return result

    def cleanup(self, site):
        ins = self.instruction(site)
        if ins is None or ins.mnemonic != 'call':
            return None
        addr = site+ins.size
        for _ in range(12):
            ins = self.instruction(addr)
            if ins is None:
                break
            ops = ins.operands
            if ins.mnemonic == 'add' and len(ops) == 2 and ops[0].type == X.X86_OP_REG and ins.reg_name(ops[0].reg) == 'esp' and ops[1].type == X.X86_OP_IMM:
                n = ops[1].imm//4 if ops[1].imm >= 0 and ops[1].imm % 4 == 0 else None
                return self.unbatched_cleanup(site, n)
            if ins.mnemonic == 'lea' and len(ops) == 2 and ops[0].type == X.X86_OP_REG and ins.reg_name(ops[0].reg) == 'esp' and ops[1].type == X.X86_OP_MEM:
                mem = ops[1].mem
                if ins.reg_name(mem.base) == 'esp' and not mem.index and mem.disp >= 0 and mem.disp % 4 == 0:
                    return self.unbatched_cleanup(site, mem.disp//4)
            if ins.mnemonic == 'pop' and ops[0].type == X.X86_OP_REG and ins.reg_name(ops[0].reg) in ('eax', 'ecx', 'edx'):
                return 1
            if ins.mnemonic.startswith(('j', 'ret', 'call', 'push', 'pop')):
                break
            _, writes = ins.regs_access()
            if X.X86_REG_ESP in writes:
                break
            addr += ins.size
        return None

    def unbatched_cleanup(self, site, count):
        """Do not attribute several calls' accumulated cleanup to the last call."""
        if count is None:
            return None
        from .channel import function_at
        owner = function_at(site)
        if owner is None:
            return count
        capacity = 0
        for addr, _, _ in function_code(owner):
            if addr >= site:
                break
            ins = self.instruction(addr)
            if not ins:
                continue
            ops = ins.operands
            if ins.mnemonic == 'call':
                capacity = 0
            elif ins.mnemonic == 'push':
                capacity += 1
            elif ins.mnemonic == 'sub' and len(ops) == 2 and ops[0].type == X.X86_OP_REG and ins.reg_name(ops[0].reg) == 'esp' and ops[1].type == X.X86_OP_IMM:
                capacity += max(0, ops[1].imm//4)
        return count if count <= capacity else None

    def at_call(self, target, original_site=None):
        decision = self.callee(target)
        n = decision['stack_dwords']
        if n is None and original_site is not None:
            site = f'{original_site:08x}'
            if site not in decision['caller_sites']:
                count = self.cleanup(original_site)
                decision['caller_sites'][site] = dict(stack_dwords=count,
                    source='original caller cleanup' if count is not None else 'unknown')
            n = decision['caller_sites'][site]['stack_dwords']
        if self.observed is not None:
            key = decision['target']
            if key not in self.observed:
                self.observed[key] = {**decision, 'caller_sites': {}}
            if original_site is not None:
                site = f'{original_site:08x}'
                if site in decision['caller_sites']:
                    self.observed[key]['caller_sites'][site] = dict(decision['caller_sites'][site])
        return decision['registers'], n

    def entry_arity(self, rec):
        known = abi_args(rec['function'])
        if known:
            return known[1]
        if rec.get('exit'):
            pop = rec['exit']['regs']['esp'] - rec['entry']['regs']['esp'] - 4
            if pop > 0 and pop % 4 == 0:
                return pop//4
        ret = rec['ret']
        for size in range(2, 8):
            ins = self.instruction(ret-size)
            if ins and ins.mnemonic == 'call' and ins.address+ins.size == ret:
                return self.at_call(rec['function'], ret-size)[1]
        return self.at_call(rec['function'])[1]


CALL_INPUTS = CallInputs()
