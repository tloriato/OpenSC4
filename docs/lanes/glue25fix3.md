# Lane glue25fix3

Per-lane row for the regenerated `HANDOFF.md` 0 table (see `docs/lanes/README.md`).

| Lane | Worktree | State at stop | Next steps |
|---|---|---|---|
| glue25fix3 | `.claude/worktrees/glue25fix3` | ready | coordinator: union regress with occmgr in the replace list, then train; after landing update Replace/coverage map |

## Report (run4 round2, 2026-10-21)

- Root cause of the `enable = occmgr` tax 0 / bankruptcy (fdd2a5ef): `FirstOut(fc4_occmgr_first_*(..., &occ), occ, out)`
  in engine/plugin/cFC4OccMgr.cpp took `occ` by value; MSVC evaluates arguments right to left, so `occ` was read as 0
  before the core call wrote it. Every first-occupant query (slots 28-35) returned false (g3: 0 hits / 42874 calls)
  and the core's AddRef leaked. Now `const uint64_t&`. The core was correct (g7 recheck of the same calls hit).
- The shutdown AV (g1, g3-g7) is gone with the fix: it was a consequence, not a separate bug.
- 51f91c0c: first-query sampler kept but off (`kSampleFirstQueries`), shutdown trace lines via FC4_LOGV (were 19k lines).
- Game: glue25fix3-g8 (fdd2a5ef, enable=occmgr, to 2026-03) clean, ref_warnings 0; first 28315 calls / 12676 hits,
  lots with building 736/741, tax 4059 (= shadow run g2). glue25fix3-g9 (51f91c0c, enable=occmgr, to 2032-01-05)
  CLEAN, foreign_exceptions 0, ref_warnings 0; first 395268 calls / 379230 hits; funds 45201 at the end, lots with
  building 677/680. The occmgr shadow is inactive under replacement (by design); shadow evidence is g2 (clean to 2032).
- Gate (qbuild all on 51f91c0c): cargo 3577/0, clippy 0, ctest 62/62. fmt clean on tests.rs.
- Questions: none filed. ABI unchanged.
- Next: union regress of all replacements including occmgr; diagnostics (budget lot-scan tallies in FC4BudgetHost.cpp,
  query counters) are one line per scan / one at shutdown and may be trimmed later.

## History

Run4 round1 diagnosis (2026-10-21):
- g1 log: the city was NOT killed by the exception first. From the first month budget tax income was 0 (g2: ~4059/month),
  funds went to -101730, the game ended the city itself at 2028-04-01 (bankruptcy), and the foreign AV (null read,
  SimCity 4.exe+0x1D0F94, called from FreeCity4.dll) happened inside "occmgr Shutdown: enter" (RemoveOccupants with
  shutdown, occupant callbacks). Same 742 lots in both runs, so lots exist but count no tax with occmgr.
- Added diagnostics (budget lot scan tallies: with building / livable / land value / building value; occmgr query
  tallies and a per-occupant shutdown trace) in 31efb035.
- g3 (enable=occmgr, turbo to 2026-05-01, corrupt): every budget lot scan has "with building 0" (745 lots, livable 723,
  land value>0 all, building value>0 0) and the lot count falls; first-occupant queries (slots 28-35) 42874 calls,
  0 hits (lists 70606 calls / 254212 items work). Shutdown AV is on the FIRST occupant's Shutdown() (trace #1: post
  Remove ok, then occ->Shutdown() of a construction occupant 0x096E763C crashes, null ecx) - likely a consequence.
- g5/g6 samples (cFC4OccMgr.cpp Sample(), every 397th first-query call): slot 33 (std cell 3,32 type 0x497F6D9D) MISSES
  though the cell holds a base texture 0x497F6D9D box [0.1,512.1..63.9,575.9]; the same cell with a null filter (slot 32)
  HITS. So the TYPE-filtered first query fails in game, while a core unit test with that exact box passes
  (tests.rs glue25fix3_first_by_standard_cell_typed_game_box, uncommitted? check git status).
- Uncommitted at the stop (commit it first): FC4BudgetHost.cpp tallies are committed; cFC4OccMgr.cpp Sample() recheck
  (logs "occmgr first-query recheck": typed fn, TYPE-kind filter struct, and first-all pointer) + the tests.rs test.
- Next: commit, quick build, queue g7 (same req as g6, label glue25fix3-g7), grep "first-query recheck". Suspects:
  includes() Filter::Type vs host occupant_type in game (HType GetType cast), or the null-filter hit being a different
  occupant (a prop) so the base texture's overlap test fails (then compare standard_box with the real box). Fix in core,
  then remove/keep diagnostics quiet, full gate, run to 2032 with enable=occmgr, then replace-union run.

- Current tip before this report f42c08b2. Baseline/full lane gate PASS: cargo3576/0, clippy clean, ctest62/62.
- glue25fix3-g1 replacement enable=occmgr is CORRUPT at 2028-04-01, foreign_exceptions=1, ref_warnings=0.
- glue25fix3-g2 is CLEAN to2032 with ref_warnings=0, but its request has ONLY occmgr_shadow=1 and no enable=occmgr. It is shadow evidence, not a replacement pass.
- NOT ready for merge; no replacement regress or coverage claim. Next: diagnose replacement g1 foreign exception, then replacement run and all-replacements regress including occmgr. Only after landing update main regress Replace and coverage-map replace key.
- The CLI background ceiling killed the round without a final result. Commits are preserved. No guard state was cleared.

## Prior Handoff

- Goal: finish the occupant manager GZCOM class (`enable = occmgr`) from saved glue25, gate, game run to 2032
  (ref_warnings=0), full regress with occmgr added to main's replace list, queue for the train.
- Done: rebased glue25 onto main ea097498 (conflicts only CMakeLists.txt / FC4Subsystems.cpp, union-resolved):
  6d10b053 (class), 0f8101f9 (2026-10-16 WIP, unverified).
- Baseline gate on 185e5ce0 (WIP included): PASS, cargo 3576/0, clippy 0, ctest 62/62.
- Next: game run to2032 with enable=occmgr (label glue25fix3-g1), then regress with occmgr added.
- Facts: submodule engine/third_party/gzcom-dll extracted by git archive, .git points at main's modules dir.
