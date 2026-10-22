# Spec: geometry utilities (`SC4GeometryUtilities`, `cS3DMeshBuilder`, `nS3D` helpers)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`). Implementers must work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled
or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[compat-limit]* depends on something FreeCity4 cannot reproduce, so only the rule is matched.

Related specs: [flora.md](flora.md) (§ on `Intersects`: the box test as the flora placer uses it),
[lot-developer.md](lot-developer.md) (`LengthOfLine2D` and `RandomDirection2` in the construction crews),
[view-utilities.md](view-utilities.md) (§3 the clipped view polygon, §7 `TrianglePick`),
[network-geometry.md](network-geometry.md) (`RotatePoint2D` for the four rotations),
[draw-context.md](draw-context.md) (§9.3 `DrawBoundingBox`), [graphics-formats.md](graphics-formats.md) (§3.5 vertex
formats, §3.7 primitive types), [foundations.md](foundations.md) (§6 random generators, §18 float rounding),
[glyph-textures.md](glyph-textures.md) and [texture-bindings.md](texture-bindings.md) (the text textures, §6.4).

## 0. Summary

`SC4GeometryUtilities` is a set of free functions for lengths, distances, normals, planes, ray and line tests, box
overlap, and 2D polygon clipping, used by picking, placement, networks and the view. `cS3DMeshBuilder` collects
lit, coloured vertices into a mesh, and `nS3D::AddBoxMesh` uses it to build a box; `nS3D::RandomDirection2` and
`RandomDirection3` make random unit vectors, and `nS3D::CreateTextureFromText` renders a string into a texture.
Apart from the builder and the texture, none keeps state.

## 1. Conventions *[compat]*

- A point or vector is three `float32` (x, y, z), y up. A 2D point is two `float32`; callers pass ground
  coordinates (x, z) as its two components, written (x, y) below.
- **ε = 0.01** (`0x00A823A4`) is the tolerance below which a denominator counts as zero. It is absolute, not
  scaled by the operands' lengths *[quirk]*.
- Output pointers may be null; a null output is skipped and the result is still returned. Results are booleans.
- Arithmetic is the x87 sequence of each function; where an exact result matters, rounding follows foundations
  §18 (`r24`). Comparisons are as stated: "within [a, b]" is closed, "below" is strict.

## 2. Lengths and distances *[compat]*

| VA | Call | Result |
|---|---|---|
| `0x005D7660` | `LengthOfLine(a, b)` | \|b − a\| |
| `0x005D76A0` | `LengthOfLine2D(a, b)` | \|b − a\| in 2D |
| `0x005D6F90` | `LengthOfLineSquared(a, b)` | \|b − a\|² |
| `0x005D6FC0` | `LengthOfLineSquared2D(a, b)` | \|b − a\|² in 2D |
| `0x005D75D0` | `TwiceTriangleArea(p, a, b)` | \|(p − a) × (b − a)\|, twice the area of the triangle (p, a, b) |
| `0x005D71F0` | `PointDistanceToRay(p, o, d)` | §2.1 |
| `0x005D7290` | `PointDistanceToRectangle(p, min, max)` | §2.2 |

### 2.1 `PointDistanceToRay(p, o, d)`

t = (p·d − o·d) ÷ (d·d), then the distance from p to o + t d. t is not clamped, so this is the distance to the
whole line through o *[quirk]*; d must not be zero.

### 2.2 `PointDistanceToRectangle(p, min, max)` (2D)

0 when p is within the rectangle (closed on both axes). Otherwise, when p's x is within [min.x, max.x], the
distance in y to the nearer side; when p's y is within [min.y, max.y], the distance in x; else the distance to the
nearest corner.

## 3. Normals and planes *[compat]*

- `GetNormalToTriangle(a, b, c, n)` (`0x005D6EA0`): n = (c − b) × (a − b), **not normalised**. For a = (1, 0, 0),
  b = (0, 0, 0), c = (0, 0, 1) it is (0, 1, 0).
- `PointAndVectorToPlaneEquation(p, n, plane)` (`0x005D6F50`): plane = (n.x, n.y, n.z, −p·n), so a point q is on
  the plane when n·q + plane.w = 0.

## 4. Intersection tests *[compat]*

### 4.1 `RayIntersectsBoundingBox(o, d, min, max, hit)` (`0x005D6580`)

