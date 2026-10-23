# Watercraft distinguishing vectors (rounds 2 and 3)

Companion to [watercraft.md](watercraft.md) §§6.2.2a–6.2.5. These are **derived expectations, not executed
native conformance**. No portable implementation, game process or runtime original was used. Binary32 words
are written as hexadecimal values, independent of byte order. Arithmetic examples use finite, valid objects
and positive ordinary tuning; event/effect service results are explicit primitive inputs.

## Speed branches and rounding

Use p6=-1/4, p7=1/4, p30=-2, p31=2, p32=p33=0, p10=1/8, p0=1/2, dt=1 ms, with group/AEMS available.
Thus a=1/8 and b=1/16 exactly. Full forward command u=2 maps close to +1/4 and full reverse u=-2 close
to -1/4 under the native transcendental constants; their magnitudes are greater than 3/16. That strict
separation, not a claimed transcendental bit pattern, establishes the exact arithmetic branch results below.
Deactivate the unrequested throttle channel with -1; the reverse channel's positive 2 produces signed u=-2.

| Case | Initial Speed and request | Expected Speed | Distinguishing observation |
|---|---|---|---|
| Forward from zero | 0, forward 2 | 1/8 (`3E000000`) | Uses a. |
| Reverse from zero | 0, reverse 2 | -1/16 (`BD800000`) | Uses b because sign0(0)=+1. |
| Already reversing | -1/32, reverse 2 | -5/32 (`BE200000`) | Same-sign acceleration uses a, not b. |
| Cross zero braking | +1/32, reverse 2 | -1/32 (`BD000000`) | One subtraction of b crosses zero; reset reverse channel to p32=0. |
| Weaker same-sign request | +1/4, forward 1/2 | +1/4 (`3E800000`) | Preserve speed and replace forward channel by inverse feedback; no target-speed deceleration. |
| Zero command | -1/8, forward +0 or -0 | -1/8 (`BE000000`) | Gear may be forward, but signed-zero command skips speed and feedback. |
| Near zero opposite speed | +2^-20, reverse 2 | -65535/1048576 (`BD7FFF00`) | No speed epsilon or stop-at-zero branch. |

For inverse helper alone with p6=-1/4, p7=1/4, p30=-2, p31=2, ordinary libm evaluation of the **native
formula and binary32 caches** gives approximately +1.80980265 for v=+1/8, and **-1.14554119** for v=-1/8.
These decimal checks distinguish the signed reverse feedback and tan/atan cache asymmetry; they are not
transcendental bit-exact goldens. The zero input returns zero without changing pointed-to Speed.

An independent FST/FSTP witness uses dt bits `3F812345`, p10 bits `3B912347`, p0 bits `3DABC001`.
The exact rational triple product is `906088057409608316707 / 2417851639229258349412352`.
Rounding that retained product to binary32 yields b bits **39C479FE**; rounding dt*p10 to binary32 first,
reloading it and multiplying p0 yields **39C479FD**. The native FST at 0056E686 does not pop/reload:
the retained product determines b. This is a constant/arithmetic witness, not an executed x87 test.
The witness assumes x87 arithmetic precision of at least 53 bits; runtime control-word provenance is not
established by this offline round. Instruction operand/store widths alone do not prove the active control word.

## Ordered steering and neutral return

Set p1=1/2, p2=1, p5=4, p8=-2, p9=2, with b=1/16 and dt=1 as above. S,L,R are the fields in §6.2.4.
All values in the table are exact powers-of-two fractions, so every specified float spill is exact.

| Initial (S,L,R) | Final (S,L,R) before inherited completion | Reason |
|---|---|---|
| (1/4,-1,-1) | (3/16,-1,-1) | Neutral returns by b*p2; neither channel is active. |
| (-1/4,-1,-1) | (-3/16,-1,-1) | Negative neutral return, same magnitude. |
| (0,-1,-1) | (0,-1,-1) | Both steering phases idle. |
| (0,1/2,-1) | (1/4,1/2,-1) | Direct left maps elapsed milliseconds through p1. |
| (0,-1,1/2) | (-1/4,-1,1/2) | Direct right; left return does not run for negative S. |
| (0,1/2,1/2) | (0,1/2,0) | Right first makes -1/4, then left return consumes 1/4 and clears R. |
| (-1/4,1/2,-1) | (0,0,-1) | Right return boosts by p5, clears L, then left direct sees L=0. |
| (-1/2,-1,0) | (0,-1,0) | Right feedback: R=(-S/p1)-dt=0. |
| (1/2,0,-1) | (1,2,-1) | Left feedback: L=S/p1+dt=2; opposite sign of dt from right. |
| (0,8,-1) | (2,8,-1) after clamp | Direct S=4 clamps to p9=2; rudder=-1/4. |

