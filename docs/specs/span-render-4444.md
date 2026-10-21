# Span renderer A4R4G4B4 (`nGZDrawSoftRender::cSpanRender4444`)

Status: RE spec, lane re-span4444-4, 2026-10-21. Source: SimCity 4 Deluxe GOG 1.1.641 Windows image, Ghidra export
(`re_query.py asm`) and raw table reads from the image. Every numeric rule below was read from the instructions at the
cited addresses; nothing is inferred from "usual" RGBA maths. Round 2 (same lane, same day) verified all 168 blit
kernels by executing their bytes against the model (section 9.5).

Scope: the owner range `0x00847CE0`-`0x00854E10` (172 functions, 51,809 bytes, `tools/coverage-ranges.tsv:21`): the
five vtable methods, the 166 blit kernels of its `RenderBlt` table that live in the range, and one helper.
Out of scope (other owners): `cGZDrawContextRenderToBuffer` itself (view3d.md section 7), the 1555/565/8888/8 span
renderers, fonts, base widgets, the DX7 driver.

## 1. Class and creation

- vtable `0x00AC185C`, 6 slots: 0 deleting destructor `0x0082FF00` (shared by all five span renderers; resets the
  vtable to `cISpanRender` `0x00AC17AC`, frees when flag bit 0 is set), 1 `RenderSolid` `0x00847CE0`,
  2 `RenderGradient` `0x00847D30`, 3 `RenderAlpha` `0x00847E40`, 4 `RenderAlphaGradient` `0x00847F10`,
  5 `RenderBlt` `0x008480C0` (Mac slot order, `re/symbols/gzdraw-softrender.tsv`).
- Created by `cGZDrawContextRenderToBuffer::Init` (`0x00830710`) when the target buffer's colour type is 7
  (A4R4G4B4), stored at context `+0xB0`. The object has no data members the methods read: every method takes all
  its inputs as arguments (`this` is never dereferenced in slots 1-5).

### 1.1 Native call boundary (SP3, re-spanbridge4444-4)

All facts below were read from the instruction bytes at the cited addresses.

**Object and vtable.** The renderer object is 4 bytes: only the vtable pointer. `Init` allocates it per context
with `GlobalHeapAlloc(4)` (`0x005E55E0`, call at `0x008307AA`), stores `0x00AC185C` into it (`0x008307B6`; jump
table `0x008308B0` indexed by colour type - 2, entry 5 = type 7) and the pointer into context `+0xB0`
(`0x008307EE`); a failed allocation stores 0. The vtable is one static table in `.rdata`: `0x00AC185C` occurs once
in the whole image as an immediate (that store), and each slot address 1-5 occurs once in the image (its own
entry; slot 0 `0x0082FF00` is shared with the other five span vtables and `0x00AC17AC`). So every A4R4G4B4 context points at the same six entries: patching entries 1-5 of `0x00AC185C` covers all
instances; nothing copies the table.

**Lifetime.** Context `Shutdown` (`0x0082FF20`, slot 4 of `0x00AC17D0`, and the first thing `Init` calls) and
`Release` (`0x008306C0`) call slot 0 with flags 1 and clear `+0xB0`. Slot 0 (`0x0082FF00`) is thiscall with one
dword (`RET 4`): it sets the vtable to `0x00AC17AC` and, when flag bit 0 is set, frees with `GlobalHeapFree`
(`0x005E5620`). A substitute object placed in `+0xB0` must therefore survive a slot-0 call with flags 1 that
releases it through its own slot 0.

**Calling convention (slots 1-5).** MSVC thiscall: `this` in ECX (ignored by all five; ECX is overwritten before
any read), arguments pushed right to left, the callee pops them, nothing is returned (EAX is undefined and no call
site reads it). EBX, EBP, ESI and EDI are preserved (each slot pushes and pops the ones it uses); the caller relies on
ESI after the call.

