# `cSC4TutorialUIWinProc`: the tutorial window (behaviour spec)

Scope: the `cSC4TutorialUIWinProc` owner as tools/coverage.py assigns it (19 functions, 7,407 bytes on `5c0ff782`;
inventory in §9). The tutorial system makes one window per running tutorial (tutorial.md §4.2). The window:
- shows the current task's text in an HTML control;
- builds the tasks from a Lua table;
- steps through them;
- restricts and highlights the city UI for each task.

[tutorial.md](tutorial.md) §4.2.2 and §4.4.1 already give the owner-facing boundary: allocation, the constructor,
`Init`, the close call, slots 6–11 and `OnTutorialEvent`. This spec gives the inside of the window and does not
repeat that boundary. The constructor (`0x00442920`), slots 6, 9 and 10 (`0x00443E10`, `0x00442320`,
`0x00441C50`) and the message target's `DoMessage` (`0x00444480`) belong to the `cSC4TutorialSystem` owner. They
are cited here, not specified.

Evidence: static RE only, on the instruction and data bytes of the Steam 1.1.641 exe (the same code as GOG,
`docs/lanes/re-dates.md`). Every function was disassembled. The two static strings were read from their start-up
initializers (`0x00A71530`, `0x00A71B30`). Written 2026-10-23 (re-tutwin7).

Conventions:
- `W` is the window (its base).
- `S` is the script window's generator (`W+0x10`, interface `cIGZWinGen` `0x5386D516`); "`S`'s window" is its
  slot 3 (`AsIGZWin`).
- `V` is the 3D view (global `0x00B43DC4`); "`V`'s window" is its slot 3.
- `O` is the tutorial system, read from the global `0x00B20E88` (tutorial.md §4.2.2).
- The current task is `W` slot 9 (`0x00442320`): the task at index `+0x30` when that index is below the task
  count, else null.
- Window slots are `cIGZWin` slots (ui-windows.md §2.2). `SetFlag(1, on)` shows or hides a window, `SetFlag(2, on)`
  enables or disables it.
- Task slots are `cSC4TutorialTask` slots (no spec yet). The names in §8 come only from how this window uses them.

## 1. Identity and fields *[compat]*

| Offset | vtable | Slots |
|---|---|---|
| `+0x00` | `0x00A8638C` (base) | 0–5 shared and destructor; 6 `0x00443E10`; 7 `ShowUIHintsForUsers` (`0x00443180`); 8 `HandleCatalogView` (`0x00442340`); 9 current task (`0x00442320`); 10 `0x00441C50`; 11 `ResetTutorialNumAndIndex` (`0x00443B40`) |
| `+0x08` | `0x00A86378` (window proc) | 3 `DoWinProcMessage` (`0x00444160`); 4 `DoWinMsg` (shared) |
| `+0x0C` | `0x00A86368` (message target) | 3 `DoMessage` (`0x00444480`) |

| Offset | Field |
|---|---|
| `+0x10` | `S`, the script window's generator (counted) |
| `+0x14` | the HTML text control (counted): class `0xAA12E5F5`, interface `0x4A11FD4A` |
| `+0x18` | the shown arrow (counted); selected by the arrow-placement helper described below (TUW1), hidden at task changes |
| `+0x1C`, `+0x20` | the two arrow windows (counted) |
| `+0x24` | the exit dialog (counted), made by the first exit request (§6.2) |
| `+0x2C` | the catalog view currently flashing an item (counted); slot 21 `(-1, 0, 0)` cancels its flashing (TUW1) |
| `+0x30` | the absolute index of the current task |
| `+0x34` | the highlighted window (counted) |
| `+0x38` | byte: skip the next highlight (set by `HandleCatalogView`) |
| `+0x39` | byte: cleared by `HandleCatalogView`; read by the click filter (slot 10) |
| `+0x3A` | byte: cleared by `Shutdown` |
| `+0x3B` | byte: the current task is the last one (written only by `moveToNext`) |
| `+0x3C`, `+0x40` | the rest position `(x, y)` of `S`'s window |
| `+0x44` | the Lua context (counted) |
| `+0x48`…`+0x50` | the task vector (counted tasks; begin, end, capacity) |

