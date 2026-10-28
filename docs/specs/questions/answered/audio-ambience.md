# audio-ambience: answered questions

**ANSWERED 2026-10-24 (re-audio-au50):** DIFFERS — AA14: audio-ambience.md §8.1.1 supplies
layer defaults, lifetime, references, uncovered slots and conformance cases; §8.3 distinguishes
direct repeated CreateFilters from the message path's explicit cleanup. Failed first Init
retains the latch/count, and singular loop removal sends no stop.

<!-- from: core-audio-close11 (2026-10-24) -->
- **AA14 - layer construction, lifetime and uncovered slots:** §8 inventories slots 0–28 but
  gives no complete constructor/QI/AddRef/Release/destructor, StaticInit/StaticShutdown, Shutdown,
  DoMessage, Enable, CanHandleOccupant, Insert/Move/RemoveOccupant, loop add/remove, freshness
  load/add/remove and score-master-volume bodies. Please specify defaults (including initial loop
  ID and process cell cache), service/reference acquisition/release, Init's first-failure/retry
  state and resource/player ownership for these routines. In §8.3 does repeated CreateFilters
  clear old lists/counters as well as bit vectors? [Implement §8.2–8.7 covered algorithms as
  explicit stages; no guessed native constructor/lifetime/score-loading or successful full Init.]

- **AA2 - section 3.4:** what does `_UpdateLotOccupantsForConditionChange` (`0x004E9DA0`) start or stop per lot
  condition, for properties `0xCA19D7CA`, `0x0A4C13CB`, `0x4A4C132E`?
  **ANSWERED 2026-10-21 (re-audioambience4):** DIFFERS (no engine rule was stated) — by new condition (data2; insert/init pass -1 = current): 0/5/6 stop 0x0A4C13CB, start 0xCA19D7CA; 2/3 stop 0xCA19D7CA, start 0x0A4C13CB; 1/4 stop both; new 0 with old (data1) 1 also starts 0x4A4C132E; all loops, building or prop occupants of the lot. audio-ambience.md 3.4.

- **AA3 - section 3.6:** confirm the stack slot copied into the alarm entry `+0x10` at `0x004E91B0` is the time
  of day `t` (the listing reuses `[ESP+0x18]` for `a[i]`).
  **ANSWERED 2026-10-21 (re-audioambience4):** MATCHES — yes: `t` is stored at 0x004E8F12 (depth 0x4C) and read back at 0x004E91AC as [ESP+0x1C] after PUSH EBX; the loop's [ESP+0x18] is a different slot (a[i]). audio-ambience.md 3.6.
- **AA1 - section 8:** the ambience layer's own behaviour (scores, loops, filters, per-layer tuning,
  `0x004E5110`-`0x004E8230`) is unspecified; only the interface the manager calls is. Needed before
  `cSC4AudioAmbienceLayer` can count as specified. (re-audioambience4: now §8.1–8.5 and §8.7 cover Init,
  tuning, filters, `CanHandle`/`HandleCell`/`HandleOccupant`, group layers, `Update` and the loops; still open:
  `_UpdateAmbienceScores` `0x004E5920` (§8.6), the ease time step `0x004F8650`, director `+0x54`/`+0x11C`.)
  **ANSWERED 2026-10-21 (re-audioambience4):** DIFFERS (no engine rule was stated) — §8.6: freshness `+0xDC = (trunc(curve1B0(s))·+0x1E8)>>10` (logical), `+0xE0 = trunc(curve1A0(s))` scaled 50%/75% by `+0x204` 0/1; per score start (MinMaxDistance trunc(d), trunc(16d); Play head-relative when `+0x34=4`), stop (fade) when not (vol≠0, pct≠0, unpaused), divisions every update, volume/percent on change. §8.7: ease values are per-ms rates; Step moves min(remaining, t·rate) along the line, scalar integer, flag `+0x38` set. §8.5 mode 6: director `+0x54` is never written, the read faults.

<!-- from: re-audio-inputs4 (2026-10-21) -->
- **AA5 - section 3.6:** what is an alarm entry's initial `last` (`+0x10`) when `_AddAlarmOccupant` creates it?
  The engine uses 0.0, so a one-shot alarm whose time already passed today fires on the first tick.
  **ANSWERED 2026-10-21 (re-audio-inputs4):** DIFFERS — §3.6: `_AddAlarmOccupant` sets `last` to the current time of day (hour + unsigned min/60) at creation, `on` 0; it refuses occupants without flags 1|4 or with flag 0x10; entry appended at the tail.
