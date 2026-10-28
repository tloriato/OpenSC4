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

## TE33-ACCIDENT-1: accident timing and draws under the fixed schedule

The traffic replacement now completes each trip phase under simulator §5.8's
available deadline. In `qcheck-3.log` of worktree `traffic-exact33`, every scenario's
first rand difference moves later. The smallest case, `paradis-arlert`, first differs
on 2005-02-01 delivery 49, immediately after the replacement's monthly traffic call
(48); its January monthly rand stamp agrees. No DIGEST differs; six save records differ.
The original-only control on that scenario is exact (`qcheck-4.log`). Replacement
evidence: main `build-runs/golden-runs/compare-20261028-015824-403ed8e8/result/`.

§9.1 TG2 permits leaving the accident clock, view, occupant and creation callbacks
unimplemented, saying this loses only visible crashes. Under the fixed schedule,
§4.13/§4.15's accident iterator also changes shared rand() and the traffic generator.
Please reconcile that omission permission with exact replacement, specifying the
accident timer's units, initial deadline, load reset, comparison and rescheduling
rule under simulator §5.8 (including zero entries and an unavailable view). §4.13
calls the timer milliseconds but also describes a seconds-based replacement.
Please include the monthly draw counts and the public view/automata interface
facts needed to supply the remaining callbacks. This is a suspected missing-draw
boundary, not a claim that all six save differences have the same cause.

Additional fixed-schedule evidence (traffic-exact33 round 10, `qcheck-32.log`):
Arlert Julian day 2453403, just before delivery 49, has original CRT state
`0xD4E5FED6`, copy `0x2D37894F`. By simulator §5.8's delivery-48 seed and
foundations §12.1, the copy state is the seed unchanged, and the original
state is exactly one `rand()` advance from it. On January 1 (2453372), both
states before delivery 49 are `0x05EF54D8`, with zero advances. The states
before delivery 48 also agree on both dates. The reference is the game.log
of golden key `22ee02cc159e865de055ed32a1bb83588538a76a5f69cb0423d08c6bf647b29d`;
replacement run is main `build-runs/golden-runs/compare-20261028-090319-ba319f0e/result/`.
This narrows the missing-draw boundary but does not prove which timer or
iterator took it. Please specify the accident timer's first-month behavior
and counter-read sequence as well as its deadline: omitted reads can also
change simulator §5.8's shared virtual clock. No compensating draw was added.

Round 11 reproduces the same Arlert boundary (`qcheck-36.log`, DLL
`f33a3897`, run `compare-20261028-095746-c95e41e5`). Its schedule month lines
report 22 frames at January 1 and 54 at February 1, with no sleeps: frame
contributions of 726 and 1,782 ms, plus the virtual counter's per-read ticks.
The copy has 16 accident-check entries in January and 11 in February.
This does not measure the original's list or establish its timer deadline.
Please include the initial next-check value and whether the period's
numeric value 60 is compared in ms or seconds; §4.13 currently names both.

Round 12 extends the boundary measurement to all eight scenarios, on the
same final DLL (`b5cba0a0`, `qcheck-39.log`, 0/8 exact, all clean/ref0/stall0).
Using simulator §5.8's preceding-delivery seed and foundations §12.1's CRT
recurrence, the first mismatching pre-delivery stamp gives these counts:

| Scenario | Julian day / next delivery | Reference state / draws | Replacement state / draws |
|---|---|---|---|
| gen28-tegel-s1-churn | 2462381 / 49 | `241661D8` / 1 | `67A1CAF9` / 0 |
| maxisland-bendburger | 2468620 / 50 | `9A98A186` / 1 | `F3BB3DBF` / 0 |
| maxisland-bevopolis | 2545564 / 50 | `7F8EC636` / 1 | `ED8B1C2F` / 0 |
| maxisland-caramel-island | 2465364 / 49 | `80FE0573` / 1 | `FF3CC870` / 0 |
| maxisland-san-indersty | 2508736 / 49 | `7A8CEFD7` / 1 | `5E79BFA4` / 0 |
| paradis-arlert | 2453403 / 49 | `D4E5FED6` / 1 | `2D37894F` / 0 |

All six immediately preceding deliveries target the traffic message
interface (`0x00AB33BC`), with equal pre-call rand stamps. Thus the one-draw
boundary is shared, rather than specific to Arlert's ten map cells. It still
does not identify which operation took the draw or establish the timer rule.
The other two scenarios first differ after a different receiver: Braus
2481982/21 gives 9 reference versus 24 replacement draws; Jinae 2468848/21
gives 11 versus 17. Their route-order evidence remains in TE33-PATH-1.
Evidence is the `game.log` files of the eight result directories in
`qcheck-39.log`; `build-runs/r12-boundaries.py` reproduces these counts from
those logs and the reference key in each result's `comparison.json` only.

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
