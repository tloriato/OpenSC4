# cSC4PathFinder: trip pathfinder (behaviour spec)

Status: M5 behaviour spec, verified on the Windows executable `bin/SimCity4.exe` (1.1.641, image base `0x00400000`).
Every address below is a Windows address. The Mac build (named pseudocode) was used for names and intent only; the
one behavioural difference found is called out in §4.13.

Related specs: [traffic-network.md](traffic-network.md) defines the per-cell network model that this class searches
(network info per cell, cell connection words, transit switches, tunnels, ferries, neighbour connections, the
destination bitmask map). [traffic-simulator.md](traffic-simulator.md) defines the callers (trip setup, route
application, the tuning that fills the speed tables, the congestion curve and the per-travel-type flags) and the
consumers of the compressed route. [foundations.md](foundations.md) §4 (response curves), §18 (x87 precision),
[simgrid.md](simgrid.md) §6.4 (grid ownership), [docs/abi/allocators.md](../abi/allocators.md).

Notation. *[compat]* = observable behaviour a replacement must reproduce. *[quirk]* = surprising original behaviour
that is reproduced. *[choice]* = FreeCity4 decision where the original is undefined. *[inferred]* = not proven.
`r24(e)` = the value of `e` rounded to float32 (foundations §18: the main thread runs the x87 FPU at 24-bit precision,
so every add, subtract and multiply rounds to float32 even when an operand is a double or an integer).

---

## 1. Role, identity, creation

`cSC4PathFinder` finds one route for one batch of trips: from a start rectangle (a lot, or one off-map neighbour cell)
over the city's cell graph, using a fixed set of travel types, to a destination chosen by a *destination filter*. It
is a modified A\* search with a binary-heap open set; the tie order of that heap is part of the contract because it
decides which of several equal-cost routes is taken, and therefore where traffic volume lands. *[compat]*

On success it reports the trip length, the end cell, the number of trips that were accepted by the destination, the
route (optionally as a deque of cells, and on request as a compressed byte string), whether the route can be driven
backwards, and it can add the trips' traffic to the traffic simulator's volume, edge-density, transit-switch and
income accumulators.

| Fact | Value |
|---|---|
| Interface | `cISC4PathFinder`, IID `0xAA0DB766` (QueryInterface also answers `cIGZUnknown`, IID `1`). No SDK header exists; `cISC4TrafficSimulator.h` only forward-declares the class. |
| Implementation vtable | `0x00AB2150`, 34 slots (§7) |
| Constructor | `0x006D89C0` (only caller: `cSC4TrafficSimulator::CreatePathFinder` `0x0070D860`) |
| Class ID | none. The class is not in the GZCOM class registry and is not a city component. |
| Object size | `0x3160` bytes |

**Creation.** The only way to obtain a pathfinder is `cISC4TrafficSimulator::CreatePathFinder` (traffic simulator
slot 6, `+0x18`, `0x0070D860`, pop 4, bool in AL, always true). It allocates the object with the global heap
(allocator C, `0x005E55E0`), runs the constructor, calls `AddRef` (count 0 → 1) and then `Init` through the vtable,
**then** stores the owning traffic simulator in the object, and writes the pointer to the caller's out parameter
(the reference from `AddRef` goes to the caller). The allocation result is not null-checked before the virtual calls.
`Init` therefore must not need the owner. Each pathfinder belongs to one traffic simulator for its whole life.

**Init** (slot 3, `0x006D78B0`): returns false if already initialised. Otherwise marks itself initialised, reads the
city size `W = CellCountX`, `H = CellCountZ` from the city (`cISC4City` slots 93 and 94, through the global city
pointer), sizes the two heuristic distance tables to `W + 2` and `H + 2` entries (index −1 … W and −1 … H), zero
filled, and returns true. Init does not allocate the node store or the open set (the constructor does).

**Shutdown** (slot 4, `0x006D5D50`): returns false if not initialised. Otherwise clears the initialised flag, calls
`ReleasePathNodes` through the vtable (slot 31), forgets the distance tables' base pointers (their storage is kept
until destruction) and returns true.

**Release** (slot 2, `0x006D61B0`): if the reference count is above 1, decrements it and returns the new count.
Otherwise (count 0 or 1) it does **not** decrement: if initialised it calls `Shutdown` (slot 4), then the scalar
deleting destructor (slot 33) with flag 1, and returns 0. The destructor frees the node pages, bucket array, heap
vector and distance tables and frees the object with allocator C (`0x005E5620`).

