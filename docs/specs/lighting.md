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

**Constructor boundary (LT5):** the reference count starts at **0**, not 1 (`0x007DA0BE`). Each successful
QueryInterface stores the requested interface and calls primary slot 1 once, including the `+4` message
interface; an unsupported IID returns false without overwriting the output (`0x007D65E0–0x007D6642`).
The adjustment thunks subtract four from the incoming message-interface pointer before forwarding.

At construction, current global/day light and tint colour are white; last-announced global/day light,
fill/key/shadow directions, scaled light, view rotation, all six cached axis colours, fade origin/gradient
and fade colour are **zero**. Day fraction is 1 and night fraction 0. The unrotated sun starts at
**(1, −1, −1)**, not a normalized direction. Time is 12, month 0, minimum red 0, tint amount 0, tint disabled,
shadows disabled, view-applied and initialized flags clear. Vector headers for the three key lists and
the two terrain grids are empty. Evidence: constructor `0x007DA0A0–0x007DA3D1`, zero data
`0x00B4E334–0x00B4E340`, white data `0x00B0DCA4–0x00B0DCAC`.

This is **not** a zero-fill constructor. In particular, it does not write map pointers `+0xCC/+0xD0`,
city dimensions/cell widths `+0x12C…+0x148`, dirty/sweep rectangle coordinates, or grid strides. Their
pre-initialization contents are not a native default. The normal map-loading Init path assigns all four
map pointers; the test-pattern path assigns only `+0xC4/+0xC8`. City-start establishes the geometry.
DoZoomAndRotationChange computes the fade formula *before* checking for terrain (`0x007DB840–0x007DBA60`):
before geometry exists it consumes those unset dimensions. Consequently no universal zero-gradient
result is specified for that call. A portable safety policy may initialize these fields, but must not
claim that policy as an observed constructor result.

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
   `0x007D6870`). With the test pattern: `+0xC4` and `+0xC8` get the 64×64 test pattern (§1.5); the other two pointers are not assigned.
4. Subscribe to `0xC53D10AA`, `0xC9F775BB`, `0x2B96B3EA`, `0xA6B79601`, `0xEA3B0AF1`, `0x6A3B0AFD`, `0x278E6003`
   (§6), in that order; initialised := true.

`Shutdown` (false when not initialised) unsubscribes the same seven, clears `+0x09`, releases `+0x10` and `+0x14`,
frees the four maps and clears initialised. The deleting destructor (slot 29, `0x007DA670`) runs the destructor
(`0x007DA400`: releases `+0x18`, `+0x14`, `+0x10` and frees the five key and grid vectors) and frees the object
when bit 0 of its argument is set.

### 1.4 Properties (`ReadProperties`, `0x007DA7E0`) *[compat]*

**Initial shared configuration (LT5):** before any exemplar read, the original image seeds the following
values. These are shared across instances; construction and Shutdown do not restore them. Missing-property
retention therefore refers to the previous shared value, not necessarily to the shipped exemplar.
Float decimals in this table mean their binary32 encodings. Property-to-storage bindings are in
`0x007DA7F3–0x007DA9F2`; initial data is `0x00B0DC50–0x00B0DC98`, `0x00B0DCBC–0x00B0DCDC` and
zero-initialized `0x00B4E32C–0x00B4E354`.

| Properties | Initial value |
|---|---|
| `29189BEB`, `29189BEC`, `29189BED`, `29189BEF` | 0.1, true, false, false |
| `29189BEE`, `29189BF1`, `29189BF2`, `29189BF3` | 0.6 each |
| `29189BF0`, `09B6D6AE` | (1,1,1) each |
| `29189BF4`, `29189BF5` | (0,0.5,1), 0.5 |
| `29189BF6`, `29189BF7` | 0.0001, 0.8 |
| `085489C8`, `085489C9`, `085489CA` | `0x0917660E`, (0,0), 8 |
| `085489CB`, `085489CC`, `085489CD` | (0,0,0), 0.5, 0 |
| `085489CE`, `085489CF`, `085489D0` | 90, 45, 1 |
| `085489D1`, `085489D2`, `085489D3` | 0.7, 50, 0.6 |

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
| `0x085489C8` | `0x69D3A23B` | decal shadow texture (§5.6) |
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

**Texture identifier correction (LT15):** the shipped lighting exemplar's `0x085489C8` is the unsigned
32-bit integer **`0x69D3A23B`** (1775477307). The previous `0x69D3BC3B` table entry was a transcription
error. ReadProperties reads this property into the shared texture value, and slot 16 with a null holder
copies that value unchanged; there is no texture-ID translation. A nonnull holder may override it, and
an absent property retains the previous shared value, initially `0x0917660E`. Evidence: the installed
GOG `SimCity_1.dat`, exemplar `6534284A/A9189CF0/00000001`, property `085489C8` (read with the read-only
DBPF reader); unsigned property read `0x007DA92C–0x007DA93A`, integer copy `0x007DA490–0x007DA49E`,
holder override `0x007DA500–0x007DA507`. The glue lane's initial 46 captures agree with the corrected
value; that run's 0/46 exact verdict remains a failure of its earlier literal expectation.

### 1.5 Colour maps *[compat]*

**Storage boundary (LT7):** `FindMinRedComponentValue` scans every 32-bit element from vector begin to
vector end, irrespective of width, height or stride. Padding elements inside that range participate; spare
capacity after end does not. The empty-vector accumulator is 255. The result is the minimum byte multiplied
by the binary32 constant `0x3B808081` (approximately 0.003921568859368563), using x87 arithmetic; Init stores
the result as binary32. Thus the empty result at the Init storage boundary is 1.0. Evidence:
`0x007D6871–0x007D689E`, constant `0x00A90BBC`. This supersedes an active-width-only scan.

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

**Failure and lifetime boundary (LT7):** a false buffer Load result assigns that map pointer null and
destroys the temporary buffer. Init continues with subsequent maps, performs subscriptions when a message
server exists, sets initialized and returns true. It does not return false because an image is missing;
false is the already-initialized path. Image copying sets width, height and stride (= width), resizes to
width×height pixels and obtains each pixel from image slot 21; the lighting helper has no per-pixel failure
test or rollback path (`0x007DB450–0x007DB4CD`, `0x007DB4D0–0x007DB565`, `0x007DB6A0–0x007DB838`). An
allocation failure or foreign decoder exception has no lighting-level recovery contract; it is not a
second, normal false-return path. Do not invent transactional cleanup or a successful retry after such an
interruption.

Shutdown is the explicit cleanup protocol: unsubscribe when the message server exists, release exemplar
and general terrain, destroy all four map allocations, null their pointers, and clear initialized and
view-applied. It retains the city-terrain reference, key lists, terrain grids, shared properties and
lighting values. A second Shutdown returns false. The destructor frees the key/grid vectors and releases
city terrain, general terrain and exemplar, but **does not call Shutdown, unsubscribe or free maps**.
Deleting destruction alone is not equivalent to the Shutdown-then-Release protocol.

