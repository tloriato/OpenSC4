# Spec: the terrain's 3D view, drawing (`cSTETerrainView3D`)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`). Implementers must work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled
or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[compat-limit]* depends on something FreeCity4 cannot reproduce, so only the rule is matched.

Related specs: [terrain.md](terrain.md) §15.5 (who creates the view, its layout, its five vtables, `Init`, what it
reads off the terrain, the vertex record) and [view3d.md](view3d.md) §6 (identity, `GetView`, the interface tables)
are **not restated here**; this spec covers what the view does once it is up. Also
[draw-context.md](draw-context.md) (every draw call), [view-utilities.md](view-utilities.md) (the frustums and
ground lines), [lighting.md](lighting.md) §4.5 (the two vertex colours), [water-ripples.md](water-ripples.md) (the
selection and ripple sub-objects), [render-properties.md](render-properties.md).

Offsets are from the object base (terrain.md §15.5.2); methods of the `+0x0C` interface receive `base + 0x0C`.

## 0. Summary

The terrain is drawn as one grid of 16 m cells, row by vertex row, with only the rows and columns inside the draw
frustum and above its lower plane. Land is drawn twice: once untextured in a "floor" colour, then once per terrain
texture, added with weights that each vertex carries for up to three textures. Water is a second, alpha-faded grid
with a colour from its depth; the city's edges show stratified side cliffs and, where there is sea, the sides of a
water box. The view also draws the cell grid and the plumbing overlay, levels single cells into flat quads, picks
cells under a ray and keeps the lighting and water colours up to date as the terrain, the view and the time of day
change.

## 1. Tuning (`ReadTuningParameters`, `0x00758990`) *[compat]*

Read once (by `StaticInit`) from the exemplar `0x6534284A / 0x88CD66E9 / 1` ("Terrain Properties",
`SimCity_1.dat`):

| Property | Shipped | Use |
|---|---|---|
| `0x89A312C0` | 0.2 | texture scale: × 0.01 per metre, × 2^zoom (§5.1) |
| `0x29AB15C0` | 30 | water depth (m) at which the water becomes opaque (§6.2) |
| `0x29AB15C1` | 0 | the water's alpha at the shore (byte) |
| `0x29AB15C2` | 0.75 | water colour ramp pixels per metre of depth (§6.2) |
| `0x29AB15C3` | true | draw the water |
| `0x29AB15C4` | true | draw the land surface |
| `0x29AB15C5` | 0.1, 0.2, 0.8 | the water tint at full strength (RGB, §6.2) |
| `0x29AB15C6` | 55, 65 | the base depths (m) of the two cliff strata (§7.1); their reciprocals are the strata texture scales, with 1/64 for the third |
| `0x29AB15C7` | 0.9 | a weight used for the cliff colours (`0x007558B0`) *[inferred]* |

The plumbing overlay's colours come from the same exemplar (`ReadPlumbingTuningData`, `0x00755C50`): `0x497F42A4`
(182, 47, 0, 102), `0xE97F42B0` (0, 18, 255, 102), `0xA97F42CE` (125, 131, 255, 102), `0x097F4C4E`
(182, 119, 116, 102), and the flag `0x297F42B7` (true).

## 2. Process-wide state (`StaticInit`, `0x0075C0C0`) *[compat]*

Once per process: the tuning; the texture binding factory; the per-zoom tables of terrain textures (five zooms,
filled on demand, §5.1), the per-zoom water texture and a second per-zoom texture; two FSH images (`0x7AB50E44 /
0x891B0E1A`, instances `0xC9EFB727` and `0x69EFB6F8`) used as colour ramps (§6) and a glare image; a 256-entry span
table. The cell geometry (terrain.md §15.5.4 step 6) and the buffers below are static too:

- the **land** and **water vertex arrays**, one 0x20-byte vertex per terrain vertex: position (x, y, z), colour
  (BGRA bytes at `+0x0C`), texture coordinates at `+0x10`;
