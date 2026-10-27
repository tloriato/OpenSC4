# MySim tools: change house, change job, move in, dispatch (behaviour spec)

Scope: the four `cSC4MySimInputControl*` owners as tools/coverage.py assigns them on `5c0ff782`. Together they have
38 functions (12,016 bytes; inventory in §6):
- `ChangeHouse` (2 functions);
- `ChangeJob` (21, which also holds the methods the three building tools share);
- `MoveIn` (1);
- `Dispatch` (14).

These are the city-view input controls a MySim's panel starts:
- pick a new home;
- pick a new job;
- pick the first home of a Sim moving in;
- place or drag the Sim's dispatch pylon.

Related: city-tools-input.md §2 (the base control `cSC4ViewInputControl`: fields `+0x04`–`+0x20`, its slot table,
`Init`, `Shutdown`, `SetCursor`, `EndInput`); mysims.md (the Sim, its `cISC4MySim` interface §7.2, its class slots
§7.3, the agent simulator §7.1); mysim-windows.md (the panel buttons); callout-box.md §2 (the callout slots);
network-occupants.md §6 (the pylon occupant); city-tools-input.md (the query tool's group table, which §3.3 repeats).

Evidence: static RE only, on the instruction and data bytes of the Steam 1.1.641 exe (the same code as GOG,
`docs/lanes/re-dates.md`). Every function was disassembled. Vtables were read at `0x00A85800`, `0x00A858D0`,
`0x00A85A20`, `0x00A85988`/`0x00A85998` and `0x00A8596C`. Written 2026-10-23 (re-mysimtools7).

Conventions:
- `T` is the tool, `Sim` the MySim. "Sim slot n" is the `cISC4MySim` interface (mysims.md §7.2), "Sim class slot n"
  the object's own vtable `0x00A85218` (mysims.md §7.3).
- Window slots are `cIGZWin` slots: 11 parent, 35 child by ID (recursive), 37 typed child (recursive), 41/42
  width/height, 56 `MoveTo`, 68 `SetFlag`, 69 `Show`, 70 `Hide`.
- The 3D view `+0x1C` (`cISC4View3DWin`) slots used:
  - 3: its window;
  - 24: the current input control;
  - 25: set it;
  - 27: remove all;
  - 36: `PickTerrain(x, z, &pos, 0)`;
  - 37: the occupant under a screen point;
  - 41: clear the cursor text by key.
- `(x, z)` are the window-client pixel coordinates of the base control's mouse slots; every mouse slot pops 12.

## 1. The tools *[compat]*

| Tool | Vtable | Constructor | Made by | Holds the Sim as |
|---|---|---|---|---|
| change house | `0x00A85800` | `0x00437AF0` (owner ChangeHouse) | the Sim's change-house tool (call `0x004268D4`) | the object (`+0x28`) |
| change job | `0x00A858D0` | `0x00438740` | `CreateChangeJobTool` (`0x00426970`) | the object |
| move in | `0x00A85A20` | `0x00439FC0` (owner `cSC4MySim`) | `MoveIn` (`0x0042D880`) | the `cISC4MySim` interface (object + `0xB8`) |
| dispatch | `0x00A85998` (and `0x00A85988` at `+0x28`) | `0x00439E50` | `CreateDispatchTool` (Sim slot 67) | the object (`+0x7C`) |

`MoveIn` makes its tool (0x4C bytes), runs `Init` (§2.1) with the Sim, keeps it in the global `0x00B20E0C` (counted)
and pushes it onto the 3D view (slot 25, mode 2).

### 1.1 The building tools' fields

The three building tools share one layout. The constructors zero `+0x28`–`+0x48`.

| Offset | Content |
|---|---|
| `+0x28` | the Sim (counted) |
| `+0x2C` | the highlighted building, a `cISC4BuildingOccupant` (counted) |
| `+0x30` | that building's highlight level before the tool raised it |
| `+0x34` | the MySim agent simulator (city slot 71, counted) |
| `+0x38` | the callout box (counted) |
| `+0x3C` | the "allowed" badge window (counted) |
| `+0x40` | the "not allowed" badge window (counted) |
| `+0x44` | the badge height |
| `+0x48` | half the badge width |

### 1.2 The building tools' shared slots

| Slot | Address | Behaviour |
|---|---|---|
| 3 `Init()` | `0x00439F80` | `SetCursor(0x43401C71)` (slot 8), then the base `Init` |
| 4 `Shutdown()` | `0x00437B80` | §2.3 |
| 14 `OnKeyDown(key, mods)` | `0x00439FA0` | Esc (`0x1B`): `EndInput` (slot 27), true; any other key: false |
| 22 `OnMouseExit()` | `0x00438810` | hides `+0x3C` and `+0x40` when they exist; true. The callout and the highlight stay as they were *[quirk]* |
| 16 `OnMouseDownL` | per tool | §4 |
| 20 `OnMouseMove` | per tool | §3 |
| 26 deleting destructor | `0x00437CF0`, `0x00438840`, `0x0043A090` | the body, then free when `flags & 1` |

The destructor bodies (`0x00437B20`, `0x00438770`, `0x00439FF0`) restore the tool's vtable and release `+0x40`,
`+0x3C`, `+0x38`, `+0x34`, `+0x2C`, `+0x28`, in that order, before the base destructor (`0x005FAFC0`).

## 2. Starting and stopping a building tool

### 2.1 `Init(sim)` *[compat]*

The three tools have their own `Init(sim)` (pop 4, AL):
- change house `0x00438390`;
- change job `0x00438860`;
- move in `0x0043A730`.

They run the same steps; the change-house tool adds step 6.
1. Slot 3 `Init()`. False → return false.
2. Hold the Sim in `+0x28`. The move-in tool keeps the interface (the argument + `0xB8`, or null).
3. Hold the agent simulator (city `0x00B43CEC` slot 71) in `+0x34`.
4. Window manager slot 37 `SetFocus` on the 3D view's window.
5. Only when the badge `+0x3C` does not exist yet:
   1. `SC4CreateWindowFromScript((0, 0x96A006B0, 0x6A9455C9), the 3D view's window, 0x27DF05BE, 0x5386D516,
      &+0x3C)`. A failure skips the rest of the step (the result is still true).
   2. `+0x44` := the badge's height; `+0x48` := its width / 2 (C division). `SetFlag(0x200000, 1)` on it.
   3. `+0x40` := the window `0x27DF05BF`, found from the badge's parent (slot 11, then slot 35); held, with the same
      flag.
   4. **The portrait**: id = Sim slot 22 (`GetBitmapID`).
      - Non-zero: an image buffer for the key (0, `0x46A006B0`, id).
      - Zero: a file. The path is application slot 61 (a directory) followed by Sim slot 23 (`GetBitmapFile(0,
        −1)`), opened with mode 8 *[inferred: the buffer's format argument]*.
   5. When the image loads, it is set (child slot 4) on the bitmap child `0xEA9457BA` of `+0x3C` and on
      `0xEA9457BB` of `+0x40` (typed lookup, IID `0xC12CEA13`).
6. **Change house only** (also when the portrait failed), by the Sim's wealth (Sim slot 19), on children of the badge
   (IID `0x22BA0121`):
   - wealth 1: hide `0xCBF33D81` and `0xCBF33D8C`;
   - wealth 2: hide `0xCBF33D8C`;
   - otherwise: nothing.
7. Return true.

### 2.2 Ending

- **Esc** ends the tool (slot 14 above).
- **A successful click** ends the tool (§4).
- **`EndInput`** pops the tool from the 3D view (city-tools-input.md §2.7).

### 2.3 `Shutdown()` (`0x00437B80`)

1. Release the Sim.
2. With a highlighted building: restore its highlight (occupant slot 17 `SetHighlight(+0x30, false)`), release it.
3. Release the agent simulator.
4. With a callout: hide it, delete it from the 3D view's window (slot 16 `ChildDelete`), release it.
5. The same deletion for `+0x3C` and `+0x40` (no hide).
6. Base `Shutdown`; return true.

## 3. Hovering (`OnMouseMove`) *[compat]*

Every call returns true.

### 3.1 Change house (`0x00437D10`) and move in (`0x0043A0B0`)

The two bodies are the same code. They differ only in the residence test of step 1.6 and in the constant that
step 1.4 compares with (two copies of 1.0, `0x00B07E04` and `0x00B07E30`).

1. **Eligibility.** The building under the cursor is eligible when all of these hold, tested in order:
   1. 3D view slot 37 at (x, z) gives an occupant, whose type (slot 7) is `0x278128A0` (a building);
   2. it QIs to `cISC4BuildingOccupant` (`0x87DDA3A9`), `b`;
   3. the agent simulator exists and `MySimLivesHere(b)` (its slot 9) is null;
   4. the float that `b` slot 9 points to is ≥ 1.0. Slot 9 (`0x00458810`) hands out the address of the building's
      completion float (field `+0x58`; slot 10 stores it, and the lot developer drives it from 0 to 1.0 while the
      building is under construction, lot-developer.md §6.7), so this means "construction finished" *[compat]*;
   5. the value that `b` slot 11 points to is 1. Slot 11 (`0x00459250`) is `GetBuildingProfile`: it returns the
      shared profile of the building's type (the building-development simulator's slot 39 asked for the type, or a
      static all-zero profile when that simulator does not exist; abi/layouts.md "Building profile"). The profile's
      first field is its purpose, and purpose 1 is residence (building-development.md §1), so this means "a
      residential building" *[compat]*;
   6. the Sim accepts it: `TestForResidence(b, true, true)` for change house, `(b, true, false)` for move in (Sim
      slot 7);
   7. the occupant's lot (lot manager `0x00B43D08` slot 14) is not null and its slot 33 is 3. Lot slot 33 is `GetState` (the habitation state byte, `+0x7F`) and 3 is
      "occupied" (lot-developer.md conventions: 0 created, 1 ready, 2 constructing, 3 occupied, 4 vacant).

   Before test 1.7 the tool also reads:
   - `w` := the lot's slot 41, `GetOccupantWealth` (byte `+0x82`: 0 none, 1–3 = $, $$, $$$), through the same
     lookup with **no null test** *[quirk]*;
   - the building's name (`b` slot 19) into the callout title.
2. **Eligible:**
   1. Hide `+0x40`. Move `+0x3C` to (x − `+0x48`, z − `+0x44`) and show it.
   2. **Highlight.** Restore and release the previous building (as §2.3 step 2), hold `b`, `+0x30` := its
      highlight (occupant slot 16), then `SetHighlight(1, false)`.
   3. **The callout**, made once (a `cSC4WinCalloutBox`, 0x1B4 bytes):
      - body font `0xA85F1A83` (slot 11), title font `0x0A52F5C6` (slot 10), no icon (slot 12 (0));
      - skin: the image (0, `0x46A006B0`, `0x14416208`) with padding (12, 10) (slot 4);
      - a child of the main window `0x00B43DC8` (slot 14);
      - offset rectangle (−`+0x48`, −1000, `+0x48`, 28) (slot 17).
   4. **The wealth meter** (slot 14): the image (0, `0x46A006B0`, `0x1441621F`), count 3, fill 0.33, 0.66 or 1.0
      for `w` = 1, 2, 3. Another `w` passes a stale local (0, or a pointer's bits) *[quirk]*; the callout clamps it.
   5. Title (slot 6) := the building's name.
   6. **The body** (slot 8) := `"%s\n%s"` of an EQ text and an HQ text (§3.4). The values come from the building's
      position (`b` slot 8) turned into a cell (city slot 95), then from the residential simulator `0x00B43DA4`:
      - EQ: `GetCellEQByWealth(cx, cz, w)` (slot 37);
      - HQ: `GetCellHQByWealth(cx, cz, w)` (slot 38).
   7. The callout's point (slot 15) := (x + 1, z); show it.
3. **Not eligible:**
   1. Hide `+0x3C`. Move `+0x40` to (x − `+0x48`, z − `+0x44`) and show it.
   2. Hide the callout.
   3. Restore and release the highlighted building.

### 3.2 Change job (`0x00438EA0`)

The same structure, with these differences.
1. **Eligibility**, in order:
   1. as §3.1 tests 1.1 and 1.2;
   2. the agent simulator exists (no `MySimLivesHere` test);
   3. the float behind `b` slot 9 is ≥ 1.0 (`0x00B07E14`);
   4. the value behind `b` slot 11 is **not** 1;
   5. the lot at the building's position: lot manager slot 5 with `b` slot 8's position (from a zero vector). It is
      dereferenced without a null test *[quirk]*. Then:
      - its slot 63 with the Sim's wealth (Sim slot 19) is > 0: `GetTravelJobs(wealth)`, the lot's job supply for
        that wealth (a float; wealth 1, 2, 3 pick the lot's three travel-jobs values, any other wealth gives 0.0,
        which fails; NaN also fails) *[compat]*;
      - its slot 35 is < 4: `GetCondition` (byte `+0x80`), so a lot whose condition has reached 4 (the last step of
        an abandoned lot's decline, lot-developer.md §5.5) is refused *[compat]*;
      - its slot 33 is 3: `GetState`, "occupied", as in §3.1.
2. The name (`b` slot 19) is read before the test, for the callout title.
3. **Eligible:**
   1. The badges and the highlight as §3.1.
   2. The callout is made once with the same fonts, icon, parent and offset, but **without a skin**.
   3. Each time:
      - the skin is the image (0, `0x46A006B0`, icon) with padding (12, 10), the icon chosen by the building's
        groups (§3.3);
      - the title is the name;
      - the body is the Sim's `GetJobTitle(b, &text)` (Sim slot 27).
   4. Point (x + 1, z); show.
4. **Not eligible:** as §3.1.

### 3.3 The job callout's skin (`sGetCallOutBoxBackgroundBitmapID`, `0x00438B50`)

1. Read the occupant's `OccupantGroups` property (`0xAA1DD396`, its property holder slot 9).
2. When it is a u32 array (QI `0x60FD4D2B`, type `0x8006`), its values go into a static hash set (`0x00B20E20`,
   made once and cleared each call). Otherwise the result is `0x14416200`.
3. The first match, in this order, gives the icon instance:

   | Groups (test order) | Icon |
   |---|---|
   | `0x1000` | `0x14416208` |
   | `0x1001` | `0x14416202` |
   | `0x1002` | `0x14416206` |
   | `0x1003` | `0x14416209` |
   | `0x1004` (tested twice) | `0x1441620A` |
   | `0x1501`, `0x1500` | `0x14416207` |
   | `0x1502` | `0x14416203` |
   | `0x1503`–`0x1507` | `0x14416201` |
   | `0x150A`, `0x150B` | `0x1441620C` |
   | `0x1006` | `0x14416204` |
   | none | `0x14416200` |

This is the icon column of the query tool's table (city-tools-input.md), without its meters.

### 3.4 The EQ and HQ texts (`0x00437A30`, `0x00437A80`)

Each picks a string of table `0xEA231E96` by band:

| Value | EQ (`0x00437A30`) | HQ (`0x00437A80`) |
|---|---|---|
| below 50 | `0xEA5B8C93` | `0xEA5B8C90` |
| 50 to below 100 | `0xEA5B8C94` | `0xEA5B8C91` |
| 100 and above, or NaN | `0xEA5B8C95` | `0xEA5B8C92` |

## 4. Clicking (`OnMouseDownL`) *[compat]*

Each returns false when the tool is not the 3D view's current control (slot 24), else true. With a highlighted
building `b` (`+0x2C`):

| Tool | Address | Action |
|---|---|---|
| change house | `0x00437C90` | `TestForResidence(b, true, false)` and then `ChangeResidence(b)` (Sim slots 7, 6); both true → `EndInput` |
| change job | `0x004387D0` | `ChangeJob(b)` (Sim slot 8), then `EndInput` (whatever it returned) |
| move in | `0x0043A050` | `SetResidence(b)` (Sim slot 13), then `EndInput`. No new test: the hover's test stands |

Without a highlighted building nothing happens (still true).

## 5. The dispatch tool (`cSC4MySimInputControlDispatch`) *[compat]*

### 5.1 Fields

| Offset | Content |
|---|---|
| `+0x28` | message-target sub-object (vtable `0x00A85988`, slot 3 `DoMessage` `0x004396E0`) |
| `+0x2C` | 4 (never read here) |
| `+0x30` | the last location test (§5.6) |
| `+0x34`, `+0x38` | the drag offset, in pixels |
| `+0x40` | a timer (units 4): the double-click clock |
| `+0x58` | the normal cursor `0x2BAC4970` |
| `+0x5C` | the hovered pylon (counted twice, §5.4) |
| `+0x60` | a string (unused here) |
| `+0x74` | active: 1 from `Init`, 0 while another control is on top, 1 again when this one is |
| `+0x78` | the pylon filter (§5.3) |
| `+0x7C` | the Sim (object; constructor argument, counted) |
| `+0x80`–`+0x88` | the pylon's position saved when a drag starts |
| `+0x8C` | its dispatch state saved then |

The destructor body (`0x00439F00`):
1. Releases `+0x7C`, `+0x78`, the string and `+0x5C`.
2. Destroys the timer.
3. Runs the base destructor.

The deleting destructor is `0x00439F60`.

### 5.2 `Init` (slot 3, `0x00439450`) and `Shutdown` (slot 4, `0x00439540`)

**`Init`**, only when not initialised:
1. ID `+0x0C` := `0xEBAB04A9`; `+0x58` and the cursor ID := `0x2BAC4970`; timer units 4; base `Init`.
2. With the message server (`0x00B43CCC`), subscribe `+0x28` (slot 5) to `0x09C8B8EA`, `0xE94671EB`, `0xE94671EC`,
   `0xE94671ED`, `0x894671F6`, `0xAA25827C`.
3. `+0x74` := 1; `+0x78` := a new pylon filter (0xC bytes, an occupant filter with vtables `0x00A8596C` /
   `0x00A85954`).
4. Return true.

**`Shutdown`**, only when initialised:
1. 3D view slot 41 (`0x49D02C4D`): clear the cursor text.
2. Unsubscribe the six messages.
3. The hovered pylon: `SetUIState(1)` (pylon slot 6), then one release; `+0x5C` := null. `Init`'s path adds two
   references (§5.4), so one is leaked *[quirk]*.
4. `p` := the Sim's pylon (Sim class slot 13, `+0x248`).
   1. When it is being dragged (pylon slot 16 `GetDragState`), put it back: its occupant's position (occupant slot 9)
      := `+0x80`; `SetDispatchState(+0x8C)` (slot 11).
   2. Sim class slot 10 (`DispatchToolEnd`).
   3. With `p`: `SetDragState(0)` (slot 15).
5. Release the filter; base `Shutdown`; return true.

### 5.3 Picking a pylon

- **The filter's include test** (its slot 3, `0x004393A0`). An occupant is a candidate when:
  - its type is `0xA9B6D7DE`;
  - it QIs to the pylon (`0xC9B6D7D6`);
  - the pylon's slot 7 is not 0: `GetUIState` (field `+0x6C`; 1 normal, 2 active, 3 hovered, dispatch.md §4.1;
    the pylon's own `Pick` also refuses state 0, network-occupants.md §6);
  - it is not being dragged (slot 16);
  - its slot 20, `GetPylonOwner` (field `+0x1C0`, set by slot 19 `SetPylonOwner`), is not null and QIs to
    `0x0A1DA42F` (the Sim).
- **`PickPylonOccupant(x, z, &out)`** (`0x00439890`, pops 12):
  1. `out` := null.
  2. Renderer `0x00B43DD0` slot 65 `Pick(x, z, filter, &hit)` (render3d.md §11.5). The hit's slot 3 is QI'd to the
     pylon, whose slot 20 must QI to `0x0A1DA42F`.
  3. Then `out` := the pylon (AddRef'd) and the result is true.

### 5.4 Hover, Sim and drag offset helpers

- **`SetHoverPylon(p)`** (`0x00439660`, pops 4). Nothing for the same pylon. Otherwise:
  1. The old one gets `SetUIState(1)` and two releases.
  2. The new one is stored with two AddRefs, then gets `SetUIState(3)`.
- **`SetNewMySim(sim)`** (`0x00439840`, pops 4): when it differs, the old Sim's class slot 10 (`DispatchToolEnd`)
  runs, then the new one is held. Returns true.
- **`CalculateDragPylonOffset(p, x, z)`** (`0x004BA070`, pops 12):
  1. The pylon's occupant position (slot 8) goes to the screen (view utilities `0x00B43DD8`, `0x007F78E0`) as
     (sx, sy).
  2. `+0x34` := trunc(sx − x); `+0x38` := trunc(sy − z).

### 5.5 Clicking (`OnMouseDownL`, `0x00439980`)

Without a 3D view: `PlayResultSound(0)`, false. Otherwise:
1. `p` := the Sim's pylon (held).
2. **Double click**: when the timer runs and less than 500 have elapsed, `dbl` := true and the timer stops (`Reset`);
   else the timer restarts.
3. **No pylon yet**:
   - A pylon under the cursor (§5.3) goes to step 5 with it.
   - Otherwise `p` := Sim class slot 9 (the dispatch tool's pylon, made now). When terrain picking at (x, z) succeeds
     and the location test (§5.6) passes:
     - the pylon goes there (occupant slot 9), with y := the terrain altitude there (city slot 84, then slot 13);
     - `DispatchToolLockPylon(1)` (Sim class slot 11).

     The result is true either way.
4. **A pylon being dragged**:
   - Not a double click: the pylon goes to the terrain point at (x + `+0x34`, z + `+0x38`) when picking succeeds (no
     location test).
   - A double click: post `0x8BBBD22A` (`SC4PostMessage2(id, 1, 0, 0, 0, the Sim's interface)`).
   - Either way: `SetDragState(0)`, `DispatchToolLockPylon(1)`, true.

   **A pylon not being dragged**: pick one under the cursor; none → false.
5. **A picked pylon `q`**:
   1. The Sim it belongs to (`q` slot 20, QI `0x0A1DA42F`, minus `0xB8`) becomes the tool's Sim (`SetNewMySim`).
   2. `DispatchToolLockPylon(dbl)`; `q.SetDragState(!dbl)`; post `0x8BBBD22A` with `dbl`.
   3. Not a double click (a drag starts): `CalculateDragPylonOffset(q, x, z)`; `+0x80` := its position (occupant
      slot 8); `+0x8C` := its slot 12 (dispatch state).
   4. True.
6. Release the held objects; `PlayResultSound(0)`; return the result.

### 5.6 Moving (`OnMouseMove`, `0x00439C90`)

Returns true always. Without a 3D view or without a pylon, nothing.

- **Dragging** (`GetDragState`):
  1. Cursor `0x13B138D0`. While the click timer runs and less than 500 have elapsed, stop here. After that the timer
     stops.
  2. Pick the terrain at (x + `+0x34`, z + `+0x38`); y := the altitude there.
  3. `+0x30` := Sim class slot 15 at (x, z) (a location test through `0x00B43CF4` slot 13).
     - Passing: the pylon moves there and `SetDispatchState(3)`.
     - Failing: `SetDispatchState(1)` and it stays.
- **Not dragging**: pick a pylon under the cursor.
  - One whose slot 7 is not 0: `SetHoverPylon(it)` and cursor `0x13B138D0`. The filter already requires slot 7 ≠ 0,
    so the other branch (no hover, normal cursor) is not reached *[inferred]*.
  - None: the normal cursor (`+0x58`) and no hover.

### 5.7 Messages (`DoMessage`, `0x004396E0`) and keys

| Message | Effect | Result |
|---|---|---|
| `0x09C8B8EA` (data: old control = slot 10, new control = slot 15) | old = this and new ≠ this, null or not ID `0x48E2A1F8`: `+0x74` := 0, clear the cursor text (`0x49D02C4D`), no hover, and a dragged pylon goes back to `+0x80` with state `+0x8C`. new = this, old ≠ this: `+0x74` := 1 | true |
| `0x99EF1143` | the occupant (slot 10) is the hovered pylon: release it once, `+0x5C` := null | true |
| `0xE94671EB`–`0xE94671ED`, `0x894671F6` | nothing | true |
| `0xAA25827C` | when the Sim is not active (Sim slot 20): 3D view slot 27 (remove all controls, false) | false |
| others | nothing | false |

`OnKeyDown` (`0x00439380`) asks `IsKeyToBePassed` and returns false for every key, so keys go on to other handlers.

## 6. Quirks, cases and inventory

Quirks:
- The house tools' first lot lookup has no null test.
- The job tool dereferences its lot lookup without a test.
- A wealth outside 1–3 gives the meter a stale fill.
- `OnMouseExit` hides the badges but leaves the callout and the highlight.
- The dispatch tool's `Shutdown` releases the hovered pylon once of its two references.
- The move-in click does not re-test the house.

| Case | Setup | Expected |
|---|---|---|
| MT-C1 | change house, hovering a built house (lot wealth 2) where no MySim lives and `TestForResidence(b, 1, 1)` holds | badge `+0x3C` at (x − w/2, z − h), callout with meter fill 0.66 and body "EQ\nHQ" |
| MT-C2 | the same house, another MySim lives there | badge `+0x40`, no callout, no highlight |
| MT-C3 | change job over a house | not eligible (purpose 1) |
| MT-C4 | change-job callout over a building with groups {`0x1002`, `0x1500`} | skin `0x14416206` (industrial first) |
| MT-C5 | EQ 49.9, HQ 100 | texts `0xEA5B8C93`, `0xEA5B8C92` |
| MT-C6 | dispatch: two clicks on a pylon 300 ms apart | the second is a double click: lock 1, no drag, `0x8BBBD22A` with 1 |
| MT-C7 | dispatch: drag a pylon, then another tool takes over | the pylon returns to the saved position and state |

**Inventory** (38 functions):
- ChangeHouse: `0x00437AF0`, `0x00438390`.
- MoveIn: `0x0043A730`.
- ChangeJob:
  - `0x00437A30`, `0x00437A80` (§3.4);
  - `0x00437B20`, `0x00437C90`, `0x00437CF0`, `0x00437D10` (change house);
  - `0x00437B80`, `0x00439F80`, `0x00439FA0`, `0x00438810` (shared);
  - `0x00438740`, `0x00438770`, `0x004387D0`, `0x00438840`, `0x00438860`, `0x00438B50`, `0x00438EA0` (change
    job);
  - `0x00439FF0`, `0x0043A050`, `0x0043A090`, `0x0043A0B0` (move in).
- Dispatch: `0x00439380`, `0x004393A0`, `0x00439450`, `0x00439540`, `0x00439660`, `0x004396E0`, `0x00439840`,
  `0x00439890`, `0x00439980`, `0x00439C90`, `0x00439E50`, `0x00439F00`, `0x00439F60`, `0x004BA070`.

Not covered:
- the move-in constructor `0x00439FC0` (owner `cSC4MySim`);
- the shared ≥ helper `0x00437AD0` (owner `<shared>`: `*this ≥ *arg`);
- the Sim class slots 9–15 themselves (mysims.md).
