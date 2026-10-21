# MySims (`cSC4MySimAgentSimulator`, `cSC4MySim`) — simulation-side behaviour spec

Status: round 3, written from the Windows 1.1.641 executable. The named Mac build was used for names and
intent only; every behaviour, constant and slot below was checked on Windows unless marked *[inferred]*.

Tags: *[compat]* observable, must match; *[choice]* FreeCity4 decision; *[inferred]* not proven;
*[quirk]* original behaviour that looks unintended but is reproduced (or that a replacement must survive).

Shared material cited and not repeated here: streams, refs and strings (foundations §2), class store (§3), city
component table (§3.6), Init order (§3.7), messages (§5), random generators (§6), slot-order rule (§16),
x87 precision (§18); the advisor system, advisors, advices, the event queue, the Lua `game`/`mysim_*` functions
and the daily/monthly ticker (`docs/specs/advisors.md`, written separately); residential EQ/HQ, age and grade data
(`residential.md`, `residential-education-health.md`); grids (`simgrid.md`); the mayor rating (`aura.md`).

Out of scope (UI, automata, audio): window procs, the pick-sim and move-in input controls, balloons and their
rendering, camera following, sounds, the U-Drive-It missions and the dispatch tool, the hearse and car automata.
They are named only where the simulation side calls them.

---

## 1. Role and identity

**Summary.** A MySim is a named resident the player places in a house. It is an **advisor** (it derives from
`cSC4Advisor`) whose advices are the MySim's opinions ("thought balloons"), written in Lua. The engine side of a
MySim:

- keeps the MySim's home, job, age, wealth, generation and a few counters,
- copies values from the city simulators into the MySim's **Lua table** (§4.4–§4.6), which the advice scripts read,
- reacts to building, disaster and clock messages by pushing **advisor events** (§5.2),
- runs the life events that the advice scripts request through **script commands** (die, retire, spawn, …, §4.8).

**MySims do not feed back into any simulation result** *[compat]* (§9.1). Nothing they do changes population,
demand, budget, land value, the mayor rating or any grid. They are purely observational. Their only outputs are
Lua variables, advisor events, UI/audio messages and automata.

### 1.1 Classes

