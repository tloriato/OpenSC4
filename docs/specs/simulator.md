# Spec: the simulator service (`cISC4Simulator`)

Status: **implementation spec**, clean-room input. This document describes *behaviour*, not code. It was
written by the analysis team from observing the original program. Implementers must work from this spec,
the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled or
disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined.

## 1. Role

The simulator is the game's master clock and work scheduler. It:

1. keeps the **simulation date** (a day count) and advances it;
2. runs every subsystem ("agent") at its registered cadence, within a per-frame CPU **budget**, letting long jobs
   continue across frames;
3. broadcasts calendar messages (new day/week/month/year) and state changes (pause/speed);
4. persists its state in the city save file.

It is a GZCOM object with class ID `0x2990C1E5`. It implements these interfaces, each reachable through
`QueryInterface`:

| Interface | IID | Notes |
|---|---|---|
| `cISC4Simulator` | `0x8695664E` | primary |
| `cIGZSerializable` | gzcom-dll `GZIID_cIGZSerializable` | save/load (§8) |
| `cIGZSystemService` | per SDK | service ID `0x46956659`, service priority `-2000000` *[compat]* |
| `cIGZMessageTarget2` | per SDK | receives notifications, commands and cheats (§9) |
| `cIGZUnknown` | | |

The city object obtains the simulator through `GZCOM()->GetClassObject(0x2990C1E5, 0x8695664E, &p)`. Sub-systems
then reach it through `cISC4City::GetSimulator()`. Nothing in the game depends on the simulator's private memory
layout. Only these interfaces are used.

## 2. Time model

### 2.1 Date
- The date is a **Julian day number** (an integer day count). 2000-01-01 is day **2451545** *[compat]*.
- Calendar fields use the proleptic Gregorian calendar:
  - **day of month** 1..31, **month** 1..12, **year** (e.g. 2000);
  - **day of year** 1..366;
  - **weekday** 1..7 with **1 = Monday** and 7 = Sunday, so 2000-01-01 is weekday 6 *[compat]*.
- The date exposed through `GetSimDate()` is a `cIGZDate` object reflecting the current day.

### 2.2 Levels
Agents run at one of **11 levels**. The scheduler message for level *L* has type `0x66956810 + L`.

| L | Name | Message | Cadence |
|---|---|---|---|
| 0 | OneShot | `0x66956810` (SimBegin) | continuous: every frame until the agent finishes, then the agent is removed |
| 1 | Emergency | `0x66956811` | continuous: every frame while registered |
| 2 | Minute | `0x66956812` | each sim minute (see 2.3) |
| 3 | Hour | `0x66956813` | each sim hour |
| 4 | Day | `0x66956814` | each sim day |
| 5 | Week | `0x66956815` | each Monday |
| 6 | Month | `0x66956816` | each 1st of the month |
| 7 | Year | `0x66956817` | each 1st of January |
| 8 | SpecificDay | `0x66956818` | reserved; never fired by the clock *[compat]* |
| 9 | SpecificTime | `0x66956819` | reserved; never fired by the clock *[compat]* |
| 10 | Idle | `0x6695681A` | leftover frame time (§5.4) |

### 2.3 Interval counters
The simulator keeps a 32-bit **counter per level**. For the minute and hour levels this is a progress count. For
day/week/month/year it holds the value last announced, and is sent as `data3` of that level's messages:

- minute counter: 0..60. The hour can roll when it reaches 60.
- hour counter: 0..24. The day can roll when it reaches 24.
- day counter := day of year of the new date
- week counter := `(dayOfYear − 1) / 7 + 1` (integer division)
- month counter := month (1..12)
- year counter := year

If **no agent is registered** at the minute level, the minute counter jumps straight to 60 in one step. The same
applies to the hour level and 24. In the shipped game no agent uses minutes or hours, so the day is the effective
step *[compat]*.

## 3. State

| State | Initial value | Persisted (§8) |
|---|---|---|
| initialized | false | no |
| inTick (re-entrancy guard) | false | yes (written as a byte; ignored on read) |
| cityEstablished | false | yes |
| pauseCount | 0 | yes |
| hiddenPauseCount | 0 (set to 1 by `Init`) | no |
| emergencyPauseCount | 0 | no |
| simSpeed | 3 (= unset) | yes |
| simTime | 0 | yes |
| speedChangeEnabled | true | no |
| frameBudgetMs | 15 | yes |
| gateLevel | 4 (Day) | no |
| msPerDay | 100 | no (recomputed) |
| targetFps | 15 | no (recomputed) |
| msPerDayTable[3] | {5000, 1000, 100} (overridden by tuning, §7) | no |
| animationDilation[5] | {0, 1, 1.5, 2, 1} (overridden by tuning) | no |
| levelCounter[11] | all 0 | yes |
| levelFired[11] | all false | yes |
| date | 2000-01-01 | yes (as a Julian day) |
| agents[11] | empty ordered collections | no |
| cursor[11] | position in agents[L], initially at end | no |
| idleCursor | at end | no |
| three timers | frameTimer, intervalTimer, sliceTimer (millisecond resolution) | no |

