# sound-framework: open questions
<!-- Questions about [sound-framework.md](../sound-framework.md). Append new entries at the END; when one is answered, move it to answered/sound-framework.md. Rules: README.md. -->

<!-- from: re-sndsys (2026-10-22) -->
- **SY3 - section 4.2:** what the instance cache's keys and lists are (which ids it maps, who fills it). [sound
  instance ids grouped by sound id]
  **PARTIAL 2026-10-27:** §4.2 now specifies verified insertion/removal/reverse-lookup behavior (0x0096A760, 0x0096A6C2, 0x0096A6F5, 0x0096A53F). No producer establishing whether keys/values are sound IDs or instance IDs was found; semantic mapping remains open.

<!-- from: re-sndsys (2026-10-22) -->
- **SY4 - section 4.5:** the meaning of the two bytes after `"XA"` in the XA file header (version, channel count or
  padding). [a version, ignored]
  **PARTIAL 2026-10-27:** Open validates only XA at offsets 0–1 and the following PCM format tag; offsets 2–3 are ignored (0x0096B078, §4.5). This proves acceptance behavior, not whether those bytes are a version or padding; retained open.
