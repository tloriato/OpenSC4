# Span renderer, 8-bit (`nGZDrawSoftRender::cSpanRender8`)

Owner range `0x0085E640`–`0x00864B58` (130 functions): the five methods of vtable `0x00AC188C` (slot 0 is the
shared base destructor `0x0082FF00`, not owned here) and 125 blitter bodies `0x0085E7F0`–`0x00864A90` reached only
through the RenderBlt table. This is the span back end that `cGZDrawContextRenderToBuffer::Init` (`0x00830710`)
creates into `+0xB0` for buffer colour type **2 (8 bits per pixel)** only (Init's jump table `0x008308B0`, entry for
type 2 → `0x008307D4`, which stores vtable `0x00AC188C`). Everything below is derived from the 8-bit bytes and
re-checked by the proofs of §8; the 565/1555/4444/8888 siblings are cited only for the shared caller.

Status legend: **[exact]** = every instruction read and restated; **[verified]** = checked against the shipped
bytes by execution under an x86 emulator (§8). Every function of the owner range is [exact] or [verified].

**What a pixel is.** One byte per pixel (`u8`). This class never interprets the byte as a palette index or as a
colour: it stores bytes it is given (§2, §3), the top byte of a fixed-point ramp (§4), or a value derived **only from
the source texel's alpha** and the draw colour's alpha byte (§7). No RGB channel of any source or of the draw colour
is ever read by a blit body, and no blit body reads the destination (a keyed pixel is simply skipped). So the
blit and gradient paths behave as if the buffer were an **alpha (coverage) plane**; the solid paths store whatever
value the caller mapped (SR8-1).

## 1. Common inputs [exact]

- **Span list.** `n` records of 8 bytes: `+0` start, `+4` end, each a packed point (x low 16 bits, y high 16 bits,
  unsigned). Pixel count = `end − start` as a 32-bit difference: valid only on one row with `end.x ≥ start.x`;
  pixels `start.x … end.x − 1` are written left to right. A zero-length span writes nothing (every method and body
  tests it or uses a zero repeat count).
- **Count `n`.** Every method and body loops `do … while (--n)`: `n = 0` is **not** a no-op (wraps, walks 2³²
  records). *[compat: callers never pass 0; the original crashes]*
- **Destination record** (24 bytes; methods take a pointer, bodies a copy): `+0` pointer to pixel (0, 0), `+4` row
  stride in bytes (32-bit multiply, a negative stride works), `+8` width, `+0xC` height, `+0x10` colour type, `+0x14`
  bytes per pixel. Only `+0` and `+4` are read: **no clipping, no bounds check**. Pixel address =
  `base + y × stride + x`.
- **Order and overlap.** Spans in list order, each fully before the next, pixels in ascending x; each pixel's
  texel is read and its byte written before the next pixel is touched, so a pixel covered twice is processed twice and
  a source aliased onto the destination behaves exactly as an ascending byte-by-byte copy (proved for all 168 slots,
  `span8_alias.py`). The fill bodies (§7.3) store with `rep stosd` / `rep stosb`; they read nothing, so this is
  observably `count` stores of `0xFF`.

## 2. RenderSolid (`0x0085E640`, slot 1) [exact, verified]

Arguments: spans, n, destination, colour (dword); `ret 0x10`. Every span pixel receives the **low byte of the
colour** verbatim (bits 8–31 ignored); stored as dwords of four copies, then the remaining 0–3 bytes. No conversion.
Example: colour `0x12345678` → every pixel `0x78`.

## 3. RenderAlpha (`0x0085E720`, slot 3) [exact, verified]

Arguments: spans, n, destination, `src` (dword), `argb` (dword); `ret 0x14`. It **ignores `argb` entirely** and
makes the virtual call `this->vtable[1](spans, n, destination, src)`, i.e. RenderSolid (§2) for this vtable: no
blending happens. Example: `src = 0x000000AB`, `argb = 0x10FFFFFF` → every pixel `0xAB`.

## 4. RenderGradient (`0x0085E6A0`, slot 2) [exact, verified]

