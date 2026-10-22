| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| verify-advice-subject4: independent native AdviceSubject verification | Codex / `verify-advice-subject4` (engine lane, detached HEAD) | in progress: round 2, private clone transport authorized | Implement actual Rust clone transport, focused ownership test, stable gate, immutable probe controls. |

## Handoff
- Round 2 goal: implement actual production clone transport, then independent native subject measurement.
- Prior report and prototype read; private export/local declarations authorized, ABI tables/version unchanged.
- Done: private owned Obj::Subject clone transport invokes actual Rust clone_subject; production front rewired. Four private single-field setters preserve Rust returns and ownership effects (no aggregate-set replay).
- Focused ownership/independence CAPI test added. Opt-in verifier wired; fullword native results logged and clone refusal counted separately.
- Milestone 93ac8e92: production transport/front and initial focused test. Additional test checks null/wrong type, scalar no-reference effects, fullword fields, clone's one AddRef and replacement AddRef-before-Release.
- Baseline tests completed, clippy compiling; serial gate helper will wait for baseline PID 29392, run quick then final all, freeze DLL only on success. Logs remain in build-runs; no concurrent builds.
- Baseline cargo measured 5039 passed/0 failed/0 ignored across 21 result lines; this is NOT a full gate. Clippy still live. Stable focused/full gates pending.
- Baseline clippy subsequently passed (`Finished dev` in 10m19s); CMake is compiling fc4-core release/i686. No DLL/gate claimed yet. Helper PID 22184 remains waiting and will run serially. Initial native AddRef/Release pair uses refaudit macros.
- Baseline final: qbuild FAIL; Rust 5039/0/0, clippy PASS, CMake 54/170 edges FAIL (12 undefined FC4_LOG errors in resumed verifier), CTest skipped. Fixed all verifier calls to the actual fc4::Log API. Helper has advanced to quick; final all still follows only successful quick. Baseline is not discarded or called PASS.
- Code tip 3a2bd237 (20f67574 substantive test/verifier; 93ac8e92 production transport). Helper session 67843; baseline session 79550. The already-running helper loaded its original code before snapshot support was added; do not assume baseline/quick archives exist.
- Prepared (not queued): advice-r2-{control,probe,current40}.template, immutable DLL destination build-runs/immutable-subject-r2/FreeCity4.dll. Unique labels end 20261022a. Probe-OFF control must pass first; only then matched probe-ON, then literal CURRENT40.
- Supported script now saves at 2031-12-01 with before/after marks. No supplied persisted reload fixture was found in permitted inputs; reload remains unmeasured. No game request outstanding.
- Next: collect baseline all (session 79550, live qbuild PID 29392 at inspection), then quick/focused checks and ONE final stable all. No overlap.
- fmt absolute paths required; check currently reports pre-existing formatting differences (advisors.rs weak handles and tests); no mass format performed.
- Old log timestamps alone do not establish a live check or PASS; WMI process inspection denied.
- No game request outstanding. No native measurements yet.
- Manifest: `C:/Users/tlori/FreeCity4/build-runs/run4-verify-advice-subject4-r2-manifest.json`.

