# Lot developer (`cSC4LotDeveloper`) — behaviour spec

Status: round 1 (M4), written from the Windows 1.1.641 executable (`bin/SimCity4.exe`). The Mac-based notes in
`docs/subsystems/development.md` §6 were the starting point only; every fact below was checked on the Windows code.
Corrections to that write-up and to other specs are listed in §12.3.

Tags:
- *[compat]*: observable, must match.
- *[choice]*: a FreeCity4 decision.
- *[inferred]*: not proven.
- *[quirk]*: original behaviour that looks unintended but is reproduced.

**Cited, not repeated here**
- foundations: streams and object references §2, city components and load order §3.6 / §3.7, response curves §4,
  messages §5, lot state and condition §5.2, random generators §6, C `rand()` and `random_shuffle` §12, slot-order
  rule §16, x87 precision §18.
- simulator spec: agent levels and re-delivery §4 / §5.3, the CPU-cycle time slicer §5.5.
- simgrid spec: grid ownership rule §6.4.
- allocators.md: allocator instances A / B / C, `GetLotsInCellRect`, STLport `hash_set` iteration order.
- `building-development.md` (BDS): building profiles (slot 39), construction resources (slots 50 / 51), cheats
  (slots 54–58), `AddNewBuildingOccupant` (slot 7), `GetLotDeveloper` (slot 36).
- `growth-developer.md`: `GetPurpose` (slot 7), `GetDesirabilityMap` (slot 13), `GetWaterSupplyCap` (slot 20).
- `tract-developer.md`: the callers of `StartConstruction`, `AbandonBuilding`, `ReoccupyBuilding`,
  `NeedsAnchorBuilding`, `GetAreaSlope`.
- `zones.md`: zone types, the lot manager and its lot lists.
- `advisors.md`: the `LotDeveloperNotification` message `0x4C04DE69`.

**Notation**
- `r24(·)` describes the ordinary 24-bit x87 context (foundations §18.3), not an implicit float store.
  Native re-entry uses live PC; at PC 53 preserve the original float stores and use 53-bit arithmetic between
  them ([foundations §18.4](foundations.md#184-lua-to-native-re-entry-uses-the-live-precision-vc29-pc), VC29-PC).
- `trunc(·)` is the CRT `_ftol` (`0x009EEF04`, mislabelled `_lua_gettop` in the symbol port): int64 truncation toward
  zero; callers keep the low 32 bits unless stated. NaN gives 0 (foundations §18.1).
- `Uniform(n)` is `cRZRandom::RandomUint32Uniform(n)` (foundations §6.1): one `NextRaw` step, `(raw · n) >> 32`;
  `n = 0` returns 0 without a step.
- **File RNG** = the file-local `cRZRandom` at `0x00B49AF0` (§10).
- Lot states (`cISC4Lot::HabitationState`): 0 created, 1 ready (empty), 2 constructing, 3 occupied, 4 vacant
  (abandoned), 5 destructing, 6 destroyed.
- Vacate reasons (`cISC4Lot::VacateReason`): 0 none, 1 low desirability, 2 no power, 3 no water, 4 commute time,
  5 kick out occupants, 6 low demand. The byte is written only on a change to state 4 and is **stale** in every
  other state (saved lots outside state 4 hold 98 different values, lot-records.md); it is saved as it is.
- Zone types (`cISC4ZoneManager::ZoneType`): 0 none, 1–3 residential, 4–6 commercial, 7 agriculture, 8–9
  industrial, 10 military, 11 airport, 12 seaport, 13 spaceport, 14 landfill, 15 plopped.
- Purposes (`BuildingProfile.purpose`): 0 none (civic / plopped), 1 residence, 2 services, 3 office, 4 tourism,
  5 agriculture, 6 processing, 7 manufacturing, 8 high tech.
- Lot properties used as per-lot counters and flags (all `uint32`, read through variant slot 73, written through
  variant slot 26; created with value 0 by holder slot 13 when a counter is first needed):

  | ID | Meaning (name *[inferred]*) | Written by |
  |---|---|---|
  | `0xC999E206` | low-desirability counter | TestDesirability |
  | `0xE999B11F` | no-power counter | TestPower |
  | `0x8999B20B` | no-water counter | TestWater |
  | `0x8999B10B` | travel counter (road access and commute) | TestTravel |
  | `0xC999C45E` | current zot: 0 none, 4 power, 5 water, 6 road, 7 commute | the zot helpers (§5.6) |
  | `0x48E95539` | (on a zot prop occupant) the zot number it shows | AddBuildingZotProp |

---

## 1. Role and identity

The lot developer looks after **lots that already exist**:

1. a **daily pass** that visits about 1/28 of all lots per day (§4) and, for each, re-tests desirability, power,
   water and travel, **abandons** buildings that keep failing, updates the lot's visual **condition**, shows or
   hides **zots** (the floating "no power / no water / no road / commute" icons) and maintains **business units**
   (groups of adjoining industrial or farm lots that share road access);
2. **construction** on behalf of the tract developer and others (`StartConstruction`, slot 6), the hand-off to the
   construction occupant, and the lot content (props, flora, base texture, network nodes) added when the building
   is finished (§6);
3. **demolition** and **explosion** of lots, including rubble (§7);
4. the **night lights**: buildings are switched lit / unlit in small batches on animation ticks after the day /
   night change (§8);
5. reaction to **power changes** in a cell rectangle (§9);
6. terrain helpers used by other subsystems: altitude, slope and flatness queries, levelling, foundations and
   development cost (§11, slot table).

### 1.1 Identity

| Item | Value |
|---|---|
| Class | `cSC4LotDeveloper`, class ID `0xA990BFE0` (`GetGZCLSID` `0x006C3B00`) |
| Object | `0x108` bytes, allocated with allocator C (`0x005E55E0`) by the factory `0x006C8DF0`; constructor `0x006C8650`; deleting destructor (slot 65, `0x006C8810`) destroys the night deque and frees with C (`0x005E5620`) |
| Primary interface | `cISC4LotDeveloper`, IID `0x4818E5D2`, vtable `0x00AB1B98` (65 slots + destructor) |
| `QueryInterface` (`0x006C3B80`) | `0x4818E5D2` and `1` (`cIGZUnknown`) → the object; `0x452294AA` (`cIGZMessageTarget2`) → object + 4; `0xE4FDA3D4` (`cIGZSerializable`) → object + 8; anything else → false. Each success AddRefs |
| Sub-objects | +4 `cIGZMessageTarget2` (vtable `0x00AB1B88`), +8 `cIGZSerializable` (vtable `0x00AB1B70`) |
| `GetSimulatorType` (slot 5) | returns `0x6818E5C9` |
| Owner | the city: component slot 16 (foundations §3.6), IID `0x4818E5D2`. The global pointer `0x00B43D20` is set by the city's init code (`0x00601CF4`) and cleared at shutdown (`0x00602033`); the BDS returns it as `GetLotDeveloper()` (BDS slot 36) |
| Agent name | `"cSC4LotDeveloper"` (string `0x00A89850`) |

### 1.2 Creation, Read and Init order

- The city creates the object through GZCOM (new city) or the class store creates it and calls `Read` (load); `Read`
  happens **before** `Init` (foundations §3.7). The lot manager and its lots are therefore already loaded when
  `Init` runs *[inferred from the foundations order]*.
- The constructor sets the defaults of §2 (daily state 3 = "start a cycle", both change latches set, tuning
  defaults of §3.5, an empty response curve, an empty boundary map).

### 1.3 `Init()` → bool (slot 3, `0x006CB9A0`)

If already initialised, return true at once. Otherwise:

1. Set the static byte `0x00B49AE8` to 1 on the first call (never read: dead) *[quirk]*.
2. Read the tuning (§3.5).
3. For each service pointer that is still null, take the global and AddRef it; **if any of them is null, return
   false** (nothing is subscribed): city `0x00B43CEC`, lot manager `0x00B43D08`, occupant manager `0x00B43D0C`,
   prop manager `0x00B43D10`, BDS `0x00B43D68`, power `0x00B43DA0`, plumbing `0x00B43D94`, flora `0x00B43D80`,
   terrain `0x00B43CF4`, traffic `0x00B43DA8`. The demolition utility `0x00B43D00` is taken without a null check.
4. Cell counts from the city: X = city slot 93, Z = city slot 94 (stored on every `Init`).
5. Only when the terrain pointer was taken in this call: `vertsX := terrain slot 7 + 1`;
   `extentX := r24(float(terrain slot 7) · CellWidth)`, `extentZ := r24(float(terrain slot 8) · CellWidth)`
   (`CellWidth` = terrain slot 6). After a load these overwrite nothing (the terrain pointer is null before the first
   `Init`, so the values from `Read` are replaced) *[inferred: `Read` runs on a fresh object]*.
6. Create the blocking filter (a `cSC4BlockingLotOccupantFilter`, 12 bytes from C, vtables `0x00AB1B28` /
   `0x00AB1B10`) if absent. It accepts occupant type `0x74758926` (flora) only (`0x006C3C20`).
7. Daily state := 0. Create a `ScanningIterator` (0x20 bytes from C, `0x0060CE90`) over the rectangle
   `{0, 0, CellCountX / 4 − 1, CellCountZ / 4 − 1}` (C division) if absent. It is never used afterwards (dead).
8. If the simulator global `0x00B43D5C` is null, return false. Otherwise register the agent (§3.1) at levels 0, 4
   and 6.
9. If the message server is not yet held: take `0x00B43CCC` (return false if null), AddRef it, and subscribe the
   nine messages of §3.2 in this order: `0x44778234`, `0x99EF1142`, `0x99EF1143`, `0xE8BDA2E7`, `0xE99B3D4D`,
   `0x278E6003`, `0x8A07AE00`, `0x26D31EC4`, `0x69247DC7`.
10. Register with the animation-tick manager (service `0x86AD10EC`, IID `0x86AD10ED`): its slot 5 with
    (this + 4, `0xC0`). `0xC0` is a **priority**: ≥ `0xC0` puts the target in the every-frame bucket (§6.7.8).
11. `InitBoundaryRules` (`0x006CA160`): fills the boundary map used by `GetBoundaryLotTexture` (slot 42). No random
    draw.
12. **Force a base-texture refresh**: flip the force-empty flag, then call `SetForceEmptyLotBaseTextures(original)`
    (slot 47). Because the value differs, slot 47 walks **every lot** (§6.6). With the default flag (false) this
    calls `SetNormalLotBaseTexture(lot, lot.GetLotConfiguration(), true)` on each lot, which **draws the file RNG**
    for every lot that has a configuration with texture objects (§10). On a new city there are no lots yet.
13. `canCheckCommute := false`, `noZotUntilDay := 0`, initialised := true. Return true.

### 1.4 `Shutdown()` → true (slot 4, `0x006C7380`)

Only when initialised: clear the flag; release every building in the night deque and clear it; `RemoveAgent(this+4)`
(simulator slot 20, all levels); unregister from the animation-tick manager (slot 6, (this + 4, `0xC0`));
unsubscribe the nine messages (message server slot 6) and release the server; release all services and the
blocking filter; clear the boundary map; unregister and free the safe lot iterator (§4.1) and delete the scanning
iterator (both C); release the BDS. Returns true. Because `Init` subscribes only when it holds no server, a later
`Init` subscribes again.

`Release` (slot 2, `0x006C49D0`): if the count is above 1, decrement; otherwise call `Shutdown` (slot 4) and the
deleting destructor (slot 65, flag 1). It returns 0 in both cases *[quirk: not the new count]*.

---

## 2. State

### 2.1 Persistent (saved, §13)

| Field | Type | Default | Meaning |
|---|---|---|---|
| `vertsX` | int32 | terrain cell count X + 1 | row stride for vertex altitude reads (slot 15) |
| `extentX`, `extentZ` | float32 | cell counts × cell width | city size in metres (saved; not read elsewhere in the class *[inferred]*) |
| night deque | `deque<cISC4BuildingOccupant*>` | empty | every building, one reference each (§8) |
| night cursor | index into the deque | 0 | next building of the current batch run |
| `nightMode` | uint32 (saved as uint8) | 0 | 0 idle, 1 switching to day, 2 switching to night |
| `litFlag` | bool | false | lit state given to newly inserted buildings |

### 2.2 Not saved

| Field | Default | Meaning |
|---|---|---|
| `dailyState` | 3 (constructor), 0 (`Init`) | 0 / 3 start a cycle, 1 in a cycle (§4.1) |
| lot iterator | none | the lot manager's safe iterator (§4.1) |
| `visited` | 0 | lots updated so far today |
| `netChanged`, `unitChanged` | true, true | set by network insert / remove messages (§3.2); snapshotted at the start of each cycle |
| `netSnap`, `unitSnap`, `zotsAllowed` | false | cycle snapshots: `netSnap := netChanged`, `unitSnap := unitChanged`, `zotsAllowed := today > noZotUntilDay` |
| `noZotUntilDay` | 0 | set to today + 31 by CityEstablished |
| `canCheckCommute` | false | set by TrafficMapChanged |
| `forceEmpty` | false | force-empty base textures (slots 46 / 47) |
| `nightAcc` | 200 (constructor), tuning `0xE900EA2B` | animation-time accumulator in ms (§8) |
| `nightBatch` | 8 (constructor), tuning `0xE900EA2C` | buildings per batch |
| tuning | §3.5 | |
| response curve | empty | development cost vs slope |
| boundary map | built by `Init` | boundary-type key → (texture, byte, bool) |

The RNG is not saved (foundations §6.2).

---

## 3. Agents, messages and tuning

### 3.1 Agent registration

`AddAgent(this+4, level, "cSC4LotDeveloper", flags 2)` (simulator slot 19) for levels **0, 4 and 6**. Flags 2: the
agent runs only once the city is established (simulator §4).

| Level | Message | `DoMessage` result | Effect |
|---|---|---|---|
| 0 (one-shot) | `0x66956810` | true (default branch) | nothing; the level-0 registration is removed after its first delivery. While it is registered, calendar time does not pass (simulator §5.2) |
| 4 (day) | `0x66956814` | the daily pass's result | §4 |
| 6 (month) | `0x66956816` | true (default branch) | nothing |

### 3.2 Subscribed messages (`DoMessage`, `0x006CD940`, sub-object +4)

All return true unless stated. Message slots: type 4, data1 9, data2 14, data3 19, data4 24, void1 10,
IGZUnknown 32 (foundations §5).

| Type | Name *[inferred]* | Handling |
|---|---|---|
| `0x44778234` | day / night changed | §8.2. data2 = 0 starts night, 1 starts day |
| `0x99EF1142` | occupant inserted | data3 = occupant type. Building (`0x278128A0`): add void1 to the night deque (§8.1). Network (`0xC772BF98`): `netChanged := unitChanged := true` |
| `0x99EF1143` | occupant removed | building: remove from the night deque (§8.1); network: both latches as above |
| `0xE8BDA2E7` | lot state changed | posted by `cSC4Lot::SetState(state, wealth, reason)` (`0x006C3100`, queued `MessagePost`, only when the state changes) with IGZUnknown = a **copy** of the lot, data1 = old state, data2 = new state, data3 = building type, data4 = the lot's **occupant wealth** (not the reason; corrected, see below). §6.5 |
| `0xE99B3D4D` | lot condition changed | posted by `cSC4Lot::SetCondition` (`0x006C26D0`). §6.5 |
| `0x278E6003` | terrain heights changed | reads data1, data2, data4; does nothing *[quirk]* |
| `0x8A07AE00` | power changed in a rectangle | rect = (data1, data2, data3, data4) = (x1, z1, x2, z2); §9 |
| `0x26D31EC4` | city established | `noZotUntilDay := GetSimDateNumber() + 31` (simulator slot 8) |
| `0x69247DC7` | traffic map changed | `canCheckCommute := (demand.GetDemand(0x2000, 0x20000).QueryDemandValue() > 0.0)` (demand simulator slot 6, demand slot 9; the city-total jobs node, demand.md). NaN → false |
| `0x86AD10EE` | animation tick (delivered by the animation-tick manager by a direct `DoMessage` call, not subscribed; §6.7.8) | if `nightMode = 0`: return **false**. Otherwise `ms := data2 / 1000` (unsigned integer division; data2 is the clamped frame delta in **µs**, 2000..500000 by default, so `ms` is 2..500) and run §8.3; return true |
| `0x66956814` | SimNewDay (agent) | §4; returns the pass's "finished" flag |
| any other | | true |

**What `cSC4Lot::SetState(state, wealth, reason)` puts in `0xE8BDA2E7`** *[compat]* (`0x006C3100`, pop 12; SDK
order `(HabitationState, WealthType, VacateReason)` is correct). Nothing is posted when `state` equals the current
state. The comparison uses the whole state argument against the current byte; the stored new state is
its low byte. Otherwise, with `old` the previous state and `W0` the lot's occupant wealth byte (lot slot 41)
sampled immediately before the outer wealth recomputation (after any nested population fill):

1. The new state is stored. During the mutation sequence below, if the new byte is 4, the low byte of
   `reason` becomes the lot's vacate reason (lot slot 45, `GetVacateReason`); `reason` is used for nothing
   else and never reaches the message.
2. Before the outer wealth update, the lot updates its state-change day, releases construction resources
   when leaving constructing, fills population on constructing → occupied, rebuilds zeroed population maps
   on entering constructing, or zeroes existing counts outside states 2/3. The complete branch/order,
   capacity clamp and wealth arithmetic are in lot-records.md §3.2.1–3.2.2 (LR10/LR11).
   `UpdatePrimaryOccupantWealth` (`0x006C1430`) then yields `W1`; no stored building type sets 0,
   whereas a nonzero unknown type's default purpose-zero profile leaves wealth unchanged.
3. `occOld := old ∈ {2, 3}`, `occNew := state ∈ {2, 3}`. data4 is:

| Case | data4 | Lot wealth afterwards |
|---|---|---|
| `occOld = occNew` | `W1` | `W1` |
| entering 2/3, `wealth ≠ 0` (whole 32-bit value) | low byte of `wealth` | low byte of `wealth` |
| entering 2/3, `wealth = 0` | `W1` | `W1` |
| leaving 2/3 | `W0` (sampled before outer recomputation) | `W1` |

So `demand.md` §7, `growth-developer.md` §6.3 and `building-development.md` §3.2 are right to read data4 as the
occupant wealth; when entering from outside 2/3, `ReoccupyBuilding`'s `SetState(3, w, 0)` delivers `w`
if nonzero. `AbandonBuilding`'s `SetState(4, 0, reason)` delivers W0. The IGZUnknown is a new `cSC4Lot` built by the copy
constructor `0x006C2450` at post time (it copies the scalar fields, including state, building type, wealth and
vacate reason, and references the configuration; two of its containers start empty: the **trip list** (lot
`+0x84`, the list `Write` iterates at `0x006C0305`) and a tree at `+0x40` *[inferred: the occupant set]*; the
containers at `+0x54` and `+0x60` are copied). The copy is taken after wealth/override and UpdateNeedsPower,
and clears unit/travel-designate references. The message retains the copy and the queue retains the message;
SetState releases its own message reference then copy reference after posting (lot-records.md §3.2.2).
This is why §6.5 works on the lot
found at the copy's location rather than on the message object.

### 3.3 Messages posted

| Message | When | Payload | Delivery |
|---|---|---|---|
| `0x4C04DE69` LotDeveloperNotification | `AddBuildingZotProp` with zot 6 (data1 = 1), `RemoveBuildingZotProp` with zot 6 (data1 = 2), `StartConstruction` when the lot has a zot property (data1 = 2) | data2–4 = 0; IGZUnknown = the lot | `SC4PostMessage2` (`0x005F98C0`): new standard message, `MessagePost(msg, false)` (queued) |
| `0xA6B79621` | after an immediate whole-city night-light update (§8.2) | type only | `MessagePost(msg, false)` |
| `0x09D8E0B6` (occupant message) | to night-light props during a light update (§8.4) | `PostOccupantMessage(0x09D8E0B6, 0)` (occupant slot 15) | |
| `0x99EF1145` (occupant message / occupant-manager change) | after a base-texture occupant is updated (§6.6) | `PostOccupantMessage(0x99EF1145, 0)` | |

The lot itself posts `0xE8BDA2E7` and `0xE99B3D4D` when the lot developer changes state or condition.

### 3.4 Message handling order matters

`0xE8BDA2E7` and `0xE99B3D4D` arrive **queued**, after the call that changed the lot has returned. The content and
texture work of §6.5 (and its draws) therefore happens when the message server dispatches them, not inside
`StartConstruction` or `UpdateLot` *[compat]*.

### 3.5 Tuning (`ReadSimulatorTunableValues`, `0x006C4A20`)

Exemplar key: type `0x6534284A`, group `0xE7E2C2DB`, instance `0xE8DA7677` ("Building Development",
`re/tuning/simulators.md`), fetched with resource manager `0x00B43CA8` slot 3 (IID `0xA52160F5`). If it is missing,
nothing is read and every value keeps its previous (constructor) value. Readers: uint16 `0x005FD420`, float
`0x005FD4F0`, uint32 `0x005FD480`, bool `0x005FD390`; each leaves the member unchanged when the property is absent.

| Property | Name | Type | Use | Code default | Shipped |
|---|---|---|---|---|---|
| `0x47E2C400` | Lot Developer Stay New Time | uint16 | days threshold in TestCondition (§5.5) | 90 | 90 |
| `0x47E2C401` | Lot Developer Occupancy Thresholds | float32 ×3 | [0] **abandon**: desirability threshold (§5.1); [1] **distress**: occupancy threshold; [2] **renew**: occupancy threshold for mismatched wealth (§5.5) | 0.2, 0.4, 0.6 (re-set to these before every read) | 0.3, 0.45, 0.5 |
| `0xE900EA24` | kMaxCellAltitudeDeltaForFlatness | float32 | `IsAreaFlat` (slot 25): the largest allowed difference from the rectangle's **first vertex**, not a max − min range (§17.2) | 1.0 | 1 |
| `0xE900EA25` | kMaxSlopeForFlatness | float32 | slope above which levelling / foundations are attempted (§6.1) | 1.0 | 1 |
| `0xE900EA26` | kDefaultLotFoundationType | uint32 | `AddLotFoundation` (slot 54) | `0x890B7312` | `0x890B7312` |
| `0x6916516B` | (unnamed) | uint32 | default building foundation type (slots 58 / 59) *[inferred]* | `0x890B7312` | absent |
| `0xE900EA27` | kShouldAttemptLotLevelling | bool | `StartConstruction` | true | 1 |
| `0xE900EA28` | kShouldAttemptInternalLotLevelling | bool | `StartConstruction` | true | 1 |
| `0xE900EA29` | kLotAutoLevelInternalAltitudeDelta | float32 | `LevelAreaInternalVertices` (slot 19): an interior vertex more than this above the target is levelled (§17.4) | 0.0 | 20 |
| `0xE900EA2A` | MaxAreaSlopeAllowedForLevelling | float32 | `AttemptAreaLevelling` callers | 0.0 | 40 |
| `0xE900EA2B` | Night Update Interval | uint32 | written into the night **accumulator**, not a threshold; the threshold is a hard-coded 200 *[quirk]* | 200 | 100 |
| `0xE900EA2C` | Night Update Count | uint32 | buildings per night batch | 8 | 10 |
| `0x2A2D7824` | Cost Multiplier vs lot slope | curve | `GetLotDevelopmentCost` (slot 32) | empty | (0,0) (0.555556,10) (1.11111,20) (1.66667,40) (2.22222,80) (2.77778,170) (3.33333,350) (3.88889,720) (4.44445,1470) (5,3000) |

- `0x47E2C401`: the three values are copied from the property's float data without checking its count
  *[quirk]*: a shorter array reads past its end. FreeCity4 uses the defaults for missing elements *[choice]*.
- `0x2A2D7824` follows the **keep** pattern (foundations §4.1): the curve object is constructed empty and loaded
  in place with `0x005F4620` only when the property exists. Missing property → previous segments (empty on the
  first load). The original evaluates an empty curve through a null segment and crashes; FreeCity4 returns 0
  (foundations §4.3) *[choice]*.
- `0xE900EA2D` is in the shipped exemplar but not read by this class.
- There is no getter for any of these members: `ReadSimulatorTunableValues` has one caller, `Init` (`0x006CB9C7`),
  and nothing else writes them. A shadow that wants the original's values reads the same exemplar with the same
  rules (LD30).

---

## 4. The daily pass (`DoDailySimulation`, `0x006CD770`) *[compat]*

### 4.1 Cycle

```
if dailyState ∈ {0, 3}:                               -- start a cycle
    if no iterator: iterator := lotManager.GetSafeLotIterator()      (slot 15)
    else:           iterator.cur := first node of the lot list
    n := lotManager.GetLotCount()                     (slot 16)
    if n ≤ 0: return true
    visited := 0
    netSnap := netChanged;  unitSnap := unitChanged;  netChanged := unitChanged := false
    zotsAllowed := (GetSimDateNumber() > noZotUntilDay)          (unsigned)
    dailyState := 1                                   -- falls through
if dailyState ≠ 1: return true
start the 5 ms slicer (simulator §5.5; statics: step 0x00B49B08, initial 1024; miss counter 0x00B49B0C; guard 0x00B49B10)
n := lotManager.GetLotCount()
if n ≤ 0: dailyState := 3; return true
perDay := n / 28 + 1                                  (signed integer division)
counter := 0
while visited < perDay:
    counter += step
    if (counter & 0x3FF) = 0 and the slicer has expired: break
    if iterator is at the end of the lot list: dailyState := 3; byte 0x00B49AE9 := 0; break
    lot := iterator.cur; advance;  UpdateLot(lot);  visited += 1
finish the slicer
if visited ≥ perDay: visited := 0; return true        -- finished for today
return false                                          -- re-delivered later the same day
```

- A `false` return makes the simulator deliver the same day message again before the day advances (simulator §5.3),
  so every day processes exactly `perDay` lots, except **on the day the end of the list is reached**: the pass then
  returns false with `dailyState = 3`, the re-delivery starts a new cycle (resetting `visited := 0` and re-taking the
  snapshots), and processes `perDay` more lots from the start. That day updates the remaining lots **plus** `perDay`
  lots *[quirk]*.
- The order of lots is the lot manager's internal lot-list order (`zones.md`). The time slicer only changes how much
  work is done per call, never which lots are visited or in what order.
- The iterator is the lot manager's **safe lot iterator**: an 8-byte block `{current node, owner}` from allocator C,
  registered in the owner's iterator list so that lot deletions can move it *[inferred]*. The lot developer walks the
  owner's `std::list` nodes **directly** (next = node[0], lot = node + 8, end = the owner's head node), see §14.

### 4.2 `UpdateLot(lot)` (`0x006CD3A0`, non-virtual, pop 4, AL = "state changed")

`(x, z) := lot.GetLocation()` (lot slot 21). `changed := false`.

1. **Network refresh**: if `netSnap`: `lot.UpdateRoadAccess()` (slot 30). Then, only if the static byte
   `0x00B49AE9` is set, and the lot has a building, no travel designate, no road access and zone 7–9:
   `RotateLotToFaceRoad(lot)`. The byte is only ever cleared (§4.1), so this branch is **dead** *[quirk]*.
2. `type := lot.GetBuildingType(false)` (slot 46).
3. **Developed lot** (`type ≠ 0`): `P := BDS.GetBuildingProfile(type)` (BDS slot 39; inserts a zero profile for an
   unknown type).
   - **Purpose 0** (civic / plopped):
     ```
     powered := (TestPower(lot, x, z) = 2)
     c := lot.GetCondition()
     c = 0:        if not powered: SetCondition(1); stop (no travel test)
     c = 1:        if powered: SetCondition(0)
     c ∈ {5, 6}:   SetCondition(0)
     if powered: TestTravel(lot, P, x, z)               (result ignored)
     ```
   - **Purpose ≠ 0** (grown building):
     ```
     s := lot.GetState();  new := s;  reason := 0
     if not TestDesirability(lot, x, z):  new := 4; reason := 1
     if TestPower(lot, x, z) = 0:         new := 4; reason := 2
     if not TestWater(lot, x, z):         new := 4; reason := 3
     if not TestTravel(lot, P, x, z):     new := 4; reason := 4
     if new ≠ s: lot.SetState(new, 0, reason); changed := true
     lot.SetCondition(TestCondition(lot, x, z))
     if BDS.CheatCoffeeBar() and P.purpose = 2: TestCoffeeBar(lot)
     ```
     All four tests always run, in this order; the **last** failing test gives the reason *[compat]*. Passing tests
     never restore a lot: only the tract developer re-occupies (slot 9).
4. **Empty lot** (`type = 0`), only when `zotsAllowed`:
   - if `lot.GetDaysSinceLastStateChange() > 30` (unsigned) and `canCheckCommute`: for zones 1–3, `TestTravel` with a
     temporary profile of purpose 1; for zones 4–6, purpose 3 (constructor `0x004591E0`, destructor `0x00459220`).
     Results ignored.
   - if `IsEmptyLotSuitableForZot(lot, x, z)`: `TestPower(lot, x, z)` (result ignored).
5. **Business unit** (§5.7): `u := lot.GetUnit()` (slot 25).
   - `u` set and `lot` is its first member: if `unitSnap`, `UpdateTravelDesignates(&u)` (slot 34); then, re-reading
     `u := lot.GetUnit()`, if `lot` is still its first member, `UpdateTravelPopulations(u)` (slot 35).
   - `u` set, not first member: nothing.
   - no unit: `lot.ResetTravelPopulation()` (slot 57), `lot.AddTravelPopulation(lot)` (slot 58); then if
     `lot.IsInLivableState()` (slot 37) and not `lot.HasRoadAccess()` (slot 28): `PickUnitToJoin(lot)` (draw, §10).
6. Return `changed`.

### 4.3 Abandonment reasons

| Reason | Set by | Rule |
|---|---|---|
| 1 low desirability | UpdateLot | TestDesirability failed (§5.1) |
| 2 no power | UpdateLot | TestPower returned 0: third consecutive unpowered visit (§5.2) |
| 3 no water | UpdateLot | TestWater failed: third consecutive dry visit (§5.3) |
| 4 commute time | UpdateLot | TestTravel failed: third visit without road access, or seventh occupied visit without a commute (§5.4) |
| 5 kick out occupants | tract developer through slot 8 | `tract-developer.md` §4.4 |
| 6 low demand | tract developer through slot 8 | `tract-developer.md` §4.19 |

"Visit" is one `UpdateLot`, about every 28 days per lot. `AbandonBuilding(lot, reason)` (slot 8) is exactly
`lot.SetState(4, 0, reason)`, so the state changes **synchronously** (answers tract-developer Q5). It has no defined
result (LD34): `SetState` returns nothing (lot-records.md §13.2) and `AbandonBuilding` hands back whatever the register
holds afterwards; the tract developer's two calls (the kick-out of a state-3 lot with reason 5 and the capacity reclaim with reason 6,
tract-developer.md §4.19) both ignore it.

---

## 5. Tests and helpers *[compat]*

### 5.1 `TestDesirability(lot, x, z)` → bool (`0x006C47C0`, pop 12)

```
n := counter 0xC999E206 (created as 0)
if not lot.IsInLivableState():                         counter := 0; return true
P := BDS.GetBuildingProfile(lot.GetBuildingType(false))
dev := BDS.GetDeveloper(BDS.GetDeveloperType(P.purpose, lot.GetOccupantWealth()))   (slots 30, 32)
if dev = null:                                         counter := 0; return true
if dev.GetPurpose() = 5:                               return true          -- counter untouched
v := dev.GetDesirabilityMap().GetCellValue(x, z)       (int16; growth developer slot 13, grid slot 7)
d := r24(float(v) · 0.01f)
if not (d < abandonThreshold):                         counter := 0; return true      -- NaN impossible
r := cRZRandom(seed = (x << 16) + z).RandomSint32RangeUniform(0, 7)        -- 0..6, one step
n' := n + 1;  counter := n'
return n' ≤ r + 2                                      (unsigned)
```

