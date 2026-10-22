# Spec: lighting, time-of-day colour and hill shadows (`cSC4LightingManager`)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`). Implementers must work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled
or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[compat-limit]* depends on something FreeCity4 cannot reproduce, so only the rule is matched.

Related specs: [INVENTORY-M6.md](INVENTORY-M6.md) (§3.6, the R6 overview this spec details),
[camera.md](camera.md) (§2.7, the view messages; the zoom and rotation), [clock24.md](clock24.md) (the visual time
that drives `SetTimeOfDay`), [terrain.md](terrain.md) (§15.1, the terrain slots called here),
[prop-developer.md](prop-developer.md) (the dawn and dusk messages), [foundations.md](foundations.md) (§5.1,
messages; §18, x87 precision and the double 0.01).

## 0. Summary

The lighting manager (class `0x681BD52B`, global `0x00B43DDC`, created with the 3D view) turns the visual time of
day into colours. It keeps a **global light** colour (day light, optionally tinted), samples **colour maps** shipped as
PNG images to colour a surface by its normal, adds a key and a fill light when the maps are off, colours models by
occupant type, computes per-vertex colours for models and terrain, and keeps a **hill-shadow** height field swept
from the terrain along a sun direction fixed to the view. It announces dawn, dusk and the day/night fractions with
messages.

World coordinates: x and z on the ground (metres, a cell is 16 m), y up. Colours are RGB floats in 0…1; byte outputs
use the rounding of §4.7.

## 1. Class, state and lifetime *[compat]*

### 1.1 Identity

- Primary vtable `0x00ABB530` (30 slots, §7); `QueryInterface` (`0x007D65E0`) answers `1` and `0x681BD52A` (this),
  `0x452294AA` (the message target at `+4`, vtable `0x00ABB51C`, whose slot 3 `DoMessage` is `0x007DBCF0`).
- Reference count at `+0x0C`. `Release` (`0x007D6650`) decrements it while it is above 1; at 1 or below it calls the
  deleting destructor (slot 29) and returns 0. `AddRef` is a body shared with other classes.
- The constructor (`0x007DA0A0`) sets the time to 12.0, month 0, night false, the light colours to white
  (`0x00B0DCA4`, a constant (1, 1, 1)), no tint, no shadows, and the minimum map red `+0xD4` to 0.

### 1.2 State (0x1D8 bytes)

| Offset | Field |
|---|---|
| `+0x08` | initialised (Init done) |
| `+0x09` | a view change has been applied (set by §3, cleared by Shutdown) |
| `+0x10` | the lighting exemplar (counted) |
| `+0x14` | the terrain (global `0x00B43CF8`; set in the city and in the region view) |
| `+0x18` | the city terrain (global `0x00B43CF4`; null outside a city). In a city both are the same object |
| `+0x1C`, `+0x20` | time of day in hours (float), month 0–11 |
| `+0x24` | night (bool, slot 9) |
| `+0x28` | day-light key list (colours) |
| `+0x34` | fade-strength key list (floats) |
| `+0x40` | fade-colour key list (colours) |
| `+0x4C`, `+0x58` | fill light and key light directions (unit, view-rotated) |
| `+0x64` | shadow direction (the sun direction, view-rotated) |
| `+0x70` | the global light × light scale (§1.4, `0x29189BEB`) |
| `+0x7C`…`+0x88` | the 2×2 view rotation (cos θ, −sin θ, sin θ, cos θ), θ = −rotation × π/2 |
| `+0x8C` | the global light (tinted day light) |
| `+0x98` | the day light (untinted) |
| `+0xA4`, `+0xB0` | the global light and day light last announced (§2.2) |
| `+0xBC`, `+0xC0` | day fraction, night fraction (§2.2) |
| `+0xC4`, `+0xC8`, `+0xCC`, `+0xD0` | colour maps (§1.5) |
| `+0xD4` | the minimum red of the `+0xCC` map |
| `+0xD8` | the sun direction (unrotated) |
| `+0xE4`…`+0x128` | the model colours cached for the six axis normals +x, −x, +y, −y, +z, −z (§3) |
| `+0x12C`, `+0x130` | cell counts x, z (floats) |
| `+0x134`, `+0x138` | vertex counts x, z (cell counts + 1) |
| `+0x13C`, `+0x140` | city width and depth in metres |
| `+0x144`, `+0x148` | cell width (16) and its inverse |
| `+0x14C`, `+0x158` | fade origin and fade gradient (§3) |
| `+0x164` | fade strength (§2.1) |
| `+0x168` | fade colour (§2.1) |
| `+0x178`, `+0x17C`, `+0x188` | tint on, tint colour, tint amount |
| `+0x18C`, `+0x198` | terrain heights and shadow heights, one float per vertex (§5.1) |
| `+0x1A4`, `+0x1A8` | long terrain operation in progress; the accumulated dirty vertex rectangle |
| `+0x1B8`…`+0x1C4` | the rectangle the last shadow sweep changed (−1 when none) |
| `+0x1C8`, `+0x1CC`, `+0x1D0` | grid strides along x and z, and base (§5.1) |
| `+0x1D4` | shadows on |

