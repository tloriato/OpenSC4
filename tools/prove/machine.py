"""One emulated 32-bit x86 machine built from a captured entry state.

Pages are mapped on first touch from a provider (the live game process while capturing, the page store while
checking). The set of mapped pages is therefore exactly the set the run touched (read, write or execute). After a run
the machine reports the registers, the x87 stack and every page whose bytes changed, and can be restored to the entry
state for the next run (the original, the port, then each variant, all from the same machine).

Inputs that are not memory are fixed: RDTSC/RDTSCP return a fixed sequence (the clock is an input), the one system
call the game's heap needs inside a function (NtAllocateVirtualMemory committing reserved pages) is serviced with
zeroed pages, and x87 transcendentals run on the host CPU (x87host).
"""
import struct
import time

from .common import need_unicorn, PAGE

uc_mod = need_unicorn()
from unicorn import (Uc, UcError, UC_ARCH_X86, UC_MODE_32, UC_PROT_ALL, UC_HOOK_MEM_UNMAPPED, UC_HOOK_CODE, UC_HOOK_INSN_INVALID, UC_HOOK_BLOCK,  # noqa
                     UC_ERR_READ_UNMAPPED, UC_ERR_WRITE_UNMAPPED, UC_ERR_FETCH_UNMAPPED)
from unicorn.x86_const import *  # noqa: F401,F403
from . import x87host

ZERO = bytes(PAGE)
GPR = (('eax', UC_X86_REG_EAX), ('ecx', UC_X86_REG_ECX), ('edx', UC_X86_REG_EDX), ('ebx', UC_X86_REG_EBX),
       ('esp', UC_X86_REG_ESP), ('ebp', UC_X86_REG_EBP), ('esi', UC_X86_REG_ESI), ('edi', UC_X86_REG_EDI))
XMM = [UC_X86_REG_XMM0 + i for i in range(8)]
TSC_A = (0x0000_1000_0000_0000, 100_000)
TSC_B = (0x0000_2345_6789_0000, 77_777)


class Fault(Exception):
    pass


