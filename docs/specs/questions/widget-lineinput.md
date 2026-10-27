# widget-lineinput: open questions
<!-- Questions about [widget-lineinput.md](../widget-lineinput.md). Append new entries at the END; when one is answered, move it to answered/widget-lineinput.md. Rules: README.md. -->

<!-- from: re-gzwinli7 (2026-10-23) -->
- **WLI1 - §5.2:** Backspace calls the 1-based `DeleteChar(caret − n, n)`, which by the instructions erases the
  bytes one position too early and nothing at caret `n`. Does any shipped window use an editable `cGZWinLineInput`
  where this shows (type `abc`, Backspace: `ac`?), or do the game's text fields all use `cGZWinTextEdit`?
  [game: find a line input in a dialog and press Backspace]
