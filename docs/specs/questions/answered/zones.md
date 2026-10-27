# zones: answered questions (archive)
<!-- Archive for [zones.md](../../zones.md). Lanes do not read this file. Open questions are in ../zones.md. -->

## From the zones core (filed by the coordinator from the lane's report, 2026-10-13; zones merged as `f56e52a`)

All were about `docs/specs/zones.md`. The lane implemented each from the prose and flagged where no conformance
vector pins the choice — so these are places where the core could be wrong and nothing would catch it.

<!-- from: From the zones core (2026-10-13) -->
~~**ZQ1 — §8.3 slot 10, `GetLotsSurroundingRect` walk order.**~~ **ANSWERED 2026-10-13** from the exe and written
into §8.3: row by row, ascending z, with interior rows visiting only their two side cells. The lane's row-by-row
inference was correct. Kept here struck through rather than deleted so the numbering of ZQ2–ZQ6 still matches the
lane's report.

<!-- from: From the zones core (2026-10-13) -->
~~**ZQ2 — §7.4 `CreateParcellizableRegion`, the parenthetical "(real mode or the cell is occupied per §8.4)".**~~
**ANSWERED 2026-10-13** from the exe and written into zones.md §7.4, with a highlight-mode table. The predicate is the
zone developer's per-cell `IsCellOccupied(x, z)` (`0x0072C050`), called only in highlight mode:
- real mode: a real lot is on the cell;
- highlight mode: a real lot that is not virtually deleted, or a virtual lot.

That is §8.4's code-7 cell condition without codes 0–3, so `can_create_lot(…) == 7` is not equivalent. The lot then
examined is always the real one (`GetLot(x, z, false)`).

<!-- from: From the zones core (2026-10-13) -->
~~**ZQ3 — §7.2.1 `DetermineLotSize` has no conformance test in §15.**~~ **ANSWERED 2026-10-13** from the exe:
§7.2.1 was re-checked against the bytes and zones.md §15 now has **test 30 (a–g)**, with a scripted BDS,
configurations and developers and every expected width, depth, minDepth, maxDepth and block. Three corrections were
written into §7.2.1:
- the configuration filter also requires growth stage ≤ stage;
- the configuration `hash_set` is keyed by **pointer**, so the original's order is heap-address order. It is
  observable only through a score tie. *[choice]* FreeCity4 uses ascending configuration ID, pinned by test 30e;
- the x87 chain equals the exact floor for k ≤ 64 (shipped k = 16) but not above (test 30g).

<!-- from: From the zones core (2026-10-13) -->
~~**ZQ4 — no vector covers highlight mode.**
`HighlightParcels` (§7.1), virtual lots, virtually deleted cells, and the §7.13 draft fallbacks are all untested. The
draft fallbacks need the network tools' `OccupantExists(x, z, flag, &mask)`, which §8.8/§7.13 describe only for highlight
mode. Partly a headless limit (the highlight is a UI preview) and partly a missing spec test.~~ **ANSWERED 2026-10-14**
from the exe. zones.md §7.1 now covers HighlightParcels, ClearHighlight and a table of everything highlight mode
changes. §7.2 step 6 gives the `*out` lifecycle, including the unchecked point read (a *[choice]*). §8.8 gives the
`OccupantExists` contract. Two corrections: §7.12's parallel test does have a draft branch, and `ExistsNetworkOfType`
skips the dirt-road tool when the street bit is set. §15 **test 33 (a–e)** pins the preview of tests 19 and 23, a
virtually deleted lot, the point filter and the draft queries.

<!-- from: From the zones core (2026-10-13) -->
~~**ZQ5 — §7.3 `ExamineBoundaries` city-border behaviour is untested.**~~ **ANSWERED 2026-10-13** from the exe:
the clamp was checked on the bytes (`0x00731C8D`) and zones.md §15 now has **test 31**, with both low edges, both
high edges and the placeStreets-off case. §7.3 now states the consequence: at the border the examined cell is R's
own zoned, empty cell. It scores like an empty outside cell (+1 per cell with `placeStreets`), so a border side is
neither skipped nor scored as an obstacle.

<!-- from: From the zones core (2026-10-13) -->
~~**ZQ6 — §7.5 step 1 table, three of six cells unpinned.**~~ **ANSWERED 2026-10-13** from the exe. The table was
checked on the bytes and is correct as written: Q is `+0xCC`, D is `+0xD0`, and the jump table is at `0x0072EC44`.
zones.md §15 now has **test 32**, which gives the street positions for all eight perpendicular (Q, D) pairs on a
16 × 16 rect (n = 6), plus the edge rule. That pins every arm, including the four listed here.

From the ZQ3/ZQ5/ZQ6 follow-up lane (filed by the coordinator, 2026-10-14; merged `8ee3253`; vectors
`zn15_32`–`zn15_35`). About zones.md §7.2.1 `DetermineLotSize`; the core's current answer is in brackets.

<!-- from: From the zones core (2026-10-13) -->
~~**ZQ7 — step 2: a missing candidate developer.** The original does not null-check the candidate developers. What
happens when the BDS lacks one? [skipped: the first existing candidate becomes best]~~ **ANSWERED 2026-10-14** from
the exe: the original reads each result's vtable at once (`0x00732C72`), so a missing developer crashes it. The
bracket is kept as a *[choice]*, with no candidates at all taking the no-best path (zones.md §7.2.1 step 2).

<!-- from: From the zones core (2026-10-13) -->
~~**ZQ8 — step 5: signedness of `GetGrowthStage() ≤ stage`.** The growth stage is a u8; the stage is an int32. Signed
or unsigned compare? [zero-extended, compared signed; the two differ only for a negative `maxStage`]~~ **ANSWERED
2026-10-14** from the exe: the bracket matches (`movzx`, then `setle` at `0x0072A0B7`–`0x0072A0BD`). zones.md §7.2.1
step 5 now says so, with the negative-stage example.

