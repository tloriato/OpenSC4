# Spec: pipe and subway occupants (`cSC4PipeOccupant`, `cSC4PipeConnectionOccupant`, `cSC4SubwayOccupant`, `cSC4SubwayConnectionOccupant`)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`). Implementers must work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled
or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[compat-limit]* depends on something FreeCity4 cannot reproduce, so only the rule is matched.

Related specs: [plumbing.md](plumbing.md) (§6.4, the only caller of `SetWateredState`; §12.2),
[network-manager.md](network-manager.md) (§7.6, network types 4 and 7, the corner test that reads `GetAltitude`, the
underground refit), [traffic-network.md](traffic-network.md) (§7.2, network occupant slot 49 `GetPathInfo`),
[network-rules.md](network-rules.md) (the piece id `+0xFC` and orientation `+0x104`), [save-records.md](save-records.md)
(§3.2 and §8, the class IDs and reference counts), [INVENTORY-M6.md](INVENTORY-M6.md) (§7).

## 0. Summary

The four classes are thin layers over the underground network occupant (`cSC4UndergroundOccupant`) and the
underground connection occupant (`cSC4UndergroundConnectionOccupant`), whose behaviour is **not** specified here
(UG1). Each layer supplies its identity (class ID, network type, model instance offset, a static model descriptor),
its save-record version, and a few behaviours: a pipe remembers whether it is watered and is drawn tinted red when it
is not; a subway puts its paths and its altitude on the floor of its tunnel. The connection occupants only identify
themselves and accept their own network's occupants.

## 1. Identity *[compat]*

