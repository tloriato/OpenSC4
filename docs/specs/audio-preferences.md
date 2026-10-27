# `cSC4AudioPreferencesWinProc`: the audio options dialog (behaviour spec)

Scope: the whole `cSC4AudioPreferencesWinProc` owner as tools/coverage.py assigns it (26 functions, 11,812 bytes on
`5c0ff782`; inventory in §10). It also describes the shared song-table loader `0x004F2910` (owner `<shared>`), which
only this dialog's code calls. The radio that receives the dialog's messages is audio.md §7.2.1; the preference block
it edits is application.md §13.

Evidence: static RE only, on the instruction and data bytes of the Steam 1.1.641 exe (the same code as GOG,
`docs/lanes/re-dates.md`). Every function was disassembled; the shipped tuning and song tables were read from
`SimCity_1.dat` with `tools/dbpf.py`. Written 2026-10-23 (re-audioprefs7).

Conventions:
- `P` is the preferences object (`cSC4Preferences`, the global `0x00B43CD8`). The audio fields are the record
  `R = P + 4`, so `R + x` is `P + 4 + x`.
- `W` is the dialog object. `win` is the dialog's root window (`cIGZWin`), from `W+4` slot 3 (`AsIGZWin`).
- A "child" is `win` slot 37 (`GetChildAsRecursive(id, iid, &out)`, `+0x94`). The checkbox and button children are
  asked for IID `0x8810`, the sliders for `0x21325207` and the song grid for `0xDAA6B9BE` (`cIGZWinGrid`,
  win-grid.md). Any other window interface is named only by its slot.
- "Post `t (a, b)`" means `_PostEventMessage` (`0x004F20E0`): a new standard message, `SetType(t)`, `SetData1(a)`,
  `SetData2(b)`, posted through the message server `0x00B43CCC` slot 4 with priority 0, then released.

## 1. Identity and lifetime *[compat]*

