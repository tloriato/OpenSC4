# disasters: answered questions (archive)
<!-- Archive for [disasters.md](../../disasters.md). Lanes do not read this file. Open questions are in ../disasters.md. -->

## From the disaster layer core (filed by the coordinator from the `cores13` lane's report, 2026-10-15; merged `333ecde`)

cores13 implemented disasters.md §2-§14 as a Rust core (`engine/core/src/disasters/`), a C ABI (`fc4_disasters`,
ABI 57) and conformance vectors (`ds18_*`, `ds00_*`). Q4 (the riot's removal lock) is answered by network-manager.md
§10 and implemented. There is no plugin glue yet, so none of these readings has been checked in game. The lane's own
numbers (its Q6-Q25, continuing disasters.md §17) are given in parentheses; the current choice is in brackets.

<!-- from: From the disaster layer core (2026-10-15) -->
- ~~**DS1:** (Q6) the fire generator's class and seed are unspecified. [the core takes them from the host generator]~~ **ANSWERED 2026-10-15 (re-answers14):** MATCHES in effect. The original's fire generator is a `cRZRandom` member of the fire instance constructed with seed `0xFFFFFFFF` (clock-seeded, `0x0057AD90` → `0x00913A5C`), shared only with the toxic spill. It is not reproducible in the original, so foundations §6.3 lets the host seed it, provided it is a `cRZRandom` private to the fire with both the `lcg` and `mul` paths. disasters.md §1, §13.

<!-- from: From the disaster layer core (2026-10-15) -->
- ~~**DS2:** (Q7) which draw method (raw, u32, double, range, gaussian) does each random use? [the spec text as far as it
  goes]~~ **ANSWERED 2026-10-15 (re-answers14):** **DIFFERS** where the old spec text was wrong or silent. Raw `RandomUint32Uniform()` for every fire ID; `RandomDoubleRangeUniform(0, 100)` for `IgniteCell`, with ignition iff `r ≤ p` (`p` a `float`); `RandomUint32Uniform(k + 1)` for the resistance; `RandomDoubleRangeUniform(0, 1)` for the spread, compared `r < 1.0f/D4[s]` and drawn **only when the step reaches the spread** (the old text said always); two `RandomDoubleRangeGaussianFast` (x then z) for the spread cell; `RandomUint32Uniform(4000)` for `ExtinguishFire` only when no time is set; and a draw the old text missed, `RandomUint32Uniform(n)` per insertion into a non-empty toast list. Riot: two `RandomDoubleUniform()` (x then z, × 16 m from the cell corner) per created leader on `0x00B22B80`. Pipe: `RandomSint32RangeUniform(lo, hi)` µs per leak effect (re)creation on `0x00B22B60`. Earthquake: `RandomDoubleRangeUniform` on `0x00B22AE8` for the fault line. disasters.md §5.5–§5.10, §6.4, §7, §8, §13.

<!-- from: From the disaster layer core (2026-10-15) -->
- ~~**DS3:** (Q8) is the fire ID drawn once per IgniteCell, also when the cell does not ignite?~~ **ANSWERED 2026-10-15 (re-answers14):** Once per `IgniteCell` call that passes the probability, **even when no occupant then ignites**, and only when `fireID` is 0; a call refused by the probability draws nothing, and all occupants of the cell share that ID (`Ignite` never draws then). A core that followed the old text (the draw inside `Ignite`, per occupant) **DIFFERS**. disasters.md §5.5.

<!-- from: From the disaster layer core (2026-10-15) -->
- ~~**DS4:** (Q9) are timer seconds truncated or rounded?~~ **ANSWERED 2026-10-15 (re-answers14):** Truncated where the integer `GetElapsedTime` is used (`0x00890198`: float multiply, then `_ftol`): the fire's milliseconds and the riot's once-per-second gate (units 5). The damage assessment uses the float overload (`0x00890377`), so its seconds are fractional, not truncated. disasters.md §2.5, §5.8, §6.5.

<!-- from: From the disaster layer core (2026-10-15) -->
- ~~**DS5:** (Q10) where does GoToNextDisaster's cursor start?~~ **ANSWERED 2026-10-15 (re-answers14):** The cursor (`+0xC0`) is set to the newest entry's index on **every** `RegisterDisasterLocation` (`0x005783D0`). `GetNextDisasterLocation` (`0x00577770`) clamps it to `count − 1`, uses that entry, then decrements, wrapping below 0 to `count − 1`; unregistering does not touch it. So the first call after a registration goes to the newest entry and later calls walk backwards. The old text (advance, then use) **DIFFERS**. disasters.md §2.7, test 11.

<!-- from: From the disaster layer core (2026-10-15) -->
- ~~**DS6:** (Q11) the effectiveness with no timers, or with an overall total of zero. [0]~~ **ANSWERED 2026-10-15 (re-answers14):** **DIFFERS** in the formula; the zero cases MATCH. `EndDispatchTimers` (`0x00577AF0`) divides the summed (fractional) seconds by `float(GetTotalDispatchCount()) × overall seconds`, the count being fire protection slot 25 or police slot 26 (the number of dispatch units), **not** the number of timers; the result is 0 when that count is 0, and an empty map gives 0. Nothing is passed to the simulators; the values are only stored at `+0xAC`/`+0xB0`. disasters.md §2.5, test 10 (rewritten).

<!-- from: From the disaster layer core (2026-10-15) -->
- ~~**DS7:** (Q12) what is the dispatch key's data2, and does any message create an entry?~~ **ANSWERED 2026-10-15 (re-answers14):** data1 is the dispatch type (1, 5 → fire map; 0, 6 → police map; others ignored) and data2 is the key, the **dispatch ID** (a dispatch record, not a station). Any of the four messages with a mapped type creates the entry when absent (a new units-5 timer, not started); `0xE94671EC` starts it if stopped (keeping accumulated time) and the other three stop it if running. disasters.md §2.5.

<!-- from: From the disaster layer core (2026-10-15) -->
- ~~**DS8:** (Q13) instance Shutdown: does the instance stay in its manager's list, with no DisasterHasEnded sent? [yes,
  it stays and nothing is sent]~~ **ANSWERED 2026-10-15 (re-answers14):** **DIFFERS.** The fire (`0x0057A540`), riot (`0x0057D420`), pipe burst (`0x0057C460`), toxic spill (`0x0057EB80`) and volcano (`0x0057F920`) remove themselves from their manager in `Shutdown`; the earthquake is removed by its state 4 just before. No fire or riot `Shutdown` calls `DisasterHasEnded`, but the fire's `KillAllFires` posts `0x49E3DB1D` data2 0 per fire and both unregister their locations; the toxic spill calls `DisasterHasEnded` and the volcano calls it through `EndVolcanoLava` if it had not ended yet. disasters.md §3.1 (table).

