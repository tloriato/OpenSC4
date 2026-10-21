# Lane glue21fix2 (PL17 fixed; PL18/PL19: the game's growth pass drains lot configurations to zero)

Continues branch `glue21` in worktree `C:\Users\tlori\FreeCity4\.claude\worktrees\glue21` (no rebase; the
restart briefs' `glue21fix2` worktree does not exist - keep working here). Commits: `0c5d5625` (slot 27
vector fix = PL17), `1b840d70` (prop token QI normalization + lotcfg set-query reference spec + diagnostics),
`65d97b40`/`ba74dce5`/`ad979728` (docs).

| Lane | Worktree | State at stop | Next steps |
|---|---|---|---|
| glue21fix2 (PL17 fixed; double-add kept; PL19 filed, blocked on RE; round 5: the pin experiment proves both accommodations fail) | `C:\Users\tlori\FreeCity4\.claude\worktrees\glue21`, branch `glue21`, tip `f4277170` | in progress | see Handoff |

## Handoff
- Goal: land the `enable = propmgr` / `lotcfg` replacements game-clean (three shadows exact, ref_warnings=0,
  turbo to 2032) and flip the merge-queue entry to ready. PL17 (slot 27 `GetObjectIDArray` vector) is FIXED;
  the remaining fault is PL18/PL19: the game's growth-pass release wave (SimCity 4.exe `0x00709798`) drains a
  touched lot configuration to ZERO whatever its reference count, and the wrapper deletes under the catalogue.