The constructor sets: reference count 0; not initialised; no result node; heuristic weight 1.0; max trip length /
trip length 0.0; allowed and starting travel-type masks both `0x0002` (Car); traffic density 0; traffic arrived 0;
flags 0; no route deque; destination filter 3 (end rectangle); destination flags 0; no occupant filter; no starting
cost array (use the simulator's default); max work time 0 (unlimited); not suspended; open set with capacity 99
(vector of 100 entries, entry 0 unused); node store with one page of 4096 nodes and 3079 empty buckets. The start
and end rectangles, the end cell, the park-and-ride cell and the *destination type reached* (§4.10) are **not**
initialised by the constructor. *[quirk]* FreeCity4 zero-fills them *[choice]*; §10 Q1 says where that matters.

## 2. State

The pathfinder is a transient object: nothing is saved (§6). The state a replacement needs:

| State | Set by | Initial | Notes |
|---|---|---|---|
| Owner traffic simulator | CreatePathFinder | — | not changeable |
| City size W, H | Init | — | |
| Flags (bit set, §4.2) | SetFlag / ClearFlag / ClearAllFlags | 0 | |
| Destination filter (0–4) | SetTripDestinationFilter | 3 | |
| Destination flags (u8, bits 0–3) | Set/Clear/ClearAllDestinationFlags | 0 | |
| Allowed travel types (u16 mask) | SetTravelTypeFlag / ClearTravelTypeFlag / ClearAll | `0x0002` | gates *switch* transitions only (§4.8.3) |
| Starting travel types (u16 mask) | SetTravelTypeFlag(mask, true) / Clear… | `0x0002` | types a trip may start in |
| Starting cost array (pointer to 9 floats, caller-owned) | SetStartingCostArray | null → simulator default | read at every FindPath; the caller must keep it alive |
| Max trip length / trip length (one float) | SetMaxTripLength; overwritten by a successful search | 0.0 | **one storage for input and output** *[quirk]* (§4.3) |
| Traffic density (u32, trips in this batch) | SetTrafficDensity | 0 | |
| Traffic arrived (u32) | FindPath / goal test | 0 | |
| Start rectangle, end rectangle (inclusive cell rects) | SetStartRect / SetEndRect; filter 0 overwrites the end rect | uninitialised | |
| Route deque (caller-owned `deque<RouteData>*`) | SetRouteInfo | null | |
| Max work time (ms) | SetMaxWorkTime | 0 | |
| Suspended | FindPath (time-out) / Reset | false | |
| Occupant filter (`cISC4OccupantFilter*`) | **no setter** | null | used only by filter 2 (§4.10.3) |
| Derived per call: max cost per travel type, search rect, excluded edge rect, heuristic weight, distance tables, over-budget counters, disabled travel types | FindPath | — | §4.4 |
| Search state: node store, open set, result node | FindPath, ReleasePathNodes | empty | survives between calls (continue mode) |
| Result: end cell, destination type reached, park-and-ride cell and side | FindPath | uninitialised | |
| Process-wide: work-check interval multiplier (init 1024), adaptation counter (init 0), one-time-init guard | first FindPath in the process | — | static, shared by all pathfinders (§4.11) |

## 3. Agent registration

None. The pathfinder is not a simulator, posts and receives no messages and has no cadence. It is driven entirely by
its callers (traffic-simulator.md: trip setup and path search; also the traffic automata, emergency-vehicle and MySim
dispatch code and the query tool, §9).

## 4. Behaviour

### 4.1 Vocabulary and constant tables

**Travel types** `t` (9): 0 Walk, 1 Car, 2 Bus, 3 Passenger train, 4 Freight truck, 5 Freight train, 6 Subway,
7 Elevated train (light rail), 8 Monorail. Masks are `1 << t`.

**Transit networks** `n` (6): 0 Pedestrian, 1 Road, 2 Rail, 3 Subway, 4 Light rail, 5 Monorail. The traffic simulator
keeps one float volume grid per transit network. `network(t)` = table `0x00AB20F8` (u32[9]):

| t | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
|---|---|---|---|---|---|---|---|---|---|
| network(t) | 0 | 1 | 1 | 2 | 1 | 2 | 3 | 4 | 5 |
| ProhibitSharpTurns[t] (`0x00AB211C`, u8) | 0 | 0 | 0 | 1 | 0 | 1 | 0 | 1 | 1 |

**Network types** (13; the `type` field of a cell's network info): 0 Road, 1 Rail, 2 Highway, 3 Street, 4 Pipe,
5 Power line, 6 Avenue, 7 Subway, 8 Light rail, 9 Monorail, 10 One-way road, 11 Dirt road, 12 Ground highway.

| type | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| LegalStartingSubtype (`0x00AB2128`) | 1 | 1 | 0 | 1 | 0 | 0 | 1 | 1 | 1 | 1 | 1 | 1 | 0 |
| LegalEndingSubtype (`0x00AB2138`) | 1 | 0 | 0 | 1 | 0 | 0 | 1 | 0 | 0 | 0 | 1 | 1 | 0 |

**Directions and sides.** Direction `d`: 0 = −x, 1 = −z, 2 = +x, 3 = +z (`dir(d)` from tables `0x00AB20D8` =
{−1, 0, 1, 0} and `0x00AB20E8` = {0, −1, 0, 1}). A search node's **entry side** `s` is the direction from the node's
cell to the cell it was reached from: moving from cell `a` in direction `d` to `b = a + dir(d)` gives `b` the entry
side `(d + 2) mod 4`. In general the side of `b` relative to a predecessor `a` is *EdgeTo*: `a.x < b.x → 0`,
`a.x > b.x → 2`, otherwise `a.z ≥ b.z → 3`, else `1` (so equal cells give 3).

**In city** means `0 ≤ x < W` and `0 ≤ z < H` (unsigned comparisons). Off-map cells adjacent to the map edge
(x or z equal to −1 or W / H) stand for neighbour-city connections.

**Inputs from the cell graph** (all owned by the traffic simulator; meaning and maintenance in traffic-network.md).
Per in-city cell and travel type `t`:

* `conn[t]` (u16): for entry side `s`, nibble `s` (`conn[t] >> 4s & 0xF`) has bit `d` set when a vehicle of type `t`
  that entered from side `s` may leave in direction `d`.
* `switch[t]` (u16): nibble `d` = the travel type a type-`t` trip may change to when leaving in direction `d`
  (`0xF` = none).
* `priorCar[s]` (u16, s = 0…3): car turn permissions; bit `4d + p` allows leaving in direction `d` a cell entered from
  side `s` when the predecessor itself was entered from side `p`.
* a wormhole byte: bit 1 = subway station, bit 2 = tunnel entrance, bit 4 = ferry terminal.
* per transit network `n`: an optional *network info* (absent = network `n` not in this cell) with an inverse
  capacity (float), the network type (0–12) and an extra step cost (float).
* the transit-switch map (u32 switch ID per cell, 0 = none) and `TransitSwitchEntryCost(id)` (float). An unknown ID
  inserts a default switch. In the original that switch holds stack garbage, so its cost is not reproducible;
  FreeCity4's all-zero switch costs the float `0x7EFFFFFF` *[choice]* (traffic-network.md §4.7.3, TN23).
* the destination bitmask map, `(W+2) × (H+2)` bytes indexed `[x+1][z+1]` (so off-map cells have entries): bit `k`
  (k = 0…3) = the cell has a destination of type `k` with free capacity; plus the number of such destinations per
  type.

Per travel type from the traffic simulator (traffic-simulator.md): `invMaxSpeed[t][type]` (float, class static),
`affectedByTraffic[t]`, `canReachDestination[t]`, `generatesTraffic[t]`, `incomePerTile[t]`, the default starting
cost array, `MaxDistanceOriginToNetwork` (rings, u32), `NearestDestinationAttractiveness` (float), the congestion →
inverse speed response curve (class static), the mass-transit strike state, and the volume grid per transit network.

### 4.2 Configuration

| Setting | Meaning |
|---|---|
| Flag 1 | **No budget**: do not reject nodes whose cost exceeds the max cost (§4.9). |
| Flag 2 | **Nearest on failure**: with filter 3 and a route deque, an exhausted search returns the node nearest to the end rect (§4.12). |
| Flag 4 | **Continue**: keep the node store and open set of the previous call instead of starting over (§4.4). |
| Flag 8 | **Park-and-ride return**: keep the park-and-ride cell of the previous successful trip and let the search switch back to Car there (§4.8.5). Without it the cell is reset to (−2, −2) at each call. |
| Flag 16 | **No ferries**. |
| Destination filter 0 | **Standard destination**: the nearest lot (or neighbour pool) with free capacity for a destination type in the destination flags (§4.5, §4.10.1). |
| Filter 1 | never matches (no goal). |
| Filter 2 | **Occupant filter**: any occupant around the cell accepted by the occupant filter (§4.10.3). Unreachable through the interface. |
| Filter 3 | **End rectangle** (§4.10.2). |
| Filter 4 | never matches. |
| Destination flags | bits 0–3, destination types wanted by filter 0. |
| `SetTravelTypeFlag(mask, alsoStarting)` | `allowed |= mask`; if `alsoStarting`, `starting |= mask`. |
| `ClearTravelTypeFlag(mask)` | clears `mask` in **both** masks. |
| `ClearAllTravelTypeFlags` | both masks = 0. |

The game's callers configure a pathfinder through these slots; the traffic simulator's own per-trip configuration
(`SetupPathFinderForLot`, `SetupTrip`) is specified in traffic-simulator.md.

### 4.3 FindPath protocol

`FindPath()` (slot 23) returns in EAX: **1** found, **0** not found, **2** suspended (time-out; call again to
resume). After 1: `GetTripLength`, `GetEndCell`, `GetTrafficArrived`, `GetLastTravelType`, `WasTripReversible`,
`GetCompressedRoute` and `ApplyTraffic` describe the result node; the route deque (if any) holds the route. After 0
or 2 there is no result node, the getters return their defaults (§4.14) and `GetTripLength` returns the max trip
length that was set. *[quirk]*

**FindPath on an object that is not initialised** (PF11). `FindPath` never tests the initialised flag. Before `Init`
the city size W, H and the two distance-table base pointers are whatever the heap held (the constructor sets none of
them); after `Shutdown` the base pointers are null. The first distance-table write (`SetupDistanceTables`,
`0x006D6060`) then faults or writes through a garbage pointer. Every filter builds the tables, except that filter 0
returns 0 first when §4.5 fails. This cannot happen in the game: `CreatePathFinder` always runs `Init`, and
`Shutdown` runs only inside the last `Release`. *[choice]* FreeCity4 returns 0 and changes nothing.

`SetMaxTripLength` and `GetTripLength` share one float. A successful search overwrites it with the trip length, so
the next `FindPath` on the same object uses the previous trip length as its budget unless the caller sets the max trip
length again. *[quirk][compat]* `Reset` (slot 22) only clears the suspended state; it does not clear results or the
node store.

### 4.4 Per-call preparation

At every call, in this order:

1. First call in the process: work-check multiplier = 1024, adaptation counter = 0 (§4.11).
2. Start the work timer with the max work time (§4.11). Traffic arrived = 0.
3. If flag 8 is clear: park-and-ride cell = (−2, −2).
4. `maxCost[t] = r24(startCost[t] + maxTripLength)` for t = 0…8, where `startCost` is the caller's array or, if none
   was set, the simulator's default starting costs. This is done on every call, also when resuming.
5. If not suspended:
   * if flag 4 is clear: excluded edge rect = (−2, −2, −2, −2) and `ReleasePathNodes` (slot 31, through the vtable);
   * result node = none; the nine over-budget counters = 0; disabled travel types = none.
6. Mass-transit strike (the simulator's strike state equals 1): the allowed mask used for **switch transitions** is
   `allowed & 0x33` (Walk, Car, Freight truck, Freight train), and subway-station and ferry moves are skipped (§4.8).
   The starting mask, same-type moves and tunnels are not affected. *[quirk]*
7. If not suspended:
   * search rect = start rect grown by 4 cells on every side;
   * filter 0: heuristic weight `w` = NearestDestinationAttractiveness; `FindNearestStandardDest` (§4.5); if it
     fails, return 0; build the distance tables; with flag 4, recompute the scores of all stored nodes (§4.18);
   * filter 3: build the distance tables; `w` = NearestDestinationAttractiveness (no score recomputation, even with
     flag 4 *[quirk]*);
   * filters 1, 2, 4: build the distance tables; `w` = 1.0;
   * if the starting mask is 0, return 0;
   * if flag 4 is clear: create the start nodes (§4.6); if none were created, return 0.
8. Fetch the six volume grids' data (raw, simgrid.md §6.4). Clear the suspended state. Run the main loop (§4.8).

**Distance tables** (from the end rect `(ex0, ez0, ex1, ez1)`): for `i` = −1 … W, `dX[i] = ex0 − i` if `i < ex0`,
`0` if `ex0 ≤ i ≤ ex1`, `i − ex1` if `i > ex1`; `dZ` likewise over −1 … H. Integers.

**Index range** (PF14). The tables are read only for cells outside the search rect: `H` (`0x006D6160`) at the
cell AddTripNode or CreateStartNodes is given, and RecalculateNodeScores at a node's own cell (§4.18). In every game
flow those cells are in the city or one cell off it: in-city cells for moves, tunnels, ferry links and starts, and
the in-city cell for an exit node (§4.8.2), whose own cell is one step off the map. So the index stays in −1…W and
−1…H. The original does not check it; an index outside would read the heap next to the table. *[choice]* For such an
index FreeCity4 applies the formula above (`ex0 − i` or `i − ex1`).

**Heuristic** `H(x, z)`: 0.0 if `(x, z)` is inside the search rect (inclusive); otherwise the float conversion of the
integer `dX[x] + dZ[z]` (exact). The score of a node is `f = r24(r24(H(x, z) × w) + g)`. So the search is plain
Dijkstra within 4 cells of the start and weighted A\* beyond. *[compat]*

### 4.5 Standard destination search (filter 0)

`FindNearestStandardDest` chooses the end rect before the search:

1. Fail unless some type `k` (0–3) is in the destination flags **and** the simulator's count of free destinations
   of type `k` is non-zero.
2. Search bounds = (−1, −1, W, H) (the off-map ring included). If the start rect is off-map, exclude its edge:
   start x0 < 0: bounds x0 = 0 and excluded edge rect = (−1, −1, −1, H); else start x1 = W: bounds x1 = W − 1 and
   excluded = (W, −1, W, H); else start z0 < 0: bounds z0 = 0 and excluded = (−1, −1, W, −1); else start z1 = H:
   bounds z1 = H − 1 and excluded = (−1, H, W, H). Otherwise the excluded rect is left as set in §4.4.
3. Ring search. `r` = start rect. Repeat: grow each side of `r` by one if it is still inside the bounds (a side that
   cannot grow stops); if no side grew, fail. Then scan, in this order, only the sides that grew: the left column
   (`x = r.x0`, z from `r.z0` to `r.z1`), the right column (`x = r.x1`, same z range), the top row (`z = r.z0`,
   x from `r.x0` to `r.x1`), the bottom row (`z = r.z1`, same x range). A cell is a candidate when its destination
   byte has a bit in the destination flags and it is not inside the excluded rect. Corners can be listed twice. If
   there is no candidate, continue with the next ring.
4. Choose among the ring's candidates: if any is in city, only the in-city ones (in scan order) are considered. The
   winner is the first one with the smallest Manhattan distance to the start rect (per axis: distance to the rect's
   range, 0 inside it). Strict less-than: the earliest candidate wins ties.
5. In-city winner: the end rect becomes the bounding rect of the lot at that cell; if there is no lot, fail.
   Off-map winner: the end rect becomes that single cell.

The candidate vector is internal (allocator B). The destination bitmask map is only read here. The caller's end rect
is overwritten. *[quirk]*

### 4.6 Start nodes

The starting cost of type `t` is `startCost[t]` (caller array or simulator default).

**Start-edge table** (which entry sides a start node may have): start cell in city → all four sides; else x0 < 0 →
side 0 only; else x0 ≥ W → side 2 only; else z0 < 0 → side 1 only; else side 3 only (`0x00AB21D8`…`0x00AB21E8`).
**Lot start**: the start cell (x0, z0) is in city and the lot manager has a lot there (`GetLot(x0, z0, false)`: a
lot covering the cell, not only one whose origin it is; §8).

**Rings.** For ring `d` = 1 … MaxDistanceOriginToNetwork (all rings, there is no early stop): the ring rect is the
previous ring rect grown by one on every side and then clamped to the city (ring 0 = the start rect; the clamped
values carry over to the next ring). The ring's cells are visited in *boundary order*: the first row (`z = z0`) from
`x0` to `x1`; each middle row only at `x0` and then `x1`; the last row (`z = z1`) from `x0` to `x1` (a one-row rect is
visited once; a one-column rect visits each middle row once). Only cells with `x` inside the start rect's x range or
`z` inside its z range are used (cells diagonal to the start rect are skipped). For each such cell, for each transit
network `n` = 0…5 with network info in the cell (and, for a lot start, `LegalStartingSubtype[info.type]`), for each
travel type `t` = 0…8 in the starting mask with `network(t) = n`, for each side `s` = 0…3 allowed by the start-edge
table with nibble `s` of `conn[t]` non-zero: create (or fetch, §4.7) the start node for `(cell, t, s)`, set
`g = startCost[t]`, `f = r24(r24(H × w) + g)`, no parent, open, and push it on the open set.

Because the ring rect is clamped, the start rect's own cells can be visited when the start lot touches the map edge,
and a later ring can revisit the same edge cells; the same node is then pushed again (the heap holds it twice).
*[quirk]*

**Off-map start fallback** (ferry arrivals; `0x006D8B54`–`0x006D8F8E`). If no node was created and the start cell
`(x0, z0)` is off-map (unsigned `x0 ≥ W` or `z0 ≥ H`), the trip may begin by ferry. The loops run over the
simulator's **ferry terminals** (its terminal list, in list order; traffic-network.md §2.6), not over its neighbour
connections, as an earlier version of this section said. For each terminal, for each of its **neighbour links** in
list order whose off-map cell equals `(x0, z0)`, and for each travel type `t` = 0…8 in both the starting mask and the
terminal's travel mask:

* fetch or create the start node at the terminal's **arrival cell** `(x2, z2)` with entry side 0 (key
  `(0x10 << 24) | t << 16 | x2 << 8 | z2`);
* set `g = r24(startCost[t] + link.cost)` (the link cost of traffic-network.md §4.9.3), `f = r24(r24(H(x2, z2) × w) +
  g)`, no parent, open, not closed, and push it.

Nothing ends the loops early, and nothing checks whether the node already exists (PF5). When several links match
(two links of one terminal with the same off-map cell, or terminals sharing an arrival cell), each match overwrites
`g` and `f` of the same node and pushes it again. The node keeps the **last** match's cost, and the heap holds it
once per match, each entry placed by the `f` it had when pushed (as with the clamped rings above) *[quirk][compat]*.
Example: one terminal, travel mask Car, links L1 (cost 0.5) then L2 (cost 0.3), both to the start cell, start cost
0: the Car start node ends with `g = 0.3` and is in the heap twice.

Start nodes created here ignore the start-edge table and the cell's `conn`. If the start cell is in city and no
node was created, `FindPath` returns 0.

### 4.7 Search nodes and the open set

**Node identity.** A node is a (cell, travel type, entry side, predecessor's entry side) state. Its 32-bit key is
`((pSide << 2 | side) << 24) | (t << 16) | (x & 0xFF) << 8 | (z & 0xFF)` where `pSide` is the entry side of the
node it was reached from; start nodes use `((0x10 | side) << 24) | …` (ferry-arrival start nodes, §4.6: side 0);
**exit nodes** (nodes for an off-map cell, §4.8.2) set bit 31 and are keyed on the in-city cell they were reached
from, with `side` = the direction of the move: the node's cell is that in-city cell plus `dir(side)`. A new node
gets its cell, type and side from the key (`0x006D5DF0`): `x = (key >> 8) & 0xFF` and `z = key & 0xFF`
zero-extended, and for bit 31 plus `dir(side)` added as a 16-bit value. For every exit node the game makes (§4.8.2
moves and the ferry exits of §4.8.3, whose in-city cell is adjacent to the off-map cell) this is the true off-map
cell: `0 − 1 = −1` and `255 + 1 = 256` come out right. Including
`pSide` in the identity is what makes the car turn rule (§4.8.2) a property of the state. The key decides identity
only; a key that has been seen in this search always maps to the same node.

**Node store** (internal). Nodes are allocated from pages of 4096 nodes (allocator C, kept until destruction, reused
after `ReleasePathNodes`) and looked up in a chained hash table with 3079 buckets, bucket = `(key × 0x9E3779B1 mod
2^32) mod 3079`, new nodes inserted at the head of their chain. A new node is neither open nor closed. Bucket order
and chain order matter only for the failure fallback (§4.12).

**Open set: a 1-indexed binary min-heap on `f`.** *[compat]* This exact structure decides the tie order.

* **Push**: append at position `count + 1`; while the parent's `f` is **strictly greater** than the node's `f`, move
  the parent down. Equal (or unordered, NaN) stops.
* **Pop**: take position 1. If it was the only entry, the heap becomes empty. Otherwise move the last entry to
  position 1, decrement `count`, and sift down: at position `i`, choose the left child if its `f` is strictly less
  than the moving entry's `f`, then the right child if its `f` is strictly less than the chosen one's; stop if
  neither; swap and continue while the new position is below `count`.
* **Update** (a node already open got a lower `f`): sift up from its current position with the Push rule. Only up.
* **Position.** Each node records the slot where it was last written (`+0x20`): Push and Update set it at the slot
  where the node comes to rest, and every sift move sets it for the entry it moves. Pop does not clear the popped
  node's record. Update (`0x006D6460`) starts from the recorded slot **without checking** that the slot holds the
  node, and ends by writing the node into its final slot (PF9). A node in the heap twice (§4.6) records whichever
  copy was written last, and Update moves that copy only. In the game an open node's recorded slot always holds it:
  the only open node whose slot can be stale is the one being expanded (it stays flagged open until §4.8.5), and no
  move of §4.8 produces a node's own key, so it never receives an Update. *[choice]* FreeCity4 may treat an Update
  whose recorded slot does not hold the node as a no-op; the original would overwrite that slot's entry.
* Capacity grows to `2 × count` entries when full (initial capacity 99).

Equal-`f` nodes are therefore not served first-in-first-out (conformance test 1).

#### 4.7.1 Equal-score Walk-to-Bus corner: limits of the local trace (TE33-PATH-1, partial)

Instruction recheck on 2026-10-28 confirms the existing strict comparisons: Push compares
stored binary32 f at `0x006D8905`–`0x006D8910`; Pop moves the last entry to the root
at `0x006D63A9`–`0x006D63B5` and compares left, then right, with strict less-than at
`0x006D63E5`–`0x006D6413`. Neither key order nor insertion sequence is a secondary
comparison. This does not justify changing the heap to FIFO on ties.

Two small conformance fixtures distinguish the rules. W is the reported West node
`0x0E02F33B`, S the South node `0x0D02F43C`; both have stored f bits `0x3CAAF6A7`.

| Initial/action sequence | Required pop order | Reason |
|---|---|---|
| Empty heap; push W, then S | W, S | Equal f does not sift S above W |
| Empty heap; push an unrelated R with f=0, W, then S; pop all | R, S, W | Removing R moves last entry S to root; equal-f left child W does not displace it |

Thus the reported West-before-South *pushes* do not determine their subsequent pop
order without the entire preceding heap. This fixture is synthetic, not a replay of Jinae.
For the reported start rectangle (244,61)–(245,63), ring 1's eligible cells, before
network/type/side filtering, are (244,60), (245,60), (243,61), (246,61), (243,62),
(246,62), (243,63), (246,63), (244,64), (245,64). Apply §4.6's network 0…5,
travel-type 0…8 and side 0…3 loops within each cell. Start costs and coordinates alone
do not identify which of these nodes exist: connection nibbles and network-info records
are necessary inputs.

**What actually merges (additional code/trace check, 2026-10-28).** Equal cell/type/side
does **not** suffice for an equal-g update. AddTripNode uses the predecessor's byte
entry side in the key (`0x006D8FC7`–`0x006D8FE7`); the node key is u32, its type/side
fields are bytes, its decoded x/z fields are 16-bit, its g/f fields binary32, and its
recorded heap position is u32. The two branches have these identities when the reported
three Bus step costs all have bits `0x3CAAF6A7`:

| Branch | Cell | Entry side | Predecessor entry side | Key | g bits |
|---|---|---|---|---|---|
| South first | (244,60) | 1 | 3 | `0x0D02F43C` | `0x3CAAF6A7` |
| West first | (243,59) | 2 | 3 | `0x0E02F33B` | `0x3CAAF6A7` |
| South then West | (243,60) | 2 | 1 | `0x0602F33C` | `0x3D2AF6A7` |
| West then South | (243,60) | 1 | 2 | `0x0902F33C` | `0x3D2AF6A7` |
| South branch at (243,61) | (243,61) | 1 | 2 | `0x0902F33D` | `0x3D8038FD` |
| West branch at (243,61) | (243,61) | 1 | 1 | `0x0502F33D` | `0x3D8038FD` |

The last two are **different search nodes**, despite their equal visible tuple and g.
On their next South move, both address **`0x0502F33E`, (243,62,Bus,side1,pSide1)**.
For the costs above, each proposes g bits `0x3DAAF6A7`. This is the first shared key
on these two stated continuations, not (243,61). Earlier cross-links/other branches can
still affect either continuation.

The preserved **replacement-only** trace is available at
`.claude/worktrees/traffic-exact33/build-runs/r9-jinae-corner.txt` in the main checkout.
Lines 35–39 pop South `0x0D02F43C` before West `0x0E02F33B`; lines 54–65 pop their
distinct (243,60) successors in that same branch order; lines 82–92 pop
`0x0902F33D` before `0x0502F33D`, and both attempt (243,62). Line 97 later pops the
shared key with the South branch as parent. ADD records are **attempts**, not proof of
successful insertion/update. This is consistent with the original's equal-g rule, but
is not evidence that the original used the same preceding heap or network inputs.

**Conformance fixture for the join, independently of the city.** Begin with no node
`J=0x0502F33E`. Supply the two distinct (243,61) predecessor nodes from the table,
each g bits `0x3D8038FD`; H at J is zero, w is binary32 0.09, Bus enabled, budget large
enough, no other proposals to J. Process one predecessor's South move, step cost bits
`0x3CAAF6A7`, then the other's. The required result is:

| Proposal order | First action at J | Second action at J | Final parent |
|---|---|---|---|
| `0x0902F33D`, then `0x0502F33D` | create, g=f `0x3DAAF6A7`, Push once | ignore; no heap Update | `0x0902F33D` (South branch) |
| `0x0502F33D`, then `0x0902F33D` | create, g=f `0x3DAAF6A7`, Push once | ignore; no heap Update | `0x0502F33D` (West branch) |

The second equal proposal is also ignored if J has already been closed. This follows
from the **stored** candidate g and the ordered comparisons at `0x006D9070`–`0x006D9091`;
finite equal g does not replace a parent, reopen a closed node, or consult its key as
a tie-breaker. Candidate g is added with x87 and stored binary32 before either compare
(`0x006D9007`–`0x006D9013`). Accepted proposals store the parent and g, evaluate H, multiply
by the binary32 weight, add g, store binary32 f, then call heap Update or Push according
to the prior open byte (`0x006D9093`–`0x006D90DF`). Keep the 24-bit operation rounding
of §4.19; equal decimal printouts alone do not establish equality of those stored bits.
Heap Update uses only the recorded position and strict f comparison, with no key or
parent comparison (`0x006D6460`–`0x006D64AB`). There is no rand() draw anywhere in this
decision (§4.20). The pathfinder owns the heap/parents; the simulator supplies network,
traffic-volume and destination data.

**Inputs still required to turn these fixtures into the requested native replay.** Either
of the following would determine the native decision; the final route bytes do not:

1. An original snapshot immediately before the competing proposals: the **ordered heap
   array**, each entry's u32 key and stored g/f bits, parent key, open/closed bytes and
   recorded heap position, including repeated entries; plus the intervening successful
   inserts/updates/pops and ordered edge proposals through the first shared key. A set
   of heap members or the two equal Bus scores alone is insufficient.
2. Inputs sufficient to regenerate that snapshot from the start: the original call's
   flags, destination filter, density, both type masks, nine starting-cost bit patterns,
   maximum length, weight and rectangles; city bounds, lot-start classification and
   origin-ring limit; original legal-subtype/start-edge/turn tables; the start-ring
   network records and side connection nibbles; and all network/switch/volume/cost/H
   data consulted on the expansion prefix (§§4.4, 4.6, 4.8–4.11). Record each actual
   start **push**, in order, with key/g/f bits, including repeated-key pushes from
   clamped rings or ferry arrivals. A resumed call also needs its retained search state
   and any intervening heuristic-weight adjustment. Keep the native x87 control word
   with numeric observations.

No original snapshot or original insertion/pop prefix is present in the cited trace,
which logs the copy only. Its initial zero-score pops are not a native start-insertion
vector. The allowed mask and ring-cell order do not recover connection nibbles or
which sides pass the start filters. Thus code proves **strict heap ordering followed
by first accepted equal-g parent at the same full key**; it does not prove the native
West pop or the first differing original input for Jinae. The Braus delivery has no
cost/pop trace and other differing trips: first identify its original query/inputs,
then use the same snapshot requirements. No zero-cost-loop, rounding or heap defect
is established by its route-byte suffix. TE33-PATH-1 remains partial; no game was run.

### 4.8 Main loop

While the open set is not empty: pop node `c` (cell `(x, z)`, type `t`, entry side `s`, predecessor `p`).

#### 4.8.1 Goal test

* `cannotEnd = not canReachDestination[t]`. If `canReachDestination[t]` and the cell is in city and has network info
  for `network(t)`: `cannotEnd = not LegalEndingSubtype[info.type]`.
* If the cell is in city and carries a transit switch (switch map ≠ 0), the goal test is skipped.
* Otherwise, if the goal test (§4.10) with `cannotEnd` succeeds (for an off-map cell it is called with
  `cannotEnd = false` when `canReachDestination[t]`, and with `true` otherwise): result node = `c`; `c` is pushed back
  onto the open set (so a continued search sees it again); the trip is recorded (§4.13); the check-interval
  adaptation runs (§4.11); return 1.

Off-map nodes are never expanded (they are closed after the goal test).

#### 4.8.2 Moves along the network (in-city node)

`exits = conn[t]` nibble `s`. If `ProhibitSharpTurns[t]` and `c` has a predecessor: remove bits `s` and `p.side`
from `exits`. *Car turn check* applies when `network(t) = 1` (Car, Bus, Freight truck) and `c` has a predecessor.
Then for each direction `d` = 0, 1, 2, 3:

**(a) Same-type move**, if bit `d` of `exits` is set; `b = (x, z) + dir(d)`:

* `b` in city, entry side `s' = (d + 2) mod 4`:
  * network info `I` of `network(t)` in `b` present:
    if the car turn check applies and bit `4d + p.side` of `priorCar[s]` (of cell `c`) is clear, skip.
    Speed term `v = invMaxSpeed[t][I.type]`. If `affectedByTraffic[t]`:
    `q = r24(volume[network(t)](b) × I.invCapacity)`, `cost = r24(r24(Curve(q) × v) + I.extraCost)`,
    where `Curve` is the simulator's congestion → inverse-speed curve evaluated with `GetOutputValue` (foundations §4;
    the argument is the float32 `q`). Otherwise `cost = r24(v + I.extraCost)`.
    AddTripNode(`b`, `s'`, `t`, cost, `maxCost[t]`).
  * no info, but `b` carries a transit switch: AddTripNode(`b`, `s'`, `t`, `TransitSwitchEntryCost(id)`, `maxCost[t]`).
  * otherwise nothing.
* `b` off-map: AddTripNode(exit node from `(x, z)` in direction `d`, type `t`, `cost = r24(c.f − c.g)`,
  `maxCost[t]`). The cost of leaving the map is the node's own heuristic term. *[quirk]* The exit node's heuristic is
  evaluated at `(x, z)`, not at the off-map cell.

**(b) Switch move**: `t2 = switch[t]` nibble `d`. If `t2 ≠ 0xF` and `t2` is in the (strike-reduced) allowed mask;
`b = (x, z) + dir(d)` (no bounds check *[quirk]*: an off-map `b` reads outside the map, see §10 Q4), entry side `s'`:

* `b` carries a transit switch: AddTripNode(`b`, `s'`, `t2`, `TransitSwitchEntryCost(id)`, `maxCost[t2]`);
* else network info `I` of `network(t2)` in `b` present: cost as in (a) with `t2`, `network(t2)` and the volume of
  `network(t2)`, no car turn check; AddTripNode(`b`, `s'`, `t2`, cost, `maxCost[t2]`);
* else nothing.

The bit `d` of `exits` is not required for a switch move.

A nibble `t2` of 9…14 (no shipped switch rule produces one) passes only if bit `t2` of the allowed mask is set
(`0x006D97D1`–`0x006D97DD`: `test` of the u16 mask against `1 << t2`). No game caller sets a bit above 8 (the masks
are `0x1CF`, `0x3`, `0x30`, `0x2`, `1 << strategy` with strategy 0…8, and copies of them; traffic-simulator.md §4.4),
so such a nibble is ignored in practice. With such a bit set, the original would index its 9-entry tables
(`network`, `maxCost`, `invMaxSpeed`) past their ends. *[choice]* FreeCity4 ignores a nibble above 8 (PF13).

#### 4.8.3 Wormholes (in-city node)

In this order, from the wormhole byte of `c`'s cell:

1. **Tunnel** (bit 2): look the cell up in the simulator's tunnel table; if found and its travel-type mask has `t`:
   AddTripNode(tunnel destination cell, entry side `s` (unchanged), `t`, tunnel cost for `t`, `maxCost[t]`).
   The tunnel's travel-type mask is always `0x1FE`, so the tunnel cost is +∞ for a travel type whose speed on
   that network is zero (traffic-network.md).
2. If a mass-transit strike is on, skip 3 and 4.
3. **Subway station** (bit 1): `t' = 0` (Walk) if `t = 6` (Subway), else `t' = 6`. For each side `s2` = 0…3: if
   `conn[t] >> 4s` (the **unmasked** shift: all higher nibbles count *[quirk]*) is non-zero: AddTripNode(same cell,
   entry side `s2`, `t'`, cost 0.0, `maxCost[t']`). The shift is by the popped node's own entry side `s`, not by
   `s2` (PF4, `0x006D99D1`–`0x006D99E2`: a 16-bit `shr` by `4 · node.side`, read again in each pass of the `s2`
   loop). The test is therefore the same for all four `s2`: all four nodes are added, or none. The allowed mask is
   not checked. *[quirk]* **Unreachable in 1.1.641**: no code sets
   wormhole bit 1 (the byte is only reset, `|= 2` and `|= 4`; traffic-network.md), so this branch never runs.
   *[compat]* FreeCity4 keeps the rule but never sets the bit.
4. **Ferry** (bit 4), unless flag 16: look up the cell's ferry terminal; if its travel-type mask has `t`: for each
   terminal link in list order, AddTripNode(the target terminal's arrival cell, side 0, `t`, link cost, `maxCost[t]`); then for each neighbour (off-map) edge
   of the terminal, AddTripNode(exit node at the edge's off-map point, reached from the edge's in-city cell, `t`,
   edge cost, `maxCost[t]`). List order as stored by the simulator (traffic-network.md).

   The neighbour edge is a NeighborLink (traffic-network.md §2.6): off-map cell `O` at `+8`/`+0xA`, in-city edge
   cell `E` at `+0xC`/`+0xE`, cost at `+0x10`. The call passes cell `E`, entry side `EdgeTo(O → E)` and the exit flag
   (PF15, `0x006D9B55`–`0x006D9BA5`). The exit node's cell is therefore `E + dir(EdgeTo(O → E))` (§4.7), one step
   from `E` towards `O`, and its `H` is taken at `E` (§4.9 step 6). Every link the game makes has `E` =
   `GetNeighborConnectionInnerCell(O)` (traffic-network.md §4.9.3), adjacent to `O`, so the node's cell is `O`.
   Example: `O = (−1, 7)`, `E = (0, 7)`: `EdgeTo(O → E) = 0`, cell `(0 − 1, 7) = O`. Only a damaged save could pair
   cells that are not adjacent; the node then lands one step from `E` (the x direction first).

#### 4.8.4 Park-and-ride return (flag 8)

If `c`'s cell equals the park-and-ride cell `P` recorded by the previous trip (§4.13): `b = P + dir(Pside)`;
AddTripNode(`b`, EdgeTo(`P`→`b`), **Car**, cost 0.0, `maxCost[t]` (of `c`'s type), exit = `b` is off-map). `b` is the cell of the car
predecessor recorded with `P` (§4.13 step 1); that predecessor was expanded, so it is in city and `b` is never
off-map in practice. If it were, the exit-node key would hold `b`'s low bytes and the decode (§4.7) would add
`dir(EdgeTo(P → b))`, which points back at `P`: that gives `P` past the +x/+z edges of a city narrower than 256
cells, but 256 past the −x/−z edges (the byte 255 plus 1). *[quirk, unreachable]*

#### 4.8.5 Close and time check

Close `c` (closed, not open). If a max work time is set: advance the work counter (§4.11); when it crosses a
multiple of 1024 and the timer has expired: suspended = true, adaptation, return 2.

### 4.9 Adding a node (AddTripNode)

Arguments: cell, entry side, type `t`, step cost, predecessor `c`, `maxCost`, exit flag.

1. If `t` is disabled, ignore.
2. Build the key (§4.7). If `c`'s predecessor has exactly this key, ignore (no stepping straight back into the state
   one came from).
3. `g = r24(cost + c.g)`.
4. Unless flag 1: if not `g ≤ maxCost` (also when `g` is NaN): the over-budget counter of `t` is incremented; when it
   exceeds 64 (the 65th rejection in this call), `t` is disabled for the rest of the call; ignore. *[compat]*
5. Fetch or create the node (a new node is neither open nor closed, so neither test applies). If it is open and
   `node.g ≤ g`, ignore. If it is closed and `node.g ≤ g`, ignore. Both are **ordered** IEEE compares
   (`0x006D9070`, `0x006D9085`: `fcomp`, then only C0 is tested, and an unordered result sets C0 too), so the node is
   taken when `g < node.g` **or when either value is NaN** *[compat]* (PF1):

   | `node.g` | `g` | result |
   |---|---|---|
   | 3.0 | 2.5 | taken |
   | 3.0 | 3.0 | ignored (an equal `g` never replaces) |
   | 3.0 | NaN | taken |
   | NaN | 5.0 | taken (a NaN `node.g` is replaced by any `g`) |

   A NaN `g` reaches this step only with flag 1, because step 4 rejects it otherwise.
6. Set predecessor = `c`, not closed, `g`, entry side, `f = r24(r24(H(x, z) × w) + g)` with `(x, z)` the passed cell
   (the in-city cell for an exit node). If it was open: Update; else Push and mark open. A closed node is re-opened.

### 4.10 Goal test (AtGoal)

#### 4.10.1 Filter 0: standard destination

Let `(x, z)` be the node's cell.

* If `(x, z)` is in city, or inside the start rect, or inside the excluded edge rect: if `cannotEnd`, no goal.
  Otherwise for `d` = 0…3: `n = (x, z) + dir(d)`; skip unless `n` is in city and not inside the start rect; let
  `bits` = destination byte of `n` (read once); skip unless `bits & destFlags`; get the lot at `n` (skip if none) and
  its property holder; for `k` = 0…3 with bit `k` in `bits & destFlags`: if `CheckDestinationCapacity(holder, k)`:
  end cell = `n`, goal. Else: clear bit `k` in the destination byte of every cell of the lot's bounding rect and, if
  the simulator's free count for type `k` is non-zero, decrement it; continue with the next `k`.
* Otherwise (off-map and not in those rects): get the neighbour pool's property holder for `(t, x, z)` (none → no
  goal); for `k` = 0…3 with bit `k` in the cell's own destination byte and the flags: if `CheckDestinationCapacity`:
  end cell = `(x, z)`, goal; else clear bit `k` in that cell's byte (no count decrement). `cannotEnd` is ignored.

**CheckDestinationCapacity(holder, k)**: `max` = the simulator's `GetMaxTripCapacity(holder, k)` (slot 20);
`used` = its used trip capacity; false if `max = 0` or `used ≥ max`; else traffic arrived = `min(max − used,
density)`, destination type reached = `k`, true. The capacity is not consumed here; the caller does that when it
applies the trips (traffic-simulator.md).

The pathfinder **writes** into the simulator's destination bitmask map and free counts (stale destinations are
removed as they are discovered). *[compat]*

#### 4.10.2 Filter 3: end rectangle

If `(x, z)` is inside the end rect and (`not cannotEnd` or `(x, z)` is off-map): end cell = `(x, z)`, traffic arrived
= density, goal. Otherwise, if the end rect's first corner `(ex0, ez0)` is in city: for `d` = 0…3, the first
`n = (x, z) + dir(d)` inside the end rect with (`not cannotEnd` or `n` off-map) is the end cell; traffic arrived =
density; goal. So a node next to the end rect also ends the trip.

#### 4.10.3 Filter 2: occupant filter

Collects the occupants of the 3×3 cells around `(x, z)` (clamped to the city) with the occupant manager (slot 40, the
filter overload, null filter, all flags; the list is allocated with allocator A and released afterwards). For every
occupant the stored occupant filter's `IsOccupantIncluded` is asked; if any says yes, traffic arrived = density and
the result is a goal. The end cell is not set. There is no setter for the occupant filter in the interface (the
constructor sets it to null), so with filter 2 the original dereferences a null filter as soon as an occupant is
found *[inferred: no writer of that member found]*. FreeCity4 treats a null filter as "no goal" *[choice]*.

#### 4.10.4 Filters 1 and 4

Never a goal.

### 4.11 Work-time limit

The timer is a time-stamp-counter deadline: `now + _ftol(maxWorkTime × ticksPerMs)` (`0x0068BF50`), expired when
the counter reaches it (`0x0068BF80`). With max work time 0 the search never suspends.

To keep timer reads rare, a process-wide multiplier `m` (initially 1024, power of two, 1…1024) is used: a per-call
counter starts at 0 and, when a max work time is set, gains `m` (the value at call entry) for every popped node that
is not a goal, at its close (§4.8.5). Off-map nodes (exit nodes) are closed without expansion and still count
(PF7: the off-map branch at `0x006D9540` jumps straight to the close at `0x006D9C81`, which advances the counter at
`0x006D9C97`). A goal node does not count. The timer is read only when the counter is a multiple of 1024. With max
work time 0 the counter stays 0, so `k = 0` below and the adaptation never runs. At every return after the search started (found, suspended, exhausted), with
`k = counter >> 10` (the number of timer reads this call) and a process-wide adaptation counter `a`:
`k = 0` → nothing; `k > 20` → `a += 1`, and when `a ≥ 16`: `a = 0` and `m = m / 2` if `m > 1`; `1 ≤ k < 5` →
`a −= 1`, and when `a ≤ −4`: `a = 0` and `m = 2m` if `m < 1024`. The multiplier is shared by all pathfinders.
Suspension is wall-clock dependent and therefore not deterministic; conformance runs use max work time 0. *[compat]*

A suspended search resumes at the next `FindPath`: preparation steps 5 and 7 are skipped, `maxCost` is recomputed
(step 4), the open set and nodes are kept.

### 4.12 Exhausted open set

If the open set empties: if flag 2 is set, the filter is 3 and a route deque is set, the result node becomes the
stored node (any node created in this search, open or closed, including start and exit nodes) with the smallest
`DistanceToRectangle` to the end rect, visiting buckets 0…3078 and each chain from its head; strict less-than, so the
first visited wins ties. The trip is recorded (§4.13), traffic arrived stays 0, adaptation, return 1. Otherwise
adaptation, return 0.

`DistanceToRectangle(x, z, rect)`: 0.0 inside; if `x` is within the x range, the integer z distance to the nearer
z edge; if `z` is within the z range, the integer x distance; otherwise the Euclidean length (`LengthOfLine`
`0x005D7660`, float) from the cell to the nearest corner.

`LengthOfLine` takes the two points as 3-vectors `(x, 0, z)` of floats and computes the three differences, their
squares and the sum on the x87 at the main thread's 24-bit precision, then `fsqrt` (PF12). For cell coordinates
every step before the root is exact (integers far below 2^24), and `fsqrt` under 24-bit precision control is
correctly rounded, so the result is **the exact square root rounded once to float32**. Examples: √2 = `0x3FB504F3`,
√5 = `0x400F1BBD`, √10 = `0x404A62C2` (test 6).

### 4.13 Recording a successful trip

1. Walk from the result node back to the root (the node without predecessor). Each time a node's predecessor is a
   Car node and the node itself is not Car, record park-and-ride cell = the node's cell and side =
   EdgeTo(predecessor's cell → node's cell), i.e. the direction from the node back to the car predecessor; the last record
   of the walk (the transition nearest the start) wins.
2. Trip length `L = r24(result.g − root.g)`; if `L < 0` then `L = +0.0` (NaN is kept).
3. Windows: if the filter is 0 **or 3**, the destination type reached is not 3 and the result cell is off-map
   (`x ≥ W` or `z ≥ H`, unsigned): `L = r24(cost + L)`, where `cost` is the f32 of the cell's edge record
   (`GetTrafficNeighborConnection`, traffic-network.md §2.7.1 and §2.7.2). The lookup never fails, so the null
   test at `0x006D87F2` is dead; a cell where no connection was ever added has cost 0.0. (The Mac build does this
   for filter 0 only.) For filter 3 the destination type reached is whatever the last filter-0 success on this object
   left there, uninitialised on a fresh object (§10 Q1). *[quirk]*
4. The trip length is stored in the shared max-trip-length/trip-length float.
5. If a route deque is set: clear it, then push to the **front** one `RouteData {u8 x, u8 z, u8 travelType}` per node
   from the result node back to the root, so the deque runs root → result. Coordinates are the low bytes (−1 becomes
   255). Off-map nodes (start or exit) are included. This runs for every recorded trip, so also for the flag-2
   fallback of §4.12. The container operations are in §7 (deque layout). *[compat]* A replacement must fill the
   deque: the traffic automata, emergency and MySim dispatch and the query tool read it (§9), and returning 0 instead
   leaves them without a route (TG1).

### 4.14 Result getters

| Getter | Result |
|---|---|
| `GetTrafficArrived` | trips accepted by the destination in the last call (0 after a fallback or failure) |
| `GetTripLength` | the shared float (§4.3) |
| `GetTripLengthScaledForDisplay` | `r24(simulator.GetTripScaleForDisplay() × tripLength)` (traffic simulator slot 15, x87) |
| `GetEndCell(x&, z&)` | the end cell of the last goal test that succeeded (not updated by the fallback or a failure) |
| `GetLastTravelType` | travel type of the result node, 0 if none |

### 4.15 Compressed route (GetCompressedRoute)

`GetCompressedRoute(vector<u8>& out, u32& mask)` clears `out` (end = begin, the buffer is kept) and sets `mask = 0`.
With no result node it returns. Otherwise the route root → result is encoded and appended, and `mask` is the OR of
`1 << t` over all nodes. The format is consumed by `cSC4CompressedRouteReader` (`0x006D64B0` SetData, `0x006D6510`
GetNext), used by the traffic simulator's route confirmation and application, the ferry code, the query tool, the
map view and the network layout view, and it is stored in the trip data saved with lots (traffic-simulator.md).
*[compat]*

* **Header**: the root's type byte (no flags), then its x and z bytes. An off-map root is replaced by the in-city cell
  of the neighbour connection at that point (`GetNeighborConnectionInnerCell`).
* **Steps**: each following node is compared with the previous one. A *unit step* (one cell along x or z) has a code
  equal to its direction: 0 = −x, 1 = −z, 2 = +x, 3 = +z. Unit steps are collected into a *run* of one travel type:
  codes are packed four per byte, least significant bits first (step `i` in bits `2(i mod 4)` of byte `i div 4`).
* A run is flushed as `[t | 0x40, count, ⌈count/4⌉ packed bytes]` (count 1…255) when the next unit step has another
  type, when the run already holds 255 steps (the step then starts a new run), before a jump, and at the end.
* **Jump** (any other move: same cell with a type change, tunnel, ferry, a move of more than one cell): flush, then
  `[t | f, x, z]` of the new node, with `f = 0x80` when the **previous** cell is off-map or has the ferry wormhole bit,
  else 0; an off-map node is written as its connection's in-city cell.
* A type change on a unit step with no pending run starts the new run silently (the run carries the new type).

Unit steps to and from off-map cells are ordinary unit steps (an exit node is one cell from its predecessor).

**Inner cell of an off-map node** (PF10). "Its connection's in-city cell" is `GetNeighborConnectionInnerCell(x, z)`
(`0x0070DDD0`, traffic-network.md §2.7), a clamp onto the map, not a connection lookup: it needs exactly one
coordinate off the map, and it writes nothing and returns −1 otherwise (`0x0070DE01`, `0x0070DE10`).
`GetCompressedRoute` ignores the result and writes the low bytes of its two output variables (`0x006D71E2`,
`0x006D7550`), which then hold stack garbage or the previous call's values. The failure cannot occur: the root is
always in city (ring cells are clamped to the map and ferry-arrival nodes sit on the arrival cell, §4.6), and an
off-map node reached by a jump is a ferry exit whose off-map cell had a valid inner cell when its link was made
(traffic-network.md §4.9.3; only a damaged save could hold a corner cell). *[choice]* FreeCity4 writes the node's
own cell, low bytes.

### 4.16 WasTripReversible

True when there is no result node. Otherwise walk from the result node towards the root over pairs (node `c`, its
predecessor `p`), carrying `e` = the entry side `c` would have when travelled in reverse (−1 = unknown) and `tn` = the
type of the node after `c`:

* If `c` or `p` is off-map, or both are the same cell, or both cells have the tunnel wormhole bit: `e = −1`.
* Otherwise, if `e ≠ −1`: if `tn ≠ c.t`, the cell's `switch[c.t]` nibble `e` must not be `0xF`; if `tn = c.t` and
  `p.t = c.t`, bit `4e + c.side` of `conn[c.t]` must be set (a reverse vehicle entering from `e` may leave towards
  `p`). A failed check returns false. Then `e` is computed from the two cells (`c.x < p.x → 0`, `c.x > p.x → 2`, else
  `c.z ≥ p.z → 3`, else 1), i.e. by the §4.1 definition EdgeTo(`c` → `p`): the side through which a reverse trip
  enters `p`.

Turn rules (sharp turns, car turn bits) and the allowed mask are not checked. The traffic simulator uses the answer
to count a route in both directions (traffic-simulator.md).

### 4.17 ApplyTraffic(style)

Adds the trips of the result to the simulator. Nothing happens when there is no result node or traffic arrived is 0.
`style`: 0 = outbound, 1 = return, 2 = both directions.

* `N = float(trafficArrived)` (u32 converted exactly, then stored as float32). Car count `Nc = N`; if the Carpool
  Incentive ordinance (`0xA2BF1DDC`) is on, `Nc = float32(ceil(double(r24(N × 0.8f))))` (`0.8f` = `0x3F4CCCCD`,
  `ceil` = `0x009F0A20`).
* Walk from the result node to the root, with `prev` = the end cell (as unsigned bytes) for the first node and the
  previous node's cell bytes afterwards. For each node (type `t`, cell `(x, z)`):
  `Q = Nc` if `t` = Car else `N`; `Qi` = `Q` rounded to an integer by the `1.5·2^23` idiom
  (`r24(Q + 12582912.0f)` minus `0x4B400000` on the bit pattern: round half to even, valid for `0 ≤ Q < 2^22`).
  If the cell is in city:
  1. If the node has a predecessor of another type and the cell carries a transit switch: the switch's usage (float)
     `= r24(usage + A)` with `A = r24(Q + Q)` for style 2 else `Q` (the switch record is created if missing).
  2. If the cell has network info for `network(t)` and `generatesTraffic[t]`:
     `vol = r24(vol + Q)`; for style 2 a second `vol = r24(vol + Q)` (two separate roundings).
  3. Edge density: entry side = EdgeTo(predecessor → cell) (the direction from the cell to its predecessor) if there
     is a predecessor, otherwise from the start rect: `x < sx0 → 2`, else `x > sx1 → 0`, else `z ≤ sz1 → 3` else 1.
     Exit side = EdgeTo(prev → cell) (the direction from the cell to `prev`), computed on the **unsigned byte**
     coordinates of `prev` (an off-map −1 compares as 255 *[quirk]*). The simulator's edge-density
     record with key `(style = 1) << 24 | t << 16 | (x & 0xFF) << 8 | (z & 0xFF)` (created if missing) gets
     `current[4·entry + exit] := min(current + Qi, 0xFFFF)`. `current` is the record's **second** u16[16] array, at
     `+0x22` (traffic-simulator.md §4.8: `last[16]` at `+0x00`, `usage` at `+0x20`, `current[16]` at `+0x22`); this
     slot never writes `last` or `usage`, which only the state-7 dump sets (PF2, `0x006DA2D3`). For style 2 the
     record with key `1 << 24 | t << 16 | …` gets the same addition at the **mirrored** index
     `current[4·exit + entry]` (`0x006DA324`–`0x006DA330`), as `ApplyRoute` does (traffic-simulator.md §4.5).
  4. Income: `I = r24(I + r24(float(trafficArrived) × incomePerTile[t]))` (`I` starts at +0.0; this uses the
     uncarpooled count).
* Finally the simulator's double income accumulator `= r24-precision(I + accumulator)` stored as a double (the add is
  done at 24-bit precision, so the accumulator is rounded to a 24-bit mantissa on every call). *[quirk][compat]*

No game caller of slot 29 was identified; the traffic simulator applies saved compressed routes with its own
`ApplyRoute` (traffic-simulator.md). *[inferred]*

### 4.18 Node housekeeping

**ReleasePathNodes** (slot 31): reset the page cursor to the first page (pages are kept), empty all buckets, empty
the open set, forget the result node.

**RecalculateNodeScores** (filter 0 with flag 4, `0x006D6780`): for every stored node in allocation order:
`f = r24(g)` if the cell is inside the search rect (no multiplication by `w`), else `f = r24(r24(H × w) + g)`; open
nodes are then Updated, which only sifts **up** (a raised `f` leaves the heap out of order). *[quirk]* The cell is the
node's **own** cell (`+0x14`/`+0x16`, `0x006D67FF`, `0x006D6825`). For an exit node that is its off-map cell (§4.7),
not the in-city cell at which AddTripNode evaluated `H` (§4.9 step 6), so a recomputed exit node's `H` is usually 1
more or 1 less than before (PF8). Example: end rect `(10, 5, 10, 5)`, search rect not containing the cells, exit
node from `(0, 5)` to `(−1, 5)`: AddTripNode used `H(0, 5) = 10`, the recomputation uses `H(−1, 5) = 11`.

### 4.19 Precision summary

| Quantity | Sequence |
|---|---|
| max cost | `r24(startCost[t] + maxTripLength)` |
| congestion argument | `r24(volume × invCapacity)` |
| step cost (affected) | `r24(r24(Curve(q) × invMaxSpeed) + extraCost)` |
| step cost (not affected) | `r24(invMaxSpeed + extraCost)` |
| exit cost | `r24(c.f − c.g)` |
| g | `r24(cost + parent.g)`; start: `startCost[t]`; ferry-arrival start: `r24(startCost[t] + link.cost)` |
| f | `r24(r24(float(dX + dZ) × w) + g)` or `r24(r24(0.0 × w) + g)` inside the search rect |
| trip length | `r24(goal.g − root.g)`, clamp below at +0.0, then `r24(edgeRecord.cost + L)` (§4.13 step 3) |
| display trip length | `r24(scale × L)` |
| carpool count | `float32(ceil(double(r24(N × 0.8f))))` |
| ApplyTraffic | `Qi` by `1.5·2^23`; volumes `r24(vol + Q)`; income `r24(I + r24(N × incomePerTile))`, then a 24-bit double add |

There are no `_ftol` conversions in the search. Integer heuristic sums are exact in float32 (`< 2^24`).

### 4.20 Random draws

None. The pathfinder uses no random generator, `rand()`, shuffle or random iterator (checked over FindPath, its
helpers, ApplyTraffic and the route functions). Search results depend only on inputs and on the tie rules above;
the only non-determinism is the work-time suspension (§4.11). The helper at `0x0049FC70` that Ghidra calls
`cRZRandom::RandomDoubleRangeUniform` is the heap vector's resize.

## 5. Messages

None posted or handled.

## 6. Persistence

None. The pathfinder is never saved. The compressed route format (§4.15) is persisted by the traffic simulator's trip
data (traffic-simulator.md), which makes the format a save-compatibility requirement. *[compat]*

## 7. Interface: cISC4PathFinder (vtable `0x00AB2150`, 34 slots)

All slots are `__thiscall`. "pop" = bytes the callee pops. Return: EAX (full register), AL (bool/byte; the rest of
EAX is garbage), x87, or none (EAX garbage). Pop counts checked with `tools/abi_extract.py --check`; return channels
read from the epilogues.

| Slot | Off | Address | Signature (as compiled) | pop | Return | Behaviour |
|---|---|---|---|---|---|---|
| 0 | +0x00 | `0x006D5D20` | `bool QueryInterface(uint32 iid, void** out)` | 8 | AL | IIDs `1`, `0xAA0DB766`; AddRef through the vtable |
| 1 | +0x04 | `0x004BDF00` | `uint32 AddRef()` | 0 | EAX | shared framework AddRef |
| 2 | +0x08 | `0x006D61B0` | `uint32 Release()` | 0 | EAX | §1 |
| 3 | +0x0C | `0x006D78B0` | `bool Init()` | 0 | AL | §1 |
| 4 | +0x10 | `0x006D5D50` | `bool Shutdown()` | 0 | AL | §1 |
| 5 | +0x14 | `0x006D6CF0` | `void SetFlag(uint32 flags)` | 4 | none | OR into flags |
| 6 | +0x18 | `0x006D6D00` | `void ClearFlag(uint32 flags)` | 4 | none | |
| 7 | +0x1C | `0x006D6D20` | `void ClearAllFlags()` | 0 | none | |
| 8 | +0x20 | `0x00407AD0` | `void SetRouteInfo(deque<RouteData>* route)` | 4 | none | folded generic setter; null = no route |
| 9 | +0x24 | `0x006D6D30` | `void SetTripDestinationFilter(uint32 filter)` | 4 | none | |
| 10 | +0x28 | `0x006D6D40` | `void SetDestinationFlag(uint8 bits)` | 4 | none | OR |
| 11 | +0x2C | `0x006D6D60` | `void ClearDestinationFlag(uint8 bits)` | 4 | none | |
| 12 | +0x30 | `0x006D6D80` | `void ClearAllDestinationFlags()` | 0 | none | |
| 13 | +0x34 | `0x006D6D90` | `void SetTravelTypeFlag(uint16 mask, bool alsoStarting)` | 8 | none | §4.2 |
| 14 | +0x38 | `0x006D6DB0` | `void ClearTravelTypeFlag(uint16 mask)` | 4 | none | clears both masks |
| 15 | +0x3C | `0x006D6DD0` | `void ClearAllTravelTypeFlags()` | 0 | none | |
| 16 | +0x40 | `0x006D6DE0` | `void SetStartingCostArray(float* costs9)` | 4 | none | stores the pointer |
| 17 | +0x44 | `0x006D6DF0` | `void SetMaxTripLength(float)` | 4 | none | shared with the result (§4.3) |
| 18 | +0x48 | `0x006D6E00` | `void SetTrafficDensity(uint32 trips)` | 4 | none | |
| 19 | +0x4C | `0x006D6E10` | `void SetStartRect(SC4Rect<int32> r)` (by value: x0, z0, x1, z1) | 16 | none | inclusive |
| 20 | +0x50 | `0x006D6E40` | `void SetEndRect(SC4Rect<int32> r)` | 16 | none | inclusive |
| 21 | +0x54 | `0x006D6E70` | `void SetMaxWorkTime(uint32 ms)` | 4 | none | 0 = unlimited |
| 22 | +0x58 | `0x006D6E80` | `void Reset()` | 0 | none | clears "suspended" only |
| 23 | +0x5C | `0x006D91B0` | `uint32 FindPath()` | 0 | EAX | 0 / 1 / 2 (§4.3) |
| 24 | +0x60 | `0x00698CE0` | `uint32 GetTrafficArrived()` | 0 | EAX | folded getter |
| 25 | +0x64 | `0x006D6E90` | `float GetTripLength()` | 0 | x87 | |
| 26 | +0x68 | `0x006D6030` | `float GetTripLengthScaledForDisplay()` | 0 | x87 | |
| 27 | +0x6C | `0x006D6EA0` | `void GetEndCell(int32& x, int32& z) const` | 8 | none | |
| 28 | +0x70 | `0x006D6EC0` | `uint8 GetLastTravelType() const` | 0 | AL | upper EAX = pointer bits or 0 |
| 29 | +0x74 | `0x006DA060` | `void ApplyTraffic(uint32 style)` | 4 | none | §4.17; the `abi_extract` "x87" guess is wrong (the final `fstp` empties the stack) |
| 30 | +0x78 | `0x006D61E0` | `bool WasTripReversible()` | 0 | AL | §4.16 |
| 31 | +0x7C | `0x006D6730` | `void ReleasePathNodes()` | 0 | none | §4.18 |
| 32 | +0x80 | `0x006D7070` | `void GetCompressedRoute(vector<uint8>& out, uint32& mask)` | 8 | none | §4.15 |
| 33 | +0x84 | `0x006D7050` | scalar deleting destructor `(uint32 flags)` | 4 | EAX = this | bit 0: free with allocator C |

`RouteData` is 3 bytes `{uint8 x, uint8 z, uint8 travelType}`; the deque is the STLport `std::deque` with node
buffers of `0x7E` bytes (42 elements).

**The route deque's layout** (TG1; read from the fill at `0x006D8808`–`0x006D8896`, `clear` `0x006DB440` and the map
regrowth `0x004321B0`). The caller owns a `0x28`-byte object of ten 32-bit words; a replacement pathfinder must fill
it in place, because the callers walk it with their own inlined STLport code:

| Offset | Field |
|---|---|
| `+0x00`, `+0x04`, `+0x08`, `+0x0C` | start iterator: current element, its node buffer's first and end (`first + 0x7E`), its map slot |
| `+0x10`, `+0x14`, `+0x18`, `+0x1C` | finish iterator, the same four fields (current = one past the last element) |
| `+0x20` | the map: an array of node-buffer pointers |
| `+0x24` | the map's size in pointers |

Node buffers and the map are allocated with allocator B (`0x0090CF54`) and freed with `0x0090CF63`
(docs/abi/allocators.md). The fill (§4.13 step 5) is:

1. **clear**: free the node buffers of every map slot strictly between the start and finish slots, and the finish
   slot's buffer when it differs from the start slot's; then finish := start. The start buffer and the start
   iterator (its current element included) are kept, so a cleared deque is empty with `start.cur` where it was.
2. For each node from the result back to the root, **push_front** its 3 bytes: if `start.cur ≠ start.first`,
   `start.cur −= 3` and store there. Otherwise, if no map slot is free before `start.node`
   (`(start.node − map) / 4 < 1`), regrow the map for one node at the front (below); allocate a new `0x7E`-byte
   buffer, store it in the slot before `start.node`, move `start.node` there, set `first` and `last := first + 0x7E`,
   `cur := last − 3` and store the element there.
