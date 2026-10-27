# `cSC4WinIntroVideoScreen`: the intro video screen (behaviour spec)

Scope: the `cSC4WinIntroVideoScreen` owner as tools/coverage.py assigns it (22 functions; inventory in §5). It
consists of:
- the boot screen's 12 functions;
- 10 functions of the MAD video player `cSC4MadCowPlayerWin` that only this screen reaches: the notification
  setter, the movie queue and the player's destructor.

The player's decoder itself is outside this owner. The application creates this screen first
(`cWinSC4App::CreateIntroVideoScreen`, INVENTORY-M6 §4.3); then come the splash, region and city screens.

The screen's sequence:
1. Movie `{0x0A8B0E70, 1, 2}`.
2. The Maxis logo, about 6 s.
3. Movie `{0x0A8B0E70, 1, 1}`.
4. The splash picture, about 2 s.
5. The region screen.

Esc or a click during the first movie skips to the application's splash screen.

Evidence: static RE only, on the instruction and data bytes of the Steam 1.1.641 exe (the same code as GOG,
`docs/lanes/re-dates.md`). Every function was disassembled; the two movie keys were read from the data section
(`0x00B0DB60`, `0x00B0DB48`). Written 2026-10-23 (re-introvid7).

Conventions:
- `S` is the screen, a `cGZWin` with vtable `0x00AB78F0`.
- `P` is the player.
- "Post `m`" means `RZ::MS()` slot 7 with a 16-byte record `{m, 0, 0, 0}` and `S` as the target. That is a deferred
  delivery to `S` itself (framework-gz-services.md GZ4).

## 1. Identity and fields *[compat]*

| Offset | Field |
|---|---|
| `+0xD8` | `P` (counted) |
| `+0xDC` | the Maxis logo window (counted) |
| `+0xE0` | the splash window (counted) |
| `+0xE8` | a `cRZTimer` (constructed with 4) |
| `+0x100` | the repaint countdown |

The pieces:
- **Constructor** (`0x0079CED0`): `cGZWin`; the three windows null; the timer; countdown 0; `SetFlag(0x10000, 0)`.
- **`QueryInterface`** (`0x0079CF20`): IIDs 1 and `0x2A3832AA` give `S` with an `AddRef`; anything else goes to
  `cGZWin::QueryInterface`.
- **Destructor** (`0x0079D610`):
  1. `Shutdown`;
  2. the timer's destructor;
  3. `~cGZWin`;
  4. `GlobalHeapFree` when bit 0 of its argument is set.

The window overrides `Init` (`0x0079CFE0`), `Shutdown` (`0x0079CF50`), `DoMessage` (`0x0079D2F0`), `GZPaint`
(`0x0079D490`), `GZOnMouseDownL` (`0x0079D460`) and `GZOnKeyDown` (`0x0079D470`).

## 2. `Init` and `Shutdown`

**`Init`**: true at once when initialised. Otherwise:
1. `cGZWin::Init`.
2. `S` gets `SetArea(0, 0, width, height)` of the main window (`GetMainWindow`, slot 10; slots 41 and 42).
3. Construct `P` (0x228 bytes, `0x0080FFC0` with argument 1) and `AddRef` it.
4. `P` gets `SetArea(0, 0, 768, 384)` and moves to `((width − 768)/2, (height − 384)/2)` of `S`.
5. Queue the first movie: `QueueMovie(&{0x0A8B0E70, 1, 2}, 3)` (§4).
6. `SetNotification(S, 0x8AAAFD43)` (§4).
7. `P+0x194` := the sound preference: byte `+0x69` of the preferences (`cISC4App` slot 38; application.md: "audio"
   on/off) is not 0.
8. `S` adds `P` as a child (slot 14).
9. True.

**`Shutdown`**: when initialised, for the logo window, the splash window and `P` in that order, when present:
1. `ChildDelete` it (slot 16);
2. release it;
3. clear the field.

Then `cGZWin::Shutdown`. The result is true.

