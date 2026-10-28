# pathfinder: open questions

## TE33-PATH-1: equal-score Walk-to-Bus return-route corner

The first Jinae traffic-shadow difference under the fixed schedule is on
2047-04-06, state 17 -> 18: lot (206,150), trip 0, return route byte +8.
Original and copy agree on the displayed trip fields and route lengths (54/62
bytes), but from (244,59,t0) the original visits (243,59,t2), whereas the copy
visits (244,60,t2). Both then visit (243,60,t2), (243,61,t2). Type 2 is Bus.
Seven edge-density bytes also differ by the 12 trips at this delivery.

Evidence: worktree traffic-exact33, game run
`traffic-exact33-r9-jinae-corner1` (clean, ref_warnings=0), game log line 3710;
the temporary core trace is `build-runs/r9-jinae-corner.txt`. The trace only
observes our implementation and has been removed from the source.

The return call has start rect (244,61)-(245,63), end rect (206,150)-(207,150),
allowed mask 0x1CF, starting mask 1, starting costs [0,0.4,0,0,0,0,0,0,0],
weight 0.09. Expanding (244,59,t0,side3), key 0x0300F43B, at g=f=0,
adds West (243,59,t2,side2), key 0x0E02F33B, before South
(244,60,t2,side1), key 0x0D02F43C. Both step costs, g and f are exactly
0x3CAAF6A7 (0.020869566). Our heap pops South before West; the continuation
through (243,60,t2,side2) wins the equal-g update at (243,61,t2,side1).

The implementation follows sections 4.6-4.9's ring, direction, heap and
equal-g rules. Please give a conformance vector for this corner, including
the start-node insertion order and the first differing pop/update, to
identify any missing ordering or rounding rule. This is an unresolved
path difference, not evidence that the heap rule alone is incorrect.

Additional route case (traffic-exact33 round 10): in
`traffic-exact33-r10-braus-shadow1`, the first differing delivery is
2083-04-08, state 18 -> 2. Lot (148,144), trip 1, agrees on count 4,
type 2, strategy 1, flags 8, status 3, destination (170,194), length
2.6385972, mask 0xB, and route lengths 41/39 bytes. Outbound byte +17
differs: context +15 is original `4005D0004305FC0303A2`, copy
`400504034305FC0303A2`. Decoded cell 39, after (165,151,t0), is
original (164,151,t0), (164,150,t0), (164,151,t0), versus copy
(165,150,t0), (164,150,t0), (163,150,t0). Other trips differ in the
same delivery, so this is not necessarily the earliest differing search.
The run reaches May 8 with a clean verdict, ref_warnings=0, and one
recovered watchdog stall in our FindPath. No step-cost trace was run;
neither zero-cost moves nor a heap defect are established by this case.
The corresponding replacement `qcheck-32.log` first differs at May 2
delivery 21 (lot developer). The prior matching original-only Braus
control (`qcheck-31.log`) is exact. Please include this route case in
the requested input-and-pop/update vector, or identify an earlier
query/input difference. No route-order rule was changed.
