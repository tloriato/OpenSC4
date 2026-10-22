# Spec: the region (`cSC4Region`)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`). Implementers must work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled
or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[compat-limit]* depends on something FreeCity4 cannot reproduce, so only the rule is matched.

Related specs: [regional-city.md](regional-city.md) (the city objects the region holds),
[foundations.md](foundations.md) §11 (the regional-city record), [neighbors.md](neighbors.md) (§14.1 and NG15, the
region calls of the neighbours simulator), [paths.md](paths.md) (the folders), [tutorial.md](tutorial.md) (the
tutorial registry), [nation.md](nation.md).
The region screen and view (`cSC4WinRegionScreen`, `cSC4WinRegionView`) are not described here.

## 0. Summary

A region is a folder under the nation's regions folder holding `region.ini`, `config.bmp` and one `.sc4` file per
city tile. `config.bmp` marks where the tiles are and how big they are; the region loads every city file whose
place and size match a tile and moves the others to the Downloads folder. It keeps a map from each region cell to
the city covering it.

## 1. Identity *[compat]*

- Interface `cISC4Region` (IID `0x89D2BE26`), vtable `0x00A8FA40`, 26 slots in the SDK's order. `QueryInterface`
  (`0x004A44E0`) answers 1 and `0x89D2BE26` (adding a reference). `Release` (`0x004A4570`): a count above 1 is
  decremented and returned; otherwise `Shutdown` (slot 9), then the deleting destructor (slot 25, `0x004A53E0`; body
  `0x004A4C30`), and 0.
- Made by `cSC4Nation::LoadRegions` (`0x00497DA1`) and `CreateRegion` (§6); constructor `0x004A5B30`.

| Offset | Field | Made as |
|---|---|---|
| `+0x04` | initialised (byte) | 0 |
| `+0x08` | reference count | 0 |
| `+0x0C` | name (string; `GetName` slot 3, `SetName` slot 4) | empty |
| `+0x20` | directory name (string; slots 5, 6) | empty |
| `+0x34` | base terrain type (slots 18, 19) | 0 |
| `+0x38`, `+0x3C` | water minimum, water maximum | 60, 100 |
| `+0x40` | the overflow list (cities that fit no tile, §4) | empty |
| `+0x44` | the tile locations (`{x, z, size class}`, 12 bytes, §3) | empty |
| `+0x50` | the cell map: region cell → holder of the city covering it (§5); count at `+0x60` | empty, hint 100 |

Slots: 3–6 the names; 7 `LoadConfig` (§2, `0x004A6CD0`); 8 `Init` (§4, `0x004A6C80`); 9 `Shutdown` (§2, `0x004A5BE0`);
10 `Delete` (§6, `0x004A65A0`); 11–17 `GetCity`, `InsertCity`, `RemoveCity`, `DeleteCity`, `ReloadCity`, `MoveCity`,
`GetAllCities` (§5, §6); 18–21 the base terrain type, `GetBaseTerrainHeight` and `GetWaterPrefs` (§7); 22
`ResetTutorialCity` (§6); 23 `GetCityLocations`; 24 `GetBoundingRect` (§7).

