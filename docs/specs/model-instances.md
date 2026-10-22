# Spec: model instances (`cS3DModelInstanceBasic`, `cS3DModelInstance`, `cS3DModelProcessor`, sprite animations)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`). Implementers must work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled
or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[compat-limit]* depends on something FreeCity4 cannot reproduce, so only the rule is matched.

Related specs: [graphics-formats.md](graphics-formats.md) (§3 the S3D model, §3.9 the **animated instance's
playback**, §3.12 building a frame, which this spec does not restate), [draw-context.md](draw-context.md)
(`RenderModelInstance`), [view-utilities.md](view-utilities.md) (§7 picking and the triangle walk),
[lighting.md](lighting.md) (§4.4 `GetVertexColors`), [animated-sprite.md](animated-sprite.md) (the sprite
instance, which reads the resources of §6), [lot-models.md](lot-models.md) and
[geometry-utilities.md](geometry-utilities.md).

## 0. Summary

Everything the scene draws is a **model instance**. `cS3DModelInstanceBasic` is the common part: a reference count,
an owner, custom data, properties, draw attributes and the interface the scene calls (bounds, transform, pick,
draw, sort value); the lot models, the animated sprites and the model instances below build on it.
`cS3DModelInstance` is a static model: it holds the meshes and materials of one S3D frame and draws them through
the draw context. `cS3DModelInstanceAnimated` plays the frames of an S3D animation (graphics-formats §3.9).
`cS3DModelProcessor` takes over a static instance to light its vertices itself, to give it vertex normals and to sort
its meshes. The sprite-animation resources (`cS3DResSpriteAnim*`) hold the per-zoom sheets that sprite instances
draw.

## 1. The basic instance (`cS3DModelInstanceBasic`) *[compat]*

A virtual base (IID `0x26A68449`; constructor `0x00801CF0`) of every model instance. The instance interface, as the
base implements it (vtable `0x00ABD468`; derived classes replace the slots marked "derived"):

| Slot | Call | Base behaviour (VA) |
|---|---|---|
| 0 | `QueryInterface(iid)` | IIDs 1 and `0x26A68449` give the basic part, with a reference (`0x00801940`) |
| 1, 2 | `AddRef`, `Release` | a 16-bit count; the last release runs the most-derived deleting destructor (`0x00801900`, `0x00801910`) |
| 3, 4 | `GetOwner`, `SetOwner(o)` | a counted reference (`0x00801850`, `0x00801860`) |
| 5, 6 | `GetCustomData`, `SetCustomData(d)` | one 32-bit value (`0x00801890`, `0x008018A0`) |
| 7 | `GetBounds(box)` | calls slot 8 of the most-derived object (`0x008018B0`) |
| 8 | `GetWorldBounds(box)` | derived |
| 9 | `GetTransform(t)` | the identity (`cS3DTransform::MakeIdentity`, `0x00801DC0`) |
| 10 | `SetTransform(t)` | ignored |
| 11 | `Pick(context)` | false |
| 12 | `Draw(context, …)` | nothing (`0x008018D0`) |
| 13, 14 | `GetProperties`, `SetProperties(p)` | one 32-bit value (`0x008018E0`, `0x008018F0`) |
| 15, 16 | `GetDrawAttributes(a)`, `SetDrawAttributes(a)` | a 20-byte record copied out and in (`0x00801E10`, `0x00801E50`) |
| 17 | `GetSortValue(v)` | derived |
| 18 | `GetResModel()` | null |

