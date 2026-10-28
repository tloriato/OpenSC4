# effects: open questions
<!-- Questions about [effects.md](../effects.md). Append new entries at the END; when one is answered, move it to answered/effects.md. Rules: README.md. -->

<!-- re-answers34 (2026-10-20) answered EF1, EF4, EF5, EF6, EF7, EF8, EF9 and EF10; they moved to answered/effects.md.
     EF2 (closed re-integrate1) and EF3 (answered re-integrate1) were archived earlier. No open questions remain. -->

<!-- re-effects-contracts4 (2026-10-21) answered EF11, EF12 and EF13; they moved to answered/effects.md. No open questions remain. -->

<!-- re-effects14-4 (2026-10-21) answered EF14; it moved to answered/effects.md. No open questions remain. -->
No open questions. EF15 and EF16 are archived in answered/effects.md.

<!-- re-effects-ef18 (2026-10-24) answered EF18 and EF19; moved to answered/effects.md.
     EF17 remains reserved by the coordinating brief. No open questions remain. -->

<!-- re-effects-ef20 (2026-10-24) copied EF20 from glue-effects-shadow7 and answered
     it in effects.md section 18.7; original question archived in answered/effects.md. -->
<!-- re-effects-ef21 (2026-10-24) answered EF21 from glue-effects-capture8 in
     effects.md section 18.8; original question moved to answered/effects.md. -->

<!-- re-effects-ef21 (2026-10-24) answered EF21 from glue-effects-capture8 in
     effects.md section 18.8; original question moved to answered/effects.md. -->

<!-- re-effects-ef24 (2026-10-24) answered EF23 in effects.md section 18.8.4;
     answered EF22 in sections 18.8.6–18.8.7; originals moved to answered/effects.md. -->

No open questions remain.

<!-- re-ef29 (2026-10-26) answered EF25 from replace-effectsres28 in effects.md
     section 18.9; original question moved to answered/effects.md. -->

<!-- re-ef30 (2026-10-26) answered EF26 from replace-effectsres28 in effects.md
     section 18.10; original question moved to answered/effects.md. -->

<!-- re-ef31 (2026-10-26) imported EF27 from replace-effectsres30 (already
     identical to main) and answered it in effects.md section 18.9.7;
     original question moved to answered/effects.md. No open questions remain. -->

## EF28 — full-house startup fails before any effects-resource factory call

Lane `replace-effectsres30`, 2026-10-27. Acceptance (a) remains unqualified:
the train288 startup failure reproduces, but an unchanged-source repeat passes.
Please specify the lifecycle/first-use contract involved before an engine fix
is attempted; the resource implementation cannot explain this difference from
its current spec.

- After merging main (merge `bca6ee6ca`), full house list plus `effectsres`,
  `effectsres30-r3-house-on1.log` lines 10988–11009: first native audio tick
  faults at `004E2C06`, reading null, ECX=0. Stack: `004E51AD`, `004E537B`,
  `004E957F`, `004E9592`, `004EA216`; no plugin return address. Verdict corrupt,
  2026-01-27, real_seconds=0.031, foreign_exceptions=1, ref_warnings=0.
- `audio-filters.md` section 2 identifies `004E2C00` as the view-level global
  filter, using the 3D view at `00B43DC4` without a null check; `audio-ambience.md`
  section 8.4 specifies the enclosing CanHandle/HandleCell calls. `view3d-window.md`
  section 2 says its constructor publishes the view through SL::OnView3DCreate.
- This failed log has **zero** GetClassObject calls for CLSID `2A5118AD` and
  zero `cSC4EffectsResource` factory entries. Only registration/enumeration
  occurs. The resource's constructor, bindings and Read/Write are not entered
  through our creator before the fault. The effects manager is replaced too.
- Unchanged source/DLL, full house plus key: `effectsres30-r3-house-on2.log`
  and `.verdict` are clean through 2032-01-01, foreign_exceptions/ref_warnings=0.
  Same-source key-off control `effectsres30-r3-house-off1` is clean to that date;
  key-off `house-off2` and `house-off3` are clean to 2026-04-01. Failed and passing
  candidate DLL SHA256: `5ffd9d89b6428de369ebe045b1a4ef7a6ce589cebb545faf9ba066eeec768ac6`.
- The failed candidate constructs a **second** replacement effects manager
  (factory call 841, log lines 9552–9559) after PostCityInit; passing on2 and
  off1 each construct only one. This is an observation, not a causal claim.
- All logs/verdicts above live in `C:/Users/tlori/FreeCity4/build-runs/game-results/`;
  exact requests are in its sibling `game-queue/done/`. The original failure is
  `regress-train288-replace.log` (same native stack). Prior key-off
  `effectsres30-r2-s1off1.log` also has the same native fault, on earlier code.

Which specified path creates/destroys the city-screen view and the second
effects manager during this load, and which game interface/lifecycle boundary
guarantees that native audio sees a live view before its first tick? Is a
view-less load valid for that service? Please also identify any resource-class
registration side effect, before its first factory call, that the replacement
must preserve. No native audio/global patch or speculative timing fix was made.
<!-- re-ef32 (2026-10-27) imported EF28 from replace-effectsres30 and answered
     ownership, registration and the native-view prerequisite in effects.md
     section 18.11, audio-ambience.md section 2.5 and view3d-window.md section 2.1.
     Execution-specific attribution is narrowed to the trace in section 18.11.3;
     original question and answer moved to answered/effects.md. -->

