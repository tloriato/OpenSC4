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
- Next: quick build updated diagnostic wording, fmt check, unique own diagnostic game shadow through2032, final fullgate. Do not infer recovery from a clean rerun.
- No game request outstanding. Causal confidence: high for replay limitation, unproved for train73; expect WIP until TS68 answered and actual native sequence captured.
