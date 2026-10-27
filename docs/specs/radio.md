# Radio: the disk radio, its stations and playlists (behaviour spec)

Status: M6 behaviour spec, written 2026-10-22 (re-radio lane, outside contributor). Every fact was read on the Windows
executable `bin/SimCity4.exe` (1.1.641, image base `0x00400000`) from the Ghidra export of `cRZRadio`,
`cRZDiskRadioStation` and `cGZPlayList`; vtables, strings, event ids and the option tests were checked on the bytes
and carry addresses. Implementers work from this spec, the LGPL gzcom-dll SDK headers and general knowledge only,
never from decompiled code.

Tags: *[compat]* observable, must match; *[inferred]* consistent with the evidence, not proven; *[quirk]* original
behaviour reproduced as is; *[as read]* read from the export without a byte check.

Scope. The in-game radio: the `cRZRadio` service, the station objects it builds from folders on disk, the
station's programme logic (music, DJ, commercials, jingles), and the M3U/EXTM3U/PLS playlist reader. The sound the
radio plays on is a framework sound ([audio.md](audio.md) §8.6); the SC4 code that changes stations from messages is
`cSC4AudioEventHandler` ([audio.md](audio.md) §7.2, message `0x26D31EC5` → `kMessageSetRadioStation`).

---

## 1. Identity *[compat]*

| Item | Value |
|---|---|
| `cRZRadio` CLSID | `0x5CFBDBBF` (factory `0x004EB150`, [audio.md](audio.md) §1.2) |
| `cRZDiskRadioStation` CLSID | `0x5CFBDBD5` (factory `0x004EB180`); 0x100 bytes |
| service ID | `0x1D07B39F` (`kRZRadioDefaultServiceId`), priority `0x7A116` (constructor `0x008D35AB`) |
| radio interface | `0xE0510260` and `1` → `this+0x1C` (`cIGZRadio`, vtable `0x00ACC310`); `0x287FB697` → base |
| station interface | `0x4056574B` (`cRZDiskRadioStation::QueryInterface` `0x008D2994`, vtable `0x00ACC290`) |
| sound-event target | `this+0x20` (vtable `0x00ACC2FC`, slot 3 `DoEvent` `0x008D319B`) |

The `cIGZRadio` table (`0x00ACC310`): 3 `Init`, 4 `Shutdown`, 5 idle tick (`0x008D30D1`), 7 `PlaySong`, 8
`PlayNextSong`, 9/10 current station, 11/13 `GetStation` by call sign, 12 station at index, 14 select station
(`0x008D3066`), 16 set volume (`0x008D2FDC`), 17 `GetStationCount`, 18 `IsStationAvailable`, 19 `AddStation`, 20/21
`RemoveStation`, 22 `RemoveAllStations`, 23 `TurnOn`, 24 `IsOn`, 25 `Mute`, 26 `GetOption`, 27 `SetOption`, 28/29
radio directory, 31 `StopPlayback`, 32 `GetCurrentSong`, 33 set the current song, 34 `IsPlaying`, 35 `ReadSettings`,
36 `WriteSettings`.

---

## 2. The radio (`cRZRadio`)

### 2.1 The event channel

Every control call is turned into a **sound-system event** that is dispatched at once, under the sound system's
lock, to its event handlers — the radio's own `DoEvent` among them ([sound-framework.md](sound-framework.md) §2.2)
*[compat]*. The call is the sound system's slot 7 (`+0x1C`, `(id, a, b, c, d)`), except "play file", which uses
slot 5 (`+0x14`) with a heap `cRZString` payload:

| Call | Event | Data |
|---|---|---|
| select station (slot 12 path) | `0x4AB186E9` | the two arguments |
| `StopPlayback` | `0x0AB186E5` | — |
| `PlayNextSong(force)` | `0x6AB1A480` | `force ≠ 0` |
| set volume | `0x0AB1A61E` | volume |
| idle | `0xEAB1A483` | — |
| `SendPlayMessage(path)` (`0x008D3BF8`) | `0xEAB186DF` | a copy of `path` |

`DoEvent` (`0x008D319B`):

| Event | Effect |
|---|---|
| `0x0AB186E5` | stop the radio sound (slot 8) |
| `0x0AB1A61E` | remember the volume; when there is a sound and it is not fading, set it (slot 21) |
| `0x4AB186E9` | when the sound plays, asynchronously fade from its current volume (sound slot 20) to data1 over data2 milliseconds (sound slot 22); mark the pending station transition |
| `0x6AB1A480` | no sound → 0; data 0 and the sound playing → 0 (no skip); else stop the sound, clear the current station's wait flag and make it play the next music item (station slot 23, type 0) |
| `0xEAB186DF` | when the event has a payload: stop the sound; reset its data source (`0x09D002D2` slot 3, `(0, 2)`); open the file (slot 36, `(path, 1)`); play (slot 6, `(volume, 0, 1)`); record it as the current song |
| `0xEAB1A483` | store the sound's playing state (slot 7) and fading state (slot 23); `IsPlaying` returns the first |
| other | 0 |