Arguments: spans, n, destination, `g` (pointer to the 0x30-byte gradient record at `ctx+0x44`: `+0x00..+0x0C`
start values `c0..c3`, `+0x10..+0x1C` per-x steps, `+0x20..+0x2C` per-y steps; producer `SetGradientTransform`
`0x0082FDB0`, see `span-render-565.md` §4.1); `ret 0x10`. **Only the fourth channel is read**: `c3` (`+0x0C`),
`dx3` (`+0x1C`), `dy3` (`+0x2C`) — the channel the 565 renderer uses as alpha weight. At the span start the code
forms `V = c3 + dy3·y + dx3·start.x` (32-bit wrapping), stores **bits 16–23 of `V`** (`(V >> 16) & 0xFF`), then adds
`dx3` per pixel; this equals `(c3 + dx3·x + dy3·y) >> 16 & 0xFF` exactly for every pixel. Channels 0–2 are ignored.
Side effect: the method overwrites its own fourth argument slot on the stack with `dx3` (caller-owned scratch, no
observable effect for a C caller).

Example: `c3 = 0x00FF8000` (the producer's value for alpha 1.0), `dx3 = −0x00010000`, `dy3 = 0`: pixels from x = 0
are `0xFF, 0xFE, 0xFD, …`.

## 5. RenderAlphaGradient (`0x0085E740`, slot 4) [exact, verified]

Arguments as §4. The body is a tail jump to `this->vtable[2]` with the same stack, i.e. RenderGradient (§4) for
this vtable; `ret 0x10` is RenderGradient's. No blending.

## 6. RenderBlt (`0x0085E750`, slot 5) [exact, verified]

Arguments (`ret 0x18`): spans, n, `dst` (pointer, 24 bytes), `src` (pointer, 24 bytes), `b` (pointer, 32 bytes),
`kind`. Copies the three records onto the stack and calls `row[kind](spans, n, *dst, *src, *b)` (cdecl, 0x58 bytes
of arguments, caller pops) with `row = rows[byte b+0x1C + 2 × dword b+0x10]`; `rows` is the 14-pointer array at
`0x00AC26B0` (row `i` at `0x00AC2680 − 0x30·i`, 12 entries each, block `0x00AC2410`–`0x00AC26AF`). Neither index
is range-checked.

`src` record: `+0` pointer to texel (0, 0), `+4` stride in bytes, `+8` width W, `+0xC` height H (`+0x10`, `+0x14`
not read). `b` record: `+0` u0, `+4` v0, `+8` du, `+0xC` dv, `+0x10` source-format index `f`, `+0x14` ARGB draw
colour (only byte 3, `ca`, is ever used), `+0x18` colour key (dword), `+0x1C` key flag (byte).

The caller (`DrawPreclippedSpanList`, `0x008302E0`) is shared with the other span renderers and is specified in
`span-render-565.md` §6.1: format index from the texture colour type (A4444 → 0, A1R5G5B5 → 1, R5G5B5 → 2,
R5G6B5 → 3, A8R8G8B8 → 4, R8G8B8 → 5, 8-bit → 6), key masked per format, the key flag forced to 0 for format 6 (so
row 13 is unreachable), `b+0x14 = ctx+0xB4`, `kind = alpha + 2·modulate | 8 tiled | 4 stretch` (tiling wins), and
the plain/stretch/tiled pointer and origin set-up. The solid colour `ctx+0xC4` of §2/§3 is written by
`SetColorTable` (`0x0082FF70`) as the result of the target buffer's vfunc `+0x78(r, g, b)` on the ARGB colour (the
raw ARGB goes to `ctx+0xB4`); that mapping belongs to the buffer class, not to this owner (SR8-1).

## 7. The 8-bit dispatch table (row = `2·f + key`, column = `kind`) [verified, all 168 slots]

Columns: 0 `Blt`, 1 `BltConstantAlpha`, 2 `BltModulate`, 3 `BltModulateAlpha`, then the same four `Stretch` (4–7)
and `Tiled` (8–11). `F` = fill body `0x0085FCB0` (plain and stretch), `T` = tiled fill body `0x00861BB0` (§7.3).