## 3. The sequence

**`DoMessage(msg)`**:

| Message | Effect | Result |
|---|---|---|
| `0x8AAAFD43` (first movie over) | `SetupMaxisLogoScreen`, `SetupSplashScreen`; show the logo window (slot 69); start the timer | true |
| `0xEAAC1A96` (logo over) | `QueueMovie(&{0x0A8B0E70, 1, 1}, 0)`; `SetNotification(S, 0x4A384F52)` | true |
| `0x4A384F52` (second movie over) | the splash window: `PullToFront`, `ShowWindow`; reset and start the timer; then `cGZWin::DoMessage` | its result |
| `0xCAAC6A52` (splash over) | the application (`0x00B43CBC`): `DestroyCurrentScreen`, then `CreateRegionScreen` | true |
| `0xEAAD620A` (skipped) | the application: `DestroyCurrentScreen`, then `CreateSplashScreen` | true |
| other | `cGZWin::DoMessage` | its result |

**`SetupMaxisLogoScreen`** (`0x0079D110`):
1. The logo window `+0xDC` := `SC4CreateWindowFromScript` (`0x005F9390`) with:
   - the key `{0, 0x96A006B0, 0xAAAAF3D1}`;
   - the parent `S`;
   - the ID `0xAAA9C9D9`.

   When that fails, the result is false.
2. Move it to `(max((width − w)/2, 0), max((height − h)/2, 0))` of `S`.
3. `PullToFront`, `HideWindow`, `SetFlag(0x20, 1)`.
4. `SetFadeEffectPeriod(1000, 1000)` (slot 109).
5. True.

**`SetupSplashScreen`** (`0x0079D200`): the same, with:
- the key instance `0x8AA9AA14`, into `+0xE0`;
- `SetFadeEffectPeriod(2000, 0)`.

**`GZPaint`**: the screen keeps repainting itself while its timers run.
1. When the countdown is 0, set it to 2.
2. Clear (`0x0079D4AE`–`0x0079D4F3`): the graphics system (`RZ::GS()`, queried for IID `0xC47B747C`) gives its driver
   (slot 3, as in texture-bindings.md §1.3); skipped when the query or the driver fails. The driver gets slot 12
   `ClearColor(0, 0, 0, 0)` (a black, transparent clear colour), slot 15 `ColorMask(1)` (colour writes on in the OpenGL
   driver; the DirectX 7 driver only flushes its queue here, renderer-driver.md §5.2) and slot 11 `Clear(0x4000)` (the colour
   bit only: the whole viewport's colour buffer is cleared to black, depth and stencil are not touched; renderer-driver.md
   §5.3) *[compat]*.
3. With the logo window, `t` := the timer's elapsed ms:
   1. When the logo is visible (slot 71) and `t` > 5000: hide it, which fades it out.
   2. When `t` > 6000: `ChildDelete` the logo, release it, clear `+0xDC`, and post `0xEAAC1A96`.
   3. Countdown := 2.
4. With the splash window, while it is visible:
   1. When `t` > 2000: post `0xCAAC6A52`.
   2. Countdown := 2.

   This step repeats on every paint until the screen is destroyed, so `0xCAAC6A52` can be posted more than once
   *[quirk]*.
5. Countdown − 1; while it is not 0, `InvalidateSelf` (slot 91) asks for another paint.
6. True.

**User skip**:
- **`DoUserStop`** (`0x0079D400`): only while there is no logo window yet and `P` exists. When
  `P.StopMovie()` (`0x008103E0`) succeeds, post `0xEAAD620A`.
- **`GZOnMouseDownL`**: `DoUserStop`, then false.
- **`GZOnKeyDown(key)`**: `DoUserStop` for Esc (`0x1B`), then false.

So only the first movie can be skipped. A skip leaves for the application's splash screen and never shows the logo
or the second movie.

## 4. The player functions in this owner

