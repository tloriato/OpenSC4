#!/usr/bin/env python3
"""Byte-weighted progress metric for FreeCity4 (RE side: it reads the exe's analysis; engine lanes never read it).

Every function of SimCity 4.exe (GOG 1.1.641) gets one owner by fixed rules, every owner one subsystem row of
tools/coverage-map.tsv, and every row four yes/no statuses (specified, implemented, verified, replaced). The report
sums function body bytes and function counts per status, for the whole exe and per area. Same inputs, same numbers:
nothing depends on dict order, time or the machine, and the report lists the SHA-256 of every input (text inputs
hashed with CRLF line endings normalised to LF, so a CRLF checkout and an LF checkout give the same report).

Inputs (all read-only):
  ghidra/export/windows/{functions,function-details,calls,data-references}.tsv, instructions.asm  Ghidra 12.1.4 export of
      the exe (tools/setup_ghidra.ps1 -Windows; gitignored, found in this checkout or in the main checkout)
  bin/SimCity4.exe              hash and .text bounds checked against the export
  re/names_windows.tsv          the committed function names (symport, manual, folded, FID)
  docs/abi/vtables.tsv          class vtables (slot targets)
  docs/abi/interfaces.tsv       interface vtables with the game class that installs them
  tools/coverage-libs.tsv       library / compiler buckets (excluded from the denominator)
  tools/coverage-ranges.tsv     owner ranges: game code whose functions have no name, owned by link order (with evidence)
  tools/coverage-map.tsv        owner pattern -> subsystem, area, spec, core module, shadow key, replace key
  tools/coverage-shadows.tsv    shadow results a lane run recorded outside the regress shadows list; each row is
                                checked against the run that records its counts, and is refused if they are not there
  docs/specs/*.md, engine/core/src/*        existence of the spec files and core modules the map names
  tools/regress.ps1             the regress replace list ($Replace) and shadows list
  engine/plugin/FC4Subsystems.cpp           the enable keys and which are on by default
  build-runs/regress-<tag>.out  the regress result read for "verified" and "replaced" (default: the PASS run of the
                                newest lane merged into HEAD, see newest_regress; --regress overrides)

Output: docs/coverage-report.md (counts, class names, address ranges and statuses only; no code), and with --owners
a per-function TSV (address, bytes, owner, rule) for audits.

Usage: python tools/coverage.py [--regress build-runs/regress-<tag>.out] [--out docs/coverage-report.md]
                                [--owners build-runs/coverage-owners.tsv] [--export <dir>] [--exe <path>]
Rules: docs/COVERAGE.md.
"""
import argparse
import bisect
import csv
import fnmatch
import hashlib
import os
import re
import struct
import subprocess
import sys
from collections import Counter, defaultdict
from pathlib import Path

EXE_SHA256 = "3bc5c7fe807fe5aa24a3abb06e40f13486670bc1150612c09baf4ecedafe1a4b"
AREAS = ["simulation", "framework", "renderer", "ui", "audio", "region", "other"]
SHARED = "<shared>"
HUB_REFS = 64  # a function referring to more distinct functions than this is a registration hub
REPO = Path(__file__).resolve().parents[1]

csv.field_size_limit(1 << 30)


# ---------------------------------------------------------------- inputs


def main_checkout(repo):
    """The main checkout of a worktree (where the gitignored analysis export lives)."""
    try:
        common = subprocess.run(["git", "-C", str(repo), "rev-parse", "--git-common-dir"], capture_output=True,
                                text=True, check=True).stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return repo
    p = Path(common)
    if not p.is_absolute():
        p = (repo / p).resolve()
    return p.parent


def find_input(rel, explicit=None):
    if explicit:
        return Path(explicit)
    for root in (REPO, main_checkout(REPO)):
        if (root / rel).exists():
            return root / rel
    sys.exit(f"coverage: {rel} not found in {REPO} or the main checkout")


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def text_sha256(path):
    """SHA-256 of a text file with CRLF normalised to LF (a UTF-16 file is hashed as its UTF-8 text), so a checkout
    with core.autocrlf=true and one without give the same hash: the hash of the text as git stores it."""
    with open(path, "rb") as f:
        head = f.read(2)
        if head in (b"\xff\xfe", b"\xfe\xff"):
            data = (head + f.read()).decode("utf-16").encode("utf-8")
            return hashlib.sha256(data.replace(b"\r\n", b"\n")).hexdigest()
        h = hashlib.sha256()
        carry = head
        for chunk in iter(lambda: f.read(1 << 20), b""):
            buf = carry + chunk
            carry = b"\r" if buf.endswith(b"\r") else b""  # a CRLF split across two chunks
            h.update((buf[:-1] if carry else buf).replace(b"\r\n", b"\n"))
        h.update(carry.replace(b"\r\n", b"\n"))
    return h.hexdigest()


def read_tsv(path):
    with open(path, encoding="utf-8", errors="replace", newline="") as f:
        lines = [l for l in f if l.strip() and not l.startswith("#")]
    return list(csv.DictReader(lines, delimiter="\t", quoting=csv.QUOTE_NONE))


def pe_text(path):
    """(image base, .text start VA, .text end VA) from the PE headers."""
    with open(path, "rb") as f:
        data = f.read(4096)
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    nsec = struct.unpack_from("<H", data, pe + 6)[0]
    optsize = struct.unpack_from("<H", data, pe + 20)[0]
    base = struct.unpack_from("<I", data, pe + 24 + 28)[0]
    sec = pe + 24 + optsize
    for i in range(nsec):
        name = data[sec + 40 * i: sec + 40 * i + 8].rstrip(b"\0")
        vsize, va = struct.unpack_from("<II", data, sec + 40 * i + 8)
        if name == b".text":
            return base, base + va, base + va + vsize
    sys.exit("coverage: no .text section")


# ---------------------------------------------------------------- names -> owners


def strip_templates(name):
    prev = None
    while prev != name:
        prev = name
        name = re.sub(r"<[^<>]*>", "", name)
    return name


