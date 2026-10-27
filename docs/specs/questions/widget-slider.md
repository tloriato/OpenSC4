# widget-slider: open questions
<!-- Questions about [widget-slider.md](../widget-slider.md). Append new entries at the END; when one is answered, move it to answered/widget-slider.md. Rules: README.md. -->

<!-- from: re-gzslider7 (2026-10-24) -->
- **WSL1 - §3.1:** `cIGZWinScrollbar` slot 19 fills `(3·tw, th, 0xFFFF, th)` (horizontal) or `(tw, 3·th, tw,
  0xFFFF)`, read here as minimum and maximum sizes, and slot 17 takes `(rect, image, x)` with `x` unused. Which
  callers use slots 17–19, and with what third argument? [inferred; static: callers of the interface at `+0x4C`,
  `+0x48`, `+0x44`]
  **PARTIAL 2026-10-27:** Verified slot-17 callers in the text editor (0x009BD278, 0x009BD2F9, 0x009BE477) pass zero as the third argument. The callee discards it and slot 18 writes zero (0x009C3FC6, 0x009C3FD9); §3.1 records this. A verified caller chain for slots 18/19 and the size-limit interpretation remains unresolved.

<!-- from: re-gzslider7 (2026-10-24) -->
- **WSL2 - §5:** a track of length 0 (thumb and arrows filling the bar, e.g. the default 32×96 old scroll bar) turns
  a thumb drag into a NaN fraction and a value of `min + 0x80000000`. Does any shipped window create a slider or old
  scroll bar that small, or one with `max = min`? [game: drag such a thumb and read the value]
