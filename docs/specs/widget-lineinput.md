# `cGZWinLineInput`: the single-line text field (behaviour spec)

Scope: the `cGZWinLineInput` owner as tools/coverage.py assigns it on `bb641878` (58 functions, 5,854 bytes;
inventory in §9). It is the toolkit's one-line edit field: a byte string with a caret, a selection, horizontal
scrolling, a character limit, three text colours, outline styles and change/complete notifications. UI scripts
create it as `GZWinLineInput` (class `0x21335C5A`, legacy-control.md §9.17).

Related:
- widget-toolkit.md: the `cGZWin` base (fields §2, flags §3, focus and capture §6, `SendMsg` §7, colour
  conversions §18);
- text-edit-encoding.md (`GetCharLength`, `GetPreviousCharLength`, code pages);
- legacy-control.md §9.17 (the script properties and the slots they call).

Evidence: static RE only, on the instruction and data bytes of the Steam 1.1.641 exe (the same code as GOG,
`docs/lanes/re-dates.md`). Every function was read in the decompile; the key dispatch, the Backspace path,
`DeleteChar`, the colour accessors, the highlight alpha tests and the outline paths were checked on the
instructions. The vtables `0x00AE0F18` (`cIGZWinLineInput`, 38 slots) and `0x00AE0FB0` (window) were read and
compared with the `cGZWin` table `0x00ADC8D8`. Written 2026-10-23 (re-gzwinli7).

Conventions:
- Offsets are from the window start; the interface is at `+0xD8`. "Window slot n" is a `cIGZWin` slot; "slot n"
  alone a `cIGZWinLineInput` slot.
- `len` is the text length in bytes; positions are byte indices. `gl, gt, gr, gb` are the gutters.
- "Redraw" is window slot 92. "Notify(code)" is `SendMsg(notification target, 3, code, GetID(), 0)` (window slot 144;
  the 0 is pushed before the argument-less `GetID`, slot 63).

## 1. Identity and fields *[compat]*

- **Factory** `CreateNewObject` (`0x009C6ABE`): `0x150` bytes, constructed **editable**; it returns the
  **interface** (`+0xD8`), or null.
- **`QueryInterface`** (`0x009C59B6`): `0x21335C59` → `+0xD8` (null for null), AddRef'd, true; else `cGZWin`'s.
  Interface slot 0 (`0x009C69AA`) is the thunk.
- **Window overrides**: 0, 4 `Init`, 5 `Shutdown`, 55 `SetArea`, 68 `SetFlag`, 88 `GZPaint`, 129 character,
  130 key down, 132 set focus, 133 kill focus, 134 left down, 136 left up, 138 mouse move, 148 destructor.
- **Constructor** (`0x009C68AC`, argument: editable byte): `cGZWin` constructor, then the values below; its own
  `SetArea(0, 0, 128, 64)`; fill colour (`cGZWin::SetFillColor`) `:= WCM.GetColor(0xA8963FB2)` (window-control
  manager slot 32, **not null-checked**).
- **Destructor** (`0x009C69B5`; deleting form `0x009C6AE7`): restore the vtables, `Shutdown`, destroy the text,
  `~cGZWin`.

