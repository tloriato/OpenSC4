# City tools and input: the view input control framework and the place-lot tool (behaviour spec)

Status: behaviour spec, written 2026-10-21 (re-citytools4 lane). Every fact was checked on the Windows executable
`bin/SimCity4.exe` (1.1.641, image base `0x00400000`); every address is a Windows address. Layouts and numeric
details were checked against the instructions. Implementers work from this spec, the public LGPL gzcom-dll SDK
headers and general knowledge only; they must **not** consult decompiled or disassembled code.

Tags: *[compat]* observable, a replacement must match it; *[choice]* a FreeCity4 decision; *[inferred]* consistent
with all the evidence, not proven; *[quirk]* original behaviour that looks unintended but is reproduced.

Outline: §1 scope; §2 the base class `cSC4ViewInputControl` (identity, layout, slot table, lifecycle, cursor,
capture, stack queries, result sound); §3 the place-lot tool `cSC4ViewInputControlPlaceLot` (construction, Init,
Shutdown, messages, key, mouse, the lot's cells, the placement test, the automatic facing, placing, toll booths,
feedback); §4 quirks; §5 open questions; §6 conformance tests; §7 coverage; §8 the query
tool `cSC4ViewInputControlQuery` (creation, layout, slots, Init, Shutdown, mouse, messages, pick filters,
inventory).

## 1. Scope

The embedded local grid helper's own snapping, transform, resource and lifecycle
rules are specified in [local-grid-helper.md](local-grid-helper.md). This spec
describes the tools' calls into it.

A *view input control* is the object the city's 3D view (`cSC4View3DWin`, ui-windows.md §3) forwards mouse and key
input to while a tool is active. The 3D view keeps a stack of them (`SetCurrentViewInputControl` slot 25,
`RemoveCurrentViewInputControl` slot 26, persistent controls slots 28-30; ui-windows.md §3.3). The stack itself is
the 3D view's (not specified here). This spec covers:

- the shared base `cSC4ViewInputControl` (17 functions), which every tool class derives from;
- the place-lot tool (plop a building or lot from the catalog): its lifecycle, input dispatch, placement test,
  facing and placing.

Not here: the network tools' back end (network-tools.md; `cSC4NetworkToolUI::DoPlacement` §8.2 there), the
dispatch tool (dispatch.md), the back ends of zone, demolish, disaster, flora tools (their specs), the window and
widget layer (ui-windows.md).

## 2. `cSC4ViewInputControl` (base)

### 2.1 Identity *[compat]*

- Primary vtable `0x00AA6658` (30 slots). `QueryInterface` (`0x005FB100`) answers IID `0x00000001` (IGZUnknown) and
  `0xC6BB8345` (the view-input-control interface), each with `AddRef`; any other IID returns false.
- Reference counting is the shared GZ pattern: `AddRef` increments `+0x08`; `Release` (`0x004B9E20`) decrements
  while the count is above 1, and on the last reference calls `FinalRelease` (slot 29, `0x005FB0E0`), which calls
  `Shutdown` (slot 4) and then the deleting destructor (slot 26) with flag 1.
- Derived constructors run the base constructor first, then install their own vtables (§3.1). The base
  constructor body was not re-read for this spec (Q CT4).

### 2.2 Layout *[compat]* (the derived tools read these offsets directly)

| Offset | Type | Meaning |
|---|---|---|
| `+0x00` | vtable | |
| `+0x04` | bool | initialised: set true by `Init`, false by `Shutdown` |
| `+0x08` | uint32 | reference count |
| `+0x0C` | uint32 | control ID (slot 5 `GetID` returns it; slot 6 `SetID(id)` stores it) |
| `+0x10` | uint32 | cursor resource ID |
| `+0x14` | cIGZCursor* | cursor object (held: AddRef on store, Release on replace); slot 7 returns it |
| `+0x18` | cIGZWin* | the window the control works in (slot 10 `SetWindow`, stored without AddRef) |
| `+0x1C` | cISC4View3DWin* | the city 3D view, taken from the global 3D-view pointer `0x00B43DC4` at `Init` (held) |
| `+0x20` | cIGZWinMgr* | the window manager (`RZ::WM()`), taken at `Init` (held) |

### 2.3 Slot table (vtable `0x00AA6658`) *[compat]*

Names follow the SDK's `cISC4ViewInputControl` order; "stub" rows are shared one-instruction bodies.

| Slot | Off | Target | Method | Pop | Base behaviour |
|---|---|---|---|---|---|
| 0 | +0x00 | `005fb100` | QueryInterface(iid, out) | 8 | §2.1 |
| 1 | +0x04 | `004bdf00` | AddRef | 0 | `++[+0x08]` |
| 2 | +0x08 | `004b9e20` | Release | 0 | §2.1 |
| 3 | +0x0C | `005fb1d0` | Init() | 0 | §2.4, returns true |
| 4 | +0x10 | `005fafd0` | Shutdown() | 0 | §2.4, returns true |
| 5 | +0x14 | `00517f70` | GetID() | 0 | returns `+0x0C` |
| 6 | +0x18 | `00989960` | SetID(id) | 4 | stores `+0x0C` |
| 7 | +0x1C | `0040ce70` | GetCursor() | 0 | returns `+0x14` (no AddRef) |
| 8 | +0x20 | `005fb070` | SetCursor(cursorId) | 4 | §2.5 |
| 9 | +0x24 | `005fb020` | SetCursor(cIGZCursor*) | 4 | §2.5 |
| 10 | +0x28 | `005fb0b0` | SetWindow(win) | 4 | stores `+0x18` |
| 11 | +0x2C | `00648ef0` | IsSelfScrollingView() | 0 | stub: false |
| 12 | +0x30 | `00648f00` | ShouldStack() | 0 | stub: true |
| 13 | +0x34 | `005deb40` | OnCharacter(ch) | 4 | stub: false |
| 14 | +0x38 | `0062c980` | OnKeyDown(vkey, modifiers) | 8 | stub: false |
| 15 | +0x3C | `0062c980` | OnKeyUp(vkey, modifiers) | 8 | stub: false |
| 16-19 | +0x40..+0x4C | `005be2e0` | OnMouseDownL / DownR / UpL / UpR (x, z, modifiers) | 12 | stub: false |
| 20 | +0x50 | `005be2e0` | OnMouseMove(x, z, modifiers) | 12 | stub: false |
| 21 | +0x54 | `00735290` | OnMouseWheel(x, z, modifiers, delta) | 16 | stub: false |
| 22 | +0x58 | `00648ef0` | OnMouseExit() | 0 | stub: false |
| 23 | +0x5C | `00407990` | Activate() | 0 | stub: nothing |
| 24 | +0x60 | `00407990` | Deactivate() | 0 | stub: nothing |
| 25 | +0x64 | `005fb170` | AmCapturing() | 0 | §2.6 |
| 26 | +0x68 | `005fb1b0` | deleting destructor(flags) | 4 | resets vtable, frees when `flags & 1` |
| 27 | +0x6C | `005fb0c0` | EndInput() | 0 | §2.7 |
| 28 | +0x70 | `005fb260` | PlayResultSound(soundId) | 4 | §2.8 |
| 29 | +0x74 | `005fb0e0` | FinalRelease() | 0 | §2.1 |

Input handlers return a bool: true means "handled". How the 3D view uses it depends on the event *[compat]*
(view3d-window.md §6c, instruction-checked at `0x007E66D0`; answers CT1): the view keeps a **stack** of controls
(the tool made current; the top is the last element) and a list of **persistent** controls (the city-situations
tool). Each list is visited from the last element to the first, with an AddRef/Release around every visit.
- Key down/up: the stack from the top, stopping at the first true; then **every** persistent control (results
  ignored, so the persistent pass runs even when a tool took the key); the view's result is true when any of them
  returned true.
- Mouse press and release (left and right): the persistent controls first-taker, then the stack first-taker.
- Mouse move: every persistent control, then the stack first-taker. Wheel: the stack first-taker; only when none
  took it, every persistent control and then the zoom. Character: the stack first-taker, then the persistent
  controls (with a bound quirk, view3d-window.md §6c). Mouse exit: only the top stack control, then every
  persistent control.
- `ShouldStack` (slot 12) plays no part in this: the view's `SetCurrentViewInputControl` pushes any control whose
  `Init` succeeds and nothing in the view calls slot 12 (the `Dispatch`, `Effect`, `EffectDisaster`, `Query` and
  `Torch` tools override it to return false, the other tools return true; neither value is ever consulted)
  *[inferred from the absence of any caller in the view's code]*.

`x`, `z` are window-client pixel coordinates passed through to `PickTerrain` (3D view slot 36). The `modifiers` word
is the window message's flag word (gz-graphics.md, built at `0x0098B750`): `0x01` Shift, `0x02` Ctrl, `0x04` Alt,
`0x10` left button, `0x20` right button, `0x40` middle button, each set while that key or button is down.

Further non-virtual helpers: `IsKeyToBePassed` (`0x00505BE0`) returns true; `SetCapture` (`0x005FB130`) and
`ReleaseCapture` (`0x005FB150`), `IsOnTop` (`0x005FB190`), and the non-deleting destructor `0x005FAFC0`
(vtable reset only).

### 2.4 Init and Shutdown *[compat]*

`Init` (`0x005FB1D0`), only when not yet initialised:

1. If no cursor object is held and the cursor ID is non-zero: ask the window manager's cursor manager
   *[inferred name]* (WM slot 32 `+0x80`, then its slot 9 `+0x24`(id)) for the cursor and hold it (AddRef if non-null).
2. If a cursor is held and a window is set: window slot 85 (`+0x154`, set cursor)(cursor, true).
3. Hold the 3D view from `0x00B43DC4` (AddRef when non-null; null is stored as is).
4. Hold the window manager (`RZ::WM()`, AddRef; no null test).
5. Set initialised. Returns true either way.

`Shutdown` (`0x005FAFD0`), only when initialised: release and null, in this order, the window manager `+0x20`,
the 3D view `+0x1C`, the cursor `+0x14`; clear initialised. Returns true. The window `+0x18` is not released (it was
never held) and the cursor ID is kept, so a later `Init` reloads the same cursor.

### 2.5 SetCursor *[compat]*

- `SetCursor(cIGZCursor* c)` (slot 9): nothing when `c` equals the held cursor. Otherwise AddRef `c` (if non-null),
  Release the old one, store `c`; then if `c` is non-null, the control is initialised and a window is set, call the
  window's set-cursor slot (`+0x154`)(c, true). Setting null releases the old cursor without telling the window.
- `SetCursor(id)` (slot 8): look the ID up through the window manager's cursor manager (as §2.4 step 1). Only if
  the manager exists and returns a cursor: store `id` at `+0x10` and call slot 9 with that cursor. An unknown ID
  leaves both the ID and the cursor unchanged.

### 2.6 Capture and the stack top *[compat]*

- `SetCapture()`: with a window set, window manager slot 29 (`+0x74`)(window); returns that result; no window:
  false. `ReleaseCapture()`: the same with slot 30 (`+0x78`).
- `AmCapturing()` (slot 25): with a window set, true when window manager slot 28 (`+0x70`, current capture
  window) equals the window; no window: false.
- `IsOnTop()`: true when the 3D view's `GetCurrentViewInputControl` (slot 24) returns this control. No null test
  on the 3D view *[quirk]*: calling it before `Init` (or with no city view) faults in the original; a
  replacement returns false *[choice]*.

### 2.7 EndInput *[compat]*

`EndInput()` (slot 27): if the 3D view is held, call its `RemoveCurrentViewInputControl(false)` (slot 26). It
removes whatever control is on top of the stack, not necessarily this one *[compat]*; tools call it on themselves
while they are on top.

### 2.8 PlayResultSound *[compat]*

`PlayResultSound(soundId)` (slot 28): when `soundId != 0` and the sound system (`cIGZSndSys`, global
`0x00B43CB8`, audio.md) exists, call its slot 11 (`+0x2C`) with `(1, soundId, 0, 0, 0)`. No other effect. The base
class does not call it itself; derived tools call it after an action.

## 3. The place-lot tool `cSC4ViewInputControlPlaceLot`

### 3.1 Identity and creation *[compat]*

- Object size `0x158`. Two vtables: primary `0x00A90738` (the view-input-control interface, §2.3 layout) and a
  message-target vtable `0x00A90728` at `+0x28` whose slot 3 is `DoMessage` (`0x004C2F20`).
- Overrides: `QueryInterface` (`0x004BDF10`), `Init` (`0x004C13E0`), `Shutdown` (`0x004C0300`), `OnKeyDown`
  (`0x004C3060`), `OnMouseDownL` (`0x004C3120`), `OnMouseMove` (`0x004C3300`), `OnMouseExit` (`0x004C36C0`),
  destructor (`0x004C0070`). Every other slot is the base's (§2.3): right button, mouse up, wheel, `OnKeyUp`,
  `OnCharacter` return false; `ShouldStack` true.
- Constructor `0x004C0430`: `(buildingType, lotConfigId, buildingExemplar, monopolyFlag)` stored at `+0x2C`,
  `+0x30`, `+0x34` and the byte `+0x50` respectively *[inferred names, from their use in Init; the first is a mode:
  0 = by lot configuration, otherwise by building type (re-plop)]*. Created by
  `cSC4View3DWin::InvokeCatalogItemCommand` (`0x007EA49B`), by two cheat codes in `cSC4View3DWin::ProcessCheatCode`
  (`LotPlop` `0x007F35D9` with `(0, 0, 0, 0)`, `BuildingPlop` `0x007F363B` with `(1, 0, 0, 0)`, view3d-window.md;
  corrected 2026-10-22, re-plop: not all 0) and by `cGZToolCommandSystemSupport::DoMessage` (`0x005C5441`).
  The catalog path installs it with the 3D view's `SetCurrentViewInputControl(tool, 2)` (slot 25).
- Constructor defaults worth matching: rotation `+0x38` = `+0x3C` = 0, last rotation shown `+0x40` = -1, last mouse
  `+0x44`/`+0x48` = 0, last placed rectangle `+0x78..+0x84` = (0, 0, -1, -1), cursor world position
  `+0x88`/`+0x8C`/`+0x90` = (0, -FLT_MAX, 0), placeable `+0x94` = false, feedback flag `+0x95` = false, toll-booth
  mode `+0x150` = false, cost-indicator values `+0x138` = -100000 (int64 low/high `+0x138`/`+0x13C` = -100000, -1),
  plops left `+0x154` = 1000.

### 3.2 Init (`0x004C13E0`) *[compat]*

Only when not initialised (the initialised byte `+0x04`). Steps:

1. Control ID `+0x0C` := `0x88F154FB`; cursor ID `+0x10` := `0x88E70B76`; then the base `Init` (§2.4).
2. Subscribe its message target (`+0x28`) on the message server (`0x00B43CCC`, slot 5 `+0x14`) to, in order:
   `0x277A9364`, `0x99EF1142` (occupant inserted), `0x99EF1143` (occupant removed), `0x49D5B83F`, `0xA6B79602`,
   `0xC9F775BB`.
3. Hold the lot-configuration manager (`0x00B43D18`) at `+0x4C` and the building-development service
   (`0x00B43D68`) at `+0x68` if not already held.
4. Choose what to plop:
   - **No building type** (`+0x2C == 0`): if no lot configuration was given, build a monopoly filter
     (`cSC4LotConfigurationFilterMonopoly(+0x50)`), let the lot-configuration manager fill the candidate set
     `+0x54` (manager slot 13 `+0x34`), take the first candidate, and let the user pick
     (`DoUserLotConfigurationTypePick`, `0x004BEEF0`). The lot configuration ID `0xEBF9A440` sets toll-booth mode
     `+0x150`.
   - **Building type given**: when no exemplar was given, start from the 3D view's last selected building
     (slot 51), fall back to the first entry of the building-type list (service slot 11 `+0x2C`), let the user
     pick (`DoUserBuildingTypePick`, `0x004BF2F0`) and store the choice back (3D view slot 53). A null choice ends
     here. Then fill the candidate set with `cSC4LotConfigurationFilterByBuildingType(exemplar)`; when the count at
     `+0x64` is zero (an empty set *[inferred]*) `Init` calls the tool's own `Shutdown` and returns false
     *[compat]*. Otherwise start from the 3D
     view's last selected lot (slot 52), let the user pick a lot configuration, store it back (slot 54).
   - The pickers (`DoUserLotConfigurationTypePick`, `DoUserBuildingTypePick`) build a "Lot Plop" list window of
     the candidates, each shown as `0x%08x`, and return the chosen one. They therefore appear only on the debug
     cheats: `LotPlop` (mode 0 without a configuration) and `BuildingPlop` (mode 1: the building picker, as no
     building is given, then always the configuration picker). The catalog and the tool command pass a
     configuration in mode 0 (2026-10-22, re-plop).
5. Resolve the building exemplar (directly, or the first building of the chosen lot configuration) and read the
   int64 **plop cost** (`BuildingProfile` `+0x30`, exemplar property `0x49CAC341`, building-development.md)
   from the building-development service's `GetBuildingProfilePtr` (slot 40 `+0xA0`) into `+0x128`/`+0x12C`; a
   null profile leaves the constructor's −100000. `CanPlaceLot` reads it (§3.10; CT3 answered 2026-10-22,
   re-plop).
6. Build the preview model (`GetBuildingPreviewModel`, `0x004C0680`), init the local grid helper `+0xAC`, create
   the ring effect `+0xD8` from the effects manager (`0x00B43D1C`) if absent, and register the coverage view
   `+0x140` with the view manager (slot 4 `+0x10`) when it is non-zero. The preview model sets `+0x140`: `0xA`
   for a building in occupant group `0x1401` (water), `0xF2` for group `0x1302` (subway), otherwise it keeps its
   value (0 from the constructor); the view manager is the city view's (slot 6 `GetViewManager`, through
   `0x00602180`). `0xA` is the Water data view; `0xF2` is the subway-building placement view,
   distinct from the subway-network tool view `0xF1` (map-view.md §2; §3.14 below).
7. Return true.

### 3.3 Shutdown (`0x004C0300`) *[compat]*

