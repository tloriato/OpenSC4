| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| verify-save-records-native4: native save-record verification | `verify-save-records-native4` / engine lane, detached worktree | blocked: SV2; audit 05baaf3a; cargo 4987/4987, clippy 0 diagnostics, C++ 212/212, ctest 67/68 (cargo_test failed); no game runs or native comparisons; ABI unchanged | 1. Publish or explicitly refuse isolated SC4 segment/serializer endpoint and lifecycle under SV2. 2. Coordinator may regenerate question index. |

## Report

Native verification is blocked by SV2. Commit `05baaf3a` records the contract audit and question. No production, plugin, C ABI, default, map, tool or shared ABI files changed. No private constructor, address call, verifier clone or live-city mutation was attempted.

The public SDK provides `cISC4DBSegment` IID `0x499899DA`, `AsIGZPersistDBSegment`, `GetCOMSerializer`, and paired `OpenIStream`/`CloseIStream` methods. The input stream has IID `0x4999A8F5`. Those typed operations require an existing segment. The SDK declares no SC4 segment class factory ID or acquisition service; `cISC4COMSerializer` is only forward declared. Generic directory/packed segment and serial-record adapter factories do not establish an SC4 serializer endpoint. No published direct-call/allocator/layout contract supplies the missing isolated construction and cleanup route. Foundations section 2.7 describes implementation addresses and the table lifetime, but those facts do not authorize guessing private storage or construction. Its two-argument `ReadGZSerializable` correction must also be respected by any eventual shim.

The audit read foundations sections 2.1-2.6, the relevant section 2.7 ownership/signature text, sections 3.3-3.6 and framework slot-table context; save-records sections 5, 9 and 10; the public typed segment, stream and serial-record SDK declarations; and matching published ABI records. `engine/core/src/save.rs` implements the simulator v4 payload rather than the general SC4 class store. `stream.rs` exposes the production stream traits and portable byte implementations. Neither portable fixtures nor parsed original answers would satisfy the requested independent native comparison.

| Requested evidence | Result |
|---|---|
| Isolated original SC4 segment/COM reader and ownership lifecycle | Unavailable in published contracts; SV2 |
| Immutable city copied and hashed for native reads | Not attempted after endpoint audit blocked the task |
| Identical raw bytes and identifiers into original and production methods | 0 comparisons; no native shadow claim |
| Known versions, length/bounds, failure and roundtrip probes | All unattempted; no missing/corrupt/refused case discarded or reported passing |
| Native/clock24 control | No request submitted |
| Exact HOUSE40 controls through 2032, ref0, fullsave and shutdown | No request submitted |
| Supported reload | Not attempted |
| Focused gate / rustfmt | Not applicable: no code edits |
| Baseline full gate | FAIL: workspace tests 4987/4987, clippy exit 0 / 0 diagnostics, C++ build 212/212, ctest 67/68; cargo_test failed |

The required `qbuild.py all` completed with a failing baseline. Its ctest `cargo_test` failed compiling i686 Rust test targets through sccache (exit 1; no compiler source diagnostic in the captured failure). The 67 other ctests passed. The C++ build took 182.3 seconds; ctest took 324.3 seconds. `fc4verify` also marked the run as tree-changed because the audit documentation commit changed HEAD during the run. This is not a passing gate on the report tip. No production source changed during the gate, no failure was suppressed, and no code fix or rerun is claimed. Full logs: `build-runs/qbuild-test.log`, `qbuild-clippy.log`, `fc4verify-build.log` and `fc4verify-ctest.log`.

SV2 is appended only to `docs/specs/questions/foundations.md`, using the next free foundations SV ID in the generated question index. The index was not edited and its generator was not run: this lane is prohibited from running `tools/` scripts or editing other spec files. The coordinator can regenerate it through the normal merge workflow.

Next: publish or explicitly refuse the endpoint requested by SV2. If supported, supply complete acquisition, initialization, immutable input binding, raw-byte and raw-identifier handling, serializer prerequisites, output-record ownership and success/failure cleanup contracts before another native attempt. No implementation or coverage promotion is justified by this round.
