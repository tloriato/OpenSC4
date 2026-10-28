# Traffic network model (behaviour spec)

Status: M5 behaviour spec. Every fact was checked on the Windows executable `bin/SimCity4.exe` (1.1.641, image base
`0x00400000`), and every address is a Windows address. The earlier write-up in `docs/subsystems/traffic.md` §2.4, §3
and §8 was the starting point (mostly Mac). Corrections to it are listed in §13. Implementers work from this spec, the
public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled or disassembled
code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Tags:

- *[compat]*: observable; a replacement must match it.
- *[choice]*: a FreeCity4 decision.
- *[inferred]*: consistent with all the evidence, but not proven.
- *[quirk]*: original behaviour that looks unintended but is reproduced.

`r24(e)` is `e` rounded to float32 (foundations §18: the x87 unit runs at 24-bit precision, so every add, subtract,
multiply, divide and square root rounds to float32).

Related specs (cited, not repeated):

- [traffic-simulator.md](traffic-simulator.md): the owning class, its agent, work cycle, trip generation, tuning
  exemplar, save record and full slot table.
- [pathfinder.md](pathfinder.md): the search that walks the graph defined here. It also defines the compressed route
  format, §4.15.
- [foundations.md](foundations.md): §3.6 city components, §4 response curves, §5 messages, §6 random generators,
  §16 slot order, §18 precision.
- [simgrid.md](simgrid.md): grid classes; §6.4 grid ownership rule; §7 shared helpers.
- [simulator.md](simulator.md): work slicing.
- [neighbors.md](neighbors.md): neighbour connections.
- [landfill.md](landfill.md) §4.7.2: a caller of the subnetwork queries.
- [docs/abi/allocators.md](../abi/allocators.md).

---

## 1. Role and identity

The traffic simulator (`cSC4TrafficSimulator`) keeps a **cell graph** of the city's transport networks. This spec
covers that graph and everything built on it:

- **Network info lists**: for each cell, at most one record per *transit network*. Each record holds the capacity,
  the network type, the extra step cost and the subnetwork labels.
- **Cell connection records**: for each cell and each of the 9 travel types, which exit sides may be used from which
  entry side. Each record also holds the transit-switch type changes, the car lane-continuity bits and the
  *wormhole* bits.
- **Transit switches**: stations and other lots that change the travel type.
- **Tunnels**.
- **Ferry terminals**, together with their water routes and the **water pathfinder** that computes those routes.
- **Subnetworks**: connected components of the car graph, with per-subnetwork population and destination capacity.
- The **queries** built on these structures (traffic simulator slots 16, 22, 23, 25, 28–33, 36–42).

All of this lives inside the traffic simulator object. It is **not** a separate class and has no class ID of its own.

| Fact | Value |
|---|---|
| Owner | `cSC4TrafficSimulator`, class `0x6990C1AA`, city component #37, IID requested on read `0x68027F37` (foundations §3.6) |
| Interface | `cISC4TrafficSimulator`, vtable `0x00AB33D0` (43 slots in docs/abi; slot table in traffic-simulator.md, our slots in §7) |
| Water pathfinder | `cSC4WaterPathFinder`, an internal object of `0x305C` bytes (constructor `0x007250F0`, Init `0x00725490`). It is not a GZCOM class and is not reachable through any interface. |
| Cell connection array | fixed 256 × 256 records, indexed `(x << 8) | z`, allocated with the global heap (allocator C) |

**Init** (traffic simulator `Init`, `0x0071E7E0`; its order belongs to traffic-simulator.md). The parts that concern
this spec:

1. City size `X = CellCountX`, `Z = CellCountZ` (city slots 93 and 94). Max indices `X−1`, `Z−1`. City bounds
   rectangle `[0, X−1] × [0, Z−1]`.
2. Network info grid: X × Z empty lists.
3. Destination bitmask map: `(X+2) × (Z+2)` bytes (owned by traffic-simulator.md and pathfinder.md §4.10).
4. Cell connection array: every one of the 65 536 records gets `conn[t] = 0`, `switch[t] = 0xFFFF` and
   `wormhole = 0`. The original does not initialise the car-continuity words (§2.1). FreeCity4 sets them to
   `0xFFFF` *[choice]*. No reader sees the difference, because every record of a city cell is rewritten by the first
   full rebuild (§4.2) before any search runs.
5. Transit-switch maps: two grid objects, allocated and constructed in place. Each gets `SetTractSize(1)` and an
   **instance ID** through grid slot 6 `SetInstanceID` (`0x0071EF56`, `0x0071EFD2`):
   - a `uint8` *switch mode mask* grid (element class `0x49B9E602`), instance ID `0x69D5C424`;
   - a `uint32` *switch ID* grid (element class `0x49B9E606`), instance ID `0x69D5C42D`.

   Both IDs are instance IDs, not classes (TN13). `DoLotActivated` writes the mode mask into the first grid
   (`0x00723E2B`) and the switch ID into the second (`0x00723E1F`). The *wormhole* bits are a field of the cell
   connection record (§2.1), not a grid. The name "wormhole byte map" for `0x69D5C424` (traffic-simulator.md,
   save-records.md) was wrong and has been corrected there.
6. Water pathfinder: constructed, then `Init`. Init creates its depth grid and subscribes it to messages (§4.10).
7. Next switch ID = 1 (set by the constructor).

The full rebuild runs in **DoPostCityInit** (message `0x26D31EC1`, `0x00720430`), in this order *[compat]*:

1. Re-read the rules of every loaded transit switch (§4.7.2).
2. Refresh the switches' subway access points (§4.7.2, "Access points after the re-read").
3. Rebuild the whole city with `DoConnectionsChanged(0, 0, X−1, Z−1)` (§4.2).
4. Register the neighbour connections: Add for each of them, then the pool sweep (§2.7.2, §2.7.3).
5. The destination-map boundary pass (`0x00720660`–`0x007206B9`): for every cell of the ring around the map
   (x ∈ {−1, X} or z ∈ {−1, Z}, corners included), if `HasMapExit` (§2.7) is false, the cell's destination byte
   is set to 0. `freeDest` is not changed.
6. Clear the subnetwork labels (§4.11).
7. Run `UpdateSubnetworks` (§4.11), then build the subnetwork info synchronously with no time limit (§4.12) and
   publish it.
8. **Only when the loaded work-cycle state is 19** (FerryRoutes): mark every ferry terminal for re-routing and start
   the ferry-route pass at the first terminal (§4.9.3). *(Corrected, TS7: this is one case of a switch on the loaded
   cycle state, `0x00720A0A`; in any other state nothing happens to the ferries at load.)* The simulator-side steps
   that follow (cycle resume, strategy buckets, pacing, department costs) are traffic-simulator.md §3.5.

---

## 2. State

### 2.1 Cell connection record (one per cell, 256 × 256)

| Field | Type | Meaning |
|---|---|---|
| `conn[t]`, t = 0…8 | u16 | **Connection word** of travel type `t`. Nibble `e` (bits `4e … 4e+3`) belongs to the **entry side** `e`. Bit `x` of that nibble is set when a type-`t` traveller that entered the cell from side `e` may leave through side `x`. Bit index = `x + 4e`. |
| `switch[t]`, t = 0…8 | u16 | **Switch word**. Nibble `x` = the travel type a type-`t` traveller becomes when it leaves through side `x`. `0xF` = no change. Written only by transit switches (§4.7.4). |
| `priorCar[e]`, e = 0…3 | u16 | **Car lane continuity**. Word `e` belongs to the entry side `e` of this cell. Bit `p + 4x` set = a car that entered this cell from side `e` may leave through side `x` when it entered the *previous* cell (the one through side `e`) from side `p`. The pathfinder checks it for travel types on the car network when the node has a predecessor (pathfinder.md §4.8). |
| `wormhole` | u8 | Bit 1 = subway station (read by the pathfinder, **never set**, §10 Q1). Bit 2 = tunnel portal. Bit 4 = ferry terminal. |

**Sides**: 0 = west (x−1), 1 = north (z−1), 2 = east (x+1), 3 = south (z+1). The neighbour through side `d` is
`(x + DX[d], z + DZ[d])` with `DX = {−1, 0, 1, 0}` and `DZ = {0, −1, 0, 1}`. The side of the neighbour that faces
back is `(d + 2) mod 4`.

**Travel types** `t` (SDK `TravelType`): 0 Walk, 1 Car, 2 Bus, 3 PassengerTrain, 4 FreightTruck, 5 FreightTrain,
6 Subway, 7 ElevatedTrain, 8 Monorail.

**Transit networks** `n`:

| n | Network | Travel types on it (`net(t)`) | Path type `pt(n)` used to ask occupants | Representative travel type `rep(n)` |
|---|---|---|---|---|
| 0 | pedestrian | 0 | 2 | 0 |
| 1 | car | 1, 2, 4 | 1 | 1 |
| 2 | train | 3, 5 | 3 | 3 |
| 3 | subway | 6 | 4 | 6 |
| 4 | elevated (light) rail | 7 | 6 | 7 |
| 5 | monorail | 8 | 7 | 8 |

`net(t)` for t = 0…8 is {0, 1, 1, 2, 1, 2, 3, 4, 5}.

### 2.2 Network info (per cell, per transit network)

Each cell has a list of network-info records, at most one per transit network, in creation order. Each record holds:

| Field | Type | Meaning |
|---|---|---|
| `invCap` | f32 | Inverse capacity: `invCapTable[type]` (§4.5), possibly reduced near intersections (§4.6). |
| `connCount` | u8 | Number of connections counted at the last rebuild (§4.2), truncated to 8 bits. |
| `type` | u8 | Network type 0…12 (eNetworkType: 0 Road, 1 Rail, 2 Highway, 3 Street, 4 WaterPipe, 5 PowerPole, 6 Avenue, 7 Subway, 8 LightRail, 9 Monorail, 10 OneWayRoad, 11 DirtRoad, 12 GroundHighway). |
| `net` | u8 | Transit network `n` 0…5. |
| `flags` | u8 | Bit 0 = an occupant here has network flag `0x40000` (meaning unknown; carried for the simulator). |
| `extraCost` | f32 | Extra step cost: the "damaged road extra step cost" on a damaged car cell, else 0.0. |
| `subnet[e]`, e = 0…3 | u16 | Subnetwork label per **entry side** (§4.11). 0 = unlabelled. Car network only. |

The pathfinder reads `invCap`, `type` and `extraCost` through `GetNetworkInfo(n, x, z)` (non-virtual `0x0070FB30`).
It returns the first record of network `n` in the cell's list, or none.

**Creation quirk** *[quirk]*. Appending a new record at the tail of a cell's list also clears the `flags` byte of the
record that was the tail before it. In practice the rebuild (§4.2) creates the records of a cell in network order and
sets each `flags` right after creating it. The quirk is visible only when one cell gets records from two occupants for
the same network (the second occupant reuses the existing record), or from the ferry phase. *[compat]* where it
changes `flags`.

### 2.3 Other members

| Member | Persisted | Initial | Meaning |
|---|---|---|---|
| network info grid | no | empty | rebuilt by §4.2 at load |
| cell connection array | no | see §1 | rebuilt by §4.2 at load |
| switch ID grid (`uint32`, instance ID `0x69D5C42D`) | yes (traffic save) | 0 | switch ID per cell covered by a transit-switch lot |
| switch mode-mask grid (`uint8`, instance ID `0x69D5C424`) | yes | 0 | OR of `1 << from` over the switch's kind-0 rules |
| next switch ID | yes | 1 | incremented once per created switch |
| transit-switch table | yes (each record, traffic-simulator.md) | empty | SGI `hash_map<u32 id, TransitSwitch>`, bucket hint 100 |
| tunnel table | yes (corrected, traffic-simulator §6.1 row 17) | empty | SGI `hash_map<u16 portalKey, TunnelInfo>`, hint 100. Rebuilt from occupant-insert messages. |
| ferry terminal list | no | empty | list of FerryTerminal (§2.6) |
| ferry departure / arrival tables | no | empty | two SGI `hash_map<u16 cellKey, list position>`, hint 100 each |
| ferry terminal count per ferry type [2] | no | 0 | at most 16 terminals per type. Only types 0 and 1 have a word (`+0x128`, `+0x12C`); type 2's word is another member's pointer (§4.9.1, §4.9.5, TN18) |
| ferry pass state | no | idle | current terminal, inner position, neighbour-cell vector, phase (§4.9.3) |
| subnetwork counter | no | 1 | next label to assign (u16) |
| subnetwork info, working and published | no | empty | vectors of SubnetworkInfo (§4.12) |
| `invCapTable[13]`, `rawCapTable[13]` | no | 0 (`invCapTable` is not set by the constructor) | §4.5 |
| intersection-effect count `N` and values `[N]` | no | 0 / empty | §4.6 |
| damaged-road extra step cost | no | 32.0 | §4.5 |
| intersection dirty rectangle and flag | no | clear | §4.6 |
| "subnetworks stale" flag | no | — | set by every rebuild; the work cycle reacts to it (traffic-simulator.md) |
| "network changed" flag | no | — | set at Init and by every rebuild; consumed by traffic-simulator.md *[inferred]* |

**Keys.** A cell key is the u16 `(x << 8) | z`. Only the low 8 bits of each coordinate are used.

### 2.4 Transit switch

| Field | Meaning |
|---|---|
| building | the occupant that carries the rules (one reference held) |
| load | f32, starts at 0.0. Traffic through the switch, maintained by the pathfinder and simulator. |
| time factor | f32, building property `0xE90E25A2` |
| capacity | f32, building property `0xE90E25A3` |
| flags | u8. From the "to" types of **all** rules, whatever their kind (§4.7.2, TN19): 2 → 1, 3 → 2, 6 → 4, 7 → 8, 8 → 0x10. Plus 0x40 when a rule links subway and elevated train. |
| has access point, access cell | u8, u8 x, u8 z: the subway access point (§4.7.1) |
| two f32 counters | saved, owned by traffic-simulator.md |
| rules | vector of Rule records (below) |
| kind-0 "to" mask | u16, OR of `1 << to` over the kind-0 rules |
| toll | f32, building property `0x0BFC0A4C`, read with the converting f32 getter (`0x00723C43`; absent → 0.0). The fee per unit of traffic that passes the switch (traffic-simulator.md §4.5), summed into the toll income. Shipped only on the four toll booths, all 0.1. |
| counts as intersection | bool, building property `0x4BFC47B0` (§4.6) |

**Rule**: `kind` (0 or 1), `side[4]` (bool per city side), `from` (travel type), `to` (travel type).

### 2.5 Tunnel record

The tunnel table is keyed by the portal's cell key. Each record holds:

- the other portal's cell (x, z);
- a travel-type mask (always `0x1FE`, §4.8);
- `cost[9]` (f32);
- the network type.

### 2.6 Ferry terminal, links and neighbour links

**FerryTerminal**:

| Field | Meaning |
|---|---|
| departure cell (x1, z1) | cell of the terminal's marker prop `0x2C520000`. Key of the departure table. Gets wormhole bit 4. |
| arrival cell (x2, z2) | cell of the marker prop `0x2C530000`. Key of the arrival table. |
| water cell (x3, z3) | the first water cell next to the marker prop `0x2C540000` that is outside the lot (§4.9.1) |
| ferry type | 0…2, building property `0x0BD86FD3` |
| travel mask | u16 from the table `{1, 3}` by ferry type: type 0 = Walk, type 1 = Walk and Car. Type 2 reads past the table and gets `0xD70A` *[quirk]*. |
| terminal links | list of TerminalLink |
| neighbour links | list of NeighborLink |
| riders (last period, current period) | u16 each (simulator, §4.9.3) |
| lot cell | the lot's location (x, z) |
| re-route flag | u8 |

**TerminalLink** (from A to B):

- B's departure key;
- B's arrival cell (x2, z2) as two u16;
- cost: f32 = `r24(cells × 0.01)`;
- route: `vector<u8>` water route A → B (§4.10.3);
- four u16 usage counters, in this order: last A, current A, last B, current B. A counts outbound-leg crossings and B
  return-leg crossings in the link's direction (traffic-simulator.md §4.5, TS16).

**NeighborLink**:

- the off-map cell (x, z) as i16;
- the in-city edge cell as two u16;
- cost: f32, as for terminal links;
- route to the edge: `vector<u8>`;
- route from the edge: `vector<u8>`;
- four u16 usage counters, ordered and counted as for terminal links (both directions of the crossing count on the
  one link).

The pathfinder uses these as follows (pathfinder.md §4.8.3):

- At a cell with wormhole bit 4, it looks the cell up in the **departure** table.
- Each terminal link becomes an edge to B's **arrival cell**, with the link cost.
- Each neighbour link becomes an exit to its off-map cell, with the link cost.
- Links are tried in list order: new links are appended, and a re-route keeps an existing link in place.

### 2.7 Neighbour connection edge records

There are four arrays of 8-byte records, one per map edge. They are named here by the side `d` of pathfinder.md
§4.1, which is also the order the loops below visit them:

- side 0: x < 0, indexed by z (Z records);
- side 1: z < 0, indexed by x (X records);
- side 2: x > X−1, indexed by z (Z records);
- side 3: z > Z−1, indexed by x (X records).

Each record holds:

- a u16 mask of neighbour-connection types (bit = type: 1 Rail, 2 Road, 3 Highway, 6 Subway, 7 LightRail,
  8 Avenue, 9 Ferry, 10 DirtRoad, 11 Monorail, 12 GroundHighway; neighbors.md);
- a u16 neighbour pool ID (0 = the default pool);
- an f32 cost, the extra trip length of a trip that ends at that off-map cell (pathfinder.md §4.13).

