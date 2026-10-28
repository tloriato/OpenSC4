# Audio ambience: the ambience manager (behaviour spec)

Status: behaviour spec, written 2026-10-21 (re-audio4 lane). Every fact was checked on the Windows executable
`bin/SimCity4.exe` (1.1.641, image base `0x00400000`); every address is a Windows address. Numeric details
were checked against the instructions (`tools/re_query.py asm`). The REA decompiler session was unavailable
for this lane ("no app open"), so the evidence is the Ghidra export and its instruction listings only.
Implementers work from this spec, the public gzcom-dll SDK headers and general knowledge only; they must
**not** consult decompiled or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Tags as in [audio.md](audio.md): *[compat]* observable, *[choice]* FreeCity4 decision, *[inferred]*
consistent with the evidence but not proven, *[quirk]* reproduced oddity, *[unsettled]* open (§9).

Scope. **`cSC4AudioAmbienceManager`** (CLSID `0x6A12F51C`, factory `0x004EB0C0`, service
`kGZGUID_AudioAmbienceManagerSystemServiceId` `0xAA12F911`; [audio.md](audio.md) §1.2, §1.3): the city-side
service that turns occupants into positional sounds, runs the occupant alarm clock, keeps the listener's
"attention rectangle", and drives the ambience layers once per tick. The layers themselves
(`cSC4AudioAmbienceLayer`) are specified here as the interface the manager calls (§8) plus their `Init`,
tuning and filter creation (§8.1–8.3); their score and loop algorithms (`_UpdateAmbienceScores` `0x004E5920`,
`_UpdateAmbienceLoops` `0x004E5D50`) are specified in §8.6–§8.7 (AA1, answered). The sound-system events the
manager posts are the ones `cSC4AudioSoundManager::DoEvent` handles ([audio.md](audio.md) §8.3).

---

## 1. Object

The object is `0x120` bytes. Four interface vtables at `+0x00` (main: `IsServiceRunning`, `Init`,
`Shutdown`, `OnTick`, `DoEvent`), `+0x04` (the `cIGZMessageTarget2` the message server delivers to;
`DoMessage` `0x004EAF20` is reached through it), `+0x08` and `+0x0C` (the sound-system event target
registered with `cIGZSndSys`). Constructor `0x004EA390`, destructor `0x004EA670`.

| Offset | Field | Initial | Meaning |
|---|---|---|---|
| `+0x10` | ref count | 0 | |
| `+0x14` | service id | `0xAA12F911` | |
| `+0x18` / `+0x19` | bytes | 0 / 1 | service-running flags *[inferred]* |
| `+0x1C` | audio scape manager | null | service `0x49E50BA1`, IID `0x29E39DB4` (`Init`) |
| `+0x20` | `cIGZSndSys*` | null | service `0x23CECBA0`, IID `0x106077D2`; every sound request goes here |
| `+0x24` | occupant manager | null | copied from the global `0x00B43D0C` at city init |
| `+0x28` | occupant filter | null | an 8-byte filter (vtable `0x00A93640`) made at city init; passed to the per-cell occupant walk |
| `+0x2C` | layer list | empty | intrusive circular list of ambience layers (node: next, prev, layer) |
| `+0x30` | zoom index | 0 | copied from the view-utilities global `0x00B43DD8` `+0x0C` |
| `+0x34..+0x3C` | listener position x, y, z | | f32, from the audio director's listener (§4) |
| `+0x40..+0x48` | listener vector | | f32, the listener's second vector (direction *[inferred]*) |
| `+0x4C` | alarm list | empty | circular list of alarm entries (§3) |
| `+0x50` | ticking enabled | 1 | byte; `OnTick` does nothing while 0 |
| `+0x54..+0x5C` | attention margins | empty | `std::vector<f32>` (5 values shipped), indexed by zoom (§4) |
| `+0x60..+0x68` | layer ids | empty | `std::vector<u32>`: the ambience-layer exemplar instances (§2.2) |
| `+0x6C..+0x78` | current cell rectangle | 0 | x0, z0, x1, z1 (even values, §4) |
| `+0x7C..+0x88` | previous cell rectangle | 0 | same layout |
| `+0x8C..+0xA8` | world rectangles | | the attention rectangle and the city bounds in world units *[inferred]* |
| `+0xAC` | city ready | 0 | set to 1 at city init |
| `+0xB0` | stage | 0 | `OnTick` state machine (§5) |
| `+0xB4..` | random iterator | | `RandomIterator` over 2×2-cell blocks (`+0xB4..+0xB6` bytes 1,1,1) |
| `+0xE8` | stage timer | | `cRZTimer`, unit 3 (microseconds) |
| `+0x100` | tick timer | | `cRZTimer`, unit 4 (milliseconds) |
| `+0x118` | tracked-occupant set | null | GZCOM object, CLSID `0x4A401BA7`, IID `0x4A40074C` (`Init`) |
| `+0x11C` | tracked count | 0 | s32; +1/−1 as occupants enter/leave the set |

## 2. Lifetime

### 2.1 `Init` (`0x004E84D0`)

In order; any failure returns 0 *[compat]*:

1. Resolve the framework (cached in the global `0x00B628AC`) and call its slot 3.
2. Get the audio scape manager service (`0x49E50BA1` / `0x29E39DB4`) into `+0x1C`. Absent → return 0.
3. Get `cIGZSndSys` (`0x23CECBA0` / `0x106077D2`) into `+0x20`. Absent → return 0. Register the `+0x0C`
   event target with it (sound-system slot `+0x3C`).
4. Get the message server (`0x4FA845B` / `0x652294C7`) and subscribe the `+0x04` target to these 12 types,
   in this order: `0x26D31EC1` PostCityInit, `0x26D31EC2` PreCityShutdown, `0x8A4A69C7` ListenerPosChanged,
   `0x99EF1142` InsertOccupant, `0x99EF1146` MoveOccupant, `0x99EF1143` RemoveOccupant, `0xE99B3D4D`
   ConditionChanged, `0xE8BDA2E7` StateChanged, `0x09D92FC3` AutomataEvent, `0xC53D10AA` (exemplar changed),
   `0x8A878385` AutomatonApproachingIntersection, `0x4348B111` AppGainLoseFocus. Unlike the sound manager
   ([audio.md](audio.md) §8.3), this service **does** subscribe `0xC53D10AA` *[compat]*.
5. Create the tracked-occupant set (GZCOM `0x4A401BA7` / `0x4A40074C`) into `+0x118`; on failure return 0.
6. Start the tick timer `+0x100`, set `+0x50 = 1`, return 1.

### 2.2 City init (`0x26D31EC1`, `_HandleInitMessage` `0x004EABF0`)

1. Read the tuning (§4.1).
2. Create one ambience layer per id in `+0x60` (`_CreateAmbienceLayersFromExemplarIds` `0x004E9C10`): allocate
   a `0x25C`-byte `cSC4AudioAmbienceLayer`, call its `Init` (slot 4). On failure log
   `"Error : cSC4AudioAmbienceManager::_CreateAmbienceLayers : Failed to init layer (%08x)\n"` and drop it.
   On success set its score master volume (slot 17) from the audio preferences (the object behind
   `0x00B43C94` slot `+0x98`): 0 when the preference byte `+4` is clear, the preference volume (the 32-bit
   value at `+8`, passed as is) otherwise; then append it to `+0x2C`. Neither the app global nor the slot
   `+0x98` result is null-checked (`0x004E9C7E`–`0x004E9C94`): with no preferences object the game would
   crash, so no volume is defined (AA6c); the app always exists while a city loads *[compat]*, and a
   replacement uses volume 0 *[choice]*. With the shipped tuning that is **94 layers** (audio.md §10.8).
3. Copy the occupant manager into `+0x24`. Ask the city (`0x00B43CEC`) for its cell counts (slots `+0x174`,
   `+0x178`) and convert cell (0, 0) and (n−1, n−1) to world corners through occupant-manager slot `+0x48`
   (stored at `+0x9C..+0xA8`). Set `+0xAC = 1`.
4. Recompute the attention rectangle (§4.2).
5. Make the occupant filter `+0x28`, reset the random iterator, set stage `+0xB0 = 0`.
6. Add itself to the framework's tick list (framework slot `+0x24`).
7. Walk the existing occupants (§3.1, `_IterateOccupantsThatNeedToBeTracked` `0x004EAAE0`). *[quirk]* The walk builds an
   occupant-type filter in a freshly allocated 0x18-byte object and does not test the allocation: a failed allocation
   continues into reads through a null filter pointer (`0x004EAAF6`–`0x004EAB20`), so the original has no defined
   out-of-memory result here.
8. `+0x50 = 1`; return 1.

### 2.3 City shutdown (`0x26D31EC2`, `0x004E9810`)

`+0x50 = 0`, remove itself from the framework's tick list (slot `+0x28`), then release every layer and empty
the lists (`0x004E9660`). Returns that result.

`0x004E9660` (AA9c) *[compat]*: for each layer node in list order, the layer's slot 5 (`Shutdown`, `+0x14`),
then its `Release` (`+0x08`), then unlink and free the node; return **1**. Only the **layer list** is emptied:
the alarm list `+0x4C`, the tracked-occupant set and its count are left as they are (DIFFERS from the engine,
which also empties the alarms). Stale alarm entries therefore survive into the next city, where
`_UpdateAlarms` still visits them unless remove messages dropped them first *[quirk; a replacement should
drop alarms at city shutdown — choice]*.

### 2.4 `Shutdown` (`0x004E96A0`)

Unregister the event target from `cIGZSndSys` (slot `+0x40`), unsubscribe the 12 message types of §2.1 (the
order differs, the set is the same), release the layers (`0x004E9660`) and the held services.

### 2.5 Native first-tick prerequisite: a live city-screen view (EF28) *[compat]*

Added 2026-10-27 (re-ef32). `Init` starts the timer and sets enabled, but city
PostCityInit is the boundary that adds this service to the framework's tick
list (§2.2). PreCityShutdown synchronously clears enabled and removes it
(§2.3). The application creates the city screen **before** PreCityInit, city
Init and queued PostCityInit; on an ordinary close it sends PreCityShutdown
**before** destroying that screen (application §§11.4,11.7–11.9).

This ordering supplies a live `cISC4View3DWin` for the native ambience
filters. It is a caller/lifecycle precondition, not a view lookup or readiness
handshake in the audio service. The view-level-1 global filter `004E2C00`
reads global `00B43DC4`, dereferences it at `004E2C06`, and calls its slot 22
at `004E2C08`; it returns whether the result equals 1. There is no null check.
The level-2/level-3 and zoom filter families also require the city view
(audio-filters §2). OnTick's enabled check (`004EA156–004EA15B`) is not a view
check. Its stage-2 cell processing reaches these filters through §8.4.

