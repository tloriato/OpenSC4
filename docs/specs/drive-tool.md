# Spec: the U-Drive-It tool (`cSC4ViewInputControlPlayerDrive`)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`). Implementers must work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled
or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[compat-limit]* depends on something FreeCity4 cannot reproduce, so only the rule is matched.

Related specs: [city-tools-input.md](city-tools-input.md) (§2 the base control), [automata.md](automata.md)
(§6.2 state `0x10` PlayerDrive, §9 the U-Drive-It hooks, the manager slots 9 `Pause` and 10 `Resume`),
[ui-windows.md](ui-windows.md) (the city view slots), [network-overlay-views.md](network-overlay-views.md)
(`SC4TrainJunctionView`).

## 0. Summary

The tool has two modes. In **choosing** mode the simulation and the automata are paused and the player clicks a
vehicle (or a rail junction to switch it). In **driving** mode the clicked automaton runs the player-drive state, the
camera tracks it, the keys and mouse buttons become drive commands, a dashboard window shows its gauges, and the
simulation runs again. Escape, or the vehicle's removal, ends the tool.

## 1. Making it *[compat]*

- **Construction** (`0x00565480`): id `0x2B709676`, cursor `0xC16C4E1B`; no automaton; the camera speed value
  (`+0x40`) `0x3D2C0831`; the two toggles (`+0x44`, `+0x45`) off; a timer; an empty callback list (`+0x64`); the
  cell size and its inverse 1.
- **`Init`** (`0x00565A10`, once): the base `Init`; animation ticks requested (priority `0xFF`); the terrain held, with
  its cell width and inverse; two pick filters (an automaton filter, vtable `0x00A9D894`, and a network-occupant
  filter, vtable `0x00A8FE14`); four interface objects registered with the resource system *[summary]*; the four
  automata managers (city slots 43–46: vehicles, pedestrians, aircraft, watercraft) held; a junction view
  (`SC4TrainJunctionView`, network-overlay-views.md §2: constructor `0x005649D0`, its `Shutdown` `0x00563730`
  releasing its pictures and models) added to the renderer at layer 5, priority 1000; the dispatch vehicle view
  (`0x00B43D04` slot 22) with all its indicators shown (`ShowAllIndicators(4, 0)`); subscriptions to `0xA6B79602`
  and the three messages of the table `0x00B0A110`; the network manager (`0x00B43D54`), the simulator
  (`0x00B43D5C`) and the automata script system (city slot 48) held; with a city view, its key accelerator (slot 31)
  is saved and a new one made from resource `0xA2E3D533` (GZCOM `0x42E967BE`) and installed (slot 32); the three
  command IDs of the table are registered with the framework service `0xEB903A32` (its slot 7).
- **`Shutdown`** (`0x00563970`) undoes these *[summary]*.

## 2. Choosing mode *[compat]*

- **`EnterChoosingMode`** (`0x00564450`): `StopTracking` (§4); the hovered automaton is dropped and the cursor text
  `0xAB73510C` cleared; the simulator is paused when it is not (its slots 15 and 9) and the four managers paused
  (slot 9).
- **Hover** (`OnMouseMove`, `0x005631C0`): a terrain pick (city view slot 36) gives the junction view's hover cell
  (city `PositionToCell`; none on a miss). While driving (an automaton without flags `0x8000000` and 1, its slot 31),
  the mouse position is written into the drive state (state `0x10`, `+0x214`, `+0x218`). Otherwise the automaton under
  the mouse (`PickAutomaton`, `0x00563140`: renderer slot 65 `Pick` with the automaton filter, the instance's
  occupant answering `cISC4Automaton` `0xA9B40F05`) becomes the hovered one; none → cleared, with the cursor text.
- **Click** (`OnMouseDownL`, `0x005655D0`):
  1. A network occupant under the mouse (`PickNetworkOccupant`, `0x005630C0`, IID `0xA821EF94`) that the network
     manager accepts (its slot 42) plays sound `0x8BE8D9AC` at the occupant's position and becomes the junction
     view's selected cell *[inferred: a rail junction whose route the player's train takes]*.
  2. Otherwise, with no automaton driven, the automaton under the mouse enters driving mode (§3).
  3. While driving, unless the city situation manager (city slot 49) reports its slot 32 *[inferred: a mission's
     start countdown]*, an automaton in group `0x4006` gets command 7 pressed (§5).

## 3. Driving mode (`EnterDrivingMode(a)`, `0x00565160`) *[compat]*

1. The disaster layer (city slot 72) is switched off (its slot 5 with 0) *[inferred]*.
2. The automaton's drivable form (its slot 50) must exist; it is the automaton used below. `StopTracking`; the hover
   is cleared.
3. The city view's UI is minimized (slot 55) and its window `0x43` shown; the dashboard (`ShowUI`, §6) opens with the
   automaton's property `0x2BE8FBF3`, or by default `0x6BE8846C` for an aircraft (type `0x480E6151`) and `0x0BE8824F`
   for anything else.
4. Audio: the occupant's sound property `0xCBBBA52F` starts (AEMS events slot 3), engine event 0, and the three
   values of `0x00B229A8` *[summary]*.
5. The automaton becomes the driven one (reference held); it is given state `0x10` (its slot 52 with
   `0x10, 0, −1.0, 0, 0, 0, 0`); the vehicle manager tracks it (slot 34).
