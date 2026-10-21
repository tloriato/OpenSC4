# automata: open questions
<!-- Questions about [automata.md](../automata.md). Append new entries at the END; when one is answered, move it to answered/automata.md. Rules: README.md. -->

## From the automata state behaviours and the effect-directory measurements (re-integrate1 lane, 2026-10-16)

Four RE reports, kept verbatim in `build-runs/re-integrate1-automata.md`, `re-integrate1-effects.md`,
`re-integrate1-vehicle-ticks.md` and `re-integrate1-air-water-ticks.md`. Their findings went into automata.md
§6.2, §6.5, §6.6, §8.1, §12-§14 and effects.md §3, §5.2, §7, §10.1, §10.2, §15-§17.

**Nothing needed striking here for the automata state questions**: automata.md's Q3-Q5 (filed above as AT3-AT5)
and AM1-AM12 were already answered by re-answers16 and re-answers20, and the effects questions EF1-EF5 lived in
effects.md §16 rather than in this file. Two of those are now closed or answered and are struck below so a
reader does not re-open them. AT6-AT19 continue automata.md §13's new Q7-Q20; EF1-EF10 continue effects.md §16.
The probe named is the one the report gives. **All of AT6-AT19 were answered on 2026-10-20 by re-answers33**
and moved to `answered/automata.md`.

## From the glue19 lane (2026-10-20)

**AT20.** Automata.md section 9 gives the U-Drive-It pause and resume hooks only from the manager's side
(slots 9 and 10) and says the city situation manager drives them, but the spec does not name the manager's
side: what city-situation slots or messages reach `fc4_automata_pause_all` / `_resume_all`, and when exactly
(countdown start, countdown end, mission end)? A replacement wiring this from the outside (glue19 wires them
to the manager's `MissionActivate` / countdown-end points through the city-situation interfaces) needs the
receiver names to check it against the game's own call sites.

## From the verify-automatarefs4 lane (2026-10-21)

**AT21.** Where does the original controller manager (city slot 47) drop the references it holds on its
controllers, and on the occupants and automata of its record, at city close? §3.4 says the manager keeps the
reference its Read returned, §7.1 (AM7 step 3) that a removed controller is shut down (slot 4) and released, and
§10 lists `0x99EF1143` RemoveOccupant as "drops the occupant's controllers [inferred]", but no section names the
manager's own `Shutdown` (slot 4) or destructor behaviour. In a city loaded and then saved (foundations §2.7 SV1:
the load's object table is gone), run sg4-c1 (2026-10-21) shows 214 generator (`0x4A1628C5`) and 24 attractor
(`0x6A1628BA`) controllers whose last reference is the manager's. Please state: (a) which of `PreCityShutdown`
`0x26D31EC2`, the RemoveOccupant handler during the occupant manager's shutdown, slot 4 `Shutdown` or the
destructor releases them, in which order relative to the occupant manager's shutdown; (b) whether each controller
is shut down (controller slot 4) before that release; (c) whether the queue entries (§3.3) and the 24-hour clock
reference (§3.1 field 2) go at the same point. verify-automatarefs4 keeps them to the destructor (the literal
reading of "the manager holds them") and declares that release the manager's own (FC4_RELEASE_AT_DESTRUCTION).
