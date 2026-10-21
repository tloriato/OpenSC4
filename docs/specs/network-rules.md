# Network rules (behaviour spec)

Status: M5 behaviour spec, written 2026-10-16 (re-specs17b lane). Every fact was checked on the Windows executable
`bin/SimCity4.exe` (1.1.641, image base `0x00400000`); every address is a Windows address. The Mac build's symbol
names were used to name the Windows functions. Numeric details (bit layouts, table contents, operation order) were
checked against the instruction bytes where the pseudocode was unclear (`0x00622B50`, `0x006340D0`, `0x0063B340`,
`0x00640320`, `0x006094B0`, `0x00633A60`, `0x006335E0`). Implementers work from this spec, the public LGPL gzcom-dll SDK
headers and general knowledge only. They must **not** consult decompiled or disassembled code (see
[CONTRIBUTING.md](../../CONTRIBUTING.md)).

Amended 2026-10-16 (re-answers24): NR3–NR5 answered from the executable (§6.5, §8.3, §9.1, §10.8); the simulation
parts of NR1 and NR2 answered (§10.9, §11.1); NR6 and NR7 filed (§14).

Amended 2026-10-21 (re-netrules-boundary4): NR13 answered, the solver family's interoperability boundary (§2.4).

Tags:

- *[compat]*: observable; a replacement must match it.
- *[choice]*: a FreeCity4 decision.
- *[inferred]*: consistent with all the evidence, but not proven.
- *[quirk]*: original behaviour that looks unintended but is reproduced.

Scope. The **piece choice** half of the network tool's solve (network-geometry.md §8 names it in outline):

- the rule resources: their instances, their file grammars, and when they are loaded (§2);
- the **tile rule files** of the per-type tile evaluators, the `#Transmogrify` expansion into rotated and mirrored
  variants (§3, §4), and how a rule matches a cell (§5);
- `SolveNetwork`'s rule stages: the padded sets, `SolveUsingEvaluator` and `ApplyNetworkRules` (§6), and the
  one-way directions that the one-way road tool fixes before the solve (`ResolveOneWayConflicts`, §6.5);
- the **edge subtypes** (`AssignEdgeSubtypes`), which the rule files key on (§7);
- **multi-type intersections** (a cell with two network types) and the override matrix (§8), and the preview's
  per-type piece lookup (`SolveMultiTypeSeparation`, §8.3);
- the **tile subsets**: the `RuleOverrides` pair patches and the `TileVerifiers` pattern checks (§9);
- the **highway intersection rules** (`cIntRule`): their file grammar, `Test`, `DoAutoComplete`,
  `IdentifyHighwayIntersections`, `InsertIsolatedHighwayIntersection`, the ordering rings and the intersection pieces
  placed at commit (§10);
- what a solved cell's piece id selects, and how a network occupant gets its paths (§11).

It answers network-geometry.md NG5 and this spec's NR3–NR5, and the simulation parts of NR1 and NR2 (§12).

What is not here: the drag, the validation passes, the height solver and the commit (network-geometry.md,
network-tools.md); bridge and tunnel pieces (the evaluator instances `0x1000` and up, network-geometry.md §7.1); the
texture a piece id selects and how it is drawn (the renderer specs, §11); the path resource's format and the path-info
transforms (NR6); the consumer of the constraint layout (NR7). The shipped rule files are game data. This spec
describes their **format** and uses synthetic rules in its examples; it never reproduces the shipped rules.

Related specs (cited, not repeated):

- [network-geometry.md](network-geometry.md): the steps and cells (§3), the cell infos (§8.1), the solve outline (§8),
  the hash-set iteration order (§9.1).
- [network-tools.md](network-tools.md): `PlaceNetwork` (§5), the commit and `InsertNetworkOccupant` (§6.2),
  `PlaceNetworkOccupantById` (§4), the failure codes (§5.3).
- [network-manager.md](network-manager.md): network types (§3.1), network flags (§5), connection nodes (§7.7, §7.8).
- [occupant-manager.md](occupant-manager.md): the cell queries used by `cIntRule::Test`.

---

## 1. Vocabulary

### 1.1 Cells, sides, offsets

A **cell key** is `(z << 16) + x` (unsigned 32-bit). The four **sides** of a cell are numbered:

| Side | Name | Neighbour offset (dx, dz) |
|---|---|---|
| 0 | W | (−1, 0) |
| 1 | N | (0, −1) |
| 2 | E | (+1, 0) |
| 3 | S | (0, +1) |

The offset tables are `0x00AA89F8` (dx) and `0x00AA8A08` (dz). The opposite side of `s` is `(s + 2) & 3`.

### 1.2 Network types

Types 0–12: 0 Road, 1 Rail, 2 Highway, 3 Street, 4 Pipe, 5 PowerLine, 6 Avenue, 7 Subway, 8 LightRail, 9 Monorail,
10 OneWayRoad, 11 DirtRoad, 12 GroundHighway. The **type bit** of type `t` is `1 << t` (tables `0x00AA835C`,
`0x00AA89C4`, `0x00AA74F8`, `0x00AA8C10`, all equal). `0x1FFF` is the mask of all type bits. A cell info's flags
(`+0x08`, network-geometry.md §8.1) hold the type bits and other flags (`0x40000` bridge step, `0x80000` portal).

**Network names** (used by the rule files): the 13 names above, spelled `Road`, `Rail`, `Highway`, `Street`, `Pipe`,
`PowerLine`, `Avenue`, `Subway`, `LightRail`, `Monorail`, `OneWayRoad`, `DirtRoad`, `GroundHighway` (name table
`0x00AA845C`). `NetworkNameToID` (`0x0063C7A0`) compares case-insensitively after trimming. INI section names are
looked up lower-cased, so they are case-insensitive too.

**Elevated types**: Highway, LightRail, Monorail (bit mask `0x304`). **Street-level road types**: Road, Street,
Avenue, DirtRoad (bit mask `0x849`).

### 1.3 Edge words

Each cell info holds one **edge word** per type (`+0x18 + 4t`) and their union (`+0x4C`). Byte `i` of an edge word
(bits `8i`–`8i+7`) describes side `i`. So a word written in hex is `0xSSEENNWW`. In each byte:

| Bits | Meaning |
|---|---|
| 0–2 | the **kind** (below) |
| 3–7 | the **subtype** (or "minor type"), set by §7 or by rule files |

| Kind | Meaning |
|---|---|
| 0 | no connection |
| 2 | **orthogonal**: the network leaves through the middle of the side |
| 1 | **diagonal**, leaning to the corner that this side shares with side `(s + 1) & 3` |
| 3 | **diagonal**, leaning to the corner that this side shares with side `(s + 3) & 3` |
| 4 | the **inner side of a wide network** (avenue, highway): set by `AddGroupingInfo` (`0x006383A0`) on the sides between the two cells of a width-2 step |

So a diagonal through the north-west corner is W kind 1 plus N kind 3 (`0x00000301`), and a north–south straight is
`0x02000200`. A side "has a connection" when `byte & 3 ≠ 0`.

Helpers used below (all bytewise):

- `connmask(e)` = for each byte with bit 0 or bit 1 set, `0x03` in that byte, else `0x00`.
- `covers(eW, eL)` is true when `((eW ^ eL) & connmask(eL)) = 0`: on every side where `eL` has a connection, `eW` has
  the same low two bits.
- `D(e)` is an 8-bit mask: for each side `i` whose byte has `byte & 3 ≠ 0`, bits `2i` and `2i + 1` are set. (It is
  computed as byte 3 of `(((e & 0x03030303) + 0x03030303) & 0x04040404) × 0x00C30C30`.)
- `within(a, b)` (`0x00729EC0`) is true when, on every side where byte `a` is non-zero, the low nibble of `b` equals
  the low nibble of `a`. It is computed as `((((a + 0x0F0F0F0F) >> 4) & 0x01010101) × 0xF & (a ^ b)) = 0`, which is the
  rule above for bytes below `0x10` *[compat]*.

### 1.4 Rotation and flip of a piece

A piece's orientation is one **orientation byte**: bits 0–1 the rotation `r` (quarter turns), bit 7 the **flip**.
Rule files write it as two numbers, `rot` and `flip`; the byte is `rot | (flip ≠ 0 ? 0x80 : 0)`, with `rot` not
masked.

### 1.5 Metapositions

Rule conditions and outputs name a neighbour cell by a **metaposition** `m` (0–24), an offset (dx, dz) in the 5 × 5
square around the centre (tables `0x00B0B3D8` dx and `0x00B0B3F4` dz; the inverse grid `0x00B0B3BC`):

| m | (dx, dz) | m | (dx, dz) | m | (dx, dz) | m | (dx, dz) | m | (dx, dz) |
|---|---|---|---|---|---|---|---|---|---|
| 0 | (0, 0) | 5 | (1, 0) | 10 | (−2, −1) | 15 | (2, −2) | 20 | (1, 2) |
| 1 | (−1, 0) | 6 | (1, 1) | 11 | (−2, −2) | 16 | (2, −1) | 21 | (0, 2) |
| 2 | (−1, −1) | 7 | (0, 1) | 12 | (−1, −2) | 17 | (2, 0) | 22 | (−1, 2) |
| 3 | (0, −1) | 8 | (−1, 1) | 13 | (0, −2) | 18 | (2, 1) | 23 | (−2, 2) |
| 4 | (1, −1) | 9 | (−2, 0) | 14 | (1, −2) | 19 | (2, 2) | 24 | (−2, 1) |

Positions 1–8 are the inner ring and 9–24 the outer ring, each clockwise from the west.

### 1.6 Number readers

- `atol`: optional leading blanks and sign, then decimal digits; anything else ends the number (empty gives 0).
- `RZConvertToUint32A`: hex when the text starts with `0x`, else decimal.
- `RZAtoUint32HexA` (`0x0090FCEF`): leading blanks skipped, an optional `0x`/`0X`, then **at most 8** hex digits
  (either case); the first non-hex character ends it.

---

## 2. Resources and loading

### 2.1 The rule resources

All rule files are INI or text resources of type `0x0A5BCF4B`, group `0xAA5BCF57`. `ReloadRules` (`0x006253D0`) loads
them once per process, from the first network tool `Init` (`0x00626A0D`), in this order:

1. The network INI (`0x8A5971C5` / `0x8A5993B9`) → `ReloadNetworkTypeInfo` (`0x00623310`). For each type: clear the
   record, set its ids (§2.2), load its two **tile evaluators** (§2.3), its bridge types, supports and placement
   parameters (network-geometry.md §2), then `LoadComplete`. After all types: the **tile verifiers** from the
   section `[TileVerifiers]` of the same INI (§9.2).
2. Instance `0x10000001` → the **multi-type intersection solutions** and the `[AllowedOverrides]` matrix (§8.1).
3. Instance `0x10000002` → the **tile conflict rules**, section `[RuleOverrides]` (§9.1).
4. Instance `0x10000000` → the **highway intersection rules** (§10.1), into the global rule map `0x00B452B0`.

A file that is missing leaves its tables empty; every consumer treats an empty table as "nothing applies".

### 2.2 The network type records

Type `t`'s record is at `0x00B452C8 + t × 0x114` (network-geometry.md §2.3). The fields this spec uses:

| Offset | Meaning |
|---|---|
| `+0x00` | the **edge-subtype scheme** (§7): 1 for Road, Street, Pipe, Subway, OneWayRoad, DirtRoad; 2 for Rail, LightRail, Monorail; 0 for the others |
| `+0x0C` | the exemplar group of the type's **prebuilt-model pieces** (§11) |
| `+0x24` | the vector of tile evaluators, in load order, ended by a null entry or the vector's end |
| `+0xFC` | the **override matrix row**: bit `u` set when this type may override type `u` (§8.1) |

Prebuilt-piece exemplar groups (`+0x0C`, written by `ReloadNetworkTypeInfo`, e.g. Road at `0x0062333B`):

| Type | Group | Type | Group |
|---|---|---|---|
| 0 Road | `0x2821ED93` | 7 Subway | `0x8A15F3F2` |
| 1 Rail | `0xE8347989` | 8 LightRail | `0x2B79DFFB` |
| 2 Highway | `0xA8434037` | 9 Monorail | `0xEBE084C2` |
| 3 Street | `0xA92A02EA` | 10 OneWayRoad | `0xCBE084CB` |
| 4 Pipe | `0xA951171C` | 11 DirtRoad | `0x6BE08658` |
| 5 PowerLine | `0x088E1962` | 12 GroundHighway | `0xEBE084D1` |
| 6 Avenue | `0xCB730FAC` | | |

### 2.3 Tile evaluator instances

Each type loads two tile rule files (instances of type `0x0A5BCF4B`, group `0xAA5BCF57`), first then second
(key table `0x00B0B418`–`0x00B0B52C`). PowerLine loads none (its tool places pieces itself, network-tools.md §9).

| Type | Instances (load order) | Type | Instances (load order) |
|---|---|---|---|
| 0 Road | `0x8`, `0x7` | 7 Subway | `0xC`, `0xB` |
| 1 Rail | `0x6`, `0x5` | 8 LightRail | `0x10`, `0xF` |
| 2 Highway | `0x2`, `0x1` | 9 Monorail | `0x16`, `0x15` |
| 3 Street | `0xA`, `0x9` | 10 OneWayRoad | `0x12`, `0x11` |
| 4 Pipe | `0x4`, `0x3` | 11 DirtRoad | `0x14`, `0x13` |
| 6 Avenue | `0xE`, `0xD` | 12 GroundHighway | `0x18`, `0x17` |