Only when initialised: if the coverage overlay was raised (`+0x118`), clear it and recalculate the 3D view's
coverage helper (slot 49); unregister `+0x140` from the view manager (slot 5); clear the cursor text with key
`0xE9C877FA` (3D view slot 41); clear the cost indicator (slot 44, false); set the coverage helper's categories to
0 and recalculate; shut down the local grid helper; release the configuration effects (`0x004C0090`); release the
building-development service and the lot-configuration manager; unsubscribe the six messages of §3.2 step 2; then
the base `Shutdown`.

### 3.4 Messages (`DoMessage`, `0x004C2F20`) *[compat]*

Always returns false (the message is not consumed).

| Message | Action |
|---|---|
| `0x277A9364` | show or hide the local-grid effect: show when the terrain's (`0x00B43CF4`, not the city; corrected 2026-10-22, re-plop) slot 66 object reports slot 20 false |
| `0x49D5B83F` | `DoPlopRingEffectFinished` (`0x004BE0A0`) |
| `0x99EF1142`, `0x99EF1143` | unless `+0x118` is set: recalc the 3D view's coverage helper |
| `0xA6B79602`, `0xC9F775BB` | stop and release the two preview effects `+0xDC`, `+0xE0` (stop with slot 4(0) when slot 5 says running), then `SetVisualUserFeedback(+0x94, +0x95, +0x88)` to rebuild them |

`DoPlopRingEffectFinished` clears `+0x118` unconditionally. With a held 3D view it obtains
the coverage helper (view slot 49) and recalculates coverage, even if the flag was already clear;
without a view it only clears the flag. It neither stops nor releases the ring and does not change
placeability, categories, configuration or the number of plops left. Evidence: `0x004BE0A0–0x004BE0BD`
(instruction-verified; CT22, re-citytools-ct5, 2026-10-23).

### 3.5 OnKeyDown(vkey, modifiers) (`0x004C3060`) *[compat]*

Handled only while the tool is on top (§2.6) and the 3D view has a key accelerator (slot 31). The accelerator's
slot 10 (`+0x28`)(vkey, modifiers, 1, &command) translates the key; commands:

- `0x6A935CD5`: rotation := (rotation + 1) mod 4;
- `0x6A935CD1`: rotation := (rotation + 3) mod 4 (one step back).

The new rotation is written to both `+0x3C` and `+0x38`; then the tool re-runs its own `OnMouseMove` at the last
mouse position with modifiers `0x100` (§3.7, manual-rotation path) and calls `SetVisualUserFeedback`; returns
true. Any other key, or no translation: false.

### 3.6 OnMouseDownL(x, z, modifiers) (`0x004C3120`) *[compat]*

1. Pick the terrain under `(x, z)` (3D view slot 36, `PickTerrain(x, z, &pos, 0)`, `pos` preset to 0).
2. Only if a point was hit **and** the last move said placeable (`+0x94`): `PlaceLot(pos, rotation +0x3C, true,
   &rect)` (`0x004C1740`).
3. On success: read the cell width from the terrain (`0x00B43CF4`, slot 6 `+0x18`, as in lot-developer.md; corrected
   2026-10-22, re-plop) and store the placed rectangle in world
   units: `+0x78` = `x0*size`, `+0x7C` = `z0*size`, `+0x80` = `(x1+1)*size`, `+0x84` = `(z1+1)*size`, each
   truncated to an unsigned 32-bit integer (§4 Q4); `SetVisualUserFeedback(true, false, pos)`; stop and release
   the effects `+0xDC` and `+0xE0`; clear the cost indicator (slot 44, true); placeable := false. Then
   plops-left `+0x154` decrements and when it drops below 1 the tool ends itself (`EndInput`, slot 27).
4. On failure (no hit, not placeable or `PlaceLot` refused): play the error sound through `cIGZSndSys` slot 11
   with `(0xEA451903, 0xAA62C9D5, 0, 0, 0)`.

Returns the `PlaceLot` result (false when it was not called).

### 3.7 OnMouseMove(x, z, modifiers) (`0x004C3300`) *[compat]*

1. Save the old feedback (placeable `+0x94`, flag `+0x95`, position `+0x88..+0x90`); clear the cursor-text buffer
   `+0x98`; remember `(x, z)` in `+0x44`/`+0x48`; flag `+0x95` := false.
2. Pick the terrain. **No hit:** load the string `(0x0A419226, 0xCA711914)` as cursor text, placeable := false,
   clear the cost indicator (false).
3. **Hit:** the candidate rotation is `+0x38`; a 4-int cell rectangle starts at (-1000, -1000, -1000, -1000).
   - modifiers bit `0x100` set: `GetLotCenterAndCellCoverage` (`0x004BE750`) with that rotation;
   - else toll-booth mode: `PickTollBoothLotAndOrientation` (`0x004BFB30`), switching the lot configuration
     (`ChangeLotConfiguration`, `0x004C1360`) when it picks another one, and taking its rotation into `+0x3C`;
   - else: `GetAutomaticFacingAndCellCoverage` (`0x004BEB70`) for the current lot configuration (manager slot 8);
     on success its rotation goes to `+0x3C`.
   If that step succeeded, `CanPlaceLot` (`0x004C1110`) decides placeable and writes the cost into `+0x138`
   (int64). Placeable: the stored rectangle `+0x78..+0x84` resets to (0, 0, -1, -1). Not placeable: rotation `+0x3C`
   := `+0x38`. Cost > 0: show it (3D view slot 43 `+0xAC`(low dword)); cost <= 0: clear the indicator.
   If the step failed, `CanPlaceLot` still runs (for its reason text) and the result is not placeable.
4. If the cursor text changed (`0x00454BB0` compare), show it with key `0xE9C877FA` (3D view slot 40 `+0xA0`).
5. When placeable, flag, any coordinate of the position (by more than `1e-4`, float `0x00A90688`) or the shown
   rotation (`+0x40` vs `+0x38`) changed: `SetVisualUserFeedback(placeable, flag, pos)` and store the new values.
6. If the held window manager exists, request an **input-message queue flush** through slot 19
   (`+0x4C`); ignore its result and return true. This request also occurs for a failed terrain pick
   and for a move that leaves feedback unchanged. The named `SC4FlushInputMessageQueue` wrapper
   (`0x005F8610–0x005F8620`) makes exactly this same null-guarded call; the move's call is
   `0x004C369C–0x004C36A5`. This is an input-queue operation, with no additional tool state change
   or explicit redraw/cursor-refresh request (CT2, re-citytools-ct5, 2026-10-23).

### 3.8 OnMouseExit (`0x004C36C0`) *[compat]*

Flag `+0x95` := false; `SetVisualUserFeedback(+0x94, false, +0x88)`; empty the cursor-text buffer; clear the cursor
text (`0xE9C877FA`) and the cost indicator (false); return true.

### 3.9 The lot's cells (`GetLotCenterAndCellCoverage`, `0x004BE750`) *[compat]*

*Added 2026-10-22 (re-plop).* `GetLotCenterAndCellCoverage(rotation, x, z, &cx, &cz, &rect)` → bool. The lot
configuration's size `(sx, sz)` (configuration slot 7) is swapped for rotations 1 and 3. With `w` the terrain's cell
width (`0x00B43CF4`, slot 6), on each axis with size `s` and coordinate `p`:

- `s` odd: `c := trunc(p / w)` (the cell under the point); the centre is `(c + 0.5) · w`; the cells are
  `c − s/2 … c + s/2`.
- `s` even: `c := trunc((p + w/2) / w)` (the nearest cell corner); the centre is `c · w`; the cells are
  `c − s/2 … c + s/2 − 1`.

(`s/2` is a whole division.) The centre replaces `(x, z)`. The result is true when the rectangle lies inside the
terrain (`x0 ≥ 0`, `x1 <` terrain slot 7, `z0 ≥ 0`, `z1 <` terrain slot 8: the cell counts).

### 3.10 The placement test (`CanPlaceLot`, `0x004C1110`) *[compat]*

*Added 2026-10-22 (re-plop).* `CanPlaceLot(&point, rotation, &rect, &valid, &cost, &code, &message)` → bool:

The initial outputs are rectangle (−1, −1, −1, −1), validity false, int64 cost 0,
code 0 and an empty message. Checks have the following precedence; failure leaves
later checks unevaluated and returns false:

| Check | Required outcome and output effect |
|---|---|
| Configuration lookup | Manager slot 8 with `+0x30` must provide a lot configuration |
| Cell coverage (§3.9) | Publishes the rectangle and `valid`; invalid coverage rejects placement |
| Repeat-click exclusion | Rejects a centre whose integer-truncated x/z coordinates are inside the last placed world rectangle `+0x78..+0x84`, including all edges (§3.6) |
| Price | With plop cost `+0x128` zero, cost remains zero; otherwise it is plop cost plus `GetLotDevelopmentCost(rect)` (development service slot 36 `GetLotDeveloper`, developer slot 32; lot-developer.md §11.4) |
| Lot creation test | Publishes `CanCreatePloppedLot(rect, rotation, cfg)` from lot manager `0x00B43D08`, slot 21 (zones.md §8.4.1) |

Creation code 8 (free) permits the demolition dry run below. Its success adds the
demolition cost and makes the overall result true; its failure produces message
`0x6A711969`. Codes 0–7 reject placement with the messages in zones.md §8.4.1
(group `0x0A419226`); larger codes reject it without a message.

**`CanDemolishLot(rect, …, &cost)`** (`0x004BEAB0`): a dry run of `DemolishRegion(false, rect cells, privilege 1,
flags, …, &cost)` (demolition.md §4.4) with flags `0x2800`, or `0x800` when `+0x140` is `0xF2` (a subway station,
§3.2 step 6); its result.

### 3.11 The automatic facing *[compat]*

*Added 2026-10-22 (re-plop).*

- **`GetAutomaticFacingAndCellCoverage(cfg, &point, &rotation, &rect)`** (`0x004BEB70`): for each rotation 0–3,
  when §3.9 is valid, `CanCreatePloppedLot` gives 8 and `CanDemolishLot` succeeds, the rotation's score is the
  street count below, otherwise −8. The best rotation is the first with the highest score. The player's rotation
  (`+0x38`) is kept when (its score ≠ 0 or the best score < 1) and its score ≥ best − 4; otherwise the best one is
  taken. The rectangle and centre are the chosen rotation's; the result is whether any rotation was placeable.
  Equality at four points keeps the player's rotation: own 3 / best 7 keeps it, own 3 / best 8
  changes it. The separate zero-frontage exception still changes own 0 / best 1. This follows the
  signed greater-or-equal comparison at `0x004BED15–0x004BED2E`, not an inclusive four-point
  improvement threshold (CT24, re-citytools-ct5, 2026-10-23).
- **Street count** (`GetLotFacingStreetCount`, `0x004BE5D0`): along the row of cells just outside the lot on its
  facing side (rotation 0: `z0 − 1`, 1: `x1 + 1`, 2: `z1 + 1`, 3: `x0 − 1`), for each cell inside the city, the
  network occupant of a road, street, avenue or one-way road (traffic network map slot 8, mask `0x449`) adds 4 for
  an avenue (flag `0x40`), 2 for a road (flag 1), 1 for a street or one-way road (flags `0x408`).

### 3.12 Placing (`PlaceLot(&point, rotation, effects, &rect)`, `0x004C1740`) *[compat]*

*Added 2026-10-22 (re-plop).*

1. `rect` := (−1, −1, −1, −1). With an unknown cost (`+0x138` is −100000), `CanPlaceLot` runs first (false →
   false) and fills `+0x138`.
2. The lot configuration must exist. The lot's cells and centre: in toll-booth mode §3.13 (another configuration
   swaps the preview: `ReleaseLotConfigurationEffects`, `+0x30` := it, `GetBuildingPreviewModel`; its rotation goes
   to `+0x3C`), else §3.9.
3. **Tutorial:** when the city's tutorial system (city slot 82) reports a tutorial (its slot 8, `IsTutorial`) with a
   current task (slot 10, `GetCurrentTask`), the lot's cell rectangle must equal the task's rectangle (task slot 5;
   the four-int test `0x00441B20`).
4. **Funds:** a cost of 0 is free; otherwise the budget's (`0x00B43D64`) `GetTotalFunds` (slot 6) must be at least
   the cost.
5. **Clearing:** `DemolishRegion(true, cells, privilege 1, 0x2000, false, filter, …)` with the tool's filter
   `cSC4PlaceLotDemolishFilter` (`0x004BDF70`; its type test `0x004BDFB0` leaves power poles `0x2890D4DE` and power
   lines `0x0990B2EA` standing); then `PlaceZone(cells, 0, true, skipFunds := cost is 0, true, true, false, null, null, null)`
   (dezoning, zones.md §6).
6. **Zoning:** the zone type is 15 (plopped); 11 when the preview building is in occupant group `0x1508`; 12 when it
   is in group `0x1509` or the configuration is compatible with water (configuration slot 14) *[inferred: airports,
   seaports]*. `PlaceZone(cells, type, true, true, true, true, false, null, null, null)`.
   Both calls explicitly enable `checkSlope`, skip the size check and disable `checkLotInside`;
   their last three output pointers are null (signature in zones.md §6). The first skips funds only
   for a zero tool cost; the second always skips funds. Each region contains all cells in the rectangle.
   Demolition must succeed before dezoning, dezoning before zoning, and zoning before lot creation.
   A false result prevents the later calls and returns placement failure when no lot exists. The
   tool makes no rollback call for earlier successful clearing/zoning. These mutations are committed
   even when the tool's `effects` argument is false; that argument controls visual effects, not a dry run.
   Evidence: demolition gate `0x004C1A44–0x004C1A6B`, dezoning arguments/gate
   `0x004C1A71–0x004C1A98`, zoning arguments/gate `0x004C1AFE–0x004C1B1B`,
   null-lot exit `0x004C1B93–0x004C1B99` (CT23, re-citytools-ct5, 2026-10-23).
7. **The lot:** `CreateLotIfPossible(x0, z0, w, h, rotation, &lot, true, water-compatible, cfg)` (lot manager slot 22).
8. **The building:** the lot developer's `ConstructDefaultLot(lot, cfg, +0x34, cfg slot 20)` (its slot 7,
   lot-developer.md §6.4). On success: the cost is withdrawn (`WithdrawFunds`, budget slot 8) when it is not 0; the
   aura simulator gets a transient effect of type 2 at the building's position (`AddTransientEffect`, aura slot 6;
   `AddPlopAuraEffect`, `0x004BE150`); a building with property `0x88ED3303` (water production) gets a **pipe
   connection occupant** at the lot's centre cell, and a subway station (`IsOccupantSubwayStation`) a **subway
   connection occupant**, each inserted into the occupant manager (underground-occupants.md §6). When the builder
   fails, the cost is **deposited** (`DepositFunds`, budget slot 7) although nothing was withdrawn *[quirk]*; the lot
   stays.
9. **Effects** (when `effects`, also after a failed building): `placement_effects_1x1`, `_2x2`, `_3x3` or `_4x4` for
   the lot's larger side in cells (1, 2, 3, 4 or more) at the lot's position, and the plop ring effect scaled by the
   lot's size, which posts `0x49D5B83F` when it ends (§3.4).
10. The result is whether the building was made.

### 3.13 Toll booths (`PickTollBoothLotAndOrientation`, `0x004BFB30`) *[compat]*

*Completed 2026-10-23 (re-citytools-ct5; CT16).* The cursor position is converted to cell `(x,z)` and
snapped to that cell's corner `(X,Z)`. Height is retained. Before testing the network, output defaults
are configuration `0xEBF9A440`, rotation 2, inclusive cells `(x,z,x,z+1)` and position `(X,Z+8)`.
The network-map lookup uses mask `0x1045`, final argument true. Missing occupant fails. The first present
network type in priority order 0, 2, 6, 12 is selected; none fails. Its flags, masked by `0x1F4F`, must
contain no bit other than the selected type's bit. This is a masked exclusion, not a test of all flags.

The network occupant's slot 38 returns four side bytes, side 0 in the least-significant byte. Sides
0/1/2/3 correspond to −x/−z/+x/+z. In each byte, low two bits 0 mean disconnected, 2 connected;
1 or 3 fail immediately. Exactly two opposite sides must be connected. Bit 2 is a separate alignment
marker, including on a disconnected side; the other bits do not affect this picker.

| Selected type | Configuration |
|---|---|
| 0 (road) | `0xEBF9A440` |
| 2 (highway) | `0x8BF9B356` |
| 6 (avenue) | `0xABF9A45C` |
| 12 (ground highway) | `0x2BF9B222` |

The table gives inclusive cell bounds and final position relative to the snapped corner, including
the initial +8 z shift. These are literal 8/16 world-unit offsets, not multiples of a queried cell size.

| Type | Connected sides | Alignment marker | Rotation | Cells | Position `(x,z)` in world units |
|---|---|---|---|---|---|
| road | 0,2 | ignored | 1 | `(x,z,x+1,z)` | `(X+8,Z)` |
| road | 1,3 | ignored | 2 | `(x,z,x,z+1)` | `(X,Z+8)` |
| 2/6/12 | 0,2 | side 1 bit 2 set | 1 | `(x,z-1,x+1,z)` | `(X+8,Z-8)` |
| 2/6/12 | 0,2 | side 1 bit 2 clear | 1 | `(x,z,x+1,z+1)` | `(X+8,Z+8)` |
| 2/6/12 | 1,3 | side 0 bit 2 set | 2 | `(x-1,z,x,z+1)` | `(X-8,Z+8)` |
| 2/6/12 | 1,3 | side 0 bit 2 clear | 2 | `(x,z,x+1,z+1)` | `(X+8,Z-8)` |

Finally `SeekCompatibleNetworks(rect, 2, type)` must succeed. For every cell in the inclusive rectangle,
it independently follows the network's low four connection bits (occupant slot 40), with the side
offsets above. The starting cell is depth 0; occupants at depths 0, 1 and 2 are checked, but depth 2
does not expand. Every lookup uses mask `0x1045`, true. A missing occupant ends that branch successfully;
there is no reciprocal-connection requirement or visited-cell suppression. An occupant fails the
whole test when its slot 9 reports any flag in `((~(1 << type)) & 0x1FFF) | 0x400000`.
Thus another type among bits 0–12, or flag `0x400000` itself, disqualifies a reached occupant even
when it is the same chosen network type. This is the literal flag rule, regardless of the flag's
name. On such a failure the already selected configuration, rotation, rectangle and position remain
outputs; callers must honor the false result.

