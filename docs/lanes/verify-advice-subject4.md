| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| verify-advice-subject4: independent native AdviceSubject verification | Codex / `verify-advice-subject4` (engine lane, detached HEAD) | WIP: corrected full gate PASS; OFF lifecycle PASS; native ON FAIL (36/321 differences) | Coordinator reviews AD19-AD21 and retained evidence; CURRENT40 withheld; no further lane round reserved. |

## Final report (logical round 3/3)

The native comparison is **not exact**. The actual supported public subject factory, QI and Clone succeeded; this is not the earlier AD18 transport refusal or a zero-comparison pass. The matched ON run performed 321 checks: 285 equal, 36 different. Every difference is filed under AD19-AD21. The ordinary same-name merge-queue entry is **WIP**, not ready. No fourth round, extra matrix, renamed lane, implementation reservation, or provider/default promotion is proposed.

This final round changed only this report, two unique run records, and appended questions. Engine sources remain the previously committed production transport/verifier: private subject Clone calls actual Rust `AdviceSubject::clone_subject`, returning an owned `Obj::Subject`; the production C++ Clone uses that transport. Four private field transports preserve actual Rust setter results. Five additive C exports mean **ABI=true** for this lane. Public ABI tables/version, shared config/maps/defaults/providers and generated ABI entries were not edited. Source milestones are preserved below; final native evidence is in `ec2739af`, after gate/artifact reconciliation `4dce3ca6` and OFF record `4dfddefc`.

### Gate, source and artifact

- Corrected existing `qbuild all` PASS: Rust **5040 passed / 0 failed / 0 ignored**, summed over 21 result lines; clippy PASS; native build **4/4**; CTest **68/68**, total429.50s. Actual corrected return-channel104.62s and ABI-conformance137.26s are confirmed in raw logs. No new all was launched for documentation-only commits.
- Gated source HEAD `b1463f3b4ba9af586cc4bbc28348b9f19097426a`; `HEAD:engine` equals `2993f562:engine`, tree **6a31aef7dd6bbcc2361c9ed2fdf96a15cc57a4c1**. Tracked engine was clean before/after artifact reconciliation and both runs. Subsequent own commits are documentation/questions only.
- Actual process creation/parent/command metadata ruled out live attributable checker/compiler/output work before freezing symbols. All checker command lines were readable; the only lane-related processes were orchestration and this audit shell. No old terminal/session handle was treated as live. WMI denial and the successful metadata alternative are recorded below.
- Exact outer gate start/terminal exit timestamp is unavailable; no invented exact time or retained exit line is claimed. Raw test log finished22:34:43.475464Z; link22:34:49.903834Z; CTest records22:35 to22:42Z with final log mtime22:42:11.021649Z. DLL creation22:42:12.556Z follows PASS and is reachable in the retained helper only after `qbuild all` returned0. Before/after committed source trees agree; helper/log hashes are below.
- Immutable `build-runs/immutable-subject-r2/FreeCity4.dll`: **b3eb4974e01da2a7f47c67a95abf3ec31bd076b887f919b91ce324c5230e8a36**. Existing DLL never overwritten. Matching PDB copied once from `engine/build-x86/FreeCity4.pdb`: **e69e7331466b74dcfa42bc83b65ccd5fbac7558f27ffdca5b21d0424584cf1e7**. DLL RSDS GUID/age matches PDB stream1; both files rehashed unchanged after each game. No build or artifact replacement occurred while either request was queued/running.
- Historical failures remain failures: undefined-logger compile failure; cached native1/4 rejection; actual CTest67/68 return-channel failure. The latter was two unchecked native SetLocation/SetObject AL calls, addressed by published `[abi-ret]` annotations in2993f562. This was not a flaky game, and the baseline is not described as green.

### Actual unique game runs

Both used the same immutable DLL/PDB, Konradshohe, turbo1, seconds900, hard timeout1050, end2032-01-01, refaudit1, popup_dialogs0, autotest_end quit and the unchanged full-save script. Only label and advice_subject_verify differed. Normalized settings SHA256 **357158bcb5a09cf7adfce5d8436782fa174231d61940d54c582bb8cd8df9b6e0**; script SHA256 **e1f13846cf9d890c763586104abee9439fad72ffca8676e483633d67bbefc0d7**. Both actual provider sets were **{simulator}**; budget was disabled and original. Explicit enable was empty. The log's generic validator warned unknown `advice_subject_verify` once per run; the321 actual ON comparisons establish that the verifier's direct opt-in read worked.

