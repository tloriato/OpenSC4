| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| tools-build-singleflight4: build/check single-flight | Codex / `tools-build-singleflight4` (tooling; reserved branch, detached HEAD) | in progress | Verify installed storage/client capability and implement the reviewed whole-output ownership design within final original-family round |

## Handoff

- Goal: FINAL3/3, preserve canonical whole-worktree exclusion including service-owned writes; no production cache/compiler mutation.
- Start verified: clean detached `53ed3bc47cf16101a3a7cbc4078ccf9bc02c978f`; actual copied r2 result is WIP with 195 passed, not approval to promote.
- Historical evidence: dd50e233 9 failed / 168 passed / 306.40 s; 6/6 descendant violations; 6/6 service violations and distinct new persistent-service uncertainty must remain.
- Source design: copied `build-runs/evidence-run4/coordinator-singleflight-service-design.md`; capability/config/service-role proof mandatory, no name exemptions or legacy fallback.
- Intake: `build-runs/r3-intake-02.json`: no old recorded PID matches or attributable old check; native command-line inventory used after CIM access denied; do not touch other live lanes.
- Verified ZIP SHA256 `8056ff89839725c64c2f878a0a703ab4e3f68e90ce192f9d68ce90a770dfdcbb`, 70 files; pinned installed binary SHA256 `7600fb817625313850b8a0525c7743c5d44f53494141390223187e57f394ccb0`.
- Actual bounded isolated wire probe (`r3-protocol-probe.json`): true/file true/ERROR_LOG send StorageHandshake(5); false/empty/LOG empty/distributed send legacy Compile(4); invalid errors without request. No request serviced and no real daemon used.
- Partial safety change: detected cached paths now reject before log opening/command launch at actual qbuild/fc4verify and guardian boundaries; no service exemption, cache-off fallback or client-side readiness claim.
- Next: focused actual wrapper quarantine regressions and isolated capability evidence; complete service protocol still missing. Full ownership guarantee remains WIP.
- Required final full pytest uses provided `C:/Users/tlori/FreeCity4/build-runs/pytest-venv/Scripts/python.exe` once on stable committed source after new focused tests.
- No subagents, engine edits, current-daemon probes/mutations, shared cache/compiler checks, merge/ref edits or new rounds.

## Preserved round-two report

## Round 2 result: tested repair, still held

The three demonstrated Windows defects have substantive repairs and actual process/pipe regressions.
This is **not ready**: the source conservatively fails closed for persistent services without trustworthy
work-completion evidence. It does not implement an explicit service lifecycle acknowledgement, so it cannot
claim the requested complete guarantee while admitting an intentionally persistent sccache daemon.
Passing pytest does not promote this remaining limitation.

Commits: `7564e06c` introduces suspended-launch Job containment and bounded authentication;
`f66b5299` repairs lost-Job recovery and retains launch creation identity; `84676ad5` completes the
new fixture matrix and recovery operations note. All final code/test validation used **84676ad5**, with
tracked source clean at both start and finish. Subsequent changes are this report only.

### What changed

- A Windows command enters a fresh kernel Job before its suspended initial thread resumes. Its children
  cannot escape by an early spawn, direct normal/nonzero exit, abrupt termination or redirected output.
  No breakaway or kill-on-close policy is enabled. Drainage uses live kernel membership, not PID age,
  a process-name exemption or a single process snapshot.
- Job identities are published before command entry. After incomplete drainage, the guardian returns a
  bounded error and leaves ownership uncertain. Direct-command deadlines still report timeout; only the
  pre-existing direct-child timeout termination is retained. Descendants are not killed to recover a lease.
- Guardian failure never treats a vanished Job name as evidence that former members exited. An existing
  empty Job is evidence; missing evidence requires independent whole-tree proof before explicit metadata
  archive. The lock inode is never unlinked. Crash fixtures deliberately close observer Job handles before
  crashing, retain process handles/creation times, release owned writers and wait every retained member.
- One venv redirector launch may authorize its immediate base-Python child using the retained launch creation
  time, exact interpreter images, parent edge, Job membership and actual OS pipe peer. Direct global Python
  remains supported. The actual guardian's OS server identity is checked; copying a key grants no branch.