- ROUND 5 (2026-10-21) ran the decisive diagnostic the handoff asked for first (one change, one run,
  glue21fix2-pin-1): `cFC4LotConfiguration::Release` floors at 0 and never deletes (the wave "pins" the
  wrapper; the catalogue owns it). Result, from `build-runs/game-results/glue21fix2-pin-1.log`:
  - NO deletes and NO AV (round 4's fault gone; ref_warnings=0), but the growth pass STALLS: watchdog
    fires inside `fc4_sim_on_tick` 5.7 s after the wave (`SimCity 4.exe+0x304D97` on the wave's own
    stack), the run ends at 2026-01-28 (turbo to 2032 never finishes). Reverted (`git checkout`);
    the tip stays `f4277170`, the working tree clean.
  - The wave's structure, measured for the first time in full: ONE slot-9 query (89 matched configs,
    65194E08 = core with_refs add + 00709704 = our double-add, one each per config, back to back),
    then IMMEDIATELY the release wave at `00709798`: 34 releases over 14 of the 89 = 8 configs
    released exactly ONCE (stopping at count >= 2) + 6 configs released once per ROUND for 4
    interleaved rounds until zero (rounds = round-robin sweeps, round order 05397290, 037CF900,
    037CD8F0, 05397878, 037CDFB0, 05397A28, then again). In ref2-3 (the 40-ref cushion) the same
    shape: 5 configs x 43 rounds + 2 singles; in ref2-2: 2 configs, both drained (4 and 3 releases).
  - Membership in the wave is NOT decided by anything we can see: the 14 wave-released configs have
    ledgers identical to 75 not-released ones from the same query (same 4 refs at wave start, same
    caller chains; 05397878 held 2 game refs from 006BF53D and was drained anyway, 053981C0 held 53
    and was released once). The wave iterates a game-internal per-candidate container; its release
    count per entry is the game's own pass structure.
  - CONCLUSION (round 5, final): the wave's loop TERMINATES ONLY WHEN A DRAINED CONFIGURATION IS
    DESTROYED - with the old delete the game AVs on the dead wrapper; with the pin it spins forever.
    The original's configurations survive because the game keeps its own reference per candidate
    entry (never released - PL19's "the catalogue reference is evidently never released"), which a
    replacement cannot fake from outside: no reference hand-out, no pin, and no delete reproduces
    the original. PL19 (which container 0x00709798 iterates, what the original stores per entry)
    REMAINS the blocking question and must be answered by an RE lane.
- State: the double-add is KEPT (strict improvement: day-29 deletes 23 -> 0, residual 3/day on no-lot
  configs); the per-op trace stays in `cFC4LotConfiguration.cpp` (AddRef/Release caller chains) and the
  slot-9 query markers in `cFC4LotConfigManager.cpp` - they are the instrument any RE answer must be
  verified against. qbuild all green as of round 4 (cargo 2860/0, clippy 0, build+ctest); the game runs keep
  ref_warnings = 0; the three shadows never got to run (the AV precedes them).
- ROUND 4 (2026-10-21) measured with the per-op ref trace restored (this round's runs):
  - The PL18 candidate (2 refs per matched config per slot-9 query) was ALREADY committed in `1b840d70`
    (the isoref runs predate it and measured one add per query); round 4 only refreshed the comments
    (`f65bf327`).
  - glue21fix2-ref2-1 (double-add): day 29 no longer deletes (was ~23 wrappers); day 30 deletes 3 (k=1
    query, no game-lot ref), then the same `vector<bool>` AV (`edi` = the dead wrapper).
  - glue21fix2-ref2-2 (double-add + trace): the full ledger: base after load 2 (catalogue + raw map add),
    game bulk wave +1 (exe `0x0069C385`), per-candidate game adds exe `0x006BF53D` (1..66 per config, 736
    of 1909), query +2 (core `with_refs` + plugin loop; distinct caller chains in the trace). The wave
    (exe `0x00709798`) releases -1 per config per pass; a k=1 no-lot config (count 4) drains to 0 and dies;
    a k=1 with-lot one (count 5) keeps 1.
  - glue21fix2-ref2-3 (+40-ref cushion): DECISIVE - the wave drains to zero regardless of the references:
    five count-44 configs were released 43 logged + the deleting 44th time (deleting them); two others of
    the SAME count 44 were released exactly once and stopped. In ref2-2 a count-4 config died and a
    count-5 kept 1. The release count per config follows the game's own candidate/pass structure (a config
    the game never AddRef'd was released 44 times), so NO reference hand-out balances it.
  - FILED **PL19** (docs/specs/questions/prop-lotconfig.md): which container `0x00709798` iterates, when it
    stops, what the original does so its configurations are never drained. Blocked on RE of the game exe;
    reference arithmetic cannot fix it.
- State: the double-add is KEPT (strict improvement: day-29 deletes 23 -> 0, residual 3/day on no-lot
  configs); the per-op trace stays in `cFC4LotConfiguration.cpp` (AddRef/Release caller chains) and the
  slot-9 query markers in `cFC4LotConfigManager.cpp` - they are the instrument any RE answer must be
  verified against. qbuild all green (cargo 2860/0, clippy 0, build+ctest); the game runs keep
  ref_warnings = 0; the three shadows never got to run (the AV precedes them).
- Next (round 6, after PL19 is answered by an RE lane): (1) implement the answered convention - the round-5
  evidence narrows it: the game must keep ONE reference per entry of its per-candidate container and never
  release it while the entry lives; the container's membership (14 of 89 matched configs) is chosen by the
  game's candidate logic, so the answer must name both the container and how its entries are filled; verify
  the ledger first with `glue21fix2-ref3-*` runs (reproduce, change one thing, re-run); (2) when a run
  reaches 2032 clean with `enable = lotcfg`, run the full `enable = propmgr, lotcfg` with the three shadows
  and check them exact; (3) on clean: re-run `glue21-gate-11`, write
  `<WT>/build-runs/merge-queue/glue21.json` state ready, update `docs/runs/glue21fix2.md` and this file,
  commit small. If PL19 is still not answered: report the lane blocked on PL19 - rounds 3-5 have now
  exhausted the implementer-side accommodations (reference arithmetic, the 40-ref cushion, the pin).
- Facts: the isoref/ref2/pin logs are git-ignored but still on disk in `build-runs/game-results/`
  (pin-1 is the round-5 record). The qbuild all verdict may report `ninja did not finish (0/3)` on an
  up-to-date tree (touch a plugin .cpp first). Checked build:
  `cmake -B build-x86-checked -DCMAKE_BUILD_TYPE=RelWithDebInfo -DFC4_CHECKED=ON`; the queue accepts
  `dll = engine\build-x86-checked\FreeCity4.dll`. Game-request labels outstanding: none.
  PL15/PL16 answered, PL17 fixed, PL18 open (the over-release description), PL19 open (the decisive one)
  in docs/specs/questions/prop-lotconfig.md.
  Do not run `python tools/questions.py` (tools/ is out of bounds); the coordinator regenerates.

## What was wrong (PL17, fixed at `0c5d5625`)

The game's `CreateLotObjects` (network-manager §8.6) calls lot configuration manager slot 27
`GetObjectIDArray(object, vec)` and reads the result back as a **vector**: `RandomUint32Uniform(|ids|)`
then `ids[pick]`. Our implementation filled the caller's container with `FillSet` — an STLport **hash_set**
layout — while the game passes an `SC4Vector<uint32_t>&` (SDK header `cISC4LotConfigurationManager.h`, spec
§B5.6 "clears vec"). The hash-set insert left the vector's begin pointer at 0 with a garbage end: the draw
came out ~29.7M and `ids[pick]` faulted at `pick × 4` — exactly the PL17 signature
(`edx 0x01BA4CBE × 4 = 0x06E932F8`, `ecx = 0`). Fixed by filling the caller's `GameVector<uint32_t>` (clear
+ push_back through the game allocator, the same pattern the shadow uses). A use-after-scope in the same
function (`co.ids` pointing into a dead local vector) was fixed alongside.

## The second fault the PL17 fix exposed (RTC #0 → int3)

With slot 27 working, the run survived city Init for the first time and reached the game's `CreateProp` →
our slot 32 `AddStreetProp` → core `add_new` → host `set_position` (`cFC4PropManager.cpp:416`) → RTC check
#0 "ESP not preserved across a call" (visible only in a checked build, run `glue21fix2-rtc-1`). Root cause:
`CreatePropInstance` handed the core the **cISC4PropOccupant primary** pointer (QI `0xE9793A65`), whose
vtable slot 9 is `GetOrientation` (pop 0) — calling occupant `SetPosition` (pop 4) through it desynced the
stack. This latent ABI bug predates glue21fix2; every earlier run died at slot 27 before reaching it.

Fix (`1b840d70`): `CreatePropInstance` normalizes every created prop token to the **cISC4Occupant
sub-object** (QI `0x26BB767E`, the factory's creation reference released after the QI hands the core its
own); all occupant-side host callbacks (`OccupantInit/Shutdown/Flag`, `SetPosition`, `Position`, `Hide`,
`InsertOccupant`, `RemoveOccupant`) resolve the occupant via QI with a scoped reference; prop-side callbacks
(`SetPropType`, `SetBoundingBox`, `SetPropUsage`, `SetOrientation`) QI back to the primary;
`CreateProp`/`CreateGenericProp` return the primary to the game with the creation reference transferred
(one reference owned, §A6.1 step 6).

## The remaining fault (PL18, open)

With `enable = lotcfg` (isolated runs `glue21fix2-iso-lotcfg*`, `isoref-1..4` — no propmgr, so independent
of the prop fixes), the city loads and runs to turbo, but the growth developer's SimNewDay pass faults
deterministically: AV reading 0 inside `std::vector<bool>` code reached from our
`cFC4LotConfiguration::IsCompatibleWithPurposeType` / `GetRequiredRoads` frames — the bit vectors of a
wrapper that was **deleted** while the catalogue map still held it. The full measured ledger is in the
Handoff above; the release site is the game exe ILT `0x00709798`, release rounds 23/17/17/5, and the
release count is independent of the game's own add count (the key fact this round established).

## Run ladder (this lane)

| Run | DLL | Verdict | Fault |
|---|---|---|---|
| glue21fix2-gate-1 | debug | exited -1 | RTC #0 int3 in `SetPosition` (prop token primary/occupant mismatch) |
| glue21fix2-rtc-1 | checked | corrupt | RTC #0 named: `SetPosition` ESP mismatch via `add_street_prop` from the game's `CreateProp` |
| glue21fix2-rtc-2/3 | checked | exited -1 | over-release cascade (fixed: audit the factory reference) then the lotcfg vector<bool> AV |
| glue21fix2-iso-lotcfg1/2 | debug, lotcfg only | exited -1 | same vector<bool> AV at SimNewDay 29 (independent of propmgr) |
| glue21fix2-iso-lotcfg3/4/5, isoref-1..4 | debug, lotcfg only | exited -1 | the PL18 over-release: wrappers deleted by the game's growth pass (ledger measured) |
| glue21fix2-ref2-1/2/3 | double-add (+trace, +cushion) | exited -1 | the wave drains touched configs to zero regardless of the reference count - PL19 (round 4) |

Full run rows: docs/runs/glue21fix2.md.

## Notes for the successor

- The `qbuild all` build verdict can report `ninja did not finish (0/3)` on an up-to-date tree: the
  `fc4_capi_cargo` custom target is always dirty, `restat=1` then skips the downstream link edges and ninja
  prints only `[0/3]` — the parser reads that as unfinished. Workaround: touch any plugin .cpp (mtime) so
  real edges run, or clean-first. The build itself is fine (RC 0, DLL fresh).
- `symtool.exe` (glue24 worktree build-runs) symbolizes RVAs against `FreeCity4.pdb`; the queue copies the
  pdb next to the DLL. Compute RVA as `absolute - base` from the fault line.
- The checked build (`cmake -B build-x86-checked -DCMAKE_BUILD_TYPE=RelWithDebInfo -DFC4_CHECKED=ON`)
  turns RTC failures into named call sites — invaluable for ABI mismatches; the queue accepts
  `dll = engine\build-x86-checked\FreeCity4.dll`.
- The propmgr/lotcfg shadow steps silently skip when the city holds our own class (`IsOurObject` early
  return in `PropRunStep`/`LcmRunStep`) — expected behavior, not a regression (PL15 context).
- `refaudit_lines` ini key raises the refaudit verbose cap (default 2000; `AppCity` QI spam floods it).
