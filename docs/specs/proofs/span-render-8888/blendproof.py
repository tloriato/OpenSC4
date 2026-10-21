"""Exhaustive check that the two packed blend forms of cSpanRender8888 equal the per-channel rule B(d, s, w).
Form P (RenderAlpha 0x00854F70, RenderAlphaGradient 0x00855040) and form X (helper 0x008576B0) are word-level
transcriptions of those instruction sequences (all mod 2^32, >> logical). The random execution runs of meth.py and
check.py confirm the transcriptions against the real bytes; this script covers every (s, d) channel pair and w.
Sweep: w = 0..256, every (s, d) pair of the tested channel, the other two channels over 12 x 4 fixed pairs (both signs of
s - d, the extremes, equal values); repeated with the tested channel R, G and B."""
import numpy as np
M = np.uint64(0xFFFFFFFF)
def P(d, s, w):
    rb = (((((s & 0xFF00FF) - (d & 0xFF00FF)) * w + 0x800080) & M) >> 8) + (d & 0xFF00FF) & M
    g = (((((s & 0xFF00) - (d & 0xFF00)) & M) >> 8) * w + (d & 0xFF00) + 0x80) & M
    return (rb | 0xFF00FF00) & (g | 0xFFFF00FF) & M
def X(d, s, w):
    a = (((s & 0xFF00) - (d & 0xFF00)) * w + (((d & 0xFFFFFF00) + 0x80) << 8)) & M
    b = (((s & 0xFFFF00FF) - (d & 0xFF00FF)) * w + ((d & 0xFF) << 8) + 0xFF800080) & M
    a = (a ^ b) & 0xFF0000
    c = ((((s & 0xFF00FF) - (d & 0xFF00FF)) * w) + ((d & 0xFFFF00FF) << 8) + 0x800080) & M
    return (((a ^ c) & M) >> 8) | 0xFF000000
def B(d, s, w):
    o = np.full(d.shape, 0xFF000000, dtype=np.int64)
    for sh in (16, 8, 0):
        sc = (s >> sh) & 255; dc = (d >> sh) & 255
        o |= ((256 * dc + w * (sc - dc) + 128) // 256) << sh
    return o
others = [(0, 0), (255, 255), (0, 255), (255, 0), (1, 0), (0, 1), (128, 127), (127, 128), (200, 3), (3, 200),
          (17, 17), (90, 91), (91, 90), (254, 255), (255, 254), (64, 192), (192, 64), (10, 250), (250, 10),
          (100, 0), (0, 100), (33, 66), (66, 33), (128, 128)]
sv, dv = np.meshgrid(np.arange(256, dtype=np.int64), np.arange(256, dtype=np.int64)); sv = sv.ravel(); dv = dv.ravel()
bad = {'P': 0, 'X': 0}; n = 0
for ch in (16, 8, 0):
    rest = [k for k in (16, 8, 0) if k != ch]
    for (s1, d1) in others[:12]:
        for (s2, d2) in others[::6]:
            sa = (sv << ch) | (s1 << rest[0]) | (s2 << rest[1]) | (np.int64(0x5A) << 24)
            da = (dv << ch) | (d1 << rest[0]) | (d2 << rest[1]) | (np.int64(0xC3) << 24)
            su = sa.astype(np.uint64); du = da.astype(np.uint64)
            for w in range(257):
                ref = B(da, sa, w).astype(np.uint64); wu = np.uint64(w)
                bad['P'] += int(np.count_nonzero(P(du, su, wu) != ref))
                bad['X'] += int(np.count_nonzero(X(du, su, wu) != ref))
                n += len(sv)
print('cases', n, 'mismatches', bad)