## Report
- Goal: compare independently created native AdviceSubject fixtures against actual production Rust methods, without advisor activation or replacement registration.
- Done: reserved detached worktree reused; initial handoff a7479a5c. Baseline all invoked (cargo still compiling). Substantive opt-in verifier written before call 30.
- Next: resolve AD18 before invoking any native comparison prototype. Then collect existing builds (do not launch another concurrent build), run a final stable all gate, and complete the required own opt-in and immutable clock24/current40 controls through 2032/ref0 with normal save/shutdown and supported reload.
- Focused transport checks added to existing vtable_layout_test: all 14 native slots, argument pops/x87, and full EAX words 0, 0x12345600, highbit, MAX at both data-setter slots.
- Gate pending: baseline all started about 21:11 local; quick started about 21:30. Cargo logs show compiling core/conformance/formats/capi; CMake build log shows Rust core compiling. No fresh FreeCity4.dll yet; no verdict claimed.
- Baseline command has exec session 62542; quick command is orchestration cell 26. Do not start another concurrent build. Full logs are build-runs/qbuild-test.log and fc4verify-build.log.
- AD17 filed: spec CreateInstance spelling versus public SDK GetClassObject class-instantiation API. Only the documented public operation is used, without fallback.
- Production transport: cFC4AdviceSubject fronts invoke CAPI get/set; aggregate setter invokes actual Rust methods. Clone front uses a new Rust subject plus setters, not Rust clone_subject directly; this limitation must remain explicit.
- Constraints: one round; no subagents; no shared ABI/config/default/tools edits; serialization only with already published typed endpoints.
- Manifest: `C:/Users/tlori/FreeCity4/build-runs/run4-verify-advice-subject4-r1-manifest.json`.
- No game requests outstanding.
- Latest review: Clone failure must be a difference/refusal even if both fixtures fail. Final reference counts and controlled fixture creator references are now checked through zero.
- Cross-object ownership traces now use independent shared event logs for native and core fixtures, so replacement AddRef-before-Release order is observable across two different objects. Clone location/data setter reference effects are compared individually, with every raw difference preserved.
- Partial baseline cargo log at 21:43 local: **5026 passed, 0 failed, 0 ignored across 11 result lines**; tests still running. This is not a finished gate. Clippy and full CTest are unmeasured. CMake's Rust core compilation remains pending, no DLL; its log reports sccache server shutdown and local compilation fallback. No Rust files were changed; rustfmt is n/a.
- Expected successful native per-slot counts (3..13): 6,23,22,22,8,21,23,6,23,6,1. These are design counts only, NOT measured native evidence.
- Save script prepared at build-runs/advice-subject-save.txt. No queue request exists. Exact immutable control profile contents are not in the allowed docs read so far; do not invent them from subsystem enumeration.
- Native request template (not submitted): build-runs/advice-subject-native.template. Choose a fresh unique label before submitting, wait without qwait as instructed. If factory/required ownership refuses, file precise endpoint evidence and stop without fallback.

### Blocking endpoint and bounded source

AD18 is a required transport gap, not evidence that the original native factory refuses. The brief's asserted existing subject-clone C API is absent: `fc4.h` has subject get/set and **advice** clone only. Rust `AdviceSubject::clone_subject` exists, but `cFC4AdviceSubject::Clone` creates a fresh subject and calls the aggregate setter. Comparing that production front would not directly verify the required Rust clone method or its ownership effects. Adding an export is outside this lane's permitted shared ABI scope.

The simulator's opt-in hook therefore logs **REFUSED AD18, native_comparisons=0, core_comparisons=0, unverified=1** and does not invoke the production-front prototype. Default behavior remains off. No native object, advisor factory, city advisor graph, provider/Lua algorithm or private global is touched by this hook. The larger comparison routine is explicitly named `CompareProductionFrontsPrototype` and is unwired, preserving reviewable work without substituting it for the required comparison.

Source milestones: initial handoff `a7479a5c`; opt-in prototype `c13591cf`; native slot/return transport checks `5d735b1e`; Clone refusal and complete fixture cleanup `f4528dcb`; cross-object ownership traces `cba5d8c8`. The final blocker commit retains all source and questions. No C ABI exports, ABI version, maps, defaults, shared configuration or tools were changed.

### Measurement limits

- Actual native comparisons for **every slot 3–13: zero**. Factory availability, QI/reference balance, default fields, clamps, data returns, object traces, Clone and cleanup are all **unverified**. The design counts above are not measurements.
- No game request was submitted; there are no run labels, verdicts, reference-warning counts, save/reload observations or normal-speed differences to report. No clean 0/0 run is claimed as verification.
- Serialization is deliberately withheld: no independent fully typed native stream/reference endpoint was established. Nonfinite locations and failed-QI output semantics are outside the prototype's scope. Direct Rust setter return values are hidden by the existing aggregate setter/front transport.
- Questions **AD17 and AD18** are appended to `docs/specs/questions/advisors.md`. AD11 and AD16 remain open and unchanged. No spec behavior was rewritten.
- All and quick were invoked only through qbuild. Existing background sessions: all `62542`, quick `97587`. Logs remain in this worktree's `build-runs/`. A passing final stable gate was not obtained. Builds may continue after this report; preserve their logs and do not infer completion from the partial counts.
- **Not enqueued:** the user permits queuing only bounded tested source; this WIP has no completed gate or native evidence. The supervisor may publish the detached commits to the reserved branch, but this is not ready to land.
