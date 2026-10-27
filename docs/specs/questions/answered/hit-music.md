# hit-music: answered questions

<!-- from: re-hitman (2026-10-22) -->
- ~~**HM1 - section 6.3 step 1:** what the control group's gate (group slot 4, then slot 20 of the result) tests, and
  whether SC4 ever creates a control group (no caller of `AddToControlGroup` outside Hit was read). [a mute switch;
  no SC4 groups]~~
  **ANSWERED 2026-10-27 (re-q-audio):** DIFFERS — The gate reads group volume; a nonzero track group ID creates the group lazily on start, without AddToControlGroup; installed-data usage remains unclaimed (§6.3).

<!-- from: re-hitman (2026-10-22) -->
- ~~**HM3 - section 6.3 step 2:** what slots 7, 18 and 19 of the track player's `+0x0C` table are, and who clears the
  byte `+0x5C` that the resume path sets (the tick does nothing while it is set). [playing, unpause, paused; cleared
  by the next normal start]~~
  **ANSWERED 2026-10-27 (re-q-audio):** DIFFERS — Slots 7/18/19 are IsPlaying/Unpause/IsPaused; Unpause immediately clears the same 32-bit pause word at +0x5C (§6.3, §6.4).

<!-- from: re-hitman (2026-10-22) -->
- ~~**HM5 - section 3.9:** what `cIHitMan` slots 6 (calls `DuckMapRemoveAll`), 7 (returns true) and 8 (`IsPaused`, the
  byte `+0x21` that no HitMan method writes) are for. [unused pause API]~~
  **ANSWERED 2026-10-27 (re-q-audio):** MATCHES — Manager Pause clears ducking and returns true; slot 7 is a successful no-op; neither changes the initially zero flag read by IsPaused (§3.9).

<!-- from: re-hitman (2026-10-22) -->
- ~~**HM6 - section 3.8:** what the default track-flow error handler `0x009377DD` does: it calls slot 3 of the track,
  slot 9 of that result, then slot 16 of the track. [kills the failing track]~~
  **ANSWERED 2026-10-27 (re-q-audio):** MATCHES — The default handler kills the track through its sound interface, then sets its error condition (§3.8).

<!-- from: re-hitman (2026-10-22) -->
- ~~**HM8 - section 6.7:** whether the register-set slot 6 used by `UpdatePitch` means "set in the local set" (which makes
  the random pitch spread compound on every call). [yes]~~
  **ANSWERED 2026-10-27 (re-q-audio):** DIFFERS — Slot 6 tests presence in the local set or direct track definition; slot 3 writes the random frequency locally, so pitch spread does compound (§6.7).

