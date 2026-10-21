"""Read dwords from bin/SimCity4.exe by VA. usage: pe.py <va-hex> <count> [cols]"""
import struct, sys, os
D = open(os.environ.get('FC4_EXE', os.path.join(os.path.dirname(os.path.abspath(__file__)), '../../../../bin/SimCity4.exe')), 'rb').read()
pe = struct.unpack_from('<I', D, 0x3c)[0]
ns = struct.unpack_from('<H', D, pe + 6)[0]
osz = struct.unpack_from('<H', D, pe + 20)[0]
base = struct.unpack_from('<I', D, pe + 24 + 28)[0]
secs = []
for i in range(ns):
    o = pe + 24 + osz + 40 * i
    vs, va, rs, ra = struct.unpack_from('<IIII', D, o + 8)
    secs.append((va + base, vs, ra))
def off(v):
    for va, vs, ra in secs:
        if va <= v < va + vs:
            return v - va + ra
    raise ValueError(hex(v))
def dw(v):
    return struct.unpack_from('<I', D, off(v))[0]
def by(v, n):
    o = off(v); return D[o:o + n]
if __name__ == '__main__':
    va = int(sys.argv[1], 16); n = int(sys.argv[2]); cols = int(sys.argv[3]) if len(sys.argv) > 3 else 8
    vals = [dw(va + 4 * i) for i in range(n)]
    for r in range(0, n, cols):
        print('%08x: ' % (va + 4 * r) + ' '.join('%08x' % x for x in vals[r:r + cols]))
