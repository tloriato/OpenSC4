# Spec: watercraft (`cSC4Watercraft`): boats, ferries, tugs and U-Drive-It boats

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`). Implementers must work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult
decompiled or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[summary]* is a correct outline whose details are not given.

Related specs: [automata.md](automata.md) §5.3 (the watercraft manager record), §6.4 (the boat record), §6.6 (the
state family 70–78, the crash cases and the idle and crash accumulators), §7.2 (the manager's `CreateAutomaton`
override with its spacing test), §11 (slot names); [drive-tool.md](drive-tool.md) (U-Drive-It);
[audio.md](audio.md) (AEMS events); [aircraft.md](aircraft.md) (the same drive-parameter scheme).

## 0. Summary

A watercraft is an automaton that moves on water cells. An AI boat wanders: it keeps a short list of water cells
ahead and steers along it. A ferry shuttles between two stations along a stored route and docks at each one, with a
berth and a waiting spot per station. A boat with a neighbour route sails a route to the edge of the map. Tugs in
U-Drive-It can throw a rope and tow another boat. Collisions with other boats, the shore and buildings make
player-driven boats sink or run aground and make AI boats fade out. Handling comes from a 40-value drive-parameter
list in the boat's automata group.

## 1. Class and fields *[compat]*

| Class | Class ID | Record |
|---|---|---|
| `cSC4Watercraft` | `0x096E75F5` (`GetGZCLSID` `0x0056DC60`) | `u16` 2 + the automaton base (automata.md §6.4; Write `0x00566EE0`, Read `0x00566F00`) |

The boat's own state is runtime only (not saved). The names below come from the debug text (§9) where it has them.

| Offset | Meaning |
|---|---|
| `+0x1E4`, `+0x218` | ferry station A and B (a station record: the station lot, its occupant, the berth position at `+0x10` and the berth direction at `+0x1C` of the record) |
| `+0x215`, `+0x249` | the boat has a dock request at station A, at station B |
| `+0x24C` | the station of the current crossing, with its compressed route (`+0x250`) and start position (`+0x25C`) |
| `+0x274` | the boat holds a berth at the current station |
| `+0x278`, `+0x27C`, `+0x27D` | route cells already used, route started, ferry mode |
| `+0x280`, `+0x284` | the boat it tows; the tug towing it |
| `+0x288` (D), `+0x289` (Q) | docking, queued |
| `+0x28C` | the ferry route reader |
| `+0x2A4`–`+0x2AC` | the **travel points**: a vector of 16-byte entries (`s32` cell x, `s32` cell z, `u8` tried-direction mask, `u32` direction 0–7) |
| `+0x2B0` | docking in progress |
| `+0x2B2` | sunk by a collision |
| `+0x2B4`, `+0x2B8` | lifetime limit (0 = none) and lifetime elapsed, ms |
| `+0x2BC`, `+0x2C0` | idle and crash accumulators (automata.md §6.6) |
| `+0x2CB`, `+0x2CC` | being towed; count the idle budget in every tick (set outside this class *[inferred]*) |
| `+0x2D0`, `+0x2D4` | time without moving; time going backwards, ms |
| `+0x2D8` | tow armed |
| `+0x2DC` (Speed), `+0x2E0` (Turn) | the current speed and turn |
| `+0x2E8`, `+0x2F4` | previous and current position (Pos) |
| `+0x300`, `+0x30C` | previous and current heading (Ornt), unit vectors |
| `+0x318`, `+0x31C`, `+0x328`, `+0x340` | the neighbour route: its network occupant, route bytes, reader and the "has a neighbour route" flag |
| `+0x344`–`+0x3E0` | the 40 drive parameters (§2) |
| `+0xDAC` | the U-Drive-It gear: 0 neutral, 1 reverse, 2 forward |
| `+0xDB0`–`+0xDC4` | six effects (§3) |

## 2. Drive parameters (`LoadDriveParameters`, `0x00567310`) *[compat]*

The list comes from the boat's automata group (slot 67, list at group `+0x60`) or, when the group has none, from its
parent chain (`+0x0C`, as for aircraft). Entry *k* is stored at `+0x344 + 4k`. A given value of 0 takes the
default. Message `0x26D31EC1` (§7) reloads the list.

| # | Default | Use | # | Default | Use |
|---|---|---|---|---|---|
| 0 | 0.1 | U-Drive-It acceleration factor | 20 | 0.2618 | turn rate (into the drive state, §6) |
| 1 | 0.001 | steering return rate | 21 | 8000 | crash duration, ms (automata.md §6.6) |
| 2 | 1.0 | steering rate | 22 | 0.7854 | |
| 3 | 0.05 | coasting drag | 23 | 0.95 | |
| 4 | 0.2 | | 24 | 2.1817 | |
| 5 | 5.0 | counter-steer boost | 25 | 1.0472 (60°) | run-aground angle (automata.md §6.6) |
| 6 | −0.1 | lowest speed (reverse) | 26 | 10 | |
| 7 | 0.3 | highest speed | 27 | 0.5 | player boat-to-boat crash threshold |
| 8 | −entry 9 | lowest steering | 28 | 0.25 | player shore crash threshold |
| 9 | 2.0 | highest steering | 29 | 0.75 | speed kept after a bump |
| 10 | 0.001 | per-ms scale of the acceleration | 30 | −2000 | |
| 11 | 50 | | 31 | 3000 | |
| 12 | 5.0 | | 32 | 150 | U-Drive-It reverse threshold |
| 13 | 0.001 | | 33 | 0 | U-Drive-It forward threshold |
| 14 | 0.001 | | 34 | 2.0 | |
| 15 | 0.1 | AI boat-to-boat crash threshold | 35 | 100 | longest tick, ms |
| 16 | 0.5 | AI shore crash threshold | 36 | 300 | dock request distance |
| 17 | entry 18 / entry 7 | | 37 | 90 | final approach distance |
| 18 | 0.3142 | | 38 | 1.0 | docking turn rate, degrees per ms |
| 19 | entry 20 / entry 9 | | 39 | 1.0 | docking move rate, units per ms |

With no list at all the same defaults are used, with 8 = −2.0, 17 = 1.0472 and 19 = 0.1309. Uses not given are
not traced *[summary]*.

## 3. Init and Shutdown *[compat]*

`Init` (`0x0056DE40`), once: the automaton's `Init`, the drive parameters, and six effects from the city's effect
manager, in this order: `SMLmushroomcloud`, `boat_grounding`, `boat_splash`, `tugropethrow`, `tugropeconnection`,
`waterski` (`+0xDB0`–`+0xDC4`). It sets four flags, `0x8000`, `0x2000`, `0x20000` and 4, registers the boat for
message `0xAC400A31` (§7), clears the tow, the stations, the travel points and the timers, copies the position and the
orientation into the current and previous position and heading (normalising the headings), and enables the active
sound (slot 77, `EnableActiveSound(true)`).

The initialized test is slot 5 on the interface at `this+4`. If already initialized, return true without any
owner work. Otherwise the base Init (`0x005101D0`) return is **not tested**; continue with parameter loading.
Each of the six existing effect pointers is cleared and Released before its replacement is requested through
effect-manager slot 7, with name and output-pointer arguments. No creation return is tested, no rollback occurs,
and the method returns true even if an output remains null. This is not a promise that later effect users accept
every null pointer. Evidence: `0x0056DE49–0x0056DE69`, `0x0056DEFE–0x0056E02D`, return `0x0056E330`.

All four flag writes pass true, in the order above (`0x0056E02D–0x0056E062`). Clearing timers includes **both**
lifetime limit and elapsed (`+0x2B4/+0x2B8`), still/backwards time, crash time and `+0x2C4`; idle budget is cleared
later, after copying/normalizing the headings. Init clears `+0x2CC` as well as being-towed, collision-sunk,
docking/queue, neighbour-route and tow-armed bytes. Thus restoring a lifetime limit before Init does not preserve
it. Native stores: `0x0056E0F3–0x0056E177`, `0x0056E2A0`.

### 3.1 Serialization boundary and query interface

`Write(stream)` writes the owner version word 2 through stream slot 7, then calls base Write `0x0050AC00` with
the same stream, returning its result. It neither tests the version-write return nor appends boat fields.
`Read(stream)` reads one word through stream slot 7; only exactly 2 calls base Read `0x0050DFD0`. Every other word
returns false immediately, without a base read. The stream-read return is not separately tested. Short-read
behavior therefore belongs to the stream contract; no successful default version is synthesized here.
Evidence: `0x00566EEA–0x00566EFD`, `0x00566F0A–0x00566F2F` (word comparison at `0x00566F14`).

`QueryInterface(iid,out)` (`0x00566F30`) recognizes `0x096E75F5`, returns the complete-object pointer, AddRefs it,
and returns true; other IDs delegate to base QueryInterface. It does not validate a null output pointer.

`Shutdown` (`0x0056D0E0`), once: a towed boat is sent to `SLOW_REMOVE` (73) and released; the tug is released; a
dock request at A or B gives back its waiting spot and berth (`cSC4WatercraftManager::DoneDocking`); the message
registration is removed; the stations, the route occupant and the six effects are released (each effect is stopped
first); then the automaton's `Shutdown`.

## 4. States *[compat]*

`DoStateTick` (`0x00573340`) and the enter and exit hooks extend automata.md §6.6:

| State | Enter (`0x0056E340`) | Tick | Exit (`0x00570140`) |
|---|---|---|---|
| 7 `IDLE` | | the idle budget; above 60,000 ms `PushState(73)` (slot 52) and the same tick also runs the crash tick of 70 *[quirk]* | |
| 16 `PLAYER_DRIVE` | active sound off; in the drive state: `+0x30` := 0, `+0x234` := entry 20, `+0x34` := the group's AEMS record value at `+8` (1.0 without one), `+0x144` := 15, the shot count `+0x178` := 0, `+0x1AC` := 2000.0; end the tow and the docking | | active sound on; end the tow |
| 70, 71 crash | flags `0x10000000` and `0x8000000`; `NotifyAutomatonEvent(0xB, 0)` (slot 102); crash time 0; end the tow and the docking | crash time += dt; `DoCrashSequenceTick` (automata.md §6.6) | |
| 72 `IMMEDIATE_REMOVE`, 73 `SLOW_REMOVE` | flags `0x10000000`, `0x8000000`; `Remove(true)` / `Remove(false)`; end the tow and the docking | the same remove every tick | |
| 74 `FERRY_DOCK` | flag `0x10000000`; D := 1; docking in progress; end the tow | `DoDockingTick` (§5.3); when it returns false, `PopState` | D := 0; switch the active station (§5.4) |
| 75 `FERRY_QUEUE` | flag `0x10000000`; Q := 1; speed and turn 0; end the tow | `Ferry_DockRequests` (§5.2) | flag `0x10000000` off; Q := 0 |
| 76 `MARINA_PARKED` | speed and turn 0; end the tow and the docking | | |
| 77 `BEING_TOWED` | being towed := 1; flag `0x10000000`; end the docking | `DoFollowingTick` (§6.3) | flag `0x10000000` off; the tug's towed-boat reference and the boat's tug are released |
| 78 `UNDOCKING` | | `DoUndockingTick` (§5.5); when it returns false, `PopState` | |

"End the tow" (`EndTowing`, `0x00567E80`): the towed boat pops its state and both references are released.
"End the docking" (`EndDocking`, `0x0056D4D0`): with a current station and a held berth, clear docking in progress,
give the berth back (`DoneDocking`), clear the held flag and flag `0x10000000`.

The owner enter hook first calls base HandleEnterState and preserves its boolean; it still performs the owner
switch if the base returned false. For 70/71 the owner notification arguments are exactly **(0xB,0)**, not the
state duration: zero is pushed at `0x0056E40A`, event 0xB at `0x0056E40B`, slot 102 called at `0x0056E40F`.
This resolves the watercraft-owner argument conflict with automata §6.6; it does not erase notifications emitted
by inherited or collision-entry paths. No duration is substituted into this owner callback.

## 5. AI movement *[compat]*

### 5.1 The animation tick (`DoAnimationTick`, `0x00572CF0`)

The tick takes two binary32 times in ms, A then B. **B drives the watercraft owner's timers, sound and AI motion**;
both A and B are forwarded, unchanged and in order, to `0x00508740`. This corrects the previous inferred split.
Native stack evidence: sound loads B at `0x00572D67`; base arguments at `0x00572E53–0x00572E5B`; after the saved
EDI is popped, backwards time reads B at `0x00572E80`, lifetime at `0x00573165`, motion at `0x00573186–0x005731C5`.
The base boolean is saved in A's now-unused low byte; it is not a time conversion.

1. A speed modifier (slot 125) below 0.00001 ends the tick with true.
2. With `+0x2CC`, the idle budget grows by the second time; above 60,000 ms, `PushState(73)`.
3. A non-zero speed updates the active sound (slot 78).
4. The tow rope (`tugropeconnection`) follows the boat while it tows and stops when it does not.
5. The automaton's step (`0x00508740`); when it fails, skip to 13.
6. A negative speed adds to the time going backwards, otherwise that time is 0.
7. **Wakes.** The boat counts as moving when its speed is not 0, or it moved at least 0.2 × the city's cell size
   (city slot 91, computed once) since the last tick, or its heading changed by at least 0.01. Moving restarts the
   wake effects (`0x0056F1E0`: effect slot 44 for `SMLWake`, `SMLWash`, `MEDWake`, `MEDWash`, `LargeWake`,
   `LargeWash`) and clears the still time; otherwise the still time grows and above 250 ms the wakes are stopped
   (`0x0056EFE0`, effect slot 45 for the same names) *[inferred: which slot starts and which stops]*.
8. A boat still for more than 5000 ms whose state stack holds none of the states 7, 16, 75, 76, 77, 78 (slot 57
   `FindState`) returns false; what the caller does with that is not traced.
9. With state 16, 74, 75, 77 or 78 in the stack, or while being towed, or with `+0x2CC`, skip to 13.
10. **Lifetime.** With a limit that the elapsed time has passed, the manager is asked (manager slot 41) whether the
    boat may stay; without a manager, on a no, or with state 76 in the stack: `PushState(73)`; on a yes the limit is
    cleared and flag `0x10000` cleared.
11. State 76 in the stack, or top state 7: skip to 13.
12. The elapsed time grows. If the boat cannot move (§5.6): on land, `PushState(72)`. Otherwise, with the frame time
    clamped to 0–entry 35: `Engine_AIBoatMotion`, `Movement_PositionNew`, `Engine_DetectCollision`,
    `Ferry_DockRequests` and `Ferry_ClearDockRequests`.
13. The effect and model updates (slots 104 and 107).

All timer additions above use **unclamped B** and store binary32. Only the AI movement block clamps B to
`[0,p35]`, passes the value to `Engine_AIBoatMotion`, then its address to `Movement_PositionNew`. The lifetime
comparison precedes the elapsed-time addition, is strict `limit<elapsed`, and is guarded by `limit!=0`; crossing
the limit by addition therefore takes effect on a later tick. State-stack exclusions precede this test and can
suspend it. Expiry that retains the boat clears the limit and flag `0x10000`, then skips the rest of movement
for that tick. Top-state 7 and a state-76 stack entry suppress the elapsed addition.

The movement/heading tests in step 7 use full three-component Euclidean norms; heading change means
`length(currentHeading-previousHeading)`, **not an angle**. The cached movement threshold is a binary32 store of
city cell size times binary32 0.2. Equality counts as moving. Still time 250 or 5000 exactly does not trigger the
strict greater-than actions. If the base step returns false, only slots 104 and 107 remain and false propagates.
The exceptional still-time false return in step 8 bypasses those two callbacks. All other paths through step 13
return the saved base boolean; the speed-modifier early return is true with no callbacks.
Evidence: norms/threshold branches `0x00572EC7–0x00572FB9`; still rejection `0x00572FB9–0x00573033`;
lifetime/AI block `0x005730B2–0x0057320E`; final return `0x0057320E–0x0057322D`.

The rope-effect path contains an asymmetric null contract: while towing it checks the connection effect before
use; with no tow it directly calls effect slot 5 on `+0xDC0`, stopping with argument false if active
(`0x00572E38–0x00572E50`). A failed acquisition of that effect is thus not generally safe to animate, even though
Init reports success. Do not invent automatic rollback or silently claim native null tolerance.

Independent ordering vectors: with A=1 and B=1000, a permitted non-idle AI boat adds 1000 to elapsed and passes at
most 100 to AI movement under default tuning. With `(limit,elapsed)=(1000,1000)` and B=1 it does not expire that
tick; `(1000,1001)` tests retention before adding time. With still time 5000, B=0, zero speed and unchanged pose,
the 5000-ms removal predicate is false. A base-false vector calls neither wakes nor lifetime nor motion, but calls
slots 104 then 107 once and returns false. A still-time rejection calls neither final callback.

### 5.1a Owned AI motion (`Engine_AIBoatMotion`, `0x00571880`, final round 3)

The bounded instruction contract is in [watercraft-ai-motion.md](watercraft-ai-motion.md): waypoint query/
erase order, midpoint geometry, strict alignment and distance tests, phase latch, heading turn/translation,
Speed/Turn approach, live pose callback and history order. It also identifies exactly how the animation
and docking callers follow AI with Movement_PositionNew, which reaches the R2 geometry helpers below.
The owner computes motion itself; no foreign final-motion callback is substituted. Ordinary finite input
support and residual precision/route/exceptional/shared-setter dependencies are listed separately there.
WC4 remains open; this is no whole-owner, inherited-animation or engine qualification.

### 5.2 Picking cells (`Engine_AIPickLocation`, `0x00570620`)

The travel points are topped up to 10 entries, starting from the last point, or from the boat's cell and the
direction of its heading when the list is empty. A start outside the city ends the pick.

- **Neighbour route** (`+0x340`): the next cell of that route is appended; when the route is used up,
  `PushState(73)`.
- **Ferry** (`+0x27D`): on the first pick the route reader skips the cells already used (`+0x278`, at least 2 at
  the start of a crossing); each pick appends the next route cell and counts it.
- **Wandering.** The boat's radius in cells is 3 in group `0x4414`, 2 in group `0x4413`, else 1. From the last
  point it tries the eight directions in the order straight on, +1, −1, +2, −2, +3, −3, +4 (`Assist_PickNewDir`,
  `0x00566F60`, directions modulo 8), never the direction straight back and never one already tried from that point.
  The neighbour cell in that direction (`0x00567000`) must be navigable for that radius (surface water slot 8,
  `IsNavigable(x, z, r, 0)`, terrain.md §10.4) and the boat must not overlap another boat there (§5.7); the first
  that passes is appended. When none does, the boat backs up: with 6 or more points it drops 5, with 4–5 it
  drops 3, else it clears the list; the direction is marked tried in the new last point and the search goes on.
  At most 15 rounds are made.

`Engine_AIBoatMotion` (`0x00571880`, 2.8 KB) picks cells when needed, steers the heading toward the next point and
sets the speed; its full equations remain open. `Movement_PositionNew` (`0x0056C430`) directly calls turning
helper `0x00569940`, straight helper `0x00568230`, and orientation/height helper `0x00569E70`; §6.2.2 specifies
straight translation and publication. **`0x0056A440` and `0x0056AF90` are crash-motion helpers**, reached from
`DoCrashSequenceTick`, not direct callees of `Movement_PositionNew` (calls `0x005703FF`, `0x00570493`). These
unnamed helpers have no recovered method names here; do not turn descriptive labels into purported native names.

Direction step helper `0x00567000` mutates signed cell x/z by this table; an unsigned direction above 7 (including
negative signed inputs reinterpreted as unsigned) leaves both unchanged. Evidence: switch at `0x00567004`, table
`0x00567060–0x0056707F`, increment/decrement targets `0x00567010–0x0056705D`.

| Direction | Δx | Δz |
|---|---|---|
| 0 | −1 | −1 |
| 1 | 0 | −1 |
| 2 | +1 | −1 |
| 3 | +1 | 0 |
| 4 | +1 | +1 |
| 5 | 0 | +1 |
| 6 | −1 | +1 |
| 7 | −1 | 0 |

This proves cell stepping, not the still-untraced continuous-heading-to-direction quantizer or compressed route
byte format. Independent vector: direction 0 maps `(10,20)` to `(9,19)`; direction 5 to `(10,21)`; 8 leaves it alone.

### 5.3 Ferries: requests and docking

`Ferry_OtherStationInformation` (`0x0056F480`) returns station B when the current crossing starts at station A
(positions equal within 0.0001 on each axis), else station A: the station the boat is heading for.

`Ferry_DockRequests` (`0x005704D0`): within entry 36 (300) of the station's position and with a manager:

- queued (top state 75): when the berth is free, take it (manager `0x0056CEC0`), `PopState`, `PushState(74)`;
- otherwise: take the berth and `PushState(74)`; or take the waiting spot (`0x0056CF50`) and `PushState(75)`; or,
  with both taken, `PushState(73)`.

The manager keeps, per station lot, one berth and one waiting spot, each holding at most one boat.
`Ferry_ClearDockRequests` (`0x0056D530`): a boat holding a berth that is more than entry 36 away from its current
station gives the berth back (`DoneDocking`), clears the held flag and flag `0x10000000`.

`DoDockingTick` (`0x00572770`): on land → `PushState(72)`. The heading is flattened (y = 0, renormalised). Farther
than entry 37 (90) from the berth, the boat sails on with the AI motion and the frame time clamped to entry 35.
Nearer, it turns toward the berth direction by at most dt × entry 38 degrees (sign from the cross product) and
moves toward the berth position by at most dt × entry 39. **Docked** when the angle is below 1° and the distance
below 1: the tick returns false and the state pops.

### 5.4 Stations and routes

`Ferry_SetStationInformation` (`0x0056F670`) stores both stations; when asked to start, it begins the crossing from
station A and sets docking in progress. `Ferry_InitializeCurrentRoute` (`0x0056F3F0`) makes the given station the
current one, holds its berth, sets the used-cell count to 2 when it was 0, clears "route started", loads its
compressed route and clears the travel points. `Ferry_SwitchActiveStation` (`0x0056F700`), on leaving state 74:
with requests at both stations, the crossing restarts from the station just reached; otherwise `Remove(true)`.

`SetNbrRouteData` (`0x0056F510`) gives the boat a neighbour route: its network occupant, its route bytes and the
flag; the reader then skips cells while they are within 3 × the boat's radius of the occupant's position.

### 5.5 Undocking (`DoUndockingTick`, `0x0056BD80`)

While the boat can move and its position is navigable with radius 2 (surface water slot 7, terrain.md §10.2; with a
neighbour route, `+0x340`, the lot passed is the one at the route occupant's position), it moves away from the
route occupant's position, horizontally, toward the point 2 × radius from it, by at most the frame time × entry 39
per tick (at least 1 unit to move at all). On land → `PushState(72)`. The tick returns false (the state pops) once
the boat is 2 × radius away.

### 5.6 Can it move?

`Engine_MovementPossible` (`0x005698E0`): not when flag 1 is set; not when a corner of its cell rectangle
(occupant slot 11) is outside the city (`0x005681B0`, city slot 99; then also `PushState(72)`); not in states 70 and
71. `OnLand` (`0x00567DF0`): the boat's cell is inside the city and the terrain height at the boat (terrain slot 18)
is above the level of terrain slot 29 *[inferred: the sea level]*.

### 5.7 Collisions (`Engine_DetectCollision`, `0x0056C090`)

1. A corner of the cell rectangle outside the city → `PushState(72)`.
2. **Other boats** are tested when this boat is player-driven, or when neither is and AI collisions are enabled
   (`0x00B22A7C` clear). The other boat is the first in the manager's list within the two boats' combined size whose
   footprint overlaps (`0x005690D0`). A crash happens when |speed − other speed| / entry 7 is above entry 27 (this
   boat player-driven) or entry 15. A player crash plays AEMS event 7. The boat with the lower or equal size class
   (`0x00566D60`) sinks: a player-driven loser gets `NotifyAutomatonEvent(0xB, 0)` and `PushState(71)` for entry 21
   ms (and "sunk by a collision"); an AI loser fades out (`PushState(73)`), unless the winner is the player's boat,
   in which case it sinks. Below the threshold the boats bump (`0x0056A310`): the other boat takes a new speed when
   it can move and this boat keeps entry 29 of its speed.
3. **The shore and buildings** (`Crash_InanimateCollisionDetection`, `0x005687C0`): a hit faster than entry 28
   (player) or entry 16 (AI), as |speed| / entry 7, plays AEMS event 7 for the player and runs `Crash_DetermineCase`
   (automata.md §6.6); slower hits bump.

For the boat-to-boat threshold-passing branch, size class is 3 for group ancestry `0x4414`, else 2 for `0x4413`,
else 1 (`0x00566D60`). With at most one player-driven participant, native comparisons select the following
outcomes; **equal sizes do not select a single loser**:

| Participants | Smaller participant | Equal sizes | Larger participant |
|---|---|---|---|
| Both AI | state 73 | both state 73 | unaffected |
| Player and AI | state 71, regardless of which is smaller | both state 71 | unaffected |

Each state-71 selection first notifies `(0xB,0)`, then pushes 71 with that boat's p21 duration and the true
state argument. A selected player also gets collision-sunk `+0x2B2=true`; a selected AI does **not** get that
byte set merely because the player wins. Thus sinking and the collision-mushroom effect flag are separate.
The receiver processes its own outcome first, returns early if strictly smaller, otherwise also processes the
other boat. Evidence: no-player comparisons `0x0056C1C5–0x0056C264`; player-receiver comparisons and flag store
`0x0056C282–0x0056C2DE`; AI receiver/player-other `0x0056C2E0–0x0056C35D`. This specifies selection and callbacks,
not footprint intersection, bump equations, RNG or sink/heel animation, which remain WC5.
Independent vectors: classes (1,1), both AI, remove both; (2,2), player/AI, sink both but set collision-sunk only
on the player; player class 3 versus AI class 1 sinks only the AI without setting that AI byte.

## 6. U-Drive-It *[compat]*

### 6.1 The drive tick (`DoPlayerDriveTick`, `0x0056D310`)

The automaton's drive tick; a tug whose towed boat is ahead of it (positive dot product with the heading) ends the
tow; with no throttle and no reverse (both −1 in the drive state) the speed decays toward 0 by
dt × 0.001 × entry 3 (and stops at 0); then, when the boat can move, `Movement_PositionNew` with the frame time
clamped to 0–entry 35, and `Engine_DetectCollision`.

### 6.2 Commands (`TranslatePlayerDriveCommands`, `0x0056E5C0`)

The group must have an AEMS sound set (group `+0x64`, or its parent's); otherwise nothing happens.

- **Gear.** Throttle at least entry 33 → forward with the throttle; else reverse input at least entry 32 → reverse
  with −the input; else neutral.
- **Speed.** An accelerating speed moves toward the gear's target (clamped to entries 6–7) by dt × entry 10,
  or dt × entry 10 × entry 0 for braking and reverse-from-zero; a weaker same-sign request preserves speed.
  The exact input mapping and crossing-zero behavior are in §6.2.1.
  Same-sign acceleration and command-channel feedback are specified in §6.2.3.
- **Steering.** Elapsed left/right inputs and return/countersteer are processed in the order in §6.2.4,
  then clamped to entries 8–9; the turn
  (`+0x2E0`) is the steering, and the rudder angle shown is −0.125 × the steering.
- **Fire.** With the fire button, the AEMS event `whalefire` is requested; when accepted, the drive state's shot count
  grows and the button is cleared.
- **Water skier** (group `0x4409`): a toggle starts the `waterski` effect using the boat transform while it moves forward, or
  stops it; it also stops when the boat stops.
- **Tow rope** (group `0x4410`, a tug): with the boat stopped and the tow button, a tug that tows nothing arms the
  tow (`+0x2D8`) and plays `tugropethrow` at the boat; one that tows ends the tow.
- The speed tween is reset when |speed| changes; then the automaton's own command translation.

#### 6.2.1 Input magnitude to target speed (`0x00567080`)

This address is an unnamed native helper, **not** a linear throttle multiplier. It takes the drive state's signed
command at `+0x28` by pointer and returns an x87 floating-point value; it does not change the command. For finite
inputs and ordinary tuning (`p30<0<p31`, `p6<0<p7`), define the binary32 constants `c=0.9100000262260437`
(`0x00B0A14C`) and `h=1.5707963705062866` (`0x00A9DA50`). The cached constants are binary32 stores:
`n=round32(-c*0.5)`, `T=round32(tan(c*h))`, `A=round32(atan(n*h))`.

1. Clamp the command to `[p30,p31]`, yielding `u`.
2. For `u>0`, compute `q=(u*c)/p31`; otherwise `q=(u*n)/p30`.
3. Multiply `q` by `h`; forward gear (2) uses `tan(q*h)`, all other gears use `atan(q*h)`, yielding `r`.
4. For `r>0`, compute `(r*p7)/T`; otherwise `(r*p6)/A`. Clamp that result to `[p6,p7]` and return it.

Products and divisions follow the displayed order on the x87 stack; only the caches and actual float stores force
binary32 rounding. Do not replace these constants with mathematical π/2 or rearrange the scale divisions.
Native evidence: cache construction `0x0056708A–0x005670EA`; command clamp/normalization
`0x005670EE–0x0056714C`; tangent/arctangent and speed clamp `0x0056714C–0x005671C8`.
Zero denominators, nonfinite tuning and transcendental bit-for-bit emulation remain outside the finite-input
contract; this helper contains no explicit protection for them.

The outer translator selects gear **before** calling this helper: forward input `>=p33` wins over reverse input;
otherwise reverse input `>=p32` becomes the negative command; otherwise gear and command become zero. An exactly
zero command skips speed adjustment in this method. Consequently the default forward threshold 0 permits forward
gear with zero command. The no-input `-1` sentinels are used by the separate drive tick, not as a magic range check
inside this magnitude helper (`0x0056E679–0x0056E6B1`).

With an opposite-sign target and nonzero current speed, braking approaches that **target**, using
`a=round32(dt*p10)` and `b=round32((dt*p10)*p0)`. The product inside `b` is retained on the x87 stack,
not reloaded from `a`. This corrects the round-1 double-rounding formula: `0x0056E686` stores with FST,
and `0x0056E68A` multiplies the retained value. A positive speed becomes `max(target,speed-b)`; a negative speed becomes
`min(target,speed+b)`. The opposite input channel is reset to its threshold, and execution skips the normal
acceleration branch. There is no split of the tick at zero and no separate remaining-dt acceleration; a single
braking step can cross zero. Positive-speed branch: `0x0056E749–0x0056E785`; negative-speed branch:
`0x0056E78A–0x0056E7BE`. For example, speed `0.002`, target `-0.02`, `dt=100`, `p10=0.001`, `p0=0.1`
produces approximately `-0.008`, rather than stopping at zero.

#### 6.2.2 Straight movement and publication (`0x0056C430`, `0x00568230`)

`Movement_PositionNew` takes a **pointer to float dt**; unnamed helper `0x00568230` takes dt by value, a transform
output and a position output. The outer method gets the existing transform through slot 12, copies current
position into a candidate, clamps Turn to `[p8,p9]` and Speed to `[p6,p7]`, and writes both clamped values back.
For finite inputs, Turn zero and Speed nonzero select this complete straight-translation path:

- Normalize the horizontal vector: `L=sqrt(heading.x²+heading.z²)`, direction
  `(heading.x/L,0,heading.z/L)`. The heading's y component does not enter this norm.
- Compute distance in the order `(p4*Speed)*dt` on the x87 stack. Multiply each direction component by that
  distance and store each displacement as binary32, then add to current position and store the candidate.
- Write the candidate into the transform's translation; set its translation-dirty bit and increment its
  generation byte. This helper performs no terrain/water-height query and does not rewrite heading.

Evidence: complete helper `0x00568230–0x005682E8`, norm through `0x00568269`, distance through `0x0056827B`,
displacement stores through `0x00568296`, position stores through `0x005682C5`. Thus p4 (default 0.2) scales
translation. Zero horizontal heading length has no fallback; do not invent zero-displacement success.

The outer method copies old current position to previous position, then candidate to current position. If either
clamped Speed or Turn is nonzero it calls orientation/height helper `0x00569E70`. Then, in order: slot 75 with the
transform; copy current position/heading into base fields `+0x40/+0x58`; slot 100 `(true,true)`; slot 104; slot 107.
Even zero-Speed, zero-Turn performs this publication, but skips `0x00569E70`; it also queries slot 57 with state 16
and ignores that result. Evidence: `0x0056C4C4–0x0056C5D6`, `0x0056C74A–0x0056C84A`.

This covers straight translation and outer publication; the orientation/height and turning helper internals
remain open. Independent vector before the height helper: heading `(3,4,0)`, position `(10,20,30)`, Speed 0.25,
p4 0.2, dt 100 gives approximately `(15,20,30)`. Heading `(0,5,4)` gives approximately `(10,20,35)`. Zero dt leaves
the candidate unchanged but still publishes and calls the height helper when Speed is nonzero.

#### 6.2.2a Turning geometry (`0x00569940`, round 2)

This helper takes dt by value, the output transform, a three-float position output, and a scratch transform.
The wrapper selects it for nonzero Turn; straight translation remains §6.2.2. Inputs below are the wrapper's
already-clamped Speed/Turn, heading H at `+0x30C`, and old position P at `+0x2F4`. The following geometric
contract is supported for finite, nondegenerate headings and nonzero radius/divisors. Intermediate x87
spill rounding is **partially traced**, not a promise that reassociated vector expressions are bit exact.

| Quantity | Native rule |
|---|---|
| Distance D | `round32((p4*Speed)*dt)`; dt is milliseconds, no 0.001 factor here |
| Radius for Speed>0 | `round32(((Speed/p7 + 0.5) + (1-abs(Turn)/p9))*p11)` |
| Radius for Speed<=0 | `round32(((abs(Speed)/abs(p6) + 0.5) + (1-abs(Turn)/abs(p8)))*p12)` |
| Center side for Turn>0 | Normalize `up cross H`, with up=(0,1,0) |
| Center side otherwise | Normalize `H cross up` |
| Center C | x,z = P.x,z plus radius times that side; y = a sea-level query rounded to binary32 |
| Radial V | Normalize P-C; the vertical component is retained |
| Angle θ | D/radius, negated when Turn<=0; no angular clamp in this helper |

Evidence: D `0x00569A33–0x00569A43`, radius `0x00569A47–0x00569ACC`, center/normalization
`0x00569AD0–0x00569B9F`, angle `0x00569BA3–0x00569BBE`. The center query is global water service
`0x00B43CF4`, vtable byte offset 0x74, **GetSeaLevel**, not terrain height or GetWaterLevel(x,z)
(compare terrain.md §5 water interface). No coordinates are passed.

If the scratch transform's flags byte is nonzero, reset its matrix to identity, scale to 1, translation to
zero and flags to zero; increment its revision byte (`0x00569BC0–0x00569BFC`). Then rotate its first and
third columns by θ: new first = old first*cosθ − old third*sinθ, new third = old first*sinθ + old third*cosθ.
Its second column is unchanged. cosθ is explicitly stored as binary32; sinθ remains on the x87 stack;
there are additional binary32 product spills before matrix stores. Set flags bit 4 and increment revision.
Transform V with that matrix and scale, normalize the result, multiply by radius and add C, publishing
three binary32 position outputs in x,y,z order (`0x00569BFE–0x00569DD2`). This is a circular displacement,
not forward displacement followed by a heading rotation. Speed's sign reverses θ without changing the
Turn-selected side; radius still uses the reverse tuning for Speed<=0.

Compare `abs(publishedZ-C.z)` to the **binary64** constant 1e-5 at `0x00A95620` (strictly greater only).
If greater, call GetSeaLevel again, then discard that return (`0x00569DD5–0x00569DF6`). In particular this
call does not overwrite the published y. Compute position deltas against P, add them back to P and publish
translation to the output transform, set bit 1 and increment its revision byte (`0x00569DF8–0x00569E5A`).
The extra subtract/add and float spills mean that transform translation need not be a bitwise copy of the
position output. There are no RNG draws in this helper.

No normalization epsilon, radius floor, zero-heading fallback or antiparallel correction occurs here.
The division/sqrt path is executed even for a degenerate horizontal heading; do not invent a valid direction
for it. Exact exceptional floating-point/control-word behavior and all spill boundaries remain OPEN.

#### 6.2.2b Bank, pitch and height (`0x005682F0`, `0x00569E70`, round 2)

The old description “orientation/height” did not specify these algorithms. Neither helper queries terrain
or water. Both take an in/out transform; neither consumes dt or RNG. The matrix offsets, in rows, are
`(04,08,0C; 10,14,18; 1C,20,24)`, scale is `+34`, translation is `+28,+2C,+30`.

Bank helper `0x005682F0` (464 bytes) returns without writing when Speed<=0. For Speed>0 it computes
`β=clamp(((Speed/p7)*p19)*Turn, -p20, p20)`, normalizes H, and builds quaternion
`(x,y,z,w)=(normalize(H)*sin(β/2), cos(β/2))`. It **replaces**, rather than multiplies into, the nine
matrix elements with the row matrix below; preserves translation and scale; sets flags bit 4 and increments
revision. Native clamp and branch `0x005682F0–0x00568359`, normalization/quaternion and matrix stores
`0x00568359–0x005684B8`.

| Row | Three matrix values |
|---|---|
| 1 | `1-2y²-2z²`, `2xy+2zw`, `2xz-2yw` |
| 2 | `2xy-2zw`, `1-2x²-2z²`, `2yz+2xw` |
| 3 | `2xz+2yw`, `2yz-2xw`, `1-2x²-2y²` |

Orientation helper `0x00569E70` (1172 bytes) returns without writing when Speed=0, **including nonzero
Turn with zero Speed**. Otherwise it calls the bank helper first, then takes U as the normalized second
column of the resulting transform times its scale. Reverse motion therefore uses the incoming matrix for U,
because bank has returned without changing it. Compute `α=clamp(p17*Speed,0,p18)` and normalize `H cross up`.
Build the same quaternion matrix using that normalized axis times `sin(-α/2)` and `cos(-α/2)`; multiply it
by H and normalize the result F. Normalize F a second time; compute R=normalize(U cross F), then U'=F cross R.
Replace matrix columns with R,U',F; set bit 4 and increment revision. Preserve translation x,z, add
`p18*0.5` to translation y, set bit 1 and increment revision. The y addition is performed for **reverse as
well as forward**, even though α is zero for reverse under positive ordinary tuning. Repeated calls add
the lift repeatedly; no absolute water-height clamp or division by dt exists here.

Evidence: zero-speed gate/bank `0x00569EBA–0x00569EDC`, old-up normalization and pitch clamp
`0x00569EE1–0x00569FA3`, quaternion `0x00569FA3–0x0056A0F9`, heading rotation/first normalization
`0x0056A0FD–0x0056A19A`, second normalization/cross products `0x0056A1C5–0x0056A26B`, ordered matrix/
translation publication `0x0056A26F–0x0056A2FA`; lift branch `0x0056A19C–0x0056A1C5`.

These matrix formulas specify the owned geometric computation, not all its bitwise spill behavior.
Zero H, vertical H, zero scale, parallel U/F and invalid tuning are unprotected normalization/division cases;
their exceptional outputs remain OPEN. Nonfinite propagation and all matrix-product rounding boundaries
need further instruction work before full numeric closure. Engine_AIBoatMotion and inherited animation
remain OPEN too: these helper rules do not delegate their outcomes to a foreign callback.

#### 6.2.3 Same-sign speed and inverse input feedback (round-2 instruction contract)

This section covers finite inputs with ordered limits `p6<0<p7`, `p30<0<p31`, nonnegative dt/p10/p0,
and valid group, drive-state and owner objects. It does not prescribe results for NaNs, bad pointers,
zero divisors or uninitialized tuning. `dt` is the **second argument, binary32 milliseconds**;
the first argument is the drive-state pointer. The decompiler's apparent arithmetic with that pointer
in steering is a type error: the instructions use the second argument at stack `+0xB8`.

Let `s` be owner Speed, `u` drive-state `+0x28`, `t` the §6.2.1 target clamped to `[p6,p7]`, and
`sign0(x)` be -1 when x<0 and +1 otherwise (including both signed zeros). The steps are ordered:

1. If u is zero, preserve Speed and skip all speed feedback. Gear selection has already happened.
2. If s is nonzero and sign0(t) differs from sign0(s), use the braking branch in §6.2.1 and skip steps 3–4.
3. If sign0(u)=sign0(s) and `abs(t)<=abs(s)`, **preserve Speed**, even for a weaker command. Write the
   inverse helper result below to forward channel `+0x7C` when s>0, otherwise to reverse channel `+0x80`.
   Do not rewrite `+0x28` or gear and do not reset the other channel here.
4. Otherwise choose increment a, except that differing signs of u and s with u<0 choose b.
   If increment is strictly greater than `abs(t-s)`, store t; otherwise store
   `round32(s + sign0(u)*increment)`. Equality takes the arithmetic branch. There is no second speed clamp
   after that store. In particular reverse acceleration from exactly zero uses b; forward acceleration uses a.

Evidence: `0x0056E7BE–0x0056E89D`; sign helper `0x00566E00–0x00566E22`. The opposite-sign branch
resets reverse input to p32 for s>0 and forward input to p33 for s<0 (`0x0056E776–0x0056E7B9`).
Neutral does not decelerate here; coasting belongs to the separate drive tick (§6.1).

The inverse helper is `0x005671D0` (312 bytes). It returns an x87 value without modifying its pointed-to Speed.
Its caches are **separate** from §6.2.1: `c=0.9100000262260437` at `0x00B0A150`,
`h=1.5707963705062866` at `0x00A9DA50`, `k=0.63661974668502808` at `0x00A9DA60`.
On first use, store binary32 `n=round32(-c*0.5)`, `T=round32(tan(c*h))`, and
`N=round32(tan(n*h))` in that order. **N uses tan, whereas the reverse cache in §6.2.1 uses atan.**
Clamp Speed to `[p6,p7]`, yielding v. When v>0 compute `(v*T)/p7`, otherwise `(v*N)/p6`.
Take atan, multiply by k, yielding w. When w>0 compute `(w*p31)/c`, otherwise `(w*p30)/n`.
Clamp to `[p30,p31]`, return, then round32 when storing the selected channel. The reverse feedback is
signed (normally negative); it is not an absolute reverse-input magnitude. Do not replace it with the
mathematical inverse of the reverse mapping in §6.2.1. Cache construction `0x005671D1–0x00567234`,
normalization `0x0056723A–0x0056729C`, atan/scaling/clamp `0x0056729C–0x00567305` prove this asymmetry.

#### 6.2.4 Steering channels and tween reset (round-2 instruction contract)

Let S be drive-state steering at `+0x5C`, L its channel `+0x84`, R its channel `+0x88`.
Negative channel values are inactive; zero is active. Process the following **right phase then left phase**
using each preceding store, not a simultaneous left-minus-right calculation. The p1/p2 descriptions in
the old summary were misleading: p1 maps elapsed channel milliseconds to steering; p2 sets the return rate.

| Phase | Condition | Ordered action |
|---|---|---|
| Right direct | R>=0 and S<=0 | If `S < -(p1*R)`, store `R=round32(-S/p1-dt)`. Then store `S=round32(-(p1*R))`. |
| Right return | Otherwise, S<0 | Start with `b*p2`. If L>0 multiply this candidate by p5 and store L=0. Recompute baseline `b*p2`; if the unrounded baseline is greater than candidate, replace candidate with `round32(baseline)`. Add candidate to S, store32 the sum, then clamp a positive unrounded sum to zero. |
| Right idle | Otherwise | No store. |
| Left direct | L>=0 and S>=0 | If `p1*L < S`, store `L=round32(S/p1+dt)`. Then store `S=round32(L*p1)`. |
| Left return | Otherwise, S>0 | Start with `b*p2`. If R>0 multiply the candidate by p5 and store R=0. Recompute baseline `b*p2`; if the unrounded baseline is greater than candidate, replace candidate with `round32(baseline)`. Subtract candidate from S, store32 the difference, then clamp a negative unrounded difference to zero. |
| Left idle | Otherwise | No store. |

Candidate products, comparisons and sums retain x87 precision except at the specified stores; the baseline
store uses FST and does not round the candidate still on the stack. Direct channel adjustment uses raw dt,
**not dt*0.001**. The right phase subtracts dt while the left phase adds dt; preserve this native asymmetry.
Both inactive with nonzero steering returns toward zero; with both inactive and S=0 neither phase stores S.
With both channels active, later processing can cancel or change the earlier phase. These rules require
p1 nonzero; no zero-divisor fallback is present. Evidence `0x0056EBAE–0x0056ED57`.

After rope handling, clamp S to `[p8,p9]`, store it back to `+0x5C` and to owner Turn (`+0x2E0`).
Set drive-state `+0x23C=0`, `+0x240=0x40C90FDB` (binary32 2π), and `+0x238=round32(S*(-0.125))`.
The rudder/heading output precedes tween handling (`0x0056EEC2–0x0056EF87`). The heading rule is below.

Compare binary32 `abs(Speed)` to the tween target at owner `+0x1A4`, using float equality, not a bit comparison.
If equal, leave all four tween words alone, even when Speed changes sign. Otherwise call inherited
`TweenStruct::GetCurrent` (`0x004FA960`), store its rounded result to `+0x1A0`, the new magnitude to `+0x1A4`,
and zero both `+0x1A8` and `+0x1AC`. Then call inherited command translation (`0x00509070`) with the
original drive-state pointer and dt and return its result (`0x0056EF8D–0x0056EFD9`). The inherited bodies
are specified below, not foreign callbacks allowed to supply an owned command outcome.

#### 6.2.5 Command effects, heading, and inherited completion (round 2)

The ordered owner stages are: group/AEMS availability gate; gear/command and speed; fire; skier; steering;
rope; steering clamp and heading outputs; speed tween reset; inherited command completion. A failed group/
AEMS gate returns false before those mutations. No owner-stage RNG draw is present. Effect internals and
sound services may have their own behavior; they cannot compute the owned speed or steering outcome.

- **Fire:** if drive-state `+0x98>=0`, call embedded sound object `owner+0x18C` slot 5 first. Only if byte
  `drive+0x1FF` is nonzero, request named AEMS event `whalefire` with two zero arguments (slot 12). On a true
  return, store `drive+0x1E0=0`, clear byte `+0x1FF`, and increment the 32-bit shot counter `+0x178` (wraps).
  False return or inactive channel leaves these fields unchanged. There is **no fire output position or
  offset computation in this body**. The earlier “effect” wording was too broad (`0x0056E8A0–0x0056E959`).
- **Skier:** require group membership 0x4409 and a nonnull effect at `+0xDC4`. If toggle byte `drive+0xEB`
  is set, query IsRunning: running → Stop(1); not running and Speed>0 → GetTransform, SetTransform, Start(0).
  The toggle is not cleared here. Then independently: Speed<=0 and currently running → Stop(1); otherwise
  Speed>0 and currently running → refresh its transform. All running queries occur at the indicated stages;
  do not cache the pre-toggle result. It may be queried again after Start/Stop (`0x0056E959–0x0056EBAE`).
  SetTransform receives the **whole current boat transform**, not a constructed point behind the boat:
  the pointer passed at `0x0056EA7B` and `0x0056EBAB` is the same local transform filled by owner slot 12.
  Any trailing distance intrinsic to the effect is outside this owner body. This corrects the old summary.
- **Rope:** unconditionally snapshot the current transform after steering. A direct group id 0x4410,
  Speed exactly zero, and byte `drive+0xED` nonzero gates the button action. With a towed reference, call
  EndTowing. Otherwise set armed byte `+0x2D8=1`; if rope effect `+0xDBC` exists and is not running,
  SetTransform(snapshot), Start(0). Then, irrespective of the button/group gate, a nonnull running rope
  effect gets SetTransform(snapshot). A newly started effect can therefore receive two transform writes.
  No button clear is present (`0x0056ED5A–0x0056EEC2`).

For heading output, take **owner current transform heading** `h=(+0x58,+0x5C,+0x60)`, not steering and
not a newly normalized local H. For finite h with h.z in [-1,1], set A=acos(h.z). If `-h.x<=0`, first store
`round32(K-A)` to drive `+0x244`; otherwise first store `round32(A)`. Then replace that field with
`round32(K-field)`, where K is binary32 `0x40C90FDB`. There is no wrapping after this second subtraction.
Both +Z and -Z are covered: h=(0,0,1) yields 0 after the two stores; h=(0,0,-1) yields approximately π.
The export's `cS3DTransform::TransformDirection` name at `0x009F0090` is misleading here: it consumes the
x87 scalar and calls the acos path `0x009F00AD`, which computes atan2(sqrt((1+x)*(1-x)),x), with exact
endpoint branches +1→0 and -1→π (`0x009F00C1–0x009F0118`). The caller supplies h.z, without a clamp or
normalization. Out-of-domain/nonfinite math-library error handling remains OPEN.

`TweenStruct::GetCurrent` at `0x004FA960` returns its target for duration<=0; otherwise it stores
`q=round32(elapsed/duration)`, clamps q to [0,1], then returns `start+(target-start)*q` on the x87 stack
(`0x004FA961–0x004FA9D6`). This supplies the actual start value used by §6.2.4, without a callback oracle.

Inherited command completion (`0x00509070`, 187 bytes), after the owner mutations, processes 13 channels:

| Channel i (0..12) field in drive-state | Meaning/operation |
|---|---|
| current `+0x7C+4i`, saved `+0xB0+4i`, edge byte `+0xE4+i` | Before copying, edge is true exactly when `0 < saved-current < 250` (binary32 250 constant, x87 subtraction); otherwise false. Then copy current's raw 32-bit word to saved. |
| threshold `+0x128+4i`, count `+0x15C+4i` | Both are signed 32-bit integers. If threshold!=-1 and count>=threshold, clear latch byte `+0x1F8+i`. If current's bits differ from float -1, acquire AEMS events, call slot 6 with 0x14, reacquire, then call slot 5 with 0x14. |
| latch `+0x1F8+i`, timer `+0x1C4+4i`, limit `+0x190+4i` | Otherwise, if latch already set, do nothing. If clear and timer<limit, store32 timer+dt; if timer>=limit, set latch and zero timer. There is no second check after adding dt. |

The edge pass (`0x00505730–0x00505778`, 73 bytes) precedes **all** saved-word copies. Channel iteration is
ascending. Return true after channel 12. Decompiled float treatment of the threshold/count is incorrect:
`FILD` of the threshold plus a signed `JL` of count at `0x0050909C–0x005090B5` establish the integer rule.
This explicitly refines automata.md §6's generic PlayerDrive channel summary for Windows watercraft;
the vehicle command accumulator/speed integration described there is not substituted for this owner logic.
Its row also attributes forced flags 0x2000000/2/0x10000 to `0x00508C50`: the Windows instructions traced here
contain no such flag writes or setter calls. That attribution is not part of this watercraft tick contract;
entry-time or other inherited flag changes require their own evidence. automata.md itself is outside this lane's edits.

The preceding inherited drive tick (`0x00508C50`) first queries owner slot 31 with 1; true returns false.
Otherwise obtain the current state with slot 56, advance each of 13 nonnegative current channel floats by dt,
saturating to binary32 maximum when `current >= maxFloat-dt`, leave negative channels unchanged, then invoke
owner slot 121 with (state,dt). Query global terrain service `0x00B43D0C` slot 17 with owner current x,z and
output pointers `state+0x26C,+0x270`, then return true (`0x00508C50–0x00508CF1`). This explains how active
elapsed input zero becomes dt before the translator; neither step changes dt to seconds. Base animation
outside this drive tick and owner coasting/movement remain separate, partly open contracts.

### 6.3 Towing

A tug's armed tow is used by message `0xAC400A31` (§7). A towed boat (state 77, `DoFollowingTick`, `0x005694B0`)
keeps on the water surface and stays radius + 10 behind its tug: it turns toward the tug by at most dt × entry 38
degrees and moves toward the point behind it when farther; it does nothing without a tug.

More precisely, `DoFollowingTick(dt)` returns false without a tug and true for every normal path with one.
It first sets base position y to surface-water height. Unnamed helpers `0x00567A50` and `0x00567AB0` compute
respectively `currentPosition-radius*currentHeading` (stern) and `currentPosition+radius*currentHeading` (bow).
The radius is obtained from slot 17 on the **helper's receiver**. Heading is not normalized inside these helpers.
Following obtains the tug's stern twice, replacing the first copy's y with water height. Its horizontal desired
heading is from the towed boat's base position to the tug stern; its separation vector is from the **towed boat's
bow** to the first stern copy. The allowed separation is the **towed boat's own radius + 10**, not the tug radius.
Evidence: receivers/radius `0x005694D9–0x00569517`, bow receiver `0x00569625–0x00569642`; helper bodies
`0x00567A50–0x00567B05`.

For finite, nondegenerate geometry, let `d` be that full 3D separation vector, `D=length(d)` and
`excess=D-(ownRadius+10)`. A nonpositive excess suppresses **both translation and turning**. Otherwise, when
the heading-angle/distance gate is active, add `normalize(d)*excess` to base position in this tick: translation
has no dt multiplier, no p39 scale and no speed cap (`0x00569695–0x00569761`). Turn allowance alone is
`round32(dt*p38)` degrees, with unclamped dt; NaN in the computed angle is replaced by zero
(`0x00569551–0x0056955B`, `0x005695F4–0x00569625`). The heading-angle gate skips movement only when angle <1
degree **and** D<1. Actual angle evaluation and degenerate normalization remain WC7 residuals; no zero-vector or
antiparallel fallback is specified by this partial path. Successful motion calls slot 16 with position/heading,
then advances both previous/current pose pairs. A no-motion true return does not publish those pairs.

Independent finite aligned example: tug current position `(0,0,100)`, heading `(0,0,1)`, radius 5; follower at
`(0,0,0)`, same heading, radius 2; water height 0. Tug stern is z=95, follower bow z=2, D=93, allowed gap=12;
the follower translates 81 units to z=81 even with dt=0. Its new bow is z=83, exactly 12 behind the stern.
This is a following-translation vector, not a claim to have validated every following angle/math case.

## 7. Messages (`DoMessage`, `0x00569230`) *[compat]*

- `0x26D31EC1`: reload the drive parameters.
- `0xAC400A31` while the tow is armed: disarm it; take the world point of the message's payload; when the pick
  is valid and a manager exists, the nearest boat at that point (`0x00568C10`), not this one, in group `0x4417`, and
  not in state 70, 71, 72 or 73, becomes the towed boat: the tug is recorded in it, a boat that was docking, queued
  or parked (74–76) pops that state, it pushes state 77, and this boat's own tug reference is released
  *[inferred: the payload is the rope's landing point]*.
- Anything else goes to the automaton's handler.

## 8. Quirks

1. The 60 s idle timeout pushes `SLOW_REMOVE` and, in the same tick, also runs the crash accumulator and the crash
   sequence tick (the switch falls through from case 7 into case 70; bytes `0x0057339C`–`0x005733B5`).
2. The idle budget is counted twice per tick for a boat with `+0x2CC` that is idle: once in the animation tick and
   once in the state tick *[inferred: both run in the same frame]*.

## 9. Debug text (`GetDebugExtraText`, `0x00570AE0`)

`Tow: %d, (FT,MT): (%d,%d), (Q,D): (%d,%d), Turn: %3.2f, Speed: %3.2f, Cell(%3d,%3d), Pos(%3.2f,%3.2f,%3.2f),
Ornt<%3.2f,%3.2f,%3.2f>`: whether it tows, two counts of the manager (`+0xA9C`, `+0xAC4`), queued, docking, turn,
speed, cell, current position and current heading.

## 10. Open questions

See [questions/watercraft.md](questions/watercraft.md). WC3–WC7 include residuals beyond the traced paths below;
neither a method inventory nor one complete arithmetic subpath establishes whole-owner coverage.
The reproducible address/byte census and its discrepancy with the requested 79-function brief are published in
[watercraft-owner-inventory.md](watercraft-owner-inventory.md). Its 75 entries are not presented as a fabricated
79-method signature inventory.

## 11. Conformance tests

Round-2 independent branch/constant vectors and remaining numeric limitations are in
[watercraft-command-vectors.md](watercraft-command-vectors.md). They are proposed expectations, not executed native results.

Additional independent vectors for the traced lifecycle/input paths:

- Already-initialized Init: no parameter load, effect request, flag write or timer clear; return true.
- Base Init false plus one failed effect request: still issue all six requests in order and all four flag writes;
  return true. No synthetic release of a never-acquired effect and no rollback.
- Pre-Init lifetime `(limit,elapsed)=(1234,567)`: both become zero. Versions 1, 3 and 65535 consume just the owner
  word and reject; version 2 alone reaches base Read, whose false return must propagate.
- Magnitude helper, defaults: neutral zero command returns zero; forward command 3000 reaches `p7`; reverse command
  −2000 reaches `p6`, within float/transcendental tolerance. Half inputs are **not** half the speed: forward 1500 is
  approximately 0.03705 and reverse −1000 approximately −0.05531. Evaluate from the formula and constants, not from
  the implementation under test. Crossing-zero braking uses the worked example in §6.2.1.

1. A boat group without drive parameters gets entry 21 = 8000 and entry 36 = 300; one that gives entry 9 = 3 and
   entry 8 = 0 gets entry 8 = −3.
2. A ferry 250 units from its station with the berth free pushes state 74; with the berth taken and the waiting spot
   free, state 75; with both taken, state 73.
3. A docking boat 0.5 units from the berth, turned 0.5° from the berth direction, finishes docking (the state pops).
4. With entry 7 = 0.3 and entry 27 = 0.5, a player boat meeting an AI boat at a relative speed of 0.12
   (0.4 of entry 7) bumps; at 0.18 (0.6 of entry 7) a crash happens.
5. The wandering pick from a point in direction 2 tries 2, 3, 1, 4, 0, 5, 7 and never 6.