Thus inactive (-1) and active zero are distinct. Equality at direct feedback comparisons performs no channel
adjustment. A requested left duration is not multiplied by dt again; inherited tick advances it first.

## Movement, water boundary, heading and side-effect order

- Turning helper with H=(0,0,1), P=(0,5,0), Speed=1/4, p7=1/4, Turn=p9=2, p11=8,
  p4=1/2, dt=0, identity scratch transform: radius=12, center=(12,5,0), radial=(-1,0,0), angle=0,
  published position=(0,5,0). Sea-level return is 5. A foreign spatial water level of 500 at that location
  is irrelevant: it is never queried. Exactly one sea-level query occurs because abs(newZ-centerZ)=0.
  dt=0 still performs matrix/translation publication and revision increments. There are no RNG draws.
- Orientation helper, reverse Speed=-1/8, H=(0,0,1), identity transform, scale=1, initial y=10,
  p17=1, p18=1/4: bank returns without writing, pitch clamps to 0, final y=10+1/8=10.125
  (`41220000`). A second call yields 10.25 (`41240000`). Speed=0 instead preserves the entire transform,
  even with Turn=2. No dt or RNG is consumed in either case.
- Invalid horizontal heading H=(0,1,0) reaches a zero-length cross-product normalization in turning/pitch;
  this is an unprotected arithmetic case, **not** a native-defined straight-ahead fallback. No normal-domain
  conformance value is proposed for its NaNs/infinities or floating-point exception state.
- Heading output with h=(0,0,1): A=0, first store K, second store 0. With h=(1,0,0), A=π/2 and the
  two stores choose the h.x>=0 side; with h=(-1,0,0), they choose the other side. Compare after both stores,
  not with an atan2 value that omits the intermediate binary32 rounding.
- Equal-magnitude tween: Speed changes +1/8→-1/8 while tween target=1/8. No GetCurrent call or four-word
  reset occurs. For duration=0 and unequal new magnitude, GetCurrent returns old target; reset start to that
  value, target to abs(new Speed), duration/elapsed to zero, then run inherited command completion.
- Fire channel active, button set, request false: no counter increment or button/timer clear. Request true
  with counter FFFFFFFF: counter becomes 0, timer word clears and button clears, before skier/steering.
- Skier initially stopped, toggle set, Speed>0, Start makes running=true: running query, GetTransform,
  SetTransform, Start(0), then a fresh running query and second GetTransform/SetTransform. Toggle remains set.
  For a stationary armed tug with stopped rope effect, SetTransform/Start can likewise be followed by a
  second SetTransform. Both use a full transform, not an invented trailing point.
- Inherited edge sample saved-current exactly 0 or exactly 250: false; 1: true. A clear latch with timer=9,
  limit=10, dt=2 stores timer=11 without latching until the **next** completion. At timer=limit it latches
  immediately and clears timer. Threshold=-1 takes this timer path; threshold/count are signed integers.

R2 unexecuted/residual checklist: full AI interpolation, inherited animation, exact spill rounding for turning/
bank/pitch, nonfinite/degenerate math-library behavior, crash RNG order, route decoding/backtracking and
dock/tie cases remain OPEN. None of these vectors confers whole-owner, byte coverage or engine qualification.

## AI motion (final round 3)

These independent **proposed, unexecuted** cases target only the AI call in
[watercraft-ai-motion.md](watercraft-ai-motion.md); stop before the caller's later Movement_PositionNew.
No engine result or native execution was used. Except where overridden, use ten valid 16-byte travel entries,
so no picker is called; Q0=(0,0), Q1=(32,0), Q2=(64,0) are primitive float coordinate-query replies.
For entries i=3..9 reply `(32*i,0)` on every query. Cell words may be `(i,0)`; the query mapping is explicit,
not an assumption about city cell size. Thus A=(16,0), B=(48,0). Sea level reply=5; P=(0,5,0), H=(1,0,0),
phase=0, Speed=Turn=0. Set p6=-1/4, p7=1/4, p8=-2, p9=2, p13=p14=1/1024, dt=400.
This time is a direct-method input (or needs caller p35>=400); default animation would clamp it to 100.
Pose dispatch preserves all inputs/tuning/lifetime. Old current history P=(8,5,0), H=(0,0,1) is explicit.
The first 19 coordinate calls are last-entry, then pairs (0,1),(1,2),...,(8,9); the last three are 0,1,2.
Float D=1 (`3F800000`), T=40 (`42200000`), and approach rate=25/64 (`3EC80000`).

