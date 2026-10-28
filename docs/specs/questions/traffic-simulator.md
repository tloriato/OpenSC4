# traffic-simulator: open questions
<!-- Open questions about docs/specs/traffic-simulator.md; answered entries move to answered/traffic-simulator.md. -->

## TC29-STATS-1: state-2 entry and per-switch statistics

Blocked case: `berlin-spandau` in census `sh28-census3`, 2001-09-23, state 15 -> 2,
working-statistics record first difference at serialized offset 332, 47/544 bytes.
Original begins `0000000000000000`, copy `120002722F3E5B00`; the difference recurs
on 2001-11-07 and 2001-12-07 when entering state 2 (11/544 bytes each).
Evidence: `build-runs/run7/census3/berlin-spandau.shadows.txt` in the main checkout.
Offsets 314..529 are the twelve `u16,f32,s32,s32,f32` switch records of §6.1 row 16.
§3.2/§4.9 specifies resetting the network rows and record counts, but does not name
these switch fields; save-records §12 Q2 requires preserving them until specified.
What are their meanings, entry resets, accumulation, and publication rules? Are
their values valid immediately after load, or reconstructed by a later phase?
Please include a worked state-2 entry with a nonzero loaded switch record.

## TC29-STATS-2: published statistics trailing floats between deliveries

Blocked case: `berlin-spandau`, 2001-10-06, state 16 -> 17, and `berlin-tegel`,
2029-09-04, state 18 -> 18: the *pre-delivery* record check finds 4/544 bytes
different in published statistics, first at offset 532; original context
`803F0000803F0000`, copy `C8420000C8420000`. Evidence: the corresponding census
`.shadows.txt` files under `build-runs/run7/census3/` in the main checkout.
These are the two unnamed floats at serialized offsets 530 and 534 (§6.1 row 16).
Which operations update them, with which units and inputs, and when? Can a public
getter mutate this published block? The copy currently preserves the saved values
as required by save-records §12 Q2; neither comparison nor tolerance is relaxed.

## TC29-FERRY-1: reset point of the saved ferry rider total

Blocked case: `paradis-reiss` in census `sh28-census3`, 2016-10-24 and
2016-11-19, state 13 -> 19: the original's ferry total is 93, the copy's 186.
Evidence: main `build-runs/run7/census3/paradis-reiss.shadows.txt`, lines 422
and 596; the other record fields, trips, holder properties and grids agree.
Traffic-simulator §3.2 row 19 and traffic-network §4.9.3 say to rotate rider
counters and add the new last riders to the city total. The current core
does so, retaining the previously saved total, as §6.1 row 19 specifies.
When is this total reset (or replaced), and what operations can change it
between ferry passes? Please include a worked second pass with a nonzero
loaded total, and its relation to monthly history publication (§4.12).
The addition and exact comparison remain unchanged pending that rule.


## TE33-MAPS-1: peak volume at cells without a reported network occupant

Fresh Arlert shadow `traffic-exact33-r11-arlert-shadow1` (DLL `f33a3897`,
clean/ref_warnings=0/stalls=0) differs before the monthly rand boundary.
On 2004-12-12, state 2 -> 11, peak-volume and congestion grids each differ
at ten cells. The first displayed cells are (112,146), (112,169), (120,146)
and (120,169): original 1, copy 0 in both grids. The prior delivery's working
switch statistics also differ at +314, as in TC29-STATS-1.

Diagnostic `traffic-exact33-r11-arlert-inputs1` (DLL `48c33ec8`, clean/ref0/
stall0) queried positions of the network occupants returned to the copy's
rectangle callback. None was reported at those four cells. The temporary
trace was removed. This does not establish which original operation supplies
the difference, nor prove that the original has no network-info record there.
Please clarify §4.9 step 5 for transit-switch cells: does a switch itself
force peak volume to at least 1, or does another operation create network
info there? Include the applicable switch-statistic and map-update rules.
No map-value substitution or comparison tolerance was added.
