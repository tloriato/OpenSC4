# Spec: the regional city (`cSC4RegionalCity`)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`). Implementers must work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled
or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[compat-limit]* depends on something FreeCity4 cannot reproduce, so only the rule is matched.

Related specs: [foundations.md](foundations.md) §11 (the regional-city record, its read rules, the two deal lists,
the messages and TerrainReconciliation; this spec does not repeat them), [neighbors.md](neighbors.md) (§2.2 the cell
conversions, §4 connections, §5 deals, §8.2 `UpdateCommodityStats`, and the compiled slot notes at its end),
[demand.md](demand.md) (`CalculateJobsPerUnitOfDemand`, `GetLocalPopulationSummary`), [budget.md](budget.md)
(`GetTaxRate`), [aura.md](aura.md) (`GetMayorRating`).

## 0. Summary

One regional city object stands for each city tile of the region: its place and size, the statistics the region
view shows (population, jobs, mayor rating, tax rates, budget, the commodity balances), its names, its connections
to the neighbours and the deals that cross them. The current city's object is refreshed from the live city at
PostCityInit and PreSave; the others come from their city files.

## 1. Identity *[compat]*

- Class ID `0x6A037C24` (`GetGZCLSID`, `0x004ABA60`); factory `GZCOM_CreatecSC4RegionalCity` (`0x004AD9A0`, 0x1A0
  bytes, registered by the COM director at `0x00460399`). `SC4ReadRegionalCity`'s fallback (`0x005DC0EF`) and
  the region screen (`0x007AEAD4`) construct it directly (constructor `0x004AC040`).
- **`SC4ReadRegionalCity(package, &out)`** (`0x005DC070`): opens the record `0xCA027EDB / 0xCA027EE1 / 0` of the
  package (auto-load stream); without it, false. The object comes from `GZCOM()` (`CreateInstance(0x6A037C24,
  0x69D2BE1D)`), or is constructed directly when that fails; its serializable view reads the stream; on success
  `out` := the object (a reference added) and its `Init` runs. The result is the read's.
- Three views: `+0x00` the regional-city interface (vtable `0x00A8FB20`, 74 slots), `+0x04` the message target
  (`0x00A8FB10`, `DoMessage` `0x004ABD40`), `+0x08` the serializable (`0x00A8FAF8`: `Write` `0x004A7D10`, `Read`
  `0x004AC5B0`).
- **`QueryInterface`** (`0x004A74E0`): 1 and `0x69D2BE1D` → the object; `0x452294AA` → `+4`; `0xE4FDA3D4` → `+8`;
  each adds a reference; other IIDs false.
- **`Release`** (`0x004A7800`): a count above 1 is decremented and returned; otherwise `Shutdown` (slot 4) runs, then
  the deleting destructor (slot 73, `0x004AC590`; body `0x004AB7F0`), and 0 is returned.

## 2. Fields

| Offset | Field | Made as |
|---|---|---|
| `+0x0C` | initialised (byte) | 0 |
| `+0x10` | reference count | 0 |
| `+0x14`, `+0x18` | position x, z (region cells) | 0, 0 |
| `+0x1C`, `+0x20` | size x, z (region cells) | 1, 1 |
| `+0x24`, `+0x28`, `+0x2C` | population, commercial jobs, industrial jobs | 0 |
| `+0x30` | workforce fraction (f32) | 0.5 |
| `+0x34` | mayor rating (s8) | 0 |
| `+0x38` | difficulty | the application's default (`0x00B43CD8` `+0x46C`) |
| `+0x3C`, `+0x48`, `+0x54` | population, extrapolated and potential population maps (type → s32) | empty |
| `+0x60`–`+0x8C` | the 12 tax rates (f32, §4.2) | 7.0 each |
| `+0x90` | connection list | empty |
| `+0x94` | pending connection changes (16-byte records, §6.2) | empty |
| `+0xA0` | moved (byte) | 0 |
| `+0xA4` | serial number | 0 |
| `+0xA8`–`+0xB4` | original language, country; last language, country | 0 |
| `+0xB8` | birth date | 0 |
| `+0xBC` | save-file path (string) | empty |
| `+0xD0`, `+0xF8`, `+0x120`, `+0x154` | city name, mayor name, description, utility advisor name | empty |
| `+0xE4`, `+0x10C`, `+0x134` | the record's three unknown strings | empty |
| `+0x148`, `+0x14C`, `+0x150` | world longitude, latitude, base elevation (f32) | 0 |
| `+0x168` | established (byte) | 0 |
| `+0x16C` | tutorial ID | −1 |
| `+0x170`, `+0x178` | budget, income (f64) | 0 |
| `+0x180` | commodity map (commodity → five f64, §5) | commodities 1, 2, 3 at 0 |
| `+0x18C`, `+0x190` | the deal lists O and L (foundations.md §11.3) | empty |
| `+0x194` | points of interest (type → list of three-float positions) | empty |