MANGLED_THUNK = re.compile(r"^__ZThn\d+_N(\d+)([A-Za-z_]\w*)")


def owner_of_name(name):
    """The class (or namespace) part of a qualified name; 'fn:<name>' for a free function."""
    n = name
    if n.startswith("lib::"):
        n = n[5:]
    n = re.sub(r"\s+\(.*$", "", n)  # "X::f (folded generic setter ...)"
    m = MANGLED_THUNK.match(n)
    if m:  # Mac-mangled this-adjustor thunk: __ZThn8_N13cGZLua5Thread14QueryInterface...
        return m.group(2)[: int(m.group(1))]
    n = strip_templates(n)
    n = n.replace("`", "").replace("'", "")
    if "::" in n:
        cls = n.rsplit("::", 1)[0]
        if cls != "(anonymous_namespace)":  # a file-static helper is a free function
            return cls
        n = n.rsplit("::", 1)[1]
    return "fn:" + n


def folded_owner(note):
    """A folded (identical code) function: its owner if every folded name has the same one, else shared."""
    m = re.match(r"\d+ names: (.*)$", note or "")
    if not m:
        return SHARED
    owners = {owner_of_name(x.strip()) for x in m.group(1).split("|")}
    return owners.pop() if len(owners) == 1 else SHARED


# ---------------------------------------------------------------- attribution


