"""Paths, facts about the executable (function names, ranges, owners, call conventions, call sites) and the emulator
import."""
import bisect
import csv
import json
import os
import re
import sys
from functools import lru_cache
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
OUT = REPO / 'build-runs' / 'prove'
CAPTURES = OUT / 'captures'
STORE_DB = CAPTURES / 'pages.db'
CACHE = OUT / 'cache'
EXPORT = REPO / 'ghidra' / 'export' / 'windows'
OWNERS = REPO / 'build-runs' / 'coverage-owners.tsv'
ABI = REPO / 'docs' / 'abi' / 'functions.tsv'
PROVEN = REPO / 'docs' / 'proven.tsv'
GAME_EXE = Path(r'C:\Program Files (x86)\GOG Galaxy\Games\SimCity 4 Deluxe Edition\Apps\SimCity 4.exe')
PAGE = 0x1000
EXE_LO, EXE_HI = 0x00400000, 0x00C00000      # the game image (SimCity 4.exe 1.1.641 maps at its preferred base)

# Compiler helpers: calls to them are left out of the callee trace (their effects on registers and memory are still
# compared). A port does the same work inline or with its own copy. Listed by address because some carry wrong names
# in the export (0x009EEF04 is the float-to-integer helper the game calls after every FLD that becomes an int).
HELPER_ADDRS = {
    0x009EEF04: '_ftol2', 0x009F17B4: '__ftol', 0x009F0D10: '__allmul', 0x009F0710: '__aulldiv',
    0x009F0FC0: '__allshr', 0x009F6490: '__allshl', 0x009F64B0: '__aullshr', 0x009EF890: '__chkstk',
    0x009F1870: '_memset', 0x00407990: '_memset', 0x009EEB70: '_memmove',
}
HELPER_NAME = re.compile(r'^(lib::)?_+(ftol\w*|chkstk|alloca_probe|allmul|alldiv|allrem|aulldiv|aullrem|allshl|allshr|'
                         r'aullshr|alldvrm|aulldvrm|memset|memcpy|memmove|CI(sin|cos|tan|asin|acos|atan|atan2|exp|log|'
                         r'log10|pow|sqrt|fmod|sinh|cosh|tanh))$')

X87_MNEMONICS = {'FSIN', 'FCOS', 'FPTAN', 'FPATAN', 'FYL2X', 'F2XM1', 'FSINCOS', 'FYL2XP1', 'FSCALE', 'FPREM', 'FPREM1'}


def need_unicorn():
    """Import unicorn 2.1.4: the installed package, else a private copy under build-runs/prove/pylib."""
    # Register writes from inside hooks (EIP, x87) must not restart the translation block: the hooks rely on it.
    os.environ.setdefault('UC_IGNORE_REG_BREAK', '1')
    try:
        import unicorn  # noqa: F401
    except ImportError:
        sys.path.insert(0, str(OUT / 'pylib'))
        try:
            import unicorn  # noqa: F401
        except ImportError:
            raise SystemExit('prove needs unicorn 2.1.4: python -m pip install unicorn==2.1.4 (see docs/prove.md)')
    import unicorn
    if unicorn.__version__ != '2.1.4':
        raise SystemExit(f'prove needs unicorn 2.1.4, found {unicorn.__version__}')
    return unicorn


def h8(a):
    return f'{a:08x}'


@lru_cache(None)
def functions():
    """address -> (name, [(lo, hi_exclusive), ...]) for every function of the export."""
    out = {}
    with open(EXPORT / 'function-details.tsv', encoding='utf-8', errors='replace') as f:
        for row in csv.DictReader(f, delimiter='\t'):
            try:
                a = int(row['address'], 16)
            except (TypeError, ValueError):
                continue
            ranges = []
            for part in filter(None, (row.get('ranges') or '').split(';')):
                lo, _, hi = part.partition('-')
                ranges.append((int(lo, 16), int(hi, 16) + 1))
            out[a] = (row['name'], ranges or [(a, a + 1)])
    return out


@lru_cache(None)
def owners():
    """address -> dict(bytes, owner, rule, subsystem) from tools/coverage.py --owners."""
    if not OWNERS.is_file():
        raise SystemExit(f'{OWNERS} is missing: run python tools/coverage.py --owners {OWNERS.relative_to(REPO)}')
    out = {}
    with open(OWNERS, encoding='utf-8') as f:
        for row in csv.DictReader(f, delimiter='\t'):
            out[int(row['address'], 16)] = dict(bytes=int(row['bytes'] or 0), owner=row['owner'], rule=row['rule'],
                                                 subsystem=row['subsystem'])
    return out


