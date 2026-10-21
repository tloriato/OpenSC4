# Spec: the prop manager (`cSC4PropManager`) and the lot configuration manager (`cSC4LotConfigurationManager`)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from observing the original program and its data files. Implementers must work from this spec, the
public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled or disassembled
code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose.

Related specs: [foundations.md](foundations.md) (§2.7 references, §3.6 components, §3.7 read before `Init`, §5
messages, §17.2 property holders), [data-formats.md](data-formats.md) (exemplars, property types),
[prop-developer.md](prop-developer.md) (§4.4 prop selection through slots 8, 9 and 13; §4.6 timed props and the
timer properties), [lot-developer.md](lot-developer.md) (§6.5 lot props through prop manager slot 26 and lot
configuration manager slot 27; §15.2 allocator rows; §17.1 the 0x34-byte lot configuration object record),
[building-development.md](building-development.md) (§1.3 the static catalogue pattern and the shutdown callback;
§4.7 slots 15, 18, 20 and 40 that this spec calls), [city-detail.md](city-detail.md) (§2 the detail preference),
[flora.md](flora.md) (the flora simulator's type and family queries), [tract-developer.md](tract-developer.md) and
[zones.md](zones.md) (callers of the lot configuration queries).

## 0. Summary

Two city components hold **catalogues** built from the game's exemplar files:

- The **prop manager** (component 5) knows every prop type (prop exemplars, `ExemplarType` 0x1E): its resource
  key, a 0x1C-byte **prop profile** and the **prop families** it belongs to. It creates prop occupants from a type
  ID, and it inserts, finds and removes props in the city's occupant manager, sorted by a **usage** (city, lot,
  building, street). It also keeps the per-building prop list that a building holds as a property. The catalogue
  is built **once per process**. The timed-prop behaviour belongs to the prop developer (prop-developer.md §4.6),
  not here.
- The **lot configuration manager** (component 9) holds every **lot configuration** (exemplar group
  `0xA8FBD372`), indexed by ID, with a set of the building families they mention, cached filter searches and
  helper queries for the developers: by family, by size, by filter, "does this configuration hold building type
  T", the largest footprint of a family list, and the expansion of a configuration object's ID list into types.
  The catalogue is built at the first city `Init` and **rebuilt whenever the city detail preference has changed**
  since the last build; each configuration then keeps only the objects allowed at that detail level (§B3, the
  answer to CD1).

Both save only three or fewer references to other components; the catalogues are never saved.

Implementation size: about 300 lines for the prop manager, 350 for the lot configuration manager (without the
lot configuration class itself), 60 for the records.

---

# Part A: the prop manager

## A1. Identity

| Item | Value |
|---|---|
| Class ID (`GetGZCLSID`, `0x004A1D20`) | `0xA990BD10` |
| Component | city component **5**, IID `0x28326F3E` (foundations §3.6); global `0x00B43D10` |
| Factory | `0x004A28E0`: GZ allocator, **0x28** bytes, every field zero |
| Interface at +0 | `cISC4PropManager` (SDK header), vtable `0x00A8F928` (41 slots, §A9) |
| Message target at +4 | vtable `0x00A8F918`, `DoMessage` `0x004A34E0` |
| Serializable at +8 | vtable `0x00A8F900`, `Write` `0x004A2430`, `Read` `0x004A2470` |

`QueryInterface` (`0x004A1D30`): `0x28326F3E` → +0; `1` and `cIGZMessageTarget2` `0x452294AA` → +4;
`cIGZSerializable` `0xE4FDA3D4` → +8; otherwise false. Reference count at +0x10. The last `Release`
(`0x004A2340`) calls `Shutdown` (slot 4) and then the deleting destructor (slot 40).

## A2. State

| Offset | Field | Set by | Saved |
|---|---|---|---|
| +0x0C | initialised flag (u8) | `Init` / `Shutdown` | no |
| +0x14 | occupant manager | `Read`, else `Init` (global `0x00B43D0C`) | **ref** |
| +0x18 | resource manager | `Init` (global `0x00B43CA8`) | no |
| +0x1C | occupant filter used by every query: a 0x14-byte object (vtables `0x00A8F8E4`, `0x00A8F8CC`) whose u32 at +0xC is the **usage** to match | `Init` (new, usage 0) | no |
| +0x20 | lot manager | `Read`, else `Init` (global `0x00B43D08`) | **ref** |
| +0x24 | the 3D view's view filter | `Init`, message `0x06CDB65B` | no |

Each pointer field holds one reference. **Usage** values (prop occupant slots 22/23 `Get/SetPropUsage`):
1 city, 2 lot, 3 building, 4 street.

**Process-wide catalogue** (static, shared by every prop manager instance):

| Global | Content |
|---|---|
| `0x00B21760` | type map: prop type ID → resource key (type, group, instance) |
| `0x00B21774` | family map: family ID → vector of prop type IDs |
| `0x00B2179C` | profile map: prop type ID → prop profile (§A3.2) |
| (unnamed) | name map: prop type ID → string, never filled (§A5 slot 12) |
| `0x00B21758` | "catalogue built" flag (u8) |
| `0x00B21770` | the type map's element count (`0x00B21760` + 0x10), sent as data2 with the catalogue message (§A3.4; PL2) |

The three maps are STLport hash tables keyed by the ID; their iteration order is bucket order (foundations).

## A3. The prop catalogue *[compat]*

### A3.1 Loading (`StaticInit`, `0x004A4310`)

Run from the first `Init` in the process (§A4.1), never again. When the resource manager exists:

1. Ask it (slot 18) for every resource of type `0x6534284A` (exemplars) and visit each key in the order the
   resource manager lists them.
2. Load the exemplar (resource manager slot 3, IID `0xA52160F5`). If its `ExemplarType` (`0x10`) is **0x1E**, add
   it (`AddPropExemplarData`, `0x004A4210`):
   - the prop type ID is the key's **instance**;
   - type map[instance] := the key (a later exemplar with the same instance replaces the key);
   - read the profile and the families (§A3.2, §A3.3).
3. Afterwards, post the catalogue message (§A3.4), even when there is no resource manager.

The catalogue is torn down by the shutdown callback `0x004A2A30`, registered with the application (global
`0x00B43C94` slot 69) when the catalogue is built (building-development.md §1.3 describes the same pattern).

### A3.2 The prop profile (0x1C bytes, `0x004A3DB0`)

| Offset | Content | Exemplar property | Default |
|---|---|---|---|
| +0x00 | occupant class ID used by `CreateProp` | `0xA977A86B` | `0x2977AA47` |
| +0x04, +0x08, +0x0C | size x, y, z (floats) | `0x27812810` (OccupantSize) | 0, 0, 0 |
| +0x10 | requester satisfaction (u32) | `0x49A1E05A` | 0 |
| +0x14 | wealth (u8; the other three bytes of the dword are not written) | `0xE9A316EB` | 0 |
| +0x18 | crime effect (u32) | `0x8AA864D5` | 0 |

The values are copied in one piece into profile map[ID] (inserting it when absent). The size property is copied
value by value for **as many values as it has**, without a bound *[quirk]*. Every shipped prop exemplar has three.
The exemplar name (property `0x20`) is read and thrown away. Prop developer §4.4 reads +0x10, +0x14 and +0x18
through slot 13.

**Order of the reads** (PL8; `0x004A3DB0`–`0x004A3E74`). The profile is built in a local copy, in this order:

1. **Size.** OccupantSize present: its element count (property value slot 4) and raw data pointer (slot 111) are
   read, and the `count` dwords are copied to +0x04, +0x08, … with no bound and no type check. Absent: +0x04,
   +0x08 and +0x0C are set to 0. A present size with fewer than three values leaves the remaining size dwords
   **unwritten** (whatever the stack held) *[quirk]*; a scalar value has count 0, so all three stay unwritten.
2. **Class**: +0x00 := `0x2977AA47`, then the `0xA977A86B` read over it.
3. **Satisfaction**: +0x10 := 0, then the `0x49A1E05A` read.
4. **Wealth**: the low byte of +0x14 := 0, then the `0xE9A316EB` read (one byte).
5. **Crime**: +0x18 := 0, then the `0x8AA864D5` read.

Because steps 3–5 come after the size copy, extra OccupantSize values are mostly overwritten again: value 4 (into
+0x10) and value 6 (into +0x18) have no effect; value 5 survives only in the **upper three bytes** of +0x14. The
local copy is the last 0x1C bytes of the reader's frame, so a seventh value overwrites the reader's **return
address** and an eighth and ninth its arguments (exemplar, ID): the original crashes on return *[quirk]*.
*[choice]* FreeCity4 copies at most three values, sets the size dwords the property does not give to 0, and keeps
the upper bytes of +0x14 at 0. Unreachable with shipped data.

