# audio-preferences: open questions
<!-- Questions about [audio-preferences.md](../audio-preferences.md). Append new entries at the END; when one is answered, move it to answered/audio-preferences.md. Rules: README.md. -->

<!-- from: re-audioprefs7 (2026-10-23) -->
- **APW2 - §2.1:** the song list record word `+4` (`0xAA5FC8D5` Region, `0x2A5FC8DD` Mayor), set by `StaticInit` and
  never read by this owner: which code reads it, and is it a string ID for the list's name? [unknown]
  **PARTIAL 2026-10-27:** StaticInit writes both words (0x004F2B70); no reader was established by the owner or executable references to these two static field addresses. List-record pointers are computed by 0x004F1F70, so direct-address absence is insufficient to prove global nonuse. Their proposed localized-name meaning remains unresolved.