1. Per axis: o is "below" (o < min), "above" (o > max) or inside. When inside on all three axes: hit := o, true.
2. Per axis outside the box, the candidate distance t = (face − o) ÷ d on that axis (face = min when below, max when
   above); an axis inside the box, or one whose d component is exactly 0, gets −1.
3. The axis with the largest t is chosen (ties go to the earlier axis). t < 0: false.
4. On the other two axes the point o + t d must be within [min, max]; else false. The hit is that point, with the
   chosen axis set to its face value exactly; true.

### 4.2 `RayIntersectsTriangle(o, d, a, b, c, hit)` (`0x005D6FF0`)

With e1 = b − a, e2 = c − a, p = d × e2 and det = e1·p: |det| < ε is false. Else, with s = o − a, u = (s·p) ÷ det
and q = s × e1, v = (d·q) ÷ det: true when u is within [0, 1], v ≥ 0 and u + v ≤ 1; the hit is o + t d with
t = (e2·q) ÷ det. **t is not tested**, so a triangle behind the origin is hit as well *[quirk]*. Picking (view-utilities
§7) relies on the ray starting behind the scene.

### 4.3 `RayIntersectsPlane(o, d, plane, hit)` (`0x005D7360`)

s = n·o + plane.w and k = n·d. When |k| ≥ ε: hit := o − (s ÷ k) d, true (again the whole line *[quirk]*). Else,
when |s| ≥ ε (parallel, off the plane): hit := (0, 0, 0), false. Else (in the plane): hit := o, true.

### 4.4 `RaysIntersect2D(p1, d1, p2, d2, hit)` (`0x005D7420`)

- **With a hit pointer:** k = d2.x d1.y − d2.y d1.x; |k| < ε is false. Else
  t = (d2.x (p2.y − p1.y) − d2.y (p2.x − p1.x)) ÷ k, hit := p1 + t d1, true. Both lines are unbounded.
- **Without one:** true only when |d2.x − d1.x| > ε **and** |d2.y − d1.y| > ε, that is, when the two directions
  differ in both components; it does not test parallelism *[quirk]*. Two crossing lines with the same x direction
  component report false.

### 4.5 `LinesIntersect2D(a1, a2, b1, b2, hit)` (`0x005D74D0`)

Segments a1–a2 and b1–b2. With k = (b1.y − b2.y)(a2.x − a1.x) − (a2.y − a1.y)(b1.x − b2.x): |k| < ε is false
(nearly parallel segments never meet, even when they overlap). Else the parameters
ua = ((b1.y − b2.y)(b1.x − a1.x) − (b1.y − a1.y)(b1.x − b2.x)) ÷ k and
ub = ((b1.y − a1.y)(a2.x − a1.x) − (a2.y − a1.y)(b1.x − a1.x)) ÷ k must both be within [0, 1]; then
hit := a1 + ua (a2 − a1), true.

### 4.6 Boxes and rectangles

- `BoundingBoxesIntersect(aMin, aMax, bMin, bMax, outMin, outMax)` (`0x005D6D80`): true when aMin < bMax and
  bMin < aMax on each axis, x, y, z in that order (**strict**: touching boxes do not intersect; a NaN compare counts
  as overlapping; flora.md). When true and `outMin` is not null, the overlap box is written: the larger minimum and
  the smaller maximum per axis.
- `PointInRectangle2D(p, min, max)` (`0x005D68B0`): min ≤ p < max on both axes (half-open).

## 5. 2D vectors and polygons *[compat]*

- `ScaleVector2D(v, length)` (`0x005D76D0`): when \|v\| > ε, v := v × length ÷ \|v\|; else v is unchanged.
- `RotatePoint2D(c, p, angle, out)` (`0x005D77A0`): with (dx, dy) = p − c,
  out = c + (cos a · dx − sin a · dy, sin a · dx + cos a · dy).

### 5.1 `ClipPolygonToEdge2D(e0, e1, poly, n, out, max, count)` (`0x005D7800`)

One Sutherland–Hodgman step against the line from e0 to e1. A point p is **inside** when
(p.x − e0.x)(e1.y − e0.y) − (p.y − e0.y)(e1.x − e0.x) ≥ 0 (the right of the edge when y points up). The polygon is
walked from its last vertex to its first and on: an inside vertex after an inside one is emitted; an inside vertex
after an outside one emits the crossing and then the vertex; an outside vertex after an inside one emits the
crossing. Nothing is written once `max` points are out; `count` is the number written.

