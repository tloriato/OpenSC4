| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| tools-build-singleflight4: build/check single-flight | Codex / `tools-build-singleflight4` (tooling; reserved branch, detached HEAD) | wip: partial cache-launch quarantine; final supplied-venv pytest 207 passed in 228.91 s; compile/diff/doccheck passed; service-owned output repair and real cache reuse proof remain missing; ABI unchanged; game n/a | Keep held; review the precise missing service protocol and failed owned compiler fixtures below; no new round or promotion is requested |

## FINAL3 result: partial quarantine, service repair unimplemented

**Keep this lane WIP.** The root-reviewed client-side/storage service repair was not completed.
The new code rejects detected cached launches before execution; it does not admit a verified
storage service, implement service-control RPCs or demonstrate whole-worktree output ownership.
Passing tooling tests must not promote this partial result. This ends the original family's final
round; no fourth round, budget reset or weaker invariant is requested.

### What changed

- `96fd83c4`: added `build_cache.py` and rejection at actual qbuild/fc4verify log-entry boundaries
  and guardian command execution. Nonempty Rust wrapper settings (including Cargo environment
  settings), direct cache executable names and explicit CMake cache/launcher switches reject with
  an actionable missing-proof diagnostic. The guard has no proof-token bypass, legacy fallback,
  cache-off fallback or process exemption. **Detected cache-enabled checks now fail explicitly.**
- `c321ce7a`: actual wrapper/lease regressions and a fix for explicit `env={}` being replaced by
  the guardian's inherited environment. Tests cover qbuild, CLI verify, imported verify, nested
  delegation and direct Lease file/capture calls. Contenders are BUSY while acknowledged boundary
  entry holds the actual OS lease; sentinel logs/verdicts stay unchanged, then an uncached harmless
  fixture reacquires ownership after the rejection. No writer is launched at this boundary, so this
  is rejection/recovery evidence, **not** the requested held-service-writer drainage matrix.
- `4d094c35`: pinned installed-binary effective-mode and failed-handshake regressions. These use
  isolated loopback wire sinks, real CLI processes and Windows Jobs. They never service Compile,
  launch a daemon or execute a compiler.

Existing cache configuration, ports, directories, idle policy, endpoints and hash inputs remain
unchanged. No `CARGO_*` ownership token was added. Clippy's deliberate `RUSTC_WRAPPER` removal remains.
No service is exempted from Job accounting. Authentication, venv identity, delegated claim checks,
kernel lease/inode and conservative crash recovery were retained and pass the full suite.

The detector cannot discover arbitrary service calls hidden inside commands, copied/renamed direct
executables or all Cargo/CMake configuration files. It is not a comprehensive service containment
boundary. Config snapshots/races, trusted executable enforcement and endpoint/server identity
validation are **not implemented**. The pin in the regression is a test fixture, not a production
trust validator. Unknown installations skip that capability fixture rather than claim support.

### Intake and actual installed-binary evidence

Reused clean detached `53ed3bc47cf16101a3a7cbc4078ccf9bc02c978f`. Read the copied actual r2 result:
WIP, with 195 passing tests; the earlier pre-finish review was not treated as implementation approval.
`build-runs/r3-intake-02.json` records native process creation identities/images/command lines and
no old recorded peer/guardian PID matches or attributable old singleflight check. CIM access was
denied, so the inventory used Toolhelp and native command-line queries. Other live lanes were untouched.
The baseline helper matched copied846 exactly, SHA256
`982f23a37efe9191da703ad749ba5f6d85a480f78c4a52d706959071b12b7806`.

The copied public ZIP verified as SHA256
`8056ff89839725c64c2f878a0a703ab4e3f68e90ce192f9d68ce90a770dfdcbb`;
all 70 manifest file hashes verified before extraction into `build-runs/sccache-source/`.
Only that local source snapshot was inspected. Installed executable:
`C:/Users/tlori/.cargo/bin/sccache.exe`, SHA256
`7600fb817625313850b8a0525c7743c5d44f53494141390223187e57f394ccb0`.

Actual first-wire requests agree with the inspected config branch: enabled client mode sends
StorageHandshake(tag5); false/empty default mode, `SCCACHE_LOG` present including empty, and configured
distributed scheduler send legacy Compile(tag4). `SCCACHE_ERROR_LOG` empty/file remains on the storage
branch. Environment/file precedence, file changes between invocations and malformed bool config are
exercised. The 20 config cases and four old/partial/malformed handshake responses pass; after those
client-side handshake failures, only tag5 was observed, with no legacy fallback or successful check.
This does **not** prove a compatible handshake with an actual daemon, authenticate one, establish all
owned clients' dataflow, or protect against config changes during a production invocation.

### Failed isolated compiler capability fixtures

`r3-isolated-real-cache-01..04` are retained FAIL labels, with JSON/logs and per-fixture evidence under
`build-runs/r3-real-cache-*/`. The probe script evolved between failed attempts; these are development
experiments, not stable-source promotion tests. Every attempt stopped before an object compilation:

1. `01`: `vcvars32.bat` execution returned **Access is denied** before service startup.
2. `02`: direct compiler discovery/sysroot query succeeded, but the new service Job had extra members;
   the fixture rejected them instead of assuming the launcher or daemon was the only member.
3. `03`: retained kernel identity proved an additional live `C:/Windows/System32/conhost.exe` member
   (creation FILETIME134371835192068453); its five-second handle wait returned258. It was not exempted.
4. `04`: fixture startup requested CREATE_NO_WINDOW; startup acknowledged an owned loopback endpoint,
   then native endpoint-query DLL loading failed with **PermissionError13 / WinError31**. No compatible
   actual StorageHandshake or Rust/C++ miss/hit was established. No further compiler attempts ran.

Only test-owned scratch cache/service/endpoints were started. They used a known 30-second fixture idle
setting, not a change to production policy. No shared daemon was probed/stopped, no broad kill or
compiler replay occurred, and no member was ignored by name. Failure-fixture observer JSON records
named Jobs later absent and selected recorded startup PIDs unavailable. It explicitly records
`drained:null` and the lost startup-handle limitation; **do not treat these observations as complete
retained-kernel exit proof or as authorization to clear a production uncertain owner**.

### Validation on final committed source

The single full command was
`C:/Users/tlori/FreeCity4/build-runs/pytest-venv/Scripts/python.exe -m pytest -q tools/tests`.
It printed **207 passed in 228.91s (0:03:48)**, exit0. Source was committed and clean at start and finish:
head `4d094c352b04f48ecc16f3561df8c78793af136c`, tree
`148c8477618287c417a3094dcdde8338eec0d5dd`; both stayed identical. UTC start
`2026-10-22T23:04:30.866067+00:00`, finish `2026-10-22T23:08:20.611382+00:00`.
Helper SHA256 `9841053acee267ba203cdbb769404e3dda6f4ae74658478ea6f1b0894cd0b488`.
The capture records the provided venv executable/prefix and base interpreter separately, no PYTHONPATH
injection, and identical hashes for all seven changed Python source/test files before/after.
The final documentation commit changes no tested Python source.

| Check actually run | Printed result | Artifact under `build-runs/` |
|---|---|---|
| Focused actual-boundary pytest | 10 passed in 19.62s; exit0 | `r3-cache-focused-01.log` / `.json` |
| Focused pinned installed-mode pytest | 2 passed in 8.25s; exit0 | `r3-installed-mode-focused-01.log` / `.json` |
| One full supplied-venv tooling pytest | 207 passed in 228.91s; exit0 | `r3-final-tools-pytest-01.log` / `.json` |
| `py_compile` on seven changed Python files | no errors; exit0 | `r3-final-compilation-01.log` / `.json` |
| `git diff --check 53ed3bc4 HEAD` | no errors; exit0 | `r3-final-diffcheck-01.log` / `.json` |
| `tools/doccheck.py check` | doccheck check: ok; exit0 | `r3-final-doccheck-01.log` / `.json` |

Focused runs retain their dirty development-source provenance; the clean full run includes both modules.
Checks ran sequentially. `r3-final-validation-01.json` contains all four stable-source records;
`r3-evidence-index.json` hashes 48 artifacts. No full engine Cargo/clippy/CMake/CTest gate or game ran;
those gate fields and game/shadow are n/a. No engine, ABI, game install or provider/orchestration changes.
Questions filed: none.

### Historical failures and remaining work

Preserve original dd50e233 **9 failed / 168 passed / 306.40s**, all **6/6 descendant failures**, r2's clean
**195 passed / 207.73s** on846, and all six actual-Lease external-service violations on the pinned helper.
The service JSON still records **6/6 overlap_proved:true** with acknowledged writers alive before/after
admitted contenders and writer exit waits0. Those fixtures used actual Lease, not actual wrapper dispatch
or real sccache, and do not establish the cause of any production cache warning.
The distinct newly launched persistent-service case remains: request writer already completed,
five-second drain still failed, and contenders remained uncertain even after acknowledged service exit.
No historical failure was rewritten or cleared by switching mode for later requests.

Still missing: narrow guardian-owned authenticated storage bootstrap; exact existing/restarted service
creation/role/environment/endpoint identity; configuration race enforcement; all owned clients proven
client-side through actual Cargo/CMake/Ninja/Rust/C++ paths; unknown-member classification without subtree
exemptions; acknowledged held-writer/crash/timeout/cache-hit/preprocessing drainage and justified recovery;
and real isolated Rust/C++ cache misses followed by hits with worktree outputs owned throughout.
The requested service repair and complete ownership invariant therefore remain unproved.

No keepalive companion was adopted. The generic cache-lifetime DRAFT remains separate. Source facts still
allow late PutRaw EOF to lose upload/stats after successful compilation, and early preprocessing/handshake
EOF to fail; blocking IPC still needs an outer deadline. Nothing here proves historical910.341s fallback
causation, cache EOF immunity or permission to replay a completed compile. Coordinator review should keep
this result held and assess these exact missing implementation/proof obligations within the original scope.

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