| Slot | Offset | Method | Arguments (dwords, in order) | Callee pops |
|---|---|---|---|---|
| 0 | +0x00 | deleting destructor `0x0082FF00` | `flags` (1) | `RET 4` |
| 1 | +0x04 | RenderSolid `0x00847CE0` | `spans, count, buffer, colour` (4) | `RET 0x10` (`0x00847D23`) |
| 2 | +0x08 | RenderGradient `0x00847D30` | `spans, count, buffer, grad` (4) | `RET 0x10` (`0x00847E33`) |
| 3 | +0x0C | RenderAlpha `0x00847E40` | `spans, count, buffer, colour, alpha` (5) | `RET 0x14` (`0x00847F00`) |
| 4 | +0x10 | RenderAlphaGradient `0x00847F10` | `spans, count, buffer, grad` (4) | `RET 0x10` (`0x008480B3`) |
| 5 | +0x14 | RenderBlt `0x008480C0` | `spans, count, rectA, rectB, src, mode` (6) | `RET 0x18` (`0x0084814F`) |

`buffer`, `grad`, `rectA`, `rectB` and `src` are pointers; `colour`, `alpha`, `count` and `mode` are by value.
The kernels RenderBlt calls are cdecl with 22 dwords (`spans`, `count`, 6 + 6 + 8 dwords of the copied records),
popped by RenderBlt (`ADD ESP,0x58` at `0x0084814A`).

**The only native caller** is `cGZDrawContextRenderToBuffer::DrawPreclippedSpanList` (`0x008302E0`, thiscall
`(spans, count)`, `RET 8`; slot 22 of `0x00AC17D0`). Byte sweep of `0x0082FD60`-`0x008310B0` for a `+0xB0`
displacement finds only the constructor, `Init`, `Shutdown`, `Release` and the four loads in this function (`0x008304AD`, `0x00830529`, `0x00830583`, `0x008305C2`); the
slots of `0x00AC17D0` outside that range are shared 1-8 byte stubs that do not touch `+0xB0`. The function picks
one slot per call from context fields:

| Context byte `+0x74` (gradient) | Context byte `+0x41` (alpha) | Call | Extra arguments from the context |
|---|---|---|---|
| 0 | 0 | slot 1 (`0x00830647`) | `buffer = ctx+0x78`, `colour = [ctx+0xC4]` |
| 0 | non-zero | slot 3 (`0x00830627`) | `buffer = ctx+0x78`, `colour = [ctx+0xC4]`, `alpha = [ctx+0xB4]` |
| non-zero | 0 | slot 2 (`0x008305FC`) | `buffer = ctx+0x78`, `grad = ctx+0x44` (12 dwords) |
| non-zero | non-zero | slot 4 (`0x008305E7`) | `buffer = ctx+0x78`, `grad = ctx+0x44` |

The textured path (three sites `0x008304D1`, `0x0083053B`, `0x00830591`) calls slot 5 with `rectA = ctx+0x78`,
`rectB` and `src` in locals of the caller, and `mode` from EDI (one site ORs 4 into it at `0x0083050F`). `spans`
and `count` are always the caller's own two arguments, passed through unchanged.

**No interface reaches the renderer.** No slot of `cIGZDrawContext` (`0x00AC17D0`) returns or sets `+0xB0`.
`cGZDrawContextRenderToBuffer` is created only by the factory at `0x0082B910` (the image's only `E8` call to the
constructor `0x00830690`): slot 3 of a 4-slot factory vtable `0x00AC162C` (QueryInterface `0x0082B850` accepts
IIDs 1 and `0x6B2EA76C`), thiscall `(buffer, out)`, `RET 8`, returns a bool in AL. It queries the buffer for
`0x20732180`, allocates 0xD4 bytes, constructs, AddRefs into `*out`; it does not call `Init`.
`cGZDrawSystem::Init` (`0x0082BC90`) appends this factory to the draw system's factory list, and
`cIGZDrawSystem` vtable `0x00AC1668` slot 4 (`0x0082BAB0`, thiscall `(buffer, out)`, `RET 8`) tries each factory
in list order until one returns true. `kcGZDrawContextAsBuffer` `0xAB300B20` is not on this path: its only code
uses are the unreferenced 6-byte getter `0x0082ABD0` and `cGZScreenBuffer::Init` (`0x00992044`), which passes it
with `0xAB300B2B` to slot `+0xC` of the GZCOM object. So a plugin can reach a 4444 renderer only by patching the
shared vtable entries, or by writing context `+0xB0` after `Init` (no interface does that).

