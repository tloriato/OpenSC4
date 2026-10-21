# terrain: answered questions (archive)
<!-- Archive for [terrain.md](../../terrain.md). Lanes do not read this file. Open questions are in ../terrain.md. -->

## From the terrain and nation specs (re-specs14a lane, 2026-10-15)

Open points from terrain.md §17. The current answer is in brackets; each entry says what would settle it.

<!-- from: From the terrain and nation specs (2026-10-15) -->
- ~~**TR1:** the exact operations of brush component kinds 2, 3, 5, 6 and 8-10 (smoothing, levelling, erosion, lake,
  rain, spring). [summarised only; only the god-mode terraforming tool uses them. A trace of `0x00745DE0` and
  `0x00745BA0` settles 2 and 5] **PARTLY ANSWERED 2026-10-16 (re-answers17):** kinds 2 and 5 are traced in the bytes and specified in terrain §9.4.1. Kind 5 levels toward the target by `step = b × strength / 256` (sets the target when within a step, else raises or lowers by a step; `b = 0` or `alt = target` is no change). Kind 2 is talus smoothing: for each neighbour in the offset order, the excess drop over a threshold (`tan(TalusAngle) × 16`, × √2 on diagonals) times the step moves from the vertex to the neighbour. Kinds 3, 6 and 8-10 and the shipped `TalusAngle` stay open (god-mode only).~~ **ANSWERED 2026-10-16 (re-answers28):** kinds 6, 8, 9 and 10 and the shipped `TalusAngle` are checked in the bytes and in the shipped game data (terrain §9.4.4, §9.4.5, §9.7). **Shipped `TalusAngle` is 5 degrees**, not the 0.79 rad fallback: it lives in the terrain ini as DBPF type `0x0`, group `0x8A5971C5`, instance `0xAA597172` of `SimCity_1.dat`, and the load path turns it into `Tedge = tan(angle) × 16 ≈ 1.39982 m` and `Tdiag ≈ 1.97964 m`. The same ini carries every other terrain tunable (terrain §9.7), and several shipped values differ sharply from the compiled fallbacks (erosion rates 0.05/0.4 vs 0.02/0.06, `DepositRate` 0.65 vs 0.2, `RainFallFactor` 0.0001 vs 0.01). Kind 6 is hydrological erosion that writes only water depths and sediments over a `radius/16`-wide rectangle (no bitmap footprint); kind 8 is grey-modulated levelling toward `target_height + (b - neutral) × strength[level]`, refusing to raise a wet vertex; kind 9 raises the water surface of every vertex it can reach from the brush centre to `surface + strength`, capped at 200 m of depth; kind 10 adds 1.0 to the centre vertex's water source field. Two corrections to the old text: kind 8 uses the component's **raw** `strength[level]`, so `tBrushParameters.strength` does not apply; and kind 6 is **not** god-mode only - `ErosionBrush 0x6A4BAAFA` carries a kind-6 component and is driven by the effects manager (`erodefx_terrain`, `stream_cut_fx_terrain`, effects §8.4). Kinds 1 and 3 were also traced (terrain §9.4.2, §9.4.3): both are object scatters, not height edits, and both draw from the terrain generator per vertex, which the old "one RNG draw per application" note missed. **Still open:** kind 3's full scatter (no shipped brush uses it), the solid-footprint grey value of `FUN_00742440`, and what the surface water's tick does with the `+0x04` / `+0x08` fields kinds 10 and 9 write.

