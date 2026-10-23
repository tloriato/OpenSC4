| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| core-scene-renderer4 (first portable cSC43DRender owner) | Codex / `core-scene-renderer4` (engine lane, detached HEAD) | wip: final all Rust5155/0/0 incl13 owner tests, strict clippy and native4/4 observed; readonly stats32992 pending, final exit UNKNOWN/full68CTest unrun; seven-path fmt exit0; R2 terminal/cache UNKNOWN; denominator130, bytes UNKNOWN; ABI unchanged | Coordinator collect/account for capture27740/qbuild26472/verifier24068/stats32992 and actual terminal/source-end/full68CTest result. Keep held; no duplicate invocation, fourth round/reset or cache/process workaround. |

## Final original round 3/3: held, final wrapper incomplete

This round reconciled the absent R2 checker, preserved its measured components and UNKNOWN terminal/cache outcome, formatted six owner files, and ran exactly one final `qbuild all`. **Final source-corresponding Rust5155/0/0 (13/13 owner tests), strict clippy and native4/4 are observed. Full68CTest has not run and the final wrapper exit is UNKNOWN; this lane remains WIP.** No fourth round/reset, new checker or cache workaround is requested.

Production algorithms remain93a6fc20. Formatting commit **9a843f0a57167befb94f4e792f887ffcf766f52c**, engine tree **ae8d9d0492a4ba44b2c7f50e2b664fd6ef52c074**, replaces pre-format engine **b1124501f2deef4b5315970a855e970d4f74aec7**. All seven owner files are exactly the original651a2297 source passed through rustfmt with explicit engine config; record.rs was already identical, and lib.rs is unchanged. This is formatting only: no algorithm, assertion, fixture expectation, skip, tolerance or supported-domain changes.

### Final gate: actual evidence and remaining hold

- Command: `python C:/Users/tlori/FreeCity4/.claude/lane-bin/qbuild.py --wt C:/Users/tlori/FreeCity4/.claude/worktrees/core-scene-renderer4 all`, wrapped by local `build-runs/final-r3/capture-final.py` for actual start/end/exit snapshots.
- Actual start **2026-10-23T00:32:46.835982Z (01:32:46 BST)**, clean HEAD9a843f0a/engine ae8d9d04. All eight recorded files match committed source after CRLF normalization.
- Workspace summaries total **5155 passed / 0 failed / 0 ignored**; doctests **0/0/0**; owner tests **13/13**, names below. Strict clippy finished successfully in **46.69s**. These are actual final-source results, distinct from the older baseline and R2 quick.
- Native release build log reaches **4/4**, with the CAPI smoke executable and DLL linked. Current DLL SHA256 **61eaf78e727632f07a8435e5bec6ab6910ed01902f2ebed08110990c2117b24f**, **18127360 bytes**, mtime **01:41:16.317539 BST**. No native-owner execution/equivalence follows from building the plugin.
- At **01:42:44.721514 BST**, actual process identity/creation/parent/cwd/argv shows capture27740 -> qbuild26472 -> verifier24068 -> **readonly stats32992**, created **01:41:30.597988 BST**, command `sccache.EXE --show-stats --stats-format json`. No compiler/output writer remains for this lane. Full CTest has no final log; generated metadata expects68. No terminal PASS/FAIL, exit code, end time or end snapshot is available yet. The absence of a terminal result is not a fabricated failure or success.
- Fresh final-state attribution at **01:45:55.478601 BST** confirms those same creation identities/cwds/argv, still pending after **4m24.88s**, no candidate command inspection unknowns, no same-WT compiler/writer or final CTest log. Native release compilation finished in **3m47s**; all4 build steps are durable. The live process remains untouched; final-status-attribution.json preserves the full snapshot.
- Report-only checkpoint **efe37829021de331bb005b023c4badf7be3086cb** changed HEAD while the gate ran. Engine tree and all eight source hashes stayed unchanged; this is documentation movement, not compiled-source drift. Final terminal/source-end evidence must still be collected.
- Seven **absolute owner paths** passed `qbuild fmt`: actual **exit0/files7/diff_lines1**, sole warning `could not canonicalize path C:\Users\tlori`. Original **exit1/files8/diff_lines2044** remains preserved separately. Six files changed66 insertions/266 deletions under engine width120; record.rs needed no change. Shared lib.rs pre-existing ordering drift is separately reported; no whole-lib formatting was run.
- Durable local evidence: `build-runs/final-r3/start.json`, `owner-format-correspondence.json`, `format-result.txt`, `rust-complete.json`, `native-complete.json`, `native-terminal-attribution.json`, capture/collector scripts and the raw qbuild/fc4verify logs. The captured-console collection attempt returned no buffered text; this is not a terminal verdict.
- No process/client/daemon/service was terminated, no independent cache RPC issued, and no environment/provider/cache-policy or wrapper-tooling changes made. No exceptional coordinator action is inferred or authorized by the observation above.

