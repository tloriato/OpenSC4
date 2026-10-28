# network-manager: answered questions (archive)
<!-- Archive for [network-manager.md](../../network-manager.md). Lanes do not read this file. Open questions are in ../network-manager.md. -->

## From the network manager and network lot manager spec (re-specs13c lane, 2026-10-15)

Open points from network-manager.md §15. The current answer is in brackets; each entry says what would settle it.

<!-- from: From the network manager and network lot manager spec (2026-10-15) -->
- ~~**NM1:** are the globally managed network occupants (bridges, tunnels, prebuilt-model pieces) also saved by the
  occupant manager, or only through component 53? [both hold refs to the same objects; the occupant manager spec
  (re-specs13b) settles it]~~ **ANSWERED 2026-10-15 (re-specs14b):** both. The occupant grid (component 4) and
  component 53 save refs to the same objects; each ref resolves to one object, so `managed` holds each piece once.
  MATCHES. network-manager.md §15.

<!-- from: From the network manager and network lot manager spec (2026-10-15) -->
- ~~**NM2:** does the occupant manager post `0x99EF1142` while a city loads? If it does, the network manager's managed
  list (read first) gets every piece twice. [no, inferred. A turbo run that saves, loads and saves again and compares
  the component 53 counts would settle it]~~ **ANSWERED 2026-10-15 (re-specs14b):** no. Loading posts no Insert. Since
  the base occupant `Read` (`0x005E8D60`) sets bit 1 of the state byte, the base `Init` (`0x005E8C90`) posts no
  Create either. The network occupant `Init`s never insert. MATCHES. network-manager.md §15.

<!-- from: From the network manager and network lot manager spec (2026-10-15) -->
- ~~**NM3:** meanings of network flags `0x40000`, `0x80000`, `0x40000000` and `0x80000000`~~ **ANSWERED 2026-10-15
  (re-specs15b):** `0x40000` bridge-step piece (MATCHES), `0x80000` tunnel portal (MATCHES), `0x40000000` elevated
  with supports (MATCHES), `0x80000000` draft (preview) occupant. network-geometry.md §10.3. History: ~~and of the terrain kinds 3,
  4 and 6 in the network manager's terrain handler. [unknown; logging `data4` of `0x278E6003` for each terraforming
  brush settles the kinds]~~ **PARTLY ANSWERED 2026-10-15 (re-specs14a):** the kinds are settled from every call
  site of `RedisplayTerrain` (terrain.md §8.2): 0 internal (sea level, flat areas, shore cliffs, texture-only),
  1 terraforming in god mode, `ChangeTerrainLevel` and the edge reconciliation, 2 terraforming in mayor mode and
  the `ApplyTerrainBrush` script command,
  3 disasters and water (earthquake, pipe-burst sinkhole, effect brushes such as the volcano, lava solidification,
  the surface water's animation, and the `tBrushParameters` default), 4 the network height solver (road and rail
  grading, `0x00644530`), 5 levelling (demolition `ModifyTerrainHeight`, lot developer, landfill), 6 the tunnel cut
  (`cSC4NetworkTool::InsertTunnelPiece`). Flag `0x40000` is the **bridge** flag *[inferred from use]*: the surface
  water's `IsNavigable` reads a deck height from a piece with that flag and requires 20 m of clearance over the
  water, and its `0x89D3E9F3` handler tests it with "no adjacent piece" (terrain.md §10.3–§10.4). Still open: `0x80000`,
  `0x40000000`, `0x80000000` (a network-tool spec settles them; `0x80000` is the piece kept by a tunnel cut,
  *[inferred]* a tunnel portal).

<!-- from: From the network manager and network lot manager spec (2026-10-15) -->
- ~~**NM4:** the exact visited and rejected set tests of the avenue flood in `UpdateAvenueStyle`. [a cell is visited
  once; styling an avenue loop through an intersection and logging the styled cells settles it]~~ **ANSWERED
  2026-10-16 (re-answers18):** **MATCHES**.
  - `0x0060EF40` keeps three `u16` hash sets (work, accepted, rejected; key `(x << 8) | z`).
  - A neighbour is skipped when it is accepted (`0x0060F228`) or rejected (`0x0060D1B0`), else inserted unique
    into the work set (`0x0060E210`). Each cell is accepted or rejected in the step that takes it, so it is examined
    once.
  - New quirks: the far bounds are inclusive (nx ≤ countX, nz ≤ countZ). The 8-bit x in the key makes a neighbour
    one past the edge of a 256-cell city alias a cell on the opposite edge.

  network-manager §8.7.