- per-row **index lists**, six 16-bit indices a cell (§4.2), and the per-row visible column ranges (§3);
- for each of the four city edges, three cliff strata strips and one water-box strip (§7).

`0x00758BC0` frees them all at shutdown and releases the textures and images.

## 3. Culling (`ComputeDrawFrustumOptimizationInfo`, `0x00756750`) *[compat]*

Steps 1–3 run at the start of every land pass; the same computation for the view frustum (step 1's second table and
`0x007539D0`) only when the view has changed (flag `+0xB8`, set by the view and height messages, §9):

1. **Draw rows** (`ComputeDrawRectsForDrawFrustum`, `0x00752E80`): the vertex rows are those between the draw
   frustum's city-clipped box (view-utilities.md §3.3, §5) divided by 16, clamped to the grid. For each, the
   columns are between where the row's line z = 16 r meets the draw frustum's two ground lines, rounded outwards
   (÷ 16), clamped to 0…vertsX. The union over all rows is kept (`+0xB0`, `+0xB4`), and each row's range is then
   widened to include the next row's, so a cell row covers both of its vertex rows. `0x00753210` does the same for
   the view frustum (another table, `+0xBC`–`+0xE4`).
2. **Visible edges**: the two city edges facing the camera (view-utilities.md §4.2) are kept at `+0x5C`, `+0x60`.
3. **Heights** (`ComputeHeightBasedCullingInfoForDrawFrustum`, `0x007535A0`): for each draw row, the lowest
   visible height is where the draw frustum's lower plane passes over the row's two ends (the smaller of the two,
   `ComputeMinimumVisibleHtAtDrawRows` `0x00751EE0`). The row's **land** is drawn when its highest land vertex
   reaches that height; its **water** when its highest water vertex reaches it and is above the row's lowest land.
   The first and last rows with land, with water, and with either are kept (`+0x98`–`+0x9C`, `+0xA4`–`+0xA8`,
   `+0x8C`–`+0x94`), each widened by one row before and two after.

A cell row is drawn when either of its two vertex rows passed.

## 4. Land *[compat]*

### 4.1 The frame (`Update`, `0x0075BFD0`; `DrawTerrainLayers`, `0x0075BEE0`)

`Update(drawContext)` does nothing unless `kRenderTerrain` (boolean 15) is set. It takes the terrain's maximum and
minimum altitudes (`+0x64`, `+0x68`), a vertex-buffer extension when the driver offers one (`cSTEVertBufExtn`,
which only changes how rows are submitted), depth offset −3, and a fixed packed state (colour and depth write, depth
test less-or-equal, culling, one texture, no blending or alpha test). The draw context's `+0x50` selects the pass
*[inferred: the draw registry's pass]*: 0 draws the land layers, anything else the water layers (§6). The depth
offset is restored after; with the plumbing display on (the static `0x00B4BC5C`, terrain slots 85–86) the land
pass ends with the plumbing overlay (§8.2).

`DrawTerrainLayers`: culling (§3); a terrain-type change reloads the textures (§5.1); a zoom or
rotation change (§9.2); a pending lighting rectangle is sent to the terrain (slot 67) and the cliff colours
recomputed (`0x007558B0`); then the land surface (when `0x29AB15C4`), the side cliffs (§7.1) and, with the display
grid on, the grid (§8.1), restoring the fixed state.

### 4.2 Triangles (`SetTriangleIndicesForCell`, `0x00752AF0`)

Each cell (x, z) has two triangles over its vertices v = (x, z), v + 1, v + W and v + W + 1 (W = vertsX), chosen by
the flags of vertex (x, z) (terrain.md §7):

| Flags | First triangle | Second triangle |
|---|---|---|
| no flip (`0x3000` clear) | (v, v + W, v + W + 1) | (v, v + W + 1, v + 1) |
| flipped | (v, v + W, v + 1) | (v + 1, v + W, v + W + 1) |

`0x0020` hides the first triangle and `0x0040` the second (all three indices v); `0x0200` reverses the order of
both. A cell's indices are rebuilt whenever its flags change (§7.3, §9).

### 4.3 The two passes (`DrawTerrainSurface`, `0x0075BD10`)

1. **Floor**: every visible vertex takes lighting.md §4.5's second colour (the vertex record's bytes `+0x13`–`+0x15`),
   texturing is off, and each visible cell row is drawn as one indexed list
   (`DrawTextureLessTerrainSurface` `0x00752100`).
