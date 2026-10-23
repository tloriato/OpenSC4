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

### 0.1 Exact scope of the portable eight-owner subset

The eight published owner names below identify the scope of MI3–MI10. They are not a claim that every native
method of those classes, every renderer integration, or the complete model-instance family is portable.
In particular, `cS3DResSpriteAnimTable` and `cS3DResSpriteAnimFactory` are separate owners; a bare mesh
container and an animated **sprite instance** are not additional owners in this subset. The following
inventory distinguishes actual entry points from inherited/shared bodies and omitted native surfaces.

| Owner | Published contract and original entry inventory | Boundary |
|---|---|---|
| `cS3DModelInstanceBasic` | Constructor `0x00801CF0`, deleting destruction `0x00801D70`; all 19 instance slots in §1, of which world-bounds and sort are pure/derived; the other entries are shared bodies or adjustment thunks to §1 | Foundation thunks `0x005D5420`–`0x005D54F0` reach this contract but belong to a derived object's interface, not fourteen extra owner algorithms |
| `cS3DModelInstance` | Constructor `0x00802950`; body/deleting/container-deleting destruction `0x00802620`, `0x00802B00`, `0x00802920`; pool new/delete `0x00801B00`, `0x008020F0`; 19 instance slots at vtable `0x00ABD508`; 9 container slots at `0x00ABD554`; SetModel/UpdateBounds `0x00801C40`, `0x00801CD0` | Instance/container parts share the 32-bit count; duplicate const/nonconst mesh and material slots share bodies. Mesh-frame creation is supplied by the resource, not SetModel itself |
| `cS3DModelInstanceAnimated` | Constructor `0x007FD870`; the inherited static/basic/container surfaces; GetFrame, SetAnimMode, GetFrameCount, SetSpeed, Advance, BuildFrame, SetAnimation, SetFrame, Step at `0x007FAF90`, `0x007FAFC0`, `0x007FC150`, `0x007FC170`, `0x007FC1C0`, `0x007FC2E0`, `0x007FDA90`, `0x007FDAF0`, `0x007FDB80` | Native animation vtable `0x00ABD088` additionally has slots 0–8 at `0x007FAF40`, `0x007FB680`, `0x007FD950`, `0x007FB6B0`, `0x007FBF30`, `0x007FBE90`, `0x007FC020`, `0x007FBFC0`, `0x007FC0A0`; these extra transform/access convenience entries are outside the published playback subset, not silently counted as implemented |
| `cS3DModelProcessor` | Constructor/destructor `0x00803060`, `0x008030A0`; ReleaseModel, SortMeshes, CreateVertexNormals, SetModel, LightModel and triangle-normal helper at `0x00802B30`, `0x00802DB0`, `0x00803370`, `0x008038D0`, `0x00803E10`, `0x0080B490` | Native assignment `0x00513FF0` is a separate additional entry, outside the published operations; no copy/assignment equivalence is claimed |
| `cS3DResSpriteAnim` | Own interface vtable `0x00ABD6B4`: QueryInterface `0x008085C0`, counted wrappers `0x00808600`/`0x00808610`, borrowed data getter `0x008085F0`, CreateModelInstance `0x00808A30`; deleting destruction `0x00808A00`; constructor is inlined in factory | Parser is factory ReadAnim. Persistence-base services are inherited. CreateModelInstance delegates sprite-instance initialization, outside this subset (§6) |
| `cS3DResSpriteAnimTable` | Own interface vtable `0x00ABD6FC`: QueryInterface `0x00808630`, the same counted wrappers, borrowed block getter `0x00808620`; constructor/destruction `0x00808BA0`, `0x00808AD0` | Parser is factory ReadTable. The data getter returns the block pointer, unlike animation's address-of-data getter; both are borrowed |
| `cS3DResSpriteAnimFactory` | ReadAnim `0x00808660`, ReadTable `0x00808920`, SerialRead `0x00808B20`, destruction `0x00808B70`, CreateInstance `0x00808BD0`; persistence factory vtable `0x00ABD714` delegates the other factory service slots | Factory does not own a newly read resource merely because SerialRead was called; inherited generic persistence services are not new sprite algorithms |
| `cS3DTransform` | Dirty-gated identity `0x00801DC0`; caller-driven copy/change semantics in static Get/SetTransform `0x00801B80`/`0x00801BE0`; point transforms consumed by picking and full bounds transform `0x007F8170` | Not a counted object or virtual interface. Picking kind dispatch and full bounds transformation are different consumers; byte counter semantics are shared (§1.2) |

