# Spec: automata support (the tuning exemplar, the controller grid, prop controllers, attractor descriptions)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`) and from the shipped
`SimCity_1.dat`. Implementers must work from this spec, the public LGPL gzcom-dll SDK headers and general knowledge
only. They must **not** consult decompiled or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[summary]* is a correct outline whose details are not given.

Related specs: [automata.md](automata.md) §3 (controllers, queues and their records), §7 (the managers), §11
(slot names); [pedestrians.md](pedestrians.md), [vehicles.md](vehicles.md), [aircraft.md](aircraft.md),
[watercraft.md](watercraft.md) (the consumers); [prop-occupant.md](prop-occupant.md) §1.2 (the prop classes);
[data-formats.md](data-formats.md) (exemplars and response curves).

## 0. Summary

All automata read one set of tuning values, the **automata tuning parameters**: a static block filled with
defaults at start-up and then from one exemplar in the game data. Controllers are found by area through the
**controller grid**. A prop can run a controller (the **prop occupant automata controller**), and effect
descriptions can place controllers in the world through **attractor descriptions**.

## 1. The automata tuning parameters *[compat]*

### 1.1 The block

`cSC4AutomataTuningParams` (constructor `0x00512260`) is one static block (`0x00B21BC8`, written `T+offset` in the
automata specs). The constructor sets the defaults of §1.3, builds six response curves each holding the two points
(0, 0.5) (a flat 0.5), an empty value list (`+0xE0`) and two empty maps (§1.4).

### 1.2 Reading the exemplar (`cSC4AutomataManager::ReadTuningParameters`, `0x00512CF0`)

The exemplar is `0x6534284A` / `0xA998D30B` / `0x00000001` (`SimCity_1.dat`, 80 properties). When the resource
manager has no such exemplar nothing is read and the defaults stand. Otherwise each property of §1.3 is read into
its field. A property that is absent leaves its field as it was, except where §1.3 says otherwise. "Mask" fields
are built from a `u8` list: each value v sets bit v − 1 (or bit v where noted); a list with a 0 value, or no
property, gives all bits set (−1).

### 1.3 The fields

Values are `f32` unless the type says otherwise. "Shipped" is the value in `SimCity_1.dat`. Users are the
functions that read the field outside the reader (by name where they have one).

