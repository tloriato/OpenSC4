# lot-developer: answered questions (archive)

<!-- from: core-construction5 (2026-10-23) -->
- **LD35 — §6.7.15:** please specify the explicitly open scaffold mesh, SetScaffoldHeight, CreateCrane model geometry, Draw, DoRotationChange, and inner crane exact position/swing. [Unsupported for these visual operations; inner side/size draws and all outer fit rules implemented]
  **ANSWERED 2026-10-23 (re-construction-ld5):** DIFFERS — lot-developer §6.7.15 specifies topology, height propagation, relighting, render order, crane model/post geometry and exact inner position/swing. §6.7.16 supplies conformance vectors; no visual Unsupported boundary remains in these named operations.

<!-- from: core-construction5 (2026-10-23) -->
- **LD36 — §6.7.6:** what are the scaffold/completion interpolation endpoints and the milliseconds-per-day divisor, including state 0 and removal state 3? [accumulate saved ms and animate cranes; phase interpolation Unsupported pending endpoints]
  **ANSWERED 2026-10-23 (re-construction-ld5):** DIFFERS — §6.7.15 specifies all four phase formulas, nominal 500 ms/day, quantized N·segH span, zero-duration behavior, and verifies that completion divides by E. State 3 does not animate the crane.

<!-- from: core-construction5 (2026-10-23) -->
- **LD37 — §6.7.7 steps 3–4:** which y/anchor does CreateAndPlaceProp use, which initial values/roles do the two saved crew booleans have, and is RandomDirection2 normalized? [literal area minimum y, first crew byte = roof flag, second false; pass the two drawn direction components to the foreign automata service]
  **ANSWERED 2026-10-23 (re-construction-ld5):** DIFFERS — §6.7.15 uses footprint centre x/z and building position.y or maximum corner terrain altitude; crew bytes start false,roofFlag, with the second selecting scaffold following. RandomDirection2 returns normalized components.

<!-- from: core-construction5 (2026-10-23) -->
- **LD38 — §6.7.1/.7/.14:** what are the 6×5 sim-grid IDs and render-state fields; dust/scaffold effect creation parameters; crane saved y; and whether RemoveRectangle clips its four candidate parts to the input area when the cut crosses its boundary? [foreign render preparation/effect creation; crane y zero because CreateCrane accepts x/z; literal four full-extent candidates without clipping]
  **ANSWERED 2026-10-23 (re-construction-ld5):** DIFFERS — §6.7.15 identifies texture bindings rather than sim grids, gives all 30 IDs/render fields and dust/spark creation arguments. MATCHES for initial successful crane y=0 and unclipped cut candidates; subsequent height updates save crane y=h+heightAboveScaffold. Crew render gate is kEffectPriorityLevel.
<!-- Archive for [lot-developer.md](../../lot-developer.md). Lanes do not read this file. Open questions are in ../lot-developer.md. -->

<!-- from: From lot creation, the plop test, the lot developer and the graphics formats (2026-10-15) -->
- ~~**LD1:** is the occupant manager's box query inclusive at the box edges? Lot developer slot 50
  (`GetFoundationByPosition`) queries a degenerate box (a vertical line), which finds anything only if the test
  keeps a point inside the occupant's box. [inclusive or strict, unknown]~~
  **ANSWERED 2026-10-15** in this lane from the exe (slot 36 `0x004A0670`: one-cell path `0x0049D8B0`, many-cell
  path `0x004A08CC`–`0x004A0938`; six `fcomp` tests, each keeping the occupant only when its minimum is below the
  query's maximum and the query's minimum is below its maximum) and written into lot-developer.md §17.5.1 and test
  34. **Strict on every axis**, as flora.md §6.4.2 already states: the line at `(x − 0.05, z + 0.05)` finds a
  foundation only when it lies strictly inside the foundation's box, and only through the index cell of that
  point. A foundation whose edge is exactly on the line is not found.

## From the lot developer glue (filed by the coordinator from the `lotglue` lane's report, 2026-10-15)

