# Spec: the 3D draw context (`SC4DrawContext`, `cSC4PackedRenderState`)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`). Implementers must work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled
or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[compat-limit]* depends on something FreeCity4 cannot reproduce, so only the rule is matched.

Related specs: [view3d.md](view3d.md) (§1–§2: why the 3D view draws through this object and not the framework's 2D
draw context; §3.1 `+0x11C`; §4.1 the draw pass that calls `SetDefaultRenderStateUnilaterally`),
[graphics-formats.md](graphics-formats.md) (§3.8: the S3D material record, its flags and enums, and the texture
filter overrides), [texture-bindings.md](texture-bindings.md) (§3: the material object), [camera.md](camera.md)
(§1.3–§1.4: the view transform and projection the context loads), [lighting.md](lighting.md) (§4.3:
`GetModelLight`), [render-properties.md](render-properties.md) (the boolean, integer and float properties read
here), [view-utilities.md](view-utilities.md) (§7.1: the mesh layout and primitive types).

## 0. Summary

Every occupant, prop and model of the city is drawn through one `SC4DrawContext`. It sits on the graphics driver
(the `cIGZGDriver` of the SDK, an OpenGL-like interface) and keeps a **cache of the driver's state** in one 32-bit
word, so that a state is only sent to the driver when it changes. On top of the cache it offers **packed render
states** (a value and a mask over that word), the conversion of an S3D **material** into such a state with its
texture stages, the model-view **transform** composed from the camera and the model, model **shading** from the
lighting manager, and the drawing of meshes and whole model instances, with an emulated second texture pass when
the second stage is off.

## 1. The object *[compat]*

0x200 bytes, created by the 3D renderer's constructor (view3d.md §3.1, `+0x11C`); constructor `0x007D5F10` clears
every field, gives the highlight-colour vector (`+0x54`) 11 entries and the texture-stage table (`+0xE0`) none.
The renderer's `Init` (`0x007C6410`, at `0x007C670C`–`0x007C67F0`) then fills the links and calls `Init`:

| Offset | Content | Source |
|---|---|---|
| `+0x08`, `+0x0C` | two ints | the renderer's `+0x68`, `+0x6C` |
| `+0x10`, `+0x14`, `+0x18` | the zoom as a float and as an int, the rotation | the view utilities |
| `+0x24` | the view utilities | the renderer's `+0x114` |
| `+0x28`, `+0x2C` | two values | the renderer's `+0x14`, `+0x18` |
| `+0x30` | the **graphics driver** | the global `0x00B4E1EC` |
| `+0x34` | the renderer | |
| `+0x38` | the **render properties** | the renderer's `+0x48` |
| `+0x3C` | the **camera** | `SetCamera` (§8) |
| `+0x40` | the camera control | the renderer's `+0x8C` |
| `+0x44` | the **lighting manager** | the renderer's `+0x118` |
| `+0x48`, `+0x4C` | 0 | |

Other state: initialised (`+0x00`); two driver bytes (`+0x78`, `+0x79`) and the driver's texture-stage count
(`+0x7C`); the **state word** (`+0x80`, §2); the default packed state (`+0x88`) and its combiner record (`+0xC0`);
the default shade (`+0x94`–`+0x9C`, RGB); the depth offset (`+0xBC`); the highlight packed state (`+0xCC`) and its
combiner record (`+0xD4`); the stage table (`+0xE0`, 0x68 bytes a stage) and its size (`+0xEC`); the model-view
valid flag (`+0x138`), the model transform (`+0x13C`), the composed transform (`+0x174`) and its 4 × 4 matrix
(`+0x1AC`); a stack of packed states (`+0x1EC`–`+0x1F0`); the shade-override flag (`+0x1F8`); the index base
(`+0x1FC`).

### 1.1 `Init` (`0x007D6320`)

Once: the driver fills a record (driver slot 64); its stage count goes to `+0x7C` and the stage table is sized to
max(count, 2) default stage records (`0x007D3C60`); two of its bytes go to `+0x78` and `+0x79`. Then:

- the 11 **highlight colours** (RGBA): 0 (1, 1, 1, 1), 1 (0, 1, 0, 1), 2 (1, 0, 0, 1), 3 (0.2, 0.2, 0.4, 0.6),
  4 (0.66, 0, 0, 0.7), 5 (0.66, 0, 0, 1), 6 (0.5, 0, 0, 1), 7 (0, 0.37, 0, 1), 8 (0.5, 0, 0, 1),
  9 (0, 0.7, 0.2, 0.3), 10 (0.7, 0, 0.2, 0.3);