- **AA6 - sections 2.2, 4.1, 4.2:** (a) a `0x0A36F410` list with other than 5 values: how many margins are
  read, and what fills the rest? (b) `0x2A3FF8CD` missing: is the layer-id vector left as it was or emptied?
  (c) audio preferences object absent at city init: what volume reaches slot 17? (d) zoom index outside 0-4
  (or margins never read): the game has no bounds check; what does it read? The engine copies up to 5 values
  (rest 70.0), empties the vector, uses volume 0, and leaves the rectangle unchanged.
  **ANSWERED 2026-10-21 (re-audio-inputs4):** DIFFERS — §4.1/§4.2/§2.2: margins and ids are vectors; absent property leaves the vector unchanged, present gives exactly n values; the 70.0 fill writes 5 floats through the (possibly null) begin pointer without resizing; prefs global, view global and zoom index are unchecked (null or out-of-range = crash/foreign read). Replacement defaults are choices.
- **AA7 - section 5:** the `RandomIterator` over 2x2-cell blocks: are the block bounds `x0/2 ..= x1/2`
  (inclusive) and what visit order does it produce for a given seed? The engine visits inclusive bounds
  row-major (z outer).
  **ANSWERED 2026-10-21 (re-audio-inputs4):** DIFFERS — §5: prime-step RandomIterator, inclusive halved bounds, Reset(0) without rand, current starts 0 and carries over, step from the prime table (worked example: count 20 step 7); HandleCell is cell-outer, layer-inner.
- **AA8 - sections 3.1, 3.2, 3.5, 3.6:** instance ids and preconditions not stated: (a) is the
  `0xC9B93A56` sound started through `_AddOccupantSndInstance` (start + set-position + automaton link) with
  instance id occupant + id? (b) the move set-position for `0xCA19D7CA` (`0x004E8C70`): instance id occupant +
  that property's value? (c) automaton codes 9/12/13 when the occupant lacks `0x6A36AFAC`: nothing? (d) the
  on/off alarm loop's instance id (occupant + `0x2A53BE13` value?) and what happens when the re-read property
  is absent? (e) is the railway-crossing instance id occupant + `0xAA87817A` value? (f) do the automaton
  handlers skip preview occupants? The engine answers yes to (a), (b), (e), "nothing" to (c) and (d)-absent,
  occupant + id for (d), and no preview check for (f).
  **ANSWERED 2026-10-21 (re-audio-inputs4):** DIFFERS — §3/§3.2/§3.5/§3.6: (a) yes, loop 0; (b) no: both move set-positions use the bare occupant address; (c) nothing; (d) occupant + re-read value, absent = nothing; (e) no: instance is the automaton interface address, flag 1; (f) no preview check in the automaton handlers; automaton starts pass flag 1.
- **AA9 - sections 6, 7:** (a) what does the focus handler `0x004E8CF0` return (the engine returns 1)? (b)
  `0x2A2C0384` whose payload does not answer `0x65297976`: handled (1) or not (0)? The engine returns 0. (c)
  what does the release `0x004E9660` return, and does "empty the lists" include the alarm list? The engine
  returns 1 and empties layers and alarms.
  **ANSWERED 2026-10-21 (re-audio-inputs4):** DIFFERS — §6/§2.3: focus returns 1 (stage 1 or 4, Pause(!flag)); 0x2A2C0384 QIs the event itself and returns 1 when it does not answer; 0x2A5B69C0 returns 0; release returns 1 and empties only the layers, not the alarms.
<!-- from: re-audio4 (2026-10-21) -->
- **AA4 - section 3.1:** name occupant types `0xA823821E` and `0xC7CC75F1` of the init walk. (re-audioambience4: `0xA823821E` is the prop type; `0xC7CC75F1` is set by the
  `cSC4TrafficAutomaton`-derived constructor `0x005628F0` at `+0x28`; class name still open.)
  **ANSWERED 2026-10-22 (re-audio-final-gaps4):** MATCHES (vehicle) — §3.1: `0xC7CC75F1` is the `cSC4Vehicle` occupant type. Base ctor `0x005628F0` installs vtable `0x00A9D488` (QI `0x0055B660` answers vehicle IID `0x896E714A`) and writes the type at `+0x28` (`0x005629D2`); all ten derived vehicle ctors (trailer `0x00556DB0`, train car `0x0055B150`, ...) call it and none rewrites `+0x28`. Vector: the init walk visits every vehicle subclass (trailer, train car included) and no other automaton type except via `0x496E5783`.
