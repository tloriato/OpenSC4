# cHitTrackPlayer — unresolved implementation questions

About `docs/specs/hit-track-player.md`. Filed 2026-10-22 by `re-hittrack4`.
No complete-owner credit: **88 / 110 bodies, 7963 / 16012 bytes specified;
22 bodies / 8049 bytes remain open**. Sizes are exported body bytes. Groups below
partition every open inventory row exactly once; the cleanup boundary is additional
external work and is not added to this owner’s byte count.

**HT1 — Interpreter bytecode and control flow.** Target: `hit-track-player.md` §6–7.

Specify every opcode reachable in `0x0093C229`: instruction/operand widths, signedness, register/key translation, cursor advance and branching, wait/active/error transitions, source and external-call ordering, malformed/truncated stream behavior, and return convention. The timer wrapper is specified; that does not specify DoCommand. Start from its own jump tables and operand fetch instructions, not another Hit owner.

Round 2 partial: spec §7.1 proves the decoder envelope and 11 of 97 in-range
opcode values: 09, 0B, 0C, 0D, 0F, 1D, 1E, 5D, 5E, 60, 61 (hex). All
other **86** values remain uncontracted, including arithmetic/comparison,
conditional branches, call/return-stack helper semantics, sound actions,
wait-comparison loops, Hit-list selection and callback dispatch. Full DoCommand
remains open at all 6754 bytes; no fractional byte/body credit is claimed for
these subcontracts. Continue from the jump table at 0x0093DC8B and the saved
opcode cursor, not the decompiler's pointer arithmetic (which confuses bytes
with int pointers). Only the selected branches' unchecked/truncated behavior
has been established, not the full malformed-stream contract.

Remaining owned bodies: **1 / 6754 bytes**:

| Entry | Bytes | Export name / local status |
|---|---:|---|
| `0x0093C229` | 6754 | `cHitTrackPlayer::DoCommand` |

**HT2 — Playback preparation and source admission.** Target: `hit-track-player.md` §4, §6, §8.

The seven owned playback/definition bodies are answered in spec §6.4–6.7 and
archived as HT2-owned. Remaining: reconcile the **non-owned** cleanup boundary
0x0093E60A used by the deleting destructor, including release order, dependent
removal and channel teardown. Its misleading cHitChannel label is not proof of
inheritance, and this lane adds no complete contract to that owner. The visitor
argument 0x456B8F1D is a GetTrackDef interface query, not an integer property key.

Remaining owned bodies in HT2: **0 / 0 bytes**; external cleanup remains open.

**HT3 — Property, register and spatial dispatch.** Target: `hit-track-player.md` §3, §5, §8.

Owned dispatch, notification and spatial bodies are specified in §5.2–5.3;
archived as HT3-owned. Remaining external encoding: publish the **80-entry**
short-register map at 0x00B169F8, indexed by helper 0x009377C9. Instructions
prove an unsigned input below 80 indexes that table, otherwise helper returns
zero; callers retain their original key for a zero translation. The complete
table contents are still needed by an implementer. `_ftol2` finite/high-bit and
masked-invalid spatial outcomes are now specified in §5.3, not an open guess.

Remaining owned bodies in HT3: **0 / 0 bytes**. HT4 is fully answered in the
archive and spec §5.4; no open HT4-owned entry remains.

**HT5 — Owned container and source-index support.** Target: `hit-track-player.md` §4–8.

For each listed body specify its local container operation, ownership changes, key comparison and duplicate rules, iterator invalidation, caller view and return value, including zero-copy/empty cases and callback removals. Several exported bodies have erased or misleading STL prototypes. Resolve allocation growth and overlap behavior for the reference-provider vector, source-list/index insertion and removal, static pool lifetime and teardown. Existing calls to these helpers in specified wrappers do not supply these contracts. Separately resolve shared exception continuation 0x0093E0F9, absent from the instruction export: the focused fallback query could not decode it because capstone is unavailable. This external continuation adds no body to the owned count.

Remaining owned bodies: **21 / 1295 bytes**:

| Entry | Bytes | Export name / local status |
|---|---:|---|
| `0x0093AAB5` | 29 | unnamed; contract unresolved |
| `0x0093AAD2` | 40 | unnamed; contract unresolved |
| `0x0093AAFA` | 47 | unnamed; contract unresolved |
| `0x0093AF7E` | 44 | unnamed; contract unresolved |
| `0x0093AFAA` | 29 | unnamed; contract unresolved |
| `0x0093AFC7` | 27 | unnamed; contract unresolved |
| `0x0093AFE2` | 12 | unnamed; contract unresolved |
| `0x0093B6CE` | 32 | unnamed; contract unresolved |
| `0x0093B6EE` | 136 | unnamed; contract unresolved |
| `0x0093B776` | 27 | unnamed; contract unresolved |
| `0x0093BB39` | 51 | unnamed; contract unresolved |
| `0x0093BB6C` | 132 | unnamed; contract unresolved |
| `0x0093C041` | 35 | unnamed; contract unresolved |
| `0x0093C064` | 53 | unnamed; contract unresolved |
| `0x0093C099` | 41 | unnamed; contract unresolved |
| `0x0093C0C2` | 42 | unnamed; contract unresolved |
| `0x0093DE80` | 25 | unnamed; contract unresolved |
| `0x0093DE99` | 179 | unnamed; contract unresolved |
| `0x0093E303` | 54 | unnamed; contract unresolved |
| `0x0093E339` | 22 | unnamed; contract unresolved |
| `0x0093E34F` | 238 | unnamed; contract unresolved |