3. **Map regrowth** (`0x004321B0`, called with 1 node, at the front): `used := (finish.node − start.node) / 4 + 1`,
   `need := used + 1`. If `mapSize > 2·need`, the used slots are moved within the map so that they start at slot
   `(mapSize − need) / 2 + 1`. Otherwise a new map of `mapSize + max(mapSize, 1) + 2` slots is allocated, the used
   slots are copied to slot `(newSize − need) / 2 + 1`, and the old map is freed. Both iterators' node fields are
   updated (their `first` and `last` are re-read from the slots).

Callers construct and destroy the deque themselves (with their own STLport code); the pathfinder only clears it and
pushes to the front. A replacement that writes through these rules leaves a deque the callers can walk and free.

**SDK comparison.** There is no SDK header for `cISC4PathFinder`, so there are no SDK errors to flag. The SDK's
`cISC4TrafficSimulator.h` declares `CreatePathFinder(cISC4PathFinder**)` and `SetupPathFinderForLot(cISC4PathFinder*,
cISC4Lot*)` with an opaque forward declaration only; the table above is the authoritative layout (names from the Mac
symbols; enum parameters are 32-bit).

## 8. Dependencies

Virtual calls on other interfaces (compiled slot, offset, pop, return):

| Interface | Slot (offset) | Method | pop | Return | Used for |
|---|---|---|---|---|---|
| `cISC4City` | 93 (+0x174), 94 (+0x178) | CellCountX, CellCountZ | 0 | EAX | Init |
| `cISC4LotManager` | 6 (+0x18) | `GetLot(int x, int z, bool originOnly)` (`0x0072B410`), always called with **false** (any cell of the lot; zones.md §8 slot 6) at all three sites: `0x006D8B1D` (lot start), `0x006D7AC7` (filter 0 goal), `0x006D8688` (§4.5 step 5) | 12 | EAX | lot start, destination lots |
| `cISC4Lot` | 3 (+0x0C) | `AsPropertyHolder()` | 0 | EAX | capacity check |
| `cISC4Lot` | 14 (+0x38) | `GetBoundingRect(SC4Rect<int32>&)` (`0x006BF320`) | 4 | AL | end rect, clearing destinations |
| `cISC4OccupantManager` | 40 (+0xA0) | `GetOccupantsByStandardCityCells` filter overload (`0x0049E250`) | 20 | AL | filter 2 |
| `cISC4OccupantFilter` | 3 (+0x0C) | `IsOccupantIncluded(cISC4Occupant*)` | 4 | AL | filter 2 |
| `cISC4Occupant` | 2 (+0x08) | Release | 0 | EAX | filter 2 |
| `cISC4OrdinanceSimulator` | 8 (+0x20) | `IsOrdinanceOn(0xA2BF1DDC)` (`0x0080D470`) | 4 | AL | carpool (ApplyTraffic) |
| `cISC4TrafficSimulator` | 15 (+0x3C) | GetTripScaleForDisplay (`0x0071FFC0`) | 0 | x87 | display trip length |
| `cISC4TrafficSimulator` | 20 (+0x50) | `GetMaxTripCapacity(cISCPropertyHolder*, uint8 k)` (`0x0070DF90`) | 8 | EAX | capacity check |
| `cISC4SimGrid<T>` | 24 (+0x60) | GetGridData (`0x006A9750`) | 0 | EAX | the six volume grids (float), the transit-switch map (u32), and one more grid of the simulator whose data is fetched but unused *[inferred]* |