### A3.3 Families

For each value `f` of the exemplar's family property `0x27812870`, in order: create an empty entry for `f` in the
family map when absent, then append the prop type ID to `f`'s vector **unless it is already in it**. A family's
vector is therefore in the order its members were loaded. The loader has a "replace" mode (remove the old profile
and the ID from every family first), but the only caller always passes "add" *[compat: the replace branch is never
taken]*.

### A3.4 `kSC4MessagePropExemplarDataUpdated` (`0x896FDC70`, `0x004A2730`)

Posted (message server slot 4, `Post`) once, at the end of `StaticInit`, when the message server exists: a
standard message with void1 = the prop manager, the `cIGZUnknown` field = the prop manager, data2 = the value at
`0x00B21770` (read at `0x004A2780`, set with message slot `+0x40` `SetData2`). That address is not a variable of
its own. It is the element count of the type map, an STLport hash table at `0x00B21760` with its bucket vector at
`+4` and its count at `+0x10`. The static constructor `0x00A71EA0` zeroes the count (`0x00829C1E`), and every insert
(`find_or_insert` `0x004A2930`, `+0x10` incremented at `0x004A2997`) and erase updates it. So **data2 = the number of distinct prop type IDs in the catalogue**, that is, the number of
distinct instances among the `ExemplarType` 0x1E exemplars, counted once even when a later exemplar replaces an
earlier key (PL2).

## A4. Lifetime, messages and cadence

### A4.1 `Init` (slot 3, `0x004A4380`) → true

Only when the initialised flag is clear: set it; build the catalogue if this is the first time in the process
(§A3.1); register the command `0x0BEAF1A2` with `0x00B43CB0` (slot 6); subscribe the message target (+4) to
`0xA6B79602`, `0x06CDB65B`, `0x26D31EC1` and `0x0BEAF1A2`; fill each null field (read before `Init` keeps the
loaded ones, foundations §3.7): occupant manager, resource manager, a new occupant filter, lot manager (each with a
new reference). If the view filter is null, ask the 3D view (global `0x00B43DC4`, slot 18) for one and keep it
when non-null. The 3D view pointer itself is not tested *[quirk]*.

### A4.2 `Shutdown` (slot 4, `0x004A2370`) → true

Only when initialised: clear the flag; release and null the view filter, lot manager, filter, resource manager and
occupant manager; unsubscribe the four messages; unregister the command `0x0BEAF1A2` (`0x00B43CB0` slot 9). The
catalogue stays.

### A4.3 Messages (`DoMessage`, sub-object +4; every handler returns true)

| Message | Handling |
|---|---|
| `0x06CDB65B` `kSC4MessagePostViewFilterChange` | void2 points to the new view filter pointer; when it differs from +0x24, take a reference to the new one, release the old one and store it |
| `0x0BEAF1A2` `kCommandID_CreateTextDecal` (a debug command) | needs more than 2 arguments: text (argument 0), x (1), z (2), optional argument 3 (default 1.0) and, only with more than 7 arguments, arguments 4–7 (default 1.0 each). Creates a text decal occupant (`0x005F8300`, 0xBC bytes), adds it with this manager's slot 19 (usage 1), places it at (x, terrain altitude at (x, z) from `cISTETerrain` slot 13, z), sets the text (decal slot 4 `SetText`), then the colour from arguments 4–6 (slot 8 `SetTextColor(r, g, b)`), the alpha from argument 7 (slot 10 `SetAlpha`) and the text height from argument 3 (slot 6 `SetTextHeight`). See the setters below (PL1). The handler's own reference is never released *[quirk]* |
| `0xA6B79602` `kSC4MessageViewZRChange3` | reads the message data and does nothing |
| `0x26D31EC1` `kSC4MessagePostCityInit` | subscribed, ignored |

The text decal setters used above (primary vtable `0x00AA5680`; PL1). They are debug-only here; the label tool
uses the same occupant.

| Slot | Setter | Behaviour |
|---|---|---|
| 6 `+0x18` | `SetTextHeight(float h)` (`0x005F7B10`) | accepted only when 0 < h < 8.0 (`0x00A81054`, `0x00A8E178`); stores `+0x80` and rebuilds the decal (`UpdateDecal` `0x005F7810`); otherwise ignored |
| 8 `+0x20` | `SetTextColor(float r, float g, float b)` (`0x005F7B50`, three floats by value) | ignored when every component is within 1e-4 (`0x00AA54A0`) of the stored colour (`+0x9C`); otherwise stores it and, when the decal is shown (`+0x8C`), passes it to each of its two overlay decals (`+0x84`/`+0xB4`, `+0x88`/`+0xB8`; overlay slot `+0x48`) |
| 10 `+0x28` | `SetAlpha(float a)` (`0x005F7C00`) | ignored when equal to `+0xA8`; otherwise stores it and passes it to both overlay decals (overlay slot `+0x44`), or rebuilds the decal when it is not shown |

So the command's defaults give a white, opaque decal one unit high.

No agents, no simulation cadence, no random numbers.

## A5. Behaviour of the catalogue queries (slots 5–14)

| Slot | Behaviour |
|---|---|
| 5 `GetAllPropTypes(vec)` | clears `vec`, then appends every type map key in bucket order |
| 6 `GetAllPropFamilyTypes(vec)` | clears `vec`, then appends every family map key in bucket order |
| 7 `GetPropKeyFromType(id, key&)` | type map hit → copy the key, true; else false, `key` untouched |
| 8 `IsPropTypeValid(id)` | `id` is in the type map |
| 9 `GetPropTypesByFamily(vec, id)` | family map hit → `vec :=` a copy of the family's vector (**assigns**), returns its length. Otherwise, type map hit → `vec := [id]` (assigns) and returns **0** *[quirk]*. Otherwise returns 0 and leaves `vec` untouched |
| 10 `IsPropInFamily(type, family)` | true when `type = family`, or when `family` is in the family map and `type` is in its vector |
| 11 `IsPropFamilyTypeValid(id)` | `id` is in the family map **or** in the type map |
| 12 `GetPropName(id)` | returns the name map entry, inserting an empty string; the map is never filled, so the result is always empty *[quirk]* |
| 13 `GetPropProfile(id)` | returns profile map[id], **inserting an all-zero profile** for an unknown ID *[quirk]* |
| 14 `GetPropProfilePtr(id)` | the profile or null |

After slot 13 on an unknown ID, slot 14 finds the zero profile while slot 8 is still false.

## A6. Creating props

### A6.1 `CreateProp(id, prop&)` (slot 15, `0x004A3040`, pop 8, AL)

1. Slot 8 false → false (`prop` untouched).
2. `class :=` profile +0x00 (slot 13), or `0x2977AA47` when that is 0.
3. Ask GZCOM for an instance of `class` with IID `0xE9793A65` (`cISC4PropOccupant`). If none exists, build one of
   the three built-in classes: `0x2977AA48` terrain decal prop (0xA0 bytes), `0x2977AA49` visual effect prop
   (0x94 bytes), anything else a plain prop occupant (0x68 bytes).
4. Prop slot 8 `SetPropType(id)`.
5. Attach the exemplar and size the prop (`0x004A2A70`): through the prop's property holder (occupant slot 6),
   ask for interface `0x0AC2B5F7`; look `id` up in the type map; load the exemplar (resource manager +0x18,
   slot 3) and hand it to that interface (slot 3) so that the prop reads its properties from it. Then
   `SetBoundingBox` (prop slot 4) from the origin to the OccupantSize vector, or to (1, 1, 1) when the prop has no
   size. Any missing piece (no interface, unknown type, failed load) skips the rest. This step always succeeds.
   The ID looked up is the helper's second argument, or the prop's own type (prop slot 7) when that is 0.
   **The size read** (PL9, `0x004A2B41`–`0x004A2BBE`): when the prop's property holder (now backed by the
   exemplar, so an inherited value counts) has OccupantSize, the box's far corner is the property value's **raw
   data pointer** (variant slot 111), passed as the address of three floats, with no count or type check; the near
   corner is (0, 0, 0). So, as for buildings (building-development §4.9.1, DG32):

   | OccupantSize | Original's box |
   |---|---|
   | absent | (0,0,0)–(1,1,1) |
   | Float32 array of 3 or more | the first three values |
   | Float32 array of 1 or 2 | the missing values are read **past the property's heap block** (garbage) |
   | array of 0 values, or a scalar | a null or tiny block, or the scalar's own bits used as an address: garbage or a fault |
   | an integer array | its bit patterns read as floats (no conversion) |

   *[choice]* FreeCity4 uses 0 for each missing value and (1, 1, 1) for an empty array or a scalar; integer values
   are converted. Every shipped prop exemplar has a Float32 array of three, so only plugins can tell.
