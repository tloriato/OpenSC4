# `nSC4UI`: the UI helper functions (behaviour spec)

Scope: the `nSC4UI` owner as tools/coverage.py assigns it (25 functions, 6,802 bytes on `5c0ff782`; inventory in
§8). These are free functions shared by many windows:
- the generic modal dialogs (notification, yes/no, scripted);
- window alignment and child sets;
- the catalog item and catalog view builders used by the 3D view's menus;
- the exemplar token formatter;
- the palette ramp of the minimap.

Their callers are specified elsewhere: view3d-window.md §6i, advisor-window.md, budget-window-bodies.md,
neighbors.md NG35, region-city-dialogs.md, modal-dialogs.md, ui-windows.md §4.3–§4.4.

Evidence: static RE only, on the instruction and data bytes of the Steam 1.1.641 exe (the same code as GOG,
`docs/lanes/re-dates.md`). Every function was disassembled. The variant jump table (`0x0078D950`) was read from the
data section. Written 2026-10-23 (re-uihelpers7).

Conventions:
- All public functions are cdecl.
- "Main window" is `RZ::WM()` slot 3.
- Window slots are `cIGZWin` slots (ui-windows.md §2.2):
  - 35 `GetChildWindowFromIDRecursive`;
  - 37 `GetChildAsRecursive`;
  - 41/42 width/height;
  - 50 `GetAreaAbsolute` (left, top, right, bottom);
  - 56 `GZWinMoveTo`;
  - 57 `GZWinOffset`;
  - 60 `WindowToScreenCoordinates`;
  - 63 `GetID`;
  - 73 `GetCaption`.
- `EnumChildren` (slot 32) is as in widget-toolkit.md: it visits the direct children and never stops early.
- Divisions are signed and round toward zero.

## 1. Property readers (register calling convention)

Three internal helpers read one exemplar property through `cISCPropertyHolder` slot 9 (`GetProperty(id)`), then
the property's variant (property slot 5). The variant's type is its slot 3; bit 15 marks an array, the low 15 bits
are the type:

| Type | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Kind | bool | uint8 | sint8 | uint16 | sint16 | uint32 | sint32 | uint64 | sint64 | float32 | float64 |

- **`0x0078D810`** (holder in ECX, ID in EDX, index in EDI, `uint32*` out in EBX; AL result). Without the property,
  false. For an array, a negative index fails; a non-negative index is **not** checked against the count (variant
  slot 4) *[quirk]*. Element `index` of the typed data (variant slots 93–113, one per type) is converted:
  - bool → 0 or 1;
  - the 8- and 16-bit types are zero- or sign-extended;
  - the 32-bit types are copied;
  - the 64-bit types keep their low dword;
  - both floats are truncated (`_ftol2`).

  Any other type: false.
- **`0x0078D980(holder, id)`** (out pointer in EAX; AL result): an integer read.
  - Without the property: false.
  - An array: `0x0078D810` with index 0. An empty array is therefore read without a check.
  - Otherwise: the variant's `GetValSint32(out)` (slot 28).
- **`0x0078D9E0`** (holder in ECX, ID in EDX, `float*` out in EDI; AL result): a float read.
  - Type 10: an array gives element 0 when the count is above 0 (else false); otherwise `AsFloat32` (slot 80).
  - Type 11: the same, with `AsFloat64` (slot 82). The value is rounded to float32.
  - Other types: false.

## 2. `PropertyTokenReplacementFunction(token, out, holder)` (`0x0078EAC0`)

A token callback for the string detokenizer (§3.1). AL is true when the token was replaced.
1. Fetch the language utility: `RZ::LM()` slot 38 `(0)`.
2. The token must be exactly 10 characters with `:` at index 1, and must start with `m`/`M` (money) or `d`/`D`
   (decimal). Otherwise the result is false.
3. `id` := the 8 characters after the colon, read as hexadecimal (`RZAtoUint32HexA`).
4. By kind, the float read is tried first, then the integer read; when both fail the result is false:

| Kind | Float read (§1) succeeds | Else integer read succeeds |
|---|---|---|
| `d` | `out` := `Sprintf("%.2f", value)` (string slot 13) | `out` := the language utility's slot 18 `(value as a sign-extended int64, &s)`, then `s` |
| `m` | `SC4MakeFormattedMoneyString(value, out, "%.2f")` (`0x005F9B50`) | `SC4MakeMoneyString(sign-extended int64, out)` (`0x005F9AB0`) |

For example `d:12345678` with a float32 property 2.5 gives `2.50`. `m:12345678` with a uint32 property 1500 gives
the money string of 1500.

## 3. Catalog items and views

