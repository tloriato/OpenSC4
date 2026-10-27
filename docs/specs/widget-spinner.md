# `cGZWinSpinner`: the spin control (behaviour spec)

Scope: the `cGZWinSpinner` owner as tools/coverage.py assigns it on `bb641878` (51 functions, 4,707 bytes;
inventory in §7). The spinner is an integer field with up/down buttons: the buttons alone, or a value text with the
buttons on its right or left; it steps on clicks, arrow keys and with an accelerating auto-repeat, and shows its value
as plain, number or money text. The budget window's tax rates and loan amount use it (budget-window-bodies.md, IID
`0x612CE0C3`, message `0x682876C9`).

Related:
- widget-toolkit.md: the `cGZWin` base (fields §2: caption `+0x50`, buffer to draw to `+0x68`, its draw context
  `+0x6C`, notification target `+0x4C`; flags §3: `0x2` enabled, `0x4000` initialised; colours §17.8);
- widget-ctrlmgr.md §3.6: `CreateSpinner`; the control manager's `SystemFont` (slot 5) and `SystemBMP` (slot 7);
- widget-button.md §4: `RZDrawRaisedRect` and font slot 52 as used here;
- ui-helpers.md §2 and budget-window-bodies.md: the language utility (`RZ::LM()` slot 38 `(0)`) and its slots 16
  (money, as used by `SC4MakeMoneyString` `0x005F9AB0`) and 18 (number);
- timers.md §1: the `cRZTimer`.