| | Pipe | Subway | Pipe connection | Subway connection |
|---|---|---|---|---|
| Class ID (`GetGZCLSID`) | `0x49C05B9F` (`0x00648F40`, shared body) | `0xCA16374F` (`0x00653D90`, shared body) | `0xEA0A5ACF` (`0x00648040`) | `0xCA15ED6B` (`0x00652A70`) |
| IIDs answered before the base (`QueryInterface`) | `0x49C05B9F`, `0xA9764C8C` (`0x006484D0`) | `0xCA16374F` (`0x00652D00`) | `0xEA0A5ACF` (`0x00648080`) | `0xCA15ED6B` (`0x00652AA0`) |
| Base class (others go to its `QueryInterface`) | underground occupant (`0x0065A080`) | same | underground connection (`0x00654030`) | same |
| Network type (main slot 102, connection slot 39) | 4 | 7 | 4 | 7 |
| Main vtable | `0x00AA97B8` (106 slots) | `0x00AAA3C8` (107 slots) | `0x00AA9578` (45 slots) | `0x00AAA188` (45 slots) |
| Object size; allocation | `0x280` bytes from a dedicated pool (102 per 64 KiB page) | `0x2D8` bytes, pool (90 per page) | `0x250` bytes, global heap | `0x250` bytes, global heap |
| Factory (registered by `cSC4COMDirector`'s constructor, `0x00460090`) | `0x006489B0` | `0x00653560` | `0x00648440` | `0x00652CB0` |
| Constructor (flag 1 = most-derived) | `0x006488F0` | `0x006530B0` | `0x006483C0` | `0x00652C30` |
| Model instance offset (main slot 105) | `0x06000000` (`0x006484C0`) | `0x07000000` (`0x00652CE0`) | — | — |
| Static model descriptor (main slot 101; also passed to the base `Init`) | `0x00B0BED0` (`0x006484B0`) | `0x00B0CDC8` (`0x00652CD0`) | `0x00B0B6D0` | `0x00B0C388` |
| Save-record version | 3 | 2 | (base record) | (base record) |

The underground occupant base also answers `0xA9764C8C` (and `0x4A10AFC2`, `0xA821EF94`), so a subway answers
`0xA9764C8C` too; the pipe merely tests it first. The pipe and subway factories take a block from their pool,
construct it and return its `+4` sub-object (the occupant interface); the connection factories return the object
itself, or null when the heap allocation fails. The pools are an
implementation detail *[choice]*: FreeCity4 may allocate these objects in any way.

The shared bodies (class-ID getters reached from two vtables, the network-type getters `0x00580F60` (4) and
`0x00505070` (7) shared with other classes, and the `Shutdown` forwarders `0x00652CF0` and `0x00648070`) keep their
folded names.

## 2. Pipe occupant *[compat]*

### 2.1 State

`watered` at `+0x270` (u32; 0 dry, 1 watered). The constructor and `Init` set it to 0.

### 2.2 Overridden behaviour

| Where | Address | Behaviour |
|---|---|---|
| main slot 3 `Init(arg)` | `0x00648BF0` | run the static set-up (§2.5) once; unless the occupant is already initialised (occupant slot 5, `cSC4Occupant::IsInitialized`), call the base `Init(arg, 0x49C05B9F, 0x00B0BED0)` and set `watered := 0`; true |
| occupant slot 3 `Init()` | `0x00648BC0` | static set-up once; unless initialised, the base's other `Init` (`0x0065C070`) and then the base's static set-up (`0x0065AE80`); true |
| main slot 103 `GetTint(out)` | `0x006488A0` | `out :=` the RGB float triple at `0x00B0BE88 + 12 × watered`: dry (0.8, 0.23, 0.23), watered (1, 1, 1). The base's version (`0x0065B020`) gives (1, 1, 1); the base's `Draw` uses it |
| `SetWateredState(bool w)` (non-virtual) | `0x00648540` | if `(watered = 1)` equals `w`, nothing; else `watered := w ? 1 : 0` and main slot 100 (`0x0065AF10`, a base refresh) is called with 0 |
| `GetSortValue(out)` (the drawable virtual base's slot 17) | `0x00648500` | if network flags `+0x13C` have any of the bits `0x06000000` (UG2), `out :=` the piece id `+0xFC`; otherwise `out := 0x49C05B9F` |

### 2.3 Save record (serialisable sub-object `+8`; `Write` `0x00648800`, `Read` `0x00648840`)

1. u16 version **3**.
2. The underground occupant's record (`0x0065B140` / `0x0065E510`; it starts with its own u16 3 and the network
   occupant's record, UG1).
3. u32 `watered`.

`Write` writes all three (the third only when the base succeeded) and returns true unless the stream reports an
error (its slot 18). `Read` reads the version and returns false at once when it is not 3; otherwise it reads the base
record and, if that succeeded, the u32 into `watered` and sets the base flag at `+0x260` to 1; it returns true unless
the stream reports an error. The value is
stored as read: anything but 0 or 1 makes slot 103 read past the two-entry tint table *[quirk]*; *[choice]* FreeCity4
treats any non-zero value as watered when drawing, and writes back what it read.

### 2.4 Users of `watered`

Only the plumbing simulator's pass (plumbing.md §6.4) calls `SetWateredState`, with the watered grid's bit for the
pipe's cell. The value only changes the tint and is saved.

### 2.5 Static set-up (`0x00648A90`, once; flag `0x00B46730`)

From the pipe-corner geometry table (24-byte points at `0x00B0BA68`, x at +0 and z at +8) and the index lists
`a = (1, 3, 5, 7, 9)` (`0x00B0BC60`) and `b = (0, 2, 4, 6, 8)` (`0x00B0BC88`), compute five lengths
`L_k := sqrt((x[a_k] − x[b_k])² + (z[a_k] − z[b_k])²)` (float). `L_0` is not used. Then store, as floats:

- `0x00B46708`…: `L4/(L1+L4)`, `L3/(L2+L3)`, `L2/(L2+L3)`, `L1/(L1+L4)`, 1, 1;
- `0x00B46720`…: `L1/(L1+L4)`, `L2/(L2+L3)`, `L3/(L2+L3)`, `L4/(L1+L4)`.

With the shipped table: `L = (11.31371, 7.33553, 6.35335, 4.96035, 3.97818)`, so the ratios are 0.648375,
0.5615625, 0.4384375 and 0.351625. The descriptor (§1) points at these tables; how the base uses them is UG1
(*[inferred]*: the texture spans of the diagonal pieces). Finally, unless the base's static set-up already ran
(`0x00B45250`), it runs it and returns its result; otherwise true. A second call returns false and does nothing.

## 3. Subway occupant *[compat]*

### 3.1 State

`floor[4]` at `+0x270`: four cached corner vertices (24 bytes each: x, y, z and three more floats).

### 3.2 Overridden behaviour

| Where | Address | Behaviour |
|---|---|---|
| main slot 3 `Init(arg)` / occupant slot 3 `Init()` | `0x006537A0` / `0x00653770` | as the pipe's (§2.2) with class `0xCA16374F`, descriptor `0x00B0CDC8` and the subway's static set-up; no `watered` field |
| main slot 49 `GetPathInfo()` | `0x006537E0` | §3.4 |
| main slot 56 `GetAltitude(x, z)` | `0x00652D70` | §3.3 |
| main slot 106 `UpdateFloor()` (new) | `0x00653130` | §3.3 |
| `GetSortValue(out)` | `0x00652D30` | as the pipe's with class `0xCA16374F` |

There is no tint override: subways are drawn with the base's (1, 1, 1).

### 3.3 Tunnel floor and altitude

`UpdateFloor` fills `floor[0…3]`. By default it copies the first four vertices of the model's vertex list (`+0x1B4`),
in order. For the piece ids (`+0xFC`) `0x300` and `0x4B00` it uses the second vertex list (`+0x1E4`) instead when that
list has at least 8 vertices (else the default): with `r := (4 − rotation) mod 4` (rotation = main slot 24), its
vertices 4, 5, 7 and 6 go to `floor[r]`, `floor[(r + 1) mod 4]`, `floor[(r + 2) mod 4]` and `floor[(r + 3) mod 4]`.
It also asks the base for the zipper piece type (main slot 99); when the answer is 0, each of those four corners is
then lowered by twice the height of its vertex above vertex 0, 1, 11 and 10 of the same list respectively
(`y := y − 2 × (y_4 − y_0)`, and so on) (UG3).

`GetAltitude(x, z)` (two floats, pops 8, result on the x87 stack): with `u := (x − floor[0].x) / 16` and
`w := (z − floor[0].z) / 16`, and `y_i := floor[i].y`:

- if `u ≤ w`: `h := y0 + (y1 − y0) × w + (y2 − y1) × u`;
- otherwise, and when the comparison is unordered (a NaN): `h := y0 + (y3 − y0) × u + (y2 − y3) × w`;

and the result is `h − 0.5`. The division is a multiplication by 0.0625 (`0x00AAA250`), done term by term, at the
caller's x87 precision. So corner 0 is at `(x0, z0)`, corner 1 at `(x0, z0 + 16)`, corner 2 at `(x0 + 16, z0 + 16)` and
corner 3 at `(x0 + 16, z0)`, and the cell is split along its `u = w` diagonal *[inferred]* (the corner order follows
from the formula).

### 3.4 Path info (slot 49)

As the network occupant's (traffic-network.md §7.2: built on the first call, cached at `+0x140`, returned without
AddRef), with these differences:

1. Before the build: unless the flag `+0x262` is set, main slot 86 with 0; then `UpdateFloor` (slot 106).
2. The path info resource key is `{type 0x296678F7, group 0xA966883F, instance}` with
   `instance := piece id | 0x06000000`, or the explicit instance at `+0x164` when it is not 0 (slot 51 sets it),
   fetched from the resource manager (`0x00B45254`, slot 3, interface `0xA9667793`) and copied for this occupant.
3. Every point of every path gets `y := GetAltitude(x, z)` (slot 56, so the tunnel floor less 0.5), and each path's
   length (`+0x14`) is recomputed as the sum of the 3D distances between consecutive points.
4. Then, as the base: register with the path info (its slot 40), clear `+0x144`…`+0x163`, its slot 27.

### 3.5 Save record (`Write` `0x00653010`, `Read` `0x00653040`)

u16 version **2**, then the underground occupant's record; nothing else. `Read` returns false at once for another
version; after a successful base read it sets `+0x260` to 1. Both return true unless the stream reports an error.

### 3.6 Static set-up (`0x00653640`, once; flag `0x00B46820`)

The pipe's algorithm (§2.5) on the subway's tables: points `0x00B0C960`, `a` at `0x00B0CB58`, `b` at `0x00B0CB80`
(the same index lists), results at `0x00B467F8`… and `0x00B46810`…. Shipped: `L = (11.31371, 9.85353, 7.39846,
3.91525, 1.46018)`; ratios 0.8709375, 0.6539375, 0.3460625, 0.1290625.

## 4. Connection occupants *[compat]*

| Where | Pipe connection | Subway connection | Behaviour |
|---|---|---|---|
| main slot 3 `Init()` | `0x00648050` | `0x00652A80` | `+0x24 :=` `0x2A183167` (pipe) or `0x8A13918A` (subway) (UG4), then the base `Init(classID, descriptor)` (`0x00654710`, shared body) |
| main slot 37 | `0x00648040` | `0x00652A70` | the connection's class ID |
| main slot 38 | `0x00648F40` | `0x00653D90` | the class ID of the matching network occupant (`0x49C05B9F` / `0xCA16374F`) *[inferred]* (a body shared with that occupant's `GetGZCLSID`) |
| main slot 39 | `0x00580F60` | `0x00505070` | the network type (4 / 7) |
| main slot 44 `QueryOccupantInterface(occ, out)` | `0x00648360` | `0x00652BD0` | if `occ` answers its matching network occupant's class ID in `QueryInterface`, return `occ.QueryInterface(0x4A10AFC2, out)` (and release the first reference); otherwise false. Pops 8, AL |
| `GetSortValue(out)` | `0x006480B0` | `0x00652AD0` | `out :=` the connection's class ID, always |
| deleting destructor | `0x006482F0` | `0x00652B60` | the base destructor, then `GlobalHeapFree` on flag bit 0 |

They have no record of their own: the base's is written (save-records.md §3.2 counts four pipe-connection saves and
one subway-connection save in the corpus).

## 5. Vtables and thunks

Each occupant has a main vtable, three sub-object vtables at `+4` (the occupant interface), `+8` (serialisation) and
`+0xC`, two small ones at `+0x20` and `+0x38`, and a virtual base (the drawable object) at `+0x27C` (pipe) or `+0x2D4`
(subway) reached through the table at `+0x3C`:

| | `+4` | `+8` | `+0xC` | `+0x20` | `+0x38` | virtual base |
|---|---|---|---|---|---|---|
| Pipe | `0x00AA9730` | `0x00AA9714` | `0x00AA96B0` | `0x00AA9698` | `0x00AA9694` | `0x00AA9648` |
| Subway | `0x00AAA340` | `0x00AAA324` | `0x00AAA2C0` | `0x00AAA2A8` | `0x00AAA2A4` | `0x00AAA258` |

The connections have `+4` (`0x00AA955C` / `0x00AAA16C`), `+8` (`0x00AA94F8` / `0x00AAA108`), `+0x1C`, `+0x34` and
`+0x38` (pipe connection `0x00AA94E0`, `0x00AA94D0`, `0x00AA94CC`), and a virtual base at `+0x24C` (pipe connection
`0x00AA9480`, subway connection `0x00AAA090`). Their slots reach the class's methods through this-adjustor thunks (`this − 4`, `− 8`, `− 0xC`, `− 0x20`,
`− 0x38`, and for the connections `− 0x1C`, `− 0x34`, `− 0x38`) and vtordisp thunks (`this − [this − 4] − offset`).
The thunks that only these classes' vtables use and whose target is one of the methods above are named in the symbols
file; the many AddRef / Release thunks that other classes share are not. A sixth vtable at `0x00AA9960` belongs to the
pipe pool object at `0x00B0BEC0`, and the one at `0x00AAA578` to the subway pool at `0x00B0CDB8`; they are not the
occupants' vtables.