class Attribution:
    def __init__(self, export, names_tsv, vtables_tsv, interfaces_tsv, libs_tsv, ranges_tsv, text_lo, text_hi):
        self.funcs = {}  # va -> size
        self.gname = {}  # va -> Ghidra name (non-default)
        self.thunk = set()
        for r in read_tsv(export / "functions.tsv"):
            if not re.fullmatch(r"[0-9a-fA-F]{8}", r["address"]):
                continue
            va = int(r["address"], 16)
            if not (text_lo <= va < text_hi):
                continue
            self.funcs[va] = int(r["body_bytes"])
            if r["thunk"] == "true":
                self.thunk.add(va)
            if not r["name"].startswith("FUN_"):
                self.gname[va] = r["name"]
        self.order = sorted(self.funcs)
        # function body ranges, for mapping a code address to its function
        rng = []
        for r in read_tsv(export / "function-details.tsv"):
            if not re.fullmatch(r"[0-9a-fA-F]{8}", r["address"]):
                continue
            va = int(r["address"], 16)
            if va not in self.funcs:
                continue
            for part in r["ranges"].split(";"):
                if "-" in part:
                    a, b = part.split("-")
                    rng.append((int(a, 16), int(b, 16), va))
        rng.sort()
        self.rng_lo = [x[0] for x in rng]
        self.rng = rng
        self.names = {}
        for r in read_tsv(names_tsv):
            va = int(r["va"], 16)
            if va in self.funcs:
                self.names[va] = r
        self.vt = defaultdict(set)
        self.vtable_starts = set()
        for r in read_tsv(vtables_tsv):
            if re.fullmatch(r"[0-9a-fA-F]{8}", r["vtable"] or ""):
                self.vtable_starts.add(int(r["vtable"], 16))
            if re.fullmatch(r"[0-9a-fA-F]{8}", r["target"] or ""):
                cls = re.sub(r"::vftable(_\d+)?$", "", r["class"])
                self.vt[int(r["target"], 16)].add(strip_templates(cls))
        for r in read_tsv(interfaces_tsv):  # curated interface vtables: game_class implements the slot
            cls = re.sub(r"\(.*\)$", "", (r["game_class"] or "").strip()).strip()
            if re.fullmatch(r"[0-9a-fA-F]{8}", r["vtable"] or ""):
                self.vtable_starts.add(int(r["vtable"], 16))
            if re.fullmatch(r"[A-Za-z_][\w:]*", cls) and re.fullmatch(r"[0-9a-fA-F]{8}", r["target"] or ""):
                self.vt[int(r["target"], 16)].add(strip_templates(cls))
        self.lib_ranges, self.lib_names = [], []
        for r in read_tsv(libs_tsv):
            if r["kind"] == "range":
                a, b = r["value"].split("-")
                self.lib_ranges.append((int(a, 16), int(b, 16), r["library"]))
            elif r["kind"] == "name":
                self.lib_names.append((re.compile(r["value"]), r["library"]))
            else:
                sys.exit(f"coverage: unknown kind {r['kind']!r} in {libs_tsv}")
        self.libraries = sorted({x[2] for x in self.lib_ranges} | {x[1] for x in self.lib_names})
        self.owner_ranges = []
        for r in read_tsv(ranges_tsv):
            a, b = r["range"].split("-")
            if not r["owner"] or not r["evidence"]:
                sys.exit(f"coverage: {ranges_tsv}: range {r['range']} needs an owner and its evidence")
            self.owner_ranges.append((int(a, 16), int(b, 16), r["owner"].strip()))
        # edges: direct calls, plus code that takes a function's address (callbacks, tables built in code)
        self.callees = defaultdict(set)  # direct calls
        self.refs = defaultdict(set)  # code references and data tables
        self.vrefs = defaultdict(set)  # a vtable (docs/abi/vtables.tsv) installed by code: its slots
        for r in read_tsv(export / "calls.tsv"):
            if not re.fullmatch(r"[0-9a-fA-F]{8}", r["target"] or ""):
                continue
            s, t = int(r["caller"], 16), int(r["target"], 16)
            if s in self.funcs and t in self.funcs and s != t:
                self.callees[s].add(t)
        self.edge_counts = Counter(calls=sum(len(v) for v in self.callees.values()))
        # code references: an instruction operand equal to a function entry (push offset callback, a tail jmp,
        # mov [this], vtable) is an edge from the containing function; operands into data are table heads
        data_heads = defaultdict(set)  # data address referenced by code -> referencing functions
        imm = re.compile(r"0x([0-9a-f]{6,8})\b")
        with open(export / "instructions.asm", encoding="utf-8", errors="replace") as f:
            for line in f:
                if line.startswith(";"):
                    continue
                tab = line.find("\t")
                tab2 = line.find("\t", tab + 1)
                if tab < 0 or tab2 < 0 or "0x" not in line:
                    continue
                for m in imm.finditer(line, tab2):
                    v = int(m.group(1), 16)
                    if v < text_lo:
                        continue
                    s = self.containing(int(line[:tab], 16))
                    if s is None:
                        continue
                    if v in self.funcs:
                        if v != s and v not in self.callees[s] and v not in self.refs[s]:
                            self.refs[s].add(v)
                            self.edge_counts["code-references"] += 1
                    elif v >= text_hi:
                        data_heads[v].add(s)
        # tables of function pointers in data: a slot belongs to a table whose head (an address code refers to) is
        # reached by walking back over contiguous pointer slots, 4 bytes at a time (at most 256); the functions
        # that refer to the head get an edge to the slot's function (vtables, command and Lua registration tables)
        slots = {}
        for r in read_tsv(export / "data-references.tsv"):
            try:
                frm, to = int(r["from"], 16), int(r["to"], 16)
            except ValueError:
                continue
            if frm >= text_hi:
                slots[frm] = to
        for frm in sorted(slots):
            to = slots[frm]
            if to not in self.funcs:
                continue
            q = frm
            for _ in range(256):
                if q in data_heads or q in self.vtable_starts:
                    break
                q -= 4
                if q not in slots:
                    q = None
                    break
            if q is None or q not in data_heads:
                continue
            vt = q in self.vtable_starts
            for s in sorted(data_heads[q]):
                if s != to and to not in self.callees[s] and to not in self.refs[s] and to not in self.vrefs[s]:
                    (self.vrefs if vt else self.refs)[s].add(to)
                    self.edge_counts["vtables" if vt else "data-tables"] += 1
        # a registration hub (a COM director's constructor lists every class factory; a window constructor installs
        # a 270-slot vtable) does not hand its owner on through reference edges; its direct calls still do
        self.hubs = {s for s, t in self.refs.items() if len(t) > HUB_REFS}
        self.edges = {s: sorted(self.callees[s] | self.vrefs[s] | (set() if s in self.hubs else self.refs[s]))
                      for s in set(self.callees) | set(self.refs) | set(self.vrefs)}

    def containing(self, va):
        i = bisect.bisect_right(self.rng_lo, va) - 1
        for k in (i, i - 1):
            if k >= 0 and self.rng[k][0] <= va <= self.rng[k][1]:
                return self.rng[k][2]
        return None

    def name(self, va):
        if va in self.names:
            return self.names[va]["name"]
        return self.gname.get(va)

    def run(self):
        owner, rule = {}, {}
        self.range_overrides = Counter()
        # 1. library ranges (link order): override names
        for va in self.order:
            for a, b, lib in self.lib_ranges:
                if a <= va < b:
                    owner[va], rule[va] = "lib:" + lib, "library-range"
                    n = self.name(va)
                    if n and va in self.names and not self.names[va]["name"].startswith("lib::") \
                            and "::" in n and not re.match(r"(std|_STL|__gnu_cxx)::", n):
                        self.range_overrides[lib] += 1
                    break
        # 2. names
        for va in self.order:
            if va in owner:
                continue
            n = self.name(va)
            if not n or n.startswith("thunk_FUN_"):
                continue
            if re.match(r"(lib::)?(Catch@|switchD_)", n):
                continue  # funclets: step 6
            lib = next((l for rx, l in self.lib_names if rx.search(n)), None)
            if lib:
                owner[va], rule[va] = "lib:" + lib, "library-name"
                continue
            r = self.names.get(va)
            if r is not None and r["source"] == "folded":
                owner[va], rule[va] = folded_owner(r["note"]), "name-folded"
                continue
            conf = r["confidence"] if r is not None else "ghidra"
            owner[va], rule[va] = owner_of_name(n), "name-" + conf
        # 3. vtables
        for va in self.order:
            if va in owner or va not in self.vt:
                continue
            cls = self.vt[va]
            owner[va] = next(iter(cls)) if len(cls) == 1 else SHARED
            rule[va] = "vtable" if len(cls) == 1 else "vtable-shared"
        # 3b. owner ranges (link order, with evidence): unowned functions inside take the range's owner
        for va in self.order:
            if va in owner:
                continue
            for a, b, o in self.owner_ranges:
                if a <= va < b:
                    owner[va], rule[va] = o, "owner-range"
                    break
        # 4/5. reach (calls, code and table references) with a single owner, then the neighbour rule, repeated to a
        # fixpoint; only then does reach mark functions that several owners reach as shared (and repeat)
        while True:
            while self.reach(owner, rule, shared=False) + self.neighbours(owner, rule):
                pass
            if not self.reach(owner, rule, shared=True):
                break
        # 6. a free function (and what only it reaches) between two functions of one class joins that class's
        # object file
        cls_idx = [k for k, va in enumerate(self.order) if self._is_class(owner.get(va))]
        for k, va in enumerate(self.order):
            o = owner.get(va)
            if not (o and o.startswith("fn:")):
                continue
            p = bisect.bisect_left(cls_idx, k) - 1
            q = bisect.bisect_right(cls_idx, k)
            if p >= 0 and q < len(cls_idx):
                a, b = owner[self.order[cls_idx[p]]], owner[self.order[cls_idx[q]]]
                if a == b:
                    owner[va], rule[va] = a, "free-neighbour"
        # 7. funclets and thunks take their parent's / target's owner
        for va in self.order:
            if va in owner:
                continue
            n = self.name(va) or ""
            if va in self.thunk or n.startswith("thunk_FUN_"):
                m = re.match(r"thunk_FUN_([0-9a-f]{8})", n)
                t = int(m.group(1), 16) if m else next(iter(self.edges.get(va, ())), None)
                if t in owner:
                    owner[va], rule[va] = owner[t], "thunk"
                    continue
            if re.match(r"(lib::)?(Catch@|switchD_)", n):
                i = self.order.index(va) - 1
                while i >= 0 and re.match(r"(lib::)?(Catch@|switchD_)", self.name(self.order[i]) or ""):
                    i -= 1
                if i >= 0 and self.order[i] in owner:
                    owner[va], rule[va] = owner[self.order[i]], "funclet"
        for va in self.order:
            if va not in owner:
                owner[va], rule[va] = None, "unattributed"
        self.owner, self.rule = owner, rule
        return owner, rule

    def reach(self, owner, rule, shared):
        """Label propagation from owned functions to unowned callees: one game owner -> that owner, several ->
        shared; a library label is used only when no game owner reaches the function. Least fixpoint."""
        seeds = {va for va in self.order if va in owner}
        game, lib = {}, {}

        def join(cur, new):
            if cur is None or cur == new:
                return new
            return SHARED

        work = sorted(seeds)
        changed_set = set(work)
        while work:
            nxt = set()
            for s in work:
                o = owner.get(s) if s in seeds else None
                g = game.get(s) if s not in seeds else (o if o and not o.startswith("lib:") else None)
                l = lib.get(s) if s not in seeds else (o if o and o.startswith("lib:") and rule[s] == "library-range"
                                                        else None)
                for t in self.edges.get(s, ()):
                    if t in seeds:
                        continue
                    if g is not None:
                        ng = join(game.get(t), g)
                        if ng != game.get(t):
                            game[t] = ng
                            nxt.add(t)
                    if l is not None:
                        nl = join(lib.get(t), l)
                        if nl != lib.get(t):
                            lib[t] = nl
                            nxt.add(t)
            work = sorted(nxt)
        added = 0
        for va in self.order:
            if va in owner:
                continue
            if va in game:
                if game[va] == SHARED and not shared:
                    continue
                owner[va] = game[va]
                rule[va] = "reach" if game[va] != SHARED else "reach-shared"
                added += 1
            elif va in lib and lib[va] != SHARED:
                owner[va], rule[va] = lib[va], "reach-library"
                added += 1
        return added

    @staticmethod
    def _is_class(o):
        return bool(o) and o != SHARED and not o.startswith("lib:") and not o.startswith("fn:")

    def neighbours(self, owner, rule):
        """An unowned run of functions between two functions with the same (non-shared) owner takes that owner
        (link order: one object file's functions are contiguous)."""
        added = 0
        n = len(self.order)
        i = 0
        while i < n:
            if self.order[i] in owner:
                i += 1
                continue
            j = i
            while j < n and self.order[j] not in owner:
                j += 1
            if i > 0 and j < n:
                a, b = owner[self.order[i - 1]], owner[self.order[j]]
                if a == b and a != SHARED:
                    for k in range(i, j):
                        owner[self.order[k]], rule[self.order[k]] = a, "neighbour"
                        added += 1
            i = j
        return added