Evidence: static RE only, on the instruction and data bytes of the Steam 1.1.641 exe (the same code as GOG,
`docs/lanes/re-dates.md`). Every function was read in the decompile; the constructor, `SetArea`, `SetValue`, the
flat button paint, the colour accessors, `Init` and `SetFontStyle` were checked on the instructions (most interface
methods run on the interface pointer `+0xD8`, which the decompile's field names do not show). The vtables
`0x00AE0B90` (window) and `0x00AE0DF0` (`cIGZWinSpinner`) were read and compared with the `cGZWin` table
`0x00ADC8D8`; the format `0x00A8281C` is `"%d"`. Written 2026-10-24 (re-gzspinner7).

Conventions:
- Offsets are from the window start; the interface is at `+0xD8`. "Window slot n" is a `cIGZWin` slot; "slot n"
  alone a `cIGZWinSpinner` slot.
- `W`, `H` are window slots 41, 42; the draw area is `+0x24`…`+0x30` (left, top, right, bottom). `bw`, `bh` are the
  button cell's width and height (`+0x14C`, `+0x150`); `gx`, `gy` the gutters (`+0x154`, `+0x155`); `fh` the font's
  line height (`+0x124`).
- Parts: 0 none, 1 up (adds the step), 2 down (subtracts it). "Redraw" is window slot 92. Colours named "packed" are
  `0xAARRGGBB`; "native" ones go through window slots 127 (packed → native) and 128 (native → packed, alpha forced to
  `FF`). `0xDEADBEEF` marks an unset text colour.

## 1. Identity and fields *[compat]*

- **Factory** `CreateNewObject` (`0x009C5973`): `0x170` bytes, constructor `0x009C5327`; returns the interface
  (`+0xD8`), null on failure. Class `0x612CE0C4`.
- **`QueryInterface`** (`0x009C4745`): `0x612CE0C3` → `+0xD8` (null for null), AddRef'd, true; else
  `cGZWin::QueryInterface`. Slot 0 of the interface (`0x009C4BF3`) is the thunk.
- **Window overrides**: 0, 4 `Init`, 5 `Shutdown`, 55 `SetArea`, 88 `GZPaint`, 130 key down, 134 left down, 136 left
  up, 138 mouse move, 140 capture changed, 148 destructor. It adds no window slots.
- **Constructor**: `cGZWin` constructor, the vtables, the timer `cRZTimer(4)` (milliseconds) at `+0xE8`, then the
  values below. It does not set an area (`Init` forces at least 1×1, §4).

| Offset | Meaning | Constructor value |
|---|---|---|
| `+0xDC` | flags (§2.2) | `0x1000` |
| `+0xE0` | type: 0 buttons only, 1 value text with the buttons on the right, 2 buttons on the left | 0 |
| `+0xE8` | auto-repeat stopwatch (`cRZTimer`, unit 4) | constructed |
| `+0x100` | auto-repeat delay (ms) | 400 |
| `+0x104` | pressed part | 0 |
| `+0x108` | part under the pointer (hover) | 0 |
| `+0x10C`, `+0x110` | minimum, maximum | 0, 0 |
| `+0x114` | value | 0 |
| `+0x118` | step | 1 |
| `+0x11C` | font (counted) | null |
| `+0x120` | font style ID (for the control manager's `SystemFont`) | `0x68963C4C` |
| `+0x124` | `fh`, the font's line height (16-bit) | 0 |
| `+0x126` | text alignment (byte: 0, 1, 2) | 2 |
| `+0x128`, `+0x12C`, `+0x130` | text colours: normal, hover, disabled (packed) | `0xDEADBEEF` ×3 |
| `+0x134` | button image (a `cIGZBuffer`, counted) | null |
| `+0x138`…`+0x144` | custom button image rectangle (used with flag `0x8000`) | 0 |
| `+0x148`, `+0x14A` | source x, y of the button cells (16-bit) | 0 |
| `+0x14C`, `+0x150` | `bw`, `bh` | 0 |
| `+0x154`, `+0x155` | `gx`, `gy` (bytes) | 4, 1 |
| `+0x158`…`+0x164` | outline colours: left, top, right, bottom | 0 |
| `+0x168`, `+0x16A` | largest and smallest flat button width (16-bit) | 16, 8 |
| `+0x16C` | flat button colour: **never written** by the class *[quirk]* (WSP1) | — |

*[compat]* Factory allocation does not supply a zero default for this colour (`0x009C5973`,
`0x009C5327`). The shared allocator uses a 512-byte free-list block for this object and
returns it without clearing the payload (`0x005E55E0`, `0x005B9FC0`, `0x005E5CB0`,
`0x005E16E0`, table `0x00AA4024`, `0x005E5BC0`). Thus flat painting can consume a colour left by an earlier
allocation (`0x009C4E36`). Whether shipped UI resources exercise this path remains WSP1.
*[choice]* A replacement may initialise the flat colour to zero; that value is not an original
allocator guarantee.

## 2. The `cIGZWinSpinner` slots *[compat]*

### 2.1 Table (vtable `0x00AE0DF0`, 35 slots)

Slots 1–3 are the shared AddRef, Release and `AsIGZWin` thunks. "True" means the slot returns true.

| Slot | Address | Behaviour |
|---|---|---|
| 4 `SetSpinnerType(t)` | `0x009C5141` | `t` ∉ {0, 1, 2} → false. For 1 or 2 without a font, `SetFontStyle(+0x120)` first. Store; re-apply the area (§3); true |
| 5 `GetSpinnerType(out)` | `0x009C48C0` (shared) | true |
| 6 `SetFlag(f, on)` | `0x009C5943` | thunk to `0x009C52A3`, §2.2 |
| 7 `IsFlagSet(f)` | `0x00999055` (shared) | `(flags & f) ≠ 0` |
| 8 `GetValue()` | `0x009C48CE` | the value |
| 9 `SetValue(v)` | `0x009C517B` | §2.3 |
| 10 `SetMinMax(min, max)` | `0x009C48D2` | store only (the value is not re-clamped); true |
| 11 `GetMinMax(out, out)` | `0x009C48E5` | true |
| 12 `SetStepSize(s)` / 13 `GetStepSize(out)` | `0x009C48FC` / `0x009C4908` | store / read; true |
| 14 `SetButtonImage(image, rect)` | `0x009C4D72` | store the image (counted); `rect` null → flag `0x8000` off, else on and `rect` copied to `+0x138`; cells (§3); re-apply the area; true |
| 15 `SetButtonImage(image)` | `0x009C4DD7` | store; cells; re-apply; true |
| 16 `GetButtonImage(out)` | `0x009C4E28` | the image, not AddRef'd; true |
| 17 `SetButtonImageArea(rect)` | `0x009C4E00` | copy to `+0x138` (flag `0x8000` untouched); cells; re-apply; true |
| 18 `GetButtonImageArea(out)` | `0x009C4772` | `+0x138`; true |
| 19 `SetFont(font)` | `0x009C51DB` | if different: store (counted); with a font, `fh :=` font slot 37 `(0, 0)` (line height); re-apply the area. True |
| 20 `SetFontStyle(id)` | `0x009C4916` | store; when initialised, `font := WCM.SystemFont(id)` (no check for a missing manager) and, if not null, `SetFont(font)`; true |
| 21 `GetFontStyle(out)` | `0x009C4950` | true |
| 22 `SetFontColor(i, packed)` | `0x009C594E` | if different: store colour `i`, redraw; true |
| 23 `SetFontColor(i, native)` | `0x009C495E` | as 22 with `NativeToPacked(native)` |
| 24 `GetFontColor(i, out packed)` | `0x009C49C2` | unset → false; else the colour, true |
| 25 `GetFontColor(i, out native)` | `0x009C4995` | unset → false; else `PackedToNative(colour)`, true |
| 26 `ClearFontColor(i)` | `0x009C49E0` | slot 23 `(i, 0xDEADBEEF)`: the sentinel goes through `NativeToPacked`, so the colour is **not** cleared *[quirk]* (WSP2) |
| 27 `SetTextAlignment(a)` | `0x009C49F3` | `a` ∈ {0, 1, 2}: store, redraw, true; else false |
| 28 `GetTextAlignment(out)` | `0x009C4A29` | 0, 1 or 2 (true); another stored value → false |
| 29 `SetGutters(gx, gy)` / 30 `GetGutters(out, out)` | `0x009C521A` / `0x009C4A5F` | store and re-apply the area / read; true |
| 31 `SetOutlineColors(l, t, r, b)` | `0x009C5238` | each `NativeToPacked`; redraw; true |
| 32 `GetOutlineColors(out ×4)` | `0x009C4A76` | each `PackedToNative`; true |
| 33 `SetOutlineColor(i, &c)` | `0x009C4ADF` | colour `i` := `*c` **raw**; redraw; true |
| 34 `GetOutlineColor(i, out)` | `0x009C4B03` | raw; true |

"Cells" is §3's cell calculation; "re-apply the area" is `SetArea(GetArea())` (window slots 48 and 55,
`0x009C4BBC`), which re-runs the size rules.

### 2.2 Flags (`+0xDC`) and `SetFlag(f, on)` (`0x009C52A3`)

| Bit | Meaning |
|---|---|
| `0x1` | draw the outline (types 1, 2) |
| `0x2` | fill the text box with the fill colour (types 1, 2) |
| `0x1000` | the caption shows the value (§2.3) |
| `0x2000` | … as a number (language utility slot 18) |
| `0x4000` | … as money (language utility slot 16) |
| `0x8000` | use the custom button image rectangle |

`SetFlag` sets or clears `f`, then: `0 < f < 3` → redraw; `0x2000` (when set, `0x4000` is cleared) and `0x4000` (when
set, `0x2000` is cleared) → refresh the caption; `0x8000` → cells and re-apply the area; any other `f` (including
`0x1000` and combinations such as 3) → nothing more. True.

### 2.3 Value and caption

**`SetValue(v)`** (slot 9): `v < min → min`, then `v > max → max` (so `max` wins when `max < min`); store. If the value
changed: refresh the caption, then `SendMsg(+0x4C, 0x682876C9, GetID(), value, 0)` (window slot 144; the value and
`0` are pushed before the argument-less `GetID`). True either way.

**Caption refresh** (`0x009C4B19`): with flag `0x1000`: `0x2000` → `LM` slot 38 `(0)`, then its slot 18 `(value as
a sign-extended int64, &caption)`; else `0x4000` → its slot 16 `(int64, &caption, 0)`; otherwise, or when the
language manager or utility is missing, the caption's `Sprintf("%d", value)` (cIGZString slot 27). Without
`0x1000` the caption is left as it is. Redraw in every case.

## 3. Geometry *[compat]*

**Cells** (`0x009C4D02`): only with a button image. The source rectangle is `+0x138` with flag `0x8000`, else the
image's (buffer slot 11). `+0x148 := l`, `+0x14A := t`, `bw := (r − l)/4`, `bh := (b − t)/2`: the image is 4
columns (disabled, normal, pressed, hover) by 2 rows (up, down).

**`SetArea(l, t, r, b)`** (window slot 55, `0x009C54A2`), `w = r − l`, `h = b − t`:

| | Type 0 | Types 1, 2 |
|---|---|---|
| with a button image | `w ≥ bw`, `h ≥ 2·bh` | `w ≥ bw + 2·gx`, `h ≥ max(fh + 2·gy, 2·bh)` |
| without | `w ≥ 8`, `h ≥ 8`; then `bw := w`, `bh := h/2` | `h ≥ fh + 2·gy`, `w ≥ 8 + 2·gx`; then `bh := h/2`, `bw := clamp(h/2, 8, 16)` |

(8 and 16 are `+0x16A` and `+0x168`.) If `(l, t, l + w, t + h)` differs from the current area: `cGZWin::SetArea`
with it, then redraw.

**Layout used by painting and hit tests**: the buttons start `dx` from the left edge: `W − bw` for type 1, else 0.
The text box (types 1, 2) is `(left, top, left + W − bw − gx, bottom)` for type 1 and `(left + bw + gx, top, right,
bottom)` for type 2; the text rectangle is the box narrowed by `gx` on both sides, `fh` high, starting `(H − fh)/2`
below `top`.

**Hit test** (`0x009C4840`, `(out part, x, y)`): outside the window (window slot 121) → 0, false. Type 0, or type 1
with `x ≥ W − bw`, or type 2 with `x ≤ bw`: part 1 when `y < H/2`, else 2; otherwise 0. True.

## 4. Lifecycle and input

- **`Init`** (window slot 4, `0x009C4BFE`): type outside 0–2 → false. With the control manager (`RZ::WCM()`): no
  button image → `SetButtonImage(SystemBMP(0x68963ED6), null)` (slot 14); no font and type ≠ 0 → `SetFont(SystemFont(+
  0x120)` or, if that is null, `SystemFont(0x68963C4C))`. Then `W < 1` → `SetW(1)`, `H < 1` → `SetH(1)` (window slots
  51, 52); refresh the caption; `cGZWin::Init`.
- **`Shutdown`** (window slot 5, `0x009C4CD5`): `HideWindow` (window slot 70), release the button image and the
  font, `cGZWin::Shutdown`.
- **Destructor** (body `0x009C5459`, deleting form `0x009C599A`): restore the vtables, the spinner's `Shutdown`,
  release the image and font if still held, the stopwatch (a bare return), `~cGZWin`.
- **Key down** (window slot 130, `0x009C47D2`, `(key, mods)`): End (`0x23`) → `SetValue(max)`, Home (`0x24`) → `min`,
  Up (`0x26`) → `value + step`, Down (`0x28`) → `value − step`; true. Any other key: `SendMsg(+0x4C, 5, key, mods,
  0)` and its result.
- **Left down** (134, `0x009C5073`): hit test into the pressed part; delay := 400; restart the stopwatch; part 1 →
  `SetValue(value + step)`, part 2 → `SetValue(value − step)`; redraw; true. No capture is taken here.
- **Mouse move** (138, `0x009C50E3`): hit test into the hover part. Nothing pressed: if the hover part changed, take
  capture when it is now a button, release it when it is 0, and redraw. Pressed: if it changed, delay := 400 and
  redraw. True.
- **Left up** (136, `0x009C4786`): delay := 400; release capture when the hover part is 0; pressed := 0; redraw;
  true.
- **Capture changed** (140, `0x009C47B8`, `(…, new owner)`): another owner → pressed := 0, redraw (the hover part is
  kept). True.
- **Auto-repeat** (at the end of `GZPaint`): while the pressed and hover parts are the same button, once the stopwatch
  reads more than the delay: `SetValue(value ± step)`, delay −= 50 while it is above 50, restart; then redraw (which
  asks for the next paint). Repeats come 400, 350, 300 … 50, 50 ms apart.

## 5. Painting (`GZPaint`, window slot 88, `0x009C55F3`) *[compat]*

For types 1 and 2, on the text box (§3); type 0 skips steps 1, 2 and 4:
1. Flag `0x1`: four 1-pixel lines (draw-context slot 21, then slot 34): left edge with colour `+0x158`, top `+0x15C`,
   right (`r − 1`) `+0x160`, bottom (`b − 1`) `+0x164`; the box then shrinks by 1 on each side.
2. Flag `0x2`: fill the box with `GetFillColorRGB` (window slot 126; draw-context slots 21, 35).
3. The buttons at `dx`:
   - **flat** (no image, `0x009C4E36`): up `(left + dx, top, + bw, top + bh)`, down `(left + dx, top + bh, + bw,
     bottom)`, each `RZDrawRaisedRect(context, rect, +0x16C, raised, 1)` with `raised` = 0 only while that button is
     both pressed and hovered;
   - **image** (`0x009C4ED7`): the cell column is 0 when the window is disabled (window slot 67 with flag 2); else 2
     when the button is pressed and hovered, 3 when nothing is pressed and it is hovered, 1 otherwise. Up from row 0
     at `(left + dx, top + (H/2 − bh)/2)`, down from row 1 at the same place `H/2` lower, `bw × bh` each
     (draw-context slot 38).
4. With a font (types 1, 2): the colour is `+0x130` when disabled, else `+0x128` with no hover part, else `+0x12C`.
   Font slot 52 `(buffer to draw to, text rectangle, caption characters, length, alignment, c, 0x8000000, 0, 0)` with
   `c` = `PackedToNative(colour)`, or `0x8000000` (the font's own) when the colour is unset.
5. The auto-repeat step (§4). True.

## 6. Quirks and cases

Quirks:
1. The flat buttons are drawn with `+0x16C`, which nothing writes (WSP1).
2. `ClearFontColor` cannot restore the unset sentinel (§2.1, WSP2).
3. `SetOutlineColors`/`GetOutlineColors` convert colours, `SetOutlineColor`/`GetOutlineColor` do not.
4. `SetMinMax` leaves an out-of-range value until the next `SetValue`.
5. `SetFlag(0x1000, …)` does not refresh the caption.
6. `SetButtonImageArea` does not turn flag `0x8000` on.
7. `SetFontStyle` calls the control manager without checking it exists.
8. Losing capture keeps the hover part, so the hover text colour can stay.

Cases:
1. Range 0–100, step 5, value 50: Up arrow → `SetValue(55)`: caption `"55"` (flag `0x1000` only), message
   `{0x682876C9, id, 55, 0}` to `+0x4C`; End → 100; Down at 0 → stays 0, no message.
2. Holding the up button: +step at the press, then at about 400, 750, 1050, 1300, 1500, 1650, 1750, 1800, 1850 ms
   (gaps 400 … 50, then 50).
3. Type 1 without an image, `fh` = 12, defaults `gx` 4, `gy` 1: `SetArea(0, 0, 80, 10)` → 80×14, `bh` = 7, `bw` =
   `clamp(7, 8, 16)` = 8; up button `(72, 0, 80, 7)`, down `(72, 7, 80, 14)`; text box `(0, 0, 68, 14)`, text rectangle
   `(4, 1, 64, 13)`.
4. Image buttons, pressed up and still over it: up cell from column 2, down from column 1; disabled: both column 0.

## 7. Function inventory (owner `cGZWinSpinner`, 51)

`0x009C4745` QI, `0x009C4772` slot 18, `0x009C4786` left up, `0x009C47B8` capture changed, `0x009C47D2` key down,
`0x009C4840` hit test, `0x009C48CE` slot 8, `0x009C48D2` slot 10, `0x009C48E5` slot 11, `0x009C48FC` slot 12,
`0x009C4908` slot 13, `0x009C4916` slot 20, `0x009C4950` slot 21, `0x009C495E` slot 23, `0x009C4995` slot 25,
`0x009C49C2` slot 24, `0x009C49E0` slot 26, `0x009C49F3` slot 27, `0x009C4A29` slot 28, `0x009C4A5F` slot 30,
`0x009C4A76` slot 32, `0x009C4ADF` slot 33, `0x009C4B03` slot 34, `0x009C4B19` caption refresh, `0x009C4BBC`
re-apply the area, `0x009C4BF3` QI thunk, `0x009C4BFE` `Init`, `0x009C4CD5` `Shutdown`, `0x009C4D02` cells,
`0x009C4D72` slot 14, `0x009C4DD7` slot 15, `0x009C4E00` slot 17, `0x009C4E28` slot 16, `0x009C4E36` flat buttons,
`0x009C4ED7` image buttons, `0x009C5073` left down, `0x009C50E3` mouse move, `0x009C5141` slot 4, `0x009C517B`
slot 9, `0x009C51DB` slot 19, `0x009C521A` slot 29, `0x009C5238` slot 31, `0x009C52A3` `SetFlag`, `0x009C5327`
constructor, `0x009C5459` destructor body, `0x009C54A2` `SetArea`, `0x009C55F3` `GZPaint`, `0x009C5943` slot 6,
`0x009C594E` slot 22, `0x009C5973` `CreateNewObject`, `0x009C599A` deleting destructor.

Not covered: the draw-context slots (WGN1 in widget-wingen.md) and the language utility's formats.