Init creates all four arrays with every record `(0, 0, 0.0)`. The records are not saved; DoPostCityInit rebuilds
them (§2.7.2). The pools are saved (traffic-simulator.md §6 row 19: u32 count, then per pool its u16 ID and its
property-holder record; `Write` `0x007132F4`, `Read` `0x00722F5D`–`0x00722FB3`), so the Adds after a load find
them again by position, with their capacities and counts. A repeated ID in the record (TS41): each entry is stored
through the map's find-or-insert (`0x00714560`, called at `0x00722FA2`), so the last entry wins; the earlier holder
stays allocated, with its reference, and is never released. Add and remove (messages
`0x8A0D15DA` and `0xEA0D163E`), the lookups, the pools and their capacities are in §2.7.1–§2.7.5.
The graph uses these records only through **HasMapExit(x, z)**, which is defined for an off-map cell next to the map:

- Map the off-map cell to its in-city cell and the side facing out (`GetNeighborConnectionInnerCell`, below).
- The result is true if any travel type's connection word at that in-city cell has an exit through that side, for
  any entry side.
- Otherwise it is true if the edge record at the off-map cell has mask bit 9 set (a water/ferry neighbour
  connection).

`GetNeighborConnectionInnerCell(x, z)` (`0x0070DDD0`):

- Exactly one of x and z must be off-map (x ∉ [0, X−1] xor z ∉ [0, Z−1]). Otherwise the result is −1.
- x < 0 → (0, z), side 0.
- x > X−1 → (X−1, z), side 2.
- z < 0 → (x, 0), side 1.
- z > Z−1 → (x, Z−1), side 3.

#### 2.7.1 Lookups

**GetTrafficNeighborConnection(x, z)** (`0x0070F970`) returns the record of an off-map cell. It tests in this
order: x < 0 → side-0 record z; x > X−1 → side-2 record z; z < 0 → side-1 record x; otherwise side-3 record x.
It never fails and checks no range: a corner cell such as (−1, −1) reads index −1 of the side-0 array, and an
in-city cell gets side-3 record x. Every caller passes an off-map edge cell, so neither happens in the game.
*[choice]* Return an empty record (mask 0, pool 0, cost 0.0) for any other cell.

In full (TN28), the original returns a pointer for every cell:

| Cell | Record returned |
|---|---|
| x < 0, any z | side-0 index z. For z = −1 or z = Z this is the 8 bytes before or after the array (heap memory) |
| x > X−1, any z | side-2 index z, with the same overrun at z = −1 and z = Z |
| 0 ≤ x ≤ X−1, z < 0 | side-1 index x: the correct record when z = −1 |
| 0 ≤ x ≤ X−1, z ≥ 0 | side-3 index x. For an in-city cell this is the record of (x, Z), a real record |

So a corner cell reads, and through Add (`0x007153F5`) would write, heap memory. An in-city cell aliases the edge
record below it, and an Add there would change that record. The callers:

- the pathfinder (`0x006D87ED`), only for a cell off the map;
- FindNeighborConnection (`0x007104F3`), after its in-city test;
- Add (`0x007153F5`), with a neighbour connection's cell;
- the trip end tests at `0x0071C8BC` and `0x0071C934`, only for an off-map trip end.

GetNeighborPoolPropertyHolder repeats the same tests inline (`0x00711500`–`0x0071153E`). Every one of these cells
is the off-map cell of a neighbour connection, which is always an edge cell next to the map. So the empty record of
the *[choice]* is never observable against the original. Writes to it must be discarded.

**FindNeighborConnection(t, x, z)** (`0x007104A0`) finds the neighbour connection that a trip of travel type `t`
uses at an off-map cell. For an in-city cell the result is none. Otherwise let `wanted := connMask(net(t))` (table
below) and `rec` the cell's record. For connection type `c` = 0…12 in ascending order, with bit `c` set in both
`wanted` and `rec.mask`, ask the neighbours simulator `FindConnection(c, x, z)` (slot 11). The first connection
found is the result; none found → none.

`connMask(n)`, connection types per transit network (`0x00AB322C`, u32[6]) *[compat]*:

| n | Transit network | Mask | Connection types |
|---|---|---|---|
| 0 | Pedestrian | `0x704` | Road, Avenue, Ferry, DirtRoad |
| 1 | Road | `0x170C` | Road, Highway, Avenue, Ferry, DirtRoad, GroundHighway |
| 2 | Rail | `0x2` | Rail |
| 3 | Subway | `0x40` | Subway |
| 4 | Light rail | `0x80` | LightRail |
| 5 | Monorail | `0x800` | Monorail |

**Travel types outside 0…8** (TN27). `net(t)` is a u32[9] table at `0x00AB31A8` (0, 1, 1, 2, 1, 2, 3, 4, 5 for
t = 0…8). Both lookups index it, and then the mask table above, with no range check (`0x007104D7`, `0x007114ED`).
For t ≥ 9 the original reads the read-only data that follows:

- t = 9…26 find 0, so the mask is the pedestrian one, `0x704`;
- t = 27…30 find 2, 1, 3 and 4;
- t = 31 finds 6, whose mask word is 0;
- larger t read further unrelated words.

This never happens in the game. The callers pass −1 (the pool lookup only, `0x00713703`), 1 (`0x00716751`) or a
trip's travel type, which is 0…8 (`0x006D79D7`, `0x007147B8`, `0x007147C9`). *[choice]* Use `wanted := 0` for any
t outside 0…8, except t = −1 in the pool lookup, which keeps all bits. FindNeighborConnection then gives none. The
pool lookup gives null, except at a record with the Ferry bit (step 2 below).

**GetNeighborPoolPropertyHolder(t, x, z)** (`0x007114E0`) gives the property holder that carries the destination
capacity of an off-map cell (traffic-simulator.md §4.2; pathfinder.md §4.10.1):

1. `wanted :=` all bits if `t = −1`, else `connMask(net(t))`.
2. `rec :=` the cell's record. If `rec.mask & wanted` is 0 **and** mask bit 9 (Ferry) is clear, the result is
   null. A ferry connection therefore gives a holder to every travel type.
3. If `rec.pool ≠ 0` and a pool with that ID exists, the result is that pool. If no pool has that ID, the lookup
   writes `rec.pool := 0` and continues *[compat]*.
4. The result is the default pool.

#### 2.7.2 Adding and removing connection types

NeighborConnectionCreated `0x8A0D15DA` calls Add and NeighborConnectionDeleted `0xEA0D163E` calls Remove, with
data1 = type, data2 = x, data3 = z (neighbors.md §6.3–§6.4). DoPostCityInit (§1, step 4) calls Add for every
connection of the neighbours simulator (slot 16 `GetAllConnections`, in list order) with the connection's type and
cell, and then runs the pool sweep (§2.7.3).

**AddNeighborConnection(type, x, z)** (`0x007153C0`):

1. `conn :=` the neighbours simulator's `FindConnection(type, x, z)` (slot 11). If there is none, nothing happens.
2. `rec :=` the cell's record; `rec.mask |= 1 << type`.
3. Cost candidate `f := commuteMaxTime` (traffic-simulator.md §4.14, tuning `0xA92356BA`, default 90.0). If the
   neighbours simulator's `FindConnectionOnOtherSide(conn)` (slot 13) gives a connection whose property holder has
   property `0xCA947077`, then `f :=` that value. It is the average trip length that the neighbouring city stored
   on its side of the connection when it last ran (traffic-simulator.md §4.6).
4. If `rec.cost < f`, then `rec.cost := f`. The comparison is ordered: a NaN on either side changes nothing. The
   cost only grows. Nothing lowers or resets it, Remove included *[quirk]*.
5. Pool. `city := conn.GetCityOnOtherSideOfConnection()` (connection slot 9). If there is none, or it is not
   established (regional-city slot 43, `GetEstablished`), then `rec.pool := 0`. Otherwise
   `(px, pz) := city.GetPosition()` (regional-city slot 5, the region cell of the city's corner). Search the pools
   in ascending ID order for one whose holder has the s32 properties `0x4A23576B = px` and `0x4A23576C = pz`. If
   one is found, `rec.pool :=` its ID. If not, create a pool: a new property holder with only those two
   properties, under the smallest unused ID ≥ 1. Then `rec.pool :=` that ID. The search (`0x0071555A`–`0x00715595`)
   counts in 32 bits but looks up the low 16 bits (TS42). With IDs 1…65535 all in use it reaches 65536 and looks
   up 0; key 0 is normally free, so the new holder is stored under key 0 and `rec.pool := 0`, the default pool. With
   key 0 taken as well, the search never ends. Neither can happen in the game: there is one pool per established
   neighbouring city.

So there is one pool per established neighbouring city. Every edge cell connected to that city shares the pool's
capacity and arrival counts. Each Add sets the pool again, so the last Add at a cell decides it.

**RemoveNeighborConnection(type, x, z)** (`0x00713AB0`) does not consult the neighbours simulator:

1. `rec :=` the cell's record; `rec.mask &= ~(1 << type)`. If the mask is still non-zero, stop.
2. Otherwise `rec.pool := 0`, and the pool sweep runs (§2.7.3).
3. For each bit `k` = 0…3 set in the off-map cell's destination byte (the destination map has a one-cell border,
   traffic-simulator.md §6), `freeDest[k] −= 1`. Then the byte is set to 0.

The cost is kept.

#### 2.7.3 Pools

A pool is a property holder kept in a map ordered by its u16 ID (≥ 1). The **default pool** is a separate holder:
Init creates it new and calls `SetMaxTripCapacity(default, 3, 0xFFFFFFFF)`. It therefore accepts freight (type 3)
without limit and no commuters.

**Pool sweep** (`0x007138B0`, called by Remove and by DoPostCityInit, not by Add):

1. Collect the pool IDs of records 0 … X−2 of each of the four arrays. The bound is X−1 for all four arrays
   *[quirk]*: the last record of every array is never looked at. The z-indexed arrays use the x bound, which is
   the same in the game's square cities.

   **When Z ≠ X** (TN26). The bound is `X − 1` (member `+0x194`, `0x0071392D`) for all four arrays, while Init
   sizes the z-indexed arrays (sides 0 and 2) to Z (`0x0071E944`–`0x0071E989`).
   - Z > X: their records X−1 … Z−1 are never looked at.
   - Z < X: the loop reads records Z … X−2, past the end of those arrays (heap memory). Here a garbage pool ID only
     keeps a pool alive, but the edge pass (§2.7.4 step 6) also writes. With a garbage non-zero mask, it can clear
     the "pool" word in that memory and set destination bits outside the off-map border of the destination map.

   No shipped city has Z ≠ X: every region city is square (64, 128 or 256 cells a side). *[choice]* Stop each
   loop at its own array: `j < min(X − 1, length)`, where length is Z for sides 0 and 2 and X for sides 1 and 3.
   This is the original for every square city.
2. Release every pool whose ID was not collected, and remove it from the map.

A pool used only by the last cell of an edge (z = Z−1 on sides 0 and 2, x = X−1 on sides 1 and 3) is therefore
dropped while still in use. That record keeps the stale ID. GetNeighborPoolPropertyHolder or the edge pass of
§2.7.4 then resets it to 0, unless a pool created in the meantime took the same ID; in that case the record
silently belongs to the new pool *[quirk]*.

#### 2.7.4 Pool capacities and edge destinations (`0x00714E60`, run by StartRecalc in state 0)

StartRecalc (traffic-simulator.md §4.3) ends with this pass. It runs on the main thread (x87 24-bit precision,
`r24` below).

1. **Regional jobs**: for w = 0…2, `J[w] :=` the demand simulator's `GetDemand(0x2010 + 0x10·w, 0x10000)` (the
   regional node, demand.md §9), its `QueryDemandValue()`, truncated toward zero to a 32-bit integer. The
   original does not check for a missing node: `GetDemand` returns null for one (demand.md §9) and the next call
   goes through it, a crash (`0x00714ED9`–`0x00714EE0`, TS43). The shipped demand exemplars define the three
   regional nodes, so this does not happen in the game *[inferred]*. *[choice]* Use 0 if the node is missing.
2. **Cities**: the neighbours simulator's `GetConnectedCities(list, false)` (slot 21): the established cities
   reached by a transport connection (neighbors.md §6.2).
3. **Totals**: for w = 0…2, `T[w] :=` the 32-bit integer sum over those cities of `E_c[w]`. Here
   `E_c[w] := c.GetExtrapolatedPopulation(0x2010 + 0x10·w)` (regional-city slot 17, demand.md §9.1).
4. **Per city** `c`, in list order: `(px, pz) := c.GetPosition()`. Take the first pool, in ascending ID order,
   with `0x4A23576B = px` and `0x4A23576C = pz`. If there is none, skip the city. Otherwise:
   - for w = 0…2: `cap := trunc(r24(r24(float(E_c[w]) / float(T[w])) × float(J[w])))`, keeping the low 32 bits of
     the integer. `T` and `J` are converted as **unsigned** 32-bit values and `E_c` as signed. Then
     `SetMaxTripCapacity(pool, w, cap)` (slot 19) and arrived-this-cycle `[w] := 0` (element 4 + w,
     traffic-simulator.md §4.2). With `T[w] = 0` the quotient is NaN and `cap` is 0.
   - `SetMaxTripCapacity(pool, 3, 0xFFFFFFFF)` and arrived-this-cycle `[3] := 0`.
   Pools that match no city keep their old capacities and counts.
5. **Default pool**: arrived-this-cycle `[3] := 0`.
6. **Edge pass**: for side d = 0…3 and index j = 0 … X−2 (the same bound as the sweep, *[quirk]*; for Z ≠ X see
   §2.7.3, TN26), skip records
   with mask 0. The off-map cell is (−1, j), (j, −1), (X, j) or (j, Z) for d = 0, 1, 2, 3. If `HasMapExit(cell)`
   is true, take the holder: pool 0 → the default pool; an ID with no pool → `rec.pool := 0` and the default pool;
   otherwise that pool. There is no mask test here. Then for k = 0…3: if `GetMaxTripCapacity(holder, k) ≠ 0`
   (slot 20), set bit k in the cell's destination byte and `freeDest[k] += 1`. The count goes up even when the
   bit was already set *[quirk]*. RefreshCommuteMap (state 10) zeroes `freeDest` later in the cycle.

The connection on the last cell of an edge never gets destination bits from step 6. It is therefore never a
filter-0 destination (pathfinder.md §4.10.1) *[quirk]*.

**Example.** The regional node `0x12010` has demand value 800.0, so `J[0] = 800`. Two established neighbours are
connected: A with `E_A[0] = 300` and B with `E_B[0] = 100`, so `T[0] = 400`. A's pool gets
`r24(300/400) = 0.75`, × 800 = 600, so capacity 600. B's pool gets 0.25 × 800 = 200. For w = 1, take `J[1] = 10`,
`E_A[1] = 1` and `E_B[1] = 2`. A gets `r24(1/3) = 0.333333343` × 10 = 3.33333349, so 3. B gets
`r24(2/3) = 0.666666687` × 10 = 6.66666698, so 6. Truncation loses one job of the 10. Say A's road
connection is at (−1, 5), with pool 1 and a road exit (HasMapExit true). The edge pass then sets bits 0, 1 and 3
of the destination byte of (−1, 5), and bit 2 as well if A's capacity for w = 2 is non-zero.

#### 2.7.5 Neighbour roll (UpdateNeighborConnections, cycle state 11, `0x00714BE0`)

This state visits each connection of the neighbours simulator (slot 16 `GetAllConnections`, in list order) and
works on the connection's **own** property holder (connection slot 3):

1. For i = 0…2: if the u32[9] property `0xAA9EA81A + i` is present, copy its value to `0xAA9EA81D + i`. Then
   set `0xAA9EA81A + i` to nine zeros, even if it was absent. The `0xAA9EA81A + i` array counts trips of
   destination type i that ended at this connection, by last travel type (traffic-simulator.md §4.2). The other
   city reads the copied arrays `0xAA9EA81D + i` (neighbors.md §4.2, traffic volume sync).
2. For k = 0…3: `v :=` element 4 + k of `0x8A0B4100`, or 0 if the property is missing. Set element 8 + k to `v`,
   then element 4 + k to 0. The 12-element property is created if it is missing.

After one cycle both properties are therefore present on every connection, and they are saved with it
(neighbors.md §4.3) *[compat]*. The state finishes in one call.

---

## 3. Messages handled (network part)

The traffic simulator's `DoMessage` (`0x007241B0`) dispatches these. Only the network-model effects are listed here;
traffic-simulator.md has the full list.

| Message | Condition | Effect |
|---|---|---|
| `0x99EF1142` occupant inserted / `0x99EF1143` removed | data3 = `0xC772BF98` (network occupant), void1 = occupant, QI `0xA821EF94` (cISC4NetworkOccupant) succeeds | Simulator bookkeeping first: per-network-type counts, budget and the pothole sets (DoRoadChanged for types 6, 0, then the first of 10/3/2/12; DoRailChanged for 1; DoSubwayChanged for 7 unless network flag `0x80000000`; traffic-simulator.md §3.6). If the occupant is of any network type that takes part (§4.1): (a) if it is a tunnel (QI `0x8A4BD52B`), §4.8; (b) cell = city `PositionToCell(occupant position)`; on **remove**, erase the 18 edge-density keys of that cell (traffic-simulator.md); (c) `DoConnectionsChanged(x, z, x, z)`. Always sets the simulator's network-changed byte, which nothing reads (traffic-simulator.md §3.1, TS5). |
| `0xE8BDA2E7` lot state changed | void1 QI `0xE818FCED` (a **copy** of the lot with empty trip list, lot-developer.md §3.2) | data1 = 3 → **lot deactivated** (§4.7.5, §4.9.4). Otherwise data2 = 3 → **lot activated** (§4.7.1, §4.9.1). |
| `0x26D31EC1` post city init | | DoPostCityInit (§1) |

