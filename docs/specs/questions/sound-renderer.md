# sound-renderer: open questions
<!-- Questions about [sound-renderer.md](../sound-renderer.md). Append new entries at the END; when one is answered, move it to answered/sound-renderer.md. Rules: README.md. -->

<!-- from: re-sndrender (2026-10-22) -->
- **SRN2 - section 5:** which parameter the 0–127 value scaled by 1/127 sets on the 3D helper (cone outside volume,
  Doppler or roll-off). [roll-off]
  **PARTIAL 2026-10-27:** The scaled float reaches helper slot 6; 0x008792D0 queries a property-set interface on the buffer and writes property 1 under GUID 0x00B11E30 (§5). This is not a listener factor setter, but the property's acoustic identity was not established; retained open.