| Unique label / run record | Actual result | Save and lifecycle |
|---|---|---|
| `vas4-r3-native-control-20261023a` / [255](../runs/vas4-r3-native-control-20261023a-255.md) | OFF: harness clean; subject lines0; queue7s/wall64s, real43.188s; reached2032-01-01, final2032-01-15 | Full SaveCity ok1,11016ms on2031-12-12; before/after marks and both shutdown hooks |
| `vas4-r3-native-probe-20261023a` / [256](../runs/vas4-r3-native-probe-20261023a-256.md) | ON: harness clean but native comparison FAIL36/321; queue4s/wall62s, real42.500s; reached2032-01-01, final2032-01-14 | Full SaveCity ok1,11032ms on2031-12-06; before/after marks and both shutdown hooks |

Both runs reported **foreign_exceptions=0, watchdog_stalls=0, rtc=0, overruns=0, ref_warnings=0**; Rust panics/structured exceptions/incomplete shadows0. Each actual end-date message, SaveCity result, after-save mark, PostAppShutdown and PostSystemServiceShutdown was read from its own raw log. Harness clean does not override native differences. Per-run request/log/verdict hashes are in the run records and reconciliation notes below. Results and original requests remain under own `build-runs/game-results` and `game-queue/done`.

### Measured subject boundary

Only raw independent test inputs and primitive city dimensions128x128 were shared. Native outputs never seeded core fields or answers. Native and Rust fixtures were distinct; native creation used typed public `GetClassObject(0x6A26D049,0x6A26D050)` with no alternate endpoint, advisor-system activation, registration or global substitution. Factory/QI/Clone actually succeeded. Refusals0, fallbacks0; the verifier reported undefined_returns0, while AD19/AD20 still leave return canonicality/default compatibility unresolved.

The log measured **161 native calls/results across slots3-13**, independently of the historical prototype counts:

| Slot / method | Native calls/results | Actual comparisons |
|---|---:|---|
| 3 SetLocation, AL | 6/6 | Five base plus one clone return logged33 versus Rust1 (AD19); finite locations agree |
| 4 GetLocationX, x87 | 23/23 | Float bits agree, including0,40500000,43000000,41580000 |
| 5 GetLocationZ, x87 | 22/22 | Float bits agree, including0,40F00000,43000000,418A0000 |
| 6 HasLocation, AL | 22/22 | Defaultfalse and subsequenttrue agree |
| 7 SetObject, AL | 8/8 | Returntrue agrees; event-vector differences remain AD21 |
| 8 GetObjectA, pointer | 21/21 | Correct independent identity; borrowed ref-count checks8/8 per side unchanged |
| 9 GetData1, fullword | 23/23 | Defaults, explicit fullwords, clone and original independence agree |
| 10 SetData1, fullword | 6/6 | Native assigned word preserved; Rust returns true as deliberately specified |
| 11 GetData2, fullword | 23/23 | Initial word differs (AD20); explicit assigned words/clone/original independence agree |
| 12 SetData2, fullword | 6/6 | Native assigned word preserved; Rust returns true as deliberately specified |
| 13 Clone, AL | 1/1 | Both succeed, distinct identities, initially equal assigned fields; actual production Rust clone used |

Finite inputs covered interior3.25/7.5, negative bounds,128-cell upper clamps, mixed lower/upper bounds, zero, and independent clone13.5/17.25. Native EAX scalar results were never coerced to bool: both setters returned0,1,12345600,80000000,FFFFFFFF, plus clone42/43; Rust setters returned1. The native location result33 is the logged promotion of the typed AL return, not a full-EAX claim or a normalized true.

| Raw difference group | Count | Question |
|---|---:|---|
| Base SetLocation return33 versus1 | 5 | AD19 |
| Clone SetLocation return33 versus1 | 1 | AD19 |
| Initial GetData2=00094ABE versus0 | 6 | AD20 |
| Explicit default data2=00094ABE versus0 | 1 | AD20 |
| SetObject transition-vector equalityfalse versustrue | 3 | AD21 |
| Clone SetObject transition-vector equalityfalse versustrue | 1 | AD21 |
| Cumulative object-event-vector equalityfalse versustrue | 19 | AD21 |

These seven raw groups account for all36 differences. AD19-AD21 were appended without rewriting AD17/AD18 historical entries. The generated index still suggested AD17; existing AD17/18 were honored and19 was the next actual free subject ID. Index regeneration belongs to the coordinator; forbidden tools/shared indexes were not edited.

### Ownership evidence and limits

Two held fixture objects per implementation were distinct and survived diagnostic zero transitions so ordering failures could be recorded. Native/production ref counts matched in all23 comparisons, as did all23 zero-transition comparisons. Same-pointer transitions with only the subject's hold remained live. Clone identities were distinct, initially equal in fields, shared the corresponding held object with count rising2 to3, and later clone-only location/data/object changes left the originals' location/data/object unchanged. Borrowed GetObject calls did not change held counts. Clone scalar mutation vector comparisons agreed, as did their float/fullword values. Both created subjects were dropped; subject/clone final Release returned0; held objects ended at0 with each zero-transition count1, after all creator holds were released. Verifier and whole-run reference warnings were0.

