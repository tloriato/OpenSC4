# Spec: the region model (`cSC4Region`, `cSC4RegionalTerrainMap`, `cSC4RegionalTerrainView`)

Status: **behaviour spec**, round 1 (re-region-model, 2026-10-21), clean-room input. This document describes
*behaviour*, not code. It was written by the analysis team from the Windows 1.1.641 executable (the GOG build,
SHA-256 `3bc5c7fe807fe5aa24a3abb06e40f13486670bc1150612c09baf4ecedafe1a4b`, read in the instruction bytes and in a
Ghidra 12.1.4 project), and checked against the six regions the game ships (`Regions\Berlin`, `Fairview`, `London`,
`New York`, `San Francisco`, `Timbuktu`: 6 `region.ini`, 6 `config.bmp`, 417 city files). Implementers must work from
this spec, the public LGPL gzcom-dll SDK headers, the game's data files and general knowledge only. They must **not**
consult decompiled or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven (read from the bytes, not seen running);
*[quirk]* is surprising original behaviour that is kept on purpose; *[compat-limit]* depends on something
FreeCity4 cannot reproduce (heap addresses, the file system's listing order), so only the rule is matched, not each
value.

Related specs: [nation.md](nation.md) (the list of regions and the active region; it creates, initialises, shuts
down and deletes the regions described here), [foundations.md](foundations.md) (§11 the regional-city record each
city file carries), [neighbors.md](neighbors.md) (§9 the cities next to a city, `GetCity` as used by the
simulator), [paths.md](paths.md) (`cRZDirectory`, path joining), [framework-rz-files.md](framework-rz-files.md)
(`cRZIniFile`, `cRZFile`), [application.md](application.md) (§9 the user folders, §11.16 how a city file is
saved), [tutorial.md](tutorial.md) (the tutorial registry that `ResetTutorialCity` re-arms),
[save-records.md](save-records.md) (the app version record that gates which city files are read).

## 0. Summary

A **region** is one folder under the user's `Regions\` folder. It holds `region.ini` (name, terrain type, water
range), `config.bmp` (a small picture that says where the city tiles of the region are and how big each is) and one
`City - *.sc4` file for every city tile. `cSC4Region` (vtable `0x00A8FA40`, 26 slots, object size `0x64`) is the
in-memory region: a name, a directory name, a base terrain type, a water range, the list of **locations** read from
the picture, and a **cell map** from region cells to the regional-city objects that occupy them.

- **Loading** (`Init`, §6.1) reads `region.ini`, parses `config.bmp` into locations, reads every `.sc4` file of the
  folder as a regional city, puts each city whose position and size equal a location into the cell map, and moves
  every other city file (a second city on the same cell, a city that fits no location) out to
  `Regions\Downloads\`.
- **Unloading** (`Shutdown`, §6.2) forgets all cities and rewrites `region.ini` from memory.
- **Creating** (`CreateRegion`, §6.4) makes a new folder named after the region and copies one of the game's
  built-in picture templates into it as `config.bmp`.
- **City file work** (§7): reload, delete, move and move-out of single city files, and resetting a tutorial city
  from the pristine copy in the game data.
- The **regional terrain map** (§9) is a flat terrain stand-in used by the region screen, and the **regional
  terrain view** (§10) draws it as one coloured rectangle.

The class does no simulation. Everything it touches is file-system work, the cell map, and calls on the regional
city (`cISC4RegionalCity`) and on the application's nation object (nation.md).

Three results that are easy to get wrong:

1. The region's bounding rectangle ignores city sizes and is all zeros for a region with only one distinct
   top-left corner (§5.6).
2. `Shutdown` writes the **resolved** region name back into `region.ini`, so a shipped `Name = #0x...` token is
   replaced by the language's text the first time the region is shut down (§3.2).
3. A city file that does not exactly fill a configured location, or that sits on an already taken cell, is **moved
   out of the region** and deleted from its folder at load time (§5.3, §7.5). The shipped Timbuktu folder has such a
   file (§12).

## 1. The object

### 1.1 Interface and identity *[compat]*

`cSC4Region` implements `cISC4Region`. It has no class factory entry: the nation constructs it directly (nation
`LoadRegions`, `CreateRegion`).

| Query | Behaviour |
|---|---|
| `QueryInterface(iid, out)` (`0x004A44E0`, pops 8) | for `iid = 1` (the base unknown interface) or `iid = 0x89D2BE26` (`cISC4Region`): `*out := this`, `AddRef`, return true. Any other id: return false and leave `*out` untouched |
| `AddRef()` (slot 1, `0x004BDF00`, a shared increment body) | `refcount := refcount + 1`; returns the new count. Not atomic |
| `Release()` (`0x004A4570`) | if `refcount > 1`: decrement, return the new count. Otherwise (`refcount` is 0 or 1): call `Shutdown()` (slot 9) and then the deleting destructor (slot 25 with 1), and return 0 |

The count starts at **0**, not 1 *[quirk]*: the creator (nation `LoadRegions` and `AddNewRegion`) takes a reference
before it stores the pointer. A region released from count 0 or 1 is always shut down first, so an initialised
region that is dropped writes `region.ini` (§3.2).

### 1.2 Fields *[compat]* (only as far as the plugin ABI shows them; the object is 100 bytes)

