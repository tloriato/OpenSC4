# `cSC4WinCalloutBox`: the callout box (behaviour spec)

Scope: the whole `cSC4WinCalloutBox` owner as tools/coverage.py assigns it (24 functions, 8,194 bytes on
`5c0ff782`; inventory in §7). The box is a speech-bubble window used by:
- the city tools' hover tip (city-tools-input.md §8.4);
- the 3D view (view3d-window.md, `+0x3D8`);
- other callers of the constructor `0x00799DD0`.

Evidence: static RE only, on the instruction and data bytes of the Steam 1.1.641 exe (the same code as GOG,
`docs/lanes/re-dates.md`). Every function was disassembled; the paint routine was read on the disassembly (the
decompile mixes its frame). Written 2026-10-23 (re-callout7).

Conventions:
- All offsets are from the object (the primary interface at `+0`).
- `tw × th` is the skin cell: the skin image's width and height divided by 5 (16 × 16 without a skin).
- Rectangles are `{x0, y0, x1, y1}`, half-open.
- `dirty` is `+0x12C`: bit 0 content, bit 1 layout, bit 2 composition.

## 1. Identity and fields *[compat]*

| Offset | vtable | Notes |
|---|---|---|
| `+0x00` | `0x00AB69D0` | the callout interface (IID `0xC9B432CF`), §2 |
| `+0x04` | `0x00AB6770` | `cGZWin`: 0 QI thunk (`0x00799F30`), 4 `Init`, 5 `Shutdown`, 88 `GZPaint`, 148 deleting destructor thunk (`0x00799F40`) |

The QI (`0x00797F50`) answers `0xC9B432CF` itself and passes other IIDs to `cGZWin`'s at `+4`.

**Constructor** (`0x00799DD0`):
- `cGZWin` at `+4` and both vtables;
- two empty strings, `+0xF4` and `+0x108`;
- the offset rectangle `+0x11C` := `{−28, −28, 28, 28}`;
- the side `+0x164` := 4 (none);
- the `cGZWin` flags `+0xCC` get `0x200000`;
- everything else 0.

| Offset | Field |
|---|---|
| `+0xDC` | the bounds the box must stay in |
| `+0xEC`, `+0xF0` | the point the arrow points at |
| `+0xF4`, `+0x108` | the title and the body text |
| `+0x11C` | the offset rectangle: how far the box keeps from the point on each side |
| `+0x12C` | `dirty` |
| `+0x130` | the skin image, a 5 × 5 grid of cells |
| `+0x134` | the icon |
| `+0x138`, `+0x13C` | the title and body fonts |
| `+0x140` | the meter image |
| `+0x144`, `+0x148` | the text surface and the box surface |
| `+0x14C`, `+0x150` | the horizontal and vertical padding |
| `+0x154`, `+0x158` | the text block's width and height |
| `+0x15C`, `+0x160` | the content's width and height |
| `+0x164` | byte: the side, 0 left of the point, 1 above, 2 right, 3 below, 4 none |
| `+0x168` | the window rectangle (box and arrow) |
| `+0x178` | the box rectangle |
| `+0x188` | the arrow rectangle |
| `+0x198` | the meter rectangle in the text block |
| `+0x1A8` | the meter's repeat count |
| `+0x1AC` | the split between the title and body bands |
| `+0x1B0` | float: the meter's fill, 0–1 |

