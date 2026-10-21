# Lane: re-netocc-contracts4

| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| re-netocc-contracts4 (answer NO15-NO17 network occupants) | `re-netocc-contracts4` / `.claude/worktrees/re-netocc-contracts4` (docs lane) | ready: NO15-NO17 answered in network-occupants.md §2.2/§2.3/§3.1-§3.5 | merge train: docs batch; core-netocc follow-up implements the record and SetTerrainHeights from the corrected sections |

## Report

All three questions were answered from the instructions (re_query.py asm, exe constants read via build-runs/peek.py); REA was not needed.
They are moved to docs/specs/questions/answered/network-occupants.md (new file) and QUESTIONS.md is reindexed.

- **Key finding:** `Write`/`Read` of the network, underground, pipe and subway records sit in the `+8` vtable with no
  thunk. Every record offset in the code is therefore the object offset minus 8. The old §2.2/§3.4 mixed the two
  coordinate systems, which explains most of the NO16/NO17 clashes. The rule is stated in §2.2 ("Offsets in the
  record"); `GetBoundingBox` (+4 view) and the ctor (+0 view) confirm it.
- **NO15 (DIFFERS):** the writer emits the transform's own mask byte. The reader resets to identity only when the
  live mask is non-zero, and clears bit 2 when scale == 1.0. LegacyLoad is 56 bytes (9+3+1 f32 + u32 counter byte).
  Worked bytes are in §2.3.
- **NO16 (DIFFERS):** the record holds one quad of 4 vertices {5 f32 + R,G,B,A}, then four scalars, then the cell-info
  sub-record (version mismatch ignored by the caller). Each triangle is u32 + 3 vertices = 76 bytes. The box is 6
  f32 with min/max interleaved. The full `ConvertV5FlagsToCurrentFlags` bit table is in §2.3. The v<7/v<8 read tail
  (properties 0x4835C6AB/0x4835C6AC) is specified. The ref and the int64 are read in all versions.
- **NO17 (DIFFERS):** the underground record is u16 3 + the full v8 base record + a 13-field tail (137 bytes with
  empty walls, worked example in §3.4). Wall list = u32 count + 24-byte vertices. The heights are the four f32 at
  `+0x24C`..`+0x258`. The pipe extra u32 is at object `+0x270`, and pipe/subway set byte `+0x260`.
  IsImaginaryConnection returns false when no neighbour is found and when its own bit 31 is clear. GetPosition is
  the 4-vertex average × 0.25 (`this = object+4`).
- Commits: 7836c03b (lane file), 302306a6 (NO15/NO16), 8bac43a4 (NO17), 774b4c12 (answers moved, index).
- Not done / noticed: §2.1 says the model instance is at `+0x68`, but the ctor builds `cS3DModelInstanceBasic` at
  `+0x38` (row added to §2.2; §2.1 not rewritten). §3.1's Shutdown wall-vector offsets (`+0x184`...) do not match
  the record's object offsets (`+0x180`...) and are not re-verified. Slot names of primary vtable `0x00AA8080` and
  underground `0x00AAAD40` are missing from docs/abi (abi rows not proposed).
- Next: core-netocc implements the base/underground records against §2.3/§3.4; NO14 is still open.
