| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| re-watercraft-completion4: native watercraft contracts | Codex / `re-watercraft-completion4` (RE, reserved detached worktree) | WIP: original FINAL round 3/3 finished; 16864867 AI path and 5ebe4c84 numeric boundaries; questions/links/diff PASS; 75/38073 unchanged; no ABI/game/shadow or owner qualification | Coordinator review bounded publication; WC3–WC7 remain open and core-watercraft4 remains parked. Original three rounds exhausted; no fourth round or family reset. |

## Report — original FINAL round 3/3, 2026-10-23

**Meaningful bounded WIP publication.** The newly published ordinary AI path and selected movement precision
rules do not qualify a complete owner, complete byte coverage, default behavior, native equivalence or engine.
This is the original family's final authorized research round; there is no automatic fourth round/name reset.
The historical R2/R1 reports below describe earlier work, not additional work claimed by this round.

### New publication and source

- `16864867`: added [watercraft-ai-motion.md](../specs/watercraft-ai-motion.md), linked from watercraft §5.1a.
  Owned 00571880 branch/reset/waypoint/query/consume order; midpoint geometry; phase latch; mutable
  alignment helper; turn/translation and speed/turn approach; live pose and post-callback history order.
  The alignment threshold is strictly <2 degrees, near-point threshold strictly <7, and short-distance
  threshold strictly <float 0.1. First versus second turn feedback retains the native sign asymmetry.
  Native callers pass one float time and continue into Movement_PositionNew; no foreign final-motion
  callback, Automaton accumulator or implementation algorithm replaces owner computation.
- `5ebe4c84`: watercraft §6.2.2c specifies selected float loads/FST/FSTP/retained arithmetic boundaries,
  both-nonzero turning selection, signed-zero/zero/reverse-time behavior, exact matrix/translation store
  order, byte revision wrap, lift versus live position, and callback mutation/lifetime consequences.
  This supplements R2 equations; its supported/partial/unknown body checklist is in the owner inventory.
- Added independent proposed/unexecuted AI next/no-next/short/reached/equality/zero-distance/turn/clamp
  cases, explicit primitive query replies and history/order expectations. dt=400 gives float D=1,T=40;
  dt=12800 gives D=32 and a 16-unit cap with desired Speed=1/8. Exact 2-degree geometric-input bits
  are explicitly unknown: the equality case is a comparison-stage trace, not a fabricated native golden.
- New rational publication-stage witness: old P=(1,1,1), output=(2^-25,2^-25,2^-25), delta
  -33554431/33554432. At nearest-even and x87 precision>=53, retained x gives 33000000, spilled y,z
  give zero; native translation stores z,x,y. This is a derived stage case, not proof a whole helper
  reaches that triple. R2's separate 39C479FE/FD acceleration witness and FST correction are retained.
- Source starts at clean detached 3d591dba41659df37f7d608c036832ee5c44ce25. Read supplied host proof and
  actual round-3 manifest; lane/RE/round/WT/initial source match. Supplied canonical host GO accepted;
  no detached/scoped spawnpreflight repeat, permission expansion or guard bypass.
- Fresh read-only original SHA256 matches export metadata, program SimCity 4.exe, base 00400000:
  `3bc5c7fe807fe5aa24a3abb06e40f13486670bc1150612c09baf4ecedafe1a4b`. This matches the supplied Windows
  1.1.641 artifact/export, not an independent version-resource audit. AI bounds 00571880–0057235A,
  2779B; geometry 00569940=1314B, 00569E70=1172B, 005682F0=464B. Census remains 75/38073, not 79/38556.

### Validation and limits

- Canonical main-checkout func/callers/callees/vtable/instruction-byte queries first; only necessary local
  decomp supports. Constants decoded from the matching original. No engine/tests, other contributor WT,
  private implementation, crash/RNG, route/dock expansion, scheduler or forbidden documents were read.
- `python tools/questions.py index`: PASS, generated index unchanged. `python tools/questions.py check`:
  PASS. Relative file links: 15 checked, zero missing. Both new vector section anchors: PASS.
  `git diff --check`: PASS. Scope review: only own report and five watercraft spec/question files changed.
- Cargo/clippy/ctest/builds/game/native probes/shadow: n/a, none run. No runtime original execution, game
  request, cache/target-control RPC, process action, external message, agent, merge, rebase or push.
  No C ABI/export/version/table changes or ABI proposal file. Scripts/commit messages remain ignored local
  build-runs artifacts; no decompiler/assembly listing or game content is committed.
- No live REA request was needed. Live target readiness/server admission remains UNKNOWN; there was no
  retry, open/close/retarget/new server or invented REA failure/fallback claim. Existing canonical evidence
  sufficed. Normal runtime guard status remained within the 150000 budget, with no STOP/OVER observed.
