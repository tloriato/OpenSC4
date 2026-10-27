# `cSC4NewsTickerWinProc`: the news ticker (behaviour spec)

Scope: the whole `cSC4NewsTickerWinProc` owner as tools/coverage.py assigns it (16 functions, 7,764 bytes on
`5c0ff782`; inventory in §8). The 3D view makes it at `+0x2DC` and gives it its script window (view3d-window.md
§5 step 4). It shows the advisors' advices in two places:
- a bar of three flipping lines;
- an expandable list built like the advisor window's (advisor-window.md §5.2).

Evidence: static RE only, on the instruction and data bytes of the Steam 1.1.641 exe (the same code as GOG,
`docs/lanes/re-dates.md`). Every function was disassembled. The HTML strings are statics built at start-up
(`0x00A76300`–`0x00A76A40`) and were read from there. Written 2026-10-23 (re-ticker7).

Conventions:
- `T` is the ticker.
- `P` is the parent of its window (`T+0x44` slot 3, then slot 11).
- `SetFlag(1, on)` shows or hides a window.
- An advice's slot 6 is its mood, slot 7 its priority, slot 11 its advisor ID, and slot 15
  `CheckFlags`. *[compat]* Slot 7 reads the priority loaded from the advice's `priority` script
  property (`0x00407AB0`, `0x00408C70`, property read `0x00408D1B`–`0x00408D38`), confirming
  the meaning used for ordering and the flipper threshold (NTK1).

## 1. Identity and fields *[compat]*

| Offset | vtable | Slots |
|---|---|---|
| `+0x00` | `0x00AB5164` (message target) | 0–2 shared; 3 `DoMessage` (`0x00772B60`); 4 deleting destructor (`0x00773180`); 5 `Show` (`0x00772920`); 6 `Hide` (`0x00772A00`); 7 `RemoveAdvice` (`0x00772E90`); 8 `InsertAdvice` (`0x00772F30`); 9 `updateBodyTextCtrl` (`0x007731A0`); 10 `DoFlipperMessage` (`0x00774070`); 11 `PurgeInactive` (`0x00772EE0`) |
| `+0x04` | `0x00AB5150` (window proc) | 0–2 shared; 3 `DoWinProcMessage` (`0x00772810`); 4 `DoWinMsg` (shared) |

| Offset | Field |
|---|---|
| `+0x0C` | byte: initialised |
| `+0x0D` | byte: subscribed |
| `+0x10` | a `cRZCallbackTimer` (slots 3 restart(ms), 5 start(ms, …), 7 stop, 8 running, 9 set callback) |
| `+0x38` | the list control (child `0x6A231531` of the expanded panel, IID `0xCA1492A2`) |
| `+0x3C` | the flipper control (child `0xAA12F33C` of the bar) |
| `+0x40` | list of advices (12-byte nodes: next, previous, advice), highest priority first |
| `+0x44` | the script window's `cIGZWinGen` |
| `+0x48` | byte, 1; not read here |
| `+0x49` | byte: the list is expanded |
| `+0x4C`…`+0x54` | the three advices on the flipper, oldest first (counted) |
| `+0x5C` | the scroll offset of the flipper, in pixels |
| `+0x64` | the flipper control's rest rectangle |
| `+0x74` | the number of advices on the flipper (a working value) |
| `+0x78` | 1 when the flipper rotated |
| `+0x7C` | 1 when the flipper must be redrawn |
| `+0x80` | the line height |

The constructor (`0x00773050`) zeroes the fields, makes the timer and the list sentinel, and sets `+0x48` and
`+0x7C` to 1. The destructor (`0x00773100`) releases:
1. the three flipper advices and the window;
2. the list;
3. both controls;
4. then the timer.

## 2. `Init(gen)` (`0x007724C0`, RET 4) *[compat]*

Once:
1. `+0x44` := `gen` (counted). `gen` slot 7 `(T+4)` makes `T` the window's proc. `gen` slot 11 `(&{0, 0, 0, 0})`.
   `Hide`.
2. `P`'s children `0xCA2AEDCC`, `0xCA2AEDCD` and `0x4A231510` get the window as notification target (slot 86).
3. The list control is fetched from the expanded panel `0xAA231508`. The flipper control is fetched from the bar
   `0xCA2AEDC0` (slot 37 with IID `0xCA1492A2`).
4. The flipper control gets slot 10 and slot 8 `(0, 0, 0, 0)`. Its window's area (slot 48) becomes the rest
   rectangle `+0x64`.
5. The expanded panel's `cIGZWinGen` gets slot 11 `(&{0, 0, 0, 0})`.
6. The timer's callback is `0x00772440` with `T` (slot 9).
7. `+0x80` := font `0xAA0F4AB4`'s line height (`RZ::WCM` slot 5, then the font's slot 35). The rest rectangle's
   `y1` := its `y0` + 3 lines.