## 6. Users

- `cSC4COMDirector` (`0x00460090`) registers the four factories with `AddCls`.
- `cSC4ViewInputControlPlaceLot::PlaceLot` constructs pipe and subway connections directly (`0x004C1C9E`,
  `0x004C1D82`).
- The plumbing simulator: `SetWateredState` (plumbing.md §6.4) and its pipe references (save-records.md §4: 1,483
  references to class `0x49C05B9F` in the corpus).
- The network manager: the corner test through slot 56 and the underground refit of types 4 and 7
  (network-manager.md §7.6); the pipe tool's static `FindPipeOccupant` / `FindPipeConnectionOccupant` (manager slots
  10, 11).
- The traffic code reads subway paths through slot 49.

## 7. Quirks

1. A loaded `watered` other than 0 or 1 is kept and indexes past the tint table (§2.3).
2. `SetWateredState` compares with `watered = 1`, so a stored 2 is "dry" for the comparison: `SetWateredState(false)`
   leaves it at 2 (§2.2).
3. The subway's altitude is 0.5 below its tunnel floor, and its path points are moved there (§3.3, §3.4).
4. The static set-ups compute a fifth length they never use, and a second call returns false (§2.5).
5. A connection's `GetSortValue` is always its class ID; an occupant's is its piece id only with flags `0x06000000`
   (§2.2, §4).

