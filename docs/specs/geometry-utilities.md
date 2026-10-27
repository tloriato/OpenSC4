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
- Nullability is per body, not a general convention. Length/distance/area results return on the x87 stack;
  predicates return a boolean; normal/plane/vector operations and clipping mutate their specified outputs.
  Normal, plane and clipping output/count pointers are required. Hit-pointer rules are in §4. An accessible
  pointer containing a float NaN is distinct from an invalid pointer; the latter has no numeric contract.
- Below, **R** means one resident x87 arithmetic result using the supplied precision control (PC) and rounding
  control (RC); **S** means a binary32 memory store and subsequent reload, also using RC. Separate products,
  sums and differences are separate operations, never fused. A resident result need not equal a binary32 store:
  even PC24 retains the x87 exponent range. No universal ambient PC/RC or caller-preservation guarantee is
  established here. Explicit PC24/nearest examples do not establish a default mode. Finite normal-range
  arithmetic is the primary exact domain; quiet-NaN branch rules assume masked invalid exceptions and normal
  x87 unordered status. Signaling exceptions, traps and invalid pointers are outside those rules.
- Inputs named points/vectors are binary32 components, counts are signed 32-bit unless stated otherwise.
  Comparisons are as stated: "within [a, b]" is closed, "below" is strict. The tolerance operand is the
  binary32 value with bits `3C23D70A`, not exact decimal 1/100.

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

The two dot products have different associations: p·d = (p.y d.y + p.z d.z) + p.x d.x;
o·d = (o.y d.y + o.x d.x) + o.z d.z. Subtract those resident results, divide by
(d.x² + d.y²) + d.z², and retain t resident. Each projected component is a multiply then an addition.
Residuals are p − projected point: y and z are binary32-stored/reloaded, x stays resident. Return
sqrt((residual.x² + residual.y²) + residual.z²), resident, with no final float store
(`005D7204–728C`). There is no denominator/zero-direction check or clamping branch in this body.

### 2.2 `PointDistanceToRectangle(p, min, max)` (2D)

0 when p is within the rectangle (closed on both axes). Otherwise, when p's x is within [min.x, max.x], the
distance in y to the nearer side; when p's y is within [min.y, max.y], the distance in x; else the distance to the
nearest corner.

The actual branch tree first selects the low-x case only for ordered p.x < min.x, otherwise high-x only
for ordered p.x > max.x, otherwise treats x as within. Within each x case it selects low-y only for
ordered p.y < min.y, otherwise high-y only for ordered p.y > max.y, otherwise y is within. This specifies
also inverted bounds and unordered comparisons: NaN comparisons do not select an outside side. If neither
axis selects outside, return a loaded +0; if only one does, return the corresponding min−p or p−max
resident subtraction. If both do, use the selected corner, store/reload dx = p.x−corner.x as binary32,
retain dy = p.y−corner.y resident, and return sqrt(dy²+dx²). Evidence: branch tree `005D7294–7301`, corner
stores/root `005D7304–7324`, scalar/+0 returns `005D7327–7356`. This differs from the unordered-rejecting
point-in-rectangle predicate and does not sanitize arbitrary NaNs.

### 2.3 Length/area operation and store sequence

For both length pairs, form each difference **b − a** as a resident subtraction, then square each difference
separately. The 3D sum is **(z² + y²) + x²**; the 2D sum is **y² + x²**. There are no binary32 intermediate
memory stores. The squared variants return that resident sum; the others apply one resident square root and
return it. Storing the result to a float is the caller's additional rounding step. Evidence:
`005D6F98–6FBE`, `005D6FC8–6FE0`, `005D7668–7690`, `005D76A8–76C2`.

`TwiceTriangleArea(p,a,b)` forms u = p−a resident, and v = b−a with **v.x/v.y stored as binary32, v.z
resident**. Its cross components are u.y v.z − u.z v.y, u.z v.x − u.x v.z, u.x v.y − u.y v.x (each two
products then a subtraction). Cross x and y are stored/reloaded as binary32; cross z remains resident.
Return sqrt((cross.z² + cross.y²) + cross.x²), with one rounding per indicated operation and no final
float store in the body (`005D75DB–7653`). The cross orientation is immaterial to the magnitude, but its
operation/store sequence is not interchangeable with a call to the normal-output routine.

## 3. Normals and planes *[compat]*

- `GetNormalToTriangle(a, b, c, n)` (`0x005D6EA0`): n = (c − b) × (a − b), **not normalised**. For a = (1, 0, 0),
  b = (0, 0, 0), c = (0, 0, 1) it is (0, 1, 0).
- `PointAndVectorToPlaneEquation(p, n, plane)` (`0x005D6F50`): plane = (n.x, n.y, n.z, −p·n), so a point q is on
  the plane when n·q + plane.w = 0.

The normal body retains c−b resident on all axes. It stores/reloads (a−b).x and .y as binary32, keeps .z
resident, forms the three two-product differences in the order x, y, z, and stores each output as binary32
(`005D6EAB–6F05`). No normalization or degenerate-triangle branch occurs. Output is required; input/output
aliasing is not promised by these formulas. The plane body copies n.x/y/z as 32-bit words **before** computing
w. Its w is **−((p.y n.y + p.z n.z) + p.x n.x)**, products and additions resident, followed by one binary32
store (`005D6F50–6F82`). It does not normalize n or validate it. These are distinct associations from the
length sum and from an x-first dot product.

## 4. Intersection tests *[compat]*

### 4.1 `RayIntersectsBoundingBox(o, d, min, max, hit)` (`0x005D6580`)

1. Per axis: o is "below" (o < min), "above" (o > max) or inside. When inside on all three axes: hit := o, true.
2. Per axis outside the box, the candidate distance t = (face − o) ÷ d on that axis (face = min when below, max when
   above); an axis inside the box, or one whose d component is exactly 0, gets −1.
3. The axis with the largest t is chosen (ties go to the earlier axis). t < 0: false.
4. On the other two axes the point o + t d must be within [min, max]; else false. The hit is that point, with the
   chosen axis set to its face value exactly; true.

