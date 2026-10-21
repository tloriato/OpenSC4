"""Shared unicorn loader for the cSpanRender8 proofs (docs/specs/span-render-8.md).
Needs unicorn (pip install unicorn, or a pylib/ dir next to this file) and the GOG 1.1.641 SimCity4.exe.
usage: python span8_emu.py   -> prints the 14x12 RenderBlt table at 0x00AC26B0."""
import os, sys, struct
H = os.path.dirname(os.path.abspath(__file__))
for p in (os.path.join(H, 'pylib'), os.path.join(H, '..', '..', '..', 'build-runs', 'pylib')):
    if os.path.isdir(p): sys.path.insert(0, p)
from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_ESP
EXE = os.environ.get('SC4EXE', 'C:/Users/tlori/FreeCity4/bin/SimCity4.exe')
d = open(EXE, 'rb').read()
pe = struct.unpack_from('<I', d, 0x3c)[0]; nsec = struct.unpack_from('<H', d, pe + 6)[0]
optsz = struct.unpack_from('<H', d, pe + 20)[0]; base = struct.unpack_from('<I', d, pe + 52)[0]
isz = struct.unpack_from('<I', d, pe + 80)[0]
mu = Uc(UC_ARCH_X86, UC_MODE_32); mu.mem_map(base, (isz + 0xfff) & ~0xfff)
o = pe + 24 + optsz
for i in range(nsec):
    vs, va, rs, ra = struct.unpack_from('<IIII', d, o + 8); mu.mem_write(base + va, d[ra:ra + min(rs, vs or rs)]); o += 40
STK, DAT, RET = 0x10000000, 0x20000000, 0x30000000
mu.mem_map(STK, 0x100000); mu.mem_map(DAT, 0x400000); mu.mem_map(RET, 0x1000)
M = 0xFFFFFFFF
def rd(a, n): return bytes(mu.mem_read(a, n))
def dw(a): return struct.unpack('<I', rd(a, 4))[0]
ROWS = 0x00AC26B0
TAB = [[dw(dw(ROWS + 4 * r) + 4 * c) for c in range(12)] for r in range(14)]
def call(fa, args, count=20000000):
    esp = STK + 0x80000; mu.mem_write(esp, struct.pack('<I', RET) + args)
    mu.reg_write(UC_X86_REG_ESP, esp); mu.emu_start(fa, RET, count=count)
    return mu.reg_read(UC_X86_REG_ESP) - esp - 4   # bytes popped by the callee
if __name__ == '__main__':
    print('rows', ' '.join('%08x' % dw(ROWS + 4 * r) for r in range(14)))
    for r in range(14): print(r, ' '.join('%05X' % (a & 0xFFFFF) for a in TAB[r]))