`P` is a `cGZWin` with vtable `0x00ABF210`, plus `0x00ABF200` in a virtual base found through `P+0xD8`.
- **`SetNotification(target, message)`** (`0x0080EEF0`): `+0x178` := `target` (counted, the old one released);
  `+0x17C` := `message`. The player posts that message to the target when its movie ends.
- **`QueueMovie(&key, flags)`** (`0x008105A0`): false without the resource manager (framework service `0x056B906E`,
  IID `0x656B8EFC`). Otherwise:
  1. The manager's slot 37 `(key, &record, 0)` (`0x00972B78`): this opens the DB **record** of `key` and does not load a
     resource object. It finds the DB segment that holds the key (manager slot 32 `FindDBSegment`), opens the record in
     it (segment slot 21 `OpenRecord(key, &record, access)`) with access 1 (read) when the third argument is 0, or 3
     (read and write) otherwise, and releases the segment. It fails (false) when no segment holds the key or the open
     fails, and then so does `QueueMovie`.
  2. Query the record for the serial record `0xC56B8F08`.
  3. Create GZCOM class `0x0A32C0DA` (IID `0x8A32C0AC`) and give it the record (its slot 3). That class is
     `cGZDBSerialRecordAsIGZFile`, a serial record seen as a file with a position, a size and reads (record-as-file.md).
  4. Queue that object's slot 5 result, which is its `cIGZFile` interface (record-as-file.md §1), with `flags`
     (`0x008104F0`). The decoder then reads the movie from the package as an ordinary file.
  5. Release everything; true.
- **`Queue(object, flags)`** (`0x008104F0`): push `{flags, object}`, with a reference, at the back of a
  `std::deque` at `+0xF8`. The deque has 128-byte blocks of 16 pairs; the slow path `0x0080FF30` adds a block.
- **Deque helpers**:
  - `0x0080FD10` releases the objects of a range;
  - `0x0080FD50` pops the front;
  - `0x0080FDB0` destroys the deque.
- **Destructor** (`0x00810400`, deleting `0x00810580`):
  1. reset the vtables;
  2. call `0x008101B0`;
  3. free the strings at `+0x214` and `+0x208`;
  4. destroy the object at `+0x198` (`0x00811C10`: its critical section and its string);
  5. release `+0x190`;
  6. free the strings at `+0x180`, `+0x144`, `+0x138` and `+0x12C`;
  7. release `+0x120`;
  8. destroy the deque and the `+0xE0` member;
  9. `~cGZWin`.

| Case | Setup | Expected |
|---|---|---|
| IV-C1 | Esc during the first movie, `StopMovie` true | post `0xEAAD620A`; the splash screen; the logo never appears |
| IV-C2 | Esc while the logo shows | nothing |
| IV-C3 | timer at 5500 ms with the logo visible | the logo is hidden (fading); at 6001 ms it is deleted and `0xEAAC1A96` posted |
| IV-C4 | screen 1024 × 768 | `P` at `(128, 192)`, 768 × 384 |

## 5. Function inventory (owner `cSC4WinIntroVideoScreen`)

All 22 functions:
- §1: `0x0079CED0`, `0x0079CF20`, `0x0079D610`;
- §2: `0x0079CFE0`, `0x0079CF50`;
- §3: `0x0079D2F0`, `0x0079D110`, `0x0079D200`, `0x0079D490`, `0x0079D400`, `0x0079D460`, `0x0079D470`;
- §4: `0x0080EEF0`, `0x008105A0`, `0x008104F0`, `0x0080FF30`, `0x0080FD10`, `0x0080FD50`, `0x0080FDB0`,
  `0x00810400`, `0x00810580`, `0x00811C10`.

Not covered:
- the MAD decoder and the rest of `cSC4MadCowPlayerWin` (its own owner).

The questions IV1 (the manager's slot 37 and class `0x0A32C0DA`, §4) and IV2 (the clear, §3) are answered and archived
in [questions/answered/intro-video.md](questions/answered/intro-video.md).
