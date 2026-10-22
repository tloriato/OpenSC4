# city-tools: open questions
<!-- Questions about [city-tools.md](../city-tools.md). Append new entries at the END; when one is answered, move it to answered/city-tools.md. Rules: README.md. -->

<!-- from: re-citytools (2026-10-20) -->
- **CT1 - §2, §3.2, §3.4:** which key the terraforming tool reads as `0x14` through window manager slot 6, slot 38,
  slot 1 (Caps Lock?), what the settings flag `0x00B43CC4` → `+0x0C` → byte `+0x23C` gates (the brush echoes), and
  what the torch's modifier bit 4 is (the left button?). [as read]
- **CT2 - §2:** how the sound system's slots 13 and 12 turn the torch's tool messages into events, and which
  handler consumes each type (audio.md §7.4 names `HandleTorchControlMessages`). [as read]

<!-- from: coordinator PR review (2026-10-22) -->
- **CT3 - coverage attribution:** independently identify the library routine at0x008AE57B (498bytes), currently auto-named as TerrainScrubber::SetCursorEchoPos. Its only direct caller/callee are0x008A6D50/0x008AE16E inside the FreeType range; the real control method is0x004D9160. That is strong misnaming evidence but does not independently prove the exact library identity. The review retains the current coverage-libs range and withholds the TerrainScrubber owner credit until the attribution is reconciled. No native algorithm is changed.
