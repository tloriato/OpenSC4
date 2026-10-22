# Spec: view utilities (`SC4ViewUtilities`)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`). Implementers must work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled
or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[compat-limit]* depends on something FreeCity4 cannot reproduce, so only the rule is matched.

Related specs: [view3d.md](view3d.md) (§3.1–§3.2, §4.1: the 3D renderer that owns this object, hands it out
through `GetViewUtilities` and calls §5 from its draw pass), [camera.md](camera.md) (the camera control that feeds
§2: zoom, rotation, viewport, clip distances), [lighting.md](lighting.md) (`RotateVectorByViewRotation`,
`0x007F6CC0`, rotates vectors by the view rotation), [animation-coordinator.md](animation-coordinator.md) (uses the
visible area of §6), [terrain.md](terrain.md), [model-maker.md](model-maker.md).

## 0. Summary

The view utilities hold everything the renderer derives from the camera once per camera change: the zoom and
rotation, the camera's basis, the **view frustum** (corners, box, planes and a ground-plane form for quick tests),
the projections between world, view and viewport coordinates, and a second **draw frustum** that may be narrowed
to a dirty rectangle of the viewport. Everything that draws or picks in the 3D city asks them whether a box is
visible, where a point lands on screen, and which ground point lies under a pixel.

## 1. The object *[compat]*

- 0x470 bytes, created by the 3D renderer's constructor (`cSC43DRender`, `0x007C9B10`, kept at its `+0x114`, next to
  the draw context at `+0x11C`); the constructor `0x007F6DF0` clears every field (the corner and ground-point
  vectors to (0, 0, 0)) and sets `+0x3F4`, `+0x3F8` to 1 and `+0x410` to 0.3, three fields none of the functions
  here read. No vtable.
- The renderer (`+0x04`) supplies the camera through its slot 14 *[inferred: `cSC43DRender::GetCamera`]*; the city
  size in metres is at `+0x20` (x) and `+0x24` (z), which the renderer's `Init` (`0x007C6410`) sets to 4096 m each
  (`0x007C66D6`), the largest city, before the first camera update.

| Offset | Content |
|---|---|
| `+0x00` | valid: set at the end of a successful §2 update |
| `+0x08`, `+0x0C`, `+0x10` | the zoom as a float and as an integer, the rotation 0–3 |
| `+0x14`, `+0x18`, `+0x1C` | yaw, pitch, the rotation's part of the yaw (rotation × π/2) |
| `+0x28`, `+0x2C` (`+0x30`, `+0x34`) | viewport width and height in pixels (and as floats) |
| `+0x3C`, `+0x48` | camera position and look-at point |
| `+0x54`, `+0x60`, `+0x6C` | forward, right and up unit vectors |
| `+0x78` / `+0x84` | the view frustum's box (min / max); `+0x90` / `+0x9C` the box clipped to the city (§3.3) |
| `+0xA8`–`+0x104` | the view frustum's 8 corners (§3.1) |
| `+0x108`–`+0x11C` | the camera's view volume: left, right, bottom, top, near, far |
| `+0x120`–`+0x14C` | the ground points under the 4 viewport corners (§6, cached) |
| `+0x150`, `+0x154` | the camera control's metres per pixel and pixels per metre (camera.md §2.1) |
| `+0x158`, `+0x15C` | the camera control's distances to the near and far clip planes |
| `+0x160`–`+0x1BC` | the view frustum's 6 planes (§3.2) |
| `+0x208`–`+0x224` | the view frustum's two ground lines (§3.2) |
| `+0x228`–`+0x3D8` | the draw frustum (§5): narrowed flag, rectangle, boxes, corners, view box, planes, ground lines |
| `+0x414`–`+0x440` | the cached visible-area rectangles (§6); `+0x444`, `+0x445` their cache flags |
| `+0x448`–`+0x454` | sin and cos of −yaw, sin and cos of pitch |
| `+0x458`–`+0x46C` | the viewport scales sx, sy, sz and their inverses |

## 2. Camera update (`UpdateCameraPosition(cameraControl)`, `0x007F83F0`) *[compat]*

Called when the camera changes. It clears both visible-area caches (§6) and the valid flag; without a renderer or
camera it stops there. Otherwise:

