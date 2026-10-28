| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| owners37 save-record owner labels (tools) | Codex; C:/Users/tlori/FreeCity4/.claude/worktrees/owners37; reserved branch owners37 (detached HEAD); code 2017a394a, 928de9862, 65128bd95 | ready; ABI unchanged; seams none; pytest 449 passed, final focused 7 passed; implicit CTest 148/148; qcheck-1 outside 0/1 exact, labels 25/25; shadow n/a; questions none | - |

2026-10-28. Registry ownership remains first; generated fallbacks live in tools/record-owners.tsv (208 types).
tools/gen_record_owners.py derives class identities from explicit spec tables and cited prose bindings.
Coverage patterns keep first-match order; a missing replace key uses only a subsystem's unique key, otherwise its descriptive name. Inferred labels carry `?`.
Container, view, region, terrain and exemplar records have descriptive labels. Inferred differences participate in the canonical owner's verdict without losing their per-record `?`.
golden_dbpf.read_records(..., decompress=False) checks DBPF indices/DIR for the inventory without inflating payloads; normal reads still decompress.
Ran `python tools/gen_record_owners.py --check --store C:/Users/tlori/FreeCity4/build-runs/golden`.
Printed `record-owners.tsv: 208 record types`; `reference census: 918 saves, 168 record types, unowned 77 -> 0` (unique types across reference end saves, read in place).
Ran `python -m pytest tools/tests -q`: `449 passed in 2389.75s (0:39:49)`.
That full run collected before the additional census CLI test; final `python -m pytest tools/tests/test_record_owners.py -q` printed `7 passed in 3.39s` after the CLI addition and citation correction.
The full suite automatically queued build/CTest for its compiler-cache integration test: build 366/366 edges, CTest 148/148 passed. No manual qbuild; no engine source edits.
Ran `python <WT>/.claude/lane-bin/qcheck.py --wt <WT> --dll <MAIN>/.claude/worktrees/automata-exact33/engine/build-x86/FreeCity4.dll --replace automata --scenarios maxisland-east-weburg --harness-root <MAIN> --store <MAIN>/build-runs/golden --from 2112-06-07 --months 1 --jobs 1`.
Used the worktree wrapper to execute the updated check.py with the shared main harness/store.
qcheck-1 printed `outside; first month 2112-06-30`, `exact: 0 of 1`, wall 76.161 s; all 25 differing records are labeled, none unowned. Full output: build-runs/qcheck-1.log.
Examples: 096E75F5 and 4C0DD224 are `automata?`; 2977AA47 is `occupants and props?`; 499B23FE is `class-store index`; 6A0F82B2 is `traffic?`.
The existing candidate's mismatch persists; labels are ownership leads, not causal proof. No additional game runs or shadow claims.
Earlier q.py launch failed before pytest (Git Bash CreateFileMapping Win32 error 5); the direct full pytest result above is authoritative.
An initial payload-decoding census was interrupted without results; the completed inventory-only census above supplies the reported counts.
Missing: no known reference record types. Questions: none. ABI exports and FC4Schedule files unchanged.