6. True; the caller owns one reference.

### A6.2 `CreateGenericProp(key, prop&)` (slot 16, `0x004A24D0`, pop 8, AL)

Loads the exemplar at `key` through the **global** resource manager (false when that fails), picks the class from
its `0xA977A86B` exactly as in A6.1 step 2–3, attaches the exemplar and sets the same bounding box. It does **not**
set the prop type and does not touch the catalogue. True.

## A7. Props in the city

### A7.1 Adding

**`AddCityProp(occ)`** (slot 19), **`AddLotProp(lot, occ)`** (slot 25): with usage 1 and 2. Null occupant manager
→ false. If the occupant already has flag 1 (in the occupant manager) → true and nothing else (the usage is **not**
changed). Otherwise occupant `Init` (slot 3), then occupant manager slot 20 `InsertOccupant(occ, false)`; both must
succeed; then set the usage (query `0xE9793A65`, prop slot 23) and return true. Slot 25 ignores its `lot` argument
*[quirk]*.

**`AddCityProp(id, pos, orient)`** (slot 20, pop 12), **`AddLotProp(id, pos, orient)`** (slot 26, pop 12),
**`AddStreetProp(id, pos, orient)`** (slot 32, pop 12): usage 1, 2 and 4 (`0x004A2010`):

1. `CreateProp(id)` (slot 15); false → return null.
2. Occupant slot 9 `SetPosition(pos)`, prop slot 10 `SetOrientation(orient)`.
3. Occupant `Init`; then occupant manager slot 20 `InsertOccupant(occ, false)`. The occupant manager pointer is
   **not** tested here *[quirk]*.
4. Only when both succeed: set the usage.
5. Release the creation reference. Return the occupant on success (the caller receives no reference of its own;
   the occupant manager holds the prop), else null.

**`AddBuildingProp(building, prop)`** (slot 36, `0x004A3B90`, pop 8, AL):

1. Null occupant manager → false.
2. If the prop lacks flag 1: occupant `Init` and `InsertOccupant` must succeed (else false), then usage 3.
3. From here the result is true. Through the building's property holder (occupant slot 6), get property
   `0xA8322B7F` (holder slot 6): a property object holding a vector of prop pointers. Append the prop unless
   already present. When the building has no such property, create one (0x14 bytes, vtable `0x00A8F9CC`) holding
   just the prop and add it (holder slot 11 `AddProperty(0xA8322B7F, obj, false)`).
4. Set usage 3 (again).

The vector stores plain pointers; it takes no reference *[compat]*.

**A building without a property holder** (PL10, `0x004A3BEB`–`0x004A3BF6`): when occupant slot 6 returns null,
the slot returns **true** at once, skipping step 3 **and step 4**. So the prop's usage is 3 only if step 2 ran
(the prop was not yet inserted); a prop that was already inserted keeps whatever usage it had *[quirk]*. Game
buildings always have a holder, so this is reachable only with a foreign occupant.

### A7.2 Queries

Every query sets the shared filter's usage first and appends to the caller's list (each element with one new
reference, owned by the caller). A null occupant manager gives false.

| Slot | Query |
|---|---|
| 17 `GetCityProps(list, SC4Rect<float>)` | usage 1; occupant manager slot 36 with the box from (x1, −FLT_MAX, z1) to (x2, +FLT_MAX, z2), filter, mask `0xFFFFFFFF` |
| 18 `GetCityProps(list, SC4Rect<int32>)` | usage 1; occupant manager slot 40 (cell rectangle), filter, mask `0xFFFFFFFF` |
| 23 `GetLotProps(list, x, z)` | lot manager null → false; the lot at cell (x, z) (lot manager slot 6, `GetLot(x, z, false)`); null → false; else slot 24 |
| 24 `GetLotProps(list, lot)` | see below |
| 30 `GetStreetProps(list, SC4Rect<int32>)` | usage 4; occupant manager slot 40 |
| 31 `GetStreetProps(list, x, z)` | see below |
| 34 `GetBuildingProps(list, x, z)` | the building at the cell (occupant manager slot 33 with type `0x278128A0`); none → false; else slot 35, then release the building |
| 35 `GetBuildingProps(list, building)` | no property holder or no property `0xA8322B7F` → false; else append every prop of the vector, in order, with a reference each; true |

**`GetLotProps(list, lot)`** (slot 24, `0x004A3240`, pop 8): null lot or occupant manager → false. Take the lot's
cell rectangle (lot slot 14), usage 2, and run occupant manager slot 40 on it; when that returns false, return
false. Convert cell (x1, z1) and cell (x2 + 1, z2 + 1) to world positions (city slot 96): `(wx1, wz1)`,
`(wx2, wz2)`. Then walk the **whole list** (including elements that were there before the call) and remove and
release every element whose position (occupant slot 8) is not inside `wx1 ≤ px < wx2` and `wz1 ≤ pz < wz2`
(a NaN coordinate fails). True.

**`GetStreetProps(list, x, z)`** (slot 31, `0x004A3150`, pop 12): usage 4, occupant manager slot 40 on the
single cell (x, z). When the query succeeds, walk the whole list and remove and release every element whose
position maps (city slot 95) to a cell other than (x, z). It **always returns false**, even after filling the list
*[quirk]*.

### A7.3 Removing

**Removing one prop** (`0x004A2690`): if the occupant has flag 2 (shown), take its placeable object (occupant slot
19), call its slot 11, clear flag 2 (occupant slot 31) and drop the placeable; then, when the occupant manager
exists and flag 1 is set, occupant manager slot 21 `RemoveOccupant(occ, 0, 0)`; finally occupant `Shutdown`
(slot 4).

| Slot | Behaviour |
|---|---|
| 21 `RemoveCityProps(SC4Rect<float>)` | slot 17 into a fresh list, then remove each element and release it; true |
| 22 `RemoveCityProps(SC4Rect<int32>)` | the same with slot 18; true |
| 27 `RemoveLotProps(x, z)` | the same with slot 23; true |
| 28 `RemoveLotProps(lot)` | the same with slot 24; true |
| 29 `RemoveProp(occ)` (SDK `RemovePropA`) | remove one prop; true |
| 33 `RemoveStreetPropsFromTile(x, z)` | slot 31 into a fresh list, remove and release each; no result |
| 37 `RemoveBuildingProps(x, z)` | the building at the cell as in slot 34, then slot 38 |
| 38 `RemoveBuildingProps(building)` | no holder or no property → false. For each prop of the vector: when the occupant manager exists and flag 1 is set, `RemoveOccupant`; then occupant `Shutdown`. Empty the vector, then remove the property (holder slot 17, `RemoveProperty(0xA8322B7F, false)`); true. The placeable is **not** hidden here, unlike "removing one prop" |
| 39 `RemoveBuildingProp(building, prop)` | find the prop in the building's vector: `RemoveOccupant` (when flag 1), `Shutdown`, erase it; when the vector is then empty, remove the property. Returns whether it was found |

## A8. Persistence *[compat]*

Record type `0xA990BD10`, version **1**:

| Field | Type |
|---|---|
| version | u16 = 1 |
| occupant manager | reference (foundations §2.7; read with IID `0xC6BB8334`) |
| lot manager | reference (read with IID `0x681905BD`) |

18 bytes. `Write` (`0x004A2430`) and `Read` (`0x004A2470`) return true when the stream has no error. `Read`
rejects any other version (false). All 421 distinct saves in the sample corpus have this 18-byte record, the
references naming classes `0x098F964D` and `0x498F9B01`.

## A9. Interface slot table (`cISC4PropManager`, vtable `0x00A8F928`) *[compat]*

The game's vtable lists each SDK overload pair in **reverse** declaration order (as for the other MSVC-built
interfaces); the names below are the SDK's, matched to the code.