*[compat]* The property roles below follow executable consumers: `0x0078EF30` compares the
menu property and uses the order property as the map key; `0x0078EC90` loads the text keys
and fallback strings and supplies icon/button values to the catalog item. This verifies those
roles independently of a human-readable property-name table. The shipped names themselves
are not established from these consumers and remain UIH3.

### 3.1 `CreateCatalogItemFromExemplar(exemplar, window, kind, detokenizer)` (`0x0078EC90`)

`P` is the exemplar's property holder (exemplar slot 7). The function returns a new `cSC4CatalogItem` with one
reference, or null.
1. Register the token callback: detokenizer slot 7 `AddUnknownTokenReplacementMethod(§2, P, 1)`.
2. **Name**, the first that succeeds:
   1. `SC4LoadExemplarString(&name, P, 0x8A416A99, detokenizer)` (a text key);
   2. the string property `0x899AFBAD`;
   3. the string property `0x20` (the exemplar name).

   When all three fail, return null. The callback stays registered *[quirk]*.
3. **Description**: `SC4LoadExemplarString(&desc, P, 0xCA416AB5, detokenizer)`, else the string property
   `0x8A2602A9`.
4. Read two integer properties:
   - icon := property `0x8A2602B8` (default `0x144161EC`);
   - button := property `0x8A2602BB` (default `0xFFFFFFFF`).
5. Remove the callback (detokenizer slot 7 with 0).
6. Load the icon image: `cSLBuffer` with the key `{0x856DDBAC, 0x6A386D26, icon}`.
7. Construct the item (0x44 bytes, `0x0079B6B0`):

   | Field | Value |
   |---|---|
   | `+0x34` | `kind` |
   | `+0x38` | `button`, or the exemplar's own instance (exemplar slot 11, `GetKey`) when `button` is `0xFFFFFFFF` |
   | `+0x04` | the name |
   | `+0x18` | the description |
   | `+0x30` | the image, when there is one; its slot 23 first gets `window`'s `MakeFillColor(255, 0, 255)`, the magenta colour key |
   | `+0x2C` | 0 |

8. `SetEnabled(1)` (item slot 10).

### 3.2 `CreateCatalogItemList(map, view, group, kind, menu, required)` (`0x0078EF30`)

`map` is a `std::multimap<int, cISC4CatalogItem*>`.
1. Make a `cSCStringDetokenizerDefault` (0x94 bytes).
2. List the resource keys of type `0x6534284A` (exemplars) in `group`: resource manager `0x00B43CA8`, slot 17,
   with a key filter. Without the manager or without keys, stop.
3. For each key in list order:
   1. Get the exemplar (manager slot 3 with IID `0xA52160F5`).
   2. Skip it unless its property `0x8A2602BA` (default 0) equals `menu`.
   3. When `required` is not 0, also skip it unless the holder has the property `required` (holder slot 3
      *[inferred]*).
   4. `item := CreateCatalogItemFromExemplar(exemplar, view's window, kind, detokenizer)`. The view's window is
      view slot 3.
   5. When it exists, insert `{order, item}` into `map`, with `order` := the property `0x8A2602B9` (default 1000).
4. Release the key list and the detokenizer.

`insert_equal` puts an item after the items of equal order, so equal orders keep the key list order.

### 3.3 `CreateCatalogView(group, kind, menu, required)` (`0x0078F100`)

1. Construct a `cSC4WinCatalogView` (0x140 bytes, `0x0079B500`).
2. Fill a local multimap with `CreateCatalogItemList` (§3.2).
3. Walk the map in ascending order. Each item is added to the view (view slot 4, which takes its own reference),
   then released.
4. Free the map and return the view.

So the menu shows the group's exemplars for that menu value, sorted by item order. For example, the 3D view passes
group `0x6A01FC2A` with menu 0 (`0x007EC636`).

## 4. Windows

- **`AlignWindowToWindow(win, marker, target)`** (`0x0078D7A0`, three windows):
  1. `(ax, ay)` := `target`'s screen position of its origin; `(bx, by)` := `marker`'s (slot 60).
  2. `win.GZWinOffset(ax − bx, ay − by)`.
  3. True.

  This moves `win` so that its descendant `marker` lands on `target`.
