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
