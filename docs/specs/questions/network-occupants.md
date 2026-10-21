# network-occupants: open questions
<!-- Questions about [network-occupants.md](../network-occupants.md). Append new entries at the END; when one is answered, move it to answered/network-occupants.md. Rules: README.md. -->

## From the network occupants spec (re-netocc lane, 2026-10-20)

Spec: [network-occupants.md](../network-occupants.md) §§1–10. RE-side follow-ups; nothing here blocks an
implementer of the class tables and records.

<!-- from: the network occupants spec (2026-10-20) -->
- **NO1 - network flag 0x40 vs the intersection bit:** the meaning of network flag `0x40` tested by
  `IsIntersection` (primary slot 55, `0x0061BD80`) versus the `0x100000` intersection bit used elsewhere
  (network-manager.md §5). [inferred: `0x40` is a styling/presentation bit; the slot checks it before the
  highway-intersection piece-id lookup at `0x00B4528C`]

<!-- from: the network occupants spec (2026-10-20) -->
- **NO2 - the power pole's delegate interface:** the identity of the delegate interface `0x26A68448` answered by
  the power pole via `+0xBC` (jump-table dispatch; likely a UI/drag helper). Probe: dump the `[+0xBC]` vtables on
  a live pole. [unknown]

<!-- from: the network occupants spec (2026-10-20) -->
- **NO3 - the underground occupant's type:** does `cSC4UndergroundOccupant` ever carry a distinct occupant type?
  Its base ctor leaves `0x74758925`; the demolition dispatcher never names an underground type. [inferred:
  underground pieces are dispatched as `0xC772BF98` through their `cISC4NetworkOccupant` interface]

<!-- from: the network occupants spec (2026-10-20) -->
- **NO4 - the prebuilt record's first five fields:** the semantics of the prebuilt `Write` fields
  `+0x170`/`+0x174`/`+0x178`/`+0x17C`/`+0x180` (network-occupants.md §2.8): model refs? shear inputs?
  [not specified]

<!-- from: the network occupants spec (2026-10-20) -->
- **NO5 - the 0x4835C6A7-AC property family:** SDK names of the property-ID family `0x4835C6A7` (edge types),
  `0x4835C6A8` (passage ceiling), `0x4835C6AA` (one-way), `0x4835C6AB` (anchor), `0x4835C6AC` (retrofit) stored
  in the occupant's own property holder. [unknown]

<!-- from: the network occupants spec (2026-10-20) -->
- **NO6 - the pipe's +0x268 field:** the meaning of pipe `Read`'s extra u32 → `+0x268` and `+0x258 = 1`
  (network-occupants.md §3.3): funding state? watered state? [unverified]

<!-- from: the network occupants spec (2026-10-20) -->
- **NO7 - GetSortValue's stored field:** which field holds the sort value of an underground occupant when flag
  `0x6000000` is set (§3.2); the negative displacement (`−0x178`/`−0x274`) in the decomp is ambiguous.
  [not specified]

<!-- from: the network occupants spec (2026-10-20) -->
- **NO8 - the connection registration strings:** the strings `0x00B0B6D0`/`0x00B0C388` passed with the class IDs
  to helper `0x00654710` by the pipe/subway-connection `Init` (§3.6): GZCOM class names? [unknown]
<!-- from: core-netocc4 (engine lane, 2026-10-21) -->
- **NO9 - IsManagedGlobally for the underground family:** §2.7 gives slot 5 for the plain class (false) and the
  prebuilt/bridge/tunnel primaries (true) but not for `cSC4UndergroundOccupant`/pipe/subway (`0x00AAAD40`,
  `0x00AA97B8`, `0x00AAA3C8`). The core answers false (inherited plain body). Which body do those vtables hold?
  (Numbered NO9 per QUESTIONS.md; §12 also uses NO9/NO10 for two settled notes.) [not specified]

<!-- from: core-netocc4 (engine lane, 2026-10-21) -->
- **NO10 - the pylon's QueryInterface IIDs:** §1.2's pylon row answers only `0xC9B6D7D6` (and `0x452294AA`), while
  the prose says every class accepts `0xA9764C8C`, and other rows also accept the own class ID. Does the pylon QI
  answer `0xA9764C8C` and/or `0x49B6D69B`? The core follows the row (neither). [ambiguous]

<!-- from: core-netocc4 (engine lane, 2026-10-21) -->
- **NO11 - the tunnel constructor's flag write:** §2.8 says `+0x13C &= ~0x400 | 0x10C00000`, "clearing the prebuilt
  bit", but the prebuilt bit is `0x400000`. Is the cleared mask `0x400` or `0x400000`, and is the expression
  `(f & ~mask) | 0x10C00000`? The core applies the literal `0x400` (a tunnel keeps `0x400000`). [ambiguous]

<!-- from: core-netocc4 (engine lane, 2026-10-21) -->
- **NO12 - the path-connection table shape and the wildcard count:** §2.6 addresses `[+0x144 + exit*4 + type]` and
  `[+0x144 + type]` (a 4 × 8 byte region that also contains the saved `+0x15C` u32), traffic-network.md §7.2 says
  `(pathType, entry, exit)` with a bit per path type. Which index is entry/exit, how many bytes per row, and with
  `0xFF` wildcards what is counted (matching entry/exit cells, or path-type bits too)? The core keeps
  `[entry][exit]` path-type bit bytes and counts (type, entry, exit) triples. [ambiguous]

<!-- from: core-netocc4 (engine lane, 2026-10-21) -->
- **NO13 - the power line's field offsets and record:** §5.1's layout puts the cell at `+0x40/+0x44` and three
  poles at `+0x48/+0x4C/+0x50`; its slot table reads `GetCell` from `+0x44/+0x48` and `GetConnectedPoles` from
  `+0x4C/+0x50` plus byte `+0x54`; §5.2 writes u32 `+0x30…+0x40` then references from `+0x4C`, `+0x44`, `+0x48`.
  Which live field is each record word/reference, is the cell z saved, does `Read` check the version (only "v2"
  is stated), and does `SetPosition`'s float→int helper truncate or round? The core keeps the record as a literal
  field list and truncates. [ambiguous]

<!-- from: core-netocc4 (engine lane, 2026-10-21) -->
- **NO14 - conformance test 6's slot:** §13 test 6 says "`SetRotation(2)` then slot 24 returns 2", but §2.4 maps
  slot 24 to `GetFlip` (slots 26/24/25 = orientation/flip/rotation). Is the test's slot 25 (`GetRotation`)? The
  core tests `GetRotation`. [ambiguous]