<!-- from: From the zones core (2026-10-13) -->
~~**ZQ9 — step 6: the initial best score, and wrap of `|v − bestDemand|`.** [the first scored configuration is always
taken; the difference is computed without a 32-bit wrap]~~ **ANSWERED 2026-10-14** from the exe: the bracket differs
at the extremes. The best score starts at `0x7FFFFFFF` (`0x00732F77`), and the take test is signed `<`
(`0x007331B1`) on an int32 difference that wraps, with an abs that leaves −2³¹ alone. So a score of `0x7FFFFFFF`
is never taken, and a wrapped −2³¹ always wins. The two agree for every |difference| < 2³¹ − 1. The int32 rule is
recommended (zones.md §7.2.1 step 6, with an example).

<!-- from: From the zones core (2026-10-13) -->
~~**ZQ10 — step 7: signedness of `minDepth > 2`.** [unsigned, like the widening compares; both agree for real depths]~~
**ANSWERED 2026-10-14** from the exe: the bracket matches (`jbe` at `0x00733342`). zones.md §7.2.1 step 7 also notes
that `width mod sx` is an unsigned `div` (`0x007332AA`), so `sx = 0` crashes the original. That case is now a
*[choice]*: such a configuration fails the size filter.

<!-- from: From applying the pathfinder, zones, landfill and neighbours answers (2026-10-14) -->
- ~~**ZQ11 — zones test 33:** a second draft `Place` on the same cell. [the mask bytes are merged (OR); inferred]~~
  **ANSWERED 2026-10-14** from the exe (`PlaceNetworkOccupants` 0x00629210, cache clear only in real mode at
  0x0062932F; record update 0x00633250; edge merge 0x00638160 → 0x00620F00; draft byte 0x0062591E) and written into
  zones §8.8 (the `OccupantExists` item) and test 33's stub.
  - The draft records live in the tool's cell cache, which only `Reset` and a real-mode Place empty, so a second draft
    Place finds the first one's record and updates it: flags := old OR new; the new edge directions are merged into
    the old ones. The draft byte stays set.
  - The merge keeps every connected edge and fills unconnected ones, so for the zone manager's "byte ≠ 0" contract the
    mask is the OR. The bracket matches, now from the exe. The byte values are not an OR (direction codes, minor
    types), so a replacement promises only ≠ 0.

<!-- from: From applying the pathfinder, zones, landfill and neighbours answers (2026-10-14) -->
- ~~**ZQ12 — zones test 33:** a draft `Place` over a cell that already has a network. [no draft, as a real `Place`]~~
  **ANSWERED 2026-10-14** from the exe (record from the existing network with the draft byte clear 0x00633030; flag OR
  0x00633250; draft byte on every re-solved cell 0x0062591E/0x00625BFB; draft-mode end lengthening
  0x006296D0–0x006297A7) and written into zones §8.8 (the `OccupantExists` item) and test 33's stub.
  - In the original the cell **does** become a draft once the solve succeeds. Its flags are the existing network's OR
    the tool's (8), and its mask is the existing edges of the tool's type merged with the new ones. An existing
    network cell next to the draft that the solver includes also gets the draft byte, with its own flags and the edge
    towards the draft.
  - A segment draft is also lengthened into a neighbouring earlier draft (and, in both modes, into a real network of
    its own type).
  - The bracket differs. The test-33 stub may keep it, because no checked result depends on it: in (e) the original
    also drafts (12, 9) with an edge towards (12, 10), which only `ExistsConnectionOfType` could see, and no call to
    it comes after that Place. A replacement network tool must follow §8.8.

## From the zones glue and shadow (filed by the coordinator from the `zoneglue` lane's report, 2026-10-14)

About `docs/specs/zones.md` and the zone manager glue (`cFC4ZoneManager`, `FC4ZonesShadow`; phase 1, the zone
developer stays original). The current answer is in brackets.

<!-- from: From the zones glue and shadow (2026-10-14) -->
- ~~**ZG1:** The tuning exemplar type: `0xA52160F5` or §4's `0x6534284A`? [`0x6534284A`]~~
  **ANSWERED 2026-10-14** from the exe (`ReadSimulatorTunableValues` `0x004DC460`: key `0x004DC49B`–`0x004DC4AB`,
  resource call `0x004DC4B6`) and written into zones §4. The type is `0x6534284A`; `0xA52160F5` is the interface ID
  asked of the resource manager with that key. The bracket matches.

<!-- from: From the zones glue and shadow (2026-10-14) -->
- ~~**ZG2:** An oversized region or a bad §11.4 layout passed to PlaceZone. [fails with error 0 and touches nothing]~~
  **ANSWERED 2026-10-14** from the exe (`PlaceZone` cell loop `0x004DD8B0`–`0x004DD913`) and written into zones §6
  (pseudo-code and a new note). The original has no city-bounds test. It skips only cells outside the region's own
  bitmap or with the bit clear, then reads (and on a commit writes) the zone grid unchecked. A rect past the city edge
  therefore reads and writes outside the grid, and a bad bitmap is dereferenced. The bracket (fail with error 0, touch
  nothing) is a *[choice]*, recommended. Also corrected: §6's "only cells inside the city".

<!-- from: From the zones glue and shadow (2026-10-14) -->
- ~~**ZG3:** The zone display alpha after `Read`. [the alpha read]~~
  **ANSWERED 2026-10-14** from the exe (`Read` `0x004DD58E`; `Init` `0x004DE192`; `ReadSimulatorTunableValues`
  `0x004DC65E`–`0x004DC675`) and written into zones §10.1. Right after `Read` the alpha is the saved byte, as
  bracketed. But `Init`, which follows every load, overwrites it with the exemplar's Zone Display Alpha (shipped 170)
  and sets the default alpha to the same value. The saved alpha survives only when the exemplar or the property is
  missing.