<!-- from: From the terrain and nation specs (2026-10-15) -->
- ~~**TR2:** the brush values of the `volcano_control` effect (level, strength and radius ranges, repetitions per
  update). [effect brushes use `data4 = 3` and redisplay on the last repetition; the effects manager spec, INVENTORY
  rank 7, settles the values]~~ **ANSWERED 2026-10-15 (re-specs16):** **MATCHES** the bracket and adds the values
  (effects.md §8, §10.1; shipped effect directory, brush records 2–10). `volcano_control` (description 367) chooses
  one of nine `volcanocreator*` effects through a select group (weights 7281 × 8 and 7287, one draw from the
  manager's `cRZRandomSuper` seeded `0x1105`, effects.md §5.3). Each creator holds one VolcanoBrush `0xCA313035`
  stroke:
  - level 1;
  - strength 1.0 (range 1–1);
  - radius scale 0.5, 0.5, 0.6, 0.6, 0.8, 0.8, 1.0, 1.0 or 1.3 (degenerate ranges, but both `RandomDoubleRangeUniform`
    draws on `0x00B22BE8` still happen);
  - target 100;
  - rate 30, 40, 45, 50, 55, 65, 65, 80 or 130 applications per effect-second;
  - length 3, 4, 3.5, 3.5, 3, 3, 3, 3.5 or 2 effect-seconds.

  Repetitions per update: `n = floor(acc)` with `acc += rate × dt` (dilated seconds), `acc −= n` (`0x005817F0`).
  Each creator also carries a one-time game record at the vent: demolish 5–10 cells, burn 10–14.

<!-- from: From the terrain and nation specs (2026-10-15) -->
- ~~**TR3:** `GetLerpedWaterLevel` (terrain slot 32) and `AltitudeAtWhichFloodingWillOccur` (surface water slot 10)
  are from pseudocode only. [bilinear over the four corners; maximum over wet neighbours. Check the bytes if a
  simulation consumer appears]~~ **ANSWERED 2026-10-16 (re-answers17):** both checked in the bytes. `GetLerpedWaterLevel` MATCHES (bilinear over `depth + alt`), with no clamp at `x = sizeX` (the `ix + 1` corner wraps to the next row) (terrain §5.1). `AltitudeAtWhichFloodingWillOccur` **DIFFERS** from the round-1 text: it takes `(x, z)`, starts at the sea level, counts the vertex's own level when it is wet, then the wet neighbours in the offset order, strict maximum (terrain §10.2).

## From the terrain core (filed by the coordinator from the `cores16` lane's report, 2026-10-16; merged `d651d74`)

cores16 implemented terrain.md as a Rust core (`engine/core/src/terrain/`), a C ABI (`fc4_terrain`, ABI 58) and
conformance vectors (`tr18_*`, `tr00_*`, `nt05_economy_modifier`), and nation.md GetEconomyModifier. There is no
plugin glue yet, so none of these readings has been checked in game. Each choice is marked `[choice]` in the code
and the vectors. The current choice is in brackets.

<!-- from: From the terrain core (2026-10-16) -->
- ~~**TR4:** (terrain §11.3/§11.5) in what order are the lava neighbour offsets scanned? When several neighbours tie
  for the largest drop, the first one wins. [west, east, north, south, then the diagonals]~~ **ANSWERED 2026-10-16 (re-answers17):** **DIFFERS.** The offsets at terrain `+0x84`…`+0xA0` are W, NW, N, NE, E, SE, S, SW (`Init` `0x007500E0`–`0x00750129`; north is z − 1). The edges are scanned k = 0, 2, 4, 6, so **W, N, E, S**, then the diagonals NW, NE, SE, SW (`0x0075DB09`–`0x0075DBD6`). The first wins ties (strict `>`), as in the bracket; only the N/E order changes. terrain §11.3, test 15.

<!-- from: From the terrain core (2026-10-16) -->
- ~~**TR5:** (terrain §11.5) does SetThingsOnFire call SetCause(3) after each ignited cell, or once per call? [per cell]~~ **ANSWERED 2026-10-16 (re-answers17):** MATCHES: `SetCause(3)` follows each ignited or burned cell, and only flammable cells (`0x0075DF3D`–`0x0075DF47`); the cells go x outer, z inner. terrain §11.5, test 16.

<!-- from: From the terrain core (2026-10-16) -->
- ~~**TR6:** (terrain §10.2) what does GetWaterType return for a vertex with a NaN depth? [the comparisons fail, so it
  falls through to fresh water (2)]~~ **ANSWERED 2026-10-16 (re-answers17):** MATCHES: the first test (`depth ≤ 0`) is false for NaN, the level is NaN, and the second test returns 2 (`0x0073D811`–`0x0073D83E`). terrain §10.2.

<!-- from: From the terrain core (2026-10-16) -->
- ~~**TR7:** (terrain §9.3) ComputeSpansForRectangle's footprint spans are not fully specified. [an inclusive vertex
  rectangle clamped to the terrain]~~ **ANSWERED 2026-10-16 (re-answers17):** **DIFFERS.** `ComputeSpansForRectangle` (`0x00740BB0`) scan-converts the rotated rectangle: rows `trunc(z / 16)` of the corners (unsigned clamp to `cellsZ`), half-open edge crossings per row `Z = 16z`, each span from the leftmost crossing column to one past the rightmost (unsigned clamp to `cellsX`), visited inclusive. The full rule, the corner order and a worked example are in terrain §9.3.

<!-- from: From the terrain core (2026-10-16) -->
- ~~**TR8:** (terrain §9.1/§9.4) what is a brush description's starting angle, and is it stored per terrain or per
  description? [0, per description]~~ **ANSWERED 2026-10-16 (re-answers17):** MATCHES for what the brushes use. Each description has two angles: the stroke angle (`+0x6C`, used by oriented components) starts at 0; the walk angle (`+0x68`) starts at `RandomRange(0, 2π)` of the terrain generator when the walk step is non-zero. The step property `0x8A020102` defaults to 0.5° when missing, which is the case for every shipped brush, so each in-bounds application draws `RandomUint32Uniform(5)` (`0x00747EC0`, `0x0074D7BD`–`0x0074D855`). Stored per description in the static map, so shared by every terrain. terrain §9.1, §9.3.

<!-- from: From the terrain core (2026-10-16) -->
- ~~**TR9:** (terrain §2.3) what is the starting value of the default brush level at `+0xBC`? [0]~~ **ANSWERED 2026-10-16 (re-answers17):** MATCHES at start, **DIFFERS** in game: 0 from the constructor (`0x0074C408`), then `DoPostCityInit` copies the 3D camera's zoom (`0x0074955F`) and every view-update message `0xA6B79602` with a new zoom updates it (`0x0074AAE2`). A harness with no view keeps 0; the glue should feed the zoom. terrain §2.3.

