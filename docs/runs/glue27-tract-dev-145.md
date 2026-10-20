tag: glue27-tract-dev-145
run: 145
date: 2026-10-16
lane: glue27
dll: FreeCity4-glue27
pacing: turbo
verdict: clean
switches: glue27 `39b668a7` `enable = development, tract`, `perf_profile = 1`, `turbo = 1`, `end_date = 2032-01-01`, queue label `glue27-tract-dev` (item 4: `enable = tract` together with `development`; and our tract developer's cost against the original's with the same caller, run 143)
result: **clean, ref_warnings=0, 0 problems, 51.89 days/s** (real 41.8 s, julian 2463237, date 2032-01-05). 72 perf months, 2163 game days. Month 2032-01 against run 143 (identical switches but `enable = tract` off, same 12288 UpdatePopulation calls): **`bds:tract-Grow` 42.4ms (125) -> 34.0ms (93)** = 339 -> 366 us/call, our Grow is **1.08x** the original's (not 1.5-2.5x); **`growth:tract-UpdatePopulation` 4.4ms (12288) -> 49.4ms (12288)** = 0.36 -> 4.02 us/call, our UpdatePopulation is **11.3x** the original's. `growth:state1` 27.0ms (12) -> 72.6ms (12); `growth:call` 35.1ms -> 75.9ms; `bds:day` 47.4ms -> 38.2ms. Development agents 3.67 -> 4.44 ms/game-day. Item 4: together with `development` it reaches 2032 clean.
summary: glue27 tract + development replace, clean, 51.89 days/s