An evaluator is created and pushed onto the type's vector even when its file is missing or rejected; it then has no
rules and never matches. In the shipped data, four files (Avenue's two, LightRail's `0xF`, Monorail's `0x15`) start
with `#Transmogrify` (§4); the others carry their variants written out.

### 2.4 Interoperability boundary (NR13)

The rule solver family has **no GZCOM identity and no outside entry point**. Nothing outside the executable can create,
reach or replace `cSC4TileEvaluator`, `cSC4TileRule`, `cSC4TileInfo` or `nSC4Networks::cIntRule` on its own; the
only supported places where a replacement can take over their behaviour are the hosts listed in §2.4.3.

#### 2.4.1 What the objects are

| Owner | Kind | Calling convention and shape | Created / destroyed |
|---|---|---|---|
| `cSC4TileEvaluator` | plain C++ object, 0x10 bytes | `+0x00` vtable `0x00AA8338`; `+0x04` the rule tree's header node (0x34 bytes from the global allocator `0x0090CF54`); `+0x08` the rule count; `+0x0C` not written by the constructor. The vtable has **one** slot, the scalar deleting destructor `0x00622AE0` (flag bit 0 = free); the dword before it (`0x00010203`) is not an RTTI locator, so there is no RTTI, no QueryInterface, no reference count. | Only `cSC4NetworkTypeInfo::LoadEvaluator` (`0x0063D360`, thiscall on the type record, pops 12) allocates it (`GlobalHeapAlloc(0x10)`, `0x0063D367`), runs the constructor `0x00622A90` (its only caller, `0x0063D375`), calls `AddRulesFromResource(key*, 0)` (`0x00623020`, thiscall, pops 8) and appends it to the record's vector `+0x24`, even on failure (§2.3). `cSC4NetworkTypeInfo::Clear` (`0x006421D0`) pops the vector from the back, calling slot 0 with 1. The vtable address is stored only by the constructor and destructor (`0x00622A95`, `0x00622AE9`; no other reference in the image). |
| `cSC4TileRule` | plain struct, 7 dwords, no vtable | the centre key and two 3-dword vectors (conditions, outputs); `Read(tokenizer*)` `0x00622870` is thiscall, pops 4 | built on `AddRulesFromResource`'s stack (zeroed at `0x0062308F`–`0x006230A7`), read, copied into the evaluator's tree; its only caller is `0x006230AB` |
| `cSC4TileInfo` | not an object: free functions over static tables | `ConvertMetaToRelativePos` `0x00620D80` is cdecl (plain `RET`) and reads the byte tables `0x00B0B3D8`/`0x00B0B3F4`; `GetRotationalEquivalent` `0x00621F30` and `…OfMetaPos` `0x00621680` pop 8 | no lifecycle |
| `nSC4Networks::cIntRule` | plain value class in the process-global hash map `0x00B452B0` (bucket vector `0x00B452B4`/`0x00B452B8`) | `Test` `0x00609A00` and `DoAutoComplete` `0x006097E0` thiscall, pop 12 | filled by `ReloadRules` (§2.1 step 4), destroyed by the CRT exit handler `0x00A7C170` |

`Evaluate` (`0x006220D0`) is a thiscall on the evaluator with two stack arguments (the neighbourhood and the output
list) and pops 8; its callers are `ApplyNetworkRules` (`0x006341EA`) and `SolveMultiTypeSeparation` (`0x0063401A`),
reached by a direct call, never through the vtable.

#### 2.4.2 Who reaches them (the complete caller set)

- **Loading.** `ReloadRules` (`0x006253D0`) has exactly two callers, `cSC4NetworkTool::Init` (`0x00626A0D`) and
  `cSC4PowerLineTool::StaticInit` (`0x006269B5`). Both run it only while the process flag `0x00B452A8` is clear, set
  it, and register `0x006232F0` with the application object (`0x00B43C94` slot 69, `+0x114`, the static-data cleanup
  list). That callback only clears `0x00B452A8`, so the next tool `Init` after the application's static-data cleanup
  reloads every rule table. The 13 type records `0x00B452C8 + t × 0x114` are image globals (constructed by the CRT
  initializer `0x00A73D10`, destroyed by `0x00A7C140`).
- **Solving.** The evaluators are read only through the type records: `SolveNetwork` (`0x00635CB0`) →
  `SolveUsingEvaluator` (`0x006358C0`) → `ApplyNetworkRules` (`0x006340D0`) → `Evaluate`; and
  `SolveMultiTypeSeparation` → `Evaluate`. `SolveNetwork`'s callers are `PlaceNetwork` (`0x0063BAFE`),
  `DestroyOccupantsAndResolve` (`0x0062B783`) and `InsertIsolatedHighwayIntersection` (`0x0062D2C2`), all methods of
  the network tool.
- **Highway intersection rules.** `cIntRule` is used by the network tool (`IdentifyHighwayIntersections`,
  `InsertIsolatedHighwayIntersection`, `QueryUserForPotentialIntersections`, `InsertHighwayIntersection`,
  `MarkIntersection`, `GetIntersectionRule`, …) and, outside it, **only** by
  `cSC4ViewInputControlNetworkIntxTool` (the puzzle-piece placement control): `OnMouseDownL` (`0x00661078`) and
  `OnMouseMove` (`0x00661657`) call `DoAutoComplete`, and `OnMouseMove` reads the global map's buckets inline
  (`0x006615B8`, `0x006616E6`). That control is created by `new` in `cSC4View3DWin::InvokeCatalogItemCommand`
  (`0x007EA3A5`), not through the COM director.

Every caller above is a direct call inside the executable. A scan of the whole image for the 32-bit addresses of
`Evaluate`, `AddRulesFromResource`, `cSC4TileRule::Read`, `ReloadRules`, `ReloadNetworkTypeInfo`, `LoadEvaluator`,
`SolveNetwork`, `SolveUsingEvaluator`, `ApplyNetworkRules`, `SolveMultiTypeSeparation`, `cIntRule::Test`,
`cIntRule::DoAutoComplete` and the three `cSC4TileInfo` functions finds **none**; the evaluator's destructor appears
only in its own vtable (`0x00AA8338`), and the reload callback `0x006232F0` only as the two push immediates
(`0x006269AB`, `0x00626A03`). So no interface vtable, class-factory table or message handler refers to them.

#### 2.4.3 The supported boundaries that can host a replacement

| Boundary | What it gives a replacement | Limits |
|---|---|---|
| **The rule resources** (§2.1, §2.3): type `0x0A5BCF4B`, group `0xAA5BCF57`, read through `SC4GetResourceAsFile` (`0x005F9670`, the GZCOM resource manager) by the tokenizer (`0x006210B0`) | a plugin data file that overrides an instance changes the rules the original solver runs (the way shipped mods change them) | data only: the matching algorithm stays the original's |
| **The network tool**, interface cISC4NetworkTool (vtable `0x00AA8748`, network-tools.md §1) | a tool that implements the interface owns the whole solve: its placement, `SolveNetwork`, `InsertOccupantsForSolvedCells` (slot 55) and the evaluators it consults | the original creates its tools with `new` in `cSC4NetworkManager::GetNetworkTool` (slot 5, `0x006108D0`) and `Init`, never through the director, so a replacement tool is reachable only through a replacement network manager (class `0xC990BD46`, IID `0xA9764C8C`, network-manager.md §1) |
| **The network manager**, class `0xC990BD46` | the only GZCOM class whose replacement controls which tool objects exist | see network-manager.md |

There is **no** separate interception point for the solver: no class ID, no IID, no factory and no message names it,
so a replacement must not invent one (no stub `CreateClass` for these owners). Two consequences for a host:

1. A rule solver of ours runs **as a component of our own network tool** (behind the network tool's replacement), not
   as its own switchable unit. While the original tool solves, the original evaluators run.
2. When our tools replace the original's, so that neither `cSC4NetworkTool::Init` nor `cSC4PowerLineTool::Init`
   of the original ever runs, the original `ReloadRules` never runs either and the global `cIntRule`
   map `0x00B452B0` stays **empty** for the original puzzle-piece control, which then finds no rule (§2.1: an empty
   table means "nothing applies"). A host that replaces the network tool must also own that control's catalogue
   command, or keep the original rules loaded.

---

## 3. Tile rule files

`cSC4TileEvaluator::AddRulesFromResource` (`0x00623020`) reads one file into the evaluator's **rule map**.

### 3.1 Tokens

The file is read line by line (tokenizer `0x006210B0`, `0x00622620`):

- a line whose first character is `;` is a comment and is skipped;
- tokens are separated by commas; the end of a line also ends a token;
- empty tokens are skipped;
- numbers are read with `atol`, which skips leading blanks and stops at the first non-digit.

### 3.2 Header

The first token must be exactly `#Transmogrify`, which turns the expansion of §4 on, or any other token that starts
with `#` (for example `#RoadRules#`). Otherwise the file is **rejected**: no rules are read.

### 3.3 Rules

Rules follow one another. Each rule (`cSC4TileRule::Read`, `0x00622870`) is:

1. **Optional transform token**: `noflip` (transform mask `0x0F`) or `notrans` (mask `0x01`). Without it the mask is
   `0xFF`.
2. **Centre line**: the token `1`, then four edge tokens for sides W, N, E, S. If the token is not `1`, `Read` fails
   and the whole file stops loading; the rules read before stay in the map *[inferred]*.
3. Any number of **condition lines** `2, m, e0, e1, e2, e3`.
4. Any number of **output lines** `3, m, id, rot, flip`.

Lines 3 and 4 may come in any order. The rule ends at the end of the file, or at the first token that does not start
with a digit or whose value is neither 2 nor 3; that token starts the next rule. At the end of the file a rule is
kept only if it has at least one output; a rule ended by the next rule's token is kept even with no outputs (§5.3).

**Edge tokens** (`0x00622800`): a decimal number `v`, read as `w = v mod 256`; the byte is
`((w / 10) << 3) | (w mod 10)`. The tens digit is the subtype and the units digit is the kind. So `2` is `0x02`, `13`
is `0x0B` (subtype 1, kind 3), `27` is `0x17`. Centre tokens have no wildcard.

**Condition edge tokens**: as above, or `?`, which makes that byte a wildcard (value byte 0, mask byte 0). A
condition is `{m, value, mask}`: `m` by `atol`, `value` the four bytes, `mask` `0xFF` in each byte that is not a
wildcard.

**Output lines** (`0x00622790`): `m` by `atol`; `id` by `RZConvertToUint32A` (hex with a `0x` prefix, else decimal);
`rot` by `atol` (kept as a byte, not masked); `flip` non-zero sets bit 7 of the orientation byte. An output is
`{id (u32), m (byte), orientation (byte)}`.

### 3.4 The rule map

The rule map is an ordered multimap keyed by the rule's **centre edge word** (the four centre bytes). An insert goes
after every existing rule with an equal key. So the rules for one key are in file order, and files loaded into one
evaluator never mix: each evaluator has its own map.

---

## 4. The `#Transmogrify` expansion

After a `#Transmogrify` file is read, `0x00622B50` replaces the evaluator's map with the expanded one:

1. Visit the source rules in map order (key ascending, then insertion order).
2. For each source rule and each `t` in 0–7 whose bit is set in the rule's transform mask: build the variant
   **mirror if `t & 4`** (§4.2), then **rotate by `t & 3`** (§4.1).
3. Drop the variant if it equals an **earlier variant of the same source rule** (`0x006214A0`): the same centre key
   and the same set of conditions, compared as `(m, value)` pairs in both directions. Masks and outputs are not
   compared, so a mirrored variant whose outputs differ is still dropped when its pattern is the same.
4. Each kept variant gets transform mask `0x01` and is inserted into the new map after the existing equal keys.

So for one key the new map holds variants in source-rule order, then by `t`.

### 4.1 Rotation by `r` quarter turns (`0x00621380`, `0x00621180`)

| Part | Rule |
|---|---|
| edge byte | the byte of side `i` moves to side `(i + r) & 3`; its value is unchanged |
| metaposition | 0 stays 0; 1–8 → `((m − 1 + 2r) mod 8) + 1`; 9–24 → `((m − 9 + 4r) mod 16) + 9` |
| output orientation, not flipped | rotation := `(rot + r) & 3` |
| output orientation, flipped | rotation := entry `rot × 4 + r` of the table `0x00AA8328`, which is `(rot − r) & 3` for `rot` 0–3; bit 7 stays |

A rotation by 1 turns the offset (dx, dz) into (−dz, dx): metaposition 1 (−1, 0) becomes 3 (0, −1). The table rows are
`[0,3,2,1]`, `[1,0,3,2]`, `[2,1,0,3]`, `[3,2,1,0]`. A flipped output whose written `rot` is 4 or more reads past the
table *[quirk]*; the shipped files only use 0–3.

### 4.2 Mirror (`0x00621DF0`; edge bytes `0x00621210`)

The mirror swaps west and east:

| Part | Rule |
|---|---|
| metaposition | (dx, dz) → (−dx, dz) |
| edge bytes | the W and E bytes swap; N and S stay. Then each byte is transformed by kind: kind 1 ↔ kind 3 (subtype kept); kind 2: bit 3 kept, bits 4 and 5 swapped, **bits 6–7 dropped** *[quirk]*; kinds 0 and 4–7 unchanged. Bit 2 of the kind is kept. |
| condition | the value and the mask both go through the edge-byte rule |
| output orientation | rotation unchanged; bit 7 toggled |

Example: the north-west diagonal `0x00000301` (W kind 1, N kind 3) mirrors to `0x00030100` (N kind 1, E kind 3), the
north-east diagonal.

---

## 5. Matching a cell (`Evaluate`, `0x006220D0`)

### 5.1 The neighbourhood

The solver builds a **neighbourhood** (`0x00627080`) for the centre cell (x, z), the solving tool and type `t`:

- the centre key = the centre cell info's edge word for `t`, or 0 when the cell has no info;
- `edges(m)` = the edge word for `t` of the cell info at the metaposition's offset, or 0 when there is no info
  (`0x00623EE0`; cached per metaposition);
- `valid(m)` (`IsValidSolutionPos`, `0x00623F40`) is false when the target cell has an info whose flags carry
  **another** type bit (`flags & (typebit ^ 0x1FFF) ≠ 0`); otherwise true (also when the target has no info). The
  solver always builds the neighbourhood with this check on (`0x00634150` passes 0 for the override flag that would
  turn it off).

### 5.2 First match wins

For the rules whose key equals the centre key, in map order:

1. Every **condition** must hold: `(mask & (value ^ edges(m))) = 0`.
2. Every **output** must have `valid(m)`.

The first rule that passes is the match; `Evaluate` returns its output list. If none passes, the cell does not match
this evaluator.

### 5.3 Notes

- A rule with no outputs (§3.3) still matches. It produces nothing, so it **blocks** the later rules of the same
  evaluator for that pattern *[compat]*.
- A rule's centre key compares the whole edge word, subtypes included, so a cell whose subtypes were set by §7 only
  matches rules written with those subtypes.
- Conditions on metaposition 0 test the centre itself.

---

## 6. The rule stages of `SolveNetwork`

### 6.1 Order (`SolveNetwork(type, resolve)`, `0x00635CB0`)

`S` is the tool's set of cells to be solved (`+0x84`, built by `BuildSetOfCellsToBeSolved`, network-geometry.md §8).
Sets are hash sets of cell keys; "for each key" means their iteration order (network-geometry.md §9.1).

1. `P := BuildPaddedNetworkSet(S, 2)` (§6.2).
2. `AssignEdgeSubtypes(P, R, reset = true)` (§7). It fills `R`, the cells whose subtypes could not be assigned, and
   takes them out of `P`.
3. Take the cells of `R` out of `S`.
4. `SolveMultiTypeIntersections(S, out, type)` (§8.2). Take every cell of `out` out of `S` and `P`.
5. For each type `u` = 0 … 12 (every type, not only the tool's), for each tile evaluator of `u` in order until a null
   entry: `SolveUsingEvaluator(P, S, evaluator, u)` (§6.3).
6. Put the cells of `R` back into `S`.
7. The result (network-geometry.md §8 step 3.5): when not `resolve` and `S` is not empty, the unsolved-cell rule;
   otherwise `AdjustTileSubsets(solved, resolve)` (§9.1), then `VerifyTileSubsets(solved)` (§9.2); either failing
   gives `+0x78 − 1`; when both pass, the unsolved-cell rule on what is left in `S` (`+0x78` when `S` is empty).

A cell is **solved** when it has a record in the tool's **solved list** `+0x98`: 12-byte records
`{piece id (u32), orientation byte, cell key (u32)}`. The cell info's `+0xB8` holds the record's index.

### 6.2 BuildPaddedNetworkSet(src, out, pad) (`0x00634480`)

For each key of `src`: every cell of the square of half-width `pad` around it (clamped to the city,
`GetPaddedRectAroundCell` *[inferred]*) that has a cell info and is **not fixed** (`+0x53 = 0`) goes into `out`. The
source cell itself goes in even when it is fixed.

### 6.3 SolveUsingEvaluator(P, S, evaluator, u) (`0x006358C0`)

`W` := a new empty hash set (100 initial buckets). `ApplyNetworkRules(P, S, W, evaluator, u)`. Then every key of `W`
goes back into `P`. So each evaluator gets one pass over the open cells; a cell claimed by an output (below) leaves `P`
for good, for every later evaluator and type.

### 6.4 ApplyNetworkRules(P, S, W, evaluator, ·, u) (`0x006340D0`, pop 24)

The fifth argument (the tool's `+0x48`) is not used. While `S` is not empty and `P` is not empty:

1. `c` := the first key of `P` in iteration order.
2. `c` **qualifies** when all of these hold:
   1. `c` has a cell info;
   2. its type bits are exactly `u`'s: `(flags & 0x1FFF) = 1 << u`;
   3. it is not a connection node (`+0x56 = 0`), **or** all of: the node occupant's `GetNetworkFlag() & 0x1FFF`
      equals the cell's **whole** flags word (`+0x08`), and either the occupant's slot 13 (`0x00619110`,
      network-manager.md §7.8) returns false, or, with the two edge words `(a, b)` it returns, the cell's edge word
      `e` for `u` has no subtype or kind bit above 2 in any byte (`e & 0xF8F8F8F8 = 0`), `within(a, e)` and
      `within(e, b)` (§1.3);
   4. `Evaluate` (§5) finds a rule.
3. If `c` does not qualify: move `c` from `P` to `W`.
4. Otherwise, for each output of the rule, in order: `target` := `c` moved by the output's metaposition.
   1. If `target = c`, note "self claimed".
   2. If `target` is in `P`, remove it from `P`; else if it is in `W`, remove it from `W`; else **skip this output**.
   3. If `target` is in `S`: remove it from `S`, append `{id, orientation, target}` to the solved list and set the
      target's `+0xB8` to the record's index.
5. If "self claimed" was not noted: move `c` from `P` to `W`.

Consequences *[compat]*:

- An output may claim a **padding** cell (in `P` but not in `S`): it is removed from `P` and gets no record.
- A cell already claimed by an earlier output (no longer in `P` or `W`) is never claimed twice; the output is skipped.
- Condition 2.3 compares the occupant's masked type bits with the cell's whole flags word, so a connection-node cell
  that carries a non-type flag (for example `0x40000`) never qualifies *[quirk]*.

### 6.5 One-way directions (`ResolveOneWayConflicts(type)`, `0x0063A640`, pop 4)

**The one-way byte.** A cell info's `+0x50` holds one **flow code** per side, in bits `2s` and `2s + 1` for side `s`:

| Code | Meaning |
|---|---|
| 0 | no one-way flow on this side |
| 1 | the traffic flows toward **+axis** along the side's axis (W and E sides: x; N and S sides: z) |
| 2 | the traffic flows toward **−axis** |

The byte describes the OneWayRoad network of the cell; the other types ignore it. For an existing occupant it starts
as the occupant's slot 69 value (network-geometry.md §8.1). At commit, `CreateNetworkOccupant` hands it to the new
occupant (network occupant slot 70 `SetOneWayDirections`, `0x0061B9F0`; the call is at `0x00624406`–`0x00624440`).

**Where the new codes come from.**

- `AssignEdgeMajorTypes` (`0x00638160`) writes them when the tool's type is OneWayRoad (the test is at
  `0x00638194`). The code of a direction `d` (0–7, network-geometry.md §3.2: 0 = −x, 2 = −z, 4 = +x, 6 = +z, odd
  values the diagonals in between) on a side of axis `a` is the table `0x00AA8BD8`, entry `d + 8a` (a = 0 for W/E,
  1 for N/S):

  | d | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
  |---|---|---|---|---|---|---|---|---|
  | code on W/E sides | 2 | 2 | 0 | 1 | 1 | 1 | 0 | 2 |
  | code on N/S sides | 0 | 2 | 2 | 2 | 0 | 1 | 1 | 1 |

  For each step `i` below the valid count `+0x78`: a **straight** step (`i <` the straight count `+0x74`) sets, on its
  front side (`pen direction / 2`) and its back side (the opposite side), the code of the **pen direction** `+0x7C`
  for that side's axis. This happens on both sides whether or not the step writes an edge byte there (the front edge
  is skipped on the last valid step, the back edge on step 0). A **diagonal** step (`i ≥ +0x74`) sets, on each side
  that leads to a neighbour of the run, the code of the **end direction** `+0x80` for that side's axis.
- `DoAdditionalDraws` (`0x006384E0`) applies the extra cells of `DoAutoComplete` (§10.5): for an extra-cell record
  with a non-zero one-way mask, `+0x50 := (+0x50 & ~mask) + bits`.

**ResolveOneWayConflicts** is called by `PlaceNetwork` for type 10 only, after `BuildSetOfCellsToBeSolved` and before
`MarkNeighborConnections` (network-tools.md §5.1 step 5.7). The pass carries the dragged run's direction into the
existing one-way roads it joins, then clears the codes that make a cell's flow contradict itself. Below, `code(c, s)`
is cell `c`'s flow code on side `s`, and the neighbour offsets are the tables `0x00AA8B74` (dx) and `0x00AA8B84` (dz),
which hold the same values as §1.1.

1. **Seeds.** List `A` gets one entry `{x, z, from = −1}` for each cell of step 0 and for each cell of the last
   dragged step (`+0x70 − 1`). The steps in between are not seeds *[compat]*.
2. **Propagation.** Repeat while `A` is not empty; `B` starts empty. For each entry `{x, z, from}` of `A`, in order:
   1. `c` := the world-cache cell info at `(x, z)` (there is no null test). Set its in-run byte `+0x10 := 1`. If its
      `+0x57` is 0, set it to 1 and add the cell's key to the solve set `+0x84`. So **every cell the pass visits is
      re-solved**, run cell or not *[compat]*.
   2. `n` := the number of sides whose byte in `c`'s OneWayRoad edge word has a non-zero kind (`byte & 7 ≠ 0`). If
      `n` is not 1 or 2, go to the next entry.
   3. The **portal side**: when `c`'s flags have `0x80000` (a tunnel portal), its occupant's other portal (through
      interface `0x8A4BD52B`, the paired portal at `+0x1D0`) gives a cell `(ox, oz)` (slot 59). The portal side is 0
      if `ox < x`, else 1 if `oz < z`, else 2 if `ox > x`, else 3 if `oz > z`, else none.
   4. `to := −1`. For each side `s` = 0 … 3 with `code(c, s) ≠ 0`: the neighbour `m` is the portal's cell when `s` is
      the portal side, otherwise `(x + dx, z + dz)`. Skip `s` when `m` is outside the city (unsigned compare), has no
      cell info, or is part of a highway intersection (`+0x52`). If `m`'s in-run byte is set and (`from < 0` or
      `s = from`): `from := s`. Otherwise `to := s` and `t := m` (so the **last** such side wins).

      **A skipped side keeps its flow code (NR8 — DIFFERS, corrected 2026-10-16 by re-answers31).** "Skip `s`"
      excludes the side from `from`/`to` selection **only**. It does **not** clear `code(c, s)`, for any of the
      three reasons (outside the city, no cell info, highway intersection) — the three are the same skip and the
      same outcome. In `FUN_0063a640` the propagation loop **reads** the code field (`c + 0x50`, two bits per
      side) and never writes it; after the loop there are exactly two stores, `code(c, to) := a` and
      `code(t, (to − 2) & 3) := a`. The **only** place the whole function zeroes a code is the conflict sweep of
      step 3 (`decomp/cSC4NetworkTool.c:11343` for `c`, `:11353` for the partner cell), which stores
      `'\0' << (p·2)` when `seen[p]` already holds a different flow.

      Test 26's east drop on X is that sweep, not a skip. X's OneWayRoad edge word is `0x00000202`, so the
      partner of side 0 is `(2 + 0) & 3 = 2` and the partner of side 2 is `(0 + 2) & 3 = 2` — both arms map to
      partner side 2. Side 0 contributes `f = code(c,0) = 1` (no axis flip, since `(p ^ s) & 2 ≠ 0`), so
      `seen[2] = 1`. Side 2 contributes `f = code(c,2) = 1` flipped to `1 ^ 3 = 2` (since `(2 ^ 2) & 2 = 0`).
      `seen[2] = 1 ≠ 2` is a conflict, so `code(c, 2) := 0` — X's east code goes `1 → 0`, giving `0x09` once the
      propagation has also set the north code to 2. **The change to make:** drop the `[choice]` that clears a
      skipped side's flow code in `resolve_one_way_conflicts`; leave skipped codes alone and let step 3's sweep
      do the clearing. If test 26 then fails on X's east byte, the sweep — not the propagation — is what to
      check.
   5. If `from < 0` or `to < 0`, go to the next entry.
   6. `a := code(c, from)`. A turn between W and N, or between E and S (`to = from ^ 1`), reverses the axis sense:
      `a := a ^ 3`. (Flow entering from the west, moving +x, and leaving through the north moves −z.)
   7. If `code(c, to) = a`, the cell already agrees: go to the next entry (the propagation stops here). Otherwise
      `code(c, to) := a`, `code(t, (to − 2) & 3) := a`, and append `{t.x, t.z, from = (to − 2) & 3}` to `B`.

   Then `A := B`.
3. **Conflict sweep.** `W1` := a copy of the solve set `+0x84` (`0x0050DA70`); `W2` := an empty hash set (the first
   bucket count of the STL prime list that is at least 100). While `W1` is not empty:
   1. For each key `k` of `W1`, in its iteration order: `c` := the cell info of `k` (no null test); `seen[0..3] := 0`.
      For each side `s` = 0 … 3:
      1. `p := (byte_s + s) & 3`, where `byte_s` is byte `s` of `c`'s OneWayRoad edge word: the **partner side** that
         the arm on side `s` pairs with (orthogonal: the opposite side; diagonal kind 1: `s + 1`; kind 3: `s − 1`).
      2. `f := code(c, s)`. If `f = 0`, next side. If `(p ^ s) & 2 = 0`, `f := f ^ 3`.
      3. If `seen[p] ≠ 0` and `seen[p] ≠ f` (a **conflict**): `code(c, p) := 0`. Then, when the neighbour `m` across
         `p` is inside the city and has a cell info: `code(m, (p − 2) & 3) := 0`, and if `m.+0x57` is 0, set it to 1
         and add **`k`** (not `m`'s key) to `W2` *[quirk]*.
      4. `seen[p] := f`.
   2. Insert the keys of `W2` into `+0x84` (`0x00638FA0`). Because they are keys of `W1`, which are already in `+0x84`,
      this changes nothing. Then `W1 := W2` and `W2` is emptied.

   So a neighbour whose code was cleared gets its `+0x57` mark but **never** joins the solve set, and the cell that
   caused the conflict is swept again *[quirk]*. The sweep ends because each conflict clears a non-zero code.

The pass returns nothing. The cleared and propagated codes reach the occupants at commit (above) and are used by
`Test` and `DoAutoComplete` (§10.5) and by the multi-type stage (§8.2), which also clears them.

---

## 7. Edge subtypes (`AssignEdgeSubtypes(P, R, reset)`, `0x006359B0`)

The rule files key on subtypes (the tens digit of an edge token). Before the rules run, the solver writes them:

1. `Q := BuildPaddedNetworkSet(P, 1)`.
2. If `reset`: for each cell of `Q` whose flags word has at most one bit set, and that is in `P`: clear the subtypes
   of its edge word for its type (`ResetEdgeMinorTypes`, `0x00621440`: keep bits 0–2 of each byte), then recompute
   the union `+0x4C`.
3. For each cell `c` of `Q` whose flags word has exactly one bit set (so a bridge-step cell, with `0x40000`, is
   skipped *[compat]*): `t` := its type; the scheme := record `+0x00` of `t` (§2.2). Scheme 1 runs §7.1, scheme 2
   runs §7.2, other schemes nothing. Each gets `inP` = whether `c` is in `P`. If the scheme returns false: remove `c`
   from `P` and add it to `R`.

A cell of `Q` that is not in `P` can still land in `R`; step 6 of §6.1 then adds it to `S`, where it counts as an
unsolved cell *[compat]*.

In both schemes, "set byte `i` of `c`" changes `c`'s edge word for `t` and recomputes `c`'s union.

### 7.1 Scheme 1: diagonal stubs (`0x00633A60`; Road, Street, Pipe, Subway, OneWayRoad, DirtRoad)

Let `e` be `c`'s edge word for `t`. Classify its sides by `byte & 7` (`0x00620DF0`): **orthogonal** sides (kind 2)
and **diagonal** sides (kind 1 or 3), each in side order. A cell is **purely diagonal** (`0x00623C90`) when its edge
word for `t` has no orthogonal side and one or two diagonal sides.

If `c` has no orthogonal side or no diagonal side, return true. Otherwise compute the **through marks** `M[0..3]`
(below), then for each diagonal side `s` with `M[s] = 0`:

1. `n` := the neighbour across `s`. No cell info: next side.
2. `pd` := `n` is purely diagonal.
3. If `inP`: if `c` is fixed and `pd`, return **false**. Set `c`'s byte `s` subtype to `pd` (0 or 1, bits 3–7
   replaced). If not `pd`, also set its kind to 2 (`(byte & 0xF8) | 2`): the diagonal stub becomes an orthogonal side.
4. If `n` is in `P`: the same on `n`'s byte `(s + 2) & 3`. Otherwise, if `pd` and `n` is fixed, return **false**.

Return true.

**Through marks** (`0x006335E0`). `M` starts all 0. For each side `s` = 0 … 3 with `M[s] = 0`:

1. `k := byte(s) & 3`. If `k = 1`, `s2 := (s + 1) & 3`; if `k = 3`, `s2 := (s + 3) & 3`; otherwise next side.
2. `k2 := byte(s2) & 3`. If `k + k2 ≠ 4`, next side. (The two sides form one diagonal through their shared corner.)
3. Walk twice, first with `(dir, kind) = (s, k)`, then with `(s2, k2)`; stop at the first failed walk. A walk: step
   to the neighbour across `dir`. No cell info: **fail**. Purely diagonal: **success**. Otherwise turn,
   `dir := (dir + 1) & 3` when `kind = 3`, else `(dir − 1) & 3`; `k' :=` that neighbour's `byte(dir) & 3`; if
   `k' + kind = 4`, continue from it with `kind := k'`, else **fail**.
4. If both walks succeed: `M[s] := M[s2] := 1`.

### 7.2 Scheme 2: switches and curves (`0x00633C10`; Rail, LightRail, Monorail)

For each side `i` = 0 … 3 (re-reading `c`'s edge word each time): `b := byte(i)`, `k := b & 3`; skip when `k = 0`.
Let `opp = (i + 2) & 3`, `left = (i + 3) & 3`, `right = (i + 1) & 3`; `kind(j) = byte(j) & 3`.

- **`k = 2`**: `f := (kind(opp) ≠ 0 ? 1 : 0) | (kind(left) = 1 ? 2 : 0) | (kind(right) = 3 ? 4 : 0)`. If `f` is 0 or
  1, skip the side; otherwise the subtype is `f` (2–7).
- **`k` = 1 or 3**: skip the side unless `kind(opp) ≠ 0`, and also skip it when `kind(opp) = k` and
  `k + kind(left) = 4`. Then `p := right` when `k = 1`, else `left`; the subtype is 2 when `kind(p) = 2` or
  `kind(p) + k = 4`, else 1.

The new byte is `(subtype << 3) | k` (bit 2 is dropped). Then:

1. If `inP`: if `c` is fixed, return **false**; set `c`'s byte `i`.
2. `n` := the neighbour across `i`. If `n` is in `P`: set `n`'s byte `opp` to the same value (when `n` has a cell
   info). Otherwise, if `n` has a cell info and is fixed, return **false**.

Return true.

---

## 8. Multi-type intersections

### 8.1 The solutions file (instance `0x10000001`)

**Solutions.** For each pair of types `a < b` (by index), `ReadAllMultiTypeIntersectionSolutions` (`0x00641D30`) reads
the section `[<NameA><NameB>IntersectionSolutions]`, for example `[RoadRailIntersectionSolutions]`, through
`0x0063E340` and the entry callback `0x0063D080`:

- the entry's key is ignored (keys may repeat);
- the value is split on commas (`RZParseDelimitedText`) and must give **exactly 5** fields, else the entry is ignored:
  `edgesA, edgesB, pieceId, rot, flip`, each read with `RZConvertToUint32A` (hex with `0x`, else decimal). The edge
  words are written whole, in hex (`0xSSEENNWW`, §1.3), not with the per-side tokens of §3;
- the pair's table maps `(edgesA, edgesB)` to `{pieceId, rot | (flip ≠ 0 ? 0x80 : 0)}`; a later entry with the same
  key overwrites the earlier one.

**Override matrix.** In the section `[AllowedOverrides]`, each entry's key is ignored and its value is
`NameA,NameB` (`SC4NetworkLoadOverrideMatrix`). Both names are converted with `NetworkNameToID` and the bits are set
**symmetrically**: row A gets bit B and row B gets bit A. The 13 rows are copied into the type records (`+0xFC`).

### 8.2 SolveMultiTypeIntersections(S, out, primary) (`0x00634820`)

`primary` is the tool's type. `prio` is the table `0x00AA8A18`:

| Type | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| prio | 3 | 5 | 6 | 2 | 1 | 0 | 4 | 1 | 7 | 7 | 3 | 0 | 5 |

`typeOf(bits)` (`0x00623E60`) is the lowest type 0–11 whose bit is in `bits`, and **12 when none is**, so the
GroundHighway bit and every non-type flag bit read as type 12 *[quirk]*. `beats(j, i)` is true when
`prio[i] < prio[j]`, or the priorities are equal, `i ≠ primary` and `j = primary`. `elevated(x)` is `x & 0x304 ≠ 0`
for a set of type bits. "Remove type `i`" means: clear its bit in the flags, clear its edge word, recompute the union,
and when `i` = 10 clear the one-way byte `+0x50`.

For each key `c` of `S`, with `f` its flags word, `lo` the lowest set bit of `f` and `rest = f − lo`:

1. `rest = 0`: next cell.
2. **Three or more bits** (`rest` has two or more bits): `A := typeOf(lo)`, `B :=` typeOf(the lowest bit of `rest`),
   `C :=` typeOf(`rest` without its lowest bit). For `i` in (A, B, C), then `j` in (A, B, C): if `beats(j, i)`,
   `covers(e[j], e[i])` and `elevated(bit i) = elevated(bit j)`, **remove type `i`** and start again at step 1 for
   this cell with the new flags. The edge words compared are those read when the cell's analysis started. If no pair
   qualifies, next cell.
3. **Two bits**: `A := typeOf(lo)`, `B := typeOf(rest)`. A connection node (`+0x56`): next cell.
   1. Winner and loser: if `beats(B, A)`, `W = B`, `L = A` and the winner's bits are `rest`; otherwise `W = A`,
      `L = B` and the winner's bits are `lo`. `eW := e[W]`, `eL := e[L]`.
   2. **Covered**: if `covers(eW, eL)`: when row `W` of the override matrix has bit `L`, clear `e[L]`, recompute the
      union, set the flags to **the winner's bits only**, and clear `+0x50` if `L` = 10. In both cases, next cell; no
      piece comes from this stage, and the cell stays in `S` for the evaluators.
   3. **Crossing**: if `elevated(W's bits) = elevated(L's bits)`, clear every byte of `eL` whose low two bits equal
      `eW`'s and are non-zero (the shared arms belong to the winner); store `eL` and recompute the union; if `L` = 10,
      `+0x50 &= D(eL)`. Then, whatever the elevation, `+0x50 &= ~D(U)`, where `U` is the OR of the edge words of
      Road, Street, Avenue and DirtRoad.
   4. Look up the table of the pair `(min(W, L), max(W, L))` with the key (edge word of the lower-index type, edge
      word of the higher-index type), using the updated `eL`. Not found: next cell (it stays in `S`).
   5. Found: set `+0xB8` and append `{pieceId, orientation, c}` to the solved list; append `c` to `out`; append
      `(x, z, f)` to the tool's potential-intersection list `+0x08` (the list that
      `QueryUserForPotentialIntersections` walks, §10.7).

