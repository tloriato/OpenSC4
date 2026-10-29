#!/usr/bin/env python3
"""Byte-weighted progress metric for FreeCity4 (RE side: it reads the exe's analysis; engine lanes never read it).

Every function of SimCity 4.exe (GOG 1.1.641) gets one owner by fixed rules, every owner one subsystem row of
tools/coverage-map.tsv, and every row two yes/no statuses (specified, implemented). The report sums function body
bytes and function counts per status, for the whole exe and per area, plus Reach (game code the corpus executes in the
original, from the fixed-schedule references' entry counters, or a census's) and Exact-replaced (docs/NEXT_STEPS.md
sections 1-3, from docs/proven.tsv and docs/native-checks.tsv). Same inputs, same numbers:
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
  docs/specs/*.md, engine/core/src/*        existence of the spec files and core modules the map names
  engine/plugin/FC4Subsystems.cpp           the enable keys and which are on by default
  build-runs/suite/<tag>/       a census's reach counters (--suite; default build-runs/suite/CURRENT)
  docs/proven.tsv, docs/native-checks.tsv   native proof and whole-game check evidence
  build-runs/prove/dlls/<sha256>.dll        the proven DLL's exported owner table
  build-runs/golden/credit.json (--references, also --credit): reference entry counters for Reach only

Output: docs/coverage-report.md (counts, class names, address ranges and statuses only; no code), and with --owners
a per-function TSV (address, bytes, owner, rule) for audits.

Usage: python tools/coverage.py [--suite <census tag>] [--out docs/coverage-report.md]
                                [--owners build-runs/coverage-owners.tsv] [--export <dir>] [--exe <path>]
                                [--references <manifest> | --reference-dll <sha256>]
Rules: docs/COVERAGE.md.
"""
import argparse
import bisect
import csv
import fnmatch
import hashlib
import importlib.util
import json
import os
import re
import struct
import subprocess
import sys
from collections import Counter, defaultdict
from pathlib import Path


def _sibling(name):
    """Load a module next to this file by path, writing no __pycache__: other tools exec this file from another tree,
    where neither `tools` nor this directory is on sys.path, and a `tools` package found on sys.path could belong to a
    different tree."""
    path = Path(__file__).with_name(name + ".py")
    module = importlib.util.module_from_spec(importlib.util.spec_from_file_location(name, path))
    exec(compile(path.read_text(encoding="utf-8"), str(path), "exec"), module.__dict__)
    return module


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
    def __init__(self, export, names_tsv, vtables_tsv, interfaces_tsv, libs_tsv, ranges_tsv, text_lo, text_hi,
                 noncode_refs_tsv):
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
        noncode_refs = set()
        if noncode_refs_tsv is not None:
            for r in read_tsv(noncode_refs_tsv):
                if not all(re.fullmatch(r"[0-9a-fA-F]{8}", r.get(k, "")) for k in ("from", "to")) \
                        or not r.get("evidence", "").strip():
                    sys.exit(f"coverage: {noncode_refs_tsv}: non-code reference needs two addresses and evidence")
                pair = (int(r["from"], 16), int(r["to"], 16))
                if pair in noncode_refs:
                    sys.exit(f"coverage: {noncode_refs_tsv}: duplicate non-code reference {pair}")
                noncode_refs.add(pair)
        seen_noncode_refs = set()
        slots = {}
        for r in read_tsv(export / "data-references.tsv"):
            try:
                frm, to = int(r["from"], 16), int(r["to"], 16)
            except ValueError:
                continue
            if (frm, to) in noncode_refs:
                seen_noncode_refs.add((frm, to))
                continue  # Proven data, not a function pointer; do not propagate an owner through it.
            if frm >= text_hi:
                slots[frm] = to
        if seen_noncode_refs != noncode_refs:
            sys.exit(f"coverage: {noncode_refs_tsv}: non-code reference absent from this export")
        self.edge_counts["non-code-data-references"] = len(seen_noncode_refs)
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


