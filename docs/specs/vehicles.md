# Spec: road vehicles, trains and tanks (the concrete traffic automata)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`). Implementers must work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult
decompiled or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[summary]* is a correct outline whose details are not given.

Related specs: [automata.md](automata.md) (the automaton base, records §6.1–§6.4, the state types and their
per-tick behaviour §6.5, the slot names §11 — this spec uses those slot numbers), [dispatch.md](dispatch.md) §4.3
(the emergency dispatch states), [drive-tool.md](drive-tool.md) (U-Drive-It),
[traffic-simulator.md](traffic-simulator.md), [disasters.md](disasters.md) (fires, riots, toxic spills),
[audio.md](audio.md) (AEMS events).

## 0. Summary

The automata that travel on networks share `cSC4TrafficAutomaton` (routes, leaders, collisions) and, for road and
rail vehicles, `cSC4Vehicle` (paths, damage, motion blur, player driving). Subclasses add trailers (articulated
vehicles), trains (a locomotive with cars), the tank (a drivable reward vehicle that shoots), the emergency
vehicles (dispatch states and re-pathing; the fire truck with its fire crew and the patrol car with riot police),
the bus (station stops) and the getaway van. Their records are automata.md §6.4; their state ticks are automata.md
§6.5. This spec gives the rest.

## 1. Classes *[compat]*

| Class | Base | GZCOM / record | Notes |
|---|---|---|---|
| `cSC4TrafficAutomaton` | `cSC4Automaton` | record automata.md §6.3 | §2 |
| `cSC4Vehicle` | traffic automaton | `0x896E714A` | §3 |
| `cSC4TrailerVehicle` | vehicle | `0x4A5B69ED` | §4 |
| `cSC4TrainCar`, `cSC4Train` | vehicle | train record with car refs (`0x4A416254`) and a network occupant (`0xA821EF94`) | §5 |
| `cSC4Tank` | vehicle | vehicle record + two dwords | §6 |
| `cSC4EmergencyVehicle` | vehicle | emergency record | §7 |
| `cSC4FireTruck`, `cSC4PatrolCar` | emergency vehicle | + a ref (`0xA9B40F05`) | §7.2, §7.3 |
| `cSC4RiotPolice` | pedestrian | pedestrian record + a ref | §7.3 |
| `cSC4Bus`, `cSC4GetawayVan` | vehicle | vehicle record (+ a dword for the van) | §8 |

Each class has the usual `GetGZCLSID`, `Read`/`Write` (automata.md §6.4) and adjustor thunks for its `Init` and
`Shutdown` at `+4`.

## 2. The traffic automaton *[compat]*

- **Animation tick** (`DoAnimationTick`, `0x0054E050`): a pending speed tween advances (`+0x1AC` toward `+0x1A8`,
  time scaled by a constant; when done the speed `+0x1A0` := the target `+0x1A4`); then the tween and effect
  updates (slots 83, 103); when the automaton moves (slot 86) and its state does not handle the tick (slot 109):
  without a finished route (slot 87), a path move (slot 129) when it has a network tile, a tile position and a
  route, else a straight-line move (slot 130); at the end of a route that is not held (slots 85, 165) it removes
  itself (slot 13); then slot 105 and slot 102 with (0, 0).
- **Routes.** `BeginRoute` (`0x00554AB0`) clears the look-ahead deque, pops the first route entry and finds the
  network occupant at the first cell (traffic network) that carries the route's direction, places the automaton on
  it (slot 132) and, when the route has one cell, finishes it (flag 4). The look-ahead deque is refilled to the
  count slot 175 asks (`0x00554870`), each entry through slot 167/168; an entry that cannot be made stops the fill.
  `SetRouteInfo` copies a route deque and returns to the default speed (slot 160).
- **Outside connections.** `FindBestNeighborConnection` (`0x00554EA0`) chooses, among the region's neighbour
  connections of the network types 3, 0x0C, 8, 2 and 0x0A, the best one for the trip *[summary]*.
- **Leaders.** `FindLeader` (`0x00551C20`): with fewer than two route entries, the nearest automaton ahead in the
  current tile, or else in the next tile in the direction of travel; otherwise the first tile of the route that
  has one. `FindLeaderInTile` keeps the automata of the tile that are not removed (slot 85 of the other), that the
  automaton may follow (slot 73) and that drive straight (flag `0x2000`) or travel the same lane bits, and returns
  the nearest one (3D distance, the vertical part ignored).