**`Init`** (`0x007980D0`, on `+4`), once:
1. `SetID(0x2AAB8CC1)`, `SetFlag(0x10000, 0)`, `SetFlag(4, 1)`, `cGZWin::Init`.
2. A missing font is set to `0x68963C4C` (slots 11 and 10).
3. With empty bounds (zero width), the bounds become the parent's area inset by 16 on each side, through slots 47,
   50 and 54 *[inferred: the area moved into this window's coordinates]*.

**`Shutdown`** (`0x00798220`), when initialised:
- releases `+0x130`, `+0x134`, `+0x144`, `+0x148`, `+0x13C` and `+0x138`, then runs `cGZWin::Shutdown`;
- `+0x140` is kept for the destructor.

**Destructor** (`0x00799F50`): `Shutdown`; release `+0x148`…`+0x130`; free both strings; `cGZWin`'s destructor.
The deleting one is `0x0079A020`.

## 2. The callout interface (`0x00AB69D0`) *[compat]*

Every setter invalidates the window (`cGZWin` slot 92) after changing `dirty`.

| Slot | Address | Effect |
|---|---|---|
| 4 | `0x007982C0` | `(skin, padX, padY)`: the skin goes in `+0x130` (counted) with colour key `rgb(255, 0, 255)` (its slot 23); `dirty` gets `3`; `+0x14C` := `padX` (8 when negative), `+0x150` := `padY` (3 when negative) |
| 6 | `0x00797F80` | the title: unless equal (string slot 10 with 1), assigned (slot 13); `dirty` gets `1` |
| 7 | `0x00797FD0` | `&+0xF4` |
| 8 | `0x00797FE0` | the body, as slot 6, into `+0x108` |
| 10 | `0x00798380` | the title font: `RZ::WCM` slot 5 `(id)` into `+0x138`; `dirty` gets `1` |
| 11 | `0x007983E0` | the body font, into `+0x13C` |
| 12 | `0x00798440` | the icon into `+0x134`, colour key magenta; `dirty` gets `1` |
| 14 | `0x007984D0` | `(meter, count, fill)`: `fill` is clamped to 0–1. The meter goes in `+0x140` (magenta key), `dirty` gets `1`. A changed `count` goes in `+0x1A8` (`dirty` gets `1`); a changed `fill` goes in `+0x1B0` (invalidate only) |
| 15 | `0x007985F0` | the point, when changed; `dirty` gets `2` |
| 17 | `0x00798630` | the offset rectangle, when changed; `dirty` gets `2` |
| 18 | `0x00798050` | `&+0x11C` |
| 19 | `0x007986A0` | the bounds, when changed; `dirty` gets `2` |
| 20 | `0x00798060` | `&+0xDC` |

Slots 0–3, 5, 9, 13 and 16 are shared bodies. Two small helpers:
- `0x00798070`: point equality;
- `0x00798090`: a rectangle moved by a point.

## 3. Painting: `GZPaint` (`0x00798710`, `cGZWin` slot 88)

### 3.1 Content (`dirty` bit 0)

1. `maxH` = the bounds' height minus `2·padX`: the text height budget uses horizontal
   padding *[quirk]* (`0x00798710`).
2. **Title**, when not empty: the title font's slot 45 `(text, length, &r1, 0, 250, maxH)` measures it.
   *[compat]* The final three arguments mean minimum width, maximum width and maximum height:
   the width limit is 250 pixels; the bounds-derived value limits height (`0x008973D4`; WCB1).
   Width := `r1.w + 4`, height := `r1.h`.
3. `+0x1AC` := height + `padY` + 3.
4. **Body**, when not empty: the body font's slot 45 measures `r2`. Width := max(width, `r2.w`). The height grows by
   4 when it was positive, then by `r2.h`.
5. **Meter** `+0x140`, with `w × h` its image:
   - `mW` = `w × count`. Width := max(width, `mW`); height += `h/2 + 8`;
   - the meter rectangle is `{(width − mW)/2, r1.h + 4}` with size `mW × h/2`.
   - Without a meter the rectangle is zero.
6. **Sizes.** Content width `+0x15C` = text width + `2·padX` + the icon's width + 4 (0 without an icon). Content
   height `+0x160` = max(text height, icon height) + `2·padY`. The text height then grows by 2.
7. **The text surface** `+0x144`:
   - It is remade (graphics system slot 3) when its size differs from the **content** size, then sized to the
     **text** size (slot 3 with format `{1, 0}`). Since the two sizes differ whenever there is padding or an
     icon, it is remade on every content change *[quirk]*.
   - It is locked with `0x8010`. This function never balances that lock. *[compat]* Font slots
     55/56 bracket their own renderer access: on the outermost font-use entry/exit they call
     the renderer's lock/unlock (`0x00896B8E`, `0x00896BC2`). The default renderer adds and
     removes a separate `0x8080` surface lock (`0x00896232`, `0x00896243`). For the standard
     buffer, each successful lock increments its nesting count and each unlock decrements it
     once (`0x00826AA0`, `0x00826490`); the font pair therefore leaves the callout's original
     nesting level outstanding. It does not release the earlier `0x8010` lock (WCB3).
8. **Background**, with a skin. `s` = `+0x1AC − (content height − text height)/2` splits the surface:
   - rows above `s` are tiled from cell (1, 1) `{tw, th, 2tw, 2th}`;
   - rows below from cell (1, 2) `{tw, 2th, 2tw, 3th}`.
9. **Text**, when the title font's slot 55 `(surface)` succeeds, in black (surface slot 30):
   - the title font's slot 52 `(surface, &r1 at the origin, text, length, 0, black, 0x8000000, 0, 0)`;
   - the body below it: `r2` moved down by `r1.h + 4` when there is a title, and by the meter's height + 4 when
     there is a meter. It is drawn with the body font.

   Then the title font's slot 56 `(surface)`.

   *[compat]* Slot 52 draws a paragraph, wrapping lines to the supplied rectangle width;
   alignment 0 places each line at the left edge (`0x0089A35B`). Its `0x08000000` argument is
   the background-colour sentinel passed to line drawing, not a wrapping flag. In palette
   mode with palette-background mode 2, black foreground selects white as the palette
   background (`0x0089A099`). Slots 55/56 are font-use lock/unlock calls (§3.1 step 7; WCB1).
10. `dirty` gets `6`.

### 3.2 Layout (`dirty` bit 1)

`dl`, `dr`, `dt`, `db` are the point's distances to the bounds' left, right, top and bottom. `W × H` is the content
size and `O` the offset rectangle. The side:

| Test, in order | Side |
|---|---|
| `dl < tw`, `dt > tw`, `db > tw` | 2 (right of the point) |
| `dr < tw`, `dt > tw`, `db > tw` | 0 (left) |
| `dt > H − O.y0` | 1 (above) |
| `db > H + O.y1` | 3 (below) |
| `dl > W − O.x0` | 0 |
| otherwise | 2 when `dr > W + O.x1`, else 1 |

The vertical tests of the first two rows use `tw`, not `th`. A new side sets `dirty` bit 2. Then, by side, with
`clamp(v, a, b)` taking `a` below `a` and `b` above `b`:

| Side | Minimum size | Box | Window | Arrow (`x0`, `y0`, size) |
|---|---|---|---|---|
| 1 above | `W ≥ 3tw`, `H ≥ 3tw/2` | `x0 = clamp(P.x − W/2, B.x0, B.x1 − W)`, `y0 = P.y + O.y0 − H` | box with `y1 += 3th/2` | `ax − tw/2`, `box.y1 − th/2`, `tw × 2th`; `ax = clamp(P.x, box.x0 + 1.5tw, box.x1 − 1.5tw)` |
| 3 below | as 1 | `x0` as 1, `y0 = P.y + O.y1` | box with `y0 −= 3th/2` | `ax − tw/2`, `box.y0 − 1.5th`, `tw × 2th` |
| 0 left | `H ≥ 2tw` | `x0 = P.x + O.x0 − W`, `y0 = clamp(P.y − H/2, B.y0, B.y1 − H)` | box with `x1 += 3tw/2` | `box.x1 − tw/2`, `ay − tw/2`, `2tw × th`; `ay = clamp(P.y, box.y0 + th, box.y1 − th)` |
| 2 right | `H ≥ 2tw` | `x0 = P.x + O.x1`, `y0` as 0 | box with `x0 −= 3tw/2` | `box.x0 + tw/2 − 2tw`, `ay − tw/2`, `2tw × th` |

The box is `W × H` from its `x0`, `y0`. A changed arrow rectangle sets `dirty` bit 2.

### 3.3 Composition (`dirty` bit 2)

Rectangles are taken relative to the window rectangle.

1. The box surface `+0x148` is remade at the window's size when that differs. It is locked (`0x8010`) and cleared
   (slot 18 `(0, null)`).
2. **With a skin**, columns 0, 1 and 3 and rows 0–3 make the frame:

   | Part | Cell |
   |---|---|
   | corners (slot 29) | `{0, 0}`, `{3tw, 0}`, `{0, 3th}`, `{3tw, 3th}` |
   | top and bottom edges (tiled) | `{tw, 0}`, `{tw, 3th}` |
   | the title band's left, right and centre (tiled) | `{0, th}`, `{3tw, th}`, `{tw, th}` |
   | the body band's left, right and centre (tiled) | `{0, 2th}`, `{3tw, 2th}`, `{tw, 2th}` |

   - The title band runs from `th` below the box top to `+0x1AC`. The body band runs from there to `th` above the
     bottom.
   - The arrow cell is copied onto the arrow rectangle (slot 29):
     - side 0: `{2tw, 4th, 4tw, 5th}`;
     - side 2: `{0, 4th, 2tw, 5th}`;
     - side 1: `{4tw, 2th, 5tw, 4th}`;
     - side 3: `{4tw, 0, 5tw, 2th}`.
3. The text surface is copied to `x = box.x0 + padX + icon width + 4`, centred vertically in the box.
4. Unlock.

### 3.4 Drawing (every paint)

`dirty` := 0. *[compat]* The offset `o` is minus the **absolute** left/top of the parent.
Window slot 50 returns its absolute area (`0x0099BCE8`, rectangle beginning at window
`+0x14`); the callout takes the first two coordinates (`0x00798710`, drawing at
`0x00799B0B`). Thus the box, icon and meter rectangles are translated from screen coordinates
into the parent's drawing coordinates (WCB2). With the draw context (`cGZWin` `+0x6C`):
1. The box surface at the window rectangle + `o` (slot 38).
2. The icon at `(box.x0 + padX, box.y0 + (box height − icon height)/2)` + `o` (slot 38).
3. **The meter.** Its rectangle is moved by `(box.x0 + padX + icon width + 4, box.y0 + padX)` + `o`. The vertical
   offset is `padX` too *[quirk]*.
   1. The image's bottom half (`{0, h/2, w, h}`) is tiled over the whole rectangle (slot 39).
   2. Its top half (`{0, 0, w, h/2}`) over the first `trunc(width × fill)` pixels.
4. True.

## 4. Quirks

- The maximum text height is the bounds' height minus twice the horizontal padding; text width is limited to 250 pixels (§3.1).
- The text surface is remade on every content change, and it is never unlocked.
- The layout's near-edge tests use the cell width for vertical distances.
- The meter is placed with the horizontal padding on both axes.

## 5. Cases

| Case | Setup | Expected |
|---|---|---|
| CB-C1 | skin 80 × 80 | `tw = th = 16` |
| CB-C2 | bounds `{0, 0, 800, 600}`, point `(5, 300)` | side 2: the box on the right, arrow cell `{0, 64, 32, 80}` |
| CB-C3 | point `(400, 300)`, content 120 × 40, offset `{−28, −28, 28, 28}` | `dt = 300 > 40 + 28`: side 1, box `{340, 232, 460, 272}` |
| CB-C4 | meter 10 px wide, count 5, fill 0.5 | 50 px bar, the top half drawn over the first 25 px |
| CB-C5 | `SetSkin(img, −1, −1)` | padding 8 × 3 |

## 6. Not covered

- Font behavior beyond the measured/drawn paragraph and font-use locking contracts in §3.1; the remaining surface and draw-context operations.
- The callers, which set the texts and the point (city-tools-input.md, view3d-window.md).

## 7. Function inventory (owner `cSC4WinCalloutBox`)

All 24 functions (8,194 bytes):
- §1: `0x00797F50`, `0x007980D0`, `0x00798220`, `0x00799DD0`, `0x00799F30`, `0x00799F40`, `0x00799F50`,
  `0x0079A020`;
- §2: `0x007982C0`, `0x00797F80`, `0x00797FD0`, `0x00797FE0`, `0x00798380`, `0x007983E0`, `0x00798440`, `0x007984D0`,
  `0x007985F0`, `0x00798630`, `0x00798050`, `0x007986A0`, `0x00798060`, `0x00798070`, `0x00798090`;
- §3: `0x00798710`.