- Class and interface ID `0x22E85D8E`. `QueryInterface` (`0x004F1F30`) answers `1` and `0x22E85D8E` with the
  object (AddRef'd, true); anything else is false.
- Size 0x80 bytes (`GlobalHeapAlloc`). The constructor `0x004F2450` sets the vtable, clears `+4`, `+8`, `+0x0C`,
  `+0x10` and the count `+0x7C`, and fills the copy `+0x14` with the record defaults (the record constructor
  `0x00449010`, §2.2).
- vtable `0x00A94264`:

| Slot | Address | Method |
|---|---|---|
| 0 | `0x004F1F30` | `QueryInterface` |
| 1 | `0x004F1F60` | `AddRef`: `+0x7C` + 1, returns the new count |
| 2 | `0x004F26D0` | `Release` (below) |
| 3 | `0x004F4280` | `DoWinProcMessage` (§7.1) |
| 4 | `0x0041D4C0` | `cIGZWinProc::DoWinMsg` (shared) |
| 5 | `0x004F5120` | `ShowDialog` (§6.2) |
| 6 | `0x004F2210` | `Shutdown`: `+0x10` := 0; the grid `+8` is released and cleared; true |

- `Release`: a count above 1 is decremented and returned. Otherwise:
  1. `Shutdown` (slot 6) runs and the vtable is set again.
  2. `+8` and `+4` are released when set; neither is cleared.
  3. The object is freed (`GlobalHeapFree`) and 0 returned.
- Callers: the region screen's button `0xA98F4F88` (`0x007B0205`; region-screen.md §5) and the city view's command
  `0x6A935E34` `OpenAudioOptionsDialog` (`0x007F2CCB`; view3d-window.md §6g). Both:
  1. allocate and construct the object, holding one reference;
  2. call `ShowDialog` (slot 5), then `DoModalWin` (§6.4), **without testing `ShowDialog`'s result**;
  3. release their reference.

Object layout:

| Offset | Field |
|---|---|
| `+0x04` | the dialog window (`cIGZWinGen`, IID `0x5386D516`), counted |
| `+0x08` | the song grid (`cIGZWinGrid`), counted |
| `+0x0C` | byte: "a song was previewed" (§7.2, `0x8A550C57`/`0x8A550C58`) |
| `+0x10` | `P`, set by `ShowDialog`; cleared by `Shutdown` |
| `+0x14` | a copy of `R` taken by `InitDialog`, restored by Cancel (§2.2 lists the copied fields) |
| `+0x7C` | reference count |

## 2. Shared state *[compat]*

### 2.1 Statics

| Address | Meaning |
|---|---|
| `0x00B21A38` | byte: the dialog is open (set by `InitDialog`, cleared by `EndDialog` and by the first list load) |
| `0x00B21A39` | byte: the song lists are loaded (§3.1) |
| `0x00B21A3C`, `0x00B09884` | music volume floor and ceiling (§4.1) |
| `0x00B21A40`, `0x00B09888` | sound-effect volume floor and ceiling |
| `0x00B0988C` | the radio's station fade time, default 4000 (audio.md §7.2.1) |
| `0x00B0987C`, `0x00B09880` | the first song control ID of list 0 (`0x2A5BA8A7`) and list 1 (`0x3A5BA8A7`) |
| `0x00B09878` | the music file filter `"*.mp3,*.wav,*.xa,*.m3u,*.pls,*.lnk"` |
| `0x00B21A4C`, `0x00B21A60`, `0x00B21A74` | the strings `Radio\Stations\`, `Region\Music\` and `Mayor\Music\` (static constructors `0x00A72160`, `0x00A722A0`, `0x00A723E0`) |
| `0x00B21A88 + 0x30k` | the song list record of list `k` (k = 0 Region, 1 Mayor) |

A **song list record** (0x30 bytes):

| Offset | Field | k = 0 (Region) | k = 1 (Mayor) |
|---|---|---|---|
| `+0x00` | `k` | 0 | 1 |
| `+0x04` | a word set by `StaticInit`, never read by this owner | `0xAA5FC8D5` | `0x2A5FC8DD` |
| `+0x08` | vector of song entries (begin, end, capacity) | | |
| `+0x14` | instance of the song-table exemplar | `0x2A5E5854` | `0x2A5E5855` |
| `+0x18` | the list of excluded file names (a `cRZString` list) | | |
| `+0x1C` | the folder (a `cRZString`; its text at `+0x20`) | `…Radio\Stations\Region\Music\` | `…Radio\Stations\Mayor\Music\` |

A **song entry** (16 bytes): `+0` its checkbox's control ID, `+4` the string ID of its file name, `+8` the string ID
of its title (both in group `0x8A635D24`), `+0x0C` byte its default "enabled" flag.

### 2.2 The audio record `R` (`P + 4`)

The fields the dialog uses and their defaults (`0x00449010`):

| `R +` | `P +` | Field | Default |
|---|---|---|---|
| `0x00` | `0x04` | byte: sound effects on | 1 |
| `0x04` | `0x08` | sound-effect volume | `0x400` |
| `0x08` | `0x0C` | music volume | `0x320` |
| `0x0C` | `0x10` | music on | 1 |
| `0x28` | `0x2C` | 30 bytes: the Mayor songs' "enabled" flags | 1 each |
| `0x46` | `0x4A` | 30 bytes: the Region songs' "enabled" flags | 1 each |
| `0x64` | `0x68` | byte: the custom-tunes mode (§3.7) | 0 |

The constructor also sets `R+0x10` := 0, `R+0x14`…`R+0x20` := 1, `R+0x24` := 0 and `R+0x65`, `R+0x66` := 1; the
dialog does not use them. `InitDialog` copies the fields of the table into `W+0x14` at the same offsets, and Cancel
writes them back (§7.3, `UpdatePreferences`).

## 3. Song lists *[compat]*

### 3.1 Loading once

Every entry point that needs the lists first runs this guard (inline at each site): when `0x00B21A39` is clear, set
it, clear `0x00B21A38`, then `StaticInit`, `LoadSongTable(0)` and `LoadSongTable(1)`. Sites: `ShowDialog`,
`UpdateExclusionList`, and `HandleControlMessage` for `0x8A550C50` and OK. The audio event handler's start-up calls
the same sequence through `0x004F2D40` (audio.md §7.1).

### 3.2 `StaticInit` (`0x004F2B70`)

`dir` = the application's plugin-directory base (application slot 53 with index −1, application.md §9). For
k = 1 and then k = 0, it sets the record's `+0`, `+4` and `+0x14` words (§2.1) and its folder to
`dir + "Radio\Stations\" + sub`, with sub `Mayor\Music\` (k = 1) or `Region\Music\` (k = 0).

### 3.3 `LoadSongTable(k)` (`0x004F2910`, owner `<shared>`)

Nothing for k outside 0–1. Otherwise:
1. Load the exemplar (`0x6534284A`, `0xCA25875D`, record `+0x14`) through the resource manager (`0x00B43CA8` slot 3,
   IID `0xA52160F5`); without it, return.
2. For `i` = 0 … 29, read the property `0x8A5E7B50 + i`. A property with at least two values `(t, f[, e])` appends
   the entry {ID `base_k + i`, file `f`, title `t`, enabled `e`}. The flag:
   - is the third value when there is one;
   - else the previous entry's flag, starting at 1 *[quirk: the value is not reset per property]*.
3. Release the exemplar.

Shipped (`SimCity_1.dat`):
- "Region Mode Audio List" (`0x2A5E5854`) has 10 songs (`0x8A5E7B50`…`59`); only the sixth (`…55`) is off by
  default.
- "Mayor Mode Audio List" (`0x2A5E5855`) has 25 songs (`…50`…`…68`), all on.

### 3.4 `SetDefaultMusicListPreferences` (`0x004F2580`)

Copies the entries' default flags into `P`: Mayor entry `j` → `P+0x2C+j`, Region entry `j` → `P+0x4A+j`, for at most
30 entries each. Called by the audio event handler for first-run preferences (audio.md §7.1).

### 3.5 `AudioFileExists(k, fileId)` (`0x004F2A70`)

False for k outside 0–1. Otherwise true when the file `folder_k + text(0x8A635D24, fileId)` exists
(`EffectsBootstrap::FileExists`).

### 3.6 Custom tunes: `HasCustomTunes(k)` (`0x004F31C0`, exported as `InsertMusicList`)

`k` arrives in ECX (thiscall with `this` = k).
1. False for k outside 0–1.
2. Otherwise build the lower-case file names of list k's entries.
3. Scan the folder (`0x004F27A0`, exported as `_ShouldEnableCustomTunesCheckbox`):
   - for each comma-separated pattern of the filter, `cRZDirectory` reads the folder's matching file entries into
     one list;
   - the names are then made lower case.
4. True when a scanned name is not one of the entries' names; the search stops at the first such name.

### 3.7 `IsSongEnabled(path)` (`0x004F2D70`, RET 4)

The radio's current file path against the lists:
1. For each Region entry: when `path` equals `dir + "Radio\Stations\Region\Music\" + file` without regard to case
   (`cIGZString` slot 10 with `false`), the result is the entry's flag `P+0x4A+j` ≠ 0 and the path is found. The walk
   does not stop, so a later match wins.
2. Only when not found, the Mayor entries are walked the same way against `…\Mayor\Music\` with the flags `P+0x2C+j`.
3. Not found in either list: true.

### 3.8 `UpdateExclusionList(k)` (`0x004F3410`, cdecl)

1. Load the lists (§3.1). `flags` = `P+0x4A` for k = 0 or `P+0x2C` for k = 1; `m` = the mode byte `P+0x68`.
2. Clear the record's exclusion list `+0x18`. Every entry `j` whose flag differs from `m` (`flags[j] ≠ m`) has its
   file name (text of its file string) appended.
3. Post a standard message of type `0x0A611B34` with:
   - data1 = 1 for k = 0 or 0 for k = 1, so the radio sees `krgn` / `kmay` (audio.md §7.2.1);
   - data2 = the list's address;
   - data3 = `m`.

So with the mode byte 0 (default) the list holds the **enabled** songs, and with 1 the **disabled** ones. The radio
reads the list as it is (audio.md §7.2.1).

`HandleControlMessage` runs the same body inline, for k = 0 and then k = 1, on `0x8A550C50` and on OK.

### 3.9 Discovery versus persistence (AU43)

Sections 1–3 and 6–7 are the complete construction/song/control contract requested by
audio.md question AU43. Discovery is not a directory-generated song grid: the grid's rows
come only from the two exemplar tables. Directory scanning in §3.6 merely decides whether
the custom-tunes checkbox is enabled. File-name comparison for that scan is case-folded;
the patterns include playlists and shortcuts as well as audio files. Missing shipped files
disable their individual buttons but do not remove entries or rewrite preference flags.
`IsSongEnabled` checks full paths for shipped entries and defaults to true for an unmatched
path, even when custom mode is 0; it does not consult directory presence or the exclusion
list. That helper is separate from the radio's interpretation of mode/list messages.

The load-once byte is set **before** StaticInit and the exemplar reads. An absent song-table
resource leaves that table empty; it does not reset the byte for a retry on the next open
(`0x004F2910`–`0x004F29AB`, `0x004F5120`). Construction of a dialog neither resets this
process-wide guard nor reloads the tables. `SetDefaultMusicListPreferences` copies exemplar
default flags, capped at 30 per bank; it is not called to reset preferences on every open.

Persistent song state is the **two 30-byte flag arrays and mode byte in P** (§2.2). The
application's SavePreferences writes the live preferences block to `SimCity 4.cfg` as
documented in [application.md](application.md) §13.1 (record version `0x15`). This owner
does not serialize file-name strings, exclusion lists, directory results or the preview flag.
`UpdateExclusionList` clears/repopulates the process-static list and posts its address as a
raw Pointer2 (`+0x44` setter), not a reference-counted payload or a persisted copy. The list
object survives dialog destruction; its contents may change on a later rebuild. Its names
are the file strings from the exemplar entries, in entry order, without folder prefixes;
mode is a byte and the inclusion test compares bytes **literally**, not normalized booleans
(`0x004F3410`–`0x004F35EC`). For normal flags/mode 0 or 1 this yields §3.8's polarity.
Neither individual song toggles nor a custom-mode toggle posts a rebuild immediately.
Only music-enable handling, OK, and explicit UpdateExclusionList calls do so here.

## 4. Volumes *[compat]*

### 4.1 `ReadTunerVariables` (`0x004F22C0`)

The exemplar (`0x6534284A`, `0xCA25875D`, `0x6A359C5D`, "Audio UI Settings") is loaded through the resource manager.
When present, its properties overwrite the statics:

| Property | Static | Shipped |
|---|---|---|
| `0x4A7CB3BA` | music floor `0x00B21A3C` | 450 |
| `0x4A7CB3BB` | sound-effect floor `0x00B21A40` | 173 |
| `0x4A7CB3BC` | music ceiling `0x00B09884` | 1024 |
| `0x4A7CB3BD` | sound-effect ceiling `0x00B09888` | 1024 |
| `0x4A7CB3BF` | fade time `0x00B0988C` | 4000 |

`0x4A7CB3BE` (shipped 0) is not read. Without the exemplar, the floors stay 0 and the ceilings 1024.

### 4.2 Slider to volume (`GetVolumeFromSliderValue(s, music)`, `0x004F1F90`, cdecl)

`lo, hi` = the music pair when `music` ≠ 0, else the sound-effect pair. `s` = 1000 → `hi`; `s` = 0 → `lo`. Otherwise
`d = trunc(float(hi − lo) × float(s) × (−0.001f))`, computed in x87 extended precision with the binary32 constant
`0xBA83126F`. Both differences are read as unsigned. Then `r = lo − d` (unsigned): `r > hi` → `hi`, `r < lo` → `lo`,
else `r`.

The two slider handlers then store `r` only when it is **above** the floor, else 0 (§7.2).

### 4.3 Volume to slider (in `InitDialog`)

With `v` the copied volume (`W+0x1C` music, `W+0x18` effects) and `lo, hi` its pair:
1. `v > hi` → `v := hi`.
2. Else if `v < lo`: the copy is set to 0 *[quirk: so Cancel restores 0, not the original]* and `v := lo`.
3. Position:
   - 0 when `v ≤ lo`;
   - 1000 when `v ≥ 1000` *[quirk: the raw volume is compared with 1000, not with `hi`]*;
   - else `trunc((v − lo) / (hi − lo) × 1000.0f)` (unsigned to float, x87).

### 4.4 `UpdateSoundSys` (`0x004F2230`)

1. When the sound system (framework service `0x23CECBA0`, IID `0x106077D2`) exists, call its slot 7 with
   `(0x2A5B69C0, v, 0, 0, 0)`, where `v` = `P+8` when the effects byte `P+4` is set, else 0.
2. Post `0xAA650860 (P+0x10, P+0x0C)`: music on and music volume to the radio (audio.md §7.2.1).

**Volume units (APW1).** *[compat]* These are framework volume units: 0 mutes and
1024 is unity. The slider interpolation in §4.2 is linear in those units, with the
floor/mute discontinuity described there; it is not a linear amplitude or loudness
curve. The effects event stores the supplied master level and recomputes active
sounds (`0x004FA550`); the radio passes its level to the sound's volume setter
(`0x008D2FDC`, `0x008D319B`). At the DirectSound backend, a sound volume v is clamped
to 0–1024 and transformed to `trunc(1024 × sqrt(v/1024))` before the renderer master
is applied. Zero maps to −10000, unity to 0, and other resulting levels w to
`trunc(6000 × w/1024) − 6000` in hundredths of dB (`0x00876894`, `0x00875FFF`;
[sound-renderer.md](sound-renderer.md) §4.3). Thus, with renderer master 1024,
volume 256 reaches −3000, not a linear quarter-amplitude setting.

## 5. Saving and restoring *[compat]*

`UpdatePreferences(src)` (`0x004F2520`): copies from `src` (always `W+0x14`) into `R`, then `UpdateSoundSys`:
- `R+0`, `R+4`, `R+0x0C`, `R+8`;
- the mode byte `R+0x64` (from `src+0x64`);
- the 30 Region flags (`src+0x46` → `R+0x46`) and the 30 Mayor flags (`src+0x28` → `R+0x28`).

## 6. Opening and closing *[compat]*

### 6.1 Order

The callers run `ShowDialog` and then `DoModalWin`. The modal loop delivers the window messages of §7. Either the
OK/Cancel handler or `DoModalWin` ends with `EndDialog`.

### 6.2 `ShowDialog` (`0x004F5120`, slot 5)

1. Load the lists (§3.1). When the dialog is already open (`0x00B21A38`), return true at once.
2. `ReadTunerVariables`. Release the old window `+4`.
3. `SC4CreateWindowFromScript((0, 0x96A006B0, 0xCA53F06E), 0, 0xEA53F5DB, 0x5386D516, &W+4)`.
4. On success: `W+0x10` := `P` when still null, then `InitDialog`, then the window's slot 7 `(W)`, which installs the
   window procedure. Return the creation result.

### 6.3 `InitDialog` (`0x004F44C0`)

1. Set `0x00B21A38`; copy `R` into `W+0x14` (§2.2).
2. Disable Play (`0x8A550C57`, §7.4).
3. For each control below, when the child exists: set its state, then make the dialog window its notification
   target (`cIGZWin` slot 86, `SetNotificationTarget`):

| Control | Kind | Initial state |
|---|---|---|
| `0x8A550C57` | button (Play) | disabled |
| `0x8A550C58` | button (Next) | — |
| `0x8A550C5A` | checkbox (custom tunes) | checked = `W+0x78` ≠ 0; window flag 2 (enabled) := `HasCustomTunes(1)` or else `HasCustomTunes(0)` |
| `0x8A550C52` | checkbox (sound effects) | checked = `W+0x14` ≠ 0 |
| `0x8A550C50` | checkbox (music) | checked = `W+0x20` ≠ 0 |
| `0x8A550C51` | slider (music volume) | range 0–1000 (slots 6 and 8), position from `W+0x1C` (§4.3) |
| `0x8A550C53` | slider (effects volume) | range 0–1000, position from `W+0x18` |

   "Checked" is the button's slot 9; a slider's position is its slot 5.
4. **The song grid**: release `+8`, then find the child `0x8A550C56` as a grid into `+8`. Without it, `InitDialog`
   ends here. With it:
   1. Grid set-up:
      - `ClearSelection` and `ClearAllCells`;
      - `ClearFlag(2)`, `ClearFlag(0x8000)`, `SetFlag(0x2000)`, `SetFlag(0x80000)`;
      - `SetSelectionRules(2, 1)`.
   2. Colours:
      - the grid's window fill colour (0xED, 0xF3, 0xF0) (`cIGZWin` slot 107) and window flag `0x20000` set;
      - `SetDefaultFontColor` with (0xED, 0xF3, 0xF0) for 1, (0, 0, 0xFF) for 5 and (0xFF, 0xFF, 0xFF) for 4, the
        colours made by `RZ::GS` slot 34;
      - `SetColumnWidth(0, 1, 255)`.
   3. **Region rows** (when `P+0x4A` is not null): for each Region entry `j` < 30, a new song button
      (`cGZWinBtn`, 0x1B4 bytes) at row `j`:
      - button slots: 12 `(4, 0, 0)`; 47 `(ID)` with `ID = 0x2A5BA8A7 + j`; 9 `(P+0x4A+j ≠ 0)` (checked); 17
        `(the bitmap buffer for UI key (0x856DDBAC, 0x46A006B0, 0x14416244))`; 5
        `(4, 0)`; 45 `(2, 0x8A5C324A)`, its click sound;
      - its window: notification target = the dialog; flag 2 (enabled) := `AudioFileExists(0, file)`;
      - the grid: `SetCellTextValue(0, j, title)` and `SetCellWindow(1, j, button window, 0, 1, 1)`;
      - the button's references are released.
   4. **Mayor rows**: the same for Mayor entries `j` < 30, at row `n0 + j` (`n0` = the number of Region entries),
      with `ID = 0x3A5BA8A7 + j`, flags `P+0x2C+j` and `AudioFileExists(1, file)`.
   5. `ClearSelection`; the grid's window gets the dialog as notification target.

The button states come from `P`, not from the copy; the two are equal at this point.

**Song-button appearance (APW3).** *[compat]* Slot 12 selects bitmap style 4, which
uses eight horizontal state images. With a null bitmap it first records the style
but has no drawable image. Slot 17 supplies the bitmap buffer, reapplies that style
using the whole bitmap, and invalidates the window; each state image is one eighth
of the bitmap width and its full height. This retains the supplied buffer by
reference (`0x009B1179`, `0x009B0338`). Slot 5 with `(4, 0)` clears button flag 4,
which controls caption display; the bitmap remains the button's visible content
(`0x009B14DF`, `0x009B13DD`, `0x009B0C08`, `0x009B1541`). The checked value is set
separately by slot 9. These calls are made for both song lists by `0x004F44C0`.


### 6.4 `DoModalWin` (`0x004F25F0`)

1. A game pause token (`cSC4AutoGamePause(0, 0, 1, 1)`).
2. `S` = the window manager (framework service `0xA417445E`, IID `0x5A4`); `F` = `S` slot 36.
3. `root` = `win`, AddRef'd. Then:
   - `0x00B43CE0` slot 10 `(root, 1)`;
   - `S` slot 41 `(root)`, the modal loop (modal-dialogs.md);
   - `0x00B43CE0` slot 6 `(root)`.
4. When `S` slot 24 `(F)` is true, `S` slot 37 `(F)`.
5. `EndDialog`; release `root` and `S`; end the pause. Return true.

It does not test `W+4` (null crashes) and ignores the modal result.

### 6.5 `EndDialog` (`0x004F2480`)

1. AddRef `W` (slot 1).
2. When a song was previewed (`+0x0C`): post `0xAA650861 (0, 0)`, so the radio's slot 30 gets false (audio.md §7.2.1).
3. When `+4` is set, `mgr` = the root's slot 6. When `mgr` is set:
   - when the root is in its modal loop (`GetFlag(0x1000)`): `mgr` slot 9 `(0xF, root, 1)`, the end-modal message
     (modal-dialogs.md);
   - then `mgr` slot 23 `(root)` (`DestroyWindow`).
4. Clear `+4` before releasing it; clear `0x00B21A38`.
5. Tail-call `Release` (slot 2).

OK and Cancel call it, and `DoModalWin` calls it again after the loop *[quirk]*. The second call finds `+4` null; it
posts `0xAA650861 (0, 0)` again when a song was previewed, since `+0x0C` is never cleared. Each call's AddRef and tail
`Release` balance, so the object lives until the caller's own release.

### 6.6 Construction and close transaction boundaries (AU43)

Constructor `0x004F2450` clears the two counted holders and P pointer as 32-bit fields,
the preview flag as a **byte**, and the reference count as a 32-bit field, and calls
`0x00449010` on `W+0x14` (`0x004F2456`–`0x004F2470`). Thus the initial saved effects/music
volumes are 1024/800, both enable fields are on, the two 30-byte arrays are all 1 and
custom mode is 0; it does not zero the entire 0x80-byte object. InitDialog replaces those
used fields with the live snapshot before setting up controls. Slider initialization can
normalize a below-floor **saved** volume to 0 (§4.3), so Cancel restores that normalized
copy, not necessarily the original raw below-floor value.

The action order, including audio effects, is:

| Close action | Ordered effects |
|---|---|
| Cancel or accepted Escape | Restore effects-enable byte, effects-volume word, music-enable word, music-volume word, mode byte, Region 30 bytes, Mayor 30 bytes; UpdateSoundSys (direct sound-system master-volume event if available, then radio-volume message); PreferencesChanged; EndDialog. No SavePreferences, rebuild or current-song check. |
| OK | Temporarily retain application if present and call slot 17 SavePreferences, ignoring its result, then release it; load lists if needed; rebuild/post Region then Mayor lists; acquire radio if available and read current path; skip once only if the read succeeds and IsSongEnabled is false; PreferencesChanged; EndDialog; release temporary radio reference. |
| Modal loop returns by another route | DoModalWin calls EndDialog; EndDialog itself performs no restore, SavePreferences, exclusion rebuild or PreferencesChanged. |

Restore assignment order is `0x004F2520`–`0x004F2576`; Cancel starts at `0x004F3D07`;
OK is the `0xEA5509E4` branch of `0x004F35F0`. There is no additional Revert/Defaults
control action in this handler; the restore operation is Cancel's UpdatePreferences.
Any other control ID follows §7.2's song-range/no-op rules. EndDialog's preview-end post
precedes window destruction; the modal-loop cleanup calls EndDialog a second time, even
after OK/Cancel. Save failure, missing radio or failed path read does not abort closure.
Cancel performs no disk rollback of a SavePreferences made elsewhere while the dialog
was open. Its radio lists can remain from a live edit even after P has been restored.

## 7. Messages and controls *[compat]*

### 7.1 `DoWinProcMessage(w, m)` (`0x004F4280`, slot 3, RET 8)

1. False unless `w` is visible (`cIGZWin` slot 71).
2. False unless `w` equals the window view returned by the dialog holder `+4`'s slot 3
   (`+0x0C`), or the grid exists and its window (slot 5) is `w` (`0x004F429B`–`0x004F42BF`).
3. By `m[0]`:

| `m[0]` | `m[1]` | Action | Returns |
|---|---|---|---|
| 3 | `0x887113A3` (slider) | `m[2]` = `0x8A550C51` or `0x8A550C53` → handle `(m[2], w, m[3])` | true |
| 3 | `0xE8711255` (list) | handle `(m[2], w, m[3])` | true |
| 3 | `0x287259F6` (button) | `m[2]` ∈ {`0x8A550C57`, `0x8A550C58`, `0xEA5509E3`, `0xEA5509E4`} → handle | true |
| 3 | `0x287259F7` (toggle) | any `m[2]` **except** `0x8A550C52`, `0x8A550C57`, `0x8A550C58`, `0xEA5509E3`, `0xEA5509E4` → handle | true |
| 3 | `0x287259F8` | `m[2]` = `0x8A550C52` and `m[3]` = 0: read checkbox `0x8A550C52`'s state (slot 10) and handle `(0x8A550C52, w, state)` | true |
| 3 | other | — | true |
| 5 | `0x1B` (Escape) | handle `(0xEA5509E3, w, 0)` | false |
| `0x9A1552D2` | — | handle `(0x8A550C56, w, m[3])` (a grid click) | false |
| other | — | — | false |

"Handle" is `HandleControlMessage(id, w, value)` (`0x004F35F0`, RET 0xC); it always returns true.

The first gate calls **IsVisible**, slot 71 (`+0x11C`), not IsEnabled (slot 72).
The type-3 rows return true even when their ID/value filter prevents dispatch. The
`0x287259F8` row also does nothing if the effects-checkbox interface cannot be acquired;
otherwise it forwards that checkbox's boolean state, rather than the notification value.
There is no song-existence/enabled recheck in the direct song-toggle branch, and no separate
control-ID whitelist for list notifications. Thus filtering this envelope and executing a
control action are distinct stages (`0x004F4280`–`0x004F44AC`).

### 7.2 `HandleControlMessage(id, w, value)`

| `id` | Control | Effect |
|---|---|---|
| `0x8A550C50` | music checkbox | `P+0x10` := `value`; `UpdateSoundSys`; load the lists; `UpdateExclusionList` for k = 0 and k = 1 (inline) |
| `0x8A550C51` | music slider | `r = GetVolumeFromSliderValue(slider position, 1)`; `P+0x0C` := `r` when `r > lo_music`, else 0; `UpdateSoundSys`. Nothing when the slider child is missing |
| `0x8A550C52` | effects checkbox | `P+4` := `value` (byte); `UpdateSoundSys` |
| `0x8A550C53` | effects slider | as `…51` with `GetVolumeFromSliderValue(position, 0)` into `P+8` and `lo_sfx` |
| `0x8A550C56` | the grid clicked | §7.4 |
| `0x8A550C57` | Play | §7.4 |
| `0x8A550C58` | Next | when `+0x0C` is clear: set it, post `0xAA650861 (1, 0)`; then post `0x2A611B51 (0, 0)`: the radio's next song |
| `0x8A550C5A` | custom-tunes checkbox | `P+0x68` := `value` (byte) |
| `0xEA5509E3` | Cancel (and Escape) | §7.3 |
| `0xEA5509E4` | OK | §7.3 |
| `0x2A5BA8A7` … `+30` | Region song checkbox | `P+0x4A + (id − 0x2A5BA8A7)` := `value` |
| `0x3A5BA8A7` … `+30` | Mayor song checkbox | `P+0x2C + (id − 0x3A5BA8A7)` := `value` |
| other | — | nothing |

The song ranges are inclusive of `+30`. Index 30 would write `P+0x68` (Region) or `P+0x4A` (Mayor); only 30 buttons
per list exist *[quirk]*. The slider and checkbox changes apply to `P` at once, so the music and volume are heard
while the dialog is open.

### 7.3 OK and Cancel

- **Cancel** (`0xEA5509E3`):
  1. `UpdatePreferences(W+0x14)`: the copy goes back and the sound system and radio are told;
  2. post a standard message of type `0x231BBF91` (PreferencesChanged, application.md §6.2);
  3. `EndDialog`.

  The exclusion lists are **not** rebuilt *[quirk: song checkbox changes still apply to the radio's lists if they
  were rebuilt by the music checkbox meanwhile]*.
- **OK** (`0xEA5509E4`):
  1. The application's `SavePreferences` (slot 17, application.md §13.1).
  2. Load the lists; `UpdateExclusionList` for k = 0 and 1 (inline).
  3. When the radio (`kRZRadioDefaultServiceId` `0x1D07B39F`, IID `0xE0510260`, audio.md §7.1) exists, read its
     slot 32 into a string *[inferred: the current file path]*. When that succeeds and `IsSongEnabled(path)` is false, post `0x2A611B51 (0, 0)` to skip
     to the next song.
  4. Post `0x231BBF91`; `EndDialog`; release the radio.

A failed message allocation is not checked in either path.

### 7.4 The grid and Play

- **Grid click** (`0x8A550C56`):
  1. Disable Play.
  2. When the sound system `0x00B43CB8` is set, play the click: slot 11 `(1, 0x8A5C324A, 0, 0, 0)`.
  3. When a row is selected (§7.5) and maps to a song (§7.5), and that song's checkbox is enabled (`GetFlag(2)` on
     child `base_k + row`, `0x004F2140`), enable Play.
- **Play** (`0x8A550C57`):
  1. When `+0x0C` is clear: set it and post `0xAA650861 (1, 0)`, which puts the radio in preview mode.
  2. With a selected song: make a counted string object (0x14 bytes, `0x00408B90`) of
     `folder_k + text(0x8A635D24, file)`, AddRef it.
  3. Post a standard message of type `0xAA611B6E` whose data1 is the object (slot 12): the radio plays that file and
     releases it (audio.md §7.2.1).

### 7.5 Helpers

- `0x004F2370` `GetSelectedRow(&row)` (RET 4): false without a grid or with no selection (grid slot 40); else grid
  slot 41 `(&n, &row)` with `n` = 1, and true when `n` > 0.
- `0x004F23D0` `RowToSong(&row, &k)` (cdecl): rows `0 … n0−1` are Region entries (k = 0). Later rows have `n0`
  subtracted and are Mayor entries (k = 1) while in range. Returns the entry or null; a negative row is null.
- `0x004F1F70` `ListRecord(k)`: `0x00B21A88 + 0x30k` for k in 0–1, else null.
- `0x004F21B0` `EnableChild(w, id, on)` (thiscall on `w`): child `id` as `cIGZWin` → `SetFlag(2, on)`.
- `0x004F2140` `ChildEnabled(w, id)`: child `id` → `GetFlag(2)`; false when missing.

## 8. Quirks

- `LoadSongTable` carries the "enabled" value of a 3-value property over to later 2-value ones.
- The music and effects sliders mute (store 0) at or below the floor, so the lowest audible step is above 450 / 173.
- `InitDialog`'s position code compares the volume with 1000 rather than the ceiling.
- A volume below the floor zeroes the saved copy, so Cancel stores 0.
- The song checkbox ranges are 31 IDs wide; ID 30 writes a neighbouring field.
- `ShowDialog`'s result is ignored by both callers, and `DoModalWin` does not check the window.
- Cancel restores the preferences but not the radio's exclusion lists.
- `EndDialog` runs twice per normal close; the second call repeats the preview-end post.

## 9. Conformance cases

| Case | Setup | Expected |
|---|---|---|
| APW-C1 | shipped tuning; music slider 500 | `d = trunc(574 × 500 × −0.001f)` = −287 → `r` = 737; `P+0x0C` := 737 |
| APW-C2 | music slider 1000 / 0 | 1024 / 450 → stored 1024 / 0 (450 is not above the floor) |
| APW-C3 | music volume 800 at open | position `trunc(350 / 574 × 1000)` = 609 |
| APW-C4 | music volume 300 at open | position 0 and `W+0x1C` := 0; Cancel then stores 0 |
| APW-C5 | Region flags 1,1,0,…, mode 0 | Region exclusion list = the file names of the enabled songs, data1 1 (`krgn`), data3 0 |
| APW-C6 | a file `mysong.mp3` in `Radio\Stations\Mayor\Music\` | the custom-tunes checkbox enabled |
| APW-C7 | OK while the radio plays a Region song whose flag is 0 | `0x2A611B51` posted, then `0x231BBF91` |
| APW-C8 | toggle message for `0x2A5BA8A7 + 30` | `P+0x68` changes |
| APW-C9 | Escape key (type 5, `0x1B`) | Cancel path, `DoWinProcMessage` returns false |

### 9.1 AU43 worked cases

1. Start with a Region table whose successive properties are `(titleA,fileA,0)`,
   `(titleB,fileB)`, missing, `(titleD,fileD,1)`. The three entries have default flags
   0,0,1 and IDs base+0, base+1, base+3. Their grid rows are packed; the missing property
   does not add a row. A folder containing only those shipped file names, with different
   letter case, does not enable custom tunes. Adding `mine.PLS` does, without adding a row.
2. P Region flags are `[1,0]`, mode 0. Rebuild yields `[fileA]`, message data1=1,
   Pointer2=Region static list, data3=0. Toggling only the first song off updates P but
   leaves that list unchanged. A later music-enable action rebuilds it empty. Cancel
   restores `[1,0]` and emits master/radio-volume then PreferencesChanged, but leaves the
   list empty and does not save the file. This exposes the live-list/snapshot distinction.
3. P flags `[1,0]`, mode 1 produce `[fileB]` on rebuild. On OK the save occurs **before**
   the Region and Mayor list messages. A successful radio read returning fileB's full
   shipped path causes exactly one Next post before PreferencesChanged and preview-end.
   A missing radio, failed read, or unmatched custom path causes no Next post and still closes.
4. A visible accepted dialog receives type 3, button notification `0x287259F6`, music ID
   `0x8A550C50`: true, no action. The same ID with toggle `0x287259F7` updates P, updates
   audio, then posts both list rebuilds. Type 3 with an unknown notification also returns
   true with no action. Hidden or unrelated sources return false before all dispatch.
5. Effects ID `0x8A550C52` with notification `0x287259F8`, value 1: consumed true without
   action. With value 0 and an unchecked checkbox, store false from the checkbox state,
   update audio, return true. Escape from an accepted visible source restores and closes
   but returns false. An enabled state alone does not establish acceptance.
6. Preview Next twice posts preview-start once and Next twice. Cancel posts preview-end,
   destroys/clears the held window; DoModalWin's later EndDialog posts preview-end again.
   The preview byte remains 1. The two EndDialog AddRef/Release pairs balance independently.

## 10. Function inventory (owner `cSC4AudioPreferencesWinProc`, tools/coverage.py --owners)

All 26 functions are specified (11,812 bytes).

| Address | Bytes | Name | Section |
|---|---|---|---|
| `0x004F1F60` | 8 | AddRef | §1 |
| `0x004F1F70` | 27 | ListRecord | §7.5 |
| `0x004F1F90` | 133 | GetVolumeFromSliderValue | §4.2 |
| `0x004F2140` | 98 | ChildEnabled | §7.5 |
| `0x004F21B0` | 90 | EnableChild (`_EnableWindow`) | §7.5 |
| `0x004F2210` | 25 | Shutdown | §1 |
| `0x004F2230` | 132 | UpdateSoundSys | §4.4 |
| `0x004F22C0` | 169 | ReadTunerVariables | §4.1 |
| `0x004F2370` | 95 | GetSelectedRow | §7.5 |
| `0x004F23D0` | 117 | RowToSong | §7.5 |
| `0x004F2450` | 40 | constructor | §1 |
| `0x004F2480` | 145 | EndDialog | §6.5 |
| `0x004F2520` | 87 | UpdatePreferences | §5 |
| `0x004F2580` | 102 | SetDefaultMusicListPreferences | §3.4 |
| `0x004F25F0` | 210 | DoModalWin | §6.4 |
| `0x004F26D0` | 67 | Release | §1 |
| `0x004F27A0` | 360 | the folder scan (`_ShouldEnableCustomTunesCheckbox`) | §3.6 |
| `0x004F2A70` | 241 | AudioFileExists | §3.5 |
| `0x004F2B70` | 460 | StaticInit | §3.2 |
| `0x004F2D70` | 1092 | IsSongEnabled | §3.7 |
| `0x004F31C0` | 578 | HasCustomTunes (`InsertMusicList`) | §3.6 |
| `0x004F3410` | 480 | UpdateExclusionList | §3.8 |
| `0x004F35F0` | 3150 | HandleControlMessage | §7.2–§7.4 |
| `0x004F4280` | 556 | DoWinProcMessage | §7.1 |
| `0x004F44C0` | 3159 | InitDialog | §6.3 |
| `0x004F5120` | 191 | ShowDialog | §6.2 |

Related but not in the owner: `0x004F2910` `LoadSongTable` (§3.3), `0x004F2D40` (the audio handler's load), the
shared `QueryInterface` `0x004F1F30`, `_PostEventMessage` `0x004F20E0`.
