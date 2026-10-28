# traffic-simulator: answered questions (archive)

## TE33-ACCIDENT-1: accident timing and draws under the fixed schedule

The traffic replacement now completes each trip phase under simulator §5.8's
available deadline. In `qcheck-3.log` of worktree `traffic-exact33`, every scenario's
first rand difference moves later. The smallest case, `paradis-arlert`, first differs
on 2005-02-01 delivery 49, immediately after the replacement's monthly traffic call
(48); its January monthly rand stamp agrees. No DIGEST differs; six save records differ.
The original-only control on that scenario is exact (`qcheck-4.log`). Replacement
evidence: main `build-runs/golden-runs/compare-20261028-015824-403ed8e8/result/`.

§9.1 TG2 permits leaving the accident clock, view, occupant and creation callbacks
unimplemented, saying this loses only visible crashes. Under the fixed schedule,
§4.13/§4.15's accident iterator also changes shared rand() and the traffic generator.
Please reconcile that omission permission with exact replacement, specifying the
accident timer's units, initial deadline, load reset, comparison and rescheduling
rule under simulator §5.8 (including zero entries and an unavailable view). §4.13
calls the timer milliseconds but also describes a seconds-based replacement.
Please include the monthly draw counts and the public view/automata interface
facts needed to supply the remaining callbacks. This is a suspected missing-draw
boundary, not a claim that all six save differences have the same cause.

Additional fixed-schedule evidence (traffic-exact33 round 10, `qcheck-32.log`):
Arlert Julian day 2453403, just before delivery 49, has original CRT state
`0xD4E5FED6`, copy `0x2D37894F`. By simulator §5.8's delivery-48 seed and
foundations §12.1, the copy state is the seed unchanged, and the original
state is exactly one `rand()` advance from it. On January 1 (2453372), both
states before delivery 49 are `0x05EF54D8`, with zero advances. The states
before delivery 48 also agree on both dates. The reference is the game.log
of golden key `22ee02cc159e865de055ed32a1bb83588538a76a5f69cb0423d08c6bf647b29d`;
replacement run is main `build-runs/golden-runs/compare-20261028-090319-ba319f0e/result/`.
This narrows the missing-draw boundary but does not prove which timer or
iterator took it. Please specify the accident timer's first-month behavior
and counter-read sequence as well as its deadline: omitted reads can also
change simulator §5.8's shared virtual clock. No compensating draw was added.

Round 11 reproduces the same Arlert boundary (`qcheck-36.log`, DLL
`f33a3897`, run `compare-20261028-095746-c95e41e5`). Its schedule month lines
report 22 frames at January 1 and 54 at February 1, with no sleeps: frame
contributions of 726 and 1,782 ms, plus the virtual counter's per-read ticks.
The copy has 16 accident-check entries in January and 11 in February.
This does not measure the original's list or establish its timer deadline.
Please include the initial next-check value and whether the period's
numeric value 60 is compared in ms or seconds; §4.13 currently names both.

Round 12 extends the boundary measurement to all eight scenarios, on the
same final DLL (`b5cba0a0`, `qcheck-39.log`, 0/8 exact, all clean/ref0/stall0).
Using simulator §5.8's preceding-delivery seed and foundations §12.1's CRT
recurrence, the first mismatching pre-delivery stamp gives these counts:

| Scenario | Julian day / next delivery | Reference state / draws | Replacement state / draws |
|---|---|---|---|
| gen28-tegel-s1-churn | 2462381 / 49 | `241661D8` / 1 | `67A1CAF9` / 0 |
| maxisland-bendburger | 2468620 / 50 | `9A98A186` / 1 | `F3BB3DBF` / 0 |
| maxisland-bevopolis | 2545564 / 50 | `7F8EC636` / 1 | `ED8B1C2F` / 0 |
| maxisland-caramel-island | 2465364 / 49 | `80FE0573` / 1 | `FF3CC870` / 0 |
| maxisland-san-indersty | 2508736 / 49 | `7A8CEFD7` / 1 | `5E79BFA4` / 0 |
| paradis-arlert | 2453403 / 49 | `D4E5FED6` / 1 | `2D37894F` / 0 |

All six immediately preceding deliveries target the traffic message
interface (`0x00AB33BC`), with equal pre-call rand stamps. Thus the one-draw
boundary is shared, rather than specific to Arlert's ten map cells. It still
does not identify which operation took the draw or establish the timer rule.
The other two scenarios first differ after a different receiver: Braus
2481982/21 gives 9 reference versus 24 replacement draws; Jinae 2468848/21
gives 11 versus 17. Their route-order evidence remains in TE33-PATH-1.
Evidence is the `game.log` files of the eight result directories in
`qcheck-39.log`; `build-runs/r12-boundaries.py` reproduces these counts from
those logs and the reference key in each result's `comparison.json` only.

**ANSWERED 2026-10-28 (re-blockers37a):** DIFFERS — whole-second unit-5 stopwatch, initial deadline zero, strict expiry, and one CRT reset draw only for a due nonempty list. Exact fixed-schedule replacement must preserve the counter read and both RNG streams; the TG2 omission permission is withdrawn. See traffic-simulator §§4.13.1–4.13.2 and §9.1.

<!-- Archive for [traffic-simulator.md](../../traffic-simulator.md). Lanes do not read this file. Open questions are in ../traffic-simulator.md. -->

## From glue-traffic-flake5 Round 2 (2026-10-23)

