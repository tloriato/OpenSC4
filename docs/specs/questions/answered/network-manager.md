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
