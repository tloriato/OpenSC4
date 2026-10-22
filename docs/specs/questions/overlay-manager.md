# overlay-manager: open questions
<!-- Questions about [overlay-manager.md](../overlay-manager.md). Append new entries at the END; when one is answered, move it to answered/overlay-manager.md. Rules: README.md. -->

<!-- from: re-overlays (2026-10-20) -->
- **OV1 - §3:** the layout of the nine-float placement record (slots 12-14) and the last arguments of the add
  calls (slots 3-6, 9-11); a centre, extents and an orientation are inferred from how `PlaceOverlay`
  (`0x0073A1B0`) turns it into a cell rectangle and a texture matrix. [the record as copied]

<!-- from: re-overlays (2026-10-20) -->
- **OV2 - §3:** what slot 20 (`0x007364F0`, no function in the export) does, and how the add forms of slots 8, 9
  and 11 differ from slot 4 (slot 11 is the largest and is inferred to build a shadow from a model's
  footprint). [unread]

<!-- from: re-overlays (2026-10-20) -->
- **OV3 - §5:** what message `0xC9F775BB` is and what the managers do on it. [inferred: the overlays are
  placed again after a terrain or zoom change]
