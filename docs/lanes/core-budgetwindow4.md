| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| core-budgetwindow4: pure-Rust cSC4SubBudgetWindow core + panel sub-budget path (budget-window.md) | fc4-lane agent, .claude/worktrees/core-budgetwindow4 | ready: core only, partial owner (layout routines and BW2/BW3 open), gate PASS, no game run (core-only lane) | answers to BW1-BW9; glue lane wires BudgetWindowHost to the real cIGZWin / simulators |

## Report

### What changed
- New module `engine/core/src/budget_window/` (one line `pub mod budget_window;` in core lib.rs, alphabetical):
  - `mod.rs`: opaque handles plus the typed `BudgetWindowHost` trait. Each window, generator, simulator, budget,
    ordinance, neighbour and occupant call the spec names is one method there. The row-layout routines (§8) are
    contract callbacks: they get the dialog's data and return what the dialog keeps (slider IDs, ordinance states,
    deals, restore records).
  - `ids.rs`: every §1-§7 identity (IDs, messages, buttons, flags in order, selectors, images).
  - `dialog.rs`: `SubBudgetWindow`, which covers `CreateSubBudgetWindow` (§3 flag order), `DoWinMsg` routing (§4,
    including the Esc / modifier-0x4 Enter and 0x73 close), `on_init` with HiddenPause first and the mode order
    1 / 2 / occupant / group, the slider path (§4.4) with a shareable process-wide re-entry guard, the level split at
    0x0ABCE500, `on_button_activated` (§5: the whole table plus the common refresh tail), `RestoreStartSettings`
    (§5.1), `end_dialog` (§6) and `DestroySubBudgetWindow` (§6: deals, UI-disable off, MarkDealsDirty, resume
    before end).
  - `panel.rs`: the §2.1 selector/command table (`panel_command`), `on_command` (tutorial filter first),
    `DoSubBudgetWindow` (§2: re-entry no-op, step-4 sequence, modal run through host, then always destroy, drop and
    balance refresh) and `DoMessage` (§7: dirty coalescing).
- Tests: `budget_window/tests.rs`, 21 tests. They cover all six §9 conformance cases plus the create sequence and
  failure, end_dialog branches, scroll clamping and arrows, the occupant paths, controls outside occupant mode,
  the re-entry guard, the slider fraction, the selector table and the tutorial filter.
- No plugin, CMake, CAPI or ABI edits.

### Measured
- Baseline `qbuild all`: 3756 tests, clippy 0, ctest 63/63. Final: 3777 tests passed / 0 failed, clippy 0, ctest
  63/63, qbuild PASS (tip a500d06b + this report).
- No game run: a core-only lane with no shadow or replace hook.

### Partial (owner not complete)
- What the spec leaves open is not modelled. That is the per-row layout and text (BW1), `FUN_0077AB40` on 0xCF
  (BW2: the close path is implemented, the call is not), the 0x0ABCDExx occupant controls (BW3: forwarded to the
  host), panel slot 6 (BW4: returned as a command), on_shutdown, the loan, taxes and ShowWindow tab logic of the
  panel, and the nested DoIndividualBudgetWindow steps (forwarded to the host).
- The literal readings each carry a code comment. Questions filed in docs/specs/questions/budget-window.md:
  - BW6: the out/+0x0C window references, and HiddenResume after a failed create.
  - BW7: scroll upper bound len-9, the arrow rule, which buttons take the tail.
  - BW8: slider and level numeric types, when restore records are filled.
  - BW9: on_shutdown, return values, the second end_dialog argument, inner child and ordinance toggle inputs,
    SetOrdinanceOn args.

### Next
- A glue lane implements `BudgetWindowHost` over the real cIGZWin, cISC4Simulator, budget, ordinance and neighbour
  interfaces, then replaces cSC4SubBudgetWindow once BW1-BW3 are answered.