# ---------------------------------------------------------------- map, statuses, regress

SHADOW_LABEL_ALIAS = {"clock24": "clock"}  # the regress summary names a shadow by its log prefix


def read_map(path):
    rows = []
    seen = {}
    for i, r in enumerate(read_tsv(path)):
        if r["area"] not in AREAS:
            sys.exit(f"coverage: {path}: row {i + 1} ({r['pattern']}): unknown area {r['area']!r}")
        # A union merge can glue two rows onto one line or paste a block twice; the first match wins, so either
        # silently drops a row's statuses (train16 left 17 duplicates and one glued row).
        if r.get(None):
            sys.exit(f"coverage: {path}: row {i + 1} ({r['pattern']}): {len(r[None])} extra columns (two rows glued?)")
        if r["pattern"] in seen:
            sys.exit(f"coverage: {path}: row {i + 1}: pattern {r['pattern']!r} repeats row {seen[r['pattern']]}; "
                     "merge the two")
        seen[r["pattern"]] = i + 1
        rows.append({k: (r[k] or "-").strip() for k in ("pattern", "subsystem", "area", "spec", "core", "shadow",
                                                          "replace")})
    return rows


def split_list(v):
    return [] if v in ("", "-") else [x.strip() for x in v.split(",") if x.strip()]


def read_text_any(path):
    """Text of an input file: a UTF-16 file is read as its UTF-8 text, like text_sha256 hashes it."""
    raw = path.read_bytes()
    if raw[:2] in (b"\xff\xfe", b"\xfe\xff"):
        return raw.decode("utf-16")
    return raw.decode("utf-8", "replace")


def read_shadow_evidence(path, repo):
    """Shadow results a lane run recorded outside the regress shadows list: key -> (rows, exact), plus warnings.

    Used only for a shadow key the regress shadows list does not set; the house regress stays the primary source.
    A row counts only when its source file literally contains "<label> <clean>/<total>", so the counts are checked
    against the run that measured them and never taken on trust from this file. Any row that fails its check voids
    the whole key (fail closed): a shadow with one unproven compared set is not verified."""
    rows, warned = defaultdict(list), []
    for r in read_tsv(path):
        key, label = r["shadow"].strip(), r["label"].strip()
        try:
            clean, total = int(r["clean"]), int(r["total"])
        except ValueError:
            warned.append(f"shadow evidence `{key}`/{label}: clean and total must be integers; the key is void")
            rows.pop(key, None)
            rows[key] = None
            continue
        rel = r["source"].strip()
        src = repo / rel
        if not src.exists():
            warned.append(f"shadow evidence `{key}`/{label}: source `{rel}` does not exist; the key is void")
            rows[key] = None
            continue
        if f"{label} {clean}/{total}" not in read_text_any(src):
            warned.append(f"shadow evidence `{key}`/{label}: `{rel}` does not contain `{label} {clean}/{total}`; "
                          "the key is void")
            rows[key] = None
            continue
        if rows.get(key) is None and key in rows:
            continue  # already voided by an earlier row of the same key
        rows[key].append((label, clean, total))
    out = {}
    for k, v in rows.items():
        if v is None or not v:
            continue
        out[k] = (v, all(c == t and t > 0 for _, c, t in v))
    return out, warned


