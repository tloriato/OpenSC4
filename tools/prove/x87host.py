"""Run x87 transcendental instructions on the host CPU instead of the emulator's double-precision approximation.

The emulator computes FSIN, FCOS, FPTAN, FPATAN, FYL2X, F2XM1 and the others through host double math, so the low
11 bits of the 64-bit mantissa can be wrong. The host CPU's own x87 unit gives the bits the real game gets on this CPU
(measured: 54,000 of 54,000 results bit-identical). The helper is a tiny x64 DLL built from x87/host.asm.
"""
import ctypes
import struct
import subprocess
import os
from pathlib import Path

from .common import OUT

HERE = Path(__file__).resolve().parent
DLL = OUT / 'bin' / 'x87host.dll'

# second opcode byte after D9 -> (helper op, stack effect: 0 in place, +1 push, -1 pop)
OPS = {0xFE: (0, 0), 0xFF: (1, 0), 0xF2: (2, +1), 0xF3: (3, -1), 0xF1: (4, -1), 0xF0: (5, 0), 0xFB: (6, +1),
       0xF9: (7, -1), 0xFD: (8, 0), 0xF8: (9, 0), 0xF5: (10, 0)}

_dll = None


def load():
    global _dll
    if _dll is None:
        if not DLL.is_file():
            DLL.parent.mkdir(parents=True, exist_ok=True)
            vswhere = Path(os.environ['ProgramFiles(x86)']) / 'Microsoft Visual Studio/Installer/vswhere.exe'
            vs = subprocess.check_output([str(vswhere), '-latest', '-products', '*', '-requires',
                                          'Microsoft.VisualStudio.Component.VC.Tools.x86.x64',
                                          '-property', 'installationPath'], text=True).strip()
            if not vs:
                raise SystemExit('x87 helper needs Visual Studio 2022 C++ Build Tools')
            r = subprocess.run(['cmd', '/c', str(HERE / 'x87' / 'build.cmd'), str(DLL.parent), vs], capture_output=True,
                               text=True)
            if r.returncode or not DLL.is_file():
                raise SystemExit(f'building {DLL} failed:\n{r.stdout}\n{r.stderr}')
        _dll = ctypes.CDLL(str(DLL))
        _dll.x87host.argtypes = [ctypes.c_int, ctypes.c_uint, ctypes.c_char_p, ctypes.c_char_p]
    return _dll


def scan(code, base):
    """Candidate sites in raw code bytes. A hook at a non-instruction address never fires, so extra hits are harmless."""
    out, i = [], code.find(b'\xD9')
    while i >= 0:
        if i + 1 < len(code) and code[i + 1] in OPS:
            out.append(base + i)
        i = code.find(b'\xD9', i + 1)
    return out


class Host:
    def __init__(self, uc, sites):
        from unicorn import UC_HOOK_CODE
        from unicorn.x86_const import UC_X86_REG_FPCW
        self.dll = load()
        self.count = 0
        self.cwreg = UC_X86_REG_FPCW
        self.handles = []
        for a in sorted(set(sites)):
            self.handles.append(uc.hook_add(UC_HOOK_CODE, self._hook, None, a, a))

    def add(self, uc, sites):
        from unicorn import UC_HOOK_CODE
        for a in sorted(set(sites)):
            self.handles.append(uc.hook_add(UC_HOOK_CODE, self._hook, None, a, a))

    def _hook(self, uc, addr, size, _):
        from unicorn.x86_const import UC_X86_REG_FP0, UC_X86_REG_FPSW, UC_X86_REG_FPTAG, UC_X86_REG_EIP
        b = bytes(uc.mem_read(addr, 2))
        if b[0] != 0xD9 or b[1] not in OPS:
            return
        op, effect = OPS[b[1]]
        sw = uc.reg_read(UC_X86_REG_FPSW); top = (sw >> 11) & 7
        st = b''.join(struct.pack('<QH', *uc.reg_read(UC_X86_REG_FP0 + ((top + i) & 7))) for i in range(2))
        buf = ctypes.create_string_buffer(st + b'\0' * 62, 82)
        out = ctypes.create_string_buffer(82)
        self.dll.x87host(op, uc.reg_read(self.cwreg), buf, out)
        res = [struct.unpack_from('<QH', out.raw, 10 * i) for i in range(3)]
        hsw = struct.unpack_from('<H', out.raw, 80)[0]
        tag = uc.reg_read(UC_X86_REG_FPTAG)
        if effect == 0:
            uc.reg_write(UC_X86_REG_FP0 + top, res[0])
        elif effect == +1:
            ntop = (top - 1) & 7
            uc.reg_write(UC_X86_REG_FP0 + ntop, res[0]); uc.reg_write(UC_X86_REG_FP0 + top, res[1])
            tag &= ~(3 << (2 * ntop)); top = ntop
        else:
            uc.reg_write(UC_X86_REG_FP0 + ((top + 1) & 7), res[0])
            tag |= 3 << (2 * top); top = (top + 1) & 7
        # condition codes from the host, sticky exception flags OR-ed in, our own TOP
        nsw = (sw & 0x00FF | hsw & 0x3F) & ~0x4700 | (hsw & 0x4700)
        nsw = nsw & ~0x3800 | (top << 11)
        uc.reg_write(UC_X86_REG_FPSW, nsw); uc.reg_write(UC_X86_REG_FPTAG, tag)
        uc.reg_write(UC_X86_REG_EIP, addr + 2)
        self.count += 1