- Authentication moved out of the sole acceptor into bounded workers. The mutual handshake has a fixed
  deadline, partial receives can be cancelled, hello reads and cleanup joins are bounded, and incomplete
  cleanup preserves uncertainty. Authentication and JSON-only message decoding remain in place.
- Linux uses kernel subreaper adoption at the outer command/delegate boundary; that fallback was not run
  on this Windows host. Unsupported hosts fail closed before command entry.

Only the shared helper, isolated fixture/tests and documentation changed. Wrapper parsers and production
qbuild/fc4verify drivers remain unchanged. See [recovery operations](../ops/build-ownership-recovery.md).

### Measured final validation

Interpreter for the required suite: `C:/Users/tlori/FreeCity4/build-runs/pytest-venv/Scripts/python.exe`.
Capture helper `build-runs/run-r2.py` records full HEAD/tree, helper SHA256, dirty state, command,
start/finish UTC timestamps and log path in each JSON sidecar. It refuses existing evidence paths.

| Actual command/result | Evidence under ignored build-runs/ |
|---|---|
| Supplied venv: `-m unittest tools.tests.test_build_singleflight tools.tests.test_build_singleflight_repair tools.tests.test_gate -q`: **58 tests, OK, 113.848 s** | `r2-final-venv-focused-01.log` / `.json` |
| Global base Python: original ownership module: **16 tests, OK, 31.849 s** | `r2-final-global-ownership-01.log` / `.json` |
| Supplied venv: `-m pytest -q tools/tests`: **195 passed in 207.73 s (0:03:27)**, exit 0 | `r2-final-tools-pytest-01.log` / `.json` |
| `tools/doccheck.py check`: **doccheck check: ok** | `r2-final-doccheck-01.log`; report recheck `r2-report-doccheck-01.log` |
| Python compilation and `git diff --check`: no errors | `r2-final-compilation-01.log`, `r2-final-diffcheck-01.log` |

The one final full pytest run completed after the focused checks, against the same committed clean source.
The unittest counts are additional evidence, not a replacement for pytest. Early attempts printed
`No module named pytest`; three bounded dependency attempts found no matching distribution. Availability
subsequently allowed the exact required venv command to execute; no interpreter substitution is claimed.

The new matrix covers normal, abrupt, nonzero and timeout direct exits with both actual file-log wrappers
and captured output; all surviving grandchildren use DEVNULL. It also covers three early-exiting generations,
wrapper/delegate/guardian failures, bounded uncertain ownership, actual writer release and justified recovery.
Contenders run **while** the writer is held; sentinel logs/verdict bytes remain unchanged, then all retained
members are observed exited. No fixture duration was extended to hide a failure.

Actual half-open named-pipe peers connect and never answer the challenge. Authorized children still join;
normal, exception and parent-crash cleanup remain bounded. Abandoned/malformed peers and guardian failure
also run against the real endpoint. Original alias normalization, cross-worktree concurrency, busy entry points,
copied/inherited/stale/forged/sibling token and second-claim rejection tests remain and pass.

Separate repaired-wrapper identity capture, `r2-repaired-peer-processes-02.json`:
global launch/peer **28808**, guardian **14064**; venv redirector **32752**, actual peer **10044**, guardian
**38396**. Both return 0 with **stderr empty**. The JSON retains exact images, parents, process creation times,
separate stdout/stderr and confirmed process exits. Capture provenance is `r2-repaired-peer-probe-02.json`.
The first repaired probe had a filename collision between detail and capture metadata; its stdout/log and
capture JSON remain, and the second probe uses distinct exclusive-create files. No baseline was overwritten.

`r2-evidence-index.json` hashes 34 artifacts, including old logs, copied negative evidence and repaired process
captures. The copied baseline helper still hashes to
`cdceda66f887ed81a150acb310ae637872af09cd40af242f8c860961eacc8f2b`.
The original full-suite failure (9 failed / 168 passed), all six descendant violations, failed round-2 labels and
original pinned probe scripts/JSON remain available. They were not adapted to make negative evidence green.

