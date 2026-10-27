# `cGZWinSlider` and the old scroll bar `cGZWinScrollbar` (behaviour spec)

Scope: the owners `cGZWinSlider` (35 functions, 3,398 bytes) and `cGZWinScrollbar` (15 functions, 1,824 bytes) as
tools/coverage.py assigns them on `bb641878` (inventory in §6). The slider is the framework's value slider: a
track and a thumb, drawn from a 4-frame image or as flat rectangles. The **old** scroll bar derives from it and adds
two arrows, line and page steps and auto-repeat; it is a different class from the image scroll bar
`cGZWinScrollbar2` (widget-scrollbar.md). Ten of the scroll bar's methods sit in the `cGZWinSlider` owner (they
are listed under it in §6); the scroll bar's bitmap paint (`0x009C40F4`) is counted under `cGZDrawContext`
(gz-graphics.md §3 calls it an unrelated routine) and is described in §3.5 for completeness. The shared interface
stubs (`0x009C3EAD`–`0x009C40D0`, `<shared>`) are described with the interfaces but are not claimed.

Related:
- widget-toolkit.md: the `cGZWin` base (fields §2, flags §3: `0x4000` initialised, capture §6, `SendMsg` §7);
- widget-ctrlmgr.md §3.5: `CreateSlider` and `CreateScrollbar`, the usual way both are made;
- widget-comdirector.md §2: the class IDs `0x21325208` (slider) and `0x61325A2E` (old scroll bar);
- widget-wingen.md §5.3: the draw-context slots 21, 34, 35, 38 and 39 (WGN1);
- timers.md §1: the scroll bar's `cRZTimer`.

Evidence: static RE only, on the instruction and data bytes of the Steam 1.1.641 exe (the same code as GOG,
`docs/lanes/re-dates.md`). Every function was read in the decompile; the value and fraction setters (x87 tests),
the point-to-fraction mapping, the thumb placement, both paints and the scroll bar's interface stubs were checked on
the instructions. The vtables `0x00AE04D8` (slider window), `0x00AE049C` (`cIGZWinSlider`), `0x00AE0810` (scroll
bar window), `0x00AE07D8` (its `cIGZWinSlider`) and `0x00AE0788` (`cIGZWinScrollbar`) were read and compared with
the `cGZWin` table `0x00ADC8D8`. Floats: `0x00A84D2C` = 0.5, `0x00A81054` = 0.0, `0x00A81228` = 1.0, `0x00A80AA8`
= 2³² (the unsigned-conversion fix-up). Written 2026-10-24 (re-gzslider7).

Conventions:
- Offsets are from the window start. "Window slot n" is a `cIGZWin` slot (or one of the slots the two classes add,
  §2.3 and §3.2); "slot n" alone is a slot of the interface the section is about.
- `W`, `H` are window slots 41 and 42; the draw area is `+0x24`…`+0x30` (left, top, right, bottom).
- Orientation `o` (`+0xDC`): 1 horizontal, 2 vertical (the default). `range = max − min`. `f` is the fraction
  (`+0xE8`). `tw`, `th` are the thumb width and height (window slots 154, 155). Divisions written `/` truncate toward
  zero (C integer division); `trunc` is `_ftol2` (truncation).
- "Notify" = `SendMsg(+0x4C, 3, 0x887113A3, GetID(), 0)` (window slot 144; the `0` is a push left before the
  argument-less `GetID`, window slot 63). "Redraw" = window slot 92.

## 1. Identity and fields *[compat]*

### 1.1 Slider

- **Factory** `CreateNewObject` (`0x009C38B4`): `0x124` bytes, constructor `0x009C3836`; returns the window (null on
  failure). Class `0x21325208`.
- **`QueryInterface`** (`0x009C2E0E`): `0x21325207` (`cIGZWinSlider`) → `+0xD8` (null for null), AddRef'd, true;
  else `cGZWin::QueryInterface`. The interface's slot 0 (`0x009C32F3`) is the thunk.
