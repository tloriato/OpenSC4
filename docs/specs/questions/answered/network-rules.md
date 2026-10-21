# network-rules: answered questions (archive)
<!-- Archive for [network-rules.md](../../network-rules.md). Lanes do not read this file. Open questions are in ../network-rules.md. -->

<!-- from: From the network rules spec (2026-10-16) -->
- ~~**NR1:** the texture an ordinary piece id selects, and how the orientation byte turns it (network-rules.md §11).
  [not specified]~~ **ANSWERED 2026-10-16 (re-answers24), simulation part:** the piece id (or an intersection
  piece's path override) also selects the occupant's **path info**: `GetPathInfo` (network occupant slot 49,
  `0x0061B510`) maps it with `MapPath`, loads the path resource `{0x296678F7, 0x69668828 or 0xA966883F, id}` and
  makes the occupant's transformed copy, which is where the orientation turns the paths (network-rules.md §11.1). The
  texture an ordinary piece id selects is drawing only and is left to the renderer specs. The path-info internals are
  filed as NR6.

<!-- from: From the network rules spec (2026-10-16) -->
- ~~**NR2:** the placement of intersection models (`InsertHighwayIntersectionPiece(s)`, `0x006276D0`, `0x0062C4B0`),
  the rule's tile grid and map, `AutoTileBase`, `AutoPathBase` and the constraint layout's consumer (§10.3).
  [not specified]~~ **ANSWERED 2026-10-16 (re-answers24):** specified in network-rules.md §10.3 (the tile codes, a
  byte `row × 16 + col + 1`, and the map values `col × 0x100 + row × 0x10`) and §10.9 (each occupied cell gets piece
  `AutoTileBase + tile`, path override `AutoPathBase + tile` when `AutoPathBase` is set, the prebuilt-model class,
  group `0xA8434037`, the flags and the anchor). The constraint layout's consumer is `InsertIntersectionConstraints`
  (`0x00644710`) in the smoothing loop, height geometry; the codes' meaning is filed as NR7.

<!-- from: From the network rules spec (2026-10-16) -->
- ~~**NR3:** the exact `AddTypes` ring links (§10.8). [inferred: a second circular list through `+0xF4`/`+0xF8`]~~
  **ANSWERED 2026-10-16 (re-answers24):** the shape MATCHES: `+0xF4`/`+0xF8` form a second circular list of rule
  ids (the type ring). The spec's premise that `AddTypes` is read only without a `RotationRing` **DIFFERS**: every
  line is tested on its own (`RotationRing` first, then `AddTypes`, case-insensitively), so both may share a section.
  An `AddTypes` line forms its own rotation ring, and its k-th known rule enters the type ring of the k-th rule of
  the previous `RotationRing`/`AddTypes` line of the file, right after it (a file-wide cursor). network-rules.md
  §10.8, test 27.

<!-- from: From the network rules spec (2026-10-16) -->
- ~~**NR4:** `ResolveOneWayConflicts` and the preview's `SolveMultiTypeSeparation` (`0x00633FA0`). [not
  specified]~~ **ANSWERED 2026-10-16 (re-answers24):** network-rules.md §6.5 (the flow codes of `+0x50`, the codes
  `AssignEdgeMajorTypes` writes for the one-way tool, the propagation from the run's first and last steps into the
  existing one-way roads, and the per-cell conflict sweep, with a quirk: the sweep queues the current key instead of
  the neighbour's) and §8.3 (`SolveMultiTypeSeparation` returns the first evaluator's self-targeting output for one
  cell; it only draws and changes no state). Tests 26 and 29.

<!-- from: From the network rules spec (2026-10-16) -->
- ~~**NR5:** the order of several `[RuleOverrides]` patterns with the same first id (§9.1). [file order,
  inferred]~~ **ANSWERED 2026-10-16 (re-answers24):** file order MATCHES. `cRZIniFile::EnumEntries` (`0x0091D91C`)
  keeps every line, repeated keys included, in file order; the multimap insertion (`0x0063C920`) puts an equal `idA`
  after the existing entries; `PatchTilePair` walks the equal range forward, so the first matching line wins.
  network-rules.md §9.1, test 25.