<!-- from: From the terrain core (2026-10-16) -->
- ~~**TR10:** (terrain §13.1) no record holds the sea level, so what is it after a load? [the tuning's `new_sea_level`]~~ **ANSWERED 2026-10-16 (re-answers17):** MATCHES: `Init` sets `+0x7C` from the tuning value (`0x007500D5`) and `Load` never writes it. terrain §13.1, test 23.

<!-- from: From the terrain core (2026-10-16) -->
- ~~**TR11:** (terrain §8.1) how are the data words of texture message `0x870BC918` packed? [no data, all zero]~~ **ANSWERED 2026-10-16 (re-answers17):** **DIFFERS.** data1 and data2 pack the render rectangle like the other two messages (the rect padded by 1 and clamped when the terrain flag is set), data3 is `(trunc(lo) << 16) + trunc(hi)` of the 3D view's altitude band, and data4 is the caller's (`0x00749894`–`0x007499EE`). Render-only; a view-less replacement may use the plain minimum and maximum. terrain §8.1.

<!-- from: From the terrain core (2026-10-16) -->
- ~~**TR12:** (terrain §3/§15) what do the slots with unchecked indices (GetAltitudeAtIndex, SetAltitude(x, z) and
  others) do out of range? [a read returns 0 or NaN, a write does nothing]~~ **ANSWERED 2026-10-16 (re-answers17):** there is no defined original value: the listed slots form a flat index with no check and read or write past the array when it is out of range. The bracket is accepted as a *[choice]*, refined: reads return 0.0 (never NaN, matching the checked siblings), `GetWaterType` 0, `GetWaterVelocity` a shared zero vector, writes are ignored; and while the flat index is inside the array the access must happen (coordinates past the last column wrap to the next row). Also corrected: slots 30, 31, 33, 55 and 92 are bounds-checked. terrain §5, §10.5.

<!-- from: From the terrain core (2026-10-16) -->
- ~~**TR13:** (terrain §10.4) does a power pole block navigation only on a cell with depth > 0? [yes: depth > 0 means
  GetWaterDepthForCell > 0]~~ **ANSWERED 2026-10-16 (re-answers17):** MATCHES: the pole handler sets the dirty flag when `GetWaterDepthForCell(x, z) > 0`, strict (`0x00740294`–`0x007402A4`). The flag lives at object `+0x50`, not `+0x48`. terrain §10.3.

<!-- from: From the terrain core (2026-10-16) -->
- ~~**TR14:** (terrain §8.4) does SetWaterSurfaceDisplayState post on every call or only when the state changes?
  [every call]~~ **ANSWERED 2026-10-16 (re-answers17):** **DIFFERS**: only on a change. Slot 84 (`0x007434E0`) compares with `+0xDD` and returns when equal; otherwise it stores and posts. terrain §8.4, test 21.

<!-- from: From the terrain core (2026-10-16) -->
- ~~**TR15:** (terrain §9.3) is a kind-0 brush reported as a change even when every touched vertex was locked or
  clamped? [yes: the out rectangle and the redisplay still happen]~~ **ANSWERED 2026-10-16 (re-answers17):** MATCHES: kind 0 returns true for every `b ≠ neutral` after calling `RaiseVertex` or `LowerVertex`, which skip a locked vertex silently (`0x00745C60`). terrain §9.4.

<!-- from: From the terrain core (2026-10-16) -->
- ~~**TR16:** (terrain §9.4) with an angle of exactly pi, sin(pi) in f32 is not 0, which gives a one-pixel fringe on the
  earthquake stroke. Is that intended? [kept as computed]~~ **ANSWERED 2026-10-16 (re-answers17):** MATCHES: the original computes `fcos` / `fsin` of the f32 angle and stores f32 (`0x00742216`–`0x0074221E`), with no snapping, so it has the same fringe. The stroke angle itself is `f32(atan2 + f32(π/2))`. terrain §9.3.

<!-- from: From the terrain core (2026-10-16) -->
- ~~**TR17:** (terrain §10.4, plumbing §7.2) for IsNavigable, are the city bounds behind slot 99 the terrain's cells,
  and does the search follow the plumbing §7.2 spiral? [yes to both]~~ **ANSWERED 2026-10-16 (re-answers17):** MATCHES: city slot 99 (`CellIsInBounds` `0x004594D0`) is `0 ≤ x < cellsX`, `0 ≤ z < cellsZ` with the same counts the terrain takes, and the walk is the plumbing `SpiralIterator` (`0x0043FE10` / `0x00440850`) over the unclipped square. terrain §10.4.

<!-- from: From the terrain core (2026-10-16) -->
- ~~**TR18:** (glue contract) the lava and brush host callbacks run while the core holds the terrain mutably, so a
  callback that calls back into the same `fc4_terrain` handle is refused (and counted). Should the glue lane queue
  such work instead? [refused and counted; for the glue lane to decide]~~ **ANSWERED 2026-10-16 (re-answers17):** on the simulation's paths the original never calls back into the terrain from a lava or brush callee: the flammability and fire calls do not touch the terrain, and every terrain message is queued. Refuse-and-count is compatible (the count should stay 0); no queue is needed. Only the god-mode kinds 1 and 8–10 reach other objects that may read the terrain or are the surface water. terrain §15.4.

## From the terrain shadow (filed by the coordinator from the `glue16` lane's report, 2026-10-16; merged `4cc88cf`)

glue16 added the read-only terrain shadow (`terrain_shadow = 1`, `FC4TerrainShadow`) and the script action `terrain`. Its
final run glue16-final-2 on Konradshohe had every terrain line exact, but Konradshohe has no high bridge over water
and no power pole on water, so the bridge and power-pole steps of IsNavigable ran only through their "nothing here"
answers (23002/23002). The current choice is in brackets.

<!-- from: From the terrain shadow (2026-10-16) -->
- ~~**TR19:** (terrain §10.4 step 4) for "the tile's deck height (tile slot 67) is known", the glue reads slot 67 as the
  SDK's `cISC4NetworkOccupant::GetPassageCeiling(float&)`: "known" means it returns true, and the float is compared as
  a world height with alt + depth at the cell's north-west vertex (>= 20 m clearance). Is that the slot's meaning and
  unit? [inferred; not exercised in game: a city with a ferry under a high bridge would test it]~~ **ANSWERED 2026-10-16 (re-answers25):** MATCHES. Network occupant slot 67 (vtable `0x00AA97B8`, `0x00619900`) is `GetPassageCeiling(float&)`: it reads property `0x4835C6A8` through the occupant's property holder and returns true with the float when present, false otherwise (`0x0061991D`–`0x0061992B`); slot 68 `SetPassageCeiling` (`0x0061B9A0`) stores it. The only writer is the bridge builder (`0x0062F005`), for pieces whose exemplar lacks `0xABE1B5A3`: the lowest of the cell's four corner heights minus the bridge rule's model offset, i.e. the absolute world height of the underside. IsNavigable (`0x0073EE95`–`0x0073EECD`) returns true when ceiling − (alt + depth at vertex (x, z)) is not below 20.0 (`0x00AA0F1C`), false when below or when the property is absent. terrain §10.4 step 4, test 24.

<!-- from: From the terrain shadow (2026-10-16) -->
- ~~**TR20:** (terrain §10.4 step 5) for "a power pole (type 0x2890D4DE) whose cell is (x, z)", the glue asks occupant
  manager slot 41 (the typed GetOccupantsByStandardCityCells) for the single cell min = max = (x, z), so any pole whose
  bounding cells include (x, z) blocks it. Does the original test the pole's own position cell instead? And is (x, z)
  the visited spiral cell, not the query's centre? [the visited cell, bounding-cell overlap]~~ **ANSWERED 2026-10-16 (re-answers25):** (x, z) is the visited spiral cell (MATCHES); the pole test **DIFFERS**: after occupant manager slot 41 (`0x0073EDD4`, min = max = (x, z), type `0x2890D4DE`, -1), each returned occupant is queried for `cISC4PowerPoleOccupant` (`0xA8C86142`) and blocks only when its own cell (pole slot 8 `GetCell`) equals (x, z) (`0x0073EDF1`–`0x0073EE2F`); a bounds overlap alone does not block. Found → list freed, return false (`0x0073EEEA`). terrain §10.4 step 5, test 25.

## From the terrain 3D view and the remaining `cISTETerrain` slots (filed by the `re-answers30` lane, 2026-10-16)

glue18 wired `cISTETerrain` / `cISTESurfaceWater` to `fc4_terrain` behind `enable = terrain`. The shadow is exact
(98/98 and 100/100 lines, 0 unfollowed, 0 pending) but the replacement does not reach `clean`: it builds both
classes, reads the class record (`ok=1 version=2`), creates the core and gets through `PostCityInit`, then **faults
in the terrain 3D view**. `terrain.md` had deliberately left the view out of scope; that gap is the blocker. The
lane numbered these TR21-TR27.

<!-- from: From the terrain 3D view and the remaining `cISTETerrain` slots (2026-10-16) -->
- ~~**TR21:** the terrain 3D view (class `0xC9B84E10`, terrain `+0xE8`, slot 66) — who creates it, from what, what
  it needs from `cISTETerrain` before it can exist, and in what order relative to `PostCityInit`? What does
  zones.md §5.2's slot-66 call actually do? [unknown; the view is "render only"]~~ **ANSWERED 2026-10-16
  (re-answers30):** written up as terrain.md §15.5. **DIFFERS** from the old scope note only in that the view is
  now in scope; every mechanism MATCHES the bytes. The terrain's own `PostCityInit` handler creates it —
  `DoPostCityInit`, slot 40, `0x00749540`, dispatched from the terrain's `DoMessage` (`0x0074A569`,
  `call [edx+0xA0]`) on `0x26D31EC1`. It runs once (guard byte terrain `+0x14`) and: copies the camera zoom into
  `+0xBC`, creates the view through the GZCOM factory (CLSID `0xC9B84E10`, IID `0x6771477D`, slot 3
  `CreateInstance`) or `operator new(0xE8)` + ctor `0x007567C0`, calls `view->Init(terrain)` (`0x0075C420`,
  `__thiscall`, `ret 4`, one stack argument = the terrain), then sets the view-ready flag `+0xEC`. **Slot 66 is
  `GetView` (`0x00741810`) and returns `view + 0x0C`** — the same IID `0x6771477D` interface the factory hands out,
  which is why `DoPostCityInit` does `add eax,-0x0C`. zones.md §5.2's call is exactly that accessor, cached and
  `AddRef`'d into the zone developer's `+0x38`, and it **`AddRef`s with no null check** (`0x0073291D`), so the
  terrain's `DoPostCityInit` must run first — it does, because the terrain subscribes to `PostCityInit` first
  (`0x007502E5` vs `0x00733611`; foundations §3.6). `Init` needs only terrain slots 1, 7, 8 and 42; the vertex
  array at `+0x6C` is needed at draw time. Object layout (size `0xE8`), all five vtables with return channels and
  the full sequence are in §15.5. **Two globals `Init` dereferences with no null check are the likely fault:**
  `0x00B43CF0` (the surface water, from city slot 83) at `0x0075C4A3` and `0x00B43CD8` at `0x0075C458`.

