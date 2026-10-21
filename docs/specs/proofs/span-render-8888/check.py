"""Run every RenderBlt kernel in emu.py on random inputs and compare with the spec model (span-render-8888.md model).
usage: check.py [trials] [r c ...]   prints per-kernel mismatch counts and the first mismatch."""
import sys, random, os
MUT = int(os.environ.get('MUT', '0')); EXH = int(os.environ.get('EXH', '0'))
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pe import dw
from emu import CPU
tab = [[dw(dw(0xac23d8 + 4 * r) + 4 * c) for c in range(12)] for r in range(14)]
BPP = [2, 2, 2, 2, 4, 4, 1]
MASK = [0xFFFF if MUT == 4 else 0x0FFF, 0x7FFF, 0x7FFF, 0xFFFF, 0xFFFFFF, 0xFFFFFF, 0]
def to8888(f, p):
    w5 = lambda v: (v << 3) | (v >> 2)
    if f == 0: return sum(((p >> (4 * k)) & 15) * 17 << (8 * k) for k in range(4))
    if f in (1, 2):
        a = (0xFF if p >> 15 else 0) if f == 1 else 0xFF
        return a << 24 | w5((p >> 10) & 31) << 16 | w5((p >> 5) & 31) << 8 | w5(p & 31)
    if f == 3:
        g = (p >> 5) & 63
        return 0xFF000000 | w5(p >> 11) << 16 | ((g << 2) | (g >> 4)) << 8 | w5(p & 31)
    if f == 4: return p
    if f == 5: return (0xFE000000 if MUT == 6 else 0xFF000000) | (p & 0xFFFFFF)
    return p << 24 | 0xFFFFFF
