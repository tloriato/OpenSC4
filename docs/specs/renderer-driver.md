# Spec: the hardware 3D driver (`nSGLDX7::cGDriver`, "Maxis 3D GDriver DirectX")

Status: **implementation spec (interface coverage; residual question RD3)**, clean-room input. The
83 graphics slots have a contract or an explicit unsupported/no-op classification (§12); the owner inventory
distinguishes auxiliary interfaces and unresolved pixel-converter details. This document describes *behaviour*, not code. It was
written by the analysis team from observing the original program. Implementers must work from this spec, the public
LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled or disassembled code
(see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tags: *[compat]* required for compatibility; *[choice]* our choice
where the original was arbitrary; *[inferred]* consistent with all evidence but not proven; *[quirk]* surprising
original behaviour kept on purpose. Addresses are SimCity 4 Deluxe 1.1.641 (GOG) virtual addresses, cited only so
the analysis team can re-check a claim.

Related specs: [view3d.md](view3d.md) (the renderer `cSC43DRender`, the main client of this driver, and its
handling of the driver's messages, §3.4), [foundations.md](foundations.md) (messages §5, x87 precision §18).

## 1. Scope and owners

The game draws through an abstract "graphics driver" interface with an OpenGL-like vocabulary (matrix modes,
`Enable`/`Disable` capabilities, texture names, `DrawArrays`/`DrawElements`). Three classes implement the same
83-slot interface table; this spec covers **only the DirectX 7 implementation**:

| Interface table | Implementation | Covered here |
|---|---|---|
| `0x00AC4660` | `nSGLDX7::cGDriver` (DirectX 7 hardware driver; slots named in `docs/abi/vtables.tsv`) | yes, §2-§11 |
| `0x00AC36D0` | an OpenGL driver (slot bodies at `0x0087C330`-`0x0087ED20` call the `gl*` imports) *[inferred]*; the export files some of its bodies (`GetBoolean 0x0087CEE0`, `GetFloat 0x0087CF20`, `QueryInterface 0x0087D670`, `VertexFormatElementOffset 0x0087C4C0`) under `nSGLDX7::cGDriver` | no |
| `0x00ACC4D8` | a third driver (bodies `0x008DA8C0`-`0x008E1140`, near `nRZSimGL`) *[inferred: software / null driver]* | no |

Owner patterns covered: `nSGLDX7::cGDriver::*` bodies in `0x00881DF0`-`0x00889260` (the device, window, state and
scene lifecycle), `nSGLDX7::TranslateColorToD3D` (`0x00881AB0`), the default device state (§7),
`nSGLDX7::cPrimitiveManager` (`0x00889B60`-`0x0088A380`, §8.3) with slots 3-5 and 30-34 (§8),
`nSGLDX7::cTextureManager` (`0x0088A380`-`0x0088B2C0`, §9, including the internal-format selection
`0x0088F8D0` and the converter table, §9.6; the per-routine pixel arithmetic is only sampled) with slots 24-26,
35-39, 48, 58-61, the texture-stage records with slots 27-29, 51-57 and 62 (§10), the vertex-format slots 6-10
(§8.5) and the matrix slots 40-42 (§8.6); slots 67-76 are §11. The remaining graphics slots and the owner inventory
are §12, cache boundaries are §13, vertex-buffer and presentation support are §14, and independent vectors are
§15. Auxiliary lighting/material and buffer-region interfaces are §§12.4–12.5. CopyColorBuffer and the
remaining pixel-converter arithmetic have explicit boundaries (§16); auxiliary interfaces do not add slots
to the 83-slot graphics interface.

The separate `cSC4GDriversCOMDirector` registration/lifecycle contract is specified in
[renderer-gdrivers-director.md](renderer-gdrivers-director.md); it does not extend this driver's scope.

The driver object exposes four COM-style interface pointers at object offsets `+0x0`, `+0x4`, `+0x8` and `+0xC`
plus a reference-counted sub-object at `+0x10`; the 83-slot graphics interface is the one at `+0xC`. Slots 0-2
are the usual `QueryInterface`/`AddRef`/`Release`. Slot 78 `GetGZCLSID`, slot 77 `GetDriverInfo`.

## 2. Object state at construction (`0x00885F90`)

A new driver owns one texture manager (object of 0x348 bytes) and one primitive manager (0x40 bytes), created in
the constructor. Everything else starts empty: no window, no DirectDraw / Direct3D / device objects, no current
video mode, error code 0, "initialised" false, "device lost" false, "scene open" false. The diagonal of a static
4×4 matrix at `0x00B542F8` (elements at `0x00B542F8`, `0x00B5430C`, `0x00B54320`, `0x00B54334`) is set to 1.0;
the rest of it is zero, so it is the identity used by `LoadIdentity` (§8.6). *[inferred]* the 13-dword
current-mode record (§4) starts with mode index -1.

The current-mode index is explicitly -1, not merely inferred. Clear colour is packed zero, clear depth is
binary32 1.0 and clear stencil is zero (owner `+0x190`, `+0x194`, `+0x198`). The complete D3D material at
owner `+0x28C` is zero: diffuse, ambient, specular, emissive RGBA and power, including alpha components
(`0x0088604C`–`0x00886058`, `0x0088630B`–`0x0088636B`). No conventional white material is implied (RD9).

## 3. The error channel (`GetError`, slot 50)

The driver keeps one sticky error code. `GetError` returns it and resets it to 0 (`0x00882000`). Codes observed:

| Code | Set by |
|---|---|
| 0 | none / cleared by `GetError`, by `BeginScene` before it calls the device, by a successful `Disable` |
| 1 | a device call failed with any failure other than "surface lost" (`0x887601C2`). Every such site writes `code = (hr != 0x887601C2) ? 1 : 0`, so a lost surface **clears** the code *[quirk]* |
| 2 | `SetVideoMode` with a mode index ≥ the number of modes |
| 3 | `StencilFunc`, `StencilMask` or `StencilOp` when the current mode has no stencil buffer |
| 6 | window-class registration failed in `Init`, DirectDraw could not be created, or the window / 3D environment for `SetVideoMode` could not be built |

Codes are overwritten, not accumulated: the last failing call wins.

## 4. Lifecycle

### 4.1 `Init` (slot 79, `0x00881DF0`) and `Shutdown` (slot 80, `0x00881E40`)

`Init` first clears the error code. If the driver is already initialised it does nothing more. Otherwise it marks
it initialised, forgets any window handles, resets one word to 7 (object `+0x138`, purpose open, RD2) and sets
the current mode index to -1, then sets the mode count to 0 and:

1. registers the window class `"GDriverClass (DirectX)"` (redraw-on-resize style, arrow cursor, the driver's own
   window procedure) unless it already exists; on failure the error code becomes 6;
2. creates a DirectDraw object; on failure it shows the message box "Could not initialize Direct Draw." titled
   "Hardware Driver Error" and sets error 6;
3. builds the driver-information text: `"Maxis 3D GDriver DirectX 2.0"`, a newline, the adapter's description,
   a line `"%u.%u.%u.%u, GUID: ..."` with the driver version and device GUID, then
   `"Vendor: %04x, Device: %04x, Board: %04x, Chipset: %04x"`. If the reported driver version is 0.0.0.0 it
   is looked up another way first (`0x00888820`, unspecified). `GetDriverInfo` (slot 77) returns this text
   *[inferred]*;
4. releases the temporary DirectDraw object.

`Shutdown` does nothing unless initialised; otherwise it clears the flag and: switches to "no mode" (slot 66 with
-1, §4.2), empties the mode list through a helper (`0x00886FB0`), sets the mode count to 0, unregisters the window class, and releases every held
DirectX object (the six interface slots at object `+0x218`..`+0x230`).

**Information storage and destruction (RD9).** The `0x00886FB0` helper copies 56-byte mode records and returns
the new list end; Shutdown passes an empty source range and the list beginning as destination, so the list
becomes empty. This assignment is to owner `+0x110` (list end), not owner `+0x11C` (sticky error). It does not
invent a Shutdown error. Driver information is copied/appended into shared static string storage at
`0x00B54340`; `GetDriverInfo` returns its current character pointer directly (`0x008870A0`), with no allocation,
AddRef or output argument. The pointer is borrowed and can be invalidated by later initialization/string
mutation; it does not borrow the temporary identifier buffer (`0x00888D6B`, `0x00888DB6`). Only successful
`GetDeviceIdentifier` (result exactly zero) adds adapter/version/GUID/vendor lines.
When both version dwords are zero, `0x00888820` searches the system directory using the path fragments
`\\`, `\\Drivers\\`, `32\\Drivers\\`, and suffixes empty, `.dll`, `.vxd`, `.sys`, in that nesting order. The
first matching file supplies file-version MS/LS words from the root version resource; failure to read that
file's resource ends the search with failure rather than trying later files. No match leaves the zero version.
The formatted version is MS high16, MS low16, LS high16, LS low16 (`0x00888820`–`0x00888A2B`).

The shared driver's final-reference path calls graphics Shutdown, deletes the driver and returns zero
(`0x00881E80`). Destruction drops the primitive-manager reference, but calls **texture-manager slot 1**
(AddRef), after clearing its owner pointer, rather than Release [quirk, `0x00886450`–`0x008864B3`]. Thus a
driver's destruction must not be claimed to destroy its texture manager. Remaining held context/DirectX
references and owned arrays/string storage are released/freed. A primitive manager actually reaching final
destruction does not submit queued primitives; it releases its vertex-buffer records and frees queue,
index and record storage (`0x00889D40`, `0x0088A140`). A texture manager actually reaching destruction frees
its record storage, format-list storage and held device interfaces (`0x0088B070`); this is separate from the
driver's AddRef quirk. Do not add an implicit texture upload or draw flush to these destructors.

### 4.2 `SetVideoMode(mode, windowProc, show, flag)` (slot 66, `0x00888F50`)

1. If a scene is open, it is ended (§4.4 rule for errors) and the "scene open" flag cleared.
2. `mode == -1`: if a mode is current and a window exists, the 3D environment is destroyed (§4.5), the window is
   destroyed and the current mode becomes -1. Return.
3. Otherwise the fourth argument's low byte is stored (request for a flipping surface chain, §14.2; callers are described after the failure-path note below). If `mode` ≥ the mode count the error
   becomes 2 and the call returns without change.
4. If a mode is current, the driver first calls its own slot 66 with `(-1, 0, 0, 0)`.
5. The window procedure is remembered (`DefWindowProcA` when null). The driver fills its current-mode record from
   `GetVideoModeInfo(mode)` (slot 65). The record holds at least: mode index, width, height, a "full screen" flag
   and a "has stencil" flag.
6. Window: full-screen modes get a popup window of exactly width × height at (0,0); windowed modes get an
   overlapped, visible window whose outer size is computed so that the *client* area is width × height. Window
   class `"GDriverClass (DirectX)"`, title `"GDriverWindow (DirectX)"`. The window's user data points to the
   driver. The window is then positioned at (0,0) with that size.
7. The 3D environment is set up (§4.3). If that fails, it is destroyed again and the window too.
8. If no window remains: error 6 and the failure-path write described below. Else, if `show` is non-zero, the window is shown and
   updated.

**Failure-path pointer hazard [quirk]:** `0x0088916E`–`0x0088917F` loads the fourth argument word and writes
`0xFFFFFFFF` through it after cleanup. It does not write the current-mode index there. The same argument's low
byte was used as the flip request earlier. Passing a small boolean or null can therefore fault on this path.
The supported success path does not perform that write. An implementation must not promise a clean error-only
return for failed setup.

*[compat]* **Callers of the fourth argument.** The only caller in the executable that requests a real mode is the
canvas window-realization helper (`0x0098D9C0`, call at `0x0098DB08`). It passes the matched mode index, its window
procedure bridge, show = 0, and as the fourth argument the result of slot 23 of the graphics system's
driver-control interface (`0x00741740`, reads the one-byte "page flipping" flag; construction sets that flag to 1,
`0x0098FF40`; slot 22, `0x0098AD30`, writes it). The value reaching the driver is therefore exactly 0 or 1, never a
pointer, and with the default flag the flip-chain request is on. Every other caller passes mode −1 with all other
arguments 0 (or 1, 1 from the canvas teardown, `0x0098DBC9`), which returns before the fourth argument is read.
Consequently a failed setup in the realization helper executes the failure write into address 0 or 1 (an access
violation), exactly the hazard above; the intended meaning of the argument is "request flipping", not a result
pointer, and the failure write is a latent defect of the original.

### 4.3 Setting up the 3D environment (`SetupViewport 0x00888E10`, `Initialize3DEnvironment 0x00887A10`)

1. Any open scene is ended.
2. The DirectX objects are created through a helper (`SetupD3DX 0x008876C0`). Failure: message box "Could not
   initialize graphics card." ("Hardware Driver Error"), error per §3, set-up fails.
3. If the device capabilities lack the hardware-transform-and-lighting-class bit (`0x80000` of the first device
   capability word) the set-up fails with the message box "Driver does not support DirectX. This may mean you have
   a Microsoft driver installed instead of the vendor driver..." *[compat: the check and the text]*.
4. If the helper produced no context object: message box "Internal Error.", set-up fails.
5. The device's viewport rectangle is read and kept as (x, y, x+width, y+height).
   Owner primary/render-target/depth bindings and their owned references follow §14.2 (RD12).
6. Initial transforms, all through the driver's own interface: model-view (matrix mode 0) := identity; projection
   (matrix mode 1) := the pixel-space orthographic matrix below; matrix mode back to 0; then `Clear(0x7000)`
   (colour, depth and stencil, §5.3).
7. Device properties are read; on failure set-up fails. The texture manager and the primitive manager are
   initialised against the new device, the default device state is applied (`SetDefaultD3DDeviceState
   0x008872A0`, §7), the window's client rectangle is stored in screen coordinates, and the
   "environment ready" flag is set.

The initial projection (row-major, row-vector convention, viewport width `W` and height `H` converted from
unsigned integers to single floats):

| | col 0 | col 1 | col 2 | col 3 |
|---|---|---|---|---|
| row 0 | 2/W | 0 | 0 | 0 |
| row 1 | 0 | -2/H | 0 | 0 |
| row 2 | 0 | 0 | -1 | 0 |
| row 3 | -1 | +1 | 0 | 1 |

So pixel (x, y) maps to clip (2x/W - 1, 1 - 2y/H): (0,0) is the top-left corner. Worked example: W=800, H=600,
pixel (400, 150) → (0.0, 0.5). The quotients are single-precision (`2.0f / W`, constants `0x00A8825C` = 2.0,
`0x00ABD2D4` = -2.0).

### 4.4 Scenes and frames (`BeginScene 0x008871C0`, `Flush` slot 43 `0x00884BA0`)

There is no public "begin frame" slot: a scene is opened by the environment set-up path and by `Flush`.
`BeginScene` does nothing if a scene is already open or there is no device. Otherwise it clears the error code
and begins a device scene; the scene counts as open on success and on one specific failure code (`0x887602F8`),
otherwise not, and the error code follows §3. It then resets every texture-stage record to its default
(§7 and §10.1) and, if the "clear colour on begin" flag is set, calls `Clear(0x4000)`.

`Flush` is the frame boundary:

* **Device available, not lost**: the primitive manager's queued geometry is submitted (error per §3), the scene
  is ended (`0x00887260`, §11.1; it does not present), the context presents (§14.3), the
  vertex buffer is recreated if it was lost (`0x00889260`), and a new scene is begun.
* **Device just lost** (the cooperative-level test fails and the driver was not already lost): the driver marks
  itself lost, posts message `0xAACB92A3` (*[unlisted]*; *[inferred]* no receiver exists: the only occurrence of that 32-bit value anywhere in the executable image, code or data, is this post at `0x00884E25`, so no handler or subscription names it) through the message server, drops its texture surfaces
  (`0x0088A400`), stops the primitive manager, swaps in a placeholder device object and releases the real
  DirectX objects. `IsDeviceReady` (slot 81) now returns false.
* **Lost, cooperative level restored**: the driver asks its context to restore the surfaces; on failure it ends the
  frame and tries again next `Flush`. On success it clears "lost", re-acquires the device objects, re-applies the
  default device state, recreates the vertex buffer if needed, re-initialises the texture manager for the new
  device, then posts `0x8ACAA95F` (`kSGLMessage_RestoreTextures`) and `0x6ACB92A9` (the renderer's "force full
  redraw", view3d.md §3.4). Clients must re-upload texture contents on `kSGLMessage_RestoreTextures`.

The exact surface references dropped and reacquired on these paths are listed in §14.2 (RD12).

### 4.5 Tearing down (`DestroyViewport 0x008881D0`, `Cleanup3DEnvironment 0x008870B0`)

Ends an open scene; unbinds the texture of every stage in use; empties the stage-record list; shuts down the texture
manager and the primitive manager; releases the context, the device, Direct3D and DirectDraw objects; clears
"environment ready".

## 5. Render-state slots

Every state slot below first **submits the primitive manager's queued geometry** (so state changes take effect
between draws in call order) and then sets one or more device render states. Enumerations are small integers
chosen by the interface; the tables map them.

### 5.1 Comparison, blend, stencil and shading enumerations

| Interface value | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| compare (table `0x00AC44EC`) | never | less | equal | less-equal | greater | not-equal | greater-equal | always | | | |
| blend factor (`0x00AC452C`) | zero | one | src colour | 1-src colour | src alpha | 1-src alpha | dst alpha | 1-dst alpha | dst colour | 1-dst colour | src alpha saturate |
| stencil op (`0x00AC450C`) | zero | keep | replace | increment-saturate | decrement-saturate | invert | | | | | |
| shade model (`0x00AC4524`) | flat | smooth (Gouraud) | | | | | | | | | |

(The tables hold the D3D7 enumeration values 1..8, 1..11, {2,1,3,4,5,6} and {1,2}; names are the public D3D7
meanings of those values.) Values outside a table read the following bytes *[quirk; callers must not pass them]*.

### 5.2 State slots

| Slot | Call | Effect |
|---|---|---|
| 16 | `DepthFunc(f)` | depth compare := compare[f] |
| 15 | `ColorMask(...)` | **only flushes; the mask is ignored** *[quirk]* |
| 17 | `DepthMask(b)` | depth writes on/off (the byte as given) |
| 18 | `StencilFunc(f, ref, mask)` | needs a stencil buffer (else error 3, nothing else): stencil read mask := mask, stencil compare := compare[f], stencil reference := ref |
| 19 | `StencilMask(m)` | needs stencil (else error 3): stencil **write** mask := m |
| 20 | `StencilOp(fail, zfail, pass)` | needs stencil (else error 3): the three stencil operations via the stencil-op table |
| 21 | `BlendFunc(src, dst)` | source and destination blend factors via the blend table |
| 22 | `AlphaFunc(f, ref)` | alpha compare := compare[f]; alpha reference := `q(ref)` (§5.5) |
| 23 | `ShadeModel(m)` | shade mode via the shade table |
| 67 | `PolygonOffset(v)` | the device depth-bias state := `v` unchanged (an integer bias) |
| 12 | `ClearColor(r, g, b, a)` | stores `TranslateColor(r,g,b,a)` (§5.5) as the clear colour |
| 13 | `ClearDepth(d)` | stores `d` (a double) rounded to single as the clear depth |
| 14 | `ClearStencil(s)` | stores `s` as the clear stencil value |

Failures of the stencil and polygon-offset calls set the error code per §3; the other slots ignore device failures.

### 5.3 `Clear(mask)` (slot 11, `0x00882C80`)

Flushes the queue, then clears the whole viewport. Mask bits: `0x4000` colour, `0x1000` depth, `0x2000` stencil
(stencil is silently dropped when the mode has no stencil buffer). Other bits are ignored. The values are the ones
stored by slots 12-14. Example: `Clear(0x7000)` on a mode without stencil clears colour and depth only.

### 5.4 Capabilities: `Enable` (44), `Disable` (45), `IsEnabled` (46)

| Cap | Meaning | `Enable` / `Disable` | `IsEnabled` |
|---|---|---|---|
| 0 | alpha test | flush; alpha-test state on / off | device state == on |
| 1 | depth test | flush; depth-buffer state on / off | device state == on |
| 2 | stencil test | flush; stencil state on / off | device state == on |
| 3 | face culling | flush; cull mode := clockwise / none | cull mode ≠ none |
| 4 | blending | flush; alpha-blend state on / off | device state == on |
| 5 | texturing on the **current texture stage** | no flush; sets the stage record's "enabled" byte to 1 / 0 and marks the record and the stage set dirty (applied at draw time) | the stage record's "enabled" byte |
| 6 | fog | only when the device reports the fog capability (bit 4 of a capability word): flush; fog on / off. Otherwise ignored | reads device state 0, *not* the fog state *[quirk: result meaningless]* |
| 7 | (accepted, ignored) | nothing | always true |

`IsEnabled` returns false for every cap while the device is lost. `Disable` of caps 0-4 sets the error code to 1
when the device call fails with anything but "surface lost", and to 0 otherwise; `Enable` never touches the
error code. Caps above 7 index past the table *[quirk; callers must not pass them]*.

### 5.5 Colour and alpha quantisation (`TranslateColorToD3D 0x00881AB0`)

A float channel `x` becomes a byte by `q(x)`: compute `s = x + 49152.0` in **single precision** (round to nearest
even), take `v = bits(s) - 0x47400000` (that is `round(x·256)`), then `q = (v - (v >> 8)) & 0xFF`. So 1.0 → 255,
0.5 → 128, 0.0 → 0, 0.998 → 255 (v = 255.488 rounded to 255; 255 - 0). Inputs outside [0, 1] wrap modulo 256.
`TranslateColor(r, g, b, a)` packs `q(a)<<24 | q(r)<<16 | q(g)<<8 | q(b)`.

## 6. Video modes (slots 63-65)

### 6.1 The mode list (`CountVideoModes` slot 63, `0x008885F0`; enumerator `0x008882F0`)

The list is built lazily: `CountVideoModes` returns the stored count when it is non-zero. When it is zero it loads
`DDRAW.DLL`, creates a temporary DirectDraw object through `DirectDrawCreateEx`, enumerates the display modes
(no filter: every mode the display reports, every bit depth, in the order reported), releases the object, unloads
the library and returns the new count. If the library or the entry point is missing, or creation fails, the count
stays 0 and is retried on the next call. `Init` and `Shutdown` reset the count to 0, so the list is rebuilt after
each initialisation.

Each reported display mode appends **two** consecutive records *[compat: indices are stable for the session]*:

| Index | Width, height, bits per pixel | Full-screen flag |
|---|---|---|
| 2k | as reported | 1 |
| 2k+1 | the same | 0 (windowed) |

Worked example: a display reporting 640×480×16 then 800×600×32 gives four modes: 0 = 640×480×16 full screen,
1 = 640×480×16 windowed, 2 = 800×600×32 full screen, 3 = 800×600×32 windowed; `CountVideoModes` returns 4.

### 6.2 The mode record

A record is 13 dwords (52 bytes) as copied out by the getters; the list stores them 56 bytes apart. Known fields:

| Offset | Field | Set by the enumerator |
|---|---|---|
| +0x00 | mode index | the record's own index |
| +0x04 | width (pixels) | display mode width |
| +0x08 | height (pixels) | display mode height |
| +0x0C | bits per pixel | display mode RGB bit count |
| +0x20 (byte) | full screen (1) or windowed (0) | per §6.1 |
| +0x21 (byte) | has stencil operations (read by §5.2, §5.3) | 0; later set from nonzero device stencil-operation capability word (§14.4) |
| +0x22 (byte) | unknown | 1 |
| +0x10..+0x1F, +0x23..+0x33 | unknown | 0 |

### 6.3 Getters

* `GetVideoModeInfo(index, out)` (slot 65, `0x00887680`): index ≥ count → error 2 and `out` untouched; else
  copies record `index` (13 dwords) to `out`. It does **not** build the list: call `CountVideoModes` first.
* `GetVideoModeInfo(out)` (slot 64, `0x00886E10`): copies the **current** mode record (13 dwords); its index is
  -1 when no mode is set.

Viewport slots 74/75 `SetViewport` and 76 `GetViewport` are §11.

## 7. The default device state (`SetDefaultD3DDeviceState`, `0x008872A0`)

Called at the end of `SetupViewport` (§4.3, after the primitive manager is initialised, `0x00888EF5`) and from
`Flush` (§4.4, `0x00884C59`) when the device has been restored. It issues a fixed sequence of Direct3D 7 device
calls, **in this order**, ignoring every result *[compat]* (the order matters only to a device that rejects some
states; keep it):

| # | Render state (D3D7 number) | Value |
|---|---|---|
| 1 | cull mode (22) | 1 = no culling |
| 2 | dither enable (26) | on |
| 3 | colour-key enable (41) | off |
| 4 | specular enable (29) | off |
| 5 | alpha-blend enable (27) | off |
| 6 | texture perspective (4) | on |
| 7 | depth-write enable (14) | on |
| 8 | depth function (23) | 4 = less-or-equal |
| 9 | alpha-test enable (15) | off |
| 10 | fog enable (28) | off |
| 11 | fill mode (8) | 3 = solid |
| 12 | stencil enable (52) | off |
| 13-15 | stencil fail (53) / depth-fail (54) / pass (55) | 1 = keep / **6 = invert** / 1 = keep |
| 16 | stencil function (56) | 8 = always |
| 17-19 | stencil reference (57) / read mask (58) / write mask (59) | 1 / 1 / 1 |
| 20 | texture factor (60) | `0xFFFFFFFF` (opaque white) |
| 21 | ambient light colour (139) | 0 |
| 22 | lighting (137) | off |
| 23 | ambient material source (147) | 1 = vertex colour 1 (diffuse) |
| 24 | diffuse material source (145) | 1 = vertex colour 1 |
| 25 | specular material source (146) | 0 = material |
| 26 | colour vertex (141) | on |

The stencil defaults (depth-fail inverts, masks 1) are the original's *[quirk]*; they are not what `StencilOp`
(§5.2) would produce from any "keep everything" request, so a renderer that relies on them must get them from here.

Then four driver flags are reset (object bytes `+0x2DC`..`+0x2DF`): "lighting forced on by a multiplier" := false
(`+0x2DC`, see §8.4), "clear colour at scene start" := false (`+0x2DD`), and the two vertex-colour flags of
`EnableVertexColors` (§8.4) := true (`+0x2DE` "ambient from vertex colour", `+0x2DF` "diffuse from vertex
colour"). These flags are reset **without** re-issuing the matching device states; rows 22-26 above are what
makes them true.

Then texture stage 0 is set (D3D7 texture-stage-state numbers):

| # | Stage state | Value |
|---|---|---|
| 27 | stage 0 colour op (1) | 2 = select argument 1 |
| 28 | stage 0 address mode (12) | 1 = wrap (both axes) |
| 29 | stage 0 texture-coordinate index (11) | 0 |
| 30-32 | stage 0 magnification (16) / minification (17) / mip filter (18) | 1 = point / 1 = point / 1 = no mip-mapping |
| 33-34 | stage 0 colour argument 1 (2) / argument 2 (3) | 2 = texture / 0 = diffuse |
| 35 | stage 0 alpha op (4) | 2 = select argument 1 |
| 36-37 | stage 0 alpha argument 1 (5) / argument 2 (6) | 2 = texture / 0 = diffuse |
| 38 | stage 1 colour op (1) | 1 = disable (this ends the cascade) |
| 39 | bound texture of stage 0 | none |

Finally the driver's own texture-stage cache is invalidated: the "stages dirty" byte (`+0x134`) := true and the
"dirty" byte (offset 0) of every per-stage record is set (the records are 0x80 bytes each, array at `+0x128`,
count at `+0x124`, one per hardware stage), so the next draw re-sends every stage (§10.3).

Consequence *[compat]*: after any device restore the device is in exactly this state, **not** in the state the
game last requested through the state slots of §5; nothing re-applies the game's depth, blend, alpha or stencil
choices. A replacement must reproduce that (a renderer change of state after a restore depends on it re-issuing
its own state each frame, which `cSC43DRender` does).

The word at object `+0x138` (set to 7 by the constructor `0x00886026` and by `Init`) has **no reader** in the
driver (`0x00881A00`-`0x0088B400` reads neither `+0x138` from the object nor `+0x12C` from the interface pointer,
which is the same field); it is write-only *[inferred: a leftover]*. A replacement may keep it as an unused field.
Likewise the "clear colour at scene start" flag `+0x2DD` is only ever written false (constructor and here), so the
colour clear `BeginScene` performs when it is set (§4.4, `Clear(0x4000)`) never happens *[inferred]*.

## 8. Drawing (slots 3-5, 30-34) and the primitive manager

### 8.1 Vertex arrays: `InterleavedArrays(format, stride, pointer)` (slot 5, `0x00881EC0`)

Stores the current vertex format, its standard stride (`StandardVFStride`, [graphics-formats.md §3.5](graphics-formats.md)),
the stride to use (the given stride, or the standard stride when the given one is 0) and the base pointer. It
sets an "arrays given" byte that nothing reads. No flush, no validation, no error code.

### 8.2 `DrawArrays(mode, first, count)` (slot 3, `0x008857E0`)

1. If the primitive manager has not been started (no device, §8.3), return without doing anything.
2. If the "stages dirty" byte (§7) is set: flush the queue (§8.3), then for every hardware stage whose record is
   marked dirty, clear the mark and send the stage's texture and sampler states (`0x00884320`); when that
   succeeded and the stage's environment mode differs from the one last sent, send the environment
   (`0x00884500`); clear "stages dirty". Record contents and both steps: §10.
3. If `count < 1`, return (after step 2).
4. The source is `base + stride × first`. Modes and what is queued:

| `mode` | Meaning | Queued as (D3D7 primitive) | Generated indices (per primitive, relative to `first`) |
|---|---|---|---|
| 0 | triangles | triangle list (4), unindexed | - |
| 1 | triangle strip | triangle list, indexed, 3·(count−2) indices | triangle k: (k, k+1, k+2) for even k, (k+1, k, k+2) for odd k |
| 2 | triangle fan | triangle list, indexed, 3·(count−2) indices | triangle k: (0, k+1, k+2) |
| 3 | points | point list (1), unindexed | - |
| 4 | lines | line list (2), unindexed | - |
| 5 | line strip | line strip (3), unindexed | - |
| 6 | quads | triangle list, indexed, 3·⌊count/2⌋ indices | quad q (base b = 4q): (b, b+1, b+2), (b, b+2, b+3) |
| 7 | quad strip | triangle list, indexed, 3·count−6 indices | for i = 0, 2, 4, … < count−2: (i, i+1, i+3), (i, i+3, i+2) |

The index count is truncated to 16 bits; strips and fans with `count < 3` (and quad strips with `count < 2`) give
a negative count truncated to a huge one *[quirk; callers must not do it]*. Indices are taken from the manager's
index scratch (`GetIndices`, §8.3). The queue result, when it is a failure, sets the error code by the §3 rule.
Modes 0-5 use the table at `0x00AC44CC` (`4 5 6 1 2 3 4 4`); entries 1, 2, 6, 7 are never read.

`DrawElements(mode, count, type, indices)` (slot 4, `0x00885A00`) does steps 1-3 the same way, then copies the
caller's `count` indices into the index scratch as 16-bit values (`type` 1: 8-bit source, 3: 16-bit, 5:
32-bit, truncated; any other type leaves the scratch unfilled *[quirk; callers must not]*), tracking their minimum
and maximum. Modes 0, 3, 4, 5 are queued indexed with the table primitive; modes 1, 2, 6, 7 are expanded to a
triangle list with the index counts and patterns of §8.2 applied to the caller's index list *[inferred: helpers
`0x008824F0`-`0x00882890` not traced one by one]*. The queue receives the base pointer, minimum index = the
minimum, vertex count = maximum − minimum + 1, so only the vertices in that range are copied. Failures set the
error code by §3.

### 8.3 The primitive manager (`nSGLDX7::cPrimitiveManager`, object `+0x1C`, `0x00889B60`-`0x0088A380`)

The manager batches draws into one 64 KiB Direct3D vertex buffer and submits them later.

**Init** (`0x0088A2B0`, from `SetupViewport` with the DirectDraw, Direct3D and device objects) asks the device
for its capabilities: the vertex buffer is created write-only when the device has hardware transform and lighting
(`D3DDEVCAPS_HWTRANSFORMANDLIGHT`), else write-only in system memory (flags `0x10000` / `0x10800`). It records one
vertex-buffer descriptor for standard vertex format 11 with that format's stride. The buffer itself is created when
the manager is started (`0x008898D0`, a virtual): capacity = ⌊65536 / stride⌋ vertices, flexible format built from
the vertex format: position → XYZ, a colour element → DIFFUSE, two-component texture coordinates: one set → TEX1,
two or more → TEX2 (`0x008891C0`; normals and other kinds are not carried) *[compat]*.

**Queue** (`QueueVersionA`, `0x00889E00`; arguments: vertex format, D3D primitive, stride, minimum index,
vertex count, source pointer, index count, index pointer):

1. Not started: return failure (`0x80004005`), nothing queued.
2. Stride above 32 bytes: the draw is **not** batched. It is sent at once from the caller's memory (plain or
   strided `DrawPrimitive` / `DrawIndexedPrimitive`, `0x00889450`), **without first flushing the queue**, so it
   can overtake earlier queued draws *[quirk]*.
