| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| verify-advice-subject4: independent native AdviceSubject verification | Codex / `verify-advice-subject4` (engine lane, detached HEAD) | in progress: corrected full gate PASS; immutable DLL/PDB verified | Submit OFF control, then matched ON only if OFF passes all required lifecycle checks. |

## Handoff
- Final round 3/3: reconcile completed gate, then bounded independent native/production Rust subject measurement; no further family round or promotion.
- Current source HEAD b1463f3b4ba9af586cc4bbc28348b9f19097426a; tracked engine clean. HEAD:engine = 2993f562:engine = 6a31aef7dd6bbcc2361c9ed2fdf96a15cc57a4c1. No engine changes this round.
- Process audit 2026-10-22T23:07:45.820240Z: 112 checker/shell processes inspected by actual creation/parent/command metadata; no unreadable command lines. Only lane orchestration service and audit shell attributable to this lane; no live checker/compiler/output worker. WMI denied, NtQueryInformationProcess metadata succeeded. No old terminal/session handle used.
- Corrected qbuild all completed PASS: 21 Rust result lines total 5040/0/0; clippy PASS; native build 4/4; CTest 68/68, 429.50s. Actual ret_channel_check 104.62s; abi_conformance_test 137.26s.
- Outer gate start/terminal exit timestamp was not durably recorded; do not invent it. Corrected source committed 22:27:07Z; test log finished 22:34:43.475464Z, native link 22:34:49.903834Z. CTest LastTest.log records start 23:35/end 23:42 BST (22:35/22:42Z); final raw CTest mtime 22:42:11.021649Z. Immutable DLL creation 22:42:12.556Z is after PASS and reachable in unchanged retry-subject-gate.py only after qbuild return 0. This is durable exit-zero evidence, not a claim of a retained terminal exit line.
- Artifact DLL SHA256 b3eb4974e01da2a7f47c67a95abf3ec31bd076b887f919b91ce324c5230e8a36; matching PDB e69e7331466b74dcfa42bc83b65ccd5fbac7558f27ffdca5b21d0424584cf1e7. DLL equals current engine/build-x86 DLL; PDB copied once from engine/build-x86/FreeCity4.pdb into build-runs/immutable-subject-r2/FreeCity4.pdb after PASS. Existing immutable DLL never overwritten. Source mtimes both 22:34:49.903834Z. Own DLL RSDS GUID+age matches PDB stream 1: 75e116e66de0a14f85c9487e9395f36804000000.
- Corrected logs under build-runs/: qbuild-test.log SHA256 ff02b5dbfb5cbbfafb2907a0cf101f44f5b960f966688ed064440fea3dde113b; qbuild-clippy.log 8c005dc0ffd2eaf0d59e0a8c7ce6953f2267d09e98b8577f335847941338fa11.
- Native/log hashes: fc4verify-configure.log e9b664a1986dac9e3cd5239498d406dc07db6bff57f0d527acb57b297fb3041c; fc4verify-build.log 8735c7f33555e211a97c838858ada4a23ce32345bc1a9a604810e252125a0353; fc4verify-ctest-list.log eb5d2e3359817fe330bf186bdcd989166f391baf2c91344d1f15b9d4c45c6e80; fc4verify-ctest.log dd5a607f26d16e06eb787b8aa9283b2a63f2f262a6a4d8f313ef37d2bc9c16f6.
- Provenance helper retry-subject-gate.py SHA256 957acdf05d82353a38d36c2c8217588ea81ce64b1aa387ab076029c203f620f6; CTest LastTest.log 70d6ca835e845e7b9f2518d6cbb6eaa00fd1b90afeb773fec3e3bcde56f1a034. Hash/count/RSDS verification helper: build-runs/reconcile-gate-r3.py (exit 0).
- Earlier logger compile failure, cached 1/4 rejection, and actual 67/68 return-channel failure remain failures (below; build-runs/failed-all-r2 and failed-ret-r2 retained). Two unchecked native SetLocation/SetObject AL calls required published [abi-ret] annotations in 2993f562; no game flakiness or all-green baseline claimed.
- Existing vas4-r2 templates remain unsubmitted: own game-queue/results directories absent at first inspection. Next use unique vas4-r3 labels. No game request outstanding; actual native measurements still unmeasured.
- Next exact step: finish reading published subject boundary/verifier, submit immutable OFF (empty enable/disable budget, Konradshohe, turbo1, seconds900/end2032, explicit timeout, full-save script), wait using existing wait-advice-verdict.ps1; only after full clean OFF submit matched ON, then literal CURRENT40. No builds while requests queued/running.
- Five private additive C exports: ABI=true; public ABI tables/version unchanged. AD17 public typed GetClassObject only; AD18 actual production clone transport present. No new question IDs. AD11/AD16, serialization and reload remain unmeasured/withheld.
- Current manifest: C:/Users/tlori/FreeCity4/build-runs/run4-verify-advice-subject4-r3-manifest.json. Documentation-only commits preserve engine/artifact correspondence.