## DSEX33-EF01 — effects height-map service used by native aircraft

Lane `disasters-exact33`, 2026-10-28. The coordinator's effects-only Pyxis
comparison `compare-20261027-230157-13af6338/result/game.log`, line 1903,
faults reading null at `00537188`, inside the ABI-identified helicopter
`AI_ControlLiftOverBuildings`. The replacement's manager slot 9 returned
null. Aircraft spec section 4.2.4 requires a service obtained once through
operation +0x24, then queried through +0x18 with `(1, X, Z)`.
The SDK names these `GetEffectMaps` and `MapLerpedHeight`.

Effects sections 2 and 11/18 do not specify map contents or their lifecycle.
Please specify the map dimensions/count, coordinates and interpolation,
map-1 population/update rules (including building/terrain changes), outside
bounds behavior, and ownership/QI contract. The lane exposes an owner-lived
empty service with zero heights as a marked SPEC-GAP to remove the null
call; this does not claim compatibility for map values.

## DSEX33-EF02 — effects tick registration and fixed-clock consumption

Lane `disasters-exact33`, 2026-10-28. Effects section 6.1 says Init calls
`EnableAutoStep(1, EffectsTickCallback)` on the animation tick manager, with
the callback posting `0x697D01ED`. However animation-tick-manager.md section 2
defines slot 14 as `EnableAutoStep(bool, u32 intervalUs)`, not a callback
registration. Please reconcile these contracts and specify the effects
manager's animation-target registration, priority, callback registration and
shutdown ownership. The adapter currently subscribes through the message
server; no guessed callback-as-integer call has been added.

Please also specify any clock/stopwatch reads made by the effects manager's
Init, animation tick, ordinary DoEffectsTick and draw pass, including their
conditions/order. Simulator section 5.8 advances the fixed counter per read,
so omitting a read can change another owner's animation delta. This is a
possible explanation to investigate, not an established cause.

Evidence: final effects-only qcheck-28.log is clean/ref_warnings=0 but exact
0/4. Pennoli result `C:/Users/tlori/FreeCity4/build-runs/golden-runs/compare-20261028-072900-5d057a8e/result`
has two different records, with no shared-rand or DIGEST difference. In
`896E75AF/299B2D1B/0`, the only payload difference is the opacity tween's
elapsed float at record offset 276 (record starts at byte 86621, size 335,
object ID 318): reference `0x3D07325A` = 0.0330070033669 seconds; candidate
`0x3D07314D` = 0.0330060012639 seconds. Delta is 1.00210309029 microseconds;
the other four different bytes are that record's checksum. Carjacked vehicle
record `CBB32B0A/299B2D1B/0` has small position/destination changes in three
324-byte records. The same differences occur in qcheck-25.log. This does not
justify adding dummy clock reads or changing FC4Schedule.

### DSEX33-EF02 follow-up: direct tick delivery and embedded timer (round 11)

Animation-tick-manager.md sections 4/5 specify direct delivery to registered
targets, rather than message-server broadcasts. The replacement now registers
its +8 target in the every-tick band (192, exact priority remains a SPEC-GAP)
and removes it at Shutdown. Effects-only qcheck-34.log remains 0/4 exact,
with the same first records/counts as qcheck-33.log; all four runs are clean
with ref_warnings=0. Registration alone has not removed these differences.

Animation-tick-manager.md section 7 also identifies the effects Init call as
slot +24 on the embedded +CD8 object, with arguments 0058D9A0 and zero,
rather than EnableAutoStep. Timers.md section 2.2 specifies that shape as
SetCallback(fn,arg) on cRZCallbackTimer. Please specify whether this is that
timer, its construction/start/stop order, period, and callback ABI. This
narrows the earlier contradictory contract; no callback-as-interval call,
guessed timer period, or compensating clock read has been added.

### DSEX33-EF02 follow-up: reach replay has no getter/creation calls (round 12)

Final-source effects-only qcheck-39.log remains exact 0/4, with the same
first differences as qcheck-38.log. Reach result:
`C:/Users/tlori/FreeCity4/build-runs/golden-runs/compare-20261028-092012-9e689352/result`.
There is no shared-rand or DIGEST difference; three save records differ.
An earlier identical first difference in qcheck-38.log was decoded using
landvalue section 10: intrinsic/slope records match, total land value differs,
and the simulator's saved average is reference 74 versus candidate 87;
its dirty byte is zero on both sides and its mean altitude is 290 on both.

Separate fixed-schedule replay `disasters-exact33-r12-query-reach-2` is clean
with ref_warnings=0. The new opt-in trace confirms one primary effects QI
(`C97CF5CD`) and its enabled marker; no other QI, HasVisualEffect, creation,
GetEffectMaps or GetTallestBuildingAltitude entries occur through April 9.
This replay uses the same city/action script, but is not a qcheck proof.
Native-only `disasters-exact33-r12-native-names-2` is also clean: all 3,465
Boolean membership queries match at load and each of three month samples.
That observation covers neither creation nor lifecycle equivalence.

Please specify the effects manager's Init and ordinary-message/timer
lifecycle, including collaborator writes and clock reads. These observations
do not establish the cause of the land-value difference, but do not identify
a returned getter value to change. No schedule change or clock compensation
has been made.
