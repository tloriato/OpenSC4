# Watercraft AI motion: bounded instruction contract

Companion to [watercraft.md](watercraft.md) §5.1, WC4. Original final research round 3/3,
2026-10-23. This publishes the previously open owned body `00571880–0057235A` (2,779 body bytes).
It does not close the whole owner, the route producer, docking, inherited animation, or exceptional math.
Evidence is the matching Windows export/original SHA256
`3bc5c7fe807fe5aa24a3abb06e40f13486670bc1150612c09baf4ecedafe1a4b`.
No native execution or portable implementation supplied any result below.

## Domain, inputs and invocation

The method consumes one binary32 elapsed time, **by value**, despite the local decompiler's missing argument.
Normal supported inputs are live valid objects, an ordered valid travel-point vector, stable nonmutating
coordinate/sea-level query services, ordinary finite coordinates, nonzero finite heading norms, positive
elapsed time and positive ordered tuning bounds. All intermediates must stay finite and away from
unproved math-library exceptional domains. The explicit route-empty and short-distance exits below are
supported without requiring the later nonzero norms. Zero/reverse time rules are stated separately;
they are not replaced by a generic safe fallback.

Use P=`owner+40,44,48`, H=`+58,5C,60`, Speed=`+2DC`, Turn=`+2E0`, phase byte=`+2B1`.
Travel points are 16-byte entries at vector `+2A4,+2A8,+2AC`; cell x,z are the first two signed
32-bit words. The count is pointer difference arithmetic-shifted right four, then tested with unsigned
branches (valid vectors only). The phase byte is a nonzero/zero latch, not a new named watercraft state.
Tuning p6,p7,p8,p9,p13,p14 live at `+35C,+360,+364,+368,+378,+37C`.

The animation caller clamps its B time to `[0,p35]`, spills binary32, calls AI, then calls
Movement_PositionNew with a pointer to that same spilled time, collision detection, dock requests and
clear requests (`00573180–005731E6`). The reached/no-next exits do **not** prevent that caller from
continuing. Docking's reached AI branch likewise clamps to `[0,p35]`, calls AI, Movement_PositionNew,
then collision detection and returns true (`005728BB–00572929`); no broader docking contract is inferred.
AI does not itself call turning `00569940` or orientation `00569E70`: the following Movement_PositionNew
selects those helpers using the **new Speed/Turn**, as described in watercraft §6.2.2.
No Automaton accumulator is substituted for this direct native call sequence.

## Waypoint preparation and exits

1. Initialize local desired Speed and Turn to positive zero. If count<10, call owned
   Engine_AIPickLocation `00570620`, then unconditionally write positive zero to Speed and Turn,
   even when the picker returns false. False returns immediately. The picker's route/state algorithms
   retain §5.2's limitations; this is an explicit owned dependency, not a foreign motion-result callback.
2. Re-read the vector. If count<1, return. Query the last entry's cell coordinates via the service at
   `00B43CEC`, slot byte offset `184`, passing cell x,z and two float output addresses. Its first output
   is world x, second world z (argument/output placement `005718F4–00571927`). Form and spill binary32
   `abs(last.z-P.z)+abs(last.x-P.x)` as a local minimum.
3. For each adjacent pair i,i+1 in ascending order, query i and then i+1. Compute **half differences**
   `(Xi-Xi+1)*0.5`, `(Zi+1-Zi)*0.5`, not world midpoints (the two differences have opposite order).
   Subtract P and take the sum of absolute values.
   Replace the local minimum only for a strict decrease. This scan changes no waypoint/index/pose field;
   its minimum is overwritten before any later use. The query calls still occur. The x half-difference
   is popped to binary32 before subtraction; the z half-difference is stored with FST and remains in
   the x87 register for subtraction (`0057199B–005719E4`). The loop reloads begin/end each iteration.
4. Re-read count. Count<=2 returns, **after** the scan. With stable count n>0 the total preliminary
   coordinate query count is `1+2(n-1)`; n=1 makes only the last-entry query. None of these short-vector
   exits normalize H, publish pose or approach Speed/Turn (except the picker reset in step 1).
5. For count>=3, query entries 0,1,2 in that order. Their float world positions are Q0,Q1,Q2.
   Construct A=`Q0+(Q1-Q0)*0.5` and B=`Q1+(Q2-Q1)*0.5`, each coordinate spilled binary32.
   Construct and spill horizontal vectors a=`(A.x-P.x,0,A.z-P.z)`, b=`(B.x-P.x,0,B.z-P.z)`.
   Compute `da=sqrt(ax²+az²)` and spill binary32. Compare that reloaded da with float
   `0.10000000149011612` (word `3DCCCCCD`). If da is **strictly less**, take the short-distance exit.
   Otherwise compute db similarly but compare its retained x87 square-root result with the same float;
   db is not first spilled. Strictly less takes the same exit. Equality passes each test.