**Which shipped path creates a colour-type 7 target: not found statically** (open as SP4). None of the 34 direct
calls to the colour-pair constructor `0x0044A490` pushes the literal 7 (literals seen: 1, 3, 9; the rest take the
type from data: buffer resources, `cGZGimexFactory::LoadWithReader`, `cSLBuffer::Load`), and the ten places in the image where
`PUSH 7` precedes a `+0x14` virtual call within 16 bytes are all simulation code (prop and network managers,
automata, vehicle collision), not calls of `cIGZDrawSystem` slot 5 `CreateBuffer(out, width, height, colourType)`
(`0x0082B980`, `RET 0x10`).

### 1.2 Where colour type 7 comes from in the shipped game (SP4, re-spanbridge4444-4 round 2)

Static result, read from instruction bytes. No runtime probe was run.

**The one shipped producer: the screen buffer.** `cGZGraphicSystemW32::OnCanvasInit` (`0x0098B2A0`) re-initialises
the graphic system's screen buffer (`cGZScreenBuffer`, object `+0x100`, created by `0x00991DA0`) when the system's
byte `+0x41` is set. The constructor sets that byte to 1 (`0x0098FDC0`). The colour type is chosen at
`0x0098B3CC`-`0x0098B3DD`:

| Screen bit depth (`d`) | Colour type passed |
|---|---|
| `d <= 16` | 7 (A4R4G4B4, 16 bpp) |
| `d > 16` | 9 (A8R8G8B8, 32 bpp) |

`d` is the dword at `+0xC` of the record that GDriver slot `0x100` fills (`0x0098B37F`, `0x0098B391`). If no
GDriver is present, `d` = 32 (`0x0098B363`). Width and height are the global screen settings `0x00B17AE4` /
`0x00B17AE8`. The call goes through the screen buffer's interface at `+0x4C` (vtable `0x00ADB388`, slot 4 =
`cGZScreenBuffer::Init` `0x00991EC0`). Init stores the type and bpp unchanged in the buffer's colour-type pair (base
`+0xC`/`+0x10`, `0x00991F79`-`0x00991F7E`). It also creates a 4 x 4 auxiliary `cGZBuffer` of the same colour type
(`+0x30`; `0x00991FA5`-`0x00991FAE`) and fills it with one colour. So in a 16-bit screen mode, both the screen buffer
and its 4 x 4 buffer have colour type 7.

**That buffer never gets an A4R4G4B4 span renderer.** `cIGZDrawSystem` slot 4 (`0x0082BAB0`) gives a buffer a draw
context by trying the registered context factories in list order: the first one whose slot 3 succeeds wins. Slot 3
(`0x0082BD30`) puts each new factory at the front of the list (`0x0082BD6B`-`0x0082BD70`). `cGZScreenBuffer::Init`
registers a `cGZDrawContextRenderToSimGLFactory` (`0x00991E50`, call at `0x00992021`). That factory comes after the
`cGZDrawContextRenderToBuffer` factory that `cGZDrawSystem::Init` registered, so it sits in front of it. Its create
method (`0x00993270`) accepts only buffers that answer IID `0xE3094DC8`, and `cGZScreenBuffer::QueryInterface`
(`0x009917B0`) answers that IID (`0x009917B4`; nothing else in the image tests it). The screen buffer therefore gets a
SimGL context, not `cGZDrawContextRenderToBuffer`. Nothing in `Init` asks for a draw context for the 4 x 4 buffer.

**Other sources checked, none of which yields type 7 for a RenderToBuffer target:**

| Source | Colour type | Note |
|---|---|---|
| `cGZBuffer` (CLSID `0xC470D325`, creator `0x008298F0`, constructor `0x008272E0`) | default 3 / 16 bpp | `Init` (`0x008269B0`) stores the caller's pair unchanged; `GetColorType` is slot `0x3C` |
| `cGZWin::create_private_buffer` -> `cIGZDrawSystem` slot 5 (`0x0082B980`) | 1 | stays 1 (bpp 0, 0-byte pixel store `0x00826A50`). The context made there is `cGZDrawContext` (`0x0082C9A0`, vtable `0x00AC16C8`), and `RenderToBuffer::Init` would reject type 1 anyway (default case `0x0083087A` releases the buffer and returns 0 for any type outside 2..9) |
| Gimex image loads (`cIGZGimexFactory` IID `0x02FFBB60`; `LoadFromFile` slot 4 arg 3 = target type) | caller's choice | literal targets found: 2 (`0x007428A3`), 9 (`0x007BFDB4`), 1 (`0x008FE752`). `TranslateGimexToGZBuffer` (`0x0090C1D4`) has a type-7 case (`TranslateGimex24ToGZBufferARGB4444Dither` `0x0090BF3C`), but no caller with target 7 was found |
| Buffer resources v1/v2 (persistence.md 6.2) | from the file header | load as the file's type, then `ConvertBufferFromFormatToFormat` (`0x00931CF0`) to the target. Factory defaults: `cGZBufferResourceFactory` 3 (`0x0097DBD2`), `cGZPersistBufferResourceFactory` 1 (`0x0097807C`). `SetColorType` (`0x0097CF60`, `0x00978097`) has no direct callers |
| Lot-configuration editor texture-to-buffer conversion (`0x00604920`) | 8 or 9 | type 3 maps to 9, anything else to 8 (`0x006049A2`-`0x006049AD`) |

