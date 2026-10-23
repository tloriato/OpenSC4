| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| verify-font-renderer4: bounded native small renderer verification | Codex, verify-font-renderer4 (detached HEAD, supervisor publishes branch) | wip: corrected bridge 678/0; new own verify blocked in stats; no full gate or game qualification | Coordinator collect session 38541 / own stats PID 22680; no duplicate build or fourth lane round. |

## Final R3 report (2026-10-23)

The corrected bridge is now executed and passing. Full-gate and game qualification remain
unmeasured because a **new, positively attributed own verification operation** is waiting in
sccache statistics. This is not the historical R2 global-visibility prerequisite. No fourth round,
reset, duplicate build, cancellation, cache RPC, restart or game request was used.

### Admission and source identity

Reused clean detached `ab2bbe75d2000e731462f553bf330c4aa71e0129`; engine tree
`2e94fea473c79636e71162fd453fd3d7b278e505`. R3 manifest matches this lane, kind engine,
round 3, worktree and git_base; codexguard accepted it. Accepted copied HOST clearance
`build-runs/evidence-run4/coordinator-native-font-final-host-process-proof.json`, timestamp
`2026-10-23T05:57:53.938146+01:00`, under explicit root authority. Its references,
open_outputs and errors are empty; root confirms no conflicting Font operation. Proof SHA256:
`25F448804C2C92716E5A5099DA8E5F66C114CF72581611251F9D5742965EE395`.
No isolated global process inventory, CIM/WMI, inaccessible-cwd/cache-environment clearance
or global spawn preflight was repeated. Historical R2 41 denials remain unchanged below.

Commit `6b4ba195` records this admission and source review. The actual verification started
from that clean commit; at 06:14:25 +01:00 HEAD and engine tree still match, with clean tracked
status. No production, fixture, dependency or build configuration changes in R3. No old build
terminal or baseline PASS is inferred. q.py still fails Bash startup with AccessDenied, so reads
use bounded PowerShell. An unsupported `qbuild native` was refused without executing a build.

### Corrected bridge: actual measurements

Independently reviewed published sections 5/6 and the ledger: D=11223344, F=FFEEDDCC,
a=128 gives 88888888 by the published modulo-32-bit formula. Separate D=0/F=FFFFFFFF
fixtures retain 80808080/FEFEFEFE. Neither expected literals nor production changed.

Executed canonical `qbuild.py --wt <WT> verify` once, not another quick or baseline all.
The new build log reaches 118/118, including the rebuilt bridge link and plugin link.
Direct execution of that rebuilt `engine/build-x86/fc4_font_renderer_verify_test.exe` returns:

`font renderer bridge operations=51 pixels=84 callbacks=187 checks=678 failed=0 native-original=0`

The executable's exit is 0. Those are **actual**, previously planned denominators. Its checks
include all corrected literal fixtures, immutable input hashes, intentionally mutated pixel and
extra-trace rejection, same-IID wrong-holder/foreign-image/typed-entry refusal, OFF zero
mocks/owner calls, portable invalid source/target backing, count limit and trace-capacity
refusal. Portable unsafe-input cases are never sent to native. Target entry tests exercise the
plugin raw primitive host, not the original buffer owner or original renderer. The 187 count
is the expected callback-entry denominator; literal equality also validates actual Rust traces.
Pixels are designated positions; full supplied windows and guards are checked separately.
This is no original-native execution or source-read/same-value-store observability claim.

The initial 49/80/175, **643 checks / 2 failures** remains historical raw failure. Original quick
34799 and its exit remain **UNKNOWN**; historical 540 Rust and 220 linked native observations
are not a passing old terminal. R2 performed no build. No current all/clippy/68-CTest terminal
result exists; the link count does not qualify a gate.

### New own operation: preserve for coordinator collection

Actual current operation: unified execution session **38541**, command
`python <MAIN>/.claude/lane-bin/qbuild.py --wt <WT> verify`. It remains active without terminal
output after bounded output collection. No second build was started. The supported wrapper
invokes `fc4verify.py --wt <WT> build ctest`; no current ctest log has been produced.

`build-runs/collect-r3-operation.py` traverses only this Codex process's tool descendants,
matches the explicit own worktree/qbuild command and captures its descendants. It does not
enumerate global processes or establish global clearance. Full evidence is retained in
`build-runs/r3-operation-collection.json` (duplicate ancestry matches are not separate operations).

