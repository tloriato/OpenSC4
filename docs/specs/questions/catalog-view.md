# catalog-view: open questions
<!-- Questions about [catalog-view.md](../catalog-view.md). Append new entries at the END; when one is answered, move it to answered/catalog-view.md. Rules: README.md. -->

<!-- from: re-catview7 (2026-10-23) -->
- **CV2 - §3:** `GZPaint` calls the draw buffer's slot 6 and slot 7 with `0x8010` around the blit, and blits only
  when slot 6 returns true. Are these the buffer's lock and unlock with access flags? [inferred]
