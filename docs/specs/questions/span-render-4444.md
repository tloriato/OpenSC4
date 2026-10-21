# span-render-4444: open questions
<!-- Questions about [span-render-4444.md](../span-render-4444.md). Append new entries at the END; when one is answered, move it to answered/span-render-4444.md. Rules: README.md. -->

<!-- from: core-span4444-4 (2026-10-21) -->
- **SP1 - section 6 / section 4:** RenderAlphaGradient with an alpha ramp outside 0..255 gives `a = acc >> 27` in
  17..31 (or a wrapped value). The packed R|B register form `((sRB - dRB)*a + 16*dRB + 0x808) >> 4` can then borrow
  or carry between fields. What is the exact bit result for a > 16 (masking after the shift, field placement)?
  [core evaluates each channel separately in wrapping 32-bit maths and keeps its low nibble; alpha 0xF]
- **SP2 - section 2:** a span record whose `end - start` exceeds the pixels left in the row (or whose `end` is below
  `start`, wrapping to a huge width) writes linearly past the row end in the original. Do callers ever pass such
  records, or may a reimplementation require `x + width <= 65536` on one row? [core walks linearly from (x, y) and
  rejects any byte outside the supplied buffer before writing anything]
<!-- from: re-spanbridge4444-4 (2026-10-21) -->
- **SP4 - section 1.2 (type-7 producer; narrowed 2026-10-21 by re-spanbridge4444-4 round 2):** the static sweep in
  section 1.2 finds one shipped type-7 buffer, the screen buffer in a 16-bit screen mode, and it gets a SimGL context,
  never a `cGZDrawContextRenderToBuffer`. Colour type 1 stays 1 and is rejected. One question is still open and needs
  a runtime breakpoint on `0x008307A8` (the type-7 Init branch) in a scripted 16-bit session, or a hit count of 0 from
  it: does any `cIGZDrawSystem` slot-4 caller (`0x0082BAB0`) pass a buffer whose type came from data (a buffer
  resource v1/v2 header with type 7, or a Gimex load whose target type is not a literal)?
  [no native 4444 inputs captured; the core stays unit-tested only]