**What a game-pixel test can use.** A type-7 RenderToBuffer target can be made only through `cIGZDrawSystem` slot 4.
It needs a `cIGZBuffer` that has been `Init`-ed with colour type 7 (for example a `cGZBuffer` made through GZCOM CLSID
`0xC470D325`) and that does not answer `0xE3094DC8`. The `cGZDrawContextRenderToBuffer` factory (`0x0082B910`) accepts
any object that answers `cIGZBuffer` `0x20732180` (`0x0082B91C`). None of these interfaces is in the public SDK
headers, so this route is undocumented. It works by calling the game's own COM classes and is not a game path. The
static evidence finds no shipped scenario in which the game itself sends A4R4G4B4 spans to `0x00AC185C`.

## 2. Shared conventions

**Pixel.** 16 bits, A in bits 12-15, R 8-11, G 4-7, B 0-3 (masks `0xF000/0x0F00/0x00F0/0x000F`).

**Span record** (8 bytes). Two dwords `start`, `end`, each packed as `(y << 16) | x` with unsigned 16-bit x and y.
Width is `end - start` computed on the whole dwords, so a span is one row: `end` is `start` plus the pixel count
(exclusive end). A span with width 0 writes nothing.

**Target descriptor** (argument `buffer`): dword `+0` = base address of pixel (0,0), dword `+4` = row stride in
bytes. Pixel (x, y) is at `base + y*stride + 2*x`. No clipping is done in this class: spans are assumed
pre-clipped by the caller (`DrawPreclippedSpanList`, view3d.md 7.1).

**Span list loop.** Slots 1-4 take `(spans, count, buffer, ...)` and process `count` spans in order with a
decrement-then-test loop: `count` must be at least 1 (a count of 0 is not guarded; see U1).

## 3. RenderSolid (`0x00847CE0`)

Arguments `(spans, count, buffer, colour)`; `colour` is already a 4444 pixel (low 16 bits used). For each span,
fills `width` pixels from (x, y) with `colour` (helper `0x0090FFC8`, a 16-bit fill: one leading word if the
address is not 4-byte aligned, then dword pairs, then one trailing word). The pixel is stored verbatim, alpha
nibble included.

## 4. RenderAlpha (`0x00847E40`) - constant colour over the destination

Arguments `(spans, count, buffer, colour, alpha)`.
- Weight: `a = ((alpha >> 27) + 1) >> 1` on the unsigned 32-bit `alpha` (only its top 5 bits matter), so
  `a` is 0..16. With the alpha byte in bits 24-31: `a = ((A >> 3) + 1) >> 1`; A = 0x00..0x07 gives 0,
  0xF8..0xFF gives 16.
- Per pixel, for each of R, G, B (4-bit `s` from `colour`, 4-bit `d` from the destination):
  `out = floor((16*d + a*(s - d) + 8) / 16)`. (R and B are computed together in one register as
  `((sRB - dRB)*a + 16*dRB + 0x808) >> 4`, G alone as `((sG - dG)*a + 16*dG + 128) >> 4` on the in-place
  fields; the low field never borrows into the high one because `16*d + a*(s-d) + 8` stays in 8..248.)
- The output alpha nibble is always `0xF`, whatever the destination or `colour` alpha was.
- Worked example: d = `0x8000` (R=G=B=0), colour `0x0F84`, alpha `0x80000000` (a = 8):
  R = floor((0 + 8*15 + 8)/16) = 8, G = floor((64+8)/16) = 4, B = floor((32+8)/16) = 2, result `0xF842`.

