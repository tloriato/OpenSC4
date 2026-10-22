| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| verify-flora-records4 (native flora record verification) | `Codex` / `verify-flora-records4` (engine lane, detached HEAD) | blocked: public static-record calling/lifetime boundary absent; audit fd6ec340; FQ16 filed; cargo/clippy/ctest n/a; game n/a; no new verification credit | (1) Resolve FQ16 from already-published contracts or retain parked scope; (2) coordinator regenerates question index. |

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