The water pathfinder subscribes separately (§4.10.1).

---

## 4. Behaviour

### 4.1 Constant tables *[compat]*

| Table | Values |
|---|---|
| network type takes part in traffic (`0x00AB3164`) | Road, Rail, Highway, Street, Avenue, Subway, LightRail, Monorail, OneWay, DirtRoad, GroundHighway = yes. WaterPipe, PowerPole = no. |
| transit-network mask per network type (`0x00AB3174`) | Road 3, Rail 4, Highway 2, Street 3, Pipe 0, Pole 0, Avenue 3, Subway 8, LightRail 0x10, Monorail 0x20, OneWay 3, Dirt 3, GroundHighway 2. Bit `n` = the type carries transit network `n`. |
| `net(t)` (`0x00AB31A8`) | {0, 1, 1, 2, 1, 2, 3, 4, 5} |
| `pt(n)` (`0x00AB3214`) | {2, 1, 3, 4, 6, 7} |
| `rep(n)` (`0x00AB3244`) | {0, 1, 3, 6, 7, 8} |
| opposite side (`0x00AB3394`) | {2, 3, 0, 1} |
| switch rotation `ROT[facing][i]` (`0x00AB3280`) | facing 0: {2, 3, 0, 1}; 1: {1, 2, 3, 0}; 2: {0, 1, 2, 3}; 3: {3, 0, 1, 2} |
| ferry travel mask by type (`0x00AB32C8`) | {1, 3} (type 2 reads `0xD70A`) |

**Side-of helper** `SideOf(a → b)`: the side of cell `b` that faces cell `a`.

- `a.x < b.x` → 0;
- `a.x > b.x` → 2;
- otherwise `a.z ≥ b.z` → 3, else 1.

Equal cells give 3. This is the same rule as pathfinder.md's EdgeTo.

### 4.2 DoConnectionsChanged(x1, z1, x2, z2) (`0x0071A860`) *[compat]*

This rebuilds the graph over a rectangle. In this order:

1. **R** = the rectangle grown by 2 on every side, clamped to `[0, X−1] × [0, Z−1]`. Add R to the intersection
   dirty rectangle (union) and set the dirty flag (§4.6).
2. For every cell of R:
   - delete its network-info records of all six networks;
   - reset `conn[t] = 0` and `switch[t] = 0xFFFF` for all 9 types;
   - reset `priorCar[0…3] = 0xFFFF` and `wormhole = 0`.
3. Get all occupants of type `0xC772BF98` in R. Use occupant manager slot 41, `(list, &nw, &se, type, −1)`. The list
   uses allocator A and is internal. For each occupant, in list order, that is of at least one taking-part network
   type and does **not** have network flag `0x200000`:
   - c = `PositionToCell(occupant position)`.
   - For n = 0…5 (in this order), with `p = pt(n)` and `count = 0`:
     - For each entry side e = 0…3:
       - For each exit x = 0…3: if the occupant's `PathConnection(p, e, x)` (cISC4NetworkOccupant slot 53, §7.2)
         is true:
         - `count += 1`;
         - for every travel type t with `net(t) = n`: `conn[t] |= 1 << (x + 4e)`;
         - when t = 1 (Car): **LaneContinuity**(c, occupant, e, x) (§4.4).
       - If no exit was found for e: ask the occupant's path info (cISC4NetworkOccupant slot 49, borrowed; fetched
         at the first such entry of the occupant and kept for the rest of it, `0x0071ABC1`–`0x0071ABDB`; a null
         path info skips the test) for `GetPath(p, e, 0xFF, 0, null)` (path info slot 4, §7.3): the path with key
         `p << 24 | e << 8 | 0xFF`, exit `0xFF` being the turn-around path, lane 0. Slot 4 returns a path pointer
         or null, and the whole of EAX is tested (`0x0071ABF9`). If it is not null (the piece allows a
         turn-around): `count += 1` and `SetConnection(c, e, e, t)` for every t on n (§4.3).
     - If `count ≠ 0` (as u16):
       - get or create the record of network n in c;
       - `connCount = count` (u8);
       - `extraCost` = the damaged-road extra step cost if `n = 1` and `IsRoadDamaged(c)` (slot 16), else 0.0;
       - if the occupant has network flag `0x40000`: `flags |= 1`;
       - choose the network type: start "none". For k = 0…12 where `mask[k]` has bit n and the occupant
         `IsOfType(k)`, set `type = k`, with two exceptions: Street (3) is taken only while still "none", and Road (0)
         does not replace Highway (2). The last match wins. If still "none": type 0 and `invCap = 1.0`, else
         `invCap = invCapTable[type]`;
       - create the edge-density entries of the cell for every t on n, if missing (traffic-simulator.md §4.8). For
         each t = 0…8 with `net(t) = n`, in order, this is done for **both** evening values: first the key
         `t << 16 | x << 8 | z`, then `1 << 24 | t << 16 | x << 8 | z` (TN12, `0x0071AD20`–`0x0071AE1E`). Each key
         is looked up on its own. A missing one is inserted as an all-zero record (a copy of the shared zero record
         `0x00AB31D0`, `0x42` bytes). An existing one is left as it is, so its counts are **not** reset here.
4. **Transit switches** over **R1** = R grown by 1 more and clamped (so the original rectangle + 3). See §4.7.4.
5. **Tunnels**: every cell of R1 that is a key of the tunnel table gets `wormhole |= 2`. The test is the key alone
   (`0x0071B390`–`0x0071B3FF`): the bit is set whatever the tunnel record's network and mask and whatever type the
   cell's own network records have (TN29).
6. **Ferries**: every terminal is processed in list order (§4.9.2).
7. Set the "subnetworks stale" and "network changed" flags. Free the occupant list.

The occupants' order decides which occupant's `flags` and type win in a shared cell. The occupant manager's order is
specified in its own spec.

### 4.3 SetConnection(x, z, e, x', t) (`0x0070EAB0`)

1. `conn[t] |= 1 << (x' + 4e)` at (x, z).
2. If `net(t) = 1` and the neighbour through `x'` is in the city: that neighbour's
   `priorCar[(x' + 2) mod 4] |= 0x1111 << e`.

Step 2 allows every exit of the neighbour for cars arriving from this side. Plugins (NAM) hook this function at
`0x0070EAE7`. That hook point is not part of FreeCity4's contract.

### 4.4 Lane continuity (RemoveBadElevatedPaths, `0x00715940`)

Called for each car connection (e → x) of an occupant at cell c, from DoConnectionsChanged step 3 with the
occupant's path info (cISC4NetworkOccupant slot 49, borrowed, §7.2; not null-checked, but slot 53 has just
created it). Arguments as compiled: `(cellX, cellZ, pathInfo, entry e, exit x)`, pop 20, no return value. The path
info slots are in §7.3. A path is named by its **key** (§7.3): exit in bits 0–7, entry in bits 8–15, a lane index in
bits 16–23, the path type in bits 24–27.

1. **Collect the start vertices.** `keys := GetPathsFromEntryEdge(1, e)` (path info slot 14: every car path whose
   entry is e, in the path table's hash order). For each key in that order whose **exit byte** (bits 0–7 of the key)
   equals x: `path := GetPath(key, null)` (slot 3); if `path` is not null, append a copy of its **first vertex**
   (the first 12-byte point `(x, y, z)`, three f32 world coordinates, of the path's point array) to the list V. If
   V is empty, stop: **nothing changes**.
2. Let P = the neighbour of c through side e, `(cellX + dx[e], cellZ + dz[e])` from the side tables `0x00AB325C`
   and `0x00AB326C` (no bounds check; the occupant query returns nothing off-map). Get the network occupants of
   the one cell P (occupant manager slot 41, `(list, &P, &P, 0xC772BF98, −1)`). For each, in list order: QI
   `0xA821EF94`; skip it unless it has any of the network flags `0x1F4F` (slot 9) and its path info (slot 49) is not
   null. Then:
   - `M = 0`;
   - for each vertex v of V, in order: `near := GetPathsNearEndPoint(1, &v, 0xFF, (e + 2) mod 4, 4.0)` (slot 7):
     the keys of the car paths of P, any entry, exit `(e + 2) mod 4`, whose **last** vertex lies within 4.0 world
     units of v (3-D distance, `≤`). For each key k of `near`: `M |= 1 << entry(k)`, with `entry(k)` = bits 8–15
     of k;
   - write nibble x of `priorCar[e]` at c: `priorCar[e] := (priorCar[e] AND NOT (0xF << 4x)) OR (M << 4x)`, as
     u16 (`0x00715BBE`–`0x00715C05`). *[quirk]* `M` is not masked to 4 bits; it only holds bits 0–3 because path
     entries are sides 0–3.

The **last** qualifying occupant in P decides *[quirk]*. With no flagged occupant in P, the word keeps `0xFFFF`
from the reset. So the bits are only restricted next to occupants with those flags: elevated and highway pieces
*[inferred]*. The neighbour occupants' path infos are borrowed, not released; each QI result and each listed
occupant is released before the next.

Worked example. Occupant at c = (10, 10) with one car path entry 2 → exit 0, key `0x01000200`, whose first vertex
is (168.0, 270.0, 160.0). The connection (e = 2, x = 0) collects V = [(168, 270, 160)]. P = the neighbour through
side 2. P holds one occupant with network flag `0x0008` and car paths `0x01000000` (entry 0 → exit 0, last vertex
2.0 units away) and `0x01000100` (entry 1 → exit 0, last vertex 6.0 units away). Exit `(2 + 2) mod 4 = 0`:
`near = [0x01000000]` (6.0 > 4.0), so `M = 0x1` and nibble 0 of `priorCar[2]` at c becomes 1: `0xFFFF → 0xFFF1`.

### 4.5 Capacity tuning (read by ReadSimulatorTunableValues `0x00716AE0`, exemplar in traffic-simulator.md)

| Property | Name (re/tuning) | Use | Missing → | Shipped |
|---|---|---|---|---|
| `0xA92356B0` | Damaged road extra step cost | `extraCost` on damaged car cells | unchanged (code default 32.0) | 0.1 |
| `0xA92356B3` | Network Traffic Capacity | 13 floats | arrays unchanged (`invCapTable` then undefined before the first read *[choice]*: FreeCity4 uses 1.0) | 1000, 3000, 4000, 100, 0, 0, 2500, 3000, 3000, 3000, 2000, 0, 4000 |
| `0xA92356BB` | Intersection and Turn Capacity Effect | N = count (u8), `values[i] = r24(1.0 / v[i])`, no zero guard | unchanged | 0.7, 0.8, 0.9 |

When `0xA92356B3` is present, for k = 0…12:

- `v = (k < count) ? value[k] : 0.0`;
- `rawCapTable[k] = v`;
- `invCapTable[k] = r24(1.0 / v)` if `|v| > 1e-10`, else 1.0.

Shipped `invCapTable` bit patterns:

| Capacity | `invCapTable` |
|---|---|
| 1000 | `0x3A83126F` |
| 3000 | `0x39AEC33E` |
| 4000 | `0x3983126F` |
| 100 | `0x3C23D70A` |
| 0 | `0x3F800000` |
| 2500 | `0x39D1B717` |
| 2000 | `0x3A03126F` |

**GetCapacity(n, x, z)** (slot 23):

- returns `r24(1.0 / invCap)` of the first record of network n at (x, z), as x87 float;
- 0.0 when there is no record;
- no bounds check (x, z must be in the city).

So it reports the capacity after the intersection reduction. The round trip is not exact. For example, 1000 gives
999.99994 (`0x4479FFFF`), 3000 gives 3000.0 and 4000 gives 3999.9998. *[compat]*

**GetConnectionCount(n, x, z)** (slot 25): `connCount` of that record, or 0. Returned zero-extended in EAX.

### 4.6 Intersection capacity reduction (UpdateIntersectionData `0x0071B9A0`)

This runs at the start of the next work cycle while the dirty flag is set (traffic-simulator.md). It runs over the
dirty rectangle grown by N and clamped, then clears the flag.

For every car record (n = 1) whose type is not Highway:

1. Find `best`, the distance to the nearest intersection:
   - If `connCount > 2`, `best = 0`.
   - Otherwise, search depth-first from the cell along its car (`conn[1]`) exits, union over entry sides. Stop
     extending at depth `best`; best starts at N + 1.
   - A reached cell is an intersection if its car record has `connCount > 2`, or if it has no car record but a
     transit switch whose "counts as intersection" flag is set. Then `best := depth`.
   - **Tunnel cells and cells without car info are reached but not extended** (TN5). On entering a cell at depth
     d, in order:
     1. the intersection test above runs, so either kind of cell can set `best := d`;
     2. a cell without a car record stops there;
     3. so does any cell with `d ≥ best`;
     4. so does a tunnel cell (wormhole bit 2 of the connection record, byte `+0x2C`, `0x0071BBB8`).

     Only then are its sides 0…3 tried, in order. A side is taken when some entry side has an exit on it and the
     neighbour is in bounds.
   - **The start cell gets the same checks** (TN21, from the exe). The search enters it at depth 0 like any other
     cell (`0x0071BB01`–`0x0071BBBB`). Its car record has `connCount ≤ 2`, so the intersection test does nothing,
     and depth 0 is below `best`. The tunnel test still applies, so **a tunnel start cell is not extended**. The
     stack is empty, so the search ends at once (`0x0071BCFF` → `0x0071BD98`) with `best = N + 1`, and the cell
     keeps its table value.
   - The search keeps no visited set: a cell can be reached again by another path while the depth limit holds.
     Backtracking resumes the parent at its next side.
   - The search's bounds check compares both coordinates with X−1 *[quirk]*. It matters only on non-square maps.
2. Set `invCap = invCapTable[type]`. If `best < N`, then `invCap = r24(invCap × values[best])`.

With the shipped values, a Street (100) gives:

| Position | `invCap` | Capacity |
|---|---|---|
| intersection | `0x3C6A0EA1` | 70.0 |
| distance 1 | `0x3C4CCCCC` | 80.00001 |
| distance 2 | `0x3C360B61` | 90.0 |
| distance 3 or more | `0x3C23D70A` (unchanged) | 100 |

Worked example (TN21), with the shipped N = 3: a Street tunnel cell with 2 connections whose exit leads straight into
a 4-way intersection one cell away. Extending the start cell would give `best = 1` and `0x3C4CCCCC` (80.00001). The
original does not extend it, so the cell keeps `0x3C23D70A` (100). The intersection itself still gets `0x3C6A0EA1`.

**How a Street cell becomes a tunnel cell** (TN29). No Street portal has a tunnel record: Street (3) is not in the
§4.8 type order, so its insert stops at step 2. The bit of §4.2 step 5 depends on the cell key only, so a cell whose
car record is typed Street gets it when a portal of one of the listed types is keyed at that cell and the cell's car
record ends up typed Street (in §4.2 step 3 the occupant order decides the type of a shared cell). The worked example
is such a cell; vector tn11_04d builds it with a Road portal and a Street piece in one cell. Whether the game's
network tools can put a Street piece in a portal's cell was not traced. A Road tunnel cell behaves the same way with
the Road values.

### 4.7 Transit switches

#### 4.7.1 Activation (DoLotActivated `0x00723AF0`, network part)

Steps 1–5 apply when the lot's building has property `0xE90E25A1` (raw bytes, 4 per rule, §4.7.2). Without it,
DoLotActivated jumps straight to step 6 (`0x00723B7B` → `0x00723ED0`) with no switch created. Steps 6 and 7 run for
every activated lot that has a building, unless step 1 returned (TN2).

1. If the property's byte size is not a multiple of 4, **return from DoLotActivated** (`0x00723BAC` jumps to
   `0x00724188`, which only frees the copied bytes). Everything after this point is skipped (TN1):
   - the switch and the grid writes;
   - step 6: ferry activation and `DoConnectionsChanged`;
   - all of step 7, including the `RandomUint32Uniform(100)` draw (`0x00723FBE`), the budget items and the
     destination bits (traffic-simulator.md §4.1.3).

   The byte size is the variant's element count, because the property is read as raw bytes (§4.7.2). Every shipped
   switch stores a u8 array of 8 to 32 bytes, so none stops here. Only custom content stops here: a u8 array whose
   length is not a multiple of 4, or a u32 array whose entry count is not (§4.7.2). *[compat]* because of the
   skipped draw.
2. Build a switch:
   - the building reference;
   - properties `0xE90E25A2`, `0xE90E25A3`, `0x0BFC0A4C`, `0x4BFC47B0`;
   - the rules (§4.7.2) and the flags.
3. If flag 4 (a Subway "to" rule) is set: list the occupants of type `0x8A13918A` in the lot rectangle. The first
   one's cell is the access point. Without one, use the NW cell of the building's bounding cells. Set "has access
   point".
4. Mode mask = OR of `1 << from` over the kind-0 rules with `from < 8`.
5. Write `id = next switch ID` into the ID grid and the mode mask into the mask grid over the lot rectangle. Insert
   the switch under `id`, then increment the next switch ID.
6. If the building has `0x0BD86FD3`, run ferry activation (§4.9.1), then `DoConnectionsChanged(lot rectangle)`.
   Otherwise, if a switch was created, run `DoConnectionsChanged(lot rectangle)`. A switch is always created once
   step 1 passes (`0x00723EBC`), even from an empty property with no rules. So a building that has neither
   property gets no rebuild here, and step 7 still follows.
7. The strategy-bucket registration, the one random draw, the budget items and the destination bits follow. They
   belong to traffic-simulator.md §4.1.3 (see §12).

#### 4.7.2 Rules (ReadTransitSwitchRules `0x00715CB0`)

Find the lot at the NW corner of the building's bounding cells (lot manager slot 6, bool false). With no lot, no
rules are read *[quirk]*. Otherwise `f = lot.GetFacing()` (slot 23).

`0xE90E25A1` is read as **raw bytes** by `0x005FFB40`:
1. property holder slot 9 `GetProperty(id)` (absent → no bytes);
2. the variant's `RefUint8()` (slot 95) and `GetCount()` (slot 4);
3. `count` bytes are copied from the variant's data.

Every shipped switch stores the property as a **u8 array**: 13 exemplars in `SimCity_1.dat`, 8 to 32 bytes, with
`b0` always `0x81` or `0x82`. Each group of 4 bytes `b0 b1 b2 b3` is one rule, and there are `n = ⌊count / 4⌋` of
them. DoLotActivated has already refused a count that is not a multiple of 4 (§4.7.1 step 1). For each rule:

- `kind = (b0 = 0x82) ? 1 : 0`;
- for i = 0…3: `side[ROT[f][i]] = bit (7 − i) of b1`;
- `from = b2`, `to = b3`.

Kind-0 rules also OR `1 << to` into the "to" mask.

**Flags (TN19).** After the rules are pushed, the flags byte is reset to 0. It is then rebuilt from the first n rules
of the vector, **whatever their kind** (`0x00715EF8`–`0x00715F6D`, jump table `0x00715F88`):

| `to` (travel type) | Flag |
|---|---|
| 2 (Bus) | `0x01` |
| 3 (Passenger train) | `0x02` |
| 6 (Subway) | `0x04` |
| 7 (El train) | `0x08` |
| 8 (Monorail) | `0x10` |
| any other | none |

The flags also get `0x40` when (from, to) is (6, 7) or (7, 6).

Worked example: the shipped light-rail/subway station (`0x0ADE0000`). Its bytes are `81 F0 07 07`, `82 F0 07 06`,
`81 F0 06 07`, `82 F0 07 07`, and the facing does not matter because every rule has all four sides:

| # | kind | sides | from → to |
|---|---|---|---|
| 1 | 0 | all | 7 → 7 |
| 2 | 1 | all | 7 → 6 |
| 3 | 0 | all | 6 → 7 |
| 4 | 1 | all | 7 → 7 |

This gives "to" mask `0x0080` and mode mask `0xC0` (§4.7.1 step 4). The flags are `0x08 | 0x04 | 0x40 = 0x4C`. Only
the kind-1 rule 2 sets flag 4, so this switch gets a subway access point (§4.7.1 step 3). Reading kind-0 rules only
would give `0x48` and no access point.

*[quirk]* A property stored as a **u32 array** of N entries has count N, so only its first N bytes are copied.
They give N/4 rules, taken whole from the first N/4 entries (`b0` = each entry's least significant byte), and
DoLotActivated stops unless N is a multiple of 4. A scalar has count 0, so it gives a switch with no rules. FreeCity4
reads the same raw bytes.

**Empty and object-reference values (TN22, from the exe).** These have count 0 as well. The count is a field of its
own (`+0x10`, read by `GetCount` `0x0040CE60`), not derived from the type:
- the constructors and `Destroy` (`0x00914EA3`, run when the type changes) set it to 0;
- storing an object reference (`0x00915A5F`, type `0x10`) does not touch it.

So the copy takes 0 bytes, even though the raw accessor returns the object pointer. The property counts as present,
so DoLotActivated passes step 1 (0 is a multiple of 4) and builds a switch with no rules: mode mask 0, flags 0, no
access point. Only an absent property skips the switch (`0x005FFBF5` returns false).

**After a load (TN3, from the exe).** The save **does** contain the rules: `Read` reads them for record versions
above `0x1C` (§6). `DoPostCityInit` still calls this function for **every** switch in the table (`0x00720466`). The
function appends and never clears: `0x00711CD0` only reserves room, and each rule is pushed after the existing ones.
In order:

1. The "to" mask is zeroed, before the lot lookup.
2. With no lot at the NW corner it returns at once. The rules and flags stay as `Read` left them, and the mask
   stays 0.
3. Otherwise the property's n entries are appended and the mask is rebuilt from them. The flags are reset and
   rebuilt from the **first n rules of the vector**, which after a load are the saved copy.

After loading a current save, each switch therefore holds its saved rules followed by the same rules again
*[quirk]*. Nothing rebuilt depends on the duplicates. §4.7.4 applies the same rules twice, in the same order, and so
writes the same values, and the flags use the first n. The duplicates show only in the next save, which writes the
doubled list, so the record grows by n rules per load. *[choice]* FreeCity4's re-read **replaces** the rules (clear,
then read), so a load followed by a save reproduces the file.

**When the saved rules differ from the property (TN25, from the exe).** This happens only when the building's
`0xE90E25A1` changed between the save and the load (a plugin added, removed or edited). The lot and its facing are
the same, so an unchanged property always matches. With m saved rules and n property rules, the original differs
from FreeCity4's replacement in three ways:
- **flags:** taken from the first n entries of the list (`0x00715EF8`–`0x00715F6D`). These are the old rules, plus
  the first n − m new ones when m < n. FreeCity4 takes them from the n new rules, so the access-point walk can
  disagree as well;
- **connections:** §4.7.4 applies all m + n rules, the old ones first. So connections that only the old rules make
  survive, and where an old and a new rule write the same switch nibble, the new one wins. FreeCity4 applies only
  the new rules;
- **persistence:** the old rules are never dropped. Each later save writes the whole list back, so the old rules
  stay at the front after every later load.

The "to" mask (from the n new entries) and the mode-mask grid (from the save) are the same in both.

Worked example: a switch saved with `81 F0 07 07`, `82 F0 07 07` (m = 2, el train only), loaded after its exemplar
became `81 F0 06 06`, `82 F0 06 06` (n = 2, subway only). The original's list holds all four rules. Its flags come
from the first two, so they are `0x08`: flag 4 is clear and the access-point walk clears "has access point". Both the
el-train and the subway connections are built. FreeCity4 holds the two subway rules with flags `0x04`. If the save had
no access point, its walk then sets one at the NW cell, and only the subway connections are built. *[choice]*
FreeCity4 follows the current property: a changed station plugin takes full effect on load. This only differs from
the original for cities saved before the plugin changed.

**Access points after the re-read (TN4, `0x007204E4`–`0x0072051E`).** Once every switch has been re-read,
DoPostCityInit walks the table again. Each switch is handled on its own, so the order does not matter:

- **flag 4 set and "has access point" = 0:** set "has access point" := 1. The access cell is the NW cell
  `(x1, z1)` of the building's bounding cells (building slot 11, `+0x2C`). The occupant search of §4.7.1 step 3
  (type `0x8A13918A`) is **not** repeated *[quirk]*;
- **flag 4 set and "has access point" = 1** (as saved): kept unchanged;
- **flag 4 clear:** "has access point" := 0. The saved cell bytes stay but are no longer used.

So after a load, a switch's access point is the saved one when the save had one. Otherwise it is the NW cell, even
when a fresh activation would have found an access occupant. The mode-mask grid is not touched: it comes from the
save (§6), and DoPostCityInit does not rewrite it from the re-read rules.

**A switch without a usable building (TN23, from the exe).** Neither walk checks the building:
- the re-read dereferences it first, to get its property holder (`0x00715CCF`), before the mask is zeroed;
- the access-point walk calls its slot 11 (`0x00720504`) when flag 4 is set and there is no access point.

A null building therefore crashes the original's DoPostCityInit. A switch inserted by a §4.7.3 or §4.7.4 lookup does
not even hold null (§4.7.3 step 1): its building is a stale stack value. *[choice]* FreeCity4's inserted switch is
all zeros, and DoPostCityInit handles a switch with building 0 like this:
- no re-read: the rules, the flags and the "to" mask stay as they are;
- flag 4 clear: "has access point" := 0, as for any switch;
- flag 4 set: "has access point" and the cell stay as they are, because there is no building to take a NW cell from.
  This case needs a loaded switch whose building reference was null. An inserted switch has flags 0.

For saves of version ≤ `0x1C` (most of the corpus is version 19), `Read` reads no rules and the re-read is their only
source. `Read` also computes its own mask and flags. They survive only when the NW lot is missing, and they differ
from the rules above *[quirk]*:
- the mask ORs the raw `to` of the **kind-1** rules;
- the flags also map `to = 5` to `0x20` (`0x007235FC`).

Everything else in Read's flags is the same as above (TS33, `0x00722202`–`0x0072227D`). The flags start at 0, the
other `to` values map as in the table, and `0x40` is added when (from, to) is (6, 7) or (7, 6), whatever the
rule's kind. Read compares the full u16 `from` and `to`; the re-read's table uses the low byte of `to`. The two
differ only for a `to` above 255, which shipped rules cannot hold.

#### 4.7.3 TransitSwitchEntryCost(id) (non-virtual `0x0071CB20`, x87 return; used by pathfinder.md §4.8)

1. Look up the switch with insert-if-missing semantics *[quirk]*: an unknown ID inserts a default switch. In the
   original that switch is a copy of an **uninitialised** stack temporary (`0x006D90F0`). Only the second f32 counter
   (`+0x18`) and the rules are zeroed (`0x006D9145`–`0x006D9155`). The building, load, time factor, capacity, flags,
   access fields, "to" mask, property `0x0BFC0A4C` and intersection flag are whatever was on the stack *[quirk]*, so
   the cost of an unknown ID is not reproducible. *[choice]* FreeCity4 inserts an all-zero switch: 0/0 is NaN, so the
   cost is `1.7014117e38` (TN23).
2. `q = r24(load / capacity)`.
3. If `q < 5.0` (ordered): cost = `r24(r24(Curve(q) × transitMultiplier) × timeFactor)`. Curve is the
   simulator's congestion → inverse-speed curve (`0x00B4B2F0`). `transitMultiplier` is simulator-owned (1.0 at
   construction, then set from a curve in DoPostCityInit and restored by Read; traffic-simulator.md).
