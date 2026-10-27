# city-tools: answered questions (archive)

<!-- from: re-citytools (2026-10-20) -->
- ~~**CT1 - §2, §3.2, §3.4:** which key the terraforming tool reads as `0x14` through window manager slot 6, slot 38,
  slot 1 (Caps Lock?), what the settings flag `0x00B43CC4` → `+0x0C` → byte `+0x23C` gates (the brush echoes), and
  what the torch's modifier bit 4 is (the left button?). [as read]~~
  **ANSWERED 2026-10-27 (re-q-city):** MATCHES — key `0x14` is the virtual key Caps Lock, tested as "held with no Shift, Ctrl or Alt" through the keyboard state (`IsKeyComboDown(0x14, 0)`, reached by the view window's slot 6, the window manager's slot 38 and the keyboard's slot 1). The settings flag is the render property boolean 17 `kUseCursorEffects`, which gates the two brush-cursor echo effects. The torch's modifier bit `0x10` is the left mouse button held (§2, §3.2, §3.4).

<!-- from: re-citytools (2026-10-20) -->
- ~~**CT2 - §2:** how the sound system's slots 13 and 12 turn the torch's tool messages into events, and which
  handler consumes each type (audio.md §7.4 names `HandleTorchControlMessages`). [as read]~~
  **ANSWERED 2026-10-27 (re-q-city):** MATCHES — `SendTorchToolAudioMessage` creates the event with the sound system's slot 13 `(type, IID 0x5B523BA4, &event, a, b, c, 0)` and posts it with slot 12; the words are x, y and the tool timer's elapsed milliseconds (zeros for on, off, the active flag and the deactivation release). The consumer is `cSC4AudioEventHandler::DoEvent`, audio.md §7.5 (§2).

<!-- from: coordinator PR review (2026-10-22) -->
- ~~**CT3 - coverage attribution:** independently identify the library routine at0x008AE57B (498bytes), currently auto-named as TerrainScrubber::SetCursorEchoPos. Its only direct caller/callee are0x008A6D50/0x008AE16E inside the FreeType range; the real control method is0x004D9160. That is strong misnaming evidence but does not independently prove the exact library identity. The review retains the current coverage-libs range and withholds the TerrainScrubber owner credit until the attribution is reconciled. No native algorithm is changed.~~
  **ANSWERED 2026-10-27 (re-q-city):** MATCHES — `0x008AE57B` is FreeType's, not the terraforming tool's: it is the wrapper that sets up a TrueType bytecode interpreter context: it calls `0x008AE16E` first, which stores the 256-entry opcode dispatch table whose repeats follow the TrueType opcode groups; the coverage range and the withheld owner credit stand. The library's exact function name is not recoverable (§3.4 "Library identity").
