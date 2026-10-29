"""CPU anchors and explicit exceptions for kernel-maintained shared inputs."""
from .compare import context_result, difference

SHARED = 0x7ffe0000
# Offsets in the 32-bit KUSER_SHARED_DATA layout. These inputs can change
# even while every thread of the observed process is stopped.
KERNEL_FIELDS = (
    (0, 4, 'TickCountLowDeprecated'),
    (8, 32, 'InterruptTime/SystemTime'),
    (0x320, 0x330, 'TickCount'),
    (0x340, 0x358, 'TimeUpdateLock/BaselineQpc'),
    (0x36a, 0x36c, 'UnparkedProcessorCount'),
    (0x3b0, 0x3c0, 'InterruptTimeBias/QpcBias'),
)
KERNEL_BITS = ((0x2f0, 0x01, 'SharedDataFlags.DbgErrorPortPresent'),)


def normalize_kernel(actual, emulated, initial):
    actual, emulated, rules = dict(actual), dict(emulated), []
    if SHARED in initial:
        a = bytearray(actual.get(SHARED, initial[SHARED]))
        b = bytearray(emulated.get(SHARED, initial[SHARED]))
        for lo, hi, label in KERNEL_FIELDS:
            for i in range(lo, hi):
                if a[i] != b[i]:
                    rules.append(dict(rule='kernel maintained shared input', field=label,
                        address=f'{SHARED+i:08x}', cpu=a[i], replay=b[i]))
                a[i] = b[i] = initial[SHARED][i]
        for i, mask, label in KERNEL_BITS:
            if (a[i] ^ b[i]) & mask:
                rules.append(dict(rule='kernel maintained shared input', field=label,
                    address=f'{SHARED+i:08x}', cpu=a[i], replay=b[i], mask=mask))
            a[i] = (a[i] & ~mask) | (initial[SHARED][i] & mask)
            b[i] = (b[i] & ~mask) | (initial[SHARED][i] & mask)
        actual[SHARED], emulated[SHARED] = bytes(a), bytes(b)
    return actual, emulated, rules


def shared_page_audit(actual, emulated, initial):
    """List every other differing byte on this page; no implicit exceptions."""
    if SHARED not in initial:
        return []
    allowed = {i:255 for lo,hi,_ in KERNEL_FIELDS for i in range(lo,hi)}
    for i,mask,_ in KERNEL_BITS:
        allowed[i] = allowed.get(i,0) | mask
    a, b = actual.get(SHARED, initial[SHARED]), emulated.get(SHARED, initial[SHARED])
    return [dict(address=f'{SHARED+i:08x}', cpu=x, replay=y)
            for i,(x,y) in enumerate(zip(a,b)) if (x ^ y) & ~allowed.get(i,0)]


def cpu_difference(rec, original, changed, initial, actual):
    if not rec.get('exit'):
        return dict(field='CPU return was not captured'), []
    cpu = context_result(rec['exit'])
    cpu['trace'] = original.get('trace', [])
    actual, changed, rules = normalize_kernel(actual, changed, initial)
    diff = difference(cpu, original, ('al', 'ah', 'eaxhi', 'edx', 'ecx'),
                      actual, changed, initial, rec['stack'])
    return diff, rules


def repeat_difference(first, second):
    """Describe the first differing output, including non-live scratch outputs."""
    a, b = first[0], second[0]
    keys = ['err', 'kind', 'eip', 'regs', 'top', 'tag', 'st', 'cw', 'mxcsr', 'eflags']
    for key in keys + sorted((a.keys() | b.keys()) - set(keys)):
        if key != 'seconds' and a.get(key) != b.get(key):
            x, y = a.get(key), b.get(key)
            if isinstance(x, dict) and isinstance(y, dict):
                for sub in sorted(x.keys() | y.keys()):
                    if x.get(sub) != y.get(sub):
                        return dict(field=f'{key}.{sub}', original=x.get(sub), ours=y.get(sub))
            return dict(field=key, original=x, ours=y)
    for p in sorted(first[1].keys() | second[1].keys()):
        x, y = first[1].get(p), second[1].get(p)
        if x == y:
            continue
        if x is None or y is None:
            return dict(field=f'changed page {p:08x}', original=x is not None, ours=y is not None)
        i = next(i for i in range(len(x)) if x[i] != y[i])
        return dict(field=f'memory {p+i:08x}', original=f'{x[i]:02x}', ours=f'{y[i]:02x}')
    return None


class FrozenPages:
    """A second capture replay can read only the frozen entry, never CPU exit memory."""
    strict = True

    def __init__(self, pages):
        self.pages = dict(pages)

    def get(self, p):
        return self.pages.get(p)