@lru_cache(None)
def abi():
    """address -> ABI row (convention, this, callee_pop, stack_arg_dwords, reg_args, ret, notes)."""
    out = {}
    with open(ABI, encoding='utf-8') as f:
        for row in csv.DictReader(f, delimiter='\t'):
            try:
                out[int(row['address'], 16)] = row
            except ValueError:
                pass
    return out


def name_of(a):
    if a in HELPER_ADDRS:
        return HELPER_ADDRS[a]
    f = functions().get(a)
    return f[0] if f else f'FUN_{a:08x}'


def owner_of(a):
    o = owners().get(a)
    return o['owner'] if o else ''


def ranges_of(a):
    f = functions().get(a)
    return f[1] if f else [(a, a + 1)]


def resolve(spec):
    """'cSC4LandValueSimulator,006bd540,landvalue' -> sorted function addresses (owners, subsystems or addresses)."""
    out = set()
    own = owners()
    for tok in filter(None, (t.strip() for t in spec.split(','))):
        t = tok.lower().removeprefix('0x')
        if 6 <= len(t) <= 8 and all(c in '0123456789abcdef' for c in t):
            out.add(int(t, 16)); continue
        hits = [a for a, o in own.items() if o['owner'] == tok or o['subsystem'] == tok]
        if not hits:
            raise SystemExit(f'{tok!r} is neither an address nor an owner/subsystem in {OWNERS.name}')
        out.update(hits)
    return sorted(out)


class Ranges:
    """A sorted set of [lo, hi) intervals with fast membership."""

    def __init__(self, ranges):
        merged = []
        for lo, hi in sorted(ranges):
            if merged and lo <= merged[-1][1]:
                merged[-1][1] = max(merged[-1][1], hi)
            else:
                merged.append([lo, hi])
        self.lo = [r[0] for r in merged]
        self.hi = [r[1] for r in merged]

    def __contains__(self, a):
        i = bisect.bisect_right(self.lo, a) - 1
        return i >= 0 and a < self.hi[i]

    def __iter__(self):
        return iter(zip(self.lo, self.hi))

    def overlaps(self, lo, hi):
        i = bisect.bisect_right(self.lo, hi - 1) - 1
        return i >= 0 and self.hi[i] > lo


@lru_cache(None)
def helpers():
    """Addresses of compiler helper entries (left out of the callee trace)."""
    out = set(HELPER_ADDRS)
    for a, (name, _) in functions().items():
        if HELPER_NAME.match(name) and a not in (0x009EEF04,):
            out.add(a)
    return frozenset(out)


@lru_cache(None)
def exe_sites():
    """Addresses in the exe of x87 transcendentals (run on the host CPU) and of RDTSC/RDTSCP (the clock input)."""
    src = EXPORT / 'instructions.asm'
    st = src.stat()
    CACHE.mkdir(parents=True, exist_ok=True)
    cache = CACHE / 'exe-sites.json'
    key = f'{st.st_size}:{int(st.st_mtime)}'
    if cache.is_file():
        d = json.loads(cache.read_text())
        if d.get('key') == key:
            return d['x87'], d['clock']
    x87, clock = [], []
    with open(src, encoding='utf-8', errors='replace') as f:
        for line in f:
            if line[0] == ';':
                continue
            parts = line.split('\t', 2)
            if len(parts) < 3:
                continue
            m = parts[2].split(None, 1)[0] if parts[2].strip() else ''
            if m in X87_MNEMONICS:
                x87.append(int(parts[0], 16))
            elif m in ('RDTSC', 'RDTSCP'):
                clock.append(int(parts[0], 16))
    cache.write_text(json.dumps(dict(key=key, x87=x87, clock=clock)))
    return x87, clock


class ExeImage:
    """The game executable's bytes by virtual address, read from the (read-only) install directory."""

    def __init__(self, path=GAME_EXE):
        import pefile
        pe = pefile.PE(str(path), fast_load=True)
        base = pe.OPTIONAL_HEADER.ImageBase
        img = bytearray(pe.OPTIONAL_HEADER.SizeOfImage)
        img[:pe.OPTIONAL_HEADER.SizeOfHeaders] = pe.get_data(0, pe.OPTIONAL_HEADER.SizeOfHeaders)
        for s in pe.sections:
            d = s.get_data()[:max(s.Misc_VirtualSize, 0) or len(s.get_data())]
            img[s.VirtualAddress:s.VirtualAddress + len(d)] = d
        self.base, self.img = base, bytes(img)

    def read(self, va, n):
        off = va - self.base
        if off < 0 or off + n > len(self.img):
            return None
        return self.img[off:off + n]


@lru_cache(None)
def exe():
    return ExeImage()


_code_cache = {}