These tables count interfaces and list actual entries rather than using substring search totals as a coverage
denominator. For example, the export's auto-name `cS3DTransform::TransformDirection` at `0x009F0090` is not
evidence of another vector-transform contract: its instructions consume an x87 value and call math/runtime
helpers without reading a vector or the transform. It is excluded pending corrected symbol ownership.
Animated count wrappers `0x007FB530`/`0x007FB540` reach the same static 32-bit reference bodies; animation does
not inherit the basic class's 16-bit count. No whole-owner/family implementation, runtime or shadow credit is
established by this RE contract audit.

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

### 1.1 Construction, records and lifetime (MI3, MI10)

The basic constructor starts with **zero** native references, null owner, custom data 0 and properties 2
(`0x00801D2B`–`0x00801D35`). A successful query adds a reference; an unsupported query returns false without
writing the output (`0x00801940`). The 16-bit reference count increments modulo 65536 and returns the
zero-extended result. Release decrements only counts greater than 1; **both 0 and 1 destroy the most-derived
allocation and return 0** (`0x00801900`, `0x00801910`). Thus incrementing 65535 returns 0, and the following
Release destroys the allocation; there is no native overflow protection or hidden surviving ownership.
A portable handle may protect memory safety, but must not describe surviving handles after that destruction
as native behaviour. A newly returned counted instance normally has 1, distinct from the constructor's 0.

Owner getters return a borrowed pointer. Setting the same owner causes no reference calls. Basic owner
replacement releases the old owner **before** adding a reference to the new one, then stores the pointer
(`0x00801860`); destruction releases the remaining owner once (`0x00801D93`). Static owner replacement uses
the opposite order: retain new, store new, release old (`0x00801B50`). These orders matter to callbacks.

The 20-byte draw record is copied field-for-field, including its otherwise uninterpreted bytes. Its relative
layout and constructor defaults are:

| Record bytes | Default in both basic and static instances | Contract |
|---|---|---|
| `+0` | 0 | Flag byte; static SetModel ORs `0x0E` into this byte only |
| `+1` | 0 | Separate byte, preserved by the copy |
| `+2..3` | Unwritten by these constructors | Preserved 16-bit field; no deterministic native default |
| `+4`, `+8`, `+12` | binary32 1, 1, 1 | Three copied words, initialized from `0x00B0E35C..64` |
| `+16` | binary32 1 | Copied word |

Evidence: basic construction `0x00801D3C`–`0x00801D5D`, static construction
`0x00802AA0`–`0x00802ACA`, copies `0x00801E10`/`0x00801E50` and `0x00801F00`/`0x00801F40`;
the executable data at `0x00B0E35C..64` contains three `0x3F800000` words. A portable API may choose initial
values for the two unwritten bytes, explicitly as a choice; it must preserve them after a supplied record.

### 1.2 Transform counter (MI3)

The change counter is an **unsigned byte**, incremented modulo 256. MakeIdentity increments the destination
counter only when its byte `+1` is nonzero; otherwise it changes nothing, even if the other fields are not an
identity (`0x00801DC4`–`0x00801DFC`). Static GetTransform increments the caller's destination counter, copies
the 13 binary32 words at `+4..+0x34`, then copies byte `+1`; it does not return the instance's counter
(`0x00801B80`). SetTransform increments the **instance's existing** counter, copies the same words and byte
`+1`, and ignores the supplied counter (`0x00801BE0`). Neither operation copies bytes `+2..3`.

## 2. Static model instances (`cS3DModelInstance`) *[compat]*

An object of `0xA4` bytes from a pool (`operator new` `0x00801B00`, released by `0x008020F0`; constructor
`0x00802950`, destructor `0x00802620`, deleting destructor `0x00802B00`). It is also a **mesh container**
(IID `0x26A68448`): the meshes (12-byte records: vertex block, index block, primitive block, each counted) and the
materials (52-byte records, graphics-formats §3.8) of one model frame.