About `docs/specs/lot-developer.md` and the replacement `cFC4LotDeveloper` (`enable = lotdev`, host
`FC4LotDevHost`) with its shadow (`lotdev_shadow = 1`); merged as `5212df2`, ABI 54. The lane numbered its first
questions LD-Q1..LD-Q11; they are filed here as LD2..LD11 (LD-Q9 is left out: it was superseded by LD1, answered
strict, and the conformance world's slot 37 query is now strict). LD12..LD30 keep the lane's numbers, which the code
comments and the game log cite. The current answer is in brackets.

<!-- from: From the lot developer glue (2026-10-15) -->
- ~~**LD2:** §16 test 30 says a 2-cell-wide rectangle makes no slot 19 call, but §17.4 step 2 gives it one interior
  column of vertices. [implemented as §17.4 is written: only a 1-cell-wide rectangle gives no call]~~ **ANSWERED 2026-10-15 (re-answers12):** MATCHES (lot-developer.md §17.4, §16 test 30 corrected). Slot 19 (`0x006C43D0`) walks `x1+1 … x2`, `z1+1 … z2`, so only a 1-cell-wide rectangle has no interior vertex; a 2-cell-wide one checks its one interior column. Test 30 was wrong: the "2-cell" skip is slot 55's own guard (`x2 − x1 > 1` and `z2 − z1 > 1`) before it calls slot 19.

<!-- from: From the lot developer glue (2026-10-15) -->
- ~~**LD3:** the retaining-wall helpers `0x005E3070`, `0x005E3960` and `0x005E37F0` have no rows in direct-calls.md.
  [host calls the plugin cannot make, so no walls are built; logged once]~~ **ANSWERED 2026-10-15 (re-answers12):** Answered (lot-developer.md §7.2, ABI rows in `build-runs/re-answers12-abi-rows.md`). The three are `__cdecl` statics of `cSC4LotRetainingWallOccupant` taking the `cISC4Lot*`, caller pops: `AddRetainingWallOccupant(lot, bool add)` `0x005E3070` (8 bytes, AL; callers `0x006C8A3D`, `0x006C8C23`), `UpdateRetainingWalls(lot)` `0x005E37F0` (4 bytes, AL a leftover), `UpdateSurroundingRetainingWalls(lot)` `0x005E3960` (4 bytes, no return). They use the global occupant manager, lot manager and BDS, so a plugin can call them by address behind the executable check. Not building walls stays a gap until the host calls them.

<!-- from: From the lot developer glue (2026-10-15) -->
- ~~**LD4:** what does slot 42 do with an empty boundary map, before `Init`? [nothing is written, and slot 43's record
  gets texture 0]~~ **ANSWERED 2026-10-15 (re-answers12):** DIFFERS, choice stands (lot-developer.md §17.7). With an empty map the key-0 retry also returns the end node (`0x006C6D2F`), and the three outputs are read from the head node's never-initialised value area: they are written, with indeterminate values. Writing nothing and giving slot 43 texture 0 is a safe *[choice]*; after `Init` key 0 always exists.

<!-- from: From the lot developer glue (2026-10-15) -->
- ~~**LD5:** what does `PickLotFoundationAltitude` do with a facing outside 0-3? [taken mod 4]~~ **ANSWERED 2026-10-15 (re-answers12):** DIFFERS, choice stands (lot-developer.md §17.5.4). The facing is not masked (`0x006C53E6`): `corners[f]` and `corners[(f + 1) % 4]` (C signed remainder) read outside the 4-float array for a facing outside 0-3. Facings are 0-3, so `mod 4` is a safe *[choice]*.

<!-- from: From the lot developer glue (2026-10-15) -->
- ~~**LD6:** what does slot 40 return when the lot has no configuration or no primary object? [position (0,0,0),
  orientation 0]~~ **ANSWERED 2026-10-15 (re-answers12):** **DIFFERS** (lot-developer.md §17.6, verified `0x006C3CF0`). With no configuration (lot slot 9) or no primary object (cfg slot 24) slot 40 returns EAX = 0 and writes neither `pos` nor `orient`: the caller's values stay. Do not write (0,0,0) / 0.

<!-- from: From the lot developer glue (2026-10-15) -->
- ~~**LD7:** what does slot 58 do with a null configuration? [treated as having no primary object]~~ **ANSWERED 2026-10-15 (re-answers12):** DIFFERS, choice stands (lot-developer.md §17.5.5). `cfg` is called through its vtable without a null check (`0x006C40F5`), so the original faults on a null configuration. Treating it as having no primary object is a safe *[choice]*.

<!-- from: From the lot developer glue (2026-10-15) -->
- ~~**LD8:** where does the orientation of a §6.6 normal base-texture record come from? [from slot 39: object
  orientation plus facing]~~ **ANSWERED 2026-10-15 (re-answers12):** MATCHES (lot-developer.md §6.6). The record's orientation byte is slot 39's `orient` output, `(object byte +0x02 + lot facing) mod 4` (stored at `0x006CA9F7`).

<!-- from: From the lot developer glue (2026-10-15) -->
- ~~**LD9:** is there a limit on slot 16's edge sweep for huge rectangles? [capped at `MAX_EDGE_SAMPLES` = 1<<20
  samples]~~ **ANSWERED 2026-10-15 (re-answers12):** MATCHES, the cap never binds (lot-developer.md §17.2). The original has no cap; after the clamp both ends lie in `[0, extent − 0.1]`, so an edge has at most `extent / 4 + 2` samples (1,026 for a 4,096 m city), and a NaN end skips the sweep.

