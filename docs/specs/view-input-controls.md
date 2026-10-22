# Spec: view input controls (scrolling, levelling, labels and signs)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`). Implementers must work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled
or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[compat-limit]* depends on something FreeCity4 cannot reproduce, so only the rule is matched.

Related specs: [ui-windows.md](ui-windows.md) (the city view's slots 24–36), [render3d.md](render3d.md) (§11
picking), [terrain.md](terrain.md) (`GetAltitude`, `SetAltitude(rect, …)`), [water-ripples.md](water-ripples.md)
(§2.2 `MarkSelected`), [prop-developer.md](prop-developer.md) (`ShowSignsAndLabels`, signposts),
[network-rules.md](network-rules.md) (§1 edge bytes), [network-tools.md](network-tools.md) (§13.1 prop occupant
slots), [modal-dialogs.md](modal-dialogs.md), [audio.md](audio.md) (§7.3 sound events).

## 0. Summary

A **view input control** is a tool: the city view (`cSC4View3DWin`) hands it the mouse and keyboard while it is
current. This spec covers five tools built on the base class of [city-tools-input.md](city-tools-input.md) §2:
right-drag **scrolling**, the **levelling** tool, the **label** tool (text painted on the ground), the **sign** tool
(signposts) and the bulldozer for signs and labels. The other tools (query, lot placement, zones, networks,
demolition, U-Drive-It…) are left to their own specs.

## 1. The base control (`cSC4ViewInputControl`, vtable `0x00AA6658`) *[compat]*

The base control is specified in [city-tools-input.md](city-tools-input.md) §2: its interface and slot table, its
fields (`+0x04` initialised, `+0x0C` id, `+0x10` cursor id, `+0x14` cursor, `+0x18` window, `+0x1C` the city view,
`+0x20` the window manager; each tool's own fields from `+0x24`), `Init`, `Shutdown`, `SetCursor`, capture,
`EndInput` and `PlayResultSound`. The tools below use that layout.

## 2. Right-drag scrolling (`cSC4ViewInputControlCityScroll`) *[compat]*

### 2.1 Start

The city view's right press (`GZOnMouseDownR`, `0x007E6A00`) first offers the press (slot 17) to the persistent
controls, then to the stacked current controls, each list from its last entry back; the first that returns true
takes it. Otherwise the view keeps the press point, makes a scroll control, makes it current
(`SetCurrentViewInputControl(control, false)`) and calls its `SetMouseScroll(x, y)`.

### 2.2 The control

- **Construction** (`0x004B7DC0`): inner threshold `+0x40 := 12`, outer threshold `+0x44 := 80` (pixels).
- **`Init`** (`0x004B82A0`): id `0x48E2A1F8`, cursor `0xC2A676AC`; the tunables (`ReadTunableValues`,
  `0x004B8220`): from the exemplar (`0x6534284A`, `0x690F693F`, `0xE90F6C84`), property `0xE90F5DE4` replaces the
  inner threshold and `0xA90F5DEC` the outer one, when present. Then the base `Init`; the control records whether
  the mouse was already captured (`AmCapturing`, `+0x39`) and captures it if not.
- **`SetMouseScroll(x, y)`** (`0x004B82F0`): the anchor `(+0x28, +0x2C) := (x, y)`. The first time, the anchor
  picture is loaded (type `0x856DDBAC`, group `0x46A006B0`, instance `0x094AC89A`), given the colour (255, 0, 255)
  *[inferred: as its transparent colour]* and put in a picture window (id `0x48E945B4`) centred on the anchor, flag 2
  off and flag 4 on, shown, and added to the city view's window (`ChildAdd`): brought to the front on success,
  destroyed (window manager slot 23) otherwise.
- `IsSelfScrollingView` is true.

### 2.3 `OnMouseMove(x, y, modifiers)` (`0x004B7EF0`)

1. If the physical right button is up (`GetAsyncKeyState`; the left one when the buttons are swapped), the control
   behaves as on a right release: `OnMouseUpR` calls `EndInput` (§2.4) and returns true, and so does the move.
2. `dx := −trunc((anchorX − x) / 2)`, `dz := −trunc((anchorY − y) / 2)` (half the mouse offset, rounded toward 0);
   with `i` and `o` the inner and outer thresholds, each axis keeps a speed: `±o` (by sign) when `|d| > o`, `d` when
   `i < |d| ≤ o`, else 0 (`+0x30` for x, `+0x34` for z; the control never reads them again).
3. The cursor: when no `|d|` exceeds `i`, `0xC2A676AC`; otherwise by region (`dz ≥ i` below, `dz ≤ −i` above,
   `dx ≥ i` right, `dx ≤ −i` left):

   | | left | middle | right |
   |---|---|---|---|
   | above | `0x02A67691` | `0x62A67606` | `0x82A676A8` |
   | middle | `0x62A67694` | `0xC2A676AC` | `0xC2A676A4` |
   | below | `0xA2A67698` | `0x62A6769B` | `0x22A676A0` |

   A cursor other than `0xC2A676AC` sets the "has scrolled" byte `+0x38`. A change of cursor id calls
   `SetCursor(id)`.
4. `speed := min(1, sqrt(dx² + dz²) / (o − i))`. With a scrolling cursor, the angle is `atan(dz / dx)` for `dx > 0`
   (0 when `dz = 0`), `atan(dz / dx) + π` for `dx < 0` (π when `dz = 0`), and for `dx = 0`, 3π/2 when `dz < 0`, else
   π/2; scrolling is on. Otherwise the angle is 0 and scrolling is off.
5. The city view's `SetScrolling(on, angle, speed)` (slot 33). Return false.

### 2.4 End

`EndInput` (`0x004B7E70`): the city view's `ScrollStop` (slot 34), the anchor window (`GetChildWindowFromID`)
deleted from the city view's window (`ChildDelete`), then the base `EndInput`. `Shutdown` (`0x004B7E00`, when
initialised) does the same without removing the control, releases the capture unless it was already held at `Init`,
releases the picture and calls the base `Shutdown`.