4. Otherwise (q ≥ 5, NaN from 0/0, or +∞): cost = `1.7014117e38` (`0x7EFFFFFF`).

#### 4.7.4 Switch phase of DoConnectionsChanged (over R1)

For each cell of R1 with switch ID ≠ 0, let S be the switch (looked up with insert-if-missing).

1. For every travel type except 6 (Subway): `conn[t] = 0` and `switch[t] = 0xFFFF` in that cell.
2. For each side d = 0…3 whose neighbour N = cell + side d is in the city:
   - **Same switch** (N has the same ID): for each kind-0 rule with `side[d]`: `SetConnection(cell, k, d, to)` for
     every k with `side[k]`.
   - **Other cell**: `ns = SideOf(cell → N)`. For each rule with `side[d]`:
     - **Kind 1 (entering the switch)**:
       - If N has a record of `net(to)`: when `from = to`, `SetConnection(cell, k, d, from)` for k = 0…3;
         otherwise set nibble d of the switch cell's `switch[from]` to `to`.
       - Then for each k ≠ ns such that N's `conn[to]` has an exit k from some entry: `SetConnection(N, ns, k, to)`.
     - **Kind 0 (leaving the switch)**:
       - If N has a record of `net(from)`: when `from ≠ to`, set nibble `ns` of N's `switch[from]` to `to`;
         otherwise for each entry j of N with any exit in `conn[from]`, `SetConnection(N, j, ns, from)`.
       - If `from = 1` (Car): for each k ≠ ns such that N's `conn[1]` has an exit k, `SetConnection(N, ns, k, 1)`.

The order is cells in R1 order (x outer, z inner, as the rectangle loop), sides 0…3, rules in stored order.
*[compat]* where several rules write the same switch nibble (the last write wins).

#### 4.7.5 Deactivation (DoLotDeactivated `0x0071E2F0`, network part)

1. Read the switch ID at the lot rectangle's NW cell. If the switch exists:
   - release the building reference;
   - erase the switch;
   - zero the ID and mask grids over the lot rectangle;
   - `DoConnectionsChanged(x1−1, z1−1, x2+1, z2+1)`.
2. Ferry terminal removal (§4.9.4).

### 4.8 Tunnels (DoTunnelChanged `0x007140E0`)

The key is the portal's cell key.

**Remove**: erase the key. No rebuild of the tunnel bit happens here: the rebuild that follows (§3, step c) clears
wormhole bit 2.

**Insert**:

1. The other portal is the tunnel object's paired portal. The original reads it from the concrete
   `cSC4NetworkTunnelOccupant`, not through an interface (§9): the pointer member at **`+0x1D0`** of the object
   (offsets from the pointer that QI `0x8A4BD52B` and QI `0xA821EF94` return, the class's primary base), read by a
   one-instruction getter (`0x00647570`, COMDAT-folded with `cSC4City::GetVehicleManager`, so it has no symbol of
   its own) at `0x0071414B`. Its occupant position gives cell (ox, oz). With no paired portal (null), return:
   **no record** is created or changed (`0x00714154`, TN11). The paired portal is used under a temporary reference:
   `AddRef` (`0x0071415E`), its `AsOccupant` (network occupant slot 48: the `+4` sub-object) and that occupant's
   position (occupant slot 8), the city's `PositionToCell`, then `Release` (`0x0071419A`).

   **The member** (TG25). Type: a pointer to the other portal's `cSC4NetworkTunnelOccupant`, at its primary base
   (usable as a `cISC4NetworkOccupant*`). It is a **counted** reference:
   - set only by a non-virtual setter (`0x00647530`): if the new pointer differs, `AddRef` the new one, store it,
     `Release` the old one. Its only callers are the network tool's `InsertTunnelPieces` (`0x00628AFE`,
     `0x00628B08`), which pairs the two portals it has just made, each with the other;
   - released and set to null by the occupant's `Shutdown` (`0x00647500`–`0x00647516`), released, not nulled, by the
     function at `0x00647260` that restores the class's vtables before chaining to the base (`0x006472AA`; the
     destructor body *[inferred]*);
   - saved: `Write` (`0x00647580`) writes u16 version 2, the network occupant base record, then a bool "has pair"
     and, when true, the pair as a saved-object reference (QI `0xE4FDA3D4` of the pair, `0x005D99A0`); `Read`
     (`0x00647610`, version must be 2) releases any old pair and reads the reference back into the member, to be
     resolved as type `0x8A4BD52B` (`0x005D9730`), or nulls it when the bool is false. (In `Write` and `Read` the
     member appears at `+0x1C8` because those methods run on the `+8` serializable sub-object.)

   So both portals point at each other, each holding a reference on the other (a reference cycle that `Shutdown`
   breaks).
2. Network type = the first of 2, 6, 0, 1, 10, 12, 9 that the tunnel `IsOfType`. None → no record.
3. Travel mask = `0x1FE` (every type but Walk) *[quirk]*. The test that was meant to use the type's transit networks
   always passes.
4. `d = r24(√ r24(r24(r24(dz²) + r24(dy²)) + r24(dx²)))`, with (dx, dy, dz) = this portal's 3-D position minus the
   other's, in world units.
5. `cost[t] = r24(r24(d × invMaxSpeed[t][type]) × 0.0625)` for t = 0…8. `invMaxSpeed` is the class static
   (`0x00B4B110`) filled from the shipped "Max speed by Network" tuning (traffic-simulator.md):
   `invMaxSpeed = r24(1 / r24(r24(v × 16.666666) / CellWidthX))`, or FLT_MAX when v ≤ 0.

For types with zero speed on that network the cost is +∞. Example: Road, Car (v = 31, cell width 16), with portals
64 m apart: `invMaxSpeed = 0x3CFDB011` (0.030967744), cost = `0x3DFDB011` (0.123870976).

Each portal's insert message creates its own record. A tunnel works in both directions once both portals are
inserted.

The record is stored with find-or-insert on the key (`0x00713D60`), and the whole `0x2C`-byte record is then copied
over it (`0x00714386`–`0x00714396`). So a second insert for the same portal key **overwrites** the record: other
cell, mask, costs and type (TN11). Steps 1 and 2 return before this, so a portal with no pair, or of none of the
listed types, leaves any existing record unchanged.

### 4.9 Ferries

#### 4.9.1 Terminal activation (DoFerryTerminalActivated `0x00723620`)