### 1.3 `Init` (slot 3, `0x007DB6A0`) and `Shutdown` (slot 4, `0x007DA690`)

`Init` refuses (false) when already initialised. Otherwise:

1. **Shadows on** := the settings object's slot 7 (global `0x00B43CC4`, argument `0x0C`) returns 2 or more.
2. Load the exemplar `0x6534284A / 0xA9189CF0 / 0x00000001` into `+0x10`; `ReadProperties` (§1.4).
3. Unless the test-pattern property is true: load the four colour maps (§1.5); when the `+0xCC` map exists,
   `+0xD4` := its minimum red byte ÷ 255 (1.0 for an empty map; `cImageARGB8::FindMinRedComponentValue`
   `0x007D6870`). With the test pattern: `+0xC4` and `+0xC8` get the 64×64 test pattern (§1.5), the others stay null.
4. Subscribe to `0xC53D10AA`, `0xC9F775BB`, `0x2B96B3EA`, `0xA6B79601`, `0xEA3B0AF1`, `0x6A3B0AFD`, `0x278E6003`
   (§6), in that order; initialised := true.

`Shutdown` (false when not initialised) unsubscribes the same seven, clears `+0x09`, releases `+0x10` and `+0x14`,
frees the four maps and clears initialised. The deleting destructor (slot 29, `0x007DA670`) runs the destructor
(`0x007DA400`: releases `+0x18`, `+0x14`, `+0x10` and frees the five key and grid vectors) and frees the object
when bit 0 of its argument is set.

### 1.4 Properties (`ReadProperties`, `0x007DA7E0`) *[compat]*

Read from the exemplar into statics, every time it runs (also on `0xC53D10AA`). A missing property keeps the
static's previous value. The shipped values are those of the exemplar in `SimCity_1.dat`; "—" marks a property the
shipped exemplar lacks, with the static's initial value in brackets.

| Property | Shipped | Meaning |
|---|---|---|
| `0x29189BEB` | 0.1 | light scale: `+0x70` = global light × it |
| `0x29189BEC` | true | use the colour maps (§4.1) |
| `0x29189BED` | — (false) | test pattern instead of the maps (§1.3) |
| `0x29189BEE` | 0.8 | terrain directional weight (§4.5) |
| `0x29189BEF` | — (false) | debug model light: colour by position (§4.3) |
| `0x29189BF0` | (1, 1, 1) | day light colour |
| `0x09B6D6AE` | (0.5, 0.5, 0.78) | night light colour |
| `0x29189BF1` | 0.9 | model directional weight, flora and group `0x5004` (§4.3) |
| `0x29189BF2` | 0.4 | model directional weight, default (§4.3) |
| `0x29189BF3` | 0.7 | model directional weight, vehicles, pedestrians, aircraft, watercraft (§4.3) |
| `0x29189BF4` | (0.875, 0.949, 1) | fade colour |
| `0x29189BF5` | 0.15 | fade strength at midday |
| `0x29189BF6` | 1e-4 | light change threshold (§2.2); overwritten on `0xC9F775BB` (§6) |
| `0x29189BF7` | 0.8 | night threshold on the day light's red |
| `0x085489C8` | `0x69D3BC3B` | decal shadow texture (§5.6) |
| `0x085489C9` | — (0, 0) | decal shadow pair (2 floats) |
| `0x085489CA` | 12.0 | decal shadow value |
| `0x085489CB` | (0.08, 0.06, 0.23) | shadow colour |
| `0x085489CC`, `0x085489CD` | 0.4, 0.55 | shadow parameters (§5.6) |
| `0x085489CE`, `0x085489CF` | 67.5, 45 | sun azimuth and elevation, degrees |
| `0x085489D0` | 2.0 | shadow normal bend (§5.3) |
| `0x085489D1` | 0.7 | hill-shadow slope factor (§5.2) |
| `0x085489D2` | 50 | shadow depth fade, metres (§5.4) |
| `0x085489D3` | — (0.6) | night reduction of shadow strength (§5.4) |

