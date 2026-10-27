# freshness: answered questions (archive)

<!-- from: re-freshness (2026-10-22) -->
- ~~**FS1 - section 3.1:** the roles of cell fields 8 (`+0x3C`), 9 (`+0x44`), 12 (`+0x4C`), 16 (`+0x20`) and 18
  (`+0x60`), and of score `+0x38`/`+0x3C`: parsed, with no reader in the `cFresh*` classes. [unused by this build]~~
  **ANSWERED 2026-10-27 (re-q-city):** MATCHES — none of them has a role in this build. Cell fields 8, 9, 12, 16 and 18 and the score's `+0x38`/`+0x3C` are parsed and stored, and nothing reads them: the player, the cell player and the dimension player read the cell's other fields directly, and SimCity 4's only client (the ambience layer) never touches cells. A re-implementation keeps the numbers and lets them change nothing (§3.2).

<!-- from: re-freshness (2026-10-22) -->
- ~~**FS2 - section 4.6 step 3:** what sets the start switch `+0x4088` and the two suspend bytes `+0x4090`/`+0x4091`
  the beat clock tests; no `cFreshPlayer` method read here writes them. [set through the +0x04 interface]~~
  **ANSWERED 2026-10-27 (re-q-city):** DIFFERS — there is no separate start switch. `+0x4088` is the **play percent** (slot 16 sets it, only once initialised; default 100): the beat clock starts nothing while it is 0, and §4.4 scales the cell budget by it. `+0x4090` is the stress flag (slot 50): while non-zero the beat clock skips its whole update. `+0x4091` is "a volume fade is running" (set with the fade timer, cleared when the fade ends): while set, no new cell is started (§4.6 step 3, §4.9, §4.2).

<!-- from: re-freshness (2026-10-22) -->
- ~~**FS3 - section 5.3:** the looping cell's stop rule while the stopping mark is set (the export stops while the
  loop counter is non-zero, which reads inverted). [stop when the counter reaches 0]~~
  **ANSWERED 2026-10-27 (re-q-city):** DIFFERS — the export is as read, not inverted, but the rule is not "stop when the counter reaches 0". A looping cell with the stopping mark is **killed** (sound released at once, no fade, state idle, mark cleared) at an update where the decremented counter is non-zero; a counter that lands exactly on 0 leaves it playing until the next update. So a marked cell is cut at its next update (the same beat when the beat clock marked it), not at the end of its loop (§5.3).

<!-- from: re-freshness (2026-10-22) -->
- ~~**FS4 - section 4.9:** the meaning of the callback timer's slot-1 call with 100 before it is started. [a priority
  or resolution]~~
  **ANSWERED 2026-10-27 (re-q-city):** DIFFERS — it is `SetDelay(100)` on the callback timer, not a priority or resolution. The delay is already 100 by default and the timer never passes it to its service (timers.md §2.2), so the call has no effect; only the period `ms / 20` of the start call matters (§4.9).
