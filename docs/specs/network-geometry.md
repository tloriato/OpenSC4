# Network geometry (behaviour spec)

Status: M5 behaviour spec, written 2026-10-15 (re-specs15b lane). Every fact was checked on the Windows executable
`bin/SimCity4.exe` (1.1.641, image base `0x00400000`); every address is a Windows address. The Mac build's symbol
names were used to name the Windows functions (the Mac and Windows vtables agree slot for slot up to 50; Windows slot
51 is the deleting destructor and Windows slot n ≥ 52 is Mac slot n + 2). Numeric details (float or double, operation
order, constants) were checked against the instruction bytes. Implementers work from this spec, the public LGPL
gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled or disassembled code (see
[CONTRIBUTING.md](../../CONTRIBUTING.md)).

Tags:

- *[compat]*: observable; a replacement must match it.
- *[choice]*: a FreeCity4 decision.
- *[inferred]*: consistent with all the evidence, but not proven.
- *[quirk]*: original behaviour that looks unintended but is reproduced.

`r24(e)` is `e` rounded to float32 (foundations §18). The FPU runs at 24-bit precision, so every intermediate x87
result below is an `r24` value.

Scope. The geometry half of the network tool's `PlaceNetwork` pipeline (network-tools.md §5), which that spec names by
its passes and failure codes only:

- how a dragged line becomes **steps** and **cells** (`DrawNetworkLine`);
- the **validation passes** in order, and what each one cuts;
- the **tunnel** and **bridge** decisions;
- `CheckForReservedTiles`;
- the **cell infos** and their height ranges;
- `SolveNetwork` (piece choice) in outline;
- the **height solver**: the vertex-height constraint satisfier, `SmoothenNetwork`, the slope and smoothness limits;
- `CanTilesBeSupportedOnTerrain` and the **grading** of the terrain (`0x00644530`, terrain.md `data4` kind 4);
- **supports** and the network flags they set;
- the failure codes.

It also answers network-tools.md NT1–NT5 and network-manager.md NM3 (§15).

What is not here: the network rule files and the tile evaluators that choose a piece for a cell (named in §8 only);
highway intersection pieces; one-way resolution; the bridge models. The player's cursor and preview are out of scope.

Related specs (cited, not repeated):

- [network-tools.md](network-tools.md): the pipeline (§5), the commit (§6), auto-connection (§7), cost (§8), the
  power-line tool (§9), `DestroyOccupantsAndResolve` (§10), the underground refit (§11), the slot table (§14).
- [network-manager.md](network-manager.md): network types (§3.1), network flags (§5), `RemoveOrReplaceOccupant`
  (§7.4), the terrain handler (§7.6), `CreateConnectionNode` (§7.7).
- [terrain.md](terrain.md): the cell size and vertex grid (§2), `MaxAllowedHeight` (slot 24), `SetAltitudeAtVertex`
  (slot 50), `RedisplayTerrain` and the `data4` kinds (§8), `Begin`/`EndLongTerrainOperation` (slots 87, 88).
- [occupant-manager.md](occupant-manager.md): the cell queries used by the reserved-tile test.
- [foundations.md](foundations.md) §18: x87 precision.

---

## 1. Identity and slot tables

### 1.1 cSC4NetworkTool slots past the SDK (base vtable `0x00AA8748`)

These are class methods. A replacement needs them only to keep the original's layout *[choice]*. Names are the Mac
build's. Pops were read from each target's `ret`.

| Slot | Offset | Address | Method | Pop | Return |
|---|---|---|---|---|---|
| 51 | +0xCC | `0x0062CD30` | deleting destructor (flag argument) | 4 | EAX |
| 54 | +0xD8 | `0x00624210` | `CreateNetworkOccupant` (§10.1) | 16 | not checked |
| 56 | +0xE0 | `0x00644B20` | `CanTilesBeSupportedOnTerrain(type)` (§9.6) | 4 | AL |
| 57 | +0xE4 | `0x00644530` | `UpdateTerrainHeights(&changedVertices)` (§9.7) | 4 | — |
| 60 | +0xF0 | `0x00624920` | `DestroyHilightedOccupants` | 0 | — |
| 61 | +0xF4 | `0x00648F00` | `AllowBridges`: returns 1 | 0 | AL |
| 62 | +0xF8 | `0x00648F00` | `AllowTunnels`: returns 1 | 0 | AL |
| 63 | +0xFC | `0x00648F00` | `IsStoppedByWater`: returns 1 | 0 | AL |
| 64 | +0x100 | `0x00648EF0` | `ShouldAutoConnect`: returns 0 | 0 | AL |
| 65 | +0x104 | `0x00638680` | `TestForAutoConnectOccupant(x, z)` | 8 | AL |
| 66 | +0x108 | `0x00638710` | `IdentifyStartingConditions(&valid)` (§4.1) | 4 | AL |
| 67 | +0x10C | `0x006392F0` | `CheckForReservedTiles(x, z, type)`, one cell (§5) | 12 | AL |
| 68 | +0x110 | `0x00639170` | `CheckForReservedTiles(&valid, type, skipSteps)`, the pass (§5) | 12 | AL |
| 69 | +0x114 | `0x00638990` | `CheckForGoodTerrain(&valid)` (§4.5) | 4 | AL |
| 70 | +0x118 | `0x00407990` | `StartPlacing`: no-op | 0 | — |
| 71 | +0x11C | `0x00633250` | `CreateCellInfosForCurrentPlacementTiles(type)` (§7) | 4 | — |
| 73 | +0x124 | `0x006444F0` | `GetNetworkVertexHt(vertex)` | 4 | ST0 |
| 74 | +0x128 | `0x0062C990` | `GetNetworkHeightForTerrainHeight(h)` (network-tools.md §14) | 4 | ST0 |
| 75 | +0x12C | `0x00407990` | `PropagateSolvedCellsForSmoothing`: no-op | 0 | — |
| 76 | +0x130 | `0x006360C0` | `InsertRangeConstraintsForCell(cellKey)` (§9.3) | 4 | — |
| 77 | +0x134 | `0x00637B90` | `ApplyPlaneConstraintToRegularCells(&bridgeAndTunnelConstraints)` | 4 | — |
| 78 | +0x138 | `0x00648F10` | `InsertNetworkSpecificConstraintsForCell`: no-op | 4 | — |
| 79 | +0x13C | `0x00648F10` | `ReloadNetworkSpecificInfo`: no-op | 4 | — |
| 80 | +0x140 | `0x00623CE0` | `SetQuadTrianglesToDraw` | 8 | — |

The SDK-range slots already in network-tools.md §14 are not repeated. Mac names for some unnamed SDK-range slots:
10 `UpdateOnZoneChange`, 11 `ForceReload`, 12 `SetTraceFinal`, 16 `ClearCellPreviews`, 17 `ClearOccupantHighlights`,
23 `GetDraggedCells`, 25–29 the step getters, 30–40 the bridge getters and setters, 41 `GetPossibleNeighborConnection`,
46 `StopPickAtWater`, 47 `GetToolViewId`.

### 1.2 Overrides

| Slot | Pipe `0x00AA99A8` and subway `0x00AAA5C0` | Power line `0x00AA9F30` |
|---|---|---|
| 51 | `0x00648F50` (destructor) | `0x00650560` (destructor) |
| 54 | `0x00657740` | base |
| 55 | `0x006599D0` `InsertOccupantsForSolvedCells` | base |
| 56 | `0x004FC900`: returns 1 (no terrain support test) | base |
| 57 | `0x00648F10`: no-op (**underground tools never grade the terrain**) | base |
| 58 | `0x006597E0` `AddCellPreview` | base |
| 59 | `0x00648F30`: returns its argument (no demolition highlights) | base |
| 60 | `0x00407990`: no-op | base |
| 61, 62, 63 | `0x00648EF0`: return 0 (no bridges, no tunnels, not stopped by water) | base |
| 64 | `0x00648F00`: returns 1 (auto-connects) | base |
| 65 | `0x00659470` | base |
| 68 | `0x00657730`: no-op, pop 12 (**no reserved-tile pass**) | base |
| 69 | `0x004FC900`: returns 1 (any terrain is good) | base |
| 70 | `0x00658710` `StartPlacing` | base |
| 71 | `0x00657DA0` `CreateCellInfosForCurrentPlacementTiles` | base |
| 73 | `0x00657790` | base |
| 74 | `0x00648F20`: `h − depth` (`+0x36C`) | base |
| 75 | `0x00659410` `PropagateSolvedCellsForSmoothing` | base |
| 76 | `0x00658040` `InsertRangeConstraintsForCell` | base |
| 77 | `0x00648F10`: no-op | base |
| 78 | `0x00657860` `InsertNetworkSpecificConstraintsForCell` | base |
| 79 | `0x00657980` `ReloadNetworkSpecificInfo` | base |
| 80 | `0x00657A30` | base |
| 81 | occupant class (`0x00648F40` → `0x49C05B9F`; `0x00653D90` → `0xCA16374F`) | `CheckForReservedTiles(&valid)` `0x0064FED0`, pop 4 |
| 82 | `0x00648D10` / `0x00653A00` (unnamed) | `SetTerrainHilight` `0x0064F2B0` |

---

## 2. Data

### 2.1 Tool fields used here

Offsets from the tool object; network-tools.md §2 has the rest.

| Offset | Meaning |
|---|---|
| `+0x54` | vector of **dragged steps** (12 bytes each, §2.2) |
| `+0x60` | vector of **dragged cells** (8 bytes each: x, z) |
| `+0x6C` | the major-axis flag from `BreakIntoStraightAndDiagSegments` (§3.2) |
| `+0x70` | dragged step count |
| `+0x74` | straight step count |
| `+0x78` | **valid** step count |
| `+0x7C` | pen direction: 0 = −x, 2 = −z, 4 = +x, 6 = +z |
| `+0x80` | end direction: 1, 3, 5 or 7 (§3.2) |
| `+0x84` | hash set of cell keys to solve (`BuildSetOfCellsToBeSolved`) |
| `+0x98` | vector of solved cells |
| `+0xA4` | hash set of cell keys to **smooth** (§9.4) |
| `+0xF0`–`+0xF4` | vector of **extra cells**, 20 bytes each: x, z (i32), an edge word (u32), a network type (i32), two bytes. Filled only by the intersection tool's auto-complete (§4.5, NG3) |
| `+0x124` | the **network** vertex-height satisfier (§9.1) |
| `+0x188` | the **terrain** vertex-height satisfier (§9.6) |
| `+0x238` | the cell size (terrain slot 6; 16.0) |
| `+0x23C` | the **tunnel trigger rise** per step: `tan(p+0x18) × +0x238` |
| `+0x240`, `+0x244` | city size in cells, X and Z |
| `+0x248`, `+0x24C` | vertex counts X + 1 and Z + 1 (`+0x248` is the vertex row stride) |
| `+0x250`, `+0x270` | per-direction vertex offset tables (§6.1) |
| `+0x290` | the tool's copy of the placement parameters (§2.4); tool offset = `0x290` + parameter offset |
| `+0x2A4` | p+0x14, the re-solve tolerance of an existing cell |
| `+0x2AC` | p+0x1C, the tunnel end slope (radians) |
| `+0x2B0` | p+0x20, the **plane tolerance** of regular cells (§9.8) |
| `+0x2B5` | p+0x25, byte **straight only** (street) |
| `+0x2C4` | p+0x34, the new-cell tolerance |
| `+0x2C8` | p+0x38, the support height threshold |
| `+0x2FC` | p+0x6C, vector of floats: the tunnel clearance |
| `+0x320`, `+0x324`, `+0x328` | p+0x90, p+0x94, p+0x98: bridge cost per cell, tunnel cost per cell, earthwork cost (NT2) |
| `+0x330` | p+0xA0, the tool's copy of the id list (§2.4); never read (NG6) |
| `+0x354` | byte: land bridges (network-tools.md §2) |

A **cell key** for the tool's own sets and cell-info lookups (`GetCellInfo`) is `(z << 16) + x`. A **vertex id** is
`z × +0x248 + x`.

### 2.2 tDraggedStep (12 bytes)

`{first cell index, last cell index, kind}`; the indexes are into `+0x60`. A step is the row of cells across the pen
axis at one position along the drag. Kinds:

| Kind | Meaning |
|---|---|
| 0 | regular |
| 1 | bridge (including the two abutment steps) |
| 2 | tunnel portal |
| 3 | tunnel interior |