| Case and changed incoming primitives | Expected owned stores/output/order |
|---|---|
| Next point, default inputs above | Normalize H through a-alignment, then query sea once. P becomes (1,5,0), words (3F800000,40A00000,00000000); final normalize H; pose(P,H); previous history becomes old current, then current becomes live P/H; Speed snaps to 1/4 (3E800000), Turn=0. No entry erased. |
| No next point: empty vector; owned picker returns false, initial Speed=1/8, Turn=-1, phase=1 | Picker's own changes are outside this case; following its false return AI writes Speed=Turn=+0, then returns. No coordinate/sea query, phase write, pose or history copy by AI. This conditional exit case does not close the owned picker. |
| Picker succeeds but leaves exactly two valid entries | AI zeros Speed/Turn, queries last then pair(0,1), and returns. No final three queries, sea, pose, approach or history copy. Picker behavior is again an explicit prerequisite. |
| Reached: P=(15,5,0), H=(2,0,0), phase=1, Speed=1/8, Turn=-1/4 | da=1, b aligned: H normalizes to (1,0,0); phase=0; shift entries 1..9 to 0..8 and decrement count to 9; P unchanged. No sea, pose or history copies; Speed=Turn=+0 after approach. Only one entry consumed. |
| Distance equality: P=(9,5,0), otherwise default | da=7 exactly (40E00000), so no initial b-alignment/consume; a-alignment true; translate to (10,5,0); Speed=1/4. Count=10, phase=0, sea/pose/history occur. |
| Just inside: P.x word 41100001 (9+2^-20), otherwise default | da=7-2^-20 (<7); b aligned, consume one entry and preserve P. No sea/pose/history. This distinguishes the equality branch without transcendental uncertainty. |
| Step reaches clamp: dt=12800, otherwise default | D rounds to 32 (42000000), L=t=16, P=(16,5,0); desired Speed=(16/32)*(1/4)=1/8 (3E000000), reached in approach. Count still 10 this call: there is no post-translation consume test. The next call sees da=0. |
| Zero horizontal distance: P=(16,5,0), phase=1, Speed=1/8, Turn=-1 | da=0: after all 22 coordinate calls, end=begin and dispatch D0(72,0,-1,0,0,0,0). Speed/Turn/phase/history unchanged by AI (dispatch side effects separate). No b normalization, sea or pose callback. |
| Zero time on reached case: P=(15,5,0), H=(1,0,0), dt=0, Speed=1/8, Turn=-1/4 | Consume one point and clear phase; approach rate=0, so Speed stays 1/8 and Turn stays -1/4. No 0/0 because it takes the reached exit. A direct zero-time translation/rotation instead reaches unprotected feedback division: no finite result asserted. |
| First turn: H=(0,0,-1), otherwise default | a misaligned; m=T=40, cross=-1, signed d=-40. P unchanged, phase=0, no sea query. Rotation H approximately (sin(40°),0,-cos(40°)); pose/history then approach. Desired Turn=+2 despite negative d, so stored Turn=25/64 (3EC80000); Speed=0. H transcendental bits are unknown. |
| Second turn: phase=1, P=(15,5,0), H=(0,0,-1) | da=1, initial b alignment false; skip a-alignment; set phase=1, rotate toward b. Same signed d=-40 and approximate H, but desired Turn=-2, hence Turn=-25/64 (BEC80000). No sea, entry erase or position change. Pose/history occur. |

The 2-degree boundary is a comparison-stage trace, **precision-unknown at concrete geometric input**:
the literal threshold word is `40000000`. Helper's retained post-acos degree result exactly 2 returns false;
any finite result strictly below 2 returns true. A proposed unit direction `(sin(2°),0,cos(2°))` against
H=(0,0,1) does not guarantee that exact comparison result after binary32 coordinates, normalization,
acos and multiplication. No invented exact-angle bit golden is supplied. Likewise da equal to float 0.1
passes its short-distance test, whereas zero takes the clear/dispatch exit; db compares its retained sqrt,
so a rounded db=0.1 alone does not decide that branch.

Callback order witness: if a valid pose dispatch changes live P.x to 2 before returning, current history
records 2, not the pre-call 1; desired Speed remains the already computed value. This is a conditional
mutation/lifetime-order trace, not a claim that the unresolved native setter actually makes that change.
No object-destruction tolerance is inferred. R2's rational FST witness 39C479FE/FD above remains unchanged.
