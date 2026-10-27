# lighting: open questions
<!-- Questions about [lighting.md](../lighting.md). Append new entries at the END; when one is answered, move it to answered/lighting.md. Rules: README.md. -->

<!-- from: re-lighting (2026-10-19) -->
- **LT3 - §3, §4.5:** what the fade term does on screen. It grows across the city from one edge (which edge follows the
  view rotation), scales with `2^-zoom` and with the time-of-day fade strength (`0x29189BF5`, 0.15 at midday), and is
  only recomputed on a view change.

<!-- from: re-lighting (2026-10-19) -->
- **LT4 - §5.6:** the meaning of the decal shadow values `0x085489C9` (a float pair the shipped exemplar lacks) and
  `0x085489CA` (12.0), and of the shadow parameters `0x085489CC` (0.4) and `0x085489CD` (0.55). Their users draw
  decal shadows outside this lane.

  **PARTIAL 2026-10-27 (re-q-misc):** Established: `0x085489CC` and `0x085489CD` are the two factors of the alpha-test reference in the terrain overlay manager's shadow passes (reference = CC × CD, comparison greater-or-equal, enabled only when CD > 0; shipped 0.4 × 0.55 = 0.22; the colour is `0x085489CB`), see lighting.md §5.6 and overlay-manager.md §4.6. Still open: `0x085489C9` (the float pair) and `0x085489CA` (12.0) are read only by `GetDecalShadowParams` (slot 16), and no call of that slot was found in the executable, so their meaning cannot be derived statically.

<!-- from: core-lighting-complete4 (2026-10-22) -->
- **LT12 - §2.2, §5.7, §5.8.2 inherited numeric environment:** which precision-control and
  rounding-control modes reach these lighting entries in the normal game call paths? §5.8 says inherited
  control word, and vectors 19–22 hold at 53/64 significant bits, but does not establish the caller's
  mode or whether it changes between entries. [Portable core requires an explicit 24/53/64-bit register
  precision at construction; implemented arithmetic assumes nearest-even and masked exceptions. No
  implicit game-mode default or whole-owner numeric exactness is claimed for unproved CW modes.]

<!-- from: core-lighting-complete4 (2026-10-22) -->
- **LT13 - §5.8 NaN output encoding:** for NaN inputs and invalid operations which proceed through the
  unordered determinant path, are NaN payloads/signs or signaling-NaN exception flags part of the exact
  64-byte result contract? The arithmetic schedule establishes propagation/classification but does not
  specify those encodings. [Core returns true and propagates quiet NaN values without a finite fallback;
  its integer register primitive canonicalizes payloads. NaN bit-pattern equivalence is not claimed.]