1. Camera position, look-at, and forward = look-at − position.
2. Zoom := the camera control's zoom (`+0x108`, an integer); rotation := its `+0x10C`; viewport size, clip
   distances (camera.md §2.6), metres per pixel and pixels per metre (`+0x134`, `+0x138`) and the camera's view
   volume.
3. Viewport scales: sx = width ÷ (right − left), sy = height ÷ (top − bottom), sz = 1 ÷ (far − near).
4. Angles: yaw = rotation × π/2 + **yawOffset[zoom]**, pitch = **pitch[zoom]**, with the tables below; the sin and
   cos of −yaw and of pitch are kept.
5. Basis: forward is normalised; right = normalise(forward × (0, 1, 0)); up = normalise(right × forward).
6. The view frustum (§3) is built, then the valid flag set.

| Zoom | 0 | 1 | 2 | 3 | 4 |
|---|---|---|---|---|---|
| yawOffset (`0x00ABCFC4`) | −π/8 | −π/8 | −π/8 | −π/8 | −π/8 |
| pitch (`0x00ABCFD8`) | 30° | 35° | 40° | 45° | 45° |

### 2.1 Angle helpers

- `GetCameraViewAnglesForRotation(rotation, zoom, &yaw, &pitch, &groundPitch)` (`0x007F7780`): yaw and pitch as in
  step 4, and groundPitch = atan(tan(pitch) ÷ cos(yawOffset[zoom])), the pitch seen along a city edge (32.0°,
  37.2°, 42.2°, 47.3°, 47.3° for zooms 0–4).
- `GetObjectYRotationAngle(rotation)` (`0x007F6CB0`): −rotation × π/2.
- `GetCityWidthFromOrthoView(width, zoom)` (`0x007F8300`): width × (cos a + sin a), a = |yawOffset[zoom]|; the
  screen width of a square of that side (× 1.3066 at every zoom).
- `GetZoomAndRotation` (`0x007F66A0`) returns `+0x0C` and `+0x10`; `ViewportUnitsToWorldUnits(v)` (`0x007F6690`)
  returns v × `+0x150`.

## 3. The view frustum *[compat]*

### 3.1 Corners and box

With n = position + forward × near and f = position + forward × far, h = right × (right − left)/2 and
v = up × (top − bottom)/2 (the view volume is taken as centred *[inferred]*), the corners are:

| Corner | 0 | 1 | 2 | 3 |
|---|---|---|---|---|
| near (`+0xA8` + 12 i) | n + h + v | n + h − v | n − h − v | n − h + v |

and corners 4–7 are the same at f, for the orthographic camera (projection type 1) that the city view uses
*[inferred]*. For a perspective camera the original pushes the **previous** corners 4–7 away from the camera
position by far ÷ near, so they compound across updates *[quirk]*; FreeCity4 should push corners 0–3 instead
*[choice]*. The box (`+0x78`) is the axis-aligned box of the 8 corners (`0x005D6900`).

### 3.2 Planes and ground lines

Six planes, each a normal and an offset so that a point p is inside when normal · p + offset > 0: right (`+0x160`,
normal = right vector, through corner 3), left (`+0x170`, −right, through corner 0), bottom (`+0x180`, up, through
corner 2), top (`+0x190`, −up, through corner 3), near (`+0x1A0`, forward, through corner 3), far (`+0x1B0`,
−forward, through corner 7).

Because the right vector is horizontal, the left and right planes are vertical and are also kept as **ground
lines** in (x, z): line A from corner 3 along (corner 7 − corner 3) (`+0x208`), line B from corner 0 along
(corner 4 − corner 0) (`+0x218`).

### 3.3 City-clipped box (`GetFrustumAABBCityClipped(viewFrustum, &min, &max)`, `0x007F7F60`)

For the view frustum (true) or the draw frustum (false): the ground quadrilateral made of corners 0 and 3 (in x,
z) and the two points where the rays corner 0 → corner 4 and corner 3 → corner 7 meet the plane y = 0 is clipped
to the city square (0, 0)–(x size, z size) (`SC4GeometryUtilities::ClipPolygonToPolygon2D`, at most 10 points). The
result's x and z are the clipped polygon's extent (0 when nothing is left); min y is 0 and max y is corner 3's y.

### 3.4 Visibility tests