3. Otherwise reserve `count` vertices in the buffer. When the queue is empty or the reservation does not fit
   behind the cursor, flush the queue (§ below), then reserve again allowing a wrap: the cursor restarts at 0 and
   the next lock discards the old contents. A request larger than the whole buffer, or a failed lock, drops the
   draw silently (result 0). Locks use wait + write-only + no-overwrite (discard after a wrap); a lock that
   reports a lost surface restores all surfaces, re-creates the buffer and retries.
4. Copy the vertices into the reserved space, converting from the source stride to the compact layout
   (`0x00889280`). Supported packed formats: position; position+colour; position+1 or 2 UV sets;
   position+colour+1 UV set; position+colour+2 UV sets (straight copy, only at stride 32). A stride that is not a
   multiple of 4 or any other format performs no copy but still reports copy success; it does not release the
   reservation on that account (`0x00889409`–`0x00889422`) [quirk].
5. Subtract the minimum index from each index. If the last queued entry has the same primitive and that
   primitive is a triangle, line or point **list**, merge: both unindexed → add the vertex count; both indexed →
   add vertex and index counts and rebase the new indices onto the merged entry's first vertex (this assumes the
   new indices follow the previous ones in memory, which holds for the index scratch). Otherwise append a new
   entry (primitive, first vertex in the buffer, vertex count, index pointer, index count).