### 2.3 Network type info (`sNetworkTypeInfo`, base `0x00B452C8`, stride `0x114`, 13 records)

Type `t`'s record is at `0x00B452C8 + t × 0x114`.

| Offset | Meaning |
|---|---|
| `+0x1C`, `+0x20` | the two **widths** in cells: 2, 2 for highway, avenue and ground highway; 1, 2 for power line; 1, 1 otherwise |
| `+0x24` | vector of tile evaluators (§8) |
| `+0x30` | vector of bridge types (`0x178` bytes each) |
| `+0x3C` | hash map piece id → **support structure info** (§10.2, NG6) |
| `+0x50` | the **placement parameters** (§2.4) |
| `+0x100` | `r24(tan(p+0x00) × 16)`: the straight **slope** rise per cell |
| `+0x104` | Binary32 store of the still resident x87 straight-slope intermediate × binary32 0.70710677: the diagonal slope rise |
| `+0x108` | `r24(tan(p+0x08) × 16)`: the straight **smoothness** limit |
| `+0x10C` | Binary32 store of the still resident x87 straight-smoothness intermediate × binary32 0.70710677: the diagonal smoothness limit |
| `+0x110` | `r24(tan(p+0x04) × 16)`; written only by `0x0063D460` and **never read** (NG6) |

These five fields are computed by `0x0063D460` after calling the parameter loader, even if it reports failure,
with x87 `fptan` and binary32 constants 16.0 (`0x00A8D45C`) and 0.70710677 (`0x00AA8C50`). The straight
store does not pop/reload its intermediate before diagonal multiplication (`0x0063D47F–0x0063D48B`,
`0x0063D49E–0x0063D4AA`). Do not infer mandatory intermediate rounding from the stored straight field.
Constructor values, Clear/reload preservation, property fallback corrections and precision bounds are in
[network-type-info.md §§2–5](network-type-info.md) *[compat]*.

### 2.4 The placement parameters

`LoadPlacementParams` (`0x0063CBE0`) reads one exemplar per type: type `0x6534284A`, group `0x084344E0`, instance
below. The exemplars have no parent cohort. Power line (5) has none; its parameters stay at the defaults. Angles are
read in degrees and stored in radians, multiplied by the float constant 0.017453292 (`0x00AA8C60`).

| Type | Instance | Type | Instance |
|---|---|---|---|
| 0 Road | `0x0781214F` | 7 Subway | `0x0A367CA7` |
| 1 Rail | `0x0814E612` | 8 Light rail | `0xCB79E089` |
| 2 Highway | `0xA8433F6B` | 9 Monorail | `0xEBE08882` |
| 3 Street | `0x492A03A3` | 10 One-way road | `0x8BDA0C2B` |
| 4 Pipe | `0x095117DB` | 11 Dirt road | `0xEBE08CDA` |
| 6 Avenue | `0x2B730A17` | 12 Ground highway | `0x0BE08C87` |

Fields that this spec uses (p = parameter offset; "—" = absent, default applies):

| p | Property | Meaning | Road | Rail | Highway | Street | Pipe | Avenue | Subway |
|---|---|---|---|---|---|---|---|---|---|
| `0x00` | `0x879D12E7` | max slope (deg) | 35 | 25 | 20 | 40 | 45 | 40 | 35 |
| `0x04` | `0x279D1707` | (deg), feeds `+0x110` | 40 | 30 | 30 | 40 | 40 | 30 | 40 |
| `0x08` | `0x07B36C71` | max change of slope (deg) | 35 | 25 | 20 | 40 | 45 | 40 | 35 |
| `0x0C` | `0xC7B36CA4` | max terrain **raise** under the network (m) | 25 | 25 | 50 | 50000 | 0 | 50 | 0 |
| `0x10` | `0xC7B36CA5` | max terrain **cut** under the network (m) | 10000 | 10000 | 10000 | 10000 | 45 | 10000 | 35 |
| `0x14` | `0xC804DD86` | re-solve tolerance of an existing cell (m) | 10 | 101 | 101 | 500 | 500 | 101 | 300 |
| `0x18` | `0xC804DD87` | tunnel trigger slope (deg) | 35 | 25 | 20 | 89.5 | 89.5 | 40 | 89.5 |
| `0x1C` | `0xC804DD88` | tunnel end slope (deg) | 40 | 25 | 20 | 40 | 45 | 40 | 35 |
| `0x20` | `0xC804DD89` | plane tolerance (m), §9.8; stored as read, not converted | 35 | 15 | 15 | 15 | 15 | 15 | 15 |
| `0x34` | `0xC804DD93` | new-cell tolerance (m) | 500 | 180 | 180 | 50000 | 40 | 180 | 40 |
| `0x38` | `0xC804DD94` | support height threshold (m) | 45 | 10 | 10 | 10 | 10 | 10 | 10 |
| `0x3C` | `0xC804DD95` | max height above the graded terrain with supports (m) | 55 | 30 | 30 | 30 | 30 | 30 | 30 |
| `0x6C` | `0xE974EAA6` | tunnel clearance (m; default 10.0) | 13.8 | 7.25 | 23.5, 23.5 | — | — | 7.5, 7.5 | — |
| `0x8C` | `0x4A2D49B9` | cost per cell (default 1) | 10 | 8 | 300 | 5 | 5 | 30 | 150 |
| `0x90` | `0x8A2D49EA` | bridge cost per cell (default 10) | 100 | 100 | 1000 | — | — | 1000 | — |
| `0x94` | `0x8A2D49F0` | tunnel cost per cell (default 20) | 20 | 20 | 400 | — | — | — | — |
| `0x98` | `0x6A2D49F4` | earthwork cost per metre (default 0.1) | 0.1 | — | — | — | — | — | — |
| `0xA0` | `0xE974EAA7` | id list; non-empty only for highway, light rail and monorail. Only its emptiness is read (§9.2, NG6) | — | — | 1 id | — | — | — | — |

