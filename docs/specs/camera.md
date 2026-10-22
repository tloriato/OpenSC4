# Spec: camera and projection (`cSC4CameraControl`, `cS3DCamera`)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`). Implementers must work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled
or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[compat-limit]* depends on something FreeCity4 cannot reproduce, so only the rule is matched.

Related specs: [INVENTORY-M6.md](INVENTORY-M6.md) (§3.3, the camera, zoom and rotation overview; §9 rank 3 proposes
this spec), [graphics-formats.md](graphics-formats.md) (§4.2, the model keys per zoom and rotation),
[zones.md](zones.md) (§5.2, a ViewZRChange3 subscriber), [foundations.md](foundations.md) (§5.1, messages; §18, the
x87 precision of the float maths), [save-records.md](save-records.md) (§3.2, the camera control's class record).

## 0. Summary

The city view looks at the ground with an **orthographic** camera. `cSC4CameraControl` (class `0xC9C628EC`) owns the
view state: a view target on the ground, a zoom (0–4, with a fractional magnification above 4), a rotation (0–3)
and the city size; from them it sets the projection box and the eye of a `cS3DCamera` (class `0xE9C6262A`), keeps
the target inside scroll bounds, converts between world points and a 2D "camera plane", and announces every change
with view messages. `cS3DCamera` is a general camera: viewport, subview, an orthographic or perspective projection
matrix with a sub-pixel offset, a look-at view transform, and `Project` / `UnProject` / `GetEyeRay`.

World coordinates: x and z on the ground (metres, a cell is 16 m), y up. Screen coordinates: pixels, y down.

## 1. `cS3DCamera` *[compat]*

### 1.1 State (0x1A8 bytes; vtable `0x00ABD2C4`)

| Offset | Field | Default |
|---|---|---|
| `+0x04` | reference count | 0 |
| `+0x08`, `+0x0C` | viewport width, height (float pixels) | 100, 100 |
| `+0x10` | depth size | 65536 |
| `+0x14` | subview in use (bool) | false |
| `+0x18`, `+0x1C`, `+0x20`, `+0x24` | subview x, y, width, height | 0, 0, 100, 100 |
| `+0x28` | the **local transform** (camera to world): a change counter (byte), flags `+0x29` (1 translated, 2 scaled, 4 rotated), a 3×3 rotation `+0x2C`…`+0x4C` whose columns are the camera's right, up and back axes, the position `+0x50`…`+0x58`, a uniform scale `+0x5C` | identity |
| `+0x60` | the **view transform** (world to camera), same layout, rebuilt from the local one when its counter differs (§1.3) | identity |
| `+0x9C`…`+0xD8` | the projection matrix (4×4, row-major, column vectors: `p' = M p`) | |
| `+0x11C`…`+0x158` | its inverse | |
| `+0x160` / `+0x170` | the last look-at target and up vector | |
| `+0x17C` | projection change counter (the matrix is rebuilt when it differs from `+0x15C`) | |
| `+0x180` | projection type: 2 perspective, anything else orthographic (`SetOrtho` stores 1) | 0 |
| `+0x184`, `+0x188` | near, far | 1, 2 *[inferred]* |
| `+0x18C`, `+0x190`, `+0x198`, `+0x194` | left, right, bottom, top | a unit box *[inferred]* |
| `+0x19C`, `+0x1A0` | pixel offset x, y | 0, 0 |
| `+0x1A4` | depth bias, set by `SetDepthBias` (§1.2) | 0 |

### 1.2 Methods

| Method | Address | Behaviour |
|---|---|---|
| `GetGZCLSID()` | `0x007FF010` | `0xE9C6262A` |
| `QueryInterface(iid, out)` (slot 0) | `0x007FF040` | IIDs 1 and `0xE9C6262A` |
| `Release` (slot 2) | `0x007FF020` | count ≤ 1: slot 3 with 1 and return 0; else decrement |
| deleting destructor (slot 3) | `0x007FF130` | frees to the global heap on flag bit 0 |
| `SetDepthBias(b)` | `0x007FF0F0` | `+0x1A4 := b` and the projection change counter `+0x17C` is bumped; the 3D draw context's `SetDepthOffset(n)` passes `kDepthOffset` × n (draw-context.md §8) |
| constructor / factory | `0x007FF930` / `0x00800320` | the defaults of §1.1 (0x1A8 bytes from the global heap) |
| `SetViewport(w, h)` | `0x007FF150` | viewport := (w, h); subview := (0, 0, w, h), not in use; projection counter += 2 |
| `SetSubview(x, y, w, h)` | `0x007FF080` | subview := (x, y, w, h), in use; projection counter += 1 |
| `SetDepthSize(d)` | `0x007FF070` | `+0x10 := d`; projection counter += 1 |
| `SetOrtho(l, r, b, t, n, f)` | `0x007FF2E0` | the box; type := 1; projection counter += 1 |
| `SetPixelOffset(dx, dy)` | `0x007FF2C0` | `+0x19C`, `+0x1A0`; projection counter += 1 |
| `GetViewVolume(&l, &r, &t, &b, &n, &f)` | `0x007FF340` | the box shifted by the pixel offset: `l + δx`, `r + δx`, `t − δy`, `b − δy`, n, f, with `δx = (r − l) / viewportWidth × dx` and `δy = (t − b) / viewportHeight × dy` |
| `SetPosition(p)` | `0x007FF250` | local position := p, flag 1, counter += 1 |
| `GetPosition(&p)` | `0x007FF230` | the local position |
| `SetLookAt(target, up)` | `0x007FFD60` | §1.3; flag 4, counter += 2; remembers target and up |
| `GetLookAt(&p)` | `0x007FF280` | the remembered target |
| `ResetXforms()` | `0x007FF190` | each transform that has flags becomes the identity (flags 0, counter + 1) |
| `LocalXform()` / `ViewXform()` | `0x007FF0E0` / `0x007FFED0` | the transforms; `ViewXform` rebuilds the view transform first if needed |
| `UpdateViewXform()` | `0x007FF3D0` | §1.3 |
| `ProjectionMatrix()` / `_UpdateProjection()` | `0x007FFEF0` / `0x007FF580` | §1.4 |
| `GetProjectionType()` / `GetViewState()` | `0x007FF110` / `0x007FF3C0` | `+0x180` / the local counter |
| `Project(world, &screen)` | `0x007FFF10` | §1.5 |
| `UnProject(screen, &world)` | `0x00800120` | §1.5 |
| `GetEyeRay(screenXY, &origin, &direction)` | `0x00800340` | §1.6 |

### 1.3 Look-at and the view transform

`SetLookAt(target, up)`: `back := normalize(position − target)`, `right := normalize(up × back)`,
`trueUp := back × right`; the local rotation's columns are (right, trueUp, back). So the camera looks along
`−back`, right-handed.

The view transform is the inverse of the local one: rotation transposed (when the local one is rotated), translation
`−Rᵀ · position`, and, when the local one is scaled, scale `1 / s` with the translation multiplied by it. A world
point `p` in camera space is `v = (R_view · p) × s_view + t_view`.

### 1.4 The projection matrix

With `sx = 1 / (r − l)`, `sy = 1 / (t − b)`, `sz = 1 / (f − n)`, `px = 1 / subviewWidth`,
`py = 1 / subviewHeight`, the offsets `ox = −2 · px · dx` and `oy = −2 · py · dy` (the pixel offset as clip-space
units), and the bias `β = −(+0x1A4 / depthSize)`; if the subview is in use, first `sx := sx · (px · viewportWidth)`,
`ox := (px · viewportWidth − 1) − 2 · px · subviewX + ox`, and the same for y.

- **Orthographic** (type ≠ 2): `m00 = 2 sx`, `m03 = ox − (l + r) sx`; `m11 = 2 sy`, `m13 = oy − (b + t) sy`;
  `m22 = −2 sz`, `m23 = β − (n + f) sz`; `m33 = 1`; the rest 0. This is `glOrtho` plus the pixel offset.
- **Perspective** (type 2): `m00 = 2n sx`, `m02 = (l + r) sx − ox`; `m11 = 2n sy`, `m12 = (b + t) sy − oy`;
  `m22 = −(n + f) sz − β`, `m23 = −2n f sz`; `m32 = −1`, `m33 = 0` (`glFrustum`).

The inverse is stored alongside (diagonal reciprocals and the matching offsets).

### 1.5 `Project` and `UnProject`

`Project(p)`: `v` as in §1.3, `c = M · (v, 1)`; if `|c.w| < 0.0001`, false. Otherwise `ndc = c / c.w` and
`screen.x = (ndc.x + 1) · subviewWidth / 2 + subviewX`, `screen.y = (1 − ndc.y) · subviewHeight / 2 + subviewY`,
`screen.z = ndc.z`; true.

`UnProject(s)`: `ndc.x = 2 (s.x − subviewX) / subviewWidth − 1`, `ndc.y = 2 (subviewY + subviewHeight − s.y) /
subviewHeight − 1`, `ndc.z = s.z`; `c = M⁻¹ · (ndc, 1)`; false when `|c.w| < 0.0001`; else `v = c / c.w`, then the
inverse view transform: subtract the view translation, divide by the view scale, multiply by the transposed view
rotation.

### 1.6 `GetEyeRay(s)`

- Orthographic: `d := target − position` (the look-at target); `q := UnProject(s.x, s.y, 0)`; if `|d|² > 0`, `d` is
  scaled to the length `far`; `origin := q − d`, `direction := d`. The ray runs from one far-distance behind the
  picked point through it.
- Perspective: from the whole viewport (not the subview), `(nx, ny) = (2 s.x / w − 1, 2 (h − s.y) / h − 1)`;
  `origin :=` the camera position; `direction := −R · (nx, ny, −near)` with the local rotation `R`.

## 2. `cSC4CameraControl` *[compat]*

### 2.1 State (0x160 bytes; vtable `0x00ABAD18`)

| Offset | Field | Default |
|---|---|---|
| `+0x04` | initialised | false |
| `+0x05` | loaded from a save | false |
| `+0x08` | reference count | 0 |
| `+0x0C` | the attached `cS3DCamera` (counted) | null |
| `+0x10`…`+0x18` | the camera-plane origin (rotation corner + eye offset) | |
| `+0x1C`…`+0x24` | the rotation corner (§2.3) | |
| `+0x38`…`+0x40` | the eye position | |
| `+0x44`…`+0x4C` | the **view target** | |
| `+0x50`…`+0x58` | the view-target velocity | 0 |
| `+0x5C`…`+0x7C` | the view direction and the camera-plane axes (copied from the view transform) | |
| `+0xB0`, `+0xB4` | the position tweak (pixels, §2.3) | 0, 0 |
| `+0xB8`…`+0xC8` | scroll step per zoom: 100, 50, 25, 12.5, 6.25 (m) | |
| `+0xF0` | custom magnification | 1 |
| `+0xF4`, `+0xF8`, `+0xFC` | minimum zoom 0, maximum zoom 4, magnification limit 5 | |
| `+0x100`, `+0x104` | previous zoom and rotation | 0, 0 |
| `+0x108`, `+0x10C` | **zoom** (0–4) and **rotation** (0–3) | 0, 0 |
| `+0x110`, `+0x114` | new zoom and rotation (as sent in the messages) | 0, 0 |
| `+0x118` | yaw base −π/4 | |
| `+0x11C` | the current pitch (π/6 at construction) | |
| `+0x120` | the eye's horizontal distance, 8192 m | |
| `+0x124`, `+0x128` | near and far distances | |
| `+0x12C`, `+0x130` | viewport size (int pixels) | 800, 600 |
| `+0x134`, `+0x138` | metres per pixel and pixels per metre | 1, 1 |
| `+0x13C`…`+0x144` | the eye offset from the target | |
| `+0x148` | scroll bounds in use | true |
| `+0x14C`…`+0x154` | the scroll-bounds polygon (a vector of 2D points) | empty |
| `+0x158`, `+0x15C` | city width and depth (m) | 4096, 4096 |

### 2.2 Zoom and rotation → projection (`UpdateCameraZoomAndRotationParams`, `0x007CBE40`)

Tables (by zoom 0–4): **cell width in pixels** 8, 16, 32, 73, 146 (`0x00ABACE0`); **pitch** 30°, 35°, 40°, 45°, 45°
(`0x00ABACCC`).

1. `pixels := cellPixels[zoom] × magnification`; `cellSpan := 16 · (sin 22.5° + cos 22.5°) ≈ 20.90501`
   (`SC4ViewUtilities::GetCityWidthFromOrthoView(16, zoom)`, angle table `0x00ABCFC4`, −22.5° at every zoom):
   metres per pixel `+0x134 := cellSpan / pixels`, pixels per metre `+0x138 :=` its reciprocal. At magnification 1:
   2.613126, 1.306563, 0.653281, 0.286370, 0.143185 m per pixel.
2. `yaw := (rotation + 1) · π/2 − π/4`; `pitch := pitch[zoom]` (stored at `+0x11C`).
3. `d := 8192 / cos(pitch)`; with `W, H :=` the city size, each at least 4096: near `:= (d − √(W² + H²) / cos(pitch))
   × 0.95`, far `:= d × 1.5`.
4. `SetOrtho(−a, a, −c, c, near, far)` with `a = viewportWidth × metresPerPixel × 0.5` and
   `c = viewportHeight × metresPerPixel × 0.5`.
5. Eye offset `:= (cos(yaw) · 8192, tan(pitch) · 8192, sin(yaw) · 8192)` (the y term is `sin(pitch) · d`).

### 2.3 Eye placement and pixel snapping (`UpdateCameraPositionParams`, `0x007CC290`)

1. **Rotation corner** (`0x007CBA10`): for rotation 0, 1, 2, 3 the city corner (0, 0, 0), (W, 0, 0), (W, 0, H),
   (0, 0, H). The camera-plane origin is that corner plus the eye offset `o`.
2. `k := 1 − ((target − corner) · o) / |o|²` (0 when `|o|² = 0`); **eye := target + k · o**. Every eye position lies on
   the plane through `corner + o` perpendicular to `o`, so the depth range is the same everywhere.
3. `camera.SetPosition(eye)`, `camera.SetLookAt(target, (0, 1, 0))`, pixel offset (0, 0).
4. **Snapping**: project the rotation corner, round its screen x and y to the nearest integer (the 1.5·2²³ addition),
   and set the pixel offset to `(x − round(x) + tweakX, −(y − round(y)) + tweakY)`. The city is therefore drawn on
   the same pixel grid at every scroll position *[compat]*.
5. Copy the view direction and camera-plane axes from the view transform, build the camera plane's equation through
   the eye, and keep the target's camera-plane point.

### 2.4 Camera-plane conversions

- `ConvertWorldPointToCameraPlanePoint(p)` (`0x007CBCE0`): `(p − planeOrigin)` dotted with the view transform's first
  and second rows, times the view scale: a 2D point in metres.
- `ConvertCameraPlanePointToWorldPoint(q)` (`0x007CBD70`): the inverse, a point on the plane through the
  plane origin.
- `IntersectWithGroundPlane(direction, origin)` (`0x007CBA90`): false when `|direction.y| < 0.0001`; else
  `t := −origin.y / direction.y`; false when `t < 0`; else the point `origin + t · direction` (on y = 0).

### 2.5 Scroll bounds

`UpdateScrollBounds` (`0x007CCC60`, only when bounds are in use): the four city corners (`SC4ViewUtilities::
GetCityCornerPosition`, corners 1, 0, 2, 3) in camera-plane coordinates `A, B, C, D`, and
`lift := (o.y × 0.48) / viewRow1.y − metresPerPixel × viewportHeight × 0.5`. The polygon is six points:
`(A.x, A.y + lift)`, `(B.x − 500, B.y + lift)`, `(B.x − 500, B.y)`, `(D.x, D.y)`, `(C.x + 500, C.y)`,
`(C.x + 500, C.y + lift)`: the city's outline widened by 500 m left and right and raised by `lift`
*[inferred: the screen-space meaning of `lift`]*.

`ClampScrollToBounds(from, to, &out)` (`0x007CC650`): with bounds off or empty, `out := to`. Otherwise the move from
`from` towards `to` is cut at the polygon edge it crosses first (the smallest crossing fraction over the edges whose
outer side `to` lies on; edges shorter than 1e-8 are skipped), stopping 0.0001 of the move short when the fraction is
at least 0.001; the rest of the move is then projected onto that edge's direction and the process repeats, at most as
many times as the polygon has edges, until a step crosses nothing (`out :=` that point). When it runs out of steps,
`out := from` *[inferred: the exact sliding rule; the bytes were read but not run]*.

`ClampTargetToScrollBounds` (`0x007CD000`): clamp the target's camera-plane point (from itself to itself, so only an
outside target moves); if it moved by at least 0.0001 in either axis, convert back to a world point and intersect
the view direction with the ground plane: the result becomes the target and the velocity is zeroed; then
`UpdateCameraPositionParams`, CameraUpdated, ViewTChange2, ViewTChange3 (§2.7).