`cS3DTransform::MakeIdentity` (only when the transform's "dirty" byte is set) writes the identity rotation, a zero
translation and scale 1, clears the byte and bumps the change counter (the transform layout is view-utilities §7).

## 2. Static model instances (`cS3DModelInstance`) *[compat]*

An object of `0xA4` bytes from a pool (`operator new` `0x00801B00`, released by `0x008020F0`; constructor
`0x00802950`, destructor `0x00802620`, deleting destructor `0x00802B00`). It is also a **mesh container**
(IID `0x26A68448`): the meshes (12-byte records: vertex block, index block, primitive block, each counted) and the
materials (52-byte records, graphics-formats §3.8) of one model frame.

Mesh container slots (vtable `0x00ABD554`; the bare container of `0x00ABD4E0` has the same slots 3–7):

| Slot | Call | VA |
|---|---|---|
| 0 | `QueryInterface`: `0x26A68449` gives the basic part; 1 and `0x26A68448` the container | `0x008017B0` |
| 3 | `GetMeshCount()` | `0x00801AD0` |
| 4, 5 | `GetMesh(i)` | `0x00801AF0` |
| 6, 7 | `GetMaterial(i)` | `0x00801EC0` |
| 8 | deleting destructor | `0x00802B00` |

Instance slots (vtable `0x00ABD508`, thunks `0x008026A0`–`0x008027C0` to the bodies):

| Slot | Call | Behaviour (body) |
|---|---|---|
| 3, 4 | owner | a counted reference (`0x00801B40`, `0x00801B50`) |
| 5, 6 | custom data | (`0x00801700`, `0x00801710`) |
| 7 | `GetBounds(box)` | the model's box, refreshed from the model (its slot 7) when marked stale (`0x00801FC0`) |
| 8 | `GetWorldBounds(box)` | the same box, then transformed by the instance's transform (`0x00802010`) |
| 9, 10 | `GetTransform`, `SetTransform` | the 56-byte transform; setting it bumps the change counter (`0x00801B80`, `0x00801BE0`) |
| 11 | `Pick(context)` | view-utilities §7 over the meshes (`0x00801720`) |
| 12 | `Draw(context, …)` | `RenderModelInstance(context, instance, transform, draw attributes, …)` (`0x00801750`; draw-context.md) |
| 13, 14 | properties | a 16-bit value here (`0x00801790`, `0x008017A0`) |
| 15, 16 | draw attributes | (`0x00801F00`, `0x00801F40`) |
| 17 | `GetSortValue(v)` | the first material's first texture ID, else 0 (`0x00801F80`) |
| 18 | `GetResModel()` | the model (`0x00614A20`) |

`SetModel(model)` (`0x00801C40`) takes a counted reference to the model and inspects its first material
(graphics-formats §3.8). When its bytes `+9` and `+10` are 8 and 1 *[inferred: the blend pair]*, the instance's
properties get bit 2 set and bit 1 cleared, its draw attributes bits 1–3 set, and the material loses flag `0x80`
(depth write) and its byte `+0x18`; a material with flag `0x10` (blending) only changes the properties that way.
`0x00801CD0` refreshes the instance's box from the model.

## 3. Animated model instances (`cS3DModelInstanceAnimated`) *[compat]*

The playback (constructor `0x007FD870`, `SetFrame`, `SetSpeed`, `Advance`, `Step`, `SetAnimMode`) is
graphics-formats §3.9. The other animation slots:

- `GetFrame()` (slot 11, `0x007FAF90`) and `GetFrameCount()` (slot 12, `0x007FC150`): from the sub-range table at
  `+0xA4` when set, else the current frame and the header's frame count F.
- `SetAnimation(index)` (slot 9, `0x007FDA90`): with a sub-range table only, selects range `index` (its first frame
  and length) and builds its frame; without one it does nothing. graphics-formats §3.9 notes that the table is
  never set.
- Building frame f (`0x007FC2E0`): the model's frame f (model slot 8, `GetModelFrame`) into the instance's meshes;
  a frame equal to the current one is not rebuilt.

## 4. The model processor (`cS3DModelProcessor`) *[compat]*

A helper that relights a static model instance on the CPU. Its state: the instance, its mesh container, the target
vertex format (standard 10: position, colour, uv; graphics-formats §3.5), the original vertex blocks, the vertex
normals per mesh, and the mesh order. Constructor `0x00803060`; destructor `0x008030A0`.

- `SetModel(instance)` (`0x008038D0`):
  1. A previous instance first gets its original vertex blocks back; then the new instance is taken.
  2. An animated instance (IID `0xA818EDD8`) with more than one frame is refused: false, nothing kept.
  3. Its mesh container (IID `0x26A68448`) is kept only when every mesh's vertex block is standard format 2
     (position, uv); otherwise the processor keeps no container and does nothing later.
  4. Each mesh's vertex block is replaced by a new one in format 10 with the same vertices: position and uv copied,
     colour white (`0xFFFFFFFF`), the same bounds. The original blocks are kept for step 1. True.
- `CreateVertexNormals()` (`0x00803370`): for each mesh, each triangle (the triangle walk, view-utilities §7.1)
  adds its normal (`0x0080B490`, Newell's formula, whose length is twice the triangle's area) to its three vertices
  and its length to their weights; each vertex normal is then the sum ÷ the weight sum, when the weight is above 0.
  The result is the area-weighted mean of the unit face normals, **shorter than 1** where faces meet at an angle
  *[quirk]*.
- `LightModel(lighting, flip)` (`0x00803E10`): makes the normals when there are none; sets flag 8 on material 0
  *[inferred: use vertex colours]*; takes the instance's rotation (slot 9), negated when `flip` is set; then for
  each mesh whose original block was format 2, the lighting manager's `GetVertexColors` (lighting.md §4.4) writes
  the vertex colours from the normals and that rotation.
- `SortMeshes(direction)` (`0x00802DB0`): orders the meshes by the dot product of their box centre with
  `direction`, largest first (an insertion sort, so equal values keep mesh order), into the processor's mesh order.
- `ReleaseModel()` (`0x00802B30`): releases the instance and the container and marks the container released, so the
  destructor does not release it twice.

## 5. Bounding boxes and small types *[compat]*

`cS3DVertexArray::UpdateBounds` (`0x00801040`) is geometry-utilities §6.1. `cS3DBoundingBox::Center` (`0x0046FFC0`)
is (min + max) ÷ 2 per axis.

## 6. Sprite-animation resources (`cS3DResSpriteAnimFactory`) *[compat]*

The factory (destructor `0x00808B70`) creates two resource types (`0x00808BD0`) and reads them by type
(`SerialRead`, `0x00808B20`):

| Type | Object | Interface | Reader |
|---|---|---|---|
| `0x29A5D1EC` | sprite animation (`0x5C` bytes) | `0x29A5D1EC` (`0x008085C0`) | `ReadAnim` (`0x00808660`) |
| `0x09ADCD75` | sprite-animation table (`0x20` bytes) | `0x69B6F01C` (`cS3DResSpriteAnimTable::QueryInterface`, `0x00808630`) | `ReadTable` (`0x00808920`) |

A failed read releases the new object and gives null.

### 6.1 Sprite animation (`ReadAnim`)

All fields are little-endian `u32`:

1. A type tag, which must be `0x29A5D1EC`.
2. Five header values *[inferred: not interpreted here]*.
3. Five per-zoom table keys k₀–k₄ (instances of `0x09ADCD75` tables). k₀ and k₁ are read and replaced by 0. A key
   below the resource's own instance I counts as relative: k := k + I (so 0 means I).
4. One more value.

Then each zoom z gets a source zoom and a scale s_z, with the zoom sizes P = (8, 16, 32, 73, 146) (`0x00ABD668`):

- **Downwards:** zoom 4 is its own source (s₄ = 1). For z = 3, 2, 1, 0: a key equal to the current source's
  shares it, s_z = P_z ÷ P_source; a different key becomes the source, s_z = 1.
- **Upwards, only when k₄ = I:** for z = 0 … 4, a key other than I becomes the source; a key equal to I takes the
  last source's key with s_z = P_z ÷ P_source.

The sprite instance scales a frame by s_z when it differs from 1 (animated-sprite.md §4.1). Zooms 0 and 1 always end
up with the instance's own key or a shared one, but the sprite draws nothing there.

### 6.2 Sprite-animation table (`ReadTable`)

A type tag `0x09ADCD75`, then the rest of the resource as one block: valid when its size is 32 + 8 n, where n is
the 32-bit value at block offset `0x1C`. So a table is a 32-byte header holding the entry count n at `+0x1C`,
followed by n entries of 8 bytes (animated-sprite.md AS1 asks what the header and entries hold).

## 7. Users

Every scene object (lots, buildings, props, networks, automata) holds model instances; the occupant and automata
code creates them through the model maker (model-maker.md). Automata use the processor (`SetModel` when a vehicle's
model changes, `LightModel` when it turns), and so do the dispatch view's 3D indicator, the tank and the
model-based particle effects (`SortMeshes`, `LightModel`); the effects model processor makes normals.

## 8. Quirks

1. A basic instance reports the identity transform and ignores a new one (§1).
2. Processed models get vertex normals shorter than 1 at creases (§4).
3. Only format-2 models are relit; others are left as they are (§4).
4. A sprite animation never has its own sheet at zooms 0 and 1 (§6.1).

## 9. Open questions

See [questions/model-instances.md](questions/model-instances.md) MI1–MI2: what the five header values and the last
value of a sprite animation hold; what `flip` in `LightModel` is used for.

## 10. Conformance tests

1. **Basic transform:** a lot model's `GetTransform` gives the identity even after `SetTransform`.
2. **Sort value:** a static instance whose first material's first texture is `0x12345678` sorts by `0x12345678`.
3. **Processor refusal:** `SetModel` with a two-frame animated instance is false.
4. **Normals:** two triangles of equal area at right angles sharing a vertex give it a normal of length
   1/√2 ≈ 0.707.
5. **Mesh order:** boxes centred at z = 1, 3, 2 with direction (0, 0, 1) sort as 3, 2, 1.
6. **Shared sheet:** keys k₂ = k₃ = k₄ = K ≠ I: zoom 3 uses K with scale 73 ÷ 146 = 0.5, zoom 2 with 32 ÷ 146.