| Offset | Meaning | Constructor value |
|---|---|---|
| `+0xDC` | code page (for multi-byte characters) | 0 (`Init`: the framework's) |
| `+0xE0` | caret | 0 |
| `+0xE4`, `+0xE8` | selection anchor and end (`[min, max)` is selected) | 0, 0 |
| `+0xEC` | first visible byte (horizontal scroll) | 0 |
| `+0xF0` | the text (`cRZString`; begin `+0xF4`, end `+0xF8`) | empty |
| `+0x104` | "notifying" guard byte (§4) | 0 |
| `+0x108` | font (counted) | null |
| `+0x10C` | line height | 1 |
| `+0x110`, `+0x114`, `+0x118` | text colours 0 normal, 1 disabled, 2 selected (`0xDEADBEEF` = unset) | 0, 0, `0xFFFFFFFF` |
| `+0x11C` | outline type: 0 none, 1 plain, 2 3D, 3 double 3D | 1 |
| `+0x120` … `+0x12C` | outline colours 0–3 (stored packed `0xFFRRGGBB`) | 0 |
| `+0x130` | highlight fill colour (packed; `0xDEADBEEF` = unset) | `0xDEADBEEF` |
| `+0x134` | highlight alpha (float, clamped to [0, 1]) | 1.0 |
| `+0x138` | filled (byte) | 1 |
| `+0x139` | editable (byte) | the argument |
| `+0x13C` | character limit (unsigned; `0xFFFFFFFF` = none) | `0xFFFFFFFF` |
| `+0x140` … `+0x143` | gutters `gl, gt, gr, gb` (signed bytes) | 2, 0, 2, 0 |
| `+0x144` … `+0x14C` | sound IDs 0–2 | **not initialised** |

## 2. The `cIGZWinLineInput` slots *[compat]*

Vtable `0x00AE0F18`, 38 slots; 1–3 are the shared AddRef, Release and `AsIGZWin` thunks. Setters return true unless
stated.

| Slot | Address | Behaviour |
|---|---|---|
| 4 `SetFont(font)` | `0x009C5BBA` | AddRef the new one, release the old one (no equality test), store; line height := font slot 37 `(0, 0)`, or 1 without a font. No redraw |
| 5 `SetFontColor(i, c)` | `0x009C5C11` | `i < 3`: store, redraw when changed, true; else false |
| 6 `SetFontColor(c)` | `0x009C5C01` | slot 5 `(0, c)` |
| 7 `GetFontColor(i, out)` | `0x009C5C48` | false for `i ≥ 3` or an unset colour |
| 8 `ClearFontColor(i)` | `0x009C5C6B` | slot 5 `(i, 0xDEADBEEF)` |
| 9 `SetFilled(b)` / 10 `IsFilled` | `0x009C5C7C` / `0x009C5C96` | store and redraw / the byte |
| 11 `SetOutlineType(t)` / 12 `GetOutlineType` | `0x009C5CBD` / `0x009BCB20` | store, **no redraw** / the value (a shared getter) |
| 13 `SetOutlineColor(c)` | `0x009C5CAF` | slot 14 `(0, c)` |
| 14 `SetOutlineColor(i, c)` | `0x009C5CC9` | `stored := ConvertNativeToPackedRGB(c)` (window slot 128, computed first); `i < 4`: store, redraw when changed, true; else false |
| 15 `GetOutlineColor(i, out)` | `0x009C5D0E` | `i < 4`: `*out := ConvertPackedRGBToNative(stored)` (window slot 127), true; else false |
| 16 `SetOutline(b)` | `0x009C5C9A` | slot 11 `(b != 0)`: type 1 or 0 |
| 17 `SetHighlightFillColor(c)` / 18 `Get…(out)` | `0x009C5D3C` / `0x009C5D68` | slot 128 in, redraw / slot 127 out |
| 19 `SetHighlightFillAlpha(a)` / 20 `Get…(out)` | `0x009C5D86` / `0x009C5DC7` | clamp to [0, 1], redraw / the float |
| 21 `GetText(cIGZString& out)` | `0x009C62EB` | `out` slot 14 (assign) from the text; returns `len` |
| 22 `SetText(const char*)` | `0x009C6A7E` | builds a `cRZString`, slot 23 |
| 23 `SetText(cIGZString* s)` | `0x009C701E` | §4 |
| 24 `SetText(cIGZString&)` | `0x009C5DFC` | jumps to slot 23 |
| 25 `SetCursorLoc(p)` | `0x009C62C9` | `p := min(p, len)`; caret, anchor and end `:= p`. No scroll update, no redraw |
| 26 `GetCursorLoc(out)` | `0x009C48C0` | the caret (a shared getter) |
| 27 `IsEditable` / 28 `SetEditable(b)` | `0x009C5E01` / `0x009C5E05` | the byte / store; off: window slot 85 `SetCursor(null, 1)`; on: the I-beam (cursor manager slot 9 with `(3, 0)`) through window slot 85 |
| 29 `SetCharacterLimit(n)` / 30 `Get…(out)` | `0x009C5E50` / `0x009C5E5C` | store only (the text is not cut) |
| 31 `SetSelection(start, count)` | `0x009C6305` | §5.3 |
| 32 `GetSelection(&start, &count)` | `0x009C5E6A` | anchor, `end − anchor` (negative for a backward selection) |
| 33 `SetGutters(l, t, r, b)` / 34 `GetGutters` | `0x009C5E84` / `0x009C5EB3` | bytes; store and redraw / out |
| 35 `GetMinMaxInfo(out[4])` | `0x009C5EDC` | §3 |
| 36 `SetSoundID(i, id)` | `0x009C5F34` | `i < 3`: store; returns `i < 3` |
| 37 `PlaySound(i)` | `0x009C6096` | §6 |

## 3. Size

**`GetMinMaxInfo`**: outline width `o` = 1 for types 1 and 2, 2 for type 3, else 0. Minimum
`(gl + gr + 2o, gt + gb + line height + 2o)`, maximum `(0x7FFFFFFF, 0x7FFFFFFF)`.

**`SetArea(l, t, r, b)`** (window slot 55, `0x009C5F96`): width and height raised to that minimum (the left and top
stay), then `cGZWin::SetArea`.

## 4. Text and notifications *[compat]*

**`SetText(s)`** (slot 23):
1. `s` null → the text is cleared. Otherwise copy it (`s` slot 14 into the text); if `len` exceeds the limit, cut
   it to the limit.
2. If the guard `+0x104` is 0: guard := 1, **Notify(`0xA871112F`)** (changed), guard := 0.
3. Caret, anchor, end and scroll := 0; guard := 0; redraw. True.

The guard keeps a changed-handler that calls `SetText` again from notifying twice.

- **Changed** `0xA871112F` (`0x009C5F72`): sent by `SetText`, `DeleteChar` and `InsertChar`.
- **Completed** `0xA8711130` (`0x009C5F4E`): Enter in an editable field (§5.2).

**`DeleteChar(pos, n)`** (`0x009C647A`): **`pos` is 1-based**: `pos = 0`, `pos > len` or `n = 0` → false; else
erase `n` bytes from index `pos − 1`, notify changed, true.

**`InsertChar(ch, at)`** (`0x009C6F14`): `len ≥ limit` → `PlaySound(1)`, false. Else insert one byte `ch` at `at`,
notify changed, true.

## 5. Editing *[compat]*

### 5.1 Characters (window slot 129, `0x009C6F5A`)

Only when editable (otherwise false):
- `ch < 0x20`: `PlaySound(0)`, false;
- the limit is `0xFFFFFFFF`, or `len − |end − anchor| < limit`: if there is a selection, send key down Backspace
  (window slot 130 with `(8, 0)`), which deletes it (§5.2); `InsertChar(ch, caret)`; caret `+= 1` (whatever
  `InsertChar` returned); anchor and end := caret; scroll update (§5.4); redraw; true;
- otherwise `PlaySound(1)`, false.

### 5.2 Keys (window slot 130, `0x009C64B7`)

`shift` = modifier bit 1. All keys not listed return true and do nothing.

| Key | Effect |
|---|---|
| Tab `0x09`, Esc `0x1B` | `SendMsg(notification target, 5, vk, modifiers, 0)`; its result is returned |
| Enter `0x0D` | editable → Notify(`0xA8711130`); then, with a selection, anchor and end := caret (deselect) |
| Home `0x24` | caret ≠ 0: caret, anchor, end := 0 (`shift` ignored) |
| End `0x23` | caret ≠ `len`: caret, anchor, end := `len` (`shift` ignored) |
| Left `0x25` | caret 0: without `shift`, `SetCursorLoc(0)` and deselect; with `shift` nothing. Else `n` = the previous character's length (`GetPreviousCharLength`): without `shift`, `SetCursorLoc(caret − n)`; with `shift`, caret `−= n` and, when `end > 0`, end `−= n` (the anchor stays) |
| Right `0x27` | caret ≥ `len`: without `shift`, `SetCursorLoc(len)`; with `shift` nothing. Else `n` = the next character's length (`GetCharLength`): without `shift`, `SetCursorLoc(caret + n)`; with `shift`, caret and end `+= n` while below `len` |
| Delete `0x2E` | with a selection: §5.2.1. Without: editable and non-empty → `DeleteChar(caret + 1, n)` (the character at the caret) |
| Backspace `0x08` | with a selection: §5.2.1. Without: caret ≠ 0 and editable → `n` = previous character length, `DeleteChar(caret − n, n)`, caret `−= n`, anchor and end := caret; if the scroll equals the new caret, scroll `:= max(scroll − 10, 0)` |

After every change: scroll update (§5.4) and redraw.

*Quirk*: Backspace passes `caret − n` to the 1-based `DeleteChar`, so it erases the `n` bytes that start one byte
**before** the previous character (`"hello|"` → `"helo|"`), and at caret `n` it erases nothing (index 0 is
rejected) while the caret still moves back. Delete and the selection delete pass the 1-based position correctly
(WLI1).

**§5.2.1 Selection delete** (Delete or Backspace with `anchor ≠ end`): `lo = min`, `hi = max`; when editable
`DeleteChar(lo + 1, hi − lo)`; then `SetCursorLoc(lo)` (also when not editable: it only deselects).

### 5.3 `SetSelection(start, count)` (slot 31)

`start ≥ len` → `start := len − 1` (0 for empty text); `start + count > len` → `count := len − start`. Anchor
`:= start`; end and caret `:= start + count`; scroll update; redraw. A negative `count` is not corrected.

### 5.4 Scrolling (`UpdateViewLocation`, `0x009C61D1`)

Empty text: caret and scroll := 0. Otherwise, when `scroll < caret`: while the width of `text[scroll, caret)` (font
slot 48) exceeds the inner width `drawArea.r − gr − gl − drawArea.l`, advance `scroll` by one character. When
`caret ≤ scroll`: `scroll := min(caret, len − 1)`.

### 5.5 Mouse and focus

- **`FindTextIndexFromCursorLocation(x)`** (`0x009C610B`): `x' = x − gl`. `x' ≤ 0` → `scroll`. Walking the characters
  from `scroll`, with `w` the width of `text[scroll, scroll + k)`: at the first `k` with `x' < w`, return
  `scroll + k`, or one character less when `x'` is nearer the previous boundary (`x' − wPrev ≤ w − x'`). Past the
  end → `len`.
- **Left down** (`0x009C636A`), `(x, y, modifiers)`:
  - not editable: the **parent** (not null-checked) is asked for `IGZWinCombo` (`0x412CE496`); if it has one,
    `SendMsg(notification target, 7, x, y, modifiers)` is returned (a line input inside a combo box passes its
    click on), the reference released; otherwise true;
  - editable: `GZSetCapture`; on success: without `shift`, `SetCursorLoc(index(x))`; with `shift`, caret
    `:= index(x)` (the anchor stays); end := caret; scroll update; redraw. True.
- **Mouse move** while capturing: `i := index(x)`; if it differs from the end, end := `i`, redraw. The caret does
  not follow. True.
- **Left up** while capturing: release, end := `index(x)`. True.
- **Set focus** (window slot 132): editable → redraw, true; else false. **Kill focus**: redraw, true.
- **`SetFlag(mask, on)`** (window slot 68, `0x009C5B55`): for mask 2 (enabled): off → `SetCursor(null, 1)`; on and
  editable → the I-beam cursor; then `cGZWin::SetFlag`.

## 6. Sounds (`PlaySound(i)`, slot 37)

With a non-zero sound ID `i` and the sound system (`RZ::SS`): its slot 13 `(5, 0x5B523BA4, &obj, id, 0, 0, 0)`;
on success its slot 8 `(obj)` and that result is returned; the object is released either way. Otherwise false.
Index 0 is played for a control character, index 1 at the limit. The constructor does not set the IDs.

*[compat]* The factory allocates the control without clearing its storage (`0x009C6ABE`,
`0x005E55E0`, `0x005B9FC0`). Both this control and the spinner fit the allocator's 512-byte
size class. That class removes a block from a free list without clearing its payload
(`0x005E5CB0`, table `0x00AA4024`, `0x005E16E0`); freeing a block writes the free-list link, not zeros throughout
it (`0x005E5BC0`). Consequently the unwritten sound IDs can retain earlier allocation contents;
the allocator does not establish silence (`0x009C68AC`, `0x009C6096`).

*[choice]* Initialise all three IDs to zero in the replacement. This gives deterministic silence
until configured, without reproducing dependence on previous heap contents (WLI2).

## 7. Lifecycle and painting

**`Init`** (window slot 4, `0x009C59E3`), once:
1. Scroll := 0.
2. With the window-control manager: no font → `SetFont(manager slot 5 (0x68963C4C))`; colour 1 unset →
   `SetFontColor(1, GetColor(0x08963FB9))`; colour 2 unset → `(2, GetColor(0x28963FBC))`; fill colour reading as
   `0xDEADBEEF` (window slot 103) → window slot 106 `SetFillColor(GetColor(0xA8963FB2))`; highlight unset →
   `SetHighlightFillColor(GetColor(0x88963FB4))`. Because the constructor sets colours 1 and 2 to 0 and
   `0xFFFFFFFF`, steps for them only apply after `ClearFontColor`.
3. Editable → the I-beam cursor.
4. Code page := `GetCurrentFrameworkCodePage()`.
5. `SetSize(W, H)` (window slot 53; arguments through two leftover pushes), so the minimum of §3 applies with the
   font's line height.
6. Return `cGZWin::Init`.

**`Shutdown`** (`0x009C5B35`): hide (window slot 70), `SetFont(null)`, `cGZWin::Shutdown`.

**`GZPaint`** (window slot 88, `0x009C6B03`):
1. Filled → fill the draw area with the fill colour (window slot 126; draw context slots 21 and 35).
2. No font → return **false**.
3. Text box `x0 = drawArea.l + gl`, `y0 = drawArea.t + gt`, `x1 = drawArea.r − gr`, `y1 = drawArea.b − gb`; text
   `y = y0 + (y1 − y0 − line height) / 2`. `fit` = the characters from `scroll` that fit in `x1 − x0` (font slot 42).
4. **Focused** (`GetFocus()` is this window):
   - no selection: draw `text[scroll, scroll + fit)` at `(x0, y)` with colour 0, or 1 when disabled (window flag
     2 clear); then, when editable and `caret ≥ scroll`, the caret: black (`0xFF000000`), line width `w = 1`, or 2
     when the line height is above 23 (draw context slot 25), a vertical line (slot 34) at
     `x = x0 + width(text[scroll, caret)) − 1 − w/2` (just `x0` at caret 0) from `y + w` to `y − w + line height`;
     line width back to 1.0;
   - selection: `lo = max(min(anchor, end), 0, scroll)`, `hi = min(max(anchor, end), len, scroll + fit)`; `xs`, `xe`
     their pixel offsets from `scroll`. If the alpha is exactly 1.0, fill `{x0 + xs, y0, x0 + xe, y1}` with the
     highlight colour (any alpha strictly between 0 and 1 draws no highlight). Draw the visible text with colour 0
     (also when disabled), then, if `hi > lo`, `text[lo, hi)` at `x0 + xs` with colour 2.
5. **Not focused**: non-empty text with `fit > 0` → draw it with colour 0 or 1 (disabled).
6. Outline type ≠ 0 → `paint_outline` (`0x009C5FEB`): type 2 `RZDraw3DRectangleOutline(drawArea, c0, c1)`; type 3
   the same, then again 1 pixel inside with `c2, c3`; type 1 `RZDrawRectangleOutline(drawArea,
   ConvertNativeToPackedRGB(c0), 1)`. *Quirk*: type 1 converts the already-packed colour again, and its width 1 is
   the type value left on the stack.
7. Return true.

**`draw_text(x, y, start, count, i)`** (`0x009C69E0`): colour `i` unset → font slot 51 `(buffer to draw to, x, y,
text + start, count, 0x8000000, 0x8000000, 0, 0)`; set → the same with a copy of the substring and the colour in
place of the first `0x8000000`.

## 8. Quirks and cases

Quirks:
1. Backspace erases the wrong bytes and cannot erase the first character (§5.2, WLI1).
2. The sound IDs are never initialised by the constructor and the allocator does not clear them (§6, WLI2).
3. The constructor and a non-editable left click use the control manager and the parent without null checks.
4. A highlight alpha strictly between 0 and 1 draws no highlight (§7).
5. `SetCursorLoc` does not scroll or redraw; `SetOutlineType` does not redraw; `SetCharacterLimit` does not cut.
6. The outline type 1 colour conversion (§7).
7. A drag selection leaves the caret at the mouse-down point.

Cases:
1. Editable, empty, limit 3: characters `a`, `b`, `c` give `"abc"`, caret 3, three changed notifications; `d` plays
   sound 1 and changes nothing.
2. `"hello"`, caret 5, Backspace → `"helo"`, caret 4; at caret 1 (`"h|ello"`) Backspace leaves the text and moves
   the caret to 0.
3. `"hello"`, `SetSelection(1, 3)` → anchor 1, end and caret 4; typing `X` → `"hXo"`, caret 2.
4. `SetText("abcdef")` with limit 4 → `"abcd"`, caret 0, one changed notification.
5. Default gutters, outline type 1, line height 12: minimum size `(6, 14)`.

## 9. Function inventory (owner `cGZWinLineInput`, 58)

`0x009C59B6` QI, `0x009C59E3` `Init`, `0x009C5B35` `Shutdown`, `0x009C5B55` `SetFlag`, `0x009C5BBA` slot 4,
`0x009C5C01` slot 6, `0x009C5C11` slot 5, `0x009C5C48` slot 7, `0x009C5C6B` slot 8, `0x009C5C7C` slot 9,
`0x009C5C96` slot 10, `0x009C5C9A` slot 16, `0x009C5CAF` slot 13, `0x009C5CBD` slot 11, `0x009C5CC9` slot 14,
`0x009C5D0E` slot 15, `0x009C5D3C` slot 17, `0x009C5D68` slot 18, `0x009C5D86` slot 19, `0x009C5DC7` slot 20,
`0x009C5DD5` set focus, `0x009C5DEF` kill focus, `0x009C5DFC` slot 24, `0x009C5E01` slot 27, `0x009C5E05`
slot 28, `0x009C5E50` slot 29, `0x009C5E5C` slot 30, `0x009C5E6A` slot 32, `0x009C5E84` slot 33, `0x009C5EB3`
slot 34, `0x009C5EDC` slot 35, `0x009C5F34` slot 36, `0x009C5F4E` completed, `0x009C5F72` changed,
`0x009C5F96` `SetArea`, `0x009C5FEB` `paint_outline`, `0x009C6096` slot 37, `0x009C610B`
`FindTextIndexFromCursorLocation`, `0x009C61D1` `UpdateViewLocation`, `0x009C62C9` slot 25, `0x009C62EB`
slot 21, `0x009C6305` slot 31, `0x009C636A` left down, `0x009C641D` left up, `0x009C6447` mouse move,
`0x009C647A` `DeleteChar`, `0x009C64B7` key down, `0x009C68AC` constructor, `0x009C69AA` QI thunk, `0x009C69B5`
destructor, `0x009C69E0` `draw_text`, `0x009C6A7E` slot 22, `0x009C6ABE` `CreateNewObject`, `0x009C6AE7`
deleting destructor, `0x009C6B03` `GZPaint`, `0x009C6F14` `InsertChar`, `0x009C6F5A` character, `0x009C701E`
slot 23.

Not covered: the font slots (37, 42, 48, 51) and the sound-system slots, cited by number.