- Git printed packed-refs.lock permission errors but created both detached-HEAD milestone commits and
  left a clean tracked tree. No shared refs/config/hooks repair or maintenance was attempted.

### Questions and remaining gaps

No question filed or moved to answered. WC4/WC6 subpart progress is recorded in
[questions/watercraft.md](../specs/questions/watercraft.md); WC3–WC7 all remain OPEN, and core-watercraft4's
park is unchanged. Remaining AI/geometry limits include complete coefficient spill equivalence, exact
acos/FSIN/FCOS bits, active x87 control word, shared pose-setter effects/lifetime, direct zero-time
exceptional feedback, nonfinite/underflow/overflow/degenerate/library-fault inputs and inherited animation.
Existing census/constructor/destructor, crash/bump/RNG and route/decoder/backtracking/ferry/tow/dock/tie
residuals are retained; none is silently closed. Ordinary callback preservation is an explicit domain,
not a native safety fallback. Next is coordinator review of this bounded WIP docs publication only.

## Report — original round 2/3, 2026-10-23

**Bounded WIP publication, not complete movement/control or whole-owner closure.** This is exactly the second
original research round in the existing family. No restart, renamed family, automatic third round, implementation
qualification, byte promotion or change to core-watercraft4's parked WC3–WC7 status is implied.

### New publication

- `45b12f3d`: same-sign acceleration/preservation and signed inverse feedback, ordered right/left steering,
  neutral/countersteer, tween reset. Corrects R1's intermediate-rounding formula using the FST instruction.
- `adff3bd7`: turning radius/center/sea-level query/rotation and output geometry; bank/pitch/basis/lift
  algorithms. Exact intermediate spill precision and degenerate normalization remain explicitly partial.
- `0bb56d6c`: command fire/AEMS, skier/rope full-transform ordering, heading acos path and inherited
  edge/timer/tween/channel tick contracts. Added watercraft-command-vectors.md and per-body statuses in
  watercraft-owner-inventory.md; refined compound question residuals without moving any to answered.
- Explicitly reconciled old summary conflicts: weaker same-sign requests do not decelerate; reverse feedback
  is signed; steering's channel adjustment uses raw milliseconds with asymmetric +/-dt; skier uses the current
  whole transform, not an owned trailing-point formula; whalefire is an AEMS request. automata.md's vehicle
  accumulator is not watercraft control, and its forced-flag attribution to Windows 00508C50 lacks those writes
  in that body. automata.md itself was not edited.

### Evidence, checks and limits

- Canonical main-checkout re_query.py function/callee/instruction queries came first. Existing bounded local
  decompilation supported the trace; numerical/branch/store claims were checked against instruction bytes.
  No decompiler/assembly text is committed. No engine or other contributor implementation was read.
- Export program SimCity 4.exe, base 00400000, SHA256
  `3bc5c7fe807fe5aa24a3abb06e40f13486670bc1150612c09baf4ecedafe1a4b`; read-only original hash matched.
  This matches the supplied Windows artifact/export, not an independent product-version-resource audit.
- Actual sizes: turning 00569940=1314B, orientation 00569E70=1172B, bank 005682F0=464B,
  inverse feedback 005671D0=312B; AI 00571880=2779B remains OPEN. Shared/inherited helpers are identified
  separately from owned algorithms, with no fabricated missing four-function census entries.
- Independent local arithmetic derivation produced signed inverse examples about +1.80980265/-1.14554119
  at +/-1/8 speed, and a rational retained-vs-reloaded product witness 39C479FE versus 39C479FD. Published
  exact rational/power-of-two branch vectors distinguish near-zero/reverse/cross-zero speed, neutral and
  simultaneous steering, clamps, equal-magnitude tween, sea-level boundary, repeated reverse lift, event
  failure and inherited latch equality. No RNG draws occur in these owner command/geometry bodies;
  crash RNG order remains OPEN. These are **proposed, unexecuted** conformance vectors, not native goldens.
- Reviewed the whole new published sections and vector companion against the trace, including the subtle
  unrounded baseline comparison in steering. `python tools/questions.py index` and `check`: PASS;
  generated index unchanged. `git diff --check`: PASS. Cargo/clippy/ctest n/a; no game requests, runtime
  original calls, builds, tests, shadow qualification or ABI changes. No ABI proposal file needed.
- REA tool discovery exposed only procedure_pseudo_code/batch_decompile. Live NoApp capability and server
  admission remain UNKNOWN; no successful full server handshake or live admission is claimed or used.
  No live procedure retry, open/close/retarget/new server or cache/target-control RPC occurred. Canonical
  existing export/original evidence sufficed for the bounded publication; no REA failure was invented.
