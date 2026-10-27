# `cSC4WinCatalogView` and `cSC4CatalogItem`: the catalog list (behaviour spec)

Scope: the `cSC4WinCatalogView` and `cSC4CatalogItem` owners as tools/coverage.py assigns them (inventory in §6).
The catalog view is the vertical list of item icons in the 3D view's building, transport and other menus
(view3d-window.md §6i). It:
- draws each item's icon in one of four states;
- highlights the item under the cursor and shows its callout;
- posts a command when an item is clicked;
- scrolls, and can flash an item for the tutorial.

`nSC4UI::CreateCatalogView` builds and fills it (ui-helpers.md §3).

Evidence: static RE only, on the instruction and data bytes of the Steam 1.1.641 exe (the same code as GOG,
`docs/lanes/re-dates.md`). Every function was disassembled; the vtables were read from the data section. Written
2026-10-23 (re-catview7).

Conventions:
- `V` is the view (its base).
- `W` = `V+4` is its `cGZWin`; window field offsets are as in widget-toolkit.md §2, relative to `W`.
- `w`, `h`, `s` are the cell width, cell height and spacing (§1).
- `pitch` = `h + s`.
- `visible` = `(W's area height + s) / pitch` (signed division).
- Item `i` is the `i`-th entry of the item vector; an item's ID is its slot 8.
- Sounds go through the sound system `0x00B43CB8` slot 11 `(1, sound, 0, 0, 0)`, when it exists:
  - `0x2A5C322B` is the click and scroll sound;
  - `0xAA62C9D5` is the refusal sound.

## 1. Identity and fields *[compat]*

| Offset | vtable / field |
|---|---|
| `+0x000` | the catalog view interface, vtable `0x00AB6D28` (IID `0x49D98264`, §2) |
| `+0x004` | `cGZWin`, vtable `0x00AB6D88` (§3) |
| `+0x0DC`…`+0x0E4` | `std::vector<cISC4CatalogItem*>` (begin, end, capacity), each item counted |
| `+0x0E8` | the item count |
| `+0x0EC` | the scroll position: the first visible item |
| `+0x0F0` | the highlighted index (−1 = none) |
| `+0x0F4` | the selected index (−1 = none) |
| `+0x0F8`, `+0x0FC`, `+0x100` | `w`, `h`, `s` |
| `+0x104` | the callout font (counted) |
| `+0x108` | the callout (a `cSC4WinCalloutBox` interface, counted; callout-box.md) |
| `+0x10C` | an object released at shutdown (not written by this owner) |
| `+0x110` | a `cRZCallbackTimer` for flashing |
| `+0x138` | the flash count |
| `+0x13C` | the flash index |

- **Constructor** (`0x0079B500`): the `cGZWin` base constructor; everything 0; both indexes −1; the timer.
- **`QueryInterface`** (`0x0079A080`): IID `0x49D98264` gives `V` (with an `AddRef`); any other IID goes to
  `cGZWin::QueryInterface` on `W`.
- **Destructor** (`0x0079B5B0`, deleting `0x0079B620`, thunk `0x0079B5A0`):
  1. `Shutdown` (§3);
  2. the timer's destructor;
  3. release `+0x10C`, `+0x108` and `+0x104` (already cleared by `Shutdown`);
  4. free the vector's buffer;
  5. `~cGZWin`;
  6. `GlobalHeapFree` when bit 0 of its argument is set.

**`cSC4CatalogItem`** (0x44 bytes, constructor `0x0079B6B0`, vtable `0x00AB6FE4`):

| Offset | Field | Slot |
|---|---|---|
| `+0x04` | name (`cRZString`) | 3 (its address) |
| `+0x18` | description (`cRZString`) | 4 `GetDescription` (`0x0082ACF0`, its address) |
| `+0x2C` | cost | 5 |
| `+0x30` | icon image (counted) | 6 |
| `+0x34` | kind | 7 |
| `+0x38` | ID | 8 |
| `+0x3C` | reference count | 1 `AddRef` (`0x0079A070`), 2 `Release` (`0x0079B820`) |
| `+0x40` | enabled byte, 1 after construction | 9 `GetEnabled` (`0x0079B730`), 10 `SetEnabled` (`0x0079B740`) |

