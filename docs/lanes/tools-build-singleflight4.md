| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| tools-build-singleflight4: build/check single-flight | Codex / `tools-build-singleflight4` (tooling) | in progress | Implement and test shared canonical-worktree ownership |

## Handoff
- Goal: fail-fast shared qbuild/fc4verify ownership through verdict publication, preserving cross-worktree concurrency.
- Done: reused reserved detached worktree; read rules and wrapper entry points; no engine changes.
- Next: design OS-backed lease with verified single-child delegation; implement helper and integrate top-level wrappers.
- Validation required: isolated subprocess concurrency/recovery tests, full tools pytest once on stable source, doccheck.
- Constraints: no real builds/game, no subagents/main edits; manifest `C:/Users/tlori/FreeCity4/build-runs/run4-tools-build-singleflight4-r1-manifest.json`.