All thirteen final-source owner test functions passed:

- `backing_b1_b4_and_both_ignored_results`
- `backing_probe_failure_frees_both_short_circuits_and_missing_buffer_cleanup`
- `dirty_strict_ties_mode_zero_penalty_deadline_and_threshold`
- `failed_frustum_stays_unsupported_on_repeat`
- `full_redraw_area_boundary_coalesces_actual_existing_frustum_and_queue_clearing`
- `identity_lifetime_reaches_destructor_only_after_shutdown_boundary`
- `lists_unsigned_stable_duplicate_refs_first_per_list_and_null_terrain`
- `literal_dirty_fixtures`
- `shake_s1_s6_stop_fade_strict_expiry_and_rounding`
- `shared_backing_buffers_are_owned_once_and_switch_is_process_wide`
- `model_queue_kinds_old_current_boxes_refs_predicate_and_boundaries`
- `signed_filter_literal_cases`
- `typed_record_version_and_sticky_error_partial_writes_are_literal`

Final-source raw SHA256 values (lib.rs differs from its committed blob only by CRLF; normalized committed SHA256 `a462c04fc5e091a4f22580783c66e76076e8fc3706ea43b55a6b2d9e7a227f20`):

| Source under engine/core/src | SHA256 |
|---|---|
| scene_renderer/backing.rs | `55827f7e524a71ba048d3f25ee2ae157a900646086d47d4a4991163ef832f329` |
| scene_renderer/fixtures.rs | `4cc919284a7c8a09f4ac2ce0709ad67c2beb745232359283d1393bfa6b6ac614` |
| scene_renderer/mod.rs | `7b60e07da901a6bfe0e31821adf9cbccdd9267dd8381a6f687a8f80fd68f5a27` |
| scene_renderer/models.rs | `b9d4dbf3154318e047f457712981b0668ef7ea2a8170921fc3c634bc6687362c` |
| scene_renderer/record.rs | `70986fb40235cf88529c67dac8a1002638c440a564a689e798a7a86a471d3eac` |
| scene_renderer/shake.rs | `356f1c134cf121ffa47717314ad04b61b800c760318a8c8e085659c917364923` |
| scene_renderer/tests.rs | `9bdd52e115e45a95a7e2885a34549e210e4fb6d9734021b0ff484dc5ff3a0585` |
| lib.rs | `ad329c87dda520ce692dc234f17283f1cfc1205d573ac27a70eb9f6e46f7eafc` |

Observed final log hashes (the native log is complete; final wrapper/cache/fullCTest are still pending):

| Log under build-runs | SHA256 |
|---|---|
| fc4verify-build.log | `4531544d9daa9cf70a0202acfa823379a0a928f36909109392040db84330c36a` |
| fc4verify-configure.log | `32dd863e1cedee6cf44e3a3b2f6e4849851752a7de2a69defaa5b5e89391a420` |
| qbuild-clippy.log | `9d88b2036c9a9b23107b233bceb5ba4a0d1f148789ef4097559f61422e340995` |
| qbuild-test.log | `a0af7e14b4b13de85080e5177bb7291a68b5d7e66fa43f31079c1853fae30413` |

