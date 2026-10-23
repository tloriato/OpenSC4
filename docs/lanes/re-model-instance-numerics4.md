| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| re-model-instance-numerics4 — MI11–13 primary numeric research | Codex / re-model-instance-numerics4 | wip: MI12/13 answered, MI11 partial | Complete native entry-path precision/rounding preservation proof before claiming numeric equivalence. |

## Report

The bounded research round is complete with MI11 explicitly unresolved in part. This is documentation evidence,
not whole-owner implementation, native equivalence, runtime or shadow credit. No ABI exports/proposals changed.

### Answered and partial checklist

| Item | Result and published location |
|---|---|
| MI13 unsigned normalization | Answered, DIFFERS from conservative failure: §6.1 uses unsigned comparison and wrapping addition. |
| MI13 interior zero | Answered: success; final scales (.5,1,1,.5,1). Zero at zoom2 resets source and writes 1, not immediate failure. |
| MI13 initial zoom4 zero | Answered: success with scale4 unwritten; final lower scales (8/73,16/73,32/73,1). |
| MI13 failure/order/write controls | Five complete literal resource-word fixtures in §6.1.1 include nonzero wrap, I=0 all-zero failure, and I=0 partial-write failure; no rollback. Sources are conceptual, not invented stored indices. |
| MI12 independent material copies | Answered, MATCHES staged interpretation: vector15 observes the second unbound frame copy, then binds it. Both factory-returned instances have undergone SetModel. |
| MI12 ownership/order | §4.1 proves frame build before SetModel/publishing, retain-new/store/release-old model and binding order, independent scalar bytes and shared counted bindings. Every SetModel inspects its own array, including same-pointer rebinding. |
| MI11 point stores | §2.2.1 specifies kinds0–5/default for all three vertices, row order, intermediate storage and final store order. |
| MI11 ray/length schedule | §2.2.2 specifies E1/E2, reciprocal, T/u/Q/v/t stores versus retained values; LengthOfLine has no memory rounding before nearest comparison. |
| MI11 proved mode writers | §2.2.3: application initialization explicitly selects PC24, preserving RC. Parser can conditionally leave PC53; inspected Lua Pause temporarily selects PC53 and restores on ordinary return. Conditional CRT request selects PC53. |
| MI11 actual complete native entry modes | **WIP/unknown**: full startup/thread/indirect callback routes and their entering rounding/precision controls for bounds, pick, Advance and processor. Unknown external CW remains input; no blanket f64/x87/24/53/64 policy. |
| Independent numeric examples | §2.2.4 N1–N6 discriminate PC24/53/64 cancellation, asymmetric stores, transient nearest comparison, determinant threshold and RC. Rational/bit-level derivations are independent of Rust. |

MI12 and MI13 were moved to `docs/specs/questions/answered/model-instances.md` with dated lane answers.
MI11 remains in `docs/specs/questions/model-instances.md` with its precise partial result and residual.
Prior MI3–10 rules and the canonical twenty-vector sequence remain; only vector15's staging was clarified.

### Primary sources and measurements

- Export-first queries over original Windows instructions, native caller/callee references and data; local source
  material only, no engine/tests or portable implementation read. No live REA query was needed; live target status
  remains unknown, and no session repair/preflight was attempted.
- `bin/SimCity4.exe` SHA256 matches export identity:
  `3bc5c7fe807fe5aa24a3abb06e40f13486670bc1150612c09baf4ecedafe1a4b`, image base `00400000`.
- Byte comparisons: 1,104 decoded instructions, zero mismatches across ray165, length21, MeshPick601,
  SetModel47, ReadAnim241, precision setter/getter29. Read raw P constants and kind jump table from this same image.
- ReadAnim `00808660`; SetModel `00801C40`; frame copy `00806E41–59`, helpers `00802240`/`00802070`;
  mode setter/getter `008929E0`/`00892A0A`; app initialization `0045236C`; parser `0067BA40`.
- Control writer scan found 138 candidate instructions, 25 below CRT area `009E0000`: 24 conversion sites and
  one precision setter. Only the cited conversion pair and selected CRT/native paths were traced in detail;
  this is not a complete preservation proof of all 138 sites or callbacks.
- Ignored reproducibility helpers: `build-runs/model-numeric-evidence.py` (identity/data/instruction comparison),
  `build-runs/model-numeric-vectors.py` (exact rational rounding and integer-square-root derivation). Their outputs
  are analytic/source checks, not execution of native helpers. q.py could not start Bash (E_ACCESSDENIED);
  subsequent queries used capped PowerShell and local Python.

### Validation and commits

- `python tools/questions.py index` and `check`: pass. `git diff --check`: pass.
- Independent numeric script: PC24/53/64 and all four RC expectations agree with published N1–N6 derivations.
- No build, cargo, clippy, ctest, game run, native execution or shadow comparison performed; all are n/a.
- Milestones: `96965f53` (MI13/MI12), `1acef253` (MI11 schedules/mode boundary); final commit adds vectors,
  provenance, factory order, partial question wording and this report.
- Reserved detached HEAD retained. No branch/ref changes, merge, rebase, push, restart or queue entry requested
  or performed. Supervisor publishes commits. No excluded owner operations were researched.

### Remaining work

Trace complete normal native caller paths and all intervening mode setters/restores, including external/indirect
callbacks, to prove entering RC/PC for each published numeric operation. Until that evidence exists, keep MI11
open and fixed portable modes explicitly labeled as portable limits. No next round is started by this lane.
