tag: verify-regress4-normal-224
run: 224
date: 2026-10-21
lane: verify-regress4
dll: verify-regress4 at 28971497 (later changes docs only)
pacing: normal
verdict: clean
switches: queue vr4-normal1; enable=simulator, disable=budget; aura_shadow=1, netmgr_shadow=1; turbo=0; end_date=2032-01-01
result: Clean through 2032-01-01, ref_warnings=0, zero problems. aura 73/73 exact and netmgr 73/73 exact in this two-shadow normal-speed profile. It is not the combined house profile and does not prove the turbo house netmgr differences resolved. The full house run still has NM10/NM11 differences; no comparisons were suppressed.