SampleRowLerp computes linear addresses `begin + 4×(stride×row + column)` without bounds tests, including
both horizontal sample addresses even when their interpolation weight is zero. Row spill is ordinary
linear addressing when storage exists. Beyond allocated storage there is no defined colour/error result
from this owner. Similarly, GetModelColor tests the *surface* map pointer to enter its map path, then
dereferences the model map using coordinates bounded by the surface dimensions: it does not test model
presence/length. Absent or too-short model storage has no native fallback/error contract. Portable checked
errors are safety policies for these invalid reads, not observed native outcomes (`0x007D6690–0x007D6861`,
`0x007D83C0–0x007D854E`).

The **test pattern** (`MakeTestPattern`, `0x007DB570`) is 64×64: opaque, red 255 where x > 32, green 255 where
y > 32, blue 0 (so the dark half is 33 pixels wide) *[quirk]*.

### 1.6 Independent live-owner input replay (LT14) *[compat]*

**Snapshot boundary:** this owner exposes neither Read/Write nor a serializable interface. Its
QueryInterface accepts only Unknown, lighting and message-target interfaces (§1.1; instruction evidence
`0x007D65E0–0x007D6642`). A city save plus a poll of current clock, view and settings is therefore
**not a complete owner snapshot**. Last-announced colours, cached month, fade gradient, pending dirty
rectangle and retained shared properties depend on earlier inputs. There is no public operation that
recovers all of these independently. Whole-owner replay requires a complete ordered input history
from a known constructor/Init boundary, including the process's earlier shared-property changes, or
an independently maintained checkpoint built from that history. Starting a monthly observer on an
already live owner supports only comparisons whose inputs are independently known. It does not
authorize filling missing expected state from lighting getters. This is the literal answer to LT14;
the replay record below is a harness contract, not a native serialization format.

Each replay record must identify the owner lifetime, sequence of entry/delivery, event kind, copied
argument bits and dependency results actually consumed by that event. Preserve binary32 values as
bits, property presence/type/count as well as values, and interface/resource availability. Borrowed
message payload pointers and mutable property holders must be copied while valid. A later poll
cannot supply an earlier dependency result. The input boundary is:

| Input family | Independent source and record | When it matters |
|---|---|---|
| Construction and Init | §1.2 constructor state; §1.4 process-initial shared configuration plus all prior property reloads; settings slot 7 with argument `0x0C`; resource lookup success and effective properties of `6534284A/A9189CF0/00000001`; decoded pixels, dimensions/stride and load success of all four §1.5 maps | Init reads settings, exemplar and maps before subscribing to the seven messages in §1.3 order (`0x007DB6B4–0x007DB82D`). Construction/Shutdown do not reset the shared statics. |
| Time delivery | Exact argument of each lighting slot-6 call; for the normal view driver, clock visual hour and minute read at that delivery (§2.4) | Include calls returning false: time, sampled colours and possibly month still change. On a backwards time step record the city date service's month result (city slot 70, then service slot 15); it is not read on every delivery (`0x007D9DF0–0x007D9E1C`). |
| Tint/reset | Arguments of lighting slot 7 (RGB and amount), and every slot-8 reset, from the actual caller | These are direct interface calls, not owner message subscriptions. They immediately recompute at stored time (§2.3). Clock messages alone cannot replay tint. |
| Render settings | Raw result of settings slot 7 (`0x0C`); threshold binary32 at settings member `+0x24`, offset `+0x10C`; fade-suppression byte at settings member `+0x0C`, offset `+0x39C` | Threshold/shadow toggle are consumed at Init or `0xC9F775BB` as specified in §§1.3/6. The byte is consumed by time-dependent recomputation and dawn/dusk handling (§2). The byte is the boolean render property `kRenderSnapshot` (boolean 28; LT1, §2.1); it is not the day/night clock preference. There is no established complete public settings snapshot here. |
| View and city binding | New zoom/rotation pair carried by `0xA6B79601`; at `0x2B96B3EA` and a shadow-toggle refresh, current view zoom/rotation from view utilities; terrain/city-terrain availability, cell width and cell counts | City-start establishes geometry and grids, then applies the view (§6). A view refresh consumes the fade strength existing at that instant (§3). |
| Terrain edits | Ordered `0xEA3B0AF1`, `0x278E6003`, `0x6A3B0AFD` deliveries; inclusive incoming rectangle; terrain initialized result and altitude-read results when requested | Long-operation edits accumulate rectangles; heights are read at the eventual copy, not necessarily at each notification. Full view refresh copies all heights when shadows are enabled. Preserve the §§5/6 stride convention. |
| Exemplar changes | Effective property presence/values from the retained holder at each `0xC53D10AA` delivery | §1.4 reloads retained statics and derived key lists/sun; it is not a constructor or a map reload. Missing properties retain history. |
| Query-specific dependencies | All query arguments: points, normals, matrices/flags, bounds and per-vertex inputs; holder properties for nonnull slot 16; occupant type/interface answers for model-light queries; terrain height/normal results requested by §§4/5 | These are inputs to the requested comparison, not state recoverable from a lighting output. Capturing owner events alone does not establish foreign terrain/occupant behavior. |

**Ordering and safe use:** preserve delivery order as observed at the owner, including nested entries;
there is no lighting-wide atomic snapshot or prescribed global ordering between different message
subscribers. A shadow observer that reads a service at its own earlier/later callback cannot assume
the lighting callback saw the same value. The view's `0x44778233` callback reads the then-current
clock, not the event's posted hour/minute payload (§2.4). Do not sort a transcript by simulation date,
merge repeated time deliveries, or move property/settings reloads to city start. Preserve dirty-operation
begin/end and view changes interleaved with time/tint. Reproduce emitted dawn/dusk, fraction and terrain
notification events as expected results; do not feed those owner outputs back as inputs or redispatch
them to the native game. Replay runs only in the independent model; a read-only comparison must not
call the native owner's setters, Init/Shutdown, or send synthetic owner messages to create a checkpoint.
If the chosen observation facilities miss a direct tint call, settings byte, prior shared-property
history or delivery-time dependency, mark that scope unavailable instead of claiming whole-owner
exactness. Slots 14 and 16(null) can still be compared from known effective shared parameter history;
one resource read suffices only while no unrecorded reload/history changes those six properties.

Evidence for history-sensitive ordering: time and tint entries `0x007D9DF0–0x007DA099`,
`0x007D6AB0–0x007D6B1F`; suppression-byte read `0x007D99BA–0x007D99FA`; property refresh
`0x007DA7E0–0x007DAE64`; owner messages `0x007DBCF0–0x007DC19C`; see §§3/6 for altitude-copy
and notification ordering. These requirements close the input-contract question, not an implementation
or instrumentation completeness claim; inherited numeric environment and NaN encoding remain LT12/LT13.

## 2. Time of day *[compat]*

### 2.1 The time-dependent values (`SetTimeDependentLightingValues`, `0x007D9820`)

Let f = time × (1/24) and v = (month + 0.5) × (1/12).