| Slot | Offset | Method | Pop | Return | Address |
|---|---|---|---|---|---|
| 0 | +0x00 | `QueryInterface(iid, void**)` | 8 | AL | `0x004A1D30` |
| 1 | +0x04 | `AddRef()` | 0 | EAX | `0x004F7BA0` (shared) |
| 2 | +0x08 | `Release()` | 0 | EAX | `0x004A2340` |
| 3 | +0x0C | `Init()` | 0 | AL = 1 | `0x004A4380` |
| 4 | +0x10 | `Shutdown()` | 0 | AL = 1 | `0x004A2370` |
| 5 | +0x14 | `GetAllPropTypes(vec&)` | 4 | — | `0x004A2BF0` |
| 6 | +0x18 | `GetAllPropFamilyTypes(vec&)` | 4 | — | `0x004A2CB0` |
| 7 | +0x1C | `GetPropKeyFromType(id, key&)` | 8 | AL | `0x004A2D70` |
| 8 | +0x20 | `IsPropTypeValid(id)` | 4 | AL | `0x004A2DD0` |
| 9 | +0x24 | `GetPropTypesByFamily(vec&, id)` | 8 | EAX count | `0x004A2E90` |
| 10 | +0x28 | `IsPropInFamily(type, family)` | 8 | AL | `0x004A2F80` |
| 11 | +0x2C | `IsPropFamilyTypeValid(id)` | 4 | AL | `0x004A2E10` |
| 12 | +0x30 | `GetPropName(id)` | 4 | EAX string | `0x004A4360` |
| 13 | +0x34 | `GetPropProfile(id)` | 4 | EAX profile& | `0x004A3750` |
| 14 | +0x38 | `GetPropProfilePtr(id)` | 4 | EAX or null | `0x004A2FF0` |
| 15 | +0x3C | `CreateProp(id, prop*&)` | 8 | AL | `0x004A3040` |
| 16 | +0x40 | `CreateGenericProp(key const&, prop*&)` | 8 | AL | `0x004A24D0` |
| 17 | +0x44 | `GetCityProps(list&, SC4Rect<float> const&)` | 8 | AL | `0x004A1DF0` |
| 18 | +0x48 | `GetCityProps(list&, SC4Rect<int32> const&)` | 8 | AL | `0x004A1DB0` |
| 19 | +0x4C | `AddCityProp(occ)` | 4 | AL | `0x004A1F30` |
| 20 | +0x50 | `AddCityProp(id, pos const&, orient)` | 12 | EAX occ | `0x004A1E70` |
| 21 | +0x54 | `RemoveCityProps(SC4Rect<float> const&)` | 4 | AL = 1 | `0x004A38F0` |
| 22 | +0x58 | `RemoveCityProps(SC4Rect<int32> const&)` | 4 | AL = 1 | `0x004A3850` |
| 23 | +0x5C | `GetLotProps(list&, x, z)` | 12 | AL | `0x004A1FD0` |
| 24 | +0x60 | `GetLotProps(list&, lot)` | 8 | AL | `0x004A3240` |
| 25 | +0x64 | `AddLotProp(lot, occ)` | 8 | AL | `0x004A20C0` |
| 26 | +0x68 | `AddLotProp(id, pos const&, orient)` | 12 | EAX occ | `0x004A2670` |
| 27 | +0x6C | `RemoveLotProps(x, z)` | 8 | AL = 1 | `0x004A3AF0` |
| 28 | +0x70 | `RemoveLotProps(lot)` | 4 | AL = 1 | `0x004A3A50` |
| 29 | +0x74 | `RemoveProp(occ)` (SDK `RemovePropA`) | 4 | AL = 1 | `0x004A27B0` |
| 30 | +0x78 | `GetStreetProps(list&, SC4Rect<int32> const&)` | 8 | AL | `0x004A1F90` |
| 31 | +0x7C | `GetStreetProps(list&, x, z)` | 12 | AL = 0 | `0x004A3150` |
| 32 | +0x80 | `AddStreetProp(id, pos const&, orient)` | 12 | EAX occ | `0x004A2120` |
| 33 | +0x84 | `RemoveStreetPropsFromTile(x, z)` | 8 | — | `0x004A3990` |
| 34 | +0x88 | `GetBuildingProps(list&, x, z)` | 12 | AL | `0x004A2140` |
| 35 | +0x8C | `GetBuildingProps(list&, building)` | 8 | AL | `0x004A3770` |
| 36 | +0x90 | `AddBuildingProp(building, prop)` | 8 | AL | `0x004A3B90` |
| 37 | +0x94 | `RemoveBuildingProps(x, z)` | 8 | AL | `0x004A21A0` |
| 38 | +0x98 | `RemoveBuildingProps(building)` | 4 | AL | `0x004A3390` |
| 39 | +0x9C | `RemoveBuildingProp(building, prop)` | 8 | AL | `0x004A27C0` |
| 40 | +0xA0 | deleting destructor (flags) | 4 | EAX | `0x004A2310` |

"—" means the function leaves no defined value. Proposed `docs/abi` rows:
`build-runs/re-specs13d-abi-rows.tsv`.

**Who calls in** (beyond the specs listed at the top): the prop developer (slots 8, 9, 13), crime (slot 13),
neighbours (slots 15, 18, 19), the lot developer (slots 15, 24, 26, 29), landfill (slots 26, 29), demolition
(slot 29), the city detail manager (slots 26, 27, 29), the construction occupant (slot 15), the network lot
manager (slots 32, 33) and the lot configuration manager (slots 8, 9, 14).

---

# Part B: the lot configuration manager

## B1. Identity

| Item | Value |
|---|---|
| Class ID | `0x299062C0` |
| Component | city component **9**, IID `0xC8EDBEC0` (foundations §3.6); global `0x00B43D18` |
| Constructor | `0x0048A1B0`: every field zero; +0x20 a new empty red-black tree header (0x18 bytes) |
| Interface at +0 | `cISC4LotConfigurationManager` (SDK header), vtable `0x00A8EC58` (29 slots, §B7) |
| Message target at +4 | vtable `0x00A8EC48`, `DoMessage` `0x0048BC50` |
| Serializable at +8 | vtable `0x00A8EC30`, `Write` `0x00489850`, `Read` `0x004898A0` |

`QueryInterface` (`0x004896A0`): `1` and `0xC8EDBEC0` → +0; `0xE4FDA3D4` → +8; otherwise false (the message target
is not offered). Reference count at +0x10; the last `Release` (`0x004897C0`) calls `Shutdown` and then the
deleting destructor (slot 28).

## B2. State

| Offset | Field | Set by | Saved |
|---|---|---|---|
| +0x0C | initialised flag (u8) | `Init` / `Shutdown` | no |
| +0x14 | building development simulator | `Read`, else `Init` (global `0x00B43D68`) | **ref** |
| +0x18 | flora simulator | `Read`, else `Init` (global `0x00B43D80`) | **ref** |
| +0x1C | prop manager | `Read`, else `Init` (global `0x00B43D10`) | **ref** |
| +0x20, +0x24 | registered filters: a sorted map filter pointer → cached result vector (null until first used), and its count | slots 18, 19 | no |

**Process-wide catalogue** (static):

| Global | Content |
|---|---|
| `0x00B216C8` | configuration map: configuration ID → configuration (one reference each); slot 20 returns its address |
| `0x00B216DC` | family set: every building family ID listed by an added configuration (slot 6); slot 21 returns its address |
| `0x00B216BC` | "catalogue loaded" flag (u8) |
| `0x00B216C0` | the detail level the catalogue was built for (u32) |

Iteration over the map is bucket order.

## B3. The catalogue and the detail filter *[compat]*

### B3.1 Building and rebuilding (`StaticInit`, `0x0048A540`)

Called by every `Init` (§B4.1). `d :=` the city detail preference (the byte at +0xF16 of the preferences object,
global `0x00B43CD8`; 1 low, 2 medium, 3 high; city-detail.md §2).

1. Loaded and `d` equals the stored level → nothing.
2. Loaded and `d` differs → **unload** (`0x0048A240`): clear the flag, unregister the callback, release every
   configuration and empty the map. The family set is **not** emptied.
3. Load: set the flag; register `0x0048A240` as a shutdown callback (application slot 69); when the resource
   manager exists, list every resource of type `0x6534284A` (slot 18) and, for each key whose **group** is
   `0xA8FBD372` (`0x00489990`), call slot 5 with the key. The exemplar type is not checked.
4. Stored level `:= d`.

So a game that opens a city at detail 3, changes the preference to 1 and opens a city again works with the
detail-1 catalogue from then on.

### B3.2 Reading one configuration (slot 5, `0x00489A60`; `0x00489910`)

Create a lot configuration (0xC0 bytes), and call its `Load(key, d)` (configuration slot 31; the SDK names the
second argument `ucPropArraySize`, but it is the **detail level**, read from the preferences at this moment).
On failure release it and return false; else add it with slot 6 `(cfg, true)`, release the creation reference and
return slot 6's result.

### B3.3 Which lot objects are kept (answers CD1)

The configuration builds its object array (lot-developer.md §17.1) from the lot-object properties `0x88EDC900`,
`0x88EDC901`, … (`0x00479D10`, called from `Load` at `0x0047A536`). The rule:

- The scan stops at the first missing property (or at `0x88EDCDFF`). A property with fewer than 10 values is
  skipped.