**Station transition (RA1).** *[compat]* The event uses
`Fade(currentVolume, data1, data2, true)`, not a playback-position operation
(`0x008D32BB`–`0x008D32F6`, sound table `0x00AC2C88`, fade `0x0087630C`). Its
transition mark is at radio object `+0x71`. Once the sound is no longer playing,
the unsuspended idle path clears that mark, stops playback and invokes the current
station's idle handler (`0x008D30D1`). The event itself does not open or start a
second sound. Fade execution follows [sound-renderer.md](sound-renderer.md) §4.4,
including its integer step and existing-fade refusal. The idle event caches the
fading state through sound slot 23 (`0x00877815`), and volume events defer setting
the sound volume while that cached flag is set; this flag is not a pause state.

### 2.2 `Init` and `Shutdown`

`Init` (`0x008D4687`) *[compat]*: needs `cIGZSndSys` (else false); stores it and the language manager. Once: when
there is no sound yet, the sound manager's default 2D data-source player class (slot 9) is created with IID
`0x206077D2` and initialised; the radio directory := the file system's data directory (slot 7) +
`Res\Music\Radio\`; the stations are built (§2.4); a pending turn-on is applied; the radio registers as a
sound-system event target (slot 15). Each call also (re)adds the radio to the framework's two hook lists (framework
slots 9 and 11). `Shutdown` (`0x008D2A09`): removes all stations (releasing them), stops and releases the sound,
leaves the framework lists, unregisters the event target.

### 2.3 Settings, options, volume

- `ReadSettings(s[8])` / `WriteSettings(s[8])` (`0x008D2ADF`/`0x008D2B3C`): `s[0]` volume, `s[1]` on, `s[2]` station
  index, `s[3]` option 0, `s[4]` option 1, `s[6]` option 3, `s[7]` a value kept at `+0x10`; `s[5]` is neither read
  nor written. Reading applies the options, then the volume, the station and `TurnOn(s[1] ≠ 0)`.
- `GetOption(i)` / `SetOption(v, i)`: options 0, 1 and **3** only (`0x008D2BA3`); any other index reads false and
  writes nothing *[compat]*. Option 0 enables DJ items, option 1 commercials, option 3 the default-language folder
  fallback (§2.4).
- Volume (`0x008D2FDC`): clamped to `0x400`, stored (also as the un-muted level); sent when there is a sound and no
  mute is active. `Mute(on)` counts: the first mute sends volume 0, the last unmute re-sends the stored level.
- `TurnOn(on)` (`0x008D2F21`): off → when on, mark off and stop playback; on → when off, mark on, build the stations
  when there are none, then (stations and a sound) start the current station (station slot 7), run its idle (slot
  5) and send the volume.

### 2.4 Building the stations (`SetupDiskStations`, `0x008D49F8`) *[compat]*

1. `stations := radioDir + "Stations\"`, `commercials := radioDir + "Commercials\"`.
2. List the sub-directories of `stations`. When there is at least one, remove all current stations first.
3. For each sub-directory `d` that contains a `*.id` file, make a `cRZDiskRadioStation`, add it (`AddStation`) and
   set its shuffle strategy from bit 0 of the settings word described below. Its folders:
   - station folder `stations + d + "\"`;
   - Music `station + "Music\"`;
   - DJ `station + "DJ\"`, Commercials `commercials`, Jingle `station + "Jingle\"`, each followed by the current
     language's folder name (radio slot 18 with 0); when the language is not 1 and that folder holds no sound file
     (§2.5): if option 3 is set, the default language's folder (slot 18 with 1) is used, otherwise the language part
     is dropped;
   - then the station reads its call sign (§3.1).
4. A saved station index beyond the count is reset to 0. Returns whether any station was made.

**Shuffle setting (RA2).** *[compat]* The tested word is at object `+0x2C`
(`0x008D4BD8`), also addressed as `+0x10` from the radio interface. ReadSettings
copies settings word `s[7]` into it and WriteSettings copies it back
(`0x008D2ADF`, `0x008D2B3C`). Construction clears it (`0x008D35AB`). A clear bit
selects strategy 0 (random shuffle); a set bit selects strategy 1 (file-time sort).
The value is captured when a station is built; it is separate from DJ/commercial/
language options and the current station index.

`AddStation(s)` (`0x008D38E5`): ignores a station already present; gives it the radio (station slot 6), initialises
it (slot 3) and appends it; when it is the first station and the radio is on, selects it and runs the idle tick.