- **Day light** `+0x98`: when the `+0xCC` map exists, sample it at (f, v) (below); else interpolate the day-light key
  list at f (below).
- When the settings object's `+0x0C` member has its byte `+0x39C` clear (that byte is the boolean render property
  `kRenderSnapshot`, boolean 28, default false: render-properties.md §3, §4; the offset is 0x20 × 28 + 0x1C): **fade colour** `+0x168` from the `+0xD0`
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

**Distance and precision (LT6):** subtract the current RGB components from the last-announced components,
square them, sum in the order blue squared plus green squared, then red squared, and take the square root.
All operands are binary32 fields, but these intermediate operations stay on the x87 stack without a
binary32 store. Compare that length directly with the binary32 threshold: only an ordered, strictly greater
length proceeds. Equality and unordered comparisons return false. There is no squared-threshold shortcut.
Evidence: `0x007D9E21–0x007D9E68` (including the square root at `0x007D9E55`).

1. If t is below the current time (the day wrapped), month := slot 15 of the city object at global `0x00B43DAC` (city
   slot 70) − 1 *[inferred: the month 1–12]*.
2. Time := t; §2.1.
3. If the distance between the global light and the last announced global light is **not above** the change threshold,
   return false.
4. If there is a terrain (`+0x14`):
   - wasNight := last announced day light red < T, isNight := day light red < T (T = `0x29189BF7`);
   - if they differ and the `kRenderSnapshot` byte (`+0x39C`, §2.1) is clear, post (message-server slot 4) `0xC9DA96E8` (dawn) when it
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

### 2.4 Normal view time driver (LT14)

`cSC4View3DWin::UpdateAmbientLightLevel` (`0x007E6E40`) obtains **GetVisualHour (clock slot 12),
then GetVisualMinute (slot 15)** from the city 24-hour clock (`cISC4City` slot 86; clock24.md §§1/8).
With a renderer and its lighting interface present it calls lighting slot 6 with
**binary32(hour + minute × c)**, where c is binary32 `0x3C888889`
(0.01666666753590107). Multiplication and addition stay in the inherited x87 environment until the
single binary32 argument store; do not first round the minute product to binary32. It requests renderer
slot 20 redraw only when SetTimeOfDay returns true. Evidence: reads `0x007E6E54–0x007E6E67`,
arithmetic/argument store `0x007E6E89–0x007E6EBD`, constant `0x00A94124`, redraw `0x007E6EC0–0x007E6ED2`.
Seconds and GetVisualTimePercentage are not used by this driver.

The view invokes this at post-city-init (`0x007ED1CD`) and on **VisualTimeTick `0x44778233`**
(`0x007F5551–0x007F556A`); it reads current visual hour/minute rather than the message payload.
Clock ticks follow real time even when the simulation date is paused (clock24.md §§3/4), so monthly
simulation-date sampling loses lighting deliveries. The clock's saved game time is also not its initial
visual time: visual time starts at noon and follows the loaded game time at a later clock step in
follow mode (clock24.md §4.9). Replay must preserve the initial noon delivery and subsequent tick,
not initialize lighting directly from the saved hour. Plugins may call slot 6 directly with other
arguments; those calls also belong in the transcript. The owner's month is initially zero and only
refreshes on a backwards time argument (§2.2), not at each simulation-month notification.

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
   - a network occupant (`0xA821EF94`) whose slot 8 (`HasNetworkFlag`, true when every given bit is set) answers true
     for `0x40000`, i.e. a piece on a **bridge step**, abutments included (network-geometry.md §10.3,
     network-geometry.md §10.3), so bridge decks and ramps are lit with the up normal and the default weight: out := light ⊙ (w·m + (1 − w)·white),
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

**Dimensions and zero horizontal direction (LT8):** the sweep always allocates the running vector to
vertex-count Z (`+0x138`), takes exactly vertex-count X (`+0x134`) rows, and advances each physical row by Z
floats. These counts are not exchanged for rotations 1 or 3. Rectangular inputs follow that same flat
X-by-Z scan; do not substitute a rotation-dependent rectangular traversal. Terrain copying uses §3's
strides independently. Evidence: `0x007DAEBF–0x007DAF32` and the row advance at the end of the sweep.

The drift is computed as **(−1 / sx) × sz**, then stored binary32; drop is
**((cellWidth / sx) × sy) × slopeFactor**, also stored binary32 (`0x007DAF5F–0x007DAF85`). With the ordinary
masked floating-point exceptions and sx = 0, drift is infinite or NaN and fails the ordered [−1,1] tests;
the sweep rectangle is retained. No special sx = 0 reset or rectangular-city rejection occurs. For an
executed sweep with no differences, the rectangle is **(−1, −1, 0, 0)**, not four −1 values.

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

**Upper endpoint precision (LT8):** ε is binary64 **0.001**, verified from the original image's data at
`0x00AA2270`. Subtract it from the binary32 extent in x87 precision. Compare the incoming binary32 coordinate
against that unrounded upper bound; only when the coordinate is greater, store the bound back to the
binary32 argument before forwarding it to terrain slot 27. Negative coordinates become positive zero.
There is no conversion to a cell index in this helper. For W = 1024 and x = 1024 the forwarded x is
**1023.9990234375**; x = 1023.9990234375 itself also compares above the extended-precision bound but stores
the same value. Evidence: `0x007D8340–0x007D83BA` (double subtraction at `0x007D8346`/`0x007D836D`,
binary32 replacement at `0x007D83A3`/`0x007D83B6`).

### 5.6 Shadow parameters and projections

- `GetShadowParams(a&, b&, colour&)` (slot 14, `0x007D6B70`): `0x085489CC`, `0x085489CD`, `0x085489CB`.
  Its user is the terrain overlay manager's two shadow passes, which treat `a` and `b` as two factors of one alpha
  test: when `b` is above 0 the shadows are drawn with the alpha test on (comparison greater-or-equal, reference
  `a × b`; the shipped 0.4 and 0.55 give 0.22), otherwise with it off and nothing else skipped (the code defaults
  0.5 and 0 therefore mean "off"); `colour`
  is the texture environment colour of the shadow (overlay-manager.md §4.6).
- `GetDecalShadowParams(holder, tex&, a&, b&, c&, pair&, colour&)` (slot 16, `0x007DA490`): the lighting values of
  `0x085489C8`, `0x085489CC`, `0x085489CD`, `0x085489CA`, `0x085489C9`, `0x085489CB`, each then overridden by the
  given property holder when it has them, so a model exemplar can change its own decal shadow. The first two values
  outside the texture are the same `0x085489CC` / `0x085489CD` pair as above. The remaining decal-only values,
  `0x085489C9` (a float pair) and `0x085489CA` (12.0), have no other reader in the executable, and no call of slot 16
  was found anywhere in it, so the shipped game does not appear to use them (LT4 stays open for the consumers that a
  plugin or an unlocated indirect call might have) *[inferred]*.