<!-- from: From the disaster layer core (2026-10-15) -->
- ~~**DS9:** (Q14) D3-D5 property arrays of other lengths. [the D4 index is clamped]~~ **ANSWERED 2026-10-15 (re-answers14):** MATCHES on every defined input; clamping is accepted as a *[choice]* for short arrays, which are undefined in the original (it reads past the array). The cumulation always reads D3[0..4] and the stage lookup (`0x005796B0`) returns 0..5; D4 is indexed by that stage, D5 by the fire's current stage; extra entries are ignored. Note a related **DIFFERS** in the old §5.1: the defaults apply **only when the whole exemplar is missing**; with the exemplar present, a missing array stays empty and D6/zoom are left unset. disasters.md §5.1.

<!-- from: From the disaster layer core (2026-10-15) -->
- ~~**DS10:** (Q15) which fires count for ExtinguishCell and GetFireCountInCellRect?~~ **ANSWERED 2026-10-15 (re-answers14):** `ExtinguishCell` (`0x0057B350`) takes the occupants of the single cell (x, z) through the occupant manager (slot 40) with an "active fire" filter: fire occupants with a non-null target, **including** ones already extinguishing. `GetFireCountInCellRect` (`0x00579A60`) counts every fire in the list, extinguishing or not, whose fire occupant's bounding cells overlap the rect (inclusive). `GetFireCount` counts the list. disasters.md §5.7.

<!-- from: From the disaster layer core (2026-10-15) -->
- ~~**DS11:** (Q16) riot defaults when the exemplar is missing (max riots, max leaders, zoom). [no leaders-spawned
  default]~~ **ANSWERED 2026-10-15 (re-answers14):** **DIFFERS.** The instance sets defaults first and lets present properties override them (`0x0057E3D0`): 6 stages, 30 s per stage, `0xEA400622` = 3, **leaders spawned 0, 1, 0, 1, 0, 0**. The police curve defaults to (64, 0) (65, 1) (255, 3) when the exemplar exists but the curve is empty, and stays empty when the exemplar is missing. The zoom byte has no default (uninitialised; use 0). The manager statics default to max riots 3 and **max leaders 5** (shipped 10). disasters.md §6.1.

<!-- from: From the disaster layer core (2026-10-15) -->
- ~~**DS12:** (Q17) what does message `0xEA400622` mean?~~ **ANSWERED 2026-10-15 (re-answers14):** It is not a message: `0xEA400622` is a riot tuning property, stored at instance `+0x54` (default 3) and **never read**. A port may drop it. disasters.md §6.1.

<!-- from: From the disaster layer core (2026-10-15) -->
- ~~**DS13:** (Q18) the exact spiral order of the cell search.~~ **ANSWERED 2026-10-15 (re-answers14):** `FindNearestRoadTile` (`0x0057D4C0`) uses the city's `SpiralIterator` in its clockwise mode, exactly as plumbing.md §7.2: centre first, ring r from (x + r, z − r + 1) running +z first, rings 0 to 7, inside the whole-city box; the first cell whose traffic-network tile occupant has network type 0, 3 or 6 wins, and (x, z) is rewritten to it. disasters.md §6.4, test 13.

<!-- from: From the disaster layer core (2026-10-15) -->
- ~~**DS14:** (Q19) does the riot leave its manager right after phase 3?~~ **ANSWERED 2026-10-15 (re-answers14):** No. Phase 1 → 3 (`0x0057D390`) only unregisters the locations and calls `DisasterHasEnded`; the instance stays in the list, keeps ticking, and leaves only at the riot manager's `Shutdown`. So the removal locks are released only then (network-manager.md §10 note added; disasters.md Q4 corrected). Also **DIFFERS** from the old §6.2: `AttemptDisasterStart` (`0x0057EA10`) **always returns null**. Ended riots do not count as active, so they never block new ones. disasters.md §6.2, §6.7.

