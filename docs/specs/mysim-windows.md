# Spec: the MySim windows (`cSC4MySimWinProc`, `cSC4PickSimWinProc`)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`). Implementers must work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult
decompiled or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[summary]* is a correct outline whose details are not given.

Related specs: [mysims.md](mysims.md) (the MySim agents: `MoveIn` §4.1, the messages §5, the slots §1.3);
[advisors.md](advisors.md) (advice items and the message server); [ui-windows.md](ui-windows.md) (scripted windows,
`nsGZWinUtils`); [drive-tool.md](drive-tool.md) (the transport choice drives a car or a pedestrian).

## 0. Summary

The MySim window shows the city's up to five MySims: a main view with one button per Sim, and for the selected
Sim a small or large view with its status, its message list and its transport. Adding a Sim opens the **pick-sim**
dialog, where the player chooses a head, a name, a sign and a biography; that dialog moves the Sim in.

## 1. The MySim window *[compat]*

### 1.1 Life cycle

`Init` (`0x0076E9F0`) hides the window's panels, binds the five Sim buttons and registers the window for the
messages of §1.4; `Shutdown` (`0x0076ED10`) unregisters them (and itself from service `0xEBE0E860`), closes an open
pick-sim dialog and releases the window and the five Sim references. `Show` (`0x0076FCE0`) refreshes the Sims and
shows the main panel; `Hide` (`0x0076E8E0`) hides every panel.

### 1.2 Panels

The panels are child windows switched with `SetWindowVisible`: `0xCA1F1D9C`, `0xEA1F1E4D` (small), `0xAA1F1EC5`
(large), `0xEA1F1E5E`, `0x6A61E29F`, `0xABBAA2D3`, and the mode panel `0xEA1F1E4E`.

- `SetMySimMainWindow` (`0x007719F0`): the main panel with the current Sims (`GetCurrentSims`) and the selected one.
- `SetMySimSmallWindow` / `SetMySimLargeWindow` (`0x00771440`, `0x00771550`): the Sim's message list in the small
  or the large text control (`GetMessageList`, `updateBodyTextCtrl`).
- `SetMySimStatWindow` (`0x0076F450`): the Sim's statistics as captions `0x8A1F1EFF`–`0x8A1F1F07` (name, home,
  job and the numbers of mysims.md §2.2) *[summary of the fields]*.
- `SetMySimModeWindow` (`0x00771660`): the mode panel with the Sim's name and portrait.
- `SetDialogState` (`0x0076E7B0`): only panel `0xEA1F1E5E` visible.

### 1.3 The Sims

`GetCurrentSims` (`0x0076F7C0`) reads the MySim agent simulator's Sims into the five slots, with their portraits
(`0x46A006B0` images) and buttons; a free slot shows the "add" button. `SetCurrentSim(slot)` (`0x00771940`)
selects a Sim (opening the pick-sim dialog for an empty slot, `AddSim`) and updates the transport button
(`SetTransportButtonImage`, `0x0076EE90`). `BootSim` (`0x00771B40`) asks the agent to drop the selected Sim
(button `0xEA1F1DBA`) and returns to the main panel. `AddSim` (`0x0076E830`) disables the small and large panels
and runs the pick-sim dialog (§2), then releases it.

### 1.4 Messages (`DoMessage`, `0x007721A0`)

| Message | Action |
|---|---|
| `0xAA25827C` MySimsChanged (mysims.md §5.1) | while shown: re-read the Sims; keep the selected slot when it still holds a Sim; else the main panel |
| `0xCA09FA6C` | for an advice item (interface `0x8A09F5F5`) of the selected Sim: `AddMessage` (`0x0076EF40`), which takes the item out of the window's set-aside list (`+0x38`), and the text is redrawn |
| `0x8A09FA64` | for an advice item of the selected Sim: `RemoveMessage` (`0x0076EFB0`), which puts the item in the set-aside list and clears its flag `0x20`, and the text is redrawn |
| `0x8A09FA65` | the text is redrawn |
| `0xAC08746E`, `0xAC08746F` | the transport choice (§1.6) |

### 1.5 The message text (`updateBodyTextCtrl`, `0x00770740`)