Then:

- the day light and the global light := the day colour; `+0x70` := light scale × it; fade strength := `0x29189BF5`;
- the **day-light key list** := 8 keys: night, night, day, day, day, day, night, night;
- the **fade-strength list** := 4 keys: 0, s, s, 0 (s = `0x29189BF5`); the **fade-colour list** := 1 key, the fade
  colour;
- the sun direction := (sin a · cos e, −sin e, −cos a · cos e), a and e the azimuth and elevation in radians
  (× 0.017453292).

### 1.5 Colour maps *[compat]*

Each map is a `cImageARGB8` (0x18 bytes: a vector of 32-bit pixels, width, height, row stride). `CreateColorMap`
(`0x007DB4D0`) loads the image of group `0xA9179251` and the given instance through `cSLBuffer` (`0x00602B70`, type
0 and format 9) and copies it pixel by pixel (`0x007DB450`); a missing image leaves the map null. A pixel's red,
green and blue are bits 16–23, 8–15 and 0–7, each ÷ 255.

| Map | Instance | Shipped size | Use |
|---|---|---|---|
| `+0xC4` | `0x0917660C` | 64×64 | surface colour by normal (§4.1) |
| `+0xC8` | `0x0917660D` | 64×64 | model colour by normal (§4.2) |
| `+0xCC` | `0x0917660E` | 32×32 | day light over time × month (§2.1) |
| `+0xD0` | `0x0917660F` | 32×32 | fade colour over time × month (§2.1) |

The shipped images are PNGs (type `0x856DDBAC`) in `SimCity_1.dat`.

The **test pattern** (`MakeTestPattern`, `0x007DB570`) is 64×64: opaque, red 255 where x > 32, green 255 where
y > 32, blue 0 (so the dark half is 33 pixels wide) *[quirk]*.

## 2. Time of day *[compat]*

### 2.1 The time-dependent values (`SetTimeDependentLightingValues`, `0x007D9820`)

Let f = time × (1/24) and v = (month + 0.5) × (1/12).

- **Day light** `+0x98`: when the `+0xCC` map exists, sample it at (f, v) (below); else interpolate the day-light key
  list at f (below).
- When the settings object's `+0x0C` member has its byte `+0x39C` clear: **fade colour** `+0x168` from the `+0xD0`
  map at (f, v), else from the fade-colour list; **fade strength** `+0x164` from the fade-strength list at f.
  When the byte is set: fade colour := (0, 0, 0) and fade strength := 0.
- **Global light** `+0x8C` := the day light, or with the tint on, lerp(day light, tint colour, tint amount); the fade
  colour gets the same lerp. `+0x70` := light scale × global light.

**Key list at f**, n keys: with n = 1 the key; else p = (n − 1) f, i = ⌊p⌋, t = p − i; t ≤ 0 gives key i, else
(1 − t)·key[i] + t·key[i + 1].

