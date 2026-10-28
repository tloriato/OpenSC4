# Draw checks, 2026-10-28

`check.py record`, `run`, `run --replace each`, and `credit` now include the
paused camera sweeps in `engine/tests/scenarios/draw.tsv`. Draw references share
the save reference store and include the pinned DLL, capture controls, stream,
and entry reach. A difference names its frame, call, method, and argument or
buffer. Missing references and varying originals cannot establish draw credit.
Required replacement owners participate in draw reach selection too.

## References and determinism

Store: `C:/Users/tlori/FreeCity4/build-runs/golden`. Reference pin r5:
`d79069d9f36e620be6bd946022176269a44641c13e1760e006959f886774fee6`,
schedule fingerprint `bde008a451b1c7fb`. Each row captures zooms 1–5 at rotations
0 and 1, with explicit pause, a visible window, and the UI window tree.

| Scenario | Reference key prefix | Frames | Ordinary repeat | Holes | Reverse |
|---|---|---:|---|---|---|
| draw-konradshohe | 6fc90bac1fa1 | 10 | exact | exact | exact |
| draw-konradshohe-frames | f5104721576e | 40 | exact | exact | exact |
| draw-kensington | b5970d3e05fe | 10 | exact | exact | exact |
| draw-kensington-frames | c2b4befea354 | 40 | exact | exact | exact |
| draw-tegel | 8c38cb3939fe | 10 | exact | exact | exact |
| draw-tegel-frames | 9534598f3b12 | 40 | exact | exact | exact |
| draw-ui-bevopolis | 5fd2c9ffb904 | 40 | exact | exact | exact |

All seven references repeat exactly in all three modes: 190 frames and
8,960,135 driver calls per complete corpus capture. No draw difference was
observed. Each mode is an independent game run, not a copy of the baseline.

The retained evidence is in this worktree's `build-runs/draw-measure-r3/`
and `build-runs/draw-measure-r3-fixed/`, with the final seven-row summary and
native game labels in `draw-measure-r3/final.json`, and each reference's immutable
`baseline.json` and `repeat-*.json` files in the shared store. Audits retain
`GOLDEN_INI_EXTRA` and compare against the same unperturbed reference.

## Per-key draw verdicts

The supplied save-only `owner-check.tsv` had 108 rows: 104 errors, three
differences, and one exact result (`ports` on `paradis-ragako`). No key was exact
on its whole save cover. `ports` was therefore the only key with any exact save
to measure; its recorded original reach enters all seven draw rows.

The save-table DLL was `ref-r4.dll`, SHA-256
`cd3be21923a759e97f73c24a8dfea7f28fedd39950a198b2182144419ef5a76e`.
The local draw candidate uses the current r5 schedule. Candidate DLL SHA-256:
`b42f4a2269fe7ea55e9a6a612b8fb5755936461faa2e4279e954e74086869cd3`.
| Key | Draw rows reached | Exact draw rows | First difference |
|---|---:|---:|---|
| ports | 7 | 7 | none |

The six retained streams and the corrected Konradshohe stream all compare
exactly. These observations do not establish a whole-save-cover claim.
The local candidate exported no entry counters. Complete candidates can be
compared, as save candidates can; `coverage.py` still withholds byte coverage
credit when an exact candidate lacks reach. References continue to require it.

## Excluded attempts and corrections

The original 60-second Bevopolis capture stopped before all ten cameras.
The ordinary 60-second Konradshohe forty-frame repeat stopped at camera 6
and ended inside JSON record 1,963,896. Their logs identify the autotest
watchdog as the cause. Both long rows now use 170 seconds, below three minutes,
with new input keys and independent recordings. These invalid attempts are
retained separately and establish neither equality nor a draw difference.

The first seven-row candidate collector rejected complete streams for missing
candidate reach. After correcting that requirement, six retained streams were
compared with `golden.compare_cached`/`drawcheck`; the corrected sweep is separate.
The original failed collector table remains `ports-before-correction.tsv`.
An early direct comparator probe named a missing labelled copy and returned
`INVALID`; no equality was counted. Surviving queued originals were collected
from their direct exports without launching duplicates, after checking the
pinned DLL, unchanged input timestamps, and complete streams.

## Validation and reproduction

- `python -m pytest tools/tests -q`: **417 passed in 244.29s**.
- `qbuild.py --wt <WT> all`, once: **8,410 Rust tests passed; clippy zero
  diagnostics; build PASS 3/3; CTest 147/147; qbuild PASS**.
- `python build-runs/draw-round3.py`: surviving originals collected and
  independent ordinary, holes, and reverse audits run through `check.py record`.
- `python build-runs/draw-konrad-fixed.py`: corrected reference and independent
  audits/candidate capture under the 170-second key.
- `check.py run --dll <WT>/engine/build-x86/FreeCity4.dll --replace each
  --keys ports --scenarios draw --store <store> --jobs 7`: seven captures;
  initial missing-counter errors preserved, followed by comparison of retained
  complete streams after the counter-policy fix.
- `python build-runs/compare-retained-ports.py`: **6 retained candidates, 6 exact**;
  repeated with the corrected sweep's collector table: **1 candidate, 1 exact**.
- `python build-runs/finalize-draw-results.py`: **7/7 reference qualifications,
  ports 7/7 draw EXACT, first difference none; candidate reach 0/7, credit withheld**.
  The final `draw-owner-check.tsv` joins the six retained comparisons and the
  corrected sweep; the save-only owner table remains unchanged. The original
  collector was stopped after its audits finished, suppressing its superseded
  serial replacement measurement.

## Schedule request

None: all seven original streams repeat identically. The incomplete
attempts required longer scenario watchdogs. No `FC4Schedule.*` file changed.