*[compat]* These references are written by tutorial-window helpers, rather than the tasks.
The message target (`0x00444480`) handles the queued self-message `0x4C05A5E6`:
kind 3, once its delay count reaches zero, flashes the requested catalog item through catalog
slot 21, retains that catalog interface at `+0x2C`, and places an arrow at the item's rectangle.
Kind 4 derives a hint window's screen rectangle (`0x00442760`) and places an arrow there.
Both reach the arrow-placement helper (`0x00442430`), which hides/releases the previous
`+0x18`, selects and retains one of `+0x1C`/`+0x20` for applicable task types 4 and `0x10`,
then positions, raises and shows it. Task type 4 also requires a nonzero task parameter
(slot 18). Later task transitions cancel the retained catalog flashing and hide the arrow
(§3.4; TUW1).

Shared state:
- The global `0x00B2140C` is a `std::map<uint32, uint32>` from window ID to a visit count; its size is at
  `0x00B21410`. Both window walkers (§5.1) use it.
- The static `cRZString` `0x00B21204` holds `"tutorialtasks"`.
- The static `cRZString` `0x00B213E4` holds `"sc4://tutorial"`.

## 2. `Init` (`0x00443E60`) *[compat]*

1. Release `S` and clear it. Create the script window: `SC4CreateWindowFromScript` (`0x005F9480`) with
   - the key `{0, 0x96A006B0, 0x0A2DD355}`;
   - the parent `RZ::WM()` slot 3;
   - the ID `0x4A35B0F2`;
   - the IID `0x5386D516`;
   - the result going into `&S`.
2. When that fails, go to step 4.
3. Otherwise:
   1. `S` slot 7 `(W+8)`: the window procedure.
   2. `+0x44` := `O` slot 7 (`GetScriptingContext`), counted.
   3. `+0x3C` := `S`'s window `GetL` (slot 43); `+0x40` := its `GetT` (slot 44).
   4. `+0x30` := the registry item's start index − 1 + `O` slot 11. The item is `GetItem(O slot 12)`
      (tutorial.md §4.2.2).
   5. `load_task_script` (§3.1). When it fails, go to step 4.
   6. Find the child `0x0A35F767` of `S`'s window (slot 35). When there is none, go to step 4.
   7. Release `+0x14`, then create it: GZCOM `CreateInstance(0xAA12E5F5, 0x4A11FD4A, &+0x14)`. When that fails,
      go to step 4.
   8. Its window (slot 3) gets `SetArea(0, 0, W, H)` of the child (slot 55, child slots 41 and 42).
   9. The control gets slot 28 `(0x40)`.
   10. The child gets the control's window as a child (`ChildAdd`).
   11. The control's window gets `S`'s window as its notification target (slot 86).
4. Load the two arrows (§7.1):
   - `+0x1C` := `LoadArrow(0x0A41BE3E, 0x0A41C7B2)`;
   - `+0x20` := `LoadArrow(0x0A41BE3F, 0x0A41C7B3)`.

   Each field is released and cleared first.
5. When the message server (`0x00B43CCC`) exists, subscribe `W+0xC` (server slot 5) to `0x07699455`,
   `0x99EF1142`, `0xE94671EB`, `0x894671F6`, `0x66956816`, `0x99EF1143` and `0xE8BDA2E7`, in that order.
6. `ReStartCurrentTask` (§3.3).

## 3. Tasks

### 3.1 `load_task_script` (`0x00443090`)

