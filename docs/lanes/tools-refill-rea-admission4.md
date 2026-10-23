| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| tools-refill-rea-admission4: per-worker REA startup/admission | Codex, `tools-refill-rea-admission4` | in progress | Reproduce isolated MCP target state; implement scoped same-client bootstrap and validate. |

## Handoff
- Goal: truthful per-worker REA readiness, scoped to the authorized read-only target; preserve all lane policies.
- Worktree reused at detached HEAD; initial main.. log and status empty.
- q.py cannot start Bash (E_ACCESSDENIED); use bounded PowerShell commands.
- Milestone: real process fixture reproduces tools/list success followed by independent client `No app is open` (1 unittest passes). Old handshake also emits ResourceWarnings for unreaped/unclosed child.
- Inspected backend/config/guard/prompt. Configured REA source access denied; metadata-only owned launch reports EOF and MODULE_NOT_FOUND. No protocol schema available in sandbox.
- Design: reviewed exact-target/schema contract required; Codex attaches to the very process admitted before reservation via a narrow owned transport. Claude RE workflow explicitly unsupported until equivalent attachment exists.
- Next: implement transport, fail-closed admission/config/guard integration; test processes serially. Default Python has no pytest.
- No live target evidence; actual readiness UNKNOWN. No engine/game/native checks required.
- Manifest: C:/Users/tlori/FreeCity4/build-runs/run4-tools-refill-rea-admission4-r1-manifest.json.