def function_code(a):
    """[(address, bytes, text)] of the function's instructions from the export (cached per function)."""
    if a in _code_cache:
        return _code_cache[a]
    path = CACHE / 'code' / f'{a:08x}.tsv'
    if path.is_file():
        out = []
        for line in path.read_text().splitlines():
            x, b, t = line.split('\t', 2)
            out.append((int(x, 16), bytes.fromhex(b), t))
        _code_cache[a] = out
        return out
    return _load_code([a])[a]


def _load_code(addrs):
    """Read the instructions of several functions in one pass over the export and cache them."""
    fn = functions()
    want = {a: fn[a][1] for a in addrs if a in fn}
    spans = sorted((lo, hi, a) for a, rs in want.items() for lo, hi in rs)
    los = [s[0] for s in spans]
    out = {a: [] for a in addrs}
    if spans:
        lo_all, hi_all = spans[0][0], max(s[1] for s in spans)
        with open(EXPORT / 'instructions.asm', encoding='utf-8', errors='replace') as f:
            for line in f:
                if line[0] == ';':
                    continue
                va = int(line[:8], 16)
                if va < lo_all:
                    continue
                if va >= hi_all:
                    break
                i = bisect.bisect_right(los, va) - 1
                while i >= 0 and spans[i][0] <= va:
                    if va < spans[i][1]:
                        _, b, t = line.rstrip('\n').split('\t', 2)
                        out[spans[i][2]].append((va, bytes.fromhex(b), t))
                        break
                    i -= 1
    (CACHE / 'code').mkdir(parents=True, exist_ok=True)
    for a, insns in out.items():
        (CACHE / 'code' / f'{a:08x}.tsv').write_text(''.join(f'{x:08x}\t{b.hex()}\t{t}\n' for x, b, t in insns))
        _code_cache[a] = insns
    return out


def preload_code(addrs):
    missing = [a for a in addrs if a not in _code_cache and not (CACHE / 'code' / f'{a:08x}.tsv').is_file()]
    if missing:
        _load_code(missing)


@lru_cache(None)
def call_sites():
    """target -> [(site, kind)] for every direct CALL and every direct JMP from another function (tail call)."""
    cache = CACHE / 'call-sites.json'
    src = EXPORT / 'instructions.asm'
    key = f'{src.stat().st_size}:{int(src.stat().st_mtime)}'
    if cache.is_file():
        d = json.loads(cache.read_text())
        if d.get('key') == key:
            return {int(k, 16): [(s, k2) for s, k2 in v] for k, v in d['sites'].items()}
    fn = functions()
    entries = set(fn)
    starts = sorted((lo, a) for a, (_, rs) in fn.items() for lo, _ in rs)
    los = [s[0] for s in starts]
    out = {}
    pat = re.compile(r'^(CALL|JMP) 0x([0-9a-f]{8})$')
    with open(src, encoding='utf-8', errors='replace') as f:
        for line in f:
            if line[0] == ';':
                continue
            parts = line.rstrip('\n').split('\t', 2)
            if len(parts) < 3:
                continue
            m = pat.match(parts[2])
            if not m:
                continue
            t = int(m.group(2), 16)
            if t not in entries:
                continue
            site = int(parts[0], 16)
            if m.group(1) == 'JMP':
                i = bisect.bisect_right(los, site) - 1
                owner = starts[i][1] if i >= 0 else None
                if owner == t or (owner is not None and any(lo <= t < hi for lo, hi in fn[owner][1])):
                    continue
            out.setdefault(t, []).append((site, m.group(1).lower()))
    CACHE.mkdir(parents=True, exist_ok=True)
    cache.write_text(json.dumps(dict(key=key, sites={f'{k:08x}': v for k, v in out.items()})))
    return out


def scenario(name):
    """The suite row (region, city, script, end) of a scenario name."""
    with open(REPO / 'engine' / 'tests' / 'scenarios' / 'suite.tsv', encoding='utf-8') as f:
        for row in csv.DictReader(f, delimiter='\t'):
            if row['name'] == name:
                return row
    raise SystemExit(f'no scenario {name!r} in engine/tests/scenarios/suite.tsv')


def abi_args(target):
    """(register names, stack argument dwords) a callee takes, from the ABI table; None when unknown."""
    if target in (0x009eef04, 0x009f17b4):
        return [], 0  # ST0 input; the temporary integer lives in the callee's frame
    row = abi().get(target)
    if not row:
        return None
    regs = [r for r in (row.get('reg_args') or '').split(',') if r and r != 'none']
    n = row.get('stack_arg_dwords') or ''
    if not n.isdigit():
        m = re.search(r'body_stack_dwords=(\d+)', row.get('notes') or '')
        n = m.group(1) if m else '0'
    return regs, int(n)