| Process | PID | Creation epoch | Parent | Actual arguments / status |
|---|---|---|---|---|
| qbuild | 25112 | 1792732144.3912296 | 35928 | qbuild.py --wt this worktree verify |
| fc4verify | 32908 | 1792732144.4300804 | 25112 | fc4verify.py --wt this worktree build ctest |
| stats | 22680 | 1792732194.48161 | 32908 | sccache.EXE --show-stats --stats-format json |

All three cwd values are this reserved worktree. Stats has an ESTABLISHED TCP connection
127.0.0.1:64327 to 127.0.0.1:4597. Open-file lists and full argv/ancestry are preserved in JSON;
they expose only system resource files for these waiting processes, not open build outputs.
The build log stops at link 118/118 (06:09:52 +01:00); wrapper exit/native build exit remain
UNKNOWN. This is concrete new stats blockage, not an inference from old R2 errors. The lane
has not killed any process, changed cache configuration, queried cache RPC, restarted anything
or manufactured success. Root must collect this same operation before any subsequent build.

### Content hashes and unqualified game scope

SHA256 values captured at 06:14:25 +01:00 (not a frozen artifact manifest):

- Fixtures.h: `446706499F58067AE4E861FC63321ADCC0CF80BA416AFC3F18044211452E6AB4`
- Ledger.md: `AFA64E33D59ABF53E16228D93CD00D3D4E2AD41657F355D163705AC167674C10`
- Verify.cpp: `F8D5D9BFE6538A855206737DFEE708FC906A7D76AD38190156324279BFB2685F`
- bridge test.cpp: `011F630DDE5345BA7F26C4CA96FBD2DACEE85A67D6645FA681F581BDF0674629`
- CAPI adapter.rs: `D2E8C40AD88417A37AD72B3ED88BA537E8310BFF81EDAB664A4E7F641047EC1B`
- rebuilt bridge exe: `ED7D008BC02A2A4FEB2F0BC3410501985309CCD08046A3808A8AA6D37DE6153D`
- current build log: `59621DF3F01CB8D35DBFA7D0B6A90C04BA2418FDBA12054C503273367C6B5693`

No DLL/PDB pair frozen, game requests submitted or discarded, native-original operations,
game verdicts, literal CURRENT40/provider counts/defaults, phase/lifecycle/ref/foreign/stall/
shutdown results, 2032 or save/reload measurements. Every such field is **unmeasured**, not zero.
Both matched OFF-first profiles remain unqualified. FR1-3, natural cache/layout/palette producer,
larger holder/span renderer, original buffer owner and invalid-native-backing exclusions remain.
No new spec questions: corrected operands pass; the remaining obstacle is runtime tooling.
ABI exports changed in the original implementation; no ABI version bump or R3 ABI change.

Next: coordinator collects session 38541 and the identified stats child without attributing a
passing terminal. Only a real supported full-gate PASS permits freezing artifacts and the
authorized bounded matched game controls. This final original R3 does not request another lane
round. The retained R2/R1 pending instructions below are historical, superseded by this report.

## Retained R2/R1 evidence

## R2 measured report (2026-10-23)

Admitted manifest `run4-verify-font-renderer4-r2-manifest.json` matches lane,
engine role, round 2, reserved worktree, detached base
`1236e02d1ca63b26b82fe1b1bc3a8b42c3bc981d`, budget 150000 and 100 calls.
Initial HEAD is that base; engine tree is
`2e94fea473c79636e71162fd453fd3d7b278e505`; tracked status is clean.
No production, fixture, dependency, build configuration or artifact changes in R2.

Copied host proof `build-runs/evidence-run4/coordinator-native-font-r1-process-proof.json`
at 05:48:44 +01:00 has no own references, output handles, listener or audit errors.
It establishes drain at that instant only. Original quick session 34799, its exit,
540 Rust results and 220 linked native targets still have **UNKNOWN terminal status**.
The original bridge **643 checks / 2 failures** remains a failure, not superseded.

Fresh read-only psutil audit at 05:52:03 +01:00 is retained in
`build-runs/r2-focused-process-proof.json`, generated by `build-runs/audit-r2.py`
from the copied audit. The only accessible own reference was the audit's invoking
PowerShell PID 22796 (parent 34000, creation epoch 1792731123.579811).
No accessible own open outputs or port-4597 connections were found. However,
**41 AccessDenied errors** prevent claiming a complete current no-old-writer check.
Those include Python processes whose cwd/arguments could not be determined.
Absence among readable processes is not proof that inaccessible processes are unrelated.

