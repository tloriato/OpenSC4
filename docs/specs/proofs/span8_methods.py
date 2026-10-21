"""Differential proof of the five cSpanRender8 methods (span-render-8.md sections 2-6) under unicorn: random
surfaces, spans, colours and gradient records (full 32-bit range) against the spec model, bytes popped, and that
RenderBlt reaches rows[key + 2f][kind]. this = an object whose vtable is 0x00AC188C.
usage: python span8_methods.py [trials]"""
import sys, struct, random
import span8_emu as E
from unicorn.x86_const import UC_X86_REG_ECX
mu, DAT, M, TAB = E.mu, E.DAT, E.M, E.TAB
DW, DH, DSTR = 64, 6, 72
THIS = DAT + 0x3F000
mu.mem_write(THIS, struct.pack('<I', 0x00AC188C))
def call(fa, args):
    mu.reg_write(UC_X86_REG_ECX, THIS); return E.call(fa, args)
def setup(rng):
    dst = [rng.getrandbits(8) for _ in range(DW * DH)]; buf = bytearray(DSTR * DH)
    for i, v in enumerate(dst): buf[(i // DW) * DSTR + i % DW] = v
    mu.mem_write(DAT + 0x10000, bytes(buf))
    spans = []
    for _ in range(rng.randint(1, 4)):
        y = rng.randrange(DH); a = rng.randrange(DW); spans.append((a, rng.randint(a, DW), y))
    mu.mem_write(DAT + 0x20000, b''.join(struct.pack('<II', (y << 16) | a, (y << 16) | b) for a, b, y in spans))
    mu.mem_write(DAT + 0x30000, struct.pack('<6I', DAT + 0x10000, DSTR, DW, DH, 2, 1))
    return dst, spans, struct.pack('<III', DAT + 0x20000, len(spans), DAT + 0x30000)
def check(exp, popped, want):
    if popped != want: return 'popped %d want %d' % (popped, want)
    out = E.rd(DAT + 0x10000, DSTR * DH)
    for i, e in enumerate(exp):
        g = out[(i // DW) * DSTR + i % DW]
        if g != e: return 'x=%d y=%d exp=%02x got=%02x' % (i % DW, i // DW, e, g)
    return ''
def fill(dst, spans, v):
    for x0, x1, y in spans:
        for x in range(x0, x1): dst[y * DW + x] = v
def t_solid(rng):      # section 2: low byte of the colour
    dst, spans, a = setup(rng); col = rng.getrandbits(32); p = call(0x0085E640, a + struct.pack('<I', col))
    fill(dst, spans, col & 255); return check(dst, p, 0x10)
def t_alpha(rng):      # section 3: = RenderSolid(src) through vtable slot 1; argb ignored
    dst, spans, a = setup(rng); s, c = rng.getrandbits(32), rng.getrandbits(32)
    p = call(0x0085E720, a + struct.pack('<II', s, c)); fill(dst, spans, s & 255); return check(dst, p, 0x14)
def grad(rng, fa):     # sections 4, 5: byte = bits 16-23 of c3 + dx3*x + dy3*y (32-bit wrapping)
    dst, spans, a = setup(rng); g = [rng.getrandbits(32) for _ in range(12)]
    mu.mem_write(DAT + 0x31000, struct.pack('<12I', *g))
    p = call(fa, a + struct.pack('<I', DAT + 0x31000))
    for x0, x1, y in spans:
        for x in range(x0, x1): dst[y * DW + x] = ((g[3] + g[7] * x + g[11] * y) & M) >> 16 & 255
    return check(dst, p, 0x10)
def t_grad(rng): return grad(rng, 0x0085E6A0)
def t_agrad(rng): return grad(rng, 0x0085E740)
def t_blt(rng):        # section 6: RenderBlt calls rows[byte b+0x1C + 2 * dword b+0x10][kind] with the records by value
    hit = []
    f, key, kind = rng.randrange(7), rng.randrange(2), rng.randrange(12)
    target = TAB[2 * f + key][kind]
    h = mu.hook_add(E.UC_HOOK_CODE, lambda u, ad, sz, d: (hit.append(1), u.emu_stop()), begin=target, end=target)
    dst, spans, a = setup(rng)
    mu.mem_write(DAT + 0x32000, struct.pack('<6I', DAT + 0x10000, DSTR, DW, DH, 2, 1) + struct.pack('<6I', DAT, 4, 1, 1, 0, 1)
                 + struct.pack('<8I', 0, 0, 0, 0, f, 0, 0, key | (rng.getrandbits(24) << 8)))
    call(0x0085E750, a[:8] + struct.pack('<4I', DAT + 0x32000, DAT + 0x32018, DAT + 0x32030, kind))
    mu.hook_del(h)
    return '' if hit else 'slot %d,%d not reached' % (2 * f + key, kind)
def main():
    nt = int(sys.argv[1]) if len(sys.argv) > 1 else 100
    for name, fn in (('RenderSolid', t_solid), ('RenderAlpha', t_alpha), ('RenderGradient', t_grad),
                     ('RenderAlphaGradient', t_agrad), ('RenderBlt dispatch', t_blt)):
        rng = random.Random(name); msg = 'PASS %d' % nt
        for _ in range(nt):
            r = fn(rng)
            if r: msg = 'FAIL ' + r; break
        print(name, msg)
if __name__ == '__main__': main()
