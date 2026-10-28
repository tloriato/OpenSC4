# network-geometry: answered questions (archive)
<!-- Archive for [network-geometry.md](../../network-geometry.md). Lanes do not read this file. Open questions are in ../network-geometry.md. -->

## From the network geometry spec (re-specs15b lane, 2026-10-15)

Open points from network-geometry.md §17. The current answer is in brackets; each entry says what would settle it.

<!-- from: From the network geometry spec (2026-10-15) -->
- ~~**NG1:** the satisfier's tie-break between vertices with equally narrow ranges follows the hash-map iteration order
  of vertex ids. [reproduce the SGI `hash_map<int>` bucket order; comparing one hill crossing in both engines shows
  whether ties occur]~~ **ANSWERED 2026-10-16 (re-answers19):** MATCHES, with a correction (network-geometry.md
  §9.1). The pick (`0x0065ED80`) scans the unsolved hash map at `+0x28`; a width strictly below the best wins, so the
  first of equal widths is kept; a negative width fails; the scan stops at a best width of 0. The "already pinned
  vertex first" step of the spec does not exist in effect: the stack at `+0x50` it pops is never pushed. The order:
  identity hash, bucket `id mod n`, 193 buckets at first (the constructor asks 100, `0x00660810`; prime list
  `0x00A829B0`), head insertion after `resize(count + 1)` (`0x004A29B0`), the SGI resize (`0x004F0940`), erase on
  pin (`0x00660130`), and `clear` keeps the bucket count (`0x008EBA60`), so it only grows over the tool's life.
  Ties are common (every unnarrowed vertex has the same width).

<!-- from: From the network geometry spec (2026-10-15) -->
- ~~**NG2:** the bridge internals: the step and edge tests, `AdjustBridgeSpan`, `FindBestBridgeType`,
  `SelectBridgeElevation`, specialised and parametric bridge models. [not specified]~~ **PARTLY ANSWERED 2026-10-16
  (re-answers19):** MATCHES (still not specified; too large for a section). network-geometry.md §7 now gives the
  bridge record (`+0xD8`, `0x28` bytes) and corrects the length map: bridge type `+0x16C`, keyed by the step count
  `end − start + 1` (`0x00632A68`), not `+0x164`. §7.1 lists the entry points for a follow-up bridge spec (span
  selection `0x00630570`, step test `0x0062FDB0`, edge test `0x0062EB80`, elevation `0x0062EA90`, specialised
  `0x00632060`, parametric `0x00631300`, the piece inserters and the loaders). **ANSWERED 2026-10-16 (re-specs17a):**
  MATCHES the outline, now specified in network-bridges.md: the INI sections and the `0x178`-byte bridge type record
  with every exemplar property (§2), the specialisation grammar (§3), the water pass, span selection, bank adjustment
  and type choice (§4–§6), the elevations (§8), the specialised and parametric models (§9, §10), pieces and columns
  (§11), the cost (§12). Corrections: the straight test reads `v` (§4.1); the parametric model reads bridge type
  `+0x34`–`+0x7C` and `+0xA8`–`+0xD4`, not `+0xF8`–`+0x120`; the bridge cost fields are bridge **type** fields, not
  record fields (network-tools.md §8.1 fixed). New open questions NB1–NB3 (network-bridges.md §16).

<!-- from: From the network geometry spec (2026-10-15) -->
- ~~**NG3:** who fills the tool's extra-cell list (`+0xF0`), and whether `CheckForGoodTerrain`'s inverted extra-cell
  test can be reached. [empty in an ordinary drag, inferred]~~ **ANSWERED 2026-10-16 (re-answers19):** MATCHES
  (network-geometry.md §4.5). Only `0x00629170` appends (20-byte entries: x, z, edge word, network type, two bytes),
  called only by `cIntRule::DoAutoComplete` (`0x006097E0`), which only the intersection tool calls (`OnMouseDownL`
  `0x00661078`, `OnMouseMove` `0x00661657`), between two tool resets around `InsertIsolatedHighwayIntersection`
  (`0x0062CD50`). That function runs slot 68 but never slot 69, so the inverted test is unreachable in the shipped
  game.

<!-- from: From the network geometry spec (2026-10-15) -->
- ~~**NG4:** `IdentifyLandBridgeCells` and which types set `+0x354`. [not specified]~~ *Partly answered 2026-10-16
  (re-answers19): `Init` sets `+0x354` for every type with at least one bridge type, because it tests a field's
  address (`0x00626B53`); network-geometry.md §4.4.* **ANSWERED 2026-10-16 (re-specs17a):** MATCHES. The address
  test is confirmed (`lea` of bridge type `+0x98`, then a null test); the field is the land flag (property
  `0x482CB44F`), which every shipped bridge type sets, so reading the byte would give the same result: no shipped
  effect. The pass (network-bridges.md §7) scans the first cell's +x or +z edge altitudes for a drop below the last
  bank, keeps the lowest ground, and builds a land bridge (`CreateLandBridge` `0x00632580`) when the span has ≥ 4
  steps, is deeper than p+0x3C (55 m road, 30 m others) and the far bank is within p+0x40 (`0xC804DD96`, 20 m) of the
  near bank. The deck is the near bank's height; any occupant in the span's rectangle blocks it; the record's byte
  `+0x18` is 1. It never lowers `v` and sets no code.

