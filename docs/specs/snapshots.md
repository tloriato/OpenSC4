# Spec: snapshots (`cSC4WinSnapshotTaker`) and the snapshot album (`cSC4WinSnapshotAlbum`)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`). Implementers must work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult
decompiled or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[summary]* is a correct outline whose details are not given.

Related specs: [application.md](application.md) §9 (the `Albums` user directory), [paths.md](paths.md) (file
names), [region-screen.md](region-screen.md) (the command that opens the album),
[ui-windows.md](ui-windows.md) (scripted windows and the window classes).

## 0. Summary

The snapshot taker is a frame laid over the 3D view. The player sizes it, then takes a picture of the view inside
it. The picture is saved as a PNG in the city's **album**, a folder under the user's `Albums` directory, with a
small obfuscated information file and an optional description. The album window browses the albums and their
pictures, edits descriptions, and creates and deletes albums.

## 1. The album store *[compat]*

### 1.1 Folders

- The album root is the application's `Albums` directory (application.md §9).
- An album is the folder `<name>--<n>\` (format `%s--%u`), where `<name>` is the city name made file-safe
  (`SC4SanitizeFileName`) and cut to at most 48 bytes on a character boundary (`CityNameToAlbumName`, `0x005F8970`),
  and `<n>` a number that tells apart cities of the same name *[inferred: the region's city index]*. `AlbumExists`
  (`0x007B7990`) tests the folder; `CreateAlbum` (`0x007BB320`) creates it with an empty `album.ini` and refreshes
  the album list.
- `update_list_of_album_directories` (`0x007BAEA0`) lists the root's folders, sorted, keeping those that hold
  album entries (`album_directory_has_album_entries`, `0x007B8FB0`: an `album.ini` or any `*.png`).

### 1.2 Pictures and their files

Each picture `X.png` may have two companions with the same stem:

- `X.info` (`get_path_for_info_file`, `0x007B9140`): the picture's information, obfuscated by XOR of every byte
  with `0x7B` (`ObfuscateData`, `0x007B7F20`, used both ways);
- `X.txt` (`get_path_for_description_file`, `0x007B9280`): the player's description, plain text; saving an empty
  description removes the file.

Recognised image extensions (constructor `0x007B8C70`): `.png`, `.jpg`, `.jpeg`, `.jp2`, `.gif`, `.bmp`.

**New file names** (`GetNewTempFileName`, `0x007B7F90`): the city name (file-safe, cut to 48 bytes), a hyphen, the
in-game date formatted by the language manager, made file-valid, and then a free name in the album folder with the
extension `.png` (`cRZFile::MakeTempFileName`). Without a city the name comes from the region *[summary]*.

### 1.3 The information file

`GetGraphicInfoForCurrentCity` (`0x007B8330`) writes ten text fields, each ended by a line feed, in this order
*[as read]*: the marker `SanityCheck`; the user name (`cRZVersion::GetUserName`); the city name; the mayor name;
the simulation day (`%d`); the date as text; the population (`%d`); the funds (`%I64d`); a further `%d` value; and
the view as `zoom,rotation,x,y` (`%u,%u,%u,%u`, the last two taken at the centre of the 3D view).

`GetInformationOfStandardFile` (`0x007B93C0`) reads the file back, de-obfuscates it and splits it at line feeds.
When the marker field matches `SanityCheck` it builds the caption shown under the picture from five language
strings (`0x6A231EAA` / `0xEA946EFE` to `0xEA946EFA`): the city, the mayor, the date, the population and the funds
(shown with the money format) *[summary of the exact layout]*; otherwise it shows no information.

## 2. The snapshot taker *[compat]*

`CreateWinSnapshotTaker` (`0x007B7530`) shows the existing frame (window `0x85202C0E`) or creates one (`0x10C`
bytes) in the given parent, optionally at a size index. `Init` (`0x007B7A80`) sets the font (`Arial MT`) and the
help text from three language strings (`0x2A56DE25`, `0xCA56DE46`, `0x0A56DE50`) *[summary]*.

- **Sizes.** The frame has a list of (width, height) sizes; `GoToSizeIndex` (`0x007B6D90`) clamps the index to the
  list and resizes the frame, keeping it inside the view (`AdjustPosition`, `0x007B6680`).
- **Keys** (`GZOnKeyDown`, `0x007B6FF0`): Space selects the next size (wrapping); Esc closes the frame; a digit
  0–9 sets a percentage 10, 20, … 100 (`+0xE8`) *[inferred: the frame's tint]*; Enter, when the frame's slot 71
  test passes *[inferred: it is shown]*, calls its slot 70 and posts itself message `0x1A5D056D` (data "close
  afterwards" = 1 unless Shift, Ctrl or Alt is held), and raises Lua event `0xAA8B82A9`.
- **Mouse**: the left button grabs the frame and moving drags it (`GZOnMouseMove`) *[inferred]*; the right button
  closes it.
- **Capture** (`DoMessage`, `0x007BE0E0`): message `0x1A5D056D` re-posts itself until its counter reaches 3, so the
  capture happens three frames later, once the frame is gone from the picture. Then (`0x007BDE90`): the 3D view's
  picture of the frame's rectangle is taken (the whole view when the rectangle has the view's size and is offset
  by at most 5 pixels), its alpha channel is forced opaque for the 16- and 32-bit formats (`TakeSnapshot`,
  `0x007B67A0`), the album window opens at the current city's album (§3.3), the picture is written
  (`cWinBMPView::WriteBMPToFile`) to a new `.png` name of that album and added with its information
  (`AddNewGraphicForCurrentCity`, `0x007BD9A0`); a failed write removes the file. The album runs modally;
  afterwards the frame closes or comes back.

## 3. The album window *[compat]*

### 3.1 Creation

`CreateSnapshotAlbumWindow` (`0x007BCE40`) builds the window (`0x0A8CD3EE`) from its UI script;
`ShowSnapshotAlbum` (`0x007BDD60`) creates it and goes to the current city's album; `DoModalWin` (`0x007B69E0`)
pauses the game while it runs (`cSC4AutoGamePause`). `on_init` (`0x007BC350`) binds the controls (picture view,
album list, next and previous, description, count text) *[summary]*; `on_shutdown` (`0x007BAD30`) saves and clears.

### 3.2 Browsing

- `RefreshGraphicList` (`0x007BBD70`) lists the album folder's pictures; `SetInitialFile` (`0x007BC120`),
  `SetCurrentFile` (`0x007BA9D0`) and `GoToGraphic` (`0x007BA780`) show a picture with its information and
  description; the count text is `(i/n)` (`set_image_count_text`, `0x007B7230`); with no picture the view shows the
  "no image" state (`set_to_no_image_file`, `0x007B72D0`).
- `GoToNextGraphic` and `GoToPreviousGraphic` (`0x007BC910`, `0x007BCA60`) save the current description first.
- `DeleteCurrentGraphic` (`0x007BCBB0`) removes the picture and its `.info` and `.txt`, then moves on.
- `DeleteAlbum` (`0x007BB5C0`) removes every file of the album folder and the folder *[summary]*.
- `on_button_activated` (`0x007BCFC0`) and `DoWinMsg` (`0x007BDAE0`) route the buttons: zoom in and out
  (`cWinBMPViewGVExtra::SetSizeInOut`), previous, next, save description, close (`end_dialog`, `0x007B6510`),
  album selection (`GoToAlbum`).

### 3.3 Albums

- `GoToAlbum(name, n, create)` (`0x007BD200`) makes the album name, creates the album when asked and it does not
  exist, and selects it in the list (`convert_album_directory_name_to_album_index`, `0x007B6CF0`; −1 when it is not
  listed); `GoToAlbum(index)` (`0x007BC820`) shows the album at a list index.
- `GoToAlbumForCurrentCity` (`0x007BDC90`) does the same for the current city.
- `AddNewGraphic(file, info, description, city, n, move)` (`0x007BD3D0`): makes sure the album exists (creating
  it); a file already in the album keeps its name, any other is copied in under a new name (and removed after the
  copy when `move` is set); the information and the description are saved beside it; the album and the picture are
  then shown.

## 4. Quirks

1. The info file's "obfuscation" is a single-byte XOR with `0x7B`.
2. City names are cut to 48 bytes for album and file names, so two long names that share their first 48 bytes
   share an album.

## 5. Open questions

See [questions/snapshots.md](questions/snapshots.md) (SN1–SN2).

## 6. Conformance tests

1. The city `Big Town`, number 2, has the album folder `Big Town--2\` with an `album.ini`.
2. Writing the info string `SanityCheck\n…` produces a file whose first byte is `'S' XOR 0x7B` = `0x28`.
3. Saving an empty description of `X.png` removes `X.txt`.
4. Space on the last size selects the first; the digit 5 sets 60.
5. A capture message posted with counter 0 is handled on the fourth delivery.