The other types: light rail and monorail are like rail (slopes 25/30/25, raise 25, cell cost 50 and 100, tunnel cost
150, clearance 7.25 and 22.4; light rail's tunnel trigger is 89); one-way and dirt road are identical to road; ground
highway is like highway (cell cost 100, tunnel cost 200, clearance 11.25, 11.25). The plane tolerance p+0x20 is 35 for
one-way and dirt road and 15 for light rail, monorail and ground highway. Power line has no exemplar, and the type info
constructor (`0x00641F70`) never writes p+0x20, so its plane tolerance is 0 (zero-initialised static storage).

p+0x25 is not read from the exemplar. It is 0, except the street's (`0x00B45679`), which is 1: the **straight only**
flag. Properties `0xC804DD91` and `0xC804DD9A` are shipped but never read.

---

## 3. From a drag to steps and cells

### 3.1 DrawNetworkLine(&start, &end, snap, type) (slot 18, `0x0063AF40`, pop 16, AL)

The third argument is a **snap** flag (network-tools.md §14 called it `useCache`; corrected there).

1. Clear the steps and the cells.
2. `BreakIntoStraightAndDiagSegments` (`0x00637EB0`, §3.2) with the type's two widths. It may move `end`.
3. If straight-only (`+0x2B5`) and the dragged count ≠ the straight count: return **false**.
4. `ComputeDraggedCells` (`0x00639790`, §3.3).
5. For a type wider than one cell: if any dragged cell is outside the city (x ≥ `+0x240` or z ≥ `+0x244`), clear and
   return **false**.
6. Return true.

### 3.2 BreakIntoStraightAndDiagSegments *[compat]*

Inputs: start (x1, z1), end (x2, z2), snap, the widths w1 = type `+0x1C` and w2 = type `+0x20`. Let dx = |x2 − x1|,
dz = |z2 − z1|.

1. **Pre-adjust** (always; the first matching rule):
   1. dx = 1 and dz > 2: x2 := x1, dx := 0.
   2. dz = 1 and dx > 2: z2 := z1, dz := 0.
   3. dx = dz + 2 and dz ≠ 0: move x2 one cell towards x1; dx := dz + 1.
   4. dz = dx + 2 and dx ≠ 0: move z2 one cell towards z1; dz := dx + 1.
2. **Straight only**: if dz < dx, z2 := z1 and dz := 0; else x2 := x1 and dx := 0.
3. Otherwise, if **snap**:
   1. 2·dz < dx: z2 := z1, dz := 0.
   2. else 2·dx < dz: x2 := x1, dx := 0.
   3. else make it diagonal. If dx > dz: dx := dz + 1 when w1 = 1, else dx := dz. If dz > dx: dz := dx + 1 when
      w1 = 1, else dz := dx. If equal, nothing changes. Then end := start + (sign·dx, sign·dz), each sign pointing
      from start to the original end.
4. **Counts.** If dx > dz: dragged count := dx + 1, and the axis outputs are (0, 1). Otherwise dragged count := dz + 1
   and (1, 0).
5. **Pen direction** (`+0x7C`): if dz < dx, 0 when x2 ≤ x1, else 4. Otherwise 6 when z1 < z2, else 2. Here and in
   step 6, x2 and z2 are the end as passed in, before any adjustment.
6. **End direction** (`+0x80`): if x2 ≤ x1, 7 when z1 < z2, else 1. Otherwise 5 when z1 < z2, else 3.
7. **Straight count** (`+0x74`): dz + 1 if dx = 0; dx + 1 if dz = 0; else |dx − dz|, except that it is 0 when w2 = 1,
   |dx − dz| = 1 and the dragged count > 1.

A width-1 network snapped to "diagonal" therefore has one extra cell along the major axis *[compat]*: the diagonal
staircase is drawn as (minor + 1) steps along the major axis.

### 3.3 ComputeDraggedCells (`0x00639790`)

1. Walk the pen from start to end, always along the axis with the larger remaining delta, and append one step per
   position. A step's cells are the row across the pen axis; a width-1 network has one cell per step.
2. Networks wider than one cell are widened across the pen axis, and their cells are snapped to an existing network of
   the same type through the placed-network filter (edge flags `0x4000400`, `0x20000`, `0x4040404`) *[inferred:
   alignment with an existing parallel run]*.
3. A width-1 network with ≥ 3 steps (or a 2-step wholly straight drag) can extend at **both ends**, including on a
   wholly straight drag. Each end requires an in-city candidate, `ShouldAutoConnect` (slot 64), and a successful
   `TestForAutoConnectOccupant` (slot 65). **Slot 67 is not part of this decision.** See §3.3.1.

### 3.3.1 Underground extension and the observation boundary (NG8)

*[compat]* Pipe and subway return true from slot 64. Their slot-65 override (`0x00659470`, two signed cell
coordinates, pop 8, AL) accepts an occupant of the tool's **own underground layer**, not any occupied tile.
The layer discriminator is slot 81: pipe `0x49C05B9F`, subway `0xCA16374F`. First enumerate occupants overlapping
the one standard-city cell: occupant-manager slot 40, `GetOccupantsByStandardCityCells(list, &cell, &cell,
null filter, 0xFFFFFFFF)`. Both bounds are the same pair of signed 32-bit coordinates. This is an unfiltered
spatial query, not a surface-only query, a network-type filter or an occupant-manager-cell query. Its bounding
box and ordering contract are [occupant-manager.md](occupant-manager.md) §8.4; do not replace it with an
occupant's declared home-cell equality test. Query each returned occupant for the layer discriminator. Any
successful QI makes this first search succeed (`FindUndergroundOccupant`, `0x006584D0`).

Only if that search finds no matching interface, enumerate the same cell again with the same arguments.
QI each result for **`0x8A139184`**, the underground-connection interface. Accept it when its main slot 38
(+0x98, no arguments, u32 in EAX) equals the tool's layer discriminator. The pipe/subway connection getters
are `0x00648F40` / `0x00653D90`; these return matching **occupant class IDs**, not network numbers 4/7,
connection class IDs or `GetType` values. See [underground-occupants.md](underground-occupants.md) §4.
An unattached matching connection is sufficient: no attached-occupant, edge, orientation, height, immovable,
connection-node-flag or model-readiness test is added (`0x00658690`–`0x006586D9`).

Both helpers visit the complete returned list and retain the **last** acceptable interface; replacement releases
the preceding accepted reference. Release every enumerated occupant reference, every rejected connection QI,
and finally the accepted QI after the predicate consumes it. A first-search hit skips the second enumeration
(`0x006594A2`–`0x006594BA`). The list storage is caller-owned and must also be destroyed. An unavailable manager
or failed host observation is unavailable, not evidence that the tile is empty. A complete successful empty
enumeration makes the predicate false. These public occupant calls suffice to reproduce the predicate without
reading tool fields; they do not expose the tool's solved inventory.

**Which ends and what changes.** The start end is considered before the finish end, with the latter using the
updated step vector. For a wholly straight drag use the displacement from the second step's first cell back
to the first step's first cell as the outward start direction. Otherwise use the second and third steps'
first cells. Add that displacement to the first step's first cell for the candidate. At the finish, use
last minus penultimate first-cell coordinates for wholly straight, or penultimate minus antepenultimate for
the other case; add it to the last step's first cell. Candidates must satisfy unsigned x<city-width and
z<city-height, so negative coordinates are rejected before occupant queries. The qualifying drag-length
test is performed once, before either end extension (`0x0063A1A1`–`0x0063A1C9`).

An accepted end inserts an outward translated copy of that end's step and increments total steps. Normally
straight steps also increment. With straight-prefix zero, straight steps remain zero; if the terminal step
is a singleton, move that singleton to the candidate and insert the translated adjacent step inside it.
Thus a diagonal extension can add two cells while adding only one step. New step kind is copied from the
first step. Existing steps' cell indices shift to retain their original cells. Start/finish anchor arguments
are not rewritten by this extension. Evidence: start geometry/gates `0x0063A20C`–`0x0063A2C8`, start count
updates `0x0063A3CE`–`0x0063A3D9`, finish geometry/gates `0x0063A3DC`–`0x0063A4AB`, finish insertion/count
updates `0x0063A4B1`–`0x0063A60B`.

**Reserved cells are a separate concern.** Slot 67 returns true for a reserved cell (§5.1), but neither end
gate calls it, positively or negatively. Underground overrides slot 68 (the later reserved-tile pass), not
slot 67. Consequently an otherwise reserved cell does not veto a matching pipe/subway extension *[quirk]*.
The base tool's slot 64 is false; the width-one branch does not bypass that gate for road. The wider-network
alignment logic is a separate operation. This corrects the former §3.3 sentence combining those operations.

**Conformance vectors.** Use an in-bounds 128-cell city, a fresh pipe tool, snap=false, and draw `(4,4)` to
`(6,4)` unless stated otherwise. All unlisted candidate-cell enumerations are complete and empty. Initial
cells are `(4,4),(5,4),(6,4)`, straight=total=3. Each listed predicate success is the public QI rule above.

| Fixture | Expected ordered cells / counts or predicate result |
|---|---|
| Empty candidates `(3,4)` and `(7,4)` | Original three cells; straight=total=3; predicate false at each end |
| Pipe occupant QI succeeds at `(3,4)` | `(3,4),(4,4),(5,4),(6,4)`; straight=total=4; no second enumeration at `(3,4)` |
| No pipe QI, connection QI succeeds and slot 38=`0x49C05B9F` at `(7,4)` | `(4,4),(5,4),(6,4),(7,4)`; straight=total=4; two enumerations at `(7,4)` |
| Matching connections at both candidates | `(3,4)` through `(7,4)` in ascending x; straight=total=5 |
| Only subway QI or connection slot 38=`0xCA16374F` at `(7,4)` | No extension for pipe; with subway tool, finish extends to `(7,4)` |
| Landfill plus matching pipe at `(7,4)` | Finish extends; a separate slot-67 query returns reserved=true |
| Two matching pipe QIs A then B at one candidate | Predicate true; release A when B replaces it, then B after consumption; release both list references |
| One-cell draw `(4,4)` to itself, matching occupants next to it | No extension and no candidate queries; straight=total=1 |
| Two-cell straight draw `(0,4)` to `(1,4)`, empty finish candidate | Start candidate `(-1,4)` rejected without query; cells unchanged; straight=total=2 |

Runs 512/513's type-4 `(16,44)`–`(10,40)` observation remains native/core cells 12/10, straight 0/0,
total 7/6 for both snap modes. A dimension-only adapter supplies neither occupant search. The native totals
do not identify which candidate matched, and must not be substituted for the missing observations. Re-run
that matrix with these occupant inputs before claiming narrow-drag exactness; this answer adds no game verdict.

---

## 4. The validation passes

`PlaceNetwork` (network-tools.md §5.1 step 4) runs these in order. Each takes the valid step count `v` (initially the
dragged count) by reference and may lower it. A pass that lowers `v` sets the failure code, and the run is then
refused (`+0x78 := 0`). The slot passes are virtual, so the subclasses' overrides (§1.2) apply.

| # | Pass | Code |
|---|---|---|
| 1 | slot 66 `IdentifyStartingConditions` | `0xF0000001` |
| 2 | `IdentifyTunnelCells` (`0x00639490`, §6) | `0xF0000004` (never set; §6.3) |
| 3 | slot 68 `CheckForReservedTiles` (§5) | `0xF0000002` |
| 4 | slot 59 `UpdateOccupantHighlights` (network-tools.md §6) | `0xF0000002` |
| 5 | `IdentifyBridgeCells` (`0x00632830`, §7) | `0xF0000003` |
| 6 | slot 69 `CheckForGoodTerrain` | `0xF0000001` |
| 7 | `IdentifyLandBridgeCells` (`0x00632AD0`), only if `+0x354` | none |

### 4.1 IsTerrainOK(x, z)

True, unless the terrain says the cell has water (terrain slot 34 `CellHasWater`). A water cell is OK only if it has
a cell info that is a **connection node** (cell info `+0x56`).

### 4.2 IdentifyStartingConditions (slot 66)

If `IsStoppedByWater` (slot 63): test the **first** dragged cell with `IsTerrainOK`. If it fails, `v := 0`. A road
cannot start in water.

### 4.3 Tunnels, reserved tiles, highlights, bridges

§6, §5, network-tools.md §6 and §7.

### 4.4 IdentifyLandBridgeCells

Runs only for types whose `+0x354` byte is set. It marks bridge steps over land. It never lowers `v`. Its internals
are in [network-bridges.md](network-bridges.md) §7 (NG4, answered).

`Init` (`0x006269C0`) clears `+0x354` and then, for each bridge type of the network type (type info `+0x30`), tests
the address of a field inside that bridge type record (`0x00626B53`). The address is never null, so **`+0x354` is
set exactly when the type has at least one bridge type** *[quirk: the test was presumably meant to read the field]*.
The field is bridge type `+0x98` (property `0x482CB44F`, a land bridge type); every shipped bridge type sets it, so the
quirk has no shipped effect (network-bridges.md §2.5).

### 4.5 CheckForGoodTerrain (slot 69)

Only if `IsStoppedByWater`; otherwise it returns true at once.

1. For each step `i < v` of kind 0, for each of its cells: if the cell has water and is not a connection node, then
   `v := i` and stop.
2. For each **extra cell** (`+0xF0`): continue while the cell has water and is not a connection node. At the first
   extra cell that is dry, or is a connection node, `v := 0` *[quirk]*. The test is the reverse of step 1's
   (`0x00638A50`–`0x00638A91`).

**Who fills the extra-cell list** (NG3). Only `0x00629170` appends to `+0xF0`, and its only caller is the
intersection rule's auto-complete (`cIntRule::DoAutoComplete`, `0x006097E0`, call at `0x00609962`). That runs only
from the intersection tool (`cSC4ViewInputControlNetworkIntxTool`: `OnMouseDownL` `0x00661078`, `OnMouseMove`
`0x00661657`). In both, the tool is reset (tool slot 5; `Reset` `0x00626F90` empties the list at `0x0062706B`), the
rule's auto-complete appends one extra cell per missing neighbour piece of the rule, and
`InsertIsolatedHighwayIntersection` (`0x0062CD50`) places the intersection; the tool is reset again afterwards.
`InsertIsolatedHighwayIntersection` uses the extra cells as additional cells to place (those that are not connection
nodes, `0x0062CFEE`) and runs `CheckForReservedTiles` (slot 68, `0x0062D25C`), but it never calls slot 69. A drag never
has extra cells, so step 2 and its quirk cannot be reached in the shipped game. A replacement may keep the step as
written; it has no observable effect.

Bridge and tunnel steps are skipped, so their water cells pass.

---

## 5. CheckForReservedTiles

### 5.1 One cell: slot 67 `(x, z, type)` (`0x006392F0`)

Query the occupants of the cell (occupant manager slot 40). The cell is **reserved** if any of them is:

- a **landfill** occupant (type `0x6A11BD2B`); or
- a **network** occupant (`0xC772BF98`) that is immovable (network occupant slot 17) and is either not a connection
  node (flag `0x200000`) or not of this type (network occupant slot 22).

Buildings (`0x278128A0`) are skipped here; the highlight pass decides about them (network-tools.md §6).

### 5.2 The pass: slot 68 `(&v, type, skipSteps)` (`0x00639170`)

1. Unless `skipSteps`: for each step `i < v` that is not kind 3 (tunnel interior), for each cell: if reserved, `v := i`
   and stop.
2. For each extra cell, tested with its own type: if reserved, `v := 0`.

Underground tools have no reserved-tile pass (slot 68 is a no-op). The power-line tool's slot 81 is
network-tools.md §9.

---

## 6. Tunnels

### 6.1 Step heights

A step's **mean height** on an edge is the mean of four terrain vertex altitudes (terrain slot 12): the two corners of
the edge of the step's first cell and of its last cell. The edge corners come from two tables of vertex offsets
indexed by direction `d` (built in `Init`; S = the stride `+0x248`):

| d | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|---|---|---|---|---|---|---|---|---|
| `+0x250[d]` | S + 1 | S | S | 0 | 0 | 1 | 1 | S + 1 |
| `+0x270[d]` | 1 | 1 | S + 1 | S + 1 | S | S | 0 | 0 |

The **trailing** edge uses `d` = the pen direction; the **leading** edge uses `(d − 4) & 7`.

### 6.2 IdentifyTunnelCells (`0x00639490`)

Needs `AllowTunnels` (slot 62) and `v > 3`.

1. `start height` := the trailing-edge mean of the first regular step.
2. For each step i from 2 while i < v: if steps i, i−1 and i−2 are all regular and i < the straight count, and
   `mean(i) − mean(i−1) > +0x23C` (strictly), then look for the tunnel's end (`FindTunnelEnd`, `0x00638770`) from
   start step i − 1 with that step's height.
3. **FindTunnelEnd.** Walk the steps from start + 1 while the step is < v − 1, is regular and is < the straight
   count:
   - stop (no tunnel) at any step with a water cell;
   - use the leading-edge mean;
   - remember the step whose mean is lower than the previous step's mean and is closest to the start height;
   - keep walking while the mean is above `start height + +0x2FC[0]` (the clearance).

   The end is accepted if `|end height − start height| ≤ tan(+0x2AC) × +0x238 × length` and length > 2.
4. If an end was found and `end − start > 3`: record the tunnel; the start and end steps become kind 2 (portals) and
   the steps between become kind 3.

With the shipped data, street, pipe and subway have a trigger slope of 89.5° (light rail 89°), so they effectively
never tunnel *[compat]*.

### 6.3 The tunnel failure code is never set *[quirk]*

`IdentifyTunnelCells` reads `v` but never writes it, so `PlaceNetwork` never sets `0xF0000004` ("a tunnel needs a
straight drag"). A non-straight drag simply gets no tunnel (step 2 needs i < the straight count).

---

## 7. Bridges

`IdentifyBridgeCells` (`0x00632830`). Needs `AllowBridges` (slot 61), `v > 3`, a **wholly straight** drag (`v` ≤
straight count) and at least one bridge type for the network type (type info `+0x30`). The full behaviour is in
[network-bridges.md](network-bridges.md); this section is the summary.

For each step i ≥ 2 such that steps i, i−1 and i−2 are regular:

1. If the step is good for a bridge (no placed network occupants in the step's rectangle *[inferred]*), one of its
   cells has water, and its edge types allow a bridge: choose the span (`SelectBridgeStartAndEnd`):
   - start := i − 1;
   - end := the first step after the water whose cells are all dry;
   - every step between must be good for a bridge with acceptable edge types, and the end must be < v − 1 and < the
     straight count.
2. Then `AdjustBridgeSpan` and `FindBestBridgeType`.
3. If the span fails or no bridge type fits: `v := i` (the run is cut here, code `0xF0000003`) and stop.
4. Otherwise, with `span := end − start`: if `span > 3` and the bridge type's minimum span (bridge type `+4`) ≤
   span + 1 (`0x006329C1`–`0x006329D4`): mark steps start … end kind 1, append a bridge record to the tool's vector
   `+0xD8` (`0x28` bytes: network type, start, end, bridge type index, the elevation value from the span selection at
   `+0x14`, a byte at `+0x18` cleared), choose its elevation (`0x0062EA90`), then look the step count `span + 1` up in
   the bridge type's map at **`+0x16C`** (`0x00632A68`; an earlier text said `+0x164`). Found: a **specialised**
   bridge from the map's entry (`0x00632060`); not found: a **parametric** bridge (`0x00631300`). A span that fails
   this test records nothing, and the loop goes on with the next step.

A crooked drag across water therefore never gets a bridge, and the later terrain pass (§4.5) cuts it at the first water
cell with `0xF0000001` *[compat]*.

### 7.1 The bridge internals (NG2)

The span selection, the elevation and the two bridge models are specified in [network-bridges.md](network-bridges.md)
(§4–§11). The entry points:

| Part | Address | Notes |
|---|---|---|
| step good for a bridge | `0x0062FDB0` | called with the tool's `+0x364` object and the step's first and last cells |
| step edge types | `0x0062EB80` | `(step, type)`; reads the terrain (slot 34) per cell |
| span selection | `0x00630570` | `(step, &start, &end, &bridge type index, &elevation, type)`; calls `0x0062FDB0`, `0x0062EB80`, `0x0062FED0` and `0x0062F760` (the adjust and best-type steps *[inferred]*) |
| elevation | `0x0062EA90` | on the new bridge record |
| specialised model | `0x00632060` | the `+0x16C` map entry's data at `+0x14` |
| parametric model | `0x00631300` | uses the bridge type's offsets around `+0xF8`–`+0x120` |
| pieces | `InsertBridgePieces` `0x0062F610`, `InsertBridgePiece` `0x0062EEB0`, `AddSuspendedBridgeStructures` `0x0062DF30`, `RemoveBridgePiecesFromSetOfCellsToBeSolved` `0x00638B40` | |
| loading | `LoadBridgeTypes` `0x006437A0` (road: `"RoadBridgeExemplars"`), `ReadBridgeTypeInfo` `0x00642220`, `ReadBridgeTypeSpecializations` `0x006436D0` with the parser at `0x00643580` | the specialisations are keyed by name (hash map `0x00640E10`) |
| occupant | `cSC4NetworkBridgeOccupant` (`0x00604BB0` onwards) | |

The tool's `Init` also derives `+0x354` from the bridge types (§4.4).

---

## 8. The solve (piece choice) in outline

After the passes, if `v > 0`, `PlaceNetwork` pushes the world cache and runs (network-tools.md §5.1 step 5):

1. **Cell infos** (slot 71, §8.1).
2. `AssignEdgeMajorTypes`, the grouping info for wide types, `IdentifyHighwayIntersections` (§8.2),
   `BuildSetOfCellsToBeSolved` (the set `+0x84`, `0x006355C0`), `ResolveOneWayConflicts` (one-way road only),
   `MarkNeighborConnections`. `BuildSetOfCellsToBeSolved` ends by taking three kinds of cell out of the set: the
   highway-intersection cells (`0x0063B1A0`, network-rules.md §10.6), the tunnel cells (`0x00638BB0`: every cell of
   the steps from each tunnel record's start step to its end step, stopping at the first record whose end step is
   ≥ `+0x78`) and the bridge cells (`0x00638B40`,
   network-bridges.md §8.3). The piece solver never chooses pieces for them; their pieces come from their own
   inserters.
3. `SolveNetwork(type, resolve)` (`0x00635CB0`), returning a valid count:
   1. Pad the solve set by 2 cells. Cells whose edges get special subtypes are taken out of the set for now.
   2. Multi-type intersections are solved first and removed.
   3. For each of the 13 types, each tile evaluator of the type (type info `+0x24`, until a null entry) solves what it
      can (`SolveUsingEvaluator`, which applies the network rules).
   4. The removed edge-subtype cells go back into the set.
   5. If cells are left unsolved (and this is not a re-solve): the result is the smallest step index of an unsolved
      cell that is part of the run; an unsolved cell outside the run lowers the result by one, once. Otherwise
      `AdjustTileSubsets` and `VerifyTileSubsets` must both pass, or the result is `+0x78 − 1`.
   A result below `+0x78` fails the run with `0xF0000007`.
4. The **smoothing loop** (§9.4) with the terrain test (§9.6).

On any failure `+0x78 := 0` and the world cache is popped.

### 8.1 Cell infos (`cSC4NetworkCellInfo`, `0xBC` bytes)

**Existing occupant selection (NG16).** `ComputeAndStoreCellInfo` (`00633030`) calls the base tool's
`GetNetworkOccupant(x,z,toolFilter,&network)` (`00624B70`). The filter is tool `+0x364`, the OR filter made
from layer mask `+0x360`: `1F4F` for surface tools and the own-type bit for pipe, power line and subway
(network-tools.md §2). The helper checks x/z against unsigned city width/height, initializes a local
occupant pointer to null, and calls the retained occupant manager at tool `+0x48`, **slot 32**
`GetFirstOccupantByStandardCityCell(out,x,z,filter)` (`00624B89–00624BA3`). This is the strict standard-cell
overlap query in occupant-manager.md §7, taking the **first match in newest-first spatial list order**.
On a hit QI only that occupant for `A821EF94`, return its Boolean QI result, and release the temporary
occupant reference (`00624BB2–00624BCD`). There is no second match search after a failed QI, no sort by
network type, and no combination of several occupants' data. The successful QI reference is owned by the
caller. Miss/outside leaves the caller's output unchanged; this caller initializes it to null.

The cell-info builder reads all thirteen type positions in increasing order 0…12 from that **one** network:
slot 22 (`+0x58`) tests membership, and slot 38 (`+0x98`) supplies the corresponding edge word only on a
true membership test. Thus a crossing inside one occupant retains each represented type; a separate
overlapping occupant contributes nothing to this record. The record retains the selected network pointer
and uses its network-kind mask restricted to `1FFF`. No random draw is involved. For two qualifying
overlapping surface occupants A then newer B, B supplies all thirteen membership/edge decisions; removing
B lets a subsequent uncached rebuild choose A. A cached cell record is reused by `GetCellInfo` until the
world cache is cleared; changing spatial order alone does not revise that cached record.

| Offset | Field |
|---|---|
| `+0x00`, `+0x04` | x, z |
| `+0x08` | network flags for the piece: the type bits, plus `0x40000` on bridge-step cells and `0x80000` on portal cells |
| `+0x0C` | the existing occupant, or null |
| `+0x10` | byte: part of the run |
| `+0x14` | step index |
| `+0x18`–`+0x48` | 13 per-type edge masks (one byte per side) |
| `+0x4C` | the union of the edge masks |
| `+0x50` | byte, from the occupant's slot 69 |
| `+0x51` | byte: placed (an existing piece) |
| `+0x52` | byte: an intersection piece (`0x100000`) |
| `+0x53` | byte: **fixed** (an immovable piece, or a step of kind ≠ 0) |
| `+0x55` | byte: tunnel interior |
| `+0x56` | byte: connection node (`0x200000`) |
| `+0x58` | 4 corner heights. Corner order: (x, z), (x, z+1), (x+1, z+1), (x+1, z) |
| `+0x68` | 4 allowed ranges (min, max) |
| `+0x88` | 4 preferred ranges (min, max) |
| `+0xA8` | 4 vertex ids |
| `+0xB8` | solved-cell id |

`CreateCellInfosForCurrentPlacementTiles` (`0x00633250`), for each dragged cell:

- **New cell**: corner heights := the terrain altitudes; allowed range := height ± the new-cell tolerance (`+0x2C4`);
  preferred range := ±4.25e37 (unbounded); **fixed** if its step kind ≠ 0, as for an existing piece. So every cell of
  a bridge, portal or tunnel-interior step is fixed (§9.3); the corners of the bridge and portal cells are later
  overwritten by the bridge pieces' and the portals' heights (§9.8).
- **Existing piece in the run**: in-run; fixed if its step kind ≠ 0; allowed range := height ± the re-solve
  tolerance (`+0x2A4`); preferred range := [height, height].
- Bridge-step and portal cells get `0x40000` and `0x80000` in `+0x08`.
- Each corner's minimum and preferred minimum are raised to the **surface-water level** at that vertex when it is
  higher (a NaN level is ignored): a network never sinks below the water surface.

Cells around the run (`CreatePlaceholderCellInfo`, `0x00632EF0`) get a range of ± type info `+0x84` (p+0x34, the
new-cell tolerance). An existing occupant's info (`ComputeAndStoreCellInfo`, `0x00633030`) gets ±FLT_MAX.

### 8.2 IdentifyHighwayIntersections and the rule solver: outline for a follow-up spec (NG5)

The follow-up spec is [network-rules.md](network-rules.md) (re-specs17b), which replaces this outline: rule files
§2–§5, the solve stages §6, edge subtypes §7, multi-type intersections §8, tile subsets §9, `cIntRule` §10.

**`IdentifyHighwayIntersections(v, type)`** (`0x0063B340`, pop 8; called at `0x0063BAC2`). It clears the tool's
intersection list (`+0xCC`/`+0xD0`), then for each step `< v` and each cell of the step that has an existing occupant
(cell info `+0x0C`): it walks the global highway-intersection rule map (`0x00B452B0`, buckets `0x00B452B4`…), and a
rule matches when the map entry's enabled byte (entry `+0xF1`; the rule starts at entry `+8`) is set, its flag mask
(entry `+0xEC`) meets the cell's flags (cell info `+0x08`), `cIntRule::IsWithinBounds` passes and `cIntRule::Test`
(`0x00609A00`) returns 0. Consecutive steps with a match form one intersection: it is marked (`MarkIntersection`) at
its first match and recorded (the `+0xCC` list and an `(x, z, value)` entry in the tool's `+0x08` vector) when a step
without a match ends the run, or after the last step. It **returns its `v` argument unchanged on every path**
(`0x0063B7B5`, `0x0063B7D7`). The caller sets `0xF0000005` only when the result is below `+0x78`
(`0x0063BAC7`–`0x0063BAD0`), so **`0xF0000005` is never set** *[quirk]*.

**What the follow-up spec needs to cover:**

| Part | Address / resource |
|---|---|
| rule files | INI resources type `0x0A5BCF4B`, group `0xAA5BCF57`: instance `0x10000000` highway intersection rules (`ReadHighwayIntersectionRules` into `0x00B452B0`), `0x10000001` multi-type intersection solutions and the override matrix (copied into each type info at `+0xFC`, `0x00B453C4` with stride `0x114`), `0x10000002` the `RuleOverrides` tile conflict rules; all loaded by `ReloadRules` (`0x006253D0`), called by the first network tool `Init` of the process (`0x00626A0D`) |
| per-type evaluators | type info `+0x24` (`LoadEvaluator`); `SolveNetwork` (`0x00635CB0`) calls each evaluator through `SolveUsingEvaluator`, which applies the network rules (`ApplyNetworkRules`) |
| multi-type intersections | `SolveMultiTypeIntersections` (`0x00634820`), `SolveMultiTypeSeparation` (`0x00633FA0`) |
| edge subtypes | `AssignEdgeSubtypes` (`0x006359B0`) |
| tile subsets | `AdjustTileSubsets`, `VerifyTileSubsets` (§8 step 3.5) |
| intersection rules | `nSC4Networks::cIntRule`: `Test` `0x00609A00`, `DoAutoComplete` `0x006097E0` (§4.5), `IsWithinBounds`; the intersection tool's `InsertIsolatedHighwayIntersection` (`0x0062CD50`) |

---

## 9. The height solver

### 9.1 The vertex-height constraint satisfier

`cSC4VertexHtConstraintSatisfier` (`Init` `0x0065E710`, `Shutdown` `0x006601A0`, `AddVertexToSolve` `0x0065FB60`,
`InsertRangeConstraint` `0x0065FED0`, `InsertFixedVertex` `0x00660360`, `Solve` `0x00660B10`, `GetHeight`
`0x0065EB00`). It assigns one height to each vertex in its to-solve set.

**Init(lo, hi)** sets the default range; a second `Init` before `Shutdown` does nothing.

**Per vertex** it keeps: the allowed range [min, max] (initially [lo, hi]); an optional **preferred value**; an
optional **preferred range**; and the **initial height** given to `AddVertexToSolve`. Adding a vertex twice keeps the
first record.

**Constraints:**

| Constraint | Meaning |
|---|---|
| range (v, a, b) | intersect v's range with [a, b]; if the result is inverted by at most 0.01 (`0x00AA8A74`), it collapses to the lower bound |
| fixed (v, h) | add v with initial height h, and range [h, h] |
| equality (a, b) | h(a) = h(b) |
| slope (a, b, s) | \|h(a) − h(b)\| ≤ s. A second slope constraint on the same pair keeps the smaller s |
| smoothness (a, b, c, s) | \|h(a) − 2·h(b) + h(c)\| ≤ s, where b is the middle vertex |
| preferred value (v, h) | the value to pick when it fits |
| preferred range (v, a, b) | the range to pick from when it overlaps |

**Solve** *[compat]*:

1. Propagate from **all unsolved vertices until no affected vertices remain**, before the first pick. This is
   not one sweep in hash order; the ordered work sets and equality rule are below. Any empty range fails.
2. Repeat while vertices remain:
   1. **Pick** a vertex (`0x0065ED80`): first pop a pending `(vertex, height)` pair from the stack at `+0x50`, if
      present. Otherwise choose the unsolved vertex with the **narrowest** range, in the iteration order of
      the unsolved set (below). A width (max − min) below 0 fails the solve (the vertex id is stored at `+0x10`); a
      width strictly below the best so far becomes the new best, so **the first of equal widths wins**; a NaN width
      is skipped; the scan stops as soon as the best width is exactly 0.
   2. **Choose its height**, for a hash pick: the preferred value, if set and inside the range. Otherwise, if a preferred range is set
      and overlaps the range (within 0.01), narrow the range to the overlap; then clamp the **initial height** into
      that picking window. This preference does not become a hard constraint on the other vertices. A stack pop
      already supplies its height.
   3. Pin a hash-picked vertex at the chosen height and propagate through its slope, smoothness and equality
      constraints until the affected work sets are empty. Record the solution and erase the picked/popped vertex
      from the unsolved set. Any empty range fails.
3. The result is true when every vertex got a height.

A failed solve dumps its constraints (debug only) and returns false.

**Propagation order and equalities (NT36, Tempelhof draft).** The initial pass (`0x006606E0`, called at
`0x00660B49`) inserts every unsolved id into an ordered set. The propagator (`0x00660390`) alternates two ordered
sets: visit the current set in **ascending signed vertex-id order**, using the ranges as changed by earlier
visits; collect affected ids in the other set; clear the current set; swap and repeat. Duplicates in a work set
are suppressed. Stop when the next current set is empty. After a non-singleton hash pick the same propagator
starts with just that id (`0x00660B84`–`0x00660B9E`).

Equality insertion merges the pair into an ordered **equivalence group** (`0x0065FDB0`); propagation is over
the group's ascending ids, not just the most recently inserted pair. For a non-singleton source vertex, intersect
each other group member's allowed range with the source's current allowed range. Enqueue each member whose
bounds change, and reject an intersection inverted by more than 0.01 (`0x0065F440`–`0x0065F4A7`, range helper
`0x0065EC40`–`0x0065EC84`, `0x0065E6A0`–`0x0065E701`). Equality propagates **allowed bounds**, not initial heights,
preferred values or preferred ranges. Thus before the first pick an isolated equality group has the common
intersection of its allowed ranges, but each vertex still has its own initial height and preferences.

A singleton source instead propagates its fixed height through the group (`0x006601C0`, `0x0065F2C0`). When a
propagation visit first finds a singleton not already marked as selected/queued, mark it and **append its
`(id,height)` to the pending stack** (`0x006604B7`–`0x006604F5`, append helper `0x006600D0`–`0x0066011A`). Subsequent
picks pop those pairs last-in, first-out before scanning hash buckets (`0x0065ED8F`–`0x0065EDB9`). This corrects
the former claim that nothing pushes onto this stack. The initial Tempelhof ranges are non-singletons, so this
stack is empty at its first hash pick; §9.8.1 traces both phases.

**Iteration order of the unsolved set** (answers NG1) *[compat]*. The unsolved vertices are an SGI
`hash_map<int, …>` at `+0x28`, with the identity hash. To reproduce the tie-break exactly:

- Buckets: the bucket of vertex id `k` is `k mod n`. The constructor (`0x00660810`) asks for 100 buckets, which
  becomes **193**, the first entry ≥ 100 of the prime list at `0x00A829B0` (53, 97, 193, 389, 769, 1543, 3079,
  6151, 12289, 24593, 49157, 98317, 196613, 393241, 786433, 1572869, 3145739, …).
- Insert (`0x004A29B0`): first `resize(count + 1)`, then, if the id is new, the node goes to the **head** of its
  bucket's chain.
- Resize (`0x004F0940`): when `count + 1 > n`, `n` becomes the first prime ≥ `count + 1`; the old buckets are walked
  from 0 upwards, each chain from its head, and every node is moved to the head of its new bucket.
- A vertex that is picked and pinned is erased from the set (`0x00660130`).
- `Shutdown` clears the set but keeps its bucket count (`0x008EBA60`), so `n` only grows over the life of the tool
  (one satisfier per tool, reused by every placement).
- Iteration (`0x004B0390`) goes through the buckets in ascending order, each chain from its head.

With 193 or more buckets and vertex ids `z × (X + 1) + x`, a run's vertices mostly fall in distinct buckets, so the
order is roughly ascending `id mod n`. Equal widths are common: every vertex whose range was never narrowed has the
same default width, and vertices tied to the same pinned neighbour by equal slope limits often get equal windows
*[inferred]*.

### 9.2 Slope and smoothness constraints per cell

`InsertSlopeAndSmoothnessConstraintsForCell` (`0x00636690`), for each cell to smooth that is not an intersection piece
and not fixed. Let `off` = 100000 if the cell is a connection node, else 0; vertex ids below include `off`.

1. **Flat cells.** The cell's four corners are tied with equality constraints (corner 0 = 1 = 2 = 3) when:
   - it has a number of connected sides other than 1 or 2, unless it carries a single network type and its edge
     union is `0x03010301` or `0x01030103` (two four-sided edge patterns); or
   - its type has a non-empty p+0xA0 list (highway, light rail, monorail) and exactly 2 connected sides, at least one
     with a diagonal or special edge value (edge union & `0xF8F8F8F8` ≠ 0).

   So intersections are level, and the elevated types' curves are level *[compat]*.
2. **Other cells.** For each of the 4 sides, `GetCrossSections` (`0x00636360`) gives three cross-sections along the
   run through this cell (previous, this, next), two vertices each, and whether the cell is diagonal. With the
   straight limits (type info `+0x100`, `+0x108`) or the diagonal ones (`+0x104`, `+0x10C`):
   - each cross-section is level: equality between its two vertices;
   - slope constraints between previous and this, and between this and next, with the slope rise;
   - a smoothness constraint over (previous, this, next) with the smoothness limit.

   Constraints with a negative vertex id, or with repeated vertices, are skipped.

Shipped limits (§2.4): road 11.2033 m per cell straight, 7.9219 diagonal (35°); rail 7.4609 and 5.2757 (25°);
highway 5.8235 and 4.1179 (20°); street 13.4256 and 9.4933 (40°); pipe 16.0 and 11.3137 (45°).

### 9.3 Range constraints per cell (slot 76, `0x006360C0`)

- **Connection node**: fix the vertices `id + 100000` at the lot's heights, plus the vertices the node's edges require
  (`kEdgesToFixVertex`).
- **Fixed cell**: fix all 4 corners at their current heights.
- Otherwise, per corner: if the vertex is in the boundary set (§9.4), fix it. Else add it with its height, insert its
  allowed range and its preferred range, and, if the cell is a placed piece, its height as the preferred value.

### 9.4 SmoothenNetwork(level) (`0x00637A80`)

1. Note the solved-cell count `n` (`+0x98`); put the intersection cells into the solved set
   (`InsertIntersectionCellsIntoSolvedSet`, `0x006371E0`); slot 75; `ComputeInitialSmoothingSet(level)`
   (`0x00636AB0`; an earlier text gave `0x00637720`, which is the vector resize below):
   - **Straight case** (dragged count = straight count = solved-cell count): the bounding box of all dragged cells,
     grown by `level` cells on each side and clipped to the city. Every cell in the box with a cell info that is not
     a tunnel interior goes into the **smoothing set** `+0xA4`. Every vertex on the box's border that is not on the
     city's edge goes into the **boundary set**: those vertices are held at the terrain.
   - **Otherwise**: the same, but the region is the union, per row, of each solved cell's span grown by `level`.
   Then the solved-cell vector is cut back to its first `n` entries (`0x00637720`, a resize), so the intersection
   cells appended in this step do not stay in it.
2. Build the run's **height constraints** and pin the bridge and tunnel cells: `CopyBridgeElevationConstraintsIntoCellInfo`
   (`0x0062FB90`, network-bridges.md §8.3), then the tunnel equivalent (`0x006274B0`), both appending to one local
   list; then slot 77, `ApplyPlaneConstraintToRegularCells(list)` (§9.8). These run again at every level, before the
   satisfier is reset, and write into the cell infos that step 4 reads.
3. Reset the network satisfier (`+0x124`): `Shutdown`, then `Init(0.0, MaxAllowedHeight + 20.0)` (terrain slot 24;
   the 20.0 is a double, `0x00AA2010`). Networks are never placed below 0 m or more than 20 m above the highest
   allowed terrain.
4. For each cell to smooth: slot 76 (§9.3), §9.2, slot 78. Then the intersection constraints (`0x00644710`).
5. `Solve`.

**The smoothing loop** (in `PlaceNetwork`): for level = (1 if the run has a bridge, else 0) to 3: if
`SmoothenNetwork(level)` fails, the code is `0xF0000006`; else if slot 56 (§9.6) fails, the code is `0xF0000008`;
else stop with success. If all levels fail, the **last** level's code is the failure code *[compat]*.

### 9.5 Height limits (summary)

| Limit | Value |
|---|---|
| network height | [0, `MaxAllowedHeight` + 20] (§9.4) |
| a new cell's corner | its terrain altitude ± the new-cell tolerance (p+0x34) |
| a re-solved piece's corner | its height ± the re-solve tolerance (p+0x14) |
| a regular cell's corner | within the plane height ± the plane tolerance (p+0x20), widened to include its current height (§9.8) |
| a bridge or portal cell's corner | fixed at the bridge piece's corner or the portal's height (§9.8, network-bridges.md §8.3) |
| any corner | ≥ the surface-water level at the vertex (§8.1) |
| slope | type info `+0x100` / `+0x104` per cell (§9.2) |
| change of slope | type info `+0x108` / `+0x10C` (§9.2) |
| terrain raise / cut | p+0x0C / p+0x10 (§9.6) |
| height above graded terrain | p+0x3C with supports; 0 without (§9.6) |

### 9.6 CanTilesBeSupportedOnTerrain(type) (slot 56, `0x00644B20`)

It solves the **terrain** heights under the smoothed network with the second satisfier (`+0x188`), `Init(−FLT_MAX,
FLT_MAX)`. For each cell in the smoothing set that is not a connection node, for each corner vertex:

1. `n` := the network height (the network satisfier's solution, or the cell's stored corner if the vertex was not
   solved); `t` := the terrain altitude (terrain slot 12). Add the vertex with initial height `t`.
2. If the vertex is in the boundary set, or the cell is both fixed and placed: **fix** it at `t`.
3. Otherwise: `hi := min(n, t + p0C)`, `lo := t − p10` (p0C, p10 = type info `+0x5C`, `+0x60`).
   - If the piece **has supports** (`GetSupportStructureInfo`, asked about the solved piece or, for an unsolved cell,
     the existing occupant):
     `lo := max(lo, n − p3C)` (type info `+0x8C`). If `hi < lo`: return **false**.
   - Else, if the cell is not fixed: if `hi < n` (the network is more than p0C above the terrain), return **false**;
     otherwise `lo := hi := n`: the terrain is graded to the network.
   - If `lo = hi`: fix the vertex there. Else insert the range [lo, hi] and the preferred value `n` (`t` for a fixed
     cell).
4. Then the intersection constraints, and every boundary vertex not yet added is fixed at its terrain altitude.
5. Return `Solve()`.

So a surface network without supports always sits **on** the graded terrain: the terrain is cut (up to p10) or
raised (up to p0C) to meet it *[compat]*.

### 9.7 UpdateTerrainHeights (slot 57, `0x00644530`): the grading

Called at commit (network-tools.md §6.1). It writes the terrain satisfier's solution into the terrain; terrain.md §8.2
(`data4` kind 4) has the message side.

1. Clear the output vector.
2. For each solved vertex of `+0x188` in solution order: if `|solved − altitude| > 0.001` (float `0x00A867A4`), set
   the altitude (terrain slot 50 `SetAltitudeAtVertex`), append the vertex id, and grow the rectangle: x = id mod
   `+0x248`, z = id / `+0x24C` *[quirk: the z divisor is the Z vertex count, not the stride; equal in square cities]*.
3. Grow the rectangle by 1 on each side (not below 0; the max side only while it is below the city's cell count,
   city slots 93 and 94).
4. If any vertex changed: `BeginLongTerrainOperation`, `RedisplayTerrain(1, 1, rect, 4)`, `EndLongTerrainOperation`.

The underground tools' slot 57 is a no-op.

### 9.8 Bridges, tunnels and the plane constraint (slot 77, `0x00637B90`, pop 4; answers network-bridges NB1)

This is how bridges and tunnels constrain the height solve. It runs in step 2 of every `SmoothenNetwork` level
(§9.4): after the smoothing set is known and before the satisfier is reset and fed (steps 3–5). It acts only on the
**cell infos** (§8.1); slot 76 (§9.3) then hands their corner heights and ranges to the satisfier.

**The constraint list.** A local list of 16-byte entries `{start step, end step, start height, end height}`, filled in
this order:

1. **Bridges** (`CopyBridgeElevationConstraintsIntoCellInfo`, `0x0062FB90`): one entry per bridge record, and every
   bridge cell's corners pinned at its piece's corner heights (network-bridges.md §8.3).
2. **Tunnels** (`0x006274B0`): for each tunnel record (`+0xE4`, 16 bytes: two heights `h0`, `h1`, then the start and
   end steps), stopping at the first whose end step is ≥ `+0x78`: the entry `{start, end, h0, h1}`; every corner of
   every cell of the start (portal) step gets height `h0` with its allowed range collapsed to `[h0, h0]`, and the end
   step's cells likewise with `h1`. Interior steps are not touched *[inferred: h0 and h1 are the two portal heights of
   §6.2]*.

Bridge and portal cells are fixed (§8.1), so slot 76 pins their four corners at these heights (§9.3).

**The walk** *[compat]*. For each step `s` from 0 to `+0x78 − 1` whose kind is 0 (regular; bridge, portal and tunnel
steps are skipped):

1. If no plane has been built yet, or `s` is past the current plane's far step `B`: choose the two **anchors**.
   - `A`, `hA`: the entry with the **largest end step below `s`** (strictly): its end step and end height. None:
     `A := s`, `hA := 0`.
   - `B`, `hB`: the entry with the **smallest start step above `s`** (strictly): its start step and start height.
     None: `B := +0x78 − 1`, `hB := 0`.

   Build the plane (below). If its Y coefficient `b` is exactly 0, the **whole pass ends** here and no later step is
   constrained *[quirk]*. Otherwise `m := r24(−1 / b)`.
2. For each cell of the step and each corner `i` (cell info order, vertex `(vx, vz)`; the coordinates are converted
   to float as unsigned values):
   - the plane height `h := r24(r24(r24(tx + tz) + d) × m)`, with `tx := r24(r24(vx × cs) × a)`,
     `tz := r24(r24(vz × cs) × c)` and `cs` = the cell size `+0x238`;
   - the window: `lo := r24(h − tol)`, `hi := r24(h + tol)` with `tol` = `+0x2B0` (p+0x20, §2.4). Let `k` be the
     corner's current height (`+0x58[i]`). If `k < lo`, `lo := k`; else if `hi < k`, `hi := k`. The window always
     contains `k`;
   - the allowed range (`+0x68[i]`): `min := max(min, lo)`, `max := min(max, hi)`; then, if `max < min` and
     `min − max ≤ 0.01` (`0x00AA8A74`), `max := min`. A wider inversion is left for the satisfier to reject;
   - the preferred range (`+0x88[i]`) is **replaced** by `[min(h, k), max(h, k)]`.

**The plane** (`0x00637780`). `d` := the pen direction `+0x7C` if the straight count `+0x74` is ≥ 1, else the end
direction `+0x80`; `d2 := (d + 4) mod 8`. With the vertex offset tables of §6.1 as `(dx, dz)` pairs (`+0x250` is the
static table `0x00AA8AE0`, `+0x270` is `0x00AA8AA0`):

- `Q1`, `Q2` := the first cell of step `A` plus `+0x250[d]` and `+0x270[d]`: the two corners of its **trailing**
  edge;
- `Q3`, `Q4` := the first cell of step `B` plus `+0x250[d2]` and `+0x270[d2]`: the corners of its **leading** edge.
- If `hA ≤ 0`, `hA :=` the anchor height of `(Q1, Q2)`; if `hB ≤ 0`, `hB :=` the anchor height of `(Q3, Q4)`
  (below). So a missing neighbour, and also an entry whose height is ≤ 0, takes its height from the terrain.
- If `A = B` and `A ≥ +0x74` (a single step in the diagonal part): the level plane `(0, 1, 0, −hA)`.
- Otherwise the plane through `P1 = (Q1.x × cs, hA, Q1.z × cs)`, `P2 = (Q2.x × cs, hA, Q2.z × cs)` and
  `P3 = (Q3.x × cs, hB, Q3.z × cs)` (`Q4` is computed and not used). The coefficients (`0x005D67E0`), each operation
  rounded to float, summed left to right:
  - `a = (z3 − z1)·y2 + (z2 − z3)·y1 + (z1 − z2)·y3`
  - `b = (x2 − x3)·z1 + (x3 − x1)·z2 + (x1 − x2)·z3`
  - `c = (y1 − y2)·x3 + (y3 − y1)·x2 + (y2 − y3)·x1`
  - `d = −((y2·z3 − z2·y3)·x1) − ((z1·y3 − y1·z3)·x2) − ((y1·z2 − z1·y2)·x3)`

  For a straight run the plane is level across the drag and rises linearly from `hA` on step `A`'s trailing edge to
  `hB` on step `B`'s leading edge. The products `x × cs` are rounded to float before use.

**Anchor height of an edge `(Q, Q')`** (`0x006374B0`, once per vertex). For a vertex `v`, `t(v)` is its terrain
altitude (terrain slot 12). `v` is **held** when one of the four cells around it, `(x, z)`, `(x, z − 1)`,
`(x − 1, z − 1)`, `(x − 1, z)` (offset tables `0x00AA8A8C`, `0x00AA8A7C`), is inside the city, is **not** in the
smoothing set `+0xA4`, and holds an occupant found through the tool's filter (`+0x364`, occupant manager slot 32)
whose network flags meet the tool's layer mask `+0x360` (network occupant slot 9). Then:

- `Q` held: `t(Q)`;
- else `Q'` held: `t(Q')`;
- else `r24(r24(t(Q') + t(Q)) × 0.5)` (`0x00A84D2C`).

The vertex test first asks the lot manager (`+0x4C`, the city's lot manager; slot 26, `GetLotVertexAltitudeAdjustmentRange`, zones.md §8.7) and
treats the vertex as held when its second output is exactly 0.0. That slot only ever outputs FLT_MAX or 32.0
(`0x0072AEE6`–`0x0072AF17`), so lots never hold a vertex here *[quirk]*. The height used is always the terrain
altitude, never a network or lot height.

**Consequences.**

- Every run of a surface tool gets a plane, with or without bridges and tunnels. With no entry, one plane joins the
  trailing edge of step 0 to the leading edge of the last valid step, at the terrain heights there: each regular
  corner may stray from that straight line by at most the tolerance (35 m for road, 15 m for the other shipped types),
  or stay at its own current height if that is farther.
- Between two structures (bridges or tunnels) the regular cells follow the straight line from the first one's end
  height to the second one's start height. Before the first structure and after the last, the line runs to the run's
  end, anchored on the terrain.
- A structure's anchor edges are one cell **inside** it: the trailing edge of its end step and the leading edge of
  its start step, at the outer height of its abutments (network-bridges.md §8.3). The line is therefore shifted by
  one cell toward the structure *[quirk]*; with tolerances of 15 m or more and the window widening, it rarely binds.
- Power line, with tolerance 0, gets the window `[min(h, k), max(h, k)]` on every regular corner, if its placement
  reaches the smoothing step *[inferred]*. The underground tools' slot 77 is a no-op (§1.2).

#### 9.8.1 Isolated nonflat road draft: Tempelhof (NT36)

This is an **instruction-derived fixture**, using NT36's supplied live terrain values, not an extracted native
private-state snapshot. It closes the plane/constraint inputs for an isolated new cell; the public getter limit
in network-tools §8.1.4 still applies to arbitrary occupied city runs. Use a fresh tool with a 193-bucket network
satisfier, a 128-square city, vertex stride 129, cell size 16, surface-water altitude 250, and network default
range `[0,H]` with H ≥ 328. Draft road from `(86,3)` to itself, no neighbour connection, bridge, tunnel,
intersection, connection node or additional draw. All visited surrounding cells are empty and supply no cell
info; no lots hold the anchor vertices. The sole solved entry has, **before smoothing**, index 0, key `(86,3)`, run=true,
intersection=false, placed=false, fixed=false, connection=false, road type only, **edge union 0**. There are no
other retained entries. The zero connected-side count invokes the flat-cell rule (§9.2): insert equalities
`473=602`, `602=603`, `603=474`, in that order (`0x00636782`–`0x006367D9`).

| Corner | Vertex coordinates | Id | Initial/current network corner = live terrain (exact float32) |
|---|---|---:|---:|
| 0 | `(86,3)` | 473 | 291.402679443359375 |
| 1 | `(86,4)` | 602 | 293.308929443359375 |
| 2 | `(87,4)` | 603 | 290.7928466796875 |
| 3 | `(87,3)` | 474 | 289.532623291015625 |

**Smoothing retry.** Counts are dragged=straight=valid=1, pen direction 2, end direction 1 (§3.2). At level 0 the
vertex box is `[86,87] × [3,4]`; all four vertices are non-city-edge boundary vertices and slot 76 fixes them at
their distinct terrain heights. The equality group is inconsistent, so this level fails. Level 1 uses vertex
box `[85,88] × [2,5]`; none of the four run vertices is on its border. Its smoothing set still contains only
`(86,3)`, because empty neighbours have no cell info. The outer boundary ids are not network solve inputs;
§9.6 may add them to the separate terrain satisfier. No additional solved cells result. The box expansion and
inclusive border walk are `0x00636AB0`–`0x00636CD0`; the retry is §9.4. Level 1 succeeds, leaving final failure=0,
return=true and the same one-cell counts. A failed earlier level does not pin corners in the reset satisfier.

**Plane, recomputed at each level.** The structure list is empty, A=B=0. Straight count 1 means the special
single-diagonal level-plane branch does **not** apply. Direction 2 selects Q1=`(86,4)`/602 and Q2=`(87,4)`/603;
direction 6 selects Q3=`(87,3)`/474 and Q4=`(86,3)`/473. Neither edge is held. In the order specified above:

- hA = `r24(r24(t603+t602)×0.5)` = **292.0509033203125**;
- hB = `r24(r24(t473+t474)×0.5)` = **290.4676513671875**;
- P1=`(1376,hA,64)`, P2=`(1392,hA,64)`, P3=`(1392,hB,48)`;
- the §9.8 float32 operations give `(a,b,c,d)` = **`(0,256,−25.33203125,−73144)`**, m=−0.00390625;
- evaluating the stored coefficients gives plane heights **290.468505859375** at z=3 and
  **292.0517578125** at z=4. These differ slightly from the edge means because coefficient construction rounds;
  do not replace the stored plane by an algebraically simplified interpolation.

The means and branch are `0x0063795F`–`0x00637A29`, coefficient operations `0x005D67E0`–`0x005D6883`, plane
evaluation `0x00637CFB`–`0x00637D30`. Road tolerance is 35. Initial new-cell allowed ranges are terrain ±500,
with lower bounds raised to water altitude 250; the plane window is the tighter constraint at all four corners.
No window needs widening to include its initial corner. Slot 77 produces these **level-1 inputs to slot 76**:

| Id | Allowed range | Preferred range | Initial height |
|---:|---|---|---:|
| 473 | `[255.468505859375,325.468505859375]` | `[290.468505859375,291.402679443359375]` | 291.402679443359375 |
| 602 | `[257.0517578125,327.0517578125]` | `[292.0517578125,293.308929443359375]` | 293.308929443359375 |
| 603 | `[257.0517578125,327.0517578125]` | `[290.7928466796875,292.0517578125]` | 290.7928466796875 |
| 474 | `[255.468505859375,325.468505859375]` | `[289.532623291015625,290.468505859375]` | 289.532623291015625 |

Add the vertices in corner order `(473,602,603,474)` with those initial heights, ranges and preferred ranges;
there are **no preferred values**, slope or smoothness constraints, fixed run vertices or intersection constraints.
The flat-cell branch supplies only the equality group. Allowed/preferred writes are
`0x00637D34`–`0x00637DCA`; slot 76 transfers them without substituting a preferred-range endpoint (§9.3).

**Initial propagation and the first pick.** The first ordered wave is `(473,474,602,603)`. Source 473 narrows
602 and 603; source 474 changes nothing further; source 602 narrows 473 and 474; source 603 changes nothing.
All four ids are in the next work set, which makes no further changes. Every allowed range is now
**`[257.0517578125,325.468505859375]`**, width **68.416748046875**. Initial heights and preferred ranges remain
as in the table. There are no singleton ranges and hence no pending pairs. Hash buckets for ids
`(473,602,603,474)` are `(87,23,24,88)`, so the scan order is **602,603,473,474**. Equal widths make 602 win.
Its initial **293.308929443359375** lies in both its allowed and preferred ranges; choose that height.

Pin 602. Equality propagation fixes all three others at the same height, even where that is outside their
preferred ranges. In the next ordered propagation wave, singleton ids 473,474,603 append pending pairs in
that order; after recording/removing 602, the stack drains **603,474,473** (§9.1). All four network heights,
and then all four retained draft corners, are **293.308929443359375**. Draft preparation refreshes corners
through slot 73 before preview/cost use (`0x00626000`; `0x00625BB9`–`0x00625BF6`). Ordinary no-support terrain
feasibility is admitted: all required raises are below road's 25 m limit; §9.6 succeeds. Draft does not apply
those terrain targets. Getter-time terrain therefore remains the four inputs above, and the sole solved cell
prices at **11** (network-tools §8.1.5). The ordinary draft insertion with its third argument false sets the
placed byte after this solve/refresh (`0x00625BF4`–`0x00625BFB`); this does not suppress the getter's run-cell charge.

The plane supplies windows/preferences; it does not demand a least-earthwork height or a flat edge mean.
Changing bucket count/history can change the first equal-width pick (§9.1). This fixture explicitly supplies
193 buckets; a different retained bucket count is a different input, not permission to adjust the price.

---

## 10. Occupants, supports and flags

### 10.1 CreateNetworkOccupant (slot 54, `0x00624210`)

The new occupant's network flags := the cell info's flags (`+0x08`). In draft mode only the bridge and portal bits
(`& 0xC0000`) of the extra bits survive, and **`0x80000000`** is set; in real mode `0x80000000` is cleared. A tunnel
occupant (`0x8A4BD52B`) is made usable (`0x10000000`) with `0x4000` cleared; every other occupant gets `0x4000`.

### 10.2 Supports (InsertNetworkOccupant, slot 72, `0x006278A0`)

If the piece has support structure info (`GetSupportStructureInfo`, `0x00625860`: a lookup of the piece id in type
info `+0x3C`), its **support level** (`0x00623BB0`) is the maximum over its 4 corners of:

| d := r24(corner − terrain altitude) | level |
|---|---|
| d ≤ 0.01 | 0 |
| 0.01 < d < `+0x2C8` | 1 |
| d ≥ `+0x2C8` | 2 |

If the level is > 0: `AddSupportStructures` (`0x00643FB0`), then set flags **`0x40000000`** and `0x8000000`.

**Where the support info comes from** (answers NG6). `ReloadNetworkTypeInfo` (`0x00623310`) calls `LoadSupports`
(`0x0063F780`) with group `0x8974F80F` for **road, one-way road, dirt road, rail, light rail, monorail and street**
(types 0, 10, 11, 1, 8, 9, 3), and for no other type. `ReadSupportStructureInfos` (`0x0063F700`) visits every
exemplar of type `0x6534284A` in that group and `ReadSupportStructureInfo` (`0x0063F400`) reads from each:

| Property | Value |
|---|---|
| `0xE974EAA1` | the piece id (the map key) |
| `0xE974EAA2` | a u32 list (the support models) |
| `0xE974EAA3` | 4 points (x, z pairs), stored as given and rotated by 90°, 180° and 270° (`SC4GeometryUtilities::RotatePoint2D`) for the four rotations |

All seven types read the same group, so they share one set of entries. Every other type (highway, avenue, ground
highway, pipe, subway, power line) has an empty map: its pieces never get supports here, and §9.6 treats them as
pieces without supports. The p+0xA0 list plays no part (it is read only for its emptiness, §9.2).

### 10.3 The flags (answers NM3)

| Flag | Meaning | Set by |
|---|---|---|
| `0x40000` | the piece is on a **bridge step** (kind 1, abutments included) | cell infos (§8.1) |
| `0x80000` | the piece is a **tunnel portal** (kind 2) | cell infos (§8.1) |
| `0x40000000` | the piece is **elevated** and has support structures | §10.2 |
| `0x80000000` | a **draft** (preview) occupant, never part of the real city | §10.1 |

---

## 11. Costs (answers NT2) and the power line (answers NT3)

**NT19:** fresh construction plus Init with missing properties gives `+0x3A4` uint32=20 (drag-step
spacing), `+0x3A8` float32=7.0 (clearance added to ray endpoints), `+0x3AC` uint32=0 and `+0x3B0`
float32=0.0. Re-Init resets only spacing/clearance; absent cost properties retain previous values.
Evidence and numeric order: [power-line-tool.md §§1,4](power-line-tool.md).

**NT20(b):** initial `last` is input count-1. Zones and nonterminal roads preserve it. Results above 3
preserve last/failure but update the previous-result local. A trailing zone after a clear step loses
the final step; an all-zone walk can retain the original count. See power-line-tool §6 and its vectors.

**NT2.** The per-type cell cost table `0x00B453A4` is type info `+0xDC` = p+0x8C (property `0x4A2D49B9`). `+0x320`,
`+0x324` and `+0x328` are the tool's copy of p+0x90 (`0x8A2D49EA`), p+0x94 (`0x8A2D49F0`) and p+0x98 (`0x6A2D49F4`),
with defaults 10, 20 and 0.1. Shipped values: §2.4. A flat straight road of 11 cells on empty land costs 11 × 10 = 110
(no earthwork, no demolition).

**NT3, cost.** `cSC4PowerLineTool::GetCostOfSolution` (`0x0064F0F0`): `poles` is the retained count at
`+0x388`. DeterminePolePositions snapshots (`+0x370` − `+0x36C`) / 48 there; releasing the records later
does not reset either pricing count (network-tools §8.3.1, NT32):

- poles ≤ 1: 0;
- otherwise truncate the x87 product of uint32 wire count `+0x384` and float32 `+0x3B0`, then add
  `+0x3AC × poles`. No intermediate float32 rounding (network-tools §8.3.1, NT31).

`+0x384` is counted up by `DeterminePolePositions` *[inferred: the number of wire cells]*. The values come from the
exemplar (type `0x6534284A`, group `0xE7E2C2DB`, instance `0xC911E35B`): `0x4A5F5953` = 15 (cost per pole, `+0x3AC`),
`0x0A5F596B` = 2.0 (cost per wire cell, `+0x3B0`), `0x098B25C8` = 10 (`+0x3A4`, default 20), `0x098B27D0` = 7
(`+0x3A8`).

**NT3, result 3.** In the power-line `CheckForReservedTiles` (`0x0064FED0`), `last` starts at count − 1 and the walk
remembers the previous result:

| Result | Effect |
|---|---|
| 0 | `last := i` |
| 1 (zoned) | code `0xF0000011`; `last` unchanged; continue |
| 2 (building) | code `0xF0000012`; `last := i − 1`; stop |
| 3 (road) | code `0xF0000013`; stop only if i = count − 1 and the previous result was 0 |

Then `v := last + 1`. On the first cell the "previous result" is an uninitialised local *[quirk]*; a one-cell drag
onto a road may or may not stop. A road cell in the middle of a line is crossed.

---

## 12. Underground occupants (answers NT4)

The underground occupant's slots used by the refit (network-tools.md §11.1 step 6), on its network-occupant interface
(vtables `0x00AAAD40`, `0x00AA97B8`):

| Slot | Address | Method | Effect |
|---|---|---|---|
| 50 | `0x00619680` | `ReleasePathInfo` | releases the path info at `+0x140` and clears the pointer |
| 84 | `0x0065AE30` | `SetVertexHeights` | |
| 86 | `0x0065ADD0` | `CreateGeometry(bool)`, pop 4 | rebuilds the model: reads its vertex heights (slots 24, 25), computes the model's Y position (`ComputeModelYPosition`, `0x0065AB50`), then rebuilds the model through slots 85, 87 (two values and the flag), 89 and 90 |
| 94 | `0x00659DD0` | `GetTerrainHeights` | |
| 95 | `0x0065B6B0` | `SetTerrainHeights` | |

So step 6 drops the cached path information (the traffic path through the piece) and regenerates the piece's model at
its new heights. Neither changes flags or posts a message.

---

## 13. DestroyOccupantsAndResolve, step 5.3 (answers NT1)

network-tools.md §10 step 5.3 said the test is "the set of cells that will be re-solved". It is not. The tested set
is the set of **destroyed cells**: the cells of the occupants in the caller's list, plus the cells added while closing
the set for the neighbours of doomed structures. For a removed occupant:

- its cell is a destroyed cell: `RemoveOccupant(o, 0, 0)` (nothing is put back);
- otherwise (a piece pulled in by the closure, such as the rest of an intersection structure):
  `RemoveOrReplaceOccupant`, which may put a plain piece back. If it replaced the piece, the cell goes into the
  **re-solve** set; otherwise the cell is erased from it.

The re-solve set already holds, from the closure, the cells of the removed pieces' ordinary neighbours (movable
pieces and connection nodes). Every cell key built in this function is `(z << 16) + x`, like the tool's sets: the
row loop shifts z (`0x0062B4E1`) and the neighbour loop builds its key from the neighbour's z and x
(`0x0062BA9A`–`0x0062BA9D`); there is no swapped key (NG7). Then `SolveNetwork(−1, true)` over the re-solve set.
DIFFERS from network-tools.md §10 (corrected there).

---

## 14. Failure codes

| Code | Set by | When |
|---|---|---|
| `0xF0000001` | §4.2, §4.5 | the run starts in water; a regular step crosses water (cut there); an extra cell is dry (quirk, unreachable) |
| `0xF0000002` | §5, network-tools.md §6 | a reserved cell (cut there); an undemolishable occupant in the way |
| `0xF0000003` | §7 | a bridge span or bridge type could not be found (cut there) |
| `0xF0000004` | — | never (§6.3) |
| `0xF0000005` | — | never: `IdentifyHighwayIntersections` never lowers `v` (§8.2) |
| `0xF0000006` | §9.4 | the last smoothing level's height solve failed |
| `0xF0000007` | §8 step 3 | a cell got no piece |
| `0xF0000008` | §9.6 | the last smoothing level solved, but the terrain cannot support it |
| `0xF0000011`–`13` | §11 | power line only |

Any code refuses the whole run (`+0x78 := 0`), even when a pass only cut it (network-tools.md §5.1).

---

## 15. Answers given here

- **NT1** (network-tools.md §17): §13. DIFFERS.
- **NT2**: §11. MATCHES the inference (the costs come from the placement exemplars).
- **NT3**: §11. The cost was not described; the result-3 rule MATCHES the inference, with the uninitialised first
  "previous result".
- **NT4**: §12.
- **NT5**: this spec (§3–§9).
- **NG1, NG3, NG6, NG7** (§17, answered 2026-10-16): §9.1, §4.5, §2.3 and §10.2, §13. **NG2, NG4**:
  [network-bridges.md](network-bridges.md) (answered 2026-10-16, re-specs17a). **NG5**: §8.2 and
  [network-rules.md](network-rules.md) (answered 2026-10-16, re-specs17b).
- **NB1** (network-bridges.md §16, answered 2026-10-16, re-answers26): §9.8 (the plane constraint and the tunnel
  entries), §8 step 2 (the solve-set removals), §8.1 (bridge-step cells are fixed) and §9.4 step 2 (where it runs);
  the bridge side is network-bridges.md §8.3.
- **NM3** (network-manager.md §15): §10.3. `0x40000` MATCHES (bridge); `0x80000` MATCHES (tunnel portal);
  `0x40000000` MATCHES (supports); `0x80000000` = draft occupant.

## 16. Quirks (summary)

1. The tunnel failure code `0xF0000004` is never set (§6.3).
2. `CheckForGoodTerrain` refuses the run when an **extra** cell is dry, the reverse of its test on dragged cells
   (§4.5). Unreachable: only the intersection tool fills extra cells, and it never calls that check.
3. The power-line reserved-tile walk reads an uninitialised "previous result" on its first cell (§11).
4. The grading rectangle divides vertex ids by the Z vertex count (§9.7).
5. A width-1 diagonal snap adds one step along the major axis (§3.2).
6. A failed smoothing loop reports the **last** level's code, even if an earlier level failed for the other reason
   (§9.4).
7. The highway-intersection failure code `0xF0000005` is never set (§8.2).
8. `+0x354` (land bridges) is set for every type with a bridge type, because `Init` tests a field's address (§4.4).
9. Type info `+0x110` is computed and never read; the p+0xA0 ids are never read, only the list's emptiness (§2.3).
10. The satisfier's pending-pin stack (`+0x50`) is never filled (§9.1).
11. A plane whose Y coefficient is exactly 0 ends the plane pass for the rest of the run (§9.8).
12. The plane's anchor test asks the lot manager for a vertex range that it never reports, so lots never hold an
    anchor vertex (§9.8).
13. A structure anchors the plane one cell inside itself, on the inner edges of its end steps (§9.8).
14. A bridge or tunnel height ≤ 0 cannot anchor the plane: it is replaced by the terrain anchor (§9.8).

## 17. Open questions

- ~~**NG1.** The tie-break of the satisfier's pick is the hash-map iteration order of vertex ids (§9.1).~~
  **ANSWERED 2026-10-16 (re-answers19):** §9.1 gives the exact order (identity hash, `id mod n`, 193 initial
  buckets, head insertion, the SGI resize and prime list, buckets never shrink) and corrects the pick: there is no
  "already pinned first" step; the first of equal widths wins.
- ~~**NG2.** The bridge internals (§7).~~ **ANSWERED 2026-10-16 (re-specs17a):** [network-bridges.md](network-bridges.md).
  Earlier (re-answers19): §7 corrected the length map offset (`+0x16C`, keyed by the step count) and gave the bridge
  record.
- ~~**NG3.** Who fills the extra-cell list `+0xF0`, and whether quirk 2 can be reached in play.~~ **ANSWERED
  2026-10-16 (re-answers19):** only the intersection tool's rule auto-complete; quirk 2 is unreachable (§4.5).
- ~~**NG4.** `IdentifyLandBridgeCells` (§4.4).~~ **ANSWERED 2026-10-16 (re-specs17a):** network-bridges.md §7 (the
  scan and `CreateLandBridge`) and §2.5 (the `+0x354` address test reads bridge type `+0x98`, property `0x482CB44F`;
  no shipped effect).
- ~~**NG5.** The rule-driven piece solver and `IdentifyHighwayIntersections`.~~ **PARTLY ANSWERED 2026-10-16
  (re-answers19):** `IdentifyHighwayIntersections` is described and never fails (§8.2, quirk 7); the rule solver is
  left to a follow-up spec, outlined in §8.2. **ANSWERED 2026-10-16 (re-specs17b):** the follow-up spec is
  [network-rules.md](network-rules.md).
- ~~**NG6.** What reads type info `+0x110` and p+0xA0, and the source of road's support info.~~ **ANSWERED 2026-10-16
  (re-answers19):** nothing reads `+0x110`; p+0xA0 is read only for its emptiness (§9.2); the support info comes
  from the `0x8974F80F` exemplars for seven types (§10.2).
- ~~**NG7.** The neighbour key in `DestroyOccupantsAndResolve`.~~ **ANSWERED 2026-10-16 (re-answers19):** every key
  there is `(z << 16) + x`; no mismatch (§13).

---

## 18. Conformance tests

A "city" is a fresh flat, dry 64 × 64 city, unless stated. Road = type 0. Cells are (x, z). Tests 7 and 8 drive the
satisfier directly.

1. **Straight drag** (§3.2). Road, snap on, (10, 10) → (20, 10): dragged count 11, straight count 11, pen direction
   4, end direction 3, one cell per step.
2. **Near-straight drag** (§3.2 step 1.2). Road, snap off, (10, 10) → (20, 11): the end becomes (20, 10); counts as in
   test 1.
3. **Diagonal snap, width 1** (§3.2). Road, snap on, (0, 0) → (6, 4): pre-adjust makes dx = 5; the snap keeps dx =
   dz + 1 = 5; end (5, 4); dragged count 6; straight count 0 (|5 − 4| = 1 with w2 = 1); pen direction 4; end
   direction 5.
4. **Diagonal snap, width 2** (§3.2). Avenue, snap on, (0, 0) → (5, 4): dx := dz = 4; end (4, 4); dragged count 5;
   straight count 0.
5. **Street is straight only** (§3.2 step 2). Street, (0, 0) → (5, 4): pre-adjust none; dz < dx, so z2 := 0; end
   (5, 0); count 6; straight count 6; `DrawNetworkLine` returns true. (Step 2 always leaves one delta at 0, so the
   straight-only refusal in §3.1 step 3 cannot trigger from a drag *[inferred]*.)
6. **Slope limits** (§2.3). Road type info `+0x100` ≈ 11.2033, `+0x104` ≈ 7.9219; highway `+0x100` ≈ 5.8235.
7. **Satisfier, slope** (§9.1). `Init(0, 1000)`; fixed a = 0; fixed c = 30; b added with initial height 0;
   slope(a, b, 11.2033) and slope(b, c, 11.2033) → `Solve` = false (|c − a| = 30 > 22.4066). With c = 20 instead →
   true, b's range is [r24(20 − 11.2033), 11.2033] ≈ [8.7967, 11.2033], and b = 8.7967 (the initial height 0 clamped
   into the range).
8. **Satisfier, smoothness and preference** (§9.1). Fixed a = 0 and c = 0; b with initial height 0;
   smoothness(a, b, c, 11.2) → b's range [−5.6, 5.6]. Preferred value 3 → b = 3. Preferred value 9 → b = 0 (out of
   range, so the initial height is clamped).
9. **Tunnel trigger** (§6.2). Road, straight drag of 12 steps; terrain whose step means rise by 11.0 m between steps
   2 and 3 → no tunnel search. Rise 12.0 m → `FindTunnelEnd` runs from step 2.
10. **No tunnel code** (§6.3). Any drag: `GetFailureState()` is never `0xF0000004`.
11. **Bridge needs a straight drag** (§7, §4.5). Road (0, 10) → (20, 16) across a river at x = 8..12, snap off: no
    bridge; the run fails with `0xF0000001` (the first wet regular step cuts it). The same drag snapped straight,
    (0, 10) → (20, 10), gets a bridge when a bridge type fits.
12. **Starting in water** (§4.2). Road whose first cell has water → `0xF0000001`, nothing placed. A pipe drag from the
    same cell is not refused (slot 63 returns 0).
13. **Reserved landfill** (§5.1). A landfill occupant at (12, 10); road (10, 10) → (15, 10) → `0xF0000002`.
14. **Support level** (§10.2). Road (`+0x2C8` = 45): corners 30 m above the terrain → level 1, flags `0x40000000` and
    `0x8000000` set; 50 m → level 2; 0.005 m → level 0, no flags.
15. **Terrain support** (§9.6). Road (p0C = 25, p10 = 10000, p3C = 55), a corner solved at n = 100 over terrain t = 0
    with supports → hi = 25, lo = 45 → false (`0xF0000008`). At n = 70 → range [15, 25], preferred 70 out of range, so
    the terrain becomes clamp(0, 15, 25) = 15. Without supports and n = 20 → the terrain vertex is fixed at 20.
16. **Draft flag** (§10.1). A preview (`real` = 0) road piece has `0x80000000`; the committed piece does not.
17. **Cell cost** (§11). Road (10, 10) → (20, 10) on flat empty land: `GetCostOfSolution()` = 110.
18. **Power-line cost** (§11). 3 poles and `+0x384` = 20 → trunc(20 × 2.0) + 15 × 3 = 85. One pole → 0.
19. **Destroy test set** (§13). A four-way intersection structure (pieces with `0x100000`) at (20..21, 20..21):
    destroy the piece at (20, 20) alone → that piece is removed with `RemoveOccupant`; the other three pieces of the
    structure go through `RemoveOrReplaceOccupant` (network-manager.md §7.4), and the cells it replaced are re-solved.
20. **Grading threshold** (§9.7). A solved terrain vertex 0.0005 m from its altitude is not written and does not grow
    the rectangle; 0.002 m is written.
21. **Smoothing code** (§9.4). A run whose height solve fails at every level reports `0xF0000006`; one that solves but
    cannot be supported at every level reports `0xF0000008`.
22. **Satisfier tie-break** (§9.1). A fresh satisfier (193 buckets), `Init(0, 1000)`; add vertices 10, 203 and 11 in
    that order (203 mod 193 = 10), no constraints. All widths are 1000, so the pick order is 203, 10, 11 (bucket 10
    holds 203 before 10, head insertion; then bucket 11).
23. **No intersection code** (§8.2). Any drag, including a highway over an existing road: `GetFailureState()` is
    never `0xF0000005`.
24. **Supports by type** (§10.2). Road, rail and street pieces can have support info; a highway or avenue piece never
    has (`GetSupportStructureInfo` returns null), whatever its height.
25. **Plane, flat** (§9.8). Road, straight drag (0, 10) → (10, 10), pen 4, flat terrain at 100 m, nothing built: the
    plane is `(0, 2816, 0, −281600)`, `h` = 100 at every vertex; every regular corner's allowed range becomes
    [65, 135] (from the new-cell range [−400, 600]) and its preferred range [100, 100].
26. **Plane, slope** (§9.8). As test 25 with the terrain altitude `100 + 2x` at vertex x: the anchors are 100 (x = 0)
    and 122 (x = 11); the plane is `(−352, 2816, 0, −281600)` and `h` equals the terrain. Corners at x = 5: allowed
    [75, 145], preferred [110, 110]; at x = 11: [87, 157].
27. **Plane window widening** (§9.8). As test 25 with the two vertices at x = 5 raised to 150: `h` = 100 there, `hi`
    widens to 150: allowed [65, 150], preferred [100, 150]. Lowered to 40 instead: allowed [40, 135], preferred
    [40, 100].
28. **Held anchor** (§9.8). Road (1, 10) → (11, 10), flat at 100 except vertex (1, 11) at 110, smoothing level 0:
    `hA` = 105 (the mean), and the corner at (6, 10) gets `h` ≈ 102.72727, allowed ≈ [67.72727, 137.72726]. With a
    rail piece at (0, 9) (outside the level-0 smoothing set, touching vertex (1, 10)): `hA` = 100 and every `h` is 100.
    With the rail piece at (0, 11) instead (touching (1, 11) only): `hA` = 110, `h`(6, 10) ≈ 105.45455.
29. **Bridge cells leave the piece solve** (§8). The bridge of network-bridges.md test 8: no cell of steps 9 … 20 is
    in the set `+0x84` after `BuildSetOfCellsToBeSolved`; the cells of steps 0 … 8 and 21 … 30 are.
