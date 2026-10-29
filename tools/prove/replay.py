"""Replay instrumentation: external call order, input reads, and native-port guards."""
import struct

from .common import PAGE, EXE_LO, EXE_HI, functions, helpers, abi_args, Ranges, ranges_of, need_unicorn
from .machine import Machine
from .calls import CALL_INPUTS
from .observer import ObserverWrites

need_unicorn()
from unicorn import UC_HOOK_BLOCK, UC_HOOK_CODE, UC_HOOK_MEM_READ, UC_HOOK_MEM_WRITE, UC_PROT_NONE, UC_PROT_ALL
from unicorn.x86_const import UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESP, UC_X86_REG_EIP
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
        # Entries outside this capture cannot execute at all. Filter once,
        # rather than probing every export again on every generated input.
        pages = self.m.provider.pages
        self.enabled = {a: port.exports[a] for a in enabled if port and a in port.exports
                        and a & -PAGE in pages and (a+4) & -PAGE in pages}
        self.md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)
        self.blocks = {}
        # The instruction quantum bounds replay without a second Python hook
        # on every basic block. Capturing keeps the live debugger's deadline hook.
        self.m.uc.hook_del(self.m.budget_hook)
        self.m.quantum = 50_000
        self.m.uc.hook_add(UC_HOOK_BLOCK, self.block)
        self.in_run = False
        self.m.write_observer = self.kernel_write
        self.entry_addresses = set(functions())
        self.helper_addresses = helpers()
        self.immutable = Ranges((r[0], r[0]+r[1]) for r in rec['regions']
                                if len(r) > 3 and r[3] & 0xff in (2, 0x10, 0x20))
        self.collect = False
        self.port_run = False
        self.original_sites = []
        self.observer = ObserverWrites(rec)
        self.observer_exclusions = {}
        self.observer_excluded_byte_events = 0
        self.reset()

    def reset(self):
        self.observer.reset()
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
            kind, src, length, operand = self.previous
            tail = kind == 'jmp' and addr in self.entry_addresses
            if tail:
                if operand.startswith('0x') and int(operand, 16) != addr:
                    tail = False  # the quantum stopped before the jump executed
                elif any(lo <= src < hi for lo,hi in ranges_of(addr)):
                    tail = False  # a branch within the callee is not a call
            if kind == 'ret' and addr in self.entry_addresses:
                # A quantum can resume a block whose RET has not executed.
                # A PUSH/RET forwarding jump has actually popped this target.
                popped = uc.reg_read(UC_X86_REG_ESP)-4
                tail = self.m.ensure(popped, 4) and struct.unpack('<I', self.m.read(popped, 4))[0] == addr
            candidate = kind == 'call' or tail
            esp = uc.reg_read(UC_X86_REG_ESP) if candidate else 0
            # A quantum can end partway through a block. A block ending in CALL
            # is only a completed call when its return address is on the stack.
            called = kind == 'call' and self.m.ensure(esp, 4) and struct.unpack('<I', self.m.read(esp, 4))[0] == src+length
            if called or tail:
                if addr not in self.helper_addresses:
                    esp = uc.reg_read(UC_X86_REG_ESP)
                    index = len(self.trace)
                    site = src if not self.port_run else (
                        self.original_sites[index][1] if index < len(self.original_sites) and self.original_sites[index][0] == addr else None)
                    regs, n = CALL_INPUTS.at_call(addr, site)
                    offset = 4  # stack holds a return address at call/tail-call entry
                    if n and not self.m.ensure(esp + offset, n*4):
                        self.stop(uc, f'uncaptured callee arguments at {esp+offset:08x}')
                        return
                    words = list(struct.unpack('<'+'I'*n, bytes(uc.mem_read(esp+offset, n*4)))) if n else ([] if n == 0 else None)
                    values = {name: uc.reg_read(UC_X86_REG_ECX if name == 'ecx' else UC_X86_REG_EDX) for name in regs}
                    self.trace.append(dict(target=f'{addr:08x}', registers=values, arguments=words,
                                           frame=[esp+4, self.m.state['regs']['esp']], site=f'{src:08x}'))
                    if not self.port_run:
                        self.current_sites.append((addr, src))
        ins = self.blocks.get((addr, size))
        if ins is None:
            data = bytes(uc.mem_read(addr, size))
            decoded = list(self.md.disasm_lite(data, addr))
            ins = (decoded[-1][2], decoded[-1][0], decoded[-1][1], decoded[-1][3]) if decoded else ('', addr, 0, '')
            self.blocks[addr, size] = ins
        self.previous = ins

    def write(self, uc, access, addr, size, value, _):
        if self.port and addr < self.port.base + len(self.port.image) and addr + size > self.port.base:
            self.m.image_writes.update(p for p in range(addr & -PAGE, ((addr+size-1) & -PAGE)+PAGE, PAGE)
                                       if p in self.m.image_pages)
            self.stop(uc, f'write into port DLL at {addr:08x} ({size} bytes)' if self.port_run else
                      f'original wrote outside captured address space at {addr:08x}')
        if self.collect:
            self.written.update(range(addr, addr+size))

    def kernel_write(self, addr, data):
        if self.in_run:
            self.write(self.m.uc, 0, addr, len(data), 0, None)
            self.observer_write(self.m.uc, 0, addr, len(data), 0, None)

    def observer_write(self, uc, access, addr, size, value, _):
        self.observer.write(addr, size, uc.reg_read(UC_X86_REG_EIP))

    def read(self, uc, access, addr, size, value, _):
        if not self.port_run and self.port and addr < self.port.base+len(self.port.image) and addr+size > self.port.base:
            self.stop(uc, f'original read outside captured address space at {addr:08x}')
            return
        if not self.collect or size not in (1, 2, 4, 8) or len(self.inputs) >= 4096 or addr & -PAGE in self.m.static:
            return
        if addr in self.immutable:
            return  # immutable image constants are not scalar object inputs
        # Stack arguments are swept separately. Include scalar image data as
        # well as object fields; pointer values are removed by the generator.
        if self.rec['stack'][0] <= addr < self.rec['stack'][1]:
            return
        if any(a in self.written for a in range(addr, addr+size)):
            return
        self.inputs.setdefault((addr, size), bytes(uc.mem_read(addr, size)))

    def run(self, *, port=False, collect=False, timeout=30, instruction_budget=None):
        self.reset()
        self.current_sites = []
        self.port_run, self.collect = port, collect
        m, rec = self.m, self.rec
        patches = {}
        body_hooks = []
        memory_hooks = []
        for lo, hi in self.observer.writable:
            # Unicorn filters stores by their starting address. Include the
            # preceding boundary page for wide stores crossing into this range;
            # ObserverWrites clips the actual bytes strictly to writable data.
            memory_hooks.append(m.uc.hook_add(UC_HOOK_MEM_WRITE, self.observer_write, None, max(0,lo-PAGE), hi-1))
        if self.port:
            m.uc.mem_protect(self.port.base, len(self.port.image), UC_PROT_ALL if port else UC_PROT_NONE)
        if collect:
            memory_hooks.append(m.uc.hook_add(UC_HOOK_MEM_READ, self.read))
            memory_hooks.append(m.uc.hook_add(UC_HOOK_MEM_WRITE, self.write))
        elif port:
            memory_hooks.append(m.uc.hook_add(UC_HOOK_MEM_WRITE, self.write, None,
                                             max(0, self.port.base-PAGE), self.port.base+len(self.port.image)-1))
        if port:
            for lo, hi in self.own:
                body_hooks.append(m.uc.hook_add(UC_HOOK_CODE, self.body, None, lo, hi-1))
            for a, dst in self.enabled.items():
                if m.ensure(a, 5):
                    patches[a] = m.read(a, 5)
                    m.write(a, b'\xe9'+struct.pack('<I', (dst-a-5) & 0xffffffff))
                    m.uc.ctl_remove_cache(a, a+5)
            for lo, hi in self.own:
                m.uc.ctl_remove_cache(lo, hi)
        try:
            self.in_run = True
            out = m.run(self.port.exports[rec['function']] if port else rec['function'],
                        rec['ret'], m.state['regs']['esp'], timeout=timeout,
                        instruction_budget=instruction_budget)
            for a, data in patches.items():
                expected = b'\xe9'+struct.pack('<I', (self.enabled[a]-a-5) & 0xffffffff)
                if m.read(a, 5) != expected:
                    self.guard = f'modified enabled entry jump at {a:08x}'
            if self.guard:
                out['err'], out['kind'] = self.guard, 'guard'
            out['trace'] = self.trace
            out['observer_writes'] = self.observer.outcome()
            if not port:
                self.original_sites = self.current_sites
        finally:
            self.in_run = False
            for handle in memory_hooks:
                m.uc.hook_del(handle)
            for handle in body_hooks:
                m.uc.hook_del(handle)
            if body_hooks:
                for lo, hi in self.own:
                    m.uc.ctl_remove_cache(lo, hi)
            for a, data in patches.items():
                m.write(a, data)
                m.uc.ctl_remove_cache(a, a+5)
        dirty = {p: b for p, b in m.dirty().items() if p not in m.static}
        return out, dirty
