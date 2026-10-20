tag: glue27-base-142
run: 142
date: 2026-10-16
lane: glue27
dll: FreeCity4-glue27
pacing: turbo
verdict: clean
switches: glue27 `39b668a7` base (no `enable` key: every subsystem is the game's), `perf_profile = 1`, `turbo = 1`, `end_date = 2032-01-01`, queue label `glue27-base` (the development unit's cost against the original's, HANDOFF open item 1)
result: **clean, ref_warnings=0, 0 problems, 29.90 days/s** (real 72.6 s, julian 2463239, date 2032-01-07). 72 perf months, 2163 game days. Whole-sim 33.02 ms/game-day. The development agents (`cSC4BuildingDevelopmentSimulator` + `cSC4GrowthDeveloper`, every level) cost **4.35 ms/game-day**. Biggest agent `cSC4TrafficSimulator` (the game's) 127-129 ms/month. No `perf month ... sections` line: our development glue is not running.
summary: glue27 base (all original), clean, 29.90 days/s