class Machine:
    def __init__(self, entry, provider, *, x87_sites=(), clock_sites=(), syscall_range=None, gdt=None,
                 dead_stack=None, tsc=TSC_A):
        """entry: dict(regs, fpu, fs_base); provider.get(page) -> 4 KB bytes or None.

        dead_stack: (lo, hi) page range of the thread's stack below the entry ESP; an uncaptured page there is mapped
        as zeros (it holds nothing live at entry).
        """
        self.state = entry
        self.provider = provider
        self.uc = Uc(UC_ARCH_X86, UC_MODE_32)
        self.uc.ctl_set_cpu_model(UC_CPU_X86_HASWELL)
        self.entry = {}          # page -> bytes it held when first mapped (the entry state)
        self.commits = set()     # pages that did not exist at entry and were committed by the run
        self.dead = set()        # dead stack pages mapped as zeros
        self.static = set()      # pages excluded from the effect comparison (GDT, port image)
        self.faults = []
        self.syscalls = []
        self.dead_stack = dead_stack
        self.tsc_cfg = tsc
        self.clock_reads = 0
        uc = self.uc
        uc.hook_add(UC_HOOK_MEM_UNMAPPED, self._unmapped)
        uc.hook_add(UC_HOOK_INSN_INVALID, self._vector_instruction)
        uc.hook_add(UC_HOOK_BLOCK, self._budget)
        self.x87 = x87host.Host(uc, x87_sites)
        for a in sorted(set(clock_sites)):
            uc.hook_add(UC_HOOK_CODE, self._clock, None, a, a)
        if syscall_range:
            uc.hook_add(UC_HOOK_CODE, self._syscall, None, syscall_range[0], syscall_range[1] - 1)
        self.gdt = gdt
        self._setup_gdt(entry['fs_base'])
        self.clean_cpu = self.uc.context_save()
        self.deadline, self.budget_blocks, self.budget_expired = float('inf'), 0, False
        self.bootstrapping = False

    # ---- memory -------------------------------------------------------------------------------------------------
    def map_page(self, p, data=None, *, static=False):
        if data is None:
            data = self.provider.get(p)
            if data is None and self.dead_stack and self.dead_stack[0] <= p < self.dead_stack[1]:
                data = ZERO
                self.dead.add(p)
            if data is None:
                return False
        self.uc.mem_map(p, PAGE, UC_PROT_ALL)
        self.uc.mem_write(p, data)
        self.entry[p] = data
        # Scan every mapped page, including system libraries and the port. Hooks only
        # fire at instruction boundaries, so candidate bytes in data are harmless.
        self.x87.add(self.uc, x87host.scan(data, p))
        clock_candidates = set()
        for op in (b'\x0f\x31', b'\x0f\x01\xf9'):
            i = data.find(op)
            while i >= 0:
                clock_candidates.add(p+i)
                i = data.find(op, i + 1)
        for i in (PAGE-2, PAGE-1):
            if data[i] == 0x0f:
                clock_candidates.add(p+i)
        for a in clock_candidates:
            self.uc.hook_add(UC_HOOK_CODE, self._clock, None, a, a)
        if data[-1] == 0xd9:
            self.x87.add(self.uc, [p+PAGE-1])
        if static:
            self.static.add(p)
        return True

    def _unmapped(self, uc, access, addr, size, value, _):
        lo, hi = addr & ~(PAGE - 1), (addr + max(size, 1) - 1) & ~(PAGE - 1)
        for p in range(lo, hi + PAGE, PAGE):
            if p in self.entry:
                continue
            try:
                ok = self.map_page(p)
            except Exception as e:     # a provider failure must not escape into the emulator
                self.faults.append((access, addr, size, f'provider: {e}'))
                return False
            if not ok:
                self.faults.append((access, addr, size, None))
                return False
        return True

    def ensure(self, addr, size):
        """Map the pages under [addr, addr+size) from the provider; False if one is not available."""
        for p in range(addr & ~(PAGE - 1), ((addr + size - 1) & ~(PAGE - 1)) + PAGE, PAGE):
            if p not in self.entry and not self.map_page(p):
                return False
        return True

    def read(self, addr, size):
        return bytes(self.uc.mem_read(addr, size))

    def write(self, addr, data):
        if not self.ensure(addr, len(data)):
            raise Fault(f'{addr:#x} is not captured')
        self.uc.mem_write(addr, data)

    def map_image(self, base, image):
        """Map a port DLL image (static: never compared, restored after each run)."""
        size = (len(image) + PAGE - 1) & ~(PAGE - 1)
        image = image + bytes(size - len(image))
        for off in range(0, size, PAGE):
            p = base + off
            if p in self.entry:
                raise Fault(f'port image page {p:#x} collides with a captured page')
            self.map_page(p, image[off:off + PAGE], static=True)

    def _runs(self, pages):
        run = []
        for p in sorted(pages):
            if run and p != run[-1] + PAGE:
                yield run
                run = []
            run.append(p)
        if run:
            yield run

    def dirty(self):
        """{page: bytes now} for every mapped page whose bytes differ from the entry state."""
        out = {}
        for run in self._runs(self.entry):
            data = bytes(self.uc.mem_read(run[0], len(run) * PAGE))
            for i, p in enumerate(run):
                chunk = data[i * PAGE:(i + 1) * PAGE]
                if chunk != self.entry[p]:
                    out[p] = chunk
        return out

    def restore(self, dirty=None):
        dirty = self.dirty() if dirty is None else dirty
        for p in dirty:
            if p in self.commits:
                continue
            self.uc.mem_write(p, self.entry[p])
        for p in list(self.commits):
            self.uc.mem_unmap(p, PAGE)
            del self.entry[p]
        self.commits.clear()

    # ---- inputs that are not memory -------------------------------------------------------------------------------
    def _vector_instruction(self, uc, _):
        """Integer AVX2 operations used by the pinned DLL's string routines.

        Unicorn handles the vector loads and register state but lacks these
        integer operations. All bytes are read through the same page provider.
        Any other unsupported instruction remains a replay failure.
        """
        import capstone
        import capstone.x86_const as X
        addr = uc.reg_read(UC_X86_REG_EIP)
        if not self.ensure(addr, 15):
            return False
        md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)
        md.detail = True
        ins = next(md.disasm(self.read(addr, 15), addr, 1), None)
        if ins is None or ins.mnemonic not in ('vpcmpeqb', 'vpmovmskb'):
            return False

        def reg(op):
            return globals()['UC_X86_REG_' + ins.reg_name(op.reg).upper()]

        def value(op):
            if op.type == X.X86_OP_REG:
                return uc.reg_read(reg(op)).to_bytes(op.size, 'little')
            if op.type != X.X86_OP_MEM:
                return None
            a = op.mem.disp
            for rr, scale in ((op.mem.base, 1), (op.mem.index, op.mem.scale)):
                if rr:
                    a += uc.reg_read(globals()['UC_X86_REG_' + ins.reg_name(rr).upper()]) * scale
            a &= 0xffffffff
            return self.read(a, op.size) if self.ensure(a, op.size) else None

        ops = ins.operands
        if ins.mnemonic == 'vpcmpeqb':
            a, b = value(ops[1]), value(ops[2])
            if a is None or b is None:
                return False
            data = bytes(255 if x == y else 0 for x, y in zip(a, b))
            uc.reg_write(reg(ops[0]), int.from_bytes(data, 'little'))
        else:
            data = value(ops[1])
            if data is None:
                return False
            uc.reg_write(reg(ops[0]), sum((v >> 7) << i for i, v in enumerate(data)))
        uc.reg_write(UC_X86_REG_EIP, addr + ins.size)
        return True

    def _clock(self, uc, addr, size, _):
        if not self.ensure(addr, 2):
            return
        b = bytes(uc.mem_read(addr, 2))
        if b == b'\x0f\x01':
            if not self.ensure(addr, 3):
                return
            b = bytes(uc.mem_read(addr, 3))
        if b[:2] == b'\x0f\x31' or b == b'\x0f\x01\xf9':
            self.tsc += self.tsc_cfg[1]
            self.clock_reads += 1
            uc.reg_write(UC_X86_REG_EAX, self.tsc & 0xFFFFFFFF); uc.reg_write(UC_X86_REG_EDX, self.tsc >> 32)
            if b[1] == 0x01:
                uc.reg_write(UC_X86_REG_ECX, 0)
            uc.reg_write(UC_X86_REG_EIP, addr + (2 if b[1] == 0x31 else 3))

    def _syscall(self, uc, addr, size, _):
        # 32-bit ntdll stubs call into wow64cpu for the switch to the kernel; [ESP] returns into the stub
        esp = uc.reg_read(UC_X86_REG_ESP); n = uc.reg_read(UC_X86_REG_EAX)
        try:
            args = struct.unpack('<6I', bytes(uc.mem_read(esp + 8, 24)))
        except UcError:
            self.syscalls.append(('unreadable system call arguments', n)); uc.emu_stop(); return
        if n == 0x1a0008:
            # Synchronous NtWriteFile used by the pinned observer's log. The
            # file stays external; only its successful completion record is
            # visible to the function. Live validation verifies this model.
            try:
                args9 = struct.unpack('<9I', self.read(esp + 8, 36))
                if args9[1] or args9[2] or args9[3] or not args9[4]:
                    raise ValueError('asynchronous write')
                self.write(args9[4], struct.pack('<II', 0, args9[6]))
                self.syscalls.append(('NtWriteFile synchronous', args9[6]))
                uc.reg_write(UC_X86_REG_EAX, 0)
                uc.reg_write(UC_X86_REG_EIP, struct.unpack('<I', self.read(esp, 4))[0])
                uc.reg_write(UC_X86_REG_ESP, esp + 4)
                return
            except (UcError, ValueError, Fault):
                self.syscalls.append(('unhandled NtWriteFile arguments', n)); uc.emu_stop(); return
        if n == 0x18 and args[0] == 0xFFFFFFFF and args[4] & 0x1000:
            # NtAllocateVirtualMemory(current process, &base, 0, &size, MEM_COMMIT, protect) inside a reservation:
            # the kernel commits zeroed pages at the given address, so we do the same
            base = struct.unpack('<I', bytes(uc.mem_read(args[1], 4)))[0]
            size = struct.unpack('<I', bytes(uc.mem_read(args[3], 4)))[0]
            if base == 0:
                self.syscalls.append(('NtAllocateVirtualMemory without an address', n)); uc.emu_stop(); return
            lo, hi = base & ~(PAGE - 1), (base + size + PAGE - 1) & ~(PAGE - 1)
            for p in range(lo, hi, PAGE):
                if p not in self.entry and not self.map_page(p):
                    if getattr(self.provider, 'strict', False):
                        self.syscalls.append(('commit outside captured pages', p))
                        uc.emu_stop()
                        return
                    self.uc.mem_map(p, PAGE, UC_PROT_ALL)
                    self.entry[p] = ZERO
                    self.commits.add(p)
            uc.mem_write(args[1], struct.pack('<I', lo)); uc.mem_write(args[3], struct.pack('<I', hi - lo))
            self.syscalls.append(('NtAllocateVirtualMemory commit', lo, hi - lo))
            uc.reg_write(UC_X86_REG_EAX, 0)
            uc.reg_write(UC_X86_REG_EIP, struct.unpack('<I', bytes(uc.mem_read(esp, 4)))[0])
            uc.reg_write(UC_X86_REG_ESP, esp + 4)
            return
        self.syscalls.append(('unhandled system call', n))
        uc.emu_stop()

    def _setup_gdt(self, fs_base):
        # 32-bit unicorn has no FS base register: give FS a GDT descriptor whose base is the thread's TEB
        def desc(base, limit, access, flags):
            return struct.pack('<Q', (limit & 0xFFFF) | ((base & 0xFFFFFF) << 16) | (access << 40)
                               | (((limit >> 16) & 0xF) << 48) | (flags << 52) | ((base >> 24) << 56))
        g = self.gdt
        defaults = dict(cs=16, ss=24, ds=24, es=24, gs=24, fs=8)
        self.segments = {n: self.state['regs'].get(n, v) for n, v in defaults.items()}
        table = bytearray(PAGE)
        for n, selector in self.segments.items():
            if not selector:
                continue
            if selector & 4 or selector >> 3 >= PAGE // 8:
                raise Fault(f'unsupported segment selector {selector:#x}')
            dpl = (selector & 3) << 5
            base, limit, flags = (fs_base, 0xfff, 4) if n == 'fs' else (0, 0xfffff, 12)
            access = (0x9b if n == 'cs' else 0x93) | dpl
            table[(selector >> 3)*8:(selector >> 3)*8+8] = desc(base, limit, access, flags)
        unused = [i for i in range(1, 128) if i not in {s >> 3 for s in self.segments.values()}]
        self.bootstrap_cs, self.bootstrap_ss = unused[0] << 3, unused[1] << 3
        table[self.bootstrap_cs:self.bootstrap_cs+8] = desc(0, 0xfffff, 0x9b, 12)
        table[self.bootstrap_ss:self.bootstrap_ss+8] = desc(0, 0xfffff, 0x93, 12)
        table[0xb00] = 0xcf  # IRETD: load a user-mode CS/SS and their privilege level
        self.map_page(g, bytes(table), static=True)
        self.uc.reg_write(UC_X86_REG_GDTR, (0, g, 0xFFF, 0))

    # ---- running --------------------------------------------------------------------------------------------------
    def _budget(self, uc, addr, size, _):
        self.budget_blocks += 1
        if self.budget_blocks % 128 == 0 and time.perf_counter() >= self.deadline:
            self.budget_expired = True
            uc.emu_stop()

    def load_state(self, eip):
        uc, s = self.uc, self.state
        r = s['regs']
        uc.context_restore(self.clean_cpu)
        self.deadline, self.budget_blocks, self.budget_expired = float('inf'), 0, False
        if self.segments['cs'] & 3:
            # A direct Unicorn SS register write checks the current privilege
            # level. Enter the captured ring through IRETD so CS and SS retain
            # their actual selectors rather than substituting ring-zero ones.
            uc.reg_write(UC_X86_REG_CS, self.bootstrap_cs)
            uc.reg_write(UC_X86_REG_SS, self.bootstrap_ss)
            frame = struct.pack('<5I', self.gdt+0xb01, self.segments['cs'], 2,
                                r['esp'], self.segments['ss'])
            uc.mem_write(self.gdt+0xc00, frame)
            uc.reg_write(UC_X86_REG_ESP, self.gdt+0xc00)
            self.bootstrapping = True
            try:
                uc.emu_start(self.gdt+0xb00, self.gdt+0xb01, count=1)
            finally:
                self.bootstrapping = False
        for name in ('cs', 'ss', 'ds', 'es', 'gs', 'fs'):
            uc.reg_write(globals()['UC_X86_REG_' + name.upper()], self.segments[name])
        for name, reg in GPR:
            uc.reg_write(reg, r[name])
        uc.reg_write(UC_X86_REG_EFLAGS, (r['eflags'] & ~0x100) | 0x2)
        uc.reg_write(UC_X86_REG_EIP, eip)
        f = s['fpu']
        uc.reg_write(UC_X86_REG_FPCW, f['cw'])
        top = (f['sw'] >> 11) & 7
        st = bytes.fromhex(f['st'])
        for i in range(8):
            mant, exp = struct.unpack_from('<QH', st, 10 * i)
            uc.reg_write(UC_X86_REG_FP0 + ((top + i) & 7), (mant, exp))
        uc.reg_write(UC_X86_REG_FPSW, f['sw']); uc.reg_write(UC_X86_REG_FPTAG, f['tw'])
        if f.get('mxcsr') is not None:
            uc.reg_write(UC_X86_REG_MXCSR, f['mxcsr'])
        if f.get('xmm'):
            x = bytes.fromhex(f['xmm'])
            for i in range(8):
                uc.reg_write(XMM[i], int.from_bytes(x[16 * i:16 * i + 16], 'little'))
        self.tsc = self.tsc_cfg[0]
        self.clock_reads = 0
        self.faults = []
        self.syscalls = []

    def run(self, eip, ret, esp0, *, timeout=30.0):
        """Run from eip until EIP == ret with ESP above esp0. Returns a result dict (see outcome())."""
        uc = self.uc
        self.load_state(eip)
        t0 = time.perf_counter()
        err = kind = None
        deadline = t0 + timeout
        self.deadline, self.budget_blocks, self.budget_expired = deadline, 0, False
        start = eip
        while True:
            left = deadline - time.perf_counter()
            if left <= 0:
                err, kind = 'did not return in time', 'timeout'
                break
            try:
                # A block deadline plus an instruction quantum avoids starting
                # an OS timer thread for each tiny variant replay.
                uc.emu_start(start, ret, count=1_000_000)
            except UcError as e:
                kind = 'fault' if self.faults else 'exception'
                eip_now = uc.reg_read(UC_X86_REG_EIP)
                if self.faults:
                    acc, a, n, why = self.faults[-1]
                    err = f'touched uncaptured memory at {a:#010x} (from EIP {eip_now:#010x})' + (f'; {why}' if why else '')
                else:
                    err = f'{e} at EIP {eip_now:#010x}'
                break
            eip_now = uc.reg_read(UC_X86_REG_EIP)
            if self.budget_expired:
                err, kind = 'did not return in time', 'timeout'
                break
            if self.syscalls and self.syscalls[-1][0] not in ('NtAllocateVirtualMemory commit', 'NtWriteFile synchronous'):
                err, kind = f'{self.syscalls[-1][0]} {self.syscalls[-1][1]:#x}', 'syscall'
                break
            if eip_now != ret:
                # A guard/system callback can stop before the quantum ends.
                # Its caller adds the precise guard failure to the outcome.
                if getattr(self, 'guard_stop', False):
                    err, kind = f'stopped at {eip_now:#010x} before returning', 'guard'
                    break
                start = eip_now
                continue
            if uc.reg_read(UC_X86_REG_ESP) > esp0:
                break
            # a deeper activation returned to the same address (recursion): step over it and go on
            try:
                uc.emu_start(ret, 0xFFFFFFFF, count=1)
            except UcError as e:
                err, kind = f'{e} at EIP {ret:#010x}', 'exception'
                break
            start = uc.reg_read(UC_X86_REG_EIP)
        return self.outcome(err, kind, time.perf_counter() - t0)

    def outcome(self, err=None, kind=None, seconds=0.0):
        uc = self.uc
        regs = {n: uc.reg_read(r) for n, r in GPR}
        sw = uc.reg_read(UC_X86_REG_FPSW); top = (sw >> 11) & 7
        tag = uc.reg_read(UC_X86_REG_FPTAG)
        st = []
        for i in range(8):
            phys = (top + i) & 7
            if (tag >> (2 * phys)) & 3 == 3:
                st.append(None)
            else:
                st.append(struct.pack('<QH', *uc.reg_read(UC_X86_REG_FP0 + phys)).hex())
        return dict(err=err, kind=kind, seconds=seconds, regs=regs, top=top, cw=uc.reg_read(UC_X86_REG_FPCW), st=st,
                    tag=tag, mxcsr=uc.reg_read(UC_X86_REG_MXCSR),
                    eflags=uc.reg_read(UC_X86_REG_EFLAGS), clock_reads=self.clock_reads,
                    syscalls=list(self.syscalls))


def st_of_ctx(fpu):
    """x87 stack of a captured context in stack order (None = empty), like Machine.outcome()['st']."""
    top = (fpu['sw'] >> 11) & 7
    st = bytes.fromhex(fpu['st'])
    out = []
    for i in range(8):
        phys = (top + i) & 7
        out.append(None if (fpu['tw'] >> (2 * phys)) & 3 == 3 else st[10 * i:10 * i + 10].hex())
    return out
