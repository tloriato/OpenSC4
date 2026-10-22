# render-properties: open questions
<!-- Questions about [render-properties.md](../render-properties.md). Append new entries at the END; when one is answered, move it to answered/render-properties.md. Rules: README.md. -->

<!-- from: re-renderprops (2026-10-19) -->
- **RP1 - §1.2, §3:** who reads the two extra values of an integer or float entry (`+0x20`, `+0x24`; for example
  1–10 for `kParticleEffectsLOD`, 0–5 for the backing-store zooms)? None of the 18 slots does, and set ignores them.

<!-- from: re-renderprops (2026-10-19) -->
- **RP2 - §1.2:** what the two extra bytes of a boolean entry (`+0x1D`, `+0x1E`, always 0 at registration) are for.