The item's other functions:
- `QueryInterface` (`0x0079A040`) answers IIDs `0x49D98263` and 1 with the item.
- `Release` decrements a count above 1 and returns the new count. At 1 or 0 it runs the destructor (`0x0079B750`:
  releases the image, destroys the strings), frees the item and returns 0.

## 2. The view interface (`0x00AB6D28`)

| Slot | Address | Behaviour |
|---|---|---|
| 3 | shared | `W` |
| 4 | `0x0079B7B0` | `AddItem(item)`: `AddRef`, append, count + 1 |
| 5 | shared | one argument, returns false |
| 6 | `0x0079A3F0` | the item with this ID, or null (no `AddRef`) |
| 7 | `0x0079A5E0` | the highlighted item, or null when the index is out of range |
| 8 | `0x0079A600` | the selected item, or null when the index is out of range |
| 9 | `0x0079A820` | the selected item's ID, or 0 |
| 10 | `0x0079A850` | `(id)`: selected := the index of that ID, or −1. No repaint |
| 11 | `0x0079B640` | remove all: release every item, empty the vector, count := 0. The indexes, scroll and flash are **not** reset *[quirk]* |
| 12 | `0x0079A0E0` | `(w, h, s)` |
| 13 | `0x0079A620` | `(&size, n)`: `size` := `(w, n·pitch − s)`, or `(w, 0)` when `n` is 0 |
| 14 | `0x0079B580` | the count |
| 15 | `0x0079A7D0` | `(font ID)`: `+0x104` := `RZ::WCM` slot 5 `(id)` |
| 16 | `0x0079A660` | `SetScrollPos(p)` (below) |
| 17 | shared | the scroll position |
| 18 | `0x0079A430` | `(id, on)`: every item **except** the one with that ID gets `SetEnabled(on)`. The result is true when that ID was found |
| 19 | `0x0079A480` | `(id, &x, &y)`: the callout point of that item (below), in screen coordinates. False when the ID is absent |
| 20 | `0x0079A8A0` | `(id)`: make the item visible. False when absent. Otherwise, by its index `i`: when `i` < scroll, `SetScrollPos(i)`; when `i` ≥ scroll + `visible`, `SetScrollPos(i − visible + 1)`. True |
| 21 | `0x0079A510` | `FlashItem(id, count, ms)` (below) |
| 22 | `0x0079A380` | `SetCallout(c)` (below) |

**`SetScrollPos(p)`**:
1. `p` := min(`p`, count − `visible`), then max(`p`, 0).
2. When `p` changed, store it and invalidate `W` (slot 92).
3. Then, always: when `W`'s parent answers IID `0x8A25CE2C`, its slot 7 gets `(p > 0, p < count − visible)`. These
   update the tertiary menu frame's up/down arrow availability *[compat]*.

The receiver is `cSC4WinTertiaryMenuFrame`: its constructor installs interface table
`0x00AB6D04` (`0x0079AFF0`), and its interface query accepts `0x8A25CE2C`
(`0x0079A110`). Slot 7 stores the two availability bytes and invalidates the frame when either
changes (`0x0079B080`). Painting selects the corresponding arrow images from those bytes
(`0x0079B0E0`). These are frame state, rather than separate child-button enable calls.
The notification runs even when the clamped scroll position did not change (`0x0079A660`).

**The callout point** of item `i`:
- `x` = `w` + 16;
- `y` = `(i − scroll)·pitch + h/2`;
- converted to the screen with `WindowToScreenCoordinates` (slot 60).

So the callout points from just right of the icon, at the middle of its row.

**`FlashItem(id, count, ms)`**:
- With `id` = −1: stop the timer if it runs; flash index := 0; flash count := `count`; false.
- Otherwise, find the item's index; when it is absent, false. Then:
  1. stop the timer if it runs;
  2. set its callback (`0x0079A0B0`) with the view;
  3. flash index := the index; flash count := `count`;
  4. start the timer `(ms, 0)`;
  5. true.