| Offset | Meaning | Initial |
|---|---|---|
| `+0x00` | vtable `0x00A8FA40` | |
| `+0x04` | byte: initialised. Set by `Init`, cleared by `Shutdown` | 0 |
| `+0x08` | reference count | 0 |
| `+0x0C` | the **name** (an embedded `cRZString`; text at `+0x10`) | empty |
| `+0x20` | the **directory name** (an embedded `cRZString`; text at `+0x24`) | empty |
| `+0x30` | a dword no function of this class reads | 0 *[inferred]* |
| `+0x34` | base terrain type | 0 |
| `+0x38` | water minimum | 60 |
| `+0x3C` | water maximum | 100 |
| `+0x40` | the **overflow list**: a circular list of counted references to regional cities (§5.3) | empty |
| `+0x44 / +0x48 / +0x4C` | the **locations**: a vector of 12-byte records `{ s32 x, s32 y, s32 sizeExp }` (§4) | empty |
| `+0x50 .. +0x60` | the **cell map**: a hash table from region cell `(x, z)` to a heap box holding one counted reference to a regional city. `+0x60` is the number of entries | 100 buckets, empty |

The name and the directory name are the objects that `GetName` (slot 3, `0x00577450`) and `GetDirectoryName` (slot
5, `0x006D4790`) return **by reference** (`this + 0x0C` and `this + 0x20`). `SetName` (slot 4) and
`SetDirectoryName` (slot 6) copy the argument string into them. The directory name is a **single folder name**
relative to the nation's region directory (§2), never a full path.

The cell map's hash is `((x & 15) << 4 | (z & 15))` modulo the bucket count; it is not observable.

### 1.3 Vtable (`0x00A8FA40`) *[compat]*

| Slot | Offset | Method | Address | Pops | Notes |
|---|---|---|---|---|---|
| 0 | `+0x00` | `QueryInterface` | `0x004A44E0` | 8 | §1.1 |
| 1 | `+0x04` | `AddRef` | `0x004BDF00` | 0 | |
| 2 | `+0x08` | `Release` | `0x004A4570` | 0 | |
| 3 | `+0x0C` | `GetName` | `0x00577450` | 0 | returns `&name` |
| 4 | `+0x10` | `SetName(const cIGZString&)` | `0x004A4510` | 4 | tail-calls the string's assign |
| 5 | `+0x14` | `GetDirectoryName` | `0x006D4790` | 0 | returns `&directoryName` |
| 6 | `+0x18` | `SetDirectoryName(const cIGZString&)` | `0x004A4520` | 4 | tail-calls the string's assign |
| 7 | `+0x1C` | `LoadConfig` | `0x004A6CD0` | 0 | §3.1 |
| 8 | `+0x20` | `Init` | `0x004A6C80` | 0 | §6.1 |
| 9 | `+0x24` | `Shutdown` | `0x004A5BE0` | 0 | §6.2 |
| 10 | `+0x28` | `Delete` | `0x004A65A0` | 0 | §6.3 |
| 11 | `+0x2C` | `GetCity(x, z)` | `0x004A4930` | 8 | §5.4 |
| 12 | `+0x30` | `InsertCity(city)` | `0x004A4F00` | 4 | §5.5 |
| 13 | `+0x34` | `RemoveCity(&box)` | `0x004A4B50` | 4 | §5.5 |
| 14 | `+0x38` | `DeleteCity(&box)` | `0x004A5120` | 4 | §7.2 |
| 15 | `+0x3C` | `ReloadCity(&box)` | `0x004A5030` | 4 | §7.1 |
| 16 | `+0x40` | `MoveCity(region, city, x, z)` | `0x004A7200` | 16 | §7.3 |
| 17 | `+0x44` | `GetAllCities(list&)` | `0x004A4CE0` | 4 | §5.4 |
| 18 | `+0x48` | `GetBaseTerrainType` | `0x00A6CAB0` | 0 | returns `+0x34` |
| 19 | `+0x4C` | `SetBaseTerrainType(t)` | `0x004A4CB0` | 4 | stores `t` unchecked |
| 20 | `+0x50` | `GetBaseTerrainHeight` | `0x004A4CA0` | 0 | §8.1 |
| 21 | `+0x54` | `GetWaterPrefs(&min, &max)` | `0x004A4CC0` | 8 | §8.2 |
| 22 | `+0x58` | `ResetTutorialCity(id)` | `0x004A5400` | 4 | §7.6 |
| 23 | `+0x5C` | `GetCityLocations(vector&)` | `0x004A4C20` | 4 | §5.6 |
| 24 | `+0x60` | `GetBoundingRect(rect*)` | `0x004A45A0` | 4 | §5.6 |
| 25 | `+0x64` | deleting destructor | `0x004A53E0` | 4 | §1.4 |

`GetCity`'s result and the `&box` argument of `RemoveCity`, `DeleteCity` and `ReloadCity` are the **boxes** of §1.2:
`GetCity` returns the box pointer (not the city), and a caller that wants to remove or reload a city passes the
address of a variable that holds that box pointer. `GetAllCities`' argument is the address of a list head; its
elements are counted references.

### 1.4 Constructor and destructor

The constructor (`0x004A5B30`) sets the fields to the initial values of §1.2. The destructor body (`0x004A4C30`)
empties the cell map without releasing the boxes (a region is shut down before it is destroyed, which empties the
map), destroys the locations, the overflow list and the two strings. `Release` guarantees the `Shutdown` before it.

## 2. Folders and file names

Everything below is under the application's **user data** folder (application.md §9): `Documents\SimCity 4\`.

```
Regions\                             created on first use by the nation (nation StaticInit)
    Downloads\                       created likewise; never listed as a region; receives overflow city files
    <directory name>\                one folder per region (§2.1)
        region.ini                   §3
        config.bmp                   §4
        City - <city name>.sc4       one file per city tile (§5.1, §7.4)
```