<!-- from: From the enable=/spec-contradiction wave (filed by the coordinator from the implementer lanes' reports, 2026-10-16; answered by re-answers31) -->
- ~~**NR8** (`network-rules.md` §6.5 step 2.4): the step says only "skip `s`", but test 26 needs the east flow
  code dropped. Does a skipped side lose its flow code, on all three skip reasons or only missing-neighbour?~~
  **ANSWERED 2026-10-16 (re-answers31): DIFFERS.** A skipped side **keeps** its code, for all three reasons.
  The propagation only reads the code field; the sole zeroing is the conflict sweep of step 3
  (`decomp/cSC4NetworkTool.c:11343`). Test 26's X east drop is that sweep (edge word `0x00000202` maps sides 0
  and 2 both to partner side 2 with contradictory flows). Drop the `[choice]` in `solve.rs`. Written into
  [network-rules.md](../../network-rules.md) §6.5 step 2.4.

<!-- from: From the enable=/spec-contradiction wave (filed by the coordinator from the implementer lanes' reports, 2026-10-16; answered by re-answers31) -->
- ~~**NR12** (cores22): `fc4_network_rules_cell` carries neither `+0x57` (the one-way pass's re-solve mark) nor
  the occupant's paired portal (interface `0x8A4BD52B` slot 59), so a cell that crosses the C boundary loses
  both. Should the cell struct grow those fields, or should a glue lane pass them another way?~~
  **ANSWERED 2026-10-16 (re-answers31): neither belongs in the cell struct.** `+0x57` is per-pass scratch on the
  world-cache cell info (set in §6.5 step 2.1 and again in the sweep at `decomp/cSC4NetworkTool.c:11360`, never
  saved, read by nothing outside `ResolveOneWayConflicts`); the paired portal is a property of the *occupant*,
  reached only when the cell flags carry `0x80000`, through `0x8A4BD52B` slot 59 on the occupant's `+0x1D0`.
  Use a marks side-channel for the former and resolve the latter on demand. Written into
  [network-rules.md](../../network-rules.md) §14.

## From the glue-netrules4 lane (2026-10-21)

<!-- from: glue-netrules4 (2026-10-21) -->
- **NR13:** interoperability boundary for the rule solver. `cSC4TileEvaluator`, `cSC4TileRule`, `cSC4TileInfo` and
  `nSC4Networks` (§6, §9, §10) have no rows in docs/abi (no CLSID, IID, vtable or factory), and the spec names
  them only as internals of `cSC4NetworkTool::SolveNetwork`. Is there any game-reachable, supported boundary (a GZCOM
  class/interface the game creates through the COM director, a resource-loader/exemplar hook, or a message) through
  which the original game would call a replacement of these owners? If not, should the landed Rust
  `network_rules` (capi `fc4_network_rules_*`) be wired only as a component of our own network tool
  (engine/plugin/FC4NetToolHost.cpp `InsertOccupantsForSolvedCells`, today piece id 0, active under
  `enable = netmgr`), with no separate `enable = netrules` key? [blocks: glue for nr owners; no interception invented]
  **ANSWERED 2026-10-21 (re-netrules-boundary4):** MATCHES — no boundary of its own: the evaluator is a 0x10-byte plain object (one-slot vtable 0x00AA8338, deleting destructor only, no RTTI) made only by LoadEvaluator; the rule is a stack struct, cSC4TileInfo free functions, cIntRule an image-global map; every caller is a direct call in the network tool or the puzzle-piece input control, and no image pointer refers to them. Hosts: the rule resources (data) or the whole network tool (reachable only via a replacement network manager 0xC990BD46). Wire network_rules as a component of our own network tool, no separate enable key. Written into [network-rules.md](../../network-rules.md) §2.4, §12, §14.