Static construction starts with empty mesh/material arrays, null owner and model, custom data 0, properties 2,
an identity transform with counter 0 and byte `+1` zero, and stored bounds min=(1,1,1), max=(0,0,0)
(`0x00802950`–`0x00802AFE`). The bounds-refresh flags start set. Properties store only the low 16 bits
(`0x008017A0`), unlike the basic class's full 32 bits. Static instances have a **32-bit**, initially zero,
reference count shared by their instance and container interfaces: increment modulo 2³², Release destroys
at 0 or 1 (`0x004F0240`, `0x008016B0`, static vtable `0x00ABD554`). The static factory explicitly adds the
first reference after construction (`0x00804104`–`0x00804108`); the basic 16-bit rule must not be applied to it.

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
| 9, 10 | `GetTransform`, `SetTransform` | transform words/kind copied with destination-local byte-counter increment; §1.2 (`0x00801B80`, `0x00801BE0`) |
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

### 2.1 Bounds and pick distance (MI4, MI5)

GetBounds and GetWorldBounds share the cached local box. If it is stale and a model exists, they fetch model
slot 7 and clear the stale flag; without a model they copy the stored box and leave that flag set
(`0x00801FC0`, `0x00802010`). Explicit UpdateBounds does the same refresh when there is a model
(`0x00801CD0`). GetWorldBounds then applies the full matrix, uniform scale and translation; it does **not**
dispatch on the transform kind byte or skip an empty/inverted box (`0x007F8170`).

For each input axis, first multiply both endpoints by s, storing six binary32 scaled endpoints. For each output
axis r, start its min and max at translation component t_r. In input-axis order x,y,z, form the two products
M[r,j] × scaled endpoint j. Add the smaller contribution to min and the larger to max, storing each sum as
binary32 before the next contribution (`0x007F81C0`–`0x007F8270`). This is the enclosing box of the eight
transformed endpoint combinations for finite ordinary inputs, including negative scale and nonsymmetric M;
it is not a transform of just min and max. Inverted endpoints are reordered by the product comparisons,
so an inverted sentinel box is **not preserved as empty**. For numeric emulation, the first endpoint product is
stored as binary32 before comparison; the second remains in x87 until used. The comparison selects the first
product as min only when it is strictly less; equal or unordered selects the second as min. No NaN sanitation
or explicit finite/empty test occurs here.

Picking stores **Euclidean distance**, sqrt((hit−origin)·(hit−origin)), not signed ray parameter t. The triangle
intersection helper writes the hit point as origin+t×direction (`0x005D719B`–`0x005D71D4`); TrianglePick and
the inlined MeshPick path then call LengthOfLine (`0x007F6B23`, `0x007F9993`; square root `0x005D7688`).
Only a strict, ordered distance decrease updates the nearest field; equal and NaN comparisons do not hit
(`0x007F6B28`–`0x007F6B35`, `0x007F9998`–`0x007F9A07`). A geometrically accepted intersection behind the
origin can therefore update a **positive** nearest distance. MeshPick returns after the first improvement
within that mesh; ModelPick still visits subsequent meshes (view-utilities §7). Nonunit directions do not
change the distance to a given hit point.

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

### 3.1 Native numeric boundaries (MI9)

Construction uses the static base and initializes the subrange pointer null, current frame 0, mode 3,
direction +1, speed −100.0, accumulator +0 and catch-up false (`0x007FD870`). A failed frame build returns
false without updating the current-frame word; a same-frame build returns true without calling the model
(`0x007FC2E0`). This is a guarantee about the frame word, not transactional rollback of an arbitrary external
frame provider's writes into the supplied mesh container. No native container snapshot is taken.

SetSpeed stores all inputs other than the exact −200.0 sentinel literally, including NaNs and infinities
(`0x007FC170`). Advance only recognizes the exact −100.0 bits as unset and ordered zero speed as stopped.
It computes (dt×speed)×signed-direction+acc using x87, stores acc as binary32, and **retains the unpopped
x87 result** for its first magnitude comparisons (`0x007FC1F9`–`0x007FC235`). Do not prescribe rounding to
binary32 after every arithmetic operation or fuse operations. Storage precision and transient precision are
different: exact edge rounding additionally depends on the active x87 precision control.