## 8. Open questions

- **UG1:** the underground occupant and underground connection occupant base classes (`0x0065A080` …,
  `0x00654030` …): their records, `Draw`, model building and the use of the descriptors and ratio tables.
- **UG2:** the meaning of network flag bits `0x02000000` and `0x04000000` (`+0x13C`) that switch the sort value to
  the piece id.
- **UG3:** subway pieces `0x300` and `0x4B00` (the ones whose floor comes from the second vertex list), and what the
  zipper piece type answer selects.
- **UG4:** the IDs `0x2A183167` and `0x8A13918A` written to a connection's `+0x24` by `Init`.

## 9. Conformance tests

1. **Identity:** the four class IDs, network types 4 / 7 / 4 / 7, model offsets `0x06000000` / `0x07000000`; a pipe
   and a subway both answer `QueryInterface(0xA9764C8C)`; a pipe connection answers `0xEA0A5ACF` and not
   `0xCA15ED6B`.
2. **Watered state:** a new pipe is dry; `SetWateredState(true)` makes it watered and refreshes once; a second
   `SetWateredState(true)` does nothing; the tint is (0.8, 0.23, 0.23) dry and (1, 1, 1) watered.
3. **Pipe record:** writing a watered pipe gives u16 3, the base record, u32 1; reading a record with version 2
   fails; reading u32 1 makes the pipe watered.
4. **Subway record:** u16 2 then the base record only; version 3 fails.
5. **Subway altitude:** with `floor[0] = (0, −10, 0)`, `y1 = −10`, `y2 = −12`, `y3 = −11`: `GetAltitude(0, 0) = −10.5`,
   `GetAltitude(16, 16) = −12.5`, `GetAltitude(16, 0) = −11.5`, `GetAltitude(4, 8)` (`u = 0.25 ≤ w = 0.5`)
   `= −10 + 0 × 0.5 + (−2) × 0.25 − 0.5 = −11`, `GetAltitude(8, 4)` (`u > w`) `= −10 + (−1) × 0.5 + (−1) × 0.25 − 0.5
   = −11.25`.
6. **Subway paths:** every point of a built path info has `y = GetAltitude(x, z)`, and each length is the 3D polyline
   length.
7. **Connections:** `QueryOccupantInterface` of a pipe connection is false for a subway occupant and forwards
   `0x4A10AFC2` for a pipe occupant; both connections' `GetSortValue` give their class IDs.
8. **Static set-up:** with the shipped tables the pipe ratios are 0.648375, 0.5615625, 0.4384375, 0.351625 and the
   subway ratios 0.8709375, 0.6539375, 0.3460625, 0.1290625.