<!-- from: From the lot developer glue (2026-10-15) -->
- ~~**LD10:** interfaces.tsv has no return-channel rows yet for slots 18, 19, 20, 21, 39, 40, 42 and 61. [these slots
  follow §11.1 and §17]~~ **ANSWERED 2026-10-15 (re-answers12):** MATCHES. interfaces.tsv now has `CORRECTS` rows for slots 18-21, 39, 40 and 42 (re-specs11, `be30d89`). Slot 61 returns the AL of slot 55 / 54, or the flags' low byte when the lot has no foundation flag (lot-developer.md §17.5.4); the proposed row is in `build-runs/re-answers12-abi-rows.md`. Slot 22 has no result (LD27).

<!-- from: From the lot developer glue (2026-10-15) -->
- ~~**LD11:** carried over from earlier stages (as in the earlier stages' code comments): the C++ runtime `rand()`
  state; the off-thread fallback; refusing re-entrant calls; base texture usage 2; hash set order; the terrain cell
  width read through city slot 91; the class ID behind `building_ref`; the reference returned by `unit_create`; side
  effects of `Write` in shadow mode; tuning that cannot be read from the original. [as in those comments]~~ **ANSWERED 2026-10-15 (re-answers12):** MATCHES (glue choices; nothing in the executable contradicts them). Facts where the executable has one: base-texture usage 2 is set on every path of slot 41 (`0x006C41FB`, `0x006C426F`; lot-developer.md §6.6); the unit reference is LD13; the tuning cannot be read back (LD30). The C runtime `rand()` state, the off-thread fallback, re-entrancy refusal and shadow-mode `Write` effects have no original counterpart.

<!-- from: From the lot developer glue (2026-10-15) -->
- ~~**LD12:** the view rotation used by the lot content and the base textures is not specified. [the host reads none;
  rotation 0]~~ **ANSWERED 2026-10-15 (re-answers12):** **DIFFERS** for zots only (lot-developer.md §5.6, §12.2 Q3). The lot developer reads the view rotation once, for the zot prop's orientation (`SC4ViewUtilities` `0x00B43DD8` +0x10 at `0x006C99ED`); base textures, props, flora and buildings use only the lot facing and the object orientation. Rotation 0 is right except for zots, which must follow the view (0 without a view).

<!-- from: From the lot developer glue (2026-10-15) -->
- ~~**LD13:** creating a business unit: the GZCOM class and IID for the new unit are not specified. [no unit is
  created; joining and leaving existing units works (units=2 in a 6-year run)]~~ **ANSWERED 2026-10-15 (re-answers12):** **DIFFERS** (lot-developer.md §5.7 step 4). The unit is `cSC4BusinessUnit`, class `0x4A232DA8`, IID `0x6A233069` (`cISC4BusinessUnit`; QI `0x006BF390` also answers 1 and `0xE4FDA3D4` at +4). The original builds it directly (0x20 bytes from C, constructor `0x006C0800`, count 0, no AddRef: the member lots hold the references), and the class is registered (factory `0x006C0EB0`). A replacement creates it with `CreateInstance(0x4A232DA8, 0x6A233069)`, adds the candidate and the lot, then releases its own reference.

<!-- from: From the lot developer glue (2026-10-15) -->
- ~~**LD14:** §7.3: the burnt-building prop and the rubble decal (type IDs, placement) are not specified. [not added;
  logged once]~~ **ANSWERED 2026-10-15 (re-answers12):** **DIFFERS** (lot-developer.md §7.2 ExplodeLot steps 2 and 4; §7.3 already gives the rubble types). Burnt building: class `0xC9EFB016`, constructor `0x005EF810(1)`, non-virtual `SetBoundsFromOccupant` `0x005EEB70` (`__thiscall` on the base, pop 4), occupant sub-object at +8 (flag `0x40`, `Init`). Decal: `cSC4PropOccupantTerrainDecal`, class `0x2977AA48`, IID `0xE982078A`, slots 3 `SetDecalTexture(key*, 2s)` and 4 `SetOpacity(0.7)`, occupant sub-object at +0xC, key `{0x7AB50E44, 0x1ABE787D, burnt ? 0x49C80EC7 : 0x2A2EF55D}`. Both classes are registered in the COM directory; both go in through prop manager slot 25.

<!-- from: From the lot developer glue (2026-10-15) -->
- ~~**LD15:** slot 38's body is not specified beyond its signature. [answers neutrally from the host side; logged once]~~ **ANSWERED 2026-10-15 (re-answers12):** **DIFFERS** (lot-developer.md §7.1). Slot 38 (`0x006C9100`): the building occupants (`0x278128A0`) in the lot's bounding rectangle from occupant manager slot 41 (flags 1); only the first entry is QueryInterfaced for `0x87DDA3A9`, that reference is released at once, the list is released (slot 48), and the pointer (no reference) or null is returned.

<!-- from: From the lot developer glue (2026-10-15) -->
- ~~**LD16:** slots 41 and 45 lack details (arguments, effects). [host side, neutral answer; logged once]~~ **ANSWERED 2026-10-15 (re-answers12):** **DIFFERS** (lot-developer.md §6.6). Slot 41: `out := null`; the first base-texture occupant at the lot's location cell (lot slot 21; occupant manager slot 33) is QueryInterfaced for `0x097F6D98` and given `SetUsage(2)`; otherwise, with `create`, `CreateInstance(0xC97F987C, 0x097F6D98)` (fallback `new`, constructor `0x005E05B0`), `Init`, `SetUsage(2)`, not inserted; AL = `out ≠ null`, caller owns the reference. Slot 45: slot 41 without create, then `UpdateVertexAltitudes(true, force, null)` (base-texture slot 9): rebounds from the terrain and, when forced or the box changed, notifies the occupant manager (slot 24) and rebuilds the models.

<!-- from: From the lot developer glue (2026-10-15) -->
- ~~**LD17:** `UpdateLotObjectAlitudes` and `Add/RemoveNetworkConnectionNodes` (§6.5 b) are named but not specified.
  [nothing is done; logged once. The completions counter counts the `AddNetworkConnectionNodes` calls: 116 in a
  2-year diagnostic run]~~ **ANSWERED 2026-10-15 (re-answers12):** **DIFFERS** (lot-developer.md §6.5). All three use interfaces only. `UpdateLotObjectAlitudes` (`0x006C91F0`): for each building, prop and flora occupant of the lot whose y differs from its slot 39 position (or the terrain without a configuration object) by more than 0.1, set only y, notify the occupant manager (slot 24, 1) and, for a building, redo its foundation (slots 48, 60, 56, 59 / 58). `AddNetworkConnectionNodes` (`0x006C9C80`): per type-7 object with ≥ 3 IDs, network manager slot 20 `CreateConnectionNode(&n, ids[0], &cell, rotl(ids[1], 8·orient), rotl(ids[2], 8·orient), ids[3] or 0, orient)`, then slot 21 `AutoconnectLotNodes(lot, &list)`. `RemoveNetworkConnectionNodes` (`0x006C9FB0`): the lot's network occupants with flag `0x200000` go to network manager slot 6 `DestroyNetworkOccupants`.

<!-- from: From the lot developer glue (2026-10-15) -->
- ~~**LD18:** what is AL from city slot 95 (used by the host)? [read as a bool, 0 / non-zero]~~ **ANSWERED 2026-10-15 (re-answers12):** **DIFFERS** (lot-developer.md §17.2). City slot 95 `PositionToCell` (`0x004599E0`) returns nothing; AL is the low byte of the x87 status word from its last `fnstsw` (sticky exception flags: 0x20 precision, 0x10 underflow, 0x02 denormal, 0x01 invalid). Treat it as void and do not read AL.

<!-- from: From the lot developer glue (2026-10-15) -->
- ~~**LD19:** flora placement: is the occupant manager's blocking filter applied before the typed flora query, or
  after? [typed query first, then the filter. Konradshohe's configurations list no flora (`cfgFlora=0`), so no game
  run has exercised this]~~ **ANSWERED 2026-10-15 (re-answers12):** MATCHES (lot-developer.md §6.2). The filter is passed into occupant manager slot 36 and tests only the type (`IsOccupantIncluded` → `IsOccupantTypeIncluded(o.GetType())`, accept = `0x74758926`, `0x006C3C20`), so typed query then filter gives the same set; the order is the manager's box-query order.

<!-- from: From the lot developer glue (2026-10-15) -->
- ~~**LD20:** lot slot 61 `GetOccupancy`: its ABI (return channel and arguments) is not in interfaces.tsv. [treated as
  returning in EAX]~~ **ANSWERED 2026-10-15 (re-answers12):** **DIFFERS** (lot-developer.md §11.4, §12.3). Slot 61 is `void GetOccupancy(float& out)`, pop 4: it writes the float through the pointer (`0x006C0E8A`; 0.0 from the BSS constant `0x00B49114` when the capacity is 0) and leaves EAX = the out pointer, so reading EAX gives an address, not the occupancy.

<!-- from: From the lot developer glue (2026-10-15) -->
- ~~**LD21:** the night deque and the shadow: the original's record has 670-740 night-deque buildings while the mode
  stays 0 in turbo, and ours matches (`roundtrip=same`, `night=L`). Should the deque be filled on insertion even when
  the visual time never changes? [yes, as recorded]~~ **ANSWERED 2026-10-15 (re-answers12):** MATCHES (lot-developer.md §8.1). Insertion (`0x006C8F80`) appends every building to the deque with no mode test; turbo with mode 0 still fills it.

<!-- from: From the lot developer glue (2026-10-15) -->
- ~~**LD22:** the §5 tests (`TestCondition` / `TestAbandonment`) have no read-only entry in the original, so the shadow
  cannot compare them. Is there a getter, or a message that exposes their result? [not compared]~~ **ANSWERED 2026-10-15 (re-answers12):** MATCHES (not compared). There is no read-only entry. The outcomes are visible only as lot changes: `cSC4Lot::SetCondition` posts `0xE99B3D4D` (`0x006C2773`) and `cSC4Lot::SetState` posts `0xE8BDA2E7` (`0x006C347C`), so a shadow can compare the condition / state after each daily pass instead.

<!-- from: From the lot developer glue (2026-10-15) -->
- ~~**LD23:** safe lot iterator fallback order: is `GetLotsInCellRect`'s snapshot order (used when the executable is
  not identified) acceptable in place of the master-list order? [used only as a fallback; `fallbacks=0` in every run]~~ **ANSWERED 2026-10-15 (re-answers12):** MATCHES as a fallback only (glue choice). The original always walks the master list through its safe iterator (§4.1); any other order changes which lots draw first from the file RNG, so the fallback is not exact. Keep it counted and logged.

