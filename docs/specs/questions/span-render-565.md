# span-render-565: open questions
<!-- Questions about [span-render-565.md](../span-render-565.md). Append new entries at the END; when one is answered, move it to answered/span-render-565.md. Rules: README.md. -->

<!-- from: re-span565-4 (2026-10-21) -->
- **SR565-1 - section 4.1 / 4.2:** does any shipped code path draw a gradient (`+0x74` set) into a colour-type 5
  buffer? `SetGradientTransform` (`0x0082FDB0`) and its base forwarder `0x0082C360` have no direct caller in the
  export. If a path exists, an in-game capture would show whether the B-channel drift of section 4.2 (start shifted
  right by 3, step with its low 3 bits dropped) is visible.

<!-- from: core-span565-4 (2026-10-21) -->
- **SR565-2 - section 7.3 (Modulate / ModulateAlpha, A1555 source):** the 8-bit expansion of the 1-bit alpha
  of an A1555 texel is not spelled out ("expand the texel to 8 bits per channel"). The core uses bit 15 clear → 0,
  set → 255 (`engine/core/src/span_render565/blit.rs`, `texel_to_8888`). Which value does the A1555 ModulateAlpha
  body (row 2/3, column 3/7/11) use for `A`?
- **SR565-3 - section 7 / 7.3 (ConstantAlpha, A-format rows):** rows 2, 3, 8, 9 have ConstantAlpha bodies of their
  own (columns 1, 5, 9: `0x0083E2B0`, `0x0083E360`, `0x0083E480`, ...) distinct from the X-format rows 4, 5, 10, 11,
  yet §7.3 gives `B(T, D, ca >> 3)` with no use of the texel alpha. Does the A1555 / A8888 ConstantAlpha weight
  involve the texel alpha (or a texel-alpha test), or do the bodies differ only in code? The core follows §7.3
  literally (texel alpha ignored).
- **SR565-4 - section 6.1 / 7.2 (X888 texel size):** the core reads an `f = 5` (colour type 8, R8G8B8) texel as
  4 bytes, inferred from rows 10/11 sharing every non-alpha body with A8888 and from the 32-bit key mask and
  example. Please confirm the bytes per texel the f = 5 bodies step by (plain `x·bpp`, stretch, tiled).