<!-- from: From the zones glue and shadow (2026-10-14) -->
- ~~**ZG4:** A second `Read`: the services the first one set. [replaced; the old ones released]~~
  **ANSWERED 2026-10-14** from the exe (`Read` reference reads `0x004DD43F`, `0x004DD714`–`0x004DD75B`; reference
  reader `0x005D9730`, null store `0x005D9742`) and written into zones §10.1. The original does not release the old
  references: the reader nulls the field and reads the new one, so the old grid and services leak one reference each.
  Releasing them (the bracket) is the implementer's choice, recommended.

<!-- from: From the zones glue and shadow (2026-10-14) -->
- ~~**ZG5:** ZoningChange `void1` / `IGZUnknown`. [SetVoid1 plus SetIGZUnknown on the zone manager; the message holds a
  reference]~~
  **ANSWERED 2026-10-14** from the exe (`PlaceZone` `0x004DDFDB` message slot 12, `0x004DDFE3` slot 33; standard
  message vtable `0x00A92D40`) and written into zones §5.3. `SetVoid1` stores the zone manager raw and `SetIGZUnknown`
  AddRefs it, so the message holds one reference. The bracket matches.

<!-- from: From the zones glue and shadow (2026-10-14) -->
- ~~**ZG6:** A dezone arriving during a PlaceZone. [runs at once and is counted]~~
  **ANSWERED 2026-10-14** from the exe (`DoMessage` `0x004DCA70`, no re-entry guard; `PlaceZone` `0x004DD780`)
  and written into zones §5.1. It runs at once. The InsertOccupant is delivered synchronously, so a network occupant
  inserted during a `PlaceZone` commit (for example a street drawn by the parcelling) runs the dezone `PlaceZone` nested
  inside it. The original counts nothing; the counting is glue, the implementer's choice.

<!-- from: From the zones glue and shadow (2026-10-14) -->
- ~~**ZG7:** When are `outLots` inserted? [at the pricing call]~~
  **ANSWERED 2026-10-14** from the exe (`PlaceZone` `0x004DDC94`–`0x004DDCC6`, after the pricing `DemolishLots`
  at `0x004DDC75`) and written into zones §6. The lots are inserted right after the pricing call, before the landfill
  cost and the funds test, so also on a dry run, after an abort and on error 1. The bracket matches.

<!-- from: From the zones glue and shadow (2026-10-14) -->
- ~~**ZG8:** What is released after a failed `Read`? [the grid, at Shutdown or destruction]~~
  **ANSWERED 2026-10-14** from the exe (`Read` `0x004DD415`–`0x004DD425`; `Shutdown` `0x004DC230`; `Release`
  `0x004DC441`–`0x004DC44E`; destructor body `0x004DCC70`; `Init` `0x004DE220`, `0x004DE26A`) and written into zones
  §10.1. A failed `Read` stops at the version and acquires nothing. The grid and services held are released by
  `Shutdown` only, which the final `Release` runs before the destructor; the destructor frees only the two count maps.
  The bracket matches, with "destruction" meaning the final `Release`.

<!-- from: From the zones glue and shadow (2026-10-14) -->
- ~~**ZG9:** Glue behaviour with switches off or calls from another thread. [`zones_budget` off: unlimited funds,
  nothing charged; no budget simulator: funds 0, withdraw false; PlaceZone from another thread without the lock:
  false, error 0; such a message: dropped; SetZoneDisplayAlpha: unchanged; GetZoneType fallback: 0 outside the grid]~~
  **ANSWERED 2026-10-14** from the exe (`PlaceZone` `0x004DD78A`; `GetZoneType` `0x004DC740`) and written into zones
  §6: no original behaviour; the implementer's choice stands. The original has no switches and no threads. Where it
  meets these cases it does not cope: `PlaceZone` calls the budget simulator with no null test (a crash without one),
  and `GetZoneType` reads outside the grid unchecked (§11.1).

<!-- from: From the zones glue and shadow (2026-10-14) -->
- ~~**ZG10:** Is the loaded flag set before the version check, even on failure? [yes; the core changed to do this]~~
  **ANSWERED 2026-10-14** from the exe (`Read` `0x004DD415`: the flag at +0x14 is set before the version read at
  `0x004DD41D` and the compare at `0x004DD420`) and written into zones §10.1. Yes: the flag is set first, also when the
  version check fails. The bracket matches.

<!-- from: From the zones glue and shadow (2026-10-14) -->
- ~~**ZG11:** Occupant manager slot 33. [called through the SDK overload, relying on MSVC's reversal of overloaded
  virtuals]~~
  **ANSWERED 2026-10-14** from the exe (occupant manager vtable `0x00A8F7F0` slots 28–35; slot 33 `0x0049DCE0`
  forwarding to slot 32 `0x0049E690`) and written into zones §6 and §12. Slot 33 is the SDK's typed
  `GetFirstOccupantByStandardCityCell(out, x, z, uint32)`, declared before its filter overload and placed after it by
  MSVC's reversal; every typed/filter pair in slots 28–35 follows the same pattern. The bracket matches; it holds for
  MSVC and clang-cl, not GCC.

<!-- from: From the zones glue and shadow (2026-10-14) -->
- ~~**ZG12:** Shutdown followed by the final Release. [Shutdown runs twice]~~
  **ANSWERED 2026-10-14** from the exe (`Release` `0x004DC441`–`0x004DC44E`; `Shutdown` guard
  `0x004DC234`–`0x004DC246`) and written into zones §1. Yes, `Shutdown` runs twice: the final `Release` calls it before
  the deleting destructor. The second call finds the initialised flag clear, does nothing and returns 1. The bracket
  matches.

<!-- from: From the zones glue and shadow (2026-10-14) -->
- ~~**ZG13:** Shadow: "our core's bookkeeping after it has loaded the same grid". [the core loads the original's own
  record through its Write]~~
  **ANSWERED 2026-10-14**: no original behaviour; the implementer's choice stands. The original has no shadow. For
  reference, its `Write` (`0x004DC7A0`) record (zones §10.1) carries the counts, both count maps, the tuning arrays and
  the alpha, but not the max slopes or the default alpha; `Init` re-reads the tuning after a load (§4, ZG3).

