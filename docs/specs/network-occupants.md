# cSC4NetworkOccupant and its family (behaviour spec)

Status: network-occupants spec, written 2026-10-20 (re-netocc lane). Every fact was checked on the Windows
executable `bin/SimCity4.exe` (1.1.641, image base `0x00400000`, SHA-256
`3bc5c7fe807fe5aa24a3abb06e40f13486670bc1150612c09baf4ecedafe1a4b`); every address is a Windows address. Numeric
details (float or double, operation order, widths) were checked against the instruction bytes. Implementers work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult
decompiled or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Tags: *[compat]* is required for compatibility with the original game and its plugins; *[choice]* is a behaviour we
chose where the original was arbitrary or undefined; *[inferred]* is consistent with all evidence but not proven;
*[quirk]* is surprising original behaviour that is kept on purpose.

`r24(e)` is `e` rounded to float32 (foundations §18). All field offsets are **from the object base**, which is the
class's primary interface pointer (`cISC4NetworkOccupant` for the network-piece classes, the class's own interface
for the others), unless a section says otherwise.

**Scope.** The in-world objects the network tools place: the *network occupant* family. The piece occupant
(`cSC4NetworkOccupant` and its subclasses) is the geometry of a network tile — the object that carries the piece's
model, its terrain-following triangle soup, its connection matrix and its network flags. Alongside it: the power
pole and power line occupants, the pipe and subway occupants, the underground connection occupants, and the
dispatch pylon. This spec gives the identity of every class, the object layout, the lifecycle (`Init` /
`Shutdown`), the interface slot tables with return channels, what each class places on the grid, and the save
record.

**What is not here:** the tools that decide *which* piece to build (network-tools.md, network-geometry.md), the
piece solve and height solver (network-geometry.md §8–§9), supports and bridges (network-geometry.md §10,
network-bridges.md), the network manager and lot manager (network-manager.md), the traffic rebuild that consumes
paths (traffic-network.md), the power and water simulations that consume poles, lines and pipes (power.md,
plumbing.md).

**Shared material cited and not repeated here:**

- [foundations.md](foundations.md) §2.3 (`gzstr`), §2.7 (references and the class store), §3.3, §3.5 (save/load
  order), §10.3 (exemplar property holders), §18 (x87 precision).
- [occupant-manager.md](occupant-manager.md) (insertion, the spatial index, the flags byte, threading).
- [lot-records.md](lot-records.md) §2.3 (the occupant base record), §14 (references and resolution on load).
- [network-manager.md](network-manager.md) §5 (the network flags table — this spec does not repeat it), §6.1
  (the `managed` list), §8.5–§8.6 (how the lot manager finds and decorates a piece).
- [network-geometry.md](network-geometry.md) §2.3/§2.4 (the network type info tables), §8.1 (corner heights),
  §10 (creation, supports, flags).
- [network-bridges.md](network-bridges.md) §11.2 (bridge piece creation), [traffic-network.md](traffic-network.md)
  §4.8 (tunnel portals), §7.2 (slots 48–54).
- [power.md](power.md) §5.1/§5.4 (pole and line consumers), [plumbing.md](plumbing.md) §5.2/§9 (pipe consumers),
  [dispatch.md](dispatch.md) §4.1 (pylon slots), [demolition.md](demolition.md) §4.2/§4.3 (demolition dispatch).

---

## 0. Summary

The network-piece family is rooted in `cSC4NetworkOccupant` (class ID `0xC9C05C6E`), which derives from
`cSC4Occupant` and adds the `cISC4NetworkOccupant` interface (81 slots in the shipped SDK order, extended by the
underground subclasses). On top of it sit the pre-built-model subclass (which owns a real exemplar model), the
bridge and tunnel pieces, and the three underground pieces (base, pipe, subway). The power pole, power line,
underground connection and pylon classes do **not** derive from `cSC4NetworkOccupant` — they derive directly from
`cSC4Occupant` and answer their own interfaces.

One object, many roles. A single road tile's occupant holds: the four corner points the tool solved (slot 3), the
cell's network flags (`+0x13C`), the 4 × 4 connection byte matrix (`+0x144`), the four road quads and the triangle
soup that draw the tile, the cached path info the traffic simulator walks (`+0x140`), and the demolition cost
(`+0x170`). The network lot manager finds lots *through* these objects' flags, not through a back-pointer
(network-manager.md §8.5); a network lot's `occupant` reference points at the piece that covers its cell.

Implementation size: about 2 500 lines for the whole family — the base class and its record (≈600), the prebuilt
subclass (≈500), the underground family (≈700), the pole and line pair (≈500), the connection occupants (≈150,
mostly shared) and the pylon (≈200) — plus the type-info/routines tables (§11, §12) which are data and one
function.

---

## 1. Identity and inheritance

### 1.1 The class table

| Class | Class ID | Occupant type (u32, `GetType` slot 7) | Primary interface (vtable) | Object notes |
|---|---|---|---|---|
| `cSC4Occupant` (base of all) | `0xA9BC9AB6` | `0x74758925` (bare occupant) | `cISC4Occupant` `0x00AA41D8` | ctor `0x005E9220` |
| `cSC4NetworkOccupant` | `0xC9C05C6E` | `0xC772BF98` (network piece) | `cISC4NetworkOccupant` `0x00AA8080` | ctor `0x0061BDD0` |
| `cSC4NetworkOccupantWithPreBuiltModel` | `0x49C1A034` | `0xC772BF98` | `cISC4NetworkOccupant` `0x00AA7038` | ctor `0x0061E1A0`; sets flag `0x400000` at `+0x13C` |
| `cSC4NetworkBridgeOccupant` | `0x49CC1BCD` | `0xC772BF98` | `cISC4NetworkOccupant` `0x00AA7300` | ctor via prebuilt `0x00605180` (`0x006051F0`'s callee) |
| `cSC4NetworkTunnelOccupant` | `0x8A4BD52B` | `0xC772BF98` | `cISC4NetworkOccupant` `0x00AA9308` | ctor `0x006476F0` |
| `cSC4UndergroundOccupant` | `0x4A10AFC2` | `0xC772BF98` (no override; §3.2) | `cISC4NetworkOccupant` `0x00AAAD40` | extended underground interface (106 slots) |
| `cSC4PipeOccupant` | `0x49C05B9F` | `0xC772BF98` | `cISC4NetworkOccupant` `0x00AA97B8` | 110 slots |
| `cSC4SubwayOccupant` | `0xCA16374F` | `0xC772BF98` | `cISC4NetworkOccupant` `0x00AAA3C8` | 112 slots |
| `cSC4UndergroundConnectionOccupant` | `0x8A139184` | none set by the base ctor (subclasses set it, §6.4) | `cISC4UndergroundConnectionOccupant` `0x00AAA840` (25 slots) | ctor `0x00654F00` |
| `cSC4PipeConnectionOccupant` | `0xEA0A5ACF` | `0x2A183167` (set by `Init`) | `0x00AA9578` (45 slots) | ctor `0x006483E0` |
| `cSC4SubwayConnectionOccupant` | `0xCA15ED6B` | `0x8A13918A` (set by `Init`) | `0x00AAA188` (45 slots) | ctor `0x00652C50` |
| `cSC4PowerPoleOccupant` | `0x09C05C6A` | `0x2890D4DE` | `cISC4PowerPoleOccupant` `0x00AA9EB0` (19 slots) | ctor `0x0064DB30`, object `0x124` bytes |
| `cSC4PowerLineOccupant` | `0xC9C05C5D` | `0x0990B2EA` | `cISC4PowerLineOccupant` `0x00AA9C80` (8 slots) | ctor `0x0064A270` |
| `cSC4PylonOccupant` | `0x49B6D69B` | `0xA9B6D7DE` | `cISC4PylonOccupant` `0x00AA5150` (21 slots) | ctor `0x005F1730` |

All eleven class IDs of §1.1 are registered in the exe's class-id table between `0x00B08D40` and `0x00B08E18`
(`re/class_id_table.tsv`): pylon `0x00B08D40`, bridge `0x00B08DC8`, plain `0x00B08DD8`, prebuilt `0x00B08DE8`,
tunnel `0x00B08DF0`, pipe `0x00B08DF8`, power line `0x00B08E00`, pole `0x00B08E08`, subway `0x00B08E10`,
underground `0x00B08E18`. The two connection classes (`0xEA0A5ACF`, `0xCA15ED6B`) and the connection *types* are
not in that table — they reach saves as class IDs on their records instead (save-records.md §3.2 Q5, answered:
the classes exist and are read from saves).

The occupant types are what `cISC4Occupant::GetType` (slot 7) returns and what the demolition dispatcher and the
simulators dispatch on. The plain network-piece classes all share `0xC772BF98`: they are told apart only by
`QueryInterface` (§1.2) — there is no per-subclass occupant type for bridge, tunnel, prebuilt, underground, pipe
or subway *[inferred from the absence of a type write in their constructors and from the consumers, which all QI]*.

The type constants live in the *type info* table `0x00AA7CE8` (13 dwords, one per network type index, §11) and in
`IsOfType` (§1.4).

### 1.2 Inheritance and `QueryInterface`

Inheritance, from the constructors' vtable writes (each row lists the object's sub-object pointers):

| Class | `+0x00` | `+0x04` | `+0x08` | `+0x0C` | other |
|---|---|---|---|---|---|
| `cSC4NetworkOccupant` | `cISC4NetworkOccupant` `0x00AA8080` | `cISC4Occupant` `0x00AA7FF8` | `cIGZSerializable` `0x00AA7FDC` | property holder `0x00AA7F78` | `cS3DModelInstanceBasic` at `+0x68` |
| `…WithPreBuiltModel` | `0x00AA7038` | `0x00AA6FB0` | `0x00AA6F94` | `0x00AA6F30` | as the base |
| `…BridgeOccupant` | `0x00AA7300` | `0x00AA7278` | `0x00AA725C` | `0x00AA71F8` | as the base |
| `…TunnelOccupant` | `0x00AA9308` | `0x00AA9280` | `0x00AA9264` | `0x00AA9200` | as the base |
| `cSC4UndergroundOccupant` | `cISC4NetworkOccupant` `0x00AAAD40` | `cISC4Occupant` `0x00AAACB8` | `cIGZSerializable` `0x00AAAC9C` | property holder `0x00AAAC38` | model instance at `+0x68` |
| `cSC4PipeOccupant` | `0x00AA97B8` | `0x00AA9730` | `0x00AA9714` | `0x00AA96B0` | as the base |
| `cSC4SubwayOccupant` | `0x00AAA3C8` | `0x00AAA340` | `0x00AAA324` | `0x00AAA2C0` | as the base |
| `cSC4UndergroundConnectionOccupant` | `cISC4UndergroundConnectionOccupant` `0x00AAA840` | `cIGZSerializable` `0x00AAA824` | property holder `0x00AAA7C0` | `cISC4Occupant`-shaped `0x00AAA7A8` at `+0x1C` | model instance at `+0x38` |
| `cSC4PipeConnectionOccupant` | `0x00AA9578` | `0x00AA955C` | `0x00AA94F8` | — | as the connection base |
| `cSC4SubwayConnectionOccupant` | `0x00AAA188` | `0x00AAA16C` | `0x00AAA108` | — | as the connection base |
| `cSC4PowerPoleOccupant` | `cISC4PowerPoleOccupant` `0x00AA9EB0` | `cIGZMessageTarget2` `0x00AA9EA0` | `cISC4Occupant` `0x00AA9E18` | `cIGZSerializable` `0x00AA9DFC` | property holder `+0x10` `0x00AA9D98`; 5-slot table `+0x24` `0x00AA9D80`; model instance `+0x3C` |
| `cSC4PowerLineOccupant` | `cISC4PowerLineOccupant` `0x00AA9C80` | `cISC4Occupant` `0x00AA9BF8` | `cIGZSerializable` `0x00AA9BDC` | property holder `0x00AA9B78` | |
| `cSC4PylonOccupant` | `cISC4PylonOccupant` `0x00AA5150` | `cIGZMessageTarget2` `0x00AA5140` | occupant view `0x00AA50B8` | `cIGZSerializable` `0x00AA509C` | property holder `+0x10` `0x00AA5038`; model instance `+0x3C` |

So the inheritance chain is: `cSC4NetworkOccupant` → `cSC4Occupant`; the prebuilt, bridge, tunnel and underground
classes derive from `cSC4NetworkOccupant`; pipe and subway derive from `cSC4UndergroundOccupant`; the connection
occupants derive **directly** from `cSC4Occupant` (their constructor calls `cSC4Occupant::cSC4Occupant`
(`0x005E9220`) only — there is no network-occupant base). The pole, line and pylon also derive directly from
`cSC4Occupant`.

`QueryInterface` per class (each is `__thiscall`, one IID and one `void**`, callee pops 8, returns `al`, and
`AddRef`s on success):

