# span-render-1555: open questions
<!-- Questions about [span-render-1555.md](../span-render-1555.md). Append new entries at the END; when one is answered, move it to answered/span-render-1555.md. Rules: README.md. -->

<!-- from: re-span1555-4 (2026-10-21) -->
- **SR1555-2 - section 4.2:** does any shipped code path draw a gradient (`+0x74` set, `+0x41` clear) into a
  colour-type 4 or 6 buffer? RenderGradient takes B from the low 5 bits of the 8-bit value and does not scale the
  G/R start values, unlike the 565 sibling `0x0083DEA0`; if a path exists, an in-game capture of such a gradient
  would confirm the quirk is visible.

<!-- from: core-span1555-4 (2026-10-21) -->
- **SR1555-5 - sections 4.1 and 7.3/7.4:** two points the spec leaves implicit. (a) A4444 (colour type 7): the
  spec fixes A in bits 12-15 (key mask `0x0FFF`, "A = bit 15") but not the order of the other nibbles; the
  core assumes R bits 8-11, G 4-7, B 0-3 (A-R-G-B like A8R8G8B8). Please confirm. (b) SetGradientTransform's
  `c_i = trunc(s_i - (dx_i*x0 + dy_i*y0))` on the x87 stack: under which precision control (the game's 24-bit
  one of foundations section 18, or 53/64-bit)? The core rounds every operation to 24 bits; with 24-bit
  precision the subtraction loses low bits of `c_i` once `|s_i|` exceeds 2^24.
