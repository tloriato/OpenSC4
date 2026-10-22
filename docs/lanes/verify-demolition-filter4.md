| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| verify-demolition-filter4: native demolition filter comparison | Codex / `verify-demolition-filter4` (engine lane, detached HEAD) | Blocked by DM15; contract-gap commit 049b9f8d; cargo/clippy/ctest n/a, no game requests, no filter coverage credit, ABI unchanged | 1. Publish supported native filter acquisition/configuration/lifetime boundary and reconcile occupant-manager query slot. 2. Rebrief verification after publication. |

## Result

Stopped at the requested early contract check, before implementation/build/game activity. The published material does not establish a complete safe acquisition/calling/lifetime boundary for the requested independent original-native demolition-filter comparison. No engine code or C ABI exports changed. Commit `049b9f8d` records the boundary question.

## Published boundary findings

- Demolition sections 1 and 8 and SDK `cISC4Demolition.h` expose construction/access to the demolition utility and demolition actions, but no getter for its owned filter. `SetDefaultOccupantFilter` is explicitly a no-op.
- Section 2 describes the filter as internally created at Init; section 4.2 specifies predicate state and evaluation order. These descriptions do not supply an allowed factory/configuration interface. No private native fields or absolute native addresses were used.
- SDK `cISC4OccupantFilter.h` supplies occupant, type and property-holder predicate signatures. The seven filter ABI vtable rows supply slot positions; those rows alone do not supply construction, owner identification or per-call configuration access.
- Section 4.4 does publish a potential synchronous borrowed-filter observation when native demolition calls the occupant manager. That is narrower than a full owner contract: it does not specify how this observer identifies Phi and captures all configured state, nor how constructed probes exercise the native filter lifecycle. AddRef would preserve object lifetime, not the changing bounds/privilege/caller/view state.
- The same call boundary needs clarification: demolition section 4.4 step 4 names occupant-manager slot 41 with a filter, while occupant-manager sections 8.4-8.5 describe filter slot 40 and typed slot 41. No speculative ABI correction was made.
- Existing Rust `Filter` state and demolition plugin/host/shadow code were inspected narrowly. The existing simulator shadow is not evidence for equality of the three filter predicates or native filter lifecycle.

## Validation and accounting

No native filter events were instrumented or compared. Compared/different/refused/fallback/unaccounted event counters are unavailable; no equality ratio or algorithm coverage is claimed. No constructed probes, natural-call capture, deletion, save/reload or shutdown exercise ran. No game request is outstanding. The existing demolition simulator 73/73 was not reused as filter evidence.

Cargo, clippy, CMake/ctest and game verdicts are n/a: this is a docs-only contract stop under the explicit instruction to avoid a code-free baseline build. Verification requirements remain unmet; this lane is blocked and its queue entry is wip, not ready.

## Question and next steps

DM15 is appended to `docs/specs/questions/demolition.md`, using the next free ID published in `docs/specs/QUESTIONS.md`. It asks for the supported native filter acquisition/configuration/lifetime route, a bounded callback/proxy alternative if one exists, and reconciliation of the occupant-manager slot. Question-index regeneration was not run because this lane may neither execute tools/ nor edit the generated index; the coordinator can regenerate it through its authorized workflow.

After the contract is published, rebrief verification to compare independent Rust with the authoritative original on identical typed inputs, preserve native outcomes, account for all events, and test real actions/refusals plus save/reload/shutdown without duplicate mutation. No whole-owner credit is warranted by this report.

Manifest: `C:/Users/tlori/FreeCity4/build-runs/run4-verify-demolition-filter4-r1-manifest.json`. Reserved worktree reused on detached HEAD; the supervisor owns publication to the reserved branch.