A view-less service can exist before city initialization or after city
shutdown without being scheduled for city ambience. A **view-less active city
tick with one of these filters is not supported** by the original; neither
PostCityInit receipt alone nor the presence of a region renderer/effects
manager makes it valid. A replacement must preserve the lifecycle ordering
when retaining native audio. This specifies no null-global patch or timing
delay.

Worked case: publish city view V, deliver PostCityInit, then tick a layer using
the level-1 filter. With V slot 22 returning 1, CanHandle accepts that global
predicate. Send PreCityShutdown: enabled becomes 0, tick-list membership is
removed, and layers shut down. Only then shut down the city screen and clear
V. There must be no later city-filter evaluation against that cleared view.
Manually calling the filter while V is absent violates its prerequisite and
faults at `004E2C06`; it does not return false. The trace needed to classify
the reported intermittent violation is effects §18.11.3.

## 3. Occupant sounds and alarms

Every occupant handler first skips **preview occupants**: an occupant whose property holder has `0x029244DB`
set to true is ignored (foundations §5.2) *[compat]*. The occupant's property holder is its slot `+0x18`.

Sound requests go to `cIGZSndSys` slot `+0x2C` as sound-system events ([audio.md](audio.md) §8.3):
`0xEA451903` start, `0x6A451908` stop, `0xCA45190F` set position. The **instance id** of an occupant sound is
the occupant's address plus the sound id *[compat for uniqueness; the value itself is internal]*. Positions
are the occupant's position (slot `+0x20`) truncated to integers. Exceptions (AA8): the move set-positions of §3.2
use the bare occupant address, the railway crossing start of §3.5 uses the automaton's address, and one-shot
alarms use a fresh id (§3.6).

`_AddOccupantSndInstance(o, id, loop)` (`0x004E8AD0`, pop 12) in full *[compat]*: `o` null, or `o` lacking
**both** occupant flags 1 and 4 (`IsFlagSet(5)`, slot `+0x80`; flag 1 = in the occupant manager, 4 = visible,
lot-records.md §2), or a preview occupant (`0x029244DB` true) → return 0, nothing posted (`0x004E8AD8`–
`0x004E8B1D`). Otherwise: if `o` answers the automaton IID `0xA9B40F05`, call its slot `+0x9C` with the
manager's `+0x04` interface (the link), releasing the reference afterwards; then, only when `cIGZSndSys` is
present, post start `(0xEA451903, id, o + id, loop ≠ 0, 0)` and set-position `(0xCA45190F, o + id, x, y, z)`;
return 1. `_RemoveOccupantSndInstance(o, id)` (`0x004E8BF0`): `o` null → 0; automaton → slot `+0xA0` with the
same interface; stop `(0x6A451908, o + id, 0, 0, 0)` when the sound system is present; return 1.

| Property | Meaning to this manager |
|---|---|
| `0x6A36AFAC` | ambient sound id: a looping sound started at insert, moved on move, stopped on remove |
| `0xC9B93A56` | a second sound id started at insert (non-looping flag) |
| `0x6A36AFAB` | alarm time list (f32 hours); with `0x0A36AFA2` → one-shot alarm, with `0x2A53BE13` → on/off alarm |
| `0x0A36AFA2` | one-shot alarm sound id |
| `0x2A53BE13` | on/off alarm sound id |
| `0xCA19D7CA`, `0x0A4C13CB`, `0x4A4C132E` | condition-dependent sounds: any of them triggers the lot condition update (§3.4) |
| `0xAA87817A` | automaton crossing sound id (§3.5) |

### 3.1 Insert (`0x99EF1142`, `HandleInsertOccupant` `0x004EA7D0`) and init walk (`HandleInitOccupant` `0x004EA970`)

1. Add the occupant to the tracked set `+0x118` (its slot `+0x14`); if it was added, `+0x11C += 1`.
2. Insert only: if it has `0x6A36AFAC`, start that sound looping (`_AddOccupantSndInstance` `0x004E8AD0`).
3. If it has `0x6A36AFAB`: register a one-shot alarm when `0x0A36AFA2` is present and an on/off alarm when
   `0x2A53BE13` is present (`_AddAlarmOccupant` `0x004EA690`; both may be registered).
4. Insert only: if it has `0xC9B93A56`, start that sound once: `_AddOccupantSndInstance(o, value, loop = 0)`
   with instance `o + value`, plus the set-position and automaton link (`0x004EA8D5`–`0x004EA8F0`) (AA8a).
5. If it has any of `0xCA19D7CA`, `0x0A4C13CB`, `0x4A4C132E`: find its lot (lot manager `0x00B43D08` slot
   `+0x38`) and run the condition update for that lot (§3.4).

The init walk (city init step 7) visits every occupant of types `0x278128A0` (buildings), `0xA823821E` (props, as in city-detail.md) and
`0xC7CC75F1` (vehicles, AA4: the `cSC4Vehicle` base constructor `0x005628F0` installs vtable `0x00A9D488`, whose
`QueryInterface` `0x0055B660` answers the vehicle IID `0x896E714A`, and writes the type at `+0x28` (`0x005629D2`); all
ten derived vehicle constructors (`0x005148E0` … `0x00562C10`, e.g. trailer `0x00556DB0`, train car `0x0055B150`) call it
and none rewrites `+0x28`, so every vehicle subclass carries this one type; `0x004C3AB0` accepts it beside the sim
type `0x496E5783`) (occupant-manager slot `+0xB0` with a type filter) and applies the init variant, which starts
**no** sound itself *[compat]*.

`_AddOccupantSndInstance` also links an automaton occupant (IID `0xA9B40F05`) back to this manager (its
slot `+0x9C`); the start event carries the instance id, the loop flag and the position, and is followed by a
set-position event.

### 3.2 Move (`0x99EF1146`, `HandleMoveOccupant` `0x004E8D40`)