Without a Lua context (`+0x44`) the result is false. Otherwise:
1. Context slot 6 (`GetTop`); its result is not used.
2. `GetGlobal("tutorialtasks")` (slot 37).
3. When `IsTable(-1)` (slot 72) is false: `Pop(1)` (slot 11) and return false.
4. `n` := `GetN(-1)` (slot 64), then `Pop(1)`.
5. Reserve `n` places in the vector. When `n` ≤ 0, return false.
6. For `i` = 1 … `n`:
   1. Allocate 0xFC bytes from the global heap (`0x005E55E0`). When that fails, skip this `i`.
   2. Construct a `cSC4TutorialTask` (`0x00440F30`).
   3. Call its slot 3 `(context, i)`. Its result is not checked.
   4. Append it to the vector.
7. Return true.

The vector is not cleared first. Only `Init` calls this.

### 3.2 Text: `setInstructionText(text)` (`0x00443930`)

`text` is a `cRZString*`. Without the text control (`+0x14`) nothing happens. Otherwise:
1. The control gets slot 22 `(the text's characters, the base "sc4://tutorial")`.
2. `DoSpecialDocumentProcessing(text)` (§4).

### 3.3 `ReStartCurrentTask` (`0x004439F0`)

Without a current task the result is false. Otherwise:
1. Copy task slot 8 (a `cIGZString*`, read through its slots 6 and 7) into a string.
2. When `+0x14` is set, do step 3.2 on that string.
3. Task slot 14 (start).
4. `W` slot 7: `ShowUIHintsForUsers` (§5.2).
5. When the simulator (`0x00B43D5C`) `IsPaused` (slot 15), call its `Resume` (slot 12).
6. With `V`:
   1. `V` `SetZoom` (slot 8) with task slot 22.
   2. `+0x3C` := width of `V`'s window − width of `S`'s window; `+0x40` := 0.
   3. `S`'s window moves to `(+0x3C, +0x40)`.
7. The 24-hour clock (`0x00B43DBC`) `SetVisualTimeMode(1)` (slot 5).
8. Return true.

### 3.4 `moveToNext` (`0x00443BD0`)

Nothing happens without a current task `T`. Otherwise `T` is held for the call, and:
1. Hide `+0x18` (slot 70) when it is set.
2. Without `O`, stop.
3. Hold `O`. Read the registry item `GetItem(O slot 12)` (no null check):
   - `base` := start index − 1;
   - `end` := `base` + task count.
4. `next` := `O` slot 11 + `base` + 1.
5. When `next` is not below the vector length (unsigned compare), is ≤ 0, or is ≥ `end`: `finish_tutorial(true)`
   (§3.6) and stop.
6. Otherwise:
   1. Unless `T` slot 20 is 8: `V` `RemoveCurrentViewInputControl(0)` (slot 26). There is no null check on `V`.
   2. When `+0x2C` is set: its slot 21 `(-1, 0, 0)`, then release and clear it.
   3. `T` slot 16 (end).
   4. When `T` slot 20 is 4, hide `+0x18` again.
   5. `O` slot 20 (increment); then `+0x30` := `O` slot 11 + `base`.
   6. `N` := the new current task. Copy `N` slot 8 into a string and `setInstructionText` it (§3.2).
   7. `V` `SetZoom(N slot 22)` (no null check). `N` slot 14 (start).
   8. `x` := `N` slot 23 + `+0x3C`; `y` := `N` slot 24 + `+0x40`. Each is raised to 0 when negative. `S`'s
      window moves to `(x, y)`.
   9. `tick_self(2, 0, 0, 0, 0)` (§5.4).
   10. `+0x3B` := 1 when `next` + 1 ≥ `end` or `next` + 1 ≥ the vector length, else 0.

`moveToNext` does not call `ShowUIHintsForUsers`, resume the simulator or set the clock (compare §3.3).

### 3.5 `ResetTutorialNumAndIndex(guid, n)` (`0x00443B40`, slot 11)

tutorial.md §4.4.1 gives the checks and the result. In the success branch, the order is:
1. hide `+0x18`;
2. the current task's slot 16 (end);
3. `+0x30` := the candidate;
4. `ReStartCurrentTask`.