<!-- from: From the disaster layer core (2026-10-15) -->
- ~~**DS15:** (Q20) the tuning defaults of the short forms (earthquake, pipe burst, volcano, effect).~~ **ANSWERED 2026-10-15 (re-answers14):** Earthquake: read once per process, only if the exemplar exists; static defaults spacing 40, curviness 30, slope 0, speed 25, shaker 10000 µs, repulsor spacing 100, curves empty; the slope (× π/180) and speed (× 1e-6) conversions run whenever the exemplar exists, so a missing speed means 25 m/s, and with no exemplar the speed stays 25 m/µs *[quirk]*. Pipe burst: 5 s and 10 s, zoom 0. Volcano (read at every `Init`): T 20 s, scale 1, radius 100, curves empty, lava disabled. Effect disasters: no tuning. disasters.md §7, §8, §10, §11.

<!-- from: From the disaster layer core (2026-10-15) -->
- ~~**DS16:** (Q21) the earthquake's fault line geometry.~~ **ANSWERED 2026-10-15 (re-answers14):** `ComputeFaultLine` (`0x00576640`), all on the earthquake generator `0x00B22AE8`: a base angle in [0, π] drawn once per process; angle = base + uniform(±slope); L = SizeX/3; two four-point polylines from the epicentre in opposite directions, each step L along ±dir plus a perpendicular uniform(±curviness) offset (first step one shared draw, +d and −d; second and third steps one draw each, A first). disasters.md §7.

<!-- from: From the disaster layer core (2026-10-15) -->
- ~~**DS17:** (Q22) the earthquake's transition from rumble to deformation.~~ **ANSWERED 2026-10-15 (re-answers14):** The manager's `AttemptDisasterStart` (`0x00576D80`) sets state 2 and calls `StartDeformation` in the same call as `Init`, so the rumble state never ticks; nothing else drives state 1 in this build. The intensity keeps its constructor value 0.1 (strength 0.19, width 0.55) and "QuakerShaker" never plays. Deformation, settle (3 s) and end as in disasters.md §7, which also corrects the redraw to terrain slot 78 then slot 68 per side.

<!-- from: From the disaster layer core (2026-10-15) -->
- ~~**DS18:** (Q23) the sinkhole brush parameters.~~ **ANSWERED 2026-10-15 (re-answers14):** `0x0057C1A0`: terrain brush `0x14` (terrain slot 57) with start = end = (x × w, z × w), w = terrain slot 6, strength 1.0, width 1.0, and fields `+0x10` = 0, `+0x14` (byte) = 1, `+0x20` = 3, `+0x34` = 100.0 whose meaning is not established. It is dug only when the cell's pipe was found, and even without a demolition utility. disasters.md §8. **Confirmed 2026-10-15 (re-specs16):** terrain.md §9.5 (re-specs14a) gives the same values, checked again in the bytes `0x0057C1A0`–`0x0057C20D`: `ApplyBrush(0x14)` with from = to = (x·s, z·s), `s` from slot 6 of the object at `+0x18` (the 16 m cell size); level 0, redisplay on, strength 1, radius 1, `data4 = 3`, target 100. The brush itself (PipeSinkHole) is in terrain.md §9.1.

<!-- from: From the disaster layer core (2026-10-15) -->
- ~~**DS19:** (Q24) when is the pipe-burst occupant inserted and removed?~~ **ANSWERED 2026-10-15 (re-answers14):** The pipe burst inserts **no occupant** of its own. Leaks are records keyed by cell (`StartLeakAtPipe` `0x0057C7C0`), removed when they burst, when a pipe occupant on their cell is inserted (non-preview) or removed (`0x99EF1142`/`0x99EF1143`), or at `Shutdown`. The only occupant handled is the existing pipe: the layer location holds it from `Init`, and `BurstPipe` demolishes it. disasters.md §8.

<!-- from: From the disaster layer core (2026-10-15) -->
- ~~**DS20:** (Q25) the volcano: the timing of states 3 to 5, its end request, and state 7.~~ **ANSWERED 2026-10-15 (re-answers14):** States 1 → 3 → 5 all happen inside the start call. State 3 starts the "volcano_control" effect, whose completion posts `0xCA493C1D` (the end request) for this instance. State 5 flows for T seconds (clock > T, strictly), then `EndVolcanoLava` (which calls `DisasterHasEnded` and unregisters the locations) and state 3, or state 8 if the end was already requested. State 7 is the end request: it marks it, keeps a flowing lava going, else goes to 8. State 8 ends the lava, removes the instance from the manager and shuts it down. `GetPhase` maps states 1 and 5–8 to phase 2. disasters.md §3.2, §10, test 14. **Added 2026-10-15 (re-specs16):** effects.md §10.1. The end request `0xCA493C1D` (data1 = the instance) is the end message of the instance's `volcano_control` effect, posted only when the effect **finishes** in the city view's draw pass (effects.md §7). The finish comes once the chosen creator's brush stroke (2–4 dilated seconds) and its particles and decals are inactive. State 7 sets the end-requested flag (`+0x3E`), returns to state 5 if the lava is enabled and running (the lava then ends in state 8), otherwise goes straight to state 8. Still open: the exact finish time, which depends on render components (effects.md EF1), and, *[inferred]*, that the volcano never ends in a session that does not draw the city (EF2).