### 2.6 Commands

| Method | Address | Behaviour |
|---|---|---|
| `Init(target)` | `0x007CDB20` | once: if loaded from a save, the new zoom/rotation := the loaded ones, else `target :=` the argument; `AttachToCamera`, §2.2, §2.3, scroll bounds, PostViewInit; initialised := true; true |
| `AttachToCamera(cam)` | `0x007CD200` | a different camera: keep it (counted), `SetViewport(800×600 or the size set)`; if initialised: §2.2, §2.3, bounds, PostViewInit |
| `DetachFromCamera()` | `0x007CB970` | release the camera |
| `SetZoom(z)` (float) | `0x007CDC50` | if `4 < z < 5`: magnification `:= 2^(z − 4)` (CRT `pow`) and `z := 4`; else magnification 1. Refused (false) for `z` outside 0…4 and when the integer zoom and the magnification are unchanged. Else `SetCustomMagnification`, previous := current, zoom := `trunc(z)`, ViewZRChange1, §2.2, §2.3, bounds, CameraUpdated, ViewZRChange2, ViewZRChange3, `ClampTargetToScrollBounds`; true |
| `SetZoomAndRotation(z, r)` (ints) | `0x007CDE60` | the same with a rotation 0…3 (refused otherwise); nothing happens when both are unchanged |
| `SetRotation(r)` | `0x007CD540` | 0…3; an unchanged rotation returns true at once; else as above with the zoom kept |
| `SetCustomMagnification(m)` | `0x007CD6E0` | when `m` differs and the **current** magnification is in (0, 1000): store it, §2.2, §2.3, bounds, PostViewRefresh *[quirk: the range test is on the old value]* |
| `SetViewTarget(p)` | `0x007CD810` | every component finite and within ±100000: target := p, velocity 0; §2.3, CameraUpdated, ViewTChange2, ViewTChange3; true |
| `SetViewTargetVelocity(v)` | `0x007CBC60` | stores `v` when every component is a number (no NaN) |
| `ScrollByViewportAngle(angle, amount)` | `0x007CD330` | camera-plane step `(cos(angle), −sin(angle)) × amount × step[zoom]`, clamped (§2.5), back to the world, then the terrain view's ray cast along the view direction (terrain view slot 3), else the ground plane: the hit becomes the target; §2.3, messages as `SetViewTarget`; true (false without a camera) |
| `SetCameraPositionTweakAdd(x, y)` | `0x007CDA20` | on a change: §2.2, §2.3, bounds |
| `SetCitySize(w, h)` | `0x007CDAA0` | `+0x158`, `+0x15C`; when initialised the bounds polygon is emptied and rebuilt after §2.2, §2.3 |
| `SetViewportSize(w, h)` / `GetViewportSize` | `0x007CB9B0` / `0x007CB990` | also the camera's viewport |
| `GetVirtualZoom()` | `0x007CBC40` | zoom, plus 1 when the magnification exceeds 1.5 |
| `GetCustomMagnification()` / `GetDistancesToClipPlanes` | `0x007624E0` / `0x007CB9F0` | `+0xF0` / near and far |