## 5. RenderGradient (`0x00847D30`) - linear colour ramp, opaque

Arguments `(spans, count, buffer, grad)`. `grad` is 12 signed dwords: `+0x00..+0x0C` start values for channels
0-3, `+0x10..+0x1C` x steps, `+0x20..+0x2C` y steps. Channels 0, 1, 2 are B, G, R in 8.16 fixed point (8-bit
value in bits 16-23); channel 3 is alpha and is ignored here.
- At each span's (x, y) the start value is `c = start + dx*x + dy*y` (32-bit wrapping integer maths).
- B: the span start is then shifted right logically by 4 and the per-pixel step is `dx_B` shifted right
  arithmetically by 4, so B accumulates with the low 4 bits of its step dropped (B drifts versus the exact ramp).
- G: start and step unchanged.
- R: start shifted left 4, then OR `0xF0000000`; step `dx_R << 4`.
- Each pixel: the 16-bit pixel is bits 16-31 of `(Racc & 0xFF000000) + (Gacc & 0x00F00000) + (Bacc & 0x000F0000)`,
  i.e. each channel is the top 4 bits of its 8-bit integer part (truncation, no rounding, no dither), and the
  alpha nibble is the top nibble of the R accumulator, which is `0xF` while R stays within 0..255.99 along the
  span. If R leaves that range mid-span the carry/borrow lands in the alpha nibble (U2).
- Then each accumulator adds its step and the next pixel (x+1) is written.

## 6. RenderAlphaGradient (`0x00847F10`) - linear colour and alpha ramp over the destination

Arguments `(spans, count, buffer, grad)`, same 12-dword layout as section 5, all four channels used.
- B, G, R accumulators are set up exactly as in section 5 (including the B >> 4 drift; R uses `<< 4` but no OR).
- Alpha accumulator: `(startA + dxA*x + dyA*y) * 0x88`, step `dxA * 0x88` (32-bit wrapping). Per pixel
  `a = acc >> 27` (unsigned), so for an 8.16 alpha value A, `a = floor(A * 136 / 2048)` = floor(A*17/256):
  0..16 over A = 0..255. This is a different alpha-to-weight map from RenderAlpha (section 4); see the table in
  section 8.
- Source colour per pixel: R = bits 24-27 of the R accumulator, G = bits 20-23 of G, B = bits 16-19 of the
  shifted B accumulator (top nibbles of the 8-bit integer parts); then the blend of section 4 with this `a`.
  Output alpha nibble `0xF`. The R accumulator's top nibble is masked off, so out-of-range R does not reach
  alpha here.

## 7. RenderBlt (`0x008480C0`) - kernel dispatch

Arguments `(p1, p2, rectA, rectB, src, mode)` (6 dwords, thiscall, `RET 0x18`; section 1.1): `rectA`, `rectB` point to 24-byte records, `src` to a 32-byte
source descriptor, `mode` an int. It copies the three records by value and calls
`kernel(p1, p2, *rectA, *rectB, *src)` (cdecl, 88 bytes of arguments), where
`kernel = table[src.format * 2 + src.flag][mode]`: `src.format` is the dword at `src+0x10` (0..6),
`src.flag` the byte at `src+0x1C` (0/1). The row-pointer array is at `0x00AC2100` (14 entries); row `r` is the
12-entry array at `0x00AC20D0 - 0x30*r`, so the 168 entries occupy `0x00AC1E60`-`0x00AC20FF`. Neither index is
range-checked (U3).

Kernel arguments as the kernels read them (`p1`, `p2` and the three records):
- `p1` = span list, `p2` = span count (same span loop as section 2).
- `rectA` (24 bytes) = destination: `+0` base, `+4` stride (bytes). `+8..+0x14` not read by any examined kernel.
- `rectB` (24 bytes) = source image: `+0` base, `+4` stride, `+8` width, `+0x0C` height (the last two read only
  by tiled kernels). `+0x10`, `+0x14` not read by any examined kernel.
- `src` (32 bytes): `+0x00` u0, `+0x04` v0, `+0x08` du/dx, `+0x0C` dv/dy, `+0x10` source format (row), `+0x14`
  constant colour/alpha (ARGB 8888), `+0x18` colour key, `+0x1C` keyed flag (byte), other bytes unread.