6. Short-distance exit: set vector end=begin without freeing storage, then dispatch slot byte offset D0
   with `(72,0,-1.0f,0,0,0,0)`. Return immediately; no Speed/Turn approach, phase reset, pose callback,
   or history shift. At the db exit the retained norm is popped first. This is a state-stack dispatch,
   not an invented final-motion service (`0057231A–0057234B`). The automatic name of `007257C0` is
   misleading: its instructions copy 16-byte entries over a range; here the empty source range returns
   begin, which becomes end. No callback computes the clear for the owner.

Evidence: preparation `00571888–00571A20`; midpoint construction and rejection `00571A20–00571B53`.

## Alignment helper and owned branch selection

Normalize b with the reciprocal of its **retained** db, spilling x,y,z in that order. When da<7.0f
(`40E00000`), test alignment of b and current H using owned helper `00567ED0`.
This helper is not a pure predicate:

- Normalize its by-value float candidate with a retained reciprocal norm. Normalize H in place x,y,z;
  H's norm is first spilled binary32 and reloaded for reciprocal.
- Compute normalized candidate dot stored normalized H, sum in the order z product plus y product,
  then x product. Clamp to [-1,1], spill binary32, call the shared acos path `009F0090`, multiply
  by float degrees-per-radian `57.295780181884766` (word `42652EE1`).
- Return true iff the resulting retained angle is **strictly <2.0f**; equality returns false. The candidate
  normalization has no zero guard; H normalization has no zero guard. No RNG or pose callback occurs.
  Instructions `00567ED0–00567FC0`; this acos entry's TransformDirection label is not its semantics.

If da<7 and b is aligned: clear phase byte, erase **one first entry** (shift remaining entries down
in forward 16-byte copies; decrement end by 16), leave P unchanged, and go directly to the Speed/Turn
approach with desired values zero. Skip pose callback/history publication. H may already have changed
through the alignment helper. Erase helpers `0056BD50–0056BD7E`, `007257C0–0072580B`.
There is no repeated consume loop or leftover-time motion this call.

Otherwise, normalize a using `1/reloaded_da`, spilling x,y,z; normalize H in place with a retained
norm/reciprocal (no norm spill). Before this, an explicit exponent/mantissa bit test replaces **only
H.y NaN** with positive zero (`00571BDD–00571C04`); it does not establish general NaN safety.

| Ordered condition | Owned action before common pose publication | Desired Speed | Desired Turn |
|---|---|---|---|
| phase=0 and a-alignment helper returns false | Rotate H toward a using first rotation path below; P unchanged | 0 | First-path turn feedback |
| Otherwise da<7 and b-alignment helper returns false | Set phase=1; rotate H toward b using second rotation path | 0 | Second-path turn feedback |
| Otherwise da<7 and b aligned | No displacement/rotation here; earlier near/aligned exit normally consumed the point under stable finite inputs | 0 | 0 |
| Otherwise da>=7 | Translate P toward A at queried sea level; phase unchanged | Translation feedback | 0 |

The a-alignment check is short-circuited when phase is nonzero. It is called even if the earlier b check
already normalized H. da=7 selects translation once the a check passes or phase was nonzero; da<7 alone
does not guarantee consuming a point. Evidence `00571B77–00571C9F`, `00571E4A–00571F7F`.

## Turn and translation arithmetic

Both rotations form target-dot-H in z,y,x order, call acos **without the helper's [-1,1] clamp**,
and multiply by float degrees-per-radian. Let T be binary32 `(0.10000000149011612*dt)`.
Select angle magnitude m=0 if the retained angle<0; otherwise select min(angle,T), with equality
selecting the angle rather than T. Spill m binary32. Form cross scalar
`target.x*H.z - target.z*H.x`, spill binary32, and apply finite sign0 (negative gives -1; zero gives +1).
The signed angle d=sign0(cross)*m rotates H in the x/z plane:
`Hx'=c*Hx+s*Hz`, `Hy'=Hy` through explicit zero terms, `Hz'=c*Hz-s*Hx`.
Here c is FCOS of d times float radians-per-degree `0.01745329238474369` (`3C8EFA35`),
spilled binary32, and s is FSIN copied through binary32 coefficient slots. These are derived geometric
equations for ordinary finite values; they do not collapse every retained zero/product or reload.
The z result is spilled/copied first, then x, then y. All outputs use old H products, not the already
stored z. Common normalization follows. Exact transcendental bits and x87 control word are unknown.

