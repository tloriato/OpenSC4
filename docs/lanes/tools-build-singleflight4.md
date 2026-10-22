| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| tools-build-singleflight4: build/check single-flight | Codex / `tools-build-singleflight4` (tooling; reserved branch, detached HEAD) | in progress: same-family round 2 ownership repair; original failed evidence preserved | Repair descendant containment, venv peer authorization and bounded authentication; validate with supplied pytest venv |

## Handoff

- Goal: repair all three demonstrated ownership defects on original family, no engine/build/game work.
- Baseline/report tip dd50e233; copied baseline scripts/JSON remain untouched under build-runs/evidence-run4.
- Inspected helper: direct Popen.communicate drains no DEVNULL descendants; Listener.accept authenticates without deadline; venv peer differs from Popen PID.
- Next: implement Windows Job containment before command entry, strict venv redirector authorization and bounded authenticated accept/close.
- Required interpreter: C:/Users/tlori/FreeCity4/build-runs/pytest-venv/Scripts/python.exe.
- Old full-suite log: build-runs/coordinator-full-tools-2231.log (9 failed, 168 passed); do not overwrite.
- No current run labels; cache idle DRAFT remains deferred. No new queue publication until measured validation.

## Changes

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

## Measured validation

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

## Limits and next step

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

## Coordinator validation and review, 2026-10-22

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