8. When `P` has child `0xCA2AEEC0`, it is resized (slot 53) to its width and min(2 lines, its height).
9. True.

**The timer callback** (`0x00772440`, cdecl): it gets message server 2 (framework service `0x04FA845B`, IID
`0x652294C7`), makes a `cRZMessage2Standard` of type `0x0A344006`, and hands it with `T` to the server's slot 7.

*[compat]* Slot 7 queues delivery to that specific target. The concrete service interface
(base table `0x00AD1BF4`; application table `0x00A8CAFC`, installed by
`0x00472EB0`; query `0x0092D091`) reaches `0x0092EA00`, which retains a message/target
pair and appends it to the directed queue (`0x0092D1C5`, `0x0092E6E2`). The callback can
release its message reference immediately. The queue pump later removes the pair and calls
the target's message handler (`0x0092F90C`); this is deferred delivery, not a synchronous
handler call within the timer callback (NTK4).

## 3. Showing and messages *[compat]*

- **`Show`** (`0x00772920`):
  1. The bar `0xCA2AEDC0` and the expanded panel each get `SetFlag(0x800, 1)`, slot 23, then `SetFlag(0x800, 0)`;
     the bar is shown.
  2. When expanded, the panel is shown and the list rebuilt (slot 9).
  3. A stopped timer is started with 3000 ms (slot 5).
- **`Hide`** (`0x00772A00`): hide the panel and the bar; stop a running timer.
- **`RegisterMessageNotification(on)`** (`0x00772760`): `T` subscribes to (or unsubscribes from) `0x8A09FA65`,
  `0x8A09FA67`, `0x8A09FA64` and `0xCA09FA6C` (server `0x00B43CCC`). `+0x0D` follows `on` even without a server.
- **`DoWinProcMessage(w, m)`** (`0x00772810`, RET 8) always returns false, and does nothing unless `w` is its window:

| `m[0]` | Effect |
|---|---|
| `0xA2BF8AD5` / `0xA2BF8AD6` | `RegisterMessageNotification(1 / 0)` |
| 3, `m[1]` = `0x287259F6`, `m[2]` = `0x4A231510` | hide the expanded panel; collapsed |
| 3, `m[2]` = `0xCA2AEDCC` or `0xCA2AEDCD` | expanded: hide it and collapse; collapsed: show it, `PurgeInactive`, rebuild the list (slot 9), expanded |

- **`DoMessage(m)`** (`0x00772B60`) always returns true. `adv` is the message's object (slot 32) asked for
  `cISC4Advice` (`0x8A09F5F5`):

| Type | Effect |
|---|---|
| `0x8A09FA64` AdviceAdded | unless `adv` has flag `0x2000`: `+0x7C` := 1, `InsertAdvice(adv)`, rebuild the list |
| `0x8A09FA65` AdviceRemoved | rebuild the list; `+0x7C` := 1. The advice stays listed |
| `0xCA09FA6C` | `RemoveAdvice(adv)`, rebuild, `+0x7C` := 1 |
| `0x8A09FA67` AdviceRemovedAll | the object is an advisor (`0xCA09F544`). Every listed advice of its ID (advisor slot 5 against advice slot 11) is removed. If any was: rebuild, `+0x7C` := 1, `DoFlipperMessage` |
| `0x0A344006` (the timer) | §5.2 |

*[compat]* Flag `0x2000` is set by `cSC4CitySituationAdvice::Init` after the base advice
initialisation (`0x004239D0`). This identifies city-situation advice as a concrete excluded
category, regardless of priority: the ticker tests the flag before inserting an added advice
(`0x00772B60`). The flag is also publicly mutable through advice slot 14
(`0x00407AE0`), so exclusion is a flag test rather than a test of the advice's class (NTK2).

## 4. The list

- **`InsertAdvice(adv)`** (`0x00772F30`):
  - an advice already listed is not added again;
  - otherwise it goes before the first advice whose priority is not higher than its own, or at the end, counted.

  So the list runs from the highest priority down, a new advice before equal ones.
- **`RemoveAdvice(adv)`** (`0x00772E90`): the first node holding it is unlinked and released.
- **`PurgeInactive`** (`0x00772EE0`): every advice without flag 2 is removed.
- **`updateBodyTextCtrl`** (`0x007731A0`) follows advisor-window.md §5.2 with these differences:
  - the strings are the statics `0x00B4DD70`…`0x00B4DFB4`;
  - mood 0 uses plain red (`#b00000`) and the normal title close, with no blinking;
  - mood-7 rows are `(a, a, &title, &title, 0, adv, &d, 1, 0x14416264)`: no body, the title twice, and 1 as the
    eighth argument;
  - the end is the control's slot 5 `(1)`, with no children disabled.

  The other moods, the greying without flag 2, the icons and the normal row
  `(c, b, &body, &title, CheckFlags(0x20), adv, &d, 0, 0x14416264)` are the same.

## 5. The flipper

### 5.1 `DoFlipperMessage` (`0x00774070`)

