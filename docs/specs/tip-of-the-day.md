# `cSC4TipOfTheDayWinProc`: the tip of the day dialog (behaviour spec)

Scope: the `cSC4TipOfTheDayWinProc` owner as tools/coverage.py assigns it (12 functions, 1,982 bytes). The dialog shows
one tip at a time in an HTML control; the tips are the exemplars of exemplar type `0x29`. The 3D view opens it once
per run, after a city loads (view3d-window.md, `DoPostCityInit`).

Related: ui-windows.md (scripted windows, `SC4CreateWindowFromScript`), advice-list.md and tutorial-window.md (the same
HTML control), modal-dialogs.md (the end message `0xF`), audio-preferences.md (the preferences object `P`,
`0x00B43CD8`), exemplar/property access (`cSCPropertyHelp::GetPropertyValue`).

Evidence: static RE only, on the instruction and data bytes of the Steam 1.1.641 exe (the same code as GOG,
`docs/lanes/re-dates.md`). Every function of the owner was read in the decompile; `DoModelessWin`, `EndDialog`,
`GoToNextTip`, `InitDialog` and the exemplar enumeration were checked on the instructions. Written 2026-10-24
(re-gztips7).

## 1. State *[compat]*

**The proc** (0x24 bytes, vtable `0x00AB5500`: 0 `QueryInterface`, 1 `AddRef`, 2 `Release` (shared), 3
`DoWinProcMessage`, 4 the base `DoWinMsg`):

| Offset | Field |
|---|---|
| `+0x04` | reference count |
| `+0x08`…`+0x10` | the tips: a vector of resource keys (type, group, instance; 12 bytes each) |
| `+0x14` | the current tip's index |
| `+0x18` | the dialog window (generic window, counted) |
| `+0x1C` | the HTML control (class `0xAA12E5F5`, interface `0x4A11FD4A`, counted) |
| `+0x20` | a counted pointer that no function of the owner sets (released by `EndDialog` and the destructor) |

**Statics:** `0x00B4DFD0` shown (set by `InitDialog`); `0x00B4DFD1` tips found; `0x00B4DFD2` presence-scan mode;
`0x00B4DFD3` presence scanned.

**Preferences** (`P` = `0x00B43CD8`): byte `+0x49C` the dialog's check box; dword `+0x4A0` the instance of the last
tip shown.

*[compat]* The checkbox byte defaults to 1 (`0x00449070`). It is persisted with the complete
`0x1734`-byte preferences payload, record type `0xA9DD6E06`, group
`0x49DD6E08`, instance 1, version `0x15` (`0x0044BDB0`); reading that version restores the payload
including this byte (`0x0044BC50`). Persistence does not make the checkbox an opening gate:
`DoPostCityInit` tests only the session's shown flag and the presence of tips before invoking
the opener (`0x007ED0F0`), and the opener checks the parent, existing dialog and script
creation (`0x0078C8C0`). Clearing the checkbox therefore does not suppress this automatic
opening on a later run. The shown flag already prevents another automatic opening during
the same run (`0x0078C760`, `0x0078CDB0`; TOD1).

## 2. Entry points *[compat]*

- **`HasBeenShownAlready`** (`0x0078C760`): the shown byte.
- **`EntriesPresent`** (`0x0078CCE0`): when tips were found, or the scan was already done, return the found byte.
  Otherwise mark it done, enter presence mode, enumerate (§3) with a null context, leave presence mode and return the
  found byte. So the scan runs at most once per run.
