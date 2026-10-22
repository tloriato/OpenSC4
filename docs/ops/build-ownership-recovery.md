# Build/check ownership and recovery

qbuild and fc4verify use the same nonblocking lock in the canonical worktree.
Busy rejection happens before command entry, log replacement or verdict publication.
There is no queue, lease TTL, result reuse or permission to unlink the active lock.

On Windows a command starts suspended, enters a fresh kernel Job, then resumes.
Descendants inherit containment; no breakaway or kill-on-close exemption is enabled.
Normal return, failure and timeout all require descendant drainage. After five seconds
of incomplete drainage the invocation fails closed with an owner/job diagnostic;
it does not kill the descendants or publish `released`. A command's existing timeout
still terminates its direct Popen child, and reports timeout truthfully.

The guardian's owner JSON records Job identities before commands can execute.
An existing empty Job supplies drainage evidence. A missing Job after a guardian
crash supplies **no** lifetime evidence: the kernel name can disappear while former
members remain alive. Such metadata remains uncertain. PID existence, process name,
elapsed time, an empty process snapshot and direct-child exit cannot recover it.

For uncertain ownership, retain the owner JSON and let the original work drain.
Only independently demonstrated whole-command drainage justifies archiving that
JSON and retrying. For test fixtures the release sockets, known spawn lifecycle,
and retained process handles/creation times provide that proof. A real invocation
needs equivalent evidence for every relevant writer, including service-side work;
when that evidence is unavailable, leave it closed and escalate the diagnostic.
Never remove or replace `.build-check.lock`, kill compilers, or stop shared caches
as a recovery shortcut. No automated recovery command accepts a PID/age assertion.

Linux uses kernel subreaper adoption at the outer command/delegate boundary, including
double-forked descendants. Guardian death loses that evidence and fails closed.
This fallback has not been executed by the Windows lane; other platforms reject
command launch if they lack a kernel containment primitive.

Delegation authorizes one intended launch. A Windows venv redirector may authorize
its immediate base-Python child only after retained launch creation-time, executable,
parent-edge and Job checks, followed by the actual pipe peer's OS identity check.
Copied invitations, siblings, stale tokens and second claims grant no ownership.
Authentication has a fixed deadline in bounded workers; an unresponsive pipe peer
cannot occupy the sole acceptor. Incomplete cleanup also preserves uncertainty.

Cache remains enabled with existing ports and idle policy. There is deliberately no
process-name exemption for sccache or other services. A newly contained persistent
service without trustworthy work-completion evidence therefore produces bounded
failure and retained uncertainty. This round does not claim service-side idle/work
acknowledgement; the preserved cache-lifetime DRAFT remains deferred. Such uncertainty
is a limitation requiring review, not a fresh passing build verdict.
