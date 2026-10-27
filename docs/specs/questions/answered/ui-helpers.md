# ui-helpers: answered questions (archive)
<!-- Archive for [ui-helpers.md](../../ui-helpers.md). Open questions are in ../ui-helpers.md. -->

<!-- from: re-uihelpers7 (2026-10-23) -->
- ~~**UIH1 - §5:** the global `0x00B43CE0` gets slot 10 `(dialog, 1)` before the modal run and slot 6 `(dialog)`
  after it. Is it a window-location store that restores and saves a dialog's place? Which class is it? [inferred]~~
  **ANSWERED 2026-10-27 (re-q-widget):** MATCHES — The service is cSLWinLocationSaver: restore by window ID before the modal run, save parent-relative left/top afterward, with an 8-pixel visibility check on restoration (§5).

<!-- from: re-uihelpers7 (2026-10-23) -->
- ~~**UIH2 - §5:** the dialog texts go through interface `0x212CDC1F` slot 4 and the button labels through the
  button interface `0x8810` slot 49. Are these the caption setters of a text window and a button? [inferred]~~
  **ANSWERED 2026-10-27 (re-q-widget):** MATCHES — Text slot 4 and button slot 49 are caption setters that update text layout and redraw state (§5).
