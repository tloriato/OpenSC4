# command-system: open questions
<!-- Questions about [command-system.md](../command-system.md). Append new entries at the END; when one is answered, move it to answered/command-system.md. Rules: README.md. -->

<!-- from: re-commands (2026-10-19) -->
- **GC1 - §1.3, §3.3, §4.2, §4.5:** the names of the interfaces behind the second IIDs: `0x4B8C39E4` / `0x4B8C39E5`
  (parameter set), `0x0B90251E` (string generator), `0x0B903CC3` / `0x0B903CC4` (dispatcher), `0x4B99446A` /
  `0x0B903CC5` (command message). Only `0x4B99446A` is asked for in the executable (two view handlers). Do plugins
  ask for any of the others?

<!-- from: re-commands (2026-10-19) -->
- **GC2 - §1.3:** what is `ValidateParameter`'s second argument? Only `0xFF` gives true, whatever the value's type,
  and the function has no direct caller in the executable.

<!-- from: re-commands (2026-10-19) -->
- **GC3 - §4.4:** does any handler execute a command while it handles one (for example `ExecuteScript` or
  `ExecuteCheat` running text that issues commands), so that the shared message and default sets are reused during
  delivery? A probe that logs nested `ExecuteCommand` calls would settle it.

<!-- from: re-commands (2026-10-19) -->
- **GC4 - §1.5:** which status values do the handlers use besides 2? A survey of the `SetStatusParameterValue` calls
  in the handlers would give the vocabulary.

<!-- from: core-commands4 (2026-10-21) -->
- **GC5 - §1.4 `CompareParameterString` with ID −2:** how does the "any parameter" walk visit the parameters: the
  map itself (so a parameter under `0xFFFFFFFF` is compared) or slots 13/14 (which stop before it)? And a parameter
  stored under `0xFFFFFFFE` itself would recurse through slot 11 forever; does the original guard it? Same question
  for the "each parameter in ID order" walks of §3.6. [the −2 walk uses the map, every ID, and compares a parameter
  under `0xFFFFFFFE` directly; the §3.6 walks stop before `0xFFFFFFFF` as §1.4 describes]

<!-- from: core-commands4 (2026-10-21) -->
- **GC6 - §2.1 rule 4 / §2.2:** with a delimiter other than space, does a trailing delimiter (`a,`, or `a, ` with
  blanks after it) add a final empty word? Rule 4 says the text after the last break is the last word, but the
  space rule "trailing blanks add nothing" does not say whether an empty remainder is a word for other delimiters.
  [no: an empty remainder adds no word, so `a,` gives one word `a` and `a,,` gives `a` | ∅]

<!-- from: core-commands4 (2026-10-21) -->
- **GC7 - §3.6 / data-formats §5.1:** what text does `GetValString` give for a **VoidPtr scalar** (memory type
  `0x0F`) parameter? §5.1 covers the VoidPtr *array* (empty text) and objects, not the scalar. [no text: the
  getter returns false and leaves its output unchanged, so command-to-text writes an empty word]
