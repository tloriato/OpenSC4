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
- Done: reproduction committed 482862b5. Implemented retained owned-child transport, reviewed target/schema contract, before-reservation admission, two-tool boundary, truthful Claude rejection, prompt/guard integration, safe coordinator inspect/smoke commands and contract docs.
- Measured: 14 admission tests pass, including actual configured Python bridge, two independent process identities, all failure cases and no queue/state/round mutation. Existing backend 23 and guard 23 tests passed before final added guard test; rerun focused before full suite.
- Actual inspect command preserves REA EOF/MODULE_NOT_FOUND; live readiness UNKNOWN; no production contract created. Default Python has no pytest; authorized main pytest-venv reports pytest 8.2.2.
- Next: review cleanup/protocol/schema edge cases, rerun focused serially, commit stable source, run ONE full tools suite with authorized pytest venv; report unsupported real protocol until coordinator review/smoke.
- No live target evidence; actual readiness UNKNOWN. No engine/game/native checks required.
- Manifest: C:/Users/tlori/FreeCity4/build-runs/run4-tools-refill-rea-admission4-r1-manifest.json.