## 3. The levelling tool (`cSC4ViewInputControlLevelTerrain`) *[compat]*

- Made by the city view's `RunGlobalTerrainTool` (`0x007E75B0`) for command 7 and made current. Construction
  (`0x004BDC60`): id `0x88CEA620`, cursor `0x071E5703`. `Init` (`0x004BDC90`): base `Init`, then the terrain
  (`0x00B43CF4`, held), its cell width `w` (slot 6) and its terrain view (slot 66, held). Keys are passed on.
- **Press** (`0x004BDCE0`): false without the city view. The city view's `PickTerrain(x, y, &p, 0)` (slot 36); on a
  hit: dragging; the mouse is captured; the start and end cells are `(trunc(p.x / w), trunc(p.z / w))`; the terrain
  view's automatic cell cursor is turned off (slot 10), its old setting kept. Returns true, hit or not.
- **Move** (`0x004BDE30`), while dragging: a hit in another cell (here `trunc(p × (1 / w))`) becomes the end cell and
  the area is re-marked. Returns true while dragging, else false.
- **Marking** (`ComputeAndMarkSelectedArea`, `0x004BDB00`): the rectangle `(min x, min z, max x, max z)` of the start
  and end cells, marked through the terrain view's `MarkSelected(rect, 2, true)` (slot 7, water-ripples.md §2.2).
- **Release** (`0x004BDD80`), while dragging: a hit sets the end cell; the area is marked, then levelled; the
  selection is cleared (slot 9), the cell cursor setting restored, the capture released; not dragging; true.
- **Levelling** (`LevelTerrain`, `0x004BDB50`): `h :=` the altitude of the start cell's first vertex (terrain slot
  12, vertex `(cellsX + 1) × startZ + startX`); `SetAltitude(rect, h, true, true)` (terrain slot 49) over the
  vertices from `(min x, min z)` to `(min(max x + 1, cellsX), min(max z + 1, cellsZ))`.

