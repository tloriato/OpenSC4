# `cSC4CheatCodeEntryDialog`: the cheat code dialog (behaviour spec)

Scope: the `cSC4CheatCodeEntryDialog` owner as tools/coverage.py assigns it (33 functions, 6,403 bytes on
`5c0ff782`; inventory in §7). Its 16 own functions are the dialog. The other 17 are the `std::sort` instantiation
over its list of cheat names (§5).

The application opens the dialog on its command `0x2A9496C9` (`OpenCheatCodeDialog`, view3d-window.md; handler
`0x004E0BB0` of `cWinSC4App`):
1. allocate 0x120 bytes and construct;
2. run it modally (window manager slot 41, `DoModalWin`);
3. destroy it (manager slot 23);
4. release it.

The cheats themselves belong to the cheat code manager (global `0x00B43CAC`, class `cGZCheatCodeManager`):
- `DoDefaultCheatCodeProcessing` (slot 22) runs a cheat text;
- `GetListOfAllCheatStrings` (slot 13) lists the names.

Evidence: static RE only, on the instruction and data bytes of the Steam 1.1.641 exe (the same code as GOG,
`docs/lanes/re-dates.md`). Every function was disassembled; the vtables were read from the data section. Written
2026-10-23 (re-cheatdlg7).

Conventions:
- `D` is the dialog.
- `E` is its text edit (`D+0x110`, interface `IGZWinTextEdit` `0x231A1C57`; slots as in text-edit.md).
- `M` is `D`'s window manager (`D+4`).
- `H` is the history: a global `std::vector<cRZString>` (20-byte elements, begin `0x00B4DFDC`, end `0x00B4DFE0`)
  shared by every dialog and kept for the whole session.
- Window slots are `cIGZWin` slots (ui-windows.md §2.2).

## 1. Identity and fields *[compat]*

`cSC4CheatCodeEntryDialog` derives from `cGZWinGenDlg`.

| Offset | vtable | Notes |
|---|---|---|
| `+0x000` | `0x00AB7078` (window) | slot 130 `GZOnKeyDown` (`0x0079C400`), slot 135 `GZOnMouseDownR` (`0x0079C550`) |
| `+0x0D8` | `0x00AB7020` | the `cGZWinGenDlg` interface (slot 0 thunk `0x0079BC30`; slot 13 `AddTextEdit`) |
| `+0x10C` | `0x00AB7010` | the message filter (IID `0xC6AE7085`); slots 0–2 thunks (`0x0079BC40`, `0x0079BC50`, `0x0079BC60`), slot 3 the filter (`0x0079BC70`, §4.3) |

| Offset | Field |
|---|---|
| `+0x0E8` | an object released by `Shutdown` (not written by this owner) |
| `+0x110` | `E` (counted) |
| `+0x114`, `+0x118` | the sound IDs of `PlaySound(0)` and `PlaySound(1)` |
| `+0x11C` | the history index |

The pieces:
- **`QueryInterface`** (`0x0079BAB0`): IID `0xC6AE7085` gives `D+0x10C` with an `AddRef`; any other IID goes to
  `cGZWinGenDlg::QueryInterface`.
- **The constructor** (`0x0079BD00`):
  1. the base constructor (`0x009C2CEC`, argument 0) and the three vtables;
  2. `E` := null;
  3. `+0x11C` := the history count;
  4. both sound IDs := 0.
- **The destructor** (`0x0079BD80`): `Shutdown`, then `~cGZWinGenDlg`. It frees the object (global heap) when bit
  0 of its argument is set.

*[compat]* In 1.1.641 the application creator (`0x004E0BB0`) constructs the dialog and
immediately runs it modally; it supplies no sound IDs. The constructor explicitly zeroes both
IDs (`0x0079BD00`), and `Init` does not configure them (`0x0079BDC0`). Thus the normal
application path remains silent: `PlaySound` rejects a zero ID before obtaining a sound
(`0x0079BB70`). This conclusion concerns the analysed build (CCD3).

## 2. `Init` (`0x0079BDC0`) and `Shutdown` (`0x0079BAF0`) *[compat]*

`Init` returns true at once when the window is already initialised. Otherwise:
1. When the width (slot 41) or the height (slot 42) is ≤ 16: `SetArea(8, 8, 0xF8, 0x1C)` (slot 55).
2. `cGZWinGenDlg::Init`.
3. The window's private buffer:
   1. slot 100 `PrivateBuffer(1)`;
   2. fill it with the transparent colour (`0x0099BA3E`);
   3. its transparency on (`0x0099BA92`, argument 1).
4. Add a text edit: `AddTextEdit(rect, "")` (the `+0xD8` interface, slot 13), with the rectangle
   (left 4, top 6, right 308, bottom 26). Take `E` from the new window by `QueryInterface(0x231A1C57)`.
