| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| verify-fsh-native4 (independent native FSH verification) | `verify-fsh-native4` / (engine lane, detached worktree) | blocked; stable all gate at 5ac5a2b7: cargo 4947/0, clippy clean, ctest 68/68; GF25–GF27; native observed/exact/diff 0/0/0; no game runs or ABI changes | Publish typed native acquisition/output/lifetime contract, then implement and run native/current40 controls. |

## Result

Contract blocked. This round delivers append-only questions GF25–GF27 and this report. Engine/plugin code, ABI tables, C ABI exports, defaults and generated ABI entries are unchanged. The reserved detached worktree was reused; no branch switching, main edits, subagents, RE, game execution, network or GPU implementation occurred.

The acquisition/output/lifetime review reached its decision within the first 15 orchestration calls. No native verification code was justified by the published contract, so none was added to meet the call-30 deadline artificially. The requested guarded 2032/ref0 native and current40 controls were not submitted: there is no supported native comparator or changed plugin to exercise. State is blocked, with no owner/algorithm/metadata/pixel verification promotion.

## Published contract review

| Route | Established from allowed sources | Missing or conflicting contract | Outcome |
|---|---|---|---|
| Isolated original FSH factory/resource | graphics-formats §2.1 names class 0x3AB50E2A, resource types 0x7AB50E44/45 and the type-45 creation refusal; SDK publishes cIGZPersistResourceFactory, cIGZPersistResource and serial-record interfaces; §6 and vtables.tsv slot numbers agree after accounting for their four-byte origin difference | SDK omits the SerialRead extension; isolated creation arguments, exact extension interface and failure/success ownership are not fully published | Not called; GF25 |
| Read-only native image/shape outputs | graphics-formats §2.7 specifies acceptance/dimensions/frame consistency; §2.8 specifies level layout; texture-bindings §1.5 identifies resource IID 0x1AB50C3F and image-info getter slot 3 | No typed image-info getter/interface, complete image/level-table storage ABI, pointer bounds or lifetime; no complete typed cS3DShapeFile construction/open/getter/destruction route | Not called; GF26 |
| Texture-binding fallback | graphics-formats §2.10 identifies service 0x1AC0E11A/IID 0xFAC0E219; texture-bindings §1.5/§1.10/§2 describes binding outputs and counted lifetime; foundations §16 reconciles SDK overload source order with compiled info/ID/key slots | Both output structures are only forward declarations; SDK binding class has no callable Release method; cache/upload side effects prevent treating acquisition as a pure observation | Not called; GF27 |

Sources checked: selected graphics-formats §§2.1–2.2, 2.7–2.10, 6, 10–11; relevant texture-bindings §§1–2; foundations §§16, 17.6; docs/abi README and owner vtable rows; public SDK resource factory/resource/record/serializable/binding headers. An initial overload-order concern was corrected using the published MSVC reversal rule; the slot-origin difference also accounts for the factory numbering difference. These are not remaining blockers. Addresses and pop sizes were treated as identifiers, never as sufficient function signatures or storage layouts. Original resource/QFS transport would be foreign I/O and would not prove our QFS implementation.

## Measurements and coverage

| Measure | Count/domain |
|---|---|
| Contract routes reviewed / declined | 3 / 3; static review only |
| Native executions / observed outputs / runtime refusals | 0 / 0 / 0 |
| Native exact / diff / fallback attempts / resync | 0 / 0 / 0 / 0 |
| Independently compared preserved FSH inputs | 0; no Rust/native paired cases |
| Acceptance / dimensions / mips / format / pixels | 0 observations in each domain; no coverage claimed |
| Resource references acquired / released by this lane | 0 / 0; no native resource/host mutation |
| Game requests / outstanding / reference-warning measurement | 0 / 0 / unmeasured |
| Original runtime cap consumed | 0 of 3 |

Zero differences are not exactness evidence when no comparison ran. The existing Rust parser/decoder remains implemented; this round supplies no native verification evidence for it. S3D models and DX7 drivers remain outside this lane.

## Validation

Baseline `qbuild.py --wt <WT> all` passed: cargo 4947 passed, 0 failed, 0 ignored; clippy clean; CMake PASS 208/208 edges; ctest 68/68, 0 failed. fc4verify noted the documentation/HEAD changed during that run; engine inputs were unchanged. A second, stable final `all` at 5ac5a2b7 passed cargo 4947/0/0, clippy 0 diagnostics, CMake 2/2 edges and ctest 68/68 (74.8 seconds); fc4verify PASS (75.6 seconds), with no tree-change warning. Final edits only correct and finalize documentation. Logs: `build-runs/qbuild-test.log`, `qbuild-clippy.log`, `fc4verify-build.log`, `fc4verify-ctest.log`; baseline cargo/ctest logs preserved as `baseline-qbuild-test.log` and `baseline-fc4verify-ctest.log`. No Rust files were touched, so fmt and focused quick tests do not apply. q.py could not start Bash (E_ACCESSDENIED); bounded native PowerShell reads and the quiet build wrapper worked. Git commits succeeded despite a shared packed-refs.lock permission diagnostic; no shared ref/config/maintenance action was requested.

## Questions and next steps

GF25–GF27 and the explicit overload/slot-origin clarification are appended to `docs/specs/questions/graphics-formats.md`; QUESTIONS.md and ABI tables are untouched. Coordinator owns index reconciliation. Commits: 2696c9d9 initial handoff; 6d96bf1d contract questions; 2e0c380e baseline progress checkpoint; 5ac5a2b7 report checkpoint and stable gate input.

Publish the reconciled typed acquisition/read/output/lifetime/support-image contract before another implementation attempt. Then build a bounded original-resource observer using identical preserved, prevalidated FSH bytes independently parsed/decoded by formats/src/fsh, audit every acquired/created reference, and run the requested focused/full gates and immutable unique-label guarded 2032/ref0 native/current40 controls. Keep native acceptance, metadata, compressed payload and decoded-pixel coverage separate.
