# region-model: open questions
<!-- Questions about [region-model.md](../region-model.md). Append new entries at the END; when one is answered, move it to answered/region-model.md. Rules: README.md. -->

<!-- from: re-region-model (2026-10-21) -->
- **RM1 - §3.2:** `Shutdown` writes the in-memory name into `region.ini`, and `LoadConfig` stores the resolved
  LTEXT text for a `Name = #0x...` token. Does a shipped region's `region.ini` really lose its token (for example
  `Name = #0xcab6d26e` becomes `Name = Berlin`) the first time the region is shut down, and does the language of
  that session then stick? [yes, both halves are read from the bytes; the engine keeps the token only if the name
  was never changed from the resolved text, which differs from the original]
- **RM2 - §3.2, §6.3:** `Delete` ends with `Shutdown`, which writes `region.ini` into the folder that `Delete` has
  just removed. Does the write recreate the folder or the file (does `cRZFile` create parent directories), or does
  it fail silently? [it fails silently, the folder stays gone]
- **RM3 - §4.2:** the picture is a BMP (bottom-up rows). The spec reads `y = 0` as the **top** row on screen, which
  matches the position of all 417 shipped city files. Which decoder loads a file called `config.bmp` into the
  buffer (a BMP reader, or a content-sniffing reader that also takes PNG, since the game-data templates are
  type `0x856DDBAC`), and does it always flip the BMP to top-down? [BMP and PNG both accepted, top row first]
- **RM4 - §5.1:** `SC4ReadAppLoadSaveVersion` (`0x005DBF30`) returns 0 or 65 when the package has no version
  record, depending on a second package call (DB segment slot `+0x4C`). What is that call, and is a city file
  without a version record accepted (65) or refused (0)? Do files saved by the retail 1.1.641 game always carry
  the record? [accepted as 65]
- **RM5 - §6.4:** the sanitiser removes `< > : \ / " | * ?` and cuts to 256 bytes, but before that it converts the
  text between encodings, along a path that depends on a global flag (`0x00B62910`). What is the effect for a
  region name with non-ASCII letters (does the folder get the UTF-8 bytes, the system code page, or are the
  letters kept)? [kept as UTF-8]
- **RM6 - §6.4:** `CreateStarterRegion` passes an empty name, and the folder-name loop then finds that
  `Regions\` + `""` + `\` exists, so the first free candidate is ` (2)`. Is the starter region's folder really
  named ` (2)` in the game, and is its name really empty? [yes, as read from the bytes]
- **RM7 - §7.3:** `MoveCity` has four arguments and no caller in the executable. The spec reads them as
  `(sourceRegion, city, x, z)` with the destination being `this` (the active region). Which windows call it (the
  region view's drag-and-drop?), what do they pass, and is the serial number change on a move within the same
  region (and the possible ` (n)` rename of its file) really visible in play? [as read]
- **RM8 - §7.6:** `ResetTutorialCity` returns true exactly when the export of the pristine tutorial city failed, and
  false when no city had the id or the reset worked. Is the polarity intended, and what does the nation's
  `ResetTutorialCities` do with it? [as read; the engine returns the same value]
- **RM9 - §8:** what do the two base heights (properties `0xCA383CAD` and `0xCA383CAE` of the region tuning
  exemplar), the two terrain types and the water minimum and maximum (60 and 100) mean, and what reads
  `GetBaseTerrainHeight` and `GetWaterPrefs`? What values do the shipped exemplars hold? [unknown; stored and
  returned only]
- **RM10 - §9:** which interface is IID `0xAB953253` (the terrain-map interface that `cSC4RegionalTerrainMap`
  answers)? What do the vtable slots 30 to 39 do, and what is the byte at `+0x24` that slots 3 and 4 set and clear?
  Is the table in §9.2 the complete behaviour a consumer can see? [the table is complete for slots 0 to 33]
- **RM11 - §10:** what do the 14 arguments of the packed render state of `cSC4RegionalTerrainView` mean (depth
  test, blending, culling), is the vertex colour `0xFF400000` read as ARGB, and is the rectangle visible in the
  region screen (it may only fill the depth buffer)? [flat dark red quad]
- **RM12 - §12 test 2:** applying the spec to the shipped Timbuktu folder moves `City - New City (21).sc4` out to
  `Regions\Downloads\` at the first load (it shares its cell with the Making Money tutorial city). Does a fresh
  install behave this way, in which order does the file system list the two names, and is the tutorial registry
  expected to rebuild the tutorial city from the game data instead (`ResetTutorialCity`)? [as read]