<!-- from: From the zones glue and shadow (2026-10-14) -->
- ~~**ZG14:** Is a PlaceZone dry run (commit=false, outLots null) on the original safe? `CanRemove(2)` is an inferred
  name. [yes, per §6 and §13.2; `CanRemove(2)` assumed to be a query]~~
  **ANSWERED 2026-10-14** from the exe (`PlaceZone` commit gate `0x004DDE5E`–`0x004DDE67`, `outLots` test
  `0x004DDC9B`; `DemolishRegion` `0x00468410`; `cSC4Occupant::CanRemove` `0x005E8AE0`) and written into zones §6 and
  §12. Yes, it changes no game state: no withdrawal, grid write, demolition, parcelling or message, and a null
  `outLots` is skipped. It writes only the caller's region, the occupant manager's shared filter type and the
  demolition utility's scratch fields. `CanRemove` is the SDK name and a pure query of a removal counter. The bracket
  matches. Not every callee of the pricing pass was traced.

<!-- from: From the zones glue and shadow (2026-10-14) -->
- ~~**ZG15:** Dry-run sample regions and flags (the spec names no UI caller). [four seeded regions: 1×1 checkSlope with
  size check, 2×3 checkSlope without size check and a lot inside, 4×4 the building-development form, 7×3 checkerboard
  without flags]~~
  **ANSWERED 2026-10-14**: no original behaviour; the implementer's choice stands. The original's own callers of
  `PlaceZone` in this spec (the dezone, §5.1; the BDS, §6) both commit. The UI zoning tool, not traced, is the likely
  dry-run caller; tracing its calls to slot 11 would show which flags it uses.

<!-- from: From the zones glue and shadow (2026-10-14) -->
- ~~**ZG16:** Are `*outCost` and `*outError` written on an early size-check failure? [the shadow fills the original's
  with 0 first; our core writes 0]~~
  **ANSWERED 2026-10-14** from the exe (`PlaceZone` outputs `0x004DE02B`–`0x004DE052`; ZG17's trace) and written into
  zones §6. Yes: both are written on every exit, the size-check failure included. `*outError` is 3 or 4, or 1 when the
  cost exceeds the funds; `*outCost` is 0 for every type except 14, which gets the landfill zoning cost of the whole
  region (0 with `skipFunds`). The shadow's pre-fill is harmless and is overwritten. Our core's 0 matches for every type
  but 14 (ZG17).

<!-- from: From the zones glue and shadow (2026-10-14) -->
- ~~**ZG17 — zones §6, in game (regress rzones, coordinator):** the zones shadow's 1×1 PlaceZone dry run (checkSlope,
  size check, commit=false) with zone type 14 fails with error 3 on both sides, but the original writes a cost
  (36–208, varying by month and place) and ours writes 0. 15 of 92 months differ, only in this case. What does the
  original write to `*outCost` when PlaceZone fails with error 3, and for which zone types? [ours: 0]~~
  **ANSWERED 2026-10-14** from the exe (`PlaceZone` `0x004DD780`: size failures `0x004DD82A`/`0x004DD846` jump to the
  abort path `0x004DDC31`; landfill cost `0x004DDCC8`–`0x004DDCF5`; funds test `0x004DDE34`; outputs
  `0x004DE02B`) and written into zones §6 (notes, worked example). **A failed size check is an abort, not an early
  return.** The cell loop is skipped but the post-loop block runs: for type 14 only, the cost is
  `landfillDeveloper.GetZoningCost(region)` of the whole request region (0 with `skipFunds`; every other type: 0).
  `*outCost` gets that cost and `*outError` 3 (or 4), unless the cost exceeds the funds, which turns the error into
  1. Also corrected: a type ≥ 16 is replaced by 0 (dezone), not a failure.