| `T+` | Property | Type | Default | Shipped | Notes and users |
|---|---|---|---|---|---|
| `0x000` | `0x4998C89F` | `u32` | 300 | 250 | aircraft manager's tuning handler, vehicle manager, U-Drive-It input |
| `0x004`, `0x008` | `0x4998C900` | 2 × `u32` | 100, 120 | 80, 110 | |
| `0x00C`, `0x010` | `0x4998C902` | 2 × `u8` → `u32` | 10, 30 | 10, 20 | |
| `0x014` | `0x4998C904` | `u32` | 300 | 512 | pedestrian manager |
| `0x018`, `0x01C` | `0x4998C905` | 2 × `u32` | 5, 10 | 15, 20 | pedestrian default speed range, × 1/3600 (pedestrians.md §2.1) |
| `0x020`, `0x024` | `0x4998C909` | 2 × `u8` → `u32` | 5, 10 | 2, 4 | |
| `0x028` | `0x4998C90B` | `u32` | 100 | 20 | watercraft manager's tuning handler |
| `0x02C` / `0x03C` | `0x4998C89C` / `0x4998C89E` | curve / scale | flat 0.5 / 3.0 | 4 values / 10 | |
| `0x040` / `0x050` | `0x4A511873` / `0x4998C89E` | curve / scale | flat 0.5 / 0.5 | 4 values / 10 | the scale reuses `0x4998C89E` *[quirk]* |
| `0x054` / `0x064` | `0x0A51188A` / `0xEA511DE0` | curve / scale | flat 0.5 / 0.1 | 4 values / 0.01 | |
| `0x068` / `0x078` | `0x2A511893` / `0xCA511DE6` | curve / scale | flat 0.5 / 0.1 | 4 values / 0.05 | |
| `0x07C` / `0x08C` | `0xCA51189A` / `0xCA511DF2` | curve / scale | flat 0.5 / 3.0 | 4 values / 0.1 | |
| `0x090` | `0xEC0700C0` | | 0 | 100 | |
| `0x094` | `0x6C0700C4` | | 0 | 0.01 | |
| `0x098` / `0x0A8` | `0x4998C901` / `0x4998C903` | curve / scale | flat 0.5 / 10 | 4 values / 4.0 | |
| `0x0AC` | `0x4998C912` | | 1.0 | 0.75 | the traffic blocked wait (`UpdateCollision`, automata.md type 13) |
| `0x0B0`–`0x0B8` | `0x4998C913`–`0x4998C915` | 3 × | 5, 5, 5 | 1.2, 6, 8 | intersection timing (`cSC4TrafficNetworkMap::UpdateIntersectionTiming`, `TickIntersections`) |
| `0x0BC` | `0x4998C916` | | 1.0 | 1.0 | the fade time of many automata (`TweenOpacity`) |
| `0x0C0` | `0x2A511CBC` | | 0.7 | 0.5 | |
| `0x0C4` | `0xCA5545EA` | `u32` → × 1/3600 | 150 | 550 → 0.1528 | absent: 100/3600 (the field is always rewritten) |
| `0x0C8`, `0x0CC` | `0x4998C90A` | 2 × | 0.5, 0.7 | 0.3, 0.5 | traffic network map |
| `0x0D0` | `0x4998C91D` | `u8` | 2 | 3 | traffic network map |
| `0x0D4` | `0xA9AF1E54` | mask (bit v − 1) | −1 | (1, 2) → 3 | traffic network map |
| `0x0D8` | `0xA9AF1E55` | mask (bit v − 1) | −1 | (1–4) → `0xF` | traffic network map |
| `0x0DC`, `0x0DD` | `0x4998C918`, `0x4998C919` | `u8` − 1 | 2, 3 | 3, 4 → 2, 3 | vehicle manager, pedestrian manager. The 1 is subtracted even when the property is absent *[quirk]* |
| `0x0E0` | `0x4BAE8FE0` | `f32` list | empty | 12 values | |
| `0x0EC` | `0x89AEC88B` | | 1.0 | 5.0 | emergency re-path wait, s (`DoRepathingTick`) |
| `0x0F0` | `0x89AEC88C` | `u8` | 3 | 8 | emergency re-path distance, × 16 (`AppendRouteToNearbyRoad`) |
| `0x0F1` | `0x89AEC88D` | `u8` | 3 | 5 | emergency re-path tries (`DoRepathingTick`) |
| `0x0F4` | `0x89AEC88E` | `u32` | 150 | 310 | `cSC4EmergencyVehicle::Init` |
| `0x0F8` | `0xC9EE640D` | `u8` | 4 | 7 | |
| `0x0F9` | `0x49BE9565` | `bool` | true | true | `cSC4RegionalCity::CreateSymmetricConnections` |
| `0x0FC` | `0xC9C18EAE` | | 1.0 | 1.0 | `cSC4VehicleManager::HandleNewBuilding` |
| `0x100`, `0x104` | `0xA9D6CC17` | up to 2 × | 7, 6 | 7, 6 | `cSC4SimFireCrew::SetupFireCrew` |
| `0x108`–`0x110` | `0xA9D6CC18` | up to 3 × | 1.0, 0.6, 0 | 1.0, 0.6, 0.0 | fire-crew competence levels (pedestrians.md §4.1); `cSC4RiotPolice::Init` |
| `0x114` | `0xC9EE640C` | | 75 | 128 | fire-crew target radius; fire truck U-Drive-It |
| `0x118`, `0x11C` | `0xC9DA95D4` | 2 × | 0, 0 | −60, 80 | missing values: −90, 90 |
| `0x120`, `0x124` | `0x29DD40C1` | 2 × | −0.5, 0.5 | −0.5, −0.5 | missing values: −1, 1 |
| `0x128`, `0x12C` | `0x29DD40C2` | 2 × | 0, 0 | 0, 0 | missing values: −1, 1 |
| `0x130` | `0xC9EE640A` | degrees → radians | 0.7854 | 45 → 0.7854 | set to 0 before the read, so absent gives 0 *[quirk]*; trains |
| `0x134` | `0xC9EE640B` | | 0.3 | 0.3 | set to 0 before the read *[quirk]*; trains |
| `0x138` | `0x6A09F62F` | | 0.01 | 1.0 | |
| `0x13C`, `0x13D` | `0x6A09F630` | 2 × `u8` | 3, 10 | 3, 10 | the one-shot play count range (pedestrians.md §3) |
| `0x13E`, `0x13F` | `0xEA441C97` | 2 × `u8` | 5, 8 | 9, 15 | `cSC4Train::Init` |
| `0x140`, `0x141` | `0x8A441C9C` | 2 × `u8` | 3, 8 | 3, 8 | `cSC4Train::Init` |
| `0x144`, `0x148` | `0x6A440D40` | 2 × `u32` | 140, 150 | 140, 150 | `cSC4Train::Init` |
| `0x14C` | `0x0A53FD32` | `u32` | 40 | 40 | |
| `0x150`–`0x15C` | `0x6B42922C` | 4 × | 0.25, 1, 2, 0 | 1, 2, 4, 0.5 | four values are copied whatever the count *[quirk]*; airplane animation tick |
| `0x160` | `0x6BAC0B3E` | `u8` | 0 | 3 | |
| `0x164` | `0x4BAE8FE1` | | 0 | 0.5 | |
| `0x168` | `0x4BAE8FE2` | | 0 | 8 | |
| `0x16C` | `0xEBFB4A0A` | `u8` | 5 | 5 | the city situation manager's maximum CSI |
| `0x170` | `0xCC101142` | | 0.1 | 1.0 | |
| `0x174` | `0x6C200139` | `u32` | 100 | 100 | `cSC4AutomataManager::EnterPlayerDriveView` |
| `0x178` | `0x8C3E8185` | `u32` → 1 / value | 0 | 50 → 0.02 | absent: 1/50 (always rewritten) |
| `0x190` | `0x4998C91C` | `bool` | true | true | |
| `0x194` | `0x4998C89D` | mask (bit v) | −1 | (1–10) → `0x7FE` | |

