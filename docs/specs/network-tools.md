# Network tools (behaviour spec)

Status: M5 behaviour spec, written 2026-10-15 (re-specs14b lane). Every fact was checked on the Windows executable
`bin/SimCity4.exe` (1.1.641, image base `0x00400000`); every address is a Windows address. Numeric details (float or
double, operation order, widths) were checked against the instruction bytes. Implementers work from this spec, the
public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled or disassembled
code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Tags:

- *[compat]*: observable; a replacement must match it.
- *[choice]*: a FreeCity4 decision.
- *[inferred]*: consistent with all the evidence, but not proven.
- *[quirk]*: original behaviour that looks unintended but is reproduced.

`r24(e)` is `e` rounded to float32 (foundations §18).

Scope. The **network tool** (`cSC4NetworkTool` and its pipe, subway and power-line subclasses) as the simulation and
the harness's scripted actions use it:

- placing road, rail, pipe and power-line pieces by code (`PlaceNetworkOccupants`, `PlaceNetworkOccupantById`);
- the draw / validate / solve / commit pipeline that both code and the player's drag go through (`PlaceNetwork`);
- what is charged to the budget, and by whom;
- auto-connection: run extension, joins to other network types, and regional (neighbour) connections;
- `DestroyOccupantsAndResolve` (removal and rebuilding of neighbours);
- `UpdateUndergroundOccupantTerrainHeights` (pipes and subways after a terrain edit);
- how the network manager's paths call in.

It also gives the prop occupant slots that other specs use (§13). The NM5 answer is in §13.2.

What is not here: the network solver's geometry (`SolveNetwork`, `SmoothenNetwork`, bridges, tunnels, highway
intersections, one-way resolution). It is named by its passes and failure codes only; the geometry is
[network-geometry.md](network-geometry.md). The player's UI (cursor, preview
and sounds) is covered only where it charges the budget (§8).

Related specs (cited, not repeated):

- [network-manager.md](network-manager.md): the 13 tools and `GetNetworkTool` (§3.1, §7.1); network flags (§5);
  `DestroyNetworkOccupants` (§7.2), `RemoveOrReplaceOccupant` (§7.4), the terrain handler (§7.6),
  `CreateConnectionNode` (§7.7), `AutoconnectLotNodes` (§7.8); the harness view of placement (§9); potholes (§8.8).