| Row | Source, key | Columns 0–11 (`0x008…`) |
|---|---|---|
| 0 | A4444, off | 5E7F0 F 5E870 5E950 5EA30 F 5EAD0 5EBD0 5ECD0 T 5ED80 5EEA0 |
| 1 | A4444, on | 5EFC0 5F050 5F0D0 5F1D0 5F2D0 5F380 5F410 5F520 5F630 5F6E0 5F780 5F8B0 |
| 2 | A1555, off | 5F9E0 F 5FA60 5FB40 5FC20 F 5FD00 5FE20 5FF30 T 5FFC0 600D0 |
| 3 | A1555, on | 601E0 60260 602E0 603E0 604E0 60580 60610 60730 60850 61510 60900 60A30 |
| 4 | X555, off | F F 60B50 60C20 F F 60CF0 60E00 T T 60F00 61010 |
| 5 | X555, on | 60260 60260 61110 61200 60580 60580 612F0 61400 61510 61510 615B0 616E0 |
| 6 | R565, off | F F 617F0 618D0 F F 619B0 61AB0 T T 61C30 61D50 |
| 7 | R565, on | 61E70 61E70 61EE0 61FE0 620E0 620E0 62170 62280 62390 62390 62420 62560 |
| 8 | A8888, off | 62690 F 62700 627B0 62850 F 628E0 629A0 62A60 T 62AF0 62BD0 |
| 9 | A8888, on | 62C90 637B0 62D10 62DC0 62E70 63990 62F10 62FE0 630B0 63BC0 63150 63230 |
| 10 | X888, off | F F 63310 633C0 F F 63470 63540 T T 63610 636E0 |
| 11 | X888, on | 637B0 637B0 63830 638E0 63990 63990 63A20 63AF0 63BC0 63BC0 63C50 63D40 |
| 12 | 8-bit, off | 63E10 F 63E70 63F20 63FD0 F 64050 64120 641F0 T 64270 64350 |
| 13 | 8-bit, on (unreachable) | 64420 64480 644D0 64580 64630 646C0 64710 647E0 648B0 64940 649B0 64A90 |

125 distinct bodies, all in the owner range (no identical-code folding with other owners). Bodies shared across
slots compute the same thing because the slots agree under the rules below (e.g. a keyed X555 `Blt` and a keyed
A1555 `ConstantAlpha` both store `0xFF` on every non-keyed pixel, body `0x00860260`).

### 7.1 Body calling convention

cdecl `(spans, n, dst, src, b)` with the three records by value (0x58 bytes); spans walked as §1.

### 7.2 Source texel → alpha `A` (the only texel property any body uses)

| f | Source | `A` |
|---|---|---|
| 0 | A4444 | top nibble × 17 (`a<<4 | a`): `0x8F00` → `0x88` |
| 1 | A1555 | bit 15 set → `0xFF`, else `0x00` (`0x7FFF` → `0x00`, `0x8000` → `0xFF`) |
| 2, 3 | X555, R565 | `0xFF` |
| 4 | A8888 | byte 3 (`0x80123456` → `0x80`) |
| 5 | X888 | `0xFF` |
| 6 | 8-bit | the texel itself |

The RGB bits never matter. For f = 2, 3, 5 the texel is read only for the key test (odd rows); in the even rows
those `Blt` / `ConstantAlpha` slots are the fill bodies and read nothing.

### 7.3 Columns (kind & 3)

Let `ca` = byte 3 of `b+0x14`.

- **Blt** (0, 4, 8): store `A`.
- **ConstantAlpha** (1, 5, 9): store `0xFF`, whatever the texel and `ca`.
- **Modulate** (2, 6, 10): store `(A × ca + 128) >> 8` (MMX; in the 8-bit-source bodies the texel is widened to the dword `A<<24 | 0x00FFFFFF`,
  `pmullw` with the unpacked colour, `paddw` `0x0080` (`0x00AC1B80`), `psrlw 8`, `packuswb`, top byte kept; the
  result never exceeds 254, so the saturation never acts). Examples: opaque source (`A = 0xFF`) with `ca = 0xFF` →
  **`0xFE`**, not `0xFF`; `ca = 0x80` → `0x80`; 8-bit texel `0x40`, `ca = 0xC0` → `0x30`.
- **ModulateAlpha** (3, 7, 11): store `0xFF`. The bodies compute the Modulate value and **discard it** (e.g.
  `0x00863F20` stores the immediate `0xFF`).