`+0x50` is the cell's one-way direction byte (two bits per side, §6.5).

### 8.3 SolveMultiTypeSeparation(cell, t, &id, &rot, &flip) (`0x00633FA0`, pop 20, returns AL)

The preview's per-type piece lookup. Its callers are `AddCellPreview` (`0x006246A0`) and the draft (not real) branch
of `InsertOccupantsForSolvedCells`; both only draw, so this function never changes the solved list, the sets or the
cell infos. `cell` points at an `(x, z)` pair.

For each tile evaluator of type `t` (type record `+0x24`, §2.3), in order, stopping at the first null entry:

1. Build the neighbourhood of `(x, z)` for type `t` (`0x00627080`, §5.1) with the override flag set to 1, so
   `valid(m)` is always true: an output may land on a cell that carries another type *[compat]*.
2. `Evaluate` (§5). No match: next evaluator.
3. On a match, find the first output of the rule whose metaposition target (`0x00620DA0`) is the cell itself. If
   there is none, next evaluator. Otherwise `*id :=` the output's piece id, `*rot :=` its orientation byte without bit 7
   (`& 0x7F`), `*flip :=` bit 7 of it (0 or 1), and return **true**.

Return **false** when no evaluator gives a piece for the cell. The rules' other outputs are ignored, and no
claiming happens (§6.4 is not involved) *[compat]*.