## 4. Agents

An **agent** is a `cIGZMessageTarget2*` plus a level, a name (string) and flags.

- `AddAgent(target, level, name, flags) -> bool` *[compat]*:
  - If `level ≥ 11`, do nothing. In both cases return true.
  - The agent is **enabled** if `cityEstablished ? (flags & 2) : (flags & 1)`.
  - A target may appear at most once per level. Adding it again is a no-op.
  - The simulator holds a reference: `AddRef` on add, `Release` on removal.
  - Order within a level: *[choice]* **registration order**, with a fixed-rank tail for month-level agents decided
    in `foundations.md` §13.4. The original ordered agents by object address, which is not reproducible. **Order is
    behaviourally significant** — `landfill.md` §3.3 and `foundations.md` §13.3 derive observable requirements from
    it (whether this month's landfill intake is reported and charged this month or next; whether the budget books
    this month's or last month's ordinance and neighbour-deal totals) — so the earlier claim that "no behaviour may
    depend on it" was wrong and is corrected here. **The tail mechanism is deferred until a tail agent's glue
    actually lands** (§13.4 "Sequencing"): the core is merged and verified in-game with plain registration order,
    and none of the tail agents is wired up yet, so adding ranks now would rework verified code for no behavioural
    change. Until then, registration order alone is correct and must keep passing.
- `RemoveAgent(target, level) -> bool`: removes that registration. If the level's cursor is on it, the cursor first
  moves to the next agent. Returns true if something was removed, false otherwise (including `level ≥ 11`).
- `RemoveAgent(target) -> bool`: the same for every level. Returns true if any registration was removed.
- `RemoveAllAgents(level)` and `RemoveAllAgents()`: clear the level (or all levels). Their cursors move to end. Return true.
- `SetCityEstablished(b)`: if the value changes, store it and recompute every agent's *enabled* bit from its flags
  (overwriting any `SetAgentEnabled` override). Returns true.
- `GetAgentEnabled(name) -> bool`: true if **any** registration whose name equals `name` (case-sensitive) is
  enabled.
- `SetAgentEnabled(name, b)`: sets *enabled* on **every** registration with that name. Returns true.
- `EnumerateAgentsByName(vector<SC4String>&)`: appends the name of every registration, level by level. Returns true.

**Delivering an agent message**: create a `cIGZMessage2Standard`-compatible message with
`type = 0x66956810 + L`, `data1 = current Julian day`, `data2 = 0`, `data3 = levelCounter[L]`, then call the agent's
`DoMessage` synchronously. Its **boolean result means "finished"**:
- **true**: the agent completed this interval's work;
- **false**: it wants to be called again with the same message on a later frame.

Disabled agents are skipped and count as finished.

## 5. Per-frame behaviour

### 5.1 `OnTick` (cIGZSystemService)

```
elapsed := frameTimer.elapsedMs();  frameTimer.restart()
target  := (pauseCount ≤ 0 and hiddenPauseCount ≤ 0) ? 1000 / targetFps : 33
frameBudgetMs := max(15, target − elapsed)
if initialized and not inTick:
    inTick := true;  sliceTimer.restart()
    repeat:
        if hiddenPauseCount > 0: stop
        if pauseCount ≤ 0:
            SimulateTimePassage();  SimulateAgents();  SimulateIdleTime()
        else:
            SimulateIdleTime()
    until sliceTimer.elapsedMs() ≥ frameBudgetMs
    inTick := false
return true
```

A higher speed gives the simulator *more* time per frame (a lower target fps means a larger budget).

### 5.2 `SimulateTimePassage`

1. Clear all `levelFired[]`.
2. If any agent is registered at level 0 or level 1, return. Calendar time does not pass while one-shot or
   emergency work is pending.
3. `ready := intervalTimer.elapsedMs() ≥ msPerDay`.
4. `done[L] := (cursor[L] is at end of agents[L])` for L = 2..10.
5. Let `tomorrow := date + 1`. Compute `startsWeek := tomorrow.weekday == 1`, `startsMonth := tomorrow.day == 1`,
   `startsYear := tomorrow.dayOfYear == 1`.
6. **Minute**: if `done[2]` and (`gateLevel ≠ 2` or `ready`):
   - if `gateLevel == 2`, restart the interval timer;
   - if level 2 has no agents, set `levelCounter[2] := 60`; otherwise set `levelFired[2] := true` and increment
     `levelCounter[2]`.
7. **Hour**: if `levelCounter[2] ≥ 60` and `done[3]` and (`gateLevel ≠ 3` or `ready`):
   - set `levelCounter[2] := 0`; if `gateLevel == 3`, restart the interval timer;
   - if level 3 has no agents, set `levelCounter[3] := 24`; otherwise set `levelFired[3] := true` and increment
     `levelCounter[3]`.