**Map sample at (u, v)** (`0x007D6690`): x = W·u, x0 = ⌊x⌋, x1 = ⌈x⌉, y = ⌊H·v⌋; the result is the pixel (x0, y)
lerped toward the pixel (x1, y) by x − x0. There is no clamp: for u above (W − 1)/W, x1 = W reads the first pixel of
the next row *[quirk]* (between 23:15 and midnight on the shipped 32-wide maps).

### 2.2 `SetTimeOfDay(t)` (slot 6, `0x007D9DF0`)

1. If t is below the current time (the day wrapped), month := slot 15 of the city object at global `0x00B43DAC` (city
   slot 70) − 1 *[inferred: the month 1–12]*.
2. Time := t; §2.1.
3. If the distance between the global light and the last announced global light is **not above** the change threshold,
   return false.
4. If there is a terrain (`+0x14`):
   - wasNight := last announced day light red < T, isNight := day light red < T (T = `0x29189BF7`);
   - if they differ and the `+0x39C` byte (§2.1) is clear, post (message-server slot 4) `0xC9DA96E8` (dawn) when it
     was night, else `0xC9DA96E9` (dusk);
   - day fraction `+0xBC` := max(0, (red − T) / (1 − T)); night fraction `+0xC0` := max(0, 1 − (red − m) / (T − m)),
     m = `+0xD4`, with T − m replaced by 0.0001 when it is 0;
   - send (slot 3) `0xC9DA96EA` with the day fraction (message slot 11) and the night fraction (message slot 16);
   - last announced := the current global light and day light; night (`+0x24`) := isNight.
5. Return true.

Night is decided on the **untinted** day light; the threshold test uses the **tinted** global light.

### 2.3 Tint

`SetLightingTint(colour, amount)` (slot 7, `0x007D6AB0`) stores the tint and turns it on; `ResetLightingTint`
(slot 8, `0x007D6B00`) turns it off. Both then call `SetTimeOfDay(current time)` and, when it returns true, the
global `0x00B43DD0`'s slot 20 *[inferred: a redraw request]*.

## 3. View changes (`DoZoomAndRotationChange(zoom, rotation)`, `0x007DB840`) *[compat]*

Run on ViewZRChange2 (`0xA6B79601`), at city start (`0x2B96B3EA`) and when the shadow setting changes.

1. Key light := normalize(−1, 1, 1), fill light := normalize(1, 0.5, −1), shadow direction := the sun direction; each
   rotated by the view rotation r (`0x007F6CC0`: r = 1 maps (x, z) to (−z, x), r = 2 to (−x, −z), r = 3 to (z, −x)).
2. The 2×2 view rotation `+0x7C` for θ = −r × π/2.
3. **Fade:** origin := (−W/2, 0, D/2) and gradient := (0, 0, −s / (2^zoom × D)), with W, D the city size in metres
   and s the fade strength **at this moment**; both rotated by r; then the origin += (W/2, 0, D/2). For r = 0 the
   fade term `(p − origin) · gradient` is s (D − p.z) / (2^zoom D).
4. Without a terrain, skip to 6. Grid strides for vertex (i, j), with X, Z the vertex counts:
   r = 0: i·Z + j; r = 1: −i + j·X + (X − 1); r = 2: −i·Z − j + (XZ − 1); r = 3: i − j·X + (Z − 1)·X.
5. With shadows on: copy all terrain heights into `+0x18C` (terrain slot 15, `GetAltitudes`, with the strides
   above) and sweep the shadows (§5.2).
6. Mark the view applied (`+0x09`) and cache the model colours (§4.2) for +x, −x, +y, −y, +z, −z into `+0xE4`…
   — but the −y entry is computed with the **up** normal (0, 1, 0) *[quirk]*. With the colour maps on, the two are
   equal anyway (the map ignores y); with them off, downward faces get the upward colour.

## 4. Colour queries *[compat]*

### 4.1 `GetColor(out, n)` (slot 23, `0x007D7460`) — the base colour of a normal

- **Colour-map path** (maps on and `+0xC4` present): (x', z') := the 2×2 view rotation applied to (n.x, n.z);
  pixel (⌊W (x' + 1) / 2⌋, ⌊H (z' + 1) / 2⌋) of `+0xC4`, each clamped to W − 1, H − 1.