- **Collision** (`UpdateCollision`, `0x0054E1A0`): with flag 8 the leader is refreshed when it is gone or flag
  `0x10` is set. With flag `0x800`, or flag `0x200000` while the occupant flag 4 is clear, the automaton is not
  blocked (flag `0x10` cleared) and returns to its default speed when it has no leader. In behaviour `0x0D` nothing
  is done. Otherwise the traffic network is asked whether the way ahead is taken (traffic network slot 33 with the
  tile and the collision position); taken: flag `0x10` on, and an automaton with flag `0x20` that met result 1 enters
  state `0x0D` (wait) for a tuning time; then it slows behind the obstacle (slot 159). Not taken: flag `0x10` off
  and the default speed (slot 160) — unless it was blocked and still too close (`0x00A9BCE8`).
- `HandleExitState` (`0x005535D0`) stops the traffic effects (`cSC4TrafficEffectManager::StopEffect`) and deletes
  the dynamic particle system of the state; `HandleEnterState` starts them *[summary]*.

## 3. The vehicle *[compat]*

- `Init` (`0x0055B9B0`) sets its flags and calls the traffic automaton's `Init`; `DoStateTick` adds states 14 and 21
  (automata.md §6.2); `DoPathTick`, `DoPlummetTick`, `DoPullOverTick` are automata.md §6.5.
- **Collisions.** `HandleCollision` (`0x0055C8C0`) posts `0x49E64802`, picks a random direction, sets flag
  `0x10000000` and enters a 5000 ms state. `ResolveCollision` and `UpdateAfterDamage` (`0x0055F3F0`) report an
  accident to the traffic automata manager (`CreateAccident`), set flag `0x08000000`, raise an event (slot 102) and
  replace the model by its damaged form *[summary]*.
- `UpdateMotionBlur` (`0x0055DD50`) loads the motion-blur model (`GetAnimatedModelFromResourceKey`) and fades it in
  with the speed *[summary]*.
- **Player driving** (U-Drive-It, drive-tool.md): `TranslatePlayerDriveCommands` turns the commands into the
  steering and throttle tweens; `PlayerDriveUpdateSpeed` applies them; `PlayerDriveGetNetwork` finds the network the
  vehicle is on (network mask `0x40341004`); `DoPlayerDriveTick` (4.3 KB) moves it along the network, plays the AEMS
  engine and horn events and collides; `DoPlayerDriveOffRoadTick` moves it off the network;
  `ChooseNewPlayerDriveDestination` picks a target about four cells away *[summary]*.

## 4. Trailers

A trailer vehicle drags its cars along the path of the head (`FindPointOnPathSegment`: the point on a path segment
at the car spacing, by segment intersection); its record lists the cars and the path cells (automata.md §6.4,
AM10) *[summary]*.

## 5. Trains

- `Init` (`0x00557A20`) makes the cars (`cSC4TrainCar::Init`, flag `0x10000000`), with random lengths and
  colours *[summary]*; `Shutdown` releases them.
- `StartTrip` (`0x005588D0`) builds the trip and posts `0x0C05E4CD`; a train that cannot start reports an accident
  *[summary]*.
- `DoAnimationTick` (2.3 KB) moves the cars behind the engine and plays random AEMS sounds; `HandleCollision`
  enters a 5000 ms state with a random direction.
- **Player driving**: `DoPlayerDriveTick` and `PlayerDriveCheckForDerail`: going too fast through a curve (a random
  draw against the speed) **derails** the train (flag `0x08000000`, an event, the derail sound) *[summary]*.

## 6. The tank

`Init` (`0x0054C880`) loads the turret model and the shot effect (`tankshot`, `fx_00_tankshot`), speed 0.2;
`HandleEnterState` sets its driving parameters (0.4, 2π, 30, 100, effect `0xAC0ECE77`, 2000);
`TranslatePlayerDriveCommands` (1.9 KB) aims the turret and fires: a shot is an effect at the aimed point that
damages what it hits *[summary]*; `DoPlayerDriveTick` plays the AEMS events. Its record adds two dwords.

## 7. Emergency vehicles *[compat]*

### 7.1 The base