Evidence: defaults/snapping `0x004BFB30–0x004BFBC5`; type exclusion and side decoding
`0x004BFBD2–0x004BFC9E`; shape/position writes `0x004BFCBF–0x004BFE93` (float constants
`0x00A8E178` = 8, `0x00A8D45C` = 16); configuration table `0x00A90650`;
search `0x004BF6B0`, direction tables `0x00A90708`/`0x00A90718`.

### 3.14 Feedback *[compat]*

*Completed 2026-10-23 (re-citytools-ct5; CT17).* Visibility and placement validity are separate inputs:
an invalid lot can have a visible preview. Missing terrain pick suppresses feedback (§3.7); invalidity
alone selects the failure appearance below.

#### 3.14.1 Preview occupants and coverage (`GetBuildingPreviewModel`, `0x004C0680`)

A missing lot configuration makes this routine do nothing. With no existing building preview, use
the configuration's first building entry (slot 24), retain its local rotation and position, and
subtract half the unrotated lot's world width/depth from its local x/z. Each dimension is the unsigned
configuration cell count times terrain cell size; multiply by binary32 0.5 before subtraction and
store the result as binary32. Gather building exemplars/family members through the building-development
service for every identifier in the entry. Use the first hash-set entry's profile (not a random draw,
not the lowest ID) and create the preview through the building occupant manager. No building entry,
empty candidates, unavailable profile or failed creation leaves no building preview.

The new occupant gets flag `0x10` and boolean properties `0x029244DB` and `0x29F013E6` true. Initially
hide it through the drawing helper. This is a temporary preview, not a constructed lot building.
Coverage categories are cleared then ORed in police/fire/health/education order: bits 1/2/4/8.
Police and fire use their station predicates; health and education use successful property reads
of `0x091B42A2` and `0x691B42B3` respectively (presence/read success, not a nonzero-value test).
Each matching category supplies the coverage query's radius/auxiliary result and the two effect names
`PlopMode_<category>_Plop` and `PlopMode_<category>_Inactive`, with capitalized category names
`Police`, `Fire`, `Health`, `Education`. A failed radius query sets radius 32. Multiple categories
retain all bits, but the last matching category supplies the final radius and effect names. Update
the view's coverage helper with the categories and recalculate it.

Building group `0x1401` selects registered view `0xA`; otherwise group `0x1302` selects `0xF2`.
Neither group leaves the previous ID unchanged (initially zero). Both groups therefore select `0xA`.
These are the Water data view and the subway-building placement view, respectively, as listed in
map-view.md §2. They request the shipped views' complete display/filter settings, rather than
constructing an ad hoc pipe/subway layer here. Registration and removal are §3.2/§3.3.

For props, consider configuration entries of type 1 only, with flag bit 0 clear and exactly one
exemplar identifier. Require prop-manager availability and successful creation, then boolean
property `0x4A89FCF3` true (missing/false excludes the prop). Accepted props get flag `0x10`, both
preview properties true, and a retained occupant reference with local rotation and position.
Their x/z receive the same half-lot centering adjustment as the building. Family/multiple-ID
and optional entries do not become preview props on this path. Configuration-effect release
releases these previews; repeated feedback updates move the retained occupants.

Evidence: building entry/offsets `0x004C0713–0x004C079E`, candidate/profile/creation
`0x004C07A4–0x004C0827`, flags/properties `0x004C084A–0x004C08B8`, coverage and view
selection `0x004C08BD–0x004C0C05`, prop processing `0x004C0CAB–0x004C0EF3`.

#### 3.14.2 Drawing and colours (`SetVisualUserFeedback`, `0x004C2660`)

When feedback is hidden, hide the local grid, stop any running coverage ring, footprint and direction
arrow, remove registered preview building/props, and stop and clear both constraint-effect arrays.
For the hidden drawing request the attribute flag byte is 2, highlight selector 0, RGB `(1,1,1)`
and alpha 0. A hidden request does not create a model to compensate for a missing building preview.

When visible, put the local grid at the supplied position; show it unless city slot 66's slot 20
reports the suppressing terrain mode. A present coverage ring with positive radius gets an identity
orientation, position at that point and uniform scale `2 × radius`; start it with argument 0 if
stopped. A nonpositive radius does not cause a new ring start in this branch.

Use `building_footprint_ok` for valid placement and `building_footprint_notok` for invalid placement.
Recreate the footprint when validity changes, it is missing, or it has stopped; stop/release the old
one. Bind an available generated lot-footprint texture as effect parameter 6 (one texture). Set its
position to the supplied point, orientation about y to rotation times the binary32 constant
`1.5707950592041016` (`0x3FC90FD0`), and uniform scale to the generated footprint extent `+0x104`
times binary32 `0.9999899864196777` (`0x3F7FFF58`); start with 0 if stopped. The slight inset is
intentional. Texture/effect resources supply the footprint artwork and colours.

The direction effect is `Lot_Direction_Arrow`. Recreate it when validity changes or it is absent;
an existing stopped arrow is reused and restarted. For unrotated lot depth `D` and terrain cell size
`s`, its inset distance is `d = trunc(D × s × 0.5 − 6)` using x87 operations and `_ftol2`.
Place it at point plus `(0,0,−d)`, `(d,0,0)`, `(0,0,d)`, `(-d,0,0)` for rotations 0–3.
Its y-axis orientation is rotation times the **binary64** constant −1.570795 (`0xBFF921F9F01B866E`),
with scale 1. The footprint uses a positive binary32 angle; the arrow uses a negative binary64
angle. Start with 0 if stopped. The tool does not recolour the arrow by validity.

For a present building preview, pass flag byte 3, highlight selector **3 valid / 4 invalid**, RGB
`(1,1,1)`, alpha binary32 0.4 (`0x3ECCCCCD`) to the building and accepted props, and show them.
These are highlight selectors, not replacement RGB values: the renderer's default selector 3 is
blue-grey `(0.2,0.2,0.4,0.6)` and selector 4 dark red `(0.66,0,0,0.7)` (draw-context.md §1.1/§6).
Renderer property overrides of that palette remain effective. Do not turn the valid model green
by analogy with other tools. Every contained model instance receives draw mode 5 and the supplied
20-byte attribute record; its uninterpreted bytes are not additional deterministic tool state
(model-instances.md §1). Then update constraints, irrespective of validity.

For both building and props, let centered local offset be `(a,b)` in x/z. Relative to the supplied
point the world x/z offsets for rotations 0–3 are `(-a,-b)`, `(b,-a)`, `(a,b)`, `(-b,a)`.
World y is the supplied point's y: the saved local y is not added. Occupant orientation is
`(local rotation + tool rotation) mod 4` for the normal 0–3 rotations. Showing an unregistered
occupant sets position/orientation and inserts it; showing a registered one updates it and, on
an orientation change, sends `0x99EF1145` with argument 0. Hiding removes a registered occupant.
The same transform and drawing record apply to every accepted prop, not only the main building.

A validity transition selects cursor `0x417EE36B` (valid) or `0x88E70B76` (invalid), including on
the hidden branch. Missing optional visual effects are skipped; no replacement colour effect is made.
Evidence: hide path `0x004C2D4F–0x004C2EDF`, ring/grid `0x004C2675–0x004C27CE`, footprint
`0x004C27CE–0x004C2A1A`, arrow `0x004C2A1A–0x004C2CEC`, attributes `0x004C2CEC–0x004C2D45`,
cursor `0x004C2EDF–0x004C2F0D`; occupant transforms/draw calls `0x004BE180–0x004BE5AA`;
default highlight palette `0x007D63C6–0x007D63FC` and palette selection `0x007D5670`.

#### 3.14.3 Constraint tiles (`SetConstraintFeedback`, `0x004C2090`)

Type 5 configuration entries (water constraints) use `selection_blue`; type 6 (land constraints)
use `selection_red`. These colours describe the entry type, **not** valid/invalid placement. Process
entries in configuration order, maintaining a separate effect array for each colour. A missing
configuration returns without updates. When §3.9 cannot obtain a lot rectangle, no entries are
used and the existing surplus effects are stopped.

For each entry convert local minimum and maximum x/z, each with 8 world units added, through terrain
`PositionToCell`. Subtract 1 from both maximum cell coordinates to obtain inclusive cell bounds
`(u0,v0,u1,v1)`. With unrotated configuration width `W` and depth `D`, rotate them as follows:

| Tool rotation | Rotated inclusive bounds |
|---|---|
| 0 | `(W-1-u1, D-1-v1, W-1-u0, D-1-v0)` |
| 1 | `(v0, W-1-u1, v1, W-1-u0)` |
| 2 | `(u0,v0,u1,v1)` |
| 3 | `(D-1-v1,u0,D-1-v0,u1)` |

Translate by the lot rectangle's minimum cell from §3.9. Set effect parameter `0xC` to four
binary32 **cell coordinates** `(x0,z0,x1+1,z1+1)` (count 4), then start with argument 0.
Reuse the next effect of that colour, creating the named effect only when the array needs another
entry; stop unused trailing effects after the pass. A later pass may reuse those stopped effects.
Do not multiply the parameter by cell size or collapse two adjacent constraint entries into one.
The effect resources supply actual blue/red shades; this function supplies no numeric tint.
Evidence: conversion/rotation/translation `0x004C2160–0x004C227F` and `0x004C237C–0x004C248E`,
effect updates/cleanup `0x004C2280–0x004C2655`; integer rotation helper `0x004BDFD0–0x004BE083`
with dispatch table `0x004BE084`.

## 4. Quirks

1. `IsOnTop` and the place-lot handlers use the 3D view pointer without a null test (§2.6).
2. `EndInput` removes the stack top, whichever control it is (§2.7).
3. The place-lot tool ends itself after 1000 successful plops in one activation (§3.6) *[compat]*.
4. The world rectangle of §3.6 is computed in x87 precision as `x*size - 2^32` (min corners) and
   `(x+1)*size + 2^32` (max corners) and truncated to int64 (`_ftol2`, `0x009EEF04`); only the low dword is
   stored, so the 2^32 bias cancels and the stored values equal `trunc(x*size)` mod 2^32 *[compat]*.
5. On the automatic-facing path, when there is no lot configuration or automatic facing fails, the gate for the
   `CanPlaceLot` call is the **previous** placeable value (a stack slot reused from step 1) *[quirk, inferred]*.

6. Query tool: sliders apply on every move; the close-time write-back (result 0 only) applies them again
   (§8.12.2, §8.13) (re-query).
7. Query tool: the hospital staff slider refreshes four related IDs, including two adjacent-data words that are
   not ordinary query control IDs (§8.16.1) (re-query-ct5).
8. Query tool: a network occupant both tries the neighbour dialog and continues to the query dialog (§8.12.1).
9. Place-lot: after a successful click no lot is placeable while the lot's centre stays inside the placed lot's
   world rectangle (§3.6, §3.10) (re-plop).
10. Place-lot: the automatic facing keeps the player's rotation at exactly four fewer street points;
    an improvement greater than four changes it, subject to §3.11's zero-frontage exception
    (corrected CT24, re-citytools-ct5).
11. Place-lot: a building that fails after its lot was made credits the city with the lot's cost (§3.12)
    *[quirk]* (re-plop).

## 5. Open questions

See docs/specs/questions/city-tools-input.md for remaining query-tool questions. Place-lot questions
CT2, CT16, CT17 and CT22–CT24 are answered in the corresponding answered file (2026-10-23).

## 6. Conformance tests

1. Base: `Init` twice holds the 3D view once; `Shutdown` releases window manager, 3D view, cursor in that order and
   keeps the cursor ID; `SetCursor(unknownId)` changes nothing.
2. `QueryInterface(0xC6BB8345)` and `(1)` succeed with AddRef; any other IID fails.
3. Place-lot: `Init` sets ID `0x88F154FB` and cursor `0x88E70B76`, and subscribes the six messages in the §3.2
   order; `Shutdown` unsubscribes the same six.
4. Rotation keys: four presses of `0x6A935CD5` return the rotation to its start; `0x6A935CD1` then `0x6A935CD5`
   is the identity.
5. A failed left click plays `(0xEA451903, 0xAA62C9D5, 0, 0, 0)`; a successful one does not.
6. Query tool (§8): mode 0 `Init` sets ID `0xC7AF928E` and cursor `0x816D7F74`, mode 1 `0xC7AF928F` and
   `0xCB8D5B54`; both subscribe the six messages of §8.4 in order and `Shutdown` unsubscribes them; only mode 1
   holds `+0x48`/`+0x4C`.
7. Query tool hover filter: mode 0 accepts a power pole and rejects a prop without property `0x8A416A99`; mode 1
   rejects a power line (network type 5) and a pipe (type 4) and accepts a road (type 0).
8. Query tool: a mouse move that changes the hovered occupant restores the old occupant's saved highlight state
   and sets state 7 on the new one; `OnMouseExit` leaves no occupant highlighted; message `0x26D31EC2` stops tip
   text until the next mouse move restarts the timer.
9. Query tool tips (§8.11): `Ratio(90, 100)` = 0.5, `Ratio(80, 100)` = 1.0, `Ratio(5, 0)` = 0.0; a terrain tip
   on zone 0 has title `0xAA5BBDE4` and an empty body; a network tip on a road (type 0, not 2/8/12) has title
   `0x6A5589D4`; a tip with icon 0 goes to the cursor text (3D view slot 40) and hides the callout; `SetTip` with
   two empty strings hides the callout and clears the cursor text.
10. Query click (§8.12): a click on a building whose exemplar has property `0x2A499F85` = V opens the UI script
    {0, `0x96A006B0`, V} modally and plays the occupant query sound first; a click with no occupant on an airport
    (zone 11), seaport (12) or landfill (14) cell opens the zone dialog, and on any other empty cell plays UI
    sound `0xAA62C9D5` and opens nothing; the read-outs afterwards are `Query`, `Query extra: none`,
    `Query extra2: none` before being overwritten, and on a building cell `extra2` = `[Building <name>] [Lot:
    <cfg>_<id hex8>]\n`. In the dialog, a child with ID `0x1001` is filled from the fire simulator's station
    efficiency; a child with ID `0x1005` (null slot) is not filled and its own children are walked; a child
    without bit `0x1000` is skipped and walked. The coverage rings switch to the occupant's categories while the
    dialog is open and the previous categories are restored on close; cancelling (result non-zero) applies no
    set pass and `DoQueryDialog` returns false.

Query tool (re-query, 2026-10-21):

- `L(80, 100)` = 1.0; `L(90, 100)` = 0.5; `L(120, 100)` = 0; `L(5, 0)` = 0 (§8.12.4).
- A mayor rating percent of 33.2 shows low, 33.3 medium, 66.6 high (§8.12.4).
- Moving a fire station's funding slider to 80 calls `SetStationFunding(o, 0.8)` before the dialog closes, and the
  cost text shows `trunc(80 × 0.01 × c)` (§8.13).
- A Shift-click that finds no automaton still runs `DescribePick` (§8.6).

Place-lot tool (re-plop, 2026-10-22):

- Odd size: a 3 × 3 lot at point (40, 40) with 16 m cells covers cells 1–3 on both axes, centre (40, 40) (§3.9).
- Even size: a 2 × 2 lot at (40, 40) snaps to corner 3 (48): cells 2–3, centre (48, 48) (§3.9).
- Frontage: an avenue cell and two road cells in front give a score of 8 (§3.11).
- Keep facing: player score 3 with best 6 keeps the player's rotation; best 8 changes it; player score 0 with
  best 1 changes it (§3.11).
- A plop on cells (2, 2)–(3, 3) with 16 m cells stores (32, 32, 64, 64); a placement centred at (48, 48) then
  fails (§3.6, §3.10).
- A builder that fails on a lot costing 500 deposits 500 and withdraws nothing (§3.12).

Place-lot closure vectors (re-citytools-ct5, 2026-10-23):

| ID | Setup/action | Required observation |
|---|---|---|
| CT2-a | Move with a held WM, no terrain hit | One WM input-queue flush after feedback/text processing; handled = true. |
| CT16-a | Road at cell (10,20), 16-unit corner (160,320), side word `0x00020002`; search clean | Config `EBF9A440`, rotation 1, cells (10,20,11,20), position (168,320). |
| CT16-b | Avenue at that cell, side word `0x00020402`; search clean | Config `ABF9A45C`, rotation 1, cells (10,19,11,20), position (168,312); side 1 marker shifts the footprint. |
| CT16-c | Ground highway at that cell, side word `0x02000200`; search clean | Config `2BF9B222`, rotation 2, cells (10,20,11,21), position (168,312). With side 0 byte 4, cells (9,20,10,21), position (152,328). |
| CT16-d | Side low bits contain 1/3, adjacent connections, or three connections | Picker false. A road plus avenue in masked flags also fails. |
| CT16-e | A reached depth-2 occupant has flag `400000`; then put the flag only at depth 3 | First search false; second true if every checked occupant is otherwise compatible. Missing branch occupants pass; do not inspect depth 3. |
| CT17-a | Show valid, then invalid, at the same point with a preview | All building/prop model instances get mode 5, flag 3, white RGB, alpha 0.4; highlight selectors 3 then 4. Footprint changes ok → notok; cursor changes to `88E70B76`. Constraint colours retain entry types. |
| CT17-b | Hide feedback with ring/footprint/arrow running, registered preview and two constraint arrays | Grid hidden, three running effects stopped, preview occupants removed, constraint effects stopped and arrays cleared. Validity is independent of this visibility input. |
| CT17-c | Building local centered offset (3,5), local rotation 1, local y 9; point (100,7,200); tool rotations 0/1/2/3 | World positions (97,7,195), (105,7,197), (103,7,205), (95,7,203); orientations 1/2/3/0. Same rule for accepted props. |
| CT17-d | W=4, D=3; a constraint converts to inclusive local cells (0,0,1,0); translated lot minimum (10,20); rotations 0/1/2/3 | Parameter `0xC` respectively (12,22,14,23), (10,22,11,24), (10,20,12,21), (12,20,13,22), in cell units. Type 5 blue, type 6 red at every rotation. |
| CT17-e | Preview matches police and education; final radius query fails | Categories 9, radius 32, Education effect names. A present zero-valued education property still matches if its read succeeds. |
| CT17-f | Building belongs to water and subway groups; then a subway-only building | Registered view IDs `A` then `F2` (Water and subway-building placement); neither uses subway-network tool view `F1`. |
| CT17-g | Prop entries: optional singleton; nonoptional two-ID; nonoptional singleton whose preview property is false; nonoptional available singleton with property true | Only the last is retained as a preview prop. Both preview properties are set true on that occupant. |
| CT17-h | D=3, s=16, point (100,7,200), rotations 0–3 | Arrow distance 18; positions (100,7,182), (118,7,200), (100,7,218), (82,7,200). A stopped existing arrow restarts; a stopped footprint is recreated. |
| CT2-b | Repeat an unchanged move; then move with no held WM | One flush on the repeat; no WM call with null; both handled. |
| CT22-a | Ring-finished message, flag set, view present | Flag clears before one coverage recalculation; message result false. |
| CT22-b | Repeat ring-finished with flag clear; then with null view | Recalculates again with view; only flag clear with null. No ring release or plop decrement. |
| CT23-a | Nonzero cost, effects false; services succeed | Dezoning flags `(commit,skipFunds,checkSlope,skipSize,checkLotInside)` = `(true,false,true,true,false)`; zoning = `(true,true,true,true,false)`; all three output pointers null. |
| CT23-b | Zero cost; services succeed | Both zoning calls use `(true,true,true,true,false)`. |
| CT23-c | Demolition false / dezoning false / zoning false, separately | Respectively no zoning calls / no second zoning or lot creation / no lot creation. No rollback call, construction, refund or placement effects. |
| CT24-a | Own frontage 3; best 6, 7, then 8 | Keep, keep, change, respectively. Own 0 / best 1 changes despite the small gap. |