### 2.5 Files

- `PlaySong(path)` (`0x008D3DC6`): when the file exists, `SendPlayMessage(path)`.
- `DoesAtLeastOneSoundEntryExistForTheGivenDirectory(dir)` (`0x008D3D5D`): any entry matching
  `*.mp3,*.wav,*.xa,*.m3u,*.pls` (`0x00ACC180`).

### 2.6 Station selection and the idle tick

Offsets `+0x54` and `+0x6D` below are relative to the `cIGZRadio` interface,
which begins at object `+0x1C`.

Select (`0x008D3066`, index `i`): when `i` differs from the current one, stop the current station (its slot 8),
stop playback unless a flag at `+0x6D` is set, clamp `i` to the last station, make it current and, when on, start
it (station slot 7). `GetStation(callSign)` (`0x008D484A`/`0x008D490B`) compares call signs (station slot 9); the
second variant also posts a station event with its extra argument and selects the match.

The idle tick (slot 5, `0x008D30D1`), when on and with stations: posts the idle event; then, unless suspended
(`+0x54`) and while the sound is not playing, clears a "station changed" flag (stopping playback when it was set)
and runs the current station's `OnIdle` (§3.2).

**Preview and deferred station changes (RA3).** *[compat]* Radio slot 30 directly
sets the suspend byte (interface `+0x54`, object `+0x70`; `0x009B1F4B`, table
`0x00ACC310`). The SC4 audio handler uses this slot for preview-start/end message
`0xAA650861` (`0x004EF810`); preview suppresses station idle programming, while the
radio still refreshes its playing/fading state. The keep-playing byte (interface
`+0x6D`, object `+0x89`) is instead set internally by the call-sign selection
variant, slot 13, when its transition-argument pointer is nonnull. That path sets
the byte before dispatching the fade event and finding/selecting the station
(`0x008D4926`–`0x008D4939`). It therefore prevents station selection from immediately
stopping the sound being faded (`0x008D3066`). Both bytes start at zero
(`0x008D35AB`). The call-sign path does not clear the keep-playing byte afterwards,
including when no call sign matches; it is a retained flag, not a scoped guard.

---

## 3. The disk station (`cRZDiskRadioStation`)

### 3.1 State

Constructor `0x008D376E`: on-the-air `+0x08` = 1, welcome-played `+0x09`, the **cycle** (a vector of the four item
types 0 music, 1 DJ, 2 commercial, 3 jingle, shuffled at creation), cycle index `+0x1C`, songs-in-a-row `+0x20`,
waiting `+0x24`, a millisecond timer `+0x28`, the radio `+0x40`, call sign `+0x48` (`*.id` file name), description
`+0x5C` (`*.txt` file path), the folders (`+0x70` station, `+0x84` Music, `+0xA0` DJ, `+0xBC` Commercials, `+0xD8`
Jingle) and one list + cursor per type (`+0x98`, `+0xB4`, `+0xD0`, `+0xEC`), the modifier list `+0xF4` and its mode
`+0xF8`, the shuffle strategy `+0xFC`.

`ReadCallSignFromDisk` (slot 22, `0x008D4579`): in the station folder, the first `*.id` entry's name is the call sign
and the first `*.txt` entry's path the description; true when a call sign was found. `GetStationCallSign` copies the
first **4** characters; `GetStationDescription` is the description when present, else the call sign.

Start (slot 7, `0x008D29D0`): read the call sign, load all playlists, shuffle, and play `Welcome.wav` from the
station folder (slot 24) when it exists and has not been played.

### 3.2 `OnIdle` (`0x008D2CFD`) *[compat]*

Nothing happens unless the station is on the air, the radio has a sound and that sound is not playing. Then:

1. If not waiting: start waiting and reset the timer; return.
2. Wait at least **4000 ms**.
3. If the current cycle entry is music, music exists and `rand() % 100 < 75`: count one more song and play music.
4. Otherwise advance the cycle index; past the end, reshuffle the cycle (§3.4) and restart at 0. By the new entry's
   type: music → play music if any; DJ → when `rand() % 100 ≤ 49` and DJ items are present; commercial → when
   `rand() % 100 ≤ 9` and present; jingle → when `rand() % 100 ≤ 9` and present. A failed test returns with the
   station still waiting, so the next idle call (the 4 s have passed) moves on again.
5. Clear waiting and play the next item of that type (§3.3).

`IsCycleTypePresent(t)` (`0x008D2EB0`): music → its list is non-empty; DJ → radio option 0 and its list; commercial →
option 1 and its list; jingle → **option 2** and its list. *[quirk]* The radio has no option 2 (§2.3), so the test
is always false and **jingles never play** (`push 2` at `0x008D2F03`).

### 3.3 Choosing the item

