# The city-screen window `cSC4View3DWin`: lifecycle, interfaces and event dispatch (behaviour spec)

Status: M6 behaviour spec, written 2026-10-21 (re-view3dwin4 lane). Every fact was read on the Windows executable
`bin/SimCity4.exe` (1.1.641, image base `0x00400000`) through `tools/re_query.py` (export) and the class's
`decomp/` file; numeric facts were checked on the instructions and carry addresses. REA was unavailable to this lane
("No app is open"), so no fresh pseudocode was taken. Implementers work from this spec, the LGPL gzcom-dll SDK
headers and general knowledge only, never from decompiled code.

Tags: *[compat]* observable, must match; *[inferred]* consistent with the evidence, not proven; *[quirk]* original
behaviour reproduced as is.

Scope. This spec owns the **window object's lifecycle and event routing**: its four interface sub-objects and their
tables, construction, `Init`/`Shutdown`, the cheat and command-name registrations, the MessageServer2 subscription
set and how each notification is routed, and the `cIGZWin::DoMessage` (window message) routing. It does **not**
restate:

- the object layout around the catalog menus, the save record or the 61-slot `cISC4View3DWin` table: see
  [ui-windows.md](ui-windows.md) §3–§4;
- the paint path (`GZPaint`, slot 88 of the window table) and the draw-relevant fields: see [view3d.md](view3d.md) §5;
- the terrain's 3D view, the DX7 driver and the particle/effects layer (terrain.md §15.5, renderer-driver.md,
  view3d.md §8).

## 1. Identity and interfaces *[compat]*

One `cSC4View3DWin` object carries four vtable pointers. Construction (`0x007F0A70`) writes them at
`0x007F0A9D`–`0x007F0AB1`:

| Offset | Interface | Vtable | Slots | How it is reached | Evidence |
|---|---|---|---|---|---|
| `+0x00` | `cISC4View3DWin` (IID `0xFA47B3F9`), also answers IID `1` (`cIGZUnknown`) | `0x00ABCE40` | 61 | `QueryInterface` returns `this` | QI `0x007E5630`; ui-windows.md §3.3 |
| `+0x04` | `cIGZMessageTarget2` (IID `0x452294AA`) | `0x00ABCE2C` | 4 | QI returns `this+4` | slots 0–2 are thunks (`this-4`) to QI/AddRef/Release; slot 3 is the notification handler `0x007F53C0` (§5) |
| `+0x08` | window message filter (IID `0xC6AE7085`) *[inferred: `cIGZWinMessageFilter`]* | `0x00ABCE1C` | 4 | QI returns `this+8` | slots 0–2 thunks (`this-8`, e.g. `0x007F10D0`); slot 3 `0x007F58E0` (§6.3) |
| `+0x0C` | `cIGZWin` (IID `0x22BA0121`), a `cGZWin` base | `0x00ABCBC0` | 151 | QI returns `this+0xC`; also `AsIGZWin` (slot 3 of `0x00ABCE40`) | ctor `0x007F0AB1`; ui-windows.md §3.2 |

`QueryInterface` (`0x007E5630`) answers exactly these five IIDs (`1`, `0xFA47B3F9`, `0x452294AA`, `0xC6AE7085`,
`0x22BA0121`) and returns false for anything else; it does **not** fall back to `cGZWin`'s own QI, so asking a
city-screen window for any other window-family IID (e.g. a `cIGZWinGen` IID) fails. Every success AddRefs through
`+0`. `AddRef`/`Release` (`0x007E56D0`/`0x007E56E0`) forward to the `cGZWin` sub-object's count: the object has one
reference count, the window's.

The three small tables are not in `docs/abi/vtables.tsv`; this lane read them as raw data (`0x00ABCE1C` =
`007F10D0 007F10E0 007F10F0 007F58E0`; `0x00ABCE2C` = `007F10A0 007F10B0 007F10C0 007F53C0`, followed by a zero
dword). Proposed rows: `build-runs/abi-rows-re-view3dwin4.tsv`.

### 1.1 The window table `0x00ABCBC0`: slots the class overrides *[compat]*

All other slots of the 151 are inherited `cGZWin` code.

| Slot | Offset | Address | Role |
|---|---|---|---|
| 0–2 | `+0x0`–`+0x8` | `0x007F1100`/`0x007F1110`/`0x007F1120` | thunks (`this-0xC`) to QI / AddRef / Release |
| 3 | `+0xC` | `0x007F3E50` | window `DoMessage` (§6) |
| 4 | `+0x10` | `0x007E9900` | `Init` (§3) |
| 5 | `+0x14` | `0x007F1320` | `Shutdown` (§4) |
| 88 | `+0x160` | `0x007F6200` | `GZPaint` (view3d.md §5.1) |
| 129 | `+0x204` | `0x007E6600` | `GZOnCharacter` |
| 134–139 | `+0x218`–`+0x22C` | `0x007E68D0` `0x007E6A00` `0x007E8170` `0x007E82E0` `0x007E9460` `0x007E6B50` | mouse down L/R, up L/R, move, wheel (city-tools-input.md) |
| 142 | `+0x238` | `0x007E96E0` | `GZOnMouseExit` |
| 143 | `+0x23C` | `0x007E5BA0` | `GZOnCommand` |
| 148 | `+0x250` | `0x007F1130` | deleting destructor thunk (`this-0xC` → `0x007F3E30`) |

## 2. Construction (`0x007F0A70`) *[compat]*

Order of effects:

1. The four vptrs are first set to pure-call stubs (`0x00ABC9C0`, `0x00A81174`, `0x00A86864`), the `cGZWin` base is
   constructed at `+0x0C` (`0x0099D938`), then the four final tables of §1 are stored.
2. All owned pointers are nulled (renderer, camera control, model maker, view manager slots, the catalog-menu fields
   `+0x1F8`…`+0x208`, the panel window procs, the occupant filter at `+0x320`, the progress screen).
3. Non-zero defaults (absolute offsets, read on the instructions of the ctor; meanings *[inferred]* where marked):

