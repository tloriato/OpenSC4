## 0. Current state (2026-10-21, run 3 stopped at 8/8 rounds)

**STOP is set: `run3 round limit reached (8/8); land committed ready work only`.** No lane is running,
the merge queue is empty, and main is pushed. No further lane rounds are authorized in this run.
The guard is authoritative; no parking, watchdog or context state was cleared or bypassed.

- **Train19 landed `832c7238e0`:** MySims shadow plus its lifecycle/ref-tracking fixes, and MS1-MS7 RE answers.
  Gate PASS (clippy clean, build, ctest 62/62 including cargo_test); full regress PASS, both runs clean to 2032,
  zero problems and ref_warnings=0. MySims is exact **73/73** in the combined shadow run (run 223; standalone
  run 222 also 73/73). The recorded exclusions still apply; the new MS3/MS5/MS7 engine changes are future work.
- **Train20 landed `37544298d4`:** PL19 answered in prop-lotconfig.md, docs only; no engine change or game claim.
  Slot 9 AddRefs every returned element on every call, including cache hits, on the pointer inserted in the set.
  The PL18 second add is wrong. This does not by itself explain the measured two-adds/drain contradiction:
  inspect cache hits and pointer identity, then validate against glue21's per-op trace before claiming a fix.

**Coverage:** 36.8% specified / 28.3% implemented / 12.1% verified / 15.9% replaced, from train19.
The current regress is authoritative. MySim and MyStreetSim now map to mysims_shadow; untested classes do not.
The earlier lane-DLL regress passed with three worse comparisons versus train17 (netmgr, landvalue, aura);
the full train regression including MySims passed and its own shadow column is in build-runs/regress-train19.out.

**WIP preserved and pushed; none is queued for landing:**

| Branch | Tip | State / next |
|---|---|---|
| `glue25fix3` (continues `glue25`) | `209ff7fc` | gate PASS, ctest 62/62. enable=occmgr game g1 corrupt at 2028-04-01, ref_warnings=0. g2 clean to 2032 was shadow-only, not a replacement pass. Diagnose g1, then replacement game and regress with occmgr. After it lands, add occmgr to tools/regress.ps1 $Replace and the cSC4OccupantManager replace key in tools/coverage-map.tsv. |
| `glue-terrain3` | `d112c328` | round 2 committed view/raw-record glue and terrain Load (c1b06da3, 25a4ec76); the no-code parking condition did not apply. Full gate PASS, ctest 62/62; gt3-repl-4 corrupt before a game date, ref_warnings=0. Diagnose crash; finish Save and TR33-TR37. Its worktree lane file records the exact implementation plan. |
| `glue21` | `405da5f0` | parked implementation. PL19's RE contract is now on main; apply and trace-validate the per-call slot-9 ownership rule before a new game run. No implementation round remained under the 8-round cap. |
| `glue26` | `fca64f23` | skipped as requested; original glue25 branch also retained. |

**Operator notes for resuming:**
- The first three continuation workflows were killed by the CLI's default ten-minute background wait ceiling,
  without final lane reports. Their code was preserved. The coordinator committed and validated the last MySims
  timing edit; terrain/occmgr failures were not relabeled as ready. Future CLI hosts set
  `CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS=0` and wait for TaskOutput; keep the guarded fc4-lane workflow.
- The old game queue's child PowerShell could not load Get-FileHash. It was restarted from Windows PowerShell
  with the working module path (queue pid 22296, six slots); real game verdicts then worked. No harness code changed.
- REA was reconnected before each RE spawn; check and preflight both said GO. Both RE lanes used export bytes.
  A later RE spawn still needs a fresh GO; do not treat an earlier connection as persistent.
- Tool changes were separate commits (370f438d regression list, a9490b4c coverage map), each with tools/tests
  passing 61/61. ABI reconciliation and glue26 were not worked on.
- After compaction/restart, re-read this section and run `python tools/lanes.py guard` before any spawn.
  Do not clear STOP or unpark a lane without the authorization/cause fix required by the user's run rules.

The superseded start note is preserved verbatim in [history/handoff-run3-start-2026-10-21.md](history/handoff-run3-start-2026-10-21.md).

<!-- BEGIN GENERATED: latest-run -->
- **Latest in-game run:** 223 (train19 PASS, MySims shadow exact 73/73 alongside the full regression, ref_warnings=0); 222 (MySims shadow exact 73/73 through 2032, including load, ref_warnings=0); 221 (the network-tool replacement places roads and streets in the real game (inserts=8..69, valid=solved, failure 0), 20/20 scripted placements ok, city runs to 2032 clean with ref_warnings=0.); 216 (component 53 / 11 save round trip, record53 equal at load and save); 215 (devperf2 shadows over the replacement exact after the cell-walk host change). See `docs/manual-checks.md`.
<!-- END GENERATED -->