Focused cache identity is retained in `build-runs/r2-cache-identity.json`:
PID 32276, `sccache.exe`, creation epoch 1792730273.4603968, parent 19052,
argv `C:\\Users\\tlori\\.cargo\\bin\\sccache.EXE`, listener 127.0.0.1:4390.
Its cwd and open outputs are AccessDenied. This is **not** the worktree's 4597
listener and is not identified as an old lane writer. No cache RPC, process action,
CIM/WMI, global spawn preflight, cancellation, restart or duplicate build was used.
The helper initially used a psutil method unavailable in this installed version;
it was corrected to `connections` and completed. This was an audit-helper error,
not an engine diagnostic.

Independent source review against published font-renderer.md sections 5 and 6.2
confirms the corrected asymmetric fixture uses F=FFEEDDCC with D=11223344, a=128.
Modulo-32-bit packed arithmetic gives H=88008800, J=88808880,
H-007FFF80=87808880, H+00800080=88808880 and
((J>>8 XOR (H-007FFF80)) AND 00FF00FF)=00080008;
the result is therefore 88888888. This derives the literal from the contract,
without using Rust or native output. The separate D=0/F=FFFFFFFF cases retain
80808080/FEFEFEFE. This is a source/arithmetic review, **not executable validation**.

Because the mandatory fresh no-old-writer prerequisite cannot be fully established
from this isolated worker, no checker or build was launched. Corrected bridge,
negative/provenance/portable cases, actual revised denominators, final all/clippy/
68-CTest terminal result remain **unmeasured**. No frozen DLL/PDB, game requests,
native execution, default-setting measurement or native exactness claim exists.
Both matched profiles, lifecycle counts, ref/foreign/stall/shutdown fields and
save/reload remain unqualified. No spec questions: this is an operational visibility
limitation, not a contract ambiguity. The earlier report below is retained verbatim
apart from this lane row; its pending-session instructions are historical R1 status.

Next: coordinator supplies current host-visible writer clearance (including the
inaccessible candidates), without interpreting drain as an old passing gate.
Then use the supported focused build to run the corrected bridge and actual negative
cases, commit stable source, run the single canonical all through actual terminal,
and only after PASS freeze artifacts and perform the authorized matched controls.
No normal-game or native-owner credit is granted by this R2 report.

## Changes

First implementation milestone: `fee9594d`. The latest source additionally strengthens
dispatch provenance, fresh-origin discrimination, safe transport refusal checks, and documentation.
The reserved detached worktree was reused without switching branches or updating shared refs.
Baseline `all` was omitted as explicitly required by this brief; main was already gated.

`font_renderer_verify` / `FC4_FONT_RENDERER_VERIFY` defaults OFF. Its gate precedes fixture
construction and SDK factory acquisition. Load, presave and shutdown hooks run a bounded
constructed comparison when enabled. No natural rendering or city target is intercepted.

The factory route is SDK GetClassObject(08CDA30B,A8BC2A75), with the public SDK's factory/QI
ownership convention and published small-renderer QI reference behavior. All owner calls use
typed compiler-generated i686 virtual dispatch. Immediately before each dispatch, the gate checks
the exact published vtable identity, table image allocation, and the specific typed entry's
published identity and executable original-image allocation. Published addresses are comparisons
only. The larger holder, foreign modules, unknown slots, and changed method entries are refused.
Failure of provenance withholds unsafe dispatch, including Release, and is explicitly reported.
Ordinary owned factory, QI and AddRef references are released through the reference audit.
No count field is modified, no overflow is manufactured, and no original body is called by address.

`FC4FontRendererFixtures.h` defines the native 0x24 record and typed raw primitive target with
32-bit field/slot assertions. Unreachable other target slots abort rather than emulate a buffer
owner. Target methods only expose raw dimensions/depth/format/pitch/base and AL lock results,
and record typed callback tuples. Production Rust owns raster computation through its existing
draw/lock/unlock functions, reached by additive C export `fc4_font_renderer_verify` and a typed
header included from fc4.h. ABI version and generated abi_entries.inc were not edited.

Each implementation receives separate pixel storage and immutable source/palette/record copies.
Per-run input hashes are checked before/after; final bytes, guard bytes, callback sequence and
arguments, and AL results are checked against the independent literal ledger and each other.
Combined input hashes include each run's own pointer-valued record fields, so the two hashes
need not match. No native output initializes Rust. Internal Rust Event reads/stores are never
used as native observations. Same-value stores and initial palette reads remain unobservable.

## Measured validation and remaining gate