def blend(d, s, w):
    o = 0xFF000000
    for sh in (16, 8, 0):
        sc = (s >> sh) & 255; dc = (d >> sh) & 255
        o |= ((256 * dc + w * (sc - dc) + (127 if MUT == 1 else 128)) // 256) << sh
    return o
def pix(f, op, p, d, col):
    C = to8888(f, p)
    if op == 0: return C
    if op == 1: return blend(d, C, (col >> 24) + (MUT == 2))
    Mv = sum(((((C >> (8 * k)) & 255) * ((col >> (8 * k)) & 255) + (0 if MUT == 3 else 128)) >> 8) << (8 * k) for k in range(4))
    return Mv if op == 2 else blend(d, Mv, Mv >> 24)
DST, SRC, SPN, STK = 0x10000000, 0x20000000, 0x30000000, 0x40100000
def trial(r, c, rnd):
    f, keyed = r // 2, r & 1; geo, op = c // 4, c % 4; bpp = BPP[f]
    W, H = rnd.choice([(37, 11), (64, 16), (5, 3)]); sstr = W * bpp + rnd.choice([0, 6])
    dstr = 4 * 80 + rnd.choice([0, 4]); mem = {}
    if EXH: W, H = (256, 256 if bpp == 2 else 1); sstr = W * bpp; dstr = 2048
    simg = {}
    for y in range(H):
        for x in range(W):
            v = rnd.getrandbits(8 * bpp)
            if EXH: v = y * 256 + x
            if rnd.random() < 0.1 and not EXH: v = rnd.choice([0, (1 << 8 * bpp) - 1])
            simg[(x, y)] = v
            for k in range(bpp): mem[SRC + y * sstr + x * bpp + k] = (v >> 8 * k) & 255
    spans = []
    for _ in range(rnd.randint(1, 4)):
        y = rnd.randrange(0, 40 if geo == 2 else (H if geo == 0 else 40)); x0 = rnd.randrange(0, 50 if geo else W)
        n = rnd.randint(1, 25 if geo else max(1, W - x0)); spans.append((x0, y, n))
    if EXH: spans = [(0, y, 256) for y in range(H)]
    dimg = {}
    for (x0, y, n) in spans:
        for x in range(x0 - 1, x0 + n + 1):
            if x < 0: continue
            v = rnd.getrandbits(32); dimg[(x, y)] = v
            for b in range(4): mem[DST + y * dstr + 4 * x + b] = (v >> 8 * b) & 255
    for k, (x0, y, n) in enumerate(spans):
        st = (y << 16) | x0; en = st + n
        for j, v in enumerate((st, en)):
            for b in range(4): mem[SPN + 8 * k + 4 * j + b] = (v >> 8 * b) & 255
    col = rnd.getrandbits(32)
    if rnd.random() < 0.3: col = rnd.choice([0xFFFFFFFF, 0, 0x80808080, 0xF0FFFFFF])
    if geo == 1:
        u0, v0 = rnd.randrange(0, 0x20000), rnd.randrange(0, 0x10000)
        du = rnd.randrange(0x800, max(0x1000, (W << 16) // 80)); dv = rnd.randrange(0x800, max(0x1000, ((H - 1) << 16) // 40 + 1))
    elif geo == 2:
        u0, v0, du, dv = rnd.randrange(0, 1000), rnd.randrange(0, 1000), rnd.getrandbits(32), rnd.getrandbits(32)
    else:
        u0 = v0 = du = dv = 0  # direct: ignored
        du, dv = rnd.getrandbits(32), rnd.getrandbits(32); u0, v0 = rnd.getrandbits(32), rnd.getrandbits(32)
    texels = list(simg.values())
    key = rnd.getrandbits(32)
    if keyed and rnd.random() < 0.8: key = rnd.choice(texels) & MASK[f]
    if f == 6 and rnd.random() < 0.5: key = 0
    args = [SPN, len(spans), DST, dstr] + [rnd.getrandbits(32) for _ in range(4)] + [SRC, sstr, W, H] + \
        [rnd.getrandbits(32) for _ in range(2)] + [u0, v0, du, dv, r // 2, col, key, keyed | (rnd.getrandbits(24) << 8)]
    cpu = CPU(mem); sp = STK
    words = [0xDEAD0000] + args
    for k, v in enumerate(words):
        for b in range(4): mem[sp + 4 * k + b] = (v >> 8 * b) & 255
    cpu.r['esp'] = sp
    for n_ in ('eax', 'ecx', 'edx', 'ebx', 'ebp', 'esi', 'edi'): cpu.r[n_] = rnd.getrandbits(32)
    keep = {n_: cpu.r[n_] for n_ in ('ebx', 'esi', 'edi', 'ebp')}
    cpu.run(tab[r][c])
    clob = [n_ for n_ in keep if cpu.r[n_] != keep[n_]]
    exp = dict(dimg)
    for (x0, y, n) in spans:
        if geo == 1:
            vv = ((dv * y + v0) & 0xFFFFFFFF) >> 16; u = (du * x0 + u0) & 0xFFFFFFFF
        elif geo == 2:
            tx = (x0 + u0 * (MUT != 5)) % W; ty = (y + v0) % H
        for x in range(x0, x0 + n):
            if geo == 0: sx, sy = x, y
            elif geo == 1: sx, sy = u >> 16, vv; u = (u + du) & 0xFFFFFFFF
            else:
                sx, sy = tx, ty; tx += 1
                if tx == W: tx = 0
            p = simg.get((sx, sy))
            if p is None:  # outside the filled image: model reads the same zero-default memory
                p = sum(mem.get(SRC + sy * sstr + sx * bpp + k, 0) << 8 * k for k in range(bpp))
            if keyed and (p & MASK[f]) == key: continue
            exp[(x, y)] = pix(f, op, p, exp[(x, y)], col)
    bad = []
    for (x, y), v in exp.items():
        got = sum(mem[DST + y * dstr + 4 * x + b] << 8 * b for b in range(4))
        if got != v: bad.append((x, y, v, got))
    npx = sum(n for _, _, n in spans)
    if clob: bad.append(('clobber', clob))
    return bad, npx, cpu.steps, (cpu.r['esp'] - sp)
if __name__ == '__main__':
    T = int(sys.argv[1]) if len(sys.argv) > 1 else 5
    rc = [int(x) for x in sys.argv[2:]]
    pairs = [(rc[i], rc[i + 1]) for i in range(0, len(rc), 2)] or [(r, c) for r in range(14) for c in range(12)]
    tot = 0; fails = 0; px = 0
    for r, c in pairs:
        rnd = random.Random(r * 100 + c); nb = 0; first = None; np_ = 0
        for t in range(T):
            try:
                bad, n, st, dsp = trial(r, c, rnd)
            except Exception as e:
                bad, n, dsp = [('exc', str(e))], 0, 0
            np_ += n
            if dsp != 4: bad = bad + [('esp', dsp)]
            if bad and first is None: first = bad[0]
            nb += len(bad)
        tot += 1; px += np_
        if nb: fails += 1; print('r%d c%d %08x mism %d first %s' % (r, c, tab[r][c], nb, first))
    print('kernels', tot, 'failing', fails, 'pixels', px)