<!-- from: From the disaster layer core (2026-10-15) -->
- ~~**DS21:** is the toast list processed only when Update runs? [yes]~~ **ANSWERED 2026-10-15 (re-answers14):** MATCHES, with a consequence: `ProcessToastList` (at most 5 per call, from the front) runs only inside `Update`, which runs only while an episode keeps the instance on the animation tick (plus once at `Init`). Toasts queued while no fire burns wait for the next episode. disasters.md §5.7.

<!-- from: From the disaster layer core (2026-10-15) -->
- ~~**DS22:** the contents of the special-building record.~~ **ANSWERED 2026-10-15 (re-answers14):** {name, cost}: the name is the BDS `GetBuildingName` (BDS slot 38) of the building's type (building occupant slot 12), and the cost is the occupant's int64 `0x8A1C3E72` (0 if absent). A record is pushed when the BDS profile flags (`GetBuildingProfile`, slot 39, `+0x1C`) have any bit of `0x1800400`. disasters.md §2.5.

<!-- from: From the disaster layer core (2026-10-15) -->
- ~~**DS23:** the network default cost. [taken from the host]~~ **ANSWERED 2026-10-15 (re-answers14):** **DIFFERS.** It is one int64 constant, **10** (`0x00B0A1C0`, read only at `0x00578D41`), used for any network occupant whose `0x8A1C3E72` lookup failed. It is not per network type and comes from no host data. disasters.md §2.5.

<!-- from: From the disaster layer core (2026-10-15) -->
- ~~**DS24:** the order in which the riot releases its removal locks. [insertion order, a choice the spec does not
  make]~~ **ANSWERED 2026-10-15 (re-answers14):** MATCHES in effect. `ClearRiotNetworkOccupants` walks the manager's `std::set` keyed by occupant pointer (ascending address), calling `SetRemovalFlags(2)` then Release on each. The order is not observable, so insertion order conforms. disasters.md §17.

<!-- from: From the disaster layer glue (2026-10-16) -->
- ~~**DS25:** (Q26) fire ID mapping: IgniteCell passes a drawn nonzero ID to Ignite, although §5.5 says 0. [the shadow maps
  the original's IDs to the mirror's by a heuristic]~~ **ANSWERED 2026-10-16 (re-answers23):** MATCHES disasters.md
  §5.5, which never said 0: `IgniteCell` (`0x0057B170`) draws the ID itself (`0x0057B201`) only when its argument is 0
  and the probability passed, then calls `Ignite(o, teleport, id, !teleport)` through slot 6 with that non-zero ID, so
  those `Ignite`s draw nothing. `Ignite` draws only when called with 0 (through `IgniteOccupant`). For the shadow:
  copy the original fire generator's state (`lcg` at identity `+0x18`, `mul` at `+0x1C`) into the mirror before each
  forwarded call and every ID matches, with no heuristic. disasters.md §5.5, §13.

<!-- from: From the disaster layer glue (2026-10-16) -->
- ~~**DS26:** (Q27) the fire occupant class `0x69D25C5F`: its interface and visuals are not specified. [not wired in live
  mode]~~ **ANSWERED 2026-10-16 (re-answers23):** Specified in disasters.md §5.11 and the ABI rows. Object `0x6C`
  bytes (constructor `0x004711C0`, GZCLSID `0x29D25F1D`); interface `0x69D25C60` at vtable `0x00A88460`, slots 0-14
  (AsOccupant, SetTarget, GetTarget, Get/SetStage, GetMaxStage, Get/SetCounter, GetHosed, AddHosed, Get/SetFireID);
  occupant sub-object at `+4` (vtable `0x00A883D8`) overriding Init (group `0x5005`, flames), Shutdown, GetPosition
  and GetBoundingBox (forwarded to the target). **DIFFERS** from the old §5.11 in one point: `SetTarget`'s occupant
  slot 12 call is `SetRemovalFlags(2)`, so a burning occupant cannot be bulldozed; `Shutdown` clears `0x69DBEBE1` and
  undoes it. Rows: `build-runs/re-answers23-abi-rows.tsv`.