| Offset | Default | Meaning |
|---|---|---|
| `+0x1F4` | `0xFFFFFFFF` | "no selection" sentinel *[inferred]* |
| `+0x210`, `+0x218`, `+0x220` | `1`, `4`, `7` | three small counters/limits *[inferred]* |
| `+0x278` … `+0x2C8` | `0x8C329937`, `0x31`, `0x29`, `0x299237BF`, `0x6A33E1BA`, `0x2A33E1A3`, `0xEA353064`, `0x2A312F97`, `0x0A5242DF` (each followed by a zero dword) | a table of resource/ID pairs (`{id, 0}`) used by the GUI *[inferred]* |
| `+0x324` | `1` | (written twice: 0 then 1 at `0x007F0FB4`) |
| `+0x334` | `1.0f` | a scale factor *[inferred]* |
| `+0x340`, `+0x344` | `0.2f`, `0.2f` | two tuning terms *[inferred]* (not the ones focus loss zeroes) |
| `+0x368`, `+0x370` | `5`, `0x15200003` | *[inferred]* |
| `+0x380` | a `cSC4RenderProperties` sub-object, constructed in place | render properties (owner class cSC4RenderProperties) |
| `+0x400` | `-1` | *[inferred]* |
| `+0x408` | `1` | *[inferred]* |

4. Two empty intrusive lists are allocated (12-byte and 60-byte heads, each pointing at itself) and a 100-bucket
   `hash_set<uint32>` is constructed.
5. The `cRZAutoMsg2Notification` member is told its message set: **33 message types** from the table at
   `0x00ABC070` (the call `AddMessageTypes(0x00ABC070, 0x84)` at `0x007F0FBE`; `0x84` bytes = 33 ids *[inferred]*
   that the argument is a byte length). The set is in §5.
6. `SL::OnView3DCreate(this)` (`0x00602120`) is called last: the scripting layer learns the window exists before
   `Init`.

## 3. `Init` (window slot 4, `0x007E9900`) *[compat]*

`Init` runs once: it returns at once if the `cGZWin` init flag is already set. Otherwise, in order:

1. **Notification subscription**: the auto-notification member registers the object (as `cIGZMessageTarget2`).
2. **Command names**: the framework is asked (`0x007E9959`, slot 5 with class `0xCB90253A`, IID `0xEB903A32`) for a
   command-name registry and given the table at `0x00B0DD98` with the argument `0x68` (slot 7). The table holds
   records `{command id, name, 0}`; the first nine are `0x6A935C34 ToggleGodPanel`, `0x6A935CA4 ToggleMayorPanel`,
   `0x6A935CA8 ToggleMySimPanel`, `0x6A935CAB ToggleOptionsPanel`, `0x6A935CAD SimSpeedPause`, `0x6A935CB0
   SimSpeedSlow`, `0x6A935CB2 SimSpeedMedium`, `0x6A935CB5 SimSpeedFast`, `0x6A935CB7 ZoomIn`. These are the names a
   key-configuration file binds keys to. *[inferred]* `0x68` is the record count (104); the table was not walked
   to its end (VW3).
3. **Cheat registration** (only when the cheat manager global `0x00B43CAC` exists): the window registers itself as a
   cheat listener (slot 5), then registers these cheat strings with their IDs (slot 3, `(id, string)`), in this
   order:

| ID | Cheat text | Registered |
|---|---|---|
| `0x8973A69C` | `GOL` | always |
| `0xCA7BC5B5` | `SizeOf` | always |
| `0xAA7A2746` | `TastyZots` | always |
| `0x6A7A276E` | `Zoneria` | always |
| `0x8AB730AE` | `TerrainQuery` | always |
| `0x88F3D4BB` | `RenderProp` | always |
| `0x88F3D4BC` | `RP` | always |
| `0x6976AFA0` | `Recorder` | always |
| `0x4A972F37` | `WatchMeMove` | always |
| `0x4A82253E` | `You don't deserve it` | always |
| `0x4AC096C6` | `LotPlop` | only if the app object `0x00B43C94` slot 21 (`+0x54`) returns true (`0x007E9B85`) |
| `0x4AC096CA` | `BuildingPlop` | same condition |
| `0x0BFD8064` | `toll` | same condition |

   The pairing is read from the instruction order (`0x007E998C`–`0x007E9C06`: each string load is followed by its
   ID push). Cheat text is matched by the cheat manager, not here; what each cheat does is `ProcessCheatCode`
   (`0x007F3510`), reached through the window message `0x230E27AC` (§6); per-cheat effects are §3.1.
4. `cGZWin::Init` runs (`0x007E9C27`); its bool result is stored at `+0xE4` and is `Init`'s return value.
5. The window names itself `"cSC4View3DWin"` (`0x007E9C2E`), sets its window ID class `0x9A47B417` (`0x007E9C62`,
   the class ID by which `GetChildAs(0x9A47B417, 0xFA47B3F9, …)` finds it, landfill.md §3.2), sizes itself to its
   parent's client area, and becomes visible/enabled through `cGZWin` slots.
6. Cursor: through the cursor manager, cursor `0xC16C4E1B` is set as the window's cursor (`0x007E9CB2`).
7. The coverage-radius helper at `+0x3D8` is initialised, a byte at `+0x150` cleared, and a `cRZTimer` (the scroll /
   frame timer *[inferred]*) is started (`0x007E9CDD`).
8. Two GZCOM lookups: object `0x42E3EA4B` and class `0x42E967BE` / IID `0xE2C1B3C4` (stored at `+0x114`); the latter
   is then given the window (slot 3). *[inferred]* these are the camera/view-input helpers; identity open (VW2).
9. `+0x119` := 1.
10. If the app's current city record (app slot 39 `+0x9C`, field `+0x204`) is non-zero, the progress screen is
   created (`CreateProgressScreen`, `0x007E5BB0`) — a city is being loaded.
11. `0x00562DD0` is called last.

### 3.1 What each cheat does (`ProcessCheatCode`, `0x007F3510`) *[compat]*

**How it is called.** The window `DoMessage` (§6) handles message `0x230E27AC`: it takes the message's string
(`data2`, string slot 5), splits it into a command line (`cRZCmdLine`, `0x007F40D4`) and calls
`ProcessCheatCode(cheat ID = data1, command line)` on the object's `+0` sub-object (`0x007F40E2` passes `this − 0xC`).
The `bool` result is the window message's result (`0x007F40EE`), so `false` means "not consumed". In the command line,
argument 0 is the cheat word itself; `argc` (command-line slot 3, `+0xC`) counts it, and argument *i* is command-line
slot 16 (`+0x40`). The dispatch is a compare tree on the unsigned cheat ID (`JA`/`JZ`/`JC`, `0x007F3517`–`0x007F3888`);
an ID not in the table returns `false` (`0x007F39C0`).

