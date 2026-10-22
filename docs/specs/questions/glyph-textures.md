# glyph-textures: open questions
<!-- Questions about [glyph-textures.md](../glyph-textures.md). Append new entries at the END; when one is answered, move it to answered/glyph-textures.md. Rules: README.md. -->

<!-- from: re-glyphs (2026-10-19) -->
- **GT1 - §0, §1.3:** what `RZ::WCM()` (`0x00913C72`) returns, and whether its slot 5 is a font lookup by ID as this
  spec infers.

<!-- from: re-glyphs (2026-10-19) -->
- **GT2 - §1.2:** which framework services `Init` registers the glyph manager with through framework slots 3 and 9
  (`0x005D7D50`); the tick list is inferred from `OnTick`.
