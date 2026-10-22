| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| verify-timers4 (independent native cRZTimer verification) | `verify-timers4` / detached lane worktree (engine lane; docs-only result) | Parked TM4; question committed c72009d5. Cargo 4938/0, clippy 0 diagnostics, C++ 208/208, ctest 68/68. No engine/ABI changes; no native comparisons or game runs. | (1) Publish existing safe typed native boundary facts requested by TM4. (2) Only then implement independent stopped-fixture verification and required game fixture/control checks. |

## Result

Parked on the missing published safe native boundary, as required by the task's stop condition. No engine code, plugin configuration, ABI export, or verification counter changed. `cRZCallbackTimer` remains excluded. Commit `c72009d5` records the boundary audit and TM4.

## Published boundary audit

Read `docs/specs/timers.md` sections 1.1-1.5 and `engine/core/src/timers/rz_timer.rs`. The actual core file is `rz_timer.rs`, rather than the brief's doubled `.rs.rs` path. Searched the published `docs/abi/` files for `cRZTimer` and the constructor/method addresses, and the public SDK for the class/method declarations. No corresponding typed native contracts were present.

The spec publishes behavior, 24-byte state offsets, addresses, EAX for the running query, and x87/pop-zero facts for the static factor getters. Those partial facts do not establish the constructor, all method conventions/argument cleanup/return channels, or safe storage and lifetime obligations. The general calling-convention rule in `docs/abi/direct-calls.md` applies to the functions listed there; it cannot establish a timer contract absent from that document. The Rust struct and Rust tests cannot supply an independent native ABI or oracle.

TM4 in `docs/specs/questions/timers.md` requests the missing typed constructor/method/output contracts, allocation/alignment/padding/lifetime facts, permission for explicit stopped-fixture state, and calibration/factor readiness. It requests publication of existing interoperability facts, with no new RE round. The question index was left unchanged: this lane may edit only the question file under `docs/specs/` and must not run `tools/` scripts. Coordinator regeneration remains necessary.

## Verification evidence and limits

- Native calls: 0. Native comparisons: 0/0; no exactness percentage or measured differences.
- Refusal occurred at the missing-boundary audit, before constructing or enumerating executable native cases. No per-case refusal denominator was manufactured.
- Fallbacks: 0; resynchronizations: 0. No Rust self-oracle, substituted return/output channel, clock detour, original clock mutation, or tolerance claim.
- Stopped native-constructed arithmetic, natural callers, and running-clock behavior are all unverified. No fixture/control game runs were submitted; no labels or requests are outstanding. There is no measured game verdict, shutdown result, or reference-warning count.

## Gate

The mandatory `qbuild.py --wt <WT> all` run started before the audit and completed before this report. With no engine changes, this single run is both the baseline and the final gate for the unchanged engine tree; no redundant gate was run after documentation changes.

- Cargo: exit 0; 4938 passed, 0 failed, 0 ignored (`build-runs/qbuild-test.log`).
- Clippy with warnings denied: exit 0; 0 diagnostics (`build-runs/qbuild-clippy.log`).
- C++ build: PASS 208/208 edges, 79.1 s (`build-runs/fc4verify-build.log`).
- Full CTest: PASS 68/68, 0 failed, 119.7 s (`build-runs/fc4verify-ctest.log`). Overall `fc4verify` PASS, 202.9 s; `qbuild` PASS.
- No Rust files changed, so no format check or new focused tests were applicable. Gate success supplies no independent timer verification credit.

## Next steps

Keep the lane parked until TM4's existing facts are published. Only then construct isolated original stopped timers with explicit identical state, capture actual QPC frequency and native factors, compare genuine full-width integer/float output channels, and report a complete comparison/refusal/fallback denominator. Natural-caller and running-clock coverage must remain separate. Any subsequent plugin implementation requires the focused checks, stable full gate, and unique immutable fixture/control game runs through 2032 with full shutdown and zero reference warnings specified by the task.
