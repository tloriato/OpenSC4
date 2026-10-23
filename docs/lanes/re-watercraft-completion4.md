| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| re-watercraft-completion4: native watercraft contracts | Codex / `re-watercraft-completion4` (RE, reserved detached worktree) | in progress: original research round 2/3; R1 evidence preserved; no owner qualification | Trace movement and command residuals from canonical Windows instructions; publish bounded algorithms and retain unknowns. |

## Handoff

- Goal: original round 2/3, movement/command first; no implementation, runtime calls or owner credit.
- Starting tip: a2f2b75c; R1 substantive commits bd529644 and 9267ce96 are already present.
- Done: 45b12f3d publishes same-sign speed/inverse feedback, ordered right/left steering and tween reset; corrects R1 FST rounding.
- Done: published turning-radius/center/sea-query algorithm and bank/pitch/lift geometry with explicit spill-rounding residuals.
- Next: independent vectors and command side-effect/inherited-channel contract; final consistency check and bounded WIP report.
- AI 00571880 (2779B) has heavily mistyped local decomp; only initial queries made; do not claim full AI interpolation.
- Original executable hash equals export executable_sha256; no target-control tools or server admission capability exposed.
- Export identity: SimCity 4.exe, SHA256 3bc5c7fe807fe5aa24a3abb06e40f13486670bc1150612c09baf4ecedafe1a4b, image base 00400000.
- 00569940 is 1314B, 00569E70 1172B; forward bank helper 005682F0 464B; 005671D0 inverse feedback 312B.
- 009F0090 export TransformDirection is actually acos-like math (009F00AD: sqrt((1+x)*(1-x)), FPATAN; endpoints 0/pi).
- Turning calls water global B43CF4 slot29 GetSeaLevel, not spatial water query; second conditional query return is discarded.
- Census remains 75/38073 versus old 79/38556, four functions/483 bytes unexplained.
- Local q.py Bash wrapper failed with CreateInstance/E_ACCESSDENIED; use bounded PowerShell commands.
- Only configured REA tools are procedure_pseudo_code and batch_decompile; no target-control tools permitted.
- No games, ABI changes, tests, builds, engine reads, agents, refs changes, merge or push.
- Manifest: C:/Users/tlori/FreeCity4/build-runs/run4-re-watercraft-completion4-r2-manifest.json.
- Prior round report below is preserved until the final round-2 report replaces this handoff.

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