## Round 2 handoff (historical; superseded by current evidence above)
- Goal: actual production Rust clone transport, then bounded independent native subject measurement.
- 93ac8e92: private owned Obj::Subject clone calls actual clone_subject; C++ production front rewired, four single-field transports preserve Rust returns/ownership. Public ABI tables/version unchanged; five private exports added.
- 20f67574/3a2bd237: focused test covers null/wrong type, distinct identity/equal fields, scalar no-ref effects, fullword fields, exactly one clone AddRef, replacement AddRef-before-Release, independent mutation/destruction and balanced final holds.
- 47063b57: fixed resumed verifier's undefined FC4_LOG calls to actual fc4::Log. Opt-in default OFF; native fullwords logged, clone failures counted, serialization/AD11/AD16/owner promotion withheld.
- Baseline all FAIL: cargo 5039/0/0, clippy PASS; CMake 54/170, 12 undefined logger errors; CTest skipped. Preserved as failure.
- Quick PASS: i686 CAPI 533/0/0 including clone test; build 119/119. Tree changed due logger fix during quick; no complete stable gate claimed.
- First stable all FAIL: cargo 5040/0/0, clippy PASS; cached Ninja progress 1/4 rejected, CTest skipped. Logs archived build-runs/failed-all-r2.
- Retry all completed FAIL: cargo 5040/0/0, clippy PASS, build 4/4 PASS after rebuilding ONE generated simulator object; CTest **67/68**, only ret_channel_check failed. Archived build-runs/failed-ret-r2. Original terminal 40624/helper 37956 finished; do NOT collect them as live checks.
- Source 2993f562 adds [abi-ret] comments for published advisors 12.4 AL returns on native clone SetLocation/SetObject. No signatures/ABI tables changed. Corrected run has ret_channel_check PASS (104.62s), abi_conformance PASS (137.26s), last progress 20/68.
- NEXT EXACT STEP: collect genuinely live corrected gate (terminal 5607, build-runs/retry-subject-gate.py 37956; already advanced past its wait, build 4/4 PASS). Inspect live matching processes before collecting; NEVER launch an overlap. Corrected CTest is still running at last inspection.
- Helper archives old failure, rebuilds only generated simulator object, runs full qbuild all, freezes DLL ONLY on PASS and prints gated source/hash. At last inspection NO immutable DLL existed. Destination build-runs/immutable-subject-r2/FreeCity4.dll. Verify complete all result/hash before any game request.
- Context nearly exhausted (147524/150000 at last guard). No merge queue written. Native measured comparisons remain 0/unverified; no game requests outstanding.
- Prepared advice-r2-{control,probe,current40}.template. Unique labels vas4-r2-{native-control,native-probe,current40}-20261022a remain UNSUBMITTED. Control OFF first must pass clean/2032/fullsave/shutdown/ref0; then same DLL/settings ON (only label/probe differ); then literal CURRENT40 (40 explicit + 2 defaults).
- Script advice-subject-save.txt: 2031-12-01 mark/save, 2031-12-02 mark; supported published actions. No permitted supplied persisted reload fixture; reload unmeasured.
- fmt checked absolute touched paths: remaining 206 diff lines are pre-existing formatting; no added-line diffs or mass format. WMI denied; inspect-checks.py reads process command-line metadata for genuinely live checks.
- Prior report below is historical; its permanent AD18 refusal and missing-transport state are superseded. AD17/AD18 implementation follow-up appended, no new question IDs.
- Manifest: `C:/Users/tlori/FreeCity4/build-runs/run4-verify-advice-subject4-r2-manifest.json`.

## Prior-round report (historical; superseded by Handoff)
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
