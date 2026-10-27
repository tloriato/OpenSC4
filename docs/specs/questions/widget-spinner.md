# widget-spinner: open questions
<!-- Questions about [widget-spinner.md](../widget-spinner.md). Append new entries at the END; when one is answered, move it to answered/widget-spinner.md. Rules: README.md. -->

<!-- from: re-gzspinner7 (2026-10-24) -->
- **WSP1 - §1:** the flat buttons are drawn with the colour at `+0x16C`, which the constructor and every spinner
  method leave unwritten. Does any shipped spinner run without a button image (only possible when the control
  manager is missing at `Init`), and what colour does it show? [static: whether `GlobalHeapAlloc` zeroes; until then
  a reimplementation should start it at 0]
  **PARTIAL 2026-10-27:** GlobalHeapAlloc does not clear the reused 512-byte block (0x005E55E0, 0x005B9FC0, 0x005E16E0, 0x005E5BC0). Thus +0x16C has no guaranteed zero colour. Which shipped windows exercise flat painting, and the resulting heap-dependent colour, remain unresolved; §1 now distinguishes this from the zero-initialisation choice.

<!-- from: re-gzspinner7 (2026-10-24) -->
- **WSP2 - §2.1:** `ClearFontColor(i)` stores `NativeToPacked(0xDEADBEEF)` (alpha forced to `FF`), so the colour is
  never unset again. Which colour results with the game's native buffer format, and does any caller use slot 26?
  [static: callers of interface offset `+0x68`; game: clear a spinner's text colour]