All face/origin classification tests select outside only on an ordered comparison; unordered classifies that
axis as inside. A NaN d is not equal to zero. x's candidate quotient is binary32-stored **and retained
resident**; y and z candidates are binary32-stored. The first axis selection compares resident x against
stored y, choosing y only when x < y is ordered. Then it compares the selected **stored** value against
stored z, choosing z only for ordered less. Subsequent t use is the stored candidate. Ties/unordered retain
the current axis. A stored t rejects only on ordered t < 0. Each other-axis projection is multiply then add
resident, float-stored into scratch but compared **resident** against both bounds; only ordered below/above
rejects. Unordered projections pass. A successful hit is copied from scratch as words; the all-inside path
copies o as words. A null hit skips only the final copy; false paths never write hit. Evidence:
`005D6595–6679`, `005D6637–6738`, `005D673C–67BB`. Required readable inputs and nonaliasing outputs are the
specified domain; the finite formula above is not a promise of robust handling for every malformed box.

### 4.2 `RayIntersectsTriangle(o, d, a, b, c, hit)` (`0x005D6FF0`)

With e1 = b − a, e2 = c − a, p = d × e2 and det = e1·p: |det| < ε is false. Else, with s = o − a, u = (s·p) ÷ det
and q = s × e1, v = (d·q) ÷ det: true when u is within [0, 1], v ≥ 0 and u + v ≤ 1; the hit is o + t d with
t = (e2·q) ÷ det. **t is not tested**, so a triangle behind the origin is hit as well *[quirk]*. Picking (view-utilities
§7) relies on the ray starting behind the scene.

e1/e2 are all binary32-stored differences; the three p cross components are resident two-product
differences. det = (e1.z p.z + e1.y p.y) + e1.x p.x stays resident for its absolute-tolerance comparison.
After that passes, compute 1/det resident and **store/reload the reciprocal as binary32**. s.x/y are
binary32-stored; s.z is float-stored too but its resident version is used for u. u's dot is
((s.z p.z + s.y p.y) + s.x p.x), multiplied by the stored reciprocal and binary32-stored before both u bounds. q.x/y are
binary32-stored two-product differences; q.z is float-stored while retained resident for v. v uses
(d.z q.zResident + d.y q.yStored) + d.x q.xStored, then the stored reciprocal; **v is compared resident**
against zero and resident uStored+v against 1. Only after acceptance, and only with a hit pointer, t uses
(e2.z q.zStored + e2.y q.yStored) + e2.x q.xStored, times the stored reciprocal. Each hit component is
a resident multiply and add followed by a binary32 store. No intermediate t store. Evidence:
`005D6FFB–708E`, `005D709E–70E3`, `005D70ED–7199`, `005D719B–71D4`; the saved-register push at `005D70A4`
is included in this argument/local attribution.

The denominator rejects only ordered |det| < ε. u/v checks reject only ordered u < 0, u > 1, v < 0,
or u+v > 1. Consequently unordered barycentric checks can pass and return true, storing NaNs if a hit is
requested. All false paths leave hit untouched. A null hit still runs the barycentric tests and skips t/output.

### 4.3 `RayIntersectsPlane(o, d, plane, hit)` (`0x005D7360`)

s = n·o + plane.w and k = n·d. When |k| ≥ ε: hit := o − (s ÷ k) d, true (again the whole line *[quirk]*). Else,
when |s| ≥ ε (parallel, off the plane): hit := (0, 0, 0), false. Else (in the plane): hit := o, true.

Precisely s = ((o.y n.y + o.z n.z) + o.x n.x) + w, and k = (d.x n.x + d.z n.z) + d.y n.y,
with resident products and sums. k is float-stored **without discarding the resident value**; |kResident|
is compared with ε. Ordered |k| < ε takes the parallel branch; equality **or unordered** takes the
nonparallel branch. The latter returns true even with a null hit (no division in that case); otherwise
divide resident s by **stored k**, negate, multiply by each d component, add o, and float-store each hit.
In the parallel branch, ordered |s| < ε copies o as words and returns true; equality, greater **or unordered**
writes three +0 words and returns false. A null hit skips these writes. Evidence: `005D7368–73A8`,
`005D73AA–73E4`, `005D73E5–7414`. The failure-zero write supersedes any general failure-preserves-hit rule.

### 4.4 `RaysIntersect2D(p1, d1, p2, d2, hit)` (`0x005D7420`)

- **With a hit pointer:** k = d2.x d1.y − d2.y d1.x; |k| < ε is false. Else
  t = (d2.x (p2.y − p1.y) − d2.y (p2.x − p1.x)) ÷ k, hit := p1 + t d1, true. Both lines are unbounded.
- **Without one:** true only when |d2.x − d1.x| > ε **and** |d2.y − d1.y| > ε, that is, when the two directions
  differ in both components; it does not test parallelism *[quirk]*. Two crossing lines with the same x direction
  component report false.

With hit, negate d2.y first, then form k as d2.x*d1.y + (−d2.y)*d1.x and the numerator as
d2.x*(p2.y−p1.y) + (−d2.y)*(p2.x−p1.x). Differences, products, additions and the **direct division**
remain resident; there is no reciprocal float store. A hit coordinate is multiply/add then binary32 store.
Only ordered |k| < ε rejects; unordered proceeds and can store NaNs/return true. Without hit, both direction
differences must be ordered and strictly greater than ε in magnitude; equality/unordered returns false.
Evidence: `005D7429–7496`, `005D749D–74CC`. On denominator failure a nonnull hit is untouched.

*[compat]* All ten direct call sites supply nonnull hit storage: the polygon clipper
(`0x005D7910`, `0x005D7984`), the two terrain frustum routines (`0x00752FC3`, `0x00752FE1`,
`0x00753353`, `0x00753371`), side-cliff drawing (`0x00753ED8`, `0x00753EFA`) and water-box drawing
(`0x007552EC`, `0x0075530A`). The eight terrain calls use stack buffers. No address-taking reference to
`0x005D7420` was found in the export. Thus the null-output algorithm is unused by the identified executable
callers *[inferred]*; this does not remove its behavior from the helper's contract.

