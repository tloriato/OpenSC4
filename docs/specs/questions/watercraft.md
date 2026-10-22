# watercraft: open questions
<!-- Questions about [watercraft.md](../watercraft.md). Append new entries at the END; when one is answered, move it to answered/watercraft.md. Rules: README.md. -->

<!-- from: re-boatsped (2026-10-21) -->
- **WC1 - §1, §5.1:** who sets the byte `+0x2CC` (count the idle budget in every animation tick); no watercraft
  function writes it except `Init`, which clears it. [as read]

<!-- from: re-boatsped (2026-10-21) -->
- **WC2 - §5.1:** what the caller of the boat's animation tick does when it returns false (a boat still for more
  than 5 s outside states 7, 16, 75–78), and the meaning of drive-parameter entries 4, 11–14, 18, 22–24, 26, 30, 31
  and 34. [as read]

<!-- from: core-watercraft4 (2026-10-22) -->
- **WC3 - complete public owner inventory:** the implementation brief attributes 79 functions / 38556 bytes to cSC4Watercraft, but sections 1-7 and 9 name only part of them. Please publish the exact 79-method inventory, signatures, constructor/destructor and inherited-call contracts, including return values and failure paths. Which four Init flags are set (only three are named), how are failed effect creation/base Init handled, does clearing timers also clear the lifetime limit, and what version checks does Read perform? [no complete-owner coverage claim; owner initialization portion sets the three named flags and clears all named timers including limit; failed resource creation is an explicit gap; record reader supports documented version 2 only]

<!-- from: core-watercraft4 (2026-10-22) -->
- **WC4 - sections 5.1, 5.2, 5.3 and 6.1 movement:** please expand Engine_AIBoatMotion, Movement_PositionNew and its four helpers, inherited animation/player-drive steps, and effect/model updates into executable arithmetic, ordering and failure contracts. Missing details include interpolation, steering, speed selection, water height/tilt, collision footprint construction, route-coordinate encodings, whether final docking completion is tested before or after motion, whether wake heading change means vector-delta magnitude or angle, and whether final-approach/following dt is clamped. [no original owner computation accepted as provider input; final docking completion tested also after motion, final approach dt unclamped, wake change uses vector-delta magnitude; far-approach and full animation/drive ticks remain unsupported]

<!-- from: core-watercraft4 (2026-10-22) -->
- **WC5 - section 5.7 and automata section 6.6 crash/bump:** what are the exact footprint-overlap, inanimate hit, bump response, size-class and crash-sequence equations (sink/heel axes, ramps, RNG draws and boundary rules)? Section 6.6 gives accumulator writes but no crash animation formula. Does crash entry notify (0xB,0) as watercraft section 4 states, or (0xB,duration) as automata section 6.6 states? How are equal-size losers and non-player boats sinking after a player win handled? [literal watercraft entry notification (0xB,0), question-marked; threshold classification/state-case dispatch only, no full collision/crash-motion claim]

<!-- from: core-watercraft4 (2026-10-22) -->
- **WC6 - section 6.2 command arithmetic:** please expand summarized speed/steering cases, input ranges/sentinels, counter-steer sign, neutral steering return, tween resets, sound selection and skier/fire positions into exact branches. In particular, how does throttle/reverse magnitude map to speed, and can braking consume leftover dt to accelerate in the opposite direction? [implement exact gear selection/coasting/button gates separately; full command translation unsupported]

<!-- from: core-watercraft4 (2026-10-22) -->
- **WC7 - sections 5.2-5.5 routes and docking:** publish the direction-to-cell/heading convention, compressed route reader format and errors, backtracking tried-bit choice, ferry exhaustion, station-vs-berth distance coordinates, initial station dock-request acquisition, and queued-with-occupied-berth behavior. When undocking cannot move/is not navigable, or docking pushes 72 on land, what does its tick return? What happens for zero-length/antiparallel heading or away vectors, including a boat exactly at the route occupant? Which station owns a newly acquired berth when current crossing still starts at the other station; when is held-berth set? For towing, which radius determines following distance, what is its translation rate, and what breaks nearest-boat distance ties? Does exiting 77 clear being_towed (not stated in section 4)? [decoded network cells only; occupied queued branch retains state; zero/antiparallel/tied cases explicitly unsupported; no guessed held-berth assignment; being_towed not cleared on exit]

## Partial evidence published 2026-10-22 (re-watercraft-completion4)

These questions remain open as compound requests; the following subparts now have native contracts in watercraft.md.
No entry is moved to answered while its remaining requested bodies or failure cases are untraced.

- WC3: §3/§3.1 prove the fourth Init flag 0x8000, unconditional continuation after base Init, ignored effect
  acquisition returns/no rollback, clearing both lifetime fields, exact owner-version-2 gate, and query ownership.
  The companion watercraft-owner-inventory.md lists all 75 functions / 38073 bytes reproducible with current
  attribution, versus the requested 79 / 38556. The four-function/483-byte discrepancy, constructor/destructor
  closure, all signatures and inherited internals remain unresolved. No attribution or coverage inputs changed.
- WC4: §5.1 corrects time provenance: the second float drives owner timers and AI motion; gives lifetime/wake/
  callback ordering and strict boundaries, plus an asymmetric null-effect hazard. §6.2.2 specifies full straight
  translation and outer movement publication, including p4 scale and displacement stores. §5.2 corrects the
  helper grouping: 0x0056A440 and 0x0056AF90 belong to crash motion. Full AI steering/interpolation, turning,
  height/tilt and inherited step internals remain open; no whole animation/movement implementation claim.
- WC5: §4 proves owner crash-entry notification (0xB,0); §5.7 gives equal-size double outcomes, class priority,
  callback order, and the distinction between AI sinking and the collision-sunk effect byte. Footprint/inanimate
  geometry, bump response and the complete crash/RNG equations remain open.
- WC6: §6.2.1 gives native nonlinear magnitude mapping and crossing-zero braking, with independent numeric
  vectors. A braking step can cross zero without a separate leftover-dt acceleration step. Full same-sign speed
  cases, inverse feedback, steering, sound, fire/skier positions and tween behavior remain open.
- WC7: §5.2 gives exact eight-direction signed cell deltas; §6.3 proves stern/bow coordinates, own-radius+10
  separation and instantaneous excess-distance correction, independent of dt/p39. Degenerate angle/normalization,
  route decoding, route selection/backtracking, dock ownership and nearest-boat ties remain open.
