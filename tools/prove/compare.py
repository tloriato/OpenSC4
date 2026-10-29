"""Compare observable function results, with the dead part of the stack excluded."""
from .machine import st_of_ctx


def call_values(a, b):
    """Compare a pointer into either live frame by its frame role, not its address."""
    frames = (a['frame'], b['frame'])
    def value(v):
        return '<frame>' if any(lo <= v < hi for lo, hi in frames) else v
    def normal(call):
        return dict(target=call['target'], registers={n: value(v) for n, v in call['registers'].items()},
                    arguments=None if call['arguments'] is None else [value(v) for v in call['arguments']])
    return normal(a), normal(b)


def context_result(state):
    f = state['fpu']
    return dict(regs=state['regs'], eflags=state['regs']['eflags'],
                st=st_of_ctx(f), tag=f['tw'], top=(f['sw'] >> 11) & 7,
                cw=f['cw'], mxcsr=f['mxcsr'], err=None)


def difference(left, right, parts, a_pages, b_pages, initial, stack):
    if left.get('err') or right.get('err'):
        return dict(field='execution', original=left.get('err'), ours=right.get('err'))
    masks = {'al': ('eax', 255), 'ah': ('eax', 0xff00), 'eaxhi': ('eax', 0xffff0000),
             'edx': ('edx', 0xffffffff), 'ecx': ('ecx', 0xffffffff)}
    for name in ('ebx', 'esi', 'edi', 'ebp', 'esp'):
        x, y = left['regs'][name], right['regs'][name]
        if x != y:
            return dict(field=name, original=hex(x), ours=hex(y))
    for name in parts:
        reg, mask = masks[name]
        x, y = left['regs'][reg] & mask, right['regs'][reg] & mask
        if x != y:
            return dict(field=name, original=hex(x), ours=hex(y))
    for name in ('top', 'tag', 'st', 'cw'):
        if left[name] != right[name]:
            return dict(field=name, original=left[name], ours=right[name])
    for name, mask in (('eflags', 0x400), ('mxcsr', 0xffc0)):
        if left[name] & mask != right[name] & mask:
            return dict(field=name, original=hex(left[name] & mask), ours=hex(right[name] & mask))
    a, b = left.get('trace', []), right.get('trace', [])
    for n in range(max(len(a), len(b))):
        x, y = (a[n] if n < len(a) else None), (b[n] if n < len(b) else None)
        if x is not None and y is not None:
            if x['target'] == y['target'] and x['registers'] == y['registers'] and x['arguments'] == y['arguments']:
                continue
            normalized = call_values(x, y)
            if normalized[0] == normalized[1]:
                continue
            detail = dict(zip(('normalized_original', 'normalized_ours'), normalized))
        else:
            detail = {}
        return dict(field=f'call[{n}]', original=x, ours=y, **detail)
    for p in sorted(a_pages.keys() | b_pages.keys()):
        if stack[0] <= p and p+4096 <= min(stack[1], left['regs']['esp']):
            continue
        a, b = a_pages.get(p, initial.get(p)), b_pages.get(p, initial.get(p))
        if a == b:
            continue
        if a is None or b is None:
            return dict(field=f'page {p:08x}', original=a is not None, ours=b is not None)
        lo = max(0, min(4096, left['regs']['esp'] - p)) if stack[0] <= p < stack[1] else 0
        if a[lo:] != b[lo:]:
            i = next(i for i in range(lo, 4096) if a[i] != b[i])
            return dict(field=f'memory {p+i:08x}', original=f'{a[i]:02x}', ours=f'{b[i]:02x}')
    return None
