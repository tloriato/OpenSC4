| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| verify-flora-records4 (final round 3/3) | Codex / verify-flora-records4, detached HEAD | wip: 25-case occupant verification exact; fresh-save reload stalls with probe off; final stable gate pending | Coordinator: investigate fresh-fixture core tick stall; no fourth occupant-family round or owner promotion. |

## Result

Final source is 29f66555 (substantive implementation 367e155e, committed before call30). Added twenty normal state0..4/orientation0..3 fixtures to the five preserved narrowing cases. Production Rust query methods now compute scalar, flag, highlight and removal results; visibility post=false and high-bit/nibble/removal-edge mutations are compared against native SDK calls. Positive QI crosses occupant/flora/serializable/canonical-IUnknown views, checks identity and independently counted references through destruction at zero. Negative QI tests valid null/sentinel output slots; failed output states are observations refused under FQ18. No null output address is called.

Six additive C ABI exports across this family (one new this round); ABI version/tables unchanged. No static-record, whole-owner, simulator/default promotion, RE, private field access, subagents, merges or pushes.

## Measured evidence

- Focused quick:3456 Rust tests passed, dependents/native build passed. Absolute-path fmt:2 files passed. Relative-path fmt failed; temporary signature-width formatting failure corrected; logs retained.
- First full attempt:526 CAPI tests passed/2 existing timing tests failed (slot42_callbacks_run_unlocked_across_threads, slot44_holds_the_lock_across_threads,300ms scheduling window); native build138/138 and CTest68/68 passed. Failure logs preserved as flora-r3-failed-workspace-test.log and flora-r3-attempt1-*.
- Serial retry (RUST_TEST_THREADS=1, no skips):4943/0, clippy0, CTest68/68 (1035.77s), qbuild PASS, but tree-changed marker from evidence commits. Final fixed-HEAD all pending; no stable-gate credit yet.
- Immutable game DLL:build-runs/dlls/vfocc-r3-games/FreeCity4.dll SHA256 a4579088af1b685bd30bd62d12104ec5e4dbb98021bf162a28098a6d1b2e1cf1. Passing retry gate's DLL hash matched.
- vfocc-r3-guard2032-20261022-a:clean/ref0 to2032-01-09; full SaveCity ok=1,11047ms on2031-01-06;8 passes/200 owned creations,95600 mixed assertions/0 differences.
- vfocc-r3-reload2032-20261022-a:STALLED/ref0 at2031-01-07, core fc4_sim_on_tick >10s, zero probes before termination. No reload credit.
- vfocc-r3-control2032-20261022-a:STALLED/ref0 at same date/core tick with probe disabled, zero probes. Fresh-save failure remains unresolved.
- vfocc-r3-knownreload2032-20261022-a:preserved round2 fixture, current immutable DLL, clean/ref0 to2032-01-11;2 passes/50 creations,23900 mixed/0 differences.
- vfocc-r3-knowncontrol2032-20261022-a:same preserved fixture/DLL, clean/ref0 to2032-01-04; probe/natural lines0.
- Fresh fixture SHA256 efb152e603bdb0ac760514891084e9ea76e9956f1cabdbf54a7ae9515b7ca14a; preserved fixture82de3ffe2ac3e7b11ec36ce04917203d1afd82ad6d8286a2d42ce1ea396d4390. Both uncommitted main build-runs/fixtures saves; staging hashes matched. No game request outstanding.

## Accounting and limits

Qualified guard+known reload total:10 passes,250 creations across25 unique inputs (not250 unique cases),119500 mixed assertions/0 differences;63500 production Rust query result comparisons,12750 QI assertions,21500 reference results,500 complete record comparisons/33000 bytes,17250 Read results,750 Write results. The log field algorithms means query result comparisons, not distinct algorithms: ten query selectors, repeatedly exercised. Position checks, Rust-only setup, stream errors and initial factory/view assertions remain in mixed totals.

21250 measured refusals:16500 short-read partial-state comparisons,500 bounds projections,4000 failed-QI output-state observations,250 null-output-address safety groups skipped.5500 undefined return channels ignored/count retained;0 fallbacks/static calls. All4000 failed-QI output values were unchanged (observation only). Natural snapshots:25827 visits/154962 read-only queries,invalid0,mutations0,computed credit0. Failed runs add no comparison credit. No shadow difference required a normal-speed comparison.

Prior round2 c5fa937b evidence remains historical:4941/clippy0/68;8100 mixed repetitions of five cases,3400 refusals,700 undefined; diagnostic585/0 separate. Prior FQ16 static refusal remains. Baseline all was launched first but source changed while it ran, so no stable final credit. Bash q.py E_ACCESSDENIED and detached commit packed-refs.lock permission messages are preserved; detached commits succeeded, shared refs untouched.

FQ18 appended to docs/specs/questions/flora.md; FQ16 callable gap and FQ17 partial-state/default/bounds/override gaps retained. Coordinator owns generated question index.

Precise unverified surface:AsPropertyHolder/holder QI; IsOccupantGroup/AddOccupantGroup/GetOccupantGroups numeric/name forms; GetLotTag/SetLotTag (lot-tag bytes only proved); Init/Shutdown/IsInitialized; PostOccupantMessage and post=true highlight/visibility; bounding-box/city-cell/manager-box get/set; GetPlaceableObject/GetOrCreatePlaceableObject/SetPlaceableObject; SetStaticFloraData. No nonempty holder/group or notification-lifetime fixture boundary was fabricated. Constructor defaults and partial Read state remain refused.

Evidence:build-runs/flora-r3-counts.json, preserved failure/gate logs, game-results/<labels>.{log,verdict}, immutable DLL. This closes the final existing occupant family round; no fourth or renamed continuation is proposed. WIP queue holds fresh-save acceptance for coordinator investigation outside occupant verification.