<!-- from: From the network manager and network lot manager spec (2026-10-15) -->
- ~~**NM5:** the effect-prop slots 6, 7 and 8 used for potholes. [set the effect name, start, stop; the prop occupant
  spec should confirm]~~ **ANSWERED 2026-10-15 (re-specs14b):** confirmed: `SetEffectType(char const*)`,
  `StartEffect(bool)`, `StopEffect(bool)` (vtable `0x00AA4ED8`, overloads in reverse SDK order). MATCHES.
  network-tools.md §13.2.

## From the network manager and network lot manager cores (filed by the coordinator from the `cores14c` lane's report, 2026-10-16; merged `9dad25e`)

cores14c implemented network-manager.md as a Rust core (`engine/core/src/network_manager/`), a C ABI (`fc4_netmgr`,
`fc4_netlot`, ABI 66) and conformance vectors (`nm16_*`, `nm00_*`). There is no plugin glue yet, so none of these
readings has been checked in game. The current choice is in brackets.

<!-- from: From the network manager and network lot manager cores (2026-10-16) -->
- ~~**NM6:** §4.1, Read of component 53: "any failed ref read returns false at once". Does a null reference in the
  saved list count as a failed read, or is it appended as a null entry? [a null reference is appended as a null entry
  and counts as read; a reference that does not resolve fails the read at once, and the entries appended so far stay.
  Core `manager.rs` read_record, vector `nm16_01`]~~ **ANSWERED 2026-10-16 (re-answers24):** MATCHES
  (network-manager.md §4.1). The ref helper (`0x005D96C0`) returns true with a null pointer for object ID 0
  (`0x005D96F2`–`0x005D9713`), and Read appends it (`0x0061252D`–`0x00612544`). An unresolved reference, or one that
  fails the QI to `0xA821EF94`, makes the helper return false and Read returns false at once (`0x0061252B`), with
  nothing released or removed.

<!-- from: From the network manager and network lot manager cores (2026-10-16) -->
- ~~**NM7:** §7.10, the retrofit at PostCityInit: when the list is emptied (`0x00611A20`), is each entry released? Is
  the list emptied even when no entry still asks for a retrofit? [every entry still asking goes to the host's retrofit
  callback, then the list is always emptied and every entry released once (the reference taken at slot 23)]~~
  **ANSWERED 2026-10-16 (re-answers24):** MATCHES (network-manager.md §7.10). The release is in the walk, not in
  `0x00611A20`: each entry is released once right after its `CheckForRetrofit` test (`0x00612DB3`, also on the false
  branch from `0x006128E0`), and `0x00611A20` then frees the nodes without releasing. An empty list returns at once
  (`0x0061283A`). The net effect is the bracket's.

<!-- from: From the network manager and network lot manager cores (2026-10-16) -->
- ~~**NM8:** §8.2, §8.4, §8.7: zone and wealth values outside the tables (zone outside 0..15, wealth outside 0..3, for
  example from a corrupt zone grid). [a neighbour zone outside 0..15 never wins in GetZoneTypeForNetwork;
  IsLotCompatible fails for such a zone or wealth; the road style tables give such a zone the "every other zone" rows]~~
  **ANSWERED 2026-10-16 (re-answers24):** no defined original behaviour; the choices are **confirmed** as *[choice]*
  (network-manager.md §8.2, §8.4, §8.7). None of the three has a range check. GetZoneTypeForNetwork indexes the
  priority table with the signed neighbour value (`0x0060D0E7`–`0x0060D0F9`) and reads the neighbouring read-only
  tables: most such values never win, but some do (28 reads 0, 92 reads −1, −4 reads 2); an out-of-range own-cell
  value is returned as it is. IsLotCompatible reads bit vectors whose unused bits the constructor never initialises
  (`0x00477E70` grows them bit by bit), and beyond 31 or below 0 reads heap memory. Road style reads past `T` and `V`
  (`0x0060F5AE`, `0x0060F5DA`). Only a corrupt zone grid reaches any of this (wealth comes from §8.3, which clamps).
  For `V`, the "every other zone" row is taken to be the zones 10–13 row, `1 1 1`.

