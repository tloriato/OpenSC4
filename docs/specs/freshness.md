# Freshness: the grid-score ambience player (behaviour spec)

Status: M6 behaviour spec, written 2026-10-22 (re-freshness lane, outside contributor). Every fact was read on the
Windows executable `bin/SimCity4.exe` (1.1.641, image base `0x00400000`) from the Ghidra export of the `cFresh*`
classes; tables, constants, vtables and the ambiguous arithmetic were checked on the bytes and carry addresses.
Implementers work from this spec, the LGPL gzcom-dll SDK headers and general knowledge only, never from decompiled
code.

Tags: *[compat]* observable, must match; *[inferred]* consistent with the evidence, not proven; *[quirk]* original
behaviour reproduced as is; *[as read]* read from the export without a byte check.

Scope. "Freshness" is the EA library that plays a **score**: a grid of up to 64 × 64 **cells**, each one sound with
its own volume, pan, pitch spread, loop and start-probability settings. A **player** keeps a listener position on the
grid and, on a musical beat clock, starts and stops cells inside a selection window around it, so the mix keeps
changing. SimCity 4 uses it for the city ambience: `cSC4AudioAmbienceLayer` creates one player per ambience score and
drives it through sound events ([audio-ambience.md](audio-ambience.md) §6 and §8.6, which own the caller side). The
framework sound objects the cells play are the sound system's ([audio.md](audio.md) §8.6).

