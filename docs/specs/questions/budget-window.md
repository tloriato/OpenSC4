# budget-window: open questions
<!-- Questions about [budget-window.md](../budget-window.md). Append new entries at the END; when one is answered, move it to answered/budget-window.md. Rules: README.md. -->

<!-- from: re-panels4 (2026-10-21) -->
- **BW1 - section 4 group mode:** groups `0x6A357B96`, `0xEA597195`, `0x0A5A72D1` are tested at `0x0078807C`-`0x00788092` in `on_init` and take another row layout. What differs (one slider per department vs one per item, per-building rows through `0x0ABCE6xx`)? [unknown; spec gives the common slider contract only]

<!-- from: re-panels4 (2026-10-21) -->
- **BW2 - section 5 button 0xCF:** what does `FUN_0077AB40` do before the dialog closes (neighbour mode: accept the selected deals)? [unknown]

<!-- from: re-panels4 (2026-10-21) -->
- **BW3 - section 5 occupant controls:** what do buttons `0x0ABCDE00`-`0x0ABCDEFF` do in occupant mode, and what are funding levels 0 and 1 of `SetOccupantFundingLevel` (`0x0077BC90`)? [unknown]

<!-- from: re-panels4 (2026-10-21) -->
- **BW4 - section 2.1 panel slot 6:** commands `0xAA3AC40E`/`0xAA3AC40F` (and `0x4A8879CB`/`0x4A8879DB`) set `+0x18C` to 1/0 and call the budget proc's slot 6 (`+0x18`). Which method is it (ShowWindow with expanded/compact)? [inferred: switch between the compact and expanded panel]

<!-- from: re-panels4 (2026-10-21) -->
- **BW5 - section 4 key close:** key messages `0x0D` and `0x73` close the dialog only when modifier bit `0x4` is set. Is `0x73` the virtual key F4 (Alt+F4) and bit `0x4` Alt, and does plain Enter really do nothing? [yes to F4 by key value; modifier meaning unverified, needs a game check]