Each timer call posts `0xAA5187A4` to `W` itself (`PostMsg`, window slot 146).

**`SetCallout(c)`**:
1. When there is an old callout, `W` deletes its window (`ChildDelete`, slot 16).
2. `+0x108` := `c`, counted.
3. When `c` is not null, `W` adds `c`'s window as a child (slot 14).

## 3. The window (`0x00AB6D88`)

| Slot | Address | Behaviour |
|---|---|---|
| 3 | `0x0079A760` | `DoMessage`, below |
| 4 | `0x0079A280` | `Init`, below |
| 5 | `0x0079A2E0` | `Shutdown`, below |
| 88 | `0x0079AA70` | `GZPaint`, below |
| 136 | `0x0079A940` | `GZOnMouseUpL` (§4) |
| 138 | `0x0079BA40` | `GZOnMouseMove` (§4) |
| 139 | `0x0079A9F0` | the mouse wheel (§4) |
| 142 | `0x0079BAA0` | `GZOnMouseExit`: `SetHighlightedItem(−1)`, true |

Window slot 151 is the same body as the item's `QueryInterface` (identical code).

- **`Init`**: true at once when already initialised. Otherwise:
  1. `SetFlag(0x10000, 0)` and `SetFlag(4, 1)`;
  2. `cGZWin::Init`;
  3. when there is no font, `SetFont(0x68963C4C)` (interface slot 15);
  4. `SetScrollPos(0)`;
  5. true.
- **`Shutdown`**: true at once when not initialised. Otherwise:
  1. stop the timer when it runs;
  2. remove all items (interface slot 11);
  3. release the font;
  4. delete the callout's window (`ChildDelete`) and release the callout;
  5. release `+0x10C`;
  6. tail-call `cGZWin::Shutdown`.
- **`DoMessage(msg)`**:
  - `0xAA5187A4` (the flash tick): when the flash count is above 0, decrement it; otherwise stop the timer. True.
    The tick does not invalidate the window.
  - `0xCA35E4BA`: `ChangeScrollPos(data1)` (`0x0079A710`), true. `ChangeScrollPos(d)` is `SetScrollPos(scroll +
    d)`, plus the scroll sound when the position changed.
  - Anything else: `cGZWin::DoMessage`.
- **`GZPaint`**:
  1. `rows` := `visible`.
  2. Start `y` at the top of `W`'s draw area.
  3. For each row `r` < `rows` with item `i` = scroll + `r` < count:
     1. Without an icon (item slot 6), skip the item **without advancing `y`** *[quirk]*.
     2. Pick the frame `f`:
        - 0 when the item is disabled;
        - else 3 when `i` is highlighted, or when `i` is the flash index and the flash count is odd;
        - else 2 when `i` is selected;
        - else 1.
     3. Blit the icon's source rectangle `(f·w, 0, f·w + w, h)` to `(draw left, y, draw left + w, y + h)` on `W`'s
        draw buffer (`+0x68`): buffer slot 6 `(0x8010)` locks it, slot 29 `(image, &src, &dst, 0)` blits it, slot 7
        `(0x8010)` unlocks it. The blit happens only when the lock succeeds.
     4. `y` += `pitch`.
  4. True.

  So an icon is a strip of four frames, `w` wide each: disabled, normal, selected, highlighted.

## 4. Mouse and highlight

**`GZOnMouseMove(x, y, flags)`**:
1. `r` := `(y − s/2) / pitch` (signed, toward zero).
2. When `r` ≥ 0 and scroll + `r` < count, `SetHighlightedItem(scroll + r)`; otherwise `SetHighlightedItem(−1)`.
3. True. `x` is not used.

**`GZOnMouseUpL`** (three arguments). With a highlighted, enabled item:
1. Post to `W`'s notification target (`W+0x4C`) a command message: `PostMsg(target, 3, 0x49D993D4, kind, ID)`. The
   kind and ID are item slots 7 and 8. The 3D view runs it as `InvokeCatalogItemCommand` (view3d-window.md §6i).