Relation to [sound-system.md](sound-system.md) §6, the interface contract for `cFreshPlayer` and
`cFreshCellPlayer` (its §8 lists the director): this spec adds what that spec leaves open in SS6 and SS7 (the score
format, the beat clock, cell selection, divisions, fades, the cell player's start and loop paths) and `cFreshScore`,
`cFreshCell` and `cFreshDimensionPlayer`. That spec's slot table follows the port's names, several of which sit one
slot off (§4.2 here gives what each body does); the two agree on the bodies they both cite.

---

## 1. Identity *[compat]*

`cFreshnessCOMDirector` (director ID `0xFD53FF17`, constructor `0x009456EC`, added by `cSC4AudioCOMDirector::OnStart`,
[audio.md](audio.md) §1.1) registers:

| CLSID | Factory | Object | Size | Returned |
|---|---|---|---|---|
| `0x3C1CF59E` | `cFreshPlayer::Create` (`0x009456A4`) | `cFreshPlayer` (§4) | 0x4144 | `+0x04` |
| `0x5C1CF5A3` | `cFreshScore::Create` (`0x00945675`) | `cFreshScore` (§2) | 0x4048 | base |
| `0xBDDA72DD` | `0x009456C8` | the dimension player (§6) | 400 | `+0x04` |
| `0x7DD9C9F2` | `cFreshCell::Create` (`0x0094565F`) | `cFreshCell` (§3) | 0x68 | base |
| `0xBDD9D7A5` | `0x0094568E` | `cFreshCellPlayer` (§5) | 0x5C | base |

Interfaces: player `0xDC1CF595` (`QueryInterface` `0x00946FE6` answers only this one), cell `0x1DD9C9DE`, cell player
`0x5DD9D787`, dimension player `0xDDDA72F1` (and `0xDC1CF595` at its `+0x04`).

Only the player is used by SC4: `cSC4AudioAmbienceLayer::AddFreshnessScore` (`0x004E6E00`) creates `0x3C1CF59E`
with IID `0xDC1CF595` (the only other byte occurrences of the two IDs are the director and the dimension player).
The dimension player is registered but never created by the game *[inferred: no other reference to `0xBDDA72DD`]*.

All player calls that change state take one process-wide lock (the critical section at `0x00B634AC`).

---

## 2. The score (`cFreshScore`)

### 2.1 Object

0x4048 bytes: a header (`+0x08`…`+0x44`) and a 64 × 64 array of cell pointers at `+0x48` (index `row × 64 + col`;
`GetCell` `0x009458C4`). Constructor defaults (`0x0094585F`) *[compat]*:

| Field | Default | Meaning |
|---|---|---|
| `+0x08` | `0x400` | master volume (`0x400` = unity) |
| `+0x0C` | 0 | block mode: snap the listener to blocks and allow cells to be stopped (§4.5, §4.6) |
| `+0x10` | 0 | maximum-cells base (§4.4) |
| `+0x14` | 4 | minimum-cells base |
| `+0x18`, `+0x1C` | 0 | maximum-cells column and row terms |
| `+0x20`, `+0x24` | 2, 2 | selection half-height (rows) and half-width (columns) |
| `+0x28`, `+0x2C` | 4, 4 | rows, columns |
| `+0x30` | 120 | tempo, beats per minute |
| `+0x34` | 8 | beats per bar |
| `+0x38`, `+0x3C` | 1, 1 | parsed and stored, never read (§3.2) |
| `+0x40`, `+0x44` | 0, 0 | the listener's start X and Y, 0–1000 (never parsed, so always 0) |

### 2.2 The text format *[compat]*

`LoadScore` reads a whole file (`0x00946B30`, by path) or record (`0x00946A79`, an `IGZFile`), NUL-terminates it and
calls `ParseScore` (`0x00946039`). Lines end with `\n`. `cRZString::SplitToken(s, sep)` returns the text before the
first `sep` and removes it and `sep` from `s`; it returns an empty string, leaving `s` unchanged, when `sep` is
absent *[inferred from its callers]*.

1. The text must begin with `version`; otherwise nothing is read (`ParseScore` still returns 1). The rest of that line
   is the version number.
2. Versions 7, 6 and 5 are read; any other leaves the score empty.
3. Skip one line; the next line is the header: 14 tab-prefixed integers (`"\t%ld"` × 14) into `+0x08`, `+0x0C`,
   `+0x10`, `+0x14`, rows `+0x28`, columns `+0x2C`, tempo `+0x30`, beats per bar `+0x34`, `+0x24`, `+0x20`, `+0x38`,
   `+0x3C`, `+0x18`, `+0x1C` (in that order). Skip one more line.
4. Then one line per cell, row by row, `rows × columns` lines (no bound check against 64 *[quirk]*). Each line is
   tab-prefixed integers then a name (`%s`): 18 integers for version 7, 17 for versions 5 and 6 (field 18 is then 0),
   in the order of §3.1.
5. After the fields the line may carry links `(c,r)`: split on `(`; while the part before it is non-empty, split the
   rest on `)` and read `"%ld,%ld"`. *[quirk]* Two links written back to back (`…)(…`) stop after the first,
   because the part between them is empty; links must be separated (for example by a tab).
6. A cell whose name is `NONE` (case-insensitive, `0x00AD49B4`) is discarded: that grid slot stays empty.

---

## 3. The cell (`cFreshCell`)

### 3.1 Fields *[compat]*

0x68 bytes (constructor `0x00945DCC`). Fields in score-line order:

| # | Offset | Default | Meaning |
|---|---|---|---|
| 1 | `+0x24` | `0x400` | volume |
| 2 | `+0x30` (byte) | 0 | random volume: each start lowers the volume by up to a quarter (§5.2) |
| 3 | `+0x28` | `0x200` | pan X (`0x200` = centre) |
| 4 | `+0x2C` | `0x200` | pan Z |
| 5 | `+0x31` (byte) | 0 | random pan (§5.4) |
| 6 | `+0x34` | 0 | fade in on start when non-zero |
| 7 | `+0x38` | 0 | fade time (ms) for fade-in and stop |
| 8 | `+0x3C` | 0 | stored, never read (§3.2) |
| 9 | `+0x44` (byte) | 0 | stored, never read (§3.2) |
| 10 | `+0x48` | 8 | loop length, in beats |
| 11 | `+0x50` (byte) | 1 | looping |
| 12 | `+0x4C` | 0 | stored, never read (§3.2) |
| 13 | `+0x40` | 0 | start class 0–20 (§4.7) |
| 14 | `+0x54` | 0 | pitch spread low, semitones |
| 15 | `+0x58` | 0 | pitch spread high, semitones |
| 16 | `+0x20` (byte) | 0 | stored, never read (§3.2) |
| 17 | `+0x5C` | 0 | group: at most one cell of a non-zero group plays at a time |
| 18 | `+0x60` (byte) | 0 | version 7 only; stored, never read (§3.2) |
| — | `+0x0C` | "" | the sound's file name |
| — | `+0x64` | empty | links: cells `(col, row)` that must not be playing for this one to start |
| — | `+0x08` | 0 | sound id, filled at start from the name (§5.1) |

The cell's interface (vtable `0x00AD4908`) is plain getters and setters of these fields.

### 3.2 Fields nothing reads *[inferred]*

Cell fields 8, 9, 12, 16 and 18 and the score's `+0x38`/`+0x3C` have a parser, a constructor default and a setter
(and, for the cell, a plain getter) but no consumer: neither the player, the cell player and the dimension player
(which read the cell's other fields directly, not through the getters) nor SimCity 4's only client, the ambience
layer (it creates a player and drives it by key, volume, play percent, fades and the head-relative and distance
settings, [audio-ambience.md](audio-ambience.md) §6), ever load them. They therefore have no role in this build: a
re-implementation parses and keeps the numbers (a score written for another host may use them) and lets them change
nothing. Do not confuse the score's `+0x38`/`+0x3C` with the player's own `+0x38` (cells in the window, §4.4) and
`+0x3C` (beat in bar, §4.1), which are live.

---

## 4. The player (`cFreshPlayer`)

### 4.1 Object

0x4144 bytes (constructor `0x009489AE`). Main fields: `+0x08` initialised, `+0x10` its `cRZRandom` (seeded
`0xFFFFFFFF`), `+0x14` volume (`0x400`), `+0x18` the beat timer, `+0x1C` the score, `+0x20` playing, `+0x21` manual
(no automatic starts or stops), `+0x24` tempo, `+0x28`/`+0x2C` listener column/row, `+0x34` zoom, `+0x38` cells in
the window, `+0x3C` beat in bar, `+0x40` next beat number, `+0x44` beat origin, `+0x50` distinct groups in the window,
`+0x54`/`+0x58` selection half-width/height (3, 3), `+0x5C`/`+0x60` the last cell stopped, `+0x64`/`+0x68` the last
cell started, `+0x74` the 64 × 64 cell players, the play percent (`+0x4088`, 100), the stress flag (`+0x4090`) and the fade-running flag (`+0x4091`), the minimum and maximum 3D distance (100,
1000), the 3 × 3 divisions (§4.8) and the resource key.

### 4.2 The table

The vtable at `0x00AD4B48` (66 slots). Several of the port's names (from the Mac build) sit one slot off; this table
gives what each body does *[compat]*:

| Slot | Address | Does | Port name |
|---|---|---|---|
| 3 | `0x00947C38` | `Init(key)`: set the key, open the record, make a score, `LoadScore`, then slot 4 | `Init` |
| 4 | `0x0094701E` | `Init()` (§4.3) | `Init` |
| 5 | `0x00948C11` | load by path (slot 38) | `LoadScore` |
| 6 | `0x00948B7F` | the score | `GetScore` |
| 7 | `0x00947D28` | `Shutdown`: end any fade, stop, free the score and every cell player | `Shutdown` |
| 8 | `0x00947DB8` | `Play` (§4.5) | `Play` |
| 9 | `0x00948B85` | is playing | `IsPlaying` |
| 10 | `0x00947167` | `Stop` (locked slot 53) | `Stop` |
| 11 | `0x00947559` | `SetVolume` (locked slot 43) | `SetVolume` |
| 12 | `0x0094832A` | `FadeVolume(ms, target, stopAtEnd)` (§4.9) | `FadeVolume` |
| 13 / 14 | `0x00948504` / `0x009484A0` | set listener Y / X from 0–1000 (§4.6) | `SetYPos` / `SetXPos` |
| 15 | `0x0094776D` | zoom := v | `SetZoom` |
| 16 | `0x0094777F` | play percent := v (only once initialised; §4.6) | `SetPlayPercent` |
| 17 | `0x00948420` | `SetTempo(bpm)`: when playing and changed, restart the beat clock at the current beat | `SetTempo` |
| 21 | `0x00947794` | manual := v | `SetManual` |
| 22–25 | `0x00947996`… | first/last selectable row and column: listener ± half-size, clamped to the grid | as named |
| 26 | `0x00948B89` | cell player `(row, col)` | `GetCellPlayer` |
| 27 | `0x009479D4` | `Reseed(seed)`: reseed, reset the beat state, recount; when playing, stop and play | `Reseed` |
| 28 / 29 | `0x00948B9B` / `0x00948BAA` | head-relative flag; min and max distance | as named |
| 30 / 31 | `0x00947AA9` / `0x00947AC1` | divisions in X / Z: stores `n` when `n < 3` (the count is `n + 1`) | as named |
| 32 | `0x00947A2A` | `SetDivisionEnabled(x, z, on)`, `x, z < 3` | as named |
| 33 | `0x00947A55` | `SetDivision3DPosition(x, z, px, py, pz)` | as named |
| 34 / 35 | `0x00948BDD` / `0x00948BFA` | resource key | `SetKey` / `GetKey` |
| 38–40 | `0x00948CB4`… | load by path → (directory, file) → (score, directory) → slot 4 | `Init` |
| 41 | `0x00947150` | `SetVolume(v)` then `Play` | `Update` |
| 42 | `0x00947197` | end a running fade: stop and delete its timer | `Play` |
| 43 | `0x0094758E` | set the volume on every cell player (`SetVol`) | `StopFade` |
| 47 / 48 | `0x009478EA` / `0x009478FD` | set / get the selection half-height and half-width | `GetDataDir` / `SetSelectionAreaMaxDistances` |
| 50 | `0x00948BD0` | stress flag: while non-zero the beat clock does nothing (§4.6) | `SetStressLevel` |
| 51 | `0x00947914` | two fields `+0x6C`, `+0x70` (no reader found) | `SetPause` |
| 52 | `0x009471EE` | is `(row, col)` inside the selection window | `UpdateListener` |
| 53 | `0x00947E90` | stop without the lock (§4.5) | `StopNoLock` |
| 54 | `0x00948719` | find the best cell to start (§4.7) | — |
| 55 | `0x00947220` | find a cell to stop (§4.6) | `GetCellToAdd` |
| 56 | `0x00947519` | stop a cell, remembering it | `GetCellToSubtract` |
| 57 | `0x009482C5` | toggle a cell: start it if idle, else slot 56 | `SubtractCell` |
| 58 | `0x00948573` | a cell's start score (§4.7) | `ToggleCell` |
| 59 | `0x009477A6` | number of busy cells | `ProbOfCellStarting` |
| 60 | `0x0094780D` | number of cells being stopped | `NumCellsBusy` |
| 61 | `0x00947858` | number of playing cells | `NumCellsDying` |
| 62 | `0x009478BF` | debug: calls slots 59 and 61 | `NumCellsPlaying` |
| 63 | `0x00947925` | maximum cells (§4.4) | `ShowUsage` |
| 64 | `0x00947982` | minimum cells | `GetMinCellsPlaying` |

### 4.3 `Init()` (`0x0094701E`) *[compat]*

Once: volume := score `+0x08`; listener at column 1, row 1; the two cell-count bases := score `+0x10`, `+0x14`;
half-width := score `+0x24`, half-height := score `+0x20`; one `cFreshCellPlayer` (§5) for every non-empty grid
cell; count the cells inside the window; initialised := 1.

### 4.4 Cell budget *[compat]*

- minimum := score `+0x14` × playPercent / 100;
- maximum := playPercent × score `+0x10` / 100 + (row + 1) × score `+0x1C` / max(rows − 1, 1) + (col + 1) × score
  `+0x18` / max(cols − 1, 1), capped at the number of distinct groups in the window (`+0x50`).

`+0x38` (cells in the window) and `+0x50` are recounted (`0x009476C3`) whenever the listener moves: `+0x50` counts
each group-0 cell once and each non-zero group once. *[quirk]* The seen-groups array has 32 entries; a group ≥ 32
writes past it.

### 4.5 `Play` and `Stop`

`Play` (`0x00947DB8`, when initialised and not playing): end any fade; manual := 0; tempo := score tempo; origin :=
beats per bar (minus 1 when above 2); beat in bar := next beat := origin; recount; start the beat timer; set X and Y
from score `+0x40`/`+0x44` (always 0, so the window sits at its first legal position); playing := 1; register the
player with the sound system for an update every **250 ms** (`0xFA`, sound-system slot 30).

`StopNoLock` (`0x00947E90`): unregister, end the fade, stop and free the timer, stop every playing cell, playing := 0.

### 4.6 The beat clock (`0x00947F58`) *[compat]*

Each update (while initialised, playing and the stress flag, set through slot 50, is zero; with the flag set the
whole update, including step 4, is skipped): `now` := the timer's elapsed ms. Beat `n` falls at
`(n − origin) × 60000 / tempo` ms (`0x009475FB`; −1 when the tempo is 0). While the next beat is due:

1. After 24 hours (`now > 86,400,000`): reset the beat state to the bar length, restart the timer and stop catching
   up.
2. On beat-in-bar `= bpb − 1` or `bpb − 6` (bpb = beats per bar), with block mode on (score `+0x0C`), not manual,
   and at least the minimum number of cells playing: pick a cell to stop (slot 55): first a random cell anywhere
   (random start row = byte 1 of a draw mod rows, column = byte 0 mod columns, then scanning on and wrapping) that is
   playing, outside the window and not already stopping; failing that, a uniformly random playing cell inside the
   window whose class is not 20 and that is not stopping. Mark it stopping; remember its group (and its class when
   the group is non-zero).
3. On the same beats, when not manual, the play percent is non-zero (the same `+0x4088` field slot 16 sets and §4.4
   scales the budget by, so a play percent of 0 starts nothing), no volume fade is running (§4.9) and the active
   cells (busy − stopping) are below the maximum: find the best cell to start (slot 54) for the remembered group;
   failing that, retry once more for a class-19 group (`0x13`), and again when below the minimum. When found and a
   division can be chosen (§4.8), give the cell player its cell, the external sound interface, and start it.
4. Update every cell player (§5.3).
5. beat in bar += 1, wrapping to 1 after bpb; next beat += 1.

`SetXPos(v)` / `SetYPos(v)` (`0x009484A0`/`0x00948504`, `v` in 0–1000): `p := size × v / 1000`; in block mode snap
`p` to the centre of its block of `2·half + 1`; clamp to `[half, size − half − 1]`, then to `[0, size − 1]`; recount.

### 4.7 Choosing a cell to start *[compat]*

**The start score** of cell `(r, c)` for a group `g` (slot 58, `0x00948573`) is 0 unless: the grid cell exists, its
player is neither dead nor busy, it is inside the window and it is not the last stopped cell. Then:

- if the cell's group is non-zero and another playing, non-stopping cell has the same group → 0;
- if any linked cell (§3.1) is playing and not stopping → 0;
- class := the cell's `+0x40` clamped to 0–20; class 0 → 0 *[quirk: the function then returns an undefined
  value]*;
- class 20 → 101 when `g` equals the cell's group, else 100;
- otherwise, if `+0x38` is 0 it is set to 10; a cell of group `g` (`g ≠ 0`) → 2; else 1 when `rand() <
  T[class] / +0x38` (C `rand`, 0–32767), else 0.

`T` is the 21-entry table at `0x00B16F30`:

```
0  1    2    3    4    5     6     7     8     9     10    11    12    13    14    15    16     17     18     19        20
0  600  1500 3000 7500 12000 15000 18000 21000 25500 32767 36000 45000 58000 67500 90000 120000 210000 300000 40000000 40000000
```

**The best cell** (slot 54, `0x00948719`): a random origin inside the window (row = listener row ± half-height from
byte 1 of a draw, column likewise from byte 0, clamped to the grid); scan the window from that origin, wrapping in
both directions; keep the cell with the highest score (strictly greater). Found when the best score is non-zero.

### 4.8 Divisions

A 3 × 3 set of 3D positions the cells are placed at. Defaults: counts 3 × 3 (stored 2, 2); position `(i, j)` =
`(minDist × (i − 1), 0, minDist × (1 − j))` with `minDist` = 100; only the four edge centres `(0,1)`, `(1,0)`,
`(1,2)`, `(2,1)` enabled. Choosing (`0x00947AD9`) for a cell player: with `n` enabled divisions, draw `k =
Random(n × 1000)` and walk the enabled ones **other than the current**, taking the one where `k < 1000` (subtracting
1000 per step); when the walk runs out the current one is kept (probability `1/n` when the current one is
enabled). The cell player then gets the head-relative flag, the min/max distances and that position.

### 4.9 Volume fades

`FadeVolume(ms, target, stopAtEnd)` (`0x0094832A`, when playing): `step := (target − volume) / 20`. A zero step sets
the volume at once (and stops when asked). Otherwise a callback timer is created (its delay set to 100, which is already
its default and which the timer never passes to its service, [timers.md](timers.md) §2.2, then started
with period `ms / 20`) unless one runs; the fade-running flag (`+0x4091`) is set with the timer and cleared when the fade ends (slot 42 or the callback
reaching its target), and while it is set the beat clock starts no new cell (§4.6 step 3); each callback (`0x0094761C`, skipped while the lock is held elsewhere) adds
`step`, clamps to 0–`0x400` and sets the volume; reaching the target ends the fade, and a downward fade with
`stopAtEnd` then stops the player.

---

## 5. The cell player (`cFreshCellPlayer`)

### 5.1 States and start

0x5C bytes (constructor `0x009458D6`): state `+0x20` (0 idle, 2 playing, 3 stopping), the sound `+0x1C`, the cell
`+0x24`, loop counter `+0x28`, stopping mark `+0x2D`, random-volume factor `+0x30`, player volume `+0x34` (`0x400`),
scaled volume `+0x38`, head-relative `+0x3D` (1), position `+0x40`–`+0x48` (0, 0, 100) with "set" flag `+0x3C`,
distances `+0x4C` = 1000 and `+0x50` = 100, the external sound interface `+0x54`, dead `+0x58`. Vtable `0x00AD48A0`:
4 `SetCell`, 5 `Start`, 6 `Update`, 7 `Stop`, 8 `Kill`, 9 `SetVol`, 10 playing, 11 busy, 12 dead, 13 set dead,
14 `AutoPan`, 15 head-relative, 16 position, 17 min/max distance, 18 set the external interface.

`Start` (`0x00946C2C`) *[compat]*: fails without a sound system, when dead or busy. Clears the stopping mark. With an
external interface → §5.5. Otherwise:

1. **Sound**: if the cell's name has a run of 8 hex digits, cut it at the first `.`, take the last 8 characters as hex
   and use that as the sound id (stored in the cell), created by the sound manager (`GetSound`, slot 18, IID
   `0x206077D2`); otherwise the sound manager creates it by file name (slot 17). Failure → false.
2. Make it 3D: its register `0x1C` (alias of `0xFFCC156F`, [hit-music.md](hit-music.md) §7.1) := 1; 3D interface:
   head-relative, the distances (slots 12 and 14, as unsigned floats), the position when set.
3. Play: cell `+0x34 == 0` → play at the scaled volume with the cell's loop flag; else play at 0 then fade to the
   scaled volume over the cell's fade time (sound slot 22).
4. State := 2; register the sound's `0x5B34F0AF` interface with the sound system for 250 ms updates.
5. Volume: factor := `0x400`, minus `|r| mod 256` when the cell has random volume; recompute the scaled volume (§5.2)
   and set it.
6. Loop counter := the cell's loop length when looping.
7. **Pitch**: when the spread is set (low or high ≠ 0) and low ≤ high: `k := r mod (high − low + 1) + low`; the
   sound's frequency × `P[k]` for `k ≥ 0`, ÷ `P[−k]` for `k < 0`, where `P` is the 13 doubles at `0x00B16EB0`,
   `P[k] = 2^(k/12)`. *[quirk]* `|k| > 12` reads past the table (zeros).

### 5.2 Scaled volume (`UpdateScaledVol`, `0x00945820`) *[compat]*

`v := cell volume × factor; v := player volume (+0x14) × v / 2^20; v := score master × cell-player volume × v /
2^20` (32-bit signed arithmetic).

### 5.3 `Update` (each beat, `0x009459BF`)

- Playing and looping: decrement the loop counter. Without the stopping mark, reload it from the loop length when it
  reaches 0. With the mark, kill the cell (slot 8: release the sound at once, no fade, state idle, mark cleared)
  whenever the decremented counter is **non-zero**; a counter that lands exactly on 0 leaves it playing until the
  next update (where it goes to −1 and is killed). So a marked cell is cut at the next update, which is the same
  beat when the beat clock marked it, except for the exact-zero case *[quirk]*; it does not wait for the loop to
  end.
- Playing, not looping: once the sound no longer plays, stop and go idle.
- Stopping: once the sound's fade has finished (slot 20 returns 0), kill it and go idle.

`Stop` (`0x00945D1A`): unregister the sound's updates; playing with fade time 0 → kill now; with a fade time →
state 3 and fade the sound to 0 over that time; idle → idle.

### 5.4 `AutoPan` (`0x00945AAF`)

`x := (panX − 0x200) / 512`, `z := (panZ − 0x200) / 512`. With random pan and both pans centred: a random quadrant
`r mod 4` → (0, 1), (0, −1), (−1, 0), (1, 0). With random pan and only one axis centred, that axis becomes ±1 at
random. The 3D position becomes `(x × 100, 0, z × 100)`. Debug globals at `0x00B6349C`–`0x00B634A0` can force the
quadrant and the random pan.

### 5.5 The external path

`StartCellExternal` (`0x0094680F`) hands the cell to an external sound interface instead (sound id from the name as
in §5.1, else `CRC24(file name) | 0xFF000000`), then sets its position, distances, volume (scaled × 1/1024) and pitch
ratio. *[quirk]* For a negative pitch step it uses `P[−k]` **without** inverting it, so negative steps raise the
pitch. SC4 never sets the external interface *[inferred: the slot-18 setter is only reached from the dimension
player]*.

---

## 6. The dimension player (`0xBDDA72DD`)

400 bytes (constructor `0x00949902`): a manager of several players keyed by id (`LoadScore(path, id)`,
`0x00949C27`, creates a `cFreshPlayer` and forwards its settings), with an enabled set (`0x00949EA4`: disabling a
player stops all its cells), a pooled list of the enabled players' window cells with their class sum
(`0x00949D19`), and the same 3 × 3 division logic (`CalculateCellplayer3DPosition`, `0x00948F62`). Its `Update`
(`0x0094970D`) starts, every `max(2, 7 − level)` seconds while fewer than 4 cells play, one pooled cell chosen with
probability proportional to its class *[as read]*. Not used by SC4 (§1).

---

## 7. Quirks *[quirk]*

1. A score's listener start position (`+0x40`/`+0x44`) is never parsed (§2.1, §4.5).
2. Back-to-back cell links stop after the first (§2.2).
3. The group-count array overflows for groups ≥ 32 (§4.4).
4. A class-0 cell's start score is undefined (§4.7).
5. Pitch steps beyond ±12 read zeros; the external path does not invert negative steps (§5.1, §5.5).
6. The cell player's distances default to min 1000, max 100 (§5.1).
7. Several port names are one slot off (§4.2).
8. A looping cell with the stopping mark is killed at its next update, not at the end of its loop; a counter landing exactly on 0 delays that by one update (§5.3).
9. The cell fields 8, 9, 12, 16 and 18 and the score's `+0x38`/`+0x3C` are write-only (§3.2).

## 8. Open items

Questions are filed in [questions/freshness.md](questions/freshness.md) (prefix **FS**).

## 9. Conformance tests

1. **Header.** `version 7`, a skipped line, then `\t1024\t1\t3\t2\t8\t8\t100\t4\t2\t2\t1\t1\t0\t0` gives an 8 × 8
   score at 100 bpm, 4 beats per bar, block mode, minimum base 2, maximum base 3.
2. **NONE.** A cell line ending in `NONE` leaves the grid slot empty and makes no cell player.
3. **Beat times.** With 8 beats per bar the origin is 7; at 120 bpm the first beat (7) is due at once, beat 8 at
   500 ms and beat 9 at 1000 ms.
4. **Budget.** Play percent 50, minimum base 4 → minimum 2.
5. **Class 20.** A class-20 cell in the window, idle and unlinked, scores 100 (101 for its own group) and is always
   chosen over class < 20 cells.
6. **Links.** A cell linked to a playing cell scores 0.
7. **Pitch.** Spread −12…−12 halves the frequency; +12…+12 doubles it.
8. **Random volume.** With random volume the factor is in 769…1024.
9. **Fade.** `FadeVolume(2000, 0, 1)` from 1024 steps by −51 every 100 ms and stops the player at 0.
10. **Divisions.** With only the default four enabled and the current one among them, a new choice keeps the
    current one with probability 1/4.
11. **Stopping mark.** A playing looping cell with loop length 8 and the mark set is killed by its first update (the counter reads 7); with loop length 1 the first update leaves it playing (the counter reads 0) and the second kills it.
12. **Play percent 0.** With the play percent 0 the beat clock never starts a cell (it may still stop cells); with the stress flag set it does nothing at all; with a volume fade running it starts no cell.
