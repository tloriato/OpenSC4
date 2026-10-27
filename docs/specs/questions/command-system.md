# command-system: open questions
<!-- Questions about [command-system.md](../command-system.md). Append new entries at the END; when one is answered, move it to answered/command-system.md. Rules: README.md. -->

<!-- from: re-commands (2026-10-19) -->
- **GC1 - §1.3, §3.3, §4.2, §4.5:** the names of the interfaces behind the second IIDs: `0x4B8C39E4` / `0x4B8C39E5`
  (parameter set), `0x0B90251E` (string generator), `0x0B903CC3` / `0x0B903CC4` (dispatcher), `0x4B99446A` /
  `0x0B903CC5` (command message). Only `0x4B99446A` is asked for in the executable (two view handlers). Do plugins
  ask for any of the others?
  **PARTIAL 2026-10-27:** Rechecked QueryInterface bodies 0x00929676, 0x009296F5, 0x009297DE and 0x009297B0: each listed IID pair returns the same corresponding interface view and AddRefs it. This does not recover the public interface names or determine which IIDs external plugins request.

<!-- from: re-commands (2026-10-19) -->
- **GC2 - §1.3:** what is `ValidateParameter`'s second argument? Only `0xFF` gives true, whatever the value's type,
  and the function has no direct caller in the executable.

<!-- from: re-commands (2026-10-19) -->
- **GC3 - §4.4:** does any handler execute a command while it handles one (for example `ExecuteScript` or
  `ExecuteCheat` running text that issues commands), so that the shared message and default sets are reused during
  delivery? A probe that logs nested `ExecuteCommand` calls would settle it.

<!-- from: core-commands4 (2026-10-21) -->
- **GC6 - §2.1 rule 4 / §2.2:** with a delimiter other than space, does a trailing delimiter (`a,`, or `a, ` with
  blanks after it) add a final empty word? Rule 4 says the text after the last break is the last word, but the
  space rule "trailing blanks add nothing" does not say whether an empty remainder is a word for other delimiters.
  [no: an empty remainder adds no word, so `a,` gives one word `a` and `a,,` gives `a` | ∅]

<!-- from: core-commands4 (2026-10-21) -->
- **GC7 - §3.6 / data-formats §5.1:** what text does `GetValString` give for a **VoidPtr scalar** (memory type
  `0x0F`) parameter? §5.1 covers the VoidPtr *array* (empty text) and objects, not the scalar. [no text: the
  getter returns false and leaves its output unchanged, so command-to-text writes an empty word]

<!-- from: verify-command-system4 (2026-10-22) -->
- **GC8 - §1 typed parameter observations / data-formats §5.2:** the independently owned ORIGINAL-set fixture
  `commands4-native-r1-e50c5bbc-a` made 7,414 comparisons against actual persistent production command-core
  sets, with 171 differences and zero refusals. The first twelve differences are variant `GetCount()` metadata
  on scalar parameters: original 0, `Variant::count()` 1. Data-formats §5.2 explicitly says scalar GetCount is 0,
  while the production `Variant::count()` documentation says 1. Is that Rust method intentionally a logical
  element count requiring a separate native-interface adapter, or should its contract be corrected? Preserve
  the measured discrepancy; no conversion or command semantics have been fitted. The probe also reports
  aggregate element-count versus other differences so later differences cannot be hidden by its log cap.