2. Selected := highlighted.
3. The click sound.

Otherwise the refusal sound. True in both cases.

**The mouse wheel `(x, y, flags, delta)`**:
1. `n` := `delta / 300` (toward zero).
2. `SetScrollPos(scroll − n)`, with the scroll sound when it moved.
3. Return `GZOnMouseMove(x, y, flags)` (slot 138), which refreshes the highlight.

**`SetHighlightedItem(i)`** (`0x0079B850`):
1. Nothing when `i` is already highlighted.
2. Store `i`; invalidate `W`.
3. Without a callout, stop.
4. With `i` < 0, hide the callout's window (slot 70) and stop.
5. Otherwise the callout shows item `i`:
   1. `r` := the callout's bounds (callout slot 20).
   2. The title is the item's name. When the cost (item slot 5) is above 0 and the view has a font, it becomes
      `name + " - " + SC4MakeMoneyString(cost)`.
   3. Compute the callout point `(x, y)` (§2); `r.left` := `x`.
   4. Callout slot 6 gets the title; slot 8 the description (item slot 4).
   5. Slot 15 gets the point; slot 17 the offset `(0, 0, 0, 0)`; slot 19 the bounds `r`.
   6. Show the callout's window (slot 69).

## 5. Quirks and cases

Quirks:
- An item without an icon is skipped in painting without advancing `y`. The later icons shift up, while hit testing
  still uses full rows.
- Interface slot 18 changes every item **but** the named one.
- Removing all items keeps the highlight, selection, scroll and flash indexes.
- The flash tick does not repaint.
- `r` truncates toward zero, so a point up to one pitch above the first row still hits row 0.

| Case | Setup | Expected |
|---|---|---|
| CV-C1 | `w` = `h` = 44, `s` = 5, area height 250, 10 items | `visible` = 255/49 = 5; `SetScrollPos(9)` stores 5; arrows `(true, false)` |
| CV-C2 | same, scroll 2, cursor `y` = 100 | `r` = (100 − 2)/49 = 2 → item 4 highlighted; callout point `(60, 2·49 + 22)` = `(60, 120)` before conversion |
| CV-C3 | item 3 selected, item 4 highlighted, item 5 disabled | frames 2, 3, 0 |
| CV-C4 | tutorial reset walker on the view (tutorial-window.md §5.1) | slot 18 `(−1, 1)`: every item enabled (no item has ID −1) |
| CV-C5 | wheel delta −600 | scroll + 2 |

## 6. Function inventory

Owner `cSC4WinCatalogView` (34 functions):
- §1: `0x0079B500`, `0x0079A080`, `0x0079B590`, `0x0079B5A0`, `0x0079B5B0`, `0x0079B620`;
- §2: `0x0079B7B0`, `0x0079A3F0`, `0x0079A5E0`, `0x0079A600`, `0x0079A820`, `0x0079A850`, `0x0079B640`,
  `0x0079A0E0`, `0x0079A620`, `0x0079B580`, `0x0079A7D0`, `0x0079A660`, `0x0079A430`, `0x0079A480`,
  `0x0079A8A0`, `0x0079A510`, `0x0079A0B0`, `0x0079A380`;
- §3: `0x0079A280`, `0x0079A2E0`, `0x0079A760`, `0x0079A710`, `0x0079AA70`;
- §4: `0x0079BA40`, `0x0079A940`, `0x0079A9F0`, `0x0079BAA0`, `0x0079B850`.

Owner `cSC4CatalogItem` (8 functions): `0x0079B6B0`, `0x0079B750`, `0x0079A040`, `0x0079A070`, `0x0079B820`,
`0x0079B730`, `0x0079B740`, `0x0082ACF0`.

Not covered:
- the rest of the tertiary menu frame beyond its arrow-availability receiver (§2, CV1);
- the draw buffer's lock flags (CV2).