## 7. Coverage

`cSC4ViewInputControl` (all 17 functions) is specified by §2. `cSC4ViewInputControlPlaceLot` is specified for its
lifecycle and input dispatch (§3.1–§3.8) and for its cells, placement test, automatic facing and placing
(§3.9–§3.12; re-plop 2026-10-22), so it is claimed as specified, with `cSC4PlaceLotDemolishFilter`. The toll
booth picker/search (§3.13), preview occupants, coverage, drawing and constraint feedback (§3.14)
are now behaviour-level contracts with CT16/CT17 vectors (re-citytools-ct5, 2026-10-23).

`cSC4ViewInputControlQuery` is specified for its creation, lifecycle, input dispatch, message dispatch and pick
filters (§8.1–§8.9), its tips (§8.11), the query click and every dialog filler (§8.12), the other dialogs (§8.13),
the building values (§8.14) and route mode in outline (§8.15; re-query 2026-10-21), so it is claimed as specified,
with `cSC4QueryDialogWinProc`. The neighbour dialog and the debug description writers stay at summary level
(CT14).

## 8. The query tool `cSC4ViewInputControlQuery`

Added 2026-10-21 (re-querytool4 lane). Byte counts and the function inventory are from
`python tools/coverage.py --owners` at main `fd9beb23`: the owner has 215 functions, 76,564 bytes.

### 8.1 Creation *[compat]*

The 3D view's `ProcessCommand` (`0x007F1C10`) creates the tool for two commands; it first asks the current
control (3D view slot 24) for its ID and does nothing when that tool is already active:

| Command | Mode `+0x8C` | Skipped when the current ID is | Control ID | Cursor ID |
|---|---|---|---|---|
| `0x6A935CF3` | 0 (query) | `0xC7AF928E` | `0xC7AF928E` | `0x816D7F74` |
| `0x6A935CF4` | 1 (route query) | `0xC7AF928F` | `0xC7AF928F` | `0xCB8D5B54` |

The object is 0xA0 bytes; the constructor (`0x004C4590`) takes (mode, tip delay) and both call sites pass a tip
delay of 50 (`0x32`). The 3D view then pushes it with `SetCurrentViewInputControl(control, 0)` (slot 25). Mode 1
is the route-query tool: it alone shows the legend dialog, traces paths and uses the route filter (§8.8)
*[inferred name]*.

### 8.2 Layout *[compat]*

Base fields `+0x00..+0x20` as §2.2.

| Offset | Type | Constructor value | Meaning |
|---|---|---|---|
| `+0x28` | vtable | `0x00A90A78` | message-target sub-object (its `DoMessage` is §8.7; `this` is the object + 0x28) |
| `+0x2C` | cISC4Occupant* | null | the highlighted (hovered) occupant, held |
| `+0x40` | uint32 | 0 | that occupant's highlight state before it was highlighted (restored on un-highlight) |
| `+0x44` | cSC4WinCalloutBox* | null | the hover tip window, held |
| `+0x48`, `+0x4C` | cIGZWin* | null | mode 1: the legend dialog's full and minimised panes, held |
| `+0x50` | cRZCallbackTimer | constructed | the hover-tip timer (0x28 bytes) |
| `+0x78` | bool | true | tip allowed (cleared by message `0x26D31EC2`, set again by `Init` and each timer start) |
| `+0x7C`, `+0x80`, `+0x84` | float x3 | the global default vector `0x00B21858` (all three from the same dword) | the last terrain point under the cursor |
| `+0x88` | bool | false | something queriable is under the cursor |
| `+0x89` | bool | false | a click is in progress (suppresses the tip timer restart) |
| `+0x8C` | uint32 | mode | §8.1 |
| `+0x90` | uint32 | tip delay | the timer period passed to the timer start (§8.5) |
| `+0x94` | bool | false | the 3D view's `GetTerrainQueryEnabled()` (slot 39) at `Init` |
| `+0x98` | uint32 | 3 | the **route-step filter** (§8.15): which travel-type steps of a route count when the route is tested against a rectangle. 1: every step except subway steps (travel type 6); 2: only subway steps; 3 (the constructor's value, and any other): every step. Written by the building and network path queries and by the building traffic-info builder, read by `DoesRouteCrossCell` (CT6) |
| `+0x9C` | cSC4PathView* | null | mode 1: the path view, held |

### 8.3 Slot table (vtable `0x00AA6658`-shaped, at `0x00A90A88`) *[compat]*

Same 30 slots as §2.3 with these overrides; every other slot is the base body (§2.3), including the
`QueryInterface` shared with the place-lot tool (`0x004BDF10`).

| Slot | Method | Body |
|---|---|---|
| 3 | Init | `0x004C57A0` (§8.4) |
| 4 | Shutdown | `0x004CBA80` (§8.4) |
| 16 | OnMouseDownL | `0x004D4D70` (§8.6) |
| 18 | OnMouseUpL | `0x004C4C50` (§8.6) |
| 20 | OnMouseMove | `0x004CBC70` (§8.5) |
| 22 | OnMouseExit | `0x004CC0B0` (§8.5) |
| 26 | deleting destructor | `0x004C5730`: the destructor body `0x004C5750` (release `+0x9C`, destroy the timer `+0x50`, release `+0x4C`, `+0x48`, `+0x44`, then the base destructor), free when `flags & 1` |

Key, character, right-button and wheel input are not handled (base stubs return false).

### 8.4 Init (`0x004C57A0`) and Shutdown (`0x004CBA80`) *[compat]*

`Init`, only when not initialised:

1. Set the control and cursor IDs for the mode (§8.1), then the base `Init` (§2.4).
2. Subscribe `+0x28` on the message server (`0x00B43CCC`, slot 5), in order, to `0x0A456D95` (tip timer fired),
   `0x0A456D96` (coverage ring refresh), `0x26D31EC2` (pre-city-shutdown), `0xA6B79602` (view change),
   `0x66956814` (sim new day), `0x69247DC7`.
3. Set the timer callback to `MouseTipCallback` (`0x004C4E70`) with this object as its context (timer slot 9);
   set `+0x78`; store the 3D view's `GetTerrainQueryEnabled()` in `+0x94`.
4. Create the callout box (0x1B4 bytes, `cSC4WinCalloutBox`), hold it in `+0x44`; set its two style IDs
   `0xA85F1A83` (slot 11) and `0x0A52F5C6` (slot 10), its rectangle (-40, -40, 40, 40) (slot 17), hide its window
   (window slot 70) and add that window as a child of the 3D view's window (window slot 14).
5. Mode 1 only: build the legend dialog from a UI script (`SC4CreateWindowFromScript`, `0x005F9480`; the key
   words `0x96A006B0` / `0xABB0120F`, child ID `0x6BB92BCB`) under the 3D view's window, with a
   `cSC4LegendDialogWinProc` (vtable `0x00A90980`, `+0x08` = this tool); look up its child windows `0x0BB0F5E7`
   (into `+0x48`) and `0x6BB92BCA` (into `+0x4C`) as interface `0x22BA0121` (window slot 36), attach the window
   proc to each, add each to the 3D view's window and position it from the 3D view's size (window slots 41, 44,
   45, 56). Exact steps (answers CT7):
   1. The script key is `{type 0, group 0x96A006B0, instance 0xABB0120F}`; the parent is the 3D view's window; the
      child ID `0x6BB92BCB` names the dialog shell `D`, taken as interface `0x22BA0121`. A failed creation ends
      the step (the tool then has no legend).
   2. The window proc is a new 12-byte object (vtable `0x00A90980`, `+4` zero, `+8` this tool).
   3. The full pane `F` (`+0x48`, child `0x0BB0F5E7`) and the small pane `M` (`+0x4C`, child `0x6BB92BCA`) are looked
      up in `D` as `0x22BA0121`. An existing `+0x48`/`+0x4C` is released first.
   4. `F`: removed from `D` (window slot 15), added to the 3D view's window, then moved to
      `(viewWidth − paneWidth − 2, 2)`, its window proc set (after a QI for `0x5386D516`, that interface's slot 7),
      pulled to the front (slot 23) and **shown**. That puts the full legend in the view's top right corner,
      2 pixels from the top and right edges.
   5. `M`: removed from `D`, added to the 3D view's window, then moved to `(F.right − M.width, F.top)`, i.e. the
      same top edge, with its right edge on `F`'s right edge; window proc set, pulled to the front, **hidden**.
   6. `D` itself is removed from the 3D view's window (window slot 16); the proc and the shell are released.
   `SetLegendMinimized(on)` (`0x004C4630`, pops 4): with either pane missing nothing happens. `on` true hides `F`,
   shows `M` and re-places `M` at `(F.right − M.width, F.top)` with the same arithmetic as step 5; `on` false shows
   `F` and hides `M` (no move). The window proc (`DoWinProcMessage`, `0x004CB900`) calls it with true for the button
   `0xEBB91356`; the button `0x4BB92C1F` restores directly (shows `F`, hides `M`).
6. Return true.

`Shutdown`, only when initialised: clear the cursor text with key `0x2A456CB1` (3D view slot 41); remove the
callout window from the 3D view's window (slot 16) and release `+0x44`; for each of `+0x48`, `+0x4C`: detach the
window proc (window slot 7 with null, after a QI for `0x5386D516`), remove it from the 3D view's window and release
it; if a path view is held, unregister it from the renderer (`0x00B43DD0` slot 33), shut it down and release it;
if an occupant is highlighted, restore its state (`+0x40`, occupant slot 17), `SetHighlightState(0)`, release it
and clear `+0x40`; unsubscribe the six messages of step 2; stop the timer if it is running (and clear `+0x78`);
then the base `Shutdown`. Returns true.

### 8.5 Hover: OnMouseMove (`0x004CBC70`) and OnMouseExit (`0x004CC0B0`) *[compat]*

`OnMouseMove(x, z, modifiers)` always returns true. Steps:

1. Clear `+0x88`. No 3D view: return.
2. Build the mode's hover filter (§8.8) and pick an occupant at (x, z) through it (`PickQueriableOccupant`,
   §8.8). On a hit set `+0x88`; when the occupant's boolean property `0x6A95E503` is true and the lot manager
   (`0x00B43D08` slot 14) finds its lot and the lot has a building (lot slot 48), the building replaces the hit.
3. No hit: pick the terrain (3D view slot 36) into a local point preset to the default vector. If it hits, turn
   it into a cell (city `0x00B43CEC` slot 95 `PositionToCell`) and get the lot there (lot manager slot 6); if
   there is a lot with a building, mode 1 takes the building when the route filter accepts it (and sets `+0x88`),
   mode 0 takes it when it is in occupant group `0x1006` (park). In mode 0 `+0x88` is set whenever the terrain
   pick hit. Then, when any coordinate differs from `+0x7C/+0x80/+0x84` by 0.0001 or more (float `0x00A9097C`),
   store the point; with no occupant found, the tip is marked stale.
   With a hit in step 2 and `+0x94` set, the terrain point is still picked straight into `+0x7C`.
4. When the occupant differs from the highlighted one `+0x2C`: un-highlight the old one (§8.4 Shutdown wording)
   and release it; store the new one; if non-null, save its state (occupant slot 16) in `+0x40`, set state 7
   (slot 17, then `SetHighlightState(7)`) and hold it. A change, a stale tip or no occupant clears the cursor text
   (key `0x2A456CB1`) and hides the callout.
5. Unless a click is in progress (`+0x89`): stop the timer, set `+0x78`, start it with period `+0x90` and this
   object as context (timer slot 6).
6. If the callout window is visible (window slot 71), move the callout to (x, z) (callout slot 15).

`OnMouseExit()`: stop the timer, clear the cursor text (`0x2A456CB1`), hide the callout, un-highlight and release
the occupant (as step 4), clear `+0x88`; returns true.

### 8.6 Clicks: OnMouseDownL (`0x004D4D70`) and OnMouseUpL (`0x004C4C50`) *[compat]*

`OnMouseDownL(x, z, modifiers)` always returns true:

1. Stop the timer, clear the cursor text (`0x2A456CB1`), hide the callout.
2. Modifier bit `0x01` set (Q CT8):
   - pick at (x, z) with the automaton filter (vtable `0x00A90A18`, `0x004C4D10`; interface `0xA9B40F05`); a hit
     goes to the object at city slot 43 (`GetVehicleManager`), its slot 34, `SetCameraFollow(automaton)`
     (`0x0050A930`); a miss posts message `0x2ACD2CF3` with data (x, z) (message server slot 3);
   - then pick a stoplight prop (§8.8); a hit goes to city slot 40 (`GetTrafficNetwork`), its slot 37,
     `SwitchStoplightStates(prop)` (`0x004B0A00`). Both are **Shift** clicks (modifier `0x01`) and both steps run in
     one click (answers CT9, §8.6.1).
3. Modifier bit `0x02` set: pick the terrain at (x, z) (result not tested *[quirk]*), turn the point into a cell
   and call the residential simulator (`0x00B43DA4`) slot 59 with (cellX / 4, cellZ / 4), C division rounding
   toward zero (Q CT10).
4. Set `+0x89`. Mode 1: `GetOccupantPaths` (`0x004CAC50`) and return; `+0x89` stays set until `OnMouseUpL`.
5. Mode 0: clear `+0x89`; unless step 2 found an automaton (the pointer it fills stays null otherwise,
   `0x004D4F43`; corrected 2026-10-21, re-query), `DescribePick(x, z, <three static strings
   0x00B2185C, 0x00B21870, 0x00B21884>, IsDebugQueryEnabled())` (`0x004D4140`, the query dialog; §8.10);
   then `PlayResultSound(0)` (slot 28, which does nothing, §2.8).

`OnMouseUpL(x, z, modifiers)`: stop the timer, clear the cursor text, hide the callout, clear `+0x89`; returns
true.

#### 8.6.1 The two Shift-click targets (CT9)

*Added 2026-10-27 (re-q-city).* Both calls are made while Shift is held (modifier `0x01`) on a press; they are
independent (a click can reach both), and neither return value is used.

- **`SetCameraFollow(automaton)`** (vehicle manager slot 34, `0x0050A930`, pops 4). The followed automaton is one
  process-wide reference. The same automaton: nothing. Otherwise the old one is released and the new one stored
  with a reference; a null argument stops there (it ends following). With a non-null automaton and a renderer:
  the camera control (renderer slot 16) is taken, an "updating" flag is set, and when the render property
  `kDoAutoZoomAndRotationChanges` (render-properties.md, default true) is on and the camera's zoom step is below 3 it
  is zoomed to 4.0 (`SetZoom`). The camera's view target is then set to the automaton's position (its occupant's
  position), the follow-smoothing state is cleared to the same start, the manager's `UpdateCameraFollow` (slot 64,
  automata-managers.md) runs once with 100.0, and the updating flag is cleared. So Shift-clicking a vehicle or pedestrian makes the
  camera track it from then on *[compat]*; the continued tracking is `UpdateCameraFollow` and the zoom-dependent
  activation of automata managers (automata-managers.md slot 60).
- **`SwitchStoplightStates(prop)`** (traffic network map slot 37, `0x004B0A00`, pops 4). A null prop does nothing.
  The prop's position becomes a map cell (each of x and z multiplied by 1/16 and truncated), and every network tile
  stacked at that cell (the tile chain, next link at tile `+0x54`) whose ID has a record in the map's intersection
  table is processed. A record holds four approaches of 20 bytes: state (dword), light flag (byte `+4`, set =
  green phase), warning flag (byte `+5`), on-time and off-time (floats `+8`, `+0x0C`) and the remaining time
  (float `+0x10`) (the layout `TickSingleIntersection`, `0x004AFB80`, counts down). For each approach: in state 2
  (timed signal) the light flag **flips** and the remaining time is reloaded from the on-time when the flag is now
  set, else from the off-time (the warning flag is left alone); in state 3 the flag is cleared and the remaining
  time set to 5.0; other states are untouched. The intersection's stoplight props are refreshed after the loop only if the last approach that changed left a
  "changed" marker set: a state-3 approach always sets it, a state-2 flip sets it only when the flag ends up set, so a
  state-2 flip to cleared as the last change (and no changes at all) skips the refresh *[compat]* (`UpdateIntersectionProps`, `0x004AE850`, with the record and a false flag). It therefore advances the
  signal phase of the clicked intersection at once *[compat]*; how the props show a state is traffic work outside
  this spec.

### 8.7 Messages (`DoMessage`, `0x004D7160`, on the `+0x28` sub-object) *[compat]*

Always returns false.

