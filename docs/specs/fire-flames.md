# Spec: fire flames (`cSC4Flame`, `GetPointsOnModel`, the fire occupant's flames)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`). Implementers must work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled
or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[compat-limit]* depends on something FreeCity4 cannot reproduce, so only the rule is matched.

Related specs: [disasters.md](disasters.md) (§5.11 the fire occupant: its interface, the flame counts from the D7
and D8 curves, stages, targets; this spec only adds the flames), [effects.md](effects.md) (§4.1 creating a visual
effect by name), [model-instances.md](model-instances.md) (the meshes and transforms read here),
[geometry-utilities.md](geometry-utilities.md) (§2 `TwiceTriangleArea`), [foundations.md](foundations.md) (§6.1
the generators).

## 0. Summary

A burning occupant shows its fire as a set of **flames**, each a visual effect placed on the target. When the
target has a mesh, the flames are scattered over its triangles, weighted by area and set 1 m out along each face;
otherwise they are spread over its bounding box: some on top, the rest on the four sides. Each flame plays the
effect for the fire's stage, `fire_1`–`fire_5` on top or `wfire_1`–`wfire_5` on a wall.

## 1. The flame (`cSC4Flame`) *[compat]*

A flame holds: a "made" byte, a visible flag, the **wall** flag (`+0x02`), the **side** (`+0x04`, 0–3), the
position (x, y, z at `+0x08`), a transform and its effect (`+0x4C`).

- `Update(stage)` (`0x004710D0`): the current effect, if any, is stopped (effect slot 4) and released; the effects
  manager (global `0x00B43D1C`, slot 7, effects.md §4.1) makes the effect named `wfire_<stage + 1>` for a wall
  flame or `fire_<stage + 1>` otherwise (tables `0x00B07FA4` and `0x00B07F90`, stages 0–4); on success the transform
  is applied and, when the flame is visible, the effect starts (slot 3).
- `UpdateTransform()` (`0x00470CA0`): the effect's transform (slot 7) becomes a translation to the position and a
  rotation about the vertical axis by the side × 90, **passed to sine and cosine as radians** *[quirk]*: the four
  sides turn the flame by 0, 90, 180 and 270 radians (about 0°, 116.6°, 233.2° and 349.9°), not quarter turns.
- `Read(stream)` (`0x004700A0`): a 16-bit version (below 2 fails), the wall byte, the side, the position, then the
  transform (version 3 and later; earlier versions use the legacy transform reader); true when the stream reports no
  error.

## 2. Points on a model (`GetPointsOnModel(n, occupant, out)`, `0x00471CF0`) *[compat]*

1. The occupant's model instances (occupant slot 19); none is false.
2. For each instance that is a mesh container (model-instances.md §2), with its transform (instance slot 9):
   each mesh's indices are taken three at a time as triangles, `ceil(k ÷ 3)` triangles for the k indices of the
   mesh's first primitive *[inferred: the meshes are triangle lists]*. Each triangle (`0x00470EC0`) records its
   three vertices, the running total of `TwiceTriangleArea` over all triangles so far, its unit normal
   n = (v₁ − v₀) × (v₂ − v₀) ÷ \|…\|, a flag "n.z > 0", and a **side** code: 0 when \|n.x\| ≥ \|n.z\| and n.x ≥ n.z,
   1 when \|n.z\| > \|n.x\| and n.x ≥ n.z, 2 when \|n.x\| ≥ \|n.z\| and n.x < n.z, 3 otherwise (the faces +x, −z,
   −x, +z).
3. No triangle at all is false. Else `out` gets n entries from a new generator seeded from the clock (seed −1,
   foundations §6.1), so **flame placement is not deterministic** *[compat-limit]*. For each entry:
   - r := `RandomDoubleRangeUniform(0, total)`; the triangle is the first whose running total is ≥ r;
   - u := `RandomDoubleUniform()`, v := `RandomDoubleRangeUniform(0, 1 − u)`, w := 1 − u − v; the point is
     u v₀ + v v₁ + w v₂ (this is not uniform over the triangle: it gathers points towards v₀ *[quirk]*);
   - the point moves 1 m along the unit normal, then through the instance's transform, s (M p) + t;
   - the entry is (point, the triangle's flag, its side). True.

## 3. Making the flames (`CreateFlames`, `0x00472380`) *[compat]*

With T and S the top-flame and side-flame counts (disasters.md §5.11) and the 3D view's current view level
(global `0x00B43DC4`, slot 22):

- **On the model:** `GetPointsOnModel(T + S, target, points)`. When true, one flame per point: wall := the point's
  flag, position and side from the point; the fire remembers that its flames are on the model (`+0x64`).
- **On the box** (otherwise): the target's bounding box (min, max; occupant slot 10). Flames are made while the
  count is **≤ T + S**, so there are **T + S + 1** *[quirk]*; each starts at the box's centre in x and z, and:
  - a flora target (type `0x74758926`): y = min.y;
  - else the first T flames are top flames: y = max(max.y − 1, min.y), and when T > 1, x and z are drawn with
    `RandomDoubleRangeGaussian` over the box (foundations §6.1);
  - the others are wall flames on a random side `RandomUint32Uniform(4)`: side 0 x = max.x, side 1 z = min.z,
    side 2 x = min.x, side 3 z = max.z, the free horizontal coordinate uniform over the box, y uniform over
    [min.y, max.y].

  These draws use the fire code's own generator (`0x00B21648`).
- Every flame: position and transform set; when the view level is not 1, a visible flame is hidden (effect slot 4).

## 4. Stage, start and view updates *[compat]*

- `SetStage(s)` (`0x004728B0`): when s differs, it is stored; with no flames yet, `CreateFlames` runs; then every
  flame's `Update(s)`.
- `Init` (`0x00471170`): the occupant `Init`, the fire joins occupant group `0x5005`, and every flame gets
  `Update(stage)`.
- `ViewUpdate` (`0x00472790`, on the view messages of disasters.md §5.3 step 6): only for flames on the model, and only
  when `GetPointsOnModel` gives as many points as there are flames: every flame is **placed afresh** at a new random
  point (its wall flag, side and position), and hidden when the view level is not 1. So model flames jump to new
  places on every view update *[quirk]*.

## 5. Users

The fire disaster (disasters.md §5) creates fire occupants; their stage changes and the view messages drive this
spec. `GetPointsOnModel` has no other caller.

## 6. Quirks

1. The flame's yaw uses degrees as radians (§1).
2. Box fires get one flame more than the counts say (§3).
3. Model flames are placed by a clock-seeded generator, biased towards each triangle's first vertex, and re-placed
   on every view update (§2, §4).

## 7. Open questions

See [questions/fire-flames.md](questions/fire-flames.md) FF1: whether the "n.z > 0" wall flag is meant for model
space with z up or is an original slip (with y up, roofs get top flames and only faces towards +z get wall flames).

## 8. Conformance tests

1. **Effect name:** a wall flame at stage 2 plays `wfire_3`; a top flame at stage 0 plays `fire_1`.
2. **Box count:** T = 1, S = 6 on a target without a mesh gives 8 flames: 1 top, 7 walls.
3. **Top height:** a box from y = 0 to y = 20 puts top flames at y = 19.
4. **Side code:** a face with normal (0, 0, −1) has side 1; (0.6, 0, 0.8) has side 3.
5. **Yaw:** side 1 rotates by 90 radians, cos = −0.448.
