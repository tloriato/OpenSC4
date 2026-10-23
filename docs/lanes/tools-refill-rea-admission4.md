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
- Substantive fix committed 57636fcc. Review follow-up preserves private stderr, pins declared path/read-only schema options, verifies Windows owned-descendant cleanup without touching another admitted session, and retains valid JSON-RPC error framing on identity drift.
- Measured focused checks: admission 15/15, backend 24/24, guard 23/23, evidence 9 passed + 1 skipped. Actual configured bridge, two independent process identities, failure/cancellation cases, no queue/state/round mutation all exercised with harmless processes.
- Actual inspect command preserves REA EOF/MODULE_NOT_FOUND; live readiness UNKNOWN; no production contract created. Default Python has no pytest; authorized main pytest-venv reports pytest 8.2.2.
- Next: run ONE full tools suite on this stable source with authorized pytest venv, retain exact counts/log, then finalize report/queue wip because real API/schema support could not be established here. Coordinator inspect/review and live smoke commands are in docs/ops/rea-admission.md.
- No live target evidence; actual readiness UNKNOWN. No engine/game/native checks required.
- Manifest: C:/Users/tlori/FreeCity4/build-runs/run4-tools-refill-rea-admission4-r1-manifest.json.