---

## 9. Tile subsets

### 9.1 Conflict patches: `[RuleOverrides]` and AdjustTileSubsets

**File** (instance `0x10000002`, section `[RuleOverrides]`, entry callback `0x0063D1E0`). Each entry's key and value
are both six numbers `idA, rotA, flipA, idB, rotB, flipB`, each parsed with the pattern
`" %i , %i , %d , %i , %i , %d"`, which must yield 6 fields (`%i` accepts `0x` hex). The key is the **pattern**, the
value the **replacement**; an orientation is `rot | (flip ≠ 0 ? 0x80 : 0)`. Entries go into an ordered map
(`0x00B466D0`) keyed by `idA`, several per `idA` allowed, in **file order** *[compat]* (NR5):

- the INI reader's entry enumeration (`cRZIniFile::EnumEntries`, `0x0091D91C`) reads the section's lines in file order
  up to the next `[`; it skips empty lines and `;` comments, splits each line at its first `=`, trims the key on the
  right and the value on both sides, and skips a line whose key or value is then empty. It collects the entries in a
  vector and calls the callback in vector order. **Repeated keys are kept**: two lines with the same pattern give two
  entries;
- the insertion (`0x0063C920`) walks the tree going left when the new `idA` is below the node's (unsigned) and right
  otherwise, so an entry with an `idA` already present goes **after** the existing ones;