- `LOD := (v1 >> 4) & 3`, where `v1` is the property's second value (record byte +0x01).
- An object is kept when its type `v0` is **0** (the building) or when **`LOD < d`**. In other words: LOD 0 at
  every level, LOD 1 at medium and high, LOD 2 at high only, LOD 3 never (unless the object is the building).
- A dropped object leaves no gap: the array holds only the kept objects, in property order.

The same reader builds the object lists of network lot configurations, always with d = 3 (`0x0060F80E`;
city-detail.md §3.1). The other details of the record (fixed-point offsets, the tag, the generated serial number
for configurations of version < 2 at `0x00B2166C`) are in lot-developer.md §17.1. The configuration's version is
property `0x88EDC789`; every shipped configuration has version 2.

**Shipped data** (`SimCity_1.dat` … `SimCity_5.dat`: 1908 lot configurations, all in group `0xA8FBD372`, all
version 2; 73,832 object properties with at least 10 values). Count by type and LOD:

| Type `v0` | LOD 0 | LOD 1 | LOD 2 | LOD 3 |
|---|---|---|---|---|
| 0 building | 1908 | 0 | 0 | 0 |
| 1 prop | 1235 | 16516 | 27261 | 0 |
| 2 base texture | 20021 | 0 | 5431 | 0 |
| 4 flora | 0 | 0 | 1359 | 0 |
| 5 | 43 | 0 | 0 | 0 |
| 6 | 17 | 0 | 0 | 0 |
| 7 | 41 | 0 | 0 | 0 |

Kept objects: detail 3 all 73,832; detail 2 39,781 (no LOD-2 props, textures or flora); detail 1 23,265 (props
only at LOD 0: 1235 of 45,012). So at low detail a shipped lot keeps its building, its LOD-0 base textures and
about 3 % of its props, and loses all its flora.

### B3.4 Adding and removing (slot 6, `0x0048A690`, pop 8, always true)

`id :=` the configuration's ID (configuration slot 3).

| `id` in the map? | `add` | Effect |
|---|---|---|
| no | true | take a reference; insert every ID of the configuration's family list (configuration slot 6) into the family set; flush every filter cache (§B5.4); insert `id → cfg` |
| no | false | nothing |
| yes | true | release the stored configuration, take a reference to the new one and store it in place. No flush, no family-set update *[quirk]* |
| yes | false | flush the filter caches and erase the entry. The configuration is **not released** and the family set is not updated *[quirk]* |

Slot 7 `RemoveLotConfiguration(id)`: when `id` is in the map, slot 6 `(cfg, false)`; else false.

## B4. Lifetime and messages

### B4.1 `Init` (slot 3, `0x0048A5E0`) → true

Only when not initialised: set the flag; `StaticInit` (§B3.1); fill each null field (building development
simulator, flora simulator, prop manager), each with a reference; subscribe to `0x26D31EC1`
(`kSC4MessagePostCityInit`).

### B4.2 `Shutdown` (slot 4, `0x004897F0`) → true

Only when initialised: unsubscribe; release and null the three fields; clear the flag. The catalogue stays until the
process shutdown callback or the next detail change.

### B4.3 `kSC4MessagePostCityInit` (`0x26D31EC1`, `0x0048BC50`) → true

For every configuration in the map (bucket order), fill its building information (`0x0048AD40`):

1. Take the configuration's primary building record (configuration slot 24). For each ID in its ID list, ask the
   building development simulator (slot 19) for the building types of that family (into a hash set of hint
   100); for each type, in the set's bucket order:
   - its key (building development slot 14) and the occupant groups of its exemplar, collected into a sorted set;
   - its exemplar's monopoly property `0x2781283F` (bool): absent or false sets a flag;
   - its building profile (slot 39) and the u16 at profile +8: track the smallest and the largest.
2. Store in the configuration: +0x3E the smallest u16, +0x40 the largest (both **0** when no type was found, or
   when there is no building record); +0x80 the sorted group set as a vector; +0x8C the flag (1 when at least one
   type lacks the monopoly property). The SDK's `GetMinBuildingCapacity` and `GetMaxBuildingCapacity`
   (configuration slots 12 and 13) report the two u16 values (confirmed: lot-configuration.md §4, §5.1). Nothing
   reads +0x8C (lot-configuration.md §5.4).

**A type whose exemplar does not load** (PL11). The three per-type reads are independent:

- **Groups**: the occupant-group collector (`0x005E9500`) loads the exemplar itself through the global resource
  manager; when the load fails it returns an empty list, so the type adds no group.
- **Monopoly**: the monopoly flag is cleared before the load (`0x0048AE95`) and the property is read only when the
  load succeeds (`0x0048AEAA`), so a failed load counts as "lacks the property": +0x8C becomes 1.
- **Capacity**: the profile comes from the building development simulator (slot 39), not from the exemplar, so the
  type **still counts** towards the smallest and largest capacity (`0x0048AEE8`–`0x0048AF04`).

The type's key (building development slot 14) is read first; a type the simulator does not know gives a zero key and
then the same failed-load path. The types come from the simulator's own family list (slot 19), so in the shipped
game every one of them has a loadable exemplar.

## B5. Behaviour of the queries

### B5.1 Lookups

| Slot | Behaviour |
|---|---|
| 8 `GetLotConfiguration(id)` | the configuration or null; no reference is added |
| 20 `GetLotConfigurationMap()` | address of the map (`0x00B216C8`) |
| 21 `GetLotConfigurationFamilyIDSet()` | address of the family set (`0x00B216DC`) |

### B5.2 Set queries

Each walks the whole map in bucket order and **adds** matches to the caller's hash set (it never clears it), and
returns the number of matches in EAX (not a bool).

| Slot | Match | Adds |
|---|---|---|
| 9 `GetLotConfigurationsByFilter(set, filter)` | see §B5.4 | configuration, with a reference |
| 10 `GetLotConfigurationsByFamily(set, family)` | configuration slot 5 `BelongsToFamily(family)` | configuration, with a reference |
| 11 `GetLotConfigurationsBySize(set, x, z)` | configuration slot 7 `GetSize` gives exactly (x, z) | configuration, with a reference |
| 13 `GetLotConfigurationIDsByFilter(set, filter)` | null filter, or filter slot 3 true | configuration ID |
| 14 `GetLotConfigurationIDsByFamily(set, family)` | as slot 10 | configuration ID |
| 15 `GetLotConfigurationIDsBySize(set, x, z)` | as slot 11 | configuration ID |

A configuration that is already in the set still gets a new reference and still counts *[quirk]*: the reference
is never released by slot 12.

Slot 12 `ClearLotConfigurationSet(set)`: release every element once and empty the set.

**Slot 9 reference rule (answers PL19).** On **every** call, slot 9 adds exactly one reference to each configuration
it returns, on the same interface pointer it inserts (configuration slot 1 `AddRef`, vtable `+0x04`). This holds on
all three paths of §B5.4, and in particular on the registered-filter path whether the cache was just built or is
being reused: the cache-miss branch falls through into the same loop the cache-hit branch jumps to
(`0x0048B1D8` → `0x0048B29F`), which calls `AddRef` once per cached element (`0x0048B2B0`–`0x0048B2BC`) and only
then inserts the cached elements into the caller's set (`0x0048A170`, one insert-if-absent per element). The cache
itself holds plain pointers, with **no** reference (built at `0x0048B222`). The count returned on that path is the
cache length (`0x0048B2D3`–`0x0048B2DD`), not the number of new set entries. Nothing else in the manager gives out
or takes back a reference for a slot-9 result: the caller owns the references and releases them.

**The caller that releases them: the growth pass** (`cSC4TractDeveloper::ListPossibleLotConfigurations`,
`0x00709650`, called once per `cSC4TractDeveloper::Grow` call at `0x0070D4EA`; the "release wave" at
`0x00709798` is the return address of its `Release` call at `0x00709795`):

1. A fresh, empty local hash set (100 buckets, `0x00709661`–`0x00709677`) is made per call.
2. The developer keeps its own map (developer +0x130) from a two-value key (developer +0x80 slots `+0x20` and
   `+0x1C`) to a 12-byte filter object (vtable `0x00AB30DC`). On a miss it creates the filter and **registers**
   it (slot 18) once per key. So in a running city every growth query is the registered,
   cached path of §B5.4: one cache build per (key, flush), then cache hits.
3. Slot 9 (`set`, `filter`) is called once (`0x00709701`). A zero result returns 5 at once; nothing is released
   (the set is empty).
