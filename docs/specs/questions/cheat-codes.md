# cheat-codes: open questions
<!-- Questions about [cheat-codes.md](../cheat-codes.md). Append new entries at the END; when one is answered, move it to answered/cheat-codes.md. Rules: README.md. -->

<!-- from: re-gzcheats7 (2026-10-24) -->
- **CCM1 - §4:** `GetCheatCodeSaveData` writes the state-ID count as `4·e` (bytes) and `SetCheatCodeSaveData` reads it
  back as a count, inserting `4·e` dwords (the `e` IDs plus `3·e` dwords past the block). Which code saves and
  restores this block (a city or region record?), and what follows it in the buffer, so what the extra IDs are in
  practice? [static: callers of slots 11 and 12 (`+0x2C`, `+0x30`); a reimplementation should insert only the `e`
  saved IDs]
  **PARTIAL 2026-10-27:** City Write/Read (0x0045C5B0, 0x0045CB10) save/load a separate version-1 record with type 0x89D9B9EE, group 0x69D9B9F1, instance 0. The IDs end its payload, so the extra reads are beyond an independent buffer, not another specified city field (§4). Actual excess IDs depend on runtime allocation contents and remain unresolved.