- `CalcShadowTerrainRect(…, box, outRect)` (slot 18, `0x007D6CC0`): the cell rectangle ⌊min / 16⌋ … ⌈max / 16⌉ of the
  box's x and z, intersected with the city and then extended by the ordered edge probes in §5.7; always true.
- `CalcShadowProjection(count, points, uvs, xform, out)` (slot 17, `0x007D8C80`): fits, by least squares, the
  texture coordinates of the points as an affine function of their position in a plane perpendicular to the shadow
  direction (expressed in the model's frame, `0x0080B9F0` building the plane basis), and writes the resulting texture
  matrix into `out`. The complete direction, mixed-precision fit, inverse-transform composition and output layout
  are specified in §5.8; false on the signed determinant test there. The flag byte gates composition as a whole.

### 5.7 Exact terrain rectangle extension (LT11)

Slot 18 takes **(unused word, unused word, box, outRect)**. The box is six binary32 values
(minX,minY,minZ,maxX,maxY,maxZ); minY is unused. The first two arguments are never read. Output is four
signed integers (minXcell,minZcell,maxXcell,maxZcell), with inclusive vertex-index endpoints, and the normal
return is always true. The method reads the **view-rotated** direction `+0x64`, not the unrotated sun or
the slope-factor-adjusted hill-shadow direction.

First obtain floor(minX/cellWidth), floor(minZ/cellWidth), ceil(maxX/cellWidth), ceil(maxZ/cellWidth), using
the stored inverse cell width for multiplication. For the finite city-scale domain the native rounding
sequence uses binary32 storage of `(q − 0.5) + 12582912` for floor, or `(q + 0.5) + 12582912` for ceil,
then integer bit-pattern subtraction by `0x4B400000`, with an exact-integer correction (+1 for floor,
−1 for ceil). This is not truncation toward zero. Intersect inclusively with [0,cellCountX] ×
[0,cellCountZ]. A disjoint intersection sets all four bounds to zero; its false result is ignored, and
extension still proceeds. The inclusive intersection helper is `0x0048ED90–0x0048EE0A`; the call and
rounding are `0x007D6CC0–0x007D6E06`.

Let the current bounds be L,T,R,B, direction (dx,dy,dz), cell width C, and binary32 slopes
px = dy/dx, pz = dy/dz. The two divisions occur unconditionally (`0x007D6E0B–0x007D6E24`), including zero
components. An X extension starts at height **maxY + (edge×C − corresponding box X face)×px**;
a Z extension starts at **maxY + (edge×C − corresponding box Z face)×pz**. Use the max box face for
increasing extensions and the min face for decreasing ones. Heights accumulate in x87 arithmetic; slopes
are stored binary32. Read the terrain-height grid, not the hill-shadow grid, at the current world-index
pair using §3's strides and base. There is no bilinear sample, fractional cross-axis drift, or edge-wide
ray sweep.

The following table is the complete ordered sequence. A two-probe extension keeps the height and moving
edge reached by the first probe; it does **not** restart at the original face for the second probe.
The last extension initializes its own height from its box face using the then-current edge index.

| Direction signs | First extension: probes in order | Last extension: probe |
|---|---|---|
| dx > 0, dz ≤ 0 | increase R at z=B, then z=T | decrease T at x=R |
| dx > 0, dz > 0 | increase B at x=L, then x=R | increase R at z=B |
| dx ≤ 0, dz ≤ 0 | decrease T at x=R, then x=L | decrease L at z=T |
| dx ≤ 0, dz > 0 | decrease L at z=T, then z=B | increase B at x=L |

For each probe, while the moving edge is inside its extension limit (R < cellCountX, B < cellCountZ,
L > 0, or T > 0), compare its current height with that grid vertex. Only **ordered height > terrain**
advances the edge by one. Add C×slope on increasing steps and subtract it on decreasing steps. Equality
or NaN stops the probe; reaching the city limit stops without sampling that boundary vertex. The stopped
vertex remains an output endpoint. Thus the output can include the first unshadowed vertex. The second
probe, when present, can continue after the first probe hits high ground. No clipping/expansion follows
extension, and untouched edges retain the initially intersected values. Evidence for the four branches:
`0x007D6E25–0x007D7147`, `0x007D7170–0x007D72BC`, `0x007D72BD–0x007D73D4`; common final output stores at
`0x007D7149–0x007D7168`.

### 5.8 Exact shadow projection (LT10)

The actual slot-17 argument order is **(unsigned count, points, uvs, transform, out)**, correcting the
earlier points-first summary. Points are contiguous three-binary32 vectors (12-byte stride); UVs are
contiguous two-binary32 vectors (8-byte stride). The transform and output are **separate pointers**.
Transform fields read are a flag byte at byte offset 1, nine binary32 matrix entries at offsets 4…36,
three translation entries at 40…48 and a scale at 52. Output is sixteen binary32 entries (64 bytes).
Evidence: transform reads `0x007D8C87–0x007D8D05`; count/array accesses `0x007D8F26–0x007D90F3`, and output
stores `0x007D96A1–0x007D9801`. The mathematical convention is defined below; do not treat the transform's
first word as a matrix component or require transform and output to alias.

The attributed basis helper accepts direction d and writes three consecutive three-float rows. With
nonzero horizontal length h = sqrt(dx²+dz²), row 0 is (dz/h,0,−dx/h), row 1 is d cross row 0, row 2 is d.
It does **not** normalize d or row 1. The length/reciprocal use x87 arithmetic; row 0 is stored binary32
before the cross product, whose components are also stored binary32. For a vertical direction, including
the zero vector, rows 0 and 1 are fixed **(0,0,1)** and **(1,0,0)**, independent of dy; row 2 still copies d.
Evidence: `0x0080B9F0–0x0080BAC6`. Thus the vertical fallback is not a general cross-product construction.

The determinant check is **signed**, with no absolute value. The assembled determinant remains on the
x87 stack and is compared directly with binary64 **0.01** at `0x00A92688`. An *ordered* value below that
constant returns false, before any output store; equality proceeds. The instruction branch also lets an
unordered (NaN) determinant proceed into reciprocal/composition, rather than returning false. The false
path leaves all 64 output bytes untouched. Count zero supplies a zero normal-equation matrix and takes
this false path, after reading the transform and obtaining/building the direction basis; it does not read
the point/UV arrays (`0x007D91CC–0x007D9205`, count skip `0x007D8F33–0x007D8F35`).

#### 5.8.1 Model-frame direction (LT10)

Name the nine transform floats at offsets 4…36 **R**, in three consecutive rows, and the byte at offset 1
**f**. Let **A = transpose(R)** when f has bit `0x04`, otherwise **A = R**. This is a conditional transpose,
not a general matrix inverse: no determinant, orthogonality test or normalization is performed. The
transpose swaps the three off-diagonal pairs (`0x007D8D13–0x007D8D4A`).

Slot 13 supplies **s**, copied directly from the manager's three floats at +0x64…+0x6C
(`0x007D6B50–0x007D6B65`). Negate all three components and form each direction component as
**dᵢ = binary32((Aᵢ₀ × (−sₓ) + Aᵢ₂ × (−s_z)) + Aᵢ₁ × (−s_y))**. The two additions occur in that
order in x87 registers; the products and intermediate sum are not first rounded to binary32. Each final
component is stored binary32 before the basis helper runs. Evidence: `0x007D8E36–0x007D8EC5`.

The direction does **not** use translation or the scale at +52, including when f has bit `0x02`.
The bit-2 inverse-scale path only changes separate local scale/translation fields
(`0x007D8DB1–0x007D8DE6`), and the direction reads only A. Do not divide d by that scale or normalize d
before building the basis. Unknown flag bits do not change this direction calculation; their later
composition effect is a separate issue.

#### 5.8.2 Arithmetic notation and plane coordinates

In the formulas below **q(v)** means a binary32 store. All other operations are separate x87 operations
in the parenthesized order, under the inherited control word, with no fused multiply-add. Neither this
entry nor its basis helper changes that control word. An unmarked intermediate stays in a register;
do not silently insert q, use binary64 as a universal substitute, reassociate, or exploit symmetry of the
normal-equation matrix. The worked examples use round-to-nearest, ties-to-even, and masked exceptions;
their finite results hold with either 53- or 64-significand-bit x87 intermediates.

For the nonvertical basis in §5.8, the precise horizontal setup is **h = sqrt(dx×dx + (−dz)×(−dz))**,
followed by **k = 1/(−h)**, both kept in x87 registers. Row B₀ is
**(q((−dz)×k), q(0×k), q(dx×k))**, and B₁ is the componentwise cross product d cross B₀, with each
product difference rounded once to binary32. B₂ copies d. The helper selects this path only for an
ordered −h < 0; zero or unordered −h takes the fixed fallback rows described above. In particular, the
zero in the ordinary B₀.y is obtained by multiplication, whereas fallback zeros are literal positive zero.
Evidence: `0x0080BA09–0x0080BAA5`; the binary32 constants are 0 at `0x00A81054` and 1 at `0x00B0EC64`.

For each input point p, in increasing unsigned array-index order, compute two stored plane coordinates:

| Coordinate | Ordered arithmetic, including the only binary32 store |
|---|---|
| a | q((B₀.x×p.x + B₀.y×p.y) + B₀.z×p.z) |
| b | q((B₁.z×p.z + B₁.y×p.y) + B₁.x×p.x) |

No translation or uniform scale is applied to the point before this fit. Evidence:
`0x007D8F50–0x007D8F96`. The two dot products have different addition orders.

#### 5.8.3 Accumulation and cofactor solve

Initialize all accumulators to positive zero from the shared zero at `0x00B4E334`. Maintain the following
**distinct** sums; their names specify storage, not alternative formulas for one shared value:

| Sums retained in x87 registers | Update for each (a,b) |
|---|---|
| X2 | X2 + q(a×a) |
| XYe | XYe + q(a×b) |
| Xe | Xe + a |
| Ye | Ye + b |
| N | N + 1 |

Separately update four binary32 memory sums: **XYf := q(q(a×b) + XYf)**,
**Y2f := q(q(b×b) + Y2f)**, **Xf := q(a + Xf)**, **Yf := q(b + Yf)**. The two copies of the xy sum
can differ, as can the copies of x and y. Do not replace this with a symmetric matrix accumulated in one
precision. For each input UV component w, keep three binary32 right-hand sums
**Wa := q(a×w + Wa)**, **Wb := q(q(b×w) + Wb)**, **W0 := q(w + W0)**.
The a×w product stays in x87 precision until its addition; the b×w product spills first. U and V sums
are independent. Evidence: product spills `0x007D8F9E–0x007D8FE4`, normal-sum updates
`0x007D8FEF–0x007D905A`, UV product spills and updates `0x007D9060–0x007D90E1`.

Conceptually the stored/retained matrix is
**H = [(X2,XYe,Xe); (XYf,Y2f,Ye); (Xf,Yf,N)]**. Calculate all nine **cofactors**, rounding each once:

| C | Ordered expression |
|---|---|
| C₀₀ | q(Y2f×N − Ye×Yf) |
| C₀₁ | q(Ye×Xf − N×XYf) |
| C₀₂ | q(Yf×XYf − Y2f×Xf) |
| C₁₀ | q(Xe×Yf − N×XYe) |
| C₁₁ | q(X2×N − Xe×Xf) |
| C₁₂ | q(XYe×Xf − X2×Yf) |
| C₂₀ | q(Ye×XYe − Xe×Y2f) |
| C₂₁ | q(Xe×XYf − X2×Ye) |
| C₂₂ | q(X2×Y2f − XYe×XYf) |

The determinant is **D = (C₀₂×Xe + C₀₁×XYe) + C₀₀×X2**, with no binary32 store before the signed
binary64 0.01 test above. This is a row-0 cofactor expansion in reverse column order. Cofactor stores:
`0x007D9101`, `0x007D9119`, `0x007D913B`, `0x007D914E`, `0x007D9161`, `0x007D917B`, `0x007D918E`,
`0x007D91A4`, `0x007D91C1`; determinant assembly `0x007D91C8–0x007D91DE`.

On success keep **r = 1/D** in x87 precision (`0x007D91FF`). The inverse coefficients K are the transposed
cofactors times r: **K₀ⱼ = Cⱼ₀×r** remain in registers without q, whereas
**K₁ⱼ = q(Cⱼ₁×r)** and **K₂ⱼ = q(Cⱼ₂×r)** are stored binary32, j=0,1,2.
Evidence: `0x007D9205–0x007D9267`. Then fit the three coefficients for each UV component:

| Coefficient | Ordered expression | Result storage |
|---|---|---|
| uₐ | (K₀₂×U0 + K₀₁×Ub) + K₀₀×Ua | q |
| u_b | (K₁₀×Ua + K₁₂×U0) + K₁₁×Ub | q |
| u₀ | (K₂₀×Ua + K₂₂×U0) + K₂₁×Ub | q |
| vₐ | (K₀₂×V0 + K₀₁×Vb) + K₀₀×Va | retain x87 |
| v_b | (K₁₀×Va + K₁₂×V0) + K₁₁×Vb | retain x87 |
| v₀ | (K₂₀×Va + K₂₂×V0) + K₂₁×Vb | q |

The asymmetric coefficient stores are native, even for identical U and V inputs. Evidence:
`0x007D926E–0x007D9338`. Lift the two fitted linear rows into model coordinates with these exact spills:

| Component | Binary32 result |
|---|---|
| u.x | q(uₐ×B₀.x + u_b×B₁.x) |
| u.y | q(q(uₐ×B₀.y) + u_b×B₁.y) |
| u.z | q(q(uₐ×B₀.z) + q(u_b×B₁.z)) |
| v.x | q(vₐ×B₀.x + q(v_b×B₁.x)) |
| v.y | q(vₐ×B₀.y + q(v_b×B₁.y)) |
| v.z | q(q(vₐ×B₀.z) + v_b×B₁.z) |

Evidence: `0x007D933F–0x007D9491`. These six rounded components, together with the two fitted constants,
feed the composition; they are not a matrix that can be replaced by an arbitrarily ordered multiplication.

#### 5.8.4 Inverse fields, flag composition and output

Read the transform translation as **t**, scale as **s**. The local matrix **A** is defined in §5.8.1.
Initially **T = −t** and **S = s**. When f has bit `0x04`, replace each component by
**Tᵢ = q((Aᵢ₀×(−t.x) + Aᵢ₂×(−t.z)) + Aᵢ₁×(−t.y))**. When f has bit `0x02`, next set
**S = q(1/s)**, then **Tᵢ = q(Tᵢ×S)** independently. Otherwise retain the initial scale without using
it in the result. These are the only inverse operations; the function does not invert a general matrix.
Evidence: transpose/translation `0x007D8D13–0x007D8DAD`, reciprocal scale `0x007D8DB8–0x007D8DE6`.

The later composition gate is **the entire byte f != 0**, not a test of bit 4, a masked set of known
flags, or a test that the numeric transform differs from identity (`0x007D935C`, `0x007D949C`).

- For **f = 0**, use the already rounded u and v rows, constants u₀/v₀, third row (0,0,1), third
  constant 0. Skip all matrix/translation composition and final scaling, even if R/t/s are nonidentity.
  The earlier direction calculation still used R.
- For **any nonzero f**, compute, for each column j=0,1,2,
  **u′ⱼ = q((A₂ⱼ×u.z + A₁ⱼ×u.y) + u.x×A₀ⱼ)** and
  **v′ⱼ = q((A₂ⱼ×v.z + A₁ⱼ×v.y) + v.x×A₀ⱼ)**.
  The third row comes from **w′ⱼ = (A₁ⱼ + A₀ⱼ)×0 + A₂ⱼ**: q the x and z components here,
  but keep the y component in x87 precision until its final output store. Do not simplify the zero
  products: nonfinite matrix entries can propagate through them.
  The three constants are **u′₀const = q(q((T.z×u.z + T.y×u.y) + u.x×T.x) + u₀)**,
  **v′₀const = q(((T.z×v.z + T.y×v.y) + v.x×T.x) + v₀)**,
  **w′₀const = q((T.y + T.x)×0 + T.z)**.
  In particular, the u translation dot product spills before addition of u₀; the v dot product does not.

Composition evidence: constants/zero products `0x007D94A4–0x007D956F`; column products/stores
`0x007D9576–0x007D9688`. For nonzero f with bit `0x02`, finally multiply **only the nine linear
components** by S and store each binary32. Eight components were already rounded as above; w′.y was
retained until this multiply/store. Do not multiply the constants again. Bit 4 controls the transpose
and translation rotation; bit 2 controls reciprocal scale; every other nonzero flag byte still enables
composition with all matrix and translation fields. No separate translation-presence bit is consulted.
Evidence: `0x007D95E0`, `0x007D969F–0x007D97D7`.

The output is a **column-major 4×4** float array. As mathematical rows it is
**[(u.x,u.y,u.z,uconst); (v.x,v.y,v.z,vconst); (w.x,w.y,w.z,wconst); (0,0,0,1)]** after the applicable
branch and scale. Thus flat indices 0/4/8/12 hold the u row, 1/5/9/13 the v row, 2/6/10/14 the w row;
3/7/11 are literal positive zero and 15 is literal 1. All sixteen entries are written on success and
the return boolean is true (`0x007D96A1–0x007D9811`). This is a texture-affine output with a separately
composed third row, not the original flagged transform layout. There is no allocation, bounds check,
singular-transform rejection, scale-zero rejection, or NaN recovery. Under masked exceptions, values
that reach arithmetic follow x87 behavior; an unordered determinant proceeds as already specified.

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

**Dirty rectangles and notifications (LT9):** the incoming rectangle is passed directly to terrain slot
15 as an inclusive *vertex-index* rectangle, with its starting grid address and the world-X/world-Z
strides. The owner does not enlarge it to neighboring cells. During a long operation it unions incoming
rectangles; end clears the flag before testing whether work is required. Both immediate and deferred
paths sweep the whole shadow grid. Only when the sweep's first changed row is nonnegative do they transform
the **sweep-changed** rectangle and call terrain slot 67. Deferred end then calls slot 68 with
(0,0,the same transformed rectangle,0). An executed no-change sweep causes neither call; a skipped sweep
retains the old rectangle and can therefore notify it again. There is no fallback to the incoming dirty
rectangle for the no-change case. Evidence: `0x007DBD36–0x007DBE12`, `0x007DBF3F–0x007DC030`.

For changed stored bounds (c0,r0,c1,r1) and world vertex counts X,Z, helper `0x007D6540` writes the following
world bounds. It only permutes/subtracts indices, with **no** cell expansion, +1 endpoint conversion or
clipping; rotations outside 0…3 leave the destination unchanged. Branch table data is at `0x007D65D0`.

| Rotation | Output (x0,z0,x1,z1) |
|---|---|
| 0 | (r0,c0,r1,c1) |
| 1 | (X−1−c1,r0,X−1−c0,r1) |
| 2 | (X−1−r1,Z−1−c1,X−1−r0,Z−1−c0) |
| 3 | (c0,Z−1−r1,c1,Z−1−r0) |

Slot 67/68 receives those integer endpoints verbatim. Any further vertex-to-cell update expansion belongs
to the terrain recipient, not to lighting; these calls do not establish that recipient's primitive output.

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

LT1 (the settings byte `+0x39C`, §2.1) and LT2 (the network occupant test `0x40000`, §4.3) are answered and
archived in [questions/answered/lighting.md](questions/answered/lighting.md). Still open:
[questions/lighting.md](questions/lighting.md) LT3 (what the fade term is for on screen, §3) and LT4 (the decal
shadow values `0x085489C9` / `0x085489CA`, §5.6; their only reader is slot 16).

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
10. **RGB change discriminator (LT6):** threshold = binary32 0.0001; last-announced RGB = (0,0,0),
    current RGB = (binary32 0.00008, binary32 0.00008, 0). The length exceeds the threshold, so the call
    proceeds (and returns true even without terrain). A squared-distance comparison with 0.0001 would fail.
    With current RGB = (threshold,0,0), equality returns false. A NaN component also returns false.
11. **Stored padding discriminator (LT7):** width = height = 1, stride = 2, vector contains a red-255 active
    pixel followed by a red-0 padding pixel. Minimum red is 0; one white element of unused capacity beyond
    vector end must have no effect. With vector end equal to begin, Init's stored minimum is 1.0.
12. **Normal clamp discriminator (LT8):** a terrain spy receiving (1024, −1) for a 1024-by-1024 world must
    see (1023.9990234375, +0), with its output otherwise passed through unchanged.
13. **Constructor discriminator (LT5):** construct in storage prefilled with a nonzero sentinel. Count is 0;
    current global/day light is white, announced global/day light and all six cached colours are black;
    day/night fractions are 1/0; city geometry and `+0xCC/+0xD0` retain the sentinel. Successful queries of
    IID 1 then IID `452294AA` acquire two references total and return base then base+4. Unsupported IID leaves
    an output sentinel unchanged. After balancing to count 1, one Release deletes and returns zero.
14. **Missing image/lifetime discriminator (LT7):** make buffer Load return false for each of the four maps,
    with an otherwise usable exemplar/settings/message service. First Init returns true with null maps;
    second Init returns false and performs no reload/subscription. First Shutdown unsubscribes and returns
    true; second returns false. Destruction without Shutdown must not be used as evidence of map cleanup.
15. **Rectangular sweep discriminator (LT8):** vertex counts X=3,Z=5 always give three physical rows of five
    floats, at every rotation. With zero initialized shadow/terrain arrays and a valid sweep, the changed
    rectangle is (−1,−1,0,0). Following a prior nonempty changed rectangle, sun (0,−1,0) skips and retains it
    under masked floating-point exceptions.
16. **Dirty notification discriminator (LT9):** X=9,Z=5, changed stored rectangle (1,2,3,4) transforms at
    rotation 1 into (5,2,7,4). Slot 67 receives those exact endpoints. Long-operation end also calls slot 68
    with (0,0,the same rectangle,0). Repeat with an executed no-change sweep: neither slot is called.
17. **Edge-probe discriminator (LT11):** C=16, city cell counts 8×8, box (16,0,16,32,32,32), direction
    (1,−1,−1), flat height 0 everywhere. Initial bounds are (1,1,2,2). Increasing R probes z=2: heights 32,
    16,0 at R=2,3,4; the zero stops at R=4. The z=1 probe also stops at (4,1). Decreasing T then starts
    at height 32 and moves from 1 to boundary 0. Output is **(1,0,4,2)**. With height 32 at (2,2) alone,
    the first probe stops immediately, but the second continues from (2,1) to R=4: the same output results.
    Sampling every top-edge ray, excluding the stopped vertex, or comparing ≥ instead of > would not
    implement the specified procedure.
18. **Projection failure and vertical fallback (LT10):** with count 0, null points/UVs, a valid fully initialized
    transform, a manager supplying a finite direction, and a sentinel-filled 64-byte output, return false
    without modifying any output byte. The transform/direction work still occurs. Separately, the basis
    helper for d=(0,−1,0) has rows (0,0,1), (1,0,0), (0,−1,0); computing row 1 as d cross row 0 in this
    special case would incorrectly give (−1,0,0).

19. **Model-frame direction discriminator (LT10):** set the manager's direction to (1,−2,0), and transform
    rows R=(0,0,1), (0,1,0), (−1,0,0), translation (8,16,32), scale 2. With f=0 or 2 the helper receives
    **d=(0,2,1)** and produces rows (1,0,0), (0,1,−2), (0,2,1). With f=4 or 6 it receives
    **d=(0,2,−1)** and produces rows (−1,0,0), (0,1,2), (0,2,−1). All displayed nonzero components
    are exact binary32 values; zeros here are compared numerically. This independently distinguishes
    transpose selection, omission of translation/scale, the negation, and absence of normalization.
    It is an intermediate native arithmetic vector, not a claim of an observed game run.
20. **Whole-byte flag and inverse-composition discriminator (LT10):** direction s=(0,−1,0); points
    (0,0,0), (1,0,0), (0,0,1); UVs (0,0), (1,0), (0,1). Transform R has rows (0,0,1), (0,1,0),
    (−1,0,0), t=(8,16,32), scale=2. Both direction branches are vertical, so the fallback yields a=z,
    b=x; the independent affine fit is u=b, v=a, with D=1. Expected output **mathematical rows** below
    must be flattened column-major as in §5.8.4. The last row is always (0,0,0,1).

    | f | u row | v row | w row |
    |---|---|---|---|
    | 0 | (1,0,0,0) | (0,0,1,0) | (0,0,1,0) |
    | 1 or 128 | (0,0,1,−8) | (−1,0,0,−32) | (−1,0,0,−32) |
    | 2 | (0,0,0.5,−4) | (−0.5,0,0,−16) | (−0.5,0,0,−16) |
    | 4 | (0,0,−1,32) | (1,0,0,−8) | (1,0,0,−8) |
    | 6 | (0,0,−0.5,16) | (0.5,0,0,−4) | (0.5,0,0,−4) |

    All nonzero results are exact binary32. This distinguishes whole-byte gating, unknown flag bits,
    transpose selection, inverse translation, reciprocal scale, and unscaled constants. Zeros in this
    table are numerical zeros; signed-zero outcomes follow the arithmetic schedule.
21. **Product-spill rejection (LT10):** f=0, identity R, direction s=(0,0,−1); points
    (4096,0,0), (4097,0,0), (4096,1,0); UVs (0,0), (1,0), (0,1). Basis coordinates are exactly a=x,
    b=y. Exact arithmetic gives Σa²=50,339,841 and determinant **1**. Native q(4097²)=16,785,408
    instead of 16,785,409, giving X2=50,339,840. With XYe=XYf=4096, Xe=Xf=12289, Ye=Yf=Y2f=1,
    N=3, the first-row cofactors are (2,1,−8193), and
    **D = (−8193×12289 + 1×4096) + 2×50339840 = −1**. Return **false**, leaving every output byte
    unchanged. A generic least-squares fit of this noncollinear triangle would succeed; omitting the
    per-product binary32 spill changes observable success/failure.
22. **Different U/V translation rounding with identical UV columns (LT10):** f=1, identity R,
    t=(16777216,0,1), direction s=(0,−1,0), points (0,0,0), (1,0,0), (0,0,1), UVs (1,1), (2,2),
    (2,2). D=1; both fitted model rows are exactly (1,0,1), both constants 1. The translation dot product
    is −16777217. U rounds it first to −16777216, then adds 1: **uconst=−16777215**. V adds 1 before
    storing: **vconst=−16777216**. The complete output rows are **(1,0,1,−16777215)**,
    **(1,0,1,−16777216)**, **(0,0,1,−1)**, **(0,0,0,1)**, stored column-major. Rounding only the final
    sum, or imposing one uniform rounding policy on both fitted rows, fails this vector.

23. **Shipped texture and retention (LT15):** start from §1.4's process-initial configuration. Read
    the shipped exemplar, then query slot 16 with a null holder: texture **`0x69D3A23B`**, a/b/c are
    binary32 0.4/0.55/12, pair (0,0), colour binary32 (0.08,0.06,0.23). A subsequent reload with
    `085489C8` absent retains `0x69D3A23B`; it does not revert to `0x0917660E`. A nonnull holder
    supplying `085489C8 = 0x12345678` changes only that call's texture result to `0x12345678`;
    the next null-holder call still returns `0x69D3A23B`. The independent expected value comes from
    resource properties and retention history, never the getter under test.
24. **Driver reads current clock, not posted payload (LT14):** under nearest-even arithmetic, deliver
    VisualTimeTick with posted hour/minute (12,0), but let the clock reads at the view callback return
    visual hour/minute (12,30). Expected lighting argument is **12.5**, one slot-6 call. Changing
    visual seconds within that minute does not change the argument. If the renderer's lighting
    interface is absent there is no slot-6 call. Keep both outcomes distinct from a direct plugin call.
25. **Loaded time and month history (LT14):** a new owner starts at time 12, cached month 0. A saved
    game clock of 21:00 does not seed this owner. Let the normal initial visual reads be (12,0),
    then a follow-mode tick read (21,1): arguments are **12** then binary32(21 + c), with c from
    §2.4. Neither call reads the city month because neither argument is below stored time. Now deliver
    (0,1) with the city month service returning 7: the month query occurs once and cached month
    becomes **6**. A second identical (0,1) delivery does not query month, even if the service now
    returns 8. Calls returning false still retain their new time/month and recomputed colours.
26. **Same current inputs, different replay histories (LT14):** use valid geometry W=D=1024,
    rotation 0, zoom 0 and no fade map, with §1.4's four fade-strength keys and midday strength
    0.5 (an exact binary32 test input). In history A apply the view at noon (s=0.5), then change
    time to midnight (s=0). In history B change to midnight first, then apply the same view.
    Current time, view and properties agree, but the cached z-gradient is **−1/2048** in A and
    **0** in B. A snapshot of those current inputs cannot reconstruct both. A further identical
    view refresh makes both gradients 0. Similarly, preserve long-operation begin/edit/end order
    and copy heights at the deferred end (§6/vector 16), not at the earlier edit notification.
27. **Shared-state lifetime discriminator (LT14):** from process-initial configuration, deliver a
    property read setting `085489C8 = 0x12345678`, shut down and destroy that owner, then create
    and Init a second owner whose exemplar omits that property. Its null-holder slot-16 texture is
    **`0x12345678`**, whereas a fresh process with the same second exemplar yields **`0x0917660E`**.
    A replay beginning at the second constructor without prior shared history must report incomplete
    input coverage. It must not silently select either value or seed the expectation from slot 16.

## 12. Individual owner and attributed-helper checklist

The exact lighting inventory is **43 cSC4LightingManager bodies / 21,714 bytes** and
**2 cImageARGB8 bodies / 517 bytes**, measured against the native export's body sizes. The former
includes two adjustment thunks and both destructors; shared AddRef bodies are dependencies, not extra
lighting bodies. Each row below identifies an individual entry, even where several entries share a section.
Existing sections describe baseline behaviour; open LT items explicitly withhold complete-owner credit.
The export's original names alone omit manually attributed entries; this inventory uses the export addresses
plus `re/symbols/lighting.tsv`, not a name-only export count.

| Entry | Behaviour section | Remaining exact-contract review |
|---|---|---|
| `007D65E0` QueryInterface | §1.1, §1.2 | LT5 answered |
| `007D6650` Release | §1.1, §1.2 | LT5 answered |
| `007D6670` GetTimeOfDay | §2 | baseline |
| `007D6680` IsNight | §2 | baseline |
| `007D68B0` BendShadowedNormal | §5.3 | baseline |
| `007D6950` ShadowDepth | §5.1, §5.3 | baseline |
| `007D6A00` HeightAboveLand | §5.1, §5.3 | baseline |
| `007D6AB0` SetLightingTint | §2.3 | baseline |
| `007D6B00` ResetLightingTint | §2.3 | baseline |
| `007D6B20` GetGlobalLight | §4.6 | baseline |
| `007D6B50` GetShadowDirection | §4.6 | baseline |
| `007D6B70` GetShadowParams | §5.6 | baseline |
| `007D6BB0` GetShadowStrengthFromLocation | §5.4 | baseline |
| `007D6CC0` CalcShadowTerrainRect | §5.7 | LT11 answered |
| `007D73E0` GetColor(out) | §4.1 | baseline |
| `007D7420` GetColor(bytes) | §4.1 | baseline |
| `007D7460` GetColor(out, normal) | §4.1 | baseline |
| `007D7730` GetColor(normal, bytes) | §4.1 | baseline |
| `007D77C0` GetColor(out, normal, position) | §4.1 | baseline |
| `007D7840` GetColor(position, normal, bytes) | §4.1 | baseline |
| `007D7940` GetTerrainColor | §4.5 | baseline |
| `007D7AE0` GetTerrainColorFloor | §4.5 | baseline |
| `007D7BF0` GetVertexColors | §4.4 | baseline |
| `007D7F00` GetTerrainVertexColors | §4.5 | baseline |
| `007D8340` GetTerrainNormalClamped | §5.5 | epsilon/upper rounding proved |
| `007D83C0` GetModelColor | §1.5, §4.2 | LT7 answered |
| `007D8550` GetModelLight | §4.3 | baseline |
| `007D8B20` GetTerrainLighting | §4.5 | baseline |
| `007D8C80` CalcShadowProjection | §5.6, §5.8.1–§5.8.4 | LT10 native arithmetic/flags/layout proved; vectors 18–22 |
| `007D9820` SetTimeDependentLightingValues | §1.5, §2.1 | LT7 answered |
| `007D9DF0` SetTimeOfDay | §2.2 | LT6 proved |
| `007DA0A0` constructor | §1.1, §1.2 | LT5 answered |
| `007DA3E0` QueryInterface adjustment thunk | §1.2, §7 | LT5 answered |
| `007DA3F0` Release adjustment thunk | §1.2, §7 | LT5 answered |
| `007DA400` destructor | §1.3, §1.5 | LT5/LT7 answered |
| `007DA490` GetDecalShadowParams | §5.6 | baseline |
| `007DA670` deleting destructor | §1.3, §1.5 | LT5/LT7 answered |
| `007DA690` Shutdown | §1.3, §1.5 | LT7 answered |
| `007DA7E0` ReadProperties | §1.4 | LT5 answered |
| `007DAE70` FindHillShadows | §5.2 | LT8 answered |
| `007DB6A0` Init | §1.3, §1.5 | LT7 answered |
| `007DB840` DoZoomAndRotationChange | §1.2, §3 | LT5/LT8 answered |
| `007DBCF0` DoMessage | §6 | LT9 answered |
| `007D6690` cImageARGB8::SampleRowLerp | §1.5, §2.1 | LT7 answered |
| `007D6870` cImageARGB8::FindMinRedComponentValue | §1.5 | vector boundary/constant proved |

Attributed helpers are `007D6540` RotateLightingRectToWorld (§6), `007DB450` CopyImageToARGB8 (§1.5),
`007DB4D0` CreateColorMap (§1.5), `007DB570` MakeTestPattern (§1.5), `007F6CC0` RotateVectorByViewRotation (§3),
and `0080B9F0` BuildBasisFromDirection (§5.6). The shared CRT `_CIfmod` at `009F068A` is a numeric dependency
of model lighting (§4.3), not a lighting-owned primitive. Foreign terrain normal/height reads, image decode
and property/message services remain dependency boundaries; this spec does not prescribe their outputs.
