# Run apply15b-hw2-shadow (2026-10-16)

Lane `apply15b`, DLL `engine/build-x86/FreeCity4.dll` at `e63d0b8f` (sha256 `b652c228`), city **Konradshohe**,
`turbo = 1`, `end_date = 2032-01-01`, `seconds = 600` (timeout 750). Request
`build-runs/game-queue/apply15b-hw2-shadow.req`, results `build-runs/game-results/apply15b-hw2-shadow.{verdict,log}`,
slot 1, wall 46 s.

```
[ini]
history_shadow = 1
```

## Verdict

`verdict: clean`

```
AUTOTEST VERDICT: clean (foreign_exceptions=0, watchdog_stalls=0, rtc=0, overruns=0, ref_warnings=0,
  julian=2463236, date=2032-01-04, real_seconds=34.109, days_per_second=64.26, switches_off=-, disabled=-)
```

Problems (all 0): foreign first-chance exceptions, watchdog stalls, reference warnings, Rust panics, structured
exceptions, shadow INCOMPLETE lines, other WARNING lines.

## Shadow summary lines

```
history         79 lines, 79 with every mismatch count 0
```

The load and the last month and pass lines (the shadow compares the two records byte for byte and the queries of
slots 5-8 on six ranges and four dates per series at 24-bit and 53-bit precision, **bit for bit**):

```
history shadow 2026-01-27 load: the original's record 147442 bytes (143 series, 18215 points) read by the shadow
  core: ok, written back identical; mismatches=0
history shadow 2026-01-27 load queries: series=143 points=18215 record=147442 bytes identical; queries=5472
  (24/53-bit) differing=0; records=0 lengths=0 days=0 years=0 split_steps=0 step_results_differing=0 pass=0;
  mismatches=0; comparisons=1 clean=1
history shadow 2027-03-31 pass end: series=143 points=16565 record=134242 bytes identical; queries=5472 (24/53-bit)
  differing=0; records=636 lengths=0 days=23 years=0 split_steps=0 step_results_differing=0 pass=0;
  mismatches=0; comparisons=16 clean=16
history shadow 2032-01-01 month: series=143 points=18585 record=150402 bytes identical; queries=5472 (24/53-bit)
  differing=0; records=727 lengths=0 days=23 years=1 split_steps=0 step_results_differing=0 pass=2;
  mismatches=0; comparisons=78 clean=78
```

## What it verifies (HW2)

HW2: the graph window (`cSC4GraphsViewWinProc`) reads a series through **slot 8 `GetPoint` only** — one call per
sample date, never `GetCount` — so the three-point regression at an interior point is what every curve in the game
shows and must be reproduced in whatever answers `GetPoint`.

This run is the in-game check of that. FreeCity4's warehouse answers `GetPoint` (widened range + `fit_line`), and the
history shadow ran that against the **original** warehouse's answers on the live Konradshohe series: 5472 queries
per comparison (slots 5-8, six ranges and four dates per series, at 24-bit and 53-bit x87 precision), compared
**bit for bit**, on 79 lines across six years of turbo months and two yearly passes.

- `queries=5472 (24/53-bit) differing=0` on every line
- `mismatches=0; comparisons=78 clean=78` (79 lines with the load)
- the records also stayed byte identical, so the series the smoothing reads matched too

So the smoothed `GetPoint` answer — the value the graphs draw at a sample date that falls on an interior point — is
bit-identical to the original's at both precisions, over the real corpus. The unit test
`hw2_graph_window_draws_the_smoothed_value` and vector `hw12_07_get_point` pin the same behaviour offline (an
interior point is drawn at 10/3 where 10 is recorded, and 11 where 3 is recorded; the two end points are drawn as
recorded; `GetPoint` and `GetCount` disagree at the same date).
