# Spec: the region screen (`cSC4WinRegionScreen`)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`) and from the shipped
tuning exemplar. Implementers must work from this spec, the public LGPL gzcom-dll SDK headers and general
knowledge only. They must **not** consult decompiled or disassembled code (see
[CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[compat-limit]* depends on something FreeCity4 cannot reproduce, so only the rule is matched.

Related specs: [region.md](region.md) (the region, its tiles and cities), [regional-city.md](regional-city.md)
(the city records), [ui-windows.md](ui-windows.md), [camera.md](camera.md), [command-system.md](command-system.md).

## 0. Summary

The region screen is the window shown between cities. It draws the region's terrain with each city's snapshot,
the clouds and the city signposts; hovering highlights a tile, a click selects it and opens its panel, and a second
click (or the play button) opens the city, creating its record first for an empty tile. Its buttons open the
region, city, snapshot and preference dialogs.

## 1. Tuning *[compat]*

`ReadTunableValues` (`0x007ABF10`) reads the exemplar (type `0x6534284A`, group `0x690F693F`, instance
`0xAA383BFE`, "Region UI Tuning Parameters"):

| Property | Shipped | Use |
|---|---|---|
| `0xCA383CA3`, `0xCA383CA4`, `0xCA383CA7` | 64, 10, 0.1 | three values of the scroll state (`+0x174`) *[summary]* |
| `0xCA383CA5`, `0xCA383CA6` | 150, 150 | `+0x198`, `+0x19C` *[summary]* |
| `0xCA383CA8`, `0xCA383CA9`, `0xCA383CAA` | 2, 20, 4 | three global values *[summary]* |
| `0xCA383CAB`, `0xCA383CAC` | (absent) | two values also given to the region view (`+0xF8`, `+0xFC`) |
| `0xCA383CAD`, `0xCA383CAE` | 90, 64 | two bytes, also copied to two globals *[summary]* |
| `0xCA383CAF`, `0xCA383CB0` | (absent) | `+0x1F0`, `+0x1EC` |
| `0xCA383CB1` | 32, 64 | the clouds' opacity range (`cSC4WinRegionClouds::SetOpacityRange`) |

`0xCA383CA2` (0.2) is not read here *[inferred: read elsewhere]*.

## 2. Making it *[summary; QR1]*

`Init` (`0x007B1900`, once): the window's own `Init`; a key accelerator from a resource (GZCOM `0x42E967BE`); the
current region from the region manager (application slot 34; its slot 8); the application services
(`InitApplicationServices`, `0x007ACC90`); a background window, the region view (`cSC4WinRegionView`), the clouds and
the GUI (`LoadGUI`, `0x007B0470`); the tuning (§1); the cities put in view (§3). `Shutdown` (`0x007B0F60`) undoes it.

## 3. The cities in view (`PutCitiesInView`, `0x007B13C0`) *[compat]*

For each city of the region (region slot 17): the region-view thumbnails are read from its save file
(`SC4ReadCityRegionViewThumbnails`) or, without one, the default snapshots are used (`SetCitySnapshotsToDefault`,
`0x007ABB80`); the images are rebuilt from the snapshots (`RecalculateImagesFromCitySnapshots`, `0x007AE510`,
resampled to the view's scale); the city is added to the region view with its signpost type. Then
`ComputeBaseLimits` (`0x007AB7C0`) sets the scroll limits from the region's size and the clouds' area. The total
population (`UpdatePopulation`, `0x007AEC00`: the sum of the cities' slot 10) is shown in the text `0xC9E41918`,
formatted by the locale.

## 4. Mouse and keys *[compat]*

- **Move** (`GZOnMouseMove`, `0x007AB760`): unless a city is held (`+0x1A0`), the region view highlights the city
  under the cursor (`SelectCity`).
- **Left press** (`GZOnMouseDownL`, `0x007ACAD0`):
  1. With a held city, a press on that same city within the double-click time (window manager slot 40) since it was
     held is the **play** command `0x4A560000` (§5). Otherwise the city under the cursor is selected; none → the
     signposts are restored (`RestoreSignposts`, `0x007AC110`) and the press ends.
  2. With a selected city: sound `0x2A5C322B` (sound system slot 11); every panel closed (`ShowPanel(-1)`); the
     tutorial arrow and the signposts and labels updated; the city's signpost type is `0xCA539340` when its record
     is established (regional city slot 43), else `0x0A8CD184` (with `0x0A551C50`); the city is **held** and the
     double-click timer restarted.
- **Scroll**: the message `0xAAA1CDF2` moves the view by its two values times 1/256 (`+0x178`, `+0x17C`). The
  commands `0x6A935CD8` / `0x6A935CE0` set the horizontal scroll speed to −v / +v and `0x6A935CDD` / `0x6A935CE2`
  the vertical one, `v` being the scroll state's speed; `0x2A94826E`, `0x2A948275` stop the horizontal scroll and
  `0x2A94826B`, `0x2A948272` the vertical one (`ProcessCommand`, `0x007AF720`) *[inferred: the arrow keys]*.

## 5. Buttons (`GZOnCommand`, `0x007AFAA0`; notification `0x287259F6`) *[compat]*

| Button | Action |
|---|---|
| `0x4A560000` | **play** the selected city: its record (§6.1, created if missing), then `LaunchRegionalCity` (§6.2) |
| `0x4A560001` | the signposts restored, then the delete-city dialog (`cSC4DeleteCityDialog`) |
| `0x4A560002` | **import** a city into the selected tile (§6.5) |
| `0x4A560003` | the selected city's snapshots reset to the defaults; its images, the view and the population updated |
| `0x2A5B0000`, `0x2A5B0001`, `0x2A5B0002` | the new-region, load-region and delete-region dialogs |
| `0x098F4F6C` | the video preferences dialog |
| `0x0A5510A9` | the game preferences dialog |
| `0x09EBF2BD`, `0x09EBF2C3`, `0x09EBF2C8` | `ShowPanel` (`0x007ABDF0`) of a side panel *[summary]* |
| `0x26C10A3E` | the region and position saved to the preferences (§7), then a quit request *[inferred]* |
| `0x8A1DA655` | the snapshot taker (`cSC4WinSnapshotTaker::CreateWinSnapshotTaker`) |
| `0xCA1DA670` | the snapshot album at the current city (`ShowSnapshotAlbum`, `GoToAlbumForCurrentCity`, modal) |
| `0xABA290E1`, `0xCBA290EC`, `0xABA290F6` | the region view's map mode (`SetMapMode`) |
| `0xEA5A96E6` | the region view's grid (`EnableGrid`, with notification `0x287259F7`) |
| `0x4A779A09`, `0x4A779A1A`, `0x4BB92C1F` | other window toggles and texts *[summary; QR3]* |
| `0xA98F4F88`, `0xCA5CFEE2`, `0xEBB91356` | the same *[summary; QR3]* |

`ProcessCommand` (`0x007AF720`, the command messages `0x4B99446A` the screen receives) also handles:
`0x6A935CF1` clears the selection; `0x6A9757C2` builds the whole region (`MakeWholeRegion`, `0x007AEDD0`
*[summary]*); `0x6AA9FE51` sets or toggles the preference byte `+0xF0A`; `0x6A935E3C` saves the region and
position and the preferences (application slot 17); `0x0BB3C277 <name>` loads a region (§6.3);
`0x0BB2747D <name> <size> [empty]` opens a city (§6.4), the size word being `small`, `medium`, `large` or `any`
(codes 0–3), else the parameter's number.

## 6. Cities and regions *[compat]*

### 6.1 `GetOrCreateRegionalCity(tile, create)` (`0x007AEA00`)

The region's city at the tile's cell (region slot 11). Without one and with `create`: a new `cSC4RegionalCity`
is initialised (slot 3), sized `2^k × 2^k` tiles with `k` the tile's size code (slot 8), placed at the tile's cell
(slot 6, `(x, z, 0)`), named with string `0xAA738C4E` of group `0x6A231EAA` (slot 34), and inserted in the region
(slot 12); the region's city at the cell is returned.

### 6.2 `LaunchRegionalCity(record)` (`0x007ADC20`)

The position is saved to the preferences (§7). When the record has a save-file path (slot 31) and the
application's `LoadCity(path, record)` (slot 13) succeeds, the result is true. Otherwise the new-city
specification (application slot 39) drops its object at `+0x200` and the result is the application's
`RequestNewCity(record)` (slot 6).

### 6.3 `LoadRegion(name)` (`0x007ADAC0`)

Each region of the region manager (its slots 6 and 11) is compared by name; the current one → true; another → it
becomes current (manager slot 9) and the result is `RequestGoToRegionView(false)` (application slot 12); none →
false.

### 6.4 `LaunchCity(name, size)` (`0x007AF4B0`)

The size code 0, 1, 2 selects tiles of area 1, 4 or 16 (small, medium, large); 3 (`any`) none. With a name, the
first established city whose name (slot 33) matches is launched; without one, the first empty tile of the size gets
a record (§6.1) and is launched; the `empty` flag restricts the search to empty tiles *[summary]*.

### 6.5 Import *[summary; QR2]*

When the selected tile already has a city, a confirmation is asked first (strings `0x4AA59E23` and `0xEA8DFC1B` of
group `0x6A231EAA`, `GenericConfirmationDialog`). `SC4DisplayImportCityDialog` then picks a city file; its record
is read (`SC4ReadRegionalCity`) and placed in the region at the tile, and the view is refreshed.

## 7. Preferences *[compat]*

`SetUserPreferencesToCurrentRegion` (`0x007AC270`) copies the current region's name into the application
preferences (slot 38); `SetUserPreferencesToCurrentPosition` (`0x007AC2D0`) stores the view position (its two
coordinates, truncated). `SavePreferences` is application slot 17.

## 8. Messages *[compat]*

| Message | Effect |
|---|---|
| `0x8A4BAC53` / `0x8A4BAC5B` | sets / clears a flag of the region view (`+0x110`) and redraws on set |
| `0x1B` (key, with code 5 or 6) | the key accelerator's command is posted *[summary]* |
| `0x231BBF91` | shows or hides a window by the preference byte `+0xF11` |
| `0x2B96B3EA` | the camera's post-view-init message, and a window shown |
| `0xAAA1CDF2` | scroll (§4) |

## 9. Quirks

1. The tuning exemplar's `0xCA383CA2` is not read by the screen (§1).
2. A held city stays held until another press; only a second press on it within the double-click time plays it (§4).

## 10. Open questions

See [questions/region-screen.md](questions/region-screen.md): QR1 (the window construction and the services of
`InitApplicationServices`), QR2 (the import path), QR3 (the remaining buttons, `ShowPanel` and `MakeWholeRegion`).

## 11. Conformance tests

1. **Double click:** two presses on the same city within the double-click time open it; two presses slower than that
   only select it.
2. **New tile:** playing an empty large tile (size code 2) creates a record of size 4 × 4 at the tile's cell,
   named with string `0xAA738C4E`, and calls `RequestNewCity` with it.
3. **Existing city:** playing a tile whose record has a save path calls `LoadCity(path, record)`.
4. **Region load:** `LoadRegion` with the current region's name returns true without changing the region.