- the default packed state `0x230A8587` with mask all ones, and the highlight state `0x030A85B5` with mask
  `0x1FFFFFFF` (§5.2);
- the two combiner records (bytes `+0xC0` 2, 0, 1, …, `+0xC4` 0, 1 and `+0xD4` 2, 2, 0, …, `+0xD8` 1, 2, 0)
  *[inferred: texture-combiner settings for the highlight blend]*.

## 2. The state word (`+0x80`) *[compat]*

| Bits | State | Driver call when sent |
|---|---|---|
| 0 | colour write | `ColorMask` (slot 15) |
| 1 | depth write | `DepthMask` (slot 17) |
| 2 | depth test | `Enable` / `Disable` (slots 44 / 45) of capability 1 |
| 3 | alpha test | capability 0 |
| 4 | blending | capability 4 |
| 5 | texturing on stage 0 | capability 5, on stage 0 (`TexStage`, slot 51) |
| 6 | back-face culling | capability 3 |
| 7–9 | depth function | `DepthFunc` (slot 16) |
| 10–12 | alpha function | `AlphaFunc` (slot 22), with the reference below |
| 13–16, 17–20 | source and destination blend | `BlendFunc` (slot 21) |
| 21–24 | alpha reference, q = round(ref × 15), read back as q / 15 | |
| 25–27 | stage 0 texture-environment mode | `TexEnv` (slot 28) on stage 0 |
| 28 | texturing on stage 1 | capability 5 on stage 1 |
| 29–31 | stage 1 texture-environment mode | `TexEnv` on stage 1 |

The function and blend values use graphics-formats.md §3.8's enums (the compare enum, the blend enum). Mode 4 of
a texture-environment field is the **combiner** (§7.3); the other mode values are the driver's *[inferred]*.

## 3. State setters *[compat]*

Each setter compares with the word and calls the driver only on a change; a value of −1 (or −1.0 for the alpha
reference) means "keep".

| Setter | VA | Behaviour |
|---|---|---|
| `EnableColorMaskFlag(on)` | `0x007D2760` | bit 0 |
| `EnableDepthMaskFlag(on)` | `0x007D2800` | bit 1 |
| `EnableDepthTestFlag(on)` | `0x007D27B0` | bit 2 |
| `EnableAlphaTestFlag(on)` | `0x007D3E90` | bit 3; on is treated as off while `kForceNoAlphaTest` (boolean 8) is set |
| `EnableBlendStateFlag(on)` | `0x007D4010` | bit 4; on is off while `kForceNoAlphaBlend` (boolean 7) is set |
| `EnableCullFaceFlag(on)` | `0x007D2850` | bit 6 |
| `EnableTextureStateFlag(on, stage)` | `0x007D3EF0` | bits 5 and 28: on with stage 0 turns stage 0 on, with stage 1 both; off with stage 0 turns both off, with stage 1 only stage 1; stage 1 reaches the driver only with `kUseSecondStage` (boolean 11); on with a higher stage goes straight to the driver, off with one does nothing |
| `SetDepthFunc(f)` | `0x007D28A0` | bits 7–9 |
| `SetAlphaFunc(f, ref)` | `0x007D2F50` | bits 10–12 and 21–24; the driver gets the unrounded reference |
| `SetBlendFunc(src, dst)` | `0x007D28F0` | bits 13–20 |
| `SetModelViewTransformChanged()` | `0x007D2750` | the model-view becomes invalid (§8) |
| `SetTransparency(a)` / `ResetTransparency()` | `0x007D2A20` / `0x007D2A30` | `AlphaMultiplier` (slot 33) a / 1.0 |

## 4. Render-property overrides *[compat]*

Besides the two "force no" switches of §3: the texture filters and wrap modes are overridden by
`kForcePointSampling`, `kForceBilinear` and `kForceTextureRepeat` exactly as graphics-formats.md §3.8 states, and
nothing above stage 0 reaches the driver without `kUseSecondStage` (§7). Model culling and bounding boxes follow
two more booleans (§9.2).

## 5. Packed render states *[compat]*

### 5.1 `cSC4PackedRenderState::PackRenderState` (`0x007D2B50`)

