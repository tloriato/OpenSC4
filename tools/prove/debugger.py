"""A small WoW64 debugger for the running 32-bit game: breakpoints at function entries and their return addresses.

At an entry it hands the stopped thread's registers to a callback (the capture replays the original in the emulator,
reading pages from the stopped process). It then lets the real CPU run the function with every other thread suspended
and calls back again when the same thread returns (ESP above the entry ESP), so the real exit can be compared with the
emulated one. Breakpoints are single int3 bytes written into the process's memory only; reads of the process memory go
through an overlay that puts the original bytes back, so no capture ever sees a breakpoint.
"""
import ctypes
import ctypes.wintypes as wt
import struct
import time

k32 = ctypes.WinDLL('kernel32', use_last_error=True)
psapi = ctypes.WinDLL('psapi', use_last_error=True)
k32.OpenProcess.restype = wt.HANDLE
k32.OpenThread.restype = wt.HANDLE
k32.VirtualQueryEx.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_size_t]
k32.VirtualQueryEx.restype = ctypes.c_size_t
k32.ReadProcessMemory.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_size_t,
                                  ctypes.POINTER(ctypes.c_size_t)]
k32.WriteProcessMemory.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_size_t,
                                   ctypes.POINTER(ctypes.c_size_t)]
k32.FlushInstructionCache.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.c_size_t]
k32.CloseHandle.argtypes = [wt.HANDLE]
k32.Wow64GetThreadContext.argtypes = [wt.HANDLE, ctypes.c_void_p]
k32.Wow64SetThreadContext.argtypes = [wt.HANDLE, ctypes.c_void_p]
k32.Wow64GetThreadSelectorEntry.argtypes = [wt.HANDLE, wt.DWORD, ctypes.c_void_p]
k32.SuspendThread.argtypes = [wt.HANDLE]
k32.SuspendThread.restype = wt.DWORD
k32.Wow64SuspendThread.argtypes = [wt.HANDLE]
k32.Wow64SuspendThread.restype = wt.DWORD
k32.ResumeThread.argtypes = [wt.HANDLE]
k32.ResumeThread.restype = wt.DWORD
k32.GetThreadTimes.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p]
k32.WaitForSingleObject.argtypes = [wt.HANDLE, wt.DWORD]
k32.WaitForSingleObject.restype = wt.DWORD
psapi.GetMappedFileNameW.argtypes = [wt.HANDLE, ctypes.c_void_p, wt.LPWSTR, wt.DWORD]

DBG_CONTINUE, DBG_NOT_HANDLED = 0x00010002, 0x80010001
EXCEPTION, CREATE_THREAD, CREATE_PROCESS, EXIT_THREAD, EXIT_PROCESS, LOAD_DLL = 1, 2, 3, 4, 5, 6
BREAKPOINTS = (0x80000003, 0x4000001F)        # STATUS_BREAKPOINT, STATUS_WX86_BREAKPOINT
SINGLE_STEPS = (0x80000004, 0x4000001E)
WOW64_CONTEXT_ALL = 0x0001003F
PAGE = 0x1000
MEM_COMMIT, MEM_FREE, MEM_IMAGE = 0x1000, 0x10000, 0x1000000
READONLY_PROTECT = {0x02, 0x10, 0x20}          # PAGE_READONLY, PAGE_EXECUTE, PAGE_EXECUTE_READ


class DebugEvent(ctypes.Structure):
    _fields_ = [('code', wt.DWORD), ('pid', wt.DWORD), ('tid', wt.DWORD), ('_pad', wt.DWORD),
                ('u', ctypes.c_ubyte * 160)]


class MBI(ctypes.Structure):
    _fields_ = [('BaseAddress', ctypes.c_ulonglong), ('AllocationBase', ctypes.c_ulonglong),
                ('AllocationProtect', wt.DWORD), ('PartitionId', wt.WORD), ('_p', wt.WORD),
                ('RegionSize', ctypes.c_ulonglong), ('State', wt.DWORD), ('Protect', wt.DWORD), ('Type', wt.DWORD),
                ('_p2', wt.DWORD)]


def _u64(ev, off):
    return struct.unpack_from('<Q', bytes(ev.u), off)[0]


