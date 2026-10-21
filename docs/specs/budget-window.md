# Budget window family: `cSC4BudgetUIWinProc` and the sub-budget dialog `cSC4SubBudgetWindow` (behaviour spec)

Status: first pass (re-panels4, 2026-10-21). Covers the **lifecycle, command routing and data contracts** of the two
largest SC4-specific panel owners (`cSC4SubBudgetWindow` 71 235 B, `cSC4BudgetUIWinProc` 17 952 B; ui-windows.md
§14.3). It does not re-specify the money model (budget.md) or generic widgets (the widget lane). The per-row layout
code (`initNeighborBudget` 13 933 B, `updateNeighborBudget` 14 988 B, `initOrdinanceBudget` 4 468 B, the `add_*`
helpers) is summarised by contract only; pixel layout is §8 open work.

Evidence: Ghidra export via `tools/re_query.py` (asm, callers, callees, vtable) and decomp/. REA was **not available**
in this round (no binary open in the shared session), so every claim below is from the export; the decompiler output
of `on_init` and `DoSubBudgetWindow` is stack-mangled and was checked against the instructions at the cited addresses.

## 1. Identities *[compat]*

| Item | Value |
|---|---|
| `cSC4SubBudgetWindow` primary vtable | `0x00AB5494`, 6 slots: 0 QueryInterface, 1 AddRef, 2 Release, 3 `cIGZWinProc::DoWinProcMessage` thunk, 4 `DoWinMsg` `0x0078BCA0`, 5 deleting dtor `0x0077E420` |
| `cSC4SubBudgetWindow` size | `0x140` bytes (operator new at `0x00768478` via `DoSubBudgetWindow`) |
| Dialog window object | created through GZCOM: class `0x4386D516`, interface `0x5386D516` (the generic window-generator class; the same IID is queried again in `DoSubBudgetWindow`) |
| Dialog window ID | `SetID(0x0423278F)` (`cIGZWin` slot 64, `+0x100`, at `0x00779F96`) |
| `cSC4BudgetUIWinProc` vtables | `0x00AB4A40`, `0x00AB4A2C`, `0x00AB4A1C` |
| Budget-panel tab windows | `0xAA3AC000` and `0xAA3AC001` (ShowWindow `0x00768670` shows one, hides the other by the flag at `+0x194`) |

## 2. Opening a sub-budget dialog (`cSC4BudgetUIWinProc::DoSubBudgetWindow`, `0x00768460`) *[compat]*

Input: one 32-bit **selector**. Only the budget panel's button handler calls it (9 call sites in `DoWinProcMessage`
`0x00768750`). Steps, in order:

1. If a sub-budget object is already held (`+0x18`), do **nothing** (no re-entry, no refresh).
2. Allocate a `cSC4SubBudgetWindow` (0x140 B), construct it (`0x007862B0`), hold one reference at `+0x18`.
3. `CreateSubBudgetWindow(out win, selector, this+0x198 value, 0)` (§3). On failure skip to step 5.
4. On success: `ShowWindow`; `SetFlag(0x800, 1)`; `PullToFront`; `SetFlag(0x800, 0)`; `SetFlag(0x100, 1)`; if the
   window answers interface `0x5386D516`, set generator flags `0x1000`, `0x2000`, `0x4000` to 1. Then
   `WinLocationSaver.RestoreWindowLocation(win, 1)` (app slot 43 service, `+0x28`), **run the window modally**
   (`DoModalWin`, window manager `+0xA4`, `0x00768598`), hand focus back to the 3D view window (manager `+0x94`), and
   `WinLocationSaver.SaveWindowLocation(win)` (`+0x18`). The dialog's position therefore persists per window ID.
5. Always: `DestroySubBudgetWindow` (§6), drop the held object (`+0x18 := null`, Release), then
   `UpdateBalanceSheetInfo` (`0x00767990`) so the main panel shows any changed funding.

### 2.1 Selector table (from the panel's command IDs) *[compat]*

