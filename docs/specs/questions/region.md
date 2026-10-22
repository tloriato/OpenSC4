# region: open questions
<!-- Questions about [region.md](../region.md). Append new entries at the END; when one is answered, move it to answered/region.md. Rules: README.md. -->

<!-- from: re-region (2026-10-20) -->
- **RQ1 - §2, §4, §6:** what file attribute 2 is (`cRZFile::GetAttribute` / `SetAttribute(path, 2, 1)`), which the
  region sets on `region.ini` before writing it and on the city files it loads or moves; the spec reads it as
  "writable". [as read]
<!-- from: re-region (2026-10-20) -->
- **RQ2 - §6, §8:** the file-name rule of `SC4CreateSavePathForCity` (`0x005DC400`) and the name the starter region
  gets from its default-constructed string. [as read]
