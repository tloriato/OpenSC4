# Lane glue21 (the prop manager / lot configuration manager replacements, the save sweep and the refcount fixes)

A HANDOFF §0-style row for the coordinator's table, plus the knowledge the next lane needs.

| Lane | Worktree | State at stop | Next steps |
|---|---|---|---|
| glue21 (`enable = propmgr` / `enable = lotcfg`; run4: PL19 closed; integrated with main 96f9516d) | `C:/Users/tlori/FreeCity4/.claude/worktrees/glue21`, branch `glue21` | ready: merge ffedac59; gate cargo 3577/0, clippy 0, ctest 62/62; glue21-i1-1 (enable=propmgr,lotcfg, turbo to 2032) clean, ref_warnings=0; glue21-i1-2 (shadows over originals) clean, all propmgr/lotcfg/tutorial shadows mismatches=0 | coordinator: union regress + train; PL20 (confirm the pointer hasher v>>2), PL21; the shadows skip FreeCity4's own class, so the union regress checks the replacement |

## Integration fix (2026-10-21, run4 round 2/3)

- Merged main 96f9516d into the branch (ffedac59). Conflicts: `FC4Subsystems.cpp` (main's includes and
  registrations kept, propmgr/lotcfg appended after them), `fc4.h` (main's side: ABI 75, the history comment
  closes; the branch had no declaration changes), `city_detail_tests.rs` / `prop_developer_tests.rs` (one
  `sweep_round_trip` each: main's, used by both `prop_city_sweep_tests.rs` and `save_sweep_tests.rs`; main's
  `cd8_pass_count_per_handle` kept), `engine/README.md` (both bullets), open questions (PL19 dropped from the
  open file since main archived it answered in `answered/prop-lotconfig.md`; PL15-PL18, PL20, PL21 stay open;
  QUESTIONS.md row 6 open / 8 answered / next PL22, total 58).
