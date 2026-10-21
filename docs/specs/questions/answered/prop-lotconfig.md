# prop-lotconfig: answered questions (archive)
<!-- Archive for [prop-lotconfig.md](../../prop-lotconfig.md). Lanes do not read this file. Open questions are in ../prop-lotconfig.md. -->

## From the prop manager, lot configuration manager and tutorial cores (filed by the coordinator from the `cores15` lane's report, 2026-10-16; merged `03e2dcf`)

cores15 implemented prop-lotconfig.md (Parts A and B), the object reader of lot-configuration.md §3.5 and tutorial.md
as Rust cores (`engine/core/src/{prop_manager,lot_config_manager,tutorial}/`), C ABIs (`fc4_prop_manager`,
`fc4_lot_config`, `fc4_tutorial`, ABI 68) and conformance vectors (`plt_*`, `tu8_*`, `tu00_*`). There is no plugin
glue yet, so none of these readings has been checked in game. The current choice is in brackets. The lane numbered
these PL4-PL10 and TU3-TU4; they are filed as PL8-PL14 (plumbing's first-generation questions used PL4 and PL7) and
TU4-TU5 (tutorial.md already has TU3).

<!-- from: From the prop manager, lot configuration manager and tutorial cores (2026-10-16) -->
- ~~**PL8:** (prop-lotconfig §A3.2) in what order does the profile reader `0x004A3DB0` apply its properties (class,
  size, satisfaction, wealth, crime)? OccupantSize values past the third overwrite the dwords from +0x10 on; values past
  the profile's end are dropped (the original writes past its local copy). [that table order, each over the defaults]~~ **ANSWERED 2026-10-16 (re-answers25):** **DIFFERS** in the overflow (prop-lotconfig §A3.2). The order is size first (`0x004A3DC4`–`0x004A3E11`: count and raw data pointer, copied with no bound; absent → three zeros), then class (default `0x2977AA47`, `0x004A3E20`), satisfaction (`0x004A3E38`), wealth (low byte only, `0x004A3E52`) and crime (`0x004A3E6D`), each default written just before its read. So extra size values are overwritten again: value 4 and value 6 vanish, value 5 survives in the upper three bytes of +0x14, and a seventh value overwrites the reader's return address (the local profile is the top 0x1C bytes of the frame): a crash, not a drop. A size with fewer than three values (or a scalar, count 0) leaves the missing size dwords unwritten (stack garbage). The bracket's table order and "each over the defaults" MATCH for the four scalar properties; *[choice]* FreeCity4 copies at most three size values, zero-fills the rest and keeps +0x14's upper bytes 0 (unreachable with shipped data). Test 13.

<!-- from: From the prop manager, lot configuration manager and tutorial cores (2026-10-16) -->
- ~~**PL9:** (§A6.1 step 5) what bounding box does an OccupantSize with one or two values give? [the missing values are
  0 (the building loader's bracket, DG32); an absent or empty size gives (1,1,1)]~~ **ANSWERED 2026-10-16 (re-answers25):** **DIFFERS** (unreachable with shipped data; the bracket is accepted as a *[choice]*, as for DG32). `0x004A2A70` passes the OccupantSize value's raw data pointer (variant slot 111, `0x004A2B72`) to `SetBoundingBox` as the far corner (`0x004A2B78`–`0x004A2BBE`), with no count or type check: one or two Float32 values read past the heap block (garbage, not 0), an empty array or a scalar gives a null or bogus address (garbage or a fault), integer arrays give their bits as floats. Absent → (0,0,0)–(1,1,1) MATCHES. prop-lotconfig §A6.1 step 5.

<!-- from: From the prop manager, lot configuration manager and tutorial cores (2026-10-16) -->
- ~~**PL10:** (§A7.1, slot 36 AddBuildingProp) when the building has no property holder, does the original skip the
  property step and still set usage 3 and return true? [yes *[inferred]*]~~ **ANSWERED 2026-10-16 (re-answers25):** **DIFFERS** in the usage. With no property holder the slot returns true at once (`0x004A3BEB` sets the result, `0x004A3BF6` jumps to the exit), skipping both the property step and the final usage write. The prop's usage is 3 only when this call inserted it (step 2, `0x004A3BDF`); an already-inserted prop keeps its usage. prop-lotconfig §A7.1.

<!-- from: From the prop manager, lot configuration manager and tutorial cores (2026-10-16) -->
- ~~**PL11:** (§B4.3, `0x0048AD40`) how does a building type whose exemplar does not load affect the building
  information? lot-configuration.md §4 does not cover it. [monopoly property absent (sets +0x8C), no groups
  *[inferred]*]~~ **ANSWERED 2026-10-16 (re-answers25):** MATCHES, with one addition (prop-lotconfig §B4.3, lot-configuration §4). The group collector `0x005E9500` loads the exemplar itself and returns nothing on failure (no groups); the monopoly flag is cleared before the load (`0x0048AE95`) and read only on success (`0x0048AEAA`), so a failed load sets +0x8C. The capacity comes from building development slot 39 (`0x0048AEE8`), not the exemplar, so the type **still counts** towards +0x3E/+0x40.

<!-- from: From the prop manager, lot configuration manager and tutorial cores (2026-10-16) -->
- ~~**PL12:** (§B5.7, CreateGenericLotConfiguration) what are the lot size and the object placement? [ceil(w/16) x
  ceil(d/16) cells, at least 1, for the slot 24 footprint, the building object centred *[choice]*]~~ **ANSWERED 2026-10-16 (re-answers25):** **DIFFERS** in the size. Each side is `1 − trunc(w × −0.0625)` (`0x0048BD66`–`0x0048BD9B`, `_ftol`), which is `1 + floor(w/16)`: one cell more than `ceil(w/16)` when w is a multiple of 16 (w = 16 → 2, 32 → 3; 4 → 1). The building object is centred on the lot (MATCHES): offset (8·sizeX, 0, 8·sizeZ), footprint ± (w/2, d/2), ID list [type]. Also: the ID is a fresh GUID (`cIGZCOM::CreateGuid`), maximum slope before foundation 5.0, and the object's lot tag (+0x24) is never written (stack garbage; *[choice]* 0). prop-lotconfig §B5.7, test 14.

<!-- from: From the prop manager, lot configuration manager and tutorial cores (2026-10-16) -->
- ~~**PL13:** (§B5.7, FixLotConfiguration) which validity test applies per record type, and how is the footprint
  recomputed? [building: BDS slot 16; prop: prop manager slot 11; flora: slot 11 or a non-empty family. The footprint
  is rebuilt from slots 24-26 (w and d swapped for orientations 1 and 3) around the old footprint's centre; one moved
  outside the lot by more than 0.049 m is shifted back in, a centre outside it is clamped *[choice]*. The result is
  the first problem's code]~~ **ANSWERED 2026-10-16 (re-answers25):** Validity tests MATCH, through the manager's held components (+0x14, +0x18, +0x1C): building development slot 16, prop manager slot 11, flora simulator slot 12 `IsFloraFamilyTypeValid` (a family or a registered type; equivalent to the bracket). Footprint **DIFFERS** in three ways: (1) a changed footprint is rebuilt around the record's **offset** (+0x04, +0x0C), not the old footprint's centre (`0x0048B667`–`0x0048B6A6`); (2) "outside the lot" (`minX/minZ < 0.049`, or `max > f32(lot − 0.05) + 0.001`) only sets code 4, **nothing is shifted** (`0x0048B6B3`–`0x0048B71F`); (3) an offset outside its footprint is **moved to the footprint's centre**, not clamped (`0x0048B727`–`0x0048B781`). The rebuild compares `|old − new| > 0` per axis after the orientation-1/3 swap. Only types 0, 1, 4 are examined. Result = the first problem's code (MATCHES). prop-lotconfig §B5.7, test 15.

<!-- from: From the prop manager, lot configuration manager and tutorial cores (2026-10-16) -->
- ~~**PL14:** (§B5.7) what counts as a valid retaining wall ID for the slot 28 list in FixLotConfiguration, and is only
  the first invalid ID removed (code 6)? [a host callback decides; the first invalid ID is removed]~~ **ANSWERED 2026-10-16 (re-answers25):** MATCHES on "first invalid only"; the rule is no host choice: an ID is valid when building development slot 28 `GetLotRetainingWallKeyFromType` (the held simulator, `0x0048B800`) returns true, i.e. it is the instance of an `ExemplarType 0x15` exemplar in the simulator's wall catalogue (`0x00698FC0`, lookup `0x00698250`). The first invalid ID is erased and the slot returns at once (`0x0048B822`–`0x0048B858`): the earlier code, else 6. prop-lotconfig §B5.7.

## From the parked glue21 lotcfg replacement (copied verbatim by re-pl19-run3, 2026-10-21)

- **PL19:** (§B5.2, §B7 slot 9; the SimNewDay growth-pass release wave) with the PL18 double-add
  (2 references per matched configuration per slot-9 query) the day-29 deletes are gone and the
  deletes move to day 30 and shrink to the configurations with no game-lot reference - but they do
  not stop: the game's release wave (SimCity 4.exe `0x00709798`) drains a touched configuration to
  ZERO whatever its reference count. Measured with the per-op trace restored
  (glue21fix2-ref2-2: no cushion; -ref2-3: a 40-reference diagnostic cushion per matched config):
  per configuration the load is create +1, core `add_configuration` +1, creation release -1
  (count 1), the game's own bulk wave +1 (exe `0x0069C385`), the game's per-candidate AddRefs
  (exe `0x006BF53D`, 1..66 per configuration over 736 of 1909), the slot-9 query adds (+2 per
  matched configuration: the core's §B5.2 add plus the plugin loop's), then the wave releases
  -1 per configuration per pass until the wrapper is DELETED: in ref2-3 five configurations of
  count 44 were released 43 logged times plus the deleting 44th; two others of the same count 44
  were released exactly once and stopped; in ref2-2 (no cushion) a count-4 configuration was
  released 4 times (deleted) and a count-5 one 4 times (survived at 1). The release count per
  configuration therefore follows the game's own candidate/pass structure, not the references it
  acquired (a configuration the game never AddRef'd was released 44 times), and no reference
  hand-out can balance it. Which container does `0x00709798` iterate (the growth developer's
  per-candidate list? the configuration set's entries per Grow pass), when does it stop, and what
  does the ORIGINAL do so its configurations are never drained to 0 (its catalogue reference is
  evidently never released)? Until this is answered from the exe, no FreeCity4 reading of slot 9 /
  slot 20 can make the lotcfg replacement game-clean; the double-add is kept (it strictly improves
  the ledger) and the fault stands between the run and a clean verdict.

  **ANSWERED 2026-10-21 (re-pl19-run3):** DIFFERS — `0x00709798` is the return address of the Release at `0x00709795` in `cSC4TractDeveloper::ListPossibleLotConfigurations` (`0x00709650`, once per Grow at `0x0070D4EA`); it walks the fresh local hash set that slot 9 filled, releases each element once, stops at the end of the set. The original balances it inside slot 9: one AddRef per returned element on EVERY call, cache hit included (`0x0048B29F`-`0x0048B2BC`); the filter cache holds no references. Net per configuration per query is 0; no extra add elsewhere (the PL18 double-add is wrong). A per-query drain means the add is skipped on some calls (e.g. cache hits) or lands on another object than the one put in the set. Rule: prop-lotconfig.md §B5.2 "Slot 9 reference rule", §B5.4, §B7 slot 9.