- **`AlignWindowToWindow(winID, markerID, targetID, root)`** (`0x0078DB20`):
  1. `root` := `root`, or the main window when null.
  2. `w` := `root`'s child `winID` (recursive; there is **no** null check) *[quirk]*.
  3. `m` := `w`'s child `markerID` (recursive). When `m` is null: false.
  4. With `targetID` ≠ 0: `t` := `root`'s child `targetID` (recursive). When `t` is null: false. Otherwise the
     three-window form `(w, m, t)` and true.
  5. With `targetID` = 0, read `m`'s caption `c`:
     - **`c` has length ≥ 3 and `c[0]` = `-`**: `v` := the index of `c[1]` in `"tmb"` and `h` := the index of
       `c[2]` in `"lcr"`. When either character is not found: true, without moving. Otherwise, with `M` the main
       window's absolute area and `A` = `m`'s:
       `w.GZWinOffset(((M.width − A.width)·h)/2 − A.left, ((M.height − A.height)·v)/2 − A.top)`, then true.
       `M`'s own left and top are not added *[quirk]*.
     - **Otherwise**: `c` is read as a hexadecimal window ID. 0 gives false; any other ID is `targetID` in step 4.

  ui-windows.md §4.3 calls it with the marker ID `0xAAAA`. So a script window aligns itself through a marker child
  `0xAAAA`, whose caption holds either `-` plus a vertical and a horizontal letter (top/middle/bottom,
  left/centre/right) or the ID of a window to cover. Example: caption `-mc` on a 200 × 100 marker at absolute
  (10, 20), main window 800 × 600: offset `(300 − 10, 250 − 20)` = `(290, 230)`.
- **`AlignWindowSetToWindow(set, root)`** (`0x0078E610`): for each window of the `std::set<cIGZWin*>` in
  ascending pointer order, `AlignWindowToWindow(window.GetID(), 0xAAAA, 0, root)`. True.
- **`GetWindowChildrenSet(win, set)`** (`0x0078E990`):
  1. Clear `set`.
  2. `win.EnumChildren(0x22BA0121, add, set)`, where the callback `0x0078E960` inserts each child and returns
     true. Only the direct children are added.
  3. Return the set's size.
- **`ClearAcceptFocusFlagRecursive(win)`** (`0x0078DAC0`): `SetFlag(0x8000, 0)` on `win`, then on every
  descendant, through the callback `0x0078DA80`. The callback does the same for each child and its children, and
  returns false; the walk does not stop early, so all descendants are reached.

## 5. Generic modal dialogs

The three dialogs share one sequence. In step 2, each creates its window with the main window as parent:

| Dialog | Creation | Result |
|---|---|---|
| `GenericNotificationDialog(title, text)` (`0x0078DD80`) | `SC4CreateWindowFromScript` (`0x005F9390`) with key `{0, 0x96A006B0, 0xCA8CBF0F}`, ID `0xAA8DEF97` | none |
| `GenericConfirmationDialog(title, text, yes, no)` (`0x0078DFF0`) | the same with key `{0, 0x96A006B0, 0xEA8CC3C6}`, ID `0x8A8DFCF5` | AL true when the result is `0x8A8DFD4E` (Yes), else false |
| `DoScriptedModalDialog(instance, id)` (`0x0078E2F0`) | `SC4CreateWindowFromScript` (`0x005F9480`) with key `{0, 0x96A006B0, instance}`, ID `id`, IID `0x22BA0121` | EAX = the modal result, or 0 when not shown |

1. When `SC4GetPopupModalDialogsEnabled()` is false, return at once: no result, false, or 0 (neighbors.md NG35).
2. Read three things:
   - the main window, through the framework service `0xA417445E` (IID `0x5A4`) slot 3;
   - the window manager `RZ::WM()`;
   - its focus window (slot 36 `GZGetFocus`).

   Then create the dialog. When creation fails, return the same "not shown" value.
3. Hold a `cSC4AutoGamePause(0, 0, 1, 1)` for the rest of the call (modal-dialogs.md §4).
4. `PullToFront`. Move the dialog to `((main width − width)/2, (main height − height)/3)` (slot 56).
5. When the dialog has `cIGZWinGen` (`0x5386D516`): install a window procedure through generator slot 7. It is an
   8-byte object with vtable `0x00AB55C0` and its count at `+4`.
6. Set the texts. The text children are taken with `GetChildAsRecursive(id, 0x212CDC1F)`; their slot 4 gets the
   string. The buttons are taken with the button interface `0x8810`; their slot 49 gets the label, when the
   argument is not null. *[compat]* Both are caption setters: text slot 4 reaches
   `cGZWinText::SetCaption`, which stores the caption, recalculates text placement and
   invalidates the window (`0x009C160F`, interface table `0x00AE0118`, thunk `0x007BE730`,
   body `0x009C20B2`). Button slot 49 similarly stores the caption, updates label presence
   and size, and lays out the button (`0x009AFFD3`, table `0x00ADDD50`, `0x009B14E4`; UIH2).
   - Notification: `title` into `0xCA8CC4B6`, `text` into `0x4A8CC4A9`.
   - Confirmation: `title` into `0x8A91E4FD`, `text` into `0x6A8CC495`; `yes` into button `0x8A8DFD4E`, `no` into
     button `0x8A8DFD4A`.