Other artifacts: fc4_capi_smoke.exe SHA256 `2e46ac15973a21cbc0e3a41bde37a859734e7778645803e052f0ecd1150ea452`,11021312 bytes; unchanged fc4_plugin_load_test.exe `06b249a28a0703c01b1bbad7eaf95badafe5d9993add07e26edc4fb4e050a355`,3120128 bytes. Neither artifact hash supplies a missing CTest verdict. Published spec SHA256: render3d.md `8c47d249712b863abd398622612497ddbc003f5bd42e13f98c1b193596ade829`; view3d.md `00324f403ccc59d1ab12af4358a623cba38ca9637eaa719b86483d819e54275d`.

### Old operation reconciliation and exact transport diagnostic

At01:27:46/01:28:10 BST this round's Toolhelp/NT audit found old qbuild38324/verifier9384/readonlystats36584/worker23512 absent, with no same-WT checker or output writer. Full current argv/image/creation/parent/output attribution is preserved in process-details{,-followup}.json. No candidate command inspection was unknown; shared server35784 and its sole conhost25092 child had denied service cwd reads5, recorded without hiding them. Other output paths belonged to train-integration/core-legacy-control4; netstat returned no4584 row. Dated absence establishes neither a terminal outcome nor a disappearance cause.

R2 raw log bytes are archived as base64 in final-r3/archived-r2-*.b64; decoded SHA256s match the historical table below. Reused measurements remain core3113/0/0 incl13 owner tests, compile-only dependents and native4/4. No unchanged quick was rerun to repair a missing terminal verdict. The still-earlier baseline5142/0/0, clean clippy and native215/215 remains **partial earlier-source evidence**, with UNKNOWN wrapper exit and no durable fullCTest result.

Exact retained raw R2 stderr:

`2026-10-23T00:21:20.005747Z ERROR codex_core::session: failed to record rollout items: thread 01a165fd-6a9c-72d0-b2cc-88de0ce8717d not found`

This rollout-recording error is separate from compiler/native diagnostics. Structured result, committed report and queue exist, so no fatal-execution inference, invented root cause or claimed fix follows from it.

### Bounded scope, questions and next step

The attributed denominator stays **130 owner functions including folded bodies; exact implemented body bytes UNKNOWN**. No implementation/native-equivalent percentage, foreign-body credit, whole-owner/native/shadow/default/replaced promotion is claimed.

Support remains the existing bounded dirty scoring/filter/strict ties/threshold/deadline/recursive area overcount; stable unsigned view lists and reference decisions; two-buffer backing order/failure decisions with explicit shared ownership; queue kinds0/1/3/4; restricted shake/typed-record prefixes; actual existing CameraControl forwarding and the existing frustum restrictions. D5 spacing100->1000 rationale remains: spacing100 scores-900 and merges before exercising the16-node threshold. Tests/expectations were not changed to fit results.

Queue kind2 stops Unsupported before MoveGridElement; kind5 derivation and queue200 DoFullViewUpdate stay Unsupported. General geometry, full Draw/ProcessDirtyRectangleQueue/UpdateViewportRect/DoPartialViewUpdate/DoViewCleanup, lifecycle/static driver/destructor, picking/visibility, general sine/extended shake, render3d section7 and view3d residuals remain Unsupported. Raw foreign primitives cannot supply renderer decisions or complete unexecuted plans. Repeated frustum errors remain errors; explicit elapsed time, backing ownership/reentry limits persist; Rust Drop gives no native destruction/reference proof.

Retained questions: **RND1/RND2/RND4/RND5-RND7** in docs/specs/questions/render3d.md and all V3D-1 through V3D-10; none new this round. No spec, native/plugin/CAPI/ABI/default/coverage changes, terrain/other-owner work, game/shadow requests, network/live-user-file operations, external messages, subagents, merge/rebase/push or shared-ref changes were made.

**Next:** coordinator must collect/account for the one still-live final operation, including real terminal/source-end and full68CTest outcomes. Keep this lane held until actual full source-corresponding checks pass. No fourth round/reset or new invocation is requested.

## Historical report: round 2 verification, WIP

