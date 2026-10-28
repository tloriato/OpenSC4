# City save records: inventory, validation and the records no other spec covers (M6)

Status: M6 spec. Written from the 417 distinct city saves shipped with the game and from the Windows executable
`bin/SimCity4.exe` (1.1.641, image base `0x00400000`). Every address is a Windows address. Every class record of
every save was parsed with the layouts of the owning specs by a validator (§9); this spec reports the result,
fixes the layouts that were wrong or missing, and lists the corrections to other specs (§11). Implementers work from
this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled
or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Tags:

- *[compat]*: observable; a replacement must match it.
- *[choice]*: a FreeCity4 decision.
- *[inferred]*: consistent with all the evidence, but not proven.
- *[quirk]*: original behaviour that looks unintended but is reproduced (or survived).

Cited, not repeated: [foundations.md](foundations.md) §2 (streams, refs, strings), §3 (container, class store,
CRC, city components, save and load sequences), §9 (variants), §10 (property holders), §11 (regional-city record).
Each class's own layout is in its owning spec; the owner is named in §4 and §6.

Out of scope here (another M6 spec): lot records (`cSC4Lot`), every occupant record (buildings, props, flora,
network, pipes, poles…), the lot manager's lot list, and the trip data inside lot records. §8 lists where the
records of this spec depend on them.

---

## 1. Corpus

| Item | Value |
|---|---|
| Files | 587 `.sc4` files under the user's `Documents\SimCity 4` tree; **417 distinct** by content hash. They are the 417 saves shipped in the game's region folders (the `FC4 Test` region and its backup are copies of Berlin). The folder is read-only for all M6 work. |
| Writer | All were written by Maxis builds. App version record (§2): 62 in 403 saves, 64 in 1, 65 in 13. |
| Module version record | Present in 10 saves only (the newest builds: 1.1.577 and similar). |
| Written by 1.1.641 | **None.** A record version that only the 1.1.641 `Write` produces (fire v3, police v8, landfill zone v4, MySim v12) is verified against the executable only. |
| Container | DBPF 1.0, index 7.0, all 417. Every class record's entry CRCs (foundations §3.4) are correct; every class record's entry count matches the index; every index total matches the sum; no record has trailing bytes. |

Saves are cited by file name only. Several shipped regions reuse names such as `City - New City (10).sc4`; the
name cited is the first in path order.

---

## 2. Non-class records *[compat]*

| Type / group / instance | Saves | Written by | FreeCity4 | Layout |
|---|---|---|---|---|
| `0x8A2482B9 / 0x4A2482BB / 0, 2 (4, 6)` | 417 | region-view thumbnails (PNG), `cSC4City::WriteRegionViewThumbnail`; two per save in old builds, four in 13 saves | kept (not a simulation record) | — |
| `0x2026960B / 0x4A2482BB / 1, 3 (5, 7)` | 417 | the thumbnails' companion names (12 bytes max) | kept | — |
| `0xEA3FF4F5 / 0xEA3FF506 / 0xEA3FF4F5` | 417 | app version, `SC4WriteAppLoadSaveVersion` (`0x005DBFA0`); u32 62, 64 or 65 | written by FreeCity4 (65) | foundations §3.7 |
| `0x0B7D45E0 / 0x0B7D45E1 / 0x0B7D45E2` | 10 | module version (four u32, written at `0x004561CC`) | written | foundations §3.7 |
| `0xCA027EDB / 0xCA027EE1 / 0` | 417 | regional-city record | neighbours (standalone mode) | foundations §11.2; validated in §7.5 |
| `0xA9C8647C / 0xE9C8647E / 1` | 417 | `cSC4View3DWin::DoSave` (`0x007EA5B0`). Holds refs to the camera control `0xC9C628EC`, the 3D renderer `0xE9C622D8`, the class `0xA9C741B3` and the **simulator** `0x2990C1E5`, then u32 lists | not replaced, but it refers to a replaced object (§5) | not specified |
| `0x49DD6F4C / 0x49DD6F4E / 1` | 417 | root record: `u16 1, u16 2, ref city` | written | foundations §3.7 |
| `0x89D9B9EE / 0x69D9B9F1 / 0` | 417 | auxiliary cheat-code state, written from inside `cSC4City::Write` (`0x0045C5B0`), 58 bytes max in that corpus | city (not replaced) | **§2.1** |
| `0x8A91E7E0 / 0xE98F9525 / 2` | 417 | terrain vertex properties (`cSTETerrain::SaveVertexProperties`) | not replaced | — |
| `0x8A91E7E3 / 0xE98F9525 / 3` | 417 | terrain misc data (`SaveMiscTerrainData`) | not replaced | — |
| `0xA9DD6FF4 / 0xE98F9525 / 1, 0x64…0x67` | 417 | terrain altitudes and the four edge-altitude strips (`SaveAltitudes`, `SaveEdgeAltitudes`) | not replaced | — |
| `0x2026960B / 0x6AAEE931` and `/ 0x6AAEE932`, instances 1…6 | 14 | city-name and mayor-name strings per language (foundations §3.2, §11.2) | neighbours / city | foundations §11.2 |
| `0x499B23FE / 0x299B2D1B / 0` | 417 | class-store index (version 1) | written | foundations §3.3 |
| `0x?? / 0x299B2D1B / 0` | 417 | class records (§3) | per class | §4 |
| `0xE86B1EEF / 0xE86B1EEF / 0x286B1F03` | 417 | DBPF directory of compressed records | written | foundations §3.1 |

No save holds a record of group `0xAB85E27D` (the retained group, foundations §3.8).

---

### 2.1 City auxiliary cheat-state record *[compat]*

Key **`0x89D9B9EE / 0x69D9B9F1 / 0`**. This is a non-class record: no class-entry size/CRC/object ID surrounds
it. `cSC4City::Write` opens it with `cSC4AutoSaveRecord`, version **1** (`0x0045CA2A`–`0x0045CA3C`);
the helper writes the first u16 (auto-records §2). The remaining fields are:

| Order | Field | Wire type / rule |
|---|---|---|
| 1 | version | `u16 1` |
| 2 | cheat-data byte length `N` | `u32` |
| 3 | cheat data | exactly `N` bytes, structured below |

All integers in both layers are little-endian. The city asks the cheat-code manager for the required size,
allocates a zero-filled byte buffer, asks it to fill that buffer, writes `N` and then the bytes
(`0x0045CA4B`–`0x0045CAB1`). These are serial-record field calls (`SetFieldUint32`, then a byte-array write),
not a §2.7 reference and not a GZ string. There is no 58-byte format limit: 58 was a corpus maximum.
If no cheat manager exists, the writer still opens/closes the version-1 record but emits no length or data;
that exceptional output consists of the two version bytes alone.

