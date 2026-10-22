# disasters: open questions
<!-- Questions about [disasters.md](../disasters.md). Append new entries at the END; when one is answered, move it to answered/disasters.md. Rules: README.md. -->

## From the disaster layer glue (filed by the coordinator from the `glue13` lane's report, 2026-10-16; merged `e7386b0`)

glue13 wired `fc4_disasters_host` to the game (`FC4DisasterHost`) and added `disasters_shadow = 1` (`FC4DisasterShadow`):
the original layer runs and a mirror core follows it (ABI 67, taken as 61 on the lane). The lane's final run
dis-final-11 was clean: fire 211/263 lines exact, riot 2/354, quiet 0/1, and every other difference is tagged with one
of the questions below in the log. The lane's Q36 (the riot's AttemptDisasterStart) is answered by disasters.md §6.2
(it always returns null; the shadow takes the instance from the first riot RegisterDisasterLocation). The lane's own
numbers (its Q26-Q42) are given in parentheses; the current choice is in brackets.

<!-- from: From the disaster layer glue (2026-10-16) -->
- **DS28:** (Q29) toxic spill, terrain, pipe burst and volcano are not mirrored: no start path was exercised. [fire and
  riot only]

<!-- from: verify-disasters4 (2026-10-21) -->
- **DS41:** §5.4 step 4 says an occupant rectangle with no overlap with the city "gives an empty rectangle", then
  averages it with `GetAverageValueInCellRect(x, z, w, h)` (`sum / (w × h)`). With w × h = 0 the division (and
  simgrid §4.1 step 1's one-tract test with `x + w − 1 < x`) is not specified: what coverage byte does the original's
  step 4 (and §5.6's spread) read for such an occupant, and does it refuse? [the plugin reads 0 coverage, so it does
  not refuse at step 4; unreachable for an occupant inside the city]

<!-- from: glue-disasters-final4, final publication boundary audit (2026-10-22) -->
- **DS47 - sections 2.8 and 15, complete typed layer interface:** please publish the complete C++ declaration
  of the 19-slot `cISC4DisasterLayer` interface, explicitly identifying the `void` returns required for activation.
  The current SDK header declares `bool` for slots 3/4/8/9/11/13/14/15 and gives slot 9 an instance argument;
  section 2.4 resolves the slot 9 argument to none, but does not specify its C++ return type. The published
  `interfaces.tsv` rows classify slot 8 as `via-call` and slots 3/4/9/11/13/14/15 as `eax-or-void`, rather than
  giving typed return contracts. The existing compiled `FC4AutomataInterfaces.h` declaration has only 16 slots,
  placeholders for 3-12, and a one-argument slot 15; the preserved owned draft supplies 19 slots but declares
  those methods `bool`. Which of these returns must be `void`, and what are the exact declarations of all 19
  slots (including slot 15 arguments and slot 18 record-pointer type)? [activation withheld; no guessed return
  signatures, no native calls through the draft, no plugin/config/build registration changes; DS46 is resolved by
  section 2.6.1 and is not the blocker]
