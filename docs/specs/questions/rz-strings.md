# Framework strings: open questions

Questions about [rz-strings.md](../rz-strings.md). Filed 2026-10-23 by re-strings5.

- **RZS3 — §§1–3, mutation boundaries:** Determine alias-overlap guarantees for setters/append/insert/replace, allocation-failure outcomes and maximal byte counts; distinguish normal range errors from crash/termination and adapter policy. [Ordinary independent-range edits settled; no recoverable false return inferred.]
- **RZS5 — §5, Sprintf edges:** Verify signed negative/extreme integers and exact sign/prefix order, negative dynamic width/precision, unsupported/wide conversions, null strings, malformed/trailing percent, embedded-NUL %c and %n destination/value semantics. Verify glyph counting and precision for invalid encodings. [Literal/%%/%s and positive unsigned/padding vectors settled; no complete host printf equivalence claim.]

<!-- from: core-rzstrings5b (2026-10-23) -->
- **RZS7 — §5.1, reproducible finite decimal intermediate:** Specify the 12-byte extended representation (significand width, exponent and rounding), scaling constants, operation order and rounding points used to generate the 17 digits and guard. Section 5.1 says legacy extended scaling can affect the last digit and expressly excludes exact/arbitrary-precision binary64 conversion as an equivalent, but does not define an algorithm that distinguishes the two. Include boundary witnesses where extended scaling differs from exact binary64-to-17-digit half-up rounding, plus smallest subnormal/largest finite results. [Core implements all post-intermediate rounding/assembly, integer/pointer rules, zero and special markers; finite nonzero decimal extraction remains an explicit LegacyDecimal provider. The 20 RZS6 vectors are covered, using an exact-rational test provider for their finite inputs; this does not claim parity for unvectored legacy intermediates.]
<!-- from: glue-rzstrings-shadow5 (2026-10-24); RZS7 reserved by concurrent core-rzstrings5b -->