<!-- from: From the terrain 3D view and the remaining `cISTETerrain` slots (2026-10-16) -->
- ~~**TR22:** does the game read the terrain object's fields directly, beyond the interface slots? [§2.3 marks them
  internal]~~ **ANSWERED 2026-10-16 (re-answers30):** **DIFFERS** — §2.3's "otherwise internal" is wrong. Three
  classes hold a `cISTETerrain*` and read fields directly: `cSTESurfaceWater` (water `+0x14`; reads `+0x28`,
  `+0x2C`, `+0x38`, `+0x3C`, `+0x40`..`+0x4C` by address-of, `+0x5C`, `+0x6C`, `+0x7C`, `+0x84`..`+0xA0`),
  `cSTEVolcanoLava` (lava `+0x14`; `+0x6C` cached into lava `+0x8C`, `+0x84`..`+0xA0`) and `cSTETerrainView3D`
  (view `+0x20`; `+0x6C` only). Full lists in terrain.md §2.3's `**Corrects (TR22)**` block. The view also
  **writes** the vertex records: `LevelCell` `0x00759873` does `or ...,0x60` and `RemoveCellLevelling`
  `0x007586DA` does `and ...,0x9F` on `+0x22` — the §7 hide bits used as levelling marks — so the vertex array
  must be writable and those bits must survive. Nothing external reads `+0x24`.

