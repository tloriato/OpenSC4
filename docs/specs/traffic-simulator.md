# cSC4TrafficSimulator: commute trips, traffic volume and the traffic maps (behaviour spec)

Status: M5 behaviour spec, verified on the Windows executable `bin/SimCity4.exe` (1.1.641, image base `0x00400000`).
Every address below is a Windows address. `docs/subsystems/traffic.md` and `re/symbols/traffic.tsv` (mostly Mac
build) were used as a starting point for names only; the differences found are listed in the final section of
§10.

Related specs:

* [traffic-network.md](traffic-network.md): the per-cell network model this class owns and keeps up to date (network
  info per cell and transit network, cell connection words, transit switches and their rules, tunnels, ferries and
  water routes, neighbour connections, subnetworks and the connectivity queries, the lot insertion point,
  `DoConnectionsChanged`). This spec only says **when** those structures are read or rebuilt.
* [pathfinder.md](pathfinder.md): `cSC4PathFinder`, the A\* search, the compressed route format (§4.15), the goal test
  with destination capacity (§4.10) and the class statics the search reads. This spec says how the simulator
  configures each search and what it does with the result.
* [foundations.md](foundations.md) §4 (response curves), §6 and §12 (random numbers), §18 (x87 precision);
  [simulator.md](simulator.md) §4–§5 (agents, levels, slicing); [simgrid.md](simgrid.md) (grids; §6.4 raw readers);
  [budget.md](budget.md) §7–§8 (departments and line items); [ordinance.md](ordinance.md) §6.3;
  [neighbors.md](neighbors.md) (neighbour connections; their edge records and pools are traffic-network.md §2.7);
  [growth-developer.md](growth-developer.md) (the commute layer reads the trip-length map);
  [docs/abi/allocators.md](../abi/allocators.md).

Notation. *[compat]* = observable behaviour a replacement must reproduce. *[quirk]* = surprising original behaviour
that is reproduced. *[choice]* = FreeCity4 decision where the original is undefined or non-deterministic.
*[inferred]* = not proven. `r24(e)` = `e` rounded to float32 (foundations §18: every x87 add, subtract, multiply and
divide on the simulation thread rounds to a 24-bit significand, also when an operand is a double or an integer).
`round(v)` = the `1.5·2^23` idiom: `r24(v + 12582912.0f)` (constant `0x00AB32D8`, a **dword**), bit pattern minus
`0x4B400000` = round half to even, exact for `|v| < 2^22`; outside that range the result is garbage *[compat]* (all
sites below stay inside it or clamp first, unless stated). `ftol(v)` = the runtime's `_ftol` (`0x009EEF04`):
truncation toward zero to **int64**, of which the callers keep the low 32 (or 64) bits.

---

## 1. Role, identity, creation, Init

The traffic simulator turns the city's population and jobs into commute and freight **trips**, routes them over the
transport networks with the pathfinder, accumulates the resulting **traffic volume** per cell and transit network,
and publishes the derived maps: congestion, air-polluting traffic, commercial traffic and commute **trip length**.
It also collects mass-transit fares and tolls, drives the roads and mass-transit departments' monthly costs,
records history, runs the accident check and the mass-transit strike, and owns the network model of
traffic-network.md.

| Fact | Value |
|---|---|
| Class ID | `0x6990C1AA` (city component) |
| Interfaces (QueryInterface `0x0070D7E0`) | `cIGZUnknown` (`1`) and `cISC4TrafficSimulator` (`0x68027F37`) → the object; `cIGZMessageTarget2` (`0x452294AA`) → object + 4; `cIGZSerializable` (`0xE4FDA3D4`) → object + 8. Any other IID: false. |
| Primary vtable | `0x00AB33D0`, 44 slots (§7) |
| Message-target vtable | `0x00AB33BC`; `DoMessage` `0x007241B0` (runs with `this` = object + 4) |
| Serializable vtable | `0x00AB33A4`: slot 3 `Write` `0x00712400`, slot 4 `Read` `0x00721990`, slot 5 `GetGZCLSID` (all with `this` = object + 8) |
| `GetSimulatorType` (slot 5) | returns `0x88027F0D` |
| City accessor | `cISC4City` slot 69 (`+0x114`) `GetTrafficSimulator` |

**Release** (slot 2, `0x0070E2F0`): above 1 it decrements and returns the new count; at 0 or 1 it calls `Shutdown`
(slot 4) and the scalar deleting destructor (slot 43, flag 1: free with allocator C `0x005E5620`) and returns 0.

**Init** (slot 3, `0x0071E7E0`), only when not yet initialised, in this order *[compat]*:

1. `ReadSimulatorTunableValues` (§4.14).
2. Registers its message target (object + 4) as a simulator agent at level 0 (one-shot), level 6 (month) and level
   4 (day), name `"cSC4TrafficSimulator"`, flags 2 (simulator slot `+0x4C`, simulator.md §4).
3. Subscribes to `0x99EF1142`, `0x99EF1143` (occupant inserted / removed), `0x26D31EC1` (post city init),
   `0xE8BDA2E7` (lot (de)activated), `0xC53D10AA` (tuning reload), `0x8A0D15DA`, `0xEA0D163E` (neighbour connection
   added / removed), `0x08DAC3E7` (budget change), `0xC2C09554` (`OrdinanceUpdated`).
4. Reads the city size `W × H` cells (city slots 93, 94) and creates the grids, each with `SetTractSize` and `SetID`:

| Grid | Element | Tract | ID | Saved | Meaning |
|---|---|---|---|---|---|
| air-polluting traffic | u8 | `tractSize` (code default 1, not read from tuning) | `0x89D5C3A2` | yes | slot 8 (§4.10) |
| peak volume | f32 | `tractSize` | `0x69D5C3AC` | yes | per-cell maximum volume (§4.9) |
| commercial traffic | u8 | **4** | `0xCABD2925` | yes | slot 9 (§4.10) |
| commercial traffic (work copy) | u8 | 4 | `0xCABD2926` | yes | built during the map pass, copied into the published grid |
| (unused) | u8 | 1 | `0x69D5C3B5` | no | created, never written *[inferred]* |
| trip length | u8 | 1 | `0x69D5C3BF` | yes | slot 13 (§4.6) |
| traffic volume, one per transit network `n` = 0…5 | f32 | 1 | `0x69D5C3E0 + n` | yes | §4.5 |
| (float) | f32 | 1 | `0x69D5C3FA` | yes | not written by the code in this spec *[inferred]* |
| congestion | u8 | 1 | `0x69D5C402` | yes | slot 10 (§4.10) |
| peak volume/capacity ratio | f32 | 1 | `0x69D5C40E` | yes | §4.9 |
| transit-switch mode-mask map (corrected: not a "wormhole" map; TN13) | u8 | 1 | `0x69D5C424` | yes | traffic-network.md §2.3 |
| transit-switch ID map | u32 | 1 | `0x69D5C42D` | yes | traffic-network.md |

   Grid element types are the framework's grid classes (u8 `0x49B9E602`, f32 `0x49B9E60A`, u32 `0x49B9E606`;
   simgrid.md). The destination bitmask map is a plain `(W+2) × (H+2)` byte array indexed `[x+1][z+1]` (pathfinder.md
   §4.1), zero-filled.
5. Allocates the cell connection array (traffic-network.md), the generator `cRZRandomSuper` seeded from the wall
   clock (§4.15), the scanning iterator over the city rect (simulator.md §5.5 slicing), and the department budgets:
   roads (`0xC921CEEA`) and mass transit (`0xC921CEEB`) with line items 1…6 (budget.md §7–§8; the exact calls are
   in §9.1, TG7).
6. Creates two pathfinders through its own slot 6 (`CreatePathFinder`): **pf** (outbound) and **pf2** (return trip).
7. Creates the lot iterator (lot manager slot 15), the traffic-source iterator (§4.2), its property holder, the
   ferry `cSC4WaterPathFinder` (traffic-network.md) and the accident timer.

**Init after Read** (TG4). In a city load every saved object is `Read` **before** any `Init` (foundations §3.7), so
this is the normal order, not a special case. Init keeps what Read loaded: each grid, the scanning iterator, the
generator, the two departments, the pathfinders and the lot iterator are created **only when the member is still
null** (`0x0071E9BC`–`0x0071EC32` grids; `0x0071F461` iterator; `0x0071F132` generator; `0x0071F1AF`, `0x0071F263`
departments; `0x0071F50A`, `0x0071F529` pathfinders; `0x0071F54C` lot iterator). The cycle state is set to 0 only
together with a newly created scanning iterator (`0x0071F4FE`), so a loaded state survives. The destination map is
resized and zeroed only when its row count differs from `W + 2` (`0x0071EE7E`). What Init rebuilds unconditionally
is unsaved: the tuning values (§4.14), the agent registrations and subscriptions, the four neighbour edge vectors
(zero-filled to W or H entries, `0x0071E961`–`0x0071E99D`), the per-cell network-info table (`0x0071EFF3`) and the
cell connection array (allocated anew and filled with the empty record, `0x0071F0DF`); DoPostCityInit (§3.5)
rebuilds those from the city.

The constructor (`0x0071F830`; tuning defaults in §4.14) sets the cycle state to 0 (StartRecalc), the last
seen day of month to −1, the strike state to "no strike" (2) and the transit-switch cost multiplier to 1.0.

**Shutdown** (slot 4, `0x00719290`, TG11): returns at once (true) when not initialised. Otherwise it clears the
initialised flag, releases the pathfinders, grids, departments and iterators *[inferred: that order is not part of
the contract]*, then unsubscribes the nine messages of Init step 3 (in the order `0xC2C09554`, `0x08DAC3E7`,
`0x99EF1142`, `0x99EF1143`, `0x26D31EC1`, `0xE8BDA2E7`, `0xC53D10AA`, `0x8A0D15DA`, `0xEA0D163E`) and releases the
message server, then removes its agent from **every** level with `cISC4Simulator::RemoveAgent(object + 4)` (slot 20,
the one-argument overload) on the city's simulator (city slot 50 `GetSimulator`; skipped when null,
`0x00719807`–`0x00719821`). It then releases the strategy-bucket and other containers, the property holder and the
ferry water pathfinder (its own `Shutdown` first), and returns true.