## 8. Kernel table: rows and columns

**Rows** (`format*2 + keyed`). The source format is identified from each row's plain `Blt` kernel (how it loads
and splits the source pixel):

| format | source pixel | read as | Mac name (RGBFormat masks) |
|---|---|---|---|
| 0 | A4R4G4B4, 16-bit | copied verbatim | `61440,3840,240,15,2,4` |
| 1 | A1R5G5B5, 16-bit | A = bit 15 | `32768,31744,992,31,2,5` |
| 2 | X1R5G5B5, 16-bit | A forced opaque | `0,31744,992,31,2,5` |
| 3 | R5G6B5, 16-bit | A forced opaque | `0,63488,2016,31,2,5` |
| 4 | A8R8G8B8, 32-bit | A = bits 24-31 | `4278190080,16711680,65280,255,4,8` |
| 5 | X8R8G8B8, 32-bit | A forced opaque | `0,16711680,65280,255,4,8` |
| 6 | A8, 8-bit (alpha only) | R = G = B = max | `255,0,0,0,1,0` |

**Columns** (`mode`): geometry x operation, `mode = 4*geometry + op`.
- geometry 0 = direct (source pixel at the same (x, y) as the destination pixel), 1 = stretch, 2 = tiled.
- op 0 = copy, 1 = constant alpha, 2 = modulate, 3 = modulate + alpha.
- Evidence: every kernel in columns 4-7 carries the extra 16.16 shifts of the stretch walk and every kernel in
  columns 8-11 a `DIV` (tile wrap); columns 1/5/9 take the alpha nibble of `src+0x14`, columns 2/3/6/7/10/11
  use the MMX multiply, columns 3/7/11 additionally blend with the destination. This matches, one for one, the
  12 Mac method names `Blt, BltConstantAlpha, BltModulate, BltModulateAlpha` and their `Stretch`/`Tiled` forms
  (Mac `cBlitter<Dst, Src, keyed>`); the Mac `BltRect` has no Windows column.
- Two table entries are outside this owner range because identical code was folded: row 0 column 4
  (`0x00833730`, in the 1555 range) and row 0 column 8 (`0x00842C50`, in the 565 range); both are a raw 16-bit
  copy that does not depend on the pixel format. So this range holds 166 kernels + 5 methods + the helper
  `0x0084BD40` = 172 functions.

## 9. Kernel semantics

### 9.1 Geometry (source texel for destination (x, y))
- Direct: source pixel `(x, y)` of `rectB` (the caller pre-offsets `rectB.base`). Verified `0x00848160`.
- Stretch: per span, `v = (dvdy*y + v0) >> 16` (unsigned), source row `base + v*stride`; `u = dudx*x + u0`, each
  pixel reads texel `u >> 16` (unsigned) then `u += dudx`. x and y are the absolute destination coordinates; no
  clamping. Verified `0x00833730`.
- Tiled: per span, `tx = (x + u0) mod width`, `ty = (y + v0) mod height` (unsigned 32-bit division; here u0, v0
  are integer pixel offsets, not 16.16); each pixel reads `(tx, ty)`, then `tx += 1` and wraps to 0 on reaching
  `width`. Verified `0x00842C50`.

### 9.2 Conversion to 4444 (copy and constant-alpha ops)
Each channel keeps the top 4 bits of the source channel (truncation, no rounding, no dither):
A1R5G5B5: A = 0xF if bit 15 else 0, R = bits 11-14, G = bits 6-9, B = bits 1-4. X1R5G5B5: same with A = 0xF.
R5G6B5: R = bits 12-15, G = bits 7-10, B = bits 1-4, A = 0xF. A8R8G8B8: A = bits 28-31, R 20-23, G 12-15,
B 4-7. X8R8G8B8: same with A = 0xF. A8: A = bits 4-7 of the byte, RGB = `0xFFF`. 4444: unchanged.
Verified at the plain `Blt` kernels `0x00849C50`, `0x0084BBD0`, `0x0084DAB0`, `0x0084F9E0`, `0x00851700`,
`0x008533A0`.