### 4.5 `LinesIntersect2D(a1, a2, b1, b2, hit)` (`0x005D74D0`)

Segments a1–a2 and b1–b2. With k = (b1.y − b2.y)(a2.x − a1.x) − (a2.y − a1.y)(b1.x − b2.x): |k| < ε is false
(nearly parallel segments never meet, even when they overlap). Else the parameters
ua = ((b1.y − b2.y)(b1.x − a1.x) − (b1.y − a1.y)(b1.x − b2.x)) ÷ k and
ub = ((b1.y − a1.y)(a2.x − a1.x) − (a2.y − a1.y)(b1.x − a1.x)) ÷ k must both be within [0, 1]; then
hit := a1 + ua (a2 − a1), true.

The x difference a2.x−a1.x is binary32-stored/reloaded before k and both numerators. b1.x−b2.x is retained
resident; other differences are resident and recomputed where used. k uses the two products and subtraction
shown above, tests |k| resident, then forms a **resident reciprocal** 1/k. Each numerator multiplies that
reciprocal and is float-stored as ua or ub before any interval check. The checks run ua lower/upper, then
ub lower/upper; a lower check rejects unordered as well as <0 (upper unordered is never reached for a NaN
parameter). Hence an unordered denominator passes its local tolerance gate but NaN ua/ub fails the full call.
On success with a hit, x uses stored dx and ua; y recomputes a2.y−a1.y resident and uses stored ua; each is
multiply/add then float-store. Null hit skips only those final writes; **all false paths preserve hit**.
Evidence: `005D74DD–7550`, `005D755A–759C`, `005D75A2–75C6`.

### 4.6 Boxes and rectangles

- `BoundingBoxesIntersect(aMin, aMax, bMin, bMax, outMin, outMax)` (`0x005D6D80`): true when aMin < bMax and
  bMin < aMax on each axis, x, y, z in that order (**strict**: touching boxes do not intersect; a NaN compare counts
  as overlapping; flora.md). When true and `outMin` is not null, the overlap box is written: the larger minimum and
  the smaller maximum per axis.
- `PointInRectangle2D(p, min, max)` (`0x005D68B0`): min ≤ p < max on both axes (half-open).

Overlap rejection compares aMin against bMax, then bMin against aMax, in x, y, z order. Only an ordered
greater-or-equal rejects: unordered proceeds to the next comparison. On rejection neither output is written.
On acceptance, **outMin null skips both outputs**; otherwise outMax must also be valid (no separate check).
For each output minimum, choose aMin only for ordered aMin > bMin, otherwise bMin. For each maximum, choose
aMax only for ordered aMax < bMax, otherwise bMax. Thus the **second box wins ties (including signed zero)
and unordered selections**, whether the NaN is in the first or second operand. The selected operand is loaded
and float-stored; this is a selection rule, not NaN sanitization. Evidence: rejection `005D6D84–6DEE`, output
gate `005D6DF5–6DFB`, minima `005D6E01–6E41`, maxima `005D6E44–6E8F`. The point predicate, in contrast,
rejects **any** unordered comparison as well as an ordered failure (`005D68BB–68EB`). These bounds rules
do not establish the foreign vertex-array bounds helper's tie/NaN policy (§6.1).

## 5. 2D vectors and polygons *[compat]*

- `ScaleVector2D(v, length)` (`0x005D76D0`): when \|v\| > ε, v := v × length ÷ \|v\|; else v is unchanged.
- `RotatePoint2D(c, p, angle, out)` (`0x005D77A0`): with (dx, dy) = p − c,
  out = c + (cos a · dx − sin a · dy, sin a · dx + cos a · dy).

Scale forms sqrt(x²+y²) resident and compares that resident length with ε; equality, less **or unordered**
leaves both components untouched. Otherwise compute lengthArgument/magnitude resident once, then multiply
each original component and store each binary32 (`005D76D4–7703`). No component is pre-divided/normalized.
Rotate forms p−c resident, loads the binary32 angle independently for x87 FCOS and FSIN, and retains their
results resident. The rotated x two-product difference is **stored/reloaded binary32 before** adding c.x;
that translated x is stored again. Rotated y is a resident two-product sum through adding c.y, then stored
to output. Output is required (`005D77AB–77F4`). Native trig primitive results must be supplied parameters
for exact operation-stage vectors; host double sin/cos are not an established native oracle. No range-error
fallback or control-word setup is present in this body.

### 5.1 `ClipPolygonToEdge2D(e0, e1, poly, n, out, max, count)` (`0x005D7800`)

One Sutherland–Hodgman step against the line from e0 to e1. A point p is **inside** when
(p.x − e0.x)(e1.y − e0.y) − (p.y − e0.y)(e1.x − e0.x) ≥ 0 (the right of the edge when y points up). The polygon is
walked from its last vertex to its first and on: an inside vertex after an inside one is emitted; an inside vertex
after an outside one emits the crossing and then the vertex; an outside vertex after an inside one emits the
crossing. Nothing is written once `max` points are out; `count` is the number written.

The crossing is `RaysIntersect2D(previous, current − previous, e0, e1 − e0)`. Edge and segment direction
components passed to that call have binary32 stores. The previous/current vertex coordinates themselves are
copied as words on direct-emission paths. The initial last-vertex side test uses resident edge differences
while also storing them; later side tests recompute them resident. The two side products and subtraction
are resident. Side ≥ 0 includes equality and both signed zeros; **unordered is outside** (`005D7834–7869`,
`005D7888–78B2`). All loops and capacity comparisons use signed 32-bit counts.

The count output is required and is set to zero first; n ≤ 0 returns immediately without reading vertices or
edge coordinates (`005D7808–7815`). For n > 0, all vertices are still traversed when max ≤ 0 or after capacity
is reached. Each direct/crossing emission is individually gated by count < max. For outside→inside, a single
remaining slot gets the crossing, not the vertex. The crossing call happens **before** its capacity gate.
Its boolean is ignored (`005D7910–7920`, `005D7984–7994`). When it fails, the shared crossing scratch retains
its preceding value, or is uninitialized if no preceding successful crossing wrote it. With a free slot that
scratch is loaded and emitted *[quirk]*. No deterministic coordinates are specified for an uninitialized
read. FreeCity4's current-vertex fallback is a deliberate safe *[choice]*, not a native result. With max ≤ 0
the emission gate avoids that scratch read. Supply accessible n-point input and sufficient output backing;
do not use an unsafe buffer or an uninitialized-read case as an output oracle.

