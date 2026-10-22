# Spec: the bulldozer tool (`cSC4ViewInputControlDemolish`)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`). Implementers must work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled
or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[compat-limit]* depends on something FreeCity4 cannot reproduce, so only the rule is matched.

Related specs: [demolition.md](demolition.md) (§9, the calls this tool makes), [overlay-views.md](overlay-views.md)
(§2 the marked-cell view), [city-tools-input.md](city-tools-input.md) (§2), [zones.md](zones.md) (the lot
manager's `GetLot`), [occupant-manager.md](occupant-manager.md) (slot 41), [render3d.md](render3d.md) (§11
picking).

## 0. Summary

The bulldozer previews and demolishes what the player clicks or drags over. A click takes the lot, occupant or
cell under the mouse (a signpost alone); a drag takes the dragged rectangle, grown to whole lots for lots larger
than a cell that reach out of it. The preview is a dry run of the demolition utility; the cells are drawn by a
marked-cell view in one colour pair when the demolition is possible and another when not.

## 1. Making it *[compat]*

- `Init` (`0x004B8AB0`): id `0x46DDB5F1`, cursor `0xE1855ADC`; base `Init`. Once: a marked-cell view
  (overlay-views.md §2) that does not ignore depth, uses the water altitude when the terrain shows its water surface
  (terrain slot 83), with cells of 16 × 16, added to the renderer at layer 5, priority 1000 (slot 32); the terrain
  view's automatic cell cursor off (slot 10). Held: the budget (`0x00B43D64`), the city (`0x00B43CEC`), the
  demolition utility (`0x00B43D00`), an object of the building development simulator (slot 36), the lot manager
  (`0x00B43D08`), the occupant manager (`0x00B43D0C`) and the tool's filter (§1.1). Four colours (four floats each)
  are copied from the renderer's draw context (its `+0x54` object): the valid outline (`+0x50`), the invalid outline
  (`+0x60`), the valid fill (`+0x90`) and the invalid fill (`+0xA0`).
- `Shutdown` (`0x004B8F30`): ends the input, clears the selection, removes the marked-cell view from the renderer
  (slot 33) and releases it and everything `Init` held; the cost indicator is cleared.

### 1.1 What can be bulldozed (`cSC4DemolishFilter`, `0x004B8880`)

Flora (`0x74758926`), buildings (`0x278128A0`), power poles (`0x2890D4DE`) and network occupants (`0xC772BF98`),
when `CanRemove(2)` (occupant slot 14) holds; at view level 3 (the city view's slot 22) a network occupant
(interface `0xA821EF94`) with any network flag of `0x1F4F` (the surface networks, network-rules.md) is refused.
Props (`0xA823821E`) only when they are signposts (prop type `0xAB72FBB3`).

## 2. Picking *[compat]*

- **Cell** (`DoCellPick`, `0x004B8980`): the city view's `PickTerrain(x, y, &p, water)` with `water` := the water
  surface state, then the city's `PositionToCell`.
- **Occupant** (`PickDemolishableOccupant`, `0x004B8A00`): the renderer's `Pick(x, y, filter, &instance)` (slot
  65); the instance's occupant (instance slot 3) answering `0x26BB767E` is the pick (`+0x70`), and whether it is a
  signpost (`0x4B44FBE2`) is kept (`+0x94`).
- **Lot** (`DoLotPick(x, y, &rect)`, `0x004B8D10`; false without the city view): an occupant pick gives its
  position, otherwise a terrain pick (false on a miss). With the lot at that cell (lot manager slot 6) when the lot
  answers its slot 39 and either has no building, or the view filter (city view slot 18) is missing, or the filter
  accepts the building: `rect` := the lot's rectangle (lot slot 14). Otherwise: a picked network occupant
  (`0xA8C86142`) gives its cell (its slot 8); another picked occupant its cell rectangle (its slot 11); nothing picked,
  the cell. True.

## 3. Input *[compat]*

- **Press** (`0x004B9AE0`): pressed (`+0x44`) := the cell pick of the start cell (`+0x58`). On a hit: the mouse is
  captured, the lot pick fills the lot rectangle (`+0x48`), the end cell (`+0x60`) := the start, a kept set is
  emptied, and the selection is updated (§4). Always the press sound `0xCA5C328B` (`PlayResultSound`). True.