7. The window-location store `0x00B43CE0` is `cSLWinLocationSaver` *[compat]*
   (`0x0044B4E0`, `0x00603650`, table `0x00AA6E74`). Slot 10 `(dialog, 1)` restores the
   position saved under the window ID (`0x00603710`, `0x00603A20`). With a parent, a saved
   position is accepted when left ≤ parent width − 8, top ≤ parent height − 8, right ≥ 8
   and bottom ≥ 8; otherwise it moves to `(8, 8)`. Without a saved entry the initial
   position is kept (UIH1).
8. Run the dialog: `result` := `DoModalWin(dialog)` (manager slot 41).
9. The location store slot 6 `(dialog)` saves its parent-relative left/top under the window ID
   (`0x00603680`, `0x00603D70`) *[compat]*. The saver imports its map from up to 200
   preferences entries on initialisation and exports it on shutdown (`0x00603810`,
   `0x00603910`; UIH1).
10. Detach the procedure (generator slot 7 with 0) and release it. Destroy the dialog (manager slot 23).
11. When the old focus is still valid (manager slot 24 `IsWindowValid`), give it back the focus (slot 37).
12. Release the dialog. The pause ends.

**The window procedure** (`DoWinProcMessage`, `0x0078DCE0`):
- For a command (kind 3) with ID `0x287259F6` (a button), `RZ::WM()` `GZWinMsgPost(0xF, win, button ID)`. This
  ends the modal loop with the button's ID. True.
- Anything else: false.

Its deleting destructor (`0x0078DD20`) frees it from the global heap when bit 0 of its argument is set.

## 6. Set helpers

`std::set<cIGZWin*>` instantiations (nodes of 0x14 bytes: colour, parent, left, right, value):
- `_M_copy` (`0x0078E560`);
- `operator=` (`0x0078E690`);
- insert with a hint (`0x0078E750`);
- the insert iterator's assignment (`0x0078E9E0`);
- a copy into an insert iterator (`0x0078EA50`).

Their behaviour is the standard one; the order is the unsigned pointer order.

## 7. Quirks and cases

Quirks:
- `0x0078D810` does not check a non-negative index against the array length, so an empty array is read at element 0.
- `AlignWindowToWindow` uses `w` without a null check.
- The `-XY` alignment ignores the main window's own left and top.
- A nameless exemplar leaves the token callback registered on the shared detokenizer.

| Case | Setup | Expected |
|---|---|---|
| UIH-C1 | marker caption `-br`, marker 100 × 40 at absolute (5, 5), main 640 × 480 | `v` = `h` = 2: offset `(540 − 5, 440 − 5)` = `(535, 435)` |
| UIH-C2 | marker caption `5A2B` | aligned onto window `0x5A2B`; caption `0` or `zz` gives false |
| UIH-C3 | three exemplars in the group with orders 20, (none), 20 and menu 0 | order 20, 20 (list order), then 1000 |
| UIH-C4 | token `M:0000ABCD`, property `0xABCD` sint32 −12 | `SC4MakeMoneyString(−12)` |
| UIH-C5 | confirmation, popups disabled | false, no window |

## 8. Function inventory (owner `nSC4UI`)

All 25 functions (6,802 bytes):
- §1: `0x0078D810`, `0x0078D980`, `0x0078D9E0`;
- §2: `0x0078EAC0`;
- §3: `0x0078EC90`, `0x0078EF30`, `0x0078F100`;
- §4: `0x0078D7A0`, `0x0078DB20`, `0x0078E610`, `0x0078E990`, `0x0078E960`, `0x0078DAC0`, `0x0078DA80`;
- §5: `0x0078DD80`, `0x0078DFF0`, `0x0078E2F0`, `0x0078DCE0`, `0x0078DD20`;
- §6: `0x0078E560`, `0x0078E690`, `0x0078E750`, `0x0078E9E0`, `0x0078EA50`;
- `InterpolateRampToPalette` (`0x0078D520`), specified in minimap.md §4.2.

Not covered:
- `cSC4WinCatalogView` and `cSC4CatalogItem` (their own owners);
- the detokenizer;
- `SC4LoadExemplarString` and the money formatters;
- the window-location store beyond the restoration and persistence boundary in §5 (UIH1);
- the text interface beyond its caption setter (§5, UIH2);
- the property names (UIH3).
