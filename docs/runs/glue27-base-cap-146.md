tag: glue27-base-cap-146
run: 146
date: 2026-10-16
lane: glue27
dll: FreeCity4-glue27
pacing: turbo
verdict: clean
switches: glue27 `39b668a7` base (no `enable` key), `perf_profile = 1`, `sim_turbo_frame_ms = 5`, `turbo = 1`, `end_date = 2028-01-01`, queue label `glue27-base-cap` (the equal-pacing control for run 142: the turbo frame budget caps the CPU share, so days/s becomes the inverse of the cost per game day and DG31's pacing dependence is held fixed)
result: **clean, ref_warnings=0, 0 problems, 58.29 days/s** (real 12.1 s, julian 2461772, date 2028-01-01). 24 perf months, 703 game days. Whole-sim 16.69 ms/game-day. The development agents cost **3.13 ms/game-day**. Against run 147 (our development, same budget): days/s 58.29 vs 53.58 and ms/game-day 16.69 vs 18.48.
summary: glue27 base at equal pacing (frame budget 5 ms), clean, 58.29 days/s
