# widget-lineinput: answered questions (archive)
<!-- Archive for [widget-lineinput.md](../../widget-lineinput.md). Open questions are in ../widget-lineinput.md. -->

<!-- from: re-gzwinli7 (2026-10-23) -->
- ~~**WLI2 - §6:** the constructor never writes the sound IDs `+0x144`…`+0x14C`, so `PlaySound` reads whatever the
  global heap left there unless a script or caller sets them. Does `GlobalHeapAlloc` return zeroed memory? [static:
  the allocator; until then a reimplementation should start them at 0]~~
  **ANSWERED 2026-10-27 (re-q-widget):** MATCHES — The allocator does not clear reused blocks; unwritten sound IDs retain allocation contents. Zero initialisation is an explicit replacement choice (§6).