- `IsBoundingBoxInViewFrustum(min, max)` (`0x007F6770`): a box is in when, for each of four tests, its corner
  furthest towards the inside passes: line A with (x − ax) dz − (z − az) dx < 0, line B with the same expression
  > 0, then the bottom and top planes. **There is no near or far test** *[quirk]*: a box behind the camera or past
  the far plane passes when it lies between the four sides.
- `IsBoundingBoxInDrawFrustum(min, max)` (`0x007F6910`): the same with the draw frustum's lines and planes (§5).
- `IsPointInViewFrustum(p)` (`0x007F7E50`): p in view coordinates (§4) must satisfy left ≤ x ≤ right,
  bottom ≤ y ≤ top and near < −z < far.

## 4. Coordinates *[compat]*

**View coordinates** (`ProjectPointToViewCoordinates`, `0x007F77D0`): d = p − camera position;
x₁ = d.x cos(−yaw) − d.z sin(−yaw), z₁ = d.x sin(−yaw) + d.z cos(−yaw); then
x = x₁, y = d.y cos(pitch) − z₁ sin(pitch), z = d.y sin(pitch) + z₁ cos(pitch). Depth is −z.
`ProjectPointToFixedViewCoordinates` (`0x007F6710`) applies the same rotation without the translation.

**Viewport coordinates** (`ProjectPointFromViewCoordinatesToViewportCoordinates`, `0x007F66C0`):
vx = (x − left) sx, vy = (top − y) sy, vz = (z + near) sz, so vz is 0 at the near plane and −1 at the far plane.

| Function | VA | Effect |
|---|---|---|
| `ProjectPointToViewportCoordinates` | `0x007F7850` | world to view to viewport |
| `ProjectPointToViewportCoordinates2D` | `0x007F78E0` | the same, leaving z in view coordinates |
| `ProjectPointToViewportZ` | `0x007F78A0` | only vz |
| `UnProjectPointFromViewCoordinatesToWorldCoordinates` | `0x007F7920` | the inverse rotation, then + camera position |
| `UnProjectPointFromViewportCoordinatesToWorldCoordinates` | `0x007F79B0` | viewport to view (with the inverse scales) to world |
| `ProjectPointFromViewportCoordinatesToFixedViewCoordinates` | `0x007F8390` | viewport to world, then the fixed-view rotation |

`ProjectBoundingBoxToViewportRectangle(min, max, &topLeft, &bottomRight)` (`0x007F7A40`) projects four chosen box
corners (2D projection) and rounds: left and top to nearest of (v − 0.5), right and bottom to nearest of v. The
corners, as (x, y, z) picks of min / max:

| Rotation | left | top | right | bottom |
|---|---|---|---|---|
| 0 | (min, max, max) | (min, max, min) | (max, max, min) | (max, min, max) |
| 1 | (min, max, min) | (max, max, min) | (max, max, max) | (min, min, max) |
| 2 | (max, max, min) | (max, max, max) | (min, max, max) | (min, min, min) |
| 3 | (max, max, max) | (min, max, max) | (min, max, min) | (max, min, min) |

A rotation outside 0–3 leaves the outputs unwritten.

### 4.1 Rays and the ground

- `ViewportPositionToRay(x, y, &origin, &direction)` (`0x007F9080`): the camera's eye ray through pixel (x, y),
  direction normalised; false without a renderer, a camera or a ray.
- `GetRayIntersectionWithGroundPlane(origin, direction, &out)` (`0x007F6AB0`): out = origin + t direction with
  t = −origin.y ÷ direction.y, unguarded.
- `GetViewportPointIntersectionWithGroundPlane(x, y, &out)` (`0x007F9160`): the pixel's ray met with y = 0; it
  always returns true and does not check the ray's result *[quirk]*.
- `RayIntersectsGroundPlane(origin, direction, &out)` (`0x007F6B50`): the geometry utilities' ray–plane test with the
  plane (0, 1, 0, 0).
- `GetViewportRectGroundCorners` (`0x007F9EA0`): for a pixel rectangle (top-left, bottom-right), the ground (x, z) of
  its four corners in the order top-left, top-right, bottom-left, bottom-right.

### 4.2 City helpers

- `GetCityCornerPosition(corner, &out)` (`0x007F6BA0`): out = (x, 0, z) by corner and rotation, with W = `+0x20`,
  D = `+0x24`:

| Corner | rotation 0 | 1 | 2 | 3 |
|---|---|---|---|---|
| 0 | (0, D) | (0, 0) | (W, 0) | (W, D) |
| 1 | (0, 0) | (W, 0) | (W, D) | (0, D) |
| 2 | (W, 0) | **(W, W)** | (0, D) | (0, 0) |
| 3 | (W, D) | (0, D) | (0, 0) | (W, 0) |

  Corner 2 at rotation 1 uses the x size for z *[quirk]*; cities are square, so it does not show. Other corner or
  rotation values leave x and z unwritten.
- `GetVectorForEdge(edge)` (`0x007F6D10`), edge mod 4: 0 (0, 0, −1), 1 (1, 0, 0), 2 (0, 0, 1), 3 (−1, 0, 0).
- `GetVisibleEdges(&a, &b)` (`0x007F7750`): a = (rotation + 1) mod 4, b = (rotation + 2) mod 4.
- `ClipWorldPositionToCityPosition(p)` (`0x007F7710`, with `ClipPositionToCity2D` `0x007F6620`): x below 0 becomes 0
  and x ≥ W becomes W − 1; z likewise with D; y is kept.
- `RoundDownToNearestBlockBoundary(v, block)` (`0x007F6D70`) and `RoundUpToNearestBlockBoundary` (`0x007F6DB0`):
  v is first **truncated** to an integer, then rounded down (up) to a multiple of block. So RoundUp(0.5, 64) = 0
  and RoundDown(−0.5, 64) = 0 *[quirk]*.

## 5. The draw frustum (`BuildDrawFrustumDataFromViewportRect(topLeft, bottomRight)`, `0x007F9F50`) *[compat]*

Drawing may be limited to a dirty rectangle of the viewport (pixels); the renderer's draw pass calls this with its
rectangles (view3d.md §4.1). The call clears both visible-area caches and stores the rectangle (`+0x22C`). For the
whole viewport the draw frustum is a copy of the view frustum (flag `+0x228` := 0), and the four viewport ground
points (`+0x120`) are computed if not cached. Otherwise (flag := 1), with c the rectangle's centre (integer halves):

1. The draw view box: left and right := (rect left − c.x) × factor and (rect right − c.x) × factor, top and bottom
   := (c.y − rect top) × factor and (c.y − rect bottom) × factor, factor = `+0x150`; when the rectangle spans the
   full width (or height), that pair is the camera's own. Near and far are the camera's.
2. The ground points (`+0x2E4`) of the rectangle's corners (§4.1).
3. Corners as in §3.1 around the line through c (c unprojected at viewport depth 0, moved back by near along
   forward), then the box, the six planes and the two ground lines as in §3.2, and the city-clipped box (§3.3).
   The perspective case has the same defect as §3.1.

`IsBoundingBoxInDrawFrustum` then tests against this frustum.

## 6. Visible area (`GetGroundPlaneVisibleAreaRectangle(&innerMin, &innerMax, &outerMin, &outerMax)`, `0x007F9B40`)

Computed once per camera update or draw rectangle (cache flag `+0x444`) from the ground points P₀–P₃ under the
viewport corners (top-left, top-right, bottom-left, bottom-right):

- **Inner** (`+0x414`): the x–z box of the midpoint of the top edge, (P₀ + P₁)/2, and of the bottom edge, (P₂ + P₃)/2.
- **Outer** (`+0x42C`): the x–z box of P₀–P₃ (`0x007F6550` takes a minimum of four, `0x007F65B0` a maximum), with
  the minimum rounded down and the maximum up to multiples of **64 m**, after truncation as in §4.2.

Every y is 0. Any output pointer may be null.

## 7. Picking *[compat]*

A pick record holds the ray origin (`+0x00`), its direction (`+0x0C`) and the nearest hit distance so far (`+0x28`).

- `TrianglePick(pick, a, b, c)` (`0x007F6AF0`): when the ray hits the triangle (`SC4GeometryUtilities::
  RayIntersectsTriangle`) closer than the nearest distance, that distance is updated and the result is true.
- `MeshPick(pick, mesh, transform)` (`0x007F91C0`): for a mesh with vertices, each of its primitive groups is walked
  triangle by triangle (§7.1); each triangle's vertices are transformed (below) and tested as above. **The first
  triangle that is closer than the nearest distance ends the mesh** with true *[quirk]*: a nearer triangle later in
  the mesh is not found.