Non-virtual calls into `cSC4TrafficSimulator` (Windows `__thiscall` on the owner; traffic-network.md and
traffic-simulator.md must specify their results):

| Address | Function | Arguments → result |
|---|---|---|
| `0x0070FB30` | GetNetworkInfo | `(transitNetwork, x, z)` → network info pointer or null |
| `0x0071CB20` | TransitSwitchEntryCost | `(switchId)` → float (x87) |
| `0x0070DFF0` | used trip capacity | `(holder, k)` → u32 |
| `0x007114E0` | GetNeighborPoolPropertyHolder | `(t, x, z)` → holder or null (traffic-network.md §2.7.1) |
| `0x0070F970` | GetTrafficNeighborConnection | `(x, z)` → the cell's edge record (f32 cost at +4), never null (traffic-network.md §2.7.1) |
| `0x0070DDD0` | GetNeighborConnectionInnerCell | `(x, z, &ix, &iz)` |
| `0x006D90F0`, `0x006D7820` | transit-switch map and edge-density map `operator[]` | insert-if-missing |

Direct member reads of the owner (Windows layout, owned by traffic-network.md / traffic-simulator.md): the cell
connection array, destination bitmask map and free counts, tunnel and ferry hash tables, ferry terminal list (§4.6),
volume-grid vector, transit-switch grid, strike state, the per-travel-type tables (default start cost, can reach,
generates traffic, income per tile), MaxDistanceOriginToNetwork, NearestDestinationAttractiveness, the income
accumulator; class statics `invMaxSpeed[9][13]` (`0x00B4B110`), `affectedByTraffic[9]` (`0x00B4B100`), the
congestion curve (`0x00B4B2F0`, evaluated with `cSC4ResponseCurve::GetOutputValue` `0x005F4250`). Globals: city
`0x00B43CEC`, lot manager `0x00B43D08`, occupant manager `0x00B43D0C`, ordinance simulator `0x00B43D90`.