### 5.2 `ClipPolygonToPolygon2D(poly, n, clip, m, out, max, count)` (`0x005D79D0`)

`out` first receives the n points of `poly` (not limited by `max`, so it must hold n points), then is clipped
against each edge of `clip` in turn, from its last vertex to its first and then along the polygon, through a
40-point scratch polygon; after each edge the count is limited to `max`. A clip polygon whose inside lies on the
right of each edge (§5.1) keeps its interior; the view-utilities clip square is such a polygon.

Precisely, count := n and copy the positive-n input before testing m. This initial copy ignores max and the
40 limit (`005D79F8–7A95`); output must hold **at least n points** for positive n. For m ≤ 0, return with that
count and copied output even if n > max, including max ≤ 0. With n ≤ 0 and m ≤ 0, preserve n as count and
write no point. These non-mutating point-buffer branches do not imply general safety for negative counts.

For each of the m edges (m > 0), call the edge clipper with **literal capacity 40**, using the current output
as input and a distinct fixed scratch buffer (`005D7AB3–7AD8`). If that call emits k points, set count to the
**signed minimum of k and max**, then copy just that many points back if count > 0 (`005D7AEB–7B17`). The
first edge can therefore read n > 40 properly backed input points but retains only the first 40 emissions.
max > 40 cannot enlarge the retained prefix. max = 0 yields count 0 after the first edge; max < 0 yields that
negative max, with no copy-back, and later edge calls accept the nonpositive input count and emit zero.
The loop still visits all clip edges. For any positive n, the initial copy remains visible in the output
tail beyond a shortened count. Initial-copy capacity and scratch capacity are different preconditions.
No region-management or view implementation is implied by this generic polygon routine.

## 6. S3D helpers *[compat]*

### 6.1 The mesh builder (`cS3DMeshBuilder`)

The builder holds a vertex array of standard format 1 (position, colour; 16 bytes; graphics-formats.md §3.5), a
primitive list of (type, first vertex, vertex count), a light direction L, the current normal n, the current colour
(r, g, b) and alpha as fractions, and a "changed" flag.

| VA | Call | Behaviour |
|---|---|---|
| `0x008012F0` | constructor | null vertex/index/primitive pointers; copies the float at `0x00B4E708` to L, n, RGB and alpha; changed := false (the executable's zero-filled image gives that float +0; no literal alpha-one default) |
| `0x00801660` | `Reset()` | if present, sets the 16-bit vertex count to zero and resizes vertex bytes to zero; resizes the primitive sequence to zero; retains the two builder handles; does **not** write changed or bounds |
| `0x00800F80` | `SetLightDirection(L)` | |
| `0x00800F10` | `SetNormal(n)` | |
| `0x00800F30` | `SetColour(r, g, b)` | alpha := 1 |
| `0x00800F60` | `SetColour(r, g, b, a)` | |
| `0x00801200` | `BeginPrimitive(type)` | makes the arrays when missing; appends (type, current vertex count, 0); changed |
| `0x00801380` | `AddVertex(p)` | below |
| `0x00801140` | `Mesh()` | when changed and vertex handle nonnull, calls the foreign vertex-array bounds updater (`0x00801040`) then clears changed; returns the builder as mesh; the helper's empty/tie/NaN policy remains open |

`AddVertex(p)` appends p with the colour k × (r, g, b) and alpha a, where k = 0.3 + 0.7 (L·n). Each channel
uses a float-bit conversion, **not** an x87 integer conversion. Add the float32 constant 49152 (`0x00ABD444`),
store as float32, reinterpret its bits as uint32, add `0xB8C00000` modulo 2³², then subtract bits 8–15 from
bits 0–7 modulo 256. The byte subtraction matters even in the ordinary range: channel 1 maps to 255,
where nearest-even 256×value followed by a byte cast gives 0. There is no clamp, `FIST`, or indefinite-i32 step.
For the isolated conversion stage, +infinity gives byte 0 under masked exceptions; a quiet NaN's payload
can affect the byte. These are not whole-call infinity/NaN guarantees for lighting, allocation or append.
The stored order is B, G, R, A.
Lighting evaluates `(L.z*n.z + L.y*n.y) + L.x*n.x`, multiplies by the **float64** constant 0.7
(`0x00ABD458`), then adds **float64** 0.3 (`0x00ABD450`). G and B products are stored as float32 before the
49152 addition; R stays on the x87 stack until that addition; alpha is loaded from its float32 field
(`0x00801388`–`0x0080146E`). Follow the active x87 precision/rounding controls at each arithmetic operation.

The four setters only copy fields (RGB also copies float32 1 to alpha); none writes changed. `BeginPrimitive`
sets changed even before allocating, appends a new zero-count primitive, and does not close its predecessor
(`0x0080120E`, `0x00801285`–`0x008012E7`). `AddVertex` requires an existing vertex array: it dereferences it
without a null check, appends at the old unsigned-16-bit vertex count, and stores count+1 modulo 65536. It
neither sets changed nor changes any primitive count (`0x008013BC`, `0x0080146A`–`0x008014B3`). Thus adding
after `Mesh()` alone does not invalidate bounds. `Mesh()` clears changed only when changed is true **and**
the vertex handle is nonnull (`0x00801143`–`0x00801155`). Reset after a completed Mesh leaves changed false.
The called array resize and bounds methods have separate owners; retention of their backing allocation,
empty bounds and unordered bound selection are not established by these builder bodies (§11).

There is no separately exported `EndPrimitive` or builder destructor in the 37-entry inventory (§11).
`AddBoxMesh` supplies an inline closure, described below; this alone does not establish a general
EndPrimitive-on-empty or destructor contract. The constructor's null-handle release branches cannot prove
destructor ownership. In particular, native destruction must not be inferred from a language's automatic drop.

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

This closure is inline in this body: if the primitive sequence is nonempty, zero-extend the current u16
vertex count, subtract the last primitive's 32-bit first-vertex word, and store the **32-bit wrapping
difference** as its count (`0080162B–164A`). It does not repair a u16 vertex-counter wrap into an expected
24-vertex count, and proves no separately callable EndPrimitive method.

### 6.3 Random directions

- `RandomDirection2(v, random)` (`0x0080B680`): x and y are drawn with the generator's
  `RandomDoubleRangeUniform(−1, 1)` (foundations §6.1), then v := (x, y) ÷ √(x² + y²). The rejection loop meant to
  redraw points outside the unit disc never repeats, so exactly two draws are made and the direction leans
  towards the diagonals *[quirk]* (lot-developer.md); (0, 0) would divide by zero.
- `RandomDirection3(v, random)` (`0x0080B730`): x, y, z are drawn the same way until x² + y² + z² ≤ 1 and ≥ 0.0001
  (a real rejection loop), then normalised.

Both routines pass **double** −1 and +1 to the foreign random-range primitive. Treat its returned resident
values as supplied inputs; this section does not replace the generator's algorithm or promise host-RNG bits.
Random2 stores x as binary32 and keeps y resident. Its rejection test uses yResident²+xStored²; the apparent
redraw path requires simultaneously sum>1 and sum<0.0001, and is unreachable for finite ordered sums.
Normalization recomputes yResident²+xStored², square-roots it and forms a resident reciprocal using binary32
1. x is multiplied and float-stored/reloaded; y multiplies the resident reciprocal and is stored at output
(`0080B6A9–B727`). No zero-vector sanitization is present.

Random3 stores x/y/z as binary32 but retains the last returned z resident. Rejection sum is
(zResident*zStored + yStored²) + xStored²; ordered >1 or ordered <**double** 0.0001 redraws all three.
Equality at either threshold accepts. Unordered comparisons also take acceptance, under §1's exception
qualification (`0080B759–B7D8`). Accepted normalization **recomputes** (zStored²+xStored²)+yStored², then
sqrt and reciprocal. The reciprocal is float-stored **without discarding the resident version**: x uses
the resident reciprocal, y/z the stored reciprocal. All output coordinates are float-stored
(`0080B7DE–B825`). Thus the rejection and normalization sums are not interchangeable, even with the same
random values. Constant evidence: bounds `00A80AB0/00A80AD0`, threshold `00A84E60`, unity `00B0EC64`.

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
   compressed helper uses global `0x00B43CD4`; plain helper queries the framework service):
   - **compressed** (`0x0080A910`): only for a 32-bit surface with W and H multiples of 4 (else false); a DXT3
     image (format 9; graphics-formats.md §2.6) whose colour is white in every block (colours 0 and `0xFFFF`,
     every index 1) and whose alpha is the top 4 bits of each pixel's alpha;
   - **plain** (`0x0080A7C0`): the pixels as they are, format 3 (BGRA) with the type from the surface's pixel code
     (`0x0080A720`: 0, 2, 3, 5 → 14; 4, 6 → 10; 7 → 13; 8, 9 → 1, bytes); another code is false.