6. The camera: from the automaton's drive settings (its slot 67, then `+0x64` or its AEMS data) the zoom range and
   the camera speed (default zoom 1, level 5, speed `0x3D2C0831`); the city view's `SetZoomLimits(zoom − 1, 4 or 5)`
   (5 when the application preference at `+0xF10` is above 0) and `SetZoom(level − 1)`; the dashboard's zoom buttons
   `0x4992046A` / `0xE9920494` enabled by `CanZoomIn` / `CanZoomOut`.
7. The toggles: `+0x44` on, `+0x45` off; the dispatch vehicle view tracks the occupant; the automaton gets the tool as
   a callback target (its slot 39); the timer is reset; `PauseEverything(false)` resumes the simulator when it is
   paused (its slots 15 and 12 `Resume`) and the managers (slot 10).

## 4. Leaving *[compat]*

- **`StopTracking`** (`0x00562FD0`): with a driven automaton: its callback target removed (slot 40); the dispatch
  vehicle view untracks it; the vehicle manager tracks nothing; the zoom limits back to `(0, 5)`; the AEMS engine
  event stopped and the occupant's sound `0xCBBBA52F` stopped. In state `0x10`, an automaton not flagged `0x8000000`
  and not in group `0x4003` gets state 0 (its slot 13 with 0); otherwise its slot 53 runs. The automaton is released.
- **`EndInput`** (`0x005650E0`): the callback message `0x0BB324BD` (§7) with code 1; an automaton for which its slot
  18 is true gets slot 13 with 0; `StopTracking`; the hover cleared; the disaster layer switched on again (slot 5
  with 1); the base `EndInput`.
- **Escape** (`OnKeyDown`, key `0x1B`) ends the input. **`AutomatonRemoved(a)`** (`0x00563310`): when `a` is the driven
  one, `StopTracking` and `EndInput`.

## 5. Drive commands *[compat]*

Keys (not auto-repeats, flag `0x40000000`) and releases go, unless the city situation manager's slot 32 is true,
through the automata script system's key map (its slot 14) to a command 0–12 (13 = none). First the automaton's
occupant group template's key effects (`HandleKeyEffects`, `0x00566A70`) may take the key; otherwise:

| Command | Down | Up | Sound event (AEMS start / stop) |
|---|---|---|---|
| 0, 1 | `SetDriveCommand(c, 1)` | `(c, 0)` | 1, 2 |
| 2, 3 | the dashboard light `0x8BCB93FC` (2) or `0x6BCB93F5` (3) on, then `(c, 1)` | light off, `(c, 0)` | 3 |
| 4, 10 | toggles `+0x44` (4) or `+0x45` (10), then `(c, toggle)` | nothing | — |
| 5 | `(5, 1)` | `(5, 0)` | 6 |
| 6, 11, 12 | `(c, 1)` | `(c, 0)` | — |
| 7 | `(7, 1)` | `(7, 0)` | 4 |
| 9 | `(9, 1)`, and the drive state's byte `+0x229` toggles | `(9, 0)` | — |

`SetDriveCommand` is the automaton's slot 60; every command needs a driven automaton without flags `0x8000000` and 1
(`IsDriveAutoValid`, `0x00562ED0`). The right mouse button is command 8 (down 1, up 0); the left button's release
clears the junction view's selected cell and turns command 7 off (sound event 4 stopped).

## 6. The dashboard *[summary; DR1]*

`ShowUI(id)` (`0x00564570`) loads the dashboard window `id` into the city view. Its buttons (`DoWinProcMessage`,
`0x00562E20`) post commands: `0x4C1A5CC9` → `0xCC1F0457`, `0x2C16C9CB` → `0xAC1F0449`, `0x4992046A` → `0x6A935CB7`,
`0xE9920494` → `0x6A935CBA` (command server slot 20). On each animation tick `Update` (`0x00566000`) fills the gauges
from the drive state (speed, altitude and other values; a number formatted with `%…`).

## 7. Messages and callbacks *[compat]*

- `DoMessage` (`0x00566890`): the animation tick `0x86AD10EE` runs `Update`; `0xA6B79602` re-enables the zoom buttons;
  `0xAC1F0449`, `0xAC1F045D` and `0xCC1F0457` show or hide two dashboard panels; the automaton notification
  `0x09D92FC3` with code 3 → `AutomatonRemoved`, code `0xB` → the engine sound stopped, the alternate model used and
  a timer restarted, code `0xD` with the drive state gone → the engine sound stopped.
- **Callbacks:** `AddCallbackTarget` (`0x00565FA0`) adds a message target; `SendCallbackMessage` (`0x005644E0`)
  sends message `0x0BB324BD` (with the tool and two values) to each target.

## 8. Quirks

1. The simulation is paused while choosing and runs while driving (§2, §3).
2. A left click on a network occupant is taken before the automaton pick (§2).

## 9. Open questions

See [questions/drive-tool.md](questions/drive-tool.md): DR1 (the dashboard: `ShowUI`, `Update`'s gauges and the
interface objects of `Init`), DR2 (the meaning of commands 0–12 and of the automaton slots used).

## 10. Conformance tests

1. **Toggle:** two presses of command 4 send `SetDriveCommand(4, 0)` then `(4, 1)` (starting from on).
2. **Repeat:** a key down with flag `0x40000000` does nothing.
3. **Escape:** key `0x1B` ends the input and switches the disaster layer back on.
4. **Pause:** entering choosing mode pauses the four automata managers; entering driving mode resumes them.