5. With `E`:
   1. `SetFlags(0x17C1)` (slot 77). This sets editor bit `0x400`, so Return sends `0x9B1552D5`, text-edit.md.
   2. The font `RZ::WCM` slot 5 `(0x68963C4C)`, when found (`SetFont`, slot 9).
   3. `SetFontColor(0, MakeFillColor(0, 0, 0))` (slot 12, window slot 108).
   4. `AddMessageFilter(D+0x10C)` (window slot 116) on `E`'s window.
6. `M` slot 37 `(the new window)`: the focus goes to the text edit.
7. `SetW(right − left + 8)` = 312 (slot 51); `SetH(bottom − top + 8)` = 28 (slot 52).
8. `SetFillColor(MakeFillColor(255, 0, 255))` (slots 106 and 108).
9. Return true.

`Shutdown` returns true without doing anything when the window is not initialised. Otherwise:
1. Release `+0xE8` and clear it.
2. With `E`: `RemoveMessageFilter(D+0x10C)` (window slot 117) on `E`'s window; release `E` and clear it.
3. Hide the dialog: `SetFlag(1, 0)` (slot 68).
4. Tail-call `cGZWinGenDlg::Shutdown`.

## 3. `DoMessage(msg)` (`0x0079BFC0`)

By message type (`msg+0`):
- **`0x11` (keyboard focus gained)**: `M` slot 37 `(E's window)`, then the base
  `cGZWin::DoMessage` *[compat]*. The base dispatcher's original jump table maps message 17
  to `GZOnSetFocus`, window slot 132 (`0x0099CCF0`, table `0x0099CEF9`, handler dispatch
  `0x0099CE9C`). The dialog redirects focus into its edit before base processing
  (`0x0079BFC0`; CCD1).
- **`0x9B1552D5`** (a key notification of `E`; Return): see below. The result is true, except false when there is
  no cheat code manager.
- **Anything else**: the base `cGZWin::DoMessage(msg)`.

For `0x9B1552D5`:
1. Without the cheat code manager: `EndDialog(0)`, result false.
2. Read the text into a string `s` (`E` slot 45). When `s` is empty: `EndDialog(0)`.
3. Otherwise:
   1. `id` := 0. Call `DoDefaultCheatCodeProcessing(s, &id)` (manager slot 22), which runs a known cheat and
      sets its ID.
   2. When `id` is still 0, give `s` to the command server (`0x00B43CB0`): `ExecuteCommand(text, length, null)`
      (slot 19, command-system.md).
      - A result of 2 (failure): `PlaySound(1)`, and the dialog stays open with the text.
      - Any other result: `id` := `0xFFFFFFFF`.