The Sim's messages are written as HTML into the text control: each item's text in a colour by its kind
(`#8e9499`, `#3f4967`, `#095a22`, `#b00000`, a blinking `#f70a0b`, `#0f0f0f`), bold titles in the face
`StocletITC TT` and italic notes in `Arta`, with an icon per item from the images `sc4://HTML/46a006b0/14416250` to
`…/1441625F` (some selected by flag bits `0x1000000`–`0x8000000`) *[summary of the item layout]*. `GetMessageList`
(`0x0076F260`) clears flag `0x20` of the set-aside items that lack flag 2, empties the set-aside list and copies
the Sim's advice items *[as read]*.

### 1.6 Transport (`ShowChooseTransportDialog`, `0x00770420`)

A modal dialog (game paused; script window `0xCBF32603`) offers the Sim's ways to travel: its vehicles
(`DisplayVehicleIcons`, `0x0076FDB0`: the automata exemplars of the Sim's group, with their icons from properties
`0xEBFC5E26`, `0x2C0C922E`, `0xEBFC5FBA`, `0x899AFBAD`, `0xEBFC5E5E` and their names from `0x8A416A99`, or
`(Automata needs a name!)`) and walking (buttons `0x2C201CB0`–`0x2C201CB2`, `0x34560000`). Choosing a car posts
`0xAC08746E` and choosing to walk `0xAC08746F` (`s_sim_select_car`, `s_sim_select_pedestrian`, `0x0076E710`,
`0x0076E760`) *[inferred: the window then starts U-Drive-It with that Sim, drive-tool.md]*. The change-home and
change-job buttons create the MySim tools (`cSC4MySim::CreateChangeHouseTool`, `CreateChangeJobTool`; mysims.md
§4.7).

## 2. The pick-sim dialog *[compat]*

`Init` (`0x00775CC0`) builds the dialog from its script (window `0x6A243D9E`), reads the MySim templates of the
pool (`cISC4MySimPool::tMySimTemplate`: name, portrait, sex, sign, biography), and fills the name (`0x8A243F1F`),
biography (`0x8A243F6E`) and sign (`0xAA243F5E`) fields from strings of table `0x6A231EAA`.

- **Heads.** `SetupPickSims(first)` (`0x007751A0`) shows up to 21 portraits from `first` (clamped so the last page
  is full); `SetScrollButtons` (`0x00774690`) enables the scroll buttons (`0x12345000`–`0x12345105`).
  `SortByName` (`0x007754B0`) sorts the templates by name with the language manager's comparison, ascending or
  descending. `PickHead` (`0x007757F0`) selects a template and copies its portrait, name, biography and sign into
  the fields.
- **Name.** `ValidateText` (`0x00774FC0`) enables the OK button only when the name has a character above space;
  `DoMessage` (`0x007750D0`) re-validates on each tick, and `TickSelf` (`0x00774620`) posts the tick message
  `0xCC327B60` to the dialog.
- **Move in.** `MoveIn` (`0x007755A0`) reads the name, biography and sign, and calls the agent simulator's
  `MoveIn` (city slot 71; mysims.md §4.1) with them, the template's sex, portrait key and path and the slot; then
  it ends the dialog.
- `EndDialog` (`0x007745C0`) and `Shutdown` (`0x007766A0`) close it and free the templates. `DoWinProcMessage`
  (`0x00776760`) routes the buttons (scrolling, sorting, OK, cancel; Enter) *[summary]*.
- `SC4InsertNumberIntoString` (`0x00775BE0`) replaces a marker character in a string with a number formatted by
  the language manager *[inferred: the slot or count in captions]*.

## 3. Quirks

1. The pick-sim page holds 21 portraits; with fewer than 21 templates the page starts at the first.

## 4. Open questions

See [questions/mysim-windows.md](questions/mysim-windows.md) (MW1–MW2).

## 5. Conformance tests

1. With three Sims, the main panel shows three Sim buttons and an add button.
2. A name of only spaces leaves the OK button disabled.
3. Asking for page start 30 with 40 templates shows templates 19–39.
4. Message `0x8A09FA65` redraws the selected Sim's text without changing its list.