**Installing an input control** (four cheats) means: allocate the control, hold one reference, call the window's own
`SetCurrentViewInputControl(control, 2)` (slot 25, `+0x64`; the same call the catalog makes, city-tools-input.md §3.1),
release the reference, return `true`. If the allocation fails the call is still made with a null control.

| Cheat | ID | Effect | Returns |
|---|---|---|---|
| `GOL` | `0x8973A69C` | installs a new `cSC4ViewInputControlGameOfLife` (0x88 bytes, ctor `0x004BC310`) | true |
| `SizeOf` | `0xCA7BC5B5` | needs `argc ≥ 2`. `v := atof(argument 1)` (`RZAtofA`, stored as a float). If `v < 0.9f` (`0x00A88218`) then `v := 0.9f`, else if `v > 64.0f` (`0x00A9552C`) then `v := 64.0f` (equal to a bound is kept). Calls the camera control's (`+0xF0`) `0x007CD6E0` with `v` (below) | true; false when `argc < 2` |
| `TastyZots` | `0xAA7A2746` | new occupant filter (below), flags: `+0xD` := **not** old `+0xD`, `+0xE` := old `+0xE`, `+0xC` := old `+0xC`; `SetCurrentViewFilterInfo` | **false** *[quirk]* |
| `Zoneria` | `0x6A7A276E` | new occupant filter, flags: `+0xC` := **not** old `+0xC`, `+0xD` := old `+0xD`, `+0xE` := 0 (not copied) *[quirk]*; `SetCurrentViewFilterInfo` | **false** *[quirk]* |
| `TerrainQuery` | `0x8AB730AE` | byte `+0x126` := 1. If `argc ≥ 2` and argument 1, lower-cased (`MakeLower`), equals `"off"` (`0x00A82B94`), `+0x126` := 0. `+0x126` is what `GetTerrainQueryEnabled` (slot 39, `0x007E5900`) returns | true |
| `RenderProp`, `RP` | `0x88F3D4BB`, `0x88F3D4BC` | `argc ≥ 3`: `SetRenderProperties(argument 1, argument 2)` (`0x007ECDA0`). `argc = 2`: `SetRenderProperties(argument 1, "")`. `argc < 2`: shows the text `"Cheat code usage: renderprop <property name> [0|1] [true|false]"` (`0x00ABCF70`) through the window's slot 45 (`+0xB4`; an empty stub in this build, `0x00648F10`, so nothing is shown) | true; false when `argc < 2` |
| `Recorder` | `0x6976AFA0` | `AddWinAnimationRecorder(true)` (`0x007F0840`, below) | true |
| `WatchMeMove` | `0x4A972F37` | byte `+0x124` := 1. Nothing clears it here | true |
| `You don't deserve it` | `0x4A82253E` | byte `+0x446` := 1. Nothing clears it here | true |
| `LotPlop` | `0x4AC096C6` | installs a new `cSC4ViewInputControlPlaceLot` (0x158 bytes, ctor `0x004C0430`) built with arguments `(0, 0, 0, 0)` | true |
| `BuildingPlop` | `0x4AC096CA` | the same control built with `(1, 0, 0, 0)`: the first argument is the only difference | true |
| `toll` | `0x0BFD8064` | installs a new `cSC4ViewInputControlTollBooth` (0x38 bytes, ctor `0x004D9E80`) | true |

No cheat reads its arguments beyond those listed: extra words are ignored.

**The two filter cheats** (`TastyZots` `0x007F391F`, `Zoneria` `0x007F365D`). Both build a fresh 16-byte
`cSC4RealisticOccupantFilter` (vtables `0x00A88084`/`0x00A8806C`, flag bytes `+0xC`/`+0xD`/`+0xE` all 0; `TastyZots`
through `0x004666C0`, `Zoneria` inline with the same stores) and take the current filter from `+0x320` (the field
`GetCurrentViewFilterInfo`, slot 18, returns). The old filter's flags are copied (as in the table) only when it answers
`QueryInterface(0xE4FDA3D4)` and that interface's slot 5 returns `0x8A7A1C2C` (the realistic filter's type,
occupant-manager.md filter table); otherwise the new filter keeps all three flags at 0. The new filter is handed to
`SetCurrentViewFilterInfo` (slot 19, `+0x4C`) and the cheat's references are released. Each use therefore toggles one
flag: `TastyZots` toggles `+0xD`, `Zoneria` toggles `+0xC` and clears `+0xE`. What the flags hide is the filter's
business (occupant-manager.md, `cSC4RealisticOccupantFilter`: `+0xD` = 0 hides holders with property `0x5001`, `+0xC`
gates lot base textures, `+0xE` = 1 shows every prop). Both branches end on the shared `return false` (`0x007F39A8` →
`0x007F39C0`), so the cheat message counts as unconsumed although the filter changed *[quirk]*. If the 16-byte
allocation fails and the old filter is realistic, the original writes through a null pointer (crash); a replacement
may skip the cheat instead.

**`SizeOf` and the camera control (`0x007CD6E0`).** The camera control stores the new value in its float `+0xF0` only
when the value differs from the current one **and the current one** lies strictly between 0.0 (`0x00A81054`) and
1000.0 (`0x00A8FAF0`): the bounds test the old value, not the new one. When it stores and its `+0xC` is non-null, it
recomputes its zoom/rotation and position parameters, updates the view utilities (global `0x00B43DD8`), refreshes the
scroll bounds when `+0x14C == +0x150`, and posts message `0xA6B79621` (PostViewRefresh) carrying its `+0x100` and
`+0x110` vectors. With the cheat's clamp the stored value is always in [0.9, 64].

Example: `SizeOf 100` stores 64.0; `SizeOf 0.5` stores 0.9; `SizeOf abc` stores 0.9 (`atof` gives 0); `SizeOf`
alone does nothing and returns false.

**`Recorder` (`AddWinAnimationRecorder(bool add)`, `0x007F0840`).** Looks for a child window with ID `0xA802B4EB`
(`cIGZWin` slot 34). With `add` true and no such child: creates a `cSC4WinAnimationRecorder` (0x218 bytes, ctor
`0x00796380`), sets its output directory to the user-data directory (`SL::UserDataDirectory`) + `"RecordedAnimations"`
+ `"\"` (`SetAnimationOutputDirectory(dir, 1)`, `0x00795100`), gives it window ID `0xA802B4EB` and adds it as a child
(`cIGZWin` slot 14). With `add` true and the child present: nothing. With `add` false: removes the child if present
(slot 15). Always returns true. The cheat only ever passes true, so a second `Recorder` is a no-op.