4. Otherwise it walks the **local result set** (the container `0x00709798` iterates) node by node in bucket
   order: for each configuration it calls `IsLotConfigurationSuitable` (`0x00704C00`), counts the verdict in a
   histogram (developer +0x170, +0x18), keeps the largest verdict, and on verdict `0x1B` inserts the bare pointer
   into the developer's candidate set (developer +0xD8, `0x0049A050`: no `AddRef`); then it calls the
   configuration's `Release` (vtable `+0x08`) **once**, whatever the verdict.
5. The walk stops at the end of the set (next node null and no further non-empty bucket, `0x0047C1A0`); there is
   no early exit. The set is then destroyed without any further release; the largest verdict is returned.

Per configuration and per growth query the original's net is therefore zero: +1 in slot 9, −1 at `0x00709795`.
The release count of a configuration equals the number of growth queries whose result contained it, which is
why it follows the game's pass structure. Configurations are never drained, because nothing else releases the
catalogue's own reference (§B3) and the per-lot references (`cSC4Lot::Init`, `0x006BF53D`) are balanced by the
lot. The candidate set (+0xD8) and the filter cache both hold pointers kept alive only by the catalogue reference.

What an implementation needs: one `AddRef` per returned element per slot-9 call, on every call (cache hit
included), on the pointer whose `Release` the caller will call; no second add anywhere else (a second add makes
the count grow by one per query instead of draining). A ledger that drains by one per query means some
slot-9 calls handed out no reference on that pointer, for example a cache-hit path that skips the add, or the add
landing on a different object than the one placed in the set. *(Which of these the parked glue21 build does is
not decidable from the exe; the exe only fixes the contract above.)*

### B5.3 Building tests (slots 16, 17; pop 12, AL)

Both set `*obj := null` first (when `obj` is given) and return false for a null configuration or a zero ID. They
scan the configuration's object array (slot 23) in order and look only at **type-0** records:

- **16 `LotConfigurationHasBuildingFamily(cfg, family, obj)`**: the first record whose ID list contains `family`.
- **17 `LotConfigurationHasBuildingType(cfg, type, obj)`**: per record, first whether its ID list contains `type`,
  then, for each ID `f` in order, whether building development slot 20 `IsBuildingInFamily(type, f)` is true.

On a hit `*obj :=` that record and the result is true.

### B5.4 Filters (slots 9, 18, 19)

- **18 `Register(filter)`** adds the filter with no cached result (a second registration does nothing).
- **19 `Unregister(filter)`** frees its cached result and removes it.
- **9 `GetLotConfigurationsByFilter(set, filter)`**:
  - null filter → every configuration;
  - registered filter → on first use, the configurations for which filter slot 3 is true are cached, in map bucket
    order; the cache is then reused (the filter is **not** asked again) until the next flush; each cached
    configuration is added to the set with a reference, on every call, cache hit or not (§B5.2);
  - unregistered filter → filter slot 3 for each configuration.
- **Flush** (`0x0048A080`): drops every cached result. It happens when a configuration is added (new ID) or erased
  through slot 6, and therefore during every catalogue rebuild.

### B5.5 Footprints (slots 24–26, pop 8)

Each writes a `cS3DRect2D` as four floats `(w, d, 0, 0)` and returns nothing defined:

| Slot | Start `(w, d)` | Per ID of the list |
|---|---|---|
| 24 buildings | (4, 4) | a valid building type (building development slot 15) with a profile (slot 40): its profile +0x20 and +0x28; otherwise the types of the family (slot 18, used only when it returns non-zero), each the same way |
| 25 flora | (2, 2) | a flora type with a profile (flora simulator slot 7): twice its +0x3C for both; otherwise its family's types (slot 14) |
| 26 props | (2, 2) | a prop profile (prop manager slot 14): size x (+0x04) and size z (+0x0C); otherwise its family (prop manager slot 9, used only when it returns non-zero) |

Each candidate raises `w` and `d` separately when strictly larger (NaN never). Slots 25 and 26 use the **global**
flora simulator and prop manager, not the held ones.

### B5.6 Expanding an object's ID list (slot 27, `0x0048BA10`, pop 8)

`GetObjectIDArray(obj, vec)` clears `vec`; a null `obj` leaves it empty. Then for each ID of the record's ID list
(+0x28), in order, by the record's type (+0x00); the managers are the **global** ones:

| Type | ID is a type | ID is a family |
|---|---|---|
| 0 building (building development slot 15 `IsBuildingTypeValid`) | append | slot 18 `GetBuildingTypesByFamily(vec, id)`, which **assigns** `vec`, discarding what was appended before *[quirk]* |
| 1 prop (prop manager slot 8) | append | prop manager slot 9 into a temporary vector, then append it (family load order, §A3.3) |
| 4 flora (flora simulator slot 11) | append | flora simulator slot 14 into a hash set (hint 100), appended in its bucket order |
| any other | nothing | nothing |

lot-developer.md §6.5 describes the lot developer's use of this list; its "families expanded in hash order"
holds only for flora.

### B5.7 Editing helpers