def map_statuses(rows, replace_list, shadow_list, keys, reg, shadow_ev, regress_name):
    """The four statuses of every map row, its shadow note, and any warnings; returns (warnings, replace run).

    One place for the status rules: coverage.py's report and coverage_treemap.py both call this, so the two cannot
    disagree about what a row counts as (the treemap once recomputed them and drifted)."""
    warnings = []
    rrun = reg["runs"].get("replace", ("none", 1))
    replace_clean = rrun[0] == "clean" and rrun[1] == 0
    if not replace_clean:
        warnings.append(f"the replace run of `{regress_name}` is not clean ({rrun[0]}): nothing counts as replaced")
    for r in rows:
        specs = split_list(r["spec"])
        cores = split_list(r["core"])
        r["specified"] = bool(specs) and all((REPO / "docs" / "specs" / s).exists() for s in specs)
        r["implemented"] = bool(cores) and all((REPO / "engine" / c).exists() for c in cores)
        for s in specs:
            if not (REPO / "docs" / "specs" / s).exists():
                warnings.append(f"map row `{r['pattern']}`: spec `{s}` does not exist")
        for c in cores:
            if not (REPO / "engine" / c).exists():
                warnings.append(f"map row `{r['pattern']}`: core path `engine/{c}` does not exist")
        sh = r["shadow"]
        r["verified"] = False
        r["shadow_note"] = "-"
        if sh != "-":
            label = sh[: -len("_shadow")] if sh.endswith("_shadow") else sh
            label = SHADOW_LABEL_ALIAS.get(label, label)
            if sh in shadow_list and label in reg["shadows"]:
                clean, lines = reg["shadows"][label]
                resynced = reg.get("resynced", {}).get(label)
                r["verified"] = lines > 0 and clean == lines and resynced == 0
                r["shadow_note"] = f"{clean}/{lines}"
                if resynced is None:
                    r["shadow_note"] += "; resync evidence missing"
                elif resynced:
                    r["shadow_note"] += f"; {resynced} resynced lines"
            elif sh in shadow_list:
                r["shadow_note"] = "no lines"
                warnings.append(f"shadow `{sh}` is in the regress list but `{regress_name}` has no `{label}` lines")
            elif sh in shadow_ev:
                sets, exact = shadow_ev[sh]
                r["verified"] = exact
                r["shadow_note"] = (", ".join(f"{lab} {c}/{t}" for lab, c, t in sets)
                                    + (" (lane run)" if exact else " (lane run, not exact)"))
            else:
                r["shadow_note"] = "not in regress"
        rp = r["replace"]
        r["replaced"] = False
        if rp != "-":
            if rp not in keys:
                warnings.append(f"map row `{r['pattern']}`: replace key `{rp}` is not an enable key in "
                                "FC4Subsystems.cpp")
            r["replaced"] = (rp in replace_list or keys.get(rp, False)) and replace_clean
    return warnings, rrun


def regress_lists(path):
    text = path.read_text(encoding="utf-8", errors="replace")
    m = re.search(r'\[string\]\$Replace\s*=\s*"([^"]*)"', text)
    replace = split_list(m.group(1)) if m else []
    m = re.search(r'^\s*shadows\s*=\s*"([^"]*)"', text, re.M)
    body = m.group(1).replace("`n", " ") if m else ""  # PowerShell newline escapes
    shadows = sorted(set(re.findall(r"(\w+_shadow)\s*=\s*1", body)))
    return replace, shadows


def subsystem_keys(path):
    text = path.read_text(encoding="utf-8", errors="replace")
    keys = {}
    for m in re.finditer(r'\{"(\w+)",\s*(?:nullptr|"\w+"),\s*k\w+,[^{}]*?(true|false)\}', text, re.S):
        keys[m.group(1)] = m.group(2) == "true"
    return keys


def read_regress(path, audit_legacy=True):
    raw = path.read_bytes()
    text = raw.decode("utf-16") if raw[:2] in (b"\xff\xfe", b"\xfe\xff") else raw.decode("utf-8", "replace")
    out = {"tag": None, "pass": False, "profile": "", "runs": {}, "shadows": {}, "resynced": {}}
    run = None
    for line in text.splitlines():
        line = line.rstrip()
        m = re.match(r"^REGRESS (\S+): (PASS|FAIL)", line)
        if m:
            out["tag"], out["pass"] = m.group(1), out["pass"] or m.group(2) == "PASS"
            continue
        if line.startswith("profile:") and not out["profile"]:
            out["profile"] = line[len("profile:"):].strip()
        m = re.match(r"^=== regress-.+-(shadows|replace) \(", line)
        if m:
            run = m.group(1)
            continue
        if line.startswith("=== regress "):
            run = None
        m = re.match(r"^(shadows|replace)\s+(\S+)\s+(\d+)\s+\d+", line)
        if m:
            out["runs"][m.group(1)] = (m.group(2), int(m.group(3)))
            continue
        m = re.match(r"^\s+([a-z0-9]+)\s+(\d+) lines, (\d+) with every mismatch count 0", line)
        if m and run == "shadows":
            out["shadows"][m.group(1)] = (int(m.group(3)), int(m.group(2)))
        m = re.match(r"^\s+([a-z0-9]+) resynced lines: (\d+)\s*$", line)
        if m and run == "shadows":
            label, count = m.group(1), int(m.group(2))
            out["resynced"][label] = max(count, out["resynced"].get(label, 0))
    # Older summaries omitted reset counters. Recover them only from the
    # corresponding recorded shadow log, with the same comparison-line filter
    # as run_summary.ps1. A truncated log cannot prove zero resets.
    missing = set(out["shadows"]) - set(out["resynced"])
    source = path.with_name(path.stem + "-shadows.log")
    if audit_legacy and missing and source.is_file():
        counts, resets = Counter(), Counter()
        comparison = re.compile(r"\] ([a-z]+) shadow [^:]*\d{4}-\d\d-\d\d[^:]*: .*"
                                r"(mismatch(es|ed)[= ]\d|: clean\b)")
        reset = re.compile(r"\bresynced:\s*(?!none\b|0\b)\S|\bresyncs?\s*[=:]\s*[1-9]\d*\b")
        with source.open(encoding="utf-8", errors="replace") as fh:
            for line in fh:
                match = comparison.search(line)
                if match and match.group(1) in missing:
                    label = match.group(1)
                    counts[label] += 1
                    resets[label] += bool(reset.search(line))
        for label in missing:
            if resets[label] or counts[label] == out["shadows"][label][1] > 0:
                out["resynced"][label] = resets[label]
        out["resync_source"] = source
    return out


