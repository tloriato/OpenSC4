| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| glue-tutorial-complete4: native tutorial replacement | Codex / `glue-tutorial-complete4` (engine lane, detached HEAD) | in progress: final shipped-fixture validation round; preserved6d6bb03d; ABI=true; no coverage promotion | Observe original/ours automatic init on shipped fixture, capture save/reload, broaden actions, final gate and union. |

## Handoff

- Goal: final implementation-family round; shipped BigCityTutorial automatic init/reopen, matching original/ours actions, persistence reload and candidate union.
- Preserved:6d6bb03d TU6-TU9 implementation; prior full gate cargo4240/clippy0/ctest67/67 and constructed trace12/12 (separate processes, no shadow).
- Next: inspect harness request/action grammar and existing tutorial probe; queue unique original fixture run with early fixture-date query before any manual start.
- Fixture: `C:/Users/tlori/FreeCity4/build-runs/fixtures/tutorial/BigCityTutorial.sc4`; immutable version2/flag1/task0/GUIDCC02D3A3; read adjacent provenance.
- Capture successful isolated slot save before slot reuse to unique file under main build-runs/fixtures/glue-tutorial-complete4; only data snapshot/JSON writes authorized there.
- No outstanding request; no new questions; no defaults/map/ABI version changes; no subagents/branch changes.
- Manifest: `C:/Users/tlori/FreeCity4/build-runs/run4-glue-tutorial-complete4-r2-manifest.json`.
- Earlier report below is supporting prior-round evidence, not shipped-fixture validation.

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
