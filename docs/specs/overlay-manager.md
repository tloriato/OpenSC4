# Spec: terrain overlays, decals and shadows (`cSTEOverlayManager`)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`). Implementers must work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled
or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[compat-limit]* depends on something FreeCity4 cannot reproduce, so only the rule is matched.

Related specs: [terrain-view.md](terrain-view.md) (the terrain view that owns the four managers, and its land and
water passes), [terrain.md](terrain.md) §15.5 (the view's `+0x78`–`+0x84` and the terrain's `+0xF8`),
[draw-context.md](draw-context.md) (the states, texture stages and texture transforms used here),
[texture-bindings.md](texture-bindings.md) (the binding factory), [render-properties.md](render-properties.md).

## 0. Summary

An overlay manager keeps textured **decals** and **shadows** that are drawn onto the terrain: each overlay is a
texture with a world-to-texture transform and a rectangle of cells it covers, and is drawn by redrawing those
terrain cells with the texture projected on them. Callers (tools, lots, buildings, effects) get a handle when
they add an overlay and use it to move, show, hide, recolour, fade or remove it. The terrain view owns four
managers: static and dynamic, each for land and for water.

## 1. The object *[compat]*

- 0xFC bytes, constructor `0x0073BC20`; vtables: `+0x00` `0x00AB39E8` (the interface, IID `0x895D6544`, 28 slots,
  §3), `+0x04` `0x00AB39D0` (the draw interface the draw registry calls: slot 3 `Draw`, `0x00739490`), `+0x08` a
  `cRZUnknown`, `+0x10` the message target. `QueryInterface` (`0x00735260`) answers `0x895D6544` with the object,
  anything else through the `cRZUnknown`.
- `Init(view, dynamic, water)` (`0x0073CB60`): keeps the view (`+0x8C`), the two flags (`+0x15` dynamic, `+0x16`
  water) and the initialised flag (`+0x14`); takes the texture binding factory (`+0x90`) and the terrain (`+0x88`);
  from the terrain's cell counts (`+0x5C`, `+0x60`) and cell width the cell scale (`+0x68` = 1 ÷ width) and the city
  size in metres (`+0x6C`–`+0x74`); the binding for key `0xAA592B77` *[inferred: the shadow texture of §4.3,
  `+0xD4`]*; the draw states (§4.1); for a static manager the per-cell index (`0x0073CB10`, §2.2); subscribes to
  `0xC9F775BB`; and reads `kShadowQuality` (integer 12, `+0x18`) and *[inferred]* `kShadowDebug` (integer 4,
  `+0x1C`).
- The terrain view creates four and initialises them as (static, land) `+0x78`, (static, water) `+0x7C`,
  (dynamic, land) `+0x80`, (dynamic, water) `+0x84`, registered at draw layers 2, 2, 5, 5 with priorities 0, 1100,
  1000, 1100 (terrain.md §15.5.4). The terrain keeps the first at its `+0xF8`.
- `0x0073AC90` (on shutdown) unsubscribes, releases the view, factory and terrain, empties both record tables and
  forgets the free lists.

## 2. Records and handles *[compat]*

### 2.1 Two tables

- **Overlays** (`+0x98`, 0xB4 bytes each, with a free list `+0xA4`, `+0xA8`): a free-list link (−1 while in use),
  flags (`+0x04`: bit 0 shown, bit 1 repeat the texture, bit 4 a decal or shadow part, bit 5 a texture matrix is set,
  bit 6 self-lit, bit 7 drawn in snapshots), the draw state index (`+0x08`, §4.1), the covered cell rectangle
  (`+0x0C`, x0, z0, x1, z1), a 4 × 4 world-to-texture matrix (`+0x1C`), the alpha (`+0x9C`), the colour (`+0xA0`, RGB),
  the texture binding (`+0xAC`) or a texture source object (`+0xB0`).
- **Handles** (`+0xAC` table, 0x30 bytes each): what callers hold. A handle with the high bit set names a handle
  record: its placement (nine floats from `+0x04`) and the overlay it drives (`+0x2C`). An overlay index whose high
  bit is set names a **group** (`+0xC0`, 0x24 bytes: eight overlay indices), and every call on a group is applied to
  its eight parts. Handle −1 is "none" and is ignored by every call.

Counters: the decals and shadows drawn in the last frame (`+0xD8`, `+0xDC`).

### 2.2 The cell index (static managers)

