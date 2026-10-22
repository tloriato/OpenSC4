| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| core-model-effects-contractfix4 (portable model effects contracts) | Codex / core-model-effects-contractfix4 | in progress | Audit Consumer 1, correct production owner, focused checks and final full gate |

## Handoff
- Goal: implement unambiguous graphics-formats 3.11 contracts; no native/glue/default promotion.
- Reserved detached worktree reused; initial status clean and main.. empty.
- First q.py attempt failed because Bash service access is denied; direct PowerShell reads work.
- Read 3.11.1-4 and dependency coordinator review; first correction 5548319b separates Shutdown and attachment; same pointer is no-op and attachment does not discover.
- Baseline qbuild all launched (session 60676); collect its completed result/log before reporting.
- Implemented explicit reference primitives, shared record storage leases, CRT byte-table boundary, controls/recreation/failures and transform/motion updates; added independent core traces and adapted formats integration.
- GF25 appended for unidentified empty-key-map check. Existing GF19-GF24 preserved open.
- Rustfmt patches restricted to four touched Rust files; wrapper fmt needs absolute paths (relative attempt failed).
- Next: collect baseline and quick gates; fix compile/test diagnostics; inventory report for 23 roles, stable-source final all, then ready queue.
- Storage leases extend portable allocation lifetimes without native AddRef; destruction drops host wrappers without foreign Stop/Release. No native adapter or equivalence claimed.

## Contract reconciliation (working)

The 23 attributed bodies / 8,406 bytes identify roles, not whole-owner or native completion. All named operations below are production core behavior; primitive providers supply no owner decisions or recorded owner outputs.

| Published role | Production behavior or explicit residual |
|---|---|
| 0 Init | `init`: true, detached no-op; discovery without clearing or starting. |
| 1 Shutdown | `shutdown`: clear effects while instance alive, then explicit instance Release, true. |
| 2 SetModelInstance | `attach`: pointer identity no-op (including null); otherwise Shutdown then explicit acquire; no discovery. |
| 4 motion update | Started nonnull entries only, SetTransform then parameter 15 / vector of three binary32 speed products / count 1. Unused native first word has no positioning API. |
| 5 update | Instance transform and temporary animation query/frame/release bracket traversal. Native true is `UpdateReport.success`; checked diagnostics are portable additions. |
| 6 StartAll | Query primitive IsStarted and forward full control only when false. |
| 7 StopAll | Query primitive IsStarted and forward full control only when true, including repeated deferred stops. |
| 8 AddEffect | Exact-key replacement Stop(1)/Release before creation; preserve old storage through creation; erase on false; no extra effect acquire, Start or transform. |
| 9 AddEffectAtRegion | Core exact region lookup; missing instance/model/container/key still creates with null records. No model/container reference operations. |
| 10 GetEffect | Exact unsigned UTF-8 key-byte lookup, borrowed nullable effect. |
| 11 RemoveEffect | Nonnull Stop(1)/Release then erase; absent/null handled without invented scalar result. |
| 12 StartEffectsNamed | CRT byte-table name comparison; repeated starts; boolean match/fallback result; only low byte enables creation. |
| 13 StopEffectsNamed | IsStarted before name comparison; started matches only; void. |
| 14 ToggleEffectsNamed | Name matches then IsStarted, Stop/Start with same control; fallback only with no nonnull match; void. |
| 15 RecreateEffectsNamed | Manager-null no-op; Release without Stop; requested spelling used for creation; failure retains null effect/name/records and continues; matched failures suppress fallback; void. |
| 16 InitEffectsList | Core prefix/name extraction and exact key-order traversal; failures continue; unrelated entries remain; repeated discovery replaces keys. Unidentified empty-map behavior withheld (GF25). |
| Constructor | `new`/Default: null instance and empty BTreeMap. Native sentinel allocation unsupported. |
| Clear helper | Key-order Stop(0)/Release/null for every nonnull effect, then all entries erased. |
| Erase-node helper | BTreeMap removal supplies logical erasure after explicit effect operations. Native rebalance, string destruction order and allocator mechanics unsupported. |
| Copy-entry helper | Native shallow effect/vector pointer copy without acquire is documented; unsafe duplicate owning entries are not exposed. No helper-equivalence claim. |
| Make-entry helper | Core creates separate key/name strings and a record lease, transferring creation's effect reference. Native payload layout/shallow-copy ABI unsupported. |
| Link-node helper | BTreeMap owns insertion/order/count; 64-byte native allocation, hints and rebalance unsupported. No foreign effect calls added. |
| Unique-insert helper | Exact-key uniqueness is provided by BTreeMap; no native iterator/result-out ABI or scalar result invented. AddEffect's replacement policy is separate. |