### 9.3 Conversion to 8888 (modulate ops)
Each channel is widened by bit replication: 4-bit `n` -> `n*17`; 5-bit `v` -> `(v << 3) | (v >> 2)`; 6-bit
`g` -> `(g << 2) | (g >> 4)`; 1-bit alpha -> 0xFF or 0; formats without alpha -> A = 0xFF; A8 -> A = byte,
R = G = B = 0xFF; 8888 unchanged. Verified at column 2 of rows 0, 2, 4, 6, 8, 10, 12
(`0x00848260`, `0x00849DE0`, `0x0084BDB0`, `0x0084DC20`, `0x0084FB60`, `0x00851860`, `0x008534C0`).

### 9.4 Operations
- **Copy** (op 0): write the 9.2 conversion. The destination is not read. Alpha comes from the source.
- **Constant alpha** (op 1): `a = src.colour >> 28` (top nibble of the alpha byte of `src+0x14`, 0..15). The
  source is converted per 9.2, its alpha ignored, then the helper `0x0084BD40` blends each of R, G, B:
  `out = floor((16*d + a*(s - d) + 8) / 16)`, output alpha `0xF`. Because `a` tops out at 15, a fully opaque
  constant alpha does not reproduce the source: s = 15 over d = 0 gives floor(233/16) = 14. (Contrast
  RenderAlpha, whose weight reaches 16.)
- **Modulate** (op 2): widen per 9.3 to `C`, then per byte channel (A, R, G, B)
  `M = (C * K + 128) >> 8` with `K` the matching byte of `src.colour` (MMX word multiply, rounding constant
  `0x0080` from `0x00AC1B80`); write the top nibble of each M byte (A from M too). The destination is not read.
- **Modulate + alpha** (op 3): M as above, then blend R, G, B (top nibbles of M) over the destination with
  `a = M >> 28` by the op-1 formula; output alpha `0xF`. Verified `0x00848380`.
- **Keyed rows** (odd rows): before anything else each source texel is compared with `src+0x18`; if
  `(texel & rgbmask) == key` the destination pixel is left untouched. `rgbmask` is the format's colour bits:
  4444 `0x0FFF`, 1555 and X555 `0x7FFF`, 565 `0xFFFF`, 8888 `0xFFFFFF` (`0x00848DB0`, `0x0084AB70`,
  `0x0084CAD0`, `0x0084E990`, `0x008507C0`). For A8 the mask is empty, so the test degenerates to "key == 0":
  with a zero key **nothing** is drawn, with any other key every texel is drawn (`0x00854070`, `0x00854190`).
  The key compare is present in every column of the keyed rows sampled (rows 1, 3, 13; columns 0-3, 5, 9).

### 9.5 Verification level
Fully read (round 1): the five methods, the helper, row 0 columns 0-5 and 8, the Blt kernel of every format, the
key test of column 0 in all seven keyed rows (mask per format; X8R8G8B8 `0x008524C0` masks `0xFFFFFF`), the 8888
widening of every unkeyed row.

All 168 table entries (round 2) were then checked by **executing the image bytes** against the model of 9.1-9.4,
which closes the old feature-sweep gap (U4). Method and criteria, so a successor can repeat it:
1. Byte identity. Each kernel's bytes were disassembled independently of Ghidra (Capstone, from
   `bin/SimCity4.exe`) and compared with the export's instruction list: 168 functions, 0 mismatches in
   bytes or mnemonic (Ghidra `JZ/JNZ/JC` = `je/jne/jb`); the only bytes the export omits are 91 alignment
   fillers inside loops (`lea r,[r]`, `nop`, `mov edi,edi`), which have no effect.
2. Text normalization is not enough. With in-function branch targets renamed to ordinal labels and all
   immediates, displacements and widths kept, no two kernels are textually identical, and kernels of one column
   differ from row 0 by 5-91 lines (difflib, longer side of each hunk): register allocation, instruction scheduling and stack offsets (one more or one
   fewer saved register) differ between rows, so a textual "same template, different conversion" proof fails.
   The constant inventory (AND/CMP/TEST immediates) does follow the row: 4444 rows `0xF..0xF000`, 1555/X555 add
   `0x1F 0x3E0 0x7C00` and the replication mask `0x70707`, 565 `0x7E0 0xF800` and `0x70007`, keyed rows add
   their key mask (`0xFFF`, `0x7FFF`, `0xFFFFFF`; the 565 key uses all 16 bits and A8 has no mask, so neither adds
   a constant), and only the 1555 rows hold `0x1`/`0xFF000000` (the alpha bit).