An unordered magnitude comparison at the first threshold (1.0) returns 0 without Step, after the accumulator
store (`0x007FC219`–`0x007FC224`, `0x007FC2C0`). Thus NaN speed, NaN dt, and 0×infinity normally leave a NaN
accumulator and do not advance. An infinite magnitude passes both finite thresholds: with the shipped
catch-up=false state, Step runs once, accumulator is overwritten with 1.0, and Advance returns 2
(`0x007FC27B`–`0x007FC292`). If an external API exposes catch-up=true, infinity can keep its native loop
running indefinitely; that flag has no setter in this published native owner. For finite ordinary inputs the
playback rules in graphics-formats §3.9 continue to apply.

## 4. The model processor (`cS3DModelProcessor`) *[compat]*

A helper that relights a static model instance on the CPU. Its state: the instance, its mesh container, the target
vertex format (standard 10: position, colour, uv; graphics-formats §3.5), the original vertex blocks, the vertex
normals per mesh, and the mesh order. Constructor `0x00803060`; destructor `0x008030A0`.

- `SetModel(instance)` (`0x008038D0`):
  1. A previous instance first gets its original vertex blocks back; then the new instance is taken.
  2. An animated instance (IID `0xA818EDD8`) with more than one frame is refused: false, nothing kept.
  3. Its mesh container (IID `0x26A68448`) must exist, be nonempty and have every vertex block in standard
     format 2 (position, uv). Otherwise both instance and container references are cleared and the result is false.
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
- `SortMeshes(direction)` (`0x00802DB0`): rebuilds the order from container indices 0 upwards on every call.
  It inserts each index before the first previous score for which the new score is **not strictly less**.
  Scores are the dot product of mesh-box centre with direction, largest first for finite unequal values.
  Equal scores (including signed zeros) reverse container order. An unordered comparison also inserts before
  that entry; this is not a stable sort (`0x00802E82`–`0x00802EFC`).
- `ReleaseModel()` (`0x00802B30`): clears/releases the instance, releases the container, leaves the container
  pointer stored, and marks that reference already released. It does not restore vertices or clear the saved
  original blocks. It requires a nonnull container and is not safe to repeat as a native operation.

### 4.1 Ownership, returns and restoration (MI6, MI8)

Construction has null instance/container, target vertex format 10, empty saved-block/normal/order arrays, and
the already-released flag false (`0x00803060`). A successful SetModel owns separate counted references to the
instance and its queried mesh container, even when they are parts of the same allocation. It also owns one
reference per saved original vertex block; each container mesh owns its replacement block. The target format
does not relax the eligibility check for original format 2. Null input, absent container, zero meshes and any
non-format-2 block all return false with neither instance nor container retained
(`0x00803AFC`–`0x00803B0D`, `0x00803DD6`–`0x00803E03`). The multi-frame animation case is an earlier refusal.
Only completion of a nonempty conversion returns true (`0x00803DCA`).

On SetModel, the previous container's vertex slots are restored before its saved originals and normals are
cleared and its container reference is dropped. Rebinding the same instance still performs that restoration
and conversion. Destruction also restores saved originals: **first** release/clear the instance, **then** restore
each original vertex slot, **then** release/clear the container unless ReleaseModel already released it,
and finally discard the processor arrays (`0x008030A4`–`0x0080312F`). Reference replacement retains the restored
block before releasing the replaced block. Thus ordinary destruction restores vertices, unlike ReleaseModel
alone. After ReleaseModel, a later destruction/rebind can still use that borrowed container pointer; an external
owner must keep it valid. Repeated ReleaseModel, ReleaseModel with no container, and use of a dangling released
container have no defined safe native result. An idempotent portable clear is a separate API choice.
The shipped direct caller is network `FlipAndLight`, which calls ReleaseModel after LightModel
(`0x00616CA6`–`0x00616CAD`); this supports its transfer-of-ownership use, not an idempotency guarantee.