- `ModelPick(pick, instance, model)` (`0x007F9A40`): for each of the model's meshes (slot 3 the count, slot 5 a mesh),
  the transform starts as the identity of kind 0 and the instance fills it (slot 9); true when any mesh hit. The
  model instances' pick methods call it (for example through `0x00801720`).

Transforms: a kind byte (`+0x01`), a 3 × 3 matrix M (rows at `+0x04`, `+0x10`, `+0x1C`), a translation t (`+0x28`)
and a uniform scale s (`+0x34`). Kind 0: none; 1: v + t; 2: s v; 3: s v + t; 4: M v; 5: M v + t; any other: s (M v) + t.

### 7.1 Triangle walk (`0x0080AE60`–`0x0080B3B0`)

A mesh has vertices (position in the first 12 bytes; 16-bit stride at `+0x04`, data at `+0x08`, count at `+0x06`),
optional 16-bit indices, and groups of 12 bytes (type, first, count). Without indices the group's vertices start at
`first`; with them its indices do. Triangle i of a group of count n, by type (`0x0080AE80` the count,
`0x0080AEF0` the indices):

| Type | Triangles | Triangle i |
|---|---|---|
| 0 | n / 3 | (3i, 3i + 1, 3i + 2) |
| 1 | n − 2 | (i, i + 1, i + 2) |
| 2 | n − 2 | (0, i + 1, i + 2) |
| 3 | n | (i, i, i) |
| 4 | n / 2 | (2i, 2i + 1, 2i) |
| 5 | n − 1 | (i, i + 1, i) |
| 6 | n / 2 | (2i, 2i + 1, 2i + 2) for even i, (2i, 2i + 2, 2i + 3) for odd i |
| 7 | n − 2 | (j, j + 1, j + 2) with j = i + (i mod 2) |

Other types have no triangles. Types 3–5 give degenerate triangles, which the ray test misses. Types 6 and 7 are
quads and quad strips (graphics-formats.md §3.7); their second triangles do not follow the quads, but no shipped
model uses either type *[quirk]*. The index reader
(`0x0080B090`) knows eight index formats (signed and unsigned 8 and 16 bits, two 32-bit codes, float and double,
floored), but the walk only uses unsigned 16 bits.

## 8. Users

The 3D renderer updates the object on camera changes; the terrain, model, overlay and automata views test their
boxes against the frustums; the tools pick through §4.1 and §7; the animation coordinator reads §6.

## 9. Quirks

1. The box tests skip the near and far planes (§3.4).
2. `GetViewportPointIntersectionWithGroundPlane` ignores a failed ray (§4.1).
3. `GetCityCornerPosition` corner 2 at rotation 1 uses the x size for z (§4.2).
4. The block rounding truncates first (§4.2, §6).
5. A mesh pick stops at the first closer triangle (§7).
6. Primitive types 6 and 7 (quads and quad strips) index unusually (§7.1).
7. A perspective camera's far corners are scaled from their previous values (§3.1).

## 10. Open questions

See [questions/view-utilities.md](questions/view-utilities.md) VU2–VU3: what the fields `+0x3DC`–`+0x410` hold;
whether the view volume is always centred (§3.1 builds it from half-extents). VU1 is answered: types 6 and 7 are
quads and quad strips (graphics-formats.md §3.7), which no shipped model uses.

## 11. Conformance tests

1. **Angles:** zoom 4, rotation 1 gives yaw = π/2 − π/8 = 67.5° and pitch 45°; groundPitch at zoom 0 is 32.0°.
2. **Ground ray:** origin (0, 100, 0), direction (0, −1, 1)/√2 meets the ground at (0, 0, 100).
3. **Box test:** a box behind the camera but between the four side planes is reported visible.
4. **Edges:** rotation 3 gives visible edges 0 and 1; edge 1 is (1, 0, 0).
5. **Corners:** in a 1024 m city, corner 0 at rotation 0 is (0, 0, 1024).
6. **Rounding:** RoundDown(−65, 64) = −128, RoundUp(65, 64) = 128, RoundUp(0.5, 64) = 0.
7. **Clip:** (−3, 7, 2000) in a 1024 m city becomes (0, 7, 1023).
8. **Pick:** a strip of 4 vertices (type 1) yields triangles (0, 1, 2) and (1, 2, 3).
9. **Viewport depth:** a point at depth near has vz 0; at depth far, −1.