The previously committed bounded renderer now has meaningful source-corresponding test evidence: all **13 owner tests passed**, within **3113 passed / 0 failed / 0 ignored** core tests. This round changed the support inventory and preserved evidence; it did not expand production algorithms, alter fixtures or add tests. Production source remains 93a6fc20, engine tree `b1124501f2deef4b5315970a855e970d4f74aec7`. The full final gate is incomplete because the authorized focused wrapper is waiting on an ancillary read-only cache query.

| Check | Actual result |
|---|---|
| Focused core i686 tests | 3113/0/0, including all 13 renderer tests; compile11m37s; completed01:01:16 BST |
| Dependents | fc4-capi, fc4-conformance, fc4-formats compile-only completed01:05:40 BST |
| Native build | 4/4 incremental steps completed, including CAPI smoke and DLL links |
| Focused wrapper terminal | UNKNOWN/pending; no successful wrapper/cache result claimed |
| Final qbuild all / final clippy | Not started; existing wrapper must first be accounted for |
| Full CTest | Expected68 from generated metadata; not run, no observed full-suite result |
| Absolute touched-file fmt | exit1, files8, diff_lines2044; owner width100 vs engine120 and shared lib ordering |
| Game / shadow / ABI | No requests, no shadow or native-owner credit; C ABI unchanged |

At **01:13:44 and 01:16:48 BST**, OS identity/creation/command/cwd attribution showed qbuild **38324** (created00:48:17) -> fc4verify **9384** -> sccache **36584** (created **01:12:10.094 BST**), command `sccache.EXE --show-stats --stats-format json`, cwd this worktree. No compiler/output process targeting this lane remained in the current enumerated compiler candidates, including service-owned compiler output-path attribution. The native build log ends in 4/4, but the ancillary query remains pending. No exit/timeout diagnostic was emitted. This is a tooling block, not a successful or failed cache query. No process/client/daemon/service was terminated or changed; no independent cache RPC was issued, no environment/policy workaround was used, and no new compiled check or source edit overlapped it. Session40000 is only the historical transport handle, not liveness evidence.