5. `size`, when given, receives (W, H); `rect`, when given, four floats (x₀ + w, y₀ + h, x₀, y₀). The result is
   the upload's.

#### 6.4.1 Failures, widths and ownership

The only early false return is failure to obtain a font: no mode or output is changed. With a font, it is
AddRef'd (slot 1), its mode is saved, and mode 3 is set. Missing GS, a false surface-factory result, or a false
surface-create result all join the **same** epilogue as upload failure: restore the saved mode, write size and
rect, release a nonnull surface (slot 2), then release the font (slot 2). Result is false unless an upload was
actually called and returned true. Mode 3 remains active during the upload. Evidence: `0x0080ABCE`–`0x0080AC24`,
failure branches at `0x0080AC8B/ACB8/ACDD`, common epilogue `0x0080ADE9`–`0x0080AE51`.

The clear-access request `0x8001` and border-access request `0x8004` each gate only their own surface operations;
failure of either does not skip text drawing or upload. Each successful request is paired with its matching
end-access before proceeding; draw and line results are ignored (`0x0080ACE3`–`0x0080ADB5`). A factory that
returns true with a null surface is not checked before dereference. Font lookup's framework pointer and text
pointer are likewise assumed valid. The caller's texture pointer is passed through; this function never
AddRefs or releases it. It releases a nonnull surface even on a false factory/create result.

Width, height, length and placement are 32-bit integer quantities, not float dimensions. The NUL-terminated
byte string is counted before font lookup; its byte count is passed to both font measurement calls and draw.
Width/height + 2 and successive doublings wrap in 32 bits; loop comparisons are **signed**. There is no
negative-size or overflow rejection (`0x0080AC24`–`0x0080AC78`). For a wrapped target ≤1 the dimension is 1;
a target in `(2³⁰, 2³¹−1]` never terminates (doubling reaches `0x80000000`, then zero forever). Placement
differences/additions also wrap; halving uses signed truncation toward zero. The output rect converts the
signed integer results through x87 integer loads to float32, writing x₀, y₀ first at offsets 8,12 and the
far coordinates at 0,4 (`0x0080AE0A`–`0x0080AE3A`). No normalized UV division occurs here.

#### 6.4.2 Raw surface-upload boundaries

These are contracts of the named nS3D helpers, **not** implementations of the foreign font, surface,
graphics-system, texture factory or driver calls. Their call results/data are supplied primitives.

`GetTextureFormatForPixelCode(pixelCode, auxiliary, formatOut, typeOut)` has two input words; the auxiliary
word is unused. It treats pixelCode as unsigned: 0/2/3/5 → (format 3,type 14), 4/6 → (3,10), 7 → (3,13),
8/9 → (3,1). Code 1 or any value >9 (including a negative signed representation) returns false and writes
neither output. Both outputs are required for a supported code (`0080A720–A795`; dispatch tables
`0080A798–A7B5`). No format conversion occurs here.