<!-- from: From the terrain 3D view and the remaining `cISTETerrain` slots (2026-10-16) -->
- ~~**TR23:** for `0xA6B79602`, is the camera zoom in `data1` or in `*void2`? And is the camera reachable from the
  terrain object? [§2.3's "first data word"]~~ **ANSWERED 2026-10-16 (re-answers30):** **DIFFERS**. The zoom is at
  **`*GetVoid2()`** — message slot 15 (`0x0074AADD`), dereferenced at `0x0074AAE0`, compared with terrain `+0xBC`
  and stored only when it differs (`0x0074AAF4`). Not `data1`; §2.3 has been corrected. It is the same source
  zones.md §5.2 reads. The camera is **not** reachable from the terrain object (there is no camera interface on it),
  but it is reachable at all: `SL::OnView3DInit` (`0x006025D7`) stores `0x00B43DD0` slot 8 into the camera global
  `0x00B43DD8`, and `0x00B43DD0` is the same draw-callback registry the view's `Init` already needs. The camera is
  otherwise a raw global that many components read directly (`cSC4ModelMaker::DoPostCityInit` `0x00495050`,
  `cSC4MissingPluginDataReporter::DoPostCityInit`, `cSC4AnimationCoordinator::Init`, …). A host with neither route
  leaves `+0xBC` at 0, which is what a headless harness sees and is harmless until the first view message.

<!-- from: From the terrain 3D view and the remaining `cISTETerrain` slots (2026-10-16) -->
- ~~**TR24:** rules for the §15.1 slots §4-§10 do not define: 25 `GetTerrainGridIndex`, 39 `AreSeaVertices`,
  58 `GetBrushExemplarID`, 61 `BrushRadius`, 62 `GetCostPerBrushApplication`. [undefined]~~ **ANSWERED 2026-10-16
  (re-answers30):** rules in terrain.md §15.6.1. **PARTLY** — the names are broadly right; every rule was missing,
  and slot 39 **DIFFERS** from the natural reading. `AreSeaVertices(rect*)` (`0x00740750`, pop 4, **AL only**) is
  an **"all sea"** test: false if any coordinate is out of bounds (unsigned), true for an empty rect, else true iff
  **every** vertex has `altitude < sea(+0x7C)` — ground altitude, not depth, so it is not a presence test. Slot 25
  (`0x0074C6B0`, pop 8, EAX) is `trunc(r24(x·0.0625)) + trunc(r24(z·0.0625))·vertsX`, no bounds check, and uses
  `_ftol2` (truncate) where `GetAltitude` uses floor, so they disagree for negative coordinates. Slot 58
  (`0x007415D0`, pop 8, AL only, `this` unused) returns `*(u32*)(0x00AB3C88 + 4·index)` with no bounds check (8
  entries). Slots 61 (`0x00747AE0`, ST0) and 62 (`0x00747C90`, EDX:EAX) index a per-level array by the **default
  brush level** `+0xBC` with no clamp; an unknown brushID gives 32.0 and 32.

<!-- from: From the terrain 3D view and the remaining `cISTETerrain` slots (2026-10-16) -->
- ~~**TR25:** rules for 43 `CreateFlatAreas`, 44 `CreateShoreCliffs`, 46 `ChangeSeaLevel`, 48 `ChangeTerrainLevel`,
  56 `StartFlood`. [data4 0 / 0 / — / 1 / "+500 m on vertices 20..39 x 20..39"]~~ **ANSWERED 2026-10-16
  (re-answers30):** rules in terrain.md §15.6.2. **DIFFERS** in four places. (a) **Quirk 19 is wrong**: slot 47
  (`0x0074C660`) returns `fld [0x00B0D588]`, filled from tuning `0x48CD7B73` — shipped **40.0**, fallback 20.0 —
  so the ini value is used, not thrown away, and it is not "the constant 20.0". (b) `0x48CD7B72`/`73` are
  `ChangeSeaLevel`'s step (5.0) and the default raise/lower amount (40.0), not "two editor values". (c) The height
  clamps are the globals `0x00B0D5E0` (`MinAllowedHeight`, fallback **5.0**) and `0x00B0D5DC` (`MaxAllowedHeight`,
  fallback **200.0**); terrain.md §6 and §4.2 quote the shipped 20/2500 as if absolute. (d) Slot 56
  (`0x00747720`, no args) does far more than the documented dump: it snapshots depths to the scratch at `+0xE0`,
  arms a `(20,20,40,40)` flood rectangle at `+0x19C` with counter `+0x1AC`, adds 500.0 to vertices 20..39 × 20..39,
  sets `+0xD4`, and **tail-calls one `DoFloodFlowStep` immediately**. Slots 43/44/46/48 themselves MATCH the
  spec's one-liners once expanded (data4 0 / 0 / — / 1 confirmed). New fields `+0xD4`, `+0xE0`, `+0x19C`..`+0x1A8`,
  `+0x1AC`; flag bits `0x0001`/`0x0002`/`0x0004` are transient work marks, never saved.

<!-- from: From the terrain 3D view and the remaining `cISTETerrain` slots (2026-10-16) -->
- ~~**TR26:** the five argument dwords of surface water slot 7 `IsNavigableAtPosition`. ["converts the position to a
  cell with city slot 95 and calls slot 8"]~~ **ANSWERED 2026-10-16 (re-answers30):** **PARTLY** — the outline
  MATCHES, the signature did not. It is `bool IsNavigableAtPosition(float x, float y, float z, int32 r, void* lot)`
  (`0x0073CEF0`), thiscall, **callee pops 20 bytes, returns in AL only** (quirk 10). **`y` is never read.** `r` is
  a cell radius and `lot` an opaque handle, both forwarded untouched to slot 8. City slot 95 `LocationToCell`
  (`0x004599E0`) is `floor(coord × 0.0625)` per horizontal axis — plain world→cell, no origin offset, not
  `GetVertIndex`. *[quirk]* it passes **`&arg1`** as the cell-z out-parameter, so the incoming `x` is overwritten;
  invisible to a caller (callee-pop) but a reimplementation must not reuse `x`.

<!-- from: From the terrain 3D view and the remaining `cISTETerrain` slots (2026-10-16) -->
- ~~**TR27:** does the surface water need its own message target (§10.1 subscriptions, §10.3's triggers) and its
  animation-tick registration at priority `0xC0`? [both yes]~~ **ANSWERED 2026-10-16 (re-answers30):** **MATCHES**
  §10.1 on both — a `cIGZMessageTarget2` at water `+0x08` (vtable `0x00AB3AD8`, `DoMessage` `0x00740160`)
  subscribing to exactly those six messages (`0x0073FF7F`–`0x0073FFC3`), and `AddTickableObject` at **priority
  `0xC0`** (`0x0074001E`); the guessed value is correct. **DIFFERS on reachability, and this is what glue18
  needs:** the tick body is gated on the water's byte **`+0x34`** (`0x007401E9`–`0x007401EE`) and **nothing ever
  sets it non-zero** — the only writes are the two `= 0` of the constructor and `Init`, and no slot exposes it. So
  `AnimateWater` (`0x0073FDD0`) and `UpdateFloodWaters` (`0x0073EF00`) are **dead** in the shipped binary (each has
  exactly one caller, the gated handler). **A surface water that does not animate is behaviour-compatible with the
  original**; animating would change water depths over time and would be a *divergence*, not a fix. Also
  **DIFFERS** on §10.3: the SimNewDay `0x66956814` trigger is **unreachable** — the handler exists (`0x0074019A`)
  but `Init` never subscribes to it and there is no wildcard — so the dirty flag `+0x50` is set by the power-pole
  handlers (`0x007402A4`) and never read. `0x26D31EC5` is subscribed and its handler does nothing.
- **TR38 - §15.5.8 / TR30:** the precondition of the edge-layer refresh `FUN_00755B60` that a replacement terrain
  breaks. Runtime evidence (gt3-repl-4): the first queued `0x278E6003` after `DoPostCityInit` (whole-city rect
  `(0, 0, 128, 128)`, data1 = data2 = `0x80`, a flat 300 m terrain, sea 250, 128 x 128 cells, `T+0x6C` a live
  `0x24`-byte array) faults in the view at `0x00755C2B` (ACCESS_VIOLATION writing `0x16B23FF8`; caller
  `0x00756AAC` in `FUN_007569F0`, under the view `DoMessage` `0x007575DE`). Registers: ecx `0x16B84C24`, edx
  `0xFFF9F3D0`, esi `0xFFF9FFE8` (edx - esi = -24 x 129), edi `0x4102`, ebx 2, eax 6, ebp `0x437A0000` (250.0f).
  The game itself posts whole-city heights redisplays (`SetSeaLevel`, `0x470BD214`, the grid toggle), so a
  whole-city rect alone is not the fault. Which input does `FUN_00755B60` index with (terrain altitudes, slot 29,
  terrain fields beyond §2.3, the edge records `0x64 + side`, or state set by §15.5.1 step 7's pair `0x00743B60` /
  `0x007490B0`, TR33), and what range must it be in? The replacement keeps the TR33 whole-city redisplay; it is
  the fault's trigger [choice: unchanged until answered]. What settles it: a byte walk of `0x00755B60`-`0x00755C47`
  naming the array written at `0x00755C2B` and the source of its index.
  **ANSWERED 2026-10-21 (re-terrain4):** DIFFERS — the fault is not a missing precondition of `FUN_00755B60`; the
  delivered message rectangle was out of range. `FUN_007569F0` loops `x = x0…x1`, `z = z0…z1` straight from
  data1/data2 with no clamp and calls `FUN_00755B60(side, k, alt, sea)` with `k = x`, `cellsZ − z`, `cellsX − x`, `z`
  for sides 0–3; the arrays hold `2n` elements and need `0 ≤ k < n`. The fault registers give `x = 0x4102`, `z = 0`,
  `side = 2`, `k = −16514`, `n = 129`: the view's statics were right and the loop ran to `x = 16642`, so the received
  data1 low half was ≥ `0x4102`, not 128. `Init` already copies `{0, 0, cellsX, cellsZ}` itself. Rule (terrain.md
  §15.5.12.1, §15.5.11 item 19): post each message on a fresh `cRZMessage2Standard` as helper `0x00743380` does,
  never reuse or rewrite a posted object, and keep every rectangle inside `[0, cellsX] × [0, cellsZ]`; the TR33
  whole-city post may stay.
- **TR40 - §15.5.5 slot 67 / §15.5.9:** who initialises the view's city-edge layer state (`0x00B4C760`,
  `0x00B4C790`, `InitializeCityEdgeLayerHeights` `0x00757300`) for a city, and what the terrain's slot 67
  `DoLighting` (`0x00742930`, called once from `DrawTerrainLayers` with `V+0x34`) must do. Runtime evidence
  (gt3-repl-5, the TR33 whole-city post switched off, slot 67 a no-op, no other change): the first draw faults at
  `0x007540A5` (ACCESS_VIOLATION reading `0x00000008`; edx 8, esi 0, ebx `0x81`; `0x00B4C760` twice on the
  stack; called from `0x0075BFA3` in `DrawTerrainLayers` under `Update` `0x0075C07E`). Together with TR38 this
  says the edge-layer state is not set up when a replacement terrain is in place: is it set by §15.5.1 step 7's
  `0x00743B60` / `0x007490B0` (TR33), by the original slot 67, or by a terrain field the view reads that §2.3 does
  not list? [choice: slot 67 a no-op, as §15 calls it render] What settles it: the callers of `0x00757300` and a
  description of `0x00742930`, `0x00743B60` and `0x007490B0`.
  **ANSWERED 2026-10-21 (re-terrain4):** DIFFERS — the gt3-repl-5 fault is not the edge-layer state and not slot 67.
  `DrawSideCliffs` binds texture id `0x11` from `table[level]` (`0x00B4C670[0x00B4C70C]`, per detail level); the
  table is `0x00B4BC44` entries long and filled for the ids in `0x00B4BCD8[terrainType]`. Those two statics are
  written only by `cSTETerrain::StaticInit` (`0x0074FE40`, from `cSTETerrain::Init` `0x0074FEAF`: `ReadTextureInfos`,
  `ReadTerrainIniFile` → `ReadMiscTexturesForTerrainTypes`, `ReadTuningParameters`, `ReadBrushDescriptions`). Without
  it `0x00B4BC44` = 0, the view allocates zero-length tables and reads past them. Rule (terrain.md §15.5.12.2,
  §15.5.11 item 18): run `0x0074FE40` (no `this`, idempotent) or reproduce those statics before the first view
  `Init`. The edge-layer arrays and depths are the view's own (§15.5.12.3). Slot 67 `DoLighting` lights record
  `+0x04` normals into `+0x10` through the lighting object `0x00B43DDC` slot 28 (§15.5.12.4).
- **TR39 - §13.1 slot 96:** who calls the terrain's `Load` (slot 96, `0x00747800`) on a city load, with which
  segment, and when relative to `Read`, `Init` and `PostCityInit`. In gt3-repl-4 the replacement's slot 96 was
  never called with a non-null segment (no Load log line; `Read` ran, then `Init`), so the replaced city ran on the
  §2 flat default instead of its saved altitudes. Is slot 96 called by `cSC4City` (which function, which
  interface pointer), or does the original terrain call `Load` itself from `Init` with a segment it obtains (from
  where)? [choice: the replacement only answers slot 96] What settles it: the call sites of `0x00747800`.
  **ANSWERED 2026-10-21 (re-terrain4):** DIFFERS — nobody outside the terrain calls slot 96. The terrain's own `Read`
  (`0x00745380`, serializable slot 4 at `base+8`) reads the u16 version (> 2 fails), reads the view reference with
  `SC4ReadGZSerializable(stream, 0xC9B84E10, &base[+0xE8], 0)`, gets the stream's segment with
  `SC4GetIGZDBSegment(stream)` (`0x0045BA70`) and calls `base->Load(segment)` (slot 96, `0x007453C7`), returning its
  AL. The only other entry is slot 95 `LoadObliterated`, a tail jump to slot 96. So the records load during `Read`,
  before `Init` and `PostCityInit` (terrain.md §13.2).
- **TR35 - §15.5.8 / §15.5.11 item 15:** who calls the view's `Shutdown` (`0x007560F0`, which unsubscribes the six
  messages) and when: the terrain's own `Shutdown` / `PreCityShutdown`, the view's last `Release`, or the app.
  The replacement only `Release`s its factory reference in the terrain's destructor [choice]. What settles it: the
  call sites of `0x007560F0`.
  **ANSWERED 2026-10-21 (re-terrain4):** DIFFERS — the terrain's own slot-4 `Shutdown` (`0x007435C0`, gated by
  `+0x0C`) calls the view's `Shutdown` (`0x007560F0`, `ECX` = view base, no arguments, `0x007437BE`) when `+0xE8` is
  set, then `Release`s the view (base slot 2), clears `+0xE8` and `+0xEC`. The view destructor `FUN_00756980` also
  calls it when the view's `+0x18` init byte is still set. A replacement does those steps in its `Shutdown`, not only
  a `Release` in its destructor (terrain.md §15.5.11 item 15a).
