| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| verify-advice-subject4: independent native AdviceSubject verification | Codex / `verify-advice-subject4` (engine lane) | in progress | Record baseline; establish published factory and ownership; implement and run bounded opt-in verifier. |

## Handoff
- Goal: compare independently created native AdviceSubject fixtures against actual production Rust methods, without advisor activation or replacement registration.
- Done: reserved detached worktree reused; initial handoff a7479a5c. Baseline all invoked (cargo still compiling). Substantive opt-in verifier written before call 30.
- Next: finish baseline, quick build, focused transport tests, then native run if the public endpoint succeeds.
- AD17 filed: spec CreateInstance spelling versus public SDK GetClassObject class-instantiation API. Only the documented public operation is used, without fallback.
- Production transport: cFC4AdviceSubject fronts invoke CAPI get/set; aggregate setter invokes actual Rust methods. Clone front uses a new Rust subject plus setters, not Rust clone_subject directly; this limitation must remain explicit.
- Constraints: one round; no subagents; no shared ABI/config/default/tools edits; serialization only with already published typed endpoints.
- Manifest: `C:/Users/tlori/FreeCity4/build-runs/run4-verify-advice-subject4-r1-manifest.json`.
- No game requests outstanding.
