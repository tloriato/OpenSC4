# network-occupants: answered questions (archive)
<!-- Archive for [network-occupants.md](../../network-occupants.md). Lanes do not read this file. Open questions are in ../network-occupants.md. -->

<!-- from: re-netocc-contracts4 (2026-10-21) -->
- **NO15 - the `S3DTransform` stream forms:** §2.3 field 3 writes `operator<<` (`0x005DA8C0`); lot-records.md §2.1
  gives its form (u8 mask; `& 4` 9 floats, `& 1` 3 floats, `& 2` 1 float, in that order). Which mask does the
  writer emit for a given live transform (the `+0x01` flags byte of the 0x38-byte object, a computed mask, or
  always 7), and what does the reader leave in a part the mask omits? And what is the byte layout of
  `S3DTransformLegacyLoad` (`0x005DA930`, records below v5; effects.md's component placement row calls it "56 bytes")? The core keeps
  the mask as data, leaves omitted parts at identity, and fails v4 records. [missing]
  **ANSWERED 2026-10-21 (re-netocc-contracts4):** DIFFERS — network-occupants.md §2.3 field 3 and "Transform read rules": the writer emits the live mask byte (+0x65 of the object, +1 of the transform) unchanged; the reader resets to identity only when the live mask is non-zero, ORs each present bit, clears bit 2 when the scale read is exactly 1.0; omitted parts keep their prior (identity after reset) values. LegacyLoad (v4) is 56 bytes: 9 f32 rotation, 3 f32 translation, 1 f32 scale, u32 whose low byte replaces the counter; mask bits are ORed, never reset.
- **NO16 - the §2.3 record's field shapes and v5-v7 conversions:** (a) field 4 says 4 x {5 u32, 4 u8} from
  `+0x94` but notes the loop covers only `+0x94...+0xE8`, then field 5 writes `+0xF4/+0xFD/+0xF8/+0xFC`: is the
  quad loop 3 or 4 iterations, and which quad dword is the fifth u32? (b) field 8's type column gives
  u32 + 3 f32 + 3 f32 + u32 + u32 + 4 u8 (40 bytes), its notes name u32 + 3 floats + u32 + u32 + 4 bytes
  (28 bytes), and the stride is 0x4C: which is the triangle entry? (c) field 9 names three u32 pairs at
  `+0x11C...+0x130` but §2.2 puts the box at `+0x120...+0x134`. (d) the rule of
  `ConvertV5FlagsToCurrentFlags` (`0x006191A0`, v < 6). (e) which "two floats it just read" the v >= 7 tail
  passes to slot 0x120, and what v < 7 with `+0x15C == 0` does. The core keeps the record in stream order
  (4 quads, type-column triangles), masks without converting, and reports the tail decision only. [ambiguous]
  **ANSWERED 2026-10-21 (re-netocc-contracts4):** DIFFERS — §2.2/§2.3: Write/Read run with this = object+8, so record offsets are object-8. (a) the loop is 4 iterations over ONE quad of 4 vertices {f32 x,y,z,u,v; bytes R,G,B,A}; field 5 is four separate scalars u32 +0xFC, u8 +0x105, u32 +0x100, u8 +0x104. (b) triangle = u32 + 3 such vertices = 76 stream bytes (0x4C in memory). (c) box = 6 f32, min/max interleaved per axis, object +0x124..+0x138. (d) bit map table in §2.3. (e) v<7 with +0x164 != 0 reads two extra f32 offsets, derives a cell, sets property 0x4835C6AB = (z<<16)+x via slot 72; every v<8 with +0x164 != 0 adds property 0x4835C6AC; the ref and int64 are read in all versions.
- **NO17 - the underground record and heights:** (a) §3.4 says the underground record is "the base network
  record (§2.3) with version 3": does the underground `Write` emit its own u16 3 and then the v8 base record,
  or the base record with version 3? (b) its offsets (u8 `+0x48`, u8 `+0x49`, u32 `+0x58`, ..., u32 `+0x260`)
  clash with §2.2/§3.1 (`+0x48` is the u32 lifecycle word, `+0x68` the model instance): which object are they
  relative to? (c) the byte layout of the wall-quad helper `0x0065B070` (six/seven calls in §3.4/§3.6).
  (d) `SetTerrainHeights(const float[4])` writes `+0x248...+0x258` (five floats) while `GetTerrainHeights` reads
  `+0x24C...+0x258` and `GetPosition` takes the minimum of `+0x248...+0x254`: which four floats are the heights?
  And pipe/subway `Read` set "`+0x258 = 1`" (a float height slot) where §3.4 sets bytes `+0x259`/`+0x25A`.
  (e) `IsImaginaryConnection` with no neighbour found. (f) `GetPosition` sums five points' x/z without dividing:
  confirm (not an average). The core implements the stated parts and leaves the record and
  `SetTerrainHeights` unimplemented. [ambiguous]
  **ANSWERED 2026-10-21 (re-netocc-contracts4):** DIFFERS — §3.1/§3.2/§3.4: (a) own u16 3 then the full v8 base record (own u16 8), then a 13-field tail; (b) offsets are object-8 (code this = object+8); (c) wall list = u32 count + count x 24-byte vertex {5 f32, R,G,B,A}; (d) the heights are the four f32 +0x24C..+0x258 (Set writes exactly these; GetPosition uses this = object+4); pipe/subway set the byte object +0x260, underground Read +0x261/+0x262; (e) no neighbour -> false, and a piece without flag bit 31 -> false; (f) x/z are the 4-vertex average (x 0.25), not a sum of five.
