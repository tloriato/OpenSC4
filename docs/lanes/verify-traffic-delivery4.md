| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| verify-traffic-delivery4: train73 traffic delivery diagnosis | Codex / `verify-traffic-delivery4` (engine lane; detached HEAD) | in progress | 1. Compare preserved train72/train73 evidence and engine changes. 2. Prove any correction against the public spec. 3. Full gate and unique diagnostic shadow through 2032. |

## Handoff
- Goal: explain train73's new state18-to15 traffic mismatch; train72 was exact. Do not weaken comparison, sampling, budgets, resync, or goldens.
- Reserved worktree reused at detached HEAD `1fd2aa6bdb1dbe7d1b94ab4a6a4839b90512302a`; no branch/ref changes authorized.
- Evidence is in this worktree's `build-runs/evidence-run4`, not the main checkout's build-runs.
- Done: initial handoff committed `b2e5abc2`; baseline workspace tests and clippy passed; CMake built 206 edges, full ctest 67/67 (`build-runs/fc4verify-ctest.log`).
- Done: `f9aaceaa..d8eb2cac` has zero traffic simulator/network/pathfinder/core/CAPI/glue/shadow changes; shared context fixtures and landvalue diagnostics changed. Same logged shadow keys and enabled replacement set; no claim of identical runtime inputs.
- Preserved train72 log SHA256 `6f40a91b6d00fe7e3af081a3b81b0582c868fbbf33508d24d2f847b2b3e08bd2`; train73 `54d5032562d073ce4c84b761aac48c763afaa6084c3878e5d62ba223dc97f98d`. Read-only summary script/JSON in `build-runs/traffic-delivery-evidence.*`.
- Counts: train72 monthly72/72 exact/resync0; train73 monthly71/72 exact plus differing day =71/73 exact result lines/resync1; no incomplete deliveries. First failure remains 2031-05-29, 408 differences.
- Proved witness: provider volumes changed across a suspended search can yield different trip-length bits from an atomic boundary replay, even with the same route. Quick gate PASS (2372 core tests); this is a possible mechanism, not proved train73 causality.
- TS68 appended to `docs/specs/questions/traffic-simulator.md`: missing safe native continuation/provider read-event observation contract. Native offset reads/hooks stopped pending that answer.
- Plugin diagnostic wording now distinguishes uncaptured RNG state from unreadable state and names native search/provider capture0; no algorithm, comparison, budget, resync or ABI changes.
- Done: diagnostic milestone committed `2c5dc0fa`; fmt absolute-path check PASS. Fullgate: workspace4239/0, clippy0 diagnostics, build4/4, ctest67/67; wrapper noted tree changed because TS68 prose was corrected during run, so repeat on stable committed tree.
- TS68 now correctly acknowledges section1 already publishes concrete/interface bases and TS66 publishes RNG layout. Missing facts are owned pathfinder continuations and native provider-read capture. No reason to invent that base contract.
- Outstanding game: `traffic-delivery4-diag-2032-20261022-a`, turbo through2032, thirty logged train73 shadow-on keys, only simulator replacement. Submitted once; `qwait.py` running in session77507 (no --fresh). DLL SHA256 `4438abb17c8703e4d35417d0e78c853110b5b6077c5ef87715a7bdd9a6c6f673`.
- Next: repeat fullgate on stable tree; wait for this unique diagnostic verdict, count exact/diff/resync honestly, report WIP. Do not infer recovery from a clean rerun.
- Causal confidence: high for replay limitation, unproved for train73; TS68 and actual native sequence remain blockers. No functional correction or coverage promotion.
