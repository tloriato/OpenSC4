| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| verify-traffic-delivery4: train73 traffic delivery diagnosis | Codex / `verify-traffic-delivery4` (engine lane; detached HEAD) | WIP: causal attribution blocked on TS68; diagnostic witness and honest capture-scope log committed; workspace4239/0, clippy clean, ctest67/67; own2032 diagnostic clean/ref0, traffic72/72 exact, diff0/resync0; no ABI change or coverage recovery | 1. Resolve TS68's native continuation/provider-read observation contract. 2. Retain train73 as failing evidence; require actual native-input evidence before a causal fix. No fourth verification round requested. |

## Result and causal confidence

The new train73 failure remains unresolved. No functional traffic correction is justified by the preserved evidence. High confidence: the direct traffic implementation did not change between the two train builds, and a delivery-boundary replay can diverge from a retained native search when provider values change across suspension. Unproved: that this mechanism caused train73. The clean diagnostic run below provides no recovery or replacement credit.

The public-spec path to native continuation/provider-read observation stops at TS68. Section 1 already supplies the native/interface base identities and TS66 supplies RNG layout; those facts are not missing. The missing observation contract concerns the owned pathfinders, retained continuation/node state and native provider-read events. This lane installed no speculative native hooks or offset reads, and made no seed writes.

## Preserved failure and build comparison

Evidence stays untouched in `build-runs/evidence-run4/`:

| Evidence | SHA256 | Traffic results |
|---|---|---|
| `regress-train72-shadows.log` (5658462 bytes) | `6f40a91b6d00fe7e3af081a3b81b0582c868fbbf33508d24d2f847b2b3e08bd2` | monthly72/72 exact; diff0, resync0, incomplete0 |
| `regress-train73-shadows.log` (5650638 bytes) | `54d5032562d073ce4c84b761aac48c763afaa6084c3878e5d62ba223dc97f98d` | monthly71/72 exact plus one differing delivery line:71/73 exact result lines; resync1, incomplete0 |

Train73 line21664 is the first failure: 2031-05-29 day, state18->15, progress0, units120, 408 mismatches. Edge densities differ in559/479920 bytes; the car-length sum and multiple lot destinations, lengths, counts and trip-list splits differ. The 2031-06-02 summary reports two differing deliveries: four record-field differences,21 trip-list differences,385 grid-tract differences, zero holder-property differences. It contains one known suspended-search marker. This is substantive routing/output divergence, not the old TS67 phase-position-only failure. Suspended-search markers occur in one train72 monthly summary and six train73 summaries; that correlation does not identify the failing search.

The compared train resolution commits are `f9aaceaa` (72) and `d8eb2cac` (73). Their engine diff leaves traffic simulator/network/pathfinder core, traffic C API/host layout, traffic glue/shadow, simulator delivery glue and host-grid code unchanged. Actual logs identify ABI82 / staged DLL prefix `79f101c0` versus ABI83 / `ab2b3352`. Both enable only the simulator replacement and log the same thirty shadow-on keys. The relevant shared changes add the opt-in span8888 bridge/context fixture and change landvalue diagnostic wording; the new budget-window/terrain modules do not alter traffic code. Train73 explicitly logs span8888 off, and its bridge startup returns immediately unless enabled. There is no evidence attributing the mismatch to span8888. Different builds can perturb wall-clock slicing without a traffic algorithm delta; this is not a causal finding.

The preserved logs contain no native per-search/provider trace and cannot prove identical traffic inputs, RNG state or deadline schedules across processes. Public section4.15 says routing itself has no random draws. The shadow does not capture native RNG state; lack of an interface getter does not make that state unreadable (TS66).

## Changes and regression

`engine/core/src/pathfinder/tests.rs::resumed_search_and_atomic_replay_can_read_different_volume_histories` is a deterministic witness using pathfinder section4.4: suspend after the first expansion, change provider volumes between calls, resume, then compare with a fresh unlimited search on the current boundary inputs. Both routes agree; trip-length bits differ. The pre-existing constant-input suspension test remains in place. This proves a replay precondition matters, not that native train73 followed this sequence.

`engine/plugin/FC4TrafficShadow.cpp` corrects the RNG diagnostic from "not readable" to "state is not captured" and logs `native_search_capture=0 native_provider_read_capture=0 native_rng_capture=0`, `pre-image=delivery-boundary`, `replay_search=atomic-unlimited`. The shadow snapshots trips/capacity properties before native delivery, runs after it to the recorded stop, and does not capture retained native search nodes or individual provider reads. No goldens, sampling, dates, budgets, comparison/resync acceptance, simulation algorithms or C ABI exports changed. No tools changed.

TS68 is appended to `docs/specs/questions/traffic-simulator.md`, with hashes, exact failure details and the bounded witness. No generated question index was edited or forbidden tools run.

## Validation and own diagnostic game

Baseline `qbuild.py all` passed workspace tests/clippy and ctest67/67. Quick after the witness passed2372 core tests and compiled dependents. Rustfmt check passed using the absolute touched-file path. A fullgate passed while TS68 prose changed; it was repeated on stable committed `93814d58`: workspace4239 passed/0 failed, clippy0 diagnostics, build2/2, full ctest67/67, qbuild PASS with no tree-changed qualifier. Later tracked changes are this report only. Logs: `build-runs/qbuild-test.log`, `qbuild-clippy.log`, `fc4verify-build.log`, `fc4verify-ctest.log`.

Unique own run `traffic-delivery4-diag-2032-20261022-a` was submitted once and awaited by one qwait process without `--fresh`; the completed verdict was retained. It used the thirty shadow-on keys recovered from the old startup logs, `enable=simulator`, Konradshohe, turbo through2032. This is a current-main-descendant diagnostic, not a reconstruction of train73's exact native input stream.

- Verdict clean; end2032-01-01; ref_warnings0; foreign exceptions0; stalls0; RTC0; overruns0;503.641s real.
- Traffic monthly summaries72/72 exact; differing summaries0; differing delivery lines0; resync0; incomplete0. Final summary is labelled2032-01-02 by the existing shadow.
- DLL SHA256 `4438abb17c8703e4d35417d0e78c853110b5b6077c5ef87715a7bdd9a6c6f673`; diagnostic log SHA256 `a84179a42de08fba0fcf0f7c4b4241cf59066b397ee0b88c9be78ce274649ec0`.
- Request/verdict/log under this worktree's `build-runs/game-queue/done/` and `build-runs/game-results/`; no request remains outstanding. Read-only evidence summary/script: `build-runs/traffic-delivery-evidence.json` and `.py`.

## Commits and remaining blockers

`b2e5abc2` initializes the handoff; early diagnostic milestone `2c5dc0fa` adds the witness, capture-scope log and TS68; `93814d58` records the request and clarifies existing base/RNG facts; `6b045d61` records the stable fullgate. Worktree remains detached as reserved; no branch switch, merge, push, restart or subagent was used.

Coordinator: retain WIP and the original train73 failure. Route TS68 to the spec writers. Actual failing native continuation/provider evidence is required before a causal fix or any traffic coverage/replacement promotion. This final third verification round requests no fourth round.