- **TR33 - §15.5.1 step 7:** what `0x00743B60` and `0x007490B0` do with `&terrain+0x40` (the whole-city vertex
  rect) right after `+0xEC` is set. The replacement does a whole-city `RedisplayTerrain(1, 1, rect, 0)` instead
  [choice], because §15.5.8 makes the two redisplay posts the view's only channel for heights and water. Is that
  the observable effect of the pair (and with which last argument), or do they also refresh textures/lighting?
  What settles it: a description of the two functions' effects.
  **ANSWERED 2026-10-21 (re-terrain4):** MATCHES in part, DIFFERS in effect: 0x00743B60 recomputes record normals and cliff flags 0x8/0x800 (slot-93 flips); 0x007490B0 updates water proximity, weather moisture, the three texture layers (+0x16..+0x21) and marks the view rows dirty. It posts nothing, so a whole-city RedisplayTerrain does not replace it. Rule in terrain.md 15.5.13.2-3.
- **TR34 - §15.5.1 step 6:** what object terrain `+0x100` holds and what its slot 5 (`+0x14`) does. §2.3's field
  table does not list `+0x100`, so the replacement does not call it [choice]. What settles it: the writer of
  terrain `+0x100`.
  **ANSWERED 2026-10-21 (re-terrain4):** DIFFERS: +0x100 is the city weather simulator (city slot 70, set in Init 0x00750064); step 6 calls its DoCompleteSimulation (slot 5, 0x007296F0, once-only). The replacement must make that call. Rule in terrain.md 15.5.13.1.