8. **Day**: if `levelCounter[3] ≥ 24` and (`gateLevel ≠ 4` or `ready`) and `done[4]` and none of
   (`startsWeek and not done[5]`, `startsMonth and not done[6]`, `startsYear and not done[7]`):
   - set `levelCounter[3] := 0`; if `gateLevel == 4`, restart the interval timer;
   - set `levelFired[4] := true`; advance `date` by one day;
   - set `levelCounter[4] := dayOfYear`, then **post** `SimNewDay(0x66956814, data1 = julian, data2 = 0, data3 = dayOfYear)`;
   - if weekday == 1: `levelFired[5] := true`, `levelCounter[5] := weekOfYear` (2.3), post `SimNewWeek`;
   - if day == 1: `levelFired[6] := true`, `levelCounter[6] := month`, post `SimNewMonth`;
   - if dayOfYear == 1: `levelFired[7] := true`, `levelCounter[7] := year`, post `SimNewYear`.

"Post" means a broadcast through the message server's `MessagePost(msg, false)` (queued), so it is visible to
every subscriber. This is separate from the synchronous agent delivery in §4.

### 5.3 `SimulateAgents`

1. For each level L = 2..7 with `levelFired[L]`: **BeginNewInterval(L)**:
   - if level L has agents: deliver the level-L message to every agent from `cursor[L]` to the end, **repeating
     each agent until it reports finished** (a synchronous flush of the previous interval's unfinished work);
     then set `cursor[L]` to the first agent.
2. Then walk levels in order, starting at L = 0:
   - If `cursor[L]` is at end:
     - for L ∈ {0, 1} with agents registered: *[choice]* set `cursor[L]` to the first agent and keep going at that
       level (these levels run continuously);
     - otherwise go to L + 1. Stop when L reaches 10. The idle level is not handled here.
   - Otherwise deliver the level-L message to the agent at `cursor[L]`. If it is disabled, or reports finished,
     advance `cursor[L]`. A finished level-0 agent is then **removed** from level 0.
   - After each delivery, stop if `sliceTimer.elapsedMs() ≥ frameBudgetMs`.

Consequence *[compat]*: a level's work for one interval can span many frames, but the clock (5.2) will not open the
next day until the day level, and any week/month/year level due tomorrow, has drained.

### 5.4 `SimulateIdleTime`
If `hiddenPauseCount ≤ 0` and the idle level has agents:
- If `idleCursor` is at end, move it to the first idle agent. Let `share := frameBudgetMs / 3`.
- Repeat until `idleCursor` returns to end:
  - Let `t := sliceTimer.elapsedMs()`. Stop if `t ≥ frameBudgetMs`.
  - Take the agent at `idleCursor`, then advance the cursor.
  - If it is enabled, deliver `SimIdle(0x6695681A, data1 = julian, data2 = (pauseCount > 0 ? 1 : 0),
    data3 = min(frameBudgetMs − t, share))`. Its result is ignored.

### 5.5 CPU-cycle limit-timer calibration *[compat]*

Several original subsystems (for example the lot developer's daily pass) time-slice their own work with a
**CPU-cycle budget**: they read the processor's time-stamp counter (TSC) and stop after a number of cycles equal to
"N milliseconds" (5 ms for the lot developer). They convert milliseconds to cycles with two **process-wide
coefficients** that the simulator owns and refreshes:

| Global | Meaning |
|---|---|
| `CyclesToMillisecs` | milliseconds per TSC cycle (`calib`) |
| `MillisecsToCycles` | TSC cycles per millisecond (`1 / calib`) |

If the simulator does not maintain them they keep their static value of 1.0. A "5 ms" budget then becomes 5 cycles,
those subsystems make no progress, return "not finished" forever, and the day never advances (observed in-game).

Calibration (`calib`, a float):
- **Initial value**: an estimate of milliseconds per TSC cycle (for example, measure the TSC against a
  high-resolution wall clock over a few milliseconds at construction). Publish both globals at construction.
- **On every `OnTick`, before anything else**: read the wall clock (`QueryPerformanceCounter`, ticks `q`) and the TSC
  (`c`). If more than **one second** of wall-clock ticks (`qpcFrequency`) has passed since the last sample:
  `sample = (q − q_last) × (1000 / qpcFrequency) / (c − c_last)` (ms per cycle), and if `c − c_last > 0`:
  `calib := 0.9 × calib + 0.1 × sample`. Then set `q_last := q` and `c_last := c`. Before the first sample, the last
  values come from construction.
- After the update, publish `CyclesToMillisecs := calib` and `MillisecsToCycles := 1 / calib`.

**Locating the globals** (an interop fact for the 1.1.641 Windows executable; there is no GZCOM interface for them):
`CyclesToMillisecs` is the 32-bit float at virtual address `0x00B0A840`, and `MillisecsToCycles` is the float at
`0x00B0A83C` (image base `0x00400000`). An implementation **must** confirm it is running inside exactly that
executable before writing (for example, check the module's PE timestamp/size or a byte signature at those
addresses, both holding `1.0f` before first use).
Identification values for the 1.1.641 GOG executable: PE `TimeDateStamp` = `0x4C12BFB7`, `SizeOfImage` = `0x007C8000`. Otherwise it must not write, and must log a warning. Keep such
version-specific addresses in a single compatibility table (`engine/plugin/compat/`).

**Adaptive slicing helper** *[compat]*. Several agents (aura, growth developer, land value, plumbing, residential)
use one shared helper (`0x0068CC20` / `0x0068BF50` / `0x0068BF80` / `0x0068B960`) on top of this budget. The
deadline is `TSC + trunc(float32(ms × MillisecsToCycles))`. Each processed item adds the current `step` to a
counter, and the clock is read only when the counter reaches a multiple of 1024. At the end of each call,
`n = counter >> 10`; `n > 20` increments a miss counter (at 16 the step halves), `1 ≤ n < 5` decrements it (at −4
the step doubles), `n = 0` changes nothing. `step` (initially 1024) and the miss counter are a **process-wide
static per caller** (aura `0x00B47B60`, growth developer `0x00B49098`, land value `0x00B49104`, plumbing
`0x00B49B4C`, residential `0x00B4AFB0`). The slicing affects only how much work happens per call, never the
results; aura §6.5 gives the details. *[choice]* FreeCity4 may keep the step per simulator.

### 5.6 Verified slot order of `cISC4Simulator` overloads *[compat]* (see foundations §16)

| Slot | vtable offset | Method (as the game expects it) |
|---|---|---|
| 6 | `+0x18` | `void GetSimDate(uint32* year, uint32* month, uint32* day, uint32* dayOfYear, uint32* weekDay)` |
| 7 | `+0x1C` | `cIGZDate* GetSimDate()` |
| 20 | `+0x50` | `bool RemoveAgent(cIGZMessageTarget2* agent)` |
| 21 | `+0x54` | `bool RemoveAgent(cIGZMessageTarget2* agent, int32 level)` |
| 22 | `+0x58` | `bool RemoveAllAgents(int32 level)` |
| 23 | `+0x5C` | `bool RemoveAllAgents()` |

This is the compiled layout; MSVC produces it from the SDK header's source order (foundations §16). Every other `cISC4Simulator` slot matches the SDK.

### 5.7 Work bound to frames or real time, not to game days *[compat]* (DG31)

The calendar (§5.2) makes day-level work complete before the next day, but three kinds of original work are paced by
**frames and milliseconds**, so the same city evolves differently at different days per real second. The amount
that matters is the **idle work per game day**: idle agents get one delivery per idle pass (§5.4), each with at most
`frameBudgetMs / 3` ms, every frame, however many days that frame advances. At the shipped cheetah speed (50 ms per
day, §7: 20 days per second) the idle level gets on the order of 50 ms per day; at 85 days per second it gets about 12
ms per day minus the day-level work, and agents that do one step per delivery get roughly one idle step per day.

1. **Idle-driven cycles with a per-day bound.** One step per NewDay plus any number of idle steps, and the day is held
   open once the cycle falls behind (`state < today − start` returns "not finished"): growth developers
   (growth-developer §3.1: the monthly map update, states 1–20), aura (aura §5.1, the SimNewDay rule), and plumbing
   (plumbing §6: states 6 and 7 time out with "not finished"). At normal speed idle steps finish the cycle within its
   first day; at high speed it runs on the daily step and finishes up to about 20 game days later (growth developers).
   Nothing is skipped, but every reader sees older maps for longer, and work that waits for the cycle moves later in the
   month: the BDS's decline plan (building-development §4.2) and the growth/decline lists (valid only while
   `state < 17`).
2. **Idle-driven cycles without a per-day bound** that a month message restarts only when they have finished: the
   demand census (demand §6.2: `DoIdle(5)` per day plus idle; a census still running at month start is not
   restarted, and the tax and economy modifiers of demand §8.1 are recomputed only at the start of each census),
   land value (landvalue §5: a pass starts at a month only if the previous one ended), crime (crime §5: a pipeline of
   steps, one per day or idle delivery), power (power §4.3: one step per day or idle delivery) and the BDS monthly
   rebuild (building-development §4.1: a month that arrives while the developers' updates are still running is skipped,
   with its construction-resource update). When a cycle takes longer than a month in game days, whole months are
   skipped: the census-driven supplies (R/C/I census, demand §6.4) and with them the BDS construction resources and the
   developers' demand change less often.
3. **Real-time budgets inside day work.** The BDS daily growth stops after 3 ms (building-development §4.4); the
   developers it does not reach get **no attempt that day**. This depends on the machine and its load, not on the
   speed, as long as the cycle calibration of §5.5 is maintained. The other CPU-cycle budgets (lot developer 5 ms,
   the adaptive helper users of §5.5) return "not finished" and resume, so they cost real time but lose nothing.
4. **Frame-bound delivery.** Messages that are posted (not sent) are delivered at a later message-server tick
   (foundations §5.1): `SimNewDay` / `SimNewMonth` broadcasts to subscribers that are not agents, and lot
   `StateChanged` (`0xE8BDA2E7`, lot-developer §3.2), which the demand simulator, the BDS and the growth developers
   handle. When one frame advances several days, those handlers run after the agents of all of those days.

Consequences for comparisons: original runs are comparable only at equal pacing, and only at speeds where the idle
level finishes its cycles within a day or so (the shipped speeds, up to about 10–20 days per second). *[choice]*
FreeCity4's own engine does not depend on frames: each cycle of kind 1 and 2 runs to completion at the step where
the original, at normal speed, would have finished it (within the day it starts), the 3 ms cut of kind 3 is replaced
by the daily growth budget (building-development §4.4.1), and posted messages are delivered before the next day's
agents. A harness that drives the original faster than about 20 days per second measures the pacing, not the
simulation.

#### 5.7.1 Work per day and per cycle at the shipped tuning (DG31)

**Deliveries per day.** Each round of the `OnTick` loop (§5.1) opens at most one day (§5.2 step 8) and then makes
one idle pass (§5.4). An idle pass gives every enabled idle agent one delivery, unless the frame budget runs out
first; the next frame then continues from `idleCursor`. So with K idle passes per day, each idle agent gets **K**
idle deliveries per day plus its **one** day delivery (more when its day delivery reports "not finished", §5.3).

- At the shipped speeds K is large: a 50 ms day holds many short rounds.
- When the clock opens a day in every round (a day shorter than a round, as under a turbo clock), **K ≤ 1**. K is 0
  for the agents that a frame-ending pass did not reach.
- Each idle delivery offers `min(frameBudgetMs − t, frameBudgetMs / 3)` ms (§5.4), at least 0 and at most 22 ms (the
  budget is at least 15 ms and at most 66 ms).

**Per-cycle amounts.** "Call" means one day or idle delivery to that agent. A time-sliced state processes items until
its millisecond budget runs out, so how many items fit in a call depends on the machine. The code fixes only the
item count; the clock is read through the adaptive helper of §5.5.

| Agent | One call does | Calls per cycle | What starts a cycle | A month during a cycle |
|---|---|---|---|---|
| Growth developer (12 shipped types, each with its own day and idle agents; growth-developer §3) | one state; state 15 applies one special-building effect; state 18 updates tracts for up to the ms budget (day: 5 ms) | **20 + E** at least (E = special buildings with an effect for that developer type), plus extra state-18 calls when the tracts (64², 128², 256² cities: 256, 1,024, 4,096 at S = 4) do not fit in one budget | the BDS month, only when the BDS is idle (building-development §4.1) | the day is held open while `state < elapsed days` (kind 1). The highest state is 20, so a cycle ends by about day 21 of its start; a slower pace slows the calendar instead |
| BDS rebuild (building-development §4.2) | waits for all 12 developers, then one more call for `PlanDecline` | the slowest developer's cycle + 2 | its month handler when `state = 0` | **skipped**, with its construction-resource update; but the developers' kind-1 bound ends them by about day 21, so this needs a month shorter than that |
| Aura (aura §5.1, §6) | one state; state 3 one landmark department; state 5 lots for up to the ms budget | **17 + N** at least (N = departments in the landmark map), plus extra state-5 calls | month, if state 0 | day held open while behind (kind 1): the cycle ends by about day 18 |
| Plumbing (plumbing §6) | one state; states 6 and 7 cells for up to the ms budget (5 ms on the day, which then reports "not finished") | **2W + 7** at least (W = water networks flooded), plus extra calls for states 6 and 7 over all city cells (4,096 / 16,384 / 65,536) | the recompute flag: every month, every building or pipe change, every pipe burst and water-deal change | not lost: the flag stays set and starts the next pass |
| Demand census (demand §6.2) | state 0 runs straight into state 1; state 1 runs `PerformCensus` on cells for up to the ms budget; state 2 records history | **2** at least, plus as many calls as the cells (4,096 / 16,384 / 65,536, every cell visited) need | month, only from state 3 | **skipped** (kind 2): no new census or modifiers that month |
| Land value (landvalue §5, §6) | one state; state 7 one developer; states 1, 3, 4, 5 and 8 tracts for up to the ms budget | **21** at least (9 + 12 developers; 22 with the zoning fallback), plus extra calls for the tract states (64², 128², 256²: 1,024, 4,096, 16,384 tracts at S = 2; slope 64, 256, 1,024) | month, only when the pass is finished and `dirty` | deferred: `dirty` stays set, so the next month starts a pass |
| Crime (crime §5) | one state, no slicing | **6** | month, only from state 6 | **lost**: no crime pass for that month |
| Power (power §6) | one state, no slicing (the wire rebuild of §5.3 runs first in the same call) | **5** | the recompute flag: every month and every relevant occupant or zoning change | not lost: recompute stays set |

**Predicting skipped months at a given pace.** With D game days per month (28 to 31) and K idle passes per day:

- **Step agents** (crime, power, land value, aura, growth developers): a cycle of C calls takes about C / (1 + K)
  days. At K = 1 this is 3 days for crime and power, about 11 for land value, 9 + N/2 for aura and 10 + E/2 for
  each growth developer. Crime loses a month only if fewer than 6 deliveries arrive in a month, which needs
  K ≪ 1.
- **The BDS** cannot skip a month at any K. Its developers' kind-1 bound finishes them by about day 21, and the BDS
  needs one or two more calls after that. A large E or a small K holds days open (the calendar slows) instead.
- **The demand census** is bound by time, not calls. It skips months when its CPU time T (all cells) exceeds about
  D × (5 + K × m) ms, where m is the share of an idle delivery that it actually gets. The code does not fix T; it
  has to be measured on the machine (one census of the city's cells).
- **Plumbing, aura and the growth developers** never skip a month. Kind 1 holds the day open, which slows the
  calendar (days per real second) instead.

*[inferred]* The table counts calls at their minimum and assumes that each time-sliced state finishes in one call.
On a current machine that holds for every per-tract state at these sizes. The census of a 256² city is the most
likely exception.

### 5.8 The fixed schedule *[choice]*

The original is not deterministic (§5.5, §5.7, foundations §6, §12). Under the **fixed schedule**, the same city,
actions and session seed `S` give the same save, compared record by record after decompression. FreeCity4's game
**must** follow these rules. The original follows them through `FreeCity4.dll` (`schedule = fixed`) and is the
reference. Everything is counted from the city load; the start-up screens are outside the schedule.

| Part | Rule |
|---|---|
| Calendar | The simulator with a fixed step: each frame (one `OnTick`) makes 64 agent deliveries, and a day opens only after every day-level agent has finished (§5.2). |
| Clock | The main thread reads one virtual counter. It starts at the load, advances 1 tick per read, 33 ms per frame and `n` ms per sleep of `n` ms. A stopwatch (timers.md §1) still running at the load reads as started at the load, with nothing accumulated. |
| Timer callbacks | The timer schedulers' callbacks (timers.md §2.5) run on the main thread at the start of each frame. They run in each scheduler's list order, by a clock that is 0 at the load and advances 33 ms per frame. Each entry is first due one period after the load. A one-shot entry is removed when it fires. A fixed-rate entry is next due at `due + period`; any other at `now + period`. Nothing fires on the scheduler's own thread. |
| Time limits | A `LimitTimer` (small-owners-b9.md §4) counts reads from its `Start`. The building developer's Grow timer (building-development.md §4.4) expires at its 16th read; every other timer expires at its 2²⁴th read. Doubling 2²⁴ must not change any result. The traffic remaining-time query (`0x0070E2B0`) answers 1 ms. |
| Wall clock | `time()` = 1767225600 (2026-01-01 00:00 UTC). The `clock()` reads of the generators' seeding (`cRZRandom`) return 1000. |
| Random numbers | The file-local generators are seeded with `derive(K)` (foundations §6.3) at start-up and again at the load. `rand()` (foundations §12) is `srand(derive(0x4C0AD000))` at the load. Before the `n`-th agent delivery (from 0) of day `d` (the Julian day last opened), `rand()` is `srand(S XOR (d × 0x9E3779B1) XOR (n × 0x85EBCA77))`, in 32-bit arithmetic. A generator the original seeds from the clock gets `derive(K)` with the `K` of foundations §6.3: the owner's GZCLSID; for each MySim, the city detail increase pass, the pipe burst start and the civic monthly generator, XOR the number of earlier seeds at that site since the load. The traffic simulator's `Init` seed and the prop developer's (`derive(K) × 28`) are replaced whatever their value. The flammability generator's zero-state reseed (its first fire roll, flammability.md §6.8) gets `derive(0xE990C045)`. Other clock seeds keep the `time()` and `clock()` above. |
| Scripted input | A test script's UI actions wait on the Clock above, never on the wall clock: `uiwait n` lasts `n` ms, and every other UI action is followed by 150 ms. |
| Object IDs | The save's reference writer (foundations §2.7) numbers objects 1, 2, 3, … in the order it first writes them, from 1 in each save. |
| Address order | Where the original walks a container keyed by object address, the walk is in the objects' creation order, first created first; a load creates the saved objects in the order of foundations §3.5. So far: the automata controller manager's save, its occupant entries and queues (automata.md §3.2, §3.3), its walks of the occupants at each animation tick (automata.md §3.1), the traffic network map's save (record `0x6A0F82B2`: its intersections and its last table, network occupants to four lists) and its rebuild of the intersections, the targets of each animation tick class (animation-tick.md §4), the city situation manager's pick and match of automata (city-situation.md §4.2), the watercraft manager's save and its creation of ferries and marina boats at each animation tick (automata.md §5.3), and demolition's removal of a region's occupants and its network update (demolition.md §4.4 step 5, §4.6). The zone developer's lot size choice walks its lot configurations in ascending ID instead (zones.md §7.2.1). The neighbours simulator's adjacent cities are in region position order, x then z (neighbors.md §9.1). An object made before the city load (at start-up) comes after those made since, in address order (as for the walks below). |
| Uninitialised values | What the original copies from an uninitialised stack temporary into results or the save is 0: a transit switch inserted for an unknown ID (traffic-network.md §4.7.3), and a new intersection's four sides (state, flag, three f32) and flag in the traffic network map (`0x6A0F82B2`). The original's add-intersection (`0x004B4300`) sets only the temporary's vector and list; reading a save sets all of them. A new train junction's four u16 junction indices (the network map's last table, network occupant to four (path list, index) pairs): the original's add-junction (`0x004B44F0`) sets a side's index only when that side has a path, and the game never reads the index of a side without one. Heap memory the original reads before writing it is 0 as well: a reused heap block or pool page is cleared, as fresh memory is (the history record `0x89EFA536` read a stale value otherwise). |

**The original only.** These make the original repeat its own heap addresses. They are not rules for FreeCity4's
game:

- Audio and the background model loader are off (`-audio:off`, `-backgroundLoader:off`). The sound update frees
  exemplar properties on the timer thread, and the loader allocates them on its own thread, both from unlocked
  pools.
- `FreeCity4.dll` loads at a fixed base.
- The city loads at the first frame on which the region lists its cities. About 3 s (wall clock) after the region,
  the region screen seeds its clouds from the clock (`0x007A9B66`) and allocates on the main thread; a run in which
  any clock seed comes before the load logs a warning and is not a reference.
- Every test slot's user directory has a path of one length (`slot01` to `slot16`): the game copies the city's
  path into the main thread's heap, and a longer path moves every block after it.
- From the load, the main thread allocates from one region at `0x20000000`. The free lists hand out the lowest
  address first.
- The 45 static object pools start fresh pages at the load.
- Each process has a temp directory of its own, empty at start-up. The original names temporary files from `time()`
  and counts down while a name exists (`0x0091A497`), so games side by side would race for the same names.
- The game is never the foreground window (`0x005F85F0` answers false): every save writes the default region-view
  thumbnail after its wait, instead of rendering one when the game happens to be in front.
- The game's windows are minimized at the city load and again at any frame that finds one shown: a shown window is
  painted when other windows uncover it, and the paint creates the screen's draw context on the main thread's heap.

Where an original result still depends on address order (a walk not yet in the Address order row), the reference
is the order under this heap; agent order (foundations §13) is an example. Such an order is not yet a rule for
FreeCity4's game.

## 6. Speed and pause

| Method | Behaviour |
|---|---|
| `Pause()` | `pauseCount += 1`; on 0→1, post `0xAA7FB7E0` (data1 = 1). Return true. |
| `Resume()` | **only if `speedChangeEnabled`**: `pauseCount −= 1`; on reaching 0, post `0xAA7FB7E0` (data1 = 0). Return true. |
| `HiddenPause()` / `HiddenResume()` | the same with `hiddenPauseCount` and message `0x4A7FB7E2`. There is no speed-lock check. |
| `EmergencyPause()` | call `Pause()`; set `speedChangeEnabled := false`; `emergencyPauseCount += 1`; on 0→1 post `0x4A7FB807` (data1 = 1). |
| `EmergencyResume()` | set `speedChangeEnabled := true`; call `Resume()`; `emergencyPauseCount −= 1`; on reaching 0 post `0x4A7FB807` (data1 = 0). |
| `IsPaused()` / `IsHiddenPaused()` / `IsEmergencyPaused()` | the matching count > 0 |
| `IsAnyPaused()` | any of the three counts > 0 |
| `GetSimSpeed()` / `SetSimSpeed(s)` | `SetSimSpeed` returns false if `!speedChangeEnabled`. Otherwise, if `s ≠ simSpeed`, it applies **DoSetSimSpeed(s)**; return true. |
| DoSetSimSpeed(s) | For `s` ∈ {0, 1, 2}: set `targetFps` to 30/20/15, `simSpeed := s`, `msPerDay := msPerDayTable[s]`, restart the interval timer, and post `0x4A7FB7E9` (data1 = s). **Any other value changes nothing and posts nothing** *[compat]*. |
| `GetSimTime()` / `SetSimTime(t)` | plain accessor for `simTime`. It has no other effect. |
| `SetMaxMillisecondsPerTick(ms)` | accepted and ignored *[compat]*. Return true. |
| `GetAnimationTimeDilation()` | `dil[4]` if emergency-paused; else `dil[0]` if paused; else `dil[2]` for speed 1, `dil[3]` for speed 2, `dil[1]` otherwise. *[choice]* The original also returns `dil[4]` while the vehicle manager is in "driving" mode. We will add that when the vehicle manager is specified. |

Mapping to the UI: turtle = speed 0, llama = 1, cheetah = 2.

## 7. Tuning

`Init` reads the exemplar with TGI type `0x6534284A`, group `0xE7E2C2DB`, instance `0xE9590E52` ("Main
Simulator"), through the game's resource manager, as a property holder:

| Property | Type | Effect |
|---|---|---|
| `0x89590E6D` | uint32[3] | `msPerDayTable`. It is only used if it has exactly 3 values. Shipped data: {2000, 1000, 50}. |
| `0x89590E6E` | float[5] | `animationDilation`. Shipped data: {1, 1, 1.5, 3, 1}. |

If the exemplar or a property is missing, the defaults in §3 apply.

## 8. Persistence (`cIGZSerializable`)

`GetGZCLSID()` returns `0x2990C1E5`. The record is little-endian and written in this order *[compat]*. Existing
saves must load, and saves we write must load in the original game:

```
uint16  version = 4
uint8   inTick
uint8   cityEstablished
int32   pauseCount
int32   simSpeed
int32   simTime
uint32  frameBudgetMs
11 × { uint32 levelCounter[L]; uint8 levelFired[L] }     L = 0..10
uint32  julianDay
```

- `Read` must fail (return false) for `version ≥ 5`.
- For `version < 4`, a serialized timer sits between the level entries and `julianDay`. Read it with the stream's
  timer format and discard it. The timer is `u8 units` (always 3), `u64 elapsed`, `u8 running` (11 bytes). Version
  3 **must** be read (corrected): 407 of the 417 corpus saves hold it (save-records.md §7.6) *[compat]*.
- After `Read`, the speed is re-applied by `Init` (`msPerDay` and `targetFps` are derived, not saved).

## 9. Lifecycle, notifications, commands and cheats

`Init()` (does nothing if already initialized):
1. Set `initialized := true` and `hiddenPauseCount := 1`. The game starts hidden-paused until the city view resumes it.
2. Read the tuning values (§7).
3. Subscribe on the message server to `0x26D31EC1` (**PostCityInit**: the city has finished initializing) and `0x8BAC4B43` (command).
4. Register the command `0x8BAC4B43` with the name `"GetSimulationDate"` with the command server.
5. Set every cursor to the start of its level.
6. If `simSpeed == 3`, call `SetSimSpeed(0)`. Otherwise apply `DoSetSimSpeed(simSpeed)`.
7. Start the frame timer and the interval timer.
8. Register with the framework as a system service and add it to the tick list.
9. With the cheat code manager:
   - register cheat `0x0AC07E24` with the text `"FPS"` and subscribe to it;
   - if the app is in debug mode, also register `0x8A78BEFF` with the text `"SimDate"` and subscribe to it.

`Shutdown()` (does nothing if not initialized):
1. Unregister both cheats.
2. Reset the date to 2000-01-01 and set `simSpeed := 3`.
3. Remove all agents.
4. Unsubscribe the notifications and release the message server.
5. Unregister the command.
6. Remove the service from the tick list and from the framework.
7. Set `initialized := false`.

`DoMessage(msg)` handles:
- `0x26D31EC1` (PostCityInit): unsubscribe from it and call **`HiddenResume()`**. This releases the hidden pause
  that `Init` set, so the clock starts running once the city is ready *[compat]*. (Erratum: an earlier revision
  said `Shutdown()`, which was wrong.)
- `0x8BAC4B43` (GetSimulationDate command): fill the command's output parameters named "year", "month",
  "day of month", "day of week", "day of year" and "number" (Julian) with the current values.
- Cheat notifications (cheat code manager message `0x230E27AC`, data = cheat ID and argument string):
  - `FPS <n>`: set `targetFps := n`.
  - `SimDate <m> <d> [<y>]`: set the date (keep the year if omitted), then run `SimulateTimePassage` once.
- Anything else is ignored. Return true.

## 10. Conformance tests (must exist in `engine/tests`)

1. Date conversions: 2000-01-01 ↔ 2451545 and weekday 6. Leap years 2000 (leap), 2100 (not), 2400 (leap).
2. With a day agent that always finishes, speed 2 and `msPerDay = 50`: after enough simulated time, exactly one
   day message per ≥ 50 ms, carrying data1 = Julian day and data3 = day of year.
3. A day agent that needs 3 calls per day delays the next day until it finishes, even when `ready` is true.
4. Week, month and year messages fire on Monday / the 1st / January 1st with the documented `data3`, and only
   after their own agents have drained.
5. One-shot agents block calendar time until they finish, and are then removed.
6. Pause/Resume counting, the emergency speed lock, and posting on 0↔1 transitions only.
7. Enabled-bit selection by `cityEstablished` and the `SetAgentEnabled` override.
8. Save round-trip (write → read → write gives identical bytes) and rejection of version 5.
9. Limit-timer calibration: with a fake clock and fake TSC, `calib` follows the 0.9/0.1 average once per second of
   wall time, and both published coefficients are updated (the publish target is injectable for tests).
