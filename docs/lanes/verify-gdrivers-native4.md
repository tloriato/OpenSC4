| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| verify-gdrivers-native4: director and attributed helpers | `verify-gdrivers-native4` / (engine verification lane, detached worktree) | blocked: GD2; baseline cargo 4987/4987, clippy 0 diagnostics, CMake 212/212, ctest 68/68; no native comparison or game run | 1. Resolve GD2 acquisition/lifetime and independently owned native fixture contracts. 2. Regenerate the question index in the coordinating session. |

## Result

Blocked at the task's acquisition boundary. GD2 was filed within the first 15 tool calls and committed in `6a1b6597`. No production binding was implemented: a fully typed public acquisition and lifetime contract for the original graphics director and independently owned native helper fixtures is not published in the reviewed sources.

## Acquisition evidence

- SDK `cIGZFrameWork.h:192` exposes `GetCOMObject`. `cIGZCOM.h:108,115` declares array and callback library enumeration; neither publishes ownership/lifetime guarantees or how to reach this built-in child. Array capacity/count behavior is not specified there.
- `cIGZCOMLibrary.h:39` declares a virtual `GetDirector`; it does not establish that an enumerated library exposes director ID `0x64B2A903`, nor a route from a containing director to that child. The callback overload supplies concrete `cGZCOMLibrary*`; its nonvirtual getter is not a documented child acquisition contract.
- `cIGZCOMDirector.h` provides identity, interface/class query and class enumeration, but no child enumeration or director-ID lookup. The published primary ABI slots agree with the parent spec's section 6.1; a vtable inventory does not supply an object acquisition endpoint. Section 6.2 specifies QueryInterface for an already valid director, not a way to find it.
- Parent spec sections 2 and 3.1 name the private singleton accessor and containing director's `AddDirector`. Neither supplies an authorized public call sequence or an independently owned native fixture constructor/destructor/helper API. No private address, guessed object layout, global registration, or application registry mutation was used.
- The existing `engine/core/src/gdrivers_director/README.md` explicitly describes a pure core without C ABI, plugin, game/shadow or replacement wiring. Its paper-vector conformance evidence is not a native comparison result.

## Baseline validation

Ran the required `qbuild.py --wt C:/Users/tlori/FreeCity4/.claude/worktrees/verify-gdrivers-native4 all` once, beginning before implementation review:

- Workspace tests: PASS, 4,987 passed, 0 failed, 0 ignored (`build-runs/qbuild-test.log`).
- Clippy with warnings denied: PASS, 0 diagnostics, 48.67s (`build-runs/qbuild-clippy.log`).
- CMake build: PASS, 212/212 edges, 394.8s (`build-runs/fc4verify-build.log`).
- Full CTest: PASS, 68/68, 0 failed, 729.8s (`build-runs/fc4verify-ctest.log`). Wrapper final verdict: `qbuild: PASS`.

The wrapper noted that the tree changed during the run: only this lane report and the question file were committed. An explicit Git comparison confirms engine source is unchanged from base `ce5ce28e9244237a71ed61edacf0fbaecb812c6e`. These are baseline results, not a stable plugin-change release gate or native owner verification. Progress was retained in `55dec2b9` and `e2422cbf`.

## Scope and measured limits

No game requests were submitted; no request or verdict is outstanding. Clock24/house40 controls, 2032 progression, reference warnings, save and shutdown are unmeasured for this lane because the safe binding prerequisite failed. No native shadow, production transition comparison, whole-owner completion or runtime replacement is claimed. No rendering callback, graphics factory, driver initialization/shutdown, default, tool, map or ABI table was changed. C ABI exports are unchanged.

GD1 remains the explicit boundary beyond the specified 193-bucket regime; GI1 remains the unknown matrix extent/construction-order boundary; GI2 remains the pool storage/deallocation contract boundary. No new out-of-specification registry, matrix or pool operation was introduced.

## Questions and next steps

GD2 is appended to `docs/specs/questions/renderer-gdrivers-director.md`: publish a fully typed public route to the existing original director, reference and callback lifetime rules, and safe creation/destruction/helper-call contracts for independent native fixtures. Resolve that contract before another native verification attempt. This lane does not request further RE or another graphics-owner round.

The generated `docs/specs/QUESTIONS.md` was not edited or regenerated: this task forbids running `tools/` and allows edits only to the question file under `docs/specs/`. The coordinating session can regenerate its index. The merge queue entry is `wip`, not eligible for a ready promotion.
