"""Texel sweep: every RenderBlt slot of cSpanRender8 run on a 256x256 source holding every 16-bit texel (all 256
for 8-bit, 65536 random ones plus alpha 0/255 extremes for 32-bit), identity addressing in all three modes
(stretch du=dv=0x10000, tiled origin 0), random destination; compared with span8_blt.model.
usage: python span8_sweep.py [colours-per-slot] [row,col ...]"""
import sys, struct, random
import span8_emu as E, span8_blt as B
mu, DAT, TAB, rd = E.mu, E.DAT, E.TAB, E.rd
SP, DP, SPANS = DAT + 0x100000, DAT + 0x200000, DAT + 0x30000
N = 256
def run(r, c, col, keyv, rng):
    f, key, mode = r // 2, r & 1, c // 4
    bpp = B.BPP[f]
    if bpp == 2: src = list(range(65536))
    elif bpp == 1: src = [i & 255 for i in range(65536)]
    else: src = [rng.choice((0, 255, rng.getrandbits(8))) << 24 | rng.getrandbits(24) for _ in range(65536)]
    if key:
        for i in range(0, 65536, 7): src[i] = (src[i] & ~B.KEYM[f] & ((1 << 8 * bpp) - 1)) | keyv
    sbuf = b''.join(v.to_bytes(bpp, 'little') for v in src)
    dst = [rng.getrandbits(8) for _ in range(N * N)]
    mu.mem_write(SP, sbuf); mu.mem_write(DP, bytes(dst))
    mu.mem_write(SPANS, b''.join(struct.pack('<II', y << 16, (y << 16) | N) for y in range(N)))
    du = 0x10000 if mode == 1 else 0
    args = struct.pack('<II', SPANS, N) + struct.pack('<6I', DP, N, N, N, 2, 1) + \
        struct.pack('<6I', SP, bpp * N, N, N, 0, bpp) + struct.pack('<8I', 0, 0, du, du, f, col, keyv, key)
    E.call(TAB[r][c], args, 200000000)
    out = rd(DP, N * N)
    for i in range(N * N):
        e = B.model(f, key, c, src[i], dst[i], col, keyv)
        if out[i] != e: return 'i=%d s=%x D=%02x col=%08x key=%x exp=%02x got=%02x' % (i, src[i], dst[i], col, keyv, e, out[i])
    return ''
def main():
    nc = int(sys.argv[1]) if len(sys.argv) > 1 else 2
    sel = [tuple(map(int, a.split(','))) for a in sys.argv[2:]] or [(r, c) for r in range(14) for c in range(12)]
    npass = 0
    for r, c in sel:
        rng = random.Random(7000 + r * 100 + c); msg = 'PASS'
        for k in range(nc):
            col = (0xFFFFFFFF, rng.getrandbits(32), 0x7F80FF01, rng.getrandbits(32))[k % 4]
            keyv = 0 if r // 2 == 6 and k % 2 else rng.getrandbits(32) & B.KEYM[r // 2]
            res = run(r, c, col, keyv, rng)
            if res: msg = 'FAIL ' + res; break
        npass += msg == 'PASS'
        print('r%d c%d %08x %s' % (r, c, TAB[r][c], msg)); sys.stdout.flush()
    print('sweep passed %d/%d' % (npass, len(sel)))
if __name__ == '__main__': main()