- **Dispatch states** are dispatch.md §4.3. `SetDispatchState` (`0x0052FF60`) also: state 1 sends the AEMS event
  `0x6A451908` (siren off) and the **path-blocked message**; state 3 enters state 100, sets flags `0x80`, `0x10000`,
  `0x800` and `0x1000`, turns the siren on (slot 194(1)) and starts the siren sound (AEMS event `0xEA451903` with
  `0x29DE9021` for automata of group `0x4228`, `0x09DC5728` for group `0x4202`); state 6 clears those flags but
  `0x10000` and turns the siren off; states 0, 2, 4 and 5 send `0x6A451908`.
- **Path-blocked message** `0xCA8DCDB4` (posted): data1 = slot 192 (the call number *[inferred]*), data2 = the
  re-path attempts, IGZUnknown = the vehicle's occupant.
- `HandleDispatched(target, home)` (`0x005329B0`): stores the target, clears flag `0x400` (and its speed modifier),
  keeps the home station, and when the route's last cell has no road extends the route to a nearby road
  (`AppendRouteToNearbyRoad`, the nearest road cell by squared distance).
- `DoEnRouteTick` (`0x0052FD50`): moves (path or straight line); at the end of the route, arrives (slot 79) or is
  removed; while it holds a valid network occupant it keeps its leader fresh; otherwise it is removed.
- `DoRepathingTick` (`0x0052FE50`): a new path to the target (vehicle manager slot 17, mode 3); success → state 3
  and back to the previous state, attempts := 0; failure → attempts + 1, and while it is at most the tuning byte
  `0x00B21CB9`: the path-blocked message and a wait state 7 for a tuning time; past that it is removed.
- `GetDebugExtraText` appends one of " [at station]", " [route blocked]", " [done]", " [en route]",
  " [no resources]", " [handling event]", " [returning home]" and " repath attempts: %d".

### 7.2 The fire truck

- `GetFireCount` (`0x00533410`): the number of fires in the city (the fire disaster `0x621CDA33`, interface
  `0x49B43F57`) and the number of burning occupants in a box around the truck. `GetToxicSpillCount`: the same for
  toxic spills (`0x4C1E8345`).
- `DoHandlingEventTick` (`0x005335F0`): in state 2 without a crew and with fires or spills nearby → state 5; with
  none nearby, state 5 and not in behaviour `0x10` → state 2; in state 2 with no fire and no spill left at all → go
  home (slot 29).
- The **fire crew**: `CreateFireCrew` makes a `cSC4SimFireCrew` (pedestrian, with a competence) and sets it up;
  `ReleaseFireCrew`; `SetDispatchState` creates or releases it with the state; U-Drive-It commands deploy and call
  it back (`HandlePlayerDriveCommand`, `TranslatePlayerDriveCommands`); `GetStreamTargetPoint` gives the water
  target.

### 7.3 The patrol car and riot police

- The patrol car's `SetDispatchState` creates or releases **riot police** (a search radius of 20, a 2000 ms
  delay); its arrest tick is automata.md §6.5.
- `cSC4RiotPolice` (a pedestrian): `Init` chooses a random model (`0x29A5D1EC`/`0x49A593E7`) and the
  `riot_police_repulsor` effect and announces itself (`0xA9E3B6E2`); `SetupRiotPolice` (1.5 KB) places the squad
  around the patrol car and the rioters (`0xAA0F3F75`) *[summary]*; `DoAnimationTick` reports (`0x8A0F3F7B`) and
  swaps models at random.

## 8. Bus and getaway van

- The bus's `DoStopAtStationTick` (`0x00524180`) stops at a station, picks the pedestrians to board or leave at
  random from the occupants of group `0x496E5783` nearby, then continues (the base stop tick).
- The getaway van's `PostInit` (`0x005339B0`) sets its speed (5) and a random model *[summary]*.

## 9. Quirks

1. A traffic automaton that is blocked but no longer too close keeps flag `0x10` only until the next collision test.
2. The emergency base sends the siren-off event for states 0, 2, 4 and 5 even when the siren was never on.

## 10. Open questions

See [questions/vehicles.md](questions/vehicles.md) (VH1–VH2).

## 11. Conformance tests

1. An emergency vehicle that fails to re-path more than the tuning byte `0x00B21CB9` times is removed, after that
   many path-blocked messages.
2. `SetDispatchState(3)` on an emergency vehicle of group `0x4202` (not `0x4228`) starts the siren sound
   `0x09DC5728`.
3. A fire truck in state 5 with no fire and no spill nearby moves to state 2; with none in the city it then goes
   home.
4. `FindLeaderInTile` returns the nearest eligible automaton of the tile, ignoring height.
