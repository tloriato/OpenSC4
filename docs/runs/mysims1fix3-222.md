tag: mysims1fix3-222
run: 222
date: 2026-10-21
lane: mysims1fix3
dll: 11b6356c (09b3e722)
pacing: turbo
verdict: clean
switches: mysims_shadow = 1; Konradshohe; end_date = 2032-01-01; queue label mysims1fix3-sh-6
result: mysims 73/73 exact, load mismatches=0, last_mismatches=0; clean through 2032-01-07, ref_warnings=0, foreign_exceptions=0, watchdog_stalls=0, rtc=0. The comparison excludes last_commute_time, pylon, dispatch_tool, the unnamed pair/flag (MS1), per-advice nextDay/expiry/active, trip state and dispatch null-ness. Earlier sh-5 was clean but only 72/73; deferring the shadow PostCityInit until the original load dispatch finishes makes all 73 compared months exact. Full gate at 09b3e722: cargo 3576/0, clippy clean, ctest 62/62.
summary: MySims shadow exact 73/73 through 2032, including load, ref_warnings=0