<!-- from: From the lot developer glue (2026-10-15) -->
- ~~**LD24:** which base-texture occupant belongs to a lot when a cell has several? [the first occupant of type base
  texture whose lot pointer matches]~~ **ANSWERED 2026-10-15 (re-answers12):** **DIFFERS** (lot-developer.md §6.6 slot 41, §16 test 43). The original asks occupant manager slot 33 for the first base-texture occupant overlapping the lot's location cell (lot slot 21, bytes +0x7A / +0x7B), in the cell bucket's order, with **no lot-pointer check**.

<!-- from: From the lot developer glue (2026-10-15) -->
- ~~**LD25:** the construction occupant: §6.2 creates it through the BDS. When that answers null, should it be created
  directly with `new`? [no direct `new`; construction is refused and counted]~~ **ANSWERED 2026-10-15 (re-answers12):** **DIFFERS** in the premise (lot-developer.md §6.3). The construction occupant does not come from the BDS: it is `CreateInstance(0xA97F909E, 0x496E7636)` (`0x006CC7E9`), with a direct `new` (0x1A0 bytes, constructor `0x005D00F0(1)`) only if that fails. The class is registered (factory `0x005D0BC0`), so the fallback is never reached in the shipped game. Use `CreateInstance`; refusing when it fails is acceptable.

