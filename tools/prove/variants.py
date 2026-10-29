"""Deterministic scalar, argument, and register sweeps from a captured entry."""
import copy
import random
import struct

from .calls import CALL_INPUTS

# Integer boundaries and float bit patterns, including the 32-bit conversion
# overflow region where truncating to 64 bits differs from cvttss2si.
VALUES = [0, 1, 0xffffffff, 2, 0x7fffffff, 0x80000000, 0x7ffffffe, 0xfffffffe,
          0x3f800000, 0xbf800000, 0x3f000000, 0x80000001, 0x007fffff, 0x00800000,
          0x7f800000, 0xff800000, 0x7fc00000, 0x4f000000, 0xcf000000, 0x4f800000,
          0x5f000000, 0xdf000000, 0x7f7fffff, 0x00000001, 0x3f7fffff, 0x3f800001]


def typed_values(size):
    """Integer boundaries use the field width; eight-byte floats use binary64 bits."""
    if size == 8:
        return [0,1,0xffffffffffffffff,2,0x7fffffffffffffff,0x8000000000000000,
                0x7ffffffffffffffe,0xfffffffffffffffe,
                0x3ff0000000000000,0xbff0000000000000,0x3fe0000000000000,0x8000000000000001,
                0x000fffffffffffff,0x0010000000000000,0x7ff0000000000000,0xfff0000000000000,
                0x7ff8000000000000,0x41e0000000000000,0xc1e0000000000000,0x41f0000000000000,
                0x43e0000000000000,0xc3e0000000000000,0x7fefffffffffffff,1,
                0x3fefffffffffffff,0x3ff0000000000001]
    values = list(VALUES)
    limit = (1 << (size*8))-1
    values[2:8] = [limit,2,limit>>1,1 << (size*8-1),(limit>>1)-1,limit-1]
    return [v & limit for v in values]


def float80(bits):
    """Encode an IEEE binary32 value in the x87 extended format without rounding."""
    sign, exponent, fraction = bits >> 31, (bits >> 23) & 255, bits & 0x7fffff
    if exponent == 255:
        mant, exp = (1 << 63) | (fraction << 40), 0x7fff
        tag = 2
    elif exponent == 0 and fraction == 0:
        mant, exp, tag = 0, 0, 1
    else:
        sig, power = ((1 << 23) | fraction, exponent-127-23) if exponent else (fraction, -149)
        shift = 64 - sig.bit_length()
        mant, exp, tag = sig << shift, power + 63 - shift + 16383, 0
    return struct.pack('<QH', mant, exp | (sign << 15)), tag


def variants(rec, replay, reads, count, seed, start=0):
    rng = random.Random(seed ^ rec['function'])
    regs = rec['entry']['regs']
    regions = [(r[0], r[0]+r[1]) for r in rec['regions'] if r[2] == 0x1000]

    def pointer(value):
        return any(lo <= value < hi for lo, hi in regions)

    fields = []
    for (addr, n), data in sorted(reads.items()):
        if n == 4 and pointer(int.from_bytes(data, 'little')):
            continue
        if n == 8 and any(pointer(int.from_bytes(data[i:i+4], 'little')) for i in (0, 4)):
            continue
        fields.append(('memory', addr, n))
    nargs = CALL_INPUTS.entry_arity(rec) or 0
    targets = [('register', name, 4) for name in ('edi', 'esi', 'ebx', 'ebp', 'eax', 'edx', 'ecx')]
    top = (rec['entry']['fpu']['sw'] >> 11) & 7
    if (rec['entry']['fpu']['tw'] >> (top*2)) & 3 != 3:
        targets.append(('x87', 'st0', 4))
    # Put arguments before heap fields so small sweeps still hit float boundaries.
    targets += [('memory', regs['esp']+4+i*4, 4) for i in range(nargs)]
    targets += fields
    for i in range(start + count):
        # Sweep all typed boundaries at each target, then random single values.
        target = targets[(i // len(VALUES)) % len(targets)]
        kind, where, n = target
        v = typed_values(n)[i % len(VALUES)] if i < len(targets)*len(VALUES) else rng.getrandbits(n*8)
        v &= (1 << (n*8))-1
        if i < start:
            continue
        state = copy.deepcopy(rec['entry'])
        writes = [(where, v.to_bytes(n, 'little'))] if kind == 'memory' else []
        if kind == 'register':
            state['regs'][where] = v
        elif kind == 'x87':
            data, tag = float80(v)
            state['fpu']['st'] = data.hex() + state['fpu']['st'][20:]
            state['fpu']['tw'] = (state['fpu']['tw'] & ~(3 << (top*2))) | tag << (top*2)
        yield dict(id=i, kind=kind, target=f'{where:08x}' if kind == 'memory' else where,
                   size=n, value=f'{v:0{n*2}x}'), state, writes