`+0x3B` is not updated.

### 3.6 Events and the end

- **`OnTutorialEvent(e)`** (`0x00443970`). The current task is held for the call. When `e` = 1 and task slot 20
  is 1:
  1. task slot 15;
  2. `setInstructionText` with a copy of task slot 9.

  AL is always true.
- **`finish_tutorial(flag)`** (`0x00441E50`). Without a current task nothing happens, not even the message.
  Otherwise:
  1. task slot 16, called **twice**;
  2. post `0xAA3840E6` (message server slot 4 with second argument 0), data 1 (slot 11) = 1 when `flag` is
     false, else 0.

  The tutorial system then calls `endTutorial(data == 0)` (tutorial.md §4.3).

## 4. `DoSpecialDocumentProcessing(text)` (`0x00443410`)

Nothing happens without `S`. The text's HTML comments carry window commands, in this form:
`<!-- legacy id=<hex> winflag_visible=yes winflag_enabled=no href="url" -->`.

The scan:
1. `p` := the index of the first `<!--` (none when the text is shorter than 4 characters).
2. While `p` exists:
   1. `e` := the index of the first `-->` **in the whole text**, searched from the start every time. Without
      one, stop.
   2. The comment string is the substring at `p` of length min(length − `p`, `e` − 1) *[quirk]*.
   3. Trim it, then take tokens separated by spaces (`cRZString::SplitTokenSeparated(' ')`), as in step 3.
   4. `p` := the first `<!--` at or after index `e` (none when `e` + 4 > length).
3. For each token in order:
   - `<!--` and `-->` (exact) are skipped.
   - The first other token is lowercased and must be `legacy`; otherwise the rest of the comment is dropped.
   - A later token without `=` is ignored. Otherwise it is split at `=`. Key and value are both trimmed and
     lowercased.
   - `id`: the value is read as hexadecimal (`RZAtoUint32HexA`). The **target** becomes that child of `S`'s
     window (slot 35, recursive), which may be null. The target persists for the rest of the call.
   - `winflag_visible`: with a target, `SetFlag(1, value == "yes")`.
   - `winflag_enabled`: with a target, `SetFlag(2, value == "yes")`.
   - `href`: with a target, drop one leading and one trailing `"`. When the rest is not empty, the target gets
     `SetParam(0x0A8FE6CB, string variant)` (slot 114). §6.1 reads it back.
   - Other keys are ignored.

## 5. UI hints

### 5.1 The two window walkers

Both are `EnumChildren` callbacks (window slot 32, interface `0x22BA0121`). The child is the third argument,
the context the fourth. Both return true. Each visits a child `c` with ID `k` (slot 63) as follows:
1. Add `k` to the map with count 0 when it is missing.
2. When `k`'s count is below 2, raise it by 1 and act on `c` (below).
3. Then walk `c`'s children with the same walker and context, whatever the count.

The walkers' actions:
- **Reset walker** (`0x00442B30`):
  - when `c` has the button interface `0x8810`, `c` gets `SetFlag(2, 1)`;
  - when `c` is a catalog view (interface `0x49D98264`, catalog-view.md), its slot 18 gets `(-1, 1)`. Slot 18
    sets every item except the given ID, so every item is enabled (TUW6, answered).
- **Hint walker** (`0x00442EF0`), context = the task `T`:
  1. `allowed` := `T` slot 7 `(k)`.
  2. When `c` has the button interface, `c` gets `SetFlag(2, allowed)`.
  3. When `allowed`, walk `c`'s children with the reset walker.
  4. When `c` is a catalog view, its slot 18 gets `(T slot 19, 0)`: every item except the task's item ID is
     disabled.

  A null child or task skips steps 1–4, not the walk.

### 5.2 `ShowUIHintsForUsers` (`0x00443180`, slot 7)