The crossing is `RaysIntersect2D(previous, current − previous, e0, e1 − e0)`. When that reports the lines as
parallel (only possible for very short segments, by the absolute ε), the original emits an unset point
*[quirk]*; FreeCity4 emits the current vertex instead *[choice]*.

### 5.2 `ClipPolygonToPolygon2D(poly, n, clip, m, out, max, count)` (`0x005D79D0`)

`out` first receives the n points of `poly` (not limited by `max`, so it must hold n points), then is clipped
against each edge of `clip` in turn, from its last vertex to its first and then along the polygon, through a
40-point scratch polygon; after each edge the count is limited to `max`. A clip polygon whose inside lies on the
right of each edge (§5.1) keeps its interior; the view-utilities clip square is such a polygon.

## 6. S3D helpers *[compat]*

### 6.1 The mesh builder (`cS3DMeshBuilder`)

The builder holds a vertex array of standard format 1 (position, colour; 16 bytes; graphics-formats.md §3.5), a
primitive list of (type, first vertex, vertex count), a light direction L, the current normal n, the current colour
(r, g, b) and alpha as fractions, and a "changed" flag.

| VA | Call | Behaviour |
|---|---|---|
| `0x008012F0` | constructor | all fields zero, no arrays |
| `0x00801660` | `Reset()` | no vertices, no primitives |
| `0x00800F80` | `SetLightDirection(L)` | |
| `0x00800F10` | `SetNormal(n)` | |
| `0x00800F30` | `SetColour(r, g, b)` | alpha := 1 |
| `0x00800F60` | `SetColour(r, g, b, a)` | |
| `0x00801200` | `BeginPrimitive(type)` | makes the arrays when missing; appends (type, current vertex count, 0); changed |
| `0x00801380` | `AddVertex(p)` | below |
| `0x00801140` | `Mesh()` | when changed, the vertex array's bounds are recomputed (`0x00801040`: the minimum and maximum over the vertices, starting from the first vertex when the box is empty) and the flag cleared; the builder is the mesh |

`AddVertex(p)` appends p with the colour k × (r, g, b) and alpha a, where k = 0.3 + 0.7 (L·n). Each channel
becomes a byte as follows: c = 256 × value rounded to nearest (ties to even), byte = (c − (c >> 8)) mod 256. So
0–1 maps to 0–255 with exactly 1 giving 255, but values outside 0–1 **wrap** rather than clamp *[quirk]* (with
L·n = −1, k = −0.4 and a full red channel gives 155). The colour is stored as bytes B, G, R, A.

### 6.2 `AddBoxMesh(builder, corners)` (`0x008014D0`)

`corners` holds the eight corners of a box, corner i at (bit 0 of i: x, bit 1: y, bit 2: z; set = maximum); the
corner maker `0x00800FA0` builds them from a box (min, max). One primitive of type 6 (quads, graphics-formats.md
§3.7) receives 24 vertices, four per face, each face with its outward normal set first:

| Normal | Corners |
|---|---|
| −z | 2, 3, 1, 0 |
| +z | 4, 5, 7, 6 |
| −y | 0, 1, 5, 4 |
| +x | 1, 3, 7, 5 |
| +y | 3, 2, 6, 7 |
| −x | 2, 0, 4, 6 |

The primitive is then closed: its count := the vertex count − its first vertex. `DrawBoundingBox`
(draw-context.md §9.3) builds its box this way with L = (0.25, 0.5, 1) (not unit length) and the caller's colour;
the effect maps build their meshes with the same builder.

### 6.3 Random directions

- `RandomDirection2(v, random)` (`0x0080B680`): x and y are drawn with the generator's
  `RandomDoubleRangeUniform(−1, 1)` (foundations §6.1), then v := (x, y) ÷ √(x² + y²). The rejection loop meant to
  redraw points outside the unit disc never repeats, so exactly two draws are made and the direction leans
  towards the diagonals *[quirk]* (lot-developer.md); (0, 0) would divide by zero.
- `RandomDirection3(v, random)` (`0x0080B730`): x, y, z are drawn the same way until x² + y² + z² ≤ 1 and ≥ 0.0001
  (a real rejection loop), then normalised.

### 6.4 Text textures (`CreateTextureFromText`, `0x0080AB90`)

