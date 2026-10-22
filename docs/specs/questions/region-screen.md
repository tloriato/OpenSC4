# region-screen: open questions
<!-- Questions about [region-screen.md](../region-screen.md). Append new entries at the END; when one is answered, move it to answered/region-screen.md. Rules: README.md. -->

<!-- from: re-regionscreen (2026-10-20) -->
- **QR1 - §1, §2:** the window construction: what `InitApplicationServices` (`0x007ACC90`) starts and
  `ShutdownApplicationServices` (`0x007AC380`) stops, the windows `LoadGUI` (`0x007B0470`) builds, and what the
  tuning values `0xCA383CA3`-`0xCA383CB0` other than the clouds' opacity control. [as read]
- **QR2 - §6.5:** the import path after `SC4DisplayImportCityDialog`: the size check against the tile, which region
  slots place the read record, and what happens to the city that was on the tile. [as read]
- **QR3 - §5:** the buttons `0x4A779A09`, `0x4A779A1A`, `0x4BB92C1F`, `0xA98F4F88`, `0xCA5CFEE2`, `0xEBB91356`, the
  panels of `ShowPanel` (`0x007ABDF0`), and what `MakeWholeRegion` (`0x007AEDD0`, command `0x6A9757C2`) builds.
  [as read]
