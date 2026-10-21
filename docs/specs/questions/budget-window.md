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

<!-- from: core-budgetwindow4 (2026-10-21) -->
- **BW6 - sections 2, 3, 6 references and pause balance:** (a) `CreateSubBudgetWindow` AddRefs the `*out` window and also holds it at `+0x0C`: who releases each (the panel after step 5, `DestroySubBudgetWindow`, the deleting dtor)? (b) `DestroySubBudgetWindow` calls `HiddenResume` "in every mode": also when `CreateSubBudgetWindow` failed and `on_init` (the `HiddenPause`) never ran, or is the resume guarded? [core implements: no Release of either; resume always]

<!-- from: core-budgetwindow4 (2026-10-21) -->
- **BW7 - section 5 scrolling and the button tail:** (a) the upper bound of a list position (core: `len - 9`, 0 when the list fits), the length field of list A (B is `+0x128`), and which mode fills which list; (b) the arrow rule "hidden at the ends" (core: up hidden at 0, down hidden at the upper bound, set after each scroll only); (c) which buttons take the "mode refresh + `UpdateValues`" tail (core: every button not returning 1, unlisted IDs included, and `0xCD`/`0xCE`/`0xCF` after their `end_dialog`). [literal readings]

<!-- from: core-budgetwindow4 (2026-10-21) -->
- **BW8 - sections 4.4, 5.1 numeric types:** the slider position type and the precision of `position x 0.01` (float or double constant, x87 product rounded once to float?); the type of `SetOccupantFundingLevel`'s level and of `IndividualRestoreInfo` level0/level1; when the restore records (`+0x44`) are filled. [core: double product stored as f32; levels i32 as the slider position; restore records from initIndividualBudget]

<!-- from: core-budgetwindow4 (2026-10-21) -->
- **BW9 - sections 4, 5 unspecified routing:** what `on_shutdown` (`0x0077E440`) does; `DoWinMsg`'s return value for init, shutdown, slider, combo and ignored keys; the meaning of the second argument `0x42B7C353` of `end_dialog` on the Esc path (section 6 gives one argument); which code creates the inner child at `+0x14`; the ordinance row check IDs that toggle the edited byte; the arguments of `SetOrdinanceOn` in `0xCD` (core: ordinance, edited state). [unknown; core takes child and toggles as typed host input and returns "unspecified"]
