# port-developers: open questions
<!-- Questions about [port-developers.md](../port-developers.md). Append new entries at the END; when one is answered, move it to answered/port-developers.md. Rules: README.md. -->

<!-- from: glue-portdev4 (2026-10-21) -->
- **PD12 - section 4.4 step 3 (seaport without sea access):** which interface lists "its props" (the lot's
  occupants through the lot manager or the lot, the prop manager by the lot's rectangle, or the lot configuration's
  objects), where property `0xC98204B8` is read (the prop occupant's property holder or its exemplar), and which
  `cISC4Prop` slot `SetState(1)` is (number and calling convention)? What does the host call return (the number of
  props switched)? [not wired: the glue logs it and returns 0; the advisor event `0x4A490D33` is pushed.
  Seen in game, portdev4-shd-1: a seaport built at cells (0, 90) of Konradshohe has no sea access in month
  2026-03-01, and the core asked for the switch once]
