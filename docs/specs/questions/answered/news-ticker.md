# news-ticker: answered questions (archive)
<!-- Archive for [news-ticker.md](../../news-ticker.md). Open questions are in ../news-ticker.md. -->

<!-- from: re-ticker7 (2026-10-23) -->
- ~~**NTK1 - §4, §5.1:** advice slot 7, which orders the list and gates the flipper (above 59). Is it the advice's
  priority (advisors.md)? [inferred]~~
  **ANSWERED 2026-10-27 (re-q-widget):** MATCHES — Advice slot 7 reads the priority loaded from the script priority property; it governs ordering and the flipper threshold (§4, §5.1; conventions).

<!-- from: re-ticker7 (2026-10-23) -->
- ~~**NTK2 - §3:** advice flag `0x2000` keeps an added advice out of the ticker. Which advices carry it? [unknown]~~
  **ANSWERED 2026-10-27 (re-q-widget):** MATCHES — City-situation advice sets 0x2000 during Init; the ticker excludes by the mutable flag rather than by class (§3).

<!-- from: re-ticker7 (2026-10-23) -->
- ~~**NTK3 - §5.3:** language 15 (`RZ::LM` slot 7) switches the flipper to the smaller `SIZE=2` face. Which language
  is 15? [inferred: an East Asian language]~~
  **ANSWERED 2026-10-27 (re-q-widget):** MATCHES — Language ID 15 is Japanese, selecting the SIZE=2 face (§5.3).

<!-- from: re-ticker7 (2026-10-23) -->
- ~~**NTK4 - §2:** the timer callback hands the `0x0A344006` message and the ticker to message server 2's slot 7.
  Is that a synchronous send to that target? [inferred]~~
  **ANSWERED 2026-10-27 (re-q-widget):** DIFFERS — Message-server slot 7 retains and queues a directed message/target pair; the queue pump delivers it later (§2).
