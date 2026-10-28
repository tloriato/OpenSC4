"""Which return registers a function's callers read: liveness of AL, AH, the upper half of EAX, EDX and ECX at every
return site.

A port must leave every register a caller reads exactly as the original does, and only those: a bool function whose
callers test AL may leave anything in the upper bytes of EAX, while a caller that tests all of EAX, saves EDX or
passes ECX on to its next call reads those too. The answer comes from the callers' code, not from labels: from each
return address the caller's instructions are followed (both ways at branches) until each register is written; a
register read before it is written is part of the channel. Where the walk cannot decide (an indirect jump, a return
that hands the value to an unknown caller, a long path), it counts the register as read.

The sites are every direct call or tail jump to the function in the executable plus every return address seen in a
capture (virtual calls have no static site; their captured return addresses do).
"""
import json
from functools import lru_cache

from .common import CACHE, EXE_LO, EXE_HI, exe, abi, call_sites, functions

PARTS = ('al', 'ah', 'eaxhi', 'edx', 'ecx')
ALL = frozenset(PARTS)
READ = {'al': {'al'}, 'ah': {'ah'}, 'ax': {'al', 'ah'}, 'eax': {'al', 'ah', 'eaxhi'},
        'dl': {'edx'}, 'dh': {'edx'}, 'dx': {'edx'}, 'edx': {'edx'},
        'cl': {'ecx'}, 'ch': {'ecx'}, 'cx': {'ecx'}, 'ecx': {'ecx'}}
KILL = {'al': {'al'}, 'ah': {'ah'}, 'ax': {'al', 'ah'}, 'eax': {'al', 'ah', 'eaxhi'}, 'edx': {'edx'}, 'ecx': {'ecx'}}
MAX_STEPS = 2000
MAX_PATH = 160

_md = None


def _disasm():
    global _md
    if _md is None:
        import capstone
        _md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)
        _md.detail = True
    return _md


def _insn(addr):
    code = exe().read(addr, 16)
    if code is None:
        return None
    return next(_disasm().disasm(code, addr, 1), None)


@lru_cache(None)
def _starts():
    import bisect
    spans = sorted((lo, hi, a) for a, (_, rs) in functions().items() for lo, hi in rs)
    return spans, [s[0] for s in spans]


def function_at(addr):
    import bisect
    spans, los = _starts()
    i = bisect.bisect_right(los, addr) - 1
    while i >= 0 and los[i] <= addr:
        if addr < spans[i][1]:
            return spans[i][2]
        i -= 1
    return None


def _ret_reads(addr, depth):
    """What a RET at addr hands on: the containing function's own callers read its return registers."""
    f = function_at(addr)
    if f is None:
        return set(ALL - {'ecx'})
    if depth < 2:
        sites = call_sites().get(f, [])
        if sites and len(sites) <= 40:
            out = set()
            for site, kind in sites:
                out |= set(site_reads(site, kind, depth + 1))
            return out
    ret = (abi().get(f, {}).get('ret') or '')
    return {'al', 'ah', 'eaxhi'} | ({'edx'} if 'edx' in ret else set())