# ---------------------------------------------------------------- map, statuses

SHADOW_LABEL_ALIAS = {"clock24": "clock", "automata_owners": "automataowners"}  # the regress summary names a shadow by its log prefix


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


def map_statuses(rows, keys):
    """The two statuses of every map row, and any warnings.

    One place for the status rules: coverage.py's report and coverage_treemap.py both call this, so the two cannot
    disagree about what a row counts as (the treemap once recomputed them and drifted)."""
    warnings = []
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
        rp = r["replace"]
        if rp != "-" and rp not in keys:
            warnings.append(f"map row `{r['pattern']}`: replace key `{rp}` is not an enable key in FC4Subsystems.cpp")
    return warnings


def subsystem_keys(path):
    text = path.read_text(encoding="utf-8", errors="replace")
    keys = {}
    for m in re.finditer(r'\{"(\w+)",\s*(?:nullptr|"\w+"),\s*k\w+,[^{}]*?(true|false)\}', text, re.S):
        keys[m.group(1)] = m.group(2) == "true"
    return keys


# ---------------------------------------------------------------- report


def pct(a, b):
    return f"{100.0 * a / b:.1f}%" if b else "-"


def kib(n):
    return f"{n / 1024:,.1f}"


STAT = ("specified", "implemented")


def reference_evidence(path, reached):
    """Read original reference counters from a scenario manifest, solely for Reach."""
    data = json.loads(path.read_text(encoding="utf-8"))
    by, missing, files, unrecorded = defaultdict(set), [], [], []
    for name, entry in sorted(data["scenarios"].items()):
        if (entry.get("kind") != "draw" and entry["reference"] is None
                and "no golden baseline" in entry.get("detail", "")):
            unrecorded.append(name)  # no reference: not in the corpus (Reach comes from the references too)
            continue
        reference = path.parent / (entry["reference"] or "-") / "reach.tsv"
        if not reference.is_file():
            missing.append(f"{name} (reference)")
            continue
        files.append(reference)
        for va in reached(reference)[0]:
            by[va].add(name)
    return {"n": len(data["scenarios"]) - len(unrecorded), "by": by, "missing": missing, "files": files}


def stored_reference_evidence(store, dll_sha, reached):
    """Select a historical fixed-schedule corpus by its pinned reference DLL, without running the game."""
    if not re.fullmatch(r"[0-9a-f]{64}", dll_sha):
        raise ValueError("reference DLL needs its full sha256")
    by, files, missing, count = defaultdict(set), [], [], 0
    for path in sorted(store.glob("*/baseline.json")):
        data = json.loads(path.read_text(encoding="utf-8"))
        if data["inputs"]["files"].get("reference.dll", {}).get("sha256") != dll_sha:
            continue
        count += 1
        trace = path.with_name("reach.tsv")
        if not trace.is_file():
            missing.append(path.parent.name)
            continue
        files.extend((path, trace))
        for va in reached(trace)[0]:
            by[va].add(path.parent.name)
    if not count:
        raise ValueError(f"no stored references for DLL {dll_sha}")
    return {"n": count, "by": by, "missing": missing, "files": files}