3. Execution. A small x86-32 interpreter covering exactly the 35 mnemonics the kernels use (integer ALU with
   ZF/CF, `DIV`, `IMUL`, the MMX `MOVD/MOVQ/PUNPCKLBW/PXOR/PMULLW/PADDW/PSRLW/PACKUSWB/EMMS`) runs each kernel
   with the RenderBlt argument block (cdecl, 88 bytes) and random contents: 1-4 spans of 1-25 pixels, random
   destination pixels, random texels (10 % forced to all-zero or all-one), constant colour random or one of
   `0xFFFFFFFF, 0, 0x80808080, 0xF0FFFFFF`, keys equal to a masked texel 80 % of the time (A8: key 0 half the
   time), random entry registers, random values in every field the spec calls unread (U6). Images 5x3, 37x11
   and 64x16 with row padding; stretch with u0 < 2.0, du/dv < 1.0 texel; tiled with offsets 0..999 and random
   du/dv (which tiled kernels must ignore). The result is compared pixel by pixel with 9.1-9.4, including the
   untouched pixel either side of each span.
   Result: 150 trials per kernel, **168/168 kernels, 747,927 pixels, 0 mismatches**; every kernel returns with
   ESP restored and EBX/ESI/EDI/EBP preserved. Exhaustive texel sweep (all 65,536 16-bit texels, all 256 A8
   bytes) for columns 0-3 of rows 0, 2, 4, 6, 12 and column 0 of rows 1, 3, 5, 7, 13: 25 kernels, 1,312,000 pixels, 0 mismatches (the 9.2/9.3
   conversions and the per-format key masks hold for every texel value).
4. Sensitivity. The same harness with one rule of the model changed fails at once: blend rounding `+7`
   instead of `+8` (78 kernels fail at 4 trials), constant-alpha weight `a+1` (39), modulate without `+128` (71),
   4444 key mask `0xFFFF` (all 12 keyed 4444 kernels at 40 trials), tiled start without `u0` (52).

So every rule in 9.1-9.4 is verified on all 168 kernels for the input ranges above. What execution does not
cover is listed in U5 and U7.

## 10. Unknowns

- U1. Count 0 in slots 1-4: the loop decrements before testing, so 0 would run 2^32 iterations; whether any
  caller passes 0 is not checked.
- U2. RenderGradient with R outside 0..255 along a span corrupts the alpha nibble (section 5); whether callers
  ever produce such ramps is not checked.
- U3. RenderBlt does not validate `src.format`, `src.flag` or `mode`.
- U4. (Closed in round 2.) The 144 kernels that were only feature-swept are now verified by execution against
  9.1-9.4 (section 9.5); none differs from the model.
- U5. Whether the stretch walk ever lets `u >> 16` or `v` run past the source (no clamp in the kernel) depends
  on the caller's u0/du; not checked here.
- U6. Fields `rectA+8..+0x14`, `rectB+0x10..+0x14` and the bytes of `src` beyond those listed are unread by the
  examined kernels; their meaning belongs to the caller (`cGZDrawContextRenderToBuffer`). Round 2: randomizing
  them (and u0/v0/du/dv for direct kernels, du/dv for tiled kernels, `src+0x10` and bytes `src+0x1D..0x1F`)
  never changed any kernel's output.
- U7. Not covered by the execution check: spans wider than 25 pixels or beyond x, y = 65,535; stretch steps of
  1.0 texel or more and u/v accumulators that wrap 2^32; tiled `x + u0` wrapping 2^32; zero width or height in
  tiled kernels (the `DIV` would fault); negative strides. The model's formulas are expected to hold there, but
  only by the reading of 9.1.

## 11. Coverage proposal

- specified: `nGZDrawSoftRender::cSpanRender4444` (whole owner range, 172 functions, 51,809 bytes): 5 methods +
  helper + ~24 kernels read in full; all 168 table entries (166 in range + 2 folded) verified by execution
  against 9.1-9.4 (section 9.5); remaining limits U1-U3, U5-U7.
- ABI: the class is internal to `cGZDrawContextRenderToBuffer`. Proposed `docs/abi/vtables.tsv` rows for
  `0x00AC185C` (section 1.1 table) are in lane re-spanbridge4444-4's `build-runs/abi-rows-re-spanbridge4444-4.tsv`;
  not reconciled into docs/abi.
