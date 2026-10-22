# Spec: shore ripples and terrain selection highlights (`cSTEWaterRipple`, `cSTESelectedTerrainArea`)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`). Implementers must work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled
or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[compat-limit]* depends on something FreeCity4 cannot reproduce, so only the rule is matched.

Related specs: [terrain.md](terrain.md) (§15, the terrain view's sub-objects: the selection at `+0x6C`, the ripples
at `+0x70`; the vertex flags), [texture-bindings.md](texture-bindings.md) (the ripple texture),
[render-properties.md](render-properties.md) (`kRenderWaterEffects`, `kPreferOpaque`, `kNoAddSigned`),
[INVENTORY-M6.md](INVENTORY-M6.md) (R5).

## 0. Summary

Both objects belong to the terrain's 3D view. The **ripples** are animated foam strips laid along every shore: the
shores are traced from the terrain's vertex flags, smoothed with a cubic B-spline and turned into triangle strips
from just inland to a wobbling distance out over the water, textured with an animated FSH whose frame advances at
random intervals. The **selection** highlights terrain vertices for tools: plain coloured cells and textured cells,
drawn over the terrain with a depth offset.

## 1. Ripples (`cSTEWaterRipple`) *[compat]*

### 1.1 Tuning (`ReadTuningParameters`, `0x00760060`)

Read once (the first `Init`) from the exemplar `0x6534284A / 0x88CD66E9 / 2` ("Ripple Properties", `SimCity_1.dat`).
"Per zoom" values are 5 entries, zoom 0–4.

| Property | Shipped | Use |
|---|---|---|
| `0x491C9C00` | true | ripples on |
| `0x491C9C01` | 50 | minimum shore length in points (at least 2); also the terrain-edit count that triggers a rebuild (§1.5) |
| `0x491C9C02` | 16 ×5 | per zoom: spacing between ripple points (its square is compared); the spline step is it ÷ 64 |
| `0x491C9C03` | 0.3 | the ripple height above the water level |
| `0x491C9C04` | 0.15, 0.3, 0.3, 0.6, 0.75 | per zoom: texture advance per segment |
| `0x491C9C05` | 45, 40, 35, 30, 26 | per zoom: the outer distance (m) |
| `0x491C9C06` | 1 ×5 | per zoom: the inner distance (m, inland) |
| `0x491C9C07` | 175000 | frame period; frames last a random 0.7–1.3 of it (122500–227500) *[inferred: µs]* |
| `0x491C9C08`, `0x491C9C09` | 100, 150, 200, 255, 255 | per zoom: the inner and outer alpha |
| `0x491C9C0A` | 0.2 ×5 | per zoom: the wobble as a fraction of the outer distance |
| `0x491C9C0B` | 60 ×5 | per zoom: the largest turn between segments (degrees) |
| `0x491C9C0C` | 12 | terrain-edit area that starts a check (§1.5) |
| `0x491C9C0D` | 6 | the number of point pairs of the fade at each end |

### 1.2 Shores (`DetectShores`, `0x00761B70`)

Every terrain vertex with flag `0x10` set and flag `0x01` clear starts a shore trace (terrain helpers `0x0074B570`);
a shore with more points than `0x491C9C01` is kept (`0x00761A00`). The terrain's edit counter (terrain slot 54) is
remembered.

### 1.3 Strips (`CreateRipples` `0x00761CF0`, `CreateRipplesForShore` `0x00761140`)

For each shore, the parameter walks the shore's points in steps of `0x491C9C02` ÷ 64 (zoom-dependent). At each step
(`ComputeSplinePoints`, `0x0075FC40`) the uniform cubic B-spline of four consecutive shore points gives a point P
and a unit tangent; with N the tangent turned a quarter, the **outer** vertex is P + (D + w sin φ) N and the
**inner** vertex P − d N, with D = `0x491C9C05`, w = D × `0x491C9C0A`, d = `0x491C9C06` for the zoom, and φ
growing by 0.1 per step. N = (tangent z, −tangent x) *[inferred: it points over the water for the traced shore
direction]*.

A strip (`StartNewRipple`, `0x00760EE0`) collects pairs: a new outer or inner point is kept only when it is at
least the spacing from the previous one and the turn from the previous segment is at most `0x491C9C0B`; the texture
coordinate grows by `0x491C9C04` per kept pair; a point too close to the previous one is simply skipped. A strip
ends, and the next starts, when a point comes within 0.2 m of the city's edge, when the turn is too sharp, or after
1000 pairs; strips of fewer than 8 vertices are dropped.

Each strip then (`PrepareRippleForDisplay`, `0x0075FEE0`) gets a random start frame, a random frame period, an even
vertex count, and alpha ramps over the first and last min(pairs / 2, `0x491C9C0D`) pairs up to the zoom's inner and
outer alpha.

The texture is the FSH (`0x7AB50E44`, `0x891B0E1A`, `0xE9A44A88`) bound through the texture binding factory (slot
6); without it no strip is kept.