<!-- from: From the disaster layer glue (2026-10-16) -->
- ~~**DS27:** (Q28) how riot leader automata are created is not specified. [not wired in live mode]~~ **ANSWERED
  2026-10-16 (re-answers23):** MATCHES automata.md §7.2/§8.1 and disasters.md §6.4; §6.4 now has the whole sequence of
  `AddRiotLeader` (`0x0057DD00`): corner + 16·(u1, u2) at terrain altitude, pedestrian manager slot 15
  `CreateAutomaton(0x4107, &pos, 2, &{0,0,0}, null, 0xFF, null)`, AddRef, the leader record (last network occupant =
  the found road tile's occupant), `SetState(8, 0, −1.0f, 1, 0, 0, 0)`, slot 9 (1), the location registration, the
  network mark, then `UpdatePhase`. The visuals are the "Riot<n>" automaton effects (`0x0057DC20`) and the
  "riot_leader_%d" controller message.

<!-- from: From the disaster layer glue (2026-10-16) -->
- ~~**DS29:** (Q30) the data1 token of the posts.~~ **ANSWERED 2026-10-16 (re-answers23):** data1 of `0x49E3DB1D` is
  the fire occupant's `cISC4Occupant` pointer (fire occupant + 4), data2 1 or 0. Nothing in the executable subscribes
  to it (the only other reference is the debug name table `0x00B080C0`), so a mirror token is acceptable as a choice;
  live mode should keep the pointer for plugins. All fire posts are `MessagePost(msg, false)` with the fire instance
  as IGZUnknown. Minor correction: `0xCA1B1E64` carries the instance as IGZUnknown (slot `+0x84`), not void1.
  disasters.md §2.4, §12.

<!-- from: From the disaster layer glue (2026-10-16) -->
- ~~**DS30:** (Q31) the follow-disasters preference, the BDS type flags, and the dispatch effectiveness slots 25 and
  26.~~ **ANSWERED 2026-10-16 (re-answers23):** Preference: byte `+0xF08` of `cISC4App` (`0x00B43C94`) slot 38's
  preferences object, cached in `0x00B43CD8`, default 1 (`0x00449070`), set by the game-preferences checkbox
  `0x4A7F19F2`. BDS flags: building slot 11 (`0x00459250`) is BDS (`0x00B43D68`) slot 39 `GetBuildingProfile(type at
  +0x38)`, flags at profile `+0x1C` (an empty static profile without a BDS). Slots 25 (fire) and 26 (police) are both
  `GetTotalDispatchCount`, unsigned: MATCHES §2.5; if either simulator is missing neither value is written
  (`0x00577C0F`). disasters.md §2.5, §2.7.

<!-- from: From the disaster layer glue (2026-10-16) -->
- ~~**DS31:** (Q32) the road cell test.~~ **ANSWERED 2026-10-16 (re-answers23):** `FindNearestRoadTile` (`0x0057D4C0`)
  accepts a cell whose tile occupant passes network occupant slot 22 `IsNetworkType(t)` (`0x00619520`) for t = 0, 3 or
  6; slot 22 maps t to one bit (table `0x00AA7CE8`) and tests `(flags +0x13C & mask) == mask` (slot 8). So road,
  street and avenue only. The occupant is not null-checked. disasters.md §6.4 step 1, test 21.

<!-- from: From the disaster layer glue (2026-10-16) -->
- ~~**DS32:** (Q33) SafeGetTileInfoAtCell. [a fallback; it matched 4/4 at the start]~~ **ANSWERED 2026-10-16
  (re-answers23):** Traffic map slot 14 (`0x004AE0C0`): null unless `0 ≤ x < width (+0x1C)` and `0 ≤ z < height
  (+0x20)`, else slot 13 (`0x004B3AC0`), the table at `+0x18` indexed `(z << shift(+0x24)) + x`, which may itself be
  null; the tile's first field is the network occupant. MATCHES the glue's fallback. disasters.md §6.4 step 1.

<!-- from: From the disaster layer glue (2026-10-16) -->
- ~~**DS33:** (Q34) the extinguish delay Random(4000) and a covered fire's counter draw cannot be followed: the mirror's
  draw is 0.~~ **ANSWERED 2026-10-16 (re-answers23):** Not a spec gap: both are ordinary draws on the fire generator
  (`ExtinguishFire` `0x00579250`: `RandomUint32Uniform(4000)` once; the step's `RandomUint32Uniform(k + 1)`). Copy the
  original generator's state into the mirror before each forwarded call (as in DS25), or read the outcomes back: the
  counter through fire occupant slot 9 and the extinguish time at fire occupant `+0x50`. disasters.md §13.

<!-- from: From the disaster layer glue (2026-10-16) -->
- ~~**DS34:** (Q35) does IgniteCellRadius take its radii as float or int?~~ **ANSWERED 2026-10-16 (re-answers23):**
  MATCHES §5.6 (r × cell width). Unsigned 32-bit integers in cells: each is loaded as unsigned and multiplied by the
  `float` cell width (city slot 91) into `float` metres; the distance `sqrt(dx² + dz²)` (`0x005D76A0`) is unrounded
  x87. The ID is drawn first; the box clamps are signed below and unsigned above; the toast branch also requires
  flammability > 0; the call returns whether anything happened. The effect action (`0x005BCBF0`) calls it with r0 = 0,
  then `SetCause(4)`. These details are now in disasters.md §5.6, test 20.

<!-- from: From the disaster layer glue (2026-10-16) -->
- ~~**DS35:** (Q37) occupant location entries read (0,0), or the cell at registration when focus = 1, and never follow
  the occupant, although §2.7 says they resolve while the occupant is in the manager. Do they resolve only on focus or
  GoToNextDisaster? [tagged in the log: fire location 43 lines, riot location 2339 in dis-final-11]~~ **ANSWERED
  2026-10-16 (re-answers23):** **DIFFERS** from the old §2.7 wording, MATCHES the glue's observation. An entry's x and
  z are written only when it is resolved (`ResolveDisasterLocation` `0x00577540`: the occupant's cell while the
  occupant manager holds it, else the instance's `GetLocation`), and only slot 12 (`0x0057764E`) and
  `GetNextDisasterLocation` (`0x005777C3`) resolve; slot 17 (`0x00577920`) reads the stored values. So an entry reads
  (0, 0) until `DisasterHasBegun`, a focused registration with the preference set, or the view's next-disaster command
  resolves it, then keeps that cell. Also: `Ignite` passes focus only for the first ID of an episode (`0x0057A9BA`).
  disasters.md §2.7, §5.4, test 15.

<!-- from: From the disaster layer glue (2026-10-16) -->
- ~~**DS36:** (Q38) the original calls LockNetworkOccupant whenever a riot leader enters a new cell, not only at
  AddRiotLeader. When exactly? [the core locks at AddRiotLeader; 316 riot lines tagged]~~ **ANSWERED 2026-10-16
  (re-answers23):** MATCHES disasters.md §6.5 step 6; the core **DIFFERS**. `MarkRiotNetworkOccupants` (`0x0057D300`)
  has three callers: `AddRiotLeader` (`0x0057DF18`); the update step (`0x0057E382`), once per step for each surviving
  leader whose cell floor(pos/16) has a non-null tile occupant different from the leader's last one (the last one
  starts as the road tile's occupant found at creation); and `Init` (`0x0057E85C`) for leaders already in the list
  (none at a fresh start). Cells crossed between steps are not sampled. disasters.md §6.3, §6.4, §6.5, test 19.

<!-- from: From the disaster layer glue (2026-10-16) -->
- ~~**DS37:** (Q39) the original's riot counter is one second ahead: its stage-1 leader spawns at 29 s, while the core's
  *[choice]* puts the first update 1 s after Init. What is the riot's second offset?~~ **ANSWERED 2026-10-16
  (re-answers23):** MATCHES disasters.md §6.5; the core's choice **DIFFERS**. `next` is 0 from the constructor
  (`0x0057DB29`) and `Init` leaves it; `Update` (`0x0057DFE0`) steps when integer elapsed seconds ≥ next and sets next
  := elapsed + 1, so step 1 runs at elapsed 0 and stage 1 spawns at step 30 (29 s). Leaders spawned in a step are
  processed in the same pass unless the spawner was the last node (`0x0057E026`). disasters.md §6.5.

<!-- from: From the disaster layer glue (2026-10-16) -->
- ~~**DS38:** (Q40) the original refuses some Ignites the spec admits: an edge building at (0,36) with flammability 75,
  coverage 0, not paused; interior buildings; spread targets with coverage 0-4 during the emergency pause. Which
  Ignites does it refuse? [the mirror puts its fire out at once]~~ **ANSWERED 2026-10-16 (re-answers23):** Partly
  answered. `Ignite` (`0x0057A7D0`) refuses at exactly five exits: manual pause without emergency pause and without
  force (`0x0057A80D`); wrong type (`0x0057A907`); flammability `0x29244DB5` = 0; not forced and
  round-half-even(D9(average coverage over the city-clamped bounding cell rectangle)) > 0; target's `0x69DBEBE1` true
  and not forced. No other refusal exists. Which exit refused the logged targets cannot be fixed statically: the
  shadow should log those original-side inputs at each refusal. For spread targets the likeliest is the burning test.
  disasters.md §5.4 "The refusals, exactly".

<!-- from: From the disaster layer glue (2026-10-16) -->
- ~~**DS39:** (Q41) layer slot 18 (GetDisasterCost) returns a pointer-like value (about 332 million) against ours
  (140-1630), although §2.6 says totalCost. Is the slot or the return type wrong? [cost not compared]~~ **ANSWERED
  2026-10-16 (re-answers23):** The slot is right; the return type is wrong (**DIFFERS** from §2.6). `GetDisasterCost`
  (`0x00578900`) returns a pointer to the layer's statistics record at instance `+0x50` (interface + `0x48`): building
  count, total cost, four kind pairs, special records, fire and police effectiveness. The logged 332771428 =
  `0x13D5B064` = identity `0x13D5B014` + `0x50`. Reader: the safety advisor (`0x00416ED1`). The interfaces.tsv row
  must change (`build-runs/re-answers23-abi-rows.tsv`). disasters.md §2.6, test 16.

<!-- from: From the disaster layer glue (2026-10-16) -->
- ~~**DS40:** (Q42) does UpdateAllFires step a fire lit during the same pass? The original sometimes does (spread
  `150215A4` burned down one step earlier than in the mirror); the mirror never does, and §5.4 / §5.8 do not say which
  container the fire list is or how it is iterated. [never]~~ **ANSWERED 2026-10-16 (re-answers23):** Sometimes, as
  the glue saw; the mirror's "never" **DIFFERS**. The fire list is a `std::list` at instance `+0xE0`, appended by
  `Ignite`; `UpdateAllFires` (`0x0057BEA0`) takes the next node before each step, so a fire appended during the pass
  is stepped in it unless the spreading fire was the last node. A null target triggers `KillAllFires` after the
  earlier fires were stepped. Also: the extinguish-time test is strict (`0x0057BBE0`), and `KillFire` (`0x0057B640`)
  re-registers or erases the ID only when the unregister removed an entry. disasters.md §5.8, §5.10, tests 17-18.

<!-- from: From the enable=/spec-contradiction wave (filed by the coordinator from the implementer lanes' reports, 2026-10-16; answered by re-answers31) -->
- ~~**DS38 leftover**: why does the original refuse `Ignite` of a **spread** target (logged
  `rect=56,86..56,87 coverage=1 flammability=40 burning=0 paused=1 emergency=1`, `exits=none-of-1,2,3,5`)?~~
  **ANSWERED 2026-10-16 (re-answers31): exit 4, the average-coverage test.** The log rules exits 1, 2, 3 and 5
  out on its own; `coverage=1` is exactly exit 4's input. A spread target is a cell the fire has already
  covered, so `round(D9(average coverage)) > 0` refuses it unless forced. The earlier guess that it was the
  burning test (exit 5) is ruled out by `burning=0`. Written into [disasters.md](../../disasters.md) §5.4.

<!-- from: glue-disasters4 (2026-10-21) -->
- **DS42:** §1 says the city creates the layer in `CreateCityComponents` (`0x0045B01C`) and gives its class
  `0x61F6ABF5`, but not how. Is it created through GZCOM (`GetClassObject` / `CreateInstance(0x61F6ABF5, iid)`), so
  that a class factory a plugin registers for `0x61F6ABF5` replaces it (as flammability.md §1 states for its class),
  or by a direct constructor? If through GZCOM: which IID is requested, how many references the city holds at
  `+0x1C4`, and what the city calls at teardown (`Shutdown` slot 4, `Release`), in which order relative to the other
  components (the managers' `Shutdown` releases instances that post messages and touch other subsystems).
  [blocked: an `enable = disasters` layer replacement needs this boundary; nothing implemented]
  **ANSWERED 2026-10-21 (re-disasters-boundary4):** MATCHES (GZCOM) — `CreateCityComponents` calls `GetClassObject(0x61F6ABF5, 0x29B02A2C, &city[+0x1C4])` only if the slot is null (`0x0045B004`); the factory `0x00578B70` is registered by the COM director, so a plugin factory replaces it; the city holds exactly one reference; city `Shutdown` calls layer `Shutdown` then `Release` after the demolition utility and before the civic building simulator; not saved. Written into [disasters.md](../../disasters.md) §2.8.
- **DS43:** §2.3 names a cached layer pointer `0x00B43D3C`. Who writes it (the layer's constructor or `Init`, the
  city, a getter), when, and who clears it? Which code outside the layer's own functions reads it, and which layer
  slots does it call through it? Likewise the layer pointers kept by the effect and toxic instances: taken from city
  slot 72 or handed over by the layer, and AddRef'd or not? A replacement layer cannot know whether native readers of
  the global would see null or a stale pointer. [blocked with DS42]
  **ANSWERED 2026-10-21 (re-disasters-boundary4):** DIFFERS (from both guesses: neither the layer nor a getter) — `SL::OnCityInit` writes the global from city slot 72 without AddRef right after `CreateCityComponents`, before the layer's `Init`; `SL::OnCityShutdown` clears it first thing in the city's `Shutdown`; 8 readers call only slots 7, 10, 16, 17 (one null check). The effect (`+0x20`) and toxic (`+0x14`) instances take city slot 72 and AddRef in `Init`, and call 9, 14 then Release in `Shutdown`. Written into [disasters.md](../../disasters.md) §2.8.
- **DS44:** Is the `cISC4DisasterLayer` interface (§15) the whole boundary to the native managers, instances and
  other consumers? That is: does any code outside the layer `QueryInterface` the layer for `0x452294AA` (its message
  target at `+0xC`) or another IID, or cast the interface to the concrete class and read fields directly (other than
  through the `GetDisasterCost` record pointer of §2.6)? [blocked with DS42]
  **ANSWERED 2026-10-21 (re-disasters-boundary4):** MATCHES — no outside code QIs the layer or reads its fields except the slot 18 record (safety advisor); but slot 7's result is cast to the concrete native manager (−8) by six instance `Init`s and plumbing, so slot 7 must return the native managers' `0x021E9CBF` pointers. Written into [disasters.md](../../disasters.md) §2.8.

<!-- from: glue-disasters-impl4 (2026-10-21) -->
- **DS46:** §2.6 gives record `+0x50` as "vector of special-building records {name, cost}" but not the vector's
  layout (begin/end/capacity pointers?) nor the element's (the name's type, size, padding before the int64 cost).
  Only the safety advisor reads the record and only at `+0x00/+0x08/+0x5C/+0x60` (§2.8 DS44). Does anything read
  `+0x50`, and what is the element layout? [the replacement keeps the 12 bytes as an empty vector (three null
  pointers) and keeps the special records' count and cost in its own state]
  **ANSWERED 2026-10-21 (re-disasters46-4):** MATCHES — nothing outside the layer reads record `+0x50` (advisor reads only +0x00/+0x08/+0x5C/+0x60; layer not saved). It is a 12-byte {begin, finish, end_of_storage} vector, allocator B, of 0x20-byte elements {cRZString 0x14 bytes, 4 unwritten padding, int64 cost at +0x18}; ctor 0x00578A10 leaves three nulls, Reset keeps the buffer. Three null pointers forever is conforming (§2.6.1).

<!-- from: replace-lotocc28 (2026-10-26) -->
- ~~**DS-lotocc28-1 — independent fire-occupant factory boundary:** section 5.11 calls `0x69D25C5F` the class
  and occupant type, but gives `0x29D25F1D` as GetGZCLSID; section 5.4 does not identify a registry lookup for
  native ignition. Which ID must a replacement factory register, and does native ignition use it? Section 13
  also specifies direct access to extinguish time at fire-object +0x50, with no interface slot. Please specify
  the supported lifetime/layout or owner-dependency boundary for an independent `fireocc` key while the native
  fire manager/instance remains active. No guessed factory ID, native layout or patch is implemented.~~
  **ANSWERED 2026-10-26 (re-lotocc29):** DIFFERS — §§5.4/5.11/5.11.1 identify class/factory key 0x29D25F1D, occupant type 0x69D25C5F and IID 0x69D25C60 separately. Native ignition directly allocates/constructs the fire and the native owner uses +4 and +0x50/direct Extinguish. An independent factory does not replace that owner; replace the owning manager/instance paths together or prove a complete installation/layout/lifetime/restoration bridge. The explicit-factory/native-ignition/extinguish/kill worked case specifies the distinction.

## DSEX33-DS01 — period-start instance identity and native safety consumer

Lane `disasters-exact33`, 2026-10-28. Sections 2.4 and 3.2 specify the period-start
message's counted instance and generic slots, while advisors.md 7.6 obtains the
fire cause through the instance. Please specify the exact interface pointer
identity stored in the message, any QI/adjustment made by the native safety
advisor, and the supported call contract by which it reads the cause. Does this
consumer require an additional instance IID or a concrete sub-object layout?

Evidence: disasters-only `paradis-dreyse`, qcheck-15.log, result
`C:/Users/tlori/FreeCity4/build-runs/golden-runs/compare-20261028-041512-83113dc4/result`.
The run is clean/ref_warnings=0 but not exact. Class 6A26D049 has 22 reference
subjects versus 21 candidate subjects. Payload-multiset differences are reference
locations (167,6) and (9,8), both data words zero, versus candidate (67,218),
data1=0 and data2=22937454 hex; all three have null object references. These
subjects have not been attributed to a particular event. Copying locations into
the public layer before forwarding core posts leaves this difference unchanged.
The adapter currently stores its generic instance view in the counted unknown;
no guessed IID, native layout, or consumer adjustment has been added.

### DSEX33-DS01 follow-up: generic +0x38 experiment (round 5)

An isolated experiment reserved a cause word at counted generic-instance
pointer +0x38 and synchronized it after every core call, including SetCause.
The adapter tests passed, but disasters-only Dreyse qcheck-19.log still has
17 different records and its first shifted ID is still 22/class 6A3848DB.
Result: `C:/Users/tlori/FreeCity4/build-runs/golden-runs/compare-20261028-044515-bef8b707/result`.
The speculative storage and its tests were removed. This does not settle the
native consumer's actual pointer origin or prove that the cause query caused
the missing advice subject. The script reached fire (165,5) and riot (9,8);
both managers reported one active instance.

### DSEX33-DS01 follow-up: original period-start fields (round 6)

Original-only `disasters-exact33-r5-shadow-dreyse-2` completed cleanly with
`ref_warnings=0`. Its period-start messages contain count/cell/type
`1/(167,6)/621CDA33` and `2/(9,8)/CA380B06` (log lines 4783-4784).
Both shadow summaries report zero mismatches. This confirms that the two
reference subject cells match original period-start messages; it does not yet
attribute the missing candidate subject to a cause query or pointer adjustment.
The replacement now logs instance QI requests/results and GetCause callers to
distinguish those possibilities without implementing an unspecified layout.

### DSEX33-DS01 follow-up: rejected advisor interface (round 7)

After merging the authorized reference pin, disasters-only Dreyse still has
17 different records, with the first object-ID shift at 22/class 6A3848DB.
Result: `C:/Users/tlori/FreeCity4/build-runs/golden-runs/compare-20261028-061122-cfe7bdf6/result`.
The run is clean with `ref_warnings=0`. Its game.log lines 2915-2947 show
the specified period-start count/cell/type for fire and riot. Lines 2973-2974
then show the native consumer at caller 00417163 requesting IID **22223BA4**
from both counted instances; both requests are rejected. There are no native
GetID/GetCause calls in this trace. The only logged GetID callers belong to
our period-post construction/logging.

No contract for this IID was found in disasters.md, advisors.md, foundations.md,
docs/abi, or the SDK headers. Please specify its identity, supported instance
types, returned pointer adjustment and slots, including the fire-cause read's
pointer origin. The trace now localizes the missing interface; the adapter
still rejects it rather than guessing a native layout or vtable.

**ANSWERED 2026-10-28 (re-blockers37b):** DIFFERS — disasters.md §3.2.1 and advisors.md §7.6 identify generic IID 22223BA4, instance pointer adjustments, counted period-start identity and native subject creation; fire cause is virtual slot 14 on IID 49B43F58, not a generic +0x38 field.
