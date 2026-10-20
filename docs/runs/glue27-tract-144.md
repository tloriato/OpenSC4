tag: glue27-tract-144
run: 144
date: 2026-10-16
lane: glue27
dll: FreeCity4-glue27
pacing: turbo
verdict: clean
switches: glue27 `39b668a7` `enable = tract`, `perf_profile = 1`, `turbo = 1`, `end_date = 2032-01-01`, queue label `glue27-tract` (item 4: `enable = tract` alone to 2032)
result: **clean, ref_warnings=0, 0 problems, 50.98 days/s** (real 42.6 s, julian 2463237, date 2032-01-05). 72 perf months, 2163 game days. Whole-sim 31.1 ms/game-day in month 2032-01. No `perf month ... sections` line (our development glue is off), so the tract cost is read off the agents: the development agents cost 4.90 ms/game-day against run 142's 4.35 (the original BDS driving our tract). Item 4: `enable = tract` alone reaches 2032 clean.
summary: glue27 tract replace alone, clean, 50.98 days/s