- The generator is a **temporary** `cRZRandom` constructed on the stack with the seed `(x << 16) + z` (32-bit wrap;
  `x`, `z` are the lot's `GetLocation` outputs). Its first draw is the same at every visit, so each lot location has a
  fixed tolerance of `r + 2` consecutive low-desirability visits (3 to 9) *[compat]*.
- With the shipped threshold 0.3: `v = 30` gives `d = 0.29999998 < 0.3` (**low**), `v = 31` gives 0.31 (not low).

### 5.2 `TestPower(lot, x, z)` → int32 0 / 1 / 2 (`0x006CABB0`, pop 12)

```
n := counter 0xE999B11F (created as 0)
b := lot.GetBuilding()                                 (slot 48)
needs := (b = null) or lot.GetNeedsPower()             (slot 65)
lot.GetBoundingRect(&r)                                (result unused)
powered := BDS.CheatIgnorePower() or not needs or power.IsPowered(cx, cz)    (BDS slot 55, power slot 37)
    (cx, cz) := building centre cell when b exists, else lot.GetLocation():
        (min, max) := b.AsOccupant().GetBoundingBox()   (occupant slot 10)
        cx := trunc(r24(r24(min.x + max.x) / r24(CellWidthX + CellWidthX)))   (city slot 91)
        cz := the same with z and CellWidthZ (city slot 92)
if powered ≠ lot.GetHasPower():                         (slot 66)
    lot.SetHasPower(powered)                            (slot 67)
    if not powered and lot.GetCondition() = 0: lot.SetCondition(1)
    if powered     and lot.GetCondition() = 1: lot.SetCondition(0)
if not powered (needs is then true):
    n' := n + 1; counter := n'
    AddBuildingZotProp(lot, 0x0FD10000, 4)
    return (n' ≤ 2) ? 1 : 0
counter := 0
if b ≠ null and zot = 4: RemoveLotProp(lot, 4); zot := 0
return 2
```

- 2 = powered, 1 = unpowered within the grace of two visits, 0 = unpowered for the third time or more.
- The zot prop is added on **every** unpowered visit (the priority rule of §5.6 makes repeats no-ops).
- An empty lot's power zot is **not** removed here when power returns (`b = null`); only the power-changed handler
  (§9) removes it *[quirk]*.

### 5.3 `TestWater(lot, x, z)` → bool (`0x006CAE30`, pop 12)

```
n := counter 0x8999B20B (created as 0)
needs := false
if 2 ≤ lot.GetState() ≤ 4:
    P := BDS.GetBuildingProfile(lot.GetBuildingType(false))
    if P.purpose = 0: needs := true
    else:
        dev := BDS.GetDeveloper(P.purpose, P.wealth)    (slot 33);  cfg := lot.GetLotConfiguration()  (slot 9)
        if dev and cfg: needs := cfg.GetGrowthStage() > dev.GetWaterSupplyCap()   (bytes, unsigned; cfg slot 11, growth developer slot 20)
watered := BDS.CheatIgnorePlumbing() or plumbing.IsCellWatered(x, z)          (BDS slot 54, plumbing slot 11)
if not watered and needs:
    n' := n + 1; counter := n'
    AddBuildingZotProp(lot, 0x1C430000, 5)
    return n' ≤ 2
counter := 0
if zot = 5: RemoveLotProp(lot, 5); zot := 0
return true
```

The water test uses the lot's location cell, not the building centre.

### 5.4 `TestTravel(lot, P, x, z)` → bool (`0x006CAFC0`, pop 16)

```
s := lot.GetState()
if s ∉ {1, 3, 4}: goto PASS
tripOK := true
if P.purpose = 1:
    road := lot.HasRoadAccess() and lot.HasJobAccess()                    (slots 28, 27)
    tripOK := (traffic.GetTripLengthMap().GetCellValue(x, z) ≠ 0)           (traffic slot 13, uint8 grid slot 7)
elif 5 ≤ P.purpose ≤ 8:
    road := lot.HasOrganicRoadAccess(P.purpose)                            (slot 29)
else:
    lot.GetJobs(&tmp)                                                      (slot 64, result unused)
    if not (GetTravelJobs(1) > 0 or GetTravelJobs(2) > 0 or GetTravelJobs(3) > 0): goto PASS     (slot 63, NaN → not > 0)
    road := lot.HasRoadAccess()
if road and tripOK: goto PASS
n' := counter 0x8999B10B + 1 (created as 0); counter := n'
if not road:
    AddBuildingZotProp(lot, 0x107A0000, 6);  return n' ≤ 2
if s = 3 and n' > 6: AddBuildingZotProp(lot, 0x1C440000, 7); return false
if s = 3 and n' > 2: AddBuildingZotProp(lot, 0x1C440000, 7); return true
goto PASS_KEEP_COUNTER
PASS:              if counter 0x8999B10B exists: counter := 0
PASS_KEEP_COUNTER: RemoveBuildingZotProp(lot, 6);  if zot = 7: RemoveLotProp(lot, 7); zot := 0
                   return true
```

- Road access and commute share one counter. No road: grace of 2 visits. A residence with a road but trip length 0:
  the commute zot appears from the 3rd visit and the building is abandoned on the 7th (occupied lots only).
- Purposes 2–4 and 0 with no travel jobs at any wealth pass without a road test.
- `PASS_KEEP_COUNTER` keeps the incremented counter *[compat]*.

### 5.5 `TestCondition(lot, x, z)` → condition (`0x006C4630`, pop 12)

```
s := lot.GetState();  c := lot.GetCondition()
if s = 3:
    P := BDS.GetBuildingProfile(lot.GetBuildingType(false))
    d := P.wealth − lot.GetOccupantWealth()             (int32)
    b := (c − 1) > d                                    (signed)
    if b or lot.GetDaysSinceLastStateChange() > stayNewTime:            (unsigned vs uint16)
        lot.GetOccupancy(&f)                            (slot 61, writes a float; see §11.4)
        if f < distress:            d += 1
        elif b and f < renew:       d += 1              (NaN: no change)
    if d > 2:  return 3
    if d = 0:  return (counter 0xE999B11F exists and ≠ 0) ? 1 : 0
    if d < 0:  return c
    return d + 1                                        (d = 1 → 2, d = 2 → 3)
if s = 4:
    if c = 0: c := 1
    if lot.GetDaysSinceLastStateChange() > stayNewTime and c ≤ 3: return c + 1
    return c
return c
```

- Occupied lots get 0–3: 0 normal, 1 normal but recently unpowered, 2 / 3 run down by a wealth mismatch or low
  occupancy. Abandoned lots step 1 → 2 → 3 → 4, one step per visit after `stayNewTime` days since the last state
  change (`SetCondition` on an abandoned lot restarts that count, foundations §5.2).
- UpdateLot always calls `SetCondition` with the result; `SetCondition` is a no-op when unchanged.

### 5.6 Zots

**`AddBuildingZotProp(lot, propType, zot)`** (`0x006C98C0`, pop 12):
1. `b := lot.GetBuilding()`, `cfg := lot.GetLotConfiguration()`, `obj := cfg ? cfg.GetPrimaryBuildingConfigurationObject() : null`.
2. If `zot = 6`: post `0x4C04DE69` with data1 = 1 (§3.3). This happens **before** the priority check *[quirk]*.
3. Zot property `0xC999C45E` (created as 0). If `cur ≠ 0` and `zot ≥ cur` (unsigned): return. Otherwise
   `RemoveLotProp(lot, cur)` and set the property to `zot`. **Lower numbers win**: power 4 < water 5 < road 6 <
   commute 7.
4. Position: when `b`, `cfg` and `obj` exist, x and z from `GetPositionForBuilding` (slot 40) and `y := b`'s bounding
   box max y; otherwise the lot centre (`lot.GetCenter`, slot 15). Then `y := r24(y + 3.0)`.
5. Orientation: field `+0x10` of the global `SC4ViewUtilities` object `0x00B43DD8` (read at `0x006C99ED`), the
   **current view rotation** 0…3 (§12.2 Q3; landfill.md §4.7.5).
6. `occ := propManager.AddLotProp(propType, &pos, orientation)` (prop manager slot 26; not null-checked);
   `occ.SetRemovalFlags(3)` (occupant slot 12); add property `0x48E95539 = zot` to `occ`.

**`RemoveBuildingZotProp(lot, zot)`** (`0x006C9A40`, non-virtual, pop 8): if `zot = 6`, post `0x4C04DE69` with
data1 = 2. Then if `zot = 0` or the current zot equals `zot`: `RemoveLotProp(lot, cur)` when `cur ≠ 0`, and set the
property to 0. Slot 11 (`0x006CB940`) removes whatever zot is shown.

**`RemoveLotProp(lot, zot)`** (`0x006C9780`, pop 8): gets the lot's props (prop manager slot 24, a fresh A list),
removes **every** prop whose `0x48E95539` equals `zot` (prop manager slot 29), and returns true if any was removed.

**`IsEmptyLotSuitableForZot(lot, x, z)`** (`0x006C3B10`, register arguments: lot in ECX, z in EAX, x in EDX):
zones 1–6 → `(x + z) mod 4 = 0`; zones 7–9 → `x mod 4 = 0 and z mod 4 = 0`; other zones → false (C remainder). Empty
lots therefore show zots on a sparse diagonal (R, C) or grid (I, A) pattern.

### 5.7 Business units

**`PickUnitToJoin(lot)`** (`0x006CB260`, pop 4):
1. Return if the lot already has a unit, if `lot.GetBuildingType(true) = 0`, or if the profile's purpose is not
   5–8.
2. `L :=` a B list filled by `lotManager.GetLotsSurroundingLot(lot, L, f)` (slot 9) for `f` = 1, 2, 4, 8 in that
   order (appending). **All four calls append to the same list**, which is allocated once (`0x0090CF54`, 12-byte
   head) before the first call (`0x006CB2D0`–`0x006CB2FE`).
   - Slot 9 (`0x0072A750`) is a thunk: it takes the lot's bounding rectangle (lot slot 14) and delegates to
     `GetLotsSurroundingRect` (slot 10, `0x0072E2B0`).
   - `GetLotsSurroundingRect` (`0x0072E2B0`) walks the cells just outside the rectangle with a `BoundaryIterator`,
     and **before appending each candidate lot it scans the output list for that lot pointer and skips it if already
     present**: it `AddRef`s the candidate, walks the list while `node[2] != lot`, `Release`s it, and appends a new
     12-byte node **only when the walk reached the list head**. So a corner lot touching two sides is appended once.
   - **Therefore a lot can never appear twice in `L` across the four side flags**, and no neighbour is
     double-weighted by step 4's `Uniform(count)` pick *[answers Q5]*.
   - The same function drops sides the flags exclude. With `(x1, z1, x2, z2)` the lot rectangle and a cell `(x, z)`,
     the tested sides and their bits are: bit 0 (flag 1) = `z = z1 − 1`, bit 1 (2) = `x = x2 + 1`, bit 2 (4) =
     `z = z2 + 1`, bit 3 (8) = `x = x1 − 1`. A side lying outside the city clears its own bit before the walk
     (`x1 − 1 < 0` clears bit 3, `z1 − 1 < 0` clears bit 0, `x2 + 1 ≥ cellCountX` clears bit 1,
     `z2 + 1 ≥ cellCountZ` clears bit 2). With all four bits set the side test is skipped entirely (`flags == 0xF`).
     The function returns the number of lots appended.
3. Candidates (a B vector, list order): neighbours with a building type, with (state > 1 or a unit), and with the
   same purpose; for purpose 5 the neighbour must have a building (slot 48), for 6–8 it must have road access or a
   travel designate with road access.
4. If there is at least one candidate: `c := cand[Uniform(count)]` (**file RNG**); `u := c.GetUnit()`, or a new
   `cSC4BusinessUnit` (0x20 bytes from C, constructor `0x006C0800`) with `u.Add(c)`; then `u.Add(lot)`.
   - **The new unit** (LD13) is class `0x4A232DA8` (`GetGZCLSID` `0x006BF380`), primary interface
     `cISC4BusinessUnit`, IID `0x6A233069` (vtable `0x00AB18DC`: 3 `Add`, 4 `Remove`, 5 `GetMembers`).
     `QueryInterface` (`0x006BF390`) answers `1`, `0x6A233069` → the object, and `0xE4FDA3D4`
     (`cIGZSerializable`) → object + 4; anything else is false. The executable registers the class in the COM
     directory (factory `0x006C0EB0`, registration `0x0046046C`, the same 0x20-byte object), so the save loader can
     build units.
   - The lot developer itself calls neither the COM directory nor `AddRef`: the object starts with a reference count
     of 0 (`+0xC`), and the member lots hold the only references (`Add` → `lot.SetUnit(u)`, lot slot 26, AddRefs).
     `Release` (`0x006BFE40`) destroys it when the count is 1 or less.
   - A replacement creates the same object with the framework's `CreateInstance(0x4A232DA8, 0x6A233069)`. That
     returns one reference: add `c` and `lot`, then release it, which leaves the count where the original's is
     (one per member).
5. Purpose 5: `lot.SetTravelDesignate(first member of u)` (slot 60). Otherwise `unitChanged := true`.

**`UpdateTravelDesignates(u&)`** (slot 34, `0x006CB4D0`, pop 4, no defined return):

`anchor :=` the first member (in member-list order) with road access (lot slot 28) **and** a building (lot slot 48);
when found it is AddRef'd into a one-element B vector (`0x006CB528`–`0x006CB56A`).

**With an anchor** — for each member lot `L` in member-list order, let `d := L.GetTravelDesignate()` (lot slot 59,
`0x0081F910` = `mov eax,[ecx+0x50]; ret`), and let `notMember := (d ≠ null) and (d is not in u's member list)`:

| Case | Action |
|---|---|
| `notMember` | `L.SetTravelDesignate(0)`, then the anchor step |
| `d = null` | **no clear at all** (`0x006CB603` jumps straight past it), then the anchor step |
| `d` is a member of `u` and `d.HasRoadAccess()` | **keep** — no clear, no anchor step (`0x006CB60E`) |
| `d` is a member of `u` and not `d.HasRoadAccess()` | `L.SetTravelDesignate(0)`, then the anchor step |

The **anchor step** is `if L ≠ anchor: L.SetTravelDesignate(anchor)` (`0x006CB61C`–`0x006CB62B`).

**The anchor itself is never given a designate, and in the usual case is not cleared either** *[compat]* (answers
§12.2 Q9). When the anchor's own designate is null — the state this function always leaves it in, and the state a
freshly built unit has — the `d = null` row applies and control jumps over the clear: **no `SetTravelDesignate(0)`
call is issued for the anchor**. `SetTravelDesignate` (`0x006BF660`, pop 4) is a pure reference-counted store with an
`old == new` early-out and **no other side effect** (no message, no dirty flag), so "clear unconditionally" and
"leave untouched" are indistinguishable while the designate is null: the early-out makes the clear a no-op and no
reference moves. An implementation that clears unconditionally is therefore safe **as long as it reproduces the
`keep` row above**, which is the one case where the original deliberately does not clear.

The anchor *can* hold a non-null designate, and then the rows do differ: if the previous run's anchor `A` is still a
member with road access, the new anchor `B` (whose designate is `A`) takes the **keep** row and ends up with a stale
designate pointing at `A`; if `A` has lost road access or left the unit, `B` is cleared to null. Both are correct
reproductions of the table; neither is "the anchor gets no designate" in the sense of "the anchor's designate ends up
null".

**Without an anchor** the unit is **dissolved** (`0x006CB648`–`0x006CB6E0`):

1. `M :=` a **snapshot copy** of the member list — a fresh B `std::list` (head `0x0090CF54`) filled by `0x006C6CA0`,
   one AddRef'd reference per member, in member order.
2. For each `L` in `M`, in order: `u.Remove(L)` (unit slot 4), `PickUnitToJoin(L)` (draws R2), then
   `u' := L.GetUnit()`. If `u' = null`, continue with the next `L`.
3. As soon as some `L` yields `u' ≠ null`, migrate: **for every `X` in `M`, in copy order — including `L` itself** —
   `if X.GetUnit() ≠ null: X.GetUnit().Remove(X)`, then `u'.Add(X)`. Then `*param_2 := u'`.
4. If no `L` ever joins, `*param_2 := null`.
5. Release every lot of `M` and free its nodes and head with B (`0x0090CF63`).

**The original does *not* skip the just-joined member** *[compat]* (answers §12.2 Q10; this contradicts the reading
"move every other lot of the copy"). `0x006CB6B1`–`0x006CB6DA` re-reads `M` from its head and applies
remove-then-add to **all** of it, so `L` is removed from `u'` and immediately re-added. The exact set migrated is
`M` (every member the unit had at snapshot time), in `M`'s order; `L` is not excluded and not moved to the front.

Observable consequences of the remove-and-re-add — an implementation that skips `L` differs on all three:

- **`L`'s travel designate is destroyed.** `cISC4BusinessUnit::Remove(lot)` (`0x006C08B0`, pop 4, `RET 0x4` at
  `0x006C08F8`) does *not* only unlink: after the erase helper `0x006C0900` and the guard Release it calls
  **`lot.SetUnit(0)`** (lot slot 26, `CALL [EAX+0x68]` at `0x006C08E7`) and **`lot.SetTravelDesignate(0)`** (lot slot
  60, `CALL [EDX+0xF0]` at `0x006C08F0`). So for a **purpose-5 (farm)** unit, where `PickUnitToJoin` step 5 has just
  set `L.SetTravelDesignate(first member of u')`, the migration wipes it and `Add` never restores it (`Add` sets only
  the unit pointer, `CALL [EDX+0x68]` at `0x006C09CB`): the dissolved farm unit ends with **`L`'s designate null**.
  A skip-the-joined-member implementation leaves it pointing at `u'`'s first member.
- **Member order inside `u'`** differs when `L` is not `M`'s first element. The single migration pass appends in `M`
  order, so `L` is unlinked and re-appended at its `M` position: with `M = [M1, M2, L]` and `L` the joiner,
  `u' = [c, M1, M2, L]` (`c` the candidate `PickUnitToJoin` paired with), whereas skipping `L` gives `[c, L, M1, M2]`.
  Member order is observable: it decides which lot is the unit's **first member**, i.e. the anchor candidate that
  `UpdateLot` step 5 and `UpdateTravelPopulations` act on, and the order a later `UpdateTravelDesignates` walks.
- **Reference-count churn**: `Remove` takes a guard AddRef and drops it, and `Add` AddRefs for the node and releases
  its temporary, so `L`'s count nets out unchanged and never transiently reaches 0. No message is posted by `Add` or
  `Remove`, so there is no extra message traffic. `Add` (`0x006C0940`, pop 4) cannot duplicate `L`: it walks the member
  list first and, when the lot is already a member, skips the append (`JZ 0x006C09C6`) while still calling
  `lot.SetUnit(this)` — so the re-add is a no-op on the list and only re-points the unit.

`cISC4BusinessUnit` slot facts used above: 3 `Add(lot)` `0x006C0940` pop 4 (AddRef, append a 12-byte B node, then
`lot.SetUnit(this)`); 4 `Remove(lot)` `0x006C08B0` pop 4 (unlink, then `lot.SetUnit(0)` and
`lot.SetTravelDesignate(0)`); 5 `GetMembers()` `0x008085F0` pop 0 (`lea eax,[ecx+0x1C]; ret` — the member list head,
no reference).

**`UpdateTravelPopulations(u)`** (slot 35): `ResetTravelPopulation` on every member, then each member adds its travel
population to its designate (or itself).

**`RemoveLotFromItsBusinessUnit(lot)`** (slot 33): `lot.GetUnit().Remove(lot)` (no null check),
`unitChanged := true`, returns `Remove`'s AL.

**`NeedsAnchorBuilding(rect, purpose)`** (slot 36, pop 8, AL) — answers tract-developer Q1:
- purpose not 6–8 → false;
- any lot in `GetLotsInCellRect(rect, false)` whose building is in **occupant group `0x3000`** (occupant slot 22) →
  true;
- else, any lot from `GetLotsSurroundingRect(rect, f)` (lot manager slot 10, f = 1, 2, 4, 8) with a building type,
  (state > 1 or a unit) and the same purpose → false (an existing neighbour can serve as the unit's anchor);
- else true.

### 5.8 `TestCoffeeBar(lot)` (`0x006C78D0`, cheat only)

Called from UpdateLot when BDS slot 58 (CheatCoffeeBar) is on and the purpose is 2:
1. `r := Uniform(100)` (**file RNG**, always drawn first).
2. Stop if the lot is historical (slot 43), its state is not 3 or 4, `r ≥ 9`, its configuration is null or is
   `0x60000C40`, its growth stage > 3, or its size differs from configuration `0x60000C40`'s
   (lot configuration manager slot 8).
3. `demolition.DemolishLots(1, [lot], 0, 0x800, 0, 0, 0, 0)` (a B `std::list<cRZAutoRefCount<cISC4Lot>>` built and
   freed by the caller).
4. If `BDS.ClaimConstructionResources(profile(0x05B80000).capacity)`: `StartConstruction(lot, cfg 0x60000C40,
   0x05B80000, false)` and `lot.SetHistorical(true)`.

---

## 6. Construction *[compat]*

### 6.1 `StartConstruction(lot, cfg, type, animate)` → bool (slot 6, `0x006CC490`, pop 16)

```
lot.AddRef();  ok := false
lot.SetLotConfiguration(cfg); lot.SetBuildingType(type)                 (slots 10, 47)
lot.SetState(2, 0, 0); lot.SetCondition(1)                               -- condition 1 while building
PickUnitToJoin(lot)                                                      -- may draw
lot.SetFlags(lot.GetFlags() & ~7);  lot.SetVerticesAreLocked(true)       (slots 12, 13, 69)
if the lot has zot property 0xC999C45E:
    post 0x4C04DE69 (data1 = 2, lot)                                     -- whatever the zot value
    if zot = 6: RemoveLotProp(lot, 6); zot := 0                         -- other zots stay *[quirk]*
if type = 0: ok := true; goto END
notFarm := (BDS.GetBuildingProfile(type).purpose ≠ 5)
water := cfg.IsCompatibleWithWater()                                     (cfg slot 14)
found := false
if water and cfg.GetMaximumSlopeBeforeFoundation() = 0.0:                (cfg slot 10)
    found := AddLotFoundation(lot, cfg, false, true, true)               (slot 54, may draw)
if not found:
    if notFarm and internalLevelling and lot is wider and deeper than 2 cells (x2 − x1 > 1 and z2 − z1 > 1):
        LevelAreaInternalVertices(rect, −1.0, water, false, lot)        (slot 19)
    s := GetLotSlope(lot, water)                                         (slot 30)
    m := cfg.GetMaximumSlopeBeforeFoundation()
    if s > maxSlopeForFlatness and s > m:                                (NaN: no)
        if notFarm and lotLevelling and AttemptAreaLevelling(rect, m, true, water, maxAreaSlopeForLevelling, lot):   (slot 21)
            s := GetLotSlope(lot, water)
        if s > m: found := AddLotFoundation(lot, cfg, false, true, true)   (slot 54, may draw)
ClearLotBlockingObjects(lot, cfg, type, found)                           -- §6.2
GetPositionForBuilding(lot, &pos, &orient := 0)                          (slot 40)
b := BDS.AddNewBuildingOccupant(type, &pos, orient)                      (BDS slot 7)
if b = null: goto END                                                    -- ok stays false
if not found:
    so := GetOccupantSlope(b, water)                                     (slot 31)
    if so > maxSlopeForFlatness:
        if AttemptAreaLevelling(GetOccupantCellCoverage(b), maxSlopeForFlatness, true, water, maxAreaSlopeForLevelling, lot):
            so := GetOccupantSlope(b, water)
        if so > maxSlopeForFlatness: AddBuildingFoundation(b, cfg)       (slot 58)
if animate: construction-occupant hand-off (§6.3)
ok := true
END:
if not animate: lot.SetState(3, 0, 0)            -- even when no building was created *[quirk]*
lot.Release();  return ok
```

### 6.2 `ClearLotBlockingObjects(lot, …)` (`0x006C9AD0`, pop 16)

Bulldozes flora on the lot: box from `(x1·16 + 0.5, 0, z1·16 + 0.5)` to `((x2+1)·16 − 0.5, top, (z2+1)·16 − 0.5)`
with `top := r24(GetLowestOrHighestAreaAltitude(rect, highest, true) + 32.0)` (slot 17); the hard-coded 16 is the
cell size in metres. Occupants from `occupantManager.GetOccupantsByBBox(list, min, max, blockingFilter, −1)` (slot
36; the filter keeps flora `0x74758926`) are each demolished with `demolition.DemolishOccupant(1, occ, 0, 1, 0, 0, 0)`
(demolition slot 8) and released. The other three arguments are not used.

### 6.3 Construction-occupant hand-off

With `animate = true`:
1. `GZCOM()->CreateInstance(0xA97F909E, 0x496E7636)` (`cSC4ConstructionOccupant`; `0x006CC7E9`); if that fails, a
   direct `new cSC4ConstructionOccupant` (0x1A0 bytes, constructor `0x005D00F0` with argument 1) is AddRef'd.
   The occupant does not come from the BDS. The executable registers the class (factory `0x005D0BC0`,
   registration `0x004607EA`), so `CreateInstance` fails only when something has replaced the class and refuses;
   the fallback is never reached in the shipped game (LD25).
2. `co.SetLot(lot)` (its slot 4, `0x005D0890`): it builds the construction scene and records the start day.
3. `occupantManager.InsertOccupant(co.AsOccupant(), 0)` (occupant manager slot 20); release `co`.

The lot stays in state 2. The construction occupant's daily update (`0x005D0680`) runs its own phases; when it
completes, it sets the building completion, removes itself (occupant manager slot 21) and calls
**`lot.SetState(3, 0, 0)`**. Its phases, timing and draws are given in §6.7. The lot developer only reacts to the
resulting state-change message (§6.5).

Without animation the state is set to 3 at once, so §6.5 runs when the queued message is delivered.

### 6.4 Other construction slots

- **`ConstructDefaultLot(lot, cfg, type, animate)`** (slot 7, `0x006CCEF0`, pop 16): when `cfg` is set and
  `type = 0`, takes the primary building object's ID list; builds a `hash_set` (hint 100 → 193 buckets): each ID that
  is a valid building type (BDS slot 15) is inserted, otherwise its family members are added (BDS slot 19). If the set
  is not empty, `type :=` its **first element in STLport iteration order** (allocators.md, *hash_set*). Then returns
  `StartConstruction(lot, cfg, type, animate)`. No random draw.
- **`ChangeBuilding(lot, type)`** (slot 10, `0x006C5210`, pop 8): no-op when the lot has no configuration (returns
  0). Otherwise `ClearLotBuildings(lot, 1, true, 0)` (§7.1), place `type` with BDS slot 7 at `GetPositionForBuilding`,
  and, when the lot has no foundation (flags & 3 = 0), the occupant-slope levelling / `AddBuildingFoundation` step
  of §6.1. Finally `lot.SetState(3, 0, 0)` (also when no building was created *[quirk]*); returns whatever `SetState` leaves (no defined flag, see LD34). The lot's
  building type is not changed here.
- **`AbandonBuilding(lot, reason)`** (slot 8, `0x006C3D30`): `lot.SetState(4, 0, reason)` and nothing else; **no defined
  result** (LD34): the function returns straight after the call, so AL is whatever `SetState` left, which is not a
  success flag. Do not forward or test it.
- **`ReoccupyBuilding(lot, devType)`** (slot 9, `0x006C3D50`, pop 8): `w := BDS.GetWealth(devType)` (slot 31);
  `d := profile.wealth − w`; condition := 2 if `d = 1`, 3 if `d = 2`, otherwise (counter `0xE999B11F` exists and ≠ 0 ?
  1 : 0); `lot.SetState(3, w, 0)`; `lot.SetCondition(condition)`;
  `lot.SetPopulation(devType, lot.GetAvailableCapacity(devType, 0))` (slots 52, 55); returns its AL.
  Slot 55 explicitly defines AL as whether a profile capacity map contained devType (lot-records.md §3.2.1);
  slot 34 defines no success boolean even though the other wrappers pass AL through.
- **`UpdateLotElements(lot, uint32 kind)`** (slot 37, `0x006CC880`, pop 8, AL = 1) *[compat]*. The second
  argument is the BDS's 32-bit terrain-change kind (`building-development.md` §4.11), **not** a bool (corrected;
  the SDK's `uint32_t` is right). Its only use is a full 32-bit compare `kind ≠ 5` (`0x006CC8C7`); no byte of it is
  tested on its own. Behaviour: state < 2 → nothing. `added := false`. If the lot has a foundation (flags & 3):
  `UpdateLotFoundation` (61) and `UpdateLotRetainingWalls` (62). Otherwise, when `kind ≠ 5` and a configuration
  exists (lot slot 9): `s := GetLotSlope(lot, cfg.IsCompatibleWithWater())`; if
  `cfg.GetMaximumSlopeBeforeFoundation() < s` (NaN → no), `AddLotFoundation(lot, cfg, cfg.HasObjectsOfType(7),
  false, true)` (slot 54, may draw) and `added := true`; if `|s| > 0.25` (float `0x00A8821C`; NaN → no),
  `UpdateLotObjectAlitudes`. Then `UpdateLotBaseTextureOccupant(lot, added)` (slot 45: its result is not a defined flag, see §6.6) and lot slot 20 (`UpdateSlope`). The original pushes `added` as a dword whose upper three
  bytes are leftover stack contents; slot 45 reads only the low byte. The kind-5 sender is not identified here
  (the power simulator also ignores kinds 4 and 5, `power.md`).

### 6.5 Lot-state and condition messages

**`0xE8BDA2E7`** (old = data1, new = data2): let `a := new ∈ {3, 4}` and `b := (old = 2 and new = 3)`. If neither,
nothing. Otherwise `L :=` the message lot (QueryInterface `0xE818FCED`), `(x, z) := L.GetLocation()`, and
`L2 := lotManager.GetLot(x, z, true)` (slot 6). Continue only if `L2` has a configuration and the **same size** as
`L` (lot slot 22) *[quirk: the work is applied to the lot found at that location]*:
1. if `a`: `SetNormalLotBaseTexture(L2, cfg, true)` (slot 44; draws);
2. if `b`, in this order: `AddLotProps(L2, cfg, 1)` (draws), `AddLotFlora(L2, cfg, 1)` (draws),
   `AddNetworkConnectionNodes(L2, cfg, 1)`.

**`0xE99B3D4D`**: `L2 := lotManager.GetLot` at the message lot's location; if its state is 3 or 4:
`SetNormalLotBaseTexture(L2, L2.GetLotConfiguration(), true)` (draws). Every condition change of an occupied or
abandoned lot therefore re-picks its base texture.

**`AddNetworkConnectionNodes(lot, cfg, unused)`** (`0x006C9C80`, non-virtual, pop 12) (LD17):
1. `objs := cfg.GetLotConfigurationObjectArray()` (cfg slot 23); `L :=` an A list (framework allocator service).
2. For each object of **type 7** (network), in array order, with an ID list of at least 3 entries (objects with
   fewer are skipped):
   - `(pos, orient) := slot 39(lot, cfg, obj)`; `cell := (floor(pos.x / 16), floor(pos.z / 16))` (the same
     rounding as `city.PositionToCell`, §17.2);
   - `a := ids[0]`; `b := rotl32(ids[1], 8·orient)`, `c := rotl32(ids[2], 8·orient)` (no rotation for orient 0);
     `d := ids[3]` when there are at least 4 IDs, else 0;
   - `networkManager.CreateConnectionNode(&n, a, &cell, b, c, d, orient)` (`SL::NetworkManager()` `0x00602680`,
     network manager slot 20, `0x00611300`, pop 28); `n` is appended to `L` whatever the call returned
     (`0x006C9EE4`). `n` is one stack slot for the whole loop, never cleared, and `CreateConnectionNode` writes it
     only on success (`0x006114DE`, followed by an AddRef). After a failure the original therefore appends the
     **previous** object's node a second time, or, for the first network object, an uninitialised value
     *[quirk]* (LD32). `AutoconnectLotNodes` QueryInterfaces every entry without a null test (`0x006115F7`), and
     slot 48 releases every entry, so a repeated node is released once more than it was referenced.
     *[choice]* FreeCity4 appends only the nodes that were created and counts the failures (`nodesFailed`).
3. `networkManager.AutoconnectLotNodes(lot, &L)` (slot 21, `0x00611530`, pop 8); then occupant manager slot 48 on
   `L` (releases the entries) and the list is freed.
The third argument is never read. The network manager's side (what a connection node is: flag `0x200000`, zones.md
§8.4.1) belongs to the network manager spec.