**Flush** (`0x00889DE0`, called by every state slot before it changes state): submit every entry in order as
`DrawPrimitiveVB` (unindexed) or `DrawIndexedPrimitiveVB` from the buffer, clear the queue, reset the index
scratch. Not started with a non-empty queue: returns failure and **keeps** the queue.

**Exact compact-buffer writes and flush result (RD10).** The destination is always 32 bytes per vertex:
XYZ at bytes 0–11, diffuse colour at 12–15, UV0 at 16–23, UV1 at 24–31. Position-only writes XYZ and
`0xFFFFFFFF` diffuse, leaving both UV sets untouched. Position+colour copies bytes 0–15 and leaves both UV
sets untouched. Position+UV writes XYZ, `0xFFFFFFFF` diffuse and UV0, leaving UV1 untouched; the two-UV
variant also copies UV1. Position+colour+UV copies the supplied elements and leaves UV1 untouched;
position+colour+two-UV at stride 32 copies all 32 bytes. These are byte copies, with no float arithmetic or
colour conversion (`0x0088C680`–`0x0088C81D`). Flush submits all queued entries despite failures and returns
the HRESULT of the **last submission**, including a last success that hides earlier failures. An empty flush
returns zero. The index-scratch cursor is reset even when a nonempty, stopped manager returns `0x80004005`
without clearing its queue (`0x00889BF0`–`0x00889C88`, `0x00889DE0`).

For nonindexed quads the declared index count remains `3*floor(count/2)`, truncated to 16 bits, but the
generator writes a whole six-index quad for every base `b=0,4,8,... < count`, including an incomplete last
quad. For positive ordinary counts it therefore writes `6*ceil(count/4)` indices, with pattern
`b,b+1,b+2,b,b+2,b+3`. For count 6 it declares 9 but writes
`0,1,2,0,2,3,4,5,6,4,6,7`; the submitted prefix has 9 elements and refers to vertex 6 outside the six
copied vertices. For count 5 it declares 6 but writes those same 12 elements. These writes may exceed the
logical scratch reservation; they are not permission to invent readable vertices or stable memory-corruption
results. Quad strips likewise emit whole groups of six while `b < count-2`, stepping by two, even for odd
counts (`0x0088B460`–`0x0088B500`). Indexed quads declare the same count, but their generator emits
`ceil((3*floor(count/2))/6)` groups, reading four caller indices per group. Thus count 5 emits one complete
group; count 6 emits two and can read beyond the caller's logical list. The 16-bit source helper is
`0x0088B6C0`–`0x0088B7AA`; the public count truncation and generator count are distinct.

**Index scratch** (`GetIndices(n)`, `0x0088A240`): a growable array of 16-bit indices handed out sequentially;
when `n` more would exceed its allocated capacity, the queue is flushed first (queued entries point into it),
then the array grows. **Stop** (`0x00889B60`) clears the queue and releases the buffer.

### 8.4 Colour and alpha multipliers, vertex colours, fog (slots 30-34)

All flush the queue first.

- `ColorMultiplier(r, g, b)` (32, `0x00883240`): on first use since the last default-state reset (§7 flag
  `+0x2DC`) turn lighting on (error code by §3 on failure); set the material's ambient colour to (1, 1, 1) and
  send the material; ambient light := `TranslateColor(r, g, b, 1.0)` (§5.5). The multiplier is thus applied by
  fixed-function lighting with ambient taken from the vertex colour (§7 row 23).
- `AlphaMultiplier(a)` (33, `0x00883310`): same lighting step; material diffuse alpha := `a`, send the material;
  then if `a < 1.0` (compared as a double with `0x00A80AB0` = 1.0): diffuse source := material (so alpha comes
  from the material); else (also NaN), only when the "diffuse from vertex colour" flag is set: diffuse source :=
  vertex colour 1.
- `EnableVertexColors(ambient, diffuse)` (34, `0x008833E0`): stores both flags (§7), colour-vertex on, ambient
  source := vertex colour 1 if `ambient` else material, diffuse source likewise from `diffuse`.
- `Fog(pname, const float* v)` (30, `0x00883180`): 0 → error code 1; 1 → fog colour := `TranslateColor(v[0..3])`;
  2 → fog density := `v[0]`; 3 → fog start := `v[0]`; 4 → fog end := `v[0]`; others ignored. A failing device
  call sets the error code by §3.
- `Fog(pname, int v)` (31, `0x00883100`): without the device fog capability (the bit tested by cap 6, §5.4):
  error code 3. Otherwise 0 → vertex fog mode := table `0x00AC45F8`[v] (0 → exponential, 1 → exponential
  squared, 2 → linear, 3 → exponential squared); 1-4 → error code 1; others ignored.

### 8.5 Vertex-format slots (6-10)

All five are stateless forwards to the shared `nRZSimGL` vertex-format helpers of
[graphics-formats.md §3.5](graphics-formats.md) (packed layout, standard numbers, strides); they touch no driver
state and never set the error code.

| Slot | Name | Forwards to | Behaviour |
|---|---|---|---|
| 6 (`0x008DA980`) | `MakeVertexFormat(standard)` | `PackStandardVertexFormat` `0x008D01F0` | standard number → packed format (0 for an unknown number) |
| 7 (`0x00881F30`) | `MakeVertexFormat(count, kinds[])` | `MakeStandardVertexFormat` `0x008D02C0` | builds a packed format from a list of element kinds (below) |
| 8 (`0x008DA990`) | `VertexFormatStride(format)` | `StandardVFStride` `0x008D0370` | stride in bytes |
| 9 (`0x0087C4C0`) | `VertexFormatElementOffset(format, kind, index)` | `StandardVFElementOffset` `0x008D04A0` | byte offset of an element (with the shifted-mask quirk of graphics-formats.md §3.5) |
| 10 (`0x00881F50`) | `VertexFormatNumElements(format, kind)` | `0x008D04F0` | count of elements of `kind` |

`MakeVertexFormat(count, kinds[])`: counts how often each kind 0-9 occurs in the first `count` entries of
`kinds` (count 0 gives all zeros), then packs the ten counts into the fields of graphics-formats.md §3.5
(kind 0 at bit 0, 1 at bit 2, 2 at bit 5, 3 at bit 6, 4 at bit 7, 5 at bit 8, 6 at bit 10, 7 at bit 14, 8 at
bit 18, 9 at bit 22) and sets bit 31. The counts are shifted and OR-ed **without masking**: a count larger than
its field (for example three positions, field width 2) spills into the next field *[quirk]*. Kinds outside 0-9
are not checked (they corrupt the routine's locals); callers must pass 0-9.
Example: kinds (0, 5, 7) → `0x80004101` (standard format 10, position, colour, uv).

`VertexFormatNumElements(format, kind)`: a format below `0x80000000` (a standard number) is packed first; the
result is `(format >> shift[kind]) & mask[kind]` with the arithmetic shifts 0, 2, 5, 6, 7, 8, 10, 14, 18, 22 and
masks 3, 7, 1, 1, 1, 3, 15, 15, 15, 15 (tables `0x00ACC0D4`, `0x00ACC0FC`, read from the exe). `kind` is not
range-checked. Example: (`0x80008101`, 7) → 2.

### 8.6 Matrices (slots 40-42)

The driver keeps a **matrix mode** word (object `+0x214`; 0 = model-view, 1 = projection) and a copy of the last
projection as given by the caller (16 floats at `+0x150`). Matrices are 16 floats, row-major with the row-vector
convention of Direct3D (element `m[r*4+c]`, translation in row 3).

- `MatrixMode(mode)` (40, `0x00881EA0`): stores `mode`; nothing else (no flush, no check).
- `LoadMatrix(m)` (41, `0x00882AC0`): first flushes the primitive manager's queue (§8.3; its result is not
  checked here), then by mode:
  - **0 (model-view)**: device world transform := `m`, then device view transform := the fixed matrix
    diag(1, 1, −1, 1) (`0x00AC4618`, read from the exe).
  - **1 (projection)**: copies `m` to `+0x150`, reads the device viewport (width `W`, height `H` as unsigned
    integers) and sends as the device projection the matrix `P` built from `m` as follows, with
    `a = −0.5 / (W × 0.5)` and `b = 0.5 / (H × 0.5)` (constants 0.5 `0x00A84D2C`, −0.5 `0x00A949FC`): for every
    row r, `P[r][0] = m[r][0] + m[r][3] × a`, `P[r][1] = m[r][1] + m[r][3] × b`,
    `P[r][2] = (m[r][2] + m[r][3]) × 0.5`, `P[r][3] = m[r][3]`; then row 2 of `P` is negated (all four entries).
    The arithmetic is x87 extended precision with every `P` entry rounded to a single float when stored, rows
    in order 0-3.
  - **any other mode**: nothing after the flush.
  The three adjustments are the usual OpenGL-to-Direct3D conversion: a shift of half a pixel left and up
  (`a`, `b`: Direct3D 7 samples pixel centres at integer coordinates), depth range [−1, 1] → [0, 1], and the
  handedness flip. The view matrix's z flip of mode 0 and the negated row 2 of the projection cancel, so a
  caller's model-view × projection pair maps vertices to the same clip coordinates as in OpenGL apart from the
  depth remap and the half-pixel shift.
  Worked example: the initial pixel-space projection of §4.3 for an 800×600 viewport (row 3 = (−1, +1, 0, 1),
  `m[r][3]` = 0 for rows 0-2) gives `a` = −1/800 and `b` = 1/600, so row 3 of `P` becomes
  (−1.00125, 1.0016667, 0.5, 1) and row 2 becomes (0, 0, +0.5, 0): the caller's pixel corner (0, 0) lands at
  Direct3D screen position (−0.5, −0.5), the outer corner of the pixel whose centre Direct3D 7 puts at (0, 0).
- `LoadIdentity()` (42, `0x00881EB0`): `LoadMatrix` (through the interface, slot 41) with the static identity
  matrix at `0x00B542F8`, whose diagonal the constructor sets to 1.0 (§2); it therefore applies to the current
  mode with the same side effects.

`SetViewport` (slots 74-75, §11) re-sends the stored projection through `LoadMatrix` with the mode forced to 1, so
the half-pixel shift follows the new viewport size.

**Floating-point environment (RD11).** `LoadMatrix` uses the caller's current x87 control word; it neither
loads a control word nor clears/restores exception status (`0x00882AC0`–`0x00882C7C`). For portable finite
conformance use round-to-nearest/even, 64-significand-bit extended intermediates and masked exceptions:
form the width/height quotients, round each x87 arithmetic operation at that precision, and round to binary32
at each stored matrix element. Binary64 intermediates are insufficient. With viewport width 30 and input
matrix words m0=`3E2F59FF`, m3=`40A4645F`, the corrected m0 is `30888889` (binary64 gives `30888888`).
The original does not validate finite inputs or nonzero viewport sizes: zeros, infinities, NaNs and unmasked
exceptions follow the ambient x87 environment, including traps and status flags. There is no driver-defined
NaN canonicalization or payload policy. Finite bit-exact vectors under the stated environment are required;
portable core need not emulate host FPU exception flags, traps or NaN payloads unless exposing an explicit
x87-environment compatibility mode. Ordinary finite examples alone must not excuse the discriminator above.

## 9. Textures (`nSGLDX7::cTextureManager`, object `+0x18`, `0x0088A380`-`0x0088B2C0`; slots 24-26, 35-39, 58-61)

Textures are named by small integers ("names") that index a table of 48-byte records. Name 0 is a built-in 1×1
opaque white texture (§9.2) and is never freed.

### 9.1 The texture manager object

| Offset | Content |
|---|---|
| `+0x8` | the DirectDraw object (used to create surfaces) |
| `+0xC` | the device; null while the device is lost (surfaces are then not created, §9.3) |
| `+0x10` | internal-format table: 8 entries of 0x28 bytes, one per internal format 0-7 (pixel-format masks, flags, FourCC, bit count, and a converter key at entry `+0x24`), filled by `Init_PopulateInternalFormatTable` (`0x0088F8D0`) from the device's texture formats (§9.6; full ranking remains RD6) |
| `+0x150` | converter table: 15 source formats × 8 internal formats of copy routines (`0x0088F890` per source format); a null entry means "this source cannot be loaded into this internal format" |
| `+0x330`..`+0x338` | the record array (begin, end, capacity) |
| `+0x33C`..`+0x344` | the list of freed names (begin, end, capacity) |

The texture record (48 bytes):

| Offset | Field | Default (new name) |
|---|---|---|
| `+0x00` | in use (byte) | 1 |
| `+0x04` | width | 0 |
| `+0x08` | height | 0 |
| `+0x0C` | mip level count | 0 |
| `+0x10` | priority (float) | 0.5 |
| `+0x14` | internal format 0-7 | 4 |
| `+0x18` | creation flags (bit 0 dynamic, bit 1 opaque) | 0 |
| `+0x1C`..`+0x28` | the four sampler parameters of §10.2: magnification, minification, wrap S, wrap T | 0, 0, 3, 3 |
| `+0x2C` | the DirectDraw texture surface, or none | none |

A name is **valid** when it is below the record count and its "in use" byte is set. Every manager operation on an
invalid name does nothing and reports failure.