`CreateTextureFromText(text, texture, font, size, rect, compressed)` renders a string into a new texture; the glyph
texture manager uses it for its glyph and string textures (glyph-textures.md §1).

1. The font is `font`, or when null the framework font source's font `0x68963C4C` (`RZ::WCM()` slot 5); none is
   false. The font's mode (slot 15) is saved and set to 3 (slot 16) for the drawing, then restored.
2. w and h are the text's width (font slot 48) and height (font slot 37). The texture is W × H, each the smallest
   power of two ≥ the size + 2. The text is placed at x₀ = (W − w) ÷ 2 + 1 and y₀ = (H − h) ÷ 2 − 1 (divisions
   truncated toward zero).
3. A W × H drawing surface with pixel code 9 (32 bits) is made through the graphic system (`RZ::GS()`), cleared,
   and the text drawn on it in white (`0xFFFFFFFF`, font slot 51). The 1-pixel border (rows 0 and H − 1, columns 0
   and W − 1) is then drawn over in colour 0, so filtering never picks up text at the edges.
4. The surface becomes a texture through the texture binding factory's slot 4 (texture-bindings.md §1.5; the
   factory is the global `0x00B43CD4`):
   - **compressed** (`0x0080A910`): only for a 32-bit surface with W and H multiples of 4 (else false); a DXT3
     image (format 9; graphics-formats.md §2.6) whose colour is white in every block (colours 0 and `0xFFFF`,
     every index 1) and whose alpha is the top 4 bits of each pixel's alpha;
   - **plain** (`0x0080A7C0`): the pixels as they are, format 3 (BGRA) with the type from the surface's pixel code
     (`0x0080A720`: 0, 2, 3, 5 → 14; 4, 6 → 10; 7 → 13; 8, 9 → 1, bytes); another code is false.
5. `size`, when given, receives (W, H); `rect`, when given, four floats (x₀ + w, y₀ + h, x₀, y₀). The result is
   the upload's.

## 7. Users

Picking and the view (view-utilities), the flora placer (`BoundingBoxesIntersect`), the lot developer and the
network code (lengths, rotations, line tests), `DrawBoundingBox` and the effect maps (the builder), the particle
effects, the 2D rigid-body simulator, construction crews and vehicle collisions (random directions), the glyph
texture manager (text textures).

## 8. Quirks

1. ε = 0.01 is absolute (§1).
2. Ray tests are line tests: hits behind the origin count (§4.2, §4.3).
3. `RaysIntersect2D` without an output compares the directions instead of testing parallelism (§4.4).
4. Box overlap is strict; point-in-rectangle is half-open (§4.6).
5. Builder colours wrap outside 0–1 (§6.1).
6. `RandomDirection2` never rejects (§6.3).
7. Text textures keep a cleared 1-pixel border; the compressed form keeps only 4 bits of alpha (§6.4).

## 9. Open questions

See [questions/geometry-utilities.md](questions/geometry-utilities.md) GU1: whether any caller of
`RaysIntersect2D` passes no output (§4.4).

## 10. Conformance tests

1. **Normal:** `GetNormalToTriangle((1,0,0), (0,0,0), (0,0,1))` = (0, 1, 0).
2. **Box ray:** o = (−5, 0.5, 0.5), d = (1, 0, 0), box (0,0,0)–(1,1,1): hit (0, 0.5, 0.5).
3. **Behind:** o = (0.2, 5, 0.2), d = (0, 1, 0) against the triangle (0,0,0), (1,0,0), (0,0,1): true, hit (0.2, 0, 0.2).
4. **Touching boxes:** (0,0,0)–(1,1,1) and (1,0,0)–(2,1,1) do not intersect.
5. **Rays without output:** d1 = (1, 0), d2 = (1, 1): false; with an output and p1 = (0, 0), p2 = (2, −1): hit (3, 0).
6. **Segments:** (0,0)–(2,2) and (0,2)–(2,0) meet at (1, 1).
7. **Builder colour:** red (1, 0, 0), L·n = −1: the vertex's red byte is 155.
8. **Clip:** the square (0,0), (0,4), (4,4), (4,0) clipped by the square (2,2), (2,6), (6,6), (6,2) leaves
   (2,2)–(4,4) (four points).
9. **Text texture:** a text 30 × 12 pixels gives a 32 × 16 texture with the text at (2, 1).
