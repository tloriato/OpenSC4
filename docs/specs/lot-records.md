# Lot and growth-occupant save records — data-format spec

Status: M6, written from the Windows 1.1.641 executable (`bin/SimCity4.exe`, image base `0x00400000`) and checked
against **every entry of every class record in all 587 city saves** under `Documents\SimCity 4` (417 distinct
files by content). Every layout below was read in the class's `Write` **and** `Read`, and a scratch validator parsed
each entry with these layouts, requiring exact consumption, byte-identical re-encoding, a correct entry CRC and
resolvable references (§17).

Tags: *[compat]* observable, must match; *[choice]* FreeCity4 decision; *[inferred]* not proven;
*[quirk]* original behaviour that looks unintended but is reproduced (or that a replacement must survive).

Shared material cited and not repeated: stream primitives (foundations §2.2), strings (§2.3), inline nested objects
(§2.6), references (§2.7), version convention (§2.10), the class-object store, its entries, CRC and load order
(§3.3–§3.5), random generators (§6), variants (§9), plain and exemplar property holders (§10.2, §10.3). The occupant
manager's own record, the simulator records that refer to lots (crime, budget, traffic, …) and the classes reached
only through them (pedestrians, automata) are specified in `save-records.md`. The trip-data element is also given in
`traffic-simulator.md` §6.3; the zone developer's record in `zones.md` §10.2; the lot developer's record in
`lot-developer.md` §13; flora behaviour in `flora.md`.

---

## 1. Scope and identity

These are the classes that **growth** (zone developer, lot developer, tract developer, construction) and **traffic**
create or change on a lot. A standalone FreeCity4 that replaces growth writes all of them. Each one is stored in the
class-object store (foundations §3.3): record key `(classID, 0x299B2D1B, 0)`, one entry per object, written only
because some other saved object holds a reference to it (§14).

| Class | Class ID | Role | Write / Read | `cIGZSerializable` vtable | Version written / accepted |
|---|---|---|---|---|---|
| `cSC4Lot` | `0xC9BD5D4A` | the lot | `0x006C0080` / `0x006C35F0` | `0x00AB1994` (sub-object +4) | 8 / ≤ 8 |
| `cSC4BusinessUnit` | `0x4A232DA8` | group of lots that share a business | `0x006C0370` / `0x006C09E0` | `0x00AB18C4` | 1 / 1 |
| `cSC4ZoneDeveloper` (lot map) | `0x498F9B01` | cell → lot map | `zones.md` §10.2 | — | 1 / 1 |
| `cSC4Occupant` (base part; bare instances) | `0xA9BC9AB6` | occupant base record; bare = dummy automaton occupant | `0x005E8CD0` / `0x005E8D60` | `0x00A9ECBC` | 4 / 4 |
| `cSC4BuildingOccupant` | `0xA9BD882D` | the grown (or plopped) building | `0x00458A90` / `0x00458B10` | `0x00A8721C` | 2 / 2 |
| `cSC4LotBaseTextureOccupant` | `0xC97F987C` | lot ground textures | `0x005DF1E0` / `0x005E10B0` | `0x00AA2D74` | 2 / 2 |
| `cSC4LotRetainingWallOccupant` | `0x49C05C9F` | retaining walls around a sunk / raised lot | `0x005E2AB0` / `0x005E2B40` | `0x00AA2FDC` | 2 / 2 |
| `cSC4FoundationOccupant` | `0x49C05C8F` | building foundations on slopes | `0x005D6250` / `0x005D62D0` | `0x00AA28FC` | 2 / 2 |
| `cSC4ConstructionOccupant` | `0xA97F909E` | construction in progress (crane, crew, dust) | `0x005CD250` / `0x005CE770` | `0x00AA219C` | 5 / ≥ 4 |
| `cSC4PropOccupant` | `0x2977AA47` | props placed by the lot developer | `0x005ECE90` / `0x005EF360` | `0x00AA484C` | 6 / any |
| `cSC4PropOccupantVisualEffect` | `0x2977AA49` | effect props (lot and construction effects) | `0x005F03B0` / `0x005EF750` | `0x00AA4DC4` | 3 / 3 |
| `cSC4SignpostOccupant` | `0xAB72FBB3` | region-view signposts with editable text (§10.3) | `0x005F5080` / `0x005F50F0` | `0x00AA5364` | 2 / any |
| `cSC4TextDecalOccupant` | `0x8B79C707` | terrain text labels (§10.4) | `0x005F7DB0` / `0x005F7E50` | `0x00AA556C` | 3 / any |
| `cSC4FloraOccupant` | `0xA9C05C85` | trees and plants, on lots and wild | `0x00472990` / `0x00472A20` | `0x00A88564` | 3 / 3 |
| `cSC4TripData` | — (inline element) | one trip of a lot's trip list | `0x0070E0B0` / `0x0070E160` | non-virtual | 2 / any |

Two further prop subclasses occur in the reference end-save corpus: terrain decal `0x2977AA48`
(Write `0x005ED710` / Read `0x005EF4E0`, §10.5) and burnt building `0xC9EFB016`
(`0x005EDB10` / `0x005EFBA0`, §10.6). Both write and accept outer version 3 only.
The automata-controller prop `0x2A4017A0` (`0x005EDDB0` / `0x005EFBF0`) remains outside these
specified layouts (§16 Q6).

Interfaces requested when these objects are read through references (foundations §2.7): `cISC4Lot` `0xE818FCED`,
`cISC4BusinessUnit` `0x6A233069`, `cISC4BuildingOccupant` `0x87DDA3A9`, `cISC4PropOccupant` `0xE9793A65`,
`cISC4Automaton` `0xA9B40F05`, and for effect props the class ID `0x2977AA49` itself (used as an IID). Objects are
created by the store through `GZCOM()->GetClassObject(classID, cIGZSerializable)`, so every class above must be
registered with the class registry (§15).

Agents, ticks and tuning: none of these records is an agent. No message is posted by any `Write` or `Read`. The
occupant base `Init` posts `CreateOccupant 0x99EF1140` only for occupants that were **not** loaded (§2.4).

---

## 2. Common pieces

### 2.1 Encodings used *[compat]*

All little-endian, no padding (foundations §2.2). Short names used in the tables:

| Name | Bytes | Encoding |
|---|---|---|
| `u8 / s8 / u16 / s16 / u32 / s32 / f32` | 1/1/2/2/4/4/4 | primitive |
| `bool` | 1 | written 0 / 1 (`operator<<` `0x005D9FD0`); read as byte ≠ 0 (`operator>>` `0x005D9FA0`) |
| `vec3` | 12 | 3 × f32 x, y, z (`0x005DA580` / `0x005DA550`) |
| `box` | 24 | 6 × f32: min x, y, z, max x, y, z (`0x005DA600`) |
| `day` | 4 | u32 day number of a `cRZDate` (`0x005DA1A0`; read `0x005DA170` sets the date from it). Observed values are Julian day numbers (2,451,545 = 1 Jan 2000) |
| `ref` | 4 or 8 | foundations §2.7: u32 objectID (0 = null, nothing follows), u32 classID |
| `bytes` | 4 + n | u32 n, then n bytes (`0x005DADA0`); read `0x005DB520`: if n ≥ 1,000,000 nothing more is read and the vector is left unchanged |
| `xform` | 1 + … | u8 mask; if mask & 4: 9 × f32 3×3 matrix (`0x005DA680`); if mask & 1: 3 × f32 translation; if mask & 2: 1 × f32 scale (`0x005DA8C0`; read `0x005DA710`) |
| `gzstr` | 4 + n | foundations §2.3 |
| `plain holder` | | foundations §10.2 (u16 2, u32 count, entries) |
| `exemplar holder` | | foundations §10.3 (u16 2, plain holder, u32 group, u32 type, u32 instance of the parent exemplar) |

### 2.2 Write and Read conventions *[compat]*

- `Write(stream)` and `Read(stream)` are `cIGZSerializable` slots 3 and 4, `__thiscall`, callee pops 4. `Write`
  returns true exactly when the stream has no error afterwards (`GetError() = 0`); it never returns false for any
  other reason (foundations §3.5 requires this).
- `Read` returns false (and stops) on its **own** unaccepted version; nested rejection propagates only where
  the containing body tests it (see §2.3 for the occupant-base exception). Otherwise its final acceptance uses
  `GetError() = 0`. The class
  store ignores the result and resynchronises on the entry size (foundations §3.5), so a short read never shifts the
  next entry; a read past the payload puts the stream into the sticky error state and fails the rest of the class
  *[compat]*.
- Subclass records start with their own u16 version, then the base record **inline** (its own version included),
  then their fields. A subclass `Read` that finds a bad subclass version returns false before reading the base.
- References read in `Read` are only stored; the referenced object may not have been read yet (foundations §2.7).
  Work that needs them is done in `Init` / `PostCityInit` (§3.5, §2.4).

### 2.3 Occupant base record (`cSC4Occupant`) *[compat]*

Every occupant record below contains this block (14 bytes + the holder):

| # | Field | Type | Meaning, observed values |
|---|---|---|---|
| 1 | version | u16 | 4. `Read`: anything else → false, nothing more read |
| 2 | removal counters | u16 | low byte = number of holders of removal flag 2, high byte = holders of removal flag 1 (`SetRemovalFlags` `0x005E8A60`, `CanRemove` `0x005E8AE0`). 0 in all records except props (0, 1, 256, 257) |
| 3 | highlight | u8 | `GetHighlight()` (`cISC4Occupant` slot 16): the highlight level. Always 0 in saves |
| 4 | occupant flags | u8 | the flag byte **with bits 1 and 5 cleared** (`& 0xDD`). Bit 0 = inserted in the occupant manager, bit 2 = visible (constructor default 4). Observed: 5 for every class; flora 13 (bit 3 also set); props also 0x85 (80×) and 0x15 (1×). Bits 3, 4, 7 unnamed |
| 5 | occupant type | u32 | `0x278128A0` building, `0x497F6D9D` base texture, `0x895D1169` retaining wall, `0x68FD0C69` foundation, `0x74758926` flora, `0xA823821E` prop (also effect props), `0x096E763C` construction, `0x74758925` bare occupant |
| 6 | manager cell rect | 4 × u8 | the occupant manager's spatial-index rectangle x1, z1, x2, z2 (first byte `0xFF` = not indexed). Set by the occupant manager on insertion. *[inferred]* `floor(coord / 64) + 64` of the occupant's world bounds (holds for 99.2 % of buildings, base textures, foundations and props; edge and model-extent cases differ). Never `0xFF` in saves |
| 7 | exemplar holder | | foundations §10.3; parent = the occupant's exemplar (type `0x6534284A`) or `(0, 0, 0)` (base textures, constructions, bare occupants, construction effect props). Entries in ascending ID, key = ID (all saves) |

`Read` (`0x005E8D60`) reads the same fields; the highlight byte becomes the occupant's status byte as
`(highlight << 4) | 2`, i.e. "not yet initialised, loaded from a save" (bit 1). Flags and type are stored as read.

**Nested-holder return (LG33):** this base reader ignores the exemplar holder's Boolean at `0x005E8DD8`
and immediately queries stream `GetError` at `0x005E8DE1`. It returns true when that error is zero, even
when the holder has just rejected its version or its nested plain-holder version. No cursor repair or rollback
occurs. This is the common **base** body; each derived reader decides whether to test the base return and
what follows. Landfill's tested base return and misaligned continuation are specified in landfill §6.4;
do not generalise its 21-byte tail or total return to other derived occupants. The exemplar holder itself
still tests the plain holder's return (foundations §10.3).

### 2.4 Occupant `Init` after a load (`0x005E8C90`) *[compat]*

If `IsInitialized()` (slot 5) is true: return true. Otherwise set status bit 0 (initialised). If status bit 1
(loaded) is **clear**, post `PostOccupantMessage(0x99EF1140 CreateOccupant, 0)` (slot 15); if it is set, call
`SetHighlight(0, false)` (slot 17) instead. Returns true. So loaded occupants do not announce themselves as
newly created.

---

## 3. The lot (`cSC4Lot`, `0xC9BD5D4A`)

### 3.1 Role, creation, interfaces

A lot is a rectangle of cells with a configuration (the lot plan), a state, a zone, an occupant wealth, a list of
populations and a trip list. The zone developer creates lots during parcellization and owns them through its cell
map (`zones.md` §10.2); the lot developer develops them. The constructor (`0x006C2340`) installs four vtables:
`cISC4Lot` primary `0x00AB19B0`, `cIGZSerializable` `0x00AB1994` (sub-object +4), the plain property holder
`0x00AB1930` (sub-object +8), `cISC4TrafficSource` `0x00AB1914` (sub-object +0x1C). It initializes the record
fields, constructs the property holder and allocates the empty containers; it does not zero the whole object.

**View access (LR9).** The offsets above are correct. On the primary `cISC4Lot` view, **slot 3**
(`0x006C2040`) is `AsPropertyHolder`, returning object +8, and **slot 4** (`0x006C2050`) is
`AsTrafficSource`, returning object +0x1C. Both are non-owning casts (`__thiscall`, no stack arguments,
EAX pointer, no AddRef); both return null for a null ECX. The names attached to these shared functions in
the export and the old ABI rows are not reliable interface names: the same address serves different classes.
In particular, the old `cISC4Lot` slot-4 name `AsISCPropertyHolder` is wrong; its +0x1C result is right.

Primary `QueryInterface` (`0x006BF190`, callee pops 8, result AL) accepts exactly IID 1 and lot IID
`0xE818FCED` (the primary pointer), and Serializable IID `0xE4FDA3D4` (object +4). Each success AddRefs
the primary object once. Any other IID returns false **without writing the destination**. Neither secondary
view is acquired through this QI. QI through the property-holder view first subtracts 8
(`0x006C2280`), through Serializable subtracts 4 (`0x006C2260`), and through the traffic-source view
subtracts 0x1C (`0x006C22B0`); all delegate to the same restricted primary QI. The property-holder
AddRef/Release thunks (`0x006C2290` / `0x006C2310`) subtract 8; the traffic-source thunks
(`0x006C22C0` / `0x006C2320`) subtract 0x1C. All views share the lot's count at primary +0xC
(AddRef `0x004664A0`, Release `0x006BF460`).

Native source/target evidence, checked with instruction bytes:

| Source | Target/result | Discrimination |
|---|---|---|
| Constructor `0x006C2345`, `0x006C2357`, `0x006C2372`, `0x006C2378` | construct property holder at +8; install `0x00AB1930` there and `0x00AB1914` at +0x1C | The views are not exchanged. The parameterized constructor repeats this at `0x006C1E55`–`0x006C1E88`. |
| Primary vtable entries `0x00AB19BC`, `0x00AB19C0` | slots 3/4 target `0x006C2040` / `0x006C2050` | Slot numbers are zero-based. |
| `0x006C2044` / `0x006C2054` | EAX = ECX +8 / +0x1C, respectively | Neither function calls AddRef. |
| Traffic source iterator `GetNext`, `0x0071083D`–`0x0071084E` | call lot slot 4 at `0x00710841`, store its result in source output, then AddRef the **lot** | A native caller consumes +0x1C as a traffic source, not a property holder. The iterator supplies the reference; the cast does not. |
| QI `0x006BF194`–`0x006BF1F2`; property-view QI `0x006C2280` | only the three IIDs above; property-view QI adjusts back to primary | The vtable's presence does not imply QI accepts that secondary view's IID. |

The lot does **not** save its occupants. `GetBuilding` and `GetLotOccupants` query the occupant manager over the
lot's bounding rect (with an "inside lot" filter) each time *[compat]*. The occupants' only link back is the
**lot tag** field of props and flora (§11, §12) and the lot reference of walls and constructions.

### 3.2 Record *[compat]*

| # | Field | Type | Meaning, observed values (12,986 lots) |
|---|---|---|---|
| 1 | version | u16 | 8 |
| 2 | lot configuration ID | u32 | `GetID()` (configuration slot 3) of the lot's configuration object; 0 when it has none (692 lots: the 691 empty zoned lots and one more). A lot read but never `Init`ed has no object and writes 0, not the ID it read (§3.5) |
| 3 | lot flags | u8 | bit 0 terrain locked flat at the lot's altitude; bits 0+1 together = foundation lot (lot developer `AddLotFoundation` sets `(f & ~4) | 3`); bit 2 retaining walls present *[inferred: set by `AddRetainingWallOccupant`; cleared with bits 0–1 by `StartConstruction`]*; bit 3 needs power; bit 4 has power; bit 5 historical; bit 6 vertices locked; bit 7 traffic mark, cleared by the traffic simulator's `FinishTripMap` (`traffic-simulator.md` §4.7), setter not identified. Observed: 0x50, 0x58, 0x5B, 0x10, 0xD8, 0x5F, 0x53, 0xDB, 0xDF, 0xD0, 0x57 |
| 4 | bounding rect | 4 × u8 | x1, z1, x2, z2 in cells, inclusive; x1 ≤ x2, z1 ≤ z2 always |
| 5 | location | 2 × u8 | x, z: always one of the four corners of the rect |
| 6 | altitude | f32 | lot altitude in metres (observed 256.79 … 391.05) |
| 7 | slope at altitude change | f32 | terrain slope stored when the altitude was last set (`SetAltitude`) |
| 8 | current slope | f32 | terrain slope at the last `UpdateSlope` (0 … 30.53) |
| 9 | size x, size z | 2 × u8 | lot size in cells (1…6, 1…8); always the rect's extent, possibly swapped by facing |
| 10 | facing | u8 | 0…3 |
| 11 | state | u8 | `cISC4Lot::HabitationState` (`lot-developer.md` conventions): observed 1 (691), 2 (31), 3 (12,158), 4 (106) |
| 12 | condition | u8 | observed 0…4 |
| 13 | zone type | u8 | observed 1, 2, 3, 4, 5, 7, 8, 9, 15 |
| 14 | occupant wealth | u8 | 0…3 |
| 15 | day of last state change | day | written by `SetState`; never 0 in saves |
| 16 | building type | u32 | the building's type; 0 for 691 lots. The multiset of non-zero values equals the multiset of the building occupants' building types (§5) |
| 17 | vacate reason | u8 | written only by `SetState(4, …)` (state 4 = vacant) and kept afterwards. Every vacant lot has 1…6; non-vacant lots carry stale values, 98 distinct ones including > 6 (e.g. 186 in 4,646 lots) *[quirk]* (§16 Q2) |
| 18 | business unit | ref | `cSC4BusinessUnit` (read IID `0x6A233069`); 7,746 lots |
| 19 | travel designate | ref | another lot (read IID `0xE818FCED`) whose trips stand in for this one; 6,110 lots |
| 20 | population map count | u8 | P (observed 0, 1) |
| 21 | P × population map | | u8 n (1…3), then n × {u32 developer type, u16 count}: the vector of maps returned by `GetPopulations` (slot 54). Keys observed: `0x1010/0x1020/0x1030` (R$…R$$$), `0x3110/0x3120/0x3130` (CS), `0x3320/0x3330` (CO), `0x4100/0x4200/0x4300` (I-R, I-D, I-M); counts 0…2,337 |
| 22 | travel-population map | | u8 n (0…3), then n × {u32 developer type, u16 count}; same keys |
| 23 | travel jobs | 3 × f32 | `GetTravelJobs`; observed [0] 0…450, [1] 0…73.8, [2] 0…8.2 |
| 24 | properties | plain holder | foundations §10.2 (§3.6) |
| 25 | trip count | u32 | T (0…31) |
| 26 | T × trip | | §3.3 |
| 27 | traffic strategy byte | u8 | 0…99 (`traffic-simulator.md` §4.1) |

Map entries are written in ascending key order (an ordered map); a map with more than 255 entries would be
truncated by the u8 count *[quirk; never observed]*. The trip list is written front to back.

`Write` returns false **without writing field 27** if a trip's `Write` fails (only on a stream error); the return
value is in **AL only** (§13).

### 3.2.1 Population mutation (LR10) *[compat]*

`SetPopulation(devType: u32, population: u16)` (slot 55, `0x006C1940`, pop 8) first calls
slot 52 `GetAvailableCapacity(devType, false)`. It compares the requested **unsigned word** with AX and
uses the smaller word (`0x006C1958–006C195F`). It then calls `GetBuildingType(true)` (slot 46),
and BDS slot 39 to obtain that type's profile. Let C be the profile's ordered vector of capacity maps,
and P the lot's vector of population maps. Search C in vector order for the first map containing devType
(`0x006C1980–006C19EC`). The key comparisons are unsigned u32 comparisons.

- No match, including C empty: return **AL = 0**, without writing P or recomputing wealth.
- First match at index i: assign the clamped u16 to key devType in **P[i]**, inserting the key there
  if absent; recompute occupant wealth (§3.2.2); return **AL = 1** (`0x006C19F8–006C1A28`).
- It does not append a map, search P for a suitable map, update all duplicate keys, or remove a zero entry.
  A zero assignment, unchanged assignment, and insertion all take the same successful path. It performs
  no state/day update, queued message, travel-population update or job-cache update.
- P must already contain the selected index. The native method indexes it without a size check; an empty
  P paired with a matching nonempty C is outside its valid container invariant, not a successful creation
  case. A profile pointer is dereferenced without a null check. Unknown profiles represented by a nonnull
  profile with an empty capacity vector take the no-match path. Native BDS slot 39 (`0x0069CA60`)
  uses its profile catalogue's indexing operation: an unknown type creates a default profile with
  purpose/wealth/resource count zero and an empty capacity vector (`0x0069B766–0069B7CA`).

The clamp's collaborator is `GetAvailableCapacity` (`0x006C0640`, slot 52). With its second argument
false, it returns AX=0 unless the state byte is 3. Otherwise it selects the same first C[i] containing
devType, or returns AX=0 if none. Start a float32 fraction F=1; for each **other** key q of C[i], in
ascending unsigned-key order, if P[i] contains q, replace F with
`r24(F − extended(P[i][q] / C[i][q]))`. The division uses exact u16-to-x87 conversion and no intervening
float32 store (`0x006C0733–006C0751`). Missing population keys are skipped, not inserted by this getter.
If F equals 1, return the requested key's capacity word. Otherwise, if the profile's mixed-occupancy byte
is zero, return AX=0; if nonzero, return the low word of
`bits32(r24(extended(capacity × F) + 12582912.0f)) − 0x4B400000`
(`0x006C079D–006C07F9`; constants `0x00A81228`, `0x00AB16B4`). There is no nonnegative clamp of F or
this result; zero denominators are not guarded. The equality test treats NaN as unequal. These rules
also explain why a requested u16 can be clamped to zero outside state 3 while the setter still succeeds.
The second argument true to slot 52 additionally permits state 4; SetPopulation always supplies false.

Only AL is defined as the boolean result; EAX's upper 24 bits are incidental. This is stronger than the old
`eax-or-void` annotation: the two explicit AL assignments prove a boolean result. ReoccupyBuilding passes
this AL through (lot-developer.md §6.4); tract UpdatePopulation ignores it (tract-developer.md §4.18).
Caller pass-through alone would not establish a boolean contract.

### 3.2.2 State mutation and queued snapshot (LR11) *[compat]*

`SetState(state: u32, wealth: u32, reason: u32)` (slot 34, `0x006C3100`, pop 12) compares the
zero-extended current state byte with the **whole** state argument. Equality does nothing. Otherwise it
stores the low state byte, then obtains the current city day and stores it as the state-change day
(`0x006C3106–006C3133`). The following actions precede the message copy, in this order:

1. When the old state was 2, the stored building type is nonzero, and its BDS profile purpose is nonzero,
   release construction resources via BDS slot 51 with the zero-extended profile u16 resource field.
   This happens even if the new state is not 3. If the new state is 3, obtain the developer type for the
   profile purpose and native wealth, call lot slot 50 `GetCapacity(devType, true)`, then slot 55 with
   that result (`0x006C3136–006C3198`). That nested setter can recompute wealth before step 4.
   Slot 50's true argument bypasses its state gate; it returns the capacity word from the first profile
   map containing the key, or zero when no such key/type exists (`0x006C05A0–006C0633`). Resource
   release is called even for resource count zero: native BDS `0x00695980` sends float32 −R to its
   construction-resource counter's slot 17, then invokes counter slot 30 (`0x00695980–006959A3`).
2. On entering state 2, obtain `GetBuildingType(true)`. If nonzero, destroy all existing population maps
   (retaining vector storage), then copy each profile capacity map into P in vector order and set every
   copied count to zero (`0x006C31C0–006C32F0`, clear helper `0x006C2E30`). Empty capacity maps remain
   empty maps. No building type means this rebuild is skipped. On entering state 4, store low byte of
   reason as the vacate reason (`0x006C31B1`); other states retain the previous reason.
3. If the new state byte is outside {2,3}, set all existing population counts in every map to zero,
   retaining the maps and their keys (`0x006C32F6–006C33A7`). Travel populations, travel jobs and trips
   are not reset by these loops.
4. Save the current occupant wealth as Wpre, then call `UpdatePrimaryOccupantWealth` (`0x006C33A9`).
   With no stored building type (`GetBuildingType(false) = 0`) it sets wealth to zero; with a nonzero
   type whose nonnull profile has purpose zero it leaves wealth unchanged (`0x006C143B–006C1466`,
   `0x006C15C3`). The profile/occupancy selection for a known profile is detailed below.
5. Let Wcomputed be the resulting wealth; initially message data4 = Wcomputed. If old/new state bytes
   differ in membership of {2,3}: leaving uses Wpre for data4 while keeping Wcomputed in the lot;
   entering with whole wealth argument nonzero stores its low byte in the lot and data4. Entering with
   wealth argument zero keeps Wcomputed. Within or outside the group, the argument is ignored
   (`0x006C33BA–006C3411`). Wpre is sampled here, not necessarily at method entry.
6. Call `UpdateNeedsPower` (`0x006C3417`), then construct the independent lot copy with `0x006C2450`
   (`0x006C3449`). This is the snapshot point: after all preceding mutation and synchronous calls,
   before message allocation and posting. Message type `0xE8BDA2E7` has data1 = old byte, data2 = new
   byte, data3 = stored building type, data4 as above. Post via MessagePost(message, 0), then release
   the caller's message reference, then its copy reference (`0x006C3473–006C34DB`).

There is no defined success return. The equal-state path happens to leave AL = current state; the changed
path leaves the final copy Release's EAX (normally 1 while the queued message owns the copy).
Neither is a SetState boolean. A caller forwarding AL does not
make either value a meaningful transition result.

**Known-profile wealth selection.** For nonzero stored building type and profile purpose, build the
occupancy-by-type map O through lot slot 62 (`GetOccupancyByType`, `0x006C10F0`), before choosing wealth.
It uses `GetBuildingType(true)` to obtain its own profile, clears its output, and walks every capacity
map C[i] in vector order and every key in ascending unsigned order. Each capacity key is indexed in
P[i], inserting a zero count if missing, **even before any floating-point occupancy calculation**
(`0x006C11E0–006C1209`). Thus the wealth update may mutate multiple population maps and keys after
SetPopulation's single selected assignment.

For each map compute the signed-int32 sum of its u16 capacities, then divide by the number of keys
with signed integer division, truncating toward zero; call this integer mean A[i]. Sum these integer
means into signed-int32 S (`0x006C1250–006C126A`). Native arithmetic wraps at the int32 additions.
C empty produces empty O. Any empty map **inside** nonempty C reaches integer division by zero;
there is no empty-map skip (`0x006C1258`). If S≤0 the output remains empty. Otherwise for each capacity
entry (q,c) in each map, in the same order, compute
`r24(extended(P[i][q] / c) × extended(A[i] / r24(S)))` as its contribution. The two quotients and
their product stay in x87 precision until the one float32 store (`0x006C12DE–006C1336`). The first
contribution initializes O[q]; subsequent contributions to that key are accumulated with a float32
store after each addition (`0x006C136E–006C1386`). Zero c is not guarded; nonfinite contributions are
kept. Neither travel populations nor travel designates participate.

Wealth zero is first replaced by the low byte of the profile's native wealth. Nonzero wealth is retained
as an initial candidate only if the developer type obtained from BDS slot 30 for (purpose, current wealth)
occurs in O; otherwise it too falls back to native wealth (`0x006C14A8–006C14F0`). Now start the best
occupancy at **0.0f** (zero-filled native datum `0x00B49114`, read at `0x006C14FC`). Visit wealth values
from the profile's signed-int32 native wealth down through 1, obtaining each developer type through
BDS slot 30. For each type present in O, replace the lot wealth and best occupancy only if its float32
occupancy is **strictly greater** than the best. NaN, zero, negative values and equal scores do not win
(`0x006C1557–006C1574`). Consequently a positive tie chooses the higher wealth, whereas all-zero O
retains a supported nonzero current wealth, or the native-wealth fallback. The temporary O tree and
its sentinel are freed before return (`0x006C1576–006C15B8`); there is no wealth-change notification.

**Snapshot contents and lifetime.** The copy constructor gives population maps and the travel-population map independent tree storage
(`0x006C250F`, `0x006C251B`); the helper at `0x004C9470`, despite its exported template name,
constructs a tree copy, not a vector append. Scalar saved fields, travel-job floats, state/day/wealth/reason
and strategy byte are copied. Configuration is retained once (`0x006C24BF–006C24CC`). The runtime tree
at native +0x40 (*[inferred: occupant set]*) and trip list start empty. **Business unit and travel designate
both start null** (`0x006C2505`, `0x006C250C`; unit identity confirmed by SetUnit `0x006C20B8`).
The raw scalar at +0x30 is the unresolved configuration ID used by Init (`0x006BF515`), not a unit
reference. The property holder is copied into its own entry-vector storage; its property objects are
shared, with one AddRef per nonnull entry (`0x006C247A` → `0x005FF120` → `0x005FEA70` →
`0x0077AC37–0077AC47`).
This describes snapshot semantics, not a layout contract for forwarding original methods on replacement objects.