1. `type` = property `0x0BD86FD3` (u8) of the building. Stop if it is missing, if `type > 2`, or if there are
   already 16 terminals of that type (signed compare of the type's count word with 16, `0x0072368A`).

   **Type 2 never activates** *[quirk]* (TN18). Its count word (simulator `+0x130`) is the begin pointer of the
   potential-accident vector, which the constructor reserves for 100 entries (`0x0071FF7C`). That is a heap
   address, and since the exe is not large-address-aware it is below 2³¹, so the compare always stops. Shipped
   content uses only type 0 (`0x038B0000`, passenger ferry) and type 1 (`0x03820000`, car ferry). A reimplementation
   should reject type 2 here.

   *[quirk]* If the exe is patched to be large-address-aware, a pointer at or above `0x80000000` compares
   negative. Activation then goes on, and step 3's count increment (`0x00723942`) adds 1 to that pointer, which
   corrupts the vector.
2. Get the prop occupants (type `0xA823821E`, QI `0xE9793A65`) in the lot rectangle. Classify them by
   `cISC4PropOccupant::GetPropType` (slot 7):
   - `0x2C520000` → departure cell;
   - `0x2C530000` → arrival cell;
   - `0x2C540000` → water cell: the first side d = 0…3 whose neighbour is outside the lot rectangle and is under
     water (terrain slot 34 `+0x88`, `(x, z)`). The water cell is that **neighbour** N, not the prop's own cell
     (`0x0072389F`).

   The departure and arrival cells are the prop's own cell (`PositionToCell` of its position). Later props of the
   same kind overwrite earlier ones. A water prop with **no** qualifying side writes nothing: an earlier water cell
   and its "found" flag stay as they were (TN10, `0x0072389B`). All three must be found, otherwise stop.
3. Append the terminal and count it. Insert its departure key and arrival key in the two tables.
4. For every other terminal of the **same type**, in list order: **RouteAndLinkTerminals**(new, other) (§4.9.3).
5. For every neighbour connection of type 9 (neighbours simulator slot 17 `GetConnectionsOfType(9, list)`; the
   list uses allocator A and is internal): take its cell (connection slot 6) and run
   **RouteTerminalToNeighbor**(new, x, z).

#### 4.9.2 Ferry phase of DoConnectionsChanged

For each terminal, with departure cell T = (x1, z1) and travel mask m:

1. If T is in R1:
   - `wormhole |= 4` at T;
   - make sure T has records for networks 0 and 1. A new record gets `connCount 1`, type 0,
     `invCap = invCapTable[0]` and extraCost 0.
   - For each side d whose neighbour N is in the city, with `ns = SideOf(T → N)`, and for t = 0…8:
     - if m has t: for each entry e ≠ ns of N with any exit in `conn[t]`, `SetConnection(N, e, ns, t)` (roads
       around the terminal lead into it);
     - else clear bit ns from every nibble of N's `conn[t]`.
2. If the arrival cell A = (x2, z2) is in R1: for each side d whose neighbour N is in the city, with
   `ns = SideOf(A → N)`, and for t = 0…8:
   - if m has t: for each pair (ns → x) present in N's `conn[t]` with x ≠ ns, `SetConnection(N, ns, x, t)`. If
     any were set, also `SetConnection(A, s, d, t)` for s = 0…3;
   - else clear N's `conn[t]` nibble ns.

#### 4.9.3 Routing

**RouteAndLinkTerminals(A, B)** (`0x0071BE50`):

1. Water FindPath from A's water cell to B's, giving `cells`, route A → B and route B → A (§4.10).
2. **Failure**: remove A's link to B and B's link to A, if present.
3. **Success**: find or append A's link keyed by B's departure key and set:
   - B's arrival cell;
   - `cost = r24(cells × 0.01)`;
   - route = A → B.

   Do the same for B's link to A, with route B → A. A new link starts with zero counters.

**RouteTerminalToNeighbor(T, x, z)** (`0x0071C0C0`):

1. (ix, iz) = GetNeighborConnectionInnerCell(x, z). Stop if the result is −1.
2. Water FindPath from T's water cell to (ix, iz).
3. **Failure**: remove T's neighbour link for (x, z).
4. **Success**: find or append it and set (x, z), (ix, iz), `cost = r24(cells × 0.01)`, route to the edge and route
   from the edge.

**Periodic re-routing** (WorkOnFerryRoutes `0x0071DC90`, work-cycle state 19, time-sliced like §4.12; cycle order in
traffic-simulator.md).

Entering the state (`0x0072134C`):

1. For every terminal: last riders := current riders and current := 0. The simulator adds the last riders to a city
   total. Each link's last := current and current := 0, for both counter pairs.
2. Set every terminal's re-route flag.
3. Rebuild the vector of type-9 neighbour connection cells.
4. Start at the first terminal.

For each terminal A in list order:

1. For every other terminal B of the same type whose re-route flag is still set: RouteAndLinkTerminals(A, B).
2. Clear A's flag. So each pair is routed once.
3. RouteTerminalToNeighbor(A, cell) for each cell of the vector.

When the list is exhausted, the pass posts message `0x2C06BA82` (§5).

When the city was saved in work-cycle state 19, DoPostCityInit sets every flag and restarts the pass, so links are
computed again after load; in any other saved state it leaves the ferries alone and the loaded links stand until the
next state-19 pass (traffic-simulator.md §3.5 step 11; corrected by TS7). In state 19 it does **only** that
(`0x00720C8A`–`0x00720CAE`): every terminal's re-route flag := 1, the pass points at the first terminal, and its
sub-step := 0 (TN8). Unlike entering state 19:
- it does **not** rotate any counter (riders and link counters keep their loaded values);
- it does **not** rebuild the vector of type-9 neighbour connection cells.

So the post-load pass reroutes every pair of terminals. Its neighbour step walks whatever the vector already holds:
empty in a newly constructed simulator (the constructor `0x0071F830` zeroes it), so loaded neighbour links are kept
as read. *[inferred: `Read` does not fill this vector; not traced byte by byte.]* The neighbour routes are recomputed
at the first state-19 pass.

#### 4.9.4 Terminal removal (in DoLotDeactivated)

The first terminal (list order) whose lot cell lies in the lot rectangle is removed. At most one terminal per call:

1. Decrement its type count (not below 0).
2. In every terminal reached through its terminal links, delete the link back to it.
3. If the ferry pass is running and points at it, advance the pass.
4. Erase its departure and arrival keys and free it.

No DoConnectionsChanged is run *[quirk]*. The departure cell keeps wormhole bit 4 and its network records until a
later rebuild covers that cell. A search that reaches it then finds no terminal in the departure table and adds no
ferry edges.

#### 4.9.5 Queries

**GetFerryTerminalCount(type)** (slot 32, `0x0071FFF0`): the u32 at simulator `+0x128 + 4·type`, with no bounds
check. Only types 0 and 1 have their own words; the constructor zeroes them (`0x0071FF83`). Higher types read the
members that follow:
- type 2 → `+0x130`, the potential-accident vector's begin pointer (non-null from construction);
- type 3 → `+0x134`, its end pointer;
- type 4 → `+0x138`, its capacity end;
- and so on.

So the original returns 0 only for an empty type 0 or 1, never for a type above 1 (TN18). FreeCity4 should return
0 for every type ≥ 2: no type-2 terminal can exist (§4.9.1), and the pointer values have no meaning.

**GetFerryRouteBetweenTiles(x1, z1, x2, z2)** (slot 29) returns a pointer to a stored route vector (internal; §9),
or null:

| Case | Lookup | Returns |
|---|---|---|
| (x1, z1) off-map | terminal by **departure** key (x2, z2); its neighbour link with off-map cell (x1, z1) | its route *from* the edge |
| (x1, z1) in city, (x2, z2) off-map | terminal by departure key (x1, z1); neighbour link (x2, z2) | its route *to* the edge |
| (x1, z1) and (x2, z2) in city, (x2, z2) **not** an arrival key | terminal by departure key (x1, z1); neighbour link whose in-city edge cell is (x2, z2) | route to the edge |
| (x1, z1) and (x2, z2) in city, (x2, z2) an arrival key of B | terminal by departure key (x1, z1); its link whose key is B's departure key | that link's route |

"Off-map" means outside `[0, X−1] × [0, Z−1]`.

**GetAllFerryRoutes(list<vector<u8>>&)** (slot 30):

1. Clear the list.
2. For each terminal: append a copy of every terminal link's route, then every neighbour link's route *to* the
   edge.

**GetFerryRoutesInUse(list<FerryRouteInfo>&)** (slot 31):

1. Clear the list. Keep a temporary set of visited terminal keys.
2. For each terminal A, in list order:
   - For each terminal link to B with a non-zero *last* counter (either pair), unless B was already visited: append
     a record with:
     - lot A and lot B (lot manager slot 6 at the lot cells, bool false, each AddRef'd);
     - type;
     - A→B counters, and B→A counters from B's link to A (0 when absent);
     - "neighbour" = 0;
     - neighbour cell = (−2, −2);
     - route A→B and route B→A.
   - For each neighbour link with a non-zero last counter: append a record with:
     - lot A, lot B = null;
     - type;
     - both counter fields of each pair = the link's counter;
     - "neighbour" = 1;
     - neighbour cell;
     - route to the edge and route from the edge.
   - Mark A visited.

Every counter in these records is a **last** value, the one rotated in at the start of state 19, never a current
one (TN9; `0x0071C2F1`, `0x0071C3F0`–`0x0071C3FC`, `0x0071C69B`–`0x0071C6A3`, `0x0071C4F0`, `0x0071C571`–
`0x0071C58F`). Both the non-zero test and the copied values use them:
- a terminal link holds counter 1 as (last `+0x20`, current `+0x22`) and counter 2 as (last `+0x24`, current
  `+0x26`);
- a neighbour link holds them as (`+0x2C`, `+0x2E`) and (`+0x30`, `+0x32`).

When B has no link back to A, the B→A counters are 0 and the B→A route is left empty.

FerryRouteInfo layout (`0x34` bytes, copy constructor `0x0056D770`). Its users are cSC4WatercraftManager and the
query tool:

| Offset | Field |
|---|---|
| +0x00 | `cISC4Lot*` A (referenced) |
| +0x04 | `cISC4Lot*` B (referenced, may be null) |
| +0x08 | u32 ferry type |
| +0x0C, +0x0E | u16 counter 1: A→B, B→A |
| +0x10, +0x12 | u16 counter 2: A→B, B→A |
| +0x14 | u8 neighbour flag |
| +0x16, +0x18 | i16 neighbour x, z |
| +0x1C | `vector<u8>` route A→B |
| +0x28 | `vector<u8>` route B→A |

**GetWaterRoute(x1, z1, x2, z2, vector<u8>&)** (slot 33): water FindPath from (x1, z1) to (x2, z2), writing the
forward route into the caller's vector. Returns the success bool in AL. Its caller is
`cSC4WatercraftManager::CreateMarinaBoat`.

### 4.10 Water pathfinder (cSC4WaterPathFinder)

#### 4.10.1 Depth map

The depth map is a `uint8` grid over the city. It is created directly (vtable `0x00A878C0`, not through GZCOM), with
type `0x49B9E602`, and given the ID `0xCBDF6BF8` by its slot 6.

For each cell:

- If surface water `IsNavigable(x, z, 0, null)` (cISTESurfaceWater slot 8) is false: 0.
- Otherwise let `w = GetWaterDepthForCell(x, z)` (slot 6, x87). If `w ≤ 0` or w is NaN: 0.
- Otherwise `byte = ceil(r24(min(w, 40.0) × 5.0))`, giving 1…200. Here min takes 40.0 only when `40.0 < w`.
- The ceiling is computed with the 1.5·2²³ idiom: `r = r24(r24(0.5 + p) + 12582912)` (round-to-even integer), and
  `r − 1` is used when `p = r − 1` exactly. This equals `ceil(p)` for 0 ≤ p ≤ 200.

Updates are lazy. A dirty rectangle (with a flag) is updated before each FindPath and on message `0x6A3B0AFD`.
"dirty ∪= r" means: if the flag is clear, dirty := r and set the flag; otherwise dirty := the union (min x1, min
z1, max x2, max z2).

| Message | Effect |
|---|---|
| `0x26D31EC1` | whole map dirty and updated at once |
| `0x278E6003`, `0x678E6E3A` | dirty ∪= (data1 >> 16) − 1 … data1 & 0xFFFF, (data2 >> 16) − 1 … data2 & 0xFFFF, clamped to the map (terrain change) |
| `0x99EF1142` / `0x99EF1143` with data3 = `0xC772BF98` | r = the occupant's (void data1) **bounding city cells** (occupant slot 11 `GetBoundingCityCells`). If the cell `(r.x1, r.z1)` is under water (terrain slot 34, NW corner only): dirty ∪= r, not grown and not clamped (bridges). TN15, `0x00724B19`–`0x00724BC8` |

**The update** (UpdateDepthMap `0x00724840`) visits every cell of the dirty rectangle, x inner and z outer, then
clears the flag. It does not clamp the rectangle and does not check bounds (TN24, from the exe). An off-map cell is
both asked and written:
- **asked:** surface water `IsNavigable` (`0x0073EBF0`) tests the city's `CellIsInBounds` (`0x004594D0`, called at
  `0x0073ECA2`) first and returns false. So the depth is never read, and the cell's value is 0;
- **written:** the 0 is stored through the grid's raw rows (`0x007249A6`–`0x007249B2`), at row x, byte z, with no
  check. For x in the map and z past it, this writes past the end of row x. For x outside the map, it uses a row
  pointer read from outside the row table. Either way it is an out-of-bounds heap write *[quirk]*.

*[choice]* FreeCity4 visits only the in-map part of the rectangle. The in-map cells get the same values. Example on a
256 × 256 map: a bridge occupant whose bounding cells are (250, 254)–(252, 256) makes the original write a 0 at byte
256 of rows 250, 251 and 252. That is one byte past each 256-byte row. FreeCity4 updates (250…252, 254…255) and
nothing else.

#### 4.10.2 FindPath(sx, sz, tx, tz, &cells, fwd, back) (`0x00725170`, pop 28, AL)

1. If the map is dirty, update it.
2. Both cells must have the same non-zero water-body ID (surface water slot 9 `GetBodyOfWaterMap`, then grid slot 7
   at the cell). Otherwise fail.
3. A\* over cells:
   - **Nodes**: key = cell key; predecessor; `g`; `f`; open and closed flags.
   - **Node store**: a 3079-bucket table (`(key × 0x9E3779B1) mod 3079`) over pages of 4096 nodes (allocator C).
     It never fails and is reset per call.
   - **Start node**: `g = 0`, `f = r24(√ (float)(dx² + dz²))`, with integer (dx, dz) = start − target.
   - **Open list**: a sorted singly linked list. Insert before the first node with a strictly greater f, so equal f
     is FIFO. A NaN f is treated as "not less". To update a node, unlink it and insert it again.
4. Loop: pop the head. If it is the target, succeed. Otherwise, for directions d = 0…3 in order (DX, DZ as §2.1):
   - Skip b if it is outside `[0, X−1] × [0, Z−1]`.
   - Skip b if its depth byte is 0, unless b is the target.
   - Skip b if it is the current node's predecessor.
   - `g' = r24(r24(r24(r24(40.0 − r24(depth × 0.2)) × 0.1) + g) + 1.0)`. 0.1 is the static at `0x00B0D490`, never
     written. Every constant is a **float32** memory operand (`0x00725370`–`0x00725385`, TN14):

     | Constant | Bits | Address |
     |---|---|---|
     | 0.2 | `0x3E4CCCCD` (0.20000000298…) | `0x00AB34F4` |
     | 40.0 | `0x42200000` | `0x00AB34F0` |
     | 0.1 | `0x3DCCCCCD` (0.10000000149…) | `0x00B0D490` |
     | 1.0 | `0x3F800000` | `0x00A81228` |

     Each operation is rounded to 24 bits. The depth byte is loaded exactly (`fild`). Using the double 0.2 instead
     changes `r24(depth × 0.2)` for 39 of the depth bytes 0…200, and the step cost for 25 of them (at g = 0).
     Examples (g = 0):

     | Depth byte | Step cost |
     |---|---|
     | 200 | 1.0 |
     | 100 | 3.0 |
     | 47 | 4.06 = `0x4081EB85` (the double 0.2 would give `0x4081EB86`) |
     | 0 (target on land) | 5.0 |

     For depth 47: `r24(47 × 0.2f)` = 9.4000006 (`0x41166667`); `40 −` that = 30.599998 (`0x41F4CCCC`);
     `× 0.1f` = 3.06 (`0x4043D70A`); `+ 0`, then `+ 1.0` = `0x4081EB85`.

   - Get or create b. If b is open and not `g' < b.g`, skip. If b is closed and not `g' < b.g`, skip.
   - Otherwise set closed = false, `b.g = g'`, `b.f = r24(√ (float)(Δx² + Δz²) + g')` (to the target),
     predecessor = current. Reposition b if it was open; else insert it and mark it open.
   - Close the current node.
5. The open list empties → fail (AL 0).

**Success**:

- `cells` = the number of nodes on the path, including both ends.
- If `fwd` is given, it receives the route start → target.
- If `back` is given, it receives the route target → start.

#### 4.10.3 Water route format (MakeFinishedPath `0x00724C50`)

The vector is cleared (end = begin) and filled with allocator B:

- `[0, x, z]` of the first cell;
- then runs `[0x40, count, ⌈count/4⌉ bytes]` of unit-step codes (0 = −x, 1 = −z, 2 = +x, 3 = +z, four per byte,
  least significant first). A run holds at most 255 steps; the 256th step starts a new run.

This is the pathfinder's compressed format with travel type 0 (pathfinder.md §4.15). The watercraft code reads it
with `cSC4CompressedRouteReader`.

### 4.11 Subnetworks (UpdateSubnetworks `0x007183B0`, work-cycle state 12)

**Clear** (`0x0070FCE0`): subnetwork counter := 1, and every record's `subnet[0…3]` := 0.

**Seeds**:

1. Forward pass: x = 0 … X−2 (outer), z = 0 … Z−2 (inner). For the cell's first car record (n = 1): if
   `subnet[0] = 0`, Fill(x, z, 0); then if `subnet[1] = 0`, Fill(x, z, 1).
2. Backward pass: x = X−2 … 0, z = Z−2 … 0, the same with entry sides 2 and 3.

The last row and the last column are never seeds *[quirk]*. They are still labelled when reached from other cells.

**Fill(x, z, e)** labels the (cell, entry side) states reachable along directed car edges. It uses travel type
`rep(1)` = Car and an explicit stack (a deque, allocator B, internal).

1. Start only if:
   - the cell has a car record with `subnet[e] = 0` and `connCount ≤ 2`;
   - and the cell is not a tunnel cell (wormhole bit 2).
2. The state is (cell, entry e, next k). For k = 0…3, if `conn[1]` has bit `k + 4e`:
   - `subnet[e] := counter` in this cell's record, and remember that something was labelled.
   - Find the next state:
     - **Off-map neighbour**: if the cell also has bit `e + 4k` (the connection from k back to e) and
       `subnet[k] = 0`, continue in the same cell with entry k. This turns around at map edges.
     - **In-city neighbour N with a car record**, where `subnet[(k+2) mod 4] = 0`: continue at N with entry
       `(k + 2) mod 4`.
     - **N without a car record but with switch ID s ≠ 0**: step further in direction k while the cells stay in the
       city and carry ID s. At the first cell with another ID: if it has a car record with `subnet[(k+2) mod 4] = 0`,
       continue there. This skips straight across a switch lot.
   - When a next state is found, push the current state with k + 1 and move to the next state (k = 0).
3. After k = 3, a tunnel cell (wormhole bit 2) whose tunnel record allows Car:
   - `subnet[e] := counter`;
   - if the other portal's car record has `subnet[e] = 0` (the **same** entry side), push and continue there with
     entry e.
4. Pop and resume. When the stack is empty, `counter += 1` if anything was labelled.

**Resume point** (TN16, `Fill` `0x00717EC0`). The state's k runs 0…5, and a popped state resumes at its stored
k, without redoing earlier work:
- k = 0…3: step 2 continues with that k;
- k = 4: step 3 runs (`0x00718186`);
- k = 5: step 3 is skipped and the next state is popped.

Step 3 sets k := 5 **before** it looks up the tunnel record (`0x007181A5`). So the state it pushes is resumed
after step 3 and never redoes it, which would loop between the two portals forever. A state pushed by step 2 at
k = 3 is stored with k = 4, so step 3 still runs once for it when it is popped.

Labels are "first reached" labels, not strongly connected components. A one-way road reached first from seed A
keeps A's label. *[compat]*

### 4.12 Subnetwork info (UpdateSubnetworkInfo `0x00716180`, state 13; WorkOnUpdateSubnetworkInfo `0x0071DC50`)

**SubnetworkInfo** (`0x1C` bytes):

- `population`: `map<u32 developerType, u32>`;
- `capacity[4]`: u32.

When state 13 is entered (in the work cycle, and synchronously in DoPostCityInit):

1. Clear the working vector and resize it to `counter` default entries. Index 0 is unused.
2. Reset the scanning region to the whole city (cell order: x inner, z outer). In the work cycle this is the
   simulator's own **scanning iterator** (member `+0x818`, the one saved in traffic-simulator §6.1 row 14), reset
   on entering 13 to state 0 at (0, 0) with its rectangle and step kept (`0x00721308`–`0x0072132A`; state 2 at once
   if (0, 0) is outside the rectangle). Entering 12 does not touch it, and UpdateSubnetworks (state 12) does not use
   it (TS64).

For each cell (x, z) of the region with `lot = GetLot(x, z, true)` (lot manager slot 6; only the lot's anchor cell
answers, so each lot is visited once):

1. Rectangle `r` = the lot's bounding rectangle (lot slot 14). Ring = the boundary (BoundaryIterator `0x0060CEE0`)
   of `r` grown by 1 and clamped to the city. Skip the ring cells whose x is outside `[r.x1, r.x2]` **and** whose z
   is outside `[r.z1, r.z2]`, which are the four corners.

   The ring is walked in **boundary order** (pathfinder.md §4.6; x varies fastest, `0x0071633C`–`0x00716357`):
   - the first row `z = z0`, from `x0` to `x1`;
   - each middle row at `x0`, then at `x1`;
   - the last row `z = z1`, from `x0` to `x1`.
2. `ids` = the distinct non-zero `subnet[0…3]` values of the first car record of each remaining ring cell, in order
   of discovery.
3. If `ids` is empty: remove lot property `0x2A4F9C74` (property holder slot 17). Done.
4. Otherwise:
   - set lot property `0x2A4F9C74` = `ids` (u32 array);
   - for every (type, count) in every map of `lot.GetPopulations()` (slot 54): `info[id].population[type] += count`
     for each id;
   - for k = 0…3: `info[id].capacity[k] += GetMaxTripCapacity(lot's property holder, k)` (slot 20) for each id.

After the last cell, the neighbour connections are added. For every connection of the neighbours simulator (slot 16
`GetAllConnections`; list with allocator A, internal), with cell (x, z) from connection slot 6:

1. Map (x, z) to the in-city cell (x', z'): x = −1 → x' = 0; x = X → x' = X−1; else z = −1 → z' = 0; z = Z →
   z' = Z−1. The other coordinate is kept.
2. If the **in-city** cell (x', z') has a car record, and `GetNeighborPoolPropertyHolder(1, x, z)` (non-virtual
   `0x007114E0`) gives a holder, then the steps below run. The holder is asked with the **off-map** cell (x, z), the
   connection's own cell, not (x', z') (TN6, `0x0071674F`–`0x00716755`):
   - `c[k] = GetMaxTripCapacity(holder, k)`;
   - add `c` to `info[subnet[e]].capacity` for **each** e = 0…3 with `subnet[e] ≠ 0`. This is not deduplicated: a
     label shared by two entry sides is counted twice *[quirk]*.

Then the working vector is copied to the published vector. All queries read the published vector.

The work is time-sliced with an adaptive work-unit counter (statics `0x00B4B33C`, `0x00B4B340`, start 1024; the same
scheme as pathfinder.md §4.11). Slicing changes only when the result is published, not the result itself.

**The iterator during and after state 13** (TS64). The deadline test comes before each step of the iterator
(`0x00716212`), so a call can stop between two cells; the iterator then reads state 1 (running) at the last cell
visited, and the next call continues from it. A step adds `step` (1) to x; past `x1`, x returns to `x0` and z grows
by `step`; past `z1` the state becomes 2 (done) and the neighbour connections are added in the same call with no
further deadline test. So right after 13 finishes, the iterator reads state 2 at `(x0, z1 + 1)`, for a 128-cell
city (0, 128), and it stays so until the next scanning state resets it (traffic-simulator §3.2). Example: entering
13 shows state 0 at (0, 0); a call that stops after visiting (34, 87) shows state 1 at (34, 87). DoPostCityInit
(§1 step 7) runs the same work on a **local** iterator with a deadline about 2^63 cycles away
(`0x00720808`–`0x00720847`), so the member iterator keeps its loaded value there.

### 4.13 Connectivity queries (slots 36–42)

The lot's subnetwork list is its property `0x2A4F9C74`, as written by §4.12. A lot built after the last pass has no
list until the next pass.

| Slot | Query | Result |
|---|---|---|
| 36 | `AreLotsConnected(A, B)` | AL true when both lots have a non-empty list and the lists share an ID. |
| 37 | `GetConnectedOccupantCount(lot, type)` | Maximum over the lot's IDs (with `id < size`) of `published[id].population[type]` (0 when absent). 0 without a list. |
| 38 | `GetConnectedDestinationCount(lot, k)` | k > 3 (signed) → 0. Otherwise the maximum over the lot's IDs (with `id < size`) of `published[id].capacity[k]`. 0 without a list. `cSC4Lot::HasJobAccess` (`0x006BFCE0`) is true when this is > 0 for k = 0 or 1. A negative k is **not** rejected (see below the table). |
| 39 | `GetSubnetworksForLot(lot, vector<u32>&)` | Clears the vector, then reads the property into it. AL = whether the property exists. |
| 40 | `GetSubnetworksInRectangle(rect, vector<u32>&)` | The rectangle is inclusive (x1, z1, x2, z2). Collect the distinct non-zero `subnet[0…3]` of the first car record of every in-city cell of rect into an SGI `hash_set<u32>` (hint 100 → 193 buckets, identity hash). Clear the vector and append the set in hash order (bucket order; within a bucket, most recent first). For fewer than 194 IDs all below 193, this is ascending order. |
| 41 | `GetSubnetworksInRegion(region, vector<u32>&)` | As slot 40, over the cells of an SC4CellRegion: its bounding rectangle filtered by its bitmap. Row `x − x1`, bit `(z − z1)`, 32 bits per word, least significant first; cells outside width × height are excluded. |
| 42 | `GetOccupantCountForAllSubnetworks(type, vector<u32>&)` | `vector.assign(size, 0)`, then `v[id] = published[id].population[type]` (if present) for id = 1 … size−1. |

**Negative k in slot 38** *[quirk]* (TN17, `0x007106C7` `jle`, `0x0071075B`–`0x00710767`). The only bound test is
the signed `k > 3`, so a negative k passes. The function then reads the u32 at index `7·id + 3 + k` of the
published vector (0x1C-byte records, `capacity` at `+0xC`). The result is still the unsigned maximum, starting
from 0:
- k = −1, −2, −3 read the three words of `published[id].population`, the map object itself. Its word `+0` is the
  tree's header-node pointer (the map's end test compares with it, `0x00711B12`). So k = −3 returns a heap
  address. The words `+4` and `+8` are, in STLport 4's layout, the element count and the comparator's padding
  *[inferred]*.
