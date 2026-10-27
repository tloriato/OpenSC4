# callout-box: answered questions (archive)
<!-- Archive for [callout-box.md](../../callout-box.md). Open questions are in ../callout-box.md. -->

<!-- from: re-callout7 (2026-10-23) -->
- ~~**WCB1 - §3.1:** the font slots used to lay out and draw the texts: 45 `(text, length, &rect, 0, 250, maxW)`
  (measure in a box), 55 and 56 (begin and end drawing on a surface) and 52 `(surface, &rect, text, length, 0,
  colour, 0x8000000, 0, 0)`. What are the 250 and the `0x8000000` flag (word wrap)? [inferred]~~
  **ANSWERED 2026-10-27 (re-q-widget):** DIFFERS — 250 is the maximum text width, the bounds-derived argument is maximum height, and 0x08000000 is a background-colour sentinel; paragraph drawing supplies wrapping (§3.1).

<!-- from: re-callout7 (2026-10-23) -->
- ~~**WCB2 - §3.4:** the drawing offset is minus the parent's slot-50 point, while the callout window is usually
  given its parent's whole area. Is slot 50 the window's position in its parent (so the box is drawn where the
  parent-relative rectangles say)? [inferred]~~
  **ANSWERED 2026-10-27 (re-q-widget):** DIFFERS — Slot 50 returns the absolute area; subtracting its left/top translates screen rectangles into the parent drawing coordinates (§3.4).

<!-- from: re-callout7 (2026-10-23) -->
- ~~**WCB3 - §3.1:** the text surface is locked with `0x8010` before the text is drawn and is not unlocked in this
  function. Do the font slots 55/56 release that lock, or does the surface stay locked until its next use?
  [unknown]~~
  **ANSWERED 2026-10-27 (re-q-widget):** MATCHES — Font-use entry/exit balance their own 0x8080 surface access and leave the earlier callout 0x8010 nesting level outstanding (§3.1).
