# Spec: pedestrians (`cSC4Pedestrian`, `cSC4PedestrianOneShot`), fire crews and the traffic effect manager

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`). Implementers must work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult
decompiled or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[summary]* is a correct outline whose details are not given.

Related specs: [automata.md](automata.md) §5.1 (the pedestrian manager), §6.4 (the pedestrian record), §6.5 (the
per-state behaviour table: `BEE_LINE`, `CROWD`, `QUEUE`, `STRIKE`, `RIOT_LEADER`, `RIOT_FOLLOWER`,
`IGNITE_OCCUPANT`), §11 (slot names); [vehicles.md](vehicles.md) §2 (the traffic automaton) and §7.2 (the fire
truck, which creates fire crews); [disasters.md](disasters.md) §5.11 (the fire occupant) and §6.5–§6.6 (riots);
[automata-support.md](automata-support.md) (the automata tuning values `T+offset` used below);
[traffic-simulator.md](traffic-simulator.md) (accidents).

## 0. Summary

Pedestrians are traffic automata that walk. Their per-state behaviour is in automata.md §6.5; this spec adds what
the pedestrian class itself does around it: its speed, its flags per state, its collision push, its crowd
reaction, the one-shot variant that plays an animation a few times, and the fire crew, a pedestrian that a fire
truck puts on the scene to hose a fire. It also covers the small manager that attaches the accident controller to
road occupants.

## 1. Classes *[compat]*

| Class | Class ID | Record |
|---|---|---|
| `cSC4Pedestrian` | `0x896E75AF` (`0x00546030`) | automata.md §6.4 (`u16` 5, Write `0x00544650`, Read `0x005446C0`) |
| `cSC4PedestrianOneShot` | `0xAA09DC28` (`0x005468C0`) | the pedestrian's (it overrides only `Init`, §3) |
| `cSC4SimFireCrew` | `0x4A222E4D` (`0x0054A430`) | §4.5 |
| `cSC4TrafficEffectManager` | — | none (§5) |

## 2. The pedestrian *[compat]*

### 2.1 Construction and Init

The constructor (`0x00545F20`) builds a traffic automaton with occupant type `0x496E5783`, clears the pending
push (§2.4) and its collision timer, and sets the **default speed**: a uniform random integer between `T+0x18` and
`T+0x1C` (tuning `0x4998C905`, shipped 15 and 20) × 1/3600, which also becomes the speed tween's target.

`Init` (`0x005444F0`): on the first call only, virtual slot158; then the traffic automaton's `Init`. For the concrete pedestrian, the primary table at `0x00A9A718` maps slot158 to `0x00544620`: draw one uniform double in the range `T+0x120` to `T+0x124` from the automata RNG (`0x00B21D78`) and store the result as f32 at `+0x200`. The field's semantic name is not established; this specifies its value without guessing a name. Bounds are promoted from f32 to double before the draw. The first-call test is the state interface's slot5 at `this+4` (`0x005444F9`).

### 2.2 The animation tick (`0x00544580`)

It reads flag `0x20` before the tick, runs the traffic automaton's tick (vehicles.md §2), and when the automaton
moves (slot 86) compares the flag before and after: newly set → slot 176; set before → slot 19 (`PlayAnim`) with
0 (the walk animation again *[inferred]*). It returns the traffic tick's result.

### 2.3 State flags (`HandleEnterState`, `0x00546A10`)

When the traffic automaton's enter hook does not handle the state:

| Entered state | Flags (on / off) | Then |
|---|---|---|
| 1 `DISAPPEAR` | `0x2000`, `0x4000`, `0x20000` on | |
| 2 `BEE_LINE`, 3 `CROWD` | `0x2000` on, `0x4000` off | |
| 4 `QUEUE` | 2, `0x2000` on, `0x4000` off | |
| 6 `STRIKE` | 2 on, `0x2000` off | re-path (slot 134) |
| 7 `IDLE` | none | |
| 8 `RIOT_LEADER` | the riot animation `0x3AB60B61` (`0x004FDF90`); 2, `0x2000`, `0x10000`, `0x1000000`, `0x2000000`, `0x10000000` on, `0x4000` off | the first leader step (`DoRiotLeaderTick` with "start") |
| 9 `RIOT_FOLLOWER` | the riot animation; 2, `0x2000` on, `0x4000` off | |
| any other | `0x4000` off, `0x2000` off | re-path (slot 134) |
| none (empty stack) | `0x2000` off, `0x4000` off | |

`DoStateTick` (`0x00546BC0`) does nothing with an empty state stack; otherwise the traffic automaton's state tick
first, and when that does not handle it the pedestrian ticks of automata.md §6.5 by state: 2 `DoBeeLineTick`,
3 `DoCrowdTick`, 4 `DoQueueTick`, 6 `DoStrikeTick`, 8 `DoRiotLeaderTick`, 9 `DoRiotFollowerTick`,
12 `DoIgniteTick`.

### 2.4 Walking with collisions

`MoveForwardWithCollision(dir, dt)` (`0x00545D60`): the step is the speed modifier (slot 125) × the current speed
(slot 34) × dt along the normalised direction. Unless flag `0x20000` is set, the new position is put on the ground
by the traffic network map (city slot 40, slot 44, flags `0xC49`). When the move collides (below) and a push is
pending and non-zero, the push is added, the position put on the ground again and tested again; a move that still
collides is not made, otherwise the pedestrian moves there (slot 16). The pending push is then cleared. The result
is "moved".

`TestCollisionExpensive` (`0x00545840`) runs at most once per 500 ms of accumulated time (between tests it reports
no collision). It asks the occupant manager for the automata (occupant type `0x496E5783`) in the box from
(x − 1, y, z − 1) to (x + 1, y + 2, z + 1) around the new position. Each other automaton nearer than 1.0 gets a
pending push away from this one (the vector from this position to it), or a random push with x and z uniform in
[−0.5, 0.5] and y 0 when it is nearer than 0.01. Any such hit is a collision.

`DoBeeLineTick` (`0x00546150`) walks along the stored bee-line heading when it is not zero, then the behaviour
refresh (slot 105); it always returns 1.

### 2.5 Crowd reactions (`HandleCrowdReactionEvent`, `0x005448D0`)

Ignored (false) while a flag (slot 31) is set. Otherwise the reaction kind (`+0x254`) becomes 1 when the event's
mode is non-zero and 2 when it is 0, the speed tweens back to the default speed (slot 36), and the pedestrian turns
toward the event (slot 16) *[summary of the turn]*; the result is true. `DoCrowdTick` plays the reactions
(automata.md §6.5).

### 2.6 The other ticks *[summary]*

- `DoQueueTick` (`0x00545690`): with a controller, the controller's queue step (controller slot 28, automata.md
  §3.3) and a straight-line move; with flag `0x20` it faces the queue head and plays the animation `idle`
  (slot 21); then slot 105. Without a controller it pops the state.
- `DoStrikeTick` (`0x00544950`): with a controller target, re-paths toward a box around it that shrinks by one cell
  on each side; once inside, or when no path is found, it returns to its source occupant (slot 29).
- `DoRiotLeaderTick` (`0x00546220`) and `DoRiotFollowerTick` (`0x00546530`): disasters.md §6.5 and automata.md
  §6.5; both walk with `MoveForwardWithCollision` and random turns, the follower toward its leader (IID
  `0xA9B40F05`) past 10 units.
- `DoIgniteTick` (`0x00545280`): the controller of the newest state 12 in the stack gives the target; the fire
  disaster (`0x621CDA33`, interface `0x49B43F57`) ignites it with cause 2 (arson); then `SetState(1)`
  (disasters.md §6.6).

### 2.7 Base code reached through the pedestrian

The automaton and traffic-automaton functions that the pedestrian's tables reach first (the small getters, the
state-record writer `0x0050EF50` of automata.md §6.2, the traffic `SetDestination` `0x0054E5B0`, the path tick
`0x0054FDC0` of slot 129, and the traffic slots 131–134 at `0x0054E920`, `0x00550760`, `0x005504E0`,
`0x00550B60` with their helpers `0x00550F40`, `0x005510D0`, `0x00551300`) are base-class code: their behaviour is
vehicles.md §2 and automata.md §6.5 *[summary]*.

## 3. The one-shot pedestrian (`cSC4PedestrianOneShot::Init`, `0x00544DA0`) *[compat]*

On the first call slot 158, then the traffic automaton's `Init`; when that succeeds, every model instance gets
the value 0.01 (instance slot 14) *[inferred: an animation rate]*, and the **play count** is drawn uniformly between
two bounds: `T+0x13C` and `T+0x13D` (tuning `0x6A09F630`, shipped 3 and 10), replaced by the two `u8` values of
property `0xEA958845` of the automaton's exemplar when it has at least two.

## 4. The fire crew (`cSC4SimFireCrew`) *[compat]*

### 4.1 Creation and model

The fire truck creates the crew and gives it a **competence** (`SetCompetence`, `0x00546CB0`; vehicles.md §7.2).
`Init` (`0x00549890`), once:

1. When the truck's dispatch state (truck slot 26) is 5 (on the scene), the **competence level** is the first i
   in 0, 1, 2 with `T+0x108`[i] ≤ competence (tuning `0xA9D6CC18`, shipped 1.0, 0.6, 0.0); without a match it is 3
   *[quirk: only 0–2 index the tables]*. Level 2 sends crowd message `0xA9E3B6E2` with (−1, `0x40`). The crew gets
   two models (type `0x5AD0E817`, group `0xBADB57F1`): instance `0x21010000`, `0x21010000`, `0x21030000` and instance
   `0x21020000`, `0x21020000`, `0x21040000` for levels 0, 1, 2.
2. Otherwise one model at random of `0x21050000`, `0x21060000`, `0x21070000` (the walking crew).
3. Flags `0x2000` and `0x4000` on, the pedestrian's `Init`, the hose registration points (`InitRegTranslationArray`,
   graphics-formats.md "Consumer 2"), and a registration for message `0x99EF1143`.

### 4.2 Setup on the scene (`SetupFireCrew`, `0x00547EF0`) *[summary]*

With the truck on the scene, the crew is placed beside the truck: along the truck's direction from truck slot 143
(its heading when that is zero) when the truck is not player-driven, or on a random side (a fixed turn left or
right of the heading) when it is; at the ground height, with its start and target positions recorded. It then sends
message `0xAA0F3F75` to the truck with the level's value (`0x29DE8FD6`, `0xC9DE8EE4`, `0xC9DE8EE4` for levels 0, 1,
2). Off the scene it is placed from the truck's transform. Both end with a fade-in: the opacity is set to 0 and
tweened to 1.0 over `T+0xBC` (slot 33).

### 4.3 The animation tick (`0x00549A50`)

1. The pedestrian's tick.
2. When the crew is not marked for removal and the truck is player-driven (state 16 in its stack) or on the scene:
   - the target is the truck's stream target point (`cSC4FireTruck::GetStreamTargetPoint`) or, without one, the
     nearest burning occupant within `T+0x114` (tuning `0xC9EE640C`, shipped 128; `FindTargetsInRadius`,
     `0x00549000`, which also takes the fire occupant `0x69D25C60` as the crew's fire);
   - **no target**: the hose effects stop; once, after at least one fire was handled, the crew sends crowd message
     `0xA9E3B6E2` with (1, `0x50`); then it goes back to the truck (truck slot 199) or, without a truck, is
     removed;
   - **a target**: the crew turns toward it, creates the two hose effects from the effect manager when it has none,
     and aims them at the target; at level 2 the aim wanders by a uniform ±75 on x and z, smoothed by a factor of
     0.002 × the frame time; the first effect restarts when it stops; the second starts once the hose has run for
     0.8 s (time += frame time × 0.001).
3. Otherwise it sends message `0x8A0F3F7B` to the truck with the level's value and stops the hose effects.
4. When the truck's dispatch state is 1 and the crew's first model has reached its last frame, that model is
   replaced by a random walking model (step 4.1.2).

### 4.4 Hosing and messages

The crew counts on its fire: assigning a fire adds the crew's strength, 2 (1 at level 2), to the fire occupant's
hosed count (fire slot 12 `AddHosed`, disasters.md §5.11) and to the target occupant (slot 5 of interface
`0xABF98CB3` *[inferred: the same count on the burning building]*); releasing the fire takes it away. The fire is
released when the crew shuts down (`0x00547D90`, which also sends crowd message `0xA9E3B6E2`, stops and releases
both hose effects and drops the message registration) and on message `0x99EF1143` naming the fire or the target
(`0x00548DC0`), which also stops both hose effects (with the arguments 1 and 0). `SendFireCrewMessage`
(`0x00546D20`) posts a `cRZMessage2Standard` of the given type with the given data to the message server.

### 4.5 Record (Write `0x00547BD0`, Read `0x00547C80`)

`u16` 2 (Read requires 2), the pedestrian record, `ref` the truck (IID `0xA9B40F05`), `f32` competence, `u8`
level, `s32`, `bool`, `vec3`, `f32`, `vec3` *[as read: the hose and position state]*.

## 5. The traffic effect manager (`cSC4TrafficEffectManager`) *[compat]*

It keeps a list of effects on road occupants.

- `CreateAccident(occupant, …, duration)` (`0x00554040`): appends an entry with the duration, the centre cell of
  the occupant's cell rectangle and the occupant, starts it (`0x0054EC60`), attaches the automata controller
  **`traffic_accident_2`** to the occupant (city slot 47, controller manager slot 10; the result is kept as
  "attached") and keeps a reference to the occupant.
- `StopEffect(entry)` (`0x00552B80`): when the entry is in the list, an attached entry removes the occupant's
  controllers (controller manager slot 14, `0x0054ECC0`), the entry is unlinked and its occupant released.

The traffic automaton's exit hook stops its effects (vehicles.md §2).

## 6. Quirks

1. A competence below every threshold (possible only when a tuning list does not end with 0) gives level 3, which
   reads past the three-entry model tables.
2. `TestCollisionExpensive` reports no collision between its 500 ms tests, so most steps never collide.

## 7. Open questions

See [questions/pedestrians.md](questions/pedestrians.md) (PN1–PN2).

## 8. Conformance tests

1. With tuning `0x4998C905` = (15, 20), a new pedestrian's default speed is one of 15/3600 … 20/3600.
2. A fire crew of competence 0.7 with the shipped thresholds (1.0, 0.6, 0.0) has level 1 and models `0x21010000` and
   `0x21020000`; one of competence 0.5 has level 2, models `0x21030000` and `0x21040000`, and hoses with strength 1.
3. Entering state 4 sets flags 2 and `0x2000` and clears `0x4000`; entering state 7 changes no flag.
4. Two pedestrians 0.005 apart: the first one's test gives the second a push with |x|, |z| ≤ 0.5 and y = 0.

### PR review conformance: initial random parameter

For concrete `cSC4Pedestrian` with equal tuning bounds -0.5 (the shipped values of property `0x29DD40C1`), first initialization performs one double-range draw and stores f32 -0.5 at+0x200 before the base Init. An already initialized object skips this draw. For unequal bounds, use the existing RNG contract; do not substitute the default-speed integer draw from section2.1. Overrides of slot158 retain their own behavior. The identity is byte-proved by the constructor table store `0x00545F4A`, table entry+0x278, and body `0x00544620`-`0x0054464A`.

The primary tables of `cSC4PedestrianOneShot` (`0x00A9AB50`, first entry `0x00544780`) and `cSC4SimFireCrew` (`0x00A9B488`, first entry `0x00547920`) also map slot158 to `0x00544620`, so the same initial-parameter operation applies to these two inherited calls.