| Class (address) | IID → result |
|---|---|
| `cSC4NetworkOccupant` (`0x006192C0`) | `0xA821EF94` (`cISC4NetworkOccupant`) or `0xA9764C8C` (class ID) → `this`; else the `cSC4Occupant` QI (`0x005E89C0` at `+4`): IID 1 (`cIGZUnknown`) or `0x26BB767E` (`cISC4Occupant`) → `this + 4`; `0xE4FDA3D4` (`cIGZSerializable`) → `+8`; anything else → the property holder (`+0xC`, through `0x005FD310`); then the model instance at `+0x68` (`0x00801940`) |
| `…WithPreBuiltModel` (`0x0061D060`) | `0x49C1A034` → `this`; else the base QI |
| `…BridgeOccupant` (`0x00604BC0`) | `0x49CC1BCD` → `this`; else the prebuilt QI |
| `…TunnelOccupant` (`0x00647170`) | `0x8A4BD52B` → `this`; else the prebuilt QI |
| `cSC4UndergroundOccupant` (`0x0065A080`) | `0x4A10AFC2`, `0xA821EF94` or `0xA9764C8C` → `this`; else the base QI, then the model instance |
| `cSC4PipeOccupant` (`0x006484D0`) | `0x49C05B9F` or `0xA9764C8C` → `this`; else the underground QI |
| `cSC4SubwayOccupant` (`0x00652D00`) | `0xCA16374F` → `this`; else the underground QI |
| `cSC4UndergroundConnectionOccupant` (`0x00654030`) | `0x8A139184` or `0xA9764C8C` → `this`; else the `cSC4Occupant` QI, then the model instance (`+0x38`) |
| `cSC4PipeConnectionOccupant` (`0x00648080`) | `0xEA0A5ACF` → `this`; else the connection QI |
| `cSC4SubwayConnectionOccupant` (`0x00652AA0`) | `0xCA15ED6B` → `this`; else the connection QI; its slot 44 (`QueryOccupantInterface`, `0x00652BD0`) answers QI `0xCA16374F` then `0x4A10AFC2` — "is it an underground occupant" |
| `cSC4PowerPoleOccupant` (`0x0064A030`) | `0xA8C86142` (`cISC4PowerPoleOccupant`), `0x09C05C6A` or `0xA9764C8C` → `this` (null-`this` quirk: out = 0, returns 1); `0x452294AA` (`cIGZMessageTarget2`) → `+4`; `0x26A68448` → dispatches through the delegate at `+0xBC` (§7.5); else the `cSC4Occupant` QI (`+8`), then the model instance |
| `cSC4PowerLineOccupant` (`0x00649350`) | `0xC98F3B24`, `0xC9C05C5D` or `0xA9764C8C` → `this`; else a tail jump to the `cSC4Occupant` QI with `ECX = this + 4` (**no** model-instance fallback) |
| `cSC4PylonOccupant` (`0x005F0820`) | `0xC9B6D7D6` (`cISC4PylonOccupant`) → `this`; `0x452294AA` → `+4` (null-`this` quirk: out = 0, returns 1); else the `cSC4Occupant` QI, then the model instance |

The IID `0xA9764C8C` accepted by every class is the family's shared "class ID as IID" (compare prop-occupant.md
§1.1). The pole additionally answers `0x452294AA` by returning `this + 4` even when `this` is null *[quirk]*:
the body tests `this` only after writing the out-pointer.

### 1.3 `GetGZCLSID` and the record class IDs

Every class answers `GetGZCLSID` (serializable slot 5) with its class ID:

| Class | Body | Returns |
|---|---|---|
| `cSC4NetworkOccupant` | `0x00619040` | `0xC9C05C6E` |
| `…WithPreBuiltModel` | `0x00604E50` | `0x49C1A034` |
| `…BridgeOccupant` | `0x00604BB0` | `0x49CC1BCD` |
| `…TunnelOccupant` | `0x006472D0` | `0x8A4BD52B` |
| `cSC4UndergroundOccupant` | `_purecall` `0x005D4A10` | — the base class is abstract; only pipe and subway are serializable |
| `cSC4PipeOccupant` | `0x00648F40` | `0x49C05B9F` |
| `cSC4SubwayOccupant` | `0x00653D90` | `0xCA16374F` |
| `cSC4UndergroundConnectionOccupant` | `0x00654E70` | `0x8A139184` |
| `cSC4PipeConnectionOccupant` | `0x00648040` | `0xEA0A5ACF` |
| `cSC4SubwayConnectionOccupant` | `0x00652A70` | `0xCA15ED6B` |
| `cSC4PowerPoleOccupant` | `0x0064DD50` | `0x09C05C6A` |
| `cSC4PowerLineOccupant` | `0x00649260` | `0xC9C05C5D` |
| `cSC4PylonOccupant` | `0x005F1A10` | `0x49B6D69B` |

`cSC4UndergroundOccupant`'s pure virtual `GetGZCLSID` (and `_purecall` slots 101/102/105 of its primary vtable)
make it an abstract class: it is never instantiated directly; the tools create pipe and subway occupants.

### 1.4 `IsOfType` and the type table

`cISC4NetworkOccupant` slot 22 (`0x00619520`): with the argument `t`, reads `EDX = [t*4 + 0x00AA7CE8]` and
tail-calls primary slot 8 (`HasAllNetworkFlags`, all bits). So **`IsOfType(t)` ≡ the occupant carries network flag
`1 << t`** — the same 13-entry table the network manager's own type queries use (network-manager.md §5: bit `1<<t`
= network type `t`). The table at `0x00AA7CE8` holds `1<<0` … `1<<12` in order. An implementer needs no table at
all: `IsOfType(t)` is `GetNetworkFlags() & (1 << t)`.

---

## 2. Object layout: `cSC4NetworkOccupant`

All offsets from the object base (= the `cISC4NetworkOccupant` pointer). The constructor is `0x0061BDD0`.

### 2.1 Sub-object map