A packed state is a value and a mask over the word of §2. The arguments, in order: colour write, depth write,
depth test, alpha test, blending, texture count (0: stage 0 off; 1: stage 0 on, stage 1 off; 2 or more: both on),
culling, depth function, source blend, destination blend, alpha function, alpha reference, stage 0 mode, stage 1
mode. The booleans and the count always set their bits; each of the last seven, when −1 (−1.0), clears its field
from the mask instead, so the state leaves it alone.

### 5.2 `SetRenderState(packed)` (`0x007D4530`)

The fields that differ from the word under the mask are written into the word and sent to the driver, group by
group in §2's order, with the force switches of §3 applied to alpha test and blending and stage 1 sent only with
`kUseSecondStage`. Whatever changed, stage 0 then gets wrap repeat / repeat and filters linear /
linear-mipmap-linear (§7), and both stages lose their texture transform.

The default state is colour and depth write, depth test with function 3 (less-or-equal), alpha function 1 (less)
at reference 8/15, blend 4 / 5 (source alpha, one minus source alpha) but blending, alpha test, texturing and
culling off, both modes 1. The highlight state is the same with depth write off, blending and stage-0 texturing on
and stage-1 texturing off; it leaves stage 1's mode alone.

- `SetDefaultRenderState()` (`0x007D5200`): the top of the stack of packed states, or the default when it is empty.
- `SetDefaultRenderStateUnilaterally()` (`0x007D5230`): for every stage, the texture transform and coordinate set
  are reset at the driver (`TexStageMatrix` slot 53 with none, `TexStageCoord` slot 52 with 0); every stage record
  becomes the default record; then the default state. view3d.md §4.1 calls it at the start of each draw.
- `SetRenderStateHighlight(colour)` (`0x007D4FB0`): the highlight state, then the highlight colour `colour` as the
  stage-0 environment colour (`TexEnv`, slot 27) and the highlight combiner (`SetCombiner`, slot 62) on stage 0.

## 6. Material render state (`SetRenderState(material, drawFlags)`, `0x007D5310`) *[compat]*

The material (texture-bindings.md §3) holds its flags (`+0x04`), the depth function (`+0x06`), the alpha function
(`+0x07`), the alpha threshold byte (`+0x08`), the blend pair (`+0x09`, `+0x0A`), the texture count T (`+0x0B`) and
two texture slots of 0x14 bytes from `+0x0C`. The draw flags come with the model instance: a byte of bits and a
float a (`+0x10`). With ref = threshold / 255:

1. Draw-flag bit 3 (**grey**): the driver colour multiplier := (a, a, a), and bit 1 is ignored.
2. Draw-flag bit 1 (**fade**): when a < 1, ref := ref × a and the alpha multiplier := a. Even at a = 1 a faded draw
   is blended with source alpha / one minus source alpha *[quirk]*.
3. The packed state is built (§5.1) from: colour write = flag `0x100`; depth test = flag `0x02`, but on with draw
   bit 5 and off with draw bit 4; depth write = flag `0x80`, but off with draw bit 4 or 5; alpha test = flag `0x01`;
   blending = flag `0x10`, or on when faded; culling = flag `0x08`; the depth function, or 4 (greater) with draw bit
   5; the blend pair, or 4 / 5 when faded; the alpha function; ref; stage modes from slot 0's and slot 1's mode byte
   (−1 for an absent slot). Then §5.2.