The region's folder is the nation's regions folder (the application's slot 34, its slot 5) followed by the directory
name and `\`.

## 2. `region.ini` *[compat]*

**`LoadConfig`** reads the section `[Regional Settings]` of `<folder>region.ini` (`cRZIniFile`):

| Key | Rule |
|---|---|
| `Name` | `#` and a non-zero hex number: the localized string (group `0x6A231EAA`, that instance); else the text |
| `Terrain type` | taken when 0 or 1 |
| `Water Min`, `Water Max` | each taken when 0 to 254 |

A missing key or a value out of range leaves the field as it was.

**`Shutdown`** (once `Init` has run): every city is removed from the cell map (`RemoveCity`, §5), every city of the
overflow list is shut down, and `region.ini` is written back: the file is made writable first when it is not
*[inferred: file attribute 2]*, then `Name` (the current name as text), `Terrain type`, `Water Min` and `Water Max`
(numbers). True.

## 3. `config.bmp`: the tile locations *[compat]*

**`LoadCityLocations`** (`0x004A6B00`) loads `<folder>config.bmp` as an image; when the file is missing or does not
load, it uses the game's built-in layout (resource `0x856DDBAC / 0x6A1EED2C / 0x2A1F0276`). The image is scanned
(`0x004A5640`) row by row (z), each row from left to right (x), with a "taken" mark per pixel, starting clear:

1. A pixel already taken is skipped.
2. Its colour gives the tile size `s`: red 255 → 1, green 255 → 2, blue 255 → 4, counting only the channels that
   are exactly 255. A colour with no such channel, or with more than one, is skipped (and not marked).
3. The tile must fit: `x + s ≤ width` and `z + s ≤ height`; otherwise skipped.
4. The `s × s` pixels from `(x, z)` are marked taken, and the location `{x, z, s / 2}` (size class 0, 1, 2) is added.

Only the first pixel's colour is read; the other pixels of the tile are taken whatever their colour *[quirk]*. The
list is emptied first.

## 4. `Init` and the city files *[compat]*

**`Init`** (once): `LoadConfig`, `LoadCityLocations`, `LoadCities`, then `MoveOverflowCitiesToDownloadDirectory`.
It returns whether the cell map holds any city (also on later calls).

- **`LoadCitiesFromDirectory(list, folder)`** (`0x004A58D0`): nothing when the folder does not exist. Each file of
  the folder whose extension is exactly `.sc4` is read with `SC4ReadRegionalCity(path, &city)` (§8). A city read is
  added to `list`; its file is given attribute 2 when it lacks it *[inferred: writable]*, and its save-file path is
  set (regional-city slot 32).
- **`LoadCities`** (`0x004A6330`): loads the folder's cities into a temporary list; then for each, in order, when no
  cell of its rectangle is in the cell map yet (`0x004A4970`): the first location whose square contains the city's
  corner decides; when the location's corner equals the city's position and `1 << sizeClass` equals the city's size,
  the city is inserted (`InsertCity`) and leaves the list; otherwise it stays. The cities left over join the overflow
  list.
- **`MoveOverflowCitiesToDownloadDirectory`** (`0x004A51C0`): each overflow city is taken off the list and shut down;
  its file is copied to the nation's Downloads folder (the nation's slot 15 gives the folder, then
  `SC4CreateSavePathForCity` with the city's name and serial number) and, when the copy succeeds, the original is
  removed.

## 5. The cell map *[compat]*

The map is a hash table keyed by region cell, with the bucket `((x & 15) << 4 | (z & 15)) mod buckets`. Each city
has one heap **holder** (a reference to the city); every cell of its rectangle (position, size) maps to that holder.

- **`GetCity(x, z)`**: the holder of that cell or null (neighbors.md NG15).
- **`InsertCity(city)`**: a new holder with a reference to `city`; each cell of the city's rectangle maps to it,
  replacing what was there.
- **`RemoveCity(&h)`**: the cells of the city's rectangle are erased, the city's `Shutdown` runs, and the holder is
  released and freed; the caller's `h` then points to freed memory.
- **`DeleteCity(&h)`**: the city's save path is taken, `RemoveCity(&h)`, and the file is deleted.
- **`ReloadCity(&h)`**: neighbors.md §14.1 (remove, read the file again, insert, `h` := the new holder; the result is
  not set).
- **`GetAllCities(list)`**: each city once (a set of the holders' cities), appended to `list` with a reference, in
  the order of the cities' addresses.

## 6. Moving, deleting, creating *[compat]*

- **`MoveCity(source, city, x, z)`** (`0x004A7200`): a `source` equal to this region counts as none.
  1. When `(x, z)` already holds `city`: true. When it holds another city, that city is deleted (`DeleteCity`).
  2. The city's save path, name and serial number are taken. While a city of this region (a cell map entry) has the
     same serial number, the serial becomes `SC4CreateCitySerialNumber()` (§8), then one less on each further clash.
  3. The source region (when given) and this region are shut down (`Shutdown`).
  4. The file is renamed to `SC4CreateSavePathForCity(name, serial)`; on failure the result is false.
  5. The renamed file is made writable *[inferred]*, read (`SC4ReadRegionalCity`), given the serial number
     (regional-city slot 28) and the position `(x, z)` with rearrange (slot 6, so `moved` is set), written back
     (`SC4WriteRegionalCity`), shut down and released.
  6. This region's `Init` runs again. True unless step 4 failed.
- **`Delete`** (`0x004A65A0`): every city is deleted (`DeleteCity`, files included) until the map is empty; then
  `config.bmp` and `region.ini` are deleted, the folder is removed (`0x0091BF93`), and `Shutdown` runs.
- **`ResetTutorialCity(id)`** (`0x004A5400`): false for −1. Among all cities, the first whose tutorial ID (its slot
  65) is `id` has its file overwritten with the built-in tutorial city (record `0x6A5B7BF5 / 0x6A5B7B57 / id`,
  `SC4ReadBinaryRecord`); when that works, the city is reloaded (`ReloadCity` of the holder at its position) and
  given the tutorial ID again (slot 66), the application's slot 49 runs, and the tutorial is made active again
  (`cSC4TutorialRegistry::SetActive(id, 1)`, tutorial.md). The result is whether the record was written
  *[inferred]*.
- **`CreateRegion(name)`** (`0x004A67B0`, a static function; `cSC4Nation::AddNewRegion`): a new region; the folder
  name is the sanitized name (`SC4SanitizeFileName`), and `"<name> (n)"` with n = 1, 2, … while that folder exists;
  the region's name and directory name are set and the folder is created. The game's layouts
  `0x856DDBAC / 0x6A1EED2C / 0x6C3568C4 + i` are counted for i = 0, 1, … while the resource manager has them (its slot
  14), and one of them, chosen with `RandomUint32Uniform(count)`, is written as `<folder>config.bmp`. The new region
  is returned (not initialised) *[inferred]*.
- **`CreateStarterRegion`** (`0x004A6AC0`; `cSC4Nation::Init`): `CreateRegion` with a default-constructed name
  *[inferred: empty]*.

## 7. Queries *[compat]*

- **`GetBoundingRect(rect)`**: the minimum and maximum of the locations' corners (`x0, z0, x1, z1`, the tiles'
  sizes are not added); all four 0 when there is no location or `x1 ≤ x0` or `z1 ≤ z0` *[quirk]*.
- **`GetCityLocations(list)`**: a copy of the location list.
- **`GetBaseTerrainHeight`**: the entry of a two-entry table (`0x00B217B0`, set by the region screen, `0x007AC06A`)
  for the base terrain type. **`GetWaterPrefs(&min, &max)`**: the two water values as bytes.

## 8. File helpers *[compat]*

- **`SC4ReadRegionalCity(path, &city)`** (`0x005DC220`): opens the file as a package (a database segment, read only);
  reads only when the file's application save version (`SC4ReadAppLoadSaveVersion`) is 62 to 65, through the package
  form (regional-city.md §1); then closes and releases the package. **`SC4WriteRegionalCity(path, city)`**
  (`0x005DC2E0`) opens the file the same way, with the same version test, and writes the record (`0x005DC1A0`).
- **`SC4CreateCitySerialNumber()`** (`0x005DBD40`): `((now − 2000-01-01 00:00 local + 28800) << 4) | r`, with `now`
  the time in seconds, the date through `mktime`, and `r` four random bits *[inferred: 28800 s is an 8-hour time-zone
  offset]*.
- **`SC4CreateSavePathForCity(name, serial, &path)`** (`0x005DC790`): the path of a city file in the current region's
  folder *[summary: the file-name rule of `0x005DC400` is not traced; the files are named `City - <name>.sc4`]*.

## 9. Quirks

1. Only the first pixel of a `config.bmp` tile is read (§3).
2. A city whose corner lies inside a tile but does not match it exactly goes to Downloads (§4).
3. Moving a city onto another city's tile deletes that city's file (§6).
4. `GetBoundingRect` ignores the tile sizes (§7).

## 10. Open questions

See [questions/region.md](questions/region.md): RQ1 (the file attribute 2 the region sets on its files), RQ2 (the
file-name rule of `SC4CreateSavePathForCity` and the starter region's name).

## 11. Conformance tests

1. **Tiles:** a 4 × 4 image with blue at (0, 0) and red at (0, 1)… gives one location `{0, 0, 2}`; the red pixel is
   inside the taken square and is skipped.
2. **Colours:** a pixel (255, 255, 0) gives no tile.
3. **Fit:** a green pixel at x = 3 of a 4-wide image gives no tile.
4. **Overflow:** a city at (1, 0) of size 1 with a single location `{0, 0, 1}` (2 × 2 at the origin) goes to the
   overflow list.
5. **ini:** `Terrain type = 2` leaves the terrain type unchanged; `Water Min = 254` is taken.


## Complementary region analysis

See [region-model.md](region-model.md) for the other contributor's independent region analysis. Both analyses and their open RM/RQ questions are retained. Coverage credits exact cSC4Region once; this cross-reference does not promote partial terrain-map/view or location/signpost helper owners. Inferred naming/argument and filesystem observations retain their tags until their questions are independently resolved.