def _u32(ev, off):
    return struct.unpack_from('<I', bytes(ev.u), off)[0]


class Process:
    def __init__(self, pid):
        self.pid = pid
        self.hp = k32.OpenProcess(0x1F0FFF, False, pid)
        if not self.hp:
            raise OSError(ctypes.get_last_error(), f'OpenProcess {pid}')
        self.threads = {}
        self.patches = {}       # address -> original byte, for every int3 currently written

    # ---- memory -------------------------------------------------------------------------------------------------
    def query(self, addr):
        mbi = MBI()
        if not k32.VirtualQueryEx(self.hp, addr, ctypes.byref(mbi), ctypes.sizeof(mbi)):
            return None
        return mbi

    def raw_read(self, addr, n):
        buf = ctypes.create_string_buffer(n); got = ctypes.c_size_t()
        if not k32.ReadProcessMemory(self.hp, addr, buf, n, ctypes.byref(got)) or got.value != n:
            return None
        return buf.raw

    def read(self, addr, n):
        """Process memory with the original bytes under our breakpoints."""
        data = self.raw_read(addr, n)
        if data is None:
            return None
        hits = [a for a in self.patches if addr <= a < addr + n]
        if hits:
            b = bytearray(data)
            for a in hits:
                b[a - addr] = self.patches[a][0]
            data = bytes(b)
        return data

    def page(self, p):
        """One 4 KB page as the program sees it, or None when it is not committed and readable."""
        mbi = self.query(p)
        if mbi is None or mbi.State != MEM_COMMIT or mbi.Protect & 0x101 or mbi.Protect == 0:
            return None
        return self.read(p, PAGE)

    def writable(self, p):
        mbi = self.query(p)
        return mbi is not None and mbi.State == MEM_COMMIT and (mbi.Protect & 0xFF) not in READONLY_PROTECT

    def write(self, addr, data):
        got = ctypes.c_size_t()
        ok = k32.WriteProcessMemory(self.hp, addr, data, len(data), ctypes.byref(got))
        k32.FlushInstructionCache(self.hp, addr, len(data))
        return ok

    def arm(self, addr):
        if addr not in self.patches:
            orig = self.raw_read(addr, 1)
            if orig is None:
                return False
            self.patches[addr] = orig
            self.write(addr, b'\xCC')
        return True

    def disarm(self, addr):
        orig = self.patches.pop(addr, None)
        if orig is not None:
            self.write(addr, orig)

    def disarm_all(self):
        for a in list(self.patches):
            self.disarm(a)

    def regions(self):
        """[(base, size, state, protect, type, module)] of the 32-bit address space."""
        out, addr = [], 0
        while addr < 0x100000000:
            mbi = self.query(addr)
            if mbi is None or mbi.RegionSize == 0:
                break
            mod = ''
            if mbi.Type == MEM_IMAGE:
                name = ctypes.create_unicode_buffer(520)
                if psapi.GetMappedFileNameW(self.hp, mbi.BaseAddress, name, 520):
                    mod = name.value
            out.append((mbi.BaseAddress, min(mbi.RegionSize, 0x100000000 - mbi.BaseAddress), mbi.State, mbi.Protect,
                        mbi.Type, mod))
            addr = mbi.BaseAddress + mbi.RegionSize
        return out

    # ---- threads ------------------------------------------------------------------------------------------------
    def thread(self, tid):
        h = self.threads.get(tid)
        if h is None:
            h = k32.OpenThread(0x1FFFFF, False, tid)
            if not h:
                raise OSError(ctypes.get_last_error(), f'OpenThread {tid}')
            self.threads[tid] = h
        return h

    def forget(self, tid):
        h = self.threads.pop(tid, None)
        if h:
            k32.CloseHandle(h)

    def get_ctx(self, tid):
        ctx = ctypes.create_string_buffer(0x2CC)
        struct.pack_into('<I', ctx, 0, WOW64_CONTEXT_ALL)
        if not k32.Wow64GetThreadContext(self.thread(tid), ctx):
            raise OSError(ctypes.get_last_error(), 'Wow64GetThreadContext')
        return ctx

    def set_ctx(self, tid, ctx):
        if not k32.Wow64SetThreadContext(self.thread(tid), ctx):
            raise OSError(ctypes.get_last_error(), 'Wow64SetThreadContext')

    def fs_base(self, tid, fs):
        ldt = ctypes.create_string_buffer(8)
        if not k32.Wow64GetThreadSelectorEntry(self.thread(tid), fs, ldt):
            return None
        lo, bl, bm, f1, f2, bh = struct.unpack('<HHBBBB', ldt.raw)
        return bl | (bm << 16) | (bh << 24)

    def cpu_seconds(self, tid):
        values = [ctypes.c_ulonglong() for _ in range(4)]
        if not k32.GetThreadTimes(self.thread(tid), *(ctypes.byref(v) for v in values)):
            return None
        return (values[2].value + values[3].value) / 10_000_000

    def wait_site(self, tid):
        """Sample the stopped owner for a named ntdll wait stub."""
        import pefile
        from pathlib import Path
        h = self.thread(tid)
        if k32.Wow64SuspendThread(h) == 0xFFFFFFFF:
            return None
        try:
            ctx = self.get_ctx(tid)
            ip, sp = ctx_get(ctx, EIP), ctx_get(ctx, ESP)
            stack = self.read(sp, 256) or b''
            # The current instruction or its immediate thunk return can
            # identify a wait. Deeper stack slots can contain stale returns.
            addresses = [ip] + list(struct.unpack('<'+'I'*(min(len(stack), 8)//4), stack[:8]))
            regions = [r for r in self.regions() if r[5].lower().endswith('\\ntdll.dll')]
            if not regions:
                return None
            base = min(r[0] for r in regions)
            pe = pefile.PE(str(Path(__import__('os').environ['WINDIR'])/'SysWOW64/ntdll.dll'))
            names = {'NtWaitForSingleObject', 'NtWaitForMultipleObjects', 'NtWaitForMultipleObjects32',
                     'NtSignalAndWaitForSingleObject', 'NtWaitForKeyedEvent', 'NtRemoveIoCompletion',
                     'NtWaitForAlertByThreadId', 'NtDelayExecution'}
            for symbol in pe.DIRECTORY_ENTRY_EXPORT.symbols:
                name = (symbol.name or b'').decode()
                if name in names:
                    lo = base + symbol.address
                    if any(lo <= a < lo+32 for a in addresses):
                        return dict(service=name, eip=f'{ip:08x}', esp=f'{sp:08x}',
                                    stub=f'{lo:08x}', stack=stack.hex())
            return None
        finally:
            k32.ResumeThread(h)

    def close(self):
        for tid in list(self.threads):
            self.forget(tid)
        k32.CloseHandle(self.hp)


def ctx_get(ctx, off):
    return struct.unpack_from('<I', ctx, off)[0]


def ctx_set(ctx, off, v):
    struct.pack_into('<I', ctx, off, v & 0xFFFFFFFF)


EIP, ESP, EFLAGS = 0xB8, 0xC4, 0xC0


def state_of(ctx, fs_base, tid):
    """The capture's view of a WOW64 context: registers, x87 (stack order, full tag word), MXCSR and XMM0-7."""
    g = lambda off: ctx_get(ctx, off)
    regs = dict(edi=g(0x9C), esi=g(0xA0), ebx=g(0xA4), edx=g(0xA8), ecx=g(0xAC), eax=g(0xB0), ebp=g(0xB4),
                eip=g(0xB8), eflags=g(0xC0), esp=g(0xC4), cs=g(0xBC) & 0xFFFF, ss=g(0xC8) & 0xFFFF,
                fs=g(0x90) & 0xFFFF, ds=g(0x98) & 0xFFFF, es=g(0x94) & 0xFFFF, gs=g(0x8C) & 0xFFFF)
    ext = 0xCC
    fpu = dict(cw=g(0x1C) & 0xFFFF, sw=g(0x20) & 0xFFFF, tw=g(0x24) & 0xFFFF,
               st=bytes(ctx.raw[0x38:0x38 + 80]).hex(), mxcsr=ctx_get(ctx, ext + 24),
               xmm=bytes(ctx.raw[ext + 160:ext + 160 + 128]).hex())
    return dict(regs=regs, fpu=fpu, fs_base=fs_base, tid=tid)


class Debugger:
    """Event loop. on_entry(tid, addr, ctx) -> None; on_exit(tid, pending, ctx); see capture.py."""

    def __init__(self, pid, *, log=print):
        self.proc = Process(pid)
        self.pid = pid
        self.log = log
        self.targets = set()        # entry breakpoints wanted
        self.rearm = {}             # tid -> address to write int3 back after a single step
        self.exits = []             # pending: dict(tid, ret, esp0, ...) awaiting the real return
        self.suspended = []         # thread handles we suspended
        self.suspend_since = None
        self.owner = None
        self.running_since = None
        self.running_seconds = 0.0
        self.owner_cpu = None
        self.next_hold_check = 0.0
        self.isolation_issue = None
        self.attached = False
        self.armed = False

    def attach(self):
        if not k32.DebugActiveProcess(self.pid):
            raise OSError(ctypes.get_last_error(), 'DebugActiveProcess')
        k32.DebugSetProcessKillOnExit(False)
        self.attached = True

    def detach(self):
        self.resume_others()
        try:
            self.proc.disarm_all()
        finally:
            if self.attached:
                k32.DebugActiveProcessStop(self.pid)
                self.attached = False

    def suspend_others(self, tid):
        if self.owner is not None:
            return
        self.owner = tid
        self.isolation_issue = None
        self.running_seconds = 0.0
        self.next_hold_check = 3.0
        self.owner_cpu = self.proc.cpu_seconds(tid)
        for t in list(self.proc.threads):
            if t == tid:
                continue
            h = self.proc.threads[t]
            if k32.Wow64SuspendThread(h) != 0xFFFFFFFF:
                self.suspended.append(h)
            elif k32.WaitForSingleObject(h,0) != 0:
                self.failed_hold(t)
        self.suspend_since = time.time()

    def failed_hold(self, tid):
        self.isolation_issue = dict(field='could not suspend another process thread', held=False,
                                    thread=tid, error=ctypes.get_last_error())
        for p in self.exits:
            p['isolation'] = self.isolation_issue

    def resume_others(self):
        for h in self.suspended:
            k32.ResumeThread(h)
        self.suspended = []
        self.suspend_since = None
        self.owner = None
        self.running_since = None

    def pause_clock(self, now=None):
        if self.running_since is not None and self.owner is not None:
            self.running_seconds += (time.perf_counter() if now is None else now) - self.running_since
        self.running_since = None

    def check_hold(self, hold):
        if self.owner is None or self.running_seconds < max(hold, self.next_hold_check):
            return
        cpu = self.proc.cpu_seconds(self.owner)
        used = None if cpu is None or self.owner_cpu is None else cpu-self.owner_cpu
        wait = self.proc.wait_site(self.owner)
        self.next_hold_check = self.running_seconds + hold
        if used is not None and used > self.running_seconds*.5 and self.running_seconds < 60:
            return
        if wait is None and self.running_seconds < 60:
            return  # computation, or an unproven wait: keep the other threads held
        detail = dict(field='function waits with other threads held' if wait else 'thread isolation runtime limit',
                      running_seconds=self.running_seconds, cpu_seconds=used, wait=wait)
        for p in self.exits:
            p['others_resumed'] = True
            p['isolation'] = detail
        self.log(f'  releasing held threads: {detail}')
        self.resume_others()

    def step_over(self, tid, ctx, addr):
        """Run the original instruction at addr once, then put the int3 back (TF single step)."""
        orig = self.proc.patches.get(addr)
        if orig is not None:
            self.proc.write(addr, orig)
        ctx_set(ctx, EFLAGS, ctx_get(ctx, EFLAGS) | 0x100)
        self.rearm[tid] = addr

    def loop(self, on_entry, on_exit, *, until, timeout, on_idle=None, hold=3.0):
        """Dispatch debug events until until() is true and no exit is pending, or the timeout."""
        ev = DebugEvent()
        deadline = time.time() + timeout
        proc = self.proc
        while time.time() < deadline:
            if self.armed and until() and not self.exits:
                break
            received = k32.WaitForDebugEvent(ctypes.byref(ev), 200)
            # Only time after ContinueDebugEvent counts. Entry replay, nested
            # validation and page-store callbacks happen while the game is paused.
            self.pause_clock()
            if not received:
                self.check_hold(hold)
                if on_idle:
                    on_idle()
                if self.owner is not None:
                    self.running_since = time.perf_counter()
                continue
            status = DBG_CONTINUE
            code = ev.code
            if code == CREATE_PROCESS:
                k32.CloseHandle(_u64(ev, 0))
                proc.thread(ev.tid)
            elif code == CREATE_THREAD:
                proc.thread(ev.tid)
                if self.owner is not None and ev.tid != self.owner:
                    h = proc.threads[ev.tid]
                    if k32.Wow64SuspendThread(h) != 0xFFFFFFFF:
                        self.suspended.append(h)
                    elif k32.WaitForSingleObject(h,0) != 0:
                        self.failed_hold(ev.tid)
            elif code == EXIT_THREAD:
                h = proc.threads.get(ev.tid)
                if h in self.suspended:
                    self.suspended.remove(h)
                proc.forget(ev.tid)
                self.exits = [p for p in self.exits if p['tid'] != ev.tid]
            elif code == LOAD_DLL:
                k32.CloseHandle(_u64(ev, 0))
            elif code == EXIT_PROCESS:
                self.log('game process exited')
                self.attached = False
                return 'exited'
            elif code == EXCEPTION:
                exc, addr = _u32(ev, 0), _u64(ev, 16)
                if exc in BREAKPOINTS and not self.armed:
                    for a in self.targets:
                        proc.arm(a)
                    self.armed = True
                    self.log(f'attached; {len(self.targets)} entry breakpoints armed')
                elif exc in BREAKPOINTS and addr in proc.patches:
                    ctx = proc.get_ctx(ev.tid)
                    ctx_set(ctx, EIP, addr)
                    esp = ctx_get(ctx, ESP)
                    mine = [p for p in self.exits if p['tid'] == ev.tid and p['ret'] == addr and esp > p['esp0']]
                    if mine:
                        # Tail-called functions can share an entry ESP and a
                        # return address. They all finish at this one stop.
                        for p in sorted(mine, key=lambda q: q['esp0']):
                            self.exits.remove(p)
                            on_exit(ev.tid, p, ctx)
                        if not any(q['ret'] == addr for q in self.exits) and addr not in self.targets:
                            proc.disarm(addr)
                        else:
                            self.step_over(ev.tid, ctx, addr)
                        if not self.exits:
                            self.resume_others()
                    elif addr in self.targets:
                        keep = True if self.exits and self.exits[0]['tid'] != ev.tid else on_entry(ev.tid, addr, ctx)
                        if keep:
                            self.step_over(ev.tid, ctx, addr)
                        else:
                            self.targets.discard(addr)
                            proc.disarm(addr)
                    else:
                        # another thread, or a deeper activation, passing a watched return address
                        self.step_over(ev.tid, ctx, addr)
                    proc.set_ctx(ev.tid, ctx)
                elif exc in SINGLE_STEPS and ev.tid in self.rearm:
                    a = self.rearm.pop(ev.tid)
                    if a in proc.patches:
                        proc.write(a, b'\xCC')
                elif exc in BREAKPOINTS:
                    pass                   # the attach breakpoint of a later thread, or the game's own int3
                else:
                    status = DBG_NOT_HANDLED
            k32.ContinueDebugEvent(ev.pid, ev.tid, status)
            if self.owner is not None:
                self.running_since = time.perf_counter()
        return 'timeout' if time.time() >= deadline else 'done'

    def wait_exit(self, p, tid, ret, esp0):
        p.update(tid=tid, ret=ret, esp0=esp0)
        self.proc.arm(ret)
        self.exits.append(p)
        self.suspend_others(tid)
        if self.isolation_issue:
            p['isolation'] = self.isolation_issue


class LiveProvider:
    """Pages of the stopped game process, read on demand (the emulator maps them on first touch)."""

    def __init__(self, proc):
        self.proc = proc
        self.reads = 0

    def get(self, p):
        self.reads += 1
        return self.proc.page(p)