def newest_regress():
    """The regress run of the newest lane merged into HEAD ("Merge <tag>: ..." subjects of every merge reachable from
    HEAD, in topological order), so the default depends on the checked-out commit, not on file times; runs of lanes
    not yet merged are ignored. Every reachable merge is walked, not only first-parent ones: a lane branch that merged
    main carries main's later merges only on its second parent, so a first-parent walk falls back to a run older than
    the code checked out. Falls back to the newest PASS file by modification time when no merged lane has one."""
    files = {}
    for root in (main_checkout(REPO), REPO):
        for p in (root / "build-runs").glob("regress-*.out"):
            files[p.name[len("regress-"):-len(".out")]] = p
    usable = {}
    for tag, p in files.items():
        try:
            r = read_regress(p, audit_legacy=False)
        except (OSError, UnicodeError):
            continue
        if r["pass"] and r["shadows"]:
            usable[tag] = p
    if not usable:
        sys.exit("coverage: no PASS regress-*.out found; pass --regress")
    try:
        log = subprocess.run(["git", "-C", str(REPO), "log", "--merges", "--topo-order", "--format=%s"],
                             capture_output=True, text=True, encoding="utf-8", check=True).stdout
    except (OSError, subprocess.CalledProcessError):
        log = ""
    for line in log.splitlines():
        # A merge-train batch records its regress under the batch tag ("Merge glue29 into integration/17
        # (merge-train batch train17)" -> regress-train17.out), not the lane's.
        b = re.search(r"merge-train batch (train\d+)", line)
        if b and b.group(1) in usable:
            return usable[b.group(1)]
        m = re.match(r"Merge (?:branch ')?([A-Za-z0-9_.-]+?)'?(?::| into |$)", line)
        if m and m.group(1) in usable:
            return usable[m.group(1)]
    return max((p.stat().st_mtime, p.name, p) for p in usable.values())[2]


# ---------------------------------------------------------------- report


def pct(a, b):
    return f"{100.0 * a / b:.1f}%" if b else "-"


def kib(n):
    return f"{n / 1024:,.1f}"


