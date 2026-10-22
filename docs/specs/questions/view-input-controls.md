# view-input-controls: open questions
<!-- Questions about [view-input-controls.md](../view-input-controls.md). Append new entries at the END; when one is answered, move it to answered/view-input-controls.md. Rules: README.md. -->

<!-- from: re-viewtools (2026-10-20) -->
- **VIC1 - §3, §4.1, §5.1, §6:** which commands of the city view's `ProcessCommand` (`0x007F1C10`) make the label,
  sign and sign-bulldozer tools, which buttons send them, and what the other cases of `RunGlobalTerrainTool`
  (`0x007E75B0`, commands 0–6 beside levelling's 7) do. [tools specified from their own code; makers named only]
- **VIC2 - §5.1, §5.4:** what message `0x0BE10A20` is (the sign tool answers it with a screen point while typing)
  and what framework service `0x27EBFFFD` (interface `0x07EC0010`, slot 18 reads text) is. [read as an input-method
  position request and the clipboard]