Nothing happens without a current task `T` and `V`. Otherwise:
1. Clear the map when it is not empty. Walk `V`'s window with the hint walker (context `T`).
2. When `+0x38` is set: clear it and stop.
3. `t` := `T` slot 20.
4. `t` = 0xB, 0xC or 0xD: unhighlight (below) and stop.
5. `t` = 0x10:
   1. Look up `T` slot 11 as an ID in `V`'s window (`GetChildAsRecursive`, slot 37, interface `0x22BA0121`).
   2. When found: `T` slot 5, `tick_self(4, 0, 0, w, w)`, then `w` gets `SetFlag(2, 1)`.

   The highlight is not touched.
6. Any other `t`:
   1. `w` := `T` slot 12. When it is null, stop.
   2. `tick_self(4, 0, 0, w, w)`; `w` gets `SetFlag(2, 1)`.
   3. When `w` is not `+0x34`: unhighlight, then highlight `w`, and `+0x34` := `w`.

Highlight and unhighlight:
- To highlight `w`: its button interface `0x8810` gets slot 44 `(0x7FFFFFFF, 200)`.
  *[compat]* Slot 44 starts flashing with a countdown and a timer interval in milliseconds
  (`0x009B1058`). Each timer message updates the layout and decrements a positive countdown;
  alternating parity supplies the flashing phases (`0x009B0311`, `0x009B1A3B`,
  `0x009B0C08`). Thus 200 ms is the interval between phase updates, and the count measures
  updates, not pairs of phases. The tutorial requests an effectively continuous sequence (TUW2).
- To unhighlight: when `+0x34` is set, its button interface gets slot 44 `(0, 200)`; then release and clear
  `+0x34`. A zero count immediately reapplies the resting layout; the next timer message stops
  the timer without another countdown change (`0x009B1058`, `0x009B1A3B`).

### 5.3 `HandleCatalogView(menu)` (`0x00442340`, slot 8)

1. `+0x39` := 0.
2. When the menu's window (`menu` slot 3) is already `+0x34`, stop.
3. Unhighlight (§5.2). `+0x34` := the menu's window. It is **not** highlighted.
4. `v` := slot 19 of the task at index `+0x30`. There is no bound check.
5. When `+0x2C` is set: its slot 21 `(-1, 0, 0)`.
6. `+0x38` := 1.
7. `menu` slot 20 `(v)`: the catalog view scrolls item `v` into view (catalog-view.md §2).
8. `tick_self(3, v, 1, menu, menu)`.

### 5.4 `tick_self(kind, a, b, c, d)` (`0x00442200`)

1. Make a `cRZMessage2Standard` of type `0x4C05A5E6`. Its five data fields (slots 11, 16, 21, 27 and 33) get
   `kind`, `a`, `b`, `c` and `d`.
2. Hand it with `W+0xC` to message server slot 7, which queues it for the window's own
   `DoMessage` (`0x00444480`, kinds 1–4) *[compat]*. The server retains the message and target
   in its directed queue (`0x0092EA00`); delivery occurs when that queue is pumped
   (`0x0092F90C`), not inside slot 7.
3. Release it.

The kinds used here:
- 2 = a task step;
- 3 = a catalog view;
- 4 = a hint window.

## 6. The window procedure

### 6.1 `DoWinProcMessage(win, msg)` (`0x00444160`)

The result is false unless `win` is `S`'s window. Otherwise, by message type:
- **`0xE4A85D91`** (a link, URL `cIGZString*` at `msg+8`):
  - When the URL equals `sc4://action/next_tutorial` (string slot 10 with `false`), `moveToNext` and return
    false.
  - Otherwise `cSC4HTMLLinkHandler::DoURL(url, 0)` on an empty local handler, and return the **negation** of
    its result.
- **`0xA2BF8AD5`** (attach). With `S`, the child `0x4A383DA6` of `S`'s window gets `S`'s window as its
  notification target. Return false.