<!-- from: From the network geometry spec (2026-10-15) -->
- ~~**NG5:** the rule-driven piece solver (`SolveUsingEvaluator`, `ApplyNetworkRules`, tile subsets) and
  `IdentifyHighwayIntersections`. [a later spec]~~ **PARTLY ANSWERED 2026-10-16 (re-answers19):** MATCHES for the
  solver (network-geometry.md §8.2 outlines the follow-up spec: the three rule INI resources, type `0x0A5BCF4B`
  group `0xAA5BCF57` instances `0x10000000`–`0x10000002`, loaded once by `ReloadRules` `0x006253D0`, and the entry
  points). `IdentifyHighwayIntersections` (`0x0063B340`) is described there: it marks rule matches but returns its
  `v` argument on every path, so **`0xF0000005` is never set** (quirk 7). **ANSWERED 2026-10-16 (re-specs17b):**
  MATCHES; the new spec [network-rules.md](network-rules.md) covers the rule resources and their grammars (§2–§3,
  §8.1, §9, §10.1–§10.3), the `#Transmogrify` expansion (§4), matching (§5), `SolveUsingEvaluator` and
  `ApplyNetworkRules` (§6), the edge subtypes (§7), multi-type intersections (§8.2), `AdjustTileSubsets` and
  `VerifyTileSubsets` (§9), `cIntRule` `Test`, `DoAutoComplete`, `IdentifyHighwayIntersections` and
  `InsertIsolatedHighwayIntersection` (§10), and what a piece id selects (§11). One addition to the outline:
  `SolveNetwork` runs the evaluators of all 13 types, not only the tool's. New open questions NR1–NR5 (network-rules.md
  §14).

<!-- from: From the network geometry spec (2026-10-15) -->
- ~~**NG6:** the readers of type info `+0x110` and of the p+0xA0 id list, and the source of road's support info.
  [only the flat-cell test is known]~~ **ANSWERED 2026-10-16 (re-answers19):** MATCHES (network-geometry.md §2.3,
  §10.2). `+0x110` is written only by `0x0063D460` and never read. The p+0xA0 list is read only at `0x00636763`, for
  its emptiness (the flat-cell test); its ids and the tool's copy at `+0x330` are never read. The support info is the
  type info's map at `+0x3C`, loaded by `LoadSupports` (`0x0063F780`) from the type `0x6534284A` exemplars of group
  `0x8974F80F` (`0xE974EAA1` piece id, `0xE974EAA2` u32 list, `0xE974EAA3` four points, rotated for the four
  rotations; `0x0063F400`), for road, one-way, dirt road, rail, light rail, monorail and street only.

<!-- from: From the network geometry spec (2026-10-15) -->
- ~~**NG7:** `DestroyOccupantsAndResolve` builds one neighbour key as `(x << 16) + z` while the tool's sets use
  `(z << 16) + x`. [unchecked; a destroy next to an off-diagonal neighbour would show it]~~ **ANSWERED 2026-10-16
  (re-answers19):** **DIFFERS** from the premise: there is no swapped key. Every key in the function is
  `(z << 16) + x` (row loop `0x0062B4E1`; neighbour key `0x0062BA9A`–`0x0062BA9D`); the earlier reading came from
  the decompiler's variable naming. network-geometry.md §13.

<!-- from: glue-nettools-shadow12 (engine lane, 2026-10-24) -->
- ~~**NG8 - section 3.3 underground drag extension, slot 65 override:** what is
  the precise occupant lookup/filter/layer and acceptance rule of the pipe/subway
  `TestForAutoConnectOccupant(x,z)` override (`0x00659470`), and how does the
  one-cell reserved-tile result participate in extending a drag? Section 1.1
  supplies the callable slot signatures, but section 3.3 does not specify that
  selection body. Clean run 512, `nettools-shadow12-draw-2032-20261024-aeaff8388`,
  observes type 4 `(16,44)-(10,40)` with snap false and true: native/core return
  true/true, cells 12/10, straight 0/0, total 7/6, repeated through 2032-01-01.
  The dimension-only recording host intentionally omits `extends_diagonal_end`;
  its empty-world answer is an adapter difference, not proof the native is wrong.
  Which public occupant queries reproduce the underground predicate without
  reading native tool fields or substituting the observed total? [retain the
  missing extension input and all differences; no narrow-drag exact credit;
  wide-drag public projection remains separately measured, not whole-owner proof]
  Normal-speed run 513, `nettools-shadow12-draw-normal-2032-20261024-aeaff8388`,
  is also clean to 2032-01-01 with ref_warnings=0 and reproduces both differences.~~
  **ANSWERED 2026-10-24 (re-nettools-ng9):** DIFFERS — network-geometry §3.3.1.
  Both ends can extend. Enumerate the candidate cell unfiltered with occupant-manager slot 40;
  accept QI for the tool's underground occupant class, otherwise a connection QI whose slot 38
  returns that class. Release list and QI references. Slot 67 never participates in the draw
  extension. Nine conformance vectors specify selection, counters, bounds and ownership.

- **NG16 — section 8.1 existing-piece lookup during removal rebuilds.**
  Please specify how `ComputeAndStoreCellInfo` selects an existing occupant when a cell
  contains several matching network occupants: layer mask, query interface and tie order,
  or whether their data is combined. The replacement's section 10 rebuild now reads the
  requested surviving neighbours instead of starting an empty drag; its current choice is
  the first spatial-query match on the tool's layer. A crossing within one occupant retains
  all thirteen edge words. No remaining qcheck difference is attributed to this tie choice.

**ANSWERED 2026-10-28 (re-blockers37b):** MATCHES — network-geometry.md §8.1 specifies first spatial match through occupant-manager slot 32 with the tool OR layer filter, one QI, newest-first tie order and no merging of distinct occupants.