Complete event vectors were compared in memory across both held objects, but several failed and **the individual raw event arrays were not logged**. Consequently native exact clone AddRef event count and native replacement/same/null event order are not established merely by net count agreement; Rust's focused test verifies its exactly-one clone hold and AddRef-before-Release contract. AD21 records this evidentiary limit. No native ordering was inferred, no production reorder was made to fit results, and full ownership-trace compatibility remains unverified. This alone prevents promotion to READY.

CURRENT40 remains **unsubmitted/unmeasured** because matched ON did not pass. Its old template is retained; no actual provider set or assumed42 additions are claimed. Existing vas4-r2 templates were never submitted and remain intact. There are exactly two unique actual run labels this round; lifecycle launches add no new algorithms or unique native fixture coverage beyond one subject/clone pair and two held objects per implementation. No request is outstanding.

Serialization/typed stream/reference endpoints, nonfinite locations, failed-QI output semantics, AD11/AD16, advisor-family completeness and owner/default/replaced promotion remain withheld. Reload is explicitly unmeasured because no supplied documented persisted reload fixture exists. No executable/slot/Documents lookup or guessed reload action occurred. No normal-speed replay was justified: the measurements do not establish a turbo-only difference.

Coordinator review of the three questions and this WIP is the remaining action; this final round makes no further implementation reservation. All historical failures and unsubmitted templates remain available. The final queue entry names the actual clean committed detached HEAD; shared refs/queue/main were not changed.

## Gate/artifact reconciliation notes (chronological snapshots; final status above)
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
- 4dce3ca6: corrected gate/artifact reconciliation committed. OFF vas4-r3-native-control-20261023a now clean: reached 2032-01-01, final verdict date 2032-01-15; full SaveCity ok=1 in 11016ms on 2031-12-12, both script marks and both shutdown hooks; ref/exception/stall/rtc/overrun all 0. Actual enabled {simulator}, budget original/disabled. Subject lines 0 as required OFF. Generic config validator warns unknown advice_subject_verify (same warning must be retained ON; actual verifier output required).
- OFF queued 2026-10-23 00:12:09 BST, started 00:12:16 slot1, wall64s; seconds900/timeout1050. Own request SHA256 8fddf995688a21bf1fdbb6f3fe6f69c0b316e389f990f2de6de3bad6b6db49a3; log 1586cdba95ca06c7936f0eebe4ed67f7dcc4fd18d9e133df2a4c957214130879; verdict caeabca7131836dfe81124f8764772e1f7fab03b00f9b080b8de69f10d9f0c49. Script e1f13846cf9d890c763586104abee9439fad72ffca8676e483633d67bbefc0d7. Normalized request excluding label/probe SHA256 357158bcb5a09cf7adfce5d8436782fa174231d61940d54c582bb8cd8df9b6e0. Run record 255.
- 4dfddefc: OFF evidence committed. Matched ON vas4-r3-native-probe-20261023a completed harness-clean with full SaveCity ok=1 in11032ms on2031-12-06, both marks and both shutdown hooks; ref/exception/stall/rtc/overrun all0. Reached2032-01-01, verdict2032-01-14, real42.500s/wall62s; queued00:16:03, started00:16:07 BST slot1. Actual enabled {simulator}; budget disabled/original. Same generic unknown-key warning; actual verifier ran.
- ON actual native factory/QI/Clone succeeded; 161 native slot calls, 321 checks, 285 equal/36 unequal, refusals0, reported undefined_returns0, created2/dropped2, verifier ref_warnings0. Measured differences: AD19 six SetLocation AL-promoted33 versus01; AD20 seven initial data2=00094ABE versus0; AD21 three transition vectors, one clone transition vector and nineteen cumulative vectors differ (23 checks). Full raw event arrays absent, so complete native order remains unverified; final counts/zero transitions agree. No engine changes or output fitting.
- ON request SHA25609491dea5d6a71879f8aea6a8908ef255e34d61e3b8afd7ea4d73e88506b7285; log35fe3d919afc3fad3b66e3727f8dce69e4644dc1b28ef73b232627acfecea665; verdictf765661e49ecadad5bf0e797e5d72167e88ddcb04f33d46295091ec58af10c3b. Script and normalized settings hashes exactly match OFF; immutable DLL/PDB rehashed unchanged. Run256. AD19-AD21 appended only to questions/advisors.md (index stale at AD17; actual existing AD17/18 preserved; coordinator regenerates index).
- Next exact step: write bounded final WIP report and same-name WIP merge-queue record. CURRENT40 is not submitted because matched native ON did not pass. No outstanding request or further round reserved; no normal replay (no evidence difference is turbo-only), no build (unchanged engine), no reload fixture supplied.
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
