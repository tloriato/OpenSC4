| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| re-hittrack4: cHitTrackPlayer behaviour | `re-hittrack4` / RE spec, detached worktree | parked incomplete, final round 3/3; 88/110 bodies, 7963/16012 bytes credited; native opcode draft awaiting reconciliation; guard stopped at 151k/150k | Preserve final-round evidence; no fourth round, restart or merge queue. Residual owned functions: DoCommand 1/6754 and containers 21/1295 bytes. |

## Handoff

- FINAL round 3/3 parked by context guard (151k/150k). No fourth round or self-restart.
- Baseline `dbcb08ab` preserved. Credited totals remain 88/110 bodies, 7963/16012 bytes.
- Milestone `281a8691`: substantive native arithmetic/property/wait families §7.2,
  independent vector groups 27–32. Committed before call 30. No whole-body credit.
- Final draft §7.3–7.4 maps the remaining opcode values from native dispatch/bytes;
  covers control/return stack, groups, lists, cache calls and failure surfaces.
- This draft is NOT final-owner completion: table partition, control/service/list
  vectors, question archive/index and body inventory were not reconciled before denial.
- Exact owned residual: 0x0093C229 / 6754 bytes (zero whole-body credit), plus all
  21 / 1295 bytes listed by address/size in questions/hit-track-player.md HT5.
- Next review action (not authorization for another round): audit draft opcode
  partition and examples; reconcile HT1/public questions before any coverage promotion.
- Container native asm was read for all 21 bodies but not contracted. In particular
  DE99 growth selects size+max(size,n), E34F has alias-safe retained temporary.
- External cleanup 0x0093E60A / 338 bytes was read: C0 Release; clear AC,B0,7C,80;
  decrement B633B4; channel C8 cleanup F87C; B4 DE67; vector A0 DE49; list 98 BAEF;
  Release 94,90,8C,88,84,80,7C; stack 78 96EDBC; Release 34; string 20 414250.
- Raw SEH continuation 0x0093E0F9 bytes 8B 7D DC restore EDI from [EBP-0x24],
  then fall through E0FC timer count/retry. Exception matching/runtime remains external.
- Raw 80-word key map B169F8 was read but not yet published; HT3 stays open.
- Scratch: hittrack-command.asm.txt, hittrack-owned.tsv, hittrack-read.py in build-runs.
- No engine/tools/ABI changes, game runs, cargo/clippy/ctest or shadow claims.
- Only diff --check ran for current draft; no final validation or question-index run.
- Guard manifest: C:/Users/tlori/FreeCity4/build-runs/run4-re-hittrack4-r3-manifest.json.

## Preserved round-2 report

## Report — guarded round 2, 2026-10-22

Preserved `1c57242f` and all prior contracts in `docs/specs/hit-track-player.md`.
This round edited only the scoped spec, its open/answered questions, the generated
question index and this report. No engine reads/edits, tracked tools changes, ABI
table changes, coverage promotion, merge, push, branch switch or shared-ref update.
Detached HEAD was committed normally for supervisor publication.

Milestones: `53b0aaec` supplies seven playback/definition/visitor contracts (1939
bytes), committed before call 30. `a83ef7f0` supplies six notification/spatial/
numeric contracts (1639 bytes), plus partial DoCommand decoding without body
credit. Total change: **+13 fully contracted bodies / +3578 bytes**. Final owner
completion is **88/110 bodies, 7963/16012 bytes (49.7% of bytes)**. The other
**22 bodies / 8049 bytes** remain explicitly open. Ready describes the document
publication, not a completed owner or an implemented/verified replacement.

Whole-owner checklist (disjoint body/byte accounting):