A static manager keeps, per terrain cell, the overlays that cover it (`0x00738230` adds an overlay's rectangle,
`0x007382E0` removes it), so that a frame only visits the overlays of visible cells (§4). Any change to an overlay
marks it changed (`0x00737D40`); a round decal (slot 7) also redisplays the terrain under each part's rectangle
(terrain slot 68).

## 3. The interface (`0x00AB39E8`) *[compat]*

| Slot | VA | Call | Behaviour |
|---|---|---|---|
| 3 | `0x0073C0B0` | `AddDecal(source, placement, a, b)` | a decal textured from a texture source object (its size from the object's slot 3) |
| 4 | `0x0073BF10` | `AddDecal(binding, placement, a, b)` | the same from a texture binding; −1 without one |
| 5 | `0x00736150` | `AddDecal(key, placement, a)` | the factory's binding for the key (factory slot 6), then slot 4 |
| 6 | `0x007361B0` | `AddDecal(key, placement, a)` | the same with factory slot 5 |
| 7 | `0x0073B170` | `AddRoundDecal(key, placement)` | a **group** of eight parts covering a disc (§3.1) |
| 8 | `0x0073C9A0` | `AddCityDecal(key)` | one overlay over the whole city (`0x0073B8A0`, flags `0x12`) *[inferred]* |
| 9 | `0x0073B3B0` | `AddShadow(binding, …)` | a shadow overlay |
| 10 | `0x00735480` | `AddShadow(…)` | −1 when `kShadowQuality` is 0; else the factory's shadow binding (factory slot 11) and slot 9 |
| 11 | `0x0073C0D0` | `AddModelShadow(…)` *[inferred]* | a shadow built from a model's footprint |
| 12 | `0x0073AD80` | `SetPosition(handle, xz)` | for a handle record: when the two floats change, the overlay is placed again |
| 13 | `0x0073ADE0` | `SetPlacement(handle, nine floats)` | the whole placement, then placed again |
| 14 | `0x00736490` | `GetPlacement(handle, out)` | the nine floats |
| 15 | `0x007388C0` | `Remove(handle)` | a group's parts are removed, then the record is freed |
| 16 | `0x00737E50` | `SetShown(handle, on)` | flag bit 0 |
| 17 | `0x00737F00` | `SetAlpha(handle, a)` | `+0x9C`; a static manager only redraws when a reaches 0 or 1 or moves by more than the step `0x00AB39A0` |
| 18 | `0x00738010` | `SetColor(handle, rgb)` | `+0xA0` |
| 19 | `0x007380C0` | `SetDrawState(handle, index)` | `+0x08` |
| 20 | `0x007364F0` | `EnableSelfLighting(handle, on)` (descriptive name) | when the low bit of `on` is set, OR flag bit 6; false does not clear it; groups recurse over all eight parts |
| 21 | `0x00738160` | `SetTextureScale(handle, size, offset)` | the matrix becomes a scale of 1 ÷ size with the second axis flipped and an offset of 1 + offset ÷ size; flag bit 5 |
| 22–26 | `0x00736580`–`0x00736720` | getters | an overlay field; the alpha (`+0x9C`, 0 for none); the colour (`+0xA0`); the draw state byte; shown |
| 27 | `0x007352A0` | `GetStatsString(out)` | "%d decals, %d shadows" from the counters |

Slots 0–2 are `QueryInterface`, `AddRef`, `Release`. The placement record's nine floats are a centre, extents and
an orientation *[inferred]*; the add calls turn it into the cell rectangle with the cell scale, the texture's size
and a margin (`0x007354E0`).

### 3.1 Round decals (`0x007394B0`)

A disc of radius r is split into eight rectangles of cells that follow its outline (half-chords of the circle at
the cells' rows); each becomes one part of the group, flagged as a part (bit 4), so that the decal touches only the
cells inside the disc *[inferred: the coverage circles shown when placing a building]*.

## 4. Drawing (`Draw`, `0x00738A50`) *[compat]*

A dynamic manager is not drawn under `kRenderSnapshot` (boolean 28). Otherwise:

1. The model-view is reset to the view and inverted, so that a camera-space position (texture coordinate set 16)
   can be taken back to the world; each overlay's texture transform is that inverse followed by its own matrix.
2. The overlays: for a static manager with its cell index, those that cover visible cells; otherwise every overlay
   in use.
3. With `kRenderDecals` (boolean 10): the decals (§4.2). With `kRenderShadows` (boolean 9): the shadows, the rough
   form (`DrawShadowsRough`, `0x00737870`) when `kShadowQuality` < 3 or `kUseSecondStage` is off, else the full form
   (`DrawShadows`, `0x00736BF0`).
4. The texture transform is cleared and the depth offset restored.

### 4.1 Draw states (`InitDrawStates`, `0x0073BA10`)

Five packed states, all with colour write, depth test, blending and one texture, and stage-0 mode 1 except state 4
(which keeps the mode):

| Index | Depth write | Depth function | Blend | Alpha test |
|---|---|---|---|---|
| 0 | no | less-or-equal | source alpha / one minus source alpha | no |
| 1 | no | less-or-equal | source alpha / one (additive) | no |
| 2 | no | less-or-equal | zero / source colour (multiply) | no |
| 3 | no | greater | source alpha / one minus source alpha | no |
| 4 | yes | less | source alpha / one minus source alpha | greater than 0.5 |

### 4.2 Decals (`DrawDecals`, `0x00736790`)

Depth offset 2. Each overlay in use whose flags have bits 0 and 4 (and bit 7 under `kRenderSnapshot`): the alpha as
the transparency, the colour as the shade (self-lit with bit 6) with vertex colours off for a self-lit decal, its
draw state, its texture (filters linear / linear-mipmap-linear), wrap clamp / clamp, or repeat / repeat with bit 1,
or repeat / clamp with bit 5, the texture transform, then its cell rectangle is drawn (`DrawRect`, `0x00735720`)
from the terrain view's land or water arrays (`0x007541C0`–`0x00755700`, by the water flag, the dynamic flag and
the vertex-buffer extension). Transparency, shade and vertex colours are restored.

### 4.3 Shadows (`DrawShadows`, `0x00736BF0`; `DrawShadowsRough`, `0x00737870`)

Depth offset 3, draw state 4, the shadow colour and strength from the 3D view manager (`0x00B43DD0` slot 9, then
slot 14) as the environment colour, alpha testing at "greater or equal" scaled by the strength when the strength is
above a minimum, a texture combiner; the full form adds a second stage with the shadow texture (`+0xD4`, clamped,
point-sampled) and a second texture transform, and on land reads a terrain height there (terrain slot 15)
*[inferred]*. Each shadow's cells are then drawn as for decals.

## 5. Messages

`0xC9F775BB` is subscribed by `Init` and unsubscribed at shutdown *[inferred: a terrain or zoom change that makes the
overlays be placed again]*.

## 6. Users

The terrain view creates and draws the managers; the tools, lots, buildings and effects add and update decals and
shadows through the terrain view's `GetOverlayManager(index)` (slot 18) or the terrain's `+0xF8`.

## 7. Quirks

1. A dynamic manager's overlays vanish from snapshots (§4).
2. Alpha changes smaller than the step are not redrawn in a static manager (§3, slot 17).
3. Every call on a group repeats on its eight parts, so a round decal costs eight overlays (§3.1).

## 8. Open questions

See [questions/overlay-manager.md](questions/overlay-manager.md) OV1–OV3: the placement record's nine floats and
the add calls' last arguments; slot 20 and the add forms of slots 8, 9 and 11; what message `0xC9F775BB` is.

## 9. Conformance tests

1. **Handles:** a call with handle −1 does nothing.
2. **Groups:** `SetShown(group, false)` hides all eight parts.
3. **Shadow quality:** with `kShadowQuality` 0, slot 10 returns −1.
4. **Texture scale:** `SetTextureScale(h, 16, 0)` sets flag bit 5 and a matrix scale of 1/16.
5. **Snapshot:** a dynamic manager draws nothing while `kRenderSnapshot` is on.
6. **Stats:** after drawing 3 decals and 2 shadows, slot 27 gives "3 decals, 2 shadows".

### PR review: slot 20 self-lighting

The body at `0x007364F0` ignores handle -1, resolves indirect handles as in section2.1, and applies itself recursively to all eight indices of a group. For a resolved overlay it sets flag bit6 only when the low bit of the second argument is1. It does not clear the bit for0, and makes no redraw call. This is a two-argument member call, callee pops8; no return value is specified. Evidence: branches at `0x007364FA`, `0x00736515`, recursion at `0x00736548`, bit test at `0x00736558` and flag update at `0x0073656F`.

Conformance: starting with flags0, calls with on=0,1,0 yield flags0,0x40,0x40. A group applies that same operation to its eight indices; -1 leaves every record unchanged.