The region reaches these folders **only** through the application's nation object (nation.md §4): it asks for
`GetRegionDirectory()` (slot 5, the string `<user data>\Regions\`, ending in a backslash) and
`GetRegionDownloadDirectory()` (slot 15, `<user data>\Regions\Downloads\`). The path of the region's own folder is
always

```
folder := RegionDirectory + directoryName + "\"
```

so a region object works only relative to the application's one Regions folder *[compat]*.

### 2.1 How a region gets its directory name *[compat]*

- The nation's `LoadRegions` (`0x00497CF0`) lists the sub-folders of `Regions\`. A folder named `Downloads`
  (compared as strings; the case rule of the comparison is *[inferred]* to be case-insensitive) is skipped. For
  every other folder it creates a region and calls `SetName(folderName)`, `SetDirectoryName(folderName)` and
  `LoadConfig()` in that order. So the **name defaults to the folder name** and `region.ini` may replace it (§3.1).
- `CreateRegion(name)` chooses the folder (§6.4).

### 2.2 Which files are city files *[compat]*

A file in the region folder is a city file when it is a regular file (not a folder) whose **extension is exactly
`.sc4`** — the comparison is byte-exact, so `.SC4` is not recognised *[quirk]* — and which reads as a regional city
(§5.1). The name does not have to start with `City - `; the game writes that prefix (§7.4) but does not require it.
The listing is not recursive. The order the files are met is the order the file system lists them
*[compat-limit]*; it decides which of two cities on the same cell is kept (§5.3).

## 3. `region.ini`

A text file `<folder>region.ini` in the format that framework-rz-files.md §3 describes (sections in brackets,
`key = value` lines, keys and sections case-insensitive, CRLF line ends when written). The only section is
`[Regional Settings]`. Shipped example:

```
[Regional Settings]
Name = #0xcab6d26e
Terrain type = 0
Water Min = 60
Water Max = 100
```

(All six shipped regions have exactly these four keys with `Terrain type = 0`, `Water Min = 60`, `Water Max = 100`;
only the `Name` token differs.) A region created by `CreateRegion` has **no** `region.ini` until its first
`Shutdown`.

### 3.1 Reading: `LoadConfig` (slot 7, `0x004A6CD0`) *[compat]*

`LoadConfig` runs at the start of `Init` (§6.1) and also from the nation's `LoadRegions`. It does not clear anything:
every field keeps its current value unless the file gives a valid replacement. A missing file, section or key leaves
the field alone.

| Key | Rule |
|---|---|
| `Name` | read as text. If the read succeeded and the text is **not empty**: if it starts with `#` and the rest, parsed as a hexadecimal number (`RZAtoUint32HexA`: blanks, optional `0x`, at most 8 digits), is **non-zero** `N`, then `name :=` the localized text of the LTEXT resource (type `0x2026960B`) in group `0x6A231EAA`, instance `N`, loaded in the current language with the detokenize flag set (neighbors.md §6 gives that loader's rules: a missing LTEXT gives an **empty** name); otherwise `name :=` the text as written (including a `#` that did not parse) |
| `Terrain type` | decimal integer (`ReadNumber`, so `0x1` reads as 0). Accepted only when `0 <= v < 2`; otherwise the field is left alone |
| `Water Min` | decimal integer; accepted only when `0 <= v < 255` |
| `Water Max` | decimal integer; accepted only when `0 <= v < 255` |

There is no check that `Water Min <= Water Max`. The values are stored as 32-bit integers.

### 3.2 Writing: `Shutdown` (§6.2) *[compat]*

`Shutdown` writes the four keys, in this order, with `WriteEntry` / `WriteNumber` (framework-rz-files.md §3.6):
`Name` (the **current** name text), `Terrain type`, `Water Min`, `Water Max` (decimal). If the file is read-only
(`cRZFile` attribute 2 not set) it is made writable first. An existing key is replaced in place, a missing one is
added at the end of the section, and a missing file is created, so a region folder that exists gets its
`region.ini` the first time the region is shut down.

*[quirk]* Because the name is written from memory, a `Name = #0x...` token that `LoadConfig` resolved is written
back as the **resolved text** in the language of that session. The localisation token is lost after the first
shutdown of an initialised region *[inferred]*: both halves are read from the bytes, the combination has not been
seen running (RM1).

*[inferred]* When the folder no longer exists (§6.3), creating the file fails and nothing is written (RM2).

## 4. `config.bmp`: where the city tiles are

`config.bmp` is a small picture. Each coloured square in it is one **location** of a city tile; the picture's
pixel grid is the region's cell grid. The shipped files are 24-bit BMPs: 16 x 16 for Berlin, London, New York and
Timbuktu, 15 x 15 for San Francisco, 8 x 8 for Fairview.

### 4.1 `LoadCityLocations` (`0x004A6B00`, run by `Init` after `LoadConfig`) *[compat]*

1. The picture is `<folder>config.bmp`. If that file exists it is decoded as a 32-bit-per-pixel buffer (colour type
   9, the A8R8G8B8 family; the colour order of the three channels is used as red, green, blue, §4.2).
2. If the file does not exist, or does not decode, the **built-in default** is used instead: the record of type
   `0x856DDBAC`, group `0x6A1EED2C`, instance `0x2A1F0276` in the game's data (the same type and group as the
   templates of §6.4).
3. If neither can be had, nothing happens and the locations keep their previous contents (empty after
   construction).
4. Otherwise the locations are **cleared** and re-read from the picture by §4.2.

### 4.2 Reading the picture *[compat]*

Let `w` and `h` be the width and height of the picture's rectangle. Scan the pixels row by row, `y = 0 ..< h` (the
**top** row of the picture first), and in each row `x = 0 ..< w`. Keep a set of visited cells (initially empty).