The constructor resets the lot reference count to zero through the property-holder constructor
(`0x005FF134`). SetState AddRefs the new copy once at `0x006C3454`. Assigning it to the message's
IGZUnknown AddRefs it once more (`0x0091346B` → `0x005447C7`); it does not transfer the caller's
reference. MessagePost with flag false appends to the queue (`0x0092D0CF–0092D0E2`).
PushBack temporarily retains the message and then retains it in the queue
node, releasing the temporary afterwards (`0x0092DDF8–0092DE0E`, `0x004F20CF`, `0x007E98E5`).
After SetState releases its message and copy references, the queue therefore owns one message reference
and that message owns one copy reference. When the last message reference is released, its destructor
releases the copy (`0x009133C0–009133C9`); additional references acquired by receivers can extend this
lifetime. The copy's final Release invokes Shutdown then destruction (`0x006BF471–006BF480`). If its
copied initialized byte is set, Shutdown destroys the population maps, clears the travel map and releases
and nulls configuration; the destructor consequently does not release configuration twice
(`0x006C3072–006C30D7`, `0x006C26B4`). If uninitialized, the destructor alone releases configuration.
The destructor releases each shared property once (`0x005FF150` → `0x005FF0D0–005FF0D9`) and frees
the independent containers. No unit/travel-designate reference was acquired, and neither is released.

**Power cache before snapshot.** `UpdateNeedsPower` changes only flag bit 3, directly, without SetFlags
or a message (`0x006BFE25`, `0x006BFE2F`). It clears the bit unless state is 3 or 4 and
GetBuildingType(false) is nonzero. In that case it obtains the building through lot slot 48 and the BDS
profile. Purpose zero skips the developer/configuration test below. Otherwise it obtains the developer
for (purpose,native wealth) through BDS slot 33 and the configuration through lot slot 9; a null result
clears the bit. It calls developer slot 19 and configuration slot 11, comparing their **low unsigned
bytes**; configuration's byte must be strictly greater to continue (`0x006BFDA8–006BFDBC`).
If the building is null, set the bit. If present, obtain its property holder via building slot 3, result
slot 6, then look up properties through slot 9. A present `0x27812852` whose value's slot 72 returns
nonzero suppresses power need; regardless, look up `0x27812854`, requiring that property to exist and
its value's slot 72 to return nonzero. Set bit 3 only if this latter test passes and the suppression test
did not; otherwise clear it (`0x006BFDC4–006BFE23`). All these calls finish before the snapshot.

### 3.3 Trip element (`cSC4TripData`) *[compat]*

Inline, not a reference. Matches `traffic-simulator.md` §6.3; the observed values:

| # | Field | Type | Observed (8,109 trips) |
|---|---|---|---|
| 1 | version | u32 | 2 (all) |
| 2 | route | bytes | 0…47 bytes |
| 3 | return route | bytes | 0…33 bytes |
| 4 | count | u32 | 1…148 |
| 5 | type | u8 | 0…3 |
| 6 | strategy | u8 | 0…2 |
| 7 | flags | u8 | 0…11 |
| 8 | status | u8 | 0…5 |
| 9 | destination x, z | 2 × s16 | include `0xCCCC` (−13,108) and other uninitialised values |
| 10 | trip length | f32 | 0 … 12.74 |
| 11 | travel types used | u32 | only if version > 1 (else 0 on read). Observed 1 (28), 2 (217) and **`0xCCCCCCCC` (126)**: the uninitialised field of probe / bulk trips (`traffic-simulator.md` §4.2 quirk) is saved as is |

`Read` accepts any version. A writer must reproduce uninitialised fields byte-for-byte to round-trip a loaded
city *[compat]*; for trips it creates itself FreeCity4 writes 0 *[choice]* (`traffic-simulator.md` §4.2).

### 3.4 `Read` (`0x006C35F0`) *[compat]*

1. Set the lot's **initialised** flag (not saved) to true first, before the version check.
2. Read the version. If > 8: return false (only the version consumed).
3. Read the fields in the order of §3.2. Versions < 6 use a legacy layout: the rect is 4 × s32, the location
   2 × s32, the size 2 × s32; after the wealth byte a `bool` → `SetVerticesAreLocked` (slot 69); after the building
   type three `bool`s → flag bit 3 (needs power), `SetHasPower` (slot 67), `SetHistorical` (slot 44). No version
   < 8 occurs in the saves.
4. Business unit: the destination is cleared and the old unit released, then the reference is read. Travel
   designate: the old one is released and the reference read.
5. Population maps: the vector is **resized** to P and each map's entries are inserted (`operator[]`, so an entry of
   a key already present overwrites it). The travel-population map is inserted into likewise (not cleared).
6. The plain holder is read; its result is ignored.
7. Only if version > 6: the trips; each read trip is **appended** to the existing list (not cleared first). A trip
   `Read` failure returns false at once.
8. Only if version > 7: the strategy byte.
9. Return `GetError() = 0` in **AL**.

### 3.5 What happens after the load *[compat]*

The zone developer's `Init` calls `lot.Init()` for every lot on its map, once per lot at its origin cell
(`zones.md` §1.2), and later `PostCityInit`.

`Init` (`0x006BF490`, slot 5):

1. If the lot is **not** initialised (a new lot): travel jobs := 0, 0, 0; `c := GetCenter()` (slot 15);
   `a := terrain.GetAltitude(c.x, c.z)` (terrain `0x00B43CF4`, `cISTETerrain` slot 13, terrain.md §4.1, result in
   ST0); `SetAltitude(a)` (slot 17, `a` stored to a float argument); initialised := true. A loaded lot skips this
   (step 1 of §3.4), so its saved altitude and slopes are kept.

   **`GetCenter(out vec3)`** (slot 15, `0x006BF560`, pop 4) — the centre of the rect in metres, cell size 16:
   `x := (f32(16·x1) + f32(16·(x2 + 1))) × 0.5`, `y := altitude` (the stored float, copied), `z :=
   (f32(16·z1) + f32(16·(z2 + 1))) × 0.5`. The rect bytes are zero-extended, the products are integers converted
   exactly (`FILD`), the sum is one x87 add and the factor is the float 0.5 at `0x00A84D2C`; every value is exact.
   Example: rect (10, 20, 12, 21) gives x = (160 + 208) / 2 = 184.0, z = (320 + 352) / 2 = 336.0.

   **`SetAltitude(a)`** (slot 17, `0x006BF5C0`, pop 4): if `|a − altitude| > 0.001` (float `a` minus the stored
   float, `FABS`, compared with the **double** 0.001 at `0x00AA2270`; equal or NaN → no change): `altitude := a`,
   then `s := lotDeveloper.GetLotSlope(lot, false)` (BDS `0x00B43D68` slot 36 `GetLotDeveloper`, then lot developer
   slot 30, which is `GetAreaSlope(rect, false)`, lot-developer.md §11.3; it runs after the altitude is stored and
   does not depend on it) and **both** slope fields := `f32(s)`: field 7 (slope at altitude change, `+0x2C`) and
   field 8 (current slope, `+0x28`) get the same value. Otherwise nothing changes, including the slopes. Because the
   constructor zeroes the altitude, a new lot whose terrain altitude is within 0.001 of 0 keeps altitude 0 and both
   slopes 0 *[quirk]*. `UpdateSlope` (slot 20, `0x006BF610`) sets only field 8 to `GetLotSlope(lot, false)`.
2. If the strategy byte is `0xFF` (constructor value): `RandomUint32Uniform(100)` on the lot file's `cRZRandomSuper`
   `0x00B49120` (foundations §6.2), one draw. Saved lots have 0…99, so no draw on load.
3. Release the current configuration object. If the configuration ID is non-zero: look it up with the lot
   configuration manager (`0x00B43D18`, slot 8 `GetLotConfiguration(id)`) and keep the result (AddRef). Then set
   the stored ID to 0. If the configuration is missing from the data (for example a removed plug-in), the lot has no
   configuration and its next save writes 0 *[compat]*.
4. Return true.

**The ID read is never written back.** The lot keeps two members: the configuration **ID** read by `Read` (`+0x30`,
the `Read` at `0x006C3621` stores into it) and the configuration **object** (`+0x34`). `Write` (`0x006C0080`)
writes only `object ? object.GetID() : 0` (`0x006C0093`); it never reads `+0x30`. So a lot that was read but never
`Init`ed (the crime-only lot of §14) saves field 2 as **0**, whatever ID it read, and a loaded lot that `Init`
resolved writes the ID of the object found *[compat]*. A writer that round-trips such a lot byte for byte only
does so when the ID read was 0 (the observed crime-only lot is an empty zoned lot, so it read 0).

`PostCityInit` (`0x006BF200`, slot 8): `f := flags; flags := 0; SetFlags(f)`. `SetFlags` acts on a change of bit 0:
when bit 0 becomes set, each cell of the rect is locked in the terrain view at the lot's altitude; when it becomes
clear, each cell is unlocked. So a loaded lot with bit 0 re-locks its terrain *[compat]*.

**`SetFlags(f)`** (slot 13, `0x006BF220`, pop 4), exactly: `old := flags; flags := f` (stored first, always). If
`(old XOR f) & 1 = 0`, return. `v := SL::TerrainView()` (`0x00602160`, terrain.md); if null, return (the flags stay
stored, no cell is touched). Then walk the rect **x outer, z inner**, both inclusive and ascending: for `x` from x1
to x2, for `z` from z1 to z2 (bytes zero-extended; an empty range when x1 > x2 or z1 > z2, which no lot has):
- bit 0 now set: `v.LevelCell(x, z, altitude)` (terrain view slot 16, terrain.md, pop 12; its return is ignored)
  with the lot's stored altitude float as given, re-read for every cell;
- bit 0 now clear: `v.RemoveCellLevelling(x, z)` (terrain view slot 17, pop 8).

Example: rect (10, 20, 11, 21), bit 0 set → LevelCell (10,20), (10,21), (11,20), (11,21) in that order. No other
bit has an effect here; nothing is returned.

`Shutdown` (`0x006C3060`): if initialised: initialised := false; clear the population vector and the
travel-population map; release the configuration and the travel designate; `unit.Remove(lot)` (business unit
slot 4) and drop the unit. Returns true.

Not saved: the configuration object (saved as its ID), the initialised flag, everything derived from the occupant
manager (buildings, props), road-access caches and other runtime members, which their owners recompute.

### 3.6 Lot properties (plain holder) — observed

| Property | Lots | Meaning (owner spec) |
|---|---|---|
| `0x8A0B4100` | 12,977 | uint16[12] trip statistics (`traffic-simulator.md`, `pathfinder.md`) |
| `0xE999B11F` | 12,329 | no-power counter (`lot-developer.md` conventions) |
| `0x4A611773` | 12,280 | traffic random value (`traffic-simulator.md` §4.1) |
| `0x691C863C` | 12,280 | traffic strategy bucket (`traffic-simulator.md`) |
| `0x8999B20B` | 12,177 | no-water counter |
| `0xC999E206` | 12,177 | low-desirability counter |
| `0x2A4F9C74` | 7,737 | network link IDs (`traffic-network.md`) |
| `0x8999B10B` | 1,610 | travel counter |
| `0xC999C45E` | 1,397 | current zot |
| `0xEA54D283…0xEA54D286` | 6 each | locally funded line items (`budget.md`) |

All holders are in ascending ID order with key = ID. No object-valued property occurs.

---

## 4. Business unit (`cSC4BusinessUnit`, `0x4A232DA8`) *[compat]*

For live record observation, §14.2 (LR15) defines the distinct master-lot GetUnit inventory,
the borrowed getter/owned Serializable boundary, and its completeness limits. It is not an
all-allocation business-unit census; §17.9 T80–T81 exercise that distinction.

| # | Field | Type | Meaning |
|---|---|---|---|
| 1 | version | u16 | 1 (`Read`: anything else → false) |
| 2 | n | u32 | number of member lots (observed 1…269) |
| 3 | n × lot | ref | read IID `0xE818FCED`, appended to the member list |

`Write` returns in AL, `Read` in AL. The lot developer creates units and adds lots (`lot-developer.md` §5.7); each
member lot refers back to its unit (§3.2 field 18). In every save the two directions agree: 7,746 lot → unit
references and 7,746 unit → lot references.

Creation during play is direct from native `PickUnitToJoin`, not through GZCOM (§15.1, LR8). Constructor
`0x006C0800` takes storage in ECX, no stack arguments, returns that storage in EAX, and begins with count 0:
the property-holder constructor `0x005FF100` called on unit +8 clears its +4, i.e. unit +0xC. Adding members
retains both directions: a list node retains its lot (`0x006C09AB`), and `lot.SetUnit` retains the unit
(`0x006C09CB` → `0x006C20D3`). A replacement developer using a factory that supplies one creation reference
must drop that reference after adding the members; the native direct constructor supplies no such reference.

**Membership slots** (`cISC4BusinessUnit`, vtable `0x00AB18DC`; the member list is a `std::list` of AddRef'd lot
pointers whose head is at unit `+0x1C`):

| Slot | Address | Behaviour | Return |
|---|---|---|---|
| 3 `Add(lot)` | `0x006C0940`, pop 4 | Search the list for the pointer `lot` (temporary AddRef/Release around the search). Only if absent: append it at the **back** (one AddRef held by the node). Then **always** `lot.SetUnit(this)` (lot slot 26), also when the lot was already a member | none defined: AL is whatever `SetUnit` left in EAX |
| 4 `Remove(lot)` | `0x006C08B0`, pop 4 | AddRef `lot` (if non-null); erase **every** node equal to `lot` (`0x006C0900`, each erase releases the node's reference; nothing happens when the lot is absent); Release the temporary reference. Then **always**, without a null test and whether or not the lot was a member: `lot.SetUnit(0)` (slot 26) and `lot.SetTravelDesignate(0)` (slot 60) | none defined: AL is the low byte of `SetTravelDesignate`'s EAX, i.e. 0 when the lot had no travel designate, else the low byte of the old designate's `Release()` result |
| 5 `GetMembers()` | `0x008085F0` | the address of the member list (unit `+0x1C`) | EAX |

`SetUnit(u)` (`0x006C20B0`): if `u` differs from the current unit, Release the old one **first**, store `u`, AddRef
it. So `Remove` on the unit's last member drops the unit's last reference inside `Remove` (`Release`
`0x006BFE40` destroys it at a count ≤ 1); `Remove` touches only the lot after that point. `Remove(lot)` on a unit
the lot does not belong to still clears the lot's unit and travel designate (releasing its real unit without
removing it from that unit's list) *[quirk; no caller does this]*. `SetTravelDesignate(0)` (`0x006BF660`) releases
the old designate if there was one.

Neither return value is used: `Shutdown` (`0x006C30F3`) and the lot developer's `RemoveLotFromItsBusinessUnit`
(`0x006C7320`: `lot.GetUnit().Remove(lot)` with no null test, then its unit-changed flag `+0x5E := 1`) ignore
AL. `Shutdown` releases the travel designate before it calls `Remove`, then stores 0 into the unit pointer without
a release (`Remove` already released it through `SetUnit(0)`); it returns AL = 1.

---

## 5. Building occupant (`cSC4BuildingOccupant`, `0xA9BD882D`) *[compat]*

| # | Field | Type | Meaning, observed (12,295) |
|---|---|---|---|
| 1 | version | u16 | 2 |
| 2 | occupant base | §2.3 | type `0x278128A0`; exemplar holder parent = the building exemplar |
| 3 | building type | u32 | the building's type ID (equals the lot's building type, §3.2) |
| 4 | building flags | u8 | bit 0 lit (night lights, `lot-developer.md` §8). 0 in all saves |
| 5 | bounds | box | world bounding box |
| 6 | orientation | s8 | 0…3 |
| 7 | completion | f32 | `SC4Percentage`, observed 0…1 (below 1 only while under construction *[inferred]*) |

`Read`: version must be 2 and the base `Read` must succeed, else false. Holder properties observed on buildings
include `0xEA54D284/7`, `0x08D9763D`, `0x2A58D350`, `0xC910B4CF`, `0x69B425DD/E` and others set by the utility,
budget and protection simulators (their specs).

---

## 6. Lot base-texture occupant (`cSC4LotBaseTextureOccupant`, `0xC97F987C`) *[compat]*

For live record observation, §14.2 (LR15) defines full-grid registered-occupant discovery,
type `0x497F6D9D`, list/QI ownership and empty/failure results; §17.9 supplies conformance
fixtures. Lot bounds do not limit that inventory.

| # | Field | Type | Meaning, observed (13,079) |
|---|---|---|---|
| 1 | version | u16 | 2 |
| 2 | occupant base | §2.3 | type `0x497F6D9D`, holder parent `(0, 0, 0)` |
| 3 | bounds | box | |
| 4 | usage | s8 | 1 (94) or 2 (12,985) |
| 5 | n | u32 | number of texture records, 1…50 |
| 6 | n × texture record | 14 bytes | below |

Texture record (one per textured cell; `lot-developer.md` §6.6):

| Bytes | Field | Observed |
|---|---|---|
| u32 | texture instance | never 0 |
| u8, u8 | cell x, z | 0…126, 0…127 |
| u8 | orientation | 0…3 |
| u8 | configuration object byte 3 | 0, 1 |
| 5 × u8 | unnamed; constructor default `0xFF` (`0x005CA770`) | mostly `0xFF`; other values in up to 1,018 of the 28,815 records, e.g. `FF B2 33 AA 03` (T6) |
| u8 | animation frame | 0…7 |

`Read` resizes the record vector to n (no bound) and reads them. When textures are **set** (not on load), a frame
byte of `0xFF` becomes `RandomUint32Uniform(frames)` on the base-texture file's `cRZRandom` `0x00B23078` if the
texture has ≥ 2 frames (`GetFrameCountInLotBaseTexture` `0x005E08F0`), else 0: one draw per such record
(`SetTextureSpecification` `0x005E0B20`). Saved frames are kept on load.

---

## 7. Retaining-wall occupant (`cSC4LotRetainingWallOccupant`, `0x49C05C9F`) *[compat]*

For live record observation, §14.2 (LR15) defines full-grid registered-occupant discovery,
type `0x895D1169`, list/QI ownership and completeness; §17.9 includes the wall-mask mismatch
fixture. Lot bounds and master-lot membership do not limit that inventory.

