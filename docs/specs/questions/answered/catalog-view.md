# catalog-view: answered questions (archive)
<!-- Archive for [catalog-view.md](../../catalog-view.md). Open questions are in ../catalog-view.md. -->

<!-- from: re-catview7 (2026-10-23) -->
- ~~**CV1 - §2:** after every `SetScrollPos`, the parent window's interface `0x8A25CE2C` slot 7 gets
  `(pos > 0, pos < count − visible)`. Is that the tertiary menu frame enabling its up and down arrows? [inferred]~~
  **ANSWERED 2026-10-27 (re-q-widget):** MATCHES — The receiver is the tertiary menu frame; slot 7 stores up/down arrow availability and invalidates changed artwork (§2).
