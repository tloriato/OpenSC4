# Spec: the automata COM director and the automata managers' class virtuals

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`). Implementers must work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult
decompiled or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[summary]* is a correct outline whose details are not given.

Related specs: [automata.md](automata.md) §5 (the manager records, the layer list and its cursor, the incidents),
§7 (`CreateAutomaton`, `SetMaxCount`, the make-room rule), §11 (interface slot names);
[automata-support.md](automata-support.md) (the tuning block `T+offset`, its clocks and spawn-and-recall entries);
[vehicles.md](vehicles.md), [aircraft.md](aircraft.md), [watercraft.md](watercraft.md),
[pedestrians.md](pedestrians.md) (the automata classes); [crime.md](crime.md) (the crime layer);
[city-situation.md](city-situation.md) (the city situation manager).

## 0. Summary

One GZCOM director creates every automata class. Behind the four automata managers' interface (automata.md §11)
there are two layers of class virtuals: those of the base manager (view rectangle, zoom visibility, time-of-day
clocks, group tracking, the onscreen tests) and those of the **traffic automata manager** that the vehicle and
pedestrian managers share (the traffic layers, their groups and traffic types, and the population pass that turns
the traffic simulator's counts into visible automata). The director also reaches many inherited slots of the
vehicle family; they are listed in §5.

## 1. The automata COM director *[compat]*

`cSC4AutomataCOMDirector` (constructor `0x00514A00`, made by `SC4AutomataGetCOMDirector`; `GetDirectorID`
`0x00514C20`) registers 30 classes, each with a factory that allocates the object, runs its constructor and returns
it (the managers return the interface at `+8`):

| Class | Class ID | Factory | Size | Constructor |
|---|---|---|---|---|
| `cSC4AutomataScriptSystem` | `0x4A1B664E` | `0x005148A0` | `0xB8` | `0x005207A0` |
| `cSC4VehicleManager` | `0xA98F9D3B` | `0x005145A0` | `0xAE8` | `0x0055FB50` |
| `cSC4PedestrianManager` | `0xC98F9D17` | `0x005145D0` | `0xAE0` | `0x00545500` |
| `cSC4AircraftManager` | `0xC98F9D4A` | `0x00514600` | `0xA9C` | `0x004FBB40` |
| `cSC4WatercraftManager` | `0xA98F9D28` | `0x00514630` | `0xAD4` | `0x00574860` |
| `cSC4Vehicle` | `0x896E714A` | `0x005148E0` | `0x2B8` | vehicle `0x005628F0` |
| `cSC4FireTruck` | `0xAA222B83` | `0x00514660` | `0x2E4` | `0x005337E0` (emergency `0x00532190`) |
| `cSC4PatrolCar` | `0x4A222B98` | `0x00514680` | `0x2DC` | `0x00544240` (emergency) |
| `cSC4TrainCar` | `0x4A416254` | `0x00514910` | `0x2C8` | `0x0055B150`, also run by the train's (allocator `0x00557290`) |
| `cSC4Train` | `0x8A41625A` | `0x00514940` | `0x2FC` | `0x0055B2F0` |
| `cSC4TrailerVehicle` | `0x4A5B69ED` | `0x00514970` | `0x2E4` | `0x00556DB0` |
| `cSC4GetawayVan` | `0xEA863423` | `0x005146A0` | `0x2B8` | vehicle, then `0x00533BA0` |
| `cSC4Bus` | `0xAA9F4A3D` | `0x005146C0` | `0x2B8` | vehicle, then `0x00524360` |
| `cSC4CarjackedVehicle` | `0xCBB32B0A` | `0x005146E0` | `0x2B8` | vehicle, then `0x005244C0` |
| `cSC4Tank` | `0xEBE0DCAD` | `0x00514700` | `0x37C` | vehicle, then `0x0054CD90` |
| `cSC4MayorLimo` | `0x4C0DD224` | `0x00514720` | `0x2C4` | vehicle, then `0x0053CEC0` |
| `cSC4Ambulance` | `0x6C1ACDE1` | `0x00514740` | `0x2D8` | `0x00504F30` (emergency) |
| `cSC4MassTransitTrain` | `0x4C22AE53` | `0x00514760` | `0x2FC` | `0x0053CBB0` (train) |
| `cSC4Pedestrian` | `0x896E75AF` | `0x005149A0` | `0x270` | `0x00545F20` |
| `cSC4PedestrianOneShot` | `0xAA09DC28` | `0x005149D0` | `0x274` | `0x00546860` |
| `cSC4SimFireCrew` | `0x4A222E4D` | `0x00514780` | `0x2C4` | `0x0054A2A0` |
| `cSC4RiotPolice` | `0x8A6E4FA4` | `0x005147A0` | `0x27C` | `0x00547800` |
| `cSC4Aircraft` | `0xC96E75DC` | `0x005147C0` | `0x1EC` | the automaton's |
| `cSC4Helicopter` | `0x8BA98EDA` | `0x005147E0` | `0x278` | aircraft `0x004FAC60` |
| `cSC4Airplane` | `0xCBEB4F62` | `0x00514800` | `0x358` | `0x004FCFE0` (aircraft) |
| `cSC4Watercraft` | `0x096E75F5` | `0x00514820` | `0xDC8` | `0x0056D9A0` |
| `cSC4AutomataControllerManager` | `0xCA14CA03` | `0x00514840` | `0x80` | `0x0051AA50` |
| `cSC4AutomataAttractor` | `0x6A1628BA` | `0x00514860` | `0x6C` | `0x005181F0` (controller `0x00517D40`) |
| `cSC4AutomataGenerator` | `0x4A1628C5` | `0x00514880` | `0xA0` | `0x005182B0` (controller) |
| `cSC4CitySituationManager` | `0x0BB14381` | `0x005148C0` | `0xB00` | city-situation.md |

The vehicles and the pedestrians are allocated from their classes' pools (`0x0055B970` and
`cSC4Pedestrian::operator new`). The **carjacked vehicle**, **mayor's limo**, **ambulance** (an emergency
vehicle) and **mass transit train** (a train) have record contracts in [automata.md](automata.md)
§§6.4.1–6.4.4. Their constructors install their own tables and initial fields over the base class
*[summary]*; their records include distinct outer version wrappers.

## 2. The manager tables *[compat]*

### 2.1 Layout

Each manager object has four tables: a message target (slot 3 `DoMessage`), a serializable view (slots 3 `Write`,
4 `Read`, 5 `GetGZCLSID`), a third small view and the manager interface. The interface slots 0–37 are those of
automata.md §11; slots 38–67 are the base manager's class virtuals; the vehicle and pedestrian managers add slots
68–91, the traffic automata manager's. The pedestrian table is at `0x00A9A470`, the vehicle table at `0x00A9D1E0`,
the aircraft table at `0x00A94850` and the watercraft table at `0x00A9DF38` (automata.md §11).

Implementations that the managers share are listed once. The traffic automata manager (vehicle and pedestrian)
gives slot 16 `GetRandomCreationPoint` (`0x00553080`), 17 `Dispatch` (`0x00551730`), 18 (`0x0054DBF0`) and 8
`RemoveAutomaton` (`0x0054D690`); its record is Write `0x00553570`, Read `0x00554410` (automata.md §5.1, with the
incident list `0x00552A80` / `0x00553E90`).

### 2.2 The base manager's class virtuals (slots 38–67)

| Slot | Address | Behaviour |
|---|---|---|
| 38 | `0x00545670` (traffic), `0x004FBCF0` (aircraft), `0x00574950` (watercraft) | the deleting destructor |
| 41 | `0x00507AC0` (base); `0x0054DFB0` (traffic) | `IsAutomatonOnscreen`: the traffic version tests the automaton's position (slot 7) against the manager's view rectangle *[inferred]* |
| 42 | `0x0050C8B0` | **clock value** for a traffic type: 1.0 unless the clocks are on (`0x00B21D58`); else the clock of the type (types 6, 9 and 10 use clock 4; automata-support.md §1.4) at the hour plus minutes × 1/60, or 1.0 without one |
| 43 | `0x00507B80` | posts a message with the given type to the message server |
| 44 | `0x00507B00` | finds the exemplar-data entry of a resource key (entries of `0x6C` bytes at `+0xA18`) |
| 45 | `0x00511990` | removes every automaton: the automaton sets are released (slot 33), then each automaton is unplaced (slot 62), the camera stops following it when it is the followed one, it shuts down and is released; the list is emptied |
| 46 | `0x00510690` | `CreateAutomatonInternal` (automata.md §7.2) |
| 47 | `0x0050A740` | the make-room rule (automata.md §7) |
| 48 | `0x00506CD0` | **view rectangle**: the current one becomes the previous one; the new one is the 3D view's visible area grown by 64 m on each side; then slot 60 |
| 49 | `0x00505350` | when the two arguments differ, slot 48 |
| 50 | `0x0050A5D0` | sets "visible" from the zoom (zoom ≥ the manager's threshold `+0xA3C`) and shows or hides every automaton accordingly (those outside the given filter are hidden) |
| 54 | `0x00506D50` | on a building occupant (`0x278128A0`) whose source property (`0x87DDA3A9`) value is at least 1, slot 53 (`HandleNewBuilding`) |
| 55 | `0x0050C7E0` (base); `0x005516C0` (traffic) | on an occupant removal: a city situation `0xC772BF98` / `0xA821EF94` is passed to slot 52; group counters of the occupant's groups go down; the traffic version first drops the occupant's queued dispatch requests (§3.3) |
| 56 | `0x0050F2C0` (base); `0x0054F7B0` (traffic) | **PostCityInit**, no stack args; base restores/removes loaded automata before the old-version source recount, then updates view/zoom. Traffic performs a second path-info/leader reference-repair pass. Ordered calls, native Init's call to extended manager slot 68, placement and allocator preconditions: automata.md §4.2 (AD6). No atomic public restoration service is called. |
| 57 | `0x0050A670` | passes a value to every automaton (automaton slot 81) |
| 59 | `0x0050F690` | registers an occupant in the tracked groups it belongs to (once per group) |
| 60 | `0x0050A520` | **zoom update**: at or above the threshold zoom (`+0xA3C`), or when the camera follows an automaton whose group this manager can create (slot 30), an inactive manager activates (slot 5 `Activate(true)`); otherwise an active one deactivates; while active every automaton gets the zoom and rotation (automaton slot 74) |
| 61, 62 | `0x005073D0`, `0x00507430` | **place** and **unplace** an automaton (automata.md §7: `RemoveAutomatonInternal`): placing an automaton whose occupant has flag 4 and that is not placed calls its slot 70 (true) and, when that succeeds and it is not yet in the second state, its slot 71 (true); unplacing turns off whichever of the two is on (two bits at `+0x33`) *[inferred: the occupant layer and the draw list]* |
| 63 | `0x0050A7B0` (base); `0x00553300` (traffic); `0x0055B600` (vehicle) | the animation tick; the traffic version first runs the traffic effects (§3.4), then, when active and not paused and not globally frozen (`0x00B21B9C`), the population pass (slot 76) and the dispatch queue (slot 84) |
| 64 | `0x00507470` | `UpdateCameraFollow` |
| 65 | `0x0050F930` | seeds the tracked groups at `Init` (automata.md §7) |
| 66 | `0x00514520` (base), `0x0054D780` (traffic, the same) | reloads the exemplar data and registers for changes to the tuning exemplars (`0x6534284A`, callback `0x00513A60`) |
| 67 | `0x0050F9C0` | `InitExemplarData` |

The other slots of the range (39 `StaticInit`, 40 `StaticShutdown`, 51, 52, 53 `HandleNewBuilding`, 58) belong to
other specs or to the manager classes (automata.md §7). The pedestrian and vehicle managers' slot 58
(`0x005450A0`, `0x0055C9D0`) and their slots 85–87 (§3.1) are their own.

### 2.3 The traffic automata manager's virtuals (slots 68–91)

| Slot | Address | Behaviour |
|---|---|---|
| 68 | `0x0054D6C0` | the traffic type of an automaton: by its groups, `0x422F`/`0x422E` → 7, `0x422D`/`0x422C` → 6, `0x4208`/`0x4217` → 4, else 3; without traffic data the manager's default (`+0xA8C`) |
| 69 | `0x0054FBC0` | refreshes the route of every automaton on a given network occupant |
| 70 | `0x005538D0` | queues a **dispatch request** (§3.3) |
| 71 | `0x0054F8A0` | creates a traffic automaton on a network occupant for a dispatch: the group is the given one or the default; on failure the automaton is removed |
| 72 | `0x0054DDB0` | the 4 × 4 traffic counts of a layer on a tile: layer → traffic-simulator type 1 → the default (`+0xA90`), 2 → 2, 3 → 4, 4 → 3, 5 → 5, 6 → 6, 9 → 7, 10 → 8 (7 and 8 go to slot 88); the `u16` counts are scaled by the factor of slot 87 and the tile's own factor *[summary]* |
| 73 | `0x0054D3C0` | layer → traffic type: 4, 5 → 3; 6 → 4; 9 → 6; 10 → 7; else the default |
| 74 | `0x0054D420` | **layer → automata group**: 2 → `0x4200`, 3 → `0x4204`, 4 → `0x4215`, 5 → `0x4216`, 6 → `0x4217`, 8 → `0x4102`, 9 → `0x422C`, 10 → `0x422E`; else the manager's default (`+0xA44`) |
| 75 | `0x0054DF40` | a demand value: a curve of the layer (slot 85) at the city's date value, × 1/3600 × the given rate |
| 76 | `0x00552000` | **the population pass** (§3.2) |
| 77 | `0x005554E0` | the per-tile spawn and recall (§3.2) |
| 78 | `0x00551A50` | smooths a tile's per-direction counts and spawns along it (§3.2) |
| 79, 80 | `0x0054EF00`, `0x0054F3C0` | create one traffic automaton on a network tile in a direction and lane *[summary]* |
| 81 | `0x005539A0` | collects the automata of a layer on a tile that are self-controlling (flag 2), without flags `0x200000`, `0x80` and `0x2000000`, and whose opacity (slot 32) is above 0.95 |
| 82 | `0x0054D260` | a move to the neighbour cell in a direction (0: x − 1, 1: z − 1, 2: x + 1, 3: z + 1) is allowed unless that cell is inside the network map and holds a tile without network flag `0x200000` |
| 83 | `0x00553370` | `PopulateCrimeLayer` (crime.md), with the same 5 ms budget |
| 84 | `0x00552DE0` | runs the dispatch queue (§3.3) |
| 85, 86 | ped `0x00544440`, `0x00544460`; veh `0x0055B440`, `0x0055B460` | the spawn curve and its scale of a layer: the base (`0x0054D560`, `0x0054D5C0`) picks one of the tuning curves; the pedestrian manager uses the curve `T+0x098` and scale `T+0x0A8` for layers 1 and 7 |
| 87 | ped `0x00544480`; veh `0x0054D630` | a factor from the traffic data (`+0xA84` slot 23) *[inferred: the congestion]*: 1.0 when the data is off (`+0xA3D`); pedestrians 1 + 0.02 × the value; vehicles 1 − 0.01 × the value, clamped to 0–0.5 |
| 88 | `0x0054D4A0` | the 4 × 4 counts of layers 7 and 8 (a transposed table when the rate is negative) |
| 89 | `0x0054FA10` | layer 8 on a tile: removes the extra automata above the target count and creates one of group `0x4102` when there are fewer *[summary]* |
| 90 | `0x00552450` | whether an automaton's next route cell has a network tile it may enter |
| 91 | `0x0054D300` | the network type for a dispatch kind: kinds 1 and 2 take the first of 2, 6, 0, 3 that the automaton may use; 3 → 1; 4 → 7; else −1 |

## 3. Traffic population *[summary]*

### 3.1 Layers

The layers are the traffic kinds 1–10 of automata.md §5.1 (vehicle manager 1, 2, 3, 5, 4, 7, 6, 8, 9, 10;
pedestrian manager 1, 7, 8). A layer takes part only while its bit is set in the tuning mask `T+0x194`
(automata-support.md §1.3) and its clock (slot 42) is not 0.

### 3.2 The population pass (slot 76, `0x00552000`)

Each node of the layer list adds the frame time to its accumulated time. The pass serves the cursor's layer: when
the walk position is unset, it starts at the top-left of the view rectangle and the cursor moves to the next
enabled layer (automata.md §5.1). Layers 1–5 and 7–10 run only while a global mode (`0x00B43DC4` slot 22) is 1,
layer 6 only while it is 2, and a layer whose clock (slot 42) is below 0.001 is skipped. For layers 1, 2, 3 and 7
the pass walks the tiles of the view rectangle row by row, calling slot 77 for each network tile, and stops when it
has used **5 ms**; it continues there in the next frame. The other layers take their own branches *[summary]*.

The per-tile step (slot 77, `0x005554E0`): a tile inside both the current and the previous view rectangle
(`0x00551630`) is skipped. For each of the six travel classes that have a traffic type (table `0x00A9C330`:
−1, 1, 0, 2, 3, −1) the tile's smoothed count is updated as (count × w + smoothed) / (w + 1) with w = the frame
time / 1000; the automata of the layer already on the tile are counted, and the layer's spawn-and-recall entry
(automata-support.md §1.4) decides: an automaton is created (slot 80) when the ratio of present to wanted is below
the entry's second value and a uniform random number is below its first; extras are recalled *[summary of the
recall]*. Slot 78 does the same per direction and lane for the 4 × 4 counts.

### 3.3 Dispatch requests

A request (slot 70) holds a kind, a network occupant (with a reference), a count, a flag word (with `0x2000000`
added), a mode byte, a byte, a timer and a try count. The queue (slot 84, `0x00552DE0`) counts each timer down by
the frame time; when one expires it collects the network occupants of the request's kind reachable from its
occupant (traffic network slot 26, flags `0xC49`) and creates automata on random ones (interface slot 18, or slot
71 when the mode byte is set) until the request's count is reached; a request that still lacks automata waits
1000 ms more, up to 5 tries, then is dropped. Removing an occupant drops its requests (slot 55).

### 3.4 Traffic effects

The traffic effect manager (pedestrians.md §5) is ticked from slot 63 (`0x00552BD0`): every 500 ms each effect's
time is reduced by the time since the last tick; an expired effect detaches its controller and is removed; a live
one follows its occupant's cell. `0x00552A20` detaches all of them and `0x00551FC0` frees them; `0x00552D00`
(called from the vehicle and pedestrian managers' `Shutdown`) releases the traffic data and the dispatch requests
and clears the effects.

### 3.4.1 Accident creation and the native caller boundary (AD5) *[compat]*

Both entries have x86 `thiscall` ABI: ECX is the concrete receiver; three 32-bit stack
arguments in declaration order are **`cISC4Occupant* occupant, int32 radius,
uint32 durationMs`**; callee pops 12 bytes, EAX returns an opaque borrowed effect-record
pointer. This is a token for StopEffect, not an `IGZUnknown*`, boolean or owning reference.
`0x00554400` adjusts ECX by `+0xAB8` and tail-jumps to `0x00554040`; its receiver is the
traffic/vehicle manager's primary concrete view. The latter receiver is its embedded traffic
effect manager. Neither is a virtual/interface slot. The conflicting argument-source inference
in the ABI inventory is resolved by the body and native caller's pushes (`0x0071876E`–`0x0071877E`),
not by the decompiler's argument list. Network-occupant `AsOccupant()` has no argument;
the pushed 2 is the radius of CreateAccident. Full caller ordering: traffic-simulator §4.13.2.

Creation is unconditional for a valid live receiver, initialized list and non-null occupant:

1. Append a new list node (0x40 bytes including two link words), with a zero-initialized
   0x38-byte effect payload, to the tail. The returned token addresses the payload, not the links.
   There is no capacity, duplicate, visibility, pause or random admission test here, and no
   graceful allocation-failure return in this body. `0x0055404B` loads the list sentinel;
   `0x005540A3` and `0x005540AF` access/link its tail. Faults there precede controller creation.
2. Store durationMs (u32). Ask the **occupant** view for its cell rectangle (slot 11,
   one output pointer to four s32 bounds; no reference transfer), compute centre x and z
   as signed division of each endpoint sum by 2, truncating toward zero, and store radius.
   Compute two inclusive effect rectangles: centre ± max(radius−1,0) and centre ± radius
   (`0x005540C5`–`0x005540FE`, `0x0054EC60`–`0x0054ECB1`). The helper calculates bounds;
   it does not start a timer, call a controller or draw randomness.
3. Obtain the automata-controller manager from city slot 47. Ask the same occupant for its
   world position (slot 8, output pointer to three binary32 coordinates). Call controller-manager
   slot 10 with `(string "traffic_accident_2", occupant, positionPointer)`. Store its AL result
   as the attached byte, including false. This record is kept even if attachment fails.
4. Retain the occupant with AddRef and store it in the record (Release a different old occupant
   if present; a new record has none). Return the payload pointer in EAX. The creation body
   makes **zero RNG calls** and reads/restarts **no stopwatch**. Any RNG activity within the
   named controller belongs to the controller manager/script, not an extra manager admission draw.

The effect manager owns the token and occupant reference until removal. StopEffect searches
for the exact payload pointer; if found, an attached non-null occupant has all its controllers
removed through controller-manager slot 14, the attachment byte is cleared, its retained
occupant released and duration cleared, then the node is unlinked/freed. It returns true in AL
also when the token is absent (`0x00552B80`–`0x00552BCD`, `0x0054ECC0`–`0x0054ED06`).
Callers must not AddRef, Release or free the token.

The embedded stopwatch is constructed in unit 4 (milliseconds), with next-update and last-update
u32 values zero (`0x005543DA`–`0x005543F3`). Update reads elapsed milliseconds once; if
`now < nextUpdate` it returns. Otherwise set `nextUpdate=now+500` (u32), and in list order use
`delta=now−lastUpdate` (u32). Remove records whose remaining duration is **strictly less than**
delta; otherwise subtract delta, including the equality case which leaves a live zero-duration
record. A retained occupant is converted from its world position to a city cell through city
slot 95; a changed centre recomputes both rectangles. Finally set lastUpdate=now, also with
an empty list (`0x00552BD0`–`0x00552CFB`). CreateAccident does not give a new record its own
start stamp: the next effect update charges the shared delta to it.

**Boundary for replacing a manager while the simulator remains native.** City slot 43's vehicle
manager pointer is used as a *concrete receiver*, then the simulator calls `0x00554400` directly.
There is no QueryInterface/version test or vtable dispatch at that call. A replacement that only
implements the published manager vtable cannot service it: the native entry still adds 0xAB8 and
interprets replacement storage as an STL list. This explains the shape of the reported sentinel
faults; it does not establish that every reported pool fault has the same cause. A version-checked
integration must route this concrete three-argument operation to the owner of the replacement
manager, or replace the traffic-simulator caller as a whole. Supporting a public manager IID alone
does not establish that boundary. No guessed private padding/list layout or same-owner native
forwarding is licensed by this contract. No new public accident-creation interface was found.

## 4. Constructors and base helpers *[summary]*

- `0x005125E0` builds the base manager (its random generator and its group sets; `0x00512780` is its destructor);
  `0x00554360` adds the traffic part (the layer list and timers), called by the vehicle (`0x0055FB50`) and
  pedestrian (`0x00545500`) constructors, which set the default path type (automata.md §5.1); `0x004FBB40` and
  `0x00574860` are the aircraft and watercraft managers'.
- `0x00513A60`, the tuning-exemplar callback, rereads the exemplar data and the tracked groups (`0x0050CB40`,
  `0x005137D0`, `0x00512B60`, `0x00510FA0`, `0x00511960`, `0x00510650`, `0x00512C80`, `0x005120E0` maintain the
  group tables).
- `0x0050F790` adds an automaton to the manager's lists (called by every manager's `AddAutomaton`).
- The automaton record helpers `0x004FB5B0` / `0x004FBE70` are the aircraft manager's record (automata.md §5.2);
  `0x00518230`, `0x005183A0`, `0x005183D0` and `0x005DB190` belong to the attractor and generator records
  (automata.md §3.5).

## 5. Vehicle-family slots reached through the director *[summary]*

Slot numbers are those of the primary automaton table (automata.md §11). "All vehicles" means the vehicle, its
variants, the emergency vehicles, trains, train cars and trailers.

| Slot | Address | Classes | Behaviour |
|---|---|---|---|
| 4 | `0x00532FB0`, `0x00543EC0` | fire truck, patrol car | `Shutdown`: releases the crew or riot police and the dispatched incident, then the emergency vehicle's |
| 23, 24 | `0x00505030`, `0x00505040` | emergency | get and set the call number (`u16` at `+0x2CA`) |
| 72 | `0x0052F3F0` | emergency | the animation tick: the vehicle's; then a dispatched vehicle (flag `0x80`) for which slot 85 is false calls slot 147 with a stored value and `T+0x0F8`, and raises Lua event `0xCA45190F` *[summary]* |
| 73 | `0x005304B0` | emergency | may this vehicle follow another: an undispatched vehicle uses the base rule (`0x005059C0`); a dispatched one may follow another dispatched vehicle that lacks flag `0x20`, or one that has it when its own route is used up and `0x00530130` gives at least 3 |
| 79 | `0x0052F210` | emergency (also `0x00532AD0`) | the arrival handler (automata.md §6.5, dispatch.md) |
| 105 | `0x0052F730` | emergency | the behaviour refresh override (automata.md §11) |
| 110, 111 | `0x0052F7C0`, `0x0052F1C0`; fire truck `0x00532E60`; patrol car `0x00543E80`, `0x00543B80` | emergency | enter: state 16 raises event 7 (slot 102), turns the pull-over request off (slot 194) and raises Lua event `0x6A451908`; state `0x34` turns it on; state `0x67` fades out over `T+0x0BC`. Exit of 16 or `0x34` turns it off. The fire truck releases its crew on leaving state 16 and the patrol car its riot police on entering it; the patrol car's exit of state `0x68` plays animation 0 |
| 115 | `0x0052F1B0` | emergency | the player-drive tick (a stub) |
| 120 | `0x0055B740`, trains `0x005572D0` | all vehicles | `DoCollideTick` (automata.md §6.5); trains first raise a short collide timeout to 10,000 ms |
| 123, 124 | `0x005573C0`, `0x0055BE70` | all vehicles | calls into the object of city slot 43 *[summary]* |
| 125 | `0x0055DF50` | vehicles | `GetSpeedModifier`: slowed on bridges and slopes (tuning factor) *[summary]* |
| 131, 132 | `0x0055D210`, `0x0055D5A0`; pedestrians `0x0054E920`; train car `0x00557320` | traffic | begin a path on a network tile; place the automaton on a network occupant (vehicles.md §2) |
| 135 | `0x0055BE90`; emergency `0x0052F300` | vehicles | the route end: a self-controlling vehicle in the city whose height differs by more than 5 from the higher of the terrain and the water pushes state 14 (the fall); otherwise the traffic automaton's (`0x0054D820`); a dispatched emergency vehicle runs slot 79 instead |
| 140, 143, 178–183 | `0x00504FF0`, `0x00505050`, `0x00505110`–`0x00505140`, `0x00505000`, `0x00505010` | all vehicles | getters and setters of three stored vectors (`+0x264`, `+0x270`, `+0x27C`) and the emergency target (`+0x2B8`) |
| 146 | `0x0052F9A0` (also `0x00543B30`) | emergency | at the target: within the dispatch radius of it and no occupant of flag `0x304` in the cell |
| 151 | `0x0055D9D0` | all vehicles | the gap ahead to the leader or the path end *[summary]* |
| 155 | `0x0054EA80` | all vehicles | the unit vector toward a point ((0, 0, 1) when nearer than 0.00001) |
| 158 | `0x0055B690` | road vehicles | a random lateral offset from the tuning range `T+0x128`–`T+0x12C` |
| 159 | `0x0054DA00` | all vehicles | slows down (speed tween to 0) |
| 165 | `0x005332C0` | fire truck | keeps the truck on its route when it has a crew to set up *[summary]* |
| 168, 169 | `0x005527E0`, `0x00552520` | all traffic automata | look-ahead entries of the route (vehicles.md §2) *[summary]* |
| 170 | `0x0054E790` | all traffic automata | a straight move toward a point with collision *[summary]* |
| 174 | `0x0055BF60` | all vehicles | forwards to city slot 43 |
| 176 | `0x0055DBE0` | road vehicles | shows or hides the lights or extra models *[summary]* |
| 181, 182, 188, 189 | `0x00561910`, `0x0055FE60`, `0x0055C480`, `0x00561D20` | all vehicles | route bookkeeping and path finding helpers *[summary]* |
| 193 | `0x00532BF0`, `0x00543BC0`, `0x00504BE0` | fire truck, patrol car, ambulance | the dispatch radius: from the fire (city slot 57) or police (city slot 65) simulator for the vehicle's station (`+0x2C4`), 16 without one; the ambulance a constant |
| 194 | `0x00530AF0` | emergency | turns the pull-over request (`automata_pullover`) on and off |
| 197 | `0x00543C10` | patrol car | `DoHandlingEventTick` (the fire truck's is vehicles.md §7.2): with no riot in progress (city slot 72, slot 10): in dispatch state 2 it goes home (slot 29); in state 5 it returns to state 2 |
| 199 | `0x00533060`, `0x00543F10` | fire truck, patrol car | ends the handling: releases the crew or riot police (outside state 16 for the truck) and sets dispatch state 2 |
| 198 | `0x0052F570` | emergency | `DoTeleportTick` (automata.md §6.5) |

## 6. Quirks

1. The pedestrian manager's slot 87 rises with congestion, the vehicle manager's falls; both are 1.0 when the
   traffic data is off.
2. A dispatch request is retried at most 5 times, 1000 ms apart, whatever its kind.

## 7. Open questions

See [questions/automata-managers.md](questions/automata-managers.md) (AD1–AD2).

## 8. Conformance tests

1. The director's class list has 30 entries; asking it for class `0x6C1ACDE1` gives an ambulance, an emergency
   vehicle of `0x2D8` bytes.
2. Slot 74 of the vehicle manager gives `0x4215` for layer 4 and the default group for layer 7.
3. With the clocks on, slot 42 for type 9 reads clock 4.
4. The population pass stops a frame's walk after 5 ms and resumes at the next tile in the next frame.
5. A dispatch request whose occupants never appear is dropped after the sixth expiry.