`CreateTextureFromSurface(surface, texture, optionalRect)` first queries a framework factory service.
A null texture, null surface or false access request 0x8040 returns false without upload. After access succeeds,
read base address, width/height, byte pitch and bytes-per-pixel from the surface. Compute the upload row stride
as **signed bytePitch / bytesPerPixel**, truncating toward zero. With an integer rect (left,top,right,bottom),
upload dimensions are right−left, bottom−top, and source pointer is base+left*bytesPerPixel+top*bytePitch;
without it use full dimensions/base. Arithmetic uses 32-bit words without range checks. It calls the format
helper using the surface's descriptor, **ignores that helper's boolean**, calls the factory's upload, retains
the upload boolean, ends access 0x8040, then calls the foreign local-service cleanup primitive. No surface/texture
release is issued by this helper (`0080A7CE–A814`, `0080A84C–A907`). The normal supported domain requires a
valid acquired factory, nonzero bytes-per-pixel, representable offsets/stride, a supported format descriptor,
and accessible requested rows. Unsupported descriptors can leave upload fields unset: no deterministic
upload descriptor/result is specified for that path. A false upload still ends access before returning false.

`CreateCompressedTextureFromSurface(surface, texture)` requires a readable surface (no initial null check),
pixel code 9, and width/height whose low two bits are zero, then successful access 0x8040. Each failed check
returns false without upload; no end-access is called after a failed access request (`0080A918–A96B`).
For positive, representable, properly backed dimensions and successful temporary allocation, it creates
16 bytes per 4×4 block: row-major 4-bit alpha values from the high nibble of each source pixel's alpha,
followed by colour endpoints 0 and FFFF and sixteen colour indices 1. Blocks run left-to-right then top-to-bottom.
Source row stepping uses `(bytePitch unsigned>>2)*4`; for ordinary aligned positive pitches this is bytePitch.
The upload has full surface dimensions, format 9, type 0, row-stride field equal to width and unit auxiliary
dimensions. The upload's boolean is saved; temporary memory is freed **before** ending access 0x8040, then
that boolean is returned (`0080A97F–AAE6`, `0080AAEC–AB87`). The allocation result, global factory and texture
are not null-checked. Zero/negative/overflowed dimensions or allocation failure are not a validated safe
domain, despite the limited early checks. No device-side behavior is inferred from this descriptor.

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

See [questions/geometry-utilities.md](questions/geometry-utilities.md). GU1's direct caller survey is resolved in
§4.4. GU2 retains ambient/exception/trig domains beyond the explicit operation sequences; GU3/GU5 retain
foreign bounds/container and missing lifecycle behavior; GU6 retains any ownership attribution beyond the
enumerated bodies. GU4's text epilogue is specified in §§6.4.1–6.4.2. A local helper call is not proof of the
callee's entire algorithm, resource semantics or error behavior.

## 10. Conformance tests

All examples in this section are **PROPOSED / UNEXECUTED**. They are specification-derived expectations,
not observations of native or portable execution. Arithmetic-sensitive legacy examples below assume PC24,
nearest-even RC and masked exceptions; §10.1 states any different mode explicitly.

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

### 10.1 Independent distinguishing vectors (PROPOSED / UNEXECUTED)

These expectations were derived before reading any implementation/test output. They are not native runs,
portable-oracle observations or a default-mode promise. Arithmetic rows specify PC24, nearest-even RC,
masked exceptions, finite normal operands/intermediates unless explicitly qualified. The numerical bits below
were recomputed with integer/rational significand rounding at each operation, not host floating-point math.
NaN rows use quiet `7FC00001` with masked invalid; only the proved branch/store outcome is promised.
All buffers are valid and distinct unless an explicitly permitted null output is stated.