Static SetModel's material changes affect the **instance material copy**, not the source resource material.
The original frame builder copies source material fields into the destination material array
(`0x00806E41`–`0x00806E59`, copy helper `0x00802240`), while retaining shared texture bindings. SetModel changes
the instance's first material bytes at `+4` and `+0x18` (`0x00801C92`, `0x00801C9B`). Sharing texture bindings
does not make the material flags shared. A second instance built from the resource therefore starts with the
resource's original flags. The model pointer itself is counted, with retain-new/store/release-old order and a
same-pointer shortcut, but the material inspection still runs after that shortcut (`0x00801C40`).

Sort's centre computation uses each original container mesh's current vertex-block bounds. It resets the
output list, not the container mesh array (`0x00802DF8`, `0x00802E22`). The three min+max sums and dot product
use x87 operations; the score is stored as binary32 before comparison (`0x00802E28`–`0x00802E79`). The insertion
rule above applies literally to NaNs as well as finite scores; it does not establish a total order for NaNs.

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

CreateInstance allocates an unread object and queries the requested interface; successful query supplies the
returned counted reference. Unsupported type or unsupported requested interface returns false and null;
the latter destroys the new allocation (`0x00808BD0`). SerialRead dispatches on the existing resource's type
and returns the reader's Boolean; it does not itself release or null the resource on failure (`0x00808B20`).
Neither constructor invents unread animation headers. The table constructor sets its block pointer null
(`0x00808BA0`); its destructor frees that block once, then destroys the persistence base (`0x00808AD0`). The
animation destructor destroys the persistence base and allocation, with no sheet references to release
(`0x00808A00`). Both own-IID queries retain the returned interface and otherwise delegate to the persistence
base (`0x008085C0`, `0x00808630`); the table's **resource type is not its own IID**.

The persistence base initializes its count to zero (`0x00975633`). Both sprite resources use a **32-bit atomic**
increment/decrement through `0x009753B7`/`0x009753C5`, via wrappers `0x00808600`/`0x00808610`. Release decrements
first and destroys only when the result is zero. Consequently Release on constructor count 0 wraps to
`0xFFFFFFFF` without destruction, unlike basic/static instance Release; a successful factory query first
produces count 1. Overflow from `0xFFFFFFFF` to 0 is likewise unprotected. The inherited factory count also
starts 0 (`0x0097564D`), increments a 32-bit word under its lock (`0x009756CF`), and destroys on Release when
the pre-release count is 0 or 1 (`0x0097545B`). These are distinct native count policies, not a universal rule.

Animation CreateModelInstance allocates a sprite instance, takes a temporary reference, calls its resource
binding operation, and on success writes a counted result; failure writes null and drops the temporary
reference (`0x00808A30`–`0x00808AC5`). Sprite binding/drawing belongs to animated-sprite.md. Allocation failure
is not a proved safe false/null path: the original contains dereferences after the allocation-failure branch.

### 6.1 Sprite animation (`ReadAnim`)

All fields are little-endian `u32`:

1. A type tag, which must be `0x29A5D1EC`.
2. Five header values *[inferred: not interpreted here]*.
3. Five per-zoom table keys k₀–k₄ (instances of `0x09ADCD75` tables). k₀ and k₁ are read and replaced by 0. A key
   below the resource's own instance I counts as relative: k := k + I (so 0 means I).
4. One more value.

Then each zoom z gets a source zoom and a scale s_z, with the zoom sizes P = (8, 16, 32, 73, 146) (`0x00ABD668`):

- **Downwards:** start with current key zero and no usable source. If k₄ is nonzero, zoom 4 becomes the
  source and s₄ is written as 1; otherwise s₄ is **unwritten**. For z = 3, 2, 1, 0: a different key becomes
  the current key/source and s_z is written as 1, **including when that different key is zero**. An equal
  nonzero key shares the source and writes s_z = P_z ÷ P_source. An equal zero key leaves s_z unwritten;
  at z=0 that equal-zero case returns false immediately. After a different key at z=0, a zero current key
  also returns false. Only a nonzero final current key reaches the optional upward pass.