- q.py failed at Bash CreateInstance/E_ACCESSDENIED; subsequent commands used bounded PowerShell. Git
  emitted packed-refs.lock permission errors but created detached-HEAD commits; no shared ref/config repair.

### Residuals and next steps

WC3–WC7 remain open in docs/specs/questions/watercraft.md. Census is still 75/38073 versus 79/38556,
four functions/483 bytes unexplained. AI interpolation (00571880), all geometric spill/exceptional cases,
inherited animation/remaining owner-drive details, crash/bump/RNG (0056A310/0056A440/0056AF90), routes,
decoder/backtracking/ferry/dock/nearest ties and other R1 residuals prevent a complete owner contract.
Further work needs a coordinator decision within the one remaining original round; this lane does not restart.

## Prior round-1 report (preserved)

This is a **partial evidence-backed contract**, not complete cSC4Watercraft ownership or an implementation-ready
79-method closure. One guarded round; no agents, restart, merge, push or external traffic.

### Changes

- `bd529644`: substantive lifecycle/serialization/input arithmetic and complete straight-translation branch,
  committed within the requested early-work window. Native fourth flag is 0x8000; base Init/effect failures do
  not stop owner initialization; both lifetime fields clear; Read accepts exactly owner version 2.
- `9267ce96`: animation argument provenance, timers/wakes/lifetime/callback ordering, direction step table,
  following excess-distance translation and collision selection/callbacks. The second animation float drives
  owner motion/timers, correcting the old inferred split. Braking can cross zero in a single step. Following
  translates excess distance instantly, independent of dt/p39, using the follower's own radius+10.
- Added `docs/specs/watercraft-owner-inventory.md`: all 75 currently attributed entries with exact addresses,
  body sizes, committed names and attribution rules. The requested 79/38556 could not be reproduced; current
  result is 75/38073, a four-function/483-byte discrepancy. No guessed entries, signatures or method names.
- Corrected helper grouping: Movement_PositionNew directly calls 0x00569940, 0x00568230 and 0x00569E70;
  0x0056A440/0x0056AF90 belong to crash motion. Crash owner entry notifies (0xB,0). Equal-size collisions affect
  both participants; sinking an AI after a player win does not itself set that AI's collision-sunk byte.

### Evidence and measured validation

- Used configured local re_query.py export queries first, then bounded class/unnamed decomp reads; verified
  numerical/branch/callback claims against native instruction bytes and cited addresses in the spec.
- Local-only `build-runs/owner-audit.py` uses coverage Attribution, not its main routine; reads no engine inputs.
  Both main and lane-base attribution inputs reproduced 75/38073. Export body bytes, not address differences.
- Local-only `build-runs/read-constants.py` decoded PE constants: c=0.9100000262260437 and h=1.5707963705062866.
  Independent formula evaluation gives default half-input speeds about +0.03704996 and -0.05530950. Spec vectors
  also cover version rejection, failed acquisition ordering, straight translation, unequal tick times, lifetime
  equality, callback suppression, cell deltas, following translation and collision ties. These are constructed
  conformance expectations, not executed native goldens or tests of the portable implementation.
- `python tools/questions.py index` and `check`: PASS; index content unchanged. `git diff --check`: PASS.
  Cargo/clippy/ctest: n/a for spec-only changes. No game requests/runs, shadow results or ABI changes.
- No REA call was needed; all evidence used existed locally. No invented probes or fallback claim.
- Git emitted a packed-refs.lock permission warning during commits but created the detached-HEAD commits;
  no shared ref/config repair was attempted.

### Questions and remaining work

WC3-WC7 remain in `docs/specs/questions/watercraft.md`, each with published subparts and explicit residuals.
None is moved to answered because each compound request still has untraced requirements. WC1/WC2 remain open.

- WC3: reconcile census, constructor/destructor closure, all signatures and inherited lifecycle/step internals.
- WC4: complete Engine_AIBoatMotion (0x00571880), turning (0x00569940), orientation/height (0x00569E70), and
  inherited animation/player steps. Outer orchestration and straight translation are not full movement closure.
- WC5: footprint/inanimate geometry, bump (0x0056A310), crash dispatch and full crash-motion/RNG bodies
  0x0056A440 and 0x0056AF90; sink/heel equations remain open.
- WC6: same-sign acceleration/inverse feedback, steering, sound, fire/skier positions and tween reset details.
- WC7: Engine_AIPickLocation/backtracking, compressed route decoder, ferry exhaustion, dock ownership/returns,
  degenerate heading/away vectors and nearest-boat ties. Following angle internals remain unclosed.

Public edits are confined to this lane report, watercraft specs and the watercraft question file. No engine,
tools, maps, ABI tables or unreconciled generated edits. Coordinator should retain WIP/no whole-owner credit.