The users that this table does not name are unnamed functions of the automata COM director's code
*[summary]*.

### 1.4 Clocks and spawn-and-recall entries

Eight properties are **clocks**, read by `ReadTuningClock` (`0x00512860`) into the map at `T+0x17C` keyed by an
index: `0x4998C90C` → 1, `0x4998C90D` → 2, `0x4998C911` → 3, `0x4998C90F` → 4, `0x4998C90E` → 5, `0x4998C910` →
7, `0x6A09F631` → 8, `0xEC06E7CF` → 11. Each is a response curve (shipped: point lists such as 0 → −0.15, 1 → 0,
4.5 → 0, 5 → 0.2, 5.5 → 0.5, 6.5 → 0.9, …) whose x is the hour of the day *[inferred: x runs from 0 to 24]*.

Five properties are **spawn-and-recall** entries, read by `ReadSpawnRecallParameters` (`0x00512940`) into the map
at `T+0x198` keyed by an index: `0xCBA5895B` → 3, `0xCBA5895C` → 0, `0xCBA5895D` → 6, `0xCBA5895E` → 2,
`0xCBA5895F` → 1 (not shipped). An entry holds four values (defaults 0.2, 0.5, 0.2, 0.5; shipped 0.4, 0.3, 0.4,
0.3) and a curve (default (0, 0)–(1, 1)) *[summary of the use]*.

## 2. The controller grid (`cSC4ControllerGrid`) *[compat]*

The controller manager keeps its controllers in a grid of square **tracts** of 8 × 8 cells (128 m).

- `Init` (`0x0051C090`): n := the city's cell count in x / 8 (truncated); the grid is n × n (the z count is not
  used for the size *[quirk]*); the row shift is the k with 2^k = n (a size that is not a power of two leaves it at
  32, used modulo 32 *[quirk]*); every tract starts with an empty list. The world extents are 16 × the cell counts.
- `GetTractByWorldPos(x, z)` (`0x00515660`): negative coordinates become 0; tract (⌊x / 128⌋, ⌊z / 128⌋), each
  clamped to n − 1 (floor by the rounding of foundations.md §18); the tract is entry (tz << shift) + tx.
- `GetGridBounds(controller)` (`0x00515290`): the controller's world rectangle (controller slot 21) gives the tract
  bounds x0, z0, x1, z1 (bytes) the same way; only x1 and z1 are clamped to n − 1.
- `AddController` (`0x00517A90`): stores the bounds in the controller (controller slot 34) and adds the controller
  to every tract inside them that does not list it yet.
- `MoveController` (`0x00517B70`): when the bounds changed, the controller is added to the tracts of the new bounds
  outside the old ones and removed from the tracts of the old bounds outside the new ones, and the new bounds are
  stored.

