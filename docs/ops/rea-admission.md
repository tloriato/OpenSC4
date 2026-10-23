# Retained-client REA admission

The host dispatcher performs normal full admission before reservation, worktree
or round mutation. A retained owned client is separate from the global tool
catalog/keeper handshake: those are metadata and never establish target readiness.

## Reviewed version 2 contract

Root supplies an ignored JSON contract (`MAIN/build-runs/rea-target.json`, or
`smoke --contract PATH`). It requires `version: 2`, `reviewed: true`, label
`Windows 1.1.641`, the exact canonical `binary_path` and `binary_sha256`,
`format: pe`, `architecture: x86`, `kind: executable`,
`provider_selection: auto-single-candidate`, and `open_arguments: {path: EXACT_PATH}`.
`schemas` maps exactly open_binary, binary_session, procedure_pseudo_code and
batch_decompile to `{input: SHA256, output: SHA256}` of sorted compact JSON schemas.
The coordinator reviews this metadata against the installed server before use.
The binary hash is independently checked before process startup. Filesystem and
policy protect game inputs; the installed API has no read_only or project option.
No snapshot/provider/document override is reviewed by this narrow contract.

The actual installed open_binary input has required path and optional format,
provider_id, snapshot_path. The format option is specifically headerless dos-com;
it must not be populated with the detected PE format. Its structuredContent.result
is path/format/kind/sha256/architecture, never an invented opened boolean.
binary_session{} returns structuredContent.result with typed open false/true,
target identity when open, analysis_provider_binding, tool_availability and
server_identity. An occupied wrong target refuses without close/reset/retarget.
Only typed closed with no target fields permits the exact reviewed path-only open.
Automatic deterministic provider binding is observed after open; null is UNKNOWN,
not a fictional requirement for a preexisting provider. Provider/profile and actual
server identity plus nullable analysis_run.run_id are bound stably on that child.
The installed server identity has package/server/start/client/protocol metadata;
it has no asserted server PID. transport_pid is the owned launcher, which can be
a venv redirector. Fixture server PID witnesses are measured independently.

## Worker boundary and host provenance

Only procedure_pseudo_code{procedure: STRING} and batch_decompile{addresses:
STRING_ARRAY} are forwarded. The installed optional document selector is denied.
No target/path/provider/command/document overrides or bootstrap tools are exposed.
Input schemas resolve actual local $refs and validate types/required/extra fields.
Input/output schemas and the same live identity are rechecked before each dispatch.
Metadata admission records live_query UNKNOWN; it is not successful live research.
Canonical export/re_query/instruction research remains available independently
within the task's evidence scope; live pseudocode success is not its prerequisite.

Host dispatch performs guard/dependency/key/role/source checks using full host
refs and installation visibility. The supervisor rechecks normal preflight and
issues a bounded admission assertion bound to lane, round, source, arguments,
target/session, runtime limits and Git registration. A SHA256 literal of that
assertion is pinned in the host-launched hook command, never accepted from the
worker manifest. The recovery manifest remains in build-runs; authoritative hook
registration is under MAIN/.claude/lane-admissions (outside writable roots).
There is no signing secret/key file. Reading public assertion metadata gives no
authority to change the pinned literal in the already launched host configuration.
The actual isolated sandbox trust boundary still requires independent root proof.
Do not mistake unit tests for that proof. Earlier b181451d MAC design was rejected
because its key was under a worker-granted root; no actual key exploit was run.

Workers consume inherited provenance. They do not rerun coordinator spawn checks,
npm discovery or dependency/own-branch discovery in intentionally scoped refs.
Every normal per-call STOP/role/runtime/budget/call-cap guard remains mandatory.
No permission roots, models/providers or guard policy are broadened by admission.

## Ownership and qualification

Windows broker and configured child own kill-on-close jobs. The child starts
suspended and enters its job before resuming, so redirector descendants are owned
from birth. Failed assignment/resume kills only that suspended owned process.
Cleanup closes stdin and the child job before joining pipe readers. The supervisor
holds a lifetime socket; its EOF, cancellation and worker EOF close owned sessions.
Shared keepers/other sessions are never killed or reset. Notification traffic
cannot extend the monotonic RPC deadline and is not queued without bound.
Private stderr diagnostics retain original failure causes outside MCP stdout.

Root-only metadata smoke (zero research calls):

```
python WT/tools/rea_session.py smoke --root MAIN --contract REVIEWED_JSON --lane UNIQUE_LABEL
```

This opens two independent owned sessions, checks exact typed identities, exposes
only the two research tools, then cleans its own children. A successful smoke is
not installed Codex CLI attachment or actual query proof. Those require separate
root evidence from one retained child with query status/subject/result digests and
identity checks before/after; keep native body private and out of coordinator review.
Source-pinned ignored coordinator request/response artifacts record staged proof.
Absent/refused evidence stays UNKNOWN/WIP, never keeper or fake-fixture success.
