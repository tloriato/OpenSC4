| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| verify-flora-records4 (occupant native verification, round 2) | `Codex` / `verify-flora-records4` (engine lane, detached HEAD) | in progress | Independent occupant comparison, focused/full gate, unique guarded2032/ref0/save/reload controls. |

## Handoff

- Goal: owned native factory versus independent Rust occupant; full specified queries/mutations/serialization/lifetime; read-only natural snapshots.
- Prior round d5c82b44 parked static-record FQ16; excluded this round, no static getter calls or static coverage credit.
- Start HEAD d5c82b44 detached; no branch/ref changes. Prior report below is historical.
- Baseline all: workspace tests and clippy passed, CMake/ctest 68/68 passed (logs build-runs/qbuild-*, fc4verify-*).
- q.py cannot start Bash (E_ACCESSDENIED); use capped local PowerShell commands.
- Done: a25cef82 independent Rust occupant codec committed before call 30; FQ17 partial-state/override question appended.
- Done: 6f3a7fad typed probe; quick 3452 tests PASS, dependents compiled, build PASS (tree changed; stable gate still required).
- Game vfocc-r2-d1-20261022-1758 clean/ref0: 585 comparisons/0 differences, 340 FQ17 refusals, 2607 natural occupants/15642 read-only queries, invalid0.
- Stable full all on 17369af7: cargo 4941/0, clippy0 diagnostics, CMake4/4, CTest68/68 (242.2s); no tree-changed marker.
- Guard2032 vfocc-r2-guard2032-20261022-a clean/ref0: each probe810/0, 340 refusals,70 undefined returns,0 fallbacks. Script full save ok1 (11484ms), date2031-01-11.
- Immutable DLL build-runs/dlls/vfocc-r2-final/FreeCity4.dll SHA256 daed6fc7e7d83e3ed793787adbf4c20061c5b289220c1743b82ebaad2384a8aa.
- Saved isolated-slot game-data fixture main build-runs/fixtures/vfocc-r2-save-20261022-a.sc4 SHA25682de3ffe2ac3e7b11ec36ce04917203d1afd82ad6d8286a2d42ce1ea396d4390 (never committed).
- Reload vfocc-r2-reload2032-20261022-a clean/ref0 to2032-01-09; fixture hash matched; probes2x810/0, natural2575 then2569/invalid0.
- Next: wait same-fixture probe-disabled2032 control; final report with remaining FQ17 coverage gaps.
- Expanded probe compares native release counts with independently allocated Rust atomic reference model and destroys both at zero.
- Probe opt-in flora_occupant_verify=1; enable simulator, keep flora_shadow=0 (avoids static-record getters entirely).
- No game request outstanding. New labels unique; wait without qwait.

## Result

Stopped at the required acquisition audit, concluded within the first 10 tool calls. The joint original-native versus Rust occupant/static-record comparison cannot meet the requested public-boundary and lifetime constraints with the published contracts. No engine implementation was changed. The audit and FQ16 were committed as fd6ec340 on the reserved detached worktree, starting from eaae6c54.

## Published boundary and blocker

- `docs/specs/flora.md` sections 3.4, 11.4 and 13.2 publish occupant CLSID `0xA9C05C85`, QI `0xC83DD057`, the getters/setters, and counted occupant ownership. `engine/plugin/cFC4Flora.cpp` already acquires original occupants using `GetClassObject` and QI with tracked references. This portion has a safe boundary; it does not establish static-record verification.
- Section 11.1 slot 7 publishes `GetStaticFloraData(type)` as a borrowed concrete pointer. The SDK's `cISC4FloraSimulator.h` and `cISC4FloraOccupant.h` forward-declare `cSC4StaticFloraData`; the latter exposes only a setter taking the pointer. They provide no public full-record query or lifetime interface.
- Section 13.2 and `docs/abi/layouts.md` (Static flora record, Memory) publish raw layout/counting requirements: no AddRef function, direct reference-count increments and non-virtual Release. Those facts support the existing compatibility block, but using native fields or address-based non-virtual calls is outside this task's allowed boundary. This audit read published documentation and existing source only, never native memory or executable bytes.
- Existing `FC4FloraShadow.cpp` seeds simulator comparisons with `FloraBlockValues` extraction of native records. Extending that extraction or feeding those results back as Rust inputs would not supply the independent typed-input algorithm comparison requested here. Occupant bounding-box projections also cannot expose the complete table, flags, families, cost and record lifetime.
- The current core `FloraOccupant` is a value snapshot and the flora host callbacks delegate occupant actions to original interfaces; this existing delegation is not an independent occupant record/lifetime oracle.

## Validation and accounting

Builds and games were not run: the task explicitly requires stopping if the safe native endpoint is absent and prohibits a code-free baseline build. Cargo, clippy, ctest, native comparisons, constructed/natural event counts, differences, refusals, fallbacks and unaccounted events are **n/a (no comparison instrumentation executed)**. No creation/query/destruction/save/reload/shutdown results or ref-warning measurements were obtained. No claim of exactness, a successful zero-event comparison, owner coverage, or extension of existing simulator71/71 coverage is made.

No C ABI exports, ABI tables, defaults, replacement keys, shared glue files or game files changed. No game requests are outstanding. No mutation was replayed, RNG reseeded, private state inferred, snapshot resynchronized, or second Rust oracle introduced.

## Question and next steps

FQ16 is appended in `docs/specs/questions/flora.md` (next free ID from `docs/specs/QUESTIONS.md`). It asks for an already-published callable static-record input/output and lifetime boundary, or explicit confirmation that this scope must remain parked; it requests no new RE. The coordinator must regenerate the question index: this lane is forbidden to run `tools/` or edit the generated index. Queue state is wip, not ready, because the native-verification objective remains blocked.