Shared helpers used: `BoundaryIterator::GetNext` (`0x0060CEE0`, order in §4.6), rect-contains (`0x0043FDE0`,
inclusive), `LengthOfLine` (`0x005D7660`).

## 9. Replaceability

**Replacement unit** *[choice]*: the pathfinder and the traffic simulator (traffic-simulator.md) are one unit.

* **Not replaceable on its own.** The class is not registered with GZCOM; it is created with `new` inside
  `cSC4TrafficSimulator::CreatePathFinder` and it reads the traffic simulator's members and calls its non-virtual
  functions (§8). A replacement pathfinder therefore comes with a replacement traffic simulator (or a hook of
  CreatePathFinder that also provides the simulator internals). A replacement traffic simulator, conversely, must
  supply pathfinders through slot 6 that behave as specified here, because other subsystems use them.
* **External users of the interface** (virtual calls only, through CreatePathFinder): traffic automata
  (`cSC4TrafficAutomaton` route start and best neighbour connection `0x00554EA0`; automata manager dispatch), emergency
  vehicles (`0x00532660` and dispatch), MySim dispatch, the query tool (`0x004CE8A0`, `0x004D12A0`). Their typical
  setup: start rect, `SetTravelTypeFlag(2, true)`, density 0, flags 1 and 2, filter 3, a route deque, end rect,
  `FindPath`, `Reset`. They rely on the route deque contents and on the fallback (flag 2).
