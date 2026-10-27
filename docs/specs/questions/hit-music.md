# hit-music: open questions
<!-- Questions about [hit-music.md](../hit-music.md). Append new entries at the END; when one is answered, move it to answered/hit-music.md. Rules: README.md. -->

<!-- from: re-hitman (2026-10-22) -->
- **HM2 - section 6.3, 6.4:** where a starting track first registers its tick with the sound system (only the
  `Unpause` path, `0x0093B08B`, calls sound-system slot 30), and the unit of its period `0x14` and of the tick's
  `dt`. [registered through the +0x0C interface's slot 18 on start; milliseconds]

<!-- from: re-hitman (2026-10-22) -->
- **HM4 - section 5.2:** the roles of TKDT fields 6 (`0xFF706616`), 14 (`0xFFD7EE4D`) and 16 (`0xFF2D7CD1`): read and
  written by the TKDT codec, copied by the sems (field 6), no other reader in the Hit classes. [read by the SC4 audio
  managers]
  **PARTIAL 2026-10-27:** Field 6 is copied as alias 0x25 by both the default sem setup (0x00944117) and SC4 sem setup (0x004ECDA0). The fixed property IDs for fields 14/16 appear in the codec, but no semantic reader was established; indirect register access prevents treating that search as proof of nonuse. Acoustic roles remain unresolved.

<!-- from: re-hitman (2026-10-22) -->
- **HM7 - section 2, 3.2:** which `.hit` files the shipped Hit INI lists under `TrackDataBlocks` (the INI path comes
  from sound-system slot 20), and the layout of a `.hit` file beyond the `ENTP`…`EENT` entry table. [the INI and the
  .hit data are in the sound packages; layout unknown]