| Class | CLSID | Interfaces (IID) | Role | Spec |
|---|---|---|---|---|
| `cSC4MySimAgentSimulator` | `0x29E37DEC` | `cISC4MySimAgentSimulator` `0xA9E37EBB` (also answers `1`), `cIGZMessageTarget2` `0x452294AA`, `cIGZSerializable` `0xE4FDA3D4` | per-city dispatcher of messages to the MySims | this spec |
| `cSC4MySim` | `0x4A1DBBBF` | `cISC4MySim` `0x0A1DA42F`, private class pointer `0x8BC01FFE` *[inferred: returns the object itself, AddRef'd]*, plus everything `cSC4Advisor` answers (`cISC4Advisor` `0xCA09F544`, `cIGZSerializable`, `cIGZMessageTarget2`, `1`) | one MySim | this spec |
| `cSC4MyStreetSim` | `0x0BF606A5` | answers `cISC4MySim` `0x0A1DA42F` | the "My Street" sim (advisor id 30) | out of scope; it shares the agent's sixth slot (§1.3) |
| `cSC4MySimDispatch` | `0xCBC14674` | read as IID `0xEB8B6F9A` | per-MySim trip controller (path, automaton, commute destination) | **automata**, out of scope; named where the simulation side calls it (§4.7) |
| `cSC4MySimPool` | — | — | imports Sim templates (names, portraits) for the pick-sim UI | **app/UI**, out of scope |
| `cMySimBuildingFilter` | — | `cISC4OccupantFilter` | building query filter | this spec, §4.9 |
| `cSC4AdviceSubject` | `0x6A26D049` | `0x6A26D050` | event subject | advisors.md |

Every interface of one object shares one reference count. `cSC4MySim`'s release path is the advisor's (advisors.md).
The agent's `Release` at count ≤ 1 calls `Shutdown()` and then the destructor; it returns 0 then, otherwise the
new count.

### 1.2 Creation, ownership and persistence owner

- **The agent simulator is a per-city component.** `cSC4City::CreateCityComponents` creates it with
  `GetClassObject(0x29E37DEC, 0xA9E37EBB)`. The city exposes it through `cISC4City` slot 71 (`+0x11C`)
  `GetMySimAgentSimulator`. It is **not** one of the city's saved components (foundations §3.6): the city's
  Read/Write never touch it, and nothing writes a reference to it *[compat]*.
- Init order: the agent is initialised after the advisor system, the city situation manager and the tutorial
  system, and before the city detail manager (foundations §3.7). The city releases it in its Shutdown.
- **The MySims are children of the advisor system.** The advisor system creates the MySim advisors from the Lua
  `advisors` table (ids 1–5, class `0x4A1DBBBF`) and the My Street sim (id 30, class `0x0BF606A5`), keeps them in
  its advisor map, and **saves them** (its record holds a ref plus the advisor id for each advisor; advisors.md
  §11). On load the class store recreates them from those refs. The agent only holds extra references.
- The agent's `Init` rebuilds its slot vector from the advisor system every time (§1.3). So a replacement agent
  needs no save data.

### 1.3 The agent's slot vector

The agent holds a vector of **6** `cISC4MySim` pointers (AddRef'd), all null at construction.

`Init` (once; a flag guards it):

1. Take the city (AddRef) and its advisor system (`+0x144`, AddRef).
2. Walk the advisor system's advisor map in its own order (by advisor id *[inferred]*). For each advisor that
   answers `QueryInterface(0x0A1DA42F)`:
   - advisor id (advisor slot 5) = **30** → store it at index **5**;
   - otherwise → store it at the next index, starting at 0.
3. Subscribe to the messages of §3.2 in the order listed there.
4. Register two Lua functions in the game data registry (service `0xEBE0E860`/`0xE898ED03`): `get_sim`
   (returns the MySim at the registry's current index) and `get_count` (returns the vector's size, 6). These are
   used by the Lua scripts; they are not simulation.

*[quirk]* The running index has no bound. With the shipped table it receives ids 1–5 → indices 0–4. A sixth
non-30 MySim advisor (a plugin) would overwrite index 5 and a seventh would write past the vector (undefined).
*[choice]* FreeCity4 ignores MySim advisors beyond index 4.

Most agent loops visit **indices 0–4 only**. Index 5 (My Street sim) is visited only by `TrackLocation` and
`HideLocation` and is reachable through `GetActiveUIPanelSim`.

---

## 2. State

### 2.1 Agent simulator

| Field | Initial | Persisted |
|---|---|---|
| initialised flag | false | no |
| slot vector (6 refs) | nulls | no (rebuilt in `Init`) |
| last disaster type | 0 | no |
| city, message server | null | no |
| active UI panel index | −1 | no |

### 2.2 One MySim

"Index" below means the MySim's own record, not memory layout.

| Field | Initial | Persisted | Notes |
|---|---|---|---|
| state | 0 | yes (u8) | 0 none (slot empty), 1 moving in, 2 at home, 3 at work, 4 travelling to work, 5 travelling home, 6 travelling elsewhere (a dispatch-tool pylon or other target). Set by `cSC4MySim::DoMessage` on dispatch notifications (§3.3): **trip start** (sub-id `0x4BB71A7E`, `0x0042D364`): destination = default destination (home) → 5 unless the state is 1 (a move-in keeps 1); else destination = commute destination → 4, otherwise 6. **Arrival** (`0x0042D5B3`): current location = home → state 2 (when it was 1, `AnnounceMoveIn` is called right after state := 2); = commute destination → 3; elsewhere unchanged. `DispatchToolLockPylon` sets 6 when the dispatch accepts the pylon `GoTo`; `QuitJob` sets 2 unless 5 |
| slot | 0 | yes | the UI slot passed to `MoveIn` |
| residence | null | ref | `cISC4BuildingOccupant` |
| home lot | null | no | looked up from the residence |
| home cell x, z | 0 | yes | from the home lot's location |
| home position x, y, z (metres) | 0 | yes (3 f32) | used by every distance (§4.10) |
| job | null | ref | `cISC4BuildingOccupant` in this city |
| neighbour job city | null | **no** | `cISC4RegionalCity`; re-resolved by `Init` (§2.2.1) from the saved neighbour city cell. "Has a neighbour job" everywhere in this spec means this reference is non-null; there is no separate flag |
| neighbour city cell x, z; commute destination cell x, z | 0 | yes (4 s32) | |
| jobless days | 0 | yes | |
| vehicle key | 0 | yes (res key) | |
| wealth | 0 | yes (u8) | 1..3 |
| generation | 0 | yes (u16) | 1 at move-in, +1 per Spawn |
| zodiac sign, is male, bio, name | — | sign u8, bool, bio GZStr; the name is the advisor's personal name (advisor record) | |
| age | 0 | yes (u8) | years |
| birth month | 0 | yes (u8) | 0..11 |
| portrait bitmap id and path | — | yes | UI |
| last commute time | 0 | yes | minutes, integer |
| retired | false | yes | |
| last local disaster | 0 | yes | disaster type ID that destroyed home or job, or 0 |
| EQ, HQ (current) | 0.0 | **no** | recomputed by `UpdateVariables` |
| EQ ring index, EQ 6-month delta, EQ ring[6] | 0 | yes | float32 |
| HQ ring index, HQ delta, HQ ring[6] | 0 | yes | float32 |
| car key, pedestrian key | 0 | yes (res keys) | automata |
| reserved s32 pair | 0 | yes (version ≥ 17) | **dead data**: zeroed by the constructor, written and read, never otherwise used by any MySim method (all 69 scanned; Windows ctor `0x00429A9E`) — carry the read values and write them back |
| dispatch | null | ref (`0xEB8B6F9A`) | automata |
| pylon timer, dispatch tool | — | yes | UI |
| pylon locked | false | yes (version ≥ 14) | set to the argument of `DispatchToolLockPylon` (`0x0042728E`); cleared by `DispatchToolDestroyPylon`, `LeaveCity`, `DispatchToolBegin`, `DeleteFromGame`, `Shutdown`; while false `is_pylon_location` is false, and `DispatchToolEnd` (`0x004264C0`) destroys the pylon of a still-active dispatch tool; the pylon timer (§3.3) and `UpdateTripVariables` (§4.6) only act on a locked pylon |
| last player trip day | 0 | yes (s32, version ≥ 16) | |
| commute check pending | false | no | set by neighbour-connection removal and by `Init` (§2.2.1) |
| last trip-update day | 0 | no | |
| local building distances (map occupant group → metres) | empty | no | §4.9 |
| per-MySim RNG | `cRZRandom`, seeded from the wall clock (`0xFFFFFFFF`) at construction | no | §8 |
| Lua table handle | 0 | no | created on first use |

#### 2.2.1 Neighbour job city at `Init` (`0x0042CCF0`, Windows `0x0042CEF3`–`0x0042CF44`)

Only when state ≠ 0, after the dispatch is created and the home lot looked up:

- If the MySim has **no job** and the saved neighbour city cell is not `(0, 0)` (x ≠ 0 **or** z ≠ 0): ask the
  region (`cISC4App` slot 35 `+0x8C`, then `cISC4Region::GetCity(x, z)` slot 11 `+0x2C`) for the city at that
  cell; if one is returned, it becomes the neighbour job city (a null result leaves the current reference as it is,
  which after a load is null). In both cases the **commute-check flag is set**, so the next daily step runs
  `CheckCommute` (§4.2 step 2, §4.7), which drops a neighbour job that no longer resolves or connects.
- Otherwise (the MySim has a job, or the cell is `(0, 0)`): the neighbour job city reference is released.

*[quirk]* A neighbour city at region cell `(0, 0)` is never re-resolved, so a MySim working there loses that
neighbour job on every load.

---

## 3. Scheduling and messages

### 3.1 No simulator agent

Neither class registers with the simulator scheduler (simulator.md): there are **no agent levels and no
finished/unfinished semantics**. MySims are driven by:

- the **advisor system's ticker** (advisors.md §5.1), which calls each advisor's `Simulate(kind)`: daily (kind 0)
  and, on day 28 of the ticker's date, monthly (kind 1) before the daily call. `cSC4Advisor::Simulate` dispatches to
  the class's `simulate_day` / `simulate_month` (§4.2, §4.3);
- **messages** forwarded by the agent (§3.2) and the MySim's own subscriptions (§3.3);
- **advices** whose `command` field asks for a life event (§4.8).

**Ordering.** The ticker is a self-posted advisor-system message, so MySim updates run **between** simulator
ticks and read whatever the simulators last published (residential EQ/HQ, grids, budget). There is no defined
order relative to a particular simulator's month *[compat]*: a replacement must not assume one.

### 3.2 Agent message handling (`cIGZMessageTarget2::DoMessage`, always returns true)

Subscriptions, in `Init` order: `0x99EF1142`, `0x99EF1145`, `0x99EF1143`, `0xA6B79602`, `0xA6B79612`, `0x44778230`,
`0x8A09FA6C`, `0xCA1B1E64`, `0xEA1F0DCD`, `0x0A36F1E9`, `0xEA0D163E`. `Shutdown` removes them.
"For each MySim" means indices 0–4, skipping nulls, in index order.

| Message | Fields read | Action |
|---|---|---|
| `0x99EF1142` occupant inserted | data3 = occupant type, void1 = occupant | type `0x278128A0` (building) → QI `0x87DDA3A9`, each MySim `DoBuildingAdded(b)`. Type `0x69D25C5F` (fire) → occupant position, each MySim `DoFireAdded(x, z)`. **Then** the changed handling below runs too (building → `DoBuildingChanged`). |
| `0x99EF1145` occupant changed | data3, void1 | building → each MySim `DoBuildingChanged(b)` |
| `0x99EF1143` occupant removed | data3, void1, data4 | building only. `cause := lastDisasterType` if `data4 = 4` **and** the city's disaster layer `IsADisasterOngoing()` is true; otherwise 0. Each MySim `DoBuildingRemoved(b, cause)` |
| `0xCA1B1E64` disaster period started | data2, data3, data4 | `lastDisasterType := data4`; each MySim `DoDisaster(data4, data2, data3)` |
| `0xEA1F0DCD` pipe burst | data1, data2 (cell) | `cISC4City::CellCenterToPosition(data1, data2)` → each MySim `DoPipeBurst(x, z)` |
| `0x0A36F1E9` tornado | data2, data3 (cell) | cell centre → each MySim `DoTornado(x, z)` |
| `0xEA0D163E` neighbour connection removed (posted by the neighbours simulator) | data2, data3 | each MySim `DoNeighborConnectionRemoved(data2, data3)` |
| `0x44778230` hour changed (24-hour clock) | data2 = hour | 8 → `LeaveForWork`; 17 → `ReturnHome`; 22 → `PlayNightSound` (automata/audio) |
| `0x8A09FA6C` AdviceCommand (advisors.md §6.4) | IGZUnknown = advisor, data1 = command | if the advisor answers `0x0A1DA42F`: that MySim's `DoScriptCommand(data1)` |
| `0xA6B79602`, `0xA6B79612` view changes | — | each MySim `UpdateResidence` (visual) |
| `0x230E27AC` cheat, data1 `0xEB6F3C97` | cheat text | debug only: posts `0x0B6F3E27` / `0xAB6F3E2D` / `0x4B6F3EBF` / `0x0BB04126` with data1 = a hex argument (print, new job, new house, dispatch) |

### 3.3 MySim's own messages (`cSC4MySim::DoMessage`)

The MySim subscribes (at its Init) to `0x0B6F3E27`, `0xAB6F3E2D`, `0x4B6F3EBF`, `0x0BB04126`, `0x4BC42C48`,
`0x26D31EC1`, and to the automata manager's broadcast. Simulation-relevant ones:

| Message | Action |
|---|---|
| `0x4BC42C48` MySim update, IGZUnknown = this MySim | `UpdateVariables` (§4.5); balloons (UI) |
| `0x2BA06B99` dispatch notification, data1 = sub-id | `0x4BB2D020` / `0x4BB2CFC0` → `UpdateTripVariables` with the new/old trip state (§4.6). `0xEBA06BF4` → **job found** (§4.7). `0xEBA06C12` → arrival: if the destination is still "by commute", `FailCommute` (§4.7); if the dispatch reports "home reached" and state is 1 → state 2 and announce move-in. `0xEBA06BD4` / `0x4BB71A7E` → state transitions (1/2/3/4/5/6) and posts `0x4BC42C48`. Others: visual. |
| `0x26D31EC1` | hide markers (UI) *[inferred: city shutdown notice]* |
| `0x86AD10EE` | balloons every > 2 s of wall time (UI) |
| others | `cSC4Advisor::DoMessage` |

---

## 4. Algorithms *[compat]*

### 4.1 Move-in

1. The UI calls the agent's `MoveIn(name, isMale, sign, bio, bitmapID, bitmapPath, slot)`. The agent picks the
   **first** MySim among indices 0–4 that is non-null and not active (`IsActive` = state ≠ 0) and calls its
   `MoveIn(slot, name, isMale, sign, bio, bitmapID, bitmapPath)`. If all five are active, nothing happens. The
   `slot` argument is only stored; it does not choose the MySim *[compat]*.
2. `MySim::MoveIn` stores the data, sets generation := 1, birth month := advisor month counter mod 12,
   last player trip day := 0, and starts the move-in input control (UI: the player clicks a house).
3. The input control accepts a building with `TestForResidence` (§4.7) and calls `SetResidence(b)` *[inferred:
   the control's exact arguments were not traced; UI]*.

**`SetResidence(b)`** (slot 13):

1. Store the residence; look up its lot (lot manager `GetOccupantLot`) and the lot's cell location.
2. If the state was 0 (a new MySim):
   - `ResetAdvisorState` (advisor Shutdown + Init with the same id, keeping the personal name; balloons rebuilt);
   - age := residential **average-age grid** value at the home cell (`GetAverageAgeMap` → `GetCellValue`), then
     `max(age, 18)` *[compat]*;
   - state := 1; wealth := the home lot's `GetOccupantWealth()`;
   - EQ := residential `GetCellEQByWealth(x, z, wealth)`, HQ := `GetCellHQByWealth(x, z, wealth)` (residential.md
     §10.1 slots 37/38, float32 in ST0); both 6-entry rings are filled with these values, deltas 0;
   - post `MySimsChanged` `0xAA25827C`;
   - `FindNewJob()` (§4.7); start the commute (dispatch); set the dispatch flag.
3. Otherwise: the "tracking" flag := dispatch `CheckFlags(0x100)`.
4. `UpdateVariables()` (§4.5).

The **first arrival home** (dispatch notification, state 1 → 2) calls `AnnounceMoveIn` (slot 40): if `RequestToPlaySound()` (slot 56, `0x00424DE0`) returns true, post `MySimMovedIn` `0xEA3ADA33` (void1 = home occupant,
data1 = is-male, data2 = wealth); then `UpdateVariables` and push the advisor event `MYSIM_MOVE_IN` `0xCA3E87A8`.
`RequestToPlaySound` is a sound throttle, **not** a reload test: it is true only when state ≠ 0 and the MySim's
wall-clock timer (`cRZTimer` in seconds, units 5, constructed with the MySim at `+0x2A0`) shows more than 4 whole seconds since the
last time it returned true (initially 0; unsigned difference); when true it stores the current reading. So a
move-in within 4 s of the previous announcement (or of the MySim's construction) posts no `MySimMovedIn`; the
`UpdateVariables` and the `MYSIM_MOVE_IN` event happen in every case (`0x0042BFF2`).

### 4.2 Daily step (`simulate_day`, advisor `Simulate(0)`)

Skipped entirely when state = 0, or while **a modal window is running**: the window manager (framework service
`0xA417445E`, `cIGZWinMgr` IID `0x5A4`) slot 42 (`+0xA8`, `0x009DB025`) returns true when its modal depth (the
counter `DoModalWin` increments around its loop) is not 0 — the same test as advisors.md §6 popup gate. The monthly
step (§4.3) makes the same check. Otherwise, in order:

1. **Job search.** If there is no job, no neighbour job, the MySim is **not retired**, and state = 2 (at home):
   `FindNewJob()`; if it returns false, jobless days += 1.
2. If the commute-check flag is set: `CheckCommute()` (§4.7), clear the flag.
3. `ActivateLuaContext()` (sets the Lua global `mysim` to this MySim's table, §4.4).
4. Balloons (UI). `cSC4Advisor::simulate_day` (advice evaluation, advisors.md §6).
5. If the advisor day counter is greater than the last trip-update day: store it and
   `UpdateTripVariables(−1, −1, −1, −1, false)` (§4.6).
6. Dispatch-tool pylon timeout (UI).

`simulate_day_events` (end of the advisor day pass) only re-queues balloon advices (UI) and then runs the base
event processing (advisors.md §6.3).

### 4.3 Monthly step (`simulate_month`, advisor `Simulate(1)`)

Same skip conditions as §4.2. Then:

1. **Ageing.** If `monthCounter mod 12 = birthMonth` (signed division; the advisor's month counter is incremented
   by `cSC4Advisor::Simulate(1)` before this call): age += 1 (u8, wraps at 256).
2. If the MySim has a job or a neighbour job: `CheckCommute()`.
3. `UpdateVariables()`.
4. **6-month EQ/HQ change** (float32, §18):
   `eqDelta := EQ − eqRing[i]; eqRing[i] := EQ; i := (i + 1) mod 6`, and the same for HQ with its own index.
   The ring holds the values of the last six monthly steps, so `delta_eq` is the change over six months.
5. `ActivateLuaContext()`. The base `simulate_month` is empty on Windows.

There is **no** death, retirement or wealth rule in the engine: those come from advice scripts (§4.8).

### 4.4 The Lua table

Each MySim has a table in the advisor system's Lua state, created on first use (`CreateLuaTable`, which also
forces a full `UpdateTripVariables(…, true)`). `ActivateLuaContext` (slot 68) pushes the table as the global
**`mysim`** and returns true (false if there is no Lua state). The advice scripts (`adv_mysim*.lua`, consumers)
read the fields below as `mysim.<name>`. Every value is stored as a Lua number unless noted; "int(x)" is the CRT
`_ftol` truncation toward zero. When a source is missing the field is **not written** and keeps its previous
value *[quirk]*.

### 4.5 `UpdateVariables` (full refresh) and `UpdateQuickVariables`

**Quick** (also called at the start of the full refresh):

| Field | Value |
|---|---|
| `state`, `age`, `wealth`, `generation`, `retired`, `zodiac`, `is_male`, `last_local_disaster` | the MySim's fields |
| `home_location`, `job_location` | building subjects (Lua helper `SetTableBuilding`) |
| `name` | the advisor's personal name |

**Full** (only when the Lua table exists; home fields only when there is a residence):

| Field | Source *[compat]* |
|---|---|
| `eq`, `hq` | `EQ := GetCellEQByWealth(homeX, homeZ, wealth)`, kept only if `> 0.0` (else the previous EQ stays); field = int(EQ). Same for HQ (`GetCellHQByWealth`). |
| `delta_eq`, `delta_hq` | int(eqDelta), int(hqDelta) (§4.3) |
| `le` | residential `GetCellLifeExpectancy(homeX, homeZ)` (uint8, residential.md slot 29) |
| `home_condition` | home lot `GetCondition()` (byte), 0 without a lot |
| `home_building` | the residence's building name (string) |
| `home_has_road` | home lot `HasRoadAccess()` as 0/1 |
| `home_has_power` | power simulator `IsOccupantPowered(home)` (slot 31) |
| `home_has_water` | plumbing simulator `IsBuildingWatered(home)` (slot 9) |
| `local_crime` | crime grid `GetCellValue(homeX, homeZ)` (uint8) |
| `home_police_coverage`, `home_fire_coverage` | police / fire coverage grid (`+0x14` of each simulator) `GetAverageValueInCellRect(x0, z0, x1 − x0 + 1, z1 − z0 + 1)` over the home lot's cell rectangle (uint8) |
| `local_air_pollution`, `local_water_pollution`, `local_garbage`, `local_radiation` | pollution simulator `GetPollutionGrid(0..3)` → `GetCellValue(homeX, homeZ)` (int16, signed) |
| `last_player_trip_day` | the MySim's field |
| `desirability` | wealth 1/2/3 → building development simulator `GetDeveloper(0x1010 / 0x1020 / 0x1030)` → its desirability grid (developer slot 13) → `GetCellValue(homeX, homeZ)` (int16). Other wealth or no developer → not written. |
| `home_occupant_wealth` | home lot `GetOccupantWealth()` |
| `local_police_funding`, `local_fire_funding` | the **nearest** police / fire station (police `+0x54` / fire `+0x58` station lists; nearest by §4.10 distance, strict `<`, list order) → that station's funding fraction (`+0x1C` / `+0x20`, `SC4Percentage`), field = int(f × 100). No station → 0 (`kSC4PercentageZero`). |
| `local_traffic_congestion` | the home lot's bounding rect (`cISC4Lot` slot 14, inclusive cells minX, minZ, maxX, maxZ) grown by 1 cell on each side; minX and minZ clamped up to 0, maxX **and maxZ** clamped down to `CellCountX − 1` (`cISC4City` slot 93 for both, *[quirk]*, invisible in square cities). Only the **border ring** of that rect is visited, not its interior: for x = minX..maxX the cells (x, maxZ) then (x, minZ); for z = minZ+1..maxZ−1 the cells (minX, z) then (maxX, z). A cell counts only if the traffic simulator's `GetCapacity(1, x, z)` (slot 23) is not 0.0; the value is the u8 maximum of the congestion map (slot 10, map slot 7 `Get(x, z)`) over counted cells, 0 if none (Windows `0x0042B3DE`–`0x0042B581`) |
| `job_title`, `job_building`, `job_city` | strings (`GetJobTitle`, the job building's name, the neighbour city's name) |
| `last_commute_time` | minutes (§4.7) |
| `employed`, `unemployed_days` | 1 if job or neighbour job; jobless days |
| `job_is_neighbor`, `job_condition`, `job_has_power`, `job_has_water` | for a city job: 0; the job lot's condition (`cISC4Lot` slot 35, 0 when the job has no lot); power as for home; water: **1 when the job building's profile purpose is 5 (agriculture)** (BDS `GetBuildingProfile`, `0x0042BA92`), else as for home. For a neighbour job: 1, 0, 1, 1 (constants, `0x0042BB54`–`0x0042BC0F`) |

`UpdateLocalBuildings` (§4.9) also runs inside the full refresh.

### 4.6 `UpdateTripVariables(newTripState, oldTripState, newFlags, oldFlags, force)`

Runs only when the Lua table exists, its handle is > 0 and the class's "active" check passes. It selects the
MySim's table, writes, and restores the Lua stack.

- If the trip state changed or `force`: `trip_in_transit` := (new = 2), `trip_at_location` := (new = 3). On
  arrival (new = 3) the MySim's state follows the dispatch: at the default destination (home) → state 2 (and
  `AnnounceMoveIn` if the state was 1); at the commute destination → state 3. Then `UpdateQuickVariables`.
  `destination_subject` := the occupant at the current destination when travelling, or at the current location
  when it is neither home nor work (null otherwise).
- If the flags changed beyond bit `0x100`, or `force`: `trip_at_transit_switch` (flag `0x10`), `trip_idle` (flag 1);
  if bit 15 of the flags went from clear to set, push event `MYSIM_APPROACHING_LOCATION` `0xEC107DF9`; the
  travel-type flags `trip_on_bus` (2), `trip_on_foot` (0), `trip_on_subway` (6), `trip_in_car` (1),
  `trip_on_train` (3), `trip_on_elevated_train` (7), `trip_on_monorail` (8), and `last_trip_type` (the matching
  number, 255 when none matches).
- **Local area.** `half := LOCAL_AREA_SIZE / 2` (integer); the square is the MySim's current cell ± half
  (current dispatch location; with `force` the cell is reset to (0, 0) first *[quirk]*).
- Only if the traffic, pollution and crime simulators all exist:
  - if the dispatch state is 2 (travelling) or `force`:
    - if not idle or `force`: `trip_traffic_congestion` (congestion map value, uint8), `trip_time_ratio`
      (traffic `GetTravelTimeRatio(cellX, cellZ, travelType)` as int; travel type 0xFE/0xFF → 1),
      `trip_wait_length` (dispatch transit-switch wait, minutes);
    - `trip_pollution_air/water/garbage` (pollution grids 0–2, int16), `trip_crime` (crime grid, uint8),
      **`trip_mayor_rating`** (aura simulator `GetAuraGrid()` value, **int8, signed**; aura.md), `trip_eq` /
      `trip_hq` (residential `GetAverageEQGrid` / `GetAverageHQGrid` values as int).
    All grid reads convert the current position with `PositionToTract` and read with `GetTractValue`
    (simgrid §4 slots 19/22, **no bounds check** *[quirk]*).
  - if `force`, dispatch state 3 (at a location), on foot, in a car, or idle: query the lots in the square
    (lot manager `GetLotsInCellRect(x0, z0, x1, z1, false)`). For every lot **with a building**, count it (n) and:
    - condition ≥ 4 → distress3; = 3 → distress2; = 2 → distress1; 0–1 → none;
    - lot occupant wealth ≥ 3 → high; = 2 → medium; = 1 → low;
    - the first building in **list order** that is in occupant group `0x150A` (landmark) is remembered.

    Then, float32 (§18): `r := 1.0 / (float(n) + 0.0001)` (the double constants enter at 24-bit precision);
    each percentage field = int(float(count) × r × 100.0): `trip_distress1_p`, `trip_distress2_p`,
    `trip_distress3_p`, `trip_wealth_low_p`, `trip_wealth_med_p`, `trip_wealth_high_p`.
    `trip_landmark_nearby` (0/1), `trip_landmark_name` (string, empty if none), `trip_landmark_funding_p` =
    int(funding × 100) of the landmark's budget item for purpose `0xAA59670C` (budget `GetBudgetItemForPurpose`,
    slot 51; 0 when no landmark; the funding field defaults to 0.0 if the item is missing).
    Disaster flags: walk the disaster layer's registered locations (`GetDisasterLocationCount`,
    `GetDisasterLocationByIndex`); a location inside the square sets `trip_any_disaster_nearby`; its manager type
    `0x621CDA33` sets `trip_fire_nearby`, `0xCA380B06` sets `trip_riot_nearby`; the walk stops early once both are
    set.
- Still inside the "simulators exist" block: if the trip state changed **and the flag block above did not run**,
  post `0x4BC42C48` (MySim update, IGZUnknown = this MySim's `cISC4MySim`). The flag block clears the "changed"
  marker *[quirk]* (Windows `0x00427C29`).

The argument names follow the dispatch notification that calls it: `0x4BB2CFC0` passes (data3, data2, −1, −1)
and `0x4BB2D020` passes (−1, −1, data3, data2) *[inferred: data3 = new, data2 = old]*.

### 4.7 Job, commute and residence changes

**`FindNewJob()`** asks the dispatch to set a commute destination "by commute": the dispatch runs a traffic
pathfinder (traffic simulator `CreatePathFinder`, slot 6) from the home lot towards job destinations and takes
the lot (building) or neighbour connection at the end *[inferred: destination mask `0x1CF`; see the traffic spec
for the pathfinder]*. It returns whether a destination was set. When the dispatch later reports the destination
(`0xEBA06BF4`):

- a city lot → job := the lot's building; neighbour city := null;
- no lot → neighbour city := the region's city at the neighbour cell (app → region → city at cell); job := null;
- in both cases store the cells, jobless days := 0, `UpdateVariables`, push event `MYSIM_NEW_JOB` `0xAA3EDBB1`.

**`ChangeJob(b)`** (slot 8): set the dispatch commute destination to building `b`; result of the dispatch.

**`CheckCommute()`**: creates a pathfinder from the home lot rectangle to the job lot rectangle (or the
neighbour cell), runs it, and on success stores `lastCommuteTime := uint(trunc(max(0, totalTime)))`. On failure
→ **`FailCommute`**: `QuitJob()`, push event `MYSIM_COMMUTE_FAILED` `0xCA4553B7`, reset the commute destination.
The pathfinder's slot use is in §7.2.

**`QuitJob()`** (state ≠ 0): drop the job (or the neighbour city and its cells); if state ≠ 5, state := 2;
`UpdateVariables`.

**`DoNeighborConnectionRemoved(x, z)`**: if the MySim has a neighbour job, set the commute-check flag (the next
daily step runs `CheckCommute`). The arguments are ignored *[compat]*.

**`TestForResidence(b, filter, matchWealth)`** (slot 7) → bool:

1. false if `b` is the current residence;
2. if `matchWealth` and `b` has a lot: false unless the lot's occupant wealth equals the MySim's wealth;
3. false if another MySim lives there (agent `MySimLivesHere(b)` non-null; this includes the caller only if it
   already lives there, which step 1 excludes);
4. if `filter`: the result of the residential `cMySimBuildingFilter` (§4.9); else true.

**`ChangeResidence(b)`** (slot 6) → bool: if `TestForResidence(b, false, false)`: move out, `SetResidence(b)`;
then if `CommandWouldTrigger(MoveOut 0x6A47E79B)` (advisors.md, advisor slot 19: some move-out advice would fire
now) **revert**: move out and `SetResidence(old)`. Then location tracking (UI). If the residence actually
changed: wealth := new lot's occupant wealth, state := 1, return true. Else false.

**`FindNewResidence()`** (slot 5) → **always false** *[quirk]* (SDK: bool):

1. Query all occupants citywide with the residential `cMySimBuildingFilter` (occupant manager slot 38, no
   rectangle, filter, group −1).
2. If the query succeeds: `n :=` the list size; **one draw** `r := rng.RandomUint32Uniform(n)` on the MySim's
   own `cRZRandom` (§8). Start at list position `max(r − 1, 0)` (r = 0 walks to the end and wraps to the
   first element). Visit up to n buildings cyclically; the first one that answers `0x87DDA3A9`, passes
   `TestForResidence(b, false, false)` and `ChangeResidence(b)` wins.
3. If there is still no residence, or the residence is unchanged (then move out first): push event
   `MYSIM_CANT_FIND_HOME` `0x6A57DD94`.

**`MoveOut()`**: drop residence and home lot, hide markers and sounds; returns true.
**`LeaveCity()`** (state ≠ 0): `MoveOut`, hide, state := 0, dispatch Shutdown, clear the U-Drive-It state,
destroy the pylon, post `MySimsChanged`.
**`DeleteFromGame()`** (slot 9, from the UI "remove sim"): dispatch Shutdown, pylon, `MoveOut`, state := 0,
hide, `ResetAdvisorState`, post `MySimsChanged`. Returns true.

### 4.8 Life events: script commands *[compat]*

The advice scripts drive life events. When an advice with a non-zero `command` activates, the advisor system posts
`AdviceCommand` `0x8A09FA6C` (advisors.md §6.4); the agent routes it to that MySim's `DoScriptCommand(cmd)`
(slot 41), which does nothing when state = 0. Command IDs (SDK enum; names also in `adv_const.lua`
`game_commands.MYSIM_*`):

| Command | ID | Effect |
|---|---|---|
| MoveOut | `0x6A47E79B` | `FindNewResidence()` (slot 5), not a plain move-out *[compat]* |
| Retire | `0xAA4934B4` | `QuitJob`, retired := true, update location tracker |
| Die | `0xEA4D269C` | the city's automata controller manager (`cISC4City` slot 47) gets the trigger **`my_sim_died`** at the home position (spawns a hearse, game data `my_sim_died.lua`; visual only); ghost effect; `LeaveCity` |
| LeaveCity | `0x0A57DE1F` | `LeaveCity` |
| Spawn | `0x6A495F7F` | next generation: retired := false, generation += 1, bio := empty, age := residential `GetAverageNewAgeByWealth(wealth)` (residential.md §6.5; AL uint8), `QuitJob`, ghost effect, post `MySimsChanged` |
| IncreaseWealth | `0x6A4A5E9E` | if wealth < 3: wealth += 1, `QuitJob`, `FindNewResidence` |
| QuitJob | `0x6A5E6860` | `QuitJob` |
| other | — | ignored |

**Where randomness and thresholds live.** The engine draws no random number for death, retirement, spawn or
wealth. The shipped scripts use `tuning_constants.MYSIM_*` (ages, HQ/EQ thresholds, desirability bands; game data
in `adv_const.lua`) and the advice system's own frequency/once/random fields (advisors.md). So these rules are
**data**, not engine behaviour; FreeCity4 must run the same scripts to reproduce them.

### 4.9 Building filter and local building distances

**`cMySimBuildingFilter(residentialOnly)`** accepts an occupant when all hold:

- its type is building (`0x278128A0`) and it answers `0x87DDA3A9`;
- its lot (lot manager `GetOccupantLot`) is missing, or the lot's condition is ≤ 3 (not at the last abandonment
  stage; crime.md §6.7 condition values);
- `GetCompletionPercent()` ≥ 1.0 (fully built);
- if `residentialOnly`: the building profile's purpose is 1 (Residence).

**`UpdateLocalBuildings`** (part of the full refresh; only with a residence): clear the map; query the occupants
in the cell box `homeCell ± 20` (occupant manager slot 40, both corners inclusive *[inferred]*) with
`cMySimBuildingFilter(false)`; for each building, `d :=` distance (§4.10); for each occupant group of the
building's exemplar (building development simulator slot 14 → exemplar key → `GetOccupantGroups`):
`map[g] := d` if `map[g] = 0` or `d < map[g]`.

*[quirk]* A stored 0 counts as "unset": a building at distance 0 (the home itself, whose centre is the home
position) is overwritten by the next building of the same group. So `DistanceToClosest(residential group)` rarely
returns 0.

**`DistanceToClosest(g)`** (slot 50) → `map[g]`, or `0xFFFFFFFF` (−1) when state = 0, there is no map, or `g` is
absent. **`DistanceToClosestSpecial(type)`** (slot 51) → minimum distance over the building development
simulator's special-building list (`cISC4City` slot 53 → BDS slot 44 `GetSpecialBuildings`) of buildings whose
`GetBuildingType()` equals `type`; `0xFFFFFFFF` when none. That list holds exactly the occupants in occupant group
`0x150A` (landmark) or `0x150B` (reward), excluding previews (building-development.md §3, messages
`0x99EF1142`/`0x99EF1143`), in insertion order. Per entry (`0x00426AE0`): query `cISC4BuildingOccupant`
(`0x87DDA3A9`, entries that fail are skipped), compare its type, take its position, distance =
`sqrt(dx² + dz²)` (x and z only, home position minus building position, both float32; the products, sum and root
stay on the x87 stack, no float32 rounding before `__ftol`) truncated toward zero to an integer;
the result is the **unsigned** minimum, starting from `0xFFFFFFFF`.

**`LocalSchoolGrade(group)` / `LocalHospitalGrade(group)`** (slots 52/53): over the residential simulator's school
(slot 47) / hospital (slot 49) building list (all groups, including pending), take the building in occupant group
`group` (`IsOccupantGroup`) with the smallest distance (strict `<`, list order); return its query-data
**grade** (`GetSchoolQueryData` / `GetHospitalQueryData`, slots 26/27, field "grade" of
residential-education-health.md §4.11) as `SC4Percentage`. No building → 0.0. A failed query → 0.0 (the record
is zero-initialised).

### 4.10 Distance

`DistanceToOccupant(o)`: `o.GetPosition()`; `dx := homeX − px`, `dz := homeZ − pz` (float32, §18),
`d := sqrt(dx·dx + dz·dz)` computed on the x87 stack and truncated by `_ftol` to an unsigned integer (metres).
The home **position** (not the cell) is used. The disaster/fire/pipe/tornado handlers use the same formula with
their event position (clamped below at 0, converted to unsigned with the usual large-value handling).

### 4.11 Tuning (read in MySim Init, from Lua) *[compat]*

No exemplar and **no response curves** are loaded (so no foundations §4.1 pattern applies). The MySim reads the
Lua table `tuning_constants` (game data, `adv_const.lua`) with a table property reader; each value has a code
default that stays when the key is missing *[inferred]*:

| Key | Code default | Shipped | Used by |
|---|---|---|---|
| `MYSIM_BALLOON_FADE_DURATION` | 300 | 300 | balloons (UI) |
| `MYSIM_BALLOON_SHOWN_DURATION` | 1500 | 2500 | balloons (UI) |
| `MYSIM_BALLOON_HIDDEN_DURATION` | 300 | 300 | balloons (UI) |
| `MYSIM_TRIP_MIN_TIME_AT_LOCATION` | 1 | 30 | dispatch |
| `MYSIM_TRIP_MIN_TIME_AT_TRANSIT_SWITCH` | 1 | 1 | dispatch |
| `MYSIM_TRIP_MIN_TIME_IDLE` | 1 | 1 | dispatch |
| `MYSIM_TRIP_DESTINATION_PROXIMITY_MIN` | 10 | 4 | dispatch |
| **`MYSIM_TRIP_LOCAL_AREA_SIZE`** | 10 | 10 | the trip-variable square (§4.6) |
| `MYSIM_TRIP_APPROACH_DISTANCE` | 10 | 8 | dispatch |
| `MYSIM_TRIP_DEFAULT_CAR_MODEL` | 0 | `0x106A0000` | automata |

The other `tuning_constants.MYSIM_*` values (ages, EQ/HQ/desirability thresholds, radii) are read only by the
scripts.

---

## 5. Messages and events posted

### 5.1 Messages

| ID | Name | When | Fields | Consumers |
|---|---|---|---|---|
| `0xAA25827C` | MySimsChanged | `SetResidence` (new MySim), `LeaveCity`, `Spawn`, `DeleteFromGame` | none | MySim window, MySim input control (UI) |
| `0xEA3ADA33` | MySimMovedIn | `AnnounceMoveIn` | void1 = home occupant, data1 = is-male, data2 = wealth | audio event handler |
| `0x4BC42C48` | MySim update | trip state changes, arrival at a pylon | IGZUnknown = the MySim | the MySim itself (§3.3) |

No simulator subscribes to any of them (checked by scanning every use of the three IDs) *[compat]*.

### 5.2 Advisor events (`PushEvent(event, subject)`, advisors.md §6.3)

Subjects are new `cSC4AdviceSubject` objects (`0x6A26D049`/`0x6A26D050`). Names from `adv_const.lua`.

| Event | ID | Trigger | Subject |
|---|---|---|---|
| MYSIM_BUILDING_ADD | `0xCA4E8986` | `DoBuildingAdded(b)`, MySim has a residence | `SetObject(b)`, `SetData1(distance in metres)` (§4.10); read by `event_building_distance()` |
| MYSIM_FIRE_STARTED | `0xEA5E4E89` | `DoFireAdded(x, z)` | data1 = distance |
| MYSIM_PIPE_BURST | `0x6A5E598C` | `DoPipeBurst(x, z)` | data1 = distance |
| MYSIM_TORNADO | `0x0A5E8243` | `DoTornado(x, z)` | data1 = distance |
| MYSIM_RIOT | `0x8AC11A9B` | `DoDisaster(0xCA380B06 riot, cx, cz)` | `SetLocation(float(cx), float(cz))` — **cell indices, not metres** *[quirk]*; data1 = distance from the cell centre |
| MYSIM_VOLCANO | `0x2AC14492` | `DoDisaster(0xC4C90997 volcano, cx, cz)` | as riot |
| MYSIM_HOME_DESTROYED | `0xAA48F5C1` | home removed, cause 0 | null; then `FindNewResidence` |
| MYSIM_HOME_DESTROYED_BY_DISASTER | `0xAA4D2D64` | home removed, cause ≠ 0 | null; last local disaster := cause, `UpdateVariables` first, then `MoveOut` |
| MYSIM_JOB_DESTROYED | `0xCA48F879` | job removed, cause 0 | null; then `QuitJob` |
| MYSIM_JOB_DESTROYED_BY_DISASTER | `0xAA4D2D65` | job removed, cause ≠ 0 | null; last local disaster := cause, `UpdateVariables`, then `QuitJob` |
| MYSIM_MOVE_IN | `0xCA3E87A8` | `AnnounceMoveIn` | null |
| MYSIM_NEW_JOB | `0xAA3EDBB1` | job found (§4.7) | null |
| MYSIM_COMMUTE_FAILED | `0xCA4553B7` | `FailCommute` | null |
| MYSIM_CANT_FIND_HOME | `0x6A57DD94` | `FindNewResidence` without a new home | null |
| MYSIM_APPROACHING_LOCATION | `0xEC107DF9` | trip flag bit 15 set | null |

All handlers do nothing when state = 0. Other disaster types are ignored by `DoDisaster`. `DoBuildingChanged(b)`
only re-runs `UpdateVariables` when `b` is the residence or the job.

---

## 6. Persistence *[compat]*

### 6.1 Agent simulator

`Write` writes `u16 1` and nothing else; `Read` (a shared body) requires version 1. `GetGZCLSID` = `0x29E37DEC`.
Nothing in the shipped game writes a reference to the agent, so **no record exists in saves** *[compat]*.

### 6.2 MySim record (class `0x4A1DBBBF`, written by the class store because the advisor system holds a ref)

Version **17** (`0x11`). All integers little-endian; "ref" is foundations §2.7; "key" is three u32 (§2.5);
"bool" is a u8 0/1.

| # | Type | Field |
|---|---|---|
| 1 | u16 | version = 17 |
| 2 | u8 | state (the int truncated to a byte). **If 0, the record continues directly with the advisor part (row 32).** |
| 3 | u8 | slot |
| 4 | ref | residence (read as `cISC4BuildingOccupant` `0x87DDA3A9`) |
| 5 | u32, u32 | home cell x, z |
| 6 | ref | job (`0x87DDA3A9`) |
| 7 | s32 × 4 | neighbour city cell x, z; commute destination cell x, z |
| 8 | u32 | jobless days |
| 9 | key | vehicle key |
| 10 | u8 | wealth |
| 11 | u16 | generation |
| 12 | u8 | zodiac sign |
| 13 | bool | is male |
| 14 | GZStr | bio |
| 15 | u8, u8 | age, birth month |
| 16 | u32 | portrait bitmap id |
| 17 | GZStr | portrait path |
| 18 | u32 | last commute time |
| 19 | bool | retired |
| 20 | u32 | last local disaster |
| 21 | f32 × 3 | home position x, y, z |
| 22 | u8, f32, f32 × 6 | EQ ring index, EQ delta, EQ ring |
| 23 | u8, f32, f32 × 6 | HQ ring index, HQ delta, HQ ring |
| 24 | key, key | car key, pedestrian key |
| 25 | s32, s32 | reserved pair (dead data, §2.2) |
| 26 | ref | dispatch (`0xEB8B6F9A`) |
| 27 | u8, u64, u8 | pylon timer: units, elapsed, running |
| 28 | bool | has dispatch tool (tool exists and its slot `+0x40` is false) |
| 29 | ref | dispatch tool (`0xC9B6D7D6`), only if row 28 is true |
| 30 | bool | pylon locked (§2.2) |
| 31 | s32 | last player trip day |
| 32 | — | the `cSC4Advisor` record (advisors.md §11): advices, personal name, … |

**Read** accepts every version (no lower or upper check) *[quirk]*:

- rows 1–23 always;
- version 11–14: five s32 are read and discarded instead of row 24; version ≥ 15: row 24; version < 11: neither;
- version ≥ 17: row 25;
- version < 12: no rows 26–29; the existing dispatch is released (it is recreated when the MySim initialises);
- version 12: the dispatch is read **inline** (`cIGZSerializable::Read` on the existing dispatch), then rows 27–29;
- version ≥ 13: the existing dispatch is shut down and released, row 26 read as a ref, then rows 27–29;
- version ≥ 14: row 30; version ≥ 16: row 31;
- then the advisor part. The current EQ/HQ, the Lua table, the local building map and the neighbour city are
  **recomputed**, not read.

*[choice]* FreeCity4 writes version 17 only and reads versions 11–17; it rejects other versions in standalone mode.

---

## 7. Interfaces *[compat]*

Return channels were read from the epilogues; pops come from `abi_extract.py --check`. "AL" = bool in AL only.

### 7.1 `cISC4MySimAgentSimulator` (vtable `0x00A85544`, 13 interface slots + destructor)

| Slot | Off | Method (as the game expects it) | Pop | Ret | Address | Behaviour / SDK |
|---|---|---|---|---|---|---|
| 0 | +0x00 | `QueryInterface(iid, void**)` | 8 | AL | `0x0042E310` | §1.1 |
| 1 | +0x04 | `AddRef()` | 0 | EAX | `0x004F7BA0` | |
| 2 | +0x08 | `Release()` | 0 | EAX | `0x0042E410` | §1.1 |
| 3 | +0x0C | `bool Init()` | 0 | AL | `0x0042EC30` | §1.3 |
| 4 | +0x10 | `bool Shutdown()` | 0 | AL | `0x0042E500` | unregister Lua functions, unsubscribe, release the city and the six MySims |
| 5 | +0x14 | `bool GetActiveMySims(SC4Vector<cISC4MySim*>&)` | 4 | AL | `0x0042F4A0` | appends (AddRef'd) every MySim among 0–4 with `IsActive`; true if any |
| 6 | +0x18 | `void MoveIn(name&, isMale, sign, bio&, bitmapID, path&, slot)` | 28 | — | `0x0042E990` | §4.1 |
| 7 | +0x1C | `void TrackLocation(cISC4MySim*)` | 4 | — | `0x0042E9F0` | UI, all six |
| 8 | +0x20 | `void HideLocation()` | 0 | — | `0x0042EA70` | UI |
| 9 | +0x24 | `cISC4MySim* MySimLivesHere(cISC4BuildingOccupant*)` | 4 | EAX | `0x0042EB60` | first of 0–4 whose `GetResidence()` equals the building; else null. Not AddRef'd. |
| 10 | +0x28 | `void CancelMoveIn()` | 0 | — | `0x0042E3A0` | removes the move-in input control (UI) |
| 11 | +0x2C | `void SetActiveUIPanelSim(int32 slot)` | 4 | — | `0x0042E910` | active index := the first index 0–4 whose MySim `GetSlot()` equals `slot`, else −1 |
| 12 | +0x30 | **`cISC4MySim* GetActiveUIPanelSim(int32& index)`** | 4 | **EAX pointer** | `0x0042E960` | if the active index is in [0, vector size): `index :=` the **vector index** (not the slot), return that MySim (not AddRef'd); else return null, `index` unchanged. **SDK error** (§7.4). |
| 13 | +0x34 | (destructor) | 4 | | `0x0042F450` | not part of the interface |

Other vtables of the object: message target `0x00A85534` (DoMessage `0x0042EFA0`, pop 4, AL = 1), serializable
`0x00A8551C` (Write `0x0042E440`, Read `0x006B8A70`, GetGZCLSID `0x0042E300`).

The advisor writer's claim (advisors.md §14.3) that CitySituations `UpdateMySimAdvices` calls agent `+0x30` and
then MySim `+0x110` is consistent with this table: slot 12 returns the MySim pointer, and `+0x110` is
`ActivateLuaContext` (§7.2 slot 68).

### 7.2 `cISC4MySim` (sub-object vtable `0x00A85080`, 69 slots)

The compiled order equals the SDK order except that the two `GetJobTitle` overloads are grouped at the position
of the first declaration, in reverse order (foundations §16): slot 27 is `GetJobTitle(occ, str&)` and slot 28 is
`GetJobTitle()`. Therefore **SDK slots 28–54 sit one slot (+4 bytes) later** in the compiled table, and slots
≥ 56 match. This is the MSVC rule, so the SDK header compiled with MSVC/Intel produces the right layout; it is
not an SDK error, but a non-MSVC replacement must lay the table out as below.

| Slot | Off | Method | Pop | Ret | Address | Notes |
|---|---|---|---|---|---|---|
| 0–2 | | QI / AddRef / Release | 8/0/0 | | `0x00429D60`… | thunks to the object |
| 3 | +0x0C | `cISC4Advisor* AsAdvisor()` | 0 | EAX | `0x00429C90` | |
| 4 | +0x10 | `void MoveIn(slot, name&, isMale, sign, bio&, bitmapID, path&)` | 28 | — | `0x0042D880` | §4.1 |
| 5 | +0x14 | `bool FindNewResidence()` | 0 | AL = **0 always** | `0x0042A700` | §4.7 |
| 6 | +0x18 | `bool ChangeResidence(b)` | 4 | AL | `0x00426C10` | §4.7 |
| 7 | +0x1C | `bool TestForResidence(b, bool filter, bool matchWealth)` | 12 | AL | `0x00425CA0` | §4.7 |
| 8 | +0x20 | `bool ChangeJob(b)` | 4 | AL | `0x00425D70` | §4.7 |
| 9 | +0x24 | `bool DeleteFromGame()` | 0 | AL = 1 | `0x0042DF50` | §4.7 |
| 10 | +0x28 | `uint8 GetSlot()` | 0 | AL | `0x0040CE50` | shared getter body |
| 11 | +0x2C | `cISC4BuildingOccupant* GetResidence()` | 0 | EAX | `0x00517F70` | shared getter body, no AddRef |
| 12 | +0x30 | `cISC4BuildingOccupant* GetJob()` | 0 | EAX | `0x0060FB80` | shared getter body, no AddRef |
| 13 | +0x34 | `bool SetResidence(b)` | 4 | AL | `0x0042E020` | §4.1 |
| 14 | +0x38 | `bool GetLocation(float[3])` | 4 | AL | `0x004265D0` | UI/automata |
| 15 | +0x3C | `void GetVehicleKey(key&)` | 4 | — | `0x00424C80` | |
| 16–19 | +0x40… | `GetSign`, `GetAge`, `IsMale`, `GetWealth` | 0 | AL/AL/AL/EAX | `0x00424CA0`, `0x00424CB0`, `0x00424D90`, `0x006AAEB0` | |
| 20 | +0x50 | `bool IsActive()` | 0 | AL | `0x00449550` | state ≠ 0 |
| 21–26 | +0x54… | `GetBio`, `GetBitmapID`, `GetBitmapFile`, `GetBitmapBuffer(cIGZBuffer**)` (pop 4), `GetHomeName`, `GetJobName` | | | `0x00698CD0`, `0x00407AC0`, `0x00424CC0`, `0x0042BDB0`, `0x00425DD0`, `0x00425DF0` | UI |
| 27 | +0x6C | `void GetJobTitle(cISC4BuildingOccupant*, cIGZString&)` | 8 | | `0x00424BD0` | reordered overload |
| 28 | +0x70 | `SC4String* GetJobTitle()` | 0 | EAX | `0x00425E30` | |
| 29 | +0x74 | `GetEQAsText(cIGZString&)` | 4 | AL = 1 | `0x00424CD0` | `"%d"` of int(EQ). SDK `void` (harmless) |
| 30 | +0x78 | `GetHQAsText(cIGZString&)` | 4 | AL | `0x00424D00` | text by HQ band < 50 / < 100 / else (UI) |
| 31–36 | +0x7C… | `Find`, `ShowResidence`, `HideResidence`, `ShowLocation`, `HideLocation`, `SetPylonVisibility(bool)` (pop 4) | | | `0x004293B0`, `0x00429590`, `0x00425E70`, `0x0042C820`, `0x004288D0`, `0x00425F00` | UI |
| 37 | +0x94 | `UpdateResidence()` | 0 | | `0x00425EA0` | visual |
| 38, 39 | +0x98, +0x9C | `LeaveForWork()`, `ReturnHome()` | 0 | | `0x00426EF0`, `0x00426F60` | dispatch trips (automata) |
| 40 | +0xA0 | `AnnounceMoveIn()` | 0 | | `0x0042BF70` | §4.1 |
| 41 | +0xA4 | `DoScriptCommand(uint32)` | 4 | | `0x0042DB50` | §4.8 |
| 42 | +0xA8 | `DoBuildingAdded(b)` | 4 | | `0x00426060` | §5.2 |
| 43 | +0xAC | `DoBuildingChanged(b)` | 4 | | `0x0042C020` | §5.2 |
| 44 | +0xB0 | `DoBuildingRemoved(b, uint32 cause)` | 8 | | `0x0042C940` | §5.2 |
| 45–47 | +0xB4… | `DoFireAdded(x, z)`, `DoPipeBurst(x, z)`, `DoTornado(x, z)` | 8 | | `0x00426120`, `0x004261D0`, `0x00426280` | §5.2 |
| 48 | +0xC0 | `DoDisaster(uint32 type, int32 cx, int32 cz)` | 12 | | `0x00426330` | §5.2 |
| 49 | +0xC4 | `DoNeighborConnectionRemoved(int32, int32)` | 8 | | `0x00426460` | §4.7 |
| 50 | +0xC8 | `uint32 DistanceToClosest(uint32 group)` | 4 | EAX | `0x00427600` | §4.9 (SDK `int32`: same bits) |
| 51 | +0xCC | `uint32 DistanceToClosestSpecial(uint32 type)` | 4 | EAX | `0x00426AE0` | §4.9 |
| 52 | +0xD0 | **`SC4Percentage LocalSchoolGrade(uint32 group)`** (hidden result pointer first) | 8 | EAX = result pointer | `0x00429FE0` | **SDK error** (§7.4) |
| 53 | +0xD4 | **`SC4Percentage LocalHospitalGrade(uint32 group)`** | 8 | EAX = result pointer | `0x0042A200` | **SDK error** |
| 54 | +0xD8 | `PlayNightSound()` | 0 | | `0x00426EC0` | audio |
| 55 | +0xDC | `bool GetCurrentScreenLocationXY(int32*, int32*)` | 8 | | `0x00425F20` | UI |
| 56 | +0xE0 | `RequestToPlaySound()` | 0 | | `0x00424DE0` | audio |
| 57, 58 | +0xE4, +0xE8 | `SetCar(exemplar)`, `SetPedestrian(exemplar)` | 4 | | `0x00426660`, `0x004272B0` | automata |
| 59 | +0xEC | `GetModelButtonBitmapID(bool)` | 4 | | `0x00429CB0` | UI |
| 60–63 | +0xF0… | balloon getters | 0 | | `0x004275E0`, `0x004275F0`, `0x00427410`, `0x00427440` | UI |
| 64–66 | +0x100… | `CanUDriveIt`, `EnterUDriveIt`, `ExitUDriveIt` | 0 | | `0x00426A00`, `0x00426A20`, `0x00426A40` | UI (tail jumps into the dispatch) |
| 67 | +0x10C | `bool CreateDispatchTool()` | 0 | | `0x00426830` | UI |
| 68 | +0x110 | `bool ActivateLuaContext()` | 0 | AL | `0x00429830` | §4.4 |

**Lua callers** (advisors.md §8.3, verified): the `mysim_*` functions take the context advisor, QI it to
`0x0A1DA42F`, read one number argument with `_ftol`, and call: `mysim_distance_to_closest_building` → `+0xC8`,
`mysim_distance_to_closest_landmark_or_reward` → `+0xCC`, `mysim_local_school_grade` → `+0xD0` with
(result pointer, group) and pushes `grade × 100.0`, `mysim_local_hospital_grade` → `+0xD4` likewise,
`mysim_dispatch_tool` → `+0x10C` (wrappers `0x0040A410`, `0x0040A4F0`, `0x0040A5D0`, `0x0040A6B0`,
`0x0040A790`). So the compiled `+0xC4..+0xD4` slots are indeed the SDK slot + 4 bytes.

### 7.3 Other vtables of `cSC4MySim`

| Vtable | Address | Overrides of the base |
|---|---|---|
| `cISC4Advisor` (at +8 of the object) | `0x00A851C0` (21 slots) | 3 Init `0x0042CCF0` (base Init, then dispatch creation, Lua table, tuning §4.11), 4 Shutdown `0x00428C20`, 8 GetPersonalName `0x0042D000`, 19 **CommandWouldTrigger** `0x00424DA0` (false when state = 0; otherwise `ActivateLuaContext` then the base). The rest are the base's (advisors.md §13). |
| `cIGZSerializable` | `0x00A851A4` | Write `0x004251A0`, Read `0x00425490`, GetGZCLSID `0x00424AF0` (= `0x4A1DBBBF`) |
| `cIGZMessageTarget2` | `0x00A85194` | DoMessage `0x0042D150` |
| class (non-interface) | `0x00A85218` (17 slots) | 6 simulate_day `0x0042DC90`, 7 simulate_day_events `0x004286C0`, 8 monthly hook `0x0042DDF0`, 9 update_variables `0x0042CB10`; 10–16 MySim-private |

### 7.4 SDK errors (checked)

1. **`cISC4MySimAgentSimulator::GetActiveUIPanelSim`** (slot 12, checked at `0x0042E960`, pop 4): returns the
   `cISC4MySim*` in EAX (epilogue loads the vector element), not `bool`. The out parameter receives the vector
   index, not the UI slot. A caller using the SDK `bool` reads AL, which is 0 for any 256-aligned MySim pointer.
   Replacement: return the pointer (null when none) and write the index.
2. **`cISC4MySim::LocalSchoolGrade` / `LocalHospitalGrade`** (slots 52/53, checked at `0x00429FE0` and
   `0x0042A200`, pop 8, `ret 8`, EAX = the hidden pointer): the signature is
   `SC4Percentage F(uint32 occupantGroup)` returned through a hidden result pointer, not
   `void F(SC4Percentage&)`. Replacement: write the grade (float32) to the result pointer and return it.
3. Not errors, noted: `FindNewResidence` always returns false; `GetEQAsText`/`GetHQAsText` return AL = 1 where
   the SDK says `void`; the `GetJobTitle` overload order (§7.2); `MySimLivesHere` already returns a pointer in the
   SDK.

---

## 8. Random numbers *[compat]*

| Draw | Generator | Method | Draws |
|---|---|---|---|
| start position in `FindNewResidence` | the MySim's own `cRZRandom` member, seeded from the wall clock at construction (foundations §6.2, a new row: "MySim, per object") | `RandomUint32Uniform(n)` (foundations §6.1) | exactly 1 per call that reaches step 2 (n = 0 → no state change) |

No other simulation-side draw exists in either class. The dispatch, automata, balloons and idle animation
(`PickIdleOffset`) may draw from their own generators (out of scope). The advice scripts' randomness belongs to
the advisor system (advisors.md).

---

## 9. Dependencies

### 9.1 Feedback into the simulation: none *[compat]*

- No simulator reads MySim state. The only engine readers are UI windows, input controls, the query tool's MySim
  icon, the audio handler, the CitySituations advisor (`UpdateMySimAdvices`, which activates MySim missions; UI),
  and the Lua scripts.
- MySims write nothing to the city: no population, jobs, demand, budget, ordinance, grid or lot change. A MySim's
  home and job are references only; the buildings do not know about them.
- `CheckCommute` (`0x0042D9E0`) and `FindNewJob` (via the dispatch's `validateCommuteLocation`, `0x00432F50`) only
  run pathfinder queries: neither calls `ApplyTraffic` (`cISC4PathFinder` slot 29, pathfinder.md §4.17) nor any
  other traffic-volume writer, so **no trip is recorded** and the traffic data is unchanged. `CheckCommute` creates
  its own pathfinder and releases it. The only shared state a query touches is the process-wide work-check
  interval / adaptation counter of pathfinder.md §4.11 (timing, not traffic data).
- Side outputs: advisor events and advices (news, balloons), the `my_sim_died` automata trigger, MySim cars and
  pedestrians, UI/audio messages (§5.1).

### 9.2 Interfaces called (Windows slots)

| Interface | Slot / offset | Method | Pop | Ret | Used in |
|---|---|---|---|---|---|
| `cISC4City` | 33 `+0x84` / 34 `+0x88` | GetLotManager / GetOccupantManager | 0 | EAX | §4.7, §4.9 |
| | 47 `+0xBC` | GetAutomataControllerManager | 0 | EAX | Die |
| | 53 `+0xD4` | GetBuildingDevelopmentSimulator | 0 | EAX | §4.5, §4.9 |
| | 71 `+0x11C` | GetMySimAgentSimulator | 0 | EAX | MySim → agent |
| | 72 `+0x120` | GetDisasterLayer | 0 | EAX | §3.2, §4.6 |
| | 81 `+0x144` | GetAdvisorSystem | 0 | EAX | agent Init |
| | 93/94 `+0x174/+0x178` | CellCountX / CellCountZ | 0 | EAX | §4.5 congestion |
| | 97 `+0x184` | CellCenterToPosition(cx, cz, &x, &z) | 16 | — | §3.2, §5.2 |
| `cISC4ResidentialSimulator` (`0x00AB2C10`) | 21/22 `+0x54/+0x58` | GetAverageEQGrid / GetAverageHQGrid | 12 | EAX | §4.6 |
| | 26/27 `+0x68/+0x6C` | GetSchoolQueryData / GetHospitalQueryData | 8 | AL | §4.9 |
| | 29 `+0x74` | GetCellLifeExpectancy(x, z) | 8 | AL | §4.5 |
| | 37/38 `+0x94/+0x98` | GetCellEQByWealth / GetCellHQByWealth(x, z, w) | 12 | ST0 | §4.1, §4.5 |
| | 41 `+0xA4` | GetAverageAgeMap | 0 | EAX | §4.1 |
| | 42 `+0xA8` | GetAverageNewAgeByWealth(w) | 4 | AL | Spawn |
| | 47/49 `+0xBC/+0xC4` | GetSchoolBuildings / GetHospitalBuildings(list&, groups*) | 8 | (garbage) | §4.9 |
| `cISC4CrimeSimulator` | 5 `+0x14` | GetCrimeGrid | 0 | EAX | §4.5, §4.6 |
| `cISC4PollutionSimulator` | 5 `+0x14` | GetPollutionGrid(type) | 4 | EAX | §4.5, §4.6 |
| `cISC4AuraSimulator` (`0x00AB07C8`) | 12 `+0x30` | GetAuraGrid | 0 | EAX | §4.6 |
| `cISC4TrafficSimulator` | 6 `+0x18` | CreatePathFinder | 4 | EAX | §4.7 |
| | 10 `+0x28` | GetCongestionMap | 0 | EAX | §4.5, §4.6 |
| | 24 `+0x60` | GetTravelTimeRatio(x, z, type) | 12 | ST0 | §4.6 |
| `cISC4PowerSimulator` | 31 `+0x7C` | IsOccupantPowered | 4 | AL | §4.5 |
| `cISC4PlumbingSimulator` | 9 `+0x24` | IsBuildingWatered | 4 | AL | §4.5 |
| `cISC4PoliceSimulator` / `cISC4FireProtectionSimulator` | 5 `+0x14`; 21 `+0x54` / 22 `+0x58`; `+0x1C` / `+0x20` | coverage grid; station list; station funding | | | §4.5 |
| `cISC4BuildingDevelopmentSimulator` | 14 `+0x38`, 32 `+0x80`, 44 `+0xB0` | exemplar key of a building type; GetDeveloper(type); special-building list | 8 / 4 / 0 | EAX | §4.5, §4.9 |
| `cISC4BudgetSimulator` | 51 `+0xCC` | GetBudgetItemForPurpose(holder, purpose, item&) | 12 | EAX | §4.6 |
| `cISC4LotManager` (IID `0x681905BD`) | 7 `+0x1C`; 14 `+0x38`; 5 `+0x14` | GetLotsInCellRect; GetOccupantLot; GetLot(x, z, …) | 24 / 4 / … | AL / EAX | §4.6, §4.7, §4.9 |
| `cISC4OccupantManager` | 38 `+0x98`; 40 `+0xA0`; 48 `+0xC0` | occupant queries (all / in box) with a filter; release list | 20 / 20 / 4 | AL | §4.7, §4.9 |
| `cISC4Lot` (`0x00AB19B0`) | 21 `+0x54`, 28 `+0x70`, 35 `+0x8C`, 41 `+0xA4`, 48 `+0xC0` | GetLocation, HasRoadAccess, GetCondition, GetOccupantWealth, GetBuilding | | | §4 |
| `cISC4BuildingOccupant` | 3, 4, 9, 11 | AsOccupant, GetBuildingType, GetCompletionPercent, GetBuildingProfile | 0 | EAX | §4.9 |
| `cISC4Occupant` | 8 `+0x20`, 22 `+0x58` | GetPosition, IsOccupantGroup | 4 | AL | §4.9, §4.10 |
| `cISC4DisasterLayer` (`0x00A9E390`) | 10 `+0x28`, 16 `+0x40`, 17 `+0x44` | IsADisasterOngoing, location count, location by index | 0/0/16 | AL/EAX | §3.2, §4.6 |
| `cISC4SimGrid<T>` | 7, 8, 19, 22 | GetCellValue, GetAverageValueInCellRect, PositionToTract, GetTractValue | 8/16/16/8 | T | §4.5, §4.6 |
| advisor system / `cSC4Advisor` | advisors.md | GetAdvisors map, `PushEvent` (advisor slot 18), `CommandWouldTrigger` (19), Lua state | | | throughout |

Shared grid helpers of simgrid §7 are **not** used. Grids are read only through `GetCellValue`,
`GetAverageValueInCellRect` and `PositionToTract` + `GetTractValue`, never through `GetGridData` (simgrid §6.4:
the MySims own no grid).

---

## 10. Replaceability

- **Agent simulator**: replaceable through the class registry (`0x29E37DEC`). It has no save data. A replacement
  must keep the slot table of §7.1 (CitySituations calls slot 12 and uses the returned pointer; the UI calls
  slots 5–12) and must forward the messages of §3.2 with the same field use. Nothing reaches into its memory.
- **MySim**: replaceable per class (`0x4A1DBBBF`) through the Lua `advisors` table's class id, but a replacement
  must also be a full `cSC4Advisor` (advisor slot table, record format, Lua context) because the advisor system
  owns, ticks and saves it. The UI windows and the Lua wrappers call `cISC4MySim` slots by offset (§7.2).
- **Blockers**: (1) the Lua state shared with the advisor system (advisors.md §14.2); (2) `cSC4MySimDispatch` and
  the input controls call non-virtual MySim members (the dispatch notifies the MySim by message, but the dispatch
  tool and U-Drive-It code reach the MySim directly *[inferred]*); (3) the record format of §6.2 if old saves
  must load.
- Nobody reads MySim or agent memory by offset outside these classes *[inferred: no raw offsets found]*.
- *[choice]* FreeCity4 implements the simulation half (state, events, Lua variables, persistence) and leaves the
  dispatch, balloons and automata as stubs that report "arrived" immediately, which keeps the observable Lua
  variables except the trip fields.

---

## 11. Quirks (reproduced) and open questions

### 11.1 Quirks

1. MySims never change any simulation result (§9.1).
2. `FindNewResidence` always returns false (§4.7); the MoveOut script command finds a new home rather than
   leaving.
3. `ChangeResidence` reverts the move when a move-out advice would trigger at once (§4.7).
4. Missing sources leave stale Lua fields (EQ ≤ 0, desirability with wealth outside 1–3, §4.5).
5. Riot/volcano event subjects carry cell indices as their location (§5.2).
6. Grid reads in trip variables have no bounds check (§4.6); `force` resets the cell to (0, 0) before reading.
7. The local building map treats distance 0 as unset (§4.9).
8. `GetActiveUIPanelSim` returns a pointer and reports the vector index (§7.4).
9. The neighbour job city is not saved (§2.2).
10. The agent's slot index is unbounded (§1.3).
11. `MySim::Read` accepts any version (§6.2).
12. Ageing uses the advisor month counter and a birth month fixed at move-in, so a MySim ages once per 12 monthly
    steps regardless of the calendar.

### 11.2 Open questions

1. *Answered (MS1, 2026-10-21):* row 25 is dead data, row 30 is the pylon-locked flag (§2.2).
2. *Answered (MS2, 2026-10-21):* window-manager slot 42 = "a modal window is running" (§4.2).
3. *Answered (MS3, 2026-10-21):* yes, see §2.2.1.
4. *Answered (MS4, 2026-10-21):* groups `0x150A`/`0x150B` (landmarks and rewards), §4.9.
5. *Answered (MS5, 2026-10-21):* traced on Windows, §4.5 (border ring only; agriculture jobs count as watered).
6. *Answered (MS6, 2026-10-21):* no traffic-data side effect (§9).
7. *Answered (MS7, 2026-10-21):* the check is the `RequestToPlaySound` throttle (§4.1); states 4/5/6 in §2.2.

---

## 12. Conformance tests (must exist in `engine/conformance`)

1. **Ownership.** New city with the shipped advisors table: the agent's vector holds MySim advisors 1–5 at
   indices 0–4 and the My Street sim at index 5. `get_count()` = 6. The city save contains no record for
   `0x29E37DEC`.
2. **Move-in age.** Average-age grid value 12 at the home cell → age 18; value 40 → age 40.
3. **Move-in EQ ring.** `GetCellEQByWealth` = 87.5 at move-in (ring all 87.5, index 0). The next seven monthly
   steps see EQ 90.0, 95.0, 100.0, 100.0, 100.0, 110.0, 110.0. Step k (1-based) replaces ring entry
   (k − 1) mod 6: after step 6, eqDelta = 110.0 − 87.5 = 22.5 → `delta_eq` = 22; after step 7 (replaces entry 0,
   which holds 90.0), eqDelta = 20.0 → `delta_eq` = 20.
4. **EQ ≤ 0 is ignored.** EQ 50.0, then the residential call returns 0.0 → `eq` stays 50.
5. **Ageing.** Birth month 3; monthly steps with month counters 14, 15, 27 → age +1 at 15 and 27 only.
6. **Job search.** No job, state 2, `FindNewJob` false on 3 consecutive days → `unemployed_days` = 3; a job found
   → 0 and event `0xAA3EDBB1` pushed.
7. **Home destroyed (no disaster).** Remove the home building with data4 = 0 → event `0xAA48F5C1`, then one
   `RandomUint32Uniform(n)` draw on the MySim RNG (seed 1, n = 4 → first draw `(0x000041C6 × 4) >> 32` = 0 → start
   at the first candidate).
8. **Home destroyed by disaster.** Disaster period started with data4 = `0xCA380B06`; remove the home with data4 = 4
   while a disaster is ongoing → `last_local_disaster` = `0xCA380B06`, event `0xAA4D2D64`, residence null, state
   unchanged.
9. **Job destroyed.** Remove the job with data4 = 0 → event `0xCA48F879`, `employed` = 0, state 2.
10. **Script commands.** AdviceCommand `0xEA4D269C` for MySim 2 → its state 0, `MySimsChanged` posted, automata
    trigger `my_sim_died` requested once; other MySims unchanged. `0x6A4A5E9E` with wealth 3 → no change; with
    wealth 1 → wealth 2, job dropped, one `FindNewResidence`.
11. **Spawn.** Wealth 2, `GetAverageNewAgeByWealth(2)` = 30 → age 30, generation +1, retired 0, bio empty.
12. **Building added.** Home position (100, 0, 100), new building at (130, 0, 140) → event `0xCA4E8986`, subject
    data1 = 50.
13. **Riot subject.** `DoDisaster(0xCA380B06, 10, 20)` → subject location (10.0, 20.0), data1 = distance from the
    centre of cell (10, 20); `DoDisaster(0x621CDA33, …)` → no event.
14. **Trip percentages.** 3 lots with buildings in the square, conditions {2, 2, 4}, wealth {1, 2, 3}:
    r = float32(1 / 3.0001) → distress1 = int(2 × r × 100) = 66, distress2 = 0, distress3 = 33, each wealth band
    = 33.
15. **Local distances.** Buildings of group G at distances 0 (home) and 37 inside ±20 cells → `DistanceToClosest(G)`
    = 37; nothing of group H → `0xFFFFFFFF`.
16. **School grade.** Two schools of group G at 120 m and 80 m with grades 0.6 and 0.9 → `LocalSchoolGrade(G)`
    writes 0.9 to the result pointer and returns it; Lua `mysim_local_school_grade(G)` = 90.
17. **GetActiveUIPanelSim.** MySims at indices 0–4 with slots 4, 3, 2, 1, 0: `SetActiveUIPanelSim(1)` →
    `GetActiveUIPanelSim(i)` returns MySim index 3, i = 3. `SetActiveUIPanelSim(7)` → null, i unchanged.
18. **Persistence round trip.** A MySim with every field set writes a version-17 record whose bytes match §6.2;
    reading it back and writing again gives identical bytes. A state-0 MySim writes `11 00 00` followed by the
    advisor part.
19. **Old version.** A version-14 record (five discarded s32 instead of the keys, no rows 25 and 31) loads with
    car/pedestrian keys zero and last player trip day 0.
20. **No feedback.** Run 24 months with and without five MySims placed (same seeds for every simulator): every
    simulator record except the advisor system's is byte-identical.