Nothing without a flipper control. Otherwise:
1. The control is cleared (slot 7) and gets slot 6 `(0x14416265)`. `+0x74`, `+0x78` := 0.
2. `n` = the number of advices on the flipper. `last` = the newest one and `p` its priority.
3. **Eligible advices** have flag 2 and priority ≥ `t`:
   - `t` = 59 when some listed advice has flag 2 and priority > 59;
   - otherwise `t` = 0.
4. **The next advice** is the first eligible advice other than `last`, searched in two passes:
   1. after `last` in list order (skipped when `t` is 59 and `p` < 59);
   2. then from the list's head. Meeting `last` in this pass marks the search as wrapped, and the search goes on.
5. **Found an advice `x`:**
   1. When all three places are full, the advices shift one place towards the oldest (`+0x78` := 1) and `n` goes
      down by 1.
   2. `x` goes in place `n`.
6. **Nothing found:** with two or more advices, or when the search did not wrap, they shift one place (`+0x78` :=
   1). The third place is then cleared. So a single advice whose search did not wrap is dropped.
7. The three places are drawn in order with `DisplayNewsFlipperLine` (null for an empty place).
8. When `+0x7C` is set, the control's slot 5 `(1)` runs and `+0x7C` := 0.

### 5.2 The tick (`0x0A344006` in `DoMessage`)

- **Without a flipper control:** the timer stops.
- **Offset 0:**
  1. The control's window takes the rest rectangle (slot 54). `+0x7C` := 1. `DoFlipperMessage`.
  2. Without a rotation the offset becomes one line height; with a rotation it becomes 1.
  3. The timer restarts with 3000 ms.
- **Otherwise:**
  1. The window takes the rest rectangle moved up by the offset.
  2. Below one line height the offset grows by 1 and the timer restarts with 40 ms. At one line height it goes
     back to 0 with 3000 ms.

  So after a rotation, and a 3 s wait, the lines scroll up by one line at one pixel per 40 ms, then rest 3 s.
  Without a rotation the next tick moves them up by a whole line at once for 3 s *[quirk]*.

### 5.3 `DisplayNewsFlipperLine(adv)` (`0x00773980`, RET 4)

1. **The colour and close**, by mood:
   - 0: blinking red with `</B></FONT></BLINK>`;
   - 1: red;
   - 5: green;
   - other: dark blue.

   Each uses `</B></FONT>` otherwise. Without an advice: dark blue.
2. **The face** is ` FACE="StocletITC TT" SIZE=2><B>` when the language (`RZ::LM` slot 7) is 15, else the `SIZE=3`
   one. *[compat]* Language 15 is Japanese: the language manager maps ID `0x0F` to
   `Japanese` (`0x00924798`), confirming this branch in `0x00773980` (NTK3).
3. **The line** = colour + face + the title (advice slot 18, or `" "` without an advice) + close.
4. The flipper control's slot 4 gets `(&e, &e, &line, &line, 0, adv, &e, 0, 0x14416265)`, with `e` empty strings.

## 6. Quirks and cases

Quirks:
- `AdviceRemoved` leaves the advice in the list; only `0xCA09FA6C` and `AdviceRemovedAll` take advices out.
- Mood-7 list rows repeat the title in the body place.
- In `DoFlipperMessage` the append for an empty flipper inside the after-`last` search cannot be reached; an empty
  flipper always takes the found advice in the general case.
- Without a rotation the flipper jumps up a line for 3 s.

| Case | Setup | Expected |
|---|---|---|
| NT-C1 | list with priorities 80, 40, 70 (flag 2) arriving in that order | list order 80, 70, 40; `t` = 59, so only 80 and 70 are eligible |
| NT-C2 | flipper `[A, B, C]`, next eligible `D` | `[B, C, D]`, `+0x78` = 1, then the 40 ms scroll |
| NT-C3 | line height 14, rest `y0` 100, a rotation | rest `y1` 142; 3 s, then offsets 1…14 applied 40 ms apart, 3 s at 14, then back to the rest rectangle with the next flip |
| NT-C4 | mood 0 advice on the flipper | blinking red line with `</BLINK>` |

## 7. Not covered

- The list control and the flipper control are `cSC4WinAdviceList` controls, specified in advice-list.md.
- Advice flags `2` and `0x20` beyond their uses above (AW4); priority and the city-situation exclusion are established here (NTK1, NTK2).

## 8. Function inventory (owner `cSC4NewsTickerWinProc`)

All 16 functions (7,764 bytes):
- §1: `0x00773050`, `0x00773100`, `0x00773180`;
- §2: `0x007724C0`, `0x00772440`;
- §3: `0x00772920`, `0x00772A00`, `0x00772760`, `0x00772810`, `0x00772B60`;
- §4: `0x00772F30`, `0x00772E90`, `0x00772EE0`, `0x007731A0`;
- §5: `0x00774070`, `0x00773980`.