<!-- from: From the lot developer glue (2026-10-15) -->
- ~~**LD26:** the base-texture record layout (`extra[5]`, frame) beyond the documented fields. [the undocumented bytes
  are copied unchanged]~~ **ANSWERED 2026-10-15 (re-answers12):** MATCHES (lot-developer.md §17.7, lot-records.md §6). The in-memory record is 16 bytes: texture, x, z, orientation, flip / object byte, five bytes (constructor `0x005CA770` sets `0xFF`), frame (`0xFF`), 2 bytes padding. For empty lots the five bytes are the zone drag colour R, G, B, the display alpha and 3; for normal textures they stay `0xFF`. Copying them unchanged is right.

<!-- from: From the lot developer glue (2026-10-15) -->
- ~~**LD27:** slot 22 `GetOccupantCellCoverage`'s AL is the raw AL of its second `PositionToCell`. In game it is
  0x32 / 0x33 for 62-64 of the 64 sampled buildings per month (`rawAL=62-64`), not 0 / 1. Do callers rely on the raw
  byte, or only on whether it is true? [the core returns a bool, and the shadow compares only the truth value;
  rectangles match. If callers need the byte, `fc4_lotdev_occupant_cell_coverage` returns it and the shadow's
  `CompareLot` compares it raw]~~ **ANSWERED 2026-10-15 (re-answers12):** MATCHES (lot-developer.md §17.2). The byte is the x87 status flags left by `PositionToCell` (LD18), not a result. Slot 22's only callers are `ChangeBuilding` (`0x006C52DB`), `GetOccupantSlope` (`0x006C5F64`) and `StartConstruction` (`0x006CC764`), through `this`; none reads AL. Comparing only the rectangle is right.