<!-- from: From applying the exemplar, zones draft and traffic-network answers (2026-10-14) -->
- ~~**ZQ13:** Does a real-mode Place empty the draft cache even when the Place fails, and before the solve? [yes, at
  the start of every real-mode Place; no vector depends on it]~~
  **ANSWERED 2026-10-14** from the exe (`PlaceNetworkOccupants` `0x00629210`: real-mode test and
  `cSC4NetworkWorldCache::Clear` `0x00647B20` at `0x00629327`–`0x00629334`) and written into zones.md (the draft
  cache's "Lifetime"). Yes: right after the bounds setup and before any solve, every real-mode Place empties the
  cache, and nothing before that point can return early, so a Place that then fails has emptied it too. **The
  bracket matches.**

## From applying the zones, landfill, budget and crime answers (filed by the coordinator from the `apply9b` lane's report, 2026-10-14)

apply9b applied ZG1–ZG17, LG27–LG29, BQ4, BQ7 and CQ23 (merged `969527d`). In game the zones shadow is exact (392/392,
296/296), and landfill game tiles insert with no failures (`gameTiles=12 gameTileFailures=0`; the tile exemplar is set
through the holder at occupant + 0x20).

<!-- from: From applying the zones, landfill, budget and crime answers (2026-10-14) -->
- ~~**ZG18:** ZG3: with no exemplar, or no Zone Display Alpha property, does `Init` still copy the alpha to the default
  alpha? [no: the saved alpha survives and the default alpha stays as it was (0xFF from the constructor); only the
  exemplar's value is copied]~~ **ANSWERED 2026-10-15 (re-answers10):** partly **DIFFERS**. With no exemplar the tuning reader returns before both alphas (`0x004DC4BB`): matches. With the exemplar present but no Zone Display Alpha, the copy to the default alpha still runs (`0x004DC675` is unconditional), so the default alpha becomes the current alpha (the saved byte after a load). Also, the constructor never sets the default alpha (only the alpha, 0xFF at `0x004DCEB8`), so 0xFF there is our choice. zones §10.1, §4.

<!-- from: From applying the zones, landfill, budget and crime answers (2026-10-14) -->
- ~~**ZG19:** ZG3 / §10.1: does `Read` set the default alpha? [no: the default alpha is not saved, so after `Read` and
  before `Init` it is 0xFF]~~ **ANSWERED 2026-10-15 (re-answers10):** MATCHES: `Read` writes only the alpha (`0x004DD590`); the default alpha is neither saved nor read. Its value before the first `Init` is uninitialised in the original; 0xFF is our choice. zones §10.1.

<!-- from: From applying the zones, landfill, budget and crime answers (2026-10-14) -->
- ~~**ZG20:** zones §15.2 still says "Type 16 → false", which conflicts with §6 and ZG17. [ZG17 followed: type 16
  dezones; `zn15_02` updated. The spec text needs the fix]~~ **ANSWERED 2026-10-15 (re-answers10):** the bracket MATCHES and the spec text was wrong: test 2 now says type 16 becomes type 0 and dezones with type 0's limits (zones §15 test 2, §6).

<!-- from: From applying the zones, landfill, budget and crime answers (2026-10-14) -->
- ~~**ZG21:** ZG2 and the §5.1 dezone: under the ZG2 choice, a network whose bounding cells cross the city edge has its
  whole dezone refused. Before, the inside cells were dezoned, and the original dezones them too while touching
  memory outside the grid. [refused whole, with error 0; not seen in game]~~ **ANSWERED 2026-10-15 (re-answers10):** **DIFFERS**. The original dezones every in-city cell of the rectangle and also touches zone-grid memory outside the grid (no clamp in the handler, in `GetBoundingCityCells` `0x005E8E20`, or in `PlaceZone`). Recommended choice for this internal call: clip the region to the city and dezone the inside cells; refusing the whole call leaves them zoned. zones §5.1.

## From lot creation, the plop test, the lot developer and the graphics formats (re-specs11 lane, 2026-10-15)

Open points from zones.md (§5.2, §7.1.1, §8.4, §8.5.1), lot-developer.md §17 and graphics-formats.md §3.9 and
§3.11. The current answer is in brackets; each entry says what would settle it. "Turbo run" means the in-game
harness with the FreeCity4 turbo clock and a message log.

<!-- from: From lot creation, the plop test, the lot developer and the graphics formats (2026-10-15) -->
- ~~**ZQ14:** who posts InsertOccupant with data4 = 1, 2 or 3? The zone developer joins dead-end stubs on 1 and 2
  (`JoinDeadEndStubs`, zones.md §5.2.1) and joins across a rail line on 3 (`JoinAcrossRail`, §5.2.2). [inferred:
  the network tools' real-mode Place, with a value per network kind; not traced. Settled by finding the callers of
  occupant manager slot 20 (`InsertOccupant`) that pass a non-zero data4, or in a turbo run by logging data4 of
  `InsertOccupant` messages while laying a road, a street and a rail line]~~ **ANSWERED 2026-10-15 (re-answers11):** **DIFFERS** (zones.md §5.2). data4 is `v` of occupant manager slot 20 (`0x0049EF60`, stored at `0x0049F205`–`0x0049F214`); the network tool passes its field `+0x208`: 1 for a network the player draws (all kinds), 3 inside tool slot 7 `PlaceNetworkOccupants` (`0x0062924A` sets 3, `0x00629980` resets 1), i.e. every network laid by code (zone developer, zone manager PlaceZone, network manager lot connections, tract developer, regional connections, BDS terrain change); 0 for the remove-and-reinsert in `InsertOccupantsForSolvedCells` (`0x00625AEE`) and other managers. Nothing passes 2. It depends on the call path, not on the network kind.

<!-- from: From lot creation, the plop test, the lot developer and the graphics formats (2026-10-15) -->
- ~~**ZQ15:** two values of the zone developer's preview and plop test are not explained. (a) The terrain view's slot 4
  (zones.md §7.1.1) takes seven arguments; the last two are always 3 and 0 here. (b) `CanCreatePloppedLot`
  (§8.4.1) rejects a cell whose network has every bit of `0x200000`, and blocks an outward connection to a network
  with any bit of `0x600000`. [(a) unknown, *[inferred]* a draw layer and a flag; the preview is UI and a headless
  replacement leaves it out. (b) the flag meanings belong to traffic-network.md (its Q3); the codes are reproduced
  from the bits without needing the names. Settled by the terrain view's slot 4 and by the network spec naming the
  two bits]~~ **ANSWERED 2026-10-15 (re-answers11):** (a) **DIFFERS** (zones.md §7.1.1): slot 4 is the textured-cell `MarkSelected` (`0x007526E0` → `0x00751A70`, pop 28); 3 is the blend (combiner) op, replaced by op 1 at alpha 0.5 when a renderer capability byte is set (`0x00751B57`–`0x00751B83`), and 0 means "do not clear the marked cells first" (`0x00751A7C`). Still UI only; leaving it out headless is fine. (b) **MATCHES** (the codes need only the bits), and the names are now in zones.md §8.4.1 and traffic-network.md Q3: `0x200000` = connection node (set by `CreateConnectionNode`, `0x00611486`); `0x400000` = pre-built-model piece not placed by the network tool (set by the `cSC4NetworkOccupantWithPreBuiltModel` constructor at `0x0061E2BC`, cleared by `InsertNetworkOccupant` at `0x006279F2`; name *[inferred]*).

<!-- from: From lot creation, the plop test, the lot developer and the graphics formats (2026-10-15) -->
- ~~**ZQ16:** lot-records.md §16 Q2 (vacate-reason values above 6 in saves) has an answer that lot-records.md does not
  carry yet. [**answered** in zones.md §8.5.1 *[inferred]*: the lot constructor `0x006C1E50` does not write the
  vacate reason (`+0x83`) and the pool's free path writes only the block's first dword, so a new lot inherits the
  byte of whatever last used its block, and a lot that has never been vacant saves that stale byte. The owner of
  lot-records.md should cite zones.md §8.5.1 under Q2. Settled for good by creating lots in a fresh city in a turbo
  run and reading field 17 of never-vacant lots: values that change with the order of earlier demolitions confirm
  it]~~ **ANSWERED 2026-10-15 (re-answers11):** **MATCHES**. Re-checked: the lot constructor `0x006C1E50` writes the byte at `+0x80` (`0x006C1F19`) but nothing at `+0x83`. lot-records.md §16 Q2 now cites zones.md §8.5.1.

## From the zone developer / lot manager glue, Stage A (filed by the coordinator from the `zones2` lane's report, 2026-10-15)

About `docs/specs/zones.md` §1.2, §7-§11.3 and the replacement `cFC4ZoneDeveloper` (`enable = zonedev`) with its shadow
(`zonedev_shadow = 1`); merged as `9a9b0df`, ABI 51. The lane numbered these ZONES-ZD1..ZD13. The current answer is in
brackets. ZG23, ZG24 and ZG27 were asked before re-specs11 merged; zones.md §8.5.1, §7.1.1 and §8.4.1-§8.4.3 now cover
them, and Stage B applies those sections.

<!-- from: From the zone developer / lot manager glue, Stage A (2026-10-15) -->
- ~~**ZG22:** lot manager slot 19 `GetLotSavvyCellAltitude`: does it write `*ok`? §8.7 does not say. [`*ok` left
  untouched]~~ **ANSWERED 2026-10-15 (re-answers11):** **DIFFERS** (zones.md §8.7, slot 19). When `ok` is non-null it is always written: 1 for a lot with flag bit 0 (`0x0072AAEF`), 0 for the terrain path (`0x0072AB06`).

<!-- from: From the zone developer / lot manager glue, Stage A (2026-10-15) -->
- ~~**ZG23:** lot construction (§8.5 step 1): the lot object's allocation and constructor arguments. [Stage A: `new_lot`
  builds nothing, CreateLot fails and is logged; **answered by re-specs11** in zones.md §8.5.1 (pool `0x00B0D3AC`,
  allocate `0x006BF430`, constructor `0x006C1E50`); Stage B implements it]~~ **ANSWERED 2026-10-15 (re-answers11):** Answered by re-specs11 (zones.md §8.5.1: pool `0x00B0D3AC`, allocation `0x006BF430`, constructor `0x006C1E50`); nothing to add. Stage B implements it.

<!-- from: From the zone developer / lot manager glue, Stage A (2026-10-15) -->
- ~~**ZG24:** `HighlightParcels`' `*out` region (§7.2, §13.2): its allocator and the caller that frees it are not
  traced. [`*out = null`, logged once; zones.md §7.1.1 now describes the highlight textures; check whether it also
  settles the region's owner]~~ **ANSWERED 2026-10-15 (re-answers11):** Answered (zones.md §13.2, the HighlightParcels row). The only caller that passes `out` is `cSC4ViewInputControlZone::DoCursorUpdate` (`0x004DBDCE`), which keeps the region in its `+0x58` and frees the previous one (rows and object) when it stores the next (`0x004DBDD1`–`0x004DBDF3`). `ClearHighlights` (`0x004DAE2D`) and `ComputeAndMarkSelectedArea` (`0x004DB734`) pass null. So the zone tool owns it; `*out = null` is safe only for a glue with no zone tool.

<!-- from: From the zone developer / lot manager glue, Stage A (2026-10-15) -->
- ~~**ZG25:** §5.2 step 3, the street-joining helpers `0x00731F60` / `0x00732450`, are not specified. [no-op plus a log
  line; zones.md §5.2.1-§5.2.2 (`JoinDeadEndStubs`, `JoinAcrossRail`, ZQ14) may cover them]~~ **ANSWERED 2026-10-15 (re-answers11):** Answered by zones.md §5.2.1 (`JoinDeadEndStubs`) and §5.2.2 (`JoinAcrossRail`), with the data4 values in §5.2 (ZQ14): 1 for player-drawn networks, 3 for networks laid by code. The no-op is now a gap Stage B can close.

<!-- from: From the zone developer / lot manager glue, Stage A (2026-10-15) -->
- ~~**ZG26:** the PlopAllLots / PlopAllBuildings cheats (§1.2 step 7): their bodies are not specified, and the host has
  no cheat-manager registration call. [not registered]~~ **ANSWERED 2026-10-15 (re-answers12):** MATCHES (zones.md §5.4, now specified). Both cheats are registered only when the debug-functionality flag (app slot 21) is set, so a shipped game without it never reaches them. `PlopAllLotTypes` (`0x0072C8E0`) places one lot per lot configuration in rows from (0, 0). `PlopAllBuildingTypes` (`0x0072BE50`) places one generic lot per building type on a spiral from the map centre. Both use `CreateLotIfPossible(…, true, false, cfg)` followed by `ConstructDefaultLot`. Leaving them unregistered is a safe *[choice]*.

<!-- from: From the zone developer / lot manager glue, Stage A (2026-10-15) -->
- ~~**ZG27:** slot 21 `CanCreatePloppedLot` (§14.3 Q2). [Stage A returns 0, logged; **answered by re-specs11** in zones.md
  §8.4.1-§8.4.3 (codes 0, 1, 3-8); Stage B implements it]~~ **ANSWERED 2026-10-15 (re-answers11):** Answered by re-specs11 (zones.md §8.4.1–§8.4.3), with the two network flags now named (ZQ15); nothing to add.

<!-- from: From the zone developer / lot manager glue, Stage A (2026-10-15) -->
- ~~**ZG28:** slots 11 and 24-26 leave EAX undefined in the original. [return true]~~ **ANSWERED 2026-10-15 (re-answers11):** MATCHES (choice). The original leaves EAX undefined, so no caller can depend on it; returning true is safe.

<!-- from: From the zone developer / lot manager glue, Stage A (2026-10-15) -->
- ~~**ZG29:** the budget and traffic glue walk the original lot manager's raw list (the list object at `lm+0x5C`). Our
  master list keeps the same raw layout and list object, but not at `+0x5C` inside our object. Does any caller read the
  member directly rather than through slot 15? [assumed none; not traced]~~ **ANSWERED 2026-10-15 (re-answers11):** MATCHES (zones.md §8.3). No load of the lot manager from `0x00B43D08`, or from city slot 33 (`+0x84`), is followed by a read of `+0x5C` anywhere in the executable, and the budget's tax pass reaches the list through slot 15 (budget.md BQ6). Every other class goes through the interface.

<!-- from: From the zone developer / lot manager glue, Stage A (2026-10-15) -->
- ~~**ZG30:** parcellization has no dry run (`HighlightParcels` draws, posts and places street drafts). [the shadow
  compares the lookups and CanCreateLot only]~~ **ANSWERED 2026-10-15 (re-answers12):** MATCHES (zones.md §7.1). The executable has no side-effect-free parcellization. `HighlightParcels` is the closest, but it still draws random numbers (§9), posts both DeferOccupantOperations messages and places street drafts. A shadow cannot call it without moving the zone developer's generator `0x00B4B398`. Comparing the lookups and `CanCreateLot` is the right scope.

<!-- from: From the zone developer / lot manager glue, Stage A (2026-10-15) -->
- ~~**ZG31:** a traffic insertion-point iterator from a foreign list at CreateLot (§8.5 step 4). [the lot is appended,
  logged]~~ **ANSWERED 2026-10-15 (re-answers12):** MATCHES (zones.md §8.5 step 4). The iterator is the traffic simulator's safe lot iterator, taken at its `Init` from slot 15 of the lot manager installed at that time (traffic-network.md §4.14). It is returned only in work-cycle state 1, which the cycle never enters, so it is null in practice and appending is exact. A foreign iterator is reachable only if the traffic simulator's `Init` ran against the original manager.

<!-- from: From the zone developer / lot manager glue, Stage A (2026-10-15) -->
- ~~**ZG32:** reference audit for the manager's last lot reference at DeleteLot / Shutdown. [`FC4_RELEASE_AT_DESTRUCTION`,
  as DG28]~~ **ANSWERED 2026-10-15 (re-answers12):** **DIFFERS** in timing (zones.md §1.2 Shutdown). The original deletes every lot in `Shutdown`, not in its destructor: `0x0072B8D0` (called at `0x00733663`) walks the cell map x outer, z inner, and calls its own `DeleteLot` (slot 24) on each non-null cell. That runs the full §8.6 sequence: zot prop, base texture, cells, master list, business unit, `lot.Shutdown()`, and the manager's release. The expected-release marker therefore belongs at `Shutdown`'s `DeleteLot`, as at any `DeleteLot`.

<!-- from: From the zone developer / lot manager glue, Stage A (2026-10-15) -->
- ~~**ZG33:** another thread that cannot take the lock within 20 ms. [neutral answers, except the snapshotted lot count
  and grid; counted as a fallback]~~ **ANSWERED 2026-10-15 (re-answers12):** Not an RE question; the bracket stands. The original lot manager has no lock: every caller runs on the simulation thread, so there is no original behaviour to match.

<!-- from: From the zone developer / lot manager glue, Stage A (2026-10-15) -->
- ~~**ZG34:** with `enable = zonedev` and CreateLot refused, the original tract developer's Grow (2026-01-30 on
  Konradshohe) gives two "over-release of game object [class 0xC9BD5D4A] at ReleaseToken" warnings (DeleteLot's
  manager release, §8.6 step 6) and then an access violation at `0x0070690C` / `0x00AB1995` (runs `zonedev-enable-1`,
  `-2`). Does the game's growth path tolerate a failed CreateLot after DeleteLot, or must lot creation never fail
  there? [not resolved; re-test after Stage B builds lots, then audit Grow's delete-and-recreate reference flow (slot 7
  caller references against the manager's); **Stage B (2026-10-15):** with lots built it does not reproduce (`zonedev-enable-final` clean, ref_warnings=0, 68 lots built, 0 refused); the refusal path is still logged. Open part: is a refused CreateLot on the growth path ever expected?]~~ **ANSWERED 2026-10-15 (re-answers12):** **Answered: lot creation must not fail there** (tract-developer.md §4.15.3). After a failed `CreateLotIfPossible`, `Build` returns 0 and the pass continues with the next candidate (§4.3). Candidate lot sets hold borrowed pointers (`GetLot`, no AddRef), and overlapping aggregation candidates share lots, so the next aggregation runs `PerformAggregation` step 1 on lots that were already deleted and freed. That gives the two over-release warnings at the second `DeleteLot` of the same lot, then the AddRef at `0x0070690C` through a stale vtable (`0x00AB1995` is data, not code). The original clears the rectangle itself first, so in practice step 4 succeeds there. Stage A must not let Grow reach aggregation while it cannot create lots; Stage B's working `CreateLot` removes the cause. Stage B's clean run (`zonedev-enable-final`) agrees: with `CreateLot` working the path is not reached, so a refused CreateLot on the growth path is never expected.

## From the zone developer / lot manager glue, Stage B (filed by the coordinator from the `zones2` lane's report, 2026-10-15)

About `docs/specs/zones.md` §5.2.1-§5.2.2, §7.2.2, §8.4.1-§8.4.3, §8.5.1 and their implementation in `cFC4ZoneDeveloper` /
`FC4ZoneDevHost` (`enable = zonedev`); merged as `3cb9b43`, ABI 55. The lane numbered these ZONES-ZD1..ZD19; ZD1-ZD13
repeat ZG22-ZG34. Stage B did not apply two re-answers11 answers: ZG22 (slot 19 always writes `*ok`; ours leaves it
untouched) and ZG24 (the zone tool owns the highlight region; ours returns null with the zone tool present). The current
answer is in brackets.

<!-- from: From the zone developer / lot manager glue, Stage B (2026-10-15) -->
- ~~**ZG35:** §8.5.1: the lot constructor does not write the vacate reason at `+0x83`, and the pool block is not zeroed, so a
  new lot carries a stale byte there. [choice: FreeCity4 writes 0 after construction]~~ **ANSWERED 2026-10-16 (re-answers22):** MATCHES the spec's *[choice]* (zones.md §8.5.1). Re-checked: the constructor `0x006C1E50` writes `+0x80`, `+0x81`, `+0x82`, `+0x84` and `+0x88` but never `+0x83` (`0x006C1F19`–`0x006C1F5B`), so the original's byte is whatever the pool block held; writing 0 for lots FreeCity4 creates is the documented *[choice]*, and a loaded lot keeps its byte.

<!-- from: From the zone developer / lot manager glue, Stage B (2026-10-15) -->
- ~~**ZG36:** §8.4.1 `RotateLotRect` with a facing outside 0..3. [choice: the rect is left unrotated]~~ **ANSWERED 2026-10-16 (re-answers22):** *[choice]* confirmed (zones.md §8.4.3). For a facing above 3 (unsigned, `0x004BDFDE`) `RotateLotRect` writes nothing, and both callers then use a stack rect that the original leaves as it was (never set before the call in `GetNetworkObjectPlacement`, `0x0072AD70`): undefined, not "unrotated". The only caller chain (the plop tool) passes the lot's facing 0…3, so leaving the rect unrotated is a safe *[choice]*.

<!-- from: From the zone developer / lot manager glue, Stage B (2026-10-15) -->
- ~~**ZG37:** §8.4.2 / §8.4.3: a constraint or network configuration object whose data vector has fewer values than the
  check reads. [choice: missing values read as 0]~~ **ANSWERED 2026-10-16 (re-answers22):** *[choice]* confirmed (zones.md §8.4.3). `GetNetworkObjectPlacement` reads v[0], v[1], v[2] with no size check (`0x0072ADB7`–`0x0072ADD0`); a shorter vector reads past its end in the original. Shipped data never does: all 41 type-7 records carry 3 or 4 extra values; the 43 type-5 records carry 0 or 1 (the `p0` rule handles 0) and the 17 type-6 records none. Missing values as 0 is a safe *[choice]*.

<!-- from: From the zone developer / lot manager glue, Stage B (2026-10-15) -->
- ~~**ZG38:** §8.7 `PositionToCell`: the core computes `floor(p / 16)`; does the original round differently at negative
  positions or at the map edge? [floor]~~ **ANSWERED 2026-10-16 (re-answers22):** MATCHES (zones.md §8.7). City slot 95 (`0x004599E0`) is an exact floor per axis: `t := x × 0.0625`, `c := round-half-even(t − 0.5)` by the 1.5·2²³ idiom, plus 1 when `t = c + 1` exactly. That is ⌊t⌋ for every finite |t| < 2²², negative positions included (−0.1 → −1, −16 → −1, −16.1 → −2), with no clamp at the map edge. Only a NaN differs (a meaningless large value).

<!-- from: From the zone developer / lot manager glue, Stage B (2026-10-15) -->
- ~~**ZG39:** §1.2 Shutdown and §5.2 PostCityInit: `GetNetworkTool(type, 1)` returns a tool whose only reference is the zone
  developer's AddRef, so Shutdown's Release frees it (the lane's game runs show this). The spec does not state the
  ownership. [`FC4_RELEASE_CREATED`]~~ **ANSWERED 2026-10-16 (re-answers22):** MATCHES `FC4_RELEASE_CREATED` (network-manager.md §7.1, zones.md §1.2). `GetNetworkTool(type, true)` returns a freshly constructed tool with reference count 0 (base constructor `0x0062C5F0` clears `+0x14` at `0x0062C617`; no AddRef in the factory, `0x00610907`–`0x00610910`); the zone developer's AddRef at PostCityInit is the only reference, and tool `Release` (`0x00623270`) destroys it at count ≤ 1, so the developer's `Shutdown` Release frees it.

<!-- from: From the zone developer / lot manager glue, Stage B (2026-10-15) -->
- ~~**ZG40:** `UpdateHighlightTextures` (§7.1.1), which the spec allows to be omitted, is not implemented. [omitted]~~ **ANSWERED 2026-10-16 (re-answers22):** MATCHES (zones.md §7.1.1). The function changes no simulation state and draws nothing; omitting it only removes the zoning preview textures while the zone tool drags, a visible UI difference in game and nothing else.

<!-- from: From the return-channel audit (2026-10-16) -->
- ~~**ZG41:** lot manager slot 24 DeleteLot is "EAX undefined; error: no bool result" in zones.md §8.6 and §11.3, but
  lot-developer.md (cross-reference table near line 1659) and tract-developer.md's interface table (line 1164) list it
  as AL. Those two tables should be corrected. [our hosts ignore it and return true (F3)]~~
  **ANSWERED 2026-10-27 (re-q-misc):** MATCHES — zones.md is right: `DeleteLot` (slot 24, `0x0072F8E0`) has no defined result; in real mode its last action is the lot's `Release` (so EAX is a remaining reference count, 0 when the lot was freed), in highlight mode an unrelated leftover (zones.md §8.6, §11.3). lot-developer.md's dependency table is corrected; tract-developer.md's interface table ("AL") is wrong and still to be fixed outside this batch.

<!-- from: From the return-channel audit (2026-10-16) -->
- ~~**ZG42:** (tract-developer.md, the lane's "Q8") the tract-developer.md interface table gives lot configuration manager
  slots 18/19 (Register/UnregisterLotConfigurationFilter) as AL, but prop-lotconfig.md §B7 gives "—" (no defined
  return). Which is right? [no defined return; the result is not tested (F6)]~~
  **ANSWERED 2026-10-27 (re-q-misc):** MATCHES prop-lotconfig.md §B7 — slots 18/19 of the lot configuration manager have no defined return. `RegisterLotConfigurationFilter` (`0x0048A660`) inserts the filter into an ordered set and returns with a pointer to its own stack temporary in EAX; `UnregisterLotConfigurationFilter` (`0x0048A3F0`) leaves whatever its find/erase left. tract-developer.md's "AL" is wrong (outside this batch). Recorded in zones.md §12.
