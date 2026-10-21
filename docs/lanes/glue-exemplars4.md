| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| glue-exemplars4 (exemplar/cohort resource-factory replacement, data-formats §8.1) | `glue-exemplars4` / (engine lane) | ready: replacement behind enable=exemplars (not default), native control EXACT, full gate PASS | candidate-union run before any default promotion; answers to EX46-EX50 |

## Report

What changed
- 79540b2f capi: `fc4_exrec_*` per-record exemplar parse for the plug-in factory (additive exports, with a unit test; no ABI version bump).
- 1ea955e9 plugin: `FC4ExemplarFactory.{h,cpp}`. Takes over the exemplar (type 0x6534284A, class 0x453429B3) and cohort (0x05342861, class 0x053429C8) factories at PostAppInit when `enable = exemplars` and restores the originals at PreAppShutdown (§8.1: unregister the class with slot 20, register ours under the same class). formats_shadow logs which side the game used ("game side = ORIGINAL" = native verification, "REPLACEMENT" = ours vs core).
- fd9806f3 plugin: ABI review fixes found by the full gate. abi_conformance_test failed because its name scan read the `FindObjectFactory` call as the SDK slot-22 overload (SDK 4 bytes, game pops 8). The call is the (type, out) overload, slot 21, 8 bytes, and now goes through a member pointer. ret_channel_check flagged 5 unreviewed bool reads; they now carry `[abi-ret]` notes from §8.1 (slots 19/20/21) and §4.4 (GetPropertyList).
- 64676ead question EX50 (GetPropertyList callers). EX46-EX49 were filed earlier. QUESTIONS.md is not regenerated because tools/ is off limits.

Measured
- Full gate at 64676ead: cargo test --workspace 3677 passed / 0 failed, clippy 0 diagnostics, ctest 62/62, build PASS. The first gate this round had 60/62, with the 2 failures fixed in fd9806f3.
- gex4-r3-orig-c (native control, no enable, formats_shadow=1, turbo to 2032-01-15): verdict clean, ref_warnings=0. "game side = ORIGINAL". Per-file 8957/8957 exact (148756 props). Store sets: exemplar own 8959/8959, exemplar effective 8959/8959, cohort own 388/388, cohort effective 388/388. EXACT.
- gex4-r4-build (enable=exemplars, script build-runs/gex4-build.script: 2 networks, 6 zones (4 committed, 2 short of funds), growth to 2030-01-03, R pop 15025 -> 22000): verdict clean, ref_warnings=0. REPLACEMENT shadow EXACT. reads=9546 failed=0 unsupported=0 property_lists=0. Restore at shutdown yes/yes.
- Earlier runs: gex4-r1-turbo (to 2032) and gex4-r2-normal (normal speed, to 2034) were both clean, ref_warnings=0, REPLACEMENT EXACT, property_lists=0.
- gex4-r3-orig and gex4-r3-orig-b ended in harness-error ("requested operation requires elevation" at game Start). The cause is the environment, not the DLL. The rerun under a new label (r3-orig-c) passed.

Open
- EX49 (the class and slot order of the list class) cannot be settled from the public contract. The replacement builds no list itself: it asks the game's own kept factory for an object (CreateInstance), adds the effective properties to it, and returns that object's GetPropertyList. No game path called it in any trace (EX50), so this path is still unexercised in game.
- EX46-EX48: serializable slots, slots 14/15, and message/parent ownership. Each has a marked choice in the code.

Next
- Candidate-union run with the other replacements before any default promotion. enable=exemplars stays opt-in.
- When EX49/EX50 are answered, add a direct GetPropertyList test.
