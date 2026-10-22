| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| glue-tutorial-complete4: native tutorial replacement | Codex / `glue-tutorial-complete4` (engine lane, detached HEAD) | WIP: guard context stop; gate PASS4240/clippy0/67; shipped init, active reload, safe UI and union clean2032/ref0; closed reload corrupt both; ABI=true; no promotion | Coordinator collect final evidence/report; no fourth implementation round assumed; no queue file written. |

## Handoff

- Goal: final family round, preserved6d6bb03d TU6-TU9 and prior constructed12/12; no fourth round assumed. Guard stopped at153k/150k; no merge-queue file written.
- Done: e920474c native init/filter/hints/click harness and binding teardown log; 0a2037f5 creator-owned window release audit fix (both existing AddRefs retained; lifetime/order unchanged). No new CAPI export this round; ABI=true includes parent prepare_city_save export.
- Stable gate on final engine source0a2037f5: cargo4240 passed/0 failed, clippy0 diagnostics, CTest67/67, build4/4. No Rust touched; fmt n/a. Gate milestone35f65c15; later milestones4037fd92,c3a84b02,f82e89aa.
- Exact next step: coordinator collect `build-runs/*comparison.json` and raw final verdicts; run existing summarize-tutorial-evidence.py, correct/render final report if desired. Do not resume implementation automatically; retain WIP/no promotion for residuals below.
- All scheduled runs COMPLETED. No outstanding request; final helper26750 exited0. No need to wait or submit another request.
- Prefix for all labels: `gtc4-r2-`. Acceptance PASS clean2032/ref0: original fixture persist1022b; fixed ours persist-ref0-1022e; original/ours reload-core-1022f/g; ours reload-ui-1022h; candidate-union-1022i; original-safe-ui-1022j.
- Final normalized comparisons: persist5/5 exact; active reload7/7 exact; safe UI38/38 exact; closed reload6/6 exact but FAILED acceptance. Separate processes, NOT same-process tutorial shadow. All four JSONs exist.
- Closed reload1022k/l BOTH corrupt: foreign_exceptions1, ref0, final date2000-01-01, durations600.234s/180.281s. Original heartbeat reached2015-09-22 before stall; do NOT assert initial load failed. Six observations match; no successful closed-reload save/soak credit.
- Original broad UI1022d rejected: cheat save atline32 entered modal until autotest teardown602s; date2015-09-21/ref1, later actions skipped and explicit save0. Safe UI scripts remove only the two valid cheat-save calls, retaining line numbers. Initial ours persist1022c rejectedref1; corrected1022e is ref0.
- Fixture immutable main/build-runs/fixtures/tutorial/BigCityTutorial.sc4: v2/flag1/task0/GUIDCC02D3A3; SHA256adc2e09ef041f169db69d5d8343786790baa2d96a52f3aeb338e7719c17fc38f. Actual first Julian2457283=2015-09-17; scripts2015-09-18; no start/cheat-save before first query. Ours native GetItem found1/active1 and window Init precede query; NEXT0->1, active reload1->2; binding restored1.
- Captures under main/build-runs/fixtures/glue-tutorial-complete4 with adjacent JSON hashes: original activec8eb1347, corrected ours activebe51d685, original closed66ff9124, ours closedc775a697. SaveCity renames alias to City - Big City Tutorial.sc4; captured before slot reuse; no overwrite/game data committed.
- Union1022i explicit39 keys+tutorial, follow shadows budget/power/plumbing/zones/demand/history/clock24/propmgr/lotcfg; clean2032-01-03/ref0 (105s). Exact request/log retained; no default/map changes.
- Safe UI native event0/1/2, all TU8 IDs/ignored cases, hints/click0/1/2, quote/numeric/arity/case/step success/failure, quit and no-window calls exercised. Residuals: closed-reload stall/exception, live catalog path, both valid cheat-save forms completing, regional reset cheat, branch-specific UI success/visual effects and concurrent getter stress.
- No new numbered questions: known TU6-TU9 sufficient; runtime/harness gaps remain. No ABI version/generated ABI entries/defaults/maps changed. Proposed f2 retry withdrawn before running; completed unique verdicts retained. Helper sessions1661/41188 interrupted after process query denied; no daemon/game restart.
- Evidence scripts: tutorial-fixture-{persist,reload,ui}.script; compare-tutorial-fixture.py; capture-tutorial-save.py; finish-tutorial-evidence.py. Existing render-tutorial-report.py is an UNUSED DRAFT: must add explicit closed-run failure explanation before rendering. summarize-tutorial-evidence.py has NOT run yet; no tutorial-final-evidence.json yet.
- Manifest: C:/Users/tlori/FreeCity4/build-runs/run4-glue-tutorial-complete4-r2-manifest.json. Older report below is prior-round supporting evidence, not final shipped-fixture results.

## Report (2026-10-22, one guarded round)

Substantive commits: a504c48a (core publication/refcounts/reentry), 80570fae (native glue and harness), bcea299b (test formatting), 954e39cb (return-channel annotations). No defaults, coverage maps, ABI tables, generated ABI entries or ABI version were edited. No game files are committed. No new spec questions: TU6-TU9 supplied the implemented contracts; the remaining fixture work is validation infrastructure.

### Changes and whole-owner checklist

