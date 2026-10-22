# map-view: open questions
<!-- Questions about [map-view.md](../map-view.md). Append new entries at the END; when one is answered, move it to answered/map-view.md. Rules: README.md. -->

<!-- from: re-mapview (2026-10-20) -->
- **QV1 - §2, §3.1:** the 3D view filter a data view installs: what `0x4A0B47E0` (occupant types), `0x4A0B47E1`,
  `0x4A0B47E2` and `0x4A0B47F0` change in the filter object (`0x0079FE60`, `0x0079FB90`), and what the terrain
  (slots 84, 86), zone manager (slots 17, 18) and `0x4A0B47E6` calls display. [as read]
- **QV2 - §4.1:** source 76 (traffic volume): which grid it draws for the kept travel types (`0x6B7C5CDD`) and how.
  [as read]
- **QV3 - §1, §3.2:** the panels `0x8A2871B1`-`0x8A2871C4`, `0xAA32BCD4`/`0xAA32BCD5`; the groups' remembered views
  behind the buttons `0x5005`, `0x5008` and `0x8002`; the toggle `+0x9AC` of `0x8100`/`0x8101`; the command
  `0x6A935CF1`. [as read]
- **QV4 - §4.1, §4.3:** the rule of `BlockMapByLot` (`0x0079FEF0`) and its argument `0x4A0B47E8`, the crime incident
  markers (`0x0079E600`), and the developer grid of layer 5 (`0x007A2080`). [as read]
