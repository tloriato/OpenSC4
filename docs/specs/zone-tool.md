# Spec: the zone tool (`cSC4ViewInputControlZone`)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`). Implementers must work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled
or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[compat-limit]* depends on something FreeCity4 cannot reproduce, so only the rule is matched.

Related specs: [zones.md](zones.md) (§6 `PlaceZone`, §7.1 `HighlightParcels`, `SetOptions`, §8.4 `CanCreateLot`,
the zone types), [city-tools-input.md](city-tools-input.md) (§2 the base control), [tutorial.md](tutorial.md)
(§6 the tutorial slots), [water-ripples.md](water-ripples.md) (§2.2 `MarkSelected`), [ui-windows.md](ui-windows.md)
(the city view's cursor text and cost indicator slots).

## 0. Summary

The zone tool zones (or, as type 0, dezones) the rectangle the player drags, or, when the player holds the button
still on one cell, the connected eligible cells around it. While dragging it previews the result with a dry run of
the zone manager's `PlaceZone`, marks the cells, shows the cost and a message, and on release places the zone for
real. Shift, Ctrl and Alt are passed to the zone developer as its parcel options.

## 1. Making it *[compat]*

- Made by the city view's `InvokeZoningTool` (`0x007E6DA0`). Construction (`0x004DAE80`) with the zone type `T`
  (`+0x3C`, zones.md's type numbers: 0 dezone, 14 landfill …): id `0xA766C101`; cursor := the table `0x00A92C40`
  entry for `T`.
- `Init` (`0x004DA6C0`): base `Init`; the terrain's cell width `w` (`+0x6C`) and terrain view (held); the local grid
  helper; the zone manager (`0x00B43D14`), the lot manager (`0x00B43D08`, the zone developer's `cISC4LotManager`) and
  the sound system, held; notification `0x277A9364` (the grid display toggle); the effect `local_tile_outline`
  (effects manager slot 7).
- Sounds, by `T` (start-sound event 1; none for types 10–13 and 15):

  | Moment | Types 1–9, 14 | Type 0 |
  |---|---|---|
  | press (`0x00A92B40`) | `0xCA5C328B` | `0xCA5C328B` |
  | a valid preview (`0x00A92B80`) | `0x0A5C32AB` | `0x0A5C32AB` |
  | a valid release (`0x00A92BC0`) | `0x8A5C329D` | `0x8A5EC5B8` |
  | an invalid preview or release (`0x00A92C00`) | `0x495A6D86` | `0x495A6D86` |

## 2. Input *[compat]*

- **Options.** Every press, release and move first calls the zone developer's `SetOptions(Alt, not Shift, Ctrl)`
  (modifier bits 2, 0 and 1; zones.md slot 7).
- **Press** (`0x004DB980`): false without the city view. The outline effect is stopped. `PickTerrain(x, y, &p, 0)`
  (city view slot 36); on a hit: the press sound; dragging (`+0x40`); keyboard scrolling stopped (slot 35); the mouse
  captured; the start and end cells := `(trunc(p.x / w), trunc(p.z / w))`; the terrain view's cell cursor off (old
  setting kept); the preview (§3); animation ticks requested (`0xC0`); the hold time (`+0x54`) := 0; any flood
  region dropped; the local grid helper hidden.
- **Move** (`0x004DBE40`): the local grid helper follows a terrain hit; the outline effect is placed on the hit
  cell's corner (`floor(p / w) × w`, scaled by `w`), started while not dragging and stopped while dragging. While
  dragging: if modifier bit 4 is clear (the button is up) the input ends (`EndInput`); otherwise a hit in another
  cell becomes the end cell, then the hold check (§2.1) and the preview run. True while dragging.
- **Release** (`0x004DBAF0`), while dragging: a hit sets the end cell; the preview runs. Invalid: the failure sound.
  Valid: the success sound, then `PlaceZone` (§4) on the dragged rectangle, or on the flood region when there is one;
  the cost indicator is cleared (slot 44) with the result. Then: the cursor text cleared (slot 41, `0xA9C87CF4`); the
  highlights cleared; the outline effect started; the grid helper shown unless the terrain view's grid display is on
  (terrain view slot 20); the terrain selection cleared and the cell cursor setting restored; ticks released; the
  flood region dropped; capture released.
- **Keys** (`0x004DB8B0`, `0x004DB920`), only while the tool is current: Escape while dragging ends the input (true);
  any other key down or up while dragging re-applies the options and re-runs the preview.
- **`EndInput`** (`0x004DB020`): not dragging, capture and ticks released; the flood region dropped; the cost
  indicator cleared; the cursor text cleared; the highlights cleared; the grid helper hidden; the terrain selection
  cleared. **`OnMouseExit`** (`0x004DA9B0`) releases the capture, stops the outline, hides the grid helper and clears
  the cursor text. **`Shutdown`** (`0x004DA5A0`) ends the input, clears the cursor text and the cost indicator,
  removes the notification and releases what `Init` took.

### 2.1 Holding still: the flood region

- **Animation ticks** (`DoMessage`, `0x004DC110`, message `0x86AD10EE`) add the tick's time (message slot 19) to the
  hold time and run the hold check. `0x277A9364` shows the grid helper when the terrain view's grid display is off
  and the tool is not dragging, else hides it. Before `Init` both are ignored.
- **Hold check** (`DoCursorUpdate`, `0x004DBD40`): while the end cell equals the start cell, once the hold time
  exceeds 750 and there is no flood region yet, the flood region is made (`FloodFillRegion`); for a zone type ≠ 0 it
  is replaced by the zone developer's `HighlightParcels(region, T, &start, &out)` result (slot 5); then the preview
  runs. As soon as the end cell differs, the hold time := 0 and the flood region is dropped.
- **`FloodFillRegion(start)`** (`0x004DAA20`): a breadth-first fill over the four neighbours, inside the city, of
  the cells `TestCell` accepts; the result is a cell region (rectangle and bitmap) or none when the start cell is
  refused.
- **`TestCell(c, start, 8, list)`** (`0x004DA880`): `c` must not be in the list and must lie within 7 cells of the
  start on both axes (`|dx| < 8`, `|dz| < 8`). Dezoning: the cell's zone type (zone manager slot 12) must not be 0,
  15, 11 or 12. Zoning: the zone type must be 0 and `CanCreateLot(x, z, 1, 1, true, true, null)` (lot manager slot
  20) must return 8 (free).

## 3. The preview (`ComputeAndMarkSelectedArea`, `0x004DB240`) *[compat]*

1. The rectangle `R` of the start and end cells (min and max, `+0x5C`–`+0x68`); the highlights are cleared.
2. The dry run: `PlaceZone(region, T, commit false, skipFunds false, checkSlope true, skipSize := whether a flood
   region is used, checkLotInside := T ≠ 0, &cost, &error, &lots)` (zones.md §6), the region being the flood region
   or `R`.
3. Each lot in `lots` with a building gets its building's occupant highlighted (occupant slot 17 with 5; the
   highlights are later reset with 0) and counted; when none has a building but one answers lot slot 46 with
   non-zero, the count is 1.
4. **Valid:** the valid-preview sound; the zone's texture (zone manager slot 14) and drag colour value (slot 15
   with 0); the terrain selection cleared; `SetCostIndicator(cost)` (slot 43); with a count, the cursor text
   (`0xA9C87CF4`, slot 40) := the string `0xCA711A5E` (group `0x0A419226`) formatted with the count, set only when it
   changes; without, the cursor text is cleared. Then the marks: for type 0 each cell of the region (`R`, or the
   flood region's set cells), and for type 14 each set cell of the region inside `R`, is marked with the zone's
   texture (terrain view slot 5, values `0, 0, 4`, the colour value, `0`); every other type is drawn by the zone
   developer's `HighlightParcels(region, T, null, null)`.
5. **Invalid:** the failure sound; `MarkSelected(R, 0, true)` (terrain view slot 7); the cursor text := the failure
   reason for `error` (below), when it changes; for type 0 with error 1 the cost indicator shows `(cost, 0)` (slot
   42), otherwise it is cleared.
6. Returns whether the dry run succeeded.

**Failure reasons** (`GetFailureReasonString`, `0x004DB0D0`; strings of group `0x0A419226`): 0 none, 1 `0x6A711976`
(funds), 2 `0x6A711A47` (landfill on water), 3 `0xAA711A4E` (too small), 4 `0x8A711A54` (too large), 5 `0x6A711A59`
(blocked); other codes leave the text unchanged.

## 4. Placing (`PlaceZone(T, region, flood, skipSize, outCost, outError)`, `0x004DA7A0`) *[compat]*

- **Tutorials:** when the tutorial system (`0x00B43D38`) reports a tutorial (slot 8) whose current task (slot 10)
  has a rectangle with a non-negative first value (task slot 5), a flood placement, or a rectangle that
  `0x004461C0` reports against the task's *[inferred: one reaching outside it]*, is refused: `OnTutorialEvent(1)`
  (slot 18) and false.
- A flood placement stores the region's rectangle as the start and end cells.
- Then `PlaceZone(region, T, commit true, skipFunds false, checkSlope true, skipSize, checkLotInside := T ≠ 0,
  outCost, outError, null)` (zone manager slot 11). The release passes `skipSize` false for a rectangle and true for
  a flood region.

## 5. Quirks

1. The flood fill reaches at most 7 cells from the start cell in each direction (§2.1).
2. A hold of more than 750 tick units (milliseconds *[inferred]*) on one cell replaces the dragged rectangle by the
   flood region (§2.1).
3. During tutorials with a task rectangle the flood placement is always refused (§4).
4. Landfill (type 14) marks cells one by one like dezoning, every other zone through the zone developer (§3).

## 6. Open questions

See [questions/zone-tool.md](questions/zone-tool.md): ZT1 (lot slot 46, the drag colour value, the meaning of
terrain view slot 5's values here, and the test `0x004461C0`).

## 7. Conformance tests

1. **Options:** Shift held at a press calls `SetOptions(false, false, false)`; Ctrl alone `SetOptions(false, true,
   true)`.
2. **Flood bounds:** from start (20, 20) a cell (27, 20) is refused and (26, 26) may be accepted.
3. **Dezone flood:** a plopped cell (zone type 15) is not taken by a dezoning flood.
4. **Failure text:** a dry run returning error 3 sets the cursor text to string `0xAA711A4E`.
5. **Release:** a valid rectangle release calls `PlaceZone(region, T, true, false, true, false, T ≠ 0, …)`.
