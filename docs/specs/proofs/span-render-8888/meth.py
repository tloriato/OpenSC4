"""Execute the four span methods of cSpanRender8888 in emu.py against the spec model.
usage: meth.py [trials]   env MUT=n changes one model rule (sensitivity), BIGW=1 lets the alpha-gradient weight exceed 256."""
import sys, random, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from emu import CPU
MUT = int(os.environ.get('MUT', '0')); BIGW = int(os.environ.get('BIGW', '0'))
M32 = 0xFFFFFFFF
DST, SPN, REC, STK = 0x10000000, 0x30000000, 0x50000000, 0x40100000
def packed(d, s, w):
    rb = ((((((s & 0xFF00FF) - (d & 0xFF00FF)) * w + 0x800080) & M32) >> 8) + (d & 0xFF00FF)) & M32
    g = (((((s & 0xFF00) - (d & 0xFF00)) & M32) >> 8) * w + (d & 0xFF00) + 0x80) & M32
    return (rb | 0xFF00FF00) & (g | 0xFFFF00FF)
def chan(d, s, w):
    o = 0xFF000000
    for sh in (16, 8, 0):
        sc = (s >> sh) & 255; dc = (d >> sh) & 255
        o |= ((256 * dc + w * (sc - dc) + 128) // 256) << sh
    return o
def run(addr, args, mem, rnd):
    cpu = CPU(mem); sp = STK
    for k, v in enumerate([0xDEAD0000] + args):
        for b in range(4): mem[sp + 4 * k + b] = (v >> 8 * b) & 255
    for n in ('eax', 'edx', 'ebx', 'ebp', 'esi', 'edi'): cpu.r[n] = rnd.getrandbits(32)
    cpu.r['ecx'] = 0x60000000; cpu.r['esp'] = sp
    keep = {n: cpu.r[n] for n in ('ebx', 'esi', 'edi', 'ebp')}
    cpu.run(addr)
    bad = [('clobber', n) for n in keep if cpu.r[n] != keep[n]]
    if cpu.r['esp'] - sp != 4 + 4 * len(args): bad.append(('esp', cpu.r['esp'] - sp))
    return bad
def trial(meth, rnd):
    mem = {}; dstr = 4 * 90 + rnd.choice([0, 4, 12]); spans = []; dimg = {}
    for _ in range(rnd.randint(1, 4)):
        y = rnd.randrange(0, 30); x0 = rnd.randrange(0, 60); n = rnd.randint(0, 25)
        spans.append((x0, y, n))
    for (x0, y, n) in spans:
        for x in range(max(0, x0 - 1), x0 + n + 1):
            v = rnd.getrandbits(32)
            if rnd.random() < 0.1: v = rnd.choice([0, M32])
            dimg[(x, y)] = v
            for b in range(4): mem[DST + y * dstr + 4 * x + b] = (v >> 8 * b) & 255
    for k, (x0, y, n) in enumerate(spans):
        st = (y << 16) | x0
        for j, v in enumerate((st, st + n)):
            for b in range(4): mem[SPN + 8 * k + 4 * j + b] = (v >> 8 * b) & 255
    for b in range(4): mem[0x20000000 + b] = 0
    buf = 0x20000100
    for j, v in enumerate((DST, dstr)):
        for b in range(4): mem[buf + 4 * j + b] = (v >> 8 * b) & 255
    exp = dict(dimg)
    col = rnd.getrandbits(32)
    if rnd.random() < 0.3: col = rnd.choice([0, M32, 0x80808080, 0x7FFFFFFF, 0xFF000000])
    if meth == 'solid':
        args = [SPN, len(spans), buf, col]; addr = 0x854e10
        for (x0, y, n) in spans:
            for x in range(x0, x0 + n): exp[(x, y)] = col
    elif meth == 'alpha':
        al = rnd.getrandbits(32)
        if rnd.random() < 0.4: al = rnd.choice([0, M32, 0x80000000, 0x7FFFFFFF, 0x01000000, 0xFF000000])
        args = [SPN, len(spans), buf, col, al]; addr = 0x854f70
        a = (al >> 24) + (al >> 31) + (MUT == 1)
        for (x0, y, n) in spans:
            for x in range(x0, x0 + n): exp[(x, y)] = chan(exp[(x, y)], col, a)
    else:
        nc = 3 if meth == 'grad' else 4
        rec = [0] * 12
        for k in range(nc):
            if meth == 'grad' or k < 3 or BIGW:
                rec[k] = rnd.getrandbits(32); rec[4 + k] = rnd.getrandbits(32) >> rnd.choice([0, 8, 12, 16]); rec[8 + k] = rnd.getrandbits(32) >> rnd.choice([8, 12, 16])
                if rnd.random() < 0.5: rec[4 + k] = -rec[4 + k] & M32
            else:  # weight accumulator kept in 0..256<<16 over the whole test area
                rec[4 + k] = rnd.randrange(-0x2000, 0x2000) & M32; rec[8 + k] = rnd.randrange(-0x400, 0x400) & M32
                lo = 0x1000000 if rnd.random() < 0.2 else 0  # exact 256 tops reachable
                rec[k] = rnd.randrange(0, 0x1000001) if not lo else 0x1000000
        for k in range(nc, 4): rec[k] = rnd.getrandbits(32); rec[4 + k] = rnd.getrandbits(32); rec[8 + k] = rnd.getrandbits(32)
        for j, v in enumerate(rec):
            for b in range(4): mem[REC + 4 * j + b] = (v >> 8 * b) & 255
        args = [SPN, len(spans), buf, REC]; addr = 0x854e60 if meth == 'grad' else 0x855040
        for (x0, y, n) in spans:
            acc = [(rec[k] + x0 * rec[4 + k] + y * rec[8 + k]) & M32 for k in range(4)]
            if meth == 'grad': acc[2] |= 0 if MUT == 2 else 0xFF000000
            for x in range(x0, x0 + n):
                s = (acc[2] & (0xFFFF0000 if meth == 'grad' else 0xFF0000)) | ((acc[1] >> 16) & 255) << 8 | ((acc[0] >> 16) & 255)
                if meth == 'grad': exp[(x, y)] = s
                else:
                    w = acc[3] >> 16
                    exp[(x, y)] = packed(exp[(x, y)], s, w) if (BIGW or w > 256) else chan(exp[(x, y)], s, w + (MUT == 3))
                acc = [(acc[k] + rec[4 + k]) & M32 for k in range(4)]
    bad = run(addr, args, mem, rnd)
    for (x, y), v in exp.items():
        got = sum(mem[DST + y * dstr + 4 * x + b] << 8 * b for b in range(4))
        if got != v: bad.append((x, y, '%08x' % v, '%08x' % got))
    return bad, sum(n for _, _, n in spans)
if __name__ == '__main__':
    T = int(sys.argv[1]) if len(sys.argv) > 1 else 20
    for meth in ('solid', 'alpha', 'grad', 'agrad'):
        rnd = random.Random(hash(meth) & 0xFFFF if False else len(meth) * 7919); nb = px = 0; first = None
        for t in range(T):
            try:
                bad, n = trial(meth, rnd)
            except Exception as e:
                bad, n = [('exc', repr(e))], 0
            px += n; nb += len(bad)
            if bad and first is None: first = bad[:2]
        print(meth, 'trials', T, 'pixels', px, 'mismatches', nb, 'first', first)