The record (foundations.md §11.2) stores these fields; its "moved" byte is `+0xA0`.

## 3. Slots *[compat]*

The compiled order is the SDK's, except where neighbors.md's slot notes say otherwise (overloads grouped).

- 3, 4: `Init`, `Shutdown` (§4.1; `0x004A7830`, `0x004A8470`).
- 5–8: `GetPosition` (`0x004ABA70`), `SetPosition` (§6.1, `0x004A99F0`), `GetCitySize` (`0x004ABA90`),
  `SetCitySize` (`0x004ABAB0`).
- 9, 10: `GetPopulation(type)` (`0x004A8500`), `GetPopulation()` (`0x006BDD80`).
- 11–15: `GetCommercialJobs`, `GetIndustrialJobs`, `GetWorkforcePercentage`, `GetMayorRating`,
  `GetDifficultyLevel` (getters shared with other classes).
- 16: `GetTaxRate(type)` (§4.2, `0x004A7580`).
- 17–19: `GetExtrapolatedPopulation`, `GetAllowableExtrapolation`, `ExtrapolateGrowth` (§4.3; `0x004A8530`,
  `0x004A9820`, `0x004A9860`).
- 20–22: `FindConnection`, `GetAllConnections`, `ChangeSymmetricConnection` (§6.2; `0x004A7B40`, `0x004A8DB0`,
  `0x004A8DF0`).
- 23–26: `SetupPreferences` (`0x004A78A0`), `UpdateCityCache(city)` (`0x004AD290`), `UpdateCityCache(preferences)`
  (`0x004AA4F0`), `SetupCity` (`0x004A8960`) (§7).
- 27–30: the serial number's getter and setter (`0x006F6F70`, `0x004ABAE0`); the original and the last language and
  country (`0x004ABAF0`, `0x004ABB10`).
- 31–40: getters and setters of the save-file path, the city name, the mayor name, the utility advisor name and the
  description (`0x004ABB30`–`0x004ABC10`).
- 41–49: the birth date, the established flag, the world position and base elevation, `GetWorldHemisphere`
  (`0x004ABC20`–`0x004ABCC0`).
- 50–53: `GetBudget`, `SetBudget`, `GetIncome`, `SetIncome`, doubles (`0x004A7640`–`0x004A7670`).
- 54–63: getters and setters of exported, imported, produced, demanded and cost per unit (§5;
  `0x004A9EE0`–`0x004AA080`).
- 64: `GetCommodityBalance` (§5, `0x004A7680`).
- 65–67: `GetTutorialGUID`, `SetTutorialGUID`, `IsTutorial` (`0x004ABCE0`, `0x004ABCF0`, `0x004A76E0`).
- 68–71: `UpdateLocalDeals`, `SetLocalDeals`, `GetLocalDeals`, `UpdateImportExport` (foundations.md §11.3;
  `0x004A9110`, `0x004A90D0`, `0x004AA0C0`, `0x004AA200`).
- 72: `GetPointsOfInterest(type, list)` (§7.3, `0x004A8A40`).
- 73: the deleting destructor (`0x004AC590`).

Notes: the string getters copy into the caller's string (its `operator=`), the setters copy from it; `IsTutorial` is
`GetTutorialGUID() ≠ −1`; `GetWorldHemisphere` is 1 when the latitude is above 0, otherwise 2 (also for a NaN)
*[inferred: 1 northern, 2 southern]*; `GetOriginalLanguageAndCountry` and `GetLastLanguageAndCountry` return true.

## 4. Life and statistics *[compat]*

### 4.1 Init and Shutdown

- **`Init`** (once): subscribes the message view to PostLoad `0x26C63342`, PreSave `0x26C63343`, PostCityInit
  `0x26D31EC1`, PostCityShutdown `0x26D31EC3` and TerrainReconciliation `0xCA63D517` (foundations.md §11.3). True.
- **`Shutdown`** (once Init has run): the five subscriptions are removed, and the connection list and both deal
  lists are emptied (their references released). True.

### 4.2 Tax rates

