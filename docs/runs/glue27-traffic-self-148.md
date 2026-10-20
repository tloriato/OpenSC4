tag: glue27-traffic-self-148
run: 148
date: 2026-10-16
lane: glue27
dll: FreeCity4-glue27
pacing: turbo
verdict: clean
switches: glue27 (after the shadow-over-replacement change) `enable = traffic` **and** `traffic_shadow = 1` together - the combination that used to end with `traffic shadow: requested ... but INACTIVE` - `traffic_shadow_lines = 8`, `traffic_shadow_diffs = 8`, `perf_profile = 1`, `turbo = 1`, `end_date = 2032-01-01`, queue label `glue27-traffic-self`
result: **clean, ref_warnings=0, 0 problems, 5.85 days/s** (real 370.5 s; the shadow makes this the slow shape, as it does the regress shadows run). The self-check ran and is exact: `traffic shadow: ON over FreeCity4's replacement (traffic_shadow from FreeCity4.ini, \`enable = traffic\`)`, `comparing with the live traffic simulator 00F5CE10 (city 030D3014)`. **72 month lines, 72 clean, 0 with any mismatch, 0 INCOMPLETE, resyncs=0.** Deliveries 3077 compared / 3077 clean / 0 incomplete. Record fields 206159 compared, **0 mismatched**. Trip lists 2116617 compared, **0 mismatched**. Holder properties 2300036 compared, **0 mismatched**. Grid tracts 712091648 compared, **0 mismatched**. Every month ends `first differences: none`.
summary: glue27 traffic replacement + its shadow self-check, clean and exact to 2032