| Panel command (`0x287259F6` button message, value) | Selector | Dialog mode |
|---|---|---|
| `0xAA3AC401`, `0xAA3AC409`, `0xAA3AC501`, `0xAA3AC509` | `1` | ordinances (§4.1) |
| `0xAA3AC402`, `0xAA3AC40A`, `0xAA3AC502`, `0xAA3AC50A` | `2` | neighbour deals (§4.2) |
| `0xAA3AC403`, `0xAA3AC503` | `0x0A5A72D1` | department group (civic group, civic-buildings.md) |
| `0xAA3AC404`, `0xAA3AC405`, `0xAA3AC504`, `0xAA3AC505` | `0xAA369059` | group: transportation |
| `0xAA3AC406`, `0xAA3AC506` | `0x4A357B40` | group: police/fire/jail (protection.md) |
| `0xAA3AC407`, `0xAA3AC507` | `0x6A357B7F` | group: education/health |
| `0xAA3AC408`, `0xAA3AC508` | `0x4A357EAF` | group: utilities (power, water, waste) |
| `0xAA3AC40B`, `0xAA3AC50B` | `0x6A357B96` | group: parks/landmarks |
| `0xAA3AC40C`, `0xAA3AC50C` | `0xEA597195` | group: the second civic group (civic-buildings.md, department `0xEA59717A`) |

The `0xAA3AC4xx` set is the compact panel, the `0xAA3AC5xx` set the expanded one; both map identically. Every
command first passes `nsSC4TutorialSystem::FilterUICommandMessage`; a filtered command is swallowed (returns 1).

Other panel commands seen in the same handler, for completeness: `0xAA3AC400`/`0xAA3AC500` → `DisplayTaxesWindow`;
`0xAA3AC40D`/`0xAA3AC50D` → `DisplayLoanWindow`; `0xAA3AC40E`/`0xAA3AC50E` and `0x4A8879CB` set `+0x18C := 1` then
call the proc's own slot 6 (`+0x18`); `0xAA3AC40F`/`0xAA3AC50F` and `0x4A8879DB` set `+0x18C := 0` then the same
slot; `0x0A4C3354` → `RemoveLoanWindow`; `0x2A4C33A6` / `0xAA4C339C` scroll the loan list by 2 (bounded below by 0 and
above by count − 4); `0xAA4C33E6` takes the loan (§5.2).

## 3. `CreateSubBudgetWindow` (`0x00779E50`, thiscall, 4 args, returns bool) *[compat]*

1. `*out := null`. Create the generator (`0x4386D516` / `0x5386D516`); failure → return false.
2. `*out :=` its `cIGZWin` (AddRef); also hold it at `this+0x0C`.
3. Generator flags, in this order: `1:=1`, `2:=0`, `0x10000:=0`, `0x20:=0`, `0x40:=0`, `0x1000:=1`, `0x2000:=0`,
   `0x4000:=0`; set the generator's window procedure to **this object** (generator `+0x1C`).
4. Window flags: `SetFlag(0x100,1)`, `SetFlag(0x10000,0)`, `SetFlag(4,1)`, `SetFlag(0x800,0)`; `SetID(0x0423278F)`.
5. Store `+0x20 := selector`, `+0x28 := arg3`, `+0x24 := arg4` (the occupant for the individual mode, null from the
   panel). Return true.

The window is empty until the window manager delivers the init message (§4).

## 4. Window procedure `DoWinMsg` (`0x0078BCA0`) *[compat]*

| Message | Action |
|---|---|
| `0xA2BF8AD0` (init) | `on_init` (`0x007876B0`) |
| `0xA2BF8AD1` (shutdown) | `on_shutdown` (`0x0077E440`) |
| 3 / `0x287259F6` (button) | `on_button_activated(win, buttonID)` (`0x0078B120`), §5 |
| 3 / `0x887113A4` (slider moved) | §4.4 |
| 3 / `0xE8711255` (combo box changed) | neighbour mode: rewrite the selected deal rows (vector `+0x50..+0x54`, 24-byte entries), then `updateNeighborBudget` |
| 5 (key down), key `0x1B` (Esc) | close: remove the child held at `+0x14` (window `+0x3C`), drop it, `end_dialog(win, 0x42B7C353)`, return 1 |
| 5, key `0x0D` or `0x73` | the same close, **only** when modifier bit `0x4` is set; otherwise ignored |

