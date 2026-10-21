"""Overlap proof: every RenderBlt slot run with the source aliased onto the destination buffer (source pointer =
destination base + random byte offset, either sign, same stride), compared with a byte-level model that, pixel by
pixel in span order and x ascending, reads the texel from the current memory and then writes the result.
usage: python span8_alias.py [trials] [row,col ...]"""
import sys, struct, random
import span8_emu as E, span8_blt as B
mu, DAT, TAB, rd, M = E.mu, E.DAT, E.TAB, E.rd, E.M
DW, DH, DSTR = 64, 8, 72
def trial(r, c, rng):
    f, key, mode = r // 2, r & 1, c // 4
    bpp = B.BPP[f]; base = DAT + 0x10000; buf = bytearray(rng.getrandbits(8) for _ in range(DSTR * (DH + 8)))
    dp = base + 4 * DSTR                                    # destination rows 0..DH-1 start 4 rows into buf
    off = rng.randint(-2 * DSTR, 2 * DSTR); sp = dp + off   # source anywhere around it
    W, Hh = rng.randint(4, 24), rng.randint(1, 3)           # source reads stay inside buf (|off| <= 2 rows)
    keyv = rng.getrandbits(32) & B.KEYM[f]
    if f == 6 and rng.random() < 0.5: keyv = 0
    col = rng.choice((rng.getrandbits(32), 0xFFFFFFFF, 0x80808080))
    u0 = v0 = du = dv = 0
    if mode == 1: du, dv = rng.randint(0x4000, 0x20000), rng.randint(0x4000, 0x10000); u0 = v0 = 0
    elif mode == 2: u0, v0 = rng.randrange(W), rng.randrange(Hh)
    spans = []
    for _ in range(rng.randint(1, 4)):
        y = rng.randrange(Hh if mode == 0 else min(DH, 2)); x1 = W if mode == 0 else min(DW, 20)
        if mode == 1:
            while x1 and ((u0 + du * (x1 - 1)) >> 16) >= W: x1 -= 1
            if ((v0 + dv * y) >> 16) >= Hh or not x1: continue
        a = rng.randrange(x1); spans.append((a, rng.randint(a, x1), y))
    if not spans: return None
    mu.mem_write(base, bytes(buf)); sim = bytearray(buf)
    mu.mem_write(DAT + 0x20000, b''.join(struct.pack('<II', (y << 16) | a, (y << 16) | b) for a, b, y in spans))
    args = struct.pack('<II', DAT + 0x20000, len(spans)) + struct.pack('<6I', dp, DSTR, DW, DH, 2, 1) + \
        struct.pack('<6I', sp & M, DSTR, W, Hh, 0, bpp) + struct.pack('<8I', u0, v0, du, dv, f, col, keyv, key)
    E.call(TAB[r][c], args, 4000000)
    for a, b, y in spans:
        for x in range(a, b):
            sx, sy = B.addr(mode, a, x, y, u0, v0, du, dv, W, Hh)
            so = sp - base + sy * DSTR + sx * bpp; s = int.from_bytes(sim[so:so + bpp], 'little')
            do = dp - base + y * DSTR + x; D = sim[do]
            sim[do] = B.model(f, key, c, s, D, col, keyv)
    out = rd(base, len(buf))
    for i in range(len(buf)):
        if out[i] != sim[i]: return 'byte %d off=%d exp=%02x got=%02x' % (i, off, sim[i], out[i])
    return ''
def main():
    nt = int(sys.argv[1]) if len(sys.argv) > 1 else 50
    sel = [tuple(map(int, a.split(','))) for a in sys.argv[2:]] or [(r, c) for r in range(14) for c in range(12)]
    npass = 0
    for r, c in sel:
        rng = random.Random(9000 + r * 100 + c); msg = 'PASS'; ok = 0
        for t in range(nt):
            try: res = trial(r, c, rng)
            except Exception as e: res = 'EMU %s' % e
            if res is None: continue
            if res: msg = 'FAIL ' + res; break
            ok += 1
        npass += msg == 'PASS'
        print('r%d c%d %08x %s %d' % (r, c, TAB[r][c], msg, ok))
    print('alias passed %d/%d' % (npass, len(sel)))
if __name__ == '__main__': main()