## 4. The label tool (`cSC4ViewInputControlLabelTool`) *[compat]*

### 4.1 Making and finding labels

- Made by the city view's `ProcessCommand` (slot 7, `0x007F1C10`). Construction (`0x004BD230`): idle, no selection,
  orientation 1. `Init` (`0x004BC980`): id `0x8B3CE416`, cursor `0xEB72FEF4`; the prop developer's
  `ShowSignsAndLabels(true)`; base `Init`.
- A **label** is a text-decal occupant (`cSC4TextDecalOccupant`, interface `0x2B3B7D86`): slot 3 `AsOccupant`, 4
  `SetText`, 5 `GetText`, 6 `SetTextHeight`, 7 `SetDynamic`, 8 `SetTextColor(r, g, b)`, 9 `SetOrientation`, 10
  `SetAlpha`. As a prop its type is `0xCB79919B` (prop-developer.md's "street-name sign") *[inferred: the bulldozer
  treats that type as a label, §6]*.
- `GetDecalOccupantAt(p)` (`0x004BCDC0`): the occupant manager's first occupant at `(p.x, p.z)` (slot 28) through a
  filter that accepts props (`0xA823821E`) answering the text-decal interface (`cSC4LabelFilter`, `0x004BC9B0`).
- **Selection** (`0x004BCE80`): the new label gets alpha 0.7, the old one 0.5.

### 4.2 Mouse (`+0x2C`: 0 idle, 1 pressed, 2 dragging)

