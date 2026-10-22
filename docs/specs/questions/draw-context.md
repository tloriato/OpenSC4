# draw-context: open questions
<!-- Questions about [draw-context.md](../draw-context.md). Append new entries at the END; when one is answered, move it to answered/draw-context.md. Rules: README.md. -->

<!-- from: re-drawcontext (2026-10-20) -->
- **DC1 - §1.1:** what record the driver's slot 64 fills for `Init` (the DX7 driver's name for the slot is
  `GetVideoModeInfo`), and what its bytes `+0x29` and `+0x2A`, kept at `+0x78` and `+0x79`, mean. Its dword
  `+0x24` is used as the texture-stage count. [the stage count as measured; the bytes unused here]

<!-- from: re-drawcontext (2026-10-20) -->
- **DC2 - §6, §9.2:** who builds a model instance's draw-flags record and what its bits 1-5, its second byte
  (the highlight colour) and its colour at `+0x04` and float at `+0x10` stand for in game terms (fade, grey,
  hidden-part outline and constant shade are inferred from use). [the effects as measured]