`on_init` first takes a **hidden simulator pause** (`cISC4Simulator::HiddenPause`, slot 10, at `0x007878C0`);
`DestroySubBudgetWindow` releases it (`HiddenResume`, slot 13). The date does not advance while any sub-budget dialog
is open. It then branches on the selector (`+0x20`, `0x00787BE7`):

- **1 → ordinances**: load the background image (type `0x856DDBAC`, group `0x46A006B0`, instance `0x140155F2`), then
  `initOrdinanceBudget` (`0x0077C660`): one row per ordinance with an on/off check, cost text; the original on/off
  state and the edited state are kept as byte pairs (`+0xA8`, count `+0xA0`).
- **2 → neighbour deals**: image instance `0x140155D2`, then `initNeighborBudget` (`0x0077E600`), which calls
  `NeighborsSimulator.SetUIDisableFlag(true)` and `ProposeDeals(list, true)` (neighbors.md §7).
- **`+0x24` non-null → one occupant** (`initIndividualBudget`, `0x00786690`).
- **otherwise → a department group**: `budget.GetDepartmentBudgetsInGroup(selector, list)` (budget slot 31,
  `+0x7C`), one slider row per department (`SliderInfo` = {slider window ID, department}, vector `+0x2C..+0x30`), the
  starting funding fraction of each department saved (vector `+0x38..+0x3C`, 8 bytes each), totals with
  `SC4MakeMoneyString`. Groups `0x6A357B96`, `0xEA597195`, `0x0A5A72D1` take a different row layout (tests at
  `0x0078807C`–`0x00788092`); see §8.

### 4.4 Slider moved (`0x887113A4`)

Guarded by a process-wide re-entry byte (`0x00B4DFC8`): a nested slider message while one is being handled is ignored.

- Group mode (`+0x24` null): for every `SliderInfo` whose window ID equals the slider, call the department's
  `SetFundingPercentage(position × 0.01, 0)` (department slot 11, `+0x2C`; budget.md §7.1, which redistributes), then
  `UpdateValues` (`0x007898A0`) to refresh every row's text.
- Occupant mode: look the slider up with `get_window_indiv_item`; if found, `SetOccupantFundingLevel(occupant, which,
  position)` where `which = 0` for slider IDs below `0x0ABCE500` and `1` otherwise, then `updateIndividualBudget`.

## 5. Buttons (`on_button_activated`, `0x0078B120`) *[compat]*

| Button ID | Action | Returns |
|---|---|---|
| `0x67`, `0x42B7C353` | **OK**: `end_dialog` (keeps all edits) | 1 |
| `0x6D` | **Cancel**: `RestoreStartSettings` (§5.1), then as OK | 1 |
| `0x68`, `0x384` | remove and drop the child at `+0x14` (an inner popup); the dialog stays | 0 |
| `0xCC`, `0x385` | `end_dialog` | 1 |
| `0xCD` | ordinances **apply**: for each row whose edited byte differs from its original byte, `OrdinanceSimulator.SetOrdinanceOn(…)` (slot 13); then close the inner child and `end_dialog` | 0 |
| `0xCE` | close the inner child, `end_dialog` | 0 |
| `0xCF` | `FUN_0077AB40` (unnamed, §8), close the inner child, `end_dialog` | 0 |
| `0x451` / `0x452` | scroll list A (`+0x11C`) up/down by 2 rows; page = 9 rows; arrows are hidden at the ends; then refresh the mode's rows (ordinances: `updateOrdinanceBudget`; neighbours: `updateNeighborBudget`) | 0 |
| `0x453` / `0x454` | the same for list B (`+0x120`, length `+0x128`) | 0 |
| `0x0ABCE600`–`0x0ABCE6FF` | occupant row clicked: open the **nested** individual dialog for it (`DoIndividualBudgetWindow`, `0x00786530`, which builds a second `cSC4SubBudgetWindow` through `CreateSubBudgetWindow`) | 0 |
| `0x0ABCDE00`–`0x0ABCDEFF` | occupant mode only: per-occupant control row (§8) | — |

