# persistence: open questions
<!-- Questions about [persistence.md](../persistence.md). Append new entries at the END; when one is answered, move it to answered/persistence.md. Rules: README.md. -->

<!-- from: re-persistence4 (2026-10-21) -->
- **PR1:** (§3.1) which native-order flag do the DBPF packed-file records (`cGZDBRecordPackedFile` constructor
  `0x0096D859`, called from `0x0096D8D0`, `0x0096D9D1`, `0x0096DA30` and the two write-record constructors) receive
  from their segment, and can any segment in the shipped game be configured big-endian? [expected: always 1 for
  the shipped packages and saves, which are little-endian]

<!-- from: re-persistence4 (2026-10-21) -->
- **PR3:** (§6.2 v1) the original's padded-stride copy swaps the source and destination steps. FreeCity4 chose
  the correct copy (identical whenever stride = packed row size). Confirm no shipped data or caller depends on a
  padded v1 load. [the choice stands]

<!-- from: core-persist-runtime4 (2026-10-22) -->
- **PR9 - sections 2 and 4.2 constructor/failure domains:** what initial reference counts do DBRecord and ResourceFactory receive, and what happens to out parameters when allocation or QI fails? Are there defined overflow/reentrant-callback contracts? [Counts are explicit constructor inputs; no native constructor-default, overflow or unspecified failure-output claim. Portable factories preserve caller outputs unless QI succeeds; reentrant foreign callbacks are unsupported.]

<!-- from: core-persist-runtime4 (2026-10-22) -->
- **PR10 - section 3.4 malformed strings and short I/O:** what destination state and byte values are defined after failed header/payload/legacy byte reads, malformed UTF-16, or the second-byte-zero guess reading beyond the payload? [Return the published raw result; preserve only host-defined partial-byte effects for typed fields. Support complete valid UTF-16, never read outside an allocation; malformed/odd guessed input is an explicitly unsupported domain. No post-failure string-state/native allocation claim.]

<!-- from: core-persist-runtime4 (2026-10-22) -->
- **PR11 - sections 6.1/6.4 buffer validation/conversion:** are there native width/height/colour-type thresholds beyond the published bits table? What are BMP translation row direction, negative-height handling, palette index failure, and colour conversion/alpha semantics? [Use owned packed storage with checked arithmetic, caller-supplied pixel conversion primitives, positive-dimension constructed fixtures and bottom-up BMP row copies as an explicit FreeCity4 choice. Reject unsafe dimensions/unsupported conversions without claiming native-exact failure effects; no guessed RGB565/ARGB semantics.]

<!-- from: core-persist-runtime4 (2026-10-22) -->
- **PR12 - sections 6.2/6.3 raw failures and size domains:** what are v1 dataSize mismatch effects, failed v0 array-read pixel effects, lock/init/convert failure cleanup, and the exact QFS stream framing used by this resource? [Use existing formats QFS/RefPack codecs; require safely sized packed pixels; unlock every successful lock; preserve documented return quirks. Constructed DBPF-header QFS vectors establish this supported framing only. Malformed native memory effects and hidden allocation paths remain unverified.]

<!-- from: core-persist-runtime4 (2026-10-22) -->
- **PR13 - section 3 record versus stream error state:** foundations section 2.3 describes a sticky stream error on invalid lengths, while the published serial-record primitive contract supplies raw boolean read/write/seek operations and no error setter/state field. Do invalid string lengths, unsupported variant tags and failed typed operations also change shared input/output adapter error state, and through which published operation? Do file-wrapper close failures override successful load/save results? [Portable typed fields return the documented raw/typed booleans without inventing a shared sticky-error field; full native stream-vtable error behavior remains unsupported. File wrappers retain load/save results after closing; no native-exact close-failure result claim.]
