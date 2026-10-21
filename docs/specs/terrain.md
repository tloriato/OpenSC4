# Terrain (`cSTETerrain`, `cSTESurfaceWater`, `cSTEVolcanoLava`): behaviour spec

Status: round 2 (re-answers17, 2026-10-16: TR1 kinds 2 and 5, TR3–TR18 answered in place). Round 1:
re-specs14a, 2026-10-15. Written from the Windows 1.1.641 executable (terrain code
`0x007405D0`–`0x0074FEA0`, surface water `0x0073CD50`–`0x0073FE70`, lava `0x0075CE60`–`0x0075FAE0`). Every numeric
rule below was checked against the instruction bytes unless it is tagged *[decomp only]*. The save-record facts were
also checked against a corpus of 502 saves in the user's regions.

Scope: the parts of the terrain that the **simulation** reads or changes: the height field and its queries, slope,
water presence and depth, the change messages and their `data4` kinds, the brushes that the disasters and tools
apply, the lava as far as the volcano disaster needs it, the saved records, and the surface-water object that the
traffic, zone and watercraft code query. Drawing (textures, cliffs as art, waves, decals, steam, lighting, the 3D
terrain view) is out of scope; where a function mixes both, only the simulation-visible effects are given.

**Corrects (TR21, 2026-10-16, re-answers30).** The 3D terrain view is **no longer out of scope**: §15.5 now
specifies its creation, object layout, vtables with return channels, `Init` contract and init sequence, because the
terrain creates it during `PostCityInit`, hands its pointer to other components through slot 66, and the view reads
the terrain's vertex array directly. The rest of the drawing work (textures, cliffs as art, waves, decals, steam,
lighting) stays out of scope.

Tags: *[compat]* observable, must match; *[choice]* FreeCity4 decision; *[inferred]* not proven;
*[quirk]* original behaviour that looks unintended but is reproduced, or that a replacement must survive.

Shared material cited and not repeated here:
- streams, records and object references: foundations §2;
- class store, load order and the component table (terrain is component 49): §3.5, §3.6;
- messages, `MessagePost` (queued) and `MessageSend` (synchronous): foundations §5;
- x87 precision (every float operation below rounds to a 24-bit significand, written `r24`): foundations §18;
- the volcano and earthquake disasters, the fire manager and its slots: disasters.md §4, §5, §7, §8, §10;
- the network manager's terrain handler: network-manager.md §7.6;
- the economy modifier of the nation: demand.md §8.2, and [nation.md](nation.md).

---

## 1. Identity and ownership

| Item | Value |
|---|---|
| Terrain class | `cSTETerrain`, CLSID `0xE98F9525` (`GetGZCLSID` `0x007405D0`). Created by the city as component 49 (foundations §3.6), returned by city slot 84 (`GetTerrain`, `+0x150`) |
| Terrain IIDs | `0x00000001`, `0xAB953253` and `0xC70BD37E` (`cISTETerrain`, the component IID) → this; `0x452294AA` (`cIGZMessageTarget2`) → this + 4; `0xE4FDA3D4` (`cIGZSerializable`) → this + 8 (QueryInterface `0x00741E90`) |
| Terrain vtable | `cISTETerrain` at `0x00AB3D18`, slots 0–97 (§15.1) |
| Surface water class | `cSTESurfaceWater`, CLSID `0x0B8DA7DB`, vtable `0x00AB3B00` (slots 0–14, §15.2). Owned by the terrain (terrain `+0x24`) and returned by city slot 83 (`GetSurfaceWater`, `+0x14C`) |
| Lava class | `cSTEVolcanoLava`, object size `0x178`. Not a city component: the volcano disaster instance creates one per eruption (§11) and releases it at the end |
| Terrain view | the 3D terrain view (terrain `+0xE8`, class `0xC9B84E10`), saved through a reference in the terrain's class record (§13.2). Rendering only |

The terrain object holds the vertex array (altitudes, normals, flags). The surface water object holds the per-vertex
water array (depths, velocities, sediment). Most water queries are terrain slots that read the surface water's array
directly (§5); the surface water's own slots add the cell queries and navigability (§10).

---

## 2. Grid, coordinates and state

### 2.1 Grid *[compat]*

- A **cell** is 16 m × 16 m (`CellWidth`, slot 6, constant 16.0). The city has `cellsX × cellsZ` cells (64, 128 or
  256 per side; city slots 93 and 94, read in `Init`).
- **Vertices** sit on cell corners: `vertsX = cellsX + 1`, `vertsZ = cellsZ + 1`. Vertex `(x, z)` is at world
  position `(16x, 16z)`. Its **index** is `x + z × vertsX` (`GetVertIndex`, slot 9, no bounds check).
- `GetVertex(index, &x, &z)` (slot 10): `x = index mod vertsX`, `z = index div vertsX` (unsigned).
- Cell `(cx, cz)` has the corners `(cx, cz)`, `(cx + 1, cz)`, `(cx, cz + 1)`, `(cx + 1, cz + 1)`.
- World positions are metres: `0 ≤ x ≤ sizeX = 16 × cellsX`, same for z. y is the altitude in metres.

### 2.2 Per-vertex records *[compat]*

| Array | Record | Fields read by the simulation |
|---|---|---|
| terrain vertices (`+0x6C`) | `0x24` bytes | `+0x00` altitude (float); `+0x04..+0x0C` normal (render; §15.5.13.2); `+0x16..+0x21` three texture layers `{u16 id, u16 weight}` (render; §15.5.13.3); `+0x22` uint16 flags (§7) |
| surface water (`water +0x18`) | `0x24` bytes | `+0x00` **depth** (float, metres, relative to the ground: water level = altitude + depth); `+0x10` velocity vector; `+0x1C` sediment (float) |

The depth is negative on dry land: almost every height setter writes `depth := seaLevel − altitude` (§6), so a land
vertex above the sea has a negative depth. **"Water" always means depth > 0**, "land" means depth ≤ 0.

### 2.3 Terrain object fields *[compat where a slot returns them; otherwise internal]*

| Offset | Meaning |
|---|---|
| `+0x0C` | initialised flag (byte; slot 5 returns it) |
| `+0x18` | terrain type (int32; slots 41/42; saved in the misc record; 0 in every corpus save) |
| `+0x1C` | the city |
| `+0x24` | the surface water object |
| `+0x28`, `+0x2C` | `cellsX`, `cellsZ` (slots 7 and 8 return them) |
| `+0x30`, `+0x34` | `cellsX − 1`, `cellsZ − 1` |
| `+0x38`, `+0x3C` | `vertsX`, `vertsZ` |
| `+0x40..+0x4C` | the whole-city vertex rectangle `(0, 0, cellsX, cellsZ)` |
| `+0x50`, `+0x54` | `sizeX = 16 × cellsX`, `sizeZ` (metres, float) |
| `+0x58` | city size type (city slot 87) |
| `+0x5C` | vertex count `vertsX × vertsZ` |
| `+0x6C` | the vertex array |
| `+0x70`, `+0x74` | cached minimum and maximum altitude (slots 21, 22; §4.6) |
| `+0x78` | tuning `0x48CD7B20` (shipped 308); saved |
| `+0x7C` | sea level (slot 29) |
| `+0x84..+0xA0` | the 8 neighbour index offsets (for the lava and the flood code) |
| `+0xBC` | the default brush level (§9.2): the view's zoom index. 0 from the constructor (`0x0074C408`); `DoPostCityInit` copies the 3D camera's zoom into it when a camera exists (`0x0074955F`; the camera is the global at `0x00B43DD8`, its `+0xC`) |
| `+0xC0` | "accumulating texture updates" flag (slots 78–80; render) |
| `+0xD8` | tick counter for the reconciliation post (§12.1) |
| `+0xDD` | water-surface display state (slot 83 returns it, slot 84 sets it) |
| `+0xE8` | the terrain 3D view **object base** (§15.5) |
| `+0xEC` | **view-ready** flag, set by `DoPostCityInit` (slot 40, `0x00749540`). `RedisplayTerrain` does nothing until it is set (§8.1) |
| `+0x14` | **`DoPostCityInit` once-only guard** (byte; `0x00749544`, `0x0074954F`) |
| `+0xF0`, `+0xF4` | pointers copied out of the view at `+0x70` / `+0x74` and `AddRef`'d (`0x00749606`–`0x00749634`) |
| `+0xF8` | the view's overlay manager 0 (`view+0x78`), `AddRef`'d (`0x00749637`) |
| `+0x100` | the city's **weather simulator** (city slot 70, set and `AddRef`'d in `Init` `0x00750064`–`0x00750079`, released in `Shutdown`); `DoPostCityInit` calls its slot 5 `DoCompleteSimulation` (§15.5.13.1, TR34) |
| `+0x104`, `+0xFC` | further collaborator pointers taken from the city in `Init` (`0x00750088`, `0x007500AC`) |

**Corrects (TR23).** The `0xA6B79602` update does **not** use "the first data word". At `0x0074AAD9`–`0x0074AAF4`
the handler calls message slot 15 `GetVoid2` (`[eax+0x3C]`), **dereferences** the returned pointer, and compares
that u32 with `+0xBC`, storing it only when it differs:

```
0074aadd  call  dword ptr [eax + 0x3c]   ; message slot 15 = GetVoid2
0074aae0  mov   eax, dword ptr [eax]     ; *void2  <-- the zoom index
0074aae2  cmp   eax, dword ptr [edi+0xb8]; edi = this+4, so terrain +0xBC
0074aaf4  mov   dword ptr [edi+0xb8], eax
```

So the zoom lives at **`*GetVoid2()`**, the same source zones.md §5.2 reads for the same message. A replacement
that read `data1` would track the wrong word.

**Corrects (TR22).** The heading of this table says "otherwise internal". That is wrong for the fields below:
**three other classes hold a `cISTETerrain*` and read terrain-object fields directly**, not through the slot table.
A replacement must reproduce these offsets and their meaning exactly — a slot-only implementation is not enough.

| Holder | where it keeps the terrain | how it gets it |
|---|---|---|
| `cSTESurfaceWater` | water `+0x14` | global `0x00B43CF4` (`0x0073FE86`, `0x0073FEA0`) |
| `cSTEVolcanoLava` | lava `+0x14` | city slot 84 `GetTerrain` `+0x150` (`0x0075E2FF`, `0x0075E307`) |
| `cSTETerrainView3D` | view `+0x20` | the `Init` argument (§15.5) |

**`cSTESurfaceWater` reads (all reads, never writes):**

| Offset | Read by | For |
|---|---|---|
| `+0x38` vertsX | `ResetWaterToSeaLevel` `0x0073D180` (`0x0073D1A3`), `GetWaterDepthForCell` `0x0073D210` (`0x0073D251`) | the vertex-array row stride |
| `+0x38`, `+0x3C` | `Init` `0x0073FE70`, `AnimateWater` `0x0073FDD0` (`0x0073FDDE`/`0x0073FDE1`) | the cell map, the whole-grid rect |
| `+0x28`, `+0x2C` cellsX/Z | `UpdateNeighborConnections` `0x0073D850`, `UpdateFloodWaters` `0x0073EF00` | bounds |
| `+0x40`..`+0x4C` city rect | `Init` `0x0073FE70` (`0x0074002C`–`0x00740035`) | **address-of** the rect, passed to `ResetWaterToSeaLevel` |
| `+0x5C` vertex count | `Init` `0x0073FE70`, `SaveWaterInfo` `0x0073D090` | sizing the water arrays |
| `+0x6C` vertex array | `ResetWaterToSeaLevel`, `FindLocalMinimumOrWater` `0x0073D2B0`, `AltitudeAtWhichFloodingWillOccur` `0x0073D4F0`, `GetWaterType` `0x0073D7F0`, `GetBasinVertices` `0x0073DFF0`, `Rain_Static` `0x0073E350`, `UpdateFloodWaters` `0x0073EF00` | altitude at record `+0x00`, 0x24-byte records |
| `+0x7C` sea level | `ResetWaterToSeaLevel`, `GetWaterType`, `UpdateFloodWaters` | `depth := sea − alt` |
| `+0x84`..`+0xA0` | `FindLocalMinimumOrWater`, `AltitudeAtWhichFloodingWillOccur`, `UpdateFloodWaters` | the 8 neighbour offsets |

It mixes both styles in one function: e.g. `GetWaterDepthForCell` calls **slot 9** `GetVertIndex` (`0x0073D222`)
then reads `+0x38`; `GetWaterType` calls slot 9 then reads `+0x6C` and `+0x7C`.

**`cSTEVolcanoLava` reads:** `+0x6C` is cached into lava `+0x8C` for the whole eruption (`0x0075E316`–`0x0075E319`)
and `+0x84`..`+0xA0` supply the neighbour indices in `DoLavaFlow` `0x0075F4A0`. It also calls `RaiseVertex`
directly (not by slot) and terrain slot 6 `CellWidth` (`0x0075E321`).