* **Non-virtual users of the route format**: `cSC4CompressedRouteReader` (`0x006D64B0`, `0x006D6510`) is called by
  the traffic simulator (`ConfirmRoute` `0x0070E320`, `ApplyRoute` `0x00719910`), the ferry code
  (`cSC4Watercraft` `0x0056F3F0`, `0x0056F510`, `0x00570620`; `cSC4WatercraftManager::CreateFerryStruct`
  `0x0056FD60`), the query tool (`0x004C3860`, `0x004C4AE0`, `0x004C5E20`), the map view (`0x007A2F60`) and the
  network layout view (`0x0060C0A0`). The byte format of §4.15 is binding.
* **Memory reached into**: nobody outside the class reads the pathfinder's own memory (the traffic simulator only
  stores itself in it at creation). The pathfinder reads the traffic simulator's grids raw (`GetGridData` on the six
  volume grids and the transit-switch map, simgrid.md §6.4) and writes the destination bitmask map and the free
  destination counts (§4.10.1).
* **Containers crossing the interface**: the route deque (caller-owned, filled with allocator B), the compressed route
  vector (caller-owned, allocator B), the filter-2 occupant list (allocator A, internal to the call). Internal only:
  node pages (allocator C), bucket array, open-set vector, distance tables, the FindNearestStandardDest candidate
  vectors and the compressed-route node array (allocator B).

