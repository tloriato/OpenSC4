tag: glue27-dev-cap2-152
run: 152
date: 2026-10-16
lane: glue27
dll: FreeCity4-glue27
pacing: turbo
verdict: clean
switches: glue27 (after the speed fixes) `enable = development`, `perf_profile = 1`, `sim_turbo_frame_ms = 5`, `turbo = 1`, `end_date = 2028-01-01`, queue label `glue27-dev-cap2` (the after half of the equal-pacing pair with run 147)
result: **clean, ref_warnings=0, 0 problems, 61.21 days/s** (real 12.6 s, date 2028-01-01). 24 perf months, 702 game days. Whole sim **16.8 ms/game-day** (run 147's 24.3, run 146's original 25.4). Development agents **2.34 ms/game-day** (run 147's 2.56, run 146's original 3.13) - our development unit is **0.75x** the original's cost per game day. At the same frame budget the whole simulation now runs at **61.21 days/s against the original's 58.29** (run 146), where before the fix it was 53.58. Per outermost call: `bds:call` 23.4 -> 18.3 us (74.4ms/3182 -> 62.3ms/3396), `growth:call` 6.7 -> 5.5 us (45.3ms/6778 -> 28.2ms/5133), `bds:day` 1.39 -> 1.15 ms/game-day (43.2ms/31 -> 35.8ms/31).
summary: glue27 development at equal pacing after the speed fixes, clean, 61.21 days/s