```
for y in 0 ..< h:
  for x in 0 ..< w:
    if (x, y) is visited: continue
    (r, g, b) := the pixel
    code := (r == 255 ? 1 : 0) + (g == 255 ? 2 : 0) + (b == 255 ? 4 : 0)
    if code is not 1, 2 or 4: continue          // zero or two or three channels at 255
    if x + code > w or y + code > h: continue   // the square would leave the picture
    mark every cell (x + i, y + j), 0 <= i, j < code, visited
    append location { x, y, sizeExp = log2(code) }   // 0, 1 or 2
```

So a pixel with exactly one channel at 255 starts a square: **red = 1 x 1** (small city, `sizeExp` 0), **green = 2 x 2**
(medium, 1), **blue = 4 x 4** (large, 2). The other two channels can have any value other than 255, and only the
first pixel met (the top-left corner) is looked at; the rest of the square is not compared with it, only marked
visited. A pixel that does not start a valid square and is not already covered is ignored and stays unvisited, so a
later pixel is not affected by it.

Applied to the six shipped pictures with the **top row of the BMP as `y = 0`** (a BMP stores its rows bottom-up,
so the decoder's `y = 0` is the top row on screen) this gives 85, 49, 73, 49, 69 and 91 locations for Berlin,
Fairview, London, New York, San Francisco and Timbuktu. For every one of the 417 shipped city files the pair
`(positionX, positionZ)` and the size in their regional-city record is exactly one of the locations, with
`positionX = x`, `positionZ = y` (the file's `sizeX` equals `1 << sizeExp`; all 417 are square). The
vertical orientation is proven by this match *[inferred]* (RM3: which decoder flips the BMP).

The location vector keeps discovery order (row-major by top-left corner) and is what `GetCityLocations` returns.

## 5. City files and the cell map

### 5.1 Reading one city file: `LoadCitiesFromDirectory` (`0x004A58D0`) *[compat]*

Input: a folder path (ending in a backslash) and an output list of counted regional-city references.

1. If the folder does not exist, do nothing.
2. For each file entry of the folder (§2.2) whose extension is `.sc4`: read it as a regional city. This is the
   game's `SC4ReadRegionalCity` (`0x005DC220`):
   - the file is opened as a package; a package that cannot be opened fails;
   - the **application version record** of the package (type `0xEA3FF4F5`, group `0xEA3FF506`, instance
     `0xEA3FF4F5`; save-records.md lists it) must give a version from **62 to 65** inclusive, else the file is
     ignored. A package with no readable version record gives 65 or 0, depending on a second test of the package
     that is not decoded (RM4);
   - the **regional-city record** (type `0xCA027EDB`, group `0xCA027EE1`, instance 0; foundations.md §11) is read.
     A package without it, or with a record that fails to read (foundations.md §11.2: version 14 or more, a limit
     exceeded), is ignored.
   Files that fail are skipped silently; there is no message.
3. For a file that read: if the file is read-only, clear the read-only attribute. The new city gets the file's full
   path as its **file path** (regional city slot `+0x80`), and is appended to the output list.

### 5.2 `LoadCities` (`0x004A6330`, run by `Init` after `LoadCityLocations`) *[compat]*

```
folder := RegionDirectory + directoryName + "\"
loaded := LoadCitiesFromDirectory(folder)           // in file listing order
for each city C in loaded, in order:
    if OverlapsOccupiedCell(C): continue             // stays in `loaded` (§5.3)
    (px, pz) := C.GetPosition();  sx := C.GetCitySize().x
    for each location L in the locations vector, in order:
        if (px - L.x) and (pz - L.y) are both in [0, 1 << L.sizeExp):    // C's corner lies inside L
            if px == L.x and pz == L.y and sx == (1 << L.sizeExp):       // and L is C's exact tile
                InsertCity(C);  remove C from `loaded`
            break                                                         // either way, no further location is tried
append what is left of `loaded` to the overflow list (+0x40)
```

`OverlapsOccupiedCell(C)` (`0x004A4970`) is true when **any** cell of the rectangle
`[px, px + sizeX) x [pz, pz + sizeZ)` already has an entry in the cell map. The comparison with the location uses
only the X size of the city; the Z size is never compared *[quirk]* (all shipped cities are square).
The "inside" test is unsigned, so a corner left of or above the location fails it.

### 5.3 What ends up in the overflow list *[compat]*

After `LoadCities` the cell map holds the cities that exactly fill a location and did not collide. The overflow list
holds, in load order:

- a city whose rectangle touches a cell an **earlier** city already took (two files for one location: the one met
  first in the listing wins *[compat-limit]*);
- a city whose corner lies in no location, or lies in a location it does not exactly fill (wrong corner, wrong
  size).

`Init` then calls `MoveOverflowCitiesToDownloadDirectory` (§7.5), which empties this list and moves the files.

### 5.4 Queries

- `GetCity(x, z)` (`0x004A4930`): the box pointer stored for the cell `(x, z)`, or null.
- `GetAllCities(list)` (`0x004A4CE0`): collects the distinct cities of the cell map into an ordered set keyed by the
  **object address**, then appends one counted reference per city to the **end** of `list` in ascending address
  order. The list is not cleared first. The order depends on heap addresses *[compat-limit]*; neighbors.md §9.1
  (NB14) has the same finding for its callers. FreeCity4 *[choice]*: order by position (x, then z).
- `GetCityLocations(out)` (`0x004A4C20`): `out :=` a copy of the locations vector (12-byte records, §1.2).

### 5.5 Changing the cell map

- `InsertCity(city)` (`0x004A4F00`): allocates one 4-byte box that holds a new counted reference to `city`, then for
  every cell `(px + i, pz + j)`, `0 <= i < sizeX`, `0 <= j < sizeZ`, stores the **same box** in the cell map,
  replacing whatever was there. A replaced box is not released and not freed *[quirk]*: a city inserted onto an
  occupied cell leaks the old box and leaves the old city's other cells pointing at it. The caller (§5.2) rules
  this out by the overlap test. A city with `sizeX <= 0` or `sizeZ <= 0` stores nothing (and its box is leaked).
- `RemoveCity(&box)` (`0x004A4B50`): takes a hold on the city, erases all cells of its rectangle from the map,
  calls the city's `Shutdown`, releases and frees the box that the argument points to, releases its hold. The city
  object itself lives on while anyone else holds it. Cells are erased by key, so a rectangle that overlaps cells of
  another city would erase those too; the cell map normally prevents that.

### 5.6 Rectangle: `GetBoundingRect(rect*)` (`0x004A45A0`) *[compat]*

`rect` is four `s32`: `{ minX, minY, maxX, maxY }`. Start with `{ INT_MAX, INT_MAX, INT_MIN, INT_MIN }`; for every
location take the **top-left corner only** `(x, y)` and fold it into all four. Afterwards, if `maxX <= minX` or
`maxY <= minY`, set all four to **0**. So the maximum is the largest top-left corner, not the largest extent; a
region with no locations, with one location, or with all locations in one row or column, gives all zeros; and a
region whose corners span `n` cells gives a rectangle `n` cells wide even though the cities reach one more tile (or
three more for a large one). Callers add one (the region screen, §9.1) *[quirk]*.

## 6. Life cycle

### 6.1 `Init` (slot 8, `0x004A6C80`) *[compat]*

```
if not initialised:
    initialised := true
    (a process-wide flag at 0x00B217B8 is set; nothing reads it)
    LoadConfig(); LoadCityLocations(); LoadCities(); MoveOverflowCitiesToDownloadDirectory()
return (number of cell-map entries) != 0
```

A second `Init` without a `Shutdown` does nothing and returns the same test. The result is true when at least one
city occupies a cell. The nation calls `Init` through `SetActiveRegion` (nation.md §4 slot 9).

### 6.2 `Shutdown` (slot 9, `0x004A5BE0`) *[compat]*

Always returns true. If the region is not initialised it does nothing else. Otherwise:

1. `initialised := false`.
2. While the cell map is not empty: take any entry and `RemoveCity` its box (§5.5). Every city of the map is shut
   down and released.
3. Call `Shutdown` on every city of the overflow list, then empty the list (this normally has no entries: `Init`
   already emptied it).
4. Write `region.ini` (§3.2).

The locations are **kept** (they are re-read by the next `Init`).

### 6.3 `Delete` (slot 10, `0x004A65A0`) *[compat]*

Deletes the region from disk.

1. While the cell map is not empty: `DeleteCity` (§7.2) on the first entry (this removes the city from the map and
   deletes its file).
2. Remove `<folder>region.ini` and `<folder>config.bmp` (`cRZFile::Remove`).
3. Remove the folder **recursively** (`cRZDirectory::RemoveDirectoryA(folder, true)`, paths.md §3.8), which also
   takes any other file in it, including the city files of the overflow list (empty after a normal `Init`) and
   files that were not loaded.
4. Call `Shutdown` (slot 9).

The nation first makes the region active (`SetActiveRegion`) so that `Delete` finds the cities loaded (nation.md,
`DeleteRegion`). *[quirk]* Step 4 then tries to write `region.ini` into the folder that step 3 removed (§3.2).

### 6.4 `CreateRegion(name)` (static, `0x004A67B0`) and `CreateStarterRegion` (static, `0x004A6AC0`) *[compat]*

`CreateRegion` takes a `cIGZString` and returns a **new, uninitialised** region (count 0). It never fails to return
the object, even when the folder cannot be created.

1. `s :=` `SC4SanitizeFileName(name)` (`0x005F9BF0`): the characters `< > : \ / " | * ?` are removed, the text is cut
   to 256 bytes, and trailing bytes are dropped until what is left is valid in the system code page's encoding (the
   exact conversion path is *[inferred]*, RM5).
2. The directory name is the first of `s`, `s (2)`, `s (3)`, ... (`"%s (%d)"`, counting from 2) for which
   `RegionDirectory + candidate + "\"` is not an existing folder.
3. `region.SetName(name)` (the **unsanitised** name) and `region.SetDirectoryName(candidate)`.
4. Create the folder (`CreateDirectoryA`, which also creates missing parents, paths.md §3.7).
5. If it was created: count the templates, `N :=` the number of consecutive instances `0x6C3568C4`, `0x6C3568C5`, ...
   that exist as type `0x856DDBAC`, group `0x6A1EED2C` in the game's data. If `N > 0`, pick `k` uniformly in
   `[0, N)` (the game's own random generator) and **export** the record `(0x856DDBAC, 0x6A1EED2C, 0x6C3568C4 + k)`
   as the file `<folder>config.bmp`. The file is the record's bytes as they are in the data (a bitmap), and an
   export that fails removes a file it created.

The region does not read the file back; its first `Init` does. With `N = 0` the folder has no `config.bmp` and
`Init` uses the built-in default picture (§4.1).

*[quirk]* `CreateStarterRegion` (called by the nation when no region exists and none could be copied) calls
`CreateRegion` with an **empty** name. The sanitised name is empty, the candidate `Regions\` + `""` + `\` is the
Regions folder itself and exists, so the first free candidate is `" (2)"`. The starter region's folder is therefore
named ` (2)` (a space, then `(2)`) and its name is empty *[inferred]*: derived from the bytes, not seen running (RM6).

## 7. Operations on single city files

The city is always given as a box (§1.3) or a regional-city pointer. The calls made on `cISC4RegionalCity`
(vtable `0x00A8FB20`, foundations.md §11 and neighbors.md §15 have the others):

| Slot | Offset | Meaning here |
|---|---|---|
| 4 | `+0x10` | `Shutdown`: unsubscribe the city's messages |
| 5 | `+0x14` | `GetPosition(&x, &z)` |
| 6 | `+0x18` | `SetPosition(x, z, flag)`; with `flag` true also recomputes the cached connection data *[inferred]* |
| 7 | `+0x1C` | `GetCitySize(&sx, &sz)` |
| 27 / 28 | `+0x6C` / `+0x70` | `GetSerialNumber()` / `SetSerialNumber(n)` |
| 31 / 32 | `+0x7C` / `+0x80` | `GetFilePath(out)` / `SetFilePath(in)`: the string at `+0xBC`; **not stored in the file** |
| 33 / 34 | `+0x84` / `+0x88` | `GetCityName(out)` / `SetCityName(in)`: the string at `+0xD0` |
| 65 / 66 | `+0x104` / `+0x108` | `GetTutorialID()` / `SetTutorialID(id)`; -1 means none |

### 7.1 `ReloadCity(&box)` (slot 15, `0x004A5030`) *[compat]*

Re-reads one city from its file. `path := city.GetFilePath()`; `RemoveCity(&box)` (§5.5); read `path` as a regional
city (§5.1 step 2). If that succeeded: the new city gets `SetFilePath(path)`, is passed to `InsertCity`, and
`*(&box)` is set to `GetCity(position)` of the new city (a fresh box). If the read failed the city is **gone** from
the map and the argument is left pointing at the freed box *[quirk]*. The neighbours simulator calls this for every
adjacent established city in its `Init` (foundations.md §11.1).

### 7.2 `DeleteCity(&box)` (slot 14, `0x004A5120`) *[compat]*

`path := city.GetFilePath()`, `RemoveCity(&box)`, then **delete the file** at `path` (`cRZFile::Remove`, a hard
delete, no recycle bin).

### 7.3 `MoveCity(region, city, x, z)` (slot 16, `0x004A7200`, pops 16) *[inferred]*

The roles of the four arguments are read from how they are used; the function has no caller in the executable
(it is reached only through the region view's interface calls) (RM7). Returns a bool.

```
R := region;  if R == this: R := null
occ := GetCity(x, z)
if occ exists:
    if *occ == city: return true               // already there
    DeleteCity(&occ)                           // the previous occupant's file is deleted
path := city.GetFilePath();  name := city.GetCityName();  serial := city.GetSerialNumber()
candidate := serial; fresh := SC4CreateCitySerialNumber()
loop: if any city in this region's cell map has serial == candidate:
          candidate := fresh;  fresh := fresh - 1;  restart the scan
if R != null: R.Shutdown()
this.Shutdown()
ok := true
target := SaveFileFor(name, candidate)         // §7.4, in the ACTIVE region's folder
if not Rename(path, target): ok := false
else:
    clear the read-only attribute of target
    read target as a regional city rc; if that fails: ok := false
    else: rc.SetSerialNumber(candidate); rc.SetPosition(x, z, true); write rc back into target (SC4WriteRegionalCity); rc.Shutdown()
this.Init()
return ok
```

Consequences *[inferred]*:

- The destination is the **active** region's folder (the file-name helper of §7.4 uses the nation's active region),
  so `this` is expected to be the active region.
- A city that is already in this region (`region == this`, or the occupant scan finds the city itself) always finds
  its **own** serial in the cell map and so is given a new serial number; if its file name then collides with its old
  file, the helper of §7.4 picks the next ` (n)` name. A city coming from another region keeps its serial unless
  another city of this region has it.
- The source region `R` is shut down (its `region.ini` is written and its cities dropped) but **not initialised
  again**. The destination region is shut down and initialised again, so it reloads everything from disk.
- Only the regional-city record of the moved file is updated (serial and position); the rest of the city file is
  untouched.
- The branch of the original that would handle `R == this` separately is unreachable because `R` is nulled first.
- Any failure leaves `ok` false, but the shut-down regions stay shut down (this region is re-initialised in every
  case).

### 7.4 The file name of a city: `SaveFileFor(name, serial)` (`0x005DC790`, helper `0x005DC400`) *[compat]*

Used by `MoveCity` (active region) and `MoveOverflowCitiesToDownloadDirectory` (the Downloads folder). The helper
`0x005DC400(directory, name, serial, out)`:

1. `base := directory + SC4SanitizeFileName("City - " + name + ".sc4")`: the prefix `City - ` and the extension
   `.sc4` are fixed, the sanitiser (§6.4) acts on the whole name.
2. `n := 1`. Loop: if no file exists at the candidate path, return it. If `serial != 0` and the existing file reads
   as a regional city (§5.1) and its serial is `serial` **or 0**, return the same path (it is this city's file; it
   will be overwritten). Otherwise `n := n + 1` and the candidate becomes `base` with ` (n)` inserted before the
   last `.`.

The wrapper `0x005DC790(name, serial, out)` first sets `directory` to the nation's region directory joined with
the **active region's** directory name. Shipped files follow the rule: `City - New City (10).sc4`.

### 7.5 `MoveOverflowCitiesToDownloadDirectory` (`0x004A51C0`, run by `Init`) *[compat]*

While the overflow list is not empty, take the first city and remove it from the list. Then: `city.Shutdown()`;
`source := city.GetFilePath()`; `target := SaveFileFor(nation.GetRegionDownloadDirectory(), cityName, serial)`
(§7.4, with the Downloads folder as the directory); copy `source` to `target` (overwriting); **only if the copy
succeeded**, delete `source`. A failed copy leaves the file in the region folder (it will be an overflow city again
at the next `Init`).

So a shipped or user file that does not fit the picture disappears from its region the first time the region is
loaded, and shows up in `Regions\Downloads\`. The Downloads folder is not read as a region (§2.1), and nothing in
this class reads the files back.

### 7.6 `ResetTutorialCity(tutorialId)` (slot 22, `0x004A5400`) *[compat]*

Rebuilds a tutorial city from the pristine copy in the game data.

1. `tutorialId == -1`: return false.
2. `cities :=` `GetAllCities`. Find the first whose `GetTutorialID()` equals `tutorialId`. If none: skip to the
   result (it is false).
3. `path :=` its file path. Export the record of type `0x6A5B7BF5`, group `0x6A5B7B57`, instance `tutorialId` to
   `path` (the same export as §6.4 step 5).
4. If the export succeeded: `(x, z) :=` its position; if `GetCity(x, z)` finds the box, `ReloadCity(&box)` and set
   the reloaded city's tutorial id to `tutorialId`. Then, whether or not a city was found at `(x, z)`, mark the
   tutorial **active** again in the application's tutorial registry (`cSC4TutorialRegistry::SetActive(id, true)`,
   tutorial.md).
5. The result is `true` exactly when the export in step 3 **failed** (it is false when no city matched or when the
   reset worked) *[inferred]*: the polarity is read from the bytes and looks inverted (RM8). The caller in the
   nation (`ResetTutorialCities`) is outside this spec.

## 8. Terrain type, base height and water

### 8.1 Base terrain type and height

`GetBaseTerrainType` returns the stored integer (initially 0). `SetBaseTerrainType(t)` stores any integer.
`LoadConfig` accepts only 0 and 1 (§3.1), so the game only ever has these two kinds *[inferred]*.
`GetBaseTerrainHeight()` returns the entry `t` of a **two-entry table of unsigned 32-bit values** that is
**process-wide** and is filled by the region screen when it reads its tunable values
(`cSC4WinRegionScreen::ReadTunableValues`, `0x007ABF10`): the two entries are the unsigned bytes of the properties
`0xCA383CAD` (entry 0) and `0xCA383CAE` (entry 1) of the exemplar `0x6534284A / 0x690F693F / 0xAA383BFE` (the
"region UI tuning" exemplar, INVENTORY-M6.md §6).

Until that read has happened the table is all zeros, so `GetBaseTerrainHeight()` returns 0 *[inferred]*. There is no
range check: a type outside 0..1 reads whatever follows the table in memory (the next dword is the byte that `Init`
sets, §6.1) *[quirk]*. The unit and the use of the height are not known to this spec (RM9).

### 8.2 Water preferences

`GetWaterPrefs(&min, &max)` stores the **low byte** of the water minimum and the water maximum (§3.1) in the two
bytes it is given. The defaults are 60 and 100. What reads them is not covered here (RM9).

## 9. `cSC4RegionalTerrainMap` (constructor `0x007AACE0`)

A tiny object that the region screen builds and publishes as the process-wide terrain pointer (`0x00B43CF8`) while
the region screen is up. It stands in for the city terrain: a **perfectly flat** terrain over a rectangle of cells,
for the regional view to draw and query.

### 9.1 Construction and identity *[compat]*

`cSC4RegionalTerrainMap(widthCells, heightCells)` (`0x007AACE0`) is built by `cSC4WinRegionScreen::InitApplicationServices`
(`0x007ACC90`) as follows: take the active region's `GetBoundingRect` (§5.6), `dx := maxX - minX + 1`,
`dz := maxY - minY + 1`, then `widthCells := (min(dx, 32) * 64) / 4` and `heightCells := (min(dz, 32) * 64) / 4`:
**16 terrain cells per region cell**, at most 32 region cells per side (512 terrain cells).
(With the quirk of §5.6, `dx` and `dz` are one more than the corner spread; a region whose rectangle is all zeros
gives a 1 x 1 region-cell map, 16 x 16 cells.)

The object has the fields `base height` (`+0x0C`, the float `270.0`), `+0x10` (0), `width` (`+0x14`), `height`
(`+0x18`), `vertex stride` (`+0x1C`, `width + 1`), `cell size` (`+0x20`, the float `64.0` metres) and a byte `+0x24`
(0) that slots 3 and 4 set and clear (an enabled flag; `InitApplicationServices` calls slot 3 right after it stores the
object, so the flag is 1 while the region screen is up) *[inferred]*.

`QueryInterface` answers `iid = 1` and `iid = 0xAB953253` with `*out := this` and an `AddRef`; any other id returns
false. The interface behind `0xAB953253` is a terrain-map interface whose slot meanings are not in the SDK (RM10).

### 9.2 Behaviour *[inferred]* (read from the bodies of the vtable `0x00AB8C00`, 40 slots; none is called outside the region screen and the view)

| Slots | Behaviour |
|---|---|
| 3, 4 | set / clear the byte at `+0x24`; return true |
| 5 | return the byte at `+0x24` (the shared reader `0x007D6680`) |
| 6 | return the cell size (64.0) |
| 7, 8 | return the width and the height in cells |
| 9 | vertex index of cell `(x, y)`: `y * (width + 1) + x` |
| 10 | inverse of 9: `x := index mod stride`, `y := index / stride` |
| 11 | true when `x >= 0`, `z >= 0`, `x / cellSize < width` and `z / cellSize < height` (world metres) |
| 12, 13, 14, 18, 19, 21, 22 | the height at any point (and the unqualified height): the base height 270.0 |
| 15, 16 | fill a rectangle of a float buffer with the base height / with 0 |
| 17 | all four values of a four-float output set to the base height |
| 20, 23, 29, 30, 31, 32, 33 | return 0.0 (slot 29 returns the float at `+0x10`, which is 0) |
| 24 | return 1023.0 |
| 25 | world position `(x, z)` to a vertex index: `stride * floor(z / cellSize) + floor(x / cellSize)`, with the cell indices computed by a rounding trick that gives the floor and is exact at multiples of the cell size |
| 26, 27, 28 | the surface normal: (0, 1, 0) |

The terrain has no slope, no water and no varying height; the base height of the map (270 m) is a constant of the
class and is **not** the table of §8.1. What the rest of the 40 slots do (30..39 point into shared code) is not
decoded (RM10).

## 10. `cSC4RegionalTerrainView` (constructor `0x007AC7D0`)

The 3D view layer that draws the terrain map. Its `Update(drawContext)` (`0x007AB630`) is the only decoded method.

### 10.1 Construction

The constructor sets the packed render state that `Update` uses (`cSC4PackedRenderState::PackRenderState` with the
arguments `1, 1, 1, 0, 1, 0, 0, 7, 0, 1, 0xFFFFFFFF, -1.0f, 0xFFFFFFFF, 0xFFFFFFFF`; their meanings are not decoded,
RM11).

### 10.2 `Update` *[inferred]*

1. Apply the render state to the draw context.
2. Read from the process-wide terrain (§9): the base height `h` (slot 21), the width `W` and height `H` in cells and
   the cell size `s`.
3. Build **four vertices** forming the axis-aligned rectangle `(0, h, 0) .. (W * s, h, H * s)` in the ground plane,
   each with the colour `0xFF400000` (alpha 255, red 0x40, green 0, blue 0 when read as ARGB) and no texture
   coordinate set.
4. Draw them as one primitive of type 6 (a triangle fan) with 4 vertices. Return true.

It is a flat, single-colour floor. It does not read the heights per cell and does not depend on the region's cities.

## 11. How the pieces are driven (outside the class)

The nation (nation.md §4) owns the list of regions. It sets the name and directory name and calls `LoadConfig` for
each folder (§2.1), `Init` on the region that becomes active and `Shutdown` on the region that stops being active
(`SetActiveRegion`, `0x004972F0`), `CreateRegion` for a new region (`AddNewRegion`), and `Delete` for a deleted one
(`DeleteRegion`). The region screen (`cSC4WinRegionScreen`) builds the terrain map and the view while it is up
(§9.1). The neighbours simulator uses `GetCity` and `ReloadCity` (neighbors.md §9, §14.1).

## 12. Conformance tests

1. **Pictures.** Parsing the shipped `config.bmp` of each region by §4.2 gives 85 locations for Berlin (52 medium, 32
   small, 1 large), 49 for Fairview (5 medium, 44 small), 73 for London (56 medium, 16 small, 1 large), 49 for New
   York (44 medium, 5 large), 69 for San Francisco (37 medium, 29 small, 3 large) and 91 for Timbuktu (55 medium, 36
   small). Every shipped city file of Berlin, Fairview, London, New York and San Francisco sits exactly on one
   location, one to one.
2. **A duplicate cell.** Timbuktu has 92 city files for 91 locations: `City - Making Money Tutorial.sc4` and
   `City - New City (21).sc4` both have position (6, 5) and size 2. With a directory listing in alphabetical order
   the first is placed and the second is overflow: `Init` moves it to `Regions\Downloads\` and deletes it from the
   Timbuktu folder *[inferred]* (RM12 asks whether the game really does this on a fresh install).
3. **Bounding rectangle.** Locations (0,0,1), (2,0,0) give `{0, 0, 2, 0}` reset to `{0, 0, 0, 0}` because
   `maxY - minY` is 0; locations (0,0,1), (3,2,0) give `{0, 0, 3, 2}`.
4. **`LoadConfig`.** A `region.ini` with `Terrain type = 2`, `Water Min = 255`, `Water Max = 254`, `Name = ` leaves
   the terrain type and water minimum and name unchanged and sets the water maximum to 254. `Name = #0` and
   `Name = #zz` give the literal names `#0` and `#zz`. `Name = #0xcab6d26e` gives the text of LTEXT
   `0x6A231EAA / 0xCAB6D26E`.
5. **`CreateRegion`.** With `Regions\Berlin` present, `CreateRegion("Berlin")` creates `Regions\Berlin (2)` and the
   region's name is `Berlin`; `CreateRegion("a:b?")` creates `Regions\ab`; the folder has a `config.bmp` that is
   one of the template records.
6. **`Init` result.** An empty folder (no city files) with the default picture returns false from `Init`; one valid
   city returns true.
7. **File naming.** A city named `New City` with serial `S` in a folder that has `City - New City.sc4` of serial
   `T != S, T != 0` is given `City - New City (2).sc4`; if its serial is `T` or the existing file's is 0 it reuses
   `City - New City.sc4`.
8. **Terrain map.** For a region whose rectangle is `{0,0,3,5}`: dx = 4, dz = 6, the map is 64 x 96 cells, the
   vertex index of cell (2, 3) is 3 * 65 + 2 = 197, and every height is 270.0.

## 13. Open

Questions RM1 to RM12 are in [questions/region-model.md](questions/region-model.md). Not covered here:
the region screen and the region view windows (`cSC4WinRegion*`), the new-region and import dialogs, city
obliteration, the nation's list handling beyond §11, and the sub-view classes that share the `cSC4Region*` row of
`tools/coverage-map.tsv` (`cSC4RegionSignpostAnimatorWinProc`).


## Complementary region analysis

See [region.md](region.md) for the other contributor's independent region analysis. Both analyses and their open RM/RQ questions are retained. Coverage credits exact cSC4Region once; this cross-reference does not promote partial terrain-map/view or location/signpost helper owners. Inferred naming/argument and filesystem observations retain their tags until their questions are independently resolved.