**`cSTETerrainView3D` reads `+0x6C`** (§15.5.5) — everything else from slots and from its own cached statics —
**and it also writes into the vertex records**: `LevelCell` `0x00759750` does
`or byte [vertex+i*4+0x22], 0x60` (`0x00759873`) and `RemoveCellLevelling` `0x00758620` does
`and byte [vertex+i*4+0x22], 0x9F` (`0x007586DA`). Those are the §7 hide bits `0x20|0x40` ("the whole cell
hidden"), used as the levelling marks. **So the vertex array must be writable and those two bits must survive**;
a replacement that recomputes `+0x22` from scratch will fight the view. It also reads **record `+0x16`** (a u16
texture/tile id: `ComputeTextureSpansForRow` `0x0075AC5D`, `DrawTerrainSurfaceRows` `0x0075B5F2`) and — in `Pick`
(`0x0075A58A`–`0x0075A61D`) — the record `+0x00` altitudes of a cell's four corners.

**Nobody outside the terrain reads `+0x24`** (the surface-water pointer) — it is terrain-internal, and neither
`cSTEWaterRipple`, `cSTEOverlayManager`, `cSTESelectedTerrainArea` nor `cSTEVertBufExtn` holds a terrain pointer.
The only writes from outside the class are into the **vertex records**, never into the terrain object's own fields.
*[inferred: completeness]* a caller holding the terrain in an untyped `int*` would not be caught by this sweep;
settling it needs a data-flow sweep of the city slot 84 results.

### 2.4 Tuning *[compat]*

Read once per process by `ReadTuningParameters` (`0x0074B6A0`) from exemplar `0x6534284A / 0x88CD66E9 / 1`
("Terrain Properties"):

| Property | Meaning | Code default | Shipped |
|---|---|---|---|
| `0x48CD7B26` | maximum allowed height (slot 24; the raise clamp, §6) | 200 | 2500 |
| `0x48CD7B27` | minimum allowed height (slot 23; the lower clamp) | 5 | 20 |
| `0x48CD7B21` | sea level of a new terrain (clamped ≥ minimum allowed height) | — | 250 |
| `0x48CD7B2A` | flat height of a new terrain (clamped ≥ minimum) | — | 300 |
| `0x48CD7B28` | height scale for a new terrain from an image | — | 3 |
| `0x48CD7B70`..`73` | sea-level range for `SetSeaLevel`: min `−10`, max `200`; and — **Corrects (TR25)** — `0x48CD7B72` is **not** an editor value but `ChangeSeaLevel`'s step (slot 46, stored at `0x00B0D584`, shipped **5.0**, fallback 20.0), and `0x48CD7B73` is the default raise/lower amount (slot 47, `0x00B0D588`, shipped **40.0**, fallback 20.0) | — | −10, 200, 5, 40 |
| `0x48CD7B50` | minimum region size for `LevelAreaStartingAt` (slot 43's levelling) | 401 | 1000 |
| `0x48CD7B51` | `CreateFlatAreas` tolerance scale, `tol = 0.003 × maxAllowedHeight` | — | 0.003 |
| `0x48CD7B52`, `0x48CD7B53` | `CreateFlatAreas` band scales, `f1 = 0.1·M + sea`, `f2 = 0.9·M + sea` | — | 0.9, 0.1 |
| `0x48CD7B65` | cliff slope threshold (render) | — | 0.5 |
| `0x48CD7B20` | stored at `+0x78` and saved | — | 308 |

**Corrects (TR25).** The clamps are two globals, not "the tuning (20, 2500)":
`MinAllowedHeight` (slot 23, `0x0074C690`) is `0x00B0D5E0`, filled from `0x48CD7B27` with **fallback 5.0**;
`MaxAllowedHeight` (slot 24, `0x0074C6A0`) is `0x00B0D5DC`, from `0x48CD7B26` with **fallback 200.0**. The shipped
20 / 2500 hold only when the exemplar loaded; §6 and §4.2 quote them as if absolute.

`GetDefaultTerrainRaiseLowerAmount` (slot 47, `0x0074C660`) is **`fld [0x00B0D588]`**, filled from `0x48CD7B73`
(`0x0074B964`–`0x0074B96F`) — **shipped 40.0, fallback 20.0.** Quirk 19's "returns the constant 20.0 / the ini
value is parsed and thrown away" is **wrong**: the value is read and returned.

---

## 3. Bounds

| Slot | Function | Rule *[compat]* |
|---|---|---|
| 11 | `LocationIsInBounds(x, z)` (`0x0074C610`) | `0 ≤ x ≤ sizeX` and `0 ≤ z ≤ sizeZ` (both ends inclusive). NaN is out |
| — | vertex bounds | `x < vertsX` and `z < vertsZ`, compared **unsigned** (negative is out) |
| — | cell bounds | `x < cellsX` and `z < cellsZ`, unsigned |

---

## 4. Height queries

### 4.1 `GetAltitude(x, z)` (slot 13, `0x00741260`) *[compat]*

The altitude of the ground at a world position, by linear interpolation on one of the two triangles of the cell.

1. If not `LocationIsInBounds(x, z)`: return 0.0.
2. `ix := floor(r24(x × 0.0625))`, `iz := floor(r24(z × 0.0625))`. The original computes the floor with the
   "1.5 × 2^23" trick: round `t − 0.5` to the nearest integer (ties to even), then add 1 if `t` equals that
   integer + 1. The result is exactly `floor(t)` for every in-bounds `t`.
3. Clamp `ix ≤ cellsX − 1` and `iz ≤ cellsZ − 1` (so `x = sizeX` uses the last cell).
4. `fx := r24(x − 16·ix)`, `fz := r24(z − 16·iz)` (metres into the cell, 0..16).
5. Heights: `h00 = (ix, iz)`, `h10 = (ix + 1, iz)`, `h01 = (ix, iz + 1)`, `h11 = (ix + 1, iz + 1)`.
6. The diagonal always runs from `(ix, iz)` to `(ix + 1, iz + 1)`:
   - if `fx ≤ fz` (the triangle `h00, h01, h11`):
     `h := h00 + r24(r24(r24((h11 − h01) × fx) + r24((h01 − h00) × fz)) × 0.0625)`;
   - otherwise (`fx > fz`, or a NaN compare; the triangle `h00, h10, h11`):
     `h := h00 + r24(r24(r24((h11 − h10) × fz) + r24((h10 − h00) × fx)) × 0.0625)`.

   Every subtraction is rounded too (`r24`), and the final addition is rounded.

*[quirk]* The query **ignores** the triangulation-flip flags of §7 (`0x0800`, `0x1000`, `0x2000`): a flipped cell is
drawn with the other diagonal, but its altitude is still read across `(ix, iz)–(ix + 1, iz + 1)`.

### 4.2 The other height reads *[compat]*

| Slot | Function | Rule |
|---|---|---|
| 12 | `GetAltitude(index)` (`0x007413C0`) | `index ≥ vertexCount` (unsigned) → 0.0; else the vertex altitude |
| 14 | `GetAltitudeAtVertex(x, z)` (`0x007413E0`) | vertex out of bounds → 0.0; else the altitude |
| 15 | `GetAltitudes(rect*, out, rowStride, colStride)` (`0x00742A00`) | copies the altitudes of an inclusive vertex rectangle *[decomp only]* |
| 17 | `GetCellCornerAltitudes(x, z, out[4])` (`0x007415F0`) | only if the **cell** is in bounds; order `(x, z)`, `(x, z + 1)`, `(x + 1, z + 1)`, `(x + 1, z)`. Out of bounds: `out` is untouched |
| 19 | `GetAltitudeAtNearestGrid(x, z)` (`0x00741420`) | `LocationIsInBounds` else 0.0; vertex `(round(x / 16), round(z / 16))`, rounding to nearest, ties to even; no further clamp |
| 20 | `GetAltitudeCheckSum(rect*)` (`0x007414A0`) | the sum of the altitudes over an inclusive vertex rectangle; 0.0 for an invalid rectangle |
| 21, 22 | `MinHeight`, `MaxHeight` | the cached `+0x70`, `+0x74` (§4.6) |
| 23, 24 | `MinAllowedHeight`, `MaxAllowedHeight` | the tuning (20, 2500) |

### 4.3 Slope: `GetFaceNormal(x, z, out)` (slot 26, `0x00742B10`) *[compat]*

1. Only if `0 ≤ x < sizeX` and `0 ≤ z < sizeZ` (upper ends **exclusive**). Otherwise `out` is untouched.
2. `ix, iz, fx, fz` as in §4.1 (no clamp is needed here). The triangle test is `fz < fx` (the same split as §4.1).
3. Unnormalised normal, each step rounded:
   - `fz < fx`: `nx := r24(0 × d) − 16(h10 − h00)` and `nz := 16(h10 − h00) − 16(h11 − h00)`;
   - otherwise: `nx := 16(h01 − h00) − 16(h11 − h00)` and `nz := r24(0 × d) − 16(h01 − h00)`;
   with `d = r24(h11 − h00)`, and `ny := 256`. Algebraically: `nx = −16(h10 − h00)`, `nz = −16(h11 − h10)` in the
   first case, and `nx = −16(h11 − h01)`, `nz = −16(h01 − h00)` in the second.
4. `s := 1 / sqrt(nx² + nz² + 65536)` (sum in that order) and `out := (nx·s, 256·s, nz·s)`.

The slope angle of the face is `acos(out.y)`. Slots 27 (`GetLerpedNormal`) and 28 (`GetVertexNormal`) return the
smoothed render normals; no simulation code in this spec reads them.

### 4.4 Worked example *[compat]*

Cell (0, 0) with `h00 = 100`, `h10 = 104`, `h01 = 108`, `h11 = 120`:

| Query | Triangle | Result |
|---|---|---|
| `GetAltitude(4, 12)` | `fx ≤ fz` | 100 + (12·4 + 8·12) / 16 = **109** |
| `GetAltitude(12, 4)` | `fx > fz` | 100 + (16·4 + 4·12) / 16 = **107** |
| `GetAltitude(8, 8)` | `fx ≤ fz` | 100 + (12·8 + 8·8) / 16 = **110** (the other formula also gives 110) |
| `GetAltitude(0, 0)` | `fx ≤ fz` | **100** |
| `GetAltitude(−0.001, 0)` | out of bounds | **0** |
| `GetFaceNormal(12, 4)` | `fz < fx` | n = (−64, 256, −256) / sqrt(4096 + 65536 + 65536) |

### 4.5 Edge case: the last row and column

For a 64-cell city, `GetAltitude(1024, 1024)` uses cell (63, 63) with `fx = fz = 16` and returns `h11` of that
cell, the corner vertex (64, 64). `GetFaceNormal(1024, z)` is out of bounds (untouched output).

### 4.6 The cached minimum and maximum *[compat]*

`+0x70` and `+0x74` are recomputed over **all** vertices only by `Create` (new terrain), `CreateFlatAreas` and
`RedisplayTerrain` with the terrain flag (after the view is ready, §8.1). Load reads them from the misc record
(§13.1). *[quirk]* So they go stale: in 67 of the 502 corpus saves the stored values differ from the real minimum
and maximum of the stored altitudes. A replacement must return the cached values, not a fresh scan.

---

## 5. Water queries on the terrain

All read the surface water's depth array. Water level = altitude + depth.

| Slot | Function | Rule *[compat]* |
|---|---|---|
| 29 | `GetSeaLevel()` | `+0x7C` |
| 30 | `GetWaterLevel(x, z)` (`0x007449B0`) | not `LocationIsInBounds(x, z)` → 0.0; else vertex `(floor(r24(x × 0.0625)), floor(r24(z × 0.0625)))` (the floor of §4.1, no further clamp; `x = sizeX` gives the vertex column `cellsX`): `depth + altitude` |
| 31 | `GetWaterLevel(index)` (`0x00744980`) | `index ≥ vertexCount` (unsigned) → 0.0; else `depth + altitude` |
| 32 | `GetLerpedWaterLevel(x, z)` (`0x007444E0`) | §5.1 |
| 33, 92 | `GetWaterDepthAtVertex(x, z)` (`0x00744AB0`) / `(index)` (`0x00744A80`) | vertex out of bounds (unsigned) or `index ≥ vertexCount` → 0.0; else the depth (may be negative) |
| 18 | `GetMaxSurfaceLevel(x, z)` (`0x00744620`) | cell in bounds, else 0.0. The maximum over the corners `(x, z)`, `(x, z + 1)`, `(x + 1, z + 1)`, `(x + 1, z)` of `altitude + max(depth, 0)` |
| 34 | `CellHasWater(x, z)` (`0x00743FA0`) | cell out of bounds → false. True if **any** of the corners `(x, z)`, `(x + 1, z)`, `(x, z + 1)`, `(x + 1, z + 1)` has `depth > 0`. A NaN depth is not water |
| 35 | `CellHasLand(x, z)` (`0x00744030`) | cell out of bounds → false. True if any corner has `depth ≤ 0`. A NaN depth is not land |
| 36 | `IsLandVertex(x, z)` (`0x00747680`) | vertex out of bounds → false; `depth ≤ 0` |
| 37 | `IsWaterVertex(x, z)` (`0x007476D0`) | vertex out of bounds → false; `depth > 0` |
| 38 | `IsSeaVertex(x, z)` (`0x00741B30`) | vertex out of bounds → false; `altitude < seaLevel` |
| 53 | `NumberOfWaterVertices(rect*)` | counts `depth > 0` in an inclusive rectangle *[decomp only]* |
| 55 | `GetSedimentValue(x, z)` (`0x007448C0`) | not `LocationIsInBounds(x, z)` → 0.0; else the sediment field |

The boolean slots return their result in AL only (the upper bytes of EAX are not cleared on every path); callers test
AL. A cell can be both "has water" and "has land" (a shore cell).

### 5.1 `GetLerpedWaterLevel(x, z)` (slot 32, `0x007444E0`) *[compat]*

Checked in the bytes (TR3). Every operation rounds (`r24`).

1. Not `LocationIsInBounds(x, z)` → 0.0.
2. `tx := r24(x × 0.0625)`, `tz := r24(z × 0.0625)`; `ix := floor(tx)`, `iz := floor(tz)` (the floor of §4.1).
   **No clamp**: at `x = sizeX`, `ix = cellsX` and the `ix + 1` corner below is read from the flat index, which is
   the first vertex of the next row (or past the array on the last row) *[quirk]*.
3. `fx := tx − ix`, `fz := tz − iz`.
4. With `L(v) := depth(v) + altitude(v)`, `i := ix + iz × vertsX`:
   `a := L(i) + (L(i + 1) − L(i)) × fx`, `b := L(i + vertsX) + (L(i + vertsX + 1) − L(i + vertsX)) × fx`,
   result `a + (b − a) × fz`.

---

## 6. Height setters and the water bookkeeping *[compat]*

Every setter skips a **locked** vertex (flag `0x4000`, §7). The clamps use the tuning (min 20, max 2500).

| Function | Rule |
|---|---|
| `RaiseVertex(i, d)` (`0x00744CA0`) | `alt := r24(alt + d)`; if `alt > max`: `alt := max`. `depth := r24(sea − alt)` |
| `LowerVertex(i, d)` (`0x00744AF0`, exported as `RecomputeWaterLevel`) | `alt := r24(alt − d)`; if `alt < min`: `alt := min`. `depth := r24(sea − alt)` |
| `SetAltitudeAtVertex(i, h)` (slot 50, `0x00744E60`) | `level := depth + alt`; `alt := h`. If the old depth ≤ 0 (land): if `sea ≤ h`, **stop** (the depth keeps its old value); else `level := sea`. Then `depth := level − h`. No clamp |
| `SetAltitude(x, z, h)` (slot 51, `0x00744F90`) | `i := (cellsX + 1) × z + x` (no bounds check). If `alt < h` and the vertex is not locked: `alt := r24(r24(h − alt) + alt)`, clamp to max, `depth := sea − alt`. If `alt > h`: `LowerVertex(i, alt − h)`. Equal or NaN: nothing |
| `SetAltitude(rect*, h, redisplay, data4)` (slot 49, `0x00744F10`) | `SetAltitudeAtVertex` for every vertex of the inclusive rectangle, z outer, x inner. If `redisplay`: first **reset the water** of the rectangle (`depth := sea − alt` where `alt < sea`, else `depth := 0`; `0x00744CF0`), then `RedisplayTerrain(1, 1, rect, data4)` |
| `LockAltitude(rect*, lock)` (slot 52) | sets or clears `0x4000` on every vertex of the inclusive rectangle |
| `SetSeaLevel(h)` (slot 45, `0x00743550`) | accepted only if `−10 ≤ h ≤ 200` (tuning `0x48CD7B70`/`71`). Sets `+0x7C`, resets every vertex's water to `depth := sea − alt` (other water fields := 0), posts `0x89171496`, then `RedisplayTerrain(1, 1, whole city, 0)` |
| `SetTerrainType(t)` (slot 41, `0x00749A00`) | if `t` differs: store it, post `0xC922EB91` with data1 = `t`, then `RedisplayTerrain(0, 0, whole city, 0)` (only the texture message, §8.1) |

*[quirk]* `SetAltitudeAtVertex` on a land vertex raised to or above the sea leaves the stale (negative) depth, so the
vertex stays land; lowered below the sea it becomes sea water at the sea level. A water vertex keeps its water
**level** and only its depth changes (it becomes land when raised above that level).

*[quirk]* `RaiseVertex` and `LowerVertex` overwrite the depth with `sea − alt` even when `d = 0`, so a lake vertex
(water above the sea) touched by either loses its lake water. The lava does this (§11.4).

---

## 7. Vertex flags (`+0x22`, uint16) *[compat]*

| Bit | Meaning |
|---|---|
| `0x0008` | cliff mark (render; set by the cliff texturing) |
| `0x0010` | shore or cliff render mark (seen in saves) |
| `0x0020` | even triangle hidden (cell flag 1) |
| `0x0040` | odd triangle hidden (cell flag 2); `0x20 | 0x40` = the whole cell hidden |
| `0x0200` | cell flag 4 |
| `0x0400` | mark set by brush component kind 7 (§9.4) |
| `0x0800` | triangulation flipped because of a cliff |
| `0x1000` | internal triangulation flip (slot 93) |
| `0x2000` | external triangulation flip (slot 94); also cell flag 8 |
| `0x4000` | **locked**: every height setter skips the vertex (slot 52) |

`IsTriangulationFlipped(x, z)` (slot 70) tests `0x3000`. `GetTerrainCellFlags` / `SetTerrainCellFlags` (slots 82,
81) pack the cell flags 1, 2, 4, 8 as the bits `0x20`, `0x40`, `0x200`, `0x2000` of the cell's `(x, z)` vertex. The
hide slots 71–77 work on the same bits. Values seen in the corpus: `0x0`, `0x8`, `0x10`, `0x18`, `0x200`, `0x208`,
`0x260`, `0x268`, `0x1800`, `0x1808`, `0x1810`, `0x1818`, `0x2000`; never `0x4000` (locks are transient).

---

## 8. `RedisplayTerrain` and the change messages

### 8.1 `RedisplayTerrain(terrainFlag, waterFlag, rect*, data4)` (slot 68, `0x00749850`) *[compat]*

`rect` is an **inclusive vertex rectangle** `(x0, z0, x1, z1)` (left, top, right, bottom).

1. If the view-ready flag (`+0xEC`) is clear: return. Nothing is posted.
2. If `terrainFlag`: recompute the cached minimum and maximum (§4.6), recompute the normals and cliff marks in the
   rectangle grown by 1 (render), and post **`0x278E6003`** (terrain heights changed) with
   `data1 = (x0 << 16) + x1`, `data2 = (z0 << 16) + z1`, `data3 = 0`, `data4 = data4`.
3. If `waterFlag`: post **`0x678E6E3A`** (water heights changed) with the same four values.
4. Always (when the view is ready): post `0x870BC918` (texture redisplay). Only the renderer reads it. Its words
   (TR11, checked at `0x00749894`–`0x007499EE`):
   - the **render rectangle** `r`: with the terrain flag, `rect` padded by 1 on every side and clamped to
     `[0, cellsX] × [0, cellsZ]` (`PadRectangle` `0x00741E30`; the union with the unpadded `rect` only matters for
     a rectangle outside the city); without it, `rect` itself;
   - `data1 = (r.x0 << 16) + r.x1`, `data2 = (r.z0 << 16) + r.z1` (the packing of step 2);
   - `data3 = (trunc(lo) << 16) + trunc(hi)`, where `lo, hi` is the altitude band that the 3D view redraws
     (`ComputeMinAndMaxAltitudeForTerrainRedisplay` `0x00742660`): the minimum and maximum altitude of the vertices
     of `r` (the cached §4.6 values when `r` is the whole city), widened by 20 m and merged with the 3D view's own
     band, `lo := 0` when the camera sees a city edge that `r` touches, then `lo ≥ 0` and `hi` clamped between the
     sea level and the maximum allowed height *[render; decomp only for the widening]*;
   - `data4 = data4`.

   A replacement that keeps no 3D view may post `lo, hi` = the plain minimum and maximum altitude over `r`
   *[choice]*; nothing in the simulation reads data3.

All three are posted with `MessagePost(msg, false)` (queued, normal priority, through the helper `0x00743380`), in
this order. Receivers see them on the next message pump, never during the edit.

Slot 69 (`0x00743460`) takes two cell corners `(a, b)`, sorts them, sets `x1 := max + 1` clamped to `cellsX` (and
the same for z), and calls slot 68. It converts a cell rectangle to its vertex rectangle.

**Unpacking.** The consumers recover the **cells** touched by the vertex rectangle as `x0 − 1 … x1` and
`z0 − 1 … z1`, clamped to the city (building-development.md §4.11, network-manager.md §7.6, landfill.md §3.2,
flora.md, power.md, neighbors.md, traffic-network.md).

### 8.2 The `data4` kinds *[compat]*

`data4` is only `RedisplayTerrain`'s fourth argument. Every call site of slots 68 and 69 in the executable, by kind:

| `data4` | Who | Where |
|---|---|---|
| 0 | internal and non-height changes: `SetSeaLevel`, `SetTerrainType` (texture only), `CreateFlatAreas`, `CreateShoreCliffs`, `StopAccumulatingTextureUpdates` and the volcanic-ash ticks (texture only), the debug flood (`DoFloodFlowStep`, water only), the lot configuration editor | terrain `0x00743550`, `0x00749A00`, `0x0074A220`, `0x0074C0F0`, `0x00749A70`, `0x00749AF0`, `0x007467F0` |
| 1 | the terraforming tool in **god** mode (scrubber flag set, §9.6); `ChangeTerrainLevel` (`0x00746440`); the edge reconciliation with the neighbours (`LockEdgeVerticesAndSmoothen` `0x00748C90`); a view message `0x470BD214` (whole city, §12.1) | terrain, terrain scrubber |
| 2 | the terraforming tool in **mayor** mode (cost charged per application, §9.6) *[inferred: the mode names]*; the script / cheat command `ApplyTerrainBrush` (§9.5) | terrain scrubber, terrain `0x0074A470` |
| 3 | disasters and water: the earthquake (`0x00575C40`, after the fault brushes), the pipe-burst sinkhole (`0x0057C1A0`), every effect brush (volcano deformation, §9.5), the lava's solidification (`0x0075F4A0`), the surface water's animation (`AnimateWater` `0x0073FDD0`, **water flag only**), and any `tBrushParameters` left at its default (`0x004D8B90` sets 3) | §9.5, §11 |
| 4 | the network height constraint solver (`0x00644530`): road and rail grading. It sets each vertex whose change exceeds 0.001 m (slot 50), grows the rectangle by 1, and wraps `RedisplayTerrain(1, 1, rect, 4)` in `BeginLongTerrainOperation` / `EndLongTerrainOperation` | network tool |
| 5 | levelling: demolition `ModifyTerrainHeight` with reason 5 (`0x00467BB0`), the lot developer's area levelling through it, and the landfill developer (`HandleZoneOverlap` `0x006B96E0`) | demolition.md §4.7, landfill.md §4.5.3 |
| 6 | `cSC4NetworkTool::InsertTunnelPiece` (`0x00628390`): the tunnel cut | network tool |
| any | demolition `ModifyTerrainHeight` passes its caller's reason through | demolition.md §4.7 |

So the network manager's rule (network-manager.md §7.6) reads: kind 3 = a disaster or water change (pipes and
subways are destroyed too), kind 4 = its own grading (nothing is destroyed), kind 5 = levelling (the handler does
nothing), kind 6 = a tunnel cut (pieces with flag `0x80000` survive). The power simulator ignores 4 and 5, and the
landfill reacts only to 1, 2 and 3.

### 8.3 The long-operation messages *[compat]*

`BeginLongTerrainOperation` (slot 87, `0x00743B40`) posts `0xEA3B0AF1` and `EndLongTerrainOperation` (slot 88,
`0x007443A0`) posts `0x6A3B0AFD`, both with data1..data4 = 0, queued. There is **no nesting counter**: each call
posts one message, and the receivers (the network manager's `longTerrainOp`, network-manager.md §7.6) keep their own
state. The earthquake and the network grading use them.

### 8.4 Other terrain messages

| Message | Posted by | Data |
|---|---|---|
| `0x89171496` sea level changed | `SetSeaLevel` | 0 |
| `0xC922EB91` terrain type changed | `SetTerrainType` | data1 = the new type |
| `0x6AB97795` water display state | `SetWaterSurfaceDisplayState` (slot 84, `0x007434E0`): **only when the new byte differs** from `+0xDD`; it stores the byte, then posts (queued, the §8.1 post) | data1 = the state (zero-extended byte); the rest 0 |
| `0xCA63D517` TerrainReconciliation | the animation tick (§12.1) | 0 (foundations §11.6) |

---

## 9. Brushes

### 9.1 Descriptions *[compat]*

Brushes are exemplars of type `0x6534284A`, group `0x6A01FC2A`, read once at `StaticInit`
(`ReadBrushDescriptions` `0x0074FDC0`) into a global map keyed by the exemplar **instance** (the brush ID). Each has
up to four **components**; component `i` (0–3) uses the properties `0x8A020202 + 0x100·i + k`:

| `k` | Meaning |
|---|---|
| `+3` | kind, 0–11 (a description with a kind > 11 is rejected) |
| `+0` | radius, 5 floats (one per brush level) |
| `+1` | strength, 5 floats |
| `−1` | bool: orient the bitmap along the stroke direction |
| `−2` | bitmap instance (a grey image, type `0x7AB50E44`, group `0x891B0E1A`) |
| `+2` | uint8 neutral grey value |
| `+4`..`+7` | texture, angle (`× 1/360`), and render values |

Whole-brush properties (`ReadBrushWholeProperties` `0x00747EC0`): the random-walk angle step `0x8A020102` in
degrees, **0.5 when the property is missing**, stored `× π/180`; UI and cost values (render and tool only).

**The two angles of a description** (TR8). Each description holds its own pair, so they are per description and
shared by every caller and every city of the process (the map is static):
- the **walk angle** (`+0x68`): when the step is non-zero (NaN counts as non-zero) the walk flag (`+0x04`) is set and
  the walk angle starts at `RandomRange(0, 2π)` of the terrain's own generator (the global `cRZRandom` at
  `0x00B4BC78`, seeded once per process from the clock XOR `0x4C09C543`, `0x00A74FF0`); with a zero step the flag is
  clear and the angle is 0;
- the **stroke angle** (`+0x6C`): always starts at **0**.

None of the shipped brushes in the table below has `0x8A020102`, so all of them have the walk flag set with a
0.5° step. Their components are all oriented, so they draw with the stroke angle and the walk angle only consumes
draws (§9.3 step 2).

The shipped brushes the simulation uses:

| ID | Name | Component |
|---|---|---|
| `0x11` | EarthquakeFault | kind 0, radius 4 (all levels), strength 0.001, neutral 128, oriented, bitmap `0x0A0B485E` |
| `0x12` | EarthquakeFaultFlipped | the same with bitmap `0xCA131C51` |
| `0x14` | PipeSinkHole | kind 0, radius 16, strength 0.01, 0.008, 0.006, 0.0045, 0.0035, neutral 255, oriented, bitmap `0x2A05BB19` |
| `0xCA313035` | VolcanoBrush | two kind-0 components |

### 9.2 `ApplyBrush(brushID, params*)` (slot 57, `0x0074F0E0`) *[compat]*

`tBrushParameters` (`0x38` bytes):

| Offset | Field | Default (`0x004D8B90`) |
|---|---|---|
| `+0x00`, `+0x04` | from x, z (metres) | — |
| `+0x08`, `+0x0C` | to x, z (metres) | — |
| `+0x10` | level index 0–4; < 0 means the terrain default (`+0xBC`) | −1 |
| `+0x14` | redisplay (byte) | 1 |
| `+0x18` | strength scale | 1.0 |
| `+0x1C` | radius scale | 1.0 |
| `+0x20` | `data4` for `RedisplayTerrain` | 3 |
| `+0x24..+0x30` | out: the touched vertex rectangle | — |
| `+0x34` | target height (levelling components) | 100.0 |

1. Set the out rectangle to empty.
2. Look up `brushID`. Not found: return false.
3. Apply the components (§9.3).
4. If `redisplay` and the out rectangle is not empty: `RedisplayTerrain(1, 1, out, data4)`.
5. Return true.

### 9.3 Applying the components (`ApplyBrushComponents` `0x0074D730`) *[compat]*

1. The centre vertex is `vx := trunc(r24(to.x × 0.0625))`, `vz := trunc(r24(to.z × 0.0625))` (truncation, `_ftol`).
   If `vx ≥ vertsX` or `vz ≥ vertsZ` (unsigned): nothing is applied (and no random draw is made).
2. The **angles** (checked at `0x0074D7BD`–`0x0074D855`), both updated once per call, before the components and
   whatever their kinds:
   1. Walk angle: if the walk flag is set, draw `RandomUint32Uniform(5)` from the terrain generator (`0x00B4BC78`).
      Not 0: `walk := r24(step + walk)`, and if `walk ≥ 2π` (f32 `6.2831855`): `walk −= 2π`. 0: `walk := r24(walk −
      step)`, and if `walk < 0`: `walk += 2π`. If the flag is clear: `walk := 0`.
   2. Stroke angle: if `from.x ≠ to.x` or `from.z ≠ to.z` (a NaN counts as different): `stroke := f32(atan2(to.z −
      from.z, to.x − from.x) + f32(π/2))` (x87 `fpatan`, then the add, rounded to f32 on the store). When
      `from = to` the previous stroke angle of that description is reused *[quirk: global state shared by every
      caller]*.
   3. Each component uses `stroke` when it is oriented (component property `−1`), else `walk`.
3. For each component: `strength := strength[level] × params.strength`, `radius := radius[level] × params.radius`,
   and the kind's operation runs on the bitmap footprint (§9.4).

**Bitmap footprint** (`0x00742180`). With `W × H` the bitmap size, `q := r24(radius / W)`, `k := r24(16 / q)`,
`c := f32(cos(angle))`, `s := f32(sin(angle))` (x87 `fcos` / `fsin` on the f32 angle, rounded on the store at
`0x00742216`–`0x0074221E`; no snapping to 0 or ±1, TR16):
- the footprint is a `w × h` metre rectangle with `w := radius`, `h := r24(H × q)`, centred on `(cx, cz) :=
  (16vx, 16vz)`, rotated by the angle; its vertex rows and column spans come from `ComputeSpansForRectangle` (below);
- rows `z` ascending (outer), and in each row the span's vertices `x` ascending (inner). For the first vertex `x0`
  of the row: `U := s·(dz·k) + c·(dx·k) + W/2`, `V := c·(dz·k) − s·(dx·k) + H/2` with `dz = z − vz`, `dx = x0 − vx`;
  then per step in x, `U += c·k`, `V −= s·k`;
- the pixel is `(trunc(U), trunc(V))`. If `u < W` and `v < H` (unsigned) the operation runs with that grey value `b`;
  when it reports a change, the vertex joins the out rectangle.

*[quirk]* Truncation maps U in (−1, 0) to column 0, so the footprint reaches half a pixel further on the low side.

*[quirk]* Because `s` and `c` are not snapped, an angle that is a multiple of π/2 in exact arithmetic still gives a
tiny non-zero `s` or `c` (for example `f32(sin(f32(π)))` ≈ −8.7e−8). The rectangle's edges are then not exactly
axis-aligned, and the spans below can differ by one vertex at the rectangle's sides from those of an exact
rectangle. The original computes this way; a replacement must too (TR16).

**The rectangle's corners** (`0x00740AB0`). With `a := −w/2`, `e := −h/2` (the code multiplies by −0.5), the local
corners in order are `p0 = (a, e)`, `p1 = (a, −e)`, `p2 = (−a, −e)`, `p3 = (−a, e)`, and each maps to
`x := r24(r24(c·px) − r24(s·pz)) + cx`, `z := r24(r24(s·px) + r24(c·pz)) + cz` (each step rounded). The edges are
`p0→p1→p2→p3→p0`.

**`ComputeSpansForRectangle(corners, start[], end[], &zMin, &zMax)`** (`0x00740BB0`, checked in the bytes, TR7). It
scan-converts the rotated rectangle; it is **not** the bounding rectangle:
1. `zMin`, `zMax` := the minimum and maximum over the four corners of `trunc(r24(z × 0.0625))` (`_ftol`, toward
   zero). Then `zMin := max(zMin, 0)` (signed); `zMax := cellsZ` if `zMax > cellsZ` compared **unsigned** (so a
   negative `zMax` also becomes `cellsZ`).
2. For each row `z` from `zMin` to `zMax` (signed, inclusive): `Z := 16z`; `start[z] := INT32_MAX`,
   `end[z] := INT32_MIN`. For each edge `i = 0..3` from corner `i` to corner `j = (i + 1) mod 4`, the edge
   **crosses** the row when `zi ≤ Z < zj` or `zj ≤ Z < zi` (half-open, so a horizontal edge never crosses, and a
   NaN never crosses). For a crossing edge:
   - if `xi = xj` (exact compare): `col := trunc(r24(xi × 0.0625))`;
   - else `x := r24(xi + r24(r24(Z − zi) × r24(r24(xj − xi) / r24(zj − zi))))`, `col := trunc(r24(trunc(x) ×
     0.0625))` (the crossing is truncated to whole metres first; the result equals `trunc(x / 16)`);

   and `start[z] := min(start[z], col)`, `end[z] := max(end[z], col)` (signed).
3. Then `start[z] := max(start[z], 0)` (signed) and `end[z] := end[z] + 1`, set to `cellsX` if larger
   **unsigned**.

The footprint loop visits `x` from `start[z]` to `end[z]` inclusive (signed compare), so:
- a row that no edge crosses (`start = INT32_MAX`, `end` → `cellsX`) is empty;
- the first row is often empty: with the rectangle's lowest corner at `z_lo`, the row `trunc(z_lo / 16)` lies at
  `Z < z_lo` unless `z_lo` is a multiple of 16; likewise a top corner exactly on a row is not crossed. The rows
  that can hold vertices are those with `z_lo ≤ 16z < z_hi`;
- each column span runs from the leftmost crossing column to **one past** the rightmost crossing column, so it
  includes the vertex just right of the rectangle, and, through the truncation, the vertex at or just left of its
  left side (the bitmap test then decides);
- a row whose crossings all lie left of `x = −16` gets `end + 1 ≤ −1`, which the unsigned clamp turns into
  `cellsX`: the whole row is visited, and the bitmap test rejects the extra vertices.

The original keeps `start[]` and `end[]` in 256-entry stack arrays; row 256 of a 256-cell city is written one entry
past each array, into stack words that nothing reads afterwards. A replacement sizes them `cellsZ + 1`.

*Worked example* (the sinkhole of §9.5 with a square bitmap and angle 0, so `w = h = 16`, `c = 1`, `s = 0`):
centre (160, 160), corners (152, 152), (152, 168), (168, 168), (168, 152). Rows 9..10. Row 9 (`Z = 144`): no
crossing, empty. Row 10 (`Z = 160`): the edges p0→p1 (x 152, column 9) and p2→p3 (x 168, column 10) cross; the span
is 9..11. The bitmap test keeps only vertex (10, 10) (`U = W/2`); vertices 9 and 11 give `U = −W/2` and `3W/2`.

### 9.4 Component kinds *[compat]*

The dispatcher (`ApplyBrushComponents` `0x0074D730`) selects the operation by the component kind with a jump table
at `0x0074DB64`, kinds 0–10 (a kind > 11 is rejected at load, §9.1). Each kind's entry point is given below.

| Kind | Operation | Entry |
|---|---|---|
| 0 | raise or lower: §9.4.1 | `0x00745C60` |
| 1 | flora painting: §9.4.2 | `0x00745A40` |
| 2 | talus smoothing: §9.4.1 | `0x00745DE0` |
| 3 | object scatter with a per-cell count: §9.4.3 | `0x00749130` |
| 4 | object placement (the animal brushes): §9.4.2 | `0x00749310` |
| 5 | level toward the target height: §9.4.1 | `0x00745BA0` |
| 6 | hydrological erosion (no bitmap footprint): §9.4.4 | `0x007479B0` |
| 7 | mark: sets vertex flag `0x0400`: §9.4.2 | `0x00740820` |
| 8 | lake (grey-modulated levelling): §9.4.5 | `0x00745CD0` |
| 9 | rain (surface water): §9.4.5 | `0x0073EAB0` |
| 10 | spring (surface water): §9.4.5 | `0x0073D4D0` |

**What each kind returns** (§9.4.6). Kinds 0–5, 7 and 8 return a **per-vertex callback** pointer; the bitmap
footprint (`FUN_00742180`, §9.3) or the solid footprint (`FUN_00742440`, §9.4.6) then runs it once per vertex and keeps
the vertices where it reports a change. Kinds 6, 9 and 10 do **not** use the footprint: they run once per
component and write the out rectangle themselves. (Kind 9 has a per-vertex alternative selected by a surface-water
mode byte that this build never sets; see §9.4.5.)

**Per-kind pre-steps** (`0x0074D91D`–`0x0074D94C`), before the kind is selected:
- kind 1: nothing (the kind-1 branch below decides);
- kind 2: if the scaled strength `> 1.0` it is **capped to 1.0** (`fcomp` against `0x00A81228` = 1.0f at `0x0074D92E`).
  The cap uses the raw compare, so a NaN strength is not capped.
- every other kind: no pre-step.

**After the operation**, per kind:
- kind 2: the out rectangle grows by **1** vertex on every side, clamped to `0 … cellsX` / `0 … cellsZ` (`0x0074D977`
  region, signed compares) *[quirk: clamped to the cell counts, not the vertex counts]*
- kind 7: the out rectangle is grown again by `FUN_00748040`;
- all others: unchanged.

**Random draws.** The walk-angle draw of §9.3 step 2 (one `RandomUint32Uniform(5)` of the terrain generator at
`0x00B4BC78` per in-bounds application, and only when the description's walk flag is set — true for every shipped
brush, §9.1) is the only draw in a **height** brush. Kinds 0, 2, 5, 6, 8, 9 and 10 draw nothing themselves, so the
count "one terrain RNG draw per in-bounds `ApplyBrush`" is **confirmed** for those, including kind 8. **Kinds 1, 3
and 4 draw from the same generator per vertex** (§9.4.2, §9.4.3): kind 1 up to four draws
(`RandomUint32Uniform(100)`, two `RandomFloatUniform`, then `RandomUint32Uniform(t->+0x108)`), kind 3 up to three
`RandomFloatUniform`, kind 4 exactly two. So an application of `FloraBrush`, `Animals` or a kind-3 brush consumes
`1 + (draws per vertex that passes its gate)`, not 1.

**The change message.** No kind posts the terrain's own change message. The only post is step 4 of `ApplyBrush`
(§9.2): `RedisplayTerrain(1, 1, out, data4)` with the **caller's** `data4` (`tBrushParameters +0x20`), and only when
the caller set the redisplay byte and the out rectangle is non-empty. So every kind posts `0x278E6003` with the
`data4` its caller chose: 3 for the disasters and the effects manager, 2 for the mayor-mode tool and
`ApplyTerrainBrush`, 1 for the god-mode tool (§9.5, §9.6). Kinds 6, 9 and 10 also post nothing extra; their water and
height edits reach the view through that same single message. **Kind 1 is different**: when it refuses a placement
because the vertex is wet it posts its own message through the global message service at `0x00B43CB8`
(`0xAA62C9D5`, §9.4.2) — an occupant-side message, not `0x278E6003`. Kinds 3, 4 and 7 post nothing of their own.

#### 9.4.1 Kinds 2 and 5 *[compat; checked in the bytes, TR1]*

Both read the grey value `b` and the scaled strength, and compute `step := r24(r24(b × strength) × (1/256))`.
`b = 0` → no change (false).

**Kind 5, level** (`0x00745BA0`), with `T` the target height (`tBrushParameters +0x34`): `d := r24(alt − T)`.
- `d > 0`: `d ≤ step` → `SetAltitudeAtVertex(i, T)` (slot 50, §6); else (`d > step`) → `LowerVertex(i, step)`.
- `d < 0`: `d < −step` → `RaiseVertex(i, step)`; else `SetAltitudeAtVertex(i, T)`.
- `d = 0` or NaN: no change (false).

Every other branch reports a change, also for a locked vertex.

**Kind 2, talus smoothing** (`0x00745DE0`). Two thresholds come from the terrain ini (§9.7; `ReadTerrainIniFile`
`0x0074DC30`, section `WindErosion`, key `TalusAngle`, an **integer** number of degrees, × π/180 at `0x0074E1C2`;
code default 0.79 rad at `0x00B0D59C`, **shipped 5** — §9.7): the load path precomputes
`Tedge := f32(tan(angle) × 16)` (`fptan` at `0x0074E1DA`, × the 16.0 at `0x00AB3B5C`) into the global `0x00B4BCF4`, and
`Tdiag := r24(Tedge × √2)` into `0x00B4BCF0` (`0x0074E1F6`–`0x0074E204`). Nothing else reads the TalusAngle global
(`0x00B0D59C` has exactly two references, both inside the load path), so a replacement keeps the two thresholds, not
the angle. With the shipped value: `Tedge ≈ 1.39982 m`, `Tdiag ≈ 1.97964 m`; with the code default they would be
16.1479 and 22.8366 m. With the neighbour mask of §11.3 (the `GetNeighbors` rules, computed inline from
`x = i mod vertsX`, `z = i div vertsX`), for each neighbour `n` in the offset order W, NW, N, NE, E, SE, S, SW
(`+0x84`…`+0xA0`) that the mask allows:
1. `excess := r24(r24(alt_i − alt_n) − T)` with `T = Tedge` for W, N, E, S and `Tdiag` for the diagonals.
2. If `excess > 0`: `move := r24(excess × step)`. If `i` is not locked: `alt_i := r24(alt_i − move)`, clamped to the
   minimum allowed height, `depth_i := sea − alt_i`. If `n` is not locked: `alt_n := r24(alt_n + move)`, clamped to
   the maximum, `depth_n := sea − alt_n`. Either way the call reports a change.

`alt_i` is re-read for each neighbour, so a vertex can shed material to several lower neighbours in one call.
Material only moves downhill from `i`.

#### 9.4.2 Kinds 1, 4 and 7 (the flora, animal and mark brushes) *[compat]*

All three are per-vertex callbacks in the contract of §9.4.6 and **all three return 0** except kind 7, so a
brush made only of kinds 1 and 4 never adds a vertex to the out rectangle and posts no `0x278E6003`. None of them
writes the altitude, the water array or any vertex flag except kind 7.

**Kind 1, flora painting** (`0x00745A40`-`0x00745B97`, `ret 8`). The dispatcher sets the callback only when the
terrain's count at `+0x108` is non-zero (`0x0074D973`-`0x0074D97B`: `test eax, eax; jbe` skips the whole kind
otherwise). `+0x108` is the number of flora types; the flora simulator is the terrain's `+0x104`. Per vertex:

1. `b < 10` (unsigned) to no change (`0x00745A4A`: `cmp byte [ctx+0x14], 0xa; jb`). **The neutral grey is not
   consulted**; the gate is a fixed 10.
2. `RandomUint32Uniform(100)` on the terrain generator (`0x00B4BC78`); if the result is `> 30` the vertex is skipped
   (`0x00745A54`-`0x00745A63`). So the placement chance is **31/100** per vertex.
3. Read the vertex's water depth from the surface water array (`terrain +0x24` then `+0x18`, 36-byte records,
   `+0x00`). If `depth > 0.0` (strict; a NaN takes the flora path) the placement is refused and a message is posted
   through the global message service at `0x00B43CB8`: vtable `+0x2C` with `(1, 0xAA62C9D5, 0, 0, 0)`
   (`0x00745A86`-`0x00745AA3`), skipped when the global is null. **No flora is placed on water.**
4. `x := i mod vertsX`, `z := i div vertsX`; if `x >= cellsX` or `z >= cellsZ` (unsigned) nothing happens.
5. Three more draws on the same generator: `rx := RandomFloatUniform(x*16, x*16 + 16)`,
   `rz := RandomFloatUniform(z*16, z*16 + 16)` (no clamp to the city, unlike kinds 3 and 4) and
   `k := RandomUint32Uniform(t->+0x108)` - a flora-type index. So a kind-1 vertex that reaches the scatter costs
   **four** generator draws.
6. `h := GetAltitude(rx, rz)` (terrain slot 13, vtable `+0x34`, the return is an f32 in ST0), then
   `floraSimulator.AddNewFloraOccupant(0, &{rx, h, rz}, 0)` (`terrain +0x104`, **slot 17**, vtable `+0x44`,
   3 arguments, callee pops 12) at `0x00745B56`-`0x00745B67`.
7. Then `city.GetEstablished()` (`0x00B43CEC`, **slot 22**, vtable `+0x58`, no arguments, the result tested in AL) at
   `0x00745B6A`-`0x00745B77`; when it is false the vertex stops here. When it is true the aura simulator
   (`0x00B43D60`) **slot 6** (vtable `+0x18`) is called with `(rx, rz, 1)` (`0x00745B79`-`0x00745B8D`).
8. Returns **0**.

**Kind 4, object placement** (`0x00749310`-`0x00749537`, `ret 8`). The `Animals`, `AnimalsTemperate` and
`GodValleyRain` brushes. It is **not** render-only: it creates an object through a placement manager.

1. `c := ctx->component`; if null, no change. **`b == neutral` to no change** (`0x00749326`-`0x0074932C`) - unlike
   kinds 1, 3 and 7, this one does compare against the component's neutral grey, and acts in **both** directions.