- **System identity/refcount/destructor:** existing four interface views retained. The borrowed executable singleton binds to the actual `cISC4TutorialSystem*`, without taking a reference; destruction restores the prior borrowed binding only if the global is still ours. Last Release no longer invents Shutdown; destructor drops the documented one window reference and retained Lua context. Normal city Shutdown remains separate.
- **Slots3/4 Init/Shutdown and messages:** existing subscriptions, cheat registration/unregistration and ordered teardown retained. Close now precedes first owner release, state clear, second release; a lifecycle reentry guard prevents a second close. The city-init GetItem/active-byte path is implemented but **not exercised by these runs**.
- **Slots5/6 start/end:** heap C allocates100 bytes; documented constructor leaves refcount0. Both owner refs and the stored window exist before native Init can reenter. Start returns native SetActive's AL changed-state result; end retains catalog-menu close ordering and pending-registry semantics. Construct/open/close/pre-save paths exercised in both games.
- **Slots7-12 context/flags/task/GUID:** core state uses a mutex rather than RefCell. C++ CoreLock explicitly suspends around synchronous native callbacks and preserves its deferred queue. Init uses no invented arguments; owner slots7/12/11 are callable through the bound interface. Native NEXT demonstrated owner slot20 updating task0->1 before the next query. Off-main getter count in the measured replacement run was0; concurrent native getter stress remains unmeasured.
- **Slots13-18 filters/hints/catalog/click/reset/event:** TU8 plain record decoding and window slots6-11 are wired with the documented argument widths/pointers/returns. Reset publishes GUID/task after the native reset callback. Direct event delivery is uint32/pop4/AL. Event1 exercised; real distinct filter payloads, hints/catalog/click/reset success/failure were not individually exercised by this probe.
- **Slots19/20 task writes:** existing whole-dword/wrapping updates retained, with native task advancement observed.
- **Cheat handler:** plain notification offsets, ToChar, tokenization/quotes, case-folding, numeric base choice, arity and ordered reset/save/step/next/quit effects implemented. `save 0`, `save 0xCC02D3A3`, `next 1` and `NEXT` measured. Quote/numeric edge cases, reset, step, save without number and quit need broader game cases.
- **Persistence:** Read-before-Init remains city-independent; Write snapshots before calling stream callbacks. New CAPI `fc4_tutorial_prepare_city_save` publishes all four TU7 save fields before metadata/save reentry; ABI=true. Existing record/conformance tests pass.
- **Registry/task owners:** native GetItem/SetActive and original window/task implementations are retained through the specified boundaries. This is tutorial-system glue; it does not replace the full registry, UI-window or task owners. No full-owner or coverage promotion is claimed.

### Measured checks

Stable full gate on source commit954e39cb: qbuild all PASS, cargo4240 passed/0failed, clippy0 diagnostics, CMake4/4, ctest67/67 (89.9s). Six touched Rust files passed qbuild fmt with absolute paths. Quick:4083 passed and CMake131/131. The initial baseline was invalidated by edits during its run; later gates fixed its stale ref1 fake and two missing return-channel annotations. Final source has not changed since the passing gate.

Game runs (same DLL SHA prefix `a4483b1d`):

- `gtc4-r1-original-20261022a`: disable=tutorial, clean2032-01-01/ref_warnings0, 63.859s. Verdict also records a40s hidden-dialog hold; no foreign exception, watchdog, RTC or overrun.
- `gtc4-r1-ours-20261022a`: enable=tutorial, clean2032-01-01/ref_warnings0, 51.094s; no foreign exception, watchdog, RTC or overrun. Native owner0502D180; window180AA114 constructed at count0, Init task0, active=true changed0, closed, pre-save active=false changed1;2619 IsTutorial calls.
- Normalized original-versus-ours tutorial trace **12/12 exact**, zero differences, recorded in `build-runs/tutorial-trace-comparison.json`. This is a pair of separate-process traces, **not a same-process shadow**. No tutorial shadow exactness claim. No turbo-only difference requiring a normal-speed rerun was observed.

### Reproducible constructed fixture/actions

Queue starts its fresh ordinary Konradshohe city (Read flag0/GUIDFFFFFFFF). `build-runs/tutorial-native-probe.script` uses these ordered actions, all dated2026-02-01 so native tutorial pause cannot strand later actions: query; cheat save0; cheat save0xCC02D3A3; start; query; cheat next1; cheat NEXT; event1; query; end0; query; save; query; resume; speed2. Start/cheat are queued to the next framework tick, outside SimNewDay. Both runs executed the queued group on2026-02-14. The script's `cheat` text is a direct TU7 notification to the queried handler, independent of debug cheat registration.

Both traces showed task present, edgeScroll0/task0 after start; `next1` unchanged; `NEXT` task1; event1 task1; end0 cleared IsTutorial/currentTask; full save succeeded. Raw logs/verdicts and the comparison helper remain under this worktree's ignored build-runs. Waiting used a single bounded local Python waiter per unique request, without qwait. No request remains outstanding.

### Residuals and next steps

**WIP, not ready:** constructed ordinary-city evidence does not meet the requested persisted tutorial-city reopen requirement. Provision an immutable shipped Timbuktu tutorial save (e.g. Big City, GUIDCC02D3A3), with component48 saved flag1/task0 and a known active native registry entry. Load it through the queue without cheat-save or manual start; show GetItem found/active and first city-init automatic window creation, second init suppression, task progress, close, save/reload, borrowed binding restoration and ref0. Never commit the save itself or edit Documents.

Then exercise the remaining UI/cheat cases above and run the coordinator's broader candidate union regression. The measured replacement run is an opt-in tutorial2032 soak with existing defaults, not evidence for every candidate subsystem. Keep defaults and coverage unchanged until those requirements pass. The queue's documented city source is FC4 Test; this round did not establish a way to select a shipped persisted tutorial fixture there.

Environment: q.py could not create Bash (E_ACCESSDENIED), so bounded PowerShell reads were used; all builds stayed on qbuild. Git emitted a denied shared packed-refs.lock warning but detached commits succeeded. Manifest `C:/Users/tlori/FreeCity4/build-runs/run4-glue-tutorial-complete4-r1-manifest.json`. No subagents, branch switches, merges, pushes or restarts.
