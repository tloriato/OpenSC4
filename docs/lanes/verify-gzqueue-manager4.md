| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| verify-gzqueue-manager4: original queue manager verification | Codex / `verify-gzqueue-manager4` (engine lane) | blocked on GZ11; train99 ancestry confirmed; cargo 4987/4987, clippy 0 diagnostics, CTest 68/68; native constructed 0, natural 0; ABI unchanged | 1. Await published GZ11 contract. 2. Coordinator must authorize any resumption; no verification credit. |

## Result

Stopped at the required published-contract audit, within the first 15 tool calls. No plugin or engine implementation was added. Commit `fe34e849` records the precise acquisition/interface gap as GZ11 in `docs/specs/questions/framework-gz-services.md`.

Train99 genuinely landed before this round: starting HEAD and main were `8e80ddfa`, with source merge `e29a1560` an ancestor of both. The production file audited was `engine/core/src/gz_messages/manager.rs`, last changed in `d60acdea`. No merge, rebase, branch/ref change, or source substitution was performed.

## Published contract gap

Spec sections 1 and 3 identify service `0x64FA8434`, IID `0x652294BC`, vtable `0x00AD1B80`, queue operations in slots 3-8, own-IID QI returning an AddRef'd pointer, and counted queue map/output ownership. Section 2 also specifies queue AddRef/Release behavior.

The permitted public SDK has no queue-manager interface declaration. Targeted searches of `docs/abi/interfaces.tsv` and `docs/abi/vtables.tsv` found no manager rows by class name, vtable, or operation addresses. The spec gives behavioral slots but no complete typed x86 manager declaration, including slots 1/2, exact signatures, calling convention and interface adjustment. A native verifier must not invent those details.

`cIGZFrameWork.h` declares generic `GetSystemService(uint32_t, uint32_t, void**)`; `cRZSysServPtr.h` releases its generic result. These headers do not publish the manager-specific borrowed acquisition/application lifetime/separate QI-hold contract required by this task. No supported public owned-manager factory is published; a constructor address is not sufficient. GZ11 requests this narrow entry/typed-call contract, without expanding into GZ8/GZ9/GZ10 server, base, platform, or whole-owner completion work.

## Validation and observations

One baseline `qbuild.py --wt <WT> all` completed successfully after question commit `fe34e849` (documentation-only changes from the starting source): Rust workspace tests 4,987 passed, 0 failed, 0 ignored; clippy exit 0 with 0 diagnostics; CMake build 212/212 edges; full CTest 68/68 passed, 0 failed. `fc4verify` reported PASS. Logs are `build-runs/qbuild-test.log`, `qbuild-clippy.log`, `fc4verify-build.log`, and `fc4verify-ctest.log`. The final report changes documentation only. No Rust files were touched, so fmt is not applicable.

Preflight contract refusals: 1. Native manager operations: 0. Constructed comparisons: 0. Natural comparisons: 0. Native fault/difference counters were not exercised. No shadow exactness, whole-owner, replacement, or other verification credit is claimed from 0/0.

No game requests were submitted and no labels are outstanding. No native pointer was acquired, retained, or released. No queue IDs were inspected or mutated. No global lifecycle, foreign queues, service registrations, dispatch, or threads were touched. House40/full-save/shutdown controls were not run because native verification stopped before any plugin change. C ABI exports are unchanged.

## Questions and next steps

GZ11 was appended to `docs/specs/questions/framework-gz-services.md`; next-free ID was read from the generated QUESTIONS index. The index was not edited or regenerated, since this lane may edit only the per-spec question file and may not execute `tools/` scripts. The coordinator can regenerate the index through its authorized workflow.

Await a published GZ11 contract and coordinator authorization before reconsidering a native implementation. This round is stopped; no automatic follow-up round or private ABI inference. Any future probe still needs fresh slot/GO checks, collision-refusing private IDs only, balanced documented holds and complete own-entry cleanup before city load/shutdown, independent operations against the actual production Rust manager, and the immutable guarded actual house40 controls required by the brief.