- Initial quick build: `python <MAIN>/.claude/lane-bin/qbuild.py --wt <WT> quick`.
  **Still outstanding**, unified execution session **34799**. No terminal exit has been returned.
  `build-runs/fc4verify-quick-test.log` records **540 passed, 0 failed**, plus 0 doc tests.
  `fc4verify-build.log` reached **220/220**, including DLL and bridge-test links, at local
  2026-10-23 05:17:37. These are stage observations, not a passing wrapper result.
  Output collection still returned the same active session at 05:32; no duplicate quick/all,
  process kill, or RPC/WMI process discovery was used. OS PID was not exposed by the tool;
  no guessed process ID is claimed as the operation owner.
- Initial bridge executable: **49 operations, 80 designated pixel positions, 175 expected target
  callback entries, 643 checks, 2 failures**. This executable did not execute the original renderer.
  Both failures were the dword asymmetric literal: fixture foreground FFFFFFFF was accidentally
  paired with the published expected value for FFEEDDCC. The ledger records the correction;
  expected values were not changed or derived from production. The corrected two operand pairs
  are now separate fixtures. This corrected suite has **not been rebuilt or rerun**.
- In that initial run, all intentionally changed-pixel/extra-trace checks rejected the mutation;
  holder-vtable, foreign-image and OFF-zero-mocks/zero-owner-call checks passed. Later typed-entry
  refusal and portable-only invalid-backing/capacity checks are source-only until rebuilt.
- Current planned suite: **51 operations, 84 designated pixel positions, 187 expected callback
  entries** per phase. These are planned denominators, not measurements of the current source.
  Each operation compares the entire supplied pixel window and its two 16-byte guards in addition
  to those designated positions; guard bytes are separate checks, not pixel-algorithm credit.
  Callback counts include lock/unlock. Repeated phases do not add algorithm coverage.
- Adapter rustfmt check passed. Checking touched lib.rs recursively reports existing formatting
  differences in other modules (first advisors.rs:2598; 645 diff lines); no mass formatting applied.
  `git diff --check` passed. Relative paths do not work for this wrapper's fmt step; use absolute paths.
- `q.py` could not start Bash (`Bash/Service/CreateInstance/E_ACCESSDENIED`); the bridge executable
  was collected directly in PowerShell with bounded output. This did not authorize any fallback build.
- **Final all not run, clippy not measured, full CTest not measured.** Starting another build while
  the original operation lacks a terminal result would violate the explicit collection-first rule.
  Current source includes changes after the compiled artifact and is not gate-qualified.

## Game evidence and limits

**No requests, no native-original operations, no verdicts, no frozen DLL/PDB/source hashes.**
The native optional-provider-none profile and literal CURRENT40 profile are both unqualified.
No 0/0 OFF result is called exactness. No save/reload, 2032, reference-warning, foreign-pointer,
stall or shutdown-result claims are made. There is no outstanding game request.

The ledger covers published clipping/source offsets, fresh record origins, source width not bounding
extent, threshold/palette cases including a full 256-entry palette, 16/24/32-bit packing, dyadic
asymmetry, odd pitch, emptiness/query order, and ignored lock modes/AL forwarding. Safe transport
refusal vectors are Rust-only and never sent to native unchecked pointers. FR1–3 and all natural
font/cache/layout/producer, larger-holder, span-renderer and original buffer-owner coverage remain
excluded. No new spec question is filed: the observed failures were fixture transcription errors,
not a measured native/spec disagreement.

## Next steps

1. Collect the actual terminal output of original quick session **34799** before starting any build.
   Logs alone do not establish terminal success. Resolve the wrapper stall through the coordinator;
   this lane did not kill processes, restart tools or request a second operation.
2. Rebuild and run the corrected bridge and portable-only negative cases. Its original binary is stale.
3. Commit stable source, run **one** final `qbuild.py ... all`, collect actual terminal cargo/clippy/
   native build/full CTest counts, and retain source hashes. Only then freeze durable DLL/PDB copies.
4. Under the queue policy, submit OFF then ON matched optional-provider-none / explicit-budget-ON
   runs, and OFF then ON matched literal CURRENT40 / explicit-budget-ON runs, unique labels,
   one outstanding request, same frozen artifacts/settings within each pair. Establish/log the
   actual explicit 40-provider list and defaults; no guessed profile or new save/reload claims.
5. Preserve all mismatches/refusals; file numbered questions for actual contract differences.
   Keep the merge queue WIP until the final gate and required real-game evidence exist.