- **Window overrides**: 0, 4 `Init`, 5 `Shutdown`, 55 `SetArea`, 88 `GZPaint`, 134 left down, 136 left up, 138
  mouse move, 148 destructor; it adds slots 151–156 (§2.3). Slots 149, 150 are `cGZWin`'s.
- **Constructor**: `cGZWin` constructor, the values below, window flag `0x8000` cleared (a `cGZWin` default), then
  its own `SetArea(0, 0, 32, 96)` (which lays out, §2.3).

| Offset | Meaning | Constructor value |
|---|---|---|
| `+0xD8` | `cIGZWinSlider` vtable | `0x00AE049C` |
| `+0xDC` | orientation `o` | 2 |
| `+0xE0`, `+0xE4` | minimum, maximum | 0, 32 |
| `+0xE8` | fraction `f` (float) | 0.0 |
| `+0xEC` | track start `lo` (thumb offset at `f` = 0) | 0 |
| `+0xF0` | thumb offset along the bar | 0 |
| `+0xF4` | track end `hi` (thumb offset at `f` = 1) | 0 |
| `+0xF8` | margin `m` (no function of either class writes it after construction) | 0 |
| `+0xFC`…`+0x108` | thumb rectangle (left, top, right, bottom; window coordinates) | — (set by the layout) |
| `+0x10C` | re-entrancy guard of the two setters (byte) | 0 |
| `+0x110` | image (a `cIGZBuffer`, counted) | null |
| `+0x114`…`+0x120` | frame rectangle `F` (§2.5) | — |

### 1.2 Old scroll bar

- **Factory** (`0x009C471E`): `0x150` bytes, constructor `0x009C46AF`; returns the `cIGZWinScrollbar` interface
  (`+0x124`), null on failure. Class `0x61325A2E`.
- **`QueryInterface`** (`0x009C38FD`): `0x61325A2D` (`cIGZWinScrollbar`) → `+0x124`, AddRef'd, true; else the
  slider's (so `0x21325207` still gives `+0xD8`).
- **Window overrides** (on top of the slider's): 0, 88 `GZPaint`, 134, 136, 138, 148 destructor, 153 layout; it
  adds slots 157–167 (§3.2). Its `cIGZWinSlider` table `0x00AE07D8` is the slider's except slot 0 (thunk
  `0x009C4094` to its QI), slots 4 and 5 (stubs `0x009C3EB2` / `0x009C3EAD` jumping to the same `GetValue` and
  `SetValue`) and slot 12 (`SetImage`, §3.5).