- `PatchTilePair` walks the equal range of `idA` forward.

So the first line (in the file) whose pattern matches wins.

A pattern describes two pieces side by side, **B east of A** (B on A's side 2), in a normalised frame.

**PatchTilePair(A, B, s)** (`0x006337E0`), where B is the piece on A's side `s` and the patterns are those with
`idA = A.id`:

1. Normalise both orientations to the frame where B is east: an unflipped orientation's rotation becomes
   `(rot + 2 − s) & 3`; a flipped one's `(rot + s + 2) & 3`, keeping bit 7. Call them `nA`, `nB`.
2. For each pattern in map order with `idB = B.id`:
   1. If the flip bits of the pattern's A orientation and `nA` differ, compare against `nA ^ 0x82` and `nB ^ 0x82`
      (the pair mirrored north–south, B still east); otherwise against `nA` and `nB`.
   2. It matches when the A orientation equals, and either `B.id = 0` or the B orientation equals.
   3. On a match: if the replacement's `idA` is 0, return **2** (a conflict that cannot be patched). Otherwise A and B
      take the replacement's ids and orientations (`^ 0x82` again when step 2.1 mirrored), the orientations are
      de-normalised (unflipped `(rot + s + 2) & 3`, flipped `(rot + 2 − s) & 3`), and the result is **1**.
3. No match: **0**.

**AdjustTileSubsets(solved, resolve)** (`0x00634D70`):

1. An empty conflict table: return true.
2. `budget := max(100, 8 × the solved count)`.
3. A **pass**: `changed := false`; for each record A of the solved list, in index order:
   1. `tries := 0`. For each side `s` = 0 … 3, with `n` the cell info across `s` (none: next side):
      1. B := `n`'s solved record when `n.+0xB8 ≥ 0`. Otherwise, when `n` has an occupant, an **existing** B =
         `{occupant PieceId (slot 23), occupant orientation (slot 26), n's key}`. Otherwise next side.
      2. If `n` is fixed (`+0x53`) or a connection node (`+0x56`): B becomes an existing copy with **id 0** (so only
         patterns with `idB = 0` match it).
      3. `r := PatchTilePair(A, B, s)`. If `r = 0` and B is existing, try the reverse once:
         `r := PatchTilePair(B, A, (s + 2) & 3)` with the patterns of `B.id`.
      4. `r = 1`: `budget −= 1`; below 0, return **false**. `changed := true`. An existing B is appended to the
         solved list (and `n.+0xB8` set), so it is placed again with its new piece. `tries += 1`; if `tries > 100`, go
         to the next record; otherwise restart the sides of A from 0.
      5. `r = 2`: if not `resolve`, return **false**. Otherwise put A's cell back into `S`, decrement `+0xB8` of
         every later record's cell, remove A from the list, and **start the pass again from the first record** with
         `changed := false`. A's own `+0xB8` is left as it was *[quirk]*.
4. At the end of a pass: if `changed`, run another pass; otherwise return true.

### 9.2 Pattern checks: `[TileVerifiers]` and VerifyTileSubsets

**File** (the network INI of §2.1 step 1, section `[TileVerifiers]`, entry callback `LoadVerifiersCallback`
`0x0063F7A0`). Each entry adds one **verifier** for the piece id in its key (hex, `RZAtoUint32HexA`). The value is a
list of groups of four comma-separated tokens `m, id, rot, flip`: `m` by `atol`, `id` in hex, `rot` by `atol`, and
`flip` non-zero adds `0x80` to the orientation. Each group is a **requirement** `{id, orientation, m}`. Several
entries for one piece id give several verifiers, any one of which may pass.

**VerifyTileSubsets(solved)** (`0x00635290`): with no verifiers, return true. For each record A, in index order:

1. No verifier for `A.id`: keep A.
2. A verifier **passes** when every requirement holds:
   1. `(dx, dz)` := the offset of `m`, then turned by A's orientation: when rotation bit 0 is set,
      `(dx, dz) := (−dz, dx)`; when rotation bit 1 is set, `(dx, dz) := (−dx, −dz)`; when flipped, `dx := −dx`.
   2. The cell `(A.x + dx, A.z + dz)` lies inside the city (both coordinates below the city size, compared unsigned)
      and has a cell info.
   3. Its piece: its solved record, else its occupant's PieceId and orientation, else id 0 and orientation 0.
   4. The relative orientation: when the flip bits of A's and the piece's orientations differ,
      `(o + oA) & 0x83`, otherwise `(o − oA + 8) & 0x83` (byte arithmetic). The piece id must equal the requirement's,
      and the relative orientation must equal the requirement's orientation.
3. If no verifier of `A.id` passes: put A's cell back into `S`, decrement `+0xB8` of every later record's cell, remove
   A from the list (the next record moves into its index) and note a failure.

Return true when nothing was removed.

---

## 10. Highway intersection rules (`nSC4Networks::cIntRule`)

These rules describe the **prebuilt intersection pieces** (interchanges, highway–road crossings and the like): a
pattern of existing network cells that the piece replaces, the cells it newly occupies and the models it places.

### 10.1 The file (instance `0x10000000`)

`ReadHighwayIntersectionRules` (`0x00641B60`) enumerates every section of the INI (INI slot 14) and hands each to
`ReadIntersectionRule` (`0x00641A50`):

- a section whose name starts with `Ordering` is an **ordering** section (§10.8);
- a section named `HighwayIntersectionInfo_<hex id>` is a **rule** with that id (read with `RZAtoUint32HexA`). The
  rule is created in the global rule map `0x00B452B0` (a hash map keyed by id; entry = `{next, id, rule}`, the rule at
  entry `+8`). Its id is stored, its ring links (§10.8) are set to its own id, its `CellLayout` and `ConsLayout` line
  lists are cleared, its keys are read (`ReadIntersectionEntries`, `0x00640320`), and then the two ASCII layouts are
  parsed (§10.3) when their line lists are not empty;
- other sections are ignored.

**Keys** (compared case-insensitively, `cRZString::IsEqual`), in the order the reader tests them; every key may
repeat, and a later one overwrites (lists append):

| Key | Value | Effect (rule field) |
|---|---|---|
| `Piece` | `x, z, rot, flip, 0xid` (two floats, then `" , %i , %i , 0x%x"`) | append a **model** `{id, rot byte, flip byte, x (f32), z (f32)}` (`+0x04`, 16-byte entries) |
| `StepOffsets` | `a,b` (`%d,%d`, must give 2) | `+0x10 := a`, `+0x14 := b` |
| `CellLayout` | one text line | append to the cell layout lines |
| `ConsLayout` | one text line | append to the constraint layout lines |
| `AutoTileBase` | number | `+0xC8` |
| `AutoPathBase` | number | `+0xCC` |
| `CheckType` | a check-type definition (§10.2) | add to the tag map `+0x64` |
| `CopyFrom` | hex id | if that rule exists: copy it whole into this one (`0x0063E060`), then restore this rule's id and reset its ring links to its own id; otherwise nothing |
| `Transpose` | (any) | transpose the rule (§10.4) |
| `Rotate` | `n` | rotate the rule by `n` quarter turns (§10.4) |
| `Translate` | `a,b` (`%d,%d`) | translate the rule by (a, b) (§10.4) |
| `ReplacementIntersection` | `a,b` (`%d,%d`, the count is not checked) | `+0x48 := 1`; `+0x40 := a`, `+0x44 := b` |
| `ConvertQueryID` | number | `+0x78`: the string id of the conversion question (§10.7) |
| `PlaceQueryID` | number | `+0x7C` |
| `HandleOffset` | `a,b` (`%d,%d`, must give 2) | `+0x94 := a`, `+0x98 := b` |
| `PreviewEffect` | text | `+0x80` (a string) |
| `Costs` | `" %d "` | `+0xB4` |
| `AutoPlace` | (any) | `+0xE9 := 1`, whatever the value |
| `OneWayDir` | number | `+0xE8 := atol(value)` |

The reader also builds the strings `PieceType` and `ResetPieces` but never compares a key with them; such keys are
ignored *[compat]*. Keys are applied in file order, so `CopyFrom` must come before the keys that adjust the copy, and
`Rotate`/`Transpose`/`Translate` act on everything read or copied before them.

Other rule fields: `+0xE4` the **type mask** (the OR of the type bits named by the check types), the tag lists and
bounds of §10.3.

### 10.2 Check types (`CheckType`)

The value is parsed by a small tokenizer (`cParser`, `0x0063DDF0`, `0x0063F1F0`); `#` starts a comment that runs to
the end of the value and `;` ends it. The grammar:

```
<tag> - <Network>:<edges>[,<mask>] [<Network>:<edges>[,<mask>]] [check] [multiok] [optional]
```

- `<tag>` is one character; longer tags fail with "Bad map tag".
- At most two network checks; a third fails with "Too many network checks in one rule".
- `<Network>` is a network name (§1.2); `<edges>` and `<mask>` are hex edge words; the mask defaults to `0xFFFFFFFF`.
- `check` clears the tag's **occupies** flag; `optional` sets **optional** and clears **occupies**; `multiok` sets
  **multi-ok**. By default a tag occupies its cell, is not optional and is not multi-ok.

A check-type record (24 bytes): `+0` type 1, `+1` type 2 (−1 when absent), `+4` edges 1, `+8` mask 1, `+0xC`
edges 2, `+0x10` mask 2, `+0x14` multi-ok, `+0x15` occupies, `+0x16` optional. Each named type's bit is ORed into the
rule's `+0xE4`.

### 10.3 The ASCII layouts

**Cell layout** (`ParseCellsFromASCIIArt`, `0x00609FC0`). Each `CellLayout` line is one row `z` (from 0, in order)
and each character one column `x` (from 0):

| Character | Meaning |
|---|---|
| space, `.` | nothing |
| a letter or digit with a check type | a **tagged cell** `(x, z, tag)` (list `+0x58`, 12-byte entries); if its type occupies, it also extends the **bounds** and joins the **occupied cells** (`+0x4C`) |
| a letter or digit without a check type | nothing |
| `+` | a **new cell** (list `+0x28`): a cell the piece takes that holds no network yet; it also joins the occupied cells and extends the bounds |
| `<` | marks this row as the centre row |
| `^` | marks this column as the centre column |

The bounds start at `minX = minZ = 1000`, `maxX = maxZ = −1000` (`+0x18`, `+0x1C`, `+0x20`, `+0x24`). Afterwards every
coordinate (bounds, occupied, new and tagged cells) is shifted by minus the centre (column, row). Without a `<` or
`^` the centre stays −1, so the cells shift by +1 *[quirk]*. The parser then builds the **tile grid** and its
**tile map**, which give each occupied cell its own piece (§10.9):

- the grid (`+0xB8`, a byte vector) has `(maxZ − minZ + 1) × (maxX − minX + 1)` entries, row by row, width `+0xC4`,
  all 0 at first;
- for each occupied cell, in list order, with `col = x − minX` and `row = z − minZ`: its **tile code** is the byte
  `(row × 16 + col + 1) & 0xFF`, written into the grid at `row × width + col`;
- the tile map (`+0xD0`, a hash map from the code byte; buckets at `+0xD4`) gets, for that code, the value
  `{tile = col × 0x100 + row × 0x10 (u32), orientation byte 0, new-cell byte}`, where the new-cell byte is 1 unless
  the cell is also a tagged cell (then 0). When the code is already in the map, its value is overwritten.

Codes repeat when a layout is more than 16 columns wide (`col` 16 of row 0 gives the code of `col` 0 of row 1) or
when `row × 16 + col + 1` passes 255; such cells share one map entry, so they all get the last one's tile and
orientation *[quirk]*. Only the orientation of a map entry changes later (the transforms of §10.4).

**Constraint layout** (`ParseConstraintsFromASCIIArt`, `0x0060A560`) maps characters to constraint codes:
`-` 1, `|` 2, `/` 3, `\` 4, `F` 5, `R` 6, `#` 7, `X` 8; `<` and `^` set its centre (`+0xB0`, `+0xAC`). The grid is
`+0x9C` (width `+0xA8`). Its consumer is `InsertIntersectionConstraints` (`0x00644710`), which the smoothing loop
runs (network-geometry.md §9.4 step 4) to turn the codes into vertex-height equalities and slope limits. That is
height geometry, not piece choice or simulation; its exact meaning is NR7 (§14).

### 10.4 Transforms

All three keep the rule internally consistent; the cell lists hold integer offsets from the anchor.

**Translate(a, b)** (`0x00608D20`): add (a, b) to every new, occupied and tagged cell, to every model position, to the
handle offset (`+0x94`, `+0x98`) and to the bounds; subtract (a, b) from the constraint centre (`+0xAC`, `+0xB0`).

**Transpose** (`0x006091F0`), the mirror across the line x = z:

- swap x and z in every cell list, model position, the handle offset and the bounds (min with min, max with max);
- each check type's edges: swap bytes W ↔ N and E ↔ S, and map each byte whose **whole value** is 1, 2 or 3 to
  `4 − value` (1 ↔ 3); a byte with subtype bits is left as it is (`0x00608C50`) *[quirk]*. Masks: the same byte swap
  only;
- each model: flip toggled; rotation `+1` when it becomes flipped, `−1` when it becomes unflipped (mod 4); the tile
  map entries likewise;
- the constraint and tile grids are transposed (`0x00608F20`);
- `OneWayDir := (−d − 2) & 7`.

**Rotate(n)** (`0x006094B0`), `n & 3` quarter turns; 0 does nothing. One turn maps (x, z) to (−z, x):

- every cell list and the handle offset are turned;
- each check type's edges and masks are rotated left by `8n` bits (byte `i` moves to side `i + n`);
- each model: rotation `+n` (unflipped) or `−n` (flipped), mod 4; its position turned;
- the tile map entries: rotation `+n` or `−n` likewise, the flip kept;
- the constraint and tile grids are turned (`0x00609040`);
- the bounds: for a half turn (bit 1 of `n`), min and max swap and all four are negated; then for an odd turn (bit 0),
  `minZ := minX`, `minX := −maxZ`, `maxZ := maxX` and **`maxX := −(old minX)`**. The correct value would be
  `−(old minZ)`, so after an odd turn `maxX` is wrong whenever `minX ≠ minZ` *[quirk]* (`0x006097A3`–`0x006097B9`). It
  only affects `IsWithinBounds`;
- `OneWayDir := (d + 2n) & 7`.

### 10.5 Matching an intersection rule

**IsWithinBounds(x, z, width, height)** (`0x00608CD0`): `x + minX ≥ 0`, `z + minZ ≥ 0`, `x + maxX < width` and
`z + maxZ < height` (the last two compared unsigned).

**Test(x, z, tool)** (`0x00609A00`) returns 0 (match), 1 (no match) or 2 (hard failure). `ow` := the one-way pattern
for `OneWayDir`, table `0x00AA752C` = `0x22, 0xAA, 0x88, 0x99, 0x11, 0x55, 0x44, 0x66` (index 0–7).

For each tagged cell `(rx, rz, tag)` with its check type `ct`:

1. `ct` optional: next cell.
2. `info` := the tool's cell info at `(x + rx, z + rz)`. None: return **2**.
3. If `ct` occupies:
   1. The cell is an **intersection** when its occupant has all of the flags `0x100000` (network occupant slot 8) or
      `info.+0x52` is set. If so: without `ReplacementIntersection` (`+0x48 = 0`), return **1**. If the flag came from
      the occupant, its anchor cell (network occupant slot 71, `+0x11C`: a cell key split into x and z, false when the occupant has
      none) must equal
      `(x + +0x40, z + +0x44)`, else return **1**.
   2. If `info` is fixed (`+0x53`) and `+0x52` is not set: return **1**.
4. `acc := 0`. For each network check `k` (type `t ≥ 0`): if the cell's flags lack `t`'s bit, return **1**;
   `acc |= bit`; if `(mask_k & (info.edges[t] ^ edges_k)) ≠ 0`, return **2**.
5. If not multi-ok and the cell's flags carry a type bit outside `acc` (`flags & ~acc & 0x1FFF ≠ 0`): return **1**.
6. If `acc` has the OneWayRoad bit and `ct` does **not** occupy: if `((info.+0x50 ^ ow) & D(e)) ≠ 0`, where `e` is the
   one-way check's edges, return **2**.

Then, if the rule has no new cells (`+`), return **0**. Otherwise, for each new cell `(x + rx, z + rz)`: if the
terrain says it holds water (terrain slot 34), return **1**; if the occupant manager has a network occupant there
(slot 32 with the network-type filter `0x1F4F`), return **1**. Finally build the demolition region of the new cells
(`ComputeDemolitionRegionForPlacementCells`, `0x0061F7C0`, with the tool's occupant filter `+0x364`): it fails when a
lot on those cells has a zone type from 10 to 15 (the zone-type mask `0xFC00`, zones.md) or the demolition utility
(query slot 6, flags `0x2001`) refuses; a failure returns **1**, success **0**.

**DoAutoComplete(x, z, tool)** (`0x006097E0`, returns 1). It adds the **extra cells** that complete the
intersection's arms (the tool's list `+0xF0`, network-geometry.md NG3). For each tagged cell (optional ones included)
and each network check `k` with type `t ≥ 0`:

1. `info` := the cell info at `(x + rx, z + rz)`; `mismatch := info` absent, or `(mask_k & (info.edges[t] ^
   edges_k)) ≠ 0`.
2. For `t` = OneWayRoad: `cow := info ? info.+0x50 : 0`, `d := D(edges_k)`, `owbad := (d & (cow ^ ow)) ≠ 0`. If the
   tag does not occupy, `G` := `cow` with each two-bit pair set to `11` when it is non-zero; if
   `(d & G & (cow ^ ow)) ≠ 0`, skip this check (an existing one-way arm points the other way).
3. If `info` exists and neither `mismatch` nor `owbad`: skip. Otherwise add an extra cell (`0x00629170`) at
   `(x + rx, z + rz)` with edges `edges_k`, type `t`, one-way bits `ow & d` and mask `d` (both 0 for other types).

**GetPrimaryType()** (`0x006099A0`): the first type of the tagged cell at (0, 0), else 2 (Highway).

### 10.6 IdentifyHighwayIntersections(v, type) (`0x0063B340`, pop 8)

Called by `PlaceNetwork` after the edge major types (network-geometry.md §8.2). It finds the intersections that a drag
across existing networks forms automatically:

1. Clear the tool's intersection list `+0xCC`. `inRun := false`.
2. For each step `s` < `v`, for each cell `(x, z)` of the step whose cell info has an existing occupant:
   1. Call the occupant's slot 38 (`GetEdgeMaskForType(type)`); the result is not used.
   2. For each rule of the global map, in its hash order (buckets `0x00B452B4`, ids by identity modulo the bucket
      count): the rule matches when `AutoPlace` (`+0xE9`), `+0xE4` meets the cell's flags, `IsWithinBounds(x, z,
      city width, city height)` and `Test(x, z, tool) = 0`.
   3. On a match: the step matched. If not `inRun`: start a run with this rule's id, `(x, z)` and `s`;
      `MarkIntersection(run, true)` (`0x0063B130`) sets `+0x52` on the cell infos of the rule's occupied cells. Later
      matches during a run are ignored.
3. After a step without a match, if `inRun`: **record** the run and append `(x, z, type)` to the tool's
   potential-intersection list `+0x08`; `inRun := false`.
4. After the last step, if `inRun`: record the run and append `(x, z, 4)` to `+0x08`. The 4 is a constant, not the
   type *[quirk]*.
5. Return `v`, on every path. So the caller's failure code `0xF0000005` is never set (network-geometry.md quirk 7).

A **record** (`+0xCC`, 40 bytes): `{rule id, x, z, start step s, s + StepOffsets.a, s + StepOffsets.b, an empty
vector}` *[inferred: the vector]*. `MarkHighwayIntersectionPieces` (`0x0063B7F0`) and
`RemoveHighwayIntersectionPiecesFromSetOfCellsToBeSolved` (`0x0063B1A0`) use the records whose start step is below
`+0x78`; `InsertHighwayIntersectionPieces` (`0x0062C4B0`) places their models at commit (network-tools.md §6.1).

### 10.7 Placing one intersection

**InsertIsolatedHighwayIntersection(x, z, id, real)** (`0x0062CD50`, the intersection tool) places rule `id` at
`(x, z)`:

1. Reset the run state; `+0x50 := real`; `type := GetPrimaryType()`; record the intersection in `+0xCC`; one step of
   one cell `(x, z)`; the dragged and valid counts `+0x74 = +0x78 = 1`.
2. The cells: the rule's new cells plus the extra cells (`DoAutoComplete`), without connection nodes.
3. The demolition region of those cells (`+0x210`, `+0x220`) must succeed, else return 0.
4. Slot 68 (reserved tiles, `(&v, type, 1)`): if it cuts `v` below `+0x78`, fail with `0xF0000002`.
5. Cell infos (slot 71), `DoAdditionalDraws`; `Test` must return 0, else `0xF0000007`.
6. `MarkHighwayIntersectionPieces(true)`; `BuildSetOfCellsToBeSolved`; `SolveNetwork(type, false)` must return a
   positive count, else `0xF0000007`.
7. The smoothing levels 0–2 (slot 56); a failure at level 3 gives `0xF0000006` or `0xF0000008` (network-geometry.md
   §9.4).
8. If `real`: slots 70, 60, 57 and the position-changed marks (network-tools.md §6.1).
9. Slot 55 (`InsertOccupantsForSolvedCells`), `InsertHighwayIntersectionPieces`, `MarkOccupantsUsable`; return 1.

**QueryUserForPotentialIntersections** (`0x0062D450`). For each `(x, z, ·)` entry of the tool's `+0x08` list (only x
and z are read): get the highway tool (`GetNetworkTool(2, true)`); take the first rule, in map order, that has a
`ConvertQueryID` and whose `Test(x, z)` is 0; do a dry `InsertIsolatedHighwayIntersection(x, z, id, false)`; if it
succeeds and the city's funds (city slot 52, budget slot 6) cover its cost, show the confirmation text (string table
group `0x2A592FD1`, instance `ConvertQueryID`; the fallback text is `## Intersection query string missing ##`). On
"yes": place it for real, spend the cost (budget slot 8) and post message `0xEA5EC59F`.

