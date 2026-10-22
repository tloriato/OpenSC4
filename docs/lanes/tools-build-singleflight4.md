| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| tools-build-singleflight4: build/check single-flight | Codex / `tools-build-singleflight4` (tooling) | in progress | Implement and test shared canonical-worktree ownership |

## Handoff
- Goal: fail-fast shared qbuild/fc4verify ownership through verdict publication, preserving cross-worktree concurrency.
- Done: initial handoff `87a9e064`; substantive lease integration `ff8a6997`.
- Done: qbuild whole-invocation ownership; CLI/imported verify validation and JSON publication inside lease; conventions updated.
- Done: final focused Windows run: `python -m unittest tools.tests.test_build_singleflight tools.tests.test_gate -q`: 40 tests in 30.121s, OK (16 ownership + 24 gate/parser tests). `python tools/doccheck.py check`: ok.
- Done: authenticated JSON-only child IPC, verified server/client OS peer PID and live spawned-child process object, one claimed child session, signal-isolated guardian.
- Next: commit this test milestone, then attempt full pytest on committed stable source and run available stdlib suite/doccheck; report exact results.
- Environment: Windows Python 3.12 has no pytest; bounded pip install found no available version. q.py cannot start Bash (Bash/Service/CreateInstance/E_ACCESSDENIED). POSIX execution unavailable here.
- Design: guardian owns lock and parents/waits build commands; wrapper/child crash drains commands; guardian crash fails closed via active metadata.
- Validation required: isolated subprocess concurrency/recovery tests, full tools pytest once on stable source, doccheck.
- Constraints: no real builds/game, no subagents/main edits; manifest `C:/Users/tlori/FreeCity4/build-runs/run4-tools-build-singleflight4-r1-manifest.json`.
