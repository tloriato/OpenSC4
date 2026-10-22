# Spec: city tools (flora, torch, terraforming, city situations)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`). Implementers must work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled
or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[compat-limit]* depends on something FreeCity4 cannot reproduce, so only the rule is matched.

Related specs: [city-tools-input.md](city-tools-input.md) (§2 the base control, its fields and slots),
[terrain.md](terrain.md) (§9.2 `ApplyBrush` and its parameters, §9.6 the terraforming tool), [flora.md](flora.md)
(§6.4, §6.5, §11.1), [disasters.md](disasters.md) (§5.5 `IgniteCell`), [effects.md](effects.md) (§4.1, §4.2 making
visual effects), [audio.md](audio.md) (§7.3 sound events, §7.4 the torch sounds), [city-situation.md](city-situation.md)
(the situation manager's slot 22), [dispatch.md](dispatch.md) (§2 and §8: the vehicle view, slot 22),
[ui-windows.md](ui-windows.md) (the city view's slots).

## 0. Summary

Four more view input controls (tools): the **flora** tool plants trees from the catalog, the **torch** sets fires,
the **terraforming** tool (`cSC4ViewInputControlTerrainScrubber`) strokes terrain brushes, and the
**city-situations** tool lets the player click the mission indicators of the dispatch vehicle view. Fields are
offsets in the tool object; `+0x1C` is the city view and `+0x24` onward belong to the tool (city-tools-input.md
§2.2).

## 1. The flora tool (`cSC4ViewInputControlFlora`) *[compat]*

- Made by the city view's `InvokeCatalogItemCommand` (`0x007E9FA0`) with a flora type (`+0x28`). `Init`
  (`0x004BBA20`): id `0x26D7881C`, cursor `0x66DB057F`; base `Init`; the flora simulator (`0x00B43D80`, held); the
  price `c` (`+0x38`, int64) := the type's static data (flora slot 7) `+0x160`; the stroke total `T` (`+0x40`) := 0;
  the city view's `SetCostIndicator(c)` (slot 43). `Shutdown` (`0x004BB940`): the simulator is released, the cost
  indicator cleared (slot 44, true).
- **Planting at `p`** (the city view's `PickTerrain(x, y, &p, 0)`, slot 36): `AddNewFloraOccupant(type, &p, false)`
  (flora slot 17). A tree planted: the sound `0x49DC0C90` is started with a new instance id and positioned at the
  truncated `(x, y, z)` (events `0xEA451903` and `0xCA45190F`); `WithdrawFunds(c)` (budget `0x00B43D64`, slot 8);
  `T += c`; the city view's `SetCostIndicator(T, c)` (slot 42); the aura simulator's
  `AddTransientEffect(p.x, p.z, 1)` (slot 6). No tree, and the vertex `(trunc(x / 16), trunc(z / 16))` is water
  (terrain slot 37): the result sound `0xAA62C9D5` (`PlayResultSound`, slot 28).
- **Press** (`0x004BBAF0`): with a terrain hit and funds (budget slot 6) of at least `c`, plant at `p`, adding `c`
  to `T` and updating the indicator only when the withdrawal succeeds. Pressed (`+0x31`) whenever there was a hit.
  True.
- **Move** (`0x004BBC80`): with a hit while pressed: funds below `c` set the out-of-money byte `+0x48`; else plant
  at `p` (`T += c` whatever the withdrawal returns). With a hit while not pressed: `+0x30` := whether
  `LocationIsOKForNewFlora(type, p.x, p.z, null)` (flora slot 15). Then, every move, `+0x48` plays the result sound
  `0xAA62C9D5`. True.
- **Release** (`0x004BB980`): not pressed; `T > 0` clears the cost indicator, else `SetCostIndicator(c, c)`; `+0x48`
  and `T` reset. True.

## 2. The torch (`cSC4ViewInputControlTorch`) *[compat]*

- Made by the city view's `InvokeDisaster` (`0x007E9DB0`). `Init` (`0x00580660`): id `0xE9C24AC4`, cursor
  `0x4A08C57E`; inactive (`+0x60`); the fire manager (city slot 72, layer slot 7 for `0x621CDA33`, interface
  `0x49B43F57`, `+0x2C`); base `Init`; a timer (`+0x68`) started; tool message `0x2A106113`.
- **Tool messages** (`SendTorchToolAudioMessage(type, a, b, c)`, `0x00580550`): the sound system (`0x00B43CB8`) makes
  an event of `type` with `a`, `b`, `c` (slot 13, interface `0x5B523BA4`) and posts it (slot 12) *[inferred]*;
  audio.md §7.4's torch entry points use them. Types: `0x2A106113` on, `0x8A10611A` off, `0x8A10611D` press,
  `0x0A106122` release (and on deactivation), `0x4A106126` move (each of these three with `x`, `y` and the timer's
  count, milliseconds *[inferred]*), `0xAA96FE8F` with the active flag.
- **The effect** (`+0x30`): `torch_inactive` or `torch_active` by name (effects manager `0x00B43D1C`, slot 7). Turning
  active or inactive (`ActivateTorch(on)`, `0x005806D0`), with an effect and a change: the old effect is stopped
  (slot 4, 0) and released, the other made and started (slot 3, 0); deactivation sends `0x0A106122`; then
  `0xAA96FE8F(on)`. `UpdateTorchEffect` (`0x00580760`) places it at `+0x34` (identity rotation, scale 1). Dropping it
  (`ShutdownTorchEffect`, `0x005807F0`, also `OnMouseExit`): a started effect is stopped (slot 4, 1); released;
  inactive; `0xAA96FE8F(0)`.
- **Move** (`0x00580A30`): `PickTerrain(x, y, &+0x34, 0)`. A miss drops the effect. A hit: with no effect, make
  `torch_inactive` and start it; inactive with modifier bit 4 (`0x10`) → active *[inferred: the left button is
  down]*; place the effect; while active, the cell `(+0x40, +0x44)` := the hit's cell. Then `0x4A106126`. True.
- **Press** (`0x005808B0`): `0x8A10611D`; a terrain hit (else true): an existing inactive effect turns active (as
  above, without the deactivation message); placed; the animation tick manager is told to call the tool (`+0x28`,
  `0xC0`); the fire manager's `IgniteCell(cell of the hit, false, true, 0)` (slot 5) and `SetCause(1)` (slot 9);
  cursor `0x0A08C54E`. True.
- **Animation ticks** (message `0x86AD10EE`, `0x00580510`) while active: `IgniteCell(+0x40, +0x44, true, true, 0)`
  and `SetCause(1)`.
- **Release** (`0x005809D0`): `0x0A106122`; the tick registration removed; inactive; the tool's own cursor. True.
  `Shutdown` (`0x00580840`): the timer stopped, the effect dropped, `0x8A10611A`, the fire manager released, the
  tick registration removed.

## 3. The terraforming tool (`cSC4ViewInputControlTerrainScrubber`) *[compat]*

### 3.1 Making it

- `InvokeLandscapeTool(brush, godMode)` (`0x007E5910`) makes the tool and makes it current. Construction
  (`0x004D9430`): brush `+0x2C`, god mode `+0xB4`; the brush parameters (terrain.md §9.2) at `+0x74` (level −1,
  redisplay, strength `+0x8C` and radius `+0x90` 1.0, `data4` 3, target height 100); base strength `+0xE0` and
  radius `+0xE4` 1.0; id `0x68E29401`, cursor `0x071E5703`, stroke cursor `0x071E5705` (`+0xFC`), idle cursor 1
  (`+0x100`); an empty action queue (`+0x128`) and two timers (`+0xC0`, `+0x108`).
- **Tunables** (`ReadTuningParameters`, `0x004D8BC0`), read once per process (the flag is reset by a static-data
  callback registered with the application, slot 69): the exemplar (`0x6534284A`, `0x88CD66E9`, 1), properties
  `0x69DB8D00` the texture batch `N` (default 100), `0x69DB8D01`/`02` the Ctrl strength and radius (0.25, 0.25),
  `0x69DB8D03`/`04` the Shift strength and radius (4.0, 1.0), `0x69DB8D05` the ramp period `P` when > 0 (`k := 1 /
  P`; default `k` 10).
- `Init` (`0x004D9080`): base `Init`; the terrain (held), the brush's UI elements (`GetUIElementsForBrush`, slot 63,
  into `+0x30`…`+0x57`), the terrain view (held), the window manager (held); notifications `0xA6B79602` and
  `0xA6B79612`; the echoes off (§3.4); the idle cursor.

### 3.2 A stroke

- **Modifiers** (`0x004D8AC0`): strength and radius := the base pair; Ctrl (modifier bit 1) → the Ctrl pair, else
  Shift (bit 0) → the Shift pair.
- **Where** (`GetCursorPositionInWorld`, `0x004D8D10`): during a stroke while key `0x14` reads down (window manager
  slot 6, slot 38, slot 1) *[inferred: Caps Lock]*, the mouse ray meets the level plane at the anchor height
  (`+0x124`), clipped to the city: true. Otherwise `PickTerrain(x, y, &p, 0)`: true on a hit, else the plane point
  and false. The anchor (`SetCursorAnchor`, `0x004D8CB0`) is the altitude (terrain slot 13) under a
  `PickTerrain(x, y, &p, 1)` hit.
- **Press** (`0x004D9B90`, false without the city view): no anchor or no position → the echoes hidden at `p`; true.
  In mayor mode with a positive application cost (terrain slot 62) and a budget whose funds are ≤ 0: true, nothing
  else. Otherwise the stroke starts: the base pair, then the modifiers; scrubbing (`+0x64`); first application
  (`+0xD8`); `data4 := 1` in god mode, 2 in mayor mode; target height := `p.y`; `BeginBrushScrub(brush, data4)`
  (slot 59); the last point (`+0x68`) := `(p.x, p.z)`; the city view's keyboard scrolling stopped (slot 35); the mouse
  captured; the terrain view's cell cursor off (old setting kept); the application count `n` (`+0xB0`) := 0 and, when
  `N > 0`, `StartAccumulatingTextureUpdates` (slot 78); the echoes on and shown at `p`; the spent total (`+0xB8`)
  := 0; the queue timer restarted; one action from the last point to itself queued and the queue run; animation
  ticks requested. True.
- **Move** (`0x004D9D80`): at most one per animation tick (`+0xAC`). Not scrubbing and no position → echoes hidden;
  true. While scrubbing: an action from the last point to `p` is queued, the queue run, the last point := `p`, the
  window manager's slot 19 called, the tick byte set. The echoes are shown at `p`. True.
- **Animation ticks** (message `0x86AD10EE`) while scrubbing: the tick byte cleared; a counter (`+0x58`) counts the
  ticks since the last application and at its third queues an action from the last point to itself (resetting the
  counter); the queue is run. True.
- **View changes** (`0xA6B79602`, `0xA6B79612`) while the tool is current: the tick byte cleared, the mouse position
  (window manager slot 33) replayed as a move (slot 20, modifiers 0); with a first echo, both echoes re-placed at
  the echo point, shown when the first is started.
- **Release** (`0x004D9970`, false unless scrubbing): the whole queue is run; the cell cursor restored; the echoes
  off and re-placed; the cost indicator cleared; capture and ticks released; `EndBrushScrub(brush, data4)` (slot
  60); not scrubbing; `StopAccumulatingTextureUpdates` (slot 79). True.

### 3.3 The action queue

- `PushDelayedAction(from, to)` (`0x004D9A30`): appends `{due, from, to}` with `due` := the queue timer's elapsed
  time plus `+0x120` (0 as constructed; nothing here changes it).
- `ProcessDelayedActions(all)` (`0x004D9740`): every due entry (all of them when `all`) is taken from the front; none
  → return. Only the **last** taken entry is applied, once: the strength is scaled for this application by
  `min(2, k × s)`, `s` := the count of timer `+0xC0` × 10⁻⁶ (seconds *[inferred]*) since the previous application,
  except on the stroke's first one;
  `from` and `to` go into the parameters, redisplay on. In mayor mode the charge is `ceil(cost × strength)`: the
  cost indicator shows `(spent + charge, charge)`, then `WithdrawFunds(charge)`; a refused withdrawal skips the
  brush, else the charge is added to the spent total. Then `ApplyBrush(brush, params)` (slot 57). Last: not the first
  application, `RecordBrushApplication` (`0x004D8B20`: the tick counter := 0; `n += 1`; when `n > N`, the texture
  updates are flushed (stop, then start again when `N > 0`) and `n` := 0), and the strength restored.

### 3.4 Keys and echoes

- **Keys** (`OnKeyDown`, `0x004D95D0`), only while the tool is current. With Shift: F1–F10 set the base and current
  strength to 0.1–1.0; `1`–`9` and `0` set the base and current radius to 0.1–1.0; true. Shift or Ctrl itself going
  down or up (`OnKeyUp`, `0x004D96E0`) re-applies the modifiers and re-places the echoes; false.
- **Echoes** (`SetCursorEchoState(on)`, `0x004D8F50`): the two UI effects of the state (`on`: `+0x30`/`+0x34` and
  `+0x40`/`+0x44`; off: `+0x38`/`+0x3C` and `+0x48`/`+0x4C`) replace the old ones (a started old one is stopped with
  0 and 1), made by ID pair (effects manager slot 6) when the settings flag (`0x00B43CC4` → `+0x0C` → byte `+0x23C`)
  is set; the stroke cursor := `+0x50` (on) or `+0x54`. `SetCursorEchoPos(p, show)` (`0x004D9160`): the echo point
  (`+0xF0`) := `p`; the first echo scaled by the brush radius (terrain slot 61) × the radius scale, the second by
  `3 / 1.7^zoom`, both at `p`; each started when shown and stopped when not; the cursor: the stroke cursor when
  shown, else the idle one. `OnMouseExit` (`0x004D9720`) hides them.
- **Shutdown** (`0x004D8DD0`): a running stroke is ended (`EndBrushScrub`), capture released; notifications removed;
  started echoes stopped; the window manager, the terrain view (its selection cleared) and the terrain (texture
  updates stopped) released.

## 4. The city-situations tool (`cSC4ViewInputControlCitySituations`) *[compat]*

- A persistent control added by the city view's `DoPostCityInit` (`0x007ED0F0`, slot 28). Construction
  (`0x004B8690`): id `0xABEC6CFE`, cursor `0x43401C71`. `Init` (`0x004B84B0`): base `Init`; the city's situation
  manager (slot 49) and dispatch manager (slot 39), held; notification `0xA6B79602`.
- The indicators belong to the dispatch manager's vehicle view (slot 22): `FindIndicatorHandle(x, y)`,
  `LockIndicator(h, on)`, `SetActiveIndicator(h, state)`, `GetActiveOccupant()`.
- **Move** (`0x004B8620`): `h` := the indicator under the mouse. With none locked: lock `h`, remember it (`+0x34`),
  `SetActiveIndicator(h, 3)`. With another one locked: unlock it and forget it. False.
- **Press** (`0x004B8720`), only at view level 1 (the city view's slot 22): an indicator under the mouse is set to
  state 2 and remembered (`+0x38`); true. Else false.
- **Release** (`0x004B8790`), at view level 1 with both managers: an indicator under the mouse is set to state 1;
  when it is the pressed one and the view has an active occupant, the situation manager's
  `InitiateSituationForOccupant(occupant, null)` (slot 22); true. Otherwise the pressed indicator is forgotten;
  false.
- **Messages** (`DoMessage`, `0x004B85B0`): `0x06CDB65B`, `0xA6B79602` or `0xA6B79612` unlock the locked indicator,
  `SetActiveIndicator(0, 1)`, and forget both. True.

## 5. Quirks

1. A flora drag adds the price to the stroke total even when the withdrawal fails; a press checks it (§1).
2. While the funds are short, every mouse move of a flora drag replays the result sound (§1).
3. The torch's ticks burn the cell of the last active move: right after a press, before any move, that is an older
   stroke's cell, or cell (0, 0) (§2).
4. A mayor-mode stroke is refused only when the funds are ≤ 0; an application the budget refuses is skipped
   silently (§3.2, §3.3).
5. Several queued actions are collapsed into one application, the last (§3.3).
6. The terraforming tool's `Shutdown` stops the second echo but clears the first echo's slot again; the second echo
   is released only by the destructor (§3.4).
7. The city-situations tool handles three messages but subscribes to one (§4).

## 6. Open questions

See [questions/city-tools.md](questions/city-tools.md): CT1 (the key read as Caps Lock, the settings flag and the
torch's modifier bit 4), CT2 (the torch messages' event creation in the sound system).

## 7. Conformance tests

1. **Ramp:** `k` = 10 and 50 ms since the previous application scale the strength by 0.5; 300 ms by 2.
2. **Charge:** an application cost of 10 at strength 0.25 charges `ceil(2.5)` = 3.
3. **Keys:** Shift+F3 sets the strength to 0.3; Shift+0 sets the radius to 1.0.
4. **Modifiers:** Ctrl held at the press gives strength 0.25 and radius 0.25; Shift gives 4.0 and 1.0.
5. **Texture batches:** with `N` = 100 the texture updates are flushed after the 101st application.
6. **Echo:** at zoom 3 the second echo's scale is `3 / 4.913` ≈ 0.6106.
7. **Flora funds:** funds 50 and a price of 100: a press plants nothing; a drag sets the out-of-money byte.
