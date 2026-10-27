# cheat-dialog: answered questions (archive)
<!-- Archive for [cheat-dialog.md](../../cheat-dialog.md). Open questions are in ../cheat-dialog.md. -->

<!-- from: re-cheatdlg7 (2026-10-23) -->
- ~~**CCD1 - §3:** message type `0x11` moves the focus to the text edit before the base handling. Which window
  message is `0x11` (activation, or shown)? [inferred]~~
  **ANSWERED 2026-10-27 (re-q-widget):** DIFFERS — Message 0x11 is keyboard focus gained; the original jump table dispatches it to window slot 132, and the dialog redirects focus to its edit (§3).

<!-- from: re-cheatdlg7 (2026-10-23) -->
- ~~**CCD3 - §1, §4.4:** the sound IDs `+0x114` and `+0x118` stay 0, so the dialog never plays a sound. Did a
  build set them (for example from the creator `0x004E0BB0`), or are the sounds simply unused? [unknown]~~
  **ANSWERED 2026-10-27 (re-q-widget):** MATCHES — The 1.1.641 creator and Init supply no sound configuration after the constructor zeroes both IDs; the normal dialog path is silent (§1, §4.4).
