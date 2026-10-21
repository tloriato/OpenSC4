| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| glue-tutorial4 (validate/finish cFC4TutorialSystem, `enable = tutorial`) | `glue-tutorial4` / (engine lane) | ready. Gate after a6a95a3b: cargo 3756/0, clippy 0, ctest 63/63. ABI unchanged. Game runs (turbo to 2032-01-01, Konradshohe, script build-runs/tut-script.txt): `glue-tutorial4-orig-1` (original class) clean, ref_warnings=0, tutorial shadow 148/148; `glue-tutorial4-ours-1` (enable = tutorial) clean, ref0, script trace identical to orig (11/11 lines) but no shadow lines (shadow bug, fixed a6a95a3b); `glue-tutorial4-ours-2` clean, ref0, trace identical 11/11, shadow 148/148 exact. Questions TU6-TU9 filed (894c2bf5) | (1) coordinator: regenerate docs/specs/QUESTIONS.md (tools/questions.py index; engine lanes cannot run tools/); (2) union regress with --add-replace tutorial before promotion; (3) RE side: answer TU6 (registry direct calls) and TU9 (window) so a tutorial city can reopen its window |

## Report

**Owner and contract audit.** The supported family is `cSC4TutorialSystem` (city component 48, cISC4City slot 82),
implemented by glue15's `cFC4TutorialSystem` over `fc4_tutorial`. `cSC4TutorialRegistry` is a plain C++ object at
app+0x1C0 (tutorial.md TU1) with no vtable and no docs/abi/direct-calls.md entries; `cSC4TutorialTask` objects are
built by the tutorial window `cSC4TutorialUIWinProc`, which has no docs/abi rows. Neither can be implemented
without guessing, so the plugin keeps the without-window/no-entry answers and the gaps are filed as questions.
The old comments cited TU6-TU8, but those questions had never been filed.

**Changes**
- 894c2bf5: docs/specs/questions/tutorial.md: TU6 (registry `SetActive` 0x004B5D80 / `GetItem` 0x004B5D10 as
  direct calls), TU7 (the debug cheat's subcommands), TU8 (`FilterMessage`'s argument layout), TU9 (the tutorial
  window as direct calls). QUESTIONS.md is not regenerated.
- 44b67160 + 202af346: `endTutorial` now calls the 3D view slot 60 `CloseTertiaryMenu` and then slot 59
  `CloseSecondaryMenu`, as tutorial.md §4.2 / TU2 (answered) says. The view comes from the city view route
  (`fc4::propcity::View3D`). The calls are deferred out of the core lock. The slot 59 result is not read because
  docs/abi gives it as eax-or-void (ret_channel_check). Comments now cite TU2/TU4 as answered and TU6-TU9 as open.
- 202af346: new script action `tutorial query|task n|next|end 0/1|event e|reset a b` (FC4HarnessLogic.h,
  FC4Script.cpp, README script table, harness_logic_test, plugin_load_test message). It makes one interface call,
  then logs slots 8-12. There is no `start`, because the original's startTutorial would open its window.
- a6a95a3b: the tutorial shadow returned early on FreeCity4's class, even though its game-start line says
  "ON over FreeCity4's replacement". It now runs over the replacement as a self-check.

**In-game.** The original's trace matches the spec: end 0 gives AL 0, end 1 gives AL 1, event 1 gives true and
reset gives false (no window); task 7 then next gives 8; the pre-save message's `endTutorial(true)` path is hit
by end 0 followed by the save. Ours gives the same 11 lines, byte for byte. Ours logs `registry SetActive(0xFFFFFFFF, 0) not made (TU6)` twice
and the 3D view calls three times. The run is clean with ref_warnings=0 and the shadow is exact 148/148.

**Not done (blocked on questions).** Registry SetActive and the entry lookup (TU6): a loaded tutorial city keeps
is-tutorial set without a window, which is the state of quirk 2. The window and its tasks (TU9). The cheat
subcommands (TU7). Decoding FilterMessage (TU8).
