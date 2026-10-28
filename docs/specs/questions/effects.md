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