**Name allocation** (`NewTexture 0x0088B160`): if the freed-name list is non-empty, its **last** entry is
reused (LIFO); otherwise a new record is appended with the defaults above. Either way the record is then marked in
use and its width and height are set to 0 (a reused record keeps its other fields: priority, format, flags,
parameters and mip count from its previous life *[quirk]*). **Freeing** (`FreeTexture 0x0088ADF0`) releases the
surface, clears "in use" and appends the name to the freed list.

`HasAlpha(name)` (`0x0088A4F0`) is true when the internal format is neither 0 nor 1. "Has surface"
(`0x0088A490`) is true when the record has a surface. `SetPriority(name, p)` (`0x0088A5A0`) stores `p`; when `p`
equals 0.95f exactly (`0x00A9BEF8`, single-precision compare) it also sets creation flag bit 0 (dynamic), which
takes effect the next time the surface is created.

### 9.2 Initialisation, device loss and shutdown

`Init(ddraw, device, caps)` (`0x0088B2C0`) runs only when both objects are given and the manager has no DirectDraw
object yet: it keeps both, fills the internal-format table from `caps` and the converter table, then creates
name 0: `NewTexture`, `CreateTexture(0, flags 0, format 4, 1×1, 1 level)` and loads one 32-bit pixel
`0xFFFFFFFF` from source format 0 (`0x0088B260`).

When the device is lost (§4.4), `0x0088A400` releases every record's surface (records keep their sizes and
parameters) and drops the device. `InitDevice(device)` (`0x0088AC40`) adopts the new device and re-creates the
surface of every in-use record whose width is non-zero, with its stored flags, format, size and level count.
The pixel contents are **not** restored: clients re-upload on `kSGLMessage_RestoreTextures` (§4.4).
`Shutdown` (`0x0088B0D0`) frees name 0 only (checked in the instructions) *[inferred: other names' surfaces are
left to the record-range erase `0x0088AED0`]*, empties the record array and the freed list, and releases the device and DirectDraw
objects.

### 9.3 Creating a surface (`CreateTexture`, `0x0088A900`; device call `0x0088A720`)

`CreateTexture(name, flags, format, width, height, levels)` fails unless the name is valid and
1 ≤ width ≤ 4096 and 1 ≤ height ≤ 4096. It stores width, height, levels, flags and format in the record. Without
a device it releases any old surface and reports **success** (the surface is created later by `InitDevice`).
With a device it releases the old surface and creates a new one: surface description size 0x7C, flags
CAPS | HEIGHT | WIDTH | PIXELFORMAT | MIPMAPCOUNT (`0x21007`), mip count = max(levels, 1), caps
TEXTURE | MIPMAP | COMPLEX (`0x401008`), caps2 = D3D-managed texture (`0x20000`), plus "hint dynamic"
(`0x20004`) when flag bit 0 is set, else "opaque" (`0x20080`) when flag bit 1 is set; the pixel format is the
internal-format entry's (alpha mask, red, green, blue masks, flags, FourCC, bit count). A failed creation leaves
no surface and reports failure.

### 9.4 Source formats and loading pixels (`LoadTextureLevel`, `0x0088ACD0`)

The caller describes its pixels by a (format, type) pair, translated to one of 15 source formats
(`0x0088D0B0`); bits per pixel from `0x0088D050`:

| type | format | Source format | Bits | *[inferred]* layout |
|---|---|---|---|---|
| 0 or 1 | 3 | 0 | 32 | 8-8-8-8 |
| 0 or 1 | 1 | 1 | 32 | 8-8-8-8, other channel order |
| 0 or 1 | 2 | 2 | 24 | 8-8-8 |
| 0 or 1 | 0 | 3 | 24 | 8-8-8, other order |
| 13 | 3 / 1 | 4 / 5 | 16 | 1-5-5-5 |
| 10 | 3 / 1 | 6 / 7 | 16 | 4-4-4-4 |
| 14 | 2 / 0 | 8 / 9 | 16 | 5-6-5 |
| 0 or 1 | 7 or 8 | 10 | 4 | DXT1 block-compressed |
| 0 or 1 | 9 | 12 | 8 | DXT3 |
| 0 or 1 | 10 | 14 | 8 | DXT5 |

Source formats 11 and 13 have 0 bits and are never produced. Any other pair fails.

