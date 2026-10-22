# Spec: lot render models (`cSC4FoundationModel`, `cSC4LotRetainingWallModel`, `cSC4CellTextureModel`)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`). Implementers must work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled
or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[compat-limit]* depends on something FreeCity4 cannot reproduce, so only the rule is matched.

Related specs: [building-development.md](building-development.md) (the foundation and retaining-wall occupants,
their types and properties; DG35), [terrain-view.md](terrain-view.md) (§4.4 `DrawTexturedTerrainQuad`, which draws
sloped cell textures), [draw-context.md](draw-context.md), [lighting.md](lighting.md) (the light colours),
[view-utilities.md](view-utilities.md) (§4.2 visible edges), [water-ripples.md](water-ripples.md) (§2.2, whose
textured-cell modes the cell texture repeats).

## 0. Summary

Three small model instances draw the parts of a lot that are not building models: the **foundation** box under a
building on a slope, the **retaining walls** along a lot's edges where the terrain falls away or rises, and the
**cell textures**, one textured square per lot cell. Each keeps its own vertices, rebuilds them when the zoom or
rotation changes, takes its texture per zoom with a fallback key, and draws with a fixed opaque state.

## 1. Common behaviour *[compat]*

- All three derive from `cS3DModelInstanceBasic` (as a virtual base; pool-allocated, freed to the pool by the
  deleting destructors) and answer one IID each besides the basic ones: `0xA9E2831D` (cell texture, `0x005DEB00`),
  `0xA9E2831E` (retaining wall, `0x005E1200`).
- **Sort value** (`GetSortValue`): the current texture's ID OR a class constant, so models sharing a texture draw
  together; without a texture a fixed value. Foundation: `| 0x2AB6`, else `0xE9862AB6` (`0x005D5570`); wall:
  `| 0x2A86`, else `0xC9862A86` (`0x005E14F0`).
- **Zoom changes** load the zoom's texture through the texture binding factory; when the key for that zoom is
  missing, the key is replaced by a fixed per-zoom fallback and loaded again (below).
- The draw state, made once from the 3D view manager's draw context, is colour and depth write, depth test
  less-or-equal, one texture, no alpha test, no blending, no culling.

## 2. Foundations (`cSC4FoundationModel`) *[compat]*

- Constructor `0x005D5D70`; destructor body `0x005D5300`.
- **Shape** (`SetupVertices(geometry, colours, lighting)`, `0x005D55C0`): a box over the occupant's footprint from
  the base height to the top height: four side faces of four vertices and a top face. The sides' texture
  coordinates count one repeat per 16 m; the top's divide the footprint by the top texture's scale (`+0x58`). With
  `colours`, each side takes the lighting manager's light for its outward normal (`GetVectorForEdge`, lighting
  slot 20), the top the light for (0, 1, 0).
- **Height** (`AdjustHeightToTerrain`, `0x005D5A90`): along the footprint's border vertices, the largest drop from
  the top to the ground, or to the water surface where that is higher; when that needs more whole 16 m steps n than
  the box has *[inferred: rounded up]*, the base is lowered to top − 16 n and the box rebuilt (or rebuilt anyway
  when asked). The base never moves up here.
- **Textures** (`0x005D5C70`): a side texture and a top texture per zoom; a missing side key falls back to
  `0x69F20000 + zoom`, a missing top key to `0x0912220F + zoom`.
- **Draw** (`0x005D5FC0`): not when highlighted. A rotation change re-takes the two visible edges (view-utilities
  §4.2) and recolours; a zoom change reloads the textures. With a side texture, the two visible sides are drawn as
  four-vertex primitives of type 6 (quads, graphics-formats.md §3.7); then the top with the top texture.

## 3. Retaining walls (`cSC4LotRetainingWallModel`) *[compat]*

- Constructor `0x005E17C0`; destructor body `0x005E1A60`. `Init(box, side, lot)` (`0x005E2780`): the wall's cell
  range along the side (0–3) of the lot's box (`SetupVerticesFromBBoxAndWallEdge`, `0x005E1520`), then its vertices.
- **Shape** (`SetupVertices`, `0x005E1BC0`): the lot's flat altitude (the lot at the wall's inner cell, lot slot 16)
  against the terrain altitude at each vertex along the edge (terrain slot 14). For each 16 m segment the wall
  fills the gap between the lot level and the terrain, above or below, as triangles (position, colour, texture
  coordinates) set 0.001–0.002 m out from the edge, with the top and bottom carried 0.2 m past the lot level and the
  terrain to close the seams, and v = height × the wall's texture scale; segments where the terrain meets the lot
  level get nothing.