The queues of automata.md §3.3 (`tControllerQInfo`) are specified there.

## 3. Prop occupant automata controllers *[compat]*

`cSC4PropOccupantAutomataController` (class `0x2A4017A0`, prop-occupant.md §1.2) is a prop that runs one
automata controller.

- `Init` (`0x005EFF60`), once: the size from property `0x27812810` (three values, default 16, 16, 16); the
  controller template name from property `0xC98204B8`, or else the exemplar name (`0x20`); the occupant's `Init`;
  with a name, a controller and occupant flag `0x200`, the controller is activated.
- `SetControllerTemplate(name)` (`0x005F06F0`) and `SetState(n)` (`0x005F0530`): a new value is stored and the
  controller restarted; the same value does nothing.
- `RestartController` (`0x005F00B0`): the current controller is deactivated (controller slot 18), removed from the
  controller manager (city slot 47, slot 7) and released. With a name, a new controller is made by the controller
  manager (slot 5) for this prop from the template `name` when the state is 0, else from `name_<state>` (format
  `%s_%d`).
- `ActivateController(on)` (`0x005EDE60`): passes `on` to the controller (slot 18) and stores it.
- `Shutdown` (`0x005EEDD0`): the controller is removed from the manager and released; then the occupant's
  `Shutdown`.
- **Record** (Write `0x005EDDB0`, Read `0x005EFBF0`): `u16` 3 (Read requires 3), the prop occupant record, `bool`,
  `GZStr` (the template name) *[as read: the bool is the active flag]*. No corpus save holds one.

## 4. Attractor descriptions (`cSC4AttractorDescription`) *[summary]*

A description (`0x00580F10`) holds a controller template name and a flag. Record: `u16` 0, the name (`SetRZCharStr`),
`bool` (Write `0x00580B70`; Read `0x00581290` ignores the version).

An instance (`0x00580BA0`, made by `0x00580E90`) holds its description, a position, an active flag, an elapsed time
(`0x00580B60` adds to it), a point occupant (`0x00580C20`, an occupant whose position is a stored vector,
`0x005B2AE0` / `0x005B2B20`) and a controller.

- **Start** (`0x00580FD0`): when inactive and the occupant manager exists: active, time 0, a new point occupant,
  initialised and added to the occupant manager (slot 20). With the flag clear, the controller manager (city slot
  47, slot 5) makes a controller from the template for the occupant; without one the instance is inactive again.
  With the flag set, the controller manager attaches the named controller to it (slot 10, as for the accident and
  fire controllers). Then the occupant gets the position (slot 9), the occupant manager and the controller manager
  are told (slots 24 and 15).
- **Stop** (`0x00581120`): when active: a made controller is removed (controller manager slot 7) and released, an
  attached one detached (slot 12); the occupant is removed from the occupant manager (slot 21), shut down and
  released; inactive.
- **Move** (`0x00580D20`): the position is an offset carried through a parent transform when both use one, else
  the plain position; the occupant and the controller manager get it.

## 5. Quirks

1. `0x4998C89E` is read twice, into `T+0x03C` and `T+0x050`, so the two curves share one scale in the shipped data.
2. `T+0x0DC` and `T+0x0DD` are decremented on every read, even when their properties are absent.
3. `T+0x130` and `T+0x134` become 0 when their properties are absent, though the constructor gives 0.7854 and 0.3.
4. `0x6B42922C` is copied as four values without a count check.
5. The controller grid sizes both axes from the x cell count.

## 6. Open questions

See [questions/automata-support.md](questions/automata-support.md) (AN1–AN2).

## 7. Conformance tests

1. With the shipped exemplar, `T+0x018` = 15, `T+0x01C` = 20, `T+0x0C4` = 550/3600, `T+0x0D4` = 3, `T+0x0DC` = 2,
   `T+0x178` = 0.02 and `T+0x194` = `0x7FE`.
2. Without the exemplar, every field keeps the constructor default of §1.3.
3. An exemplar whose `0xC9DA95D4` holds one value v gives `T+0x118` = v and `T+0x11C` = 90.
4. In a 256-cell city the controller grid is 32 × 32; a controller at x 0–200, z 300–700 is listed in tracts
   x 0–1, z 2–5.
5. A prop controller with template `crowd` in state 2 restarts with the template `crowd_2`.