- The PL19 pointer hash (`PointerHashU32`) and the per-call AddRef in slot 9 are unchanged.
- Main's aura / MySims / automata / network-manager shadow files: 0 lines of diff against main.
- Gate (qbuild all): cargo test 3577 passed 0 failed, clippy 0, build 182/182, ctest 62/62.
- glue21-i1-1: `enable = propmgr, lotcfg`, turbo to 2032-01-05, clean, ref_warnings=0, foreign exceptions 0.
- glue21-i1-2: the three shadows over the originals, turbo to 2032-01-12, clean, ref_warnings=0; every
  propmgr / lotcfg / tutorial shadow line mismatches=0 (PL21's intermittent sample did not recur this run).

## The refcount fixes (what was actually wrong)

1. **The manager was destroyed during the class-store walk.** With the original
   `QueryInterface` (no AddRef), the segment table's reference and the loader's
   transient reference were the same count; the loader's release after `Read`
   reached 0 and `delete this` ran while the city's component-9 field still held
   the pointer. City Init then called slot 3 through a freed vtable
   (`[0x097A0710] = 0x80`, the heap's fill) — the 0x45BF49 fault of gates 1-4.
   The fix is the COM contract: every successful `QueryInterface` AddRefs
   (cFC4Simulator::QueryInterfaceImpl is the model).
2. **The configurations were deleted right after the add.** `add_by_key` creates
   the object, `add_configuration` takes the catalogue's reference and
   `add_by_key` releases the creation one. The original helper AddRefs right
   after the construct, so the creation reference exists to be released.
   `CreateConfiguration` now does the same; without it the count hit 0 and the
   object died, leaving every `GetLotConfiguration` a dangling token (the
   garbage-config fault of gates 7/8).
3. **Slot 20 handed the game null.** `GetLotConfigurationMap` /
   `GetLotConfigurationFamilyIDSet` returned nullptr; the BDS's density walk
   (building-development §4.7.4) dereferences the map with no null check
   (gate-5: the AV at `SimCity 4.exe+0x4B0390` reading 8). Both now rebuild
   game-shaped STLport hash containers (0x14-byte table, 12-byte map nodes, 8-byte
   set nodes, prime buckets) from the core's catalogue on every call and hand the
   game real addresses. The map holds one reference per configuration.
4. **`Position` used the wrong interface on the token.** The occupant manager's
   queries hand the core `cISC4Occupant*` tokens; `Position` cast them to
   `cISC4PropOccupant*` and called `AsOccupant` — prop-occupant slot 3 on the
   OCCUPANT vtable is `Init` (returns 1), and the follow-up `GetPosition`
   dereferenced 1 (gates 9/10: the AV at `+0xA843DB` reading 1). Fixed to call
   occupant slot 8 directly, as the shadow's `PropPosition` already does.

## The run ladder (all `glue21-gate-N`, enable = propmgr, lotcfg, three shadows)

| Run | Verdict | Fault |
|---|---|---|
| gate-1..4 | corrupt | `SimCity 4.exe+0x5BF49` reading `0x8C` — the manager freed under the city (fix 1) |
| gate-5 | corrupt | `SimCity 4.exe+0x4B0390` reading 8 — the BDS walked our null slot-20 map (fix 2) |
| gate-6 | corrupt | eip 0 — the map's rebuild crashed on the dangling config tokens (fix 3) |
| gate-7/8 | corrupt | the lot's `GetLotConfiguration` returned freed objects (fix 3) |
| gate-9/10 | exited | our `Position` dereferenced 1 after the wrong-vtable `AsOccupant` (fix 4) |
| gate-11 | corrupt | `SimCity 4.exe+0x60D4B5` in the game's `CreateLotObjects` — **open, PL17** |

gate-11 reaches: PostCityInit, `lotcfg: process configuration map rebuilt, 1909
entries`, the shadows ON over the replacements, and the demand/growth/... agent
registrations — the whole city Init except the tail.

## The save sweep (done, unchanged)

Record `0x89C48F47` is the **prop developer** (`cSC4PropDeveloper`, city component 10), not a lot-configuration
record; the "city detail class" is `cSC4CityDetailManager` (`0xEAD4BA36`, component 51). Both have `fc4-capi`
`Read` / `Write` and unit tests and are in the class-record sweep (`engine/capi/src/save_sweep_tests.rs`).
Run over the 421-distinct-save corpus: prop-developer 421 records (409 converted v4 → v5, second trip stable),
city-detail 16 records, 0 failures.

## The three shadows over a replacement

`FC4PropLotShadow.cpp`'s shadows stay on when `enable = propmgr` / `lotcfg` / `tutorial` is set: the shadow reads
whatever the city holds, writes that class's own record and runs a private core over it on a read-only host.
Over a replacement the comparison is a class-layer self-check; over the original it is glue15's differential.
gate-11 reaches PostCityInit with all three ON and no INCOMPLETE lines before the fault.

## Numbered questions

1. **PL15** (the shadow-over-a-replacement comparison) and **PL16** (the B7 slot 5/6 renames) — filed in
   `docs/specs/questions/prop-lotconfig.md` with the answers bracketed.
2. **PL17** (the network-lot `CreateLotObjects` fault) — filed open; see the table row's next steps.
## Run4 report (2026-10-21)
- Cause of PL19's "drain to zero" (from the round-5 per-op trace glue21fix2-pin-1, replayed by
  build-runs/wave_walk.py): not a reference deficit. Our slot 9 placed set nodes at v mod 193, but the growth
  pass's iterator recomputes a chain-end node's bucket as (v >> 2) mod n, so it jumped backwards and cycled over
  6 nodes forever (26/26 chain-end jumps fit v>>2, 0/26 the identity). Releasing on each visit drained them.
- d5f89876: FC4GameList.h `GameHashSetU32<Alloc, Hash>` (IdentityHashU32 default, PointerHashU32 = v>>2);
  cFC4LotConfigManager slots 9/10/11/12 use `LotConfigSet`; the PL18 second AddRef dropped (the core's
  with_refs takes exactly one per returned element per call, cache hit included, §B5.2 slot 9 rule).
  6d362fea: the per-op trace and slot-9 markers removed (200 MB logs crowded out the shadow lines).
- Game: glue21-r4-1 (trace on) clean to 2032-01-07, ref_warnings=0, core slot-9 adds 949608 = releases at
  00709798 949608, zero deletes; glue21-r4-2 (no trace) clean to 2032-01-04, ref_warnings=0.
- Shadows: FC4PropLotShadow skips FreeCity4's own class (IsOurObject), so they were run over the originals:
  r4-3 (turbo) propmgr 219/220, lotcfg 293/293, tutorial 146/146; r4-4 (turbo) 220/220, 293/293, 146/146;
  r4-5 (normal speed, to 2035-11) 357/358, 477/477, 238/238. The one propmgr difference is a GetLotProps
  sample on a different lot each time (PL21); this branch does not touch the propmgr core or shadow.
- Gate: qbuild all PASS (cargo 2860/0, clippy 0, build, ctest 58/58). fc4verify reports "ninja did not
  finish (0/3)" when the cargo restat prunes every edge; touch a plugin source to force a relink.
- Questions: PL20 (the hash_set<cISC4LotConfiguration*> hasher), PL21 (the intermittent GetLotProps sample).
- Next: coordinator union regress with both keys; when PL20 is answered, update the PointerHashU32 comment.
