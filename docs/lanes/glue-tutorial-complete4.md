| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| glue-tutorial-complete4: tutorial replacement | Codex / `glue-tutorial-complete4` (engine lane) | in progress | 1. Read scoped contracts and baseline. 2. Complete supported tutorial glue. 3. Gate and real-game evidence. |

## Handoff
- Goal: finish actual enable=tutorial replacement with TU6-TU9 contracts; no coverage/default changes.
- Worktree: reserved detached HEAD; commit normally, never switch refs.
- Baseline all: 4238 Rust passed, clippy clean; ctest 66/67 (CAPI fake still assumed constructor ref1); invalidated by edits during gate. Fixed fake; stable final gate required.
- q.py cannot start Bash (E_ACCESSDENIED); use bounded native PowerShell commands.
- Done a504c48a: TU9 constructor ref0, two owner refs, synchronized state, publish before Init and reentry test.
- Done (next commit): version-gated TU6/TU9 native boundaries; singleton binding/conditional restoration; unlock native calls; native window slots; TU7 cheat/TU8 filter; CAPI save-preparation export (ABI=true); start/cheat harness actions.
- Quick first attempt failed on stale ref1 CAPI fake and missing initializer_list include; both fixed. Next: quick, stable all gate, real-game original/ours traces and candidate regression.
- Fixture unresolved: queue documents only FC4 Test region; native tutorial-city reopen requires a persisted shipped tutorial fixture, not constructed-only ordinary-city actions.
- Manifest: C:/Users/tlori/FreeCity4/build-runs/run4-glue-tutorial-complete4-r1-manifest.json.
- No game request outstanding. No subagents. One round only.