The two paths intentionally differ (`00571D27–00571E45`, `00571FF6–00572100`):

- First path spills d using FST, retains its x87 product for radians/trig, and later computes
  desired Turn=`((reloaded_d*p9)/T)*sign0(reloaded_d)`. Thus either turn direction ordinarily requests
  **positive** Turn (zero also yields zero for positive T). Do not repair this to signed feedback.
- Second path retains d across rotation and computes desired Turn=`(retained_d*p9)/T` before its final
  binary32 spill. Its feedback remains signed. It sets phase=1 before the angle computation.

Translation uses float movement constant `0.0024999999441206455` (`3B23D70A`), not p4 or Speed.
Spill D=`constant*dt`. Query sea level at service `00B43CF4`, byte slot 74, with no coordinate arguments.
Form dx=`A.x-P.x`, dy=`sea-P.y`, dz=`A.z-P.z`; dx,dy pop/spill binary32, dz uses FST and its retained
value times reloaded dz in the first square. Sum z square plus y square plus x square; retain sqrt L
and also store L binary32 with FST. Choose travel t=0 if retained L<0, otherwise min(retained L,D).
Divide 1 by **reloaded** L. Retain dx times that reciprocal, but spill/reload normalized dy and dz.
Multiply components by retained t; z displacement is spilled before adding. Store P.x, then P.y,
then P.z, each binary32. Desired Speed=`(retained_t*p7)/D`, popped to binary32
(`00571E5F–00571F36`). Thus direct translation can change P while Speed was zero; it is followed
by the separate movement helper at the caller. There is no terrain, navigability or RNG query here.

## Pose publication, history, and approach

After any non-exit movement branch, normalize H in place x,y,z with retained norm/reciprocal, then
dispatch slot 16 (byte offset 40) with **addresses of live P and H**, in that argument order.
No callback returns an interpolated motion/final state: owned P/H have already been written.
After callback return copy old current position `+2F4..2FC` to previous `+2E8..2F0` in x,y,z order;
copy then-current live P to current position; copy old current heading `+30C..314` to previous
`+300..308`; copy then-current live H to current heading, all component copies ordered x,y,z.
The snapshots therefore observe callback mutations; there is no pre-callback P/H snapshot or null/
lifetime revalidation. The exact dispatched setter/visual side effects remain a shared dependency,
not permission to delegate owned motion. Callbacks must preserve object lifetime for this normal path
(`00572104–005721C4`).

Reached-point paths join immediately at approach and skip this callback/history sequence. Desired values
remain those computed before callback; tuning and actual Speed/Turn are loaded after callback. First Speed,
then Turn are approached: retain rate=`dt*p13` or `dt*p14`; if abs(desired-current)<=rate, copy desired.
Otherwise subtract rate when desired<=current, add rate when desired>current. Spill current, then clamp
it to [p6,p7] or [p8,p9], respectively. Equality snaps rather than doing an incremental arithmetic update.
Rate is not first spilled; subtraction and magnitude comparison are retained x87 computations
(`005721C4–00572317`). These are parameter rates, not the AI geometric movement constants.

The method writes no time accumulator. The caller owns elapsed-time accumulation and per-tick clamp.
For direct dt=0, a valid translation branch has D=t=0 and unchanged P but still computes desired Speed
as 0/0; a turn branch can similarly divide 0/0 for feedback. No general zero-time early return exists.
Those exceptional feedback outputs are OPEN. A reached/short/no-next exit avoids these divisions and
has the ordinary stated effects even at zero time. Direct negative dt is not clamped in AI: D and T
become negative, so the positive-domain min and approach logic can select negative steps/rates.
Native callers above supply nonnegative time; reverse-time exceptional/full numeric results remain partial.

## Evidence status and distinguishing traces

Supported: entry/count/reset/scan/short exits, two midpoint construction, mutable alignment predicate,
owned branch order, ordinary turn/translation equations, final pose/history/approach order and exact listed
float constants/operand widths. Partial: full rotation coefficient spill equivalence, acos/FSIN/FCOS bits,
active x87 control word, callback target effects/lifetime, direct zero/reverse-time exceptional feedback.
Unknown: route-producer residuals, nonfinite/overflow/underflow/degenerate math/library faults and any
inherited animation not already specified. No supported claim is made for an arbitrary callback replacing
this owner algorithm. Independent **proposed, unexecuted** literal cases are linked in
[watercraft-command-vectors.md](watercraft-command-vectors.md#ai-motion-final-round-3).