## 10. Quirks and open questions

Quirks reproduced (all marked above): shared max-trip-length/trip-length float; strike blocks subway stations and
ferries but not starts or same-type moves; subway-station transfer uses the unmasked connection shift and ignores the
allowed mask; switch moves skip the map-bounds check; exit cost = heuristic term; clamped start rings re-push nodes, and so does a repeated
ferry-arrival match (§4.6);
the goal node is pushed back on success; RecalculateNodeScores only sifts up; filter 3 with flag 4 keeps stale
scores; the excluded edge rect only exists for filter 0; the 65th over-budget rejection disables a travel type for
the call; a NaN `g` (new or stored) passes the "better" test (§4.9 step 5); ApplyTraffic's exit side uses unsigned bytes; the income double is
rounded to 24 bits; filter 0 overwrites the caller's end rect.

Open questions:

* **Q1** The destination type reached (used by §4.13 step 3 for filter 3) is never initialised by the constructor.
  The original reads heap garbage on a fresh object for an off-map filter-3 goal. FreeCity4 initialises it to 0 (so
  the connection cost is added) *[choice]*; a replay of the original could show the other behaviour.
* **Q2** The occupant filter (filter 2) has no setter; filter 2 looks unusable *[inferred]*. Candidate call sites of
  `SetTripDestinationFilter(2)` found by a byte scan (`0x00545328`, `0x0060F588`, `0x006A1D10`, `0x0070E0BE`) are other
  interfaces (stream writes), not the pathfinder.
* **Q3** No caller of `ApplyTraffic` (slot 29) was found *[inferred]*. The simulator applies routes with its own
  `ApplyRoute` (`0x00719910`, traffic-simulator.md). Both mirror style 2 (the second add uses index
  4·exit + entry; corrected by PF2, an earlier version said this slot does not mirror). `ApplyRoute` adds
  transit-switch usage once, this slot `r24(Q + Q)` for style 2. For the first route cell `ApplyRoute` tests
  `z < sz0 → 3` where this slot tests `z ≤ sz1 → 3`; the two agree for cells outside the rect.
* **Q4** A switch move towards an off-map cell reads the transit-switch map and the network info outside the map; the
  result there depends on the simulator's accessor bounds handling (traffic-network.md). FreeCity4 treats it as "no
  switch, no info" *[choice]*.
* **Q5** *Answered* (traffic-simulator.md): destination types 0, 1, 2 are commuters of wealth $, $$, $$$ (jobs of
  that wealth) and 3 is freight; property `0x8A0B4100` holds u16[12] = max [0..3], arrived this cycle [4..7]
  (`0x0070DFF0` reads it as "used"), arrived last cycle [8..11].
* **Q6** One grid of the simulator is fetched with GetGridData at the start of the main loop and the result is not
  used *[inferred]*.

## 11. Conformance tests

Tests 2–7 use a stub traffic simulator with an 8 × 8 city (W = H = 8), max work time 0, `invMaxSpeed[Car][Street] =
1.5`, `affectedByTraffic[Car] = true`, a congestion curve with `Curve(0.0) = 1.0`, all volumes 0, NearestDestination
Attractiveness 1.0, MaxDistanceOriginToNetwork 1, `canReachDestination[Car] = generatesTraffic[Car] = true`,
`incomePerTile[Car] = 0`, no strike, no carpool unless stated. Cells `(0,3)…(4,3)` are a street: road-network info
`{invCapacity 0.0625, type 3 (Street), extraCost 0.0}`, `conn[Car] = 0x0104` (entered from −x: leave +x; entered from
+x: leave −x), `switch[Car] = 0xFFFF`, `priorCar[0…3] = 0xFFFF`, wormhole byte 0, no transit switches. All other
cells and networks are empty. Start rect `(2,2,2,2)` with a lot at `(2,2)`; end rect `(4,4,4,4)`; filter 3;
`SetTravelTypeFlag(0x2, true)` only; starting cost array all 0.0; route deque set; density 10.

1. **Open-set tie order.** Push nodes with `f` = a 5, b 3, c 3, d 1, e 3 (in this order) into the heap of §4.7; the
   pop order is d, e, b, c, a.
2. **Straight street.** Flags 1. Start nodes: only `(2,3)` (ring-1 cells visited `(2,1)`, `(1,2)`, `(3,2)`, `(2,3)`;
   only `(2,3)` has a network), sides 0 and 2, `g = f = 0`, pushed in that order. Pops: `(2,3)` side 0 → adds `(3,3)`
   side 0, `g = 1.5`; `(2,3)` side 2 → adds `(1,3)` side 2, `g = 1.5`; `(3,3)` → adds `(4,3)` side 0, `g = 3.0`;
   `(1,3)` → adds `(0,3)` side 2, `g = 3.0`; `(4,3)` is a goal (its +z neighbour `(4,4)` is in the end rect).
   `FindPath` = 1; trip length `3.0` (`0x40400000`); end cell `(4,4)`; traffic arrived 10; last travel type 1;
   route deque `(2,3,1) (3,3,1) (4,3,1)`; `WasTripReversible` = true.
3. **Compressed route** of test 2: bytes `01 02 03 41 02 0A`, mask `0x00000002`.
4. **ApplyTraffic(0)** after test 2: road volume `(2,3)`, `(3,3)`, `(4,3)` = 10.0 each; edge density, all in the
   `current` array (`+0x22`), with style bit 0 and type 1: cell `(4,3)` `current[4·0 + 3] = 10`, `(3,3)`
   `current[4·0 + 2] = 10`, `(2,3)` `current[4·1 + 2] = 10` (root: entry from the start rect = side 1); `last` and
   `usage` stay 0. With style 2 instead: volumes 20.0, the same three counts in the key-bit-0 records, and the
   mirrored counts in the key-bit-1 records: `(4,3)` `current[4·3 + 0] = 10`, `(3,3)` `current[4·2 + 0] = 10`,
   `(2,3)` `current[4·2 + 1] = 10`. With the carpool ordinance on and density 10: `Q = 8.0`
   (`r24(10 × 0.8f) = 8.0`, ceil 8), volumes 8.0, counts 8.
5. **Starting cost.** As test 2 but starting cost `[0, 0.4f, 0, …]`: start `g = 0.4f` (`0x3ECCCCCD`), `(3,3)`
   `g = 1.9f` (`0x3FF33333`), `(4,3)` `g = 3.4000001f` (`0x4059999A`), trip length `r24(3.4000001f − 0.4f) = 3.0`
   (`0x40400000`).
6. **Budget and shared trip length.** As test 2 without flag 1 and `SetMaxTripLength(2.0)`: the nodes with `g = 3.0`
   are rejected (over-budget count 2 for Car); `FindPath` = 0; `GetTripLength` = 2.0 (the max, unchanged). With flag 2
   added: fallback to the stored node nearest the end rect, `(3,3)` (distance √2 against √5, √5, √10); `FindPath` = 1,
   trip length 1.5, route `(2,3,1) (3,3,1)`, traffic arrived 0, end cell unchanged. After test 2 (success), a second
   `FindPath` without calling `SetMaxTripLength` uses `maxCost = 3.0`.
7. **Over-budget disable.** Any search in which 65 nodes of type `t` fail the budget test: the 65th rejection
   disables `t`; later moves of type `t` in that call are ignored even when within budget.
8. **Boundary order.** Ring rect `(1,1)-(3,3)`: `(1,1) (2,1) (3,1) (1,2) (3,2) (1,3) (2,3) (3,3)`. Ring rect
   `(4,0)-(4,2)` (one column): `(4,0) (4,1) (4,2)`.
9. **Standard destination order.** 16 × 16 city, start rect `(5,5,5,5)`, destination flags bit 0, free count 1,
   destination bit 0 at `(4,6)` and `(6,4)` (lots of one cell each). Ring 1 scans the left column `x = 4`, z 4…6
   first, finds `(4,6)`, then `(6,4)` in the right column; both at Manhattan distance 2; the end rect becomes the lot
   at `(4,6)`.
10. **Heuristic.** Start rect `(2,2,2,2)`, end rect `(12,3,12,3)`, `w = 0.09f` (`0x3DB851EC`): `H(10,3) = 2`,
    `r24(2 × 0.09f) = 0x3E3851EC`; `H(6,3) = 0` (inside the search rect `(−2,−2)-(6,6)`).
11. **Compressed route with jumps.** Node chain (cell, type): `(5,5,0) (6,5,0) (6,5,6) (6,6,6) (6,7,6) (6,9,6)
    (7,9,6)`, no ferry bits: bytes `00 05 05 40 01 02 06 06 05 46 02 0F 06 06 09 46 01 02`, mask `0x41`.
12. **Run length limit.** 300 consecutive unit steps of type 1, all with code 2: `41 FF` + 63 × `AA` + `2A`, then
    `41 2D` + 11 × `AA` + `02`.
13. **Check-interval adaptation.** Fresh process, max work time large: 16 consecutive `FindPath` calls that each
    expand at least 21 nodes (with `m = 1024`, `k ≥ 21`) leave `m = 512`; then four consecutive calls that each
    expand 2–9 nodes (`m = 512`, so `1 ≤ k ≤ 4`) restore `m = 1024`. A call expanding 0 or 1 node (`k = 0`) changes
    nothing.
14. **Lifetime.** `CreatePathFinder` returns an initialised object with count 1; one `Release` returns 0 and destroys
    it; with count 2, `Release` returns 1.