Evidence is durable under **build-runs/quick-r2/**: rust-evidence.json, native-progress.json, native-attribution.json and cache-query-block-evidence.json. The latter contains the full raw current process attribution, eight source hashes, log hashes, source tree and artifact data. The eight source hashes still exactly match the source measured for the passing core tests.

| Current log | SHA256 |
|---|---|
| fc4verify-quick-test.log | b5f725ce899d1fa885fad5b7810789bb9844e39aaeeed3bad27b0f4c0cb03f2c |
| fc4verify-quick-deps.log | b26fb74b1a9eca9c880b8171cf891167d83b8858f9a7e67b50368f12574c8f18 |
| fc4verify-build.log | db2945ac5928d93bd1b84d5a5044c96526982188497c415d1fcd791a2b9f4424 |
| fc4verify-configure.log | 4d29f35a5eea64a94f4c2c66ad0d2eec10a5ca5b6affc2edb36c03c455310f09 |

Current built DLL: **e95aa730eba521981e2bfd622f8cc7de35330210c7cdae67c32a3c6daf2c114b**, 18127360 bytes; mtime01:12:07.648 BST. Its native rebuild log and unchanged source hashes correspond to this focused operation. This does not validate the missing final workspace/clippy/CTest gate and gives no native replacement or whole-owner credit.

The thirteen passing functions cover D1-D6 dirty/filter cases, strict ties, mode0 penalty, 15-vs16 threshold, inclusive -2500, deadline effects and recursive31000 overcount; B1-B4 and probe/allocation failures, ignored foreign bools, explicit shared backing ownership; stable unsigned duplicate lists, first removal per list and release/clear policies; model queue kinds0/1/3/4, old/current boxes and reference decisions, explicit kinds2/5 and queue200 boundaries; lifetime/destructor boundary; typed-record partial-error order; restricted shake state/expiry/rounding; full-redraw/frustum coalescing and repeated failure. Camera forwarding was also inspected through the actual existing CameraControl methods. Foreign hosts supply raw reference/property/model/grid/buffer/query primitives only. D5's documented spacing correction100->1000 remains: spacing100 scores-900 and merges before exercising the threshold. No expectations were adjusted to fit implementation.

## Earlier operation reconciliation and preserved baseline

The supplied coordinator reconciliation (00:40:16 BST) records qbuild1880 -> fc4verify3680 -> sccache23088 waiting on readonly stats at00:36, all absent by00:37:58, and no coordinator termination. Actual r1 result00:37:16 says continue at12bb9830; stderr records guard154k/150k. The terminal cause/code is **UNKNOWN**. Old session3939 cannot be collected portably and is not current liveness evidence.

Read-only OS audits00:43:44/00:44:13/00:46:03 used Toolhelp identities/parents, creation/image/command and accessible PEB cwd. Old1880/3680/23088/6076 were absent; current compiler/cache-client output paths were attributed to other lanes, including server-owned Rust compilers. Some service cwd reads denied5, but command output paths remained attributable. Shared bare servers were left intact. DLL/plugin-load/.ninja_log timestamps matched the earlier native log tail. This established no remaining attributable old output operation, while preserving the unknown terminal outcome before one necessary stable-source replacement.

All four original logs were moved byte-for-byte with apply_patch into **build-runs/baseline-r1/** before wrappers could overwrite them, with verified hashes:

| Baseline log | SHA256 |
|---|---|
| qbuild-test.log | a8d5eeacc0e0b779c1e97c97087dd1957b42eedcdb3679d37947be831d35f816 |
| qbuild-clippy.log | 80072f9ba4c8ba51de131c2521230152808b9208fdcaf57e7eb2a157f473d9b7 |
| fc4verify-build.log | 07d8f72ccf02f607ed64ea2b4b75c9bdedb2c75a9ee288f0dea2937061010063 |
| fc4verify-configure.log | 34970e139997eb50963cf4c607951dff6b2409a432a7d77c3d4eb06f6e4acd2f |

Baseline earlier-source Rust5142/0/0, clean clippy3m20s and native215/215 remain **partial earlier evidence**, not final acceptance. There is no durable baseline CTest log or known terminal wrapper result. Supplied r1 result/stderr provenance and exact diagnostics are preserved in baseline-r1/reconciliation.md; q.py's Bash bootstrap access denial and unavailable psutil were not used as process-absence evidence.

## Next operations and remaining scope

1. Collect/observe the existing qbuild38324/fc4verify9384/stats36584 operation through its actual terminal outcome, accounting for service-owned compiler/output work. Do not terminate even the read-only client, start quick/all, edit compiled source or apply a cache/environment workaround while it remains outstanding. Exceptional root action requires explicit coordinator evidence.
2. Preserve these quick/native logs and any eventual ancillary outcome before overwrite. The already valid core3113/13-owner evidence remains valid for its measured source; do not repeat an unchanged baseline.
3. Apply only the prepared owner formatting correction after terminal. build-runs/format-scene-renderer4.py emits an apply_patch patch using the explicit engine rustfmt configuration; six owner files differ, record.rs needs no change. Run the authorized fmt wrapper on absolute owner paths and the necessary lib.rs declaration. Preserve the additive declaration and do not broadly reorder pre-existing shared modules.
4. Commit stable source, run **one final qbuild all**, fix only demonstrated owner implementation/test/compiler/clippy failures, and inspect all actual Rust totals/13 owner results, clippy, native,68 expected/observed CTest, tree/source/log/artifact correspondence. No duplicate gate for a report-only commit. Enqueue ready only with completed required checks.
5. This is original-family **round2/3**; at most **one round remains**. Round1 consumed154k/150k; no family reset or renamed/fourth round. Round2 limit remains150k/max100calls; guard after017b0a37 measured132532/150000 (17468 tokens remaining at that measurement). Use the current guard manifest/status for subsequent measured allowance. WIP reflects the current tooling block, not a budget reset.

Final OS identity check **01:19:50.767 BST**: qbuild38324 and fc4verify9384 still have their00:48:17 creation identities, and stats36584 still has01:12:10 identity and sccache executable path. Query has been outstanding over7m40s without a terminal diagnostic; the last blocking collection likewise returned no output and an active transport handle. Tracked worktree was clean. The wrapper/client remain untouched; formatting patch remains unapplied and no all gate was started. This is the exact next operation to collect, not evidence of success or a failed-query exit code.

Questions remain RND1/RND2/RND4 and appended RND5-RND7 in docs/specs/questions/render3d.md; all V3D-1 through V3D-10 are retained. No new question/spec/index changes in round2. No native/plugin/CAPI/ABI/default/map/coverage changes, game/nativeprobe/reload, network/live-user-file operations, external communication, merge/rebase/push or subagents. Denominator **130**, exact body bytes **UNKNOWN**; supported/partial/Unsupported methods follow.

## Named-method inventory (initial inventory committed in f4d643d0; support refined in round 2)

Denominator: **130 owner functions**, including folded bodies. Exact implemented body-byte subtotal **unknown**. Interface slots are not unique bodies (slots 18/19 share one folded body); no 130/130 claim can be made. The publication does not enumerate every physical body by name/address, so unnamed residual members of that denominator stay uncredited.

| Published methods / distinct overloads | Specification status | Current bounded implementation support |
|---|---|---|
| QueryInterface; AddRef; Release; constructor; GetGZCLSID | §1 algorithms, destruction body absent | Typed identity/count; last Release reaches explicit Shutdown boundary before destruction |
| Write; Read | §2 record, RND1 unresolved native slot name | Portable typed record only; version and partial-read ordering, no native wire binding |
| SetMapScaleFactor; GetViewUtilities; GetLightingManager; GetDrawContext; GetViewportSize; SetViewportSize; SetSubpixelImageBias; GetCamera; SetCamera; GetCameraControl; SetCameraControl | §3 specified | Local size/map/bias/borrowed camera getters supported; SetCamera has raw reference/depth primitives and checked domain; SetCameraControl partial within existing view-utilities domain. GetLightingManager/GetDrawContext Unsupported |
| GetProjectionMatrixEntries; GetViewMatrixEntries (same folded body) | view3d slot names only | Unsupported |
| SetZoom; ZoomIn; ZoomOut; SetRotation; RotateLeft; RotateRight; SetZoomAndRotation | §3 forwarding only | SetZoom/SetRotation/SetZoomAndRotation forward to actual existing CameraControl methods; ZoomIn/ZoomOut/RotateLeft/RotateRight Unsupported |
| Set3DCameraView; Set2DCameraView; SetUICameraView; BorrowRenderSystem; ReturnRenderSystem | §3 specified primitive effects | Borrow flag supported; Return clears flag then returns explicit Unsupported at camera/driver state; all three camera-view setters Unsupported |
| GetViewFogEnabled; SetViewFogEnabled; GetLightingEnabled; SetLightingEnabled; SetViewportClearColor; EnableBackgroundPicture; GetBackgroundPicture; EnableTransparentBackground; GetBackingStoreEnabled; SetBackingStoreEnabled; GetShakeOffsets | §3 specified | Local state and explicit shared backing state; RND2 retained |
| ForceFullRedraw; DoFullViewUpdate; ClearDirtyRectangleQueue | §§4,6 specified | Coalescing and owned queue effects; existing frustum helper restrictions propagate |
| UpdateViewportRect; DoPartialViewUpdate; ProcessDirtyRectangleQueue; DoViewCleanup | §§4,6 specified outer policy, own draw/clip helpers absent | Unsupported, no callbacks returning renderer outcomes |
| AddDirtyRectangle(rect); AddDirtyRectangle(box); AddDirtyRectangleInternal; signed rect filter 0x007C2C70 | §6 specified | Owned merge/filter algorithm; existing projection only |
| ModelShouldGoIntoBackingStore; queue-item copy 0x007C2E50; ProcessModelChangeQueue; IsModelInserted; InsertModel; RemoveModel; MoveModel; ChangeModel; UpdateGridElementObjectFlags; MoveGridElement; SetQuadrantFlagsForElement | §5 specified queue policy; flag derivation/grid geometry partial | Owned typed queue and decision effects; missing geometry/flag derivation remains explicit |
| StartScreenShake(parameters); StartScreenShake(overload); StopScreenShake; SetShakeOffsets; node-copy 0x007C63A0 | §§6,10 specified | Owned list, start/stop, seed-88 random table, bounded exact arithmetic; RND4 fade=0 choice; extended sine domain unsupported |
| AddViewObject; InsertIntoViewObjectList; FindInViewObjectList; FindViewObject; RemoveViewObject; list clear 0x007C3B60 | §11 specified | Stable unsigned lists, duplicate refs, first removal per list, null terrain quirk |
| IsModelVisible(model); IsModelVisible(element) | §11 routing, existing helper restrictions | Unsupported; existing view utilities alone do not implement these owner wrappers |
| PickTerrain(ray); PickTerrain(screen); Pick(ray); Pick(screen) | §11 specified selection/routing but own ray-container helper absent for Pick | Unsupported, no picking/routing credit |
| PickTerrainAlways(ray); PickTerrainAlways(screen); point insert 0x007C6EC0 | §11.6 outline | Unsupported; no invented ray clip |
| CreateBackingStore; DestroyBackingStore; SaveBackingStore; RestoreBackingStore(rect); RestoreBackingStore(); ReevaluateBackingStoreObjectFlags | §12 specified | First five supported within checked arithmetic/explicit owned shared-state domain, raw buffer primitives only; ReevaluateBackingStoreObjectFlags Unsupported |
| Init; ReadProperties; Shutdown | §9 outer sequence, grid/destruction/static service dependencies partial | Only never-initialized Shutdown no-op supported; Init, ReadProperties and initialized Shutdown Unsupported |
| DrawPreDynamicView; DrawPostDynamicView; DrawPreStaticView; DrawPostStaticView | view3d §3.3 calls specified, slot interpretation inferred | No native binding or invented pass contract |
| DoMessage; Draw; DrawStaticView; DrawDynamicView | view3d §§3–4 outer policy partial, own helpers missing | Explicit Unsupported, no unexecuted plan promoted as complete Draw |
| SetGraphicSystem; SetGDriver; StaticSetGDriver; StaticShutdown; destructor body; deleting destructor | §7 residual / outline | Unsupported |
| DoTranslatedViewUpdate; DoViewZoomChange3; DoViewRotationChange2; AttemptTranslatedViewUpdate; UpdateVisibleTerrainMetrics | §7 residual | Unsupported |
| WorldToContainerRect; GetViewFrustumContainerRect; GetDrawFrustumContainerRect; GetVisibleContainerRect; GetViewportRayContainerRect; GetModelContainerRect; CreateQuadGrid; RebuildQuadGrid; SetClipRect; UnsetClipRect; SC4ComputeViewportZProjection | §7 residual / call sites | Unsupported |
| ClearViewport; ClearViewportWithSolidColor; DrawViewportRectangle; DrawTexturedViewportRectangle; SetBackgroundPicture(two overloads); TakeSnapshot(two overloads); GetStatsString1; GetStatsString2; TurnOnLighting; TurnOnFog; CreateMaterialVariant | §7 / view3d slot names, not complete bodies | Unsupported |

This inventory distinguishes method algorithms from raw foreign model/QI/world-box, grid, buffer, draw-context writes, notification, clock, property and reference primitives. It grants no byte credit and no whole-owner/native equivalence.

## Bounded implementation (production algorithms93a6fc20; formatted9a843f0a; final gate held above)

- Milestones: f4d643d0 inventory/literal fixtures; 09fcae9f first substantive module; 36adf9ee discriminators/typed record/reference and camera calls; 93a6fc20 persistent frustum failure and explicit elapsed-time start input.
- Executable local algorithms: signed rectangle filter; AddDirtyRectangle(rect/box) and Internal scoring, strict ties, mode-0 penalty, unsigned 16-node threshold, inclusive -2500 merge, deadline adjustment, recursive mode 6 with area overcount; ClearDirtyRectangleQueue; DoFullViewUpdate/ForceFullRedraw coalescing through the actual existing frustum helper. Existing view-utilities unsupported inputs remain errors, including on repeated requests.
- Executable lists: AddViewObject/InsertIntoViewObjectList, FindInViewObjectList/FindViewObject, RemoveViewObject and list clear policies. Stable unsigned keys, duplicate AddRefs, terrain-null equality, one removal per list in +18c,+188,+194,+190 order, saved-store invalidation. Inlined list helpers are algorithm coverage, not extra fabricated unique-body counts.
- Executable backing algorithms: Create/Destroy/Save/Restore(rect)/Restore(), enabled getter/setter. Explicit Arc/Mutex process state owns its driver and two handles, including Drop cleanup. Ordered create/probe/free, first probe failure cleanup, ignored later service bools, clipping/early true and partial gate are implemented. Driver callbacks must not reenter the same process mutex; that reentry domain is unsupported.
- Executable identity/accessor algorithms: QI/AddRef/GetGZCLSID; viewport/map/bias getters/setters; camera borrowed getters and AddRef-new/Release-old SetCamera with raw depth query/write; GetViewUtilities/GetShakeOffsets; clear colour, background/transparency toggles, fog/lighting toggle order, backing enabled state and borrow flag. SetCameraControl and full-redraw operations retain existing view-utilities restrictions. SetZoom/SetRotation/SetZoomAndRotation call actual unchanged CameraControl methods; other camera wrappers are unimplemented.
- Partial model methods: typed queue push/copy and exact-200 redraw effect; ChangeModel/MoveModel/IsModelInserted decisions; ProcessModelChangeQueue completes kinds 0/1/3/4 within projection domain, preserving ordered references, current versus old boxes and backing dirty gate. Kind 2 stops before MoveGridElement; its separately executable post-move old/new region policy does not perform movement. Kind 5 stops at flag derivation. ModelShouldGoIntoBackingStore predicate is local. InsertModel, general RemoveModel and full grid geometry are not claimed. Queue-entry methods at exactly 200 return Unsupported requiring owned DoFullViewUpdate rather than pretending an unexecuted redraw completed.
- Partial shake methods: owned parameter copying/prepend/handles, explicit clock-restart precondition, overload fade=0 portable RND4 choice, stop/fade, strict expiry, disabled freeze, seed-88 table via existing RzRandom, exact restricted curve/phase arithmetic and half-even rounding via x87::rhe where applicable. Arbitrary interpolation, nonzero sine entries, broad positional/extended accumulation and nonzero repeated scale beyond one factor remain Unsupported. These are not complete general SetShakeOffsets/Start bodies.
- Partial lifetime/record: never-initialized Shutdown no-op; last Release reaches Shutdown then explicit unknown-destructor error without decrementing 0/1 or reporting deletion. Initialized Init/Shutdown, destructor and full Draw remain Unsupported. Portable typed Record Read/Write preserve version/order/sticky-error prefix writes but do not bind native slot 8 or transfer native reference ownership. Rust Drop does not perform unspecified renderer destruction: an embedding must explicitly drain acquired queue/list refs and clear camera references before dropping this portable owner.
- Remaining Unsupported: Draw and its static/dynamic, translated, clipping/clear/draw helpers; ProcessDirtyRectangleQueue; UpdateViewportRect/DoPartialViewUpdate/DoViewCleanup; full lifecycle/static driver ownership; picking/terrain clipping and visibility wrappers; ReevaluateBackingStoreObjectFlags; all §7 residuals and unlisted owner bodies. No complete Draw/Update credit from action plans or callbacks. No terrain, drawcontext, DX7, effects, window, CAPI, ABI, defaults, coverage or native changes.
- Final measured results and current readonly-query hold are recorded at the top; initial baseline, R2 quick and final-source validation remain distinct.
- Next operation is collection/accounting of final qbuild26472/verifier24068/stats32992. R1 session3939/R2 session40000 are historical only; current transport25324 is not independent liveness evidence.