<!-- from: From the network manager and network lot manager cores (2026-10-16) -->
- ~~**NM9:** §7.6, the underground refit: step 2 has no "if any" test, unlike §7.2. When dirty is not empty but holds no
  pipe (or no subway), are tool 4 (or 7) Init, UpdateUndergroundOccupantTerrainHeights and Shutdown still called with
  an empty list? [yes: both kinds always run when dirty is not empty (vector `nm16_10`)]~~ **ANSWERED 2026-10-16
  (re-answers24):** **DIFFERS** (network-manager.md §7.6). Each kind runs only when its occupant list is not empty:
  pipes at `0x00613E53`–`0x00613E59`, subways at `0x00613E9B`–`0x00613EA1`. A kind with no occupant in `dirty` gets no
  `GetNetworkTool`, `Init`, `UpdateUndergroundOccupantTerrainHeights` or `Shutdown`. Also: a failed query (occupant
  manager slot 40 returning false) skips the split and both kinds. Vector `nm16_10` needs updating.
<!-- from: verify-regress4 (2026-10-21) -->
- **NM10 - section 6 (`0xE8BDA2E7` lot state changed), section 8.5, simgrid.md section 7.4:** in game, after a lot
  state message whose grown rect is {x0, z0, x1, z1}, the original never changes the road style variation of the
  first row's inner cells (x0 < x < x1, z = z0), while the spec adds every border cell to `pending` and
  `UpdateRoadsInCell` restyles them (ours moves such cells 5 -> 6 or 4 -> 5 where the original keeps them;
  examples: runs regress-train17/19, rect 14,93..17,97 at 16,93; rect at 20..22,103). Rarely the original restyles
  such a cell later, during another message, as if its key had stayed pending. Does the BoundaryIterator skip
  (or postpone) the first row's inner cells, or does `ProcessUpdateList` stop early? The network lot of such a cell
  then differs too (§8.5: a lot is kept only while compatible with the style). [the whole border, as written;
  the netmgr shadow counts differences at such cells under NM10, not as mismatches. The code has cited "NM10" since
  an earlier lane, but it was never filed: this is the entry.]
  **ANSWERED 2026-10-21 (re-netmgr-contracts4):** DIFFERS — the lot-state walk starts at cell (0, 0), not at the rect's corner (network-manager.md §6.3): row 0 from x = 0 to x1, then (x0, z) and (x1, z) for z = 1..z1−1, then the full row z1. The first row's inner cells (x0 < x < x1, z = z0 ≥ 1) are never added; a later restyle of such a cell comes from another message's walk (§6.3, §8.5 table). `ProcessUpdateList` does not stop early.
