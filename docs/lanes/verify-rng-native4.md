| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| verify-rng-native4: independent native RNG verification | Codex / `verify-rng-native4` (engine lane; detached HEAD) | blocked on RN4/RN6; cargo passed; clippy passed; baseline CMake/CTest completion not yet established; native comparisons 0; game runs 0; ABI unchanged | Publish a complete owned typed native binding or a read-only stream with independently proven initial inputs before another verification attempt. |

## Result

Blocked at the published-binding prerequisite. Audit milestone: `17f2afaf`. This round changes documentation only; no production, plugin, shared map/default/tool or ABI edits. The reserved worktree was reused with detached HEAD and committed normally.

## Exact binding gap

- Public SDK `engine/third_party/gzcom-dll/gzcom-dll/include/cISC4AdvisorSystem.h:45` exposes only `intptr_t GetRandom(void)`, with the comment that it points to an RZRandom that cannot yet be recreated. It supplies no typed owned generator or callable generator operations.
- `cIGZCOM.h:55` publishes generic `GetClassObject(uint32_t clsid, uint32_t iid, void** ppvObj)`. No published RNG CLSID/IID or factory acquisition path for either exact owner was found. A generic factory is insufficient to establish construction, storage size/alignment, ownership, disposal, or method calling/argument/return conventions.
- Exact-owner/method searches of the public SDK and `docs/abi/` found no callable `cRZRandom`/`cRZRandomSuper` contract. Incidental RNG references in another owner's ABI prose do not establish one. Foundations 6.1 algorithm addresses are not construction/lifetime/native method bindings.
- No supported read-only native capture with independently proven initial seed/state was identified in the permitted sources. Section 6.2 says RNG state is not saved; a city save does not prove initial native RNG state or intervening draw order. Section 6.3's derived session seeds are FreeCity4 choices, not native seed provenance.

## Production and precision audit

The production `engine/core/src/rng.rs` contains persistent `RzRandom` and `RzRandomSuper`, including Gaussian cache and weighted-choice operations. `seeding.rs::process_super` shares persistent Rust state across city loads. These are the objects a later valid native comparison must exercise; no private verifier algorithm was introduced. Sections 6.1-6.3 and 18 rounding were the behavior sources, alongside production code and public SDK/ABI. No forbidden analysis material or game executable was accessed.

Section 18.3 requires each arithmetic result rounded to a 24-bit significand while preserving unrounded loaded operands. No native/clock24 profile was declared or measured. The production Gaussian bounded retry/fallback and runtime logarithm compatibility choices remain visible and unchanged. With zero comparisons, there is no measured equality, mismatch, refusal set, or Gaussian equivalence to claim.

## Validation and questions

- Baseline command: `python C:/Users/tlori/FreeCity4/.claude/lane-bin/qbuild.py --wt C:/Users/tlori/FreeCity4/.claude/worktrees/verify-rng-native4 all` (started before the audit).
- Observed gate: cargo passed; clippy passed; baseline CMake/CTest completion not yet established. Logs: `build-runs/qbuild-test.log`, `qbuild-clippy.log`, and `fc4verify-*.log`. If CMake/CTest was still running at stop, its eventual result must be collected from this worktree; no passing result is inferred.
- No Rust changes, so no rustfmt or quick rebuild needed. No plugin change, game request, immutable DLL/PDB profile, HOUSE40 control, save/shutdown/reload chain, or ref-warning measurement was made. Native comparisons: **0**. No shadow/default/replacement or native verification credit.
- Existing **RN4** remains the owned binding prerequisite. Appended **RN6** to `docs/specs/questions/foundations.md` for independently proven read-only capture provenance. The generated questions index was not edited and prohibited tools were not run.

## Resume conditions

Publish either the complete typed owned native construction/lifetime/method binding requested by RN4 or the read-only capture contract requested by RN6. The latter must prove generator identity/lifetime, initial LCG/multiplicative or MT state/index, Gaussian cache, clock inputs, intervening draws, method arguments and precision independently of outputs. Then compare actual persistent production generators over independent fixed inputs, every specified operation/state advance, Gaussian cache and weighted boundaries, with a separate measured 24-bit profile; preserve all choices, mismatches and refusals. Run the requested immutable-artifact native/clock24 and exact HOUSE40 controls through 2032/ref0/fullsave/shutdown/reload only after the prerequisite is met.

Manifest: `C:/Users/tlori/FreeCity4/build-runs/run4-verify-rng-native4-r1-manifest.json`. No outstanding game request. No further native attempt authorized by current published contracts.