2. **Textures**: the vertices take the first colour (`+0x10`–`+0x12`); blending (source alpha, one), texturing
   with mode 1, filters linear / linear-mipmap-linear, wrap repeat (`DrawTexturedTerrainSurface` `0x0075BA90`,
   `DrawTerrainSurfaceRows` `0x0075B460`). Each vertex record holds up to three (texture key, weight) pairs at
   `+0x16`, `+0x1A`, `+0x1E` (a 16-bit key, a weight byte). Per row, the spans of cells that use each key are
   cached (`ComputeTextureSpansForRow` `0x0075ABA0`, recomputed when the row is marked dirty); for each key the
   texture is bound (§5.1) and every vertex's alpha := its weight for that key (0 when it has none) before the
   spans are drawn. So the visible colour is floor + Σ weight × lit colour × texture.

Blending is turned off at the end.

### 4.4 Single quads

`DrawTerrainQuad(drawContext, x, z)` (slot 14, `0x00752870`) redraws one visible cell from the land arrays (its
row's index list, flipped or not); `DrawTexturedTerrainQuad(drawContext, x, z, uv[8], alpha, force)` (slot 15,
`0x00752940`) draws the cell's four vertices with the given texture coordinates and one alpha byte, even when the
cell is not visible if `force` is set. Used for overlays drawn on single cells *[inferred]*.

## 5. Texture coordinates and textures *[compat]*

### 5.1 Per zoom

- Coordinates (`0x00751C80`): for every land and water vertex, (u, v) = f × (x, z) at rotation 0, f × (z, X − x) at
  1, f × (X − x, Z − z) at 2 and f × (Z − z, x) at 3, with f = `0x89A312C0` × 0.01 × 2^zoom (1, 2, 4, 8, 16 for
  zooms 0–4) and X, Z the city's size in metres; recomputed when the zoom or rotation changes.
- Textures (`0x007571D0`): for the zoom, every texture of the terrain type's list is bound from FSH type
  `0x7AB50E44`, group `0x891B0E1A`, instance key + 0x100 × zoom, else the key itself; a texture key's bits 12–15
  choose one of up to 16 variants. `ChangeTerrainType` (`0x00756CA0`) releases every zoom's textures, and the next
  draw reloads them for the new type.

## 6. Water *[compat]*

### 6.1 Heights (`0x00756B00`, on `0x678E6E3A`)

In the changed rectangle each water vertex's height := altitude + depth where the depth (surface water array,
terrain.md §2.2) is positive, else the sea level; the water-box strips follow along the edges, coloured from a ramp
image where the water stands above the box bottom; the rectangle joins the water-colour update
rectangle.

### 6.2 Colour (`SetColorOfWaterSurface`, `0x00756D20`)

Over the pending water-colour rectangle (`+0x4C`, empty flag `+0x48`): each vertex's colour is the depth ramp's
pixel at index depth × `0x29AB15C2` (clamped to the image), times the tint white + k × (`0x29AB15C5` − white), k
being the value kept from the last light-change message (§9.2) *[inferred: darker, bluer water at night]*, and
mixed with the glare image's pixel when water glare is on (slot 12, `+0x49`) and `kRenderSnapshot` (boolean 28) is
off, the rotation-adjusted cell position picking the glare pixel. Its alpha: depth ≥ `0x29AB15C0` (30 m)
gives 255; 0 < depth < 30 gives `0x29AB15C1` + depth × (255 − `0x29AB15C1`) / 30; depth ≤ 0 gives `0x29AB15C1`.
With `kRenderSnapshot` every alpha is 255. Then the rectangle is empty again.