- **Textures** (`SetupTextures`, `0x005E1620`; `DoZoomChange`, `0x005E1710`): the zoom's key from FSH group
  `0x891B0E1A`; a missing key falls back to `0x695C00C6 + zoom` — the five shipped wall textures `0x695C00C6`–
  `0x695C00CA` of the Terrain Properties exemplar's `0x295961F2` (terrain-view.md §1).
- **Draw** (`0x005E27C0`): not when highlighted. A zoom change reloads the texture; a rotation change recolours.
  With the facing test on (`+0x6A`), a wall is drawn only when its side is the rotation or the rotation + 3 (mod 4),
  the sides facing the camera *[inferred]*; then one triangle list of its vertices.
- `0x005E1240` tells whether the lot on the cell across the wall's edge is flat (lot flag bit 0) *[inferred: to skip
  walls between flat lots]*.

## 4. Cell textures (`cSC4CellTextureModel`) *[compat]*

- Constructor `0x005DF880`; destructor body `0x005DEAB0`. A record (`0x005DF9A0`): the texture key, the cell
  (x, z), the orientation (0–3), a mirror flag, a colour (RGB), an alpha and a mode.
- **Texture** (`0x005DF020`): FSH type `0x7AB50E44`, group `0x0986135E`, instance key + zoom, else the key; the
  texture's own alpha flag is kept.
- **Corners** (`0x005DEEA0`): the four texture corners are rotated by the orientation and mirrored in u by the flag.
- **Place** (`0x005DF9E0`): the cell is 16 m square at (16 x, 16 z). When the lot on that cell is flat (lot manager
  slot 6, the lot's flag bit 0), the four corners take the lot's altitude and the lighting manager's colour for each
  corner (slot 19, normal (0, 1, 0)); otherwise the terrain's corner altitudes (terrain slot 17) and no colours. The
  bounds are the square inset by 0.1 m and the height range widened by 2 m.
- **Init** (`0x005DF100`): three draw states: opaque; alpha (alpha test "greater" at 0, blend source alpha / one
  minus source alpha); tinted (blending, no depth write, depth less); and a combiner record.
- **Draw** (`0x005E0200`): not when highlighted. A zoom change reloads the texture; the mode is **tinted** when the
  record's mode is set, else **alpha** when the texture has alpha or the alpha is not 255, else **opaque**; alpha and
  tinted cells are skipped while the draw context's `+0x04` is 0 *[inferred: drawn in the translucent pass]*. Wrap
  clamp. Tinted: linear filtering, the environment colour := the record's RGB × the global light ÷ 255 and
  alpha ÷ 255, the combiner with the record's mode, where mode 3 with `kNoAddSigned` becomes mode 1 at half the
  alpha. On a flat lot the model draws its own four-vertex strip; on a slope it asks the terrain view to draw the
  cell (`DrawTexturedTerrainQuad`, terrain-view.md §4.4) with its corners and alpha, forced when the draw context's
  `+0x20` is 8.

## 5. Users

The building developer creates the foundation and wall occupants (building-development.md) and the lot developer
the cell textures; the model maker inserts their models; the 3D renderer draws them in the static pass.

## 6. Quirks

1. Foundation bases move in whole 16 m steps (§2).
2. Walls overshoot the lot level and the terrain by 0.2 m (§3).
3. A cell texture on a slope is drawn by the terrain view, not by itself (§4).

## 7. Open questions

See [questions/lot-models.md](questions/lot-models.md) LM1: what the cell texture record's mode values mean
beyond 3 and who sets the colour.

## 8. Conformance tests

1. **Foundation steps:** top 40 m, lowest border ground 21 m: base at 40 − 32 = 8 m (two steps).
2. **Fallback:** a foundation at zoom 3 without a side key uses `0x69F20003`.
3. **Wall texture:** a wall at zoom 4 without a key uses `0x695C00CA`.
4. **Facing:** at rotation 1 a wall on side 1 or side 0 is drawn, one on side 2 is not.
5. **Cell mode:** a record with alpha 128 and no mode draws with the alpha state.
6. **Orientation:** orientation 1 rotates the corners one step; the mirror flag swaps u for 1 − u.