### 10.8 Ordering sections (the intersection tool's cycling)

Every rule carries two circular lists, each as a pair of **rule ids** (not pointers): the **rotation ring**
(`+0xEC` previous, `+0xF0` next), which the rotate command walks, and the **type ring** (`+0xF4` previous, `+0xF8`
next), which cycles between intersection types. A new rule, and a rule after `CopyFrom`, has all four links set to its
own id (a ring of one).

`ReadIntersectionOrderingEntries` (`0x006416B0`) is the per-entry callback of an `Ordering…` section (INI slot 13,
the entry enumeration of §9.1, so lines are handled in file order). `ReadHighwayIntersectionRules` builds its context
once per file; the context holds a **cursor** `K`, set to −1 once and kept across all the `Ordering` sections of the
file. Each line's key is compared case-insensitively (`IsEqual` with the case flag off): first with `RotationRing`,
then with `AddTypes`; other keys are ignored. The value is a comma list of hex ids (`RZAtoUint32HexA`, §1.6). Each line
is handled on its own, so **one section may hold both keys, and several of each** *[compat]* (NR3).

Linking `R` into a rotation ring with head `F` and previous member `P` (both rule records of the current line) means:
`R.prev := P.id`, `R.next := F.id`, `P.next := R.id`, `F.prev := R.id`. The ring stays closed after every step.

**`RotationRing`.** `K := −1`. `F := P := none`. For each token, in order, with `id` its value:

1. If `K < 0` (signed): `K := id`. The first token sets `K` even when no rule has that id.
2. Look `id` up in the rule map. Unknown: next token.
3. If `P` exists, link `R` into the rotation ring as above. Then `P := R`, and `F := R` when `F` is none.

So the listed rules form one rotation ring in list order, the unknown ids left out. Each rule keeps its own type
ring. If the first id has bit 31 set, `K` stays "negative", so the next token overwrites it *[quirk]*.

**`AddTypes`.** `cur := K` (the head of the previous `RotationRing` or `AddTypes` line of the file, or −1). `F := P :=
none`. For each token, in order:

1. If `F` is none: `K := id`. So `K` ends as the line's first **known** id, or as its last token when none is known.
2. Look `id` up. Unknown: next token (`cur` does not move).
3. Link `R` into this line's rotation ring exactly as `RotationRing` does; `P := R`; `F := R` when `F` is none.
4. Look `cur` up. If it is a rule `C`: `N :=` the rule whose id is `C`'s type-ring next; then `R.typePrev := C.id`,
   `R.typeNext := N.id`, `C.typeNext := R.id`, `N.typePrev := R.id` (`R` goes into `C`'s type ring right after
   `C`); then `cur := C`'s rotation next. If `cur` is unknown, `R` joins no type ring and `cur` stays.