*The traffic-source iterator in Shutdown* (TG26, `0x007196F8`–`0x00719765`; the iterator object is the member at
`+0x908` of the simulator's primary base). If it exists: its connection list is cleared, releasing every connection
reference of the snapshot (`0x00411D70`: `Release` on each element, then the node freed); the list's sentinel node is
freed; the 16-byte iterator object is freed with allocator C; the member is set to null. This comes right after the
simulator's own safe lot iterator is unregistered and freed (`+0x904`), and before the message unsubscriptions.
*[quirk]* The **lot** safe iterator that the traffic-source iterator holds (its first field, zones.md §8.3.1) is
neither unregistered nor freed here: it stays in the lot manager's registry until the lot manager itself goes. A
later `Init` sees a null member and creates a new iterator (`0x0071F5FF`). The destructor does not touch the member;
`Release` of the last reference calls `Shutdown` first (`0x0070E305`), which returns at once if the simulator was
already shut down. On a city close the city's `Shutdown` runs the simulator's `Shutdown` (foundations.md, city close
order), so the snapshot is released at the close, not when the simulator object is finally destroyed.

## 2. State

The data model, by meaning (offsets are deliberately not part of it):

| State | Notes |
|---|---|
| Tuning values (§4.14) | read at Init and on the reload message |
| The grids of §1 and the destination bitmask map | §4.5–§4.11 |
| Free destination counts `freeDest[k]`, k = 0…3 | number of lots that set destination bit `k`; written by the pathfinder too (pathfinder.md §4.10.1) |
| Destination-type capacity totals `jobCapacity[k]`, k = 0…2 | §4.11 |
| Commuter statistics per trip type k = 0…2: `attempted[k]`, `routed[k]` | §4.6 |
| Edge-density map: key → record of 33 u16 | §4.8 |
| Transit switches (hash: switch ID → record with usage, previous usage, fee, ratio, rules) | traffic-network.md; usage and fee accumulation in §4.5 |
| Accident-prone list (cells with probability), and the copy used by the accident check | §4.9, §4.13 |
| Per-network-type statistics (13 types, current and last) and the traffic stats block (slot 34) | §4.9 |
| Income accumulators (two doubles: income, fees), monthly income floats (transit, tolls, total) | §4.5, §4.12 |
| Commute accumulators: car-commute length sum and count, freight length sum and count | §4.6, §4.12 |
| Per-travel-type trip counts (two vectors of 9 u32: building `+0x430`, published `+0x424`; the end of FullTrips swaps them, §4.4; saved as pairs (published, building), §6.1 row 19; TS56) | §4.6 |
| Cycle state machine: state, phase-entry flags, slicing step state, iterators | §3 |
| Day pacing: day index, days for the phase, total, processed counters, the ten pass counters (§10 Q4); the last day of month seen (`+0x1AC`, unsaved, TS50) | §3.3 |
| Re-route context (current source, search state, flags, work budget) | §4.4 |
| Re-route bucket `rerouteBucket` (0…3) | §4.3 |
| Strategy bucket counter, strategy bucket multimap (bucket → lot cell) | §4.1.3 |
| Travel strategy percentages `pct[w][s]` (4 wealth rows × 3 strategies, u8) | §4.1.2 |
| Strike state, damaged-cell set | §4.13, traffic-network.md |
| The network model | traffic-network.md |

**Trip data** (`cSC4TripData`, `0x2C` bytes) lives in each traffic source's list (§4.1):

| Field | Type | Meaning |
|---|---|---|
| route | `vector<u8>` | compressed route of the outbound leg (pathfinder.md §4.15) |
| returnRoute | `vector<u8>` | compressed route of the return leg. A freight or reversible result does **not** touch it: it keeps whatever an earlier two-leg result, the split it was copied from or the save left there (TS58, §4.4) |
| count | u32 | number of trips in this batch |
| type | u8 | trip type = destination type: 0, 1, 2 = commuters of wealth $, $$, $$$ (jobs of that wealth), 3 = freight (§4.1) |
| strategy | u8 | lots: travel strategy 0 = mass-transit preference, 1 = car preference, 2 = neutral; neighbour connections: a travel type 0…8 (§4.1.4) |
| flags | u8 | bit 0 *reversible* (routed one way, counted both ways), bit 1 *probe*, bit 2 *forced start type* (`1 << strategy`), bit 3 *routed in the last full pass* |
| status | u8 | 0 needs a full search, 1 needs a re-route to its known destination, 3 routed, 4 new (lot side), 5 failed |
| destX, destZ | s16 | destination cell of the last successful search |
| tripLength | f32 | trip length of the last successful search |
| travelTypesUsed | u32 | mask `1 << t` of the travel types on the route(s) |

## 3. Agents, the daily work cycle and the state machine

### 3.1 Messages handled (`DoMessage` `0x007241B0`)

| Message | Condition | Action |
|---|---|---|
| `0x66956810` (SimBegin, the one-shot agent) | | sets the **network-changed byte** := 1 and returns true. The byte is cleared by the constructor and saved and loaded (§6.1 row 14: the `bool` after the intersection-dirty rectangle). Nothing else reads it, in 1.1.641 (`0x00724558`, its only writer besides the constructor `0x0071FC1A`) or in the Mac build. No rebuild follows: the network model is kept current by the occupant and neighbour handlers below (traffic-network.md §3) and rebuilt only by DoPostCityInit (§3.5). *[compat: keep the byte for the round trip; it drives nothing]* (TS5) |
| `0x66956814` (day agent) | | `DoDailySimulation` (§3.3); its result is the agent's "finished" answer |
| `0x66956816` (month agent) | | `DoMonthlySimulation` (§4.12) |
| `0x26D31EC1` | | `DoPostCityInit` (`0x00720430`, §3.5: builds the network model, traffic-network.md, then resumes the saved cycle) |
| `0x08DAC3E7` | data2 = `0xC921CEEB` (mass transit) | `transitMultiplier` (the transit-switch entry cost multiplier, traffic-network.md §4.7.3) := curve `0xCA5F7821`(`r24(f × 100.0f)`), `f` = the department's funding fraction (slot 10 `+0x28` returns a pointer to it); also set the same way in DoPostCityInit; saved |
| `0x8A0D15DA` / `0xEA0D163E` | | `AddNeighborConnection(data1 type, data2 x, data3 z)` (`0x007153C0`) / `RemoveNeighborConnection(type, x, z)` (`0x00713AB0`): they maintain the off-map cell's edge record (traffic-network.md §2.7.2–§2.7.3). Add sets the type bit, raises the trip cost and sets the pool. Remove clears the bit; once the mask is empty it also clears the pool, runs the pool sweep and frees the destination byte. Neither touches the network-changed byte (TS6) |
| `0xC53D10AA` | data1 = `0xE7E2C2DB` and data2 = `0xC9133286` | re-read tuning (§4.14) |
| `0xC2C09554` | | `UpdateTravelStrategies` (§4.1.2) |
| `0x99EF1142` / `0x99EF1143` | data3 = `0xC772BF98` | network occupant inserted / removed: the cell-count and pothole-set bookkeeping (§3.6), then the network part (traffic-network.md); on remove the cell's edge-density records are erased. Any such message (even a failed cast) sets the network-changed byte (see `0x66956810`; never read) |
| `0xE8BDA2E7` | lot (`IGZUnknown` → a **copy** of the lot with an empty trip list, lot-developer.md §3.2; §4.1.3 finds the real lot again at its corner) | data1 = 3: `DoLotDeactivated` (`0x0071E2F0`); else data2 = 3: `DoLotActivated` (`0x00723AF0`, §4.1.3) |

All handlers return true except the day handler.

### 3.2 WorkOnSimCycle (`0x00720F00`) and the next-state table

The traffic computation is one long **cycle** of phases, spread over several days. `WorkOnSimCycle(deadline)` runs
the current phase until it finishes or the deadline passes. When a phase finishes, the state moves to
`next[state]` (table `0x00AB3480`, u32[20]) and the new state's entry action runs. Before any phase, if the
intersection data are marked dirty, `UpdateIntersectionData` runs (traffic-network.md).

`next` = {16, 14, 4, 7, 5, 11, 10, 6, 0, 0, 12, 3, 13, 19, 15, 2, 17, 18, 15, 8}; state 12 is skipped (13 instead)
when the subnetwork data need no rebuild *[inferred: flag meaning, traffic-network.md]*.

| State | Phase | Spec | Entry action when entered |
|---|---|---|---|
| 0 | StartRecalc (`0x00717C90`) | §4.3 | |
| 16 | ConfirmTrips (`0x0071E1B0`) | §4.3 | traffic-source iterator reset; phase positions (saved source cell, +0x90C/+0x90E) := (0, 0); "take the next source" flag `+0x838` := 1; `+0x884` := 0 (arm `0x007210B0`, zeroes at `0x007210BE`/`0x007210C5`; §3.5, TS44, TS67); re-route context x/z (`+0x83C`/`+0x840`) untouched |
| 17 | ReRouteTrips (`0x0071D020`) | §4.4 | same |
| 18 | FullTrips (`0x0071D440`) | §4.4 | same; the per-travel-type count vector being built := 9 zeros |
| 15 | FinishTripMap (`0x0070FEF0`) | §4.7 | lot iterator reset (`+0x904` := first lot); **phase positions := (0, 0)** (`0x00721123`/`0x0072112A`); `+0x838` := 1 (arm `0x0072110B`, shared with state 1; TS67) |
| 2 | CreateMaps (`0x0071A0C0`) | §4.9 | scanning iterator reset; per-network-type statistics reset; commercial work grid := 0 |
| 4 | CreateFinalMap (`0x0070F460`) | §4.10 | scanning iterator reset; commercial traffic grid := commercial work grid (copy of the grid data) |
| 5 | ClearTripDestinations (`0x0070E4F0`) | §4.11 | scanning iterator reset |
| 11 | UpdateNeighborConnections (`0x00714BE0`) | traffic-network.md §2.7.5 (not a no-op: on every neighbour connection it rolls the per-travel-type arrival arrays `0xAA9EA81A + i` into `0xAA9EA81D + i` and the arrival counts of `0x8A0B4100`, elements 4–7 into 8–11; TS2) | |
| 3 | ReduceTransitSwitches (`0x00713750`) | §4.8 | |
| 7 | DumpEdgeDensities (`0x007137D0`) | §4.8 | |
| 6 | ReduceEdgeDensities (`0x007110B0`) | §4.8 | scanning iterator reset |
| 10 | RefreshCommuteMap (`0x0070F6E0`) | §4.11 | `freeDest[0…3]` := 0; scanning iterator reset |
| 12 | UpdateSubnetworks (`0x007183B0`) | traffic-network.md §4.11 | runs in one call |
| 13 | UpdateSubnetworkInfo (`0x0071DC50`) | traffic-network.md §4.12 | working subnetwork info reset; **scanning iterator reset** to (0, 0) (`0x00721308`–`0x0072132A`, TS64); when the phase finishes the working vector is copied to the published one (`vector::operator=` `0x0071CD90`, also called by DoPostCityInit) |
| 19 | FerryRoutes (`0x0071DC90`) | traffic-network.md §4.9.3 | ferry counters roll (last := current, current := 0; the riders are added to the ferry total used by history) and the pass restarts; posts `0x2C06BA82` when done |
| 8 | FinalCalculations (`0x0070E7D0`) | §4.12 | |
| 1 | (lot sweep) | | entry: same arm as 15 (lot iterator reset, phase positions := (0, 0), `+0x838` := 1). **No `next` entry leads to 1**, so the cycle never enters it after construction (state 0); `GetDesiredLotInsertionPoint` (slot 22), which answers only in state 1, therefore returns null in practice *[inferred]* |
| 9, 14 | (none) | | finish immediately, never entered either (14 resets a further iterator) |

The cycle is therefore *[compat]*:

`0 → 16 → 17 → 18 → 15 → 2 → 4 → 5 → 11 → 3 → 7 → 6 → 10 → (12) → 13 → 19 → 8 → 0`.

**Phase positions across the trip phases** *[compat]* (TS67). The two s16 phase positions (+0x90C, +0x90E; §6.1 row 14) are written in exactly these places: (1) as each source is taken in 16, 17 and 18 (`0x0071E276`, `0x0071D0FD`, `0x0071D51C`), and as each lot is taken in 15 (`0x0070FFE0`); (2) by the entry action of the state being **entered** when a phase finishes, set to (0, 0): entering 16, 17 or 18 (one shared arm `0x007210B0`) and entering 15 or 1 (one shared arm `0x0072110B`). The entry actions run only on the finish path (`0x00721073`: the phase function returned done, or state 12 which runs in one call); a deadline stop runs no entry action and leaves the positions as the phase left them. The exhausted-iterator end of each trip phase (e.g. FullTrips `0x0071D806`) does not write them. So: 16 → 17, 17 → 18 and 18 → 15 all leave (0, 0) until the new phase takes its first source or lot; a save or record taken at that point (e.g. state 15, no lot taken yet) holds (0, 0), not the last source's cell. The re-route context x/z (`+0x83C`/`+0x840`) is not reset by any entry action and keeps the last 16–18 source's cell. 15 → 2 does not touch the positions (they keep the last lot's cell through states 2 … 8 and 0 until 16 is entered).

The cell-scanning phases (2, 4, 5, 6, 10, …) use the adaptive slicing helper of simulator.md §5.5 (each with its
own static step state); slicing only splits the work, the results do not depend on it.

### 3.3 DoDailySimulation (`0x007215C0`): the day agent

1. Deadline := now + **4 ms** (`0x0068BF50`).
2. Day of month from the simulator (slot 6 `GetSimDate`). The last seen value (`+0x1AC`) is set to −1 by the
   constructor (`0x0071FA3B`) and written only here (`0x00721603`); it is not saved, and neither Read, Init nor
   DoPostCityInit touches it (TS50). The simulator is built with each city, so the first call after a load always
   takes the branch below, also on the day the save was taken: on day 1 the incomes are set again, and the strategy
   re-draw of that date runs a second time. If the day differs from the last seen value: store it, and
   * on day **1**: mass-transit line item 2 `SetIncome(int64 ftol(transitIncome))`, roads line item 5
     `SetIncome(int64 ftol(tollIncome))` (line item slot 15 `+0x3C`, pop 8; budget.md §8);
     `lastMonthlyIncome := r24(tollIncome + transitIncome)` (slot 18);
   * **strategy re-draw**: if `GetSimDateNumber() mod (strategyPeriod / strategyBuckets) = 0` (unsigned integer
     divisions; code defaults 365 and 128, so every 2nd day), for each lot cell registered under bucket
     `strategyBucket` (§4.1.3), in multimap order: lot := lot manager `GetLot(x, z, false)`; if present, draw
     `RandomUint32Uniform(100)` (§4.15) and store it as property `0x4A611773` (u8) on the lot. Then
     `strategyBucket := (strategyBucket + 1) mod strategyBuckets`.
3. Loop:
   * deadline passed → return **false** (not done for today; the simulator calls again within the same day,
     simulator.md §4);
   * `finished := WorkOnSimCycle(deadline)`;
   * if finished, by the **new** state `s`:
     * `s = 16`: `total := passTotal` (the number of trips counted in the last cycle, §4.6), `days := max(1, total
       div 20)`; day index and processed := 0; `toReRoute`, `toSearch` := 0 (`0x0072184B`, the only place they are
       zeroed; Read restores them, ConfirmTrips counts them and states 17 and 18 add to `toSearch`, §4.4, TS21); of
       the ten pass counters that follow
       them (all ten saved in §6.1 row 15; §10 Q4 names them, TS59) all are zeroed **except** counter 1 (the re-route counter,
       incremented by ReRouteTrips; zeroed on entering 17 instead) and counter 2 = `rerouteBucket` (kept: only
       StartRecalc advances it). Counter 3, the source counter of §4.3, is zeroed here, so it restarts at 0 every
       cycle (TS1); continue;
     * `s = 17`: `total := toReRoute`, `days := max(1, total div 18)`, reset (day index, processed and pass
       counter 1 := 0), return **true**;
     * `s = 18` (`0x007217BF`–`0x007217FD`): `total := toSearch`, `days := max(1, total div 18)`, day index and
       processed := 0, `passTotal := 0`, return **true**. Pass counter 1 is **not** zeroed here (TS35): it keeps
       the re-route count of state 17 through state 18, and a save taken then holds it;
     * `s = 2` or `s = 11`: return **true**;
     * otherwise continue;
   * if not finished: if `16 ≤ state ≤ 18`, `dayIndex < days` and `processed ≥ total · (dayIndex + 1) / days`
     (all unsigned, `0x0072191E`), then `dayIndex += 1` and return **true**; else return false. The members:
     `processed` `+0x914`, `dayIndex` `+0x918`, `total` `+0x91C`, `days` `+0x92C` (`0x00721912`–`0x0072193E`).
     In the record (§6.1 row 15) `dayIndex` is therefore the s32 right after `processed`, not the u32 after `days`
     (TS60). *(Corrected: the
     `dayIndex < days` guard was missing.)* Once `dayIndex` has reached `days` (a cycle with more trips than the
     last one, or `days = 0` after a load, §3.5 step 13) no share ends the day: the phase runs on, slice after
     slice, until it finishes.

So the trip phases are spread over `days` days (at least the share `total/days` per day), and a day also ends after
FinishTripMap (on entering 2) and after ClearTripDestinations (on entering 11).

*[compat]* The amount of work done on a given day in the original depends on wall-clock time: the per-day share is
tested only when a 4 ms slice ends without finishing the phase, so the last slice of a day overshoots the share by
a timing-dependent amount, and a fast machine may finish a whole phase inside one slice. *[choice]* FreeCity4 makes
this deterministic with a work budget per slice (§3.4). Differential tests compare per-cycle results, not per-day
progress (§11).

### 3.4 Traffic work budget *[choice]*

The original tests its 4 ms deadline at fixed **test points** (TS62):

| Phase | Test points | Step statics |
|---|---|---|
| ConfirmTrips (16, `0x0071E1B0`) | before each source is taken (`0x0071E222`), including the first of each call | `0x00B4B378`, `0x00B4B37C` |
| ReRouteTrips (17, `0x0071D020`) | (a) at the top of the source loop (`0x0071D094`): at the start of each call and after each finished source, that is after its merge and before the next source is taken; (b) before **each trip** of the list, whatever its status (`0x0071D1D9`, before the status-1 test) | `0x00B4B354`, `0x00B4B358` |
| FullTrips (18, `0x0071D440`) | (a) at the top of the source loop (`0x0071D4B3`): at the start of each call and after each finished source, after its CompleteTripSourceCalculations and before the next source is taken; (b) before each trip, whatever its status (`0x0071D607`, before the status-0 test) | `0x00B4B360`, `0x00B4B364` |
| FinishTripMap (15, `0x0070FEF0`) | before each lot is taken (`0x0070FF84`, TS63) | `0x00B4B324`, `0x00B4B328` |
| UpdateSubnetworkInfo (13, `0x00716180`) | before each cell step of the scanning iterator (TS64) | `0x00B4B33C`, `0x00B4B340` |

Every test point first adds the phase's `step` to a per-call counter and reads the clock only when the counter is a
multiple of 1024 (the adaptive helper of simulator.md §5.5; `step` starts at 1024, so at first every point reads
the clock; at the end of each call it adapts by the rule of simulator.md §5.5 with `n = counter >> 10`, inlined in
the suspended-search exits of 17 and 18). The statics are per phase and process-wide. Both kinds of point count, so
with `step` 512 the clock is read at every second point, whether that point is a trip or a source.

What a stop leaves (16–18). The source being worked keeps its cell in the phase positions (§3.5) and the "take the
next source" flag (`+0x838`, unsaved) stays 0. A stop at (b) leaves the list part-done: the next call walks the same
list again from its head, skipping the trips whose status no longer qualifies; in 17 the merge and in 18
CompleteTripSourceCalculations have not run yet. A stop at (a) after a source leaves that source finished (merge or
CompleteTripSourceCalculations done) and the next source not yet taken. Since a non-qualifying trip costs almost no
time, stops in game fall almost always right after a search; that is timing, not a rule. FreeCity4 replaces the
deadline with a count of units.

- **Unit of work**: in ConfirmTrips, one traffic source (`UpdateTrafficVolume` + `ConfirmTrips(source)`); in
  ReRouteTrips and FullTrips, one trip examined (with `SetupTrip` + `PerformPathSearch` when it is searched). A
  search never suspends (pathfinder max work time 0), so a trip is atomic. *[choice]* The original passes the
  slice's remaining milliseconds to `SetMaxWorkTime`, so its searches can suspend (§4.4, TS65).
- **Budget** `trafficUnitsPerSlice` = N (tuning value, ≥ 1). One `WorkOnSimCycle` call in states 16–18 is a
  slice: it ends unfinished after N units (where the original's deadline would fire). Step 3 of §3.3 then applies
  unchanged: the share test runs at each unfinished slice end; when the share is not reached the day continues
  with another slice (the original's "return false, called again within the same day"). A slice that finishes its
  phase and continues into the next one (state 16) starts the next phase with a fresh N *[choice: the original
  shares one deadline]*. The other phases run to completion in one call (their slicing does not change results).
- Consequently the work of one trip-phase day is the shortest run of whole slices whose end reaches the share
  `total · (dayIndex + 1) / days`: at least the share, and at most N − 1 units more.
- **Default**: to be calibrated in the original (§12). N = 1 is the minimum (the day ends exactly at the share).
  The original has no such constant (TS27): its slice is the 4 ms deadline of §3.3 step 1, tested once per unit,
  so its units per slice depend on the machine. The default N is FreeCity4's choice until the §12 counters
  calibrate it.
- **Observable effect of the budget** (for the M5 exit tests):
  - Larger N means more work per day than the share, so the trip phases finish in fewer days and the **traffic
    cycle is shorter** (in game days). The commute, congestion, air-polluting and commercial maps are then
    republished more often and converge faster after a change to the network or the zoning. Monthly sums that
    accumulate per applied route (toll and transit income, edge-density counters, accident draws per entry) grow
    with the number of cycles per month.
  - Smaller N has the opposite effect. The cycle length tends to `days(16) + days(17) + days(18)` plus the fixed
    day ends after phases 15 and 5 (§3.3).
  - Budget-sensitive statistics: cycle length, maps sampled at a fixed date, convergence time after a change, and
    monthly income and accident counts. Budget-insensitive: the result of one cycle given the same inputs (routes,
    volumes and the maps it publishes), and the draw sequence of each operation.

### 3.5 DoPostCityInit (`0x00720430`, message `0x26D31EC1`) *[compat]*

Runs once per city load (and for a new city), after every lot and occupant of the save exists and after `Read`. The
whole order (TS7):

1. – 7. **Network part**, traffic-network.md §1 steps 1–7: re-read every transit switch's rules; refresh the subway
   access points; `DoConnectionsChanged` over the whole city; **register the neighbour connections** (an Add for each
   connection of the neighbours simulator, slot 16 `GetAllConnections` in list order, then one pool sweep;
   traffic-network.md §2.7.2–§2.7.3); the **destination-map boundary pass** (each ring cell around the map whose
   `HasMapExit` is false gets destination byte 0, `freeDest` untouched; traffic-network.md §1 step 5, §2.7); clear
   the subnetwork labels and run `UpdateSubnetworks`; build the subnetwork info with no time limit and publish it.
8. If the loaded cycle state is 12 or 13, it becomes **8** (FinalCalculations): the subnetwork work was just done in
   full, so the cycle skips the rest of 12/13 and also state 19 (`0x00720859`).
9. `transitMultiplier := curve 0xCA5F7821(r24(f × 100.0f))`, `f` = the mass-transit department's funding fraction
   (as §3.1 for message `0x08DAC3E7`).
10. A rebuild of the per-network-type cell counts from the network occupants of the city, guarded by a flag that
    the constructor clears and nothing in 1.1.641 sets (`0x00720895`): **never runs**. The counts come from the save
    and from the occupant messages *[compat: do nothing]*.
11. **Resume the interrupted phase**, by the loaded cycle state:
    * 16, 17, 18: the re-route context's search state := idle; the traffic-source iterator is reset (lots, then
      neighbour connections, §4.1) and advanced until it has returned the source whose cell is the saved source cell.
      That cell is the two s16 phase positions of §6.1 row 14 (+0x90C, +0x90E; TS44). ConfirmTrips, ReRouteTrips
      and FullTrips write there, as they take a source, the cell the source iterator returns: a lot's location (lot
      slot 21) or a neighbour connection's cell (connection slot 6) (`0x0071E276`, `0x0071D0FD`, `0x0071D51C`). The
      resume compares x and z as 16-bit values (`0x00720C17`–`0x00720C25`). That source becomes the held current
      source, so the phase continues after it. If no source has that cell, the iterator is reset again and no source
      is held (the phase restarts from the first source). *[inferred: the position inside a partly processed source's
      trip list is the saved context's, not traced]*
    * 15: the lot iterator is reset; when the lot manager has a lot at the saved cell (`GetLot(x, z, false)`; the
      same two phase positions, which FinishTripMap sets to each lot's location as it takes it,
      `0x0070FFD3`–`0x0070FFE7`, read back at `0x00720B2C`–`0x00720B46`; TS45), the
      iterator is advanced past that lot; if the walk reaches the end without meeting it, the iterator restarts from
      the first lot. A stop before the first lot leaves the positions (0, 0) (entry action, §3.2, TS67); the resume then
      looks up a lot at (0, 0) like any other cell *[inferred: consequence of the two traced rules]*.
    * 19: every ferry terminal's re-route flag := 1 and the ferry pass restarts at the first terminal, its sub-step
      0 (traffic-network.md §4.9.3). **Only in this state**: a city saved in any other state keeps the loaded ferry
      links until the next state-19 pass.
    * 1 or 14 (never saved by 1.1.641, §3.2): `freeDest[0…3] := 0`, then the ClearTripDestinations work (§4.11) and
      the RefreshCommuteMap work over the whole city, each run to completion, then state := 0.
    * any other state: nothing.
12. **Strategy buckets**: the bucket multimap (§4.1.3) is cleared and rebuilt from the lots, in the lot manager's
    list order (slot 15): each lot whose property `0x691C863C` is present is registered under that value with its
    location (lot slot 21). Equal keys keep list order. (So the daily re-draw of §3.3 does reach loaded lots; the
    bucket counter and the current bucket come from the save.)
13. `days := lotCount div 18` (lot manager slot 16 `GetLotCount`, unsigned; **no** `max(1, …)`, so 0 with fewer than
    18 lots). `total`, `dayIndex` and `processed` keep their loaded values; §3.3's share test is skipped while
    `dayIndex ≥ days`.
14. `UpdateMassTransitBudget`, then `UpdateRoadsBudget` (§4.9.1): the departments' monthly costs from the loaded cell
    counts.

### 3.6 Network occupant bookkeeping: DoRoadChanged, DoRailChanged, DoSubwayChanged *[compat]*

On `0x99EF1142` (inserted, `ins` = true) and `0x99EF1143` (removed, `ins` = false) for a network occupant (§3.1),
DoMessage first does this bookkeeping (`0x00724334`–`0x0072442E`), then the network part (traffic-network.md §3). Let
`has(t)` mean that the occupant's network flag word contains bit `1 << t` (cISC4NetworkOccupant slot 22; network
types as traffic-network.md §2.2). The calls, in this order (TS4):

1. `has(6)` Avenue → `DoRoadChanged(occ, ins, 6)`.
2. `has(0)` Road → `DoRoadChanged(occ, ins, 0)`.
3. The first of OneWayRoad 10, Street 3, Highway 2, GroundHighway 12 that `has` holds for →
   `DoRoadChanged(occ, ins, t)`. At most one call comes from this step.
4. `has(1)` Rail → `DoRailChanged(occ, ins)`.
5. `has(7)` Subway, and the flag word does **not** contain bit 31 (slot 8, all-bits test of `0x80000000`) →
   `DoSubwayChanged(ins)`.

An occupant with several of these types gets several calls (Avenue and Road: steps 1 and 2). DirtRoad 11, LightRail 8
and Monorail 9 get no bookkeeping.

`cells[t]` (t = 0…12, u32, saved in §6.1 row 18) are the per-network-type counts that `UpdateRoadsBudget` and
`UpdateMassTransitBudget` (§4.9.1) price. `bump(t)` is `cells[t] += 1` on insert. On remove it is `cells[t] −= 1` only
if `cells[t] > 0`; at 0 it stays 0. At the end of every CreateMaps pass, `cells[t]` is replaced by the pass's count
of network-info records of type `t` (§4.9; about two per road or street cell, bridges excluded). So these bumps hold
only until the next pass.

**DoRoadChanged(occ, ins, t)** (`0x007155D0`):

1. Cell `(x, z)` := the city's `PositionToCell` (slot 95) of the occupant's position (occupant slot 8, its x and z).
2. `bump(t)`, then `UpdateRoadsBudget`.
3. If `t` is Highway 2 or GroundHighway 12, stop: highways are not in the pothole sets.
4. `key := x << 8 | z` (u16, as §4.12).
5. **Insert**: add `key` to the intact road set; nothing happens if it is already there. The damaged set is not
   looked at, so a key can end up in both sets. Then `roadCellCount += 1` in **every** case, including a key that
   was already present.
6. **Remove**, in this order:
   * If `key` is in the intact set, erase it.
   * Otherwise, if `key` is in the damaged set, it is repaired by removal. Erase it, then `damagedRoadCount −= 1`.
     Then decrement the published **pothole count** (the u16 at offset `0x134` of the slot-34 statistics block, read
     by the traffic advisor as `g_pothole_count`; it wraps below 0). Then post `0x69247DC8` (message-server slot 4,
     queued) with data1 = x, data2 = z, data3 = 0.
   * In **every** case, including a key in neither set: `roadCellCount −= 1` (u32, no clamp).

**DoRailChanged(occ, ins)** (`0x00715790`):

1. `bump(1)`.
2. If the occupant also has Road or Street (slot 9, any-bit test of mask 9: a level crossing), skip to step 5.
3. Cell `(x, z)` := the network occupant's own cell (slot 59). `key := x << 8 | z`.
4. Insert and remove as DoRoadChanged steps 5–6, with the rail sets, `railCellCount` and `damagedRailCount`. On
   insert the rail set is first grown to hold `count + 1` (the same rehash rule as the road insert). *[quirk]*
   Removing a damaged rail cell also decrements the **road** pothole count. It posts `0x69247DC8` with data3 = 0, like
   a road.
5. `UpdateMassTransitBudget`, also when step 2 skipped.

**DoSubwayChanged(ins)** (`0x0070EA70`): `bump(7)`, then `UpdateMassTransitBudget`. It has no cell set.

The bracket of TS4 holds for the sets. What it misses is the count, the damaged set, the highway and level-crossing
exclusions, and the per-type counts with the budget refresh.

* **The cell counts count calls, not cells.** Both cases below add 2 to the count but only one key to the set:
  * an occupant re-inserted at a cell it already holds;
  * an occupant with two counted road types (steps 1 and 2).

  The monthly pothole target is `cellCount × d` (§4.12), so a rebuild that counts distinct cells drifts from the
  original. A replacement keeps the running count, updated by these rules. A rebuild from the live occupants is
  exact only when it repeats the same per-call rules. To do so, count each occupant once per DoRoadChanged call
  (steps 1–3, excluding Highway and GroundHighway), and once per rail occupant without Road or Street.
* The pothole count is only re-set by `ApplyRoadFunding`, and only in a month when the road target changes
  (`damagedRoadCount := target`, then the pothole count := its low 16 bits; `0x00717DF2`). In the months between,
  the drops above stay, including the rail drops. `ApplyMassTransitFunding` writes no pothole count.
* *[quirk]* The cell counts have no floor: with more removes than inserts (an occupant removed twice), `roadCellCount`
  wraps from 0 to 2³² − 1. The next month's target is then `trunc(r24(4294967295.0 × d))`, and `ApplyPotholeChanges`
  caps the move at the intact set's size. Keep the wrap *[compat]*.

Worked example. 100 road calls so far, `cells[0]` = 100, `roadCellCount` = 100, `damagedRoadCount` = 3, pothole
count 3, and cell (12, 34) is damaged (key `0x0C22`). Bulldoze its road piece (flags: Road only):

1. Message `0x99EF1143`. Step 2 calls `DoRoadChanged(occ, false, 0)`.
2. `cells[0]` = 99, and the roads budget is recomputed from it.
3. Key `0x0C22` is not intact but is damaged. It is erased, `damagedRoadCount` = 2, the pothole count = 2,
   `0x69247DC8 (12, 34, 0)` is posted, and `roadCellCount` = 99.

Then place a level-crossing piece at (20, 5) with flags Road | Rail:

1. Step 2 inserts key `0x1405` into the intact road set: `cells[0]` = 100, `roadCellCount` = 100.
2. Step 4: `cells[1] += 1`. The rail sets and `railCellCount` are untouched (Road is set), and the mass-transit budget
   is recomputed.

## 4. Behaviour

### 4.1 Trip generation (traffic sources)

A **traffic source** is a lot or a neighbour connection. Its interface `cISC4TrafficSource` (vtable `0x00AB1914` on
lots, `0x00AB1FA0` on neighbour connections) has: slot 3 `GetTripDataList` (the source's `std::list<cSC4TripData>`),
slot 4 `UpdateTrafficVolume`, slot 5 `GetSourceCells(SC4Rect<u32>&)` (the lot's cell rect, or the connection's
cell).

**Traffic-source iterator** (`Reset` `0x007107A0`, `GetNext` `0x007107F0`): all lots in the lot manager's list
order (lot manager slot 15), then all neighbour connections in the neighbour simulator's list order
(`GetAllConnections`, slot 16). Each step yields the source's location and its traffic-source interface. The
interface comes from `AsTrafficSource` (lot slot 4 `0x006C2050`, a plain cast to `lot + 0x1C`, **no** AddRef;
TG16); the iterator then calls `AddRef` (slot 1) on the lot or connection object itself (`0x0071084E`,
`0x007108A7`), so the caller holds one reference per source handed out. The iterator has no other lot order and
no fallback: it never asks for lots by rectangle (TG10).

The two halves differ in when they are fixed (TS20). `Reset` runs on entering states 16, 17 and 18 and on the
DoPostCityInit resume (§3.5).

- **Connections are a snapshot.** `Reset` has the neighbour simulator clear and refill the iterator's own list
  (`0x007107C2`). The list holds counted references (`cRZAutoRefCount`), so a connection removed during the phase
  stays alive and is still visited, and one added during the phase is not. The snapshot is released only in two
  places (TG26): by the next `Reset` (the neighbour simulator's clear releases the old elements before it refills),
  and by the simulator's `Shutdown` (below). `Read`, `Init` and the destructor never touch it.
- **Lots are live.** The iterator walks the lot manager's master list through the manager's safe lot iterator
  (zones.md §8.2 slot 15; created once, at the first `Reset`, and rewound to the list's first node by every
  `Reset`). A lot removed before the iterator reaches it is skipped, because removal advances the safe iterator.
  A lot added during the phase is appended to the master list (zones.md §8.1), so the same pass visits it.

*[compat]* A shadow comparison that snapshots the lots at the phase start differs from the original on a lot
plopped or bulldozed during a long phase (FullTrips can run over several days).

#### 4.1.1 Lot: `cSC4Lot::UpdateTrafficVolume` (`0x006C27E0`)

The target per strategy `w[0…2]` (u16) starts at 0, `type` at 0.

* **Residential** (zone 1–3):
  * not in a livable state (lot slot 37) → probe (below);
  * `scale := residential simulator GetCellWorkforcePercent(x, z)` (slot 30; float);
  * `pop := GetTravelPopulation(0x1010)` (lot slot 56) with `wealth = 0`; if 0, `0x1020` with `wealth = 1`; if 0,
    `0x1030` with `wealth = 2`; all 0 → probe. `type := wealth`.
  * `n := r24(float(pop) × scale)`; `commuters := max(1, floor(n))` where floor is computed as
    `r := round(r24(n − 0.5))`, then `r + 1` if `float(r + 1) = n` (u32 arithmetic). *[compat]*
  * `pct := traffic simulator GetTravelStrategyPercentages(wealth + 1)` (slot 26, §4.1.2, a 3-float vector).
  * If `commuters < 50`: all commuters use **one** strategy. `c := 1`; for `s` = 0, 1, 2:
    `c := c + (round(r24(pct[s] × 100.0f)) & 0xFF)`; the first `s` with `strategyByte < c` gets
    `w[s] := commuters`. `strategyByte` is the lot's own byte (below). *[quirk]* Byte 0 always selects strategy 0,
    even when `pct[0] = 0`.
  * Else: `w[s] := round(r24(float(commuters) × pct[s])) & 0xFFFF` for each `s`. The parts need not add up to
    `commuters` (test 8). *[compat]*
* **Industrial** (zone 7–9): only if the lot has road access (slot 28) and slot 59 returns 0, else return without
  change. `type := 3`. Not livable → probe. `jobs := Σ GetTravelPopulation(t)` for `0x4400, 0x4300, 0x4200, 0x4100`
  (in this order); `n := r24(freightScaling × float(jobs))` (slot 27 value); `w[2] := ceil(n)` computed as
  `r := round(r24(n + 0.5))`, then `r − 1` if `float(r − 1) = n`.
* **Other zones**: return without change (no trips).

**Probe**: a single trip that only tests reachability. **Reconcile** the trip list with the targets
(`total = w[0] + w[1] + w[2]`):

* `total = 0`: if the list's first trip is a probe, keep the list unchanged; otherwise clear the list and append one
  probe `{count 1, type, strategy 0, flags 2 (probe), status 0, tripLength 0}` (destination and travel-type fields
  undefined in the original *[quirk]*; FreeCity4: 0 *[choice]*).
* otherwise: erase all probe trips; then for `s` = 0, 1, 2: walk the list, **erasing trips of another type** and
  summing `have` = the counts of strategy-`s` trips;
  * `have < w[s]` (deficit `d`): the first strategy-`s` trip with status 4 or 5 gets `count += d`; if none, append
    `{count d, type, s, flags 0, status = (any strategy-s trip exists ? 4 : 0), tripLength 0}`;
  * `have > w[s]` (surplus): reduce, in list order, first the status-4 trips of strategy `s`, then any trips of
    strategy `s` (a trip whose count would reach 0 is erased).

**Strategy byte.** Each lot has a byte, `0xFF` at construction; `cSC4Lot::Init` replaces `0xFF` by
`RandomUint32Uniform(100)` from the lot file's `cRZRandomSuper` (`0x00B49120`, foundations §6.2); it is saved with
the lot (§6.3). It is the lot member `+0x88`: UpdateTrafficVolume reads it at `0x006C2986` (traffic-source `+0x6C`),
and it is the **last byte of the lot's own record**, written after the trip list by `cSC4Lot::Write` (`0x006C0347`).
No interface slot returns it (no getter of that member is in any `cSC4Lot` vtable); outside the lot, that record
byte is the only way to read it (TS51). The copy constructor copies it (`0x006C25AA`). The value the traffic
simulator writes into property `0x4A611773` (§3.3, §4.1.3) is **never read**
by the executable (the only references are the two writers `0x007216F6`, `0x00723FD0`) *[compat: the property is
saved with the lot's properties]*.

#### 4.1.2 Travel strategy percentages

`pct[w][s]` (u8, w = 0…3 = no wealth, $, $$, $$$) comes from tuning properties `0x4953E8A3 + w` (first 3 bytes,
no length check *[quirk]*). `UpdateTravelStrategies` (`0x00711810`): re-reads them from the exemplar, then for every
ordinance that is on (ordinance simulator slots 14/15, the ID array allocated with allocator C) adds its misc
property `0x8A612FEE` (s32[9], read without a size check *[quirk]*; ordinance.md §6.3) element `3(w−1) + s` to
`pct[w][s]` for w = 1…3, clamping each sum to 0…100. Runs at Init and on `OrdinanceUpdated`. *[quirk]* When the
exemplar is missing, the re-read is skipped and the ordinance offsets accumulate on every update.

Short or missing data (TS28):

- **Strategy property.** A missing `0x4953E8A3 + w` leaves `pct[w]` unchanged. A present one is read as raw bytes
  (no type check), and its first three bytes are copied whatever its length (`0x00711899`). With fewer than 3 bytes
  the original reads past the copied data. *[choice]* The missing ones keep their value.
- **Ordinance offsets.** An ordinance with no `0x8A612FEE` (or no property holder) adds nothing. With fewer than 9
  entries the original reads past the array (`0x007119A0`). *[choice]* Entries past the end count as 0.
- **Wealth above 3.** `GetTravelStrategyPercentages(w)` reads the three bytes at `pct + 3w` with no bound
  (`0x00718BD6`). For `w = 4` that is the intersection-effect count byte (§4.14) and two padding bytes; higher `w`
  reads further members. *[choice]* Return three zeros for `w > 3`.

`GetTravelStrategyPercentages(w, vector<float>* out)` (slot 26) resizes `*out` to 3 (allocator B) and writes
`out[s] := r24(float(pct[w][s]) × 0.01f)` (dword constant).

#### 4.1.3 Lot activation and deactivation

`DoLotActivated` (`0x00723AF0`):
- building := lot slot 48 (none → return);
- lot := the lot manager's `GetLot` at the lot rectangle's corner (slot 6, bool false), which may be null;
- the transit-switch and ferry-terminal steps from the building's properties are traffic-network.md §4.7.1.

If the building's `0xE90E25A1` has a byte size that is not a multiple of 4, DoLotActivated returns inside the switch
step, and **none of the steps below runs**, not even the draw (TN1, traffic-network.md §4.7.1 step 1). Otherwise
(the property is missing or well formed), in order:

1. Only if the lot was found: `b := strategyBucket counter`; counter := `(counter + 1) mod strategyBuckets`; lot
   property `0x691C863C` := `b`; the lot cell is added to the bucket multimap under `b`.
2. Only if the lot was found: `RandomUint32Uniform(100)` (`0x00723FBE`) → lot property `0x4A611773` (u8).
3. Budget items. Budget simulator slot 51 is asked with the building's property holder for `0xCA550301` first, and
   for `0x0C3BF549` only when that fails. If one succeeds and the lot was found, slots 58 and 55 follow. Only on the
   `0xCA550301` path is the int64 at `+0x10` of the record that slot 51 filled added to the total `E` (`+0x778`,
   saved, §6).
4. Unless the cycle is in state 10, and whether or not the lot was found: `bits := 0`; for `k` = 0…3, if
   `GetMaxTripCapacity(holder of the message's lot, k) > 0`, set bit `k` and `freeDest[k] += 1`. Then **store**
   `bits` into the destination byte of every cell of the lot rectangle (`0x00724167`). *(Corrected: the byte is
   replaced, not ORed into.)*

`DoLotDeactivated(lot)` (`0x0071E2F0`, TS23) removes what activation added. `holder` is the lot's own property
holder (lot slot 3). A null lot does nothing. In order:

1. **Bucket entry.** If `holder` has `0x691C863C` (read as u32) = `b`: among the multimap entries under `b`, the
   first whose cell equals the lot's location (lot slot 21) is erased. Only that one entry goes.
2. **Transit switch** (traffic-network.md §4.7.5 step 1): the switch at the NW cell of the lot's bounding rect
   (lot slot 14) is released and erased, the ID and mask grids over the rect are zeroed, and
   `DoConnectionsChanged(x1 − 1, z1 − 1, x2 + 1, z2 + 1)` runs.
3. **Budget items.** Budget simulator slot 51 `GetBudgetItemForPurpose(holder, 0xCA550301, item)`. If found:
   slot 56 `RemoveBudgetItemFromBudget(holder, item.id)` and `E −= item.cost` (the int64 at record `+0x10`).
   Then, **independently** (unlike activation, which asks the second purpose only when the first fails):
   `GetBudgetItemForPurpose(holder, 0x0C3BF549, item)`, and if found `RemoveBudgetItemFromBudget(holder,
   item.id)`, with no change to `E`. The lookup is the city budget simulator's slot 51 (`0x00691880`) with the
   lot's own property holder as a `cISCPropertyHolder*` (`0x0071E4BD`–`0x0071E4CA`); it only fills `item` and
   changes nothing, so a reader may call it to learn `item.cost` without removing the item (TS52). It copies a whole
   `0x20`-byte `BudgetItem` into the caller's record (`rep movsd`, 8 dwords, `0x006918EE`): department `+0x00`,
   line `+0x04`, purpose `+0x08`, cost s64 `+0x10`, index `+0x18`, local funding f32 `+0x1C`. A smaller buffer is
   overrun.
4. **Free destinations.** Unless the cycle is in state 10: for `k` = 0…3, if bit `k` of the destination byte at the
   rect's NW cell `(x1, z1)` is set, `freeDest[k] −= 1` (`0x0071E570`). Only that one cell is read, so each lot is
   counted once, as RefreshCommuteMap counts it.
5. **Ferry terminal** (traffic-network.md §4.9.4): the first terminal whose lot cell is in the rect is removed.
6. **Destination bytes** over the whole rect := 0 (`0x0071E7B7`), bit 3 included.

Example: a 2 × 2 commercial lot at (10, 20)–(11, 21) with destination byte 3 at every cell (bits 0 and 1) is
deactivated in state 18. `freeDest[0]` and `freeDest[1]` each drop by 1 and the four bytes become 0. Its bucket
entry and its `0xCA550301` budget item go, and `E` drops by the item's cost.

#### 4.1.4 Neighbour connection: `UpdateTrafficVolume` (`0x006D4C10`)

Runs only when the connection's "volume dirty" flag is set (then cleared). Holder := the property holder of the
matching connection on the other side (neighbour simulator slot 13, `FindConnectionOnOtherSide(conn)`). If there is
none, the whole trip list is cleared instead (`0x006D5030`) and the function returns (neighbors §15.4, NG19). For trip type `w` = 0…2 and index `s` = 0…8: `target := ` element `s` of the
u32 vector property `0xAA9EA81D + w` (padded with zeros to 9; set by the neighbour simulation, neighbors.md);
`have :=` the sum of counts of the list's trips with type `w` and strategy `s` (trips of type > 2 are ignored and
never erased).

* `have < target`: `d := (target − have) & 0xFFFF` *[quirk: 16-bit]*; the first type-`w`, strategy-`s` trip with
  status 5 absorbs `d`; else append `{count d, type w, strategy s, flags 4 (forced start type), status 0,
  tripLength 0}` (destination and travel-type fields uninitialised in the original *[quirk]*; FreeCity4: 0
  *[choice]*).
* `have > target`: remove the excess from status-5 trips of `(w, s)` first, then from any trips of `(w, s)`, in list
  order (erasing trips that reach 0).

So for neighbour connections the strategy field is a **travel type** and the trip must start in it (§4.4).

### 4.2 Capacity of destinations

Each lot and neighbour pool carries property `0x8A0B4100`, an array of 12 u16 (promoted to u32 elements once a value
reaches `0xFFFF`): elements 0–3 = **max trip capacity** per destination type `k`, 4–7 = trips **arrived this
cycle**, 8–11 = trips **arrived last cycle**.

**Element store** (`SetTripCapacity`, `0x0070DE70`; TS18). Every write goes through it: slot 19, the arrived count
in ApplyTrafficToDestination, the roll in ClearTripDestinations (§4.11) and the neighbour-capacity code.

1. If the holder has no `0x8A0B4100`, it is created as 12 u16 zeros.
2. If the property is a u16 array (variant type `0x8004`) and `value < 0xFFFF`, element `i := value` as a u16.
3. If it is a u16 array and `value ≥ 0xFFFF` (so `0xFFFF` itself promotes), the 12 elements are copied
   zero-extended into a 12-element u32 array, which replaces the data (`0x0070DF30`). Then the u32 element
   `i := value`.
4. Any other type: the u32 element `i := value`.

A promoted property stays u32: no write turns it back. The type is saved with the holder's properties, so a save
keeps it *[compat]*. The readers (slot 20, slot 21, `0x0070DFF0`) take the u16 element when the type is `0x8004`,
else the u32 element. The variant type is the only check: neither side checks the element count, and "any other
type" is accessed as u32 data. A short or mistyped property (only from damaged or custom data) is read and written
past its end. *[choice]* Treat a `0x8A0B4100` that is not a 12-element u16 or u32 array as missing: read 0, and
replace it with 12 u16 zeros before a write. Example: a pool holds `arrived[0] = 65530` (u16) and 10 commuters
arrive. `65540 ≥ 0xFFFF`, so all twelve values become u32 with their old values, and element 4 := 65540.

| Function | Result |
|---|---|
| `GetMaxTripCapacity(holder, k)` (slot 20) | element `k`, 0 if the property is missing |
| arrived this cycle (`0x0070DFF0`, non-virtual; the pathfinder's "used capacity") | element `4 + k` |
| `GetTrafficArrived(holder, k)` (slot 21) | element `8 + k` |
| `SetMaxTripCapacity(holder, k, value)` (slot 19) | creates the property if missing; sets element `k` |

The capacities are set by the building development and civic code from jobs and freight acceptance (callers of
slot 19; building-development.md) *[inferred: caller list not part of this spec]*.

`CapacityStillAvailable(trip)` (`0x007136B0`): holder := the lot at the trip's destination cell (in city) or the
neighbour pool holder of that off-map cell (`GetNeighborPoolPropertyHolder(−1, x, z)`, `0x007114E0`); result :=
`max(k) − arrivedThisCycle(k) ≥ count` (unsigned subtraction) with `k = type`; with no holder `0 ≥ count`.

`ApplyTrafficToDestination(count, x, z, lastTravelType, type)` (`0x00714740`): holder as above, except that an
off-map cell asks `GetNeighborPoolPropertyHolder(lastTravelType, x, z)` (traffic-network.md §2.7.1), which is null
when no connection type at the cell carries that travel type and the cell has no ferry connection. For an off-map
cell and `type < 3` it also adds `count` to element `lastTravelType` of the u32[9] property `0xAA9EA81A + type`
(nine zeros if missing) on the **neighbour connection's own** holder, the connection that
`FindNeighborConnection(lastTravelType, x, z)` finds (`0x007147C9`). This is not the pool. If there is no such
connection, nothing is added. *(Corrected: an earlier text said the pool's property.)* Then
`a := arrivedThisCycle(type) + count` (u32, wrapping) is stored on the holder (element `4 + type`). For an off-map cell
where `FindNeighborConnection` found a connection, `a` is **also stored** into element `4 + type` of the
connection's own holder's `0x8A0B4100`, whatever `type` (`0x007148C1`). That write is the pool's new total, not
the connection's own count plus `count`, and it creates the property on the connection if needed. If `a ≥ max`
(the holder's `GetMaxTripCapacity(type)`, read before the store), clear destination bit `type` over the destination
lot's rect (for an off-map cell, the cell itself in the bordered destination map) and decrement `freeDest[type]`
*[quirk: decremented on every such call, also when the bit was already clear]*.

### 4.3 StartRecalc and ConfirmTrips (states 0 and 16)

**StartRecalc** (§4.12) advances `rerouteBucket` (0…3) and resets the cycle accumulators. It then recomputes the
neighbour pool capacities and sets the destination bits of the off-map edge cells (traffic-network.md §2.7.4,
`0x00714E60`, called at the end of `0x00717C90`).

**ConfirmTrips** walks all traffic sources (§4.1). For each source: `source.UpdateTrafficVolume()` (§4.1.1 /
§4.1.4), then `ConfirmTrips(source)` (`0x0071D920`):

* `sourceCounter` (pass counter 3) `+= 1`; `rerouteSource := ((sourceCounter & 3) = rerouteBucket)` — one source in
  four takes part in this cycle's re-balancing. `sourceCounter` is 0 on entering state 16 (§3.3), so the first
  source has counter 1.
* Before anything else, pass counter 7 += the number of status-0 trips in the list (`0x0071D95D`, every source).
* If `rerouteSource`: **merge** (`0x0071D983`–`0x0071DA18`, TS55). For each trip `A` that has status 3 when the walk
  reaches it, every later trip `B` of the same type and strategy, **whatever `B`'s status**, is merged into it:
  `A.count += B.count`, `A.status := 0`, `B` is erased (its two route vectors and its node freed). `A`'s status
  becomes 0 **only when it absorbed at least one trip** (the store is inside the match, `0x0071D9B6`); a routed trip
  with no later partner keeps status 3 and goes on to the capacity test below. Each `A` that ends the scan with
  status 0 adds 1 to pass counter 5 (`0x0071DA0D`). `A` keeps its own route vectors, destination and trip length.
* Then for each trip (`processed += 1`):
  * **status 3** (routed):
    * if `CapacityStillAvailable`: status := 1 (re-route to the known destination), but 0 for freight. If not
      `rerouteSource`, the old route is **confirmed** without a search: `ConfirmRoute(route)` (`0x0070E320`; below);
      on success:
      * freight: `ApplyRoute(route, mode 0)` (§4.5) unless probe; status := 3;
      * reversible (flag 0): `ApplyRoute(route, mode 2)` unless probe; status := 3;
      * otherwise `ConfirmRoute(returnRoute)` must succeed too; then `ApplyRoute(route, mode 0)` and
        `ApplyRoute(returnRoute, mode 1)` unless probe; status := 3. When it fails the status stays 1 and pass
        counter 6 += 1 (`0x0071DBA0`).

      Each status := 3 here adds 1 to pass counter 0 (`0x0071DBF5`).
    * else (destination full): status := 0, pass counter 4 += 1 (`0x0071DC01`).
  * **status 5** (failed last time): status := 0, pass counter 8 += 1 (`0x0071DC11`).
  * afterwards status 1 counts towards `toReRoute`, status 0 towards `toSearch`.

**ConfirmRoute(route)** (`0x0070E320`, one argument) *[compat]* (TS14). It replays the compressed route
(pathfinder.md §4.15) with the route reader and tests each cell against the cell connection records
(traffic-network.md §2.1). The reader turns the route into one record per visited cell, in route order:

* the cell (x, z) and its travel type `t` (the type of the header or token that brought the route there);
* the next type `t'` (the type of the following run or jump; `t' = t` at the last cell);
* the **exit** side `x`: the direction of the following unit step; for the following **jump** to cell `(qx, qz)`,
  with `dx = qx − cx` and `dz = qz − cz`: when both are non-zero, the one smaller in absolute value is set to 0
  (`dx` when the magnitudes are equal); then `|dx| ≥ 2` → `+x` or `−x` by its sign; `dx = ±1` (so `dz = 0`) →
  **none**, and the cell's own entry side also becomes none; `dx = 0` → `dz ≤ −2` gives `−z` and **every other `dz`
  gives `+z`**, including `dz = −1` and `dz = 0` *[quirk]* (`0x006D65E0`–`0x006D666A`). The last cell's exit is
  none.
* the **entry** side `e`: none at the first cell; after a unit step or a jump with an exit, the side facing back
  (`(x + 2) mod 4`); after a jump whose exit is none, the entry side the cell before the jump had on arrival carries
  over unchanged (TS36, `0x006D666F`). The reader blanks that side in the record of the cell before the jump but
  keeps it for the next record, so it is none only when the cell before the jump is the first cell (the reader
  starts with none, `0x006D64FD`). Example: a route steps +x into (5, 3), entry `−x`, and then jumps to (6, 3). The
  record of (5, 3) has entry none and exit none; the record of (6, 3) has entry `−x`.

The test, with `strike := (strike state = 1)` (§4.12) read once at the start:

1. An empty route: **false**.
2. **First cell**, if inside the city and its exit is not none: when `t = t'`, some entry side `e' ∈ 0…3` must have
   bit `x + 4e'` set in `conn[t]` (any entry may leave through `x`); when `t ≠ t'`, nibble `x` of `switch[t]` must
   equal `t'`. No strike test and no wormhole test here.
3. **Every later cell**, if inside the city (cells off the map are skipped) and its exit is not none (the last cell
   and a cell before a jump with no exit are skipped):
   * `t = t'`: pass when bit `x + 4e` of `conn[t]` is set (never when `e` is none). Otherwise pass when the cell's
     wormhole byte has the tunnel bit (2); otherwise pass when it has the ferry bit (4) **and** there is no strike;
     otherwise **false**.
   * `t ≠ t'` (a type change): during a strike **false**; otherwise nibble `x` of `switch[t]` must equal `t'`, else
     **false**.
4. **true**.

So a strike fails every confirmed route that changes travel type after its first cell or that crosses a ferry
terminal's water link, whatever the types; a route that keeps one type (a car route, or a neighbour trip forced to
start on the bus or a train and staying on it) still confirms. The last cell is never tested. *[choice]* A type
byte above 8 (only in a damaged save) fails the test; the original reads unrelated words of the record.

**Example** (§11 test 10's route `01 02 03 41 02 0A`): cells (2,3) type 1, exit `+x`, entry none; (3,3) type 1,
entry `−x` side 0, exit `+x` side 2; (4,3), exit none. ConfirmRoute is true when (2,3)'s `conn[1]` has bit 2, 6, 10
or 14 and (3,3)'s `conn[1]` has bit `2 + 4·0 = 2`; (4,3) is not looked at. The same with a walk-to-bus change at
(3,3) (`t = 0`, `t' = 2`): during a strike false; otherwise nibble 2 of (3,3)'s `switch[0]` must be 2.

### 4.4 Path searches: ReRouteTrips and FullTrips (states 17 and 18)

**ReRouteTrips** (`0x0071D020`) takes, for each source, the status-1 trips: if the destination has no capacity left
(`CapacityStillAvailable` false) the trip goes back to status 0; otherwise `SetupTrip` then `PerformPathSearch`;
result found → status 3 and `tripLength := pf.GetTripLength()`; not found → status 5. Then, for the same source, a
**merge** (`0x0071D2B2`–`0x0071D344`, TS57): for each trip `A` with status 0, every later trip `B` of the same type
and strategy is merged into it (`A.count += B.count`, `B` erased with its route vectors), **whatever `B`'s status**
(0, 1, 3, 4 or 5). The code has a test meant to skip routed trips, but it reads `A`'s status (`0x0071D2D7`), which is
always 0 there, so it never skips *[quirk]*. `A` stays status 0; no counter moves. Example: a lot with `[st0 ×2,
st3 ×1]`, all of type 0 and strategy 1, after its status-1 trips: the first trip absorbs both others and the list is
`[st0 ×3]`. A network change (`DoConnectionsChanged`) aborts a suspended search: both pathfinders `Reset` and the
context restarts.
Counters (TS21): a trip sent back to status 0 for lack of capacity adds 1 to `toSearch` and to pass counter 4
(`0x0071D20D`), not to `processed`. So the next state's `total` (§3.3) includes it. Each search that is not
suspended adds 1 to `processed`; a found one also adds 1 to pass counter 1, the re-route counter.

**FullTrips** (`0x0071D440`) takes, for each source, the status-0 trips: `SetupTrip`, `PerformPathSearch`;

* found: status 3, `tripLength := pf.GetTripLength()`, `arrived := pf.GetTrafficArrived()`; if `arrived < count` the
  trip is **split**: a copy with `count − arrived` and status 0 is inserted right after it (allocator B node) and the
  original keeps `arrived`; the copy is searched next with the *continue* flag (pathfinder flag 4) so the search
  resumes from the previous open set (`continueSearch := true`); otherwise `continueSearch := false`;
* not found: status 5, `continueSearch := false`.

**Continuation lifetime (TS68, compat).** `continueSearch` is a simulator-owned byte (`+0x881`, context
`+0x83C + 0x45`), not a local of WorkOnFullTrips. A split sets it true (`0x0071D785`); a completed search
without a split, including failure, clears it (`0x0071D78E`–`0x0071D792`). It survives an unfinished call,
another slice in the same delivery, and a day/delivery boundary while the same source is held: neither a
deadline exit (`0x0071D7AC`–`0x0071D803`) nor the suspended-search exit (`0x0071D870`) clears it. Thus a
deadline between a split and its remainder leaves it true; the next call walks the held list from its head,
skips the now-status-3 original, and searches the status-0 remainder with flag 4. Taking a new source clears
the byte in both 17 and 18 (`0x0071D152`, `0x0071D571`), even if the new source is taken in the same call.
Entering a trip phase resets the separate search-state word `+0x884` and arms taking a source, but does not
itself clear `+0x881` (`0x007210B0`–`0x007210D3`); the first source take supplies that reset. A network
change through DoConnectionsChanged clears continuation immediately (`0x0071A900`, again `0x0071B902`)
and marks the context for reset (`+0x888 := 1`, `0x0071B909`): on the next source-loop visit both
pathfinders reset and search state becomes idle (`0x0071D578`–`0x0071D5A2`; same rule in 17). If the
remainder's own search suspends, continuation remains true and `+0x884` holds outbound/return state 1/2;
the next call skips SetupTrip while that state is nonzero (`0x0071D622`–`0x0071D63B`), refreshes both
work-time limits, and resumes that search. It does not reissue flag 4 or restart the outbound search when
the return leg is suspended. When SetupTrip does run for an idle commuter search, its flag-4 decision
reads this stored byte (`0x0070EDE4`–`0x0070EDF7`). This confirms retaining the bool across slices; it
does not establish the cause of the normal-speed density mismatch reported with TS68.

Counters (TS21): each search that is not suspended adds 1 to `processed`. Each split adds 1 to `toSearch`, to the
phase's `total` and to pass counter 9 (`0x0071D75A`). The day shares of the running phase (§3.3) therefore stretch
with the splits: `total` was `toSearch` on entering 18 and grows as copies are made.

After each source's list: `CompleteTripSourceCalculations(source)` (§4.6). At the end of the phase (the source
iterator has no more sources, `0x0071D806`–`0x0071D84E`) the two per-travel-type count vectors are **swapped**, not
copied (TS56): the three words (begin, end, capacity) of the building vector (`+0x430`) and the published vector
(`+0x424`) are exchanged. The published vector (read by DoMonthlySimulation's history, §4.12) then holds this pass's
counts and the building vector the previous published counts, until the next entry into 18 refills it with 9 zeros
(`0x007210E6`). Example: published `[…, 132, …]` and this pass built `[…, 130, …]`; right after the end the record
(§6.1 row 19, pairs (published, building)) holds `(130, 132)`.

**SetupTrip(trip)** (`0x0070EC40`) configures **pf** (pathfinder.md §4.2 for the meaning of every setting):

1. `SetTrafficDensity(count)`.
2. Status 1 (re-route): `SetTripDestinationFilter(3)`, `SetEndRect(` the destination lot's bounding rect if the
   destination cell is in the city and holds a lot, else the single cell `(destX, destZ))`.
   Otherwise: `SetTripDestinationFilter(0)`, `ClearAllDestinationFlags`, `SetDestinationFlag(probe and type 0 ? 7 :
   1 << type)`.
3. `SetMaxWorkTime(remaining slice ms)` on pf and pf2; `SetStartRect(source.GetSourceCells())`.
4. `ClearAllFlags`, `ClearAllTravelTypeFlags`, `SetStartingCostArray(null)` (= the simulator's default array),
   `SetMaxTripLength(commuteMaxTime)`.
5. Commuters (type ≤ 2): if `continueSearch`, `SetFlag(4)`. Forced start (flags bit 2):
   `SetTravelTypeFlag(0x1CF, false)` then `SetTravelTypeFlag(1 << strategy, true)`. Otherwise
   `SetTravelTypeFlag(0x1CF, false)`, `SetTravelTypeFlag(0x3, true)` (may start walking or by car), and
   `SetStartingCostArray` by strategy: 0 → mass-transit array (`0xEA8C3CDB`), 1 → car array (`0xCAD64136`), 2 →
   neutral array (`0xA92356BC`). `0x1CF` = walk, car, bus, passenger train, subway, el-train, monorail.
6. Freight (type 3): `SetFlag(1)` (no budget), `SetTravelTypeFlag(0x30, true)` (truck, freight train).

**PerformPathSearch(trip)** (`0x0071C6C0`):

1. Outbound: `r := pf.FindPath()` (unless resuming the return leg). 2 → suspended (resume later); 0 → not found.
2. Found: `(destX, destZ) := pf.GetEndCell()`.
   * Freight: `pf.GetCompressedRoute(route, travelTypesUsed)`; `ApplyRoute(route, pf.arrived, mode 0)` unless probe;
     done.
   * `pf.WasTripReversible()`: flags |= 1; route as above; `ApplyRoute(…, mode 2)` unless probe; done.
   * Otherwise flags &= ~1 and the **return leg** is searched with **pf2**, configured by writing its fields
     directly *[compat: the pathfinder's memory layout is shared with the simulator; pathfinder.md §9]*
     (`0x0071C858`–`0x0071CA03`, TS17):
     - **start rect**, from pf's end cell `(x, z)`. In the city: the bounding rect (lot slot 14) of
       `GetLot(x, z, false)`. If there is no lot there, the start rect is **not written** and keeps pf2's previous
       one *[quirk]*. Off the map: if the travel type of pf's result node is 1 (car) or 2 (bus), and the cell's
       edge record (`GetTrafficNeighborConnection`, traffic-network.md §2.7.1) has a Highway, Avenue or
       GroundHighway connection (mask `& 0x1108`), then `FindHighwayReturnTile(x, z, back = false)` (below). The
       start rect is then the single cell `(x, z)`.
     - **end rect**: pf's start rect when its `(x0, z0)` corner is in the city. Otherwise, if the edge record at
       `(x0, z0)` has mask `& 0x1108` (whatever the travel type), the single cell
       `FindHighwayReturnTile(x0, z0, back = true)`; else pf's start rect.
     - flags := 1 (no budget), traffic density := pf's traffic arrived, filter := 3, allowed types := pf's allowed
       types, starting types := `1 << ` the travel type of pf's result node, starting cost array := pf's; if pf
       recorded a park-and-ride cell, it is copied and flag 8 is set.
     *[quirk]* pf2's max trip length is never set: it is the shared max-length/trip-length float (pathfinder.md §4.3),
     so it holds pf2's previous trip length; with flag 1 it is not used as a budget.

     **FindHighwayReturnTile(x, z, back)** (`0x0070DD30`, its only callers are the two above) moves an off-map cell
     one step along its map edge. So a car or bus that left by a divided road (highway, avenue, ground highway)
     comes back on the other carriageway. First `right := true`. The language manager's utility for the current
     language (`cIGZLanguageManager` slot 38 `GetLanguageUtility(0)`, then its slot 25
     `GetLanguageRoadDrivingSide`) returns 0 (drive on the left) for UK English (language 2) and Japanese (15) and 1
     for every other language. `right := (that value = 1)`; with no utility it stays true. If `back`,
     `right := not right`. Then the first case that matches applies (`X`, `Z` the city size in cells):

     | Cell | `right` | not `right` |
     |---|---|---|
     | `x < 0` | `z + 1` | `z − 1` |
     | `x > X − 1` | `z − 1` | `z + 1` |
     | `z < 0` | `x − 1` | `x + 1` |
     | `z > Z − 1` | `x + 1` | `x − 1` |

     An in-city cell is unchanged. The new cell is not checked: it may have no connection, or be a map corner when
     the connection is the last cell of its edge. Example, English: a commuter drives out by a highway to `(−1, 40)`.
     The return leg starts at `(−1, 41)` and searches back to its source lot. A trip from a neighbour connection at
     `(−1, 40)` by highway has its return leg end at `(−1, 39)`. With UK English the shifts are reversed.

     *[choice]* For the no-lot case, keep pf2's previous start rect, as the original does. It needs an in-city end
     cell without a lot, which a found destination does not normally have.
   * `pf2.FindPath()`: 2 → suspended; 0 → not found (the whole trip fails). Found: both compressed routes are
     fetched (`travelTypesUsed |=` pf2's mask), `ApplyRoute(route, mode 0)`, `ApplyRoute(returnRoute, mode 1)` unless
     probe.

**What a search writes into the trip, and when** (TS58, TS65). The search state is the context word `+0x884`
(0 idle, 1 outbound suspended, 2 return leg suspended; `0x0071C702`, `0x0071CA0A`, `0x0071CA2C`):

* Outbound suspended (state 1): nothing in the trip changes.
* Outbound found: `destX, destZ := pf.GetEndCell()` at once (`0x0071C752`), before anything else. On the two-leg
  path flags bit 0 is cleared (`0x0071C858`) and the return leg starts; if that leg **suspends** (state 2) the trip
  already holds the new destination and the cleared bit 0, and nothing else: `route`, `returnRoute`, `tripLength`,
  `travelTypesUsed`, `count` and the status are unchanged, and `processed` is not counted. If the return leg then
  fails, the trip becomes status 5 with that destination kept.
* Freight and reversible results fetch only the outbound route (pathfinder slot 32 `+0x80`, into `route` and
  `travelTypesUsed`). **`returnRoute` is not touched**: it keeps whatever it held, from an earlier two-leg result of
  the same record, from the trip a split copy was made from (the copy is built with both vectors, `0x0071D6DE`,
  `0x0071D6EA`), or from the save. Nothing reads it while the reversible bit is set (ConfirmTrips and ApplyRoute use
  mode 2 on `route` only), but it is saved with the trip and the query tool's route display may read it.
* A resumed search (state ≠ 0 when FullTrips or ReRouteTrips reaches the trip again) skips `SetupTrip` and goes on
  with the suspended leg. Resumption takes the first qualifying trip of the list; the suspended trip is that one,
  because every trip before it was already handled. A network change (`+0x888`) resets both pathfinders and the
  state to 0 first, so the trip is set up again; the destination written by the abandoned search stays.

Example (FullTrips, a split): trip T (count 40, dest (18, 40)) arrives 25, so a copy C (count 15, status 0, dest
(18, 40) copied from T) follows it and is searched with the continue flag. C's outbound leg finds (28, 37) and its
return leg suspends at the deadline: C is still status 0 with dest (28, 37). FreeCity4 never suspends (§3.4), so its
C keeps (18, 40) until its search completes.

**SetupPathFinderForLot(pf, lot)** (slot 7, `0x00711610`, returns a bool in AL) configures a caller's pathfinder
from a lot's trip list, for outside callers (TS10):

1. If `pf` or `lot` is null: return false.
2. `src := lot.AsTrafficSource()` (lot slot 4). If `src.GetTripDataList()` (slot 3) is empty, call
   `src.UpdateTrafficVolume()` (slot 4, §4.1.1) **once**, which generates the lot's trips as a side effect, and look
   again. If it is still empty, return false.
3. `sum[s]` (u32, s = 0…2) := the `count`s of the list's trips by `strategy`. The strategy is not range-checked
   (TS47, `0x00711680`–`0x00711687`): a strategy above 2, only in a damaged save, adds its count past the 12-byte
   heap array of sums. *[choice]* Ignore such trips. Then pick the dominant strategy:
   `best := 0`; if `sum[1] > sum[0]` then `best := 1`; if `sum[2] > sum[best]` then `best := 2`. The comparisons are
   unsigned and strict, so a tie keeps the lower index. Also `type :=` the **first** trip's `type`.
4. `SetStartRect(src.GetSourceCells())`, `ClearAllFlags`, `ClearAllTravelTypeFlags`, `SetStartingCostArray(null)`,
   `SetMaxTripLength(commuteMaxTime)`.
5. `SetTripDestinationFilter(0)`, `ClearAllDestinationFlags`, `SetDestinationFlag(1 << type)`. There is no probe
   case here: SetupTrip's 7 for a type-0 probe does not occur.
6. By `type`:
   * **Commuter** (`type ≤ 2`): `SetTravelTypeFlag(0x1CF, false)`, `SetTravelTypeFlag(0x3, true)`, and
     `SetStartingCostArray` of `best`, with SetupTrip's mapping (0 mass transit, 1 car, 2 neutral). Return true.
   * **Freight** (`type = 3`): `SetFlag(1)`, `SetTravelTypeFlag(0x30, true)`. Return true.
   * **Anything else**: return false. Every exit path frees the strategy sums.

Compared with SetupTrip, it never sets the traffic density, the work-time limit, flag 4 (continue search), a forced
start type or a re-route end rect; those keep whatever the caller's pathfinder held. It makes no search and changes
no trip. *[compat]* Two side effects remain: the trip generation of step 2 on a lot with no trips, and the use of the
first trip's type.

### 4.5 Applying a route: ApplyRoute (`0x00719910`)

`ApplyRoute(route, count, sourceRect, destX, destZ, mode, type)` replays a compressed route (reader `0x006D64B0` /
`0x006D6510`, pathfinder.md §4.15) and adds its traffic. `mode`: 0 = outbound, 1 = return leg, 2 = both directions
(reversible trip).

* `N := float(count)`; `Nc := N`, or with the Carpool ordinance (`0xA2BF1DDC`) on,
  `Nc := float32(ceil(double(r24(N × 0.8f))))`. Car (travel type 1) uses `Nc`, all others `N`. `Q` = that value,
  `Qi := round(Q)`.
* The **start rect** `R` is `sourceRect` in modes 0 and 2. In mode 1 it is the bounding rect (lot slot 14) of the lot
  at `(destX, destZ)` when that cell is in the city and holds a lot (`GetLot(x, z, false)`), else the single cell
  `(destX, destZ)`. `I` (income) and `fees` start at 0.
* **Ferry leg from a neighbour, at the first cell** (TS16, `0x00719A4E`–`0x00719B29`). Let `(x, z)` be the first
  route cell. If exactly one of "`x = x0 − 1` or `x = x1 + 1`" and "`z = z0 − 1` or `z = z1 + 1`" holds (`R` =
  `(x0, z0, x1, z1)`; the other coordinate is not compared with the rect), `(x, z)` is the arrival cell of a terminal
  `T` (traffic-network.md §2.6), and `T` has a neighbour link whose off-map cell or edge cell is `(x0, z0)`: that
  link's counters are bumped by mode as in step 4 below, and `I := r24(Q × ferryFare)`, with `Q` of the first cell's
  travel type. This is the crossing from a neighbour city into the city, whose departure cell is not on the route.
  It is a store, not an add, which is harmless since `I` is still 0. `T`'s riders are not counted.
* For each route cell `(x, z, t)`, steps 1–4 only when the cell is in the city and step 5 for every cell, off-map cells
  included (`0x00719C06`):
  1. **Volume**: if the cell has network info for transit network `network(t)` (pathfinder.md §4.1) and
     `generatesTraffic[t]`: `vol := r24(vol + Q)`; mode 2 adds a second time, `vol := r24(vol + Q)`.
  2. **Transit switch** (`0x00719C8A`–`0x00719CDB`, TS25): `id :=` the cell's switch ID (switch map); `last` starts
     at 0 for each route. If `id ≠ last`: when `id ≠ 0`, and not both this cell's type and the **next** record's type
     are subway (6), `usage := r24(usage + Q)` (once, also in mode 2) and `fees := r24(fees + r24(Q × switchFee))`.
     In every case where `id ≠ last`, `last := id`, also when `id = 0` and in the subway case. So `last` follows every
     in-city cell, and leaving a station's lot and coming back counts it again. Off-map cells do not touch it. At the
     last cell the "next" record is the last record itself, so a subway last cell inside a switch is never counted.
     The switch is looked up with insert-if-missing (as in §4.16); only a damaged save has an ID with no switch.
     *(Corrected: an earlier text said the previous step.)* Example: a bus route whose in-city cells have IDs
     0, 7, 7, 0, 7 counts switch 7 twice, at the second and the fifth cell.

     `switchFee` is the switch's toll, the f32 building property `0x0BFC0A4C` (traffic-network.md §2.4; absent →
     0.0), so only toll booths charge (TS15). Example: 37
     commuters by car, Carpool off, `Q = 37.0`, through a toll booth with toll 0.1 (f32 `0.100000001`):
     `r24(37 × 0.1)` = 3.70000005 (the f32 nearest 3.7), added to `fees` and at month end to the toll income
     (§4.12). A return leg through the same booth is a separate ApplyRoute and charges again; mode 2 charges once.
  3. **Edge density** (§4.8): `e` and `o` are the route reader's entry and exit sides of the cell (§4.3, as
     ConfirmRoute reads them; around a jump they follow the reader's jump rules). There are two overrides (TS24):
     - the **first** cell's entry is the side towards `R`: `x < x0 → +x`, `x > x1 → −x`, else `z < z0 → +z`,
       else `−z` (`0x00719B2D`). This is always defined, however far the rect is;
     - the **last** cell's exit, in modes 0 and 2, is the unit step from the cell to `(destX, destZ)`. It is
       defined only when that cell is exactly one step away along x or z (16-bit differences), so no update when
       the destination is the cell itself or farther away. In mode 1 it is the side towards the `sourceRect`
       parameter (not `R`): `x < x0 → +x`, `x > x1 → −x`, `z < z0 → +z`, else `−z`. This is always defined; a cell
       inside the rect gives `−z`.

     With both defined: record key `(mode = 1) << 24 | t << 16 | x << 8 | z`, `current[4e + o] := min(0xFFFF,
     current + Qi)`.
     **Mode 2 also adds `Qi` to the record with key `1 << 24 | t << 16 | x << 8 | z` at index `4o + e`** (the mirrored
     direction) *[compat]*. The pathfinder's `ApplyTraffic` (pathfinder.md §4.17, no known caller) mirrors the same
     way (corrected by PF2).

     At the last cell an undefined side only skips this update. At any **other** in-city cell, an undefined side
     ends ApplyRoute at once (`0x00719DC3` → `0x0071A0AF`) *[quirk]*. Steps 4–5 of the cell, all later cells, the
     destination step and the final accumulation are skipped, so the route's `I` and `fees` are lost. Volume,
     switch usage and edge densities already added stay. The reader leaves a side undefined only around a jump
     to an x-neighbour (`dx = ±1`, `dz = 0`, §4.3). For example, a ferry departure cell beside the in-city edge
     cell of its neighbour link: the off-map node is written as that edge cell (pathfinder.md §4.15). *[choice]*
     Follow the original.
  4. **Ferry legs** (TS16, `0x00719E8F`–`0x0071A054`): only at a cell that is not the last, when the **next** route
     record has the jump flag and the cell is the departure cell of a terminal `T` (traffic-network.md §2.6). The
     jump flag (TS39) is bit 7 (`0x80`) of the jump token that leads to the next record. The pathfinder sets it when
     the cell before the jump is off-map or has the ferry wormhole bit (pathfinder.md §4.15). The reader keeps it in
     the record the jump arrives at (`0x006D65C6`–`0x006D65CC`), a run token clears it (`0x006D6593`), and
     ApplyRoute tests it at `0x00719E97`. Then:
     - `T`'s current riders `+= Qi`, twice in mode 2;
     - `I := r24(I + r24(Q × ferryFare))`, once in every mode. The fare is income, not a fee;
     - if the next cell is the arrival cell of a terminal `T2`: `L :=` `T`'s terminal link keyed by `T2`'s departure
       cell. Mode 0: `L.currentA += Qi`. Mode 1: `L.currentB += Qi`. Mode 2: `L.currentA += Qi`, and `T2`'s link
       keyed by `T`'s departure cell gets `currentB += Qi`, the reverse crossing. The reverse link is looked up even
       when `L` is missing; a missing link is skipped;
     - otherwise `T`'s neighbour link whose off-map cell or edge cell is the next cell gets `currentA += Qi` (mode
       0), `currentB += Qi` (mode 1) or both (mode 2); none, nothing.

     The counters are u16 and wrap; unlike the edge densities they are not clamped. A is the outbound-leg count and B
     the return-leg count, each in the link's own direction. On a crossing to a neighbour the next record holds the
     connection's in-city edge cell, because the route writes an off-map node that way (pathfinder.md §4.15), so the
     neighbour link is found by its edge cell. The arrival table is tried first, so an edge cell that is also some
     terminal's arrival cell counts on a terminal link instead.

     Example, mode 2: 30 pedestrians (`Q = 30.0`, `Qi = 30`) cross from terminal A to terminal B with the shipped
     fare 0.01. A's current riders `+= 60`; `I += r24(30 × 0.01f)` = 0.29999998; A's link to B gets `currentA +=
     30`; B's link to A gets `currentB += 30`.
  5. **Income**: `I := r24(I + r24(Q × incomePerTile[t]))`.
* At the last cell, unless mode 1: `ApplyTrafficToDestination(count, destX, destZ, lastTravelType, type)` (§4.2).
* Finally `incomeAcc := incomeAcc + I` and `feeAcc := feeAcc + fees` (doubles, the additions rounded to 24 bits).

*[choice]* A travel type above 8 (TS48; only in a damaged save, as the reader allows 0…63) adds nothing here. The
original indexes its 9-entry tables with it unchecked: the type-to-network table (`0x00AB31A8`, read at
`0x00719C2F`), the generates-traffic flags (`0x00719C4B`) and the income per tile (`0x00719EE9`). It reads whatever
follows them and may add volume or income.

**Off-map roots** (TS13). The original never produces a route whose root is off-map. Start nodes are seeded on rings
clamped to the map: a neighbour-connection start rect `(−1, z)` seeds at `(0, z)` with entry side 0, and the ferry
fallback starts on the in-city arrival cell (pathfinder.md §4.6). So the header's "off-map root → inner cell"
replacement (pathfinder.md §4.15) is unreachable. The reader (`0x006D64B0`) takes the header x and z as zero-extended
bytes and decodes every step literally, with no off-map adjustment. A route with an off-map root, encoded as the inner
cell followed by unit steps measured from the off-map cell, would therefore be replayed one cell off by the original
too. *[choice]* Decode literally, as the core does. A shadow difference here points at a pathfinder that seeded off
the map, not at the decoder.

### 4.6 Daily results: trip length per source (CompleteTripSourceCalculations, `0x0070F0A0`)

For each trip of the source (`passTotal += 1` per trip):

* **status 3**: flags |= 8; `t := r24(float(count) × tripLength)`; `acc := r24(acc + t)`; `n += count`. Unless probe:
  commuters (type < 3): `attempted[type] += count`, `routed[type] += count`, `carLenSum := r24(r24(float(count) ×
  tripLength) + carLenSum)`, `carLenCount += count`; freight: `freightLenSum := r24(freightLenSum + t)`,
  `freightCount += count`; for each bit `i` of `travelTypesUsed` (0…8): `typeCount[i] += count`.
* **status 5**: flags &= ~8; `failed := true`; `t := r24(float(count) × commuteMaxTime)`; `acc`, `n` as above;
  commuters (not probe): `attempted[type] += count`; freight (probe or not) *[quirk]*: `freightLenSum`,
  `freightCount` as above.
* other status: flags &= ~8.

If `n ≠ 0`: `avg := r24(acc / float(int32 n))`. If the source rect's first corner is in the city, the trip-length map
is filled over the whole source rect with

`v := failed ? 0 : clamp(round(r24(r24(avg / commuteMaxTime) × 255.0f)))`, clamp: `> 255` (unsigned) → 255,
`0` → 1.

(`255.0f` = `0x00A9D8B0`, dword.) A lot with any failed trip therefore shows **0** ("no commute"; the growth
developer's commute layer ignores 0, growth-developer.md §4.3) *[compat]*. Otherwise (off-map source) the first
neighbour connection found at that point (neighbour simulator slot 11 `FindConnection(type, x, z)` for type 0…12)
gets property `0xCA947077 := avg` (float).

### 4.7 FinishTripMap (state 15, `0x0070FEF0`)

**Time slicing** (TS63). The phase is deadline-stoppable per lot: the test point is before each lot is taken
(`0x0070FF84`, §3.4). A lot is taken from the lot iterator, its location (slot 21) is written to the phase positions
(`0x0070FFE0`) and it is then handled completely (flag clear, borrow, write). Entering 15 sets the positions to (0, 0) (§3.2, TS67), so before the first lot is taken they are (0, 0), not the last FullTrips source's cell. So a stop always falls after a whole
lot, and the saved cell is that of the last lot handled; the resume (§3.5 step 11) skips past it. A call can
therefore return part-way through state 15; DoDailySimulation then returns false and the next call of the same day
continues (§3.3 step 3).

For each lot (lot iterator): clear the lot's flag `0x80` (slots 12/13). For residential lots that are not
"traffic sources of their own" (zone 8 or 9 *[inferred: these are the zone values tested]* and slot 28 false), the
trip-length value is borrowed: from the cell of the lot named by slot 59 if any; else by searching outward along
the four directions (−x, −z, +x, +z, distance 1, 2, …; each direction stops when it leaves the city, finds no lot or
a lot of another zone) for a lot of the same zone with slot 28 true; its trip-length value is written over the
lot's rect.

The borrow in detail (`0x00710016`–`0x007101D7`, TS26). `value := 0`.

- **Linked lot.** If slot 59 gives a lot, `value :=` the trip-length map byte at that lot's location (slot 21).
  Its zone and slot 28 are not checked.
- **Search.** Otherwise `(x, z)` := the lot's own location. The search visits distance `n = 1, 2, …`, and at each
  distance the directions in the order `−x, −z, +x, +z` (tables `0x00AB325C`/`0x00AB326C`), skipping those already
  closed. The cell is `(x + n·dx, z + n·dz)`.
  - A cell outside the city, with no lot (`GetLot(…, false)`) or with a lot of another zone **closes** that
    direction.
  - A cell of the lot itself, or of a same-zone lot whose slot 28 is false, is passed over: the direction stays
    open.
  - The first cell whose lot has the same zone and slot 28 true ends the search, with `value :=` the trip-length
    map byte at **that cell** (not at the donor's location).
- **Write.** `value` is stored into every cell of the lot's bounding rect (slot 14). When every direction closes
  without a donor, the value written is **0**.

Example: a lot at location (20, 20) with rect (20, 20)–(20, 21).

- Distance 1. −x, (19, 20): a lot of another zone, so −x closes. −z, (20, 19): no lot, so −z closes. +x, (21, 20): a
  same-zone lot with slot 28 false, passed over. +z, (20, 21): the lot itself, passed over.
- Distance 2. Only +x and +z are open. +x, (22, 20): a same-zone lot with slot 28 true, whose map byte is 37.

So `value = 37` is written into (20, 20) and (20, 21), and +z at distance 2 is never looked at. The bracket order
(distance first, then −x, −z, +x, +z) is the original's.

### 4.8 Edge densities and transit switches (states 3, 7, 6; slot 12)

`TrafficEdgeDensity` = 33 u16: `last[16]` (index `4·entry + exit`), `usage`, `current[16]` (the SDK's layout of
`0x42` bytes is correct). Keys `evening << 24 | t << 16 | x << 8 | z` (`evening` = the mode-1 / mode-2-mirror
record).

* **Dump** (state 7): for every record `last := current`, `usage := min(0xFFFF, Σ current)`.
* **Reduce** (state 6): `current := 0` for all 9 travel types and both evening values at every cell with network
  info or a transit switch, and at the ferry terminal cells. Records are erased only when the network occupant is
  removed (§3.1). In detail (`0x007110B0`, TS22):
  1. The time-sliced cell scan visits every cell. A cell takes part when it has a network-info record or a non-zero
     switch ID. Each of its 18 keys (t = 0…8, evening 0 and 1) is looked up, and a record that exists has its
     `current[16]` zeroed. A missing record is **not** created.
  2. When the scan is done, in the same call and with no deadline test, each terminal in list order is handled the
     same way at its **arrival** cell (x2, z2).

  The departure cell needs no pass of its own: the ferry phase of every rebuild gives it network-info records for
  the pedestrian and road networks (traffic-network.md §4.9.2), so step 1 covers it. The arrival cell gets only
  connection bits there, hence step 2. An arrival cell that also has network info is cleared twice, which is
  harmless.
* **GetTrafficEdgeDensity(x, z, t, evening)** (slot 12): the record, or a shared all-zero record (`0x00AB31D0`) when
  missing. The returned pointer must not be written by callers.
* **ReduceTransitSwitches** (state 3, `0x00713750`): for every transit switch, in table order, `load :=
  r24(densityReduction × load)` (`0x29136788`, the same factor as the volume reset of §4.9 step 3). Nothing
  else: the two saved counters keep what CreateMaps (state 2, earlier in the same cycle) wrote, `prevUsage := load`
  and `ratio := min(load / capacity, 1)` (§4.9 step 4). Finishes in one call. With the shipped factor 0 the load
  restarts from 0 for the coming trip phases (`0 × NaN` and `0 × ±∞` give NaN; a negative load would become −0.0).
  With a non-zero factor the load decays instead *[compat]* (TS3).
* **GetTravelTimeRatio(x, z, t)** (slot 24, `0x007103B0`): 1.0 if `affectedByTraffic[t]` is false. Else
  `u := Σ` over travel types `i` with `generatesTraffic[i]` of `usage(x, z, i, 0) + usage(x, z, i, 1)` (u32), and with
  the cell's network info for `network(t)` (none → 1.0): `congestionCurve(r24(float(u) × invCapacity))`.

### 4.9 CreateMaps (state 2, `0x0071A0C0`)

Per cell (x innermost), with `B` = peak volume grid, `D` = peak ratio grid:

1. `B := 0`, `D := 0`.
2. If `x mod 4 = 0` and `z mod 4 = 0`: `commercialWork[x/4][z/4] := min(255, round(r24(float(v) × commercialScale)))`,
   where `v` is the commercial traffic window value of block `(bx, bz) = (x/4, z/4)` (`0x00714AA0`, TS8). Rounding is
   to nearest, ties to even: the f32 magic-number trick `+ 12582912.0f`, `0x0071A271`. Definitions:
   * **Cell value** `cv(x, z, k)` (`0x00714940`), `k` = the `evening` byte of §4.8 (0 or 1), with `usage(x, z, t, k)`
     = the `usage` field of the edge-density record (§4.8; 0 when there is no record). The cell value is the maximum
     (u16) of three per-network sums:
     * road: `usage(t = 1 Car) + usage(2 Bus) + usage(4 FreightTruck)`, summed in u16 (mod 2¹⁶);
     * el-train: `usage(7)`;
     * monorail: `usage(8)`.

     Travel types whose network (traffic-network.md §2.1, table `0x00AB31A8`) is pedestrian, rail or subway add
     nothing.
   * **Window** of block `(bx, bz)`, with `X0 = 4·bx` and `Z0 = 4·bz`: a plus shape, the 4 × 4 block widened by 2
     cells on each side in each direction, without the corners:
     * `z` runs from `max(0, Z0 − 2)` to `min(H, Z0 + 6) − 1`;
     * on the block's own rows (`Z0 ≤ z < Z0 + 4`), `x` runs from `max(0, X0 − 2)` to `X0 + 5`;
     * on the other rows, `x` runs from `X0` to `X0 + 3`.

     That is at most 48 cells.
   * `v := (maxM + maxE) mod 2¹⁶`, where `maxM` = the maximum over the window of `cv(·, ·, 0)` and `maxE` = the
     maximum of `cv(·, ·, 1)`. The two maxima are taken independently and may come from different cells. The sum
     is truncated to u16 by the caller.
   * *[quirk]* The `x` range on the block rows has **no upper clamp**, and the record key uses the low byte of `x`.
     So in a 256-wide city the east-most block column (`X0 = 252`) also reads cells `x = 0` and `x = 1` on the same
     rows: the west edge. In 64- and 128-wide cities, `x = W` and `W + 1` have no records and give 0. Keep it
     *[compat]*.

   Worked example, block (2, 3): `X0 = 8`, `Z0 = 12`. The window is rows 12–15 with x = 6–13, and rows 10, 11, 16, 17
   with x = 8–11 (48 cells).
   * Morning: at (13, 14), Car 300 + Bus 40 + FreightTruck 100 = road 440. At (9, 10), el-train 500. So `maxM` =
     500.
   * Evening: the largest cell value is 380, at (6, 12). So `maxE` = 380.
   * `v` = 880. With `commercialScale` 0.128: `r24(880 × 0.128)` = 112.64, so `commercialWork[2][3]` = 113.
3. For transit network `n` = 0…5 with network info `e` at the cell (`vol := volume[n]`):
   * `n ∉ {2, 3}` (not rail, not subway): `B := max(B, vol)`;
   * `r := r24(vol × e.invCapacity)`; `D := max(D, r)`;
   * accident list (road and rail, `n ∈ {1, 2}`, and the cell's connection count `> 3`):
     `p := accidentCongestionCurve(r)`; if `p > 0`, `p2 := r24(accidentCapacityCurve(r24(1 / e.invCapacity)) × p)`;
     if `p2 > 0`, the cell is added to the accident-prone list with probability `p2` (`0x00716950`);
   * **statistics** (TS9). No record is excluded; the flag only picks the slot. The slot is `s := 13` (the
     **bridge** slot) when `e.flags` bit 0 is set, else `s := e.type`. Bit 0 means the occupant has network flag
     `0x40000` (traffic-network.md §2.2). The traffic advisor publishes slot 13 as `ga_bridge_congestion` and
     `g_bridge_congestion_tile_count` (`cSC4AdvisorTraffic::update_variables`, `0x0041C1A0`; it reads published row 13, advisors.md §7.7), hence the name.
     * `sum[s] := r24(sum[s] + r)`, `count[s] += 1` (u32), `records[s] += 1` (u16, wraps).
     * If `r > max[s]` (ordered, so a NaN never wins; `max` starts at −1.0): `max[s] := r` and
       `maxCell[s] := (x, z)`.
     * If `r > 1.0` (ordered): `congested[s] += 1`.

     Every record of every transit network counts, the pedestrian one (`n = 0`) included. A road or street cell
     therefore normally adds 2 to `records[type]`: its pedestrian record and its car record have the same type
     (traffic-network.md §4.2).
   * `volume[n] := r24(densityReduction × vol)` (shipped `0x29136788` = 0: volumes restart from 0 every cycle);
   * road (`n = 1`): `volume[1] := r24(volume[1] + GetBackgroundTraffic(x, z))` — background traffic therefore
     appears in the **next** cycle's maps.
4. Transit switch at the cell: `prevUsage := usage`; `ratio := usage / capacity`; `D := max(D, ratio)`;
   `switch.ratio := min(ratio, 1)`.
5. If any network info exists at the cell: `B := max(B, 1.0)`.

At the end of the pass (`0x0071A5D7`–`0x0071A701`):

1. **Publish the statistics.** For each slot `s` = 0…13: if `count[s] ≠ 0`, `sum[s] := r24(sum[s] / float(u32
   count[s]))`. Then copy the five fields `(average, congested, maxX, maxZ, max)` into the "last" statistics (the
   slot-34 block, rows 0–12, then the bridge row). A slot with no record publishes `(0.0, 0, 0, 0, 0.0)` (TS40). The
   state-2 entry reset (`0x00721196`–`0x007211D6`) zeroes the five fields of every working row and sets a separate
   per-slot comparison value to −1.0. A record (`0x0071A3D7`–`0x0071A40C`) writes the row's max fields only when `r`
   beats that value. An empty slot's max cell (0, 0) is therefore not a cell; its record count (step 2) is 0.
2. **Replace the cell counts.** `records[t]` for t = 0…12 become the published u16 × 13, **and**
   `cells[t] := records[t]` (u32). This replaces the running counts that the occupant messages keep between passes
   (§3.6). The bridge slot's `records[13]` is neither published nor a cell count, so bridge pieces are in no cell
   count after a pass. Road-like counts are about twice the cells. *[compat]*
3. `UpdateRoadsBudget` (`0x0070D8A0`), then `UpdateMassTransitBudget` (`0x0070DA40`) (call sites `0x0071A6FA`,
   `0x0071A701`), every pass, whether or not a count changed. They set the departments' line items from these
   counts and the per-tile costs `0x6A84493E`; the exact rule is §4.9.1.
4. The accident-prone list is copied for the accident check.

Corpus check (read-only, the eleven shipped saves with record version ≥ 37):
* The saved cell counts equal the published u16 counts in every one.
* Road + Street + Avenue + OneWayRoad counts are twice the distinct road keys of the damage block, exactly in seven
  of them, five distinct cities (Fulham: 538 + 458 = 996 = 2 × 498).
* The tutorial city's rail count is 143 against 135 rail keys. The difference is consistent with level crossings,
  which DoRailChanged leaves out of the rail sets (§3.6).

**GetBackgroundTraffic(x, z)** (slot 11, `0x0070E870`): with `R`, `C`, `I` = the sums of the BDS developers'
population grid values (developer slot 15 `GetCellValue`, s16) at the cell for residential `0x1010, 0x1020,
0x1030`, commercial `0x3110, 0x3120, 0x3130, 0x3320, 0x3330`, industrial `0x4100, 0x4200, 0x4300, 0x4400`:
`r24(r24(r24(R × bg[0]) + r24(C × bg[1])) + r24(I × bg[2]))`, returned in ST0.

#### 4.9.1 UpdateRoadsBudget and UpdateMassTransitBudget (TG23, re-answers21) *[compat]*

Both functions price the per-network-type counts `cells[t]` (§3.6, replaced at every CreateMaps pass) with the
per-tile costs `tileCost[t]` (property `0x6A84493E`, 13 floats indexed by network type, §4.14) and **set** (not add
to) the full expenses of fixed line items of the simulator's own departments. They read nothing else, keep no
"previous value", and have no early return: every call rewrites every line item below.

**Pricing one type.** `price(t)` is a u32 computed with single-precision x87 arithmetic (24-bit rounding, as all
simulation arithmetic):

1. `n := cells[t]` loaded as an unsigned 32-bit value (a value at or above 2³¹ is corrected by adding 2³²).
2. `p := r24(n × tileCost[t])`.
3. `s := r24(p + 12582912.0)` (the f32 constant `0x4B400000`, 1.5 × 2²³), stored as an f32.
4. `price(t) := bits(s) − 0x4B400000`, as a u32 (wrapping).

For `0 ≤ p < 2²²` this is `p` rounded to the nearest integer, **ties to even** (0.5 → 0, 1.5 → 2, 2.5 → 2,
10.5 → 10). Outside that range the bit pattern gives something else, which a replacement keeps *[compat]*: a
negative tuned cost gives a negative integer read as a u32 (−3 → `0xFFFFFFFD`), and a product of 2²² or more no
longer lands on an integer grid of 1. With the shipped costs a product of 2²² would need tens of millions of
records, so normal play never leaves the exact range.

**Combining two types.** A line item fed by two types gets `price(a) + price(b)` computed as an unsigned 64-bit
sum of the two u32 values (the carry goes into the high word, `0x0070D91C`/`0x0070D91E`). A line item fed by one
type gets that u32 zero-extended to 64 bits.

**Writing.** For each line item, in the order of the tables below: `department.GetLineItem(i)`
(cISC4DepartmentBudget slot 20, `+0x50`, `0x006AA500`, borrowed pointer or null), then
`lineItem.SetFullExpenses(v)` (cISC4LineItem slot 12, `+0x30`, `0x006A9A00`; argument is the int64 `v`, pushed as
two dwords). `SetFullExpenses` is budget.md §8: `full := v`, `current := trunc(v × department funding fraction)` for
a non-local line, then the department's `0x08DAC3E8` notification, which is sent even when `v` equals the old value.
`AddToFullExpenses` (slot 13) is **not** used.

UpdateRoadsBudget (`0x0070D8A0`), roads department `0xC921CEEA`:

| Order | Line item | Name (TG7) | `v` |
|---|---|---|---|
| 1 | 2 | `0x2A8446DA` | `price(0 Road) + price(10 OneWayRoad)` |
| 2 | 1 | `0xEA8446BE` | `price(3 Street)` |
| 3 | 3 | `0x2A8446DF` | `price(2 Highway) + price(12 GroundHighway)` |
| 4 | 4 | `0xCBCE33D5` | `price(6 Avenue)` |

No line item is null-checked: Init creates items 1–4 (§9.1 TG7), and the original would fault on a missing one. Roads
items 5 (tolls, Income) and 6 are not touched.

UpdateMassTransitBudget (`0x0070DA40`), mass transit department `0xC921CEEB`:

| Order | Line item | Name (TG7) | `v` |
|---|---|---|---|
| 1 | 3 | `0x6A8B0CFB` | `price(1 Rail)` |
| 2 | 4 | `0x2AB7EA93` | `price(7 Subway)` |
| 3 | 5 | `0x8BCE33E0` | `price(8 LightRail)` |
| 4 | 6 | `0x8BCE33E1` | `price(9 Monorail)` |

Item 3 alone is null-checked: when `GetLineItem(3)` returns null, it is created on the spot with
`CreateLineItem(3, false)` (slot 17, `+0x44`) and named `SetName(0x6A3FF01C, 0x6A8B0CFB)` (`0x0070DA8A`–
`0x0070DAAA`), then priced. Items 4–6 are not checked. Items 1 (`0xEA5F96E7`) and 2 (the Income line `0x8A5F978C`)
are not touched. The types 4, 5 and 11 (DirtRoad) are priced by neither function.

**Callers** (every direct call in the executable, found by a scan for calls to the two addresses):

| Caller | Calls |
|---|---|
| DoRoadChanged (§3.6, `0x00715630`) | UpdateRoadsBudget, after `bump(t)`, on every insert and remove of a counted road type, highways included |
| DoRailChanged (§3.6, `0x00715927`) | UpdateMassTransitBudget, level crossings included |
| DoSubwayChanged (§3.6, `0x0070EA85` insert, `0x0070EA9E` remove) | UpdateMassTransitBudget |
| CreateMaps (§4.9 step 3, `0x0071A6FA`, `0x0071A701`) | UpdateRoadsBudget, then UpdateMassTransitBudget |
| DoPostCityInit (§3.5 step 14, `0x00720E06`, `0x00720E0D`) | UpdateMassTransitBudget, then UpdateRoadsBudget |

So after a load the departments' saved full expenses are overwritten at PostCityInit from the saved cell counts and
the current tuning, and afterwards they follow every road, rail or subway piece placed or removed and every
CreateMaps pass.

Worked example (shipped costs `0.1, 0.03, 0.5, 0.05, 0, 0, 0.15, 0.3, 0.2, 0.3, 0.1, 0, 0.5`). Counts Road 538,
OneWayRoad 10, Street 458, Highway 100, GroundHighway 21, Avenue 37, Rail 143, Subway 60, LightRail 7, Monorail 3:

| Type | `p` | `price` |
|---|---|---|
| Road 538 | 53.799999 | 54 |
| OneWayRoad 10 | 1.0 | 1 |
| Street 458 | 22.9 | 23 |
| Highway 100 | 50.0 | 50 |
| GroundHighway 21 | 10.5 | 10 (tie to even) |
| Avenue 37 | 5.55 | 6 |
| Rail 143 | 4.29 | 4 |
| Subway 60 | 18.0 | 18 |
| LightRail 7 | 1.4 | 1 |
| Monorail 3 | 0.90000004 | 1 |

Roads: item 2 := 55, item 1 := 23, item 3 := 60, item 4 := 6. Mass transit: item 3 := 4, item 4 := 18, item 5 := 1,
item 6 := 1.

### 4.10 CreateFinalMap (state 4, `0x0070F460`)

Per cell with `b := B`:

* `b = 0`: air-polluting := 0, congestion := 0.
* else: air-polluting := `min(255, round(r24(b × 0.1f)))` (unsigned compare; `0.1f` = `0x00A8C950`);
  `c := congestionCurve(D)`; congestion := 1 if `c ≤ 1`; else `g := round(r24(r24(c − 1) × congestionScale))`,
  congestion := 255 if `g > 255` (unsigned), 1 if `g = 0`, else `g`.

`congestionScale := 255 / (lastY − 1)` of the (inverted) congestion curve (§4.14). Writes address `[x][z]` of the grid
data directly, assuming tract size 1 for both u8 grids *[quirk]*. Then posts `0x69247DC7` (TrafficMapChanged, §5).

### 4.11 ClearTripDestinations (state 5) and RefreshCommuteMap (state 10)

**ClearTripDestinations** (`0x0070E4F0`): per cell with a lot (`GetLot(x, z, true)`): `arrivedLast[k] :=
arrivedThisCycle[k]`, `arrivedThisCycle[k] := 0` (k = 0…3); `jobCapacity[k] += GetMaxTripCapacity(holder, k)` for
k = 0…2. The cell iterator steps one cell at a time (step 1, `0x0071F47B`), and `GetLot(…, true)` answers only at
the lot's origin cell (`0x0072B453`), so each lot is rolled and counted **once**. *(Corrected (TS19): an earlier
text inferred once per lot cell.)*

**RefreshCommuteMap** (`0x0070F6E0`, despite the name it refreshes the **destination map**): per cell, the
destination byte `&= 0xF8` (bits 0–2 cleared, *[quirk]* bit 3 = freight destination is never cleared); with the lot
(`GetLot(x, z, true)`): bit `k` := `GetMaxTripCapacity(holder, k) ≠ 0` and `freeDest[k] += 1` for each set bit
(k = 0…3); else with `GetLot(x, z, false)`: bits only.

### 4.12 FinalCalculations (state 8), DoMonthlySimulation and history

**FinalCalculations** (`0x0070E7D0`): `transitIncome := float(incomeAcc)`, `tollIncome := float(feeAcc)`;
`avgCarCommute := r24(carLenSum / float(u32 carLenCount))` *[quirk: no zero check; 0/0 = NaN]*; if
`freightCount ≠ 0`, `freightByte := low8(ftol(r24(r24(r24(freightLenSum / float(freightCount)) / commuteMaxTime)
× 255.0f)))` (truncation, no clamp *[quirk]*), else 0. `freightByte` is the **last byte of the published statistics
set** (offset `0x238` of the slot-34 block, member `+0x41C`; written at `0x0070E84A` / `0x0070E862`), the unnamed
`u8` that ends each set in §6.1 row 16. It changes once per cycle, here, by recomputation, not by an increment (TS61).
The same byte of the working set (`+0x678`) is written only by the constructor (0, `0x0071FE52`) and Read (the saved
byte, or `0x80` for an old record, `0x00722D55`), and the publishing step of CreateMaps copies field by field without
it, so it keeps its loaded (or constructed) value. The next state is 0:

**StartRecalc** (state 0, `0x00717C90`): remembers the cycle start date (`GetSimDateNumber`); income and fee
accumulators := 0.0; `jobCapacity[k]`, `attempted[k]`, `routed[k]` := 0; `rerouteBucket := (rerouteBucket + 1) & 3`;
car and freight length sums and counts := 0; then `RecalculateNeighborConnectionCapacities` (`0x00714E60`,
traffic-network.md §2.7.4: regional job capacities per established neighbour pool, the default pool's freight
counter, and destination bits plus `freeDest` on the off-map edge cells; not a no-op, TS12). Finishes in one call.

**DoMonthlySimulation** (`0x00718AC0`), in this order:

1. `ApplyRoadFunding(f)` (`0x00717D30`), `f` = the roads department's funding fraction (department slot 10 returns a
   pointer to it), then `ApplyMassTransitFunding(f')` (`0x00717E00`) with the mass-transit fraction. Each keeps its
   own **damage rate** `d` (road, rail; saved):
   * `d := r24(damageCurve(r24(f × 100.0f)) + d)`, then `d := max(d, 0)`, `d := min(d, 1)` (NaN passes both);
     `damageCurve` = `0x0BD86FD4` for both kinds (shipped: +0.02 per month at 0 % funding, −0.01 at 100 %, −0.04 at
     120 %);
   * `target := low32(ftol(r24(float(u32 cellCount) × d)))` with `cellCount` = the number of road (resp. rail)
     cells kept by the network part;
   * if `target ≠ damagedCount`: `ApplyPotholeChanges(kind, target − damagedCount)` and `damagedCount := target`.
     Road only: then the published pothole count (u16 at offset `0x134` of the slot-34 block, §3.6) := the low 16
     bits of `damagedCount`. Rail writes no pothole count, and sets `damagedCount` before the call rather than after
     (the outcome is the same).

   **ApplyPotholeChanges(kind, delta)** (`0x00715FB0`; kind 1 road, 2 rail): the network part keeps, per kind, a
   set of intact cells and a set of damaged cells (SGI hash tables keyed by the u16 cell key `x << 8 | z`;
   traffic-network.md). For `delta > 0` cells move from intact to damaged, for `delta < 0` back; `|delta|` is capped
   at the source set's size. For each move: `r := RandomUint32Uniform(sourceCount)` on the simulator's generator;
   the `r`-th element of the source set **in its iteration order** (bucket index ascending, then chain order;
   SGI inserts at the head of a chain and grows the bucket array to the next prime of its prime list when the count
   would exceed it, re-linking old chains in order, each node at the head of its new chain) *[compat]*; if the cell
   has network info for transit network `kind`, its extra step cost := `damagedRoadExtraCost` (damaging) or 0
   (repairing) and message `0x69247DC8` is sent (data1 = x, data2 = z, data3 = 1 when damaged *[inferred: slot
   names, foundations §5.1]*); the key moves to the other set. `IsRoadDamaged` (slot 16) and `CheckRailAccident`
   (slot 17) test the damaged sets.
2. `UpdateStrikes` (`0x0070DBA0`), with `f` = the mass-transit funding fraction, `E` = the int64 mass-transit
   building expense total maintained by lot activation (§4.1.3; non-zero when the city has such buildings
   *[inferred]*), and the code-only constants `strikeStart` 0.7, `strikeKeep` 1.0, `strikeMaxMonths` 24,
   `strikeScale` 1.0 (no tuning property):

   | State | Condition | Result |
   |---|---|---|
   | 2 normal | `f < strikeStart` and `E > 0` (signed) | `p := r24(r24(strikeStart − f) × strikeScale)` (kept for the stats); `u := RandomDoubleUniform()`; if `p > u`: state 0, message `0x69247DC6` (strike starts) |
   | 2 normal | otherwise | nothing (no draw) |
   | 0 strike started | `f < strikeKeep` and `E ≠ 0` | state 1, month counter 0, "strike" flag := 1, message `0x69247DC4` |
   | 0 | otherwise | state 2, message `0x69247DC3` (strike over) |
   | 1 strike | `f < strikeKeep`, counter `< strikeMaxMonths`, `E ≠ 0` | counter += 1 |
   | 1 | otherwise | state 2, flag := 0, message `0x69247DC5` (strike over) |

   The pathfinder and `ConfirmRoute` treat state 1 as "on strike" (pathfinder.md, traffic-network.md).
3. `CheckForAccident` (`0x007184E0`, §4.13).
4. History (history warehouse slot 12 `+0x30`, `(id, u32)`): `0xEA5BA01D ← low32(ftol(r24(stats[0] × 100)))`,
   `0x8A5D4116 ← low32(ftol(r24(avgCarCommute × 25.0f)))` (25 hard-coded, `0x00AB32C0`), `0xABB8FAAC + i ←
   typeCount[i]` (i = 0…8), `0xEC0BDBF8 ←` the ferry total.

### 4.13 Accidents

**CheckForAccident** uses the running wall-clock stopwatch in **whole seconds**, not milliseconds. The
complete timing and empty-list rules are in §4.13.1. It runs only when elapsed seconds have passed the next check time; next check
:= now + `ftol(accidentCheckPeriod)` *[compat: wall-clock driven]*. It computes the visible cell rect from the view
(`0x00B43DD8` + `0x3EC`, world units / 16, rounded with the idiom and adjusted to the outer cells) and walks the
accident-prone list in the order of a **linear random iterator** (foundations §12.4, prime-step mode: one `rand()`
at reset). For each entry: draw `u := RandomDoubleRangeUniform(0.0, 1.0)` on the simulator's generator; if `u < p`
and the cell is in the visible rect and has an occupant (network-occupant query, `+0x1C` grid slot 14): create an
accident there (`cSC4TrafficAutomataManager::CreateAccident`, duration `ftol(r24(accidentDuration × 1000))` ms),
post the accident message `0x0C05E4C3` (§5; data1 = x, data2 = z of the entry, queued) and stop. The result of
`CreateAccident` is not checked: the message is posted even if the automata manager made no accident.
For exact replacement under the fixed schedule, use the shared virtual counter of simulator §5.8,
with the same stopwatch reads; a game-date or simulation-speed timer is insufficient.

**CheckRailAccident(x, z)** (slot 17, `0x00711590`): if the cell is in the damaged-cell set (hash at +0x804, key
`(x << 8 | z)` truncated to 16 bits, `x` the first argument): `u := RandomDoubleUniform()` on the simulator's
generator (+0x6FC, `cRZRandomSuper::RandomDoubleUniform` `0x00913A29`) and the result is
`railDamageAccidentFactor (+0xD04) > u`; else false, no draw. It posts **no** message; its result goes back to the
caller (train automata *[not traced]*).

**IsRoadDamaged(x, z)** (slot 16, `0x00720070`): membership of `(x << 8 | z)` in the damaged-road list
(traffic-network.md).

#### 4.13.1 Accident clock and the monthly random boundary (TE33-ACCIDENT-1) *[compat]*

Evidence: Init `0x0071F781`–`0x0071F798`, monthly call `0x00718AF4`, check
`0x007184EF`–`0x00718515`, iterator reset `0x00718675`–`0x0071869A` and
`0x005CF610`–`0x005CF636`, `0x005CF773`–`0x005CF784`; timer conversion
`0x00890229`–`0x0089024C`. These instruction checks supersede the earlier ms description.

- Init sets the unsigned 32-bit next-check field to **0**, calls timer `SetUnits(5)`
  (which clears accumulated time and the start stamp), then `Start()` once. Unit 5 is seconds
  (timers.md §1). This is not an initial deadline of 60 or 60,000. The stopwatch and deadline
  are runtime state, absent from the traffic save record. Reading the traffic record does not
  restore or restart them. At a fixed-schedule load, simulator §5.8 rebases the already running
  stopwatch to the load with zero accumulated ticks; the Init deadline is still zero.
- Every monthly call runs roads funding, mass-transit funding and UpdateStrikes, then this check,
  before funding snapshots and history publication (`0x00718AC4`–`0x00718AF9`). The check makes
  **one** `GetElapsedTime()` call, even if not due or the accident list is empty. A running unit-5
  timer makes one performance-counter read; it subtracts timer overhead and converts the tick
  interval using the stored binary32 seconds-per-tick factor, then truncates to integer seconds
  (timers.md §1.3). It neither stops nor restarts the stopwatch. Do not add a second read for scheduling.
- Compare the low unsigned 32-bit elapsed result `t` with the unsigned deadline: return immediately
  when `t <= nextCheck`. If `t > nextCheck`, set `nextCheck := t + trunc(accidentCheckPeriod)`
  modulo 2³², using the binary32 tuning value directly. There is **no ×1000** for this period,
  no catch-up loop, and no addition to the old deadline. The separate accident *duration* is
  converted to milliseconds by multiplication by 1000 (`0x0071875D`–`0x00718769`).
- After advancing the deadline, convert the view's four world-coordinate binary32 bounds to the
  inclusive cell rectangle (minima floor, maxima ceil, world scale 1/16). Then construct a fresh
  linear random iterator of the current accident-copy vector's entry count, with random traversal
  and reset-randomization enabled. With **zero entries**, Reset terminates without `rand()`;
  with **one or more entries**, it consumes exactly one CRT `rand()` to choose its initial index
  (`rand() % count`). The stride/visit rules are those of the linear random iterator (§4.15).
- Each visited entry consumes one simulator-generator `RandomDoubleRangeUniform(0.0,1.0)`
  **before** probability, rectangle or occupant rejection. Stop at the first qualifying entry
  that has a network occupant; otherwise visit the whole list. These are separate streams:
  the initial CRT draw does not replace a simulator-generator word. Controller attachment can
  have further effects owned by automata; its concrete call contract is automata-managers §3.4.1.
- The original directly dereferences the global view, without a null/unavailable-view branch
  (`0x0071851B`–`0x00718525`). There is no native rule permitting a missing view to skip the
  check or its CRT draw. A replacement needs a valid view-bounds provider for exact operation;
  reporting no occupant still happens after the iterator and per-entry draws.

Worked timing, shipped period 60: with `nextCheck=0`, a monthly read of 0 seconds consumes
zero accident draws. A later monthly read of 1 second sets the deadline to 61 and, for a
nonempty list of ten entries with no qualifying visible occupant, consumes **one CRT draw and
ten simulator-generator words**. A read of exactly 61 consumes neither; a read of 62 sets
122 and starts another check. This accounts for the type of January/February boundary in
TE33-ACCIDENT-1: 22 and 54 frames contribute 726 and 1782 ms before counter-read ticks, so
the first read can still truncate to zero and the second to one. The question's six measured
one-advance CRT states are consistent with this reset site. Their exact original entry counts,
probability outcomes and total counter-read ticks were not measured by that evidence and are
not inferred from the CRT stamps alone.

#### 4.13.2 Native accident creation boundary (AD5) *[compat]*

On probability/visibility success, the simulator obtains the vehicle manager from city slot 43,
then queries its network-info grid (slot 14) for the cell. If the network-info pointer and its first
network-occupant pointer are both non-null, it calls network-occupant slot 48 `AsOccupant()`
(no stack arguments), and directly calls the **concrete**, nonvirtual traffic-manager entry with
`(occupantView, int32 radius=2, uint32 durationMs)`. The occupant view is `cISC4Occupant*`,
not `cISC4NetworkOccupant*`. The duration is `trunc(r24(accidentDuration × 1000))`, low 32 bits.
The manager lookup precedes the grid lookup, with no manager-null check. Evidence:
`0x00718719`–`0x0071877E`; network-occupants §2.4 slot 48. See automata-managers §3.4.1
for the returned effect token, ownership and the integration restriction when that manager is replaced.

### 4.14 Tuning (`ReadSimulatorTunableValues`, `0x00716AE0`)

Exemplar `0x6534284A / 0xE7E2C2DB / 0xC9133286` from the resource manager; absent → nothing is read (constructor
defaults stay). Shipped values: `re/tuning/simulators.md` "Traffic Simulator". Order of reads *[compat]* below. Float
scalars use the f32 helper (unchanged if missing); "vector" helpers leave the target unchanged if the property is
missing.

| Property | Name | Code default | Shipped | Use |
|---|---|---|---|---|
| `0x491332E6` | Traffic Volume Per Population | — | 2 | read, unused |
| `0x491332E7 + t` | Max speed by network for travel type t (13 floats) | `FLT_MAX` inverse | see tuning | `invMaxSpeed[t][type] := 1 / r24(r24(v × 16.666666f) / cellWidth)` for `v > 0`, else `FLT_MAX` (class static `0x00B4B110`, pathfinder.md §4.1); only the first min(count, 13) |
| `0x29136788` | Monthly Traffic Density Reduction | — (uninitialised *[quirk]*; FreeCity4 0) | 0 | `densityReduction` (§4.9) |
| `0x29136789` | — | 0.03125 | — | read, unused |
| `0x6A84493E` | Monthly cost for network tile [13] | 0 | see tuning | department costs (§4.9) |
| `0xA92356AE`, `0xA92356AF` | Max roads / mass transit funding percent | 1.2, 1.2 | 120, 120 (→ 1.1999999) | department maximum funding; `r24(v × 0.01f)`, below |
| `0xA92356B0` | Damaged road extra step cost | 32.0 | 0.1 | traffic-network.md |
| `0xA92356B6` | strategy period (u32) | 365 | — | §3.3 |
| `0xA92356B7` | strategy buckets (u32) | 128 | — | §3.3 |
| `0xA92356B8` | Maximum distance from origin to network (u32) | 1 | 1 | pathfinder.md |
| `0xA92356B1` | Income per tile by travel type [9] | 0 | see tuning | §4.5 |
| `0xA92356B3` | Network Traffic Capacity [13] | 0 | see tuning | capacity and `invCapacity = 1/cap` (1.0 when `|cap| ≤ 1e-10`, double constant), traffic-network.md |
| `0xA92356B4` | Travel type generates traffic [9] | true | see tuning | §4.5 |
| `0xA92356BF` | Travel type affected by traffic [9] | true | see tuning | class static `0x00B4B100` |
| `0xA92356B5` | Travel type can reach destination [9] | true | see tuning | pathfinder.md |
| `0xA92356B9` | Congestion vs Speed (curve) | — | (0,1)(1,1)(2,0.65)(3,0.3) | class static curve `0x00B4B2F0` (below) |
| `0xA92356BA` | Commute trip max time | 90.0 | 6 | `commuteMaxTime` (max trip length, trip-length scale; slot 14) |
| `0xA92356BB` | Intersection and Turn Capacity Effect [n] | — | 0.7, 0.8, 0.9 | reciprocals stored; traffic-network.md |
| `0xA92356BC` | Trip Starting Cost by travel type [9] | 0 | 0, 0.4, 0… | neutral start costs, also the default array |
| `0xEA8C3CDB` | … for Mass Transit [9] | 0 | 0, 1.95, 0… | strategy 0 |
| `0xCAD64136` | … for Car Pref [9] | 0 | 0.1, 0… | strategy 1 |
| `0xA92356BD` | Job Scaling Constant | 1.0 | 1.2 | traffic-network.md *[inferred]* |
| `0x49A2E8BE` | Freight traffic scaling factor | — (uninitialised; FreeCity4 0) | 0.05 | slot 27, §4.1.1 |
| `0x4A678060` | Nearest Destination Attractiveness | 0.75 | 0.09 | pathfinder.md |
| `0xCA76013B` | Trip Length to Minutes Display Multiplier | 0.5 | 25 | slot 15 |
| `0xEA7B5F06` | Max Mass Transit Strategy Trip Length | 4.0 | 4 | *[inferred: not found in a read path]* |
| `0xA92356BE` | Population Background Traffic [3] | — (missing entries 1.0) | 0.05, 0.2, 0.2 | `bg[0…2]`, §4.9 |
| `0xCA5F7821` | Transit Switch Entry Cost vs. Budget (curve) | (0,0)(1,1) | (0,2)(100,1) | multiplier, §3.1 |
| `0x4BE09EC9` | Congestion to Accident Probability (curve) | (0,0)(0.25,0)(1,0.05)(2,0.25) | same | §4.9 |
| `0x4BE09ECA` | Capacity to Accident Probability (curve) | (0,1)(1000,1) | same | §4.9 |
| `0x4BE09ECB` | Accident Duration | 10 | 20 | §4.13 |
| `0x4BE09ECC` | Accident Check Period | 60 | 60 | §4.13 |
| `0x0BD86FD4` | Funding to Damage Acceleration Curve | (0,0.02)(100,−0.01)(120,−0.04) | same | traffic-network.md |
| `0x0BD86FD5` | Rail Damage Accident Factor | 1.0 | 1 | §4.13 |
| `0x8C3CAEF5` | Ferry Fare | 0.001 | 0.01 | §4.5 |
| `0x0CAAAEB8` | (commercial traffic scale) | 0.128 | 0.128 | `commercialScale`, §4.9 |

Then `UpdateTravelStrategies` (§4.1.2). `0xA92356B2` (Mass Transit Usage Chance) is not read by this function.

**Funding percents** (TS29). `0xA92356AE` and `0xA92356AF` (stored at +0xB88, +0xB8C; `Init` hands them to the
roads and mass transit departments) go through the percent getter `0x005FDAC0` (calls `0x00716D67`, `0x00716D79`).
For any numeric property type (u8, s8, u16, s16, u32 as unsigned, s32, 64-bit, f32, f64) it **multiplies** the
value by the f32 constant `0.01f` (0x3C23D70A, `0x00A823A4`) and rounds once to f32: `r24(v × 0.01f)`, not
`r24(v / 100)`. For an integer below 2^29 or an f32 the product is exact before that rounding. A missing property, or
one of another type (bool, string), leaves the code default 1.2 (0x3F99999A): the getter returns without a store
(TS37; missing `0x005FDAD6` → `0x005FDCBE`, other types `0x005FDAF3` → `0x005FDCBD`).

Worked example: shipped 120. `0.01f` is 0.00999999977648…; × 120 = 1.19999997317…, which lies below the midpoint
1.19999998807… of the two nearest f32 values, so the result is 0x3F999999 (1.1999999), one ulp **below** the code
default 1.2. Dividing by 100 would give 0x3F99999A. The two formulas agree for 50, 100, 110, 150 and 200.

**Short and missing arrays** (TS28). Each array read has its own rule; "the rest" means the target entries past the
property's count. Entries past the target's size are ignored.

| Property | Missing | Fewer entries |
|---|---|---|
| `0x491332E7 + t` max speed (13) | all 13 of type `t` reset as for `v ≤ 0` (speed 0, inverse `FLT_MAX`) | the rest reset the same way |
| `0x6A84493E` network tile cost (13) | all 13 := 0.0. There is no presence test (`0x00716D34`) | the rest := 0.0 |
| `0xA92356B1` income per tile (9) | unchanged | all 9 are copied with **no count check** (`0x00716E07`), so the original reads past the array *[quirk]*. *[choice]* the rest := 0.0 |
| `0xA92356B3` capacity (13) | unchanged | the rest := capacity 0.0, `invCapacity` 1.0 |
| `0xA92356B4`, `0xA92356BF`, `0xA92356B5` bools (9) | unchanged | the rest := true |
| `0xA92356BB` intersection effects | unchanged | the count is the array's own, stored as a u8 (`0x00717172`), so a count ≥ 256 wraps; reciprocals of that many entries |
| `0xA92356BC`, `0xEA8C3CDB`, `0xCAD64136` start costs (9) | unchanged | the rest := 0.0 |
| `0xA92356BE` background traffic (3) | unchanged | the rest := 1.0 |

The shipped exemplar gives full arrays everywhere, so only modified tuning reaches these rules.

**Shipped "Max speed by Network"** (`0x491332E7 + t`, 13 floats each). Source: the Traffic Simulator exemplar
`0xC9133286` in `SimCity_1.dat` (dump `re/tuning/simulators.md`). Columns are the network types in index order. A
blank cell is 0, which gives `invMaxSpeed = FLT_MAX`: the type cannot use that network, and a tunnel's cost is +∞.

| t | Travel type (property) | Road | Rail | Highway | Street | Avenue | Subway | El | Monorail | OneWay | GroundHwy |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 0 | Walk (`…E7`) | 3.5 | | | 3.5 | 3.5 | | | | 3.5 | |
| 1 | Car (`…E8`) | 31 | | 82 | 21 | 40 | | | | 31 | 82 |
| 2 | Bus (`…E9`) | 46 | | 100 | 31 | 60 | | | | 46 | 100 |
| 3 | Passenger train (`…EA`) | | 110 | | | | | | | | |
| 4 | Truck (`…EB`) | 31 | | 82 | 21 | 40 | | | | 31 | 82 |
| 5 | Freight train (`…EC`) | | 150 | | | | | | | | |
| 6 | Subway (`…ED`) | | | | | | 150 | | | | |
| 7 | El train (`…EE`) | | | | | | | 150 | | | |
| 8 | Monorail (`…EF`) | | | | | | | | 200 | | |

Pipe (4), PowerPole (5) and **DirtRoad (11)** are 0 for every travel type, so they have no columns here. With
CellWidthX 16, `invMaxSpeed` is:

| v | 3.5 | 21 | 31 | 40 | 46 | 60 | 82 | 100 | 110 | 150 | 200 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| bits | `0x3E8C6F2E` | `0x3D3B3EE7` | `0x3CFDB011` | `0x3CC49BA7` | `0x3CAAF6A7` | `0x3C83126F` | `0x3C3FD00C` | `0x3C1D4952` | `0x3C0EFCD6` | `0x3BD1B717` | `0x3B9D4952` |

For example, Walk on Road (3.5) gives 0.27428573, and Car on Road (31) gives 0.030967744. The constant is
`16.666666f` = `0x41855555`.

**Curves** (foundations §4.1 patterns):

| Curve | Pattern | Notes |
|---|---|---|
| Congestion vs Speed `0xA92356B9` | **keep** (`0x005F4700`), then transformed | after the load each point's y is replaced by `r24(1 / y)` and the curve is rebuilt from the new points (`0x005F4480`); `congestionScale := r24(255 / r24(lastY − 1))`. *[quirk]* A reload with the property missing inverts the kept curve again; an empty curve reads before its point vector (crash). FreeCity4: keep the previous curve unchanged when the property is missing, and treat an empty curve as "speed factor 1" with `congestionScale` 255 *[choice]*. |
| `0xCA5F7821`, `0x4BE09EC9`, `0x4BE09ECA`, `0x0BD86FD4` | **default** | rebuilt from the code default above, then loaded when present (`0x005F4620`) |

Shipped congestion curve after inversion: (0, 1), (1, 1), (2, 1.5384616 `0x3FC4EC4F`), (3, 3.3333333 `0x40555555`);
`congestionScale` = 109.28572 (`0x42DA924A`).

### 4.15 Random draws *[compat]*

The simulator owns one `cRZRandomSuper` (MT19937 with the non-standard seeding of foundations §6.1), seeded at Init
from the wall clock. *[choice]* FreeCity4 seeds it from the host-supplied seed for "traffic simulator" (foundations
§6.3).

**Where the original's generator is** (TS66). No interface slot and no save field exposes it. It is a heap object
of `0x9C8` bytes whose pointer is the simulator member `+0x6FC`, created by Init only when that member is null
(`0x0071F132`–`0x0071F1A9`; constructor `0x00913B74`) and seeded there once from the local date and time (C
`time`/`localtime`, built into a date-time value, then `Seed` `0x00913A72`), so there is no deterministic seed
path in the original. Layout: `u32 state[624]` at `+0x000`, a pointer to the next unread state word at `+0x9C0`,
and the count of words left before the next regeneration at `+0x9C4` (a draw decrements it and regenerates when it
goes negative, `0x009139B9`). Those 624 words, the index `(next − state) / 4` and the count are the generator's
whole state. A same-process observer that can read the simulator object can therefore copy that state after each
delivery and compare draw sequences (reading changes nothing), or seed both generators alike by calling `Seed(s)`
(`thiscall`, one u32, `ret 4`) on that object after Init (a write into the original, so a test-only choice).

| # | Operation | Generator | Method (advances) | Arguments | Position |
|---|---|---|---|---|---|
| 1 | strategy re-draw (§3.3) | simulator's `cRZRandomSuper` | `RandomUint32Uniform(100)` (1 word) | — | once per lot of the day's bucket, in multimap order, before any cycle work of that day |
| 2 | `DoLotActivated` (§4.1.3) | same | `RandomUint32Uniform(100)` (1) | — | after the bucket registration; only when the lot is found again and the building's `0xE90E25A1`, if present, has a byte size that is a multiple of 4 |
| 3 | `CheckForAccident` (§4.13.1) | C `rand()` (one call at a due check's iterator reset **only if its entry count is nonzero**), then the simulator's generator | `RandomDoubleRangeUniform(0.0, 1.0)` (1 word per visited entry, before visibility/occupant rejection) | doubles 0.0, 1.0 | until the first created accident or list exhaustion; not-due checks take neither stream |
| 4 | `CheckRailAccident` (slot 17) | same generator | `RandomDoubleUniform()` (1) | — | only for damaged cells; caller-driven (train automata) |
| 6 | `ApplyPotholeChanges` (§4.12) | same generator | `RandomUint32Uniform(sourceCount)` (1 per moved cell) | current size of the source set | monthly: road moves first, then rail |
| 7 | `UpdateStrikes` (§4.12) | same generator | `RandomDoubleUniform()` (1) | — | monthly, only in the normal state with `f < 0.7` and `E > 0`; after the pothole draws, before the accident check |
| 5 | `cSC4Lot::Init` strategy byte | the lot file's `cRZRandomSuper` (`0x00B49120`) | `RandomUint32Uniform(100)` (1) | — | only when the byte is `0xFF` (new lot) |

Draws 1, 2 and 5 only write values nobody reads (§4.1.1) but 1 and 2 advance the shared generator, so they matter
for the sequence of draws 3, 4, 6 and 7. The month order is fixed: pothole draws (road, rail), strike draw, accident
draws. No other draw exists in this class: trip generation, routing and the maps are deterministic, and the network
part draws nothing (traffic-network.md §12).

### 4.16 Station query: GetTransitSwitchQueryData(x, z, out) (slot 35, `0x0071CB80`) *[compat]*

`out` is five 32-bit fields, all written on success (TS11). The names are those of the query window's readers in
the Mac build (`cSC4ViewInputControlQuery::SetOrGetTransitStation…`):

| Field | Meaning (query reader) | Transit switch at the cell | Ferry terminal in the lot at the cell |
|---|---|---|---|
| +0x00 u32 | available capacity (service quality numerator) | `low32(ftol(capacity))`; 0 during a strike | riders last period; 0 during a strike |
| +0x04 u32 | maximum capacity | `low32(ftol(capacity))`, **also during a strike** | riders last period; 0 during a strike |
| +0x08 f32 | load fraction (no query reader) | the switch's `ratio = min(load / capacity, 1)` written by the last CreateMaps (§4.9 step 4); 0.0 during a strike | 1.0; 0.0 during a strike |
| +0x0C u32 | actual usage | `low32(ftol(prevUsage))` (the load seen by the last CreateMaps); 0 during a strike | riders last period; 0 during a strike |
| +0x10 u32 | monthly maintenance cost | cost (below), whatever the strike | cost (below), whatever the strike |

"During a strike" is the mass-transit strike state 1 (§4.12), for both kinds. `capacity` is the switch's building
property `0xE90E25A3`; "riders last period" is the terminal's u16 rolled on entering state 19 (traffic-network.md
§2.6). `ftol` truncates toward zero.

1. `id :=` the switch-ID grid at `(x, z)` (grid slot 7). If `id ≠ 0`: the switch is looked up with insert-if-missing
   (as traffic-network.md §4.7.3), the fields are filled from it, and the cost is taken from its building. Return
   **true**.
2. Else `lot := GetLot(x, z, false)`; none → return **false** (`out` untouched). Walk the ferry terminals in list
   order; the first whose lot cell (x, z) lies inside the lot's bounding rect (lot slot 14, bounds inclusive) is
   used, the cost taken from the lot's building (lot slot 48; no building → the cost stays 0). Return **true**. No
   such terminal → return **false** (`out` untouched).

**Cost**: `holder :=` the building's property holder; the budget simulator's `GetBudgetItemForPurpose(holder,
0xCA550301, item)` (slot 51) gives the building's first budget item with that purpose (budget.md §2); then
`dept := GetDepartmentBudget(item.department)` (slot 28). When both exist, cost `:= low32(ftol(r24(item.cost × f)))`,
`item.cost` the item's int64 monthly cost at 100 % and `f` the department's funding fraction (department slot 10, a
pointer to the fraction). Otherwise 0. The int64 is loaded exactly (x87 `fild`, `0x0071CC55` and `0x0071CD6F`), not
converted to f32 first, so the product is rounded once (TS46). This differs from an f32 conversion only above 2^24.
Example: cost 16,777,217 at `f = 0.75` gives 12,582,912.75, rounded to 12,582,913; converting first would give
16,777,216 × 0.75 = 12,582,912.

The query window then shows: service quality `0` if the call fails or +0x00 is 0, `1` if +0x0C is 0, else
`min(+0x00 / +0x0C, 1)`; usage percent `+0x0C / +0x04 × 100` (0 when +0x04 is 0); maximum capacity +0x04; usage
+0x0C; maintenance +0x10 *[inferred: from the Mac readers; the Windows readers were not traced]*.

*[choice]* A switch ID in the grid with no switch behind it (only possible from a damaged save) makes the original
insert a default switch with no building and crash on the cost step (TS49: inserted at `0x0071CBB7`; the null
building is read at `0x0071CC14`, after four of the five fields are written). FreeCity4 returns false and leaves
`out` untouched.

**Example.** A subway station with capacity 2500.0, last cycle's load 3100.0 (so ratio 1.0), no strike, budget item
cost 150 for purpose `0xCA550301` in a department funded at 0.9: `out = {2500, 2500, 1.0f, 3100, 135}`
(`r24(150 × 0.9) = 135.0`, truncated 135). The window shows service quality `min(2500/3100, 1) = 0.80645`, usage
124 %. During a strike: `{0, 2500, 0.0f, 0, 135}`.

## 5. Messages posted

| Message | When | Data |
|---|---|---|
| `0x69247DC7` TrafficMapChanged | end of CreateFinalMap (state 4) | `cRZMessage2Standard`: type, void1 and IGZUnknown = the simulator; **posted, queued** (message server `+0x10` = `MessagePost(msg, false)`, `0x0070F6C0`) |
| `0x0C05E4C3` traffic accident *[inferred name]* | `CheckForAccident` (§4.13), right after `CreateAccident` (`0x0071877E`) | new `cRZMessage2Standard` (`0x2C` bytes); `SetType(0x0C05E4C3)`, `SetData1(x)`, `SetData2(z)` (the accident-prone entry's cell, words +0 and +4); data3, data4, void1 and IGZUnknown are not set (0). **Posted, queued, normal priority** (`MessagePost(msg, false)` on `0x00B43CCC`, call at `0x007187D7`; skipped when the server is null), then released. Receivers: **none compiled**: the constant appears once in the image (`0x007187B2`, also only at the poster in the Mac build), so only wildcard or plug-in subscribers see it. Rail accidents post nothing (`CheckRailAccident` returns a bool). The advisor events CAR_CRASH `0xABF4B4B2` / TRAIN_DERAIL `0xABF4B57F` have no code reference to this path *[not traced]* |
| `0x69247DC3`, `0x69247DC4`, `0x69247DC5`, `0x69247DC6` | strike transitions (§4.12) | type, data1 (`+0x30` void1) and IGZUnknown = the simulator; posted, queued, through the simulator's message server (`+0x10` = `MessagePost(msg, false)`, `0x0070DD18`) |
| `0x69247DC8` | a cell becomes damaged or repaired (§4.12) | x, z, damaged flag |
| `0x2C06BA82` | the ferry pass of state 19 has finished all terminals | traffic-network.md §5 |

## 6. Persistence

### 6.1 Record (`Write` `0x00712400`, `Read` `0x00721990`) *[compat]*

Version **0x29** (41). Read accepts versions ≥ `0x13` and returns false below that; the version gates are listed
in §6.4. Primitive encodings are foundations §2.2.

**A failed Read** (TS30). The original does not parse first. Its only early exit is the version test: below `0x13`
it returns false having consumed only the version, and nothing has changed. Otherwise it writes into the simulator
as it goes and never stops on an error:

- each failed stream read leaves its destination unchanged;
- each ref (the grids, the switch buildings, the departments) is first set to null by the ref helper
  (`0x005D9730`), which does not release the old object, so on an error the ref stays null;
- counts are read into locals, some never initialised (the destination-map dimensions, `0x00721ACA` and
  `0x00721ADA`), so after an error the map is resized to, and loops run over, garbage counts.

At the end it returns `!stream.GetError()` (stream slot 18, `0x007235E5`), and the class-store loader ignores that
result (foundations §3.5). A failed Read therefore leaves a partly loaded simulator with null grids, which is
unusable. *[choice]* FreeCity4 parses the whole record first and commits only on success; a failed Read returns
false and leaves the simulator as it was before the call.

**Corrected (M6):** the authoritative byte layout, with every Read version gate, is save-records.md §7.1.1; it
parses all 417 corpus records. The table below is kept as a field overview; where it differs, save-records.md
wins. The differences (§7.1.2 there): the grids (rows 3, 5, 6, 9), the switch building (row 10) and the
departments (row 11) are **refs** (ref writer `0x005D99A0`, 14 calls in `Write`), not inline objects; row 14 holds
a fixed `4 × u32` and a `3 × D` u32 table with no count fields; row 17 is the **tunnel table**, not the
neighbour-connection records; in row 18 the `u32 4` is a count; in row 19 the usage table is a map from u16 to a
property holder, the two per-travel-type vectors are interleaved pairs, and the link counters come after the route
vectors.

| # | Field | Encoding |
|---|---|---|
| 1 | version | u16 = `0x29` |
| 2 | dimension counts | s32 13, s32 1, s32 13, s32 9 (network types, —, network types, travel types *[inferred: names]*) |
| 3 | grids | air-polluting, peak volume, commercial traffic, commercial work, trip length, congestion (6 grids) |
| 4 | destination map | u32 `W + 2`, u32 `H + 2`, then `(W+2)(H+2)` u8 in `[x][z]` order (x outer) |
| 5 | volume grids | u32 count (6), then each grid |
| 6 | float grids | `0x69D5C3FA` grid, peak ratio grid |
| 7 | edge densities | u32 count; per record: u32 key, `last[16]` u16 (index order 0…15), u16 usage, `current[16]` u16 |
| 8 | per trip type k = 0…2 | u32 `jobCapacity[k]`, u32 `attempted[k]`, u32 `routed[k]` |
| 9 | network grids | transit-switch mode-mask grid (`0x69D5C424`), transit-switch ID grid (`0x69D5C42D`) |
| 10 | switch block (traffic-network.md §2.4, §6) | u32 next switch ID, u32 count, then per switch in table iteration order: u32 ID, building reference (`SetGZSerializable`), f32 load, f32 time factor, f32 capacity, bool has-access-point, u8 access x, u8 access z, f32, f32 (the two counters), u32 rule count, per rule: u8 kind, 4 × bool side, u16 from, u16 to; then f32 toll (the `0x0BFC0A4C` value), bool counts-as-intersection |
| 11 | departments | roads, mass transit (serializables, budget.md) |
| 12 | income | s64 budget-item total (§4.1.3), f32 transit income, f32 toll income, f32 last monthly income, f64 income accumulator, f64 fee accumulator |
| 13 | cycle date, strike, damage | u32 cycle start date (§4.12 StartRecalc), u8 strike state, s32 strike month counter; road: u32 cell count, u32 damaged count, f32 damage rate; u32 n + n × u16 intact road cell keys, u32 n + n × u16 damaged road cell keys (hash iteration order); rail: the same five fields and two key lists |
| 14 | cycle position | u8 cycle state, the scanning iterator (helper `0x005D9FD0`/`operator<<`), bool intersection-dirty flag, s32 × 4 intersection dirty rect, bool network-changed byte (§3.1; never read), u32 strategy bucket counter, u32 strategy bucket, s16 × 2 phase positions (the saved source or lot cell x, z, §3.5), the lot iterator position (helper `0x005DA390`), bool, bool, f32, s32 x, s32 z (re-route context), u32 n + n × u32, u8, u32, s32 × 4, u32 n + n × u32 (per entry), u8 × 2 (context flags) |
| 15 | pacing and counters | s32 processed, s32 day index, s32 total, u32 days, u32, u32, u32 (unused), s32 `passTotal`, s32 `toReRoute`, s32 `toSearch`, then u32 × 10 pass counters (§10 Q4); corrected, see below (TS60) |
| 16 | network-type statistics, two sets (published "last" set = slot 34 block, then the working set) | per set: 13 × (f32 average (the working set holds the sum), u32 congested count (`r > 1`), s32 max x, s32 max z, f32 max), the bridge row (same 5 fields, §4.9), u16 × 13 (network-info records per type; published = the cell counts), u16 (bridge records; never published), u16 (published set: the pothole count, §3.6), s32 12, 12 × (u16, f32, s32, s32, f32) per-switch-type records, f32, f32, bool, f32, u8 |
| 17 | neighbour-connection records | u32 count, per record (traffic-network.md §2.7): u16, u8, u8, u16, u16, then its f32 values |
| 18 | commute | f32 `avgCarCommute`, f32 `carLenSum`, u32 `carLenCount`, f32 `freightLenSum`, u32 `freightCount`, u32 × 13 cells per network type, u32 4, u32 × 4 `freeDest` |
| 19 | tail | u32 count + records of the neighbour pool map (per pool: u16 ID, then its property-holder record; traffic-network.md §2.7.3), the per-travel-type count vectors (u32 × 9 each) and the ferry total, then the **ferry terminals**: u32 count, per terminal u8 × 6, u8, u16 × 3, u8 × 2 (kind, cell, counters), u32 link count + per link (f32 cost, s16 x, s16 z, u16, u16 × 4 counters, route bytes `vector<u8>`), u32 neighbour-link count + per link (f32, s16 × 4, u16 × 4, two route vectors); Read rebuilds the terminal tables from them (`0x00713DF0`) *[inferred: field grouping]*; traffic-network.md §6 agrees |

**Named fields of rows 14, 15 and 19** (TS56, TS59, TS60; member offsets of the simulator object; `Write` and `Read`
run on the object `+8`, so their listings show each offset 8 lower):

- Row 14, re-route context `s32 x, s32 z` (`+0x83C`, `+0x840`): the cell of the traffic source being worked, written
  as each source is taken in 16, 17 and 18 (`0x0071E26D`/`0x0071E284`, `0x0071D104`/`0x0071D119`,
  `0x0071D523`/`0x0071D538`), the s16 phase positions widened. Entering 16–18 (and 15) resets the phase positions to (0, 0) but not
  these, so between trip phases, and in 15 before its first lot, they hold the last source's cell while the positions are (0, 0) (TS67).
- Row 15, in record order: `processed` (`+0x914`), **`dayIndex`** (`+0x918`, the s32 right after `processed`; it
  counts the days of the running trip phase, so with `days` 1 or 2 it toggles 0/1, §3.3), `total` (`+0x91C`),
  `days` (`+0x92C`), then three u32 (`+0x930`, `+0x934`, `+0x938`) that no code reads or writes besides Write and
  Read (the constructor zeroes `+0x934` and `+0x938`, not `+0x930`), then `passTotal`, `toReRoute`, `toSearch`, and
  the ten pass counters `+0x948`…`+0x96C` of §10 Q4. *(Corrected: save-records.md §7.1.1 called the fifth field
  `dayIndex`.)*
- Row 19, the per-travel-type pairs: for `i` = 0…8, `published[i]` (`+0x424`) then `building[i]` (`+0x430`)
  (`0x00713370`–`0x00713398`; Read the same, `0x00722FD0`). After the end of FullTrips the pair is (this pass, previous
  pass), because the vectors are swapped (§4.4); after entering 18 the second element is 0.

Rows 14–19 are partly given by type sequence; the meaning of each unnamed scalar is either traffic-network.md's or an
internal counter that FreeCity4 writes back unchanged. The full write order with addresses is in the evidence file named in
§10 Q5. FreeCity4 writes version `0x29` only.

**The scanning iterator in row 14** (TS31). Read replaces the iterator with a new one (`0x00722623`; the old one is
not freed) and fills its eight fields from the record without any check (`0x005DA410`). The scanning phases step
it with an inline GetNext (`0x0071A1A5`, `0x0070F514`, `0x0070E585`, `0x00711120`, `0x0070F771`, `0x00716240`) that
tests only two values: state byte 2 is done and 0 is fresh. **Every other byte, 1 and any unknown 3…255, acts as
running** (simgrid.md §7.4), so the scan steps on from the saved cell. The byte is not rewritten until the scan
ends (2) or the iterator is reset, so an unknown value is also written back by the next save. Each scanning state
resets the iterator to (0, 0) on entry (`0x00721308`), so a loaded iterator matters only for a save taken
mid-scan. Example: state byte 7, current (5, 3), step 1, rectangle [0, 63] × [0, 63]: the next GetNext returns
(6, 3) and the scan goes on; a byte of 2 would end the phase at once. *[compat]* FreeCity4 keeps the raw byte and
treats it in the same way.

### 6.2 What must survive a save

The six published grids (consumers read them right after load), the volume grids (the next CreateMaps uses them),
the trip-data lists (in the lots, §6.3), edge densities (traffic automata and the query tool), income accumulators
and monthly income, `freeDest`, the destination map, damage sets and strike state, the transit switches, the ferry
terminals, and the cycle position. Everything else is rebuilt within one
cycle.

**Variants in the record** (TS53). The only variants are inside the pool property holders of row 19. Each pool is a
plain `cSCPropertyHolder` (the map is `map<u16, cSCPropertyHolder*>`); Write writes its u16 key and then calls the
holder's own `Write` (holder slot `+0x5C`, `0x00713324`), which writes the plain holder record of foundations §10.2,
each property through `WriteVariant` (foundations §9.2): u32 marker 0, u32 disk tag, then for an array a u32 count
and the elements. The traffic code puts one property on a pool, `0x8A0B4100` (§4.2), as a u16 array (disk tag
`0x8002`) or, once promoted, a u32 array (`0x8003`). A writer that produces that form for those two types produces
the original's bytes. Other properties appear only if some other code adds them to a pool holder; none is known.

**Hash container order after Read** (TS54). Read inserts into the freshly constructed containers, each built with a
hint of 100, so 193 buckets (`0x00829BC0`, prime list `0x00A829B0`: 53, 97, 193, 389, 769, …). For each key in file
order: grow to the smallest listed prime ≥ count + 1 when that exceeds the bucket count (`0x00710DD0`; a grow walks
the old buckets in ascending order and moves each chain's nodes from its head, each to the head of its new chain),
then insert the new node at the **head** of its chain (`0x0060D270` for the four damage sets, `0x0072249C`–
`0x007225FB`; `operator[]` `0x006D7820` → `0x006D6FD0` for the edge densities, `0x00721DD8`). Iteration is bucket
ascending, chain order. The order matters in two places: ApplyPotholeChanges picks the `r`-th element of a damage
set in iteration order (§4.12), so the cells that get potholes depend on it, and the next Write lists keys in it.
Nothing else depends on the order of these containers (the edge-density passes of §4.8 treat each record alone).

### 6.3 Trip data in the lot record

Each lot record (`cSC4Lot::Write` `0x006C0080`) ends with: u32 trip count, the trips, then u8 strategy byte. Each
trip (`cSC4TripData` Write `0x0070E0B0` / Read `0x0070E160`): u32 version (2), u32 n + n × u8 route, u32 n + n × u8
returnRoute, u32 count, u8 type, u8 strategy, u8 flags, u8 status, s16 destX, s16 destZ, f32 tripLength, u32
travelTypesUsed (only when version > 1; 0 otherwise). Read accepts any version; for a vector count ≥ 1,000,000
nothing more is read and the vector keeps its previous contents (corrected, `0x005DB520`) *[compat]*. The lot's
Read **appends** the trips to its existing list, and its Write stops before the strategy byte when a trip Write
fails (`0x006C35F0`, `0x006C0334`; lot-records.md). Fields the original never initialises are saved as they are:
`travelTypesUsed` is `0xCCCCCCCC` in 126 of 8,109 saved trips and destZ holds −13,108 (`0xCCCC`) in some, so a
writer must copy loaded values back unchanged to round-trip a city. Neighbour connections save their lists the same
way (neighbors.md).

### 6.4 Version gates (Read)

The reader branches on the version at these points (`m5sim/read_trace.txt` evidence): `0x18`, `0x25`, `0x1C`, `0x1C`
(the dimension counts, defaulting to 8, 1, 4, 7 for older saves), `0x14`, `0x1D`, `0x17`, `0x1C`, `0x20`, `0x15`,
`0x1A`, `0x19`, `0x27`, `0x23` (×2), `0x25` (×2), `0x22`, `0x14` (×2), `0x1A`, `0x26`, `0x1D`, `0x20`, `0x25`,
`0x23`, `0x1E`, `0x23`. FreeCity4 reads **every version from `0x13` to `0x29`** with these gates (corrected: 403 of
the 417 corpus saves hold version 19; the gate-by-gate layout is save-records.md §7.1.1) *[compat]*. Skipping an
older record is not an option: the six published grids are separate objects that would be left unreferenced, and
the destination map, damage and switch state would be lost. An old record is written back as version `0x29`; a
byte-exact round trip is expected only for versions 40 and 41 (save-records.md §7.1.4).

**Versions above `0x29`** (TS32). Read has no upper bound. Every gate, including the three that hold the constant
in a register (`0x007223B6`, `0x0072290F`, `0x00722C4D`), is an unsigned ordered test of the u16 version (`jb`,
`jae` or `jbe`); none tests for equality. A record of version `0x2A`…`0xFFFF` is therefore read exactly as version
`0x29`. Bytes left at the end of the entry are skipped by the class-store loader, which moves to the next entry
(foundations §3.5), and Write writes it back as `0x29`.

## 7. Interface: cISC4TrafficSimulator (vtable `0x00AB33D0`, 44 slots)

All slots `__thiscall`. "pop" = bytes popped by the callee (checked with `tools/abi_extract.py`, docs/abi
interfaces.tsv); return channel read from the epilogues. AL = bool/byte (upper EAX garbage); x87 = ST0.

| Slot | Off | Address | Signature (as compiled) | pop | Return | Notes / SDK |
|---|---|---|---|---|---|---|
| 0 | +0x00 | `0x0070D7E0` | `bool QueryInterface(uint32, void**)` | 8 | AL | §1 |
| 1 | +0x04 | `0x004F7BA0` | `uint32 AddRef()` | 0 | EAX | shared |
| 2 | +0x08 | `0x0070E2F0` | `uint32 Release()` | 0 | EAX | §1 |
| 3 | +0x0C | `0x0071E7E0` | `bool Init()` | 0 | AL | |
| 4 | +0x10 | `0x00719290` | `bool Shutdown()` | 0 | AL | |
| 5 | +0x14 | `0x0071FFA0` | `uint32 GetSimulatorType()` | 0 | EAX | `0x88027F0D` |
| 6 | +0x18 | `0x0070D860` | `bool CreatePathFinder(cISC4PathFinder**)` | 4 | AL | pathfinder.md §1 |
| 7 | +0x1C | `0x00711610` | `bool SetupPathFinderForLot(cISC4PathFinder*, cISC4Lot*)` | 8 | AL | §4.4 |
| 8 | +0x20 | `0x006AAC50` | `cISC4SimGrid<uint8>* GetAirPollutingTrafficMap()` | 0 | EAX | folded getter |
| 9 | +0x24 | `0x0080BC00` | `cISC4SimGrid<uint8>* GetCommercialTrafficMap()` | 0 | EAX | tract 4 |
| 10 | +0x28 | `0x00698CF0` | `cISC4SimGrid<uint8>* GetCongestionMap()` | 0 | EAX | |
| 11 | +0x2C | `0x0070E870` | `float GetBackgroundTraffic(int32 x, int32 z)` | 8 | x87 | **SDK error**: declared `intptr_t` |
| 12 | +0x30 | `0x00711460` | `TrafficEdgeDensity* GetTrafficEdgeDensity(uint8 x, uint8 z, uint32 travelType, bool evening)` | 16 | EAX | §4.8; never null |
| 13 | +0x34 | `0x00820250` | `cISC4SimGrid<uint8>* GetTripLengthMap()` | 0 | EAX | |
| 14 | +0x38 | `0x0071FFB0` | `float GetTripScale()` | 0 | x87 | = `commuteMaxTime` |
| 15 | +0x3C | `0x0071FFC0` | `float GetTripScaleForDisplay()` | 0 | x87 | = display multiplier (25 shipped) |
| 16 | +0x40 | `0x00720070` | `bool IsRoadDamaged(int32 x, int32 z)` | 8 | AL | §4.13 |
| 17 | +0x44 | `0x00711590` | `bool CheckRailAccident(int32 x, int32 z)` | 8 | AL | RNG draw (§4.15) |
| 18 | +0x48 | `0x0071FFD0` | `float GetLastMonthlyIncome()` | 0 | x87 | tolls + transit (§3.3) |
| 19 | +0x4C | `0x0070DF70` | `void SetMaxTripCapacity(cISCPropertyHolder*, uint32 k, uint32 value)` | 12 | none | **SDK error**: declared `bool`; EAX is garbage |
| 20 | +0x50 | `0x0070DF90` | `uint32 GetMaxTripCapacity(cISCPropertyHolder*, uint32 k)` | 8 | EAX | §4.2 |
| 21 | +0x54 | `0x0070E050` | `uint32 GetTrafficArrived(cISCPropertyHolder*, uint32 k)` | 8 | EAX | arrived **last** cycle |
| 22 | +0x58 | `0x00720010` | `cISC4Lot* GetDesiredLotInsertionPoint()` | 0 | EAX | traffic-network.md; **SDK**: `uint32` is a pointer |
| 23 | +0x5C | `0x00710320` | `float GetCapacity(uint32 transitNetwork, int32 x, int32 z)` | 12 | x87 | `1 / invCapacity` or 0.0; **SDK error**: declared `uint32_t` (and the first argument is the transit network 0…5, not a network type) |
| 24 | +0x60 | `0x007103B0` | `float GetTravelTimeRatio(int32 x, int32 z, uint32 travelType)` | 12 | x87 | §4.8 |
| 25 | +0x64 | `0x00710370` | `uint32 GetConnectionCount(uint32 transitNetwork, int32 x, int32 z)` | 12 | EAX | byte value; traffic-network.md |
| 26 | +0x68 | `0x00718BB0` | `void GetTravelStrategyPercentages(uint32 wealth, std::vector<float>* out)` | 8 | none | **SDK error**: declared `bool` with `SC4Vector<SC4Percentage>` **by value**; the game passes a pointer to an STL vector (allocator B) and EAX/ST0 are garbage |
| 27 | +0x6C | `0x0071FFE0` | `float GetFreightScalingFactor()` | 0 | x87 | |
| 28 | +0x70 | `0x00714460` | `bool GetTransitSwitches(list&)` | 4 | AL | traffic-network.md |
| 29 | +0x74 | `0x00713B70` | `GetFerryRouteBetweenTiles(…)` | 16 | EAX | traffic-network.md |
| 30 | +0x78 | `0x00718C40` | `GetAllFerryRoutes(list&)` | 4 | AL | traffic-network.md |
| 31 | +0x7C | `0x0071C250` | `GetFerryRoutesInUse(list&)` | 4 | AL | traffic-network.md |
| 32 | +0x80 | `0x0071FFF0` | `uint32 GetFerryTerminalCount(uint32 type)` | 4 | EAX | |
| 33 | +0x84 | `0x0070EC10` | `GetWaterRoute(…, vector<uint8>&)` | 20 | AL | traffic-network.md |
| 34 | +0x88 | `0x00720000` | `const TrafficStats* GetTrafficStats()` | 0 | EAX | pointer to the published statistics block (§4.9), simulator `+0x1E4`; the block is **0x23C** bytes (last field the u8 at `+0x238`, 4-byte aligned; the next member is at `+0x420`; the working set has the same layout at `+0x440`; constructor `0x0071FDD3`–`0x0071FECF`; TG5); **SDK**: `intptr_t` is a pointer |
| 35 | +0x8C | `0x0071CB80` | `bool GetTransitSwitchQueryData(uint32 x, uint32 z, TransitSwitchQueryData&)` | 12 | AL | §4.16 (five 32-bit fields); target lookup traffic-network.md |
| 36–41 | +0x90…+0xA4 | `0x00710540`, `0x00711A70`, `0x007106C0`, `0x00711B50`, `0x00718D20`, `0x00718F90` | AreLotsConnected, GetConnectedOccupantCount, GetConnectedDestinationCount, GetSubnetworksForLot, GetSubnetworksInRectangle, GetSubnetworksInRegion | 8 each | AL / EAX | traffic-network.md |
| 42 | +0xA8 | `0x007143B0` | GetOccupantCountForAllSubnetworks | 8 | | traffic-network.md |
| 43 | +0xAC | `0x007215A0` | scalar deleting destructor `(uint32 flags)` | 4 | EAX = this | not in the SDK; bit 0 frees with allocator C |

Other SDK remarks: `GetTrafficEdgeDensity`'s `TrafficEdgeDensity` (0x42 bytes, usage at `+0x20`) is correct; the
travel type enum is 32-bit. `GetTripDataList` on `cISC4TrafficSource` returns an STL `std::list<cSC4TripData>`
(allocator B), not an `SC4List`.

## 8. Dependencies

Virtual calls (slot, offset, pop, return):

| Interface | Slot (off) | Method | pop | Ret | Used for |
|---|---|---|---|---|---|
| `cISC4City` | 93, 94 (+0x174, +0x178) | cell counts | 0 | EAX | Init |
| `cISC4City` | (+0x16C) | cell width (float) | 0 | x87 | speed tables (§4.14) |
| `cISC4LotManager` | 6 (+0x18) | `GetLot(x, z, bool originOnly)` (`0x0072B410`): false = any cell of the lot, true = only the lot's origin (lot slot 21); off-map → null. **false** at every site except ClearTripDestinations, RefreshCommuteMap's first lookup and UpdateSubnetworkInfo (true). The pathfinder's three sites pass false (pathfinder.md §8, TS19) | 12 | EAX | everywhere |
| `cISC4LotManager` | 15 (+0x3C) | lot list / iterator | | | source iterator, FinishTripMap |
| `cISC4Lot` | 3 (+0x0C), 12/13 (+0x30/+0x34), 14 (+0x38), 21 (+0x54), 28 (+0x70), 31 (+0x7C), 37 (+0x94), 48 (+0xC0), 56 (+0xE0), 59 (+0xEC) | property holder, flags, bounding rect, location, road access, zone type, livable, building, travel population, lot link | | | §4.1, §4.7 |
| `cISC4TrafficSource` | 3, 4, 5 | §4.1 | 0, 0, 4 | | |
| `cISC4NeighborsSimulator` | 11 (+0x2C), 13 (+0x34), 16 (+0x40) | FindConnection, `FindConnectionOnOtherSide(conn)` (`0x006CDFA0`, pop 4; the caller reads the result's properties, TS38), GetAllConnections | | | §4.1.4, §4.6 |
| `cISC4Simulator` | 6 (+0x18), 8 (+0x20), +0x4C | GetSimDate (5 pointers), GetSimDateNumber, AddAgent | | | §3.3 |
| `cISC4BuildingDevelopmentSimulator` | 32 (+0x80) | GetDeveloper(group) → developer slot 15 grid → GetCellValue (s16) | | | §4.9 |
| `cISC4ResidentialSimulator` | 30 (+0x78) | GetCellWorkforcePercent(x, z) | | x87/hidden | §4.1.1 |
| `cISC4OrdinanceSimulator` | 8 (+0x20), 14, 15, 20 | IsOrdinanceOn, count, array, by ID (+ ordinance slot 16 misc properties) | | | §4.1.2, §4.5 |
| `cISC4DepartmentBudget` | 10 (+0x28), (+0x50) | funding fraction, GetLineItem | | | §3.3 |
| `cISC4LineItem` | 15 (+0x3C) | SetIncome(int64) | 8 | none | §3.3 |
| history warehouse | 12 (+0x30) | add point (id, u32) | 8 | | §4.12 |
| message server | (+0x10) | send message (msg, 0) | | | §5 |
| `cISCPropertyHolder` | (+0x24) | GetProperty(id) | | | §4.2 |

Globals: city `0x00B43CEC`, lot manager `0x00B43D08`, simulator `0x00B43D5C`, BDS `0x00B43D68`, neighbours
`0x00B43D8C`, ordinance `0x00B43D90`, budget `0x00B43D64`, history `0x00B43DC0`, resource manager `0x00B43CA8`.

Pathfinder: every slot listed in pathfinder.md §7 that §4.4 names, plus the direct field writes of §4.4.

## 9. Replaceability

**Replacement unit** *[choice]*: the traffic simulator and the pathfinder (pathfinder.md) are one unit; each reads and
writes the other's members directly.

* **Raw readers of this class's memory** (simgrid.md §6.4): the pathfinder reads the simulator's members directly
  through its owner pointer (cell connection array, destination map and `freeDest`, volume-grid vector, transit
  switch grid, strike state, per-travel-type tables, start-cost arrays, `MaxDistanceOriginToNetwork`,
  `NearestDestinationAttractiveness`, the class statics) and writes the destination map, `freeDest` and the income
  double (pathfinder.md §8). The simulator in turn writes pf2's fields directly (§4.4). The two classes are one unit:
  **neither can be replaced alone** (pathfinder.md §9).
* Other modules use only the interface: grids through slots 8–10, 13 (growth developer, pollution, demand, aura, land
  value, map views), edge densities through slot 12 (traffic automata, query tool; the records are read as the
  33-u16 layout), capacities through slots 19–21, strategy percentages through slot 26 (lots), the pathfinder
  through slot 6.
* Lots (`cSC4Lot::UpdateTrafficVolume`) and neighbour connections are the other half of trip generation: a
  replacement simulator relies on their trip lists (allocator B) and must leave their format intact.
* Internal-only containers: the bucket multimap, accident-prone lists, edge-density and transit-switch hashes (but
  the record layout of slot 12 is public), per-type vectors, the source iterator's list (allocator A).

### 9.1 Facts for a replacement in game (TG1–TG22, re-answers10)

The traffic glue lane asked these after its phase 1. Each item gives the original's rule; where the original has no
behaviour (a glue-only case), it says so.

**What the simulation reads from the game (TG2).** The FreeCity4 host callbacks are named by the glue, not by the
original, so this list goes by what the original reads. Everything in the first table changes routes, volumes,
money or trips, so a replacement that leaves it out differs from the original in its maps and its budget.

| Original read | Where | Needed for |
|---|---|---|
| Occupant manager slot 41 (`GetOccupantsByStandardCityCells`, typed, network occupants `0xC772BF98`) over a rectangle | DoConnectionsChanged step 3 (traffic-network §4.2), lane continuity step 2 (§4.4), switch and ferry-terminal activation (§4.7.1, §4.9.1) | every rebuild of the cell graph: each occupant in the grown rectangle is re-read, not only the one in the message |
| Network occupant slot 53 `PathConnection(pathType, entry, exit)` and slot 49 `GetPathInfo`, then path info slots 4 (turn-around), 14 and 3 (car paths by entry, first vertex) and 7 (paths near a point) | traffic-network §4.2, §4.4 | the connection bits, the turn-arounds on dead ends and the lane-continuity masks next to elevated and highway pieces; routes change without them |
| The paired portal of `cSC4NetworkTunnelOccupant` (a member of the concrete class) | traffic-network §4.8 | tunnel records; with no pair no record is made and the tunnel carries no trips |
| The lot's own strategy byte (`cSC4Lot` + `0x88`, that is traffic source + `0x6C`, read at `0x006C2986`; no interface getter) | §4.1.1 | which single strategy a residential lot with fewer than 50 commuters uses; only needed when the replacement computes the lots' trip targets itself instead of calling the lot's own `UpdateTrafficVolume` (traffic source slot 4) |
| The neighbour connection's volume-dirty byte (connection + `0x40`), tested and cleared | §4.1.4 (neighbors.md NG8) | the trip reconcile of a neighbour connection: it runs **once per connection object** (after creation, or after the load that created it). Treating the flag as always clear means no neighbour trips; treating it as always set re-syncs a connection whose other side was missing at its first sync, which the original never does |
| The roads and mass-transit departments' cost update (`UpdateRoadsBudget` `0x0070D8A0`, `UpdateMassTransitBudget` `0x0070DA40`; exact rule §4.9.1, TG23) and the monthly funding reads (`ApplyRoadFunding`, `ApplyMassTransitFunding`, `UpdateStrikes`) | §3.5 step 14, §3.6, §4.9 step 3, §4.9.1, §4.12 | the transport line items of the city budget, potholes and strikes |

Not needed for the simulation:

| Original read | Why not |
|---|---|
| The wall clock for the 4 ms slice deadline (§3.3) | replaced by the unit budget (§3.4) |
| The accident check's visible creation can be omitted only by a replacement that does not claim exact equivalence | **TE33-ACCIDENT-1 correction:** exact replacement requires the unit-5 stopwatch read and scheduling, the CRT iterator-reset draw and the simulator-generator draws (§4.13.1), plus a valid view rectangle and network-info/occupant provider. Omitting this check changes both shared randomness and the fixed schedule's counter-read clock. Omitting only creation also differs in controller/automata state. The earlier permission to omit the entire check lost more than visible crashes and is withdrawn. The native manager call additionally requires the concrete boundary of automata-managers §3.4.1. |
| The occupant filter of pathfinder filter 2 | filter 2 is unreachable (pathfinder.md §4.10.3, Q2) |

**TG1, a route deque.** The original fills it (pathfinder.md §4.13 step 5, §4.12 fallback), and the traffic
automata, emergency dispatch, MySim dispatch and the query tool depend on it (pathfinder.md §9). A `FindPath` that
returns 0 when a deque is set therefore **differs**: those callers get no route. The deque layout is in
pathfinder.md §7.

**TG3, trip records.** Four parts of the game read the trips from the lots' and connections' own lists
(`GetTripDataList`, traffic source slot 3 = `lot + 0x84` as an STLport `std::list<cSC4TripData>`, allocator B), in
the `0x2C`-byte layout of §2:
- the lot's `Write` and `Read` (§6.3, `0x006C0080`, `0x006C35F0`) and the neighbour connection's;
- the query tool's route display (`cSC4ViewInputControlQuery::ShowAllRoutesThatIntersectCityRect` `0x004CA460`
  walks the lot list, then each lot's trip list through lot slot 4 and source slot 3, `0x004CA67C`–`0x004CA6BF`;
  `MakeRoutePath` `0x004C5E20` reads `route`, `returnRoute` and the reversible flag at `+0x1E`) and
  `CalculateTripVolumeFromTrafficPaths` (`0x004CFD20`);
- `SetupPathFinderForLot` (slot 7, §4.4), which reads the list of the lot it is given.

A private record type is acceptable only if those lists still hold the trips in this layout whenever one of those
readers runs: copied in from the lists at load (after the lots' `Read`), written back before the lots' `Write`, and
kept current for the query tool. Otherwise a save loses every trip (the next load starts with status 0 everywhere,
so a whole cycle of full searches) and the route display shows nothing.

**TG5, slot 34.** 0x23C bytes (§7).

**TG6, slot 22.** Returns the lot iterator only while the cycle state is 1, else null (`0x00720010`). No transition
leads to state 1 (§3.2) and DoPostCityInit turns a loaded 1 into 0 (§3.5 step 11), so null is exact except between
the `Read` of a foreign save holding state 1 and PostCityInit. Returning null matches.

**TG7, departments.** `CreateDepartmentBudget` returns an existing department unchanged (budget.md §7), so "get,
else create" is the same thing. The original's calls, made **only when the member is still null** after Read
(roads `0x0071F1AF`, mass transit `0x0071F263`): `CreateDepartmentBudget(id, group 0xAA369059)` (budget simulator
slot 25), `AddRef` (the simulator keeps a reference), `SetDepartmentName(0x6A3FF01C, name)`,
`SetMaxAllowedFundingPercentage(&maxFunding)` (§4.14, TS29), then the line items:

| Department | Name | Line items created with it (`CreateLineItem(i, false)` then `SetName(0x6A3FF01C, n)`) |
|---|---|---|
| Roads `0xC921CEEA` | `0x4A493B73` | 1: `0xEA8446BE`; 2: `0x2A8446DA`; 3: `0x2A8446DF` |
| Mass transit `0xC921CEEB` | `0xEA493B83` | 1: `0xEA5F96E7`; 2: `0x8A5F978C`, **type Income** (`SetType(1)`); 3: `0x6A8B0CFB` |

Then, on every Init, loaded departments included, each of these line items is created only if `GetLineItem(i)`
returns null (`0x0071F336`–`0x0071F45E`, in this order): mass transit 4 `0x2AB7EA93`, mass transit 5 `0x8BCE33E0`,
roads 4 `0xCBCE33D5`, mass transit 6 `0x8BCE33E1`, roads 5 `0x0C3BF3C4` **type Income** (tolls), roads 6
`0xEA5F96E7`. A loaded department keeps its saved maximum funding (no `SetMaxAllowedFundingPercentage`). The bracket
(group 0, no names or types) **differs** in the group, the names and the two income lines; the budget window and
the income totals show the difference.

**TG8, `GetLanguageUtility`.** Borrowed: the language manager returns a pointer to its own member
(`0x00924B50`), and `FindHighwayReturnTile` (`0x0070DD30`) neither AddRefs nor releases it. Matches.

**TG9, budget items on activation.** As §4.1.3 step 3, with the arguments: `GetBudgetItemForPurpose(building
holder, purpose, &item)` (slot 51), then, when the lot was found, `CopyBudgetItemProperties(building holder, lot
holder)` (slot 58, source first) and `AddBudgetItemToBudget(lot holder, item.index)` (slot 55; the index is the
item record's `+0x18`, the cost its int64 at `+0x10`) (`0x00724000`–`0x007240DF`). Matches.

**TG10, lot order.** Only the master list through the lot manager's safe iterator (§4.1). The rectangle query has
no counterpart; as a fallback that the game never needs it is acceptable, but it must not run in normal play.

**TG12, a null department.** No original behaviour: the original never checks (DoMonthlySimulation calls the
department at `0x00718AC4` without a test), and a department is null only after a failed `Read`. 1.0 (full
funding) is accepted.

**TG13, slot 12.** The original returns a pointer into the edge-density hash node (`0x007114D4`), valid until that
record is erased (network occupant removed, §3.1), or the shared all-zero record `0x00AB31D0`. The record is live:
later Dump and Reduce passes change what the pointer shows. A ring of 64 copies is accepted as a choice: the values
are those at the time of the call, and the pointer must stay valid at least until the caller's next call.

**TG14, slot 29.** The original returns a pointer to the route vector stored in the ferry terminal's link
(traffic-network §4.9.5), valid until the terminal tables are rebuilt; the ferry pass of state 19 rewrites its
contents. A vector kept per tile pair is accepted if it holds the current route at each call, is an STLport
`vector<u8>` (allocator B) and is null in exactly the cases of the table there.

**TG15 and TG21 (slot 19), the capacity slots.** Slots 19, 20 and 21 use no simulator state at all: each reads or
writes property `0x8A0B4100` on the holder it is given (`0x0070DF70`, `0x0070DF90`, `0x0070E050`; §4.2). A direct
property read or write without the simulator's lock matches, on any thread; for slot 19 that is the normal path,
not a fallback.

**TG11, Shutdown.** As §1. The bracket matches.

**TG16, `AsTrafficSource`.** Borrowed (§4.1). Matches.

**TG17, `CreatePathFinder(null)`.** No original behaviour: the original writes through the pointer without a test
(`0x0070D897`) and would fault. Returning false is accepted.

**TG18, `SetupPathFinderForLot` with a pathfinder another simulator made.** The original configures the pathfinder
only through its interface (§4.4), so it would configure any `cISC4PathFinder`. Returning false differs, but the case
cannot arise when ours is the city's simulator, because every pathfinder then comes from our slot 6. Accepted;
configuring it through the interface would match.

**TG19, stale tokens.** Lots: a lot removed before the iterator reaches it is skipped, as in the original (the safe
iterator, §4.1). Connections **differ**: the original's connection list is a snapshot of counted references taken
at `Reset` (entering 16, 17 and 18), so a connection removed during the phase is still visited and its trips are
still routed until the next `Reset`.

**TG20, messages with an object.** Matches: only `0xE8BDA2E7` (the lot, from the message's IGZUnknown,
`0x007245E9`, QI `0xE818FCED`, released after the handler) and `0x99EF1142`/`0x99EF1143` (the occupant, from void1,
`0x00724317`, QI `0xA821EF94`) are read for an object. `0x08DAC3E7` reads only data2 and then the simulator's own
mass-transit department (`0x00724220`); the neighbour messages read data1–data3; the others read nothing.

**TG21, a busy lock.** The original handles every message on the delivering thread, with no lock, and drops none
(as neighbors.md NG33). Dropping a message differs whenever it happens; it is acceptable only if counted and
never seen in normal play.

**TG22, units per slice.** No original constant (TS27, §3.4). In the original the work of a trip-phase day is the
share `total/days` plus the overshoot of the last 4 ms slice, so the per-day cost of traffic is the share, whatever
the slice length; N changes only the overshoot. 16 is accepted; the turbo cost against a city without traffic is
the cost of the share, which the original pays too.

**TG23, the network cost update** (re-answers21). §4.9.1 gives the exact rule: SetFullExpenses (slot 12), never
AddToFullExpenses, on roads items 2, 1, 3, 4 and mass transit items 3, 4, 5, 6, with the count × tile cost rounded to
nearest, ties to even, and called unconditionally from five places (every road, rail and subway insert or remove,
every CreateMaps pass, and PostCityInit). Leaving the update out (the line items keep the expenses they were loaded
with) **differs**: in the original the budget window's road and transit maintenance follow the network within the
same day, and a city whose tuning changed is re-priced at PostCityInit.

**TG24, path info.** traffic-network.md §4.4 and §7.3 give the slots, arguments, return values and containers. The
path info returned by occupant slot 49 is **borrowed** (the occupant's cached member, returned without `AddRef`;
neither caller releases it). The turn-around test (slot 4) tests the whole of EAX: slot 4 returns a path pointer or
null, not a bool. Leaving lane continuity out **differs** next to occupants with network flags in `0x1F4F`.

**TG25, the paired portal.** The member at `+0x1D0` of `cSC4NetworkTunnelOccupant`, a counted reference to the
other portal (traffic-network.md §4.8). Leaving it out **differs**: no tunnel record, so tunnels carry no trips.

**TG26, the connection snapshot.** Released by the next `Reset` and by `Shutdown` (§1, §4.1); `Read` never touches
it. In the original a `Read` always comes before the `Init` that creates the iterator, so there is no snapshot to
release at `Read` time; releasing at a `Read` into a live simulator is a glue choice for a case the original never
reaches, and is accepted.

## 10. Quirks and open questions

Quirks reproduced: strategy byte 0 always picks strategy 0; percentage splits can lose or gain commuters; the
16-bit deficit for neighbour trips; uninitialised fields in new trips (FreeCity4 0); the free-destination count is
decremented even when the bit was already clear; bit 3 of the destination byte is never cleared by the refresh;
pf2's max trip length is a leftover; mode 2 mirrors edge densities (unlike `ApplyTraffic`); background traffic
lands one cycle late; a lot with one failed trip shows trip length 0; `avgCarCommute` divides by zero when nothing
was routed; the freight byte is not clamped; the final maps assume tract size 1; the congestion curve is inverted
again on a reload without the property; ordinance offsets accumulate without the exemplar; the 0x4A611773 value is
never read.

Open questions:

* **Q1** `UpdateStrikes` and `ApplyRoad/MassTransitFunding` were not traced line by line *[inferred]*; their
  observable outputs (strike state, potholes) need differential tests. *`UpdateRoads/MassTransitBudget` answered*
  (re-answers21, TG23): §4.9.1.
  (`DoLotDeactivated` is traced in §4.1.3, TS23.)
* **Q2** The meaning of the slot-59 lot link and of zones 8/9 in FinishTripMap (§4.7).
* **Q3** *Answered* (§5): `0x0C05E4C3`, data1 = x, data2 = z, posted queued, no compiled receiver; rail accidents
  post nothing. Open: who calls `CheckRailAccident` and what the caller does with a true result.
* **Q4** *Answered* (TS59). The ten pass counters of §6.1 row 15 (`+0x948` + 4·i), all saved; on entering 16 every
  one is zeroed except 1 and 2 (`0x00721857`–`0x00721881`):

  | # | Meaning | Increment points |
  |---|---|---|
  | 0 | routes confirmed without a search | ConfirmTrips, each status := 3 after ConfirmRoute (`0x0071DBF5`) |
  | 1 | re-route searches found | ReRouteTrips (`0x0071D289`); zeroed on entering 17 (`0x007218F2`), kept through 18 (TS35) |
  | 2 | `rerouteBucket` | StartRecalc `+1 & 3` (`0x00717CF4`) |
  | 3 | source counter | ConfirmTrips, once per source (`0x0071D93F`) |
  | 4 | destination full | ConfirmTrips, status 3 → 0 (`0x0071DC01`); ReRouteTrips, status 1 → 0 (`0x0071D221`) |
  | 5 | merges | ConfirmTrips merge, per routed trip that absorbed at least one trip (`0x0071DA0D`) |
  | 6 | return leg not confirmed | ConfirmTrips, outbound confirmed but `ConfirmRoute(returnRoute)` false (`0x0071DBA0`) |
  | 7 | status-0 trips met | ConfirmTrips, each status-0 trip of a source before its merge (`0x0071D95D`) |
  | 8 | failures retried | ConfirmTrips, status 5 → 0 (`0x0071DC11`) |
  | 9 | splits | FullTrips, per split (`0x0071D77F`) |

  Only `toReRoute`, `toSearch`, `passTotal`, `processed`, `dayIndex`, `total`, `days` and counter 2 drive behaviour;
  the others are statistics nobody reads (no reader outside Write besides the code above).
* **Q5** Full save layout: rows 14–19 of §6.1 are given by type sequence (write trace of `0x00712400`, kept in the
  helper's scratch folder `m5sim/write_trace.txt`); traffic-network.md must name the network-owned fields.
* **Q6** `0x0CAAAEB8` has no name in the tuning file; its only use is the commercial traffic scale.
* **Q7** Lot activation (`0xE8BDA2E7`, posted only by the virtual `cSC4Lot::SetState` `0x006C3100`) drives the
  strategy-bucket registration, the destination bits, the transit switches and the ferry terminals. Whether loading a
  city re-sends it for every lot is the lot spec's question (traffic-network.md Q2). The bucket multimap is not in
  the save record, but DoPostCityInit rebuilds it from the lots' saved `0x691C863C` values (§3.5 step 12, TS7); the
  destination bits come back with the next RefreshCommuteMap.
* **Q8** The int64 total `E` that gates strikes (§4.12) comes from budget items of activated lots (properties
  `0xCA550301` / `0x0C3BF549`); its exact meaning needs the budget items' owner.

Corrections to `docs/subsystems/traffic.md` (Mac-derived): the lot's trip list is at a different place on Windows
(the interface, not the offset, is the contract); new trips are distinguished by **status 4**, not a flag; property
`0x4A611773` is never read; the inverse speed table is a class static filled from tuning (the member copy holds the
speeds); `GetBackgroundTraffic`, `GetCapacity` and `GetTravelStrategyPercentages` have the signatures of §7.

## 11. Conformance tests

Shipped tuning unless stated. Exact values are float32 bit patterns where given.

1. **Cycle order.** From state 0 with an empty city, successive finished phases visit
   `0, 16, 17, 18, 15, 2, 4, 5, 11, 3, 7, 6, 10, 12, 13, 19, 8, 0` (12 skipped when no subnetwork rebuild is
   needed). `DoDailySimulation` returns true on entering 17, 18, 2 and 11.
2. **Day pacing.** `passTotal = 100` at the start of ConfirmTrips: `days = 5`; with `trafficUnitsPerSlice` = 1 the
   first day ends after the first source that brings `processed ≥ 20`. `toReRoute = 17` → `days = 1`. With 10
   sources of 4 trips each (same `passTotal = 100`, shares 20, 40, …) and N = 3: day 1 runs slices of sources 1–3
   (processed 12 < 20, continue) and 4–6 (24 ≥ 20, day ends); day 2 runs sources 7–9 (36 < 40, continue), then
   source 10, after which the phase finishes and the day ends on entering 17. With N = 1 day 1 ends after source 5
   (processed 20).
3. **Congestion scale.** Shipped curve: inverted points `(2, 0x3FC4EC4F)`, `(3, 0x40555555)`; `congestionScale =
   0x42DA924A` (109.28572).
4. **Congestion map.** Peak ratio `D` → congestion: 0.5 → 1; 1.0 → 1; 1.5 → `c = 0x3FA27628` (1.2692308) → 29;
   2.0 → `c = 0x3FC4EC4F` → 59; 2.5 → `c = 0x401BE5BE` (2.4358974) → 157; 3.0 → 255; 4.0 → 255. Peak volume 0 → 0.
5. **Air-polluting map.** Peak volume 150 → 15; 15.0 → 2 (1.5 rounds to even 2); 25.0 → 2 (2.5 rounds to even 2);
   2.0 → 0; 3000 → 255.
6. **Trip-length map** (`commuteMaxTime` 6): average 3.0 → 128 (127.5 rounds to even); 0.6 → 26; 1.5 → 64; 6.0 →
   255; 7.0 → 255 (298 clamped); a lot with any failed trip → 0.
7. **Strategy percentages.** `pct = (80, 20, 0)` → `GetTravelStrategyPercentages` = `0x3F4CCCCC`, `0x3E4CCCCC`, 0.
   Lot with 30 commuters: thresholds 81, 101, 101; strategy byte 80 → strategy 0, 81 → strategy 1, 0 → strategy 0
   even with `pct = (0, 50, 50)`.
8. **Large lot split.** 200 commuters, `(80, 20, 0)` → 160, 40, 0. 75 commuters with `$$` `(30, 0, 70)` → 22, 0, 52
   (total 74: 22.499998 rounds down, 52.5 rounds to even 52).
9. **Carpool.** Count 10 → car `Q = 8.0`; 7 → 6.0 (`r24(5.6) → ceil`); 3 → 3.0; 1 → 1.0.
10. **Hand-checkable network** (pathfinder.md §11 tests 2–3 city: 8 × 8, street `(0,3)…(4,3)` with road network info
    `invCapacity 0.0625`, connection count ≤ 3, lot A at `(2,2)` = source, end cell `(4,4)` = job lot B with
    `max[0] = 100`; route bytes `01 02 03 41 02 0A`). Trip `{count 40, type 0, status 3}` confirmed with
    `ApplyRoute(mode 0)`:
    * road volume `(2,3)`, `(3,3)`, `(4,3)` = 40.0 (`0x42200000`);
    * edge densities (key bit 24 = 0, type 1): `(2,3)` `current[4·1 + 2] = 40`, `(3,3)` `current[4·0 + 2] = 40`,
      `(4,3)` `current[4·0 + 3] = 40`;
    * lot B `arrivedThisCycle[0] = 40`; income 0 (car income per tile 0);
    * with mode 2 instead: volumes 80.0, the same counts in the bit-24-clear records and `current[4·2 + 1]`,
      `current[4·2 + 0]`, `current[4·3 + 0]` = 40 in the bit-24-set records of the same cells;
    * CreateMaps on `(3,3)`: `r = 2.5`, peak ratio 2.5, peak volume 40 → congestion 157, air-polluting 4
      (`round(4.0000001)`); volumes reset to 0 (`densityReduction` 0);
    * CompleteTripSourceCalculations for A with `tripLength 3.0`: average 3.0 → trip-length 128 at `(2,2)`;
      `attempted[0] = routed[0] = 40`; `carLenSum = 120.0`, `carLenCount = 40` → `avgCarCommute = 3.0`, history
      `0x8A5D4116 ← 75`.
11. **Background traffic.** `R = 100`, `C = 10`, `I = 0` at a road cell: `r24(r24(100 × 0.05f) + r24(10 × 0.2f)) =
    7.0` (`0x40E00000`), added to the road volume after the reset, so the next CreateMaps sees 7.0.
12. **Seeded draws.** Simulator generator seeded with 1 (foundations §6.1 test 14: first words `0xD3F6B9E5`,
    `0xEF284157`, `0x0D3639C7`): strategy re-draw for a bucket with lots L1, L2, L3 writes `0x4A611773` = 82, 93, 5;
    `RandomUint32Uniform(100)` continues 13, 78, 34. With seed 12345: 81, 12, 25, 77, 2, 59. Seed 1,
    `RandomDoubleUniform` on the first three words (`r24(r24(i · 2^-32) + 0.5)`, `i` signed): 0.32798350
    (`0x3EA7ED74`), 0.43420801 (`0x3EDE5083`), 0.55160868 (`0x3F0D363A`); with rail damage accident factor 1 every
    `CheckRailAccident` on a damaged cell returns true for these draws, with factor 0.5 only the first two do.
13. **Capacity.** `max[1] = 30`, arrived 25, trip count 5 → `CapacityStillAvailable` true; count 6 → false; after
    `ApplyTrafficToDestination(5, …, type 1)` arrived 30 → destination bit 1 cleared over the lot and `freeDest[1]`
    decremented.
14. **Neighbour reconcile.** Property `0xAA9EA81D` = `{0, 50, 0, …}` and an empty list → one trip `{count 50, type 0,
    strategy 1, flags 4, status 0}`; SetupTrip then allows only start type car (`SetTravelTypeFlag(0x1CF, false)`,
    `SetTravelTypeFlag(0x2, true)`).
15. **Save round trip.** Write then Read of a fresh simulator gives the same bytes; the header is
    `29 00 0D 00 00 00 01 00 00 00 0D 00 00 00 09 00 00 00`; a trip record with empty routes is
    `02 00 00 00 | 00 00 00 00 | 00 00 00 00 | count | type strategy flags status | destX destZ | length | mask`.
16. **Network cost update** (§4.9.1, TG23). Shipped tile costs, counts as the §4.9.1 worked example: after
    `UpdateRoadsBudget` the roads department's full expenses are item 1 = 23, item 2 = 55, item 3 = 60, item 4 = 6
    (items 5 and 6 unchanged); after `UpdateMassTransitBudget` the mass transit items 3–6 are 4, 18, 1, 1 (items 1 and
    2 unchanged). Ties: Street counts 10, 30, 50 give item 1 = 0, 2, 2; Street 15 and 25 give 1 and 1. Calling either
    function twice with the same counts leaves the same values (set, not added) and sends the department's
    `0x08DAC3E8` notification once per line item per call. With mass transit item 3 deleted beforehand, the call
    re-creates it as an Expense line named `0x6A3FF01C`/`0x6A8B0CFB` with full expenses `price(Rail)`.
17. **Snapshot release** (§1 Shutdown, TG26). Two neighbour connections, enter state 16 (`Reset` takes the snapshot:
    each connection's reference count rises by 1). Remove one connection from the neighbour simulator: its count
    stays raised and `GetNext` still yields it. `Shutdown`: both counts drop by 1. A `Read` between the `Reset` and
    the `Shutdown` changes neither count.

18. **Split continuation across returns (TS68, §4.4).** Script a commuter trip of count 10 whose first
    search arrives with 3, leaving `[status 3 ×3, status 0 ×7]`. That completed search adds 1 to processed;
    the split adds 1 each to toSearch, total and pass counter 9. Use these conformance cases (pathfinder
    outcomes are fixtures, not measured game routes):

    | Case | Interleaving after the split | Required observation |
    |---|---|---|
    | TS68-a | Deadline before the remainder; another call the same day | Continuation true across return; revisit list head and skip the status-3 trip; SetupTrip for count 7 sets flag 4. No source completion before the remainder. |
    | TS68-b | Same deadline, then next daily delivery | Same outcome as a; the day boundary does not clear continuation. |
    | TS68-c | Remainder's outbound search suspends, then resume | Status/count stay 0/7, continuation true, search state 1, processed unchanged by suspension; no second SetupTrip on resume, both work-time limits refreshed. |
    | TS68-d | Remainder's outbound succeeds but return leg suspends, then resume | Search state 2 and continuation true; destination/flags change as TS65 specifies, count/status stay 7/0, no processed increment yet; resume return leg without SetupTrip or another outbound search. |
    | TS68-e | Network change while the remainder is pending or suspended | Continuation false immediately; next source-loop visit resets both pathfinders and search state to 0; any ensuing idle commuter SetupTrip omits flag 4. |

    After a successful unsplit remainder, or a failed remainder, continuation is false. Also test two
    sources in one call: taking the second clears continuation before its first SetupTrip. For a phase
    transition, the entry action arms source-taking and clears search state; the first source starts
    with continuation false. These cases specify the original's return boundaries even when the
    replacement chooses atomic searches (§3.4).

## 12. Calibration (counters for the observer in the original)

Purpose: set the default of `trafficUnitsPerSlice` (§3.4) from the original's behaviour on the reference machine.
The observer logs, for every `WorkOnSimCycle` call (`0x00720F00`) made from `DoDailySimulation` (`0x007215C0`):

| Counter | Where to count in the original | Meaning |
|---|---|---|
| `state` | the simulator's current work-cycle state at the call's entry and exit | which phase the slice ran |
| `sources` | calls to `ConfirmTrips(source)` `0x0071D920` from ConfirmTrips `0x0071E1B0` (call at `0x0071E294`) | units of state 16 in the slice |
| `trips` | trips examined by ReRouteTrips `0x0071D020` and FullTrips `0x0071D440`: each passage through the deadline checks at `0x0071D1D9` and `0x0071D607`; searches: calls to `PerformPathSearch` `0x0071C6C0` (at `0x0071D257`, `0x0071D672`) | units of states 17 and 18 in the slice |
| `finished` | the return value of `WorkOnSimCycle` | the slice finished its phase |
| `suspended` | `PerformPathSearch` returning with the search incomplete (pathfinder work-time limit) | a trip split across slices (FreeCity4 has none) |
| `elapsed` | rdtsc at the call's entry and exit | slice length |

and, per game day (all `DoDailySimulation` calls with the same simulation date until one returns true): the
number of slices, `processed`, `total`, `days` and `dayIndex` before and after (§6, save row 15). The default is
the distribution of units per unfinished slice in states 16–18; log the city population and trip count with it,
because the cost of a unit grows with the network size.
