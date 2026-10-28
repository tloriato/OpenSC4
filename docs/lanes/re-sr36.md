| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| re-sr36 save layouts (RE/spec) | Codex; re-sr36 worktree; detached HEAD, reserved branch re-sr36 | ready: layouts c225f49b7, 9f1e343d4; answers 206e025a4; full-corpus proof 84c6251d0; docs build gate n/a; corpus 1,641/1,641; diff/index passed; global question check has 18 pre-existing errors; ABI unchanged, no seam reservations; game/shadow n/a; SR33-2 and SR33-3 answered | 1. Enqueue the docs-only tip. 2. Reconcile the archived SR33 answers if save-roundtrip33 later introduces the open question file. |

- Layouts: traffic-network §6.2 (map v3+ gates, tiles, intersections/sides, references, occupant-to-four-lists table); foundations §3.6 (city v8–12); save-records §2.1 (auxiliary v1 and inner cheat data).
- Map ID correction: IID 0xCA0F81B9, class/record type 0x6A0F82B2; documented creation-order walks and variable-width references.
- Verification: build-runs/verify_sr36.py --limit 100000 --summary, run directly in PowerShell, exit 0; all 547 available main/build-runs/golden/*/end.sc4 files, read in place.
- Map: 547 v7 objects, 583,750,387 decompressed record bytes; 1,602,187 tiles, 44,270,243 path pairs, 95,284 tile references, 1,608,109 extra triples, 110,768 intersections, 245 final occupant entries / 498 list values.
- City: 547 v12 objects, 318,073 decompressed record bytes, 28,991 component-reference slots.
- Auxiliary: 547 v1 records, 14,222 decompressed bytes; all inner 20-byte empty cheat blocks independently parsed, including both total sizes and section lengths.
- Total: 1,641/1,641 target records, 584,082,682 bytes consumed, zero bytes left over; class byte counts include entry headers. Both full-corpus runs passed; the second also checked independent map/city size equations, now in the specs.
- Corpus key range: 004c1a2b77ee732977859a79a8a726b95199310799ede8bb070c6b7f66aec3a3 through fe2bec374208e4f7659dfb3347d78ca78895f7242ee98b554dab3b86e879099b (lexicographic).
- Legacy gates, nonempty city property holders, marked-train lists and keyed-reference lists were verified statically, not exercised by these saves.
- REA again returned 4/4 target_unavailable (no app open); used re_query.py, main decomp/ and byte-bearing assembly pages as prescribed. Float constant 0x00A8FDF4 independently rechecked in the executable: 12582912.0.
- The historical q.py attempt failed before parser startup (Bash CreateFileMapping error 5); the initial direct 20-save checks and both new direct 547-save checks passed. No failed or partial corpus parse is counted.
- SR33-2 / SR33-3: docs/specs/questions/answered/save-records.md; questions.py index passed; find resolves both as answered. The open save-records.md question file was absent in this base and main, so archive entries use the assigned brief.
- Gate: git diff --check and question-index regeneration passed. questions.py check still reports the same 18 pre-existing diagnostics in untouched files; none concern this lane. Per docs/merge-train.md, the build gate is skipped for this docs-only change; the unrelated question diagnostics do not require engine/tool changes or hold this lane.
- Cargo/clippy/ctest and game/shadow runs n/a for this spec-only lane. No engine/, tools/, docs/abi/ or executable edits; no proposed ABI rows.
- Commits succeeded on detached HEAD despite the shared packed-refs.lock permission diagnostic; shared refs were not changed. No proposed ABI-row file.
