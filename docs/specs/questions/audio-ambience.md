# audio-ambience: open questions
<!-- Questions about [audio-ambience.md](../audio-ambience.md). Append new entries at the END; when one is answered, move it to answered/audio-ambience.md. Rules: README.md. -->

<!-- from: re-audio4 (2026-10-21) -->
- **AA1 - section 8:** the ambience layer's own behaviour (scores, loops, filters, per-layer tuning,
  `0x004E5110`-`0x004E8230`) is unspecified; only the interface the manager calls is. Needed before
  `cSC4AudioAmbienceLayer` can count as specified.
- **AA2 - section 3.4:** what does `_UpdateLotOccupantsForConditionChange` (`0x004E9DA0`) start or stop per lot
  condition, for properties `0xCA19D7CA`, `0x0A4C13CB`, `0x4A4C132E`?
- **AA3 - section 3.6:** confirm the stack slot copied into the alarm entry `+0x10` at `0x004E91B0` is the time
  of day `t` (the listing reuses `[ESP+0x18]` for `a[i]`).
- **AA4 - section 3.1:** name occupant types `0xA823821E` and `0xC7CC75F1` of the init walk.

<!-- from: core-audioambience4 (2026-10-21) -->
- **AA5 - section 3.6:** what is an alarm entry's initial `last` (`+0x10`) when `_AddAlarmOccupant` creates it?
  The engine uses 0.0, so a one-shot alarm whose time already passed today fires on the first tick.
- **AA6 - sections 2.2, 4.1, 4.2:** (a) a `0x0A36F410` list with other than 5 values: how many margins are
  read, and what fills the rest? (b) `0x2A3FF8CD` missing: is the layer-id vector left as it was or emptied?
  (c) audio preferences object absent at city init: what volume reaches slot 17? (d) zoom index outside 0-4
  (or margins never read): the game has no bounds check; what does it read? The engine copies up to 5 values
  (rest 70.0), empties the vector, uses volume 0, and leaves the rectangle unchanged.
- **AA7 - section 5:** the `RandomIterator` over 2x2-cell blocks: are the block bounds `x0/2 ..= x1/2`
  (inclusive) and what visit order does it produce for a given seed? The engine visits inclusive bounds
  row-major (z outer).
- **AA8 - sections 3.1, 3.2, 3.5, 3.6:** instance ids and preconditions not stated: (a) is the
  `0xC9B93A56` sound started through `_AddOccupantSndInstance` (start + set-position + automaton link) with
  instance id occupant + id? (b) the move set-position for `0xCA19D7CA` (`0x004E8C70`): instance id occupant +
  that property's value? (c) automaton codes 9/12/13 when the occupant lacks `0x6A36AFAC`: nothing? (d) the
  on/off alarm loop's instance id (occupant + `0x2A53BE13` value?) and what happens when the re-read property
  is absent? (e) is the railway-crossing instance id occupant + `0xAA87817A` value? (f) do the automaton
  handlers skip preview occupants? The engine answers yes to (a), (b), (e), "nothing" to (c) and (d)-absent,
  occupant + id for (d), and no preview check for (f).
- **AA9 - sections 6, 7:** (a) what does the focus handler `0x004E8CF0` return (the engine returns 1)? (b)
  `0x2A2C0384` whose payload does not answer `0x65297976`: handled (1) or not (0)? The engine returns 0. (c)
  what does the release `0x004E9660` return, and does "empty the lists" include the alarm list? The engine
  returns 1 and empties layers and alarms.