def native_ports(path):
    """Read address -> owner from the exported FC4P v1 table of a PE32 DLL, without loading it."""
    raw = path.read_bytes()
    try:
        pe = struct.unpack_from("<I", raw, 0x3c)[0]
        machine, nsec = struct.unpack_from("<HH", raw, pe + 4)
        optsize = struct.unpack_from("<H", raw, pe + 20)[0]
        opt = pe + 24
        if raw[:2] != b"MZ" or raw[pe:pe + 4] != b"PE\0\0" or machine != 0x14c \
                or struct.unpack_from("<H", raw, opt)[0] != 0x10b:
            raise ValueError("expected a PE32 x86 DLL")
        base = struct.unpack_from("<I", raw, opt + 28)[0]
        headers = struct.unpack_from("<I", raw, opt + 60)[0]
        sections = [struct.unpack_from("<IIII", raw, opt + optsize + 40 * i + 8) for i in range(nsec)]

        def offset(rva, size=1):
            if 0 <= rva < headers and rva + size <= min(headers, len(raw)):
                return rva
            for _vsize, va, length, start in sections:
                if va <= rva and rva + size <= va + length and start + rva - va + size <= len(raw):
                    return start + rva - va
            raise ValueError(f"unmapped port table RVA {rva:x}")

        def uint(rva):
            return struct.unpack_from("<I", raw, offset(rva, 4))[0]

        def string(rva):
            chars = bytearray()
            for i in range(4096):
                c = raw[offset(rva + i)]
                if c == 0:
                    return chars.decode("utf-8")
                chars.append(c)
            raise ValueError("unterminated port table string")

        export, export_size = struct.unpack_from("<II", raw, opt + 96)
        offset(export, 40)
        count, names, ordinals = uint(export + 24), uint(export + 32), uint(export + 36)
        table = None
        for i in range(count):
            if string(uint(names + 4 * i)) == "fc4native_ports":
                ordinal = struct.unpack_from("<H", raw, offset(ordinals + 2 * i, 2))[0]
                if ordinal >= uint(export + 20):
                    raise ValueError("invalid port table export ordinal")
                table = uint(uint(export + 28) + 4 * ordinal)
                break
        if table is None or export <= table < export + export_size:
            raise ValueError("missing fc4native_ports data export")
        magic, version, count, size, entries = struct.unpack_from("<4sIIII", raw, offset(table, 20))
        if (magic, version, size) != (b"FC4P", 1, 16):
            raise ValueError("unsupported FC4P header")
        offset(entries - base, count * size)
        ports = {}
        for i in range(count):
            address, entry, owner, name = struct.unpack_from("<IIII", raw, offset(entries - base + i * size, size))
            owner, name = string(owner - base), string(name - base)
            if address in ports or not entry or not owner or not name:
                raise ValueError("duplicate or invalid port table entry")
            ports[address] = owner
        return ports
    except (struct.error, UnicodeError, IndexError) as exc:
        raise ValueError(f"{path}: malformed native DLL") from exc


def native_evidence(proven, checks, dll_dir, functions):
    """Join proofs to whole-game checks by the actual DLL hash and its port-table owner.

    functions is the denominator's address -> body-byte table. A function can earn its bytes only once.
    Each owner/DLL row shows the latest successful check, or the latest check if none succeeded.
    """
    proofs, checks = read_tsv(proven), read_tsv(checks)
    tables, files = {}, []
    for sha in sorted({r["dll_sha256"] for r in proofs + checks}):
        if not re.fullmatch(r"[0-9a-f]{64}", sha):
            raise ValueError(f"invalid native DLL sha256: {sha!r}")
        path = dll_dir / f"{sha}.dll"
        if sha256(path) != sha:
            raise ValueError(f"{path}: native DLL sha256 mismatch")
        tables[sha] = native_ports(path)
        files.append(path)
    port_counts = {sha: Counter(table.values()) for sha, table in tables.items()}
    per_owner, checked = {}, defaultdict(list)

    def owner_row(owner, sha):
        return per_owner.setdefault((owner, sha), {"exact": set(), "ports": port_counts[sha][owner],
                                                   "check": None, "bytes": 0})

    for r in checks:
        n, identical = int(r["scenarios"]), int(r["identical"])
        if not 0 <= identical <= n:
            raise ValueError("native check counts must satisfy 0 <= identical <= scenarios")
        key = (r["owner"], r["dll_sha256"])
        owner_row(*key)
        checked[key].append(r)
    for key, runs in checked.items():
        good = [r for r in runs if int(r["scenarios"]) > 0 and int(r["identical"]) == int(r["scenarios"])]
        per_owner[key]["check"] = max(good or runs, key=lambda r: r["checked"])
    credited, seen = set(), set()
    for r in proofs:
        va, sha = int(r["address"], 16), r["dll_sha256"]
        if va in seen:
            raise ValueError(f"duplicate proven function {va:08x}")
        seen.add(va)
        owner = tables[sha].get(va)
        if owner is None:
            continue
        row = owner_row(owner, sha)
        if r["verdict"] != "EXACT" or len(set(split_list(r["scenarios"]))) < 3:
            continue
        row["exact"].add(va)
        check = row["check"]
        if check and int(check["scenarios"]) > 0 and int(check["identical"]) == int(check["scenarios"]) \
                and va in functions:
            credited.add(va)
            row["bytes"] += functions[va]
    return {"credited": credited, "owners": per_owner, "files": files}