### 2.7 Messages (through the message server 2)

`SendPostViewChangeMessage(kind)` (`0x007CC0B0`) and the inline senders use: CameraUpdated `0x4A9F2FFE` (data1 =
the camera control); ViewZRChange1–3 `0xA6B79600`–`0xA6B79602` and ViewTChange1–3 `0xA6B79610`–`0xA6B79612`, PostViewInit
`0xA6B79620` and PostViewRefresh `0xA6B79621`, each with data1 = the previous (zoom, rotation) pair `+0x100` and
data2 = the new pair `+0x110`. A zoom or rotation change sends ViewZRChange1 (`0x007CC000`) before the update, then
CameraUpdated, ViewZRChange2 and ViewZRChange3 after it; a target move sends CameraUpdated, ViewTChange2 and
ViewTChange3.

### 2.8 Save record (class `0xC9C628EC`; `Write` slot 3 `0x007CBB10`, `Read` slot 4 `0x007CBBA0`)

u16 version **1**; the view target (three floats); then six u32: previous zoom, previous rotation, zoom, rotation,
new zoom, new rotation. `Read` accepts only version 1 and sets "loaded" (§2.6 `Init`). Both return true unless the
stream reports an error.

## 3. Users

- `cSC4View3DWin` (the city view) and its input controls drive the camera control (zoom commands, scrolling, the
  initial target); its record holds a reference to the camera control (save-records.md §3.2).
