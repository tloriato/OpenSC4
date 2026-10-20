tag: glue27-tract-dev2-151
run: 151
date: 2026-10-16
lane: glue27
dll: FreeCity4-glue27
pacing: turbo
verdict: clean
switches: glue27 (after the speed fixes) `enable = development, tract`, `perf_profile = 1`, `turbo = 1`, `end_date = 2032-01-01`, queue label `glue27-tract-dev2` (re-measure of run 145)
result: **clean, ref_warnings=0, 0 problems, 14.21 days/s** (real 153 s; the machine was loaded - glue25 was running - so read the per-call figures, not days/s). Same 12288 `UpdatePopulation` calls as run 145: **`growth:tract-UpdatePopulation` 49.4 ms -> 34.0 ms** = 4.02 -> **2.77 us/call**, our UpdatePopulation is now **7.7x** the original's 0.36 us (was 11.3x); the fix bought 1.46x. `bds:tract-Grow` 34.0ms (93) -> 29.6ms (93) = 366 -> **318 us/call**, our Grow is now **0.94x** the original's 339 us (was 1.08x). `growth:state1` 72.6ms (12) -> 63.0ms (12); `growth:call` 75.9ms (1667) -> 65.2ms (1613); `bds:day` 38.2ms (31) -> 33.3ms (31). What is left of UpdatePopulation is structural: `LotsInCellRect` still builds the game's `GameList` and takes a reference per lot on every call (0.36 us a call is what the original does with none of that).
summary: glue27 tract + development after the speed fixes, clean; UpdatePopulation 11.3x -> 7.7x
