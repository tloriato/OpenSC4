| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| tools-build-singleflight4: build/check single-flight | Codex / `tools-build-singleflight4` (tooling) | in progress | Implement and test shared canonical-worktree ownership |

## Handoff
- Goal: fail-fast shared qbuild/fc4verify ownership through verdict publication, preserving cross-worktree concurrency.
- Done: initial handoff `87a9e064`; implemented shared nonblocking guardian-owned OS lease and PID-verified child delegation.
- Done: qbuild whole-invocation ownership; CLI/imported verify validation and JSON publication inside lease; conventions updated.
- Next: add/run isolated subprocess concurrency, crash and provenance tests; inspect failures before full tools suite.
- Design: guardian owns lock and parents/waits build commands; wrapper/child crash drains commands; guardian crash fails closed via active metadata.
- Validation required: isolated subprocess concurrency/recovery tests, full tools pytest once on stable source, doccheck.
- Constraints: no real builds/game, no subagents/main edits; manifest `C:/Users/tlori/FreeCity4/build-runs/run4-tools-build-singleflight4-r1-manifest.json`.
