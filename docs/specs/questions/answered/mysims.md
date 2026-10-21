# mysims: answered questions (archive)
<!-- Archive for [mysims.md](../../mysims.md). Lanes do not read this file. Open questions are in ../mysims.md. -->

## From the mysims core audit (cores26, 2026-10-20); answered by re-answers35

- **MS1 (§11.2.1, §6.2 rows 25 and 30):** the unnamed s32 pair (record row 25, version ≥ 17) and the unnamed bool
  (row 30, version ≥ 14) have no observed meaning. [inferred: the pair is the idle offset; the bool is UI state]
  **ANSWERED 2026-10-21 (re-answers35):** DIFFERS — row 25 (MySim+0x204/+0x208) is dead data: zeroed in the constructor (0x00429A9E), written/read by Write/Read, and touched by no other cSC4MySim method (asm scan of all 69); row 30 (MySim+0x245) is the "pylon locked" flag set by DispatchToolLockPylon (0x0042728E) and gating is_pylon_location, DispatchToolEnd, the pylon timer and UpdateTripVariables. Neither is idle offset or UI state. Spec §2.2, §6.2 rows 25/30.

- **MS2 (§11.2.2, §4.2):** UI service `0xA417445E` (instance `0x5A4`) slot `+0xA8` — which window or state blocks
  the MySim daily and monthly steps? [inferred: a modal window]
  **ANSWERED 2026-10-21 (re-answers35):** MATCHES — service 0xA417445E is the window manager (kGZWinMgrDefaultSysServiceID, cIGZWinMgr IID 0x5A4); slot 42 (+0xA8) resolves to 0x009DB025, which returns (modal depth at WinMgr+0x17C) != 0, the counter cGZWinMgrBase::DoModalWin increments/decrements around its loop (0x009DBF11/0x009DBF2D). simulate_day (0x0042DCD9) and simulate_month both skip while any modal window runs. Spec §4.2.

- **MS3 (§11.2.3, §2.2):** does MySim `Init` re-resolve the neighbour job city from the saved cell (the app's
  region lookup), and does it set `neighbor_job` true only when a city is found? [inferred: yes]
  **ANSWERED 2026-10-21 (re-answers35):** DIFFERS (in part) — yes, Init (0x0042CEF3-0x0042CF44, state != 0 only) re-resolves: if the MySim has no job and the saved neighbour cell is not (0,0), it calls cISC4App slot 35 then cISC4Region::GetCity(x, z) and stores a non-null result; there is no separate neighbor_job bool ("has neighbour job" = the city ref is non-null). What Init sets unconditionally in that branch is the commute-check flag, so the next day's CheckCommute validates the job. With a job or cell (0,0) the ref is released (quirk: cell (0,0) is never re-resolved). Spec §2.2.1.

- **MS4 (§11.2.4, §4.9):** the building development simulator's slot 44 list used by `DistanceToClosestSpecial`
  (§7.2): landmarks and rewards only, or every special building? [inferred: landmarks and rewards]
  **ANSWERED 2026-10-21 (re-answers35):** MATCHES — DistanceToClosestSpecial (0x00426AE0) walks cISC4City slot 53 -> BDS slot 44 (0x00520080, the list at BDS+0x4C), which BDS fills from occupant inserted/removed messages only for occupant groups 0x150A (landmark) and 0x150B (reward), previews excluded (building-development.md §3). Distance is float32 sqrt(dx^2+dz^2) truncated, unsigned minimum from 0xFFFFFFFF; entries without cISC4BuildingOccupant skipped. Spec §4.9.

- **MS5 (§11.2.5, §4.5):** the exact clamp and neighbourhood of `local_traffic_congestion` (the Mac build shows
  the rectangle grown by 1 cell on each side, clamped to `[0, CellCountX/Z − 1]`), and the neighbour-job constants
  of `job_condition` / `job_has_power` / `job_has_water` (0 / 1 / 1 in the core). [inferred]
  **ANSWERED 2026-10-21 (re-answers35):** DIFFERS — local_traffic_congestion (0x0042B3DE-0x0042B581): home lot bounding rect grown by 1, min edges clamped to 0, BOTH max edges clamped to CellCountX-1 (city slot 93; quirk); only the border ring of the grown rect is visited (not the interior), and only cells with traffic GetCapacity(1,x,z) != 0.0 count; u8 max of the congestion map, 0 if none. Neighbour-job constants confirmed 1/0/1/1 (is_neighbor/condition/power/water). New: a city job whose building profile purpose is 5 (agriculture) gets job_has_water = 1 without asking the plumbing simulator (0x0042BA92). Spec §4.5.

- **MS6 (§11.2.6, §4.7):** does the traffic pathfinder `CheckCommute` and `FindNewJob` run have any side effect on
  traffic data (a recorded trip)? [inferred: no]
  **ANSWERED 2026-10-21 (re-answers35):** MATCHES — neither CheckCommute (0x0042D9E0; pathfinder slots 5, 9, 18, 19, 20, 23, 26, then Release) nor FindNewJob (dispatch SetCurrentCommuteDestination 0x004350D0 -> validateCommuteLocation 0x00432F50) calls ApplyTraffic (cISC4PathFinder slot 29, +0x74) or another volume writer: no trip is recorded. Only the process-wide work-check interval/adaptation counter (pathfinder.md §4.11) is shared. Spec §9.

- **MS7 (§11.2.7, §4.1, §4.6):** the "announce" check of `AnnounceMoveIn` (the class slot that passes on the
  reload case), and the dispatch state numbering 4/5/6. [inferred: announce is skipped after a load]
  **ANSWERED 2026-10-21 (re-answers35):** DIFFERS — the "announce" check is cISC4MySim slot 56 RequestToPlaySound (0x00424DE0), a sound throttle, not a reload test: true when state != 0 and the MySim's seconds timer (cRZTimer units 5, MySim+0x2A0) is more than 4 s past the last true reading (MySim+0x29C, initially 0), which it then stores; only MySimMovedIn is gated, UpdateVariables and MYSIM_MOVE_IN always run. States: 4 to work, 5 home (not when state 1), 6 elsewhere/pylon, set on dispatch trip start 0x4BB71A7E (0x0042D364); arrival sets 2 (AnnounceMoveIn if it was 1) or 3. Spec §2.2, §4.1.