2. `x := i mod vertsX`, `z := i div vertsX`, `lin := vertsX*z + x`.
3. A per-cell counter lives in the `std::map` at `terrain +0x1D4` (key `uint32` at node `+0x10`, counter `uint32` at
   node `+0x14`; lookup `FUN_0x497520`, insert `FUN_0x4FB6C0`). If the counter is already `> component.maxCount`
   (the component's `+0x2C`, unsigned compare) nothing happens.
4. `FUN_0x7493A8`: ask the placement manager at `terrain +0xFC` (vtable `+0x18`) for an object from
   `(component +0x38, component +0x3C)`; if it refuses, nothing happens. Only then is the counter incremented.
5. `xHi = min(x*16 + 16, terrain +0x50)`, `zHi = min(z*16 + 16, terrain +0x54)` (the city size; NaN keeps the
   unclamped bound), then `randX := RandomFloatUniform(x*16, xHi)` and `randZ := RandomFloatUniform(z*16, zHi)` -
   exactly **two** generator draws.
6. `h := GetAltitude(randX, randZ)`; if `h < seaLevel` then `h := seaLevel` (NaN passes through).
7. Build the 0x38-byte placement descriptor at `0x007494F1`-`0x00749510`: a counter byte and a flags byte with bit 0
   set, then thirteen floats: `1, 0, 0, 0, 1, 0, 0, 0, 1, randX, h, randZ, 1`. *[inferred: an orientation or scale
   block rather than a row-major 4x4 identity+translation, whose diagonal would sit elsewhere; the position
   `(randX, h, randZ)` is unambiguous.]*
8. `obj.vtable+0x20(&desc)`, then `obj.vtable+0x0C(0)`, then `obj.vtable+0x08()`.
9. Returns **0**.

**Kind 7, mark** (`0x00740820`-`0x00740845`, `ret 8`). 25 instructions, no calls, no stack frame.

1. `b == 0` to no change (`0x00740820`-`0x0074082D`) - **`!= 0`, not `!= neutral`**.
2. `vertex[i].flags |= 0x0400` - byte `+0x23` of the 36-byte record `or`ed with 4 (`0x0074083A`), which is bit 2 of
   the high byte of the uint16 at `+0x22` (§7).
3. Returns **1** (reports a change). Idempotent.

After the component the dispatcher calls `FUN_00748040(&out)` once more for kind 7 only.

#### 9.4.3 Kind 3, the object scatter *[compat; checked in the bytes, TR1]*

`0x00749130`-`0x00749308`, `ret 8`, thiscall with `ECX` = the terrain. **No shipped brush uses kind 3** (§9.7.1),
so nothing observable in the shipped data depends on it. It is the same family as kind 4: a per-cell object scatter
with a hard cap.

1. `mgr := terrain +0xF8`; if null, no change. `c := ctx->component`; if null, no change.
2. **`b == 0` to no change** (`0x0074915A`) - `!= 0`, not `!= neutral`.
3. `x := i mod vertsX`, `z := i div vertsX`; `x >= cellsX` or `z >= cellsZ` (unsigned) to no change.
4. `lin := vertsX*z + x`; look the counter up in the `std::map` at `terrain +0x1D4` (kind 4's map) and insert it with
   0 when it is new. If `counter > component.maxCount` (the component's `+0x2C`, unsigned) nothing happens.
5. `counter++`.
6. Build a 12-byte id triple `d := (0x7AB50E44, 0x891B0E1A, component +0x30)` - the grey-image type and group of
§9.1 with the component's own id - and `pos := (0, 0)`.
7. `aux := 0`; only when `component +0x34 > 0.0` (strict; a NaN gives 0) is
   `aux := RandomFloatUniform(0.0, component +0x34)` drawn.
8. `xHi := min(x*16 + 16, terrain +0x50)`, `zHi := min(z*16 + 16, terrain +0x54)` (NaN keeps the unclamped bound),
   then `pos.x := RandomFloatUniform(x*16, xHi)`, `pos.z := RandomFloatUniform(z*16, zHi)`.
9. `mgr.vtable+0x14(&d, &pos, scaledStrength, aux)` - four arguments, the strength from the argument block.

So up to **three** generator draws per vertex (the `aux` draw only when the component's `+0x34` is positive). It
posts **no** message and never reports a change (return 0). The only state it writes is the counter in the map at
`+0x1D4`; no altitude, water, sediment or flag is touched. What the `+0xF8` manager does with the placement is
outside the terrain *[inferred: it spawns the object]*.

#### 9.4.4 Kind 6, hydrological erosion *[compat; checked in the bytes, TR1]*

Entry `0x007479B0`, a terrain method (`this` in ECX, `ret 0xC`), called **instead of** the footprint with the whole
component: the bitmap, if any, is ignored (the ErosionBrush's kind-6 component has none, §9.7.1). It reads the
argument block of §9.4.6: `to.x`, `to.z` (`+0x00`, `+0x04`), the brush level (`+0x10`) and the scaled strength
(`+0x18`).

1. The centre vertex is `vx := ftol(to.x × 0.0625)`, `vz := ftol(to.z × 0.0625)` (`0x007479BA`–`0x007479D2`; the
   constant `0x00B0D688` is 0.0625). Same rule as §9.3 step 1, but without the `r24` rounding.
2. The out rectangle is set to the single vertex `(vx, vz)` and then grown by `count / 2` on every side
   (`FUN_00741E30`, `0x00741E30`), where `count := fistp(0.0625 × radius)` and `radius` is the scaled radius
   (`0x0074D9BD`–`0x0074D9D3`). `FUN_00741E30` clamps `x0, z0 ≥ 0` and `x1 ≤ cellsX`, `z1 ≤ cellsZ` (signed; the
   **cell** counts, so on a 256-cell city the last vertex column 256 is reachable only as `x1 = 256`).
   *[quirk: the erosion rectangle is therefore about `radius / 16` vertices wide, centred on the vertex, and is not
   the bitmap footprint.]*
3. Then `n := fistp(strength)` iterations run (`0x007479F8`–`0x00747A26`), and each iteration is the erosion pass
   `Terrain::ErodeRect(rect, k)` (`FUN_00746AD0`, `0x00746AD0`) with `k := table[level]`, the level-indexed value at
   `*(uint32*)0x00B4BD4C + level·4` (the table pointer is filled at init).

`ErodeRect` works on the vertex rectangle in **z outer, x inner** order, with `i = vertsX·z + x`, and touches only the
**water array** (`terrain +0x24` → `water +0x18`, 36-byte records): the depth at `+0x00` and the sediment at `+0x1C`.
It never writes the altitude. Its constants are the terrain ini's `Hydrology` section (§9.7); the values below are
the **shipped** ones, with the compiled-in fallbacks in brackets:

- `T := seaLevel (terrain +0x7C) − 15.0` (the `15.0` at `0x00AB3CC8` is a hard constant, not the ini's
  `SurfaceWaterThreshold`).
- **Pass 1 (once, before the loop).** For every vertex of the rectangle: `alt < T` → `depth := T − alt`; otherwise
  (`alt ≥ T`, and NaN) → `depth := 0`. Then `sediment := 0`. So the rectangle is filled with water up to 15 m below
  the sea, or dried. (`0x00746B41`–`0x00746BAD`; the compare is `fcomp` with the `test ah, 5; jp` sense: jump taken =
  `alt ≥ T` or unordered.)
- **Passes 2 and 3, `n` times each.** Each iteration first **rains**, then **flows**:
  - *Rain.* For every vertex with `alt > T` (strict): `depth += RainFallFactor × alt` (shipped 0.0001 at
    `0x00B0D5A8`; fallback 0.01). (`0x00746BF0`–`0x00746C21`, `test ah, 0x41; jne` skips `alt ≤ T` and NaN.)
  - *Flow and erosion.* For every vertex with `depth > 0` (NaN falls into the body):
    1. `alt < T` → `depth := T − alt` and on to the next vertex (re-fill).
    2. Otherwise pick a flow neighbour `k` with `FUN_00744760` (`0x00744760`): the §11.3 `GetNeighbors` mask from
       `x = i mod vertsX`, `z = i div vertsX` and the cells bounds, then the largest `drop := (alt_i + depth_i) −
       (alt_n + depth_n)` among the four edge neighbours (offsets `+0x84`, `+0x88`, `+0x8C`, `+0x90`, scanned with
       `k += 2`), then the four diagonals (`+0x88` onward, `k` odd) against `√2 × best` — the same shape as the lava's
       rule (§11.3). On a rectangle border vertex (`z == z0` or `z1`, `x == x0` or `x1`) no neighbour is picked and
       `k := −1`. The lazy constant `0x00B4BD74` is `√2`, `0x00B4BD84` is `1/√2`.
    3. `amount := 0.5 × drop` (`0x5` `0x00A84D2C`), clamped down to `depth_i` when it would exceed it; on a border
       vertex `amount := depth_i` (the whole depth).
    4. Sediment exchange: `cap := MaxSedimentPerUnitWater × amount` (shipped 8.0 at `0x00B0D5AC`; fallback 5.0).
       If `cap < sediment_i`: `sediment_i −= cap` and `cap` is carried to the neighbour. Otherwise the carried
       amount is 0 and the rest is deposited: `alt_i −= rate × (cap − sediment_i)`, `sediment_i := 0`, where `rate`
       is the erosion rate below.
    5. The erosion rate from the slope `s := alt_i − alt_n` (× `1/√2` when `k` is a diagonal; 0 when `k < 0`):
       `s ≤ HydrErosionRateRampBegin` → `MinErosionRate`; `s ≥ HydrErosionRateRampEnd` → `MaxErosionRate`; else
       `MinErosionRate + (s − RampBegin) × (RampEnd − RampBegin) / (MaxErosionRate − MinErosionRate)` (the ratio is
       computed once per call into `esp+0x2c`, `0x00746B14`–`0x00746B3B`). Shipped: 0.05, 0.4, 5° and 40° (the two
       ramp endpoints are × π/180 on load, `0x0074E847`, `0x0074E8D2`).
       *[quirk: the ini documents the ramp endpoints as a **slope in degrees**, and the load path converts them to
       radians, but the pass compares them with a raw height difference in **metres**. With the shipped values the
       rate therefore saturates at `MaxErosionRate` as soon as the drop exceeds 0.698 m. The degrees-of-slope intent
       is *[inferred]*; what would settle it is a controlled in-game stroke on a known slope.]*
    6. If vertex `i`'s flags at `+0x23` have bit `0x40` set the sediment step is skipped (`0x00746DC7`);
       `depth_i := depth_i − amount`, and, when a neighbour was picked and `alt_n > T`, `depth_n += amount` and
       `sediment_n += carried` (`0x00746DF1`–`0x00746E34`).
    7. Finally, if `depth_i ≤ 0` the sediment is zeroed; otherwise if `depth_i > 0` and the flags do **not** have
       `0x40`, `alt_i += DepositRate × sediment_i` and `sediment_i −= ` the same amount (`DepositRate` shipped 0.65 at
       `0x00B0D5B0`; fallback 0.2; `0x00B4` `0x00B0D5B0` is the constant `0.20000000298023224` only as the compiled
       fallback). (`0x00746E38`–`0x00746E83`.)

**What it does not do.** It writes no altitude and no vertex flags, so it does not move objects and does not change
navigability by itself — but it changes water depths, and `IsNavigable` and the bodies of water (§10.3, §10.4) read
those, so navigability changes once the surface water rebuilds. It posts no message of its own (see the §9.4
change-message note): the caller's single `RedisplayTerrain` covers the out rectangle.

Shipped brush: `ErosionBrush 0x6A4BAAFA` comp0 (radius 900, 700, 350, 250, 200; strength 10, 8, 6, 5, 4 — the
strength is the **iteration count**), used by the effects manager's `erodefx_terrain` and `stream_cut_fx_terrain`
records (effects.md §8.4). So kind 6 is **not** exclusive to the god-mode tool: it is reachable through a brush
effect. Which caller starts those two effect names is not yet traced *[inferred: a network/stream or disaster
effect]*.

#### 9.4.5 Kinds 8, 9 and 10 (the god-mode water brushes) *[compat; checked in the bytes, TR1]*

These are the `GodValleyLake`, `GodValleyRain` and `GodValleySpring` brushes (§9.7.1).

**Kind 8, lake** (`0x00745CD0`), a per-vertex callback (`this` = the terrain, `ret 8`). With `b` the grey byte and
`neutral` the component's neutral grey (`component +0x28`):

```
amount := f32((int)(b − neutral)) × component.strength[level]   // the RAW per-level strength
if amount == 0: return false                                     // also when b == neutral
target := amount + params.targetHeight                           // the +0x1C of §9.4.6 = tBrushParameters +0x34
tol    := |amount × 0.0625|                                      // 0x00A8E174
```

*[quirk: `amount` uses the component's raw `strength[level]`, **not** the scaled strength the other kinds take from
the argument block, so `tBrushParameters.strength` has no effect on kind 8.]*

Then (`0x00745D2C`–`0x00745DD4`):

- `target > alt_i` (strict): a **wet** vertex (`depth_i > 0`, NaN counts as wet) is left **unchanged** and the call
  reports false. A dry vertex (`depth_i ≤ 0`) is changed: `target − alt_i < tol` → `SetAltitudeAtVertex(i, target)`
  (terrain vtable slot 50, `+0xC8`); else `RaiseVertex(i, tol)` (`0x00744CA0`). Either way it reports true.
- `target < alt_i` (strict), wet or dry: `alt_i − target < tol` → `SetAltitudeAtVertex(i, target)`; else
  `LowerVertex(i, tol)` (`0x00744AF0`). Reports true.
- `target == alt_i`, or NaN: no change, reports false.

So the lake brush is **grey-modulated levelling**: with neutral 255 (GodValleyLake) `b − neutral ≤ 0`, so it only ever
carves **down** toward `target_height − |amount|`. Water appears only as a side effect: the height setters recompute
`depth := sea − alt` (§6), so ground carved below the sea becomes water. It never writes the water array directly,
and `RaiseVertex` / `LowerVertex` erase any lake water they cross (§16.5). Objects and navigability follow the
altitude change through the usual setters.

Shipped: `GodValleyLake 0xEB70C719` comp0, neutral 255, radius 800/500/300/200/100, strength 0.6/0.3/0.15/0.08/0.04.

**Kind 9, rain** — entry `FUN_0073EAB0` (`0x0073EAB0`), a **surface-water** method (the surface water is the terrain's
`+0x24`; `ret 0x10`). It is called with `(vertexIndex, &argumentBlock, &outRect, scaledStrength)` where the vertex
index is `GetVertIndex(vx, vz)` of the §9.3 centre vertex (terrain slot 9). It has two forms selected by the surface
water's mode byte at `+0x34` (`0x0073EAB0`–`0x0073EABE`):

- `+0x34` **non-zero** → the component is applied per vertex with the callback `0x0073D490`, which adds
  `strength × 0.1` (`0x00A8C950`) to the water record's field at `+0x08` and reports false always (so no vertex joins
  the out rectangle). `b == neutral` makes it a no-op.
- `+0x34` **zero** (the default) → `FUN_0073E350` runs a whole-region rain and no per-vertex callback is used.

*[In this build the byte is only ever stored at `0x0073DF25` and `0x0073FF24`, both writing 0, so the per-vertex form
is unreachable and the region form always runs. A replacement may implement the region form only.]*

The region form (`FUN_0073E350`, `0x0073E350`) is a surface-water method with `(vertexIndex, &argumentBlock,
&outRect, scaledStrength)`:

1. The out rectangle is reset to empty (`x0, z0 := INT32_MAX`, `x1, z1 := INT32_MIN`).
2. `FUN_0073D2B0(vertexIndex)` then the terrain's `GetVertex` (slot 10, `+0x28`) give the vertex's `(x, z)`; the point
   is rejected unless `FUN_00743A10(terrain, x, z, block)` accepts it and `b ≠ neutral`.
3. `newLevel := waterSurface(i) + strength`, where `waterSurface(i) := alt_i + (depth_i > 0 ? depth_i : 0)`.
4. `FUN_0073DFF0` builds the set of vertices the water reaches from `i`.
5. For each reached vertex `j` with `waterSurface(j) < newLevel` (strict; NaN is skipped):
   `depth_j := newLevel − alt_j`, clamped to at most **200.0** (`0x00B0D5DC`). Each such vertex joins the out
   rectangle through `GetVertex` and a signed min/max, so the rectangle ends as the bounding box of the vertices
   actually raised (or empty).

It writes only water depths; no altitude, no flags, no objects. Navigability and the bodies of water follow through
§10.3/§10.4 after `EndBrushScrub`. Shipped: `GodValleyRain 0xCB8D4CA7` comp0 and `GodValleyLake 0xEB70C719` comp1,
neutral 255, strength 6/4/2/1/0.5.

**Kind 10, spring** — entry `FUN_0073D4D0` (`0x0073D4D0`), a **surface-water** method, `ret 4`, one argument:
`water[GetVertIndex(vx, vz)].field_4 += 1.0` (`0x0073D4D0`–`0x0073D4EA`; the 1.0 is `0x00A81228`). That is the whole
operation: no bitmap, no per-vertex loop, no out-rectangle entry (the call is made with the callback slot left 0, so
nothing is added to the out rectangle and, unless another component of the same brush touched something, no message
is posted). Only the **centre** vertex is affected, whatever the radius.

The water record's fields at `+0x04` and `+0x08` (the spring and rain accumulators) are not in the §2.2 table; they
are `f32` and are read by the surface water's own tick *[inferred: consumed as a water source]*. A trace of the
surface water's tick settles what each becomes.

Shipped: `GodValleySpring 0x4B9539D1` comp0, no bitmap and no neutral grey, radius 5/4/3/2/1, strength 5/4/3/2/1.

#### 9.4.6 The callback contract *[compat]*

Kinds 0, 1, 2, 4, 5, 7 and 8 are per-vertex callbacks. The footprint walkers pass three things:

```
bool __stdcall Op(Terrain* this /* ECX */, int vertexIndex /* stack arg 1 */, ArgBlock* p /* stack arg 2 */);
```

`Op` returns non-zero when it changed the vertex; the walker then adds that vertex to the out rectangle. Both walkers
pop 8 bytes (`ret 8`).

The **argument block** is built once per component by the dispatcher and reused for every vertex (a block on the
dispatcher's stack):

| Offset | Field |
|---|---|
| `+0x00`, `+0x04` | `to.x`, `to.z` (metres) — also what kind 6 uses for the centre |
| `+0x08`, `+0x0C` | `from.x`, `from.z` |
| `+0x10` | the brush level index (int) |
| `+0x14` | the grey byte `b` — **written by the walker before each call** |
| `+0x18` | the scaled strength `component.strength[level] × tBrushParameters.strength` |
| `+0x1C` | the target height (`tBrushParameters +0x34`) |
| `+0x20` | the component (`cSC4BrushComponent*`) |

and the component (76 bytes, the stride of §9.1's list at `0x0074D7A7`) carries, as the callbacks read them:
`+0x04` kind, `+0x08` bitmap object, `+0x0C` oriented byte, `+0x10` the radius array, `+0x1C` the strength array,
`+0x28` the neutral grey byte.

- **With a bitmap** (`FUN_00742180`, `0x00742180`) the walker is the §9.3 footprint and `b` is the bitmap pixel.
- **Without one** (`FUN_00742440`, `0x00742440`, `ret 0x24`) the walker is the same rectangle scan but it **never
  writes the grey byte**. Its loop body at `0x00742530`–`0x0074253D` only pushes the arguments and calls; the store
  `mov byte [ctx+0x14], cl` that the bitmap walker makes at `0x0074239E` has no counterpart here, and a raw-byte
  scan of the whole dispatcher finds no store into the block at all. So the grey the callback sees is the residual
  byte left in the dispatcher's stack slot — neither a constant nor the component's neutral. *[quirk]* Kind 0 and
  kind 4 compare that residual against the neutral grey, and kinds 1, 3 and 7 test it against 0, so the no-bitmap
  path is not reproducible from the component alone. Only `GodValleySpring` and `ErosionBrush`'s kind-6 component
  ship without a bitmap (§9.7.1), and neither uses the callback path, so nothing observable in the shipped data
  depends on it. A replacement must choose one; the natural choice is the component's neutral grey, recorded as a
  *[choice]*.

  The solid walker takes one argument more than the bitmap one: a pass count, which the dispatcher sets to **1**
  (`0x0074DA78`), so its loop runs exactly once; the bitmap walker passes 0 there and uses the slot as the ECX
  offset instead. Both walkers report their changes the same way: they initialise the out rectangle to
  `(INT32_MAX, INT32_MAX, INT32_MIN, INT32_MIN)` and widen it with the x and z of every vertex whose callback
  returned non-zero (`0x00742548`–`0x0074255C`), so a component that changes nothing leaves it empty — and that
  sentinel pair is what `ApplyBrush`'s step 4 tests before it posts.


**Kind 0** (`0x00745C60`) reads `p +0x14` (b), `p +0x18` (scaled strength) and `p +0x20 → +0x28` (neutral):
`b == neutral` → false; `b > neutral` → `RaiseVertex(i, f32((b − neutral) × strength))` (`0x00744CA0`); `b < neutral`
→ `LowerVertex(i, f32((neutral − b) × strength))` (`0x00744AF0`). Every `b ≠ neutral` reports a change, even when the
vertex is locked or already at the clamp (TR15).


### 9.5 Who applies brushes

- **Earthquake** (disasters.md §7, `StartDeformation` `0x00575C40`): level 0, redisplay off, `data4 = 3`. It calls
  `BeginLongTerrainOperation`, then, per 8 m step along the fault, `ApplyBrush(0x11, left)` and
  `ApplyBrush(0x12, right)`, and at the end `StopAccumulatingTextureUpdates` and
  `RedisplayTerrain(1, 1, leftRect, 3)`, `RedisplayTerrain(1, 1, rightRect, 3)`.
- **Pipe-burst sinkhole** (`0x0057C1A0`): from = to = the cell's north-west corner `(16x, 16z)`, level 0, redisplay
  on, strength 1, radius 1, `data4 = 3`, target 100 → `ApplyBrush(0x14)`: a 16 m footprint lowered by up to
  `(255 − b) × 0.01` m.
- **Effect brushes** (the effects manager's brush effect, `0x00581640`; [effects.md](effects.md) §8): the volcano's
  deformation is the effect `"volcano_control"` started by the volcano instance (`StartDeformation` `0x0057F4F0`).
  Each update applies the effect's brush `count` times from the previous to the current position, both inside the
  city (city slot 98), with the level from the effect, strength and radius drawn once from the effect's ranges,
  `data4 = 3`, the effect's target height, and **redisplay only on the last repetition**. The shipped values
  (effects.md §8.4, TR2): `volcano_control` picks one of nine creators (a select group, effects.md §5.3); each uses
  VolcanoBrush `0xCA313035` at level 1, strength 1.0, a fixed radius scale of 0.5–1.3 and target 100, as a stroke of
  30–130 applications per effect-second for 2–4 effect-seconds (`count` = the whole part of the accumulated rate,
  effects.md §8.3). Meteor craters are one-shot: CraterBrush `0x8A33E17A` applied twice in a single update (level 3,
  strength 2–6, radius 0.5–1.0).
- **Terraforming tool** (§9.6) and the cheat / Lua command `ApplyTerrainBrush` (`0x0C4A6AF6`): level −1, redisplay
  on, arguments (brushID, x0, z0, x1, z1[, strength, radius]) with both points required inside the city (slot 11),
  from = (x0, z0), to = (x1, z1), strength and radius default 1.0, `data4 = 2`, target height = `GetAltitude(x1,
  z1)` (checked in the bytes at `0x0074A933`–`0x0074AAB7`).

### 9.6 The terraforming tool

The tool (`cSC4ViewInputControlTerrainScrubber`) wraps strokes in `BeginBrushScrub` / `EndBrushScrub` (slots 59, 60)
and calls `ApplyBrush` per application. In god mode it sets `data4 = 1`; in mayor mode `data4 = 2` and each
application is charged `GetCostPerBrushApplication` (slot 62) against the budget. `EndBrushScrub` makes the surface
water rebuild its bodies of water (§10.3).

---

### 9.7 The terrain ini and the shipped tunables *[compat; checked in the bytes and in the shipped data, TR1]*

The terrain tunables are **not** in `SimCity 4.ini` (that file only holds `[Directories]`) and not in a file on disk.
`ReadTerrainIniFile` (`0x0074DC30`) fetches them as a single DBPF resource:

| | |
|---|---|
| Type | `0x00000000` |
| Group | `0x8A5971C5` |
| Instance | `0xAA597172` |
| Package | `SimCity_1.dat` |
| Format | plain-text ini, 17 056 bytes decompressed (3 979 compressed) |

The TGI is loaded at `0x0074DC4C`–`0x0074DC54` and looked up at `0x0074DC5C`. The sections are `MiscTerrainTunables`
(`0x00AB4124`), `WindErosion` (`0x00AB40E8`) and `Hydrology` (`0x00AB407C`); the key strings are `0x00AB3F00`–
`0x00AB4110`. Each present key is parsed — `atoi` for the integer ones, `atof` for the rest — and stored in a global;
when a key is absent the global keeps its compiled-in fallback (the `.data` initial value). The load is once per
process.

| Section | Key | Global | Fallback | **Shipped** | Notes |
|---|---|---|---|---|---|
| MiscTerrainTunables | `EnableSimulation` | `0x00B0D568` (byte) | 0 | **0** | terrain simulation off |
| MiscTerrainTunables | `RaiseLowerAmount` | `0x00B0D578` | — | **80.5** | **write-only**: nothing reads this global (two references, both in the load path). Slot 47 returns the constant 20.0 (§2.4) |
| WindErosion | `NumIterationsPerUserClick` | `0x00B0D594` | — | **1** | |
| WindErosion | `NumIterationsPerSimMonth` | `0x00B0D598` | — | **5** | |
| WindErosion | `TalusAngle` | `0x00B0D59C` | 0.79 rad | **5** (degrees) | `atoi`, then × π/180 (`0x00B0D694`) at `0x0074E1C2`. Only the load path reads it: it is turned into the two kind-2 thresholds (§9.4.1). `Tedge = tan(5°)·16 ≈ 1.39982` at `0x00B4BCF4`, `Tdiag ≈ 1.97964` at `0x00B4BCF0` |
| WindErosion | `ErosionFactor` | `0x00B4B40C` | — | **0.05** | the wind-erosion rate; nothing in the brush path reads it (kind 2 uses only the two thresholds) |
| Hydrology | `NumWaterErosionIterationsPerPlayerClick` | `0x00B0D5A0` | — | **1** | |
| Hydrology | `NumFlowStepsForGlobalWaterErosion` | `0x00B0D5A4` | — | **20** | |
| Hydrology | `SurfaceWaterThreshold` | `0x00B0D574` | — | **2.0** | not the `15.0` kind 6 subtracts (§9.4.4) |
| Hydrology | `RainFallFactor` | `0x00B0D5A8` | 0.01 | **0.0001** | kind 6's rain term |
| Hydrology | `WaterSourceCapacity` | `0x00B0D56C` | — | **10.8** | |
| Hydrology | `WaterSinkCapacity` | `0x00B0D570` | — | **10.8** | |
| Hydrology | `MaxSedimentPerUnitWater` | `0x00B0D5AC` | 5.0 | **8.0** | kind 6 |
| Hydrology | `MinErosionRate` | `0x00B0D5B8` | 0.02 | **0.05** | kind 6 |
| Hydrology | `MaxErosionRate` | `0x00B0D5BC` | 0.06 | **0.4** | kind 6 |
| Hydrology | `HydrErosionRateRampBegin` | `0x00B0D5C0` | 5.128 | **5** (degrees) | × π/180 at `0x0074E847`; kind 6. *[quirk: the fallback is not pre-converted, so with no ini the ramp is nonsense]* |
| Hydrology | `HydrErosionRateRampEnd` | `0x00B0D5C4` | 38.14 | **40** (degrees) | × π/180 at `0x0074E8D2`; kind 6 |
| Hydrology | `DepositRate` | `0x00B0D5B0` | 0.2 | **0.65** | kind 6 |
| Hydrology | `SedimentSpreadRateUnderSea` | `0x00B4B408` | — | **0.5** | clamped to ≤ 1.0 at `0x0074E9F4` |
| Hydrology | `DepositRateUnderSea` | `0x00B0D5B4` | — | **0.4** | |
| Hydrology | `SedimentInfluenceOnWaterColor` | `0x00B0D5C8` | — | **0.2** | render |
| Hydrology | `MaxSedimentAlpha` | `0x00B0D5CC` | — | **1.0** | clamped to ≤ 1.0 at `0x0074EB82`; render |
| Hydrology | `SedimentColorRed` / `Green` / `Blue` | `0x00B0D5D0` / `D1` / `D2` (bytes) | — | **116 / 108 / 85** | render |

The ini also carries the `TerrainMiscTextures` / `*TextureMapTable` sections (render texture ids) and, commented out,
the `Desert`/`Temperate`/`Arctic` tables.

**What this means for a replacement.** The shipped values are the ones the game actually runs with, and several of
them differ a lot from the compiled fallbacks (in particular `TalusAngle` 5° vs 0.79 rad, and the whole erosion-rate
ramp). A replacement that wants to match the original must read this resource, not rely on the fallbacks. Conformance
test 30 covers the load and the two kind-2 thresholds.

#### 9.7.1 The shipped brushes and which kinds they use

30 brush exemplars (type `0x6534284A`, group `0x6A01FC2A`) ship in `SimCity_1.dat`. Their component kinds:

| Kind | Brushes |
|---|---|
| 0 | CanyonBrush `0xCA33E1CC`, CanyonSoftBrush `0xEA312F77`, CliffRidge `0xEA33E145`, CraterBrush `0x8A33E17A`, EarthquakeFault `0x11`, EarthquakeFaultFlipped `0x12`, GodMountainDefault `0x6A33E1BA`, GodMountainHill `0xAA33E18F`, GodMountainMesa `0xEA312EF9`, GodMountainSteepHills `0x6A33E1DB`, GodValleyDefault `0x2A33E1A3`, GodValleySoft `0x8A33E159`, GodValleySteep `0x6A33E168`, MayorMountainHill `0xEA33E2FD`, MayorValleyDefault `0x0A33E37B`, PipeSinkHole `0x14`, VolcanoBrush `0xCA313035` |
| 1 | FloraBrush `0x2A31304E` |
| 2 | CanyonBrush, CanyonSoftBrush, ErosionBrush `0x6A4BAAFA`, GodMountainHill, GodValleySoft, MayorMountainHill, PlainsBrush `0xEA393C05`, SmoothingBrush `0xEA313BB9` |
| 3 | **none** |
| 4 | Animals `0xEA353064`, AnimalsTemperate `0x0A6F81D8` / `0x8A6F2E83`, GodValleyRain |
| 5 | GodMountainMesa, LevelBrush `0x2A312F97`, MayorLevelBrush, PlainsBrush, SuperLevelBrush `0xCA835E0E` |
| 6 | ErosionBrush (comp0) |
| 7 | **none** |
| 8 | GodValleyLake `0xEB70C719` (comp0) |
| 9 | GodValleyLake (comp1), GodValleyRain `0xCB8D4CA7` (comp0) |
| 10 | GodValleySpring `0x4B9539D1` |

So **kind 3 and kind 7 have no shipped brush at all**, and every kind except 3 and 7 is reachable from data. The
god-mode water brushes are `GodValleyLake`, `GodValleyRain` and `GodValleySpring`; `ErosionBrush` and
`SmoothingBrush` are ordinary effect brushes (effects.md §8.4).


---

## 10. Surface water (`cSTESurfaceWater`)

### 10.1 State and life cycle *[compat]*

The water array (§2.2) is rebuilt, not loaded: the surface water `Init` (`0x0073FE70`) and the terrain `Create`
call `ResetWaterToSeaLevel`, which sets every vertex's `depth := sea − alt` and the other water fields to 0. Lakes
made by the god-mode brushes are therefore lost on reload (§13.3).

`Init` subscribes to `0x26D31EC1` (PostCityInit), `0x26D31EC4`, `0x26D31EC5`, `0x89D3E9F3` (network occupant
changed), `0x99EF1142`, `0x99EF1143` (occupant inserted, removed), and registers with the animation tick manager at
priority `0xC0`.

### 10.2 Slots *[compat]*

| Slot | Function | Rule |
|---|---|---|
| 5 | `GetWaterDepthAtVertex(i)` (`0x0073D200`) | the depth (no bounds check, §10.5) |
| 6 | `GetWaterDepthForCell(x, z)` (`0x0073D210`) | `i := terrain.GetVertIndex(x, z)` (no bounds check, §10.5). `m :=` the minimum of the depths at `i`, `i + 1`, `i + 1 + vertsX`, `i + vertsX`, in that order, a NaN candidate never replacing the minimum. Return `m` if `m > 0`, else 0.0 |
| 13 | `HasDraftClearance(x, z)` (`0x0073CDB0`) | `GetWaterDepthForCell(x, z) > 5.0` |
| 8 | `IsNavigable(x, z, r, lot)` (`0x0073EBF0`) | §10.4 |
| 7 | `IsNavigable(x, y, z, r, lot)` (`0x0073CEF0`) | **Corrects (TR26):** five dwords are `(float x, float y, float z, int32 r, void* lot)` — a 3D world position in metres plus the radius and lot that slot 8 takes. **`y` is never read.** Callee pops 20 bytes; **returns in AL only**. Converts with city slot 95 `LocationToCell(x, z, &cellX, &cellZ)` (`0x004599E0`), which is `floor(coord × 0.0625)` per horizontal axis — plain world→cell, no origin offset, not `GetVertIndex` — then calls slot 8 `IsNavigable(cellX, cellZ, r, lot)`. *[quirk]* it passes **`&arg1`** as city slot 95's cell-z out-parameter, so the incoming `x` is overwritten with the cell z; invisible to a caller (callee-pop) but a reimplementation must not reuse `x` afterwards |
| 9 | `GetBodyOfWaterMap()` | the uint16 grid at `+0x4C` (SimGrid type `0x49B9E600`, ID `0x4BE3618B`), one value per cell, 0 = none |
| 10 | `AltitudeAtWhichFloodingWillOccur(x, z)` (`0x0073D4F0`) | checked in the bytes (TR3). `i := GetVertIndex(x, z)` (no bounds check). `r := sea level`. With `L(v) := depth(v) + alt(v)` (one rounded add): if `depth(i) > 0` and `L(i) > r`: `r := L(i)`. Then `GetVertex(i)` and the `GetNeighbors` mask of §11.3; for each allowed neighbour in the offset order W, NW, N, NE, E, SE, S, SW: if its `depth > 0` and its `L > r` (strict; NaN never wins): `r := L`. Return `r` |
| 11 | `GetWaterVelocity(x, z)` (`0x0073D7C0`) | a pointer to the vertex's velocity (no bounds check) |
| 12 | `GetWaterType(x, z)` (`0x0073D7F0`) | vertex `i := GetVertIndex(x, z)` (no bounds check). `depth ≤ 0` → **0**; else `level := depth + alt`: `level > sea` (or NaN) → **2** (fresh water above the sea), otherwise **3** (sea water). A **NaN depth** is not `≤ 0`, so it takes the second test with a NaN level and returns **2** (checked at `0x0073D811`–`0x0073D83E`, TR6) |

### 10.3 Bodies of water *[compat]*

`UpdateBodiesOfWater` clears the map, then scans the cells with **x outer, z inner**. Each unlabelled cell that is
navigable (`IsNavigable(x, z, 0, 0)`) gets the next ID (1, 2, …) and a FIFO flood fill over the 4-neighbours, pushed
in the order west, east, north, south, labels every navigable cell it reaches.

It runs:
- on `0x26D31EC1` (only if the city is established, city slot 22) and on `0x26D31EC4`, after
  `UpdateNeighborConnections`;
- on SimNewDay `0x66956814` when the dirty flag (byte `+0x50` of the object; `+0x48` of the message-target
  interface at `+0x08`) is set, which it then clears;
- on `0x89D3E9F3` for a bridge piece (network flag `0x40000`) with no adjacent bridge piece;
- at `EndBrushScrub` when the city is established.

**Corrects (TR27).** The SimNewDay trigger is **unreachable**. `DoMessage` has the branch (`0x0074019A`), but
`Init` subscribes to exactly six messages (`0x0073FF7F`–`0x0073FFC3`) and `0x66956814` is not one of them, nor does
it subscribe to the type-0 wildcard, so the server never delivers it — and `0x66956814` appears nowhere in
`0x00730000`–`0x00750000` as an immediate. The consequence is a **dead dirty flag**: the power-pole handlers set
`this+0x50` (`0x007402A4`) and the only reader is that unreachable branch, so it is set and never used.
*[inferred: no other delivery path]* — a runtime breakpoint on `0x0074019A` would settle it. **A replacement must
match this**: set the flag, and either also never deliver SimNewDay to the water or document the divergence. Also:
`0x26D31EC5` **is** subscribed (`0x0073FF9C`) and its handler does nothing (it falls out of the compare chain at
`0x00740194`).

A power pole inserted or removed (`0x99EF1142` / `0x99EF1143`, occupant type `0x2890D4DE`, pole interface
`0xA8C86142` with slot 13 false) sets the dirty flag when `GetWaterDepthForCell(x, z) > 0` (strict) at the pole's
cell (pole slot 8) (`0x0074022F`–`0x007402A4`, TR13). Both messages are posted by the occupant manager, so this
handler runs from the queue (occupant-manager.md).

### 10.4 `IsNavigable(x, z, r, lot)` (slot 8) *[compat]*

Visit the cells of the square `[x − r, x + r] × [z − r, z + r]` with the spiral iterator (from the centre outward;
when `max < min` the maximum is set to the minimum). It is the plumbing iterator (constructor `0x0043FE10`,
`GetNext` `0x00440850`; plumbing.md §7.2 gives the clockwise order and the clipping), and the square is **not**
clipped to the city first (`0x0073EBF0`–`0x0073EC80`, TR17). For each cell:
1. Out of the city: `city.CellIsInBounds(x, z)` (city slot 99, `0x004594D0`) is `0 ≤ x < cellsX` and
   `0 ≤ z < cellsZ`, signed, with the city's cell counts (`+0x118`, `+0x11C`, the same values the terrain takes in
   `Init`). Out: return false. So the first out-of-city cell in spiral order ends the walk with false.
2. Not `HasDraftClearance`: return false.
3. If `lot ≠ 0`: the lot at the cell centre (city slot 97, y = sea level; lot manager slot 5) must be null or `lot`.
4. If the traffic network map has a tile at the cell (`FindNetworkTile(x, z, 0x1F4F, 1)`) with network flag
   `0x40000` (**bridge**; tile slot 8, `0x0073ED36`–`0x0073ED4D`): read the tile's **passage ceiling** (network
   occupant slot 67 `GetPassageCeiling(float&)`, `0x00619900`, `0x0073EE95`). It returns false when the occupant has
   no property `0x4835C6A8`; then return false. Otherwise the float is an absolute world height, the underside of
   the bridge: the only writer, slot 68 `SetPassageCeiling` (`0x0061B9A0`), is called by the bridge builder
   (`0x0062F005`) for each piece whose exemplar lacks `0xABE1B5A3`, with the lowest of the cell's four corner
   heights minus the bridge rule's model offset (network-bridges.md §11.2). If
   `ceiling − (alt + depth at vertex (x, z))` (terrain slot 9 index; `0x0073EE9F`–`0x0073EEBE`) is **not below
   20.0** (`0x00AA0F1C`; equal passes, NaN fails), return **true at once**; otherwise return false (TR19).
5. Otherwise ask the occupant manager (slot 41, `0x0073EDD4`) for the occupants of type `0x2890D4DE` (any
   flags, -1) touching the single cell min = max = `(x, z)`, the **visited** spiral cell. For each one returned,
   query `cISC4PowerPoleOccupant` (`0xA8C86142`) and read its own cell (pole slot 8, `GetCell`); the cell is
   blocked only when that cell **equals** `(x, z)` (`0x0073EDF1`–`0x0073EE2F`). An occupant that fails the query, or
   a pole whose own cell is another one (its bounds merely overlapping), does not block. Blocked: free the list and
   return false (`0x0073EEEA`) (TR20). All occupants are visited even after a match; each is released.

If every cell passes: true.

*[quirk]* Step 4 returns true without checking the cells still to be visited, so a high bridge anywhere in the square
makes the whole square navigable from that point of the spiral.

*[compat]* A bridge piece built from an exemplar with `0xABE1B5A3` has no passage ceiling, so step 4 returns false
on it however high it is.

Consumers: traffic-network.md §4.10 (ferries and watercraft), zones.md (seaport and waterfront tests), the city
situation manager (`IsNavigable(r = 2)`, `GetWaterDepthForCell`), the watercraft automata.

### 10.5 Slots without a bounds check (TR12)

Checked in the bytes. These slots form a flat index and use it as is:

| Object | Slot | Index used |
|---|---|---|
| terrain | 50 `SetAltitudeAtVertex(i, h)` (`0x00744E60`) | `i` |
| terrain | 51 `SetAltitude(x, z, h)` (`0x00744F90`) | `x + z × vertsX` |
| surface water | 5 `GetWaterDepthAtVertex(i)`, 11 `GetWaterVelocity(x, z)`, 12 `GetWaterType(x, z)` | `i`, `GetVertIndex(x, z)` |
| surface water | 6 `GetWaterDepthForCell(x, z)` (and 13 through it) | `i`, `i + 1`, `i + vertsX`, `i + vertsX + 1` with `i = GetVertIndex(x, z)` |
| surface water | 10 `AltitudeAtWhichFloodingWillOccur(x, z)` | `GetVertIndex(x, z)`, then the neighbour mask |

Every other index-taking slot of §4.2 and §5 is checked and returns 0.0 (or false, or leaves its output untouched).

*[compat]* While the flat index lies inside the array (`0 ≤ index < vertexCount`), the original reads or writes that
element, even when a coordinate is out of range: `GetWaterDepthForCell(cellsX, z)` reads the vertex `(cellsX, z)`
and, as `i + 1`, the first vertex of row `z + 1`. A replacement must do the same.

*[choice]* A flat index outside the array reads or writes memory past the array in the original (undefined: heap
garbage or a crash); no caller in the simulation does it (the callers check the city bounds first). A replacement
returns 0.0 for a float read (the value of the checked siblings), 0 for `GetWaterType`, and a pointer to a shared
zero vector for `GetWaterVelocity`, and ignores the write. NaN is not used.

---

## 11. Volcano lava (`cSTEVolcanoLava`)

### 11.1 Tuning *[compat]*

Exemplar `0x6534284A / 0x88CD66E9 / 5` ("Lava Properties"), read once (`0x0075DCE0`). The ones the flow uses:

| Property | Use | Shipped |
|---|---|---|
| `0x69987E61` | maximum flow rate, × 1e-6 per microsecond | 4 → 4e-6 |
| `0x69987E62` | level difference for the full rate | 12 |
| `0x69987E63` | solidified lava → terrain height factor | **0** |
| `0x69987E68` | solidification multiplier over water | 2 |
| `0x69987E69` | ignition level (`IgniteCell`) | 1.5 |
| `0x69987E6A` | burn level (`QuickBurnCell`) | 3 |
| `0x69987E6B` | animation frame period (µs; render) | 200000 |
| `0x69987E6C` | solidification threshold | 3 |

The others (`0x69987E60`, `64`–`67`, `6D`–`6F`) are render values (decals, steam).

### 11.2 Start *[compat]*

The volcano instance (disasters.md §10) creates the lava in state 3 (`InitializeVolcanoLava` `0x0057F360`, only when
`0x6A4BC8ED` is true), with production and solidification rates 0. In state 5 (`0x0057F3D0`) it starts the flow at
its location `(x, z)` (metres, clamped to `[0, size − 1]`) with radius `0x0A4BC752` (20 m) (`0x0075F8C0`):

- the source vertex `(floor(x / 16), floor(z / 16))`;
- the **source rectangle**: the vertex square of half-width `trunc(20 / 16) = 1` around it, clamped to
  `[0, cellsX] × [0, cellsZ]` (so 3 × 3 vertices);
- the active rectangle := the source rectangle;
- the burn rectangle: half-width 7 around the source vertex, clipped to `[0, cellsX − 1] × [0, cellsZ − 1]`, with a
  random iterator over it;
- it registers with the animation tick manager (priority `0xC0`).

Each volcano tick sets the production rate and the solidification rate (disasters.md §10). The lava level array
(one float per vertex) starts at 0. The lava is **not saved** (saving is disabled during a disaster).

### 11.3 The flow step (`DoLavaFlow(dt)` `0x0075F4A0`, every animation tick, dt in µs) *[compat]*

1. `solid := dt × solidificationRate`, `solidWater := solid × 2`; `solidifying := solid > 0`; then clamp both ≤ 1.
2. Every source vertex: `lava += dt × productionRate` (the active rectangle grows to include it).
3. For each vertex of the active rectangle (z outer, x inner) with `lava > 0`:
   1. **Flow.** On the city edge (`x = 0`, `x = cellsX`, `z = 0` or `z = cellsZ`): `amount := dt × 1e-7 × lava`,
      and it leaves the map. Otherwise find the neighbour with the lowest surface (`lava + alt`):
      `GetNeighborWithLowestLavaLevel` (`0x0075DA90`), below. No neighbour lower: no flow. Else
      `h := (surface − neighbourSurface) × 0.5`; `rate := h < 12 ? (4e-6 / 12) × h : 4e-6`; `h := min(h, lava)`;
      `amount := min(dt × h × rate, h)`.
   2. If `amount > 0`: if there is a neighbour, `SetThingsOnFire(neighbour, its old lava, amount)` (§11.5) and
      `neighbour.lava += amount`; then `lava −= amount`.
   3. **Solidification**, when `solidifying`: if `lava ≥ 3`: `f := solidWater` if the vertex has water
      (`depth > 0`), else `solid`; `lava −= f × lava`; `RaiseVertex(vertex, 0 × f × oldLava)`; the vertex joins the
      touched rectangle. Else if `lava ≥ 0.1`: `lava × 0.9`. Else `lava := 0`.
4. Grow the active rectangle by 1.
5. If the touched rectangle is not empty: `RedisplayTerrain(1, 1, touched, 3)` (texture-accumulation calls around it
   are render-only).

**`GetNeighborWithLowestLavaLevel(i)`** (checked in the bytes, TR4). The 8 neighbour offsets at terrain
`+0x84`…`+0xA0` are set in `Init` (`0x007500E0`–`0x00750129`), in this order (k = 0..7; "north" is z − 1):

| k | offset | neighbour |
|---|---|---|
| 0 | `−1` | W `(x − 1, z)` |
| 1 | `−1 − vertsX` | NW `(x − 1, z − 1)` |
| 2 | `−vertsX` | N `(x, z − 1)` |
| 3 | `1 − vertsX` | NE `(x + 1, z − 1)` |
| 4 | `+1` | E `(x + 1, z)` |
| 5 | `vertsX + 1` | SE `(x + 1, z + 1)` |
| 6 | `vertsX` | S `(x, z + 1)` |
| 7 | `vertsX − 1` | SW `(x − 1, z + 1)` |

`GetNeighbors(x, z)` (`0x00741710`) gives a mask, bit k for neighbour k: start `0xFF`; `x < 1` (signed) → `0x7C`;
`x ≥ cellsX` (unsigned) → `& 0xC7`; `z < 1` (signed) → `& 0xF1`; `z ≥ cellsZ` (unsigned) → `& 0x1F`.

1. `surf := r24(lava(i) + alt(i))`; `best := 0`; `result := −1`.
2. The edge neighbours, k = 0, 2, 4, 6 (W, N, E, S), when the mask allows: `drop := r24(surf − r24(lava(n) +
   alt(n)))`; if `drop > best` (strict, so the **first wins a tie** and a NaN never wins): `best := drop`,
   `result := k`.
3. `best := r24(√2 × best)` (√2 is `f32(sqrt(2.0))`, computed once).
4. The diagonals, k = 1, 3, 5, 7 (NW, NE, SE, SW), the same test against `best`; a diagonal that wins sets `best` to
   its own unscaled drop, which the next diagonals must beat.
5. Return `result` (the neighbour index is `i + offset[result]`).

So with no edge neighbour lower, any lower diagonal wins (the scaled best is 0).

*[quirk]* With the shipped factor 0 the lava never raises the ground, but every solidifying vertex still gets
`RaiseVertex(…, 0)`, which rewrites its water depth to `sea − alt` (§6) and posts `0x278E6003` with kind 3 for the
touched rectangle every tick. So the network manager re-checks the networks under solidifying lava every tick.

### 11.4 Burning *[compat]*

While the production rate is positive, `BurnSurroundingStuff` (`0x0075F340`) draws `(7 × 7) / 6 = 8` cells per tick
from the burn rectangle's random iterator (an exhausted iterator is reset, and that draw is lost), and for each cell
that the flammability simulator calls flammable (slot 6, `IsCellFlammable`) calls the fire manager's
`IgniteCell(x, z, false, true, 0)` (fire interface slot 5).

### 11.5 `SetThingsOnFire(vertex, oldLava, amount)` (`0x0075DE60`) *[compat]*

- If `oldLava < 1.5 ≤ oldLava + amount`: mode **ignite**.
- Else if `oldLava < 3 ≤ oldLava + amount`: mode **burn**.
- Else nothing.

With `x = vertex mod vertsX`, `z = vertex div vertsX`, for the four cells in the order `(x − 1, z − 1)`,
`(x − 1, z)`, `(x, z − 1)`, `(x, z)` (x outer, z inner): if the flammability simulator's `IsCellFlammable(cx, cz)`
(slot 6, false outside the city) is true: ignite → `IgniteCell(cx, cz, false, true, 0)`, burn →
`QuickBurnCell(cx, cz)`; then **`SetCause(3)` for that cell** (fire interface slots 5, 7 and 9; disasters.md §5).
So a call makes one `SetCause(3)` per flammable cell, 0 to 4 in all, and none for a cell that is not flammable
(checked at `0x0075DEE6`–`0x0075DF5A`, TR5). The thresholds compare `oldLava` and the rounded sum
`r24(oldLava + amount)`; a NaN fails both modes.

### 11.6 Messages

The lava subscribes to `0x26D31EC2` (PreCityShutdown → `Shutdown`) and `0x69E65264` (steam effect ended, render),
and handles the animation tick `0x86AD10EE` (→ `DoAnimationTick`: render frame, `DoLavaFlow`, burning, decals).

---

## 12. Messages and tick work

### 12.1 Terrain (`DoMessage` `0x0074A470`; foundations §11.6 cites an address inside it, `0x0074A590`) *[compat]*

| Message | Action |
|---|---|
| `0x26D31EC1` PostCityInit | `DoPostCityInit` (slot 40): creates the view and sets the view-ready flag |
| `0x86AD10EE` animation tick | `DoAnimationTick(dt)` (`0x0074A390`), below |
| `0x2A612643` (to the terrain itself) | the deferred edge reconciliation, foundations §11.6 (re-posted while a modal window runs, else `ReconcileEdgesWithNeighboringCities(0, 1, 1)`) |
| `0x470BD214` (from the 3D view window) | refresh the textures, then `RedisplayTerrain(1, 1, whole city, 1)` |
| `0xA6B79602` view update | render only |
| `0x66956816`, `0x6716A4E6` | subscribed, nothing |
| `0x0C4A6AF0`..`0x0C4A6AF6` | the script and cheat commands `GetAltitude`, `SetAltitude`, `GetCellAltitude`, `SetCellAltitude`, `GetSeaLevel`, `SetSeaLevel`, `ApplyTerrainBrush` |

`DoAnimationTick`:
1. If the view exists: count ticks up to 2; on the second tick post `0xCA63D517` (TerrainReconciliation) when the
   game preference `+0x4EC` is 0, else post `0x2A612643` to itself (foundations §11.6 has the full rule).
2. If the debug flood is on: one flood step.
3. Volcanic-ash regions (render).
4. Every 10 s of animation time (more than 9,999,999 µs accumulated), recompute the water proximity values of a
   pending rectangle (render).

### 12.2 Surface water

**Corrects (TR27).** The surface water has its own message target and its own animation-tick registration, and the
tick body is **gated off in the shipped binary**.

| Item | Rule |
|---|---|
| Message target | **yes** — the `cIGZMessageTarget2` sub-object at water `+0x08`, vtable `0x00AB3AD8`; `DoMessage` is its slot 3 (`0x00740160`, `ret 4`, returns AL). `ecx` is `+0x08`, so the water object is `ecx − 8`. Ctor `0x0073DEE4`/`0x0073DEF1` |
| Subscriptions | six, in `Init` (`0x0073FF7F`–`0x0073FFC3`): `0x26D31EC1`, `0x26D31EC4`, `0x26D31EC5` (no action), `0x89D3E9F3`, `0x99EF1142`, `0x99EF1143`. `Shutdown` (`0x0073E9DC`–`0x0073EA38`) removes exactly those six |
| Tick registration | **yes** — `cSC4AnimationTickManager` (vtable `0x00A86760`) `AddTickableObject` (slot 5, `0x00448B70`) with **priority `0xC0`** and `target = &this+0x08` (`0x00740013`–`0x00740029`); `Shutdown` removes it with the same priority (slot 6, `0x00448C70`). The **guessed value `0xC0` is correct** |
| Tick delivery | **not a queued message.** The manager keeps a reusable message typed `0x86AD10EE` and calls `target->DoMessage` **directly** (`0x00448A0F`), which is why the water never subscribes to `0x86AD10EE`. Nominal period 100 000 µs (10 Hz), delivered `dt` clamped to [2 000, 500 000] µs |
| Priority bands | `p ≥ 0xC0` → band 0; `0x80 ≤ p < 0xC0` → 1; `0x40 ≤ p < 0x80` → 2; `p < 0x40` → 3 (`0x00448BF6`–`0x00448C1D`). `0xC0` is the **lowest** priority that still lands in band 0 |

**The tick body never runs.** The animation-tick branch of `DoMessage` is gated on the water's byte **`+0x34`**
(`0x007401E9`–`0x007401EE`: `mov al,[edi+0x2c]; test al,al; je skip`, `edi = this+8`) and **nothing ever sets that
byte to non-zero**: the only writes in `0x0073CD50`–`0x00740900` are the two `= 0` of the constructor (`0x0073DF25`)
and `Init` (`0x0073FF24`), no `cISTESurfaceWater` slot (5–13 are all getters) exposes it, and a whole-image scan for
`mov byte [r+0x34], imm8` finds no writer in the terrain or water code. `[inferred: an external object could store
it]` — settle with a hardware write breakpoint on that byte. The same gate is inverted in `Rain` (`0x0073EAB0`),
which bails when `+0x34 != 0`, so the god-mode water brushes **do** run.

So `AnimateWater` (`0x0073FDD0`) and, through it, `UpdateFloodWaters` (`0x0073EF00`) are **dead** — each has exactly
one caller, and that caller is the gated handler (`0x007401F7`, `0x0073FDD6`). `UpdateFloodWaters` is a real
simulation step (it ping-pongs a per-vertex water-level field between record offsets `+8` and `+0xC` of the 0x24-byte
water records), but the shipped game never takes it.

**The rule for a replacement (this is the TR27 answer glue18 needs):** a surface water that does not animate is
**behaviour-compatible with the original**. To be byte-compatible, register the tick at priority `0xC0` and leave
`+0x34` at 0 — or omit `AnimateWater` entirely. **Starting to animate would change water depths over time and would
be a divergence, not a fix.** `AnimateWater` itself, when called, runs `UpdateFloodWaters()` then
`RedisplayTerrain(0, 1, &rect, 3)` with `rect = (0, 0, cellsX−1, cellsZ−1)` taken from terrain `+0x38`/`+0x3C`
(`0x0073FDDB`–`0x0073FE07`) — the whole city, not a changed rect. Messages: §10.1, §10.3.

---

## 13. Saved records

### 13.1 Terrain records *[compat]*

`Save` (slot 97, `0x007477C0`) writes, in order, the misc record, the altitudes, the vertex properties and the four
edges. `Load` (slot 96, `0x00747800`) reads misc, altitudes, properties (success flag at `+0x68`); the edges are read
on demand by the region code.

| Record (type / group / instance) | Version | Payload |
|---|---|---|
| `0x8A91E7E3 / 0xE98F9525 / 3` misc | u16 2 | u32 `vertsX`; u32 `vertsZ`; i32 terrain type (`+0x18`; a version < 2 with a value ≥ 1 loads as 0); f32 cached minimum; f32 cached maximum; f32 `+0x78`. 26 bytes |
| `0xA9DD6FF4 / 0xE98F9525 / 1` altitudes | u16 2 | `vertsX × vertsZ` f32, row-major (index order) |
| `0x8A91E7E0 / 0xE98F9525 / 2` vertex properties | u16 1 | one u16 flag word per vertex (§7) |
| `0xA9DD6FF4 / 0xE98F9525 / 0x64 + side` edges | u16 1 | u32 `n`, then `n` f32: side 0 = the column `x = 0` (`n = vertsZ`), 1 = the row `z = 0` (`n = vertsX`), 2 = the column `x = cellsX`, 3 = the row `z = cellsZ` |

The loader takes `vertsX` and `vertsZ` from the record and sets the vertex count from them; it does not check them
against the city size.

**The sea level is not saved** (TR10). `Init` (`0x007500D5`–`0x007500DD`) sets `+0x7C` from the process-wide tuning
value (`0x48CD7B21`, clamped to at least the minimum allowed height by `ReadTuningParameters`, shipped 250), and
`Load` (`0x00747800` and its three record readers) never writes `+0x7C`. So after a load the sea level is that
tuning value, whatever a script set before the save. The water array is then rebuilt from it (§10.1). Corpus: 129 × 129 (302 saves), 257 × 257 (11), 65 × 65 (189); terrain type 0 and `+0x78` =
308 in every save.

### 13.2 Class record

The terrain's serializable record (type `0xE98F9525`, group `0x299B2D1B`): u16 version 2 (`Read` accepts ≤ 2) and
a reference to the terrain view (class `0xC9B84E10`, whose own record is u16 1 + one byte, 0 or 1 in the corpus;
render only). This settles the "unnamed `0xC9B84E10` (3 bytes)" of save-records.md §3.2.

**`Read` loads the records itself (TR39).** `Read` is slot 4 of the serializable interface at terrain `base+0x08`
(`0x00745380`, vtable `0x00AB3CF0`; `__thiscall` with `this = base+8`, one argument = the input stream, `ret 4`,
result in AL). In order: read the u16 version through stream slot 7 (`+0x1C`); if it is above 2 return 0;
`SC4ReadGZSerializable(stream, 0xC9B84E10, &base[+0xE8], 0)` (`0x005D9730`, cdecl) — the saved view reference
lands in the view field `+0xE8`; `segment = SC4GetIGZDBSegment(stream)` (`0x0045BA70`, cdecl, the DB segment the
stream reads from); then `base->Load(segment)` (slot 96 through the primary vtable, `0x007453C7`) and return
its AL. No other code calls slot 96 directly: its only other entry is slot 95 `LoadObliterated` (`0x00742170`), a
tail jump to slot 96. So on a city load the records of §13.1 are read **during `Read`**, before `Init` and
`PostCityInit`, from the same segment as the class record. A replacement whose `Read` only parses the class
record never loads the altitudes.

### 13.3 Surface water

The surface water (CLSID `0x0B8DA7DB`) writes u16 1 and a reference (read with IID `0xC70BD37E`, the terrain).
`SaveWaterInfo` (record `0x8A91E7EF / 0xE98F9525 / 4`, u16 1, u32 count, per vertex 4 f32 + a vector + 1 f32)
exists in the code, but **no corpus save holds that record**: the water is rebuilt from the sea level on every load
(§10.1). *[compat]* A replacement must not save lakes either.

---

## 14. Nation

What `cSC4Nation` adds beyond neighbors.md and the economy modifier: see [nation.md](nation.md). In short, nothing
the simulation reads except `GetEconomyModifier` (demand.md §8.2, with two extra quirks in nation.md).

---

## 15. Slot tables *[compat]*

### 15.1 `cISTETerrain` (vtable `0x00AB3D18`)

Return channels and callee pops are in docs/abi/interfaces.tsv. Names that the export folded or got wrong are
corrected here (and proposed for that table).

| Slot | Function | Notes |
|---|---|---|
| 0–4 | QueryInterface, AddRef, Release, Init, Shutdown | |
| 5 | `IsInitialized()` | byte `+0x0C` (exported as a folded `IsInDelayedStop`) |
| 6 | `CellWidth()` | 16.0 |
| 7, 8 | `GetCellCountX()`, `GetCellCountZ()` | `+0x28`, `+0x2C` (exported as folded getters) |
| 9, 10 | `GetVertIndex`, `GetVertex` | §2.1 |
| 11 | `LocationIsInBounds` | §3 |
| 12–14, 17, 19–24 | height reads | §4 |
| 15, 16 | `GetAltitudes`, `GetWaterDepths` (rect copies) | |
| 18 | `GetMaxSurfaceLevel` | §5 |
| 25 | `GetTerrainGridIndex` | §15.6.1 |
| 26–28 | normals | §4.3 |
| 29–38, 53, 55, 92 | water reads | §5 |
| 39 | `AreSeaVertices(rect*)` | §15.6.1 — "all sea", not a presence test |
| 40 | `DoPostCityInit` | §12.1 |
| 41 | `SetTerrainType` | §6 |
| 42 | `GetTerrainType()` | `+0x18` (exported as a folded `GetTractSize`) |
| 43, 44 | `CreateFlatAreas`, `CreateShoreCliffs` | §15.6.2 (data4 0) |
| 45, 46 | `SetSeaLevel`, `ChangeSeaLevel` | §6, §15.6.2 |
| 47 | `GetDefaultTerrainRaiseLowerAmount` | **Corrects:** `0x00B0D588`, tuning `0x48CD7B73`, shipped 40.0 — not a constant (§2.4) |
| 48 | `ChangeTerrainLevel` | §15.6.2 (data4 1) |
| 49–52 | setters, lock | §6 |
| 54 | returns `+0xE4` | render object (exported as a folded `GetSelectedCity`) |
| 56 | `StartFlood` | §15.6.2 — the +500 m / 20..39 block is right, but it also snapshots, arms and steps a flood |
| 57 | `ApplyBrush` | §9.2 |
| 58–63 | brush and scrub helpers | §9.6; 58, 61, 62 in §15.6.1 |
| 64–67 | waves, view, lighting | render |
| 68, 69 | `RedisplayTerrain` | §8.1 |
| 70–77, 81, 82 | flags, hiding | §7 |
| 78–80 | texture accumulation | render |
| 83 | `GetWaterSurfaceDisplayState()` | byte `+0xDD` (exported as a folded `IsPageFlippingEnabled`) |
| 84 | `SetWaterSurfaceDisplayState` | §8.4 |
| 85, 86 | plumbing display state | a global byte |
| 87, 88 | `Begin` / `EndLongTerrainOperation` | §8.3 |
| 89 | `ReconcileEdgesWithNeighboringCities` | foundations §11.6 |
| 90, 95 | `SaveObliterated`, `LoadObliterated` | |
| 91 | deleting destructor | |
| 93, 94 | `FlipInternalTriangulation`, `FlipExternalTriangulation` | |
| 96, 97 | `Load`, `Save` | §13 |

### 15.2 `cISTESurfaceWater` (vtable `0x00AB3B00`)

0 QueryInterface, 1 AddRef, 2 Release, 3 Init, 4 Shutdown, 5–13 as §10.2 (slot 9 is exported as a folded
`GetTransientAuraGrid`; it returns the body-of-water map at `+0x4C`), 14 deleting destructor.

### 15.3 Consumers' slots (for the implementer)

BDS 7, 8, 13; flora 5, 13, 26, 29; landfill 13, 19, 29, 49, 68; land value 12, 13, 23, 24, 31, 36; lot developer
6, 7, 8, 12, 13, 30, 31, 34, 37; demolition 14, 37, 49, 51, 68; network manager 13; pollution 13; protection 13, 29;
plumbing 7, 8, 34, 68, 69; traffic network 34; disasters 57, 69, 79, 87, 88; surface water 6, 8, 9, 12, 13 (traffic
network, zones, city situation, watercraft).

---

### 15.4 Re-entrancy (for the glue, TR18)

The original takes no lock: the lava and the brushes call their collaborators synchronously and keep running. What
they call, as traced in the bytes:

| Path | Synchronous callees | Do they call the terrain or surface water back? |
|---|---|---|
| lava flow and burning (§11.3–§11.5) | flammability `IsCellFlammable` (a grid read); fire manager `IgniteCell`, `QuickBurnCell`, `SetCause`; the terrain's own `RaiseVertex` and `RedisplayTerrain` | No. `QuickBurnCell` only appends to the toast list. `IgniteCell` creates fire occupants and inserts them in the occupant manager, whose Insert message is **queued** (occupant-manager.md). The fire manager, fire occupant and flammability code hold no reference to the terrain globals (`0x00B43CF4`, `0x00B43CF0`) or city slot 84. Deeper effect and audio paths of a new fire occupant were not traced *[inferred]* |
| brush kinds 0, 2, 5, 7 (all the simulation uses) | none outside the terrain: the vertex setters, the terrain generator's draw, and the queued posts of `RedisplayTerrain` | No |
| brush kind 6 (hydrological erosion, §9.4.4; reachable through `ErosionBrush`) | terrain-internal only: `FUN_00741E30` (grow rect), `FUN_00746AD0` (the pass, which calls `FUN_00744760` for the flow neighbour). It reads and writes the surface water's **array** through the terrain's `+0x24` pointer and never calls the surface water object | No: a field access, not a call |
| brush kind 1 (god-mode flora painting, `0x00745A40`) | the terrain's own `GetAltitude`, then the flora placer (terrain `+0x104`, slot 17) and a simulator (`0x00B43D60`, slot 6); only when the terrain's count at `+0x108` is non-zero | Possibly, read-only *[inferred]* |
| brush kind 8 (lake, §9.4.5) | only the terrain's own `SetAltitudeAtVertex` (vtable `+0xC8`, slot 50), `RaiseVertex` (`0x00744CA0`) and `LowerVertex` (`0x00744AF0`) | No |
| brush kind 9 (rain, §9.4.5) | the surface water's own `FUN_0073EAB0` → `FUN_0073E350` → `FUN_0073DFF0`, which then calls the **terrain** back: `GetVertex` (slot 10, `+0x28`) and `FUN_00743A10` | **Yes, read-only**: the surface water calls the terrain's `GetVertex` while the brush is in progress. It writes only the water array |
| brush kind 10 (spring, §9.4.5) | the surface water's `FUN_0073D4D0`, which writes one float of the water array | No |
| every `RedisplayTerrain`, `SetSeaLevel`, `SetTerrainType`, slot 84, long-operation messages | `MessagePost` (queued) | No: receivers run on the next pump |

So on the simulation's paths the original never re-enters the terrain from a lava or brush callback. The cores16
contract (a re-entrant call on the same handle is refused and counted) is therefore compatible: the count should
stay 0, and queueing is not needed. A non-zero count in a run points at an untraced path and is worth a report. If
the glue ever drives kinds 1 or 8–10, it must run the flora and water calls as part of the core's own operation (or
outside the borrow), not refuse them. **The one real re-entrancy in the brush code is kind 9**, where the surface
water's rain walks back into the terrain for `GetVertex` (slot 10) and `FUN_00743A10` while `ApplyBrush` is still on
the stack. Both read the terrain's own grid, so a replacement that performs the rain *inside* the terrain borrow must
make those two queries internal rather than going out through the slot table — or run the rain outside the borrow.
Kind 6 is the other case worth noting: it writes the surface water's array directly through the terrain's `+0x24`
pointer without calling the object, which a replacement should treat as the same component pair, not as a host call.

---

## 15.5 The terrain 3D view (`cSTETerrainView3D`, class `0xC9B84E10`) — TR21 *[compat]*

**Corrects (TR21).** The Scope paragraph above puts "the 3D terrain view" out of scope. That is what blocks
`enable = terrain`: the view is **created and initialised by the terrain itself**, its pointer is handed to the zone
developer and other components, and the view reads the terrain's vertex array **directly** rather than through a
slot. A replacement cannot survive past `PostCityInit` without bringing the view up. This section replaces the gap.
Everything below is checked in the Windows instruction bytes of `SimCity4.exe` (SHA-256 `3bc5c7fe…`) unless tagged
*[inferred]*.

### 15.5.1 Who creates it, from what, and when

The **terrain's own `PostCityInit` handler** creates it — `DoPostCityInit`, slot 40, `0x00749540`, reached from the
terrain's `DoMessage` (`0x0074A470`) when it sees `kSC4MessagePostCityInit` `0x26D31EC1` (`0x0074A569`:
`lea ecx,[edi-4]; call [edx+0xA0]`, i.e. slot 40 of the terrain base). Nothing else in the image constructs a
`cSTETerrainView3D`; the factory path is the only alternative entry.

`DoPostCityInit` runs **once** (guard byte at terrain `+0x14`) and does, in order (`0x00749540`–`0x00749659`):

| # | Step | Bytes |
|---|---|---|
| 1 | if terrain `+0x14` is set, return; else set it to 1 | `0x00749544`, `0x0074954F` |
| 2 | if the 3D camera global `0x00B43DD8` is non-null, copy its `+0xC` (the zoom index) into terrain `+0xBC` | `0x00749553`–`0x0074955F` |
| 3 | if terrain `+0xE8` (the view) is null, create it: **try the GZCOM class factory first** — `CreateInstance(CLSID 0xC9B84E10, IID 0x6771477D, &out)`, then store `out - 0x0C` | `0x00749565`–`0x0074959F` |
| 4 | on factory failure, fall back to `operator new(0xE8)` + `cSTETerrainView3D::cSTETerrainView3D` (`0x007567C0`), store the result, then `AddRef` (slot 1) | `0x007495AB`–`0x007495D1` |
| 5 | `view->Init(terrain)` — `0x0075C420`, `__thiscall`, `ecx` = the **view base**, one stack argument = the terrain | `0x007495D4`–`0x007495DC` |
| 6 | load the object at terrain `+0x100` into `ecx`, **then** set the **view-ready** flag terrain `+0xEC` = 1, **then** call that object's slot 5 (`+0x14`): the weather simulator's `DoCompleteSimulation` (§15.5.13.1, TR34) | `0x007495E1`, `0x007495E7`, `0x007495EE`–`0x007495F0` |
| 7 | `0x00743B60` (normals and cliff marks) and `0x007490B0` (texture layers, weather moisture, view row-dirty), each thiscall with `&terrain+0x40` (the whole-city vertex rect) (§15.5.13.2–3, TR33) | `0x007495F3`–`0x00749601` |
| 8 | `terrain+0xF0 := view+0x70`, `terrain+0xF4 := view+0x74`, each `AddRef`'d | `0x00749606`–`0x00749634` |
| 9 | `terrain+0xF8 := (view+0x0C)->GetOverlayManager(0)` (`view+0x78`), `AddRef`'d | `0x00749637`–`0x00749655` |

**Order relative to `PostCityInit`:** the view exists *because of* `PostCityInit`, not before it. `RedisplayTerrain`
(slot 68) posts nothing until `+0xEC` is set (§8.1 quirk 3), and that flag is written in step 6 — **after**
`view->Init` returns but **before** the `+0x100` slot-5 call, so anything that call does is already allowed to
redisplay. The correct ordering is: construct the view → `Init(terrain)` → set `+0xEC` → then the view is usable.

**On a loaded city** `Read` has already put the deserialised view in `+0xE8`, so steps 3–4 are skipped and step 5
`Init`s that object (§15.5.13.5).

**It must run before the zone developer's `PostCityInit` handler.** Both are subscribers of the same queued message
`0x26D31EC1`. The zone developer's handler (`0x007328D0`) does `SL::TerrainView` (`0x00602160`, a tail-call to
terrain slot 66) and then **`AddRef`s the result with no null check** — `0x00732915`–`0x00732921`
(`call 0x602160; mov [esi+0x30],eax; mov edx,[eax]; call [edx+4]`). If `GetView` still returns null there, the game
faults at `0x0073291D`. The terrain subscribes first — in its `Init` at `0x007502E5`, against the zone developer's
`Init` at `0x00733611`, and foundations §3.6 initialises terrain (component 49) well before the zone developer —
and the game does not fault on load, so the terrain's handler is delivered first *[inferred: the message server
delivers in registration order]*. What would settle it: the message server's target-list walk. **A replacement must
make the terrain's `DoPostCityInit` run before any consumer calls slot 66**, by whatever ordering its own host uses.

### 15.5.2 Object layout *(size `0xE8`; verified in the Windows constructor `0x007567C0`)*

| Offset | Meaning | Set by |
|---|---|---|
| `+0x00` | vtable `0x00AB4480` (the **object base** interface) | ctor `0x00756815` |
| `+0x04` | `cRZUnknown` sub-object, vtable `0x00AB4468` | ctor `0x0075681B` |
| `+0x0C` | vtable `0x00AB4410` — the interface **IID `0x6771477D`**, i.e. what slot 66 returns | ctor `0x00756821` |
| `+0x10` | vtable `0x00AB4400` — the **message target** (what is passed to `AddNotification`) | ctor `0x00756828` |
| `+0x14` | vtable `0x00AB43E8` | ctor `0x0075682F` |
| `+0x18` | byte: already-`Init`ed | ctor 0, `Init` `0x0075C437` |
| `+0x19` | byte: some "done once" flag | ctor 0 |
| `+0x1C` | `cISC4City*` (from global `0x00B43CEC`) | `Init` `0x0075C46B` |
| `+0x20` | `cISTETerrain*` — **the terrain** (the `Init` argument) | `Init` `0x0075C48A` |
| `+0x24` | surface water (from global `0x00B43CF0`, city slot 83 `GetSurfaceWater`) | `Init` `0x0075C499` |
| `+0x28`, `+0x2C` | collaborator pointers: city slot 36 `GetZoneManager` (`+0x90`) and city slot 33 `GetLotManager` (`+0x84`) | `Init` `0x0075C51A`/`0x0075C537`, `AddRef`'d |
| `+0x30`, `+0x31` | byte flags: `+0x30` = 0 at ctor; `+0x31` = 1 at ctor, cleared until the first `DrawTerrainLayers` has called terrain slot 67 once (`0x0075BF64`–`0x0075BF81`) | ctor `0x00756800`/`0x00756838` |
| `+0x34` | the terrain-side lighting-hook argument (`DrawTerrainLayers` passes `base+0x34` to terrain slot 67, `0x0075BF70`) | ctor 0 |
| `+0x38`..`+0x3C` | `+0x38` = the draw-context pointer cache (`Update` reads `[[V+0x38]+0xC]+0x1FC` for visibility, `0x0075BFD5`–`0x0075BFE6`); `+0x3C` = the previous water-glare flag | `Init`/`SetWaterGlareStatus` |
| `+0x3D` | byte: water-glare flag (read as `IsWaterGlareEnabled`) | ctor 0 region (`0x00756800` zeroes `+0x30`; `+0x3D` is cleared by the ctor's dword clears at `+0x44`/`+0x5C` neighbours — treated as 0) |
| `+0x40` | the water-colour update rect (4 dwords) | `SetWaterGlareStatus`, `FUN_00756000` |
| `+0x44` | float; the `0xC9DA96EA` handler stores the message's `GetData2()` here (`0x00757636`–`0x0075763C`) | ctor 0 |
| `+0x48`, `+0x49` | byte flags: `+0x48` = "replace the water-colour rect next time" (ctor 1; cleared by `FUN_00756000` path A, `0x00756042`; also gates `DrawWaterLayers`' `SetColorOfWaterSurface` recompute, `0x007572C4`–`0x007572D1`); `+0x49` = the water-glare flag | ctor `0x0075683B`/`0x0075683E` |
| `+0x4C` | the second water-colour update rect (4 dwords) — `SetLightingUpdateRect`'s helper and `FUN_00756000` path B write here | ctor 0 |
| `+0x5C`, `+0x60` | counters / pointers: `+0x5C`/`+0x60` hold the level-cell row bounds used by `LevelCell`'s map walk | ctor 0 |
| `+0x64`, `+0x68` | float ±FLT_MAX (a min/max band); `Update` overwrites them with terrain slots 22/21 (`0x0075BFF5`–`0x0075C003`) | ctor `0x00756841`, `0x00756848` |
| `+0x6C` | `cSTESelectedTerrainArea*` (ctor `0x00751BC0`) | `Init` `0x0075C56C` |
| `+0x70` | `cSTEWaterRipple*` | `Init` `0x0075C5A0` |
| `+0x74` | a further collaborator pointer (the wave layer) | `Init` |
| `+0x78`, `+0x7C`, `+0x80`, `+0x84` | the **four overlay managers** (ctor `0x0073BC20`), ordered `(0,0)`, `(0,1)`, `(1,0)`, `(1,1)` | `Init` `0x0075C5D0`–`0x0075C6A7`, `Init`ed at `0x0075C6B6`–`0x0075C6E7` via `cSTEOverlayManager::Init` `0x0073CB60` |
| `+0x88`..`+0xA0`, `+0xB0`, `+0xB4`, `+0xBC`..`+0xD0`, `+0xE0`, `+0xE4` | further state (`+0x88` = a `cS3DTextureBinding*` released at `Init`/`Shutdown`); `+0xB8` is the byte the `0xA6B79601` / `0xA6B79611` handlers set | ctor |

**The `this` a slot receives depends on which vtable it belongs to** *[compat]* — this is the trap that makes a
naive layout wrong:
- slots of `0x00AB4480` (at `+0x00`) get `ecx` = the **object base**;
- slots of `0x00AB4410` (at `+0x0C`) get `ecx` = **`base + 0x0C`**, so every offset in them is `+0x0C` higher than
  the table above;
- the message target at `+0x10` gets `ecx` = `base + 0x10`.

Two independent confirmations: `ClearCurrentSelections` (`0x00751ED0`, vtable `0x00AB4410` slot 9) does
`mov ecx,[ecx+0x60]`, which with `ecx = base+0x0C` is `base+0x6C` = the selected-terrain-area pointer;
`GetOverlayManager` (`0x007558A0`, slot 18) does `mov eax,[ecx+eax*4+0x6C]; ret 4`, which with `ecx = base+0x0C`
gives `base+0x78 + index*4` = the overlay manager array.

### 15.5.3 The vtables, with return channels

Callee pops are the `ret N` of each target. Booleans come back in **AL only** (quirk 10, same as the terrain's own
boolean slots).

`0x00AB4480` at `+0x00` — object base, 5 slots:

| Slot | Offset | Target | Pop | Args | Returns |
|---|---|---|---|---|---|
| 0 | `+0x00` | `0x00752730` `QueryInterface` | 8 | 2 | EAX (bool in AL) |
| 1 | `+0x04` | `0x005BE3E0` `AddRef` | 0 | 0 | EAX |
| 2 | `+0x08` | `0x005BCB30` `Release` | 0 | 0 | EAX |
| 3 | `+0x0C` | `0x0075BFD0` `Update` | 4 | 1 | EAX; touches ST0 |
| 4 | `+0x10` | `0x00752700` `Pick` | 16 | 4 | via-call |

`0x00AB4468` at `+0x04` — the `cRZUnknown` sub-object, 6 slots: `0x007568F0`, `0x005BE420` (AddRef), `0x005BCB60`
(Release), `0x00756900`, `0x0090D981` `RemoveRef`, `0x009D7E63`; all `ret 0` except 4 and 5 which are shared helpers.

`0x00AB4410` at `+0x0C` — **IID `0x6771477D`**, the interface slot 66 hands out, 22 slots:

| Slot | Offset | Target | Pop | Args | Returns |
|---|---|---|---|---|---|
| 0 | `+0x00` | `0x00756910` `QueryInterface` | 8 | 2 | EAX |
| 1 | `+0x04` | `0x00761850` `AddRef` | 0 | 0 | EAX |
| 2 | `+0x08` | `0x00761860` `Release` | 0 | 0 | EAX |
| 3 | `+0x0C` | `0x0075A230` `Pick` | 20 | 5 | ST0 (x87) |
| 4 | `+0x10` | `0x007526E0` `MarkSelected` (textured cell, 7 args) | 28 | 7 | AL |
| 5 | `+0x14` | `0x007526C0` `MarkSelected` (7 args) | 28 | 7 | AL |
| 6 | `+0x18` | `0x007526A0` `MarkSelected` (3 args) | 12 | 3 | AL |
| 7 | `+0x1C` | `0x00752680` `MarkSelected` (3 args) | 12 | 3 | AL |
| 8 | `+0x20` | `0x00752660` `MarkSelected` (3 args) | 12 | 3 | AL |
| 9 | `+0x24` | `0x00751ED0` `ClearCurrentSelections` | 0 | 0 | void |
| 10 | `+0x28` | `0x00752860` `SetAutomaticCellCursorUpdateStatus` | 4 | 1 | void |
| 11 | `+0x2C` | `0x007568E0` `IsWaterGlareEnabled` | 0 | 0 | **AL only** (byte `base+0x49`) |
| 12 | `+0x30` | `0x00756080` `SetWaterGlareStatus` | 4 | 1 | void |
| 13 | `+0x34` | `0x00755F30` `SetLightingUpdateRect` | 4 | 1 | void |
| 14 | `+0x38` | `0x00752870` `DrawTerrainQuad` | 12 | 3 | void |
| 15 | `+0x3C` | `0x00752940` `DrawTexturedTerrainQuad` | 24 | 6 | void |
| 16 | `+0x40` | `0x00759750` `LevelCell` | 12 | 3 | ST0 |
| 17 | `+0x44` | `0x00758620` `RemoveCellLevelling` | 8 | 2 | void |
| 18 | `+0x48` | `0x007558A0` `GetOverlayManager(index)` | 4 | 1 | **EAX** = `base+0x78+index*4` |
| 19 | `+0x4C` | `0x0075A770` `HandleCheatCommand` | 4 | 1 | AL |
| 20 | `+0x50` | `0x00752AE0` `GetDisplayGridFlag` | 0 | 0 | **AL only** (the **static** `0x00B4C741`, not a field) |
| 21 | `+0x54` | `0x00755E80` `SetDisplayGridFlag` | 4 | 1 | void |

`0x00AB4400` at `+0x10` — the **message target**, 8 slots: `0x00756920`, `0x00756930`, `0x00756940`, then
**slot 3 `+0x0C` = `DoMessage` `0x00757560`** (pop 4, 1 arg, returns AL), then slots 4–7 are a repeat of
`0x00AB4410` slots 0–3.

`0x00AB43E8` at `+0x14` — 6 slots: `0x00756950`, `0x00756960`, `0x00756970`, `0x00752CD0` (pop 4), `0x007527D0`
(pop 4), `0x007523E0` (pop 0).

### 15.5.4 `Init(view, terrain)` — `0x0075C420`, `__thiscall`, `ret 4` *[compat]*

`ecx` = the view base; the single stack argument is the `cISTETerrain*`. Returns 1 on success, **0 if the view is
already initialised** (`+0x18` set, `0x0075C437`–`0x0075C445`).

What it takes, in order:

1. `StaticInit()` (`0x0075C0C0`) once per process, guarded by the static byte `0x00B4C740` (`0x0075C420`).
2. Read the global `0x00B43CD8` byte at `+1` into the static `0x00B4C741`, unless `+0x19` is set (`0x0075C453`).
3. `+0x1C` := city (`0x00B43CEC`), `AddRef` (slot 1) (`0x0075C46B`–`0x0075C47C`).
4. `+0x20` := the terrain argument, `AddRef` (`0x0075C48A`–`0x0075C491`).
5. `+0x24` := surface water (`0x00B43CF0`), `AddRef` (`0x0075C499`).
6. **From the terrain:** `GetCellCountZ()` (slot 8) then `GetCellCountX()` (slot 7), pushed as
   `FUN_007595a0(cellsX, cellsZ)` (`0x0075C4B5`–`0x0075C4C1`). That helper caches the geometry into process statics:
   `0x00B4C744` = cellsX, `0x00B4C748` = cellsZ, `0x00B4C74C` = **vertsX = cellsX+1**, `0x00B4C750` = vertsZ,
   `0x00B4C754` = vertex count, `0x00B4C734` = sizeX in metres (`0x007595A0`–`0x00759602`).
7. **From the terrain:** `GetTerrainType()` (slot 42). If it differs from the static `0x00B0D6FC`, call
   `ChangeTerrainType` and `FUN_007571D0` (`0x0075C4C3` region).
8. City slot 36 (`+0x90`) → `+0x28`; city slot 33 (`+0x84`) → `+0x2C`.
9. Construct the selected-terrain-area (`+0x6C`), the water ripple (`+0x70`) and the four overlay managers
   (`+0x78`..`+0x84`) and `Init` them.
10. Build the dirty rects from the statics and call `SetLightingUpdateRect` (slot 13).
11. **Subscribe to messages** through the message server global `0x00B43CCC`, slot 5 (`AddNotification`), target =
    `view+0x10`: `0x277A9364`, `0x278E6003` (heights), `0x678E6E3A` (water), `0xA6B79601`, `0xA6B79611`,
    `0xC9DA96EA` (`0x0075C810`–`0x0075C868`).
12. **Register draw callbacks** through the global `0x00B43DD0`, slot 32 (`0x80`), signature
    `Register(obj, layer, priority)` (`0x0075C86B`–`0x0075C964`); each of the four overlay managers is passed at
    its `+4`:

    | Object | Layer | Priority |
    |---|---|---|
    | view base | 0 | 0 |
    | overlay `+0x78` `+4` | 2 | 0 |
    | view base | 2 | 1000 (`0x3E8`) |
    | overlay `+0x7C` `+4` | 2 | 1100 (`0x44C`) |
    | `+0x6C` selected area | 5 | 1000 |
    | overlay `+0x80` `+4` | 5 | 1000 |
    | `+0x70` water ripple | 5 | 1100 |
    | `+0x74` | 5 | 1100 |
    | overlay `+0x84` `+4` | 5 | 1100 |
    | `+0x88` | 5 | 1100 |

    The registration is skipped entirely when `0x00B43DD0` is null (`0x0075C871`), so a headless host survives
    without it.

**Minimum a `cISTETerrain` must supply before the view can `Init`:** slots **1** (AddRef), **7** (`GetCellCountX`),
**8** (`GetCellCountZ`) and **42** (`GetTerrainType`). Those four are all the terrain interaction at construction
time. Everything else the view needs from the terrain is needed later, at draw time (§15.5.5).

**The two globals `Init` dereferences without a null check — check these first if it faults.** These are the most
likely cause of "faults in the terrain 3D view" from a replacement, because `Init` runs inside `DoPostCityInit` and
nothing else in the sequence touches these objects:

| Global | Use | Bytes | Safe? |
|---|---|---|---|
| `0x00B43CD8` | `mov cl,[eax+1]` to seed the static `0x00B4C741` | `0x0075C453`–`0x0075C458` | **no null check** |
| `0x00B43CF0` | stored to `view+0x24`, then `AddRef` (slot 1) | `0x0075C499`–`0x0075C4A5` | **no null check** |
| `0x00B43CEC` | city, stored to `view+0x1C` | `0x0075C46B`–`0x0075C47C` | null-checked |
| `0x00B43CCC` | message server, slot 5 subscriptions | `0x0075C810`–`0x0075C818` | null-checked |
| `0x00B43DD0` | draw registry, slot 32 registrations | `0x0075C86B`–`0x0075C873` | null-checked |

`0x00B43CF0` is the **surface water**: `SL::OnAppInit`-family code fills it from city slot 83 `GetSurfaceWater`
(`+0x14C`) at `0x00601C40`–`0x00601C4C`. So a replacement that makes `cISTESurfaceWater` obtainable only through
its own handle and never through city slot 83 / that global leaves `0x00B43CF0` null, and the view faults at
`0x0075C4A3` the moment `DoPostCityInit` runs. `0x00B43CD8` is an app-level object the 3D view is created from
(`SL::OnView3DInit`'s source, `0x00602590`); a host with no 3D view window leaves it null and faults at
`0x0075C458`. Both are cheap to check before anything else.

### 15.5.5 What the view reads off the terrain — including outside the slot table

The view keeps the terrain at `base+0x20` and reaches it two ways: through the slot table, and directly through
field pointers. Everything below was read from the Windows bytes of the named functions (TR21 contract work,
2026-10-20, re-terrain3d); the message handlers' rebuild-vs-live rule is §15.5.8 and the draw-time statics are
§15.5.9.

**Through slots** (a replacement must answer these correctly once the view is drawing):

| Terrain slot | vtable offset | Called from | Used for |
|---|---|---|---|
| 1 `AddRef` | `+0x04` | `Init` `0x0075C48F` | the terrain reference |
| 7 `GetCellCountX` | `+0x1C` | `Init` `0x0075C4BD` | the geometry cache (`FUN_007595A0`) |
| 8 `GetCellCountZ` | `+0x20` | `Init` `0x0075C4B5` | ditto |
| 21 `GetMinimumAltitude` | `+0x54` | `Update` `0x0075C000` | the draw band (`view+0x68`) |
| 22 `GetMaximumAltitude` | `+0x58` | `Update` `0x0075BFF5` | the draw band (`view+0x64`) |
| 29 `GetSeaLevel` | `+0x74` | heights handler `0x00756A00` and water handler `0x00756B23` | the water plane the draw buffers are refilled against |
| 42 `GetTerrainType` | `+0xA8` | `Init` `0x0075C4CE`, `DrawTerrainLayers` `0x0075BF15` | `ChangeTerrainType` |
| 67 | `+0x10C` | `DrawTerrainLayers` `0x0075BF74` (once, gated by `view+0x31`) | the first lighting/rect refresh |
| 68 `RedisplayTerrain` | `+0x110` | `SetDisplayGridFlag` `0x00755EC2` | the grid toggle's whole-city redisplay |
| 70 | `+0x118` | `DrawPlumbingInfo` `0x00759C7C` | per-cell flag read (`IsTriangulationFlipped`) |
| 76 | `+0x130` | `DrawPlumbingInfo` `0x00759C57` | per-cell flag read (`IsWholeCellHidden`) |
| 83 `GetWaterSurfaceDisplayState` | `+0x14C` | `DrawWaterLayers` `0x007572B1` | the water-surface display gate |

`SetDisplayGridFlag` (slot 21) is the only **write-back**: on a real change it calls terrain slot 68 with
`(1, 1, &wholeCity, 0)` (`0x00755E80`–`0x00755EC8`), so toggling the grid posts a full heights+water redisplay.

**Directly, outside the slot table** *[compat]* — this is the part §2.3 called "internal" and it is not:

```
00752af4  mov  ecx, [ecx+0x20]                 ; the terrain
00752af7  mov  ecx, [ecx+0x6c]                 ; terrain +0x6C = the vertex array   (DIRECT)
00752b01  mov  edi, [0xb4c74c]                 ; vertsX (the view's cached static)
00752b09  imul eax, esi / add eax, edx         ; index = vertsX*z + x
00752b0e  lea  eax, [eax + eax*8]
00752b11  movzx ecx, word [ecx + eax*4 + 0x22] ; record size 0x24, flags at +0x22
```

(`cSTETerrainView3D::SetTriangleIndicesForCell`, `0x00752AF0`.) So the view walks the terrain's **vertex array at
terrain `+0x6C`** with record size **`0x24`** and reads the uint16 flags at record **`+0x22`** — exactly the §2.2
layout — using its own cached `vertsX`. A replacement whose `+0x6C` is null, or whose record size is not `0x24`,
faults the first time the view draws. The same direct access is used in the span and draw code
(`0x0075ABA0` `ComputeTextureSpansForRow`, `0x0075BD10` `DrawTerrainSurface`).

**MATCHES (§7).** The four flag bits the view tests are exactly the §7 ones, so the drawing depends on those being
right and nothing new is introduced:

| Test in `0x00752AF0` | Bit | §7 meaning |
|---|---|---|
| `shr bl,5; and bl,1` (`0x00752B36`) | `0x0020` | even triangle hidden |
| `shr al,6; and al,1` (`0x00752B16`) | `0x0040` | odd triangle hidden |
| `shr eax,9; and al,1` (`0x00752B23`) | `0x0200` | cell flag 4 |
| `test ch,0x30` (`0x00752B48`) | `0x3000` | the two triangulation flips (same mask as slot 70) |

`cSTETerrainView3D::Update` (`0x0075BFD0`, the object-base slot 3) is the draw entry. It returns immediately unless
the draw context says the view is visible (`[drawCtx+0x38]` → `+0x0C` → byte `+0x1FC`, `0x0075BFDB`–`0x0075BFE6`),
null-checks the terrain at `+0x20`, then reads terrain slot **22 `GetMaximumAltitude`** and slot **21
`GetMinimumAltitude`** as **x87 floats** into `view+0x64` and `view+0x68` (`0x0075BFF3`–`0x0075C003`). Both must
return in `st0`.

**The surface water's array too** *[compat]* — `Pick` (`0x0075A230`, at `0x0075A357`–`0x0075A35F`) and the water
message handler (`FUN_00756B00`, at `0x00756B05`–`0x00756B14`) read **`view+0x24`** (the `cISTESurfaceWater*`)
and dereference it: `Pick` reads **water `+0x18`** (the per-vertex water array) for the depth offset under the
cursor, and `FUN_00756B00` reads **water `+0x18`** (record size `0x24`, depth at record `+0x00`) to refill the
water draw buffer. Water `+0x18` is the same array §2.2 gives, reached through the view's own collaborator pointer
rather than the terrain — a replacement whose water object does not keep the array at `+0x18` with `0x24`-byte
records faults in both places.

### 15.5.6 What slot 66 is, and what zones.md §5.2's call does — TR21

**Slot 66 is `GetView`, `0x00741810`** (already in `interfaces.tsv` as `cSTETerrain::GetView`, pop 0, 0 args,
EAX):

```
00741810  mov  eax, [ecx + 0xe8]   ; the view OBJECT BASE
00741816  test eax, eax
00741818  je   -> return 0
0074181a  add  eax, 0xc            ; return base + 0x0C
0074181d  ret
```

So the pointer every consumer holds is **`view + 0x0C`** (IID `0x6771477D`), *not* the object base. That is the same
adjustment `DoPostCityInit` makes in reverse when the factory hands it `out` (`add eax,-0x0C` at `0x00749596`).
`SL::TerrainView` (`0x00602160`) is a tail-call to this slot: `mov ecx,[0x00B43CF4]; jmp [eax+0x108]`, returning
**null** when the terrain is null or the view does not exist yet.

What zones.md §5.2's slot-66 call **actually does**: in the zone developer's `PostCityInit` handler it is
`SL::TerrainView` → `GetView`, and the result is cached at the zone developer's `+0x30` and `AddRef`'d — with the
null-unsafety noted in §15.5.1. It is a pure accessor: it allocates nothing, initialises nothing and has no side
effects. Everything the zone developer then does with it (slot 9 `ClearCurrentSelections` in `ClearHighlight`, and
slot 4 `MarkSelected` at `+0x10` with seven arguments in `UpdateHighlightTextures`) is a call **through
`base+0x0C`**, i.e. the `0x00AB4410` table of §15.5.3 — which is why "the terrain view `+0x10`" in zones.md §7.1.1
is slot 4 of that table (`0x007526E0`), and "slot 9" is `0x00751ED0`.

**MATCHES (zones.md).** The zone developer's `PostCityInit` handler stores the view at `esi+0x30`, but `esi` there
is its **message-target interface at `this+8`** (`lea ecx,[esi-8]` at `0x00732A2B`, `0x00732A52`, `0x00732B73`,
`0x00732B9C`, `0x00732BB1`), so the field is `+0x38` — exactly what zones.md §7.1.1 says. The other three fields in
that handler line up the same way: `esi+0x3C`/`+0x40`/`+0x44` are `+0x44` (dirt road), `+0x48` (street) and `+0x4C`
(road), matching zones.md §7.1.1. **The rule to carry forward: each object's message-target interface sits at a
different offset** — terrain `+4`, zone developer `+8`, terrain view `+0x10` (§15.5.2) — so an offset read out of a
`DoMessage` must be rebased before it is written down as an object field.

### 15.5.7 The exact sequence the game runs

1. `SL::OnView3DInit` (`0x00602590`) — when the 3D view window comes up. From `0x4E0960(0x00B43CBC)` it fills the
   globals `0x00B43DC4` (manager; slots 3/5/4 → `0x00B43DC8`/`0x00B43DCC`/`0x00B43DD0`), then
   `0x00B43DD0` slot 8 → **`0x00B43DD8` (the 3D camera)** and slot 9 → `0x00B43DDC`
   (`0x006025D7`–`0x006025E7`).
2. City `CreateCityComponents` / `Init` builds the terrain (component 49) and the surface water; the terrain's
   `Init` subscribes to `PostCityInit` (`0x007502E5`) and the zone developer's `Init` does the same later
   (`0x00733611`).
3. On load, the terrain's class record is `Read` (u16 version 2 + a reference to the view class `0xC9B84E10`,
   §13.2).
4. `kSC4MessagePostCityInit` `0x26D31EC1` is **posted** (queued; foundations §5).
5. On the next pump, the terrain's `DoMessage` → `DoPostCityInit` (slot 40) runs §15.5.1's ten steps, ending with
   `+0xEC` = 1.
6. The zone developer's `PostCityInit` handler runs, takes `GetView` (slot 66) and `AddRef`s it.
7. From then on `RedisplayTerrain` (slot 68) posts `0x278E6003` / `0x678E6E3A`, and the view's own `DoMessage`
   (`0x00757560`) consumes them to update its dirty rects.

**For the implementer:** make the replacement's terrain create the view the same way (factory by CLSID `0xC9B84E10`
/ IID `0x6771477D`, or `new(0xE8)` + ctor), call `view->Init(terrain)` with the terrain as the single stack
argument, then set `+0xEC`. Ensure slot 66 returns `view+0x0C`. Ensure terrain `+0x6C` is a live `0x24`-byte
vertex array before anything draws. Do all of that **before** any other `PostCityInit` handler calls slot 66. That
is enough to get past the point where `enable = terrain` currently faults; the remaining view slots only matter once
drawing is exercised. **Before any of that, confirm `0x00B43CF0` (the surface water, city slot 83) and
`0x00B43CD8` are non-null** — `Init` dereferences both without a check (§15.5.4) and those are the two most likely
faults at this point.

### 15.5.8 The change messages: what the view rebuilds vs. what it reads live — TR21 contract part 2

`Init` subscribes the message target `base+0x10` to six messages (`0x0075C810`–`0x0075C868`); `Shutdown`
(`0x007560F0`) unsubscribes exactly those six in reverse (`0x007561C4`–`0x00756216`). `DoMessage`
(`FUN_00757560`, the `0x00AB4400` slot 3) dispatches on the type and sets **`view+0xB8` = 1** on every accepted
message — the byte `DrawTerrainLayers` (`0x0075BEE0`, at `0x0075BEEF`–`0x0075BEF7`) consumes to rebuild its
software caches once (`FUN_00753210`, `FUN_007539D0`) before the next frame.

| Message | Handler branch | What the view does |
|---|---|---|
| `0x277A9364` ToggleTerrainGridDisplay | `0x007575EE` | calls its **own slot 21** `SetDisplayGridFlag(!0x00B4C741)` (`0x007575F3`–`0x007575FF`); the flag itself lives in the **static `0x00B4C741`**, not the object. That slot, on a real change, calls terrain slot 68 with the whole city — so a grid toggle triggers a full heights+water redisplay (§15.5.5) |
| `0x278E6003` TerrainHeightsChanged | `0x007575C3` | `FUN_007523F0` unpacks the rect from `data1`/`data2` (the same `(x0<<16)+x1` packing §8.1 posts: `x0 = data1>>16`, `x1 = data1&0xFFFF`, `z0 = data2>>16`, `z1 = data2&0xFFFF`, `0x007523FA`–`0x00752424`), then `FUN_007569F0` **copies the terrain's altitudes into the view's software vertex buffer** row by row — it reads the terrain's `+0x6C` array directly (`0x007569F3`–`0x007569F9`) and terrain slot 29 `GetSeaLevel` (`0x00756A00`), and refreshes the city-edge layer heights through `FUN_00755B60` when the rect touches a border row or column (`0x00756A61`–`0x00756ABF`, the rule is TR30). Live vertex data is **not** re-read at draw time; the copy is the draw input |
| `0x678E6E3A` WaterHeightsChanged | `0x00757598` | same unpack, then `FUN_00756B00`: reads terrain slot 29 (`0x00756B23`), walks the rect reading the **water array through `view+0x24` → water `+0x18`** (`0x00756B10`–`0x00756B14`, record `+0x00` depth, terrain record `+0x00` altitude), and refills the **water** vertex buffer `0x00B4C75C`: the surface level written is `depth > 0 ? altitude + depth : sea level` (`0x00756B5A`–`0x00756B7C`), plus the per-vertex colour/tile bytes derived from a level lookup (`0x00756BF0`–`0x00756C5A`). Then `FUN_00756000` merges the unpacked rect into the water-colour update rect at `view+0x4C` (replace when `view+0x48` is set, else union — `0x00756029`–`0x0075606A`) |
| `0xA6B79601` ViewZRChange2 | `0x0075757D`→`0x007575DE` | sets `view+0xB8` = 1 only |
| `0xA6B79611` ViewTChange2 | `0x0075760C` | sets `view+0xB8` = 1 only |
| `0xC9DA96EA` (unnamed in `re/class_id_table.tsv`) | `0x00757613` | calls **its own slot 13** `SetLightingUpdateRect(null)` with a null rect (`0x0075761A`–`0x00757622`), then `FUN_00756000(null)` (which takes the whole city), then stores the message's `GetData2()` (`[msg+0x38]`, `0x0075762F`–`0x00757633`) into `view+0x44` |

**The rule for a replacement:** the view is a **pull-through cache**, not a live reader. Terrain edits reach it
only through `0x278E6003`/`0x678E6E3A` (the `RedisplayTerrain` posts, §8.1) — there is no per-frame read of the
terrain's altitude array in `Update`/`DrawTerrainLayers`' steady state. The two exceptions, both on the *draw*
side, are (a) `DrawTerrainLayers` re-reads terrain slot 42 each frame for the terrain-type switch
(`0x0075BF10`–`0x0075BF3C`) and (b) `Update` re-reads slots 21/22 each frame for the draw band. Everything else
(also the water array through `view+0x24`) is snapshot into the software buffers by the two handlers above.

**What the view builds once, at `Init`** (not per message): the index buffers. `FUN_00758750` (`0x00759665` from
`FUN_007595A0`) frees and reallocates the statics, and `FUN_00755FB0` (`0x0075C73D`–`0x0075C73F`) then rebuilds
**every cell's 6 triangle indices** through `SetTriangleIndicesForCell` (`0x00752AF0`) over the whole grid;
`FUN_00755ED0(0, cellsZ−1)` (`0x0075C734`–`0x0075C738`) marks **all rows dirty** in the row bit-vector
`0x00B4C7EC/0x00B4C7F0`. After that, per-cell index changes happen only through the view's own slot 16/17 and the
terrain's triangulation/hide slots, which each call `SetTriangleIndicesForCell` for exactly the touched cell
(`0x007411E1`, `0x00741251`, `0x0074187D`, `0x007418DD`, `0x0074193D`, `0x0074208C` — the terrain calls the
**view's** index builder directly, not through a slot table).

### 15.5.9 The draw-time statics and buffers *(shared per process; verified in the bytes)*

`StaticInit` (`0x0075C0C0`, guarded by the byte `0x00B4C740`) and `FUN_007595A0` build the per-process state every
view instance shares. A replacement may keep them as ordinary members, but the sizes and producers are the
contract:

| Static | Content | Producer |
|---|---|---|
| `0x00B4C744`/`0x00B4C748` | cellsX, cellsZ | `FUN_007595A0` from terrain slots 8/7 (`0x0075C4B5`–`0x0075C4C1`) |
| `0x00B4C74C`/`0x00B4C750` | vertsX = cellsX+1, vertsZ | `FUN_007595A0` `0x007595BD`–`0x007595C9` |
| `0x00B4C754` | vertex count | `0x007595E8` |
| `0x00B4C734`/`0x00B4C730` | sizeX, sizeZ in metres | `0x007595D9`–`0x0075961F` |
| `0x00B4C740` | StaticInit-done flag | `0x0075C3EB` |
| `0x00B4C741` | the grid-display flag (read by slot 20, written by slot 21) | `Init` from `*(byte*)(0x00B43CD8+1)` (`0x0075C453`–`0x0075C460`); `Shutdown` writes it back (`0x00756323`–`0x0075632C`) |
| `0x00B4C758` | terrain **vertex draw buffer**: `vertexCount × 0x20` bytes | `FUN_00758EA0` → `FUN_00752430` (x at +0x00, z at +0x08, colour bytes +0x0C..+0x0F) |
| `0x00B4C75C` | **water** vertex draw buffer: `vertexCount × 0x20` bytes (level at +0x04, colour bytes +0x0C..+0x0F) | `FUN_00752520`/`FUN_00756B00` |
| `0x00B4C700` | per-row index count in u16s (`cellsX × 6`) | `FUN_00758EA0` `0x00758EF1`–`0x00758F03` |
| `0x00B4C6F4` | the terrain **index buffer** (`cellsZ × 0xB4C700 × 2` bytes; 6 u16 per cell) | `FUN_00758EA0` `0x007590BB`–`0x007590CC` |
| `0x00B4C6F8` | the cliff/second index buffer (same size) | `0x00758F20`–`0x00758F3A` |
| `0x00B4C6B8` | u16 level-depth map (`vertsX × cellsZ`) | `0x007590D1`–`0x007590E6`; `LevelCell`/`RemoveCellLevelling` maintain it |
| `0x00B4C6B0`/`0x00B4C6AC` | per-cell-row vectors of `cCellQuad` (`0x80` bytes) and `cLevelCellInfo` (`8` bytes) | `0x007590EB`–`0x00759174` |
| `0x00B4C6D0` | per-row "draw this row" flag bytes (`vertsZ`) | `0x007594E6` region |
| `0x00B4C6D4`/`0x00B4C6D8` | per-row first/last visible column (int per row) | `ComputeDrawRectsForDrawFrustum` `0x00752E80` |
| `0x00B4C6CC` | per-row pair flags for the water pass | `0x0075950E` region |
| `0x00B4C760`, `0x00B4C790` | the three city-edge layer-height arrays (see `InitializeCityEdgeLayerHeights` `0x00757300`) | `StaticInit` `0x0075965A`–`0x0075966A`, `0x0075965F`–`0x0075966C` |
| `0x00B4C7A0`/`0x00B4C7A4`/`0x00B4C7A8` | the colour/level LUT object and its level range (from the functor pair `0xC9EFB727`/`0x69EFB6F8`, queried through `0x00B4C738`) | `StaticInit` `0x0075C2AC`–`0x0075C336` |
| `0x00B4C704` | the texture-binding factory (`cS3DTextureBindingFactory`, class `0x1AC0E11A`, IID `0xFAC0E219`) | `StaticInit` `0x0075C160`–`0x0075C18C`; released in `FUN_00758BC0` `0x00758CC9`–`0x00758CCE` |
| `0x00B4C708`/`0x00B4C70C` | the current texture set pair (Init sets 4/5, `0x0075C4FD`–`0x0075C507`) | `FUN_007566C0`/`FUN_007574D0` on a draw-context change |
| `0x00B4C670`..`0x00B4C680` | five texture-binding tables, one per texture-detail **level** `0x00B4C70C` (not per terrain type; corrected by TR40, §15.5.12.2), each `0xB4BC44` entries, zeroed here and filled per terrain type by `FUN_007571D0`; `0x00B4C684`..`0x00B4C698` are separate bindings loaded here from ids `0x09187400..0x09187404` | `StaticInit` `0x0075C240`–`0x0075C281` |
| `0x00B4C7B4`, `0x00B4C7B8` | two more bindings (grey ramp `0x0912220F`; water `0x69EFB7DA`) | `StaticInit` `0x0075C336`–`0x0075C348`, `FUN_0075253A` `0x00752638`–`0x0075264B` |
| `0x00B4C7BC`/`0x00B4C800` | the packed render state + its built flag | `Update` `0x0075C025`–`0x0075C054` |
| `0x00B0D6FC` | the last-seen terrain type (drives `ChangeTerrainType`) | `Init`/`DrawTerrainLayers` |
| `0x00B4BC44`/`0x00B4BC48` | `0x00B4BC44` = the terrain-texture table length (highest texture-info instance id + 1, §15.5.12.2; not a terrain-type count); `0x00B4BC48` = five per-curve maxima (written by `cSTETerrain::ReadTextureInfos` `0x0074F410` / `ReadTextureInfo` `0x0074F180`, only inside `cSTETerrain::StaticInit` `0x0074FE40`) | read-only here |
| `0x00B0D708..0x00B0D719`, `0x00B4C808`, `0x00B4BE00..0x00B4BE08`, `0x00B4C618..0x00B4C620` | the view's own ini (`0x6534284A/0x88CD66E9/1`) properties `0x89A312C0`, `0x29AB15C0`–`C7` (water colours, the render toggles `0x00B0D718`/`0x00B0D719`, the edge-layer heights source) | `ReadTuningParameters` `0x00758990`–`0x00758B09` |

**`cSTEVertBufExtn`** (`0x0075C9F0` create, `0x0075CA90` release, `0x0075CB50` draw, `0x0075CAD0` flush) is the
renderer-side vertex cache. `Update` asks for it once per frame (`0x0075C00C` → `GetVertexBufferExtension`
`0x0075CD40`): it queries the object at `drawCtx+0x30` for IID `0x09CD86F9` (`0x0075CD70`), **gated on the game
preference byte at `[[0x00B43CC4]+0xC]+0x33C`** (`0x0075CD48`–`0x0075CD53`) — with the preference off, or the
query failing, the extension stays null and every draw goes through the plain `SetVertexBuffer` path. A
replacement host can therefore leave the extension unsupported; nothing faults.

### 15.5.10 The remaining slots, with behaviour *(TR21 contract part 3)*

`0x00AB4410` at `+0x0C`, slots not covered above. `ecx` = `base+0x0C` throughout (§15.5.2); "area" is
`[ecx+0x60]` = `base+0x6C`, the `cSTESelectedTerrainArea`.

| Slot | Target | Behaviour |
|---|---|---|
| 0 `QueryInterface` | `0x00756910` | thunk (`sub ecx,0xC`) to `0x00752730`. Answers IID `0x6771477D` → `base+0x0C`; `0x452294AA` (`cIGZMessageTarget2`) → `base+0x10`; `0xE4FDA3D4` (`cIGZSerializable`) → `base+0x14`; CLSID `0xC9B84E10` and the unknown extra `0xC989F960` → `base+0x00`; everything else falls through to `cRZUnknown::QueryInterface` `0x009457C6` (`0x00752730`–`0x007527C9`). *[quirk]* **`0xC989F960` answers the object base** — it is answered nowhere else in the image (`cSC43DRender::DoMessage` `0x007CB1C1` asks for it and stores the result at `+0x194`) |
| 3 `Pick(x, y, z, dir, out)` | `0x0075A230` | pop 20, returns **AL**. Ray/box test against the world box (x/z `0..sizeX/sizeZ`, y `−32768..32768`; `0x0075A2A0`–`0x0075A2C6`), converts the hit to a cell, walks the cell's two triangles against the **terrain vertex array read directly** (`+0x6C`, `0x0075A35C`–`0x0075A35F`), applying a per-vertex offset from the water array through `view+0x24` when arg5 says so (`0x0075A5A8`–`0x0075A61D`), and on a hit pushes the picked point and calls **area slot 8** `MarkSelected(point, 2, 1)` (`0x0075A71D`–`0x0075A72E`). Returns 0 when nothing hit |
| 4 `MarkSelected(SC4Point<int>*, key3*, u8 rot, u8 flip, u32 colour, u8 op, u8 clear)` | `0x007526E0` | pop 28, **AL**. Null-guarded forward to `cSTESelectedTerrainArea::MarkSelected` `0x00751A70` (the textured **cell** mark; zones.md §7.1.1 step 2.3) |
| 5 `MarkSelected(rect*, key3*, u8 rot, u8 flip, u32 colour, u8 op, u8 clear)` | `0x007526C0` | pop 28, **AL**. Forward to `0x007519A0`: the textured **rectangle** mark — walks the rect's cells into `AddTexturedSelection` with op forced to **2** and alpha `1.0` (`0x00751A52`–`0x00751A62`) |
| 6 `MarkSelected(SC4Point<int>*, u8 op, u8 clear)` | `0x007526A0` | pop 12, **AL**. Forward to `0x00751910`: one **untextured** point (`AddUntexturedSelection` `0x007515E0`, `0x0075197C`–`0x00751987`) |
| 7 `MarkSelected(pointVector*, u8 op, u8 clear)` | `0x00752680` | pop 12, **AL**. Forward to `FUN_00751780`: marks **every point of a vector** untextured (`0x00751820`–`0x007518E7`) |
| 8 `MarkSelected(SC4Point<int>* a, SC4Point<int>* b, u8 op, u8 clear)` | `0x00752660` | pop 12, **AL**. Forward to `0x007516D0`: marks the **span between two points**, row-walk (`0x00751730`–`0x00751767`) |
| 9 `ClearCurrentSelections` | `0x00751ED0` | pop 0, void. `jmp` to `cSTESelectedTerrainArea::ClearCurrentSelections` `0x00750CA0` when the area is non-null (`0x00751ED0`–`0x00751EDC`) |
| 10 `SetAutomaticCellCursorUpdateStatus(bool)` | `0x00752860` | pop 4, void. Stores the byte at `view+0x24` and returns the **old** value in AL (`0x00752860`–`0x0075286A`) *[quirk: a setter that returns the previous value]* |
| 11 `IsWaterGlareEnabled` | `0x007568E0` | pop 0, **AL** = byte `view+0x49` (ctor 1) |
| 12 `SetWaterGlareStatus(bool)` | `0x00756080` | pop 4, void. No-op when equal to `view+0x49`. On a change: stores it, then — when the previous flag `view+0x3C` already equals the new one — **unions** the whole-city rect into the water-colour rect at `view+0x40` via `SetWaterColorUpdateRect` (`0x007560D5`–`0x007560DE`); otherwise stores the flag at `view+0x3C` and **replaces** `view+0x40` with `(0, 0, cellsX, cellsZ)` (`0x007560BB`–`0x007560C9`) |
| 13 `SetLightingUpdateRect(rect* or null)` | `0x00755F30` | pop 4, void. Null → the whole city `(0, 0, cellsX, cellsZ)`. When the pending flag `view+0x25` is **set**: clears it and **replaces** the lighting rect at `view+0x28` (`0x00755F72`–`0x00755F82`); when clear: **unions** the new rect into `view+0x28` via `SetWaterColorUpdateRect` (`0x00755F8C`–`0x00755F9F`) |
| 14 `DrawTerrainQuad(SC4DrawContext*, u32 cellX, u32 cellZ)` | `0x00752870` | pop 12, void. Draws one cell from the software buffers: guards the cell against the frustum rows (`view+0x8C/+0x90`, `0x00B4C6D4/D8`, the row flags `0x00B4C6D0`, `0x00752877`–`0x007528C6`), `SetVertexBuffer(0xB, vertsX×2, 0x00B4C758 + cellZ×vertsX×0x20)`, picks the index buffer `0x00B4C6F8` when the cell's vertex flags (`+0x23` of the terrain record, read **directly** at `0x007528FD`–`0x00752901`) have `0x30` set else `0x00B4C6FC`, then `DrawPrimsIndexed(0, cellX×6, 6)` (`0x007528E0`–`0x00752934`) |
| 15 `DrawTexturedTerrainQuad(ctx, u32 cellX, u32 cellZ, key3*, float level, bool ignoreFrustum)` | `0x00752940` | pop 24, void. Same guards unless arg6 (`0x00752954`); reads the **terrain record `+0x23` flags directly** (`0x007529C8`–`0x007529CC`) to pick the flipped index buffer `0x00B4C6DC`/`0x00B4C6E8`; builds a 4-vertex patch from the software buffer (`0x00B4C758 + idx×0x20`, `0x007529E4`–`0x00752A31`), stamps the level byte `trunc(arg5)` into all four vertices' +0xF (`0x00752A82`–`0x00752AB0`), `SetVertexBuffer(0xB, 4, local)`, `DrawPrimsIndexed(0, 0, 6)` |
| 16 `LevelCell(u32 cellX, u32 cellZ, float level)` | `0x00759750` | pop 12, per `vtables.tsv` returns **ST0** (the body posts no value; treat as undefined *[compat]*). Guards the cell against cellsX/vertsZ. Maintains the level map `0x00B4C6B8` (`0x007597A1`–`0x007597CD`), the quad/level vectors, writes `level` into the level-info record (`0x0075982A`–`0x0075983B`), refreshes two neighbour layers (`FUN_00756350`/`FUN_007564F0`), then **writes the terrain's vertex flags directly**: `or byte [terrainArray + idx×4 + 0x22], 0x60` (`0x0075986D`–`0x00759873` — the §7 hide bits `0x20|0x40`) and rebuilds the cell's indices (`SetTriangleIndicesForCell`, `0x00759880`) |
| 17 `RemoveCellLevelling(u32 cellX, u32 cellZ)` | `0x00758620` | pop 8, void. The inverse: decrements the level map, erases the quad and level-info records, `and byte [terrainArray + idx×4 + 0x22], 0x9F` (`0x007586D1`–`0x007586DA`), rebuilds the indices (`0x007586E8`) |
| 18 `GetOverlayManager(u32 index)` | `0x007558A0` | pop 4, **EAX** = `[ecx + index*4 + 0x6C]` = `base+0x78+index*4` — **no bounds check** (`0x007558A0`–`0x007558A8`) |
| 19 `HandleCheatCommand(scriptCall*)` | `0x0075A770` | pop 4, **AL**. Recognises two command lines: `"level"` with 3 arguments → parses two ints and a float and calls **its own slot 16** `LevelCell(x, z, amount)` (`0x0075A7D1`–`0x0075A845`); `"unlevel"` with 2 arguments → **slot 17** `RemoveCellLevelling(x, z)` (`0x0075A855`–`0x0075A8A5`). Returns 1 when the command matched, 0 otherwise |
| 20 `GetDisplayGridFlag` | `0x00752AE0` | pop 0, **AL** = the **static** `0x00B4C741` — not a field (`0x00752AE0`–`0x00752AE5`) |
| 21 `SetDisplayGridFlag(bool)` | `0x00755E80` | pop 4, void. No-op when equal to the static; on a change stores it and calls **terrain slot 68** `RedisplayTerrain(1, 1, &wholeCity, 0)` through the terrain held at `base+0x20` (`0x00755E91`–`0x00755EC8`) |

`0x00AB4400` at `+0x10` — the **message target**, 8 slots: `0x00756920`, `0x00756930`, `0x00756940` are
`sub ecx,0x10` thunks to QueryInterface/AddRef/Release (`0x00756920`–`0x00756947`); **slot 3 `+0x0C` = `DoMessage`
`FUN_00757560`** (pop 4, returns **AL** = 1); slots 4–7 repeat `0x00AB4410` slots 0–3.

`0x00AB43E8` at `+0x14` — 6 slots (`ecx` = `base+0x14`): `0x00756950`/`0x00756960`/`0x00756970` are `sub
ecx,0x14` thunks to QueryInterface/AddRef/Release; slot 3 `0x00752CD0` (pop 4, **AL**) is a **load** helper: it
asks the stream (arg1) slot 18 (`+0x48`) whether it can read, and on a fresh stream writes the static
`0x00B4C741` with `operator<<` (`0x00752CE0`–`0x00752CF7`); slot 4 `0x007527D0` (pop 4, **AL**) is the matching
**save** helper: it marks the stream written (`this+5` = 1, `0x007527D5`), and when the stream says the target
version is 1 (`[esp+8] == 1`, `0x007527F0`–`0x007527F6`) reads the flag back with `operator>>` into the static
(`0x007527FE`–`0x00752809`); slot 5 `0x007523E0` returns the **CLSID** `0xC9B84E10` in EAX
(`0x007523E0`–`0x007523E5`). So the saved "one byte" of §13.2 is exactly the grid-display flag.

### 15.5.11 The contract: what a replacement `cSTETerrain` must do for the view

Everything above, as the list an implementer checks against. `V` = the view object (`0xE8` bytes), `T` = the
replacement terrain, `W` = the replacement surface water.

**Creation and identity**

1. `T` holds `V`'s **object base** at terrain `+0xE8`, set by `T`'s own `DoPostCityInit` (§15.5.1 steps 3–6), and
   `GetView` (slot 66) returns `*(T+0xE8) + 0x0C` — never the base, never a fresh object.
2. `V`'s vtables are the four of §15.5.3 (`0x00AB4480` at +0x00, `0x00AB4468` at +0x04, `0x00AB4410` at +0x0C,
   `0x00AB4400` at +0x10, `0x00AB43E8` at +0x14) with the slot/behaviour table of §15.5.10; QueryInterface answers
   `0x6771477D`, `0x452294AA`, `0xE4FDA3D4`, CLSID `0xC9B84E10`, and the extra `0xC989F960` per the slot-0 row.
3. `V->Init(T)` (`__thiscall`, one stack argument, `ret 4`) must return **0** when already initialised and 1
   otherwise, and must not run twice (the `+0x18` gate).

**What `V` reads from `T` through slots** — every one must work before the corresponding stage:

4. At `Init`: slots 1, 7, 8, 42 (§15.5.4).
5. At every `Update`: slots 21 and 22, returning **x87 floats in `st0`** (§15.5.5). They may return the cached
   `T+0x70`/`T+0x74` band; nothing in the view requires a fresh scan.
6. From the two change-message handlers: slot 29 `GetSeaLevel` (`st0`).
7. From `DrawTerrainLayers` (each frame): slot 42 `GetTerrainType`.
8. From `SetDisplayGridFlag`: slot 68 `RedisplayTerrain` with `(1, 1, rect, 0)` and the §8.1 posting rules — the
   view's grid toggle is broken without it.
9. From `DrawWaterLayers`: slot 83 `GetWaterSurfaceDisplayState` in **AL**.
10. From `DrawPlumbingInfo`: slots 76 and 70 in **AL**, per cell (quirk 10).
11. From `DrawTerrainLayers`' first pass: slot 67 with a pointer to `V+0x34` as its argument
    (`0x0075BF70`–`0x0075BF74`), gated once by `V+0x31` — the terrain-side lighting hook; `DrawTerrainLayers`
    then calls `FUN_007558B0` (a 663-byte software-state reset guarded by the static flag `0x00B4C848`) and sets
    `V+0x31` = 1, so the hook runs exactly once per session (`0x0075BF7A`–`0x0075BF81`).

**What `V` reads from `T` directly** *[compat]* — the field layout of §2.3 is load-bearing:

12. `T+0x6C` is a live array of `0x24`-byte vertex records, indexed `x + z × vertsX` with the **view's own cached
    `vertsX`** (`0x00B4C74C`, = cellsX+1 from slot 7), before anything draws. Readers: `SetTriangleIndicesForCell`
    (`+0x22` flags), `Pick` (`+0x00` altitudes), `FUN_007569F0` (the `0x278E6003` copy), the terrain-surface
    texture id at record `+0x16` (`ComputeTextureSpansForRow` `0x0075AC5D`, `DrawTerrainSurfaceRows`
    `0x0075B5F2`).
13. `T+0x6C` is **writable**: slot 16 `LevelCell` ORs `0x60` into record `+0x22` and slot 17 clears it with
    `0x9F` (§15.5.10) — a replacement that recomputes `+0x22` from scratch will fight the view (same rule as
    §2.3).
14. `W+0x18` is the live `0x24`-byte water array (depth at `+0x00`): `Pick` and the `0x678E6E3A` handler walk it
    through `V+0x24`. A replacement water object must expose the array there even if its own slots never do.

**Messages**

15. `V`'s message target (`base+0x10`) is subscribed to the six messages of §15.5.8 by `Init` and unsubscribed by
    `Shutdown`; the terrain's `RedisplayTerrain` posts (`0x278E6003`, `0x678E6E3A`, `0x870BC918`, §8.1) are the
    only channel that moves terrain edits into the view's draw buffers. A replacement terrain must keep posting
    them with the §8.1 packing, and a replacement view must unpack `(x0<<16)+x1` / `(z0<<16)+z1` the same way.
15a. **Who calls the view's `Shutdown`** (TR35): the terrain's own `Shutdown` (slot 4, `0x007435C0`, gated by the
    terrain init byte `+0x0C`, which it clears). After releasing `+0xF4` and `+0xF0` it does, when `+0xE8` is
    non-null: `cSTETerrainView3D::Shutdown` (`0x007560F0`, direct `__thiscall` with `ECX` = the view **base**,
    no arguments, `0x007437BE`), then `Release` through the view base vtable slot 2 (`+0x08`), then `+0xE8 := 0`
    and the view-ready byte `+0xEC := 0` (`0x007437B4`–`0x007437D4`). The view's destructor (`FUN_00756980`)
    also calls `Shutdown` itself when its `+0x18` initialised byte is still set (`0x007569AE`–`0x007569B0`), so
    a final `Release` alone is safe but unsubscribes only when the last reference goes. A replacement terrain
    does the same three steps in its slot-4 `Shutdown`.
16. The view sets its own `+0xB8` dirty byte on every accepted message; `DrawTerrainLayers` clears it after one
    rebuild. Anything else the view caches is rebuilt only through that path.

**Globals `Init` dereferences unchecked** (§15.5.4): a replacement host must have `0x00B43CD8` (preferences) and
`0x00B43CF0` (the surface water, from city slot 83) non-null before calling `V->Init`. `0x00B43CEC` (city),
`0x00B43CCC` (message server) and `0x00B43DD0` (draw registry) are null-checked and safe to omit in a headless
host; with `0x00B43DD0` null the draw-callback registrations of §15.5.4 step 12 are skipped entirely.

**Ordering**

17. The whole chain — view created, `Init`ed, `+0xEC` set, slot 66 answerable — must complete before any
    `PostCityInit` consumer calls terrain slot 66 (§15.5.1); the zone developer faults otherwise.
18. **The terrain's per-process static init must have run before the first `V->Init`** (§15.5.12.2, TR40):
    `cSTETerrain::StaticInit` (`0x0074FE40`) fills the texture-id count `0x00B4BC44` and the per-terrain-type
    texture-id lists `0x00B4BCD8[type]` that the view's own `StaticInit` and `FUN_007571D0` size and fill their
    texture tables from.
19. **Every `0x278E6003` / `0x678E6E3A` rectangle must lie inside `[0, cellsX] × [0, cellsZ]` with `x0 ≤ x1`,
    `z0 ≤ z1`, as delivered** (§15.5.12.1, TR38): the view clamps nothing.

### 15.5.12 Two replacement crashes, explained (TR38, TR40) *[compat]*

Both faults seen with a replacement terrain (glue-terrain3 runs gt3-repl-4, gt3-repl-5) are explained below from
the instruction bytes. Neither is caused by missing "edge-layer state": the view sets its edge-layer state up
itself (§15.5.12.3), and the original game reaches the same code with the same view state.

#### 15.5.12.1 The heights-message copy `FUN_007569F0` and the edge-layer refresh `FUN_00755B60` (TR38)

`FUN_007569F0` (`__thiscall` on view `base+0x10`'s owner pointer `ECX = base`, one stack argument = pointer to an
inclusive vertex rectangle `{x0, z0, x1, z1}` as four int32, `ret 4`) is the whole body of the `0x278E6003`
handler (`0x007575C3`–`0x007575D9`; the rectangle is unpacked by `FUN_007523F0`: `x0 = data1 >> 16`,
`x1 = data1 & 0xFFFF`, `z0 = data2 >> 16`, `z1 = data2 & 0xFFFF`, data1 = message slot 9 = field `+0x10`, data2 =
slot 14 = field `+0x14` of the `cRZMessage2Standard`). `Init` runs the same function once itself on
`{0, 0, cellsX, cellsZ}` (`0x0075C6EC`–`0x0075C70E`), followed by the water copy `FUN_00756B00` on the same
rectangle, so the view already holds a whole-city copy right after `Init`.

What it does, in order (`0x007569F0`–`0x00756AF5`):

1. `T = V+0x20`; `recs = T+0x6C`; `sea = T->slot 29()` (x87 float in `st0`, stored as float).
2. For `z = z0 … z1` (outer, inclusive), `x = x0 … x1` (inner, inclusive), with `i = z × vertsX + x`
   (`vertsX` = the static `0x00B4C74C`): copy the dword at `recs[i] + 0x00` (altitude) to the vertex draw buffer
   `0x00B4C758 + i × 0x20 + 4`; then refresh the city-edge layers for each edge the vertex lies on:

   | condition | side | edge index passed |
   |---|---|---|
   | `z == cellsZ` (`0x00B4C748`) | 0 | `x` |
   | `x == cellsX` (`0x00B4C744`) | 1 | `cellsZ − z` |
   | `z == 0` | 2 | `cellsX − x` |
   | `x == 0` | 3 | `z` |

   The re-read of `cellsZ`/`cellsX` after each call is only register reuse; corners hit two sides.
3. Neither `x` nor `z` is clamped against the grid anywhere: the loop bound is the rectangle as delivered.

`FUN_00755B60(side, k, alt, sea)` (`__cdecl`, four stack arguments, the caller pops 16; floats passed as raw
dwords) writes, with `n = vertsX` for sides 0 and 2 and `n = vertsZ` (`0x00B4C750`) for sides 1 and 3, and the
three 24-byte-element arrays `L0 = 0x00B4C760[3·side]`, `L1 = …[3·side+1]`, `L2 = …[3·side+2]` (each `2n`
elements) plus the 16-byte-element array `E = 0x00B4C790[side]` (`2n` elements):

- `E[n + k] + 4` := the smaller of `alt` and `sea` (as a dword copy of the chosen argument);
- `L0[k] + 4` := `alt`; `a1 = max(alt − D0[k], c)` with `D0` = the float array `0x00B4BE10` and `c` = the float
  at `0x00A81054`; `L0[n + k] + 4` := `a1`; `L1[k] + 4` := `a1`;
- `a2 = max(a1 − D1[k], c)` with `D1` = the float array `0x00B4C214`; `L1[n + k] + 4` := `a2`;
  `L2[k] + 4` := `a2`; `L2[k] + 0x14` := `a2 × s` with `s` = the float `0x00B4BE08`.

(`max` here is the x87 `fcom`/`test ah,5`/`jp` pattern: the constant replaces the value only when the value is
smaller; for `E`, a tie keeps `alt`.) So the arrays are indexed by `k` and `n + k` only, and `k` must be in `[0, n − 1]`; the sizes
are fixed at `Init` (§15.5.12.3).

**The gt3-repl-4 fault.** At `0x00755C2B` (the `L2[k] + 4` store) the stacked arguments were `side = 2`,
`k = 0xFFFFBF7E` (−16514), `alt = 0.0`, `sea = 250.0`; the caller's restored registers gave `x = EDI = 0x4102`
(16642) and `z = 0`, and `ESI − EDX` gives `n = 129`, so `cellsX = 128` and the view's statics were correct. The
inner loop had run `x` from `x0` up to 16642 on row 0, which needs the delivered `x1 = data1 & 0xFFFF ≥ 0x4102`:
the message the view received did **not** carry `data1 = (0 << 16) + 128`. Before faulting the loop also wrote
past the vertex draw buffer (`i` up to 16642 ≥ 129 × 129) and below the edge arrays, so heap damage precedes the
fault. The whole-city redisplay itself is not the problem: the original posts the same rectangle (§8.1, from
`SetSeaLevel`, the grid toggle) and `Init` copies it unharmed.

**Rule for a replacement terrain** (what the original's helper `0x00743380` does for each post): allocate a
**fresh** `cRZMessage2Standard` (`operator new(0x2C)` at `0x0074338E`, constructor `0x009134D6`), `AddRef`
(slot 1), `SetType` (slot 5, `+0x14`), `SetData1` (slot 11, `+0x2C`), `SetData2` (slot 16, `+0x40`), data3
(slot 21, `+0x54`), data4 (slot 26, `+0x68`), `MessagePost(msg, 0)` on the message server `0x00B43CCC`
(slot 4, `+0x10`; queued), then `Release` (slot 2, tail jump). The data fields are read only when the queue is
pumped, so a message object must never be reused or rewritten after it is posted, and data1/data2 must be the
§8.1 packing of a rectangle inside the grid. A replacement that checks its rectangle before posting and gets a
fresh object per post cannot reach this fault.

#### 15.5.12.2 The side-cliff texture table read in `DrawSideCliffs` (TR40)

`DrawTerrainLayers` (`0x0075BEE0`) calls, after the slot-67 hook, `DrawSideCliffs` (`0x00753E00`, `__thiscall`,
one argument = the draw context). For each of the four sides it draws three layers whose texture ids are the
constants `0x11`, `0x10`, `0x15` (stored at `0x00753E1E`–`0x00753E2E`), and binds each layer's texture as
`*(*(0x00B4C670[level] + id × 4))`, `level` = the static `0x00B4C70C` (`0x00754092`–`0x007540A7`). The gt3-repl-5
fault (`0x007540A5`, reading `0x8`): `EAX = level = 1`, `ECX = 0x00B4C670[1]` a valid heap pointer, and the table
entry for id `0x11` held `8` — not a texture binding and not 0.

The five tables `0x00B4C670[0..4]` are **per texture-detail level**, not per terrain type (correcting the
§15.5.9 row): `level` comes from the draw context `+0x14` (`FUN_007574D0`, `0x0075BF3F`–`0x0075BF4D`; `Init`
sets the sentinel 5 so the first draw reloads) *[decomp only for the sentinel]*. The view's `StaticInit` allocates each
table as `0x00B4BC44 × 4` bytes and zeroes it; `FUN_007571D0(level)` then loads, for every texture id in the
sorted list `0x00B4BCD8[terrainType]` (sentinel-headed doubly linked list, node = `{next, prev, id}`,
`terrainType` = `0x00B0D6FC` from slot 42), the binding for group `0x7AB50E44`/`0x891B0E1A`, instance
`id + level × 0x100` (falling back to instance `id`) into `table[level][id]`.

Both inputs are **written only by the original terrain**:

- `0x00B4BC44` — `cSTETerrain::ReadTextureInfos` (`0x0074F410`) sets it to 0, sweeps the exemplars of type
  `0x6534284A` group `0x89C2A517`, raises it to each record's instance id (`ReadTextureInfo` `0x0074F180`), then
  adds 1: the table length = highest terrain-texture id + 1;
- `0x00B4BCD8[type]` — filled (ids inserted in ascending order, no duplicates, including the fixed id `0x13`)
  by `cSTETerrain::ReadMiscTexturesForTerrainTypes` (`0x0074C800`), called from `ReadTerrainIniFile`
  (`0x0074DC30`).

Both run only inside **`cSTETerrain::StaticInit` (`0x0074FE40`)**: guarded once per process by the byte
`0x00B4BC3C`; registers `cSTETerrain::StaticShutdown` (`0x0074FC20`) through `0x00B43C94` slot 69 (`+0x114`);
stores `0x00B43CA8` (AddRef'd) into `0x00B4BC40`; then calls `ReadTextureInfos`, `ReadTerrainIniFile`,
`ReadTuningParameters` (`0x0074B6A0`), `ReadBrushDescriptions` (`0x0074FDC0`); returns 1 in AL. Its only caller
is `cSTETerrain::Init` (`0x0074FEAF`), i.e. at city init, before `DoPostCityInit` creates the view.

With a replacement terrain that never runs it, `0x00B4BC44` stays 0: the view's `StaticInit` allocates
zero-length tables, `FUN_007571D0` walks empty lists, and `DrawSideCliffs` reads past the allocation — the
gt3-repl-5 fault. **Rule:** before the first `V->Init`, `0x00B4BC44` and `0x00B4BCD8[0..4]` must hold what
`cSTETerrain::StaticInit` puts there. A replacement either calls `0x0074FE40` (no `this`, no arguments, idempotent)
from its own `Init`, as the original does, or reproduces those two statics from the same exemplars and ini file —
the view's `StaticInit` runs **once per process**, so they must be right at the first city's view `Init`.
The slot-67 hook is not involved in this fault (§15.5.12.4).

#### 15.5.12.3 Who sets up the view's edge-layer state

All of it belongs to the view; nothing comes from the terrain except `cellsX`/`cellsZ` (slots 8/7) and the
records read in §15.5.12.1:

- the per-edge-index layer depths `0x00B4BE10` / `0x00B4C214` (`D0`, `D1`) — `InitializeCityEdgeLayerHeights`
  (`0x00757300`), called only from the view's `ReadTuningParameters` (`0x00758AD3`), itself called once per
  process from the view's `StaticInit` (`0x0075C157`); its inputs are the tuning object `0x00B4C738`;
- the scale `0x00B4BE08` (`0x3C800000` = 1/64) — set by `FUN_007558B0`, the once-per-process reset that
  `DrawTerrainLayers` runs right after the slot-67 hook (0 before the first draw; it only scales `L2[k] + 0x14`)
  *[decomp only]*;
- the arrays `0x00B4C760[12]` and `0x00B4C790[4]` — allocated by `FUN_00758EA0` (sizes `n × 0x30` and `n × 0x20`
  per side, `n` as above), called from `FUN_007595A0(cellsX, cellsZ)` at `Init` `0x0075C4C1`; that helper returns
  early when the size is unchanged, so they are reallocated only on a size change and freed by the view's static
  shutdown `FUN_00758BC0`;
- their contents — the whole-city `FUN_007569F0` at `Init` and every later `0x278E6003`.

#### 15.5.12.4 Slot 67 `DoLighting` (`0x00742930`)

`__thiscall`, one argument (a pointer to an inclusive vertex rectangle `{x0, z0, x1, z1}`), `ret 4`. For each row
`z = z0 … z1` it calls slot 28 (`+0x70`) of the global lighting object `0x00B43DDC` with seven stack arguments
`(z, x0, x1 − x0 + 1, &rec(x0, z) + 0x04, 0x24, &rec(x0, z) + 0x10, 0x24)`, `rec(x, z) = T+0x6C + (z × T+0x38 + x)
× 0x24`: it lights a row of vertex normals (record `+0x04`) into record `+0x10`. It touches no view static. The
view calls it once per session with `V+0x34`; what the view stores in those 16 bytes is not settled here (§15.5.2
lists `+0x34` as zero at construction and `+0x40` as the water-colour rectangle). A replacement may implement it as that loop; a no-op leaves record `+0x10` as the replacement wrote it.

### 15.5.13 `DoPostCityInit` steps 6–7, the texture layers, the per-cell index call and the load path (TR33, TR34, TR36, TR37) *[compat]*

All facts below come from the instruction bytes (re_query export). REA was not available to this lane. "thiscall"
means `ECX` = this, arguments on the stack pushed right to left, callee pops them (`ret n`).

#### 15.5.13.1 Terrain `+0x100` is the weather simulator; step 6 runs its `DoCompleteSimulation` (TR34)

- **Writer.** `cSTETerrain::Init` (`0x00750064`–`0x00750079`): if `+0x100` is null, `+0x100 := city->GetWeatherSimulator()`
  (city = `+0x1C`, `cISC4City` slot 70, vtable `+0x118`), then `AddRef` it when it is non-null. The terrain's
  slot-4 `Shutdown` releases it (slot 2) and clears `+0x100`. No other code writes `+0x100`.
- **Slot 5** of `cISC4WeatherSimulator` is `DoCompleteSimulation` (`0x007296F0`; thiscall, no arguments, `ret`
  with no pop, result in AL, always 1). It runs only once per simulator, guarded by the simulator's byte `+0x19`.
  The first time, it calls `DoMonthlySimulation(1, 0)` once, then `DoWeeklySimulation(0, 0)` repeatedly until that
  returns non-zero, then sets `+0x19`. So step 6 brings the weather (moisture) maps to a settled state before
  step 7 reads them.
- **Contract.** A replacement calls `city.GetWeatherSimulator()->DoCompleteSimulation()` at step 6, after it sets
  `+0xEC`. It ignores the result. Step 6 dereferences `+0x100` **without a null test** (`0x007495E1`–`0x007495F0`),
  so the original assumes the city has a weather simulator.

#### 15.5.13.2 Step 7, part 1: `CalculateNormalsAndAssignCliffTextures(rect*)` `0x00743B60` (TR33)

The function is thiscall with `this` = terrain base and one argument, a pointer to an inclusive vertex rectangle
`(x0, z0, x1, z1)` (`ret 4`). It does not modify the rectangle. Step 7 passes `&terrain+0x40`, the whole city.
Its other caller is `RedisplayTerrain` (`0x007498CD`, §8.1 step 2), which passes a local copy.

1. Clear vertex flag `0x0008` (the cliff mark) on every vertex of the **ungrown** rectangle (`0x007409B0(rect, 8)`:
   `flags &= ~8`).
2. Grow the rectangle by one on the low side only: `gx0 = max(x0−1, 0)`, `gz0 = max(z0−1, 0)`. Clamp the high side to
   the cell range: `gx1 = min(x1, cellsX−1)` (`+0x30`), `gz1 = min(z1, cellsZ−1)` (`+0x34`). Clear flag `0x0800` on
   that rectangle (`ClearVertexCliffFlipTriangulationFlags` `0x00740900`).
3. For each cell row `z = gz0..gz1`, compute the two triangle normals of each cell `x = gx0..gx1` into a two-row
   scratch buffer. The buffer holds 6 floats per cell, and the rows at `+0x118` and `+0x124` swap after each row.
   With `a = alt(x,z)`, `b = alt(x+1,z)`, `c = alt(x,z+1)`, `d = alt(x+1,z+1)` (all float32):
   - triangle 0: `(16(c−a) − 16(d−a), 256, 0·(d−a) − 16(c−a))`;
   - triangle 1: `(0·(d−a) − 16(b−a), 256, 16(b−a) − 16(d−a))`.
   Each step is rounded to float32 in the order shown (the 16 and the 0.0 are float constants `0x00AB3B5C` and
   `0x00A81054`; the `0·` term only matters for an infinite altitude). Each normal is then normalised: the x87
   computes `s = 1.0 / sqrt(x² + y² + z²)` (summed x, y, z in that order, unrounded), then stores `x·s`, `y·s`, `z·s`
   as float32. If `|y| < 0.3` (`0x00B0D620`) for either triangle, the function calls
   `MarkCliff(vertexIndex, cliff0, cliff1)` (`0x00742FC0`). MarkCliff tests the other diagonal. If that diagonal
   removes the cliff, it calls slot 93 for the cell and sets flag `0x0800`. It then sets flag `0x0008` on the
   corners of the steep triangle or triangles. The exact rule is §15.5.13.6.
4. In the same row pass, once `z ≥ z0` (the ungrown top), compute the **vertex normals** of row `z` for
   `x = x0..x1`. Each one is the sum of the triangle normals of the triangles that touch vertex `(x, z)`, with each
   term present only when its cell exists:
   - cell `(x−1, z)`: triangle 1 (needs `x > 0`, `z < cellsZ`);
   - cell `(x−1, z−1)`: triangles 0 and 1 (needs `x > 0`, `z > 0`);
   - cell `(x, z)`: triangles 0 and 1 (needs `x < cellsX`, `z < cellsZ`);
   - cell `(x, z−1)`: triangle 0 (needs `x < cellsX`, `z > 0`).
   A zero sum becomes `(1, 1, 1)`. Normalise it the same way and store it at record `+0x04/+0x08/+0x0C`. **Quirk:**
   the row loop stops at `cellsZ−1`, so this function never recomputes the normals of the last vertex row
   `z = cellsZ`. They keep their previous values.

Its observable effects are the record normals `+0x04..+0x0C`, flags `0x0008` and `0x0800`, and the slot-93
triangulation flips. It posts no message, and it touches neither the view nor the texture layers.

#### 15.5.13.3 Step 7, part 2: the texture-layer update `0x007490B0` (TR33, TR36)

This is thiscall with `this` = terrain base and one argument, `rect*` (`ret 4`). **It pads the caller's rectangle
in place.** In step 7 that rectangle is `+0x40`, which the clamp leaves unchanged. Its six callers are
`DoPostCityInit`, `RedisplayTerrain`, `SetTerrainType`, `StopAccumulatingTextureUpdates`, `TickVolcanicAshRegions`
and `DoMessage`. In order (`0x007490B0`–`0x0074912C`):

1. `ComputeWaterProximityValues(rect)` (`0x007440C0`), on the unpadded rectangle.
2. `PadRectangle(rect, pad)` (`0x00741E30`, thiscall, `ret 8`): `pad = 0x00B4BC48[terrain+0xBC]` is a
   per-zoom-level table that `StaticInit` fills. The function subtracts `pad` from `x0`, `z0` and adds it to `x1`,
   `z1`. It then clamps `x0, z0 ≥ 0`, `x1 ≤ cellsX` (`+0x28`) and `z1 ≤ cellsZ` (`+0x2C`). Every later step uses the
   padded rectangle.
3. `0x00743840(rect)`: if `+0x100` (the weather simulator) is non-null, it calls the simulator's slot 16
   `UpdateMoisture(&r)` (`+0x40`). `r` is four int32 values, each one the corresponding rectangle int converted to
   float, multiplied by float 16.0 and truncated (metres).
4. `0x00740890(rect)`: for every vertex in the rectangle, set the three **texture-layer ids** at record `+0x16`,
   `+0x1A` and `+0x1E` (u16) to 0. The layer weights are not cleared.
5. If `+0xEC` (view ready): call the view's row-dirty marker `0x00755ED0(zLo, zHi)` with `ECX` = view base,
   `zLo = max(z0−1, 0)` and `zHi = min(z1+1, cellsZ−1)` (`+0x34`; unsigned compare). It returns with `ret 8`. It ORs
   bit `(row + [0x00B4C7F0])` into the bit-vector `0x00B4C7EC` for every row in `zLo..zHi` and does not read
   `this`.
6. `0x007482C0(rect)`: adds the **base** layers. Inputs: terrain type `+0x18` (per-type tables at
   `0x00B4B414 + 4·type` and `0x00B4B420 + 0x800·type`), sea level `+0x7C`, the surface-water depth
   (`[+0x24]+0x18`), the vertex altitude and normal, and response curves (`cSC4ResponseCurve::GetOutputValue`). For
   example, a vertex whose water depth is ≤ 30.0 and that has the cliff mark `0x0008` gets layer id
   `0x00B4B414[type]` with weight argument 50.0 (`0x00748402`–`0x00748418`). A positive depth selects id `0x14`
   when the depth is < 15.0, else `0x13` (`0x00748429`–`0x00748454`). Full rule: §15.5.13.6.
7. `0x00748950(rect)`: adds the **brush-painted** layers. Each entry in the list at `+0x1B0` is a painted region
   with a fade counter. For every vertex inside an entry's padded rectangle that carries the brush mark `0x0400`
   (§7), it adds that entry's texture with a weight that fades with the counter (`0x00748AD0`–`0x00748C34`).
8. `0x00740690(rect)`: normalise the weights (below).

The full selection and weight rules of steps 1, 6 and 7 are §15.5.13.6 (TR41).

**The record's texture layers (TR36).** Record bytes `+0x16..+0x21` hold three layers. Each layer is
`{u16 id, u16 weight}` at `+0x16/+0x18`, `+0x1A/+0x1C` and `+0x1E/+0x20`. Id 0 marks an empty layer, and the used
layers come first. `ComputeTextureSpansForRow` and `DrawTerrainSurfaceRows` read `+0x16`, the first layer's id.
The only writers found are `0x00740890` (clear), `0x007405E0` (add) and `0x00740690` (normalise), all reached through
`0x007490B0`. No slot writes them *[inferred: from the callers of those three]*. The layer is added by `0x007405E0(vertexIndex, id, weight)` (thiscall, `ret 0xC`;
`weight` is a float):
- `w = trunc(weight × 255.0 + 1.0)` (x87, float constants `0x00A9D8B0` and `0x00A81228`, `_ftol`), stored as u16;
- scan layers 0, 1, 2. At the first layer with id 0, store `(id, w)` there and stop. At a layer with the same id,
  raise its weight to `w` if `w` is larger, then stop;
- when all three layers are full with other ids, take the lightest layer (the first one on a tie). If its weight
  is below `w` (unsigned), replace it with `(id, w)`.

Normalisation `0x00740690`: per vertex, `sum` adds up the weights of the layers before the first id-0 layer. If
`sum > 250`, each of those layers' weights becomes `trunc(weight × 250 / sum)` (unsigned integer division).

**Replacement rule.** A replacement that leaves `+0x16` at 0 draws every vertex with texture 0. To match the
original it must run steps 1–8 on the whole city in `DoPostCityInit` step 7, and again from the same five other
callers. The view-dirty step 5 is required, or the view keeps drawing stale rows.

**Order after `Init`.** Step 6 (`+0xEC := 1`, then weather `DoCompleteSimulation`) comes first. Then step 7: first
normals and cliff marks, then the texture update, both on `+0x40`. Then step 8 copies the view pointers. The view's
`Init` built its index buffers before any of this ran. The cliff flips that MarkCliff makes through slot 93 reach
the view per cell through the TR37 call below.

#### 15.5.13.4 The per-cell index call `SetTriangleIndicesForCell` `0x00752AF0` (TR37)

- **Convention.** thiscall with `ECX` = **view base** (`terrain+0xE8`, not `+0x0C`). There are two stack arguments,
  pushed `x` first and then `z`, so the argument order is **`(z, x)`**. The callee pops them with `ret 8`.
  Evidence: `0x007411D9`–`0x007411E1` (`mov ecx,[ecx+0xE8]; push ebx(x); push edi(z); call`); `0x0074187B`–`0x0074187D`
  (`push esi(x); push edx(z)`); the callee's index `[0x00B4C74C] × arg1 + arg2` (`0x00752AF0`–`0x00752B0C`).
- **What it does.** It reads the cell's `(x, z)` record flags through `view+0x20` (the terrain) `+0x6C`: bits `0x20`,
  `0x40`, `0x200` and `0x3000`. It then rewrites that cell's six u16 triangle indices in the view's static index
  array `0x00B4C6F4`, at `z × [0x00B4C700] + 6x` (u16 units). It returns nothing that callers use.
- **Timing.** Synchronously, inside the terrain slot, right after the flag write, and only when `+0xEC` is set.
  `FlipInternalTriangulation` (slot 93) and `FlipExternalTriangulation` (slot 94) call it only when the flag word
  actually changed (`0x007411C6`). `HideEvenTriangle`, `HideOddTriangle` and `HideTerrainCell` call it whenever
  `+0xEC` is set, even when nothing changed. `SetTerrainCellFlags` calls it at `0x0074208C`.
- **Replacement rule.** Make the same call in the same slots, with the same `+0xEC` guard. Without it the view's
  index buffer keeps the old triangulation until its next whole-grid rebuild, which happens only in its `Init`.

#### 15.5.13.5 The load path: `Read` hands `DoPostCityInit` an existing view

`Read` (§13.2) calls `SC4ReadGZSerializable(stream, 0xC9B84E10, &+0xE8, 0)` (`0x005D9730`). That function zeroes
`*out`, queries the stream for interface `0x4999A8F5` and calls the inner reader `0x005D96C0`. The inner reader
calls stream slot 26 (`+0x68`) to fetch the deserialised object for the saved reference. If the object is not null,
it calls `QueryInterface(0xC9B84E10, out)` on it. The view's `QueryInterface` (`0x00752730`) answers `0xC9B84E10`
with **the view base** and `AddRef`s it (`0x007527BC`). A null saved reference leaves `*out = 0` and still
succeeds. The serialization system constructs the view object and runs its own record `Read` (`0x007527D0`,
serializable interface at view `+0x14`). That `Read` sets view byte `+0x19`, requires u16 version 1 and reads one
byte into the **static** grid-display flag `0x00B4C741`.

This changes §15.5.1 as follows. On a loaded city, `+0xE8` is already set when `PostCityInit` arrives, so
`DoPostCityInit` **skips steps 3–4** (`0x0074956D` `jnz 0x007495D4`). It goes straight to step 5,
`view->Init(terrain)`, on the deserialised object. The terrain then owns one reference in every case: the
factory's out-reference, the fallback `new` + `AddRef`, or the load path's `QueryInterface` `AddRef`. Shutdown
releases that one reference (TR35). `Init` and every later step are the same on both paths. The only state the
load path adds is the grid-display static. **Replacement rule:** a replacement may ignore the saved reference and
build a fresh view in `DoPostCityInit` *[choice]*. The view's saved state is only the static grid flag, and the
view's own record `Read` restores that flag whether or not the terrain keeps the object. If the replacement keeps
the object, it must call `Init` on it exactly as for a new view. It must not construct a second view.

#### 15.5.13.6 The exact rules of MarkCliff, the water-proximity grid and the two texture passes (TR41) *[compat]*

All facts here are from the instruction bytes (Ghidra's pseudocode inverts two of the compare polarities in
MarkCliff and is not a reliable guide). `W` = `vertsX` (`+0x38`, the record row stride), `v = z·W + x`, `alt(v)` =
record `+0x00`, `depth(v)` = float at `[[+0x24]+0x18] + 0x24·v` (the surface-water record, same stride as the
terrain records), `flags(v)` = record `+0x22` (u16). "Steep" means `|ny| < T`, where `T` = `0x00B0D620` (tuning
`0x48CD7B65`, code default 0.3, shipped 0.5, §2.4) and `ny` is the y component of the normalised triangle normal.

**MarkCliff `0x00742FC0`** (thiscall, `ret 0xC`; arguments `(u32 v, u8 steep0, u8 steep1)`, where `steep0` is
triangle 0 = corners `(v, v+W, v+W+1)` and `steep1` is triangle 1 = corners `(v, v+1, v+W+1)` of cell `(x, z)`; the
caller passes the two `|ny| < T` results and calls it only when at least one is set, `0x00743B60` §15.5.13.2 step 3):

| `steep0` | `steep1` | what happens |
|---|---|---|
| 0 | 1 | set flag `0x0008` on `v`, `v+W+1`, `v+1` (triangle 1); no flip |
| 1 | 0 | set flag `0x0008` on `v`, `v+W`, `v+W+1` (triangle 0); no flip |
| 1 | 1 | test the other diagonal (below) |

The other diagonal's triangles are A = `(v, v+1, v+W)` and B = `(v+1, v+W, v+W+1)`. With `c = alt(v+W)` as origin,
`p = alt(v) − c`, `q = alt(v+1) − c`, `r = alt(v+W+1) − c` (float32 stores `0x00743007`/`0x00743010`):
- A: `nx = q·(−16) − p·(−16)` (stored float32), `nz = p·16 − q·0`; A is steep when
  `|1.0 / sqrt(nz² + nx² + 65536) · 256| < T` (sum in that order; `0x0074302C`–`0x0074306A`);
- B: `nz = r·(−16) − q·0`, `nx = q·16 − r·16`; B is steep when `|1.0 / sqrt(nx² + nz² + 65536) · 256| < T`
  (`0x0074307D`–`0x007430CA`).
Constants: −16 `0x00A98FB0`, 16 `0x00AB3B5C`, 0 `0x00A81054`, 65536 `0x00AB3CAC`, 1.0 `0x00B0D684`, 256 `0x00A9422C`.
A NaN result counts as not steep.

- **A and B both steep:** no flip. Set flag `0x0008` on all four corners (both original triangles).
- **Otherwise** (the flip removes at least one steep triangle): call the terrain's own slot 93
  `FlipInternalTriangulation(x = v mod W, z = v div W, 1)` through the vtable (`+0x174`, unsigned divide), then set
  flag `0x0800` on `v` (byte `+0x23` bit 3), then: if A is steep, flag `0x0008` on `v`, `v+W`, `v+1`; if B is steep,
  flag `0x0008` on `v+1`, `v+W`, `v+W+1`. If neither is steep, no `0x0008` is set for this cell.

Flags are only ever ORed in; the clears are §15.5.13.2 steps 1–2. The slot-93 call is virtual, so a replacement's
own slot 93 runs (and with `+0xEC` set it reaches the view through the TR37 call, §15.5.13.4).

**Water-proximity grid `+0x190`, `ComputeWaterProximityValues(rect*)` `0x007440C0`** (step 1 of §15.5.13.3).
`+0x190` is a vector of `vertsZ` rows (12-byte vector entries, the first dword of each is the row's byte array of
`vertsX` bytes). The function first widens a pending dirty rectangle at `+0xAC/+0xB0/+0xB4/+0xB8`
(`x0 = min`, `z0 = min`, `x1 = max`, `z1 = max` with the argument). Only when `(z1−z0)·(x1−x0) ≥ 100` (signed,
`0x00744132`) does it recompute; otherwise the grid stays as it was and the rectangle keeps growing. On a
recompute: reset the pending rectangle to the empty sentinel `(INT_MAX, INT_MAX, INT_MIN, INT_MIN)`
(`0x00B0D698..0x00B0D6A4`), set `+0xA8 = 0` and `+0xE4 = 0`; for every vertex of the **whole** grid write 0 if
`depth > 0` (and count it in `+0xE4`) else `P + 2`, where `P` = u8 `0x00B0D58C` (tuning `0xA9C283D0`, code default
4). If at least one vertex is wet, run `cSC4MapUtilities::FloodFillDistance(grid, 0, P + 1)` (`0x005E5180`): for
`level = 0, 1, …, P` (stopping early when a pass changes nothing), every cell holding `level` lowers each of its
4 neighbours (clamped at the grid border) to `level + 1` if that neighbour is larger (`Expand` `0x005E4C50`, row by
row, in place). Result: `prox(v) = min(manhattanDistanceToNearestWetVertex, P + 1)`, with every vertex farther than
`P + 1` left at `P + 2`; with no wet vertex every value is `P + 2`. Step 7 passes the whole city, so the first
call always recomputes.

**Base-texture pass `0x007482C0(rect*)`** (thiscall, `ret 4`; the rectangle is the one §15.5.13.3 step 2 already
padded). Setup, in order:
1. `L0 = 7.0 + seaLevel` (`0x00B0D590`, tuning `0xA9C283D1`; sea level `+0x7C`), float32.
2. `cliffId = 0x00B4B414[type]` (`type` = `+0x18`).
3. `span = (maxAllowedHeight − L0) · 0x00B4B400` (`0x00B0D5DC`, §2.4; `0x00B4B400` = tuning `0x48CD7B30`), float32.
4. Keep a copy `R` of the rectangle as passed. Call `0x007574A0(type)` then `0x007571D0(zoom)` (cdecl, one argument
   each; `zoom` = `+0xBC`): if the view's current terrain type `0x00B0D6FC` differs, `ChangeTerrainType`, store the
   type and refill the per-level texture tables `0x00B4C670[level]` (§15.5.12.2); then refill them for `zoom`.
5. Pad a second copy `Q` with `PadRectangle(P, 0x00B4BC48[zoom])` (the same per-zoom pad again, clamped to
   `0..cellsX`, `0..cellsZ`).

Then for each source vertex, `z = Q.z0..Q.z1` outer, `x = Q.x0..Q.x1` inner (inclusive, signed compares), with
`v = z·W + x`:
- if `depth(v) > 30.0` (`0x00B0D5D8`, tuning `0x69AC5641`): nothing.
- else if `flags(v) & 0x0008`: `AddLayer(v, cliffId, 50.0)` (§15.5.13.3, `0x007405E0`). Nothing else.
- else if `depth(v) > 0` (**water source**): `id = 0x14` if `depth(v) < 15.0` (`0x00B0D5D4`, tuning `0x69AC5640`),
  else `id = 0x13`. (**Corrects** §15.5.13.3's earlier "0x14 when the depth is ≥ 15": shallow water is `0x14`.)
  `full = (Family(v, id) << 12) + id`. Splat (below) with the water-only filter.
- else if `prox(v) > P` (unsigned) **or** `alt(v) > L0` (**land source**): `band = 31` if `alt(v) ≤ L0`; else
  `t = (alt(v) − seaLevel) / span · 30.0` (x87, unrounded) and `band = 31 − floor(t)` clamped to `0..31`
  (`floor` is computed as round-to-nearest of `t − 0.5 + 12582912.0` stored float32, then +1 when `t` equals that
  plus one exactly: `0x00748532`–`0x00748588`). `m = weather.GetMoistureType(x·16.0, z·16.0)` (slot 19, 0..15,
  flora.md). `id = u32 0x00B4B420[type·0x200 + band·16 + m]`. `full = (Family(v, id) << 12) + id`. Splat with the
  land-only filter.
- else (**low dry source**: within `P` of water and not above `L0`): `id = 0x00B4B410[type]`,
  `full = (Family(v, id) << 12) + id` (`0x00742F60`); if `(x, z)` lies inside `R`, `AddLayer(v, full, 1.0)`. Then
  splat with no depth filter.

`Family(v, id)` (`0x00742F60` and inline): `q = v div W`, `r = v mod W`, `D = (*(u32**)0x00B4BC90)[zoom]`
(tuning `0x89A312C2`), `k = (q div D) · 0x00B4BC20[zoom] + (r div D)`, `f = u16 (*(u16**)(0x00B4BC9C + 12·zoom))[k]`
(filled by `DetermineTextureFamilyMembersForGridCells`), result `f mod u32 [(0x00B4C670[zoom])[id] + 4]` (the
texture's family size at that level). All unsigned.

**Splat** (source `(x, z)`, base `id`, `full`): look `id` up in the hash table `0x00B4BCE0..0x00B4BCE4` (bucket
`id mod bucketCount`, singly linked nodes `{+0 next, +4 id, +8 float strength, +0xC + 16·zoom cSC4ResponseCurve,
+0x5C + 4·zoom int radius, +0x70 + 4·zoom float reach}`). No node: done. Else `rad = radius[zoom]`,
`lim = (0.0625 · reach[zoom])²` (float32), neighbourhood `N = (max(x−rad, 0), max(z−rad, 0), min(x+rad, cellsX),
min(z+rad, cellsZ))` (low side signed, high side unsigned against `+0x28`/`+0x2C`), and `I = N ∩ R` (inclusive,
`0x0048ED90`; empty: done). For each target `(x', z')` in `I`, rows outer, columns inner, `v' = z'·W + x'`:
1. land-only filter: skip unless `depth(v') ≤ 0`; water-only filter: skip unless `depth(v') > 0`;
2. skip unless `alt(v') > L0` **or** `prox(v') > P` **or** `depth(v') > 0`;
3. `d2 = (z'−z)² + (x'−x)²` (int); skip unless `float(d2) < lim`;
4. `w = curve[zoom].GetOutputValue(float32(sqrt(d2) · 16.0)) · strength` (stored float32; curve evaluation per
   foundations.md, `0x005F4250`); if `w > 0`, `AddLayer(v', full, w)`.

The splat reaches only vertices inside `R`, so the extra padding only adds sources whose splats reach into `R`.
The order of all AddLayer calls is as above; it matters because AddLayer keeps the first three ids and replaces
the lightest.

**Brush/ash-texture pass `0x00748950(rect*)`** (thiscall, `ret 4`; `R` = the rectangle as passed, not modified).
The list at `+0x1B0` is a `std::list` (head sentinel; node `+0 next`, `+4 prev`, `+0x08..+0x14` an inclusive
rectangle `(x0, z0, x1, z1)`, `+0x18` u32 age counter, `+0x1C` a dword); its only other user in the terrain is
`TickVolcanicAshRegions` (`0x00749AF0`), which ages it *[inferred: from the users of `+0x1B0`]*.
1. If the list is empty, return. Look up hash node `id = 0xE0` exactly as the splat above; none: return.
2. For each list entry `e`, head-to-tail:
   1. `Pa = R` padded by `radius[zoom]` (node `+0x5C + 4·zoom`) with `PadRectangle` (clamped to `0..cellsX`,
      `0..cellsZ`); `Pa = Pa ∩ e.rect` (`0x0048ED90`). Empty: next entry.
   2. If `Pa` equals `e.rect` exactly, set `e+0x1C = 0`.
   3. `fade = (1.0 − float(e.age) / float(0x00B0D624)) · strength` (both u32 converted with the +2³² fix-up; the
      data value of `0x00B0D624` is 10 000 000; `strength` = node `+8`; one float32 store, `0x00748A5F`–`0x00748A99`).
   4. For each source `(x, z)` in `Pa`, rows outer, columns inner, with `flags(v) & 0x0400` (byte `+0x23` bit 2, the
      kind-7 brush mark, §9.4.2): `N` = the same clamped `±radius[zoom]` neighbourhood as the splat; `I = N ∩ R`;
      for each target `(x', z')` in `I`, rows outer: `w = curve[zoom].GetOutputValue(float32(sqrt(dx·dx + float32(dz²))
      · 16.0)) · fade` (`dz² = (z'−z)²` as an int converted and stored float32; `dx = x'−x` squared on the x87; sum,
      sqrt, ×16.0 `0x00AB3B5C`); if `w > 0`, `AddLayer(v', 0xE0, w)`. No family variant, no depth, proximity or
      altitude filter, and no `lim` distance cut-off (only the rectangle).

**Where the tables come from.** `0x00B4B410/414/420`, the hash `0x00B4BCE0`, `0x00B4BC20`, `0x00B4BC48`,
`0x00B4BC90` and `0x00B4BC9C` are process statics filled by `cSTETerrain::StaticInit` (`0x0074FE40`) and its
readers (`ReadTuningParameters` reads `0x89A312C2` into `0x00B4BC90` and calls
`DetermineTextureFamilyMembersForGridCells`; `0x00B4B410`/`0x00B4B414` are written by
`ReadMiscTexturesForTerrainTypes` `0x0074C800`, called from `ReadTerrainIniFile`; the writers of `0x00B4B420` and of
the hash nodes were not traced, so their file origin is not specified here). The per-level `0x00B4C670` tables are the view's (§15.5.12.2) and
are refreshed by step 4 of the base pass. **Replacement rule:** a replacement that runs `StaticInit` (required
anyway by §15.5.12.2) reads these statics in place and applies the rules above; it does not need to re-derive their
contents. The two passes post nothing and touch only the texture layers of the records.

## 15.6 The slots §4–§10 left undefined (TR24, TR25) *[compat]*

All confirmed by reading `0x00AB3D18 + 4N` and disassembling the target. Two helpers matter throughout:
**`0x009EEF04` is MSVC `_ftol2`** (float → int64 `EDX:EAX`, **truncate toward zero**) — `names_windows.tsv`
mis-names it `_lua_gettop`, which is why the Mac decomp of these functions lost their float arguments. And the x87
compare idiom: `test ah,5` + `jp` ⇔ `a < b`; `test ah,0x41` + `jne` ⇔ `a > b`; `test ah,0x44` + `jnp` ⇔ `a != b`;
all strict, and NaN never takes the first two's "then" branch.

### 15.6.1 TR24

| Slot | Signature | Rule |
|---|---|---|
| 25 `GetTerrainGridIndex` (`0x0074C6B0`) | `int(float x, float z)`, pop **8**, **EAX** | `EAX = trunc(r24(x·0.0625)) + trunc(r24(z·0.0625)) · vertsX(+0x38)`. **No bounds check, no clamp.** *[quirk]* it uses `_ftol2` (truncate toward zero) where `GetAltitude` uses floor, so they disagree for negative coordinates (`x = −8` → 0 here, −1 there) |
| 39 `AreSeaVertices(rect*)` (`0x00740750`) | `bool(const tRect*)`, pop **4**, **AL only** | Inclusive **vertex** rect `(x0,z0,x1,z1)`. (1) any of x0,x1 ≥ `vertsX(+0x38)` or z0,z1 ≥ `vertsZ(+0x3C)` (unsigned) → **false**; (2) empty (`z0>z1` or `x0>x1`) → **true**; (3) else **true iff every vertex has `altitude < sea(+0x7C)`**. **Ground altitude, not depth** — it is an "all sea" test, not a presence test. NaN altitude → false |
| 58 `GetBrushExemplarID` (`0x007415D0`) | `bool(int index, u32* out)`, pop **8**, **AL only**; **`this` unused** | `*out = *(u32*)(0x00AB3C88 + 4·index)`; `AL = (*out != 0)`. **No bounds check.** The table is 8 dwords; index ≥ 8 leaks adjacent `.data` |
| 61 `BrushRadius` (`0x00747AE0`) | `float(u32 brushID)`, pop **4**, **ST0** (r24) | Lookup brushID in the `std::map` at `0x00B4BD58`. **Not found → 32.0.** Found → `radiusArray[defaultBrushLevel(+0xBC)]`, `radiusArray = *(float**)(node+0x20)` — the element-wise **max over all components'** radius arrays (merge at `0x0074F533`–`0x0074F57D`). No clamp on the level. Ignores `tBrushParameters.radiusScale` |
| 62 `GetCostPerBrushApplication` (`0x00747C90`) | `int64(u32 brushID)`, pop **4**, **EDX:EAX** | Same map lookup. **Not found → 32.** Found → `EAX:EDX = trunc(cost[+0xBC])`, `cost[level] = *(float*)(node+0x38 + 4·level)`, the 5-float per-level array written from brush property `0x8A020101` (defaults all zero). No clamp on the level |

**Slot 58's table** at `0x00AB3C88`: 0 `0xEA312EF9` GodMountainMesa, 1 `0xCA313035` VolcanoBrush, 2 `0xEA312F77`
CanyonSoftBrush, 3 `0x2A312F97` LevelBrush, 4 `0xEA313BB9` SmoothingBrush, 5 `0x8A31301C` (not in §9.7.1
*[inferred]*), 6 `0x2A31304E` FloraBrush, 7 `0` (AL = 0).

### 15.6.2 TR25

| Slot | Signature | Rule |
|---|---|---|
| 43 `CreateFlatAreas` (`0x0074A220`) | `void(bool redisplay)`, pop **4**, **void** | With `M = cachedMaxAlt(+0x74)`, `S = sea(+0x7C)`, `H = MaxAllowedHeight`: `f1 = r24(0.1·M + S)`, `f2 = r24(0.9·M + S)`, `tol = r24(0.003·H)`. **Pass 1:** every vertex, skip if `flags(+0x22) & 1`; seed when **`f1 < alt < f2`** (both strict) → `LevelAreaStartingAt(i, tol, 1)`. **Pass 2, only if any seed levelled:** `MarkRegion(…,2)`, `ComputeBoundsOfMarkedRegion`, `SmoothenMarkedRegion(&r, 3, 2)`, `flags &= 0xFC` over `vertexCount(+0x5C)`, then if `redisplay`: `RedisplayTerrain(1, 1, &wholeCity(+0x40), data4 = 0)` |
| 44 `CreateShoreCliffs` (`0x0074C0F0`) | `bool(bool redisplay)`, pop **4**, **AL only** = "changed" | Select vertices with `(flags & 0x0001)==0 && (flags & 0x0010)!=0`; mark `flags |= 1`, grow a region, `AnalyzeShoreAndCreateCliffs` (`0x00749FA0`) — region size **< 4** → nothing; else walk up to 5 shore steps summing `(alt − sea)`, a run is a cliff when the sum **> 15.0**, runs > 4 flushed to `CreateShoreCliff` (`0x00744220`). **Pass 2, only if changed:** `MarkRegion(…, 8)`, bounds, `SmoothenMarkedRegion(&r, 20, 2)`, and if `redisplay`: `RedisplayTerrain(1, 1, &wholeCity, data4 = 0)`. **Always**, even if nothing changed: `flags &= 0xF8` over all vertices. Returns whether anything changed. *[inferred]* the 5-step / 15.0 / run-length geometry is from the decomp of `AnalyzeShoreAndCreateCliffs`; the Windows bytes confirm the size `< 4` early-out (`0x00749FD5`) and the 15.0 compare, but `CreateShoreCliff`'s per-vertex heights (`0x00744220`) are untraced |
| 46 `ChangeSeaLevel` (`0x00740850`) | `void(bool raise)`, pop **4**, **void** (only AL of the arg is read) | `raise` → `SetSeaLevel(r24(step + sea))`, else `SetSeaLevel(r24(sea − step))`, `step = 0x00B0D584` = tuning `0x48CD7B72` = **5.0** shipped. The call is **virtual** (slot 45), so all of §6 `SetSeaLevel` applies — including rejecting a value outside `[−10, 200]`, so at a bound the proposal is refused outright |
| 48 `ChangeTerrainLevel` (`0x00746440`) | `void(bool raise, float amount)`, pop **8**, **void** | Every vertex, skip if locked (`flags & 0x4000`). raise: `alt := r24(amount + alt)`, clamped to `MaxAllowedHeight`; lower: `alt := r24(alt − amount)`, clamped to `MinAllowedHeight`. **Always** `depth := r24(sea − alt)` into the surface-water array. Tail: `RedisplayTerrain(1, 1, &wholeCity, data4 = 1)` — **unconditional**, even when every vertex was locked |
| 56 `StartFlood` (`0x00747720`) | `void()`, pop **0**, **void** (tail-calls `DoFloodFlowStep`) | **Corrects §15.1:** more than a debug dump. (1) `SaveWaterLevelsInTempArray(&wholeCity)` (`0x00744DE0`) snapshots every vertex **depth** into the float scratch at `this+0xE0`; (2) flood rect `(20,20,40,40)` into `this+0x19C..+0x1A8`, step counter `this+0x1AC := 0`; (3) for `z,x` in 20..39 (half-open 20..40): `depth[i] := r24(depth[i] + 500.0)`, `i = x + z·vertsX`; (4) `this+0xD4 := 1` (flood-running); (5) **one `DoFloodFlowStep` runs immediately** (`jmp 0x007467F0`), which posts the water-only redisplay with data4 0 and clears `+0xD4` once `+0x1AC` passes 500. So the amount and block MATCH §15.1, but the snapshot, rect, counter and the immediate step are missing there |

**`LevelAreaStartingAt(this, startIndex, tol, flagMask)`** (`0x00749660`, pop `0xC`, **AL only**) underpins slot 43:
BFS over the offsets at `0x00AB3CD0`/`0x00AB3CE0` in the order **(0,−1), (−1,0), (0,1), (1,0)**. A neighbour joins iff
it is in bounds, `(flags & mask)==0`, **`alt > sea`**, and **`|alt − startAlt| < tol`**; then `flags |= mask`. If the
region count is **≤ `0x00B0D5F0`** (tuning `0x48CD7B50`, shipped **1000**) it unmarks and returns 0; otherwise it
calls **slot 50 `SetAltitudeAtVertex(i, startAlt)`** for every vertex and returns 1 — levelling the patch to the
seed vertex's original altitude.

### 15.6.3 Fields and flags this adds

New terrain fields (§2.3): `+0xD4` flood-running flag, `+0xE0` float scratch array of `vertexCount` depths,
`+0x19C`/`+0x1A0`/`+0x1A4`/`+0x1A8` flood rectangle, `+0x1AC` flood step counter.

**§7 addendum:** the low flag bits `0x0001`, `0x0002`, `0x0004` are **transient work marks** — `CreateFlatAreas`
clears `0xFC` when it changed something, `CreateShoreCliffs` clears `0xF8` **always**. They are never saved (the
corpus never shows them), so a replacement may use them as scratch and must not persist them.
### 15.5 The terrain 3D view (`cSTETerrainView3D`) — specified in §15.5.1–§15.5.11 above

Rendering only; the view has no simulation state. It is class `0xC9B84E10`, held at **terrain `+0xE8`**, and its own
save record is u16 `1` + one byte (§13.2; the byte is the grid-display flag, §15.5.10's `0x00AB43E8` row). The
facts this spec's callers rely on:

- **`cISTETerrain` slot 66 `GetView` (`0x00741810`) returns `*(terrain + 0xE8) + 0x0C`**, or null when the field is
  null. Five instructions. Same `+0x0C` trick as `cSC4View3DWin::AsIGZWin` (ui-windows.md §3.2).
- **`cSTETerrainView3D::Init` (`0x0075C420`) is `__thiscall` with one dword argument — the terrain — and `ret 4`.**
  It returns `AL = 0` when `+0x18` says it is already initialised, and stores into `+0x1C` (city), `+0x20` (the
  terrain argument) and `+0x24`, `AddRef`ing each only when the field is null.
- **Two globals are dereferenced with no null test:** `0x00B43CD8` (the **preferences** object, read as `*(byte*)(obj+1)`
  and cached into `0x00B4C741`) and `0x00B43CF0`. `0x00B43CEC` (the city) *is* null-tested at `0x0075C473`.
- **`0x00B43CF0` is `cISTESurfaceWater`** — city slot 83 `GetSurfaceWater` (`0x00601C20` stores city slot 84
  `GetTerrain` into `0x00B43CF4`/`0x00B43CF8` and slot 83 into `0x00B43CF0`; the teardown at `0x00601FF0` clears
  all three). The view holds city, terrain and surface water because `Pick` and the water redraw read the water
  array through `view+0x24` → water `+0x18` (§15.5.5).
- The vtables, the change-message behaviour, the draw-time statics and the full slot table are §15.5.3,
  §15.5.8–§15.5.10; the replacement checklist is §15.5.11.

## 16. Quirks (summary)

1. `GetAltitude` ignores the triangulation flips (§4.1).
2. The cached minimum and maximum go stale and are saved stale (§4.6).
3. `RedisplayTerrain` posts nothing before `DoPostCityInit` (§8.1).
4. `SetAltitudeAtVertex` leaves a stale depth on land raised above the sea (§6).
5. `RaiseVertex`/`LowerVertex` erase lake water (§6); the lava does it with a zero raise every tick (§11.3).
6. The brush angle of an oriented brush is shared global state when `from = to` (§9.3).
7. Bitmap truncation reaches half a pixel further on the low side (§9.3).
8. `IsNavigable` accepts the rest of the square at the first high bridge (§10.4).
9. Lakes are not saved (§13.3).
10. The boolean slots return in AL only (§5).
11. The brush spans are a scan conversion with half-open rows and a one-past-right column (§9.3); the brush `sin` /
    `cos` are never snapped (§9.3).
12. Every shipped brush has the 0.5° walk step by default, so each in-bounds application draws from the terrain
    generator even though no component uses the walk angle (§9.1, §9.3).
13. The default brush level follows the view's zoom (§2.3).
14. `SetWaterSurfaceDisplayState` posts only on a change (§8.4).
15. The sea level is not saved; a load always restores the tuning value (§13.1).
16. Kind 8 takes its step from the component's **raw** `strength[level]`, ignoring `tBrushParameters.strength`
    (§9.4.5).
17. Kind 6's erosion rectangle is `radius / 16` vertices wide and clamped to the **cell** counts, and it never
    touches the altitude (§9.4.4).
18. The erosion-rate ramp endpoints are read as degrees, converted to radians, then compared with a height
    difference in metres (§9.4.4).
19. ~~The ini's `RaiseLowerAmount` is parsed and thrown away; slot 47 returns the constant 20.0 (§9.7).~~
    **Corrects (TR25): wrong.** Slot 47 (`0x0074C660`) returns the global `0x00B0D588`, which
    `ReadTuningParameters` fills from `0x48CD7B73` — shipped **40.0**, fallback 20.0 (§2.4).
20. Kinds 1 and 3 scatter objects and draw from the terrain generator per vertex, so their RNG cost is not the
    one draw per application of §9.3 (§9.4.2).
21. `DoPostCityInit`'s factory-fallback path does not null-check `operator new(0xE8)`: if the allocation fails it
    faults at `0x007495CD` (`mov edx,[eax]`). The factory-success path does **not** `AddRef`; the fallback path does
    (`0x007495D1`) (§15.5.1 steps 3–4).
22. The view's grid-display flag lives in the **static** `0x00B4C741`, not in any object: slot 20 reads it there,
    slot 21 writes it there, and `Shutdown` writes the static back into the preferences object `0x00B43CD8+1`
    (`0x00756323`–`0x0075632C`). Two views would share one flag (§15.5.10).
23. `SetAutomaticCellCursorUpdateStatus` (slot 10) returns the **old** value in AL, like a getter (§15.5.10).
24. `GetOverlayManager` (slot 18) has no bounds check on the index (§15.5.10).
25. The view's QueryInterface answers the unnamed extra IID `0xC989F960` with the object base; nothing else in the
    image answers it, and `cSC43DRender::DoMessage` asks for it and caches the result (§15.5.10, TR28).
16. Flat indices are not checked in the §10.5 slots, and coordinates past the last column wrap to the next row.

---

## 17. Open questions

- ~~**TR1.** The exact operations of brush component kinds 2, 3, 5, 6 and 8–10, and the shipped `TalusAngle`
  value.~~ **ANSWERED 2026-10-16 (re-answers17 for kinds 2 and 5; re-answers28 for the rest):** kinds 0, 2, 5, 6, 8,
  9 and 10 are checked in the bytes (§9.4), and the whole terrain ini with its shipped values is in §9.7. Shipped
  `TalusAngle` is **5 degrees** (group `0x8A5971C5`, instance `0xAA597172` in `SimCity_1.dat`), which gives
  `Tedge ≈ 1.39982 m`, `Tdiag ≈ 1.97964 m` — very different from the compiled fallback of 0.79 rad. **Still open
  within TR1:** the surface-water tick's use of the water fields at `+0x04` (spring) and `+0x08` (rain) is
  *[inferred]*, and what the `+0xF8` / `+0xFC` placement managers do with a kind-3 / kind-4 placement is outside the
  terrain. One thing is settled but not reproducible: the solid footprint walker `FUN_00742440` **never writes the
  grey byte**, so a no-bitmap component sees whatever residual byte is in the dispatcher's stack slot (§9.4.6) —
  a replacement has to pick one and mark it *[choice]*. None of this is reachable from the harness: kinds 3 and 7
  have no shipped brush, and only `GodValleySpring` and the `ErosionBrush` ship without a bitmap (neither uses the
  callback path).
- ~~**TR2.** The `volcano_control` effect's brush values.~~ **ANSWERED 2026-10-15 (re-specs16), re-checked
  2026-10-16 (re-answers28):** effects.md §8.4 matches §9.5 exactly — level 1, strength 1.0 (range 1–1), radius scale
  0.5, 0.5, 0.6, 0.6, 0.8, 0.8, 1.0, 1.0 or 1.3 (one of nine creators, degenerate ranges, but both
  `RandomDoubleRangeUniform` draws on `0x00B22BE8` still happen), target 100, rate 30–130 applications per
  effect-second for 2–4 effect-seconds, `count = floor(acc)` with `acc += rate × dt`. **One correction:** §9.4 used
  to say kinds 1, 3, 4, 6, 8–11 are used only by the god-mode terraforming tool. That is wrong for kind 6:
  `ErosionBrush 0x6A4BAAFA` carries a kind-6 component and is driven by the effects manager through `erodefx_terrain`
  and `stream_cut_fx_terrain` (effects.md §8.4 records 22–24). Which caller starts those two effect names is still
  untraced.
- TR3 to TR18 are answered in place (QUESTIONS.md has the list).
- ~~**TR21.** The terrain 3D view's init contract.~~ **ANSWERED 2026-10-16 (re-answers30); completed 2026-10-20
  (re-terrain3d):** §15.5. The terrain's own
  `DoPostCityInit` (slot 40, `0x00749540`) creates it — factory CLSID `0xC9B84E10` / IID `0x6771477D`, falling back
  to `operator new(0xE8)` + ctor `0x007567C0` — then calls `view->Init(terrain)` (`0x0075C420`, one stack argument =
  the terrain) and only then sets the view-ready flag `+0xEC`. Slot 66 is `GetView` (`0x00741810`) and returns
  `view+0x0C`. Object layout (size `0xE8`), all five vtables with return channels, the `Init` contract (terrain
  slots 1, 7, 8, 42) and the draw-time reads (the vertex array at `+0x6C`, read directly) are in §15.5. The
  completion (re-terrain3d) adds: every slot's behaviour with pop sizes and return channels (§15.5.10), the full
  slot list the view calls on the terrain with call sites (§15.5.5), the change-message handlers and the
  rebuild-vs-live rule (§15.5.8), the draw-time statics/buffers and `cSTEVertBufExtn` (§15.5.9), and the 17-point
  replacement checklist (§15.5.11). New open questions TR28–TR32.
- ~~**TR23.** The camera zoom word of `0xA6B79602`.~~ **ANSWERED 2026-10-16 (re-answers30):** **DIFFERS** from the
  old §2.3 text. The zoom is at **`*GetVoid2()`** — message slot 15 (`0x0074AADD`), dereferenced at `0x0074AAE0` —
  not `data1`. Same source zones.md §5.2 uses. The **initial** value at `DoPostCityInit` comes from the 3D camera
  global `0x00B43DD8` `+0xC` (`0x00749553`–`0x0074955F`). The camera is **not** reachable from the terrain object,
  but it is reachable at all: `SL::OnView3DInit` (`0x006025D7`) stores `0x00B43DD0` slot 8 into `0x00B43DD8`, and
  `0x00B43DD0` is the same draw-callback registry the view's `Init` already uses (§15.5.4 step 12). A host that has
  neither leaves `+0xBC` at 0, which is what a headless harness sees and is harmless until the first view message.
- ~~**TR22.** Does the game read the terrain object's fields directly?~~ **ANSWERED 2026-10-16 (re-answers30):
  DIFFERS** — §2.3's "otherwise internal" is wrong. Three external classes hold a `cISTETerrain*` and read fields
  directly: `cSTESurfaceWater` (water `+0x14`), `cSTEVolcanoLava` (lava `+0x14`) and `cSTETerrainView3D` (view
  `+0x20`). Full offset lists are in the `**Corrects (TR22)**` block of §2.3. The standout for `enable = terrain`
  is the view's direct read of the **vertex array at `+0x6C`**, and its two writes into the vertex `+0x22` flags
  (`LevelCell` `0x00759873`, `RemoveCellLevelling` `0x007586DA`). Nothing external reads `+0x24`.
- ~~**TR24.** Slots 25, 39, 58, 61, 62.~~ **ANSWERED 2026-10-16 (re-answers30):** §15.6.1. **PARTLY** — the names
  are broadly right but every rule was undefined, and slot 39 **DIFFERS** from the natural reading: it is an
  **"all sea"** test (`every altitude < seaLevel`), not a presence test, and it compares ground altitude rather than
  depth. Slot 25 uses `_ftol2` (truncate) where `GetAltitude` uses floor. Slot 58's table is 8 dwords at
  `0x00AB3C88` with no bounds check. Slots 61/62 index a per-level array by the **default brush level** `+0xBC`
  with no clamp.
- ~~**TR25.** Slots 43, 44, 46, 48, 56.~~ **ANSWERED 2026-10-16 (re-answers30):** §15.6.2. **DIFFERS** in four
  places. (a) **Quirk 19 is wrong**: slot 47 returns the tuning `0x48CD7B73` (shipped **40.0**), not a constant
  20.0 — the ini value is used. (b) `0x48CD7B72`/`73` are `ChangeSeaLevel`'s step and the default raise/lower
  amount, not "two editor values". (c) The height clamps are the globals `0x00B0D5E0`/`0x00B0D5DC` with fallbacks
  **5.0 / 200.0** — §6 and §4.2 quote the shipped 20/2500 as if absolute. (d) Slot 56 does far more than the
  documented +500 m dump: it snapshots depths to `+0xE0`, arms a `(20,20,40,40)` flood at `+0x19C`, and runs one
  `DoFloodFlowStep` immediately. Slots 43/44/46/48 themselves MATCH the spec's one-liners once expanded.
- ~~**TR26.** Surface water slot 7's five dwords.~~ **ANSWERED 2026-10-16 (re-answers30):** **PARTLY** — the outline
  MATCHES but the signature was wrong. It is
  `bool IsNavigableAtPosition(float x, float y, float z, int32 r, void* lot)` at `0x0073CEF0`, thiscall, **callee
  pops 20 bytes, returns in AL only**. **`y` is never read.** City slot 95 `LocationToCell` (`0x004599E0`) is
  `floor(coord × 0.0625)` per horizontal axis — plain world→cell, not `GetVertIndex`. *[quirk]* it passes `&arg1`
  as the cell-z out-parameter, clobbering the caller's `x`.
- ~~**TR27.** Does the surface water need its own message target and animation tick?~~ **ANSWERED 2026-10-16
  (re-answers30):** **MATCHES** §10.1 — yes to both: a `cIGZMessageTarget2` at water `+0x08` (vtable `0x00AB3AD8`,
  `DoMessage` `0x00740160`) subscribing to exactly those six messages, and `AddTickableObject` at **priority
  `0xC0`** (`0x0074001E`) — the guessed value is correct. **But DIFFERS on reachability**, and this is the answer
  glue18 needs: the tick body is gated on the water's byte **`+0x34`**, which nothing ever sets non-zero, so
  `AnimateWater` and `UpdateFloodWaters` **never run in the shipped binary**. **A surface water that does not
  animate is behaviour-compatible with the original**; starting to animate would be a *divergence*. Also
  **DIFFERS** on §10.3: the SimNewDay `0x66956814` trigger is **unreachable** (the handler exists at `0x0074019A`
  but `Init` never subscribes to it), so the dirty flag `+0x50` is set by the pole handlers and never read.


---

## 18. Conformance tests (must exist in `engine/conformance`)

1. **GetAltitude** (§4.1, §4.4): a 2 × 2-cell terrain with the §4.4 heights; the five queries of the table give 109,
   107, 110, 100 and 0. `GetAltitude(32, 32)` on a 2 × 2 city returns the vertex (2, 2).
2. **Floor at cell boundaries**: `x = 16.0` uses `ix = 1`; `x = 15.999` uses `ix = 0`; `x = sizeX` uses the last
   cell with `fx = 16`.
3. **GetFaceNormal** (§4.3): the §4.4 cell at (12, 4) gives `(−64, 256, −256)·s`; at (4, 12) gives
   `(−16·12, 256, −16·8)·s = (−192, 256, −128)·s`; `x = sizeX` leaves the output untouched.
4. **CellHasWater / CellHasLand**: depths (−1, −1, −1, 0.5) at the four corners → both true; (0, 0, 0, 0) → water
   false, land true; a NaN corner with the rest −1 → water false, land true.
5. **GetWaterDepthForCell**: depths (2, 3, 0.5, 4) → 0.5; (2, −1, 3, 3) → 0.0; `HasDraftClearance` with (6, 6, 6,
   5) → false (5.0 is not > 5.0).
6. **GetWaterType**: depth −2 → 0; depth 3 with altitude 247 and sea 250 → 3 (level 250 = sea); depth 3 with
   altitude 260 → 2.
7. **SetAltitudeAtVertex** (§6): sea 250, a land vertex at 260 with depth −10 set to 270 → depth stays −10; set to
   240 → depth 10; a water vertex alt 240 depth 10 set to 245 → depth 5; set to 255 → depth −5.
8. **SetAltitude(x, z, h)**: alt 260 → `SetAltitude(…, 3000)` clamps to 2500 and depth = −2250; alt 30 →
   `SetAltitude(…, 10)` lowers by 20 and clamps to 20; a locked vertex never changes.
9. **SetSeaLevel**: 201 and −10.5 are rejected (no message); 100 posts `0x89171496` and then `0x278E6003` with
   data4 0 over the whole city.
10. **RedisplayTerrain packing** (§8.1): rect (3, 5, 10, 12), data4 4 → `0x278E6003` with data1 `0x0003000A`,
    data2 `0x0005000C`, data3 0, data4 4, then `0x678E6E3A` with the same, then `0x870BC918`; all queued. Before
    `DoPostCityInit`: nothing.
11. **Pipe-burst sinkhole**: a flat terrain at 300, sea 250; `ApplyBrush(0x14)` at the corner of cell (10, 10) with
    the §9.5 parameters lowers no vertex below `300 − 255 × 0.01`, raises none, touches only vertices inside the
    16 m footprint centred on (160, 160) (§9.3), and posts one `0x278E6003` with data4 3 whose rectangle is the
    touched vertex rectangle.
12. **Unknown brush**: `ApplyBrush(0x12345678, p)` returns false, leaves `p`'s out rectangle empty and posts nothing.
13. **Lava flow rule** (§11.3, the per-vertex transfer in isolation): flat ground, a vertex with lava 10 whose eight
    neighbours all hold 1.4, dt = 33333 µs. All drops are 8.6, so the first edge neighbour in the offset order wins
    (a diagonal would need more than `√2 × 8.6`). `h = 4.3`, `rate = 4e-6 × 4.3 / 12 ≈ 1.4333e-6`,
    `amount ≈ 33333 × 4.3 × 1.4333e-6 ≈ 0.2054`. The neighbour goes from 1.4 to ≈ 1.6054, crossing 1.5, so its four
    cells that are flammable get `IgniteCell(…, false, true, 0)` and `SetCause(3)`. With the neighbours at 0
    instead, `amount ≈ 0.2778` and nothing is ignited. A vertex on the city edge with lava 10 loses
    `33333 × 1e-7 × 10 ≈ 0.0333` and ignites nothing.
14. **Records** (§13.1): a 64-cell city round-trips the four record kinds with the sizes 26 bytes, `2 + 65·65·4`,
    `2 + 65·65·2` and `2 + 4 + 65·4`.
15. **Lava neighbour order** (§11.3): flat ground, a centre vertex with lava 10. (a) W at 0 and the rest at 1 →
    W wins. (b) W at 10, N and E both at 2, the rest at 10 → **N** wins (k = 2 comes before E, k = 4). (c) Every edge
    neighbour at 10 and NE at 9 → NE wins (any lower diagonal beats a scaled best of 0). (d) N at 5 (drop 5), NW
    at 3 (drop 7 ≤ √2 × 5 ≈ 7.07) and the rest at 10 → N; with NW at 2 (drop 8) → NW.
16. **SetThingsOnFire** (§11.5): a vertex (5, 5) crossing 1.5 with all four cells flammable calls
    `IgniteCell(4, 4)`, `SetCause(3)`, `IgniteCell(4, 5)`, `SetCause(3)`, `IgniteCell(5, 4)`, `SetCause(3)`,
    `IgniteCell(5, 5)`, `SetCause(3)`, in that order; with only (5, 4) flammable, one `IgniteCell` and one
    `SetCause(3)`.
17. **GetWaterType with NaN depth** (§10.2) → 2. **AltitudeAtWhichFloodingWillOccur**: sea 250, the vertex dry, one
    wet neighbour at level 260 and one at 255 → 260; all neighbours dry → 250.
18. **Spans** (§9.3): the worked example (sinkhole, angle 0, square bitmap) touches only vertex (10, 10); a 45°
    rectangle gives rows whose spans follow the crossing columns, not the bounding box.
19. **Brush angles** (§9.1, §9.3): a fresh description's stroke angle is 0, so a first `ApplyBrush` with
    `from = to` uses angle 0; a later one with `from = to` reuses the last stroke angle.
20. **Default brush level** (§2.3): 0 before `DoPostCityInit` and with no camera.
21. **SetWaterSurfaceDisplayState** (§8.4): setting the current state posts nothing; a different state posts one
    `0x6AB97795` with data1 = the state.
22. **Kind 5** (§9.4.1): target 100, `b = 255`, strength 1 → step ≈ 0.996: alt 105 → 104.004; alt 100.5 → 100;
    alt 100 → no change, reported false.
23. **Sea level after load** (§13.1): `SetSeaLevel(100)`, save, load → `GetSeaLevel() = 250`.
24. **Bridge clearance** (§10.4 step 4, TR19): a bridge tile with passage ceiling 300, vertex alt 250 and depth 30
    → 300 − 280 = 20 → true; depth 30.5 → false; a bridge tile with no ceiling property → false.
25. **Pole cell** (§10.4 step 5, TR20): a pole whose own cell is (5, 5) but whose bounds reach (6, 5) blocks
    `IsNavigable(5, 5, 0, 0)` and not `IsNavigable(6, 5, 0, 0)`.
26. **Callback contract** (§9.4.6): a kind-0 component with a 1 x 1 bitmap whose single pixel is 255,
    neutral 128 and strength 1 reports one change and raises the vertex by 127; with the pixel equal to the neutral
    it reports none; with no bitmap the walker still calls the operation once per vertex of its rectangle.
27. **Kind 6, the fill pass** (§9.4.4): sea 250, a flat terrain at 300 with a dip to 230, radius 64 (so
    the rectangle is the centre vertex grown by 2), strength 1. `T = 250 - 15 = 235`; the vertex at 300 is left
    `depth = 0`, the one at 230 becomes `depth = 235 - 230 = 5`, and every sediment is zeroed. With strength 0 no
    iteration runs at all.
28. **Kind 8, lake** (§9.4.5): `target_height = 100`, `b = 0`, neutral 255, `strength[0] = 0.6` gives
    `amount = -153`, `target = -53`, `tol = 9.5625`. A dry vertex at 300 is lowered toward -53 in steps of at most
    `tol`, so one application takes it to `300 - 9.5625`; a wet vertex (`depth > 0`) with `target > alt` is
    **unchanged** and reports false; a vertex already at the target reports false.
29. **Kind 3, the scatter** (§9.4.3): on a terrain whose `+0xF8` is null nothing happens. With a manager, a
    component with `maxCount = 1` and `b = 1` places one object per cell and a second application to the same cell
    places none; the position is uniform in `[x*16, min(x*16+16, sizeX))` x `[z*16, min(z*16+16, sizeZ))`, the
    `aux` term is 0 when the component's `+0x34` is 0 or negative, and no vertex ever reports a change.
30. **Terrain ini** (§9.7): reading `WindErosion` / `TalusAngle = 5` from the shipped resource leaves
    `Tedge = tan(5 degrees) * 16` at `0x00B4BCF4` and `Tdiag = Tedge * sqrt(2)` at `0x00B4BCF0`; a missing key keeps
    the 0.79 rad fallback and its much larger thresholds. `RaiseLowerAmount` is stored and never read.
31. **Kind 9, rain** (§9.4.5): a single wet vertex at alt 240 with depth 1 (surface 241) and
    `strength = 6` raises the surface to 247, so `depth` becomes 7, capped at 200; a vertex whose surface is already
    above 247 is untouched. The out rectangle is the bounding box of the vertices actually raised.
32. **Kind 10, spring** (§9.4.5): `GodValleySpring` at a centre vertex adds exactly 1.0 to that vertex's
    water field `+0x04` and nothing else; the out rectangle stays empty, so no `0x278E6003` is posted.
33. **Kind 1, flora** (§9.4.2): with `b = 10` and the generator's first draw of
    `RandomUint32Uniform(100)` in 0..30, one flora occupant is created at a random point of the vertex's 16 m cell
    and no vertex joins the out rectangle; with `b = 9` nothing happens and no draw is made; with `depth > 0` nothing
    is placed.