- **Constructor**: the slider constructor (so the area and first layout are the slider's), the three vtables, the
  timer `cRZTimer(4)` (milliseconds) at `+0x130`, then the values below and flag `0x8000` cleared again.

| Offset | Meaning | Constructor value |
|---|---|---|
| `+0x124` | `cIGZWinScrollbar` vtable | `0x00AE0788` |
| `+0x128` | line divisor `nL` | `range` (32) |
| `+0x12C` | page divisor `nP` | `range` (32) |
| `+0x130` | auto-repeat stopwatch (`cRZTimer`, unit 4) | constructed |
| `+0x148` | pressed part (§3.4) | 0 |
| `+0x14C` | the pointer is still on the pressed part (byte) | 1 |
| `+0xF8` | (slider margin) | 0 again |

## 2. The slider *[compat]*

### 2.1 `cIGZWinSlider` (vtable `0x00AE049C`, 14 slots)

Slots 1–3 are the shared AddRef, Release and `AsIGZWin` thunks (`0x0095BA81`, `0x0095BA8C`, `0x0095BA6F`).

| Slot | Address | Behaviour |
|---|---|---|
| 4 `GetValue()` | `0x009C312D` | `min + trunc(range · f + 0.5)` |
| 5 `SetValue(v)` | `0x009C3153` | §2.2 |
| 6 `SetMinimumValue(v)` | `0x009C3286` (shared) | store; layout (window slot 153); redraw. `f` is kept, so the value moves with the range |
| 7 `GetMinimumValue()` | `0x009C32AC` (shared) | the minimum |
| 8 `SetMaximumValue(v)` | `0x009C32B0` (shared) | as slot 6 |
| 9 `GetMaximumValue()` | `0x009CEDE1` (shared) | the maximum |
| 10 `SetOrientation(o)` | `0x009B2C1C` (shared) | store only: no layout, no redraw |
| 11 `GetOrientation(out)` | `0x009C2E3B` | writes `o` |
| 12 `SetImage(image, rect)` | `0x009C36E7` | §2.5 |
| 13 `GetImage(out image, out rect)` | `0x009C32D6` (shared) | the image (not AddRef'd) and `F`; true |

### 2.2 Value and fraction

**`SetValue(v)`** (slot 5): false at once while the guard is set. Set the guard; if `v` = `GetValue()`, clear it and
return false. Otherwise clamp (`v := min(v, max)`, then `max(v, min)`), `f := (v − min) / range` (float division),
place the thumb (window slot 152), notify, clear the guard, redraw, true. The comparison is made **before** the
clamp *[quirk]*.

**`SetFraction(p)`** (window slot 156, `0x009C31E7`): false while the guard is set. If `p` = `f` (x87 `fucompp`;
NaN counts as different), false. Otherwise `p < 0 → 0`, `p > 1 → 1` (a NaN passes both tests), store, place the
thumb, notify, clear the guard, redraw, true.

### 2.3 Geometry and the added window slots

| Window slot | Address | Behaviour |
|---|---|---|
| 151 `PointToFraction(out p, x, y)` | `0x009C363B` | below; always true |
| 152 `PlaceThumb()` | `0x009C2FF9` | below |
| 153 `Layout()` | `0x009C2F78` | below (the old scroll bar replaces it, §3.3) |
| 154 `ThumbWidth()` | `0x009C37DA` | with an image `F.r − F.l`; else horizontal `H`, vertical `W` |
| 155 `ThumbHeight()` | `0x009C3809` | with an image `F.b − F.t`; else horizontal `H`, vertical `W` |
| 156 `SetFraction(p)` | `0x009C31E7` | §2.2 |

Without an image the thumb is a square as thick as the bar.

- **Layout**: horizontal `lo := m`, `hi := W − m − tw`; otherwise `lo := H − m − th`, `hi := m` (a vertical slider
  grows **upward**). Then `PlaceThumb`, then redraw.
- **`PlaceThumb`**: the offset `t` (`+0xF0`) is `lo` when `range` = 0; else horizontal `lo + trunc((hi − lo) · f)`,
  otherwise `lo − trunc((lo − hi) · f)`. The thumb rectangle: horizontal `(t, H/2 − th/2, t + tw, H/2 − th/2 +
  th)`, otherwise `(W/2 − tw/2, t, W/2 − tw/2 + tw, t + th)`.
- **`PointToFraction`**: `a := x − tw/2` (horizontal) or `y − th/2`; clamp `a` between `lo` and `hi` (in whichever
  order they are); `p := (a − lo) / (hi − 2m − lo)` (float). A track of length 0 gives `0/0` = NaN *[quirk]*.
- **`SetArea(l, t, r, b)`** (window slot 55, `0x009C331A`): with an image, vertical forces `r := l + (F.r − F.l)`,
  horizontal `b := t + (F.b − F.t)`, and any other orientation **skips** `cGZWin::SetArea`; without an image the
  area is set as given. Then the layout (window slot 153) in every case.
- **`Init`** (window slot 4, `0x009C2E47`): if not yet initialised (flag `0x4000`), the layout first; then
  `cGZWin::Init`.

### 2.4 Input

- **Left down** (window slot 134, `0x009C2E86`): unless `GZSetCapture()` succeeds, false. Then `PointToFraction(x,
  y)` and `SetFraction`; true.
- **Mouse move** (138, `0x009C2F35`): while capturing, the same; true. Otherwise false.
- **Left up** (136, `0x009C2EC9`): while capturing: release, `PointToFraction` and `SetFraction`, then
  `SendMsg(+0x4C, 3, 0x887113A4, GetID(), 0)` (the release notice, sent whether the value changed or not); true.
  Otherwise false.

There is no keyboard handling.

### 2.5 Image and painting

**`SetImage(image, rect)`** (slot 12): AddRef the new image, release the old, store it. The source rectangle is the
image's (buffer slot 11) unless `rect` is given. Then:
- horizontal: with an image `F := (l, t, (r − l)/4, b − t)`; `SetH(F.b − F.t)` (window slot 52);
- vertical: with an image `F := (l, t, r − l, (b − t)/4)`; `SetW(F.r − F.l)` (window slot 51);
- another orientation: false (the image is kept).

A null image sets `F := (0, 0, 16, 16)` first, so the bar becomes 16 thick. Otherwise true. `F`'s right and bottom
hold **sizes**, so `F` is a true rectangle only for a source at `(0, 0)` *[quirk]*; the control manager passes the
whole image (rect null).

**`GZPaint`** (window slot 88, `0x009C3623`): the flat paint without an image, else the bitmap paint; true.

**Flat paint** (`0x009C3381`), colour `c` = `GetFillColorRGB` (window slot 126):
1. Fill the draw area with `c` (draw-context slot 21, then slot 35).
2. Colour `~c` (bitwise not). A line (slot 34): horizontal from `(left + m, top + H/2)` to `(right − m, top +
   H/2)`; otherwise from `(left + W/2, top + m)` to `(right − W/2, bottom − m − 1)`.
3. Fill the thumb rectangle, offset by `(left, top)`, with `~c`.

**Bitmap paint** (`0x009C34A3`): the image holds four frames side by side along the bar (`F` is frame 0); `d` =
`F` offset by `(left, top)`.
1. Frame 0 (start cap) at `d` (draw-context slot 38, `(image, src, dest)`).
2. Horizontal (frame step `s = F.r − F.l` along x): frame 1 (the track) with slot 39 `(image, src, dest, 0, 0)`
   *[inferred: tiled]* from `d`'s right edge to `right − s`; frame 2 (end cap) from `right − s` to `right`.
   Vertical: the same along y with `s = F.b − F.t`, to `bottom − s` and `bottom`. Another orientation: stop here.
3. Frame 3 (the thumb) at `F` offset by the draw origin plus the thumb's left and top.

## 3. The old scroll bar *[compat]*

### 3.1 `cIGZWinScrollbar` (vtable `0x00AE0788`, 20 slots, all `<shared>` stubs except 15 and 16)

| Slot | Address | Behaviour |
|---|---|---|
| 0–2 | `0x009C409F`, `0x009C40AA`, `0x009C40B5` | QI, AddRef, Release thunks |
| 3 `AsIGZWin()` | `0x009C40C0` | the window |
| 4 `AsIGZWinSlider()` | `0x009C3EB7` | `+0xD8` (null for null) |
| 5 `SetValue(v)` / 6 `GetValue()` | `0x009C40C8` / `0x009C40D0` | the slider's (§2.1) |
| 7 `SetRange(min, max)` | `0x009C3F8F` | slider slots 6 then 8; true |
| 8 `GetRange(out min, out max)` | `0x009C3FAF` | true |
| 9 `SetOrientation(o)` | `0x009C3EC7` | 1 → 1, 2 → 2, else 0; store only; true |
| 10 `GetOrientation()` | `0x009C3EEC` | 1, 2, or 0 |
| 11 `SetPageSize(n)` | `0x009C3F00` | `n` = 0 → false; `nP := range / n`; layout; true |
| 12 `GetPageSize()` | `0x009C3F2F` | `nP` = 0 → 0; else `(range + 1) / nP` (unsigned) |
| 13 `SetLineSize(n)` | `0x009C3F48` | `n` = 0 → false; `nL := range / n`; layout; true |
| 14 `GetLineSize()` | `0x009C3F77` | `nL` = 0 → 0; else `range / nL` (unsigned) |
| 15 `SetDivisors(a, b)` | `0x009C3B74` | `nL := a` (0 → `range`), `nP := b` (0 → 1); layout |
| 16 `GetDivisors(out a, out b)` | `0x009C3BAA` | `nL`, `nP` |
| 17 `SetImage(rect, image, x)` | `0x009C3FC6` | slider slot 12 `(image, rect)`; `x` unused |
| 18 `GetImage(out rect, out image, out x)` | `0x009C3FD9` | `F`, the image (not AddRef'd), `x := 0`; true when there is an image |
| 19 `GetSizeLimits(out q)` *[inferred name]* | `0x009C4004` | horizontal `q := (3·tw, th, 0xFFFF, th)`; vertical `(tw, 3·th, tw, 0xFFFF)`; true; other orientations false (`q` untouched) |

*[compat]* Verified image-setting callers include the text editor's vertical and horizontal
scrollbar-image setters (`0x009BD278`, `0x009BD2F9`) and scrollbar creation in `AddScrollBar`
(`0x009BE477`). They pass the source rectangle, image and **zero** as slot 17's third
argument; an empty rectangle selects the image's own area. Slot 17 forwards only the first
two arguments and discards the third (`0x009C3FC6`). Slot 18 writes zero to the corresponding
output (`0x009C3FD9`). This establishes that the third argument is unused on these paths;
callers establishing names for all size-limit outputs remain unresolved (WSL1).

"Layout" is window slot 153; none of these redraws. The divisors are numbers of steps, kept when the range changes
*[quirk]*.

### 3.2 The added window slots

| Window slot | Address | Behaviour |
|---|---|---|
| 157 `LineBack()` | `0x009C3A92` | `nL` ≠ 0: `SetValue(GetValue() − range / nL)` (unsigned division) |
| 158 `LineForward()` | `0x009C3ACA` | `nL` ≠ 0: `SetValue(GetValue() + range / nL)` |
| 159 `PageBack()` | `0x009C3B02` | `nP` ≠ 0: `SetValue(GetValue() − (range + 1) / nP)` |
| 160 `PageForward()` | `0x009C3B3B` | `nP` ≠ 0: `SetValue(GetValue() + (range + 1) / nP)` |
| 161 `HitTest(out part, x, y)` | `0x009C3D3B` | §3.4 |
| 162 arrow length, horizontal | `0x009C3E9D` | `ThumbWidth` (window slot 154) |
| 163 arrow length, vertical | `0x009C3EA5` | `ThumbHeight` (155) |
| 164 `LineIndex()` | `0x009C3BBF` | `nL` = 1 → 0; else `trunc((GetValue() − min) · (nL − 1) / range)` |
| 165 `LineValue(i)` | `0x009C3C20` | `nL` = 1 → `min`; else `min + trunc(range · i / (nL − 1))` |
| 166 `PageIndex()` / 167 `PageValue(i)` | `0x009C3C7D` / `0x009C3CDE` | as 164 / 165 with `nP` |

In 164–167, `i` and `n − 1` are converted to float as unsigned 32-bit numbers.

### 3.3 Geometry

**Layout** (window slot 153, `0x009C3E16`): arrow length `a` (slot 162 / 163, i.e. the thumb's size along the bar);
horizontal `lo := a`, `hi := W − a − tw`; otherwise `lo := a`, `hi := H − a − th` (the scroll bar grows
**downward**). Then `PlaceThumb` (§2.3); no redraw. Arrows and thumb are thus cells of the same size: without an
image, squares as thick as the bar.

### 3.4 Input and auto-repeat

**`HitTest(out part, x, y)`**: outside the window (window slot 121) → part 0, false. Along the bar (`x` or `y`), with
`t` the thumb offset and `L` = `W` or `H`: `< a` → 1 (start arrow); `< t` → 3 (track before the thumb); `≥ t + tw`
(or `th`) → 4 when `< L − a`, else 2 (end arrow); otherwise 5 (thumb). True.

Part actions: 1 `LineBack`, 2 `LineForward`, 3 `PageBack`, 4 `PageForward`.

- **Left down** (`0x009C392A`): the hit test into `+0x148`; on a hit, `+0x14C := 1` and, if `GZSetCapture()`
  succeeds, restart the stopwatch, redraw and run part 1–4's action (the thumb does nothing yet). True in every
  case.
- **Mouse move** (`0x009C3A1C`): while capturing: part 5 → `PointToFraction` and `SetFraction` (the drag);
  otherwise `+0x14C :=` (the hit test succeeds and gives the pressed part). True.
- **Left up** (`0x009C39BB`): while capturing: release; part 5 → `PointToFraction`, `SetFraction`; part := 0;
  redraw. True. No `0x887113A4` notice (the slider's handler is replaced).
- **Auto-repeat** (in `GZPaint`, `0x009C453D`): while capturing with `+0x14C` set and part 1–4, if the stopwatch
  reads more than 200 ms the part's action runs again and the stopwatch restarts; then redraw, which asks for the
  next paint. The first repeat and every later one come 200 ms apart.

### 3.5 Image and painting

**`SetImage(image, rect)`** (its `cIGZWinSlider` slot 12, `0x009C45F0`): AddRef/release/store as the slider. A null
image changes nothing else (true). Otherwise the source is the image's rectangle (buffer slot 12) or `rect`; `s :=
(r − l)/12` (twelve frames side by side); `F := (l, t, l + s, b)`; horizontal: `SetH(b − t)`; vertical: `SetW(s)`
and `F` moved right by `6s`; another orientation: false.

Frames (horizontal set 0–5, vertical set 6–11): +0 start arrow, +1 start arrow pressed, +2 thumb, +3 track, +4 end
arrow, +5 end arrow pressed.

**`GZPaint`**: after the auto-repeat step, the flat paint without an image, else the bitmap paint; true.

**Flat paint** (`0x009C4342`), `c` = `GetFillColorRGB`, `a` = `tw` and `b` = `th`: horizontal: `(left, top, left
+ a, top + b)` filled with `c`; the track `(left + a, top, right − a, top + b)` with `~c`; `(right − a, top, right,
top + b)` with `c`; the thumb `(left + thumb left, top, … + a, top + b)` with `c`. Vertical: the same boxes along y
(`(left, top)`, the track down to `bottom − b`, `(left, bottom − b)`, the thumb at `top + thumb top`). No arrow
glyphs.

**Bitmap paint** (`0x009C40F4`), `d` = `F` moved to the draw origin:
1. Start arrow: frame +1 when the pressed part is 1, else +0, at `d` (draw-context slot 38).
2. Track: frame +3 with slot 39 from `d`'s far edge to `right − s` (horizontal) or `bottom − (F.b − F.t)`.
3. End arrow: frame +5 when the pressed part is 2, else +4, up to `right` / `bottom`.
4. Thumb: frame +2 at `F` moved to `(left + thumb left, top)` or `(left, top + thumb top)`.

The pressed frames follow the part only, not whether the pointer is still on it (`+0x14C` is not read).

## 4. Lifecycle

- **`Shutdown`** (window slot 5, `0x009C2E65`): release the image (`+0x110` := null), then `cGZWin::Shutdown`.
- **Destructors**: the slider's body (`0x009C2DF9`) restores its two vtables and runs `~cGZWin`; the deleting form
  is `0x009C32FE`. The scroll bar's body (`0x009C38CD`) restores its three vtables, destroys the stopwatch (a bare
  return, `0x00A6D837`) and runs the slider's body; deleting form `0x009C40D8`. Neither releases the image.

## 5. Quirks and cases

Quirks:
1. `SetValue` compares before clamping: at the maximum, `SetValue(max + 1)` notifies though nothing changes (§2.2).
2. A track of length 0 (the default 32×96 old scroll bar: arrows 32, track 0) makes `PointToFraction` return NaN,
   which `SetFraction` stores; `GetValue` then returns `min + 0x80000000` (the x87 integer indefinite; invalid
   operations masked, the default). `range` = 0 with `v ≠ min` stores NaN the same way (WSL2).
3. Vertical sliders grow upward, vertical old scroll bars downward (§2.3, §3.3).
4. The drag denominator subtracts `2m`, but `m` is always 0 (§2.3).
5. The slider's `F` keeps sizes in its right/bottom (§2.5).
6. With an image and an orientation other than 1 or 2, `SetArea` ignores the area, `SetImage` returns false and the
   slider's bitmap paint draws only the start cap.
7. The divisors stay at the constructor's 32 when the range changes (§3.1), so after `SetRange(0, 1000)` a line is
   `1000/32 = 31` and a page `1001/32 = 31` until slot 11, 13 or 15 is called.
8. `GetPageSize` uses `range + 1`, `GetLineSize` `range` (§3.1).
9. Only the slider sends the release notice `0x887113A4` (§2.4, §3.4).
10. The destructors never release the image; only `Shutdown` does (§4).

Cases:
1. Default slider (vertical 32×96, no image): `tw = th = 32`, `lo = 64`, `hi = 0`. `SetValue(16)`: `f = 0.5`, offset
   `64 − trunc(64 · 0.5) = 32`, thumb `(0, 32, 32, 64)`, notify `{3, 0x887113A3, id, 0}`; `GetValue()` = 16.
2. Then a drag to `y = 40`: `a = 24`, `p = (24 − 64)/(0 − 64) = 0.625`, value `trunc(32 · 0.625 + 0.5) = 20`.
3. Old scroll bar, vertical 16×100, no image, range 0–32: `a = 16`, `lo = 16`, `hi = 68`. `SetValue(8)`: offset `16 −
   trunc(−52 · 0.25) = 29`. A press at `y = 80` is part 4 (`80 ≥ 45`, `80 < 84`): `PageForward` adds `33/32 = 1` → 9;
   held, again every 200 ms. After `SetPageSize(8)` (`nP = 4`) a page is `33/4 = 8`.
4. Default old scroll bar (vertical 32×96, laid out by its own slot 153 at `Init`; the constructor leaves the
   slider's `lo = 64`, `hi = 0`): `lo = hi = 32`; parts are 1 for `y < 32`, 5 for `32 ≤ y < 64`, 2 below; dragging
   the thumb gives value `−2,147,483,648` (quirk 2).

## 6. Function inventory

Owner `cGZWinSlider` (35): `0x009C2DF9` destructor body, `0x009C2E0E` QI, `0x009C2E3B` slot 11, `0x009C2E47`
`Init`, `0x009C2E65` `Shutdown`, `0x009C2E86` left down, `0x009C2EC9` left up, `0x009C2F35` mouse move,
`0x009C2F78` layout, `0x009C2FF9` `PlaceThumb`, `0x009C312D` slot 4, `0x009C3153` slot 5, `0x009C31E7`
`SetFraction`, `0x009C32F3` QI thunk, `0x009C32FE` deleting destructor, `0x009C331A` `SetArea`, `0x009C3381` flat
paint, `0x009C34A3` bitmap paint, `0x009C3623` `GZPaint`, `0x009C363B` `PointToFraction`, `0x009C36E7` slot 12,
`0x009C37DA` `ThumbWidth`, `0x009C3809` `ThumbHeight`, `0x009C3836` constructor, `0x009C38B4` `CreateNewObject`;
the old scroll bar's `0x009C38CD` destructor body, `0x009C3A92` `LineBack`, `0x009C3ACA` `LineForward`,
`0x009C3B02` `PageBack`, `0x009C3C20` `LineValue`, `0x009C3C7D` `PageIndex`, `0x009C3CDE` `PageValue`,
`0x009C3D3B` `HitTest`, `0x009C3E9D` and `0x009C3EA5` arrow lengths.

Owner `cGZWinScrollbar` (15): `0x009C38FD` QI, `0x009C392A` left down, `0x009C39BB` left up, `0x009C3A1C` mouse
move, `0x009C3B3B` `PageForward`, `0x009C3B74` slot 15, `0x009C3BAA` slot 16, `0x009C3BBF` `LineIndex`,
`0x009C3E16` layout, `0x009C40D8` deleting destructor, `0x009C4342` flat paint, `0x009C453D` `GZPaint`,
`0x009C45F0` `SetImage`, `0x009C46AF` constructor, `0x009C471E` `CreateNewObject`.

Not covered: the draw-context slots (WGN1 in widget-wingen.md); who calls `cIGZWinScrollbar` slots 17–19 (WSL1).