**Inner cheat data** (`cGZCheatCodeManager::GetCheatCodeSaveData`, `0x00921060`; see also cheat-codes §4):

| Order | Field | Encoding |
|---|---|---|
| 1 | duplicate total length | `u32 N`, including this word and all following inner fields |
| 2 | padded password size `P` | `u32`, password byte size rounded up to a multiple of 4 (0 when size is 0) |
| 3 | password byte size `B` | `u32` |
| 4 | password data | `B` compressed password bytes, then `P−B` zero bytes |
| 5 | history byte size `H` | `u32`, a multiple of 16 |
| 6 | history | `H / 16` records, each four retained `u32` words (cheat ID, date word, action, auxiliary word); stored history order |
| 7 | enabled-state byte size `E` | `u32`, **4 × the number of saved IDs**, not an element count |
| 8 | enabled-state IDs | `E / 4` × `u32 cheatID`, ascending ID (set iteration) |

Thus **`N = 20 + P + H + E`**, and the whole DBPF record is **`6 + N` bytes**. The password is the compressed
lock data of cheat-codes §3.1, not a NUL-terminated string. History action 1/2 records enabling/disabling a
state; other action words and the auxiliary word retain their history values. The manager's integer sizing
and raw-copy instructions are `0x0092106C`–`0x009210DB`, `0x009210F1`–`0x00921145`.

**Load and versions.** City Read opens the separate record after consuming its own final flag. Only version
**1**, with a live record and cheat manager, enters the payload reader (`0x0045D153`–`0x0045D16B`). Other
versions, or a missing record/manager, are skipped; there is no accepted legacy v0 layout or speculative
new-version decode. For v1 it reads a `u32 N`, sizes a byte vector to N, reads N bytes and passes `(buffer, N)`
to `SetCheatCodeSaveData` (`0x0045D16D`–`0x0045D1B2`). A header-only record has no successful length read;
it is not an empty 20-byte cheat block. Missing auxiliary data does not add or remove bytes from the city
class entry, and this optional read does not govern the city's success result.

**Original reader quirks** (cheat-codes §4, CCM1). `SetCheatCodeSaveData` (`0x009214DB`) checks only non-null
buffer and `N ≥ 4`. It ignores the duplicate total length, copies B password bytes, skips P bytes to reach
history, appends `H >> 4` history entries and then mistakes **E for an ID count**: it reads E dwords rather
than E/4 (`0x00921571`–`0x00921598`). For nonzero E this overreads the allocated inner block; no DBPF field
follows that block to define the extra IDs. It does not update the stored password-size field or the manager's
enabled flag and does not clear existing history or enabled IDs. *[choice]* A standalone reader validates
the inner lengths against N, consumes only E/4 IDs, and never reads beyond the record. Preserve the on-wire
byte-count convention in the writer; do not change E to an element count to compensate for the original bug.

**Worked empty-state record.** Version 1, N = 20, duplicate N = 20, P = B = H = E = 0 gives **26 bytes**, with
no password, history or state IDs. All 547 checked reference end saves have exactly that record. Both layers
were parsed to their exact end, not just skipped using the DBPF size (foundations §3.6.3).

## 3. Class store: inventory

128 classes occur. The table gives every class with the number of saves and entries. "Replaced" is the M1–M5 set
of the brief. The owner column names the spec section with the layout.

### 3.1 Classes FreeCity4 replaces

