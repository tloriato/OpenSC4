# Power implementation questions

## PW-MAPS30-01 — AgePowerPlant arithmetic in the West Weburg census

Open; maps-cities30; 2026-10-26. Value-only conformance-vector request for
§7.4; the affected implementation item is stopped pending these values.

Evidence: `build-runs/suite/sh28-census3/maxisland-west-weburg.log:4597`,
2197-05-04, state 1: property 0xC910B4CF is original/ours
0.32092583179473877/0.3209258019924164 and
0.04157695174217224/0.04157694801688194 for two plants. The cycle's grids,
production, consumption, requirement, powered/unpowered counts and other
compared properties agree. The same two value pairs recur in
`build-runs/suite/maps-cities30-4/maxisland-west-weburg.log:6301`, 2197-05-05.

For each of those two plants, return the same-call numeric tuple:
`(x87_control_word_u16, wear_before_bits, rate_bits, capacity, efficiency_bits, used, effCap,
usage_bits, usage_curve_bits, department_funding_bits, funding_curve_bits,
curve_product_bits, age_increment_bits, wear_after_bits)`.
Here `curve_product` is the product of the two curve results and
`age_increment` includes the rate. Use uint32 hex for all f32 bit patterns
and uint16 hex for the entry control word. Use signed decimal for capacity,
used and effCap; identify each tuple by
its observed wear_after value above. No addresses or implementation are
requested. No tolerance or comparison change was made.

## PW-MAPS30-02 — Indereast shortfall allocation numeric witness

Open; maps-cities30; 2026-10-26. Value-only request for §§6.3–6.4;
the affected implementation item is stopped pending a numeric vector.

Evidence: `build-runs/suite/maps-cities30-5/maxisland-indereast.log:6681`,
2086-08-05, state 1: one order-dependent flood, no iterator start reproduced
the original's powered-next mask (460 cells not exact; next powered count
original/ours 331/321). The published grid and counts agree. One wear value
also differs: original/ours 0.16816721856594086/0.16816723346710205; this
witness does not establish that its cause is the same as PW-MAPS30-01.

For that original flood, return the numeric input tuple
`(seed_x, seed_z, rowMin, rowMax, supply, imp, total, factor_f32_bits,
avail_after_exports, rand_return, iterator_start, iterator_step, block_count)`
and the ordered block rows `(bx, bz, need, n, avail_before, powered,
avail_after)`, with powered as 0 or 1. Include the entry mark and exit
powered-next bitmaps as 64 rows of 64 bits. For the plant identified by the
wear value above, also return the PW-MAPS30-01 ageing tuple. Use uint32 hex
for f32 bits and decimal integers otherwise; values only, no implementation.

## PW-SE36-1 - block iterator draw count under the fixed schedule

Open; services-exact36; 2026-10-28. Section 6.3 step 6 explicitly requires
prime-step mode and exactly one CRT rand() call per network. Please confirm
the iterator mode, construction/reset draw count, and ordered block outputs
with a value-only example. The replacement follows that one-draw contract.

Power-only qcheck-2 on paradis-berner first differs at digest 2466069.5
(2039-10-07 #5, upcoming target Plumbing). This line precedes that target's
delivery and follows Power's state 1 -> 2 delivery. The fixed seed for the
power delivery (day 2466069, n=4, S=5EED1234) is 3DE8776D. Reference state
7DD0D232 is reached after 959 CRT advances; replacement state 6C6D2C7C after
one. The next delivery reseeds and matches again. None qcheck-3 is exact.
Thus the named plumbing target does not establish a differing plumbing query.

Evidence: main build-runs/golden-runs/compare-20261028-084940-f1e063ba/result,
reference key 0402bb675e00cb340144ce72453e60792ed99741cf567fac2dda425948eaa289.
The clean/ref0 services36-power-shadow-2 run reports 3 complete cycles with
mismatches=0/not_exact=0, including the preceding first cycle (one flood).
The shadow compares the resulting grid/properties, not CRT consumption.
No behavior or schedule edit was made to force the observed draw count.