`LoadTextureLevel(name, level, x, y, w, h, format, type, rowLength, pixels)`:
1. Fails unless the name has a surface and (`level` = 0 or `level` < the record's mip count).
2. Fails if the (format, type) pair is unknown or the converter table has no routine for (source format, the
   record's internal format).
3. Source pitch in bytes = ⌊bits × rowLength / 8⌋, where `rowLength` = `w` when given as 0.
4. Locks mip level `level` (walking `level` times to the attached mip surface) with flags WAIT | WRITEONLY |
   NOSYSLOCK | DISCARDCONTENTS (`0x2821`); while the lock fails with "surface lost" (`0x887601C2`) and the
   surface restores successfully, it retries. Any other failure fails the load.
5. Uncompressed surfaces: the routine converts `w × h` pixels to the surface at byte offset
   `y × pitch + x × (surface bits / 8)`. Compressed surfaces (the surface reports a linear size) accept only
   `x = y = 0` and copy the whole level; any other offset copies nothing.
6. Unlocks the surface and reports success (also when step 5 copied nothing *[quirk]*).

### 9.5 Driver slots

Below, "current stage" is the stage selected by `TexStage` (§10.1) and "the stage record" its 0x80-byte record.

- `GenTextures(n, out)` (35, `0x00883450`): `n` new names (§9.1) written to `out[0..n-1]`.
- `DeleteTextures(n, names)` (36, `0x00883480`): only when the "environment ready" byte `+0x263` is set
  (set to 1 at the end of `SetupViewport` `0x00888F26`, cleared at the end of `DestroyViewport` `0x0088829F`,
  0 at construction); otherwise it does nothing. Frees every non-zero name. Name 0 is skipped.
- `IsTexture(name)` (37, `0x008834C0`): true when the name is below the record count and in use (no surface
  needed).
- `PrioritizeTextures(n, names, priorities)` (38, `0x008834D0`): `SetPriority` per name (§9.1).
- `AreTexturesResident(n, names, out)` (39, `0x00883510`): `out[i]` := `IsTexture(names[i])`; returns true.
- `BindTexture(target, name)` (24, `0x00884750`): `target` ≠ 0 → error code 1, nothing else. Otherwise sets the
  "parameters follow the texture" flag (`+0x13C`, §10.2), loads the stage record's four sampler parameters from
  the texture (from name 0 when `name` is invalid), stores `name` (even if invalid), the texture's "has alpha",
  resets the stage's applied-environment cache to -1, and marks the record and the stage set dirty. No flush.
- `SetTexture(name, stage)` (58, `0x00884810`): if `name` differs from that stage's name: clears the "parameters
  follow the texture" flag, stores `name` and "has alpha", resets the environment cache, marks dirty. The
  stage's sampler parameters are **not** reloaded. No range check on `stage`.
- `GetTexture(stage)` (59, `0x00884860`): that stage's name.
- `PixelStore(pname, v)` (26, `0x00881FC0`): pname 0 → unpack row length (`+0x140`) := `v`; others ignored.
  `GetInteger(pname, out)` (48, `0x00881FE0`) writes the row length when the dword at `0x00AC45F4 + 4·pname`
  is 0, i.e. for pname 0 (and, by overlap with the fog table, pnames 7 and 8 *[quirk]*); otherwise `out` is
  left unchanged.
- `TexImage2D(target, level, format, width, height, border, srcFormat, type, pixels)` (25, `0x00884880`): the
  name is the current stage's. `width < 1`, `height < 1` or name 0 → error code 2. Row length := `+0x140`, or
  `width` when 0. If the name has no surface, `CreateTexture(name, record flags, format, width, height, 1 level)`
  (failure → error code 2). Then `LoadTextureLevel(name, level, 0, 0, width, height, srcFormat, type, row
  length, pixels)`; failure → error code 3. `target` and `border` are ignored. An existing surface is **not**
  re-created when the size or format differs *[quirk]*. No flush.
- `CreateTexture(format, width, height, levels, flags)` (60, `0x00882FF0`): allocates a name, creates its
  surface (§9.3), returns the name; on failure sets error code 3 but still returns the (allocated) name.
- `LoadTextureLevel(name, level, x, y, w, h, srcFormat, type, rowLength, pixels)` (61, `0x00883040`): flushes the
  primitive queue (§8.3), then §9.4; failure → error code 3.

### 9.6 Format selection and converter contracts

An internal format is a **request**, not a promise that the device surface uses that exact packing. Initialization
chooses a device-format descriptor for each of the eight requests, and chooses the source converter through that
descriptor's converter key. Surface creation and uploads must use the same choice. A replacement must not infer
the surface bit count from the source upload format. Unsupported source/destination pairs fail the upload (§9.4),
without invalidating the texture name.

Compressed requests 5, 6 and 7 first seek descriptor converter keys 9, 11 and 13 respectively. In the absence of
the requested key, they enter the ordinary-format selection with requests 3, 2 and 2 respectively
(`0x0088F8D0`–`0x0088F979`). Thus absence of a compressed format does not itself prove texture creation must fail.
#### Ordinary descriptor ranking

For requests 0–4 the target pair (B,A) is respectively (16,0), (24,0), (16,4), (16,1), (32,8).
B means the descriptor's stored bit count at +0x1C, **including alpha**, not the sum of RGB widths;
A is its alpha-bit count at +0x20. Requests 5–7 use the fallback pairs already given, only when no matching
compressed key exists. The first descriptor with that compressed key wins, without ordinary ranking.
The target lookup tables are instruction-checked at `0x0088BCD0` / `0x0088BD20` and their jump tables
`0x0088BCF8` / `0x0088BD48`.

Ordinary selection scans the enumerated 40-byte descriptors in order. Classify each by the **highest**
matching row in this table (higher rank wins); choose the highest rank, then the stated within-rank metric,
then the earliest enumerated descriptor. This describes the native accumulating candidate-list/state scan:
no secondary RGB preference is introduced after an alpha rank has won.

| Rank | Condition on descriptor (b,a) versus requested (B,A) | Within-rank preference |
|---|---|---|
| 7 | b=B and a=A | first |
| 6 | A>0 and a=A | first; b is not compared except promotion to rank 7 |
| 5 | A>0 and a>A | smallest a; b is not compared |
| 4 | A>0 and 0<a<A | largest a; b is not compared |
| 3 | b=B | first |
| 2 | b>B | smallest b |
| 1 | b<B | largest b |

Thus any excess bit count beats any shortage in the colour-only ranks. Any positive alpha below the target
beats a no-alpha exact-bit-count descriptor when A>0; excess alpha beats insufficient alpha. For A=0 the
alpha ranks 4–6 are disabled; exact b with a=0 still promotes to rank 7. Selection is not a weighted distance
or a test of converter availability. All fields are compared as unsigned dwords, except the positive-target
test; supported target values are the small nonnegative values above. Empty enumeration produces an all-zero
40-byte descriptor for every request. The zero descriptor's key is consequently 0; this does not establish
that DirectDraw can allocate a surface from its otherwise empty pixel format.
Evidence: rank dispatch and colour comparisons `0x0088F996`–`0x0088FB2E`, alpha comparisons
`0x0088FB2E`–`0x0088FC8A`, exact-pair rank `0x0088FC8A`–`0x0088FCF1`, zero/first-candidate copy
`0x0088FD00`–`0x0088FD79`.

#### Complete availability contract for the 15×8 table

Each of the eight **request** columns stores the converter selected by its chosen descriptor's key at
+0x24; the key is not the request number. For each source s and request i, the cell is available exactly
when the key of descriptor i belongs to the following row. This completely determines all 120 cells for
any enumeration result, including compressed fallback. There is no fixed device-independent 15×8 Boolean
matrix indexed directly by request numbers. An absent cell rejects the upload before locking (§9.4).

| Source | Available descriptor keys (all other keys give null) |
|---|---|
| 0 | 0, 1, 2, 3, 4, 5, 7 |
| 1 | 0, 1, 2, 3, 4, 5, 7 |
| 2 | 0, 1, 2, 3, 4, 5, 7 |
| 3 | 0, 1, 2, 3, 4, 5, 7 |
| 4 | 0, 1, 2, 3, 4, 5, 7 |
| 5 | 0, 1, 2, 3, 4, 5, 7 |
| 6 | 0, 1, 2, 3, 4, 5, 7 |
| 7 | 0, 1, 2, 3, 4, 5, 7 |
| 8 | 0, 1, 2, 3, 4, 5, 7 |
| 9 | 0, 1, 2, 3, 4, 5, 7 |
| 10 (DXT1) | 1, 2, 4, 5, 7, 9 |
| 11 (unproduced) | 10, 11, 12, 13 |
| 12 (DXT3) | 1, 2, 4, 5, 7, 11 |
| 13 (unproduced) | 10, 11, 12, 13 |
| 14 (DXT5) | 10, 11, 12, 13 |

Keys 1/2 share a routine in every row; likewise keys 4/5. In particular DXT5 has **no software decoder to
an ordinary key** here. Falling back request 7 to an uncompressed descriptor can make creation succeed
but a DXT5 upload fail. DXT1 supports direct-copy key 9, DXT3 direct-copy key 11. Neither accepts the other's
key as a direct copy. Uncompressed sources have no encoder to compressed keys. Source rows 11 and 13 are
initialized but unreachable through §9.4's format/type translator. Evidence: `0x0088F770`–`0x0088F844`,
`0x0088F310`–`0x0088F737` and their byte/dword switch tables; compressed selectors
`0x0088D210`–`0x0088D2A2`; per-request installation `0x0088F890`–`0x0088F8C8`.

#### Uncompressed pixel arithmetic (RD6)

All available ordinary cells in the matrix above obey the following channel rule. Packed words/dwords
are little-endian; the notation below lists fields from most to least significant bit, except explicit
byte lists. Source numbers are converter rows, not public request numbers.

| Source | Input fields |
|---|---|
| 0 / 1 | A8,R8,G8,B8 / A8,B8,G8,R8 |
| 2 / 3 | Increasing-address bytes B8,G8,R8 / R8,G8,B8 |
| 4 / 5 | A4,R4,G4,B4 / A4,B4,G4,R4 |
| 6 / 7 | A1,R5,G5,B5 / A1,B5,G5,R5 |
| 8 / 9 | R5,G6,B5 / B5,G6,R5 |

| Destination key | Output fields |
|---|---|
| 0 | Increasing-address bytes B8,G8,R8; discard alpha |
| 1 or 2 | A8,R8,G8,B8 |
| 3 | R5,G6,B5; discard alpha |
| 4 or 5 | A1,R5,G5,B5 |
| 7 | A4,R4,G4,B4 |

For each RGB channel independently, narrowing keeps its most significant bits (no rounding,
dither or saturation). Equal widths preserve the value. Widening repeats the source's high bits:
4→5 is `2v+floor(v/8)`, 4→6 is `4v+floor(v/4)`, 4→8 is `17v`, 5→6 is
`2v+floor(v/16)`, 5→8 is `8v+floor(v/4)`, and 6→8 is `4v+floor(v/16)`.
Alpha uses the same significant-bit narrowing; 1→4/8 becomes 0 or all ones, 4→8 becomes 17v.
An absent source alpha supplies all ones in the output alpha field. There is no multiplication by
255 followed by division for these uncompressed texture cells. Thus source-4 `1234` to key 4 is
`10C8` (A1=0,R5=4,G5=6,B5=8), whereas source-6 `8000` to key 7 is `F000`.
The source-0/key-1 copy and source-6/key-4, source-8/key-3, source-4/key-7 and source-2/key-0
equal-layout copies follow this same rule; source padding is not pixel data.

Width is a pixel count; both ordinary strides are byte distances. Non-copy rows visit logical
rows from last to first and pixels left to right, preserving row orientation and destination padding.
The source-2/3 converters that quantize through a dword load may read a fourth byte past the
three-byte pixel, but mask it away; those bytes must be readable and do not affect the result.
Negative dimensions/overflow and overlapping buffers have no general safe conversion promise.
Instruction evidence by family: table-based quantization `0088CA17`–`0088CA57`,
`0088CAD7`–`0088CB17`, `0088CC40`–`0088CC7D`, `0088CE69`–`0088CF56` (tables
`00B12DE0`–`00B1326C` contain shifted channel values); byte reordering `0088CB40`–`0088CBBE`,
`0088CCB0`–`0088CD37`, `0088CDC2`–`0088CDE0`; scalar packing/expansion
`0088D390`–`0088D3C5`, `0088D460`–`0088DA80`, `0088DB60`–`0088EA9D`,
`0088EB40`–`0088F025`, `0088F250`–`0088F30C`. These are integer loads/shifts/masks/additions,
not float conversion. Examples below retain the two previously verified expansions.

#### Verified copy and bit-expansion examples

Converter arguments are destination/source pointers, destination/source byte strides, width and height.
Ordinary conversions below process width pixels per row without copying padding. Their row addresses
refer to the same logical row on each side; bit-expansion routines visit rows from last to first, then
pixels left to right (`0x0088F040`–`0x0088F126`, `0x0088F130`–`0x0088F205`). They do not flip the image.

| Source/key | Pixel contract |
|---|---|
| 0 → 1 or 2 | Copy each 32-bit pixel unchanged, including alpha; width×4 bytes per row, independent strides (`0x0088C990` → `0x0088C910`). |
| 5 → 1 or 2 | For the 16-bit source's nibbles from high to low, A,B,G,R, expand each nibble v to 17v. Output dword bytes from high to low are A,R,G,B. Example `1234` → `11443322`. Integer replication, no floating-point scaling (`0x0088F1A0`–`0x0088F1DD`). |
| 9 → 1 or 2 | Source bits high to low B5,G6,R5. Expand 5-bit v as (v shifted left 3) plus (v shifted right 2); expand 6-bit v as (v shifted left 2) plus (v shifted right 4). Output opaque A,R,G,B. Example `0821` → `FF080408`; no multiplication by 255/31 followed by rounding (`0x0088F0C0`–`0x0088F102`). |

For the compressed direct-copy cells, the routine is a **byte-row copier**, not a DXT codec: copy width
bytes each row, then advance by the two byte strides (`0x0088C830`–`0x0088C86D`). On a locked surface
reporting DDSD_LINEARSIZE (`0x80000`), CopyToSurface ignores the upload width, height and source row length:
with x=y=0 it invokes the selected converter with both strides and width equal to the reported linear byte
size and height 1 (`0x0088A395`–`0x0088A3B6`). Consequently the direct-copy cells copy exactly that many
bytes, with no 4×4 rounding, endpoint inspection, alpha processing or partial-block clipping. Caller storage
must cover that size. Nonzero x/y returns false from the copy helper; LoadTextureLevel ignores this return,
unlocks and still succeeds. The ordinary-surface path passes the pixel dimensions/strides instead.

#### DXT1/DXT3 software decoding to ordinary keys (RD6)

The six decoder selections are DXT1→keys 1/2, 4/5, 7 at `0080A6A0`, `0088CF90`,
`0088CFC0`, and DXT3→the same keys at `0080A6D0`, `0088CFF0`, `0088D020`.
These wrappers use signed width/4 and height/4 rounded down for nonnegative sizes; both
source-block row length and blocks-to-decode equal width/4. The supplied source byte stride
is ignored. Process those whole 4×4 blocks in increasing block-row/block-column order,
reading tightly packed 8-byte DXT1 or 16-byte DXT3 blocks. Destination pitch remains the
supplied byte pitch. A partial right column or bottom row of pixels is left untouched, not
decoded from a padded block; width<4 or height<4 decodes nothing. This is distinct from the
raw compressed copy path. Evidence: wrapper shifts/arguments `0080A6AC`–`0080A6C0`,
`0088CF9C`–`0088CFB0`, `0088CFFC`–`0088D010`; block/row advances
`008D11FD`–`008D122D`, `008D20F2`–`008D212C` and `008D15E0`–`008D1617`.

Read the two colour endpoints as unsigned RGB565 words c0,c1 (DXT3 has eight alpha bytes
before them). Form palette colours P0,P1 in the **destination's RGB channel widths**:
keys 1/2 expand 5/6→8 by replication, keys 4/5 retain R5/B5 and truncate G6→5,
key 7 truncates R5/B5→4 and G6→4. Interpolation happens after that conversion.
For each destination RGB channel independently:

| Endpoint order | P2 | P3 |
|---|---|---|
| c0>c1, keys 1/2 | floor((171P0+85P1+128)/256) | floor((85P0+171P1+128)/256) |
| c0>c1, keys 4/5 | floor((21P0+11P1+16)/32) | floor((11P0+21P1+16)/32) |
| c0>c1, key 7 | floor((11P0+5P1+8)/16) | floor((5P0+11P1+8)/16) |
| c0≤c1, every key | ceil((P0+P1)/2) | zero RGB |

These are integer weighted sums, not exact division by three. DXT1 supplies full alpha for
P0/P1/P2 and, when c0>c1, P3; otherwise P3 is the all-zero transparent pixel. DXT3 uses
the **same endpoint-order branch**, including zero RGB for code 3 when c0≤c1; it does not
force four-colour mode. Its alpha comes independently from the block's sixteen nibbles:
17v for keys 1/2, the high bit of v for keys 4/5, and v for key 7. It remains present even
for code 3 with zero RGB. No premultiplication occurs. A block's two-bit colour selectors
and DXT3 alpha nibbles run least-significant first, left-to-right, then top-to-bottom.

For c0=FFFF,c1=0000, code 2 produces `FFAAAAAA`, `D294`, `FAAA` for DXT1 to
keys 1,4,7. For c0=0000,c1=FFFF, code 2 produces `FF808080`, `C210`, `F888`,
and code 3 is zero. With DXT3 alpha nibble F the latter code 3 instead gives `FF000000`,
`8000`, `F000`. Evidence: endpoint/order and weights `008D1090`–`008D11E1`,
`008D1FD6`–`008D20DB`, `008D2196`–`008D22B7`; DXT3 endpoint/order and weights
`008D12D0`–`008D13F4`, `008D16C0`–`008D1800`, `008D1AA0`–`008D1C40`;
alpha placement `008D13F4`–`008D15D0`, `008D1800`–`008D19D0`, `008D1C40`–`008D1DFF`.

## 10. Texture stages (slots 27-29, 51-57, 62; records applied at draw time)

### 10.1 The stage record

The driver keeps one 0x80-byte record per hardware stage (array `+0x128`, count `+0x124`), a current stage index
(`+0x120`) and the "stages dirty" byte (`+0x134`, §7). `TexStage(i)` (51, `0x00882020`): `i` ≥ count → error
code 2, else current stage := `i`.

| Offset | Field | Written by |
|---|---|---|
| `+0x00` | dirty | every setter below, §7 |
| `+0x01` | texturing enabled | `Enable`/`Disable` cap 5 (§5.4) |
| `+0x02` | bound texture has alpha | `BindTexture`, `SetTexture` |
| `+0x04` | texture name | `BindTexture`, `SetTexture` |
| `+0x0C`..`+0x18` | magnification, minification, wrap S, wrap T | `BindTexture`, `TexParameter` |
| `+0x1C` | environment mode | `TexEnv` (28), `SetCombiner` (:= 4) |
| `+0x20` | environment mode last sent (-1 = none) | draw path; reset to -1 by the texture and combine setters |
| `+0x24`, `+0x28` | combine function, colour / alpha | slot 57, `SetCombiner` |
| `+0x2C`..`+0x34` | colour sources 0-2 | slot 56, `SetCombiner` |
| `+0x38`..`+0x40` | alpha sources 0-2 | slot 56, `SetCombiner` |
| `+0x44`..`+0x4C` | colour operands 0-2 | slot 55, `SetCombiner` |
| `+0x50`..`+0x58` | alpha operands 0-2 | slot 55, `SetCombiner` |
| `+0x5C`, `+0x60` | scale, colour / alpha | slot 54, `SetCombiner` |

The stage constructor (`0x00882090`–`0x008820F8`, RD9) writes the following values; offsets are relative to
each record, not to the driver. Unlisted padding is not specified.

| Stage fields | Initial values |
|---|---|
| dirty / enabled / has-alpha bytes `+0,+1,+2` | 0, 0, 1 |
| texture name `+4`, coordinate selection `+8` | 0, 0 |
| magnification/minification `+0x0C,+0x10`; wrap S/T `+0x14,+0x18` | 0, 0; 3, 3 |
| environment `+0x1C`, applied environment `+0x20` | 0, -1 |
| colour/alpha combine `+0x24,+0x28` | 0, 0 |
| all six colour/alpha sources `+0x2C..+0x40` | 3 |
| three colour operands `+0x44..+0x4C` | 2 |
| three alpha operands `+0x50..+0x58` | 4 |
| colour/alpha scales `+0x5C,+0x60` | 0, 0 |

Default-device-state reset does **not** reconstruct these records: it sets global stage-dirty and every
record's dirty byte to 1, retaining bindings, sampler parameters, combine values and applied-environment cache
(`0x00887580`–`0x008875B9`). The device calls listed in §7 still occur. Constructor values and reset effects
are different contracts; the §4.4 reference to default state must be read together with this paragraph.

### 10.2 Setters

- `TexParameter(target, pname, v)` (29, `0x00884990`): if the "parameters follow the texture" flag is set, the
  value is first stored into the bound texture's record (pname 0-3); if that fails (invalid name, or pname > 3)
  → error code 1 and nothing else. Then pname 0-3 updates the stage record's parameter and marks dirty only when
  the value changed; any other pname marks the record dirty without storing anything.
- `TexEnv(target, pname, mode)` (28, `0x00884960`): stage record environment mode := `mode`, marks dirty
  (pname not checked, no flush).
- `TexEnv(target, pname, const float* rgba)` (27, `0x008830A0`): flushes the queue, then texture factor (render
  state 60) := quantised colour (§5.5); error code by §3.
- `TexStageCoord(v)` (52, `0x00883550`): the coordinate index is `v & 7`; `v & ~7` selects generation: 0 →
  none, 0x10 → camera-space position (`0x20000`), 0x20 → camera-space normal (`0x10000`), 0x30 → camera-space
  reflection vector (`0x30000`); any other → error code 2 and nothing sent. Otherwise flushes the queue (error
  by §3) and sets the current stage's coordinate-index state (11) to generation | index (error by §3).
- `TexStageMatrix(m, count, unused, flags)` (53, `0x00883600`): flushes the queue. If `m` is null or an identity
  matrix (`0x00881970`): texture-transform flags (state 24) := 0 (disabled). Otherwise copies the 16 floats and
  negates the third row (elements 8-11) when `count` = 3 with flag bit 0 clear, or `count` = 4 with flag bit 0
  set; texture-transform flags := `count`, plus projected (`0x100`) when flag bit 1 is set; texture transform
  of stage `i` (transform state 16 + `i`) := the copy. Device results are ignored.
- Slots 54-57 set combine fields of the current stage, then mark the record and the set dirty and reset the
  applied-environment cache. Slots 54, 55 and 57 first require the combine capability byte `+0x209`, which is
  byte `+0x29` of the current mode record (§6.2): 0 until the 3D environment has been set up, then always 1
  (`0x00886E80`, no device capability is tested): without it → error code 2.
  Slot 56 has no such check.
  - 57 (`0x00885130`): `(which, f)`: 0 → colour function, 1 → alpha function.
  - 56 (`0x00885190`): `(which, s)`: 0-2 → colour source 0-2, 4-6 → alpha source 0-2.
  - 55 (`0x00885220`): `(which, o)`: 0-2 → colour operand 0-2, 4-6 → alpha operand 0-2.
  - 54 (`0x008852D0`): `(which, s)`: 0 → colour scale, 1 → alpha scale.
- `SetCombiner(const byte d[10], stage)` (62, `0x00884A70`): flushes the queue; the stage's environment mode := 4
  and the record and set are marked dirty, but the applied-environment cache is **not** reset *[quirk: a new
  combiner on a stage already drawn in mode 4 is not re-sent until the cache is reset by a texture or slot 54-57
  call or by a disabled draw]*. No range check on `stage`. Unless `d[0]` = 0xFF: colour function := `d[0]`,
  alpha function := `d[4]`; colour sources 0-2 := low nibbles of `d[1..3]`, colour operands := (2, 3, 4, 5)[high
  nibble]; alpha sources 0-2 := low nibbles of `d[5..7]`, alpha operands := (4, 5, 4, 5)[high nibble]; colour
  scale := `d[8]`, alpha scale := `d[9]`.

### 10.3 Applying a stage at draw time (`0x00884320`, `0x00884500`)

`DrawArrays`, `DrawElements` and `0x00885DD0` (§8.2 step 2) send each dirty record. Stage `i`:

1. **Texture and sampler** (`0x00884320`). If texturing is disabled on the stage, or the name is invalid, or it
   has no surface: environment cache := -1; colour op := disable, alpha op := disable; step 2 is skipped (the
   device's bound texture is left as it was). Otherwise bind the surface, then:
   - magnification 0 → point, 1 → linear (other values: unchanged);
   - minification (min filter, mip filter): 0 → (point, none); 1 → (linear, none); 4 → (point, point);
     6 → (point, linear); 7 → (linear, linear); **5 → (point, linear)** *[quirk: the case falls through
     into 6 after sending (linear, point)]*; 2, 3 and others: unchanged;
   - wrap S / wrap T: 2 → clamp, 3 → wrap (other values: unchanged; address states 13 / 14).
2. **Environment** (`0x00884500`), only when the mode differs from the cached one (the cache is updated
   first). D3D7 names; "tex" = texture, "cur" = current:

| Mode | Colour | Alpha (texture has alpha) | Alpha (no alpha) |
|---|---|---|---|
| 0 replace | select arg1 = tex | select arg1 = tex | same |
| 1 modulate | modulate tex, cur | modulate tex, cur | select arg1 = cur |
| 2 decal | blend by texture alpha (tex, cur) | select arg2 = cur | select arg1 = cur |
| 3 | disable | disable | disable |
| 4, 5 combine | from the combine fields (below) | from the combine fields | same |
| other | nothing sent | | |

   In mode 2 without alpha the colour is select arg1 = tex.

   **Combine** (`0x00882100` colour, `0x00882300` alpha). Source codes map through `0x00AC4604` = (0 →
   texture, 1 → current, 2 → texture factor, 3 → diffuse); 4 and above read past the table *[undefined]*. Colour
   arguments 1 and 2 = sources 0 and 1, with colour operand 3 → complement, 4 → alpha replicate, 5 → both;
   alpha arguments 1 and 2 = alpha sources 0 and 1, alpha operand 5 → complement. Source 2 only selects the
   interpolation op. Function → op (colour uses colour scale and source 2; alpha uses alpha scale and alpha
   source 2):

| Function | Op |
|---|---|
| 0 | select arg1 |
| 1 | scale 0 → modulate, 1 → modulate 2×, 2 → modulate 4×, other → *[undefined value]* |
| 2 | add |
| 3 | scale 0 → add signed, other → add signed 2× |
| 4 | by source 2: 0 → blend texture alpha, 1 → blend current alpha, 2 → blend factor alpha, 3 → blend diffuse alpha, other → colour: nothing sent / alpha: *[undefined value]* |
| 5 | dot product 3 |
| other | disable |

## 11. Viewport, polygon offset and blits (slots 67-76)

The driver keeps the full-window device viewport (D3D7 layout: x, y, width, height, min z, max z) at object
`+0x238`..`+0x24C`, set up with the environment (§4.3). Viewport rectangles in the interface are **bottom-up**
(origin at the lower-left corner); the device's are top-down.

- `SetViewport(x, y, w, h)` (74, `0x008837E0`): flushes the queue (error by §3). Clamps as unsigned values
  against the full width `W` (`+0x240`) and height `H` (`+0x244`): x := min(x, W), y := min(y, H), then
  w := W − x when x + w > W, h := H − y when y + h > H (negative inputs therefore clamp to the edge). Sends the
  device viewport (x, H − y − h, w, h) with the full viewport's min/max z (error by §3). Then re-loads the
  stored matrix at `+0x150` through `LoadMatrix` (slot 41) with the word `+0x214` forced to 1 for the call and
  restored afterwards: the projection is re-sent with the half-pixel shift for the new size (§8.6).
  Example: window 800×600, `SetViewport(100, 50, 200, 100)` → device viewport (100, 450, 200, 100).
- `SetViewport()` (75, `0x00883760`): flushes, sends the stored full viewport, then the same matrix re-load.
- `GetViewport(out[4])` (76, `0x008838D0`): while the device is lost (byte `+0x261`, written only by `Flush`: 1 when the
  loss is detected `0x00884DCD`, 0 once the surfaces are restored `0x00884BE6`; 0 at construction), writes four
  zeros. Otherwise reads the device viewport (result ignored) and writes (x, H − y − h, x + w, H − y), i.e.
  left, bottom, right, top in bottom-up coordinates; with the example above: (100, 50, 300, 150).
- `PolygonOffset(v)` (67, `0x00883710`): flushes, then render state 47 (z-bias) := `v` as an integer; errors by §3.
- `BitBlt` (68), `StretchBlt` (69), `BitBltAlpha` (70, 72) and `StretchBltAlpha` (71, 73) are unsupported:
  each sets error code 3 and returns, popping 9, 11, 10 and 12 argument dwords respectively (`0x00882050`,
  `0x00882060`, `0x00882070`, `0x00882080`; 72 and 73 share 70 and 71's bodies).

### 11.1 Ending the scene (`0x00887260`)

When a scene is open (`+0x260`) and there is a device: end the device scene (error by §3) and mark the scene
closed. It does not present; in `Flush` (§4.4) presentation is the context call that follows (§14.3).

## 12. Owner inventory and remaining graphics slots

### 12.1 Attribution and interface boundaries

This inventory is by responsibility, not by demangled-name prefix. Shared helpers and incorrectly attributed
OpenGL bodies must not be counted as additional DirectX implementations. It is an interface inventory, not a
claim to have independently specified every byte in the approximate 139 KB owner group.

| Owner / support object | Evidence and responsibility | Contract / remaining boundary |
|---|---|---|
| `nSGLDX7::cGDriver` | graphics table `0x00AC4660`, constructor `0x00885F90`; owns managers, window and context | §§2–11, §12.2; auxiliary identities below |
| `cTextureStageData` | 0x80-byte cached stage records; constructor `0x00882090`, application `0x00884320` / `0x00884500` | §§7, 10, 13 |
| `cTextureManager` | surface-copy/lifetime `0x0088A380` onward; 48-byte named records, device surfaces and converters | §9, including RD6 arithmetic/decoder rules in §9.6 |
| `cPrimitiveManager` / `cPrimitive` | queue and index scratch `0x00889B60`–`0x0088A380`; queue entries refer to vertex ranges and index storage | §8.3; lifetime/order obligations §13 |
| `cVertexBuffer` | 0x30-byte value records; copy constructor `0x008895B0`, assignment `0x00889620`, recreation `0x008896B0`, reserve/unlock `0x008899B0` / `0x00889A90` | §14.1 internal value records; auxiliary tables do not return independent vertex buffers (§12.4) |
| `cGDriver::D3DLight` | constructor `0x00883EC0`; auxiliary light operations reach it at `0x00886BC9` / `0x00886CD9` | D3D7 light record; initialization below; public setters §12.4 |
| Presentation context (descriptive name) | allocation `0x0088C5F0`, initialization `0x0088BF50`, surface/device rebuild `0x0088BFC0`, presentation `0x0088C400` | §14.2–§14.3, including selector/depth rules |
| Placeholder device | allocator `0x0088B440`; installed during lost-device handling | inactive-device sink in §4.4; its methods are not graphics-interface slots |
| Shared vertex-format helpers / vertex copiers | `0x008891C0`, `0x00889280`, `0x0088C680` onward | graphics-formats.md and §8; not independent driver instances |
| `nRZSimGL`, colour-key blitters, `cGZBufferSimGLCache` | third graphics table `0x00ACC4D8`, distinct from this driver's table | sibling support; see renderer-gdrivers-director-inventory.md §§5–8; no DirectX blit support is implied |
| `cSC4GDriversCOMDirector` | class registration and factory ownership | renderer-gdrivers-director.md; separate owner |

`D3DLight` initializes a D3D7 light record to type 1, zero colours/position, direction (0, 0, 10), falloff bits
`0x3F800000`, attenuation zero, theta bits `0x3F47AE14` and phi bits `0x3FC8F5C3`. Range is the square root of
the double constant at `0x00AC4658`, stored to a single float (`0x00883EC0`–`0x00883F2D`). The constant's
binary64 bits are `0x47EFFFFFE0000000`: exactly the largest finite binary32 value, not the largest double.
The constructor loads that double, evaluates x87 square root and rounds on the single store at `0x00883F2A`.
With nearest/even rounding the range bits are `0x5F7FFFFF` (18446742974197923840); do not substitute
`FLT_MAX`, infinity or exactly 2^64. The constructor does not change the ambient x87 control word.
These record fields are floats, not doubles. This record
does not establish a public graphics lighting API: none of graphics slots 3–82 is a light setter.

### 12.2 Identity, getters and unsupported tail slot

QueryInterface from graphics slot 0 adjusts from the graphics identity to the owner (`0x00886440`), then uses
the following numeric IDs (`0x00882A00`). Success writes the output and increments the relevant reference count;
an unsupported ID returns false and leaves the output untouched. A null output is not a supported success case.

| ID | Returned identity (relative to owner) |
|---|---|
| `1`, `0xA4554849` | graphics interface, `+0xC` |
| `0x669565FE` | primary interface, `+0x0` |
| `0xE69BFE2A` | auxiliary interface, `+0x4` |
| `0x87E2B87D` | auxiliary interface, `+0x8` |
| `0x09CD86F9` | primitive manager (its own reference count) |

Slots 1–2 use the driver's shared reference counter, not one count per interface pointer. An implementation
may use different in-memory offsets; the aliasing and ownership are the contract.

| Graphics slot | Interface-level contract | Evidence |
|---|---|---|
| 47 `GetBoolean(pname, out)` | no-op: does not read or write output, does not set error, does not flush; no return value is established | shared body `0x006F7DC0`, one return consuming two words |
| 48 `GetInteger(pname, out)` | row-length query as §9.5; this is not a general render-state readback facility | `0x00881FE0` |
| 49 `GetFloat(pname, out)` | same no-op contract as slot 47 | `0x006F7DC0` |
| 77 `GetDriverInfo()` | returns stored driver-information text (§4.1); does not query the adapter again | `0x008870A0`, shared storage `0x00B54340` |
| 78 `GetGZCLSID()` | returns `0xBADB6906` | `0x00886E00` |
| 81 `IsDeviceReady()` | returns the inverse of the lost byte; does not test initialization, window existence or cooperative level; construction therefore reports true | `0x00886380`, graphics-identity offset `+0x255` = owner `+0x261` |
| 82 (name unresolved) | consumes two argument words, returns zero, changes no state/output/error and does not flush; do not interpret the unrelated folded disaster-function name as its meaning | `0x0088B3E0` |

### 12.3 Complete graphics-slot checklist

| Slots | Contract location |
|---|---|
| 0–2 | §12.2 identity and reference ownership |
| 3–5 | §8.1–§8.3 drawing and arrays |
| 6–10 | §8.5 vertex formats |
| 11–23 | §5 clear and render-state operations |
| 24–26 | §9.5 binding, upload, unpack row length |
| 27–29 | §10.2 texture environment and parameters |
| 30–34 | §8.4 fog and vertex/material multipliers |
| 35–39 | §9.5 texture names and residence |
| 40–42 | §8.6 matrix selection and transforms |
| 43 | §4.4 frame boundary and loss/recovery |
| 44–46 | §5.4 capabilities |
| 47–50 | §12.2 getters, §3 error channel |
| 51–57 | §10 stage selection, coordinates, matrix and combine fields |
| 58–62 | §9.5 direct texture access, §10.2 packed combiner |
| 63–66 | §§4.2, 6 mode list, getters and selection |
| 67–76 | §11 bias, unsupported blits and viewport |
| 77–82 | §§4.1, 12.2 information, lifecycle, readiness and unsupported tail |

### 12.4 Auxiliary dispatch and lighting/material contracts (RD7)

These tables are distinct from the graphics table. Slot numbers below are zero-based **within their own
table**; they must not be added to the 83 graphics slots. Table addresses and identity adjustments are checked
against the PE words at `0x00AC47AC`, `0x00AC47DC`, `0x00AC47EC` and the thunks `0x00886390`–`0x00886440`.
Offsets in the lighting bodies are relative to owner+8: for example material `+0x284`, device `+0x21C` and
error `+0x114` there mean owner `+0x28C`, `+0x224`, `+0x11C`. They are not additional state objects.

| Identity/table | Public slots beyond QI/AddRef/Release (0–2) |
|---|---|
| owner+0, ID `669565FE`, table `00AC47EC` | 3: constant-true stub; 4: NewBufferRegion (`886750`); 5: delete region (`885330`); 6: save region (`8853B0`); 7: restore region (`8855F0`); 8: region predicate (`885790`); 9–10: constant-true stubs |
| owner+4, ID `E69BFE2A`, table `00AC47DC` | 3: CopyColorBuffer (`883950`); no subsequent public slot is established by this table |
| owner+8, ID `87E2B87D`, table `00AC47AC` | 3: lighting enable; 4: individual light enable; 5: global ambient; 6–7: light colours; 8–9: no-ops; 10–11: material colours/properties |

The constant-true body (`0x00648F00`) sets only the low return byte to 1, consumes no stack arguments and
changes no state. It does not manufacture or return a vertex-buffer object. The two lighting no-ops
(`0x006F7DC0`) consume two argument words, do not dereference output and do not flush or set error.
An unrelated folded function name is not evidence of a supported vertex-buffer or light-position API.
QI and reference operations use §12.2's shared driver ownership. The later words after the primary table's
slot 10 are destruction support, not additional region operations. Internal vertex-buffer value records
remain §14.1; none of the above functional slots returns an independently refcounted vertex buffer.

All functional lighting/material setters below return void. Unless suppressed by the enable cache, they
flush the primitive manager, apply §3's HRESULT mapping to a nonzero flush result, then make the specified
device call **even after a flush failure**, mapping its nonzero result too. Zero results do not clear a
previous error. No setter checks device readiness or rolls back its cached edits on device failure.

| Auxiliary slot/body | Arguments and exact effect |
|---|---|
| 3, `00883F30` | One argument word; compare/store its low byte at auxiliary `+0x2D4`. If unchanged, no flush or device call. Otherwise flush, then SetRenderState(137, zero-extended low byte). Values such as 2 are preserved rather than canonicalized to 1. |
| 4, `00883FA0` | `(index, enabled)`: always flush then LightEnable(index, zero-extended low byte of enabled). No index validation, allocation or enable cache. |
| 5, `00884000` | `(r,g,b)` binary32 components: construct opaque packed ambient as below, flush, then SetRenderState(139, packed colour). No cache or clamp. |
| 6, `00886CA0` | `(index, ambient*, diffuse*, specular*)`: grow the light vector to index+1 with §12.1's default records if needed. Copy each non-null four-float colour into that record; null retains the old colour. Flush then SetLight(index, whole record), including when all pointers are null. |
| 7, `00886B90` | `(index, property, values*)`: same growth; properties 0/1/2 copy four words to ambient/diffuse/specular respectively. Other properties change nothing but still flush and submit the whole record. Recognized properties require readable values. |
| 10, `00884200` | `(ambient*, diffuse*, specular*, emissive*, power)`: each non-null colour copies four words into the persistent material. Store power only when its binary32 value compares greater than or equal to zero. Negative and unordered values retain the old power; negative zero is accepted and its bits retained. Flush then SetMaterial(whole record), even for all-null colours and negative power. |
| 11, `008840D0` | `(property, values*)`: properties 0/1/2/3 copy four words into ambient/diffuse/specular/emissive; property 4 copies one word into power without a numeric test. Other values leave the material unchanged. Always flush and SetMaterial afterward. |

The D3D material's order in memory is diffuse, ambient, specular, emissive, power, despite the public
argument/property order beginning with ambient. Colours are copied bitwise, not multiplied, clamped,
premultiplied or normalized. Light edits cannot alter position, direction, type, attenuation, range or cone
angles through slots 6–7. Vector growth creates default records for skipped indices; enabling a light does
not perform that growth. Light range is exactly §12.1, not a decompiler-folded literal.

For global ambient, add binary32 49152.0 to each supplied component in x87 and store the sum to binary32.
Interpret that stored word as unsigned bits and subtract `0x47400000` modulo 2^32; subtract the next byte
from its low byte modulo 256. That byte is the component. Pack `FF`, r, g, b from high to low byte.
This is a bit-based conversion, **not** float-to-integer truncation or saturation (`0x00884000`–`0x00884078`).
With nearest/even, inputs (0, 0.5, 1) produce `FF0080FF`; component 2 produces 254, demonstrating the
absence of a clamp. Power's comparison/store is instruction-checked at `0x008842AF`–`0x008842C4`;
bulk/individual light colour writes at `0x00886C13`–`0x00886C51`, `0x00886CFC`–`0x00886D54`.

### 12.5 Buffer-region handles and transfers (RD7)

These are owner+0 slots 4–8, not methods on the internal vertex-buffer value records. A region is a
28-byte entry in the vector at owner `+0x27C`: zero-based identity/sentinel, type, owned surface pointer,
two zero reserved words, recorded window width and height. Public handles are entry index+1; zero is failure.
They have no generation counter or separate AddRef/Release. All offsets here are owner-relative.
The context aliases and automatic population of the type-selected owner slots are specified in §14.2;
in particular ordinary setup leaves the type-2 slot null.

NewBufferRegion(type), `0x00886750`, returns zero immediately while lost. Otherwise select the first free
entry (identity `FFFFFFFF`), or append a cleared entry. Type 0/1/2 selects owner surface `+0x22C`/`+0x230`/
`+0x234`; another type or missing surface fails. Obtain its pixel format and surface descriptor, failing on
either negative HRESULT. Create a new surface with descriptor size `7C`, flags `1007`, the source surface's
dimensions and pixel format; caps include `40` for type 0 or `20000` for types 1/2, plus `4000` when the
source caps have `4000`, otherwise `800`. A missing DirectDraw object fails. If CreateSurface returns
`8876017C`, call the device's EvictManagedTextures once and retry that same descriptor once; any final
negative result nulls the entry's surface and fails (`0x00886970`–`0x008869C4`). No primitive flush or
driver-error assignment occurs here. Success stores index, type and **current window** dimensions (not
necessarily the queried surface dimensions), and returns index+1. Failed entries stay free for reuse.

DeleteRegion(handle), `0x00885330`, unsigned-checks handle−1 strictly below the vector count, then verifies
the stored identity matches. Failure returns false and changes nothing. Success Releases the owned surface
if present, zeros the other six words, stores identity `FFFFFFFF`, and returns true. A second delete is false;
a later creation can reuse the handle for a different region. It neither flushes nor checks lost state.
The predicate at `0x00885790` checks the same identity but uses **less than or equal to count**, an off-by-one
read (`0x008857B8`–`0x008857C7`). For in-range entries it reports occupancy; handle zero is false. A handle
equal to count+1 can read outside the logical vector and has no portable Boolean result; do not assert safe
rejection or dereference it in a conformance harness.

Save/Restore each take `(handle, x, y, width, height, otherX, otherY)`. They return false immediately when
lost, when x/y are negative, or when the unsigned 32-bit sums x+width or y+height exceed current window
dimensions. There is no independent positive width/height test, clipping, bottom-up conversion, handle
range test or occupied-identity check on these transfer paths. A valid live handle and readable entry are
caller preconditions. Restore additionally signed-compares those two sums against the entry's saved width/
height. Rejected rectangles do not flush. Accepted rectangles flush primitives and map a nonzero flush
result into the driver error, but proceed despite failure. Missing region or selected live surface returns
false; Restore additionally requires owner `+0x230` to be non-null **even when the type selects another
surface** (`0x0088568C`–`0x008856CC`).

Save, `0x008853B0`, Blts **into the region**: destination `(x,y,x+w,y+h)`, source selected live surface at
`(otherX,otherY,otherX+w,otherY+h)`. Restore, `0x008855F0`, Blts **from the region**: source `(x,y,x+w,y+h)`,
destination selected live surface at `(otherX,otherY,otherX+w,otherY+h)`. In both cases flags first
`01000200`, then a single retry with `01000000` if the first result is negative; FX pointer null.
Nonnegative results are successes, not just zero. Blt failures themselves do not set the driver's error.
Coordinates are passed unchanged; the otherX/Y rectangle is not separately checked by the wrapper.

Save's second failure returns false unless it is `887601C2`. On that result it calls Restore on the region,
then context support slot 11 (owner `+0x258` object's table +`2C`); a negative context result returns false.
Then it invokes primitive-manager support slot 11. If region Restore did not return exactly zero, retry
the Blt pair without clearing. If Restore returned zero and the destination covers the full recorded region
(x=y=0 and w/h equal saved dimensions), also retry without clearing. Otherwise clear the region first:
zero-initialized 100-byte FX, flags `01000400` for type 0, `03000000` for type 1; type 2 has no clear call.
The clear result is ignored. Retry afterward, but retain a false result flag: a later successful save still
returns false because previous saved contents outside this rectangle were discarded. Further lost results
can repeat this sequence (`0x0088551A`–`0x008855D6`).

Restore has no such recovery loop. If its final Blt result is `887601C2`, call **IsLost** on the region
surface (table +`6C`), ignore that result, then return false. It does not call Restore or retry after IsLost
(`0x0088574D`–`0x00885772`). A final nonnegative Blt returns true, every other negative result false.

### 12.6 CopyColorBuffer (RD7)

Owner+4 slot 3, `00883950`, takes `(x,y,width,height,destination)` and returns a buffer pointer,
not a success Boolean. It reads owner `+22C` (the render target, §14.2), with no null-surface guard.
While owner lost byte `+261` is set it returns null before flushing or inspecting the destination.
Otherwise it flushes, mapping nonzero results to the driver error but continuing. Owner bytes
`+288`/`+289`, initially zero, mean probe-done/probe-succeeded. The first call locks the whole
surface with flags 1, retrying `8876021C` without a bound; a nonnegative result is unlocked and
cached as success, any other negative result as failure. Every later call uses this cached verdict,
including across mode changes and loss/recovery. Cached failure returns null even for a supplied
destination. The probe's HRESULT is not assigned to the driver error (`0088398C`–`00883A10`).

After a successful probe, each call separately locks the whole target with the same retry rule.
A negative result returns the supplied destination unchanged (null if none), without Unlock or
destination allocation. Once locked, all normal exits unlock the surface once; Unlock's result
is ignored. If no destination was supplied, request a buffer through RZ::GS service slot 3.
Missing service/factory output leaves the return null. No destination AddRef or Release occurs
on any path: a supplied pointer is borrowed and returned unchanged, and a newly created buffer
is returned with its factory reference even if initialization or copying fails. An uninitialized
destination is initialized with `(width,height,pixel-format 9)`; failure returns that same buffer.
An initialized destination is not resized: the working width/height become the signed maximum
of requested and existing dimensions. Source right/bottom were already calculated from the
original request and are **not** recomputed (`00883A16`–`00883B6A`).

Source left/top are signed max(x,0)/max(y,0), right/bottom signed min(x+originalWidth,
surfaceWidth)/min(y+originalHeight,surfaceHeight). Addition is wrapping 32-bit. Clipping does
not translate the destination origin: copied rows start at destination (0,0), advancing by its
byte pitch, while source rows start at clipped left/top and advance by surface pitch. There is
no vertical reversal. The black-background condition is the literal quirk: clipped left differs
from x, or clipped top differs from y, or clipped right differs from x+workingWidth, or
`clippedBottom+y+workingHeight != 0` modulo 2^32. In particular an ordinary positive full copy
usually clears the destination first. When that condition holds, attempt destination lock
`8001`; on success fill `FF000000` with second argument zero, then unlock `8001`. A failed
clear lock does not abort the copy (`00883B6A`–`00883BF5`).

Source format must have RGB flag `40` and bit count 16, 24 or 32. Otherwise attempt the same
lock/fill/unlock with colour `FF00FFFF`, then return the destination. Accepted formats attempt
destination lock `8080`; failure returns it without destination unlock. Success copies the
clipped width for clipped-height rows, then unlocks `8080`, including an unrecognized 16-bit
mask combination which copies nothing. Supported positive rectangles and positive destination
pitch are the portable conformance domain: there is no empty-intersection/negative-size guard;
row termination uses an unsigned pointer comparison with base+height*pitch, so disjoint or
wrapped rectangles must not be modeled as guaranteed safe empty copies.

| Source layout | Result for each copied pixel |
|---|---|
| 16 bits, R/G/B masks `F800/07E0/001F` | Opaque `FFRRGGBB`; each channel v of n bits becomes floor(255v/(2^n−1)); alpha mask is ignored |
| 16 bits, `7C00/03E0/001F`, alpha mask zero or `8000` | Same 5-bit scaling; alpha is FF when absent, otherwise 00/FF from bit 15 |
| 16 bits, `F800/07C0/003E`, alpha mask 1 | Same 5-bit scaling; alpha 00/FF from bit 0 |
| 24 bits | For source bytes b0,b1,b2, destination bytes are **FF,b0,b1,b2**, in increasing address order; masks are not consulted |
| 32 bits | Bitwise dword copy, including alpha, regardless of channel masks |

The 16-bit scaling is division with truncation, **not** bit replication (§9.6 uses replication).
Evidence: `00883C30`–`00883E3B` for pitch, mask selection and copying; `00881B66`–`00881BE1`,
`00881C06`–`00881C81`, `00881CA6`–`00881D29`, `00881D56`–`00881DDA` for integer scaling/alpha;
`00883E44`–`00883EB0` for fill and returns; constructor cache writes `0088617A`/`00886180`.

## 13. Cache and ordering obligations

The driver does not maintain one general render-state cache. Immediate state setters submit queued primitives
before changing the device; deferred texture setters mutate stage records for the next draw. An implementation
must preserve these boundaries even when it can optimize redundant device calls.

| Operation class | When existing queued geometry is submitted | Where the new state takes effect |
|---|---|---|
| Depth, blend, alpha, stencil, fog, multiplier, matrix and viewport changes | before the change (§§5, 8, 11; retain individual unsupported/no-op exceptions) | subsequent device draws |
| `BindTexture`, `SetTexture`, scalar `TexEnv`, sampler `TexParameter`, combine fields | no submission merely for the setter | next draw processes dirty stages after submitting older queued geometry |
| Vector `TexEnv`, `SetCombiner`, texture coordinate/matrix | submit first | as detailed in §10.2; packed combiner still has the environment-cache quirk |
| `TexImage2D` | no submission | updates the existing surface, including a surface that queued geometry may reference |
| Direct `LoadTextureLevel` slot 61 | submit first | surface update follows earlier geometry |
| `Flush` | submit, close scene, present | next scene follows presentation |

There are two distinct stage-cache keys: dirty-record state and last-applied environment mode. Dirtying a record
does not necessarily invalidate the latter. In particular, repeated `SetCombiner` in environment mode 4 can
leave the previously applied combine operations active (§10.2). Stage selection alone is not a state application.
The parameter-following-texture flag is driver-wide: `BindTexture` enables it, while a changing `SetTexture`
disables it. A same-name `SetTexture` does not perform that transition (§9.5).

Device restore applies defaults, not a replay of the previous immediate render states. Texture *metadata*
survives loss; texture *contents* do not. A client reacting to restore messages must upload contents and issue
its desired immediate states again. Neither `AreTexturesResident` nor `IsTexture` proves that pixels survived,
or even that a surface currently exists. Name 0 is protected against public deletion but does not make all
other names permanent. Name reuse is LIFO and retains the fields specified in §9.1.

## 14. Device resources and presentation support

### 14.1 Vertex-buffer lifetime

The primitive manager's buffer records retain a reference to their D3D vertex buffer when copied; assignment
acquires a different incoming buffer before releasing the outgoing one (`0x008895B0`, `0x00889620`). A buffer
recreation forgets its cursor and mapped address and drops the previous buffer. A zero-capacity recreation is
a successful release. A nonzero recreation first asks DirectDraw to restore all surfaces; failure returns
`0x80004005`. A failed D3D buffer creation clears capacity and leaves no buffer (`0x008896B0`).

Reservation is exclusive: an already locked record, absent buffer or request exceeding capacity returns no
space. A request that would cross the end returns no space unless wrapping is allowed. Wrapping resets the
cursor and makes the next lock discard contents. Lock flags are `0x1021` ordinarily and `0x2021` on discard
(`0x008899B0`); they apply to the D3D vertex-buffer lock, not the texture-surface lock of §9.4. Surface-lost
failure recreates and retries; other failures return no space. A successful lock with a null mapped pointer
is unlocked and treated as no space. Unlock clears the mapped address/locked state and recreates the buffer
on a negative unlock result (`0x00889A90`). These failures must not expose a usable stale mapping to clients.

### 14.2 Context creation and mode resources

`SetupD3DX` requests a context with: device selector from owner `+0x274` (initially -1), full-screen boolean
equal to the inverse of the windowed flag, the low-byte flip-chain request, the driver window, requested
display depth 16, requested back-buffer count 2 when current-mode byte `+0x22` is nonzero (otherwise 0), and
the actual client width/height (`0x0088770A`–`0x0088774E`). These are the helper's inputs; mode-record RGB
depth is not forwarded in place of the literal 16. The context allocator reserves 0x48 bytes and reports
`0x8007000E` on allocation failure (`0x0088C5F0`).

The context loads `DDRAW.DLL`, resolves `DirectDrawCreateEx`, creates DirectDraw and obtains Direct3D7.
Missing library/entry point or an unusable DirectDraw capability result fails initialization
(`0x0088BEC0`–`0x0088BFB3`). A failed context initialization is destroyed by its allocator. Setup failure
then follows the window/environment cleanup of §4.2; success is not implied merely by allocating a context.

Surface rebuild first releases the previous device and surfaces. Full-screen uses cooperative flags `0x11`,
windowed uses `0x8`; full-screen also requests the supplied width, height and display bit depth
(`0x0088BFD6`–`0x0088C020`). When full-screen and the flip-chain request is nonzero, primary-surface creation
starts with the requested back-buffer count. Positive counts use descriptor flags `0x21`, caps `0x2218`;
zero uses flags `1`, caps `0x2200`. A failure retries decreasing counts down through zero. A successful
positive count requires an attached back buffer; failure to obtain one is fatal (`0x0088C0C5`–`0x0088C1A4`).
Thus the fourth mode argument requests flipping but does not guarantee a flip chain; allocation may fall
back to a count of zero. When either full-screen or the request is false, the context creates a primary
surface (caps `0x200`) and a separate width-by-height 3D render target (flags `7`, caps `0x2040`), and windowed
operation attaches a window clipper to the primary (`0x0088C1A6`–`0x0088C25D`). Device/depth-format selection
and selector limitations follow below.

**Selector and depth contract (RD8).** `0x0088C024`–`0x0088C0AE` recognizes three selectors. For -1,
enumerate devices and retain the first device of highest score, adding 1000 each for capability bits
`0x10000` (hardware rasterization) and `0x80000` (hardware transform/light). Equal scores keep the earlier
device. A nonzero render-bit-depth word at descriptor `+0x74` is the existing-selection sentinel; if it is
zero, the next descriptor replaces it regardless of score (`0x0088BDE0`–`0x0088BE3D`). For -4, the preferred
GUID is `{881949A1-D6F3-11D0-89AB-00A0C9054129}`, fallback
`{A4665C60-2673-11CF-A31A-00AA00B93356}`. Each preferred descriptor replaces the selection; a fallback
replaces it only if no nonzero render-bit-depth selection exists or its GUID is not preferred. Other GUIDs
are ignored (`0x0088BE40`–`0x0088BEB4`).
Selector -3 uses `{50936643-13E9-11D1-89AA-00A0C9054129}` directly, capabilities zero and render bit-depth
word `0x10`. It skips device enumeration. Every other value, including concrete nonnegative selectors and
-2, skips selection rather than indexing an adapter list: GUID/capability stack contents are not initialized
for a usable choice. There is no defined device-selection result for those values; do not invent fallback.

After surface setup the selected GUID is passed to `EnumZBufferFormats`. Its callback copies exactly the
first 32-byte pixel-format descriptor and returns cancel (zero); there is **no depth/stencil ranking**, no
comparison with the requested 16 display bits and no search for a later preferred depth format
(`0x0088BDC0`). No callback leaves format size zero and returns `0x80004005`. Depth-surface descriptor flags
are `0x1007`, width/height are context size, caps `0x20000` for a selected device with `0x80000`, otherwise
`0x20800`. The format is that first descriptor. Create it, attach to render target, query current display mode,
then create the selected device and set viewport `(0,0,width,height,0.0,1.0)`.

Every negative HRESULT from cooperative level, fullscreen display mode, device enumeration, primary or
render-target creation, attached-back-buffer enumeration, clipper creation, depth-format enumeration,
depth-surface creation, depth attachment, display-mode query, CreateDevice and SetViewport ends setup with
that HRESULT, subject only to the primary back-buffer-count retry already specified. A successful attachment
enumeration with null back buffer returns `0x80004005`; display RGB bits at most 8 returns `0x88760078`.
Depth-attachment failure releases that new depth surface, nulls its slot and preserves the attachment HRESULT
(`0x0088C2E9`–`0x0088C314`). Other acquired resources remain for normal context cleanup; no second device
or second depth format is attempted after failure. Clipper SetHWnd and primary SetClipper results, and the
three primary GetColorKey calls after chain creation, are ignored. A nonnegative final SetViewport result is
normalized to zero (`0x0088C315`–`0x0088C397`).

**Owner surface bindings (RD12).** Context getters at vtable byte offsets `18` and `1C` return
borrowed context fields `+18` (primary) and `+20` (depth); they do not AddRef. Setup assigns these
to owner `+228` and `+230` using AddRef-before-Release assignment (same-pointer assignment does
nothing). Owner `+22C` instead receives the owned result of device GetRenderTarget, after releasing
and nulling its previous value. It aliases context `+1C`, the back buffer/separate render target;
with the fullscreen zero-back-buffer fallback, that target aliases the primary. Owner `+234` has
**no setup or restore population**: it starts null and is not a second depth/stencil alias. Thus region
types 0/1/2 mean render target/depth/unpopulated slot, respectively; ordinary type-2 creation fails.
On first device loss, the owner's device, depth, primary and render-target references are dropped
and nulled, in that order. Recovery reacquires device, depth and primary from context, then
releases/nulls/requeries `+22C` (normally already null); failed context rebuild leaves the owner
lost with those slots null. Full cleanup releases/nulls all four surface slots, including
`+234` if externally populated. Neither loss, recovery nor setup resets the CopyColorBuffer probe
cache (§12.6). These are distinct owned references even when their pointer values alias. Evidence:
context table `00AC5060`, getters `0040CF10`/`004ED900`, assignment `00886EF0`–`00886F15`, setup
`00887ACE`–`00887B2F`, recovery `00884BE0`–`00884C51`, loss `00884E7C`–`00884EC1`, cleanup
`008870F2`–`0088718F`. Flush uses graphics identity owner+`C`: its apparent `+220` is owner `+22C`.

### 14.3 Presentation

The context's per-frame method is `0x0088C400`. With a successfully created positive-count flip chain it calls
primary-surface `Flip(null, flags)` and returns that HRESULT (`0x0088C406`–`0x0088C421`). Without a flip chain,
the separate-surface presentation path obtains the current window client rectangle, maps both corners to
screen coordinates, then blits from the render target into that destination on the primary. Source rectangle
is null (whole render target). Initial blit flags are `0x01000200`; on a negative result it retries once with
`0x01000000` (`0x0088C424`–`0x0088C4C4`). The current rectangle is queried each presentation; the snapshot
recorded during setup is not the sole source of window position.

A zero-width or zero-height client rectangle returns `0x887601C2` before either blit (`0x0088C43E`–`0x0088C450`,
`0x0088C4B1`). If neither presentation path is active, the incoming flags/result word is returned without
presenting. The normal `Flush` caller supplies zero. This establishes presentation independently of EndScene;
it does not establish which clients consume the device-lost message (no handler is inferred, see RD4 in §14).

### 14.4 Mode descriptor enrichment

The enumerated list contains 56-byte internal records; only the first 52 bytes are copied by public getters.
The private final dword (`+0x34`) is the device selector copied from owner `+0x274`, initially -1. The append
helper used when storage is full is a vector insertion/reallocation helper, not another display-mode filter
(`0x00887D80`, `0x008882F0`).

After setup, device properties enrich the selected list record before its public portion is copied back to
the current-mode record (`0x008886E0`–`0x00888817`, `0x00888E92` onward). The render target's pixel-format query
supplies dwords `+0x10` = alpha mask, `+0x14` = red mask, `+0x18` = green mask, `+0x1C` = blue mask. These fields
are zero in newly enumerated records. A query failure uses §3's HRESULT error mapping; it is not proof that
all descriptor fields are valid.

Capability enrichment (`0x00886E30`–`0x00886E8E`) sets `+0x21` from nonzero stencil-operation capabilities,
`+0x24` to the unsigned 16-bit maximum simultaneous textures, `+0x28` to whether that number exceeds one,
`+0x29` and `+0x30` to 1, and `+0x2A`, `+0x2D`, `+0x2E` to zero. `+0x2B` is bit 7 of the D3D7 capability
dword at offset `+0x44`; `+0x2F` is bit 0 of the dword at `+0x5C`. The texture stage count is taken from the
same maximum-simultaneous-textures word (`0x008886FD`–`0x00888711`). Byte `+0x2C` can be set by texture-format
enumeration. Its full meaning and the remaining bytes remain RD3. Do not equate the stencil capability flag
with independent proof of the allocated depth surface's stencil format.

## 15. Independent conformance vectors

These are interface test cases for a recording D3D7/DirectDraw backend and injected failures, not measured
in-game results. They can be implemented without access to RE sources. Start each case with a fresh driver
unless the setup column says otherwise. Observe calls, output sentinels, errors, resource references and messages.

| ID | Setup / input | Required observations |
|---|---|---|
| DX7-01 | Pre-fill boolean and float outputs with nonzero sentinels; call slots 47 and 49 | Outputs unchanged; no D3D calls, no flush or new error |
| DX7-02 | Call slot 82 with two valid sentinel words | Return zero; no output/state/device changes |
| DX7-03 | Query readiness before Init, after loss detected by Flush, after successful restore | true, false, true; query itself never calls cooperative-level testing |
| DX7-04 | Query unsupported interface ID with sentinel output; query graphics ID through an auxiliary identity | Failure leaves sentinel and counts unchanged; success returns the same graphics identity and adds one driver reference |
| DX7-05 | Queue triangle A; BindTexture B; queue triangle C; Flush | A submitted before B is applied; C uses B; presentation follows submission and EndScene |
| DX7-06 | Draw in combine mode 4; SetCombiner with different descriptor but same stage/mode; draw again | Dirty texture/sampler processing does not itself resend the environment; reset environment cache with a combine-field setter to make the next draw apply it |
| DX7-07 | BindTexture then TexParameter; changing SetTexture then TexParameter | First parameter follows into the named record; second updates only the stage. Same-name SetTexture must not be used to trigger that transition |
| DX7-08 | Allocate names A,B; delete A then B while ready; allocate twice | B then A are reused. A reused record resets dimensions but retains priority/format/parameters/mip count |
| DX7-09 | Lose device with a valid uploaded texture, then restore | Name validity/residence queries remain true; surfaces are recreated from metadata; contents require upload after restore notification |
| DX7-10 | Queue a draw using texture T, then update T by TexImage2D versus direct LoadTextureLevel | TexImage2D does not force the queued draw out; direct slot 61 submits it before modifying pixels |
| DX7-11 | Set a nondefault depth function, force loss and successful restore | Default depth function less-equal is installed; previous function is not automatically replayed |
| DX7-12 | Full-screen, flip requested, back-buffer count 2; inject creation failures at counts 2 and 1 | Attempts counts 2,1,0; count-zero success does not subsequently call Flip |
| DX7-13 | Windowed client 800×600 at screen origin (50,70); present | Blit destination (50,70,850,670), whole render target source, flags `0x01000200`; first failure causes exactly one retry with `0x01000000` |
| DX7-14 | Present a zero-width client rectangle | No Blt; context result `0x887601C2` |
| DX7-15 | Enumerate two display modes, then query record 0 before and after setup | Four list entries in reported full-screen/windowed pairs; masks/capability fields start zero and selected record is enriched; public copy never includes private selector dword |
| DX7-16 | Lock a vertex-buffer reservation, reserve again before unlock; then reserve across end without/with wrap | Second simultaneous reservation fails; no-wrap overflow fails; allowed wrap resets cursor and uses discard lock `0x2021` instead of `0x1021` |
| DX7-17 | Cause surface-lost then another HRESULT failure, reading GetError twice after each | Surface-lost maps to 0; other failure maps to 1; every read consumes the stored error |
| DX7-18 | Inject context creation failure during SetVideoMode; fourth word points to writable sentinel storage | Window/environment cleanup and error 6 precede a `0xFFFFFFFF` write through that fourth word; do not expect a current-mode-index assignment in its place (§4.2) |
| DX7-19 | Prefill compact destination with `A5`; copy one position-only vertex | XYZ copied, colour `FFFFFFFF`; bytes 16–31 stay `A5` |
| DX7-20 | Prefill with `A5`; copy position+UV0, then independently position+colour | First: white colour and copied UV0, untouched UV1. Second: copied colour, untouched both UV sets |
| DX7-21 | Queue two submissions; return failure then success; reverse outcomes | Both submitted; flush returns zero in first trial and final failure in second; empty flush returns zero |
| DX7-22 | Generate nonindexed quads for count 5 and 6 using physically oversized scratch | Both write `0,1,2,0,2,3,4,5,6,4,6,7`; declared counts 6 and 9 respectively; do not render invalid references |
| DX7-23 | Indexed quads count 5/6 with extra readable sentinel indices after logical input | Count 5 generates one group; count 6 generates two groups, using the extra inputs; declared counts 6/9 |
| DX7-24 | Width 30; m0=`3E2F59FF`, m3=`40A4645F`; nearest/even, extended precision | Projection corrected m0=`30888889`; reject binary64-only `30888888` |
| DX7-25 | Construct driver and stage records before caller setters | Clear colour/depth/stencil = 0/1.0/0; all material words zero; stage values exactly §10.1 |
| DX7-26 | Mutate stage binding/combine/cache, then default-device-state reset | Values retained; global and per-stage dirty become 1; §7 device calls still occur |
| DX7-27 | Shutdown initialized driver with existing modes and sticky error sentinel | List end becomes beginning, mode count zero; list helper does not overwrite sticky error |
| DX7-28 | Selector -1 enumerates scores 1000, 2000, 2000; nonzero render-bit-depth sentinel | First score-2000 descriptor retained; no tie replacement |
| DX7-29 | Enumerate depth formats 16-bit then 24-bit/stencil; separately return no format | First descriptor accepted and enumeration canceled; no ranking. No format returns `80004005` |
| DX7-30 | Inject depth-attachment failure, then independently final viewport failure | First releases/nulls depth surface and returns attachment HRESULT; second returns viewport HRESULT; no alternate-device retry |
| DX7-31 | Destroy driver with externally observed manager references | Primitive manager receives Release; texture manager receives AddRef, not Release; no implicit queued submission |
| DX7-32 | Create default light under nearest/even x87 rounding | Range bits `5F7FFFFF`; type 1, direction (0,0,10); zero colours; not FLT_MAX or 2^64 |
| DX7-33 | Lighting slot 3: set low byte 2 twice, then word `00000102`, then 0 | First flush/state(137,2); next two calls suppressed; last flush/state(137,0) |
| DX7-34 | Lighting slot 4: enable index 17 twice with word `00000102` | Two flushes and two LightEnable(17,2); no vector growth or equality suppression |
| DX7-35 | Ambient (0,0.5,1), then (2,0,0), nearest/even | Device state(139,`FF0080FF`), then state(139,`FFFE0000`); no clamping |
| DX7-36 | Set light index 2 with ambient pointer only; then individual property 99 with null pointer | Three default records created; only index 2 ambient changes; both calls flush/SetLight(2); unknown property does not read null |
| DX7-37 | Set material power via property 4 to -7, then bulk all-null/power -1, then power negative zero | First stores -7; second retains -7; third preserves negative-zero bits. Each call flushes and submits whole material |
| DX7-38 | Light/material flush returns generic failure, device call returns success; repeat with surface-lost device result | First leaves error 1; second maps last nonzero result to 0. Cache edits and device calls occur in both cases |
| DX7-39 | Request (16,0), enumerate (b,a)=(15,0), (32,8), (24,8); then add (16,1), then (16,0) | Select (24,8), then (16,1), then (16,0): excess beats shortage; exact-bit count wins; exact pair finally wins |
| DX7-40 | Request (16,4), enumerate (16,0), (32,1), (24,8), (32,4), (16,4) | Successive winners follow alpha ranks 4/5/6, then exact pair rank 7. Two equal rank/metric descriptors retain first even with different RGB masks |
| DX7-41 | Empty format enumeration; independently request 7 with only an ordinary fallback descriptor | Empty produces eight zero descriptors, not eight allocation successes. Request 7 can allocate fallback surface but source 14 upload has null converter and fails before lock |
| DX7-42 | Install descriptor keys `[0,1,3,4,7,9,11,13]` into requests 0–7, respectively; inspect all source rows | Sources 0–9 availability `11111000`; source 10 `01011100`; source 12 `01011010`; sources 11/13/14 `00000011`. This exercises all 120 cells independently of selector ranking |
| DX7-43 | Source 5 word `1234` and source 9 word `0821`, each to key 1 then 2 | Outputs `11443322` and `FF080408`; key 1 and 2 results identical; destination padding unchanged |
| DX7-44 | Linear-size surface reports 32 bytes; source 12/key 11; give w=h=1 and rowLength=99 | Copies 32 raw bytes in one row, without decoding or using source pitch. With x=1 copies no bytes but outer load unlocks and succeeds |
| DX7-45 | Create two regions, delete first twice, create another; predicate only in logical range | Handles 1,2; deletion true then false; next creation reuses 1; shared driver references unchanged |
| DX7-46 | CreateSurface returns `8876017C`, then success; repeat with another negative result | First evicts managed textures and retries same descriptor exactly once; second has no retry, returns handle 0; no flush/error assignment |
| DX7-47 | Valid save/restore `(h,10,20,30,40,50,60)`; first Blt fails, retry succeeds | Save region destination (10,20,40,60) from live (50,60,80,100); restore reverses roles. Flags `01000200`, `01000000`; no coordinate flip |
| DX7-48 | Restore type 0 with owner `+230` null; then restore with both Blts surface-lost | First returns false after accepted-rectangle flush without Blt despite non-null type-0 surface; second calls region IsLost once and returns false without recovery retry |
| DX7-49 | Partial save loses surface twice; region Restore=0, context recovery succeeds, retry succeeds | Region clear occurs (type 0/1 flags as §12.5), then retry; final Boolean false despite successful copy. Full-region save retries without clear and can return true |

| Vector | Setup / operation | Required distinction |
|---|---|---|
| DX7-50 | Encode A=8/15,R=1/15,G=8/15,B=15/15 as source 4, then source 5; convert to all ordinary keys | Both encodings give bytes `FF 88 11` at key 0, `881188FF` at keys 1/2, `145F` at key 3, `8A3F` at keys 4/5 and `818F` at key 7; padding untouched |
| DX7-51 | Source-6 R5=4,G5=16,B5=31,A1=1; repeat equivalent source 7; convert to 8/6/4-bit destinations | Key 1 `FF2184FF`; key 3 `243F`; key 7 `F28F`. Bit replication differs from floor(255v/31), e.g. v=4 gives 33 versus 32 |
| DX7-52 | Source 0 pixel `7F123456`, equivalent source 1, and source 2/3 RGB bytes; ordinary keys | Keys 4/5 give `08CA` for alpha 7F and `88CA` for absent alpha; key 7 `7135` versus `F135`. RGB quantization drops low bits; key 0 always B,G,R bytes |
| DX7-53 | DXT1 white/black endpoints FFFF,0000, selectors cycle 0,1,2,3 each row | To key 1: FFFFFFFF,FF000000,FFAAAAAA,FF555555; to key 4: FFFF,8000,D294,AD6B; to key 7: FFFF,F000,FAAA,F555 |
| DX7-54 | DXT1 endpoints 0000,FFFF; codes 2,3; repeat DXT3 with alpha F | DXT1 midpoint FF808080/C210/F888; code 3 zero. DXT3 midpoint identical RGB with explicit full alpha; code 3 FF000000/8000/F000, proving endpoint-order branch retained |
| DX7-55 | DXT3 c0=c1=0, selector 0; alpha nibbles 0,7,8,F | Key 1 alpha bytes 00,77,88,FF; key 4 alpha bits 0,0,1,1; key 7 alpha nibbles unchanged |
| DX7-56 | Decode 6×5 ordinary pixels from tightly packed DXT blocks; vary source stride while holding bytes fixed | Only top-left 4×4 changes; last two columns and last row stay sentinel; source-stride changes have no effect. Width 3 or height 3 writes nothing |
| DX7-57 | CopyColorBuffer first probe fails; call again, change mode, recover loss, call again | First and every later non-lost call flush then returns null without another probe/actual lock. During loss returns null before flush. Probe cache survives lifecycle changes |
| DX7-58 | Successful probe, actual lock fails, supplied destination D; then actual lock succeeds but destination init or copy lock fails | First returns D with no surface unlock/destination calls; later returns same D and unlocks surface. No destination AddRef/Release; null-input factory failure returns null after surface unlock |
| DX7-59 | Initialized 4×4 destination, request (−1,0,3,2) against larger 32-bit target | Black clear attempted, then source x=0..1,y=0..1 copied to destination origin; no horizontal offset or vertical flip. Working dimensions max to 4 but source right remains 2 |
| DX7-60 | CopyColorBuffer 16-bit R5=4,G6=0,B5=0 in 565; separately 24-bit bytes 12,34,56; then non-RGB format | 16-bit result FF200000 (division, unlike texture FF210000); 24-bit output bytes FF,12,34,56; non-RGB attempts fill FF00FFFF. Unsupported 16-bit masks instead unlock without copying |
| DX7-61 | Setup separate primary P, target T, depth Z, then first loss and successful recovery to P2,T2,Z2 | Owner 228=P,22C=T,230=Z,234=null; acquired owner references separate from context. Loss releases/nulls Z,P,T; recovery acquires Z2,P2 then owned GetRenderTarget T2. Type-2 region creation returns zero |
| DX7-62 | Fullscreen zero-back-buffer fallback, then failed recovery and full cleanup | Primary and target owner slots hold separately owned references to same surface. Failed rebuild keeps owner lost/null slots; cleanup releases each remaining owned reference independently, without resetting CopyColorBuffer probe |

## 16. Remaining proof boundaries

See [questions/renderer-driver.md](questions/renderer-driver.md).

The checklist covers the 83-slot graphics interface, including explicit no-ops. It does not assert full
coverage of undefined invalid-pointer/overflow inputs. Available pixel converters and CopyColorBuffer
are specified in §§9.6 and 12.6; RD12 surface aliases/ownership are in §14.2. RD6/RD7/RD12
have vectors DX7-32–62. RD8–RD11 are
answered in §§2, 4.1, 8.3, 8.6, 10.1 and 14.2 with vectors DX7-19–31. RD3 retains its
specifically listed caller, descriptor and message gaps.
No C ABI proposal or implementation behavior is inferred from those open items.