- **`DoModelessWin(out)`** (`0x0078C8C0`): with the window held in `0x00B43CC0` (set together with `0x00B43CBC` by `0x00602540`) and no descendant `0x4A3BE152`
  in it (slot 35): `SC4CreateWindowFromScript(key (0, 0x96A006B0, 0xCA3BDD94), parent, ID 0x4A3BE152, interface
  0x5386D516, &win)`. On success a new proc (all zero) is made, holds `win` at `+0x18`, the window is pulled to the
  front (slot 23) and loses flag `0x800` (z-order movable; slot 68), and the proc becomes the window's proc
  (generic-window slot 7). The proc is stored in `*out` when `out` is not null (the caller's reference), else
  released. Returns true; false when that window is null, the dialog already exists or the script fails.

The 3D view calls it with `out` = null when `HasBeenShownAlready` is false and `EntriesPresent` is true.

## 3. Finding the tips (`0x0078CC70`, callback `0x0078CB40`)

The collector clears the vector of the proc it is given, gets the key list of every resource of type `0x6534284A`
(exemplars) from the resource manager (`0x00B43CA8`, slot 18) and calls the list's slot 7 with the callback and the
proc; the list is then released. Without the resource manager or a list it does nothing.

The callback, per key: in presence mode with tips already found, nothing. Otherwise the exemplar is loaded (resource
manager slot 3, interface `0xA52160F5`); when its property `0x10` (exemplar type) is `0x29`: in presence mode the found
byte is set, else the key is appended to the context's vector. The exemplar is released.

The tips keep the resource manager's enumeration order.

## 4. Messages and commands *[compat]*

**`DoWinProcMessage(win, msg)`** (`0x0078CF30`): only for the dialog's own window; else false.

| Message | Action | Result |
|---|---|---|
| 3 (command) | `DoCommand(msg)` | its result |
| `0xA2BF8AD5` | `InitDialog` | true |
| `0xA2BF8AD6` | — | true |

**`DoCommand`** (`0x0078CD50`), for type `0x287259F6` only (else false):

| Button | Action |
|---|---|
| `0x42B7C353`, `0x42B7C356`, `0xAA3BE170` | `EndDialog` |
| `0x2A3EA604` | next tip: index + 1, wrapping to 0 |
| `0x4A3EA5FD` | previous tip: index − 1, wrapping to the last |

The step (`0x0078CBF0`) does nothing with no tips; it then shows the tip at the new index (§5). Handled commands
return true.

## 5. Showing a tip

**`InitDialog`** (`0x0078CDB0`), with a window: shown := 1. When the window has a child `0xAA3BE17E` (slot 34, the
placeholder): the old control is released and a new one created (GZCOM `CreateInstance(0xAA12E5F5, 0x4A11FD4A)`); its
window gets the placeholder's area (placeholder slot 48, control window slot 54); the control gets slot 28 with
`0x40`, `4` and `8` (8 = no vertical scroll bar); its window is added to the dialog (slot 14). Then the tips are
collected (§3); the index becomes the position of the tip whose instance equals `P+0x4A0` (left unchanged, so 0,
when none matches); then index + 1 (wrapping to 0) and that tip is shown.

**`GoToNextTip(key)`** (`0x0078CA40`), with the resource manager and a control: the exemplar is loaded (interface
`0xA52160F5`); its property `0x6A3E9D02` (a string reference) becomes a string (`cSLString`), and the control gets
slot 22 `(text, "sc4://0xTTTTTTTT_0xGGGGGGGG_0xIIIIIIII.html")` with the key's type, group and instance in lower-case
8-digit hex.

## 6. Closing (`EndDialog`, `0x0078C770`)

With a window (else nothing): a self reference is taken. The check box (child `0xAA3BE181` as a button, IID `0x8810`,
slot 36), when found, gives `P+0x49C` := its state (slot 10) ≠ 0. With tips, `P+0x4A0` := the current tip's instance.
The window's proc is cleared (generic-window slot 7, null). A modal window (flag `0x1000`) is ended through the window
manager (`GZWinMsgPost(0xF, window, 1)`, slot 9); otherwise its parent deletes it (slot 16). `+0x18`, `+0x1C` and
`+0x20` are released and the self reference dropped.

The destructor (`0x0078C9E0`) releases `+0x20`, `+0x1C`, `+0x18` and frees the vector.

## 7. Quirks

- When the last tip shown is no longer among the tips, the dialog starts at the second tip (index 0 + 1), not the
  first *[quirk]*.
- `EntriesPresent` caches its answer for the run: tips added later are not seen by it (the dialog itself rescans).
- The checkbox is saved and restored, but the automatic opener ignores it (§1, TOD1).
- The callback returns nothing and the scan cannot stop early: presence mode still loads no further exemplars once one
  is found, but every key is still visited.

## 8. Function inventory

`0x0078C760` `HasBeenShownAlready`, `0x0078C770` `EndDialog`, `0x0078C8C0` `DoModelessWin`, `0x0078C9E0` destructor,
`0x0078CA40` `GoToNextTip`, `0x0078CB40` the scan callback, `0x0078CBF0` the step, `0x0078CC70` the collector,
`0x0078CCE0` `EntriesPresent`, `0x0078CD50` `DoCommand`, `0x0078CDB0` `InitDialog`, `0x0078CF30` `DoWinProcMessage`.