| Offset | What |
|---|---|
| `+0x00` | `cISC4NetworkOccupant` (vtable `0x00AA8080`) |
| `+0x04` | `cISC4Occupant` base sub-object (its ctor runs with `this = object+4`; vtable `0x00AA7FF8`, 34-slot `cISC4Occupant` interface) |
| `+0x08` | `cIGZSerializable` (`0x00AA7FDC`: Write `0x0061A190`, Read `0x0061CC00`, `GetGZCLSID` `0x00619040`) |
| `+0x0C` | exemplar property holder (`0x00AA7F78`, 25 slots; foundations §10.3) |
| `+0x1C` | second property-holder view (`0x00AA4140` comes from the base ctor's `+0x1C` write, shared with `cSC4Occupant`) |
| `+0x20` | message-target-like view `0x00AA7F60` (the base ctor writes it; unused by this spec's callers) |
| `+0x68` | `cS3DModelInstanceBasic` sub-object (ctor `0x00801CF0`; the QI fallback of §1.2) |

Within the `+4` occupant sub-object, the base `cSC4Occupant` fields sit at base-relative offsets; measured from
the base `Write` `0x005E8CD0`: type u32 at base+0x20 (= object `+0x24`), the four rect bytes at base+0x24 (=
`+0x28`), counters u16 at base+0x28, state u16 at base+0x2A, flags byte at base+0x2B (masked `& 0xDD` when saved;
ctor default 4). The record layout itself is lot-records.md §2.3 and is not repeated here. The plain
`cSC4NetworkOccupant` constructor **overwrites the occupant type dword with `0xC772BF98`** at `0x0061BF85`, after
the base ctor wrote `0x74758925`.

### 2.2 Fields *[compat]*

| Offset | Type | Meaning |
|---|---|---|
| `+0x48` | u32 | lifecycle flags; `Init` sets `0x1A`; the prebuilt `Init` also clears `0x10` and sets `0x40` (§2.5) |
| `+0x38` | object | the `cS3DModelInstanceBasic` virtual base (ctor `0x00801CF0` runs with `this = object+0x38` at `0x0061BE07`; vbtable pointer at `+0x3C`) |
| `+0x64`…`+0x9B` | `S3DTransform` (0x38) | the transform (NO15): `+0x64` u8 change counter, `+0x65` u8 part mask (`4` rotation, `1` translation, `2` scale), `+0x68`…`+0x8B` 9 f32 rotation rows, `+0x8C`/`+0x90`/`+0x94` f32 translation, `+0x98` f32 scale; the ctor zeroes `+0x64`/`+0x65` (`0x0061BE52`) and seeds the matrix from `0x00B4525C` |
| `+0x9C`…`+0xFB` | 4 × 0x18 | the road **quad**: four vertices of 0x18 bytes each — f32 x `+0`, y `+4`, z `+8`, colour dword `+0xC` (bytes B,G,R,A at `+0xC`…`+0xF`), f32 u `+0x10`, v `+0x14` (NO16) |
| `+0xFC` | u32 | saved u32 written after the quad (record field 5) |
| `+0x100` | u32 | variation (§2.4) |
| `+0x104` | u8 | orientation/variation byte: low 7 bits `& 0x7F`, bit 7 is the **flip** (§2.4) |
| `+0x105` | u8 | rotation byte |
| `+0x108`…`+0x117` | sub-record | the cell-info block (record field 6, helper `this = object+0x108`): u8 entry count `+0x108`, u8 `+0x10C`, u32 `+0x110`, heap array pointer `+0x114` of `count` 8-byte entries {u8 `+0`, u32 `+4`}; slot 44 (`0x00604E00`) is `ADD ECX,0x108; JMP 0x006199D0` (free + reload) |
| `+0x118`/`+0x11C`/`+0x120` | vector | the triangle list begin/end/capacity, element stride **0x4C**: u32 `+0`, then three vertices of 0x18 in the quad-vertex format above (`+4`, `+0x1C`, `+0x34`) |
| `+0x124`…`+0x13B` | 6 floats | the bounding box: min x/y/z `+0x124/+0x128/+0x12C`, max x/y/z `+0x130/+0x134/+0x138` (`GetBoundingBox` `0x00619360` on the `+4` view reads view `+0x120`…`+0x134`) |
| `+0x13C` | u32 | the **network flags** (ctor `0x6000000`); the flag table is network-manager.md §5; saved by the record (field 10: `Read` masks `& 0xFFFF1FFF`, converting below v6 with `ConvertV5FlagsToCurrentFlags` `0x006191A0`) |
| `+0x140` | ref | an owned reference written through `SC4WriteGZSerializable` with IID `0x6999A8FB` (the segment-o-stream QI; foundations §3) and read back with `SC4ReadGZPersistRef` IID `0xA9667793` (record field 12); §2.6 reads the same word as the cached path info |
| `+0x144`…`+0x164` | 4 × 8 bytes | the connection byte matrix: `[exit][type]` (§2.6) |
| `+0x164` | u32 | path-id override (slot 16, `0x006197D0`); saved by the record (field 11) and the trigger of the v < 8 read tail |
| `+0x168`, `+0x16C` | int64 | demolition cost / saved int64 pair (slot 63); the ctor zeroes both |
| `+0x170`, `+0x174` | int64 | the int64 the record saves (field 13: low dword `+0x170`, high `+0x174`); §2.4 also reads this pair as the one-way data backing (property `0x4835C6AA`) |

**Offsets in the record.** `Write` and `Read` sit in the `+8` vtable (`0x00AA7FDC`) with no thunk, so they run with
`this = object+8`; every offset the record code uses is 8 less than the object offset in this table (NO16 c,
NO17 b). The rows above give object offsets.

### 2.3 The base save record *[compat]*

`Write` `0x0061A190` / `Read` `0x0061CC00`, both `__thiscall` (callee pops 4), both returning a clean 0/1 in EAX
(from the stream's good flag, `NEG AL; SBB EAX,EAX; INC EAX`). `Write` emits version **8**; `Read` accepts any
version ≥ 4 (below that it fails at once) and has conversions for 4/5/6/7. In order:

| # | Field | Type | Notes |
|---|---|---|---|
| 1 | version | u16 = **8** | `Read` rejects < 4 with nothing more read |
| 2 | the base occupant record | — | `cSC4Occupant::Write` / `Read` (lot-records.md §2.3) |
| 3 | transform | `S3DTransform` stream form | object `+0x64` (view `+0x5C`). Write `operator<<` `0x005DA8C0`: u8 mask = the transform's own mask byte `+0x65`, copied, never recomputed; then if `mask & 4` the 9 rotation f32 (`+0x68`…`+0x88`, row by row), if `mask & 1` the 3 translation f32 (`+0x8C`, `+0x90`, `+0x94`), if `mask & 2` the scale f32 (`+0x98`). Read: v ≥ 5 `operator>>` `0x005DA710`, v 4 `S3DTransformLegacyLoad` `0x005DA930`; both rules below the table (NO15) |
| 4 | the road quad | 4 × { 5 × f32, 4 × u8 } = 96 bytes | object `+0x9C`…`+0xFB`, one vertex per iteration (stride 0x18, 4 iterations, `0x0061A1C0`…`0x0061A23E`): f32 x `+0`, y `+4`, z `+8`, u `+0x10`, v `+0x14`, then the colour bytes `+0xE`, `+0xD`, `+0xC`, `+0xF` — i.e. **R, G, B, A** of the little-endian ARGB dword `+0xC`. Stream slot `+0x30` is `SetFloat32`/`GetFloat32`, so the five values are floats, not u32 (NO16 a) |
| 5 | four scalars | u32, u8, u32, u8 | u32 object `+0xFC`, u8 `+0x105` (rotation), u32 `+0x100` (variation), u8 `+0x104` (orientation + flip bit), in that order. These are separate fields after the quad, not part of it |
| 6 | the cell-info sub-record | u16 = 2, u8 count, u8, u32, count × { u8, u32 } | helper `0x00619C10` with `this = object+0x108`: count `+0x108`, u8 `+0x10C`, u32 `+0x110`, then each 8-byte array entry's u8 `+0` and u32 `+4`. Read `0x00619C90`: a version other than 2 returns 0 after consuming only the u16, and **the caller ignores that result** (the stream is then misaligned); otherwise it frees the old array, reads the three header fields and allocates `count × 8` bytes before reading the entries (count 0: no array) |
| 7 | triangle count | u32 | `(end − begin) / 0x4C` over the vector `+0x118`/`+0x11C` (magic division `0x6BCA1AF3`, `SAR 5`). Read resizes the vector to the count with zero-filled elements (`0x0061CAF0`) |
| 8 | per triangle | u32 + 3 × { 5 × f32, 4 × u8 } = 76 bytes | u32 `+0`, then the three vertices at element `+4`, `+0x1C`, `+0x34` in the field-4 vertex form (x, y, z, u, v, R, G, B, A). Read forces each vertex alpha byte to `0xFF` after reading it (`0x0061CE04`). The element is 0x4C bytes in memory and 76 in the stream (NO16 b) |
| 9 | the bounding box | 6 × f32 | interleaved min/max per axis: min x `+0x124`, max x `+0x130`, min y `+0x128`, max y `+0x134`, min z `+0x12C`, max z `+0x138` (loop of 3 at `0x0061A3B8`, slot `+0x30`) (NO16 c) |
| 10 | the network flags | u32 | object `+0x13C`. Read: below v6 converted by `ConvertV5FlagsToCurrentFlags` (rule below), then always `& 0xFFFF1FFF` |
| 11 | the path-id word | u32 | object `+0x164` |
| 12 | the owned reference | segment-stream ref | object `+0x140`; written through QI `0x6999A8FB` (helper `0x005D9940`); read with `SC4ReadGZPersistRef` (`0x005D9820`) IID `0xA9667793`. Read in **every** accepted version (no v8 gate) |
| 13 | the int64 | i64 (`SetSint64`, slot `+0x28`) | low dword object `+0x170`, high `+0x174`. Read in every accepted version |

`Write` returns early (to the error-flag return) when the occupant base record fails; otherwise it writes all 13
fields and returns 1 iff the stream's error flag is clear. Byte count of a version-8 record: 2 (version) + the base occupant
record + transform (1 + 36·[mask&4] + 12·[mask&1] + 4·[mask&2]) + 96 + 10 + (8 + 5·count) + 4 + 76·triangles
+ 24 + 4 + 4 + ref + 8.

**Transform read rules (NO15).** `operator>>` `0x005DA710` reads the u8 mask; if the live mask byte `+0x65` is
non-zero it first resets the transform to identity (rotation = identity, translation 0, scale 1.0), sets the mask
to 0 and increments the counter `+0x64`; then for each mask bit present it reads that part, ORs the bit into `+0x65`
and increments the counter. A part the stream omits keeps its value from before the call (identity after the
reset; untouched when the live mask was already 0). The scale is stored as read, but bit 2 is set only when the
scale differs from 1.0 (constant `0x00A81228`; a NaN counts as different) and cleared when it equals 1.0. So a
written mask of 7 with scale 1.0 reads back as mask 5. Worked example: a transform with mask `0x01` and translation (8, 2.5, 8) writes the 13
bytes `01 | 00 00 00 41 | 00 00 20 40 | 00 00 00 41`; mask `0x07`, identity rotation, translation 0 and scale
2.0 writes 1 + 36 + 12 + 4 = 53 bytes ending `00 00 00 40`.
`S3DTransformLegacyLoad` (v 4) is fixed-size, 56 bytes: 9 f32 rotation (rows, → `+0x68`…`+0x88`, mask `|= 4`),
3 f32 translation (→ `+0x8C`…`+0x94`, mask `|= 1`), 1 f32 scale (→ `+0x98`, bit 2 by the 1.0 test as above), then
a u32 whose **low byte replaces the counter `+0x64`** (the three increments are overwritten). It never resets the
mask first: bits already set stay set.

**`ConvertV5FlagsToCurrentFlags` `0x006191A0` (NO16 d)** maps each old bit to a new bit, starting from 0:

| old | new | old | new | old | new |
|---|---|---|---|---|---|
| `0x1` | `0x1` | `0x400` | `0x1000000` | `0x100000` | `0x80` |
| `0x2` | `0x2` | `0x800` | `0x2000000` | `0x200000` | `0x100` |
| `0x4` | `0x4` | `0x1000` | `0x4000000` | `0x400000` | `0x10000` |
| `0x8` | `0x8` | `0x2000` | `0x40` | `0x800000` | `0x20000` |
| `0x10` | `0x10` | `0x4000` | `0x20` | `0x1000000` | `0x100000` |
| `0x20` | `0x40000` | `0x8000` | `0x400000` | `0x4000000` | `0x80000000` |
| `0x40` | `0x80000` | `0x10000` | `0x10000000` | `0x20000000` | `0x200000` |
| `0x200` | `0x800000` | `0x40000` | `0x20000000` | | |
| | | `0x80000` | `0x40000000` | | |

Old bits `0x80`, `0x100`, `0x20000`, `0x2000000`, `0x8000000`, `0x10000000`, `0x40000000`, `0x80000000` produce
nothing directly. Last step: if old bit `0x80` is **clear**, the result loses `0x6000000` and gains `0x8000000`.
Worked example: old `0x0000_0283` → `0x1 | 0x2 | 0x800000` = `0x00800003` (bit `0x80` set, so no last step); then
the read mask gives `0x00800003`. Old `0x0000_1001` → `0x1 | 0x4000000`, then bit `0x80` clear → `0x08000001`.

**The read tail (NO16 e).** After field 11, only when the path-id word `+0x164` is non-zero:
- v < 7: the record holds **two more f32** here (offsets `dx`, `dz`; current writers never emit them). Read computes
  a cell `x = floor(((v0.x + v2.x) · 0.5 + dx) · 0.0625)` and `z = floor(((v0.z + v2.z) · 0.5 + dz) · 0.0625)` from
  quad vertices 0 and 2 (object `+0x9C`/`+0xCC` for x, `+0xA4`/`+0xD4` for z), in x87 then rounded through
  f32 (`+ 12582912.0` trick and a fix-up that makes exact integers floor to themselves, `0x0061CEBD`…`0x0061CF5B`),
  and calls primary slot 72 (`0x0061BA90`) with `(x, z)`, which sets property `0x4835C6AB` to `(z << 16) + x`.
- v < 8 (so also after the v < 7 step): primary slot 7 (`0x0061A120`) with 1, which adds the empty property
  `0x4835C6AC` (with 0 it would remove it).
Then fields 12 and 13 are read in every version. Finally Read fetches a render-state block from the model-instance
base (its slot `+0x3C`); if vertex 0's alpha `+0xAB` is not `0xFF` or that block's bit 2 is set, it forces all four
quad vertex alphas to `0xFF`, clears bit 2 and stores `255 · (1/255)` = 1.0 as the block's alpha before handing it
back (slot `+0x40`). The return is 1 iff the stream's error flag is clear; version < 4 returns 0 having read only
the u16.

The subclass records add fields after the base block; §3.4, §5.4, §6.4, §7.4, §8.5 give each.

### 2.4 Slot notes used across the family *[compat]*

- **`GetOccupiedCell`** (slot 59, `0x0061B010`): `(x, z)` of the piece's cell. World → cell is
  `trunc(v * 0.0625 + 12582912.0) − 12582912.0` per axis (constants `0.0625` at `0x00AA7D78`, `12582912.0`
  (`0x4B400000`) at `0x00AA7D7C`) — the standard 16-metre cell quantiser with the x87 rounding trick.
- **Position** (slot 75, `0x006193D0`): x = `+0x9C`, z = `+0xA0`, y = `+0x98` guarded against `-FLT_MAX`
  (`0x00AA7D74`), plus the model offsets `+0xB4` and `+0xBC`. The base `cISC4Occupant` `GetPosition`
  (`0x005E8A20`) zeroes all three.
- **Orientation/flip/rotation** (slots 26/24/25, `0x00604DE0`/`0x00604DC0`/`0x00604DD0`): the byte `+0x104`'s low
  7 bits are the orientation/variation, `GetFlip` returns `byte >> 7`. `SetRotation` (slot 27, `0x0061B7F0`)
  writes `+0x105` and posts a queued message. `SetVariation` (slot 29, `0x0061B880`) writes `+0x100` and posts a
  queued message.
- **`SetQuadDraw`** (slot 30, `0x0061B490`): alpha `0xFF` into the quads. **`SetTextureCoordinates`**
  (`0x00619550`) fills the quads from the tables `0x00AA7D98`/`0x00AA7D88` by `(b & 3)`, swapping the pair when
  `b ≥ 0x80` by `b & 1` *[compat]*.
- **`AddTriangle`** (slot 31, `0x0061C800`): appends a 0x4C-byte vertex to the `+0x110` list, recomputes the bbox
  (slot 80, `0x0061A430`) and calls slot 46.
- **Demolition cost** (slot 63, `0x00604E10`): returns the int64 `+0x168`/`+0x16C` in `EDX:EAX`.

### 2.5 Lifecycle *[compat]*

**`Init`** (slot 3, `0x0061CB50`; the `cISC4Occupant` view's slot 3 is the same body via the vtable
`0x00AA7FF8[3]`): the class runs its `StaticInit` once (`0x0061C690`); if not yet initialised: `SetNetworkFlag(0x2000)`
through primary slot 10; `+0x48 := 0x1A`; if the occupant is in the city (primary slot 6 folded `0x006C2040`) it
calls the network manager's compiled slot 23 (`AddToRetrofitList`, manager `0x00B43C94` + slot 0x5C) — the
retrofit pass then revisits the piece (network-manager.md §7.10); then `cSC4Occupant::Init` `0x005E8C90` (which
posts the `CreateOccupant` message only for fresh occupants, lot-records.md §2.4). If already initialised (the
re-`Init` after a load): `ClearNetworkFlag(0x10000000)` then `ClearNetworkFlag(0x800000)`.

**`StaticInit`** `0x0061C690`: latched by the byte at `0x00B45250`; registers the message handler `0x0061C520`
through `0x00B43C94` slot 0x114 (it clears the texture cache `0x00B45278` on a message); subscribes
`0x1AC0E11A`/`0xFAC0E219`; packs the three render states into `0x00B45260`/`0x00B45268`/`0x00B45270`; clears the
hashtable `0x00B4528C`. `operator new` `0x00619E90` allocates from the pool `0x00B0B398`.

**`Shutdown`** (slot 4, `0x00619100`) = `cSC4Occupant::Shutdown` only; the base class holds no model to release.

### 2.6 Path connections *[compat]*

- **`AddPathConnection`** (slot 54, `0x00619790`): `OR 1 << type` into `[+0x144 + exit*4 + type]` and into
  `[+0x144 + type]` (path type 0 is always set too), then `SetNetworkFlag(0x2000)`.
- **`PathConnection`** (slot 53, `0x006196A0`): if flag `0x2000` is unset it calls `GetPathInfo` first; with
  `0xFF` wildcards it returns the count of matching bits, otherwise a bool.
- **`GetPathInfo`** (slot 49, `0x0061B510`): returns the cache at `+0x140` without `AddRef` (TG24); builds it on
  first call. **`ReleasePathInfo`** (slot 50, `0x00619680`) and the reset path (`pathinfo` slot 0xA0, zeroing
  `+0x144…+0x160`, pathinfo slot 0x6C) belong to the traffic rebuild (traffic-network.md §7.2).
- The SDK's slot order for 53/54 is **reversed** in the compiled game (traffic-network.md §7.2); this spec keeps
  the compiled order.

### 2.7 Flags and lighting *[compat]*

- `SetNetworkFlag` `0x00619EC0` / `ClearNetworkFlag` `0x00619F70` post the queued message `0x89D3E9F3`
  (usable change, data1 = 1/0) exactly as network-manager.md §5 says: the set only when the occupant is in the
  city (`GetFlags() & 1`, primary slot 0x84); the clear unconditionally tests. Setting `0x4000` also calls slot 58
  (`SetTranslucency(1, 0x80)`).
- `SetLit` (slot 61, `0x0061B910`): gated by `CanBeLit` (slot 62 = `0x00648EF0`, false for the base class);
  toggles `0x20000000` in `+0x13C` and posts the queued `0x09D8E0B6` with the result of primary slot 0x30.
- `UpdateLighting` (slot 79, `0x0061BB60`): per triangle, the normal comes from
  `SC4GeometryUtilities::GetNormalToTriangle` scaled `1/sqrt(...)` and is written into the per-vertex colour
  bytes `+0x10…+0x13`; ends with `SetNetworkFlag(0x8000)` (primary slot 10).
- `IsManagedGlobally` (slot 5) = `0x00648EF0`, **false** for the plain class. The prebuilt (`0x00AA7038`), bridge
  (`0x00AA7300`) and tunnel (`0x00AA9308`) primaries answer **true** — that is what puts a piece on the network
  manager's `managed` list (network-manager.md §6.1).
- `Draw` (`0x0061A850`): packed render states `0x00B45260/68/70`; the quads (vertex buffer 10,4) then the
  triangles (`+0x110`), wrap/filter 3,3; gated by the zoom table `0x00B0B374` and the `0x6000000` flag check.
- `GetBoundingBox` (slot 10, `0x00619360`): min/max from `+0x120…+0x134` (§2.2).

### 2.8 The prebuilt subclass *(cSC4NetworkOccupantWithPreBuiltModel)*

The constructor `0x0061E1A0` calls the base ctor, sets network flags `0x400000` (`IsImmovable`, zones.md §8.4.1)
at `+0x13C`, and adds:

| Offset | Meaning |
|---|---|
| `+0x170`/`+0x174`/`+0x178` | u32 triplet (written by the record; NO4, §12) |
| `+0x17C` | u32 |
| `+0x180` | byte |
| `+0x184`…`+0x19C` | 7 floats — the **shear transform** inputs; `RecomputeShearTransform` `0x0061D230` uses the constant `0.0625` (`0x00AA81E4`); the ctor seeds `+0x184…+0x1AC` from `0x00B45298` and then zeroes `+0x184…+0x19C` |
| `+0x194` | float gate: if ≠ 0, `+0x198…+0x1A4` hold 4 more floats; v < 4 records used the legacy `+0x1A8`/`+0x1AC` pair + 2 more |
| `+0x1B0` | vector (u32) |
| `+0x1B4` | byte |
| `+0x1B8`/`+0x1BC` | int pair, ctor sets both to **−1** |
| `+0x1C0`/`+0x1C1` | bytes |
| `+0x1C4`/`+0x1C8` | pointers (the model side; the vtable patch at `+0x3C`-relative `0x00AA6EC8`/`0x00AA6FB0` pairs) |

`Init` `0x0061DB00`: the base `Init`; then `+0x48 & ~0x10 | 0x40`; city `0x00B43DDC` slot 0x24 → primary slot
0xF4 (`SetModel`, the prebuilt model from the exemplar); `RecomputeShearTransform`.

`GetPathInfo` `0x0061D380` (overrides the base): asks its model instance first (`+0x20` list slot 0x10
`GetCount`); a null model → null (traffic-network.md §7.2); otherwise like the base, honouring the `+0x164`
override and using `FUN_00466590`; caches into `+0x140`; if the result lacks `HasNetworkFlag(0x100000)` it calls
pathinfo slot 0x68; then `ResetPathInfo`.

`Shutdown` `0x0061D0D0` releases the model. `Write` `0x0061D8E0` (version 4) / `Read` `0x0061D9B0` (versions
3…4) are the base record plus the prebuilt fields; the field list is §12 (NO4).

The **bridge** subclass adds: `QueryInterface` `0x00604BC0` (class ID `0x49CC1BCD`), `Init` `0x00604BF0` = the
prebuilt `Init`, `Shutdown` `0x00604C10` releases the `+0x1D0` model/structure list then the prebuilt
`Shutdown`, and an extra constructor part `0x00604C60` allocating a 0x42-entry zeroed array (the `0x1F0`-stride
region). Creation is network-bridges.md §11.2 (class `0x49CC1BCD` through `CreateNetworkOccupant` slot 54, corner
points via slot 3, position = mean x/z with the highest corner, flags, slot 30, slot 68 passage ceiling when the
property `0xABE1B5A3` is absent, insert after the triangles). Its primary vtable `0x00AA7300` has one extra slot
(81, `0x00605070`, a helper over `+0x1D0`) before its RTTI column.

The **tunnel** subclass ctor `0x006476F0` runs the prebuilt ctor then sets `+0x13C &= ~0x400 | 0x10C00000`
(clearing the prebuilt bit, setting usable `0x10000000` + `0x0C00000`), stores the **paired portal** at `+0x1D0`
(traffic-network.md §4.8 TG25: counted ref, non-virtual setter `0x00647530`, getter `0x00647570` folded, released
by `Shutdown` `0x006474F0`); `Write` `0x00647580` (v2) writes the has-pair bool and the ref (read with IID
`0xE4FDA3D4`); `Read` `0x00647610` (v2) uses `SC4ReadGZSerializable` IID `0x8A4BD52B`; `Init` `0x006471C0` = the
prebuilt `Init` + `ClearNetworkFlag(0x4000)`. Creation and pairing are traffic-network.md §4.8 and
network-geometry.md §6.

---

## 3. The underground pieces

### 3.1 `cSC4UndergroundOccupant` — layout and slots

The constructor `0x0065C0C0` (the GZCOM creator `0x0065C330` also serves as pipe/subway base ctor) lays out:

| Offset | What |
|---|---|
| `+0x00` | `cISC4NetworkOccupant` `0x00AAAD40` — the **extended underground interface** (106 slots; the base 81 plus 25 underground slots) |
| `+0x04` | `cISC4Occupant` `0x00AAACB8` |
| `+0x08` | `cIGZSerializable` `0x00AAAC9C` (Write `0x0065B140`, Read `0x0065E510`, `GetGZCLSID` pure) |
| `+0x0C` | property holder `0x00AAAC38` |
| `+0x68` | model instance |
| `+0x98`… | position inputs (§3.3) |
| `+0x1FC`…`+0x238` | 3 × 4 float matrix (zeroed) |
| `+0x23C`/`+0x240` | cell x/z (from city slot 0x17C `PositionToCell`) |
| `+0x248`…`+0x24A` | three saved bytes (§3.4 field 9) |
| `+0x24C`…`+0x258` | the **four terrain heights**, f32 at `+0x24C`, `+0x250`, `+0x254`, `+0x258` (NO17 d): `GetTerrainHeights` slot 94 `0x00659DD0` copies these four out; `SetTerrainHeights` slot 95 `0x0065B6B0` stores exactly these four, then calls primary slot 80 (`+0x140`) with `(&box min +0x124, &box max +0x130)` and `ComputeModelYPosition` `0x0065AB50`; `GetPosition` (on the `+4` view) reads the same four as its view `+0x248`…`+0x254` |
| `+0x25C` | saved f32 (§3.4 field 11) |
| `+0x260` | byte set to 1 by the pipe and subway `Read` (code `+0x258`) |
| `+0x261`/`+0x262` | bytes set to 1/0 by the underground `Read` (code `+0x259`/`+0x25A`) |
| `+0x264` | the class ID (saved, §3.4 field 12) |
| `+0x268` | saved u32 (§3.4 field 13), set by the one-parameter `Init` from primary slot 0x65; the pipe's own extra u32 is `+0x270` (§3.3) |
| `+0x26C` | the per-class **class info** pointer (below) |

The extended interface's underground slots (SDK indices continue past the shared 81):

| Slot | Address | Behaviour |
|---|---|---|
| 82 | `0x00648460` | pipe/subway shared stub (also at pipe 82) |
| 83 | `0x00657730` | wall-quad helper |
| 84 | `0x0065AE30` | `SetVertexHeights(4 floats)`: writes `+0xA0/+0xB8/+0xD0/+0xE8`, calls `ComputeModelYPosition` `0x0065AB50`, then rebuilds via slots 0x164/0x168 |
| 85 | `0x0065B700` | terrain helper (two values + flag with 86) |
| 86 | `0x0065ADD0` | `CreateGeometry(bool)`: slots 0x60/0x64 (rotation/variation), `ComputeModelYPosition`, slot 0x154, slots 85/87/89/90 |
| 87 | `0x0065D810` | terrain helper |
| 88 | `0x0065A3F0` | `GetZipperPieceType`-adjacent helper |
| 89 | `0x0065E260` | terrain helper |
| 90 | `0x0065E410` | terrain helper |
| 92 | `0x0065A6F0` | `IsImaginaryConnection`-adjacent |
| 93 | `0x0065AA70` | neighbour finder |
| 94 | `0x00659DD0` | `GetTerrainHeights(float[4])` |
| 95 | `0x0065B6B0` | `SetTerrainHeights(const float[4])` |
| 96…100 | `0x0065A810`, `0x00659DB0`, `0x0065A120`, `0x0065A200`, `0x0065AF10` | wall/zipper helpers; **slot 99 `GetZipperPieceType`** (below); slot 100 sets `+0x260` from city slot 0x18C of `+0x23C`/`+0x240` |
| 101/102/105 | `_purecall` `0x005D4A10` | abstract in the base (pipe/subway override, §3.5) |
| 103 | `0x0065B020` | wall helper |
| 104 | `0x0065A880` | the **model choice**: flags `0x20000` (burst) → the burst model, `0x10000` (distressed) → the distressed model (plumbing.md cites `0x0065A880`) |

`Init` (two forms). The one-parameter form `0x0065C070` (called by pipe/subway `Init`): runs `StaticInit` once
(`0x0065AE80`: two `PackRenderState`s into `0x00B46878`/`0x00B46880`, needs the renderer `0x00B43DD0`), then
`cSC4NetworkOccupant::Init`, then primary slot 0x65 → `+0x268`. The five-parameter form `0x0065C6B0(args: float
block p2, classID, param4)`: stores the four terrain heights `+0x24C…+0x258` from `p2+4/+0x10/+0x1C/+0x28`; calls
primary slot 3 (the corner points, `0x00619050`); resets the vertex heights `+0xAC…+0xF8` (0/1.0 pattern);
`PositionToCell` (city slot 0x17C) → `+0x23C/+0x240`; zeroes the matrix `+0x1FC…+0x238`; reads the **class info**
through primary slot 0x194 (`GetClassInfo`) into `+0x26C` (per-class static info: counts at +0x64, point lists at
+0x0F/+0x11, uv arrays at +0x12/+0x17, work at +0x48/+0x5C); subscribes `0x1AC0E11A`/`0xFAC0E219` twice (via
`+0x1B0`, `+0x178`); builds the per-vertex uv ratios from point-index pair distances
(`sqrt(dx*dx + dz*dz)`, ratio `d[i]/(d[i]+d[i+1])`) into the class-info arrays.

`Shutdown` `0x0065CD90`: releases the five texture bindings `+0x1CC…+0x1E0`, clears the wall vectors (`+0x184`,
`+0x190`, `+0x19C`, `+0x1B8`, `+0x1C4`, `+0x1E8`, `+0x1F4`), releases `+0x244`; then the base `Shutdown`.

### 3.2 Underground behaviour *[compat]*

- **`GetSortValue`** `0x0065A0E0`: if `HasAnyNetworkFlag(0x6000000)` it returns the stored sort value (the field
  address is ambiguous in the decomp — NO7, §12), else the class ID `0x4A10AFC2`. The subway body `0x00652D30`
  defaults to `0xCA16374F`; the pipe shape is the same.
- **`GetZipperPieceType`** `0x0065A200` (subway only: requires the class field `+0x264 == 0xCA16374F` and
  `+0xFC == 0x4B00`): finds the underground connection occupant at its cell (`FindUndergroundConnectionOccupant`
  `0x006585E0`), else the neighbours (`FindUndergroundOccupant` `0x006584D0`, offset tables
  `0x00B0CFE8`/`0x00B0CFEC`); the result is 0/1/2 plus an out byte.
- **`IsImaginaryConnection(dir)`** `0x0065A640` (NO17 e): if the piece's own flags (primary slot 12, `+0x30`) have
  bit 31 **clear** it returns false at once. Otherwise it looks for an underground occupant of its own class ID
  (`+0x264`) at cell (`+0x23C` + dx[dir], `+0x240` + dz[dir]) with `FindUndergroundOccupant` `0x006584D0`; the
  offset table `0x00B0CFE8` holds (dx, dz) pairs: dir 0 (−1, 0), 1 (0, −1), 2 (+1, 0), 3 (0, +1). **Not found →
  false.** Found → true iff the neighbour's flags have bit 31 clear (the neighbour reference is released).
  So: imaginary = "my bit 31 is set and the same-class neighbour in that direction has it clear".
- **`GetPosition`** `0x0065A720` (on the `+4` view `0x00AAACB8` slot 8, so `this = object+4`; offsets below are
  object offsets): the out vector starts as (0, `FLT_MAX`, 0). x and z are the **average of the four quad
  vertices** (NO17 f): x = `((v0.x + v1.x) + v2.x) + v3.x` then `× 0.25` (constant `0x00A8821C`), z the same
  with the z fields; vertex k is at object `+0x9C + 0x18·k` (x) and `+0xA4 + 0x18·k` (z), each partial sum stored
  as f32. y = the minimum of the four terrain heights `+0x24C`, `+0x250`, `+0x254`, `+0x258`, compared in that
  order with a strict `<` against the running value (seeded `FLT_MAX`, `0x00AAABB0`), so a NaN height never wins
  and all-`FLT_MAX` heights give `FLT_MAX`. Example: vertex x 0/16/16/0, z 0/0/16/16, heights 3, 2.5, 4, 2.5 →
  (8, 2.5, 8).
- **Draw** `0x0065B290`: the underground view — walls and floor from the wall quads.
- **No occupant type override**: the base ctor leaves the `cSC4Occupant` type (`0x74758925`) *[inferred — the
  demolition dispatcher never names an underground type, and consumers reach the piece through
  `cISC4NetworkOccupant`]*. See NO3 (§12).

### 3.3 `cSC4PipeOccupant` *(0x49C05B9F)*

Layout: `+0` primary `0x00AA97B8` (110 slots), `+4` `0x00AA9730`, `+8` serializable `0x00AA9714` (Write
`0x00648800` v3 / Read `0x00648840` v3 / `GetGZCLSID` `0x00648F40`), `+0xC` property holder `0x00AA96B0`.

`Init` (the vtable slot 3 body `0x00648BF0`; a second `Init` variant `0x00648BC0` exists on the occupant view):
`StaticInit` once `0x00648A90` (wall-slope ratios from the tables `0x00B0BA68`/`0x00B0BC60`/`0x00B0BC88` into
`0x00B46708…0x00B4672C`, ratio `d[i]/(d[i]+d[j])` pattern); `cSC4UndergroundOccupant::Init` (`0x0065C6B0`) with
`(p, classID 0x49C05B9F, param4 0x00B0BED0)`; `+0x270 = 0`.

Record: u16 **3**, the underground record (§3.4, with its own u16 3), then u32 object `+0x270` (code `+0x268`); `Read` accepts exactly 3, reads the same and then sets the byte object `+0x260 = 1` (code `+0x258`). The
meaning of `+0x270` is not established (NO6, §12).

`Shutdown` is **shared with the subway**: both classes' slot 4 is `0x00652CF0` (5 bytes, tail to the underground
`Shutdown`) *[quirk: one folded body for two classes]*.

`SetWateredState` is a **non-virtual** member `0x00648540` (direct-calls.md; plumbing.md §9): stores the state and,
when it changes, tail-calls primary slot 100 (`0x0065AF10`, the cell/`+0x260` setter). The water simulator calls
it directly (plumbing.md §6.4).

### 3.4 The underground save record *[compat]*

`Write` `0x0065B140` / `Read` `0x0065E510`, in the `+8` vtable with `this = object+8` like the base record (§2.2
"Offsets in the record"); the offsets below are **object** offsets (code offset + 8). The record is its **own
u16 version 3 followed by the complete base network record** (§2.3, with its own u16 8), then the underground tail
(NO17 a):

| # | Field | Type | Object offset |
|---|---|---|---|
| 1 | version | u16 = **3** | `Read` accepts exactly 3; anything else returns 0 having read only the u16 |
| 2 | the base network record | §2.3 | `cSC4NetworkOccupant::Write`/`Read`; on failure the tail is skipped |
| 3 | two bytes | u8, u8 | `+0x50`, `+0x51` |
| 4 | one float | f32 | `+0x60` |
| 5 | one word | u32 | `+0x17C` |
| 6 | six wall vertex lists | 6 × list | vectors (begin/end/capacity) at `+0x180`, `+0x18C`, `+0x198`, `+0x1A4`, `+0x1B4`, `+0x1C0` |
| 7 | a 4 × 4 matrix | 16 × f32 | `+0x1FC`…`+0x23B`, row by row (`operator<<` `0x005DA6E0` / `operator>>` `0x005DA6B0`) |
| 8 | the cell | s32, s32 | x `+0x23C`, z `+0x240` (`SetSint32`, slot `+0x20`) |
| 9 | three bytes | u8 × 3 | `+0x248`, `+0x249`, `+0x24A` (`operator<<(u8)` `0x005D9FD0`) |
| 10 | the four terrain heights | 4 × f32 | `+0x24C`, `+0x250`, `+0x254`, `+0x258` |
| 11 | one float | f32 | `+0x25C` |
| 12 | the class ID | u32 | `+0x264` (the field `GetZipperPieceType` and `IsImaginaryConnection` read) |
| 13 | one word | u32 | `+0x268` |

A **wall vertex list** (helper `0x0065B070` write / `0x006574A0` read, NO17 c) is a u32 count = `(end − begin) /
0x18` (magic division `0x2AAAAAAB`, `SAR 2`) followed by `count` vertices in the quad-vertex form of §2.3 field 4:
f32 x, y, z, u, v (element `+0`, `+4`, `+8`, `+0x10`, `+0x14`), then the colour bytes `+0xE`, `+0xD`, `+0xC`,
`+0xF` (R, G, B, A); 24 bytes per vertex in the stream and in memory. The reader resizes the vector to `count`
first (`0x005E1B30`) and does **not** force the alpha byte.

After the tail `Read` sets the bytes `+0x261 = 1` and `+0x262 = 0` (code `+0x259`/`+0x25A`) and returns 1 iff the
stream's error flag is clear. The tail after the base record is 2 + 4 + 4 + 6 × (4 + 24·n) + 64 + 8 + 3 + 16 + 4 + 4
+ 4 = 137 + 24·(total wall vertices) bytes.

Worked example (the stream after the base record, little-endian): bytes `+0x50 = 1`, `+0x51 = 0`, `+0x60 = 2.0`,
`+0x17C = 7`, six empty wall lists, an identity matrix, cell (12, 34), bytes 0/0/0, heights 1.0/1.5/1.0/1.5,
`+0x25C = 0`, class `0x49C05B9F`, `+0x268 = 0` give
`01 00 | 00 00 00 40 | 07 00 00 00 | 00 00 00 00` ×6 `| 00 00 80 3F, 00 00 00 00 ×3, ... (16 floats) | 0C 00 00 00
22 00 00 00 | 00 00 00 | 00 00 80 3F 00 00 C0 3F 00 00 80 3F 00 00 C0 3F | 00 00 00 00 | 9F 5B C0 49 | 00 00 00 00`
= 137 bytes.

### 3.5 `cSC4SubwayOccupant` *(0xCA16374F)*

Layout: `+0` primary `0x00AAA3C8` (112 slots), `+4` `0x00AAA340`, `+8` `0x00AAA324` (Write `0x00653010` v2 / Read
`0x00653040` v2: u16 **2**, the underground record (§3.4), **no** extra field; `Read` accepts exactly 2, then sets the byte object `+0x260 = 1`, code `+0x258`), `+0xC` `0x00AAA2C0`.

`Init` `0x006537A0`: `StaticInit` `0x00653640` (ratios from `0x00B0C960`/`0x00B0CB58`/`0x00B0CB80` into
`0x00B467F8…0x00B4681C`); `cSC4UndergroundOccupant::Init` with `(classID 0xCA16374F, param4 0x00B0CDC8)`.
`GetPathInfo` has a subway override `0x006537E0` (522 bytes — the subway-specific path build). Everything else is
the underground base.

### 3.6 The underground connection occupant *(cSC4UndergroundConnectionOccupant, 0x8A139184)*

The constructor `0x00654F00` derives **directly from `cSC4Occupant`** (no network-occupant base): `+0` primary
`0x00AAA840` (25 slots, `cISC4UndergroundConnectionOccupant`), `+4` serializable `0x00AAA824` (Write `0x00654C50`
/ Read `0x00659D70` / `GetGZCLSID` `0x00654E70`), `+8` property holder `0x00AAA7C0` (25), `+0x1C` `0x00AAA7A8`,
`+0x34` `0x00AAA798`, `+0x38` `cS3DModelInstanceBasic`, `[+0x3C]`-relative `0x00AAA748`. No stored occupant type —
the subclasses set `+0x24` themselves.

Two `Init` bodies:

- `0x00654670` (the vtable slot 3 body): one-time latch `0x00B46838`; resource manager `0x00B43CA8`; registers the
  callback `FUN_00653DA0` through `0x00B43C94` slot 0x114; `cSC4Occupant::Init`; `SetupPackedRenderState`
  `0x00654260`; subscribes the four messages `0x8B87A974`, `0x8B87A975`, `0x8B8B2CEE`, `0xA6B79602` via the
  `+0x34` sub-object.
- `0x0065471D` (the tools' creation path; 1 328 bytes): the same one-time init; `cSC4Occupant::Init`; the params
  land in `+0x23C/+0x240` (cell) and `+0x244` (byte); releases `+0xC8/+0xCC`; `PositionToCell`; city slot 0x150 +
  slot 0x44/0x180 build; y positions from `0x00B0CF9C` (the cell size); `SetupPackedRenderState`; `+0x178 = 1`,
  `+0x17C = 1`; slot 0x98; if `FindUndergroundOccupant` (`0x006584D0`) at its own cell finds a piece → slot 0xAC,
  else the 8-neighbour loop with `FindUndergroundOccupant` / `FindUndergroundConnectionOccupant` (`0x006585E0`)
  and the network manager's connections (manager slot 0x14); slot 0xA8; then the same four subscriptions.

`Shutdown` `0x006553A0`: three texture bindings (`+0x9C`, `+0x64`, `+0xB8`); the vector clears (4 × stride 0xC at
`+0x70`, then `+0xA4`, `+0xB0`, `+0xC0`); releases `+0x158`, `+0x15C`, `+0xC8`, `+0xCC`; unsubscribes the four
messages; `cSC4Occupant::Shutdown`.

Record: `Write` `0x00654C50` v2 — the `cSC4Occupant` record, then u32 `+0x64`; seven wall writes (`+0x68`, `+0x74`,
`+0x80`, `+0x8C`, `+0x9C`, `+0xA8`, `+0xB8`); u32 `+0x14C`; u32 `+0x150`; 4 dwords `+0x15C…+0x168`; u32 `+0x16C`;
u32 `+0x174`; bool `+0x178`; `SC4WriteGZSerializable(+0xC4+8 or null)`; `SC4WriteGZSerializable(+0xC8+4 or null)`;
u32 `+0x23C`; 3 dwords `+0x22C…+0x234`. `Read` `0x00659D70` is symmetric.

Other bodies worth naming: `GetHiddenTriangles` `0x006540A0` (writes 0, 1, 1 to the out array);
`GetTransform`/`SetTransform` `0x00648360`/`0x006540F0`; `GetWorldBounds` `0x00654160`;
`PostOccupantMessage` `0x00654180`; `ModelStateChanged` `0x006542F0`; `HasViewDependentModel` `0x00654620`;
`GetModelInstances` `0x00655520`; `AddWallQuad` `0x006555E0`; `CreatePieceModel` `0x00655850`;
`CreateWalls` `0x00655F10`; `CreateFloor` `0x00656010`; `CreateGeometry` `0x006562B0`; `DoMessage` `0x00656C80`;
`Draw` `0x00656E90`.

The subclasses differ in exactly three slots plus the type: the **pipe connection** ctor `0x006483E0` = the
underground-connection ctor + its own vtables (`0x00AA9578` primary etc.); `Init` `0x00648050` sets the occupant
type `+0x24 := 0x2A183167`, then calls the helper `0x00654710` `(classID 0xEA0A5ACF, string 0x00B0B6D0)` — a
13-byte one-time registration whose string argument is unexplained (NO8, §12). The **subway connection** ctor
`0x00652C50`; `Init` `0x00652A80` sets `+0x24 := 0x8A13918A` and calls the same helper with
`(0xCA15ED6B, 0x00B0C388)`. The `QueryInterface`s are `0x00648080` (pipe) and `0x00652AA0` (subway); the subway's
slot 44 `QueryOccupantInterface` `0x00652BD0` QIs `0xCA16374F` then `0x4A10AFC2` (§1.2).

Both classes appear in saves by class ID (save-records.md §3.2: subway connection `0xCA15ED6B` in 1 save, pipe
connection `0xEA0A5ACF` in 4). The primary vtables are 45 slots (`0x00AA9578`, `0x00AAA188`) with the same slot
order; only QI (0), Init (3), GetGZCLSID (37), the folded 38 and slot 44 differ.

---

## 4. The power pole *(cSC4PowerPoleOccupant, 0x09C05C6A)*

### 4.1 Layout

Constructor `0x0064DB30`, object **0x124** bytes. Sub-objects: `+0` primary `cISC4PowerPoleOccupant` `0x00AA9EB0`
(19 slots), `+4` `cIGZMessageTarget2` `0x00AA9EA0` (4), `+8` `cISC4Occupant` `0x00AA9E18` (34; its QI thunk
`0x0064DF00` subtracts 8), `+0xC` `cIGZSerializable` `0x00AA9DFC` (Write `0x0064B010`, Read `0x0064EA00`,
`GetGZCLSID` `0x0064DD50`), `+0x10` property holder `0x00AA9D98` (25), `+0x24` 5-slot table `0x00AA9D80`
(`SetDefaultExemplar` `0x0064D910` is slot 3), `+0x3C` model instance, `[+0x40]`-relative `0x00AA9D30` (19-slot
3D-instance view). Occupant type `0x2890D4DE` written at `+0x2C`.

| Offset | Meaning |
|---|---|
| `+0x6C…+0x98` | model matrix (defaults from `0x00B46778`, identity 0/1); `+0x99` float 1.0 |
| `+0x98/+0x9C/+0xA0` | position x/y/z (`GetPosition` `0x00649DA0`) |
| `+0xAC/+0xB0` | connection vector (records of stride **0x2C**: +0 pole ref, +4 wire-point list, +8 byte flag, +9 byte, +0xC u32, +0x10 u32, +0x14…+0x24 two vertex lists) |
| `+0xB4` | second vector |
| `+0xB8` | wire-exemplar index (set by `Init`'s `param_3`) |
| `+0xBC` | delegate object (QI `0x26A68448` jump-dispatches to its vtable slot 0 — identity unknown, NO2/§12) |
| `+0xC0/+0xC4` | model ids (from `0x00B43DD8+0x10/+0xC`) |
| `+0xC8` | bbox-dirty byte |
| `+0xCC/+0xD4` | x cells; `+0xD0/+0xD8` z cells |
| `+0xDC/+0xE8/+0xF4` | three `ReadFromStream` structs (reader `0x006574A0`) |
| `+0xE4` | connection-points table pointer (`InitConnectionPoints` `0x00649B30` picks `0x00B0C0D8` or `0x00B0C198` by the exemplar sub-type nibble: 1, 2 → first; 3, 4 → second) |
| `+0xE8` | vertex array, stride 0x18 |
| `+0x100` | u32 |
| `+0x106` | byte |
| `+0x10C` | float (model Y) |
| `+0x112` | distressed byte (`IsDistressed` `0x00649230` reads it) |
| `+0x118` | stored by primary slot 14 (`0x00649250`) |

### 4.2 Interface slots *[compat]*

`cISC4PowerPoleOccupant` (vtable `0x00AA9EB0`), pop sizes from the ABI table:

| Slot | Address | Behaviour | Pop |
|---|---|---|---|
| 0 | `0x0064A030` | `QueryInterface` (§1.2) | 8 |
| 1/2 | `0x006491C0`/`0x006491D0` | `AddRef`/`Release` (thunks to the shared bodies) | 0 |
| 3 | `0x006C2040` | `AsOccupant`-folded (returns `this + 8`) | 0 |
| 4 | `0x00649D80` | connection count: `([+0xB0] − [+0xAC]) / 0x2C` | 0 |
| 5 | `0x0064BE50` | connection *i* (out: the record) | 8 |
| 6 | `0x0064DAE0` | `BreakAllConnectionsAtDestination` | 4 |
| 7 | `0x006499B0` | connection *i* field 0 (the pole ref) | 4 |
| 8 | `0x00649210` | `GetCell` = (`+0xC8`, `+0xCC`) | 8 |
| 9 | `0x0064A650` | refresh connections | 0 |
| 10 | `0x0064DD60` | `GetCostOfOccupantDemolition`: `EDX:EAX` from the cost statics `0x00B46770/0x00B46774` | 0 |
| 11 | `0x00649230` | `IsDistressed` (reads `+0x112`) | 0 |
| 12 | `0x0064AA40` | `SetDistressed` | 4 |
| 13 | `0x00649240` | unnamed flag (the "ignore this pole" test power.md §5.1 uses) | 0 |
| 14 | `0x00649250` | stores `+0x118` | 4 |
| 15 | `0x0064CE40` | `Init` (below) | 8 |
| 16 | `0x00649DD0` | (out-param pair) | 8 |
| 17 | `0x00649D00` | (out-param pair + more) | 20 |
| 18 | `0x0064C6C0` | `ChangeZoomLevel` | 4 |

`GetOwner` `0x00649190` is on the `+0x24` 5-slot table (slot 3). `Pick` `0x0064A870`: ray-vs-bbox over the wire
vertices (`0x005D6580`, `0x005D7660`, shortest kept; writes ctx `+0x28/+0x2C`).
`RemoveConnection` is non-virtual `0x0064DA00` (direct-calls.md). Other members: `SetPosition` `0x0064C540`;
`SetModelPosition` `0x0064AC70`; `DrawPowerlines` `0x0064B750`; `Draw` `0x0064C800`; `CreateFloor` `0x0064BFD0`;
`AddWallQuad` `0x0064C2C0`; `CreateWalls` `0x0064C470`; `SetHighlight` `0x006491E0`; `GetBounds` `0x00649E50`;
`GetWorldBounds` `0x00649E60`; `GetTransform` `0x00649E80`; `SetTransform` `0x00649F10`; `GetSortValue`
`0x00649200`.

### 4.3 `Init` and the static control variables *[compat]*

`Init` (primary slot 15, `0x0064CE40`, pop 8): `StaticInit` once (`0x0064CC20`); `cSC4Occupant::Init`; the model
ids `+0xC0/+0xC4` from `0x00B43DD8`; `+0xB8 = param_3`; the 5-slot table's slot 3 `SetDefaultExemplar(param_2)`;
`LoadModel` `0x0064AD90` (property `0x27812821` model id, resource `0x9AD0E7E3`); `InitConnectionPoints`; clears
the three vectors (`+0xE8`, `+0xF4`, `+0x100`); `+0x54 = 0`, `+0x55 = 0`, `+0x64 = 1.0`; if `+0x112`:
`SetDistressed(1)`.

`InitPowerLineControlVariables` `0x0064A170` (part of `StaticInit`): reads the exemplar
(type `0x6534284A`, group `0xE7E2C2DB`, instance `0xC911E35B`) properties into `0x00B46740…0x00B46754`:

| Property | Where | Default |
|---|---|---|
| `0xE9A18D3F` | `0xB46754` | 16.0 (`0x41800000`) |
| `0x49A1E8C4` | `0xB46750` | 10.0 |
| `0x89A18FC4` | `0xB4674C` | 10.0 |
| `0xE9A1E8D4` | `0xB46748` | 0.15 (`0x3E19999A`) |
| `0x49A195C0` | `0xB46744` | 8.0 (`0x41000000`) |
| `0x89A1E8FB` | `0xB46740` | 5 |

`GetBoundingBox` (`0x00649A00`) uses the cell-size floats `0x00B4675C`/`0x00B46760` (`Init` reads city slots
0x16C/0x170 into them).

### 4.4 Save record *[compat]*

`Write` `0x0064B010` writes version **5** (verified: `PUSH 0x5; CALL [EAX+0x1C]` at `0x0064B01D`; `Read`
`0x0064EA00` rejects < 4):

1. the `cSC4Occupant` record;
2. the transform (`operator<<` `0x005DA8C0`, or the legacy form below v5);
3. u32 `+0x94`; u32 `+0x98`; u32 `+0x9C` (position x, z);
4. the connection count, then per connection: `SC4WriteGZSerializable` of the pole ref, then per wire-point entry
   `SC4WriteGZSerializable` (pole refs read back with IID `0x09C05C6A` and `0xC9C05C5D`), u8 rec+8, u8 rec+9,
   u32 rec+0xC, u32 rec+0x10, and the two vertex lists (counts then 0xC-stride entries);
5. u32 `+0xAC`; u32 `+0xB4`; u32 `+0xB8`; u32 `+0xBC`; u32 `+0xC0`;
6. 4 × 0xC-byte vertex writes over `+0xD8…+0x108`;
7. the three struct writes (`+0xDC`, `+0xE8`, `+0xF4`);
8. u32 `+0x100`; u8 `+0x48`; u8 `+0x49`; u32 `+0x58`; u8 `+0x106`.

`Read` is symmetric; then `InitConnectionPoints`, and it releases `+0xB0` if set.

### 4.5 Creation and consumption

Poles are created by `cSC4PowerLineTool::CreatePowerPole` `0x00650140` (`FindPoleOccupant` `0x0064FFE0` reuses an
existing pole at the cell; `operator new` pool `0x006499D0`; `SetDefaultExemplar`; `+0xB8` = the exemplar index).
The consumer is the power simulator (power.md §5.1/§5.4: dispatch on type `0x2890D4DE`, QI `0xA8C86142`, wire bits
from `GetCell`, the reference list, `costBase` from the int64 property `0x09132DE8`).

---

## 5. The power line *(cSC4PowerLineOccupant, 0xC9C05C5D)*

### 5.1 Layout and slots

Constructor `0x0064A270`. Sub-objects: `+0` primary `cISC4PowerLineOccupant` `0x00AA9C80` (**8 slots**),
`+4` `cISC4Occupant` `0x00AA9BF8` (34), `+8` serializable `0x00AA9BDC` (Write `0x0064A310` / Read `0x0064A3A0` /
`GetGZCLSID` `0x00649260`), `+0xC` property holder `0x00AA9B78`, `+0x20` `0x00AA9B60`. Occupant type `0x0990B2EA`
at `+0x28`.

| Offset | Meaning |
|---|---|
| `+0x30` | u32 (the record's field 1) |
| `+0x34/+0x38/+0x3C` | position x/y/z floats (`GetPosition` `0x005B2B00`) |
| `+0x40/+0x44` | cell x/z (from `SetPosition`) |
| `+0x48/+0x4C/+0x50` | the three connected poles (refs) |
| `+0x54` | byte (the third pole's flag) |
| `+0x55` | preview byte (slots 6/7) |

`cISC4PowerLineOccupant` slots (vtable `0x00AA9C80`):

| Slot | Address | Behaviour | Pop |
|---|---|---|---|
| 0 | `0x00649350` | `QueryInterface` (§1.2) | 8 |
| 1/2 | shared | `AddRef`/`Release` | 0 |
| 3 | `0x00649290` | `GetConnectedPoles`: out1 = `+0x4C`, out2 = `+0x50`, out3 byte = `+0x54` (`AddRef`s) | 12 |
| 4 | `0x006492D0` | `SetConnectedPoles` (releases old, `AddRef`s new) | 12 |
| 5 | `0x00649330` | `GetCell` = (`+0x44`, `+0x48`) | 8 |
| 6 | `0x00649390` | `IsPreview` = byte `+0x55` | 0 |
| 7 | `0x006493A0` | `SetPreview` stores `+0x55` | 4 |

`SetPosition` `0x0064A460`: world x × 0.0625 (`0x00A8E174`) → int cell at `+0x40`, z → `+0x44` (through the
float→int helper `0x009EEF04`). `GetBoundingBox` `0x0064A4A0`: min = max = the position (a degenerate box).
`Shutdown` `0x00649270`: releases the two pole refs, then the base `Shutdown`.

### 5.2 Record and creation *[compat]*

`Write` `0x0064A310` v2: the `cSC4Occupant` record; u32 `+0x30`; u32 `+0x34`; u32 `+0x38`; u32 `+0x3C`; u32
`+0x40`; `SC4WriteGZSerializable(+0x4C)`; `SC4WriteGZSerializable(+0x44)`; `SC4WriteGZSerializable(+0x48)`.
`Read` `0x0064A3A0` v2: symmetric (refs read with IID `0xA8C86142`).

Creation: `cSC4PowerLineTool::PlacePoles` `0x00651560` creates lines directly (ctor `0x0064A270` at `0x00651CE4`),
position from `ComputePolePosition` `0x0064EFE0` (`x = (cell + 0.5) × 16`; `y` = terrain, guarded against
`-FLT_MAX` `0x00AA9F0C`; constant 0.5 = `0x00A84D2C`). Consumers: power.md §5.4 (the wire overlay walk) and §11.

---

## 6. The pylon *(cSC4PylonOccupant, 0x49B6D69B)*

### 6.1 Layout

Constructor `0x005F1730`. Sub-objects: `+0` primary `cISC4PylonOccupant` `0x00AA5150` (21 slots), `+4`
`cIGZMessageTarget2` `0x00AA5140`, `+8` occupant view `0x00AA50B8`, `+0xC` serializable `0x00AA509C` (Write
`0x005F0A10` / Read `0x005F0AF0` / `GetGZCLSID` `0x005F1A10`), `+0x10` property holder `0x00AA5038` (25),
`+0x24` `0x00AA5020`, `+0x3C` model instance, `[+0x40]`-relative `0x00AA501C` (19-slot 3D view). The ctor writes
the occupant type **`0xA9B6D7DE`** at `+0x2C` (`0x005F19F9`) — not the bare type (correcting the earlier
[inferred] guess).

| Offset | Meaning |
|---|---|
| `+0x5C` | transform (the record writes it) |
| `+0x60/+0x64/+0x68` | bytes |
| `+0x6C` | source occupant ref (`SetSourceOccupant` `0x005F0D30`; getter folded `0x00407AB0`) |
| `+0x70` | second transform |
| `+0x74` | call number (`SetCallNumber` `0x005F1A20` / `GetCallNumber` `0x005F1A40`) |
| `+0x7C/+0x80` | drag state (`0x005F1A50`/`0x005F1A60`) |
| `+0x84…+0xB0` | matrix (0/1) |
| `+0xB4…+0xC8` | second matrix (1,1,1,0,0,0) |
| `+0x134/+0x13C` | packed render states |
| `+0x140` | u32 (the record's `SetUint32` field) |
| `+0x198/+0x199` | bytes |
| `+0x19C` | u32 |
| `+0x1A0` | int64 |
| `+0x1B4` | ref |

### 6.2 Slots and lifecycle *[compat]*

`cISC4PylonOccupant` (vtable `0x00AA5150`; dispatch.md §4.1 cites these):

| Slot | Address | Behaviour |
|---|---|---|
| 0 | `0x005F0820` | `QueryInterface` (§1.2) |
| 3 | `0x006C2040` | `AsOccupant`-folded |
| 4 | `0x005F0930` | `Init` |
| 5 | `0x005F11A0` | `Shutdown` |
| 6 | `0x005F0CA0` | `SetUIState` |
| 8 | `0x005F0900` | `SetBitmapInfo` |
| 9 | `0x005F0D00` | `SetDispatchType` |
| 10 | `0x00407AC0` | `GetDispatchType` (folded) |
| 11 | `0x005F0CD0` | `SetDispatchState` |
| 13/14 | `0x005F1A20`/`0x005F1A40` | `SetCallNumber` / `GetCallNumber` |
| 15/16 | `0x005F1A50`/`0x005F1A60` | `SetDragState` / `GetDragState` |
| 17/18 | `0x005F0D30`/`0x00407AB0` | `SetSourceOccupant` / `GetSourceOccupant` |
| 19 | `0x005F1160` | `SetPylonOwner` |
| others | | `SetBitmapInfo` `0x005F0900`, `GetBounds` `0x005F0D60`, `GetWorldBounds` `0x005F0DA0`, `GetTransform` `0x005F0DE0`, `SetTransform` `0x005F0E70`, `GetPosition` `0x005F0F00`, `SetPosition` `0x005F2670`, `Pick` `0x005F0F40`, `DoAnimationTick` `0x005F0C50`, `SetVisibility` `0x005F12A0`, `DoMessage` `0x005F26F0`, `Draw` `0x005F2830`, `GetSortValue` `0x005F08B0` |

Textures: `BuildMySimTexture` `0x005F12D0`, `RebuildCurrentTexture` `0x005F1610`, `BuildCallNumberTexture`
`0x005F1D00`, `RebuildModel` `0x005F20A0`.

`Init` `0x005F0930` (the `+8` occupant vtable's slot 4): `IsInitialized` via the `+8` vtable +0x14;
`cSC4Occupant::Init`; `0x00B23B78 = 1`; subscribes the three messages `0xA6B79602`, `0x06CDB65B`, `0x26D31EC1`
via `+4`; `cRZSysServPtr<cISC4AnimationTickManager>(0x86F9FB4D/0x86F9FB4C)` slot 0x14 subscribes the tick
`0x80`; if the renderer `0x00B43DD0` exists: its slot 0x28 + `PackRenderState` into `+0x134` and `+0x13C`.

`Shutdown` `0x005F11A0`: `cSC4Occupant::Shutdown`; if byte `+0x37 & 1` (inserted): `PlaceInOccupantManager(0)`
`0x005F1010`; unsubscribe the three messages; release `+0x78` (the source).
`PlaceInOccupantManager` `0x005F1010` / `PlaceInRenderer` `0x005F10B0`: city slot 0x88 → handle; the renderer
calls go through `[+0x40]+4` slots 0x74/0x78; the state bits live in byte `+0x37`.

### 6.3 Record *[compat]*

`Write` `0x005F0A10` v5: the `cSC4Occupant` record; the transform (`+0x5C`); 3 bytes `+0x60/+0x64/+0x68`; byte
`+0x198`; `SetUint32 +0x140`; `SC4WriteGZSerializable(+0x6C source)`; the transform (`+0x70`); byte `+0x199`;
`SetUint32 +0x19C`; `SetInt64 +0x1A0`; `SC4WriteGZSerializable(+0x1B4)`. `Read` `0x005F0AF0` v ≥ 3: symmetric;
v < 4 reads the legacy transform; releases `+0x6C` first; v > 4 reads `+0x19C`/`+0x1A0`/`+0x1B4`.

---

## 7. What each class places on the grid, and who owns it

| Class | Placed by | What it covers | Owner/consumer |
|---|---|---|---|
| `cSC4NetworkOccupant` + prebuilt/bridge/tunnel | the network tools (`CreateNetworkOccupant` slot 54, network-geometry.md §10.1) | the solved piece's cell(s); the network lot manager keys lots by these cells (network-manager.md §8.5) | network manager `managed` list (only prebuilt/bridge/tunnel, §2.7); traffic simulator (traffic-network.md); demolition (demolition.md §4.2/§4.3, via type `0xC772BF98` and the cost slot 63) |
| `cSC4UndergroundOccupant` (+ pipe/subway) | the pipe/subway tools | the trench walls/floor drawn in the underground view; the pipe's cell feeds the water grid (plumbing.md §6.4) | plumbing.md (distressed/burst model choice §3.2 slot 104), power.md §5.2 (subway wire) |
| `cSC4UndergroundConnectionOccupant` (+ pipe/subway connection) | the pipe/subway tools at connection cells | the connection piece's walls; located by `FindUndergroundConnectionOccupant` | `GetZipperPieceType` (§3.2) and the tools' neighbour loops |
| `cSC4PowerPoleOccupant` | `cSC4PowerLineTool::CreatePowerPole` (§4.5) | its own cell (`GetCell`); the wire overlay is drawn *from* it | power.md §5.1 (the pole list, wire bits, `costBase`) |
| `cSC4PowerLineOccupant` | `cSC4PowerLineTool::PlacePoles` (§5.2) | its cell; up to three connected poles | power.md §5.4/§11 |
| `cSC4PylonOccupant` | the dispatch system | nothing persistent: it is a transient MySim/dispatch marker (dispatch.md §4.1) | dispatch manager (tick `0x80`), the animation tick manager |

### 7.1 The occupant ↔ network-lot relationship *[important]*

A **network lot** (`cSC4NetworkLot`, network-manager.md §2.3) holds a reference to the network occupant of its
cell. `CreateLotObjects` (network-manager.md §8.6) creates **props and flora** for a network lot — never
occupants. The network lot's cells are covered by the network-occupant pieces the tools place; the lot manager
finds the lot at a cell through the pieces' network flags (`GetZoneTypeForNetwork`, network-manager.md §8.2), not
through an occupant back-pointer. There is **no `cSC4NetworkLotOccupant` class** in the binary — the only
`cSC4NetworkLot` objects are the 24-byte lot records of network-manager.md §2.3. A replacement must therefore:

1. place one piece occupant per solved network cell (with the flags, §2.2 `+0x13C`);
2. let the network lot manager key its lots by cell and hold references to the piece occupants;
3. never create a separate "lot occupant" object.

### 7.2 References the records hold *[compat]*

| From class | Field | To | Read IID |
|---|---|---|---|
| network occupant (base) | `+0x138` | a path-info-like object | write via `0x6999A8FB`, read `0xA9667793` |
| tunnel | `+0x1D0` | the paired tunnel portal occupant | `0xE4FDA3D4` on write, `0x8A4BD52B` on read |
| pipe connection | `+0xC4/+0xC8` | two serializables (null allowed) | as written |
| power pole | per connection | the pole ref (+ wire entries) | `0x09C05C6A`, `0xC9C05C5D` |
| power line | `+0x44/+0x48/+0x4C` | up to three poles | `0xA8C86142` |
| pylon | `+0x6C`, `+0x1B4` | the source occupant, another ref | as written |

The occupant manager's own record (occupant-manager.md §13) references every one of these objects with IID
`0x26BB767E` (`cISC4Occupant`), once per covered cell.

---

## 8. `cSC4NetworkTypeInfo` — the per-network-type tables

### 8.1 The 13 records

Thirteen static records of stride `0x114` at `0x00B452C8`, constructed in a loop by the static initialiser
`0x00A73D10` (`MOV ESI,0xb452c8; MOV EDI,0xd;` ctor `0x00641F70` per record; the static destructor `___tcf_1`
`0x00A7C140` runs 13 reverse iterations). The layout is network-geometry.md §2.3 (widths, evaluator vector,
bridge types, supports map, placement parameters at `+0x50`, the four derived slope numbers at `+0x100…+0x110`) —
**cite, do not repeat**. What this spec adds is *how the records are filled*.

`ReloadNetworkTypeInfo` (`0x00623310`, called by `cSC4NetworkTool::ReloadRules` `0x0062541B` per tool) runs, in
order:

| Loader | Address | What it fills | Detail |
|---|---|---|---|
| `LoadPlacementParams` | `0x0063CBE0` | `+0x50` (the placement parameters) | one exemplar per type (type `0x6534284A`, group `0x084344E0`, the instances of network-geometry.md §2.4); the property list is: `0x879D12E7` → p+0x00, `0x279D1707` → p+0x04, `0x07B36C71` → p+0x08, `0xC7B36CA4` → p+0x0C, `0xC7B36CA5` → p+0x10, `0xC804DD86` → p+0x14, `0xC804DD87` → p+0x18, `0xC804DD88` → p+0x1C, `0xC804DD89` → p+0x20 (stored as read, not converted), `0xC804DD90` → p+0x24, `0xC804DD9B` → p+0x50 (default 1000), `0xC804DD92…99` → p+0x28…+0x4C, `0xE974EAA4` → p+0x54 (a vector), `0xE974EAA5` → p+0x60 (default: count of p+0x54 entries × 10.0 `0x41200000`), `0xE974EAA6` → p+0x6C (default 10.0), `0xE974EAA7` → p+0x34 (tail), `0x4A270DB0` (default 1.5), `0x4A270E94` (default 32.0), `0x4A271248` (default 0.0), `0x4A27124F` (default 1), `0x4A2D49B9` → p+0x8C (default 1), `0x8A2D49EA` (default 10), `0x8A2D49F0` (default 20), `0x6A2D49F4` (default 0.1). Angles are read in degrees and multiplied by the float constant `0.017453292` (`0x00AA8C60`) |
| `LoadEvaluator` | `0x0063D360` | `+0x24` (the evaluator vector) | factory `0x00622A90` → `cSC4TileEvaluator`, `AddRulesFromResource`, appended to `+0x28` (network-geometry.md §8 step 3) |
| `LoadTileConflictRules` | `0x0063D3E0` | the store `0x00B466D0` | callback `0x0063D1E0` |
| `LoadTilePatternVerifiers` | `0x0063FAF0` | the store `0x00B466DC` | `LoadVerifiersCallback` |
| `LoadSupports` | `0x0063F780` | `+0x3C` (the supports map) | `ReadSupportStructureInfos` `0x0063F700` (group `0x8974F80F`; the properties are network-geometry.md §10.2) |
| `LoadBridgeTypes` | `0x006437A0` | `+0x30` (the bridge-type vector) | `ReadBridgeTypeInfo` `0x00642220` + `ReadBridgeTypeSpecializations` `0x006436D0` (parser `0x00643580`, hash `0x00640E10`; network-bridges.md §2) |
| `LoadComplete` | `0x0063BF90` | `+4 = 1` | the loaded latch; `Clear` `0x006421D0`; dtor `0x00643510` |

The class-object store never constructs `cSC4NetworkTypeInfo` dynamically: the 13 records are process globals.
A replacement keeps one 13-entry array with the same stride and fills it at `ReloadRules` time.

---

## 9. `cSC4NetworkRoutines`

One body: `ComputeDemolitionRegionForPlacementCells` `0x0061F7C0` (1 004 bytes). Callers:
`cIntRule::Test` `0x00609DDE`, `UpdateOccupantHighlights` `0x00629C53`, `InsertIsolatedHighwayIntersection`
`0x0062D189` — the caller contract is network-rules.md §10 (cite, do not repeat).

Measured behaviour:

1. Builds an `SC4CellRegion<long>` from the input list of u32 x/z pairs; min/max +1 per cell; each cell's lot (city
   slot 0x18 `GetLot`) rect is merged via the lot's slot 0x38.
2. For each region cell, the zone manager `0x00B43D14` slot 0x30 gives the zone type; zone ≠ 0 → the demolition
   utility (`DAT_00b43d0c` slot 0x80) decides; a failure marks the region bitmap and the zone-type mask bit.
3. If any zone-type bit within `0xFC00` is set → return 0.
4. Otherwise it builds a `cSC4OccupantFilter`-shaped filter (vtables `0x00AA825C`/`0x00AA8244`) and calls the
   demolition manager (`DAT_00b43d00`) slot 0x18 (`DemolishRegion`, flags `0x2001`) over the region; returns its
   bool.

---

## 10. The model side (cSC4NetworkOccupantModelInstance)

`cSC4NetworkOccupantModelInstance` is a `cS3DModelInstance` subclass (ctor `0x00614A30`: primary `0x00AA7CBC`,
paired `0x00AA7C70`, `+0x20` `0x00AA7CE0`, processor at `+0x2C+0x70`) with four overrides:

| Body | Address | Behaviour |
|---|---|---|
| `Deform` | `0x00614A90` | bends the flat piece model to the four corner heights (network-geometry.md §8.1's corner points) |
| `FlipAndLight` | `0x00616800` | the flip (`+0x104` bit 7) and the lighting pass |
| `Clip` | `0x00618B00` | the underground/clipping view |
| `Prelight` | `0x00614350` | the pre-light pass |

`GetModelInstances` (primary slot 33, `0x0061AF90`) returns it. A replacement renders the piece by deforming the
exemplar model to the corners; the quads and triangle soup of §2.2 are the fallback path when there is no model.

---

## 11. Quirks (reproduced unless marked *[choice]*)

1. **One type for nine classes** (§1.1): plain, prebuilt, bridge, tunnel, underground, pipe and subway pieces all
   report occupant type `0xC772BF98`; everything downstream QIs instead of dispatching on the type *[inferred from
   the consumers]*.
2. **The pole's null-`this` QI** (§1.2): `QueryInterface` writes 0 to the out-pointer and returns 1 even when
   `this` is null, for the `0xA8C86142`/`0x09C05C6A`/`0xA9764C8C` and `0x452294AA` branches.
3. **`IsOfType` is a flag test** (§1.4): it answers "does the piece carry network flag `1<<t`", not "is the class
   of type t".
4. **The folded `Shutdown`** (§3.3): pipe and subway occupants share one 5-byte shutdown body `0x00652CF0`.
5. **The `SetWateredState` seam** (§3.3): the water simulator drives the pipe through a **non-virtual** member,
   not an interface slot (plumbing.md §6.4; direct-calls.md).
6. **`Read`'s version zoo** (§2.3): the base record is written as 8 and read with conversions for 4–7; the
   underground record is v3, the prebuilt record v4 (read accepts 3), the tunnel record v2, the connection record
   v2, the pole record v5 (read rejects < 4), the line record v2, the pylon record v5.
7. **The prebuilt's `+0x1B8`/`+0x1BC` are −1** out of the constructor (§2.8).
8. **The pylon's occupant type is its own** `0xA9B6D7DE` (§6.1), not the bare occupant type an earlier draft
   guessed from the missing write.
9. **`GetOccupiedCell`'s x87 rounding** (§2.4): `trunc(v * 0.0625 + 2^23) − 2^23` — reproduce exactly, including
   the float32 rounding (foundations §18).
10. **The connection matrix is 4 × 4 bytes at `+0x144`** (§2.6): exit × path-type, path type 0 always set along
    with the requested type, and the whole thing gated by network flag `0x2000`.
11. **The pole reads its own record with two IIDs** (§4.4): pole refs come back as `0x09C05C6A` **or**
    `0xC9C05C5D` (the line class ID) — a wire entry may name either.
12. **The demolition cost is an int64** (§2.4, slot 63): returned in `EDX:EAX`, saved as one `SetInt64`
    (§2.3 field 13).
13. **The prebuilt primary's appendix** (§2.8): the vtable at `0x00AA7038` continues past the shared 81 slots with
    a long tail of entries before its RTTI column; the shared interface proper is slots 0…80. *[quirk: the extra
    entries are unreachable through the SDK order; reproduce the first 81 and their overrides.]*

---

## 12. Open questions

- **NO1.** The meaning of network flag `0x40` tested by `IsIntersection` (primary slot 55, `0x0061BD80`) versus
  the `0x100000` intersection bit used elsewhere (network-manager.md §5). [inferred: `0x40` is a
  styling/presentation bit; the slot checks it before the highway-intersection piece-id lookup at `0x00B4528C`]
- **NO2.** The identity of the delegate interface `0x26A68448` answered by the power pole via `+0xBC`
  (jump-table dispatch; likely a UI/drag helper). Probe: dump the `[+0xBC]` vtables on a live pole.
- **NO3.** Does `cSC4UndergroundOccupant` ever carry a distinct occupant type? Its base ctor leaves
  `0x74758925`; the demolition dispatcher never names an underground type. [inferred: underground pieces are
  dispatched as `0xC772BF98` through their `cISC4NetworkOccupant` interface]
- **NO4.** The semantics of the prebuilt `Write` fields `+0x170`/`+0x174`/`+0x178`/`+0x17C`/`+0x180` (§2.8):
  model refs? shear inputs?
- **NO5.** The `0x4835C6A7…AC` property-ID family (edge types `0xA7`, passage ceiling `0xA8`, one-way `0xAA`,
  anchor `0xAB`, retrofit `0xAC`) stored in the occupant's own property holder — SDK names unknown.
- **NO6.** Pipe `Read`'s extra u32 → object `+0x270` and the byte `+0x260 = 1` (§3.3; code offsets `+0x268`/`+0x258`): meaning (funding state? watered?)
  unverified.
- **NO7.** `GetSortValue`'s stored-field address (§3.2): the negative displacement (`−0x178`/`−0x274`) in the
  decomp is ambiguous; which field holds the sort value when flag `0x6000000` is set.
- **NO8.** The strings `0x00B0B6D0`/`0x00B0C388` passed with the class IDs to helper `0x00654710` by the
  pipe/subway-connection `Init` (§3.6): GZCOM class names?
- **NO9.** ~~The power-pole `Write` version number~~ — **settled while writing this spec**: it writes version
  **5** (`PUSH 0x5` at `0x0064B01D`); `Read` rejects < 4. No question remains.
- **NO10.** ~~The pylon's occupant type~~ — **settled while writing this spec**: the ctor writes `0xA9B6D7DE`
  at `+0x2C` (`0x005F19F9`). No question remains.

---

## 13. Conformance tests

Each test names its section. A "city" is a fresh 64 × 64 city unless stated otherwise.

1. **Identity** (§1.1, §1.2). A newly created road piece answers QI `0xA821EF94` with the object base; QI
   `0xA9764C8C` likewise; QI `0x26BB767E` returns base `+4`; QI `0xE4FDA3D4` returns `+8`; an unknown IID reaches
   the property holder and comes back false. `GetGZCLSID` returns `0xC9C05C6E`.
2. **Types** (§1.1). `GetType` on a road piece returns `0xC772BF98`; on a pole `0x2890D4DE`; on a line
   `0x0990B2EA`; on a pipe connection `0x2A183167` **only after `Init`**; on a subway connection `0x8A13918A`
   after `Init`; on a pylon `0xA9B6D7DE`.
3. **`IsOfType`** (§1.4). For a road piece (flag bit 0 set), `IsOfType(0)` is true and `IsOfType(1)` is false;
   the result equals `GetNetworkFlags() & (1<<t) != 0` for every t in 0…12.
4. **Object sizes** (§1.1, §4.1). A pole allocates `0x124` bytes. The other classes' sizes are whatever their
   fields sum to; only the pole's is load-bearing (its pool).
5. **Occupied cell** (§2.4). `GetOccupiedCell` of a piece positioned at world (800.0, 240.0) returns (50, 15),
   including for negative coordinates down to −4096 (the x87 rounding of quirk 9).
6. **Orientation and flip** (§2.4). `SetRotation(2)` then slot 24 returns 2; setting the flip bit (`+0x104 |=
   0x80`) makes `GetFlip` return 1 while `GetOrientation` still returns the low 7 bits.
7. **Network flags** (§2.7). `SetNetworkFlag(0x10000000)` on a piece in the city posts `0x89D3E9F3` with data1 = 1
   exactly once; `ClearNetworkFlag(0x10000000)` posts data1 = 0; setting `0x4000` also calls slot 58.
8. **Managed-globally** (§2.7). A plain road piece answers `IsManagedGlobally()` false; a bridge and a tunnel
   piece answer true; the network manager's `managed` list gains exactly the true ones (network-manager.md §6.1).
9. **Path connections** (§2.6). `AddPathConnection(pt, e, x)` sets the matrix bit for `pt` **and** for type 0, sets
   flag `0x2000`, and `PathConnection` then matches; with `0xFF` wildcards it returns the count.
10. **Init/Shutdown round trip** (§2.5). `Init` sets `+0x48 = 0x1A`; a second `Init` clears `0x10000000` and
    `0x800000` from the flags; `Shutdown` does not crash on a piece with no model.
11. **Base record round trip** (§2.3). Save and reload a city with road/rail/highway pieces: every field of
    §2.3 is identical, the version is 8, and the flag mask `& 0xFFFF1FFF` is applied on read (a save with stray
    high bits reads them cleared).
12. **v4/v5/v6/v7 record reads** (§2.3). A hand-built v5 record with the legacy transform loads; a v6 record's
    flags go through `ConvertV5FlagsToCurrentFlags`; a v7 record skips the position recompute branch.
13. **Underground record** (§3.4). A pipe occupant saves and reloads with object `+0x268`, `+0x270`, the four heights
    `+0x24C`…`+0x258` and the six wall vertex lists identical; after read the bytes `+0x260 = 1`, `+0x261 = 1`,
    `+0x262 = 0`. The stream is u16 3 (pipe), u16 3 (underground), u16 8 (base) in that order; the underground
    tail with empty wall lists is 137 bytes.
14. **Tunnel pairing** (§2.8). Two portals built across a hill save and reload with the `+0x1D0` references
    resolved to each other (traffic-network.md §4.8).
15. **Connection occupants** (§3.6). A pipe connection saves as class `0xEA0A5ACF` with the seven wall writes and
    reloads; `QueryOccupantInterface` (subway, slot 44) QIs `0xCA16374F` then `0x4A10AFC2`.
16. **Pole record** (§4.4). A pole with two connections saves version 5; on read the pole refs resolve with IID
    `0x09C05C6A` **or** `0xC9C05C5D` (quirk 11); `InitConnectionPoints` runs again.
17. **Pole cost** (§4.2). `GetCostOfOccupantDemolition` (slot 10) returns the `EDX:EAX` pair from the cost
    statics `0x00B46770/0x00B46774`; changing the exemplar property `0xE9A18D3F` changes the demolition quote.
18. **Line slots** (§5.1). `GetConnectedPoles` returns the three refs with `AddRef`; `SetConnectedPoles` releases
    the old ones; `IsPreview`/`SetPreview` round-trip.
19. **Pylon record** (§6.3). A pylon saves v5 with both transforms and the two refs; on read v < 4 takes the
    legacy transform path.
20. **Pylon lifecycle** (§6.2). `Shutdown` with byte `+0x37 & 1` set calls `PlaceInOccupantManager(0)` and
    unsubscribes the three messages.
21. **Type info load** (§8). `ReloadRules` for the road tool fills all 13 records: the widths match
    network-geometry.md §2.3, the placement parameters §2.4, `LoadComplete` sets `+4 = 1`, and the road record's
    p+0x00 slope is 35 degrees → `+0x100 = r24(tan(35°) × 16)`.
22. **Power line has no placement exemplar** (§8, network-geometry.md §2.4): the power line record's parameters
    stay at the defaults and its plane tolerance p+0x20 is 0.
23. **Demolition region** (§9). `ComputeDemolitionRegionForPlacementCells` over two cells whose lots overlap
    returns 0 when either cell's zone type has a bit within `0xFC00` set, and otherwise demolishes exactly the
    merged region (flags `0x2001`).
24. **Model instance** (§10). `GetModelInstances` (slot 33) returns the `cSC4NetworkOccupantModelInstance`; after
    `Deform`, the rendered vertices' y values equal the four corner heights interpolated per quad position.
25. **No network lot occupant** (§7.1). After building a road, the occupant manager holds exactly one occupant
    per road cell (the piece); the network lot manager's lot at that cell references the same piece occupant; no
    separate lot occupant object exists.