def reads_after(ret_addr, depth=0):
    """Return-register parts read at ret_addr before they are written, as a sorted tuple.

    Direct calls are followed into the callee (link-time code generation lets a caller rely on a callee leaving EDX
    or ECX alone, and lets a callee take its arguments in any register); an indirect call is a virtual call with
    the standard convention: it reads ECX and leaves EAX, ECX and EDX undefined.
    """
    import capstone.x86_const as X
    work = [(ret_addr, ALL, 0, ())]
    seen = set()
    reads = set()
    steps = 0
    while work:
        addr, live, n, stack = work.pop()
        if not live or (addr, live, stack) in seen:
            continue
        seen.add((addr, live, stack))
        steps += 1
        if steps > MAX_STEPS or n > MAX_PATH or not (EXE_LO <= addr < EXE_HI):
            reads |= live
            continue
        insn = _insn(addr)
        if insn is None:
            reads |= live
            continue
        try:
            rr, ww = insn.regs_access()
        except Exception:
            reads |= live
            continue
        rn = {insn.reg_name(r) for r in rr}
        wn = {insn.reg_name(r) for r in ww}
        m = insn.mnemonic
        ops = insn.operands
        if m in ('xor', 'sub') and len(ops) == 2 and ops[0].type == X.X86_OP_REG and                 ops[1].type == X.X86_OP_REG and ops[0].reg == ops[1].reg:
            rn = set()                                   # zeroing idiom: a write, not a read
        for r in rn:
            reads |= READ.get(r, set()) & live
        kill = set()
        for r in wn:
            kill |= KILL.get(r, set())
        nlive = frozenset(live - kill)
        nxt = insn.address + insn.size
        if m == 'call':
            t = ops[0].imm & 0xFFFFFFFF if ops and ops[0].type == X.X86_OP_IMM else None
            if t is not None and EXE_LO <= t < EXE_HI and len(stack) < 4:
                work.append((t, nlive, n + 1, stack + (nxt,)))     # into the callee, back to nxt at its RET
            elif t is not None and EXE_LO <= t < EXE_HI:
                reads |= nlive                                     # too deep to follow
            else:
                reads |= {'ecx'} & nlive                           # virtual call: this in ECX, volatiles clobbered
            continue
        if m.startswith('ret'):
            if stack:
                work.append((stack[-1], nlive, n + 1, stack[:-1]))
            else:
                reads |= nlive & _ret_reads(addr, depth)
            continue
        if m == 'jmp':
            if ops and ops[0].type == X.X86_OP_IMM:
                work.append((ops[0].imm & 0xFFFFFFFF, nlive, n + 1, stack))
            else:
                reads |= nlive                           # indirect jump: unknown target
            continue
        if m.startswith('j') or m.startswith('loop'):
            if ops and ops[0].type == X.X86_OP_IMM:
                work.append((ops[0].imm & 0xFFFFFFFF, nlive, n + 1, stack))
            else:
                reads |= nlive
            work.append((nxt, nlive, n + 1, stack))
            continue
        if m in ('int3', 'hlt', 'ud2', 'int'):
            continue
        work.append((nxt, nlive, n + 1, stack))
    return tuple(p for p in PARTS if p in reads)


_site_cache = None


def _cache():
    global _site_cache
    if _site_cache is None:
        p = CACHE / 'return-reads.json'
        _site_cache = json.loads(p.read_text()) if p.is_file() else {}
    return _site_cache


def save_cache():
    if _site_cache is not None:
        CACHE.mkdir(parents=True, exist_ok=True)
        (CACHE / 'return-reads.json').write_text(json.dumps(_site_cache))


def site_reads(site, kind, depth=0):
    """Parts read after a static call site ('call') or tail jump ('jmp') to the function."""
    if kind == 'jmp':
        return tuple(sorted(_ret_reads(site, depth)))
    insn = _insn(site)
    if insn is None:
        return tuple(PARTS[:4])
    return return_reads(insn.address + insn.size, depth)


def return_reads(ret_addr, depth=0):
    if depth:
        return reads_after(ret_addr, depth)
    c = _cache()
    k = f'{ret_addr:08x}'
    if k not in c:
        c[k] = list(reads_after(ret_addr, 0))
    return tuple(c[k])


def channel(func, captured_returns=()):
    """(parts, evidence) for a function: union over its static call sites and the captured return addresses."""
    parts = set()
    static = call_sites().get(func, [])
    for site, kind in static:
        parts |= set(site_reads(site, kind))
    cap = sorted(set(captured_returns))
    for r in cap:
        if EXE_LO <= r < EXE_HI:
            parts |= set(return_reads(r))
        else:
            parts |= set(PARTS[:4])                      # a caller outside the exe: assume EDX:EAX
    if not static and not cap:
        parts = set(PARTS[:4])
    save_cache()
    return tuple(p for p in PARTS if p in parts), dict(static_sites=len(static), captured_sites=len(cap))


def describe(parts):
    if not parts:
        return 'none'
    names = []
    if {'al', 'ah', 'eaxhi'} <= set(parts):
        names.append('EAX')
    else:
        names += [p.upper() if p != 'eaxhi' else 'EAX[31:16]' for p in parts if p in ('al', 'ah', 'eaxhi')]
    names += [p.upper() for p in parts if p in ('edx', 'ecx')]
    return '+'.join(names)
