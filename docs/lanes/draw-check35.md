| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| draw-check35 draw boundary checks (tools) | Codex / C:/Users/tlori/FreeCity4/.claude/worktrees/draw-check35 / detached HEAD, reserved draw-check35 | ready; commits d4d0a331c, 71d67fc81, 09ceabc06, a2e4c9088, cb2fa4934; ABI unchanged; pytest 417; cargo 8410; clippy 0; CTest 147/147; originals 7/7 ordinary/holes/reverse exact; ports 7/7 draw exact; shadow n/a; questions none | 1. Land tooling. 2. Use a candidate exporting reach for byte coverage credit. |

Seven draw rows use four existing suite cities, paused visible zoom 1–5/rotation 0,1 sweeps, and UI trees; suite.tsv unchanged.
Pinned draw references, reach selection (including required owners), exact run/each/credit comparisons and coverage scenarios are integrated.
Reference r5 DLL SHA-256 d79069d9f36e620be6bd946022176269a44641c13e1760e006959f886774fee6; schedule bde008a451b1c7fb; no repin/main merge needed.
Originals: 190 frames / 8,960,135 calls per corpus, all seven identical on independent ordinary, holes and reverse repeats.
Save-table basis: 108 rows, 104 errors, 3 differences, 1 exact (ports/paradis-ragako); no owner exact on its whole save cover.
Per-key draw verdict: ports reaches all seven rows and is exact on 7/7; first difference none. No whole-save-cover exactness claimed.
Candidate b42f4a2269fe7ea55e9a6a612b8fb5755936461faa2e4279e954e74086869cd3 exported reach on 0/7 captures; byte credit remains withheld by the existing coverage rule.
Initial candidate-counter errors are preserved; after correcting comparison parity with saves, all seven complete retained streams compared exactly. References still require counters.
Incomplete 60-second Bevopolis and Konradshohe-frames captures were watchdog cutoffs; corrected rows use 170 seconds and new reference keys; failed attempts count as neither equality nor difference.
Ran `python -m pytest tools/tests -q`: `417 passed in 244.29s`; focused draw tests: `23 passed in 0.44s`.
Ran `python <MAIN>/.claude/lane-bin/qbuild.py --wt <WT> all` once: `test: passed=8410 failed=0 ignored=0`, clippy diagnostics=0, build PASS 3/3, CTest PASS 147/147, `qbuild: PASS`.
Ran `python build-runs/draw-round3.py` and `draw-konrad-fixed.py`: pinned references and independent audits `recorded`/`reproduced`; the first collector was stopped after audits to suppress its superseded serial measurement.
Ran `check.py run --dll <WT>/engine/build-x86/FreeCity4.dll --replace each --keys ports --scenarios draw --store <MAIN>/build-runs/golden --jobs 7`: 7 complete captures, initial missing-counter errors retained.
Ran `python build-runs/compare-retained-ports.py` (6 rows), then with corrected collector table (1 row): `6 exact`, `1 exact`; no duplicate game launches for recovery.
Ran `python build-runs/finalize-draw-results.py`: `7/7 ordinary, holes and reverse EXACT`; `ports: 7/7 draw EXACT; first difference: none`; `candidate reach: 0 / 7; coverage credit withheld`.
An initial q.py pytest transport failed before tests (Bash CreateFileMapping error 5); direct pytest passed. A missing-labelled-file drawcheck probe was INVALID; direct exports were subsequently validated.
Game labels (all ports draw exact): golden-274679abaf4c42919a25dae4d27f5db8, golden-69db93b64f13419196cada1cbc1d6f75, golden-4b46ba3337244e81ba51520356466360, golden-803c9abe31044d3ab8c9dd6f0012db53, golden-2d51b0fbcefd4d7991eadf9c99182d3c, golden-45d358b436d049f2912f5bd389331a6a, golden-8283f22fbafc4691b3b9f9067ebfd787.
Evidence: build-runs/draw-measure-r3/final.json, report.json, ports-before-correction.tsv, ports-compared.tsv, ports-corrected.tsv; build-runs/draw-measure-r3-fixed/; shared golden/<key>/baseline.json and repeat-*.json; shared draw-owner-check.tsv has all seven final verdicts.
Details and current reference key prefixes: [draw-check35-results.md](../draw-check35-results.md).
Schedule request: none. Questions: none. ABI exports, FC4_ABI_VERSION and FC4Schedule.* unchanged; no seam reservation.
