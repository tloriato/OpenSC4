# language-utility: open questions
<!-- Questions about [language-utility.md](../language-utility.md). Append new entries at the END; when one is answered, move it to answered/language-utility.md. Rules: README.md. -->

<!-- from: core-language4 (2026-10-21) -->
- **LU1 - §4.7 year text:** "decimal year padded to at least 2 digits": is the year signed or unsigned, and for a negative year is it C `%.2d` (`-05`), `%02d` (`-5`) or `%.2u` (a large unsigned number)? [signed `%.2d`, `-05`]
<!-- from: core-language4 (2026-10-21) -->
- **LU2 - §4.8 case conversion:** when the length of `out` on entry is larger than the ANSI-converted text, which bytes are mapped past the text and what comes back (stop at the NUL, or the full length)? And what does slots 20/21 return? [map min(entry length, text length) bytes, return the whole converted text, return true]
<!-- from: core-language4 (2026-10-21) -->
- **LU3 - §1 slot 4:** what does the shared "shutdown" body return and does it touch the utility's state? [returns true, no state change]