| Message | Action |
|---|---|
| `0x0A456D95` (posted by the timer, §8.9) | when the tip is allowed (`+0x78`), build the tip text (below) |
| `0x0A456D96` | recalculate the 3D view's coverage helper (slot 49, `RecalcCoverage`) |
| `0x26D31EC2` | clear `+0x78` |
| `0x66956814` | every tenth day *[inferred]* (`0x00B43D5C` slot 7, then its slot 26, mod 10 == 0), when the debug info window `0x00B2184C` and its flag `0x00B21848` are set, refresh its two strings `0x00B21840`, `0x00B21844` (window slot 32) |
| `0xA6B79602` | read the cursor position (window manager slot 33) and run `OnMouseMove` there |
| `0x69247DC7` | with a path view held: clear its paths (`UpdatePaths`, `0x007DDC30`) and `ShowAllRoutesThatIntersectCityRect` (§8.10) |

Tip text (`0x0A456D95`), two strings (title, body):

- An occupant is highlighted: mode 1 asks `GetBuildingOccupantTrafficInfo` (buildings `0x278128A0`) or
  `GetNetworkOccupantTrafficInfo` (networks `0xC772BF98`); mode 0 dispatches on the type: flora `0x74758926` and
  props `0xA823821E` give the exemplar's display name (§8.8 name rule), buildings `GetBuildingOccupantTipInfo`
  (which may also return an icon), networks `GetNetworkOccupantSummaryInfo`, power poles `0x2890D4DE` and
  landfill `0x6A11BD2B` a fixed localised title (group `0xEA5524EB`, instances `0x0A5BB9FD` and `0x4A5BBA21`).
  Before the dispatch the locals are: title and body empty, icon 0, secondary icon 0, fraction 0.5 (`0x3F000000`).
  Mode 1's building call also receives the icon; the network traffic call does not.
- No occupant: when the terrain query is enabled (`+0x94`) `GetTerrainSummaryInfo` fills it.
- With the terrain query enabled (`+0x94`, whatever was found): an empty title becomes the literal `Terrain`; the
  line `x=%.1f y=%.1f z=%.1f\ncell x=%d cell z=%d` (from `+0x7C..+0x84` and the cell) is appended to the body,
  after a `\n` when the body is non-empty.
- Display: §8.11.3 (the choice between the callout and the cursor text is the icon, not the text).

### 8.8 Pick filters and picks *[compat]*

All picks go through the renderer (`0x00B43DD0`) slot 65 (x, z, filter, &hit); the hit object's slot 3 gives the
occupant, which is QI'd for the wanted interface (the result is held by the caller).

| Filter (vtable) | `IsOccupantIncluded` | Accepts |
|---|---|---|
| hover, mode 0 (`0x00A909B0`) | `0x004C37B0` | buildings, power poles, landfill, network occupants; flora and props only when their exemplar has property `0x8A416A99` |
| route, mode 1 (`0x00A909E4`) | `0x004C39A0` | buildings; network occupants (interface `0xA821EF94`) of any of the network types 0, 1, 2, 3, 6, 7, 8, 9, 10, 11, 12 (not 4 or 5) |
| stoplight (`0x00A90A4C`) | `0x004C3AD0` | props in occupant group `0x5002` or `0x5003` |
| automaton (`0x00A90A18`) | `0x004BDF90` | (not in this owner; answered in §8.6.1) |

`PickQueriableOccupant(x, z, &out, filter)` (`0x004C4C90`) wants `0x26BB767E` (cISC4Occupant);
`PickStoplightProp(x, z, &out)` (`0x004C4DC0`) wants `0xE9793A65`; both return true on a hit, false (and null)
otherwise or without a renderer.

Display name of a flora/prop (`0x004C3810`): the exemplar's string property `0x899AFBAD`, else the localised
string named by property `0x8A416A99`, else property `0x20` (the exemplar name) *[inferred order]*.

### 8.9 Timer callbacks *[compat]*

- `MouseTipCallback(context)` (`0x004C4E70`): with a message server, send a `cRZMessage2Standard` of type
  `0x0A456D95` to the context's `+0x28` target (server slot 7) and release it.
- `CoverageRingCallback` (`0x004C4EE0`): with a message server, post a `cRZMessage2Standard` of type
  `0x0A456D96` to all (server slot 4) and release it.

### 8.10 Inventory

Covered by §8 (19 functions, 5,907 bytes): `004C4590` constructor, `004C5730` / `004C5750` destructors, `004C57A0`
Init, `004CBA80` Shutdown, `004D4D70` OnMouseDownL, `004C4C50` OnMouseUpL, `004CBC70` OnMouseMove, `004CC0B0`
OnMouseExit, `004D7160` DoMessage (dispatch; display tail §8.11.3), `004C4C90` PickQueriableOccupant,
`004C4DC0` PickStoplightProp, `004C4D10` automaton pick, `004C4E70` MouseTipCallback, `004C4EE0`
CoverageRingCallback, `004C39A0` / `004C3AD0` filter predicates, `004C3810` display name, `007DDC30` UpdatePaths.
`004C37B0` (mode-0 predicate) is described but belongs to another owner.

Added by §8.11 (round 2; 5 functions, 5,818 bytes): `004C5BA0` SetTip (229), `004CACC0` SetHighlightState (276),
`004CF8A0` GetNetworkOccupantSummaryInfo (651), `004D6C70` GetTerrainSummaryInfo (1,197), `004D5E30`
GetBuildingOccupantTipInfo (3,465); plus the DoMessage display tail (§8.11.3). Described there but not in this
owner's rows: `004C3B10` Ratio, `004C3730` float greater-than, `004D5C90` lot headlines. Total covered: 24
functions, 11,725 bytes.

Added by §8.12 (round 3; 8 functions, 4,321 bytes): `004D4140` DescribePick (3,118), `004D3C30` DoQueryDialog
(783), the walks `004D3AE0` (90), `004D3BD0` (90), `004D3A90` (71), `004C4340` (47), and the sample fillers
`004C5180` (60), `004C67F0` (62). Total covered: 32 functions, 16,046 bytes. §8.12.3 also gives the exact
control-ID dispatch of all 103 filler slots (93 distinct fillers, 89 in this owner, 12,323 bytes); their bodies,
beyond the two samples, are not specified.

Remainder after round 3 (183 functions, 60,518 bytes; the lane parked, Q CT12): the other dialogs
(`DoZoneBasedQueryDialog` `0x004CCCB0`, `DoNeighborConnectionQueryDialog` `0x004CC6E0`), the refresh walk
`0x004D3B40`, `EnumFuncClearFilter` `0x004C4370`, `PlayOccupantQuerySound` `0x004C4F40`, the 87 other filler
bodies of §8.12.3 and their typed workers (`SetOrGetAirport*` `0x004C5280` and kin), the
description writers (`WriteDetailedDebugInfo` `0x004D12A0`, `DescribeCell`, `DescribeBuilding`,
`DescribePopulation`, `DescribeDemand`, `WriteBuildInfo`, `WriteDesirabilityEquation`, `WriteOutcomeStats`,
`GetLotHeadlines`), the traffic-info pair (`0x004D0190`, `0x004D07C0`), the about fifty `SetOrGet*` dialog field fillers with
`SetOrGetControlValue`, the `SetStaticControl*` / `SetSliderControlValue` helpers, the route code (`MakeRoutePath`,
`GetOccupantPaths` and its building/network variants, `ShowAllRoutesThatIntersectCityRect`,
`CalculateTripVolumeFromTrafficPaths`, `DoesRouteCrossCell`, `GetRouteTravelTypes`),
`SetLegendMinimized`, `IsDebugQueryEnabled`, `PlayOccupantQuerySound`, the rating helpers, and 42
reach-attributed helpers. The exact list is the owner's rows of `coverage.py --owners` minus the covered list.

### 8.11 Tips: builders, display, `SetTip`, `SetHighlightState` *[compat]*

All localised strings below are in group `0xEA5524EB`; only the instance is given. "Headlines" means: get the
lot's headline list (`GetLotHeadlines`, `0x004D4FD0`) and append its first three entries (at most) to the body,
each preceded by `\n` when the body is non-empty at that point.

#### 8.11.1 `GetTerrainSummaryInfo(title, body)` (`0x004D6C70`)

1. Turn `+0x7C`, `+0x84` into a cell (city slot 95). If the terrain (city slot 84) says the cell has water
   (`CellHasWater`, terrain slot 34): return, both strings untouched.
2. title := `0x6A5BBCA6`. zone := zone manager (`0x00B43D14`) slot 12 `GetZoneType(cellX, cellZ)`.
3. By zone (zones above 15 change nothing; zones 1..14 set the body, zones 0 and 15 replace the title):

| Zone | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|---|---|---|---|---|---|---|---|---|
| String | title `0xAA5BBDE4` | body `0x0A5BBDEF` | `0xAA5BBE08` | `0x8A5BBE12` | `0x6A5BBE1F` | `0x6A5BBE29` | `0xAA5BBE33` | `0x0A5BBE3C` |

| Zone | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 |
|---|---|---|---|---|---|---|---|---|
| String | body `0xCA5BBE4A` | `0x0A5BBE51` | `0x6A5BBE61` | `0xAA5BBE69` | `0x2A5BBE6F` | `0xCA5BBE78` | `0x2A5BBE80` | title `0x2A5BBE85` |

4. The lot at the cell (lot manager slot 6, `GetLot(x, z, false)`): when there is one, its headlines.

#### 8.11.2 `GetNetworkOccupantSummaryInfo(title, body)` (`0x004CF8A0`)

QI the highlighted occupant for `0xA821EF94` (network occupant); none: return. Test the network types (network
occupant slot 22, true when the occupant carries the type) in this order and take the first match; the title
gets the string:

| Order | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Type | 2 | 8 | 12 | 0 | 1 | 10 | 11 | 9 | 3 | 4 | 7 | 6 |
| Title | `0xAA5589FB` | `0x8B8C5CBD` | `0x2BE09C9E` | `0x6A5589D4` | `0x4A5589F3` | `0x4BD9FDD8` | `0xCBE09C9B` | `0x0BE09CA2` | `0x8A558A04` | `0x0A558A0A` | `0x8A558A10` | `0x8B8C5CD0` |

Type 5 and no match leave both strings empty. Type 4 only, when `IsDebugQueryEnabled()`: body :=
`Distressed Pipes: %d / %d\n` with plumbing simulator (`0x00B43D94`) slot 19 `GetNumDistressedPipes` and slot 18
`GetNumPipes` (the format replaces the body, cIGZString slot 27 `Sprintf`).

#### 8.11.3 Display (tail of message `0x0A456D95`) and `SetTip(title, body)` (`0x004C5BA0`)

After the builders and the debug line (§8.7):

1. Callout held (`+0x44`) **and** icon non-zero: set the callout's skin (callout slot 4 `(skin, padX, padY)`,
   callout-box.md §2) to the resource key (type `0x856DDBAC`, group `0x46A006B0`, instance icon) with padding
   `(12, 10)`: 12 horizontal and 10 vertical pixels between the box edge and its text; then the meter (callout
   slot 14 `(meter, count, fill)`): with a secondary icon, the key (same type and group, instance secondary),
   `count` 5 (the meter's repeat count) and `fill` the fraction (clamped to 0–1 by the callout); without one,
   `(null, 0, 0)`. So the "icon" of this section is the callout's skin (background bitmap) and the "secondary
   icon" its gauge bitmap; the names are kept because the builders and tables below use them (answers CT13). Then `SetTip(title, body)` and clear the cursor text
   (3D view slot 41, key `0x2A456CB1`).
2. Otherwise (no callout, or icon 0): hide the callout's window when there is a callout (window slot 70), then
   with a 3D view set the cursor text (3D view slot 40) with (key `0x2A456CB1`, 0, title, body, 0). So icon-less
   tips (flora, props, networks, terrain, power poles, landfill, traffic info without an icon) appear as cursor
   text, never in the callout.

`SetTip(title, body)`: both empty (null or length 0): clear the cursor text (3D view slot 41, `0x2A456CB1`) and
hide the callout window. Otherwise set the callout's title (callout slot 6) and body (slot 8); when the callout
window is not visible (window slot 71), read the cursor position relative to the 3D view's window (window
manager slot 34 with the window from 3D view slot 3), move the callout there (callout slot 15) and show its
window (window slot 69). An already visible callout only changes its text (`OnMouseMove` step 6 moves it).

#### 8.11.4 `GetBuildingOccupantTipInfo(title, body, &icon, &secondary, &fraction)` (`0x004D5E30`)

Setup: the occupant's property holder (occupant slot 6) and its lot (lot manager slot 14, may be null).

- `IsDebugQueryEnabled()`: icon := 0; title := the exemplar name (property `0x20`); body := `DescribeBuilding`
  (`0x004CCF60`); with a lot, its centre cell (lot slot 15, city slot 95) feeds `DescribePopulation`
  (`0x004CF2B0`), `DescribeDemand` (`0x004CD820`) and `DescribeCell` (`0x004CE8A0`, with the zone type from zone
  manager slot 12). Return (no headlines). The description writers are §8.10 remainder.
- Otherwise: name := the occupant's display name (interface `0x87DDA3A9`, its slot 19) or empty; title := name.
  Read the exemplar's property `0xAA1DD396` (occupant groups; property holder slot 9). When it exists, is
  interface `0x60FD4D2B` and has type `0x8006` (uint32 array), take the first group of the table below that the
  array contains (each test scans the whole array) and run its row; every row sets the title to the name again.
  A missing or mistyped property goes straight to the headlines.

| Group (test order) | Icon | Secondary | Fraction / body |
|---|---|---|---|
| `0x1000` | `0x14416208` | - | wealth map R (below) |
| `0x1001` | `0x14416202` | - | wealth map C |
| `0x1002` | `0x14416206` | - | wealth map I |
| `0x1003` | `0x14416209` | transport rule | transport rule |
| `0x1004` | `0x1441620A` | utility rule | utility rule |
| `0x1501` | `0x14416207` | `0x14416212` | `Ratio(trunc(N * V), N)`: V := the float at `*(police 0x00B43D98 slot 36 GetJailedPercentage())`, N := police slot 28 `GetJailCapacity(occupant)` (protection.md §13) |
| `0x1500` | `0x14416207` | `0x14416212` | police (`0x00B43D98`) slot 25 `GetStationEfficiency(occupant)` (an `SC4Percentage` returned through a hidden pointer; its float) |
| `0x1502` | `0x14416203` | `0x14416211` | fire protection (`0x00B43D78`) slot 10 `GetStationEfficiency(occupant)` (same shape) |
| `0x1503`, `0x1504`, `0x1505`, `0x1506` | `0x14416201` | `0x14416210` | residential simulator (`0x00B43DA4`) slot 26(occupant, &query); on true, the query's float at `+0x28` |
| `0x1507` | `0x14416201` | `0x14416216` | as above with slot 27 |
| `0x150A`, `0x150B` | `0x1441620C` | - | - |
| `0x1006` | `0x14416204` | - | - |
| none of these | `0x14416200` | - | - |

Rows that leave the fraction alone keep the caller's 0.5.

Wealth maps (groups `0x1000..0x1002`; no lot: nothing): developer type := BDS (`0x00B43D68`) slot 30(purpose,
`GetBuildingWealth(lot)`), purpose := `*(BDS slot 39(lot slot 46(0)))`; body := the string; other types leave
the body.

| Map | Developer type → body |
|---|---|
| R | `0x1010` → `0xCA95F27E`, `0x1020` → `0x6A95F2B0`, `0x1030` → `0xCA95F2B8` |
| C | `0x3110` → `0x8A95F2BF`, `0x3120` → `0xAA95F2DC`, `0x3130` → `0x8A95F2E4`, `0x3320` → `0x0A95F39A`, `0x3330` → `0x6A95F3B3` |
| I | `0x4100` → `0x4A95F2EB`, `0x4200` → `0x2A95F324`, `0x4300` → `0x6A95F313`, `0x4400` → `0xCA95F31C` |

Transport rule (`0x1003`), first match:
- `0x1508`: secondary `0x14416214`; with a lot, airport developer (city slot 77) slot 12(lot, &a, &b); on true
  fraction := `Ratio(a, b)`.
- `0x1509`: secondary `0x14416215`; the same with the seaport developer (city slot 76).
- Otherwise: fraction := 0; the occupant's position (occupant slot 8) to a cell (city slot 95); traffic simulator
  (`0x00B43DA8`) slot 35(cellX, cellZ, &rec); on true with `rec+0x00` non-zero: `rec+0x0C` zero gives 1.0, else
  fraction := `rec+0x00 / rec+0x0C` (both unsigned, to float) clamped to at most 1.0. Then the secondary by
  subgroup, first match: `0x1307` → `0x1441621C`, `0x1300` → `0x1441621B`, `0x1301` → `0x1441621A`, `0x1302` →
  `0x1441621C`, `0x1303` → `0x1441621C`, `0x1304` → `0x14416215`, `0x130B` → `0x1441620F`; none: stays 0.

Utility rule (`0x1004`), first match:
- `0x1400`: fraction := the float returned by power simulator (`0x00B43DA0`) slot 45(occupant); secondary
  `0x14416218`.
- `0x1401`: secondary `0x14416219`; plumbing simulator slot 24(occupant, &f, &flag, 0, 0), f preset 0 and flag
  false; fraction := f; flag true: body := `0x4A5FB66A`.
- `0x1402`: secondary `0x14416217`; a := pollution simulator (`0x00B43D9C`) slot 54
  `GetRecyclingCenterMaxCapacity(occupant, &a)`, b := slot 55 `GetRecyclingCenterCurrentCapacity(occupant, &b)`
  (pollution.md slot table; floats, preset
  0); fraction := 0; when b > the float at `0x00B21854` (zero-initialised data with no writer, so 0.0):
  fraction := b / a, clamped to at most 1.0 *[quirk: the guard tests b while a is the divisor; a = 0 gives +inf,
  clamped to 1.0]* (instructions from `0x004D6910`; Ghidra shows a division by 0.0).
- None: icon and title only.

`Ratio(a, b)` (`0x004C3B10`, unsigned a and b, float result, x87 with float constants): b = 0 → 0.0;
a ≤ 0.8·b (float `0x00A9095C`) → 1.0; else (b − a, or 0 when a ≥ b) / (0.2·b), 0.2 being the float `0x00A90A68`
(0.19999999). Worked: b = 100: a = 80 → 1.0; a = 90 → 10 / 20 = 0.5 (to float rounding); a = 120 → 0.0.

Every non-debug path ends by appending the headlines of the occupant's lot (`0x004D5C90`).