So the k-th known rule of an `AddTypes` line forms a rotation ring with the line's other rules and enters the type ring
of the k-th rule of the base ring (counted from the base line's head along its rotation ring). The next `AddTypes`
line then uses this line as its base. For example, the lines `RotationRing = A1,A2`, `AddTypes = B1,B2` and
`AddTypes = C1,C2` give the rotation rings `A1 → A2`, `B1 → B2`, `C1 → C2` and the type rings
`A1 → B1 → C1 → A1` and `A2 → B2 → C2 → A2` (each new rule goes right after its base rule, and the base of the `C`
line is the `B` line). `N` is fetched with the map's insert-or-find operator (`0x0062C2F0`); the links only ever
hold ids of existing rules, so it always finds one.

These links only drive the intersection tool's user interface; the solver never reads them.

### 10.9 The intersection pieces at commit

`InsertHighwayIntersectionPieces(list)` (`0x0062C4B0`) runs at commit (§10.7 step 9; network-tools.md §6.1). For each
record of `+0xCC` (§10.6) whose start step is below `+0x78`, in order, it calls
`InsertHighwayIntersection(x, z, rule id, list, real = +0x50, 0)` (`0x0062C3A0`). That function does nothing unless
`real`. Otherwise, for each occupied cell `(dx, dz)` of the rule (§10.3), in list order, when the tool has a cell
info at `(x + dx, z + dz)` and `GetTileInfo(dx, dz)` finds a tile:

- `pieceId := AutoTileBase (+0xC8) + tile`;
- `path := AutoPathBase (+0xCC) + tile` when `AutoPathBase` is not 0, else 0;
- `InsertHighwayIntersectionPiece(info, list, usable = 0, pieceId, orientation, x, z, path)`.

The tool's real byte is saved and restored around the loop.

**GetTileInfo(dx, dz)** (`0x00624F00`): `col := dx − minX`, `row := dz − minZ`. If `col < width` and
`row × width + col <` the grid size (both unsigned), the code is the grid byte there, and the result is that code's
tile-map value (§10.3); otherwise, or when the code is not in the map, none. So the cell at `(col, row)` of a rule's
bounds gets piece `AutoTileBase + col × 0x100 + row × 0x10` *[compat]*, turned by the map entry's orientation.

**InsertHighwayIntersectionPiece(info, list, usable, pieceId, orientation, x, z, path)** (`0x006276D0`, pop 32):

1. `occ :=` tool slot 54 `CreateNetworkOccupant(info, …)` with the prebuilt-model class `0x49C1A034` (§11).
2. `SetExemplarForOccupant(occ, pieceId, …, orientation)` with group `0xA8434037` (§11).
3. Network occupant slot 30 (`0x0061B490`) sets its piece id `+0xFC` and orientation `+0x104` and drops a cached path
   info.
4. Network flags: clear `0x2000000` and `0x4000000`, set `0x8000000`. Set `0x1000000` when the rounded sum of the
   corner heights `+0x5C` and `+0x64` is below the rounded sum of `+0x58` and `+0x60` (`0x00627784`); this picks the
   quad's split diagonal *[inferred]*. Then the shear transform is recomputed (`0x0061D230`, drawing only).
5. If `path ≠ 0`: network occupant slot 51 (`0x006197D0`) stores it as the **path override** (`+0x164`, §11.1).
6. Clear the cell's network occupants (`ClearNetworkOccsFromCell(info.x, info.z, +0x364)`).
7. When the tool has an occupant manager (`+0x48`): `info.+0x54 :=` the result of its slot 20 called with the
   occupant and the tool's `+0x208`.
8. If `usable`: set `0x10000000` and clear `0x4000`. Always set `0x500000` (`0x100000` is the intersection flag that
   `Test` reads, §10.5).
9. Network occupant slot 72 (`0x0061BA90`): when the occupant has property `0x4835C6AB`, store the anchor cell key
   `(z << 16) + x` in it (the anchor that `Test` compares through slot 71). An occupant without the property keeps no
   anchor.
10. If `list` is not null: `AddRef` the occupant and append it. Release the creation reference.

Every piece of one intersection gets the same anchor, the rule's position `(x, z)`.

---

## 11. From a solved cell to an occupant

A solved record `{piece id, orientation, key}` becomes an occupant at commit (`InsertOccupantsForSolvedCells`, slot
55, `0x006258C0`; network-tools.md §6.2). The record's piece id becomes the occupant's **piece id** (network occupant
slot 23, occupant `+0xFC`) and its orientation byte the occupant's rotation and flip (slot 26 returns it, occupant
byte `+0x104`; §9 reads it back).

- An **ordinary** network occupant (`CreateNetworkOccupant`, slot 54, `0x00624210`, with no class id) is a textured
  cell: its piece id names the cell's texture. The multi-type solutions file calls the field `textureId`. The texture
  lookup belongs to the network rendering, outside this spec (§14).
- A **prebuilt-model** occupant (class `0x49C1A034`) takes its model from an exemplar: `GetOccupantExemplar(piece,
  group)` (`0x006244B0`) fetches the resource with key `{type 0x6534284A, group, instance = piece id}` through the
  resource manager (slot 3, interface `0xA52160F5`), and `SetExemplarForOccupant` (`0x00624860`) adds the model when
  it is found. `group` defaults to the tool's type record `+0x0C` (§2.2); `InsertHighwayIntersectionPiece`
  (`0x006276D0`) passes `0xA8434037` (the Highway group) for the intersection models of §10.

So a rule file names pieces by id only; what the id selects depends on the occupant class and, for prebuilt models,
on the group *[compat]*.

### 11.1 The paths of a piece (what the simulation reads; NR1)

The simulation never reads a piece's texture. It reads the piece's **path info** (traffic-network.md §7.3), which the
piece id also selects. `GetPathInfo` (network occupant slot 49, `0x0061B510`) builds it on the first call and caches
it in the occupant (`+0x140`):

1. `id :=` the piece id (slot 23), `group := 0x69668828`. If the occupant has a **path override** (`+0x164`, set by
   slot 51, §10.9), `id :=` the override and `group := 0xA966883F`.
2. `instance :=` network manager slot 15 `MapPath(id)` (network-manager.md §7.11: the left-hand-traffic remap).
3. Load the resource `{type 0x296678F7, group, instance}` through the resource manager (slot 3, interface
   `0xA9667793`). The result is a path info shared by every occupant with that key. When there is no such resource,
   `GetPathInfo` returns null and caches nothing, so the next call tries again.
4. With an override: when the loaded path info does not use elevation (path-info slot 16), it is cloned (slot 24) and
   the clone's elevation flag is set (slot 17).
5. The occupant's own copy: path-info slot 25 `CreateTransformedCopy(occupant)`, which places the paths at the
   occupant's cell and orientation; the loaded copy is released.
6. With an override: slot 29 `OffsetAndScissor(+0x168, 0, +0x16C, bounds)`, where the bounds are the occupant's
   fields `+0x9C`, `+0xA4`, `+0xCC`, `+0xD4`; the copy it returns replaces the cached one, which is released. So an
   intersection piece's override names the paths of the whole intersection, and each piece keeps the part inside its
   own cell *[inferred]*.
7. Slot 26 `RemoveWrongWayPaths(occupant)` (it reads the one-way byte that slot 69 returns, §6.5), then slot 40
   `VerifyPathEdges(occupant)`.
8. The occupant's connection bitmap (`+0x144`, 32 bytes) is cleared and rebuilt by slot 27
   `InitializeNetworkConnectionBitmap(occupant)`.

The orientation byte therefore turns the paths through step 5. The texture an ordinary piece id selects, and how the
orientation turns it, is drawing only; it is left to the renderer specs. What the path resource holds and exactly what
slots 25, 26, 27, 29 and 40 do is NR6. The path-info slots are listed in traffic-network.md §7.3.

---

## 12. Answers given here

- **NG5** (network-geometry.md §17, QUESTIONS.md): the rule-driven piece solver. §2 (the resources and their load
  order), §3–§5 (the tile rule files, `#Transmogrify`, matching), §6 (`SolveNetwork`'s rule stages,
  `SolveUsingEvaluator`, `ApplyNetworkRules`), §7 (edge subtypes), §8 (multi-type intersections), §9 (tile subsets),
  §10 (`cIntRule`, `IdentifyHighwayIntersections`, `InsertIsolatedHighwayIntersection`), §11 (piece ids). The
  network-geometry.md §8.2 outline MATCHES, with one addition: `SolveNetwork` runs the evaluators of **all 13 types**,
  not only the tool's (§6.1 step 5).

Answered 2026-10-16 (re-answers24), from the executable:

- **NR3** (§10.8): `+0xF4`/`+0xF8` are a second circular list of rule ids, the **type ring**: MATCHES the inferred
  shape. But the premise that `AddTypes` is read only when there is no `RotationRing` **DIFFERS**: the callback tests
  each line's key on its own, so both keys may share a section, and each `AddTypes` line also forms its own rotation
  ring. A rule of an `AddTypes` line enters the type ring of the rule at the same position of the previous
  `RotationRing`/`AddTypes` line of the file, right after it.
- **NR4** (§6.5, §8.3): `ResolveOneWayConflicts` and `SolveMultiTypeSeparation` specified; the one-way codes that
  `AssignEdgeMajorTypes` and `DoAdditionalDraws` write are in §6.5. `SolveMultiTypeSeparation` only draws and changes
  no state.
- **NR5** (§9.1): file order MATCHES. The INI enumeration keeps repeated keys in file order, and the multimap insertion
  puts an equal `idA` after the existing entries.
- **NR1**, simulation part (§11.1): the piece id (or the path override) also selects the occupant's path info through
  `MapPath` and the path resource; the orientation turns the paths when the occupant makes its transformed copy. The
  texture lookup is drawing only and is left to the renderer specs. The path-info internals are NR6.
- **NR2** (§10.3, §10.9): the tile grid and map, `AutoTileBase`, `AutoPathBase`, `InsertHighwayIntersection(s)` and
  `InsertHighwayIntersectionPiece` specified. The constraint layout's consumer is named
  (`InsertIntersectionConstraints`, `0x00644710`, height geometry); its codes' meaning is NR7.

Answered 2026-10-21 (re-netrules-boundary4), from the executable:

- **NR13** (§2.4): there is **no** game-reachable boundary of the solver family of its own. `cSC4TileEvaluator` is a
  0x10-byte plain object with a one-slot vtable (deleting destructor) and no RTTI, created only by `LoadEvaluator`;
  `cSC4TileRule` is a stack struct; `cSC4TileInfo` is free functions; `cIntRule` lives in an image-global map. All
  callers are direct calls inside the network tool and, for `cIntRule`, the puzzle-piece input control; no image
  pointer refers to any of them. The supported hosts are the rule resources (data) and the whole network tool,
  reachable only through a replacement network manager (class `0xC990BD46`). So the premise of the question MATCHES:
  a rule solver of ours is a component of our own network tool, with no switch of its own.

---

## 13. Quirks (summary)

1. A flipped rule output written with `rot` 4 or more reads past the rotation table `0x00AA8328` when a
   `#Transmogrify` file rotates it (§4.1).
2. The mirror drops bits 6–7 of an orthogonal edge byte's subtype (§4.2).
3. A connection-node cell qualifies for the rules only when its occupant's masked type bits equal the cell's **whole**
   flags word, so a node cell with a non-type flag never qualifies (§6.4).
4. `typeOf` reads the GroundHighway bit and every non-type flag bit (`0x40000`, `0x80000`) as type 12, so a
   bridge-step cell of one type is handled as a two-type cell in §8.2 (§8.2, test 21).
5. After a hard conflict in `AdjustTileSubsets` (resolve mode), the removed cell's `+0xB8` keeps its old index (§9.1).
6. A cell layout without `<` or `^` shifts its cells by +1 (§10.3).
7. `Transpose` does not mirror an edge byte that carries subtype bits (§10.4).
8. `Rotate` by an odd number of turns sets `maxX := −(old minX)` instead of `−(old minZ)` (§10.4, test 18).
9. `IdentifyHighwayIntersections` tags its last run's potential-intersection entry with 4 instead of the type, and
   returns its `v` argument on every path (§10.6; network-geometry.md quirk 7).
10. The keys `PieceType` and `ResetPieces` are built by the reader and never matched (§10.1).
11. The one-way conflict sweep marks a neighbour whose code it cleared (`+0x57`) but queues the current cell's key
    instead of the neighbour's, so that neighbour is never added to the solve set (§6.5, test 26).
12. A `RotationRing` whose first id has bit 31 set leaves the ordering cursor "negative", so the next token becomes
    the cursor (§10.8).
13. Tile codes are one byte (`row × 16 + col + 1`), so cells of a layout wider than 16 columns, or past code 255,
    share one tile-map entry (§10.3).

---

## 14. Open questions

- ~~**NR1.**~~ Simulation part answered (§11.1, §12). The texture an ordinary piece id selects, and how the
  orientation byte turns it, belongs to the renderer specs.
- ~~**NR2.**~~ Answered (§10.3, §10.9), except the constraint codes (NR7).
- ~~**NR3.**~~ Answered (§10.8, §12).
- ~~**NR4.**~~ Answered (§6.5, §8.3).
- ~~**NR5.**~~ Answered: file order (§9.1).
- **NR6.** The path resource (`{0x296678F7, 0x69668828 or 0xA966883F, MapPath(id)}`, interface `0xA9667793`): its
  file format, and what path-info slots 25 `CreateTransformedCopy`, 26 `RemoveWrongWayPaths`, 27
  `InitializeNetworkConnectionBitmap`, 29 `OffsetAndScissor` and 40 `VerifyPathEdges` do (§11.1). Also who writes the
  override offset `+0x168`/`+0x16C` that slot 29 receives. Needed by the traffic rebuild when it builds paths itself
  instead of borrowing the original's.