| ID | Inputs / setup | Expected discriminator |
|---|---|---|
| N1 | Squared 3D length, a=(0,0,0), b=(4096,1,1) | z²+y²=2, then +x² gives 16777218, binary32 `4B800001`; x-first separate sums give 16777216 (`4B800000`). |
| N2 | Plane p=(1,1,1), n=(1,16777216,−16777216) | y/z products cancel exactly, then +1 and negate: w=−1 (`BF800000`); x-first PC24 addition loses 1 and gives zero. |
| B1 | Overlapping boxes; aMin.x=+0, bMin.x=−0, other minima 0, maxima 1 | outMin.x is second operand −0 (`80000000`), not first +0. With aMax.x=+0, bMax.x=−0 and both minima −1, outMax.x is likewise −0. |
| B2 | aMin.x=quiet NaN, bMin.x=0; all other minima 0 and maxima 1 | Overlap accepts and stores bMin.x=0. Swap aMin.x=0, bMin.x=quiet NaN: accepts and selects/stores second NaN (no payload propagation promise). PointInRectangle with NaN p.x is false. |
| B3 | Touching x faces, outputs seeded with finite sentinels | false, both outputs untouched. For overlapping boxes, outMin=null and valid sentinel outMax: true, outMax untouched. |
| C1 | poly[i]=(i,0), i=0…40; n=41; clip has one point (0,0), m=1; max=100; output backing ≥41 points | All sides zero, count=40, first 40 input points retained; output slot 40 still contains the initially copied point (40,0). |
| C2 | Same 41 backed points, m=0 or −1; max=1, 0, or −1 | count=41 and all 41 initial copies survive; neither max nor scratch limit applies without edges. |
| C3 | C1 with max=0 or −1 | Initial 41-point copy still occurs; after edge, count=0 or −1 respectively, no copy-back. max=1 returns count=1. |
| C4 | Edge (0,0)→(1,0), two points (0,−1/1024),(0,+1/1024), max=0, backed output seeded | Two crossing calls fail the absolute denominator test; count remains 0, output untouched; capacity gates prevent reading failed-crossing scratch. This does not predict garbage for max>0. |
| C5 | Edge n=0 or −1, valid count pointer, output seeded; polygon n=−1,m=0 | Edge writes count=0 and no point; polygon writes count=−1 and no point. No unsafe point access is needed. |
| M1 | Builder alpha field 65537; only the alpha conversion stage | 65537+49152=114689; stored bits `47E00080`, offset bits `00A00080`, byte=128, unlike the 256×alpha integer shortcut. Whole-call prerequisites remain §6.1. |
| H1 | Plane n=(0,1,0), w=−1; o=(2,0,3), d=(1,0,0); hit sentinel (71,72,73) | Parallel/off-plane false **stores (0,0,0)**. Box o=(−1,2,0),d=(1,0,0), box [0,1]³ false preserves the same sentinel. |
| H2 | 2D rays p1=p2=(0,0), d1=(1,0),d2=(1,1) | With hit, true and (0,0); null hit, false. Parallel d1=d2=(1,0) with a sentinel hit returns false without a write. |
| H3 | Triangle d=(quiet NaN,−1,0); o=(0,1,0), a=(0,0,0), b=(1,0,0), c=(0,0,1) | NaN determinant/u/v comparisons pass their unordered paths: true, writes unordered coordinates with hit. No exact NaN payload promised. |
| H4 | Segments a1=(0,0),a2=(1,0), b1=(quiet NaN,−1),b2=(0,1), finite sentinel hit | Unordered k passes the local tolerance gate, but NaN ua fails lower bound: false, hit untouched. |
| V1 | ScaleVector2D v=(quiet NaN,1), requested length=2 | Unordered magnitude leaves both original component words untouched, including the supplied NaN bits. |
| N3 | **PC64**, nearest-even; plane n=(1,−1,0),w=0; o=(1,0,0),d=(float bits `3C23D70A`,2⁻³³,0) | kResident=ε−2⁻³³, stored k=`3C23D70A`=ε. Resident comparison takes parallel branch, s=1 gives false and zero hit. Comparing the stored k instead would incorrectly return true. Exact rational difference is 1/8589934592. |
| M2 | Valid builder/vertex backing for all 65536 slots, current u16 vertex count=65535; dirty=false after Mesh; primitive count sentinel 17 | AddVertex appends at slot 65535, wraps counter to 0, leaves dirty=false and primitive count 17. Next AddVertex uses slot 0; no implicit primitive closure. Mesh alone does not call bounds updater in this state. |
| M3 | Reset a populated builder with dirty=false; separately repeat with dirty=true | Both handles retained, vertex count=0 and zero-length resize requests issued; dirty respectively false/true and no bounds write by Reset. Repeated BeginPrimitive appends another zero-count entry and dirties without closing the previous entry. |
| T1 | Supplied font reports width=30,height=12, saved mode=7; GS absent; size/rect sentinel outputs | false; trace: AddRef font, save/set mode 3, measure, fail GS, restore mode 7, size=(32,16), rect=(32,13,2,1), release font. No surface/upload call; outputs are not preserved sentinels. |
| T2 | Same dimensions; nonnull surface acquired, upload false | false; mode 3 through upload, then restore saved mode, write size/rect, release surface, release font. Clear/border access failures gate their own operations only. |
| U1 | Plain helper supplied source base B, bytePitch=32, bytesPerPixel=4, rect=(1,1,3,3), supported descriptor | Upload pointer B+36, dimensions 2×2, stride 8 pixels; upload-false trace still ends access before the foreign service-cleanup primitive. |
| U2 | Compressed 4×4 surface, aligned bytePitch=16, every source alpha byte `AB`, successful access/allocation | One block: first eight bytes `AA` each, endpoints `0000,FFFF`, index bytes `55 55 55 55`. Upload-false trace frees temporary before unlock. |
| N4 | PC24 nearest; a=(0,0,0), b=(4096,17/16,17/16), squared and ordinary 3D length | Each small square=289/256. Native association gives squared 16777218 (`4B800001`), root 4096 (`45800000`); x-first gives squared 16777220 (`4B800002`), root 4096+1/2048 (`45800001`). Roots independently rounded by integer square/midpoint comparisons, not host sqrt. |
| M4 | Isolated channel=1, and separately red=1 with L·n=−1 (other valid builder fields/backing) | Magic-byte channel 1 is 255; with double-operand 0.7/0.3 lighting, red byte is 155 under the stated PC24/nearest stages. Both differ from a direct rounded 256×channel cast. |
| M5 | AddBoxMesh begins with u16 vertex count=65535, valid corners, successful supplied array/list operations and sufficient backing | New primitive first=65535, final vertex count=23, inline primitive count=`FFFF0018` (4294901784), not 24; closure subtracts zero-extended wrapped vertex count from 32-bit first. |

The C1 edge is deliberately degenerate: every finite side test is zero, with no crossing-scratch dependency.
The B1/B2 selection rules belong to the overlap body only, not the builder's foreign bounds helper.

## 11. Exact owner-body ledger and proof limits

This is the **37-entry / 7509-byte export attribution**, not full-owner/runtime/default coverage. Source
identity for the 2026-10-23 instruction re-read: Windows image base `00400000`, executable SHA256
`3bc5c7fe807fe5aa24a3abb06e40f13486670bc1150612c09baf4ecedafe1a4b`, matching export-summary. Sizes are
exported decoded bytes (not a promise that every address in the span is an instruction). The bodies with
gaps list those ranges. Behavioral argument shapes below come from entry argument loads and their uses,
named-owner export entries, and direct call sites; these are not new ABI declarations.

**N** = local numeric/branch/store contract established in the cited section, conditional on §1 controls and
valid memory. **D** = local delegation/mutation sequence established, with named foreign primitives supplied.
**P** = positive local contract published but listed external/lifecycle residual remains. These statuses do
not count foreign callees as additional owner bodies. Every row's source is the instruction body at that VA.

