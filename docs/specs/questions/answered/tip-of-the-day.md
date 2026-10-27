# tip-of-the-day: answered questions (archive)
<!-- Archive for [tip-of-the-day.md](../../tip-of-the-day.md). Open questions are in ../tip-of-the-day.md. -->

<!-- from: re-gztips7 (2026-10-24) -->
- ~~**TOD1 - §6, §7:** `EndDialog` stores the check box `0xAA3BE181` in the preferences byte `+0x49C`, but the only
  opener (the 3D view's `DoPostCityInit`, which tests `HasBeenShownAlready` and `EntriesPresent`) does not read it.
  Where is `+0x49C` read (start-up, the preferences dialog, saving), and does clearing the box stop the dialog from
  appearing? [static: readers of `0x00B43CD8` `+0x49C`; dynamic: clear the box and reload a city]~~
  **ANSWERED 2026-10-27 (re-q-widget):** MATCHES — The checkbox is persisted in the version-0x15 preferences payload but does not gate the automatic city-init opener (§1, §6, §7).
