# Spec: aircraft (`cSC4Aircraft`, `cSC4Airplane`, `cSC4Helicopter`) and the air dispatch sources

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`). Implementers must work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult
decompiled or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[summary]* is a correct outline whose details are not given.

Related specs: [automata.md](automata.md) §5.2 (the aircraft manager), §6.4 (the records), §6.6 (the aircraft
state families and the two `AI_PursueGoal` dispatchers), §11 (slot names); [dispatch.md](dispatch.md) §4.3
(dispatch states 3, 4, 5, 6, 0 set by aircraft); [protection.md](protection.md) §8 (dispatch sources);
[drive-tool.md](drive-tool.md) (U-Drive-It); [audio.md](audio.md) (AEMS events); [vehicles.md](vehicles.md).

## 0. Summary

Aircraft are automata that fly: a base class with the altitude and world-clip helpers, the **airplane** (a fixed
wing flight model with runways: taxi, take-off, cruise, approach and landing; fire planes drop water) and the
**helicopter** (a rotor model with hover, climb over buildings, a spotlight and weapons in U-Drive-It). Both take
their handling from an 18-value **drive-parameter list** of their automata exemplar. Two dispatch sources let
airports and police stations with heliports send fire planes and police helicopters.

## 1. Classes *[compat]*

| Class | Class ID | Notes |
|---|---|---|
| `cSC4Aircraft` | `0xC96E75DC` | base (automata.md §6.4) |
| `cSC4Airplane` | `0xCBEB4F62` | §3; record with the home position |
| `cSC4Helicopter` | `0x8BA98EDA` | §4; record with the home position |
| `cSC4FirePlaneDispatchSource`, `cSC4PoliceHelicopterDispatchSource` | — | §5 |

## 2. The aircraft base

- `GetAltitude(x, z)` (`0x004FB0B0`): the height of the ground or water under the point, raised by the buildings and
  props there *[summary]*.
- `ClipToWorld(p)` (`0x004FB180`): keeps a point inside the city, with a margin of −1000 (so slightly outside the
  map is allowed) *[summary of the bounds]*.
- `DoPlayerDriveTick`: the automaton's, then the aircraft's own controls (§3.4, §4.4).

## 3. The airplane *[compat]*

### 3.1 Drive parameters (`LoadDriveParameters`, `0x004FC910`)

The values come from the automaton's exemplar data, or its parent group's when it has none (a walk up the
`+0x0C` parent chain) *[inferred: an exemplar property]*. A value of 0 takes the default. Entries 2 and 3 are
multiplied by 1000 when they are given (their defaults, 25 and 150, are not).

| # | Default | Derived default when 0 | # | Default | Derived default when 0 |
|---|---|---|---|---|---|
| 0 | 100 | | 9 | | entry 8 / 12 |
| 1 | 1 | | 10 | 10000 | |
| 2 | 25 | | 11 | | entry 10 |
| 3 | 150 | | 12 | 0.4 | |
| 4 | 0.2618 (15°) | | 13 | | entry 12 × 0.1 |
| 5 | | entry 4 × 0.25 | 14 | 0.5236 (30°) | |
| 6 | 0.7854 (45°) | | 15 | 0.08 | |
| 7 | | entry 6 × 0.5 | 16 | 0.68 | |
| 8 | 0.5236 (30°) | | 17 | 0 | |

With no list at all the defaults above are used, with 5 = 0.06545, 7 = 0.3927, 9 = 0.04363, 11 = 10000, 13 = 0.04.
`LoadPlayerDriveParameters` loads a second set for U-Drive-It *[summary]*.

### 3.2 Init and effects

`Init` (`0x005017C0`) loads both parameter sets, builds the three ray probes (`0x004FF840`, `0x004FFAE0`,
`0x004FFD80`), and the effects: the propeller (`fx_00_planepropstill` / `fx_00_planeprop`, or the fire-plane
versions `fx_00_fireplanepropstill` / `fx_00_fireplaneprop`), sky writing (`fx_00_skywriting`), the water drop
(`fx_00_waterdrop`), the indicator (`aircraftindicate`) and the bomb (`jetbomb`, U-Drive-It).

### 3.3 AI goals (`AI_PursueGoal`, `0x00504590`; automata.md §6.6)

The goal stack drives a set of behaviours, each a steering routine over the flight model:

- `AI_GotoDestination` (fly to a point: take off first when on the ground, turn to the vector, follow it, keep the
  approach altitude over the ground); `AI_Land` (pick a runway direction and an approach box, descend along the
  approach ray to the base altitude, touch down, `SetAirplaneState`, stop, taxi); `AI_TakeOff` (pick a runway
  direction, taxi to its start, turn, roll and climb); `AI_Taxi`, `AI_Stop`, `AI_TurnToVector`, `AI_FollowRay`,
  `AI_FollowVectorAndTurn`, `AI_ApproachAltitude` *[summary of each control law]*.
- **Runways.** `PickRunwayDirection` (`0x00502520`) takes the airport's runways from the aircraft manager
  (`GetRunwayList`) and picks the direction with the best clearance (`EvaluateClearance`: the altitude along the
  approach against the airplane's height); `PickApproachBox` places the approach start up to 500 units out, clipped
  to the world.
- **Fire planes.** `FindFireNearLocation` finds the nearest burning occupant near the target; the plane flies over
  it and drops water *[summary]*.
- Dispatch states: dispatch.md §4.3 (3, 4, 5, 6, 0).

### 3.4 Flight model *[summary]*

`UpdatePhysics` (`0x00503CB0`) integrates `CalculateForces` (thrust, lift, drag and the control surfaces, 3 KB),
keeps the plane above the ground (`GetAltitude`) and on the runways when rolling, detects collisions
(`DetectCollisions`: buildings and other automata in its path; a crash plays its AEMS event), clips to the world and
changes the airplane state (`SetAirplaneState`, which starts and stops the engine sounds). `DoAnimationTick` runs
the goals and the physics; in U-Drive-It, `TranslatePlayerDriveCommands` turns the player's commands
into control inputs and drops `jetbomb` bombs.

## 4. The helicopter *[compat]*

- **Parameters** (`LoadDriveParameters` `0x005340A0`): the same list read into the rotor model; angles are scaled
  by 572.958 and one rate by 1000 × 3.333 *[summary of the twelve entries]*.
- `PhysicsInit` and `InitInstance` set up the rotor model; `NextFrame` (`0x005397C0`) computes the engine power,
  then yaw, pitch, roll and slide (`YawPitchRollSlideReal`, with limits ±890 and 250), the
  horizontal move (`MoveXZReal`) and the vertical move (`MoveYReal`, kept above `GetAltitude`) *[summary]*.
- **AI controls**: `AI_ControlReset`, `AI_ControlHover` (bounds −150 and 100 *[inferred: vertical rates]*),
  `AI_ControlMoveXZ` (bands ±10 and ±20 *[inferred]*), `AI_ControlLiftOverBuildings` (climbs when the ground ahead
  is higher), and `AI_PursueGoal` (automata.md §6.6, including the chase of case `0x34`).
- **Spotlight** (`UpdateSpotlight`, 2 KB): the light cone follows the ground below or the target *[summary]*.
- **States** (`SetHelicopterState`): each starts or stops its AEMS rotor sounds.
- **U-Drive-It**: `OnPlayerTakeOver` loads the player parameters; `TranslatePlayerDriveCommands` (2.4 KB) flies it
  and fires `helicoptermissle` or, for the UFO, `ufolaser`.

## 5. The air dispatch sources *[compat]*

`cSC4FirePlaneDispatchSource` and `cSC4PoliceHelicopterDispatchSource` track their stations like the ground sources
(protection.md §8): when a building occupant (`0x278128A0`) with a non-zero unit count (property `0x0C0AFDD7` for
fire planes) is inserted, it is added to the source's list, its units are numbered after the other stations' in a
bit vector, and its available-units property (`0xCC0AFDDB`) is set to all units (`2^n − 1`); removal reverses it
*[summary of the removal]*.

## 6. Quirks

1. `ClipToWorld` allows positions 1000 units outside the city.

## 7. Open questions

See [questions/aircraft.md](questions/aircraft.md) (AF1–AF2).

## 8. Conformance tests

1. An airplane exemplar without drive parameters gets 100, 1, 25, 150 and the angle defaults of §3.1; one that
   gives entry 2 as 3 gets 3000.
2. An entry 5 of 0 with entry 4 = 0.4 gives 0.1.
3. A fire-plane station with a unit count of 3 sets its available-units property to 7.