- Every ViewZRChange / ViewTChange subscriber: the model maker, the terrain view, the lighting manager, the zone
  manager (zones.md §5.2), effects (effects.md) and others.
- `cS3DCamera` is also used by the 3D renderer, the region view and thumbnail rendering (its constructor has five
  callers).

## 4. Quirks

1. Pixel snapping of the rotation corner (§2.3).
2. `SetCustomMagnification` tests the old magnification, not the new one (§2.6).
3. Zooms between 4 and 5 are magnifications of zoom 4; `GetVirtualZoom` reports 5 only above 1.5× (§2.6).
4. The city size used for the clip planes is at least 4096 m in each direction (§2.2).
5. A clamped move that runs out of steps keeps the start point (§2.5).

## 5. Open questions

- **CA1:** (§2.5) the geometric meaning of the 0.48 factor and of `lift` in the scroll polygon.
- **CA2** (answered 2026-10-20 by re-drawcontext): `SetDepthBias` (§1.2), from the draw context's depth offset.

## 6. Conformance tests

1. **Tables:** metres per pixel at magnification 1: 2.613126, 1.306563, 0.653281, 0.286370, 0.143185 for zooms 0–4;
   pitch 30°, 35°, 40°, 45°, 45°; yaw for rotations 0–3: π/4, 3π/4, 5π/4, 7π/4.