- **22 `CreateGenericLotConfiguration(cfg&, buildingType)`** (`0x0048BCF0`, pop 8, AL): `cfg := null`; false
  when building development slot 16 rejects the type. Otherwise it builds a configuration (not added to the map)
  whose only object is the building, sized from the building's footprint (slot 24 on that type) and centred on a
  lot of the building's size, and returns true *[inferred: the lot editor's "new configuration"]*. In detail
  (PL12):
  1. `ids := [buildingType]`; `(w, d) :=` slot 24 on `ids` (§B5.5: at least 4 × 4).
  2. **Lot size** (`0x0048BD66`–`0x0048BD9B`): `sizeX := 1 − trunc(w × −0.0625)` and
     `sizeZ := 1 − trunc(d × −0.0625)` (float multiply, `_ftol` truncation toward zero, foundations §18.1). For
     `w ≥ 0` that is **`1 + floor(w / 16)`**, not `ceil(w / 16)`: it equals the ceiling only when `w` is not a
     multiple of 16, and gives one cell more when it is *[quirk]*. Examples: w = 4 → 1, 15.9 → 1, 16 → 2, 24 → 2,
     32 → 3, 40 → 3. Stored as the low bytes at +0x20 (x) and +0x21 (z).
  3. A new configuration (constructor `0x004797A0`), one reference for the caller in `cfg`. ID (+0x08) := a fresh
     GUID (`cIGZCOM` slot 7 `CreateGuid`). The other fields are written to the constructor's values or emptied:
     family list empty, maximum slope (+0x30) 90.0, minimum slope (+0x34) 0, maximum slope before foundation
     (+0x38) **5.0**, growth stage 1, water-compatible 0, the three bit vectors cleared, object, wall, fence and
     fence-position vectors emptied. The name, the do-construction flag (0) and road sides (8) keep the
     constructor's values.
  4. **The building object** (one 0x34-byte record, lot-developer §17.1), appended to the object array: type 0,
     `v1` 0, orientation 0, mirror 0; offset `(cx, 0, cz)` with `cx := (16 · sizeX) × 0.5` and
     `cz := (16 · sizeZ) × 0.5` (the lot's centre); footprint max `(cx + w × 0.5, cz + d × 0.5)` at +0x10/+0x14 and
     min `(cx − w × 0.5, cz − d × 0.5)` at +0x18/+0x1C; +0x20 := 0; ID list := `[buildingType]`. The lot tag at
     **+0x24 is never written**: it is whatever the stack held *[quirk]*. *[choice]* FreeCity4 writes 0 there.
- **23 `FixLotConfiguration(cfg)`** (`0x0048B3B0`, pop 4, EAX): the lot editor's checker. For each building,
  prop and flora record it removes IDs that are not valid types or families, removes records left with no IDs,
  recomputes the footprint from slots 24–26 (x and z swapped for orientations 1 and 3), checks that the footprint
  lies inside the lot with a 0.049 m margin and that the record's centre is inside its footprint, and finally
  removes the first invalid ID of the configuration's slot 28 list. It returns the code of the **first** problem
  found: 0 none, 1 an ID removed, 2 a record removed, 3 a footprint changed, 4 outside the lot, 5 a centre moved,
  6 an ID of the slot 28 list removed. No caller was found outside the editor. In detail (PL13, PL14):
  1. `lotW := 16 · sizeX`, `lotD := 16 · sizeZ` (configuration slot 7, as unsigned, converted to float).
  2. For each record of the object array (slot 23, edited in place), by index, whose type is **0, 1 or 4** (every
     other type is left alone):
     - **IDs.** For each ID of the record's list, in order, the validity test of its type, using the manager's
       **held** components (+0x14, +0x18, +0x1C), not the globals: building → building development slot 16
       `IsBuildingFamilyTypeValid` (a family or a type); prop → prop manager slot 11 `IsPropFamilyTypeValid`
       (§A5: a family or a type); flora → flora simulator slot 12 `IsFloraFamilyTypeValid` (a family or a
       registered type, flora.md). An invalid ID is erased (the next ID moves into its place and is tested next);
       code 1.
     - **Empty record.** When the list is now empty, the record is erased from the array and the next record is
       examined next; code 2. Nothing else is done for it.
     - **Footprint.** `(oldW, oldD) := (maxX − minX, maxZ − minZ)`. `(w, d) :=` slot 24 (building), 26 (prop) or
       25 (flora) on the record's list; swapped when the orientation byte (+0x02) is 1 or 3. When
       `|oldW − w| > 0` or `|oldD − d| > 0` (strict; NaN false), the footprint is rebuilt **around the record's
       offset** `(x, z)` (+0x04, +0x0C), **not** around the old footprint's centre: `min := (x − w × 0.5,
       z − d × 0.5)`, `max := (x + w × 0.5, z + d × 0.5)`; code 3.
     - **Inside the lot** (`0x0048B6B3`–`0x0048B71F`). With `ex := f32(lotW − 0.05)` and `ez := f32(lotD − 0.05)`:
       when `minX < 0.049` or `minZ < 0.049` or `ex + 0.001 < maxX` or `ez + 0.001 < maxZ` (floats as stored:
       0.049 is `0x3D48B43A`, 0.05 `0x3D4CCCCD`, 0.001 `0x3A83126F`; strict; NaN never), code 4. **Nothing is
       moved**: the record stays outside the lot.
     - **Centre.** When `x < minX` or `x > maxX` or `z < minZ` or `z > maxZ` (strict; NaN never), the offset is
       moved to the footprint's centre: `x := (maxX + minX) × 0.5`, `z := (minZ + maxZ) × 0.5`; code 5. A rebuilt
       footprint is centred on the offset, so this fires only for an unchanged footprint.
  3. **Retaining walls** (slot 28's vector, edited in place; none when slot 28 returns null). For each ID in order:
     valid when building development slot 28 `GetLotRetainingWallKeyFromType(id, key&)` (the held simulator)
     returns true, that is when `id` is the instance of an `ExemplarType 0x15` exemplar the simulator catalogued
     (building-development §1.3). The **first** invalid ID is erased and the slot returns at once: the earlier
     code if there was one, else 6. Later IDs are not examined.

  Each "code k" above means "the result becomes k if it is still 0", so the result is the first problem's code.

## B6. Persistence *[compat]*

Record type `0x299062C0`, version **1**:

| Field | Type |
|---|---|
| version | u16 = 1 |
| building development simulator | reference (read with IID `0x07813EAE`) |
| flora simulator | reference (IID `0x06C66177`) |
| prop manager | reference (IID `0x28326F3E`) |

26 bytes; another version → `Read` false. All 421 sample saves: 26 bytes, classes `0x8990BE7A`, `0xC990C034`,
`0xA990BD10`. The configurations themselves are never saved; a loaded city sees the catalogue of the current
detail level.

## B7. Interface slot table (`cISC4LotConfigurationManager`, vtable `0x00A8EC58`) *[compat]*

| Slot | Offset | Method | Pop | Return | Address |
|---|---|---|---|---|---|
| 0 | +0x00 | `QueryInterface` | 8 | AL | `0x004896A0` |
| 1 | +0x04 | `AddRef` | 0 | EAX | `0x004F7BA0` (shared) |
| 2 | +0x08 | `Release` | 0 | EAX | `0x004897C0` |
| 3 | +0x0C | `Init` | 0 | AL = 1 | `0x0048A5E0` |
| 4 | +0x10 | `Shutdown` | 0 | AL = 1 | `0x004897F0` |
| 5 | +0x14 | `AddLotConfiguration(key const&)` | **4** | AL | `0x00489A60` |
| 6 | +0x18 | `AddLotConfiguration(cfg, add)` | 8 | AL = 1 | `0x0048A690` |
| 7 | +0x1C | `RemoveLotConfiguration(id)` | 4 | AL | `0x0048A2D0` |
| 8 | +0x20 | `GetLotConfiguration(id)` | 4 | EAX | `0x0048A320` |
| 9 | +0x24 | `GetLotConfigurationsByFilter(set&, filter)` | 8 | EAX count; +1 reference per returned element on every call (§B5.2 "Slot 9 reference rule") | `0x0048B1A0` |
| 10 | +0x28 | `GetLotConfigurationsByFamily(set&, family)` | 8 | EAX count | `0x0048A7E0` |
| 11 | +0x2C | `GetLotConfigurationsBySize(set&, x, z)` | 12 | EAX count | `0x0048A8B0` |
| 12 | +0x30 | `ClearLotConfigurationSet(set&)` | 4 | — | `0x0048A370` |
| 13 | +0x34 | `GetLotConfigurationIDsByFilter(set&, filter)` | 8 | EAX count | `0x0048A9A0` |
| 14 | +0x38 | `GetLotConfigurationIDsByFamily(set&, family)` | 8 | EAX count | `0x0048AA80` |
| 15 | +0x3C | `GetLotConfigurationIDsBySize(set&, x, z)` | 12 | EAX count | `0x0048AB50` |
| 16 | +0x40 | `LotConfigurationHasBuildingFamily(cfg, family, obj**)` | 12 | AL | `0x00489AA0` |
| 17 | +0x44 | `LotConfigurationHasBuildingType(cfg, type, obj**)` | 12 | AL | `0x00489B40` |
| 18 | +0x48 | `RegisterLotConfigurationFilter(filter)` | 4 | — | `0x0048A660` |
| 19 | +0x4C | `UnregisterLotConfigurationFilter(filter)` | 4 | — | `0x0048A3F0` |
| 20 | +0x50 | `GetLotConfigurationMap()` | 0 | EAX | `0x00489720` |
| 21 | +0x54 | `GetLotConfigurationFamilyIDSet()` | 0 | EAX | `0x00489730` |
| 22 | +0x58 | `CreateGenericLotConfiguration(cfg*&, buildingType)` | 8 | AL | `0x0048BCF0` |
| 23 | +0x5C | `FixLotConfiguration(cfg)` | 4 | EAX code | `0x0048B3B0` |
| 24 | +0x60 | `GetMaxFootprintForBuildingFamilies(vec const&, rect&)` | 8 | — | `0x00489E10` |
| 25 | +0x64 | `GetMaxFootprintForFloraFamilies(vec const&, rect&)` | 8 | — | `0x0048B860` |
| 26 | +0x68 | `GetMaxFootprintForPropFamilies(vec const&, rect&)` | 8 | — | `0x00489F50` |
| 27 | +0x6C | `GetObjectIDArray(obj, vec&)` | 8 | — | `0x0048BA10` |
| 28 | +0x70 | deleting destructor (flags) | 4 | EAX | `0x0048A220` |

**SDK error:** the header declares slot 5 as `AddLotConfiguration(cISC4LotConfiguration*, bool)` and slot 6 as
`AddLotConfiguration(cGZPersistResourceKey const&, void*)`. The game's slot 5 takes **only the key** (pop 4) and
slot 6 takes the configuration and the flag. "—" means no defined return value although the SDK declares bool or
void; a caller that tests it reads whatever the last inner call left.

**Who calls in:** the lot (slot 8 in `Init`), the lot developer (slots 8, 27), the network lot manager (slot 27),
the tract developer, zone developer and zones (slots 8, 9, 18, 19, 20), the port developers (slot 8), the
building development simulator (slot 20), the place-lot and toll-booth tools, the lot configuration editor and
chooser (slots 5, 7, 22, 23) and the lot configuration filters.

---

## Q. Quirks (reproduced unless marked *[choice]*)

1. Prop manager slot 9 returns 0 for a single prop type, while filling the vector with it; callers that use the
   count treat a type as an empty family.
2. Prop manager slot 13 inserts an all-zero profile for an unknown ID; slot 14 then finds it.
3. Prop manager slot 12 always returns an empty string; the exemplar name is read and discarded.
4. Prop manager slot 31 returns false even when it found props.
5. Prop manager slots 20, 26 and 32 call the occupant manager without testing it; slots 19, 25 and 36 test it.
6. Prop manager slots 19 and 25 accept an occupant that is already inserted without changing its usage; slot 25
   ignores its lot.
7. Prop manager slot 24 filters the whole list, including what the caller had in it before.
8. The prop profile copies OccupantSize without a bound.
9. The `CreateTextDecal` debug command leaks one reference per decal.
10. The lot configuration catalogue is rebuilt only when `Init` sees a changed detail preference; the family set
    keeps IDs across rebuilds.
11. Lot configuration manager slot 6: erasing does not release; replacing does not flush the caches.
12. Lot configuration manager set queries take a reference even when the set already holds the configuration.
13. Lot configuration manager slot 27 loses earlier entries when a building family follows other IDs.
14. Lot configuration manager slots 12, 18, 19 and 24–27 leave AL undefined.
15. The prop profile reader leaves size dwords unwritten for a short OccupantSize, and more than six values
    overwrite its return address (§A3.2, PL8).
16. `CreateProp` sizes the box through the OccupantSize data pointer without a count check (§A6.1, PL9).
17. Prop manager slot 36 skips its final usage write when the building has no property holder (§A7.1, PL10).
18. `CreateGenericLotConfiguration` makes a lot of `1 + floor(w / 16)` cells (one too many for a multiple of 16)
    and leaves the object's lot tag unwritten (§B5.7, PL12).
19. `FixLotConfiguration` reports a footprint outside the lot (code 4) without moving it, and rebuilds a footprint
    around the record's offset (§B5.7, PL13).

## O. Open questions

- ~~**PL1:** prop manager `DoMessage` for `CreateTextDecal`: what text decal slots 6, 8 and 10 mean (size, colour,
  …). Debug-only; an in-game run of the command would settle it.~~ **ANSWERED 2026-10-16 (re-answers18):**
  - slot 6 is `SetTextHeight` (`0x005F7B10`, 0 < h < 8);
  - slot 8 is `SetTextColor(r, g, b)` (`0x005F7B50`);
  - slot 10 is `SetAlpha` (`0x005F7C00`).

  Argument 3 is the height, arguments 4–6 the colour and argument 7 the alpha. §A4.3.
- ~~**PL2:** the value at `0x00B21770` sent as data2 of `kSC4MessagePropExemplarDataUpdated`: no writer was found,
  so it is 0 *[inferred]*. A memory read after the first city load would settle it.~~ **ANSWERED 2026-10-16
  (re-answers18):** **DIFFERS**. `0x00B21770` is the element count (`+0x10`) of the type map, the hash table at
  `0x00B21760`, zeroed by its constructor at `0x00829C1E`. Inserts write it through the table's own member
  functions (`find_or_insert` `0x004A2930` increments it at `0x004A2997`), so no direct writer shows up. data2 is the number of prop types in the catalogue, not 0. §A2, §A3.4.
- ~~**PL3:** configuration +0x8C (the monopoly flag) and the SDK's `IsCompatibleWithMonopolyPieceUse`
  (configuration slot 19): whether the slot returns the flag or its negation belongs to a lot configuration spec.~~
  **ANSWERED 2026-10-16 (re-specs15a):** neither (lot-configuration.md §5.4). Slot 19 is the shared "return true,
  pop 4" stub `0x004FC900`: it answers **true** for every configuration, takes one ignored dword argument (the SDK
  declares none) and does not read +0x8C. Nothing else reads +0x8C either; the monopoly rule is applied from the
  building exemplars' `0x2781283F` by the tract developer and the plop tool's monopoly lot filter.

## T. Conformance tests

1. **Profile defaults.** Catalogue an exemplar with instance `0x1234`, `ExemplarType` 0x1E and no other
   property: slot 8(`0x1234`) true; slot 14 gives `{0x2977AA47, 0, 0, 0, 0, 0, 0}`; slot 15 creates a plain prop
   occupant with prop type `0x1234` and bounding box (0,0,0)–(1,1,1). With OccupantSize (2, 6, 3) the box is
   (0,0,0)–(2,6,3) and profile +0x04..+0x0C = 2, 6, 3.
2. **Unknown profile.** Slot 14(`0x9999`) → null; slot 13(`0x9999`) → a zero profile; slot 14(`0x9999`) → non-null;
   slot 8(`0x9999`) → false; slot 15(`0x9999`) → false.
3. **Families.** Exemplars A (families F, G), then B (family F), then A again: family F = [A, B], G = [A].
   Slot 9(vec = [7], F) → returns 2, vec = [A, B]. Slot 9(vec = [7], A) → returns 0, vec = [A]. Slot 9(vec = [7],
   unknown) → 0, vec = [7]. Slot 10(A, F) true; slot 10(A, A) true; slot 10(B, G) false. Slot 11(A) true, slot
   11(F) true.
4. **GetLotProps boundary** (16 m cells). A lot on cells (10, 20)–(11, 21): world box x ∈ [160, 192),
   z ∈ [320, 352). Lot-usage props at (160, 320), (191.9, 351.9), (192, 330) and (175, 319.9): the first two
   stay, the last two are removed and released. A prop the caller put in the list beforehand at (0, 0) is also
   removed. Result true.
5. **Street props.** Slot 31(list, 5, 5) with one street prop on cell (5, 5): the list has it and the result is
   false.
6. **Building props.** Slot 36(B, P1), slot 36(B, P1), slot 36(B, P2): B's property `0xA8322B7F` holds [P1, P2];
   slot 35 returns both with a reference each; slot 39(B, P1) true, leaving [P2]; slot 39(B, P2) true and the
   property is gone; slot 35 then false.
7. **Detail filter** (CD1). A configuration with objects: building with `v1 = 0x30`; prop `v1 = 0x00`; prop
   `v1 = 0x10`; texture `v1 = 0x20`; flora `v1 = 0x30`. Detail 1 → [building, prop LOD 0]; detail 2 → [building,
   prop LOD 0, prop LOD 1]; detail 3 → [building, prop 0, prop 1, texture]; the flora (LOD 3) is never kept. The
   bits outside `0x30` of `v1` do not matter: `v1 = 0x1F` is LOD 1.
8. **Rebuild on detail change.** Init at detail 3: 1908 configurations, 73,832 objects in total. Shutdown, set
   detail 1, Init: same 1908 configurations, 23,265 objects. Init again at detail 1: no reload (the same
   configuration objects are returned by slot 8).
9. **GetObjectIDArray.** Building record IDs [T3, F1] where T3 is a building type and F1 a family of {T1, T2}:
   result = F1's types only (T3 lost). IDs [F1, T3]: F1's types followed by T3. Prop record IDs [P1, PF] with PF
   = [P2, P3]: [P1, P2, P3]. A texture record: empty.
10. **Has building type.** A configuration whose building record lists [F1] with T1 in F1: slot 16(cfg, F1) true,
    slot 16(cfg, T1) false, slot 17(cfg, T1) true, slot 17(cfg, F1) true (F1 is listed directly).
11. **Prop footprint.** Slot 26 with [PF] where PF = {P2 size (3, 1, 5), P3 size (4, 1, 2)}: (4, 5, 0, 0). With an
    empty list: (2, 2, 0, 0).
12. **Records.** Prop manager: `01 00`, then (id₁, `4D 96 8F 09`), (id₂, `01 9B 8F 49`): 18 bytes. Lot
    configuration manager: `01 00` and three references, classes `0x8990BE7A`, `0xC990C034`, `0xA990BD10`:
    26 bytes. A record with version 2 → `Read` false.
13. **Profile read order** (PL8). An exemplar with a six-value OccupantSize (2, 6, 3, 7, a value whose bit
    pattern is `0x01020305`, 9), no satisfaction, wealth 2, no crime: profile +0x04..+0x0C = 2, 6, 3; +0x10 = 0 (the satisfaction default
    overwrote value 4); +0x14 = `0x01020302` in the original (low byte the wealth, upper bytes from value 5),
    0x00000002 under FreeCity4's *[choice]*; +0x18 = 0.
14. **Generic lot configuration** (PL12). A building type whose slot 24 footprint is (16, 30): lot 2 × 2 cells,
    building object at (16, 0, 16), footprint min (8, 1), max (24, 31). Footprint (4, 4): lot 1 × 1, object at
    (8, 0, 8), footprint (6, 6)–(10, 10). Footprint (33, 47.5): lot 3 × 3.
15. **FixLotConfiguration** (PL13, PL14). A 2 × 2 lot (32 × 32 m) with one prop record, orientation 1, offset
    (10, 0, 12), footprint (9, 11)–(11, 13), ID list [P] where P has size (4, 1, 2): slot 26 gives (4, 2), swapped
    to (2, 4); old (2, 2) differs → footprint (9, 10)–(11, 14); inside the lot; offset inside → result 3. The same
    record with offset (31.5, 0, 12) and footprint (30.5, 11)–(32.5, 13), ID list [P'] with P' size (2, 1, 2): no
    rebuild; 32.5 > 31.95 + 0.001 → code 4; offset inside → result 4, record unchanged. Walls [W1, X, Y] where W1
    is a catalogued wall type and X, Y are not: X is erased, Y stays; with no earlier problem the result is 6.