- **3** (a command): false without a current task, or when the command ID (`msg+4`) is not `0x287259F6`.
  Otherwise, by button ID (`msg+8`), and true in every case:
  - **`0x4A383DA6`**: `exit_to_region` (§6.2).
  - **`0x6AAEEC41`–`0x6AAEEC43`** (the exit dialog):
    1. The dialog's window manager gets `GZWinMsgPost(0xF, dialog, 1)` (slot 9), which ends its modal loop.
    2. The dialog is hidden.
    3. Then, by button:
       - **`0x6AAEEC41`**: the current task's slot 16; send `0xAA3840E6` at once (message server slot 3) with
         data 1 = 1 when `+0x3B` is clear, else 0; the simulator's `Pause` (slot 9); the application's
         `RequestGoToRegionView(false)` (slot 12).
       - **`0x6AAEEC42`**: `finish_tutorial(+0x3B)`.
       - **`0x6AAEEC43`**: nothing more.
  - **Any other ID**: the button is looked up in `S`'s window (slot 35, recursive). When it exists and the text
    control is set:
    1. `GetParam(0x0A8FE6CB)` (slot 113) on the button.
    2. When that succeeds, the variant's slot 55 converts it to a string.
    3. When that succeeds, the text control gets slot 25 with that string. So a button tagged with `href`
       (§4) opens its URL in the tutorial text *[inferred]*.
- Any other type: false.

### 6.2 `exit_to_region` (`0x00441EE0`)

The current task is held for the call but not used. Nothing happens without `S`. The first time (no `+0x24`):
1. `+0x24` := `SC4CreateWindowFromScript` (`0x005F9390`, ui-windows.md §5.1) with:
   - the key `{0, 0x96A006B0, 0xEAAEEC1B}`;
   - the parent `S`'s window;
   - the ID `0x6AAEEC4A`.
2. The dialog gets `PullToFront` (slot 23) and `GZWinMoveTo(50, 120)` (slot 56).
3. `S`'s window becomes the notification target (slot 86) of:
   - the dialog;
   - its children `0x6AAEEC41`, `0x6AAEEC43` and `0x6AAEEC42`, each when found.
4. The text edits `0xCA62395F` and `0xCA623979` (`GetChildAsRecursive`, interface `0x231A1C57`) get slot 78
   `ClearFlag(1)` (text-edit.md §10).