Non-preview occupant with a property holder: if the holder **has** `0x6A36AFAC` (holder slot `+0xC`, a
presence test; the value is not read) and the sound system is present, post set-position
`(0xCA45190F, o, x, y, z)`. If `0xCA19D7CA` reads, also post set-position `(0xCA45190F, o, x, y, z)`
(`0x004E8C70`, pop 4: its one argument is the instance id, here the occupant). The instance id in both is the
**bare occupant address** (`0x004E8DCE`, `0x004E8DFD` → `0x004E8CB2`), not `o + id` as at start (AA8b, DIFFERS
from the engine's `o + id`): unless the id is 0 the sound manager does not find the instance and ignores the
event, so a moved occupant's sounds stay at the start position *[quirk]*.

**Branch nesting (AA11).** The two posts are independent. The `0x6A36AFAC` presence test (`0x004E8D95`) and the
sound-system test (`+0x20`, `0x004E8D9C`) both branch, when they fail, to the same join `0x004E8DDE`, which is
the `0xCA19D7CA` read; so the second path is reached whether or not the holder has `0x6A36AFAC` and whether or
not the sound system exists. The `0xCA19D7CA` read clears its value buffer first and only the read's success
counts (`0x004E8DE9`–`0x004E8DFB`): a stored value of 0 still enters `0x004E8C70`. `0x004E8C70` (only caller is
this handler) tests the sound system `+0x20` itself (`0x004E8C76`) and, when absent, posts nothing and returns 1.
A missing property holder ends the handler before either test (`0x004E8D85`). The first post precedes the second.

| holder has `0x6A36AFAC` | `0xCA19D7CA` reads | sound system | posts (all `(0xCA45190F, o, x, y, z)`) |
|---|---|---|---|
| yes | no | present | 1 (first path) |
| no | yes (any value, 0 included) | present | 1 (second path, via `0x004E8C70`) |
| yes | yes | present | 2, identical, first path then second |
| yes / no | yes / no | absent | 0 (`0x004E8C70` is still entered when `0xCA19D7CA` reads) |
| no | no | present | 0 |

### 3.3 Remove (`0x99EF1143`, `HandleRemoveOccupant` `0x004EA270`)

Remove from the tracked set (`+0x11C −= 1` when it was present). Then for each of `0x0A4C13CB`,
`0xCA19D7CA`, `0x6A36AFAC` present: stop the sound (`_RemoveOccupantSndInstance` `0x004E8BF0`, which also
unlinks an automaton through its slot `+0xA0`); for each of `0x0A36AFA2`, `0x2A53BE13` present: remove the
alarm entry (`_RemoveAlarmOccupant` `0x004EA080`). The stop is posted even when nothing was started
*[compat]*; the sound manager ignores unknown instance ids (audio.md §8.3).

*[compat]* **Alarm removal match rule** (`_RemoveAlarmOccupant(o, id)`, `0x004EA080`, one call per present property,
`0x004EA35E` and `0x004EA382`, with `id` = the property value read *now*): a null occupant returns false. Otherwise
the alarm list is walked from the head and the **first entry whose occupant is `o`** is taken — the entry's saved
sound id and its kind byte are **not** compared. If that entry's "on" byte is set the sound with the *current* `id`
is stopped (`_RemoveOccupantSndInstance`) and the byte cleared; then the entry releases its occupant reference, its
time list and itself are freed and it is unlinked; true. Nothing else is touched: only one entry per call goes, no match
returns false, and an occupant holding both properties makes two calls that remove its first two entries in list
order whatever their kinds (so with duplicates or a changed property the entry removed need not be the one
that property created, and a sound started under an old id is not stopped, because the stop uses the
current id) (`0x004EA08D`–`0x004EA0EE`).

### 3.4 Condition and state changes

`0xE99B3D4D` ConditionChanged: if the message's object (void1) answers the lot IID `0xE818FCED`, run
`_UpdateLotOccupantsForConditionChange` (`0x004E9DA0`) for that lot with `old := data1` (message slot `+0x24`,
reads `+0x10`) and `new := data2` (slot `+0x38`, reads `+0x14`) (`0x004EAEC5`–`0x004EAEDE`). The poster is
`cSC4Lot::SetCondition` (`0x006C26D0`): void1 is a **copy** of the lot, data1 the previous condition byte,
data2 the new one, data3 the lot's `+0x3C` (unused here); it is posted only when the condition changes.
`0xE8BDA2E7` StateChanged is subscribed and routed to the occupant handler but has no case there: it does
nothing *[quirk]*.

**The per-lot update `U(lot, old, new)`** (`0x004E9DA0`, pop 12). Insert and the init walk (§3.1 step 5)
call it as `U(lot, −1, −1)`.

1. `lot` null → return. If `new < 0` (signed), `new := lot.GetCondition()` (lot slot `+0x8C`, byte
   zero-extended) (`0x004E9DBB`).
2. `L := lot.GetLotOccupants()` (lot slot `+0xC4`, `0x006BFF70`: the occupant manager's occupants inside the
   lot's cell rectangle, any type, through an inside-lot filter) into a fresh list.
3. For each occupant `o` of `L`, in list order: `o` qualifies when it answers `cISC4BuildingOccupant`
   (`0x87DDA3A9`) **or**, failing that, `cISC4PropOccupant` (`0xE9793A65`), and its property holder (slot
   `+0x18`) is non-null. Both interface references are released after the occupant (they gate only). No
   preview check is made here *[compat]*. For a qualifying `o`, with `A := 0x0A4C13CB`, `C := 0xCA19D7CA`,
   `R := 0x4A4C132E` (each read as u32, `0x005FD480`; "absent" means the read fails), apply in this order:

| `new` | step (only for properties present) |
|---|---|
| 0, 5 or 6 | stop `A`; start `C` looping |
| 2 or 3 | stop `C`; start `A` looping |
| 1 or 4 | stop `A`; stop `C` |
| 0, and `old` = 1 | additionally start `R` looping (after the row above) |
| any other value (7 and up) | nothing |

   "Start" is `_AddOccupantSndInstance(o, id, loop = 1)` (start event, set-position, automaton link, §3.1)
   and "stop" is `_RemoveOccupantSndInstance(o, id)` (§3.3); the instance id is `o` + the property value. No
   test is made whether the sound is already running: a repeated condition 0 re-posts the `C` start
   *[compat]*.
4. Release the list through the occupant manager (`0x00B43D0C` slot `+0xC0`, `ReleaseOccupantList`), then
   free its nodes and head.

Consequences *[compat]*: `R` (the "repowered" sound, by its trigger) starts only on a real condition change 1 →
0, never on insert or init (old = −1); it is never stopped by this function, only by `HandleRemoveOccupant`
— which stops `A`, `C` and `0x6A36AFAC` but **not** `R` (§3.3) *[quirk]*. The lot condition values are those
of lot-developer.md §5.5 (0 normal, 1 unpowered, 2–3 run down, abandoned lots step up to 4); 5 and 6 share
row 0 *[compat; no producer of 5 or 6 is specified]*.

Worked example: a lot with one building carrying `C = 0x1111` and `A = 0x2222`. Insert (lot condition 0):
start `0x1111` looping, instance `o + 0x1111`. Power is lost (0 → 1): stop `o + 0x2222` (not running; the
sound manager ignores it), stop `o + 0x1111`. Run down (1 → 2): stop `o + 0x1111`, start `o + 0x2222`.

### 3.5 Automata

`0x09D92FC3` (`_HandleAutomatonMessage` `0x004E8820`) reads the automaton event code (message slot `+0x38`):

- code **9**: with the sound system present, `A :=` message slot `+0x28` (the automaton), `o := A` slot
  `+0x14` (its occupant); if `o`'s holder reads `0x6A36AFAC` = `id`, post set-position
  `(0xCA45190F, o + id, x, y, z)` with the **automaton's** position (`A` slot `+0x1C`, truncated)
  (`0x004E88EA`–`0x004E897E`);
- codes **12** and **13**: if the automaton's kind (message slot `+0x4C`) is `0x10`, `0x14` or `0x1E`, play
  the sound of the occupant `A + 4` (the automaton's occupant base, AddRef'd and released around the call)
  (`_PlayOccupantSound` `0x004E8720`): **start** when the code is 13 and, for kind `0x10`, the automaton's
  slot `+0xE4(0x10)` is 0; otherwise **stop** it. Other codes: nothing.

`_PlayOccupantSound(o, start)`: `o` null, sound system absent, no property holder, or `0x6A36AFAC` not read →
nothing (AA8c). Stop: `(0x6A451908, o + id, 0, 0, 0)`. Start: `(0xEA451903, id, o + id, 1, 0)` — the flag
is **1**, the same argument `_AddOccupantSndInstance` fills from `loop` (`0x004E87C6`–`0x004E87D4`; DIFFERS from
the "non-looping" of earlier drafts) — then set-position `(0xCA45190F, o + id, x, y, z)` with `o`'s position.
No automaton link, no flag or preview test here: the automaton handlers make **no** preview check (AA8f)
*[compat]*.

`0x8A878385` (`_HandleRailwayCrossingMessage` `0x004E8990`): sound system absent → return 1. The message type
must be `0x8A878385` (else return 0; unreachable through `DoMessage`). Its object (message slot `+0x80`)
must answer the automaton IID `0xA9B40F05` (reference `A`) and `A`'s occupant (slot `+0x14`) must read
`0xAA87817A` = `id`; then post start `(0xEA451903, id, A, 1, 0)` and set-position `(0xCA45190F, A, x, y, z)` at
`A`'s position (slot `+0x1C`). The instance id is the **automaton interface address `A` itself**, not
`occupant + id`, and the flag is 1 (`0x004E8A5B`–`0x004E8AA5`) (AA8e, DIFFERS). Nothing here or in
`HandleRemoveOccupant` stops it. Returns 1.

### 3.6 The alarm clock (`_UpdateAlarms` `0x004E8EA0`, every tick)

The clock is the simulation clock global `0x00B43DBC` (absent → no alarms). Time of day
`t = float(GetHour()) + float(GetMinute() / 60)`, where the division is an **unsigned integer** division
(`0x004E8EDB`): with minutes 0–59 the fraction is always 0, so alarms fire on whole hours only *[quirk]*.

Each alarm entry holds the occupant, a sound id, the last time seen (`+0x10`, f32), the time list
(`+0x14` count, `+0x18` data, f32 hours), an "on" byte `+0x24` and the kind byte `+0x25` (0 one-shot, 1 on/off).
For each time `a[i]` (with `next` = `a[i+1]`, or `a[0]` for the last entry):

- **one-shot**: if `last < a[i] <= t`, start the sound once with a fresh instance id
  (`cSC4AudioSoundManager::GenerateNewInstanceId`, audio.md §8.4) through the sound system's extended post
  (slot `+0x34`, tag `0x5B523BA4`), then post set-position the same way;
- **on/off**: the times alternate on, off, on, … (even index = on). If `t` lies in `[a[i], next)` — or the
  interval wraps midnight (`next < a[i]` and `t >= a[i]`, or `t − 24 >= a[i]` and `t − 24 < next`) — read
  `0x2A53BE13` afresh from the occupant's property holder (`v`) and: on an "on" index, if `+0x24` is 0,
  `_AddOccupantSndInstance(o, v, loop = 1)` (instance `o + v`, §3) and set `+0x24 := 1` whatever that call
  returned; on an "off" index, if `+0x24` is set, `_RemoveOccupantSndInstance(o, v)` and clear it. No
  property holder, or `0x2A53BE13` no longer reads → nothing, `+0x24` unchanged (AA8d) *[compat]*.

**Creating an entry** (`_AddAlarmOccupant(o, id, &times, kind)`, `0x004EA690`, pop 16) *[compat]* (AA5):
`o` null, or not `IsFlagSet(5)` (flags 1 and 4 both set), or `IsFlagSet(0x10)` → return 0, no entry
(`0x004EA69D`–`0x004EA6CE`). Otherwise read the clock (the global is **not** null-checked before use: absent
clock → crash; AA10, native detail: `o` is AddRef'd first (`0x004EA6D5`), the null test at `0x004EA6DE` only
skips the clock's AddRef, and the next instruction `0x004EA6E9` reads the clock's vtable through the null
pointer, a read access violation at address 0, before any entry is built; neither caller, Insert `0x004EA8A0`/
`0x004EA8CB` nor the init walk `0x004EA9F2`/`0x004EAA1D`, tests the clock first. What a replacement should do
instead is an open policy question, AA10, not settled by the binary) and build `{o, id, last, times (copied), on = 0, kind}` with
`last := float(GetHour()) + float(GetMinute() / 60)` — the same unsigned-integer formula as `t` below, so in
practice the whole hour (`0x004EA6ED`–`0x004EA786`). The entry is appended at the **tail** of the alarm list
`+0x4C`; return 1. Insert and the init walk pass `id` = the value just read of `0x0A36AFA2` (kind 0) or
`0x2A53BE13` (kind 1). Consequence: an alarm time equal to the current hour at creation does **not** fire on
the first tick (`last < a[i]` fails), and one-shot times earlier today wait for tomorrow. This DIFFERS from
the engine's `last := 0.0`.

Then `last := t`, a raw 32-bit copy of the f32 (`0x004E91B0`; no integer conversion) (stack trace: `t` is stored at `0x004E8F12` at depth 0x4C, i.e. frame slot
entry − 0x34; after `PUSH EBX` at `0x004E8F20` that slot is `[ESP+0x1C]`, which `0x004E91AC` reads; the
loop's `[ESP+0x18]` is the different slot entry − 0x38 holding `a[i]`). Because the "fire" tests are `last < a[i] <= t`, an alarm at time 0 never fires
after midnight (`last` = 23, `t` = 0) *[quirk]*.

## 4. Tuning and the attention rectangle

### 4.1 `_ReadTuningValues` (`0x004E91D0`; also on `0xC53D10AA`)

From the audio settings exemplar (`0x6534284A` / `0xCA25875D` / `0x6A359C5C`, audio.md §10.8): `0x0A36F410`
into the margins `+0x54` — when missing, all five margins are **70.0** (`0x428C0000`) *[compat]* — and
`0x2A3FF8CD` into the layer-id vector `+0x60`. Shipped margins: 0, 0, 188, 150, 73.

Both targets are `std::vector`s (margins `+0x54..+0x5C` begin/end/capacity of f32, ids `+0x60..+0x68` of u32),
empty with null storage after the constructor. The vector readers (`0x005FFCC0` f32, `0x005FF510` u32)
(AA6a/b) *[compat]*:

- property **absent** → return false, the vector is **left as it was** (not emptied);
- property present with `n` values → the vector becomes exactly `n` values (resized, raw 32-bit copy of
  the property data, then shrunk to fit), for any `n` including 0.

So a 3-value margin list leaves 3 margins (zoom 3 and 4 then read past the vector's end, undefined); the
70.0 fill is five raw stores through the vector's begin pointer **without resizing**: on a first read
(empty vector, null storage) it writes through a null pointer (the game would crash; the shipped exemplar
always has the property); on a later `0xC53D10AA` re-read it overwrites the first five floats of whatever
storage the previous read left. A missing `0x2A3FF8CD` keeps the previous ids (at the first city init: none,
so no layers). A replacement should define: margins absent → five 70.0 values; fewer than 5 → the missing
entries 70.0 *[choice]*; ids absent → unchanged *[compat]*.

### 4.2 `_UpdateAttentionRect` (`0x004E92A0`; city init and `0x8A4A69C7`)

1. Copy the listener's position and vector from the audio director (`GetSC4AudioCOMDirector` `+0x64`
   object, words 9–11 and 3–5) into `+0x34` and `+0x40`.
2. `zoom := *(0x00B43DD8 + 0x0C)`, stored in `+0x30`; `m := margins[zoom]` (no bounds check).
   Neither the view-utilities global nor the margin vector is checked (`0x004E9306`–`0x004E9318`: load the
   global, read `+0x0C`, read the f32 at `begin + 4·zoom`): with the global absent, an empty margin vector or
   a zoom outside the vector, the game dereferences null or reads foreign memory (AA6d). The game only runs
   this with a city view and zoom 0–4 *[compat]*; a replacement treats any other case as "keep the previous
   rectangle" *[choice]*.
3. World rectangle `(x − m, z − m) .. (x + m, z + m)` **intersected with the city's world bounds** (`+0x9C..+0xA8`,
   §2.2 step 3) and stored as four f32 at `+0x8C` (x0, z0, x1, z1; `0x004E9322`–`0x004E9351`, helper `0x004E8380`):
   each side is the larger of the two minima and the smaller of the two maxima; rectangles that merely touch still
   intersect; when they are disjoint all four stored floats become 0.0 and the clip reports failure, which the
   caller ignores. Both corners of the stored rectangle then go to cells through occupant-manager slot `+0x4C`.
   This stored f32 rectangle (not the cell rectangle) is what stage 0 hands to every layer's `BeginUpdate`
   (§5): the layer copies those 16 bytes verbatim and uses them as the world-space attention rectangle
   (`0x004EA1B5`–`0x004EA1F6`, `0x004E4CE0`); the even cell rectangle `+0x6C` only seeds the block iterator.
4. Clamp each cell coordinate to `[0, n − 1]` (the city's cell counts), then clear bit 0 (round down to even).
5. The first call after start-up (global byte `0x00B09824` set) writes the result to both rectangles and
   clears the byte; later calls move current → previous and store the new current.

## 5. `OnTick` (`0x004EA150`)

Does nothing while `+0x50` is 0. Otherwise `e :=` tick-timer elapsed (ms), restart it, run the alarm clock
(§3.6), then:

| Stage | Action | Next |
|---|---|---|
| 0, 1 | Seed the random iterator with the current rectangle halved (2×2-cell blocks); reset it; call each layer's `BeginUpdate` (slot 25) with the attention rectangle | 2, and fall through |
| 2 | `_DoStageUpdate(e)` (below); when it reports the walk finished | 3, and fall through |
| 3 | Each layer's `EndUpdate` (slot 27) | 1 |

`_DoStageUpdate` (`0x004E94E0`): restart the stage timer (µs). While its elapsed time is **at most 999 µs**
take the next random block `(bx, bz)`; walk the occupants of cells `2bx..2bx+1 × 2bz..2bz+1` through
occupant-manager slot `+0xBC` with the filter `+0x28` (callback `0x004E8E60`, which feeds each occupant to
the layers), then call each layer's `HandleCell` (slot 20) for the four cells in the order
`(2bx, 2bz)`, `(2bx+1, 2bz)`, `(2bx, 2bz+1)`, `(2bx+1, 2bz+1)`. When the iterator is exhausted, report
finished. In every case end with each layer's `Update(e, &listenerPos, &listenerVector)` (slot 26).
The time budget makes the walk length machine-dependent; a replacement may use any budget *[choice]* but
must visit every block of the rectangle before `EndUpdate` *[compat]*. The elapsed time is tested before
each block, so each call handles at least one block. The `HandleCell` loops are **cell-outer**: all layers
(list order) get cell 1, then all get cell 2, and so on (calls at `0x004E958F`, `0x004E95B0`, `0x004E95D1`, `0x004E95F6`).

**The block iterator** (AA7) *[compat]*. It is the shared `RandomIterator` of weather.md §6.7 at `+0xB4`
(`+0xB4` state, `+0xB5` mode, `+0xB6` dirty, `+0xB8..+0xC4` x0, z0, x1, z1, `+0xC8` count, `+0xD8` index,
`+0xDC` current, `+0xE0` step). The constructor sets state, mode and dirty to 1 — mode 1 is the **prime-step**
mode — `current := 0` and `step := 1`. Stage 0/1 calls `SetClipRect` (`0x0049C820`) with the current cell
rectangle halved by an arithmetic shift (`x0 >> 1, z0 >> 1, x1 >> 1, z1 >> 1`), bounds **inclusive**
(`SetClipRect` raises x1 to x0 and z1 to z0 when smaller), then `Reset(0)` (`0x0049D1C0`): no shuffle, so no
`rand()` call and `current` is **not** reseeded — it starts at 0 for the first walk and carries over from
the previous walk (and across cities: nothing else resets it). `GetNext` (`0x0049CB10`):
`current := (current + step) mod count`, block `x = x0 + current mod w`, `z = z0 + current div w` with
`w = x1 − x0 + 1`; the step comes from the prime table of weather.md §6.7.

Worked example: current rectangle cells (0, 0)–(8, 6) → blocks x 0..4, z 0..3, `w = 5`, count 20. Step: start
k = 0 (count < 29), advance past P = 1, 3 (< 20/4 = 5) to 5, then past 5 (20 mod 5 = 0) to **7**. First walk,
current from 0: 7, 14, 1, 8, 15, 2, 9, 16, 3, 10, 17, 4, 11, 18, 5, 12, 19, 6, 13, 0 — first block (2, 1),
i.e. cells (4..5, 2..3); last block (0, 0). The next walk of the same rectangle starts again from 0
(20 × 7 ≡ 0), so it repeats this order. The engine's row-major order (z outer) DIFFERS.

## 6. Events and focus

`DoEvent` (`0x004E9870`), the sound-system event target:

- `0x2A5B69C0` (master volume): every layer's `SetScoreMasterVolume` (slot 17) with the event's value.
- Any other event: the event's data object (slot `+0x18`, then `+0x80`) must answer the freshness-player IID
  `0xDC1CF595` (`cFreshPlayer`); then

| Event | Freshness-player call |
|---|---|
| `0x4A2C0389` | `Stop` (slot 10), then `Shutdown` (slot 7) |
| `0x0A414C5C` | `SetPlayPercent(value)` (slot 16) |
| `0x2A2C0384` | load the score named by the payload's type/group/instance (payload IID `0x65297976`); on failure log `"Error: cSC4AudioAmbienceLayer::DoEvent : Failed to load freshness score (%08x:%08x:%08x)\n"` |
| `0x2A2C0393` | `FadeVolume(3000, 0, 1)` (slot 12): a 3-second fade to 0 |
| `0x4A3DA208` | `SetVolume(value)` (slot 11) |
| `0xEA2C0390` | `SetHeadRelative(value != 0)` (slot 28), then `Play` (slot 8) |
| `0xEA2D827C` | `SetMinMaxDistance(...)` (slot 29) with the payload's two values |

Returns 1 when handled, 0 otherwise, with these details (AA9b) *[compat]*: a null event → 0; `0x2A5B69C0`
returns **0** after setting the volumes (`0x004E9BFE`–`0x004E9C04`); no data object or no freshness player →
0; an event id not in the table → 0. For `0x2A2C0384` the object QI'd for `0x65297976` is **the event
itself** (`0x004E997B`), and its slots `+0x24`, `+0x38`, `+0x4C` give the score key (type, group, instance)
passed to the player's slot 3; when the event does not answer the IID nothing is loaded or logged and the
result is still **1** (`0x004E9986` → `0x004E99FC` → `0x004E9A1F`). A failed load also returns 1.

`0x4348B111` AppGainLoseFocus (`0x004E8CF0`, pop 4) (AA9a): `f :=` the message's flag (slot `+0x24`). Stage
`+0xB0 := 1` when `f` ≠ 0 (focus gained), **4** when `f` = 0 (`0x004E8D02`–`0x004E8D11`); stage 4 has no
`OnTick` case, so the ambience walk stops while the app is out of focus and restarts at stage 1 (a fresh
`BeginUpdate`) when focus returns *[compat]*. Then every layer's slot 28 (`+0x70`, `Pause`) with `f = 0`
(pause when focus is lost). Returns **1**.

## 7. Message dispatch (`DoMessage` `0x004EAF20`)

The listener **produces** `0x8A4A69C7`; its camera inputs and notification threshold are
specified in [audio owner contracts §18](audio-owner-contracts.md#18-listener-camera-inputs-and-position-notifications).
Its void payload points into listener position state, so a consumer must not assume a copied
coordinate snapshot. This manager's payload consumption and attention rectangle remain §4.2.

The message must answer `0x65297976`; otherwise return 0.

| Type | Handler | Returns |
|---|---|---|
| `0x26D31EC1` | §2.2 | its result |
| `0x26D31EC2` | §2.3 | its result |
| `0x4348B111` | §6 | its result |
| `0x8A4A69C7` | §4.2 | 1 |
| `0xC53D10AA` | §4.1 | 1 |
| `0x09D92FC3` | §3.5 | 1 |
| `0x8A878385` | §3.5 | 1 |
| `0x99EF1142`, `0x99EF1143`, `0x99EF1146`, `0xE99B3D4D`, `0xE8BDA2E7` | §3 (`_HandleOccupantMessages` `0x004EAD30`; the occupant is QI'd with `0x26BB767E`) | 1 |
| other | — | 1 |

## 8. The ambience-layer interface (as called by the manager)

`cSC4AudioAmbienceLayer`, vtable `0x00A934A8`, IID `0x4A2835FE`; slots: 3 `DoMessage`, 4 `Init`,
5 `Shutdown`, 6/7 `GetName`/`SetName`, 8/9 `SetLayerId`/`GetLayerId`, 10 `GetAveragePosition`, 11 `Enable`,
12/13 `AddAmbienceLoop`/`RemoveAmbienceLoop`, 14 `LoadFreshnessScore`, 15/16 `Add`/`RemoveFreshnessScore`,
17 `SetScoreMasterVolume`, 18 `CanHandleOccupant`, 19 `CanHandle`, 20 `HandleCell`, 21 `HandleOccupant`,
22–24 `Insert`/`Move`/`RemoveOccupant`, 25 `BeginUpdate`, 26 `Update`, 27 `EndUpdate`, 28 `Pause`.
The manager uses 4, 17, 20, 25–28 directly and 21 through the cell walk.

### 8.1 Layer `Init` (`0x004E8180`)

Runs once per layer (byte `+0x04` set on the first call; later calls do nothing). A process-wide layer count
(`0x00B21944`) is incremented; the layer that takes it from 0 to 1 first runs `_StaticInit` (`0x004E4EB0`),
and if that fails `Init` returns its result without doing anything else. Then, in this order:
`_ReadTuningValues(layerId)` (layer id `+0x1F8`, §8.2; its result is ignored); subscribe this layer to
`0xC53D10AA` on the message server (`0x652294C7`); `_CreateFilters` (`0x004E78E0`); `_CreateLayers`
(`0x004E4F60`); own slot 14 `LoadFreshnessScore`; own slot 12 `AddAmbienceLoop(+0x1D0)` (the loop sound id
read from `0x6A3FBDDA`, §8.2).

### 8.1.1 Layer construction, ownership and uncovered slots (AA14)

Constructor `0x004E7290` starts initialized byte `+4` and refcount `+8` at zero,
enabled byte `+0x0C` at one and paused byte `+0x0D` at zero. Group radii `+0x20/+0x24`
are f32 50; the three group holders are null, group mode is 1 and `+0x38` is 5.
Position/accumulator fields `+0x3C`–`+0xA4` and `+0xB0`–`+0xD0` take the zero f32 at
`0x00B2194C`; total `+0xA8`, density `+0xAC` and timer `+0xD4` are zero. It constructs
the ease-sound member and three response curves, empty score/loop lists, name string,
tuning/filter vectors, and zero filter counts, loop sound ID `+0x1D0`, score master volume
`+0x1E8` and score-selection state. It stores its constructor layer-ID argument at `+0x1F8`;
the name and tuning values are populated later. Do not infer initialized values for padding
or scalar fields not explicitly set here. Loop entries start with **instance ID zero**.
The process HandleCell cache starts with signed key pair `(0x7FFFFFFF,0x7FFFFFFF)`
at `0x00B09810/14`; its cached world x/y/z are zero-initialized storage at
`0x00B2196C/48/70`. Construction, BeginUpdate and Shutdown do not reset that cache.

QI (`0x004E4A70`) accepts only 1 and `0x4A2835FE`, returns base and AddRefs; failure
returns false without overwriting the output. AddRef increments `+8`; Release
(`0x004E7A60`) decrements when signed count >1, otherwise destroys and frees the outer
object, returning zero. Destructor `0x004E76E0` destroys vectors, curves, string, score/loop
containers and ease-sound state, and releases group holders in order `+0x30,+0x2C,+0x28`.
It does **not** call Shutdown, unsubscribe or decrement the process layer count.

StaticInit (`0x004E4EB0`) retains the current global resource manager into `0x00B21950`,
releasing any replaced reference; missing resource manager returns false. It then obtains
SndSys `(0x23CECBA0,0x106077D2)` and retains it in `0x00B21954`, balancing the temporary
service reference. Missing SndSys returns false while retaining the resource manager.
Init's initialized byte and process-count increment remain set on this first failure:
there is no rollback. A repeated Init on that layer returns true without retrying StaticInit
or doing the rest of Init. Another layer sees a nonzero count and skips StaticInit too.
After successful StaticInit, Init requires the message-server output: it dereferences it to
subscribe even if lookup failed. It ignores tuning, subscription and subordinate load/add
results, completes §8.1's ordered operations and returns true. These are literal failure
rules, not a promise that every failed service lookup returns cleanly.

Shutdown (`0x004E7860`) is a true-returning no-op when its initialized byte is clear.
Otherwise it requires the message server to unsubscribe `0xC53D10AA`, removes group layers,
filters, all freshness scores and all loops, in that order, decrements the process count,
runs StaticShutdown only when that reaches zero, clears initialized, and returns true.
StaticShutdown (`0x004E6FD0`) clears the process filter cache first, then clears/releases
resource manager and SndSys, returning true. Thus Shutdown also balances a first Init that
returned false, provided required message-server pointers are valid.

DoMessage (`0x004E8230`) rejects null and any type other than `0xC53D10AA` with false.
For that type it rereads tuning using layer ID, removes/recreates filters, removes/recreates
group layers, calls its own Enable with the inverse current enabled byte **twice**, then
returns true. Since Enable (`0x004E4A60`) only stores the supplied byte and performs no IO,
these calls restore the original enabled state. The reload does not load scores or loops.
CanHandleOccupant (slot 18, `0x005DEB40`) always returns false, with no occupant inspection;
it is distinct from the filtering path in §8.4.

InsertOccupant (`0x004E4BD0`) rejects null; otherwise occupant `+0x20` supplies its position,
which is inserted into group layer `+0x28` with extra radius zero. Only successful insertion
AddRefs the occupant, increments `+0x124` and returns true. MoveOccupant (`0x004E4C40`)
rejects null; otherwise it obtains position and calls group UpdateInstance with occupant,
position and radius `+0x20`, returning true regardless of that helper's result, with no
reference adjustment. RemoveOccupant (`0x004E4CA0`) returns false for null or failed group
removal; success Releases the occupant, decrements `+0x124` and returns true. These slots
assume an existing group layer; they do not run CanHandle or acquire the occupant on entry.

AddAmbienceLoop (`0x004E69D0`) rejects sound ID zero or an existing same-ID loop. Otherwise
it appends a loop with that sound ID, instance ID zero, three f32 position/volume-state
words zero and final word zero, and returns true without starting a sound. RemoveAmbienceLoop
(`0x004E6240`) erases the first matching sound ID and returns true, or false if absent;
it issues **no stop**, even for a running instance. Bulk removal (`0x004E6280`) instead
creates stop event `0x6A451908`, payload IID `0x5B523BA4`, data `(instance ID,0,0,0)` for
each nonzero instance, submits it through SndSys `+0x30`, releases the event, and erases
every entry. Creation failure is not checked before submission. It returns true.

LoadFreshnessScore (`0x004E6D20`) reads floor(word count/2) group/instance pairs from
`+0x1D4`, builds TGI `(0x2026960B,group,instance)` and calls its own AddFreshnessScore
for each pair. Failed additions log and processing continues; an odd trailing word is ignored.
AddFreshnessScore (`0x004E6E00`) rejects a duplicate **full TGI**. It creates class
`0x3C1CF59E`, IID `0xDC1CF595` (freshness player), then event `0x2A2C0384`, payload IID
`0x5B523BA4`, initially four zero data words. Failed object/event creation returns false,
releasing any outputs; it appends no entry. On success event `+0x18` supplies payload;
payload `+0x84(player)`, `+0x2C(type)`, `+0x40(group)`, `+0x54(instance)` are called in
order, followed by SndSys `+0x30(event)`. Dispatch result does not gate success. It appends
the score with flag bytes `(0,1)`, zero instance/timer words, saved TGI and retained player,
balances temporary references and returns true. Resource loading belongs to the event's
recipient; AddFreshnessScore does not read resource bytes itself.

RemoveFreshnessScore (`0x004E6840`) matches the first entry by **instance ID alone**, not
full TGI. Missing returns false. Found creates event `0x4A2C0389` with the same payload IID,
sets payload `+0x84(player)` and submits via `+0x30` only if creation succeeded; it removes
the score even on event-creation failure. It unconditionally Releases the player once, then
clears/releases the retained player again, unlinks and destroys the entry, releases any event
and returns true (`0x004E68B7`–`0x004E68E9`). This extra Release is literal *[quirk]*.
Bulk score removal (`0x004E6910`) sends the same removal event for every entry when creation
succeeds, but only clears/releases the entry's one retained player reference, erases each
entry and returns true. SetScoreMasterVolume (`0x004E61F0`) stores the raw word at `+0x1E8`
without a clamp and immediately calls UpdateAmbienceScores with elapsed `0x7FFFFFFF`.

**Conformance cases.** Fail first StaticInit, then call Init again: false then true, count
only incremented once and no retry; Shutdown decrements once. A tuning notification with
enabled true records Enable(false), Enable(true); other types return false. Duplicate loop
ID is false; removing a live loop singly produces no stop, while bulk removal sends its
instance ID. Three freshness words create one TGI attempt. Same instance ID in different
groups may be added twice; singular removal removes the first. Failed removal-event creation
still erases the entry and makes the stated Release calls. Insert failure acquires no occupant;
successful insert/removal make one AddRef/Release pair. Move of non-null occupant returns true
even on a failed update. A new layer does not reset an earlier layer's cell-cache keys.

### 8.2 Layer tuning (`_ReadTuningValues` `0x004E7A90`)

**Defaults, written first, before the exemplar is looked up** (so a missing exemplar leaves exactly these):
ease object `+0xE8` gets `+0x108 := 100 × 0.001 = 0.1` and `+0x10C := 4000 × 0.001 = 4.0` (setter
`0x004F7BF0`: each argument times the f32 constant 0.001 at `0x00A867A4`); name `+0x188 :=
"Noname_0x%08x"` of the layer id; `+0x1E1 := 0x1F`, `+0x1E0 := 0x1F`, `+0x1EC := 1024`, `+0x1E8 := 1024`,
`+0x1F0 := 100`, `+0x34 := 1`, `+0x38 := 5`, `+0x20 := 50.0`, `+0x24 := 50.0`, `+0x200 := 1.0`; `+0x204 :=` the
u32 at `+0x314` of the object at `[0x00B43CC4] + 0x18`; three response curves with two points each:

| Curve | Default points (x, y) | Tuning key |
|---|---|---|
| `+0x1A0` | (0, 100), (20, 100) | `0xCA4132E9` |
| `+0x1B0` | (0, 0), (20, 1024) | `0x6A4132F3` |
| `+0x1C0` | (0, 1024), (20, 1024) | `0x6A4132F6` |

**Exemplar**: type `0x6534284A`, group `0xCA25875D`, instance = the layer id, through the resource manager
(`0x00B21950` slot 3, IID `0xA52160F5`). Missing → log "Failed to get resource", return 0 (defaults stay).
Present → read, in this order (each reader leaves the destination unchanged when the property is absent, except
where "cleared" says the destination is zeroed just before the read):

| Key | Reader | Destination | Absent value |
|---|---|---|---|
| `0x00000020` | string (`0x005FD570`) | name `+0x188` | `Noname_0x…` |
| `0xEA3FBDD7` | vec<u32> (`0x005FF510`) | vector `+0x1D4` (emptied first) | empty |
| `0x2A3FBDE1` | vec<u32> | vector `+0x208` (emptied first) | empty |
| `0x2A3FBDE5` | vec<u32> | vector `+0x224` (emptied first) | empty |
| `0x0A410BA0` | vec<u32> | vector `+0x240` (emptied first) | empty |
| `0x6A3FBDDA` | u32 (`0x005FD480`) | `+0x1D0`, the layer's ambience-loop sound id (§8.1) | 0 (cleared) |
| `0x0A25896B` | u8 (`0x005FD3C0`) | `+0x1E0` | 0 (cleared; the `0x1F` default is lost) |
| `0xAA3FBDDE` | u8 | `+0x1E1` | 0 (cleared) |
| `0xEA3FC42A` | u32 | `+0x1EC` | 1024 |
| `0xAA3FC48E` | u32 | `+0x1E8` | 1024 |
| `0x6A3FC4C7` | u32 | `+0x1F0` | 100 |
| `0x8A552758` | u32 | `+0x34` | 1 |
| `0x8A70B1A2` | u32 | `+0x38` | 5 |
| `0x8A3FC5DD` | f32 (`0x005FD4F0`) | `+0x200` | 1.0 |
| `0xCA4132E9`, `0x6A4132F3`, `0x6A4132F6` | response curve (`0x005F4700`) | curves `+0x1A0`, `+0x1B0`, `+0x1C0` | the defaults above |
| `0x0A4168E1` | f32 | `+0x20` | 50.0 |
| `0x0A56212A` | f32 | `+0x24`, which is first set to the new `+0x20` | `+0x20` |
| `0x8AA039B7`, `0x4AA03DB1` | u32 into locals (defaults 100, 4000) | ease `+0x108`, `+0x10C` := float(u32) × 0.001 | 0.1, 4.0 |

The two ease values are converted **unsigned** (`FILD` plus 2^32 when negative, `0x004E8105`–`0x004E8130`);
the call is the same setter as the default, so a missing `0x8AA039B7` still yields 0.1 s. The function returns 1
on success. The two ease values are rates per millisecond (§8.7). Roles: `+0x34`/`+0x38`, `+0x200` in §8.5;
`+0x1E8`, `+0x1F0`, `+0x204` and curves `+0x1A0`/`+0x1B0` in §8.6; curve `+0x1C0` in §8.7; `+0x20`/`+0x24` and the
vectors in §8.3–§8.5. `+0x1E0`, `+0x1E1` and `+0x1EC` are read by none of the routines this spec covers.

Worked example: an exemplar with only `0x6A3FBDDA = 0x1234` and `0x0A4168E1 = 30.0` gives loop sound
`0x1234`, `+0x20 = +0x24 = 30.0`, `+0x1E0 = +0x1E1 = 0`, every other value its default.

### 8.3 Layer filters (`_CreateFilters` `0x004E78E0`, `_AddFilter` `0x004E7010`)

The three tuning id lists are **filter class ids** of three kinds: kind 1 from `0x2A3FBDE1` (`+0x208`), kind 2
from `0x2A3FBDE5` (`+0x224`), kind 3 from `0x0A410BA0` (`+0x240`). `_CreateFilters` first resets three bit
vectors (`+0x144`, `+0x158`, `+0x16C`), then for each kind in the order 1, 2, 3 sizes that kind's bit vector to
its id count (all false) and calls `_AddFilter(id, kind)` for each id in list order.

`_AddFilter(id, kind)` (pop 8):

1. Look `id` up in a **process-wide** filter cache (a hash map class id → filter, buckets `0x00B2195C`–
   `0x00B21960`, index `id mod bucket count`), shared by every layer. Found → use that filter.
2. Not found → create it through GZCOM `CreateInstance(id, IID 0x4A40074C)`. Failure → log "Failed to create
   class object filter", return (nothing counted). Then call the filter's `Init` (slot 3, `+0x0C`); false → log
   "Failed to init filter", release it, return. Success → the cache takes one reference and stores it.
3. Count the filter's interfaces on the layer: answers `0xCA400757` → `+0x138 += 1`; `0xCA400753` → `+0x13C
   += 1`; `0xEA40074F` → `+0x140 += 1` (the query references are released at the end).
4. Append the filter pointer (no reference of its own; the cache owns it) to the kind's list and bump its count:
   kind 1 → list `+0x214`, count `+0x220`; kind 2 → `+0x230`, `+0x23C`; kind 3 → `+0x24C`, `+0x258`
   (`0x004E71E0`–`0x004E7247`). Another kind value appends nothing.

The same class id listed twice, or by two layers, yields the same filter object in each list *[compat]*.
**Correction (2026-10-25, re-audioclose8):** each list entry does hold a reference: `_AddFilter` AddRefs once more
before appending (`0x004E717D`) and `_RemoveAllFilters` releases one per entry (audio-owner-closure.md §2.3).
How the kinds and interfaces are used is §8.4.

Direct repeated CreateFilters resets interface counters `+0x138/+0x13C/+0x140` and the three
bit vectors, but **does not clear** existing pointer lists or their kind counts
`+0x220/+0x23C/+0x258`. It appends newly resolved filters to those lists, even when duplicate.
The normal tuning-message route explicitly RemoveAllFilters first, which clears the lists
and counts; do not give a direct CreateFilters call that extra cleanup. A conformance case
with one successful kind-1 filter called twice therefore has kind count 2 and two pointer
entries, while the interface counters reflect only the second creation pass.

### 8.4 Filtering: `CanHandle`, `HandleCell`, `HandleOccupant`

**Filter interfaces.** A filter object may answer any of three interfaces; each has its test at slot 5
(`+0x14`), returning a bool:

| IID | Stage | Test arguments | Used by |
|---|---|---|---|
| `0xEA40074F` | global | none | `CanHandle` |
| `0xCA400757` | cell | (cell x, cell z), two ints | `HandleCell` |
| `0xCA400753` | occupant | (occupant `cISC4Occupant*`) | `HandleOccupant` |

**Kinds** (the three lists of §8.3) combine the tests of one stage the same way in all three functions. Each
list is walked in order; a filter that does not answer the stage's interface is skipped (not counted):

- kind 1 (**all**, list `+0x214`): a filter whose test is false rejects at once; true sets its bit in
  `+0x144`.
- kind 2 (**none**, list `+0x230`): a test that is true rejects at once; false sets its bit in `+0x158`.
- kind 3 (**any**, list `+0x24C`): the first true test sets its bit in `+0x16C` and ends the walk.

The item **passes** when nothing rejected, at least one filter of any kind answered the stage's interface
("answered" counter), and either the kind-3 list is empty or one of its filters passed. *[compat]* A kind-3 list
whose filters all lack the stage's interface (or all test false) makes every item fail without counting it as
a reject. The bit vectors only accumulate (they are cleared by `BeginUpdate`); nothing in the layer reads them.

**`CanHandle`** (slot 19, `0x004E5110`, no arguments): first, the audio preferences object (application
`0x00B43C94` slot `+0x98`) byte `+4` clear → return false, touching nothing. Otherwise the answer is cached
per update cycle: byte `+0x12D` set → return byte `+0x12C`. Else set `+0x12D := 1`, run the global stage;
pass → `+0x12C := 1` and return 1; fail → return 0 (`+0x12C` keeps the 0 `BeginUpdate` gave it, so the
cached answer is also false).

**`HandleCell(x, z)`** (slot 20, `0x004E5370`, pop 8, always returns 0): own `CanHandle` (vtable slot 19)
false → nothing. Run the cell stage. Rejected → `+0x134 += 1` (cell rejects). Pass → `+0x128 += 1` (cells
accepted), then the cell's world position: if (x, z) differs from a one-entry process-wide cache
(`0x00B09810`/`0x00B09814`), city (`0x00B43CEC`) slot `+0x184` `CellCenterToPosition(x, z, &px, &pz)`, then
terrain (`0x00B43CF4`) slot `+0x34` altitude at (px, pz) → py (stored as f32), and the cache keys become
(x, z); the cached (px, py, pz) is used either way. That point is inserted into group layer `+0x2C`
(below) with extra radius 0. Neither cache is reset between updates or layers.

**`HandleOccupant(occ)`** (slot 21, `0x004E5670`, pop 4, always returns 0): `CanHandle` false or `occ` null →
nothing. Run the occupant stage. Rejected → `+0x130 += 1` (occupant rejects). Pass → `+0x124 += 1`
(occupants accepted), occupant slot 8 (`+0x20`) fills its position (x, y, z), inserted into group layer
`+0x28` with extra radius 0.

Worked example: kind 1 = [A (global + occupant)], kind 3 = [B (cell)], kind 2 empty. `CanHandle`: A's global
test true, but kind 3 has B, which lacks the global interface → fail; the layer never handles anything
(compat case above). Move B to kind 1: `CanHandle` passes on A; `HandleCell` asks only B; `HandleOccupant`
asks only A.

**Group layers** (`cGroupLayer`, ctor `0x004F12E0(radius)`, vtable `0x00A94200`): created by `_CreateLayers`
(`0x004E4F60`, from `Init`) from the tuning:

| Field | Radius | When `+0x34` (tuning `0x8A552758`) = 1 |
|---|---|---|
| `+0x28` occupant groups | `+0x20` (`0x0A4168E1`) | same |
| `+0x2C` cell groups | `+0x24` (`0x0A56212A`) | `+0x2C` is `+0x28` (one shared layer) |
| `+0x30` | `+0x24` when `+0x34 = 2`, else `+0x20` | not created |

A group layer holds a list of groups; a group has a centre (3 floats), a radius (the layer's), a count and a
peak. Inserting point p with extra radius r (`_AddPositionToGroup` `0x004F0C20`): walk the groups in creation
order; the first whose distance from its centre to p, computed in x87 extended precision as
sqrt(dz² + dy² + dx²), is **strictly less** than r + group radius takes it: count += 1, peak := max(peak,
count). No group → append a new one with centre p (never moved later), count 1, peak 1. `BeginUpdate`
(`0x004E4CE0`) copies its 16-byte argument (the attention rectangle) to `+0x10`–`+0x1C`, calls `Clear(1)` on
`+0x28` and on `+0x2C` when distinct (every group: peak := count, count := 0; groups stay), zeroes
`+0x124`–`+0x134` and the two `CanHandle` bytes, and clears every bit of the three bit vectors. `EndUpdate`
(`0x004E4E80`) removes groups whose count is still 0 from the same layer(s). So after an update a group's count
is the number of points it took this cycle and its peak is the larger of that and last cycle's count.

### 8.5 `Update(e, &P, &Q)` (slot 26, `0x004E6340`, pop 12)

Called after every `_DoStageUpdate` (§5) with the elapsed ms `e` and the manager's two listener vectors
(P = `&listenerPos`, Q = `&listenerVector`; both 3 floats).

1. Copy P to `+0xB0`–`+0xB8` and Q to `+0xBC`–`+0xC4`.
2. (A walk over the three bit vectors writes byte `+0x0C`, but step 3 overwrites it: no effect.)
3. `+0x0C :=` own `CanHandle` (slot 19). False → skip to step 8.
4. **Groups**, by `+0x34` (tuning `0x8A552758`): 1 → layer L = `+0x28`; 2 or 3 → `+0x30.Intersect(+0x28,
   +0x2C, swap = (+0x34 = 2))`, L = `+0x30`; any other value → no group step (the list stays empty and the
   group count `+0xA8` keeps its previous value). For L: `GetPositions` (`0x004F0B40`, rectangle test off)
   appends every group centre to a temporary list in group order, sets `+0xA8 :=` number of groups and
   `peaks :=` the sum of their peaks.
   `Intersect(A, B, swap)` (`0x004F0EA0`): inner := A, outer := B (swapped when `swap`); delete every group of
   the target; mark every inner group unused; `left :=` inner group count; for each outer group o, for each
   inner group i, while `left > 0`: if i is unused and the distance between the centres is strictly less than
   o.radius + i.radius, append a new target group with i's centre and radius, count 1, peak 1, mark i used,
   `left -= 1`. So with `+0x34 = 3` the result is the occupant groups lying near some cell group, with 2 the cell
   groups lying near some occupant group; every result group has peak 1.
5. **Score value** `+0xAC`, by `+0x38` (tuning `0x8A70B1A2`, default 5); integers are converted with `FILD`
   and stored as f32:

| `+0x38` | `+0xAC` |
|---|---|
| 1 | `peaks` |
| 2 | residential population (`0x00B43DA4` slot 16 `GetPopulation`); 0 when that global is null |
| 3 | `FLT_MAX` (`0x7F7FFFFF`) |
| 4 | the zoom index (`[0x00B43DD8] + 0x0C`) |
| 5 | `peaks / +0xA8` (`FILD` then `FIDIV`, `0x004E659C`); 0.0 when `+0xA8 = 0` |
| 6 | the u32 at `+0x11C` of the object at audio-director `+0x54` (`0x004E82A0`), converted unsigned — see below |
| other | unchanged |

   `peaks` starts at 0 (`0x004E6511`), so with no group step mode 1 gives 0 and mode 5 gives 0 over the stale
   `+0xA8`.

   **Mode 6** (`0x004E65AE`–`0x004E65FC`): the audio director is the static object `0x00B21978`
   (`GetSC4AudioCOMDirector` `0x004ECD60`), so its `+0x54` is `0x00B219CC`. The director's constructor
   (`0x004EC72D`) zeroes it and nothing else writes it: `PostAppInit` (`0x004EBF40`) fills `+0x48`, `+0x4C`,
   `+0x50` (the ambience manager, class `0x6A12F51C`), `+0x58`, `+0x5C`, `+0x60`, `+0x64` but not `+0x54`; the
   other 14 callers of `GetSC4AudioCOMDirector` read other members; the export has no data reference to
   `0x00B219CC`. So the read is `[0 + 0x11C]` and the original **faults** (access violation in the one-instruction
   getter `0x004E82A0`); with a null director the code also passes null. A layer with `+0x38 = 6` is therefore
   unusable in the shipped game *[compat]*. `+0x11C` is the tracked-occupant count of the ambience manager (§1),
   which sits one member earlier at `+0x50`; a reimplementation that must not crash may return that count
   *[inferred]*, but no shipped behaviour exists to match.
6. **Sample positions** (only when `+0xA8 ≠ 0`; `_GetEquallyDistributedPositions` `0x004E6070`) into the array
   `+0x3C` (9 × 3 floats), count back into `+0xA8`. Up to 9 group centres → all of them, in list order.
   10 or more → a 3 × 3 grid over the attention rectangle `+0x10` (floats x0, z0, x1, z1): `sx := trunc((x1 −
   x0) × 0.33333334)`, `sz := trunc((z1 − z0) × 0.33333334)` (`0x00A92480`; integers); for each centre c in
   list order `ix := trunc((c.x − x0) / sx)`, `iz := trunc((c.z − z0) / sz)`, each clamped to 0..2, and the
   first centre to land in cell `ix + 3·iz` is kept. A zero `sx`/`sz` divides by zero (x87 masked: ±inf or NaN,
   `trunc` gives `0x80000000`, clamped to 0) *[compat]*.
7. **Average position** `+0xC8`–`+0xD0` (only when `+0xA8 ≠ 0` after step 6; read by `GetAveragePosition`,
   slot 10). With `d := P − Q` and `n := d × (1.0 / |d|)` (`0x00B09818` = 1.0; the factor stored as f32, each
   component stored f32), for each kept position p: `v := p − Q` (components f32), `a := |v · n| / |v|`
   (x87 extended, computed as `(v.z·k)·n.z + (v.y·k)·n.y + (v.x·k)·n.x` with `k = 1/|v|`, then stored f32),
   `w := a × (1.0 / (+0x200 − a))` (`+0x200` = tuning `0x8A3FC5DD`, default 1.0), `W += w`, `S += w·p`.
   Result `S × (1.0 / W)` (the factor stored f32). The sums start at 0.0 (`0x00A81054`, and the zero vector
   `0x00B2194C`). The loop also tracks the smallest |v| but never uses it. A position exactly on the view
   line with `+0x200 = 1.0` gives w = ±inf *[compat]*.
8. Free the temporary list; then `_UpdateAmbienceScores(e)` (§8.6) and `_UpdateAmbienceLoops(e)` (§8.7),
   whether or not step 3 passed.

Worked example: `+0x34 = 1`, two occupant groups with peaks 3 and 1, `+0x38 = 5` → `+0xAC = 4 / 2 = 2.0`; both
centres are kept (fewer than 10), and the average weights each by how close it lies to the P–Q line as seen
from Q (|cos| of the angle), sharpened by `a / (m − a)`.

### 8.6 `_UpdateAmbienceScores(e)` (`0x004E5920`)

The argument `e` is not read (pop 4). `Pause` (slot 28, `0x004E6210`) stores its flag in byte `+0x0D` and then
calls this and `_UpdateAmbienceLoops` with `e = 0x7FFFFFFF`.

**Freshness parameters** (`_UpdateFreshnessParameters` `0x004E4AA0`, called first). When `+0x0C` (this update's
`CanHandle`, §8.5) is clear: `+0xDC := 0`, `+0xE0 := 0`. Otherwise, with `s = +0xAC` (§8.5 step 5):

1. `+0xDC := (trunc(curve +0x1B0 at s) × +0x1E8) >> 10` — `_ftol2`, then a 32-bit `IMUL` (low half, wraps) and a
   **logical** shift (`SHR`, `0x004E4AD2`). Tuning: curve `0x6A4132F3`, `+0x1E8` = `0xAA3FC48E` (default 1024).
2. `+0xE0 := trunc(curve +0x1A0 at s)` (curve `0xCA4132E9`), then by `+0x204` (§8.2): 0 → `+0xE0 × 50 / 100`,
   1 → `+0xE0 × 75 / 100` (32-bit signed multiply, signed division truncating toward zero; the compiler's
   reciprocal `0x51EB851F`, `SAR 5`, plus the sign bit, `0x004E4B00`–`0x004E4B34`), any other value → unchanged.

So `+0xDC` is the score volume (0..1024 scale when the curve stays in 0..1024) and `+0xE0` the play percent.

**The score list `+0x180`** (filled by `LoadFreshnessScore`, §8.1). Each entry: byte `+0x08` *playing*, byte
`+0x09` *ready* (the entry has a loaded score; not written here), `+0x0C` last sent play percent, `+0x10` last
sent volume, `+0x20` the freshness player (`cFreshPlayer`, the object `DoEvent` §6 acts on). "Send X(...)"
below means: create event X through the sound-event system `0x00B21954` (slot 13 `+0x34`, IID `0x5B523BA4`);
when that succeeds, put the player in the payload's void1 (slot 33 `+0x84`), the listed values in data1
(slot 11 `+0x2C`) and data2 (slot 16 `+0x40`), post it (slot 12 `+0x30`); a failed create skips the rest of that
send. The event reference is released before the next create and at the end of the entry.

Let `on := (+0xDC ≠ 0) and (+0xE0 ≠ 0) and (+0x0D = 0)`. For each entry in list order:

1. **Start** — when `+0xDC ≠ 0`, `+0xE0 ≠ 0`, entry not playing, entry ready and `+0x0D = 0`: last percent :=
   0, last volume := 0; send `0xEA2D827C` (SetMinMaxDistance, §6) with data1 `= trunc(d)` and data2 `=
   trunc(d × 16.0)` where `d := float(+0x1F0) × 0.01 × Z`, then send `0xEA2C0390` (SetHeadRelative + Play)
   with data1 `= (+0x34 = 4)`; when that create succeeds, playing := 1. (Details below.)
2. **Stop** — when not `on` and the entry is playing: send `0x2A2C0393` (3-second fade to 0) with no data;
   when that create succeeds, playing := 0 and the entry ends here. A failed create falls through to step 3
   with the entry still playing *[compat]*.
3. **Update** — only when the entry is playing (reached when `on`, or after a failed stop create):
   a. **Divisions**, skipped when `+0x34 = 4` (the head-relative mode): for `i = 0 .. +0xA8 − 1` (signed
      count; none when ≤ 0), with column `i mod 3` and row `i div 3`, direct calls on the player: slot 33
      (`+0x84`) `(col, row, trunc x, trunc y, trunc z)` of sample position `i` (`+0x3C + 12·i`, §8.5 step 6;
      `_ftol2` each), then slot 32 (`+0x80`) `(col, row, 1)`. Then, if fewer than 9 were set, slot 32
      `(col, row, 0)` for each remaining `i` up to 8. These are sent every update, without change detection.
      (By name order in the export these are `SetDivision3DPosition` and `SetDivisionEnabled` *[inferred]*.)
   b. **Volume** — when `+0xDC ≠` last volume: last volume := `+0xDC`, then (still playing) send `0x4A3DA208`
      (SetVolume) with data1 `= +0xDC`.
   c. **Play percent** — when `+0xE0 ≠` last percent: if playing, send `0x0A414C5C` (SetPlayPercent) with
      data1 `= +0xE0`; then last percent := `+0xE0` whether or not the send happened.

Step 1 falls into the same entry's step 3, so a freshly started score also gets its divisions, its volume
(last volume was reset to 0, so the first nonzero `+0xDC` is sent) and its play percent in the same update.

**Distance `d`** (`0x004E59B5`–`0x004E5A1B`): `+0x1F0` (tuning `0x6A3FC4C7`, default 100) is loaded with `FILD`
plus 2^32 (`0x00A80AA8`) when negative (unsigned), multiplied by the f32 0.01 (`0x00A823A4`, 0x3C23D70A) and
then by `Z`, the f32 at `0x00B21A30`, in x87 extended precision, and stored once as f32. `Z` is the listener's
zoom-5 distance scale, written only by `cSC4AudioListener::_CalculateZoomValues` (`0x004F18C0`, the fifth of
five floats `0x00B21A20`–`0x00B21A30`, each `A[i] × B[i] × k` from the listener's tuning) and 0.0 before it
first runs. data1 is `_ftol2` of the stored f32; data2 is `_ftol2` of (that f32 × 16.0, `0x00A8D45C`, extended).

Worked examples:
- Freshness: `+0xAC = 20.0` with the default curves gives `+0xDC = 1024 × 1024 >> 10 = 1024` and `+0xE0 =
  100`, then 50 when `+0x204 = 0`, 75 when it is 1. A curve value of −1 gives `0xFFFFFC00 >> 10 = 4194303`
  (logical shift) *[compat]*. `+0x0C` clear gives 0/0, which stops every playing score (step 2).
- Distance: `+0x1F0 = 100`, `Z = 1000.0`: `0.01f × 100 × 1000` is 999.99997764… in extended precision and
  rounds to f32 1000.0, so data1 = 1000, data2 = 16000. With `Z = 0.0` both are 0.
- One ready entry, ticks with (`+0xDC`, `+0xE0`) = (0, 0), (512, 50), (512, 50), (512, 60), then `+0x0D` set:
  tick 1 nothing; tick 2 SetMinMaxDistance, Play, divisions, SetVolume 512, SetPlayPercent 50; tick 3 only the
  divisions; tick 4 divisions and SetPlayPercent 60; the paused tick sends the fade (`0x2A2C0393`).

### 8.7 `_UpdateAmbienceLoops(e)` (`0x004E5D50`)

**Target volume** (`0x004E4B60`): `+0xE4 := 0`; when `+0x0C` (this update's `CanHandle`) is set and `+0xAC ≠
0.0` (`FUCOMPP`; a NaN counts as unequal), `+0xE4 := trunc(curve +0x1C0 (tuning `0x6A4132F6`) at x =
+0xAC)` (`_ftol2`).

**The ease object `+0xE8`** holds a vector (target `+0x00`, current `+0x0C`), a scalar (target `+0x18`, current
`+0x1C`), a flag byte `+0x38` and the two ease times of §8.2. `SetTarget(v, snap)` (`0x004F8240` vector,
`0x004F7BD0` scalar) stores the target and, when `snap` is set or the flag `+0x38` is clear, also the current;
the getters (`0x004F8280`, `0x00808620`) return the current. The scalar is an **integer** (s32 target and
current; the setter `0x004F7BD0` copies the word). The two §8.2 values are **rates per millisecond**, not
times: `+0x20` (layer `+0x108`, default 0.1 = 100 units/s) for the vector, `+0x24` (layer `+0x10C`, default 4.0
= 4000 volume units/s) for the scalar.

**Time step** `Step(t)` (`0x004F8650`, `t` an f32 in ms, pop 4); the only caller is `_UpdateAmbienceLoops`:

1. Vector: `dx := tx − cx`, `dz := tz − cz` (x87 extended), `dy := ty − cy` stored f32; `L := sqrt((dz² + dx²)
   + dy²)`, kept extended and also stored as f32 `L32`. When `L32 = 0.0` (`FUCOMPP`; NaN counts as unequal)
   the vector is left alone. Otherwise `k := 1.0 / L` (`0x00B098C4`, extended), `nx := dx·k` and `ny := dy·k`
   stored f32, `nz := dz·k` extended; `s := t × +0x20` stored f32; `m := L32` when `L32 < s`, else `s` (ties and
   NaN take `s`). Then `cx := f32(cx + nx·m)` (the product extended), `cy := f32(cy + f32(m·ny))`, `cz :=
   f32(cz + f32(nz·m))`. Arrival is not snapped: the current lands within rounding of the target.
2. Scalar: `d := target − current` (32-bit wrapping subtraction); `a := f32(|float(d)|)`. When `a = 0.0`
   nothing changes. Otherwise `s := f32(t × +0x24)`, `m := a` when `a < s`, else `s`; `current += trunc(sign(d)
   × m)` (`_ftol2`, toward zero; the sign is `d / a` in extended).
3. Flag `+0x38 := 1` (both paths, `0x004F8773` and `0x004F8781`). From then on `SetTarget` without `snap` no
   longer moves the current, so every `SetTarget` before the first step snaps.

Worked examples: scalar 0 → 512 with rate 4.0 and `t = 33`: steps of 132 give 132, 264, 396, 512 (the last
move is the remaining 116). With `t = 16.5` the step is 66.0 exactly. Vector (0, 0, 0) → (3, 0, 4), rate 0.1,
`t = 20`: `L = 5`, `s = 2.0`, current (1.2, 0, 1.6); the next step moves the remaining 3.0. `Pause` passes
`e = 0x7FFFFFFF`, which becomes `t = 2^31` as an f32 and reaches every target in one step.

Each loop entry of the list `+0x184` (filled by `AddAmbienceLoop`, §8.1) has the sound id `+0x08`, the
instance id `+0x0C` (0 = not playing), the last sent position `+0x10`–`+0x18` and the last sent volume `+0x1C`.
For each entry in list order, with `ev` = the sound-event system `0x00B21954` (create slot 13 `+0x34` with the
id and IID `0x5B523BA4`, then post slot 12 `+0x30`):

1. `+0xE4 = 0`: if playing, post stop `0x6A451908(instance, 0, 0, 0)` and set the instance to 0.
   Otherwise, if not playing: `SetTarget(+0xC8, snap)` and `SetTarget(+0xE4, snap)`, last volume := 0, last
   position := (0, 0, 0), instance := `GenerateNewInstanceId` (`0x004F7BB0`), post start
   `0xEA451903(sound id, instance, 1, +0xE4)`.
2. `SetTarget(+0xC8, no snap)`, `+0xC8 :=` current vector; `SetTarget(+0xE4, no snap)`, `+0xE4 :=` current
   scalar. (So from the second entry on, step 1 sees the eased volume, not the target.)
3. `+0xE4 ≠` last volume: last volume := `+0xE4`; if playing, post `0x4A451915(instance, +0xE4, 0, 0)`.
4. Any component of the last position differing from `+0xC8`–`+0xD0` by **at least** 0.0001 (`0x00A9332C`,
   f32) and playing: post `0xCA45190F(instance, trunc x, trunc y, trunc z)` (`_ftol2` of each float), then last
   position := `+0xC8`–`+0xD0` (the floats).

After the walk, the ease time step with `float(e)` (e converted unsigned).

Worked example: one loop, `+0xAC` 0 then 2.0 with curve `+0x1C0` giving 512 at 2.0: tick 1 posts nothing;
tick 2 starts the loop at volume 512 (snapped), then posts volume 512 and the average position (truncated)
because the last values were reset to 0; a later tick with `CanHandle` false posts stop.

## 9. Open items

| # | Question | What would settle it |
|---|---|---|
| **AA1** | Answered 2026-10-21: §8.6 (scores and freshness parameters), §8.7 (ease rates and time step), §8.5 mode 6 (the director member is never set; the original faults). | — |
| **AA4** | Answered 2026-10-22 (re-audio-final-gaps4): §3.1, `0xC7CC75F1` is the `cSC4Vehicle` occupant type (all vehicle subclasses). | — |
| **AA10** | Open (policy): native faults on a null clock in `_AddAlarmOccupant` (§3.6); which replacement behaviour to adopt is a product choice. | coordinator / user decision |
| **AA11** | Answered 2026-10-22 (re-audio-final-gaps4): §3.2, the two move posts are independent; each needs the sound system. | — |
| **AA5-AA9** | Answered 2026-10-21 (re-audio-inputs4): §3.6 alarm creation, §4.1/§4.2/§2.2 vectors and unchecked globals, §5 iterator, §3/§3.2/§3.5/§3.6 instance ids, §2.3/§6 results. | — |

## 10. Conformance tests

1. With no `0x0A36F410` in the tuning, every margin is 70.0.
2. `Init` subscribes exactly the 12 types of §2.1, including `0xC53D10AA`.
3. Alarm clock: hour 7, minute 59 gives `t = 7.0`; a one-shot alarm at 7.5 fires when the hour turns 8.
4. An on/off alarm with times (22, 6) starts its loop at 22:00, keeps it through 03:00 (no event) and stops it
   at 06:00.
5. A preview occupant (`0x029244DB` true) produces no sound event on insert, move or remove.
6. `_DoStageUpdate` visits the four cells of a block in the order of §5.
7. Lot condition 1 → 0 on a lot whose building has `0xCA19D7CA = c` and `0x4A4C132E = r`: events in order
   stop(`o+0x0A4C13CB` value, only if present), start loop `o+c`, start loop `o+r` (§3.4). Inserting the same
   building on a condition-0 lot starts only `o+c`.
8. Condition 7 (or any value above 6) posts no event; condition 2 stops `o+c` and starts `o+a`.
9. A layer whose exemplar is missing leaves loop id `+0x1D0` as constructed, `+0x1E0 = +0x1E1 = 0x1F`, ease times 0.1 / 4.0 s; with
   the exemplar present but those keys absent, `+0x1E0 = +0x1E1 = 0` (§8.2).
10. Two layers listing the same filter class id share one filter object; each layer's kind counter counts it
    once per listing (§8.3).
11. A layer whose kind-3 list holds only cell filters never passes `CanHandle`, so it inserts nothing and posts
    stop for a playing loop (§8.4, §8.7).
12. Two occupant points 10 apart with occupant radius 10: the second is **not** joined (distance must be
    strictly less than 0 + 10 is false at exactly 10); at 9.99 it joins and the group's count becomes 2 (§8.4).
13. Update with 12 group centres all in one third of the rectangle keeps at most 3 sample positions (one per
    occupied grid cell); with 9 centres it keeps all 9 (§8.5 step 6).
14. `+0x38 = 5` with no groups (`+0xA8 = 0`) gives `+0xAC = 0.0`, target volume 0, and no start event (§8.5,
    §8.7).
15. Freshness: `+0x0C` set, `+0xAC = 20.0`, default curves, `+0x204 = 1` gives `+0xDC = 1024`, `+0xE0 = 75`; a
    curve value of −1 for `+0xDC` gives 4194303 (§8.6).
16. A ready score with (`+0xDC`, `+0xE0`) going (0,0) → (512,50) sends, in order, `0xEA2D827C`, `0xEA2C0390`,
    the division calls, `0x4A3DA208` 512 and `0x0A414C5C` 50 in that one update; unchanged values next update
    send only the division calls; `+0x0D` set sends `0x2A2C0393` (§8.6).
17. Ease scalar 0 → 512 at rate 4.0 with `t = 33` takes four steps (132, 264, 396, 512); a vector 5 units away
    at rate 0.1 with `t = 20` moves 2.0 along the line (§8.7).
18. `+0x38 = 6` on an `Update` that passes `CanHandle`: the original faults reading `0x0000011C` (§8.5 mode 6).
19. Alarm created at 14:37 with times [14.0, 15.0] (one-shot): `last = 14.0`; the 14:xx ticks fire nothing,
    the first 15:00 tick fires once (§3.6, AA5).
20. Occupant with `0x6A36AFAC = 0x100` moved: one set-position with instance `o` (not `o + 0x100`) (§3.2, AA8b).
21. Railway crossing with `0xAA87817A = 0x55`: start `(0xEA451903, 0x55, A, 1, 0)` and a set-position on `A`
    (§3.5, AA8e).
22. Iterator worked example of §5: rectangle (0, 0)–(8, 6), first walk visits 20 blocks starting (2, 1) and
    ending (0, 0); the second walk repeats it (AA7).
23. Focus lost: stage 4, every layer `Pause(1)`, result 1; focus regained: stage 1, `Pause(0)` (§6, AA9a).
24. `DoEvent(0x2A5B69C0)` returns 0; `DoEvent(0x2A2C0384)` whose event does not answer `0x65297976` returns 1
    with no load (§6, AA9b). City shutdown returns 1, leaves the alarm list as it was (§2.3, AA9c).
