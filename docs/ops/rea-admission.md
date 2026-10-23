# REA admission for reserved research workers

REA target state is process-local. A keeper, a process count and an MCP tool list
do not establish readiness in another client. `rea_handshake` is metadata-only;
`rea_check`, including its legacy no-handshake option, requires a retained session.

The Codex launcher starts an owned broker before changing queue/state/rounds or
creating a worktree. It initializes one REA child, verifies reviewed schemas,
checks typed identity, opens only an empty client with exact reviewed parameters,
then checks identity again. It retains that child. The provider's `rea` MCP
configuration attaches once to this child using `tools/rea_session.py connect`.
It exposes exactly `procedure_pseudo_code` and `batch_decompile`. Identity is
checked again before every research call. Bootstrap tools are never exposed.
Wrong/preexisting targets are rejected without close/reset/replacement.

Claude RE is explicitly **unsupported**: its saved workflow starts independent
MCP clients and cannot currently guarantee attachment to the admitted child.
The launcher rejects this path before reservation; the backend also rejects it.
Engine/tool lanes get no REA tools. Other provider, model, round, STOP, parking,
dependency, assignment, disk, concurrency, budget and watchdog policies remain.

## Required coordinator review

The coordinator must create `MAIN/build-runs/rea-target.json`. No production
contract is supplied: the installed REA source and protocol were inaccessible in
the tooling sandbox (`MODULE_NOT_FOUND`). Live readiness remains **UNKNOWN**.
Do not mark the example below reviewed or assume its fixture names are real API.
An unsupported schema/response/read-only option must fail closed; inspect and
review the installed API before adapting this contract. Never guess success
from text, an open return alone, server count or a tool list.

The version-1 contract requires:

```json
{
  "version": 1,
  "reviewed": false,
  "label": "Windows 1.1.641",
  "binary_path": "ABSOLUTE_AUTHORIZED_BINARY",
  "binary_sha256": "REVIEWED_FILE_SHA256",
  "project_path": "ABSOLUTE_AUTHORIZED_PROJECT",
  "startup_timeout": 45,
  "open": {
    "name": "open_binary",
    "arguments": {"path": "ABSOLUTE_AUTHORIZED_BINARY", "project": "ABSOLUTE_AUTHORIZED_PROJECT", "read_only": true},
    "binary_argument": "path",
    "project_argument": "project",
    "read_only_argument": "read_only",
    "success_field": "opened"
  },
  "identity": {
    "name": "REVIEWED_READ_ONLY_IDENTITY_TOOL_WITH_NO_ARGUMENTS",
    "state_field": "state",
    "ready_field": "ready",
    "binary_field": "binary",
    "project_field": "project"
  },
  "schemas": {
    "open_binary": "REVIEWED_INPUT_SCHEMA_SHA256",
    "REVIEWED_READ_ONLY_IDENTITY_TOOL_WITH_NO_ARGUMENTS": "REVIEWED_INPUT_SCHEMA_SHA256",
    "procedure_pseudo_code": "REVIEWED_INPUT_SCHEMA_SHA256",
    "batch_decompile": "REVIEWED_INPUT_SCHEMA_SHA256"
  }
}
```

Review must establish that the identity tool has no side effects, reports the
actual current binary/project, and returns typed JSON with state `empty` and
boolean ready `false` only for no target, or state `ready`, boolean ready `true`
and both absolute identity paths. Only the explicit empty case may trigger open.
The open must support a real read-only option, preserve original/game files,
use the exact binary/project, and return boolean success `true`. Plain text
success, errors, negative replies, missing identity and different response shapes
are unsupported. Response objects may be MCP `structuredContent` or one text
block containing a JSON object. Schema pins hash sorted, compact JSON inputSchema.
Review both research schemas for purely read-only procedure/address queries;
target/path/file/project/app/command overrides and unreviewed arguments are denied.

## Coordinator-only metadata inspection and live smoke

Run from the coordinator, never a research/engine/tool worker. These commands
do not launch models, alter the ledger, consume rounds or request pseudocode:

```powershell
python C:/Users/tlori/FreeCity4/tools/rea_session.py inspect
python C:/Users/tlori/FreeCity4/tools/rea_session.py inspect --identity-tool REVIEWED_TOOL_NAME
python C:/Users/tlori/FreeCity4/tools/rea_session.py smoke --root C:/Users/tlori/FreeCity4 --lane re-readiness-smoke
```

`inspect` initializes an owned server and lists metadata only. It prints tool
names and selected input schemas/digests; its readiness is always UNKNOWN.
`smoke` requires the reviewed contract and makes two independent owned sessions.
Expected evidence: two different session IDs/transport PIDs, exact expected identity
paths and SHA256, boolean ready `true`, exactly the two research tools, and
`research_calls: 0`. Keep stdout as private build-runs evidence. This verifies
typed readiness, not the success of a substantive procedure request. The real
provider command/config is additionally exercised with fake processes in tests.

No inspection/smoke failure should be hidden or replaced by keeper success. The
normal launch is still `python tools/lane_launch.py SPEC --provider codex` with
the existing family caps; no automatic retries, extra rounds or provider fallback.
Missing contract, wrong file hash, missing paths, unsupported APIs, malformed
protocol, EOF and timeouts fail admission before queue/state/round mutation.
Successful admission tickets in build-runs/manifests record typed identity,
session/transport/broker IDs and contract digest; their attach token is local/private.
They are not portable readiness attestations: every check contacts the live broker.
`transport_pid` identifies the owned command process; a Python venv redirector
can launch a different actual server PID. Neither PID is a readiness assertion.
The fixture tests separately audit actual server-side open/identity/research PIDs.

The broker owns only its child and descendants. On Windows it enters a private
kill-on-close job **before** creating REA. Failure to establish that ownership
fails startup. EOF/cancellation, bootstrap failure and backend completion clean
up owned processes. No process-name kills, keeper cleanup, global app close or
reset occur. An unclaimed session expires after 120 seconds. Research calls have
a 600-second transport deadline; the reviewed startup timeout is at most 120.
The backend holds a lifetime socket; its EOF also cleans the owned session if
the supervisor dies while a provider's client is still attached.
Stdout stays newline JSON-RPC; diagnostics go to stderr. No pseudocode is logged
by admission, inspection, smoke or the transport.
Full child stderr is retained privately in `build-runs/rea-session-*-stderr.log`;
the ticket records its path. Do not publish these private diagnostics as RE output.