def attribution_for_tree(repo, export, text_lo, text_hi):
    """Expose the report's attribution inputs for another tree, without changing the scoring rules."""
    return Attribution(export, repo / "re/names_windows.tsv", repo / "docs/abi/vtables.tsv",
                       repo / "docs/abi/interfaces.tsv", repo / "tools/coverage-libs.tsv",
                       repo / "tools/coverage-ranges.tsv", text_lo, text_hi,
                       repo / "tools/coverage-noncode-refs.tsv")


def map_functions(rows, att, owner):
    """The report's method-first row assignment and byte rollup."""
    def match(o):
        for i, r in enumerate(rows):
            if not r["pattern"].startswith("m:") and fnmatch.fnmatchcase(o, r["pattern"]):
                return i
        return None

    method_rows = {r["pattern"][2:]: i for i, r in enumerate(rows) if r["pattern"].startswith("m:")}

    def method_row(va):
        if not method_rows:
            return None
        n = att.name(va)
        if not n:
            return None
        n = re.sub(r"\s+\(.*$", "", n[5:] if n.startswith("lib::") else n)
        return method_rows.get(strip_templates(n).replace("`", "").replace("'", ""))

    row_of = {}
    used_rows = Counter()
    fun_row = {}
    lib_b, lib_n = Counter(), Counter()
    game = []
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
            i = method_row(va)
            if i is None:
                if o not in row_of:
                    row_of[o] = match(o)
                i = row_of[o]
            used_rows[i] += 1
            game.append((va, size, i, rows[i]["area"]))
            fun_row[va] = rows[i]["subsystem"]
    return game, fun_row, lib_b, lib_n, used_rows


def load_report_data(export=None, exe=None):
    """Shared report inputs: attribution, map rows and their statuses."""
    export = find_input("ghidra/export/windows", export)
    exe = find_input("bin/SimCity4.exe", exe)
    exe_sha = sha256(exe)
    if exe_sha != EXE_SHA256:
        sys.exit(f"coverage: {exe} is not GOG 1.1.641 (sha256 {exe_sha})")
    summary = (export / "export-summary.txt").read_text(encoding="utf-8", errors="replace")
    if f"executable_sha256={EXE_SHA256}" not in summary:
        sys.exit(f"coverage: {export} was not exported from GOG 1.1.641")
    base, text_lo, text_hi = pe_text(exe)
    inputs = {
        "names": REPO / "re" / "names_windows.tsv",
        "vtables": REPO / "docs" / "abi" / "vtables.tsv",
        "interfaces": REPO / "docs" / "abi" / "interfaces.tsv",
        "libs": REPO / "tools" / "coverage-libs.tsv",
        "ranges": REPO / "tools" / "coverage-ranges.tsv",
        "noncode refs": REPO / "tools" / "coverage-noncode-refs.tsv",
        "map": REPO / "tools" / "coverage-map.tsv",
        "subsystems": REPO / "engine" / "plugin" / "FC4Subsystems.cpp",
    }
    att = attribution_for_tree(REPO, export, text_lo, text_hi)
    owner, rule = att.run()
    rows = read_map(inputs["map"])
    keys = subsystem_keys(inputs["subsystems"])
    warnings = map_statuses(rows, keys)
    game, fun_row, lib_b, lib_n, used_rows = map_functions(rows, att, owner)
    for i, r in enumerate(rows):
        if not used_rows[i]:
            warnings.append(f"map row `{r['pattern']}` matches no owner")

    return dict(export=export, exe_sha=exe_sha, base=base, text_lo=text_lo, text_hi=text_hi,
                inputs=inputs, att=att, owner=owner, rule=rule, rows=rows, keys=keys,
                warnings=warnings, game=game, fun_row=fun_row, lib_b=lib_b, lib_n=lib_n)