`GetTaxRate(type)` maps the twelve tax groups to `+0x60`–`+0x8C` in this order: `0x1010`, `0x1020`, `0x1030`,
`0x3110`, `0x3120`, `0x3130`, `0x3320`, `0x3330`, `0x4100`, `0x4200`, `0x4300`, `0x4400`. Any other type returns
7.0. `UpdateCityCache(city)` refreshes them by index 0–11 from the budget simulator's `GetTaxRate` (budget.md).

### 4.3 Population maps

- `GetPopulation(type)`: the population map's value, 0 when absent. `GetExtrapolatedPopulation(type)`: the
  extrapolated map's value; when absent, `GetPopulation(type)`.
- `GetAllowableExtrapolation(type)`: `max(potential[type] − extrapolated[type], 0)`.
- `ExtrapolateGrowth(type, add)`: `extrapolated[type] := trunc(float(extrapolated[type]) + add)`.
- These two read both maps with insertion: a missing type gets a 0 entry, which the record then saves; so
  `ExtrapolateGrowth` on a type without an extrapolated entry starts from 0, not from the population *[quirk]*.
- **PreSave** on the current city's record (foundations.md §11.3) runs `UpdateCityCache(city)`, then:
  - **potential**: for each type `t` of the population map with `p := population[t]`, `potential[t] := 0` when
    `p ≤ 0`, else `trunc(max(1.1 · p, 1000))` (float, `0x004A9890`; a product below 1000 gives 1000);
  - **extrapolated**: `extrapolated[t] := population[t]` for every type of the population map (`0x004A9970`).

## 5. Commodities *[compat]*

The commodity map holds, per commodity (1 power, 2 water, 3 garbage, neighbors.md §3), five doubles: produced,
demanded, exported, imported, cost per unit. The getters and setters of slots 54–63 work on the entry of the given
commodity, **creating a zero entry** for an unknown one *[quirk]*; every setter stores 0 for a negative value
(a NaN is stored).

`GetCommodityBalance(c)` (slot 64) is `(produced + imported) − (demanded + exported)`, computed in double precision
through the slots (produced, then imported, then demanded, then exported).

`UpdateImportExport` and the deal lists are foundations.md §11.3; neighbors.md §8.2 sets budget, income and the
commodities through these setters.

## 6. Position and connections *[compat]*

### 6.1 Moving

`SetPosition(x, z, rearrange)`: the position is stored. With `rearrange`:

1. `moved := 1`;
2. every own connection is repaired with this city (connection slot 10, `Repair`, neighbors.md §4.2), so its other
   region cell follows the move (`0x004A9720`);
3. with `(dx, dz)` the change of position, every deal of O and then of L moves **both** its region cells, the city
   side (deal slots 4 and 3) and the other side (deal slots 6 and 5), by `(dx, dz)` *[quirk]*.

The TerrainReconciliation that follows a load of the moved city rebuilds the connections (§6.2) and clears `moved`.

### 6.2 Connections

- **`FindConnection(type, x, z)`**: the first connection of the list with that type (connection slot 5) and cell
  (slot 6), without a reference; null otherwise. **`GetAllConnections(list)`** empties `list` and copies the
  connection list into it.