4. When flag `0x20` is set, each of the T slots becomes a texture stage (§7): its texture (the slot's binding),
   coordinate set (slot `+0x0D`), wrap and filter bytes, its mode (mode 4 first loads the default combiner record),
   and a **texture transform** when slot `+0x0E` bit 0 is set: coordinate set 16 with s' = k (s − tx) and
   t' = k (ty − t), k = 1 / slot `+0x10` and (tx, ty) the composed transform's translation (§8); otherwise none.

`SetRenderStateHighlight(material, drawFlags)` (`0x007D5670`) uses the highlight state with the highlight colour
given by the draw flags' second byte, binds only slot 0's texture with its coordinate set, wrap and filters.

## 7. Texture stages *[compat]*

A stage record (0x68 bytes; default `0x007D3C60`) keeps the bound texture, the coordinate set, the two wrap modes,
the two filters, a transform flag and, for stage 1, a 4 × 4 transform. Every stage call selects the stage at the
driver first (`TexStage`, slot 51); stage 1 is skipped at the driver, but still recorded, without
`kUseSecondStage`.

| Call | VA | Behaviour |
|---|---|---|
| `SetTexture(texture, stage)` | `0x007D4070` | `SetTexture` (slot 58) |
| `SetTexCoord(set, stage)` | `0x007D4180` | `TexStageCoord` when the set changes |
| `SetTexWrapModes(s, t, stage)` | `0x007D41E0` | `TexParameter` (slot 29) 2 and 3; repeat when forced |
| `SetTexFiltering(mag, min, stage)` | `0x007D4280` | `TexParameter` 0 and 1, with the overrides of §4 |
| `SetTexTransform4(matrix, stage)` | `0x007D4340` | coordinate set 16, `TexStageMatrix`; stage 1's matrix is kept |
| `ClearTexTransform(stage)` | `0x007D4420` | when set: coordinate set 0 and no matrix |
| `SetTexEnvModeForStage(mode, stage)` | `0x007D40B0` | bits 25–27 or 29–31, `TexEnv` (slot 28) |
| `SetTexCombiner(record, stage)` | `0x007D3030` | `SetCombiner` (slot 62); the stage's mode bits become 4 |
| `SetTexColor(colour)` | `0x007D30B0` | the stage-0 environment colour |

### 7.1 The emulated second stage (`SetTexEnvMode`, `0x007D47E0`)

When `kUseSecondStage` is off and a material has two textures, `RenderModelInstance` draws each mesh a second time
after this call. If stage 1 is on in the word: stage 1's recorded texture, coordinate set, wrap modes, filters and
transform are put on stage 0; the depth function becomes 2 (equal) and alpha testing goes off; then by stage 1's
mode: 0, stage 0's mode 0 and blending off; 1, blend 0 / 2 (zero, source colour: a multiply) and blending on; 2,
stage 0's mode 0, blend 4 / 5 and blending on; any other mode keeps blending as it was. The Mac-matched name
`SetTexEnvMode` is kept although the function prepares this pass *[inferred]*.

## 8. Transforms and shading *[compat]*

- `SetCamera(camera)` (`0x007D3D80`): stores the camera, loads its projection matrix (`MatrixMode` 1, `LoadMatrix`
  of the transposed matrix, `MatrixMode` 0) and then the model-view as below.
- `SetModelTransform()` (`0x007D2D20`): when the model-view is not valid, the composed transform := the camera's
  view transform (camera.md §1.3), loaded as a column-major 4 × 4 matrix (rotation rows become columns, scaled by
  the transform's scale when it is scaled, translation in the last column) with `LoadMatrix` (slot 41); valid.
- `SetModelTransform(model)` (`0x007D3400`): records the model transform; the composed transform is the view
  transform (taken anew when the model-view is not valid) followed by the model's: rotation R = R_view R_model,
  translation t = t_view + s_view R_view t_model, scale s_view s_model, flags combined. It is loaded the same way
  and the model-view becomes **invalid**, so the next `SetModelTransform()` goes back to the plain view.
- `SetDepthOffset(n)` (`0x007D4480`): on a change, the camera's depth bias := `kDepthOffset` (integer property 16,
  2 by default) × n through `cS3DCamera::SetDepthBias` (`0x007FF0F0`, which also bumps the camera's projection
  counter), and the projection matrix is reloaded. The selection highlights use n = 4 (water-ripples.md §2.3).
- `SetShade(rgb)` (`0x007D3140`): the colour multiplier := rgb × the default shade; `SetSelfLitShade(rgb)`
  (`0x007D31B0`): rgb as is; `ResetShade()` (`0x007D3290`): the default shade, when an override is pending.
- `SetModelShade(instance, tint)` (`0x007D31E0`): the lighting manager's `GetModelLight(instance)` (slot 11,
  lighting.md §4.3), times the tint when given, as the colour multiplier.

## 9. Drawing *[compat]*

### 9.1 Primitives and meshes

| Call | VA | Driver |
|---|---|---|
| `SetVertexBuffer(format, -, data)` | `0x007D2970` | `InterleavedArrays(format, 0, data)` (slot 5) |
| `SetIndexBuffer(-, base)` | `0x007D2980` | the index base := base |
| `DrawPrims(type, format, n, data)` | `0x007D2990` | the vertex array, then `DrawArrays(type, 0, n)` (slot 3) |
| `DrawPrims(…)` | `0x007FC2D0` | `DrawArrays` with the same arguments |
| `DrawPrimsIndexed(type, first, n)` | `0x007D29C0` | `DrawElements(type, n, 3, base + 2 first)` (slot 4; 3 = 16-bit indices) |
| `DrawPrimsIndexed(type, n, indices)` | `0x007D29F0` | `DrawElements(type, n, 3, indices)` |

