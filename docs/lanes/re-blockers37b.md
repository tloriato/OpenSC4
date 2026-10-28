| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| re-blockers37b — disaster/effects and network blocker specs; RE/docs | Codex / re-blockers37b / detached HEAD (reserved re-blockers37b) | ready; content tip ba452bad0; 8/8 requested answers; stale NM18–NM20 removed; ABI unchanged; scoped docs/index/whitespace pass; global questions check has 15 unrelated existing errors; game n/a | 1. Implement the published contracts in disasters-exact33 and network-exact33. 2. Compare fresh reference runs; no save/rand attribution is claimed here. |

## Result
Goal: answer the eight named blocker questions; spec text only, no engine/tools/ABI-table edits.
Done: 4e455878a effects lifecycle/maps and stale NM18/NM19/NM20 removal; 8e3c6ba03 safety-consumer interfaces; afe81433a network contracts; 4e9d90d02 maps; 53c0cdfb2 underground refit; ba452bad0 highlight helper.
Answered DSEX33-EF02: effects.md §§6.1–6.1.1; direct target +8, priority 192; embedded callback timer starts PostCityInit at 1000 ms; scheduler QPC read and shutdown ownership.
Answered DSEX33-EF01: effects.md §18.12; map IID 0A5EAA1A, borrowed getter, one vertex overlay, terrain heights, bounds, shipped interpolation axis order and building updates.
Answered DSEX33-DS01 and all filed follow-ups: disasters.md §3.2.1/advisors.md §7.6; generic IID 22223BA4; fire QI 49B43F58 then virtual slot +38 GetCause, not a field read.
Answered NM22: network-manager.md §7.9.1; typed pipe lookups select last successful QI, owned output; ordered depth-first flood appends owned occupant references.
Answered NG16: network-geometry.md §8.1; occupant-manager slot 32 first match/newest-first through tool layer filter; one QI, no merging distinct occupants.
Answered NM21: network-manager.md §2.2.1; all map constructors request 100 / get B=193; full prime table, u16/u32 identity hashes, collision/rehash/Read and configuration-vector order; derived component-11 resize fixture.
Answered NT37: network-tools.md §§8.0–8.0.1; ordered cells/lot rectangles, region mask, layer query/release, zone refusal, typed preview demolition and int64 cost/reference ownership through demolition.md §§4.2–4.4; underground override.
Answered NT38: network-tools.md §§2, 11.2; exact Init exemplar selection, missing-property retention with no constructor numeric default, solved-set capacity/hash/resize/traversal and admitted-coordinate collision/resize examples.
All eight requested questions moved with provenance/follow-ups to answered files; none remains partial. NM18/NM19/NM20 open duplicates removed; existing archived answers retained.
Scoped check-blocker-docs.py passes 8/8 answers, 3/3 stale-copy checks, idempotent index regeneration, complete-lane whitespace and docs-only scope. Earlier verifier title/strikethrough matcher failures were corrected in the ignored checker, then rerun successfully.
Global questions.py check still fails with 15 pre-existing duplicate/ID-format errors across unrelated files; no failure names a file in this lane's question scope.
No game runs, engine reads, cargo/clippy/ctest, or REA calls; export/decomp/asm supplied evidence. No ABI changes or proposed rows.
All changes via apply_patch. Native lookup/check helpers and commit-message files are in ignored build-runs. No proposed ABI-row file.
Prior measured-context stop was committed as 53c83215f; this resumed round completed all three previous partials and reran the scoped gate. No engine files were read or changed.
Next: engine lanes implement and measure these contracts; the cited runtime differences have not been newly reproduced or causally assigned. Ready queue records the final report commit's full HEAD.