- **Upwards, only when k₄ = I:** start with no source. For z = 0 … 4, a key other than I becomes the source,
  leaving its downward scale unchanged; a key equal to I takes the last source's key and scale P_z ÷ P_source
  **only if a nonzero source has already been found**. Leading I keys retain their downward key and scale.
  The source is not inherited from the downward pass (`0x0080884E`–`0x008088D9`). All-I keys therefore retain
  the downward scales (8/146, 16/146, 32/146, 73/146, 1); they do not get reset to five unit scales (MI7).

The key comparison is unsigned and relative-key addition wraps at 32 bits (`0x00808731`–`0x0080873B`). Each
scale ratio is evaluated by x87 division then stored as binary32. Zero is a source-state sentinel, not an
immediate rejection: a later different nonzero key can replace it. In the upward pass a non-I zero key also
replaces the current source with zero, disabling later I-key substitutions until another nonzero non-I key.
Instruction evidence: zoom-4 gate/store `0x00808770`–`0x0080878C`, descending unequal/equal-zero branches
`0x00808790`–`0x00808836`, final gates `0x0080881E`–`0x0080883B`, upward source replacement and nonzero tests
`0x0080884E`–`0x008088F2`. I=0 forces keys 0 and 1 to zero and fails, even if higher keys were nonzero.
This is separate from opaque MI1 fields, which this reader still only copies.

#### 6.1.1 Literal normalization/write vectors (MI13)

Each resource below consists of twelve little-endian u32 words: type `0x29A5D1EC`, five zero header words,
the five input keys shown, and a final zero word. I is the resource's instance supplied by its stream, not
an additional resource word. Give the destination scale slots distinct sentinels S₀…S₄ before ReadAnim;
`unchanged` means no write, not a computed zero or unit scale. All reads succeed. All five normalized keys
and the final zero word are stored **before** normalization's scale pass; false does not roll these back.
The source column describes the denominator's zoom, not a separate stored source-index array.

| Case | I | Literal input keys (z=0…4) | Final keys | Return | Final scales (z=0…4) | Scale sources |
|---|---|---|---|---|---|---|
| Interior zero | `0x80000001` | `(0,0,0x7FFFFFFF,0,0)` | `(I,I,0,I,I)` | true | `(1/2,1,1,1/2,1)` | `(1,1,2,4,4)` |
| Initial zoom-4 zero | `0x80000001` | `(0,0,0,0,0x7FFFFFFF)` | `(I,I,I,I,0)` | true | `(8/73,16/73,32/73,1,unchanged S₄)` | `(3,3,3,3,none)` |
| Nonzero wrap control | `0x80000001` | `(0,0,0x80000000,0,0)` | `(I,I,1,1,1)` | true | `(1/2,1,1,73/32,146/32)` | `(1,1,2,2,2)` |
| I=0 control | `0` | `(0,0,0,0,0)` | `(0,0,0,0,0)` | false | `(unchanged S₀,unchanged S₁,unchanged S₂,unchanged S₃,unchanged S₄)` | none |

Ratios in this table denote division followed by binary32 storage under the active x87 rounding mode;
the dyadic ratios are exact. These are instruction-derived expectations, not native execution results.
The interior-zero writes occur downward in order 4,3,2,1,0; zero at 2 writes unit scale, then I at 1
starts a new source. Upward traversal encounters zero at 2, so leaves 3 and 4 unchanged. For initial-zero,
there is no scale write at 4 and no upward pass because k₄ differs from I. The nonzero-wrap control first
forms key 1 at zoom 2, then the upward pass rewrites zooms 3 and 4 to key 1 and the ratios shown.

The sprite instance scales a frame by s_z when it differs from 1 (animated-sprite.md §4.1). Zooms 0 and 1 always end
up with the instance's own key or a shared one, but the sprite draws nothing there.

### 6.2 Sprite-animation table (`ReadTable`)

A type tag `0x09ADCD75`, then the rest of the resource as one block: valid when its size is 32 + 8 n, where n is
the 32-bit value at block offset `0x1C`. So a table is a 32-byte header holding the entry count n at `+0x1C`,
followed by n entries of 8 bytes (animated-sprite.md AS1 asks what the header and entries hold).