<!-- from: core-audio-inputfix4 (2026-10-22) -->
- **AA11 - section 3.2:** does the second move set-position (`0x004E8C70`, entered when `0xCA19D7CA` reads) test the sound system `+0x20` itself, and is it reached when the holder lacks `0x6A36AFAC`? [both posts independent, each only with the sound system present]
  **ANSWERED 2026-10-22 (re-audio-final-gaps4):** MATCHES — §3.2: both failing tests at `0x004E8D95` (`0x6A36AFAC`) and `0x004E8D9C` (`+0x20`) join at `0x004E8DDE`, the `0xCA19D7CA` read; `0x004E8C70` tests `+0x20` itself (`0x004E8C76`), posts nothing and returns 1 when absent. Vectors (sound present): A only → 1 post; B only (value 0 included) → 1 post; both → 2 identical posts, A first; neither → 0; sound absent → 0 in every case.

<!-- from: core-audio-close11 (2026-10-24) -->
- ~~**AA14 - layer construction, lifetime and uncovered slots:** §8 inventories slots 0–28 but
  gives no complete constructor/QI/AddRef/Release/destructor, StaticInit/StaticShutdown, Shutdown,
  DoMessage, Enable, CanHandleOccupant, Insert/Move/RemoveOccupant, loop add/remove, freshness
  load/add/remove and score-master-volume bodies. Please specify defaults (including initial loop
  ID and process cell cache), service/reference acquisition/release, Init's first-failure/retry
  state and resource/player ownership for these routines. In §8.3 does repeated CreateFilters
  clear old lists/counters as well as bit vectors? [Implement §8.2–8.7 covered algorithms as
  explicit stages; no guessed native constructor/lifetime/score-loading or successful full Init.]~~
  **ANSWERED 2026-10-25 (re-audioclose8):** audio-ambience.md §8.1.1 answers every listed body and the defaults,
  and §8.3 the repeated-CreateFilters question (bit vectors and interface counters only); archived here. The
  remaining small members, `_RemoveLayers`, `_RemoveAllFilters`, `_StaticRemoveAllFilters` and a correction (each
  filter-list entry holds a reference) are audio-owner-closure.md §2.

<!-- from: core-audio-ambience5 (2026-10-23) -->
- ~~**AA13 - section 3.3 alarm removal:** when duplicate alarms exist or properties have changed since registration, does removal match occupant and kind, occupant and saved sound ID, or all entries for the occupant; is only the first match removed? [literal singular entry: remove the first entry matching occupant and kind, in list order]~~
  **ANSWERED 2026-10-27 (re-x-fw):** DIFFERS — audio-ambience.md section 3.3: removal matches the occupant only, taking the first alarm entry for it in list order; the saved sound id and the kind are never compared. The stop for a playing on/off entry uses the property value read at removal time. One entry is removed per call; HandleRemoveOccupant makes one call per property present (0x004EA35E, 0x004EA382), so an occupant with both properties loses its first two entries whatever their kinds. No match returns false.

<!-- from: core-audio-ambience5 (2026-10-23) -->
- ~~**AA12 - sections 5 and 8.4 BeginUpdate rectangle:** is the layer's 16-byte attention rectangle the world-space f32 rectangle or the even cell-coordinate rectangle? [literal attention rectangle: world-space WorldRect; the cell rectangle is used only for iterator seeding]~~
  **ANSWERED 2026-10-27 (re-x-fw):** DIFFERS — audio-ambience.md section 4.2 step 3: the layers receive the world-space f32 rectangle stored at +0x8C (OnTick passes that address at 0x004EA1F5), not the even cell rectangle at +0x6C, which only seeds the block iterator. The world rectangle is the listener +/- margin square intersected with the city's world bounds (helper 0x004E8380: max of minima, min of maxima, touching allowed, all zero when disjoint), a clipping step the spec did not state.