- **NR7.** The constraint layout's codes 1–8 as `InsertIntersectionConstraints` (`0x00644710`) applies them: which
  vertex heights each code makes equal and which slope limits it sets (§10.3; network-geometry.md §9.4 step 4).
- ~~**NR13.**~~ Answered (§2.4, §12): no boundary of its own; hosted by the network tool or the rule resources.
- **NR12** (cores22: should `fc4_network_rules_cell` grow `+0x57` and the occupant's paired portal, or should a
  glue lane pass them another way?) — **ANSWERED 2026-10-16 (re-answers31): neither belongs in the cell struct
  as a carried field.** Both are *runtime* state of the one-way pass, not properties a cell keeps:

  - **`+0x57` is per-pass scratch on the world-cache cell info, not a cell fact.** §6.5 step 2.1 sets it to 1 and
    adds the cell's key to the solve set `+0x84` the first time the pass visits the cell, and step 3.1 does the
    same when the sweep reaches a neighbour (`decomp/cSC4NetworkTool.c:11360`). Nothing outside
    `ResolveOneWayConflicts` reads it, and it is not saved. Model it as a mark on whatever object stands for the
    game's cell info — the one `GetCellInfo` returns — scoped to one pass, **not** as a field of an ABI-stable or
    persisted `fc4_network_rules_cell`. If the C boundary needs it, pass a scratch/marks table alongside the
    pass rather than growing the shared struct.
  - **The paired portal is a property of the *occupant*, not the cell.** It is reached only when the cell's flags
    carry `0x80000` (a tunnel portal), through interface **`0x8A4BD52B` slot 59**, on the occupant's
    `+0x1D0` (§6.5 step 2.3). Copying it into the cell struct would duplicate a live pointer the cell does not
    own and would go stale. Resolve it on demand from the occupant when the portal flag is set.

  So: do **not** grow `fc4_network_rules_cell` with either. The one-way pass needs a marks side-channel for
  `+0x57`, and portal resolution goes through `0x8A4BD52B` slot 59 when `0x80000` is set. Whether the C entry
  points `resolve_one_way_conflicts`, `solve_multi_type_separation`, `get_tile_info` and `intersection_pieces`
  are exposed at all is a separate decision — the specs do not require them to be, only their behaviour.

---

## 15. Conformance tests

Unless stated, a test runs on a flat dry 64 × 64 city with the given cell infos; "Road `e`" means a cell info whose
flags are the Road bit and whose Road edge word is `e`. Rule files are synthetic. Tests 1–9 need only the file reader
and the evaluator; the others drive the named function directly.

1. **Edge tokens** (§3.3). `2` → `0x02`; `13` → `0x0B`; `27` → `0x17`; `260` → `0x04` (260 mod 256 = 4); `0` → 0.
2. **One rule** (§3). The file `#TestRules#` / `1,0,2,0,2` / `3,0,0x100,0,0` gives one rule: key `0x02000200`,
   no conditions, one output `{id 0x100, m 0, orientation 0x00}`, transform mask `0xFF`.
3. **Rejected header** (§3.2). The same file with the first line `TestRules` gives no rules; the evaluator exists and
   never matches.
4. **Bad centre** (§3.3). `#T#` / `2,0,0,0,0` / `3,0,0x100,0,0`: the first rule's centre token is not 1, so loading
   stops with no rules.
5. **Transmogrify, straight** (§4). `#Transmogrify` / `1,0,2,0,2` / `3,0,0x100,0,0` expands to exactly two rules:
   `0x02000200` → `{0x100, orientation 0x00}` and `0x00020002` → `{0x100, orientation 0x01}`. Turns 2 and 3 and all
   four mirrored variants repeat a pattern and are dropped.
6. **Transmogrify, diagonal** (§4). `#Transmogrify` / `1,1,3,0,0` / `3,0,0x200,0,0` (the north-west diagonal
   `0x00000301`) gives four rules: `0x00000301` rot 0, `0x00030100` rot 1, `0x03010000` rot 2, `0x01000003` rot 3,
   all unflipped. The mirror of `0x00000301` is `0x00030100`, the same pattern as turn 1, so it is dropped even though
   its output is flipped. With `noflip` before the centre line the result is the same.
7. **Mirror of a subtype** (§4.2). The edge byte `0x12` (token `22`) mirrors to `0x22` (token `42`); `0xBA` (token
   `232`) mirrors to `0x3A`; `0x0B` (token `13`) mirrors to `0x09` (token `11`).
8. **Rotation tables** (§4.1). Metaposition 1 turned once → 3; 9 turned once → 13; 12 turned three times → 24.
   Orientation `0x03` turned twice → `0x01`; `0x81` turned once → `0x80`.
9. **Conditions and wildcards** (§5). Rule `#T#` / `1,0,2,0,0` / `2,3,0,0,0,2` / `3,0,0x200,0,0`. Road `0x00000200`
   at (10, 10). With Road `0x02000000` at (10, 9): match, output 0x200. With Road `0x02000200` at (10, 9): no match
   (the N byte differs under mask `0xFF`). With the condition `2,3,?,?,?,2` instead: match in both cases.
10. **Valid positions** (§5.1). Rule key `0x00020000` with outputs `(m 0, 0x300)` and `(m 5, 0x301)`; Road
    `0x00020000` at (10, 10). If (11, 10) has a cell info carrying the Rail bit, the rule fails; if (11, 10) has no
    cell info or only the Road bit, it matches.
11. **Claims** (§6.4). One evaluator with rule key `0x02000200`, outputs `(m 0, 0x100)` and `(m 7, 0x101)`. `S = P`
    = {(5, 5), (5, 6)}, both Road `0x02000200`, no other cell infos. With 193 buckets, (5, 6) (key `0x00060005`,
    bucket 80) comes before (5, 5) (key `0x00050005`, bucket 164). Result: records `{0x100, 0, (5, 6)}` then
    `{0x100, 0, (5, 5)}`; both `m 7` outputs are skipped ((5, 7) is in neither set; (5, 6) is already claimed); `S`
    ends empty.
12. **Two types crossing** (§8.2). A cell with the Road and Rail bits, Road `0x02000200`, Rail `0x00020002`, tool type
    Road. Rail wins (prio 5 > 3); not covered; no arm is cleared. With `[RoadRailIntersectionSolutions]` holding
    `k=0x02000200,0x00020002,0x1234,1,0`, the cell gets the record `{0x1234, 0x01}`, leaves `S` and `P`, and
    `(x, z, 0x3)` is appended to `+0x08`. Without the entry the cell stays in `S` and no evaluator takes it (two type
    bits), so it is unsolved.
13. **Covered, with override** (§8.2). Road and Street bits, both `0x00020002`. Road wins (prio 3 > 2) and covers
    Street. With `[AllowedOverrides]` `k=Road,Street`: the Street edges become 0 and the flags become the Road bit
    alone; the cell stays in `S` and the Road evaluators solve it. Without the override: nothing changes.
14. **Three types** (§8.2). Road `0x00020002`, Rail `0x02000200`, Street `0x00020002`: A = Road, B = Rail, C = Street.
    The only qualifying pair is (Street, Road), so Street is removed, then the cell is handled as test 12.
15. **Scheme 2** (§7.2). Rail `0x00000201` (W kind 1, N kind 2) in `P`, not fixed: the N byte becomes `0x12`
    (f = 2: the west diagonal leans into N), giving `0x00001201`. The W byte is unchanged (its opposite side is empty).
    If the north neighbour is in `P`, its S byte becomes `0x12`.
16. **Scheme 1** (§7.1). Road `0x00020001` (W kind 1, E kind 2) at (10, 10), in `P`. If (9, 10) is Road `0x00030100`
    (purely diagonal), (10, 10) becomes `0x00020009`, and if (9, 10) is in `P` its E byte becomes `0x0B`
    (`0x000B0100`). If (9, 10) is Road `0x00020000`, (10, 10) becomes `0x00020002`.
17. **PatchTilePair** (§9.1). Pattern `0x10,1,0,0x20,1,0` → replacement `0x11,1,0,0x21,1,0`. A = (0x10, `0x00`) with
    B = (0x20, `0x00`) on A's north side (s = 1): both normalise to rotation 1, the pattern matches, and A becomes
    (0x11, `0x00`), B (0x21, `0x00`); the result is 1. With the replacement `0,0,0,0,0,0` the result is 2, and
    `AdjustTileSubsets(resolve = false)` returns false.
18. **Rotate bounds** (§10.4). A rule whose occupied cells are (−1, 0) … (2, 0) has bounds x ∈ [−1, 2], z ∈ [0, 0].
    After `Rotate 1` the cells are (0, −1) … (0, 2) and the bounds are minX 0, **maxX 1**, minZ −1, maxZ 2.
19. **Layout and centre** (§10.3). `CheckType=a - Road:0x00020002` and the layout lines `.a+<`, `..^`: the centre is
    (column 2, row 0); the tagged cell `a` is at (−1, 0), the new cell `+` at (0, 0); bounds x ∈ [−1, 0], z ∈ [0, 0].
20. **Test** (§10.5). One tagged cell `a - Road:0x00020002` at (0, 0), no new cells. At (10, 10): Road `0x00020002`
    → 0; Road `0x02000200` → 2; Rail only → 1; Road and Street bits with Road `0x00020002` → 1 (not multi-ok), 0 when
    the tag has `multiok`; no cell info → 2.
21. **Bridge cell as two types** (§8.2, quirk 4). A cell with flags `0x1 | 0x40000` and Road `0x00020002`, tool type
    Road: it is read as Road and type 12; type 12 wins (prio 5 > 3) and does not cover. With
    `[RoadGroundHighwayIntersectionSolutions]` holding `k=0x00020002,0x00000000,0x4321,0,0`, the cell is solved by
    that entry; without it, the Road evaluators solve it as usual.
22. **OneWayDir** (§10.4). `OneWayDir = 1`: after `Rotate 1` it is 3; after `Transpose` it is 5.
23. **IdentifyHighwayIntersections** (§10.6). Any input returns `v` (for example `v = 7` → 7). A run that lasts to the
    last step appends `(x, z, 4)` to `+0x08`, whatever the type.
24. **TileVerifiers** (§9.2). A verifier for piece `0x100` with one requirement `5,0x101,0,0` (the east neighbour is
    piece `0x101`, same orientation). A record `{0x100, 0x01}` at (10, 10) looks at (10, 11) (metaposition 5 turned
    once); it is kept when (10, 11) holds `{0x101, 0x01}` and removed (back into `S`, result false) otherwise.
25. **RuleOverrides order** (§9.1, NR5). The section holds, in this order, `0x10,1,0,0x20,1,0=0x11,1,0,0x21,1,0` and
    `0x10,1,0,0x20,1,0=0x12,1,0,0x22,1,0` (the same key twice). Both entries are kept. With the pair of test 17, A
    becomes `0x11` and B `0x21`: the first line wins. With the two lines swapped, A becomes `0x12`.
26. **One-way propagation and sweep** (§6.5, quirk 11). OneWayRoad cells; the dragged run is D (9, 10) as step 0 and
    X (10, 10) as step 1 (`+0x70 = 2`), both with the in-run byte set and in the solve set (`+0x57 = 1`). Edge words
    and one-way bytes: D `0x00020000`, `0x11`; X `0x00000202`, `0x15`; Y (10, 9) `0x02000200`, `0x44`; Z (10, 8)
    `0x02000000`, `0x40`. (8, 10) has a cell info that is not in the run and not in the solve set, with one-way byte
    `0x10`. (11, 10) has no cell info. After `ResolveOneWayConflicts(10)`: X `0x09`, Y `0x88`, Z `0x80` (the flow of
    the existing road is reversed to leave X northward); D `0x10`; (8, 10) `0x00`. Y and Z have joined the solve set.
    (8, 10) has `+0x57 = 1` but is **not** in the solve set.
27. **Ordering rings** (§10.8, NR3). Rules A1, A2, B1, B2, C1, C2 exist. One section `Ordering1` holds
    `RotationRing=A1,A2`, `AddTypes=B1,B2`, `AddTypes=C1,C2`. Rotation next: A1 → A2 → A1, B1 → B2 → B1,
    C1 → C2 → C1. Type next: A1 → B1 → C1 → A1 and A2 → B2 → C2 → A2. With `AddTypes=B1,0x999,B2` (0x999 unknown)
    instead of the B line, the result is the same.
28. **Intersection tiles** (§10.3, §10.9). A rule with `AutoTileBase=0x5000`, `AutoPathBase=0` and the occupied cells
    at (col, row) (0, 0), (1, 0), (2, 0) and (0, 1) of its bounds. At commit the four pieces are `0x5000`, `0x5100`,
    `0x5200` and `0x5010`, each with orientation 0 (no transform) and no path override; when the occupants carry
    property `0x4835C6AB`, each one's anchor is the rule's position. With `AutoPathBase=0x7000`, the overrides are `0x7000`, `0x7100`, `0x7200` and `0x7010`.
29. **Preview piece** (§8.3). With the evaluator of test 11 and Road `0x02000200` at (5, 5) and (5, 6),
    `SolveMultiTypeSeparation((5, 5), Road)` returns true with id `0x100`, rot 0, flip 0, and the solved list stays
    empty. A Road cell `0x00000002` (no rule) returns false.
