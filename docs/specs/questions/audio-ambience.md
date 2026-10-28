# audio-ambience: open questions
<!-- Questions about [audio-ambience.md](../audio-ambience.md). Append new entries at the END; when one is answered, move it to answered/audio-ambience.md. Rules: README.md. -->

<!-- from: core-audio-inputfix4 (2026-10-22) -->
- **AA10 - section 3.6 alarm creation:** the original does not null-check the clock in `_AddAlarmOccupant` (crash); what should a replacement do when no clock exists at creation (create the entry with `last = 0.0`, or refuse it)? [entry created, `last = 0.0`]
  (re-audio-final-gaps4, 2026-10-22, native fact only: `o` is AddRef'd at `0x004EA6D5`; the null test at `0x004EA6DE`
  skips only the clock AddRef; `0x004EA6E9` then reads through the null clock (access violation at address 0), no
  entry; neither caller tests the clock. The binary has no non-crashing behaviour to copy, so this stays a
  policy decision for the coordinator/user, not an RE answer.)