#### 8.11.5 `SetHighlightState(state)` (`0x004CACC0`)

No highlighted occupant (`+0x2C`) or no lot for it (lot manager slot 14): nothing. Otherwise list the lot's
occupants (lot slot 49, into a list whose nodes come from the allocator service `0x3AE4BEA3`/`0x3AE4BEAB`, cached
in `0x00B207E4`); for each occupant whose boolean property `0x6A95E503` is true (default false) call occupant
slot 17(state, 0); release the list (occupant manager `0x00B43D0C` slot 48). The highlighted occupant itself is
changed only when it carries the property.

### 8.12 The query click: `DescribePick` (`0x004D4140`) and `DoQueryDialog` (`0x004D3C30`) *[compat]*

Added 2026-10-21 (re-querytool4 round 3), from the instructions (REA had no binary open; Ghidra's listing of
`DescribePick` has a broken stack frame, so offsets below come from the instructions).

#### 8.12.1 `DescribePick(x, z, text, extra, extra2, debug)` (`0x004D4140`, cdecl, 6 args)

`text`, `extra`, `extra2` are three `cIGZString`s (the static ones of §8.6 step 5); `debug` is
`IsDebugQueryEnabled()`. They are debug read-outs: nothing in this owner reads `0x00B2185C`/`0x00B21870`/
`0x00B21884` back (no data references), so their only player-visible effect is none *[inferred]*; the dialogs and
the sound are the visible effect.

1. Reset the strings: `text` := `Query`, `extra` := `Query extra: none`, `extra2` := `Query extra2: none`.
2. No 3D view (`0x00B43DC4`): return.
3. Building name := `empty`. Build a `cSC4QueriableOccupantFilter` (0x0C bytes, vtable `0x00A90B18`;
   occupant-manager.md §13.2.6: the hover test, and nothing whose holder has `0xCAA9AB92`) and `PickQueriableOccupant(x, z, &occ, filter)` (§8.8).
   - Hit: if the lot manager (`0x00B43D08` slot 14) finds the occupant's lot and that lot has a building (lot
     slot 48), the building's occupant (building slot 3) replaces the hit (no property test, unlike hover §8.5).
   - Miss: pick the terrain (3D view slot 36); on a hit take the cell (`PositionToCell`, city slot 95), the lot
     there (lot manager slot 6) and, when it has a building, that building's occupant.
4. With an occupant: `type` := occupant slot 7; `PlayOccupantQuerySound(occ)` (`0x004C4F40`); then the dialog
   choice, first match wins:

| Test (in order) | Action | "dialog shown" |
|---|---|---|
| QI `0xA821EF94` (network occupant) succeeds | `DoNeighborConnectionQueryDialog(occ)` (`0x004CC6E0`), then continue with the next rows | no |
| else QI `0xA8C86142` succeeds | `DoNeighborConnectionQueryDialog(occ)`; stop | yes |
| exemplar property `0x2A499F85` readable (`cSCPropertyHelp::GetPropertyValue` true) | `DoQueryDialog(occ)` (§8.12.2) | yes |
| else: zone type at the occupant's cell (position slot 8, `PositionToCell`, zone manager `0x00B43D14` slot 12) is 11, 12 or 14 | `DoZoneBasedQueryDialog(zone, cellX, cellZ)` (`0x004CCCB0`) | yes, and "zone dialog" |
| else | UI sound: `0x00B43CB8` slot 11(1, `0xAA62C9D5`, 0, 0, 0) | no |

5. Debug lines by type (only these two types set "described"):
   - building `0x278128A0`: a `cISC4ResidentialSimulator::tQueryData` is built and dropped (no visible effect).
   - network `0xC772BF98`: with QI `0x4A10AFC2`, `text` := `Query cell (%d, %d): Id %08lx, flip: %s, rotation:
     %d, model Y Height %f, floor heights %f %f %f %f\n` (flip `yes`/`no`); else with QI `0xA821EF94`, `text` :=
     `Query cell (%d, %d): Piece Id: %08lx, Rotation:%d, Flipped:%d \n` plus
     `, traffic cap(%.0f) vol(%d) cong(%f)` where cap is traffic simulator (`0x00B43DA8`) slot 23(1), or slot 23(2)
     when that is 0.0, vol = 10 x the low byte of the cell's volume, cong = vol / cap (float); and when `debug`,
     `WriteDetailedDebugInfo` (`0x004D12A0`).
   Release the occupant. A zone dialog was shown: return. Otherwise when not "described", pick the terrain at
   (x, z) again (miss: return) and take its cell.
6. Cell read-out (all debug text): when no dialog was shown and the cell's zone is 11, 12 or 14,
   `DoZoneBasedQueryDialog` (so an occupant-less click on an airport, seaport or landfill zone opens it); when
   `debug`, `WriteDetailedDebugInfo`; lot name := `no lot template`, or `%s_%08x` (lot configuration name, ID) for
   the lot at the cell; find the first building in the cell (occupant manager `0x00B43D0C` slot 41, type
   `0x278128A0`); with one: `DescribeBuilding`, `DescribePopulation`, `DescribeDemand`, `extra` := `%s %s\n`
   (population, demand); without: `DescribeDemand` into `extra`, suffixed `: ` + lot name, or `: no lot`; then
   ` EQ: %f HQ: %f` where HQ := residential simulator (`0x00B43DA4`) slot 36(x, z) and EQ := slot 35(x, z, HQ);
   `DescribeCell`; `extra2` := `[Building %s] [Lot: %s] %s\n` (with `text`) when `text` is longer than 10
   characters, else `[Building %s] [Lot: %s]\n`; `text` := `Query cell (%d, %d): %s\n` (cell, cell description).

#### 8.12.2 `DoQueryDialog(occ)` (`0x004D3C30`) → bool

1. Read the occupant's property `0x2A499F85` (the dialog's UI script instance); unreadable: return false.
2. Create the dialog `SC4CreateWindowFromScript({type 0, group 0x96A006B0, instance = value}, null, 0x10000005)`;
   none: return false.
3. Coverage rings: save the 3D view's coverage helper's categories (helper `+0x14`, 3D view slot 49); categories :=
   `GetCategoriesForOccupant(occ)`; when non-zero set them, `RecalcCoverage`, and start a local timer with
   `CoverageRingCallback` (§8.9) at 330 ms (`0x14A`).
4. If the dialog QIs to `0x5386D516` (window proc host), attach a 0x18-byte window proc (`0x004C5620`, built with
   the dialog) holding the occupant.
5. Make a 4-byte enumerator context (vtable `0x00A90B38`) and walk the dialog's children with `0x004C4340` (each
   child's slot 116, recursive).
6. Publish the refresh state used by message `0x66956814` (§8.7): `0x00B21848` := `0x004D3B40` (the refresh
   walk), `0x00B21840` := occ (held), `0x00B21844` := the window proc, `0x00B2184C` := the dialog (held).
7. Fill: walk the children with the get pass (§8.12.3, flag 1).
8. Run it modally: `0x00B43CE0` slot 10(dialog, 1), window manager slot 41(dialog) = result, `0x00B43CE0`
   slot 6(dialog). Clear `0x00B21848`, `0x00B21840`, `0x00B21844`, `0x00B2184C` (releasing the holds).
9. Result 0 (accepted): walk again with the set pass (flag 0). Then `EnumFuncClearFilter` (`0x004C4370`) over the
   children, detach the window proc (its slot 6 with null), window manager slot 23(dialog), release the dialog,
   then window manager slot 37(the 3D view's window, 3D view slot 3) (focus back to the view *[inferred]*).
10. If categories were set in step 3: restore the saved ones, `RecalcCoverage`, stop the timer.
11. Return result == 0.

#### 8.12.3 The control walk and the filler table (Q CT12)

A walk visits each child window of the dialog (window slot 32, interface `0x22BA0121`). For a child whose
control ID has bit `0x1000` set and whose `ID & 0xFFF` is below `0x7F` with a non-null entry in the table at
`0x00A92488` (127 dwords), it calls `filler(window, ID, get, occ)` (cdecl); when there is no filler or it returns
false, the walk descends into that child's own children. The get pass (`0x004D3AE0`) passes `get` = 1 (fill the
control from the simulation), the set pass (`0x004D3BD0`) 0 (write the control's value back; most fillers do
nothing and return true); the daily refresh (`0x004D3B40` via `0x004D3A90`) calls the same table.

Two filler shapes, exact for the samples read:

- Value fillers (e.g. index 1, `0x004C5180`, 60 B): get = 1 only; read a value from a simulator and display it
  with `SetPerformanceMeterValue(window, ID, value)` (`0x004C3CA0`: child `0xC12CEA13`, property `0xEA563AB0`
  set to a `cRZVariant`); index 1 is fire `GetStationEfficiency(occ)`; get = 0 returns true.
- Forwarders (e.g. index 17, `0x004C67F0`, 62 B): with an occupant, occupant slot 11 writes two dwords v0, v1
  and the call becomes `worker(window, ID, get, v0, v1)` (`SetOrGetAirportAirTrafficVolume`, `0x004C5280`); no
  occupant: false.

The per-index table (index = control ID & 0xFFF; ID = `0x1000 | index`), from the data at `0x00A92488`. Bytes
from `decomp/index.tsv`; `own` = in this owner's coverage rows. Unlisted indexes are null. Names and kinds:
re-query (2026-10-21); what each entry shows is §8.12.4.


| Index | ID | Filler | Name | Bytes | Own | Kind (§8.12.4) |
|---|---|---|---|---|---|---|
| 0 | `0x1000` | `0x004C70C0` | SetOrGetBuildingName | 314 | own | edit |
| 1 | `0x1001` | `0x004C5180` | SetOrGetFireStationEfficiency | 60 | own | meter |
| 2 | `0x1002` | `0x004C7200` | SetOrGetFireStationFunding | 221 | own | slider |
| 3 | `0x1003` | `0x004C72E0` | SetOrGetFireStationDispatchCount | 110 | own | number |
| 4 | `0x1004` | `0x004C7350` | SetOrGetFireStationCost | 60 | own | money |
| 6 | `0x1006` | `0x004C51C0` | SetOrGetPoliceStationEfficiency | 60 | own | meter |
| 7 | `0x1007` | `0x004C7390` | SetOrGetPoliceStationArrests | 50 | own | number |
| 8 | `0x1008` | `0x004C73D0` | SetOrGetPoliceStationCrimes | 50 | own | number |
| 9 | `0x1009` | `0x004C7410` | SetOrGetPoliceStationFunding | 221 | own | slider |
| 10 | `0x100A` | `0x004C74F0` | SetOrGetPoliceStationDispatchCount | 110 | own | number |
| 11 | `0x100B` | `0x004C7560` | SetOrGetPoliceStationCost | 60 | own | money |
| 12 | `0x100C` | `0x004C5200` | SetOrGetJailLoad | 113 | own | meter |
| 13 | `0x100D` | `0x004C75A0` | SetOrGetJailComfortableCapacity | 66 | own | number |
| 14 | `0x100E` | `0x004C75F0` | SetOrGetJailInmates | 100 | own | number |
| 15 | `0x100F` | `0x004C7660` | SetOrGetJailUpkeepCost | 43 | own | money |
| 16 | `0x1010` | `0x004C9E50` | SetOrGetOccupantCost | 5 | own | Occupant |
| 17 | `0x1011` | `0x004C67F0` | SetOrGetAirportOccupantAirTrafficVolume | 62 | own | Airport |
| 18 | `0x1012` | `0x004CC120` | SetOrGetAirportOccupantNoisePollution | 62 | own | Airport |
| 19 | `0x1013` | `0x004C9E60` | SetOrGetAirportOccupantMaintenanceCost | 62 | own | Airport |
| 20 | `0x1014` | `0x004CDC80` | SetOrGetAirportOccupantLostLuggage | 62 | own | Airport |
| 21 | `0x1015` | `0x004C5310` | SetOrGetSeaportOccupantTrafficVolume | 62 | own | meter |
| 22 | `0x1016` | `0x004C7910` | SetOrGetOccupantWaterPollution | 109 | own | number |
| 23 | `0x1017` | `0x004C7980` | SetOrGetSchoolGrade | 235 | own | meter |
| 24 | `0x1018` | `0x004C7A70` | SetOrGetSchoolStaffFunding | 305 | own | slider |
| 25 | `0x1019` | `0x004C7BB0` | SetOrGetSchoolBusFunding | 292 | own | slider |
| 26 | `0x101A` | `0x004C8160` | SetOrGetCivicBuildingEstimatedStaff | 68 | own | number |
| 27 | `0x101B` | `0x004C7CE0` | SetOrGetCivicBuildingEstimatedCapacity | 69 | own | number |
| 28 | `0x101C` | `0x004C7D30` | SetOrGetSchoolServed | 218 | own | number |
| 29 | `0x101D` | `0x004C7CE0` | SetOrGetCivicBuildingEstimatedCapacity | 69 | own | number |
| 30 | `0x101E` | `0x004C7D30` | SetOrGetSchoolServed | 218 | own | number |
| 31 | `0x101F` | `0x004C7CE0` | SetOrGetCivicBuildingEstimatedCapacity | 69 | own | number |
| 32 | `0x1020` | `0x004C7E10` | SetOrGetHospitalGrade | 235 | own | meter |
| 33 | `0x1021` | `0x004C7F00` | SetOrGetHospitalStaffFunding | 304 | own | slider |
| 34 | `0x1022` | `0x004C8030` | SetOrGetHospitalAmbulanceFunding | 291 | own | slider |
| 35 | `0x1023` | `0x004C8160` | SetOrGetCivicBuildingEstimatedStaff | 68 | own | number |
| 36 | `0x1024` | `0x004C7CE0` | SetOrGetCivicBuildingEstimatedCapacity | 69 | own | number |
| 37 | `0x1025` | `0x004C81B0` | SetOrGetHospitalServed | 218 | own | number |
| 41 | `0x1029` | `0x004C3E70` | SetOrGetGarbageBuildingEfficiency | 100 | own | meter |
| 42 | `0x102A` | `0x004C8330` | SetOrGetIncineratorUsagePercent | 121 | own | percent |
| 43 | `0x102B` | `0x004C83B0` | SetOrGetIncineratorGarbageAmount | 106 | own | number |
| 44 | `0x102C` | `0x004C8420` | SetOrGetOccupantAirPollution | 78 | own | number |
| 45 | `0x102D` | `0x004C5350` | SetOrGetPowerPlantEfficiency | 66 | own | meter |
| 46 | `0x102E` | `0x004C9FE0` | thunk_FUN_004c8660 | 5 | own | percent |
| 47 | `0x102F` | `0x004C86D0` | SetOrGetPowerPlantMaxOutput | 58 | own | number |
| 48 | `0x1030` | `0x004C8710` | SetOrGetPowerPlantOutput | 76 | own | number |
| 49 | `0x1031` | `0x004C9FF0` | SetOrGetPowerPlantUsedOutput | 77 | own | number |
| 50 | `0x1032` | `0x004D3250` | thunk_FUN_004d10f0 | 5 | own | per-unit |
| 51 | `0x1033` | `0x004C85F0` | SetOrGetWasteToEnergyGarbageAmount | 106 | own | number |
| 52 | `0x1034` | `0x004C3F90` | SetOrGetPowerPlantEfficiency2 | 63 | own | meter |
| 53 | `0x1035` | `0x004C8660` | SetOrGetPowerPlantUsagePercent | 98 | own | percent |
| 54 | `0x1036` | `0x004C86D0` | SetOrGetPowerPlantMaxOutput | 58 | own | number |
| 55 | `0x1037` | `0x004C8710` | SetOrGetPowerPlantOutput | 76 | own | number |
| 56 | `0x1038` | `0x004C9FF0` | SetOrGetPowerPlantUsedOutput | 77 | own | number |
| 57 | `0x1039` | `0x004D10F0` | SetOrGetPowerPlantCostPerUnit | 156 | own | per-unit |
| 58 | `0x103A` | `0x004C42F0` | SetOrGetWaterBuildingEfficiency | 79 | own | meter |
| 59 | `0x103B` | `0x004C8880` | SetOrGetWaterBuildingUsagePercent | 113 | own | percent |
| 60 | `0x103C` | `0x004C8900` | SetOrGetWaterBuildingMaxOutput | 71 | own | number |
| 61 | `0x103D` | `0x004C8950` | SetOrGetWaterBuildingOutput | 71 | own | number |
| 62 | `0x103E` | `0x004C89A0` | SetOrGetWaterBuildingUsedOutput | 71 | own | number |
| 63 | `0x103F` | `0x004D1190` | SetOrGetWaterBuildingCostPerUnit | 150 | - | per-unit |
| 64 | `0x1040` | `0x004CA090` | SetOrGetWaterBuildingPumpCondition | 159 | own | Water |
| 65 | `0x1041` | `0x004C3FD0` | SetOrGetTransitStationServiceQuality | 223 | own | meter |
| 66 | `0x1042` | `0x004C8A60` | SetOrGetTransitStationUsagePercent | 204 | own | percent |
| 67 | `0x1043` | `0x004C8B30` | SetOrGetTransitStationMaintenanceCost | 141 | own | money |
| 68 | `0x1044` | `0x004C8BC0` | SetOrGetTransitStationUsage | 139 | own | number |
| 69 | `0x1045` | `0x004C8C50` | SetOrGetTransitStationCapacity | 139 | own | number |
| 70 | `0x1046` | `0x004C8CE0` | SetOrGetTransitStationGlobalFunding | 92 | own | TransitStation |
| 71 | `0x1047` | `0x004C8D40` | SetOrGetTransitStationLocalCost | 97 | own | money |
| 73 | `0x1049` | `0x004C8DB0` | ? | ? | - | money |
| 74 | `0x104A` | `0x004CA130` | ? | ? | - | text |
| 81 | `0x1051` | `0x004C8FF0` | SetOrGetRCIBuildingCurrentOccupancy | 170 | own | RCIBuilding |
| 82 | `0x1052` | `0x004C90A0` | SetOrGetRCIBuildingMaxOccupancy | 115 | own | RCIBuilding |
| 83 | `0x1053` | `0x004D3500` | SetOrGetRCIBuildingWealthLevel | 554 | own | RCIBuilding |
| 84 | `0x1054` | `0x004D3730` | SetOrGetOccupantPollutionRating | 105 | own | text |
| 85 | `0x1055` | `0x004D3810` | SetOrGetOccupantCrimeRating | 105 | own | text |
| 86 | `0x1056` | `0x004C9120` | SetOrGetRCIBuildingLandValue | 215 | own | RCIBuilding |
| 87 | `0x1057` | `0x004D3880` | SetOrGetResidentialBuildingCommute | 190 | own | ResidentialBuilding |
| 88 | `0x1058` | `0x004C9200` | SetOrGetResidentialBuildingMayorApproval | 156 | own | ResidentialBuilding |
| 89 | `0x1059` | `0x004D39B0` | SetOrGetOccupantSchoolRating | 105 | own | text |
| 90 | `0x105A` | `0x004D3A20` | SetOrGetOccupantHospitalRating | 105 | own | text |
| 91 | `0x105B` | `0x004CDCC0` | SetOrGetResidentialBuildingMySimicon | 576 | own | ResidentialBuilding |
| 92 | `0x105C` | `0x004C92A0` | SetOrGetCommercialServiceBuildingCustomers | 101 | own | CommercialServiceBuilding |
| 94 | `0x105E` | `0x004C3EE0` | SetOrGetRecyclingCenterEfficiency | 167 | own | meter |
| 95 | `0x105F` | `0x004C8470` | SetOrGetRecyclingPercent | 217 | own | percent |
| 96 | `0x1060` | `0x004C8550` | SetOrGetRecyclingAmountPerCenter | 157 | own | number |
| 97 | `0x1061` | `0x004CA040` | SetOrGetPowerPlantCondition | 70 | own | percent |
| 98 | `0x1062` | `0x004CA040` | SetOrGetPowerPlantCondition | 70 | own | percent |
| 99 | `0x1063` | `0x004C8760` | SetOrGetPowerPlantFunding | 276 | own | slider |
| 107 | `0x106B` | `0x004CB880` | ? | ? | - | number |
| 108 | `0x106C` | `0x004C9310` | SetOrGetOccupantCost | 282 | own | Occupant |
| 109 | `0x106D` | `0x004C42F0` | SetOrGetWaterBuildingEfficiency | 79 | own | meter |
| 115 | `0x1073` | `0x004C89F0` | SetOrGetWaterBuildingCondition | 100 | own | percent |
| 116 | `0x1074` | `0x004D3940` | SetOrGetOccupantTrafficRating | 105 | own | text |
| 117 | `0x1075` | `0x004D37A0` | SetOrGetOccupantGarbageRating | 105 | own | text |
| 118 | `0x1076` | `0x004CA2C0` | SetOrGetOccupantIsPowered | 206 | own | text |
| 119 | `0x1077` | `0x004CA390` | SetOrGetOccupantIsWatered | 206 | own | text |
| 120 | `0x1078` | `0x004C9FA0` | SetOrGetSeaportOccupantMaintenanceCost | 62 | own | zone money |
| 121 | `0x1079` | `0x004D0EE0` | SetOrGetWhyOccupantAbandoned | 490 | own | Why |
| 122 | `0x107A` | `0x004C55A0` | SetOrGetOccupantIsHistorical | 124 | own | Occupant |
| 123 | `0x107B` | `0x004C9EA0` | SetOrGetAirportOccupantUsage | 62 | own | zone number |
| 124 | `0x107C` | `0x004C9EE0` | SetOrGetAirportOccupantCapacity | 62 | own | zone number |
| 125 | `0x107D` | `0x004C9F20` | SetOrGetSeaportOccupantUsage | 62 | own | zone number |
| 126 | `0x107E` | `0x004C9F60` | SetOrGetSeaportOccupantCapacity | 62 | own | zone number |

