"""Differential proof: run every cSpanRender8 RenderBlt slot (14 rows x 12 columns, table 0x00AC26B0) under
unicorn on random inputs and compare the destination with the model of span-render-8.md section 7.
usage: python span8_blt.py [via] [trials] [row,col ...]   (via: call through RenderBlt instead of the slot)   one line per slot: PASS n / FAIL first-mismatch."""
import sys, struct, random
import span8_emu as E
mu, DAT, STK, RET, TAB, rd, M = E.mu, E.DAT, E.STK, E.RET, E.TAB, E.rd, E.M
BPP = [2, 2, 2, 2, 4, 4, 1]
from span8_model import KEYM, KCM, model
# ---- harness ----------------------------------------------------------------------------------
def addr(mode, x0, x, y, u0, v0, du, dv, W, Hh):   # section 7.5 / 7.6
    if mode == 0: return x, y
    if mode == 1: return ((u0 + du * x) & M) >> 16, ((v0 + dv * y) & M) >> 16
    return (((x0 + u0) & M) % W + x - x0) % W, ((y + v0) & M) % Hh
def trial(r, c, rng):
    f, key, mode = r // 2, r & 1, c // 4
    bpp = BPP[f]; W, Hh = rng.randint(4, 40), rng.randint(1, 6); sstr = W * bpp + rng.choice((0, 4, 8))
    DW, DH, dstr = 64, 6, 64 + 8
    sp, dp, spans_a = DAT, DAT + 0x10000, DAT + 0x20000
    src = [rng.getrandbits(8 * bpp) for _ in range(W * Hh)]
    keyv = rng.getrandbits(32) & KEYM[f]
    if rng.random() < 0.1: keyv |= 1 << rng.randrange(KEYM[f].bit_length(), 32)   # key bits the caller would mask: never match
    if f == 6 and rng.random() < 0.5: keyv = 0
    for i in range(len(src)):
        q = rng.random()
        if q < 0.25: src[i] = (src[i] & ~KEYM[f] & ((1 << 8 * bpp) - 1)) | keyv
        elif q < 0.4: src[i] = keyv ^ (1 << rng.randrange(KEYM[f].bit_length()))   # near miss: one key bit off
        src[i] &= (1 << 8 * bpp) - 1
    sbuf = bytearray(sstr * Hh)
    for y in range(Hh):
        for x in range(W): sbuf[y * sstr + x * bpp:y * sstr + (x + 1) * bpp] = src[y * W + x].to_bytes(bpp, 'little')
    dst = [rng.getrandbits(8) for _ in range(DW * DH)]
    dbuf = bytearray(dstr * DH)
    for i, v in enumerate(dst): struct.pack_into('<B', dbuf, (i // DW) * dstr + (i % DW), v)
    col = rng.choice((rng.getrandbits(32), 0xFFFFFFFF, 0x80808080, 0x00FFFFFF, 0xFF000000 | rng.getrandbits(24)))
    u0 = v0 = du = dv = 0
    if mode == 1:
        du, dv = rng.randint(0x4000, 0x20000), rng.randint(0x4000, 0x20000); u0, v0 = rng.getrandbits(16), rng.getrandbits(16)
    elif mode == 2: u0, v0 = rng.choice(((rng.randrange(W), rng.randrange(Hh)), (rng.getrandbits(32), rng.getrandbits(32))))
    spans = []
    for _ in range(rng.randint(1, 4)):
        if mode == 1:
            y = rng.randrange(DH)
            while ((v0 + dv * y) >> 16) >= Hh: y -= 1
            if y < 0: continue
            x1 = 0
            while x1 < DW and ((u0 + du * x1) >> 16) < W: x1 += 1
            if x1 == 0: continue
        else:
            y = rng.randrange(DH if mode else Hh); x1 = DW if mode == 2 else W
        a = rng.randrange(x1); b = rng.randint(a, x1)
        spans.append((a, b, y))
    if not spans: return None
    mu.mem_write(sp, bytes(sbuf)); mu.mem_write(dp, bytes(dbuf))
    mu.mem_write(spans_a, b''.join(struct.pack('<II', (y << 16) | a, (y << 16) | b) for a, b, y in spans))
    args = struct.pack('<II', spans_a, len(spans)) + struct.pack('<6I', dp, dstr, DW, DH, 2, 1) + \
        struct.pack('<6I', sp, sstr, W, Hh, 0, bpp) + struct.pack('<8I', u0, v0, du, dv, f, col, keyv, key)
    if VIA:   # through RenderBlt 0x0085E750(spans, n, &dst, &src, &b, kind): records by pointer, dispatch on b
        mu.mem_write(DAT + 0x38000, args[8:])
        popped = E.call(0x0085E750, args[:8] + struct.pack('<4I', DAT + 0x38000, DAT + 0x38018, DAT + 0x38030, c), 4000000)
        if popped != 0x18: return 'RenderBlt popped %d' % popped
    else: E.call(TAB[r][c], args, 4000000)
    out = rd(dp, len(dbuf)); exp = list(dst); used = {}
    for a, b, y in spans:
        for x in range(a, b):
            sx, sy = addr(mode, a, x, y, u0, v0, du, dv, W, Hh)
            used[y * DW + x] = src[sy * W + sx]
            exp[y * DW + x] = model(f, key, c, src[sy * W + sx], exp[y * DW + x], col, keyv)
    for i, e in enumerate(exp):
        g = struct.unpack_from('<B', out, (i // DW) * dstr + (i % DW))[0]
        if g != e:
            return 'x=%d y=%d s=%x D=%02x col=%08x key=%x exp=%02x got=%02x' % (
                i % DW, i // DW, used.get(i, -1), dst[i], col, keyv, e, g)
    return ''
VIA = False
def main():
    global VIA
    if len(sys.argv) > 1 and sys.argv[1] == 'via': VIA = True; del sys.argv[1]
    nt = int(sys.argv[1]) if len(sys.argv) > 1 else 50
    sel = [tuple(map(int, a.split(','))) for a in sys.argv[2:]] or [(r, c) for r in range(14) for c in range(12)]
    npass = 0
    for r, c in sel:
        rng = random.Random(r * 100 + c); msg = 'PASS'; ok = 0
        for t in range(nt):
            try: res = trial(r, c, rng)
            except Exception as e: res = 'EMU %s' % e
            if res is None: continue
            if res: msg = 'FAIL ' + res; break
            ok += 1
        npass += msg == 'PASS'
        print('r%d c%d %08x %s %d' % (r, c, TAB[r][c], msg, ok))
    print('passed %d/%d' % (npass, len(sel)))
if __name__ == '__main__': main()