For well-formed tables, total stream size is 36+8n including the tag. Native validation subtracts 4 from the
stream size and compares the remaining byte count to **wrapping u32** (32+8n), without first checking a
32-byte minimum (`0x00808957`–`0x00808988`). Checked arithmetic and rejection of short headers or wrapped
counts in a portable reader are memory-safety choices, not evidence that the native parser rejected them.

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
value of a sprite animation hold; what `flip` in `LightModel` is used for. MI3–MI10 are resolved in §§1.1–1.2,
2.1, 3.1, 4.1, 6.1–6.2 and 9.1; the answers distinguish native behaviour from portable safety choices.

### 9.1 Valid input and memory-safety boundary (MI9)

Compatibility for mesh traversal requires valid live blocks, sufficient stride/data, valid primitive groups,
and in-range indices/vertices. Native GetMesh and GetMaterial directly calculate addresses with no range
check (`0x00801AF0`, `0x00801EC0`). Triangle walking likewise uses group.first to offset the index or vertex
pointer, then directly reads selected indices and positions (`0x0080B320`, `0x0080B090`); there is no rule that
invalid references are skipped. Rejecting malformed meshes, skipping invalid triangles, checked allocation
arithmetic and safe null handling must be identified as **portable choices**, not native contracts.

Small primitive counts need a finer distinction: the count helper returns signed n−2 for strips/fans/type 7
and n−1 for type 5, with no saturation (`0x0080AEA8`, `0x0080AEB8`). The iterator starts at 0 and uses a signed
less-than against that result (`0x0080B3B6`–`0x0080B3BE`), so negative or zero results yield no triangles.
For ordinary nonnegative counts this has the same traversal outcome as a saturating count; it does not prove
that the native helper saturates. Counts with the high bit set, overflowed offsets, malformed short table
headers (§6.2), absent material 0 in SetModel (§2), reference overflow (§1.1), and
released/dangling processor pointers (§4.1) do not acquire safe semantics merely by entering a portable API.
Floating-point NaNs/infinities have the explicit comparison behaviour above and in §§2.1/4.1; invalid memory
access has no deterministic portable outcome to reproduce.

## 10. Conformance tests

1. **Basic transform:** a lot model's `GetTransform` gives the identity even after `SetTransform`.
2. **Sort value:** a static instance whose first material's first texture is `0x12345678` sorts by `0x12345678`.
3. **Processor refusal:** `SetModel` with a two-frame animated instance is false.
4. **Normals:** two triangles of equal area at right angles sharing a vertex give it a normal of length
   1/√2 ≈ 0.707.
5. **Mesh order:** boxes centred at z = 1, 3, 2 with direction (0, 0, 1) sort as 3, 2, 1.
6. **Shared sheet:** keys k₂ = k₃ = k₄ = K ≠ I: zoom 3 uses K with scale 73 ÷ 146 = 0.5, zoom 2 with 32 ÷ 146.

The following independent vectors discriminate the MI3/MI10 rules; they are expected behaviours, not results
from a reimplementation:

7. **Defaults:** construct a basic instance: owner=null, custom=0, properties=2, count=0. A static instance has
   properties=2 too. Setting properties to `0x12345678` reads back that value on basic and `0x5678` on static.
8. **Draw bytes:** set a 20-byte record with byte 0=`0xA1`, byte 1=`0xB2`, bytes 2..3=`0xC3,0xD4` and distinct
   final words. Get returns all 20 bytes unchanged. A blend-pair SetModel changes byte 0 to `0xAF`, preserving
   byte 1 and bytes 2..19 except independently specified material changes.
9. **Counter source and wrap:** with instance counter 255 and source counter 17, SetTransform leaves the
   instance counter 0; a second set leaves 1. GetTransform into a destination whose counter is 40 yields 41,
   irrespective of the instance counter. MakeIdentity with counter 255 and byte `+1` nonzero yields counter 0;
   repeating it with byte `+1` zero makes no further changes.
10. **Count width:** on a basic allocation at count 65535, AddRef returns 0 and Release destroys once, returns 0.
    On a static allocation at count 65535, AddRef returns 65536 and Release returns 65535 without destruction.
    Release on a newly constructed zero-count basic or static allocation also destroys it. No access after
    destruction is part of this vector.
