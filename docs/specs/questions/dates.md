# dates: open questions
<!-- Questions about [dates.md](../dates.md). Append new entries at the END; when one is answered, move it to answered/dates.md. Rules: README.md. -->

<!-- from: re-dates (2026-10-19) -->
- **DT1 - §1.6:** are the format codes of `cRZDate::DateString` (`0x00912C79`) the language manager's language
  IDs (1 US English month/day/year, 2 UK English day/month/year, dots for 4, 10 and 11, dashes for 7, 8 and 14,
  year first for 9 and 15)? Both callers in the game pass 0, so only plugins see the other codes.
  **PARTIAL 2026-10-27:** The formatter maps 0 to 1 locally and never queries current language/locale (0x00912C7C–0x00912CF3); the COM wrapper forwards the numeric code (0x00913245–0x00913260). The name table at 0x00924798 supports the existing language-number inference but does not prove shared enum identity. §1.6 now states the independently verified behavior; identity remains unresolved.

- **DT4 — executable locale input (glue-dates4, 2026-10-22):** §1.5 establishes
  executable-CRT LC_CTYPE folding but no supported public acquisition of its
  256 byte-to-integer lowercase map. Is one available through the existing
  game interfaces? Rust now accepts a captured map explicitly; cFC4Date retains
  an explicitly marked C-locale branch pending that host input. Plugin CRT
  setlocale cannot establish the executable's locale. No arbitrary-locale
  compatibility or replacement credit is claimed.
- **DT5 — SetTime host input (glue-dates4, 2026-10-22):** §2/§4 slot 5 needs the
  executable CRT _timezone and _daylight. Is there a supported public way to
  obtain them? cFC4Date preserves its unimplemented SetTime path, logs REFUSED,
  leaves the date unchanged and returns 0. Other date slots do not use plugin
  CRT timezone as a substitute. Full-owner compatibility remains parked.