4. With a non-zero `id`, append `s` to `H` and set `+0x11C` := the new history count. Then:
   - When Ctrl or Shift is held (the window's keyboard at `D+8`, slot 3 with `0x11` then `0x10`): clear `E`
     (slot 49 `SetText(0, 0, 0)`) and `PlaySound(0)`. The dialog **stays open** for more cheats.
   - Otherwise: `PlaySound(0)` and `EndDialog(1)`.

`EndDialog(r)` (`0x0079BCD0`):
1. `M` `GZWinMsgPost(0xF, D, r)` (slot 9), which ends the modal loop with result `r`.
2. When the main window (global `0x00B43DC8`) exists, `M` slot 37 `(main window)` gives it the focus back.

## 4. Keys, the list and sounds

### 4.1 `GZOnKeyDown(key, flags)` (`0x0079C400`, window slot 130)

| Key | Action | Result |
|---|---|---|
| `0x70` (F1) | `DoPopupStringListMenu` (§4.2) | true |
| `0x26` (Up) | see below | true |
| `0x28` (Down) | see below | true |
| `0x1B` (Esc) | `EndDialog(0)` | true |
| `0x73` (F4) with `flags` bit `4` (Alt *[inferred]*) | `EndDialog(0)` | true |
| other | nothing | false |

Up and Down do nothing when `H` is empty. Let `i` = `+0x11C` and `n` = the history count.

**Up**:
- When `i` > 0: `i` := `i` − 1. Show `H[i]`: `SetText(H[i], 0)` (slot 48), then `SetInsertionIndex(its length)`
  (slot 66).
- Otherwise: `PlaySound(1)`.

**Down**:
- When `i` ≥ `n` (unsigned): `PlaySound(1)`.
- Otherwise: `i` := `i` + 1.
  - When the new `i` < `n`: show `H[i]` as for Up.
  - When the new `i` = `n`: clear `E` (slot 49 `(0, 0, 0)`).

So Up and Down walk the history, and one step past the newest entry gives an empty line.

`GZOnMouseDownR` (`0x0079C550`, window slot 135, three arguments): `DoPopupStringListMenu`, result true.

### 4.2 `DoPopupStringListMenu` (`0x0079C1D0`)

The result is false without the debug flag (`cISC4App` slot 21) or without the cheat code manager. Otherwise:
1. Get the names: `n` := `GetListOfAllCheatStrings(null, &count)` (manager slot 13).
2. Allocate `n` pointers from the global heap and call slot 13 again with that buffer and `&count`.
3. Copy each of the `count` strings into a `cRZString` list, through string slots 6 and 7. The strings themselves
   are not released.
4. Free the buffer.
5. Sort the list ascending (§5).
6. `P` := `D`'s parent window (`D+0x48`, widget-toolkit.md). Get the cursor position relative to it: `M` slot 34
   `(P, &x, &y)`.
7. `k` := `SC4DoPopupStringListMenu(P, x, y, &list, &choice, 0)` (`0x005F9F10`).
8. When `k` < the list size (unsigned): `SetText(choice, 0)` on `E`, then `SetInsertionIndex(its length)`.
9. Destroy the list and the choice. Return true.

So F1 or a right click in the text edit lists every registered cheat, sorted. Picking one puts it on the line
without running it.

### 4.3 The message filter (`0x0079BC70`, slot 3 of `D+0x10C`)

The filter is attached to `E`'s window (§2) and reads the message's kind at `+0`:
- **Kind 5**: `D` slot 130 `GZOnKeyDown(msg+4, msg+8)`; its result is returned.
- **Kind 8**: `D` slot 135 `GZOnMouseDownR` with three values:
  - the signed 16-bit value at `msg+4`;
  - the signed 16-bit value at `msg+8`;
  - the unsigned 16-bit value at `msg+0xE`.

  Its result is returned.
- **Other kinds**: false.

So the edit's key-downs reach the dialog first.

### 4.4 `PlaySound(i)` (`0x0079BB70`)

1. When sound ID `+0x114 + 4i` is 0 or there is no sound server (`RZ::SS`), return false.
2. Otherwise call the server's slot 13 `(1, 0x5B523BA4, &sound, id, 0, 0, 0)`. On success, play the sound (slot 8)
   and release it, and return true.
3. Otherwise return false.

With both IDs 0 (§1), the call always returns false and the dialog is silent.

## 5. The name sort

`DoPopupStringListMenu` sorts with the SGI `std::sort` over `cRZString` elements (`0x00663F80`):
- an introsort with depth limit 2 × ⌊log2 n⌋;
- median-of-three pivots;
- a heap sort fallback;
- a final insertion sort over runs of up to 16 elements.

The comparison (`0x004B5CA0`) is `std::string` `<`: the bytes are compared unsigned over the shorter length, and
on a tie the shorter string is less. So the order is case-sensitive byte order: `"Zap"` < `"apple"`, `"ab"` <
`"abc"`. The sort is not stable, but equal elements are equal byte strings, so the result is fully determined.

## 6. Quirks and cases

Quirks:
- The sound IDs are never set, so the accept, error and history-end sounds never play.
- Text that is not a known cheat still counts as success, enters the history and closes the dialog, when the
  command server returns anything but 2.
- The history is global: every later dialog starts past its last entry and can walk back through all earlier
  dialogs' lines.
- Esc, Alt+F4 and an empty Return all end the dialog with result 0.

| Case | Setup | Expected |
|---|---|---|
| CCD-C1 | history `[a, b]`, new dialog (`i` = 2): Up, Up, Up | lines `b`, `a`, then `PlaySound(1)` (silent) and the line stays `a` |
| CCD-C2 | history `[a, b]`, `i` = 1: Down, Down | first Down: `i` = 2, line cleared; second Down: `PlaySound(1)` |
| CCD-C3 | known cheat, Shift held, Return | runs, enters the history, line cleared, dialog stays open |
| CCD-C4 | cheat names `zoo`, `Abc`, `abc` | menu order `Abc`, `abc`, `zoo` |

## 7. Function inventory (owner `cSC4CheatCodeEntryDialog`)

All 33 functions (6,403 bytes):
- §1: `0x0079BAB0`, `0x0079BC30`, `0x0079BC40`, `0x0079BC50`, `0x0079BC60`, `0x0079BD00`, `0x0079BD80`;
- §2: `0x0079BDC0`, `0x0079BAF0`;
- §3: `0x0079BFC0`, `0x0079BCD0`;
- §4: `0x0079C400`, `0x0079C550`, `0x0079C1D0`, `0x0079BC70`, `0x0079BB70`;
- §5, the sort:
  - the driver `0x00663F80`;
  - the introsort loop `0x00663D80`;
  - the final insertion sort `0x00663D20`;
  - the other helpers `0x00662920`, `0x00662C60`, `0x00662D30`, `0x00663030`, `0x00663130`, `0x00663570`,
    `0x00663670`, `0x00663710`, `0x00663850`, `0x006639A0`, `0x00663A60`, `0x00663AC0`, `0x00663B70`;
  - the backward copy `0x005C7E50`.

Not covered here: the cheat code manager (`cGZCheatCodeManager`, now in cheat-codes.md), `SC4DoPopupStringListMenu`,
the base dialog `cGZWinGenDlg` (widget-dialogs.md). Keyboard slot 3 remains
*[inferred]* (CCD2); the focus message is established in §3 (CCD1).
