# audio-preferences: answered questions

<!-- from: re-audioprefs7 (2026-10-23) -->
- ~~**APW1 - §4.4, §4.2:** the scale of the volume words the dialog stores and sends (sound system slot 7 with
  `0x2A5B69C0`, radio slot 16 through `0xAA650860`): is 1024 (the shipped ceiling) full volume and 0 silence, and is
  the curve linear? [inferred: 0–1024 linear]~~
  **ANSWERED 2026-10-27 (re-q-audio):** DIFFERS — 0 is mute and 1024 unity; slider interpolation is linear in framework units, but renderer volume uses a square-root/dB conversion (§4.2, §4.4).

<!-- from: re-audioprefs7 (2026-10-23) -->
- ~~**APW3 - §6.3:** what the song buttons' slot 17 does with the buffer of the UI script key
  (`0x856DDBAC`, `0x46A006B0`, `0x14416244`), and what button slots 12 `(4, 0, 0)` and 5 `(4, 0)` set.
  [inferred: the button's look and its checkbox style]~~
  **ANSWERED 2026-10-27 (re-q-audio):** DIFFERS — Slot 12 selects eight-image bitmap style 4, slot 17 supplies the bitmap, and slot 5 clears caption flag 4; checked state is separate (§6.3).