- **Press** (`0x004BD890`): true without the 3D renderer (`0x00B43DD0`). `PickTerrain(x, y, &p, 0, 1)` (renderer
  slot 60); false on a miss. A label at `p`: pressed; it is selected; the offset from `p` to its position is kept
  (`+0x34`); sound `0x2C16DA45` (§7); true. No label: a new text-decal occupant is added to the city (prop manager
  `AddCityProp`, slot 19) and the edit dialog runs (§4.5); when it fails the prop is removed (slot 29). Otherwise
  the orientation becomes 1 at view rotation 0 or 2, else 0 (renderer's draw context `+0x18`), the label is placed
  at `p` (§4.3) and selected. True.
- **Move** (`0x004BD5A0`): pressed with a selection → dragging and `SetDynamic(true)`. Dragging: a terrain hit
  places the selection there (§4.3). Idle: the label under the terrain hit, or none, becomes the selection. True.
- **Release** (`0x004BDA40`): pressed → the edit dialog; when it fails, the label is set to alpha 0.5, unselected and
  removed. Dragging → `SetDynamic(false)` and sound `0x2C16DA29`. Then idle; true.

### 4.3 Placing (`UpdateTextDecal(p, label)`, `0x004BD320`)

The cell `(cx, cz)` of `p` (city `PositionToCell`). The label's colour, height, position and orientation:

| Cell | Colour | Height | Position, orientation |
|---|---|---|---|
| has water (terrain slot 34) | (0.5, 0.5, 1) | 2 | `p`, the tool's orientation (`+0x40`) |
| no tile for mask `0x44B` (traffic network map slot 8) | (0.5, 1, 0.5) | 2 | `p`, ditto |
| an avenue tile (all of flag `0x40`) | (1, 0.902, 0) | 2 | §4.4, avenue rules |
| another tile, carrying road, rail or one-way road (any of `0x403`) | (1, 0.902, 0) | 0.8 | §4.4, road rules |
| another tile | (1, 0.902, 0) | 0.64 | §4.4, road rules |

Mask `0x44B` is road, rail, street, avenue and one-way road; `0x403` road, rail and one-way road. Orientation 0 runs
north–south, 1 west–east, 2 and 3 along the two diagonals *[inferred: from the snapping rules]*.

### 4.4 Snapping to roads and avenues

Sides are W, N, E, S (bytes 0–3 of an edge word, network-rules.md §1). The position becomes the cell's centre plus
an offset `(ox, oz)`; the height is kept; the orientation starts at 1 with no offset.

- **Roads** (`FindRoadCenterAndOrientation`, `0x004BCB00`): the tile under the cell for `0x40B`; its type is rail
  if it has rail, else one-way road, else road, else street; `e` = its edge word for that type. A side is connected
  when `byte & 7 ≠ 0` and diagonal when `byte & 7` is 1 or 3; `n` counts connected sides. One side: orientation 0
  when it is N or S, else 1. Two or three sides, the first rule that holds: W and E connected, E not diagonal → 1;
  N and S, N not diagonal → 0; W and N, N diagonal → 2, (−4, −4); E and S, S diagonal → 2, (4, 4); W and S, S
  diagonal → 3, (−4, 4); N and E, E diagonal → 3, (4, −4).
- **Avenues** (`FindAvenueCenterAndOrientation`, `0x004BCEE0`): the avenue piece at `p` (traffic network map slot 7
  with `0x40`); none → orientation 1 and the position is kept. `e` = its avenue edge word; a side is connected when
  `byte & 3 ≠ 0`, diagonal when `byte & 3` is 1 or 3, and inner when bit 2 is set (kind 4). With `n` connected and
  `d` diagonal sides:
  - `d = n`, four sides: orientation 2 when W's kind is 1, else 3. Otherwise W kind 1 → 2, (−8, −8); W kind 3 → 3,
    (−8, 8); E kind 1 → 2, (8, 8); E kind 3 → 3, (8, −8); none of these → 1.
  - `d = 2`, `n = 4`: W orthogonal → (−8, 8) and 2 when S is orthogonal, else (−8, −8) and 3; E orthogonal →
    (8, 8) and 3 when S is orthogonal, else (8, −8) and 2; neither → 1.
  - Otherwise: each inner side moves the label 8 m toward it (W (−8, 0), N (0, −8), E (8, 0), S (0, 8), summed);
    orientation 1 when W or E is orthogonal, else 0.

### 4.5 Keys and the edit dialog

- **Keys** (`0x004BD4B0`): Delete, with a selection while idle, removes the label (prop manager slot 29); true. Home
  and End, with a selection, turn the orientation (Home: 0→3→1→2→0; End: 0→2→1→3→0) and re-place the label at its
  position (§4.3); false. Other keys: false.
- **Dialog** (`DoEditLabelUI(label)`, `0x004BD6B0`; no label → true): the UI script (group `0x96A006B0`, instance
  `0x6B704690`) as window `0x8A8DFCF5` under the city view's window, with the tool's message filter (below). Its text
  field `0xAA4149BF` (interface `0x231A1C57`) gets the label's text, all of it selected. The dialog runs modally
  (`DoModalWin`, modal-dialogs.md §2.2). OK (`0x8A8DFD4E`): the field's text becomes the label's; an empty text
  fails. Cancel (`0x8A8DFD4A`) fails. Any other result succeeds and keeps the text. The focus returns to the city
  view; the dialog is shut down.
- **Message filter** (`DoMessage`, `0x004BCA10`, the tool's interface at `+0x28`): a click (type 3, `0x287259F6`)
  on `0x6BA18178`, Cancel or OK ends the modal loop with that id (the end message, type `0xF`, modal-dialogs.md
  §2.4); type `0x11` gives the text field the focus; Enter (type `0x1A`, key `0x0D`) ends it with OK.
- **Shutdown** (`0x004BD2E0`): the selection gets alpha 0.5 and is dropped; base `Shutdown`.

## 5. The sign tool (`cSC4ViewInputControlSignTool`) *[compat]*

### 5.1 Making signs

- Made by the city view's `ProcessCommand`. `Init` (`0x004D7BE0`): id `0x8B4CE5A0`, cursor `0x6B72FECF`; the
  framework's code page (`+0x64`); the framework service `0x27EBFFFD` (interface `0x07EC0010`, held) *[inferred:
  the clipboard]*; the message server's notification `0x0BE10A20` for the tool (§5.4); `ShowSignsAndLabels(true)`.
- A **sign** is a signpost occupant (`cSC4SignpostOccupant`, interface `0x4B44FBE2`): slot 3 `AsOccupant`, 4
  `SetText`, 5 `GetText`, 6 `SetDynamic`, 7 `SetEditMode`; prop type `0xAB72FBB3`. Signs are found by the
  renderer's `Pick(x, y, filter, &instance)` (slot 65) with a filter for props answering that interface
  (`cSC4SignFilter`, `0x004D7CC0`); the picked instance's occupant (instance slot 3).
- **Selection** (`0x004D7FC0`): the old sign, when its text is empty, is removed (prop manager slot 29), else gets
  `SetDynamic(false)`; the new one gets `SetDynamic(true)`.

### 5.2 Mouse (`+0x2C`: 0 idle, 1 pressed, 2 dragging, 3 typing)

- **Press** (`0x004D81D0`): typing → `SetEditMode(false)`, `SetDynamic(true)`, idle; true. Without the renderer:
  true. A sign under the mouse: pressed; selected; the offset from the mouse to its position projected on the
  screen is kept (`+0x34`, `+0x38`); sound `0x2C16DA45` (a picked instance that is not a sign: nothing). Nothing
  picked: `PickTerrain(x, y, &p, 0, 1)` (false on a miss); a new signpost is added to the city and selected, at
  `(p.x, max(ground altitude, sea level), p.z)`; typing; the edit text and its backup are cleared; `SetText("")`,
  `SetEditMode(true)`; sound `0x2C16DA29`. True.
- **Move** (`0x004D84B0`): pressed with a selection → dragging. Dragging: the terrain hit at the mouse point plus the
  kept offset is the sign's new position. Idle: the sign under the mouse, or none, becomes the selection. True.
- **Release** (`0x004D8730`): dragging → sound `0x2C16DA29`. Pressed, or dragging a sign with an empty text →
  typing: the edit text and its backup (`+0x50`) := the sign's text, `SetEditMode(true)`. Otherwise idle. True.

### 5.3 Typing

- **`OnCharacter(c)`** (`0x004D8830`), typing only (else false). The code page is multi-byte when it is 8 or at
  least 100. Escape: nothing. Enter: `SetEditMode(false)`, `SetDynamic(true)`, no selection (an empty sign is
  removed, §5.1), idle. Backspace: the last character (multi-byte aware) is cut and the sign's text set. Other codes
  below `0x20`: nothing. Others: with a multi-byte code page, a lead byte starts a pending character and the next
  bytes join it until it is whole; the whole character, or the single byte otherwise, is appended. True.
- **Appending** (`AppendText(s)`, `0x004D8670`): `k :=` the glyph count of the edit text; for each character of `s`
  while `k ≤ 32`: a newline is skipped, any other character appended; `k += 1` either way. Then the sign's text is
  set.
- **`OnKeyDown(key, modifiers)`** (`0x004D89D0`): typing: Escape restores the backup into the sign and the edit text
  and returns false; Ctrl+V (modifier bit 1, without bits 0 and 2) appends the service's text (§5.1, slot 18) when
  it gives one; other keys are taken (true). Not typing: Delete with a selection removes the sign; idle; true. Other
  keys: false.

### 5.4 Messages and shutdown

- `QueryInterface` (`0x004D7AF0`) answers `0xE98B2F57` with the message target at `+0x28`, other IIDs as the base.
- `DoMessage` (`0x004D7D70`, the target at `+0x28`): message `0x0BE10A20` while typing with a selection is answered
  (its `+4 := 1`) with a screen point in `+8`, `+0x0C`: of the two corners `a`, `b` that occupant slot 10 gives,
  `((a.x + b.x) / 2, b.y, (a.z + b.z) / 2)`, projected by `SC4ViewUtilities::ProjectPointToViewportCoordinates2D`
  *[inferred: where the input-method window goes]*. Never consumed (false).
- `Shutdown` (`0x004D8130`): typing → `SetEditMode(false)`, `SetDynamic(true)`; no selection; the service released;
  the notification removed; base `Shutdown`.

## 6. The sign and label bulldozer (`cSC4ViewInputControlSignLabelDemolish`) *[compat]*

- Made by the city view's `ProcessCommand`. `Init` (`0x004D7710`): id `0xCBCC987E`, cursor `0x8BD4B428`; the filter
  `cSC4SignOrLabelFilter` (`0x004D7680`: props whose prop type is `0xAB72FBB3` or `0xCB79919B`);
  `ShowSignsAndLabels(true)`.
- **Hover** (`0x004D7AC0`): `PickSignOrLabel` (`0x004D7820`): the renderer's `Pick` with the filter; else
  `PickTerrain(x, y, &p, 0, 1)` and the occupant manager's first occupant at `(p.x, p.z)` with the filter. The prop
  found, or none, becomes the selection (`0x004D7950`): a label is highlighted by alpha 0.7 (0.5 when it loses the
  selection), a sign by `SetDynamic(true)` (`false`).
- **Release** (`0x004D77C0`): with a selection, the prop is removed (prop manager slot 29), the selection dropped and
  sound `0x8A5EC047` played; true.

## 7. Sounds

The tools post the start-sound event `0xEA451903` with the sound id to the sound system (`0x00B43CB8`, audio.md
§7.3): `0x2C16DA45` when a label or sign is picked up, `0x2C16DA29` when one is put down or a sign is planted,
`0x8A5EC047` when the bulldozer removes one *[inferred: the purposes]*.

## 8. Quirks

1. The label tool keeps the press offset (§4.2) but never uses it: a dragged label jumps to the mouse.
2. Home and End turn a label but return false, so the key is passed on; on a road or avenue the snapping overrides
   the turn (§4.5).
3. Off networks a label keeps `p` exactly, and so does an avenue tile with no avenue piece found, while a road label
   always moves to the cell's centre (§4.4).
4. A sign holds up to 33 glyphs: the limit is tested before each append (§5.3). Pasted newlines are dropped but
   still counted.
5. Escape while typing restores the text but stays in typing mode and passes the key on (§5.3).
6. The levelling press returns true even when the terrain is missed (§3).
7. The scroll thresholds apply to half the mouse offset (§2.3).

## 9. Open questions

See [questions/view-input-controls.md](questions/view-input-controls.md): VIC1 (the commands that make the label,
sign and bulldozer tools, and global terrain command 7), VIC2 (message `0x0BE10A20` and service `0x27EBFFFD`).

## 10. Conformance tests

1. **Scroll speed:** inner 12, outer 80, anchor (100, 100), mouse (300, 100): `dx = 100`, cursor `0xC2A676A4`,
   speed 1, angle 0, scrolling on. Mouse (130, 100): `dx = 15`, speed 15/68. Mouse (110, 100): `dx = 5`, cursor
   `0xC2A676AC`, scrolling off.
2. **Levelling:** a drag from cell (3, 4) to cell (1, 6) marks (1, 4)–(3, 6) and sets vertices x 1–4, z 4–7 to the
   altitude of vertex (3, 4).
3. **Road label:** a street cell with edge word `0x02000200` (N and S orthogonal) gives orientation 0, the cell's
   centre, height 0.64 and colour (1, 0.902, 0).
4. **Diagonal label:** a road cell with W kind 1 and N kind 3 (`0x00000301`) gives orientation 2 and the offset
   (−4, −4) from the centre.
5. **Turning:** Home from orientation 1 gives 2; End from 1 gives 3.
6. **Sign length:** typing into a sign of 32 glyphs adds one more; the next character is ignored.
7. **Empty sign:** pressing Enter on a new sign with no text removes it.