Shared bodies outside the count: attached-instance getter borrows without acquire. Destruction clears host nodes/wrappers before explicit instance Release and calls neither foreign effect Stop nor Release. Recursive native node destruction, sentinel freeing, and exact tree-dependent destruction order are unsupported; no low-level APIs were invented.

## Boundaries retained

- `RecordVector` leases shared immutable storage without copying records or acquiring model/record references. Region leases keep the adapter allocation alive and reborrow the same stable vector by exact key. This can extend **portable allocation** lifetime; it is not a native vector ownership claim. Arc operations and wrapper Drop are not used as evidence of native AddRef/Release.
- Providers must keep model keys/vectors stable and must not attach foreign Release to effect-wrapper Drop. Native references intentionally receive no Release during owner destruction alone. Clients requiring cleanup must call Shutdown. No native adapter/glue exists here to validate that convention in the game.
- Parsed strings are valid UTF-8. Exact keys use unsigned bytes; effect names use a caller-supplied current CRT-locale byte-lowering table and stop at NUL. Default table is explicitly C locale, not asserted native thread locale. Non-UTF-8 strings, CRT thread-locale refresh and real interface bindings remain outside this core.
- Null record vector uses local identity; a nonnull empty/out-of-range vector reports key/frame/count and skips both output operations. This is a safe portable diagnostic, never a native clamp/fallback/false result. Wrapped 32-bit record addressing/computed-zero pointers are not represented.
- Loaded records have flags 5; local-zero copies instance; instance-zero copies local at scale 1; both nonzero compose and retain instance scale/flags OR 5; outgoing version is always 0. No camera, visibility, global/local or owner-index APIs were added.
- Normalization follows `(z²+y²)+x²`, square root/reciprocal and separate binary32 component stores using the existing 24-bit numerical primitive (scaled for tiny squared axes); angle multiplies binary32 -0.5 before host sin/cos. GF13 zero-axis identity remains an explicit portable choice. Full x87/transcendental, NaN-payload and all downstream matrix-store equivalence remain unverified.
- AddEffect's unavailable-manager path is an explicit safe-domain assertion, not invented native graceful failure. Creation success and nullable output are separate; false/non-null receives no foreign Release. True/null is retained safely; unsafe native dereference behavior is not asserted.
- Direction is three binary32 values; speed has no asserted physical unit, normalization, rotation or conversion. No position effect is synthesized.

## Open questions

GF19–GF24 remain open, unedited and unanswered. Remaining scope: inventory/helper/native-equivalence and independent native vectors (GF19), real foreign-reference/lifetime validation (GF20), current CRT locale and raw-byte/interface domains (GF21), exact numerical/transform behavior including GF13 choice (GF22), unchecked frame/address domain (GF23), and physical speed units/caller integration (GF24). Their original bracketed descriptions record the parent implementation and are not this correction's behavior. GF25 is appended in `docs/specs/questions/graphics-formats.md`; coordinator must regenerate the index because this lane cannot run `tools/` or edit the generated index.

## Validation (pending)

- First substantive correction committed at `5548319b`, before call 30; implementation milestone `20956ca7`.
- Core fixtures currently 24 tests (parent 16), plus the existing one formats integration test, with independent expected traces/geometry derived from the published contract.
- Initial `all` began on the clean baseline; workspace cargo tests completed, but clippy observed the in-flight test pointer borrow error. Final stable gate will supersede this mixed-source baseline attempt.
- First quick run: E0502 in same-pointer test fixture; dependents compiled. Fixed by comparing a thin address. Its CMake configure also hit Ninja recompaction permission denial while the baseline build was outstanding. No build-system changes made; serialize subsequent gates.
- Touched-file fmt passes with absolute paths (four files); relative-path attempt could not resolve files. Final fmt/gate counts will be recorded here before ready.
- No ABI exports, shared defaults, native glue, coverage maps, tools or game files changed. No game requests/runs; no native/shadow/whole-owner promotion.
- GF19-GF24 remain open; no game runs, ABI edits, RE sources, subagents or other-worktree edits.
- Manifest: C:/Users/tlori/FreeCity4/build-runs/run4-core-model-effects-contractfix4-r1-manifest.json.
