"""Minimal x86-32 interpreter (capstone-decoded, exe bytes) for the 35 mnemonics the RenderBlt kernels use.
Only ZF/CF are modelled (the kernels branch on JZ/JNZ/JC only)."""
import struct, sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import capstone
from capstone import x86
from pe import by
md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32); md.detail = True
_cache = {}
def dec(a):
    i = _cache.get(a)
    if i is None:
        i = next(md.disasm(by(a, 16), a)); _cache[a] = i
    return i
R32 = ['eax', 'ecx', 'edx', 'ebx', 'esp', 'ebp', 'esi', 'edi']
SUB = {}
for k, n in enumerate(R32):
    SUB[n] = (n, 0, 32)
for n, p in zip(['ax', 'cx', 'dx', 'bx', 'sp', 'bp', 'si', 'di'], R32): SUB[n] = (p, 0, 16)
for n, p in zip(['al', 'cl', 'dl', 'bl'], R32[:4]): SUB[n] = (p, 0, 8)
for n, p in zip(['ah', 'ch', 'dh', 'bh'], R32[:4]): SUB[n] = (p, 8, 8)
M = (1 << 32) - 1
class CPU:
    def __init__(s, mem):
        s.r = {n: 0 for n in R32}; s.mm = [0] * 8; s.zf = 0; s.cf = 0; s.mem = mem; s.steps = 0
    def rb(s, a):
        a &= M
        v = s.mem.get(a)
        if v is None:
            v = by(a, 1)[0] if a < 0x01000000 else 0  # image data (constants); test buffers default 0
        return v
    def rd(s, a, n): return sum(s.rb(a + k) << (8 * k) for k in range(n))
    def wr(s, a, n, v):
        for k in range(n): s.mem[(a + k) & M] = (v >> (8 * k)) & 0xFF
    def ea(s, i, op):
        m = op.mem; a = m.disp
        if m.base: a += s.r[i.reg_name(m.base)]
        if m.index: a += s.r[i.reg_name(m.index)] * m.scale
        return a & M
    def get(s, i, op):
        if op.type == x86.X86_OP_REG:
            n = i.reg_name(op.reg)
            if n.startswith('mm'): return s.mm[int(n[2])]
            p, sh, w = SUB[n]; return (s.r[p] >> sh) & ((1 << w) - 1)
        if op.type == x86.X86_OP_IMM: return op.imm & ((1 << (8 * op.size)) - 1)
        return s.rd(s.ea(i, op), op.size)
    def put(s, i, op, v):
        if op.type == x86.X86_OP_REG:
            n = i.reg_name(op.reg)
            if n.startswith('mm'): s.mm[int(n[2])] = v & ((1 << 64) - 1); return
            p, sh, w = SUB[n]; mk = ((1 << w) - 1) << sh
            s.r[p] = (s.r[p] & ~mk & M) | ((v << sh) & mk); return
        s.wr(s.ea(i, op), op.size, v)
    def run(s, pc, stop=0xDEAD0000, limit=60_000_000):
        while pc != stop:
            s.steps += 1
            if s.steps > limit: raise RuntimeError('step limit at %08x' % pc)
            i = dec(pc); nx = pc + i.size; o = i.operands; mn = i.mnemonic
            if mn in ('mov', 'movzx'):
                s.put(i, o[0], s.get(i, o[1]))
            elif mn == 'lea':
                s.put(i, o[0], s.ea(i, o[1]))
            elif mn in ('add', 'sub', 'cmp', 'and', 'or', 'xor', 'test'):
                w = 8 * o[0].size; mk = (1 << w) - 1; a = s.get(i, o[0]); b = s.get(i, o[1]) & mk
                if mn == 'add': r = a + b; s.cf = r >> w & 1
                elif mn in ('sub', 'cmp'): r = a - b; s.cf = 1 if a < b else 0
                elif mn in ('and', 'test'): r = a & b; s.cf = 0
                elif mn == 'or': r = a | b; s.cf = 0
                else: r = a ^ b; s.cf = 0
                r &= mk; s.zf = 1 if r == 0 else 0
                if mn not in ('cmp', 'test'): s.put(i, o[0], r)
            elif mn in ('inc', 'dec', 'neg'):
                w = 8 * o[0].size; mk = (1 << w) - 1; a = s.get(i, o[0])
                r = (a + 1 if mn == 'inc' else a - 1 if mn == 'dec' else -a) & mk
                if mn == 'neg': s.cf = 1 if a else 0
                s.zf = 1 if r == 0 else 0; s.put(i, o[0], r)
            elif mn in ('shl', 'shr'):
                w = 8 * o[0].size; mk = (1 << w) - 1; a = s.get(i, o[0]); c = (s.get(i, o[1]) if len(o) > 1 else 1) & 31
                if c:
                    if mn == 'shl': r = (a << c) & mk; s.cf = (a >> (w - c)) & 1
                    else: r = a >> c; s.cf = (a >> (c - 1)) & 1
                    s.zf = 1 if r == 0 else 0; s.put(i, o[0], r)
            elif mn == 'imul':
                if len(o) == 3: r = s.get(i, o[1]) * s.get(i, o[2])
                elif len(o) == 2: r = s.get(i, o[0]) * s.get(i, o[1])
                else: raise RuntimeError('imul1 at %08x' % pc)
                s.put(i, o[0], r & M)  # low 32 bits identical for signed/unsigned
            elif mn == 'div':
                d = s.get(i, o[0]); n = (s.r['edx'] << 32) | s.r['eax']
                if d == 0: raise ZeroDivisionError('%08x' % pc)
                q = n // d
                if q > M: raise RuntimeError('div overflow')
                s.r['eax'] = q; s.r['edx'] = n % d
            elif mn == 'push':
                v = s.get(i, o[0]); s.r['esp'] = (s.r['esp'] - 4) & M; s.wr(s.r['esp'], 4, v)
            elif mn == 'pop':
                v = s.rd(s.r['esp'], 4); s.r['esp'] = (s.r['esp'] + 4) & M; s.put(i, o[0], v)
            elif mn == 'call':
                s.r['esp'] = (s.r['esp'] - 4) & M; s.wr(s.r['esp'], 4, nx); nx = o[0].imm
            elif mn == 'ret':
                nx = s.rd(s.r['esp'], 4); s.r['esp'] = (s.r['esp'] + 4 + (o[0].imm if o else 0)) & M
            elif mn == 'jmp': nx = o[0].imm
            elif mn in ('je', 'jne', 'jb'):
                t = {'je': s.zf, 'jne': not s.zf, 'jb': s.cf}[mn]
                if t: nx = o[0].imm
            elif mn == 'rep stosd':  # rep stosd, DF clear
                while s.r['ecx']:
                    s.wr(s.r['edi'], 4, s.r['eax']); s.r['edi'] = (s.r['edi'] + 4) & M; s.r['ecx'] -= 1
            elif mn in ('nop', 'emms'): pass
            elif mn in ('movd', 'movq'):
                n = 4 if mn == 'movd' else 8
                v = s.get(i, o[1]) & ((1 << (8 * n)) - 1)
                if o[1].type == x86.X86_OP_MEM: v = s.rd(s.ea(i, o[1]), n)
                if o[0].type == x86.X86_OP_MEM: s.wr(s.ea(i, o[0]), n, v)
                else: s.put(i, o[0], v)
            elif mn in ('punpcklbw', 'pxor', 'pmullw', 'paddw', 'psrlw', 'packuswb'):
                a = s.get(i, o[0])
                b = s.rd(s.ea(i, o[1]), 8) if o[1].type == x86.X86_OP_MEM else s.get(i, o[1])
                W = lambda x: [(x >> (16 * k)) & 0xFFFF for k in range(4)]
                J = lambda ws: sum((w & 0xFFFF) << (16 * k) for k, w in enumerate(ws))
                if mn == 'pxor': r = a ^ b
                elif mn == 'punpcklbw':
                    r = 0
                    for k in range(4): r |= ((a >> (8 * k)) & 0xFF) << (16 * k) | ((b >> (8 * k)) & 0xFF) << (16 * k + 8)
                elif mn == 'pmullw': r = J([x * y for x, y in zip(W(a), W(b))])
                elif mn == 'paddw': r = J([x + y for x, y in zip(W(a), W(b))])
                elif mn == 'psrlw':
                    c = o[1].imm if o[1].type == x86.X86_OP_IMM else b
                    r = J([x >> c if c < 16 else 0 for x in W(a)])
                else:
                    sat = lambda x: 0 if x >= 0x8000 else min(x, 255)  # signed word -> unsigned byte
                    r = sum(sat(x) << (8 * k) for k, x in enumerate(W(a) + W(b)))
                s.put(i, o[0], r)
            else:
                raise RuntimeError('unhandled %s %s at %08x' % (mn, i.op_str, pc))
            pc = nx