STAT = ("specified", "implemented", "verified", "replaced")


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--export", help="Ghidra export directory (default ghidra/export/windows)")
    ap.add_argument("--exe", help="SimCity 4.exe (default bin/SimCity4.exe)")
    ap.add_argument("--regress", help="regress-<tag>.out to read (default: the PASS run of the newest lane merged into HEAD)")
    ap.add_argument("--out", default=str(REPO / "docs" / "coverage-report.md"))
    ap.add_argument("--owners", help="also write a per-function TSV (address, bytes, owner, rule, subsystem)")
    args = ap.parse_args()

    export = find_input("ghidra/export/windows", args.export)
    exe = find_input("bin/SimCity4.exe", args.exe)
    exe_sha = sha256(exe)
    if exe_sha != EXE_SHA256:
        sys.exit(f"coverage: {exe} is not GOG 1.1.641 (sha256 {exe_sha})")
    summary = (export / "export-summary.txt").read_text(encoding="utf-8", errors="replace")
    if f"executable_sha256={EXE_SHA256}" not in summary:
        sys.exit(f"coverage: {export} was not exported from GOG 1.1.641")
    base, text_lo, text_hi = pe_text(exe)
    regress = Path(args.regress) if args.regress else newest_regress()
    inputs = {
        "names": REPO / "re" / "names_windows.tsv",
        "vtables": REPO / "docs" / "abi" / "vtables.tsv",
        "interfaces": REPO / "docs" / "abi" / "interfaces.tsv",
        "libs": REPO / "tools" / "coverage-libs.tsv",
        "ranges": REPO / "tools" / "coverage-ranges.tsv",
        "map": REPO / "tools" / "coverage-map.tsv",
        "shadows": REPO / "tools" / "coverage-shadows.tsv",
        "regress.ps1": REPO / "tools" / "regress.ps1",
        "subsystems": REPO / "engine" / "plugin" / "FC4Subsystems.cpp",
    }
    att = Attribution(export, inputs["names"], inputs["vtables"], inputs["interfaces"], inputs["libs"],
                      inputs["ranges"], text_lo, text_hi)
    owner, rule = att.run()
    rows = read_map(inputs["map"])
    replace_list, shadow_list = regress_lists(inputs["regress.ps1"])
    keys = subsystem_keys(inputs["subsystems"])
    reg = read_regress(regress)
    if reg.get("resync_source"):
        inputs["resync evidence"] = reg["resync_source"]
    shadow_ev, shadow_warn = read_shadow_evidence(inputs["shadows"], REPO)

    # statuses per map row
    status_warn, rrun = map_statuses(rows, replace_list, shadow_list, keys, reg, shadow_ev, regress.name)
    warnings = list(shadow_warn) + status_warn

    def match(o):
        for i, r in enumerate(rows):
            if fnmatch.fnmatchcase(o, r["pattern"]):
                return i
        return None

    # per-function rollup
    row_of = {}
    used_rows = Counter()
    fun_row = {}
    lib_b, lib_n = Counter(), Counter()
    game = []  # (va, size, map row index or None, area / bucket)
    for va in att.order:
        size, o = att.funcs[va], owner[va]
        if o is not None and o.startswith("lib:"):
            lib_b[o[4:]] += size
            lib_n[o[4:]] += 1
            fun_row[va] = "library " + o[4:]
        elif o is None:
            game.append((va, size, None, "unattributed"))
            fun_row[va] = "unattributed"
        elif o == SHARED:
            game.append((va, size, None, "shared"))
            fun_row[va] = "shared"
        else:
            if o not in row_of:
                row_of[o] = match(o)
            i = row_of[o]
            used_rows[i] += 1
            game.append((va, size, i, rows[i]["area"]))
            fun_row[va] = rows[i]["subsystem"]
    for i, r in enumerate(rows):
        if not used_rows[i]:
            warnings.append(f"map row `{r['pattern']}` matches no owner")

    total_b = sum(att.funcs.values())
    total_n = len(att.order)
    game_b = sum(x[1] for x in game)
    game_n = len(game)
    excl_b, excl_n = sum(lib_b.values()), sum(lib_n.values())

    def tally(items):
        t = {"n": 0, "b": 0}
        for s in STAT:
            t[s + "_n"] = 0
            t[s + "_b"] = 0
        for va, size, i, _ in items:
            t["n"] += 1
            t["b"] += size
            if i is None:
                continue
            for s in STAT:
                if rows[i][s]:
                    t[s + "_n"] += 1
                    t[s + "_b"] += size
        return t

    whole = tally(game)
    by_area = {a: tally([g for g in game if g[3] == a]) for a in AREAS + ["shared", "unattributed"]}
    subs = defaultdict(list)
    sub_rows = defaultdict(set)
    owners_in = defaultdict(set)
    for g in game:
        if g[2] is not None:
            key = (rows[g[2]]["area"], rows[g[2]]["subsystem"])
            subs[key].append(g)
            sub_rows[key].add(g[2])
            owners_in[key].add(owner[g[0]])

    rule_b, rule_n = Counter(), Counter()
    for va in att.order:
        rule_b[rule[va]] += att.funcs[va]
        rule_n[rule[va]] += 1

    # unattributed address ranges: runs of consecutive unattributed functions
    runs, cur = [], None
    for k, va in enumerate(att.order):
        if owner[va] is None:
            if cur and cur[4] == k - 1:
                cur[1] = va + att.funcs[va]
                cur[2] += att.funcs[va]
                cur[3] += 1
                cur[4] = k
            else:
                cur = [va, va + att.funcs[va], att.funcs[va], 1, k]
                runs.append(cur)
    runs.sort(key=lambda x: (-x[2], x[0]))

    # owners caught only by catch-all rows (map candidates)
    catch = {i for i, r in enumerate(rows) if r["subsystem"] in ("other", "other SC4 classes", "other free functions")}
    catch_b = Counter()
    for va, size, i, _ in game:
        if i in catch:
            catch_b[owner[va]] += size

    # ---- write
    L = []
    w = L.append
    w("# FreeCity4 coverage report")
    w("")
    w("Generated by `python tools/coverage.py`; the rules are in [COVERAGE.md](COVERAGE.md). Do not edit by hand: "
      "rerun the tool. Every number is a sum of function body bytes (or a count of functions) of `SimCity 4.exe` "
      "(GOG 1.1.641), attributed by fixed rules. Same inputs, same numbers; the inputs and their hashes are listed "
      "at the end.")
    w("")
    w("## Headline")
    w("")
    w(f"Game code, the denominator (every function outside the library and compiler buckets): **{game_n:,} "
      f"functions, {game_b:,} bytes**.")
    w("")
    w("| Status | Meaning | Bytes | % of game bytes | Functions | % of game functions |")
    w("|---|---|---:|---:|---:|---:|")
    for s, desc in (("specified", "a spec in docs/specs covers its owner"),
                    ("implemented", "a clean-room module in engine/ implements its owner"),
                    ("verified", "its same-process shadow is exact in the run that measured it"),
                    ("replaced", "our class runs instead of the original in the regress replace run")):
        w(f"| **{s}** | {desc} | {whole[s + '_b']:,} | **{pct(whole[s + '_b'], game_b)}** | {whole[s + '_n']:,} | "
          f"{pct(whole[s + '_n'], game_n)} |")
    w("")
    w("## Whole executable")
    w("")
    w("| Part | Functions | Bytes | % of .text |")
    w("|---|---:|---:|---:|")
    text_b = text_hi - text_lo
    w(f"| `.text` section 0x{text_lo:08X}-0x{text_hi:08X} | - | {text_b:,} | 100.0% |")
    w(f"| bytes in no function (padding, jump tables, data in code) | - | {text_b - total_b:,} | "
      f"{pct(text_b - total_b, text_b)} |")
    w(f"| all functions (Ghidra auto-analysis) | {total_n:,} | {total_b:,} | {pct(total_b, text_b)} |")
    w(f"| excluded: libraries and compiler-generated code | {excl_n:,} | {excl_b:,} | {pct(excl_b, text_b)} |")
    w(f"| **game code (the denominator)** | **{game_n:,}** | **{game_b:,}** | **{pct(game_b, text_b)}** |")
    w(f"| of which shared helpers (reached from several owners) | {by_area['shared']['n']:,} | "
      f"{by_area['shared']['b']:,} | {pct(by_area['shared']['b'], text_b)} |")
    w(f"| of which unattributed (no rule gives an owner) | {by_area['unattributed']['n']:,} | "
      f"{by_area['unattributed']['b']:,} | {pct(by_area['unattributed']['b'], text_b)} |")
    w("")
    w("## By area")
    w("")
    w("Each status cell is the share of the area's bytes, then (in brackets) of its functions. Shared and "
      "unattributed code has no owner, hence no status.")
    w("")
    w("| Area | Functions | Bytes | % of game | Specified | Implemented | Verified | Replaced |")
    w("|---|---:|---:|---:|---:|---:|---:|---:|")
    for a in AREAS + ["shared", "unattributed"]:
        t = by_area[a]
        cells = " | ".join(f"{pct(t[s + '_b'], t['b'])} ({pct(t[s + '_n'], t['n'])})" for s in STAT)
        w(f"| {a} | {t['n']:,} | {t['b']:,} | {pct(t['b'], game_b)} | {cells} |")
    cells = " | ".join(f"**{pct(whole[s + '_b'], game_b)}** ({pct(whole[s + '_n'], game_n)})" for s in STAT)
    w(f"| **all game code** | **{game_n:,}** | **{game_b:,}** | 100.0% | {cells} |")
    w("")
    w("## By subsystem")
    w("")
    w("One line per subsystem of `tools/coverage-map.tsv`, by area, then by size. Status columns: `yes` when every "
      "map row of the subsystem has the status, `part` when some do, `-` when none does. Shadow: clean/total lines "
      f"in `{regress.name}` (`not in regress` when the shadow exists but the regress shadows run does not use it), "
      "or the counts `tools/coverage-shadows.tsv` cites from a lane run (`lane run`, or `lane run, not exact` when "
      "a compared set differs).")
    w("")
    w("| Area | Subsystem | Owners | Functions | Bytes | Specified | Implemented | Shadow | Verified | Replaced |")
    w("|---|---|---:|---:|---:|---|---|---|---|---|")

    def flag(idx, s):
        vals = {rows[i][s] for i in idx}
        return "yes" if vals == {True} else "-" if vals == {False} else "part"

    for key in sorted(subs, key=lambda k: (AREAS.index(k[0]), -sum(g[1] for g in subs[k]), k[1])):
        items = subs[key]
        idx = sub_rows[key]
        notes = sorted({rows[i]["shadow_note"] for i in idx} - {"-"})
        w(f"| {key[0]} | {key[1]} | {len(owners_in[key])} | {len(items):,} | {sum(g[1] for g in items):,} | "
          f"{flag(idx, 'specified')} | {flag(idx, 'implemented')} | {', '.join(notes) or '-'} | "
          f"{flag(idx, 'verified')} | {flag(idx, 'replaced')} |")
    w("")
    w("## Excluded from the denominator")
    w("")
    w("| Bucket | Functions | Bytes |")
    w("|---|---:|---:|")
    for lib in sorted(lib_b, key=lambda k: (-lib_b[k], k)):
        w(f"| {lib} | {lib_n[lib]:,} | {lib_b[lib]:,} |")
    w(f"| **total** | **{excl_n:,}** | **{excl_b:,}** |")
    w("")
    if att.range_overrides:
        w("A library range overrides the names inside it. Game-looking names overridden this way (symport matches "
          "inside a library): " + ", ".join(f"{k} {v}" for k, v in sorted(att.range_overrides.items())) + ".")
        w("")
    w("## Attribution rules applied")
    w("")
    w("Which rule gave each function its owner (COVERAGE.md, applied in this order). Edges: "
      + ", ".join(f"{k} {v:,}" for k, v in sorted(att.edge_counts.items()))
      + f"; {len(att.hubs)} registration hubs (functions referring to more than {HUB_REFS} functions) pass no owner "
      "on through their references.")
    w("")
    w("| Rule | Functions | Bytes | % of all function bytes |")
    w("|---|---:|---:|---:|")
    for k in sorted(rule_b, key=lambda k: (-rule_b[k], k)):
        w(f"| {k} | {rule_n[k]:,} | {rule_b[k]:,} | {pct(rule_b[k], total_b)} |")
    w("")
    w("## Largest unattributed address ranges")
    w("")
    w("Runs of consecutive functions without an owner.")
    w("")
    w("| Range | Functions | Bytes |")
    w("|---|---:|---:|")
    for a, b, size, n, _ in runs[:20]:
        w(f"| 0x{a:08X}-0x{b:08X} | {n} | {size:,} |")
    w("")
    w("## Largest owners in the catch-all rows")
    w("")
    w("Owners that only a catch-all row of the map matches (candidates for a row of their own).")
    w("")
    w("| Owner | Bytes |")
    w("|---|---:|")
    for o, size in sorted(catch_b.items(), key=lambda x: (-x[1], x[0]))[:25]:
        w(f"| `{o}` | {size:,} |")
    w("")
    w("## Inputs")
    w("")
    w("Text inputs are hashed with CRLF line endings normalised to LF (the text as git stores it); the exe as its bytes.")
    w("")
    w("| Input | SHA-256 (first 16 hex) | Note |")
    w("|---|---|---|")
    w(f"| SimCity 4.exe | {exe_sha[:16]} | GOG 1.1.641, image base 0x{base:08X} |")
    for f in ("functions.tsv", "function-details.tsv", "calls.tsv", "data-references.tsv", "instructions.asm"):
        w(f"| ghidra/export/windows/{f} | {text_sha256(export / f)[:16]} | Ghidra 12.1.4 auto-analysis export |")
    for k in ("names", "vtables", "interfaces", "libs", "ranges", "map", "shadows", "regress.ps1", "subsystems"):
        w(f"| {inputs[k].relative_to(REPO).as_posix()} | {text_sha256(inputs[k])[:16]} | |")
    w(f"| build-runs/{regress.name} | {text_sha256(regress)[:16]} | regress {reg['tag']} "
      f"({'PASS' if reg['pass'] else 'FAIL'}; {reg['profile'] or 'profile not recorded'}); replace run "
      f"{rrun[0]} |")
    w("")
    w(f"Read from tools/regress.ps1: {len(replace_list)} replace keys, {len(shadow_list)} shadows. On by default in "
      f"FC4Subsystems.cpp: {', '.join(sorted(k for k, v in keys.items() if v))}.")
    w("")
    if warnings:
        w("## Warnings")
        w("")
        for x in sorted(set(warnings)):
            w(f"- {x}")
        w("")
    Path(args.out).write_text("\n".join(L), encoding="utf-8", newline="\n")
    if args.owners:
        with open(args.owners, "w", encoding="utf-8", newline="\n") as f:
            f.write("address\tbytes\towner\trule\tsubsystem\n")
            for va in att.order:
                f.write(f"{va:08x}\t{att.funcs[va]}\t{owner[va] or '-'}\t{rule[va]}\t{fun_row[va]}\n")
    print(f"coverage: game code {game_n} functions, {game_b} bytes; "
          + ", ".join(f"{s} {pct(whole[s + '_b'], game_b)}" for s in STAT) + f"; wrote {args.out}")


if __name__ == "__main__":
    main()
