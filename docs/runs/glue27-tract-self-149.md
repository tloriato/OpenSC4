tag: glue27-tract-self-149
run: 149
date: 2026-10-16
lane: glue27
dll: FreeCity4-glue27
pacing: turbo
verdict: clean
switches: glue27 (after the shadow-over-replacement change) `enable = tract` **and** `tract_shadow = 1` together - the combination that used to end with `tract shadow: requested ... but INACTIVE` - `perf_profile = 1`, `turbo = 1`, `end_date = 2032-01-01`, queue label `glue27-tract-self`
result: **clean, ref_warnings=0, 0 problems, 75.73 days/s** (real 29.0 s). The self-check ran: `tract shadow: ON over FreeCity4's replacement (tract_shadow from FreeCity4.ini, \`enable = tract\`)` and `the city's tract developer is FreeCity4's (enable = tract): the live class is the reference, so this is its self-check`. **Styles and record: 73 compare lines, `roundtrip=same` on all 73, 0 mismatches, 0 lines with a mismatch** - the replacement's own record round-trips through a private core and its slots 8/9/11/13 agree with that core exactly. Population sample: 73 lines, 28032 tracts, 7325 lots, 155 mismatches on 40 lines (e.g. `tract 7,10 lot 1DAAEE8C dev 0x4200 orig 74 ours 68`). This is the sample's documented drift, not a self-check failure: the sample compares what the core would set *now* against what the lots hold from an earlier call, "with that day's desirability and capacities" (FC4TractShadow.h); the slot 7 forwarder that compares every call is only installed over the original's code.
summary: glue27 tract replacement + its shadow self-check, clean; styles and record exact