2. **Ortho matrix:** `SetOrtho(−400, 400, −300, 300, 10, 20)` with an 800×600 viewport and no offset gives
   `m00 = 1/400`, `m11 = 1/300`, `m22 = −0.2`, `m23 = −3`, `m03 = m13 = 0`; a pixel offset (1, 0) adds `−2/800`
   to `m03`.
3. **Project / UnProject:** with an identity view, the box above and viewport 800×600: the point (0, 0, −15)
   projects to (400, 300, 0); (400, 0, −10) to (800, 300, −1); `UnProject` of each gives the point back.
4. **Look-at:** position (0, 0, 10), target 0, up (0, 1, 0): right (1, 0, 0), up (0, 1, 0), back (0, 0, 1).
5. **Eye placement:** zoom 0, rotation 0, city 4096×4096, target (2048, 0, 2048): the eye offset is
   (cos(π/4)·8192, tan 30°·8192, sin(π/4)·8192) and the eye lies on the plane through corner + offset.
6. **SetZoom:** `SetZoom(4.5)` gives zoom 4 and magnification √2 ≈ 1.41421 (`GetVirtualZoom` 4); `SetZoom(4.75)`
   magnification ≈ 1.68179 (`GetVirtualZoom` 5); `SetZoom(5)` and `SetZoom(−1)` are refused; repeating a zoom is
   refused.
7. **Messages:** a rotation change sends ViewZRChange1, CameraUpdated, ViewZRChange2, ViewZRChange3 in that order
   with data2 pointing at (zoom, new rotation).
8. **Record:** version 1, three floats, six u32; version 2 is refused.
9. **Ground intersection:** origin (0, 100, 0), direction (1, −1, 0) → (100, 0, 0); direction (1, 0, 0) → false;
   direction (0, 1, 0) → false (behind).