After any other handled button the routine finishes with the mode's refresh and `UpdateValues`.

### 5.1 `RestoreStartSettings` (`0x0077C5E0`)

Group mode: for every saved {fraction, department} pair, call that department's `SetFundingPercentage(&fraction, 0)`
in vector order. Occupant mode: for every `IndividualRestoreInfo` (12 bytes: occupant, level0, level1; vector
`+0x44..+0x48`) call `SetOccupantFundingLevel(occupant, 0, level0)` then `(occupant, 1, level1)`. Ordinance and
neighbour edits are not applied until `0xCD`, so Cancel needs no restore for them.

### 5.2 Loan taken (`0xAA4C33E6`, budget panel)

Reads the amount chosen (`+0x188`), asks the budget simulator for the bond terms (slots `+0xC4`, `+0xBC`), builds a
bond record (amount, months/12 split) and hands it to `FUN_00692110`; then `+0x184 += amount`, `+0x17C += 1`,
`UpdateLoanDisplay`, refresh, `RemoveLoanWindow`. The bond arithmetic is budget.md §5.4.

## 6. Closing (`end_dialog` `0x00779850`, `DestroySubBudgetWindow` `0x0077A1A0`) *[compat]*

`end_dialog(win)`: null → false. If the window has flag `0x1000` (modal), ask its parent to end the modal loop
(parent `+0x24` with code `0xF` and the window twice); otherwise remove it from its parent (`+0x38`). Returns true.

`DestroySubBudgetWindow`: if mode 2, for each held neighbour deal (vector `+0x68..+0x6C`, 8 bytes each): AddRef; if
the deal's flags (slot 36) have bit 0 set, call deal slot 34; Release. Then `NeighborsSimulator.SetUIDisableFlag(false)`
and `MarkDealsDirty()`. In every mode: `Simulator.HiddenResume()`; then `end_dialog` on the held window if any.

Order matters *[compat]*: the pause ends **before** the window is ended, and the panel refreshes its balance sheet
only after both (§2 step 5).

## 7. Budget panel message hooks (`cSC4BudgetUIWinProc::DoMessage`, `0x00768620`) *[compat]*

Messages `0x08DAC3E7` and `0x08DAC3E8` set a dirty byte (`+0x192`); message `0x86AD10EE` (a frame/update tick), when
the byte is set, clears it and runs `UpdateBalanceSheetInfo`. So the panel refreshes at most once per tick however
many money messages arrive. `ShowWindow` (`0x00768670`) refreshes immediately and sets `+0x19D := 1`.

## 8. Open work (filed as questions BW1–BW5, docs/specs/questions/budget-window.md)

Row layout of the four populate routines; the special-case groups in §4; `FUN_0077AB40` (button `0xCF`);
the `0x0ABCDExx` occupant controls; the meaning of panel slot 6 (`+0x18`) toggled by `+0x18C`.

## 9. Conformance tests

1. Opening any selector while a sub-budget dialog is held does nothing.
2. While a dialog is open `IsHiddenPaused()` is true; after OK or Cancel it is false and the panel balance sheet was
   refreshed once.
3. Group mode, move one slider to 30, press Cancel: every department in the group is back to its starting fraction.
4. Ordinances: toggle two rows, press `0xCD`: exactly two `SetOrdinanceOn` calls; toggle and press `0x6D`: none.
5. Neighbour mode: `uiDisabled` is 1 while open, 0 after close, and deals are marked dirty on close.
6. Esc closes; Enter without modifier bit `0x4` does not.