- **Fill bodies.** `F` (`0x0085FCB0`) and `T` (`0x00861BB0`) store `0xFF` over every span pixel and read neither the
  source nor the destination; `T` still evaluates `(start.x + u0) mod W` per span (unsigned `div`), so `W = 0`
  faults there even though no texel is read.

### 7.4 Key (odd rows)

With the key flag set, the pixel is **left untouched** when `(texel & mask_f) == key`, comparing against the full
32-bit key dword: masks `0x0FFF` (A4444), `0x7FFF` (A1555, X555), `0xFFFF` (R565), `0x00FFFFFF` (A8888, X888). A
key with bits outside the mask therefore never matches (the caller always passes it masked). Otherwise the column
rule of §7.3 applies — including `ConstantAlpha` / `ModulateAlpha`, which store `0xFF` only on non-keyed pixels.
Row 13 (8-bit, keyed; unreachable) compares **no texel bits**: it tests `key ≠ 0`, drawing every pixel when the key
is non-zero and none when it is 0.

### 7.5 Addressing (columns 0–3 plain, 4–7 stretch, 8–11 tiled)

- **Plain**: texel (x, y), x and y the absolute destination pixel (the caller has biased the source pointer):
  address `src + y·stride + x·bpp`, bpp 2 for f ≤ 3, 4 for f = 4, 5, 1 for f = 6.
- **Stretch**: texel (`(u0 + du·x) >> 16`, `(v0 + dv·y) >> 16`), 32-bit wrapping sums, **logical** shifts (no
  `sar` occurs anywhere in the owner range). No bounds check.
- **Tiled**: per span `sx = ((start.x + u0) mod 2³²) mod W`, `sy = ((y + v0) mod 2³²) mod H` (unsigned `div`, 82 in
  the owner), then `sx` steps by 1 and wraps to 0 at W. `W = 0`, or `H = 0` in a body that reads a texel, faults.
  Row 13 tiled `ConstantAlpha` (`0x00864940`) divides by W only.

## 8. Proofs and how to rerun them

Scripts in `docs/specs/proofs/` (need `unicorn` — `pip install --target <worktree>/build-runs/pylib unicorn` is
picked up automatically — and the GOG 1.1.641 `SimCity4.exe`; `SC4EXE` overrides the path). They load the PE image
into the emulator and run the shipped bytes against the rules above (model: `span8_model.py`).

| Script | What it checks | Result (2026-10-21) |
|---|---|---|
| `span8_emu.py` | prints the 14×12 table at `0x00AC26B0` | matches §7 |
| `span8_methods.py 300` | §2–§5 on random spans, colours and full-range gradient records; bytes popped; RenderBlt reaches `rows[key + 2f][kind]` (key flag read as a byte) | 5/5 PASS 300 |
| `span8_blt.py 300` | all 168 slots: random sources, colours, keys (incl. near-miss texels and unmasked keys), strides, stretch steps, tiled origins (incl. 32-bit u0/v0) | 168/168 |
| `span8_blt.py via 100` | the same through RenderBlt with records by pointer | 168/168 |
| `span8_sweep.py 4` | every 16-bit texel (256 for 8-bit, 65 536 random 32-bit with alpha 0/255) per slot, 4 colours | 168/168 |
| `span8_alias.py 200` | source aliased onto the destination (§1 overlap rule) | 168/168 |

## 9. Open points

- **SR8-1** (§2, §3, §6): what the solid colour byte is for a type-2 buffer — the result of the target buffer's
  colour-mapping vfunc `+0x78` called by `SetColorTable` `0x0082FF70` (palette index, grey level or alpha) — and so
  whether solid fills and the alpha-plane values of §4/§7 mean the same thing. Needs the type-2 buffer class.
- **SR8-2** (whole spec): which shipped path, if any, creates a colour-type 2 render target (`Init` `0x00830710`
  with an 8-bit buffer); no caller analysis was done here. The gradient paths additionally inherit SR565-1 (no
  direct caller of `SetGradientTransform`).
- Behaviour with records the caller never produces (stretch reads past the texture, `W = 0`, row 13, `n = 0`) is
  stated above for completeness but has no compatibility weight.