- k = −4 … −7 read `published[id−1].capacity[3 … 0]`, and lower k read further back.
- Once `7·id + 3 + k < 0`, the read is before the vector's start.

The result is therefore **not** 0, and has no meaning. The known caller (`HasJobAccess`) passes only 0 and 1.
FreeCity4 should return 0 for k < 0.

Slots 40, 41 and 42 return garbage in AL. The SDK declares bool. A replacement returns true *[choice]*. The landfill
spec's IsZoneConnected uses slots 41 and 42 (landfill.md §4.7.2).

### 4.14 GetDesiredLotInsertionPoint (slot 22)

This returns the simulator's *safe lot iterator* while the work cycle is in state 1 (the lot sweep), else null. The
iterator is obtained once at Init from lot manager slot 15 (`GetSafeLotIterator`).

The lot manager's lot insertion (`0x0072DB60`) asks for it. When non-null, the new lot is inserted into the lot list
through that iterator (just after the iterator's current element, with the iterator stepped back). Otherwise the lot
is appended. Inserting this way keeps an ongoing sweep consistent *[inferred]*. The exact list effect is the lot
manager's (its spec).

### 4.15 IsRoadDamaged(x, z) (slot 16)

True when the cell key is in the simulator's pothole table. The table is maintained by ApplyPotholeChanges
(`0x00715FB0`, traffic-simulator.md). §4.2 reads it for `extraCost`.

---

## 5. Messages posted

| ID | When | Data |
|---|---|---|
| `0x2C06BA82` | the periodic ferry pass (§4.9.3) has finished all terminals | a cRZMessage2Standard posted to the simulator's message server (slot 4): void1 = the simulator (slot 12), and the simulator again through slot 33. Consumers are not identified *[inferred: watercraft manager]*. |

The rebuilds post nothing.

---

## 6. Persistence

### 6.1 Traffic simulator state

The traffic simulator's save record belongs to traffic-simulator.md. For this spec:

- **Saved**:
  - the switch mode-mask grid and the switch ID grid (as serialisable grids);
  - the next switch ID;
  - the switch table: count, then per switch its ID, building reference, load, time factor, capacity, the access
    flag and cell, the two counters, and the rules (count, then kind, four sides, from, to; read only for versions
    above `0x1C`). This is right and the old §4.7.2 remark was wrong (TN3). `DoPostCityInit` re-reads the rules
    anyway and appends them (§4.7.2);
  - the **ferry terminals** (corrected): `Write` `0x00712400` writes the terminal list after the per-type tables
    (count, per-terminal fields, link lists with their f32 costs, s16 cells, u16 counters and route vectors), and
    `Read` `0x00721990` rebuilds them through the same helper as `DoFerryTerminalActivated` (`0x00713DF0`, called
    at `0x00723593` and `0x007235B1`). The byte layout is in traffic-simulator.md §6.
  - the **tunnel table** (corrected): it is record row 17; `Read` fills the `hash_map<u16, TunnelInfo>` at
    `0x00722E18` (save-records.md §7.1.1). The occupant-insert messages during load may add the same keys again.
- **Not saved** (rebuilt at load):
  - cell connection records and network info (§1, DoPostCityInit);
  - subnetwork labels and info;
  - the water depth map (recomputed on message `0x26D31EC1`).
- **Lot property** `0x2A4F9C74` is saved with the lot as an ordinary property. It is rewritten by the next §4.12 pass.

### 6.2 Traffic network map (`0x6A0F82B2`) *[compat]*

This is the separate **automata traffic network map**, `cSC4TrafficNetworkMap`, city component 42
(foundations §3.6). `0xCA0F81B9` is its requested **interface ID**; its serializable class/record type is
`0x6A0F82B2` (`GetGZCLSID`, `0x004B3AB0`; factory `0x004B42E0`). Do not put this payload in the traffic
simulator's `0x6990C1AA` record. Its DBPF key is `0x6A0F82B2 / 0x299B2D1B / 0`, with the class-entry framing
of foundations §3.3.

**Sources and conventions.** Write `0x004B1CE0`, Read `0x004B4DE0`. All tables below are in wire order, without
alignment or padding. `u8`, `u16`, `u32`, `s32`, `f32` occupy 1, 2, 4, 4, 4 bytes respectively, little-endian.
`ref` is foundations §2.7: a `u32 objectID`, followed by a `u32 classID` only when the ID is nonzero. Counts are
`u32` element counts, not byte sizes. The memory offsets below identify otherwise unnamed scalar state; they
are not extra wire fields. `M` means the map's serializable-interface pointer, `T` a tile record, and `I` an
intersection hash node (including its link and occupant key).

**Versions.** Write emits `u16 7` (`0x004B1CEF`–`0x004B1CF7`). Read consumes the version and rejects **0, 1, 2**
without reading the rest (`0x004B4E05`–`0x004B4E1C`). It has **no upper-version check**: versions 3–65535 enter
the reader; values above 7 take all the version-7 branches. The accepted legacy layouts are:

| Version | Tile's four-by-four values and following discard | Tile extra vector | Intersection flag and reference list | Final occupant table |
|---|---|---|---|---|
| 3 | `f32` values, then one discarded `f32` | absent | absent | absent |
| 4 | `u16` values, then one discarded `u16` | absent | absent | absent |
| 5 | as v4 | present | absent | absent |
| 6 | as v4 | present | absent | present |
| 7 and higher | as v4 | present | present | present |

These gates are instructions at `0x004B5280`, `0x004B52F5`, `0x004B531A`, `0x004B564F`, `0x004B566E` and
`0x004B59A7`. A vestigial pre-v2 branch later in Read is unreachable through the entry version check; it does
not describe an accepted v1/v2 format.

#### 6.2.1 Outer order

| Order | Field | Encoding / repetition |
|---|---|---|
| 1 | version | `u16`, writer 7 |
| 2 | cell bucket count | `u32`, writer `M+0x10` × `M+0x14` |
| 3 | tile count | `u32`, total linked tile records across all cell buckets |
| 4 | tiles | that many §6.2.2 records |
| 5 | intersection count | `u32`, `M+0x2C` |
| 6 | intersections | that many §6.2.3 records |
| 7 | keyed-reference group count | `u32`, `M+0xA74` |
| 8 | keyed-reference groups | each: `u32 key`, `u32 referenceCount`, then `referenceCount` × `ref` (read IID `0xA821EF94`) |
| 9 | four map integers | `s32` × 4, `M+0x54`, `+0x58`, `+0x5C`, `+0x60` |
| 10 | first map value and flag | `f32` (`M+0xA54`), `u8` (`M+0xA58`) |
| 11 | second map value and flag | `f32` (`M+0xA5C`), `u8` (`M+0xA60`) |
| 12 | occupant table, version ≥ 6 only | `u32 count`, then that many §6.2.4 records |

There is no footer. The four integers and two value/flag pairs occupy **26 bytes** in every accepted version
(`0x004B219F`–`0x004B2203`, `0x004B5939`–`0x004B599F`). The keyed-reference groups are neither inline
occupants nor lists of raw pointer-sized numbers: each member is a variable-width `ref`
(`0x004B2114`–`0x004B2151`, `0x004B57C1`–`0x004B585C`).

**Walk order.** Tiles are written by increasing cell bucket index, following each bucket's linked list from
head to tail (`0x004B1D50`, `0x004B1F57`–`0x004B1F71`). Per-tile vectors and all reference lists retain their
stored order. Intersections and the final network-occupant table are address-keyed hash walks in the original;
under simulator §5.8 **Address order**, write them in the occupants' creation order, first created first.
The intervening `u32`-keyed reference-group table follows its hash-container walk, not the occupant creation
walk. Read appends tiles to the saved cell's chain and reconstructs the tables in the order encountered.

#### 6.2.2 Per-cell network-tile record

| Order | Field | Wire type / rule |
|---|---|---|
| 1 | cell bucket index | `u32` |
| 2 | network occupant | `ref`, read IID `0xA821EF94` |
| 3 | path-side vector size | `u32 S` |
| 4 | path-side vectors | `S` repetitions: `u32 sideIndex`, `u32 pathCount`, then `pathCount` pairs of **`u32 pathKey`, `f32 value`** |
| 5 | tile occupant list | `u32 count`, then `count` × `ref`, read IID `0xA9B40F05` |
| 6 | discarded flag | `u8`; writer always 0, reader consumes without retaining |
| 7 | three tile state words | `u32` × 3 (`T+0x20`, `+0x24`, `+0x28`) |
| 8 | four timing rows | repeat exactly 4 times: `f32 timer`, then 4 values (`f32` in v3, `u16` in v4+) |
| 9 | discarded value | `f32` in v3; `u16` in v4+, writer always 0 |
| 10 | extra vector, v5+ only | `u32 count`, then `count` triples **`s32 key`, `f32 value`, `u16 state`** (10 bytes per triple) |

The writer emits side indices **0, 1, …, S−1**; the reader consumes each index but fills the vector in encounter
order (`0x004B1DB6`–`0x004B1DF9`, `0x004B4FC0`–`0x004B5038`). Do not confuse this variable-length path-side
vector with either of the two fixed four-side blocks elsewhere in the format. A path pair is 8 bytes on disk,
although its memory element is 16 bytes. An extra triple is 10 on disk, although its memory element is 12
(`0x004B1F2D`–`0x004B1F46`, `0x004B5360`–`0x004B5391`).

The four timers are held internally as `u16` (`T+0x2C` onwards), but are **always written as `f32`**, including
v7 (`0x004B1EA4`–`0x004B1EB7`). Their four associated values are the `u16[4][4]` beginning at `T+0x34` in
row order, not column order. Read converts the timers and the legacy v3 float values to `u16` using the game's
single-precision add/bit conversion: add **12,582,912.0** (constant `0x00A8FDF4`) to the input and round the sum
to binary32; add `0xB4C00000` to that sum's bit pattern modulo 2³². Use the resulting unsigned integer when it
is at most 65,535, otherwise `0xFFFF` (`0x004B5240`–`0x004B5274`, `0x004B5295`–`0x004B52CA`; arithmetic
conventions in foundations §18). The flag, three state words, timing
rows and discard are present even for an empty path vector and an empty occupant list.

#### 6.2.3 Intersection record and its four sides

| Order | Field | Wire type / rule |
|---|---|---|
| 1 | network occupant key | `ref`, read IID `0xA821EF94` |
| 2 | intersection flag, v7+ only | `u8` (`I+0x58`) |
| 3 | marked-train list, v7+ only | `u32 count`, then `count` × `ref`, read IID `0x8B99EE7C` |
| 4 | side 0, then 1, 2, 3 | each: **`u32 state`, `u8 flag`, `f32 value0`, `f32 value1`, `f32 value2`** |

Each side occupies **17 wire bytes** and 20 memory bytes, starting at `I+0x08` with a memory stride of `0x14`.
There are no side tags or counts around these four sides (`0x004B2030`–`0x004B206E`,
`0x004B56E0`–`0x004B5742`). The marked-train list is the list changed by `MarkTrainInCell` (`0x004B19F0`).
Versions 3–6 consume neither the intersection flag nor that list; the list starts empty. New-intersection
scalar initialization in reference runs follows simulator §5.8 **Uninitialised values**. On every load the
reader also clears unsaved per-side byte `I+0x0D+0x14×side` and intersection byte `I+0x6C`; these are not wire
fields (`0x004B566A`, `0x004B5728`).

#### 6.2.4 Final network occupant → four lists record (v6+)

Read one network-occupant **`ref`** (IID `0xA821EF94`), then exactly four list records in stored side order.
Each list record consists of **`u32 length`, `length` × `u32 value`, `u16 state`**. The values are plain
32-bit words, **not object references**; there is no conditional class ID after a nonzero value. The trailing
`u16` exists even for an empty list (`0x004B2263`–`0x004B22C4`, `0x004B59E0`–`0x004B5B28`). The `u16` is the junction's current index into that list (ToggleTrainJunction `0x004B0990` advances it); the original's add-junction (`0x004B44F0`) sets it to 0 only for a side with a path, and leaves the other sides' stack contents, which the game never reads. Under simulator §5.8 **Uninitialised values** it is 0.

The original loader uses an 8-bit loop index for the values despite the 32-bit length. A length above 255
therefore cannot terminate that loop normally (`0x004B5A38`, `0x004B5AF8`–`0x004B5B0D`). *[choice]* Reject
such malformed lists and all counts/indices that cannot fit the enclosing record or allocated cell table;
do not reproduce an unbounded read. Successful ordinary Read/Write returns “stream not in error.”

#### 6.2.5 Byte-size checks and corpus proof

These sizes exclude class-entry framing. Let `R` be the sum of the actual reference widths **within the
record being sized**, `S` a tile's path-side vector size, `P` its total path-pair count, and `X` its extra-triple
count. In particular, a null reference contributes 4 to R; a non-null reference contributes 8.

| Record | Wire bytes |
|---|---|
| Tile, v3 | `109 + R + 8×S + 8×P` |
| Tile, v4 | `75 + R + 8×S + 8×P` |
| Tile, v5+ | `79 + R + 8×S + 8×P + 10×X` |
| Intersection, v3–6 | `68 + R` |
| Intersection, v7+ | `73 + R`, including the marked-train references in R |
| Keyed-reference group | `8 + R` |
| Final occupant entry, v6+ | `24 + R + 4×(sum of its four list lengths)` |

The complete map payload is **44 bytes** in v3–5, or **48 bytes** in v6+, **plus** the sizes of all its tiles,
intersections, keyed-reference groups and final occupant entries. Those fixed bytes include the outer counts
and the 26-byte scalar tail. For example, a v7 map with all four outer element counts zero occupies 48 payload
bytes, or 60 bytes including its class-entry header. No cell bucket emits a placeholder for an empty chain.

**Corpus proof (SR33-2, 2026-10-28).** A throwaway parser checked all **547** available reference
`build-runs/golden/<key>/end.sc4` files, in place. All 547 v7 map objects consumed their complete decompressed
class records (**583,750,387 bytes**, including the 12-byte entry headers), with zero bytes left over. This
included 1,602,187 tiles, 44,270,243 path pairs, 95,284 tile-list references, 1,608,109 extra triples,
110,768 intersections and 245 final occupant entries with 498 list values. The size equations above also
matched every map payload. The marked-train and keyed-reference lists were empty in this corpus; their layouts
and the legacy gates are executable evidence, not exercised legacy save fixtures.

---

## 7. Interface slots

### 7.1 cISC4TrafficSimulator (vtable `0x00AB33D0`), slots owned by this spec

Pop and return channel were checked with `abi_extract.py --check` and by reading the epilogues.

| Slot | Offset | Address | Signature as compiled | Pop | Return | SDK |
|---|---|---|---|---|---|---|
| 16 | +0x40 | `0x00720070` | `bool IsRoadDamaged(int x, int z)` | 8 | AL | ok |
| 22 | +0x58 | `0x00720010` | `cSC4SafeListIterator<cISC4Lot*>* GetDesiredLotInsertionPoint()` | 0 | EAX pointer | **SDK error**: declares `uint32_t`. It is a pointer (or null). |
| 23 | +0x5C | `0x00710320` | `float GetCapacity(uint8 transitNetwork, int x, int z)` | 12 | **ST0** | **SDK error**: declares `uint32_t GetCapacity(uint32_t networkType, …)`. It returns a float on x87, and the first argument is a *transit network* 0…5 compared as a full u32 with the record's byte. |
| 25 | +0x64 | `0x00710370` | `uint8 GetConnectionCount(uint32 transitNetwork, int x, int z)` | 12 | EAX (zero-extended u8) | argument is a transit network, not a network type (SDK name `networkType`) |
| 28 | +0x70 | `0x00714460` | `void GetTransitSwitches(ilist<cISC4Occupant*>&)` | 4 | garbage | SDK says bool. Clears the list (releasing elements, `0x00413E60`), appends every switch building in table order (AddRef, allocator A nodes). |
| 29 | +0x74 | `0x00713B70` | `SC4Vector<uint8>* GetFerryRouteBetweenTiles(long, long, long, long)` | 16 | EAX pointer | **SDK error**: declares `int32_t`. It returns a pointer to an internal vector. |
| 30 | +0x78 | `0x00718C40` | `void GetAllFerryRoutes(SC4List<SC4Vector<uint8>>&)` | 4 | garbage | SDK says bool |
| 31 | +0x7C | `0x0071C250` | `void GetFerryRoutesInUse(SC4List<FerryRouteInfo>&)` | 4 | garbage | SDK says bool |
| 32 | +0x80 | `0x0071FFF0` | `uint32 GetFerryTerminalCount(uint32 type)` | 4 | EAX | ok (no bounds check) |
| 33 | +0x84 | `0x0070EC10` | `bool GetWaterRoute(long x1, long z1, long x2, long z2, SC4Vector<uint8>&)` | 20 | AL | ok |
| 35 | +0x8C | `0x0071CB80` | `bool GetTransitSwitchQueryData(x, z, TransitSwitchQueryData&)` | 12 | AL | SDK names the first two arguments `uint32_t unknown1, unknown2`: they are a cell. Lookup by the switch ID grid, else by a ferry terminal whose lot cell is in the lot at (x, z). The data fields belong to traffic-simulator.md. |
| 36 | +0x90 | `0x00710540` | `bool AreLotsConnected(cISC4Lot*, cISC4Lot*)` | 8 | AL | ok |
| 37 | +0x94 | `0x00711A70` | `uint32 GetConnectedOccupantCount(cISC4Lot*, uint32 developerType)` | 8 | EAX | ok |
| 38 | +0x98 | `0x007106C0` | `uint32 GetConnectedDestinationCount(cISC4Lot*, int k)` | 8 | EAX | ok |
| 39 | +0x9C | `0x00711B50` | `bool GetSubnetworksForLot(cISC4Lot*, SC4Vector<uint32>&)` | 8 | AL | ok |
| 40 | +0xA0 | `0x00718D20` | `void GetSubnetworksInRectangle(SC4Rect<int> const&, SC4Vector<uint32>&)` | 8 | garbage | SDK says bool |
| 41 | +0xA4 | `0x00718F90` | `void GetSubnetworksInRegion(SC4CellRegion<long> const&, SC4Vector<uint32>&)` | 8 | garbage | SDK says bool |
| 42 | +0xA8 | `0x007143B0` | `void GetOccupantCountForAllSubnetworks(uint32, SC4Vector<uint32>&)` | 8 | garbage | SDK says bool |

A replacement returns true wherever the original returns garbage *[choice]*.

### 7.2 cISC4NetworkOccupant (vtable `0x00AA8080`), slots used by the rebuild

| Slot | Offset | Address | Behaviour | Pop | Return |
|---|---|---|---|---|---|
| 48 | +0xC0 | `0x005FD7E0` | `AsOccupant()`: `this + 4` (the cISC4Occupant sub-object), null for a null `this`. COMDAT-folded, so the symbol at that address is unrelated. | 0 | EAX |
| 49 | +0xC4 | `0x0061B510` (`0x0061D380` in `cSC4NetworkOccupantWithPreBuiltModel`) | `GetPathInfo()`: builds the path info on the first call and caches it in the occupant (`+0x140`); returns the cached pointer **without** `AddRef`, so the caller borrows it and must not release it (TG24). May return null (the pre-built-model override returns null when it has no model). | 0 | EAX |
| 53 | +0xD4 | `0x006196A0` | `PathConnection(pathType, entry, exit)`: makes sure path info exists (calls GetPathInfo unless network flag `0x2000` is set), then tests the connection table. With `0xFF` wildcards it returns the number of matches; otherwise a bool. | 12 | AL (u8) |
| 54 | +0xD8 | `0x00619790` | `AddPathConnection(pathType, entry, exit)`: sets the bit for `pathType` and also for path type 0, then sets network flag `0x2000`. | 12 | none |

**SDK error**: the SDK declares slot 53 as `AddPathConnection` (void) and slot 54 as `PathConnectionExists`. The
compiled order is the reverse (checked at `0x006196A0` and `0x00619790`).

### 7.3 cISC4PathInfo (`cSC4PathInfo`, vtable `0x00A99E70`), slots used by the rebuild (TG24, re-answers21)

A path info holds the occupant's paths in a hash map from a u32 **path key** to a path record (SGI `hash_map`,
identity hash). Key layout:

