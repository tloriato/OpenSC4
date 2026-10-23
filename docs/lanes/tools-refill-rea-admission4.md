| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| tools-refill-rea-admission4: per-worker REA startup/admission | Codex, `tools-refill-rea-admission4` | in progress: round 2 actual API repair | Implement supported contract, inherited host admission, qualify isolated tests and request root proof |

## Handoff
- Goal: final round actual installed API repair; no engine/native study or worker target open.
- Preserved clean 5d6b8092 history and old 194 passed / 1 skipped / 2 failed full log; focused followups are not final qualification.
- Read both copied installed schema reviews and exact binary attestation (path-only open, PE/x86 SHA pinned).
- Root coordinator-rea-old-fixture-cleanup.json reports exact old processes absent after adjudication; no worker cleanup.
- Implemented v2 path-only contract, typed binary_session identity/provider/server binding, input/output pins and argument validation; captured installed schemas are metadata-only test fixture.
- Implemented host MAC admission assertion and worker prompt distinction; tests not run yet, source qualification pending.
- Next: isolated host assertion tests and focused relevant suite; exact old PIDs absent by Get-Process; CIM enumeration denied, preserve limitation and use scoped psutil check if available.
- Live startup and CLI/query proof UNKNOWN until root-only evidence; no production calls by worker.
- Prior round report retained below as historical evidence, to be replaced by final report at finish.

## Result and scope

Implemented fail-closed retained-client admission. This is **wip**, not a claim
that the installed REA can now open the real Windows target. Actual configured
source access was denied; metadata-only execution reported `REA EOF` with
`MODULE_NOT_FOUND` for the configured `rea-agents/scripts/rea.mjs`. No real tool
schemas, production target contract, project readiness or original procedure
results were obtained. The actual prior `target_unavailable: No app is open`
failure is retained as the motivating failure, not suppressed by keeper success.

Changed **7 production files, 3 test/fixture files, 3 docs files**; added 17 MCP
regression cases and one guard case. No engine, algorithms, maps, regions, ABI,
models, auth, provider/cache configuration or live coordinator queue/state edits.
No native build/game run was requested or needed. Questions: none.

## Implementation

- `tools/rea_session.py`: coordinator-reviewed binary hash, exact binary/project
  paths, declared read-only open option, schema pins and typed identity required.
  Open only an explicitly empty target; reject occupied/wrong/unready clients.
  Retain the admitted procedure-capable process; the worker attaches once to it.
  Recheck target identity before research; expose only the two research tools.
- Backend/launcher: admission precedes reservation, queue/state writes and
  worktree creation. Failed bootstrap preserves the cause without retries or
  consumed rounds. Backend lifetime EOF, transport EOF/cancellation, startup
  failure and Windows private job ownership clean only owned processes. Full
  child stderr remains private in build-runs; stdout retains JSON-RPC framing.
- Guard/prompt: deny non-RE and arbitrary MCP capabilities; keep STOP, assignment,
  dependency, parking, caps, disk, concurrency, provider and progress policies.
  A keeper or tools/list handshake can never establish target-ready GO.
- Codex support is conditional on the reviewed version-1 contract, protocol
  2025-06-18, declared read-only options and typed exact-target metadata. The
  installed server's compliance is **unqualified**. Claude RE is explicitly
  unsupported before reservation and in the backend; its current workflow cannot
  guarantee same-client attachment. Engine/tool transports remain unchanged.

## Measured validation

Final source focused checks used the authorized
`C:/Users/tlori/FreeCity4/build-runs/pytest-venv/Scripts/python.exe` (pytest 8.2.2):

- `-m pytest tools/tests/test_rea_admission.py -q --tb=short` printed
  **17 passed in 25.81s**.
- `-m pytest tools/tests/test_lane_backends.py tools/tests/test_laneguard.py tools/tests/test_lane_evidence.py -q --tb=short`
  printed **56 passed, 1 skipped in 22.30s**. Total **73 passed, 1 skipped**.
- Tests execute actual configured bridge commands and per-process fake MCP
  servers: two independent clients, same-server open/research audit witnesses,
  correct preexisting identity without reopen, isolated keeper, wrong target,
  missing path/hash/schema/options, open negative/error/untyped replies, protocol
  error, timeout/EOF, identity drift, role/tool/path denials, cancellation,
  supervisor lifetime loss, owned Windows descendants and no failed-preflight
  queue/state/round/worktree mutation. Real backend preflight/lease/cleanup runs;
  only model execution and Git preparation are stubbed in its supervisor test.
- ONE full tools suite on committed source **26a6c7e9**:
  `python build-runs/run-tools-suite.py` ran authorized Python
  `-m pytest tools/tests -q --tb=short`, printing **194 passed, 1 skipped,
  2 failed in 46.96s**. Full log: `build-runs/rea-full-tools.log`.
  Both failures were the new tests equating a Windows venv redirector PID with
  its actual server PID. Independently reproduced launcher 38760 versus Python
  24644, corrected proof to audit actual server-side calls, and renamed recorded
  process evidence to `transport_pid`. Final focused checks above pass this
  environment. The full suite was **not repeated** under the one-run limit and
  must not be reported as green on final source.
- `git diff --check` printed no whitespace errors. Default Python's
  `-m pytest --version` printed `No module named pytest`; the authorized venv
  supplied actual validation, not an assumed pass.

Milestones: **482862b5** reproduction, **57636fcc** substantive admission fix,
**26a6c7e9** schema/diagnostic/ownership checks, **03bcea3a** redirector-proof
identity evidence and supervisor lifetime cleanup.

## Coordinator follow-up

The reviewed contract and evidence expectations are in
[docs/ops/rea-admission.md](../ops/rea-admission.md). Before landing, inspect the
installed API and select/adapt a genuinely supported exact-target read-only open
and typed identity path; do not mark the fixture-shaped template reviewed. The
explicit contract is `MAIN/build-runs/rea-target.json`; no file was created there.

Safe coordinator commands before landing (no model, research output or rounds):

```powershell
python C:/Users/tlori/FreeCity4/.claude/worktrees/tools-refill-rea-admission4/tools/rea_session.py inspect
python C:/Users/tlori/FreeCity4/.claude/worktrees/tools-refill-rea-admission4/tools/rea_session.py inspect --identity-tool REVIEWED_TOOL_NAME
python C:/Users/tlori/FreeCity4/.claude/worktrees/tools-refill-rea-admission4/tools/rea_session.py smoke --root C:/Users/tlori/FreeCity4 --lane re-readiness-smoke
```

After actual API/contract review, expect two distinct owned sessions, exact typed
binary/project identity, boolean ready true, exactly two research capabilities,
and zero research calls. Preserve UNKNOWN until that actual evidence exists.
Then qualify the final full tools suite before changing this lane to ready.
No automatic extra/reset/fourth worker round; inherited family caps are unchanged.