## 4. `Shutdown` (window slot 5, `0x007F1320`) *[compat]*

1. Unregisters the cheat listener and **all thirteen** cheat IDs of §3, including `LotPlop`, `BuildingPlop` and
   `toll` even when they were never registered *[quirk]* (`0x007F1383`–`0x007F141A`). A replacement's cheat manager
   must tolerate unregistering an unknown ID.
2. Calls its own `RemoveAllViewInputControls` (slot 27, `+0x6C`) and `RemoveAllPersistentViewInputControls` (slot 30,
   `+0x78`): no input control survives the window.
3. Releases the helper objects, then `RemoveGUI` (`0x007EF200`), then `cSC4ProgressScreen::Shutdown` if a progress
   screen exists, then unsubscribes from the command-name registry (`0xCB90253A`).

## 5. Notifications (`cIGZMessageTarget2::DoMessage`, `0x007F53C0`) *[compat]*

The handler always returns true. First rule: if the message answers `QueryInterface(0x4B99446A)` (a command
message), the type and its two data values are passed to the window's own `ProcessCommand` (slot 7, `+0x1C`) and
nothing else happens. This is how the nine-plus command names of §3 step 2 reach the window. Otherwise by type:

| Type | Name (from other specs) | Effect |
|---|---|---|
| `0x26D31EC0` | PreCityInit | `DoPreCityInit` (`0x007F41B0`) |
| `0x26D31EC1` | PostCityInit | `DoPostCityInit` (`0x007ED0F0`) |
| `0x26D31EC2` | PreCityShutdown | `0x007E9780` |
| `0x26D31EC4` | CityEstablished | `DoCityEstablished` |
| `0x26D31EC5` | CityObliterated | `DoCityObliterated` |
| `0x26C63341` | load | `DoLoad(data)` (ui-windows.md §3.3a) |
| `0x26C63343` | (pre-load/clear) | `RemoveAllViewInputControls(0)` (slot 27) |
| `0x26C63344` | save | `DoSave(data)` |
| `0x0AB99380` | city name changed *[inferred]* | `UpdateCityName` |
| `0x0772FAD4`, `0x08DAC3E7`, `0x08DAC3E8` | FundsChanged and the two budget dirty messages | `UpdateBudget` |
| `0x06CDB65B` | view level changed | if the renderer (`+0xF4`, `GetRenderer`) exists, its `cISC43DRender` slot 20 (`+0x50`) (`0x007F5455`) |
| `0x44778231` | 24-hour clock tick | `Update24HourClock(data, data)` |
| `0x44778233` | VisualTimeTick | `UpdateAmbientLightLevel` |
| `0x66956814` | simulation date tick *[inferred]* | `UpdateSimulationClock`, then `UpdatePopulation` |
| `0x66956816` | mayor rating changed *[inferred]* | `UpdateMayorRatingBar` |
| `0x4A7FB7E9` | speed changed | `UpdateSpeedButtons` |
| `0x4A7FB7E2` (HiddenPause change), `0xAA7FB7E0` (SimPauseChange), `0xABBBCA64` | pause / mission state | §5.1 |
| `0xA6B79602`, `0xA6B79612` | view update / ViewTChange3 | §5.2 |
| `0xA6B79621` | PostViewRefresh | `DoTemporarySimHiddenPause` |
| `0xCA1B1E64` | DisasterPeriodStarted | if the secondary menu kind (`+0x200`, ui-windows.md §3.2) is `0x49923239`, close the secondary menu (slot 59) (`0x007F5861`); then as `0xAA1B2002` |
| `0xAA1B2002` | DisasterPeriodEnded | child window `0x8991EE08`, its slot 68 (`+0x110`) with `(2, type == 0xAA1B2002)`; then `UpdateAlertBorder` |
| `0xAA5CF1E3` | EndGameForBudgetFailure | if modal popups are safe now (`SC4GetPopupModalDialogsSafe`): `DoBudgetFailure`; otherwise the message is queued once (a list of deferred messages; a second copy of the same type is not added) |
| `0xAC083D7D` | mission target reached | `UpdateMissionProgress(data1, 1)` |
| `0x4A44667F` | progress report *[inferred]* | if the message carries interface `0x089B7DC8` and a progress screen exists, forward it to the progress screen (`0x00777640`) |

The four types `0x6A935C34`, `0x6A935CA4`, `0x6A935CA8`, `0x6A935CAB` close the 33-entry set; they are command
messages and take the first rule.

### 5.1 Pause and mission notifications

For `0x4A7FB7E2`, `0xAA7FB7E0` and `0xABBBCA64`: if a "paused" overlay object is present it is updated from the
simulator's pause state (overlay object `0xC998BB81`, shown when paused at speed 1000 *[inferred]*); then
`UpdateSpeedButtons` and `UpdateAlertBorder`. For `0xABBBCA64` only, `UpdateMissionProgress(data1-derived, data1)`
follows (the mission activation flag from city-situation.md).

### 5.2 View-update notifications

`0xA6B79602` and `0xA6B79612` act only while the app's "city view hidden" byte (`+0x39C` of the object at
`0x00B43CC4`+`0xC`) is zero. `0xA6B79602` first runs `UpdateZoomButtons` and `AddBackgroundImageToRenderer`; both
types then run `DoTemporarySimHiddenPause`, and if the cursor-text flag is set the cursor text is re-placed at the
current mouse position.

## 6. Window messages (`cIGZWin::DoMessage`, `0x007F3E50`) *[compat]*

Every message is first offered to `nsSC4TutorialSystem::FilterUICommandMessage` (tutorial.md). Then:

