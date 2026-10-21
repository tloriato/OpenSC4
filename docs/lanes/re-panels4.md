| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| re-panels4: budget window family RE spec (`cSC4SubBudgetWindow`, `cSC4BudgetUIWinProc`) | `re-panels4` / `re-panels4` (RE lane) | **ready**: new docs/specs/budget-window.md (lifecycle, selector table, window proc, buttons, close order, panel refresh hook, 6 tests); questions BW1-BW5; specs only, no ABI rows, no engine code. REA unavailable (no binary open), export + decomp only | 1. answer BW1-BW3 (row layouts, `FUN_0077AB40`, occupant controls) 2. game check BW5 (Esc/Enter/Alt+F4) 3. coordinator: map rows below |

## Report

**Scope.** Largest SC4-specific panel family by bytes: `cSC4SubBudgetWindow` (71 235 B, 32 fns) and
`cSC4BudgetUIWinProc` (17 952 B, 21 fns). Generic widgets left to re-widgets4.

**Added.** `docs/specs/budget-window.md`: identities (vtable `0x00AB5494` six slots, size 0x140, window ID
`0x0423278F`, GZCOM class/IID `0x4386D516`/`0x5386D516`); `DoSubBudgetWindow` open sequence (no re-entry, modal run,
win-location save/restore, destroy, balance-sheet refresh); the full panel command -> selector table (ordinances 1,
neighbours 2, seven department groups); `CreateSubBudgetWindow` flags; `DoWinMsg` message table; hidden pause in
`on_init` / `HiddenResume` in `DestroySubBudgetWindow`; slider -> `SetFundingPercentage(pos x 0.01, 0)` with the
re-entry byte `0x00B4DFC8`; button table (OK `0x67`, Cancel `0x6D` with `RestoreStartSettings`, ordinance apply
`0xCD` via `SetOrdinanceOn`, scroll `0x451`-`0x454`, nested occupant dialog); neighbour-mode close
(`SetUIDisableFlag(false)`, `MarkDealsDirty`), consistent with neighbors.md; panel `DoMessage` dirty-byte refresh.

**Not covered (honest).** Row layout of `initNeighborBudget`/`updateNeighborBudget`/`initOrdinanceBudget`/
`initIndividualBudget`/`UpdateValues` (about 45 KB) is described by contract only; `cSC4BudgetUIWinProc` balance-sheet,
tax and loan display routines (`InitBalanceSheet`, `CalculateValues`, `UpdateBalanceSheetInfo`, `DisplayLoanWindow`,
`UpdateLoanDisplay`, `DisplayTaxesWindow`) are named but not specified. The Expense/Income tab procs are not covered.

**Coverage-map proposal (coordinator owns the edit).** `cSC4SubBudgetWindow` -> specified `docs/specs/budget-window.md`
(lifecycle and event contracts; layout open as BW1-BW3). `cSC4BudgetUI*` -> partially specified (DoSubBudgetWindow,
DoWinProcMessage command routing, DoMessage, ShowWindow only); suggest keeping it unspecified until the balance-sheet
routines are written. Implemented: none.

**Evidence.** REA preflight NO-GO (`target_unavailable`, no app open); all facts from `tools/re_query.py` asm/callers/
callees/vtable and decomp/, mangled decomp cross-checked against instructions at the cited addresses.