| Scope | Contracted bodies/bytes | Open bodies/bytes | Status |
|---|---:|---:|---|
| Preserved identity, state, wrappers, provider dispatch and timer contracts | 75 / 4385 | 0 / 0 | Preserved; external helper boundaries remain explicit |
| HT2-owned: PlayPause, patch, NoteOn, dependent start, visitors, SetTrackDef | 7 / 1939 | 0 / 0 | §6.4–6.7; external cleanup remains below |
| HT3-owned: complete property notification tree and spatial pair | 3 / 924 | 0 / 0 | §5.3; external short-key map remains below |
| HT4: power, UpdatePitch, CompletionTime | 3 / 715 | 0 / 0 | §5.4; answered |
| HT1: DoCommand | 0 / 0 | 1 / 6754 | §7.1 envelope and 11/97 values only; no fractional body/byte credit |
| HT5: containers/source index | 0 / 0 | 21 / 1295 | Exact address/size checklist in open questions |
| **Total** | **88 / 7963** | **22 / 8049** | **110 / 16012 audited** |

Measured behavioral findings: PlayPause's gate may succeed without activation;
its fade division is signed after a wrapping product. NoteOn ignores patch
failure and its note argument. Failed dependent Play retains the list entry.
Definition chains write direct slots without capacity/cycle checks and retain
stale trailing providers. Notifications include float re-reads and target
clear-before-search. Spatial conversion keeps low 32 bits of int64 truncation.
Pitch computes independent endpoints and random selection, with no clamp/sort;
completion time is an unsigned ratio rounded to binary32 before releases.
The callback view tail-calls UpdateVolPan; it is not a completion notification.

Evidence: queried main-checkout export first, then focused local pseudocode and
instruction bytes. Numeric constants were checked in original image data;
operation order/widths cite instructions in the spec. No new REA query was
needed. Historical round-1 REA `target_unavailable` and capstone fallback failure
are preserved as limitations for external SEH continuation 0x0093E0F9, not
silently considered resolved. No decompiled output or binary data was committed.

Validation PASS: exact 110-body/16012-byte inventory against original ownership
TSV; complete open-question partition 22/8049; `questions.py index` and `check`;
sections 1–11 and 26 unique prose conformance cases; `git diff --check`.
Independent calculations checked pitch endpoints/wrapped draws (100,104,99)
and unsigned completion ratio rounding to binary32 2.0. Added 14 discriminating
cases (13–26). These are specification examples and analysis checks, not engine,
game or shadow executions. Cargo/clippy/ctest n/a for docs-only work; game runs,
shadow comparisons, ABI exports and ABI proposals: none. Commits succeeded
despite nonfatal packed-refs.lock permission diagnostics; no ref repair attempted.

Questions: `docs/specs/questions/hit-track-player.md` contains HT1, HT2, HT3,
HT5; archive `questions/answered/hit-track-player.md` records HT2-owned,
HT3-owned and HT4 with dated DIFFERS rules. The generated index is current.

Precise residuals, in priority order:

1. HT1: **86 opcode values** remain, all in the 6754-byte DoCommand body.
   Decode from jump table 0x0093DC8B; verify arithmetic, comparisons, conditional
   branches, call/return helpers, sound/control callbacks, comparison waits and
   Hit-list selection. Only 09/0B/0C/0D/0F/1D/1E/5D/5E/60/61 (hex) are
   contracted. Do not infer graceful EOF or other malformed-stream behavior.
2. HT5: **21 bodies / 1295 bytes** require ownership, duplicate/key ordering,
   vector growth/overlap, removal/iterator and pool teardown contracts. The
   external shared exception continuation 0x0093E0F9 remains undecoded.
3. HT2 external: cleanup 0x0093E60A release/dependent/channel teardown order.
   HT3 external: publish all **80** words of key map 0x00B169F8; unsigned
   helper inputs >=80 return zero, but map contents are not fully specified.

This task ran one reserved continuation round. No self-restart or additional
round was launched. Guard manifest:
`C:/Users/tlori/FreeCity4/build-runs/run4-re-hittrack4-r2-manifest.json`.