| # | Field | Type | Meaning, observed (1,735) |
|---|---|---|---|
| 1 | version | u16 | 2 |
| 2 | occupant base | §2.3 | type `0x895D1169` |
| 3 | wall type | u32 | `0xC96D2137` (1,674), `0xC96D2136` (48), `0xC96D2135` (13) |
| 4 | edge mask | s32 | bit i = wall on edge i (0…15 observed) |
| 5 | edge hashes | 4 × s32 | one per edge, 0 = none *[inferred: model key of the edge's wall]* |
| 6 | position | vec3 | |
| 7 | size | vec3 | |
| 8 | lot | ref | read IID `0xE818FCED`; never null |

`Read` releases the old lot, then reads; while reading the hashes it **ORs bit i into the edge mask for every
non-zero hash i**. In 949 of the 1,735 saved walls the stored mask lacks bits of non-zero hashes, so the original
does not round-trip these records: after a load and save the mask grows *[quirk]* (test T7). The walls are created
by `AddRetainingWallOccupant(lot, …)`, a static member called by the lot developer (`AddLotFoundation`).

### 7.1 Building a wall: `AddRetainingWallOccupant(lot, add = true)` (`0x005E3070`, `__cdecl`, 8 bytes)

This is a direct cross-owner helper, not a factory or occupant-interface method. Its native lot-developer
call sites, direct wall constructor and replacement dependency are specified in §15.1 (LR8).

1. If `lot.GetFlags() & 1 = 0` (slot 12): return AL = 0, nothing created.
2. `R := lot.GetBoundingRect()` (slot 14, 4 × s32 x1, z1, x2, z2); `a := lot.GetAltitude()` (slot 16, stored as
   float); `top := 0`.
3. `found := GetLotWallEdgeFlags(lot, a, R, h[4], &top)` (`0x005E2CE0`, below). If false: return AL = 0, nothing
   created.
4. Create the wall (0x6C bytes, constructor `0x005E2F40`) and AddRef it. `t := GetRetainingWallType(lot)`
   (`0x005E2ED0`, lot-configuration.md: one uniform draw on `0x00B230C8` from the configuration's list, 0 when the
   lot has no configuration or the list is empty). If `t ≠ 0` and BDS (`0x00B43D68`) slot 29
   `LoadLotRetainingWallProperties(wall, t)` returns false: Release the wall, return AL = 0. (`t = 0` skips that
   call and builds the wall anyway.)
5. Edge mask (wall slot 8) := `(h[0] ≠ 0) | (h[1] ≠ 0) << 1 | (h[2] ≠ 0) << 2 | (h[3] ≠ 0) << 3`.
6. Position (wall interface at `+4`, slot 9) := `(f32(16·(x1 + x2 + 1)) × 0.5, a, f32(16·(z1 + z2 + 1)) × 0.5)`
   (the rect centre in metres, as `GetCenter`, §3.5). Size (wall slot 4) := `(16·(x2 − x1 + 1), top − a,
   16·(z2 − z1 + 1))`, the middle one a float subtraction of the two stored floats.
7. Wall slot 6 := `lot` (the lot reference); wall slot 10 := `h` (the four edge hashes); wall slot 12 := `t` (the wall
   type; 0 when `t = 0`); wall `+4` slot 3; insert into the occupant manager (`0x00B43D0C` slot 20, with the
   wall's slot 3 (0) result); `lot.SetFlags(lot.GetFlags() | 4)`; Release the creation reference.
8. Return AL = **0** even on success: the result byte (`[esp+0xF]` of the frame) is set to 0 at entry and never
   written again (`0x005E325B`) *[quirk; LD3 callers ignore it]*.

**`GetLotWallEdgeFlags(lot, a, R, h[4], &top)`** (`0x005E2CE0`): `top := f32(a + 0.1)` (float 0.1 at
`0x00A8C950`); `found := false`. For each edge `i` = 0, 1, 2, 3 (flag `1 << i`, sides as in zones.md slot 10):
1. `h[i] := 0`; `need := false`; `bits := 0`; `b := 0x80000000`.
2. If lot manager (`0x00B43D08`) slot 12 `GetNeighborCellsAtEdgeOfLot(R, 1 << i, C)` returns false: next edge.
3. For each cell of `C`, x outer, z inner, ascending inclusive: `L := lotManager.GetLot(x, z, false)` (slot 6). If
   `L` is null or `L.GetFlags() & 1 = 0`: `need := true`. If `L` is non-null and `L.GetState() ≠ 6` (slot 33):
   `bits |= b`. Then `b >>= 1`, and `b := 0x80000000` when it reaches 0 (so the 33rd cell reuses bit 31).
4. If `need`: `h[i] := bits`; `V :=` slot 11 `GetVerticesAtEdgeOfLot(R, 1 << i, V)` (its return is not tested); for
   each vertex of `V`, x outer, z inner, ascending inclusive: `found := true`; `y := terrain.GetAltitudeAtVertex(x,
   z)` (`0x00B43CF4` slot 14, terrain.md); if `y > top` (x87 compare; NaN no): `top := f32(y)`; then
   `h[i] += round(y) · x · z + 1` in 32-bit wrap-around arithmetic, where `round(y)` is
   `bits(f32(y + 12582912.0)) − 0x4B400000` (`0x005E2E6A`–`0x005E2E87`): round to nearest, ties to even, exact for
   |y| < 2^22.
5. Return `found` in AL (true when at least one edge needing a wall had a vertex).

So `h[i] = 0` means no wall on edge `i` (no neighbour row, or every neighbour cell is a terrain-locked lot), and
otherwise the hash mixes which neighbour cells hold non-state-6 lots with the rounded vertex altitudes. `top` is the
highest edge vertex (at least `a + 0.1`), so the wall is `top − a` high. A wall built with `t = 0` has the type the
constructor gives (not observed in saves: every saved wall has one of the three types).

**`AddRetainingWallOccupant(lot, add = false)`** (removal, `0x005E3270`–`0x005E32CB`): `(x, z) :=
lot.GetLocation()` (slot 21, the corner cell of §3.2 field 5); `w := null`; if occupant manager slot 33
`GetFirstOccupantByStandardCityCell(&w, x, z, 0x895D1169)` returns true and `w` is non-null: occupant manager slot
21 `RemoveOccupant(w, 1, 0)`, Release `w`, result := 1. Then **always** `lot.SetFlags(lot.GetFlags() & ~4)`. Return
AL = result (0 or 1). Only the first wall found at the location cell is removed; any other wall of the lot stays.

### 7.2 Refreshing walls: `UpdateRetainingWalls(lot)` (`0x005E37F0`, `__cdecl`, 4 bytes, no return)

Native lot-developer slot 62 wraps a direct call to this helper (§15.1); substituting wall factories does
not redirect that call. The lot argument is borrowed; temporary occupant/interface references are released
by the operations below.

Static RE (re_query.py asm; REA had no binary open). Callers: lot developer slot 62 `UpdateLotRetainingWalls`
(`0x006C4190`, which passes AL through) and `UpdateSurroundingRetainingWalls` (`0x005E39A8`). In this order:

1. If `lot.GetFlags() & 1 = 0` (slot 12): return; nothing else is called.
2. `w := null`; `(x, z) := lot.GetLocation()` (slot 21); occupant manager (`0x00B43D0C`) slot 33
   `GetFirstOccupantByStandardCityCell(&w, x, z, 0x895D1169)`. If false: `AddRetainingWallOccupant(lot, true)` (§7.1)
   and return.
3. `W := w.QueryInterface(0x295D1162)` (wall interface, building-development.md DG35). **`w` is not tested for
   null** before this call (the removal path of §7.1 does test it). If the QI fails:
   `AddRetainingWallOccupant(lot, true)`, Release `w`, return.
4. `R := lot.GetBoundingRect()` (slot 14); `top := 0`; `a := f32(lot.GetAltitude())` (slot 16);
   `old := W.GetEdgeHashes()` (slot 9 `0x005E2910`, copies object `+0x44..+0x50`).
5. `GetLotWallEdgeFlags(lot, a, R, new[4], &top)` (§7.1); its AL and `top` are **ignored**.
6. For `i` = 0, 1, 2, 3: if `old[i] ≠ new[i]` (32-bit integer compare): if `old[i] ≠ 0`,
   `AddRetainingWallModel(w, W, i, false, false)`; then if `new[i] ≠ 0`, `AddRetainingWallModel(w, W, i, true,
   true)`; `changed := true`. Equal hashes (zero or not) do nothing.
7. If `changed`: `W.SetEdgeHashes(new)` (slot 10 `0x005E2930`, a plain copy of the four words into `+0x44..+0x50`;
   it does **not** touch the edge mask at `+0x40`).
8. Release `W`, then Release `w`. Return (AL is whatever the last call left; slot 62 hands it on; LD3 callers
   ignore it).

What an update does **not** change on an existing wall: position, size (`top` is discarded, so a wall keeps the
height it was built with), wall type, lot reference, lot flag bit 2. A wall whose new hashes are all 0 is **kept**
(its models removed, the occupant stays). The edge mask changes only through `AddRetainingWallModel` below.

**`AddRetainingWallModel(occ, W, edge, add, insert)`** (`0x005E3400`, `__cdecl`, 20 bytes, AL = 1 on every path):
1. `P := occ.AsPropertyHolder()` (occupant slot 6, `occ + 8`); if null: return.
2. `G :=` occupant slot 20 `GetOrCreatePlaceableObject()` (`0x005E91C0`) when `add`, slot 19 (the existing one at
   `+0x30`, may be null) otherwise; if null: return; AddRef `G`.
3. Removal (`add = false`): `FindRetainingWallModel(G, edge, &m)` (`0x005E2FC0`: asks `G` slot 4 for at most 16
   attached objects; for each, QI `0xA9E2831E`, its slot 3 object `m`; match when the signed byte `m + 0x78` equals
   `edge`; first match is AddRef'd and returned true). If found: if renderer (`0x00B43DD0`) slot 27
   `IsModelInserted(m)`: slot 29 `RemoveModel(m)` (the renderer is not null-tested here); `G` slot 6 detach `m`;
   `m` slot 4 `SetOccupant(null)`; Release `m`. **The mask bit is left as it was**; `insert` is unused.
4. Add (`add = true`): create a model (`0xB0` bytes, constructor `0x005E17C0`, AddRef). Read properties
   `0x295961F2` (vector of u32, starts as five zeros) and `0x295961F3` (one word) from `P` when `P` has
   `0x295961F2` (slot 9), else from the resource `{type 0x6534284A, group 0x88CD66E9, instance 1}` (resource manager
   `0x00B43CA8` slot 3, IID `0xA52160F5`), Released after; the word is not initialised by this function when neither
   source has it. `occ` slot 10 gives the box (min starts (1,1,1), max (0,0,0); `0x005E2C30` derives it from the
   wall's stored position and size, so a stale size stays stale). `0x005E12C0(model, vector, word)`; `Init(model,
   box, edge, &ok)` (`0x005E2780`: stores `edge` at `+0x78`, builds the edge vertices; `ok` is written by its vertex
   setup `0x005E1BC0`).
   - `ok`: model slot 4 `SetOccupant(occ)`; `G` slot 5 attach the model; if `insert` and the renderer is non-null and
     not `IsModelInserted(model)`: `InsertModel(model)` (slot 28). Mask := `W.GetMask() | (1 << edge)` (slots 7/8,
     `+0x40`).
   - not `ok`: mask := `W.GetMask() & ~(1 << edge)`; the model is not attached.
   Release the model (destroyed when not attached), free the vector, Release `G`.

So after an update the saved record (§7 fields 4 and 5) holds the new hashes; mask bits are set for edges whose new
model built, cleared for edges whose new model failed, and **stay set for edges that went to hash 0**. A failed
build leaves a non-zero hash with a clear mask bit, one way to get the masks lacking hash bits that `Read` repairs
(T7) *[inferred: the cause of the 949 saved cases is not traced]*.

**`UpdateSurroundingRetainingWalls(lot)`** (`0x005E3960`, `__cdecl`, 4 bytes, no defined return): allocate an empty
list (12-byte head, `0x0090CF54`); `n :=` lot manager (`0x00B43D08`) slot 9 `GetLotsSurroundingLot(lot, list, 0xF)`
(zones.md §8.3 slot 10: the ring one cell outside the rect, row order, each lot once, one AddRef each; the lot itself
is excluded because it intersects the rect). If `n ≠ 0`: `UpdateRetainingWalls(L)` for each `L` in list order. Then
Release every non-null element, free the nodes and the head. The lot passed in is **not** updated itself. Callers:
`ClearLot` (`0x006C8A62`, after `AddRetainingWallOccupant(lot, false)`, `RemoveLotFoundation` and
`UpdateSurroundingLotFoundations`) and `AddLotFoundation` (`0x006C8C34`); see lot-developer.md.

---

## 8. Foundation occupant (`cSC4FoundationOccupant`, `0x49C05C8F`) *[compat]*

| # | Field | Type | Meaning, observed (8,981) |
|---|---|---|---|
| 1 | version | u16 | 2 |
| 2 | occupant base | §2.3 | type `0x68FD0C69` |
| 3 | foundation type | u32 | 34 distinct values, e.g. `0x194B1000` (4,637), `0x890B7313` (1,704) |
| 4 | maximum height | f32 | 0…32 |
| 5 | position | vec3 | |
| 6 | size | vec3 | |
| 7 | usage | s8 | 1, 2 |
| 8 | orientation | s8 | 0…3 |

---

## 9. Construction occupant (`cSC4ConstructionOccupant`, `0xA97F909E`) *[compat]*

A building under construction is a construction occupant on the lot plus the building occupant (completion < 1);
the lot is in state 2. Field names after #12 are not known; their grouping follows the Write order (§16 Q3).

| # | Field | Type | Meaning, observed (31) |
|---|---|---|---|
| 1 | version | u16 | 5 |
| 2 | occupant base | §2.3 | type `0x096E763C`, holder parent `(0, 0, 0)`, one property `0xAA1DD396` |
| 3 | lot | ref | IID `0xE818FCED` |
| 4 | building | ref | IID `0x87DDA3A9` (class `0xA9BD882D`) |
| 5 | transform | xform | mask 0 in all saves |
| 6 | world bounds | box | |
| 7 | local bounds | box | |
| 8 | has crane | bool | true when a crane model exists (4 of 31) |
| 9 | crane position | vec3 | |
| 10 | ground props | u32 n + n × ref | IID `0xE9793A65` (prop class `0x2977AA47`); 0…3 |
| 11 | roof props | u32 n + n × ref | same; 0…1 |
| 12 | effects | u32 n + n × ref | IID `0x2977AA49` (effect props), written as the effect-prop interface pointer; 1…3 |
| 13 | current effect | ref | IID `0x2977AA49` |
| 14 | unnamed | s8, 5 × u32, 2 × f32, 5 × u32, s8, 2 × f32, bool, 2 × u32, 2 × f32, u8 | construction / crane animation and daily progress state (decoded below) |
| 15 | crew | u32 n + n × {bool, bool, ref} | ref IID `0xA9B40F05` (`cISC4Automaton`; class `0x896E75AF` `cSC4Pedestrian`, record in `save-records.md`); n 0…5 |

`Read`: versions < 4 → false; versions ≥ 4 are read with this layout, version 4 using the legacy transform reader
(`0x005DA930`). Versions > 5 are read as 5 *[quirk]*.

### 9.1 Where the fields live (`0x005CD250` / `0x005CE770`)

The class embeds the occupant base at `+0x08` (constructor `0x005D00F0`: `cSC4Occupant::cSC4Occupant` `0x005E9220`
with ECX = this+8), so base fields are at object +0x2C onward. Object size `0x1A0`; GZCOM creator `0x005D0BC0`
passes argument 1, which selects the per-instance-table vtable pair (`+0x3C` = `0x00AA211C`, `+0x40` = `0x00AA2264`,
and `+0x19C` = `0x00AA1F38`); argument 0 leaves them shared.

**Three `this` frames** *[compat]* (answers OP13/OP14, 2026-10-21). The class's methods run on different
sub-objects, so the same field has three offsets in the binary. Every offset in this section and in §9.2 is given
in the **object frame** (the address the GZCOM creator returns, main vtable `0x00AA2250` at +0):

| Methods | `this` | Object offset = |
|---|---|---|
| `SetLot`, `DoDailySim`, `DoAnimationTick`, `AnimateCrane`, `RebuildModel`, `CreateSparks`, the `Attempt*Crane` steps | object + 0 | the offset as coded |
| `DoMessage` `0x005D1090` (message-target vtable `0x00AA2240` at +4) | object + 4 | coded + 4 |
| `Init` `0x005D1300`, `Shutdown`, `GetPosition`, `GetBoundingBox` (occupant vtable `0x00AA21B8` at +8) | object + 8 | coded + 8 |
| `Write` `0x005CD250`, `Read` `0x005CE770` (serializable vtable `0x00AA219C` at +0xC) | object + 0xC | coded + 0xC |

Earlier revisions of this table used the Write-frame offsets (12 lower than the object frame), which made the
saved lot / building references look like separate copies of the live ones and shifted every #14 role by three
fields. The saved fields, in Write order (object frame; the Write-coded offset in brackets):

| Offset [Write] | Type | Role, observed | Record field |
|---|---|---|---|
| `+0x68`/`+0x6C` [`+0x5C`/`+0x60`] | 2 × ref | the **live** lot / building references that `SetLot` holds (there is no separate saved copy); `Read` releases the old ones and reads into the same fields | 3, 4 |
| `+0x70` [`+0x64`] | xform | `operator<<` `0x005DA8C0`, read with `0x005DA930` (v4) or `0x005DA710` (v5) | 5 |
| `+0xA8` [`+0x9C`] | box | world bounds; the box `GetPosition` / `GetBoundingBox` read (their frame offset `+0xA0` = object `+0xA8`); `RebuildModel` writes it as the scaffold box `+0xC0` widened by sfPostWidth in x and z (`0x005CB76D`–`0x005CB799`; OP17, re-construction 2026-10-23) | 6 |
| `+0xC0` [`+0xB4`] | box | the scaffold box `RebuildModel` stores (x / z = building footprint ∓/± 1.0, constant `0x00B0A7D0`); used by `CreateConstructionCrew`, `AddRoofProps`, `CreateSparks` | 7 |
| `+0x104` [`+0xF8`] → `+0xD8` [`+0xCC`] | bool | written as (live crane model pointer `+0x104` != 0; `CreateCrane` sets it); read into the load-only flag `+0xD8`, never into `+0x104`. `Read` also sets `+0xD9` = 1 (refresh pending, §9.2) | 8 |
| `+0x108` [`+0xFC`] | vec3 | crane position (`0x005DA580`) | 9 |
| `+0x128`, `+0x12C` [`+0x11C`, `+0x120`] | ground / roof-prop lists | `std::list`, nodes at +8 hold the prop ref (IID `0xE9793A65`); count then refs | 10, 11 |
| `+0x130`/`+0x134` [`+0x124`/`+0x128`] | spark-effect vector | `vector<cRZAutoRefCount<…>>` begin/end; count then one ref per entry (pointer + 4 of each element, IID `0x2977AA49`) | 12 |
| `+0x13C` [`+0x130`] | scaffold effect | written as (pointer + 0x10) or 0 (IID `0x2977AA49`); `DoDailySim` switches it to removal (its slot 8, argument 1) | 13 |
| `+0x140` [`+0x134`] | s8 | **state** (ld §6.7.5, 0…4); stored as int32, written as its low byte, read sign-extended | 14a |
| `+0x144` [`+0x138`] | u32 | `T0` (sim day of `SetLot`) | 14b |
| `+0x148` [`+0x13C`] | u32 | unused: 0 from the constructor, only written and read *[inferred: no other access in the class]* | 14b |
| `+0x14C` [`+0x140`] | u32 | `nextDay` | 14b |
| `+0x150` [`+0x144`] | u32 | `N` (segments, ≥ 1; `RebuildModel`) | 14b |
| `+0x154` [`+0x148`] | u32 | `counter` (current segment) | 14b |
| `+0x158` [`+0x14C`] | f32 | scaffold span `N · segH` (`RebuildModel`) | 14c |
| `+0x15C` [`+0x150`] | f32 | `startY` (grown height + 0.1, `RebuildModel`) | 14c |
| `+0x160`…`+0x16C` [`+0x154`…`+0x160`] | 4 × u32 | `A`, `B`, `C`, `E` (ld §6.7.4) | 14d |
| `+0x170` [`+0x164`] | u32 | `msAcc` (animation milliseconds since the last phase change) | 14d |
| `+0x174` [`+0x168`] | s8 | crane state (0 swing, 1 pause, 2 pick target; ld §6.7.6); int32 in memory, read sign-extended | 14e |
| `+0x178`, `+0x17C` [`+0x16C`, `+0x170`] | 2 × f32 | crane swing limits min / max (radians, `AttemptAddOuterCrane`, ld §6.7.6) | 14f |
| `+0x180` [`+0x174`] | bool | unused: false from the constructor, only written and read *[inferred: no other access in the class]* | 14g |
| `+0x184`, `+0x188` [`+0x178`, `+0x17C`] | 2 × u32 | crane accumulator and duration (ms) | 14h |
| `+0x18C`, `+0x190` [`+0x180`, `+0x184`] | 2 × f32 | crane target angle and previous target angle (radians) | 14i |
| `+0x124` [`+0x118`] | u8 | crane type index (into the boom-radius table `0x00B22F70`; set at `0x005CDC32`, passed to `CreateCrane`) | 14j |
| `+0x194` [`+0x188`] | crew list | `std::list`, each node {u8 at +8, u8 at +9, 16-byte sub-object at +0xC…}; count then {bool, bool, ref} per entry | 15 |

The Write writes the crew count from the `+0x188` list, then per node reads two bytes (`operator>> bool`
`0x005D9FA0` on Read) and one `SC4ReadGZSerializable` reference with IID `0xA9B40F05`.

### 9.1.1 Construction reference identity and ownership (LR13)

The reference writer accepts an **interface pointer**, not necessarily the complete-object address or
the Serializable address. For each field below, `SC4WriteGZSerializable` (`0x005D99A0`) obtains
the stream's writer view (IID `0x6999A8FB`), queries a nonnull input for `cIGZSerializable`
(`0xE4FDA3D4`), and passes the resulting Serializable, or null, to writer slot 26 (`+0x68`).
It releases that acquired Serializable after the slot returns, including when the slot returns false,
and releases the acquired writer view. It neither transfers the construction owner's reference nor
adds a permanent reference of its own. A writer retaining a reference beyond the callback must hold
its own reference before the callback returns. A failed input QI is represented as null; the helper
does not substitute the input address or a guessed interface (`0x005D99BE–0x005D9A0A`).

All construction offsets here use the complete-object frame of §9.1. `O` below is a complete
`cSC4PropOccupantVisualEffect` object, not the construction object.

| Record field | Stored reference and input to the reference writer | Reader IID / result retained by construction |
|---|---|---|
| 10 ground props | Node `+8`, the prop interface, passed unchanged | `0xE9793A65`, prop interface |
| 11 roof props | Node `+8`, the prop interface, passed unchanged | `0xE9793A65`, prop interface |
| 12 sparks | Each vector entry is the effect class view `O`; nonnull input is `O+4`, its embedded prop view | `0x2977AA49`, effect class view `O` |
| 13 dust/scaffold effect | Construction `+0x13C` holds the effect class view `O`; nonnull input is `O+0x10`, its Serializable view | `0x2977AA49`, effect class view `O` |
| 15 crew | Node `+0xC`, the automaton interface, passed unchanged after its two bools | `0xA9B40F05`, automaton interface |

Null remains null through each pointer adjustment. The field-12 adjustment is to the **entry's
referenced object**, not to the address of the vector entry. Writer evidence:
`0x005CD300–0x005CD305`, `0x005CD342–0x005CD347`, `0x005CD382–0x005CD391`,
`0x005CD3A6–0x005CD3B9`, `0x005CD530–0x005CD54D`. Reader evidence:
`0x005CE870–0x005CE87D`, `0x005CE8D0–0x005CE8DD`, `0x005CE947–0x005CE954`,
`0x005CEA32–0x005CEA41`, `0x005CEBBD–0x005CEBCA`.

**The effect's Serializable is the derived record.** Its constructor embeds a prop at `O+4`
and installs the effect's occupant at `O+0xC`, Serializable at `O+0x10` (vtable
`0x00AA4DC4`), and holder at `O+0x14` (`0x005F0215–0x005F024D`). Serializable slots
3/4/5 are effect Write `0x005F03B0`, Read `0x005EF750`, and class ID `0x005F02B0`
(`0x2977AA49`). The Serializable QI thunk `0x005F02F0` subtracts `0x10`; the prop QI
thunk `0x005F02C0` subtracts 4. Both reach effect QI `0x005ECC10`, which answers
`0x2977AA49` with `O` and an AddRef. For Serializable, it delegates through prop QI
`0x005EC960` and occupant QI `0x005E89C0`; the result is `O+0x10`, with an AddRef
through the installed derived vtable. The embedded occupant is a base subobject of this
same live effect, not a separately allocated bare occupant. Therefore valid live effects
in fields 12 and 13 resolve to the same derived Serializable regardless of those two input
views. Re-querying `O+0x10` does not select the bare class `0xA9BC9AB6` or its vtable
`0x00AA41BC`. A separately supplied bare occupant can serialize as a bare occupant, but
does not thereby acquire the effect IID; base QI's accepted IIDs are shown at
`0x005E89C0–0x005E8A1B`.

**Read ownership and rejection.** The reader helper `0x005D9730` obtains reader IID
`0x4999A8F5`; its inner helper `0x005D96C0` initializes the destination to null, obtains
a temporary Serializable through reader slot 26, then queries that object for the field's
literal IID. It releases the temporary Serializable on success and failure. Successful
non-null QI leaves one acquired field-interface reference for the construction object to
own; successful null input leaves null. A nonnull Serializable that rejects the requested
IID leaves null and returns false (`0x005D96D6–0x005D971E`). There is no transfer of the
temporary Serializable into the field and no extra acquisition merely for storing the QI
result. Any capture pin held by an observing writer is separate from this read acquisition.

Construction Read **ignores these reference-helper return values** and continues with the
remaining fields. Its final success depends on the stream error flag, so it can return true
with a rejected reference replaced by null (`0x005CEA41–0x005CEA55`,
`0x005CEBCA–0x005CEC10`) *[quirk]*. A same-input record comparison must report the
nonnull-to-null reference change as a mismatch, even if all bytes were consumed and Read
returned true. If the stream itself marks an error, report that failure as well. Do not
preserve the rejected nonnull token, coerce a bare occupant into an effect, or count such a
record as exact. Reference identity is compared through the stream's object mapping;
raw addresses of different interface views need not be equal.

**Write lifetime.** Construction Write traverses existing references; it does not create,
replace or remove occupants, change its list/vector membership, or rewrite the references
in the table. Its reference QIs have the balanced callback lifetime above. Effect Write
does create and destroy a temporary empty string when either live effect handle is set
(§10.2; `0x005F0414–0x005F0469`); that temporary is not an occupant or reference-table
object. Inline occupant/prop records are written directly, not by allocating bare occupant
copies (`0x005CD264`, `0x005ECEA4`, `0x005E8D3E`). Nested property variants and the
stream's own storage have their separate serialization contracts; this rule does not
forbid those callbacks from acquiring their own references. An observer must preserve
every reference it retains independently of the helper's temporary QI. A last-reference
release warning alone does not prove that field 13 legitimately changed identity. The
train180 254/250-byte observation is a reported mismatch, not evidence that a four-byte
loss or a bare-occupant substitution is required native behavior.

### 9.1.2 Field-13 teardown identity and observation boundary (LR14)

**Answer: the bare callback is not a second form of a live dust effect.** Construction creates a
visual effect, acquires a reference, and stores its complete effect view at `+0x13C`
(`0x005CBC17–0x005CBC46`, `0x005CBD05`). Write merely adjusts that stored nonnull pointer by
`0x10` and queries Serializable (`0x005CD3A6–0x005CD3B9`). Neither operation creates a temporary
bare occupant. There is, however, a native teardown route to exactly the reported base vtable:
Shutdown removes the effect's occupant view `O+0xC` from the occupant manager, when available,
then releases the construction-owned effect reference **without clearing `+0x13C`**
(`0x005D0F72–0x005D0F99`, Shutdown's `this` is object+8). If this destroys the effect, its
destructor eventually runs the embedded occupant destructor at `O+0xC`
(`0x005F0399–0x005F03AB`). That destructor installs `0x00AA41BC` at `O+0x10`
(`0x005E8C44–0x005E8C51`), the very address field 13 would pass to the helper. Thus a
retained construction can contain a nonnull, released effect address whose old Serializable
storage has the bare-occupant vtable *[quirk]*. After destruction this is stale storage, not a
valid bare object that the construction now owns. Allocation reuse can change it again.

The export misleadingly names `0x005E8C40` as a vector destructor; the vtable stores and
the effect destructor's `O+0xC` argument establish the occupant teardown operation here.
The derived Serializable vtable is installed before this base teardown
(`0x005F0347–0x005F0368`). A bare vtable observed afterwards therefore does not prove that
a separately allocated bare occupant ever existed. The field is not cleared by the remainder
of Shutdown (`0x005D0F99–0x005D1088`). Repeating Shutdown does not restore it: the initialized
guard and base Shutdown precede reference cleanup (`0x005D0CE6–0x005D0D06`).

This establishes a native explanation for the **identity pattern**, not a reconstruction of
run 470's timing. Its callback/class ID, byte counts and later exception alone cannot distinguish
post-Shutdown stale storage, teardown reentrancy, address reuse, or incorrect observer framing.
They do not establish that Shutdown ran on that instance. In particular, adding a reference to
an already destroyed allocation cannot make it a valid live object again. No result is specified
for invoking QI, Write or Release through such stale storage; do not reproduce its accidental
bytes or access violation as valid serialization behavior. A still-live, separately supplied bare
Serializable remains the well-defined rejected-input case in §9.1.1/T69.

For a **valid live effect**, ownership across writer slot 26 is:

| Boundary | References owned, excluding unrelated owners |
|---|---|
| Before helper | Construction owns one effect reference; an observer owns none unless separately acquired. |
| Slot 26 entry | Helper additionally owns one successful Serializable-QI reference. The callback borrows that view for its duration. |
| Callback returns | Any reference retained by the callback must already have its own acquisition. The helper then releases its QI reference, even on callback false. |
| After helper | Construction still owns its original reference unless a separate lifecycle action released it. Only the observer's independent acquisitions belong to its cleanup. |

For example, with only construction owning the effect, helper QI changes count 1→2; a
callback pin changes 2→3; helper cleanup changes 3→2; pin cleanup changes 2→1.
Pin cleanup can legitimately reach zero if construction and all other owners released their
references meanwhile. It cannot reach zero in the unchanged live-owner example. The return
count alone cannot tell these cases apart. Reader rejection separately releases its temporary
Serializable and retains no effect reference. It neither consumes construction's ownership nor
authorizes keeping an observer pin forever. A nonnull rejected field is still a mismatch; the
reported 254/250 bytes authorize no four-byte correction.

**Additional observation required for lifetime attribution.** At the original Write entry, while
the caller's construction is known alive, record its complete address, initialized state and raw
field-13 value. Correlate that instance with native Shutdown entry/exit, the effect destructor
entry/base-vtable installation, and allocator reuse. At helper entry and slot-26 entry record
the actual argument/view address, existing vtable and successful QI output; record each
observer acquisition/release and the callback's nesting. Obtain this through passive native
call/write tracing during normal operations, not by calling lifecycle methods or probing guessed
interfaces. Inspect referenced storage only while its lifetime is established; once destruction is
seen, record the stale field value without dereferencing it or trying to pin it. A genuine helper
temporary would have a distinct creation/destruction interval inside Write; these original
paths have none for occupants. A matching `O`, Shutdown release and base-vtable installation
before the callback identify stale teardown storage; a different argument before helper QI
identifies a framing/capture discrepancy. Absence of either trace is insufficient to choose a
cause. An initialized-state check is a useful exclusion, not a lifetime guarantee across callbacks.

### 9.2 Life cycle (`0x005D1300` Init, `0x005D0890` SetLot, `0x005D0680` DoDailySim)

- **`Init`** (`0x005D1300`, occupant frame; answers OP15 / OP7, 2026-10-21), in this order: return true at once if
  the occupant's own `IsInitialized` (occupant slot 5) is true; run the occupant base `Init` (`0x005E8C90`, §2.4)
  **unconditionally**; `StaticInit` once per process (flag `0x00B22F60`); subscribe **four** messages through the
  message server `0x00B43CCC` slot 0x14, target = the message-target sub-object at object `+0x4`: `0x66956814`,
  `0xA6B79602`, `0x06CDB65B`, `0x26D31EC1` (`0x86AD10EE` is **not** subscribed: the animation-tick manager calls
  `DoMessage` directly, lot-developer §6.7.8); register with the animation-tick manager (service
  `0x00602600`, slot 0x14, priority `0x80`); fill 6 × 5 **texture-binding** slots from the framework
  (`0x1AC0E11A`/`0xFAC0E219` factory, table `0x00AA1E30…0x00AA1E48`; the old SimGrid type was a
  decompiler error). IDs, render fields and the construction visual contract are in lot-developer §6.7.15;
  pack the render state at object `+0xF8`
  (`0x007D2B50`); finally call **occupant slot 23 `AddOccupantGroup(0x5007)`** on itself (`CALL [EAX+0x5C]` at
  `0x005D1505`, `ECX` = the occupant sub-object, vtable `0x00AA21B8` slot 23 = `0x005E9350`, one stack argument,
  callee pops 4). Its compiled body (`0x005E9350`): read the occupant's u32-array property `0xAA1DD396` (the
  property of §9 #2), and if `0x5007` is not in it (`std::find` `0x004696C0`), append it, write the property back
  (`0x005FE6B0`) and post the queued message `0xEBFF18A3` (occupant slot 15 `0x005E9010`, carrying the occupant and
  the group ID); if it is already present nothing happens. The return value is not used. So a construction occupant
  always carries occupant group `0x5007` in property `0xAA1DD396`; a loaded one already has it from its base record
  and gets no second entry and no message.
- **`DoMessage`** (`0x005D1090`, object `+0x4` frame): `0x66956814` → `DoDailySim`; `0x86AD10EE` →
  `DoAnimationTick(ms)` with `ms` = the message's dword at `+0x14` (message slot 14 getter; the field the tick
  manager fills with the clamped **µs**, called data2 in lot-developer §6.7.8) divided by 1000, unsigned and
  truncating (`MUL 0x10624DD3; SHR 6`); `0x06CDB65B` → the common flag-2 renderer dance (§10.3); `0xA6B79602` → sets
  byte object `+0x100` = 1 when the dwords at +4 of the two objects returned by message slots 10 and 15 differ; `0x26D31EC1` → the refresh below. All
  return true.
- **Refresh after a load** (`0x26D31EC1`, `0x005D10C7…0x005D1155`): only when the refresh-pending byte `+0xD9` is set
  (`Read` sets it, nothing else does): `RebuildModel`; if the load-only has-crane byte `+0xD8` (record #8) is set,
  `CreateCrane(position x +0x108, position z +0x110, crane type +0x124)`; `AddToRenderer`; then `DoAnimationTick(0)`,
  with `state` temporarily set from 2 to 1 around the call when it is 2 (so a state-2 load redraws the scaffold at
  the state-1 height instead of re-setting the building completion), and clear `+0xD9`. The crane model therefore
  exists only after this refresh; a `Write` before it saves record #8 = false *[quirk]*.
- `Shutdown` (`0x005D0CE0`) unsubscribes the four, unregisters the tick, sends `SendConstructionMessage(0xC9E7B1E8, 0)`
  first, and releases the lot/building/props/effects/crew.
- **`SendConstructionMessage(type, data3)`** `0x005CBD20`: posts a `cRZMessage2Standard` (0x2C bytes) with type =
  arg1, void1/data2 = this, data3 byte = arg2, through the message server slot 0x10 (queued).
- **`SetLot(lot)`** `0x005D0890` (also reachable with the "already initialised" guard): releases the old lot and
  building references (`+0x68`/`+0x6C`), holds the new lot, and when the lot has a building (`lot` slot 0xC0/4=48,
  `GetBuilding`) drives the whole placement: `RebuildModel` `0x005CAD60`, `DivideLot` `0x005CB9F0`,
  `AttemptAddCrane` `0x005CE360`, `AddGroundProps` `0x005CF790`, `CreateConstructionCrew` `0x005CFA60`,
  `AddRoofProps` `0x005CFC40`, `CreateDustClouds` `0x005CBC10`, `CreateSparks` `0x005CEC20`,
  `CreateLotTexture` `0x005CF380`. It then reads the sim day (`0x00B43D5C` slot 0x20/4=8) into `+0x144`, computes
  the segment schedule into `+0x150…+0x164` from the lot's growth rate (`0x00B0A7D8`), resets state `+0x140` = 0,
  sets completion 0 (building slots 9/10 + a queued `0x2977AE2F` message), sets the scaffold height to
  ground + 0.001 (`0x005CC020`), and if a crane exists (`+0x104` != 0, `0x005D0B46`): target `+0x18C` := min
  `+0x178`, previous `+0x190` := max `+0x17C`, accumulator `+0x184` := 1, duration `+0x188` := 0, crane state
  `+0x174` := 2, and `FUN_005CBE10(min)` poses the boom at the min angle (`0x005D0B4E…0x005D0B87`). Finally `SendConstructionMessage(0x49E7B1E2, 1)` (segment-start), then
  `SendConstructionMessage(0xC9E7B1E8, 1)` and `AddToRenderer` `0x005CA960`.
- **`DoDailySim`** `0x005D0680` matches lot-developer.md §6.7.5 (segment state machine at `+0x140`; sends
  `SendConstructionMessage(0x49E7B1E2, 1)` at segment starts, completion at `0x5D0723`–`0x5D0790` fires the sparks
  iterator over the `+0x12C` list and the effect slot 9/7 dance). This class's copy adds the day-stamp
  bookkeeping: state 0/2 path at `0x005D0701`, and the completion path walks the effect list and drives
  `SendConstructionMessage(0xC9E7B1E8, …)`.
- **`GetPosition`** `0x005CA7D0` / **`GetBoundingBox`** `0x005CA820` (occupant frame) read the saved world box,
  record #6 at object `+0xA8…+0xBC` (their coded `+0xA0…+0xB4`); `GetPosition` returns the box centre (each
  coordinate `(min + max) * 0.5f`, constant `0x00A84D2C`).
- **`DoAnimationTick`** `0x005CE110` (lot-developer §6.7.6) adds `ms` to `msAcc` `+0x170` and interpolates by
  `state` `+0x140`; it is also called with 0 by the refresh above.
- The life-cycle fields (state, `T0`, `nextDay`, `N`, `counter`, span, `startY`, `A`, `B`, `C`, `E`, `msAcc`, the
  crane fields) all sit in record #14 as tabled in §9.1, so a loaded construction resumes its schedule from the
  saved values with no recomputation: `DoDailySim` reads only `state`, `nextDay`, `counter`, `N`, `B`, `C`, `E`;
  `RebuildModel` (refresh) recomputes `N`, span and `startY` from the building as before the save.

---

### 9.3 Non-Write method inventory and live observation scope (LR14)

The following is the construction owner's inventory beyond the Write record comparison.
Addresses identify original entry points, not additional ABI exports. Detailed simulation and
rendering rules are in lot-developer §6.7 and §9.2 above. **Getter** means a call can inspect a
known-live object without advancing its state; preserve the documented `this` frame and output
arguments. **Passive** means observe the original call and its outputs when the game makes it;
do not invoke it as a read-only shadow probe. **Lifetime** changes references or allocation and
must be traced with balanced ownership. None is covered merely by an exact Write round trip.

| Method(s) and entry address(es) | Observable surface / live-city category |
|---|---|
| GetPosition `0x005CA7D0`; GetBoundingBox `0x005CA820` | Getter: world-box centre and bounds, occupant frame. |
| GetGZCLSID `0x005D04A0` | Getter: construction class ID `0xA97F909E`, Serializable frame. |
| QueryInterface `0x005CA7A0` | Lifetime: construction IID `0x496E7636` returns object view with AddRef; other IIDs delegate to occupant at object+8. Inspect only known IIDs; balance every successful acquisition. |
| AddRef `0x006491C0`; Release `0x006491D0` | Lifetime: object+8 occupant counter; final Release destroys. Reference counts are observations of these natural operations, not a reason to probe Release. |
| operator.new `0x005CA8A0`; constructor `0x005D00F0`; GZCOM creator `0x005D0BC0`; deleting destructor `0x005D0C00` | Lifetime: allocation, initial fields/interface tables, destruction/free. No read-only invocation on an existing city object. |
| Init `0x005D1300`; Shutdown `0x005D0CE0` | Passive: registration, messages, initialized state, attachment cleanup and released references; §9.2 and §9.1.2. |
| StaticInit `0x005CED80` | Passive: once-per-process configuration/resources, not a per-record getter. |
| Read `0x005CE770` | Passive during natural load: versions, field restoration, acquisitions/rejections, pending refresh. A comparison Read must target an independent replacement object, not the live native owner. |
| SetLot `0x005D0890` | Passive: lot/building ownership, placement, schedule reset and messages. |
| DoMessage `0x005D1090` | Passive: daily/tick, refresh and renderer/lighting notifications; message frame object+4. |
| DoDailySim `0x005D0680`; DoAnimationTick `0x005CE110` | Passive: schedule transitions, completion, scaffold/effect/crew changes and animation accumulator. Calling with zero elapsed time can still mutate state. |
| SetBuildingCompletion `0x005CA8D0`; SendConstructionMessage `0x005CBD20` | Passive: building completion and queued notification type/payload. |
| AddToRenderer `0x005CA960`; RebuildModel `0x005CAD60` | Passive: renderer membership, model buffers, bounds and derived schedule geometry. |
| DoRotationChange `0x005CB7C0`; Draw `0x005CC410` | Passive: rotation, lighting-dirty clearing/vertex colours, render commands/state restoration. Draw is not a pure getter; lighting is part of DoRotationChange, not a separate named construction method. |
| DivideLot `0x005CB9F0`; RemoveRectangle `0x005CCD70` | Passive: construction placement regions/footprint subtraction. |
| SetCraneAngle `0x005CBE10`; SetScaffoldHeight `0x005CC020` | Passive: mesh/crane pose and attached prop/crew positions. |
| CreateCrane `0x005CC820`; AnimateCrane `0x005CD570` | Passive: crane allocation/model state, pose and animation state. |
| AttemptAddOuterCrane `0x005CD710`; AttemptAddInnerCrane `0x005CDDA0`; AttemptAddCrane `0x005CE360` | Passive: placement attempts, success/failure, resulting crane type/position and retained model. |
| CreateAndPlaceProp `0x005CCF20`; AddGroundProps `0x005CF790`; AddRoofProps `0x005CFC40` | Passive: prop creation, position, initialization and owner list membership. |
| CreateDustClouds `0x005CBC10`; CreateSparks `0x005CEC20`; ScatterSparks `0x005CFE20` | Passive: effects, names/priorities, field-13 retention and spark membership/placement. |
| CreateAutomata `0x005CE580`; CreateConstructionCrew `0x005CFA60` | Passive: crew allocation/placement, references and two per-entry flags. |
| CreateLotTexture `0x005CF380` | Passive: texture creation and attachment to the lot. |

Interface adjustment thunks are the same operations, not separate behavioral coverage:
occupant QI `0x005D04C0`, Serializable QI `0x005D04D0`, holder QI `0x005D04E0`;
holder destructor `0x005D04F0`, Draw thunk `0x005D0640`, holder Write/Read
`0x005D0660`/`0x005D0670`; and AddRef/Release thunks `0x0064DD90–0x0064DDE0`.
The Draw thunk uses the adjustment stored before its interface (`0x005D0640–0x005D0643`);
do not infer a fixed complete-object address from its exported mangled name. Destructor body
`0x005D0C60` is teardown behind the deleting destructor, not a getter.

**Inherited surface.** Occupant vtable `0x00AA21B8` slots 5–33 additionally expose
IsInitialized; the occupant-view conversion; type ID; GetBoundingCityCells; removal flags
set/unset/CanRemove; highlight get/set; visibility set/get; GetOrCreatePlaceableObject and
SetPlaceableObject; IsOccupantGroup/AddOccupantGroup/GetOccupantGroups; occupant-manager
bounds get/set; the inherited slots 27/28; and SetFlag/SetAllFlags/ClearFlag/IsFlagSet/GetFlags.
Read-only inspection covers initialized/type, bounds, CanRemove, highlight, visibility,
group membership/enumeration and flag getters. GetOrCreatePlaceableObject can allocate;
setters and group additions are passive-only. Slot 9 and slot 28 point to the shared
`0x00648F10` one-argument no-op, not the unrelated tool operation shown by the export alias.
The main slot 3 and occupant slot 6 are subobject conversions (`0x006C2040` returns this+8
for nonnull input), not acquired references or lot getters. Slot 7 (`0x006BDD80`) reads
the occupant type at its frame+0x24, not a simgrid count. These folded-name aliases do
not add other owners' behaviors to construction. Slot 27 is the shared false-returning
one-argument method `0x005DEB40`; it can be observed as false without invoking another
owner's named operation.

Holder vtable `0x00AA2138` slots 3–21 supplies the shared property-holder surface:
HasProperty, GetPropertyList, GetProperty overloads, AddProperty overloads,
CopyAddProperty, RemoveProperty, RemoveAllProperties, EnumProperties overloads and
CompactProperties. Has/Get/Enum can observe the live holder under §2's ownership rules;
returned property/container views are not new construction records. Mutation/compaction is
passive-only. Serializable and message-target views add no separate lifecycle algorithm.
Inherited behavior remains specified with its owning base; the construction inventory does
not claim an independent comparison for it.

Natural calls can be passively observed in a live city only when their trigger occurs:
creation, load, day/tick, camera rotation, drawing, or removal. Absence of a call is not a
passing comparison. A load/month Write inventory neither stimulates those triggers nor
establishes a census of every live construction (§3.1.1). In particular, holding a construction
reference keeps its allocation alive but does not keep its children initialized/attached after
Shutdown. Record-only observation must exclude known shut-down instances and report an
unknown/stale lifetime as an incomplete observation rather than dereference it for a score.

## 10. Prop occupants

### 10.1 `cSC4PropOccupant` (`0x2977AA47`) *[compat]*

| # | Field | Type | Meaning, observed (134,420 + 948 inside effect props) |
|---|---|---|---|
| 1 | version | u16 | 6 |
| 2 | occupant base | §2.3 | type `0xA823821E` |
| 3 | prop type ID | u32 | the prop exemplar's ID. In memory the prop keeps a u16 index into a process-wide ID table (`GetPropTypeIndex` `0x005EF2E0` appends unknown IDs on read) |
| 4 | bounds | box | |
| 5 | orientation | s8 | 0…3 |
| 6 | state | u8 | 0…2 |
| 7 | time of day start, end | 2 × u8 | tenths of an hour; (0, 0) = always (120,401); e.g. (180, 80) = 18:00–08:00 |
| 8 | has date range | bool | true for 174 props |
| 9 | date range (only if #8) | 4 × u32 | interval in days (365 in all saves), duration (4…28), start day, end day |
| 10 | chance | u8 | 1…100 (percent) |
| 11 | prop usage | u8 | 1, 2, 4 |
| 12 | lot tag | u32 | tag of the lot configuration object that placed it (`lot-developer.md` §6.3); `0xFFFFFFFF` = none (221) |
| 13 | timer bits | u8 | 0, 6, 7, 11, 13, 14, 15 |

`Read` does **not** check the version: any version is read with this layout; field 13 only if version > 5, else
it becomes `0x0F` *[compat]*. If the base `Read` fails, `Read` returns `GetError() = 0` without reading more (so it
can return true). Prop holders observed: `0x89A1C16C` (8,124, unnamed), `0x48E95539` (zot number,
`lot-developer.md`), `0xCAA46595`, `0xCAA45670`, `0xCAA45672` (`neighbors.md`).

### 10.2 `cSC4PropOccupantVisualEffect` (`0x2977AA49`) *[compat]*

| # | Field | Type | Meaning, observed (948) |
|---|---|---|---|
| 1 | version | u16 | 3 (`Read`: anything else → false) |
| 2 | prop record | §10.1 | complete, from its own version 6 |
| 3 | unnamed | u8, u8 | (1, 0) 876, (1, 1) 40, (0, 1) 32 |
| 4 | effect name | gzstr | |
| 5 | unnamed | u32 | 0 in all saves |

*[quirk]* `Write` writes an **empty name and 0** instead of #4 and #5 whenever the effect is live (either of its two
effect handles is non-null). So lot effect props (849, exemplar parent) are saved with empty names; only the
construction effects (99, parent `(0, 0, 0)`) carry names: `welding_sparks` (68), `large_dust` (20),
`medium_dust` (11). On load the name comes back from the exemplar *[inferred]*.

### 10.3 `cSC4SignpostOccupant` (`0xAB72FBB3`) *[compat]*

A region-view signpost: the user places it with the sign tool and edits its text on the region map. It embeds a
complete `cSC4PropOccupant` at `+0x04` (constructor `0x005F5519` calls the prop constructor with ECX = this+4), so
every prop field and the prop save record (§10.1) live at object +4 onward; the base occupant sits inside the prop
at +0x0C (prop-relative +8), and the prop's serializable Write/Read (`0x005ECE90`/`0x005EF360`, record §10.1) are
called inline by the signpost's own Write/Read. Object size `0x590` (the allocator pool at `0x00B0A96C`).

| # | Field | Type | Meaning |
|---|---|---|---|
| 1 | version | u16 | 2 |
| 2 | prop record | §10.1 | complete inline, from the prop's own version 6 (base occupant type `0xA823821E`) |
| 3 | text | string | the sign text (object `+0x98`), stream slot 15 (`+0x3C`, a string; OP2) |
| 4 | position | vec3 | the model's position, object `+0xDC/+0xE0/+0xE4` (`operator<<` `0x005DA580`) |

*Corrected 2026-10-22 (re-signposts): `Write` (`0x005F5080`) and `Read` run on the `+0x10` serialisable view, so their
`+0x88` and `+0xCC…+0xD4` are object `+0x98` (the text) and `+0xDC…+0xE4`; `SetPosition` runs on the `+0x0C`
occupant view, whose `+0xD0…+0xD8` are the same three object fields (`0x005F4E9F`–`0x005F4EAE`). The offsets in
the next paragraph are those views'.*

The vec3 is the signpost's own position: `SetPosition` (occupant view slot 9, `0x005F4E90`) adds the requested
delta into six 3×3 orientation matrices at `+0xDC…+0x118` (one per orientation), writes the absolute position into
`+0xCC/+0xD0/+0xD4`, sets byte `+0xA9` bit 0 (moved) and increments byte `+0xA8` (a change counter), translates the
box, and re-registers with the renderer (`0x00B43DD0` slot 0x7C) when flag 2 is set. `Read` restores the vec3,
increments `+0xA4` (a load counter) and sets `+0xA9` |= 1. Version 1 records additionally read one u32 (slot 0x24)
whose value is discarded *[compat]*.

State beyond the prop (measured on the object base):

| Offset | Type | Role |
|---|---|---|
| `+0x6C`/`+0x70` | 2 × ptr | embedded `cS3DModelInstanceBasic` (`0x00801CF0`) sub-object; `+0x70` gets vtable `0x00AA5298` (edit mode) or `0x00AA5498` (game mode) from the constructor's bool argument |
| `+0x88` | u32 | not saved (corrected: record field 3 is the text at `+0x98`) |
| `+0x98` | `cRZString` | the sign text; constructor installs the shared empty-string vtable `0x00A80810` |
| `+0x9C…+0xA4` | 3 × ptr + u8 | model-table back pointer, edit target, load counter (`+0xA4`) |
| `+0xA8`, `+0xA9` | 2 × u8 | change counter and moved flag (bit 0) |
| `+0xAC` | u8 | edit mode flag (`SetEditMode` arg) |
| `+0xAD` | u8 | blink flag (toggles every 500 ms of accumulated `+0xB0` time while editing) |
| `+0xB0` | u32 | accumulated tick milliseconds |
| `+0xB4`, `+0xB5` | 2 × u8 | model-dirty flags with change counters (RebuildModel clears 0xFD / 0xFB bits, sets bit 2) |
| `+0xB8…+0x118` | 4 × 3×3 f32 | four orientation matrices (identity when not loaded) |
| `+0x11C…+0x134`, `+0x138…+0x150` | 2 × 6 × f32 | per-orientation corner positions (FUN_005F4D10 fills them) |
| `+0x194` | 16-byte | message target sub-object |
| `+0x4BC` | `cRZString*` | owned string (freed in the destructor) |
| `+0x4C` | ptr | owned date-pair record (freed by `FUN_005EE160`) |
| `+0x4D4…+0x507` | 4 rows × 4 bytes | corner colours copied from the static tables at `0x00AA5204` (row 0 = static colours, row 1 = dynamic) |
| `+0x54C…+0x570` | 10 × ptr | the ten piece textures, one per entry of the RID table `0x00AA5214…0x00AA523C` (instances `0x8B4A6560`–`0x8B4A6565`, `0x8B4A6564`, `0x8B4A6565`, `0x8B4A6566`, `0x8B4A6567`; keys `0x7AB50E44` / `0x1ABE787D`); `Init` fills them through the `+0x0C` view (`+0x540` there) |
| `+0x574…+0x57C` | vector | the text textures `LoadTextures` makes (§10.3 below); `Shutdown` empties it through the `+0x0C` view (`+0x568` there). The element name in the export comes from a folded template (`0x00527180`) |
| `+0x580` | | packed render state (`cSC4PackedRenderState::PackRenderState` `0x007D2B50`; `Init`'s `+0x574` is the `+0x0C` view's, `0x005F744F`) |

Interfaces (sub-object map): `+0x00` the 8-slot class view `0x00AA5478` (`cSC4SignpostOccupant`: QI `0x005F4930`
answers `0x4B44FBE2` with this, otherwise `ADD ECX,4` and the prop QI; slot 3 `AsOccupant` `0x005F8400` returns
this+0xC; slot 4 `SetText` `0x005F7600`; slot 5 `GetText` `0x005F5920`; slot 6 `SetDynamic` `0x005F4BA0`; slot 7
`SetEditMode` `0x005F7320`); `+0x04` the prop's `cISC4PropOccupant` view (24 slots, `0x00AA5418`, QI-4 `0x005F5950`);
`+0x08` `cIGZMessageTarget2` (QI-8 `0x005F5960`, `DoMessage` `0x005F7510`); `+0x0C` the 34-slot `cISC4Occupant`
view `0x00AA5380` (QI-12 `0x005F5970`; overrides `Init` `0x005F73A0`, `Shutdown` `0x005F5E20`, `GetPosition`
`0x005ED1F0` = box centre from the prop box at object +0x40…+0x54, `SetPosition` `0x005F4E90`, slots 27/28 =
`cSC4PropOccupant::GetLotTag`/`SetLotTag` `0x005ECAC0`/`0x004302C0`, `SetAllFlags` `0x005F4A90`, everything else
the base); `+0x10` `cIGZSerializable` `0x00AA5364` (Write `0x005F5080`, Read `0x005F50F0`, `GetGZCLSID`
`0x005F5940`); `+0x14` the 26-slot `cSCExemplarPropertyHolder` view `0x00AA5300` (slots 3–21 shared holder
implementations; 22 `0x005F59A0` and 23/24 `0x005F5AC0`/`0x005F5AD0` are `SUB ECX,4` thunks into Write/Read);
`+0x28` `cISCExemplarPropertyHolder` `0x00AA52E8`; `+0x6C` a `cSCExemplarPropertyHolder`-base view `0x00AA52E4`
(`SUB ECX,0x6C` thunks; the holder base at prop-relative +0x68).

Behaviour:

- **Creation** — GZCOM creator `0x005F5CE0` allocates from pool `0x00B0A96C` and calls the constructor with
  argument 1. The sign tool (`cSC4ViewInputControlSignTool::OnMouseDownL` `0x004D81D0`, call at `0x004D82D9`)
  creates it with argument 1, takes a reference and selects it for editing. Registered in the COM director
  `0x00460090` at `0x0046083F` with `GetGZCLSID` `0x005F5940`.
- **`Init`** `0x005F73A0` — if already initialised return true. When not loaded (status bit 1 clear): zero and
  identity-fill the four matrices (`+0xE0…+0x118`, the three at `+0x40…+0x48` too), set the prop type to
  `0xAB72FBB3` (prop-view slot 8 `SetPropType` `0x005EF4C0`, which interns the ID in the process-wide table and
  writes the index — the saved record keeps the ID), set flag 0x10 (occupant view slot 30 `SetAllFlags`). Then the
  prop's `Init` (`0x005EC940`), `PackRenderState(+0x574; args 1,1,1,0,1,1,1,0,3,4,5,-1,1.0f,1,-1)`, texture
  loading, three message subscriptions (`0xA6B79602`, `0x06CDB65B`, then a third via slot 0x14), and
  `RebuildModel`. Returns 1.
- **`LoadTextures`** `0x005F5EF0` *(rewritten 2026-10-22, re-signposts)* — empties the text-texture vector (object
  `+0x574`); when the text (`+0x98`) is not empty, the text texture service (`0x00B43CE4`, its slot 4) renders it with
  font `0x2C1FD612` into that vector *[inferred: one texture per piece of text]* (`0x005F5EF7`–`0x005F5F74`). The ten
  piece textures are `Init`'s, not this function's: `Init` asks the resource service (`0x00B43CD4`) slot 6 for
  (`0x7AB50E44`, `0x1ABE787D`, RID) for each entry of `0x00AA5214…0x00AA523C` (`0x005F745A`–`0x005F7465`).
- **`Shutdown`** `0x005F5E20` unsubscribes, releases the ten bindings, empties the text-texture vector (object
  `+0x574`) and, when byte `+0xA0` says a model instance exists, unregisters from the animation tick manager (slot
  0x18), then frees the date pair and calls the base `Shutdown` (tail-jump).
- **`SetText(text)`** `0x005F7600` — compares against the current text (`cRZString` slot 0x30 `CompareTo(arg, 1)`);
  if equal **and** `+0x9C == +0xA0` the call is skipped; otherwise copies (slot 0x34) and calls `LoadTextures` and
  `RebuildModel`. `GetText` `0x005F5920` tail-jumps the string's slot 0x34 copy into the caller's string.
- **`SetDynamic(bool)`** `0x005F4BA0` — reads the current flag through `FUN_008018E0(&+0x9C)` (returns `+0x80` of
  the object at `+0x9C`, i.e. the model instance's byte); no change → return. Sets the occupant view's flag 2 test
  (`IsFlagSet(2)`, slot 32) result aside, drives the model instance (`+0x70` view slots 0x34/0x38) with the new
  state, copies the four-byte colour row `0x00AA5204 + 4·(arg != 0)` into the four corner rows `+0x4D4…+0x4D7`,
  `+0x4E4…+0x4E7`, `+0x4F4…+0x4F7`, `+0x504…+0x507`: row 0 (static) is (144, 172, 190, 255), row 1 (dynamic)
  (255, 255, 255, 204), in memory order *[inferred: R, G, B, A]* (`0x00AA5204` = `90 AC BE FF`, `FF FF FF CC`; OP3
  answered 2026-10-22, re-signposts), and — only when
  the object is in edit mode (`+0xAC`) and the argument is false — calls the class-view slot 7 (`SetEditMode`, via
  `[EAX + 0x1C]` with 0).
- **`SetEditMode(bool)`** `0x005F7320` — no change → return. Stores `+0xAC`. Turning on: class-view slot 6 with 1
  (QI-4 helper `[EAX + 0x18]`), registers with the animation tick manager (slot 0x14, tag `0x80`, target = the
  message target at `+0x8`), clears `+0xB0`, sets `+0xAD` = 1, `RebuildModel`. Turning off: unregister (slot 0x18),
  slot 6 with 0, `RebuildModel`.
- **`DoMessage`** `0x005F7510` — `0x06CDB65B` (visibility test): reads the message's void (slot 0x3C), QIs it
  against `0x4B44FBE2` through slot 0xC; when it answers and the city config (`0x00B43DB8`) slot 0x28 forces the
  hidden state, clears flag 2 (`ClearFlag(2)` via view slot 0x7C) unless already clear; then the prop's
  `DoMessage`. `0x86AD10EE` (time tick): takes data3 (slot 0x38) as milliseconds (÷1000 via the `0x10624DD3`/SHR 6
  pair) and calls `FUN_005F4960(this, ms)`: `+0xB0 += ms`; when `+0xB0 >= 500` (`0x1F4`) toggle `+0xAD` (SETZ of
  the inverted byte) and reset `+0xB0` — the blink. Then the prop's `DoMessage`. `0xA6B79602`: `RebuildModel`.
  Default: the prop's `DoMessage`.
- **`SetAllFlags`** `0x005F4A90` — the same flag-2 protection as the decal (§10.4): when the city config slot 0x28
  forces hidden, bit 2 is stripped from the new value before the base `SetAllFlags`. With bit 2 turning on it
  registers the model with the renderer (`0x00B43DD0` slot 0x6C test, slot 0x70 add; box pointer from the per-
  instance table at `+0x64`) — OP7.
- **`Draw`** `0x005F5200` — visual only (renderer thunk `__ZThn108_N20cSC4SignpostOccupant4DrawEP14SC4DrawContextb`
  `0x005F5CD0` adjusts `this` by −108 into the +0x6C sub-object). `RebuildModel` `0x005F5FB0` (4,964 bytes) builds
  the sign geometry from the text and matrices; it is not specified further here.

### 10.4 `cSC4TextDecalOccupant` (`0x8B79C707`) *[compat]*

The city-detail terrain text label embeds a prop at complete-object +4. Its corrected
state, record and owned behavior contract is [text-decal-occupant.md](text-decal-occupant.md),
with independent **UNEXECUTED** [vectors](text-decal-vectors.md).

**OP1 correction (2026-10-23):** occupant methods use O+12, serializable methods O+16,
constructor and internal rebuild O. Record +5C is O+6C text (GZStr); repeated +84/+88 is
O+94/+98 position (Float32), not IDs or repeated height. Complete-object +84/+88 are IDs;
Shutdown's +A8/+AC are O+B4/+B8 managers, while SetOrientation's +AC is O+AC orientation.
The former mixed-frame tables and geometry summary are withdrawn, not alternative behavior.
TextDecal-only OP2 is resolved by compiled stream-width/slot facts in the dedicated spec §7;
the signpost part of OP2 remains separate.

#### Contributor interface supplement (PR10, harvested 2026-10-23)

The dedicated TextDecal spec above remains authoritative for numeric comparisons,
geometry, resource ownership and persistence. The contributor independently agrees
on complete-object field roles and the version-3 record; its duplicate summaries
are superseded by that newer contract. The signpost portion of OP2 is now resolved
by §10.3: its serializable receiver is O+0x10 and stream slot 15 receives O+0x98 text.

| Complete-object view | Vtable | Interface facts |
|---|---|---|
| +0x00 | `0x00AA5680` | 11 slots; QI `0x005F7650` answers `0x2B3B7D86`; AsOccupant returns O+0x0C |
| +0x04 | `0x00AA5620` | embedded prop |
| +0x08 | `0x00AA5610` | message target; DoMessage `0x005F8030` |
| +0x0C | `0x00AA5588` | 34-slot occupant; Init `0x005F7F40`, Shutdown `0x005F76C0`, SetPosition `0x005F7AA0`, SetAllFlags `0x005F81F0` |
| +0x10 | `0x00AA556C` | serializable Read/Write |
| +0x14 | `0x00AA5508` | 26-slot holder; slots 23/24 Write/Read |
| +0x28 | `0x00AA54F0` | exemplar property holder |

Creator `0x005F84D0` uses pool `0x00B0A9B8`; registration is at `0x00460851`.
These are interface/provenance facts, not an alternative TextDecal behavior contract.

### 10.5 `cSC4PropOccupantTerrainDecal` (`0x2977AA48`) *[compat]*

**SR36-1, re-sr37 (2026-10-28).** Class-store key `(0x2977AA48, 0x299B2D1B, 0)`.
The following is the object payload, after the class entry's size, CRC and object ID
(foundations §3.3). Every primitive is little-endian; there is no alignment or trailing padding.

| Order | Field | Encoding | Rule |
|---|---|---|---|
| 1 | decal record version | u16 | Write emits 3; Read rejects every value other than 3 before reading the base |
| 2 | complete embedded prop record | §10.1 | Starts with its own u16 version (Write emits 6), then occupant v4 and its exemplar holder; includes every prop field through timer bits when present |
| 3 | decal extension | 47 bytes | The table below, immediately after the embedded prop |

For the next table, **tail offset** starts at #3. **O** is the complete terrain-decal object,
whose terrain-decal interface is at O+0, embedded prop at O+4 and serializable receiver at O+0x10
(constructor `0x005EE365`–`0x005EE398`). These offsets are explanatory; they add no bytes to the stream.

| Tail offset | Field | Encoding | In-memory destination / meaning |
|---|---|---|---|
| 0 | decal ID | u32 | O+0x6C; terrain renderer's decal handle, persisted as its raw value, **not** an object reference |
| 4 | texture resource key | 3 × u32 | O+0x70…0x78; **type, group, instance** order (stream-helpers §2 `key3`) |
| 16 | decal size | f32 | O+0x7C; `SetDecalTexture` / `SetDecalSize` size |
| 20 | horizontal position | 2 × f32 | O+0x80, O+0x84; world x, z, not x, y; vertical bounds remain in the embedded prop box |
| 28 | opacity | f32 | O+0x88; current opacity sent to the terrain renderer |
| 32 | fade accumulator | f32 | O+0x8C; initial/current value advanced during a fade |
| 36 | fade endpoint | f32 | O+0x90 |
| 40 | fade rate | f32 | O+0x94 |
| 44 | fade completion mode | s8 | O+0x98; stored through Sint8, not a Boolean |
| 45 | held-reference flag | u8 | O+0x99; saved as a literal byte, without Boolean normalization |
| 46 | animation-tick registration flag | u8 | O+0x9A; saved as a literal byte, without Boolean normalization |

The width/order evidence is Write `0x005ED731`–`0x005ED7C0` and Read
`0x005EF519`–`0x005EF5AA`: Uint32 for the ID, `key3` helpers `0x005DA0A0` / `0x005DA070`,
Float32 for the size and four opacity/fade values, `vec2` helpers `0x005DA520` / `0x005DA4F0`,
then Sint8, Uint8, Uint8. No Float64, count prefix, nested version or reference framing belongs
to this extension. `SetPosition` copies the source x and z into its two position destinations
(`0x005EE6DA`–`0x005EE6E2`, occupant receiver O+0x0C). `SetOpacityFade`
(`0x005ECBA7`–`0x005ECBC0`) identifies the accumulator, endpoint, rate and mode.

**Version and return rules.** The outer comparison is equality with 3 (`0x005EF4F8`);
there is no outer v1/v2 legacy body or future-version acceptance. The embedded prop reader
does not reject any u16 version: versions 0…5 omit its final timer byte and set timer bits to
`0x0F`; versions 6…65535 read that byte (`0x005EF479`–`0x005EF4A0`). All its other fields,
including the optional 16-byte date range, retain §10.1's order. The extension is unchanged
by that inner version. The occupant base still requires v4 and its holder has the independent
version rules in foundations §10.2–§10.3.

Write calls the prop writer and writes the extension only when that call returns true, then
returns `GetError() == 0`. Read returns false immediately on an outer mismatch or a false
prop return; otherwise it reads the whole extension and returns `GetError() == 0`
(`0x005EF511`–`0x005EF5C2`). These operations load/store the fields directly; Read does not
call the decal setters or rebuild renderer state. The inherited base/prop false-return quirks
in §2.3/§10.1 still apply; an outer reader does not independently validate the holder version.

**Corpus proof / worked size.** The 749 reference `golden/<key>/end.sc4` files available to
re-sr37 contained 44 class records, **4,683 decal objects**, 641,571 decompressed class-record
bytes (585,375 payload bytes). A field-driven parser consumed every object and re-encoded
every primitive exactly, with zero remainder. Every object had versions outer 3 / prop 6 /
occupant 4 / exemplar and plain holder 2; no own properties or date range. Thus each embedded
prop occupied 76 bytes, each payload `2 + 76 + 47 = 125` bytes, and each class entry 137 bytes.
Flags `(mode, held-reference, tick-registered)` were `(0,0,0)` for 75 objects and `(1,0,1)`
for 4,608. Legacy inner versions are supported by the instructions, not observed corpus cases.

### 10.6 `cSC4PropOccupantBurntBuilding` (`0xC9EFB016`) *[compat]*

**SR36-3, re-sr37 (2026-10-28).** Class-store key `(0xC9EFB016, 0x299B2D1B, 0)`.

| Order | Field | Encoding | Rule |
|---|---|---|---|
| 1 | burnt-building record version | u16 | Write emits 3; Read accepts exactly 3 |
| 2 | complete embedded prop record | §10.1 | Its own version, occupant base and holder, and all prop fields, including optional date range and version-dependent timer byte |
| 3 | source-occupant bounds | box (24 bytes) | Six f32: min x, y, z, then max x, y, z; independent of the prop box already in #2 |

There are **no other trailing fields**, flags, references, counts or padding. Write
`0x005EDB1A`–`0x005EDB35` emits the version, calls the prop writer and, if it succeeds,
uses the six-Float32 box writer `0x005DA600` on serializable-receiver +0x94.
Read `0x005EFBB4`–`0x005EFBD7` rejects every outer version except 3, calls the prop reader,
then, if that succeeds, uses the corresponding box reader `0x005DA5B0` on the same field.
This is complete-object +0xA0…+0xB4 (serializable receiver O+0x0C). `SetBoundsFromOccupant`
copies the source occupant's box there (`0x005EEB86`–`0x005EEBB6`), explaining why this
second box must survive even when it equals the prop's box.

Both methods finish with `GetError() == 0`; unlike the terrain-decal reader, a false embedded
prop return skips the tail but still reaches that final stream-error test (`0x005EFBCB`–
`0x005EFBE6`). There is no rollback. No rebuild or setter is called by Read. The legacy
variant is solely the embedded prop's timer gate: prop versions ≤5 have no timer byte and
default it to `0x0F`; versions >5 include it. There is no supported outer v1/v2 variant.

**Corpus proof / worked size.** In the same 749 end saves, 38 class records contained
**75 burnt-building objects**, 8,550 decompressed class-record bytes (7,650 payload bytes).
Every field was consumed and re-encoded exactly, with zero remainder. Versions were outer
3 / prop 6 / occupant 4 / both holders 2, with no own properties or date range. The payload
was always `2 + 76 + 24 = 102` bytes, entry size 114 bytes. These measurements establish
the current layout; the inner legacy timer gate is instruction evidence only.

---

## 11. Flora occupant (`cSC4FloraOccupant`, `0xA9C05C85`) *[compat]*

Behaviour in `flora.md` §3.4 and §6. Record:

| # | Field | Type | Meaning, observed (744,617 in 568 saves) |
|---|---|---|---|
| 1 | version | u16 | 3 (`Read`: anything else → false) |
| 2 | occupant base | §2.3 | type `0x74758926`, flags 13 |
| 3 | flora type | u32 | the static flora record key (`flora.md` §3.2) |
| 4 | position | vec3 | |
| 5 | birth date | day | 2,451,545 … 2,462,350 |
| 6 | last seeding date | day | 2,451,545 … 2,466,676 |
| 7 | state | s8 | 0 (735,026), 1 (4,114), 2 (5,377), 3 (100) |
| 8 | orientation | u8 | 0…3 |
| 9 | lot tag | u32 | tag of the configuration object that planted it on a lot; 0 for wild flora (431,076) |

If the base `Read` fails, `Read` returns `GetError() = 0`. The static record pointer is not saved; it is looked up
from #3 *[inferred: on Init]*.

---

## 12. Bare occupant (`cSC4Occupant`, `0xA9BC9AB6`)

84 records (12 saves) consist of only the base block (§2.3): type `0x74758925`, holder parent `(0, 0, 0)`, no
properties. They are referenced only by the occupant manager; they are the dummy occupants of automata
*[inferred]* (`save-records.md`).

The class itself (behaviour, layout and the 34-slot `cISC4Occupant` interface) is specified in
occupant-manager.md §19. The construction signpost and text-decal classes above embed this base; the prop classes
do too (inside the prop). A replacement implements the base once and derives every occupant from it.

---

## 13. Interfaces *[compat]*

### 13.1 `cIGZSerializable` of each class

Every class above has a 6-slot `cIGZSerializable` view at the vtable listed in §1: slots 0–2 QueryInterface (pop 8),
AddRef, Release (pop 0); slot 3 `Write(cIGZOStream&)`; slot 4 `Read(cIGZIStream&)`; slot 5 `GetGZCLSID()` (EAX =
class ID). All 24 Write / Read functions were checked with `abi_extract --check`: `__thiscall`, callee pops 4.
Return channels from the epilogues:

| Class | Write returns | Read returns |
|---|---|---|
| lot `0x006C0080` / `0x006C35F0` | **AL** (`sbb al,al`; failure path `xor al,al`) | **AL** (version > 8: `xor al,al`) |
| business unit `0x006C0370` / `0x006C09E0` | **AL** | **AL** |
| occupant base, building, base texture, wall, foundation, flora, prop, effect prop, construction | EAX = 0/1 | EAX = 0/1 on the normal path; **AL** = 0 on the version-reject path |
| signpost `0x005F5080` / `0x005F50F0`, text decal `0x005F7DB0` / `0x005F7E50` | EAX = 0/1 (`NEG AL; SBB EAX,EAX; INC EAX`) | same epilogue form |
| `cSC4TripData` `0x0070E0B0` / `0x0070E160` (non-virtual, `this` = the trip) | EAX = 0/1 | EAX = 0/1 |

A replacement returns a clean 0/1 in EAX; callers in the original test AL only (`bool`).

### 13.2 Complete live-lot interface

This is the Windows 1.1.641 contract, including entries unused by the shipped game. Slot numbers are zero-based;
offsets are bytes **within the selected vtable**, not offsets in the lot record. There are **107 entries** across
four views: 70 primary, 6 Serializable, 25 property holder and 6 traffic source. Field names are those of
§§3.2–3.6. No interface to a business unit, building or other occupant is implemented by the lot itself.

All entries use `__thiscall`: ECX is the selected view pointer; explicit arguments are stack arguments in the
listed order. `Pop` is the callee's stack cleanup in bytes. Each scalar argument occupies four stack bytes,
including a bool, u8 or u16; a narrow argument uses its low byte/word. References and output arguments are 32-bit
pointers. Output storage must be valid unless a null case is explicitly specified. A **void** entry has no
defined return value: incidental EAX/AL is not success, and it supplies no ST0 result. AL and AX results leave
the higher bits of EAX unspecified unless the row promises EAX. ST0 is an x87 floating result; an out-f32 entry
is not an ST0 entry. `GetLandValue` exceptionally uses **EDX:EAX**, high:low.

Borrowed pointers do not AddRef and must not be Released by the getter's caller. Their lifetime is bounded by
the owner and by replacement/removal of the referenced item. An owned output carries one reference for the
caller to Release. Setter inputs are borrowed during the call; retained inputs are identified explicitly.
No getter makes a snapshot unless stated. Except for the writes in its row and the detailed rules below,
an entry does not change saved lot fields. Runtime caches are separate from saved fields.

**Views and identity.** The primary pointer also serves IID `0x00000001` (`cIGZUnknown`) and lot IID
`0xE818FCED`. Serializable IID `0xE4FDA3D4` returns the +4 view. These are the **only three successful QI
requests**, from any of the four views (§3.1). The property-holder (+8) and traffic-source (+0x1C) views are
obtained by primary slots 3/4; their existence does not add accepted IIDs. All views share one lifetime and
reference count. A failed QI leaves its destination unchanged. Do not clear it on failure or supply an
independent reference count/identity on a secondary view.

Caller labels: **Z** = zone developer / lot manager and city lifecycle; **D** = lot developer, construction,
terrain/foundation updates and demolition; **B** = building and tract development, including population/wealth
decisions; **T** = traffic simulation and its source iterator; **Q** = city queries, query UI and lot configuration
editor; **S** = class store and save/load; **U** = no native live-lot caller identified in the shipped executable's
audited paths (unused on that surface; the entry must still exist). A zero direct-call count is not evidence
of U: these are virtual calls. Shared targets called on other classes do not establish a lot caller. Lists
identify dependent subsystems, not a scheduling guarantee.

#### 13.2.1 Primary `cISC4Lot`, vtable `0x00AB19B0`

These are the spec's names. Exported names can belong to unrelated classes folded to the same instructions;
slots 9, 18, 19, 25 and 59 are not Lua, grid, aura or socket methods.

| Slot / offset | Name; target | Explicit arguments; Pop | Return channel and value / ownership | Record or runtime effects; dependent subsystems |
|---|---|---|---|---|
| 0 / +0x00 | QueryInterface; `0x006BF190` | IID: u32, out: interface**; 8 | AL 1 with one owned reference and selected pointer, else AL 0 with out unchanged | AddRef on success only; Z, D, B, T, Q, S |
| 1 / +0x04 | AddRef; `0x004664A0` | none; 0 | EAX = incremented u32 reference count | Increment shared count; all owned-reference holders |
| 2 / +0x08 | Release; `0x006BF460` | none; 0 | EAX = decremented count if old count >1, otherwise 0 | Old count ≤1 calls Shutdown then property-view deleting destructor with flag 1; object ceases to exist; all reference owners |
| 3 / +0x0C | AsPropertyHolder; `0x006C2040` | none; 0 | EAX = borrowed +8 view; null for null primary | No AddRef or record write; D, B, T, Q, S |
| 4 / +0x10 | AsTrafficSource; `0x006C2050` | none; 0 | EAX = borrowed +0x1C view; null for null primary | No AddRef; T (iterator separately retains the lot) |
| 5 / +0x14 | Init; `0x006BF490` | none; 0 | AL 1 | §3.5: new-lot altitude/slopes/travel jobs, strategy byte if 0xFF, resolve/retain configuration, clear unresolved ID; Z |
| 6 / +0x18 | IsInitialized; `0x006C2020` | none; 0 | AL = runtime initialized byte, normally 0/1 | Read only; D (demolition guard), Z |
| 7 / +0x1C | Shutdown; `0x006C3060` | none; 0 | AL 1, including already shut down | §3.5: clear initialized, population vector, travel-population map; release/null configuration and travel designate; remove from business unit and null unit; Z, final Release |
| 8 / +0x20 | PostCityInit; `0x006BF200` | none; 0 | void | Save lot flags locally, clear flags, reapply through slot 13; restores terrain levelling on load; Z |
| 9 / +0x24 | GetLotConfiguration; `0x00A6CAB0` | none; 0 | EAX = configuration interface, borrowed, or null | Resolved object, not unresolved configuration ID; D, B, Q, S |
| 10 / +0x28 | SetLotConfiguration; `0x0060CB80` | configuration: cISC4LotConfiguration* or null; 4 | void | If changed: AddRef new, replace configuration object, Release old; does not change unresolved ID; D, B, Q |
| 11 / +0x2C | GetLotConfigurationObject; `0x006BFEA0` | occupant: cISC4Occupant*; 4 | EAX = matching configuration-object record*, borrowed, or null | Position match described below; no record writes; D (clear lot / preserve objects), Q |
| 12 / +0x30 | GetFlags; `0x006C2030` | none; 0 | EAX = zero-extended lot flags byte | Read only; D, T, Q, S |
| 13 / +0x34 | SetFlags; `0x006BF220` | flags: u32, low byte stored; 4 | void, not a bool | Replace lot flags; bit-0 transition levels/unlevels inclusive cells, §3.5; D, Z |
| 14 / +0x38 | GetBoundingRect; `0x006BF320` | out: CellRect32*; 4 | out = four zero-extended u32 values x1,z1,x2,z2; EAX also echoes out, no ownership | No record writes; D, B, T, Q, Z |
| 15 / +0x3C | GetCenter; `0x006BF560` | out: vec3*; 4 | out = three f32 metres x,y,z; EAX also echoes out, no ownership; no ST0 result | Formula/example §3.5; no record writes; D, Q, new-lot Init |
| 16 / +0x40 | GetAltitude; `0x006C2060` | none; 0 | ST0 = stored altitude f32, loaded exactly | Read only; D, Q |
| 17 / +0x44 | SetAltitude; `0x006BF5C0` | altitude: f32; 4 | void | If absolute difference > double 0.001: store altitude and both slopes; equal/unordered does nothing, §3.5; D, Init |
| 18 / +0x48 | GetCurrentSlope; `0x006C2070` | none; 0 | ST0 = current slope f32 | Read only; D (levelling/foundation tests) |
| 19 / +0x4C | GetSlopeAtAltitudeChange; `0x006C2080` | none; 0 | ST0 = slope at altitude change f32 | Read only; D (foundation/condition decisions) |
| 20 / +0x50 | UpdateSlope; `0x006BF610` | none; 0 | void | Recompute current slope only through lot developer GetLotSlope(lot,false), rounded to f32; D |
| 21 / +0x54 | GetLocation; `0x006BF2E0` | outX: u32*, outZ: u32*; 8 | outputs = zero-extended location bytes | Writes four bytes per output; D, B, T, Q, Z |
| 22 / +0x58 | GetSize; `0x006BF300` | outSizeX: u8*, outSizeZ: u8*; 8 | outputs = saved size bytes | Writes one byte per output, not s32/u32; D, B, Q |
| 23 / +0x5C | GetFacing; `0x006C2090` | none; 0 | EAX = zero-extended facing byte | Read only; D, B, Q |
| 24 / +0x60 | SetFacing; `0x006C20A0` | facing: u32; 4 | void | Store low byte as facing; no rotation of rect, location, size, occupants or configuration; D, Q |
| 25 / +0x64 | GetUnit; `0x0068D1C0` | none; 0 | EAX = business unit*, borrowed, or null | Read only; D, B, unit management |
| 26 / +0x68 | SetUnit; `0x006C20B0` | unit: cISC4BusinessUnit* or null; 4 | void | If different: Release old, store new, AddRef new if nonnull; no call to unit Add/Remove; unit management, D |
| 27 / +0x6C | HasJobAccess; `0x006BFCE0` | none; 0 | AL 1 if traffic GetConnectedDestinationCount(lot,0) or then (lot,1) >0, else 0 | Short-circuit after first positive u32 result; no record writes; D (residential travel test) |
| 28 / +0x70 | HasRoadAccess; `0x006C20E0` | none; 0 | AL = slot 29 with purpose 0 | May fill runtime road cache; D, B, T |
| 29 / +0x74 | HasOrganicRoadAccess; `0x006C1D10` | purpose: u32 building-purpose enum; 4 | AL = direct road access or same-purpose neighbouring-lot access, 0/1 | Lazy per-purpose runtime cache, detailed rules below; D, B |
| 30 / +0x78 | UpdateRoadAccess; `0x006C1DA0` | none; 0 | void | Refresh direct-road cache and every already cached nonzero purpose; does not populate every possible purpose; D, network-change processing |
| 31 / +0x7C | GetZoneType; `0x006C20F0` | none; 0 | EAX = zero-extended zone type byte | Read only; D, B, T, Q, Z |
| 32 / +0x80 | UpdateZoneType; `0x006BFC40` | none; 0 | void | Replace zone type with zone developer's zone at saved location, not center; Z |
| 33 / +0x84 | GetState; `0x006C2100` | none; 0 | EAX = zero-extended state byte | Read only; D, B, T, Q |
| 34 / +0x88 | SetState; `0x006C3100` | state: u32, wealth: u32, reason: u32; 12 | void; incidental AL is not success | Branches, population/resources, day, wealth, vacate reason, needs-power flag and queued snapshot in §3.2.2; D, B, Z |
| 35 / +0x8C | GetCondition; `0x006C2110` | none; 0 | EAX = zero-extended condition byte | Read only; D, B, Q |
| 36 / +0x90 | SetCondition; `0x006C26D0` | condition: u32; 4 | void | Full u32 comparison with old byte; on difference store low byte, reset day if state 4, queue condition-change snapshot; detailed rule below; D |
| 37 / +0x94 | IsInLivableState; `0x006C2150` | none; 0 | EAX 1 iff state=3, else 0 | Read only; D, B, T |
| 38 / +0x98 | HasCapacity; `0x006C2160` | none; 0 | EAX 1 iff state is 2 or 3, else 0 | State test only, not capacity-map search; B |
| 39 / +0x9C | IsInDemolishableState; `0x006C2180` | none; 0 | EAX 1 iff state ∈ {2,3,4,6}, else 0 | Read only; D, demolition UI |
| 40 / +0xA0 | GetDaysSinceLastStateChange; `0x006BF630` | none; 0 | EAX = current city day minus saved day, wrapping u32; 0 if saved day=0 | Read only; D, B |
| 41 / +0xA4 | GetOccupantWealth; `0x006C21F0` | none; 0 | EAX = zero-extended occupant wealth byte | Read only; D, B, Q |
| 42 / +0xA8 | GetLandValue; `0x006BFC60` | none; 0 | EDX:EAX = nonnegative 64-bit integer sum of cell land-value bytes; not ST0 | No saved land-value field or averaging; U (monthly consumers use the grid directly; worked case below) |
| 43 / +0xAC | IsHistorical; `0x006C2220` | none; 0 | AL = (lot flags >>5) &1 | Read only; D, B, Q |
| 44 / +0xB0 | SetHistorical; `0x006C2230` | historical: bool; 4 | void | Set/clear lot flags bit 5; no message or terrain work; Q, legacy Read |
| 45 / +0xB4 | GetVacateReason; `0x006C2250` | none; 0 | EAX = zero-extended vacate reason byte | Read only, including stale non-vacant value (§3.2); Q |
| 46 / +0xB8 | GetBuildingType; `0x006BFA00` | allowConfigurationFallback: bool; 4 | EAX = u32 type, possibly 0 | Stored building type except: fallback true, stored type 0, configuration nonnull and state ≠6 → configuration GetID; does not cache fallback; D, B, Q, capacity/occupancy/jobs methods |
| 47 / +0xBC | SetBuildingType; `0x006C2200` | type: u32; 4 | void | Store building type and clear occupant wealth to 0, even when type unchanged; no state/day/resource/power update; D, B |
| 48 / +0xC0 | GetBuilding; `0x006BFA30` | none; 0 | EAX = cISC4BuildingOccupant*, borrowed, or null | Stored type 0 → null; otherwise occupant-manager rect search and QI, balancing both temporary references before return; no occupant cache; D, B, Q, road/jobs/power helpers |
| 49 / +0xC4 | GetLotOccupants; `0x006BFF70` | out: occupant-list*; 4 | Output list populated by occupant manager; no scalar result | Inside-lot filter over inclusive rect; counted occupant references in caller's list; no record writes; D, Q |
| 50 / +0xC8 | GetCapacityByDeveloperType; `0x006C05A0` | devType: u32, ignoreState: bool; 8 | AX = u16 capacity of first matching profile map, else 0 | With ignoreState false allow states 2/3 only; true bypasses guard; profile from slot 46(true); B, population setup |
| 51 / +0xCC | GetCapacityByWealth; `0x006BF9B0` | wealth: u32; 4 | AX = u16 capacity, else 0 | Slot 46(true); profile purpose + argument wealth → developer type; slot 50(type,false). Not a no-argument total; Q / capacity clients |
| 52 / +0xD0 | GetAvailableCapacity; `0x006C0640` | devType: u32, includeVacant: bool; 8 | AX = u16 maximum allowed population for this type, else 0; not capacity minus its current population | States 3 only, or 3/4 if includeVacant; first matching map, §3.2.1 arithmetic; no population insertion; D, B, SetPopulation |
| 53 / +0xD4 | GetPopulation; `0x006C04A0` | devType: u32; 4 | AX = sum of matching counts across population maps, modulo 65536 | Missing keys contribute zero without insertion; B, Q, jobs calculation |
| 54 / +0xD8 | GetPopulations; `0x006C21E0` | none; 0 | EAX = pointer to live population-map vector, borrowed, never null on a valid lot | No copy/AddRef; S, B, travel-population accumulation |
| 55 / +0xDC | SetPopulation; `0x006C1940` | devType: u32, population: u16; 8 | AL 1 iff capacity map contains devType, otherwise 0 | Clamp to slot 52(type,false), assign first corresponding population map, recompute wealth; may insert further zero keys, §§3.2.1–3.2.2; D, B |
| 56 / +0xE0 | GetTravelPopulation; `0x006C0510` | devType: u32; 4 | AX = travel-population map value, or 0 | No insertion or travel-designate forwarding; T |
| 57 / +0xE4 | ResetTravelPopulation; `0x006C0550` | none; 0 | void | Clear travel-population map, zero travel jobs, then synchronize traffic capacities in properties (UpdateJobs, below); D / travel aggregation |
| 58 / +0xE8 | AddTravelPopulation; `0x006C0BA0` | source: cISC4Lot*; 4 | void | Add every source population-map entry into travel-population map modulo 65536; add source GetJobs outputs to travel jobs with f32 stores; synchronize capacity property; source not retained; D |
| 59 / +0xEC | GetTravelDesignate; `0x0081F910` | none; 0 | EAX = cISC4Lot*, borrowed, or null | Read only; D, T, business-unit travel aggregation |
| 60 / +0xF0 | SetTravelDesignate; `0x006BF660` | designate: cISC4Lot* or null; 4 | void | If changed: AddRef new, store, Release old; no population/trip forwarding or aggregation by this setter; D |
| 61 / +0xF4 | GetOccupancy; `0x006C0CF0` | out: f32*; 4 | out f32, default 0; EAX also echoes out, no ownership; no ST0 result | Weighted occupancy below; can insert missing zero population keys; D (condition test), Q |
| 62 / +0xF8 | GetOccupancyByType; `0x006C10F0` | out: ordered-map<u32,f32>*; 4 | Clear/fill caller's map; void | Formula/edge cases §3.2.2; inserts missing zero keys in population maps; wealth recomputation, B |
| 63 / +0xFC | GetTravelJobs; `0x006BF350` | wealth: u32; 4 | ST0 = travel jobs[wealth−1] for wealth 1…3; +0.0 otherwise | No designate forwarding or recomputation; D, T |
| 64 / +0x100 | GetJobs; `0x006BF800` | out: Jobs16*; 4 | Three f32 outputs at out+4,+8,+0xC; void, no ST0 result | Leading dword untouched; profile/population or building-exemplar calculation below; D, travel aggregation |
| 65 / +0x104 | GetNeedsPower; `0x006C21A0` | none; 0 | AL = (lot flags >>3) &1 | Read cached flag, not fresh power-demand calculation; D, power updates |
| 66 / +0x108 | GetHasPower; `0x006C21B0` | none; 0 | AL = (lot flags >>4) &1 | Read only; D, power/night-light processing |
| 67 / +0x10C | SetHasPower; `0x006C21C0` | hasPower: bool; 4 | void | Set/clear lot flags bit 4 only; no message, needs-power recomputation or SetFlags call; D, legacy Read |
| 68 / +0x110 | GetVerticesAreLocked; `0x006C2120` | none; 0 | AL = (lot flags >>6) &1 | Read only; terrain/lot management |
| 69 / +0x114 | SetVerticesAreLocked; `0x006C2130` | locked: bool; 4 | void | Set/clear lot flags bit 6 only; no terrain-lock call through slot 13; D, legacy Read |

`CellRect32` is 16 bytes (four u32), `vec3` is 12 bytes (three f32), and `Jobs16` is a caller-constructed 16-byte
jobs object: four preserved leading bytes, then wealth-1/2/3 f32 values. A bare three-float array is too short
for slot 64. These describe argument/output storage, not the lot object's private layout.

Numeric/width evidence: getters/setters `0x006C2020–006C2257`; GetLocation four-byte stores
`0x006BF2E0–006BF2F9` versus GetSize byte stores `0x006BF300–006BF314`; slope/altitude loads
`0x006C2060–006C2083`; population word addition `0x006C04F2` and travel-population word returns
`0x006C0539`, `0x006C0543`; GetJobs outputs and balanced x87 stores `0x006BF80D–006BF887`;
GetTravelJobs `0x006BF350–006BF377`. The primary vtable occupies exactly `0x00AB19B0–0x00AB1AC7`.

#### 13.2.2 Serializable view, +4, vtable `0x00AB1994`

These six entries exhaust the view. Read/Write are the **lot** record operations of §§3.2–3.5, not a second
record or a property-only stream. Streams are borrowed references, not retained. The view can be Released
only when the caller owns a reference, for example after successful QI.

| Slot / offset | Name; target | Explicit arguments; Pop | Return and ownership | Effects; dependent subsystems |
|---|---|---|---|---|
| 0 / +0x00 | QueryInterface; `0x006C2260` | IID: u32, out: interface**; 8 | AL 0/1 and owned output on success | Subtract 4, primary slot 0 identity/rules; S |
| 1 / +0x04 | AddRef; `0x006C2270` | none; 0 | EAX = new shared count | Subtract 4, primary AddRef; S |
| 2 / +0x08 | Release; `0x006C2300` | none; 0 | EAX = remaining count or 0 at destruction | Subtract 4, primary Release, including Shutdown/destruction; S |
| 3 / +0x0C | Write; `0x006C0080` | stream: cIGZOStream*; 4 | AL 1 iff no stream error, else 0 | Emit §3.2 fields in order; no lot-field mutation; S |
| 4 / +0x10 | Read; `0x006C35F0` | stream: cIGZIStream*; 4 | AL 0 if version >8, a checked trip read fails or final stream error; otherwise 1 | §3.4 partial/legacy mutation, including initialized set before version rejection; does not Init/resolve references; S |
| 5 / +0x14 | GetGZCLSID; `0x006BF180` | none; 0 | EAX = `0xC9BD5D4A` | Constant, no mutation; S |

#### 13.2.3 Property-holder view, +8, vtable `0x00AB1930`

This is a plain holder: its only saved state is **properties** (§3.6; foundations §10.2). It has no exemplar
parent or inherited lookup. Own entries are ordered by unsigned u32 ID. All property, variant and list
interfaces below are the game's normal interfaces; successful getters do not wrap them in independent
copies. In particular slot 9 and property slot 5 preserve object/variant identity for in-place traffic writes.
For typed additions (slots 10–13), an existing property object remains the same object and its variant is
assigned; a missing ID creates a property. Slots 14 and 16 create a new property object even for an existing
ID. They insert it through slot 15, which replaces the entry's object, retaining the supplied object and
releasing the old one. There is no automatic change notification.

Every `ignored: bool` below is a real four-byte stack argument, even though its value has no effect. Omitting
it is an ABI error. Copy/add methods and remove methods do not infer a return bool from the SDK name.
**U** here means unused on a live lot, not unused on all holders in the executable. The game uses the shared
holder implementation extensively on other object classes.

| Slot / offset | Name; target | Explicit arguments; Pop | Return and ownership | Effects; dependent subsystems |
|---|---|---|---|---|
| 0 / +0x00 | QueryInterface; `0x006C2280` | IID: u32, out: interface**; 8 | AL 0/1; owned result on success | Subtract 8, restricted primary QI; no additional holder IID; holder clients |
| 1 / +0x04 | AddRef; `0x006C2290` | none; 0 | EAX = new shared count | Subtract 8, primary AddRef; reference owners |
| 2 / +0x08 | Release; `0x006C2310` | none; 0 | EAX = remaining count or 0 | Subtract 8, primary Release and possible destruction; reference owners |
| 3 / +0x0C | HasProperty; `0x005FEB20` | ID: u32; 4 | AL 1 iff own entry exists, else 0 | No mutation/AddRef; U (lot scalar-property helpers use slot 9) |
| 4 / +0x10 | GetPropertyList; `0x005FD9F0` | out: property-list**; 4 | AL 0 for null out; otherwise AL 1 and one owned list reference | New list of existing property objects, ascending ID, retaining each; caller Releases list; no holder mutation; U |
| 5 / +0x14 | GetPropertyBytes; `0x005FD290` | ID: u32, buffer: void*, inOutBytes: u32*; 12 | AL 0 if absent or variant not byte-block, else 1 | Copy min(input byte count, stored size) bytes; lower count to stored size if necessary; no allocation/ownership transfer; U |
| 6 / +0x18 | GetPropertyInterface; `0x005FD2D0` | ID: u32, IID: u32, out: interface**; 12 | AL 0 if absent/non-object/null object, otherwise object's QI result; success owns output reference | Delegate variant's GetValUnknown to object's QI; no holder mutation; U |
| 7 / +0x1C | GetPropertyString; `0x005FD260` | ID: u32, out: cIGZString*; 8 | AL 0 if absent or variant conversion fails, else 1 with output assigned | String assignment/conversion by variant; out is caller-owned, not retained; U |
| 8 / +0x20 | GetPropertyUint32; `0x005FD230` | ID: u32, out: u32*; 8 | AL 0 if absent or variant conversion fails, else 1 with output assigned | Variant GetValUint32 conversion, not a new property; no holder mutation; U (lot scalar-property helpers use slot 9) |
| 9 / +0x24 | GetProperty; `0x005FEB70` | ID: u32; 4 | EAX = stored cISCProperty*, borrowed, or null | No AddRef; property/variant/data identity preserved; D, B, T, Q, S |
| 10 / +0x28 | AddPropertyBytes; `0x005FFA40` | ID: u32, data: const void*, bytes: u32, ignored: bool; 16 | void | Copy byte block into own variant; retain no input buffer; U |
| 11 / +0x2C | AddPropertyInterface; `0x005FF950` | ID: u32, object: cIGZUnknown*, ignored: bool; 12 | void | Assign variant as object reference; retain nonnull object, release displaced variant contents; U |
| 12 / +0x30 | AddPropertyString; `0x005FF860` | ID: u32, value: const cIGZString*, ignored: bool; 12 | void | Copy string into own variant; no input-string reference retained; U |
| 13 / +0x34 | AddPropertyUint32; `0x005FF770` | ID: u32, value: u32, ignored: bool; 12 | void | Create/update own u32 property, keeping existing property identity; D (counters/zots), scalar property clients |
| 14 / +0x38 | AddPropertyVariant; `0x005FDEF0` | ID: u32, value: const cIGZVariant* or null, ignored: bool; 12 | void | Null value does nothing; otherwise make new property, assign variant, insert through slot 15; caller's variant not retained; U |
| 15 / +0x3C | AddPropertyObject; `0x005FF6D0` | property: cISCProperty*, ignored: bool; 8 | void | Retain exact object by its GetPropertyID; replace/release old entry if different; same object is a no-op; T (trip-statistics property), D, B |
| 16 / +0x40 | CopyAddProperty; `0x005FDFD0` | property: cISCProperty*, ignored: bool; 8 | void | Create new property with source ID and assigned variant, then slot 15; input property not retained; U |
| 17 / +0x44 | RemoveProperty; `0x005FEBC0` | ID: u32, ignored: bool; 8 | AL 1 if removed, else 0 | Remove own entry and release its property; D (zots/counters), property clients |
| 18 / +0x48 | RemoveAllProperties; `0x005FF3C0` | ignored: bool; 4 | void | Release all properties, clear entries, retain vector allocation; U as a virtual live-lot call |
| 19 / +0x4C | EnumPropertiesFiltered; `0x005FDA80` | predicate: PropertyPredicate, callback: PropertyCallback, context: void*; 12 | void | Ascending entries: call predicate(property,context); call callback on true AL; borrowed callback arguments, no retention; U |
| 20 / +0x50 | EnumProperties; `0x005FDA50` | callback: PropertyCallback, context: void*; 8 | void | Call callback(property,context) once per ascending entry; ignore its result, no retention; U |
| 21 / +0x54 | CompactProperties; `0x005FF160` | none; 0 | void | Shrink entry-vector capacity to size when different; preserve property identity and values with balanced reference changes; U |
| 22 / +0x58 | DestroyLot; `0x006C22A0` | deletionFlags: u32; 4 | EAX = former primary pointer, not a usable reference | Subtract 8; destruct lot as below; if bit 0 set, free allocation; final Release calls this with 1 after Shutdown; no second Shutdown by this entry |
| 23 / +0x5C | WriteLotRecord; `0x006C2330` | stream: cIGZOStream*; 4 | AL as Serializable slot 3 | Adjust property pointer by −4 to Serializable view; write entire lot, not just properties; U (S normally uses Serializable) |
| 24 / +0x60 | ReadLotRecord; `0x006C3AF0` | stream: cIGZIStream*; 4 | AL as Serializable slot 4 | Adjust by −4, read entire lot with all §3.4 mutations; U (S normally uses Serializable) |

`PropertyPredicate` and `PropertyCallback` are **cdecl** functions with two four-byte arguments,
`cISCProperty*` followed by `void*` context. Predicate returns bool in AL; callback's return is ignored. Iteration retains
neither input nor a per-entry protective reference and saves the original end pointer; structural changes
to this holder during callbacks are outside the native iterator invariant. On a missing ID, slots 5–8 leave
all outputs unchanged. On a present ID they use the ordinary variant conversions (foundations §9); slot 6
has the contained object's QI failure/output rules. Slot 5 requires the byte-block tag `0x8000` and never
returns the variant's internal data pointer. Object-valued properties have the save/load asymmetry of
foundations §10.2: a write includes them, but a subsequent plain-holder read drops them.

DestroyLot disposes the trip list and its payloads, travel-population map, population vector, road-access
cache and plain holder (releasing its property objects). It Releases any still nonnull configuration and
travel designate. It does not remove business-unit membership or run terrain levelling: normal final Release
first performs Shutdown. The destructor sequence is `0x006C25D0–006C26CC`; the deleting wrapper tests only
deletionFlags bit 0 at `0x006C3008`. Calling DestroyLot directly does not provide a second lifecycle path.

Width/dispatch evidence: typed getter argument forwarding and pops `0x005FD230–005FD300`;
byte-block bounds/copy `0x00915714–00915742`; object QI `0x00915A46–00915A5E`; add-method
epilogues `0x005FF856`, `0x005FF94C`, `0x005FFA3C`, `0x005FFB36`, `0x005FDF73`, `0x005FE062`;
remove pops `0x005FEC0D`, `0x005FEC18`, `0x005FF3D0`; callback cleanup `0x005FDA9D`, `0x005FDAAD`,
`0x005FDA70`; destruction `0x006C22A0–006C22A7`, `0x006C3000–006C3054`; whole-lot stream thunks
`0x006C2330–006C2337`, `0x006C3AF0–006C3AF7`. List construction/retention is at `0x005FD9F0–005FDA4F`,
`0x0090DC3D–0090DC69`. The property vtable ends at `0x00AB1993`; the following six entries are Serializable.

#### 13.2.4 Traffic-source view, +0x1C, vtable `0x00AB1914`

This view has six entries, not the primary lot's traffic-related methods. Its list is the very same trip list
saved in §3.2; trip objects/fields are §3.3. Mutating this borrowed container changes subsequent lot saves.

| Slot / offset | Name; target | Explicit arguments; Pop | Return and ownership | Effects; dependent subsystems |
|---|---|---|---|---|
| 0 / +0x00 | QueryInterface; `0x006C22B0` | IID: u32, out: interface**; 8 | AL 0/1; owned output on success | Subtract 0x1C, restricted primary QI; T |
| 1 / +0x04 | AddRef; `0x006C22C0` | none; 0 | EAX = new shared count | Subtract 0x1C, primary AddRef; T |
| 2 / +0x08 | Release; `0x006C2320` | none; 0 | EAX = remaining count or 0 | Subtract 0x1C, primary Release and possible destruction; T |
| 3 / +0x0C | GetTripList; `0x0080BC20` | none; 0 | EAX = borrowed pointer to mutable native list<cSC4TripData>, at primary +0x84 | No copy/AddRef; T creates, erases, confirms and reroutes entries through this list |
| 4 / +0x10 | UpdateTrafficVolume; `0x006C27E0` | none; 0 | void; no defined EAX/AL/ST0 result | Reconcile trip count/type/strategy/probe/status against residential/freight targets, traffic-simulator.md §4.1.1; changes trips only; T |
| 5 / +0x14 | GetBoundingRect; `0x006C22D0` | out: CellRect32*; 4 | Four u32 outputs x1,z1,x2,z2; EAX also echoes out, no ownership | Same saved rect as primary slot 14, no record writes; T |

Traffic-source slot 3 uses view+0x68 (`0x0080BC20–0080BC23`), slot 5 zero-extends four bytes and stores four
dwords (`0x006C22D0–006C22F5`), and slot 4 recovers the primary pointer at `0x006C27E3–006C27E9`. Residential
and industrial cases differ: the latter returns without change when no road or a travel designate exists
(`0x006C2A09–006C2A24`); other zones return without change. All probe/target rounding, trip reconciliation,
strategy-byte and partial-count rules in traffic-simulator.md §4.1.1 apply, including keeping an existing
leading probe when target count is zero. No slot on any lot view exposes the strategy byte directly.

#### 13.2.5 Nontrivial primary behavior and record effects

**Land value (slot 42).** Read the land-value simulator's current **total** cell-value map, not intrinsic value
or land-value class. Walk saved rect z1…z2 outside, x1…x2 inside, both inclusive; sum each zero-extended AL
cell result with a two-dword integer accumulator. Either reversed extent gives zero. Do not divide by area,
multiply by 16, convert to a float or retain a grid result in the record. Valid byte coordinates bound the sum
by 256×256×255 = 16,711,680, but the ABI still defines both output registers: EDX=0 and EAX=sum on valid lots.
The cell-service call preserves its native row/column order (z,x); the semantic cell is (x,z). Evidence:
`0x006BFC6E–006BFC84` initializes bounds/accumulator, `0x006BFC98–006BFCAB` zero-extends AL and adds with carry,
`0x006BFCC5–006BFCD4` loads EAX/EDX and returns without any x87 instruction. The simulator getter at
`0x006BD740–006BD747` forwards to its total-map cell getter. An asm-derived label such as `eax-or-void` or an
SDK floating signature must not override this integer channel.

**Occupancy (slot 61).** Use slot 46(true)'s profile capacity vector C and the population vector P, paired by
index as in §3.2.2. No state or livability guard is added. A zero building type or empty capacity vector writes
0. Otherwise start signed-int32 numerator N=0 and denominator D=0. In each C[i], visit keys in ascending u32
order; index P[i] at each key, inserting zero if missing. Start f32 F=0 and replace F with
`r24(extended(F) + extended(P[i][key]) / extended(capacity))` after each key. Sum capacities as signed-int32 with wrap,
then signed-divide by the number of keys, truncating toward zero, to get mean A. Replace N by the low int32
of `trunc64(extended(A) × extended(F) + extended(N))`; add A to D with int32 wrap. After all maps write
`r24(extended(N) / extended(D))` if signed D>0, otherwise write 0. The x87 division and multiply/add have no
intermediate f32 store except F. Empty maps inside nonempty C divide by zero; zero capacities are not guarded;
P must already have every C index. These are the same native container invariants as §3.2.2, not invented
successful empty-map cases. Only the output float and any inserted population keys change. Evidence:
`0x006C0D80–006C0DCB`, `0x006C0E15–006C0E44`, `0x006C0E70–006C0EA6`; the result store empties ST0.
Slot 62 differs: it produces the per-type f32 contributions specified in §3.2.2, without slot 61's integer
numerator truncation. Neither follows the travel designate.

**Configuration-object match (slot 11).** Null configuration or null object-array pointer gives null. Otherwise
obtain the supplied occupant's world position, visit configuration-object records in array order (52 bytes
per record), and obtain each record's world position from the lot developer's position helper. Return the
first record with both absolute x and z differences strictly less than f32 0.1 (`0x00A8C950`), with the
subtractions/comparisons in x87 precision; ignore y, type and orientation. No match gives null. The returned
record remains owned by the configuration array. The loop and comparisons are `0x006BFEE5–006BFF5F`.

**Condition mutation (slot 36).** Compare the full u32 argument with the old zero-extended condition byte.
Equal: no work. Different: store its low byte; if state is 4 replace **day of last state change** with current
city day. Then make the lot snapshot with §3.2.2's copy/ownership rules **after** that store/day update; queue
message `0xE99B3D4D` containing that copy, old condition, new condition and stored building type. Release the
local message/copy references, leaving the queued references alive. No state, vacate reason, wealth, resource
or needs-power update occurs. Thus argument 256 against old 0 still takes the changed path and posts old/new
byte values 0/0. Evidence: `0x006C26D0–006C2707`, `0x006C2720–006C27D7`. It is not a bool-returning setter.

**Road access (slots 28–30).** The cache is an unsaved ordered map from u32 purpose to bool. Key 0 is direct
road access. Slot 29 first uses cached key 0, or computes it if missing. True returns immediately, including
for nonzero purpose. False with purpose 0 returns false. Otherwise use a cached purpose value or compute it.
The purpose helper first tests slot 28; true caches true. Otherwise cache false **before** walking neighbouring
lots (prevents recursive cycles), enumerate the four adjacent edges in mask order 1,2,4,8 through the lot
manager, and seek a neighbour in state 2…4 whose fallback building profile has the same purpose and whose
slot 29(purpose) is true. Cache/return whether one was found. Slot 30 recalculates direct access, then each
already-existing nonzero cache key in ascending order. It does not clear/add all purposes first.

Direct access normally examines the lot-manager's outside edge-cell rectangles for network occupants: zones
1…6 inspect the facing edge only; other zones inspect edges 0…3. A network occupant answering IID
`0xA821EF94` qualifies through **network-interface slot 9, +0x24, `HasAnyNetworkFlag(0x449)`**:
the intersection of its u32 network flags with `0x449` must be nonzero. The mask is the union of
`0x1`, `0x8`, `0x40` and `0x400`; any one of these bits suffices, and flags outside the mask do not disqualify
the occupant. **Slot 8, +0x20, `HasNetworkFlag` / `HasAllNetworkFlags`, is not this predicate**: it requires
the intersection to equal the entire mask. Store key 0 true at the first qualifying occupant; otherwise false.
The normal path's IID, mask, indirect call and AL branch are at `0x006C1BC0–006C1BDB`; the call at
`0x006C1BD6` uses +0x24. The native all-bits and any-bit bodies are `0x00604D10–00604D25` and
`0x00604D30–00604D42`, respectively. QI for this IID returns the network primary interface
(`0x006192C6–006192E9`), matching traffic-network.md's network-occupant interface table.

The building-type test `0x1304` selects the ferry-terminal helper instead: scan occupants of type `0xA823821E`
within the rect, QI `0xE9793A65`, and inspect model IDs `0x2C520000`/`0x2C530000`; at their city cell, the
four cardinal traffic-network lookups use mask `0x449` and mode true, accepting a returned network whose
flag test `0x200000` is false. The helper starts true with no qualifying model; each qualifying model resets
the result false before its four tests, so the last such model determines the result. It returns directly,
without writing key 0. These tests acquire/release temporary occupant/list/interface references but change
no saved lot fields or network flags. Evidence: `0x006C1A30–006C1ABF`, `0x006C1B00–006C1D0F`,
`0x006C15D0–006C17DE`, `0x006C17E0–006C1939`, `0x006C1D10–006C1E01`.

**Live-state consequence and diagnostic limit (LR16; replace-lot29 LR8).** The missing normal-path contract
is the any-bit predicate. Slot 28 delegates to slot 29(0)
(`0x006C20E0–006C20E7`); a cached false therefore reaches callers without another network test. A network
whose flags are `0x1` gives true under the native predicate and false under an all-bits substitution. That
substitution can retain a false direct-access result and change the lot developer's travel decision.
In lot-developer.md §§4.2/5.4, residential purpose 1 combines slot 28 with job access; purposes 5…8 use slot
29(purpose); the other purposes test slot 28 when they have positive travel jobs. A false road result increments
the shared travel counter `0x8999B10B` and fails `TestTravel` when the new counter exceeds 2; starting at
zero, this is the third consecutive failing visit. Prior commute failures count toward the same counter. For a
developed nonzero-purpose lot whose state changes, `UpdateLot` then calls `SetState(4, 0, 4)`; the last
failing test supplies reason 4. The road result itself does not vacate the lot or change its population/income.
Road/commute decision and counter evidence: `0x006CB075–006CB0C3`, `0x006CB136–006CB1B1`.
See §17.10 for a worked flag/cache case and its travel consequence.

This explains a mechanism for excess reason-4 transitions, not the measured house run's entire decline.
The supplied ON/OFF logs first differ in a monthly DIGEST on **2026-04-01**, not at termination in 2029:
`lot29-r6-house-on1.log:16634` has funds 2044, R/C/I 12789/2386/7609 and income/expense 2957/3597;
`lot29-r6-house-off.log:11925` has 3195, 12797/2386/7609 and 4107/3597. These aggregates do not identify
which lot first differed or prove that road access caused the income difference. The reported 4238 reason-4
transitions also do not distinguish road failure from commute failure. No additional lifecycle effect, city
termination rule or explanation for the income amount is inferred from those observations.

To settle causality at runtime, compare native and replacement calls from the same starting city before that
first differing April DIGEST. At the first unequal road result, record city day and lot identity, saved rect,
zone/facing, ferry selection, key-0 cache presence/value, the ordered queried occupants and their u32 flag
words, selected interface slot/mask, predicate result and returned lot result. Compare identical inputs;
a prior cached false must be traced back to its calculation, or refreshed through slot 30 for the comparison.
Then record that lot's `TestTravel` state/purpose, job-access result, trip-length byte, travel counter before
and after, and ensuing state/reason change. A native true/replacement false with a nonzero intersection
smaller than `0x449` identifies this predicate defect; a no-road failure with a nonzero trip byte distinguishes
it from a commute-only failure. To establish that it accounts for the whole acceptance failure, rerun the
same house pair with only this predicate corrected and compare the first differing monthly populations,
income and funds through the control's 2032-01-01 endpoint. If they still diverge, locate the first unequal
per-lot population/wealth or income contribution and its preceding mutations; this spec does not assign
that remaining divergence to road access. Acceptance item (a) remains a runtime obligation.

**Jobs and travel aggregation (slots 57/58/64).** Slot 64 zeros only the three Jobs16 payload floats first.
With a nonzero fallback type and a profile whose purpose is nonzero, get the developer type for that purpose
and current occupant wealth, read its u16 population through slot 53, ask the resource simulator for the
distribution of resource `developerType+1` with mode 0, then multiply each of its three f32 values by that
population, storing each as f32. Otherwise, obtain the actual building and its exemplar holder. Property
`0x27812841` is the array of u32 (resource,count) pairs: resource `0x2010`, `0x2020`, `0x2030` assigns the
corresponding payload float from unsigned count. Optional property `0x27812843` supplies a parallel f32 count
array, overriding the integer counts by pair index. Other resource IDs are skipped; duplicate recognized
resources overwrite in pair order. Missing building/exemplar/count property leaves the zeros. If no building
but a configuration exists, the native path calls configuration GetID and returns with zeros; it does not
synthesize jobs from that ID. No lot field or input reference ownership changes. Evidence:
`0x006BF800–006BF99F`, especially three output stores `0x006BF80D–006BF813`, profile branch
`0x006BF824–006BF887`, fallback arrays `0x006BF8BC–006BF998`.

Slot 58 walks all source population maps, adding their word values by key to this lot's travel-population
map with word wrap; a key present with count zero is still inserted. Then call source slot 64 once, add each
payload float to this lot's corresponding travel jobs value and store each as f32. No source AddRef or
implicit designate assignment occurs. Slot 57 instead clears the travel map and sets all three travel jobs
to +0.0. Both finish with the native UpdateJobs helper (`0x006C0550–006C059D`, `0x006C0BA0–006C0CED`).

UpdateJobs does nothing if the traffic simulator is unavailable. Otherwise get this lot's borrowed holder
through slot 3. For i=0,1,2 in order, round travel jobs[i] by taking
`bits32(r24(extended(jobs[i]) + 12582912.0f)) − 0x4B400000` as signed int32 n. Read traffic
GetMaxTripCapacity(holder, **0**, for all three i) as signed old. If n≠old, divide the signed absolute wrapped
difference by old when n<old, or by the **rounded wealth-1 value** when n≥old; only a strictly greater-than
f32 0.1 result calls SetMaxTripCapacity(holder,i,n). Unordered compares do not write. The denominator uses
wealth 1 even in the wealth-2/3 passes, and all old-value reads use destination type 0; do not silently
correct either quirk. Each read occurs after preceding writes. The setter creates/updates the lot's
**properties** entry `0x8A0B4100` (including u16→u32 promotion), using traffic-simulator.md §4.2; it does not
change population, state, wealth or trips. Evidence for rounding, reused index and denominator slots:
`0x006BF6BC–006BF721`, `0x006BF724–006BF787`, `0x006BF78A–006BF7F8`.

#### 13.2.6 Worked month: one residential lot

This is an input fixture, not shipped tuning: an initialized lot has rect (10,20,11,21), location (10,20),
size 2×2, facing 0, altitude 300.0, state 3, occupant wealth 1, historical false, a valid residential
configuration/building, profile purpose residential with native wealth 1 and C=[{`0x1010`:100}], and population
P=[{`0x1010`:80}]. Its state-change day is
200 city days ago. The lot's flags are `0x58` (needs power, has power, vertices locked); counters are zero.
No unit or travel designate is involved. Assume the month's completed land-value grid gives these cell
values and the land/development collaborators otherwise admit the lot:

| Cell (x,z) | Total land value byte |
|---|---|
| (10,20) | 40 |
| (11,20) | 60 |
| (10,21) | 80 |
| (11,21) | 100 |

1. The land-value simulator's pass reads terrain, zoning and the developers' desirability grids at the lot's
   position; it does not call a live-lot view or store land value in the lot. A dirty, completed pipeline is
   restarted by the month message; later daily/idle slices perform the work (landvalue.md §§5–6).
   A compatibility read of slot 42 on this lot visits the four cells in the table's
   order and returns EDX=0, EAX=280, with no ST0 value. The result is not 70 (the mean), 40 (the location
   cell) or a currency conversion. Primary slot 15 writes center (176.0,300.0,336.0); slot 16 returns 300.0
   in ST0. The center output itself leaves no x87 result.
2. The tract-development suitability pass obtains the saved location through slot 21 (two u32 outputs) and,
   when its profile supplies a land-value range, reads the land-value service there: 40. A range 30…50 admits
   this **land-value test**. Other suitability tests still apply. State comes from slot 33 as EAX=3; slot 37
   gives EAX=1; slot 40 gives 200. The profile's cell test does not compare 280 against its range. In this
   path land value is an external grid input, not a new member/method result cached on the lot.
3. During the lot developer's condition read, slot 61 writes `r24(80/100)` through its f32 pointer. Here the
   inner F is f32 0.8; A=100, N=80 after truncation, D=100, giving f32 0.8 in the output and nothing in ST0.
   Slot 53(`0x1010`) returns AX=80 without changing P; slot 50(`0x1010`,false) returns AX=100; slot
   52(`0x1010`,false) returns AX=100 because no other capacity key consumes a fraction. The latter is the
   allowed count for this type, not 100−80. Slot 54 returns the same live vector on repeated reads.
4. If the population pass now requests slot 55(`0x1010`,90), the clamp is 100, the stored count becomes 90,
   wealth recomputation retains wealth 1 and AL=1. State, state-change day, travel population and travel jobs
   remain unchanged. A fresh occupancy read writes **f32 0.89**, not f32 0.9: F is
   0.89999997615814208984375 after its f32 store, so 100×F is 89.999997615814208984375;
   truncation makes N=89, D=100. The final output is f32 0.88999998569488525390625
   (bits `0x3F63D70A`). This population setter does not itself rerun traffic aggregation or change the
   published land-value map. A read-only month omits this step entirely.
5. D/B/T clients reaching counters/trip statistics use slot 3's borrowed holder without Release. The traffic
   source iterator similarly casts through slot 4, but owns a separate reference it acquired to the lot;
   releasing that iterator result reduces the shared count once. A save through the +4 Serializable view
   writes the current population (90 if step 4 occurred), the unchanged geometry/flags/day and ordinary
   properties/trips. It writes no aggregate land-value or occupancy field.

This sequence illustrates data dependencies within a month, not a global ordering barrier between completed
land-value work and every development pass. Native suitability/location/grid evidence is tract developer
`0x00707A60` (location, state/age and optional land-value bounds); lot condition uses `0x006C4630`.
Land-value pipeline work is `0x006BE7B0`, month dispatch is `0x006BF0D0`, and its total-value getter is
`0x006BD740`. The occupancy f32 store, integer truncation and final output store are
`0x006C0DCB`, `0x006C0E22–006C0E44`, `0x006C0E79–006C0E8A`. The never-called lot aggregate is
specified for ABI completeness; it must not be inserted into the game's monthly call sequence.

#### 13.2.7 Other record collaborators

These are **services called by** the lot/occupant records, not additional views answered by the live lot.
Configuration slot 3 (+0x0C) returns its u32 ID in EAX; configuration-manager slot 8 (+0x20) takes a u32 ID
(pop 4) and returns a configuration pointer. Business-unit slot 4 (+0x10) removes the given lot (pop 4).
Occupant slots 5,15,16,17 are IsInitialized, PostOccupantMessage, GetHighlight, SetHighlight, respectively
(§§2.3–2.4). Stream primitives and GetError remain foundations §2.2. They do not enlarge the 107-entry lot
surface. The four tables above specify even unused slots; no unknown/reserved live-lot entry is left for an
implementer to guess.

### 13.3 SDK notes

- `cSC4TripData.h` (gzcom-dll): the four bytes the header calls `unknown4`, `transitType`, `flags1`, `unknown5` are,
  on Windows, type, strategy, flags and status in that order, and the header's `unknown6…unknown9` bytes at the end
  are one u32 **travel types used** (checked in `Write` `0x0070E0B0` and `Read` `0x0070E160`; pop 4, EAX).
- `cISC4DBSegmentIStream::ReadGZSerializable` takes two arguments (foundations §2.7); every `Read` here calls the
  helper `SC4ReadGZSerializable` `0x005D9730` (stream, IID, destination, loadNow = 0).

---

## 14. References and resolution on load *[compat]*

| From | Field | To class | Read IID | Observed count | Resolved |
|---|---|---|---|---|---|
| zone developer | cell map | lot | `0xE818FCED` | 21,179 cells → 12,985 lots | at read (dedup, `zones.md` §10.2) |
| lot | business unit | business unit | `0x6A233069` | 7,746 | stored; used by Shutdown and the lot developer |
| lot | travel designate | lot | `0xE818FCED` | 6,110 | stored |
| business unit | members | lot | `0xE818FCED` | 7,746 | stored |
| retaining wall | lot | lot | `0xE818FCED` | 1,735 | stored |
| construction | lot, building | lot, building | `0xE818FCED`, `0x87DDA3A9` | 31 + 31 | stored |
| construction | ground / roof props | prop | `0xE9793A65` | 12 | stored |
| construction | effects, current effect | effect prop | `0x2977AA49` | 99 | stored |
| construction | crew | pedestrian | `0xA9B40F05` | 54 | stored |
| lot developer | night deque | building | `0x87DDA3A9` | `lot-developer.md` §13 | stored |
| occupant manager | all occupants | all occupant classes | `save-records.md` | — | inserts into the spatial index |
| crime simulator, budget, traffic, … | lots | lot | `save-records.md` | — | |

No reference in any save is dangling or names a class missing from the index.

Which references keep which objects alive (from all saves): lots are reached from the zone developer, other lots,
business units, walls, constructions and the crime simulator; buildings from the occupant manager, the lot
developer, constructions, automata and some simulators; base textures, walls and foundations **only** from the
occupant manager; flora from the occupant manager and automata; props from the occupant manager, the prop developer,
automata and constructions; business units only from lots; bare occupants only from the occupant manager. A writer
must therefore emit an object exactly when one of these owners references it (foundations §3.5).

*[quirk]* One lot (`City - Rush Hour Tutorial.sc4`, a 1×2 empty R$$ lot, state 1) is not on the zone developer's map
and is saved only because the crime simulator still references it. After a load it is read but never `Init`ed by
the zone developer (`zones.md` §1.2).

---

### 14.1 Load/month record observation inventory (LR12)

**Scope [choice].** `lotrec_shadow` means **all current master-list `cSC4Lot` records**, not all
classes in §1. Its denominator at each PostCityInit/month observation is the lot manager's current
master-list size. Each lot contributes one complete §3.2 record: all 27 fields, including the plain
property holder and every inline §3.3 trip. A business-unit or travel-designate reference contributes
its nullness and identity, not the referenced object's payload. The section-1 catalogue describes save
formats; it is not a promise that the lot manager enumerates all those objects. This choice defines a
record-only shadow, not complete-owner replacement (§15), save-file validation (§17.1), or a test of
simulation mutations. No original game option named `lotrec_shadow` is implied.

**Discovery and native boundary [compat].** Use the current city's lot-manager view and its registered
safe iterator; do not reconstruct membership by spatial probing, occupant lookup or following references.
The interface calls below use x86 `__thiscall` (ECX is the indicated interface pointer). Slot numbers
are zero-based. The caller runs synchronously on the simulation thread, without yielding or invoking
simulation mutations, and finishes and releases the iterator within the observation.

| Receiver / operation | Native signature and result | Ownership |
|---|---|---|
| `cISC4City`, slot 33, `0x00472E70` | `GetLotManager()`, no stack arguments, EAX = `cISC4LotManager*` | Borrowed, no AddRef; reacquire for each observation. |
| `cISC4LotManager`, slot 16, `0x00809250` | `GetLotCount()`, no stack arguments, EAX = 32-bit master-list count | Scalar; use the count before and after the walk to detect an inconsistent observation. |
| Same manager, slot 15, `0x00734850` | `GetSafeLotIterator()`, no stack arguments, EAX = iterator pointer | Caller owns the non-COM iterator; acquisition registers it, but does not AddRef any lot. |
| Borrowed `cISC4Lot*`, slot 0, `0x006BF190` | `QueryInterface(u32 iid, void** out)`, callee pops 8; test AL; IID `0xE4FDA3D4` gives the Serializable view | Successful QI AddRefs once; release that view once on every subsequent exit path. Initialize out to null before the call. |
| Acquired `cIGZSerializable*`, slot 3, `0x006C0080` | `Write(cIGZOStream&)`, callee pops 4; success is AL only (§13.1) | Borrows the stream; no ownership transfer. Use the actual interface, not a cast of the lot pointer. |

The city getter is a single pointer load with no retain (`0x00472E70–00472E76`). Count is a dword
load (`0x00809250–00809253`); its unrelated folded export name does not change this use. Iterator
acquisition allocates 8 bytes and registers the result (`0x00734860–007348AA`). Its public native
interoperation contract is the nonvirtual layout in `zones.md` §8.3.1: `{node, list}` at offsets 0/4;
list points to a one-pointer sentinel holder; nodes contain `{next, prev, lot}` at offsets 0/4/8.
End is node equal to the sentinel. Advance the cursor to the next node before using the current
node's borrowed lot. No reference is acquired by reading a node. The manager owns membership's lot
reference; QI's temporary reference protects only the individual capture. Neither retain that borrowed
pointer across observations nor Release it as though enumeration had transferred a reference.

Destroy the iterator on success **and** on early failure: `cSC4SafeListIterator<cISC4Lot*>::Release`
at `0x0045D5B0` is a **nonvirtual** `__thiscall` with no stack arguments and no required return value.
It unregisters from the list's iterator registry when list is nonnull, then frees the iterator with
the C allocator (`0x0045D5B4–0045D5D7`); it is not COM slot 2. The equivalent unregister/free sequence
is specified in `zones.md` §8.3.1. QI's Serializable adjustment is +4 and its AddRef is proved at
`0x006BF1BC–006BF1D0`; this is evidence for using QI, not permission to forward native methods on
replacement layouts. No factory installation, owner replacement or native `Read`/`Init` is required.

**Capture and comparison [choice].** For each enumerated lot, capture its own native Write payload in
an isolated memory stream implementing the existing stream/reference contracts (foundations §2.2/§2.7).
Reference writes record stable per-observation `(classID, objectID)` identities and do not recursively
serialize their targets. Preserve aliases; zero stays null; different objects must not collapse to one
identity. Keep the same identity mapping for expected and replacement observations. Feed the complete
payload to an isolated record reader, require successful acceptance and exact byte consumption, and
compare every §3.2 field and nested value, preserving float bits, map/vector/list order, property types,
and reference identities. A byte re-encoding comparison may implement this check provided it preserves
record metadata (including configuration ID) and uses that same reference mapping. Calling native Init
or resolving references into live replacement simulation objects is not part of this check. In particular,
do not compare a pre-Init Read object's Write with native Write after Init and call the configuration-ID
difference a codec failure: §3.5 describes the native distinction. No entry CRC or class-store header is
part of this per-object payload comparison.

An observation reports expected count N, visited distinct lots, successful full captures and exact
records separately. It is exact only if the walk ends normally, both counts are N, exactly N distinct
nonnull lots were visited, and all N records were captured/read/compared successfully. Missing city or
manager, failed iterator/QI/Write, stream error, malformed/null/duplicate node value, count discrepancy,
reader rejection, trailing bytes or any unequal field is an explicit incomplete/error/mismatch outcome;
never drop a failing lot from the denominator. An available empty manager is a valid `0/0` observation,
reported as empty, and supplies no evidence of nonempty record correctness. Equal counts alone are not
evidence of stable membership under mutation; the synchronous no-mutation requirement still applies.

**Objects outside this inventory.** The crime-only lot in §14 is intentionally absent unless it is also
in the current master list; its saved existence does not enlarge N. A unit reached from a lot, another
lot reached as travel designate, and an off-list lot reached through either remain reference identities
only. Buildings, base textures, retaining walls, foundations, construction, all prop types (including
signposts/decals), wild flora and bare occupants are outside this shadow, even when spatially on a lot.
There is no business-unit or occupant discovery pass in this contract. The three unspecified prop
subclasses listed in §1 must therefore be reported as outside scope, never as successfully parsed,
substituted with their base record, or silently included in an all-class exactness claim. A future
all-class shadow requires a separately named inventory and discovery contract; a save-class record
walk can validate saved off-list objects (§17.1), but is not equivalent to this live master-list walk.

### 14.2 Separate unit, base-texture and retaining-wall record inventories (LR15)

**Scope [choice].** Three separately named, default-off observations may extend §14.1:
`lotunit_shadow` for distinct units referenced by current master lots, `lotbasetexture_shadow`
for registered base-texture occupants, and `lotwall_shadow` for registered retaining-wall occupants.
These names describe record observations, not original game options. Keep their counts and verdicts
separate from `lotrec_shadow` and inline trips. They authorize no replacement, simulation mutation,
native Read/Init, or all-allocation owner census. Use the same synchronous, known-live,
PostCityInit/month boundary and isolated Write/reference-identity comparison as §14.1. Do not
observe during Shutdown, destruction or concurrent membership changes.

**Business units [compat and scope].** Lot slot 25 (+0x64), `GetUnit()`, is `__thiscall`,
no stack arguments, EAX = nullable borrowed `cISC4BusinessUnit*`. Its target `0x0068D1C0`
only reads lot +0x4C and returns (`0x0068D1C0–0x0068D1C3`); the export's shared
`GetTransientAuraGrid` name does not change its meaning on the lot vtable. Reading it neither
AddRefs nor transfers the lot's reference. Never Release the getter result. `SetUnit` owns that
reference (§4; `0x006C20BF–0x006C20D3`). A unit may be shared by several lots: equality of
their nonnull unit-interface pointers denotes one unit, not several payloads.

Enumerate every master lot with §14.1's iterator/count rules, including lots whose unit is null.
The unit denominator U is the number of **distinct nonnull GetUnit results**, not the lot count,
number of nonnull edges, or sum of unit member counts. Acquire Serializable through QI
(`0xE4FDA3D4`, slot 0, `__thiscall`, pop 8, bool in AL, out pointer); retain that acquisition
until its capture completes, and Release it once on every success/failure path. Unit QI
`0x006BF390` returns unit +4 and AddRefs (`0x006BF3AC–0x006BF3BE`); its unit IID is
`0x6A233069`. Use Serializable slot 5 to require class `0x4A232DA8`, then slot 3
`Write(stream&)`, pop 4, AL success (`0x006C0370`). Do not use a guessed primary-to-Serializable
offset in place of QI. A later observer pin, if needed, is a separate owned reference.

The union of master lots' getters is exhaustive **for this explicitly rooted inventory**, not
unconditionally for all extant units. Native constructor `0x006C0800` creates an empty member
list (`0x006C082E–0x006C0844`); the factory `0x006C0EB0` can create it without adding a lot.
Unit Write accepts an empty list and writes count zero (`0x006C0382–0x006C039C`). Conversely,
the normal developer path creates a unit and adds its chosen lot and the candidate lot
(`PickUnitToJoin`, `0x006CB260`, new-unit call `0x006CB45C`); membership retains both directions
(§4). Those normal-path invariants and the agreement observed in saves do not turn GetUnit
into a global unit enumerator. An empty unit, a unit referenced exclusively by off-master lots,
or an inconsistent unit whose members lack reciprocal getters is not independently discovered
by this walk. Do not recursively enlarge U from the unit's member-list references or silently
claim all allocated/saved units verified. A unit reached from a master lot is still captured
in full, with every member reference in its original order, including off-master members.

For example, master lots A/B/C returning X/X/null give U=1 and one X payload. If X's payload
lists A/B/D, D is preserved as a reference identity; D is not added to the lot inventory.
A separate empty unit Y with no master-lot getter is outside U, not a missed X record.
An available empty master list, or a complete nonempty list whose getters all return null,
gives valid empty U=0; report which case occurred and retain the master-lot counts.
If the master walk is incomplete, no unit census is exact even when every discovered unit matched.

**Registered occupant inventory [compat].** Obtain the borrowed manager with current city
slot 34 (+0x88), `GetOccupantManager()`, `__thiscall`, no stack arguments, EAX nullable.
`0x00423710–0x00423716` only reads city +0x12C; do not Release this borrowed getter result.
For each of the two observations use a fresh empty native `ilist<cISC4Occupant*>` and the
following native query. These are manager-vtable calls, not calls on texture/wall interfaces:

| Boundary | Arguments and result |
|---|---|
| Slot 39 (+0x9C), `0x0049DD80`, `GetOccupantsByOccupantManagerCells(list&, min2*, max2*, type, maxCount)` | ECX = manager; five 32-bit stack arguments, pop 20; bool in AL. Pass explicit signed integer corners min=(0,0), max=(191,191), type `0x497F6D9D` for base textures or `0x895D1169` for walls, maxCount=`0xFFFFFFFF` (−1/all). Corners are inclusive world-grid indices, not lot cells or world metres. |
| Slot 38 (+0x98), `0x004A0A70`, same signature except fourth argument is a filter pointer | Equivalent filter overload: full-grid corners as above, null filter to get all types, maxCount=−1. If used, select the target by occupant slot 7 `GetType()` and validate its Serializable class; do not treat unrelated classes as target records. |
| Slot 48 (+0xC0), `0x0049EF30`, `ReleaseOccupantList(list&)` | ECX = manager; pop 4; no meaningful return. Releases each list element's reference and clears/frees its nodes; the caller still frees the list sentinel with allocator A. |

Slot 39 installs the nonzero type in the shared type filter and dispatches slot 38
(`0x0049DD87–0x0049DDA2`). The filter matches GetType, not GetGZCLSID. Do not substitute the
class IDs for these type constants. Use Serializable QI/slot 5 to require actual class
`0xC97F987C` or `0x49C05C9F` before capturing §6 or §7. A selected occupant of an unexpected
class is an explicit unsupported/mismatch result, not permission to parse it as the expected
base record. QI for Serializable from either occupant view owns one additional reference:
base occupant QI `0x005E89E3–0x005E89F7` returns its +4 Serializable view and AddRefs.
The derived native views adjust through that base (texture `0x005DEA8F–0x005DEA92`, wall
`0x005E28CF–0x005E28D2`). Use QI; the list pointer need not be the allocation's start.

**Bounds and uniqueness.** Slot 38 clamps explicit coordinates to 0…191
(`0x0049DEC0–0x0049DEFA`), visits x outer/z inner inclusively
(`0x004A0CD3–0x004A0CFA`), and applies no bounding-box intersection test. Full-grid corners
cover every registered cell, including the padding beyond city terrain, without depending on
whether a lot exists or contains the occupant's box. Null corners are **not equivalent**:
they default to (64,64)…(city manager-cell counts +63), a smaller city-cell range
(`0x004A0AE4–0x004A0AE9`, `0x004A0B19–0x004A0B29`). Standard-city-cell and BBox queries
also have geometry filtering and are not substitutes. In a valid native grid, occupants in
several cells are suppressed by the query's duplicate set (`0x004A0C00–0x004A0C22`); each
selected occupant appears once, in first-visited-cell order, then newest-first within that
cell. The manager's single-cell marker bypasses that set; malformed indexing is not repaired
by the observer. A duplicate or null result is an explicit inventory error, never an extra
exact payload or a silently dropped record. See [occupant-manager.md](occupant-manager.md)
§§4, 8 and 12 for the native grid/list invariants and allocator A.

The resulting denominator B or W is all distinct selected occupants **registered in this
manager's full grid**. It includes registered objects outside all lot bounds and objects
whose lot reference is null/off-master. It excludes unregistered allocations and objects
removed before this observation; the query cannot establish an all-allocation census.
Missing/null manager is unavailable, not an empty city. No lots are needed for this query.

**List ownership and empty/failure distinction.** The list holder contains a pointer to a
12-byte circular sentinel; each 12-byte node is next/prev/occupant at +0/+4/+8. Sentinel and
nodes use allocator A, not the manager grid's STL allocator. Start empty (both sentinel links
self), as the query appends without clearing. Every appended occupant is AddRef'd once
(`0x004A0C38–0x004A0C3C`) before node allocation/linking
(`0x004A0C8B–0x004A0CA9`). The list owns that reference until slot 48 releases it
(`0x0049EF40–0x0049EF50`); do not separately Release a borrowed node value and then also call
slot 48. Release QI's separate Serializable acquisition once, on every outcome. Always clear
the list and free its sentinel, including after a failed capture or reader rejection. Do not
keep a node pointer after cleanup. Allocation failure/foreign exception is an incomplete
observation; native query success is not a checked-allocation guarantee.

Not initialized, or maxCount=0, returns false with the list untouched
(`0x004A0A8A–0x004A0AB5`). Otherwise the query returns true even with no matches
(`0x004A0D10`, `0x004A0D5A–0x004A0D62`); true does not mean nonempty. Thus true plus an
empty fresh list is successful empty B/W=0, while false plus the same empty list is failure.
Use the list query rather than callback iteration to distinguish these cases: the callback
iteration's uninitialized-true quirk (occupant-manager §9) is unsuitable as an availability test.
No finite positive maxCount is authorized for a complete census: it can truncate while still
returning true. Never reuse a nonempty list as the census denominator.

**Comparison and verdict [choice].** Count complete discovery separately from capture and exact
payload counts. Unit discovery must finish its full master walk; occupant discovery must return
true from the full-grid unlimited query and finish a well-formed distinct-node traversal.
Capture every selected record using §14.1's stream/reference map and §13.1's return channel;
base-texture/wall Write return EAX=0/1, unit Write returns AL. Preserve reference aliases,
texture record ordering/all 14 bytes per record, wall masks/hashes, and floating-point bits.
An exact observation requires complete discovery and exactly U/B/W successful full-consumption,
equal records. Failure to QI/identify/Write/read/consume, reference-map error or any unequal
field remains in the denominator as an error/mismatch. In particular, §7's hash-induced
edge-mask growth on Read is a real same-input mismatch, not a reason to normalize native
bytes or suppress a wall. Successful empty is reported as empty and supplies no nonempty
record evidence. These observations can credit only their named rooted/registered record
inventories; they do not establish complete-owner replacement or simulation equivalence.

## 15. Replaceability

### 15.1 New-object paths bypass the class registry (LR8)

Registering lot, business-unit and retaining-wall class factories covers serialized-object creation, not
creation during play. With native developers retained, the following are fixed direct edges, not directory
lookups or replaceable owner vtable slots. Addresses in this table were checked against native instruction bytes.

| Native source | Direct target | ABI/ownership at the edge |
|---|---|---|
| Zone developer CreateLot body `0x0072E520` (lot-manager vtable `0x00AB37C0`, slot 23): `0x0072E530`, `0x0072E557` | pool allocation `0x006BF430`, then parameterized lot constructor `0x006C1E50` | allocation called with one caller-cleaned 4-byte size (0x8C); constructor ECX = allocated storage, five stack words in logical order x, z, width, depth, facing, callee pops 20, EAX = same lot. At `0x0072E564` store output; `0x0072E56B` AddRef; `0x0072E573` Init. No GZCOM lookup. |
| PickUnitToJoin `0x006CB44C`–`0x006CB474` | allocate 0x20 via `0x005E55E0`, constructor `0x006C0800`, then unit slot 3 twice | allocation caller cleans 4; constructor `__thiscall`, no stack args, EAX = storage. Adds candidate then current lot; no creation AddRef or Release. Member lots acquire the unit through SetUnit (§4). |
| ClearLot `0x006C8A3D`; AddLotFoundation `0x006C8C23` | static wall helper `0x005E3070` | `__cdecl(lot*, add)`, two 4-byte stack slots, caller cleans 8; low byte is boolean. Clear uses false; foundation uses true. Return AL ignored. |
| ClearLot `0x006C8A62`; AddLotFoundation `0x006C8C34` | `0x005E3960` UpdateSurroundingRetainingWalls | `__cdecl(lot*)`, caller cleans 4; no defined return. |
| lot-developer slot 62 wrapper `0x006C4190`, direct call `0x006C4195` | `0x005E37F0` UpdateRetainingWalls | target is `__cdecl(lot*)`, not an owner virtual method. It can call `0x005E3070` itself (`0x005E392E`, `0x005E3947`). |
| AddRetainingWallOccupant `0x005E30E7`, `0x005E30F5` | wall pool allocation `0x005E2A50` (0x6C), constructor `0x005E2F40` | direct storage construction, not a class-factory call; helper AddRefs the new wall and later releases its creation reference (§7.1). |

The outer CreateLot call is `__thiscall(x, z, width, depth, facing, outLot**)`, callee pops 24,
AL = whether `*outLot` is non-null (`0x0072E67E`–`0x0072E68F`). Its constructor uses 4-byte argument
slots but stores cell bounds and facing as bytes (`0x006C1F69`–`0x006C1F7F`); this is not an ABI of
five packed bytes. The default constructor `0x006C2340` instead has no stack arguments; the
pool-allocation/default-construction helper `0x006C2FC0` tail-jumps to it at `0x006C2FEC`. Replacing
that helper does not affect the separate parameterized-constructor path above.

**Ownership across the boundary:**

- A fresh lot's count starts at 0 (property holder at +8 clears primary +0xC, `0x005FF10A`).
  CreateLot acquires one reference for the returned pointer (`0x0072E56B`) and a second for the master
  list (`0x0072E5D8`). The cell loop stores the same pointer in each cell (`0x0072E617`) without per-cell
  AddRefs; safe-iterator insertion (`0x0072E62E` → `0x0072C810`) and ordinary insertion
  (`0x0072E64D`–`0x0072E65C`) use that second reference without another AddRef. Caller and list must each
  release their own reference. Native Release at count ≤1 calls Shutdown then the property-view deleting
  destructor (`0x006BF471`–`0x006BF480`, `0x006C22A0` → `0x006C3000`); that destructor returns storage
  to the native lot pool (`0x006C3015`–`0x006C3026`). A native in-place constructor therefore requires
  its native pool storage, not a foreign replacement object or an arbitrary same-sized buffer.
- The business unit is not held by a creation reference in PickUnitToJoin. Its member list retains each lot
  and each member's back-pointer retains the unit (§4). Preserve both directions and break them through
  Remove/Shutdown; merely releasing an extra constructor reference is not membership removal.
- The static wall helpers borrow their input lot. The new wall's slot 6 SetLot (`0x005E2CB0`, pop 4)
  AddRefs a changed non-null new lot **before** storing it and releasing the old lot
  (`0x005E2CC8`–`0x005E2CD6`). The helper holds one wall reference from `0x005E3104`, hands its
  occupant view to occupant-manager slot 20 at `0x005E3246`, and releases its local wall reference at
  `0x005E3264` (also on the property-load failure path). The wall constructor `0x005E2F40` is
  `__thiscall`, no stack arguments, EAX = input storage (`0x005E2FB8`–`0x005E2FBB`). Native wall
  allocation uses the distinct pool at `0x00B0A8B0`; its free path is `0x005E32F0`. Update and surrounding
  helpers release acquired occupant/interface/list-element references (§7.2); they do not transfer ownership
  of their input lot to the caller.

**Supported replacement boundary:** use a replacement zone developer/lot manager together with replacement
lots, and a replacement lot developer together with replacement business units and retaining-wall creation/
update helpers. Their ordinary interface calls can then create the replacement objects and implement the
specified wall operations without entering the direct native edges above. This is a dependency requirement,
not evidence of a native plugin installation API. The city holds both zone-developer and lot-manager views
of one owner (foundations §3.6); replacing only one view is not this boundary.

No supported installation/restoration contract for independently intercepting these direct edges has been
established by these bytes or the class-object-store contract. Consequently **standalone complete-owner
activation with both native developers retained is unsupported by this spec**. Method addresses and factory
registration are insufficient. This does not claim machine-code interception is impossible; such an approach
would need a separately proved patch, calling convention, storage, lifetime, and restoration contract before
being used. No hook contract is implied here. Existing objects and saved references are not migrated by changing
a factory registration; restoration while such objects survive has no specified native mechanism.

#### 15.1.1 Factory-only wall activation (LR-lotocc28-1) *[compat]*

**Serialized-object substitution is a limited load boundary, not complete retaining-wall replacement.**
An explicitly requested factory for class `0x49C05C9F` (`GetGZCLSID`, `0x005E2990`) may supply a wall for a class-store
entry. That does not cause native developers to create replacement walls during play, convert existing
walls, or redirect any of the static helpers. A `lotocc` switch that only registers construction and wall
factories therefore cannot claim complete wall-owner activation. It may be described and tested as
factory/load-only substitution, provided the loaded object's ordinary interfaces and lifetime conform to
§7; this is not an installation contract for the native creation/update paths.

The wall-specific owner dependency is the **lot developer and all three wall operations it invokes**:
add/remove, update the lot's walls, and update surrounding walls. The complete incoming direct-call set
is ClearLot and AddLotFoundation to add/remove and surrounding-update, the slot-62 wrapper to update,
and the helper-to-helper calls shown in §15.1. These are direct calls even when the public entry into the
lot developer was virtual. In particular, replacing only slot 62 leaves foundation creation and clearing
on the native path. The constructor edge at `0x005E30E7`–`0x005E30F5` was rechecked against instruction
bytes: native wall-pool allocation is followed by the native constructor, with no registry lookup.

A replacement lot developer must implement those operations for both loaded and newly created walls,
including removal of the old models, lot-reference ownership and neighbouring updates (§7). It must be
installed as the city's actual lot-developer owner before those calls occur; merely implementing helpers
or registering an occupant factory does not install that owner. If the replacement also supplies lots,
the zone-developer/lot-manager and lot dependency in §15.1 applies as well. This answer does not require
replacing lots just to satisfy the wall-specific dependency, and does not establish how an independently
registered lot developer is installed or restored.

Keeping the native lot developer instead requires a separately proved interception contract for **every**
incoming edge above, including native storage/allocation, calling conventions, retained lot/occupant
references, pre-existing walls and restoration. None is supplied here. Without that contract or the owner
replacement, standalone complete `lotocc` activation with native developers remains unsupported. Restore
factory registrations only after their replacement instances and all outstanding owner/message references
have been drained; changing the registration does not change any surviving object's implementation.

**Worked case — distinguish the two scopes.** Register a counting factory for `0x49C05C9F`, then load one
valid saved wall entry. Expect one factory request for that entry. Independently, with the native lot
developer still installed, add a foundation on a lot for which §7.1 requests a new wall. Expect **zero**
additional requests to that factory: `0x006C8C23` reaches `0x005E3070`, which constructs a native wall.
Clear that lot: `0x006C8A3D` removes its wall and `0x006C8A62` updates neighbours, again without a wall
factory request. This mixed result passes the limited factory/load test and fails a claim of complete
wall replacement. For the owner-replacement case, run the same add/clear/neighbour sequence entirely
through the replacement lot developer and verify §7's models, flags and reference balance for both the
loaded wall and the new wall; no direct native wall helper may remain on that sequence.

### 15.2 Other coupling

- **Class registry**: the loader creates every object by class ID through GZCOM (foundations §2.7). A replacement
  must register all classes of §1 (including the prop subclasses and `cSC4Occupant`) or loading a city fails for
  those classes (foundations §3.5). This is the load path; the direct new-object paths of §15.1 bypass it.
- **Signpost and text-decal embed the prop**: their `cISC4PropOccupant` view at object +4 answers with `this+4`
  (`ADD ECX,4`), so a replacement must either reimplement the prop inline or forward — the two cannot be
  substituted independently.
- **Non-virtual entry points used across classes**: `cSC4TripData::Write/Read` (called from the lot and from
  neighbour connections, `neighbors.md`); `cSC4LotRetainingWallOccupant::AddRetainingWallOccupant`, `UpdateRetainingWalls` and
  `UpdateSurroundingRetainingWalls` (called by the lot developer); `GetPropTypeIndex` and the process-wide prop type
  table (used by every prop). Replacing the lot developer or traffic simulator alone therefore keeps these
  original helpers, or replaces them together.
- **Occupant-manager coupling**: `UpdateDecal` (§10.4) calls `MoveOccupant` (occupant manager slot 24) after
  changing the box; a replacement must call it too or the cell lists go stale.
- **Message subscriptions**: the construction, signpost and decal occupants subscribe through the message server
  slot 0x14 with their `cIGZMessageTarget2` view; a replacement must answer the same five message IDs with the
  same semantics, including the `0x06CDB65B` flag-2 dance shared with the prop (§10.3/§10.4).
- **Shared static RNGs**: the lot file's `0x00B49120` (lot Init, traffic) and the base-texture `0x00B23078`.
- **Spatial lookup**: lots find their occupants through the occupant manager (§3.1); a replacement lot must keep
  the occupants in the original occupant manager or replace both.
- **Raw memory access**: none found from outside these classes for the fields above; outside code uses `cISC4Lot`,
  `cISC4TrafficSource` and `cISC4Occupant` slots *[inferred: not exhaustively searched]*.
- No grid is owned by these classes (simgrid.md §6.4 does not apply).

---

## 16. Quirks and open questions

Quirks (reproduced): vacate reason kept after leaving state 4 (§3.2); uninitialised trip fields saved (§3.3);
lot trip list appended on read (§3.4); lot `Read` sets "initialised" before checking the version, so a rejected lot
skips Init's altitude step (§3.4); retaining-wall mask grows on load (§7); effect props lose their name while
live (§10.2); prop and construction readers accept unknown newer versions (§9, §10.1); u8 map counts (§3.2); the lot
configuration ID is dropped when the configuration is missing (§3.5); the crime-only lot (§14); the text-decal
record writes position twice and stores only the low orientation byte (§10.4, text-decal-occupant.md §7);
the signpost `SetText` skip requires both an equal string *and* an unchanged
capacity pair (§10.3); construction `+0xCC` (record #8) is written as `(+0xF8 != 0)`, i.e. from the live crane
pointer rather than a stored bool (§9.1).

Open questions:

- **Q1** The exact rule for the occupant manager's cell rectangle (§2.3 field 6) belongs to the occupant manager;
  the `floor(x / 64) + 64` approximation misses edge and flora-model cases.
- **Q2** Where do vacate-reason values > 6 come from? Every observed caller passes 0…6 (`UpdateLot` 0…4, tract
  developer 5 and 6); all vacant lots in saves hold 1…6, so the large values were written in an earlier state or
  build. A writer copies the byte. *Answered* in zones.md §8.5.1 *[inferred]* (cited by re-answers11, ZQ16): the
  lot constructor `0x006C1E50` writes the byte at `+0x80` (`0x006C1F19`) but never the vacate reason at `+0x83`, so a
  new lot keeps the byte left in its pool block by the lot that used it before, and a lot that was never vacant
  saves that stale value.
- **Q3** Names of the 19 scalar fields of the construction occupant (§9 #14). Partially decoded in §9.1/§9.2 from
  the state machine (`+0x140…+0x184`, `+0x118`); the remaining pure names and the meaning of `+0x118` are still
  open (filed as OP7 in questions/occupants-props.md).
- **Q4** Meaning of the five `0xFF`-default bytes of a base-texture record (§6) and of the effect prop's two bytes
  and u32 (§10.2).
- **Q5** Who sets lot flag bit 7 (§3.2)?
- **Q6** The automata-controller prop's save layout remains open. Terrain-decal and burnt-building
  layouts are answered in §10.5/§10.6 (SR36-1/SR36-3, re-sr37); both occur in the reference end saves.
- **Q7** Answered statically 2026-10-23: the text-decal serializable receiver is O+16. Its
  extension writes position twice, height then size, RGB, alpha and orientation byte; no decal
  IDs or duplicate lot tag. See text-decal-occupant.md §§1–2,7; archived OP1 and OP2-TD.
- **Q8** TextDecal portion answered in text-decal-occupant.md §7: stream +3C is GZStr and
  receiver +5C is O+6C text. Signpost's independent receiver/field reconciliation remains OP2
  in questions/occupants-props.md; this correction makes no new signpost-owner claim.

---

## 17. Validation and conformance tests

### 17.1 Validation over all saves

587 saves, 417 distinct. For every distinct save, every entry of each class record was split (foundations §3.3),
its CRC checked, parsed with the layouts above, and re-encoded; all references were resolved against the save's
class records.

| Class | Saves | Entries (all saves) | Distinct entries checked | Parsed exactly, re-encoded identically | CRC failures | References / dangling |
|---|---|---|---|---|---|---|
| `cSC4Lot` | 14 | 23,912 | 12,986 | 12,986 | 0 | 13,856 / 0 |
| `cSC4BuildingOccupant` | 14 | 23,129 | 12,295 | 12,295 | 0 | — |
| `cSC4LotBaseTextureOccupant` | 14 | 24,037 | 13,079 | 13,079 | 0 | — |
| `cSC4LotRetainingWallOccupant` | 14 | 2,131 | 1,735 | 1,735 | 0 | 1,735 / 0 |
| `cSC4FoundationOccupant` | 14 | 17,385 | 8,981 | 8,981 | 0 | — |
| `cSC4FloraOccupant` | 568 | 1,059,847 | 744,617 | 744,617 | 0 | — |
| `cSC4PropOccupant` | 15 | 198,136 | 134,420 | 134,420 | 0 | — |
| `cSC4PropOccupantVisualEffect` | 14 | 1,558 | 948 | 948 | 0 | — |
| `cSC4ConstructionOccupant` | 11 | 69 | 31 | 31 | 0 | 227 / 0 |
| `cSC4BusinessUnit` | 14 | 552 | 328 | 328 | 0 | 7,746 / 0 |
| `cSC4ZoneDeveloper` | 587 | 587 | 417 | 417 | 0 | 21,179 / 0 |
| `cSC4Occupant` (bare) | 12 | 172 | 84 | 84 | 0 | — |

Versions seen: exactly the written versions of §1 (and trip version 2). Saves with lots: Konradshohe, Spandau,
Tegel, Fulham, Kensington, Big City Tutorial, Making Money Tutorial, Rush Hour Tutorial; `New City (32)` has props
only; flora is in 568 saves. Map sizes of the zone developer: 64² (157), 128² (250), 256² (10).

### 17.2 Byte-exact round-trip tests

Each test is a real entry from `City - Making Money Tutorial.sc4` (object ID, entry CRC = CRC-32/MPEG-2 over
object ID + payload, foundations §3.4). Required: (a) the reader consumes exactly the payload and yields the
values shown; (b) writing those values back gives the same bytes; (c) the entry header is
`u32 12 + len, u32 crc, u32 objectID`.

**T1 — lot (empty zoned lot)**, ID `0x0DFA3C64`, CRC `0x1C85D610`, 118 bytes:
```
08 00  00 00 00 00  10  49 64 49 64  49 64  4C FE 99 43  80 5F 03 40  80 5F 03 40
01 01 01 01 00 08 00  74 68 25 00  00 00 00 00  CD  00 00 00 00  00 00 00 00  00  00
00 00 00 00 00 00 00 00 00 00 00 00
02 00 01 00 00 00  00 41 0B 8A  00 41 0B 8A  00 00 00 00  02 80 00 00  0C 00 00 00  (24 × 00)
00 00 00 00  4A
```
→ version 8; configuration 0; flags 0x10 (has power); rect (73, 100)–(73, 100); location (73, 100); altitude
307.9867; both slopes 2.0527; size 1×1; facing 1; state 1; condition 0; zone 8; wealth 0; day 2,451,572; building
type 0; vacate 205; no unit; no designate; 0 population maps; empty travel map; travel jobs 0, 0, 0; one property
`0x8A0B4100` = uint16[12] all 0; 0 trips; strategy 74.

**T2 — business unit**, ID `0x10BDD698`, CRC `0x73D2BA7A`, 22 bytes:
`01 00 02 00 00 00 0C 40 DA 13 4A 5D BD C9 44 CA B1 11 4A 5D BD C9` → version 1; lots `0x13DA400C`, `0x11B1CA44`,
both class `0xC9BD5D4A`.

**T3 — flora**, ID `0x00947810`, CRC `0x1600BAD3`, 66 bytes:
```
03 00  04 00 00 00 00 0D 26 89 75 74 57 5F 57 5F  02 00 02 00 00 00 00 00 37 04 3E E8 4A 28 34 65 12 82 0C 2A
12 82 0C 2A  49 5F BE 44 12 C4 98 43 3A 56 FC 44  3C 74 25 00  5A 68 25 00  00  02  00 00 00 00
```
→ base: removal 0, highlight 0, flags 13, type `0x74758926`, cell rect (87, 95, 87, 95), no properties, parent
(`0xE83E0437`, `0x6534284A`, `0x2A0C8212`); flora type `0x2A0C8212`; position (1522.978, 305.532, 2018.695); birth
2,454,588; last seeding 2,451,546; state 0; orientation 2; lot tag 0.

**T4 — prop**, ID `0x009476C4`, CRC `0x892A416D`, 76 bytes:
```
06 00  04 00 00 00 00 05 1E 82 23 A8 57 5B 58 5B  02 00 02 00 00 00 00 00 36 C5 77 C9 4A 28 34 65 00 00 84 29
00 00 84 29  34 23 BF 44 C5 74 99 43 54 EB D8 44 00 20 C1 44 C5 74 9A 43 54 0B D9 44
03 00 00 00 00 64 02 24 FE A5 0A 00
```
→ prop type `0x29840000`; bounds (1529.1, 306.912, 1735.354)–(1545.0, 308.912, 1736.354); orientation 3; state 0;
time (0, 0); no date range; chance 100; usage 2; lot tag `0x0AA5FE24`; timer 0.

**T5 — building**, ID `0x00C3CF20`, CRC `0x9175D64A`, 70 bytes: version 2; base type `0x278128A0`, rect
(84, 79, 84, 79), parent (`0x67BDDF0C`, `0x6534284A`, `0x00730000`); building type `0x00730000`; flags 0; bounds
(1314.113, 306.227, 980.606)–(1320.113, 314.227, 988.606); orientation 2; completion 1.0 (`00 00 80 3F`).

**T6 — base texture**, ID `0x00BF7198`, CRC `0xB4A11718`, 79 bytes: version 2; base type `0x497F6D9D`, rect
(82, 91, 82, 91), parent (0, 0, 0); bounds (1184.1, 306.121, 1728.1)–(1199.9, 306.404, 1743.9); usage 2; 1 record:
`00 30 00 00 4A 6C 00 00 FF B2 33 AA 03 00` (texture `0x3000`, cell (74, 108), orientation 0, byte 0, unnamed
`FF B2 33 AA 03`, frame 0).

**T7 — retaining wall, mask growth**, ID `0x0D901200`, CRC `0x62B30E0F`, 92 bytes: wall type `0xC96D2135`; mask
`09 00 00 00`; hashes `0x00F439E9`, `0x00CFA447`, `0x010DEF29`, `0xE0C042C7` (all non-zero); position (1664,
299.696, 960); size (128, 0.2437, 96); lot `0x11DEAF54`. After `Read` the mask is **15**; the original's next save
writes `0F 00 00 00` at payload offset 40. A conforming writer of an unmodified loaded wall writes 15.

**T8 — foundation**, ID `0x0DA31268`, CRC `0xB39E1107`, 70 bytes: type `0x890B7313`; maximum height 32.0; position
(1656, 304.792, 1560); size (16, 4.1668, 48); usage 2; orientation 2.

### 17.3 Behaviour tests

- **T9** Lot `Read` of a payload whose version is 9: returns false after consuming 2 bytes; the lot is marked
  initialised (a later `Init` keeps altitude 0).
- **T10** Lot `Read` into a lot that already has one trip, payload with 2 trips: the list has 3 trips.
- **T11** Prop `Read` of a version-5 payload (T4 without its last byte, first byte 05): consumes 75 bytes, timer bits
  = `0x0F`.
- **T12** Effect prop `Write` while its effect is live and named `large_dust`: the name is written as
  `00 00 00 00` and the trailing u32 as 0.
- **T13** Lot `Init` after T1: no RNG draw (strategy 74 ≠ `0xFF`); configuration stays null (ID 0); altitude stays
  307.9867. With strategy `0xFF` and the RNG `0x00B49120` in a known state: exactly one `RandomUint32Uniform(100)`.
- **T14** Lot `PostCityInit` with saved flags `0x5B`: `SetFlags(0x5B)` from 0 locks every rect cell in the terrain
  view at the lot altitude (bit 0 changed); with flags `0x50` nothing is locked.
- **T15** Trip `Read` of version 1: no travel-types field is read and it becomes 0.
- **T16** `bytes` read with count `0x000F4240`: no further bytes consumed, the route keeps its previous contents.
- **T17** New lot (not read) with rect (10, 20, 12, 21), terrain altitude 300.0 at (184.0, 336.0) and
  `GetAreaSlope(rect, false)` = 2.5: after `Init`, altitude 300.0, fields 7 and 8 both 2.5, travel jobs 0, 0, 0.
  With terrain altitude 0.0005 instead: altitude stays 0 and no slope query is made (§3.5 `SetAltitude`).
- **T18** `SetFlags(0x01)` from 0 on rect (10, 20, 11, 21), altitude 310.5: the terrain view sees exactly
  `LevelCell(10,20,310.5)`, `(10,21)`, `(11,20)`, `(11,21)`; then `SetFlags(0x00)` gives `RemoveCellLevelling` on the
  same four cells in the same order; `SetFlags(0x03)` from `0x01` makes no terrain-view call.
- **T19** Unit with members [A, B]: `Remove(C)` for a lot C not in the list leaves [A, B] and still calls
  `C.SetUnit(0)` and `C.SetTravelDesignate(0)`; `Add(A)` leaves [A, B] and calls `A.SetUnit(unit)`; `Add(C)` gives
  [A, B, C].
- **T20** Lot read with configuration ID `0x12345678` and never `Init`ed: `Write` emits field 2 as `00 00 00 00`.
  After `Init` with the configuration manager answering that ID: `Write` emits `78 56 34 12`; answering null: 0.
- **T21** `AddRetainingWallOccupant(lot, true)` for a lot with flags bit 0 clear: no wall, no RNG draw, AL = 0.
  With bit 0 set, rect (10, 20, 10, 20), altitude 100.0, edge 0 neighbour row = one cell holding no lot and its
  edge vertices (10, 20) and (11, 20) at 102.4 and 103.6: the cell has no lot, so `bits = 0` and `h[0] = (102·10·20 + 1) + (104·11·20 + 1) = 43,282`; `top = 103.6`; mask bit 0 set; size y = 3.6
  (float); AL = 0.
- **T22** `UpdateRetainingWalls(lot)`, lot flags bit 0 set, wall at the location cell with hashes (5, 0, 7, 9) and mask
  `0b1101`; the edge pass gives (5, 4, 0, 8) and every model build succeeds. Calls in order: edge 1 add(1), edge 2
  remove, edge 3 remove then add. Result: hashes (5, 4, 0, 8), mask `0b1111` (bit 2 stays), position, size and type
  unchanged, the wall is not removed.
- **T23** Same wall, edge pass equal to the stored hashes: no `AddRetainingWallModel` call, `SetEdgeHashes` not
  called, the record is byte-identical. Lot flags bit 0 clear: no occupant query at all. No wall at the location
  cell: exactly one `AddRetainingWallOccupant(lot, true)`.
- **T24** Edge 0 changes from 0 to 6 and the model's vertex setup fails: hash 0 := 6, mask bit 0 cleared; after a
  save and load the mask has bit 0 again (§7 `Read`).
- **T25** `UpdateSurroundingRetainingWalls(B)` with lot C touching only ring row z1 − 1 and lot A touching only
  column x1 − 1 at rows z1..z2 (so C comes first in the zones.md ring order): `UpdateRetainingWalls(C)`, then `(A)`; never `(B)`; each of A and C ends with its reference count
  unchanged.

### 17.4 Native boundary discriminators (LR8/LR9)

These are conformance vectors derived from native bytes, not reported game executions.

- **T26** For a live lot at symbolic address P = `0x10000000`, slot 3 returns `0x10000008`, slot 4 returns
  `0x1000001C`; neither changes the reference count. Null ECX gives null for both. A slot-4 property-holder
  interpretation fails this vector. Through the traffic source iterator, the same slot-4 result is written to
  the source output, with one additional reference acquired through the primary lot at `0x0071084E`.
- **T27** Starting with a destination sentinel and a live lot P, QI with lot IID `0xE818FCED` returns P,
  Serializable IID `0xE4FDA3D4` returns P+4, and IID 1 returns P; each success increments the owner's count once.
  An unsupported IID leaves the sentinel unchanged and returns AL = 0. Property-view QI on P+8 behaves the
  same way after adjusting to P; it does not expose more IIDs.
- **T28** Observe class-factory invocations in two separate scenarios: resolving an unseen serialized lot
  reference uses the class store's GZCOM creation (foundations §2.7), while native zone-developer CreateLot
  follows `0x0072E530` → `0x006BF430` and `0x0072E557` → `0x006C1E50` without a factory lookup.
  Factory-only substitution therefore cannot claim all lots are replacement lots.
- **T29** PickUnitToJoin with exactly one eligible candidate lacking a unit creates one native 0x20-byte
  business unit at `0x006CB45C`, calls Add(candidate), then Add(current lot). No creation reference is acquired
  by PickUnitToJoin. The two member back-pointers retain two references to the unit (§4); a replacement
  factory returning one owned reference must release that extra reference after membership is established.
  With a candidate already in a unit, no constructor is called and only the current lot is added.
- **T30** ClearLot's trace includes `0x006C8A3D` → `0x005E3070(lot, false)` and `0x006C8A62` →
  `0x005E3960(lot)`. AddLotFoundation on its wall-creation branch includes `0x006C8C23` →
  `0x005E3070(lot, true)` and `0x006C8C34` → `0x005E3960(lot)`. Merely registering a replacement wall
  factory changes neither trace. Where edge flags require a new wall, `0x005E30F5` still constructs the
  native wall; the AL = 0 success quirk remains (§7.1).

### 17.5 Native mutation discriminators (LR10/LR11)

These independent vectors follow the cited native branches; they are not game executions. C is the
building profile capacity-vector, P the lot population-vector, k = `0x1010`, j = `0x1020`.
Where a slot-52 result is supplied, it is an explicit collaborator input, not an assumed capacity formula.
For T31–T36, GetBuildingType(true) supplies the capacity profile via configuration fallback, whereas
GetBuildingType(false) is zero: the successful wealth helper sets wealth to zero and does not insert
additional keys. T44 onwards separately exercise a stored building and the helper's insertion effects.

| Vector | Inputs | Required result / order |
|---|---|---|
| T31 | SetPopulation(k, 12); slot 52 returns 100; C=[], P=[] | AL=0; P remains []; no wealth update. Capacity query still occurs before the profile query. |
| T32 | SetPopulation(k, 120); slot 52 returns 100; C=[{k:100},{k:200}], P=[{k:7},{k:9}] | P=[{k:100},{k:9}]; one wealth update; AL=1. The second occurrence is untouched. |
| T33 | SetPopulation(k, 0); slot 52 returns 100; C=[{k:100}], P=[{k:7}] | P=[{k:0}], retaining k; wealth recomputed; AL=1. Repeating the call still recomputes wealth and returns AL=1. |
| T34 | SetPopulation(k, 3); slot 52 returns 100; C=[{j:20},{k:100}], P=[{k:9},{}] | P=[{k:9},{k:3}]; insertion is at the capacity-selected index, irrespective of the existing key in P[0]. |
| T35 | SetPopulation(k, 3); slot 52 returns 0; C=[{k:100}], P=[{}] | P=[{k:0}], AL=1: clamping to zero does not mean failure. |
| T36 | C=[{k:100}], P=[] | Selected P[0] does not exist. Native has no bounds/creation branch; do not turn this invalid invariant into a successful append. |
| T37 | State 3, day D0, wealth 2, P=[{k:7}]; SetState(3,1,6) | No calls, mutations, day update or message; wealth stays 2 and population stays 7. There is no defined boolean return. |
| T38 | No stored building type and no configuration fallback; state 1, wealth 2, P=[{k:7}]; day service returns D; SetState(2,0,6) | State=2, day=D, P unchanged (no rebuild), wealth=0, reason unchanged; power update before copy; data=(1,2,0,0). |
| T39 | Same as T38, but SetState(2,0x100,6) | Whole wealth is nonzero: final wealth=0 and data4=0 (low byte). Recompute occurs before override. |
| T40 | Stored nonzero type B with nonnull purpose-zero profile; state 3, wealth 2, P=[{k:7},{k:9}]; SetState(4,0,0x106) | Every count becomes zero; wealth remains 2; reason=6; no construction-resource release; data=(3,4,B,2). |
| T41 | Old state 2; stored type B with known purpose/native wealth and resource value R; SetState(3,0,0) | Day store precedes ReleaseConstructionResources(R), developer-type lookup, GetCapacity(devType,true), SetPopulation, outer wealth update, power update, copy, post, message Release, copy Release. Do not replace this with a synthetic successful transition. |
| T42 | State 1 → 2, GetBuildingType(true) supplies known B; C=[{k:100,j:50},{k:200}], previous P arbitrary | Rebuild produces P=[{k:0,j:0},{k:0}] without changing C; copy observes that rebuilt P. Original map nodes are destroyed; no per-entry SetPopulation calls. |
| T43 | Snapshot of lot with nonempty P, travel map, trips, runtime tree and configuration Q | P and travel-map entries copy independently; snapshot trips/runtime tree empty; Q gains one constructor reference; travel designate and unit null, neither retained. Subsequent live-lot P/travel-map edits do not alter the snapshot. |
| T44 | Stored residence profile, native wealth 2; C=[{k:100,j:100}], P=[{}], current wealth 1; call wealth update | P becomes [{k:0,j:0}], O={k:0.0,j:0.0}; wealth remains 1. With current wealth 0 or unsupported 3 instead, wealth becomes 2. |
| T45 | Same profile as T44, P=[{k:50,j:50}], current wealth 1 | O={k:0.5,j:0.5}; wealth=2, because descending positive ties favor the higher wealth. With P=[{k:75,j:25}], wealth=1. |
| T46 | Same profile, native wealth 2; C=[{k:100},{k:300,j:300}], P=[{k:100},{k:0,j:300}] | A=[100,300], S=400; O={k:0.25,j:0.75}; wealth=2. Duplicate k contributions are added, not overwritten. |
| T47 | Known stored residence profile, native wealth 2, C=[], P=[], current wealth 3 | Empty O; unsupported current wealth falls back to 2; no divide by zero. Current wealth 0 also becomes 2. |
| T48 | Same known profile but C=[{}], P=[{}] | GetOccupancyByType reaches signed integer division by zero at 006C1258; no successful wealth result. If reached during SetState, preceding state/day mutations have already happened and no copy/post is reached. |
| T49 | Unknown stored nonzero type B absent from catalogue, state 1, wealth 2, nonempty P; SetState(2,0,0) | First profile lookup creates the default profile (purpose=0, native wealth=0, C=[]); rebuild clears P to []; wealth stays 2; data=(1,2,B,2). No construction-resource release. |
| T50 | State 1, no stored/fallback building, wealth 2; SetState(0x101,0,0), day service D | Full argument differs from old byte: state byte still 1 but day becomes D, populations zeroed, wealth becomes 0, message data=(1,1,0,0). This is not the equal-argument no-op. |
| T51 | Native state 3, mixed-occupancy profile enabled, C=[{k:100,j:100}], P=[{k:0,j:25}], requested SetPopulation(k,90) | Slot 52 computes F=0.75, AX=75; assigned k=75, then wealth recomputed. With mixed occupancy disabled, AX=0 and assigned k=0; AL is 1 in both cases. |
| T52 | Old state 2, known stored profile resource count 0, new state 4 | Resource-counter slot 17 receives 0.0f, then slot 30 is called; no population-fill slot-50/55 pair. Every P count becomes zero, reason byte updates, and outer wealth/power/copy/post still follow. |
| T53 | One initialized live lot owns configuration Q and one nonnull property V, with no receiver retaining the queued snapshot; refcounts before call Q=q, V=v | Copy constructor: Q=q+1, V=v+1, copy refs=0. SetState copy AddRef:1; message IGZUnknown assignment:2. After post and local releases: copy refs=1, message refs=1 in queue, Q=q+1, V=v+1. Final message destruction releases copy; copy Shutdown/destruction return Q to q and V to v. Unit/travel-designate refcounts never change; live-lot refcount never changes. |
| T54 | Stored residence profile with native wealth 0, C=[{k:100}], P=[{k:50}] | O={k:0.5}; with initial wealth 0, wealth stays 0 (descending loop has no iterations). With initial wealth 1, its developer type k is present, so wealth stays 1. There is no forced minimum wealth of 1. |
| T55 | Stored unknown type B (default purpose-zero profile), state 4, wealth 1, P=[]; SetState(3,3,0) | State/day update; no rebuild/fill or resource release; computed wealth remains 1, then entering override stores 3; copy wealth=3 and data=(4,3,B,3). With wealth argument 0, copy wealth=1 and data4=1. |
| T56 | Stored residence B, native wealth 2, C=[{k:100,j:100}], P=[{k:0,j:0}], state 2, wealth 1; SetState(3,1,0) | Release resources, GetCapacity(j,true)=100, SetPopulation(j,100) clamps to 100; P=[{k:0,j:100}], nested wealth=2; outer wealth=2; explicit wealth argument 1 ignored within {2,3}; copy wealth=2, data=(2,3,B,2). |
| T57 | No stored/fallback building, state 3, wealth 2, P=[{k:7}]; SetState(4,3,6) | Counts zeroed; computed wealth=0, leaving argument 3 ignored; copy wealth=0 and reason=6, but data4=2. Power bit cleared before copy. Copy contents and message data4 intentionally differ. |

T44–T46 use normal positive capacities; zero capacity denominators retain native x87 nonfinite behavior
rather than a repaired fraction. For any matching C index without a corresponding P map, no portable
successful result is specified: the native unchecked index has no defined container semantics. These
invalid shapes do not justify an inferred map-creation rule. No remaining LR10/LR11 gap is needed for
valid native container invariants; these vectors specify behavior, not a runtime or coverage verdict.

### 17.6 Master-list record observation vectors (LR12)

These are normative shadow-harness cases for §14.1, not measured game runs. A/B/C identify native
objects, not cell coordinates; use T1 as a valid base payload and vary the named fields as needed.

| Vector | Inventory / capture inputs | Required observation |
|---|---|---|
| T58 | Master list [A,B]; A covers four cells, B one; counts before/after = 2 | N=2, two captures in list order, two exact records; A is not counted four times. Iterator unregistered/freed; each successful QI's reference released. |
| T59 | Master [A]; A references unit U and off-list designate C; crime also owns off-list D; occupant manager owns building, construction, wild flora, signpost and an unspecified burnt-building subclass | N=1; capture only A. Preserve U/C reference identity; do not recurse to U/C/D or occupants. Report scope as master lots, never all §1 classes. |
| T60 | Master [A,B]; both refer to U, A designates B, B designates A | N=2; both unit handles equal, designate handles identify B/A respectively; no recursive loop and no extra record or retain from reference traversal. |
| T61 | Available manager, empty sentinel list, counts 0/0; separately repeat with unavailable manager | First: empty 0/0, iterator cleaned up, no record correctness evidence. Second: unavailable/incomplete, not empty/exact. |
| T62 | Master [A,B], counts 2/2; A QI succeeds, Write fails after a partial payload; B compares exactly | N=2, visited=2, full captures=1, exact=1; incomplete/error. Release A's acquired Serializable even on failure. No 1/1 exact claim. |
| T63 | Master [A]; payload otherwise T1, but strategy differs in replacement, or last inline trip field differs, or property's variant type differs | N=1, captures=1, exact=0, mismatch for each independent variation. Neither scalar-only comparison nor skipping nested fields is sufficient. |
| T64 | Counts 2/3; or counts 2/2 but walk yields A twice; or a node's lot is null | Incomplete/error in every variant; record success cannot repair inventory failure. Iterator and all acquired Serializable views still cleaned up. |
| T65 | A's stream reference mapping identifies U, replacement produces a different nonnull unit identity V; separately supply a valid record plus one trailing byte | First: reference mismatch despite equal nullness. Second: exact-consumption failure despite successful Read. Neither is exact. |

### 17.7 Construction reference identity vectors (LR13)

These are normative conformance cases for §9.1.1, not measured game results. Use a valid version-5
construction payload; leave refresh/daily/tick callbacks uncalled throughout the record comparison.
Track stream object identities separately from interface addresses. Reference-count deltas exclude
independent capture pins and references already owned by construction.

| Vector | Inputs / operation | Required result |
|---|---|---|
| T66 | One native effect O is the sole field-12 entry and field 13; capture writer callbacks | Helper inputs O+4 and O+0x10; writer slot 26 receives O+0x10 both times. Class 0x2977AA49, derived §10.2 record. One identity occurs twice; no bare-occupant substitution. |
| T67 | Ground prop P, roof prop Q, crew automaton A; crew bools true,false | Helper inputs exactly P,Q,A; callbacks get each successful Serializable QI result. Read IIDs 0xE9793A65, 0xE9793A65, 0xA9B40F05; bools/order unchanged. One owned field acquisition per successful nonnull read. |
| T68 | One null field-12 entry and null field 13; separately field-12 count=0 | Null callbacks for present nulls, no QI through null or adjustment to 4/0x10. Preserve count=1 with null versus count=0. No null reference acquisition. |
| T69 | Field 13 resolves to bare occupant B, Serializable vtable 0x00AA41BC; B rejects 0x2977AA49; reader slot succeeds, stream error stays zero | Helper false, field null, temporary B acquisition released once. Remaining fields consumed; construction Read can return true. Subsequent Write emits null: mismatch against nonnull B, never exact. No fabricated effect or retained rejected token. |
| T70 | Repeat rejection for a field-12 entry and a crew entry, each count=1 | Both entries remain present with null references; crew bools preserved. Both temporary Serializables released. Read can finish true without stream error, but both lost identities mismatch. No entry silently dropped. |
| T71 | Successful writer Serializable QI; writer slot 26 returns false; repeat with true | Temporary Serializable and writer-view acquisitions each released once in both cases; net helper delta zero. Observer retention requires a separate pin before callback return. Never consume construction's pre-existing reference. |
| T72 | Valid effect O; QI Serializable from O+4 and O+0x10; QI effect from both results | Serializable results both O+0x10; effect results both O. Each QI adds one reference, each matched Release removes one; construction's owned reference keeps O alive. |
| T73 | Capture unchanged construction twice; field 13 has a live effect handle and nonempty name | Identities/membership unchanged; §10.2 writes empty name/zero each time. Temporary string destroyed, no temporary bare occupant from this path, balanced helper QIs. Historical 254/250-byte replay still fails exact comparison; no fixed length adjustment authorized. |

### 17.8 Construction lifetime and observation vectors (LR14)

These are normative cases, not new game measurements. Use lifetime-aware native doubles or
passive traces for destruction cases; never invoke a method through freed game storage.

| Vector | Inputs / operation | Required result |
|---|---|---|
| T74 | Live field-13 effect, construction sole owner; successful helper QI; callback acquires one pin; helper and observer clean up | Counts 1→2→3→2→1. Same effect identity throughout; no bare occupant created and no last-reference destruction. Repeat callback false: same releases. |
| T75 | As T74, but construction naturally shuts down after callback returns and before pin cleanup; no other effect owners remain | Construction's release leaves only observer pin; cleanup reaches zero legitimately. Record the intervening Shutdown. Zero alone is not proof of an observer over-release or a native temporary. |
| T76 | Retain construction allocation; naturally run initialized Shutdown with manager available and field-13 effect's last owners released | Manager receives O+0xC before owner's Release. Field +0x13C remains O; it is no longer an owned reference. Trace base teardown installing AA41BC at O+0x10. Exclude instance from live Write census; do not dereference stale O or acquire a pin from it. |
| T77 | Two observations: valid live bare Serializable supplied as field-13 read token; separately a raw stale field value after T76 | First follows T69: rejection/null, temporary release, mismatch. Second is incomplete lifetime observation with no QI/Read/Release on stale storage; not an alternative valid bare-effect encoding. |
| T78 | Original Write/helper/slot-26 trace includes construction/object/view addresses and lifetime events; compare with trace having only class ID, 254/250 bytes and final release zero | First can distinguish view framing, intervening teardown and pin lifetime using §9.1.2. Second is insufficient to attribute the crash or require bare serialization. Preserve mismatch and later exception as separate observations. |
| T79 | Invoke only known-live GetPosition/GetBoundingBox/GetGZCLSID; separately passively observe Draw and DoAnimationTick(0) | Getter results match §9.2/class identity without schedule advancement. Draw/tick may change caches or state; do not invoke them as pure probes or credit them from a Write comparison. |

### 17.9 Separate unit/texture/wall discovery vectors (LR15)

These are behavioural fixtures for §14.2, not measured real-city results. Use known-live native
interfaces at a synchronous boundary; keep discovery, capture and exact counts separate.

| Test | Setup / stimulus | Required result |
|---|---|---|
| T80 | Complete master walk of A/B/C with GetUnit = X/X/null; X members A/B/D; unrelated empty unit Y | U=1, one X payload, ordered A/B/D reference identities. No capture of Y or added lot D. Three getter calls acquire no references. One successful X Serializable QI and one Release balance; class 0x4A232DA8. Result explicitly describes master-rooted units, not all allocated units. |
| T81 | Available master count=0; repeat with count=3 and all unit getters null; repeat with iterator failure after discovering X | First two are valid empty U=0 with distinct master counts. Third is incomplete despite X's exact capture; never report a complete 1/1 census. |
| T82 | Valid grid: texture P spans two cells; texture Q only in padding cell (0,191) outside every lot; wall R spans two cells; no other target occupants | Two slot-39 full-grid queries, min=(0,0), max=(191,191), types 0x497F6D9D / 0x895D1169, maxCount=−1: B=2, W=1. P/R each occur once; Q is included. Null corners or lot BBox queries are not equivalent evidence. |
| T83 | Initialized grid with no selected type; repeat not initialized; repeat initialized with maxCount=0 | First returns true, empty list, valid empty 0/0. Others return false, untouched empty list, unavailable/invalid query rather than exact empty. Successful empty supplies no nonempty evidence. |
| T84 | Caller preloads one list node; query appends two matches; separately repeat on a fresh empty list | Native call preserves preloaded node and appends two, acquiring exactly two references. Observation rejects the nonempty initial list. Fresh-list census counts two, not three. Slot 48 releases each node-owned reference and clears nodes; caller frees sentinel with A. |
| T85 | One query result with a node-owned occupant reference; Serializable QI succeeds; force Write failure, reader rejection and comparison mismatch in separate trials | Each trial releases QI once, then slot 48 releases node reference once, then sentinel is freed. No Release of city-manager getter or borrowed node value. All failures remain in selected denominator; no cleanup-dependent exact result. |
| T86 | Full grid has three walls; query with finite maxCount=1; repeat with unlimited; inject duplicate/null result into a separate malformed fixture | Finite call can return true with one result but is not complete evidence. Unlimited yields three distinct walls. Malformed duplicate/null observation is an explicit inventory error, never a deduplicated exact verdict. |
| T87 | Target type selected but Serializable class differs, or QI fails; separately unregistered target allocation exists | Selected unsupported/unqueryable object remains error in B/W; do not decode as base class or skip it. Unregistered allocation is outside registered-grid scope and cannot be credited by that census. |
| T88 | Selected wall writes mask=0, hashes=(1,0,0,0), otherwise valid record; independently select a base texture with nondefault unnamed/frame bytes | Wall Read ORs mask to 1 (§7), producing a mismatch even after full successful consumption; no mask normalization. Texture comparison preserves all 14 record bytes, including unnamed/frame bytes, without randomizing saved frames. |

### 17.10 Normal road-access predicate and live-state vectors (LR16; replace-lot29 LR8)

These are statically derived behavioural fixtures for §13.2.5, not measured house-run results. Unless a row
says otherwise, use a non-ferry lot in zone 1, facing edge 0, an empty road-access cache and exactly one
occupant in that edge's outside-cell rectangle. Its QI for `0xA821EF94` succeeds. Each fresh trial starts
from that setup. Preserve the saved lot fields and occupant flags throughout the road-only operations.

| Test | Setup / stimulus | Required result |
|---|---|---|
| T89 | Call lot slot 28 with network flags `0x1`; repeat fresh with `0x8`, `0x40`, `0x400` and `0x200001` | Each returns true and stores key 0=true. Each invokes network slot 9 (+0x24) once with mask `0x449`, never slot 8. In particular, `0x1 & 0x449` is `0x1`: this is a positive direct-access case even though the all-bits predicate is false. Extra flag `0x200000` does not exclude a normal-path occupant. |
| T90 | Fresh trials with flags `0`, `0x2`, `0x200000` and `0x449` | First three return false/store key 0=false; the last returns true/stores key 0=true. Every case uses slot 9. Testing only `0x449` would fail to discriminate slots 8 and 9. |
| T91 | Obtain key 0=true with flags `0x1`; change the fixture's network flags to `0` without notifying the lot; call slot 28, then slot 30, then slot 28 | First slot 28 returns cached true without a flag query. Slot 30 recomputes through network slot 9 and overwrites key 0=false. Last slot 28 returns false without another flag query. The fixture's cache has no nonzero purpose keys. |
| T92 | Fresh lot with flags `0x1`; call slot 29(5) | Returns true from direct access and stores key 0=true; it neither queries neighbouring lots nor creates purpose key 5. Direct success short-circuits any purpose-specific path. |
| T93 | Same edge/occupant layout, but QI fails; separately, return two occupants whose QIs succeed, with flags `0` then `0x40` | Failed-QI trial returns false with no network-flag call. Ordered two-occupant trial calls slot 9 twice and stops at the second occupant, returning true; references acquired for the query/interfaces are balanced. |

**Worked travel case.** Use T89's flags `0x1` with a developed residential lot (state 3, profile purpose 1),
job access true, a nonzero trip-length byte and travel counter `0x8999B10B` initially 0. Keep all other
`UpdateLot` tests passing. Three successive `TestTravel` visits must each pass, leave the counter at 0 and
produce no reason-4 state transition. As a negative comparison, supplying false road access with all other
inputs held equal gives counter values 1, 2, 3 and `TestTravel` results true, true, false; on the third visit
`UpdateLot` changes state 3 to 4 with reason 4. This distinguishes a road predicate defect from a failing
commute map without asserting any numeric population, tax or city-ending result. The lifetime of this
counter and the zot side effects remain those of lot-developer.md §5.4.