| # | Owner / body; behavioral inputs → channel | Bytes; inclusive source range | Published status / section |
|---|---|---|---|
| 01 | Geometry: RayIntersectsBoundingBox(o,d,min,max,optional hit) → bool | 596; `005D6580–67D3` | N §4.1 |
| 02 | Geometry: PointInRectangle2D(p,min,max) → bool | 72; `005D68B0–68F7` | N §4.6 |
| 03 | Geometry: BoundingBoxesIntersect(aMin,aMax,bMin,bMax,outMin,outMax) → bool | 285; `005D6D80–6E9C` | N §4.6; outputs gated as a pair |
| 04 | Geometry: GetNormalToTriangle(a,b,c,required normal) → normal | 112; `005D6EA0–6F0F` | N §3 |
| 05 | Geometry: PointAndVectorToPlaneEquation(p,n,required plane) → plane | 54; `005D6F50–6F85` | N §3 |
| 06 | Geometry: LengthOfLineSquared(a,b) → resident float | 47; `005D6F90–6FBE` | N §2.3 |
| 07 | Geometry: LengthOfLineSquared2D(a,b) → resident float | 33; `005D6FC0–6FE0` | N §2.3 |
| 08 | Geometry: RayIntersectsTriangle(o,d,a,b,c,optional hit) → bool | 503; `005D6FF0–71E6` | N §4.2 |
| 09 | Geometry: PointDistanceToRay(p,o,d) → resident float | 159; `005D71F0–728E` | N §2.1 |
| 10 | Geometry: PointDistanceToRectangle(p,min,max) → resident float | 199; `005D7290–7356` | N §2.2 |
| 11 | Geometry: RayIntersectsPlane(o,d,plane,optional hit) → bool | 183; `005D7360–7416` | N §4.3 |
| 12 | Geometry: RaysIntersect2D(p1,d1,p2,d2,optional hit) → bool | 173; `005D7420–74CC` | N §4.4; null selects another algorithm |
| 13 | Geometry: LinesIntersect2D(a1,a2,b1,b2,optional hit) → bool | 251; `005D74D0–75CA` | N §4.5 |
| 14 | Geometry: TwiceTriangleArea(p,a,b) → resident float | 132; `005D75D0–7653` | N §2.3 |
| 15 | Geometry: LengthOfLine(a,b) → resident float | 49; `005D7660–7690` | N §2.3 |
| 16 | Geometry: LengthOfLine2D(a,b) → resident float | 35; `005D76A0–76C2` | N §2.3 |
| 17 | Geometry: ScaleVector2D(required v,float length) → v | 58; `005D76D0–7709` | N §5 |
| 18 | Geometry: RotatePoint2D(c,p,float angle,required out) → out | 91; `005D77A0–77FA` | N §5; trig results parameterized |
| 19 | Geometry: ClipPolygonToEdge2D(e0,e1,poly,signed n,out,signed max,required count) → out/count | 454; `005D7800–787C`, `005D7880–79C8` | N §5.1; uninitialized-crossing coordinates excluded |
| 20 | Geometry: ClipPolygonToPolygon2D(poly,signed n,clip,signed m,out,signed max,required count) → out/count | 362; `005D79D0–7B39` | N §5.2; valid backing required |
| 21 | Builder: SetNormal(normal[3]) → fields | 26; `00800F10–0F29` | D §6.1 |
| 22 | Builder: SetColour(rgb[3]) → RGB/alpha | 35; `00800F30–0F52` | D §6.1 |
| 23 | Builder: SetColour(rgba[4]) → RGBA | 32; `00800F60–0F7F` | D §6.1 |
| 24 | Builder: SetLightDirection(light[3]) → fields | 26; `00800F80–0F99` | D §6.1 |
| 25 | nS3D: GetBoxCorners(box[min,max],corners[8]) → corners | 148; `00800FA0–1033` | D §6.2; finite component selection/copies |
| 26 | Builder: Mesh() → builder as mesh | 29; `00801140–115C` | P §6.1; foreign bounds updater |
| 27 | Builder: BeginPrimitive(type word) → builder/list | 232; `00801200–12E7` | P §6.1; allocation/container failure not generalized |
| 28 | Builder: constructor(this) → initialized fields | 136; `008012F0–1377` | P §6.1; no destructor inferred |
| 29 | Builder: AddVertex(point[3]) → array/u16 counter | 321; `00801380–14C0` | P §6.1; valid array/append primitive supplied |
| 30 | nS3D: AddBoxMesh(builder,corners[8]) → builder/list | 386; `008014D0–1651` | D §6.2; six faces and inline closure |
| 31 | Builder: Reset() → zero-count/resize requests | 78; `00801660–16AD` | P §6.1; foreign backing retention/empty bounds not proved |
| 32 | nS3D: GetTextureFormatForPixelCode(code,aux,formatOut,typeOut) → bool/outputs | 118; `0080A720–A795` | N §6.4.2 |
| 33 | nS3D: CreateTextureFromSurface(surface,texture,optional integer rect) → bool | 328; `0080A7C0–A907` | D §6.4.2; supported descriptor domain |
| 34 | nS3D: CreateCompressedTextureFromSurface(surface,texture) → bool | 623; `0080A910–A9F6`, `0080AA00–AA28`, `0080AA30–AB8E` | D §6.4.2; positive aligned normal domain |
| 35 | nS3D: CreateTextureFromText(text,texture,font,optional size,optional rect,compressed flag) → bool/outputs | 717; `0080AB90–AE5C` | D §§6.4–6.4.2; supplied font/surface/service calls |
| 36 | nS3D: RandomDirection2(required v,random object) → v | 174; `0080B680–B72D` | N §6.3; random primitive returns supplied |
| 37 | nS3D: RandomDirection3(required v,random object) → v | 252; `0080B730–B82B` | N §6.3; random primitive returns supplied |

Geometry means `SC4GeometryUtilities` (20 / 3848 bytes); Builder means `cS3DMeshBuilder` (9 / 915);
nS3D has 8 / 2746. No exported general EndPrimitive, builder destructor, transform or sort is in this set.
Missing/folded lifecycle attribution remains a question, not a fabricated method. In particular, shared
container resize/allocator functions and `cS3DVertexArray::UpdateBounds` are foreign dependencies, not new
credited owner bodies. Draw-context, ModelInstances, ModelMaker, renderer/driver, font/glyph and generic
trig/RNG algorithms are outside this ledger. These utility contracts imply no region-management behavior.