<!-- from: glue-traffic-flake5 Round 2 (2026-10-23) -->
- **TS68 - §3.4 / §4.4, split-search continuation at a work-slice return:** What is the lifetime of `continueSearch`? In particular, when FullTrips splits a routed trip and the deadline ends the call before its inserted status-0 remainder is searched, does the next invocation (which §3.4 says walks the held source's list again from its head) retain `continueSearch = true`, or start with it false? Please specify its storage/reset boundaries (invocation, source, phase, day, and network change) and the case where the remainder's search itself suspends. [Current implementation stores the bool in State, retaining it across slices/deliveries, and sets pathfinder flag 4 when searching the remainder. The spec defines setting it on split/success/failure, but does not explicitly define the call-return boundary. Do not change that behavior on the strength of this question alone.]
  **ANSWERED 2026-10-23 (re-traffic-ts68):** MATCHES — continuation is stored at simulator `+0x881`,
  retained across unfinished calls and daily deliveries, cleared on taking a source, unsplit completion,
  failure or DoConnectionsChanged. Phase entry arms taking a new source rather than clearing the byte
  directly. Suspension retains the byte and the separate search state; resumption skips SetupTrip and
  continues the suspended leg. Written into §4.4 with address evidence and §11 test 18 (TS68-a–e).

  Motivation/evidence: after the separate iterator-sentinel correction, `traffic-flake5-4883-turbo1 72/72` is exact to 2032, while normal run `traffic-flake5-4883-normal1 70/73` has a 2028-03-28 FullTrips 18 -> 2 difference: lot (31,89) trip 0 is routed with orig count 3/destination (28,55), ours count 55/destination (12,57); both lists have 18 entries. Edge densities, a second lot's route bytes, and volume grids also differ, while holder properties agree. This is a candidate timing-sensitive contract to investigate, **not a proven diagnosis**. Restarting a pathfinder versus continuing its old open set can change routes, so §3.4's deterministic budget choice alone is insufficient grounds for guessing this boundary. Diagnostic commit 8c36e30a records the pre-delivery cursor/continuation context on subsequent failures; its native build was still queued at filing. No mismatch is masked or excluded.

## From the traffic simulator core (filed by the coordinator from the lane's report, 2026-10-14; merged)

About `docs/specs/traffic-simulator.md`. All §11 tests 1 to 15 are unit tests and `ts11_*` vectors (29 vectors).
TN1, TN3, TN14, TN15 and TN18 hold against the merged network core. The core's answer is in brackets.

<!-- from: From the traffic simulator core (2026-10-14) -->
- ~~**TS1 — §3.3:** does the pacing reset keep `rerouteBucket` (pass counter 2)? [kept; the other counters are
  zeroed]~~ **ANSWERED 2026-10-14** from the exe (`0x0072180D`–`0x00721881`): `rerouteBucket` is kept, and so is
  pass counter 1 (the re-route counter), which is zeroed on entering 17 instead (`0x007218F2`). Everything else is
  zeroed on entering 16: processed, day index, `toReRoute`, `toSearch` and the other counters, including the
  source counter. That counter therefore restarts at 0 every cycle. Written into §3.3 and §4.3.

<!-- from: From the traffic simulator core (2026-10-14) -->
- ~~**TS2 — §3.2 state 11:** what does UpdateNeighborConnections (`0x00714BE0`) do? [nothing]~~ **ANSWERED
  2026-10-14** from the exe: not nothing. It is traffic-network.md §2.7.5 (written with PF6 after the core was cut;
  re-checked against `0x00714BE0`): per neighbour connection, copy `0xAA9EA81A + i` to `0xAA9EA81D + i` and zero it
  (i = 0…2), then arrived-last := arrived-this-cycle and arrived-this-cycle := 0 in `0x8A0B4100` (k = 0…3). One call;
  pointer added to traffic-simulator.md §3.2.

<!-- from: From the traffic simulator core (2026-10-14) -->
- ~~**TS3 — §4.8 state 3:** what does ReduceTransitSwitches do? [the switch load restarts from 0; CreateMaps keeps the
  previous value in counter 1]~~ **ANSWERED 2026-10-14** from the exe (`0x00713750`): the bracket holds for the
  shipped tuning. Exactly: every switch's `load := r24(densityReduction × load)` with the volume-reset factor
  `0x29136788` (shipped 0), nothing else; counter 1 (`prevUsage`) and the ratio are CreateMaps' (§4.9 step 4). Use the
  factor, not a literal 0 (a tuning mod decays the load). Written into §4.8.

<!-- from: From the traffic simulator core (2026-10-14) -->
- ~~**TS4 — §3.1:** what do DoRoadChanged / DoRailChanged do on occupant messages? [on insert the cell joins the damage
  block's intact set unless present; on remove it leaves]~~ **ANSWERED 2026-10-14** from the exe (`0x007155D0`,
  `0x00715790`, `0x0070EA70`, dispatch `0x00724334`): the bracket holds for the sets, but not for the counts. Insert
  adds the key and does `cellCount += 1` even when the key is already present. Remove erases from the intact set,
  or else from the damaged set. A damaged removal does `damagedCount −= 1`, pothole count −= 1, and posts
  `0x69247DC8 (x, z, 0)`. `cellCount −= 1` in every remove case. Highway and GroundHighway skip the sets; rail skips
  them on a level crossing (Road/Street flag). A rail repair decrements the road pothole count *[quirk]*. The
  per-type counts are bumped and the budgets refreshed. Written as traffic-simulator.md §3.6.

<!-- from: From the traffic simulator core (2026-10-14) -->
- ~~**TS5 — §3.1:** message `0x66956810` "marks the network as changed": which flag, and who rebuilds? [sets
  `network_changed`, consumed by the trip phases]~~ **ANSWERED 2026-10-14** from the exe (`0x00724558`) and the Mac
  build: the bracket is wrong on the consumer. SimBegin and every network-occupant message (data3 `0xC772BF98`, even
  a failed cast) set one byte, which is saved and loaded (the `bool` after the intersection-dirty rectangle) and
  read by nothing. No rebuild follows; the network is kept current incrementally and rebuilt only by
  DoPostCityInit. Keep the byte for the round trip, with no behaviour. Written into traffic-simulator.md §3.1 and
  §6.1, traffic-network.md §3, save-records.md §7.1.1.

<!-- from: From the traffic simulator core (2026-10-14) -->
- ~~**TS6 — §3.1:** what does the network part do on neighbour added / removed? [nothing]~~ **ANSWERED 2026-10-14**
  from the exe: not nothing. It is traffic-network.md §2.7.2 (written with PF6; re-checked against `0x007153C0` and
  `0x00713AB0`, args data1 type, data2 x, data3 z). Add: `FindConnection`, type bit set, trip cost raised to the
  other side's `0xCA947077` (ordered compare), one pool per established city. Remove: bit cleared; on an empty mask,
  pool 0, the pool sweep and `freeDest` decrements for the destination byte. Pointer in traffic-simulator.md §3.1.

<!-- from: From the traffic simulator core (2026-10-14) -->
- ~~**TS7:** DoPostCityInit steps 4 (neighbour connections) and 5 (destination-map boundary pass)? [skipped; the
  network rebuild runs, then the transit multiplier]~~ **ANSWERED 2026-10-14** from the exe (`0x00720430`): neither
  is skipped. Step 4 is an Add per neighbour connection plus one pool sweep, step 5 zeroes the destination byte of
  each ring cell without a map exit (traffic-network.md §1, §2.7.2–§2.7.3). DoPostCityInit also does more than the
  network rebuild and the multiplier: state 12/13 → 8, a resume switch on the loaded state (source iterator, lot
  iterator, ferry re-route **only in state 19**), the strategy-bucket multimap rebuilt from the lots,
  `days := lotCount div 18`, and both department-cost updates. Written as traffic-simulator.md §3.5; §3.3 gains the
  missing `dayIndex < days` guard; traffic-network.md §1 step 8 and §4.9.3 corrected.

<!-- from: From the traffic simulator core (2026-10-14) -->
- ~~**TS8 — §4.9 step 2:** what is the commercial traffic window? [the 4 × 4 block at (x, z); travel types car 1,
  el-train 7, monorail 8; maximum morning usage plus maximum evening usage]~~ **ANSWERED 2026-10-14** from the exe
  (`0x00714AA0`, `0x00714940`): partly. The window is a plus shape: the block widened by 2 cells each way without
  the corners, up to 48 cells, clamped at 0 and at H, but **not** at W on the block rows (x mod 256: the east
  column of a 256 city reads x = 0, 1 *[quirk]*). The per-cell value is the max of road (Car + Bus + FreightTruck,
  u16 sum), el-train and monorail `usage`. `v = (maxMorning + maxEvening) mod 2¹⁶`, rounded ties-to-even after
  the scale. Written into traffic-simulator.md §4.9 step 2 with an example.

<!-- from: From the traffic simulator core (2026-10-14) -->
- ~~**TS9 — §4.9:** which records are "not excluded" from the statistics? [flag bit 0 clear]~~ **ANSWERED
  2026-10-14** from the exe (`0x0071A3A6`–`0x0071A496`, end `0x0071A5D7`) and the advisor: none is excluded. A
  record with flag bit 0 (network flag `0x40000`) goes to slot 13, which the traffic advisor publishes as **bridge**.
  Others go to slot `type`. Pedestrian records count too, so a road cell adds 2. At the end of the pass the per-type
  record counts also **replace** the cell counts that price the road and transit budgets; bridges drop out. A
  read-only corpus check agrees (saved cell counts = published counts; road counts = 2 × road keys). Written into
  traffic-simulator.md §4.9, §3.6 and §6.1 row 16, and save-records.md §7.1.1.

<!-- from: From the traffic simulator core (2026-10-14) -->
- ~~**TS10 — slot 7 SetupPathFinderForLot:** what else is configured? [the lot rectangle as start, plus SetupTrip
  steps 3–4 only]~~ **ANSWERED 2026-10-14** from the exe (`0x00711610`): more than the bracket.
  * What it sets: SetupTrip step 4 and the start rect, without the work time. Also destination filter 0 with
    destination flag `1 << ` the **first** trip's type. Then, for commuters, the 0x1CF/0x3 travel types and the
    starting-cost array of the dominant strategy (by summed trip counts, ties to the lower index); for freight,
    flag 1 and types 0x30.
  * It returns false when `pf` or `lot` is null, when the lot has no trips even after one `UpdateTrafficVolume`, or
    for a type > 3.
  * It never sets density, work time, flag 4 or a forced start.
  Written into traffic-simulator.md §4.4.

<!-- from: From the traffic simulator core (2026-10-14) -->
- ~~**TS11 — slot 35 GetTransitSwitchQueryData:** the data fields? [returns the network core's query target only]~~
  **ANSWERED 2026-10-14** from the exe (`0x0071CB80`, field names from the Mac query readers): five u32/f32 fields,
  available capacity, maximum capacity, load fraction (f32), actual usage, monthly maintenance cost. A switch gives
  its capacity, ratio and previous load. A ferry terminal gives its last-period riders three times, and 1.0. A strike
  zeroes all but the maximum capacity (switch) and the cost. The cost is the purpose-`0xCA550301` budget item's
  cost × department funding, truncated. Written as traffic-simulator.md §4.16 with an example.

<!-- from: From the traffic simulator core (2026-10-14) -->
- ~~**TS12 — §4.12:** what does RecalculateNeighborConnectionCapacities do? [nothing]~~ **ANSWERED 2026-10-14** from
  the exe: not nothing. It is traffic-network.md §2.7.4 (written with PF6; re-checked against `0x00714E60`: the
  demand nodes `0x2010/0x2020/0x2030` at `0x10000` truncated, `GetConnectedCities(list, false)`, per-pool
  `trunc(r24(r24(E/T) × J))` with unsigned `T` and `J`, slot-19 writes, then the edge pass that sets destination bits
  and counts `freeDest`). Pointer added to traffic-simulator.md §4.12.

<!-- from: From the traffic simulator core (2026-10-14) -->
- ~~**TS13 — §4.5:** a route with an off-map root is encoded as the in-city cell while its unit steps are relative to
  the off-map cell. [decoded literally: shifted by one cell]~~ **ANSWERED 2026-10-14** from the exe: unreachable in
  the original. Roots are seeded on rings clamped to the map, so a neighbour start `(−1, z)` roots at `(0, z)`, and the
  ferry fallback roots on the arrival cell. The reader (`0x006D64B0`) decodes the header bytes zero-extended and
  literally, with no off-map adjustment, so the bracket is what the original would do. Keep it *[choice]*. Written
  into traffic-simulator.md §4.5.

<!-- from: From the traffic simulator core (2026-10-14) -->
- ~~**TS14 — §4.3:** what does ConfirmRoute check? [during a strike any route using types 2, 3, 6, 7 or 8 fails; each
  unit step leaving an in-city cell needs an exit on that side for its type, or the cell's switch for a type change;
  jumps and off-map steps are not checked]~~ **ANSWERED 2026-10-14** from the exe (`0x0070E320`): partly. The
  strike test is not by travel type. During a strike, any type change after the first cell fails, and so does a step
  that the connection word refuses at a ferry cell. A route that keeps one type passes. The unit-step test is
  `conn[t]` bit `exit + 4·entry` (any entry at the first cell). If that bit is clear, the cell passes on the
  tunnel wormhole bit, or on the ferry bit when there is no strike. Jumps are checked too: the cell before a jump
  gets an exit side from the jump's direction, with a `+z` quirk for short and same-cell jumps. The first cell has
  no strike test, the last cell is never tested, and an empty route is false. Written into traffic-simulator.md §4.3
  with an example.

<!-- from: From the traffic simulator core (2026-10-14) -->
- ~~**TS15 — §4.5:** is the switch fee the switch's `0x0BFC0A4C` value read as f32? [yes]~~ **ANSWERED 2026-10-14**
  from the exe: yes. DoLotActivated reads it with the converting f32 getter (`0x00723C43`; absent → 0.0), and
  ApplyRoute charges `r24(Q × toll)` into the fees (`0x00719CCC`), which become the toll income. Shipped only on the
  four toll booths (0.1). The saved f32 after the rules is this value. Written into traffic-simulator.md §4.5, §6.1
  and traffic-network.md §2.4.

<!-- from: From the traffic simulator core (2026-10-14) -->
- ~~**TS16 — §4.5:** the ferry-leg counters? [round(Q) to counter 1 (mode 0), 2 (mode 1) or both (mode 2), on the
  departure terminal's link to that cell, else its neighbour link]~~ **ANSWERED 2026-10-14** from the exe
  (`0x00719E8F`–`0x0071A054`): partly. Neighbour links are as bracketed (current A, current B, both). On a terminal
  link, mode 2 adds current A to the forward link and current B to the other terminal's link back. The departure
  terminal's riders also get `Qi` (twice in mode 2). The fare goes to the income, not the fees. The u16 counters
  wrap. A crossing in from a neighbour is counted at the first cell (`0x00719A4E`). Written into
  traffic-simulator.md §4.5 and traffic-network.md §2.6.

<!-- from: From the traffic simulator core (2026-10-14) -->
- ~~**TS17 — §4.4:** a highway-return adjustment for an off-map destination? [none]~~ **ANSWERED 2026-10-14** from
  the exe (`0x0071C858`–`0x0071C987`, `0x0070DD30`): the bracket does not match. The return leg's start cell moves
  one cell along the edge when pf ended off-map by car or bus at a Highway/Avenue/GroundHighway connection (mask
  `0x1108`). Its end cell moves the other way when pf started off-map at such a connection (any type). The
  direction follows the language's driving side (UK English and Japanese drive on the left). Also a *[quirk]*:
  with no lot at an in-city end cell, pf2's start rect is not written. Written into traffic-simulator.md §4.4.

<!-- from: From the traffic simulator core (2026-10-14) -->
- ~~**TS18 — §4.2:** when is the capacity property promoted from u16 to u32? [u16 until a value reaches 0xFFFF]~~
  **ANSWERED 2026-10-14** from the exe (`0x0070DE70`): yes. It is created as 12 u16 zeros, and a write of a value
  `≥ 0xFFFF` (0xFFFF included) copies all 12 elements into a u32 array, which never reverts. Any other variant type
  is accessed as u32 with no count check (*[choice]*: treat it as missing). Also found: ApplyTrafficToDestination
  stores the pool's new arrived total into the found connection's own `0x8A0B4100`. Written into
  traffic-simulator.md §4.2.

<!-- from: From the traffic simulator core (2026-10-14) -->
- ~~**TS19:** does the pathfinder adapter's GetLot match any cell of a lot? [yes (`false`)]~~ **ANSWERED
  2026-10-14** from the exe: yes. All three pathfinder sites pass false (`0x006D7AC7`, `0x006D8688`, `0x006D8B1D`).
  `GetLot` (`0x0072B410`) with false answers at any cell of the lot; with true it answers only at the lot's origin.
  In the simulator, only ClearTripDestinations, RefreshCommuteMap's first lookup and UpdateSubnetworkInfo pass true.
  That corrects §4.11: each lot is rolled and counted once, not once per cell. Written into traffic-simulator.md
  §4.11 and §8.

<!-- from: From the traffic simulator core (2026-10-14) -->
- ~~**TS20:** is the traffic-source list a snapshot? [yes: lots, then connections, when the phase is entered]~~
  **ANSWERED 2026-10-14** from the exe (`0x007107A0`, `0x007107F0`): only for connections. `Reset` (entering 16, 17,
  18, and the load resume) refills a ref-counted copy of the connection list. Lots are walked live through the lot
  manager's safe iterator, so a lot removed before it is reached is skipped and a lot added during the phase is
  visited (appended). Written into traffic-simulator.md §4.1.

<!-- from: From the traffic simulator core (2026-10-14) -->
- ~~**TS21:** when are `toReRoute` / `toSearch` zeroed? [on entering state 16]~~ **ANSWERED 2026-10-14** from the
  exe: yes, only there (`0x0072184B`); Read restores the saved values. But `toSearch` also grows after
  ConfirmTrips. ReRouteTrips adds 1 per trip sent back for lack of capacity (`0x0071D20D`), and FullTrips adds 1 per
  split, together with the running phase's `total` (`0x0071D75A`). Written into traffic-simulator.md §3.3 and §4.4.

<!-- from: From the traffic simulator core (2026-10-14) -->
- ~~**TS22:** does ReduceEdgeDensities also clear ferry terminal cells? [yes, departure and arrival; missing records
  are not created]~~ **ANSWERED 2026-10-14** from the exe (`0x007110B0`): yes, and missing records are not created.
  The departure cell is cleared by the cell scan, because the ferry phase of every rebuild gives it network-info
  records. An explicit pass after the scan (no deadline test) clears each terminal's arrival cell. Written into
  traffic-simulator.md §4.8.

<!-- from: From the traffic simulator core (2026-10-14) -->
- ~~**TS23:** what does DoLotDeactivated do? [destination bytes over the lot := 0, its bucket entries removed,
  `freeDest` unchanged]~~ **ANSWERED 2026-10-14** from the exe (`0x0071E2F0`): partly. The bytes over the rect do go
  to 0, but only the one bucket entry matching the lot's location is erased. `freeDest[k]` is **decremented** for
  each bit set at the rect's NW cell, unless in state 10. It also removes the switch (with a rebuild), the
  `0xCA550301` budget item (`E −= cost`), the `0x0C3BF549` item and the ferry terminal. Written into
  traffic-simulator.md §4.1.3.

<!-- from: From the traffic simulator core (2026-10-14) -->
- ~~**TS24:** is the direction towards a non-adjacent rectangle defined? [yes, always]~~ **ANSWERED 2026-10-14** from
  the exe: yes, for both rect cases: the first cell's entry towards `R` (`0x00719B2D`) and the mode-1 last exit
  towards `sourceRect`. In modes 0 and 2 the last exit points at the cell `(destX, destZ)` and is defined only one
  step away. Also: `R` is the destination's rect in mode 1. Other sides are the reader's. An undefined side at a
  non-last in-city cell ends ApplyRoute and loses the route's income and fees (*[quirk]*, rare). Written into
  traffic-simulator.md §4.5.

<!-- from: From the traffic simulator core (2026-10-14) -->
- ~~**TS25:** when is "the last switch seen" on a route updated? [at every in-city cell, so leaving and re-entering a
  station counts again]~~ **ANSWERED 2026-10-14** from the exe (`0x00719C8A`–`0x00719CDB`): yes. `last` starts at 0
  and takes every in-city cell's ID, 0 and the subway-skip case included, so a re-entry counts again. The subway
  exception compares the cell's type with the **next** record's type, not the previous one. At the last cell that
  record is the cell itself. Written into traffic-simulator.md §4.5.

<!-- from: From the traffic simulator core (2026-10-14) -->
- ~~**TS26:** FinishTripMap donor search order? [distance first, then −x, −z, +x, +z]~~ **ANSWERED 2026-10-14** from
  the exe (`0x00710016`–`0x007101D7`): yes. Also: the lot's own cells and same-zone non-source lots are passed over
  without closing the direction. The value is the map byte at the cell found, and no donor writes 0 over the rect.
  Written into traffic-simulator.md §4.7.

<!-- from: From the traffic simulator core (2026-10-14) -->
- ~~**TS27:** default `trafficUnitsPerSlice`? [16 until calibrated]~~ **ANSWERED 2026-10-14**: no original behaviour;
  the implementer's choice stands. The original slices by a 4 ms wall-clock deadline tested once per unit (§3.3 step
  1, §3.4), so it has no units-per-slice constant. Note added to traffic-simulator.md §3.4.

<!-- from: From the traffic simulator core (2026-10-14) -->
- ~~**TS28 — tuning reads with short or missing data:** fewer vector entries [the rest keep their value]; the
  strategy re-read with fewer than 3 values [missing ones unchanged]; missing ordinance offsets [0]; wealth above 3
  [zeros].~~ **ANSWERED 2026-10-14** from the exe (`0x00716AE0`, `0x00711810`, `0x00718BB0`). Vectors: no, each
  has its own fill (speeds to none, costs/capacities/start costs to 0.0, bools to true, background to 1.0); the
  tile costs are reset even when missing, and income reads past the array (*[choice]* 0.0). The strategy and
  ordinance arrays read past their data when short, and wealth > 3 reads the following members. The brackets stand
  as *[choice]*s there; a missing ordinance property adds nothing. Written into traffic-simulator.md §4.1.2, §4.14.

<!-- from: From the traffic simulator core (2026-10-14) -->
- ~~**TS29:** max funding conversion from percent? [`r24(v / 100)`]~~ **ANSWERED 2026-10-14** from the exe
  (`0x005FDAC0`, `0x00716D67`): the bracket does not match. The percent getter multiplies by the f32 constant 0.01f
  for every numeric type, `r24(v × 0.01f)`; shipped 120 gives 0x3F999999 (1.1999999), not 1.2. Written into
  traffic-simulator.md §4.14.

<!-- from: From the traffic simulator core (2026-10-14) -->
- ~~**TS30:** does Read commit only on success? [yes: parsed first; a failed Read leaves the simulator unchanged]~~
  **ANSWERED 2026-10-14** from the exe (`Read` `0x00721990`, ref helper `0x005D9730`): the bracket is not the
  original but stands as a *[choice]*. Past the version test (< `0x13`: false, nothing changed) the original writes
  into the simulator as it reads, nulls each ref before reading it, sizes the destination map from uninitialised
  locals on an error, and returns `!GetError()`, which the loader ignores. Written into traffic-simulator.md §6.1.

<!-- from: From the traffic simulator core (2026-10-14) -->
- ~~**TS31:** an unknown iterator-state byte in the record? [Done]~~ **ANSWERED 2026-10-14** from the exe (reader
  `0x005DA410`, inline GetNext e.g. `0x0071A1A5`): the bracket does not match. Read stores the byte unchecked, and
  GetNext tests only 2 (done) and 0 (fresh); any other byte acts as running, so the scan steps on from the saved
  cell, and the byte is written back unchanged until the scan ends. Written into traffic-simulator.md §6.1.

<!-- from: From the traffic simulator core (2026-10-14) -->
- ~~**TS32:** record versions above 0x29? [read with the 0x29 layout]~~ **ANSWERED 2026-10-14** from the exe
  (`Read` `0x00721990`): **the bracket matches.** There is no upper bound and every gate is an unsigned ordered
  test (none an equality), so versions `0x2A`…`0xFFFF` read as `0x29`; trailing bytes are skipped by the loader's
  entry framing. Written into traffic-simulator.md §6.4.

<!-- from: From the traffic simulator core (2026-10-14) -->
- ~~**TS33:** Read's own switch flags: is 0x40 set for a subway / el-train rule? [yes, as the re-read does; TN25]~~
  **ANSWERED 2026-10-14** from the exe (`Read` `0x0072225E`–`0x0072227D`): **the bracket matches.** Read adds `0x40`
  for (from, to) = (6, 7) or (7, 6) of any kind, as the re-read does. Its other differences stay as
  traffic-network.md §4.7.2 lists them (kind-1 raw-`to` mask, `to = 5` → `0x20`). Written into §4.7.2.

<!-- from: From the traffic simulator core (2026-10-14) -->
- ~~**TS34 (fixtures only):** every cell reports a network occupant; department funding defaults to 1.0; department
  tokens are 900+.~~ **ANSWERED 2026-10-14**: no original behaviour; the implementer's choice stands. These are
  test-world stand-ins. The one with an original counterpart agrees: a department created by `CreateDepartmentBudget`
  starts at 100 % funding, a fraction of 1.0 (budget.md §6). The traffic simulator's `Init` then
  sets its maximum to the TS29 value (1.1999999 shipped).
- Not covered by vectors: path searches over a real network (the vector world has no network occupants; test 10
  drives ApplyRoute with the spec's route bytes); switches, tunnels and ferries in the save record round-trip only
  as empty lists (Read's switch mask and flags have a unit test). No plugin or capi integration yet (M5 glue).

## From applying the traffic simulator answers (filed by the coordinator from the `apply6a` lane's report, 2026-10-14)

TS1–TS34 are applied to the traffic simulator core (vectors ts11_27–ts11_46 new; ts11_10a and ts11_25 adjusted). The
host gained connection, region, budget and drive-side methods. The current answer is in brackets.

<!-- from: From applying the traffic simulator answers (2026-10-14) -->
- ~~**TS35:** Does entering state 18 zero pass counter 1? [yes]~~
  **ANSWERED 2026-10-14** from the exe (`DoDailySimulation`: state 17 `0x007218B4`–`0x007218F2`, state 18
  `0x007217BF`–`0x007217FD`; the 16/17/18 entry action in `WorkOnSimCycle` `0x007210B0`) and written into
  traffic-simulator §3.3. No: entering 18 zeroes the day index, `processed` and `passTotal`, not pass counter 1
  (+0x94C). Only entering 17 zeroes it, so it keeps state 17's re-route count through state 18. The bracket (yes) is
  wrong.

<!-- from: From applying the traffic simulator answers (2026-10-14) -->
- ~~**TS36:** After a dx = ±1 jump, what entry does the cell carry? [none]~~
  **ANSWERED 2026-10-14** from the exe (route reader `GetNext` `0x006D6510`: the none exit at `0x006D665F`, the skipped
  update at `0x006D666F`; `SetData` `0x006D64FD`) and written into traffic-simulator §4.3. The landing cell carries the
  entry side that the cell before the jump had on arrival. The reader blanks it only in that cell's own record and keeps
  it for the next one. It is none only when the cell before the jump is the first cell. The bracket (none) is wrong.

<!-- from: From applying the traffic simulator answers (2026-10-14) -->
- ~~**TS37:** Does a missing funding percent leave the default? [unchanged]~~
  **ANSWERED 2026-10-14** from the exe (percent getter `0x005FDAC0`: missing property `0x005FDAD6` → `0x005FDCBE`, other
  types `0x005FDAF3` → `0x005FDCBD`; calls `0x00716D67`, `0x00716D79`) and written into traffic-simulator §4.14. Yes:
  the getter returns without a store, so the code default 1.2 stays. The bracket matches.

<!-- from: From applying the traffic simulator answers (2026-10-14) -->
- ~~**TS38:** §8 and §4.1.4 name slot 13 differently from §2.7.2. Which is right? [the code keeps its existing naming]~~
  **ANSWERED 2026-10-14** from the exe (vtable `0x00AB1EE0` slot 13 `0x006CDFA0`, pop 4; slot 14 `0x006CEDE0`, pop 12;
  regional-city slot 20 `0x004A7B40`) and written into traffic-simulator §8. §4.1.4 and traffic-network §2.7.2 are
  right: slot 13 is `FindConnectionOnOtherSide(conn)`. It takes the connection's type and cell and calls slot 14, which
  returns the matching connection from the neighbour's regional record, or null. §8's "property holder" named what the
  caller does with the result; corrected. The naming in code is the implementer's choice.

<!-- from: From applying the traffic simulator answers (2026-10-14) -->
- ~~**TS39:** What is the "jump flag" in TS16? [the 0x80 flag of the next jump record]~~
  **ANSWERED 2026-10-14** from the exe (route reader `0x006D65C6`–`0x006D65CC`, run tokens `0x006D6593`; ApplyRoute test
  `0x00719E97`) and written into traffic-simulator §4.5. It is bit 7 (`0x80`) of the jump token that leads to the next
  record. The reader stores it in the record the jump arrives at, and run tokens clear it. The bracket matches.

<!-- from: From applying the traffic simulator answers (2026-10-14) -->
- ~~**TS40:** What does a statistics slot with no record publish? [zeros]~~
  **ANSWERED 2026-10-14** from the exe (state-2 entry `0x00721196`–`0x007211D6`; record update
  `0x0071A3D7`–`0x0071A40C`; publish `0x0071A5D7`–`0x0071A67A`) and written into traffic-simulator §4.9. Zeros: average
  0.0, congested 0, max cell (0, 0), max 0.0. The −1.0 start is a separate comparison value, not the published max. The
  bracket matches.

<!-- from: From applying the traffic simulator answers (2026-10-14) -->
- ~~**TS41:** A pool ID repeated in row 19: which entry wins? [the last one]~~
  **ANSWERED 2026-10-14** from the exe (traffic `Read` `0x00722F60`–`0x00722FB3`; map find-or-insert `0x00714560`) and
  written into traffic-network §2.7. The last one wins: each entry's holder is stored through the map's find-or-insert,
  overwriting the earlier pointer, which is never released. The bracket matches.

<!-- from: From applying the traffic simulator answers (2026-10-14) -->
- ~~**TS42:** Pool ID overflow. [pool 0]~~
  **ANSWERED 2026-10-14** from the exe (`AddNeighborConnection` ID search `0x0071555A`–`0x00715595`, store `0x007155A6`)
  and written into traffic-network §2.7.2. The count wraps to ID 0: the record gets pool 0, the default pool, and the
  new holder is stored in the map under key 0. If key 0 is taken too, the search never ends. Unreachable in the game.
  The bracket (pool 0) matches.

<!-- from: From applying the traffic simulator answers (2026-10-14) -->
- ~~**TS43:** A missing regional demand node. [0]~~
  **ANSWERED 2026-10-14** from the exe (`0x00714E60`: `GetDemand` at `0x00714ED9`, its result used at once at
  `0x00714EDC`) and written into traffic-network §2.7.4. The original crashes: `GetDemand` returns null for a missing
  node and the value query goes through it. The shipped data define the nodes. The bracket (0) is a *[choice]*,
  recommended.

<!-- from: From applying the traffic simulator answers (2026-10-14) -->
- ~~**TS44:** Which row-14 field holds the saved source cell? [`phase_positions` s16×2, the lot location]~~
  **ANSWERED 2026-10-14** from the exe (writes `0x0071E276`, `0x0071D0FD`, `0x0071D51C`; source iterator `0x007107F0`;
  resume `0x00720B9A`–`0x00720C2E`) and written into traffic-simulator §3.2, §3.5 and §6.1. It is the two s16 phase
  positions. The trip phases store the cell the source iterator returns: a lot's location (lot slot 21) or a neighbour
  connection's cell. The 16–18 entry action zeroes them (the table's "pass counters reset" was these two). The bracket
  matches.

<!-- from: From applying the traffic simulator answers (2026-10-14) -->
- ~~**TS45:** Which cell is saved for the state-15 resume? [the same `phase_positions`]~~
  **ANSWERED 2026-10-14** from the exe (FinishTripMap `0x0070FFD3`–`0x0070FFE7`; resume `0x00720B2C`–`0x00720B46`) and
  written into traffic-simulator §3.5. The same two s16 phase positions: FinishTripMap stores each lot's location as it
  takes it, and the resume passes them to `GetLot`. The bracket matches.

<!-- from: From applying the traffic simulator answers (2026-10-14) -->
- ~~**TS46:** How is §4.16's `float(item.cost)` computed? [converted to f32 first, then × funding, r24]~~
  **ANSWERED 2026-10-14** from the exe (`GetTransitSwitchQueryData` `0x0071CC55`–`0x0071CC5B` and
  `0x0071CD6F`–`0x0071CD75`) and written into traffic-simulator §4.16, with a worked example. The int64 is loaded
  exactly (`fild`), multiplied by the f32 fraction with one rounding to 24 bits, then truncated. The bracket (f32 first,
  then r24) rounds twice and differs only for costs above 2^24 (16,777,217 at 0.75: 12,582,913 against 12,582,912).

<!-- from: From applying the traffic simulator answers (2026-10-14) -->
- ~~**TS47:** A strategy above 2 in SetupPathFinderForLot. [ignored; a choice in code]~~
  **ANSWERED 2026-10-14** from the exe (`SetupPathFinderForLot` `0x00711680`–`0x00711687`, sums allocated at
  `0x0071165E`) and written into traffic-simulator §4.4. The original adds the count to `sum[strategy]` unchecked, so a
  strategy above 2 writes past the 12-byte array into the heap. Only a damaged save has one. The bracket (ignore it) is
  a *[choice]*, recommended.

<!-- from: From applying the traffic simulator answers (2026-10-14) -->
- ~~**TS48:** A damaged travel type above 8. [adds nothing in ApplyRoute and fails ConfirmRoute; a choice]~~
  **ANSWERED 2026-10-14** from the exe (ApplyRoute `0x00719C2F`, `0x00719C4B`, `0x00719EE9`; type mask `0x3F` in the
  reader `0x006D6572`) and written into traffic-simulator §4.5. The original checks nothing: a type up to 63 indexes the
  9-entry network, generates-traffic and income tables and reads what follows them. ConfirmRoute reads unrelated record
  words (§4.3). The bracket (add nothing, fail ConfirmRoute) is a *[choice]*, recommended.

<!-- from: From applying the traffic simulator answers (2026-10-14) -->
- ~~**TS49:** A switch ID with no switch behind it in GetTransitSwitchQueryData. [returns false; a choice]~~
  **ANSWERED 2026-10-14** from the exe (`GetTransitSwitchQueryData`: find-or-insert `0x0071CBB7`, building read
  `0x0071CC12`–`0x0071CC14`) and written into traffic-simulator §4.16. The original inserts a default switch, writes
  four of the five fields, then crashes reading the null building. Only a damaged save has such an ID. The bracket
  (return false) is a *[choice]*, recommended.

## From the traffic glue, phase 1 (filed by the coordinator from the `trafficglue` lane's report, 2026-10-14)

Our traffic simulator (with the pathfinder) replaces the original in game (`enable = traffic`): clean under turbo,
at normal speed for 84 months and across two saves, with 0 reference warnings. There is no traffic shadow yet.
The lane's implemented answers are in brackets; most are glue choices that need only a confirmation or a spec line.

<!-- from: From the traffic glue, phase 1 (2026-10-14) -->
- ~~**TG1:** `FindPath` with a route deque. [returns 0, logged once, counted; the deque is untouched]~~ **ANSWERED 2026-10-15 (re-answers10):** the original fills the deque (pathfinder §4.13 step 5, also after the flag-2 fallback of §4.12); the automata, emergency and MySim dispatch and the query tool read it. The bracket **DIFFERS**: fill it. The deque's 0x28-byte layout and the clear / push_front / map-regrowth rules are in pathfinder §7 (traffic-simulator §9.1).

<!-- from: From the traffic glue, phase 1 (2026-10-14) -->
- ~~**TG2:** host callbacks left NULL: `path_info` and its helpers, `sim_clock_ms`, `update_network_costs`,
  `lot_strategy_byte`, `take_connection_volume_dirty`, `occupants_in_cells`, `occupant_filter_includes`,
  `cell_has_network_occupant`, `tunnel_paired_portal`, `view_cell_rect`, `create_accident`. Which of these does
  the original need for the simulation (not the view)? [all NULL in phase 1]~~ **ANSWERED 2026-10-15 (re-answers10):** by what the original reads (traffic-simulator §9.1 tables): **needed** for the simulation are the occupant query over a rectangle (`occupants_in_cells`), path connections and path info (`path_info` and helpers), the tunnel's paired portal (`tunnel_paired_portal`), the neighbour connection's volume-dirty test-and-clear (`take_connection_volume_dirty`; one sync per connection object), the roads and mass-transit department cost updates (`update_network_costs`, if that is what it does) and the lot's strategy byte at lot + 0x88 (`lot_strategy_byte`, only if our core computes the lots' trip targets). **Not needed**: the wall-clock deadline (`sim_clock_ms`), the accident check (`view_cell_rect`, `cell_has_network_occupant`, `create_accident`: monthly, wall-clock gated, visible cells only; it costs only the visible crashes) and `occupant_filter_includes` (filter 2 unreachable). So all-NULL **DIFFERS** for the first group.

<!-- from: From the traffic glue, phase 1 (2026-10-14) -->
- ~~**TG3:** the trip record layout. [FreeCity4's own `FC4TripData`]~~ **ANSWERED 2026-10-15 (re-answers10):** the lots' and connections' own `std::list<cSC4TripData>` (0x2C-byte records, allocator B) are read by their `Write`/`Read`, by the query tool's route display (`0x004CA460`, `0x004C5E20`, `0x004CFD20`) and by `SetupPathFinderForLot`. `FC4TripData` is acceptable only if those lists hold the trips in that layout whenever those run (copied in at load, written back before the lots save, kept current for the query tool); otherwise **DIFFERS** (a save loses every trip). traffic-simulator §9.1.

<!-- from: From the traffic glue, phase 1 (2026-10-14) -->
- ~~**TG4:** `Init` after state was loaded by `Read`. [the loaded state is kept]~~ **ANSWERED 2026-10-15 (re-answers10):** MATCHES. In a load `Read` always runs before `Init` (foundations §3.7), and `Init` creates each grid, the iterators, the generator, the departments and the pathfinders only when the member is still null; the cycle state is reset only with a new scanning iterator (`0x0071F4FE`). Init rebuilds only unsaved structures (tuning, subscriptions, edge vectors, cell array). traffic-simulator §1.

<!-- from: From the traffic glue, phase 1 (2026-10-14) -->
- ~~**TG5:** slot 34's record size. [0x23C]~~ **ANSWERED 2026-10-15 (re-answers10):** MATCHES: 0x23C bytes at simulator +0x1E4 (last field u8 at +0x238; next member +0x420; working set at +0x440). traffic-simulator §7.

<!-- from: From the traffic glue, phase 1 (2026-10-14) -->
- ~~**TG6:** slot 22. [returns null, logged]~~ **ANSWERED 2026-10-15 (re-answers10):** MATCHES: slot 22 returns the lot iterator only in cycle state 1, which nothing enters (a loaded 1 becomes 0 at PostCityInit). traffic-simulator §9.1.

<!-- from: From the traffic glue, phase 1 (2026-10-14) -->
- ~~**TG7:** departments. [`Get`, else `Create(id, 0)`; `SetMaxAllowedFunding`; line items 1–6]~~ **ANSWERED 2026-10-15 (re-answers10):** partly **DIFFERS**: `CreateDepartmentBudget` returns an existing department, so get-else-create is right, but the original uses group `0xAA369059`, sets the department and line-item names, makes mass-transit line 2 and roads line 5 Income, and runs these only when the member is null after Read; lines 4–6 are created on every Init when missing. Exact calls in traffic-simulator §9.1.

<!-- from: From the traffic glue, phase 1 (2026-10-14) -->
- ~~**TG8:** `GetLanguageUtility`. [borrowed, not AddRef'd]~~ **ANSWERED 2026-10-15 (re-answers10):** MATCHES: the language manager returns a pointer to its own member (`0x00924B50`) and nobody AddRefs or releases it. traffic-simulator §9.1.

<!-- from: From the traffic glue, phase 1 (2026-10-14) -->
- ~~**TG9:** `DoLotActivated` budget items. [copied, then `AddBudgetItemToBudget(lot holder, item.index)`]~~ **ANSWERED 2026-10-15 (re-answers10):** MATCHES: `GetBudgetItemForPurpose(building holder, …)`, then `CopyBudgetItemProperties(building holder, lot holder)` and `AddBudgetItemToBudget(lot holder, item.index)` (index at record +0x18). traffic-simulator §9.1, §4.1.3.

<!-- from: From the traffic glue, phase 1 (2026-10-14) -->
- ~~**TG10:** lot order. [the master list through the safe iterator; a rect query as fallback]~~ **ANSWERED 2026-10-15 (re-answers10):** MATCHES for the safe iterator, which is the original's only order; the rectangle fallback has no counterpart and must not run in normal play. traffic-simulator §4.1, §9.1.

<!-- from: From the traffic glue, phase 1 (2026-10-14) -->
- ~~**TG11:** `Shutdown`. [unsubscribe; `RemoveAgentAllLevels`]~~ **ANSWERED 2026-10-15 (re-answers10):** MATCHES: unsubscribe the nine messages, then `RemoveAgent(this + 4)` on every level through the city's simulator (skipped when null). traffic-simulator §1.

<!-- from: From the traffic glue, phase 1 (2026-10-14) -->
- ~~**TG12:** `department_funding` with a null department. [1.0]~~ **ANSWERED 2026-10-15 (re-answers10):** no original behaviour (the original never checks; only a failed Read leaves a null department). 1.0 accepted. traffic-simulator §9.1.

<!-- from: From the traffic glue, phase 1 (2026-10-14) -->
- ~~**TG13:** slot 12. [a ring of 64 records]~~ **ANSWERED 2026-10-15 (re-answers10):** accepted as a choice. The original returns a live pointer into the hash node (or the shared zero record), valid until the record is erased; the ring gives call-time copies and must keep each pointer valid until the caller's next call. traffic-simulator §9.1.

<!-- from: From the traffic glue, phase 1 (2026-10-14) -->
- ~~**TG14:** slot 29. [a vector kept per tile pair]~~ **ANSWERED 2026-10-15 (re-answers10):** accepted as a choice if each call returns the current route as an STLport `vector<u8>` (allocator B) and null in exactly the original's cases; the original points into the terminal link's stored vector. traffic-simulator §9.1.

<!-- from: From the traffic glue, phase 1 (2026-10-14) -->
- ~~**TG15:** slots 20/21 called off the main thread. [a direct property read]~~ **ANSWERED 2026-10-15 (re-answers10):** MATCHES: slots 19–21 use no simulator state, only the holder's property `0x8A0B4100`, with no lock in the original. traffic-simulator §9.1.

<!-- from: From the traffic glue, phase 1 (2026-10-14) -->
- ~~**TG16:** a lot's `AsTrafficSource`. [borrowed]~~ **ANSWERED 2026-10-15 (re-answers10):** MATCHES: lot slot 4 is a cast without AddRef; the source iterator AddRefs the lot or connection object itself. traffic-simulator §4.1.

<!-- from: From the traffic glue, phase 1 (2026-10-14) -->
- ~~**TG17:** `CreatePathFinder(null)`. [false]~~ **ANSWERED 2026-10-15 (re-answers10):** no original behaviour (the original writes through the null pointer). false accepted. traffic-simulator §9.1.

<!-- from: From the traffic glue, phase 1 (2026-10-14) -->
- ~~**TG18:** `SetupPathFinderForLot` with a pathfinder the original created. [false]~~ **ANSWERED 2026-10-15 (re-answers10):** **DIFFERS** in principle (the original configures any `cISC4PathFinder` through its interface), but unreachable when ours is the city's simulator. Accepted. traffic-simulator §9.1.

<!-- from: From the traffic glue, phase 1 (2026-10-14) -->
- ~~**TG19:** stale lot or connection tokens. [refused; a connection removed mid-phase is skipped]~~ **ANSWERED 2026-10-15 (re-answers10):** lots MATCH (skipped by the safe iterator). Connections **DIFFER**: the original's connection list is a snapshot of counted references, so a connection removed mid-phase is still visited until the next Reset (entering 16, 17 or 18). traffic-simulator §9.1, §4.1.

<!-- from: From the traffic glue, phase 1 (2026-10-14) -->
- ~~**TG20:** which messages carry an object. [only the lot message `0xE8BDA2E7` and the occupant messages
  `0x99EF1142`/`0x99EF1143`]~~ **ANSWERED 2026-10-15 (re-answers10):** MATCHES: only the lot message (IGZUnknown, QI `0xE818FCED`) and the occupant messages (void1, QI `0xA821EF94`) carry an object the handler reads; `0x08DAC3E7` reads data2 and the simulator's own department. traffic-simulator §9.1.

<!-- from: From the traffic glue, phase 1 (2026-10-14) -->
- ~~**TG21:** a message on another thread while the lock is busy; slot 19 while the lock is busy. [dropped; slot 19
  counted as a fallback]~~ **ANSWERED 2026-10-15 (re-answers10):** messages: the original never drops one (no lock, handled on the delivering thread), so a drop **DIFFERS** whenever it happens; keep it counted and rare. Slot 19 needs no lock at all (TG15): the direct write is the normal path. traffic-simulator §9.1.

<!-- from: From the traffic glue, phase 1 (2026-10-14) -->
- ~~**TG22:** units per slice against the original's 4 ms deadline (traffic-simulator §3.4). [16 deterministic units
  per slice; our turbo runs at about 43–46 game days/s with traffic against about 76 without]~~ **ANSWERED 2026-10-15 (re-answers10):** no original constant (TS27); 16 accepted. The original's per-day traffic work is the share `total/days` plus the last slice's overshoot, so N changes only the overshoot. traffic-simulator §9.1, §3.4.

## From the traffic glue, phase 2: the traffic shadow (filed by the coordinator from the `traffic2` lane's report, 2026-10-15)

About `docs/specs/traffic-simulator.md` and the same-process traffic shadow (`traffic_shadow = 1`; core
`traffic_simulator/shadow.rs`, capi `traffic_shadow.rs`, plugin `FC4TrafficShadow`, seam op 51); merged as `40d9eab`,
ABI 56. With `traffic_shadow_hypotheses = 7` (bits 1, 2, 4 below) the shadow matches the original on every delivery on
Konradshohe for 7 game months (261/261, resyncs=0); with 0 (the spec as written) 25 of 92 deliveries are clean. The
lane numbered these TS1-TS17 (TS50-TS66 here; engine comments and `engine/README.md` use the lane's numbers). The
current answer is in brackets.

<!-- from: From the traffic glue, phase 2: the traffic shadow (2026-10-15) -->
- ~~**TS50:** §6 record: the record does not save the last-day field, so a shadow (or a resync) cannot know the original's
  day pacing state. Is lastDay intentionally unsaved, and what does the original do with it on load? [the copy starts
  at the first delivery]~~ **ANSWERED 2026-10-15 (re-answers13):** MATCHES. The last day-of-month seen is member `+0x1AC`, set to −1 by the constructor (`0x0071FA3B`) and written only by DoDailySimulation (`0x00721603`); Write, Read, Init and DoPostCityInit never touch it. The simulator is built with each city, so the first call after a load always takes the new-day branch, even on the save's own day: on day 1 the two incomes are set again, and the strategy re-draw test of that date runs a second time (more draws). traffic-simulator §3.3 step 2.

<!-- from: From the traffic glue, phase 2: the traffic shadow (2026-10-15) -->
- ~~**TS51:** §4.1.2 lot strategy byte: there is no getter for a lot's travel strategy byte. Is the last byte of the lot's
  own `Write` record (§6.3) the byte §4.1.2 uses, and is there an interface slot for it? [the shadow reads that byte]~~ **ANSWERED 2026-10-15 (re-answers13):** MATCHES, no slot. The byte is lot member `+0x88`: UpdateTrafficVolume reads it at `0x006C2986`, and `cSC4Lot::Write` writes it as the record's last byte, after the trip list (`0x006C0347`). No `cSC4Lot` vtable has a getter for it; the copy constructor copies it (`0x006C25AA`). traffic-simulator §4.1.1.

<!-- from: From the traffic glue, phase 2: the traffic shadow (2026-10-15) -->
- ~~**TS52:** §4.6 DeactivateBudgetItems: the shadow needs the cost of the holder's `0xCA550301` budget item without
  removing it; passing the holder token to OccHolder's BudgetItemForPurpose broke the stack under /RTC1. Is reading
  `item.cost` through the city budget simulator (GetBudgetItemForPurpose) the right lookup? [that lookup; a choice]~~ **ANSWERED 2026-10-15 (re-answers13):** MATCHES, and it is the original's own lookup, not a choice: DoLotDeactivated calls the budget simulator's slot 51 (`0x00691880`) with the lot's own property holder (lot slot 3) as a `cISCPropertyHolder*` (`0x0071E4BD`–`0x0071E4CA`), and slot 51 only fills the record. It copies a whole `0x20`-byte `BudgetItem` (8 dwords, `0x006918EE`; cost s64 at `+0x10`, index at `+0x18`, funding f32 at `+0x1C`), so a buffer smaller than `0x20` bytes is overrun, a likely cause of the /RTC1 failure. traffic-simulator §4.1.3.

<!-- from: From the traffic glue, phase 2: the traffic shadow (2026-10-15) -->
- ~~**TS53:** §6 record variants: the original's WriteVariant output is captured through the core's writer
  (`fc4_props_write_variant`, foundations §9.2: u32 0, type tag, value). Is that the byte form the original's segment
  stream writes for every type that occurs in the traffic record? [assumed]~~ **ANSWERED 2026-10-15 (re-answers13):** MATCHES. The only variants in the record are in the pool holders of row 19: plain `cSCPropertyHolder`s whose own `Write` (holder `+0x5C`, called at `0x00713324`) writes the foundations §10.2 record, each property through `WriteVariant`: u32 marker 0, u32 disk tag, u32 count, elements. The traffic code puts only `0x8A0B4100` on a pool: a u16 array (tag `0x8002`) or, once promoted, a u32 array (`0x8003`). traffic-simulator §6.2.

<!-- from: From the traffic glue, phase 2: the traffic shadow (2026-10-15) -->
- ~~**TS54:** §6.2 Read: the SGI hash containers (damage sets, edge densities) iterate bucket ascending with head
  insertion, and our Read rebuilds the chains in reverse order. Which order does the original's Read leave, and does
  any later pass depend on it? [the shadow restores the original's order: smallest prime >= max(len, 100), non-decreasing
  keys]~~ **ANSWERED 2026-10-15 (re-answers13):** Partly **DIFFERS**. Read inserts into fresh containers of 193 buckets (hint 100, `0x00829BC0`). For each key in file order, the table first grows to the smallest listed prime ≥ count + 1 when that exceeds the bucket count (`0x00710DD0`). A grow walks the old buckets in ascending order, moving each chain's nodes from the head, each to the head of its new chain. Then the key goes to the **head** of its chain (`0x0060D270` damage sets, `0x006D6FD0` edge densities). The bracket's bucket count is the final one. Building the final table directly and ordering each chain by key gives the original's order only when no grow happens during Read (193 keys or fewer) and the file listed each chain in decreasing key order. Otherwise replay the inserts. Two things depend on the order: ApplyPotholeChanges' pick of the `r`-th cell, and the next Write's key order. Nothing else does. traffic-simulator §6.2.

<!-- from: From the traffic glue, phase 2: the traffic shadow (2026-10-15) -->
- ~~**TS55:** §4.3 ConfirmTrips merge: in game a routed trip is set back to status 0 only when it absorbed a later trip.
  As written (every routed status-0 candidate) the copy gets toReRoute 41 where the original has 0 (Konradshohe
  2026-01). Confirm. [hypothesis bit 1; clean in game, 261/261]~~ **ANSWERED 2026-10-15 (re-answers13):** MATCHES hypothesis bit 1; it becomes the rule. In a re-route source each trip that is status 3 when reached absorbs every later trip of the same type and strategy, whatever that trip's status. Its status becomes 0 **only when it absorbed at least one trip**: the store is inside the match (`0x0071D9B6`). Each such trip adds 1 to pass counter 5 (`0x0071DA0D`). A routed trip with no partner keeps status 3 and goes on to the capacity test. traffic-simulator §4.3.

<!-- from: From the traffic glue, phase 2: the traffic shadow (2026-10-15) -->
- ~~**TS56:** §6.1 row 19 / §4.4 end of FullTrips: the travel-type count pairs appear to be (published, building), and the
  end of FullTrips swaps the two vectors instead of copying (the original's row 19 is (130,132) right after the end;
  the spec reading gives the reverse). Confirm. [hypothesis bit 2; clean in game]~~ **ANSWERED 2026-10-15 (re-answers13):** MATCHES hypothesis bit 2; it becomes the rule. Write and Read store the pairs as (published `+0x424`, building `+0x430`) (`0x00713370`–`0x00713398`, `0x00722FD0`). The end of FullTrips **swaps** the two vectors' begin, end and capacity words (`0x0071D806`–`0x0071D84E`), so building holds the previous published counts until entering 18 refills it with 9 zeros (`0x007210E6`). traffic-simulator §4.4, §6.1, §2; save-records §7.1.1.

<!-- from: From the traffic glue, phase 2: the traffic shadow (2026-10-15) -->
- ~~**TS57:** §4.4 ReRouteTrips merge: the merge also absorbs later trips of the same type and strategy that are routed
  (status 3) or status 4, not only status 0 (lot (38,96) goes from [2x st0, 1x st3] to [3x st0] in the original; the
  spec reading leaves the st3 trip). Confirm. [hypothesis bit 4; clean in game]~~ **ANSWERED 2026-10-15 (re-answers13):** MATCHES hypothesis bit 4, and wider. The ReRouteTrips merge absorbs into each status-0 trip every later trip of the same type and strategy, whatever its status (0, 1, 3, 4 or 5). The test meant to skip routed trips reads the **absorbing** trip's status (`0x0071D2D7`), which is always 0 there, so it never skips. The absorbing trip stays status 0 and no counter moves. traffic-simulator §4.4.

<!-- from: From the traffic glue, phase 2: the traffic shadow (2026-10-15) -->
- ~~**TS58:** §4.4/§4.5 reversible trips: the original keeps a reversible trip's returnRoute where the spec clears or
  rewrites it (about 100 lists a month). Which rule applies? [counted as a known difference]~~ **ANSWERED 2026-10-15 (re-answers13):** The original never clears or rewrites `returnRoute` on a freight or reversible result. It fetches only the outbound route (pathfinder `+0x80` into `route` and `travelTypesUsed`), so `returnRoute` keeps what it held: an earlier two-leg result of the same record, the vector copied into a split copy (`0x0071D6EA`), or the save. Nothing reads it while the reversible bit is set. The spec's "empty" (§2) was wrong and is corrected; keep the vector untouched, and the difference goes away. traffic-simulator §2, §4.4.

<!-- from: From the traffic glue, phase 2: the traffic shadow (2026-10-15) -->
- ~~**TS59:** §10 Q4 pass counters: counters 0, 5 and 7 advance in ConfirmTrips in ways §10 does not give; counter 4 also
  advances in ConfirmTrips (4 right after entering 17 with processed 0). What are the increment points of counters 0
  and 4-8? [counted as a known difference]~~ **ANSWERED 2026-10-15 (re-answers13):** The ten counters (`+0x948` + 4·i) are named, with every increment point, in the traffic-simulator §10 Q4 table. In short: 0 confirmed `0x0071DBF5`; 1 re-routes found `0x0071D289`; 2 `rerouteBucket`; 3 sources `0x0071D93F`; 4 destination full, in ConfirmTrips `0x0071DC01` and in ReRouteTrips `0x0071D221`; 5 merges `0x0071DA0D`; 6 return leg not confirmed `0x0071DBA0`; 7 status-0 trips met before the merge `0x0071D95D`; 8 status 5 → 0 `0x0071DC11`; 9 splits `0x0071D77F`. All are zeroed on entering 16 except 1 and 2. All ten are saved. Only counter 2 drives behaviour. The bracket (a known difference) can become exact.

<!-- from: From the traffic glue, phase 2: the traffic shadow (2026-10-15) -->
- ~~**TS60:** §6.1 rows 14-15 unnamed fields: the original changes an i32 that toggles 0/1 with the pacing, and the
  re-route context's x,z. What are they? [the copy keeps them as read]~~ **ANSWERED 2026-10-15 (re-answers13):** The i32 is **`dayIndex`** (`+0x918`), the s32 right after `processed` in row 15 (`0x00721912`–`0x0072193E`). The field the spec and save-records called "dayIndex" (the u32 after `days`, `+0x930`) is never used, nor are the next two (`+0x934`, `+0x938`). The re-route context's x, z (`+0x83C`, `+0x840`) are the cell of the source being worked, written as each source is taken in 16–18 (`0x0071E26D`, `0x0071D104`, `0x0071D523`) and not reset on phase entry. Keeping them as read **DIFFERS**: model `dayIndex` at that position and write the context cell as sources are taken. traffic-simulator §3.3, §6.1; save-records §7.1.1.

<!-- from: From the traffic glue, phase 2: the traffic shadow (2026-10-15) -->
- ~~**TS61:** statistics sets: the last, unnamed byte of a statistics set increments once per cycle (1C to 1D) in the
  original. What is it, and where is it incremented? [not modelled]~~ **ANSWERED 2026-10-15 (re-answers13):** **DIFFERS** (it should be modelled). It is `freightByte`: freight trip length as a fraction of `commuteMaxTime` × 255, truncated, low 8 bits. FinalCalculations recomputes it once per cycle (`0x0070E84A`, `0x0070E862` when there is no freight) as the last byte of the **published** set (`+0x41C`, block offset `0x238`); the 1C to 1D step was a recomputation. The working set's byte (`+0x678`) is written only by the constructor and Read and is never copied. traffic-simulator §4.12.

<!-- from: From the traffic glue, phase 2: the traffic shadow (2026-10-15) -->
- ~~**TS62:** §3.4 deadline test points: in game the original also stops at the end of a source's list, in 17 and 18
  after the merge or CompleteTripSourceCalculations and before the next source is taken; in ReRouteTrips it never
  stops before a trip that is not status 1 (shadow-14: 4 FullTrips days with the original's saved cell before ours,
  next-day attempted/routed +4, +5, +97, +34; 2026-02-24 lot (40,82) merged by the original). Which test points does
  the original have (`0x0071D1D9`, `0x0071D607`), and is one at the list end? [the shadow follows the saved cell and
  takes a list end only when the record or trip list shows it, 0-2 a month]~~ **ANSWERED 2026-10-15 (re-answers13):** The test points are now in a table in traffic-simulator §3.4. ConfirmTrips tests before each source (`0x0071E222`). ReRouteTrips and FullTrips test at the top of the source loop (`0x0071D094`, `0x0071D4B3`): at each call's start and after each finished source, that is after its merge or CompleteTripSourceCalculations and before the next source is taken. **Yes, that is a list-end stop.** They also test before every trip whatever its status (`0x0071D1D9`, `0x0071D607`). A point reads the clock only when the per-call counter, stepped by the phase's adaptive `step`, is a multiple of 1024. "Never before a non-status-1 trip" is timing, not code. A stop inside a list redoes the list from its head next call; the merge or CompleteTripSourceCalculations has not run. The bracket's rule (follow the saved cell) agrees with this.

<!-- from: From the traffic glue, phase 2: the traffic shadow (2026-10-15) -->
- ~~**TS63:** §4.7 FinishTripMap: in game state 15 can stop part-way, after a lot, on the deadline (2026-01-30, 18 to 15
  in one day; saved cell lot (17,51); the trip-length grid differs in 12 tracts until the copy stops there too). Does
  FinishTripMap test the deadline per lot, and at which address? [the shadow can stop after the lot at the saved cell]~~ **ANSWERED 2026-10-15 (re-answers13):** MATCHES. FinishTripMap tests the deadline before each lot is taken (`0x0070FF84`). The lot's location becomes the phase position as it is taken (`0x0070FFE0`), and the lot is then handled completely. So a stop falls after a whole lot, at the saved cell. traffic-simulator §4.7, §3.4.

<!-- from: From the traffic glue, phase 2: the traffic shadow (2026-10-15) -->
- ~~**TS64:** §3.2 states 12/13, scanning iterator: after 11 to 13 the original's scanning iterator reads reset (all
  zero) or Running at (34,87), and Done at (0,128) after 13 finishes; the spec gives 12 and 13 no use of the iterator
  and ours keeps phase 10's Done (shadow-19 2026-02-24, shadow-20 2026-07-30). Does UpdateSubnetworks or
  UpdateSubnetworkInfo reset and run the scanning iterator, and is it deadline-stoppable? [ours keeps Done; 1-2
  deliveries differ in that field]~~ **ANSWERED 2026-10-15 (re-answers13):** **DIFFERS**. Entering 13 resets the simulator's scanning iterator (`+0x818`) to state 0 at (0, 0), keeping its rectangle and step (`0x00721308`–`0x0072132A`). UpdateSubnetworkInfo steps it per cell and tests the deadline before each step (`0x00716212`), so a call can stop mid-scan (state 1 at the last cell). When the scan ends the iterator reads state 2 at `(x0, z1 + 1)`, which is (0, 128) in a 128-cell city; the neighbour pass then runs in the same call. State 12 and its entry do not touch the iterator. DoPostCityInit uses a local iterator. traffic-network §4.12, traffic-simulator §3.2.

<!-- from: From the traffic glue, phase 2: the traffic shadow (2026-10-15) -->
- ~~**TS65:** §4.4 suspended searches: SetupTrip calls SetMaxWorkTime(remaining slice ms), so the original can suspend a
  search, and a suspended search seems to write the trip's destination (a status-0 split copy at the day's stop: lot
  (56,99) original (28,37) against ours (18,40) on 2026-03-02; lot (31,86) on 2026-05-10; same count, length and
  route). "The copy is made before the search" was refuted in game. Does a suspended PerformPathSearch write
  destX/destZ, and what else does it leave in the trip? [ours never suspend (§3.4 choice); counted as a known
  difference for status-0 trips]~~ **ANSWERED 2026-10-15 (re-answers13):** Yes. When the outbound leg is found, `destX, destZ := pf.GetEndCell()` at once (`0x0071C752`). On the two-leg path flags bit 0 is then cleared (`0x0071C858`). If the return leg suspends, the trip keeps only those two changes; `route`, `returnRoute`, `tripLength`, `travelTypesUsed`, `count` and the status are unchanged and `processed` is not counted. An outbound suspension writes nothing. A resumed search skips SetupTrip; a network change restarts it, and the written destination stays. The bracket is accepted as the §3.4 choice: the difference is confined to those fields of status-0 trips at a day's stop. traffic-simulator §4.4.

<!-- from: From the traffic glue, phase 2: the traffic shadow (2026-10-15) -->
- ~~**TS66:** §4.15 draws: the pothole and accident draws and the strategy draws (property `0x4A611773`) cannot be compared
  because the original's generator state is not readable. Is there a readable generator, or a deterministic seed path?
  [not compared]~~ **ANSWERED 2026-10-15 (re-answers13):** There is no interface, save field or deterministic seed. The generator is the heap object at simulator member `+0x6FC` (`0x9C8` bytes: `u32 state[624]`, the next-word pointer at `+0x9C0`, the words-left count at `+0x9C4`). Init creates it and seeds it once from the local date and time (`0x0071F132`–`0x0071F1A9`). A same-process shadow can read that state after each delivery and compare draws. It could also call `Seed` (`0x00913A72`, thiscall, one u32) on it after Init, but that is a write into the original, so a test-only choice. traffic-simulator §4.15.

## From applying the traffic glue answers (filed by the coordinator from the `apply11traffic` lane's report, 2026-10-16; merged `85919d3`)

apply11traffic applied the re-answers10 TG DIFFERS entries to the traffic glue (TG1, TG7, TG10, TG15/TG21, TG19, TG2
partly; ABI 61). `enable = traffic` with the 26 other regress replacements is clean to 2032 (run 128, ref_warnings=0).
Three TG2 callbacks stay NULL because the specs leave them open. The current choice is in brackets.

<!-- from: From applying the traffic glue answers (2026-10-16) -->
- ~~**TG23:** traffic-simulator §4.9 step 3, §9.1 TG2, spec Q1: what exactly do UpdateRoadsBudget (`0x0070D8A0`) and
  UpdateMassTransitBudget (`0x0070DA40`) call? Which line item numbers get which network-type cell counts (roads:
  Road+OneWayRoad, Street, Highway+GroundHighway, Avenue; mass transit not given), with which method (SetFullExpenses?
  AddToFullExpenses?), what count x tile-cost formula and rounding, and are they also called when the count did not
  change? [`update_network_costs` stays NULL: the transport line items keep the expenses they were loaded with]~~
  **ANSWERED 2026-10-16 (re-answers21):** traffic-simulator §4.9.1. Both use `GetLineItem(i)` (department slot 20)
  then `SetFullExpenses(int64)` (line item slot 12; never AddToFullExpenses). Roads, in order: item 2 = Road +
  OneWayRoad, item 1 = Street, item 3 = Highway + GroundHighway, item 4 = Avenue. Mass transit: item 3 = Rail
  (created and named `0x6A8B0CFB` if missing), 4 = Subway, 5 = LightRail, 6 = Monorail. Per type: `r24(count ×
  tileCost[t])` rounded to nearest, ties to even (the `+12582912.0f` bit trick, exact below 2²²), as u32; two-type
  items add the two u32 as an unsigned 64-bit sum. No change test: called on every road/rail/subway insert and
  remove, every CreateMaps pass (roads then transit) and at PostCityInit (transit then roads). The bracket
  **DIFFERS**: the original's maintenance lines follow the network the same day and are re-priced at load.

<!-- from: From applying the traffic glue answers (2026-10-16) -->
- ~~**TG24:** traffic-network §4.4, §7.2, §9.1 TG2: what are the arguments, return values and result containers of path
  info slots 14 (GetPathsFromEntryEdge, 3 args), 3 (GetPath, 2 args) and 7 (GetPathsNearEndPoint, 6 args)? Which field of
  a path object is the exit byte, and which is its first vertex? Also: does GetPathInfo (occupant slot 49) hand over a
  reference, and does the slot-4 turn-around test use all of EAX or only AL? [`path_info_exits_from_entry` and
  `path_info_entries_near` stay NULL, so lane continuity changes nothing; GetPathInfo is treated as borrowed; the whole
  EAX is tested]~~ **ANSWERED 2026-10-16 (re-answers21):** traffic-network §7.3 and §4.4. Slot 14 `u32
  GetPathsFromEntryEdge(pathType, entry, vector<u32>& keys)` empties `keys`, appends the matching path keys in hash
  order, returns the count. Slot 3 `Path* GetPath(key, float* lengthOut)` returns the path record or null. Slot 7
  returns a `vector<u32>` **by value** (hidden first argument, so 6 dwords): `(pathType, point*, entry, exit,
  radius)`, paths whose **last** point is within the radius (`≤`), nearest first. The exit is bits 0–7 of the path
  key (entry bits 8–15), not a record field; the first vertex is the first 12-byte point of the record's point array
  (record `+0`). GetPathInfo is borrowed (cached member, no AddRef): MATCHES. Slot 4 returns a pointer, all of EAX
  tested: MATCHES. Leaving lane continuity out **DIFFERS** next to occupants with network flags in `0x1F4F`.

<!-- from: From applying the traffic glue answers (2026-10-16) -->
- ~~**TG25:** traffic-network §4.8 step 1, §9.1 TG2: at what offset in cSC4NetworkTunnelOccupant is the paired-portal
  member, what type is it, and is it a counted reference? [`tunnel_paired_portal` stays NULL, so no tunnel record is
  made and tunnels carry no trips]~~ **ANSWERED 2026-10-16 (re-answers21):** traffic-network §4.8 step 1. `+0x1D0`
  from the primary base (the pointer QI `0x8A4BD52B`/`0xA821EF94` return; `+0x1C8` inside Write/Read, which run on
  the `+8` sub-object): a pointer to the other portal's `cSC4NetworkTunnelOccupant`, usable as `cISC4NetworkOccupant*`.
  Counted: the setter `0x00647530` (called only by `InsertTunnelPieces`) AddRefs the new and releases the old;
  Shutdown releases and nulls it; it is saved as an object reference. DoTunnelChanged reads it through `0x00647570`
  and holds a temporary reference while it reads the position. The bracket **DIFFERS** (tunnels carry no trips).

<!-- from: From applying the traffic glue answers (2026-10-16) -->
- ~~**TG26:** traffic-simulator §4.1, §9.1 TG19: when does the original release its source iterator's connection snapshot
  on a city close: at Shutdown, or only when the simulator is destroyed? And does a Read that replaces the state release
  it? [released at Shutdown and when Read replaces the state; otherwise at the next Reset]~~ **ANSWERED 2026-10-16
  (re-answers21):** traffic-simulator §1 (Shutdown) and §4.1. At Shutdown (`0x007196F8`–`0x00719765`: list cleared
  with a Release per connection, iterator freed, member nulled), which the city runs at close; the destructor does
  not touch it. Otherwise only the next `Reset`. `Read` never touches it, and in the original a Read always precedes
  the Init that creates the iterator. Shutdown: MATCHES. Release at Read: **DIFFERS** only for a Read into a live
  simulator, which the original never does; accepted. Side fact: the lot safe iterator inside it is never
  unregistered (quirk).

<!-- from: verify-traffic4 (2026-10-21; regress-train22 shadows log, traffic shadow original vs core) -->
- **TS67 - §3.2 row 15, §3.4, §4.7, §6.1 row 14 (phase positions +0x90C/+0x90E):** what sets the two s16 phase positions to (0, 0) between the last FullTrips (18) source and the first lot FinishTripMap (15) takes? Evidence: regress-train22 to 2032 (turbo), traffic shadow 59 of 72 month lines clean; all 13 differing months are one kind, a delivery `state 18 -> 15, progress 0` (the original finished 18 and stopped at the TS63 test point before its first lot) with `record phase positions: orig 00000000 ours 64008000`, i.e. the original (0, 0) and the core (100, 128), the cell the core's last 18 source wrote by `0x0071D51C`. The row-14 re-route context x/z (`+0x83C`/`+0x840`), written with the same cell at `0x0071D523`/`0x0071D538`, matched in every one of those deliveries. §3.2 lists only "lot iterator reset" for entering 15, and §3.5 step 11 / §4.7 only the per-lot write `0x0070FFE0`. Candidates: (a) entering 15 (or the 18 -> 15 transition in the cycle helper) zeroes +0x90C/+0x90E; (b) the FullTrips end path (`0x0071D806`-`0x0071D84E`, iterator exhausted) writes the iterator's end cell (0, 0) to the positions; (c) something else. Please give the address and whether 16 -> 17 and 17 -> 18 ends do the same. [literal reading: not reset; core `cycle.rs` keeps the cell, pinned by test `ts67_finish_trip_map_entry_keeps_phase_positions`]
  **ANSWERED 2026-10-21 (re-ts67-4):** DIFFERS — candidate (a): entering state 15 zeroes the phase positions. The cycle helper's finish path (`0x00721073`, next state from `0x00AB3480`; jump table `0x0072153C` via byte index `0x00721564`) runs the entry arm of the new state: states 15 and 1 share arm `0x0072110B` (lot iterator := first lot, +0x90C := 0 at `0x00721123`, +0x90E := 0 at `0x0072112A`, `+0x838` := 1); states 16, 17, 18 share arm `0x007210B0` (source iterator Reset, +0x90C/+0x90E := 0 at `0x007210BE`/`0x007210C5`, `+0x838` := 1, `+0x884` := 0, and for 18 the 9-entry vector `+0x430` := 0). So 16 -> 17, 17 -> 18 and 18 -> 15 all zero them; FullTrips' exhausted-iterator end (`0x0071D806`) writes nothing there (the only writes in the trip phases are the per-source/per-lot ones at `0x0071E276`, `0x0071D0FD`, `0x0071D51C`, `0x0070FFE0`). Re-route context `+0x83C`/`+0x840` is not reset (matches the shadow). Deadline stops run no entry arm. Rule in §3.2 (table rows 16/15/1 and "Phase positions across the trip phases"), §3.5 step 11, §4.7, §6.1 row 14. Core `cycle.rs` and test `ts67_finish_trip_map_entry_keeps_phase_positions` must change to zero on entering 15.