- **Lambert path** otherwise: (0.05, 0.05, 0.05) + max(0, n·key)·(1, 1, 0.8) + max(0, n·fill)·(0, 0.1, 0.5), each
  channel clamped to at most 1.

The other overloads wrap it:

| Slot | VA | Arguments | Result |
|---|---|---|---|
| 24 | `0x007D73E0` | `(out)` | slot 23 for the up normal |
| 22 | `0x007D77C0` | `(out, n, p)` | with shadows on and depth(p) > 0 (§5.3): slot 23 for the bent normal; else slot 23 |
| 21 | `0x007D7420` | `(r&, g&, b&)` | slot 20 for the up normal |
| 20 | `0x007D7730` | `(n, r&, g&, b&)` | slot 23 as bytes |
| 19 | `0x007D7840` | `(p, n, r&, g&, b&)` | slot 22 as bytes |

### 4.2 Model colour (`GetModelColor(out, n)`, `0x007D83C0`)

With the maps on and `+0xC4` present: the same position as §4.1 (computed from the **`+0xC4`** map's size) but the
pixel is read from **`+0xC8`** *[quirk]*: a replacement `0x0917660D` of another size than `0x0917660C` is sampled
wrongly. Otherwise slot 23.

### 4.3 `GetModelLight(model, out)` (slot 11, `0x007D8550`)

The model gives its bounding box (slot 8) and its owner (slot 3), queried for the occupant `0x26BB767E`.

1. Weight w := `0x29189BF2` (0.4). For an occupant:
   - a network occupant (`0xA821EF94`) whose slot 8 answers true for `0x40000`: out := light ⊙ (w·m + (1 − w)·white),
     m the model colour of the up normal; done;
   - flora (`0x74758926`) or in occupant group `0x5004` (occupant slot 22): w := `0x29189BF1` (0.9);
   - vehicle `0xC7CC75F1`, pedestrian `0x496E5783`, aircraft `0x480E6151` or watercraft `0x897F7EC2`:
     w := `0x29189BF3` (0.7).
2. With the debug property `0x29189BEF`: out := (fmod(cx, 256) / 256, fmod(cz, 256) / 256, 1), (cx, cz) the box
   centre; done.
3. Normal: with shadows **on**, up, bent (§5.3) when the box centre is inside the city and its shadow depth is
   positive. With shadows **off**, the terrain normal (§5.5) at the box centre, or, when the box is 50 m or more
   in x or z, the sum of the terrain normals at its four corners × 0.25.
4. out := light ⊙ (w·m + (1 − w)·white), m := the model colour of that normal, light the global light.

### 4.4 `GetVertexColors(count, normals, nStride, colours, cStride, M)` (slot 27, `0x007D7BF0`)

For each normal n (3 floats, every nStride bytes): n' := M n (M a 3×3, rows); colour := |n'.x|·C(±x) +
|n'.y|·C(±y) + |n'.z|·C(±z), C the cached colours of §3 picked by each sign (0 counts as +); each channel clamped to
at most 1; written as 4 bytes **B, G, R, 255** every cStride bytes.

### 4.5 Terrain colours

Let k := the fade term at p (§3), c := slot 23 of the (possibly bent) normal.

- `GetTerrainLighting(p, r&, g&, b&)` (slot 12, `0x007D8B20`): the terrain normal at (p.x, p.z) (§5.5), bent when
  shadows are on and depth(p) > 0; bytes of 0.8·c + 0.2·white (weights `0x29189BEE` and 1 minus it).
- `GetTerrainColor(p, n, r&, g&, b&)` (slot 25, `0x007D7940`): with shadows on, the depth is taken at p lowered to the
  land; bytes of (1 − k²)·c.
- `GetTerrainColorFloor(p, n, r&, g&, b&)` (slot 26, `0x007D7AE0`): no shadow test; bytes of
  k²·fade colour + c ⊙ `+0x70`.
- `GetTerrainVertexColors(j, i0, count, normals, nStride, colours, cStride)` (slot 28, `0x007D7F00`): for the vertices
  (i0 + t, j), t < count, writes 6 bytes R, G, B, R2, G2, B2: with shadows off, c and c ⊙ `+0x70`; with shadows on,
  k from the vertex at height 0, the normal bent by the grid depth (shadow − terrain height at the vertex), then
  (1 − k²)·c and k²·fade colour + c ⊙ `+0x70`. So with shadows off the vertices get **no fade** while
  `GetTerrainColor` still applies it *[quirk]*.

### 4.6 Simple getters

`GetGlobalLight(out)` (slot 10, `0x007D6B20`) and `GetShadowDirection(out)` (slot 13, `0x007D6B50`) copy `+0x8C` and
`+0x64`. Slots 5 (time) and 9 (night) return `+0x1C` and `+0x24`.

### 4.7 Byte conversion

A colour channel c becomes the byte n − (n >> 8) with n := round(256 c) (computed as the low bits of c + 49152.0):
0 → 0, 1 → 255. No clamp is applied; callers keep c in 0…1.

## 5. Hill shadows *[compat]*

### 5.1 The grids

At city start (`0x2B96B3EA`) both grids are sized to the vertex count and filled with 0. `+0x18C` holds terrain
heights, `+0x198` the shadow surface; both use the strides of §3, so the stored rows follow the view rotation and the
sun is fixed relative to the screen. A world point (x, z) uses the vertex (⌊x / 16⌋, ⌊z / 16⌋) **without a bounds
check** (TR12-style; callers keep points inside the city).

### 5.2 The sweep (`FindHillShadows`, `0x007DAE70`)

Let (sx, sy, sz) be the **unrotated** sun direction. Nothing happens when sx < 0. Otherwise, with Z vertices per
stored row and X rows (the city grids are square):

- drift := −sz / sx; when it is outside [−1, 1] the sweep is skipped;
- drop := (16 / sx) · sy · `0x085489D1` (negative: the shadow surface falls by 16 tan(e) × 0.7 per row at
  azimuth 90°);
- a running height h[c] per column starts at 0. For each row in order, for each column c: h[c] := max(h[c],
  terrain[c]); if shadow[c] ≠ h[c], shadow[c] := h[c] and the rectangle is grown; h[c] += drop. Then h shifts
  sideways by the drift: for drift ≤ 0, from the last column down to column 1, h[c] −= (h[c − 1] − h[c]) · drift;
  else, from column 0 up, h[c] += (h[c + 1] − h[c]) · drift.
- The changed rectangle (first and last changed column, first and last changed row, stored coordinates) is kept in
  `+0x1B8`…`+0x1C4`; −1 when nothing changed. When the sweep is skipped (sx < 0 or the drift out of range) the
  rectangle keeps its previous value *[quirk]*; the shipped sun (azimuth 67.5°, drift 0.414) always sweeps.

The whole grid is swept on every call.

### 5.3 Depth and the bent normal

- `ShadowDepth(p)` (`0x007D6950`) := shadow height at (p.x, p.z) − p.y; positive means p is in a hill shadow.
- `HeightAboveLand(p)` (`0x007D6A00`) := p.y − terrain height at (p.x, p.z).
- `BendShadowedNormal(out, n, d)` (`0x007D68B0`): normalize(n + 2.0 × d × 0.01 × fill), the fill light of §3
  (`0x085489D0`; the 0.01 is a double); the sum unnormalised when its length is 0.

### 5.4 `GetShadowStrengthFromLocation(xz)` (slot 15, `0x007D6BB0`)

The argument is a 2D point (x, z). d := shadow − terrain height at its vertex. If d > 50 (`0x085489D2`): 0. Else
s := 1 − `0x085489D3` × night fraction, × (1 − d / 50) when d > 0.

### 5.5 Terrain normal (`0x007D8340`)

(x, z) is clamped to [0, W − ε] × [0, D − ε] (ε the double at `0x00AA2270`), then terrain slot 27 (a normal read,
terrain.md §15.1).

### 5.6 Shadow parameters and projections

- `GetShadowParams(a&, b&, colour&)` (slot 14, `0x007D6B70`): `0x085489CC`, `0x085489CD`, `0x085489CB`.
- `GetDecalShadowParams(holder, tex&, a&, b&, c&, pair&, colour&)` (slot 16, `0x007DA490`): the lighting values of
  `0x085489C8`, `0x085489CC`, `0x085489CD`, `0x085489CA`, `0x085489C9`, `0x085489CB`, each then overridden by the
  given property holder when it has them, so a model exemplar can change its own decal shadow.
- `CalcShadowTerrainRect(…, box, outRect)` (slot 18, `0x007D6CC0`): the cell rectangle ⌊min / 16⌋ … ⌈max / 16⌉ of the
  box's x and z, clipped to the city, then extended cell by cell in the shadow direction while a ray from the box top
  (falling by the shadow direction's slope per cell) stays above the terrain heights; always true *[inferred: the
  cells a shadow cast from the box can reach]*.
- `CalcShadowProjection(points, uvs, count, xform, inout)` (slot 17, `0x007D8C80`): fits, by least squares, the
  texture coordinates of the points as an affine function of their position in a plane perpendicular to the shadow
  direction (expressed in the model's frame, `0x0080B9F0` building the plane basis), and writes the resulting texture
  matrix, composed with the transform, into `inout`; false when the 3×3 normal-equation determinant is below 0.01
  *[inferred: the exact composition with the transform's flags]*.

## 6. Messages (`DoMessage`, `0x007DBCF0`) *[compat]*

| Message | Handling |
|---|---|
| `0xA6B79601` ViewZRChange2 | §3 with the message's new (zoom, rotation) pair |
| `0x2B96B3EA` PostAppServicesInit | take the city terrain and the terrain (§1.2; a city terrain clears the long-operation flag); cell width, cell and vertex counts, city size; size the grids; §3 with the current view zoom and rotation (`SC4ViewUtilities`, `0x00B43DD8` `+0x0C`, `+0x10`) |
| `0x278E6003` terrain changed | with a city terrain and shadows on: during a long operation, grow the dirty rectangle; else, if the terrain is initialised, copy the heights of the rectangle into `+0x18C`, sweep (§5.2), and when something changed, rotate the changed rectangle back to world cells (`0x007D6540`) and pass it to terrain slot 67 |
| `0xEA3B0AF1` long terrain operation begins | flag on; dirty rectangle := empty |
| `0x6A3B0AFD` long terrain operation ends | flag off; with shadows on and a non-empty rectangle, as above, then also terrain slot 68 (`RedisplayTerrain(0, 0, rect, 0)`) |
| `0xC53D10AA` exemplar changed | §1.4 |
| `0xC9F775BB` render properties changed | the change threshold := the float `+0x10C` of the settings object's `+0x24` member *[quirk: overrides the exemplar]*; shadows on := settings slot 7 (`0x0C`) ≥ 2; when that changed: §3, send `0xC9DA96EA` with the stored fractions and call `0x00B43DD0` slot 20 |

## 7. Slot table (vtable `0x00ABB530`)

| Slot | VA | Name |
|---|---|---|
| 0–4 | `0x007D65E0`, (shared), `0x007D6650`, `0x007DB6A0`, `0x007DA690` | QueryInterface, AddRef, Release, Init, Shutdown |
| 5, 6 | (shared `0x007D6670`), `0x007D9DF0` | GetTimeOfDay, SetTimeOfDay |
| 7, 8, 9 | `0x007D6AB0`, `0x007D6B00`, (shared `0x007D6680`) | SetLightingTint, ResetLightingTint, IsNight |
| 10–12 | `0x007D6B20`, `0x007D8550`, `0x007D8B20` | GetGlobalLight, GetModelLight, GetTerrainLighting |
| 13–18 | `0x007D6B50`, `0x007D6B70`, `0x007D6BB0`, `0x007DA490`, `0x007D8C80`, `0x007D6CC0` | shadow direction, params, strength, decal params, projection, terrain rect |
| 19–24 | `0x007D7840`, `0x007D7730`, `0x007D7420`, `0x007D77C0`, `0x007D7460`, `0x007D73E0` | GetColor (§4.1) |
| 25–28 | `0x007D7940`, `0x007D7AE0`, `0x007D7BF0`, `0x007D7F00` | terrain colour, floor colour, vertex colours, terrain vertex colours |
| 29 | `0x007DA670` | deleting destructor |

The message-target vtable `0x00ABB5A8` holds the `this − 4` thunks `0x007DA3E0` (QueryInterface), `0x007DA3F0`
(Release) and `DoMessage`.

## 8. Users

Allocated (0x1D8 bytes) and constructed by `cSC43DRender::Init` (`0x007C6410`); the global `0x00B43DDC` is set and
cleared by `SL::OnView3DInit` / `OnView3DShutdown` and by the region screen's application services.
Called by the terrain (`cSTETerrain::DoLighting`, the terrain view), the model maker (`DoLightingChanged`), network
occupants (`FlipAndLight`, `UpdateShadow`), cell texture models, lot retaining walls, construction occupants, vehicles
and automata, the dispatch view, particles and screen flashes, and the prop developer. The time comes from the 24-hour
clock's visual time (clock24.md).

## 9. Quirks

1. The map sampler reads past the row end in the last 1/W of the day (§2.1).
2. The −y cached model colour uses the up normal (§3).
3. The model colour samples `+0xC8` at a position computed from `+0xC4`'s size (§4.2).
4. The fade gradient keeps the fade strength of the last view change; time alone does not update it (§3).
5. Terrain vertices lose the fade when shadows are off, `GetTerrainColor` does not (§4.5).
6. A render-properties change overwrites the exemplar's change threshold (§6).
7. The test pattern's split is at 33 pixels (§1.5).
8. A skipped shadow sweep leaves the previous changed rectangle in place (§5.2).

## 10. Open questions

See QUESTIONS.md LT1–LT4: the meaning of the settings byte `+0x39C` (§2.1), the network occupant test `0x40000`
(§4.3), what the fade term is for on screen (§3), and the decal shadow values `0x085489C9` / `0x085489CA` (§5.6).

## 11. Conformance tests

1. **Day light:** with no `+0xCC` map, day colour (1, 1, 1) and night colour (0.5, 0.5, 0.78): at 0:00 the day light
   is the night colour; at 6:00 it is (0.875, 0.875, 0.945) (p = 1.75: a quarter of key 1, night, and three quarters
   of key 2, day); at 12:00 it is white.
2. **Night threshold:** with T = 0.8, a day light red falling from 0.81 to 0.79 posts `0xC9DA96E9` once; rising back
   posts `0xC9DA96E8`; a change smaller than the threshold posts nothing and returns false.
3. **Fractions:** red 0.9, T 0.8, m 0.5: day fraction 0.5, night fraction 0; red 0.65: day 0, night 0.5.
4. **Lambert colour:** maps off, rotation 0: `GetColor` of the up normal is (0.05 + 0.57735 + 0, 0.05 + 0.57735 +
   0.03333, 0.05 + 0.46188 + 0.16667) = (0.627, 0.661, 0.679) to 3 decimals (key light y 1/√3, fill light y 1/3).
5. **Model weights:** with the model colour m = (0.5, 0.5, 0.5) and white light, a building gives 0.4·0.5 + 0.6 = 0.8,
   a tree 0.9·0.5 + 0.1 = 0.55, a car 0.7·0.5 + 0.3 = 0.65.
6. **Vertex colours:** maps off, the downward normal (0, −1, 0) gets the same colour as (0, 1, 0).
7. **Shadow strength:** d = 25, night fraction 0, `0x085489D3` = 0.6: 0.5; d = 60: 0; d = −3 at full night: 0.4.
8. **Sweep:** on flat ground at height 0, one 100 m vertex in row 0, sun azimuth 90° (drift 0) and elevation 45°:
   the same column of row n gets the shadow height 100 − 11.2 n (16 × 0.7 per row) until that reaches the ground,
   so rows 1–8 are shadowed and depth(row 1, ground) = 88.8.
9. **Bytes:** 0.5 → 128, 0.999 → 255, 0.0019 → 0.