- **Pending changes.** Each record is `{type, x, z, flag}` (u32, s32, s32, u8; flag 1 to create, 0 to remove
  *[inferred]*), saved with the record (foundations.md §11.2). They are added by:
  - `ChangeSymmetricConnection(conn, flag)`: the connection's cell turned into the neighbour's cell
    (`ConvertConnectionCellToNeighborCell`, neighbors.md §2.2) and its type, with `flag`;
  - **dropping** (`0x004A8570`): the pending list is emptied;
  - **own connections to remove** (`MarkConnectionsForRemoval`, `0x004A9580`, after a move): every own connection
    whose cell has a city on the other side (the neighbours simulator's `FindCityOnOtherSideOfConnection`, its slot
    12) that is established (that city's slot 43) adds `{type, its cell, 0}`;
  - **incoming as new** (`TreatAllIncomingConnectionsAsNew`, `0x004A9220`): for every adjacent city, established or
    not (the neighbours simulator's `GetAdjacentCities(list, true)`, slot 20), each of its connections whose
    other-side city is this one adds `{type, this side's cell, 1}`, the cell from the inverse conversion of
    neighbors.md §2.2 (`0x006CEBA0`).
- **TerrainReconciliation** (foundations.md §11.6), on the current city's record: when the city is not
  established, the message's data3 is set or `moved` is set, the dropping, the own removals (only when moved) and
  the incoming-as-new step run in that order; then `CreateSymmetricConnections`; then `moved := 0`.
- **`CreateSymmetricConnections`** (`0x004AA6D0`) *[summary; RY1]*: duplicate pending records (same type and cell) are
  removed; the rest are sorted by edge (west `x < 0`, north `z < 0`, east, south), then x, then z (`0x004A78E0`); for
  the flagged records of types 3, 8 and 12 a running parity, flipped on the north and east edges and by the drive-side
  setting (`0x00B21CC1`), gives each a direction; each record then finds or creates its connection through the
  neighbours simulator (`FindConnection`, `CreateConnection`) and builds or removes the network, pole or power-line
  pieces at the edge cell (types `0xC772BF98`, `0x2890D4DE`, `0x0990B2EA`), demolishing and dezoning what is in the way
  (demolition utility slot 6, zone manager), and updates the edge network occupants' flags. It always ends by posting
  ConnectionsReady `0x6AC284F3` (foundations.md §11.6).

## 7. The city cache *[compat]*

### 7.1 New-city preferences

- **`SetupPreferences(prefs)`**: the city name into `prefs + 0x000` and the mayor name into `prefs + 0x200` (C
  strings), the difficulty into `prefs + 0x400`.
- **`UpdateCityCache(prefs)`**: the reverse: city name, mayor name and difficulty from `prefs`; the save-file path
  := the drive and directory of the current path, the city name and `.sc4` (`0x00A87054`).
- **`SetupCity(city)`**: the city's mayor name (city slot 17), city name (slot 13) and difficulty (slot 25) from this
  record; its save-file path (slot 11) from `SC4CreateSavePathForCity(city name, 0, &path)`; its size (slot 88) :=
  `(sizeX · 1024, sizeZ · 1024)` metres.

### 7.2 From a live city (`UpdateCityCache(city)`)

In this order: the mayor name (city slot 16), the city name (slot 12), the difficulty (slot 24), the save-file path
(slot 10); the serial number and established flag through this record's own setters (slots 28 and 44) from city
slots 5 and 22; the mayor rating from the aura simulator (city slot 51, its slot 8); the 12 tax rates (§4.2); the
neighbours simulator (city slot 62) copies its connections into the connection list (its slot 16) and runs
`UpdateCommodityStats(city)` (its slot 34, neighbors.md §8.2); when the tutorial system (slot 82) exists and the
tutorial ID is −1, the tutorial ID := its `GetTutorialGUID`; when the residential simulator (slot 68) exists, the
population := its `GetPopulation` and the workforce fraction := its `GetGlobalWorkforcePercent`.

Then the demand simulator (city slot 56) fills the population map (`GetLocalPopulationSummary`, demand.md) and the
jobs are computed: for each type `t`, `jobs(t) := trunc(population[t] · s)` with `s` the sum
`(j[3] + j[2]) + j[1]` of its `CalculateJobsPerUnitOfDemand(j, t, 0)` (the arithmetic of demand.md's
`GetJobsBySensus`); commercial jobs := the sum over `0x3111`, `0x3121`, `0x3131`, `0x3321`, `0x3331`, industrial jobs
:= the sum over `0x4101`, `0x4201`, `0x4301`, `0x4401`.

### 7.3 Points of interest

The same call empties the points of interest and lists the lots of the airport (`0x00B43D2C`, type 0) and seaport
(`0x00B43D30`, type 1) developers (their slot 8): each lot in a livable state (lot slot 37) adds its centre (lot slot
15). `GetPointsOfInterest(type, list)` copies the type's list into `list`, leaving it unchanged for an unknown type.

## 8. Quirks

1. Reading the extrapolation or a commodity creates zero entries that are saved (§4.3, §5).
2. A moved city shifts both region cells of its deals (§6.1).
3. Unknown tax types read as 7.0 (§4.2).

## 9. Open questions

See [questions/regional-city.md](questions/regional-city.md): RY1 (the per-record rules of
`CreateSymmetricConnections`), RY2 (the three unknown strings of the record).

## 10. Conformance tests

1. **Allowable extrapolation:** potential 1500 and extrapolated 1800 give 0; potential 1500 and extrapolated 1200
   give 300.
2. **Potential:** a population of 500 gives 1000; 2000 gives 2200; 0 gives 0.
3. **Balance:** produced 10, imported 5, demanded 8, exported 2 give 5.
4. **Clamp:** `SetExported(1, −3)` stores 0.
5. **Move:** `SetPosition(5, 4, true)` from (3, 4) moves a deal's cells (3, 4) and (2, 4) to (5, 4) and (4, 4) and
   sets `moved`.
6. **Tax rate:** `GetTaxRate(0x3320)` reads the seventh rate; `GetTaxRate(0x9999)` returns 7.0.