Every time: show the dialog (slot 69), then run it modally (its window manager's `DoModalWin`, slot 41).

## 7. Arrows and shutdown

### 7.1 `LoadArrow(instance, id, &out)` (`0x00441B50`)

1. `out` := null.
2. `SC4CreateWindowFromScript` (`0x005F9480`) with:
   - the key `{0, 0x96A006B0, instance}`;
   - the parent 0;
   - `id`;
   - the IID `0x22BA0121`;
   - `&out`.
3. On success with a window: `HideWindow` (slot 70) and `SetFlag(0x200000, 1)`. Return true.
   *[compat]* This flag excludes the arrow from cursor targeting: the parent's cursor-point
   search skips flagged children (`0x0099DFA9`), and cursor-message routing also skips them
   and refuses delivery to a flagged window (`0x0099D40B`). Visible arrows therefore let
   cursor targeting continue to windows underneath them (TUW3).
4. Otherwise return false.

`Init` ignores the results.

**`KillArrow`** (`0x00441BD0`):
1. Release `+0x18` and clear it.
2. For `+0x1C`, then `+0x20`: when its window manager (slot 6) exists, `DestroyWindow` (manager slot 23) it;
   then release it and clear it.

### 7.2 `Shutdown` (`0x00442C80`, the close call of tutorial.md §4.2.2)

1. With `V` (held for the call):
   1. Clear the map when it is not empty.
   2. Walk `V`'s window with the reset walker (context 0).
2. Hold the current task and call its slot 16.
3. When the message server exists, unsubscribe `W+0xC` (server slot 6) from the seven messages of §2 step 5,
   in the same order.
4. With `S`: destroy `S`'s window through its window manager (slot 23), when there is one; then release `S` and
   clear it.
5. `KillArrow`.
6. Unhighlight (§5.2).
7. `+0x3A` := 0.

The text control, the dialog, the Lua context, `+0x2C` and the task vector stay; the destructor (`0x00442C60`,
outside this owner) frees them.

## 8. Quirks and cases

Task slots as this window uses them (confirmed by tutorial-task.md §1; TUW5, answered):

| Slot | Use |
|---|---|
| 3 | build from `tutorialtasks[i]` |
| 5 | the task's area (the window ignores the result) |
| 7 | is window ID `k` allowed |
| 8 | instruction text |
| 9 | the try-again message, shown after event 1 |
| 11 | hint window ID |
| 12 | hint window |
| 14 | start |
| 15 | event-1 action |
| 16 | end |
| 19 | the catalog item ID to keep enabled and scroll to |
| 20 | type |
| 22 | zoom |
| 23, 24 | window offset `x`, `y` |

Quirks:
- **`DoSpecialDocumentProcessing` never ends** when a `<!--` appears after the text's first `-->`. `e` is always
  the first `-->`, and the next search starts at `e` again, so it finds the same `<!--` every time. A text with
  two comments hangs (TUW4).
- The comment length is `e` − 1, not `e` − `p`:
  - a comment at index 0 loses the character before `-->`;
  - a comment at index `p` ≥ 2 runs `p` − 1 characters past it, so a later `key=value` token can apply.
- `href` values are lowercased before they are stored.
- The 2-visit cap per window ID shapes the hint walk:
  - a child of an allowed button is first enabled by the reset walker, then judged by the hint walker;
  - deeper descendants of allowed ancestors reach the cap and stay enabled;
  - after the second visit, windows sharing an ID are left as they are by both walks, including the one in
    `Shutdown`.
- `finish_tutorial` ends the current task twice, and posts nothing without a current task.
- `+0x3B` changes only in `moveToNext`. After `ResetTutorialNumAndIndex` it keeps its old value, which then picks
  the end data of the exit dialog's first two buttons.
- `moveToNext` reads `V` without a null check, and `HandleCatalogView` reads the task vector without a bound check.

| Case | Setup | Expected |
|---|---|---|
| TUW-C1 | text `ab<!-- legacy id=1A winflag_visible=no -->` | `p` = 2, `e` = 39, length 42; the comment string is the 38 characters `<!-- legacy id=1A winflag_visible=no -`; the last token `-` has no `=` and is ignored; child `0x1A` of `S`'s window is hidden |
| TUW-C1b | text `<!-- legacy id=1A -->` | `p` = 0, `e` = 18; the comment string is `<!-- legacy id=1A` (17 characters): the space before `-->` is the character lost |
| TUW-C2 | registry start 3, count 4, `O` slot 11 = 2, 10 tasks | `+0x30` = 4; `moveToNext`: `next` = 5 < 6, so step to 5, `+0x3B` = 1 (6 ≥ `end`) |
| TUW-C3 | same, `O` slot 11 = 3 | `next` = 6 = `end`: `finish_tutorial(true)`, message data 0 |
| TUW-C4 | link `sc4://action/next_tutorial` | `moveToNext`, result false |

## 9. Function inventory (owner `cSC4TutorialUIWinProc`)

All 19 functions (7,407 bytes):
- §2: `0x00443E60`;
- §3: `0x00443090`, `0x00443930`, `0x004439F0`, `0x00443BD0`, `0x00443B40`, `0x00443970`, `0x00441E50`;
- §4: `0x00443410`;
- §5: `0x00442B30`, `0x00442EF0`, `0x00443180`, `0x00442340`, `0x00442200`;
- §6: `0x00444160`, `0x00441EE0`;
- §7: `0x00441B50`, `0x00441BD0`, `0x00442C80`.

Not covered: the HTML control (out of scope, ui-windows.md), `cSC4TutorialTask` (tutorial-task.md), the message target's
`DoMessage` and the click filter (owner `cSC4TutorialSystem`).
