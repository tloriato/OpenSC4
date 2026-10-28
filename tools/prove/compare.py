"""Compare observable function results, with the dead part of the stack excluded."""
from .machine import st_of_ctx


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
    if left.get('trace') != right.get('trace'):
        a, b = left.get('trace', []), right.get('trace', [])
        n = next((i for i, (x, y) in enumerate(zip(a, b)) if x != y), min(len(a), len(b)))
        return dict(field=f'call[{n}]', original=a[n] if n < len(a) else None,
                    ours=b[n] if n < len(b) else None)
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
