# dates: open questions
<!-- Questions about [dates.md](../dates.md). Append new entries at the END; when one is answered, move it to answered/dates.md. Rules: README.md. -->

<!-- from: re-dates (2026-10-19) -->
- **DT1 - §1.6:** are the format codes of `cRZDate::DateString` (`0x00912C79`) the language manager's language
  IDs (1 US English month/day/year, 2 UK English day/month/year, dots for 4, 10 and 11, dashes for 7, 8 and 14,
  year first for 9 and 15)? Both callers in the game pass 0, so only plugins see the other codes.

<!-- from: core-dates4 (2026-10-21) -->
- **DT2 - §1.5:** `Set(monthName, d, y)` compares "the first three characters, ignoring case". What happens for a
  name shorter than three characters (`"Ma"`, `""`) and for a null `monthName` pointer, and is case folded as
  ASCII only or by the C runtime's current locale? The engine (`engine/core/src/dates/mod.rs` `month_from_name`)
  reads it as a C `strnicmp(name, "Jan", 3)` in the C locale: a shorter name matches nothing (January), folding
  is ASCII only; a null pointer is left to the plugin glue (the core takes a byte slice).
- **DT3 - §4 slots 9 and 41:** what do `Clone(cIGZDate**)` and `DateString(cIGZString&, format)` return in EAX/AL?
  The SDK declares `bool`; the table gives no return. The engine answers true for both (`engine/core/src/dates/com.rs`).
  Does `Clone` also write `*out` when `out` is null, and does `DateString` replace or append to the string?
