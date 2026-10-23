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
- Source cleanup/schema/diagnostics milestone committed 26a6c7e9. ONE full tools suite on that committed source printed 194 passed, 1 skipped, 2 failed (46.96s), log build-runs/rea-full-tools.log.
- Both failures were fixture proof using launcher PID as server PID under Windows venv redirectors. Reproduced independently: launcher PID 38760 vs actual interpreter 24644. Corrected tests to audit actual server-side open/identity/research witnesses, renamed production evidence to transport_pid; readiness still typed target identity, never PID.
- Final focused source: authorized pytest venv prints admission 17 passed (25.81s), backend/guard/evidence 56 passed + 1 skipped (22.30s): total 73 passed + 1 skipped. Includes real backend supervisor lease/preflight/cleanup with only model execution and Git preparation stubbed. diff --check clean.
- Added backend lifetime EOF cleanup; 17 admission cases include supervisor death and real backend main integration. Existing policies remain covered by backend/guard/evidence tests.
- Next: finalize report/queue wip after committing this correction. Full suite not repeated (one-run limit). Coordinator must inspect/review real API and qualify live smoke/full tools before ready; commands in docs/ops/rea-admission.md. No automatic extra worker round.
- No live target evidence; actual readiness UNKNOWN. No engine/game/native checks required.
- Manifest: C:/Users/tlori/FreeCity4/build-runs/run4-tools-refill-rea-admission4-r1-manifest.json.