### Remaining limitation and coordinator next step

Cache stays enabled with its existing server ports and idle settings. No daemon was started, stopped or
probed, and no process-name exemption was introduced. Consequently an opaque persistent service can cause
bounded failure followed by retained uncertainty instead of a passing build. That safety behavior is measured
for owned writers, but a complete trustworthy service/work-completion protocol is **not implemented**.
Keep the merge entry **wip**. The preserved cache-lifetime DRAFT is deferred for the coordinator's remaining
third-round decision only; this lane reserves or launches no follow-up and does not claim that work completed.

Linux execution is also unmeasured here. Cargo, clippy, CMake/CTest and game runs: n/a. No engine, RE, SDK,
ABI, live orchestration/provider/guard/target/queue-state or main changes. Questions filed: none.

## Historical first-round report

The following first-round report and coordinator findings are retained for provenance. Their limits and next
steps describe that earlier source; the round-2 measurements and WIP limitation above govern this tip.

Substantive integration: `ff8a6997`. Tests and final ownership hardening: `be46dee7`.
All measured final-source validation below used `be46dee79cfa16490d5e2c4645e0ff25da0cff8a`.

- Added `.claude/lane-bin/build_lease.py`, shared by qbuild and CLI/imported fc4verify.
  One persistent lock inode under the resolved, case-normalized worktree takes a nonblocking OS lock:
  Windows `msvcrt.LK_NBLCK`, POSIX `flock(LOCK_EX | LOCK_NB)`. Output directory and tag do not choose ownership.
- qbuild validates every step first and owns the entire expanded invocation. fc4verify validates steps and
  output paths before constructing its context, and owns its context/preparation, commands, record and optional
  JSON publication. Invalid/busy checks cannot truncate fixed logs or publish a new verdict. Fmt-only checking
  remains independent and creates no shared logs.
- A private guardian owns the OS lock and directly parents/waits launched commands. Windows descriptor
  inheritance is not used as lease ownership. Synchronous command RPC permits one active branch; qbuild's
  explicitly delegated fc4verify subprocess can claim exactly one child session. Authorization verifies
  canonical worktree, invocation, active owner metadata, live OS lease, server/client OS peer PID and the
  spawned child's live process object. Ordinary commands have the invitation removed from their environment.
  Authentication alone grants no authority; child messages use JSON bytes, never pickle.
- Busy diagnostics identify the canonical worktree, owner PID/command/source/start, guardian PID and invocation.
  Metadata is distinct from the lock and is published only while owning the OS lock. Neither release nor
  recovery unlinks/replaces the lock inode. No queue, previous-result reuse, cancellation or TTL/PID-based steal.
- Wrapper or delegated-child failure leaves the guardian waiting for live launched commands before releasing.
  The guardian has a separate process group/session so a wrapper interruption cannot silently release its build.
  If the guardian itself dies or command drainage is uncertain, active metadata makes subsequent acquisition
  fail closed. The diagnostic requires inspecting surviving commands and manually archiving that metadata
  only after confirming they ended; it never claims stale metadata is an active kernel lease.
- Existing parsers, gate judgement and changed-during-run behavior remain intact. Accepted records retain
  head/tree/dirty/start/finish and actual step/log records, with narrow `ownership` provenance added. SDK and
  environment preparation remain inside ownership. Conventions document contention and exceptional recovery.

### First-round measured validation

Windows, Python 3.12. Harmless Python child commands synchronized by sockets; no duration-modeling sleeps.
The fixture substitutes build context/command payloads while exercising actual drivers, log writers,
parsers, JSON publication, subprocesses, named-pipe peer identity and byte-range locks.

| Command | Printed result |
|---|---|
| `python -m unittest tools.tests.test_build_singleflight tools.tests.test_gate -q` | `Ran 40 tests in 30.121s`, `OK` (16 ownership tests + 24 existing gate/parser tests) |
| `python -m pytest -q tools/tests` on committed stable source | `C:\Program Files\Python312\python.exe: No module named pytest`; no pytest tests ran |
| `python -m unittest discover -s tools/tests -q` on committed stable source | `Ran 161 tests in 292.749s`, `OK`; this includes the focused tests, not an additional 161 disjoint tests |
| `python tools/doccheck.py check` | `doccheck check: ok` |
| `git diff --check` and Python compilation of changed modules | No diff/syntax errors |