| Class | Name | Saves | Entries | Owner (layout) |
|---|---|---|---|---|
| `0x2990C1E5` | simulator | 417 | 417 | simulator §8 |
| `0xE990BE01` | budget simulator | 417 | 417 | budget §11.1 |
| `0xE990BFFC` | department budget | 417 | 7,506 | budget §11.2 |
| `0xAA313C9F` | line item | 417 | 8,412 | budget §11.3 |
| `0x2990BFF2` | demand simulator | 417 | 417 | demand §12.1 |
| `0x8990C12D` | demand node | 417 | 72,558 | demand §12.2 |
| `0xA990C08E` | ordinance simulator | 417 | 417 | ordinance §9.1 |
| 20 ordinance classes (`0x00D0723D`, `0x02BF1DFA`, `0x22F6E80C`, `0x22F6E81B`, `0x40D07236`, `0x42BF1E18`, `0x62BF1DAA`, `0x62BF1DB9`, `0x62F6E7CF`, `0x815B4CEF`, `0x82B9999B`, `0xA0D07129`, `0xA2BF1DDC`, `0xA2BF1DE5`, `0xA2BF1E43`, `0xC2BF1DC5`, `0xC2BF1E04`, `0xC2F6E81F`, `0xE0D0722E`, `0xE0D07233`) | ordinances | 417 each | 417 each | ordinance §9.2–§9.3 |
| `0x2990C1BC` | weather | 417 | 417 | weather §10 |
| `0x49B9E602` … `0x49B9E606`, `0x49B9E60A` | sim grids (u8, s8, u16, s16, u32, f32) | 417 | 19,182 / 2,085 / 11,259 / 15,844 / 417 / 6,692 | simgrid §5 |
| `0xE990C18E` | land value | 417 | 417 | landvalue §10 |
| `0x8990C065` | pollution | 417 | 417 | pollution §6 |
| `0x8990C09A` | crime | 417 | 417 | crime §12 |
| `0x0990BDD8` | aura | 417 | 417 | aura §11.2 |
| `0x0990C005` | power | 417 | 417 | power §9 |
| `0x0990C075` | plumbing | 417 | 417 | plumbing §9 |
| `0x2990C05A` / `0x6990C04F` | fire / police | 417 / 417 | 417 / 417 | protection §11 |
| `0x4C0AE114` / `0x0C0B06E8` | fire-plane / police-helicopter dispatch source | **0** | 0 | §7.4 |
| `0xC990C034` | flora simulator | 417 | 417 | flora §10.1 |
| `0xE990C045` | flammability | 417 | 417 | flammability §10 |
| `0x4990C013` | residential (education, health) | 417 | 417 | residential §9 |
| `0x4A0DD616` | advisor system | 417 | 417 | advisors §11.1 |
| `0xEAC937B4` | advisor (plain; base-class advisors in new saves) | 14 | 28 | advisors §11.2 |
| `0xCA09F543` | advisor base (old saves) | 403 | 806 | advisors §11.2, §11.6; §7.6 here |
| `0x6A5F8755`, `0xAA5292D7`, `0xCA2C2A2F`, `0xEA19E825`, `0x4A3AD3E1` | HQ, finance, utility, safety, traffic advisors | 417 each | 417 each | advisors §11.2 |
| `0x2A5F877D` | city-planning advisor | 417 | 417 | advisors §11.5 |
| `0x4BE372CD` | city-situations advisor | 10 | 10 | advisors §11.2 |
| `0x8A09F5F4`, `0xAA371C32`, `0x8BE3753B` | advice, reward advice, city-situation advice | 417 / 417 / 10 | 271,194 / 21,338 / 811 | advisors §11.3 |
| `0x6A3848DB` | neighbour-deal advice | 417 | 10,842 | **§7.2** (advisors §11.3 is wrong) |
| `0x6A26D049` | advice subject | 8 | 36 | advisors §11.4 |
| `0x4A1DBBBF` | MySim | 417 | 2,085 | mysims §6.2 |
| `0x0BF606A5` | My Street sim | 10 | 10 | §7.3 (= the MySim record) |
| `0x6A9335DE`, `0x6B70EFAD` | MySim advice, MySim balloon advice | 417 / 10 | 375,723 / 5,712 | advisors §11.3 |
| `0xC990C0A4` | neighbours simulator | 417 | 417 | neighbors §13 |
| `0x0A134F24` | neighbour connection | 10 | 58 | neighbors §4.3 |
| `0x2A134F42` | neighbour deal | 2 | 4 | neighbors §5.6 |
| `0xEA09D4AE` | landfill developer | 417 | 417 | landfill §6.1–§6.2 |
| `0x8A22BEBE` | landfill occupant | 3 | 73 | landfill §6.4 (an occupant; also the occupant spec's) |
| `0x8990BE7A` | building development | 417 | 417 | building-development §6 |
| `0x0990C138` | growth developer | 417 | 5,004 | growth-developer §9 |
| `0x2990C142` | tract developer | 417 | 417 | tract-developer §6 |
| `0xA990BFE0` | lot developer | 417 | 417 | lot-developer §13 |
| `0x298F9B2D` / `0x498F9B01` | zone manager / zone developer (also the lot manager object) | 417 / 417 | 417 / 417 | zones §10.1 / §10.2 |
| `0x6990C1AA` | traffic simulator | 417 | 417 | **§7.1** (replaces traffic-simulator §6.1, §6.4) |

The pathfinder has no record (pathfinder §6). Its compressed routes are saved inside the lots' trip data (§8).

The separate traffic **network map**, city component 42 (IID `0xCA0F81B9`, class/record type `0x6A0F82B2`),
has its full versioned wire layout in [traffic-network.md §6.2](traffic-network.md#62-traffic-network-map-0x6a0f82b2-compat),
including tiles, intersections, reference lists and the final occupant-to-four-lists table.

The remaining SR36 occupant layouts are now specified by their owners:

| Class record | Complete wire layout | Validation in 749 reference end saves (re-sr37, 2026-10-28) |
|---|---|---|
| `0x2977AA48` terrain-decal prop | [lot-records §10.5](lot-records.md#105-csc4propoccupantterraindecal-0x2977aa48-compat): outer v3, complete prop, 47-byte extension | 44 class records / 4,683 objects / 641,571 decompressed bytes |
| `0x29D25F1D` fire occupant | [disasters §5.11.2](disasters.md#5112-saved-fire-occupant-and-inline-flames-sr36-2-compat): outer v3, occupant base, three u32, target reference, inline flames with independent versions | 498 class records / 860 objects / 1,264 inline flames / 107,840 decompressed bytes |
| `0xC9EFB016` burnt-building prop | [lot-records §10.6](lot-records.md#106-csc4propoccupantburntbuilding-0xc9efb016-compat): outer v3, complete prop, 24-byte source bounds | 38 class records / 75 objects / 8,550 decompressed bytes |

These class records use group `0x299B2D1B`, instance 0 and foundations §3.3 entry framing.
All 5,618 object payloads were decoded field by field and re-encoded exactly, with no bytes
left over; counts above include the 12-byte entry headers. Legacy inner-prop timer gates and
flame-v2 transforms are instruction-derived rules, not additional observed corpus variants.
The proof concerns these layouts; it does not rerun the standalone archive's full-type audit.

### 3.2 Classes FreeCity4 does not replace

City `0x8990C372`; history warehouse `0x89EFA536`; demolition `0x0990C38A`; dispatch manager `0x4990BD93`;
occupant manager `0x098F964D`; prop manager `0xA990BD10`; lot configuration manager `0x299062C0`; prop developer
`0x89C48F47`; network lot manager `0xE9A6CD3B`; airport / seaport developers `0xAA2049A1` / `0xEA230A28`; park
manager `0xAA3FE6BC`; commercial `0x8990C080` (19 bytes) and industrial `0xA990C022` (2 bytes) simulators; civic
building simulator `0x0A413481`; automata script system `0x4A1B664E`, controller manager `0xCA14CA03`, generator
`0x4A1628C5`, attractor `0x6A1628BA`; traffic network map `0x6A0F82B2`; vehicle, pedestrian, aircraft, watercraft
managers; tutorial system `0xAA3836FB`; terrain `0xE98F9525`; 24-hour clock `0xA7E7F929`; city detail manager
`0xEAD4BA36`; city situation manager `0x0BB14381`; network manager `0xC990BD46`; wealth requester `0x89C49986`
(empty payload); business unit `0x4A232DA8`; camera control `0xC9C628EC`; 3D renderer `0xE9C622D8`; the
model maker `0xA9C741B3` (110 bytes, View3D), terrain view `0xC9B84E10` (3 bytes), subway connection occupant
`0xCA15ED6B` (1 save), pipe connection occupant `0xEA0A5ACF` (4 saves) (named in Q5);
vehicles, pedestrians, patrol cars, trailers; and every lot and occupant class (out of scope, §8).

---

## 4. Version census and validation result *[compat]*

"Current" is the version the 1.1.641 `Write` writes (read from the executable). "Seen" is entries per version over
the 417 saves. "Parsed" counts entries that the owning spec's layout (with its version rules, and with the
corrections of §7) consumed exactly to the end of the entry payload, with every ref resolving to an object present
in the same save. "Round trip" counts entries whose parse, written back field by field with booleans as 0/1, gives
the identical bytes.

| Class | Current (Write) | Seen | Parsed | Round trip | Notes |
|---|---|---|---|---|---|
| simulator | 4 (`0x00702290`) | 3: 407, 4: 10 | 417/417 | 417 | v3 holds the timer (§7.6) |
| budget | 5 (`0x00690530`) | 4: 409, 5: 8 | 417/417 | 417 | |
| department | 10 (`0x006A9D70`) | 9: 7,326, 10: 180 | all | all | local holders → building occupants |
| line item | 4 (`0x006A9AA0`) | 4: 8,412 | all | all | |
| demand simulator | 2 (`0x006A2680`) | 1: 407, 2: 10 | 417/417 | 417 | |
| demand node | 2 (`0x005D5080`) | 1: 70,818, 2: 1,740 | all | all | refs: 72,141 from the demand simulator + 417 from building development = all 72,558 |
| ordinance simulator | 4 (`0x0080D780`) | 4: 417 | all | all | always 20 ordinance refs |
| ordinances (20 classes) | 4 (`0x0080CB30`) | 1: 403, 3: 4, 4: 10 (each class) | all | all | |
| weather | 2 (`0x00725E80`) | 1: 407, 2: 10 | all | all | the v1 legacy block is always empty (n = 0, 0 × 0 grids) |
| sim grids | 1 | 1 | all 55,479 | all | |
| land value | 1 (`0x006BD8D0`) | 1 | all | all | |
| pollution | 4 (`0x006E3380`) | 2: 407, 3: 2, 4: 8 | all | all | |
| crime | 6 (`0x0069F600`) | 6 | all | all | incident refs → lots |
| aura | 9 (`0x0068CC50`) | 8: 403, 9: 14 | all | all | |
| power | 6 (`0x006EB6A0`) | 5: 407, 6: 10 | all | all | plant refs → building occupants, pole refs → pole occupants |
| plumbing | 4 (`0x006DB090`) | 4 | all | all | producer refs → building occupants, pipe refs → pipe occupants |
| fire | **3** (`0x006AD250`) | 2: 417 | all | all | no save holds v3 (plane-source ref) |
| police | **8** (`0x006E03F0`) | 7: 417 | all | all | no save holds v8 (helicopter-source ref) |
| flora | 2 (`0x006AFEE0`) | 2 | all | all | |
| flammability | 2 (`0x006AE620`) | 2 | all | all | |
| residential | 7 (`0x006F8C40`) | 2: 403, 4: 4, 5: 1, 7: 9 | all | all | |
| advisor system | 1 (`0x00418D10`) | 1 | all | all | 13 or 15 advisor refs |
| advisors (shared Write `0x0040E2E0`) | 3 | 3 | all | all | |
| city planning | 3 (`0x00411C80`) | 2: 403, 3: 14 | all | all | |
| advice and subclasses (`0x00408090`) | 4 | 3: 643,188, 4: 31,590 (advice, reward, city-situation, MySim and balloon advice) | all | all | |
| neighbour-deal advice | 2 (`0x00420F40`) | 2: 10,842 | all (after §7.2) | all | |
| subject | 4 (`0x0040CF20`) | 4 | all | all | objects → building occupants (3), neighbour deals (4) |
| MySim | 17 (`0x004251A0`) | 10: 2,035, 14: 5, 16: 15, 17: 30 | all | all | `state` is 0 in every save (§10 Q3) |
| My Street sim | 17 (`0x0043D1B0`) | 14: 1, 16: 3, 17: 6 | all | all | §7.3 |
| neighbours simulator | 2 (`0x006CFB60`) | 2 | all | all | |
| connection | 1 (`0x006CF080`) | 1 | all | all | |
| deal | 5 (`0x006CE400`) | 5 | all | all | |
| landfill developer | 3 (`0x006B9880`) | 3; zones: v3 only | all | all | no save holds a v4 zone (zot ref) |
| landfill occupant | 3 (`0x00477350`) | 3 | all | all | |
| building development | 4 (`0x00697800`) | 4 | all | all | |
| growth developer | 1 (`0x006B3960`) | 1 | all | all | |
| tract developer | 3 (`0x00704B60`) | 1: 407, 2: 3, 3: 7 | all | all | |
| lot developer | 1 (`0x006C66F0`) | 1 | all | all | night list refs → building occupants |
| zone manager / zone developer | 1 / 1 | 1 / 1 | all | all | cell-lot refs → lots |
| traffic simulator | 41 (`0x00712400`) | 19: 403, 22: 4, 33: 1, 40: 4, 41: 5 | 417/417 (after §7.1) | 407 | 10 v19 records hold a byte 5 in a bool field (§7.1.4) |
| regional-city record | 13 | 9: 373, 10: 34, 13: 10 | 417/417 | 417 | §7.5 |

**Failures before correction** (example save, object ID, byte offset in the payload):

- Neighbour-deal advice: every entry, e.g. `City - Konradshohe.sc4`, object `0x17C6D720`: the advisors §11.3 layout
  ends at byte 38 and 10 bytes remain. Fixed by §7.2.
- Traffic simulator: no spec layout was complete enough to parse a record (traffic-simulator §6.1 rows 14, 17, 19
  are wrong, §11). Fixed by §7.1.

Every other class parsed under its owning spec's layout including that spec's older-version rules; no layout change
was needed.

---

## 5. Save and load order (foundations §3.5–§3.7) *[compat]*

File positions of the records show this sequence in 376 saves. In 34 more the regional-city record sits after the
class records, because a neighbour's save later replaced it (foundations §3.7 step 7); 7 saves were repacked by
another tool and keep no write order.

1. thumbnails and their names;
2. app version (and module version in new builds);
3. **regional-city record**;
4. the View3D record `0xA9C8647C`;
5. root record;
6. class records, with `0x89D9B9EE` and the terrain records written in between (by the city's and the terrain's
   `Write`);
7. class index, then the DBPF directory.

This is consistent with the executable: the root record's stream is opened, the app's `Write` (`0x004545D0`) writes
the city ref and the regional-city record, then the Save/PostSave messages run (the View3D window writes its
record then *[inferred: the View3D writer was not traced to the message]*), and only then is the root stream
closed. Each record lands in the file when it is closed, so the root record comes after the regional-city and
View3D records.

Consequences:

- The View3D record holds a ref to the **simulator**, so the simulator is registered in the class store before
  the city's components. In all 410 saves that keep the write order, the simulator's class record is the first
  class record in the file. A replacement simulator must be registered and serializable at the time the Save message is sent.
- Class records do not appear in ascending class-ID order in the file. The order is one ascending sweep over the
  registered classes that also visits classes registered during the sweep (when their ID is above the current
  one), then further sweeps from the lowest ID for the rest (6 to 9 ascending runs per save). This matches
  foundations §3.5 if its "for each class C, ascending class ID" loop walks the live sorted set. Loading does not
  depend on file order (the loader walks the index), so *[choice]* FreeCity4 may write class records in any order.
- The class index lists classes in ascending ID order in every save.

---

## 6. Observed values (test vectors)

Payload sizes per class and version, with the smallest and largest entry, are test vectors for the round-trip
tests (§10). Selected field values follow; every value range below was seen in the corpus.

| Class (version) | Payload bytes | Example (save, object ID) |
|---|---|---|
| simulator v3 / v4 | 89 / 79 | `City - New City (10).sc4` `0x0B987834` / `City - Konradshohe.sc4` `0x0EC64A18` |
| budget v4, v5 | 2,487 | `City - New City (10).sc4` `0x02059564` |
| department v9 / v10 | 54…92 / 54…162 | `City - Big City Tutorial.sc4` `0x14EA2598` (162) |
| demand simulator v1 / v2 | 2,310 / 2,294 | `City - New City (10).sc4` `0x0B6B0228` |
| demand node v1 / v2 | 52 / 55 | |
| ordinance v1 / v3 / v4 | 189…324 / +1 byte / +1 byte | `City - Schonerlinde.sc4` (v3) |
| weather v1 / v2 | 16,677…263,845 / 65,809…66,097 | `City - New City (49).sc4` `0x0C6E92BC` (263,845) |
| pollution v2 / v3 / v4 | 2,412…33,132 / 8,548 / 8,552…8,560 | `City - Getting Started Tutorial.sc4` `0x11347690` (v3) |
| aura v8 / v9 | 98 / 94…118 | |
| power v5 / v6 | 10,914…164,514 / 41,634…41,774 | `City - New City (30).sc4` `0x057DE0E0` (164,514) |
| residential v2 / v4 / v5 / v7 | 409 / 450 / 450 / 452…476 | `City - Getting Started Tutorial.sc4` `0x11E0C5A4` (v5) |
| tract developer v1 / v2 / v3 | 10 / 27 / 31 | `City - Big City Tutorial.sc4` `0x14EC061C` (v2) |
| MySim v10 / v14 / v16 / v17 | 4,497…4,525 / 5,572 / 5,600…6,150 / 7,750…8,272 | `City - Getting Started Tutorial.sc4` `0x13A373B8` (v14) |
| traffic v19 / v22 / v33 / v40 / v41 | 5,250…67,458 / 17,808 / 18,112 / 22,218…1,858,642 / 298,802…859,116 | `City - Big City Tutorial.sc4` `0x1058DB04` (v40, largest) |
| regional record v9 / v10 / v13 | 1,315…6,675 | `City - Fulham.sc4` (holds two deals) |

Field values:

- Simulator: `simSpeed` 0…2; `pauseCount` 0…1; `inTick` 0; `simTime` 0; `frameBudgetMs` 15…38; `julianDay`
  2,451,545…2,462,369; v3 timer: units 3, elapsed 0…8,968, running 0/1; `levelCounter[7]` up to 2,029.
- Budget: funds 834…9,837,173; 18 departments; 7 groups (names "Business Deals", "City Beautification", "Health &
  Education", "Public Safety", "Transportation", "Utilities", …); tax rates 0…9.1; max rates 0, 22, 25.
- Department: funding 0.19…1.2; max funding 1.0, 1.2; up to 6 line items, up to 8 local holders.
- Demand node: v1 `demandCapPercent` byte is **0 in all 70,818 entries** (so v1 nodes load with cap 0.0); v2 cap
  0…0.549; tax modifier −0.061…0.30; active range sentinels ±8.507 × 10^37.
- Weather: grid 64/128/256 cells; ambient temperature 48.75…81.25; wind speed 2.13…3.62; the legacy block of v1
  always has n = 0 and two 0 × 0 grids.
- Power: plants 0…7, poles 0…10, deficit 0.0, words per row 128…2,048.
- Plumbing: producers 0…2 with capacity 20,000; pipes 0…483.
- Aura: departments 0…2; mayor rating 0…55.
- Crime: incidents 0…24, all named `####`.
- Residential: school lists 7, hospital lists 3, population 0…22,618.
- Zone manager: zone counts 0…65,536; creation cost 0…50; display alpha 170.
- Lot developer: night list 0…4,294 refs.
- Neighbours: 0…13 connections.
- Traffic: header (v ≥ 24) `A = 13`, `B = 1`, `C = 13` (v33: 6), `D = 9`; destination map 66², 130² or 258²
  (city size + 2); volume grid count 4 (v19, v22) or 6; edge densities 0 (all v19–v33) … 26,185; switches 0…3
  with 5…45 rules; property-holder map 0…2 entries (keys 1, 2); tunnels and ferry terminals 0 in every save.
- Regional record: population 0…22,618; workforce 0.40…0.48 (v13) and 40.0, 50.0 (v10, a percentage); 40
  population types; 12 tax rates; up to 13 connections, up to 10 pending changes; 3 commodities; up to 2 deals.

**Fill patterns** *[quirk]*: some fields hold the Microsoft debug-heap fill words `0xCDCDCDCD` and `0xBAADF00D`
(uninitialised memory written by the original): budget ordinance expense and income by month (as s64
`0xCDCDCDCDCDCDCDCD`, `0xBAADF00DBAADF00D`), growth developer `totalCapacity`, residential unused floats
(−431,602,080.0), flora `rng.cached` when `hasCached` is 0, and many traffic counters (§7.1). A reader must accept
any value in such fields and carry it unchanged; FreeCity4 writes 0 there for new data *[choice]*.

---

## 7. Records specified or corrected here

### 7.1 Traffic simulator (`0x6990C1AA`; `Write` `0x00712400`, `Read` `0x00721990`) *[compat]*

This replaces traffic-simulator.md §6.1 and §6.4. Field meanings come from traffic-simulator §2 and §6.1 and from
traffic-network §2.4–§2.6 where they are known; the others are named by type and position. **ref** is a
foundations §2.7 object reference (u32 object ID, then the u32 class ID when the ID is not 0), not an inline
object.

#### 7.1.1 Layout (current version 41) with the Read version gates

`v` is the record version. A field marked with a gate is present only when the gate holds; otherwise Read uses the
default shown.

```
u16   v                                      Read: v < 19 → return false (nothing else read)
s32   A = 13   commute/network-type count    only if v ≥ 24, else A = 8
s32   B = 1    bridge-row count              only if v ≥ 37, else B = 1
s32   C = 13   legacy row count              only if v ≥ 28, else C = 4
s32   D = 9    travel-type count             only if v ≥ 28, else D = 7
6 × ref        the six published grids of traffic-simulator §6.1 row 3 (IID 0xC9BD939A); classes seen:
               u8 grid, f32 grid, u8, u8, u8, u8
u32   rows     destination map (W + 2)
u32   cols     (H + 2) = size of row 0
rows × cols u8 destination map, row by row
u32   n        volume grids (4 in v19/v22 saves, 6 later)
n × ref        f32 grids
2 × ref        the 0x69D5C3FA grid and the peak-ratio grid (f32 grids)
u32   n        edge densities
n × edge density (below)
3 × { u32 jobCapacity[k]; u32 attempted[k]; u32 routed[k] }        k = 0…2
2 × ref        transit-switch mode-mask grid (u8, 0x69D5C424), transit-switch ID grid (u32, 0x69D5C42D)
u32   nextSwitchID
u32   n        transit switches
n × switch (below)
2 × ref        roads department, mass-transit department (IID 0x223E2B67)
s64            budget-item total
f32            transit income
f32            toll income                   only if v ≥ 41
f32            last monthly income
f64            income accumulator            if v ≥ 22; else f32 (widened)
f64            fee accumulator               only if v ≥ 41
u32            cycle start date
u8             strike state
s32            strike month counter
damage block   road
damage block   rail                          only if v ≥ 26
u8             cycle state
29 bytes       scanning iterator: s8 state, s32 x, s32 z, u32 step, s32 x0, z0, x1, z1
bool           intersection-dirty flag
4 × s32        intersection dirty rectangle
bool           network-changed byte (traffic-simulator §3.1; set by SimBegin and occupant messages, never read)
u32            strategy bucket counter
u32            strategy bucket
2 × s16        phase positions               if v ≥ 25; else 2 × u8
30 bytes       lot iterator position: s8, s8, 7 × s32
bool, bool
f32
s32, s32       re-route context cell (x, z)
u32                                          (not a count; values 0…5 and fill words)
4 × u32
u8
u32
4 × s32
3 × D u32      a 3 × D table, row-major; Read zero-fills each row to 9
u8, u8
s32 processed, s32 dayIndex                  (corrected: dayIndex is this s32; traffic-simulator §6.1, TS60)
s32 total                                    only if v ≥ 25
u32 days, u32, u32, u32                      (the three u32 are never used, only saved and loaded)
3 × s32 (passTotal, toReRoute, toSearch)     only if v ≥ 25
4 × u32        pass counters 0…3             only if v ≥ 25
6 × u32        pass counters 4…9             only if v ≥ 40, else 0
2 × statistics set (below): the published "last" set, then the working set
u32   n        tunnels
n × tunnel (below)
f32            avgCarCommute
f32            carLenSum
u32            carLenCount
f32 freightLenSum, u32 freightCount          only if v ≥ 21, else 0
A × u32        cells per network type        Read zero-fills to 13
u32   n (4)    then n × u32 freeDest[k]      Read fills missing entries with 0xFFFFFFFF up to 4
u32   n        property-holder map
n × { u16 key; plain property holder record (foundations §10.2) }
D × { u32 a[i]; u32 b[i] }                   only if v ≥ 27; the two per-travel-type vectors, interleaved:
                                             a = published, b = building (traffic-simulator §6.1, TS56)
u32            ferry total                   only if v ≥ 39
u32   n        ferry terminals               only if v ≥ 30
n × ferry terminal (below)
```

`Write` returns `!GetError()`. Read's result is `!GetError()` as well (it does not validate counts).

**Edge density** (traffic-simulator §4.8):

- v ≥ 20: `u32 key; u16 last[16]; u16 usage; u16 current[16]` (70 bytes).
- v < 20: `u32 key; f32 × 16; f32; f32 × 16; f32` (140 bytes). Each of the first 33 floats becomes a u16 by
  rounding to the nearest integer (ties to even) and replacing any result outside 0…65,535 (negative included) by
  65,535. The 34th float is read and discarded.

**Transit switch** (traffic-network §2.4):

```
u32   id
ref   building (IID 0x26BB767E)
f32   load
f32   time factor
f32   capacity
                     v < 29 only: u32 n, n × u8 (discarded; Read then marks the switches for rebuilding)
bool  has access point
u8    access x
u8    access z
f32   counter 1
f32   counter 2      v < 23: stored × 100 (Read multiplies by 0.01)
u32   n rules        only if v ≥ 29
n × { bool kind; 4 × bool side; u16 from; u16 to }
f32   toll (0x0BFC0A4C)           only if v ≥ 33, else 0
bool  counts as intersection      only if v ≥ 33, else false
```

Read recomputes the switch's flags and its kind-0 "to" mask from the rules (traffic-network §2.4).

**Damage block** (traffic-simulator §6.1 row 13): `u32 cellCount; u32 damagedCount; f32 damageRate; u32 n, n × u16
intact cell keys; u32 n, n × u16 damaged cell keys`.

**Statistics set** (traffic-simulator §4.9):

```
R rows × { f32 sum; u32 count; s32 maxX; s32 maxZ; f32 max }    R = A if v ≥ 35, else C
                 (f32: the sum in the working set, the average in the published one; u32: the count of r > 1; traffic-simulator §4.9)
B rows × the same 5 fields (the bridge row)                       only if v ≥ 37
A × u16    network-info records per type (published: = the cell counts)
B × u16    bridge records (never published)                       only if v ≥ 37
u16        pothole count in the published set (traffic-simulator §3.6)
s32 E (12)                                                        only if v ≥ 34, else E = 3
E × { u16; f32; s32; s32; f32 }                                   v < 23: both f32 × 0.01 on read
f32, f32
bool
f32
u8                                                                only if v ≥ 21, else 0x80
```

For v < 35 the C legacy rows are stored into network-type rows through the table `{0, 0, 1, 7, 8, 9}` (row i →
type table[i]); with the default C = 4 the second row overwrites the first *[quirk]*. The only v < 35 save with
C > 4 (`City - Getting Started Tutorial.sc4`, v33) has C = 6.

**Tunnel** (traffic-network §2.5; the map is keyed by the portal cell):

```
u16  key           portal cell key
u8   otherX, u8 otherZ
u16  travel mask   (0x1FE)
u16  network type
D × f32 cost       Read zero-fills to 9
```

**Ferry terminal** (traffic-network §2.6):

```
                   v < 32 only: u16 (discarded)
u8 × 6             departure (x1, z1), arrival (x2, z2), water (x3, z3)
u8                 ferry type
u16                travel mask
u16, u16           riders (last, current)
u8, u8             lot cell (x, z)        only if v ≥ 38; else the departure cell
u32 n              terminal links
n × { f32 cost; s16 arrivalX; s16 arrivalZ; u16 departureKey; u32 m, m × u8 route;
      u16 × 4 counters                    only if v ≥ 36, else 0 }
u32 n              neighbour links
n × { f32 cost; s16 × 4 (off-map x, z, edge x, z *[inferred: order]*); u32 m, m × u8 route out;
      u32 m, m × u8 route back            only if v ≥ 31, else empty;
      u16 × 4 counters                    only if v ≥ 36, else 0 }
```

The four counters are written in the order counter 0, 2, 1, 3 of the in-memory array, and Read stores them back in
the same order, so the file order is stable *[compat]*. A route vector count ≥ 1,000,000 is read as an empty
vector (the bytes are not skipped) *[quirk]*. Read rebuilds the departure and arrival tables from each terminal and
counts terminals by ferry type (types 0 and 1 only).

#### 7.1.2 Differences from traffic-simulator §6.1

- Rows 3, 5, 6, 9, 10 (building), 11: refs, not inline objects. The grids and departments are class-store objects
  of their own (grid classes `0x49B9E602`, `0x49B9E606`, `0x49B9E60A`; department `0xE990BFFC`).
- Row 14: after the re-route context the record holds `u32`, `4 × u32` (no count), `u8`, `u32`, `4 × s32`, then the
  `3 × D` u32 table (no count), then `u8, u8`.
- Row 17 is the **tunnel table**, not neighbour-connection records. traffic-network §6 says tunnels are not saved;
  they are.
- Row 18: the `u32 4` before `freeDest` is a count.
- Row 19: the "per-type usage table" is a map from u16 to a plain property holder; the two per-travel-type vectors
  are interleaved pairs; terminal-link and neighbour-link counters come **after** the route vectors.

#### 7.1.3 Reading old versions *[choice]*

403 of the 417 saves hold version 19, and the original reads every version from 19 to 41. FreeCity4 reads all of
them with the gates above (the validator does, for every record of the corpus). The §6.4 choice of the traffic
simulator spec (skip an older record and start fresh) is withdrawn: skipping leaves the six published grids
unreferenced (they are separate objects) and loses the destination map, damage state and switch state.

#### 7.1.4 Round trip

All records at version 40 and 41 round-trip byte for byte. In 10 v19 records (for example `City - New City
(11).sc4`) the bool right after the lot iterator position holds the byte 5. Read keeps only "non-zero", so the
original writes 1 there; a byte-exact round trip of an old record is not expected (it is upgraded to v41 anyway).

### 7.2 Neighbour-deal advice (`0x6A3848DB`; `Write` `0x00420F40`, `Read` `0x00420F80`) *[compat]*

```
u16  2                        Read: anything else → false
advice record                 advisors §11.3 (u16 4 … ref eventSubject)
s32  dealType                 nd_type
```

All 10,842 entries parse with this layout and have version 2. Their subjects (when present) are advice-subject
objects whose `object` ref is a neighbour deal `0x2A134F42`.

### 7.3 My Street sim (`0x0BF606A5`; `Write` `0x0043D1B0`, `Read` `0x0043D1D0`) *[compat]*

Both functions call the MySim `Write` / `Read` (`0x004251A0` / `0x00425490`) and return `!GetError()`. The record is
therefore exactly the MySim record of mysims §6.2. The advisor system refers to it (advisor ID 30). It occurs in the
10 new saves (versions 14, 16 and 17) and parses completely. mysims.md keeps the class out of scope; a FreeCity4
MySim implementation that registers `0x0BF606A5` reads and writes it with the same code.

### 7.4 Air dispatch sources (`0x4C0AE114` fire planes, `0x0C0B06E8` police helicopters) *[compat]*

Both classes share `Write` `0x006AC930`:

```
u16  1
u32  n;  n × ref        occupant list (IID 0x26BB767E on read)
u32                     a dispatch-source value *[inferred: meaning unknown]*
```

`Read`: the fire class (`0x006AC970`) accepts only version 1; the police class (`0x006DF970`) accepts versions 0
and 1 *[quirk]*. Both read the list with the standard list reader (a count ≥ 10,000,000 or 0 reads nothing) and
return `!GetError()`.

No save of the corpus holds either class, because fire v2 and police v7 have no source ref (protection §11). Only
a save written by 1.1.641 contains them. Their behaviour belongs to a dispatch spec.

### 7.5 Regional-city record (foundations §11.2)

All 417 records parse completely with the foundations §11.2 layout and version rules, including the connection
records (neighbors §4.3) and deal records (neighbors §5.6) it embeds. Versions: 9 (373), 10 (34), 13 (10). Version 10
stores the workforce as a percentage (40.0, 50.0). Two saves hold two deals each in the local list (`City - Fulham.sc4`,
`City - Kensington.sc4`). `moved`, the language and country fields, `birthDate`, the world coordinates and the three unknown
strings are 0 / empty in every record. `tutorialID` is −1 except in the five tutorial cities (for example 1 in `City - Terraforming Tutorial.sc4`).

### 7.6 Notes on other classes

- **Simulator v3** (407 saves): between the level entries and `julianDay` sits the stream timer: `u8 units
  (always 3), u64 elapsed, u8 running` (11 bytes). simulator §8 allows rejecting version 3; that would break 98 % of
  the saves, so FreeCity4 reads it and discards the timer *[choice]*.
- **Advisor base `0xCA09F543`**: 403 old saves hold two entries each (the Environment and Fluff news advisors),
  saved under this class ID by older builds. 1.1.641 writes `0xEAC937B4` for them (advisors §11.6). The record is
  the plain advisor record. A FreeCity4 advisor system must create an advisor for both class IDs on load.
- **Landfill zone v4** and **MySim placed state** (`state` ≠ 0) are never present in the corpus; their layouts are
  checked against the executable only.

---

## 8. Dependencies on lot and occupant records

The records of this spec refer to objects owned by the lot/occupant spec. A save loads only if both specs agree on
those classes (refs are resolved by class ID and object ID, foundations §2.7).

| Referring record | Ref target class | Count in corpus |
|---|---|---|
| department local holders | building occupant `0xA9BD882D` | 62 |
| power plants / poles | `0xA9BD882D` / power-pole occupant `0x09C05C6A` | 21 / 37 |
| plumbing producers / pipes | `0xA9BD882D` / pipe occupant `0x49C05B9F` | 7 / 1,483 |
| fire, police stations, police jails | `0xA9BD882D` | 15, 3, 3 |
| pollution waste-to-energy plant | `0xA9BD882D` | 1 |
| residential schools, hospitals, pending school | `0xA9BD882D` | 5, 6, 1 |
| building development special buildings | `0xA9BD882D` | 12 |
| lot developer night list | `0xA9BD882D` | 12,295 |
| traffic transit-switch building | `0xA9BD882D` | 6 |
| advice subject object | `0xA9BD882D` (and neighbour deals) | 3 (4) |
| zone developer cell lots | lot `0xC9BD5D4A` | 21,179 |
| crime incident lots | `0xC9BD5D4A` | 43 |
| landfill zone zot (v4 only) | occupant (IID `0x26BB767E`) | 0 |
| air dispatch source lists | occupants | 0 |

The traffic simulator's trip data and the pathfinder's compressed routes live inside the lot records
(traffic-simulator §6.3), as do the neighbour connections' trip lists (neighbors). Only 8 saves contain lots at all.

---

## 9. The validator *[choice]*

FreeCity4 keeps a save validator as a conformance tool (`engine/conformance`, M6 implementation task). Its
algorithm, which produced every number in this spec:

1. Open every `.sc4` under a given folder read-only; skip files with a content hash already seen.
2. Read the DBPF index and decompress QFS records listed in the directory record.
3. Read the class index (version 1). For each class: open its record, split it into entries (foundations §3.3),
   check each entry's CRC (§3.4) and the entry count.
4. For each entry of a class in the replaced set, parse the payload with the layout of the owning spec (versioned)
   and require that the parse ends exactly at the end of the payload. Collect each ref and require that its
   (class, object ID) exists in the same save.
5. Re-emit the parsed fields (booleans as 0/1) and compare with the payload (round trip).
6. Parse the regional-city record (§7.5).
7. Report per class: saves, entries, versions, parsed, round-trip, non-canonical booleans, ref target classes,
   and the first failures (file name, object ID, byte offset, reason).

---

## 10. Conformance tests (must exist in `engine/conformance`)

1. **Corpus load.** For every save of the corpus (when the shipped saves are available), every class of §3.1
   parses with the FreeCity4 reader to the exact end of each entry, and every ref of its records resolves.
2. **Round trip, current version.** Read then Write gives identical bytes for every entry already at the current
   version: simulator v4 (10 entries), budget v5 (8), department v10 (180), demand node v2 (1,740), ordinances
   v4, weather v2, pollution v4, aura v9, power v6, residential v7, tract developer v3, MySim v17, traffic v41 (5
   records, e.g. `City - Fulham.sc4`), and every single-version class.
3. **Old version upgrade.** Read an old record (simulator v3, weather v1, traffic v19, v22, v33, v40, …), Write it
   (current version), Read the result: the state is identical to the first Read.
4. **Traffic gates.** Hand-built v19, v20, v21, v22, v23, v24, v25, v26, v27, v28, v29, v30, v31, v32, v33, v34,
   v35, v36, v37, v38, v39, v40, v41 records exercise each gate of §7.1.1 once, including the v < 20 float edge
   densities (rounding ties to even, −1.0 → 65,535, 70,000.0 → 65,535) and the v < 23 ×0.01 scaling.
5. **Neighbour-deal advice.** A record `u16 2`, advice v4, `s32 dealType` round-trips; `u16 1` is rejected.
6. **Air dispatch sources.** `u16 1, u32 0, u32 x` round-trips for both classes; version 0 is accepted by the
   police class only.
7. **Fill words.** A budget record whose by-month ordinance arrays hold `0xCDCDCDCDCDCDCDCD` loads and round-trips.

---

## 11. Corrections to other specs

All of these have been applied to the specs named (M6 integration); the list stays as the evidence record.

- **advisors.md §11.3**: "Subclasses that add state (neighbour deal `nd_type`) do not save it" → the neighbour-deal
  advice `0x6A3848DB` saves `u16 2`, the advice record, then `s32 dealType` (§7.2). Evidence: `Write` `0x00420F40`,
  `Read` `0x00420F80` (requires version 2); all 10,842 entries.
- **traffic-simulator.md §6.1**: grids, departments and the switch building are refs, not `SetGZSerializable`
  objects; rows 14, 17, 18, 19 are wrong as listed in §7.1.2; the full layout is §7.1. Evidence: `Write`
  `0x00712400` (14 calls of the ref writer `0x005D99A0`), `Read` `0x00721990`, all 417 records.
- **traffic-simulator.md §6.4**: the choice to skip older records → read versions 19–41 (§7.1.3). Evidence: 403 of
  417 saves are v19.
- **traffic-network.md §6**: "tunnel table … not saved" → it is saved (§7.1.1, tunnel records). Evidence: Read
  fills `hash_map<u16, TunnelInfo>` at `0x00722E18`.
- **simulator.md §8**: the choice to reject versions below 4 → must read v3 (407 saves); the timer is `u8, u64, u8`
  (§7.6).
- **protection.md §8.4 / §13.2**: "Old saves contain refs to these classes" → no save of the corpus does (fire v2,
  police v7 everywhere). The source record is `u16 1, u32 n + n refs, u32`; fire Read accepts version 1 only,
  police Read versions 0–1 (§7.4).
- **foundations.md §3.7**: step 3 and 4: the regional-city record and the View3D record (`0xA9C8647C /
  0xE9C8647E / 1`, written during the Save message, with a ref to the simulator) are both written while the root
  record is still open, so they precede it in the file; the View3D ref registers the simulator first (§5).
- **foundations.md §3.5**: the class loop walks the live sorted set (classes registered during a sweep are written
  in the same sweep when their ID is higher) (§5).
- **foundations.md §11.2**: "None contains a deal" holds for the test region only; two shipped saves contain deals,
  and they parse with the layout.
- **advisors.md §11.6**: add that 403 shipped saves hold the Environment and Fluff news advisors as `0xCA09F543`
  objects, so loading must accept that class ID (§7.6).

---

## 12. Open questions

- Q1: The meaning of the `u32` after the occupant list in the air dispatch-source record (§7.4).
- Q2: The meanings of the unnamed traffic fields in §7.1.1 (the `u32` and `4 × u32` after the re-route context,
  the `u8`/`u32`/`4 × s32` group, the `3 × D` table, the property-holder map keys 1 and 2, the per-set `u16` and
  E-records). They round-trip unchanged; a behaviour spec must name them before FreeCity4 can compute them.
- Q3: No save holds a placed MySim (`state` ≠ 0), a v4 landfill zone, a tunnel, a ferry terminal, or a v < 20
  edge density with data. Their layouts are verified against the executable only; a 1.1.641 save exercising them
  would close this.
- ~~Q4: The city auxiliary and View3D records were unspecified.~~ **ANSWERED 2026-10-28 (re-sr36):** auxiliary
  cheat-state record `0x89D9B9EE` is §2.1; the city object itself is foundations §3.6. The View3D record was
  answered 2026-10-21 (re-view3dwin), view3d-window.md §6k (four references, then global effect triggers).
- ~~Q5: The unnamed classes `0xA9C741B3`, `0xC9B84E10`, `0xCA15ED6B` and `0xEA0A5ACF` (§3.2).~~ **ANSWERED
  2026-10-15 (re-specs14a, re-inventory6):** `0xC9B84E10` is the terrain's 3D view (`cSTETerrainView3D`), saved
  through a reference in the terrain's class record (u16 1 and one byte, render only; terrain.md §13.2).
  `0xA9C741B3` is `cSC4ModelMaker` (`GetGZCLSID` `0x0048EA60`; the View3D record's model-maker reference is
  resolved with this ID at `0x007EC01C`). `0xEA0A5ACF` is `cSC4PipeConnectionOccupant` (`GetGZCLSID`
  `0x00648040`) and `0xCA15ED6B` is `cSC4SubwayConnectionOccupant` (`GetGZCLSID` `0x00652A70`), both
  underground connection occupants. All four are render-side; [INVENTORY-M6.md](INVENTORY-M6.md) §7.