- **Move** (`0x004B9B70`), on a cell hit: when not pressed, a new cell or a different picked occupant (from a lot
  pick) makes the start cell the hovered one. Then a new end cell updates the selection; otherwise a change seen
  while hovering does. True.
- **Release** (`0x004B9C50`, false unless pressed): the end cell is picked again and the selection updated. When the
  preview succeeded: the budget's funds := funds − the previewed cost (budget slots 6 and 5, no expense line); then,
  for a click (start = end) on a picked occupant: a signpost → `DemolishOccupant(true, o, 1, 1, &cost, false, null)`;
  another occupant → `DemolishRegion(true, region, 1, 1, false, null, &cost, null, o, x, y)`; otherwise
  `DemolishRegion(true, region, 1, 1, false, null, &cost, null, null, 0, 0)` (demolition.md §9). The cost indicator
  is cleared (true); a real cost of 0 plays `0x8A5EC5B8`. When the preview failed: the cost indicator is cleared and
  `0x495A6D86` plays. Then the selection is cleared, not pressed, capture released. True.
- **Escape** while pressed (and the tool is current) ends the input. `EndInput` (`0x004B9040`): when pressed, not
  pressed, capture released, selection cleared; the cost indicator cleared. `OnMouseExit` (`0x004B9020`) clears the
  cost indicator and the selection.

## 4. The selection (`UpdateSelectedRegion`, `0x004B93B0`) *[compat]*

1. **The rectangle:** for a drag (start ≠ end), `R` := the start and end cells' rectangle; for a click, `R` := the
   lot rectangle (§2).
2. The old region's cell count is kept and the old region dropped; the view filter is taken (city view slot 18).
3. **Whole lots:** every lot under a cell of `R` that is larger than one cell (lot slot 33 > 1), and whose building,
   if any, the view filter accepts, and that reaches out of `R` (lot slot 14) grows a union rectangle. Without such a
   lot the region is `R` with every cell set; with one, the region is the union rectangle with the cells of `R` set
   and every cell of such a lot set.
4. **The preview:** a picked signpost → `DemolishOccupant(false, o, 1, 1, &cost, false, null)`; otherwise
   `DemolishRegion(false, region, 1, 1, false, null, &cost, null, null, 0, 0)`. The result is the validity (`+0x6C`).
5. **Highlight:** the previous highlighted occupant is reset (occupant slot 17 with its handle and 0) and dropped.
   For a click: the picked signpost, or the building of the lot at the lot cell, or, without a lot, the first
   network occupant at that cell (occupant manager slot 41, type `0xC772BF98`) that the filter accepts, is
   highlighted: its handle := occupant slot 16, then occupant slot 17 with 5.
6. **Drawing:** the marked-cell view gets the valid or invalid fill (`SetCellColor`) and outline (`0x007DC2F0`,
   which also turns the outline on); its cells are cleared (`0x007DCD00`) and the region's set cells added
   (`0x004B9250`).
7. `SetCostIndicator(cost, 0)` (city view slot 42).
8. When the region's cell count changed: a valid preview while pressed plays `0x0A5C32AB`; an invalid one plays
   `0xAA62C9D5`.

`ClearSelectedRegion` (`0x004B8ED0`): the highlight is reset and dropped, the view's cells cleared, the region
dropped.

## 5. Quirks

1. The press sound plays even when the press misses the terrain (§3).
2. A click on a signpost demolishes only that sign; a click elsewhere demolishes the lot's whole rectangle (§3, §4).
3. Lots larger than one cell are taken whole as soon as the drag touches them (§4).
4. The funds are charged from the preview's cost before the real demolition runs, whatever that costs (§3).

## 6. Open questions

See [questions/bulldozer-tool.md](questions/bulldozer-tool.md): BT1 (lot slots 33 and 39, occupant slots 11, 16,
17 here, and the building development simulator object the tool holds).

## 7. Conformance tests

1. **Drag rectangle:** dragging from cell (5, 5) to (3, 8) previews the rectangle (3, 5)–(5, 8).
2. **Whole lot:** a 2 × 2 lot at (5, 5)–(6, 6) under a drag of (4, 4)–(5, 5) grows the region to (4, 4)–(6, 6).
3. **Filter:** at view level 3 a road occupant is refused; a pipe is accepted.
4. **Charge:** a preview cost of 120 with funds 1000 leaves 880 before the real demolition.
5. **Signpost:** a click on a signpost calls `DemolishOccupant` with it, not `DemolishRegion`.
