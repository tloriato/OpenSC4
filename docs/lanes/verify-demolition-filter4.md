| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| verify-demolition-filter4: native demolition filter comparison | Codex / `verify-demolition-filter4` (engine lane, detached HEAD) | in progress; boundary audit complete, blocked by DM15; no engine changes or verification credit | 1. Publish supported native filter acquisition/configuration/lifetime boundary and reconcile occupant-manager query slot. |

## Handoff
- Goal: independently compare original native cSC4DemolitionOccupantFilter with Rust on identical published inputs, including lifecycle and real selection/actions without duplicate mutation.
- Done: reused reserved detached worktree; initial status clean; audited demolition sections 1-4.4/8/10, generic SDK filter/demolition interfaces, relevant ABI rows and existing Rust/plugin implementation.
- Boundary result: utility factory/getter does not expose Phi; setter is no-op. A borrowed filter enters an occupant-manager query but lacks a published owner/full-state/lifetime boundary for the requested scope. Demolition names slot 41; occupant-manager describes filter slot 40 and typed slot 41.
- Question: DM15 appended to `docs/specs/questions/demolition.md` (next free ID DM15 in index). Index regeneration is deferred: tools/ execution and index edits are forbidden to this lane.
- Next: commit this contract-gap milestone, then finalize the docs-only blocked report and wip queue; no build/game can establish the requested equality without the missing contract.
- Constraints: no RE, tools/, executable reads, subagents, branch/ref updates, defaults/maps or unrelated edits; no code-free baseline build.
- Gate/game: none; no request outstanding. Existing demolition simulator 73/73 is not filter evidence.
- Manifest: `C:/Users/tlori/FreeCity4/build-runs/run4-verify-demolition-filter4-r1-manifest.json`.