### 6.3 Drawing (`DrawWaterLayers` `0x007572A0`, `DrawWaterBox` `0x007551C0`, `DrawWaterSurface` `0x00752240`)

Only when the plumbing display is off, the terrain's water surface display state is on (slot 83) and `0x29AB15C3`:
the pending colours (§6.2); the **water box** when the sea level is at least the terrain's minimum altitude (all four
edges, untextured, no culling, depth less, depth offset −1 on the two visible edges and −5 on the others, each edge
drawn only where its top is above its bottom and inside the draw frustum's ground lines); then the **surface**:
blending source alpha / one minus source alpha, mode 1, the zoom's water texture, depth function less, the visible
water rows (§3). Depth function and blending are restored.

## 7. City edges *[compat]*

### 7.1 Side cliffs (`DrawSideCliffs`, `0x00753E00`)

Only the two visible edges, with depth offset 1. An edge strip has, at each edge vertex, three strata: the top from
the terrain height h down to h − d₁, the next down to h − d₁ − d₂ (each clamped at 0), the last to 0
(`0x00755B60`), where d₁ and d₂ come from two 257-sample profile images (FSH `0x7AB50E44 / 0x891B0E1A /
0xCA5CC9C0` and `0xCA5CC9C1`): d = (pixel − 127) × 0.5 + 55 m or 65 m (`InitializeCityEdgeLayerHeights`,
`0x00757300`). Each stratum is drawn with its zoom's cliff texture, piece by piece where its top is above its
bottom, within the part of the edge inside the draw frustum's ground lines. The cliff vertices take the lighting
manager's light for the edge's outward normal ((0, 0, 1), (1, 0, 0), (0, 0, −1), (−1, 0, 0)), darker below 70 m
(`0x007558B0`) *[inferred]*.

### 7.2 Heights (`0x007569F0`, on `0x278E6003`)

In the changed rectangle each land vertex takes the terrain altitude, and vertices on a city edge update that edge's
strata (§7.1).

### 7.3 Levelled cells (`LevelCell` slot 16 `0x00759750`, `RemoveCellLevelling` slot 17 `0x00758620`)

`LevelCell(x, z, height)` turns the cell into a separate **flat quad** at that height (kept per row, ordered by x):
the quad copies the cell's four land vertices with y := height; both terrain triangles of the cell are hidden
(flags `|= 0x60`) and its indices rebuilt; levelling an already levelled cell only changes its height.
`RemoveCellLevelling(x, z)` removes the quad, clears `0x60` and rebuilds the indices. Out-of-grid cells are ignored.
The quads are drawn with the land passes *[inferred]*.

## 8. Overlays *[compat]*

### 8.1 The cell grid (`0x00757650`)

With the display grid on (the static `0x00B4C741`, slots 20–21; saved and restored with the preferences byte
`0x00B43CD8 + 1`) and a grid texture (`+0x88`), not under `kRenderSnapshot`: a blended (source alpha / one minus
source alpha) pass of the grid texture without depth write, filters linear / linear-mipmap-linear, wrap repeat.
`SetDisplayGridFlag` (`0x00755E80`) redisplays the whole terrain (terrain slot 68) on a change; message
`0x277A9364` toggles it.

### 8.2 Plumbing (`DrawPlumbingInfo`, `0x00759A80`)

With the plumbing display on: blending, no depth write, no texture, depth offset −2, the pipe coverage drawn from
the plumbing simulator (`0x00B43D94`) with the four colours of §1 *[inferred: by coverage state]*.

## 9. Picking, selection and updates *[compat]*

### 9.1 `Pick(origin, direction, &cell, &point, withWater)` (slot 3, `0x0075A230`)

The ray is clipped to the city's box; from the entry point (moved 4 m towards the city's centre) a cell walk
follows the ray's ground direction for up to 768 steps. At each step the three cells across the walk's minor axis
are tested: each cell's two triangles (diagonal as §4.2) at the land altitudes, or with `withWater` the altitudes
plus the positive water depth. The first hit ends the walk with true (the hit point from the triangle test); with
the automatic cell cursor on (slot 10, byte `+0x30`) that cell is also marked as a plain selection with colour value
2 (water-ripples.md §2.2). The object-base
`Pick` (`0x00752700`) calls it without water.

