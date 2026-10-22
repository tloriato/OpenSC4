| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| re-watercraft-completion4: native watercraft contracts | Codex / `re-watercraft-completion4` (RE, reserved detached worktree) | WIP: substantive contracts committed; current census 75 functions/38073 bytes versus brief 79/38556; questions check and diff check PASS; WC3-WC7 partially resolved, remain open; ABI unchanged; no game/shadow claim | 1. Reconcile four-function/483-byte census discrepancy. 2. Trace remaining AI, turning/tilt, crash, command and route bodies named below. 3. Reconcile portable subset before whole-owner credit. |

## Report

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