| Bits | Field |
|---|---|
| 0–7 | exit side (0–3; `0xFF` names the turn-around path of an entry) |
| 8–15 | entry side (0–3) |
| 16–23 | lane index (0, 1, …; distinct paths with the same type, entry and exit) |
| 24–27 | path type (1 = car, `pt(n)` of §2.1) |
| 28–31 | 0 for the paths these queries return |

A **path record** (what slots 3 and 4 point at): `+0` the start and `+4` the end pointer of its point array (12-byte
points, three f32 world coordinates x, y, z), `+0xC` its length (f32). The **first vertex** is the point at `+0`'s
address; the **end point** used by slot 7 is the last point (`end − 12`). The "exit byte" is not a field of the
record: it is bits 0–7 of the key.

| Slot | Offset | Address | Signature as compiled | Pop | Return |
|---|---|---|---|---|---|
| 3 | +0x0C | `0x0053DD30` | `Path* GetPath(u32 key, float* lengthOut)`: hash lookup; null when absent. When found and `lengthOut` is not null, `*lengthOut := ` the record's length. | 8 | EAX pointer into the path info (borrowed; valid while the path info is unchanged) |
| 4 | +0x10 | `0x0053CFF0` | `Path* GetPath(u32 pathType, u8 entry, u8 exit, u8 lane, float* lengthOut)`: slot 3 with key `pathType << 24 \| lane << 16 \| entry << 8 \| exit` (only the low byte of `pathType` survives the shift). | 20 | EAX pointer or null (not a bool: a caller tests all 32 bits) |
| 7 | +0x1C | `0x0053E350` | `vector<u32> GetPathsNearEndPoint(u32 pathType, const float point[3], u8 entry, u8 exit, float radius)`, returned **by value** through a hidden first argument (the caller's uninitialised `vector<u32>`, which the callee constructs and the caller frees). | 24 (hidden pointer included: 6 dwords) | EAX = the hidden pointer |
| 14 | +0x38 | `0x0053E870` | `u32 GetPathsFromEntryEdge(u32 pathType, u8 entry, vector<u32>& keys)`. | 12 | EAX = the number of keys appended |

**Slot 14.** Empties `keys` first (keeps the capacity). Then walks the whole hash map in bucket order (bucket 0
upward, each chain front to back) and appends every key whose bits 28–31 are 0, whose bits 24–27 equal `pathType`
(compared as a full u32, so a `pathType` above 15 matches nothing) and whose entry byte equals `entry`. Returns how
many it appended. `entry = 0xFF` is not a wildcard here.

**Slot 7.** Match rule: the key, masked, equals `pathType << 24 | entry << 8 | exit`, where the mask starts as
`0xFF00FFFF` (lane ignored), drops the entry byte (`0xFF0000FF`) when `entry = 0xFF`, and drops the exit byte when
`exit = 0xFF`. The whole type byte is compared, so keys with bits 28–31 set never match. For each matching path, in
bucket order: with `(dx, dy, dz)` = the path's last point minus the given point (each `r24`),
`d := r24(√ r24(r24(r24(dz²) + r24(dy²)) + r24(dx²)))` (the same order as the tunnel distance of §4.8). A path with `d ≤ radius`
(`d` above the radius, or a NaN, is skipped) is kept: if `d` is **below** the smallest distance kept so far (initially
`FLT_MAX`), its key is inserted at the **front** of the result and the smallest distance becomes `d`; otherwise
(`d` equal to or above it) the key is appended at the back. So the first key is the nearest path; the rest keep no
distance order. The radius test and the "nearer" test are the x87 compares at `0x0053E41E` and `0x0053E431`.

Both vectors use the STLport allocator B (`0x0090CF63` frees them). Result containers are the caller's: slot 14
fills a vector the caller owns; slot 7 constructs one in the caller's storage.

---

## 8. Dependencies

| Interface (vtable) | Slot / offset | Method | Pop | Return | Used in |
|---|---|---|---|---|---|
| `cISC4City` (`0x00A87620`) | 93 +0x174, 94 +0x178 | CellCountX / Z | 0 | EAX | Init, water |
| | 95 +0x17C | `PositionToCell(x, z, &cx, &cz)` (floats by value) | 16 | — | §3, §4.8, §4.9 |
| | 91 +0x16C | `CellWidthX` | 0 | ST0 | tuning (traffic-simulator.md) |
| | 83 +0x14C | GetSurfaceWater (cached in `0x00B43CF0`) | 0 | EAX | water |
| `cISC4OccupantManager` (`0x00A8F7F0`, global `0x00B43D0C`) | 41 +0xA4 | GetOccupantsByStandardCityCells, typed | 20 | AL | §4.2, §4.4, §4.7.1, §4.9.1 |
| `cISC4LotManager` (`0x00AB37C0`, `0x00B43D08`) | 6 +0x18 | `GetLot(x, z, bool anchorOnly)` | 12 | EAX | §4.7.2, §4.9.5, §4.12 |
| | 15 +0x3C | GetSafeLotIterator | 0 | EAX | §4.14 |
| `cISC4Lot` (`0x00AB19B0`) | 3 +0x0C | property holder | 0 | EAX | §4.12, §4.13 |
| | 14 +0x38 | GetBoundingRect | 4 | — | §4.7, §4.9, §4.12 |
| | 21 +0x54 | GetLocation | 8 | — | §4.9.1 |
| | 23 +0x5C | GetFacing | 0 | EAX | §4.7.2 |
| | 48 +0xC0 | GetBuilding | 0 | EAX | §4.7.1, §4.9.1 |
| | 54 +0xD8 | GetPopulations (vector of maps) | 0 | EAX pointer | §4.12 |
| `cISC4NetworkOccupant` (`0x00AA8080`) | 9 +0x24, 8 +0x20 | HasAnyNetworkFlag / HasNetworkFlag | 4 | AL | §3, §4.2, §4.4 |
| | 22 +0x58 | IsOfType | 4 | AL | §3, §4.2, §4.8 |
| | 49 +0xC4 | GetPathInfo | 0 | EAX | §4.2, §4.4 |
| | 53 +0xD4 | PathConnection | 12 | AL | §4.2 |
| path info (`cSC4PathInfo`, vtable `0x00A99E70`; signatures §7.3) | 3 +0x0C, 4 +0x10, 7 +0x1C, 14 +0x38 | get path, turn-around test, paths near a point, paths by entry | 8, 20, 24, 12 | EAX (pointer, pointer, hidden vector pointer, count) | §4.2, §4.4 |
| `cISC4PropOccupant` (`0x00AA4900`) | 7 +0x1C | GetPropType | 0 | EAX | §4.9.1 |
| `cISC4Occupant` (`0x00AA41D8`) | 8 +0x20 | GetPosition | 4 | — | §3, §4.8, §4.9.1 |
| `cISCPropertyHolder` (`0x00AA6CD8`) | 17 +0x44 | RemoveProperty | 8 | AL | §4.12 |
| `cISC4NeighborsSimulator` (`0x00AB1EE0`, `0x00B43D8C`) | 16 +0x40, 17 +0x44 | GetAllConnections, GetConnectionsOfType(9) | 4 / 8 | AL | §4.9, §4.12 |
| `cISC4NeighborConnection` (`0x00AB2034`) | 6 +0x18 | cell (x, z) | 8 | — | §4.9, §4.12 |
| `cISTETerrain` (`0x00AB3D18`, `0x00B43CF4`) | 34 +0x88 | cell under water | 8 | AL | §4.9.1, §4.10.1 |
| **`cISTESurfaceWater`** (`0x00AB3B00`, 15 slots, **new**) | 6 +0x18 | `GetWaterDepthForCell(x, z)` | 8 | ST0 | §4.10.1 |
| | 8 +0x20 | `IsNavigable(cellX, cellZ, radius, lot)` | 16 | AL | §4.10.1 |
| | 9 +0x24 | GetBodyOfWaterMap → `cISC4SimGrid<uint16>` | 0 | EAX | §4.10.2 |
| `cISC4SimGrid<T>` | 24 +0x60 GetGridData; 7 +0x1C get value | | 0 / 8 | EAX | §4.6, §4.7, §4.10 |
| `cIGZMessageServer2` (`0x00B43CCC`) | 5 / 6 | Add / Remove notification | 8 | AL | water pathfinder |

Non-virtual helpers that also serve pathfinder.md:

- `GetNetworkInfo` `0x0070FB30`;
- `TransitSwitchEntryCost` `0x0071CB20`;
- `GetNeighborConnectionInnerCell` `0x0070DDD0`;
- `GetNeighborPoolPropertyHolder` `0x007114E0`;
- `GetTrafficNeighborConnection` `0x0070F970`;
- `SetConnection` `0x0070EAB0`;
- `HasMapExit` `0x0070FA40`.

---

## 9. Replaceability

- **Not separable.** The model is internal to `cSC4TrafficSimulator`. A replacement is a replacement traffic
  simulator (class `0x6990C1AA` in the GZCOM registry). The pathfinder reads the following directly (pathfinder.md
  §8, §9):
  - the cell connection array, network info lists, tunnel and ferry tables, switch table and destination map;
  - non-virtual functions of the class.

  So the two classes are replaced together.
- **Raw grid reads** (simgrid.md §6.4):
  - The switch ID grid is read raw (`GetGridData`) by the pathfinder and by §4.6, §4.11 and §4.7.5.
  - The mode-mask grid is written raw by §4.7.1 and §4.7.5.
  - The water depth map is internal, and the original creates it without GZCOM. A replacement may keep it private
    *[choice]*.

  Under the grid-ownership rule, a replacement creates the switch grids as the game's grid classes. They are saved.
- **What a replacement must read from the game** for this model (occupant queries, path connections and path info,
  tunnel pairs): traffic-simulator.md §9.1 (TG2).
- **Memory reached into by this class**:
  - the paired portal of `cSC4NetworkTunnelOccupant` (the counted pointer at `+0x1D0` of the concrete class, read
    by a non-virtual getter, §4.8, TG25), so a replacement tunnel occupant breaks the original simulator, and a
    replacement simulator must read that member (or the network tool's pairing) to build tunnel records;
  - the lot manager's safe-iterator object, returned by slot 22 and used by the lot manager.
- **Containers crossing the interface**: slots 28–31, 33 and 39–42, listed in the final report's allocator rows.
  `GetFerryRouteBetweenTiles` returns a pointer into internal storage (section (3)).
- **Internal only**: occupant lists in the rebuild (A), neighbour connection lists (A), path-info vectors (A/B),
  the Fill stack (B), lot ID vectors (B), hash sets in slots 40/41 (B), water node pages (C) and the cell array (C).
- **Plugins**: NAM hooks SetConnection (`0x0070EAE7`) and the tables `0x00AB3174` and `0x00B09628` (re/prior_art.md
  §3.2). FreeCity4 exposes no such hooks.

---

## 10. Quirks and open questions

Quirks reproduced: the network-info append clears the old tail's flags; the last flagged occupant decides the lane
continuity; the tunnel mask is always `0x1FE` and the tunnel cost is +∞ for types without speed; ferry type 2 reads
the travel mask `0xD70A`; ferry removal does not rebuild the graph; subnetwork seeds skip the last row and column;
neighbour capacity is counted once per labelled entry side; the intersection search compares z with X−1; switch
entry cost inserts unknown IDs; the switch rules need a lot under the building's NW cell.

Open questions:

- **Q1** Wormhole bit 1 (subway station) is tested by the pathfinder (pathfinder.md §4.8.3), but no code in the
  executable sets it. A scan found every write to that byte: the reset, the tunnel bit and the ferry bit. The
  subway-station branch is therefore dead in 1.1.641 *[inferred]*. Subway access works through transit switches
  instead. FreeCity4 never sets the bit *[compat]*.
- **Q2** *Answered*: ferry terminals are saved and re-read (§6), so they do not depend on the lot-activated message
  being re-sent at load.
- **Q3** Network flag meanings: `0x40000` (record flag bit 0), `0x200000` (excluded from the graph), `0x1F4F` (lane
  continuity). *Partly answered (re-answers11, ZQ15)*: `0x200000` is the connection-node flag set by the network
  manager's `CreateConnectionNode` (`0x00611486`); `0x400000` marks a pre-built-model piece that the network tool
  did not place (set by the `cSC4NetworkOccupantWithPreBuiltModel` constructor at `0x0061E2BC`, cleared by the
  tool's `InsertNetworkOccupant` at `0x006279F2`). See zones.md §8.4.1.
- **Q4** The off-map reads by pathfinder switch moves (pathfinder.md Q4) index the cell array and the switch grid
  outside the city. The cell array is 256 × 256, so x = −1 wraps to row 255 *[inferred]*. FreeCity4 treats such
  reads as empty.
- **Q5** The consumers of `0x2C06BA82` are not identified.

---

## 11. Conformance tests

Unless stated otherwise:

- the city is X = Z = 4;
- tuning as shipped, so `invCapTable[Street] = 0x3C23D70A` and N = 3, values `0x3FB6DB6E`, `0x3FA00000`,
  `0x3F8E38E4`;
- stub network occupants answer `PathConnection` only for path type 1 (car), as listed;
- the path-info turn-around test returns 0 unless listed;
- occupants have no network flags and no lane-continuity neighbours.

1. **Three-cell street (hand-checkable).**

   Setup: Street occupants (IsOfType(3) only) at:
   - A = (1, 1): no car connections; turn-around at entry 2;
   - B = (2, 1): (0 → 2) and (2 → 0);
   - C = (3, 1): turn-around at entry 0.

   Insert them in the order A, B, C, then run DoPostCityInit. After the rebuild:

   | Cell | `conn[1]` = `conn[2]` = `conn[4]` | `connCount` | type | `invCap` | `extraCost` |
   |---|---|---|---|---|---|
   | A | `0x0400` | 1 | 3 | `0x3C23D70A` | 0.0 |
   | B | `0x0104` | 2 | 3 | `0x3C23D70A` | 0.0 |
   | C | `0x0001` | 1 | 3 | `0x3C23D70A` | 0.0 |

   - Every other `conn` word is 0, and `switch[*]` is `0xFFFF`.
   - `priorCar[0…3]` is `0xFFFF` in all cells.
   - There are no records for networks other than 1.
   - The intersection pass leaves `invCap` unchanged (no `connCount > 2`).
   - `GetCapacity(1, 2, 1)` = 100.0 (`0x42C80000`). `GetConnectionCount(1, 2, 1)` = 2. `GetCapacity(0, 2, 1)` =
     0.0.

2. **Subnetworks of test 1.**
   - A.subnet = [0, 0, 1, 0], B.subnet = [1, 0, 1, 0], C.subnet = [1, 0, 0, 0]; counter = 2.
   - The working and published vectors have size 2.
   - A seed at A with entry 0 labels nothing and does not advance the counter.
   - C is never a seed, because x = 3 = X−1.

3. **Lot lists and queries on test 2.**
   - Lot L1 at (1, 2), 1 × 1: its ring (corners excluded), in boundary order (§4.12), is (1, 1), (0, 2), (2, 2),
     (1, 3). Its list is [1]. (Corrected: an earlier version listed (1, 3) second, TN7.)
   - Lot L2 at (2, 0): its ring contains (2, 1). Its list is [1].
   - Stub `GetMaxTripCapacity` gives (5, 0, 0, 0) for L1 and (0, 7, 0, 0) for L2. Populations: L1 {type 0x1010: 3},
     L2 none. No neighbour connections.

   Results:
   - `AreLotsConnected(L1, L2)` = true.
   - `GetConnectedDestinationCount(L1, 1)` = 7 and `(L1, 0)` = 5; `(L1, 4)` = 0.
   - `GetConnectedOccupantCount(L2, 0x1010)` = 3.
   - `GetOccupantCountForAllSubnetworks(0x1010)` → [0, 3].
   - `GetSubnetworksInRectangle((0,0,3,3))` → [1].
   - A lot at (0, 3) (ring (0, 2), (1, 3), plus the clamped cells) has no list. `AreLotsConnected` with it is false,
     and property `0x2A4F9C74` is removed from it.

4. **Intersection reduction.**
   - A street cross centred at (2, 2), with every arm piece straight and the centre having
     `connCount = 12 > 2`:
     - centre `invCap = 0x3C6A0EA1`, so GetCapacity = 70.0;
     - the arm cells next to it: `0x3C4CCCCC`, GetCapacity 80.00001;
     - two cells away: `0x3C360B61`, GetCapacity 90.0.
   - Roads (1000): `0x3ABB3EE8` / `0x3AA3D70B` / `0x3A91A2B5`.
   - A Highway record is never reduced.
   - The same cross with the arm cell next to the centre on one side marked as a tunnel (wormhole bit 2): that
     cell keeps `0x3C23D70A` (100), because a tunnel start cell is not extended (TN21). So does the cell two away
     on that arm, whose search stops at the tunnel cell (TN5). The centre is still `0x3C6A0EA1`.

5. **Capacity table.** With shipped tuning, `invCapTable` = the §4.5 table. `GetCapacity` of an unreduced record of
   type Road = 999.99994 (`0x4479FFFF`), Highway = 3999.9998 (`0x4579FFFF`), OneWay = 1999.9999 (`0x44F9FFFF`).
   Types with capacity 0 (pipe, pole, dirt) → `invCap` 1.0 and capacity 1.0.

6. **Turn-around via SetConnection.** At cell (0, 0), a turn-around at entry 2 for car sets `conn[1] |= 0x0400`
   and the east neighbour's `priorCar[0] |= 0x4444`. A turn-around at entry 0 at (0, 0) sets only `conn[1] |= 1`,
   because the west neighbour is off-map.

7. **Tunnel cost.** A Road tunnel (IsOfType 0, not 2 or 6) with portals at world positions (8, 0, 24) and (72, 0, 24),
   with shipped speeds (traffic-simulator.md §4.14: Car on Road 31, Walk on Road 3.5, Rail 0 for both) and
   CellWidthX 16:
   - both records have mask `0x1FE`;
   - `cost[Car] = 0x3DFDB011` (0.123870976);
   - `cost[Walk] = 0x3F8C6F2E` (1.0971429): stored, but unused because of the mask;
   - `cost[PassengerTrain] = +∞`.

8. **Water route.**
   - Water at depth 3.3 → byte 17, depth 40.7 → 200, depth 0.1 → 1, depth 2.1 → 11.
   - City X = Z = 8 (overriding the 4 × 4 default). Only row z = 0 is water, all bytes 200, one water body. From
     (0, 0) to (5, 0): cells = 6, each step costs 1.0 so the target
     g = 5.0, link cost = `r24(6 × 0.01)` = `0x3D75C28F` (0.06).
   - Forward route bytes: `00 00 00 40 05 AA 02`.
   - Backward route bytes: `00 05 00 40 05 00 00`.
   - Different water-body IDs at start and target → failure, and the vectors are untouched.

9. **Ferry cost encoding.** Path lengths 1, 5, 37, 100, 250 give costs `0x3C23D70A`, `0x3D4CCCCC`, `0x3EBD70A4`,
   `0x3F800000`, `0x40200000`.

10. **Switch entry cost.**
    - load 0, capacity 100: q = 0 → `Curve(0) × multiplier × timeFactor`.
    - load 600, capacity 100: q = 6 → `0x7EFFFFFF`.
    - load 0, capacity 0: NaN → `0x7EFFFFFF`.
    - Unknown ID: inserts a default switch (capacity 0) → `0x7EFFFFFF` *[choice]*. The original's inserted switch
      holds stack garbage (§4.7.3 step 1), so this case is not differential.

11. **Rule rotation.** Facing 1, rule u32 `0x01_01_80_00`:
    - b0 = 0x00 → kind 0;
    - b1 = 0x80 → i = 0 → side `ROT[1][0]` = 1 (north);
    - from = 1, to = 1.

    The "to" mask gets `0x0002`, and the mode mask gets bit 1.

12. **Seeded RNG (no draws).** Seed a cRZRandomSuper with 12345. Run tests 1–3, 8 and a DoConnectionsChanged over the
    whole city. Then the generator's next `RandomUint32Uniform(100)` equals that of a fresh generator seeded with
    12345: the network model makes no draws.

    For comparison, DoLotActivated on a switch lot makes exactly one draw on the simulator's own generator (§12),
    after the rebuild.

13. **Path keys and the turn-around test** (§7.3, §4.2). A path info holding car paths with keys `0x01000200`,
    `0x01010200`, `0x01000203` and `0x010002FF`: `GetPathsFromEntryEdge(1, 2, v)` returns 4 and fills `v` with those
    four keys (in the map's bucket order); with `v` holding 7 old entries beforehand it still returns 4 and `v` has 4
    entries. `GetPathsFromEntryEdge(1, 0xFF, v)` returns 0. `GetPath(1, 2, 0xFF, 0, null)` looks up key `0x010002FF`
    and returns non-null (a turn-around for entry 2 when PathConnection finds no exit there); `GetPath(1, 2, 0xFF, 1,
    null)` (key `0x010102FF`) returns null. In the lane continuity of connection (2 → 0) only `0x01000200` and
    `0x01010200` are kept (exit byte 0); `0x010002FF` is listed by slot 14 but never matches an exit 0–3.
14. **Lane continuity** (§4.4). The worked example of §4.4: `priorCar[2]` at (10, 10) goes from `0xFFFF` to
    `0xFFF1`. Moving the second path's last point to exactly 4.0 units away puts both keys in `near` (the radius
    test is `≤`) and gives `0xFFF3`. Clearing the neighbour's network flags (none of `0x1F4F`) leaves `0xFFFF`.
    Removing the occupant's own path `0x01000200` (so V is empty) leaves `0xFFFF` whatever the neighbour holds.
15. **GetPathsNearEndPoint order** (§7.3). Paths visited in bucket order with end distances 3.0, 1.0, 3.5, 0.5 and
    radius 4.0: result = [0.5-path, 1.0-path, 3.0-path, 3.5-path]; with distances 1.0, 3.0, 2.0: [1.0, 3.0, 2.0]
    (2.0 is not below the smallest kept distance, so it is appended).
16. **Tunnel pair** (§4.8, TG25). Two portals paired by the network tool: each holds one reference on the other
    (`+0x1D0`). Inserting portal A makes a record keyed by A's cell pointing at B's cell, and A's and B's reference
    counts are the same before and after the insert. A portal whose member is null (never paired, or after its
    `Shutdown`) makes no record. A save and reload restores both members.

Test 8's route bytes:

- Forward: header `[0, 0, 0]`, then run `[0x40, 5]`, then codes 2, 2, 2, 2, 2 packed as `0xAA, 0x02`.
- Backward: header `[0, 5, 0]` (start = target cell), then five code-0 steps, packed as `0x00, 0x00`.

---

## 12. Random draws

None. The rebuild, the subnetworks, the queries, the water pathfinder and the ferry routing make no random draws and
call no `rand()`.

The only draw near this code is in DoLotActivated: after the switch, ferry and rebuild steps,
`RandomUint32Uniform(100)` on the simulator's `cRZRandomSuper` is stored in lot property `0x4A611773`. That
generator is seeded from the wall clock at Init (foundations §6.2/§6.3), and the draw belongs to traffic-simulator.md.
A lot whose `0xE90E25A1` has a byte size that is not a multiple of 4 returns before the draw (§4.7.1 step 1), so it
makes none.

Hash iteration orders are deterministic: SGI hash containers with identity hashes, bucket hint 100 → 193 buckets.

---

## 13. Corrections to earlier write-ups

- `docs/subsystems/traffic.md`: the connection word's nibble is the **entry** side and its bit the exit side. The
  car word is indexed by entry side with bits `p + 4x`. GetCapacity returns a float of `1/invCap`, not a raw
  capacity.
- The SDK (§7): slots 22, 23 and 29 have wrong return types, and cISC4NetworkOccupant slots 53/54 are swapped.