### 9.2 Updates

- `SetLightingUpdateRect(rect)` (slot 13, `0x00755F30`): the rectangle (or the whole city for null) replaces the
  pending lighting rectangle (`+0x34`) when it is empty (`+0x31`), else is merged into it (`0x004665C0` merges two
  rectangles). The water-colour rectangle is kept the same way (`0x00756000`).
- `SetWaterGlareStatus(on)` (slot 12, `0x00756080`): on a change, the whole city's water colours are due.
- A zoom change (`0x007574D0`) loads the zoom's textures and makes the lighting and water colours of the whole city
  due; a rotation change (`0x007566C0`) does the same without textures; both recompute the texture coordinates.
- `DoMessage` (`0x00757560`): `0x277A9364` toggles the grid; `0x278E6003` (heights) and `0x678E6E3A` (water) rebuild
  their rectangle (§7.2, §6.1) and recompute culling; `0xA6B79601` and `0xA6B79611` (view changes) recompute
  culling; `0xC9DA96EA` (the dawn and dusk light change, lighting.md) makes the whole city's lighting and water
  colours due and keeps the message's value at `+0x44`.
- `HandleCheatCommand(command)` (slot 19, `0x0075A770`): `level x z height` calls `LevelCell(x, z, height)` and
  `unlevel x z` calls `RemoveCellLevelling(x, z)`; other words are not handled.
- `ClearCurrentSelections` and the five `MarkSelected` forms (slots 4–9) pass through to the selection
  (water-ripples.md §2.2) when it exists.

### 9.3 `Shutdown` (`0x007560F0`)

Unregisters the view, its overlays and sub-objects from the draw registry and the six message subscriptions, shuts
down and releases the overlay managers, the ripples, the selection and the `+0x74` object, releases the city,
terrain, surface water and the `+0x28`, `+0x2C` collaborators, and writes the display grid flag back to the
preferences byte.

## 10. Users

The terrain creates the view (terrain.md §15.5); the 3D renderer's draw registry calls `Update` (view3d.md §4); the
zone, lot and terrain tools pick cells and mark selections; the cheat console sends `level` / `unlevel`; the
terrain's messages and the lighting manager keep the arrays current.

## 11. Quirks

1. Land is drawn twice and the textures are added with weights, not blended over each other (§4.3).
2. The water box draws all four edges, the cliffs only the two facing the camera (§6.3, §7.1).
3. Snapshots draw the water opaque and without the grid (§6.2, §8.1).
4. The cursor marking is a side effect of `Pick` (§9.1).

## 12. Open questions

See [questions/terrain-view.md](questions/terrain-view.md) TV1–TV3: what the draw context's `+0x50` pass value is
and who sets it; the meaning of the four plumbing colours and of `0x29AB15C2`, `0x29AB15C5`, `0x29AB15C7`; what the
object at `+0x74` is.

## 13. Conformance tests

1. **Triangles:** a cell without flags gives (v, v + W, v + W + 1) and (v, v + W + 1, v + 1); with `0x1000`, the other
   diagonal.
2. **Hidden:** flag `0x0020` makes the first triangle (v, v, v).
3. **Texture scale:** at zoom 4 and rotation 0, the vertex at x = 100 m has u = 100 × 0.002 × 16 = 3.2.
4. **Water alpha:** with the shipped tuning, depth 15 m gives alpha 128 (127.5 rounded to even), 30 m and more 255,
   land 0.
5. **Strata:** a profile pixel of 127 gives the first stratum 55 m deep.
6. **Levelling:** `level 10 12 50` gives cell (10, 12) a flat quad at 50 m and hides its two triangles; `unlevel
   10 12` restores them.
7. **Culling:** a row entirely below the draw frustum's lower plane is not drawn.
8. **Render property:** with `kRenderTerrain` off, `Update` draws nothing.