#### 8.12.4 What each entry shows (re-query)

*Added 2026-10-21 (re-query), from the instruction bytes; the table above now carries the fillers' names and kinds.*
Each entry is `(child window, id, fill, occupant)`. With fill = false only sliders and the two editable entries
(`0x1000`, `0x107A`) do anything. The kinds write to a child of the window:

- **number**: `"%d"` into the text control (IID `0x212CDC1F`, its slot 4); **percent**: `"%d%%"`; **money**:
  `SC4MakeMoneyString` of an int64; **text**: a string.
- **meter**: the child with IID `0xC12CEA13` gets the float window property `0xEA563AB0`
  (`SetPerformanceMeterValue`).
- **slider** (`SetSliderControlValue`, `0x004C6E30`, given a funding `f`, a cost `c` and a list of related ids): the
  slider (IID `0x21325207`) is set to `v + 1` and then `v`, `v = round(100 × f)`; its window properties `0x6A8CC9D6`
  (the entry), `0x2AA6E8AD` (`c`, int64) and `0x0AB3F6DB` (the related ids) are stored; the money text of **id +
  0x100** := `trunc(v × 0.01 × c)`. An entry run with fill = false reads the slider's value `v` back and sets the
  funding `v × 0.01` (§8.13).
- **load meter** `L(used, cap)` (`0x004C3B10`): 0 when `cap` is 0; 1.0 while `used ≤ 0.8 · cap`; otherwise
  `max(cap − used, 0) / (0.2 · cap)`.
- **per-unit cost** (`0x004D0D30`): a double formatted by the locale manager into a text.
- **level** strings (group `0xEA5524EB`): `0x4A5FE266` (low), `0x0A5FE260` (medium), `0xAA5FE249` (high).
  `GetRatingIn3DegreesFromPercent(p)`: `p < 33.3` low, `p < 66.6` medium, else high.

"Fire" is the fire simulator (`0x00B43D78`), "police" `0x00B43D98`, "school" and "hospital" the residential
simulator's `GetSchoolQueryData` / `GetHospitalQueryData` record (slots 26 / 27). Absent ids (`0x1005`,
`0x1026`–`0x1028` in this table, `0x1048`, `0x104B`–`0x1050`, `0x105D`, `0x1064`–`0x106A`, `0x106E`–`0x1072`) do
nothing.

- `0x1000` (edit): the building's name (`cISC4BuildingOccupant` slot 19); write-back renames it (slot 18) when the
  text changed.
- `0x1001` (meter): fire slot 10 `GetStationEfficiency`. `0x1002` (slider): fire slot 8 `GetStationFunding`
  (percentage, cost); write-back slot 9. `0x1003` (number): the fire dispatch interface (`0xC9C00435`) slot 7: the
  third output (the count). `0x1004` (money): the cost of fire slot 8.
- `0x1006` (meter): police slot 25 `GetStationEfficiency`. `0x1007`, `0x1008` (number): police slots 43 (arrests) and
  44 (crimes). `0x1009` (slider): police slot 7 `GetStationFunding`; write-back slot 8. `0x100A` (number): the
  police dispatch interface slot 7: the count. `0x100B` (money): the cost of police slot 7.
- `0x100C` (meter): `L(trunc(jailed × capacity), capacity)`: police slots 36 `GetJailedPercentage`, 28
  `GetJailCapacity`. `0x100D` (number): `trunc(0.8 × GetJailCapacity)`. `0x100E` (number): `trunc(GetJailedPercentage
  × GetJailCapacity)` (the inmates). `0x100F` (money): police slot 37 `GetJailCurrentUpkeepCost`.
- `0x1010`, `0x106C` (money): the sum of the costs of the budget items (budget slot 51) for each purpose in `o`'s
  property `0xEA54D285`.
- `0x1011` (meter): airport: `L(usage, cap)` of the airport developer's slot 12 for the cell's lot. `0x1012`
  (number): airport noise: the lot's buildings' pollution property `0x27812851` *[summary]*. `0x1013` (money):
  airport developer slot 14 `GetMaintenanceCost(lot)`. `0x1014` (number): lost luggage: the goofy stat (below) of the
  shipped airport exemplar (`0x6534284A`, `0x67CD5FA1`, `0xCA270E9D`). `0x1015` (meter): seaport: `L(usage, cap)` of
  the seaport developer's slot 12.
- `0x1016` (number): the water value (element 1) of `o`'s pollution at centre `0x27812851` (pollution.md).
- `0x1017` (meter): school grade (`+0x28`). `0x1018` (slider): school staff funding and cost (`+0x2C`, `+0x30`),
  related ids `0x101A`, `0x101B`, `0x101D`, `0x101F`; write-back residential slot 19 `(o, staff, bus, true)`. `0x1019`
  (slider): school bus funding and cost (`+0x34`, `+0x38`); write-back as `0x1018`. `0x101A`, `0x1023` (number):
  residential slot 28 `EstimateCurrentOccupantCapacity`: the staff. `0x101B`, `0x101D`, `0x101F`, `0x1024` (number):
  the same: the capacity. `0x101C`, `0x101E` (number): school served (`+0x24`).
- `0x1020` (meter): hospital grade. `0x1021` (slider): hospital staff funding, related ids `0x1023`, `0x1024`,
  `0x00000004`, `0x3F800000` *[quirk: adjacent static data supplies the last two; §8.16.1]*; write-back slot 19.
  `0x1022` (slider): hospital ambulance funding; write-back slot
  19. `0x1025` (number): hospital served.
- `0x1026` (zone meter): landfill: `L(usage, cap)` of `GetLandfillUsage` (landfill developer slot 10). `0x1027` (zone
  number): `trunc(GetGarbageLandfilledLastMonth × GetGarbageScalingFactor)` (pollution slots 41, 48). `0x1028` (zone
  money): the landfill developer's slot 8 `GetCurrentBudget` (city slot 78).
- `0x1029` (meter): `trunc(100 × GetCurrentOccupantEfficiency)` (pollution slot 57). `0x102A` (percent): `100 × used /
  current` of `GetIncineratorUsedCapacity` / `GetIncineratorCurrentCapacity` (slots 58, 60); 0 when current is 0.
  `0x102B` (number): `trunc(incinerator used × scaling)`. `0x102C` (number): the air value (element 0) of `o`'s
  pollution at centre `0x27812851`.
- `0x102D`, `0x1034` (meter): power slot 45 `GetCurrentPlantEfficiencyPercent`. `0x102E`, `0x1035` (percent):
  `100 × used / produced` of power slot 33 `GetOccupantPowerProduced`; 0 when produced is 0. `0x102F`, `0x1036`
  (number): power slot 34 `GetOccupantMaxPowerProduced`. `0x1030`, `0x1037` (number): produced (slot 33). `0x1031`,
  `0x1038` (number): used (slot 33). `0x1032`, `0x1039` (per-unit): power slot 49 `GetMonthlyPowerGenerationCost` /
  produced; 0 when produced is 0. `0x1033` (number): `trunc(GetWasteToEnergyUsedCapacity × scaling)` (slot 59).
- `0x103A`, `0x106D` (meter): plumbing slot 24 `GetWaterProducerInfo`: the efficiency. `0x103B` (percent): `100 ×
  used / current` of slot 24; 0 when current is 0. `0x103C`, `0x103D`, `0x103E` (number): slot 24: the maximum, the
  current, the used. `0x103F` (per-unit): plumbing slot 26 `GetOccupantMonthlyUpkeepCost` / current; 0 when current
  is 0. `0x1040` (text): slot 24's polluted flag: `0x0A5D4F43` when polluted, else `0x2A5D4F3B`.