- [traffic-network.md](traffic-network.md) §3: what the traffic graph does with each inserted or removed occupant.
- [budget.md](budget.md) §6: `WithdrawFunds`.
- [zones.md](zones.md) §5.2 (`data4` of the insert message), §7.10 (`DrawStreet`), §8.8 (`OccupantExists`, the world
  cache's lifetime), §14.3 Q1(a), which this spec answers (§16).
- [occupant-manager.md](occupant-manager.md): `InsertOccupant`, `RemoveOccupant`, the cell and rectangle queries.
- [demolition.md](demolition.md) §3 (`0xAA7A014D`), §9 (the tool's demolition call).
- [prop-developer.md](prop-developer.md) §4.6–§4.13, §8.4: the prop occupant slots it calls (§13 here).
- [foundations.md](foundations.md) §6: random generators.

---

## 1. Identity

| Tool | Network types (network-manager.md §3.1) | Main vtable | Slots | Notes |
|---|---|---|---|---|
| generic `cSC4NetworkTool` | 0–3, 6, 8–12 | `0x00AA8748` | 0–80 | constructor `0x0062C5F0` |
| pipe tool | 4 | `0x00AA99A8` | 0–81 | underground base; slot 81 returns the pipe occupant class `0x49C05B9F` |
| subway tool | 7 | `0x00AAA5C0` | 0–81 | underground base; slot 81 returns the subway occupant class `0xCA16374F` |
| power-line tool `cSC4PowerLineTool` | 5 | `0x00AA9F30` | 0–81 | constructor `0x006503C0`; slot 81 `CheckForReservedTiles` (`0x0064FED0`) |

A fifth vtable, `0x00AAA950`, uses the underground base's slots. It is probably the abstract underground tool itself
*[inferred]*.

The interface is cISC4NetworkTool. Compiled slot = SDK index + 3 (the SDK lists from `Init`). Slot table: §14.

The underground tools (`cSC4UndergroundNetworkTool`, pipe and subway) override:

| Slot | Override |
|---|---|
| 3 | `Init` (`0x00657770`) |
| 55 | `InsertOccupantsForSolvedCells` (`0x006599D0`) |
| 74 | the **placement height** (`0x00648F20`): `h − depth` |
| 81 | the occupant class |

The power-line tool overrides:

| Slot | Override |
|---|---|
| 3 | `Init` (`0x00650630`) |
| 4 | `Shutdown` (`0x0064F0D0`) |
| 10 | `UpdateOnZoneChange` (`0x006522F0`) |
| 16 | `0x0064F3F0` |
| 20 | `GetCostOfSolution` (`0x0064F0F0`) |
| 51 | `0x00650560` |
| 52 | `GetFailureReasonString` (`0x0064F570`) |
| 53 | `DoNetworkSpecificPlacement` (`0x00652980`) |

---

## 2. State used here

Offsets are from the tool object. Only the fields that the paths below read or write are listed.

| Offset | Meaning | Default (constructor `0x0062C5F0`) |
|---|---|---|
| `+0x1C` | the **world cache**: a draft overlay of cell records (zones.md §8.8) | empty |
| `+0x44` | the terrain (`cISTETerrain`) | set in `Init` |
| `+0x50` | byte **real**: 1 while committing to the city, 0 while drafting (preview) | 0 |
| `+0x52` | byte **stop on failure**: when 1, a failed validation ends `PlaceNetwork` with false | 1 |
| `+0x53` | byte, set to 1 by `DestroyOccupantsAndResolve` | — |
| `+0x70`, `+0x74`, `+0x78` | dragged step count, straight step count, **valid** step count | 0 |
| `+0x98`–`+0xA0` | vector of solved cells (12 bytes each) | empty |
| `+0xA4` | hash set of solved cell keys | empty |
| `+0x208` | `data4` for occupant-manager inserts: 1 normally, 3 inside `PlaceNetworkOccupants` (zones.md §5.2) | 1 |
| `+0x20C` | **failure code** (§5.3); 0 = none | 0 |
| `+0x220` | demolition cost of what the drag would clear (computed by `UpdateOccupantHighlights`) | 0 |
| `+0x234` | byte **preview update enabled** (slot 21 `EnablePreviewUpdate`) | 1 |
| `+0x240`, `+0x244` | city size in cells, X and Z | set in `Init` |
| `+0x248` | vertex row stride (X + 1) | set in `Init` |
| `+0x2C4` | float, the vertical tolerance of a single placed piece (§4) | per type |
| `+0x320`, `+0x324`, `+0x328` | bridge cost per span cell, tunnel cost per cell, cost per unit of earthwork (§8.1) | per type |
| `+0x354` | byte: the type has land bridges | 0; `Init` sets it per type |
| `+0x35C` | the tool's network type | constructor argument |
| `+0x360` | the layer mask: `0x1F4F` for the surface types, the type's own bit for pipe (4), power line (5) and subway (7) | `Init` |
| `+0x364` | an OR occupant filter on `+0x360` (built in `Init`, `0x00626A99`–`0x00626AE9`) | `Init` |
| `+0x36C` | underground tools: float **depth** below the terrain (property `0xA96E5641`) | `Init` |
| `+0x370`, `+0x378` | underground tools: the two floats whose sum is the **cover margin** (properties `0x496E564C`, `0x096E5653`) | `Init` |

The underground tools read their four depth properties (`0xA96E5641`, `0x496E564C`, `0x096E5653` and `0x496E5A36`
into `+0x374`) from an exemplar of type `0x6534284A`, group `0x084344E0`, with an instance that depends on the network
type (`ReloadNetworkSpecificInfo`, `0x00657980`).

A tool must be `Init`ed before use and `Shutdown` after. The network manager's shared tools (`GetNetworkTool(type,
false)`) and private ones (`true`) behave the same (network-manager.md §7.1).

**Missing underground properties (NT38, partial).** `ReloadNetworkSpecificInfo` (`00657980`) uses its
single u32 argument **directly as the exemplar instance** in key `(6534284A,084344E0,argument)`; it does not
convert a network type to an instance internally. On resource lookup failure it writes none of the four
destination fields. On success it reads the property holder, then attempts properties in this order:
`A96E5641` → float32 `+36C`, `496E564C` → float32 `+370`, `096E5653` → float32 `+378`, `496E5A36` → byte
`+374` (`006579CC–00657A0F`). The last uses the byte property helper `005FD390`, not the float helper.
All four Boolean read results are ignored. The float helper `005FD4F0` leaves the destination untouched
when the property is absent (`005FD501–005FD520`); this reload supplies **no numeric fallback**, so a
missing property retains the previous field, rather than setting it to zero. The underground constructor
`00657FD0` initializes its flag/lists but does not initialize these four fields. This establishes reload
retention, not a deterministic first-use value for a missing property.

**Partial answer 2026-10-28 (re-blockers37b):** NT38 remains open for the caller's exact per-type instance
selection/initialization before reload, any further initialization of these four fields, and the solved-cell
set's own hash/bucket/rehash/iteration contract with a collision example. Neither zero defaults nor borrowing
another set's constructor parameters is authorized by this evidence. Step 4 of §11 continues to require the
actual solved-set order, not first-input order.

### 2.1 Public result observation (NT27)

After synchronous slot 7 returns, and before another draw, `Reset` or `Shutdown`, slots 23/25/26/27 expose the
**current drag**, its valid steps, straight steps and total steps respectively. Slot 23 takes a caller-owned
vector of pairs of signed 32-bit cell coordinates `(x, z)` and **assigns a copy** of the tool's entire dragged-cell
vector to it, replacing previous contents; it neither lends internal storage nor transfers occupant references.
The Windows vector has begin/end/capacity pointers and 8-byte elements; use the public SDK's compatible container
contract, not a pointer into the tool. The caller owns and destroys the copy; subsequent tool operations do not
change it (`0x0062CA30`–`0x0062CA3F`, assignment `0x00611B00`–`0x00611BCF`). Counts are 32-bit values, not cell-vector
lengths (`0x00407AB0`, `0x00407AC0`, `0x00809240`). Slot 7 clears its temporary anchors and restores insert data4,
but does not reset these results. A failed validation may therefore expose a nonempty full drag with zero valid
steps. **There is no public cISC4NetworkTool getter for solved records, type masks, piece IDs or orientation**:
slot 23 contains coordinates only, and slots 0–50 of `0x00AA8748` provide no solved-vector accessor. The internal
12-byte solved records in §2 are not a public ABI. A city occupant query after successful real placement observes
committed occupants, not the solver's ordered record inventory (which can include re-solved neighbours). A shadow
using only these tool getters can compare drag/status/cost; it cannot claim solved-piece/type/orientation coverage.
Bridge candidate getters likewise do not expose the final solved records. Power-line placement and cost use the
subclass contracts (§9, [power-line-tool.md](power-line-tool.md)); bridges require the actual type/model resources
([network-bridges.md](network-bridges.md)). Missing adapters or literal zero Init inputs are not native defaults
and do not make these paths comparable.

---

## 3. PlaceNetworkOccupants (slot 7, `0x00629210`, pop 32, returns AL)

`PlaceNetworkOccupants(x1, z1, x2, z2, mask, callback, real, type)`. The SDK names `real` "bUseWorldCache". In the
exe it is the commit flag: true builds into the city, false only drafts. This is the call that code paths (the zone
developer's streets, `AutoconnectLotNodes`, the harness) use to lay a network.

### 3.1 Setup

1. If `type < 0`: `type := +0x35C`.
2. `+0x208 := 3` (`0x0062924A`); `+0x20C := 0`.
3. Clamp each coordinate, signed, into `[0, X − 1]` and `[0, Z − 1]`.
4. Clear the solved-cell vector and the step counts `+0x70`, `+0x74`.
5. If `real`: clear the world cache (`0x00647B20`).

### 3.2 mask = 0: one straight run

1. Anchors := [clamped start, clamped end].
2. `ok := DrawNetworkLine(start, end, false, type)` (slot 18, `0x0063AF40`).
3. If `ok`: `ok := PlaceNetwork(real, null crew, 0, 0, type)` (§5).
4. If `+0x20C ≠ 0`: `ok := false`.

### 3.3 mask ≠ 0: a run that extends into existing networks

Used by the zone developer's street drawing (`mask = 0xC49`, zones.md §7.10).

1. Build an OR occupant filter on `mask` (`cSC4NetworkORFilter`, constructor `0x0061F510`).
2. `dx := x2 − x1`, `dz := z2 − z1`, from the **unclamped** arguments.
3. **Diagonal** (`dx ≠ 0` and `dz ≠ 0`): nothing is placed, and the call returns **true** *[quirk]* (the result byte
   starts at 1, `0x0062925A`).
4. **Single cell** (`dx = dz = 0`):
   1. Look at the four neighbours (offset tables `0x00AA8390` / `0x00AA83A0`) with `GetNetworkOccupant`
      (`0x00624B70`: occupant manager slot 32 with the filter, then QI `0xA821EF94`; false outside the city).
   2. A hit to the west or east marks the run as running in x; a hit to the north or south marks it as running in z.
   3. With no hit, place the single cell: `DrawNetworkLine` + `PlaceNetwork` as in §3.2.
5. **Straight** (exactly one of `dx`, `dz` non-zero), or a single cell with a neighbour hit. Directions are
   `d = (sign dx, 0)`, then `d = (0, sign dz)`. For each direction that is non-zero:
   1. Look one cell before the start (`s' = start − d`) and one cell past the end (`e' = end + d`).
   2. If a network occupant matching the filter is there:
      - of the same type (`IsOfType(type)`, network occupant slot 22): extend the run to include it;
      - of another type: record a **join** `(start, s')` or `(e', end)` for later.
   3. If no occupant is there, in **draft** mode only: if the world-cache cell at that position has flags
      `& mask ≠ 0` (a drafted piece), extend the run to it.
   4. Anchors := [start, end]; `DrawNetworkLine`; `PlaceNetwork(real, null, 0, 0, type)`. The result is the last
      such call's.
6. After the loop:
   1. If `+0x20C ≠ 0`: the result is false.
   2. If the real byte `+0x50` is set and `callback ≠ null`: for each join, in order,
      `callback.PlaceNetworkOccupants(a.x, a.z, b.x, b.z, 0, null, real, −1)`. This lays the one-cell link in the
      callback tool's own type.
   3. Free the join list.

### 3.4 End

Anchors cleared; `+0x208 := 1` (`0x00629980`); return the result.

**So, for a scripted action** *[compat]*:

- The call is **free**: nothing is withdrawn from the budget (§8).
- It makes **no regional (neighbour) connection** (§7.3).
- It inserts the occupants into the city **before returning**, with `data4 = 3`.
- It marks them usable at once (§6.3).
- It draws **no random numbers**. There is no call to the `cRZRandomSuper` draw `0x00913A06` anywhere in
  `0x00620000`–`0x0065FFFF`; the network-side callers are the construction crew and the network lot manager only.
- A failed validation places **nothing** of that run (§5.2), and the call returns false.

### 3.5 Ordered dragged-cell footprint (NT28)

Slot 23 copies the flat **dragged-cell** vector, including every cell of every step, in stored order. It does
not return one representative cell per step, filter to valid steps, or append the separate extra-cell inventory
used by intersection auto-complete. In particular, a one-cell-wide diagonal staircase has two cells in many
steps. The statement in network-geometry §3.3 that width 1 has one cell per step applies to straight runs only.
The following specifies the footprint before existing-network alignment or auto-connection changes it.

Use the adjusted endpoints and axis outputs of network-geometry §3.2. The major axis is x when `abs(dx) > abs(dz)`,
otherwise z. Group the axis-aligned staircase positions into rows of constant major coordinate. The staircase
moves toward the end along the larger remaining absolute delta; on equality it takes the **other axis from the
previous move**, with the initial previous move treated as x (so the first equal-delta move is z). Each row's
minor-coordinate interval includes **all** positions visited at that major coordinate. Thus a diagonal's
intermediate turn cell belongs to the same step as its row neighbour, rather than becoming an extra step or an
extra-cell record (`0x00639810`–`0x0063990B`).

Widen those intervals with the type's two widths `w1, w2` (network-geometry §2.3). The widening side is negative
minor coordinates for x-major/end-direction 1 or 3, or z-major/end-direction 1 or 7; otherwise positive. Widening
means extending only that side by `width − 1`. Use `w1` on the straight-prefix rows. On the remaining diagonal
rows use `w2`: widen the first such row only if there was a straight prefix or its interval is a singleton;
widen interior rows; widen the last row only if its interval is a singleton. These endpoint tests prevent
widening an already two-position diagonal endpoint a second time (`0x0063990B`–`0x00639A0F`).

Rows are emitted from the start toward the end along the major axis. **Within each row, minor coordinates are
always increasing**, including reversed drags; neither radial distance from the start nor pointer/set order
determines the width order (`0x00639AF7`–`0x00639D66`). For a two-wide horizontal run at z=28, away from existing
networks, each x row is `(x,27),(x,28)`. Reversing the drag reverses the x rows, not those two cells within a row.
Existing-network alignment and auto-connection can subsequently shift or extend this same vector
(`0x00639D66`–`0x0063A633`; network-geometry §3.3); sample the final copy, not a reconstruction from the original
endpoints. Validation does not truncate this public footprint to its valid prefix.

---

## 4. PlaceNetworkOccupantById (slot 8, `0x00624FD0`, pop 20, no result)

`PlaceNetworkOccupantById(x, z, piece, usable, type)`. It places one piece at one cell, without a drag or a solve.
`CreateConnectionNode` uses it with `piece = 0x100`, `usable = false` (network-manager.md §7.7).

1. Build a cell record for `(x, z)`:
   - the type bit from the table at `0x00AA835C` (`1 << type` for types 0–12);
   - for each of the 4 corner vertices (vertex index `z × stride + x`, then `+ stride`, `+ stride + 1`, `+ 1`):
     `h := slot74(terrain.GetAltitude(vertex))` (terrain slot 12). Slot 74 is the identity for surface tools and
     `h − depth` for underground ones.
   - The corner's allowed range is `[h − tol, h + tol]`, `tol = +0x2C4`.
2. Save the real byte; `+0x50 := 1`.
3. `InsertNetworkOccupant(cell, {piece, mode 2}, &out)` (slot 72, §6.2).
4. For each occupant in `out`: if `usable`, `SetUsable(true)` (network occupant slot 20) and
   `ClearNetworkFlag(0x4000)` (slot 11); then Release.
5. Restore the real byte.

The occupant-manager insert uses the current `+0x208`, which is 1 here.

---

## 5. PlaceNetwork(real, crew, arg3, neighbourMode, type) (`0x0063B830`, pop 20, AL)

The common pipeline. The player's drag reaches it through `PlaceAllSegments` (slot 19, `0x0063BDC0`, §8.2); code
reaches it through §3.

### 5.1 Steps

1. If the dragged step count `+0x70 = 0`: return **true**.
2. `+0x50 := real`. Clear the per-run bridge and tunnel vectors.
3. **Network-specific placement** (slot 53, with `arg3` and `neighbourMode`). The base (`0x0062C980`) returns false.
   The power-line tool's returns true after doing its own placement (§9). True → return **true** at once.
4. **Validation passes** (repeat until stable). Each pass may cut the valid step count `+0x78` (the run is placed only
   up to the last valid step). A pass that cuts sets the failure code (§5.3), and the run is then refused entirely
   (`+0x78 := 0`):

   | Pass | Code |
   |---|---|
   | slot 66 `IdentifyStartingConditions` (`0x00638710`): the first cell is not in water | `0xF0000001` |
   | `0x00639490`, tunnels (never cuts, so this code is never set; network-geometry.md §6.3) | `0xF0000004` |
   | slot 68 (`0x00639170`), reserved tiles | `0xF0000002` |
   | slot 59 `UpdateOccupantHighlights` (`0x006299A0`), what would be demolished | `0xF0000002` |
   | `0x00632830`, bridges | `0xF0000003` |
   | slot 69 `CheckForGoodTerrain` (`0x00638990`): no regular step crosses water | `0xF0000001` |

   Then, if `+0x354`, the land-bridge pass (`0x00632AD0`). The passes are network-geometry.md §4–§7.
5. If `+0x78 > 0`, **solve**. In order:
   1. cell infos (`0x00626130`, `0x00647B40`, `0x00639040`, slot 71 `CreateCellInfos`);
   2. `AssignEdgeMajorTypes` (`0x00638160`);
   3. grouping info, if the type is wider than one cell;
   4. `DoAdditionalDraws`;
   5. `IdentifyHighwayIntersections` (failure `0xF0000005`);
   6. `BuildSetOfCellsToBeSolved` (`0x006355C0`);
   7. `ResolveOneWayConflicts` (`0x0063A640`, type 10 only; network-rules.md §6.5);
   8. `MarkNeighborConnections` (`0x00626EB0`);
   9. `SolveNetwork` (`0x00635CB0`, failure `0xF0000007`);
   10. up to 4 tries of `SmoothenNetwork` (`0x00637A80`, failure `0xF0000006`), each followed by
       `CanTilesBeSupportedOnTerrain` (slot 56, failure `0xF0000008`).
6. **On failure**: `+0x78 := 0`; clear the solve state (`0x00647E20`). If the stop-on-failure byte `+0x52` is set
   (the default), return **false**.
7. **Regional connection**, only if `neighbourMode ≠ 0`: `AttemptNeighborConnections` (`0x00628BD0`) and, if it
   finds one, a retry of the solve. When `real`: `PlaceNeighborConnections` (`0x00624610`).
8. **Commit** if `+0x78 > 0`, as in §6. Return **true**.

### 5.2 What a failure leaves behind *[compat]*

A failing run inserts nothing, and the solved set is empty. But:

- the world cache was already cleared at §3.1 step 5 (real mode);
- the failure code stays in `+0x20C` until the next `PlaceNetworkOccupants` or `ClearFailureReason` (slot 13);
- `GetFailureState` (slot 14) returns **whether it is nonzero**, not the code itself (§5.3).

### 5.3 Failure codes and reason strings

**NT28, public versus internal failure.** Slot 14 (`0x0062C8F0`–`0x0062C8FD`) returns exactly 0 or 1 in EAX:
0 when the internal reason is zero, 1 otherwise. Its Boolean result must not be compared with the internal
`F000xxxx` reasons below. The observed rail, avenue and crossing `failure=1` is this normalization, not a special
literal-1 validation reason. The default stop-on-failure contract is still §5.1: a **nonempty** run refused by
validation/solve has valid=0 and returns false; the empty-drag early success is a separate case. A replay returning
true/failure=0/valid=0 for a refused nonempty run has lost the failure, even though its valid count agrees. These
public values alone cannot identify which terrain/reservation/solve test refused a particular city drag; no new
rail or avenue rejection rule can be inferred from the Boolean 1. Apply the existing passes and their native
terrain, occupant and resource inputs. `ClearFailureReason` clears the internal reason and hence makes slot 14
return 0, without rebuilding the drag. This corrects the former claim here and in the ABI notes that slot 14
returns the raw code.

The UI shows the reason string. `GetFailureReasonString` (slot 52, `0x0062BF60`; the power-line tool's
`0x0064F570`) loads the LTEXT with instance = the
code from group `0x2A592FD1` (`SimCityLocale.DAT`). If the string is missing, it returns a fixed "missing failure
reason" text. The meanings below paraphrase the shipped English strings.

| Code | Set by | Meaning |
|---|---|---|
| `0xF0000001` | passes 66 and 69 | wrong terrain type for this network |
| `0xF0000002` | pass 68, highlights | reserved tiles (something in the way that may not be removed) |
| `0xF0000003` | bridge pass | a bridge needs a straight drag across the water |
| `0xF0000004` | tunnel pass | a tunnel needs a straight drag |
| `0xF0000005` | highway intersections | the intersection cannot be built |
| `0xF0000006` | smoothing | the grade is unsuitable |
| `0xF0000007` | solve | the area is unsuitable |
| `0xF0000008` | terrain support | smoothing failed |
| `0xF0000011` | power line (§9) | a cell is zoned |
| `0xF0000012` | power line | a cell has a building |
| `0xF0000013` | power line | a cell has a road |

---

## 6. Commit

### 6.1 Order (inside `PlaceNetwork`, `+0x78 > 0`)

1. **Real**: slot 70 (a stub); slot 60; slot 57; `MarkPositionChanged…` (`0x00629CF0`); `MarkTerrainChanged…`
   (`0x00629EB0`). **Draft**: `UpdatePositionChangedPreviewsForPreviousSegments`.
2. If preview update is enabled (`+0x234`, default 1), the inserts run in this order:
   1. `InsertOccupantsForSolvedCells(&solved, &set, false)` (slot 55, §6.2);
   2. `InsertBridgePieces` (`0x0062F610`);
   3. `InsertTunnelPieces` (`0x006287D0`);
   4. `InsertHighwayIntersectionPieces` (`0x0062C4B0`).
3. **Real**:
   1. If `crew ≠ null` and the run qualifies: `crew.AddNetworkSegment(&set, …)` (`0x00606B50`). It qualifies when
      the straight count `+0x74` equals the valid count `+0x78`, or is 0, or the type's two crew parameters (type
      record `+0x1C`, `+0x20`) both equal 1.
   2. **Always**: `MarkOccupantsUsable(&set, release = true)` (`0x00605B30`, §6.3).
4. **Draft**: the occupants go into the preview set `+0x228`; `SetTerrainHilight`.

### 6.2 InsertOccupantsForSolvedCells (slot 55, `0x006258C0`) and InsertNetworkOccupant (slot 72, `0x006278A0`)

**Draft** (or the third argument true): each solved cell becomes a preview (`AddCellPreview`, slot 58).

**Real**: for each solved cell, set its "placed" byte (cell record `+0x51 := 1`), then do the first that applies:

1. **The cell holds a connection node** (flag `0x200000`, network-manager.md §7.7):
   1. Refresh its models.
   2. Set `0x8000000`; clear `0x2000000` and `0x4000000`.
   3. Occupant manager `RemoveOccupant`, then `InsertOccupant(n, 0)`. That re-insert uses data4 **0** (`0x00625AEE`;
      zones.md §5.2).
2. **An immovable prebuilt piece** (`0x400000`) of a one-way type (cell flags `& 0x400`) whose direction changed: set
   the new direction, then `SetUsable(false)` and `SetUsable(true)`.
3. **Otherwise**: `InsertNetworkOccupant(cell, request, &out)` (slot 72):
   1. Create and configure the occupant (slot 54 `CreateNetworkOccupant`, models).
   2. If the piece needs supports (`AddSupportStructures`, `0x00643FB0`): set flags `0x40000000` and `0x8000000`.
      `0x40000000` means "elevated, with supports" (NM3; the support level rule is network-geometry.md §10.2).
   3. Clear `0x400000` (`0x006279F2`).
   4. **Clear the cell** (`ClearNetworkOccsFromCell`, `0x006272F0`, §6.4).
   5. `occupantManager.InsertOccupant(n, +0x208)` (`0x00627A16`, `0x00627A38`). The listeners see `0x99EF1142` with
      that `data4` (traffic-network.md §3, network-manager.md §6.1, zones.md §5.2.1).
   6. Keep `n` in the cell record (`+0x54`) and in `out`.
   7. If the request is usable: `ClearNetworkFlag(0x4000)`. Otherwise `SetNetworkFlag(0x4000)` (drawn translucent).

The underground override (`0x006599D0`) does the same through `CreateNewUndergroundOccupant` (`0x006596C0`).

### 6.3 MarkOccupantsUsable(set, release) (`0x00605B30`)

For each occupant in the set:

1. If the 3D view's current level (view slot 22) is 1: `ClearNetworkFlag(0x4000)`.
2. `AsOccupant().SetVisibility(true, true)` (occupant slot 18).
3. `SetNetworkFlag(0x10000000)`. This is the **usable** flag. Setting it posts `0x89D3E9F3` with data1 = 1
   (network-manager.md §5). The post is queued, so the listeners (the network lot manager, road style) see it after
   the call returns.
4. If `release`: Release.

`PlaceNetwork` calls it **unconditionally** in real mode, with or without a crew *[compat]*. A code-placed piece is
therefore usable as soon as `PlaceNetworkOccupants` returns. The crew (player path, §8.2) adds only the construction
animation.

### 6.4 ClearNetworkOccsFromCell(x, z, filter) (`0x006272F0`)

1. Query the occupants in the cell (occupant manager slot 40).
2. Each one matching the tool's layer filter (`+0x364`): `occupantManager.RemoveOccupant(o, true, 0)`. This removes
   the older piece of the same layer that the new piece replaces.
3. Otherwise, for the surface tools (not pipe, power line or subway), a **power pole** in that cell is demolished:
   - occupant type `0x2890D4DE`, in the city;
   - QI `0xA8C86142`;
   - the pole's own cell (pole slot 8) equals `(x, z)`.

   The call is `demolition.DemolishOccupant(true, pole, 1, 0x2001, null, false, null)` (`0x0062746A`;
   demolition.md §9).

A road drawn across a power line therefore removes the pole standing in that cell, but not the wire *[compat]*.

### 6.5 Corner refresh and commit-added neighbours (NT30)

The retained corners used by the price getter are the **post-refresh cell-info corners**, not necessarily the
corners originally cached when the cell entered the solve. Real commit grades the terrain first (§6.1), then
performs two different refreshes before inserting occupants:

| Refresh population | Corner rule | Admission to the solved vector |
|---|---|---|
| Every cell in the smoothing set `+0xA4` | Replace all four corners by slot 73's network height. This happens even for a cell already in the solved vector, an unplaced cell, or a cell whose refreshed heights compare equal. | Append only when at least one old corner differs, the solved-cell id `+0xB8` is negative, and the cell was already placed (`+0x51 != 0`). |
| Cells incident to each actually changed terrain vertex, after the first refresh | Only for available, already placed cell infos with negative solved-cell id: copy each available **network** satisfier height at its ordinary vertex id; a missing height leaves that corner unchanged. | Append each qualifying cell, whether its corners changed or not. The newly assigned solved-cell id prevents another append at a later shared vertex. |

Slot 73 (`0x006444F0`) returns the network satisfier's height when present, otherwise the **current terrain**
height; it does not fall back to the old cached corner. In the first refresh a connection-node cell adds
**100000** to each vertex id for this lookup. Other cells use the ordinary ids. The second refresh calls the
network satisfier directly at ordinary ids, without that offset or a terrain fallback *[quirk]*. Neither pass
copies the terrain satisfier into network corners. A replay must keep these two satisfiers separate.

For a changed vertex `(vx,vz)`, the second population is its incident cells with x from `max(vx-1,0)` through
`min(vx,X-1)` and z from `max(vz-1,0)` through `min(vz,Z-1)`, inclusive. Vertex coordinates here are obtained by
division/remainder with the X vertex stride. Visit changed vertices in the grading output order and incident
cells by increasing z then x. The first pass uses the smoothing set's hash iteration order, not dragged-cell
order. Both append the existing occupant's piece and orientation and the cell key, recording the new vector
index in `+0xB8`. They preserve run/intersection membership. Thus an appended non-run neighbour has earthwork
but no ordinary cell charge. Existing solved entries keep their position while their cached corners can change.

Evidence: first population and float32 corner stores `0x00629CFB`–`0x00629D9E`, append
`0x00629DC6`–`0x00629E28`; network/terrain fallback `0x006444FE`–`0x0064451C`; incident bounds and traversal
`0x00629ED2`–`0x00629F69`, eligibility `0x00629F70`–`0x00629F8B`, direct network lookups and append
`0x0062A026`–`0x0062A07A`. The missing-height helper returns false without writing its destination
(`0x0065EB1D`–`0x0065EB44`). Grading's changed-vertex threshold is strictly greater than 0.001
(`0x006445AD`–`0x006445C1`); a solved vertex below that threshold does not by itself admit incident neighbours.

---

## 7. Auto-connection

The tool joins new pieces to existing networks in three ways.

### 7.1 Edges in the solve

`MarkNeighborConnections` (`0x00626EB0`) and `SolveNetwork` treat the cells next to the run as part of the solve. An
existing piece next to an end of the run is re-solved and re-inserted with the new edge (a T or a cross). This is how
a code-placed road "connects" to the road it touches. The rebuilt neighbour is a new insert (`0x99EF1142`, data4 =
`+0x208`) after the removal of the old piece (§6.4, `RemoveOccupant(…, true, 0)`).

### 7.2 Extension and joins (mask ≠ 0)

See §3.3. With a mask, a run is lengthened by one cell at each end into a same-type piece. A cell of another type
masked in is linked by a separate one-cell `PlaceNetworkOccupants` on the callback tool.

### 7.3 Regional connections

These run only with `neighbourMode ≠ 0`, which only the player's UI passes (§8.2). A code placement that reaches the
city edge makes **no** neighbour connection *[compat]*.

---

## 8. Cost and the budget

### 8.0 Highlight pass boundary (NT37, partial) *[compat]*

**Partial answer 2026-10-28 (re-blockers37b).** Base slot 59 `UpdateOccupantHighlights` (`006299A0`) does
not itself reject a series of occupants one at a time. With valid-step count `+78` zero it calls virtual
slot 17 (`+44`, clear highlights) and returns integer zero (`006299A6–006299BD`). Otherwise it makes an
ordered vector of candidate **cells**: walk valid dragged steps in step order, skip steps whose kind is
3, walk their inclusive cell-index intervals in increasing order, and append a cell only if `GetCellInfo`
returns null or record byte `+56` is zero. Then walk the extra 20-byte cell records at tool `+F0…+F4` in
vector order and apply the same test/append. There is no de-duplication in this outer candidate-vector pass
(`00629A00–00629BA5`).

The pass destroys its previous region at `+210`, makes a fresh temporary result set, and calls
`ComputeDemolitionRegionForPlacementCells` (`0061F7C0`) with `(candidateCells, &regionAt210, &costAt220,
&temporarySet, toolFilterAt364)` (`00629C53`). It then replaces its stored set at `+214/+218` with the
temporary set, disposing of the old contents. Let n be slot 59's input integer. The result is **n** if
the helper returns true, otherwise **n−1**, with no per-cell truncation in this wrapper
(`00629C7D–00629C90`). This does not establish which validation pass refused the question's first road.

Still required to close NT37: the demolition helper's candidate expansion and eligibility, acquisition and
release order, per-occupant demolition-cost arithmetic, and the surface/underground virtual overrides.
The outer filter is the tool's layer OR filter; preserving n and omitting the helper's cost/region writes
does not implement this boundary. The inspected wrapper has no CRT `rand()`; no claim is made here about
unresolved helper/subclass paths. The cited native/core failures remain unexplained by this partial answer.

### 8.1 GetCostOfSolution (slot 20, `0x00625640`, returns int)

The tool only **prices**. It never touches the budget. The price of the current solution is the sum of:

| Term | Value |
|---|---|
| demolition | `+0x220` (the demolition cost of what the drag clears, from `UpdateOccupantHighlights`) |
| earthwork | `S := Σ abs(corner height − terrain altitude)` over the 4 corners of every solved cell, accumulated in a float (terrain slot 12 at the corner vertex). Then `r24(S × +0x328 + 12582912.0f)`, stored as a float, whose bit pattern minus `0x4B400000` is the integer (`0x00625834`–`0x0062584F`). This rounds to nearest, half to even, for values below 2²² |
| cells | over every solved cell that is part of the run (cell record `+0x10`) and is not marked as an intersection cell (`+0x52 = 0`): the **largest unsigned** per-type cell cost among the types in the cell's type mask, starting at 0. The per-type cost table is at `0x00B453A4`, stride `0x114`, filled from the network exemplars (network-geometry §11) |
| bridges | per bridge: `(end − start) × (bridge per-cell cost + +0x320) + bridge fixed cost` (bridge **type** `+0x154`, `+0x150`, 0 for every shipped type; network-bridges.md §12) |
| tunnels | per tunnel: `(end − start) × +0x324` |
| intersections | per highway intersection: its rule's cost (rule `+0xB4`) |

The power-line tool has its own `GetCostOfSolution` (`0x0064F0F0`, network-geometry.md §11). The per-type values
are the placement exemplars' (network-geometry.md §2.4, NT2).

#### 8.1.1 Retained solution and post-commit terrain (NT29)

Each call prices the tool's **retained solved-vector entries**, in their current order, looking up each entry's
cell key through the world cache (`0x00625647`–`0x0062568D`). It does not iterate slot 23's drag vector, the extra
cells, the smoothing set, or every cached cell. A missing cell info contributes nothing; `GetCellInfo` first
uses the cache and computes/stores a cell info on a miss (`0x00633220`–`0x00633241`). The solved vector can contain
neighbours added during commit (§6.1), so the drag footprint is **not** a universal cost inventory. A simple new
road with no re-solved neighbours nevertheless has a solved cell for each staircase footprint cell: the
intermediate cells from §3.5 are ordinary run cells and incur the normal cell cost.

For each available cell info retain its type mask, run-membership byte `+0x10`, intersection marker `+0x52`,
four corner heights `+0x58..+0x64`, and corresponding vertex IDs `+0xA8..+0xB4`. Cell cost requires run membership
and a clear intersection marker; **earthwork is evaluated regardless of both bytes**. The marker is set/cleared
by `MarkIntersection` (`0x0063B180`–`0x0063B184`); it does not mean "already built". Commit's placed byte `+0x51`
does not suppress cell cost. Consequently repeating a flat existing road can still cost 110 for eleven cells;
the getter does not deduct construction cost just because an occupant now exists there *[quirk]*.

At the time of the getter, for each corner subtract the **live terrain altitude at its stored vertex ID** from
that cell's retained network corner height and take the absolute difference. Do not substitute the cached
pre-edit terrain snapshot, a new height solve against post-commit terrain, or the terrain satisfier's target
heights for the retained network corners. Commit first applies the terrain satisfier (`+0x188`) through slot 57,
then refreshes the smoothing cells' network corners from slot 73 (`+0x124` network satisfier, with live terrain
fallback; `0x00629D47`–`0x00629D7D`, `0x006444F0`–`0x00644523`). Changed existing occupants may add solved entries.
Keep that final solution and those refreshed cell infos through the post-commit sample. Insertion sets placed
state but does not replace this inventory with a fresh occupant scan.

The price getter calls terrain slot 12 anew for **each** corner of each solved entry, including repeated vertex
IDs shared between cells. Accumulate in solved-entry/corner order, rounding the running sum to float32 **after
each addition** (`0x006256DB`–`0x0062570E`). Only then multiply by the earthwork factor, add 12582912, round to
float32 and interpret the bits as described above (`0x0062582A`–`0x00625859`); there is no deduplication of vertex
charges or per-cell integer rounding. Slot 57 changes terrain only for a difference strictly greater than
0.001 (`0x006445A3`–`0x006445D1`), so even a supported ordinary road need not have exactly zero post-commit height
residual. Supported/elevated pieces can retain larger network-to-ground differences.

The reported 110/113 and 55/61 mismatches cannot be repaired by an additive adjustment or assumed tolerance.
They require the same final solved entries, retained network heights, intersection flags and live terrain reads.
The public totals do not reveal the individual residuals. The 170/102 and 90/54 diagonal results also require
the full staircase inventory; with road cost 10, zero other charges and flat terrain, seventeen and nine new
run cells cost **170** and **90**, respectively. Bridge and power-line prices additionally require their own
retained records/resources; the generic per-cell formula cannot stand in for those owners.

#### 8.1.2 Admitted retained-corner fixtures (NT30)

The following are **controlled replay inputs**, not recovered internal snapshots of Konradshohe. They use the
question's coordinates so an implementer can separate footprint, refresh and pricing errors. Public totals
110/113, 170/192, 90/98 and 55/61 alone cannot identify a unique native corner/entry fixture. In particular,
subtracting those totals does not identify a vertex, neighbour, or terrain adjustment. Do not fit residuals to
those differences, substitute a native total, or claim the city probes exact on these vectors.

Take an otherwise empty square city with 128 cells per side, vertex stride 129 and terrain altitude 100.0 at
every vertex. For a cell `(x,z)`, its ordered vertex ids are `v, v+129, v+130, v+1`, where `v=129*z+x`.
Every listed entry is available, in-run, non-intersection, non-connection, with four retained corners
`(100,100,100,100)`; the network and terrain satisfiers both return 100 at these vertices. Demolition, bridge,
tunnel and intersection inventories are empty. Use road cost 10, street cost 5 and earthwork factor float32
0.1. The ordered entries below completely define each fixture's price inputs. They do not assert that piece
evaluation in an arbitrary occupied city produces this same solved-vector order.

| Fixture | Ordered solved entries (inclusive integer ranges) | Entries / corners | Expected price |
|---|---|---:|---:|
| Straight road | `(x,4)` for x=4..14 | 11 / 44 | 110 |
| Diagonal road | `(20,4)`, then for j=1..8, `(19+j,4+j),(20+j,4+j)` | 17 / 68 | 170 |
| Shared-endpoint road | `(14,4)`, then for j=1..4, `(13+j,4+j),(14+j,4+j)` | 9 / 36 | 90 |
| Straight street | `(x,8)` for x=64..74 | 11 / 44 | 55 |
| Repeat straight road | Same entries as the first row, now placed=true | 11 / 44 | 110 |

The third row is a price fixture with flat equal-height existing endpoint; it does not reset the endpoint's
run byte merely because an occupant was already there. Shared vertex ids remain repeated corner observations.

Two differential extensions exercise §6.5 rather than fitting the city:

- **Refresh an existing solved entry.** In the straight-road fixture change the cached corners of `(4,4)` to
  `(110,110,110,110)` before real commit, retaining 100 in the network solution and live terrain. Include it
  in the smoothing set with solved id 0. Before refresh the fixture costs 114; after the first refresh its
  four corners are 100 and it costs **110**, with the same eleven entries. An implementation refreshing only
  appended neighbours incorrectly retains 114. Equal-height network and graded terrain also explain why a
  repeat can have zero earthwork without suppressing its cell charges.
- **Admit a terrain neighbour, with one missing network height.** Start from the flat straight-road price
  fixture after its position refresh. Add one placed, non-run, non-intersection cell `(3,4)` with solved id -1,
  ordered ids `(519,648,649,520)` and cached corners `(102,103,104,104)`. Keep it outside the smoothing set.
  Grading changes vertex 520 from 99 to 100, and its output vector contains only 520. All other incident cells
  have already nonnegative solved ids or are unplaced. The network solution contains heights 100 at ids
  519/648/649 and no height at 520. The terrain-neighbour pass appends `(3,4)` once and retains corners
  `(100,100,100,104)`: total **110**, since 4×0.1 rounds to 0. With factor=1 the same inventory costs
  **114**. Replacing the missing corner by live terrain incorrectly gives 110 in that second check.
  Calling the price getter before this append ignores the cached neighbour. Repeating vertex 520 in the
  changed list must not append it a second time.

For a city differential retain, on the replay side, each entry's key and solved index; old and post-refresh
corners; run/intersection/placed/connection bytes; whether it came from the piece solve, position refresh or
terrain refresh; network lookup hit/value (including offset ids); and live terrain reads at getter time.
Compare that trace with these controlled fixtures first. NT27 supplies no public native getter for this trace;
the four observed city totals remain evidence of differences, not admitted internal inputs.

#### 8.1.3 Nonflat solve, grading and refresh fixture (NT34)

This is an admitted **constraint-boundary fixture**: it starts after piece/model selection and plane/range
construction, at the inputs to the two height solves. It certifies those solves and commit refresh, not that
the shipped road picker selects these synthetic piece constraints for a particular city drag. Use a fresh
193-bucket satisfier for each solve, a 128-cell square city, stride 129, and one available solved cell `(4,4)`
at index 0, also the sole smoothing cell. Connection, intersection, placed and run flags are false; the
boundary set and all extra/intersection constraints are empty. No other occupants are available. Other
terrain vertices are 100. All numbers below are exactly representable float32 values; no tolerance is needed.

| Corner | Vertex | Initial live terrain t | Post-piece network target n |
|---|---:|---:|---:|
| 0 | 520 | 100 | 102 |
| 1 | 649 | 101 | 104 |
| 2 | 650 | 102 | 106 |
| 3 | 521 | 101 | 104 |

Set cached network corners to n. In variants A/B the piece is fixed, placed=false (new), and its model supplies
these four fixed network heights; support lookup succeeds. These are fixed corners, not a flat-cell equality.
The network solve fixes them individually (network-geometry §9.3). In C the piece is regular, not fixed,
support lookup fails, and each post-plane allowed range is the singleton `[n,n]`, preferred range `[n,n]`;
there are no additional slope/equality constraints. Thus C also has exactly the same network solution.
Use network default range [0,1000], grading raise/cut limits 10/10, with support clearance as below.

| Variant | Fixed / support clearance | Terrain allowed ranges in corner order | Terrain preferred values | Terrain solution in corner order |
|---|---|---|---|---|
| A | true / 10 | [92,102], [94,104], [96,106], [94,104] | t | (100,101,102,101) |
| B | true / 1 | [101,102], [103,104], [105,106], [103,104] | t | (101,103,105,103) |
| C | false / no supports | [102,102], [104,104], [106,106], [104,104] | fixed at n | (102,104,106,104) |

Both solves succeed. In ascending hash-bucket iteration/pick order the network solution is
`(649,104),(650,106),(520,102),(521,104)`. Terrain solutions in that same order are respectively
A: `(649,101),(650,102),(520,100),(521,101)`;
B: `(649,103),(650,105),(520,101),(521,103)`;
C: `(649,104),(650,106),(520,102),(521,104)`.
There are no bucket collisions or constraint-propagation neighbours. Equal-width ranges therefore pick
649,650,520,521; preferred t is kept in A and clamped to the lower bound in B. C's ranges are singletons.

On real commit A writes no terrain vertices. B and C write all four in terrain-solution iteration order
649,650,520,521, since every change exceeds 0.001. The first NT30 refresh retains corners
`(102,104,106,104)` in **all three** variants: it reads the network solution. No solved entry is appended.
Getter-time live terrain equals the respective terrain-solution row. At earthwork factor=1 the cell's
earthwork contribution is A=12, B=4, C=0; shared vertices would still count per corner per solved entry.
There is no run-cell charge in this fixture. Supplying a bridge model's fixed heights does not by itself
supply a charged bridge record.

The discriminating rule is in the terrain-support solve, before commit: `hi=min(n,t+raise)`,
`lo=t-cut`, then `lo=max(lo,n-clearance)` for supports. A fixed but unplaced cell prefers **t** within that
range. A regular unsupported cell must grade exactly to **n** (and fails if the raise limit cannot reach n).
A fixed **and placed** cell instead fixes terrain at t, before these support calculations. Changing A to
fixed-and-placed therefore keeps t even with clearance=1. Changing B's raise limit to 0 makes its ranges
infeasible and the support pass returns false; it must not commit the proposed grading.
Evidence: fixed/placed gate `0x00644CAB`–`0x00644CC4`, bounds `0x00644CDD`–`0x00644D79`, unsupported rule
`0x00644D7C`–`0x00644D9C`, fixed-cell preference `0x00644DD6`–`0x00644DEA`. All arithmetic in this fixture
is exact even across the float32 stores visible at those addresses. Solver order is network-geometry §9.1;
commit and refresh are §6.5 here.

**What the city observation establishes.** At the question's vertex 659 the replay already has both network
and terrain solutions 273.388092 and refresh keeps that value, whereas native getter-time terrain is
271.987152. Refreshing that same network result again cannot manufacture the native grading target.
The first unresolved boundary is the replay's piece/height constraints and terrain-support solution versus
the native grading/writes, not the number of refreshed entries. Those two scalar observations do not reveal
which native model height, support record, fixed/placed flag, allowed/preferred range, smoothing boundary,
intersection constraint or earlier terrain input differs. Record these inputs before each smoothing level,
both ordered solutions, and the actual terrain-write/readback stream; a mismatch before writes locates a
solve-input/result difference, while a correct terrain solution with different readback locates commit or
later terrain mutation. Keep 110/113, 170/192, 90/98 and 55/61 as city mismatches. This fixture supplies no
recovered native city constraint snapshot and licenses no adjustment to match those totals.

#### 8.1.4 Live inventory cannot be extracted from the tool getters (NT35)

There is **no admitted public call that copies the native retained solved-entry inventory or its piece/plane/
support constraints**: §2.1's coordinate/count getters do not expose these objects. Neither occupied-cell
queries nor model getters on committed occupants recover an ordered, possibly uncommitted solution or its
per-smoothing-level constraints. A host must observe the live environment through its public services and
reconstruct the solution using network-geometry §§8–10 and the rule/model resources, or start from an explicitly
supplied constraint-boundary fixture such as NT34. NT30/NT34 are such fixtures, not extracted city snapshots.
The four prices 110/113, 170/192, 90/93 and 55/61 in `nettool-close7-r2-20261024-b812` do not determine a unique
inventory or corner fixture. Retaining SGI bucket order resolves an ordering input but cannot identify missing
piece heights, plane anchors or support constraints. Report a missing observation as unavailable; do not read
the private solved vector, fit the residual, or return a native total as the reconstructed price. This is the
observation limit, not a second solver rule (`0x0062CA30`–`0x0062CA3F`, `0x00625647`–`0x0062568D`).

| Boundary conformance vector | Expected observation / permitted conclusion |
|---|---|
| NT34 A and B, identical drag/counts and network corners, support clearance 10 versus 1 | Earthwork 12 versus 4; coordinate/count equality does not select the support input |
| Slot 23 called after either fixture | Copy of drag coordinates only; no corner heights, support record or solved-entry ordering is returned |
| Live replay has coordinates/status and public price, but lacks piece/plane/support inputs | Drag/status and native price remain observable; reconstructed live price is unavailable, not zero and not the native price |

#### 8.1.5 Tempelhof single-cell draft and price controls (NT36)

NT36 here means **"Tempelhof single-cell road draft costs 11 versus 10"**, filed by nettool-cost27; it is a
different filing from the older occupied-cell host-boundary NT36 in §8.3.3. Network-geometry §§9.1 and 9.8.1
now supply an admitted isolated-cell fixture with the question's terrain values, the exact plane and per-corner
ranges/preferences, the level-0 failure/level-1 success, ordered equality propagation, hash state and retained
network result. No private native solved-state observation or additive cost inferred from a total is used.

For that fixture the **only retained entry** is road cell `(86,3)`, run=true, intersection=false; corner ids are
`(473,602,603,474)`. All four retained network corners are **293.308929443359375**, the initial terrain height
of vertex 602, selected after allowed-range propagation. Live terrain remains, in corner order,
`(291.402679443359375,293.308929443359375,290.7928466796875,289.532623291015625)` throughout this draft.

| Corner/id | Absolute network-minus-terrain delta | Float32 running earthwork sum |
|---|---:|---:|
| 0 / 473 | 1.90625 | 1.90625 |
| 1 / 602 | 0 | 1.90625 |
| 2 / 603 | 2.516082763671875 | 4.422332763671875 |
| 3 / 474 | 3.77630615234375 | **8.198638916015625** |

**Rounding and addition order remain §8.1.** Sum absolute differences in retained-entry/corner order, storing
the running sum as float32 after each corner (`0x006256DB`–`0x0062570E`). Multiply the final sum once by
float32 0.1 = 0.100000001490116119384765625; the x87 single-precision result is **0.81986391544342041015625**.
Add 12582912.0f and store float32: **12582913.0**, bits `0x4B400001`; subtract `0x4B400000` to obtain earthwork
**1** (`0x0062582A`–`0x0062584F`). The ordinary road cell charge is 10; demolition, bridge, tunnel and
intersection charges are zero: **10+1=11**. This is one final nearest-even rounding of total earthwork, with
no ceiling, truncation, per-corner/per-cell integer rounding, or draft terrain write.

The replay's four corners at 290.7928466796875 do produce the question's sum 4.386138916015625 and earthwork 0.
Pricing **that different retained solution** at 10 obeys the getter. It is the missing solve inputs/order that
must be corrected. A price residual is not a rule for moving corners to a median, minimum or mean.

These controls use the same road tuning and empty, dry environment. They are **constructed conformance inputs**,
not recovered Konradshohe city state:

| Control | Required retained solution | Earthwork / final price |
|---|---|---|
| Flat single cell: §9.8.1 with all four terrain heights 300 | Level 0 succeeds with all four corners fixed at 300; one run entry | 0 / **10** |
| Gentle single cell: §9.8.1 terrain in corner order `(300,300.5,300.25,299.75)` | Level 0 fails, level 1 succeeds. hA=300.375, hB=299.875; plane `(0,256,−8,−76384)`, corner plane heights `(299.875,300.375,300.375,299.875)`. After equality, allowed range `[265.375,334.875]`; unchanged preferred ranges span each terrain/plane pair. Vertex 602 wins and all retained corners are **300.5** | Sum 1.5; product 0.1500000059604644775390625, rounded 0 / **10** |
| Konradshohe-like gentle straight road: ordered entries `(x,4)`, x=4..14, as §8.1.2; set terrain `300+0.25×(vx−4)` at every vertex `(vx,4)` and `(vx,5)`, vx=4..15 | Eleven road-only run entries, no intersection/fixed/placed/connection flags or appended neighbours. One or two connected sides per piece; cross-sections are level across z and rise 0.25 m per x. All initial heights satisfy boundary, slope and smoothness constraints, and each preferred range includes its own initial height. Level 0 succeeds and retains terrain heights at all 44 corner observations | 0 / **110** |

For the last control, the plane uses hA=300 at x=4 and hB=302.75 at x=15; all corners stay within its 35 m
window. This retains the existing eleven-cell road price while exercising a nonflat run. The gentle single
cell distinguishes the specified nearest-even rule from an incorrect ceiling, which would charge 11.
§8.1.2's other flat prices (170/90/55/110) also remain unchanged. Neither these controls nor the Tempelhof
fixture identifies the private inputs behind earlier Konradshohe mismatch totals (§8.1.3–§8.1.4).

### 8.2 Who pays: cSC4NetworkToolUI::DoPlacement (`0x00645370`)

Only the player's tool UI withdraws money:

1. `cost := tool.GetCostOfSolution()`; `budget.WithdrawFunds((int64)(uint32)cost)` (budget slot 8, budget.md §6). The
   high dword is pushed as 0, so a **negative cost is zero-extended** into a large positive amount *[quirk]*, and the
   withdrawal fails.
2. Play the result sound (UI slot at `+0xB0`).
3. If the withdrawal **failed**: nothing is built (only the reset below).
4. Otherwise, do a draft pass, then the real pass:
   1. Draft pass: `Reset` (slot 5), `SetPreferredBridge` (32), `SetBridgeAltitudeOverride` (34), `SetAnchorPoints`
      (22), then `PlaceAllSegments(false, ui+0x2B, true, type)`.
   2. If `GetPossibleNeighborConnection` (41) and not `GetFailureState` (14): ask the neighbours simulator (slot 5),
      which gives the neighbour flag.
   3. Real pass: the same set-up, then `PlaceAllSegments(true, ui+0x2B, flag, type)`. When committing,
      `PlaceAllSegments` (`0x0063BDC0`) creates a **construction crew** for types 0–3, 6 and 8–12. It then runs
      `DrawNetworkLine` and `PlaceNetwork` for each pair of anchors.
5. After either outcome:
   1. Slots 17 and 16 and the terrain view.
   2. If it was paid: `QueryUserForPotentialIntersections` (49).
   3. `Reset`; `city.UIDecreaseLockCount` (city slot 107).

The money is withdrawn **before** the real placement, and is **not refunded** if the real pass then fails *[compat]*.
The dedicated [network UI contract](network-tool-ui.md#5-placement-and-payment) qualifies this partial
description with the owner's exact branch widths, call order and cleanup. In particular the UI forwards the
zero-extended cost to the budget; whether even a large raw amount is refused is the budget's result, and the
neighbor predicate tests the failure result's low byte. This subsection does not cover the UI lifecycle/input owner.
A harness that wants the player's economics must withdraw `GetCostOfSolution()` itself before calling
`PlaceNetworkOccupants` *[choice]*.

### 8.3 Minimum subclass replay fixtures (NT31)

Dispatch type 5 to the separate power-line placement/cost owner, and surface bridges to bridge identification,
selection and construction before the ordinary solve. A generic surface replay with an empty bridge-type
enumeration and no pole/wire host has insufficient inputs. Returning a generic refusal is a measured adapter
difference, not the native subclass result. The smallest useful fixture has three separately reported scopes:
resource loading/selection, placement with a recording host, and retained pricing. Passing one scope does not
certify the others. These fixture records are test data, not new public interfaces or guessed native signatures.

#### 8.3.1 Power line

Supply Utilities exemplar TGI `6534284A/E7E2C2DB/C911E35B` with the following **shipped** values:

| Property | Type / value | Purpose |
|---|---|---|
| `0x098B25C8` | uint32 10 | maximum drag-step spacing |
| `0x098B27D0` | float32 7.0 | terrain-test clearance |
| `0x4A5F5953` | uint32 15 | cost per counted pole |
| `0x0A5F596B` | float32 2.0 | cost per counted wire cell |

The lookup/read contract is `0x00650630`, especially `0x006506CE`–`0x00650704`; values are also recorded in
[network-geometry §11](network-geometry.md#11-costs-answers-nt2-and-the-power-line-answers-nt3).
The fallback values **20, 7, 0, 0** belong to a freshly constructed object with missing properties, not to the
shipped resource. Preserve the once-only resource latch: adding a resource after a failed first load does not
make the native process retry it (power-line-tool §1).

For a controlled empty-land placement fixture use anchors `(4,32),(16,32)`, 13 one-cell steps, the pivot
helper's prefix count `+0x74`=13, flat terrain 100, clear classifier results, no existing poles or wires,
no terrain-ray hit and neighbourMode=false. The nonzero prefix count makes each step's first cell a pivot;
do not apply this spacing expectation to a helper fixture with that count zero (power-line-tool §4.1). The
recording host must support pole create/reuse, bidirectional connections, wire creation, insertion and retained
reference cleanup as specified in [power-line-tool §§4–5](power-line-tool.md). Record requests and return stable
test objects; do not silently treat missing host operations as successful. Spacing 10 and the forced endpoint
give poles at `(4,32),(14,32),(16,32)` and ten non-pole wire cells. The pole links are first-to-middle and
middle-to-last. Expected valid=13, public failure=0 and retained price **3×15 + 10×2 = 65**. This is an admitted
empty-land vector, not proof that the Konradshohe line used these same poles. Terrain, existing endpoints,
zoning and repair can change the retained counts; replay those host inputs for a city comparison.

Loading tuning alone is insufficient for pole placement. The network INI TGI `00000000/8A5971C5/8A5993B9`
must also supply its `PowerPoles` direction-mask → pole-exemplar-instance mapping. ReadPowerPoleInfos parses
unsigned keys/values (`0x0064F880`, callback `0x0064F140`–`0x0064F160`); CreatePowerPole indexes that mapping
with the new direction mask OR the reused pole's previous mask (`0x0065016A`–`0x00650183`,
`0x006501CD`–`0x006501D9`). A host-only fixture may map the masks it actually receives to explicit synthetic
pole resources, with stable attachment points and successful initialization. Label that resource set
**synthetic**; zeros or invented instances are not shipped pole data. To claim model/curve coverage also supply
the real mapped pole exemplars, their attachment orientation, and all six curve controls in power-line-tool
§5.5. A count/price replay can omit rendering while reporting those model/curve scopes uncovered.

For a pricing-only fixture the retained inputs are counted poles=3 and counted wire cells=10, with the two
cost parameters above. Price is 65 even after endpoint references are released. The unsigned pole count ≤1
returns zero. Otherwise the wire count is converted as uint32, multiplied by the float32 parameter in x87,
then truncated toward zero, and the integer pole term is added (`0x0064F0F5`–`0x0064F12B`). There is **no
float32 store between multiply and truncation**. This qualifies the `r24` shorthand in network-geometry §11:
do not insert that rounding. With wire count=16777217, wire price=1.0, poles=2 and pole price=0 the result is
16777217, not 16777216. The getter does not add the base tool's demolition or earthwork terms.

**Retained counts after release (NT32).** The getter consumes the counted poles at `+0x388`, not the
current generated-vector length. DeterminePolePositions snapshots that length into the count at completion;
ReleasePoleInfos releases saved endpoints and structurally clears the vector without touching either cost
counter (`0x006505D0`–`0x00650628`). Thus the wrapper's final release leaves poles=3, wire cells=10 and price=65
available after return. Calling release again changes none of those values. The next slot-53 placement clears
both counters before selection (`0x00652983`–`0x00652997`); a placement too short to select poles consequently
prices zero. Do not resynchronise the saved count with the empty vector. This corrects the older shorthand in
network-geometry §11 equating the cost count with the vector size at getter time; equality holds when selection
finishes, not after release. The getter reads the saved count directly at `0x0064F0F5`.

#### 8.3.1.1 Callable power-line observations (NT33)

The clearance ray is **renderer** `cISC43DRender::PickTerrain`, not a method of `cISTETerrain`.
The active renderer is the view-lifetime service cached at `0x00B43DD0`. Its primary table is `0x00ABAB60`;
QI accepts primary IID `0xE9C622D8` (also `0x681BD529` and Unknown=1), returning the same pointer with an
AddRef (`0x007C1890`–`0x007C18DE`). Use the interface pointer acquired for the active view; do not treat a
terrain pointer or terrain-view subobject as that pointer. Balance an owned QI reference with Release.

| Observation | Callable boundary, in logical argument order | Result and ownership |
|---|---|---|
| Clearance ray | Renderer slot **61**, byte offset **+0xF4**, target `0x007C35C0`; `this` in ECX; arguments: pointer to origin A, pointer to normalized direction, pointer to writable hit XYZ, zero, zero. Callee pops **20** bytes. | bool in **AL**; true supplies hit XYZ as three contiguous float32 values (12 bytes). The caller owns this ordinary value storage; no object, AddRef or release is involved. False means no successful pick; never consume the hit storage on false. |
| Final `World::blocked(x,z,0)` probe | City slot **33** `GetLotManager` (`0x00472E70`), then lot-manager slot **6**, byte offset **+0x18**, target `0x0072B410`; ECX=manager, arguments signed x, signed z, false. Callee pops **12** bytes. Primary IID **0xE818ED2B**, table `0x00AB37C0` (`0x00729EF0`–`0x00729F55`). | borrowed lot pointer in **EAX**; non-null means blocked and maps to classifier result 2. Do not release this lookup result. Null means no lot, including out-of-bounds coordinates. |

The fourth ray argument is the false ground-plane-fallback flag: after terrain picks fail the renderer calls
RayIntersectsGroundPlane only when it is nonzero (`0x007C374E`–`0x007C3768`). The fifth stack word is also
zero on this power-line call; pass it literally as zero, without inventing a hit-distance return argument.
The only hit output consumed by powerline selection is the third argument. The renderer can return false
when its terrain view is absent (`0x007C35C6`–`0x007C35D0`); this does not supply a synthetic hit. It also
retries a failed pick from an origin raised by 4 before optional ground-plane fallback
(`0x007C3696`–`0x007C374C`). Calling the renderer preserves that behavior. At the powerline call site there
is no null-renderer guard. An integration adapter lacking the active service must report unavailable,
not reinterpret unavailable as a successful clear/no-hit observation.

Use the origin, normalization, numeric order and inclusive finite-segment test in power-line-tool §4.1.
The actual call setup is `0x00650CF5`–`0x00650DA4`; the distance test consumes hit XYZ rather than an ST0
distance. For A=(8,17.5,8), B=(40,17.5,8), normalized direction=(1,0,0): false does not request a pole;
true with hit=(40,17.5,8) does; true with hit=(41,17.5,8) does not. A hit beyond B is still a successful
renderer pick but is outside the clearance segment.

The lot probe is only the **last** classifier test, after zoning and occupant checks leave result 0
(`0x0064FE41`–`0x0064FE5C`). It is not a substitute for the complete CellIsOKForPlacement contract in
power-line-tool §3. The manager lookup itself is a bounds-checked borrowed grid read with no failure/HRESULT
channel (`0x0072B410`–`0x0072B471`). False skips its extra location validation; powerline uses false.
Missing service pointers are not an ordinary no-lot result. The full classifier's integer result is in EAX,
not AL; preserve clear=0, zone=1, building/lot=2 and road=3 and the classifier's literal quirks.

**Prefix correction.** DeterminePolePositions has no explicit prefix argument; it reads tool `+0x74`.
That field is the **straight-step prefix count**, not a history count of previously placed occupants.
DrawNetworkLine supplies it via BreakIntoStraightAndDiagSegments (`0x0063AFCC`–`0x0063AFD9`;
axis outputs at `0x006380F6`–`0x0063811D`). A fresh in-bounds width-1 axis slot-7 drag from (4,32) to
(16,32) has dragged=13 and prefix=13, even on empty land. Slot 7 initially zeros it, then DrawNetworkLine
recomputes it before subclass placement (`0x00629329`–`0x0062932C`). For a diagonal/mixed drag use the
straight prefix from network-geometry §3, including any subsequent drag-cell extension; do not set it to
zero merely because no occupants have been committed. The pivot helper uses the step's first cell while
step<prefix; later steps use the anchor-distance search (`0x0064F244`–`0x0064F29F`). The separate forced
transition point exists only when 0<prefix<valid. This corrects the older “previously placed” terminology
in power-line-tool §§1,4.1; it does not change the literal pivot or transition behavior.

#### 8.3.2 Bridge

For a shipped road resource fixture enumerate **nine**, not zero, bridge types, in INI key order:
keys 1..9 map to exemplar instances `1,2,3,5,6,7,8,9,A`, all T/G `6534284A/A82CA30F`.
Their minimum spans are respectively `10,16,4,4,22,4,10,12,4`; width=1; maximum span=100000;
clearance=[0,10000]; fixed/per-step type costs=(0,0). Their water and land eligibility bytes are both true.
These are loader/selection inputs, not an assertion that a bridge must be constructed for every 17-step drag.
Use the complete corresponding ramp/deck/pier and height data from
[network-bridges §§2–3](network-bridges.md#2-bridge-types), including length-specific specialisations keyed
by network type (road instance `0x1000`) before falling back to the parametric model. Type index is its
zero-based vector position, not its INI key or exemplar instance.

A concrete **selection-only** vector: candidates in that order, no preferred bridge, start=2, end=17,
clearance=20 and no padding needed by fitting candidates. Length=16 chooses key 2 / index 1, the exact
minimum-span fit, ahead of key 8 (minimum 12). Setting preferred key 1's index=0 chooses key 1 instead.
This follows `0x0062F760` and network-bridges §6; a zero enumeration cannot produce either result.

For placement, the minimum recording-host input inventory is: final ordered drag/step cells and straight/valid
counts; AllowBridges and land-bridge enable state; current preferred index and altitude override (default -1);
per-step water flags, surfaces and corner terrain; filtered occupant queries in each step rectangle grown by
one; existing per-type edges; chosen span's bank/reference elevation; and the selected type's complete model
resource data. Preserve ordered candidates and length specialisations. The resulting bridge record must
retain network type, inclusive start/end step indices, selected type index, reference/deck heights and model
pieces, whose fixed corner heights enter the ordinary solve (network-bridges §§4–10). A host may record model
requests without drawing, but omitting the pieces' heights changes placement and earthwork. Query failures,
empty candidate sets and a missing model must remain distinguishable from successful recorded placement.

For an independent **retained-price** vector use one road bridge record with start=2, end=11, selected index=0,
type costs=(0,0), tool bridge cost=100, no other charges or solved cells. Price is **900**, not 1000: the getter
uses end-start, not inclusive length (`0x00625740`–`0x0062577F`). Changing deck height or model ids alone does
not change that term; changing the retained span does. This prices an admitted record; it does not assert
that identification accepts an isolated record without approach cells.

For the city drag `(42,43)`–`(58,43)`, neither its 17 valid cells nor total 974 identifies the bridge's chosen
index, span, elevations, approach-cell inventory or earthwork. Do not encode 974 as a model price, infer a
particular bridge from it, or inject it as the replay result. The native resource/host observations above are
needed to instantiate a city fixture; they are not exposed by slot 23. Keep the original/core 974/0 and
powerline 65/0 differences visible, default-off and record-only until the corresponding hosts/resources are
wired and a fresh comparison passes. These contracts close the input-boundary question without claiming
pole/wire/model coverage or exact whole-owner coverage from a public-total match.

### 8.3.3 Occupied cells and the subclass/model host boundary (NT36)

**An occupied cell is an observation, not a refusal.** For power lines the admitted observation boundary is
the occupant manager, the returned occupants' interfaces, and the zone/lot/terrain services in §8.3.1.1.
Use the complete classifier and pole/wire probes in [power-line-tool §6.1](power-line-tool.md#61-classifier-and-occupant-probes),
then its §§4–5 placement rules. A list containing an occupant cannot be reduced to a Boolean blocked bit:
some occupants do not affect classification, network-capable road occupants leave it unchanged, and an
existing pole can be reused. Successful ray and lot calls alone do not complete these inputs.

#### 8.3.3.1 Callable observations and reference ownership

The following are existing interfaces, not new callbacks or new C ABI exports. Slot numbers are zero-based;
the x86 interface calls are thiscall with the particular interface pointer in ECX. The
[ABI interface tables](../abi/interfaces.tsv) and [allocator contract](../abi/allocators.md) apply.

| Observation / operation | Callable boundary and payload | Ownership / meaning |
|---|---|---|
| Occupants of one standard city cell | `cISC4OccupantManager` slot 40, pop 20: output ilist, min2, max2, null filter, maximum count `0xFFFFFFFF`; both points are `(x,z)` | Appends in manager order, one AddRef per returned occupant; AL indicates the query operation, not whether the list is nonempty |
| Occupant identity and position | `cISC4Occupant` slot 7, pop 0, class/type in EAX; slot 8, pop 4, writes a three-float world position | Query-list pointers are occupant interfaces, not subclass object bases |
| Network/pole/wire capability | Occupant slot 0 QI `(IID,out)`, pop 8; test AL; IIDs `0x4A10AFC2`, `0xA8C86142`, `0xC98F3B24` respectively | Each successful query supplies a separate reference to release, even when the same object is already in the result list |
| Pole cell after selecting a pole | Pole primary slot 8, pop 8, two integer out-pointers (`0x00649210`) | This is a different slot 8 from occupant GetPosition; preserve the interface type |
| World position to standard cell | Manager slot 19, pop 16: x,z float arguments and two integer out-pointers | The pole probe compares both converted coordinates with the requested cell; a bounding-box hit alone does not qualify |
| Insert a prepared occupant | Manager slot 20, pop 8: occupant interface, data4; AL result | Power poles pass data4=0; bridge pieces pass the tool's data4. A failed insertion remains a failed insertion, not successful host completion |

In the older shorthand “flags 0 and mask -1” in the power-line spec, these final slot-40 arguments mean
**null filter and unlimited maximum count**, not an occupant-flags mask. This follows the manager slot-40
contract and the actual pushes at `0x0064FDB0`–`0x0064FDC9`. Supply a distinct, initially empty output list
for each probe. The crossing ilist uses allocator A, a circular 12-byte sentinel and 12-byte nodes
`{next,prev,occupant}`; it is not the manager's internal cell-holder list (allocator B). Preserve list order.
After consuming it, release every element reference and free the nodes and sentinel with the matching
allocator. Manager slot 48 can release/clear result elements and nodes; the caller still frees the head
([occupant-manager §12](occupant-manager.md#12-containers-and-allocators-compat)). A recorder may copy value
observations and stable test identities, but native pointers remain valid only while their references live.

Classification starts at 0. The zone test can return 1 before any occupant read, including its literal third
probe at `(z,z)`. On the occupant path class `0x278128A0` writes 2; class `0xC772BF98` writes 3 only when
network QI fails. Successful network QI leaves the result unchanged. All other classes leave it unchanged.
The walk continues after assignments, so the **last qualifying assignment wins**. Only a final 0 permits
the lot probe, whose nonnull result writes 2. Preserve the full integer 0/1/2/3 result in EAX
(`0x0064FD87`–`0x0064FE64`); a Boolean classifier loses behavior needed by pole selection.

Pole and wire selection also scans the whole result list. The selected output is the last accepted QI;
replace it by releasing the previously selected QI reference. Pole acceptance additionally requires its
GetPosition/manager-slot-19 cell to match; wire acceptance has no endpoint or direction filter. Release
all query-list occupant references independently. A DeterminePolePositions probe is released there;
CreatePowerPole queries again rather than inheriting that earlier selection (`0x0064FFE0`–`0x0065013A`,
`0x0065015E`–`0x0065016E`). Keep query failure or a missing service distinct from a successful empty query
in the adapter's availability reporting; it is not evidence of a clear cell.

#### 8.3.3.2 Placement is a separate host responsibility

The public observations do not create a complete pole/wire or bridge model host. The host must implement
the following effects, whether recording them in a fixture or placing actual occupants. Recording these
effects certifies the recording scope only. Replacement-owned records may carry these values in any
representation; they must not be passed as original STLport containers or original class objects.

| Host responsibility | Required behavior and lifetime |
|---|---|
| Pole create/reuse | Requery the cell; reuse the last accepted pole, merge its existing direction mask with the requested mask, choose the merged-mask exemplar, and update its model. With no reusable pole create a pole and initialise it with exemplar and mask. Set the computed world position and draft state in either case. Keep a record reference until placement consumes it (`0x00650140`–`0x00650241`) |
| Connections and wires | Create all pole records before connecting any. Reuse existing connections rather than duplicating them. Draft connections are temporary; real missing wire-cell occupants are initialised with endpoints, pivot, position and mode. Preserve both endpoint updates, saved endpoint references and undo/repair lifetimes; power-line-tool §5.2 includes the index-0 wire-release quirk |
| Pole publication | Insert only newly created poles through manager slot 20. Draft reused/new lists acquire their own references; real poles refresh connections. Release and null the per-record pole reference in both modes. Releasing pole records does not erase retained pricing counts (NT32) |
| Bridge geometry | Construct the selected specialised or parametric piece records before the ordinary solve, retaining all corner heights, model offsets, supports and cable data. The 0x50-byte piece record in network-bridges §9.2 is a transient geometry record, not an occupant or a renderer handle |
| Bridge publication | Create a bridge-class network occupant (`0x49CC1BCD`) from the piece/cell corners, apply exemplar/rotation/model offset, flags, columns and suspended structures, then publish through the occupant manager. Real mode clears conflicting network occupants first; optional output-list retention takes a separate AddRef; release the local occupant reference at the end (`0x0062F55F`–`0x0062F5FC`) |

For original pole pointers, QI `0xA8C86142` returns the primary base; its occupant subobject is base+8.
Use that primary for pole methods and its occupant interface for manager insertion. The existing admitted
direct-call contract for detachment is [RemoveConnection](../abi/direct-calls.md#csc4powerpoleoccupantremoveconnection-0x0064da00):
thiscall on the first original pole base, second original pole base and false, pop 8, no result.
This is not callable on a replacement-owned record. In particular the 48-byte pole record's interface at
`+0x10`, created byte at `+0x14`, direction byte at `+0x15`, link vector at `+0x18` and saved endpoint
references at `+0x24/+0x28` are **tool-owned transient data**, not the layout of the pole itself.
Wire-cell records have stride 16, coordinates at +0/+4, owned line reference at +8 and pivot byte at +12.
The complete ownership and repair rules are in power-line-tool §§4–5; neither record is publicly extracted
from a live tool.

There is no general “place model” method accepting either transient record. Original CreatePowerPole and
PlacePoles depend on the original power-line subclass and directly construct their objects. Base-tool slot
54 likewise depends on the original cell-info layout, terrain service and mode; for bridge class it directly
allocates/constructs a bridge occupant (`0x006242D3`–`0x006242ED`). Calling slot 54 on the replacement, or
calling an original helper with a replacement's record, does not outsource those dependencies. QI and manager
insertion are admitted interface operations; these internal constructors/helpers are not a newly admitted
factory ABI. A host must supply the class/model implementation and its documented effects, or leave that
placement scope unavailable. A model-id list or successful resource selection alone cannot satisfy it.

Bridge model geometry also affects simulation: the selected piece's corners enter the height constraints;
its model origin and support-column top need not coincide. Follow network-bridges §§8–11, including the
highest-corner position and model offset; omitting rendering may be a recording choice, but omitting
those heights or support records changes the solve. The bridge insertion AL result is retained as the cell's
placed byte (`0x0062F587`–`0x0062F5AB`). Neither that result nor a price certifies all pieces' model effects.

The NT36 city observations (10 rays, 40 lot reads, straight prefix 13, selection complete=0;
powerline valid 13/0 and price 65/0; bridge valid 17/0 and price 974/0) therefore remain partial observations.
They do not describe an empty-city fixture or authorise substituting native totals. Supply these occupied-cell
observations and class/model effects, then compare fresh results. Keep missing scopes unavailable and the
replacement default-off until verified. No new game or whole-owner coverage is established by this answer.

| NT36 boundary conformance vector | Required result |
|---|---|
| Zone path reaches occupant query; list is building-class then road-class with failed network QI | Classifier 3, both list references released, no lot query; reversing that list gives 2 |
| Only a road-class occupant with successful network QI, lot null | Classifier 0; release QI and list references separately; no blanket occupied-cell refusal |
| Cell `(4,5)` query returns pole A at converted `(4,5)`, B at `(5,5)`, C at `(4,5)` in that order | Select C with one caller-owned QI reference; release B's rejected QI, A's superseded QI and all three list references; a later creation probe must query anew |
| Same list, wire probe accepts all three wire QIs | Select the last QI without applying the pole coordinate filter |
| One reused pole with old mask 1 and requested mask 4, and one new pole | Reused pole uses merged mask 5 and is not inserted again; new pole uses requested mask and is inserted with data4=0; record references are released independently of draft-list references |
| Bridge selection succeeds but model/corner host is absent | Selection scope can pass; reconstructed placement/price scope is unavailable, not successful placement or native 974 |
| One fully supplied bridge piece, manager insertion returns false, optional output list supplied | Cell placed byte false; optional list acquires its reference; local reference released; model requests alone do not turn insertion into success |

---

## 9. The power-line tool

The power-line tool uses the base `PlaceNetworkOccupants` and `PlaceNetwork`. Its slot 53 override
(`DoNetworkSpecificPlacement`, `0x00652980`) takes over at §5.1 step 3:

1. Clear the pole and line lists; valid count := dragged count.
2. With at least 2 dragged steps: `DeterminePolePositions` (`0x00650840`), then `PlacePoles(1)` (`0x00651560`).
3. If the pole span (`+0x370 − +0x36C`) is under 48 units (`/ 0x30` is 0): valid count := 0.
4. **Regional connection.** Try it only if all of these hold: `neighbourMode` is set; the whole drag is valid; there
   are at least 2 anchor cells (`+0x118..+0x11C`); and no failure code is set. `AttemptNeighborConnections`
   runs in both modes. Only a successful attempt followed by `real` calls `PlaceNeighborConnections`.
5. In draft mode, slot 82 (0) refreshes the preview.
6. Release the pole infos; return **true**. The base solve never runs for power lines.

The remaining owner contracts, including the pole records, geometry, commit/preview split and lifecycle,
are in [power-line-tool.md](power-line-tool.md). Slot 82 is `SetTerrainHilight(startStep)` (`0x0064F2B0`),
not a second placement or pole-rebuild pass.

**Reserved tiles** (`CheckForReservedTiles`, slot 81, `0x0064FED0`). The tool walks the dragged cells, asking
`CellIsOKForPlacement` (`0x0064FD00`) about each:

| Result | Meaning | Code set | Effect |
|---|---|---|---|
| 0 | the cell is clear | none | the walk continues |
| 1 | the cell is zoned | `0xF0000011` | the walk continues: zoned cells do not stop a line |
| 2 | the cell has a building | `0xF0000012` | the run is cut before this cell |
| 3 | the cell has a road | `0xF0000013` | stops the walk only on the last cell when the previous result was 0; on the first cell the previous result is uninitialised *[quirk]* (network-geometry.md §11) |

---

## 10. DestroyOccupantsAndResolve(list) (slot 50, `0x0062A5C0`)

Called by `DestroyNetworkOccupants` (network-manager.md §7.2) once per layer group:

| Group | Tool |
|---|---|
| surface types (mask `0x1F4F`) | type 0 |
| pipes (`0x10`) | type 4 |
| subways (`0x80`) | type 7 |

The network manager calls `Init` before and `Shutdown` after.

1. Save the real byte; `+0x50 := 1`; `+0x53 := 1`.
2. **Collect** the occupants and their cells. A cell key is `(x << 16) + z`.
3. **Close the set.** Repeat while the list grows:
   - For each listed occupant, look at its neighbours by network type. A neighbouring **immovable prebuilt** piece
     (`0x400000`) that is not an intersection piece joins the list.
   - An **intersection** piece (`0x100000`) brings its whole structure (network manager slot 12,
     `GetAllIntersectionPieces`).
   - A listed occupant with flag `0x80000` is a tunnel portal (network-geometry.md §10.3): QI `0x8A4BD52B` (the
     tunnel occupant) brings in the portal at the other end.
4. Clear the world cache (`0x0062B121`).
5. **Remove** each occupant, in pointer order (`std::set` order):
   1. Send `0xAA7A014D` **synchronously** (message server slot 3), IGZUnknown = the occupant. Its neighbours answer
      by dropping their connection to it (demolition.md §3).
   2. Get its cell (network occupant slot 59).
   3. If the cell key is in the set of **destroyed cells** (the cells of the caller's occupants, plus those added
      while closing the set): `occupantManager.RemoveOccupant(o, false, 0)`. Otherwise (a piece pulled in by the
      closure) `networkManager.RemoveOrReplaceOccupant(tool, o, &replaced)` (network-manager.md §7.4), which may put
      a plain piece back. Corrected 2026-10-15 (re-specs15b, NT1): this was "the set of cells that will be
      re-solved".
   4. If a piece was put back, insert the cell into the re-solve set, else erase it from that set;
      `AsOccupant().Shutdown()` (occupant slot 4); Release.
6. **Rebuild the neighbours:**
   1. `SolveNetwork(−1, true)` over the re-solve set.
   2. `InsertOccupantsForSolvedCells` (§6.2). The re-inserted neighbours carry `data4 = +0x208 = 1`.
   3. `MarkOccupantsUsable(list, true)` (§6.3).
7. Restore the real byte.

Listener order for one removed road piece next to one surviving piece:

1. `0xAA7A014D` (synchronous);
2. `0x99EF1143` for the removed piece;
3. `0x99EF1143` for the old neighbour (replaced in §6.4);
4. `0x99EF1142` for the rebuilt neighbour, data4 1;
5. later, `0x89D3E9F3` data1 = 1 for it.

This step has no cost, and no budget call.

---

## 11. UpdateUndergroundOccupantTerrainHeights(cells, list, type) (underground tools, `0x006588D0`)

Called by the network manager's underground refit after a terrain edit (network-manager.md §7.6). Tool 4 handles the
pipes, then tool 7 handles the subways, each between `Init` and `Shutdown`.

Arguments:

- `cells`: a hash set passed **by value** (12 bytes, copied by the caller with `0x007CED60`, freed by the callee);
- `list`: the AddRef'd occupants of that kind;
- `type`: 4 or 7.

The underground occupant's extra slots used here are counted on its network-occupant interface, past the
cISC4NetworkOccupant slots:

| Slot | Address | Use |
|---|---|---|
| 84 | `0x0065AE30` | set the 4 vertex heights and rebuild |
| 86 | `0x0065ADD0` | rebuild the model |
| 94 | `0x00659DD0` | get the 4 stored terrain heights |
| 95 | `0x0065B6B0` | set the 4 stored terrain heights and rebuild |

### 11.1 Steps

1. Clear the world cache, the solved-cell set `+0xA4` and the solved-cell vector `+0x98`.
2. For each occupant `n` in `list`, in list order:
   1. Read its stored terrain heights (slot 94), its cell (slot 59) and the terrain's current corner altitudes
      (`GetCellCornerAltitudes`, terrain slot 17).
   2. If all four are **equal** (float compare), skip `n`: the terrain under it did not change.
   3. Otherwise make a cell record:
      - the occupant's vertex heights (slot 34) and the type's edge mask (slot 38 with the type);
      - recomputed edges;
      - for each corner, the allowed range is from **−500.0** up to `terrain corner − depth` (`+0x36C`), with a
        target of `terrain corner − depth`.
   4. Put the record into the world cache and the solved-cell set and vector.
3. Step counts: dragged := 2, valid := 2, straight := 1. Then `SmoothenNetwork(2)` (`0x00637A80`).
4. **Smoothing succeeded.** For each cell in the solved-cell set (hash order), with `FindUndergroundOccupant(x, z,
   class)` (`0x006584D0`; class from slot 81) finding a piece:
   1. For each corner, read the terrain altitude and the smoothed height (`cSC4VertexHtConstraintSatisfier::GetHeight`,
      `0x0065EB00`).
   2. Set the piece's vertex heights (slot 84) from the solved record and its stored terrain heights (slot 95).
   3. Move it from `list` (Release) to the **updated** list.
5. **Smoothing failed.** `margin := +0x378 + +0x370` (float add). For each solved cell whose piece is found:
   1. Read its vertex heights `h[i]` (slot 34).
   2. The piece **survives** if, for every corner, `terrain altitude ≥ h[i] + margin`. The compare is false only for
      a strict `<`; a NaN survives *[compat]*.
   3. A survivor gets its stored terrain heights updated (slot 95), but its vertex heights are **unchanged**. It moves
      from `list` to the updated list.
   4. A piece that fails goes on a destroy list (its `AsOccupant`).

   Then `networkManager.DestroyNetworkOccupants(destroyList)` (network-manager.md §7.2, which calls back into §10 with
   tool 4 or 7) and `occupantManager.ReleaseOccupantList` (slot 48).
6. For each piece in the updated list:
   1. slot 86 (true);
   2. `SetUsable(false)`;
   3. slot 50;
   4. `SetUsable(true)`;
   5. Release.

   The usable toggle posts `0x89D3E9F3` data1 = 0, then data1 = 1, so pipe and subway listeners rebuild the cell.
7. Release every remaining entry of `list` and empty it; clear the solved-cell vector and set; free the cell set.

So, after a terrain edit:

- A pipe or subway is re-dug to follow the new ground at `depth` below it.
- If that cannot be smoothed, a piece is kept where it is when the new ground still covers it by at least `margin`.
- Otherwise the piece is **destroyed**, and it is not rebuilt.

---

## 12. How the network manager's paths call in

| Network manager path (network-manager.md) | Tool slot(s) | Section here |
|---|---|---|
| §7.2 `DestroyNetworkOccupants` | `Init`, 50, `Shutdown` | §10 |
| §7.3 `QueueRemovals(false)` | through §7.2 | §10 |
| §7.4 `RemoveOrReplaceOccupant(tool, …)` | sets the tool's real byte `+0x50`; slot 72 `InsertNetworkOccupant` for the plain replacement | §6.2 |
| §7.6 terrain refit `0x6A3B0AFD` | `Init`, `UpdateUndergroundOccupantTerrainHeights`, `Shutdown` (tools 4, 7) | §11 |
| §7.7 `CreateConnectionNode` | `Init`, 8 `PlaceNetworkOccupantById(x, z, 0x100, false, type)`, `Shutdown`; slot 74 for the node height | §4 |
| §7.8 `AutoconnectLotNodes` | `Init`, 7 `PlaceNetworkOccupants(x, z, ex, ez, 0, null, true, −1)`, `Shutdown` | §3.2 |
| §9 scripted placement | 7 | §3 |

Other callers:

- **The zone developer.** It holds three private tools (zones.md §5.2) and draws streets with
  `PlaceNetworkOccupants(…, 0xC49, streetTool, real, −1)` (zones.md §7.10). It asks `OccupantExists` (slot 9,
  `0x006248D0`) against the world cache (zones.md §8.8).
- **The power simulator.** It takes the shared power-line tool (power.md).

---

## 13. Prop occupant slots used by other specs

This section lists only the slots other specs call. The prop occupant class itself — identity, object layout,
the full slot table with return channels, the spatial record, drawing and the save record — is
[prop-occupant.md](prop-occupant.md) (re-answers27).

### 13.1 cISC4PropOccupant (vtable `0x00AA4900`, compiled slot = SDK index + 3)

The interface pointer **is the object base** (`cISC4PropOccupant`'s vtable `0x00AA4900` sits at `+0x00`).
Field offsets below are from that pointer. *[corrected by re-answers27: this line previously said the
interface pointer was "the prop object `+4`". That is wrong — `+4` is the `cIGZMessageTarget2` sub-object
(vtable `0x00AA48F0`, `DoMessage` `0x005ECFB0`), and `QueryInterface` `0x005EC960` returns `this` unchanged
for IID `0xE9793A65`. Three measured instructions address the fields from `ECX` = the object base:
`GetPropType` `0x005ED280` reads `word [ecx+0x5C]`, `GetRandomChance` `0x005ED570` reads `byte [ecx+0x62]`,
`GetState` `0x005ED560` reads `byte [ecx+0x63]`. The offsets in the table below were right all along.]*

| Slot | Address | Name | Behaviour |
|---|---|---|---|
| 3 | `0x006C2040` | `AsOccupant` | `this + 8` (null stays null) |
| 7 | `0x005ED280` | `GetPropType` | the prop type ID from a global table (`0x00B231A4`) indexed by the u16 at `+0x5C` |
| 11 | `0x005ED290` | `SetState(s)` | see below |
| 12 | `0x005ED560` | `GetState` | the byte at `+0x63` |
| 13 | `0x005ED400` | `SetTimeOfDayRange(a, b)` | stores each hour × 10, converted to a byte, at `+0x5E` and `+0x5F`; registers with the 24-hour clock when `a ≠ b` and both are ≥ 0 |
| 14 | `0x005ECA10` | `GetTimeOfDayRange(&a, &b)` | each byte × `0.1f` |
| 15 | `0x005EEFF0` | `SetSimulatorDateRange` | prop-developer.md §4.6 |
| 16 | `0x005EE230` | `TickSimulatorDate(date)` | prop-developer.md §3.3 |
| 17 | `0x007C2740` | `SetRandomChance(c)` | the byte at `+0x62` |
| 18 | `0x005ED570` | `GetRandomChance` | the byte at `+0x62` |
| 20 | `0x005ED590` | `GetPropTimerState(mask)` | `(+0x60 & mask) ≠ 0` |
| 21 | `0x005ECA50` | `SetPropTimerState(mask, on)` | see below |

**SetState(s)** (slot 11):

1. If `s` equals the byte at `+0x63`: nothing.
2. Otherwise store `s`.
3. If the occupant is in the city: post a `0xE9C86DDB` message carrying the prop and `s`.
4. If the prop's exemplar has property `0x4A70D491`: switch the state of the model instances (QI `0xA818EDD8`) to `s`
   *[inferred: the model-state call]*.

**SetPropTimerState(mask, on)** (slot 21):

1. Set or clear `mask` in the timer bits `+0x60`.
2. Check whether the low four bits went from "not all set" to "all set" (`0xF`), or the reverse.
3. Becoming **all set**: draw `r := RandomUint32Uniform(100)` from the **prop occupant's generator** (`0x00B231B0`,
   foundations §6). If `r < chance` (`+0x62`): `SetState(0)`; otherwise nothing.
4. Becoming **not all set**: `SetState(1)`.

That draw is the only random draw here. It happens only on the transition. For a timed prop, prop-developer.md §4.6
sets state 1 and `SetRandomChance(c)`. State 1 is "off" for these props *[inferred]*.

### 13.2 cISC4PropOccupantVisualEffect (class and IID `0x2977AA49`): answers NM5

The visual-effect prop's QueryInterface (`0x005ECC10`) returns the object base for IIDs `0x09820792` and `0x2977AA49`.
Its main vtable (offset 0) is `0x00AA4ED8`. The compiled order of the two overloads is the reverse of the SDK's:

| Slot | Address | Name (SDK) | Behaviour |
|---|---|---|---|
| 3 | `0x005F8400` | `AsOccupant` | `this + 0xC` |
| 4 | `0x005FD7E0` | `AsPropOccupant` | `this + 4` |
| 5 | `0x005EF7D0` | `SetEffectType(uint32 id)` | see below |
| 6 | `0x005F0780` | `SetEffectType(char const* name)` | see below |
| 7 | `0x005EDA50` | `StartEffect(bool b)` | see below |
| 8 | `0x005EDAA0` | `StopEffect(bool b)` | see below |

**SetEffectType(uint32 id)** (slot 5): if `id` differs from the stored ID at `+0x84`:

1. Clear the stored name (`+0x74`/`+0x78`).
2. Store `id`.
3. Zero `+0x88` and `+0x8C`.
4. Restart the effect (`0x005EE9C0`).

**SetEffectType(char const* name)** (slot 6): if `name` differs from the stored name, set the stored ID to 0, store
`name`, and restart.

**StartEffect(b)** (slot 7):

- With an effect object at `+0x6C`:
  1. If the occupant's flag 2 is set (the occupant flags byte), call the effect's slot 3 with `b = 0`.
  2. `started := 1` (`+0x90`).
  3. Return true.
- With no effect object: `started := 1`; return false.

**StopEffect(b)** (slot 8): if there is an effect object and `started` is set:

1. If flag 2 is set, call the effect's slot 4 with `b = 0`.
2. `started := 0`.
3. Return true.

Otherwise return false.

The constructor (`0x005F0210`) sets `+0x91 := 1`. When occupant flag 2 changes (`SetAllFlags`, `0x005ED9D0`), the
prop starts or stops its effect.

So network-manager.md §8.8's pothole calls are:

- slot 6 = `SetEffectType(name)`;
- slot 7 with 1 = `StartEffect(true)`;
- slot 8 with 1 = `StopEffect(true)`.

**NM5 is confirmed** (MATCHES).

---

## 14. Interface slot table: cISC4NetworkTool (base vtable `0x00AA8748`)

| Slot | Offset | Address | Method | Pop | Return |
|---|---|---|---|---|---|
| 3 | +0x0C | `0x006269C0` | `Init` | 0 | AL |
| 4 | +0x10 | `0x00626D30` | `Shutdown` | 0 | AL |
| 5 | +0x14 | `0x00626F90` | `Reset` | 0 | — |
| 7 | +0x1C | `0x00629210` | `PlaceNetworkOccupants(x1, z1, x2, z2, mask, callback, real, type)` | 32 | AL |
| 8 | +0x20 | `0x00624FD0` | `PlaceNetworkOccupantById(x, z, piece, usable, type)` | 20 | — |
| 9 | +0x24 | `0x006248D0` | `OccupantExists(x, z, mask, &id)` (zones.md §8.8) | 16 | AL |
| 13 | +0x34 | `0x0062C8E0` | `ClearFailureReason` | 0 | — |
| 14 | +0x38 | `0x0062C8F0` | `GetFailureState` (`+0x20C != 0`) | 0 | EAX = 0 or 1 |
| 18 | +0x48 | `0x0063AF40` | `DrawNetworkLine(&start, &end, snap, type)` (network-geometry.md §3) | 16 | AL |
| 19 | +0x4C | `0x0063BDC0` | `PlaceAllSegments(commit, preview, neighbour, type)` | 16 | — |
| 20 | +0x50 | `0x00625640` | `GetCostOfSolution` | 0 | EAX |
| 21 | +0x54 | `0x0062C8D0` | `EnablePreviewUpdate(bool)` (`+0x234`) | 4 | — |
| 22 | +0x58 | `0x0062CA20` | `SetAnchorPoints(&points)` | 4 | — |
| 23 | +0x5C | `0x0062CA30` | `GetDraggedCells(&out)`; replaces caller's coordinate vector (§2.1) | 4 | — |
| 25 | +0x64 | `0x00407AB0` | valid step count (`+0x78`) | 0 | EAX |
| 26 | +0x68 | `0x00407AC0` | straight step count (`+0x74`) | 0 | EAX |
| 27 | +0x6C | `0x00809240` | total dragged step count (`+0x70`) | 0 | EAX |
| 50 | +0xC8 | `0x0062A5C0` | `DestroyOccupantsAndResolve(&list)` | 4 | — |
| 52 | +0xD0 | `0x0062BF60` | `GetFailureReasonString` (class method, 2 dword arguments) | 8 | — |
| 53 | +0xD4 | `0x0062C980` | `DoNetworkSpecificPlacement(arg3, neighbourMode)`; base returns false | 8 | AL |
| 55 | +0xDC | `0x006258C0` | `InsertOccupantsForSolvedCells(&solved, &set, previewOnly)` | 12 | — |
| 59 | +0xEC | `0x006299A0` | `UpdateOccupantHighlights` (1 dword argument) | 4 | AL |
| 72 | +0x120 | `0x006278A0` | `InsertNetworkOccupant(&cell, &request, &out)` | 12 | — |
| 74 | +0x128 | `0x0062C990` | placement height for a terrain altitude (float in, ST0 out); the base returns the input | 4 | ST0 |

Slots 50 and up are class methods past the SDK's interface. They are needed only by a replacement that keeps the
original's layout *[choice]*. The pops agree with docs/abi/interfaces.tsv.

---

## 15. Quirks (summary)

1. A **diagonal** `PlaceNetworkOccupants` with a non-zero mask places nothing and returns true (§3.3).
2. Code placement is **free** (§3.4, §8).
3. The UI's withdrawal **zero-extends** the cost, so a negative cost fails (§8.2).
4. Money is not refunded if the real pass fails after a successful withdrawal (§8.2).
5. `MarkOccupantsUsable` runs **unconditionally** in real mode, so even crew-built pieces are usable at once (§6.3).
6. In real mode the world cache is cleared at the start, even when the run then fails (§5.2).
7. A road through a power-line cell demolishes the **pole** in that cell, not the line (§6.4).
8. The extension test in §3.3 uses the **unclamped** deltas, so a drag that starts outside the city still counts as
   straight.
9. In the underground refit's fallback, a covered piece keeps its old vertex heights, while its stored terrain heights
   are updated (§11.1 step 5). The next refit compares against the new ground.
10. The connection-node refresh in §6.2 re-inserts with data4 **0**, not `+0x208`.

---

## 16. Answers given here

- **NT36 (Tempelhof single-cell draft):** §8.1.5 and network-geometry §§9.1/9.8.1 give the plane, retry,
  ranges/preferences, equality work sets and pending stack, retained corners and 11-cost fixture, with controls.
- **NT35:** §8.1.4 defines the observation limit for retained live piece/plane/support inputs.
- **NT36:** §8.3.3 defines occupied-cell callable observations, reference ownership and subclass/model
  host responsibilities, with seven boundary vectors; observation completion is separate from placement.
- **NT27:** §2.1 and §14 define the public copy/count contract. No public solved-record getter exists;
  drag/status/cost comparisons cannot certify solved pieces, types or orientation.
- **NT28:** §3.5 specifies staircase rows and increasing width order; §5.3 corrects slot 14 to a Boolean.
- **NT29:** §8.1.1 defines retained solved-entry/corner inputs, live terrain, intersection cost exclusion and
  repeated-placement pricing. Tests 20–30 below cover these three answers.
- **NT30:** §6.5 and §8.1.2 define the two corner refresh populations and complete controlled per-entry fixtures;
  they do not present public city totals as recovered native corner snapshots.
- **NT31:** §8.3 defines shipped tuning/bridge selection inputs, recording-host requirements and separate
  placement/retained-price fixtures; absent subclass resources remain recorded differences.
- **NM5** (network-manager.md §15): confirmed, §13.2. MATCHES.
- **zones.md §14.3 Q1(a)**, the street tool's `PlaceNetworkOccupants`:
  - when it fails: §5.1, §5.3;
  - what it costs: nothing (§8);
  - whether it draws random numbers: no (§3.4);
  - whether it inserts occupants before returning: yes, with data4 3 (§6.2);
  - whether it auto-connects: by re-solving neighbours and by one-cell joins, but never regionally (§7).
- **NM1, NM2** (network-manager.md §15) and **OM2** (occupant-manager.md §16) are answered in those specs from the
  occupant base record (see the occupant-manager edits of this lane).

## 17. Open questions

- ~~**NT1.** In `DestroyOccupantsAndResolve` step 5.3, which cell set decides between a plain `RemoveOccupant` and
  `RemoveOrReplaceOccupant`?~~ **ANSWERED 2026-10-15 (re-specs15b):** the set of destroyed cells, not the re-solve
  set (network-geometry.md §13). DIFFERS; §10 step 5.3 is corrected.
- ~~**NT2.** The source of the per-type cell cost table `0x00B453A4` and of `+0x320`, `+0x324` and `+0x328`.~~
  **ANSWERED 2026-10-15 (re-specs15b):** the placement exemplars (type `0x6534284A`, group `0x084344E0`), properties
  `0x4A2D49B9`, `0x8A2D49EA`, `0x8A2D49F0` and `0x6A2D49F4` (network-geometry.md §2.4, §11). MATCHES.
- ~~**NT3.** The power-line tool's `GetCostOfSolution` (`0x0064F0F0`) and the exact rule for result 3 (road) in
  `CheckForReservedTiles`.~~ **ANSWERED 2026-10-15 (re-specs15b):** network-geometry.md §11. The result-3 rule
  MATCHES, with an uninitialised first "previous result" *[quirk]*.
- ~~**NT4.** What slots 86 and 50 of the underground occupant rebuild (§11.1 step 6).~~ **ANSWERED 2026-10-15
  (re-specs15b):** slot 50 releases the cached path info; slot 86 is `CreateGeometry(bool)`, which rebuilds the model
  at the new heights (network-geometry.md §12).
- ~~**NT5.** The internals of the validation passes (§5.1 step 4) and of `SolveNetwork`.~~ **ANSWERED 2026-10-15
  (re-specs15b):** network-geometry.md. The piece rules and the bridge internals stay open there (NG2, NG5).

---

## 18. Conformance tests

A "city" is a fresh flat 64 × 64 city with funds 100 000 §, unless stated. Road = type 0.

1. **Free code placement** (§3.4). `PlaceNetworkOccupants(10, 10, 20, 10, 0, null, true, −1)` on the road tool →
   true. Funds are still 100 000. Eleven road occupants are in cells (10..20, 10), each inserted with data4 3, each
   with flag `0x10000000`.
2. **Insert before return** (§3.4, §6.2). In test 1, every `0x99EF1142` (data4 3) is delivered before the call
   returns. Every `0x89D3E9F3` (data1 1) is delivered after it.
3. **data4 restored** (§3.4). After test 1, `+0x208 = 1`. A later `DestroyOccupantsAndResolve` re-inserts neighbours
   with data4 1.
4. **Diagonal quirk** (§3.3). `PlaceNetworkOccupants(5, 5, 8, 9, 0xC49, null, true, 3)` → true, nothing placed, no
   message.
5. **Diagonal without a mask** (§3.2). The same call with `mask = 0` is **not** short-circuited. It goes through
   `DrawNetworkLine` and `PlaceNetwork`, and the result is theirs. The drag's layout is network-geometry.md §3.
6. **Extension** (§3.3). A street exists at (10, 10). `PlaceNetworkOccupants(11, 10, 14, 10, 0xC49, null, true, 3)`
   → the run is drawn from (10, 10) to (14, 10), and the piece at (10, 10) is re-solved with an east edge.
7. **Join callback** (§3.3). A road (type 0, bit `0x1` in `0xC49`) at (10, 10); the street tool draws (11, 10)–(14,
   10) with `callback = roadTool`. The street run covers (11..14, 10). Then
   `roadTool.PlaceNetworkOccupants(10, 10, 11, 10, 0, null, true, −1)` is called once.
8. **Failure leaves nothing** (§5.2). A road run across a cell with a building that may not be demolished → false;
   internal reason `0xF0000002`, public `GetFailureState() = 1`; no occupant inserted; no money withdrawn.
9. **UI pricing** (§8.2). A player drag priced 150 with funds 100 → `WithdrawFunds(150)` fails; nothing is built;
   funds stay 100. Priced 150 with funds 1 000 → funds 850, then the road is built.
10. **Negative price** (§8.2). If `GetCostOfSolution` returns −1, `WithdrawFunds` receives `0x00000000FFFFFFFF` and
    fails (budget.md §6).
11. **Earthwork rounding** (§8.1). Corner deltas summing to 2.5 with `+0x328 = 1.0` → 2, and summing to 3.5 → 4
    (half to even).
12. **Power pole** (§6.4). A power line with a pole at (20, 20); a road drawn through (20, 20) → the pole is
    demolished through `DemolishOccupant(true, pole, 1, 0x2001, …)`, while the road piece is inserted. A pipe drawn
    through (20, 20) leaves the pole.
13. **Destroy and resolve** (§10). Roads (10..12, 10). Destroy (11, 10). Messages, in order:
    1. `0xAA7A014D` for (11, 10);
    2. `0x99EF1143` for (11, 10);
    3. `0x99EF1143` for the old (10, 10) and (12, 10), each followed by its rebuilt `0x99EF1142` (data4 1). The order
       between the two cells follows the solved-cell order.

    Funds unchanged.
14. **Underground refit, smooth** (§11). A pipe at (5, 5) with depth `d`; raise the terrain at all four corners by 10
    → the pipe's vertex heights are the new corners − `d`. The stored terrain heights are the new corners. One
    `0x89D3E9F3` data1 = 0, then one with data1 = 1.
15. **Underground refit, unchanged** (§11.1 step 2.2). A terrain message whose rect covers the pipe but leaves its
    corners unchanged → the pipe is skipped and no message is posted.
16. **Underground fallback** (§11.1 step 5). Smoothing fails. A pipe with vertex heights `h = (−20, −20, −20, −20)`,
    `margin = 5`:
    - new ground at −15 everywhere → kept, `h` unchanged, stored terrain = −15;
    - new ground at −16 at one corner → destroyed through `DestroyNetworkOccupants`.
17. **Visual-effect slots** (§13.2). On a visual-effect prop whose occupant flag 2 is set and which has an effect
    object:
    - `StartEffect(true)` → the effect's start is called with 0, `+0x90 = 1`, true;
    - `StopEffect(true)` → the effect's stop is called with 0, `+0x90 = 0`, true;
    - `StopEffect(true)` again → false.
18. **Timer chance** (§13.1). Chance 30. Setting the last of the four timer bits with the generator's next
    `RandomUint32Uniform(100)` = 29 → `SetState(0)`; with 30 → state unchanged. Clearing any bit afterwards →
    `SetState(1)`.
19. **PlaceNetworkOccupantById** (§4). `(7, 9, 0x100, false, 0)` → one occupant at (7, 9) with flag `0x4000` still
    set (not usable), inserted with data4 1. With `usable = true`, it is usable and `0x4000` is clear.
20. **Public copy survives the next drag** (NT27). Real, mask-zero road (4,4)–(14,4), successful on flat empty
    terrain: copy slot 23 into an output vector initially holding `(99,99)`. It now holds exactly
    `(4,4),(5,4),…,(14,4)`; slots 25/26/27 return 11/11/11. Draw (30,30)–(32,30) on the same tool. The old caller
    copy still has eleven entries; another slot-23 call replaces it with the three new cells. No occupant
    references are acquired or released by either coordinate copy.
21. **Seventeen cells, nine steps** (NT28). Road, mask=0, snap=false, (20,4)–(28,12), with no adjacent networks:
    slot 23 is `(20,4), (20,5),(21,5), (21,6),(22,6), …, (27,12),(28,12)` (17 cells). Its step intervals are
    `[0,0],[1,2],[3,4],…,[15,16]`; total=9, straight=0, and valid=9 on successful placement. On flat empty land
    with the shipped road cell cost, zero demolition and zero earthwork, cost=170. The turn cells are ordinary
    drag cells; the extra-cell vector is empty.
22. **Shared-endpoint diagonal footprint** (NT28). The unaligned draw (14,4)–(18,8) gives
    `(14,4),(14,5),(15,5),(15,6),(16,6),(16,7),(17,7),(17,8),(18,8)`: nine cells, five steps, straight=0.
    With no joins/re-solved neighbours and otherwise the pricing conditions of test 21, cost=90. Existing
    endpoint networks may alter the solve, but do not justify reducing the base footprint to five cells.
23. **Width ordering in both directions** (NT28). Two-wide avenue, no existing networks, horizontal
    (4,28)–(16,28): slot 23 is `(4,27),(4,28),(5,27),(5,28),…,(16,27),(16,28)`; 26 cells, 13 total and straight
    steps. Reverse endpoints: `(16,27),(16,28),…,(4,27),(4,28)`. Vertical (4,28)–(4,30):
    `(3,28),(4,28),(3,29),(4,29),(3,30),(4,30)`. These are draw results regardless of a later validation refusal.
24. **Boolean failure and retained footprint** (NT28). For a nonempty 13-step draw whose validation pass cuts
    valid steps and sets reason `0xF0000002`, default stop-on-failure gives slot-7 AL=false, slot-14 EAX=1,
    slot-25=0, slot-27=13. Slot 23 still returns the full drawn footprint. A refusal with reason `0xF0000006`
    also exposes EAX=1, not that reason code. `ClearFailureReason` then gives EAX=0 without changing the copied
    footprint. This fixture distinguishes the empty-drag early success from a refused nonempty drag.
25. **Post-commit height source** (NT29; abstract pricing fixture). One retained solved run cell, mask road,
    intersection marker clear, retained network corners `(4,4,4,4)`, live terrain at its four vertex IDs
    `(4,4,4,4)`, cached old terrain `(0,0,0,0)`, earthwork factor=1, cell cost=10, all other charges zero:
    cost=10. Changing just the live terrain to `(3,3,3,3)` gives cost=14 on the next getter call. Neither call
    changes the retained network heights or starts another solve.
26. **Intersection marker versus placed state** (NT29; same pricing fixture). Set placed byte `+0x51=1`:
    cost remains 14. Set intersection marker `+0x52=1`: cost=4, since only the cell charge is excluded.
    Clearing run membership also excludes the cell charge but retains earthwork=4. Already existing ordinary
    road occupants are not an independent cost exemption.
27. **Inventory and shared corners** (NT29; abstract pricing fixture). Two adjacent retained solved cells,
    neither a run cell, all eight retained corner values 1, corresponding live terrain all zero, factor=1:
    cost=8, including the shared edge's two vertex IDs twice. Add an unrelated cached or extra cell that is
    absent from the solved vector: cost stays 8. Mark one solved cell as part of the run, mask road+street,
    with costs 10 and 5: cost=18, using the maximum 10 once rather than their sum.
28. **Repeat road still has a cell charge** (NT29). Repeat test 20's eleven-cell flat road with unchanged
    terrain and no special intersection marker. When the solved inventory remains those ordinary run cells,
    cost is again 110. The new call may have zero earthwork; the existing occupants do not make cell cost zero.
29. **Post-commit neighbour is priced** (NT29; abstract pricing fixture). A solved non-run neighbour appended
    during commit has four retained network corners 2 above its live terrain. With factor=1 it adds 8 earthwork
    and zero cell cost, even though it is absent from slot 23. Merely caching that neighbour without adding a
    solved entry contributes zero.
30. **Power-line and bridge observation boundary** (NT27). After a successful subclass placement, copy/count
    getters still report the current drag, but their coordinate pairs contain no pole, wire or bridge piece ID.
    Reading them cannot turn an unwired generic adapter into a comparison of those pieces. Use the subclass
    cost and placement contracts; retain a recorded mismatch until the adapters and resources agree.
31. **NT30 coordinate/corner fixtures.** Expand each row of §8.1.2 with stride 129 and the specified four
    corners. Require prices 110,170,90,55,110, including the full staircase inventories and repeated vertices.
32. **NT30 refresh already solved.** The stale-corner variant in §8.1.2 changes 114 to 110 without appending a
    solved entry. Placed=false does not prevent the first pass's corner refresh.
33. **NT30 incident neighbour and missing height.** The `(3,4)` variant appends exactly once, retains 104 at
    the missing network vertex and costs 114 at factor=1. A cached-but-not-admitted neighbour contributes zero.
34. **NT30 fallback distinction.** A smoothing cell with an absent network height uses current terrain at that
    vertex; a terrain-neighbour-only cell leaves its old corner unchanged. For connection nodes only the first
    pass adds 100000 to the lookup id. Supply unequal ordinary/offset answers to detect accidental conflation.
35. **NT31 power-line scopes.** With the empty-land placement inputs of §8.3.1 require valid=13, failure=0,
    three poles and ten counted wire cells, price=65. With retained counts only, certify price only. Missing
    Utilities properties give fresh costs 0, not 15/2; missing pole host is an uncovered placement scope.
36. **NT31 power-line numeric width.** The large-count retained-price vector in §8.3.1 returns 16777217;
    count=1 pole returns 0 regardless of wire count. No base earthwork/demolition is added.
37. **NT31 bridge scopes.** The 16-step selection vector chooses index 1, or index 0 when explicitly
    preferred. The independent retained bridge record in §8.3.2 costs 900. Neither check certifies model
    construction, native city span/elevations, nor the observed city total 974.
38. **NT32 release and reset.** Complete test 35, release all generated records, then release again:
    vector length=0, retained poles=3, wire cells=10, price=65 after both releases. Start a new slot-53
    placement with dragged=1: valid=0 (empty generated vector), both cost counts=0, price=0.
39. **NT33 renderer and lot boundaries.** Use §8.3.1.1's three ray outcomes; verify the two zero words,
    12-byte hit output and AL result. Distinguish a failed renderer acquisition from a callable renderer's
    false result. In an otherwise clear cell, lot lookup null gives classifier 0, non-null gives 2;
    neither borrowed lookup result causes a Release. Earlier zone/occupant classification skips this probe.
40. **NT33 fresh prefix.** The empty-land axis fixture (4,32)–(16,32) supplies prefix=13 and dragged=13.
    Every step chooses its first cell as pivot, and no prefix-transition forced point exists.
41. **NT34 nonflat stages.** Run §8.1.3 A/B/C: identical nonflat network solutions and refreshed corners;
    terrain solutions and earthwork contributions respectively (t,12), (n−1,4), (n,0).
    B with raise=0 fails support before commit; fixed-and-placed bypasses support bounds and keeps t.