- **TR36 - §15.5.11 item 12:** the source of the vertex record's `+0x16` surface texture id that
  `ComputeTextureSpansForRow` and `DrawTerrainSurfaceRows` read. §2.2 gives the record layout but no writer of
  `+0x16`; the replacement leaves it 0 [choice]. Which terrain function computes it (from altitude/slope, the
  terrain type, or the brush texture accumulation `+0xC0`) and when? What settles it: the writers of record `+0x16`.
  **ANSWERED 2026-10-21 (re-terrain4):** DIFFERS: record +0x16/+0x1A/+0x1E are three {u16 id, u16 weight} texture layers written only through 0x007490B0 (clear 0x00740890, add 0x007405E0, normalise 0x00740690). Leaving them 0 draws texture 0. Rule in terrain.md 15.5.13.3; detailed selection rules are TR41.
- **TR37 - §15.5.8:** the terrain's hide/flip slots call the view's `SetTriangleIndicesForCell` (`0x00752AF0`)
  directly for the touched cell. Its calling convention and arguments (view base or `+0x0C`; `(x, z)` order) are
  not given, so the replacement does not call it [choice] and the view's index buffers only follow those slots
  at its next whole-grid rebuild. What settles it: the call at `0x007411E1` and the prologue of `0x00752AF0`.
  **ANSWERED 2026-10-21 (re-terrain4):** DIFFERS: thiscall ECX = view base (terrain+0xE8), stack args (z, x), ret 8; called synchronously after the flag write when +0xEC is set (flips only on change). Rule in terrain.md 15.5.13.4.
