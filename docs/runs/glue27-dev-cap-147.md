tag: glue27-dev-cap-147
run: 147
date: 2026-10-16
lane: glue27
dll: FreeCity4-glue27
pacing: turbo
verdict: clean
switches: glue27 `39b668a7` `enable = development`, `perf_profile = 1`, `sim_turbo_frame_ms = 5`, `turbo = 1`, `end_date = 2028-01-01`, queue label `glue27-dev-cap` (the equal-pacing control for run 143)
result: **clean, ref_warnings=0, 0 problems, 53.58 days/s** (real 13.2 s, julian 2461772, date 2028-01-01). 24 perf months, 702 game days. Whole-sim 18.48 ms/game-day against run 146's 16.69 (ours is **1.11x** the whole simulation). The development agents cost **2.56 ms/game-day** against run 146's 3.13 (ours is **0.82x**). Month 2028-01: `bds:call 74.4ms (3182)`, `bds:publish 3.1ms (3182)`, `bds:day 43.2ms (31)`, `bds:tract-Grow 38.3ms (126)` (the original's Grow), `growth:call 45.3ms (6778)`, `growth:tract-UpdatePopulation 5.8ms (12288)`, `growth:state1 34.7ms (12)`, `growth:publish 2.7ms (6778)`.
summary: glue27 development at equal pacing (frame budget 5 ms), clean, 53.58 days/s