`RenderMesh(mesh)` (`0x007D4A80`): a mesh with vertices (view-utilities.md §7.1's layout) sets its vertex array,
then draws each group (type, first, count): without indices `DrawArrays(type, first, count)`, with them the index
base := the index data and `DrawElements(type, count, 3, base + 2 first)`.

### 9.2 `RenderModelInstance(instance, model, drawFlags, highlight)` (`0x007D5710`)

1. Not highlighted, with `kShowBoundingBoxes` (boolean 26): the instance's box (instance slot 8) is drawn in red
   (1, 0, 0, 1) and, when its owner (occupant `0x26BB767E`) is an effects collection, the collection's box (slot
   10) in blue at alpha 0.5; nothing else is drawn. Highlighted with draw bit 3 set: nothing is drawn.
2. With boolean 30 (`kNoPartialBackingStoreCopies`) set: the instance's largest box extent × the view utilities'
   pixels per metre must reach `kRenderModelThreshold` (float 9), else nothing is drawn *[quirk]*. The shipped
   `Graphics Rules.sgr` sets the threshold from the City Detail option (12, 6, 2 for low, medium, high) and turns
   the boolean on only for some older Radeon cards (its comment: a "special low-impact scrolling mode"), so only
   those cards cull small models while drawing.
3. `SetModelTransform(instance's transform)`; the shade: `SetModelShade(instance, drawFlags + 4)` normally, or the
   draw flags' colour (`+0x04`) as is when highlighted or with draw bit 2 (then restored at the end).
4. For each of the model's meshes: its material state (§6, or the highlight form), `RenderMesh`, and the emulated
   second pass (§7.1) when `kUseSecondStage` is off and the material has 2 textures.
5. The alpha multiplier goes back to 1.

### 9.3 `DrawBoundingBox(box, colour)` (`0x007D5030`)

A shared mesh builder makes the box as six lit quads in the colour (`AddBoxMesh`, geometry-utilities.md §6.2); it
is drawn with the packed state `0x020A8197` / mask `0x1E1FE3FF` (writes, depth test less-or-equal, blending 4 / 5)
when the box is not empty.

## 10. Users

The 3D renderer's draw pass (view3d.md §4) sets the camera and the default state; every model and occupant `Draw`
method draws through `DrawPrims` and `RenderModelInstance` (58 call sites of `DrawPrims` alone, view3d.md §1); the
terrain view, the ripples and the selection (water-ripples.md) use the state setters and `SetDepthOffset`.

## 11. Quirks

1. A faded draw is blended even at full alpha (§6).
2. Small-model culling hangs on a boolean named for backing-store copies (§9.2).
3. Stage 1 state is recorded but not sent without `kUseSecondStage`, and is then replayed on stage 0 (§7.1).
4. The draw flags' bit 5 forces the depth function "greater" with the test on and no depth write: models drawn so
   appear only where something is in front of them *[inferred: the x-ray outline]* (§6).

## 12. Open questions

See [questions/draw-context.md](questions/draw-context.md) DC1–DC2: the driver record of `Init` (slot 64) and the
bytes kept at `+0x78`, `+0x79`; the draw flags' bits and who sets them.

## 13. Conformance tests

1. **Cache:** `EnableDepthTestFlag(true)` twice sends one `Enable(1)`.
2. **Force switch:** with `kForceNoAlphaTest`, `EnableAlphaTestFlag(true)` leaves alpha testing off.
3. **Alpha reference:** `SetAlphaFunc(1, 0.5)` stores q = 8 and a later state replay sends 8/15.
4. **Packed mask:** a packed state with depth function −1 leaves bits 7–9 unchanged.
5. **Default:** `SetDefaultRenderState` with an empty stack applies `0x230A8587`.
6. **Composition:** view rotation identity, translation (1, 2, 3), scale 2; model translation (1, 0, 0): the
   composed translation is (3, 2, 3) and the scale 2.
7. **Depth offset:** with `kDepthOffset` 2, `SetDepthOffset(4)` sets the camera's depth bias to 8.
8. **Second pass:** stage 1 mode 1 gives blend (0, 2), depth function equal, alpha test off.
9. **Culling:** with boolean 30 set, a model whose largest extent × pixels per metre is below the threshold is
   skipped.