The 16 ownership tests cover all/all and all/quick contention; qbuild versus CLI/imported verify and alternate
log directories; original unmodified CLI busy entry points; nested delegation; copied/inherited/stale/forged
tokens and a forged server; bad authentication without disrupting the legitimate child; a second claim by the
same child; simultaneous different worktrees; relative/dot/absolute/Windows-case/junction aliases; unchanged
sentinel logs/verdicts and no context/command entry on busy rejection; JSON publication contention; independent
fmt; preflight invalid steps/paths; real failed commands; changed-source provenance; exceptions; wrapper crash;
parent crash with a live delegate; delegated-child crash with a live command; and guardian crash with explicit
safe recovery after the raw child exits.

Evidence: ignored `build-runs/singleflight-focused.log`, `singleflight-pytest.log`,
`singleflight-unittest.log`, `singleflight-doccheck.log`; capture helper
`build-runs/run-singleflight-validation.py`. Unit-test regress messages refer to fixtures, not actual game requests.

### First-round limits and next step

The required full pytest run is outstanding. Default Windows Python has no pytest; a bounded install into
the worktree's ignored test-deps directory printed `No matching distribution found for pytest` and installed
nothing. Unittest discovery does not execute pytest-style function/fixture tests, so its 161 passing tests
are not represented as full pytest completion. The ordinary merge-queue entry is deliberately `wip`.

POSIX implementation was not executed: q.py printed `Bash/Service/CreateInstance/E_ACCESSDENIED` here.
Linux `flock`/`SO_PEERCRED` tests are implemented for a suitable host; other POSIX platforms without a peer
credential implementation reject delegation safely. Windows locking and junction behavior were tested for real.

Coordinator: run `python -m pytest -q tools/tests` with pytest available against this source, inspect counts and
failures, then promote the queue entry to ready if clean. No engine baseline gate is required by this brief.
Cargo, clippy and CTest: n/a; no real Cargo/CMake/game/RE, ABI, defaults, coverage, guard/provider/orchestration,
live queue/state or main changes. Game runs: none. Questions filed: none.

### Coordinator validation and review, 2026-10-22

The coordinator ran the required full suite on clean source `946556c7` using
`C:/Users/tlori/FreeCity4/build-runs/pytest-venv/Scripts/python.exe -m pytest -q tools/tests`.
It completed with **9 failed, 168 passed in 306.40 s**, exit 1. All nine failing ownership tests timed out
waiting for fixture socket events; the complete tracebacks remain in this worktree's
`build-runs/coordinator-full-tools-2231.log`. The interpreter/child identity cause is not yet established.
The earlier unittest pass cannot substitute for this failure, and no ready promotion is authorized by it.

Independently, an isolated copy of the final committed helper reproduced six exclusion failures: normal
direct-child exit, abrupt exit and timeout, each with file and captured output. In every case a contender
was admitted while a retained OS handle proved the first command's output-writing grandchild remained alive.
All six fixture descendants were explicitly released and observed exited; no real compiler, worktree, cache
or shared process was touched. Exact final-copy evidence is in main's ignored
`build-runs/singleflight-descendant-repro-20261022/current-946/run-ur8nftjb/summary.json`.
The helper SHA256 was `cdceda66f887ed81a150acb310ae637872af09cd40af242f8c860961eacc8f2b`.

A separate static review found that authenticated `Listener.accept()` can block during an incomplete
handshake and `close()` joins that acceptor without a bound. This finding still needs an actual isolated
half-open-peer regression. Preserve PID-bound authentication and fail-closed command ownership while
repairing bounded handshake/cleanup. The future sccache idle-lifetime issue remains separate and unresolved.
These findings require a guarded continuation within the existing three-round family, not a renamed budget.