`StartPlayingNextSelectionOfGivenType(t)` (slot 23, `0x008D4127`): for music with a modifier mode set (1, or 2 with a
non-empty list): the modifier rule (below). Otherwise move that type's cursor to the next entry (wrapping); music
entries that do not exist as given are taken relative to the Music folder; DJ, commercial and jingle entries are
always folder + name; the radio plays it (`PlaySong`).

`PlayNextSelectionNotOnExclusionList` (slot 26, `0x008D3F8E`): one pass over the music list from the cursor; mode 1
skips names that are in the modifier list (exclusion), any other mode skips names that are **not** in it (inclusion);
the first existing file is played. When the pass returns to its start without a hit, the list is reshuffled.

`MoveToNextPlaylistItem(t)` (slot 21): advance the cursor, wrapping. `SetModifierList(list)` (slot 14) copies a list
of names.

### 3.4 Lists and shuffling

`LoadAllPlaylists` (slot 16, `0x008D3960`): for each pattern of `*.mp3,*.wav,*.xa,*.m3u,*.pls`, read the matching
files of the four folders into the four lists; then expand playlists (slot 25) in each.

`ExpandPlayLists` (`0x008D5827`): every list entry ending in `.m3u` or `.pls` (case-insensitive) is opened as a
playlist (relative names taken against the folder) and replaced by the playlist's file paths.

`ShufflePlaylist(strategy)` (slot 17, `0x008D4317`): for each non-empty list: remember its last entry; reorder it —
strategy 1 sorts by file time (`RZFileTimeSort`), any other shuffles (`random_shuffle`); when the new first entry is
the old last entry, swap the first and last; the cursor := the first entry.

`ShuffleCycle` (slot 19, `0x008D3485`): shuffle the four types; when the old last type was not music and is now
first, swap the first and last.

---

## 4. Playlists (`cGZPlayList`) *[compat]*

[sound-system.md](sound-system.md) §9 is this parser's interface contract (and owns its coverage row); this section
adds the line reader's details, the PLS keys and the M3U path rule it leaves to SS11. The two agree on the format
choice and the `#EXTINF:` pairing.

`Open(path)` (`0x008D6B22`): the directory of `path` is the base for relative entries. The whole file is read; NUL
bytes are replaced by spaces. Lines are read by `_GetLine` (`0x008D6258`): skip leading spaces, tabs, `\n`, `\r`;
take up to the next `\n` or `\r`; trim; convert the text encoding. The first line chooses the format:

- `#EXTM3U` → **EXTM3U** (`0x008D64E3`): repeated pairs: a line beginning `#EXTINF:` (length = the integer after
  `:` up to `,`; title = the rest), then the path line. *[quirk]* Any other line makes the whole open fail.
- `[playlist]` → **PLS** (`0x008D6824`): `FileN=path` lines, optionally followed by `TitleN=` and `LengthN=`, until
  `NumberOfEntries=N`; the open succeeds only when exactly `N` files were read.
- anything else → **M3U** (`0x008D66C9`, from the first line again): each line is a path; one without `:` and not
  starting with `\\` is relative to the playlist folder; the title is the file name.

Each entry (0x2C bytes) holds a title, an absolute path (`_MakeAbsolutePath` resolves `.` and `..` segments) and a
length in seconds.

---

## 5. Quirks *[quirk]*

1. Jingles never play: the station asks the radio for option 2, which does not exist (§3.2).
2. A failed DJ/commercial/jingle draw leaves the station waiting without a new 4 s gap, so items are skipped quickly
   (§3.2).
3. EXTM3U files fail on any line that is not an `#EXTINF` + path pair (§4).
4. The call sign is cut to 4 characters (§3.1).
5. Settings slot `s[5]` is untouched by both read and write (§2.3).

## 6. Open items

Questions are filed in [questions/radio.md](questions/radio.md) (prefix **RA**).

## 7. Conformance tests

1. **Folders.** `Res\Music\Radio\Stations\Rock\` with `ROCK.id` and `Music\a.mp3` gives one station "ROCK" whose
   music list is `a.mp3`; a sibling folder without a `*.id` file gives none.
2. **Gap.** After a song ends, the next item starts no sooner than 4000 ms later.
3. **Jingles.** With jingles present and every option set, no jingle is ever played.
4. **Options.** `SetOption(1, 2)` changes nothing; `GetOption(2)` is false.
5. **PLS.** `[playlist]`, `File1=a.mp3`, `Title1=A`, `Length1=10`, `NumberOfEntries=1` opens with one entry; with
   `NumberOfEntries=2` the open fails.
6. **M3U.** A line `songs\b.mp3` in `C:\list.m3u` resolves to `C:\songs\b.mp3`.
7. **Volume.** Setting `0x500` stores `0x400`; mute twice then unmute once keeps the sound at 0.
