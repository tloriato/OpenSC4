# animation-coordinator: answered questions (archive)

<!-- from: re-animcoord (2026-10-19) -->
- ~~**AC1 - §1.1, §3:** what message `0x231BBF91` is, and what the byte `+0xF16` of the global `0x00B43CD8` holds (an
  animation detail setting is inferred; animation needs it at 2 or more).~~
  **ANSWERED 2026-10-27 (re-q-misc):** DIFFERS — `0x231BBF91` is kMessageTypePreferencesChanged (posted when an options dialog is accepted), and byte `+0xF16` of `0x00B43CD8` (the preferences object) is the Graphic Detail option (1 low, 2 medium, 3 high). Animation is enabled when it is above 1 (not "2 or more" of an unnamed setting) and is re-read, with registered occupants only, in `Init` and on that message; every block is re-evaluated only when the enabled state actually changed, not on every message (§1.3, §3).

<!-- from: re-animcoord (2026-10-19) -->
- ~~**AC2 - §2:** the exact test applied to an occupant whose exemplar has property `0x8A5F88E3` (a funding check
  against its budget item is inferred).~~
  **ANSWERED 2026-10-27 (re-q-misc):** MATCHES — the test (`0x00446290`, `0x00446210`) needs at least two values: a budget purpose ID and an unsigned funding threshold in percent. The occupant's lot (lot manager slot 14), its building (lot slot 48) and the budget item of that building for the purpose (budget simulator slot 51) give a funding fraction (0 when any is missing); the occupant animates unless the threshold is above 100 × fraction (equality and NaN pass). The power property `0xEA5393ED` is tested first (§2.1).