- `0x1041` (meter): station (traffic simulator slot 35 at `o`'s cell, traffic-simulator.md §4.16): `min(available /
  usage, 1)`; 1.0 when usage is 0; 0 when available is 0 or no record. `0x1042` (percent): `trunc(100 × usage /
  maximum)`; 0 when maximum is 0 or no record. `0x1043` (money): the station's monthly maintenance cost (`+0x10`).
  `0x1044`, `0x1045` (number): the station's usage (`+0x0C`), its maximum capacity (`+0x04`).
- `0x1046` (percent): the transit department's funding (budget slot 28 `GetDepartmentBudget(0xC921CEEB)`, its slot
  10); 0 without it. `0x1047` (money): the cost of `o`'s budget item for purpose `0xAA59670C` (budget slot 51).
  `0x1049` (money): that cost times its department's funding percentage.
- `0x104A` (text): the string named by `o`'s two-word property `0xAA5C23FA` (group, instance).
- `0x1051` (text): `"%d/%d"` of the occupancy and the capacity (§8.14). `0x1052` (text): `"%d"` of the capacity;
  nothing when 0. `0x1053` (meter): the wealth level (§8.14). `0x1054`, `0x1075`, `0x1059`, `0x105A`, `0x1055`,
  `0x1074` (text): the subjective ratings 0–5 (§8.14). `0x1056` (text): land value type at `o`'s cell (land value
  slot 5): 2 medium, 3 high, else low. `0x1057` (text): commute: an abandoned lot (state 4) with reason 4 shows
  `0x0A71FAD3`; else subjective rating 6 with the strings `0xAA71FAE3`, `0x6A71FADC`, `0x0A71FAD3`. `0x1058` (text):
  the level of the mayor rating percent (§8.14). `0x105B` (icon): the MySim icon of a resident *[summary; CT14]*.
  `0x105C` (number): `trunc(100 × occupancy / capacity)`.
- `0x105E` (meter): recycling: `min(GetRecyclingCenterMaxCapacity / GetRecyclingCenterCurrentCapacity, 1)` (slots 54,
  55); 0 when current ≤ 0. `0x105F` (percent): `trunc(100 × min(r / current, 1))`, `r` = recycled / produced last
  month (slots 38, 35; 0 when none produced); 0 when current ≤ 0. `0x1060` (number): `trunc(recycled × scaling) /
  GetRecyclingCenterCount` (slot 44); 0 without centres.
- `0x1061`, `0x1062` (percent): `trunc(100 × GetCurrentPlantConditionPercent)` (power slot 46). `0x1063` (slider):
  power slot 50 `GetPowerPlantFundingPercentage`, cost slot 49; write-back slot 51.
- `0x106B` (number): the goofy stat of `o`. `0x1073` (percent): `trunc(100 × GetWaterProducerConditionPercent)`
  (plumbing slot 25). `0x1076`, `0x1077` (text): `0xCA5D4F33` when powered (power slot 31) / watered (plumbing slot
  9), else `0x6A5D4F37`. `0x1078` (zone money): seaport developer slot 14 `GetMaintenanceCost(lot)`.
- `0x1079` (text): an abandoned lot's reason (lot slot 45): 1 `0xCC06A5DE`, 2 `0xCC06A5DF`, 3 `0xCC06A5E0`, 4
  `0xCC06A5E1`, 6 `0xCC06A5E2`; otherwise empty. `0x107A` (toggle): the lot's historical flag (lot slot 43);
  write-back slot 44. `0x107B`, `0x107C` (zone number): airport developer slot 12: the usage, the capacity. `0x107D`,
  `0x107E` (zone number): seaport developer slot 12: the usage, the capacity.

The occupant entries `0x1011`–`0x1015` and `0x1078`, `0x107B`–`0x107E` take the cell from `o`'s slot 11 and call
the zone entries. The **goofy stat** (`SetOrGetGoofyStat`, `0x004CB740`): a response curve, by default the single
point (256, 256), replaced by the holder's property (`0xAA60E745`) when present, evaluated at the aura simulator's
`GetMayorRating()` (slot 8) and truncated.

### 8.13 The other dialogs and the dialog's win proc *[compat]*

*Added 2026-10-21 (re-query).*

**`PlayOccupantQuerySound(o)`** (`0x004C4F40`): with the sound system and `o`'s lot (lot manager slot 14): a lot
whose condition (lot slot 35) is 4 plays the u32 property `0x0A902434`; condition 2 or 3 plays `0xAA905AB9`.
Otherwise (or when that property is missing) the property `0xAA1DD397` plays. "Plays" is the sound system's slot 11
with `(1, sound, o, 0, 0)`.

**`DoZoneBasedQueryDialog(zone, x, z)`** (`0x004CCCB0`): the window (group `0x96A006B0`, flags `0x10000005`) is
`0x6A561B3A` for zone 11 (airport), `0x4A562DA5` for 12 (seaport), `0xAA561F93` for 14 (landfill, which also plays
`0x2A62C927`). The dialog runs as `DoQueryDialog` steps 4–11 (§8.12.2) with the handler `SetOrGetControlValue`
(`0x004CC240`), which takes the cell instead of an occupant: ids `0x1011`–`0x1015`, `0x1078`, `0x107B`–`0x107E`
(the airport and seaport rows) and `0x1026`–`0x1028` (the landfill rows) of §8.12.4.

**`DoNeighborConnectionQueryDialog(o)`** (`0x004CC6E0`) *[summary; CT14]*: only for an occupant on the city's edge
(cell x or z equal to 0 or to the city size − 1). The kind comes from the network type (network occupant slot 22
tests; code 2, 3, 5, 6, 7 or 8) or 4 for a power pole. For each of the four edge directions with a valid neighbour
connection cell (`IsValidConnectionCell`) a row is added (cell, row offset `n × 0x200`, kind); one or two rows open a
window by kind: `0xCA566F94` / `0xAA5E14CC` (one / two rows) for kinds 1, 6, 7, 11; `0xEA566A49` / `0x4A5E7ED3` for
kinds 2, 3, 8, 10, 12; a looked-up window for 4 and 5. Its controls `0x104B`–`0x104E`, `0x1066`–`0x106A` (plus the
row offset) show the neighbour city's name and mayor, the congestion, whether a deal exists, its commodity amount,
value, direction and garbage amount.

**The dialog's win proc** (`cSC4QueryDialogWinProc::DoWinProcMessage`, `0x004D3F40`, the proc `0x004C5620` attaches
in `DoQueryDialog` step 4). Only notifications (type 3) from the dialog window are handled:
- `0x287259F6` with a value below 2 ends the dialog (`EndDialog`).
- `0x887113A3` from a slider child: with its value `v`, the money text of id + 0x100 := `trunc(v × 0.01 × c)` (`c`
  from `0x2AA6E8AD`, 0 when missing); with an occupant held, the slider's entry (`0x6A8CC9D6`) runs at once as
  `entry(window, id, false, occupant)`, so the new funding applies while the dialog is open; then each related id
  (`0x0AB3F6DB`) is refilled (fill = true).

### 8.14 Building values *[compat]*

*Added 2026-10-21 (re-query).*

- **Capacity** (`GetOccupantCapacity`, `0x004C4160`): the lot at `o`'s cell; its slot 51 with its occupant wealth (slot
  41); 0 → the building's (`0x87DDA3A9`) slot 11 record's u16 at `+8`. No lot → 0.
- **Occupancy** (`GetCurrentOccupancyValue`, `0x004C53A0`): with the lot's building type (slot 46 with true) non-zero,
  `trunc(GetOccupancy × capacity)` (lot slot 61); else 0.
- **Mayor rating percent** (`GetMayorRatingPercent`, `0x004C5490`): over the cells of the lot's rect (slot 14), `100 ×
  mean(GetAuraValue + 128) / 255` (aura slot 5).
- **Wealth level** (`0x1053`): `w` := the wealth of the lot's building type (building development slot 39), else the
  lot's occupant wealth; the meter is `min(occupantWealth / w, 1)`. Its horizontal resize uses binary32
  `k = 0x3EAAAAAB` (approximately one third); see §8.16.3 for the geometry. Without a lot or a wealth the meter is 1.0.
- **Subjective ratings** (`GetSubjectiveOccupantRatingString`, `0x004D3470`): the lot's centre cell, the best growth
  developer for the lot (`FindBestDeveloper`, `0x004D32D0`) and a rating function. Rating `i` reads a value at the cell
  through one of the developer's desirability curves and compares it with the developer's slot 30
  `GetSubjectiveThresholds(i, min, max)`: below `min` → high, at least `max` → low, else medium.

| `i` | Function | Value |
|---|---|---|
| 0 | `0x004C5C90` | the sum of the curves of pollution types 0, 1 and 3 (pollution slot 5 grids) |
| 1 | `0x004C5D70` | pollution type 2 |
| 2 | `0x004C4750` | the school average grade map (residential slot 39) |
| 3 | `0x004C47F0` | the hospital average grade map (residential slot 40) |
| 4 | `0x004C4890` | the crime grid (crime slot 5) |
| 5 | `0x004C4930` | the commercial traffic map (traffic simulator slot 9) |
| 6 | `0x004C4A30` | the unsigned commute-map byte, with 0 replaced by 255, evaluated through the developer's commute curve (§8.16.4) |

### 8.15 Route mode *[summary; CT15]*

*Added 2026-10-21 (re-query).* A click in mode 1 (`GetOccupantPaths`, `0x004CAC50`) removes the old path view
(renderer slot 33) and, for a building, shows the routes crossing its lot's rect (`GetBuildingOccupantPaths`), for a
network occupant those crossing its cell (`GetNetworkOccupantPaths`). `ShowAllRoutesThatIntersectCityRect`
(`0x004CA460`) builds a `cSC4PathView` (renderer slot 32, thickness by zoom), reads the legend's travel-type toggles
(`0xEBB0F666`, `0x2BB0F616`, `0x4BB0F628`, `0xEBB0F630`, `0x4BB91326`, `0x6BB91318`, `0x2BB9131F`, `0xABB0F639`,
`0x4C081783`, …) and draws each route that crosses the rect (`DoesRouteCrossCell`, `GetRouteTravelTypes`). The
message `0x69247DC7` with a path view clears and redraws it for the same rect (§8.7).

*Route-step filter (CT6).* `DoesRouteCrossCell(route, rect, flag)` (`0x004C4AE0`) first tests the cell held in the route
record (words `+0x20`/`+0x22`, read as its start cell *[inferred]*) against the rectangle (any mode), then walks the compressed route's steps (pathfinder.md §4.15) and accepts the
route at the first step inside the rectangle that the filter `+0x98` lets through: 1 skips steps of travel type 6
(subway), 2 accepts only steps of travel type 6, any other value (3) accepts all. The tool sets the filter before
asking: `GetBuildingOccupantPaths` (`0x004CAA60`) and `GetBuildingOccupantTrafficInfo` (`0x004D0190`) use 1, or 3
when the building is in occupant group `0x1003`; `GetNetworkOccupantPaths` (`0x004CAAE0`) uses 2 when the occupant
is of network type 7 (subway) and 1 otherwise. So surface queries ignore the subway layer, subway queries show only
it, and transport buildings (stations) see every layer *[compat]*.

### 8.16 Query closure supplements (CT14, CT18–CT20) *[compat]*

Added 2026-10-23 (re-query-ct5). These rules supplement the sections above. They close the individual
subcontracts identified here; they do **not** claim complete query-owner closure. The remaining questions
are retained in `questions/city-tools-input.md`. In particular, the traffic-tip body writers must not be
replaced by host calls to the original query owner.

#### 8.16.1 Dispatch and funding refresh lists (CT18)

The occupant filler table really contains three zero pointers for `0x1026`–`0x1028` (table bytes at
`0x00A92520`–`0x00A9252B`). They are not missing names for occupant workers. Keep the separate zone
dispatch described in §8.13. An occupant-table walk must not acquire landfill behavior merely because
its control ID matches a zone worker.

Funding sliders carry these related-ID sequences, in addition to the separate `ID + 2` cost update in
§8.12.4. An empty sequence means no related-ID refreshes, not an instruction to derive IDs from nearby
controls.

| Slider ID | Related IDs, in stored order | Evidence |
|---|---|---|
| `0x1002` (fire) | empty | `0x004C7243`–`0x004C726E` |
| `0x1009` (police) | empty | `0x004C7453`–`0x004C747E` |
| `0x1018` (school staff) | `0x101A`, `0x101B`, `0x101D`, `0x101F` | `0x004C7ADF`–`0x004C7B1E`; data `0x00B096DC`, count at `0x00B096EC` |
| `0x1019` (school bus) | empty | `0x004C7C2D`–`0x004C7C55` |
| `0x1021` (hospital staff) | `0x1023`, `0x1024`, `0x00000004`, `0x3F800000` | `0x004C7F6F`–`0x004C7FAE`; data `0x00B096F0`, count at `0x00B096F8` |
| `0x1022` (hospital ambulance) | empty | `0x004C80AD`–`0x004C80D5` |

For hospital staff the count is four, not two. Reading four dwords from the supplied list includes its
own count word followed by a binary32-one word. Compatibility therefore requires retaining those two
additional **integer IDs**, without interpreting the fourth as a floating-point value or substituting
plausible hospital controls. Their eventual refresh follows the normal lookup/dispatch rules of §8.13.
The empty-cell sound conflict is still CT18; this list correction does not settle that branch.

#### 8.16.2 Best developer selection (CT19)

`FindBestDeveloper` (`0x004D32D0`) selects a developer for a lot, using the lot's centre cell. It distinguishes
a lot with a nonzero building type (lot slot 46 with false) from an undeveloped lot.

For a nonzero building type, obtain its building-development record (slot 39) and the building wealth
(`GetBuildingWealth`, `0x004D3260`). A nonzero purpose in the record determines the developer type through
building-development slot 30 `(purpose, wealth)`; slot 32 retrieves that developer. A zero purpose returns
no developer. This path does not compare all wealth classes or choose by demand.

For a zero building type, the centre cell's zone determines this ordered candidate list. Other zone
types return no developer.

| Zone type | Developer types in comparison order | Static list |
|---|---|---|
| 1, 2, 3 | `0x1010`, `0x1020`, `0x1030` | `0x00B09780` |
| 4, 5, 6 | `0x3110`, `0x3120`, `0x3130`, `0x3320`, `0x3330` | `0x00B09768` |
| 7 | `0x4100` | `0x00B09760` |
| 8 | `0x4400`, `0x4300` | `0x00B09754` |
| 9 | `0x4200`, `0x4400`, `0x4300` | `0x00B09744` |

Rank candidates by their signed 16-bit desirability at the centre cell (developer slot 29), then by
the demand value from the developer's slot-18 object, that object's slot 12, converted by truncation
toward zero. The second comparison uses the converted signed integer, so fractional differences that
truncate to the same integer do not break a tie. Exact ties retain the earlier candidate. The initial
comparison pair is `(-1000, -1000)`, with no selected developer: a candidate must beat that pair to be
selected. Thus even an available candidate can leave the result empty. Evidence: `0x004D33D0`–`0x004D342B`,
including sign extension at `0x004D33EE` and the conversion at `0x004D33FF`. The normal-domain conversion
rule here assumes a finite representable signed result; exceptional values remain CT19.

#### 8.16.3 Wealth meter size and alignment (CT19)

The resize factor is `min(binary32(0x3EAAAAAB) × w, 1)`, stored as binary32. The displayed meter fraction
is independently `min(occupantWealth / w, 1)`. The size factor uses **building wealth**, even when the
occupant wealth has fallen. Both wealth values are treated as unsigned values for floating conversion.
Evidence: `0x004D3552`–`0x004D35A8`, constant at `0x00A92480`.

The control's window rectangle retains its right, top and bottom edges. Given reference left edge `L`,
right edge `R`, and size factor `s`, its new left edge is
`L + trunc((R − L) − s × (R − L))`. This is a right-aligned bar: it is not centred and its right edge
does not move. The control uses property `0x8ADC29D4` to retain the reference left coordinate across
refreshes; refreshing the same wealth must not repeatedly shrink an already shortened bar. The property
and rectangle operations occur on the meter control's window (`0x004D3618`–`0x004D36DA`). Geometry conversion
uses truncation toward zero (`0x004D36A1`–`0x004D36C8`). If the meter interface is unavailable, no geometry
is changed. No lot or zero building wealth uses the full-value meter fallback described in §8.14.

#### 8.16.4 Commute sampling (CT19)

Subjective commute uses traffic simulator (`0x00B43DA8`) slot 13 to obtain a grid and grid slot 7 at the
requested cell. Interpret the returned byte as unsigned; replace **zero** with 255. It is not a signed
commute value and zero is not the best possible commute. Convert that substituted byte to binary32,
evaluate the developer's curve at `+0x80`, and truncate its output toward zero before comparing it
with the subjective thresholds for rating 6 (developer slot 30).

Below the lower threshold the rating index is 2 and the auxiliary value is the negated truncated curve
output. At or above the upper threshold the index is 0 and the auxiliary value is that output divided
by two with integer truncation toward zero. Otherwise the index is 1. The absent-curve path also returns
the medium index; no grid sample is needed on that path. Evidence: `0x004C4A30`–`0x004C4AC5`, especially
the zero substitution and unsigned extension at `0x004C4A71`–`0x004C4A89`. Rating labels remain those of
§8.14; the auxiliary value is not an alternative label selector.

#### 8.16.5 Curve endpoints and airport exemplar (CT19)

For an already constructed response curve with finite, strictly increasing input knots, evaluation
clamps below the first knot to its first output, and at or above the last knot to its last output.
Interior evaluation uses the segment containing the input, with the segment's stored binary32 slope:
`previousOutput + (input − previousInput) × slope`. It does not extrapolate outside the endpoints.
One knot gives a constant output. The evaluator uses x87 operations on the stored binary32 operands;
it does not round the intermediate difference/product to binary32 after every operation. Evidence:
`cSC4ResponseCurve::GetOutputValue`, `0x005F4250`–`0x005F42D4`, especially `0x005F4287`, `0x005F42B0` and
`0x005F42BA`–`0x005F42CF`. Property parsing, slope construction, duplicate/unsorted knots and an empty
property still need verification; this evaluator contract does not prescribe a parser repair policy.

Lost luggage's three constants are one resource key **(type, group, instance)**:
`(0x6534284A, 0x67CD5FA1, 0xCA270E9D)`. They are not three airport instances to choose between by airport
size or traffic. `SetOrGetAirportLostLuggage` (`0x004CC160`) obtains that resource with interface
`0xA52160F5`, uses its property holder, and evaluates property `0xAA60E745` through the goofy-stat helper.
A failed resource lookup returns failure; it does not choose another exemplar. The caller supplies the
same fixed key for every airport. The occupant wrapper merely obtains the cell coordinates and forwards
to this worker. Airport-noise arithmetic and the exceptional numeric cases remain CT19.

#### 8.16.6 Neighbour connection kinds and dialog resources (CT14)

The connection kind is a neighbour-simulator kind, not the network type number. For a network occupant,
the applicable mappings below are tested in the shown priority order: **the last successful test wins**.
The initial kind is 1. If network QI fails, QI `0xA8C86142` chooses kind 4 when successful.

| Network type tested | Connection kind |
|---|---|
| 11 | 10 |
| 0 | 2 |
| 6 | 8 |
| 12 | 12 |
| 2 | 3 |
| 7 | 6 |
| 8 | 7 |
| 9 | 11 |
| 1 | 1 |
| 4 | 5 |

Evidence: `0x004CC77C`–`0x004CC87E`. This matters for cells supporting more than one network type.
The dialog includes one or two valid connection records. Zero or at least three records produces no
dialog. Resource group is `0x96A006B0`, type is 0, and instance depends on the **number of records**:

| Connection kind | One record | Two records |
|---|---|---|
| 1, 6, 7, 11 | `0xEA566A49` | `0x4A5E7ED3` |
| 2, 3, 8, 10, 12 | `0xCA566F94` | `0xAA5E14CC` |
| 4 | `0x4A566C14` | `0x8A5E7BD2` |
| 5 | `0x4A566D6E` | `0x2A5E7490` |

Kinds 4 and 5 do not perform a special global-window lookup. Their helper `0x00411D50` counts the same
connection list used for the other kinds; its result distinguishes the one-record and two-record
scripts (resource selection in `0x004CC6E0`). This corrects the earlier
"looked-up window" uncertainty, not the still-open row-filler contracts.

#### 8.16.7 Debug-query source and writer ordering (CT20/CT14)

`IsDebugQueryEnabled` (`0x004C4700`) is a live conjunction of the application's debug-functionality
setting and all three keys Shift (`0x10`), Ctrl (`0x11`), Alt (`0x12`). The application service is held
at `0x00B43C94`; slot 21 is `cSC4App::GetDebugFunctionalityEnabled` (`0x004494C0`), which reads the byte
at application offset `+0x644`. Obtain the keyboard from window-manager slot 38 and test key state with
keyboard slot 3. Debug-functionality alone does not enable a query, and holding the three keys does not
override a disabled setting. Evidence: `0x004C4700`–`0x004C474F`, `0x004494C0`–`0x004494C6`.

The building debug branch of `DescribePick` is not a call to `DescribeBuilding`: its immediate action
creates/destroys residential query data. The later **cell** phase finds a building in the selected cell
and there calls `DescribeBuilding`, `DescribePopulation`, then `DescribeDemand`, in that order. The
population and demand texts form `extra` in population-before-demand order. EQ/HQ follows, then
`DescribeCell`. Preserve this distinction: a building pick does not by itself mean the final `text`
is a building description. See §8.12.1 for the final three output-string formats. Evidence:
`0x004D4B0F`–`0x004D4C8B`, enclosing `DescribePick` at `0x004D4140`.

The helper bodies remain incomplete contracts. Verified content inventories, useful for checking future
closure, are: `DescribeBuilding` (`0x004CCF60`) reports capacity and wealth, power consumed/produced,
incinerator capacity/use, waste-to-energy capacity/use/max power, water-treatment capacity/use, water
consumed/produced and age degradation in that order, omitting inapplicable service branches.
`DescribeDemand` (`0x004CD820`) chooses residential, commercial or industrial output by the cell's zone
family, ordering wealth/type rows as R$/R$$/R$$$, Cs$/Cs$$/Cs$$$/Co$$/Co$$$, and Ir/Id/Im/Iht respectively.
`DescribePopulation` (`0x004CF2B0`) walks the lot's developer-keyed occupancy map in key order and emits
entries of the form `%s %.0f%% (%.0f%%), `. Exact value sourcing and all labels are still CT14/CT20.
`DescribeCell` and the `Query.txt` writer likewise remain open; these inventories must not be treated
as complete specifications for their bodies.

#### 8.16.8 Additional conformance vectors

These vectors apply to the verified supplements above; none substitutes for the unresolved writer or
traffic-tip tests.

| Case | Primitive inputs | Expected behavior |
|---|---|---|
| QCT5-1 | Occupant filler walk reaches `0x1026`, `0x1027`, `0x1028` | No occupant filler called; zone dispatch remains separate. |
| QCT5-2 | Hospital staff slider initialization | Store four IDs in order: `0x1023`, `0x1024`, `4`, `0x3F800000`; do not invent hospital controls for the last two. |
| QCT5-3 | School bus or hospital ambulance slider initialization | Related sequence empty; the separately specified cost update still occurs. |
| QCT5-4 | Undeveloped residential lot; candidates have pairs `(20,10.9)`, `(20,10.1)`, `(19,999)` | Choose `0x1010`: the first two demands both truncate to 10; the third loses on desirability. |
| QCT5-5 | Same lot; pairs `(20,10.9)`, `(20,11.0)`, `(19,999)` | Choose `0x1020`. |
| QCT5-6 | All candidate desirabilities below −1000 | No developer selected, regardless of demand. |
| QCT5-7 | Empty zone 8 lot; exact equal eligible pairs for `0x4400`, `0x4300` | Select `0x4400`, preserving candidate order. |
| QCT5-8 | Meter reference rectangle `(10,20,100,30)`, building wealth 1, occupant wealth 1 | Right edge 100 and vertical bounds 20/30 retained; left becomes 69 with the stored binary32 size factor; displayed fraction 1. Repeat refresh retains left 69. |
| QCT5-9 | Same rectangle, building wealth 3, occupant wealth 1 | Full reference width; displayed fraction approximately one third. |
| QCT5-10 | Commute-map samples 0, 128, 255 | Curve inputs 255, 128, 255 respectively; 128 is not converted to −128. |
| QCT5-11 | Curve knots `(0,0)`, `(10,100)`, `(20,0)` and slopes 10, −10 | Inputs −5, 0, 5, 10, 15, 20, 25 yield 0, 0, 50, 100, 50, 0, 0. |
| QCT5-12 | Lost-luggage queries for two different airport sizes | Both request the same complete key `(0x6534284A,0x67CD5FA1,0xCA270E9D)`. |
| QCT5-13 | Network supports types 0 and 4; one valid neighbour connection | Kind 5 wins; script instance `0x4A566D6E`. |
| QCT5-14 | Kind 4 with two valid records | Script instance `0x8A5E7BD2`; no special window lookup. |
| QCT5-15 | Debug setting off and all three keys held; then setting on with only Shift+Ctrl; then setting on with all three keys | Disabled, disabled, enabled. |
| QCT5-16 | Cell readout finds a building | Writer order building, population, demand, EQ/HQ, cell; population precedes demand in `extra`. |