<!-- from: From the lot developer glue (2026-10-15) -->
- ~~**LD28:** the night batches (§8.3) never ran in any turbo run: `nightBatches=0` and `mode=0` throughout to 2032.
  Does a turbo city ever send the day/night change `0x44778234` without the 24-hour clock running? [not exercised;
  lights are set at insertion instead (`lights=203`)]~~ **ANSWERED 2026-10-15 (re-answers12):** Answered (lot-developer.md §8.2). Only the 24-hour clock posts `0x44778234` (`ChangeVisualTime` `0x00445701`, `EnableDayNightTransitionNotifications(true)` `0x0044566E`); every other use is a subscription. Without a visual-time crossing (turbo starves the idle loop, CK22) there is no message and no batches; insertion lighting is the only path, as observed.

<!-- from: From the lot developer glue (2026-10-15) -->
- ~~**LD29:** slot 25 `IsAreaFlat` disagrees with "altitude range within `0xE900EA24`" (shipped 1.0). In the shadow,
  the original answers flat for lots whose slot 27 range is bit-identical on both sides and > 1.0. Observed: 1.0355,
  1.0623, 1.0735, 1.0886, 1.0948, 1.1135, 1.1373, 1.1757, 1.1858, 1.2204, 1.2268, 1.2937, 1.3282, 1.3630, 1.4001,
  1.4131, 1.4340, 1.4482, 1.4526, 1.5149, 1.6784, all on both the dry and with-water variants; slot 26 follows. What
  exactly does slot 25 compare: another measure, another threshold, or a scaled tuning value? [range <=
  `kMaxCellAltitudeDeltaForFlatness`; 0-24 mismatches per shadow line, all of them this. Settled by the exe; then
  only `LotDeveloper::is_area_flat` (core `lot_developer/terrain.rs`) changes, with a §16 vector, and
  `lotdev_shadow = 1` should show mismatches=0 on every line (the `s25r` entries give the ranges)]~~ **ANSWERED 2026-10-15 (re-answers12):** **DIFFERS** (lot-developer.md §17.2, §16 test 42). Slot 25 (`0x006C5C30`) compares each vertex with the **first vertex** `(x1, z1)`, not the range: `ref := f32(slot 15(x1, z1, true))` (always with water, literal 1 at `0x006C5C4B`), then for x outer, z inner over `x1 … x2+1`, `z1 … z2+1`: `|slot 15(x, z, withWater) − ref| > +0xD0` (kMaxCellAltitudeDeltaForFlatness, strict, NaN no) → false. A range up to twice the tuning value is flat, which explains every observed range 1.0355-1.6784.

<!-- from: From the lot developer glue (2026-10-15) -->
- ~~**LD30:** the tuning cannot be read back from the original (no getter), so the shadow cannot check that both sides
  use the same `kMaxCellAltitudeDeltaForFlatness` and the other §3.5 values. Is there an accessor? [not compared]~~ **ANSWERED 2026-10-15 (re-answers12):** MATCHES (lot-developer.md §3.5). No accessor: `ReadSimulatorTunableValues` (`0x006C4A20`) has one caller, `Init` (`0x006CB9C7`), and nothing else writes the members (`kMaxCellAltitudeDeltaForFlatness` at `+0xD0`). A shadow reads the same exemplar with the same rules to get the original's values.