def replacement_code(data=None):
    """Original entry addresses by replace key, for save and draw scenario reach selection."""
    data = load_report_data() if data is None else data
    code = defaultdict(set)
    for va, _size, i, _area in data['game']:
        for key in split_list(data['rows'][i]['replace']) if i is not None else ():
            code[key].add(va)
    return dict(code)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--export", help="Ghidra export directory (default ghidra/export/windows)")
    ap.add_argument("--exe", help="SimCity 4.exe (default bin/SimCity4.exe)")
    ap.add_argument("--out", default=str(REPO / "docs" / "coverage-report.md"))
    ap.add_argument("--owners", help="also write a per-function TSV (address, bytes, owner, rule, subsystem)")
    ap.add_argument("--owner-scenarios", help="also write, per replace key, a greedy cover of the code the census "
                    "reaches: scenarios in order, each with the bytes it adds (tools/check.py selects by it)")
    current = REPO / "build-runs" / "suite" / "CURRENT"
    ap.add_argument("--suite", default=current.read_text(encoding="utf-8").strip() if current.is_file() else None,
                    help="census tag whose original entry counters give Reach without reference counters (default: "
                    "suite/CURRENT); --owner-scenarios uses it")
    default_references = next((p for root in (REPO, main_checkout(REPO))
                               if (p := root / "build-runs/golden/credit.json").is_file()), None)
    ap.add_argument("--references", "--credit", dest="references", default=default_references,
                    help="scenario manifest whose original reference counters give Reach only")
    ap.add_argument("--reference-dll", help="select stored original references by their pinned DLL's full sha256 "
                    "instead of the latest scenario manifest (Reach only)")
    args = ap.parse_args()
    data = load_report_data(args.export, args.exe)
    export, exe_sha = data["export"], data["exe_sha"]
    base, text_lo, text_hi = data["base"], data["text_lo"], data["text_hi"]
    inputs, att = data["inputs"], data["att"]
    owner, rule, rows = data["owner"], data["rule"], data["rows"]
    keys, warnings = data["keys"], data["warnings"]
    game, fun_row, lib_b, lib_n = data["game"], data["fun_row"], data["lib_b"], data["lib_n"]
    suite_evidence, reached, clean_runs = None, set(), 0
    if args.suite:
        suite_coverage = _sibling("suite_coverage")
        try:
            suite_evidence = suite_coverage.load(REPO, args.suite)
        except (ValueError, OSError, KeyError) as exc:
            ap.error(f"suite evidence: {exc}")
        for item in suite_evidence["scenarios"]:
            if suite_coverage.clean(item["row"]):
                reached |= item["reached"]
                clean_runs += 1
    reach_source = (f"`{args.suite}`, {clean_runs} clean of {len(suite_evidence['scenarios'])} census runs"
                    if suite_evidence else None)
    # Reference counters retain the existing Reach calculation independently of native proof credit.
    evidence = None
    if args.reference_dll or (args.references and Path(args.references).is_file()):
        try:
            read_reach = _sibling("suite_coverage").reached
            evidence = (stored_reference_evidence(find_input("build-runs/golden"), args.reference_dll, read_reach)
                        if args.reference_dll else reference_evidence(Path(args.references), read_reach))
        except (ValueError, OSError, KeyError) as exc:
            ap.error(f"reference evidence: {exc}")
        if not evidence["missing"]:
            reached = set(evidence["by"])
            reach_source = f"the fixed-schedule references of {evidence['n']} scenarios"
    inputs["proven"] = REPO / "docs/proven.tsv"
    inputs["native checks"] = REPO / "docs/native-checks.tsv"
    try:
        native = native_evidence(inputs["proven"], inputs["native checks"], find_input("build-runs/prove/dlls"),
                                 {va: size for va, size, _i, _area in game})
    except (ValueError, OSError, KeyError) as exc:
        ap.error(f"native evidence: {exc}")
    stats = STAT + (("reach",) if reach_source else ()) + ("exact",)

    total_b = sum(att.funcs.values())
    total_n = len(att.order)
    game_b = sum(x[1] for x in game)
    game_n = len(game)
    excl_b, excl_n = sum(lib_b.values()), sum(lib_n.values())

    def has(s, va, i):
        if s == "reach":
            return va in reached
        if s == "exact":
            return va in native["credited"]
        return i is not None and rows[i][s]

    def tally(items):
        t = {"n": 0, "b": 0, "impl_reach_b": 0}
        for s in stats:
            t[s + "_n"] = 0
            t[s + "_b"] = 0
        for va, size, i, _ in items:
            t["n"] += 1
            t["b"] += size
            for s in stats:
                if has(s, va, i):
                    t[s + "_n"] += 1
                    t[s + "_b"] += size
            if has("implemented", va, i) and va in reached:
                t["impl_reach_b"] += size
        return t

    whole = tally(game)
    by_area = {a: tally([g for g in game if g[3] == a]) for a in AREAS + ["shared", "unattributed"]}
    subs = defaultdict(list)
    owners_in = defaultdict(set)
    for g in game:
        if g[2] is not None:
            key = (rows[g[2]]["area"], rows[g[2]]["subsystem"])
            subs[key].append(g)
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
    unspecified = sum(size for va, size, i, _ in game if has("implemented", va, i) and not has("specified", va, i))
    w("# OpenSC4 coverage report")
    w("")
    command = "python tools/coverage.py" + (f" --reference-dll {args.reference_dll}" if args.reference_dll else "")
    w(f"Generated by `{command}`; the rules are in [COVERAGE.md](COVERAGE.md). Do not edit by hand: "
      "rerun the tool. Every number is a share of the machine code bytes of `SimCity 4.exe` (GOG 1.1.641), "
      "attributed to the class that owns each function by fixed rules; the inputs and their hashes are listed at the "
      "end. The picture of the same numbers is [coverage-map.svg](coverage-map.svg).")
    w("")
    w("## The funnel")
    w("")
    w(f"The denominator is the game's own code: **{game_b:,} bytes in {game_n:,} functions** (libraries and "
      "compiler-generated code excluded). The goal is 100% Exact-replaced.")
    w("")
    w("| Step | What it means | % of game code | Bytes | Functions | Out of order |")
    w("|---|---|---:|---:|---:|---|")
    w(f"| **Specified** | a clean-room spec in docs/specs describes the owning class | "
      f"**{pct(whole['specified_b'], game_b)}** | {whole['specified_b']:,} | {whole['specified_n']:,} | - |")
    gap = f"{pct(unspecified, game_b)} implemented without a spec" if unspecified else "-"
    w(f"| **Implemented** | our clean-room code in engine/ implements the owning class | "
      f"**{pct(whole['implemented_b'], game_b)}** | {whole['implemented_b']:,} | {whole['implemented_n']:,} | "
      f"{gap} |")
    meaning = ("the function is EXACT in prove from at least three distinct scenarios, and its port-table owner "
               "has a fully identical whole-game check on the same DLL")
    w(f"| **Exact-replaced** | {meaning} | **{pct(whole['exact_b'], game_b)}** | {whole['exact_b']:,} | "
      f"{whole['exact_n']:,} | - |")
    if reach_source:
        w(f"| Reach (the ceiling) | the corpus executes the code at all in the original ({reach_source}) | "
          f"**{pct(whole['reach_b'], game_b)}** | {whole['reach_b']:,} | {whole['reach_n']:,} | - |")
    else:
        w("| Reach (the ceiling) | the corpus executes the code at all in the original | **not measured** | - | - | "
          "no references with entry counters and no census (`--suite`) |")
    w("")
    w("Native evidence from `docs/proven.tsv` and `docs/native-checks.tsv`, joined by DLL hash and exported owner. "
      "One row per owner/DLL; the in-game column shows the latest successful check, or the latest check if none passed.")
    w("")
    w("| Owner | Functions EXACT (three scenarios) | Functions in the port table | In-game check (identical / run, DLL) | Bytes credited |")
    w("|---|---:|---:|---|---:|")
    for (key, sha), row in sorted(native["owners"].items()):
        check = row["check"]
        result = f"{check['identical']} / {check['scenarios']}" if check else "not checked"
        w(f"| {key} | {len(row['exact']):,} | {row['ports']:,} | {result}, {sha[:8]} | {row['bytes']:,} |")
    w("")
    if reach_source:
        w(f"Of the implemented owners' code, the corpus reaches {pct(whole['impl_reach_b'], whole['implemented_b'])} "
          f"({whole['impl_reach_b']:,} of {whole['implemented_b']:,} bytes), so Exact-replaced cannot exceed "
          f"{pct(whole['impl_reach_b'], game_b)} of game code until the corpus reaches more.")
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
    w("Share of each area's bytes at each step. Shared helpers and unattributed code have no owner, so they are never "
      "specified or implemented; Reach counts every function the corpus entered.")
    w("")
    cols = [("Specified", "specified"), ("Implemented", "implemented")]
    cols.append(("Exact-replaced", "exact"))
    if reach_source:
        cols.append(("Reach", "reach"))
    w("| Area | Functions | Bytes | % of game | " + " | ".join(c for c, _ in cols) + " |")
    w("|---|---:|---:|---:|" + "---:|" * len(cols))
    for a in AREAS + ["shared", "unattributed"]:
        t_ = by_area[a]
        w(f"| {a} | {t_['n']:,} | {t_['b']:,} | {pct(t_['b'], game_b)} | "
          + " | ".join(pct(t_[s + '_b'], t_['b']) for _, s in cols) + " |")
    w(f"| **all game code** | **{game_n:,}** | **{game_b:,}** | 100.0% | "
      + " | ".join(f"**{pct(whole[s + '_b'], game_b)}**" for _, s in cols) + " |")
    w("")
    w("## By subsystem")
    w("")
    w("One line per subsystem of `tools/coverage-map.tsv`, by area, then by size; each step is the share of the "
      "subsystem's bytes (function share in parentheses).")
    w("")
    w("| Area | Subsystem | Owners | Bytes | " + " | ".join(c for c, _ in cols) + " |")
    w("|---|---|---:|---:|" + "---:|" * len(cols))
    for key in sorted(subs, key=lambda k: (AREAS.index(k[0]), -sum(g[1] for g in subs[k]), k[1])):
        t_ = tally(subs[key])
        shares = [f"{pct(t_[s + '_b'], t_['b'])} ({pct(t_[s + '_n'], t_['n'])})" if t_[s + '_b'] else "-"
                  for _, s in cols]
        w(f"| {key[0]} | {key[1]} | {len(owners_in[key])} | {t_['b']:,} | " + " | ".join(shares) + " |")
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
    w("Text inputs are hashed with CRLF line endings normalised to LF (the text as git stores it); the exe and DLLs as their bytes.")
    w("")
    w("| Input | SHA-256 (first 16 hex) | Note |")
    w("|---|---|---|")
    w(f"| SimCity 4.exe | {exe_sha[:16]} | GOG 1.1.641, image base 0x{base:08X} |")
    for f in ("functions.tsv", "function-details.tsv", "calls.tsv", "data-references.tsv", "instructions.asm"):
        w(f"| ghidra/export/windows/{f} | {text_sha256(export / f)[:16]} | Ghidra 12.1.4 auto-analysis export |")
    for k in ("names", "vtables", "interfaces", "libs", "ranges", "noncode refs", "map", "subsystems", "proven", "native checks"):
        w(f"| {inputs[k].relative_to(REPO).as_posix()} | {text_sha256(inputs[k])[:16]} | |")
    for path in native["files"]:
        w(f"| build-runs/prove/dlls/{path.name} | {sha256(path)[:16]} | native port table |")
    if suite_evidence:
        reach_files = sorted(p for p in suite_evidence["inputs"] if p.name.endswith(".reach.tsv"))
        digest = hashlib.sha256("".join(text_sha256(p) for p in reach_files).encode()).hexdigest()
        w(f"| build-runs/suite/{args.suite}/*.reach.tsv | {digest[:16]} | {len(reach_files)} entry-counter files "
          "(hash of their hashes) |")
    if evidence:
        manifests = [] if args.reference_dll else [Path(args.references)]
        digest = hashlib.sha256("".join(text_sha256(p) for p in manifests + evidence["files"])
                                .encode()).hexdigest()
        if args.reference_dll:
            w(f"| build-runs/golden/*/baseline.json, reach.tsv | {digest[:16]} | Reach only, reference DLL "
              f"{args.reference_dll[:16]}, {evidence['n']} references (hash of metadata and counter hashes) |")
        else:
            w(f"| {Path(args.references).name} | {digest[:16]} | Reach only, with {len(evidence['files'])} reference "
              "entry-counter files (hash of their hashes) |")
    w("")
    w(f"On by default in FC4Subsystems.cpp: {', '.join(sorted(k for k, v in keys.items() if v))}.")
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
    if args.owner_scenarios:
        if not suite_evidence:
            ap.error("--owner-scenarios needs census reach (--suite)")
        code = {}
        for va, size, i, _area in game:
            for key in split_list(rows[i]["replace"]) if i is not None else ():
                code.setdefault(key, {})[va] = size
        # tools/check.py runs .script scenarios; Lua and clockless ones are left out of the cover.
        with open(REPO / "engine/tests/scenarios/suite.tsv", encoding="utf-8-sig", newline="") as f:
            skipped = {r["name"] for r in csv.DictReader(f, delimiter="\t")
                       if r["script"].lower().endswith(".lua") or "clockless" in r["sets"].split(",")}
        runs = [(item["name"], item["reached"]) for item in suite_evidence["scenarios"]
                if suite_coverage.clean(item["row"]) and item["name"] not in skipped]
        with open(args.owner_scenarios, "w", encoding="utf-8", newline="\n") as f:
            f.write(f"# census {args.suite}, clean runs without Lua or clockless cities; per key the scenarios of a greedy cover of its "
                    "reached code\n"
                    "key\tscenario\tadded_bytes\tcovered_bytes\treached_bytes\n")
            for key in sorted(code):
                left = {va for _, reached in runs for va in reached & code[key].keys()}
                total, covered = sum(code[key][va] for va in left), 0
                while left:
                    gain, name, hit = max((sum(code[key][va] for va in reached & left), name, reached & left)
                                          for name, reached in runs)
                    covered += gain
                    left -= hit
                    f.write(f"{key}\t{name}\t{gain}\t{covered}\t{total}\n")
    reach_text = (f"reach {pct(whole['reach_b'], game_b)} of game code, "
                  f"{pct(whole['impl_reach_b'], whole['implemented_b'])} of implemented ({reach_source})"
                  if reach_source else "reach not measured")
    exact_text = f"exact-replaced {pct(whole['exact_b'], game_b)}"
    print(f"coverage: game code {game_n} functions, {game_b} bytes; "
          + ", ".join(f"{s} {pct(whole[s + '_b'], game_b)}" for s in STAT)
          + f"; {exact_text}; {reach_text}; wrote {args.out}")


if __name__ == "__main__":
    main()
