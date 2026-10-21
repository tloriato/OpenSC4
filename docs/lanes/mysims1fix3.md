# mysims1fix3 — mysims shadow validation repair

| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| mysims1fix3 (MySims shadow and validation repair) | `mysims1fix3` / (engine lane) | merged train19 (832c7238e0): cargo 3576/0, clippy clean, ctest 62/62; run 222 clean to 2032, mysims 73/73 exact, ref_warnings=0; regress mysims1fix3-r3 PASS, 0 problems (3 worse vs train17) | Train19 gate and combined regress PASS, mysims 73/73; future engine work for MS3/MS5/MS7 |

## Report

Continues mysims1 at 53403af4. The original shadow, registrations and regression-list addition are included by ancestry. 93dcf13a pairs the AppCity QueryInterface acquisition with the ref tracker. 6679c3e3 includes advice subclasses in the shadow core and removes the synthetic CityEstablished delivery on load. 09b3e722 defers the shadow PostCityInit until the game's original load dispatch finishes; this corrects the remaining load-month counter mismatch.

At 09b3e722, qbuild all passed: cargo 3576/0, clippy clean, CMake build and ctest 62/62. mysims1fix3-sh-6 (DLL hash 11b6356c) reached 2032-01-07 clean, with load mismatches=0, months=73 clean=73 last_mismatches=0, ref_warnings=0. Run 222 records the measured comparison and its excluded fields; this is a shadow, not a MySims replacement.

The coordinator's full regress mysims1fix3-r3 passed both runs to 2032 with zero problems and zero ref warnings. Against train17, netmgr 71/73 versus 73/73, landvalue 71/72 versus 72/72, and aura 70/71 versus 71/71 were flagged worse. These deltas are retained, not described as exact. This first regression used main's pre-MySims shadow list; the train must exercise the new mysims_shadow entry added separately to main in 370f438d (tool tests 61/61).

The CLI killed the worker after its ten-minute background wait ceiling, leaving the last timing edit uncommitted. The coordinator reviewed, committed and validated that edit without starting another lane round. Future launches disable that CLI ceiling; no lane guard state was reset or bypassed. Main was not merged into this branch. No C ABI exports changed.

MS1-MS7 were answered separately by re-answers35; MS3/MS5/MS7 have future engine implications. The 73/73 result applies to the fields and city actually exercised, not to unimplemented answers or the exclusions recorded in run 222.
