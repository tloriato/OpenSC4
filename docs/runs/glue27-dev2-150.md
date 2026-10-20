tag: glue27-dev2-150
run: 150
date: 2026-10-16
lane: glue27
dll: FreeCity4-glue27
pacing: turbo
verdict: clean
switches: glue27 (after the speed fixes) `enable = development`, `perf_profile = 1`, `turbo = 1`, `end_date = 2032-01-01`, queue label `glue27-dev2` (re-measure of run 143)
result: **clean, ref_warnings=0, 0 problems, 68.89 days/s** (real 31.7 s, date 2032-01-17). 72 perf months. Development agents **2.25 ms/game-day** (run 143's 3.67, run 142's original 4.35). Month 2032-01: `bds:call 51.5ms (2694)` (19.1 us/call, run 143's 21.9), `bds:day 35.5ms (31)`, `growth:call 28.7ms (6672)` (4.3 us/call, run 143's 4.9), `growth:state1 22.9ms (12)`, `growth:tract-UpdatePopulation 4.0ms (12288)` (the original tract), `grid:get 10812` against run 143's 35511 - the GridSnapshot batching in `population_step` replaced per-cell grid crossings with bulk reads. The machine was less loaded than for run 143 (68.89 against 43.92 days/s), so read the per-call figures, not the totals; run 152 is the equal-pacing pair.
summary: glue27 development replace after the speed fixes, clean, 68.89 days/s