| Window message | Effect | Result |
|---|---|---|
| `0x230E27AC` (cheat) | `ProcessCheatCode(data1 = cheat ID, command line split from data2)` (§3.1) | its result |
| 3 (command), data1 `0x49D993D4` | the window takes keyboard focus, then `InvokeCatalogItemCommand(data2, data3)` | true |
| 3, data1 `0x287259F6` (button) | not handled here (the filter, §6.3, owns buttons) | base `cGZWin` |
| `0x13` (mouse enter) | hover byte `+0x118` := 1 (`0x007F402F`), `Update24HourClockHover`, base | base |
| `0x14` (mouse leave) | hover byte `+0x118` := 0, `Update24HourClockHover`, base | base |
| `0x0D` | `Update24HourClockHover`, base | base |
| `0x10` (focus lost) | only when global `0x00B43CEC` is non-zero: the four scroll-velocity dwords `+0x348`…`+0x354` are zeroed (`0x007F4087`–`0x007F4099`; the handler's `this` is `+0x0C`), `RecalcScrollVelocity`, `ScrollStop` (slot 34), and a `cGZWin` slot 142 call (`GZOnMouseExit`) | true |
| `0x1B` with sub-type 5 / 6 | key down / up: `ProcessKeyDown` / `ProcessKeyUp`; if neither consumes it, the key goes to the key-binding helper (`+0x120`, slot 9; `0x007F3F39` reads it as `+0x114` from the handler's `this` = `+0x0C`; the key record supplies key code `+4` and modifiers `+8`, §6c) and, if bound, a command is posted through the command dispatcher `0x00B43CB0` | false only when unbound |
| `0xE4A85D91` (link) | `cSC4HTMLLinkHandler::DoURL(data2)` | true when DoURL **failed** *[quirk]* |
| `0x6A5BDFCD`, `0x6A720993`, `0xEA5BDFBC` | `HandleAdviceLinkMessage`, then base | base |
| anything else | base `cGZWin::DoMessage` | base |

### 6.1 Focus loss stops scrolling

A replacement must stop any edge or key scroll when the city window loses focus, but only under the global flag
`0x00B43CEC` (*[inferred]* pause-on-focus-loss / windowed mode; application.md `GP` cheat).

### 6.3 The message filter (`0x007F58E0`)

The filter sub-object (`+0x08`) is installed on child windows (InvokeSecondaryMenu, ui-windows.md §4.3). It first
runs the tutorial filter, then handles window message `3` (and `0x12`): a `0x287259F6` button press goes to
`HandleButtonActivated` (`0x007F4690`); IDs `0x287259F7`, `0x8988BC94`, `0x2988BC85` and `0x4988BC6A` have their
own branches (not specified here, VW4).

## 6a. City init: what the window builds per city *[compat]*

The window exists before any city; the city-scoped objects are built by the PreCityInit notification and torn down
before the next city.

**`DoPreCityInit` (`0x007F41B0`)**, in order:

1. Reads the window's client width and height (`cGZWin` slots 41/42, `+0xA4`/`+0xA8`).
2. Creates the object of GZCOM class `0xE9C6262A` (constructor `0x007FF930`) and stores it at `+0xEC`
   (`0x007F4224`). *[inferred]* a view-side helper; identity open (VW5).
3. Creates the **camera control** (GZCOM class `0xC9C628EC`, `cSC4CameraControl`, `0x007CC990`), stores it at
   `+0xF0` (`0x007F427C`), sets its viewport to the client size (`SetViewportSize`) and calls
   `cSC4CameraControl::Init` with the start focus **(512.0, 0, 512.0)** (`0x007F42A0`–`0x007F42B0`, two `0x44000000`
   floats). The real city size is applied later (PostCityInit step 2).
4. If no occupant filter exists at `+0x320`, a 16-byte one is created and stored there (`0x007F4307`).
5. Creates the **renderer** (GZCOM class `0xE9C622D8`, IID `0x681BD529`, `cSC43DRender`, `0x007C9B10`) and stores
   it at `+0xF4`, the field `GetRenderer` (slot 4, `0x0043FD00`) returns. It is wired to the camera control and the
   `+0xEC` helper through its slots 6, 7, 12 (view3d.md §3.2).
6. Looks up GZCOM `0xA2FFB5D3`/`0x02FFBB60`, then `AddBackgroundImageToRenderer` (`0x007F1640`).
7. Registers script hooks (`cSLString` with `0x6A231EAA`/`0xAA765647`, `0xAA76563A`) and reports progress to the
   progress screen (`0x00777640`).

The model maker is held at `+0xF8` (`GetModelMaker`, slot 5, `0x007F0FE0`).

**`DoPostCityInit` (`0x007ED0F0`)**, in order: updates the progress message; sets the camera control's city size
(`cSC4CameraControl::SetCitySize`, `0x007ED17F`) from the city; refreshes `Update24HourClock` and
`UpdateAmbientLightLevel`; enables the two child windows `0x4992046A` and `0xE9920494`
(`nsGZWinUtils::SetWindowEnabled`); refreshes the simulation clock, population and day/night buttons; posts the bond
message (`cSC4BudgetSimulator::PostBondMessage`); refreshes the mayor rating bar; then runs either
`DoCityEstablished` or `DoCityObliterated` (`0x007ED333`/`0x007ED33A`) according to the city's established state
*[inferred: the branch condition was not read]*; finally it builds a `cSC4ResponseCurve` from properties
`0x6A6564F4`/`0x6A6564F6` (`0x007ED5B5`–`0x007ED5DB`).

## 6b. The city-screen GUI: `AddGUI` (`0x007EDEB0`) and `RemoveGUI` (`0x007EF200`) *[compat]*

**Pairing.** `AddGUI` has one caller, the end of `DoPreCityInit` (`0x007F4665`, after the progress message and before
`UpdateProgressMessage`); it has no guard against a second run. `RemoveGUI` has one caller, `Shutdown` (`0x007F14B5`,
§4). *[inferred]* the window object lives for one city, so the two run once each per city.

In this section "script window *I*" means `SC4CreateWindowFromScript` with key `{type 0, group 0x96A006B0, instance
I}` and the city window (`+0x0C`) as parent. The two variants are `(key, parent, window ID)` → window (`0x005F9390`)
and `(key, parent, window ID, IID, out)` (`0x005F9480`), the second querying the new window for the IID into *out*.

**`AddGUI`, in order** (fields are absolute object offsets; `this` = object `+0`):

1. **Main panel**: script window `0xC973B411`, ID `0x0987B48F`, stored at `+0x18C` (`0x007EDEF2`); queried for
   `cIGZWinGen` (`0x5386D516`) into `+0x190` (`0x007EDF00`); the window's message filter (`+0x08`) is added to it (`cIGZWin` slot 116,
   `+0x1D0`); it is placed at the bottom of the parent (y = parent height − its height + 11, `0x007EDF87`–`0x007EDFAC`);
   flag `0x800` is set (slot 68); `ClearAcceptFocusFlagRecursive` is applied to it.
   **Then a hidden button at `+0x194`** (`0x007EDFE3`–`0x007EE193`): GZCOM class `0x22ECFC47`, IID `0x22BA0121`,
   into `+0x194`; when that succeeds and the object answers IID `0x8810` (a button interface, held only for this
   step): image `{0x856DDBAC, 0x46A006B0, 0x53244588}` with colour key (255, 0, 255) (city window `MakeFillColor`,
   slot 108), set with mode 2 (button slot 12); button slot 5 `(0x20, 1)`; flag 4 on; tooltip string
   `{0x6A231EAA, 0x8A51545D}` (button slot 35); notification target = the main panel (`SetNotificationTarget`, slot
   86); ID `0x43`; moved to (12, city-window height − 28) (`GZWinMoveTo`, slot 56); added to the city window, then
   hidden (slot 70); `ClearAcceptFocusFlagRecursive`.
2. **Five tab panels**, a loop `i = 0..4` (jump table `0x007EF1D8`):

| i | Script instance | Window ID | Extra |
|---|---|---|---|
| 0 | `0x69E3D347` | `0xC991EDA8` | — |
| 1 | `0x2BC90671` | `0x69E40A1F` | its child `0xE9889775` (as `cIGZWinGen`) is also kept in the lists `+0x198` and `+0x1B0` |
| 2 | `0xAA1F1F57` | `0xCA1F1D9C` | its child `0xABB26B0E` is also kept in `+0x198` and `+0x1BC` |
| 3 | `0x49889894` | `0xC98F49F1` | a `cSC4MySimWinProc` is created, stored at `+0x2E4` and `Init`ed (`0x007EE3D6`) |
| 4 | `0xAA53E3EA` | `0x0A78827A` | — |

   Each panel window is appended (one reference) to the window list `+0x198` (begin/end/capacity `+0x198`/`+0x19C`/
   `+0x1A0`) and to panel *i*'s own list at `+0x1A4 + 12·i`; its `cIGZWinGen` pointer goes to `+0x1E0 + 4·i`; the
   message filter is added to it; it is given the object at `+0x2D8` (generator slot 7). Flag `0x800` and
   `ClearAcceptFocusFlagRecursive` are applied as in step 1, and the panel is aligned to the main panel
   (`AlignWindowSetToWindow`). Before the loop, two helper objects are made: `+0x2D8` (`0x00774550`) and a
   `cSC4NewsTickerWinProc` at `+0x2DC`.
3. **Text fields** (`GetChildAs`, slot 37, text IID `0x212CDC1F`): main panel child ID 1 → `+0x310`; city-window
   children `0x09E418FE` → `+0x314` and `0xC9E41918` → `+0x318`.
4. **News ticker**: script window `0x2A2AED99`, ID `0xCA2AEDC0`, as `cIGZWinGen`, handed to
   `cSC4NewsTickerWinProc::Init` (`0x007EE6C1`), then aligned and appended to `+0x198`.
5. **Four more script windows**, each queried as `cIGZWin` (`0x22BA0121`), aligned, and kept in its own field:

| Script instance | Window ID | Field | Winproc created after it |
|---|---|---|---|
| `0x4BC906B5` | `0x6A64E3C0` | `+0x30C` | — |
| `0x2BC9060F` | `0x00004200` | `+0x300` | — |
| `0x6BC9065A` | `0x0A4A8176` | `+0x304` | `cSC4GraphsViewWinProc` at `+0x2F8` (`Init` `0x0076CA60`) |
| `0x6BC61F19` | `0xABC619D2` | `+0x308` | `cSC4BuildingSelectWinProc` at `+0x2FC` (`Init` `0x00769420`) |

6. `cSC4BudgetUIWinProc` (0x1A0 bytes) at `+0x2E8`, `Init`ed; a `cSC4WinCalloutBox` (0x1B4 bytes) at `+0x3D8`; a
   `cGZWinBMP` at `+0x3D4` with its surface `+0x3D0`; an image at `+0x3E0`; an object at `+0x31C` (`0x00762E70`);
   an unstored overlay window and a winproc for window `0xEA8CAD14` (`0x00769E90`). (Round 2 listed `+0x1F4` here:
   corrected, `AddGUI` only clears it, step 7.)
   In detail (`0x007EED41`–`0x007EF108`):
   - **Callout box `+0x3D8`**: style IDs `0xA85F1A83` (its slot 11) and `0xE9C86C9D` (slot 10), each looked up
     through `RZ::WCM` and kept by the box; hidden (`HideWindow` on its `cIGZWin` at box `+4`); background image
     `{0x856DDBAC, 0x46A006B0, 0x14416190}` set with corners `(-1, -1)` (box slot 4; −1 selects 8) only when the
     image loads; the image `{0x856DDBAC, 0x46A006B0, 0x14416160}` is kept at `+0x3E0` (assign: AddRef new,
     release old); the box is added to the city window (`ChildAdd`, slot 14).
   - **`cGZWinBMP` (0xFC bytes) `+0x3D4`**, its `cIGZWinBMP` (object `+0xD8`): `SetWinBMPFlag(6, 1)`; window flags
     `0x200000` and `4` on; hidden. A drawing surface is requested from the framework object at `0x00B43C9C` (slot 3,
     out = `+0x3D0`, the old one released first); when it succeeds the surface is set up 128 × 32 (its slot 3,
     `(0x80, 0x20, {1, 0})`), becomes the BMP's image (`SetBMP`), and the image area is the surface rectangle moved
     to the origin, `(0, 0, w, h)` (`SetImageArea`). The BMP window is added to the city window either way.
   - **`+0x31C`**: a 0x18-byte object (`0x00762E70`), AddRef, its slot 5 (`Init`).
   - **A full-size overlay window** (not stored): GZCOM `GetClassObject(0xCA5D3294, IID 0xCA5D3290)`; when it
     succeeds its `cIGZWin` gets `SetArea(0, 0, client width, client height)` (width/height from the city window's
     rectangle `+0xB4`..`+0xC0`), ID `0x6A5E44B6`, flag `0x10000` **off**, flags `0x200000` and `4` on, and is added
     to the city window; the class-object reference is released.
   - **Window `0xEA8CAD14`**: the city window looks it up recursively as `cIGZWinGen` (`GetChildAsRecursive`, slot 37,
     IID `0x5386D516`); when found it gets a new 0x14-byte object (`0x00769E90`) through its slot 7 *[inferred]* a
     winproc; nothing is stored in this object.
7. When the city is **not** established (city slot 22 false, `0x007EF11D`): `+0x1F4` := 0, `ShowPanel(0)`
   (`0x007E8B90`), and `nsGZWinUtils::SetWindowEnabled(main panel, 0x99887766, false)` and `(…, 0x8B96B73E, false)`.
   For an established city none of this runs. The rest (`0x007EF159`–`0x007EF1B6`) frees AddGUI's locals.

   Not created by `AddGUI`: `+0x2EC` and `+0x2F0` (zeroed in the constructor, released by `RemoveGUI`); no
   `cSC4View3DWin` method stores them directly (VW6).

**`RemoveGUI`, in order.** "Destroy *w*" means: the window manager (`cGZWin` member `+0x10` of the object, i.e. the
city window's manager) `DestroyWindow(w)` (manager slot 23), then `w->Shutdown()` (slot 5), then release, then the
field := null. Each step is skipped when its field is null.

1. Destroy the main panel `+0x18C`; release `+0x190`; destroy `+0x194`.
2. For every window in the list `+0x198`: remove the message filter `+0x08` from it (slot 117, `+0x1D4`), then
   `DestroyWindow` and `Shutdown` (the list's references are dropped when the list is cleared).
3. Shut down and release the winprocs: news ticker `+0x2DC` (`0x00772A70`), MySim `+0x2E4`, then release `+0x2D8`,
   `+0x2EC` (after its slot 5), `+0x2F0`.
4. Destroy `+0x30C`, `+0x300`, `+0x304`, `+0x308`; shut down and release Graphs `+0x2F8`, BuildingSelect `+0x2FC`,
   Budget UI `+0x2E8` (`0x00764B70`).
5. Destroy the three text fields `+0x310`, `+0x314`, `+0x318` (through their `cIGZWin` interface, slot 3).
6. The callout box `+0x3D8` is removed from the city window (window slot 16, `+0x40`) and released; release `+0x3D0`,
   `+0x3D4`; clear `+0x3DC` (`0x007EDE50`); release `+0x3E0`; `+0x31C` gets its slot 6 then is released.

A replacement must leave no window created by `AddGUI` alive after `RemoveGUI` and must remove the message filter
from every panel before destroying it.

## 6c. Input handlers: keys, characters and the mouse *[compat]*

All offsets are from the start of the object. The mouse and character handlers are `cIGZWin` slots (§1.1), so the
code's `this` is `+0x0C` and its displacements are 0xC smaller; `ProcessKeyDown`/`ProcessKeyUp` are called from the
window `DoMessage` with `this` = object start (`0x007F3F14`).

**State the handlers use.**

| Field | Meaning | Written by |
|---|---|---|
| `+0x11C` | modifier flags of the last input event | every key and mouse handler, first thing (`0x007E66DB`, `0x007E68D9`, ...) |
| `+0x134`/`+0x138` | the view input control **stack** (begin/end of a vector of control pointers; the top is the last element; `GetCurrentViewInputControl` returns it, null when empty, no reference added, `0x007E65E0`) | `SetCurrentViewInputControl` / `Remove...` |
| `+0x140`/`+0x144` | the **persistent** view input controls (vector; `AddPersistentViewInputControl` appends, `0x007EBED1`) | `Add/RemovePersistentViewInputControl` |
| `+0x348`, `+0x34C` | keyboard scroll velocity pair; `KillKeyboardScrolling` (`0x007E7F90`) zeroes both and calls `RecalcScrollVelocity` | key scroll commands |
| `+0x350`, `+0x354` | edge-scroll direction X / Z, each −1, 0 or +1 | `GZOnMouseMove`, `GZOnMouseExit`, focus loss (§6) |
| `+0x358`, `+0x35C` | client x, z of the last right-button press | `GZOnMouseDownR` |
| `+0x360` (byte) | "moved since the right press" flag: 0 at the press, 1 once the pointer is more than 3 pixels away on either axis | `GZOnMouseDownR` (0), `GZOnMouseMove` (1) |

Control slots used below are the view-input-control table of city-tools-input.md §2: 13 `OnCharacter`, 14/15
`OnKeyDown`/`OnKeyUp(key, modifiers)`, 16–19 `OnMouseDownL/DownR/UpL/UpR(x, z, modifiers)`, 20 `OnMouseMove`, 21
`OnMouseWheel(x, z, modifiers, delta)`, 22 `OnMouseExit`, 11 `IsSelfScrollingView`, 25 `AmCapturing`. Every visit
of a control is bracketed by AddRef and Release. Lists are walked **from the last element to the first**.
Two visiting modes appear:

- *first-taker*: stop at the first control that returns true;
- *broadcast*: call every control, ignore the results.

**Dispatch order per handler.**

| Handler (address) | Before the controls | Controls, in order | After | Returns |
|---|---|---|---|---|
| `ProcessKeyDown(key, mods)` (`0x007E66D0`) | store mods | stack first-taker (slot 14), then persistent **broadcast** (slot 14) — the persistent pass runs even when a stack control took the key | — | true when any stack control **or any persistent control** returned true (`0x007E6747`, `0x007E67A6`) |
| `ProcessKeyUp(key, mods)` (`0x007E67D0`) | same, slot 15 | same | — | same (`0x007E6847`, `0x007E68A6`) |
| `GZOnCharacter(ch)` (`0x007E6600`) | — | stack first-taker (slot 13); only when none took it, persistent first-taker with a bounds quirk (below) | — | always true |
| `GZOnMouseDownL(x, z, mods)` (`0x007E68D0`) | store mods; tutorial filter (below), mode `-1` | persistent first-taker (slot 16), then stack first-taker | — | always true |
| `GZOnMouseDownR(x, z, mods)` (`0x007E6A00`) | store mods; **take mouse capture** (`GZSetCapture`) | persistent first-taker (slot 17), then stack first-taker | if nobody took it: start a mouse scroll (below) | always true |
| `GZOnMouseUpL(x, z, mods)` (`0x007E8170`) | store mods; scroll re-check (below); tutorial filter, mode `1` | persistent first-taker (slot 18), then stack first-taker | — | always true |
| `GZOnMouseUpR(x, z, mods)` (`0x007E82E0`) | store mods; **release capture** if the window holds it; click-to-centre (below) | persistent first-taker (slot 19), then stack first-taker | — | always true |
| `GZOnMouseMove(x, z, mods)` (`0x007E9460`) | store mods; tutorial filter, mode `0`; drag flag; `UpdateCursorTextPositions(x, z)` (`0x007E9090`) | persistent **broadcast** (slot 20), then stack first-taker | edge scrolling (below) | always true |
| `GZOnMouseWheel(x, z, mods, delta)` (`0x007E6B50`) | store mods | **stack first-taker first** (slot 21); if one took it, return | else persistent broadcast, then `ZoomIn` (view slot 9) when `delta > 0`, `ZoomOut` (slot 10) when `delta ≤ 0` (`0x007E6C1F`) | always true |
| `GZOnMouseExit()` (`0x007E96E0`) | edge directions := 0, `RecalcScrollVelocity`; `UpdateCursorTextPositions(−10000, −10000)` | slot 22 on the **top stack control only** (no AddRef), then persistent broadcast | — | always true |

So the persistent controls see mouse buttons before the tool on the stack, but keys and the wheel after it.
`GZOnMouseDownL` does not take capture and `GZOnMouseUpL` does not release it; only the right button does.

**Tutorial filter** (`DownL`, `UpL`, `Move`). When the city (`0x00B43CEC`) exists and its tutorial system (city
slot 82) reports `IsTutorial` (slot 8), the handler calls `FilterLeftMouseClick(&x, &z, mode)` (slot 16; mode `-1`
down, `1` up, `0` move; tutorial.md). False: the handler returns true at once, no control sees the event (and on a
move nothing below runs, not even the drag flag). True: the event continues with the possibly rewritten x, z. On a
move, the result of `IsEdgeScrollingAllowed` (tutorial slot 9) then gates edge scrolling; without a tutorial edge
scrolling is allowed.

**Mouse scroll start** (`DownR`, `0x007E6AD5`–`0x007E6B3B`). Only when no control took the press: store x, z at
`+0x358`/`+0x35C`, clear the drag flag `+0x360`, create a `cSC4ViewInputControlCityScroll` (0x48 bytes, constructor
`0x004B7DC0`), AddRef it, make it the current control with `SetCurrentViewInputControl(control, 0)` (view slot 25),
call its `SetMouseScroll(x, z)` (`0x004B82F0`), then Release it (the stack keeps its own reference).

**Scroll re-check on left release** (`0x007E8188`–`0x007E81B4`). When the window holds capture and the current
control exists, is not self-scrolling (slot 11 false) and reports `AmCapturing` (slot 25 true):
`RecalcScrollVelocity`. Nothing else changes.

**Click-to-centre on right release** (`0x007E8303`–`0x007E8428`). Only when the drag flag `+0x360` is 0 and
`mods & 7` is 0 (no modifier key):

1. `Pick(x, z, 0, &occupant)` on the renderer `+0xF4` (renderer slot 65). On a hit: ask the occupant (slot 8) for its
   bounding box into a box preset to min (1, 1, 1), max (0, 0, 0); take the box centre; call the terrain
   (`0x00B43CF4`) `GetAltitude` and `GetWaterLevel` at the centre's x, z and **discard both results** *[quirk]*; then
   jump the camera control `+0xF0` to the box centre (`0x007CD810`, the routine the Lua `camera_jump` uses).
2. No occupant: `PickTerrain(x, z, &point, 0, 1)` (renderer slot 60), point preset to the value at `0x00B4E43C` in
   all three components; on a hit jump the camera to `point`; on a miss nothing.
3. A picked occupant reference is released.

Then the controls see the release as in the table. A right *drag* (more than 3 pixels on either axis since the press)
never recentres.

**Drag flag** (`0x007E94BD`–`0x007E94E9`). On every move that passes the tutorial filter: if |`+0x358` − x| > 3
or |`+0x35C` − z| > 3, `+0x360` := 1. It is set whether or not a button is held; only `DownR` clears it.

**Edge scrolling** (`0x007E95C3`–`0x007E96CD`), after the controls, with `c` = the current control:

1. Not allowed (tutorial said no, or the preference byte `+0xF09` of the object at `0x00B43CD8` is 0): directions :=
   0, `RecalcScrollVelocity`.
2. Else, when `c` is null or not self-scrolling, **and** (global `0x00B43C9C` slot 12 returns true, or `c` exists and
   `AmCapturing`): X := −1 if x ≤ 1, +1 if x ≥ width − 1, else 0 (width = window slot 41); Z the same with z and
   height (slot 42); `RecalcScrollVelocity`. *[inferred]* `0x00B43C9C` is a framework object whose slot 12 means "the
   application is active"; its identity is open.
3. Otherwise: if `0x00B43C9C` slot 12 is true, keep the directions; else, if either direction is non-zero, zero both
   and `RecalcScrollVelocity`.

**`GZOnCharacter` bounds quirk** (`0x007E665B`–`0x007E66A7`). The persistent pass starts at index (persistent count −
1) but checks each index against the **stack's** count: it stops as soon as the index is not below the stack size.
With more persistent controls than stack controls the pass does nothing; with fewer, it is a normal top-down
first-taker pass. A replacement that wants identical character routing must keep this bound *[quirk]*.

## 7. Conformance tests

- VW-T1: QI on the five IIDs of §1 returns the documented sub-object offsets; any other IID returns false.
- VW-T2: after `Shutdown`, no cheat ID of §3 is registered, and no view input control (normal or persistent) remains.
- VW-T3: `0xAA5CF1E3` while modal popups are unsafe is queued once; a second identical message does not add a copy.
- VW-T4: a link message whose URL opens returns false from the window `DoMessage` (§6 quirk).
- VW-T5: `SizeOf` with arguments `100`, `0.5`, `abc`, `5` hands the camera control 64.0, 0.9, 0.9, 5.0; `SizeOf` alone returns false and changes nothing (§3.1).
- VW-T6: `TastyZots` twice in a row restores the filter flag `+0xD`; each `TastyZots`/`Zoneria` message returns false; `TerrainQuery OFF` leaves `GetTerrainQueryEnabled` false, `TerrainQuery` alone makes it true (§3.1).
- VW-T7: with a stack control that takes key-down and a persistent control that does not, `ProcessKeyDown` returns true and the persistent control was still called once; with neither taking it, a persistent control returning true alone makes the result true (§6c).
- VW-T8: right press at (100, 100) on empty controls, move to (102, 103), release with mods 0: the camera jumps to the picked point; the same with a move to (104, 100) or with mods 1: no jump (§6c).
- VW-T9: wheel delta 0 with no control taking it calls `ZoomOut`; a stack control that takes the wheel suppresses zoom and the persistent controls are not called (§6c).

## 8. Open questions

See [questions/view3d-window.md](questions/view3d-window.md): VW1 answered (§3.1); VW2
(identity of the two GZCOM helpers in `Init`), VW3 (length of the command-name table), VW4 (filter branches), VW5 (the `+0xEC` helper and the established/obliterated branch), VW6 (`AddGUI` tail objects and fields `RemoveGUI` releases).
