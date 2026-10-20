tag: glue27-dev-143
run: 143
date: 2026-10-16
lane: glue27
dll: FreeCity4-glue27
pacing: turbo
verdict: clean
switches: glue27 `39b668a7` `enable = development`, `perf_profile = 1`, `turbo = 1`, `end_date = 2032-01-01`, queue label `glue27-dev` (our development unit against the original's at the same settings; only `enable = development` differs from run 142)
result: **clean, ref_warnings=0, 0 problems, 43.92 days/s** (real 49.4 s, julian 2463235, date 2032-01-03). 72 perf months, 2163 game days. Whole-sim 22.82 ms/game-day. The development agents cost **3.67 ms/game-day** (the original's 4.35 in run 142: ours is 0.84x). Per month 2032-01: `bds:call 83.0ms (3783)` of which `bds:publish 3.4ms` (0.9 us per outermost call - the devperf batching holds), `bds:day 47.4ms (31)`, `bds:other-msg 27.9ms (770)`, `growth:call 35.1ms (7103)`, `growth:tract-UpdatePopulation 4.4ms (12288)` (the ORIGINAL tract: 0.36 us/call), `growth:state1 27.0ms (12)`, `grid:get 35511 grid:set 12389 grid:bulk 600`, `bds:peek 63716`, `growth:peek 128844`. Against run 142's agents for the same month: BDS 94.9 ms -> 52.1 ms (0.55x), growth developers 4.5 ms -> 29.4 ms (6.5x), total 99.4 -> 81.5 ms.
summary: glue27 development replace, clean, 43.92 days/s