<!-- from: verify-regress4 (2026-10-21) -->
- **NM11 - section 8.5, section 9:** in run regress-train19 the shadow saw network lot differences at cells that
  were not (yet) in any lot state message's first row: 2026-04 lot at 20,103 (config 0x4A7F8609) held by the
  original only, with no style difference that month; 2026-05 lot at 16,93 (config 0x4A7F8606) held by ours only,
  one month before the first-row message on 16,93. In regress-train17 the same cells differed only after such a
  message (both under NM10). Which messages, other than the six of §6, create or remove a network lot (for example
  a lot created through `UpdateLot` from a pending key of a neighbouring cell whose road style changed, or from the
  traffic network's own piece updates)? [only the six messages of §6.1 reach the lot core in the shadow; the
  verify-regress4 plugin now logs, for each such lot difference, the network occupant's variation, rotation,
  flip, zone, wealth and both sides' compatibility, so the next run can say which input differs]
  Run vr4-a1 (verify-regress4, turbo, 2026-03-01) repeats both cells with those inputs: 20,103 held by the
  original only, variation 6, rotation 3, usable, config compatible now, zone/wealth 3/3 on both sides; 16,93 held
  by ours only, variation 4 (the original's write), config no longer compatible. So the original ran
  `UpdateRoadsInCell` on both cells (creating one lot, removing the other) at moments when no followed message put
  their keys in ours' `pending`. 2026-02-01 shows the same pattern at 48,83, 50,81 and 51,110 (original only,
  compatible, same zone/wealth). What else adds keys to `pending`?
  **ANSWERED 2026-10-21 (re-netmgr-contracts4):** DIFFERS — no seventh message: `pending` is written only by the zoning and lot-state handlers, `UpdateRoadsInCell` is called only by `ProcessUpdateList` and usable changed, and `UpdateLot` also by the avenue flood (network-manager.md §8.5 table). The extra cells come from the §6.3 walk: every lot-state message adds columns x0 and x1 of its grown rect from row 1 down to the lot, and row 0 up to x1. So 20,103 and 16,93 are re-evaluated by any lot-state message whose grown rect has x0 or x1 = 20 (resp. 16) and z1 > 103 (resp. 93). Traffic piece updates arrive only as 0x89D3E9F3/0x99EF1143; a neighbour's style change enqueues nothing outside the avenue flood.

<!-- from: verify-netmgrfix4 (2026-10-21) -->
- **NM12 - section 8.5 step 3.3 and section 17 (the configuration draw as an external input):** the netmgr shadow
  must replay the original's draws to compare component 11 without resyncing, but the spec gives no public point at
  which that input can be captured before our step (generator `0x00B44870`, clock-seeded at static initialisation,
  advanced by the section 8.6 prop and flora draws; no interface, message or layout exposes it). (a) Is there a
  sanctioned capture point? (b) If not, is the draw officially outside exact comparison ("exact except section 17
  draws", resync specified)? [current: no capture; 4 resyncs in vnmf4-turbo2032-a, 2 in vnmf4-normal-a]
  **ANSWERED 2026-10-21 (re-netmgr-rng4):** DIFFERS — (a) yes, an in-process read of the §17.2 layout (`state[624]` at `0x00B44870`, `next` `0x00B45230`, `left` `0x00B45234`, position `k = 624 − left`; no vtable, fixed image base), sanctioned whenever no network lot manager function runs: the generator advances only at three draw sites (`0x0060D4AC`, `0x0060D512`, `0x0060DE59`), so a capture after the previous draw-performing operation is the next one's input (§17.1, §17.3). Load it as raw state, replay, and check (not copy) the post-state. (b) No: the configuration draw is compared exactly and a resync after a draw-only difference is not specified; the remaining exactness gap is the draw order inside one `ProcessUpdateList` pass (NM13). Vectors in §17.4.

<!-- from: re-netmgr-rng4 (2026-10-21) -->
- **NM13 - section 8.5 *[compat]* note and section 17.3 (draw order within one `ProcessUpdateList` pass):** exact
  replay of the §17 draws needs the original's key order when two or more cells of one pass draw. `pending` is a hash
  set at manager `+0x74` (bucket vector begin `+0x78`, end `+0x7C`, element count `+0x84`, read at
  `0x0060FA14`–`0x0060FA3E`). Needed: its initial bucket count, the growth rule (prime list, when it grows), the
  order of nodes within a bucket after insert and after a rehash, and whether the bucket count (which persists after
  the set empties) must itself be part of the §17.3 capture. [current: ascending key order *[choice]*]
  **ANSWERED 2026-10-21 (re-netmgr-rng4):** DIFFERS — the original iterates `pending` (SGI hash_set at `+0x74`) by ascending `key mod B`, latest insert first within a bucket; `B` starts at 193 (ctor asks 100, prime list `0x00A829B0`), grows on insert to the first prime not below `count + 1` with a head-pushing rehash (`0x00710DD0`), never shrinks, and is captured with the generator (§8.5 *[compat]*, §17.3). FreeCity4's ascending-key *[choice]* differs when two cells of one pass draw.
  **ANSWERED 2026-10-21 (re-netmgr-rng4, round 2):** DIFFERS — exact contract in §8.5 "Replay contract for `pending`" rules 1-6: no insert can happen during a pass (both feeding messages are posted, queued), so the set is empty between messages and `B` is the only carried state; `B` = first prime of `0x00A829B0` not below `max(100, P)`, `P` the largest `count + 1` any insert asked for since the city's manager was created (duplicates included); zoning keys go in z-outer, x-inner; capture `B` (and check count 0) before the original handles the message; after a resize, within-bucket order follows the rehash (old buckets ascending, head-pushed), not "latest first" (worked example and §16 tests 27-28).

<!-- from: replace-netlot28 (2026-10-26) -->
- ~~**NM14 - section 7.12:** What creates a construction crew (factory/CLSID or manager path), which interfaces does it expose, and what are the six arguments, return contract, ownership, subscriptions and state transitions of crew slot 4 Init? The current spec defines only registration and slot-5 shutdown; ABI rows give a 24-byte argument pop but no argument meanings. [Leave native crew creation/Init in place; retain the specified opaque-crew registration/shutdown bridge. Complete only the lot and configuration class registrations under netlotobj, with netlot providing the live manager.]~~
  **ANSWERED 2026-10-26 (re-nm28):** MATCHES — keep native creation/Init and the opaque bridge. §7.12 now specifies direct tool allocation (no CLSID), message-target-only QI, all six scalar arguments, AL=true/24-byte-pop contract, registration/service ownership, subscriptions, idempotence and reentrant shutdown, with a worked reference/countdown case.
- ~~**NM15 - sections 2.4 and 8.6:** What IID identifies cISC4NetworkLotConfiguration, and what exact container layout, comparator, allocator and ownership contract does GetNetworkLotConfigurationSet(set&) require? Are lot/configuration objects ever created through native GZCOM class IDs? [Keep unknown-only configuration QueryInterface and the inactive set bridge; register manager-created lot/configuration views under explicit FreeCity4-only IDs under netlotobj.]~~
  **ANSWERED 2026-10-26 (re-nm28):** MATCHES — configuration QI accepts only IUnknown 1; native lots/configurations are directly allocated, without a creator CLSID in these paths (§2.4). §8.6 specifies the 8-byte set, 20-byte header/nodes, unsigned pointer comparison, paired game allocator and caller-owned returned references; clearing frees nodes without Release. A three-object worked case covers pointer order, refilling and QI. Activating a native set bridge requires that exact boundary.
- ~~**NM16 - sections 3.2 and 8.5:** Which network/occupant services must remain valid while action-created native occupants shut down, and what ordering ensures that lifetime? With the 54-key house profile, `netlot28-gen-s0grow-r2` completed all 216 actions of suite.tsv's gen27-konradshohe-s0-grow, then reported one native ACCESS_VIOLATION reading null at 0x005D0F94 during occupant-manager shutdown of occupant type 0x096E763C (ref_warnings=0; shared `build-runs/game-results/netlot28-gen-s0grow-r2.log:16021`). The 53-key control with netlotobj disabled, `netlot28-gen-s0grow-control-r3`, reproduced that address after all 216 actions (log:16100, ref_warnings=0). With only netlot and netlotobj enabled, `netlot28-gen-s0grow-focused-r4` completed all 216 actions, then gave 12 released-last-reference warnings at cFC4NetworkLot::~cFC4NetworkLot:420 for native class 0xC9C05C6E (log:14106 onward) and the same native null read (log:14118). A second full-profile scenario, `netlot28-gen-s1grow-r3`, failed near its first network actions at 2026-01-30 with an access violation at a no-module address immediately after a cFC4AutomatonObject creation (log:8157); no cause is inferred. All used DLL SHA256 prefix f6b00b1e. The ordinary 54-key house run and native-owner shadow were clean through 2032, with shadow 73/73 exact. [Stop action acceptance; keep the specified balanced lot-view AddRef/Release, and do not guess native creation/shutdown contracts or alter unrelated subsystems.]~~
  **ANSWERED 2026-10-26 (re-nm28):** MATCHES — §3.2 gives the original PreCityShutdown, cache-clear, network-lot/network-manager, occupant-drain and later prop-manager order; §8.5 preserves balanced lot ownership. Message/tick services and owned children must survive native callbacks. At 0x005D0F94 the null object is the dust effect (complete construction object +0x13C), created by successful SetLot setup, not a network-service lookup. Worked cases cover lot/occupant references and valid construction setup. The reports do not prove how that reference became null or explain the separate action crash; action acceptance remains pending an engine run.
- ~~**NM17 - section 3.2 (native construction setup with replacement automata):** After implementing NM15/NM16's configuration references and configuration-before-occupant destruction, the merged-source DLL (`a11e2915`) still fails `gen27-konradshohe-s1-grow` at 2026-01-30 with the full current house profile plus `netlotobj`: 26/216 actions execute, ref_warnings=0, followed by an execute/read access violation at no-module address `0x29A5D1EC` immediately after a replacement automaton is created (native frame `SimCity 4.exe+0x1D09CA`; shared `build-runs/game-results/netlot28-nm16-s1grow-a.log:8164`). The same DLL/profile with `netlotobj` omitted reproduces it (`netlot28-nm16-s1control-a`, 26/216, ref0). Keeping `netlotobj` and all other house keys but disabling `automata` completes all 216 actions, reaches 2028-01-30 and quits cleanly (`netlot28-nm16-s1-noautomata-a`, ref0). These measurements narrow the interaction but establish no native cause. What additional observable interface/setup/ownership contract must the native construction path's returned automaton objects satisfy, beyond the valid SetLot/dust-effect precondition of NM16? Please specify the called interfaces, ownership and any required concrete layout at this boundary; route the automata portion to its owner. The full-profile s1 acceptance remains stopped; no unrelated automata or construction behavior was guessed.~~
  **ANSWERED 2026-10-26 (re-nm29):** MATCHES — network-manager.md §3.2.1 and automata.md §7.6 specify the seven-argument creation ABI, Windows model-key word order, immediate flag/orientation/AddRef calls, borrowed return plus independent manager/construction ownership, scaffold updates and deferred removal. Retained native internal creation directly accesses embedded occupant/exemplar/property-holder views and model-key fields/vector; retained native removal requires the embedded effects object. Worked starting-site, native-factory, null-result and release cases distinguish those contracts. The reported execute address equals the passed model-group constant; this is not proof of a particular DLL failure. Full-profile action acceptance still needs an engine run.

<!-- from: automata-actions29 and netocc-cities29 (2026-10-26); answered/narrowed by re-nm30 -->

- ~~**NM20 - section 9; native consumer after generated s2 network actions:** Please identify the
  consumer/interface boundary at `SimCity 4.exe+0x3529C0` (`0x007529C0`), its required object view,
  nullability and ownership contract, and the subsystem that supplies that object. With the fixed
  NM17/AT25 automata boundary, the full house list's `gen27-konradshohe-s2-grow.script` stops at
  **84/216** actions: `aa29-at25-house-s2-b.log` reads null+`0x6C` on 2026-09-01. Removing only
  `automata` produces the **same fault and 84/216** on 2026-08-31 in `aa29-at25-s2-noauto-b.log`;
  that log explicitly confirms automata is off. Both runs use DLL SHA256 prefix `afb59058`.
  Logs and exact requests are in main's `build-runs/game-results/` and `build-runs/game-queue/done/`.
  The last scripted call is line 88, `network 40 51 40 19 5`, returning `ok=0`, followed by tool
  Shutdown. This timing does not establish that placement caused the fault: please route the
  boundary to another spec if necessary. Section 9 specifies the placement sequence but does not
  identify this later consumer. A full-house repeat on the earlier same-source DLL (`6c4bb917`,
  `aa29-at25-house-s2-a.log`) has the same fault at 84/216. In contrast, s1 completes 216/216 and
  quits cleanly (`aa29-at25-house-s1-a.log`). [current choice: leave s2 unaccepted; stop this item
  rather than invent a layout, null fallback or native ownership rule]~~
  **ANSWERED 2026-10-26 (re-nm30):** DIFFERS — routed from §9 to terrain-view.md §4.4.1 and lot-models.md §4: 0x007529C0 is terrain-view slot 15, consuming receiver+0x14 = base+0x20 terrain, then terrain+0x6C. cSC4CellTextureModel::Draw calls it through draw-context+0x4C and returns to 0x005E0487. The terrain owns/supplies the initialized view, which retains terrain; no null fallback exists. Worked lifetime case included. Remaining NM20-A requires the native sequence/control and first cache/terrain-pointer write with Init/Shutdown history; the supplied 84/216 crashes remain failed. Originally also filed as NM18 by automata-actions29; netocc-cities29 retains NM18.

- ~~**NM18 — terrain removals beyond §7.6's destruction decisions (formerly filed as NM16):** in `gen-tegel-s0-default`, the shadow
  reports 14 extra original removals around cells (65,69..70), (65..66,77), (80..81,69..70). The original's
  component 53 and every component-11 lot/config/zone/wealth entry agree. The core queries with mask 0x1FFF,
  applies the kind/pipe/subway/0x80000 gates and the specified four-corner altitude test. Which native handler
  or network-tool side effect removes these pieces, and at what point should the comparison capture their
  terrain/candidate inputs? Is the §7.6 query/filter or the ownership of the resulting removals incomplete?
  `build-runs/suite/netocc-cities29-before5/gen-tegel-s0-default.log` reproduces 14 errors, netmgr exact 2/3,
  clean/ref_warnings=0. A late listener registered after city Init checks each held candidate immediately
  after 0x278E6003 / 0x6A3B0AFD. `netocc-cities29-probe1/gen-tegel-s0-default.log` still reports 14 errors;
  every listed extra removal has `placed after handler=0`. This disproves a simple later-in-the-tick removal
  explanation; moving the check alone is not justified. The diagnostic now also captures placement at the
  candidate query. This blocks the 795 terrain-detail occurrences in the supplied census, including Tegel
  and the gen27 Konradshohe/Spandau cases. No mismatch, candidate or tolerance has been removed or relaxed.
  The current boundary probe reproduces Tegel with the shadows-profile simulator settings:
  `C:/Users/tlori/FreeCity4/build-runs/game-results/netocc-cities29-profile5-tegel.log`, clean/ref_warnings=0,
  netocc exact 2/2, netmgr exact 1/3 (16 mismatches). Every reported extra removal was placed at the query
  and absent at the late terrain listener (`placed before/after handler=1/0`). Thus already-removed query
  results and a simple later-tick check do not explain these cases.
  A separate timing case is now proved in `netocc-cities29-broad6-06.log` (gen27 Tegel grow): some pieces
  reported as original-only removals were still placed at the late terrain listener (`1/1`). The comparison
  now checks every candidate, including predicted destructions, at each handler boundary; the next-tick
  path remains the fallback for explicitly selected early-listener mode. NM18 remains about the `1/0` cases.
  With that correction, `netocc-cities29-bound8-grow.log` reports 12 rather than 17 errors, and
  `netocc-cities29-bound8-default.log` reports 10 rather than 16. Both are clean/ref_warnings=0; every
  remaining reported extra removal has `1/0`. These logs are in the main checkout's `build-runs/game-results`.~~
  **ANSWERED 2026-10-26 (re-nm30):** DIFFERS — §7.6.1 distinguishes direct destruction seeds from tool closure and neighbour replacement; §7.6 records the actual x87/store boundary. Worked A/B/B′ case included. Remaining NM18-A requires ordered slot-50 seeds, closure/solve provenance and actual pre-consumption altitude operands for the Tegel 1/0 identities; existing logs cannot assign those removals to a caller.

- ~~**NM19 — Port of Shiganshina's first lot/random divergence (§§8.5, 8.7, 17.3):** the census sample
  `C:/Users/tlori/FreeCity4/build-runs/run7/census3/paradis-port-of-shiganshina.shadows.txt` first differs on
  2041-01-01: lot (1,25), config `4BA69450`, exists only in the original, with variation 1, rotation 3,
  flip 0, usable 1 and current compatibility false; zone/wealth are 0/0 in both. There are also two different
  compatible choices and generator next-word indices 386/383. Component 53 and all zone/wealth cells agree.
  After the avenue hash-order/bounds fix, which message or direct entry should first affect this edge lot,
  and which inputs must be retained before the native handler changes them? In particular, does the specified
  replay need transient forced/locked lot state not represented by component 11, or another native draw path?
  Do not infer an answer from the later incompatible configuration alone. The new diagnostic reports the
  first random disagreement's message and four data words without resyncing the generator or hiding errors.
  Fresh attempts `netocc-cities29-probe4-paradis` (corrupt during load) and `netocc-cities29-profile5-paradis`
  (stalled at the script's 2040-12-12 terrain action) cannot reach the census's first differing month;
  the latter starts with an exact netmgr load sample. This blocks that distinct census-only case, not a
  claim that the avenue fix has passed it. All comparison checks remain enabled.~~
  **ANSWERED 2026-10-26 (re-nm30):** DIFFERS — §8.5 NM19 specifies transient forced configuration and lot locks, their absence from component 11, the post-object-creation re-read, and a no-selection-draw forced/locked worked case. Remaining NM19-A requires the first event/direct-entry trace with those inputs, avenue queries and generator state; the later census and failed fresh attempts do not identify the first message or prove a new draw path.

- **NM22 - manager lookup slots 10/11/12: typed ABI and ownership (§§7.5, 7.9):**
  Please specify the three argument types/order and result channel for `FindPipeOccupant` and
  `FindPipeConnectionOccupant`, including the lookup filters, coordinates, failure/output behavior
  and reference ownership. Section 7.5 currently only forwards their names to static pipe-tool
  helpers; network-tools does not define those helpers. ABI interfaces.tsv gives pop 12 and
  `eax-or-void`, but no argument meaning. For `GetAllIntersectionPieces` (slot 12, pop 16), §7.9
  defines the flood but leaves the four argument types/order, output container and acquired-entry
  ownership unspecified. Please supply those interface facts and one worked lookup/flood example.
  Current implementation retains zero-return stubs with SPEC-GAP comments. This is an implementation
  blocker, not an attribution of an observed rand/save difference: R12's new-pin qcheck is pending
  reference recording. The specified station/water/subway predicates have been implemented separately.

**ANSWERED 2026-10-28 (re-blockers37b):** DIFFERS — network-manager.md §7.9.1 specifies typed manager slots 10/11/12, last-QI pipe selection, output ownership and depth-first pointer-list flood.

<!-- from: automata-actions29 (2026-10-26) -->
<!-- from: network-exact33 (2026-10-28) -->
- **NM21 - exact iteration order of lots, potholes and configuration groups (sections 2.2, 3.2, 4.2):**
  Please specify each map's hash function, initial bucket count, resize trigger/prime selection, collision
  insertion and overwrite order, rehash traversal, and whether Read uses ordinary insertion. Section 8.5
  gives these rules for `pending`, but does not establish that the three maps use the same parameters.
  A worked component-11 save with colliding cell keys before/after a resize would fix the reference order.
  The core currently uses insertion-order `OrderedMap` for lots/potholes and first-loaded configuration
  groups. The coordinator's pre-round netlot check on timbuktu-rush-hour first shifts object ID 16
  (class 6A26D049); this round's qcheck-2 has no fresh baseline, so map order is an unproven candidate,
  not an attribution of that difference. [current choice: retain insertion order; do not borrow the
  pending set's bucket rules for unspecified maps]
  R3 follow-up: after correcting the plugin's section 6.2 message payload dispatch, qcheck-14 on
  timbuktu-rush-hour now differs in only 6 save records (5 only relabelled); the first content
  difference is component 11 itself, E9A6CD3B/299B2D1B/0. Both saves contain 11,775 objects and
  364 lots with the same keys/configuration IDs, no potholes, and a 5,118-byte lot-manager entry.
  The first serialized lot key is 0x7072 in the reference and 0x1E64 in ours; object IDs first
  shift at 1357, class C9C05C6E. No rand() or DIGEST difference. Run:
  C:/Users/tlori/FreeCity4/build-runs/golden-runs/compare-20261028-032628-8e028584/result.
  The original-game shadow network-exact33-netlot-shadow-r3-1 is clean (ref_warnings=0), exact
  5/5 samples by its entry comparison, and reports record11 "same entries (order differs)" after
  city changes. Map serialization order is now a measured remaining difference; the specific
  native hash/bucket contract is still missing, so no implementation parameters were inferred.

**ANSWERED 2026-10-28 (re-blockers37b):** DIFFERS — network-manager.md §2.2.1 proves initial B=193 for all three maps, u16/u32 identity hashes, miss-only count/prime resizing, collision prepend and rehash traversal, ordinary saved-map Read, configuration-vector order and a derived 193→389 component-11 ordering fixture. No new game run or measured-save exactness is claimed.