11. **Owner events:** replacing A by B gives `Release(A), AddRef(B)` on basic and `AddRef(B), Release(A)` on
    static; setting B again gives no calls. Clear then destroy produces only one Release(B).
12. **All-own sheets:** I=1000 and all five input keys 0 produce five keys 1000 with scales
    (8/146,16/146,32/146,0.5,1), rounded on binary32 storage. With inputs (arbitrary,arbitrary,2000,2000,0),
    output keys are (1000,1000,2000,2000,2000) and scales (8/16,1,32/73,1,146/73): the first two retain their
    downward results, zoom 2 keeps its downward scale when it becomes the upward source. I=0 fails.
13. **Sort restart and ties:** container centres A=(0,0,0), B=(1,0,0), C=(2,0,0). Direction +x gives C,B,A;
    direction +y next gives C,B,A again, since equal scores reverse original container order. Starting with
    direction −x instead gives A,B,C, but +y still gives C,B,A. Scores (2,NaN,1) in container order give C,B,A;
    scores (+0,−0,+0) do too. These distinguish restart, ties and unordered insertion separately.
14. **Processor return/lifetime:** a two-frame animated instance, a non-format-2 container, an empty container,
    a missing container, and null input each give false and no retained instance/container. A nonempty
    format-2 container gives true. Hold an external container reference: after successful conversion,
    ReleaseModel alone leaves format-10 vertices; subsequent destruction restores the exact original block
    identities without a second container Release. Destruction without ReleaseModel restores them too.
15. **Material isolation:** first frame-copy a source material with blend bytes (8,1), flag `0x80` and nonzero
    byte `+0x18` into two independent destination containers, before either destination's SetModel inspection.
    SetModel on the first clears its flag `0x80` and byte `+0x18`, and ORs its draw byte 0 with `0x0E`;
    source and the second, still unbound copy retain their flag and byte. Then SetModel on the second applies
    the same changes to that copy and its draw attributes; the source still retains the original values.
    Two already-bound instances both have the cleared values. Texture binding identities may remain shared.
16. **Asymmetric bounds:** min=(1,2,3), max=(4,5,6), matrix rows (1,2,0), (0,1,3), (4,0,1), scale −2,
    translation (10,20,30) yield min=(−18,−26,−14), max=(0,−2,16). This uses exact small-integer arithmetic.
    Full-transform bounds give that result even when the kind byte is 0. Identity applied to the constructor's
    inverted min=(1,1,1), max=(0,0,0) gives min=(0,0,0), max=(1,1,1).
17. **Distance, sign and ties:** origin=(0,0,0), direction=(0,0,2), triangle vertices (−1,−1,6), (1,−1,6), (0,1,6),
    nearest=100: accepted hit stores 6, not t=3. Reversing direction still stores 6 for the accepted behind-origin
    intersection (t=−3). With nearest=6 or nearest=NaN, there is no improvement. Two accepted triangles in one
    mesh at distances 6 then 2 leave 6; placing them in separate meshes permits the later mesh to reduce it to 2.
18. **Playback nonfinite:** with F=4, frame=0, direction=+1, accumulator=0, mode=3, explicit speed=NaN and dt=1,
    Advance returns 0, frame stays 0 and accumulator becomes NaN. With speed=1 and dt=+infinity, shipped
    catch-up=false returns 2, frame becomes 1 and accumulator=1. With speed=+infinity and dt=0, it returns 0.
19. **Short primitives and invalid data:** type 1 count 1 produces helper count −1, then no triangles; type 5
    count 0 does likewise. An out-of-range vertex index is a portable rejection/skip vector only, with no
    native expected safe result. Table total size 36 with n=0 is valid; total size 44 with n=1 is valid;
    size 44 with n=0 fails. A wrapped count such as n=`0x20000000` is outside the safe native table contract.
20. **Resource counts and interface:** a newly constructed sprite table has count 0 and null block; querying
    `0x69B6F01C` succeeds and yields count 1, then Release destroys it. Querying resource type `0x09ADCD75` as
    an IID fails and leaves the supplied query-output sentinel untouched; factory CreateInstance failure
    instead writes null. On a separately constructed zero-count resource, Release returns `0xFFFFFFFF`
    without destruction. These distinguish resource count policy, instance policy and type-vs-IID.