- **TR41 - 15.5.13.2/15.5.13.3:** the exact rules of `MarkCliff` (`0x00742FC0`: when it flips through slot 93 and which corners get flag `0x8`), of the base-texture pass `0x007482C0` (per terrain type: which ids, which response curves, which weights from altitude, slope, water depth and moisture) and of the brush-texture pass `0x00748950` (the `+0x1B0` list and its fade). 15.5.13 gives their inputs, order and the layer add/normalise rules; the replacement cannot yet reproduce the chosen ids and weights. What settles it: a full read of the three functions and the per-type tables `0x00B4B414`/`0x00B4B420`.
  **ANSWERED 2026-10-21 (re-terrain4):** DIFFERS: §15.5.13.6. MarkCliff 0x00742FC0 flips (slot 93, flag 0x800) only when both triangles are steep and the other diagonal is not all steep, else marks the steep triangles' corners 0x8; base pass 0x007482C0 = cliff id B414 w50, water 0x14 (<15) / 0x13, land B420[type][band 31-floor(30t)][moisture], low-dry B410 w1, each splatted by hash node curve/radius; brush pass 0x00748950 = id 0xE0 splat around flag-0x400 vertices faded by age/10000000; proximity grid 0x007440C0 = capped 4-neighbour distance.
<!-- from: re-terrain4 (run4 round 2, 2026-10-21) -->