### 1.4 Drawing (`Update`, `0x00761FF0`; `DrawRipple`, `0x0075FE70`)

The first `Update` detects the shores and makes the strips. Strips are drawn only when the draw context's render
properties have `kRenderWaterEffects` (boolean 3) set, with an alpha-blended render state; a zoom change
rebuilds the strips for the new zoom. Each shore and each strip is tested against the view frustum; a strip is drawn
as one triangle strip with its current frame's texture.

### 1.5 Updates (`DoMessage`, `0x00761F80`)

- `0x86AD10EE` (animation tick with the elapsed time; `DoAnimationTick` `0x00761E70`): each strip's clock advances;
  past its period the frame advances (wrapping at the texture's frame count) and a new random period is drawn. When
  no rebuild is pending and the terrain's edit counter has grown by more than 50² / 10 = 250, a rebuild is armed; an
  armed rebuild happens after 2,000,000 time units *[inferred: 2 s]*.
- `0x278E6003` terrain changed (`DoOnTerrainEdit` `0x00760E10`): the changed area is added up; past
  `0x491C9C0C` (12) the sum restarts and the rectangle is checked: strips it touches are removed and a rebuild is
  armed; otherwise the rectangle is merged into a dirty rectangle and a rebuild is armed once that covers more than 400
  cells.
- `0x89171496`: rebuild at once.

## 2. Selection highlights (`cSTESelectedTerrainArea`) *[compat]*

### 2.1 State

Two tables keyed by vertex index (z × (cells x + 1) + x): **plain** cells (a 32-bit colour) and **textured** cells (a
texture key, two bytes, a byte, an RGB colour, an alpha, a mode). Putting a cell in one table removes it from the
other. `Init` (`0x00750AA0`) takes the terrain, its cell counts and the texture binding factory, and builds the render
states (§2.3).

### 2.2 Marking (`MarkSelected`, five forms)

The last argument of each form clears both tables first (the rectangle form then also rebuilds the render states if
`kPreferOpaque` changed).

| VA | Arguments | Effect |
|---|---|---|
| `0x007516D0` | (cell list, colour, clear) | the cells become plain cells of that colour |
| `0x00751780` | (rectangle, colour, clear) | every vertex of the inclusive rectangle becomes a plain cell of that colour |
| `0x00751910` | (cell, colour, clear) | one plain cell |
| `0x007519A0` | (cell, key, b1, b2, palette index, b3, clear) | a textured cell coloured from the palette `0x00AB4238` (0 red, 1 green, 2 blue, 3 (0.4, 0.3, 0), 4 black), alpha 1, mode 2 |
| `0x00751A70` | (cell, key, b1, b2, packed colour `0xRRGGBBxx`, mode, clear) | a textured cell of that colour; mode 3 with `kNoAddSigned` becomes mode 1 with alpha 0.5 |

The palette index is not range-checked: indices above 4 read unrelated constants *[quirk]*. `ClearCurrentSelections`
(`0x00750CA0`) empties both tables.

### 2.3 Drawing (`Update`, `0x007514F0`)

When either table has cells: depth offset 4, the plain cells (`DrawUntexturedSelections` `0x00750FC0`), then the
textured ones (`DrawTexturedSelections` `0x00750CF0`), then the previous depth offset. The four render states
(`InitRenderState` `0x00750600`) are opaque or blended variants chosen by `kPreferOpaque` (boolean 33), rebuilt when
that property changes.

## 3. Users

The terrain's 3D view creates both (terrain.md §15); the tools mark selections (zone, lot and terrain tools); the
animation tick drives the ripples.

## 4. Quirks

1. The palette index of §2.2 is not checked.
2. `0x491C9C01` serves both as the minimum shore length and as the base of the edit threshold (§1.5).
3. A failed texture leaves the shores with no strips until the next rebuild (§1.3).

## 5. Open questions

See [questions/water-ripples.md](questions/water-ripples.md) WR1–WR3: the time unit of the tick and the periods, the
terrain flag meanings `0x10` / `0x01` (shore and water *[inferred]*), and the bytes b1–b3 of a textured selection.

## 6. Conformance tests

1. **Spline:** with points (0, 0), (16, 0), (32, 0), (48, 0) and t = 0, P = (16, 0) and the tangent is +x.
2. **Strip width:** at zoom 4, φ = 0: the outer vertex is 26 m out, the inner 1 m in.
3. **Spacing:** two candidate outer points 10 m apart at zoom 0 (spacing 16): the second is not kept.
4. **Turn:** a 70° turn between segments ends the strip at zoom 2.
5. **Short shore:** a shore of 40 points gets no ripples.
6. **Frames:** a strip whose period is 150000 advances one frame after ticks totalling 150000.
7. **Selection:** marking a cell plain after textured leaves it only in the plain table.
8. **Palette:** index 3 gives (0.4, 0.3, 0) with alpha 1 and mode 2.
