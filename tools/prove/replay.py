"""Replay instrumentation: external call order, input reads, and native-port guards."""
import struct

from .common import PAGE, EXE_LO, EXE_HI, functions, helpers, abi_args, Ranges, ranges_of, need_unicorn
from .machine import Machine

need_unicorn()
from unicorn import UC_HOOK_BLOCK, UC_HOOK_CODE, UC_HOOK_MEM_READ, UC_HOOK_MEM_WRITE
from unicorn.x86_const import UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESP
import capstone


class Provider:
    strict = True

    def __init__(self, rec, store):
        self.pages = {int(p): h for p, h in rec['pages'].items()}
        self.store = store

    def get(self, p):
        h = self.pages.get(p)
        return None if h is None else self.store.get(h)


class Replay:
    def __init__(self, rec, store, port=None, enabled=()):
        self.rec, self.port = rec, port
        self.m = Machine(rec['entry'], Provider(rec, store), gdt=rec['gdt'], syscall_range=rec['syscall_range'])
        if port:
            self.m.map_image(port.base, port.image)
        self.own = Ranges(ranges_of(rec['function']))
        self.enabled = {a: port.exports[a] for a in enabled if port and a in port.exports}
        self.md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)
        self.blocks = {}
        self.m.uc.hook_add(UC_HOOK_BLOCK, self.block)
        for lo, hi in self.own:
            self.m.uc.hook_add(UC_HOOK_CODE, self.body, None, lo, hi-1)
        self.m.uc.hook_add(UC_HOOK_MEM_WRITE, self.write)
        self.m.uc.hook_add(UC_HOOK_MEM_READ, self.read)
        self.entry_addresses = set(functions())
        self.helper_addresses = helpers()
        self.collect = False
        self.port_run = False
        self.reset()

    def reset(self):
        self.trace, self.inputs, self.written = [], {}, set()
        self.previous = None
        self.guard = None
        self.m.guard_stop = False

    def stop(self, uc, reason):
        self.guard = reason
        self.m.guard_stop = True
        uc.emu_stop()

    def body(self, uc, addr, size, _):
        if self.port_run:
            self.stop(uc, f'executed original body at {addr:08x}')

    def block(self, uc, addr, size, _):
        if self.m.bootstrapping:
            return
        if not self.port_run and self.port and self.port.base <= addr < self.port.base+len(self.port.image):
            self.stop(uc, f'original executed outside captured address space at {addr:08x}')
            return
        if self.previous and EXE_LO <= addr < EXE_HI:
            kind, src = self.previous
            if kind == 'call' or (kind == 'jmp' and addr in self.entry_addresses):
                if addr not in self.helper_addresses:
                    esp = uc.reg_read(UC_X86_REG_ESP)
                    nargs = abi_args(addr)
                    n = max(4, nargs[1] if nargs else 4)
                    offset = 4  # stack holds a return address at call/tail-call entry
                    if not self.m.ensure(esp + offset, n*4):
                        self.stop(uc, f'uncaptured callee arguments at {esp+offset:08x}')
                        return
                    words = struct.unpack('<'+'I'*n, bytes(uc.mem_read(esp+offset, n*4)))
                    self.trace.append([addr, uc.reg_read(UC_X86_REG_ECX), uc.reg_read(UC_X86_REG_EDX), list(words)])
        ins = self.blocks.get((addr, size))
        if ins is None:
            data = bytes(uc.mem_read(addr, size))
            decoded = list(self.md.disasm_lite(data, addr))
            ins = (decoded[-1][2], decoded[-1][0]) if decoded else ('', addr)
            self.blocks[addr, size] = ins
        self.previous = ins

    def write(self, uc, access, addr, size, value, _):
        if self.port_run and addr < self.port.base + len(self.port.image) and addr + size > self.port.base:
            self.stop(uc, f'write into port DLL at {addr:08x} ({size} bytes)')
        if self.collect:
            self.written.update(range(addr, addr+size))

    def read(self, uc, access, addr, size, value, _):
        if not self.port_run and self.port and addr < self.port.base+len(self.port.image) and addr+size > self.port.base:
            self.stop(uc, f'original read outside captured address space at {addr:08x}')
            return
        if not self.collect or size not in (1, 2, 4, 8) or len(self.inputs) >= 4096 or addr & -PAGE in self.m.static:
            return
        # Stack arguments are swept separately. Include scalar image data as
        # well as object fields; pointer values are removed by the generator.
        if self.rec['stack'][0] <= addr < self.rec['stack'][1]:
            return
        if any(a in self.written for a in range(addr, addr+size)):
            return
        self.inputs.setdefault((addr, size), bytes(uc.mem_read(addr, size)))

    def run(self, *, port=False, collect=False, timeout=30):
        self.reset()
        self.port_run, self.collect = port, collect
        m, rec = self.m, self.rec
        patches = {}
        if port:
            for a, dst in self.enabled.items():
                if m.ensure(a, 5):
                    patches[a] = m.read(a, 5)
                    m.write(a, b'\xe9'+struct.pack('<I', (dst-a-5) & 0xffffffff))
        try:
            out = m.run(self.port.exports[rec['function']] if port else rec['function'],
                        rec['ret'], m.state['regs']['esp'], timeout=timeout)
            for a, data in patches.items():
                expected = b'\xe9'+struct.pack('<I', (self.enabled[a]-a-5) & 0xffffffff)
                if m.read(a, 5) != expected:
                    self.guard = f'modified enabled entry jump at {a:08x}'
            if self.guard:
                out['err'], out['kind'] = self.guard, 'guard'
            out['trace'] = self.trace
        finally:
            for a, data in patches.items():
                m.write(a, data)
        dirty = {p: b for p, b in m.dirty().items() if p not in m.static}
        return out, dirty