## From applying the re-answers12 lot developer and zone developer answers (filed by the coordinator from the `apply13` lane's report, 2026-10-16; merged `adf64f4`)

apply13 applied LD2-LD30 (lot-developer.md) and ZG26, ZG30-ZG34 (zones.md, tract-developer.md §4.15.3); ABI 60, taken
as 71 on the lane. Not applied here: ZG22 (slot 19 always writes `*ok`) and ZG24 (the zone tool owns the highlight
region), from re-answers11. The lane's choices are in brackets.

<!-- from: From applying the re-answers12 lot developer and zone developer answers (2026-10-16) -->
- ~~**LD31:** (lot-developer.md §6.6, LD17) UpdateLotObjectAltitudes: is the building's foundation redone only after the
  object actually moved (beyond the 0.1 tolerance), or for every building visited? [only after a move (inferred)]~~
  **ANSWERED 2026-10-16 (re-answers19):** MATCHES (lot-developer.md §6.5). When `|P.y − Q.y| ≤ 0.1` the jump at
  `0x006C9326` skips the move **and** the foundation work; the building test (`0x278128A0`, `0x006C934B`) comes after
  `SetPosition` and the occupant manager notify, inside the moved branch.

<!-- from: From applying the re-answers12 lot developer and zone developer answers (2026-10-16) -->
- ~~**LD32:** (lot-developer.md §6.5) AddNetworkConnectionNodes: when CreateConnectionNode (network manager slot 20)
  fails and leaves its out-parameter null, the original appends whatever was returned to the list that
  AutoconnectLotNodes (slot 21) reads. Is a null entry appended and passed on? [a null result is not appended; it is
  counted as `nodesFailed` (choice)]~~ **ANSWERED 2026-10-16 (re-answers19):** **DIFFERS** in the letter; the
  choice is accepted (lot-developer.md §6.5, quirk 17). The out-parameter is not nulled: it is one stack slot for
  the whole loop, never cleared, and `CreateConnectionNode` writes it only on success (`0x006114DE`). The append
  (`0x006C9EE4`) does not test the result, so after a failure the list gets the previous object's node again (or an
  uninitialised value for the first network object). `AutoconnectLotNodes` QueryInterfaces every entry with no null
  test (`0x006115F7`), and the final release (occupant manager slot 48) over-releases a repeated node. Nothing in the
  original avoids this, so skipping the failed entry is the safe *[choice]*.

<!-- from: From the return-channel audit (2026-10-16) -->
- ~~**LD33:** (lot-developer.md §6.6) cSC4LotBaseTextureOccupant slot 9 UpdateVertexAltitudes(bool, bool, ptr): what does
  it return? §6.6 passes it through as slot 45's AL. [passed through unchanged, as the original does]~~
  **ANSWERED 2026-10-27 (re-q-misc):** DIFFERS — `UpdateVertexAltitudes` (`0x005DF820`) has no defined result: it returns with its third argument (the changed-flag pointer, null from slot 45) still in EAX. Slot 45's own result is not that value either: with an occupant found its last call is the `Release` of the occupant (remaining count, 0 when freed), otherwise it is slot 41's false. Neither is a bool, so nothing is "passed through" (lot-developer.md §6.6, §11.1 corrected).

<!-- from: From the return-channel audit (2026-10-16) -->
- ~~**LD34:** lot-developer.md §11.1 gives slot 8 AbandonBuilding as "AL of SetState", and lot-records.md §13.2 gives lot
  slot 34 SetState as "none defined". So is AbandonBuilding's result undefined? tract-developer.md's interface table
  lists it as plain AL. [passed through; the tract core never reads it]~~
  **ANSWERED 2026-10-27 (re-q-misc):** DIFFERS — `AbandonBuilding` (`0x006C3D30`) calls `SetState(4, 0, reason)` and returns at once, and `SetState` has no result, so its AL is an incidental leftover, not "AL of SetState" as lot-developer.md §6.5/§11.1 said (corrected to "undefined"). Both tract-developer call sites ignore it. tract-developer.md's interface table ("AL") still needs the same correction; it is outside this batch.