**`RemoveNetworkConnectionNodes(lot)`** (`0x006C9FB0`, non-virtual, pop 4) (LD17): `r := lot.GetBoundingRect()`;
`L :=` the network occupants (`0xC772BF98`) in `r` (occupant manager slot 41, flags −1; nothing more when it
returns false). Each entry is QueryInterfaced for `cISC4NetworkOccupant` (`0xA821EF94`, not null-checked); entries
without every bit of `0x200000` (network occupant slot 8) are released and unlinked. The remaining connection nodes
go to `networkManager.DestroyNetworkOccupants(&L)` (slot 6), then occupant manager slot 48 and the list is freed.

**`UpdateLotObjectAlitudes(lot)`** (`0x006C91F0`, non-virtual, pop 4, AL = 0) (LD17), called by slot 37 (§6.4) when
`|slope| > 0.25`:
1. `L := lot.GetLotOccupants()` (lot slot 49; an A list of referenced occupants).
2. For each occupant `o` in list order, with `t := o.GetType()` (occupant slot 7):
   - `t` is building `0x278128A0`, prop `0xA823821E` or flora `0x74758926`:
     - `P := o.GetPosition()` (occupant slot 8); `cfg := lot.GetLotConfiguration()`;
       `obj := lot.GetLotConfigurationObject(o)` (lot slot 11);
     - target `Q`: with `obj`, `Q := slot 39(lot, cfg, obj).pos`; without, `Q.y := terrain.GetAltitude(P.x, P.z)`
       (`cISTETerrain` slot 13);
     - if `|P.y − Q.y| > 0.1` (float `0x00A8C950`; strict, NaN no): `P.y := Q.y` (**only y moves**; x and z stay
       the occupant's own); `o.SetPosition(P)` (occupant slot 9); `occupantManager` slot 24 `(o, 1)`;
     - then, still only when it moved (the `0.1` test at `0x006C9326` skips this too) and for a building only
       (`0x006C934B`): `f := GetFoundationByOccupant(o)` (slot 48); if `f`: `ft := f.GetFoundationType()`
       (foundation slot 4) and `RemoveBuildingFoundation(o)` (slot 60). Then if `IsLotObjectFoundationRequired(o)`
       (slot 56): QueryInterface `o` for `0x87DDA3A9` and call `AddBuildingFoundation(b, ft)` (slot 59) when there
       was a foundation, else `AddBuildingFoundation(b, cfg)` (slot 58); release `b`.
   - `t` is a network occupant (`0xC772BF98`): a QueryInterface for `0xA821EF94` that is released at once (no effect).
   - Release `o`.
3. Free `L`.

**`AddLotProps(lot, cfg, 1)`** (`0x006C7860`): for each configuration object (0x34-byte records,
`GetLotConfigurationObjectArray`, cfg slot 23) of type 1 whose flag byte has bit 0 clear, in array order:
`CreateRandomLotProp(lot, index)` (`0x006C6D70`): position from slot 39; `ids :=`
`lotConfigurationManager.GetObjectIDArray(obj, ids)` (slot 27; families expanded in hash order); if not empty,
`id := ids[Uniform(n)]` (**file RNG**); if `id ≠ 0`, `occ := propManager.AddLotProp(id, &pos, orientation)` (slot 26)
and `occ.SetLotTag(obj's tag)` (occupant slot 28).

**`AddLotFlora(lot, cfg, 1)`** (`0x006CD2B0`): for each object of type 4 in array order: position from slot 39;
`t := PickFloraTypeForLot(obj's ID list, &pos)`; if `t ≠ 0`, `occ := flora.AddNewFloraOccupant(t, &pos, false)`
(flora slot 17) and `occ.SetLotTag(obj's tag)`.

**`PickFloraTypeForLot(ids, pos)`** (`0x006CD010`): empty → 0. `id := ids[Uniform(n)]` (**file RNG**). If
`flora.IsFloraTypeValid(id)` (slot 11) → `id`. Otherwise fill a `hash_set` (193 buckets) with
`flora.GetFloraTypesByFamily(set, id)` (slot 14); if that returns 0 or the set is empty → 0; otherwise
`k := Uniform(size)` (**file RNG**) and return the element reached after `k` steps of the set's iteration order.

### 6.6 Base textures

**`SetNormalLotBaseTexture(lot, cfg, flag)`** (slot 44, `0x006CA8A0`, pop 12, no defined return):
- null lot or `cfg` → nothing.
- If `forceEmpty` and the zone is not 0, 11, 12 or 15: `SetEmptyLotBaseTexture(lot)` (slot 43) instead.
- Otherwise for each configuration object of type 2, in array order: position from slot 39; when its ID list is not
  empty, `tex := (first ID ≠ 0) ? PickLotBaseTextureForLot(ids, lot) : 0` (skipping the object when a pick gives 0);
  record `{tex, trunc(r24(pos.x · 0.0625)), trunc(r24(pos.z · 0.0625)), orientation byte, object byte 3}` if the cell is
  inside the city. The orientation byte is slot 39's `orient` output, `(object byte +0x02 + lot facing) mod 4`
  (stored at `0x006CA9F7` from slot 39's out-parameter; LD8). The cell bytes are the truncated values cut to 8 bits
  before the city-size test (`movzx` at `0x006CA9FB`). On disk each texture record is 14 bytes: tex, x, z, orientation, object byte 3, five bytes that
  default to `0xFF` (`0x005CA770`), and a frame byte; when the frame is `0xFF` and the texture has at least 2 frames,
  the base-texture occupant draws it with its file-local generator `0x00B23078` (`0x005E0B20`; lot-records.md).
- If any records: `GetLotBaseTextureOccupant(lot, &occ, true)` (slot 41), `occ.SetTextures(records, count, 0)`
  (its slot 8); if the occupant is already inserted (flag 1), notify the occupant manager (slot 24, (occ, 0)) and
  `PostOccupantMessage(0x99EF1145, 0)`; otherwise insert it (occupant manager slot 20).

**`PickLotBaseTextureForLot(ids, lot)`** (`0x006C60C0`):
```
base := ids[Uniform(n)]                                                  (file RNG)
wealth := lot.GetOccupantWealth(), or the building profile's wealth, else 1
decay := condition 0–1 → 0, 2–3 → 1, 4–6 → 2
hi := base & 0xFFFF0000
for cand in (hi | (((wealth << 4) | decay) << 8)), (hi | (wealth << 12)), (hi | (decay << 8)):
    if a resource (type 0x7AB50E44, group 0x0986135E, instance cand) exists (resource manager slot 14): return cand
return base
```

**`SetForceEmptyLotBaseTextures(f)`** (slot 47, `0x006C77D0`): if `f` equals the flag, nothing. Otherwise store it
and walk every lot with a fresh safe iterator: `f` → `SetEmptyLotBaseTexture(lot)`; else
`SetNormalLotBaseTexture(lot, lot.GetLotConfiguration(), true)` (draws). The iterator is unregistered and freed.

`SetEmptyLotBaseTexture` (slot 43) and `GetBoundaryLotTexture` (slot 42) compute empty-lot textures from the
boundary rules; no draw. Both are specified in §17.7.

**Slot 41 `GetLotBaseTextureOccupant(lot, cISC4LotBaseTextureOccupant*& out, bool create)`** (`0x006C41A0`, pop 12,
AL = `out ≠ null`) (LD16, LD24):
1. `out := null`; `(x, z) := lot.GetLocation()` (lot slot 21: the lot's location bytes `+0x7A`, `+0x7B`).
2. `occupantManager.GetFirstOccupantByStandardCityCell(&o, x, z, 0x497F6D9D)` (slot 33, landfill.md §4.8.6): the
   **first** base-texture occupant in that one cell's bucket order whose box overlaps the cell. **The lot pointer is
   not checked**; whichever base texture comes first at the lot's location cell is taken.
3. Found: QueryInterface `o` for `0x097F6D98` (`cISC4LotBaseTextureOccupant`) into `out`; on success
   `out.SetUsage(2)` (base-texture slot 5); release `o`. Return `out ≠ null`.
4. Not found and `create`: `GZCOM()->CreateInstance(0xC97F987C, 0x097F6D98, &out)`; if that fails, a direct `new`
   (0x60 bytes, `0x005DECF0`, constructor `0x005E05B0`) AddRef'd. Then `out.AsOccupant().Init()` (occupant slot
   3) and `out.SetUsage(2)`; the occupant is **not** inserted here (slots 43 / 44 insert it). Return `out ≠ null`.
5. Not found and not `create`: return false.
The caller owns the reference in `out` in both cases. Usage 2 is set on every path that returns an occupant
(`0x006C41FB`, `0x006C426F`).

**Slot 45 `UpdateLotBaseTextureOccupant(lot, bool force)`** (`0x006C4290`, pop 8; AL is whatever the last call
left, no defined value) (LD16): `occ := slot 41(lot, &occ, false)`; if found:
`occ.UpdateVertexAltitudes(true, force, null)` (base-texture slot 9, `0x005DF820`, pop 12), then release `occ`.
`UpdateVertexAltitudes(a, force, bool* changed)` recomputes the occupant's bounding box from the terrain
(`0x005DF370`); when `force` is set, or the box changed and `a` is set, it notifies the occupant manager if the
occupant is inserted (occupant manager slot 24 with the occupant sub-object at +4 and 0) and rebuilds its models
(`0x005DF640`). It stores the changed flag through `changed` when that is not null.

**Results (LD33).** `UpdateVertexAltitudes` has **no defined result**: on return EAX still holds its third argument
(the `changed` pointer, or null), which is not a flag; slot 45 passes null. Slot 45's own result is not that value
either: when the occupant was found, its last call is the `Release` of `occ`, so its EAX is that call's return (the
remaining reference count, 0 when the occupant was freed); when no occupant was found it is slot 41's false. A
re-implementation must not give either method a meaningful boolean.

### 6.7 Construction occupant (`cSC4ConstructionOccupant`, class `0xA97F909E`) *[compat]*

This is the object of §6.3. It is not part of the lot developer, but it decides **when** the lot developer's
completion work (§6.5) runs, so its timing is specified here. Everything it builds (scaffold, crane, props, crew
automata, spark effects, construction lot texture) is visual. The **completion day uses no random draw**: it depends
only on the building's bounding box, the terrain under it, the construction tuning and the building's construction
time property.

#### 6.7.1 Identity

| Item | Value |
|---|---|
| main vtable | `0x00AA2250` (slot 4 `SetLot`, `0x005D0890`) |
| occupant sub-object (+8) | vtable `0x00AA21B8`; slot 3 `Init` `0x005D1300`, slot 4 `Shutdown` `0x005D0CE0` |
| message target | vtable `0x00AA2240`, `DoMessage` `0x005D1090` |
| serializable | vtable `0x00AA219C`, `Write` `0x005CD250`, `Read` `0x005CE770` (layout not specified, §6.7.10) |
| size | 0x1A0 bytes (constructor `0x005D00F0`) |

**`Init`** (lot-records §9.2 has the full order): skip if already initialised; run the occupant base `Init`
(`0x005E8C90`, lot-records §2.4) unconditionally; the once-per-process `StaticInit` (`0x005CED80`, flag `0x00B22F60`);
subscribe (message server `0x00B43CCC`) to `0x66956814` (SimNewDay, the simulator's queued broadcast), `0xA6B79602`,
`0x06CDB65B` and `0x26D31EC1` (four; the tick is not subscribed); register with the animation-tick manager at
priority **`0x80`** (every second frame, §6.7.8); last, `AddOccupantGroup(0x5007)` on itself (occupant slot 23,
`0x005E9350`): occupant group `0x5007` is added to property `0xAA1DD396` if absent, posting `0xEBFF18A3`.

`DoMessage` handling: SimNewDay → `DoDailySim` (§6.7.5; **data1 is not used**); `0x86AD10EE` → animation tick with
`ms := data2 / 1000` (data2 = the message dword at +0x14, the clamped µs of §6.7.8; unsigned, truncating); the other three are visual only (`0x26D31EC1` rebuilds the model, crane and renderer entry when a
pending flag is set; `0x06CDB65B` adds / removes the occupant from the renderer `0x00B43DD0` (slots 28 / 29) and
toggles occupant flag 2; `0xA6B79602` sets a byte flag at +0xFC) *[inferred: their names]*. All return true.

**`Shutdown`**: posts `0xC9E7B1E8` (data1 = 0), unsubscribes and removes everything it placed (§6.7.14).

#### 6.7.2 Static tuning (`StaticInit`)

`StaticInit` **reseeds the file RNG `0x00B22FB0` with plain `cRZTime()` (no XOR)** at `0x005CEDAC`. The static
initialiser `0x00A733A0` seeded it with `cRZTime() XOR 0xEC09C32A`, but that seed is overwritten before any draw
(correction to foundations §6.2 / §6.3, §12.3).

It then reads the exemplar type `0x6534284A`, group `0x2989FD57`, instance `0x8989DBE3` ("Construction",
`SimCity_1.dat`). Each property is a float read with `GetPropertyValue` (`0x005FD4F0`); a missing property keeps the
code default. Names are the Mac build's symbols.

| Property | Global | Name | Code default | Shipped | Stored as |
|---|---|---|---|---|---|
| `0x8989EC22` | `0x00B0A7C8` | sfSegmentHeight (`segH`) | 3.0 | 3 | value |
| `0x8989EC23` | `0x00B0A7CC` | sfPostWidth | 1.0 | 1 | value |
| `0x8989EC24` | `0x00B0A7D0` | sfScaffoldGap (`gap`) | 1.0 | 2 | value |
| `0x8989EC25` | `0x00B0A7D4` | sfPlankWidth | 2.0 | 2 | value |
| `0x8989EC26` | `0x00B22F88` | sfPlankTextureScale | — | 15 | `r24(1 / v)` |
| `0x8989EC27` | `0x00B22F8C` | sfScaffoldTextureScale | — | 25 | `r24(1 / v)` |
| `0x8989EC28` | `0x00B22F90` | sfGirderTextureScale | — | 25 | `r24(1 / v)` |
| `0x8989EC29` | `0x00B0A7D8` | sfScaffoldGrowthRate (`grow`) | 1.0 | 1 | `r24(1 / v)` when present → 1.0 |
| `0x8989EC2A` | `0x00B0A7DC` | sfScaffoldRemovalRate (`remove`) | 2.0 | 0.5 | `r24(1 / v)` when present → 2.0 |
| `0x8989EC2B` | `0x00B0A7E0` | sfCranePostWidth | 1.5 | 1.5 | value |
| `0x8989EC2C` | `0x00B0A7E4` | sfCranePostToBuildingGap | 0.5 | 0.5 | value |
| `0x8989EC2D` | `0x00B0A7E8` | sfCraneHeightAboveScaffold | 1.0 | 3 | value |
| `0x8989EC2E` | `0x00B0A7EC` | sfCraneTimeOfFullRotation | 6000 | 6000 | value (ms) |

#### 6.7.3 Geometry (`RebuildModel`, `0x005CAD60`)

All float32 (§18 rule; `0.1` is a double constant entering unrounded):
- `bbox :=` the building's bounding box (lot slot 48 `GetBuilding` → `AsOccupant` → occupant slot 10).
- `ground := min(bbox.min.y, altitude at the four bbox corners)` (terrain slot 13); each comparison keeps the current
  value when the altitude is NaN.
- `startY := r24(bbox.min.y + 0.1)`, `topY := bbox.max.y`.
- `f := r24(r24(topY − startY) / segH)`; `N := floor(f) + 1`, and `N = 0` becomes 1. `floor` is the round idiom applied
  to `r24(f − 0.5)` with a fix-up (+1 when `r + 1 = f`).
- Scaffold box: min `(bbox.min.x − gap, ground, bbox.min.z − gap)`, max `(bbox.max.x + gap, topY, bbox.max.z + gap)`.

#### 6.7.4 `SetLot(lot)` (`0x005D0890`)

In this order: `Init` if needed; hold the lot and its building (lot slot 48; return if null); `RebuildModel`;
`DivideLot` (8 areas of 0x18 bytes); `AttemptAddCrane`; `AddGroundProps`; `CreateConstructionCrew(areas, box)`;
`AddRoofProps(box)`; `CreateDustClouds`; `CreateSparks`; `CreateLotTexture` (the draws of these steps are in §6.7.7);
`T0 := GetSimDateNumber()` (simulator `0x00B43D5C` slot 8); the timing values below; `state := 0`, `counter := 0`,
`msAcc := 0`, `nextDay := T0 + A`; `SetBuildingCompletion(0.0)`; scaffold height `r24(ground + 0.001)`; crane state
(when a crane exists): state 2, accumulator 1, duration 0; post `0x49E7B1E2` (data1 = 1) then `0xC9E7B1E8`
(data1 = 1); add to the renderer.

Timing values (`round` = the round-half-even idiom of foundations; integers):

```
D := building property 0x499AFA38 (uint8, profile byte +10); default 30 when absent; D = 0 → 10
A := min(round(r24(r24(startY − ground) · grow)), 1)
B := min(round(r24(segH · grow)), 1)
C := min(round(r24(r24(topY − ground) · remove)), 1)
E := max_signed(D − B·N − A − C, N) / N            (unsigned integer division)
```

`D` comes from the building's profile (building occupant slot 11 → BDS slot 39 `GetBuildingProfile`; a static zero
profile, hence `D = 10`, when the BDS is null). The `min(…, 1)` is a *[quirk]* (`std::min` where `max` was probably
meant): A, B and C are 0 or 1, so the scaffold phases never last more than a day. `E` is at least 1.

#### 6.7.5 Daily phases (`DoDailySim`, `0x005D0680`)

On each SimNewDay: nothing if there is no building or `state = 4`. `today := GetSimDateNumber()` (the current date,
not the message's data1). Act only if `today ≥ nextDay`; then `msAcc := 0` and:

| State | Action | Next |
|---|---|---|
| 0 or 2 | `counter += 1`. If `counter > N`: remove the roof props (prop manager slot 29) and switch the scaffold object to removal; else post `0x49E7B1E2` (data1 = 1). Then each spark effect slot 8(1) | `counter > N`: state 3, `nextDay := today + C`; else state 1, `nextDay := today + B` |
| 1 | scatter the sparks (§6.7.7, draws) | state 2, `nextDay := today + E` |
| 3 | completion, below | state 4 |

**Completion**, in this order: state := 4; `SetBuildingCompletion(1.0)`; if the occupant is in the renderer (bit
0x02 of byte +0x37), remove it (renderer slot 29) and clear the bit; occupant manager slot 21 `RemoveOccupant(this + 8,
0, 0)`; **`lot.SetState(3, 0, 0)`** (lot slot 34, call at `0x005D0862`); tail jump to its own `Shutdown` (`0x005D0872`).
`SetState` posts the queued `0xE8BDA2E7` (2 → 3), so the lot developer's completion work (§6.5) runs when that
message is delivered, after this day's SimNewDay handlers.

**Closed form.** Since the occupant acts at most once per day and every step lasts at least one day, with `d1` the
first SimNewDay date it receives and `d1' := max(d1, T0 + A)`:

```
segment k (1..N) starts on d1' + (k − 1)·(1 + E)
scaffold removal on       d1' + N·(1 + E)
completion (SetState 3) on d1' + N·(1 + E) + 1
```

`d1 = T0 + 1` when `SetLot` runs during a day's processing after the queued SimNewDay(`T0`) was taken from the queue;
`d1 = T0` only if the occupant subscribed before SimNewDay(`T0`) was delivered *[inferred: depends on whether the
message server delivers to targets added after posting]*.

`SetBuildingCompletion(p)` (`0x005CA8D0`): if the building's completion (building slot 9) differs from `p`, set it
(slot 10) and post `0x2977AE2F` queued with IGZUnknown = the building (received by the model maker and the automata
manager; visual).

#### 6.7.6 Animation tick (visual)

`DoAnimationTick(ms)` (`0x005CE110`) accumulates `msAcc`, interpolates the scaffold height (states 0, 1, 3) and the
building completion (state 2) between phase boundaries, and runs `AnimateCrane` in states 0–2 when a crane exists. It
changes no simulation state. Exact endpoints, the 500 ms/day multiplier and division by `E` are
specified in §6.7.15 (LD36); that instruction-verified contract supersedes the former decompiler inference.

`AnimateCrane` (`0x005CD570`): `acc += ms`; when `acc ≥ duration`, `acc := 0` and the crane advances: state 0 → 1
with `duration := 1000 + Uniform(1000)`; 1 → 2 without draw; 2 → 0 with `mid := r24(r24(r24(max − min) · 0.5) + min)`,
new target `RDRU(mid, max)` if the current target is below `mid` (`mid ≤ target` → `RDRU(min, mid)`), the previous
target := the old target, and `duration := round_f32(T · |new − previous| · k)` (`0x005CD645…0x005CD669`: x87
product `T · |Δ|`, times `k`, plus 12582912.0, stored as f32 and the bits minus `0x4B400000`, so round half to
even), where `T` is the **tunable** sfCraneTimeOfFullRotation global `0x00B0A7EC` (§6.7.2, default 6000; not a
literal) and `k` = f32 `0x3E22F983` = 0.15915494 = 1/(2π) at `0x00AA2098`. State 1 → 2 keeps `acc := 0` and leaves
the duration as drawn by 0 → 1.

Initial values (`SetLot`, `0x005D0B46…0x005D0B87`, only when a crane was created): target := min, previous := max,
`acc := 1`, `duration := 0`, state 2, boom posed at min; the first tick with `ms ≥ 0` therefore runs 2 → 0 at once
(`acc ≥ 0`), drawing the first target from `RDRU(mid, max)` (since min < mid).

Swing limits (`AttemptAddOuterCrane` `0x005CD710`, after it picks the lot area side `s` 0…3 for the crane post):
with `R` = the boom radius `0x00B22F70[type]` (type = record #14j) and the crane post at `(x, z)`, let `d` =
`min(B.xmax − x, x − B.xmin)` for sides 0 / 2 and `min(B.zmax − z, z − B.zmin)` for sides 1 / 3, where `B` spans from
DivideLot area 4's min corner (`+0x60`, `+0x68`) to area 6's max corner (`+0x9C`, `+0xA4`). The half-width
`h := π/2` when `d ≥ R`, else `asin(d / R)` (x87, stored f32), and the limits are `[c − h, c + h]` with centre `c` =
3π/2 (side 0), π/2 (side 2), 2π (side 3), π (side 1); the `d ≥ R` cases use the f32 constants π, 2π, 0, π, 3π/2,
5π/2, π/2, 3π/2 directly (`0x005CDAB1…0x005CDC17`). These draws use the **same** file RNG
as §6.7.7, at real-time moments.

#### 6.7.7 Random draws

Generator: the file-local `cRZRandom` at `0x00B22FB0` (foundations §6.1 methods; `RDRU` =
`RandomDoubleRangeUniform` `0x00913761`, `GF` = `RandomDoubleRangeGaussianFast` `0x0091386B`, 3 `RandomDoubleUniform`
draws each). The occupant's linear iterators (`Reset` `0x005CF610`, `GetNext` `0x005CBD90`) run in **table mode**:
each reset refills the table with 0..n − 1 and shuffles it with `random_shuffle` (`0x006DB010`), consuming **n − 1 C
`rand()`** calls. Ghidra names `0x005CFE20` and `0x00913761` wrongly (see §12.3).

`SetLot` order (steps without a listed draw draw nothing; `RebuildModel`, `DivideLot`, `CreateCrane`,
`CreateDustClouds`, `CreateSparks`, `Read`, `Write` draw nothing):

1. **`AttemptAddCrane`** (`0x005CE360`), building property `0xAA83558F` (uint8): 0 → no crane; 1 → outer; 2 →
   inner; 3 → outer, then inner if the outer one does not fit; > 3 → no crane.
   - Outer (`0x005CD710`): tries crane sizes 0..2 (tables `0x00B22F64`, `0x00B22F70`, `0x00B22F7C`). If one fits on
     `k` usable sides: one `Uniform(k)`; the result `r` picks the (r + 1)-th usable side in side order 0..3. If none
     fits: no draw.
   - Inner (`0x005CDDA0`): always one `NextRaw`. If the x extent ≥ the z extent: side 0 if `raw < 0x7FFFFFFF`, else
     2; otherwise side 1, else 3.
2. **`AddGroundProps`** (`0x005CF790`), only when the prop table `0x00B22FA0` (12-byte entries: type, dimA, dimB) is
   not empty: `c := SintRange(1, 4)` (1 `NextRaw`, 1..3). Each round: reset the area iterator (8 areas, 7 `rand()`);
   for each area with width > 0 and depth > 0: shuffle the ground-prop list (`nGround − 1` `rand()`), take the first
   prop with `dimA < width` and `dimB < depth` and `CreateAndPlaceProp` it, `c −= 1`, end the round. Rounds repeat
   while `c > 0` and the previous round attempted a placement.
3. **`CreateAndPlaceProp`** (`0x005CCF20`): if the prop manager (slot 15) rejects the type: no draw. Otherwise
   orientation `[0, 2, 1, 3][Uniform(4)]` (table `0x00AA2088`) when `width > dimB` and `depth > dimA`, else
   `[0, 2][Uniform(2)]`; footprint `(dimA, dimB)` for orientations 0 / 2, swapped for 1 / 3; offset x :=
   `RDRU(0, width − fx)`, then z := `RDRU(0, depth − fz)`. 3 draws.
4. **`CreateConstructionCrew`** (`0x005CFA60`), only when render-properties integer 7,
   `kEffectPriorityLevel`, at `0x00B43CC4` → +0x18 → +0x134 is ≥ 3 (signed):
   - ground crew: `k := SintRange(2, 4)` (1 `NextRaw`, 2..3); per member: reset the area iterator (7 `rand()`); the
     first area with width > 2 and depth > 2 gets `Uniform(3)` → type `[0x0210B000, 0x0210C000, 0x0210A000]`
     (`0x00AA1E68`) and `CreateAutomata(area, 0, type)`; stop when no area qualifies;
   - roof crew: `m := Uniform(2)`; for `j = 0..m`: stop if the box width or depth ≤ 2; else `Uniform(2)` → type
     `[0x02109000, 0x0210D000]` (`0x00AA1E74`), `CreateAutomata(box, 1, type)`.
   - `CreateAutomata` (`0x005CE580`): x := `RDRU(r24(min.x + 0.5), r24(r24(max.x − 0.5) − 1.0))`, then z likewise;
     the automata manager (city slot +0xB0, its slot 15) creates it; on success `nS3D::RandomDirection2`
     (`0x0080B680`) makes exactly **2** `RDRU(−1, 1)` draws (its rejection loop never repeats *[quirk]*).
5. **`AddRoofProps(box)`** (`0x005CFC40`): `c := SintRange(0, 1)` (always 0, but 1 `NextRaw`); `c += 1` if
   `r24(width · depth) > 256`, `+= 2` if `> 1024`; rounds as in step 2 over the roof-prop list (`nRoof − 1`
   `rand()` per shuffle), placing on the box.
6. **`CreateLotTexture`** (`0x005CF380`): for x from x1 to x2 (outer) and z from z1 to z2 (inner) over the lot's
   cell rectangle (lot slot 14), for cells inside the city and not water (terrain slot 34): `Uniform(4)` → texture
   instance `0xA990C100 + 0x100·r`.

Daily, state 1 → 2, **scatter** (`0x005CFE20`): `CreateSparks` made 1, 2 or 3 spark effects (`ext := max(xmax − xmin, zmax − zmin)` of the
scaffold box, record #7 at object `+0xC0`, **y excluded**: `ext < 20` → 1, `20 ≤ ext < 30` → 2, else 3). A side iterator over 4 sides is reset first (3 `rand()`); per spark, with box min `(x0, y0, z0)`
and max `(x1, y1, z1)`:

| Side | x | z | then y |
|---|---|---|---|
| 0 | `GF(x0, x1)` | z0 | `GF(y0, y1)` |
| 1 | x1 | `GF(z0, z1)` | `GF(y0, y1)` |
| 2 | `GF(x0, x1)` | z1 | `GF(y0, y1)` |
| 3 | x0 | `GF(z0, z1)` | `GF(y0, y1)` |

then the effect's slot 9(&pos) and slot 7(1).

**Determinism.** The generator is reseeded from the wall clock and also drawn by the crane at frame times, and the
C `rand()` is shared process-wide, so the original's construction scene is not reproducible. FreeCity4 seeds the
generator explicitly *[choice]*; conformance covers the draw **sequence and arguments** (§16 tests 23–25), not
original scenes. Simulation results do not depend on these draws, provided the construction props, automata and lot
texture are removed or replaced at completion: the roof props are removed at state 3, the base texture is re-picked
by §6.5 *[inferred]*, and `Shutdown` removes the props, effects and crew (§6.7.14).

#### 6.7.8 Animation-tick manager (`cSC4AnimationTickManager`, service `0x86AD10EC`)

`AddTickableObject(target, priority)` (`0x00448B70`) puts the target in a bucket: priority ≥ `0xC0` → 0; `0x80`–`0xBF`
→ 1; `0x40`–`0x7F` → 2; < `0x40` → 3. `DoAnimationTick` (`0x004487F0`) pre-increments a frame counter, `phase :=
counter & 7`, and fires the buckets whose bit is set in the mask `[3, 5, 3, 9, 3, 5, 3, 1][phase]`: bucket 0 every
frame, bucket 1 on even phases, bucket 2 on phases 1 and 5, bucket 3 on phase 3. For each firing bucket: `delta :=
now − lastTick[bucket]` in **µs**, clamped to `[min, max]` (defaults 2000 and 500000; `SetMinAndMaxTickTimeDeltas`
`0x00447F40`), `lastTick[bucket] := now`; step mode uses `last + 100000`. Each target in the bucket receives
`0x86AD10EE` by a direct `DoMessage` call (not through the server) with void1 = the manager, data2 = the clamped µs,
data3 = the float32 bits of `delta · 0.001` (ms).

So the lot developer (`0xC0`) ticks every frame and the construction occupant (`0x80`) every second frame.

#### 6.7.9 Messages posted

| Type | When | Fields | Delivery | Receivers |
|---|---|---|---|---|
| `0x49E7B1E2` | `SetLot`, and each segment start | data1 = 1, IGZUnknown = the occupant | queued | `cSC4AudioEventHandler::HandleConstructionMessages` (`0x004EEC10`) |
| `0xC9E7B1E8` | `SetLot` (data1 = 1), `Shutdown` (data1 = 0) | as above | queued | same |
| `0x2977AE2F` | building completion changed | IGZUnknown = the building | queued | model maker, automata manager |

#### 6.7.10 State and the save record

Every field and the record's row 14 are lot-records.md §9.1 (object-frame offsets). The world bounds there (record
field 6, `+0xA8…+0xBC`) are written by `RebuildModel` (`0x005CB76D`–`0x005CB799`): the scaffold box `+0xC0…+0xD4`
widened by sfPostWidth (`0x00B0A7CC`) in x and z, with the same heights (re-construction 2026-10-23, OP17).

#### 6.7.11 `DivideLot(areas)` (`0x005CB9F0`)

*Added 2026-10-23 (re-construction).* With the lot's cell rectangle (lot slot 14) turned into world corners `(X0, Z0)`
and `(X1, Z1)` (city slot 96 at `(x1, z1)` and `(x2 + 1, z2 + 1)`), the building's bounding box (`bmin`, `bmax`) and
`gap` (sfScaffoldGap): `xa := max(bmin.x − gap, X0)`, `xb := min(bmax.x + gap, X1)`, `za := max(bmin.z − gap, Z0)`,
`zb := min(bmax.z + gap, Z1)` (a NaN compare keeps the building side). The eight areas, all with y from 0 to 1:

| Area | x | z | Where |
|---|---|---|---|
| 0 | `xa`…`xb` | `Z0`…`za` | the −z strip |
| 1 | `xb`…`X1` | `za`…`zb` | the +x strip |
| 2 | `xa`…`xb` | `zb`…`Z1` | the +z strip |
| 3 | `X0`…`xa` | `za`…`zb` | the −x strip |
| 4 | `X0`…`xa` | `Z0`…`za` | the −x −z corner |
| 5 | `xb`…`X1` | `Z0`…`za` | the +x −z corner |
| 6 | `xb`…`X1` | `zb`…`Z1` | the +x +z corner |
| 7 | `X0`…`xa` | `zb`…`Z1` | the −x +z corner |

An area can be empty or inverted; the prop and crew placements skip those (width or depth not above their limits).

#### 6.7.12 Cranes

*Added 2026-10-23 (re-construction).* **Size tables** (`StaticInit`): for `k = 0, 1, 2` the crane model `{type
0x5AD0E817, group 0xBADB57F1, instance 0x1C210000, 0x1C220000, 0x282A0000}` is fetched (resource manager slot 3,
interface `0x9AD0E7E3`) and its instance's bounds read (slot 7): `width[k] := max.x − min.x` (`0x00B22F64`), `reach[k]
:= max(|min.x|, |max.x|)` (`0x00B22F70`), `back[k] := min(|min.x|, |max.x|)` (`0x00B22F7C`). A model that does not
load leaves its entries 0, and a size with `width = 0` is never used.

**Outer crane** (`0x005CD710`). With `W := xb − xa` and `D := zb − za` from the areas, for `k = 0, 1, 2` until one
fits:

1. Skip `k` unless (`0.8 · reach[k] ≤ W` or `D ≥ 12`) and the local bounds' height (`+0xD0 − +0xC4`) is at least 12.
2. Usable sides: all four, then side 3 is dropped if `X1 − xa < reach[k]`, side 1 if `xb − X0 < reach[k]`, side 2 if
   `zb − Z0 < reach[k]`, side 0 if `Z1 − za < reach[k]`.
3. `need := sfCranePostToBuildingGap + back[k] + sfCranePostWidth`; a usable side is dropped when its strip (area 0 to
   3) is not deeper than `need` (z extent for sides 0 and 2, x extent for 1 and 3).
4. Any side left: crane `k` fits.

The side is drawn as in §6.7.7 step 1. With `f := sfCranePostWidth · 0.5 + sfCranePostToBuildingGap`, the crane stands
at, for side 0: (the strip's middle x, `za − f`); side 1: (`xb + f`, middle z); side 2: (middle x, `zb + f`); side 3:
(`xa − f`, middle z), facing the building. Its swing range (`+0x178`, `+0x17C`) is §6.7.6's. Then `+0x124 := k`, the
crane is made (`CreateCrane`, `0x005CC820`), and its footprint, the square of half-size `need` around it, is cut out
of the side's area by `RemoveRectangle`.

**Inner crane** (`0x005CDDA0`). The long extent `L` of the building's box is x when the x extent is at least the z
extent (sides 0 / 2) and z otherwise (sides 1 / 3); the side is drawn as in §6.7.7. The size is the first `k` with
`width[k] > 0` and `back[k] + reach[k] < L`; none → no crane. The crane stands on the building, offset along the long
axis from the middle by `(back + reach) / 2 − back`, with a swing range chosen as for the outer crane from the half of
the short extent *[summary; exact side mapping and signed positions are in §6.7.15]*.

**`RemoveRectangle(area, r)`** (`0x005CCD70`): of the four parts of `area` outside `r` (below it in z, right of it in
x, above it in z, left of it in x, each spanning the area's full other extent) the area becomes the one with the
largest x·z product, ties keeping the earlier one; when none is larger than 0, the area becomes empty.

**`SetCraneAngle(a)`** (`0x005CBE10`): the crane model turns to `a + π`, reduced to at most 2π (equality is retained;
§6.7.15). During state 0
of §6.7.6 the angle is `start + (target − start) · s`, `s := t² (3 − 2t)` with `t := acc / duration`.

#### 6.7.13 Prop tables (`StaticInit`)

*Added 2026-10-23 (re-construction).* The prop table (`0x00B22FA0`) holds, for each of the ground prop types
`0x02000000`, `0x02010000`, `0x02050000`, `0x03040000`, `0x03050000` (`0x00AA1E54`) and then the roof prop types
`0x03030000`, `0x02050000`, `0x02060000` (`0x00AA1E48`) that the prop manager knows (slot 14 `GetPropProfilePtr`), the
entry (type, size x, size z) from the prop profile; `nGround` (`0x00B22F58`) counts the ground entries and
`0x00B22F5C` all of them, so the roof list is the entries after `nGround`.

#### 6.7.14 Shutdown cleanup

*Added 2026-10-23 (re-construction).* `Shutdown` (`0x005D0CE0`), only for an initialised occupant: post `0xC9E7B1E8`
(data1 = 0); the occupant base `Shutdown`; release the lot and the building; unsubscribe from the four messages and
leave the animation-tick manager (its slot 6, priority `0x80`); release the texture bindings it holds (`+0x118`) and
free the model buffers (`+0xDC`…`+0xF4`, `+0x114`); release the crane model; remove every ground prop, roof prop and
effect, and the current effect, from the prop manager (slot 29 `RemoveProp`) and release them (the current effect is
released without a null test *[quirk]*); remove every crew automaton from the automata manager (city slot 44, its slot
8 with 0) and release it. So the completion's tail call (§6.7.5) removes everything the occupant placed.

#### 6.7.15 Construction visual contract (LD35–LD38) *[compat]*

This section supplies the former open visual details and corrects the earlier interpolation inference.
None of these operations changes the completion day.

**Interpolation (LD36).** `DoAnimationTick` takes unsigned integer milliseconds, not a float.
It adds them to the saved 32-bit `msAcc` with wrapping, including in state 4, then computes elapsed
construction days `d = r24(unsigned(msAcc) · f32(0x3B03126F))`. The multiplier at `0x00B0A804`
is `0.0020000000949949026`, nominally **1/500 ms**; it is independent of simulation speed and date.
Let `u(X) = min(1, r24(d / unsigned(X)))` and `k = unsigned(counter−1)`:

| State | Visual value supplied |
|---|---|
| 0 | scaffold `ground + (startY−ground) · f`, where `f=1` if `d≥A`, otherwise `d/A` |
| 1 | scaffold `startY + k·segH + segH·u(B)` |
| 2 | building completion `k·(1/N) + u(E)·(1/N)` |
| 3 | scaffold `ground + ((startY−ground)+span)·(1−u(C))` |
| other | no height or completion update |

The final supplied value is float32. Within these expressions, products and sums remain in x87
precision until the result store; the state-1 base and state-2 reciprocal are not separately
rounded. Only `d` and the displayed `d/X` ratios for states 1–3 have intermediate float32 stores;
state 0 keeps its ratio in x87 precision. There is no lower clamp. For states 1–3 the native
upper-minimum selects an unordered ratio, so `0/0` propagates NaN; positive elapsed time over a
zero denominator clamps to 1. State 0 instead chooses its endpoint for `A=0,d=0`.

The saved `span` at `+0x158` is **r24(N·segH)** after RebuildModel, replacing the initial
`topY−startY` (`0x005CAF76…0x005CAF9C`). State 1 therefore ends at `startY+counter·segH`;
state 2 runs from `(counter−1)/N` to `counter/N`; state 3 starts at `startY+span` and ends at
ground. That removal start can exceed the building top. The initial SetLot height `ground+0.001`
becomes ground on a zero tick with `A>0`, or startY with `A=0`. **State 2 does divide by E**:
the contrary inference in the previous §6.7.6 text was a decompiler stack-recovery error. Only
states 0–2 animate the crane, after the visual update. Evidence: unsigned accumulator and multiplier
`0x005CE116…0x005CE14A`; states 0/1 `0x005CE15B…0x005CE228`; state 2 including the ratio
store `0x005CE232…0x005CE2B9`; removal and crane exclusion `0x005CE2D8…0x005CE344`.

**Scaffold mesh (LD35).** Let its horizontal bounds be `[x0,x1]×[z0,z1]`, and number the corners
`0=(x0,z0)`, `1=(x1,z0)`, `2=(x0,z1)`, `3=(x1,z1)`. The four vertical quads have the following
horizontal corner order; their vertical order is bottom, top, top, bottom. Initially bottom is
ground and top is topY. Vertices are world positions, with no additional lot-orientation rotation.

| Quad | Corner indices |
|---|---|
| 0 | 0, 0, 1, 1 |
| 1 | 2, 2, 3, 3 |
| 2 | 3, 3, 1, 1 |
| 3 | 0, 0, 2, 2 |

With `p=plankWidth`, the four horizontal plank rectangles are, in order,
`[x0,x1]×[z0,z0+p]`, `[x1−p,x1]×[z0+p,z1−p]`,
`[x0,x1]×[z1−p,z1]`, `[x0,x0+p]×[z0+p,z1−p]`. Each rectangle's vertex order is
`(xmin,zmax), (xmin,zmin), (xmax,zmin), (xmax,zmax)`, all at topY, with stored texture
coordinates `r24(x·plankTextureScale), r24(z·plankTextureScale)`. The inner girder quad uses
that same vertex order on `[x0+p,x1−p]×[z0+p,z1−p]` at topY. Rectangles are not clipped
for small buildings. The inner-girder plank offsets are cached at the first rebuild; later tunable
changes do not refresh those offsets.

Each of the four posts is a square centred on a scaffold corner, width `postWidth`, base ground,
initial top `r24(topY+postWidth)`. Each has four sides in the vertical-quad order above and a top
in the horizontal order above; no bottom. Scaffold/plank/girder vertices have position and two
texture coordinates (20-byte stride); scaffold side buffers are initially zeroed. Posts have position
and BGRA colour (16-byte stride), with alpha 255; rebuild marks lighting dirty. The world bounds
extend the scaffold by postWidth on each horizontal edge and retain `[ground,topY]` vertically.
Evidence: side vertices `0x005CAFA2…0x005CB084`; planks `0x005CB090…0x005CB373`;
girder `0x005CB379…0x005CB490`; posts `0x005CB493…0x005CB6EF`; bounds
`0x005CB6F5…0x005CB7A8`. Corner/winding table: `0x00AA1FA8`.

**Height updates (LD35).** `SetScaffoldHeight(h)` does not clamp h or rebuild the saved bounds.
It moves the upper two vertices of each scaffold side to h, all plank and inner-girder vertices to h,
and the twelve upper/top vertices of each post to `r24(h+postWidth)`; all post bases stay at ground.
If a crane instance exists, its saved y and model translation y become
`r24(h+sfCraneHeightAboveScaffold)`, preserving x, z and orientation. The crane post's upper
vertices use that same elevated height; its bases remain at ground.
Every roof-prop occupant gets position.y=h, is notified through occupant-manager slot 24 with
argument 0, and has property `0x99EF1146` removed (occupant slot 15, argument 0).
Every crew member whose **second** saved boolean is true gets position.y=h through automata
slots 7/8; its horizontal coordinates remain unchanged. Evidence: mesh updates
`0x005CC020…0x005CC1EB`, crane `0x005CC1ED…0x005CC30D`, props/crew
`0x005CC310…0x005CC3D8`.

**Inner crane (LD35).** Use the building occupant's world bounding box, without the scaffold gap.
For x extent ≥ z extent, the side is 0 or 2; otherwise 1 or 3, with the draw rule in §6.7.7.
Choose the first positive-width model whose `back+reach` is strictly smaller than the long extent.
Let `offset=(back+reach)·0.5−back`, and let `(mx,mz)` be the box centre. The post positions are:

| Side | x | z | Swing centre c |
|---|---|---|---|
| 0 | mx−offset | mz | 2π |
| 2 | mx+offset | mz | π |
| 1 | mx | mz+offset | π/2 |
| 3 | mx | mz−offset | 3π/2 |

Let `q` be half the short extent and `R=reach`. When q≥R, use these float32 endpoint constants
directly: side 0 `[3π/2,5π/2]`, 2 `[π/2,3π/2]`, 1 `[0,π]`, 3 `[π,2π]`.
Otherwise take `a=asin(q/R)` in x87 precision and store `c−a,c+a` to float32; the quotient uses
the float32-stored q. The centre constants are float32 π multiples, not double π. Centre/offset
arithmetic is retained in x87 precision until each position store. This differs from the outer
crane's side-to-angle mapping. No footprint is cut out for an inner crane.
Evidence: long-axis selection `0x005CDDC1…0x005CDE28`; fit `0x005CDE30…0x005CDE70`;
x-long placement/swing `0x005CDE7D…0x005CDF68`; z-long `0x005CDF6D…0x005CE051`.

**Crane model and post (LD35/LD38).** CreateCrane loads the type-indexed model resource listed
in §6.7.12, requests instance 0, and associates a successful instance with this construction occupant.
Only successful instance creation writes the saved crane position `(inputX,0,inputZ)`.
Its initial translation is that position, and its existing model orientation is rotated about Y by
`maxAngle − minAngle·0.5 + minAngle` — **not** `(maxAngle−minAngle)·0.5+minAngle`
(`0x005CC904…0x005CC936`). SetLot subsequently poses it at minAngle. The model resource is
released after the attempt. The square post mesh is rebuilt even if the resource/instance fails.
Its width is sfCranePostWidth, cached as a half-width on the first CreateCrane call. Its four side
quads follow the scaffold corner/winding table on the square centred on inputX,inputZ, initially
from ground to `r24(ground+double(0.1))`. There is no top/bottom quad. All sixteen vertices
initially have BGRA `(128,128,128,255)`. A height update replaces the upper y as described above.
Evidence: resource/instance `0x005CC820…0x005CC898`, initial transform and owner
`0x005CC8EA…0x005CCA47`, cached post geometry `0x005CCA4A…0x005CCD68`.

`SetCraneAngle(a)` sets the absolute Y rotation to a+π, subtracting float32 2π repeatedly while
the result is strictly greater than 2π (equality is retained; negative angles are not wrapped up).
It preserves translation. Its interpolated swing input remains §6.7.12's smoothstep.
Evidence: `0x005CBE10…0x005CBE4A` and transform update through `0x005CC019`.

**Lighting and Draw (LD35).** DoRotationChange clears the lighting-dirty byte at +0x100.
If either post mesh exists, it asks the lighting service (`0x00B43DDC`, slot 20) for RGB on
normals `(0,0,−1)`, `(0,0,1)`, `(1,0,0)`, `(−1,0,0)`, `(0,1,0)`; each channel is shifted
right one, then written as BGR with the vertex alpha retained. Every scaffold post uses these five
face colours in order; the crane's four faces use colour indices **0,2,1,3** (table `0x00AA1FE8`).
This is relighting, not a rotation of vertex positions (`0x005CB7C0…0x005CB9E5`).

Draw returns true. A true second argument skips all drawing. Otherwise it relights when dirty,
sets the view model transform, the packed state below, and this occupant's model shade. The
four scaffold sides are drawn in these orders for view rotation 0,1,2,3 respectively:
`[0,3,1,2]`, `[2,0,3,1]`, `[1,2,0,3]`, `[3,1,2,0]` (table `0x00AA2048`).
Texture **rows are draw-order positions**, not the selected side numbers. Each row uses the draw
context's zoom column. Per draw position, blending is `[false,false,true,false]` (bytes at
`0x00AA2040`), depth offsets `[1,1,−1,−1]` (`0x00AA2030`), and wrapping is `(3,3)`.
Side and inner-girder textures use a 4×4 transform with diagonal `(s,−s,s,1)` and translation
`(−s·vx,s·vy,−s·vz)`, where v is that mesh's first vertex projected to view coordinates.
The scales are scaffoldTextureScale and girderTextureScale respectively, and each transform entry
is stored to float32. The girder is drawn after the sides using texture row 5, blending true,
depth offset −1. Texture transform is then cleared; the sixteen plank vertices use row 4,
blending false, depth offset −1. All these draws use primitive 6, vertex format 2.

The eighty scaffold-post vertices are drawn next with texturing disabled, primitive 6, format 1,
then texturing is restored. The sixteen crane-post vertices are drawn the same way only in states
0–2. Draw resets shade, default render state and depth offset 0, then draws the crane model with
second argument false only in states 0–2. Thus state 3 retains scaffold/planks/posts but omits
both the crane model and its post. Null individual mesh pointers skip their draw.
Evidence: `0x005CC410…0x005CC634`, girder/planks `0x005CC63A…0x005CC75F`, posts and
restoration/crane `0x005CC764…0x005CC819`.

**Texture IDs and initial render state (LD38).** The six-by-five entries misleadingly named
`cISC4SimGrid<float>*` in older notes are **texture bindings**, not simulation grids. They are
created through texture-binding factory class/interface `0x1AC0E11A`/`0xFAC0E219`, slot 6,
with resource type `0x7AB50E44`, group `0x1ABE787D`. For columns j=0…4 the instance is base+j:

| Row | Instance base | Used for |
|---|---|---|
| 0 | `0x696F3000` | first side draw |
| 1 | `0x696F2000` | second side draw |
| 2 | `0x696F1000` | third side draw |
| 3 | `0x696F2000` | fourth side draw |
| 4 | `0x696F4000` | planks |
| 5 | `0x696F5000` | inner girder |

Init's packed state at +0xF8 enables colour write, depth write, depth test, blending and stage-0
texturing; disables alpha test, culling and stage-1 texturing; sets depth function 3, source/destination
blend 4/5 and stage-0 mode 1. Alpha function, alpha reference and stage-1 mode are left unchanged
(−1 sentinels). These field meanings are draw-context §5.1's contract; Init's argument tuple is
`(1,1,1,0,1,1,0,3,4,5,−1,−1.0,1,−1)`. Evidence: Init `0x005D1300`, ID table `0x00AA1E30`.
The crew gate in §6.7.7 is render-properties integer **7, kEffectPriorityLevel**, not automata
detail: value address = integer-table begin + `7·0x28+0x1C = 0x134`, tested signed ≥3
at `0x005CFA63…0x005CFA76`.

**Effects (LD38).** CreateDustClouds creates one visual-effect prop, AddRefs and initializes its
occupant, positions it at the building occupant's position, and chooses its effect name from the
maximum **horizontal scaffold extent**: <10 `small_dust`, [10,20) `medium_dust`, ≥20 `large_dust`.
It associates the effect occupant with the lot (prop manager slot 25), enables it with effect slot
7(true), and retains it as the current effect at +0x13C. No random draw, custom scale, lifetime,
direction or area is supplied. Entering removal state 3 calls its effect slot 8(true), not another
creation. CreateSparks similarly creates initialized visual-effect occupants at the building position,
names them `welding_sparks`, calls effect slot 8(true), associates them with the lot, and retains
them in the spark vector; counts and later scatter/enable calls are §6.7.7's. There is no additional
effect named scaffold: its dust/removal effect is this current dust prop, while scaffold geometry is
the mesh above. Evidence: `0x005CBC10…0x005CBD12`, `0x005CEC20…0x005CED6E`,
state-3 effect action in `0x005D0680`.

**Prop anchors and crew booleans (LD37).** After the orientation selects footprint sizes fx,fz,
CreateAndPlaceProp places its centre at `min.x+drawnX+fx/2`, `min.z+drawnZ+fz/2`.
Half-sizes are float32 stores after multiplication by double 0.5; the drawn offset, minimum and
half-size are added before the position float32 store (`0x005CCFE9…0x005CD062`). Its y is the
lot's building occupant's **position.y**, when the lot developer returns that building for this lot;
otherwise y is the maximum terrain altitude at the four chosen footprint corners (first sample
initializes, later strictly greater samples replace). The area's minimum y is not an anchor.
Position is set before rotation. For a ground prop the roof argument is false and occupant flag
0x10 is cleared. The prop is associated with the lot, then its footprint is removed from the input
area (`0x005CD066…0x005CD1F1`).

Crew record booleans start as **false, roofFlag**, in serialization order. The first is retained
through save/load but is not tested by this class's height updates; the second selects scaffold-following
crew (`0x005CE71E…0x005CE748`, `0x005CC3A2`). Initial crew y is terrain altitude at the centre
of its reserved 1×1 rectangle, including roof crew; SetScaffoldHeight subsequently raises roof crew.
RandomDirection2 **normalizes** the two drawn components using reciprocal square-root length,
returning float32 components. CreateAutomata sends `(component0,0,component1)` to automata slot 14.
Its rejection condition requires squared length both >1 and below a tiny lower threshold, so finite
draws never repeat; a zero drawn length has no fallback direction. Evidence:
`0x0080B6CF…0x0080B727`, `0x005CE634…0x005CE677`, `0x005CE6C9…0x005CE714`.

**RemoveRectangle (LD38).** For input `[xa,xb]×[za,zb]` and cut `[rx0,rx1]×[rz0,rz1]`,
the candidate rectangles, in tie priority order, are `[xa,xb]×[za,rz0]`,
`[rx1,xb]×[za,zb]`, `[xa,xb]×[rz1,zb]`, `[xa,rx0]×[za,zb]`.
There is **no clipping or intersection precheck**. Candidate scores are signed extent products,
not absolute areas and not a geometric difference union. The first score strictly greater than zero
and subsequent strictly greater scores select the retained candidate; equal scores keep the earlier
one. No positive score makes the area empty. Candidate y bounds retain the area's y bounds.
A crossing or disjoint cut can therefore enlarge the result beyond the original area. Evidence:
`0x005CCD70…0x005CCE31` (candidates), `0x005CCE35…0x005CCF13` (selection).

#### 6.7.16 Conformance vectors for LD35–LD38

These vectors belong to the construction owner; render/effect services may be recording doubles.
No frame-time RNG trace is needed for the deterministic cases.

| Case | Inputs | Expected |
|---|---|---|
| Phase endpoints | ground 10, startY 12, segH 4, N 3, span 12, counter 2; A=B=C=1, E=2 | At msAcc 0/250/500: state 0 height 10/11/12; state 1 height 16/18/20; state 3 height 24/17/10 |
| Completion divisor | same, state 2 | msAcc 250 gives completion 5/12; 500 gives 1/2; 1000 gives 2/3, each rounded float32; exposes a missing division by E |
| Zero phase | state 0, A=0, msAcc=0 | startY; state 1 with B=0 at zero elapsed propagates NaN rather than a fabricated endpoint |
| Quantized removal | startY 0.1, topY 8, segH 4 | N=2, span=8; removal starts at r24(startY+8), not topY |
| Height propagation | h=20, postWidth 1, crane extra 3 | side tops/planks/girder/roof props/roof crew y=20; post tops 21; crane model and crane-post tops 23; saved/world bounds and all bases unchanged |
| Scaffold topology | x0=0,x1=10,z0=0,z1=8,p=2, ground=1,topY=9 | four side quads, four plank quads, inner girder on [2,8]×[2,6], four posts × five quads; no bottom faces |
| Inner x-long | box x [0,40], z [0,20], back 4, reach 12, positive width | offset 4; side 0 position (16,10), side 2 (24,10); half-swing asin(10/12), centred at 2π or π |
| Inner z-long | box x [0,30], z [0,40], back 4, reach 12 | side 1 (15,24), side 3 (15,16); full ranges [0,π] or [π,2π] because half-short 15≥12 |
| Fit equality | long extent=back+reach | reject this model; the side draw has already happened |
| Crane lifetime | successful CreateCrane(x,z), then SetScaffoldHeight(20), extra=3 | saved y first 0, then 23; resource failure still rebuilds the post mesh |
| Draw order | rotation 1, zoom column 2 | side order 2,0,3,1; texture instances 0x696F3002,0x696F2002,0x696F1002,0x696F2002; blend false,false,true,false |
| Removal draw | all buffers present, state 3 | sides, girder, planks and 80 scaffold-post vertices drawn; no crane-post/model draw; shade/default/depth restored |
| Relighting | lighting returns RGB (201,100,51) for every normal | all relevant vertices get BGR (25,50,100), alpha retained; positions unchanged |
| Dust thresholds | maximum scaffold x/z extent 9.99,10,19.99,20 | small_dust,medium_dust,medium_dust,large_dust; building position retained |
| Prop anchor | area min (10,−100,20), footprint 4×2, drawn offsets 3,5; building position.y=7 | prop centre (15,7,26); area minimum y ignored; without building and corner altitudes 2,8,5,3, y=8 |
| Crew flags/direction | ground and roof crew; direction draws 0.3,0.4 | saved pairs (false,false),(false,true); direction approximately (0.6,0,0.8), not (0.3,0,0.4); only roof crew follows scaffold |
| Crew render gate | kEffectPriorityLevel=2,3 | 2: no crew or crew random draws; 3: execute crew placement |
| Crossing cut | area [0,10]×[0,10], cut [−5,−4]×[2,8] | right candidate [−4,10]×[0,10], score 140, wins; no clipping to x=0 |
| Cut tie | area [0,10]×[0,10], cut [4,6]×[4,6] | all scores 40; keep first [0,10]×[0,4] |

---

## 7. Demolition and explosion *[compat]*

### 7.1 `ClearLotBuildings(lot, state, removeFoundation, flags)` (`0x006C3C50`, pop 16)

**Slot 38 `GetFirstBuilding(lot)`** (`0x006C9100`, pop 4, EAX, no reference) (LD15): `r := lot.GetBoundingRect()`;
`L :=` an A list; `occupantManager.GetOccupantsByStandardCityCells(L, &(x1, z1), &(x2, z2), 0x278128A0, 1)` (slot 41).
If the call succeeds and `L` is not empty, the **first** entry only is QueryInterfaced for `cISC4BuildingOccupant`
(`0x87DDA3A9`) and that reference is released at once; then occupant manager slot 48 releases the list. Returns
the interface pointer, or null (empty list, failed query, or a first entry that is not a building). Nothing is
AddRef'd for the caller.

`lot.SetState(state, 0, 0)`; then while `b := GetFirstBuilding(lot)` (slot 38) is not null: AddRef `b`;
`propManager.RemoveBuildingProps(b)` (slot 38); if `removeFoundation`, `RemoveBuildingFoundation(b)` (slot 60);
`occupantManager.RemoveOccupant(b, 1, flags)` (slot 21); release. Finally returns `RemoveBuildingZotProp(lot)`
(slot 11).

### 7.2 Slots

| Slot | Behaviour |
|---|---|
| 12 `StartDemolishLot(lot)` | if lot flags & 1: `flora.DemolishFloraInCellArea(rect, 0)` (flora slot 20); `propManager.RemoveLotProps(lot)` (slot 28); zot := 0; `RemoveNetworkConnectionNodes(lot)`; `ClearLotBuildings(lot, 5, true, 0)`. Returns its AL |
| 13 `EndDemolishLot(lot)` | if `lot.IsInitialized()`: `SetState(6, 0, 0)`; `ClearLot(lot)`; if zone ∈ {0, 11, 12, 15}: `lotManager.DeleteLot(lot)` (slot 24) |
| 14 `ExplodeLot(lot, burnt)` | below |

The retaining-wall helpers below are `__cdecl` static functions of `cSC4LotRetainingWallOccupant` taking the
`cISC4Lot*` (caller pops; LD3): `AddRetainingWallOccupant(lot, bool add)` `0x005E3070` (8 bytes, AL), with `add =
false` removing the lot's wall occupant (type `0x895D1169`) and clearing lot flag 4, with `add = true` building one
when lot flag 1 is set; `UpdateRetainingWalls(lot)` `0x005E37F0` (4 bytes; AL is a leftover, slot 62 returns it);
`UpdateSurroundingRetainingWalls(lot)` `0x005E3960` (4 bytes, no return), which runs `UpdateRetainingWalls` on every
lot from `lotManager.GetLotsSurroundingLot(lot, list, 0xF)` (B list). They use the global occupant manager
(`0x00B43D0C`), lot manager (`0x00B43D08`) and BDS (`0x00B43D68`). Their rows for direct calls are in the
re-answers12 ABI proposal.

**`ClearLot(lot)`** (`0x006C8A20`): retaining-wall helper `0x005E3070(lot, 0)`; `RemoveLotFoundation(x, z)`
(slot 53); `UpdateSurroundingLotFoundations(lot)`; `0x005E3960(lot)` (surrounding retaining walls);
`SetVerticesAreLocked(false)`; `SetState(1, 0, 0)`; `SetBuildingType(0)`; `SetLotConfiguration(null)`;
`SetEmptyLotBaseTexture(lot)`. So `EndDemolishLot` leaves a non-deleted lot in state **1**, not 6.

**`ExplodeLot(lot, burnt)`** (`0x006CC250`, pop 8):
1. State 5 or 6 → nothing.
2. `b := GetFirstBuilding(lot)`. If `b`: create a `cSC4PropOccupantBurntBuilding` (0xD8 bytes) from it
   (`SetBoundsFromOccupant`, flag `0x40`, `Init`); `s :=` the larger of its bounding box's x and z extents.
   Construction details (LD14): allocation `0x005EDAE0`, constructor `0x005EF810` with argument 1, AddRef;
   `SetBoundsFromOccupant(b.AsOccupant())` is non-virtual (`0x005EEB70`, `__thiscall` on the object base, pop 4);
   then on the occupant sub-object at +8: slot 29 (`+0x74`) with `0x40` and `Init` (slot 3). The class is
   `0xC9EFB016` and is registered in the COM directory with a factory that builds the same object with argument 1
   (`0x005EFC80`, registration `0x004607C6`); its `QueryInterface` answers `cISC4PropOccupant` `0xE9793A65` with the
   base.
3. `ClearLotBuildings(lot, 6, false, 4)`; `RemoveLotProps(lot)`; `RemoveNetworkConnectionNodes(lot)`.
4. If there was a building: a terrain-decal prop with key `{0x7AB50E44, 0x1ABE787D, burnt ? 0x49C80EC7 : 0x2A2EF55D}`,
   size `2s`, alpha 0.7, at the building position, added with `AddLotProp(lot, decal)`; then
   **`AddLotRubbleProps(lot, bboxMin, bboxMax, burnt)`** (draws); then `AddLotProp(lot, burnt building)`.
   The decal (LD14) is `cSC4PropOccupantTerrainDecal`, class `0x2977AA48` (registered: factory `0x005EF180`,
   registration `0x00460790`; allocation `0x005ED6E0`, constructor `0x005EE360`, no argument). Its primary
   interface (IID `0xE982078A`, or the class ID, `QueryInterface` `0x005ECB10`) has slot 3 `SetDecalTexture(const
   key*, float size)` (`0x005ED7E0`), slot 4 `SetOpacity(float)` (`0x005ED840`), slot 6 `SetDecalSize`; the
   occupant sub-object is at +0xC. Order: AddRef; occupant `Init`; `SetDecalTexture(&key, 2s)`; `SetOpacity(0.7)`;
   occupant `SetPosition(building position)` (occupant slot 9; the position is the building occupant's slot 8);
   occupant slot 29 with `0x40`; `propManager.AddLotProp(lot, decal occupant)` (prop manager slot 25). Both props
   are added through prop manager slot 25 with their occupant sub-objects (`+8` burnt building, `+0xC` decal) and
   released after.
5. If `burnt`: `SetCondition(6)`. Finally `SetBuildingType(0)`.

### 7.3 `AddLotRubbleProps(lot, min, max, burnt)` (`0x006C4BD0`, pop 16)

```
dx := r24(max.x − min.x);  dz := r24(max.z − min.z);  area := r24(dz · dx)
small := dx < 10 or dz < 10;  large := dx > 12 and dz > 12           (NaN: false)
count := trunc(r24(area · 0.0078125)) + 4                             (0.0078125 = 2^-7, exact)
for i in 0 .. count−1:
    if i < 4:                                                         -- corners
        if small: continue                                            (no draw)
        if Uniform(2) = 0: continue                                   (file RNG)
        d2 := large ? Uniform(burnt ? 2 : 6) : 0                      (draw only when large)
        orient := (d2 = 0) ? (i + 2) mod 4 : i
        type := burnt ? {0x1C480000, 0x1C460000}[d2] : T6[d2]
        pos := (xf[i] ? max.x : min.x, min.y, zf[i] ? max.z : min.z)  with xf = {1,0,0,1}, zf = {1,1,0,0}
    else:                                                             -- interior
        orient := Uniform(4)
        type := burnt ? {0x1C470000, 0x1C490000}[Uniform(2)] : T6[Uniform(6)]
        pos.x := float(RandomDoubleRangeGaussian(min.x, max.x));  pos.z := float(RandomDoubleRangeGaussian(min.z, max.z));  pos.y := min.y
    if type = 0: continue
    occ := propManager.CreateProp(type)                               (slot 15; skip if it fails)
    Init; SetFlag(0x40); SetOrientation(orient); SetPosition(pos); (bbMin, bbMax) := GetBoundingBox()
    clamp x: if bbMin.x < min.x: pos.x := r24(pos.x + r24(min.x − bbMin.x))
             elif bbMax.x > max.x: pos.x := r24(pos.x + r24(max.x − bbMax.x))      (same for z)
    SetPosition(pos);  propManager.AddLotProp(lot, occ) (slot 25);  release
T6 = {0x1DCF0000, 0x1DD00000, 0x1DD10000, 0x1DD20000, 0x1DD30000, 0x1DD30000}
```

The Gaussian draws use the file RNG's multiplicative state (foundations §6.1 `RandomDoubleRangeGaussian`, with its
cached second value), so they interleave with the integer draws only through the shared object.

---

## 8. Night lights *[compat]*

### 8.1 The deque

- Building inserted (`0x006C8F80`): QueryInterface the occupant for `cISC4BuildingOccupant` (`0x87DDA3A9`), AddRef,
  append at the back, call `b.SetLit(litFlag)` (building slot 17), then re-derive the cursor from its index.
  `litFlag` is written only by the constructor (false) and by `Read`, so new buildings are inserted **unlit** even at
  night *[quirk]*.
- Building removed (`0x006C8830`): find its first occurrence; if the cursor is on it, the cursor moves to the next
  element; otherwise the cursor keeps its position (its index is decremented if it was after the erased element).
  Release once.
- The symbol port names the container `std::deque<int>` (identical code folding); it holds building pointers.

### 8.2 Day / night change (`0x44778234`)

Only the 24-hour clock posts this message: `ChangeVisualTime` on an hour change that crosses the night boundary
(`0x00445701`) and `EnableDayNightTransitionNotifications(true)` (`0x0044566E`; clock24.md §4.4-§4.5). Every other
use of the ID in the executable is a subscription or a compare. A city whose visual time never crosses the
boundary (for example a turbo run with a starved idle loop, CK22) never runs §8.2 or the batches (LD28).

With `mode := clock.GetVisualTimeMode()` (24-hour clock `0x00B43DBC`, slot 6):
- night (data2 = 0) and `mode = 2`, or day (data2 = 1) and `mode = 1`: **immediate** update `UpdateAll(mode)`
  (`0x006CBE50`): `isNight := (mode = 2)`; apply §8.4 to every building from the front; cursor := front;
  `nightMode := 0`. Then post `0xA6B79621`.
- otherwise: `nightMode := 2` (night) or 1 (day); **`random_shuffle`** of the whole deque (`0x006C68C0`: for i = 1 ..
  n−1, `j := rand() mod (i + 1)`, swap elements i and j; n − 1 `rand()` calls, foundations §12.3); cursor := front.

### 8.3 Batches (animation tick)

```
nightAcc += ms
if nightAcc ≤ 200: return                       (hard-coded 200; unsigned)
isNight := (nightMode = 2);  nightAcc := 0
repeat nightBatch times (none if nightBatch = 0):
    if cursor = end: nightMode := 0; stop
    b := *cursor; advance cursor; Light(b, isNight)
```

### 8.4 `Light(b, isNight)` (`0x006C8E10`)

1. `b.SetLit(isNight)`.
2. `(x1, z1, x2, z2) := b.AsOccupant().GetBoundingCityCells()` (occupant slot 11);
   centre `((x1 + x2) >> 1, (z1 + z2) >> 1)` (arithmetic shift).
3. `lot := lotManager.GetLot(cx, cz, false)`; none → stop.
4. **Discard the building rectangle** and take the lot's own: `lot.GetBoundingRect(&r)` (lot slot 14, `0x006BF320`,
   four `MOVZX` byte reads of lot +0x20..+0x23 = `x1, z1, x2, z2`), written into the **same** stack buffer that held
   the building's cells (`0x006C8E6F`). Then props in **that** rectangle:
   `occupantManager.GetOccupantsByStandardCityCells(list, &(x1, z1), &(x2, z2), 0xA823821E, −1)` (slot 41; A list).
5. For each prop with property `0x4A9F188B` present and true (variant slot 63): `PostOccupantMessage(0x09D8E0B6, 0)`
   (the same message for day and night). Release each; free the list.

**The night-light rectangle is the lot's, not the building's** *[compat]* (answers §12.2 Q8). `GetBoundingCityCells`
(step 2) feeds **only** the centre-cell `GetLot` lookup; it is overwritten before step 4 and never reaches the prop
query. An implementation that passes the building's cells to `GetOccupantsByStandardCityCells` is wrong whenever the
building does not exactly fill its lot — the two rectangles differ for any building whose footprint is smaller or
larger than the lot, and the prop set (hence which props receive `0x09D8E0B6`) differs with it. Evidence: `0x006C8E37`
(occupant slot 11 into `[ESP+0x18]`), `0x006C8E5B` (`GetLot`), `0x006C8E6F` (lot slot 14 into the same `[ESP+0x18]`),
`0x006C8F0B` (slot 41 with pointers to that buffer).

---

## 9. Power changed (`DoPowerChanged`, `0x006CBF40`) *[compat]*

For the rectangle `(x1, z1, x2, z2)` of message `0x8A07AE00`:

```
for lot in GetLotsInCellRect(list, x1, z1, x2, z2, true):              (lot manager slot 7; B list, freed with B)
    b := lot.GetBuilding()
    (cx, cz) := b ? building centre (as in §5.2) : lot.GetLocation()
    powered := BDS.CheatIgnorePower() or power.IsPowered(cx, cz)       -- no "needs power" test here
    if powered ≠ lot.GetHasPower():
        lot.SetHasPower(powered)
        if not powered and condition = 0: SetCondition(1)
        if powered and condition = 1: SetCondition(0)
    needs := (b = null) or lot.GetNeedsPower()
    if not powered and needs:
        if b or IsEmptyLotSuitableForZot(lot, x, z): AddBuildingZotProp(lot, 0x0FD10000, 4)
    elif zot = 4: RemoveLotProp(lot, 4); zot := 0                        -- also for empty lots
```

`(x, z)` in the empty-lot test is the lot location. No counters change and no state changes here.

---

## 10. Random draws *[compat]*

### 10.1 Generators

| Generator | Instance | Seed | Used by |
|---|---|---|---|
| **file RNG** | file-local `cRZRandom` at `0x00B49AF0` (static initialiser `0x00A74E00`) | `cRZTime()` (local wall clock, `0x00470B50`) **XOR `0x6C09C0F4`**, once at program start; never re-seeded | every draw in R2–R14 below; it is referenced only by this class (13 code references: `0x006C4C89`, `0x006C4CB7`, `0x006C4D31`, `0x006C4D42`, `0x006C4D73`, `0x006C4D93`, `0x006C5496`, `0x006C60E3`, `0x006C6E12`, `0x006C78F3`, `0x006CB433`, `0x006CD030`, `0x006CD088`) |
| temporary | stack `cRZRandom` | `(x << 16) + z` of the lot location | R1 |
| C `rand()` | process-wide | state 1, never seeded (foundations §12) | R15 |

FreeCity4 seeds the file RNG from the host (foundations §6.3) *[choice]*; tests use fixed seeds.

The construction occupant has its **own** file-local generator (`0x00B22FB0`) and uses C `rand()` for its shuffles;
its draws are listed in §6.7.7, not here. None of them affects the lot developer's draws above.

### 10.2 Draws

| # | Operation | Generator | Method (steps) | Arguments | Condition |
|---|---|---|---|---|---|
| R1 | TestDesirability | temporary | `RandomSint32RangeUniform(0, 7)` (1) | | lot livable, developer not purpose 5, desirability low |
| R2 | PickUnitToJoin | file | `Uniform(count)` (1) | candidates | purpose 5–8, no unit, ≥ 1 candidate |
| R3 | TestCoffeeBar | file | `Uniform(100)` (1) | | cheat on, purpose 2; drawn before any other test |
| R4 | PickLotBaseTextureForLot | file | `Uniform(n)` (1) | object's ID count | per texture object with ≥ 1 ID whose first ID ≠ 0 |
| R5 | CreateRandomLotProp | file | `Uniform(n)` (1) | expanded ID count | per prop object (bit 0 clear) with a non-empty expanded list |
| R6 | PickFloraTypeForLot | file | `Uniform(n)` (1) | object's ID count | per flora object with ≥ 1 ID |
| R7 | PickFloraTypeForLot | file | `Uniform(size)` (1) | family set size | only when R6's ID is a family with ≥ 1 member |
| R8 | AddLotFoundation (slot 54) | file | `Uniform(n)` (1) | foundation object's ID count | the foundation object has an ID list |
| R9 | rubble corner | file | `Uniform(2)` (1) | | corner, not small |
| R10 | rubble corner | file | `Uniform(burnt ? 2 : 6)` (1) | | corner, R9 ≠ 0, large |
| R11 | rubble interior orientation | file | `Uniform(4)` (1) | | interior |
| R12 | rubble interior type | file | `Uniform(burnt ? 2 : 6)` (1) | | interior |
| R13 | rubble interior x | file | `RandomDoubleRangeGaussian(min.x, max.x)` (multiplicative state; polar method, may consume a cached value) | | interior |
| R14 | rubble interior z | file | `RandomDoubleRangeGaussian(min.z, max.z)` | | interior |
| R15 | night shuffle | `rand()` | `random_shuffle`: n − 1 calls | deque size n | day / night message without immediate update |

### 10.3 Order inside an operation

- **UpdateLot** (grown building): R1 → R3 → (business unit) R2. The tests between them draw nothing.
- **UpdateTravelDesignates** dissolution: R2 per member tried, in member order, until one joins.
- **StartConstruction**: R2 (PickUnitToJoin, right after `SetState(2)`) → R8 at the water branch → R8 at the slope
  branch (at most two R8, the second only if the first did not place a foundation).
- **Completion** (queued `0xE8BDA2E7` 2 → 3): R4 for each texture object in array order → R5 for each prop object →
  R6 / R7 for each flora object. A queued `0xE99B3D4D` (condition change of a lot in state 3 or 4) adds R4 draws.
- **Init on a loaded city**: R4 for every lot with a configuration, in lot-list order (§1.3 step 12).
- **ExplodeLot**: rubble draws per index i: corners R9 [R10]; interiors R11, R12, R13, R14.
- **UpdateLotElements** (slot 37, called by the BDS): R8 when it adds a foundation.
- **AddLotFoundation with walls** (slot 55, §17.5.4): R8 in slot 54 first; then, after the retaining wall,
  `UpdateSurroundingLotFoundations` runs `UpdateLotFoundation` (slot 61) on each surrounding lot with foundation
  flags, in list order: R8 for each one whose foundation occupant is missing (slot 54 path), none otherwise.

---

## 11. Interfaces *[compat]*

All slots are `__thiscall`. Pops come from `docs/abi/vtables.tsv` and were re-checked with
`tools/abi_extract.py --check` on the addresses below; returns were read from the epilogues.

### 11.1 `cISC4LotDeveloper` (vtable `0x00AB1B98`, 65 interface slots + slot 65 destructor)

Overload pairs are compiled in reverse header order (foundations §16): 16 / 17, 54 / 55, 56 / 57, 58 / 59 (each
verified from the bodies).

| Slot | Off | As compiled | Pop | Return | Address | SDK |
|---|---|---|---|---|---|---|
| 0 | +0x00 | `QueryInterface(iid, void**)` | 8 | AL | `0x006C3B80` | ok |
| 1 | +0x04 | `AddRef` | 0 | EAX | `0x004F7BA0` (shared) | ok |
| 2 | +0x08 | `Release` | 0 | EAX = 0 always | `0x006C49D0` | ok in shape (count not returned) |
| 3 | +0x0C | `bool Init()` | 0 | AL | `0x006CB9A0` | ok |
| 4 | +0x10 | `bool Shutdown()` | 0 | AL (1) | `0x006C7380` | ok |
| 5 | +0x14 | `uint32 GetSimulatorType()` → `0x6818E5C9` | 0 | EAX | `0x006C3C40` | ok |
| 6 | +0x18 | `bool StartConstruction(lot, cfg, uint32 type, bool animate)` | 16 | AL | `0x006CC490` | ok |
| 7 | +0x1C | `bool ConstructDefaultLot(lot, cfg, uint32 type, bool)` | 16 | AL of slot 6 | `0x006CCEF0` | ok |
| 8 | +0x20 | `bool AbandonBuilding(lot, int32 reason)` | 8 | **undefined** (incidental leftover of `SetState`, which has no result) | `0x006C3D30` | **error**: no defined bool result (LD34) |
| 9 | +0x24 | `bool ReoccupyBuilding(lot, int32 devType)` | 8 | AL of `SetPopulation` | `0x006C3D50` | ok |
| 10 | +0x28 | `bool ChangeBuilding(lot, uint32 type)` | 8 | no defined flag from `SetState` (LD34); **0** when the lot has no configuration | `0x006C5210` | ok |
| 11 | +0x2C | `bool RemoveBuildingZotProp(lot)` | 4 | AL of the variant setter; 0 when no zot property | `0x006CB940` | ok |
| 12 | +0x30 | `StartDemolishLot(lot)` | 4 | AL of `ClearLotBuildings` (slot 11's) | `0x006CC1D0` | declared bool: value is incidental |
| 13 | +0x34 | `EndDemolishLot(lot)` | 4 | **undefined** (last call's EAX) | `0x006C90A0` | **error**: SDK `bool` |
| 14 | +0x38 | `ExplodeLot(lot, bool burnt)` | 8 | **undefined** | `0x006CC250` | **error**: SDK `bool` |
| 15 | +0x3C | `float GetTerrainAltitudeByVertex(int32 x, int32 z, bool withWater)` | 12 | ST0 | `0x006C4310` | ok |
| 16 | +0x40 | `float GetLowestOrHighestAreaAltitude(SC4Rect<float> const&, bool lowest, bool withWater)` | 12 | ST0 | `0x006C56C0` | order reversed vs header |
| 17 | +0x44 | `float GetLowestOrHighestAreaAltitude(SC4Rect<int32> const&, bool lowest, bool withWater)` | 12 | ST0 | `0x006C5620` | order reversed |
| 18 | +0x48 | `float GetAverageAreaAltitude(SC4Rect<int32> const&, bool withWater)` | 8 | ST0 | `0x006C5B70` | **error**: SDK `SC4Rect<float>`; the code reads int32 cells (§17.2) |
| 19 | +0x4C | `LevelAreaInternalVertices(SC4Rect<int32> const&, float target, bool skipWater, bool force, lot)` | 20 | **EAX = number of `ModifyTerrainHeight` calls** (§17.4) | `0x006C43D0` | SDK `bool`: AL is the count's low byte |
| 20 | +0x50 | `LevelBorderingVertices(SC4Rect<int32> const&, float h, bool unused, lot)` | 16 | EAX of `ModifyTerrainHeight` (a count) | `0x006C44B0` | SDK `bool` (same caveat) |
| 21 | +0x54 | `AttemptAreaLevelling(SC4Rect<int32> const&, float unused, bool withWater, bool perVertex, float maxAreaSlope, lot)` | 24 | EAX = 0, or `ModifyTerrainHeight`'s count | `0x006C44F0` | SDK `bool` (same caveat); arguments as landfill.md §4.5.3 |
| 22 | +0x58 | `GetOccupantCellCoverage(occupant, SC4Rect<int32>&)` | 8 | AL = x87 status flags left by `PositionToCell` (no result; §17.2) | `0x006C4590` | SDK `bool`: no caller reads it |
| 23 | +0x5C | `bool IsAreaDeveloped(SC4Rect<int32> const&)` → always false | **4** | AL = 0 | `0x005DEB40` (shared stub) | **error**: SDK `(void)`, pop 0 |
| 24 | +0x60 | `bool IsLotDeveloped(lot)` = slot 23 on the lot rectangle → false | 4 | AL | `0x006C5C00` | ok |
| 25 | +0x64 | `bool IsAreaFlat(SC4Rect<int32> const&, bool)` | 8 | AL | `0x006C5C30` | ok; difference from the first vertex (§17.2) |
| 26 | +0x68 | `bool IsLotFlat(lot, bool)` | 8 | AL of 25 | `0x006C5CC0` | ok |
| 27 | +0x6C | `float GetAreaAltitudeRange(SC4Rect<int32> const&, bool)` | 8 | ST0 | `0x006C5CF0` | ok |
| 28 | +0x70 | `float GetLotAltitudeRange(lot, bool)` | 8 | ST0 (of 27) | `0x006C5D90` | ok |
| 29 | +0x74 | `float GetAreaSlope(SC4Rect<int32> const&, bool)` | 8 | ST0 | `0x006C5DC0` | ok; §11.3 |
| 30 | +0x78 | `float GetLotSlope(lot, bool)` | 8 | ST0 (of 29) | `0x006C5F20` | ok |
| 31 | +0x7C | `float GetOccupantSlope(occupant, bool)` | 8 | ST0 (of 29 on slot 22's rectangle) | `0x006C5F50` | ok |
| 32 | +0x80 | `int64 GetLotDevelopmentCost(SC4Rect<int32> const&)` | 4 | EDX:EAX | `0x006C5F80` | ok; §11.3 |
| 33 | +0x84 | `bool RemoveLotFromItsBusinessUnit(lot)` | 4 | AL of unit `Remove` | `0x006C7320` | ok |
| 34 | +0x88 | `UpdateTravelDesignates(cISC4BusinessUnit*&)` | 4 | **undefined** | `0x006CB4D0` | **error**: SDK `bool` |
| 35 | +0x8C | `UpdateTravelPopulations(cISC4BusinessUnit*)` | 4 | **undefined** | `0x006C6340` | **error**: SDK `bool` |
| 36 | +0x90 | `bool NeedsAnchorBuilding(SC4Rect<int32> const&, int32 purpose)` | 8 | AL | `0x006C7A70` | ok |
| 37 | +0x94 | `bool UpdateLotElements(lot, uint32 kind)` (terrain-change kind; only `kind ≠ 5` is tested, §6.4) | 8 | AL = 1 | `0x006CC880` | ok |
| 38 | +0x98 | `cISC4BuildingOccupant* GetFirstBuilding(lot)` | 4 | EAX, **no reference** | `0x006C9100` | **error**: SDK `cISC4Occupant*` (the pointer is the IID `0x87DDA3A9` interface) |
| 39 | +0x9C | `GetPositionForLotConfigurationObject(lot, cfg, obj, float* pos, int32& orient)` | 20 | **EAX = `&orient`** (AL is a pointer's low byte) | `0x006C4EF0` | **error**: SDK `bool` |
| 40 | +0xA0 | `GetPositionForBuilding(lot, float* pos, int32& orient)` | 12 | EAX of 39 (`&orient`); EAX = 0 when no configuration or primary object | `0x006C3CF0` | SDK `bool` (nonzero = found) |
| 41 | +0xA4 | `bool GetLotBaseTextureOccupant(lot, cISC4LotBaseTextureOccupant*&, bool create)` | 12 | AL | `0x006C41A0` | ok |
| 42 | +0xA8 | `GetBoundaryLotTexture(BoundaryType*, uint32&, uint8&, bool&)` | 16 | **EAX = a map node pointer**, AL replaced by the flip byte | `0x006C6CF0` | **error**: SDK `bool` |
| 43 | +0xAC | `void SetEmptyLotBaseTexture(lot)` | 4 | none | `0x006CC970` | ok |
| 44 | +0xB0 | `SetNormalLotBaseTexture(lot, cfg, bool)` | 12 | **undefined** | `0x006CA8A0` | **error**: SDK returns `cISC4Lot*` |
| 45 | +0xB4 | `bool UpdateLotBaseTextureOccupant(lot, bool)` | 8 | **undefined**: not the update's value (LD33, §6.6) | `0x006C4290` | **error**: no defined bool result |
| 46 | +0xB8 | `bool GetForceEmptyLotBaseTextures()` | 0 | AL | `0x006AAF40` (shared getter) | ok |
| 47 | +0xBC | `void SetForceEmptyLotBaseTextures(bool)` | 4 | none | `0x006C77D0` | ok |
| 48 | +0xC0 | `cISC4FoundationOccupant* GetFoundationByOccupant(occupant)` | 4 | EAX (via slot 50) | `0x006C3E10` | SDK `intptr_t` |
| 49 | +0xC4 | `cISC4FoundationOccupant* GetFoundationByStandardCityCell(int32, int32 x, int32 z)` | 12 | EAX | `0x006C9480` | SDK `intptr_t` |
| 50 | +0xC8 | `cISC4FoundationOccupant* GetFoundationByPosition(int32, float x, float z)` | 12 | EAX | `0x006C95E0` | SDK `intptr_t` |
| 51 | +0xCC | `cISC4FoundationOccupant* GetLotFoundation(lot)` | 4 | EAX (via slot 49) | `0x006C3E70` | SDK `intptr_t` |
| 52 | +0xD0 | `bool RemoveFoundation(int32, float x, float z)` | 12 | AL | `0x006C54E0` | ok |
| 53 | +0xD4 | `bool RemoveLotFoundation(int32 x, int32 z)` | **8** | AL | `0x006C3EB0` | **error**: SDK `(cISC4Lot*)`, pop 4. Removes the foundation found by slot 49 (occupant manager slot 21) and clears the lot's flags & 3 |
| 54 | +0xD8 | `bool AddLotFoundation(lot, cfg, bool, bool, bool)` | 20 | AL | `0x006C5460` | order reversed; draws R8 |
| 55 | +0xDC | `bool AddLotFoundation(lot, uint32 type, bool, bool, bool)` | 20 | AL | `0x006C8AB0` | order reversed |
| 56 | +0xE0 | `bool IsLotObjectFoundationRequired(occupant)` | 4 | AL | `0x006C3FF0` | order reversed |
| 57 | +0xE4 | `bool IsLotObjectFoundationRequired(float* min, float* max)` | 8 | AL | `0x006C5590` | order reversed |
| 58 | +0xE8 | `bool AddBuildingFoundation(building, cfg)` | 8 | AL | `0x006C40E0` | order reversed |
| 59 | +0xEC | `bool AddBuildingFoundation(building, uint32 type)` | 8 | AL (via call) | `0x006C4020` | order reversed |
| 60 | +0xF0 | `bool RemoveBuildingFoundation(occupant)` | 4 | AL (via slot 52) | `0x006C4150` | ok |
| 61 | +0xF4 | `bool UpdateLotFoundation(lot)` | 4 | AL (via call) | `0x006C3F80` | ok |
| 62 | +0xF8 | `bool UpdateLotRetainingWalls(lot)` | 4 | AL of `0x005E37F0` | `0x006C4190` | ok |
| 63 | +0xFC | `cISC4Occupant* CreateLotProp(lot, uint32 objectIndex, uint32 propID)` | 12 | **EAX = the new prop's occupant** (borrowed) or 0 (§15.5) | `0x006C6230` | **error**: SDK `bool`; the one caller uses the pointer |
| 64 | +0x100 | `bool GetLotConfigurationObjectOccupant(lot, uint32 objectIndex, cISC4Occupant*& out)` | 12 | AL; `out` holds a reference (§15.5) | `0x006CB760` | ok |
| 65 | +0x104 | scalar deleting destructor | 4 | EAX | `0x006C8810` | not in the interface |

**What a replacement returns where the original is undefined**: slots 13, 14, 34, 35 → true; slot 44 → the lot;
slot 42 → nonzero when a map entry was found; slot 2 → 0. No game caller reads those values *[inferred: the callers
found discard EAX]*.

Behaviour of the geometry and foundation slots (15–31, 39–41, 45, 48–64), in short *[compat]*; slots 15–22, 39–40,
42–43 and 48–61 are specified in full in §17:
- 15: `alt := terrain.GetAltitude(x + z · vertsX)` (terrain slot 12, vertex index); with `withWater`, the larger of
  `alt` and `terrain.GetWaterLevel` at the same index (slot 31; NaN → `alt`).
- 17: minimum (start `+FLT_MAX`) or maximum (start `−FLT_MAX`) of slot 15 over the vertices `x1 .. x2 + 1`,
  `z1 .. z2 + 1`. 16 (float rectangle in metres) clamps it, samples the interior vertices with slot 15 and the four
  edges every 4 m with the terrain's position getters (§17.2).
- 18: average of slot 15 over the vertices. 25: every vertex within `0xE900EA24` of the first vertex (with water;
  §17.2). 27: max − min over the vertices.
- 22: the occupant's bounding box shrunk by 0.05 on each side, converted with `city.PositionToCell` (slot 95).
- 19 / 20 / 21: terrain levelling inside / around a rectangle through `cISC4Demolition::ModifyTerrainHeight`;
  21 compares the area's slope with its arguments before levelling (§17.3, §17.4).
- 39: position and orientation of a configuration object on a lot (`CellCornerToPosition`, lot facing, §17.6).
  40: slot 39 for the primary building object.
- 41: finds the lot's base-texture occupant (`0x497F6D9D`), creating a `cSC4LotBaseTextureOccupant` (class
  `0xC97F987C`) when `create`. 45: refreshes it.
- 48–61: foundation occupants (type `0x68FD0C69`, IID `0xC8FCE101`) found by occupant, cell, position or lot;
  added through BDS slot 23; building foundation type from the primary building object, else building property
  `0x88FCD877`, else the `0x6916516B` default (§17.5). 42 / 43: boundary textures of empty lots (§17.7).
- 63 / 64: one configuration object's prop, created or found (§15.5).

### 11.2 `cIGZMessageTarget2` (vtable `0x00AB1B88`, sub-object +4, 4 slots)

| Slot | Method | Pop | Return | Address |
|---|---|---|---|---|
| 0 | QueryInterface (this − 4) | 8 | AL | `0x006C7340` |
| 1 | AddRef (this − 4) | 0 | EAX | `0x0068D230` |
| 2 | Release (this − 4) | 0 | EAX | `0x006C7360` |
| 3 | `bool DoMessage(cIGZMessage2*)` | 4 | AL (§3.2) | `0x006CD940` |

### 11.3 `cIGZSerializable` (vtable `0x00AB1B70`, sub-object +8, 6 slots)

| Slot | Method | Pop | Return | Address |
|---|---|---|---|---|
| 0 | QueryInterface (this − 8) | 8 | AL | `0x006C7350` |
| 1 | AddRef (this − 8) | 0 | EAX | `0x0068D250` |
| 2 | Release (this − 8) | 0 | EAX | `0x006C7370` |
| 3 | `bool Write(cIGZOStream&)` | 4 | AL = `GetError() = 0` | `0x006C66F0` |
| 4 | `bool Read(cIGZIStream&)` | 4 | AL | `0x006CD120` |
| 5 | `uint32 GetGZCLSID()` → `0xA990BFE0` | 0 | EAX | `0x006C3B00` |

### 11.4 Formulas of the cost slots

**`GetAreaSlope(rect, withWater)`** (slot 29, `0x006C5DC0`):
```
over vertices x = x1 .. x2+1 (outer), z = z1 .. z2+1 (inner):  a := GetTerrainAltitudeByVertex(x, z, withWater)
    lo := a and loPos := (x, z) when a < lo (strict; first minimum wins)
    hi := a and hiPos := (x, z) when a > hi (strict; first maximum wins)
d := r24(sqrt(r24(r24(dx·dx) + r24(dz·dz))))          (SC4GeometryUtilities::LengthOfLine2D 0x005D76A0; dx, dz = hiPos − loPos)
if d = 0: return 0.0
if d > 6: d := r24(d · 0.25)
if d > 5: d := r24(d · 0.3f)
if d > 4: d := r24(d · 0.35f); return r24(r24(hi − lo) / d)
if d > 3: d := r24(d · 0.4f)
return r24(r24(hi − lo) / d)
```
The scale steps are applied **in cascade** to the running value *[quirk]*. Distances are in vertex units, altitudes
in metres.

The `r24` formulas above assume ordinary PC 24. In a callback entered under Lua's PC 53, the distance helper,
cascade products, rise and division all use that live precision without an intervening float spill. Stored
heights/coordinates and loaded scale constants remain floats. Foundations §18.4 gives the exact boundaries
and a (3,4) distance, rise-13 case whose final float slopes differ by one ulp (VC29-PC).

**`GetLotDevelopmentCost(rect)`** (slot 32, `0x006C5F80`):
`w := x2 − x1 + 1`, `h := z2 − z1 + 1`, `s := r24(GetAreaSlope(rect, true) · 0.0625)`,
`cost := trunc64(r24(r24(curve(s) · w) · h))` (both products by `fimul`), returned in EDX:EAX.

**`cISC4Lot::GetOccupancy`** (lot slot 61, `0x006C0CF0`), used by TestCondition, is compiled as
`GetOccupancy(float& out)` with pop 4: it writes the result through the pointer and returns nothing in ST0
(SDK: `float GetOccupancy() const`; §12.3).

---

## 12. Quirks, open questions, corrections

### 12.1 Quirks (reproduced unless marked *[choice]*)

1. The day the lot list wraps, the pass visits the rest of the list plus `perDay` lots (§4.1).
2. `RotateLotToFaceRoad` is dead: its enabling byte `0x00B49AE9` is never set (§4.2).
3. Desirability tolerance is fixed per lot location by a coordinate-seeded generator (§5.1).
4. `AddBuildingZotProp(…, 6)` posts the "add" notification even when a higher-priority zot blocks the road zot
   (§5.6).
5. TestPower never removes an empty lot's power zot; the power-changed handler does (§5.2, §9).
6. `StartConstruction` removes only a road zot; other zots stay until their tests pass (§6.1).
7. `StartConstruction(…, animate = false)` and `ChangeBuilding` set state 3 even when no building was created.
8. The state-change handler applies its work to the lot found at the message lot's location, and only when the
   sizes match (§6.5).
9. New buildings join the night deque unlit; `litFlag` is never set at run time (§8.1).
10. The night interval property only seeds the accumulator; the threshold is a hard-coded 200 (§3.5, §8.3).
11. `0x47E2C401` is read without a count check (§3.5). (An earlier version said `0xE900EA29` was unused: it is
    the slot 19 threshold, §17.4.)
12. `GetAreaSlope` scales the distance in cascade (§11.4).
13. `EndDemolishLot` leaves a kept lot in state 1, after setting 6 (§7.2).
14. `Release` returns 0, not the remaining count (§1.4).
15. The message `0x278E6003` is subscribed but ignored.
16. `Init` on a loaded city re-picks every lot's base texture, consuming file-RNG draws (§1.3).
17. `AddNetworkConnectionNodes` appends a stale or uninitialised node after a failed `CreateConnectionNode`
    (§6.5, LD32). *[choice]* FreeCity4 skips it and counts the failure.

### 12.2 Open questions

1. Q1. *Answered* in §6.7 (phase lengths, `SetState(3)` point, draws). What remains open is listed in §6.7.10.
2. Q2. *Answered* in §6.7.8: `0xC0` is a priority (every-frame bucket); `data2` is the clamped frame delta in µs.
3. Q3. *Answered* (with landfill.md LG5): `0x00B43DD8` is the global `SC4ViewUtilities` pointer. `SL::OnView3DInit`
   stores the 3D renderer's `GetViewUtilities()` there (renderer slot 8, store at `0x006025DD`; the region screen
   does the same at `0x007AD0D0`), and `SL::OnView3DShutdown` clears it (`0x00602146`). `UpdateCameraPosition`
   (`0x007F83F0`) copies the camera control's rotation (`+0x10C`) into `+0x10` (`0x007F84C0`), so `+0x10` is the
   current view rotation 0…3. The lot developer reads it only for the zot orientation (§5.6, `0x006C99ED`); nothing
   checks the global for null. A replacement uses the view rotation, or 0 without a view.
4. Q4. *Answered* in §17: altitude helpers §17.2, the terrain writer §17.3, levelling §17.4, foundations §17.5,
   `GetPositionForLotConfigurationObject` §17.6, boundary textures §17.7; tests 28–38 of §16.
5. Q5. *Answered* (§5.7 step 2): `GetLotsSurroundingRect` deduplicates against the output list before every append,
   and all four side-flag calls share that one list, so a lot can never appear twice and no neighbour is
   double-weighted.
6. Q6. `GetLotsInCellRect`'s last argument (`true` in §9, `false` in `NeedsAnchorBuilding`): allocators.md documents
   `false`; the lot developer always passes an empty list, so the clearing step does not matter here.
7. Q7. *Answered* in §15.5: slots 12–14 are called only by the demolition utility (`DemolishRegion` and its
   deferred-demolition message) and by the port developer's lot swap; disasters and the bulldozer reach them only
   through `DemolishRegion`'s flags. Slot 63 is called only by the prop developer (timed props), slot 64 only by the
   lot configuration editor.

### 12.3 Corrections to other documents

| Document | Section | Wrong | Right (evidence) |
|---|---|---|---|
| `docs/subsystems/development.md` | §6 TestDesirability | `Random(...).Sint32RangeUniform(0, 7)` described as giving a 2..9 tolerance "≈3..10 months" | range is 0..6 (`RandomSint32RangeUniform(0, 7)` = `0 + Uniform(7)`), tolerance `r + 2` = 2..8 passes, failing on the 3rd–9th consecutive low visit (`0x006C48C5`–`0x006C48E8`) |
| same | §6 TestCondition | thresholds "distress 0.4, renew 0.6" | those are code defaults; shipped 0.45 / 0.5, and `[0]` (0.3 shipped) is the desirability threshold (`0x006C4A85`–`0x006C4AD8`) |
| same | §6 StartConstruction | "picks a business unit and clears zots" | only a road zot is removed; condition is set to 1; foundation / levelling order as §6.1 (`0x006CC490`) |
| same | §6 DoDailySimulation | "empty-lot checks when `noZotUntilDay < today`" | `today > noZotUntilDay`, with `noZotUntilDay = today + 31` set by CityEstablished `0x26D31EC4` (`0x006CDB5F`) |
| `foundations.md` | §6.2 | lists "lots" under the file-local `cRZRandomSuper` row; temporary-seed row marks `a` as inferred | the lot developer uses a file-local **`cRZRandom`** at `0x00B49AF0`, seed `cRZTime() XOR 0x6C09C0F4` (static initialiser `0x00A74E00`); the temporary generator's seed is `(x << 16) + z` with `x`, `z` from `GetLocation` (`0x006C48B6`) |
| `foundations.md` | §12.5 | "lot developer: `random_shuffle` of an `int` deque" | a deque of `cISC4BuildingOccupant*` (identical-code-folded `deque<int>` symbol), shuffled on the day / night message `0x44778234` (`0x006CDA36`, `0x006C68C0`) |
| `foundations.md` | §5.2 | condition 1 not described | condition 1 = occupied, otherwise normal, but the no-power counter is non-zero (`0x006C4757`), also used while constructing (`0x006CC4D6`) |
| `tract-developer.md` | §10.2 Q1 | open | answered in §5.7 (`NeedsAnchorBuilding`, `0x006C7A70`) |
| `tract-developer.md` | §10.2 Q5 | open | `AbandonBuilding` is `lot.SetState(4, 0, reason)` and synchronous (`0x006C3D30`) |
| `growth-developer.md` | §12 open question 4 | the abandonment-threshold keys are a lot-developer matter | the lot developer never calls growth developer slot 24 (no call in `0x006C3B00`–`0x006CDEF0`); its abandonment threshold is `0x47E2C401[0]`. Slot 24 has no caller anywhere in the exe (growth-developer.md §13.3 Q4, now answered) |
| `foundations.md` | §6.2 / §6.3 | construction occupant's file RNG `0x00B22FB0` seeded `cRZTime() XOR 0xEC09C32A` | that seed (static initialiser `0x00A733A0`) is overwritten before any draw: `StaticInit` reseeds it with plain `cRZTime()` at `0x005CEDAC` (§6.7.2) |
| `foundations.md` | §12.5 | construction occupant's linear iterators / "crews" | the iterators run in table mode (refill 0..n − 1, `random_shuffle` `0x006DB010`, n − 1 `rand()` per reset); they shuffle the 8 lot areas, the 4 spark sides and the ground / roof prop lists (§6.7.7) |
| Ghidra project names | `0x005CFE20`, `0x00913761` | `cRZRandom::RandomDoubleRangeGaussianFast`, `cSC4ParticlesEffect::RandomRange` | `0x005CFE20` is the construction occupant's spark scatter (tail-jumped from `DoDailySim` state 1); `0x00913761` is `cRZRandom::RandomDoubleRangeUniform` |
| gzcom-dll SDK `cISC4Lot.h` | slot 61 | `float GetOccupancy() const` | `void GetOccupancy(float& out)`, pop 4, result written through the pointer (`0x006C0CF0`, store at `0x006C0E8A`) |
| `demolition.md` | §4.7 | per-vertex is the only mode of `ModifyTerrainHeight` in shipped use | StartConstruction passes the configuration's water compatibility as slot 21's `perVertex`, so lots that are not water-compatible are levelled in area mode (§17.3, §17.4) |
| gzcom-dll SDK `cISC4LotDeveloper.h` | slots 18, 19–21, 39, 40, 42 | `SC4Rect<float>` for slot 18; `bool` returns | slot 18 reads `SC4Rect<int32>`; 19 returns a call count, 20 / 21 `ModifyTerrainHeight`'s count, 39 / 40 `&orient`, 42 a map node pointer (§11.1, §17) |
| this document (earlier version) | §3.5, §12.1 quirk 11 | `0xE900EA29` read but never used | used by slot 19 as the interior-vertex threshold (`0x006C43D0`, §17.4) |
| `lot-records.md` | Q4 | the five `0xFF` colour bytes of a base-texture record are unexplained | for empty-lot textures they are zone colour bytes 3, 2, 1, the zone alpha and 3 (`0x006CC970`, §17.7); for normal textures they stay `0xFF` |

---

## 13. Persistence (`cIGZSerializable`, sub-object +8, class ID `0xA990BFE0`) *[compat]*

**Write** (`0x006C66F0`), in this order (foundations §2.2 encodings):

| # | Field | Encoding |
|---|---|---|
| 1 | version = 1 | `SetUint16` |
| 2 | `vertsX` | `SetSint32` |
| 3 | `extentX` | `SetFloat32` |
| 4 | `extentZ` | `SetFloat32` |
| 5 | n = night deque size | `SetUint32` |
| 6 | each building, front to back | object reference, `SC4WriteGZSerializable` (`0x005D99A0`, foundations §2.7) |
| 7 | cursor index (cursor − front) | `SetUint32` |
| 8 | `nightMode` | `SetUint8` (low byte) |
| 9 | `litFlag` | `SetUint8` 0 / 1 (`operator<<` `0x005D9FD0`) |

Returns `GetError() = 0` (stream slot 18).

**Read** (`0x006CD120`): reads the version; any value other than 1 → return false with nothing more read. Then
fields 2–4 (overwriting), resizes the deque to n (null-filled, `0x006CCE30`), reads each element with
`SC4ReadGZSerializable(stream, 0x87DDA3A9, &elem, 0)`, the cursor index (cursor := front + index), `nightMode`
(uint8 into the 32-bit field), `litFlag` (`operator>>` `0x005D9FA0`: byte ≠ 0); sets `nightAcc := 0`; returns
`GetError() = 0`.

Not saved: daily state and iterator, the latches and snapshots, `noZotUntilDay`, `canCheckCommute`, tuning, the RNG.
A loaded city therefore starts a fresh daily cycle at the front of the lot list.

---

## 14. Dependencies *[compat]*

Pops from `docs/abi/interfaces.tsv` (checked rows) unless noted.

| Interface (global) | Slot | Method | Pop | Return |
|---|---|---|---|---|
| `cISC4City` (`0x00B43CEC`) | 31 / 33 / 34 / 35 / 50 / 53 / 56 / 59 / 64 / 67 / 69 / 84 / 86 | service getters | 0 | EAX |
| | 91 / 92 | CellWidthX / Z | 0 | ST0 |
| | 93 / 94 | cell counts X / Z | 0 | EAX |
| | 95 / 96 / 97 | PositionToCell / CellCornerToPosition / CellCenterToPosition | 16 | results through the out pointers (no ST0) |
| `cISC4Simulator` (`0x00B43D5C`) | 8 | GetSimDateNumber | 0 | EAX |
| | 19 / 20 | AddAgent(target, level, name, flags) / RemoveAgent(target) | 16 / 4 | AL |
| `cIGZMessageServer2` (`0x00B43CCC`) | 4 / 5 / 6 | MessagePost(msg, bool) / AddNotification / RemoveNotification | 8 | AL |
| `cISC4LotManager` (`0x00B43D08`) | 6 | GetLot(x, z, bool) | 12 | EAX |
| | 7 | GetLotsInCellRect(list, x1, z1, x2, z2, bool) | 24 | EAX |
| | 9 / 10 | GetLotsSurroundingLot(lot, list, flags) / GetLotsSurroundingRect(rect, list, flags) | 12 | |
| | 15 / 16 | GetSafeLotIterator / GetLotCount | 0 | EAX |
| | 24 | DeleteLot(lot) | 4 | none (EAX undefined: the last call is a `Release`, zones.md §8.6) |
| `cISC4Lot` (vtable `0x00AB19B0`) | 3, 9, 10, 12–15, 20–22, 25–31, 33–37, 40, 41, 43, 44, 46–48, 52, 55, 57–61, 63–67, 69 | as named in this spec | per `interfaces.tsv` (e.g. 34 SetState 12, 36 SetCondition 4, 61 GetOccupancy 4, 63 GetTravelJobs 4 ST0) | |
| `cISC4OccupantManager` (`0x00B43D0C`) | 20 / 21 / 24 | InsertOccupant(occ, 0) / RemoveOccupant(occ, a, b) / occupant changed (occ, 0) | 8 / 12 / 8 | AL |
| | 36 | GetOccupantsByBBox(list, min, max, filter, flags) | 20 | AL |
| | 37 | GetOccupantsByBBox(list, min, max, occupant type, flags) (type filter, forwards to 36) | 20 | AL |
| | 41 | GetOccupantsByStandardCityCells(list, min, max, type, flags) | 20 | AL |
| | 48 | ReleaseOccupantList(list) | 4 | |
| `cISC4PropManager` (`0x00B43D10`) | 15 / 24 / 25 / 26 / 28 / 29 / 38 | CreateProp / GetLotProps(list, lot) / AddLotProp(lot, occ) / AddLotProp(type, pos, orient) → occ / RemoveLotProps(lot) / RemovePropA / RemoveBuildingProps | 8 / 8 / 8 / 12 / 4 / 4 / 4 | AL or EAX |
| `cISC4BuildingDevelopmentSimulator` (`0x00B43D68`) | 7 / 15 / 19 / 23 / 30–33 / 39 / 50 / 54 / 55 / 58 | building-development.md §7.1 | 12 / 4 / 8 / 28 / 8, 4, 4, 8 / 4 / 4 / 0 | |
| `cISC4PowerSimulator` (`0x00B43DA0`) | 37 | IsPowered(x, z) | 8 | AL |
| `cISC4PlumbingSimulator` (`0x00B43D94`) | 11 | IsCellWatered(x, z) | 8 | AL |
| `cISC4TrafficSimulator` (`0x00B43DA8`) | 13 | GetTripLengthMap | 0 | EAX (uint8 grid, read with grid slot 7) |
| `cISC4DemandSimulator` (city slot 56) / `cISC4Demand` | 6 / 9 | GetDemand(0x2000, 0x20000) / QueryDemandValue | 8 / 0 | EAX / ST0 |
| `cISC4FloraSimulator` (`0x00B43D80`) | 11 / 14 / 17 / 20 | IsFloraTypeValid / GetFloraTypesByFamily / AddNewFloraOccupant / DemolishFloraInCellArea | 4 / 8 / 12 / 8 | |
| `cISC4GrowthDeveloper` | 7 / 13 / 20 | GetPurpose / GetDesirabilityMap / GetWaterSupplyCap | 0 | EAX |
| `cISTETerrain` (`0x00B43CF4`) | 6 / 7 / 8 / 12 / 31 | CellWidth / counts / GetAltitude(index) / GetWaterLevel(index) | 0 / 0 / 0 / 4 / 4 | ST0 / EAX / EAX / ST0 / ST0 |
| | 13 / 30 / 34 / 37 | GetAltitude(x, z) / GetWaterLevel(x, z) (float positions) / CellHasWater(x, z) / IsWaterVertex(x, z) | 8 | ST0 / ST0 / AL / AL |
| `cISC4Demolition` (`0x00B43D00`) | 7 / 8 | DemolishLots (8 args) / DemolishOccupant (7 args) | 32 / 28 | AL |
| | 9 | ModifyTerrainHeight (10 args, §17.3; it calls terrain 14, 37, 49, 51, 68) | 40 | EAX (count) |
| `cISC4ZoneManager` (`0x00B43D14`) | 15 / 16 | GetZoneDragColor(zone) / GetZoneDisplayAlpha | 4 / 0 | EAX / AL |
| `cISC4FoundationOccupant` (IID `0xC8FCE101`) | 3 / 4 / 9 | AsOccupant / GetFoundationType / GetKind (§17.5 table) | 0 | EAX |
| `cISC4LotBaseTextureOccupant` (vtable `0x00AA2E18`) | 3 / 8 | AsOccupant / SetTextureSpecification(records, count, 0) | 0 / 12 | EAX / — |
| `cISC4LotConfigurationManager` (`0x00B43D18`) | 8 / 27 | GetLotConfiguration / GetObjectIDArray | 4 / 8 | EAX |
| `cISC4LotConfiguration` | 3, 7, 10, 11, 14, 23, 24, 26, 27 | GetID, GetSize, GetMaximumSlopeBeforeFoundation (ST0), GetGrowthStage (AL), IsCompatibleWithWater, GetLotConfigurationObjectArray, GetPrimaryBuildingConfigurationObject, GetPrimaryFoundationConfigurationObject, HasObjectsOfType | | |
| `cISC4BuildingOccupant` | 3 / 11 / 17 | AsOccupant / (struct getter) / SetLit(bool) | 0 / 0 / 4 | |
| `cISC4Occupant` | 3, 8, 10, 11, 12, 15, 22, 28, 29, 32 | Init, GetPosition, GetBoundingBox, GetBoundingCityCells, SetRemovalFlags, PostOccupantMessage, IsOccupantGroup, SetLotTag, SetFlag, IsFlagSet | per `interfaces.tsv` | |
| `cISC4BusinessUnit` | 3 / 4 / 5 | Add / Remove / GetMembers | 4 / 4 / 0 | *[pops from the call sites]* |
| 24-hour clock (`0x00B43DBC`) | 6 | GetVisualTimeMode | 0 | EAX |
| animation-tick manager (service `0x86AD10EC`) | 5 / 6 | register / unregister (target, priority `0xC0` = every frame, §6.7.8) | 8 | |
| GZCOM | 3 | CreateInstance(class, iid, void**) | 12 | AL |
| resource manager (`0x00B43CA8`) | 3 / 14 | get exemplar / resource exists | | |

---

## 15. Replaceability

### 15.1 Who reaches into the lot developer

- Only through `cISC4LotDeveloper`: the tract developer (slots 6, 8, 9, 29, 36), the BDS (16, 30, 37), the coffee
  bar path of this class, and the callers of slots 12–14, 63 and 64 listed in §15.5. The pointer is reached through
  BDS slot 36, city slot 79 (`0x006E3000`, the city's `+0x13C`) or the global `0x00B43D20`. Five places read the
  global: BDS `Init` (`0x0069D495`), tract developer `Init` (`0x0070780A`) and zone developer `Init` (`0x0073405C`)
  store and AddRef it; `cSC4Lot::GetLotConfigurationObject` (`0x006BFEE8`) calls slot 39; the base-texture cell model
  (`0x005DFA18`, in `0x005DF9E0`) calls slot 46. (An earlier version listed only the first two.)
- No code outside the class reads its members *[inferred: no raw access found next to the global's readers]*.
- The file-local generator `0x00B49AF0`, the slicer statics `0x00B49B08`–`0x00B49B10`, the dead bytes
  `0x00B49AE8` / `0x00B49AE9` and the dword `0x00B49AEC` are private to the class's translation unit. `0x00B49AEC`
  is never written: it is a zero (0.0) that five functions copy into a position vector before slot 39 fills it
  (`0x006C6272`, `0x006C6DB2`, `0x006C9D43`, `0x006CA965`, `0x006CD310`). (An earlier version called it the cached
  lot-configuration-manager pointer.)

The class is replaceable through the GZCOM registry (class ID `0xA990BFE0`) provided the replacement:
- implements the compiled slot table (§11.1), including the pop-8 slot 53 and pop-4 slot 23;
- posts and handles the messages of §3 with the same queued timing;
- walks the lot manager's safe iterator **the way the original does** (it reads the iterator's two words and the
  owner's `std::list` nodes raw: next = node[0], lot = node + 8; and frees the iterator by removing it from the
  owner's iterator list at owner + 4 and calling C free). A replacement must either do the same or, standalone,
  iterate the lot list in the same order *[blocker for a pure-interface replacement: `GetSafeLotIterator` returns a
  raw structure, not an interface]*;
- uses allocator B / A for every crossing container as in §15.2.

### 15.2 Containers crossing an interface (allocators.md format; rows for the lead to paste)

Section (1), the game fills, the caller frees:

| Method (slot) | Container | Game allocator | SDK container compatible? | Notes | Evidence |
|---|---|---|---|---|---|
| `cISC4LotManager::GetSafeLotIterator` (15) | 8-byte iterator block `{current node, owner}` | C (`0x005E55E0`) | n/a (raw struct) | registered in the owner's iterator list (owner + 4, insert `0x0072BA10`); the caller walks the owner's list nodes raw, then unregisters (`std::list::remove` `0x00809580` on owner + 4) and frees the block with C (`0x005E5620`) | `0x00734850` → `0x00734860`; callers `0x006CD7A3`, `0x006C77E8`; free in `Shutdown` `0x006C7380` |
| `cISC4PropManager::GetLotProps` (24) | `ilist<cISC4Occupant*>` | A | yes | appends through occupant manager slot 40 (one reference each), then erases (Release, free node A) every element whose position is outside the lot's world rectangle, over the **whole** list. The lot developer passes a fresh list (head 12 bytes from the cached service `0x00B207E4`), releases each element, frees nodes with `0x00413E60` and the head with A slot 4 | `0x004A3240`; caller `0x006C9780` |
| `cISC4LotConfigurationManager::GetObjectIDArray` (27) | `std::vector<uint32>` | B | **no** | clears (end = begin), then appends the object's IDs with families expanded in hash order, B growth. The caller passes a zeroed vector and frees the buffer with `0x0090CF63` | `0x0048BA10`; caller `0x006C6D70` |
| `cISC4LotManager::GetLotsInCellRect` (7), `GetLotsSurroundingLot` (9), `GetLotsSurroundingRect` (10) | `std::list<cISC4Lot*>` | B | **no** | as the existing rows; the lot developer allocates the 12-byte head with `0x0090CF54`, releases each lot, frees nodes and head with `0x0090CF63` | callers `0x006CBF40`, `0x006C7A70`, `0x006CB260` |
| `cISC4OccupantManager::GetOccupantsByStandardCityCells` (41, typed) / `GetOccupantsByBBox` (36, filter) | `ilist<cISC4Occupant*>` | A | yes | as the existing rows; callers allocate the head with A (cached `0x00B207E4`) and end with slot 48 or `0x00413E60` + A free | callers `0x006C9100`, `0x006C8E10`, `0x006C9AD0`, `0x006C9480`, `0x006C95E0` |
| `cISC4BuildingDevelopmentSimulator::GetBuildingTypesByFamily` (19), `cISC4FloraSimulator::GetFloraTypesByFamily` (14) | STLport `hash_set<uint32>` | B | **no** | as the existing rows; caller-built sets of 193 buckets | callers `0x006CCEF0`, `0x006CD010` |
| `cISC4Demolition::DemolishLots` (7) | `std::list<cRZAutoRefCount<cISC4Lot>>` (input) | B (caller's) | read only | as the existing row; the coffee-bar caller builds a one-lot list and destroys it (`0x00427670`) | caller `0x006C78D0` |

Section (2): the lot developer's own interface passes **no** game-owned container across (slot 34 takes a
`cISC4BusinessUnit*&`, slot 41 a `cISC4LotBaseTextureOccupant*&`, slot 64 a `cISC4Occupant*&`; no lists).

### 15.3 Internal containers (no allocator row)

The night deque (`deque<cISC4BuildingOccupant*>`, B node buffers), the boundary map (B), the candidate vector and
lot-list copies of `PickUnitToJoin` / `UpdateTravelDesignates` (B), the texture record buffer of
`SetNormalLotBaseTexture` (B), and the business-unit objects (C). A replacement may use any containers, but must
keep the **orders** they impose: deque order and shuffle (R15), candidate order (R2), configuration-object array
order (R4–R6), hash-set iteration (R7, `ConstructDefaultLot`).

### 15.4 Grids

The lot developer **owns no grid** and reads none raw. Desirability (growth developer slot 13 → grid slot 7), trip
length (traffic slot 13 → grid slot 7), power (`IsPowered`) and water (`IsCellWatered`) are all read through
interface calls, one cell at a time. Grid ownership is the owners' business (simgrid §6.4).

### 15.5 Outside callers of slots 12–14, 63 and 64 (§12.2 Q7) *[compat]*

**How they were found.** Every value that BDS slot 36 (`0x00698CE0`) or city slot 79 returns, or that is read
from `0x00B43D20`, was followed to calls through the lot developer's vtable at `+0x30`, `+0x34`, `+0x38`, `+0xFC`
and `+0x100`. So was every member that caches the pointer:
- demolition `+0x24` (`+0x20` from its message sub-object);
- bulldozer tool `cSC4ViewInputControlDemolish` `+0x34`;
- landfill zone `+0x6C`;
- BDS `+0x6C`;
- tract developer `+0x28`;
- zone developer `+0x50`.

Every 3-argument call at `+0xFC` and `+0x100` in the executable was checked as well.

The lot developer never calls these five slots itself, and nothing calls their bodies directly: the only references
are the vtable entries. No disaster, tool or editor calls slots 12–14 directly; they all go through
`cISC4Demolition::DemolishRegion` (`0x00468410`).

| Slot | Caller | Call site | Arguments | What the caller does with the result |
|---|---|---|---|---|
| 14 `ExplodeLot` | `DemolishRegion`, when committing (`doIt`), for each lot in the region whose configuration (lot slot 9) is null or not water-compatible (configuration slot 14), when `flags & 0x300` | `0x00468F1C` | `(lot, (flags & 0x1000) = 0)` | ignores it |
| 12 `StartDemolishLot`, then 13 `EndDemolishLot` | `DemolishRegion`, every other lot when committing; it reads the zone type (lot slot 31) first, for the re-parcelling that follows | `0x00468F30`, `0x00468F39` | `(lot)`, `(lot)` | ignores it |
| 13 `EndDemolishLot` | the demolition utility's handler for message `0xA9DED179` (`0x00466FF0`), if `lot.IsInitialized()` (lot slot 6). Nothing posts this message (demolition.md) | `0x00467047` | `(lot)` | ignores it |
| 12, then 13 | the port developer's lot swap (`0x006E92A0`, airport and seaport growth, port-developers.md), through `BDS.GetLotDeveloper()` | `0x006E936B`, `0x006E9377` | `(old lot)`, `(old lot)`; then `ConstructDefaultLot(new lot, new configuration, 0, true)` (slot 7, `0x006E93E3`) | ignores it |
| 63 `CreateLotProp` | the prop developer's `SelectAndCreateProp` (`0x006F1620`), when the timed-prop record's cell has a lot (`lotManager.GetLot(x, z, false)`) | `0x006F1887` | `(lot, the record's object index (+8), the prop ID it chose)` | uses EAX as an occupant pointer: AddRef; `SetPropertyValue(0x89A1C16C, time slot)` on its property holder (occupant slot 6); Release; returns true. EAX = 0 → returns false |
| 64 `GetLotConfigurationObjectOccupant` | the lot configuration editor's `CycleSelectedObject` (`0x0047C660`), for a selected flora object (record type 4) | `0x0047C78F` | `(the editor lot at cell (1, 1), from lotManager.GetLot(1, 1, false); the selected record's index in the configuration's object array; &occ)` | on AL = 1: QueryInterface of `occ` for `cISC4FloraOccupant` (`0xC83DD057`), then it replaces the flora with the next type in the object's ID list (flora slots 19 and 17) |

**Who sets the explode bits.** These callers pass `DemolishRegion` or `DemolishLots` a `flags` value with
`0x300` set. demolition.md §9 lists every demolition caller.

| Caller | Address | Flags | `ExplodeLot` burnt argument |
|---|---|---|---|
| the fire disaster's `ToastOccupant` | `0x00579948` | cause \| `0x300` | true |
| a power plant's failure (`AgePowerPlant`; power.md) | `0x006ECB4E` | cause \| `0x300` | true |
| the automata group scrubber command, Lua key `explode` or `createRubble` | parsed at `0x005A357E` / `0x005A35A1`; `0x005BD0F0` passes the parsed flags to `DemolishRegion` at `0x005BD22F` | `0x1300` | false |
| the same command, Lua key `createBurntRubble` alone | `0x005A35C0` | `0x300` | true |

The bulldozer, zoning, developers and ordinances never set `0x300`, so they always clear lots with slots 12 and 13.

**`CreateLotProp(lot, objectIndex, propID)`** (slot 63, `0x006C6230`, pop 12). It returns the occupant in EAX, or
0 (zero means "no prop"):
1. If `lot` is null, or `cfg := lot.GetLotConfiguration()` (lot slot 9) is null: return 0.
2. `obj := cfg`'s object array (configuration slot 23) `[objectIndex]` (0x34-byte records, §17.1). There is no
   range check *[quirk]*.
3. `pos := (0, 0, 0)`, `orient`. Call `GetPositionForLotConfigurationObject(lot, cfg, obj, pos, orient)` (slot 39,
   through the vtable; §17.6).
4. If `propID = 0`: return 0.
5. `occ := propManager.AddLotProp(propID, &pos, orient)` (prop manager slot 26, `0x004A2670`). That calls
   `AddPropWithUsage(…, 2)`, which inserts the prop and returns its `cISC4Occupant` without a reference for the
   caller. If `occ` is null: return 0.
6. `occ.SetLotTag(obj's tag +0x24)` (occupant slot 28); return `occ`.

There is no draw. This is `CreateRandomLotProp` (§6) with the ID given rather than drawn.

**`GetLotConfigurationObjectOccupant(lot, objectIndex, &out)`** (slot 64, `0x006CB760`, pop 12, AL):
1. `out := null`. If `lot` is null: return false.
2. Take a fresh A-side `ilist<cISC4Occupant*>`; its head comes from the cached allocator service `0x00B207E4`.
3. `r := lot.GetBoundingRect()` (lot slot 14). Then `min := city.CellCornerToPosition(x1, z1)` and
   `max := city.CellCornerToPosition(x2 + 1, z2 + 1)` (city slot 96, x and z only).
4. If the lot has no configuration (lot slot 9): free the list and return false.
5. `tag :=` the tag (+0x24) of the configuration's object `objectIndex`. There is no range check.
6. `occupantManager.slot40(list, &(x1, z1), &(x2, z2), null filter, −1)` (`0x0049E250`, the lot's cell rectangle).
   If it returns false: free the list and return false.
7. For each occupant `o` in list order:
   - Keep `o` only if its position (occupant slot 8) satisfies `min.x ≤ x ≤ max.x` and `min.z ≤ z ≤ max.z`
     (float32 compares; NaN fails), and `o.GetLotTag(&t)` (occupant slot 27) returns true with `t = tag`.
   - A kept `o` replaces `out`: the previous `out` is released and `o` is AddRef'd.
   - Release `o`, whether or not it was kept.

   So the **last** match in list order wins.
8. Free the list. Return `out ≠ null`; the caller owns one reference to `out`.

There is no draw.

---

## 16. Conformance tests (must exist in `engine/conformance`)

Unless stated, tuning is the shipped exemplar and the file RNG is seeded explicitly.

1. **Desirability tolerance** (§5.1). Temporary generator seeds and outcomes:

   | Location (x, z) | Seed | First raw | r | Passes while counter ≤ | Fails at counter |
   |---|---|---|---|---|---|
   | (0, 0) | `0x00000000` | `0x00000000` | 0 | 2 | 3 |
   | (1, 0) | `0x00010000` | `0x41C64E6D` | 1 | 3 | 4 |
   | (17, 3) | `0x00110003` | `0x5E2BFA90` | 2 | 4 | 5 |
   | (10, 20) | `0x000A0014` | `0x91C433C0` | 3 | 5 | 6 |
   | (64, 37) | `0x00400025` | `0x719D1CE9` | 3 | 5 | 6 |
   | (255, 255) | `0x00FF00FF` | `0x84C9A31B` | 3 | 5 | 6 |

2. **Desirability threshold** (§5.1, shipped 0.3): cell value 29 → `0.29` low; 30 → `0.29999998` (`0x3E999999`) low;
   31 → `0.31` not low. Not low resets the counter to 0.
3. **Daily quota** (§4.1): `n = 100` → 4 lots per day; `n = 27` → 1; `n = 28` → 2; `n = 0` → no work, true.
4. **Wrap day**: 29 lots, `perDay = 2`. Days 1–14 update L1–L28 (two per day). Day 15 updates L29, returns false
   (state 3), and on re-delivery updates L1 and L2: three lots that day. Day 16 updates L3, L4.
5. **Power grace** (§5.2): an unpowered building lot (needs power) visited three times returns 1, 1, 0; the zot
   property is 4 after the first visit; abandonment reason 2 on the third (if no later test fails). Powering it
   resets the counter, removes the zot and returns 2.
6. **Travel** (§5.4): residence in state 3 with road and job access but trip length 0: visits return true, true,
   true (zot 7 appears), true, true, true, **false** (counter 7). The same lot without road access: true, true, false
   with zot 6 from the first visit and one `0x4C04DE69` data1 = 1 per visit.
7. **Zot priority** (§5.6): current zot 4; request 6 → still 4 (a `0x4C04DE69` data1 = 1 is posted); request 7 →
   still 4; current 7, request 5 → every `0x48E95539 = 7` prop is removed, zot := 5.
8. **Empty-lot zot pattern** (§5.6): zone 1 at (2, 2) → yes; (1, 2) → no; zone 8 at (4, 8) → yes; (4, 6) → no;
   zone 10 → no.
9. **Condition** (§5.5, stay-new 90, thresholds 0.45 / 0.5):
   - state 3, building wealth 3, occupant wealth 1, condition 0, days 10 → 3;
   - wealth equal, condition 0, days 100, occupancy 0.40 → 2;
   - wealth equal, condition 0, days 100, occupancy 0.47, no-power counter 0 → 0; counter 1 → 1;
   - building wealth 2, occupant wealth 1 (d = 1), condition 3, days 10, occupancy 0.48 → `b` is true → d = 2 → 3;
   - state 4: condition 0, days 0 → 1; condition 2, days 91 → 3; condition 4 → 4.
10. **Development cost** (§11.4), water below terrain:
    - 1×1 lot (vertices (0,0)…(1,1)), altitude 100 except (1, 0) = 104: slope 4.0, `s = 0.25`, curve value
      `4.499996`, cost **4**.
    - 2×3 lot (x 0–1, z 0–2), altitude 100 except vertex (2, 3) = 260: `d = 3.6055512 → 1.4422205`,
      slope ≈ 110.94, `s ≈ 6.934` (clamped to 3000), cost **18000**.
    - Any flat rectangle: 0.
11. **Rubble count** (§7.3): bounding box 11 × 11 → area 121, count `trunc(0.9453125) + 4 = 4` (corners only, not
    small, not large); 8 × 8 → small, no draws, no props; 16 × 16 → count 6 (two interior props), large.
12. **Seeded rubble** (§7.3), file RNG seed 12345, non-burnt 11 × 11 box: draws `Uniform(2)` ×4 → 0, 1, 0, 1 (raw
    `0x0C63D3DC`, `0xFED6A704`, `0x6E80D665`, `0xC0090DAA`). Props: corner 1 → type `0x1DCF0000`, orientation 3, at
    (min.x, min.y, max.z); corner 3 → `0x1DCF0000`, orientation 1, at (max.x, min.y, min.z) (before clamping).
    Corners 0 and 2 get nothing.
13. **Seeded completion content** (§6.5, §10.3), file RNG seed 12345. Configuration objects in array order: one
    texture object with IDs {T0, T1, T2} (T0 ≠ 0, no wealth / decay variants present), one prop object (bit 0 clear)
    whose expanded list is {P0, P1, P2, P3}, one flora object with valid flora types {F0, F1}. On the 2 → 3 message:
    draws `Uniform(3) = 0`, `Uniform(4) = 3`, `Uniform(2) = 0` → base texture T0, prop P3, flora F0, in that order.
14. **Coffee-bar draw first** (§5.8): seed 12345, cheat on, purpose 2, lot historical: one draw `Uniform(100) = 4`
    is consumed even though the lot is rejected.
15. **Night batch** (§8.3): shipped tuning (accumulator 100, batch 10), 25 buildings, night mode 2: tick
    data2 = 150000 → `ms = 150`, acc 250 > 200 → 10 buildings lit, acc 0. Ticks of 99999 (acc 99) then 101000
    (acc 200) → nothing; then 2000 (acc 202) → next 10. After the 25th building the next batch sets mode 0. With
    mode 0 the tick message returns false.
16. **Shuffle** (§8.2): deque of 4 buildings, C `rand()` state fresh (state 1, first outputs 41, 18467, 6334): swaps
    (1, 41 mod 2 = 1), (2, 18467 mod 3 = 2), (3, 6334 mod 4 = 2) → order [b0, b1, b2, b3] becomes [b0, b1, b3, b2].
17. **Save record** (§13): 64 × 64-cell city (cell width 16), empty deque, mode 0, unlit: 24 bytes
    `01 00 | 41 00 00 00 | 00 00 80 44 | 00 00 80 44 | 00 00 00 00 | 00 00 00 00 | 00 | 00`. A version-2 record
    is rejected after 2 bytes.
18. **Abandonment order** (§4.2): a lot failing desirability and water records reason 3; failing all four records
    reason 4; `SetState` is called once.
19. **Interface pops** (§11.1): slot 23 pops 4 and returns 0; slot 53 pops 8; slot 32 returns in EDX:EAX; slot 38
    returns the building interface without a reference.
20. **Construction timing** (§6.7.3, §6.7.4), shipped construction tuning (`segH` 3, `grow` 1.0, `remove` 2.0),
    building bbox min.y 100, terrain flat at 100 unless stated, `D` 30 unless stated:

    | max.y | Variation | startY / f | N | A | B | C | E |
    |---|---|---|---|---|---|---|---|
    | 110 | — | 100.0999985 (`0x42C83333`) / 3.3000004 (`0x40533335`) | 4 | 0 | 1 | 1 | 6 |
    | 110 | one corner altitude 99.0 | as above, ground 99 | 4 | 1 | 1 | 1 | 6 |
    | 110 | one corner altitude 99.5 (`startY − ground` = 0.5999985) | ground 99.5 | 4 | 1 | 1 | 1 | 6 |
    | 110 | `D` = 0 (→ 10) | | 4 | 0 | 1 | 1 | 1 |
    | 110 | `D` = 255 | | 4 | 0 | 1 | 1 | 62 |
    | 160 | — | f 19.9666672 (`0x419FBBBC`) | 20 | 0 | 1 | 1 | 1 |
    | 100.25 | — | f 0.0500005 | 1 | 0 | 1 | 0 (`round(0.5)` = 0) | 29 |
    | 103.125 | — | f 1.0083338 | 2 | 0 | 1 | 1 | 13 |
    | 103.0 | — | f 0.9666672 | 1 | 0 | 1 | 1 | 28 |

21. **Construction schedule** (§6.7.5), `T0` = 0, first SimNewDay delivered on day 1: the first row of test 20 posts
    `0x49E7B1E2` on days 1, 8, 15, 22, removes the scaffold on day 29 and calls `lot.SetState(3, 0, 0)` on day 30
    (day 29 if the first delivery is day 0). The other rows complete on: corner 99.0 → 30 (also from day 0); corner 99.5
    → 30; `D` = 0 → 10 (segments 1, 3, 5, 7, removal 9); `D` = 255 → 254; max.y 160 → 42; 100.25 → 32; 103.125 → 30;
    103.0 → 31. A SimNewDay whose date is below `nextDay` changes nothing; data1 is ignored.
22. **Animation-tick buckets** (§6.7.8): priority `0xC0` → bucket 0, fires on all 8 phases; `0x80` → bucket 1, phases
    0, 2, 4, 6; `0x40` → bucket 2, phases 1, 5; `0x3F` → bucket 3, phase 3. Clamp with defaults: delta 16700 µs →
    data2 16700, lot developer `ms` 16; 2 000 000 → 500000 (500 ms); 1000 → 2000 (2 ms).
23. **Construction lot texture** (§6.7.7 step 6), generator seeded 12345 (raws `0x0C63D3DC`, `0xFED6A704`,
    `0x6E80D665`, `0xC0090DAA`), 2 × 2 dry lot, `CreateLotTexture` alone: `Uniform(4)` = 0, 3, 1, 3 → textures
    `0xA990C100` (x1, z1), `0xA990C400` (x1, z2), `0xA990C200` (x2, z1), `0xA990C400` (x2, z2).
24. **Spark scatter** (§6.7.7), generator seeded 12345 (first `RandomDoubleUniform` values 0.9073938, 0.06438741,
    0.7574713, 0.3998830, 0.2344488, 0.08336106), C `rand()` fresh (41, 18467, 6334), box min (0, 100, 0) max
    (20, 110, 30), one spark: the side table becomes [0, 1, 3, 2], so side 0: x = `GF(0, 20)` = 11.5283508
    (`0x41387420`), z = 0, y = `GF(100, 110)` = 102.3923111 (`0x42CCC8DD`). Six `RandomDoubleUniform` and three
    `rand()` are consumed.
25. **Crane pause** (§6.7.6), generator seeded 12345: the first state 0 → 1 step sets duration `1000 + Uniform(1000)`
    = 1048 ms.
26. **StateChanged data4** (§3.2, `cSC4Lot::SetState`): a lot in state 1 with wealth byte 0 and no building type:
    `SetState(2, 0, 0)` → data1 1, data2 2, data4 0 (the recomputation sets 0). A lot in state 4 with wealth 1:
    `SetState(3, 3, 0)` → data4 3, lot wealth 3. A lot in state 3 with wealth 2: `SetState(4, 0, 4)` → data4 2 (the
    wealth before the call, whatever the recomputation gives), vacate reason 4; the reason never appears in the
    message. `SetState(3, …)` on a lot already in state 3 posts nothing.
27. **UpdateLotElements kind** (§6.4): a lot in state 3 without a foundation, with a configuration whose maximum
    slope before foundation is below the lot slope: kind 5 → no `AddLotFoundation`, `UpdateLotBaseTextureOccupant`
    gets false; kind 0x105 → foundation added (one `AddLotFoundation` call), texture call gets true; kind 0x100
    (low byte 0) → foundation added as well.
28. **Float-rectangle altitude** (§17.2, slot 16), terrain flat at 50 except as stated, no water: rectangle
    `(100, 200, 110, 230)` makes 26 `GetAltitude(x, z)` calls in the order of §17.2 (none through slot 15); with
    `withWater` each is followed by one `GetWaterLevel(x, z)`. `(0, 0, 64, 64)` makes 16 slot-15 reads (vertices
    1…4 × 1…4, x outer) and then 68 edge samples. `(−5, −5, 1e9, 20)` is clamped to `(0, 0, extentX − 0.1, 20)`.
    With every altitude 50 except `GetAltitude(110, 216) = 80`, highest = 80 and lowest = 50.
29. **Average and area levelling** (§17.4 worked example 2): `GetAverageAreaAltitude((0, 0, 1, 1), true)` =
    100.44444 (`0x42C8E38E`); `AttemptAreaLevelling(rect, 123.0, true, true, FLT_MAX, lot)` writes the 9 vertices
    with `SetAltitude(x, z, 100.44444)` (x outer), redisplays `(0, 0, 3, 3)` with reason 5 and returns 9; with
    `perVertex = false` one `SetAltitude((0, 0, 2, 2), 100.44444, 0, 5)`, returns 9. With vertex `(2, 2) = 260`:
    returns 0 and makes no terrain call. With `maxAreaSlope = 1000` the slope is not computed and the 260 case
    levels too. The second argument (123.0) changes nothing.
30. **Internal levelling** (§17.4 worked example 1): one `ModifyTerrainHeight(1, 1, 11, 11, 11, 11, 102.8125, lot,
    5, null)`, return value 1; `force = true` → four calls in the order (11, 11), (11, 12), (12, 11), (12, 12),
    return 4; a 1-cell-wide rectangle (`x2 = x1` or `z2 = z1`) → no call, return 0, while a 2-cell-wide one checks
    its one interior column (§17.4 step 2; LD2). With `lotAutoLevelInternalAltitudeDelta = 30` the first case
    makes no call. (Slot 55's own guard, `x2 − x1 > 1` and `z2 − z1 > 1`, is what skips slot 19 for 2-cell lots
    there; §17.5.4.)
31. **ModifyTerrainHeight per vertex** (§17.3): `h = 100`, vertex (5, 5) at 100.04 → skipped (|d| < 0.05); at
    100.05 → written; a water vertex → skipped; a vertex under an immovable building whose bounding box spans z 70…90 and has min.x =
    94.4 (`16·5 + 14.5 = 94.5 ≥ 94.4`) → skipped; min.x = 94.6 → written. A neighbouring lot with locked vertices in
    one of the four cells around the vertex → skipped; the same lock on `exempt` → written.
32. **Lot foundation** (§17.5.4 worked example 3): one `SetAltitude(x, z, 101.25)` per vertex 4…6 × 4…6 (9), then
    BDS slot 23 with `(0x890B7312, 2, (80, 101.25, 80), 32, 32, 0, 100)`, foundation size `(32, 1.25, 32)`, lot
    altitude 101.25, lot flags `(old & ~4) | 3`, then `AddRetainingWallOccupant(lot, 1)`. Facings 1, 2, 3 give tops
    103.25, 102, 100 (facing 3: `100 < 104`, walls added). With `useLotAltitude = true` and lot altitude 104: no
    corner read, top 104, no walls. A 3 × 3 lot with `level` calls slot 19 with target `top − 0.5` and
    `force = true` before slot 20.
33. **R8** (§17.5.4), file RNG seeded 12345 (first raw `0x0C63D3DC`): a primary foundation object with IDs
    {A, B, C} → one draw `Uniform(3) = 0` → type A, drawn before any terrain call; IDs empty and `+0x20 = T` → no
    draw, type T; no primary foundation object → no draw, type `0x890B7312`; type 0 → AL = 0 and no slot 55.
    `UpdateLotFoundation` on a lot with flags bit 0 and no foundation occupant draws once (through slot 54).
34. **Foundation lookup by position** (§17.5.1): a lot foundation whose box covers x ∈ [100, 132], z ∈ [100, 132]:
    `GetFoundationByPosition(2, 100.04, 120)` → null (the query line is at x = 99.99); `(2, 131.9, 99.96)` → found
    (line at (131.85, 100.01)), since the box query keeps an occupant whose box contains the line strictly (LD1).
    The result carries no reference (its count is unchanged by the call).
35. **Object positions** (§17.6 worked example 4): the four facings give the table's positions and orientations;
    `f = 5` gives `pos.x = pos.z = 0` and orientation `(1 + 5) mod 4 = 2`. The returned EAX equals the address of
    the `orient` argument. A lot with flag bit 0 and altitude 77 gives `pos.y = 77.5` for every object type.
36. **Boundary rules** (§17.7): `GetBoundaryLotTexture({2, 0, 1, 1})` → `(0x2700, 0, false)`; `{2, 1, 1, 0}` →
    `(0x2700, 0, true)`; `{1, 1, 1, 1}` (absent) → `(0x3000, 0, false)`; `{0, 0, 0, 0}` → `(0x3000, 0, false)`;
    `{0, 2, 0, 1}` → `(0x2900, 1, false)`.
37. **Empty-lot textures** (§17.7 worked example 5), zone colour `0xFFB23300`, alpha `0xAA`: the 2 × 1 lot passes two
    records, `00 27 00 00 03 07 00 00 FF B2 33 AA 03 FF` and `00 27 00 00 04 07 00 01 FF B2 33 AA 03 FF` (first 14
    bytes, little-endian texture, frame byte still `0xFF`), in that order, through one
    `SetTextureSpecification(records, 2, 0)`. Zone 15
    (or 0, 11, 12) → no record; every base-texture occupant on the lot is removed and shut down. A zone-8 1 × 1 lot
    → one record with texture `0x3000`, rotation 0. A cell outside the city is skipped.
38. **Interface returns** (§17): slot 19 returns the call count in EAX (not 0/1); slots 20 and 21 return
    `ModifyTerrainHeight`'s EAX; slot 18 reads an int32 rectangle; slot 42 has no defined return; slot 53 pops 8;
    slot 61 returns without any call when the lot's flags have neither bit 0 nor bit 1.
39. **Callers of slots 12–14** (§15.5): `DemolishRegion(true, R, 0, 0x303, …)` over a lot that is not
    water-compatible makes exactly one lot-developer call, `ExplodeLot(lot, true)`. Flags `0x1303` pass false.
    Flags `0x301` with a water-compatible configuration call `StartDemolishLot(lot)` and then `EndDemolishLot(lot)`,
    in that order.
40. **CreateLotProp** (§15.5): a configuration whose object 2 has tag `0x1234`, and `propID = 0x5000`: one
    `AddLotProp` with the position and orientation of slot 39, then `SetLotTag(0x1234)` on the result, and EAX is
    that occupant. `propID = 0` makes no prop-manager call and returns 0, but slot 39 is still called once.
41. **GetLotConfigurationObjectOccupant** (§15.5): two occupants on the lot carry the object's tag: the second in
    list order is returned, with one reference. An occupant with that tag whose x equals `max.x` is kept. One whose
    x is `max.x + 0.01` is skipped.
42. **IsAreaFlat** (§17.2, LD29), tuning 1.0, rectangle `(0, 0, 0, 0)` (vertices x, z = 0..1), dry:
    `(0,0) = 100.5`, `(1,0) = 100`, `(0,1) = 101.4`, `(1,1) = 100.9` → true (range 1.4); with `(1,0) = 99.4` → false;
    with `(1,0) = 99.5` → true (|99.5 − 100.5| = 1.0 is not above the tuning). Water level 101 at vertex (0,0)
    only, first case: the reference is 101 with either `withWater`, and the other vertices give 1.0, 0.4, 0.1 → true;
    with `withWater = false` vertex (0,0) itself is sampled dry (100.5, difference 0.5). Water level 102 there
    instead: `(1,0) = 100` differs by 2.0 → false for both values of `withWater`. Slot 27 on the first case
    returns 1.4.
43. **Base-texture occupant lookup** (§6.6 slot 41, LD24): two base-texture occupants overlap the lot's location
    cell, the first in bucket order belonging to another lot: slot 41 returns that first one (no lot check) and
    calls `SetUsage(2)` on it; `create = false` with no occupant at the cell returns false and writes `out = null`.
44. **DivideLot** (§6.7.11): a lot spanning x 0–48, z 0–32, a building box x 10–30, z 8–20, gap 2: `xa = 8`,
    `xb = 32`, `za = 6`, `zb = 22`; area 0 is x 8–32, z 0–6; area 1 x 32–48, z 6–22; area 4 x 0–8, z 0–6; area 6
    x 32–48, z 22–32; every area has y 0–1.
45. **Crane swing** (§6.7.12): an outer crane of reach 20 on side 0 at x 24 of a lot x 0–48 has the range
    `[π, 2π]`; at x 10 (`m = 10 < 20`) the range is `3π/2 ± asin(0.5)` = `[3π/2 − π/6, 3π/2 + π/6]`.
46. **RemoveRectangle** (§6.7.12): area x 0–10, z 0–10 and a cut x 4–6, z 7–10: the parts are 70 (below),
    40 (right), 0 (above) and 40 (left), so the area becomes x 0–10, z 0–7.
47. **Construction cleanup** (§6.7.14): after completion, the occupant's ground props, roof props, effects and crew
    automata are no longer in the prop and automata managers.

---

## 17. Geometry: altitudes, levelling, foundations, object positions, boundary textures *[compat]*

This section answers §12.2 Q4. It specifies slots 15–22, 39–40, 42–43 and 48–61 and the private helpers they use,
verified from the instruction bytes at the cited addresses (re-specs11, 2026-10-14).

**Conventions.**
- A **cell** `(x, z)` has the corner **vertices** `(x, z)`, `(x+1, z)`, `(x+1, z+1)`, `(x, z+1)`. A cell rectangle
  `(x1, z1, x2, z2)` is inclusive (lot slot 14 gives one), so its vertices run `x1 … x2+1` by `z1 … z2+1`.
- The vertex index is `x + z · vertsX` (`+0x40`, §1.3 step 5). No slot below checks it against the terrain size.
- `r24(·)` rounds to float32 (the x87 runs at 24-bit precision); *trunc* is `_ftol`, toward zero. "Strict, NaN no"
  means a comparison that is false when either side is NaN.
- City conversions: `PositionToCell` (city slot 95) floors `p / cellWidth`; `CellCornerToPosition` (slot 96) gives
  `r24(x · cellWidth)`; `CellCenterToPosition` (slot 97) gives `r24((x + 0.5) · cellWidth)`. Constants written
  here as `16`, `0.0625` or `14.5` are **literals** in the code, not the city's cell width (16 in every shipped city).
- **Random draws**: only slot 54 (R8, §10.2). Slot 43 hands records with frame byte `0xFF` to the base-texture
  occupant, which draws its **own** generator `0x00B23078` (lot-records.md §6), not the lot developer's.
- **Terrain writes** happen only inside `cISC4Demolition::ModifyTerrainHeight` (§17.3), reached from slots 19, 20
  and 21 (and from slot 55 through 19 and 20). No slot here calls a terrain setter itself.

### 17.1 The lot-configuration object record (0x34 bytes)

`cISC4LotConfiguration` slot 23 returns the array of records; slot 24 (`GetPrimaryBuildingConfigurationObject`) and
slot 26 (`GetPrimaryFoundationConfigurationObject`) return a pointer to one record, or null. The configuration
builds each record from its lot-object property `0x88EDC900 + i`, a list of u32 values `v0, v1, …`
(`ConvertPropertiesToObjectArray`, `0x00479D10`). The lot developer reads the record raw:

| Offset | Type | Value | Read by |
|---|---|---|---|
| +0x00 | u8 | object type `v0` (this class uses 1 prop, 2 base texture, 4 flora; the primary building object is the building) | §6, slot 39 |
| +0x01 | u8 | `v1` | — |
| +0x02 | u8 | orientation, low byte of `v2` | slot 39 |
| +0x03 | u8 | `(v2 >> 31) & 1`, the mirror flag | base-texture records (§6.6 "object byte 3") |
| +0x04, +0x08, +0x0C | float | offsets x, y, z: `v3`, `v4`, `v5` as signed 16.16 fixed point (`fild`, multiply by the double `2^-16`, store float) | slot 39 |
| +0x10, +0x14 | float | footprint max x, max z: `v8`, `v9` (16.16) | slot 39 |
| +0x18, +0x1C | float | footprint min x, min z: `v6`, `v7` (16.16) | slot 39 |
| +0x20 | u32 | `v10`: slot 54's foundation type when the ID list is empty; slot 58's building foundation type when non-zero | slots 54, 58 |
| +0x24 | u32 | lot tag: `v11`, or a generated serial number (`0x00B2166C`) for configurations whose objects carry no tag | `SetLotTag` (§6.5) |
| +0x28 | `vector<u32>` (begin, end, capacity) | the remaining values (`v12 …`, or `v11 …` when the tag is generated) | slots 43, 54; §6.5 |

### 17.2 Altitude helpers (slots 15–18, 22)

**`GetTerrainAltitudeByVertex(x, z, withWater)`** (slot 15, `0x006C4310`, pop 12, ST0): `i := x + z · vertsX`;
`a := r24(terrain.GetAltitude(i))` (`cISTETerrain` slot 12). With `withWater`: `w := terrain.GetWaterLevel(i)`
(slot 31) and the result is `w` if `w > a` (strict, NaN no), else `a`.

**`GetCornerAltitudes(rect, float out[4], withWater)`** (`0x006C4360`, private, pop 12): slot 15 at `(x1, z1)`,
`(x2+1, z1)`, `(x2+1, z2+1)`, `(x1, z2+1)` into `out[0..3]`. Corner `k` is the start of the side the lot faces when
its facing is `k` (§17.8).

**`GetLowestOrHighestAreaAltitude(SC4Rect<int32>, lowest, withWater)`** (slot 17, `0x006C5620`, pop 12, ST0):
`acc := lowest ? +FLT_MAX : −FLT_MAX`; for `x := x1 … x2+1` (outer), `z := z1 … z2+1` (inner): `v := slot 15(x, z,
withWater)`, and `acc := v` when `v < acc` (lowest) or `v > acc` (highest), strict, NaN no. Returns `acc`
(`±FLT_MAX` for an empty rectangle).

**`GetLowestOrHighestAreaAltitude(SC4Rect<float>, lowest, withWater)`** (slot 16, `0x006C56C0`, pop 12, ST0). The
rectangle is in metres, `(fx1, fz1, fx2, fz2)`, worked on a copy:
1. **Clamp**: `fx1 < 0 → 0`; `fz1 < 0 → 0`; `fx2 ≥ extentX` (`+0x44`) `→ r24(extentX − 0.1)`; `fz2 ≥ extentZ`
   (`+0x48`) `→ r24(extentZ − 0.1)`. NaN is never clamped.
2. **Interior vertices**: `vx` from `trunc(r24(r24(fx1 + 16) · 0.0625))` to `trunc(r24(fx2 · 0.0625))`, `vz` the
   same with `fz1`, `fz2`. When both ranges are non-empty, every `(vx, vz)` (vx outer) is sampled with slot 15
   (`withWater` passed through).
3. **Edges**, each a sweep `t := start, start + 4, start + 8, …` (step `r24(t + 4)`) while `t` is below the end,
   with the step that would reach or pass the end replaced by the end itself, so the end is always sampled. A
   sample at `(px, pz)` is `h := terrain.GetAltitude(px, pz)` (`cISTETerrain` slot 13, float position); with
   `withWater`, `h := terrain.GetWaterLevel(px, pz)` (slot 30) when that is larger (strict, NaN no). Order:
   a. if `fx1 < fx2`: `px` sweeps `fx1 → fx2` at `pz = fz1`;
   b. if `fz1 < fz2`: `pz` sweeps `fz1 → fz2` at `px = fx2`;
   c. if `fx1 < fx2`: `px` sweeps `fx1 → fx2` at `pz = fz2`;
   d. if `fz1 < fz2`: `pz` sweeps `fz1 → fz2` at `px = fx1`.
4. Every sample updates `acc` as in slot 17. Return `acc`.

No sweep needs a cap (LD9): after the clamp both ends are inside `[0, extent − 0.1]`, so an edge has at most
`extent / 4 + 2` samples (1,026 for a 4,096 m city), and a NaN end makes the `fx1 < fx2` / `fz1 < fz2` tests false.

Example: `(100, 200, 110, 230)` has no interior vertex (`vx` from `trunc(116 / 16) = 7` to `trunc(6.875) = 6`) and
26 edge samples: x = 100, 104, 108, 110 at z = 200; z = 200, 204, …, 228, 230 at x = 110 (9); x = 100 … 110 at
z = 230 (4); z = 200 … 230 at x = 100 (9). `(0, 0, 64, 64)` (in a city wider than 64 m) samples the 16 interior
vertices `(1..4, 1..4)` first, then 4 × 17 edge points (0, 4, …, 64).

**`GetAverageAreaAltitude(SC4Rect<int32>, withWater)`** (slot 18, `0x006C5B70`, pop 8, ST0). The SDK's
`SC4Rect<float>` is wrong: the code reads int32 cells. `sum := 0`, `n := 0`; for `x := x1 … x2+1` (outer),
`z := z1 … z2+1` (inner): `sum := r24(slot 15(x, z, withWater) + sum)`, `n += 1`. Returns `r24(sum / float(n))`
(an empty rectangle gives 0 / 0 = NaN).

**`GetOccupantCellCoverage(occ, SC4Rect<int32>&)`** (slot 22, `0x006C4590`, pop 8): `(min, max) :=
occ.GetBoundingBox()` (occupant slot 10); `min.x`, `min.z` += 0.05 and `max.x`, `max.z` −= 0.05 (r24);
`(x1, z1) := city.PositionToCell(min.x, min.z)`, `(x2, z2) := city.PositionToCell(max.x, max.z)` (slot 95). AL is
the second `PositionToCell`'s.
- **`city.PositionToCell(x, z, int* cx, int* cz)`** (city slot 95, `0x004599E0`, pop 16) returns nothing. Each
  coordinate is `floor(v · (1 / cellWidth))` (the reciprocal at `0x00B07F28`), done with the 1.5·2²³ rounding trick
  and a correction when `float(n + 1)` equals the product. EAX is left from `fnstsw ax` of the last compare, so AL is
  the low byte of the x87 status word: the sticky exception flags (precision 0x20, underflow 0x10, denormal 0x02,
  invalid 0x01). The values 0x32 / 0x33 seen in game are these flags (LD18, LD27).
- So slot 22 has no defined result. Its callers are inside this class, through `this`: `ChangeBuilding`
  (`0x006C52DB`), `GetOccupantSlope` (`0x006C5F64`) and `StartConstruction` (`0x006CC764`); none tests AL, each reads
  the rectangle only. No outside caller uses slot 22 (§15.1). A replacement may return anything.

**`IsAreaFlat(SC4Rect<int32>, withWater)`** (slot 25, `0x006C5C30`, pop 8, AL) (LD29). It is **not** a range test:
1. `ref := f32(slot 15(x1, z1, true))`: the first vertex, **always with water** (the literal 1 at `0x006C5C4B`),
   whatever `withWater` says; stored as float32 (`0x006C5C5D`).
2. For `x := x1 … x2+1` (outer), `z := z1 … z2+1` (inner): `v := slot 15(x, z, withWater)`; if `|v − ref| >
   kMaxCellAltitudeDeltaForFlatness` (`+0xD0`; extended-precision subtraction; strict, NaN no) → return false.
3. Return true.

So a rectangle is flat when every vertex lies within the tuning value of its first corner: the max − min range can
reach twice the tuning value (2.0 shipped) when the first corner sits in the middle of the range. With `withWater =
false` the reference still includes water above the first vertex. Slot 26 (`IsLotFlat`) applies it to the lot's
bounding rectangle (lot slot 14).

Example: dry vertices `(0,0) = 100.5`, `(1,0) = 100`, `(0,1) = 101.4`, `(1,1) = 100.9` (rectangle `(0, 0, 0, 0)`):
the range is 1.4, but every difference from 100.5 is at most 0.9, so slot 25 answers true; `(1,0) = 99.4` instead
gives |99.4 − 100.5| = 1.1 > 1.0 → false.

### 17.3 The terrain writer (`cISC4Demolition::ModifyTerrainHeight`)

`ModifyTerrainHeight(doIt, perVertex, x0, z0, x1, z1, float h, cISC4Lot* exempt, uint32 reason, int64* cost)` is
slot 9 of the demolition utility (`+0x20`, global `0x00B43D00`; `0x00467BB0`, pop 40, EAX = a count). Its owner
spec is demolition.md §4.7 (re-specs10 lane). What the lot developer relies on, re-checked from the bytes:

`Q := (max(x0 − 1, 0), max(z0 − 1, 0), min(x1, cellsX − 1), min(z1, cellsZ − 1))` (cells). The immovable occupants
in `Q` are listed first (occupant manager slot 40 with the demolition filter; at most one in area mode).

**Per-vertex mode** (`perVertex` true). For `x := x0 … x1` (outer), `z := z0 … z1` (inner), a vertex is skipped
when:
1. `terrain.IsWaterVertex(x, z)` (`cISTETerrain` slot 37); or
2. `|terrain.GetAltitudeAtVertex(x, z) − h| < 0.05` (slot 14; strict, NaN not skipped); or
3. a lot in the cells `(x−1, z−1) … (x, z)` (lot manager slot 7), other than `exempt`, reports
   `GetVerticesAreLocked()` (lot slot 68); or
4. an immovable occupant's bounding box (occupant slot 10) meets the closed square `[16x − 14.5, 16x + 14.5] ×
   [16z − 14.5, 16z + 14.5]` (x and z only).

Every other vertex is counted and, when `doIt`, written: **`terrain.SetAltitude(x, z, h)`** (`cISTETerrain`
slot 51, pop 12). After the loop, if the count is > 0: **`terrain.RedisplayTerrain(1, 1, (Q.x0, Q.z0, Q.x1 + 1,
Q.z1 + 1), reason)`** (slot 68), also in a dry run. Returns the count.

**Area mode** (`perVertex` false). If an immovable occupant is in `Q`: return 0. `n :=` the number of vertices in
`[x0, x1] × [z0, z1]` with `|GetAltitudeAtVertex − h| > 0.05` (strict, NaN no; water vertices count). `n = 0` →
return 0. If no lot in `Q` other than `exempt` has locked vertices, and `doIt`: **`terrain.SetAltitude(R, h, 0,
reason)`** (slot 49, pop 16) with the vertex rectangle `R = (max(x0, 0), max(z0, 0), min(x1, cellsX), min(z1,
cellsZ))`, then `RedisplayTerrain(1, 1, R, reason)`. Returns `n`, **also when a locked lot stopped the write**
*[quirk]*.

The lot developer always passes `doIt = 1`, `reason = 5`, `cost = null`, and `exempt =` the lot being built, so the
vertex lock that StartConstruction sets on that lot (§6.1) never blocks its own levelling. `RedisplayTerrain` posts
`0x278E6003` with `data4 = 5` (landfill.md §4.5.3).

*Correction to demolition.md §4.7*: per-vertex mode is **not** the only mode in shipped use. StartConstruction
passes the configuration's water compatibility as slot 21's `perVertex` (§17.4), so every lot whose configuration
is not water-compatible is levelled in area mode.

### 17.4 Levelling (slots 19, 20, 21)

**`LevelAreaInternalVertices(rect, float target, bool skipWater, bool force, lot)`** (slot 19, `0x006C43D0`,
pop 20). Returns **EAX = the number of `ModifyTerrainHeight` calls**, not a bool (as a bool, AL is the count's low
byte, so 256 calls read as false *[quirk]*; no caller tests it).
1. If `target < 0` (strict, NaN no): `target := r24(GetAverageAreaAltitude(rect, true))` (slot 18).
2. For `x := x1 + 1 … x2` (outer), `z := z1 + 1 … z2` (inner) — the **interior** vertices only; there are none when
   the rectangle is 1 cell wide in either direction:
   - if `skipWater` and `terrain.IsWaterVertex(x, z)` (slot 37): next vertex;
   - `a := GetTerrainAltitudeByVertex(x, z, true)` (slot 15);
   - if `force`, or `r24(a − target) > lotAutoLevelInternalAltitudeDelta` (`+0xE4`, tuning `0xE900EA29`, shipped
     20; strict, NaN no): `ModifyTerrainHeight(1, 1, x, z, x, z, target, lot, 5, null)` and count += 1.
3. Return the count.

Without `force` only vertices **above** the target by more than the delta are lowered; vertices below it are
never raised. Each call writes at most the one vertex `(x, z) := target` (per-vertex rules of §17.3) and redisplays
cells `(x−1, z−1) … (x, z)` when it wrote.

**`LevelBorderingVertices(rect, float h, bool unused, lot)`** (slot 20, `0x006C44B0`, pop 16): returns the EAX of
`ModifyTerrainHeight(1, 1, x1, z1, x2 + 1, z2 + 1, h, lot, 5, null)`. The third argument is never read. Despite the
name, the range is **every vertex** of the rectangle, border and interior.

**`AttemptAreaLevelling(rect, float unused, bool withWater, bool perVertex, float maxAreaSlope, lot)`** (slot 21,
`0x006C44F0`, pop 24; EAX = 0 or `ModifyTerrainHeight`'s count; same reading as landfill.md §4.5.3):
1. `avg := GetAverageAreaAltitude(rect, withWater)` (slot 18).
2. `m := maxAreaSlope`; if `m` is `FLT_MAX` or NaN: `m := maxAreaSlopeForLevelling` (`+0xE8`, `0xE900EA2A`,
   shipped 40).
3. If `m < 1000` (strict, NaN no) and `GetAreaSlope(rect, withWater) > m` (slot 29, §11.4; strict, NaN no):
   return 0, nothing written.
4. Return `ModifyTerrainHeight(1, perVertex, x1, z1, x2 + 1, z2 + 1, avg, lot, 5, null)`.

The second argument is never read. The SDK and §11.1 called the fourth argument `water`: it is the
`ModifyTerrainHeight` mode.

**Callers** (all pass the lot being built as `lot`):

| Caller | Call | Mode reached |
|---|---|---|
| StartConstruction (§6.1), internal levelling | slot 19 (lot rect, −1.0, water, false, lot) | per-vertex, one vertex per call |
| StartConstruction, lot slope | slot 21 (lot rect, cfg max slope, true, water, maxAreaSlopeForLevelling, lot) | per-vertex if the configuration is water-compatible, else **area** |
| StartConstruction, building slope | slot 21 (slot 22 coverage of the building, maxSlopeForFlatness, true, water, maxAreaSlopeForLevelling, lot) | same |
| `AddLotFoundation` (slot 55) with `level` | slot 19 (lot rect, `r24(top − 0.5)`, true, true, lot) if `x2 − x1 > 1` and `z2 − z1 > 1`; then slot 20 (lot rect, top, true, lot) | per-vertex |
| landfill `LevelRect` (landfill.md §4.5.3) | slot 21 (rect, 0.0, true, true, `FLT_MAX`, null) | per-vertex |

**Worked example 1 (slot 19 from StartConstruction).** Lot `(10, 10, 12, 12)`, dry, vertices flat at 100 except
`(11, 11) = 130` and `(12, 12) = 115`; no other lot or occupant nearby. `target := 1645 / 16 = 102.8125`. Interior
vertices: `(11, 11)` +27.1875, `(11, 12)` −2.8125, `(12, 11)` −2.8125, `(12, 12)` +12.1875. One call,
`ModifyTerrainHeight(1, 1, 11, 11, 11, 11, 102.8125, lot, 5, null)`: vertex `(11, 11)` becomes 102.8125 (terrain
slot 51) and cells `(10, 10, 12, 12)` are redisplayed. Returns 1. With `force` (from slot 55) the four interior
vertices are all passed, and all four are written because each differs from the target by at least 0.05.

**Worked example 2 (slot 21).** Cells `(0, 0, 1, 1)`, vertices 0…2, dry, flat at 100 except `(2, 2) = 104`;
`maxAreaSlope = FLT_MAX` → 40. `avg = 904 / 9 = 100.44444` (`0x42C8E38E`). Slope: lowest `(0, 0)`, highest
`(2, 2)`, `d = 2.828427`, slope `4 / d = 1.4142135` ≤ 40. Per-vertex: 9 `SetAltitude(x, z, 100.44444)` calls (x
outer, z inner) and `RedisplayTerrain(1, 1, (0, 0, 3, 3), 5)`; returns 9. Area mode: `SetAltitude((0, 0, 2, 2),
100.44444, 0, 5)` and `RedisplayTerrain(1, 1, (0, 0, 2, 2), 5)`; returns 9. With `(2, 2) = 260` the slope is
`160 / 2.828427 = 56.568542` > 40: returns 0 and writes nothing.

### 17.5 Foundations (slots 48–61)

A foundation is a `cSC4FoundationOccupant` (0x5C bytes), occupant type `0x68FD0C69`, interface
`cISC4FoundationOccupant` (IID `0xC8FCE101`, vtable `0x00AA29A0`):

| Slot | Method | Pop | Return | Address |
|---|---|---|---|---|
| 3 | `AsOccupant()` (this + 4; null-safe) | 0 | EAX | `0x005FD7E0` |
| 4 | `GetFoundationType()` (u32 +0x38) | 0 | EAX | `0x0080BC00` (shared getter) |
| 5 | `SetFoundationType(u32)` | 4 | — | `0x006CE240` (shared setter) |
| 6 / 7 | get / set the float at +0x3C (property `0x88FCC3E3`) | 0 / 4 | ST0 / — | `0x005D4AC0` / `0x006CE230` |
| 8 | `SetSize(const float[3])` (+0x4C … +0x54) | 4 | AL = 1 | `0x005D6400` |
| 9 / 10 | `GetKind()` (sign-extended byte +0x58) / `SetKind(int8)` | 0 / 4 | EAX / — | `0x005D6140` / `0x005D6150` |
| 11 / 12 | `GetOrientation()` (byte +0x59) / `SetOrientation(int32)` clamped to 0…3 | 0 / 4 | EAX / AL = 1 | `0x005D6190` / `0x005D61A0` |

**Kinds**: 1 building, 2 lot, 3 prop, 4 other. **Lot flags** bits 0–1 (lot slots 12 / 13) mean "has a lot
foundation"; bit 0 also makes slot 39 use the lot's altitude.

#### 17.5.1 Lookup

- **Slot 49** `GetFoundationByStandardCityCell(kind, x, z)` (`0x006C9480`, pop 12, EAX):
  `occupantManager.GetOccupantsByStandardCityCells(list, &(x, z), &(x, z), 0x68FD0C69, −1)` (slot 41, A list); the
  first element in list order that answers `QueryInterface(0xC8FCE101)` and whose `GetKind()` (slot 9) is `kind`.
  It is returned **without a reference** (the QueryInterface reference and the list's references are released).
  Null when none.
- **Slot 50** `GetFoundationByPosition(kind, float x, float z)` (`0x006C95E0`, pop 12, EAX): takes and AddRefs the
  occupant manager `0x00B43D0C` into `+0x28` if that is null. Query: occupant manager slot 37 (box query with an
  occupant-type filter; it forwards to slot 36) with `(list, min, max, 0x68FD0C69, −1)`, `min = (x − 0.05, −FLT_MAX,
  z + 0.05)`, `max = (x − 0.05, +FLT_MAX, z + 0.05)`: a **vertical line** at `(x − 0.05, z + 0.05)`, not a 0.1 m
  square *[quirk]*. Same match rule and no reference. The box query's overlap test is strict on every axis
  (flora.md §6.4.2; `0x0049D8B0` and `0x004A08CC`–`0x004A0938`), so the line finds a foundation only when
  `x − 0.05` and `z + 0.05` lie **strictly inside** its box (a line on the box's edge misses it) and the foundation
  is listed in the 64 m index cell of that point, which its box (shrunk by 0.1 m when wider than 1 m) decides (LD1).
- **Slot 48** `GetFoundationByOccupant(occ)` (`0x006C3E10`, pop 4): `p := occ.GetPosition()` (occupant slot 8);
  `kind := 1` if the occupant type is `0x278128A0` (building), 3 if `0xA823821E` (prop), else 4; return slot 50(kind,
  p.x, p.z).
- **Slot 51** `GetLotFoundation(lot)` (`0x006C3E70`, pop 4): `(x, z) := lot.GetLocation()` (lot slot 21); slot 49(2,
  x, z).

#### 17.5.2 Removal

- **Slot 52** `RemoveFoundation(kind, float x, float z)` (`0x006C54E0`, pop 12, AL): `f := slot 50(kind, x, z)`;
  if found: `o := f.AsOccupant()`, AddRef, `occupantManager.RemoveOccupant(o, 1, 0)` (slot 21: shut down, data4 0),
  Release; AL = 1, else 0. If `kind = 2`: `L := lotManager.GetLot(trunc(x · 0.0625), trunc(z · 0.0625), false)`
  (slot 6) and, if found, `L.SetFlags(L.GetFlags() & ~3)`, **whether or not a foundation was found**.
- **Slot 53** `RemoveLotFoundation(int32 x, int32 z)` (`0x006C3EB0`, pop 8, AL): slot 49(2, x, z), removed as in
  slot 52; then `GetLot(x, z, false)` and its flags `& ~3`.
- **`RemoveLotFoundation(lot)`** (`0x006C3F30`, private, pop 4): `(x, z) := lot.GetLocation()`; slot 53(x, z); then
  `lot.SetFlags(lot.GetFlags() & ~3)` again; AL of slot 53.
- **Slot 60** `RemoveBuildingFoundation(occ)` (`0x006C4150`, pop 4): slot 52(1, position x, position z).

#### 17.5.3 Creating a foundation (BDS slot 23)

`cISC4BuildingDevelopmentSimulator::AddNewFoundationOccupant(type, kind, float* pos, float dx, float dz, int32
orient, float bottom)` (`0x00695AF0`, pop 28, AL):
1. `f :=` a new foundation occupant (`0x005D6220` / constructor `0x005D6440`), AddRef; `f.SetFoundationType(type)`.
2. `LoadFoundationProperties(f, type)` (BDS slot 26, `0x006982B0`): looks the type up in the BDS foundation table
   (`0x00B485BC`), loads its exemplar as the occupant's default exemplar (QueryInterface `0x0AC2B5F7`) and stores
   the float property `0x88FCC3E3` with `f` slot 7. On failure: release `f`, return 0.
3. `hx := dx · 0.5`, `hz := dz · 0.5` (double constant). World rectangle `(r24(pos.x − hx) + 0.5, r24(pos.z − hz) +
   0.5, r24(pos.x + hx) − 0.5, r24(pos.z + hz) − 0.5)`; cells `C` := each value × 0.0625, truncated (`0x00695610`),
   with `x1, z1 ≥ 0` and `x2 ≤ cellsX − 1`, `z2 ≤ cellsZ − 1` (BDS `+0x40`, `+0x44`).
4. If `pos.y < 0`: `pos.y := lotDeveloper.slot 17(C, highest, true)`, **written into the caller's array**.
5. `bot := bottom`, or `slot 17(C, lowest, true)` when `bottom < 0` or NaN.
6. `depth := r24(pos.y − bot)`, raised to 0.5 when below 0.5 (NaN kept).
7. `o := f`'s occupant: `o.SetPosition(pos)` (occupant slot 9), `f.SetSize({dx, depth, dz})`, `f.SetKind(kind)`,
   `f.SetOrientation(orient)`, `o.Init()` (occupant slot 3); BDS slot 24 (`0x00695740`) inserts it:
   `occupantManager.InsertOccupant(o, 0)` (slot 20). Release `f`; AL of the insertion.

The foundation's position is its **top** centre (`pos.y`), and its height is `depth`.

#### 17.5.4 Lot foundations (slots 54, 55, 61)

**Slot 54** `AddLotFoundation(lot, cfg, useLotAltitude, level, walls)` (`0x006C5460`, pop 20, AL):
1. Null `lot` or `cfg` → 0.
2. `type := kDefaultLotFoundationType` (`+0xD8`, shipped `0x890B7312`). `obj := cfg.GetPrimaryFoundation…()` (cfg
   slot 26). If `obj`: `n :=` its ID count; if `n > 0`: **R8** `type := ids[Uniform(n)]` (file RNG, the call at
   `0x006C549A`); else `type := obj.+0x20`.
3. `type = 0` → 0. Otherwise return slot 55(lot, type, useLotAltitude, level, walls).

R8 is drawn **before** any terrain or occupant change of the call.

**Slot 55** `AddLotFoundation(lot, type, useLotAltitude, level, walls)` (`0x006C8AB0`, pop 20, AL):
1. `c := lot.GetCenter()` (lot slot 15: `(8·(x1+x2+1), altitude, 8·(z1+z2+1))`); `r := lot.GetBoundingRect()`;
   `f := lot.GetFacing()`.
2. `dx := float(16 · (x2 − x1 + 1))`, `dz := float(16 · (z2 − z1 + 1))`.
3. `bottom := GetLotFoundationBottomAltitude(lot)` (below); `hi := slot 17(r, highest, true)`.
4. `top := useLotAltitude ? lot.GetAltitude() : PickLotFoundationAltitude(lot, corners)` with `corners :=
   GetCornerAltitudes(r, true)` (computed only on that branch). `c.y := top`.
5. If `level`: if `x2 − x1 > 1` and `z2 − z1 > 1`: slot 19(r, `r24(top − 0.5)`, true, true, lot); then slot 20(r,
   top, true, lot). The terrain is written here (§17.4), **after** `bottom` and `hi` were taken.
6. `ok := BDS.AddNewFoundationOccupant(type, 2, &c, dx, dz, f, bottom)` (§17.5.3; may replace `c.y` when it is
   negative).
7. `lot.SetAltitude(c.y)` (lot slot 17); `lot.SetFlags((lot.GetFlags() & ~4) | 3)`.
8. If `walls` and `c.y < hi` (strict, NaN no): `cSC4LotRetainingWallOccupant::AddRetainingWallOccupant(lot, 1)`
   (`0x005E3070`), `UpdateSurroundingLotFoundations(lot)`, `cSC4LotRetainingWallOccupant::
   UpdateSurroundingRetainingWalls(lot)` (`0x005E3960`), in that order.
9. Return `ok`.

**`PickLotFoundationAltitude(lot, bottom, hi, corners)`** (`0x006C5360`, private, pop 16; `bottom` and `hi` are
unused):
- If the lot's configuration exists and `IsCompatibleWithWater()`: for each cell of the lot rectangle (x outer, z
  inner), the first one with `terrain.CellHasWater(x, z)` (`cISTETerrain` slot 34) gives `(px, pz) :=
  city.CellCenterToPosition(x, z)` (slot 97) and the result `terrain.GetWaterLevel(px, pz) + 0.5` (slot 30).
- Otherwise (or with no water cell): `a := corners[f]`, `b := corners[(f + 1) mod 4]` — the two corners of the
  side the lot faces — and the result is `r24(min(a, b) + r24(r24(max − min) · 0.5))`.
- `f` is not masked (`0x006C53E6`): `a` indexes the 4-float array with `f` as is and `b` with C's signed `(f + 1) %
  4`, so a facing outside 0-3 reads outside the array (stack contents). Lot facings are 0-3; taking `f mod 4` is a
  safe *[choice]* (LD5).

**`GetLotFoundationBottomAltitude(lot)`** (`0x006C7630`, private, pop 4, ST0): `b := slot 17(lot rect, lowest,
true)`; for each lot from `lotManager.GetLotsSurroundingLot(lot, list, 0xF)` (slot 9; B list) whose flags have
bit 0 or 1: `b := n.GetAltitude()` when that is lower (strict, NaN no).

**`UpdateSurroundingLotFoundations(lot)`** (`0x006C7720`, private): for each lot from `GetLotsSurroundingLot(lot,
list, 0xF)` with flags bit 0 or 1, in list order: slot 61 on it.

**Slot 61** `UpdateLotFoundation(lot)` (`0x006C3F80`, pop 4):
- If `lot.GetFlags() & 3 = 0`: return (AL = the flags' low byte; no defined value).
- `F := GetLotFoundation(lot)` (slot 51). If `F`: `t := F.GetFoundationType()`; `RemoveLotFoundation(lot)`
  (`0x006C3F30`); return slot 55(lot, t, true, false, false).
- Else: return slot 54(lot, lot.GetLotConfiguration(), true, false, false) (**R8** possible).

So a refresh keeps the lot's altitude as the top, never levels and never adds walls.

**Worked example 3 (slot 55).** Lot `(4, 4, 5, 5)`, facing 0, configuration not water-compatible, no lot around
it, vertex altitudes (rows z = 4, 5, 6; columns x = 4, 5, 6): `100 101 102.5 / 100 101 103 / 100 101 104`; call
slot 54(lot, cfg, false, true, true) with no primary foundation object (type `0x890B7312`, no draw):
- `c = (80, ·, 80)`, `dx = dz = 32`, `bottom = 100`, `hi = 104`, corners `[100, 102.5, 104, 100]`;
- `top = 100 + 2.5 · 0.5 = 101.25` (facing 1 → 103.25, facing 2 → 102, facing 3 → 100);
- the lot is 2 cells wide, so no slot 19; slot 20 → `ModifyTerrainHeight(1, 1, 4, 4, 6, 6, 101.25, lot, 5, null)`:
  all 9 vertices differ by ≥ 0.05 and are set to 101.25;
- BDS slot 23(`0x890B7312`, 2, &(80, 101.25, 80), 32, 32, 0, 100): world rectangle (64.5, 64.5, 95.5, 95.5), cells
  `(4, 4, 5, 5)`, size `(32, 1.25, 32)`;
- lot altitude 101.25, flags `| 3`; `101.25 < 104`, so the retaining walls are added.

#### 17.5.5 Building foundations (slots 56–59)

- **Slot 56** `IsLotObjectFoundationRequired(occ)` (`0x006C3FF0`, pop 4, EAX 0/1): `GetOccupantSlope(occ, true)`
  (slot 31) `> kMaxSlopeForFlatness` (`+0xD4`; strict, NaN no).
- **Slot 57** `IsLotObjectFoundationRequired(float* min, float* max)` (`0x006C5590`, pop 8, EAX 0/1): `(a, b) :=
  PositionToCell(min.x, min.z)`, `(c, d) := PositionToCell(max.x, max.z)`; `GetAreaSlope((a, b, c + 1, d + 1),
  true) > +0xD4`. The rectangle is one cell larger than the box's cells in x and z *[quirk]*.
- **Slot 58** `AddBuildingFoundation(b, cfg)` (`0x006C40E0`, pop 8, AL): `type := +0xDC` (property `0x6916516B`,
  code default `0x890B7312`); `obj := cfg.GetPrimaryBuildingConfigurationObject()` (cfg slot 24; `cfg` is not
  null-checked). If `obj` and `obj.+0x20 ≠ 0`: `type := obj.+0x20`; otherwise the building's u32 property
  `0x88FCD877` replaces `type` when present. `type = 0` → 0; else slot 59(b, type). A null `cfg` is called through
  its null vtable (`0x006C40F5`) and faults; treating it as "no primary object" is a safe *[choice]* (LD7).
- **Slot 59** `AddBuildingFoundation(b, type)` (`0x006C4020`, pop 8, AL): `pos := b.AsOccupant().GetPosition()`;
  `(min, max) :=` its bounding box; `o := b.GetOrientation()` (building slot 14, byte +0x54). If `type = 0`: the
  property `0x88FCD877`, and `+0xDC` if that is 0 or absent. Return `BDS.AddNewFoundationOccupant(type, 1, &pos,
  r24(max.x − min.x), r24(max.z − min.z), o, −1.0)`: the top is the building's position y, the bottom the lowest
  vertex under it (§17.5.3 step 5).

### 17.6 Object positions (slots 39, 40)

**`GetPositionForLotConfigurationObject(lot, cfg, obj, float* pos, int32& orient)`** (slot 39, `0x006C4EF0`,
pop 20). Returns **EAX = `&orient`** (the SDK's bool is wrong; AL is the low byte of a stack address and can be 0
*[quirk]*; no caller tests it).
1. `r := lot.GetBoundingRect()`; `(c0x, c0z) := city.CellCornerToPosition(x1, z1)`; `(c1x, c1z) :=
   CellCornerToPosition(x2 + 1, z2 + 1)`; `f := lot.GetFacing()` (lot slot 23).
2. `(ox, oy, oz) := obj` +0x04, +0x08, +0x0C. If the object type is 2 (base texture): `ox := (trunc(r24(ox ·
   0.0625)) + 0.5) · 16` and the same for `oz` (the centre of the lot-space cell; truncation toward zero).
3. With `mnX, mnZ = obj +0x18, +0x1C` and `mxX, mxZ = obj +0x10, +0x14`, each value one r24 add or subtract:

   | f | pos.x | pos.z | footprint `R = (x1, z1, x2, z2)` |
   |---|---|---|---|
   | 0 | `c1x − ox` | `c1z − oz` | `(c1x − mxX, c1z − mxZ, c1x − mnX, c1z − mnZ)` |
   | 1 | `c0x + oz` | `c1z − ox` | `(c0x + mnZ, c1z − mxX, c0x + mxZ, c1z − mnX)` |
   | 2 | `c0x + ox` | `c0z + oz` | `(c0x + mnX, c0z + mnZ, c0x + mxX, c0z + mxZ)` |
   | 3 | `c1x − oz` | `c0z + ox` | `(c1x − mxZ, c0z + mnX, c1x − mnZ, c0z + mxX)` |
   | other | 0 | 0 | `(0, 1, 0, 1)` |

4. Ground `y`:
   - if `lot.GetFlags() & 1`: `y := lot.GetAltitude()` (lot slot 16);
   - else if `F := GetLotFoundation(lot)` (slot 51): `y :=` the y of `F.AsOccupant().GetPosition()`;
   - else, with `w := cfg ? cfg.IsCompatibleWithWater() : false`: object types 1, 2 and 4: `y :=
     terrain.GetAltitude(pos.x, pos.z)` (`cISTETerrain` slot 13; water ignored); any other type (0, 3, ≥ 5):
     `y := GetLowestOrHighestAreaAltitude(R, highest, w)` (slot 16).
5. `pos.y := r24(oy + y)`; `orient := (obj byte +0x02 + f) mod 4`.

**Slot 40** `GetPositionForBuilding(lot, float* pos, int32& orient)` (`0x006C3CF0`, pop 12): `cfg :=
lot.GetLotConfiguration()`; `obj := cfg.GetPrimaryBuildingConfigurationObject()`; if either is null, EAX = 0 and
nothing is written; otherwise slot 39(lot, cfg, obj, pos, orient) and its EAX.

**Worked example 4.** Lot `(10, 20, 11, 21)`, cell width 16: `c0 = (160, 320)`, `c1 = (192, 352)`. A prop
(type 1) at `(8, 0.5, 4)`, orientation byte 1, footprint min `(6, 2)` max `(10, 6)`; no flag bit 0, no foundation,
terrain flat at 50:

| f | pos | orient |
|---|---|---|
| 0 | (184, 50.5, 348) | 1 |
| 1 | (164, 50.5, 344) | 2 |
| 2 | (168, 50.5, 324) | 3 |
| 3 | (188, 50.5, 328) | 0 |

The same object as type 0 with `f = 2` has `R = (166, 322, 170, 326)`: slot 16 samples no interior vertex and the
edge points x = 166, 170 / z = 322, 326, so `y = 50`. A base texture (type 2) at `ox = 20.5`, `oz = 7.25` with
`f = 2` snaps to `(24, 8)` and lands at `(184, ·, 328)`, cell `(11, 20)` in §6.6.

### 17.7 Boundary textures (slots 42, 43)

**The boundary map** (`+0xFC`, a `std::map<int32, rule>`, filled by `InitBoundaryRules` `0x006CA160` at `Init`,
§1.3 step 11). A **side code** describes one cell: `s0` is the side towards `z − 1`, `s1` towards `x + 1`, `s2`
towards `z + 1`, `s3` towards `x − 1`, each 0 (the neighbour cell is the same lot), 1 (a boundary) or 2 (the
boundary the lot faces). `key := s0·0x1000 + s1·0x100 + s2·0x10 + s3`. Each rule is a texture instance, a rotation
and a flip. The 47 rules, by texture (key: rotation; "f" = flip 1, otherwise 0):

| Texture | Rules |
|---|---|
| `0x00003000` | `0x0000`: 0 |
| `0x00002000` | `0x1000`: 0, `0x0100`: 1, `0x0010`: 2, `0x0001`: 3 |
| `0x00002600` | `0x2000`: 0, `0x0200`: 1, `0x0020`: 2, `0x0002`: 3 |
| `0x00002100` | `0x1001`: 0, `0x1100`: 1, `0x0110`: 2, `0x0011`: 3 |
| `0x00002400` | `0x2001`: 0, `0x1200`: 1, `0x0120`: 2, `0x0012`: 3, `0x2100`: 0 f, `0x0210`: 1 f, `0x0021`: 2 f, `0x1002`: 3 f |
| `0x00002500` | `0x1010`: 0, `0x0101`: 1 |
| `0x00002900` | `0x2010`: 0, `0x0201`: 1, `0x1020`: 2, `0x0102`: 3 |
| `0x00002200` | `0x1101`: 0, `0x1110`: 1, `0x0111`: 2, `0x1011`: 3 |
| `0x00002700` | `0x2011`: 0, `0x1201`: 1, `0x1120`: 2, `0x0112`: 3, `0x2110`: 0 f, `0x0211`: 1 f, `0x1021`: 2 f, `0x1102`: 3 f |
| `0x00002800` | `0x2101`: 0, `0x1210`: 1, `0x0121`: 2, `0x1012`: 3 |
| `0x00002300` | `0x2111`: 0, `0x1211`: 1, `0x1121`: 2, `0x1112`: 3 |

Every code slot 43 can produce (one side at most is 2, the facing side is 0 or 2) is in the table.

**Slot 42** `GetBoundaryLotTexture(const int32 sides[4], uint32& texture, uint8& rotation, bool& flip)`
(`0x006C6CF0`, pop 16): `key := ((s0 · 16 + s1) · 16 + s2) · 16 + s3` (int32, no masking); look it up; if absent,
look up key 0 instead; write the rule's three fields. There is no defined return: EAX is the map node, with AL
replaced by the flip byte. With an empty map (before `Init`) the second lookup also returns the end node
(`0x006C6D2F` compares with the head), and the three fields are read from the head node's value area, which is
never initialised: the outputs are indeterminate, not left unwritten (LD4). After `Init` key 0 always exists
(texture `0x3000`).

**Slot 43** `SetEmptyLotBaseTexture(lot)` (`0x006CC970`, pop 4, no return):
1. `zone := lot.GetZoneType()` (lot slot 31). If `zone` is 0, 11, 12 or 15 (none, airport, seaport, plopped; byte
   table `0x006CCE1C`): **`RemoveLotBaseTextures(lot)`** (`0x006CAAF0`) and return. That lists the base-texture
   occupants (type `0x497F6D9D`) on the lot rectangle (occupant manager slot 41, A list, flags −1) and for each:
   if it is inserted (occupant slot 32 with 1), `occupantManager.RemoveOccupant(o, 0, 0)` (slot 21); then
   `o.Shutdown()` (occupant slot 4) and Release. A null lot or occupant manager does nothing.
2. Every other zone (1–10, 13, 14 and any value above 15):
   - `colour := zoneManager.GetZoneDragColor(zone)` (`0x00B43D14` slot 15), `alpha :=
     zoneManager.GetZoneDisplayAlpha()` (slot 16, AL);
   - `maxZ := city slot 94 − 1`, `maxX := city slot 93 − 1`; `r := lot.GetBoundingRect()`;
   - `boundaries := true`, except for zones 7, 8 and 9 when `lot.GetSize()` (lot slot 22) is 1 × 1;
   - for `x := x1 … x2` (outer), `z := z1 … z2` (inner), skipping cells with `x ≥ cellsX` (`+0x50`) or
     `z ≥ cellsZ` (`+0x4C`):
     - if `boundaries`, for the neighbours in the order `(x, z − 1)` → s0, `(x, z + 1)` → s2, `(x − 1, z)` → s3,
       `(x + 1, z)` → s1: `n := lotManager.GetLot(nx, nz, false)` (slot 6) when `0 ≤ nx ≤ maxX` and
       `0 ≤ nz ≤ maxZ`, else null; the side is 0 if `n` is the lot, else 2 if `lot.GetFacing()` (slot 23) is that
       side's number (0 → s0, 1 → s1, 2 → s2, 3 → s3), else 1. Without `boundaries` all four sides are 0;
     - `GetBoundaryLotTexture(sides, …)` (slot 42, through the vtable);
     - append the record `{texture, x, z, rotation, flip ≠ 0, colour >> 24, (colour >> 16) & 0xFF,
       (colour >> 8) & 0xFF, alpha, 3, frame 0xFF}` (the 14-byte record of lot-records.md §6; 16 bytes in memory,
       a vector with B growth: double, at least 1);
   - `GetLotBaseTextureOccupant(lot, &occ, true)` (slot 41); if it returns true:
     `occ.SetTextureSpecification(records, count, 0)` (base-texture slot 8, `0x005E0B20`; called even with 0
     records); `o := occ.AsOccupant()`; if `o.IsFlagSet(1)`: occupant manager slot 24 `(o, 0)` and
     `o.PostOccupantMessage(0x99EF1145, 0)` (occupant slot 15); else `occupantManager.InsertOccupant(o, 0)` (slot 20).
     Release `occ`.

The five bytes after the flip are therefore the zone's drag colour (top three bytes), the display alpha and 3 for
empty-lot textures (the T6 sample `FF B2 33 AA 03` of lot-records.md §6 is one), and stay `0xFF` for normal
textures (§6.6). This answers the empty-lot half of lot-records.md Q4.

**Worked example 5.** A 2 × 1 lot, cells `(3, 7)` and `(4, 7)`, zone 1, facing 0, no lot around it:
- `(3, 7)`: s0 = 2 (facing side), s1 = 0 (`(4, 7)` is the lot), s2 = 1, s3 = 1 → `0x2011` → `0x2700`, rotation 0;
- `(4, 7)`: s0 = 2, s1 = 1, s2 = 1, s3 = 0 → `0x2110` → `0x2700`, rotation 0, flip.

A 1 × 1 lot facing 2 in zone 1 gives `0x1121` → `0x2300`, rotation 2. A 1 × 1 lot in zone 8 gives code 0 →
`0x3000`. Every interior cell of a 3 × 3 lot gives `0x3000`. A 1 × 3 lot (x = 5, z = 0 … 2) facing 0 gives
`0x2101` → `0x2800` / 0, `0x0101` → `0x2500` / 1 and `0x0111` → `0x2200` / 2.
