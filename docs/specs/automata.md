# Spec: the automata layer (saved records and game hooks)

Status: **behaviour spec, short form** (the layer is mostly animation; §0 says what that means), clean-room input.
Q2–Q5 answered by re-answers16 (2026-10-16). The cores14a implementers' questions AM1–AM12 (QUESTIONS.md) are
answered and folded in by re-answers20 (2026-10-16): §3.1, §4, §4.1, §5, §6.1, §6.4, §7, §12, §14 tests 17–23.
This document describes *behaviour*, not code. It was written by the analysis team from observing the original
program and its data files. Implementers must work from this spec, the public LGPL gzcom-dll SDK headers and
general knowledge only. They must **not** consult decompiled or disassembled code (see
[CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose.

Related specs: [foundations.md](foundations.md) (streams §2, the class-object store and refs §3.3–§3.6, messages
§5), [save-records.md](save-records.md) (§3.2: these six components are still original), [lot-records.md](lot-records.md)
(§2.3: the occupant base record), [disasters.md](disasters.md) (§5 fire, §6 riot), [civic-buildings.md](civic-buildings.md)
(the ape escape), [dispatch.md](dispatch.md) (crews), [city-situation.md](city-situation.md) (U-Drive-It),
[clock24.md](clock24.md) (the 24-hour clock and `HourChanged`), [crime.md](crime.md), [advisors.md](advisors.md).

## 0. Summary and scope

The automata layer puts cars, people, planes, boats, animals and props in motion. Almost all of it is decoration.
This short form specifies only what the simulation and the save file depend on:

- the records of the six saved components (§2–§5) and of every object they reference (§6), so that a save made by
  the original loads and re-saves byte-compatibly;
- the calls other specs make into the layer (§7) and the riot, fire and U-Drive-It hooks (§8, §9);
- the message subscriptions (§10) and the interface slot tables (§11).

It does **not** specify movement, path following, animation, sound, the Lua automata scripts (`lua-host.md`), or
how automata are chosen and spawned. A replacement may animate however it likes, or not at all, provided it keeps
the records and the hooks.

| # | Component | Interface (IID) | Class ID | City slot | Saved version |
|---|---|---|---|---|---|
| 40 | automata script system | `cISC4AutomataScriptSystem` `0x8A1B6647` | `0x4A1B664E` | 48 | 1 |
| 41 | automata controller manager | `cISC4AutomataControllerManager` `0xCA14C9FC` | `0xCA14CA03` | 47 | 1 |
| 43 | vehicle manager | `cISC4AutomataManager` `0x098F9BF4` | `0xA98F9D3B` | 43 | 3 (2 read) |
| 44 | pedestrian manager | `cISC4AutomataManager` `0x098F9BF4` | `0xC98F9D17` | 44 | 3 (2 read) |
| 45 | aircraft manager | `cISC4AutomataManager` `0x098F9BF4` | `0xC98F9D4A` | 45 | 5 (≤ 5 read) |
| 46 | watercraft manager | `cISC4AutomataManager` `0x098F9BF4` | `0xA98F9D28` | 46 | 6 (≤ 6 read) |

Every save in the corpus (421 cities) holds components 40, 41, 43 and 44. Only 12 hold 45 and 46.

## 1. Encodings used below *[compat]*

All values are little-endian (foundations §2).

| Name | Bytes |
|---|---|
| `u8`, `s8`, `u16`, `s16`, `u32`, `s32`, `f32` | as named; `s8` is a signed one-byte integer |
| `bool` | one byte, 0 or 1 |
| `vec3` | three `f32`: x, y, z |
| `ref` | `u32` object ID; when non-zero, followed by `u32` class ID (foundations §3.4). 0 is null and has no class. |
| `str` | `u32` length, then that many bytes, no terminator |
| `key3` | three `u32`: type, group, instance |
| `time` | a *controller time*: `f32` hour (0 ≤ h < 24), then `s16` day of week (0–6) |
| `curve` | `u32` n, then n × (`f32` x, `f32` y) |
| `bytes` | `u32` n, then n bytes |

"Read checks IID X" means the loader resolves the ref and requires that the object answers `QueryInterface(X)`.
The IIDs used are: occupant `0x26BB767E`, automaton `0xA9B40F05`, controller `0x6A1857F8`, lot `0xE818FCED`,
network occupant `0xA821EF94`, 24-hour clock `0x681BD53A`, model `0xA818EDD8`, callback target `0x452294AA`.

## 2. Component 40: the script system (`0x8A1B6647` / `0x4A1B664E`) *[compat]*

The script system holds the automata groups, occupant groups and controller templates that the Lua automata
scripts define. None of that is saved; it is rebuilt from the scripts at `Init`.

Record: `u16` 1. Nothing else.

- Write (`0x0042E440`, a body shared with the nation) writes the `u16` and returns `GetError() == 0`.
- Read (`0x006B8A70`, shared with the industrial simulator) reads the `u16` and fails when it is not 1.
- Corpus: 421 of 421 records are the 2 bytes `01 00`.

## 3. Component 41: the controller manager (`0xCA14C9FC` / `0xCA14CA03`) *[compat]*

A **controller** is attached to an occupant and makes automata near it: a *generator* spawns them (wildlife in
trees, construction vehicles at construction sites, dogs, crowds, strike pickets), an *attractor* pulls them in
(police at a crime scene, dogs to a hydrant). Controllers are made from named *templates* that the scripts define.

The dedicated [controller-manager owner supplement](automata-controller-manager.md)
specifies initialized teardown, city close ordering, direct reference sites, conditional
occupant removal, queue mutation and tick admission. Its publication boundary and body
ledger distinguish proved owner rules from the remaining AT21 dependencies.

### 3.1 Record (Write `0x005174D0`, Read `0x0051B520`)

| # | Field | Notes |
|---|---|---|
| 1 | `u16` version = 1 | Read does not check it. |
| 2 | `ref` the 24-hour clock | Read checks IID `0x681BD53A`. Always class `0xA7E7F929` in the corpus. |
| 3 | `time` A, the **current** controller time | slot 24 `GetControllerTime` (`0x0051A310`) returns this one (object `+0x34`) |
| 4 | `time` B, the controller time at the **previous animation tick** | object `+0x3C`. A equals B in every corpus save. |
| 5 | `u32` n1, then n1 × occupant entry | the controllers, grouped by occupant (§3.2) |
| 6 | `u32` n2, then n2 × automaton set | §3.3 |

Write (`0x005174D0`) writes A then B. Each is the `f32` hour, then the day as a 16-bit value.

**How A and B advance (AM5).** The two times are the controller manager's copy of the 24-hour clock (clock24.md)
as a fractional hour and a day of the week. Three things change them:

1. **`HourChanged`** (`0x44778230`, handler at `0x005193BF`). It adds 1.0 hour to A (`0x00515080`). The added
   amount is brought into [0, 24) first, but the **result is not normalised**: 23.5 becomes 24.5, and A's day does
   not change. Then every controller of every occupant gets controller slot 24 (`+0x60`) with (A, A − B).
2. **Each animation tick** (`0x86AD10EE`, `DoAnimationTick` `0x00518B80`), in this order:
   1. t = `GetHour()` + `GetMinute()` × (1/60) (clock slots 10 and 13; the factor is the `f32` `0x00A94124`;
      x87 arithmetic, stored as `f32`). Both clock values are unsigned.
   2. If t < A's hour, add 1 to A's day (`0x00518BD6`). Then A's hour = t.
   3. Compute the step delta = A − B (below). The controllers' tick work receives it.
   4. Normalise A (`0x00514FC0`):
      - a NaN hour becomes 0.0;
      - while the hour is below 0, add 24 and subtract a day;
      - while it is 24 or more, subtract 24 and add a day;
      - the day is then brought into 0–6 (add 7 while negative, then mod 7).
   5. The controllers are updated (when the manager is active): first UpdateControllers (`0x00518950`), then the
      tick work, each over the occupants in creation order (simulator.md §5.8; the original's own order is that of
      the occupant hash map, by address).
   6. B = A: the hour and the day (`0x00518DB0`).
3. **Read** stores both times as read; it does not normalise them.

A − B (`0x00515D50`) starts from a copy of A. B's hour is first brought into [0, 24): +24 adds a day to the copy,
−24 subtracts one. That hour is subtracted from the copy's hour (`0x00515100`), then B's day is subtracted from the
copy's day. The result is not normalised.

So A and B differ only between an `HourChanged` and the next animation tick: A = B + 1.0 h, possibly 24 or more.
At midnight the tick sees t (about 0) below A's hour, so it adds the day. A save written in that window keeps the
difference, and a replacement must round-trip both times unchanged.

### 3.2 Occupant entries

Only controllers without **flag 8** (controller slot 12 `CheckFlag(8)`, `0x00517E60`) are saved. Flag 8 marks a
transient controller; such controllers are never written anywhere (also not from automata, §6.1).

Each entry, for each occupant that has at least one controller without flag 8, in the occupants' creation order
(simulator.md §5.8; the original's own order is that of the manager's occupant hash map, by address):

1. `ref` occupant (read checks `0x26BB767E`);
2. `u32` m = the number of its controllers without flag 8;
3. m × `ref` controller (read checks `0x6A1857F8`), in list order.

The original reads any order.

### 3.3 Automaton sets

Each entry: `ref` occupant (`0x26BB767E`), `u32` k, k × `ref` automaton (`0xA9B40F05`), `vec3`, `vec3`, `bool`.
The entries are in the occupants' creation order, as in §3.2.
Only one corpus save (Big City Tutorial) has an entry: a building with k = 0, the vectors (1256, 0, 1041) and
(−1, 0, 0), and `true`.

**Meaning (Q2, answered).** Each entry is the **queue** of an occupant: the automata standing in line at a building.
The manager keeps one per occupant, in a hash map keyed by the occupant (`GetControllerQInfo`, `0x0051B9D0`), and
creates it on first use.

| Field | Meaning |
|---|---|
| k automata | the line, front first. The queue holds one reference to each. |
| first `vec3` | the **queue point**, where the front of the line stands |
| second `vec3` | the **queue direction**, a unit axis vector along which the line grows behind the front |
| `bool` | the queue point has been found. While it is false, the point is searched again at the next use. |

The queue is driven by the controller's state callbacks for state **4**, which the game's own Lua table names
`QUEUE` (§6.2):
- **entering** state 4 (`0x0051BCA0`) appends the automaton at the back if it is not already in the line. It takes
  a reference and makes the previous back automaton its **leader** (automaton slot 49, `0x005184E0`).
- **each tick** in state 4 (`0x0051BB60`):
  1. find the queue point if it is not found yet;
  2. if it still is not found, remove the automaton (slot 13) and stop;
  3. otherwise the target is the queue point when the automaton has no leader. With a leader, the target is the
     leader's destination plus max(leader radius, 1) × the direction;
  4. the target's height is taken from the traffic network map (city slot 40, slot 18) at the target's x and z;
  5. set it as the destination (slot 10).
- **leaving** state 4 (`0x0051BCE0`), or the automaton's removal, takes it out of the line (`0x005172D0`). Its
  follower gets the removed automaton's leader, or no leader if the removed one was at the front. The queue's
  reference is released.

**Finding the queue point** (`0x00516070`), done once until it succeeds:
1. Choose a side.
   - If the occupant stands on a lot (lot manager `GetOccupantLot`, slot 14), take the lot's cell rectangle (lot
     slot 14) in metres, x0·16 … (x1 + 1)·16 and the same for z. The lot's facing (lot slot 23) picks the starting
     side: facing 0 → side 1, 1 → 2, 2 → 3, 3 → 0.
   - Otherwise take the occupant's bounding box and a random side from 0 to 3.
2. Each side is a segment 1 m outside one edge of the rectangle, at height 0, with a direction:

   | Side | Segment | Direction |
   |---|---|---|
   | 0 | x = xmin − 1, along z | (0, 0, −1) |
   | 1 | z = zmin − 1, along x | (1, 0, 0) |
   | 2 | x = xmax + 1, along z | (0, 0, 1) |
   | 3 | z = zmax + 1, along x | (−1, 0, 0) |

3. The candidate point is the segment's midpoint.
4. It is accepted when the traffic network map has a tile at its 16 m cell: slot 14, `SafeGetTileInfoAtCell`, with
   the cell = floor(coordinate / 16) by the 12582912 rounding trick of foundations §18. In other words, the point
   must touch a network.
5. Otherwise try the next side ((side + 1) mod 4), at most four sides in all. When none is accepted, the flag stays
   false.

The corpus entry is side 3 of a one-cell lot at x 1248…1264, z 1024…1040: the point is (1256, 0, 1041) and the
direction is (−1, 0, 0).

Read (`0x0051B520`) restores the line in order, the point, the direction and the flag; it does not recompute the
point. Each automaton keeps its leader in its own record (§6.1 field 8). A replacement that does not animate queues
must still round-trip these entries *[compat]*.

### 3.4 Load

Read sets a *loaded* flag. When the manager later handles `PostCityInit` (`0x26D31EC1`, `HandlePostCityInit`
`0x00518660`), and only when loaded, it visits every saved controller:

1. Look up the controller's template by name (script system slot 17, `GetControllerTemplate(name)`).
2. If there is none: log *Couldn't find automata controller template '<name>' in scripts. Removing controller.*,
   shut the controller down and release it.
3. Otherwise set the template (controller slot 15), `Init` it (slot 3) and add it to the controller grid.

So a save survives a changed script set: unknown templates are dropped, not fatal.

### 3.5 Controller records

Two controller classes occur in saves. Both start with the controller base record.

**Controller base, version 5** (Write `0x00517FD0`, Read `0x005180E0`; Read ignores the version):

| # | Field | Notes |
|---|---|---|
| 1 | `u16` 5 | |
| 2 | `ref` occupant | `0x26BB767E`; the occupant the controller is attached to |
| 3 | `str` template name | empty when the controller has no template |
| 4 | `vec3` position | |
| 5 | `f32`, `f32` | elapsed lifetime and lifetime limit (AT22 receiver-view correction below); 0 in all historical corpus examples |
| 6 | `u32` | 0 in the whole corpus |
| 7 | `u32` flags | mostly 7; `0x87` seen on `dog_make`; never contains 8 (§3.2) |
| 8 | `u32` n, then n × `ref` automaton | `0xA9B40F05`; the automata it currently controls |

**Object offsets and names (Q5, corrected 2026-10-23).** The stream order above is unchanged. Offsets are
given against both views of the object: the primary controller C and the serializable interface S = C + 4,
which Read and Write receive. [The inherited lifecycle contract](automata-controller-lifecycle.md) specifies
the fields' behaviour.

| Record field | Relative to S | Relative to C | Meaning |
|---|---|---|---|
| 2 | +0x08 | +0x0C | source occupant reference |
| 3 | name object +0x24 | name object +0x28 | getter returns template+4 when present, else embedded name object; not a newly owned character buffer |
| 4 | +0x10,+0x14,+0x18 | +0x14,+0x18,+0x1C | stored position; live getter delegates to occupant when occupant exists and flag 0x20 is clear |
| 5a | +0x4C | +0x50 | binary32 elapsed lifetime, incremented by scaled animation delta, not integer 1 per call |
| 5b | +0x50 | +0x54 | binary32 lifetime limit, **saved**; not the age field |
| — | +0x54 | +0x58 | activation-delay elapsed; not saved in this base record |
| 6 | +0x38 | +0x3C | selected sound ID (§6.1 of the lifecycle contract); historical corpus value 0 retained; not a returned playback handle |
| 7 | +0x3C | +0x40 | flags u32; historical corpus mostly 7 / dog_make 0x87 retained |
| 8 count | +0x44 | +0x48 | controlled-automata count u32 |
| 8 list storage | +0x40 | +0x44 | list sentinel pointer; node payload at node+8 |

CheckFlag/SetFlag therefore use the real flags word C+0x40, not the list pointer. IsExpired is
`lifetime > 0 && elapsed >= lifetime`; tick deactivation needs strict `elapsed > lifetime`. Historical corpus
measurements and the CheckFlag(8) null-reference rule of §§3.2/6.1 are unchanged.

**Generator** (class `0x4A1628C5`, Write `0x005183D0`, Read `0x00518450`): `u16` 4 (Read requires 4), the base
record, `time` (period), `time` (second period), `u32`, `u32`. Corpus: 291,346 records; names `wildlife_generator`
(291,095), `construction_vehicle_generator`, `dog_make`, `crowd_sm`, `transit_strike_gen_1`/`_2`/`_3` and others.
Both `u32`s must round-trip. Their meaning (Q5, answered from the Mac build's `ChooseAutomataGroup` and
`TriggerGenerator`):
- the first is the **current automata group ID**: the group the generator is now spawning. The corpus values
  16,674–16,697 are `0x4122`–`0x4139`, automata group IDs like the riot's `0x4107`. 0 means none chosen yet.
- the second is the **number of automata spawned** from that group so far.
- The generator picks a group at random from its template's list. It keeps it until the count reaches a random
  target drawn from the template's range; that target is not saved. Then it picks again and resets the count to 0.

Each spawn calls `CreateAutomaton` (§7.2) with this group, flags `0x2000002` and the template's priority byte.

**Attractor** (class `0x6A1628BA`, Write `0x00518230`, Read `0x00518270`): the base record, `u16` 1 (read and
ignored), then a `curve` (attraction against distance [inferred]). Corpus: 196 records; names `dog_pull`,
`police_attract` and others; curves like ((0, 60), (50, 60)).

Corpus result: all 291,346 generators and 196 attractors parse exactly to their ends. Occupant classes behind
the controllers: flora `0xA9C05C85` (291,095), prop `0x2977AA47` (104), construction `0xA97F909E` (43),
building `0xA9BD882D` (35), network `0xC9C05C6E` (1).

## 4. The automata manager base record (version 6) *[compat]*

All four managers start with this record (Write `0x0050CB80`; Read wrapper `0x005108E0`, body `0x0050FEB0`).

| # | Field | Notes |
|---|---|---|
| 1 | `u16` version = 6 | Read accepts ≤ 6 |
| 2 | `s32` max count | Read stores it directly into the max (object `+0x3C`, `0x0050FEB0`). It does **not** call slot 23 `SetMaxCount`, so the hard limit (`+0x40`) keeps its current value |
| 3 | `bool` active | slot 5 `Activate` / slot 6 `IsActive` |
| 4 | `bool` | the time-of-day scaling flag [inferred] |
| 5 | `u32` n, then n × `ref` automaton | `0xA9B40F05`; the manager's automata, in list order (front first) |
| 6 | only when version ≥ 5: `u32` m, then m × (`u32` occupant group ID, `u32` count) | the **source building counts** (AM12, below). Not the slot 31 group sets |

Read sets the loaded flag, and an *old-version* flag that is true when the version is below 6. Write always
returns true, and the base Read returns true without checking the stream error *[quirk]*. The wrappers do check it
(§4.1).

**The source building counts (field 6).** The map is keyed by occupant group ID: the building groups that the
automata scripts track (script system slot 18 `GetTrackBuildingGroups`). The value is the number of complete
buildings in the city that belong to the group.
- **Read by** slot 36 `GetSourceBuildingCount(group)` (`0x0050BE30`); slot 37 reads it for an automata group's
  source building group. Groups `0x1402` and `0x1406` are special: they ask the landfill developer instead and never
  look at the map. An unknown key gives 0.
- **Seeded** at `Init` (`0x0050F930`): each tracked group missing from the map is added with count 0.
- **Counted up** (Mac build `HandleNewBuilding`): a building that is inserted complete (completion ≥ 100 %), or
  that becomes complete, adds 1 to every key among its occupant groups.
- **Counted down** (Mac `HandleRemoveOccupant`): a removed occupant subtracts 1 from each key whose count is not 0
  and whose group the occupant belongs to.
- **Old saves.** When the read base version was below 6 (the old-version flag), `PostCityInit` (test at
  `0x0050F341`) sets every count to 0 and recounts: +1 per (occupant, tracked group) over the occupants of the
  tracked groups.

A replacement must round-trip the pairs. The order is hash order in the original and is free *[choice]*.

### 4.1 Versions on re-save, version checks and stream errors (AM1, AM2, AM11) *[compat]*

**Re-save.** Every writer in this layer writes a **constant**, its current version, and never the version it read:
- manager base 6 (`0x0050CB80`), traffic manager 3 (`0x00553570`), aircraft manager 5 (`0x004FB5B0`), watercraft
  manager 6 (`0x0056FA70`);
- automaton base 8 (`0x0050AC0D`), state base 3 (`0x00509D0A`);
- pedestrian 5 (`0x0054465A`), vehicle 2, emergency vehicle 2 (`0x0052FA6B`), trailer 2 (`0x005562AA`), traffic
  automaton 2.

The fields that are missing from an old version are written from the live object:
- a version-2 traffic manager re-saves as version 3, with an incident list (empty unless incidents occurred);
- a version-4 base re-saves as version 6, with the source building counts that `PostCityInit` recounted;
- a version-7 automaton re-saves as version 8, with its group ID;
- a version-2 state re-saves as version 3, with its final `u32`.

A replacement **must** save the live game this way: current versions, every field from the live state. A record
codec may keep a read version so that a decode-and-encode test round-trips byte for byte *[choice]* (§14 test 4).
The original reads both forms, so either output loads.

**Read version checks** (the checks the original makes; a mismatch makes that object's Read return false):

| Record | Accepts | Check at |
|---|---|---|
| script system | exactly 1 | `0x006B8A70` |
| controller manager | anything (not checked) | `0x0051B520` |
| controller base / attractor's `u16` | anything / anything | §3.5 |
| generator | exactly 4 | §3.5 |
| manager base | ≤ 6 | `0x0050FEB0` |
| traffic manager | 2 or 3 | `0x00554410` |
| traffic incident list `u16` | exactly 1 | `0x00553E90` |
| aircraft manager | ≤ 5 (below 5: base-only record) | `0x004FBE91` |
| watercraft manager | ≤ 6 (below 6: base-only record); above 6 fails at `0x00574A26` | `0x00574A0B` |
| automaton base | ≤ 8 | `0x0050DFED` |
| occupant base inside it | exactly 4 | `0x005E8D75` (lot-records §2.3) |
| state base | ≤ 3 | `0x00505E50` |
| traffic automaton | exactly 2 | `0x00554D37` |
| pedestrian | exactly 5 | `0x005446D5` |
| vehicle | exactly 2 | `0x0055BCF0` |
| emergency vehicle | exactly 2 | `0x0052FAE0` |
| patrol car | exactly 2 | `0x00543E05` |
| trailer vehicle | exactly 2 | `0x00556C47` |

**Stream errors.** The manager base Read and the object Reads store each field as they read it and never roll
back. Whether a cut record is reported depends on the wrapper:
- these return "no stream error" (`GetError() == 0`, stream slot 18) at their end:
  - the controller manager (`0x0051B9B8`);
  - the traffic manager at version 3, through the incident reader (`0x00553E90`);
  - the aircraft manager at version 5 (`0x004FBFC3`);
  - the watercraft manager at version 6 (`0x00575153`);
- the base-only versions return the base Read's result, which is true: traffic version 2, aircraft below 5 and
  watercraft below 6.

So a cut record of a current version reads partially and returns **false**. A cut base-only record returns true
*[quirk]*. A replacement may fail a short record atomically (all or nothing) *[choice]*. Every case where the original
returns false stays false. The only case that changes is a cut base-only record, which no save contains.

### 4.2 PostCityInit: retained native automata (AD6) *[compat]*

`PostCityInit` (`0x26D31EC1`) dispatches **primary manager slot 56**, not a periodic tick.
The message target is the +0x0C view: the base message handler adjusts back to the primary
manager before dispatch. The slot takes no stack arguments (`thiscall`, ret 0); its result
is not consumed by the message handler, which returns true. Base implementation
`0x0050F2C0` is inherited by aircraft/watercraft; pedestrian/vehicle use traffic override
`0x0054F7B0`. The watercraft message handler has no separate PostCityInit branch: after its
own message tests it delegates to base (`0x0057558B`–`0x0057558E`).

The instruction-checked base order is:

1. If the **loaded byte** is set (Read sets it at `0x0050FED4`), walk retained automata
   in list order. These are primary `cISC4Automaton*` views resolved by IID `0xA9B40F05`.
   For each, call slot 31 `CheckAutomataFlag(u32 0x8000)`, one stack word, result AL.
2. True: unlink/free that manager-list node and decrement the u32 count; call manager
   slot 62 `UnplaceAutomaton(A)` (borrowed primary automaton pointer); then A slot 4
   `Shutdown()` and slot 2 `Release()`, each with no argument. Continue with the saved
   next list node. False: call A slot 3 **`Init()` with no arguments**, result AL ignored,
   then continue. This is not AddAutomaton or a second creation reference. The manager
   already owns the reference acquired by Read; a surviving object's reference remains
   in the existing list (`0x0050F2D8`–`0x0050F33F`).
3. Still inside the loaded branch, when the old-version byte is set, recount source
   building groups as specified above (§4). The restoration of each surviving automaton
   precedes that recount (`0x0050F341`).
4. Whether loaded or not, call manager slot 48 `UpdateViewRect()` **once at the end**
   (`0x0050F67C`–`0x0050F686`). This copies current to previous view rectangle, reads
   the global view bounds, expands each side by binary32 64, and calls slot 60 UpdateZoom
   (`0x00506CD0`–`0x00506D44`). UpdateZoom runs the activation rule of
   automata-managers §2.2 and, while active, calls each automaton's slot 74 with
   `(int32 zoom, int32 rotation)` in list order (`0x0050A59E`–`0x0050A5C1`).

Neither base PostCityInit nor the traffic override clears the loaded byte. Re-delivery
therefore runs the restoration again; ordinary automaton Init guards its initialized state.
Base Read appends the acquired reference to its list directly (`0x0050FF40`–`0x0050FF7E`):
it does not invoke the usual AddAutomaton helper or duplicate that helper's callbacks.
PostCityInit itself makes no separate AddCallback call.

#### 4.2.1 Surviving objects, models and initialization

The list walk is over **every entry of the base manager list**, including subclasses and
objects also referenced by controllers, trailer chains or an aircraft-manager queue. It
does not recreate objects from their class IDs or substitute those other lists for the
base list. Flag `0x8000`, rather than class or trip type, decides removal. In particular,
ordinary watercraft Init sets this flag (watercraft §3), so a loaded boat retaining it is
removed at step 2; the watercraft Init path below applies only to a surviving entry.

**What the surviving native object's Init does.** The manager calls the primary automaton
slot, not occupant Init on an adjusted pointer. Base automaton Init first asks its +4 occupant
view whether initialized; if false it calls primary slot 97 (no args, AL success), initializes
the effects list only on success, then calls occupant Init **even if slot 97 failed**, returning
the slot-97 result (`0x005101D0`–`0x0051020C`). The native object owns these model/effect
operations. Traffic Init uses this base result; when successful and its byte +0x3C is clear,
it refreshes behavior through slot 105 unless flag `0x4000` is set, then calls slot 134 unless
flag `0x2000` is set (`0x0054D790`–`0x0054D7EF`). Watercraft Init's separate effects,
timer, position and heading setup is watercraft §3; it is reached through the same virtual
slot 3. A replacement manager retaining those native concrete objects should call that
interface operation rather than reconstructing their internals or marking them initialized.

The concrete entry matters even when the common base is already known:

| Concrete entry | Work before/around inherited Init |
|---|---|
| Pedestrian `0x005444F0` | If not initialized, call its slot 158, then tail-call traffic Init. |
| Vehicle `0x0055B9B0` | If not initialized, set automata flag 8 from the global **byte** `0x00B0A0A8`, call slot 158, then traffic Init. Afterwards, if city-time slot 9 returns true or byte `0x00B2296C` is nonzero, call vehicle slot 176 `(true,false)`, even if inherited Init failed. Return the inherited AL result. |
| Trailer `0x00555A30` | Set flag `0x10000000` true, then call vehicle Init. |
| Aircraft base `0x004FA770` | Clear its byte `+0x1E8`, set flag `0x2000` true, then tail-call automaton Init. Airplane/heli concrete Init wraps this with its own runtime reconstruction (aircraft §§3.2, 4.1.1). |
| Watercraft `0x0056DE40` | When not initialized, base Init, drive parameters, effects, subscriptions and runtime reset in watercraft §3; base failure does not stop those steps. |

For the base model load (automaton slot 97, `0x0050E8B0`), the object, not the
manager, owns the runtime animated-model vector. When empty, it fills absent model keys
from the saved first key if that key is nonzero, then loads keys in vector order. Each
successful factory output is appended as its acquired reference. A failed factory returns
false immediately, without undoing earlier outputs. For each loaded animated model it
gets its model-instance view, sets animated-model slot 18 `(true,0)`, positions the instance,
sets instance slot 4 to the object's **occupant** view, sets instance slot 14 to 3, then
passes current zoom/rotation to animated-model slot 8 (`0x0050E920`–`0x0050EA51`).
The first instance supplies the radius: half its third-axis bounding-box extent, capped at
binary32 32 (`0x0050E9CB`–`0x0050E9E9`; x87 subtraction/multiply, binary32 field).
The auxiliary-model property `0xCC21420D` is processed in property order; only values
1, 2, 3 call slot 99, with a temporary output reference released afterwards.
Finally call automaton slot 83 with binary32 zero, then slot 106 to bind/clear its model
processor. The returned AL is whether a main model was newly loaded, not an allocation
token (`0x0050EA57`–`0x0050EB12`). InitEffectsList subsequently associates the first
model with the object's effects and position/orientation (`0x0050EBB0`–`0x0050EC0C`).

#### 4.2.2 Calls back into services and the replacement manager

These are synchronous calls made **inside the surviving native Init**, before the
manager's source recount or view update. The native object owns them; the manager must
not separately consume their random draws or repeat their state transitions.

* Traffic slot 105 (`0x005096B0`), gated by successful base Init, clear byte `+0x3C`
  and absent flag `0x4000`, gets the controller manager from city slot 47. It calls that
  manager's slot 22 with `(occupantView, &compositeVec3, &distance, &strength)` in that
  order; all outputs are binary32, the vector has three components. The result is a
  **borrowed controller pointer**, not a boolean or a newly allocated record. Native
  Init takes references for its temporary and stored controller, releases a replaced
  controller, and releases the temporary at the end. A null result is allowed. With
  a controller, its slot 32 receives `(distance,&position)` and returns a borrowed
  behaviour record or null. Only a non-null record consumes one RandomDoubleUniform
  draw; record `+8` is the binary32 admission probability. A draw above it skips the
  subsequent state reconciliation (`0x00509778`–`0x005097BE`). State-stack removal/
  insertion then uses automaton slots 53, 55, 56 as appropriate to that record.
* Traffic slot 134 (`0x00550B60`), gated by successful base Init, clear byte `+0x3C`
  and absent flag `0x2000`, reconstructs the initial network path near the current
  position. The sentinel position test precedes service access. It gets the traffic
  network from city slot 40 and **its owning automata manager** through automaton
  slot 174. Vehicle slot 174 tail-calls city slot 43; pedestrian slot 174 tail-calls
  city slot 44 (`0x0055BF60`, `0x00544D90`). These are borrowed primary manager views.
  After finding a network occupant and a non-null path-info view, it calls that
  manager's **slot 68** (`+0x110`) with the primary automaton pointer, ret 4, and
  consumes EAX as a **32-bit traffic type** (`0x00550C2A`–`0x00550C56`). It does not
  QI for an extended interface and does not restrict itself to slots 0–37.
  Slot 68 first tests automaton slot 141: false returns the manager's signed 32-bit
  default type; true tests occupant groups `0x422F`, `0x422E`, `0x422D`, `0x422C`,
  `0x4208`, `0x4217`, in that order, returning respectively 7, 7, 6, 6, 4, 4, else 3
  (`0x0054D6C0`–`0x0054D770`). Ordinary vehicle's slot 141 returns false. A null
  manager or a shorter vtable is not handled as failure. Network/path services,
  not the automata manager, supply the borrowed path collection used next.

There is **no random draw in the outer PostCityInit/list/recount/view/repair code**.
For an uninitialized ordinary vehicle or pedestrian, slot 158 consumes exactly one
shared `cRZRandomSuper` draw, even for equal range endpoints. Vehicle uses binary32
range endpoints `0x00B21CF0/CF4`, pedestrian `0x00B21CE8/CEC`, promoted to binary64
arguments. RandomDoubleRangeUniform computes `U*(upper-lower)+lower` with x87
intermediates and stores one binary32 lateral offset at object `+0x200`
(`0x0055B690`–`0x0055B6BA`, `0x00544620`–`0x0054464A`, `0x00913A46`–`0x00913A5B`).
This is the shared source `0x00B21D78`, **not CRT rand()**. Each such range/uniform
call advances RandomUint32Uniform once (`0x00913A29`–`0x00913A45`). The conditional
controller-behaviour draw above follows the lateral-offset draw. Already initialized
vehicle/pedestrian objects skip both. The Init call to vehicle slot 176 has second
argument false, so it skips that slot's optional random delay (`0x0055DC02`–`0x0055DC46`).
Further behaviour-state entry callbacks belong to the selected state's contract (§6.2).

#### 4.2.3 Activation, placement and callbacks

The final UpdateZoom is not an unconditional placement pass. Inactive-to-active or
active-to-inactive invokes manager slot 5 `(bool)` with a four-byte stack slot, ret 4,
return AL. Activate stores the active byte first, acquires the current occupant filter
through global view-system slot 18 (output reference), then walks the base list in
order. For each non-null A it calls **A slot 9 Show**, passing false when a filter exists
and rejects A's occupant view, otherwise the new active byte. It releases the filter
after the walk and returns that byte (`0x0050A6B0`–`0x0050A739`). If the saved active
byte already agrees with UpdateZoom's decision, Activate is not called. Active entries
still receive A slot 74 `(s32 zoom,s32 rotation)`, ret 8, in list order.

Native Show sets occupant visibility with `(bool,false)` and, only if initialized,
calls A slot 71 with the same boolean. Hiding stops object effects; showing refreshes
them (`0x0050E390`–`0x0050E3E7`). Slot 71 ensures placement through slot 70 when
showing an unplaced object; slot 70 inserts the **occupant view** into the occupant
manager (global `0x00B43D0C`, slot 20 `(occupant,false)`, AL success). A failed insert
does not set placement. Slot 71 adds each model instance to the graphics system through
slot 28 and sets occupant flag 2 only when all required additions succeed; hiding removes
instances through graphics slot 29 and clears that flag (`0x00507E60`–`0x005080B7`).
Manager slot 61 is a separate guarded place operation: require occupant flag 4 and
clear placement bit, then A slot 70 `(true)`; only if AL succeeds call A slot 71 `(true)`
when needed. It is **not** called by the survivor branch of PostCityInit. Unplace
(manager slot 62) calls A slot 71 false before A slot 70 false when their respective
bits are set (`0x005073D0`–`0x00507462`).

The loaded callback-target list is already restored by automaton Read (§6.1 field 13).
The manager does not register itself as another callback or rerun AddAutomaton. Occupant
Init sets its initialized bit before its own initialization notification/update
(`0x005E8C90`–`0x005E8CC7`); model-instance attachment, graphics membership and effects
are work of the native automaton, not replacement-manager container writes.

#### 4.2.4 Traffic-only second pass and supported interface boundary

After the entire base operation, if loaded, walk the remaining
primary traffic-automaton pointers in list order (`0x0054F7BD`–`0x0054F88B`). For a stored
network occupant, obtain its path-info view with network slot 49; retain the new view before
releasing a different old view. If non-null, call path-info slot 3 with `(u32 savedPathID, 0)`
and keep the returned borrowed path-collection pointer. Then call automaton slot 150
(no args, AL flag-8 predicate); if true find its **leader automaton** through slot 148
(`FindLeader`, no args, borrowed primary automaton pointer in EAX),
retain it before releasing a different old view. The original performs these reference swaps
in concrete traffic-automaton fields (+0x1EC network, +0x1F0 path info, +0x1F4 path ID,
+0x1F8 collection, +0xB0 leader). These offsets are evidence, **not an authorized replacement
layout or a public setter contract**. The common Init call is public; this later repair is not
expressed as a public automaton restoration call in the traced body. This corrects the
earlier AD6 partial's description of `+0xB0`/slot 148 as a model. `+0xB0` is the same
leader reference saved in §6.1 field 8; slot 150 delegates to CheckAutomataFlag(8)
(`0x00504FA0`). There is no AddRef on the path collection. A null network leaves the
old path fields alone; a null newly obtained path-info clears/release-swaps that view
but leaves the collection alone. A false flag-8 predicate leaves the old leader alone.
Equal old/new reference pointers make no reference-count calls.

**Replacement boundary.** Common public operations are `Init/Shutdown/Release` on the
primary automaton (IID `0xA9B40F05`), and the base manager's public Activate on IID
`0x098F9BF4`. They do not encompass the whole second pass. The native body reads private
saved network/path state and performs those swaps itself. Existing vehicle path-setting
slot 132 (`0x0055D5A0`) is a different operation: it also changes path progress/geometry
and notifies downstream state; it is not the reference-only loaded repair. Thus calling
Init alone, or substituting AddAutomaton or a path setter, does not reproduce the native
PostCityInit contract. Conversely, an implementation retaining native traffic automata
must supply the extended **manager slot 68** ABI above when native Init reaches it.
The city's common-manager IID and a 38-entry interface table alone do not supply it.
The original exposes no version-checked atomic restore service in this path. These
facts specify the boundary; they do not authorize a private-layout adapter, direct native
same-owner forwarding, or replacing a native object's allocator-owned containers.

#### 4.2.5 Allocator preconditions and what the save fault establishes

Native objects, their property holders and their owning references must retain their
native allocation/deletion pairing across restoration. A saved reference is an interface
reference, not permission to recreate its storage with a different heap and later call
the native deleting destructor. In particular:

* GlobalHeapAlloc adds a **four-byte header**, chooses the first power-of-two class
  of at least `requestedSize+4` (8 through 2048 bytes, classes 0 through 8), and stores
  the class in the high byte. Larger allocations use the CRT path with a different
  header tag (`0x005B9FC0`–`0x005BA00B`).
* GlobalHeapFree serializes through the native global lock, reads the **byte at
  payload−1**, and for classes below 9 passes `payload−4` to that class's native pool;
  other tags go to the CRT deallocator (`0x005E5620`–`0x005E5668`). There is no ownership
  validation or recovery for a foreign/malformed header.
* For the 8-byte class, free rounds the header pointer down to a 2048-byte block,
  links the slot into the block's free chain at `+8`, and decrements its **u32** count
  at `+12`. Zero tail-calls the block-unlink helper (`0x005E5A40`–`0x005E5A7E`).
  That helper requires valid writable circular links at block `+0/+4`, rewrites both
  neighbour links, updates the pool cursor if necessary, then calls the pool's block
  release operation. It does not discover or rebuild missing allocator metadata
  (`0x0090D031`–`0x0090D055`).

Braus log
`golden-runs/compare-20261028-063556-5c19c930/result/game.log:5295`–`:5333`
reports a terminal-save fault in `0x0090D031`, an allocator node-unlink helper called via
GlobalHeapFree and property-holder destruction. The failing write at `0x0090D03F` uses
the first dword of the supplied node as a link and writes link+4. The logged node is
`0x2007A800`, first-link register `0x00AA41D8`, so the attempted target is `0x00AA41DC`.
The previous-link register is `0x00AA41BC`. The first value, `0x00AA41D8`, is specifically
the **cSC4Occupant vtable address**, verified in the binary; it cannot be a writable
allocator-neighbour block. The stack returns through GlobalHeapFree, the deleting
property-holder destructor `0x005FF250` and occupant Release `0x005E8DF0`; deeper it
includes trailer Shutdown (`0x00556400`). Release destroys the holder only after its
native reference count reaches zero. This proves incompatible allocator-link state at
the eventual free, not a model-position store, an omitted random draw, or which earlier
operation damaged/misclassified that block. The Jinae report has the same fault address
and target (`compare-20261028-063628-746bb98f/result/game.log:6974`–`:7006`).

AD6's restoration and native caller/allocator **contract** is answered here. No repaired
DLL, successful save or causal diagnosis is claimed. To identify the first bad operation
in either cited run still requires runtime allocation/header/reference history for the
freed holder and its containing block, including their last valid state; the terminal
stack alone cannot supply that history. No game was run for this answer.

## 5. The four managers

### 5.1 Vehicle (43) and pedestrian (44): the traffic automata manager, version 3

Write `0x00553570`, Read `0x00554410` (accepts 2 or 3).

| # | Field | Notes |
|---|---|---|
| 1 | `u16` 3 | |
| 2 | base record (§4) | |
| 3 | `u32` | the manager's **default path type**, a constant set by the constructor: vehicle manager 1 (`0x0055FB50`), pedestrian manager 2 (`0x00545500`). Read discards it. |
| 4 | `u32` | the **layer ID** under the manager's layer cursor, or 0 when the cursor is at the end (below) |
| 5 | only in version 3: `u16` 1, `u32` count, count × incident | the traffic incidents (accidents) |

**The layer list and its cursor (field 4, AM3).** Each traffic manager keeps a fixed, ordered list of the traffic
layers it populates, built by its constructor. Each node holds a layer ID and an accumulated time. The cursor
marks the layer that the next population pass serves (Mac `PopulateVisibleArea`).

| Manager | Layer IDs, in list order |
|---|---|
| vehicle (`0x0055FB50`) | 1, 2, 3, 5, 4, 7, 6, 8, 9, 10 |
| pedestrian (`0x00545500`) | 1, 7, 8 |

- A **new** manager's cursor is on the first node, so it writes **1** (both managers). The empty-city corpus
  records all hold 1 (§14 test 4).
- When a population pass starts, the cursor moves to the next node, wrapping from the end to the first node. It
  keeps moving while that layer is disabled in the tuning's layer mask, unless the list has one node.
- Write (`0x00553570`) writes the cursor node's layer ID, or 0 when the cursor is at the list end.
- Read (`0x00554410`) puts the cursor on the first node. If the value read is not 0, it walks forward to the first
  node with that ID. When no node has that ID, the cursor stays at the end, and a re-save then writes 0.

A replacement that does not populate layers must still write the value it read, or 1 for a new manager.

An incident (Write helper `0x00552A80`, Read helper `0x00553E90`): `s32` x, `s32` z, `u32` type, 8 × `s32`, `ref`
network occupant (read checks the **occupant** IID `0x26BB767E`), `bool`, `u32`. The single corpus incident is
(x 47, z 22, type 2), on network occupant class `0xC9C05C6E`, `bool` 1, last `u32` 6264. The list's leading `u16`
must be 1; otherwise the reader returns false without reading further (§4.1).

Corpus: 410 of the 421 pairs are the 22-byte version-2 record with a version-4 base (an empty city's managers);
11 are version 3 with base 5 or 6. Max counts seen: vehicle 336, 199, 750, 168; pedestrian 384, 512, 256.

### 5.2 Aircraft (45), version 5

Write `0x004FB5B0`, Read `0x004FBE70`. Read: a version above 5 fails; a version below 5 means the whole record is a
base record of that version (the leading `u16` is the base's own).

1. `u16` 5;
2. base record;
3. `u32` count, count × (`ref` occupant `0x26BB767E`, `ref` automaton `0xA9B40F05`): the aircraft parked at or
   assigned to airports. The original keeps these in a map ordered by pointer, so its write order is arbitrary;
   a replacement may write any fixed order *[choice]*;
4. `u32` n, n × (`vec3`, `u8`).

Corpus: 12 records, 11 of version 5 and 1 of version 4.

#### 5.2.1 Retained airport/aircraft references and teardown (AT24) *[compat]*

**The map owns one reference to each airport key and one to each aircraft value. Its payload references
are released by aircraft-manager `Shutdown`, not by its deleting destructor.** This rule is specific to
the aircraft manager; the controller-manager removal rule in §7.1 is not evidence for this lifecycle.
The following offsets are from the complete manager object `M`. The serializable interface passed as
`this` to Read is `M + 8`, so Read uses `this + 0xA80` for the map at `M + 0xA88`.

| Entry point / call signature | Native role and evidence |
|---|---|
| Constructor `0x004FBB40`, `this=M`, no stack arguments | Calls base constructor `0x005125E0`; installs primary vtable `0x00A94850` and serializable vtable `0x00A94828`. Builds an empty map at `M+0xA88` (header there, count at `+0xA8C`), an empty runway list at `+0xA94`, and nulls the filter at `+0xA98` (`0x004FBB49–0x004FBBB7`). |
| `Init()`, primary slot 3, `0x004FA9E0` | With initialized byte `M+0x34` clear, calls base Init, creates/AddRefs the filter, sets the global tuning pointer and eventually sets this byte to 1 (`0x004FA9E3–0x004FAA38`). Repeated Init returns true. |
| `Read(stream)`, `0x004FBE70`, serializable `this=M+8`, callee pops 4 bytes | Version 5 reads the base record, then the airport/aircraft pairs. Earlier versions use the base-version reader directly; later versions fail (§5.2). |
| `Shutdown()`, primary slot 4, `0x004FB7D0`, `this=M`, no stack arguments | If initialized, releases map payloads and clears map nodes, clears runway nodes, releases/nulls the filter, calls base Shutdown `0x005110A0`, then clears the global tuning pointer. Returns true, including when already uninitialized. |
| Final manager `Release()`, primary slot 2, `0x00507B50` | Count `M+0x38 > 1`: decrements only. Count at most 1: calls virtual slot 4, then virtual slot 38 with deleting flag 1, and returns zero (`0x00507B53–0x00507B74`). Thus final Release can enter Shutdown without an earlier explicit call. |
| Deleting destructor, primary slot 38, `0x004FBCF0`, `this=M`, one 32-bit flags argument | Destroys runway storage, destroys map storage through `0x004FB780`, calls base destructor `0x00512780`, and frees M if flags bit 0 is set (`0x004FBCF3–0x004FBD23`). It does not call aircraft Shutdown or Release on the airport/aircraft payloads. |

The slot identifications follow the constructor-written primary vtable: export data references
`0x00A9485C -> 0x004FA9E0`, `0x00A94860 -> 0x004FB7D0`, `0x00A948E8 -> 0x004FBCF0`,
and `0x00A94904 -> 0x00511990` (slot 45). Names attached to generic STL helpers in the export are
not their ownership semantics: `0x004FB270` recursively frees tree nodes without touching their
key/value pointers (`0x004FB280–0x004FB29B`); `0x004FB780` uses it and frees the sentinel.
The base destructor also destroys storage; it does not provide a deferred aircraft-map Release pass.
The shared map lookup uses unsigned 32-bit key comparisons (`0x00497A17–0x00497A33`), confirming
that the native traversal order is ascending pointer value, rather than insertion or stream order.

**Acquisition and loaded lifetime.** Each version-5 pair makes two calls to the stream overload
`SC4ReadGZSerializable(stream, iid, out, 0)` at `0x005D9730`: airport IID `0x26BB767E`
(`0x004FBEE7–0x004FBEFB`) and automaton IID `0xA9B40F05` (`0x004FBF00–0x004FBF0C`).
It then uses the returned airport pointer as the map key and stores the returned automaton pointer
as the value (`0x004FBF14–0x004FBF24`). There is no additional AddRef and no temporary Release of
either returned interface in this loop. These are the references retained by the map.

This is a transfer of acquired interfaces, not a borrowed object-table lookup. The stream helper
queries the serializable segment IID `0x4999A8F5`, calls the segment overload `0x005D96C0`, and
releases its temporary segment interface (`0x005D9748–0x005D9782`). The segment overload reads a
serializable reference through slot 26 (`+0x68`), queries that object for the requested IID, and
releases the temporary serializable interface (`0x005D96D1–0x005D970E`). The successful QI result
is left in `out`. Airport occupant QI AddRefs the result (`0x005E8A0C–0x005E8A14`). The observed
airplane primary vtable `0x00A94B38` uses aircraft QI `0x004FA7A0`, which delegates this IID to
automaton QI `0x00505D80`; IID `0xA9B40F05` returns the primary interface and AddRefs it
(`0x00505DC5–0x00505DCA`, `0x00505E01–0x00505E09`). Ending a loaded segment therefore does not
consume either retained map reference.

The base record separately reads every automaton list entry with the same automaton IID and retains
that QI result in a list node (`0x0050FF40–0x0050FF7E`). Thus a plane present in both the base list
and the airport map has **two manager-owned references**, plus any object-table or other owners.
Runtime airport insertion independently confirms the map ownership: after successful creation via
manager slot 15, it AddRefs the airport and plane, then stores the plane in the map
(`0x004FC1AB–0x004FC1D6`, reachable from occupant-insert handler `0x004FBFE0`).

**Release events and exact relative order.** These operations are distinct:

1. `PreCityShutdown` `0x26D31EC2` reaches the base message handler, which calls manager slot 45
   (`0x005069F7–0x005069FD`). For aircraft that slot is base `RemoveAllAutomata` `0x00511990`.
   It releases automaton sets first, then walks the base automaton list: unplace, stop camera follow
   when applicable, automaton `Shutdown()` (slot 4), automaton `Release()` (slot 2), then clear the
   list (`0x005119B7–0x00511A72`). **This does not clear the airport map.** A plane still in the map
   survives that base-list release unless another operation has already removed its map entry.
2. An accepted `RemoveOccupant` message `0x99EF1143` invokes aircraft handler `0x004FB8D0`
   (`0x004FC72A–0x004FC755`; the existing occupant-property filter still applies). On a map hit,
   it calls the mapped plane slot 13 (`+0x34`) with false, erases/frees the map node and decrements
   its count, then releases **plane first, airport second** (`0x004FB8FE–0x004FB940`). It does not
   call plane slot 4 here. A miss performs no map-payload releases. This order differs from bulk Shutdown.
3. Aircraft-manager `Shutdown`, if initialized, walks the map in its pointer-key order. For **each**
   entry it fetches the plane pointer, releases **airport first** (`0x004FB7F4–0x004FB7FC`), then
   releases **plane** (`0x004FB7FF–0x004FB803`). There is no plane Shutdown or detach call in this
   loop. Only after all payloads are released does it free all map nodes and reset header/count
   (`0x004FB84F–0x004FB874`). It then clears runways, releases/nulls the filter, and calls base
   Shutdown (`0x004FB877–0x004FB8BE`). Base Shutdown calls slot 45 again **after** the map releases
   (`0x005110B1–0x005110B4`), and eventually clears `M+0x34` (`0x00511257`).
4. City `Shutdown` explicitly calls aircraft Shutdown and then aircraft Release at city field
   `+0x1D8`, nulling that field (`0x0045B266–0x0045B280`). A final Release consequently sees an
   already uninitialized manager: it does not repeat payload releases. The deleting destructor
   that follows frees the now-empty storage. This precedes occupant-manager Shutdown: the occupant
   manager is city field `+0x12C` (`GetOccupantManager`, `0x00423710`), whose slot 4 is called later
   at `0x0045B833`. Normal city teardown therefore does not rely on occupant-manager shutdown
   notifications to empty the aircraft map. Bypassing Shutdown and directly invoking that
   destructor on a nonempty map frees nodes without balancing their payload references; this is
   not the public Release lifecycle.

**Final-owner conditions.** A map release is allowed to destroy a plane when it holds the last
reference. For example, start with one base-list reference, one map reference and no other owners
after loaded-segment disposal. PreCityShutdown shuts down the plane and drops its list reference
(2 to 1); aircraft Shutdown subsequently drops its map reference (1 to 0). If PreCityShutdown was
not delivered and both references remain, aircraft Shutdown drops the map reference first (2 to 1),
then its base Shutdown shuts down/releases the list entry (1 to 0). If an external reference is
retained, neither manager release destroys the plane. An already removed airport entry contributes
no later map release. These conclusions count actual reference events, not occurrences of a pointer
in a save or the number of observed warnings.

**AT24 audit boundary.** Three `cSC4Airplane` warnings at generic `HostRelease:53` in both
`train84-ports-union-reload-20261022-a` and `train84-ports-union-control-20261022-a` establish a
shared existing path. They do not identify which native lifetime event the replacement performed.
The native contract permits a final owning release **during aircraft Shutdown**, including Shutdown
entered by final manager Release; it does not justify postponing map ownership to a member/deleting
destructor or applying a destructor-only annotation by controller-manager analogy. If a replacement
keeps these references until member destruction after Shutdown, its timing differs. If its generic
helper is executing the specified Shutdown event, a zero result alone is not a native ownership error.
Determining which case produced those three warnings requires receiver identities, before/after
reference counts and the Shutdown-versus-destructor phase at that helper; none is inferred here.
No warning suppression, audit-macro change, coverage/default change or ports promotion follows.

#### 5.2.2 Independent discriminating ownership vectors (AT24)

These are required behavioural probes, **specified here, not executed in this RE lane**. Use counted
airport/plane interfaces, an initialized manager, valid unique map keys and an independently controlled
segment/table owner. Log IID/out receiver, AddRef/Release, plane Shutdown and manager phase. Keep
an observer reference when necessary to inspect events after manager cleanup; remove it explicitly.

| Vector | Required observation; distinguishes |
|---|---|
| Load one pair and the same plane once in the base list; destroy the segment/table before manager cleanup | After table disposal, two plane references remain owned by the manager and one airport reference by the map. Table disposal cannot substitute for either map release. |
| Same loaded state, deliver PreCityShutdown, inspect before calling manager Shutdown | Plane Shutdown then one list Release; map still has the pair and owns the remaining plane reference. Manager Shutdown later gives airport Release then final plane Release. Distinguishes base-list clearing from map clearing. |
| Same state, omit PreCityShutdown, call manager Shutdown | Map airport Release, map plane Release, then base-list plane Shutdown and plane Release. Distinguishes map-first Shutdown from destructor-held references and from shutting down each mapped plane in the map loop. |
| Retain one extra plane reference across either sequence | Manager cleanup leaves that reference alive; dropping it later, outside the manager, destroys the plane. Distinguishes legitimate final ownership from a fixed three-warning expectation. |
| Two map pairs inserted/read in reverse pointer-key order | Shutdown releases each pair airport then plane in ascending key order; it does not release every airport before every plane. Keep independent observer owners to avoid reentrant deletion affecting the probe. |
| Accepted RemoveOccupant for one present key, then repeat, then manager Shutdown | First hit: plane slot 13(false), erase, plane Release, airport Release. Second hit: no map releases. Shutdown must not release that pair again. Distinguishes removal from bulk Shutdown ordering. |
| Explicit manager Shutdown twice, then final Release; separately final Release without explicit Shutdown | First case: one payload pass, no payload releases in deleting destruction. Second case: one payload pass in virtual Shutdown before deleting destruction. Distinguishes final Release from directly invoking a destructor. |

### 5.3 Watercraft (46), version 6

Write `0x0056FA70`, Read `0x005749E0`. Read: a version above 6 fails (`0x00574A26`, AM9); a version below 6 means
the whole record is a base record of that version.

1. `u16` 6;
2. base record;
3. `s32` count, count × (`ref` lot `0xE818FCED`, `ref` occupant `0x26BB767E`);
4. `s32` count, count × `ref` occupant;
5. `s32` count, count × ferry entry: `ref` lot, `ref` occupant, `s32` n, n × route element.

Each of the three lists is in its keys' creation order (simulator.md §5.8): the lot of a pair or ferry entry, the
occupant of list 4. The original's own order is that of three hash maps keyed by those pointers, by address.

At each animation tick (`DoAnimationTick`, `0x005736A0`) the manager creates the ferries its ferry entries lack
(`CreateFerry`) and then the boats its marina occupants (list 4) lack (`CreateMarinaBoat`), each walk in its keys'
creation order too; the original walks the two hash maps by address, so the order the boats are created in, and so
their IDs, followed the heap.

A route element: `ref` lot, `ref` lot, `s32` kind (0, 1, otherwise −1), 4 × `u16`, `bool`, `s16`, `s16`,
`bytes`, `bytes`.

Corpus: 12 records: 7 of version 6, 4 of version 5, 1 of version 4. Worked example in §14.

## 6. Automaton records *[compat]*

The managers' lists, the controllers and the controller manager's sets reference automata by `ref`. These are
the automaton classes found in saves, with their records.

### 6.1 The automaton base record, version 8 (Write `0x0050AC00`, Read `0x0050DFD0`, accepts ≤ 8)

| # | Field | Notes |
|---|---|---|
| 0 | `u16` 8 | the automaton base's own version (Write `0x0050AC0D`; Read `0x0050DFED` accepts ≤ 8) |
| 1 | the occupant base record | lot-records §2.3 (its own `u16` must be 4); occupant type `0x496E5783` |
| 2 | 3 × `vec3` | position, destination, orientation (automaton slots 7, 11, 15) |
| 3 | `f32`, `f32` | radius (slot 17), default speed |
| 4 | `u32` automata flags, `u32` | the flags (slots 30/31); then the dispatch info ID (slots 27/28). Flag bits named by the Mac build: 1 marked for removal, 2 self-controlling, 4 reached destination, `0x40` special route, `0x8000000` disabled |
| 5 | `ref` controller | `0x6A1857F8`; the **attractor** now acting on the automaton, chosen as the strongest by the controller manager's `GetCompositeAttractionEffect` (§7.1 slot 22). Written as null when the controller has flag 8 |
| 6 | `f32`, `f32` | the automaton's distance to that attractor, and its attraction strength (signed) |
| 7 | `vec3` | the composite attraction: the sum over the attractors in range of strength × the unit horizontal vector toward each (y = 0). Random walks add it to their step |
| 8 | `ref` automaton | `0xA9B40F05`; the leader (slots 48/49), e.g. the automaton ahead in a queue (§3.3) |
| 9 | `u32` n, then n × (`u32` state type, state record) | the state stack (§6.2) |
| 10 | `u32` m, then m × `key3` | model keys |
| 11 | `key3` | the first model key added, e.g. type `0x5AD0E817` |
| 12 | `u32` | the instance ID of the model now shown; −1 after a state change that asks for a new one |
| 13 | `u32` k, then k × `ref` callback target | only callbacks whose target is serializable (`QueryInterface 0xE4FDA3D4`) are written; read checks `0x452294AA`. Seen: the controller manager and the city situation manager (`0x0BB14381`). |
| 14 | 8 × `f32` | two tweens of four `f32` each: speed (slots 34–38), then opacity (slots 32–33). A tween is (start, target, duration, elapsed); its value is start + clamp(elapsed / duration, 0, 1) × (target − start), or target when the duration is ≤ 0 |
| 15 | `u8` | priority (slots 63/64; `CreateAutomaton`'s priority, §7.2) |
| 16 | only in version 8: `u32` | the automata group ID (slot 67 looks the group up from it) |

Field names come from the Mac build's accessors (`GetPosition`, `GetRadius`, `GetDispatchInfoID`, `GetLeader`,
`GetPriority`, `GetAutomataGroup`, `TweenStruct`), whose Write is in the same order. The Windows getters read the
fields that the Windows Write (`0x0050AC00`) writes; Write's `this` is 8 bytes below the primary interface:

| Slot (pedestrian vtable `0x00A9A718`) | Reads | Write offset |
|---|---|---|
| 7 | `+0x40` | `+0x38` |
| 48 | `+0xB0` | `+0xA8` |
| 63 | `+0x90` | `+0x88` |
| 67 | `+0x1C4` | `+0x1BC` |

The meanings are for naming only; every field must round-trip unchanged *[compat]*.

Corpus: base version 7 in 79,509 automata, version 8 in the rest.

### 6.2 States

The state type selects the record. The state factory (automaton slot 94, `0x0050B7F0`) knows types 8 to `0x34`;
any other type gets the default state, whose record is the base record alone.

**State base, version 3** (Write `0x00509D00`, Read `0x00505E50`, accepts ≤ 3): `u16` 3, `ref` controller (null
when it has flag 8; read checks `0x6A1857F8`), `u32`, `f32`, `bool`, `u32`, `u32`, and in version 3 a final `u32`.

**Object offsets (Q5, re-answers29).** The record maps onto the state object like this; the stream order is the
compatibility surface.

| Object offset | Stream | Notes |
|---|---|---|
| `+0x00` | — | vtable (per state class) |
| `+0x04` | — | the state type number; set by the factory, **not** in this record (the outer writer stores it) |
| `+0x08` | `ref` | the controller |
| `+0x0C` | `u32` | |
| `+0x10` | `f32` | |
| `+0x14` | `bool` (one byte) | |
| `+0x18` | `u32` | **the SFX id** *[named by audio.md §6.1]* |
| `+0x1C` | `u32` | |
| `+0x20` | `u32` | written only at version ≥ 3 |
| `+0x24` | **not in the record** | Read zeroes it; the default-state constructor also zeroes it |

**Version behaviour.** Write **always emits version 3 and all seven fields**, whatever version was read; Read
accepts ≤ 3 and reads `+0x20` only at version ≥ 3. So a re-save of an older state record upgrades it (see
QUESTIONS.md AM1). The default state's constructor (`0x0050B7F0`'s default branch) zeroes `+0x08` … `+0x24`.
**The state base fields, named** (re-integrate1, from `cSC4Automaton::PushState` Mac `0x000bf7d0` /
Windows `0x0050EF50`, which writes every one of them, and `tAutomataState::Write`/`Read`
Mac `0x000c3aa0`/`0x000c43f8`, Windows `0x00509D00`/`0x00505E50`). Object offsets are **Windows**:

| Offset | Type | Name | Evidence |
|---|---|---|---|
| `+0x00` | vtable | — | every state ctor, e.g. Mac `0x0063803a` |
| `+0x04` | `u32` | **state type** | not in the save; written by `PushState` from `param_1`, read as the dispatch key (Mac `0x000bd464`) |
| `+0x08` | `ref`, IID `0x6A1857F8` | **controller** | Read/Write; every tick first goes to `controller->vtable+0x70` (Mac `0x000bd464`) |
| **`+0x0C`** | `u32` | **stack index** (0-based, bottom = 0) | `PushState` sets it to `stack.size()-1` after the push (Mac `0x000bf7d0`). `PrevState(s)` = `stack[s->+0x0C - 1]`, the state below (Mac `0x000bbee4`, Windows `0x00509900`). **Not a parameter** — see the quirk below. |
| **`+0x10`** | `f32` | **countdown, milliseconds** | `DoStateTick` subtracts `dt` and `PopState`s on expiry (Mac `0x000bd464`). ≤ 0 means "never expires". Supplied by `PushState` `param_3`; behaviour templates derive it as `template+0x28` seconds × 1000, or −1.0 if `+0x28 ≤ 0` (`tBehaviorTemplate::GetTimeoutMilliseconds` `0x000b4162`) |
| **`+0x14`** | `bool` | **`final`** | Named by the Lua behaviour key **`final`** (`ImportBehaviorTable`, Windows `0x00522860` → `template+0x2C` → `PushState` `param_4`). `SetState` refuses to run while the top state has it (Mac `0x000bd3ca`, Windows `0x00509860`). `PopState` on a `final` state that is not already type 1 does **not** reveal the state below — it does `PushState(1, 0, -1.0, 0, 0, 0, 0)` (DISAPPEAR) instead (Mac `0x000bd2de`, Windows `0x0050B660`) |
| **`+0x18`** | `u32` | **SFX id** | `HandleEnterState` calls `StartSFX` with it when non-zero (vtable `+0xF4`), `HandleExitState` calls `StopSFX` (`+0xF8`) (Mac `0x000c0c8e` / `0x000c0418`). From `PushState` `param_6`, which `PushBehavior` fills from a random entry of the template's **`sfx`** list (`template+0x1C`, `0x000be558`) |
| **`+0x1C`** | `u32` | **animation id** | `HandleEnterState` applies it via vtable `+0x4C` (Windows `0x00510240`); `DoRandomWalkTick` re-applies it when leaving an idle (Mac `0x000c4b58`). From `PushState` `param_5`, filled by `PushBehavior` from a random name in the template's **`anims`** list resolved through vtable `+0x14C` |
| **`+0x20`** | `u32` | **automata-flags snapshot** | On enter: `state+0x20 = automaton+0x84` (Mac `0x000c0c8e`). On exit: `automaton+0x84 = (automaton+0x84 & 0x600C001) \| state+0x20` (Mac `0x000c0418`). `+0x84` is the flags word (`SetAutomataFlag` `0x004FAD20`, `CheckAutomataFlag` `0x004FAD60`) |
| **`+0x24`** | `tBehaviorTemplate const*` | **behaviour template** | `PushState` `param_7`. Not saved — `Read` zeroes it (Mac `0x000c43f8`). On enter, if non-null, its six bools at `+0x44..+0x49` set automata flags `0x2000`, `0x20000`, `0x40000`, `0x4000`, `0x400000`, `0x800000`, then `SetSpeedFromBehaviorTemplate` is called with it (vtable `+0x150` = Windows `0x00505780`). `DoRandomWalkTick` falls back to `_sDefaultBehaviorTemplate` when it is null |

`PushState`'s full signature (Mac `0x000bf7d0`, authoritative):

```
PushState(u32 type, cISC4AutomataController* controller, float timeout_ms,
          bool final, ulong animId, ulong sfxId, tBehaviorTemplate const* template)
```

`SetState` = pop everything then `PushState`, aborting with 0 if any top state is `final`.
`PushState(0, 0, …)` delegates to `SetState` (Mac `0x000bf7d0`).

**The behaviour template** (`tBehaviorTemplate`, ctor Windows `0x0050BD40` / Mac `0x00639a70`) — the Lua
keys are read by `cSC4AutomataScriptSystem::ImportBehaviorTable` (Windows `0x00522860` / Mac `0x0015b978`):

| Lua key | Offset | Default | Used as |
|---|---|---|---|
| `state` | `+0x00` | — | the state type |
| `radius` | `+0x04` | `FLT_MAX` | |
| `percentage` | `+0x08` | 1.0 | chance a candidate state is adopted |
| `priority` | `+0x0C` | 1 | |
| `anims` | `+0x10` | | `vector<string>` → state `+0x1C` |
| `sfx` | `+0x1C` | | `vector<u32>` → state `+0x18` |
| `timeout` | `+0x28` | | seconds → state `+0x10` × 1000 |
| **`final`** | `+0x2C` | | → state `+0x14` |
| `kph_multiply` | `+0x30`/`+0x34` | 1, 1 | speed multiplier range |
| `kph` | `+0x38`/`+0x3C` | −1, −1 | integer kph override; × 1/3600 → units per ms (`0x000bb23c`) |
| `acceleration` | `+0x40` | 0.5 | passed to `SetSpeed` |
| `ignore_paths` … `avoid_roads` | `+0x44..+0x49` | | → the six automata flags of `+0x24` above |
| `path_offset` | `+0x4C` | 0 | |
| `random_walk.delay` | `+0x50`/`+0x54` | 10, 15 | seconds between turns (`DoRandomWalkTick` refills the turn countdown `+0x34` with `U(+0x50, +0x54)` seconds when it expires) |
| `random_walk.turn` | `+0x58`/`+0x5C` | 0.1, 0.2 | turn amplitude / bias, as a fraction of a full turn. **The offsets are settled (AT10 answered)**: `ImportBehaviorTable` writes the array/scalar into **`+0x58`** (amplitude) and **`+0x5C`** (bias) — the same offsets `DoRandomWalkTick` reads (`FLD [template+0x58]` `0x0050D139`, `FLD [template+0x5C]` `0x0050D187`). An earlier reading that put the writes at `+0x54`/`+0x58` mis-simulated the stack frame; the ctor defaults (`+0x58 = 0.1`, `+0x5C = 0.2`, `tBehaviorTemplate::tBehaviorTemplate` `0x0050BD40`) and the reader agree with the writer. A scalar `turn` sets the amplitude only (bias 0); an array sets both |
| `random_walk.strength` | `+0x60`/`+0x64` | 1, 1 | |
| `random_walk.idle_delay` | `+0x68`/`+0x6C` | 0, 0 | **walk duration** in seconds (`DoRandomWalkTick` refills the walk/idle countdown `+0x38` with `U(+0x68, +0x6C)` seconds when a rest ends); when the range is (0, 0) the walk never ends and the automaton never rests (the guard is `|+0x68 − 1e-5| ≥ 1e-4`, `0x0050D093`) |
| `random_walk.idle_time` | `+0x70`/`+0x74` | 5, 10 | **rest duration** in seconds: when the walk countdown reaches 0 the automaton sets `+0x38 = −U(+0x70, +0x74)` (negative = resting), calls `TweenSpeed(0, +0x78)` and plays the idle anim; the rest counts back up to 0 |
| `random_walk.deceleration` | `+0x78` | 0.5 | rest speed passed to `SetSpeed` (slot 36 `TweenSpeed(0, +0x78)`, `0x0050D0E3`) |

`DoRandomWalkTick` (`0x0050CF90`) therefore runs on two independent countdowns, both in **seconds** (dt is
converted with `× 0.001` first, `0x0050CFB4`): the **turn countdown** `+0x34`, refilled from
`random_walk.delay` and on expiry choosing a new heading — `heading = atan2(z, x) + U(−a − 1e-5, a + 1e-5) × 2π ± b × 2π`,
where the bias sign follows the sign of the drawn turn (`0x0050D181`–`0x0050D1A1`) — and the **walk/idle
countdown** `+0x38`: positive = walking (counting down from `U(idle_delay)`; on reaching 0 it flips to
`−U(idle_time)` and rests, tweening speed to 0), negative or 0 = resting (counting up; on reaching ≥ 0 it
refills with `U(idle_delay)`, calls slot `+0x150` = `SetSpeedFromBehaviorTemplate`, clears the idle-anim bool
`+0x3C` and re-applies the saved anim id `+0x1C` through slot `+0x4C`).

**Per-type record layouts** (the field order a round trip must use):

| Type | Object offsets | Stream order | Notes |
|---|---|---|---|
| `0x0B` RandomWalk | `+0x28..+0x3C` | `vec3`, `f32`, `f32` | heading, turn countdown, walk/idle countdown; `+0x3C` is a `bool` idle-anim-playing, unsaved |
| `0x0E` Plummet | `+0x28` descent velocity, `+0x2C` roll rate, `+0x30` roll centre, `+0x34` tumble countdown ms, `+0x38` post-splash countdown ms, `+0x3C` unsaved `ref` | **`+0x28, +0x34, +0x30, +0x2C, +0x38`** (Mac `0x000c3e36` / `0x000c47f4`) | **not offset order.** `+0x3C` is a `ref` zeroed on Read, not a sixth `f32` (§12 quirk 12) |
| `0x0F` PlopJump | `+0x28` floor Y, `+0x2C` vertical velocity | `f32`, `f32` | enter seeds `+0x2C = U(30, 50) × 1/3600` — the draw is in **km/h**, so the velocity is 8.3–13.9 m/s = 0.0083–0.0139 m/ms (AT9 answered) |
| `0x11` Bumped | `+0x28/+0x34/+0x40/+0x4C` 4 × `vec3` | `vec3` × 4 | **no reader anywhere** — written only by the record's own Read and zeroed by its ctor; `cSC4Vehicle::HandleCollision` pushes COLLIDE, not Bumped (§12 quirk 13, AT7 answered) |
| `0x10` PlayerDrive | `+0x28` one `f32` | `f32` | record is `0x274` bytes; only `+0x28` is saved. `+0x28` is the **drive (throttle/brake) command accumulator**: `cSC4Vehicle::TranslatePlayerDriveCommands` (Mac `0x000a336c`) advances it by `±dt_s × params+0x10/0x14` and zeroes it when both drive channels are inactive, and `PlayerDriveUpdateSpeed` (Mac `0x000a3e1a`) integrates it into the speed at `+0x2C` (AT6 answered) |
| `0x13` AttachAnim | `+0x28` `ref` model (`0xA818EDD8`), `+0x30` `u16` anim selector, `+0x2C` `bool` | `ref`, `u16`, `bool` (Mac `0x000c3bba` / `0x000c453a`) | **the `bool` sits before the `u16` in memory** (§12 quirk 14) |
| `0x14` Collide | none | none | pushed `final=1` with a 5000 ms timeout |
| `0x15` PullOver | `+0x28` one `f32` | `f32` | **the two reports read this field differently** — see the conflict note in §6.5 |
| `0x34` Chase | `+0x28` `ref` automaton (`0xA9B40F05`) | `ref` | **a pure record**: no `0x34` case in any `DoStateTick` (§12 quirk 15) |
| 8 RiotLeader | `+0x28` `s32` last cell X, `+0x2C` `s32` last cell Z, `+0x30` `u8` heading 0–3 | `s32`, `s32`, `u8` | |

**Corrects (re-specs20, audio.md §6.1):** `+0x18` is the **SFX id**, and it is the argument of both SFX slots.
`cSC4Automaton::HandleEnterState` (`0x00511450`) calls slot 61 = `+0xF4` `StartSFX` with `*(u32*)(state+0x18)`
when that field is non-zero, and `HandleExitState` (`0x0050E3F0`) calls slot 62 = `+0xF8` `StopSFX` with the
same field under the same guard. The §11 slot names `61–62 Start/StopSFX` **MATCH**; this adds the field's
meaning and the call sites. Full contract in [audio.md](audio.md) §6.

| Type | Name | After the base | Write / Read |
|---|---|---|---|
| 8 | RiotLeader | `s32`, `s32`, `u8` | `0x00509DE0` / `0x00505F50` |
| `0x0B` | RandomWalk | `vec3`, `f32`, `f32` | `0x00509D90` / `0x00505F00` |
| `0x0E` | Plummet | 5 × `f32` | `0x00509E70` / `0x00505FE0` |
| `0x0F` | PlopJump | `f32`, `f32` | `0x00509E30` / `0x00505FA0` |
| `0x10` | PlayerDrive | `f32` | `0x00509EE0` / `0x00506090` |
| `0x11` | Bumped | 4 × `vec3` | `0x00509F20` / `0x00506100` |
| `0x13` | AttachAnim | `ref` model (`0xA818EDD8`), `u16`, `bool` | `0x00509FB0` / `0x0050A020` |
| `0x14` | Collide | none | `0x00509F80` / `0x00506160` |
| `0x15` | PullOver | `f32` | `0x00509EE0` / `0x00506050` |
| `0x34` | Chase | `ref` automaton | `0x0050A0A0` / `0x0050A0E0` |
| any other | default | none | base only |

**Names (Q3, answered).** Two sources tie the numbers to the names:
- The game's own Lua constant table `automata_states` (`sit_constants.lua` in `SimCity_1.dat`) gives the numbers.
  **Read directly from the shipped script (2026-10-16, re-answers29) and it matches the table above exactly.**
  The script's own grouping comment is `-- general` for 0–12, `-- vehicles` for 21 and 30, `-- aircraft` for
  50–58 and `-- watercraft` for 70–78. Two extra facts from it: there is **no** entry for 5, 13 or 14 (so 13
  and 14 really are unnamed in the game's own table), and `CHASE` (52) carries the comment *"can also apply to
  vehicles"* — so the name is not aircraft-only.
- The Mac build's `CreateAutomataState` gives the classes: `tAutomataStateRandomWalk`, `…PlopJump`, `…Plummet`,
  `…Bumped`, `…AttachAnim`, `…Collide`, `…PullOver`, `…Chase`, `…RiotLeader` and `…PlayerDrive`.
- The two agree, and the Windows factory (`0x0050B7F0`, jump table `0x0050B98C` / `0x0050B9B8`) builds objects of
  the same sizes for the same numbers: RiotLeader `0x34`, RandomWalk `0x40`, Plummet `0x40`, PlopJump `0x30`,
  PlayerDrive `0x274`, Bumped `0x58`, AttachAnim `0x34`, Collide `0x2C`, PullOver `0x44`, Chase `0x2C`; default
  `0x28`.

| Type | Lua name | Record |
|---|---|---|
| 0 | `DEFAULT` | base only |
| 1 | `DISAPPEAR` | base only |
| 2 | `BEE_LINE` | base only |
| 3 | `CROWD` | base only |
| 4 | `QUEUE` (the controller queue, §3.3) | base only |
| 6 | `STRIKE` | base only |
| 7 | `IDLE` | base only |
| 8 | `RIOT_LEADER` | table above |
| 9 | `RIOT_FOLLOWER` | base only |
| 10 | `SCRIPTED` | base only |
| 11 (`0x0B`) | `RANDOM_WALK` | table above |
| 12 | `IGNITE_OCCUPANT` | base only |
| 13 (`0x0D`) | not in the Lua table; see below | base only |
| 14 (`0x0E`) | not in the Lua table; the Mac class is `tAutomataStatePlummet` | table above |
| 15 (`0x0F`) | `PLOP_JUMP` | table above |
| 16 (`0x10`) | `PLAYER_DRIVE` | table above |
| 17 (`0x11`) | `BUMPED` | table above |
| 18 | `MAGNET` | base only |
| 19 (`0x13`) | `ATTACH_ANIM` | table above |
| 20 (`0x14`) | `COLLIDE` | table above |
| 21 (`0x15`) | `PULL_OVER` | table above |
| 30 | `STOP_AT_STATION` | base only |
| 50–58 | aircraft: `RETURN_HOME`, `GO_TO_DESTINATION`, `CHASE` (52 = `0x34`, table above), `LOITER`, `LOOK_FOR_CRIME`, `LOOK_FOR_TRAFFIC`, `RESCUE`, `PUT_OUT_FIRE`, `CROP_DUST` | base only, except 52 |
| 70–78 | watercraft: `CRASH_RUNAGROUND`, `CRASH_SINK`, `IMMEDIATE_REMOVE`, `SLOW_REMOVE`, `FERRY_DOCK`, `FERRY_QUEUE`, `MARINA_PARKED`, `BEING_TOWED`, `UNDOCKING` | base only |

**Type 13** is the traffic automaton's **blocked wait**. The traffic automaton's collision update
(`cSC4TrafficAutomaton::UpdateCollision`, push at `0x0054E322`) pushes it when all of these hold:
- the traffic network map reports the automaton blocked by another one (result 1);
- the automaton has flag `0x20`.

The push is `PushState(13, 0, t, 0, 0, 0, 0)` (slot 52), where t = an automata tuning value (`float` at `0x00B21C74`)
× 1000.0 (`0x00A8FAF0`), in milliseconds. A blocked automaton also gets flag `0x10` (`0x0054E2F1`), with or
without the push. While the top state is 13 the
collision update returns at once (Mac build).

Corpus state types: `0x0B` 80,519; 0 811; 2 381; 7 107; `0x0D` 101; 3 32; `0x14` 10. In words: random walk,
default, bee line, idle, blocked wait, crowd and collide. State versions: 2 in 80,080, 3 in 1,881.

**How a state is ticked, and the returned-0 convention** (re-integrate1). `t = stack.end[-1]` (the state
stack vector at the automaton `+0xB4`/`+0xB8`), `t->+0x04` is the state type. `cSC4Automaton::DoStateTick`
(Windows `0x00508AA0` / Mac `0x000bd464`) does, in order:

1. If `t->+0x10 > 0`, subtract `dt`; if it goes negative call **`PopState`** (vtable `+0xD4`) and take the new
   top. So `+0x10` is a countdown in **milliseconds**; `dt` is in ms (`DoRandomWalkTick` divides by 1000 to
   get seconds).
2. If `t->+0x08` (controller) is non-null, give it first refusal via controller vtable `+0x70`
   `(automaton, type, dt)`. A non-zero reply suppresses the automaton's own tick.
3. Otherwise switch on the type. **A type with no case returns 0.**
4. The return value means **"I moved myself"**: `cSC4Automaton::DoAnimationTick` (Mac `0x000c50b0`) only runs
   the generic seek-to-destination when `DoStateTick` returns **0**. That is what type 0 `DEFAULT` is.

So **returned 0 ⇒ `DoAnimationTick` seeks the destination**; returned 1 ⇒ the state handled its own movement.
Derived dispatchers layered on the base: `cSC4TrafficAutomaton::DoStateTick` `0x00550090` (13, 30);
`cSC4Vehicle::DoStateTick` `0x0055CA90` (14, 21); `cSC4Pedestrian::DoStateTick` `0x00546BC0`
(2, 3, 4, 6, 8, 9, 12); `cSC4EmergencyVehicle::DoStateTick` `0x0052F490` (100–103);
`cSC4Watercraft::DoStateTick` `0x00573340` (7, 70–78).

### 6.3 Traffic automaton, version 2 (Write `0x00551DE0`, Read `0x00554D20`, requires 2)

`u16` 2, the automaton base record, `ref` automaton (`0xA9B40F05`), `u32`, `u8`, `ref` network occupant
(`0xA821EF94`; seen class `0xC9C05C6E`), `u32`, `u32`, `u32` n, n × route step (`u8`, `u8`, `u8`).

The fields after the base record are, in order: retained leader, u32 transport
selector, saved traffic option byte, retained network occupant, packed path key,
segment-index word, then the route-cell count/triples. Triples contain unsigned
cell X, cell Z and a preserved annotation byte; they are not vec3 path points.
The Read/Write receiver is the serialization interface at primary-object `+8`:
record-relative `+0xA8/+0x1DC/+0x1E0/+0x1E4/+0x1EC/+0x1F4/+0x224` therefore
alias live `+0xB0/+0x1E4/+0x1E8/+0x1EC/+0x1F4/+0x1FC/+0x22C` respectively.
This is an address translation, with no stream-format adjustment. Evidence:
QI `0x00505DCC–0x00505DEC`, constructor `0x005544FB–0x0055450F`, Read
`0x00554D9A–0x00554E11`, Write `0x00551E17–0x00551E64`.

Read does not restore the path interface or point-collection pointer, query
network geometry, or call placement. See vehicles.md §12.9 for lane matching,
advancement/reattachment, the worked record case, and the precise remaining
runtime observations VH7-R1/R2/R3 for later binding and opaque-byte meanings.
Those observations remain a boundary on full post-load path reconstruction.

### 6.4 Concrete classes

| Class | ID | Record (all start with `u16` version) |
|---|---|---|
| pedestrian | `0x896E75AF` | `u16` 5, traffic record, `bool`, `vec3`, `f32`, `u32` (Write `0x00544650`, Read `0x005446C0`) |
| vehicle | `0x896E714A` | `u16` 2, traffic record, `u32` n + n × (`u32`, `s32`, `s32`) path entries, `u32` 0, `vec3`, `vec3` (−1, −2, −3), `f32` (Write `0x0055F640`, Read `0x0055BCF0`) |
| carjacked vehicle | `0xCBB32B0A` | `u16` 1, complete vehicle record; no tail (§6.4.1; Write `0x0053CE00`, Read `0x00524490`) |
| mayor's limo | `0x4C0DD224` | `u16` 1, complete vehicle record; no tail (§6.4.1; same Write/Read as carjacked vehicle) |
| tank | `0xEBE0DCAD` | `u16` 2, complete vehicle record, `s8` first auxiliary model index, `s8` second auxiliary model index (§6.4.3; Write `0x0054AA30`, Read `0x0054A8E0`) |
| emergency vehicle | (`0x896E714A`, see §12) | `u16` 2, vehicle record, `vec3`, `ref` occupant (`0x26BB767E`), `u8`, `u16`, `u32` (Write `0x0052FA60`, Read `0x0052FAE0`) |
| ambulance | `0x6C1ACDE1` | `u16` 1, complete emergency record; no tail (§6.4.2; Write `0x00504BF0`, Read `0x00504C10`) |
| fire truck | `0xAA222B83` | `u16` 2, complete emergency record, `ref` crew automaton (`0xA9B40F05`) (§6.4.2; Write `0x00532EE0`, Read `0x00532F20`) |
| patrol car | `0x4A222B98` | `u16` 2, emergency record, `ref` automaton (`0xA9B40F05`) (Write `0x00543DB0`, Read `0x00543DF0`) |
| trailer vehicle | `0x4A5B69ED` | `u16` 2, vehicle record, `u32` n + n × `ref` vehicle (read checks IID **`0x896E714A`**, AM10), `u32` m + m × `u32`, `vec3`, `ref` network occupant (`0xA821EF94`), `u32` (Write `0x005562A0`, Read `0x00556C30`) |
| mass-transit train | `0x4C22AE53` | `u16` 1, complete Train record, including its TrainCar wrapper and train tail; no additional tail (§6.4.4; Write `0x0053CB30`, Read `0x0053CB50`) |

Except Tank (§6.4.3), each Read in this table requires its leading `u16` to equal the version shown,
exactly (AM2, §4.1). Nested records retain their own version words.

**The trailer's vehicle refs (AM10).** The trailer reads them with IID `0x896E714A` (`0x00556C8B`). That is the
vehicle's class ID, which the vehicle's `QueryInterface` (`0x0055B660`, test at `0x0055B665`) also answers as an
interface ID. So a ref to a vehicle, emergency vehicle, patrol car or trailer resolves. A ref to any other automaton
(a pedestrian, say) fails the check. Null refs are skipped, and the list keeps only the non-null ones.

**The emergency vehicle layout (AM8).** The layout above is what the Write produces:
- `0x0052FA6B` writes `u16` 2;
- `0x0052FA75` calls the vehicle Write;
- then come the `vec3`, the ref, the `u8`, the `u16` and the `u32`.

So the record is the emergency's own `u16`, then the full vehicle record. §12 quirk 3 gives the corrected account
of what happens when it is reloaded.

The vehicle's Read discards the path entries and the constant vector; a replacement must still write them
(writing an empty path list is allowed *[choice]*). Corpus: 81,787 pedestrians, 1,085 vehicles, 63 trailers and
3 patrol cars, all parsing exactly to their ends. No aircraft or watercraft automata occur in the corpus (Q4).

**Aircraft and watercraft automata (Q4, answered).** No corpus save has them, so these records come from the
executable alone. Class IDs are from each class's `GetGZCLSID`; names are from the Mac build.

| Class | ID (`GetGZCLSID`) | Record | Write / Read |
|---|---|---|---|
| aircraft base (`cSC4Aircraft`) | `0xC96E75DC` (`0x004FAE90`) | `u16` 1, the automaton base record (§6.1). Read requires exactly 1 | `0x004FB060` / `0x004FB080` |
| airplane (`cSC4Airplane`) | `0xCBEB4F62` (`0x004FD280`) | `u16` 1, the aircraft base record, `vec3` home position. Read accepts ≤ 1 | `0x004FD380` / `0x004FD3C0` |
| helicopter (`cSC4Helicopter`) | `0x8BA98EDA` (`0x0053C830`) | `u16` 1, the aircraft base record, `vec3` home position. Read accepts ≤ 1 | `0x005343C0` / `0x00534400` |
| watercraft (`cSC4Watercraft`) | `0x096E75F5` (`0x0056DC60`) | `u16` 2, the automaton base record. Read requires exactly 2 | `0x00566EE0` / `0x00566F00` |

So the aircraft base and airplane/helicopter records nest: airplane = `u16` 1, then `u16` 1 and the automaton base
record, then the `vec3`.
- The airplane and the helicopter write the `vec3` only when the aircraft part was written without error
  (`0x004FD399`, `0x005343D9`). On a stream that has not failed, it is always present.
- Read stores the `vec3` (airplane `+0x344`, helicopter `+0x1E8`) and sets a "position loaded" byte just below it
  (`0x004FD3FF`, `0x0053443F`).
- What the `vec3` is (Mac build, `Init`): a new aircraft copies its current position there at its first `Init`. A
  loaded one moves to it instead, so it is the position the aircraft was placed at.
- Aircraft and watercraft carry no traffic record (§6.3). Their own state types (50–58 and 70–78, §6.2) all use
  the default record, except `CHASE` (52).

#### 6.4.1 Carjacked vehicle and mayor's limo records (AT29; branch AT24)

Both concrete classes use the same folded writer and reader, selected by their serializable tables
`0x00A9786C` and `0x00A9993C`. Each record is exactly **`u16 1`; complete Vehicle record** from the table
above, with no class-specific trailing fields. Their class IDs remain distinct in the object store's
reference/class framing; neither class ID is an extra scalar inside this record.

For these sections, **V** denotes that complete Vehicle record, including its `u16 2`, the complete
Traffic record (§6.3, `u16 2`), the Automaton record (§6.1, current `u16 8`) and Occupant record
([lot-records.md](lot-records.md) §2.3, `u16 4`), then the Vehicle path count/entries, zero dword, two
vectors and final float. The inherited readers require Vehicle 2, Traffic 2 and Occupant 4; Automaton
accepts unsigned versions at most 8 under §6.1's version-dependent field rules. No wrapper removes a
nested version or changes any inherited reference IID. In particular the inherited controller,
leader, callback and network references request `0x6A1857F8`, `0xA9B40F05`, `0x452294AA` and
`0xA821EF94`, respectively (§§6.1–6.3). The traffic leader is another `0xA9B40F05` reference.

The outer reader accepts **exactly 1**. A different word returns false after two bytes, without entering
Vehicle. For version 1, Read returns Vehicle Read's Boolean result; Write emits 1 and returns Vehicle
Write's result. Neither adds a reference operation, tail read or final stream-error test. Inherited
failure does not rewind bytes already consumed or undo inherited object changes. Evidence: outer
write/call `0x0053CE0A–0x0053CE14`; read/check/call `0x005244A1–0x005244B6`.

**Worked cases.** Take a valid current V of length L whose inherited reference resolutions succeed.
For each of the two class IDs separately, Write must produce length **L+2**, with version-word prefix
`1, 2, 2, 8, 4`. Read consumes L+2 bytes, returns the inherited success and leaves a following sentinel
untouched. Changing just the outer word to 0 or 2 consumes two bytes and returns false. Keeping outer
1 but changing V's version to 3 consumes four bytes and returns false; no implicit retry as plain
Vehicle is performed. The distinct class identities must survive a re-save despite the shared methods.

#### 6.4.2 Ambulance and fire-truck records, including Emergency Read (AT30/AT33; branch AT25/AT28)

Let **E** be the complete Emergency record. Its exact order, following the scalar/ref encodings of §1,
is:

| Order | Field |
|---|---|
| 0 | `u16 2`, Emergency's own version |
| 1 | complete V, including Vehicle's own `u16 2` |
| 2 | `vec3` dispatch target |
| 3 | `ref` home/station occupant; reader requests IID `0x26BB767E` |
| 4 | `u8` re-path attempts |
| 5 | `u16` call number |
| 6 | `u32` dispatch state |

Emergency accepts **exactly 2**, then invokes Vehicle Read. An outer mismatch returns false without
entering Vehicle; Vehicle failure returns false without reading Emergency's tail. On Vehicle success,
the target, occupant reference, byte, word and dword are read in that order. The home/station output
member is passed directly to the shared reference reader; Emergency adds no explicit old-reference
release here. It restores the dispatch-state scalar directly, without invoking SetDispatchState or
its siren/crew side effects. Its final Boolean remains Vehicle's result: neither individual tail
getter/helper results nor a fresh stream-error check replace it. Its writer similarly writes the tail
only after Vehicle Write succeeds and returns that inherited result. These statements describe normal
returns; they do not suppress a framework exception.

Evidence: Emergency version, nested Vehicle and tail writes `0x0052FA6B–0x0052FAC9`; read version,
nested call and tail `0x0052FAF2–0x0052FB6A`. The byte's re-path use is at `0x0052FE72`; GetCallNumber
reads the word at `0x00505030`; SetDispatchState stores the dword at `0x0052FF8B`.

**Ambulance (`0x6C1ACDE1`).** The whole record is **`u16 1`; E**, with no Ambulance tail or additional
reference IID. Its outer reader requires exactly 1 and returns false after two bytes on mismatch;
otherwise Read/Write return the result of Emergency Read/Write. Evidence: `0x00504BFA–0x00504C04`,
`0x00504C21–0x00504C36`.

**Fire truck (`0xAA222B83`).** The whole record is **`u16 2`; E; `ref` crew automaton**. The outer reader
requires exactly 2. It calls Emergency Read and reads the crew only if that returned true. The crew
reference requests **IID `0xA9B40F05`**, not a fire-crew class ID or an occupant IID. It is a single
reference, not an inline crew record or a count/list. The writer emits this reference only after
Emergency Write succeeds. Both methods preserve the inherited Boolean result; the reference helper's
return and a fresh stream-error test are not used to replace it.

On Read the crew is first resolved into a null-initialized temporary. If that output differs from the
old crew, the routine explicitly AddRefs a non-null new crew, stores it and Releases a non-null old
crew, in that order. An identical output leaves the member and counts alone. Null therefore clears an
old crew without itself making FireTruck Read false. This is the wrapper's explicit ownership work,
in addition to whatever the shared reference loader does. Evidence: write `0x00532EEB–0x00532F12`;
read check/delegation `0x00532F32–0x00532F52`, IID and assignment `0x00532F55–0x00532F99`, retained
result `0x00532F9D`.

These concrete classes' distinct class IDs select their own wrappers. The standalone Emergency
class-ID collision in §12 quirk 3 does **not** shift the record nesting when an Ambulance or FireTruck
is loaded. Their common current version prefix down to Occupant is `1, 2, 2, 2, 8, 4` for Ambulance,
or `2, 2, 2, 2, 8, 4` for FireTruck. PatrolCar's similar framing is not the evidence for FireTruck.

**Worked Ambulance case.** Use successful V of length L, target `(1,2,3)`, null station, attempts 2,
call number `0x1234` and dispatch state 3. E's tail is 12 vector bytes, 4 null-reference bytes, then
`02`, `34 12`, `03 00 00 00`: **23 bytes**. Thus E is L+25 bytes and Ambulance is **L+27** bytes.
Read restores all five tail values without running dispatch-state transitions. Outer Ambulance 2
rejects after two bytes; outer 1 followed by Emergency 1 rejects after four, with no Vehicle read.

**Worked FireTruck case.** Use that same E and a crew reference with object ID 37 and class
`0x4A222E4D` (SimFireCrew), whose resolved object C answers `0xA9B40F05`. Its reference occupies
eight bytes, so the whole FireTruck record is **L+35** bytes; with a null crew it is **L+31**.
Starting with old crew X distinct from C, expect the wrapper's AddRef(C), store C, Release(X), after
the helper resolves C. Loading a null crew instead clears X and still returns true if E returned
true. If Emergency's version is wrong, only the first four bytes are consumed, false is returned
and the existing crew is untouched. No crew tail is read or written after inherited failure.

#### 6.4.3 Tank record framing and saved model indices (AT31; branch AT26)

Tank (`0xEBE0DCAD`) writes **`u16 2`; V; `s8` first auxiliary model index; `s8` second auxiliary model
index**. This corrects the earlier "Vehicle plus two dwords" wording in [vehicles.md](vehicles.md)
§§1, 6. The tail is **two bytes total**, not eight; the leading wrapper is another two bytes.
There are no Tank-specific serializable references or extra IIDs.

The indices refer to the inherited model-instance list, not yaw, barrel elevation, power, raw model
pointers or resource keys. At the serialization interface R, the list occupies `R+0x108/+0x10C`,
the two live auxiliary handles are `R+0x354/+0x358` and Read stores the two saved bytes at
`R+0x350/+0x351`. R is primary P+8: these are respectively P's list `+0x110/+0x114`, handles
`+0x35C/+0x360` and saved bytes `+0x358/+0x359`. Init requests model part 1 for the first handle and
part 2 for the second, or rebinds the saved indices into the loaded list (`0x0054C8AC–0x0054C9A0`).
The primary-view `+0x350/+0x354` elevation/power fields in vehicles.md §12.13 are different fields.

Write computes each index afresh by scanning the model-instance list in order for pointer equality
with its corresponding handle. The initial value is −1 (`FF`). Every match replaces the value with
the low eight bits of that zero-based position, so the **last match wins**; an absent handle remains
`FF`. It writes first-handle index before second-handle index. The signed byte interpretation matters
to subsequent Init; Read itself performs no range or uniqueness validation. Evidence: initial index
bytes and scan `0x0054AA57–0x0054AAA7`, byte writes `0x0054AAB3–0x0054AAC8`. Stream slot `+0x10`
is signed-byte input/output (`0x00AA2280`/`0x00AA2310`), not the dword slot `+0x24`.

Read accepts unsigned outer versions **0, 1 or 2**, rejecting values greater than 2 after two bytes.
It then calls Vehicle Read. Only when Vehicle returns true **and** Tank version is 2 does it read
the two index bytes. For versions 0/1 there is no Tank tail and existing saved-index bytes remain
unchanged. Following any accepted Tank version, its final result is **true iff the stream's GetError
is zero**, even if Vehicle Read returned false. There is no rewind or rollback. In contrast, Write
returns Vehicle Write's Boolean and writes the two bytes only when that result is true; it does not
replace that Boolean after tail errors. Evidence: wrapper write `0x0054AA3E–0x0054AA55`; unsigned
version/read gating `0x0054A8F4–0x0054A930`; final GetError/result `0x0054A933–0x0054A940`.

**Worked cases.** With model list `[body, A, B, A]`, first handle A and second B, successful Write is
`u16 2`, V of length L, then **`03 02`**, total **L+4** bytes. With neither handle in the list, its
tail is `FF FF`. A historical version-1 record ends immediately after V (L+2 bytes); loading it into
an object with saved bytes 7 and 9 leaves those bytes 7 and 9. To distinguish the failure quirk, feed
`02 00 03 00 AA BB`: Tank 2 accepts, Vehicle rejects version 3, neither index is read, and a stream
whose successful word reads leave GetError=0 makes Tank return **true after four bytes**, leaving
`AA BB`. Changing Tank's first word to 3 instead returns false after two bytes. A real stream error
after an accepted Tank version makes the final result false.

#### 6.4.4 Mass-transit train record and failures (AT32; branch AT27)

MassTransitTrain (`0x4C22AE53`) is **`u16 1`; complete Train record**, without a further derived tail.
Expanded through the train-specific fields, its exact order is:

| Order | Field |
|---|---|
| 0 | `u16 1`, MassTransitTrain's own version |
| 1 | `u16 2`, Train version |
| 2 | `u16 2`, TrainCar version |
| 3 | complete V, including its own Vehicle and Traffic versions |
| 4 | `u32` car-reference count, then that many `ref`s requesting IID `0x4A416254` |
| 5 | raw `u32` field at the train serialization view `+0x2C0` |
| 6 | `vec3` at that view `+0x2D4` |
| 7 | independent network-occupant `ref`, requesting IID `0xA821EF94` |
| 8 | raw `u32` field at that view `+0x2E4` |

The raw dwords round-trip bit-for-bit. The network reference in field 7 is additional to the traffic
network reference inside V. All other inherited reference IIDs are those in §6.4.1. The current
version-word prefix down to Occupant is **`1, 2, 2, 2, 2, 8, 4`**. See
[vehicles.md](vehicles.md) §12.1 for the inherited Train/TrainCar contract.

MassTransitTrain accepts **exactly 1**; Train and TrainCar each require **exactly 2**. An outer
mismatch returns false after two bytes without calling Train. A matching outer version delegates
Read/Write and returns Train's result, without another stream-error test or tail operation. Train
rejects a bad Train or TrainCar version before entering the next layer; Vehicle failure prevents all
train-tail reads/writes. Nothing rewinds a consumed version or undoes earlier changes.

After successful Vehicle Read, Train consumes every declared car reference, appending each non-null
resolved car in order and skipping nulls. It does not clear a pre-existing car list. It then reads
the first raw dword and vector, releases its old trailing network reference, resolves its replacement
and reads the last dword. Null cars or a null trailing network reference do not introduce a separate
false result. The train-tail getter/helper results do not replace the nested Vehicle Boolean on
normal return; the derived wrapper adds no such check either. Framework exceptions remain possible.
Evidence: outer writes `0x0053CB3A–0x0053CB44`, read/check/call `0x0053CB61–0x0053CB76`; Train writes
`0x0055793B–0x005579D1`, version checks `0x0055AAE5–0x0055AB32`, car IID/append
`0x0055AB57–0x0055ABB0`, trailing fields and retained result `0x0055ABB2–0x0055AC0A`.

**Worked case.** Use V of length L, three car references `[A, null, B]` with non-null eight-byte refs
to TrainCar objects, first raw dword `0x11223344`, vector `(1,2,3)`, null trailing network reference
and final raw dword `0x55667788`. The whole record is **L+54** bytes: six wrapper bytes before V,
four count bytes, 20 car-reference bytes, 4+12+4+4 tail bytes. Starting with car list `[X]`, successful
Read leaves `[X,A,B]`, releases an old trailing network object and stores null, and preserves both
raw dwords. With no cars and null trailing network, length is L+34 instead. Prefix versions
`2`, `1,3`, `1,2,3`, or `1,2,2,3` reject after 2, 4, 6 or 8 bytes respectively, without reading the
train tail. A reader treating this as plain Train would fail on its first word; omitting either
TrainCar's wrapper or the train tail is not an equivalent encoding.

§6.5 and §6.6 below are **behaviour, not records**: what each state type does per frame. They sit under §6
because they belong with the state types; nothing in them is saved.

### 6.5 Per-tick behaviour of the state types *[compat]*

Every claim here is from Windows `SimCity4.exe` 1.1.641 Ghidra pseudocode and Mac Rev A pseudocode
(`external/extracted/sc4-mac-symbol-analysis/artifacts/code/Mac-RevA-x86-named/functions/`). **Platform note
(re-integrate1):** for the upper automaton the Mac object offsets run `+0x10`–`+0x18` **above** the Windows ones
for fields ≥ roughly `+0x1A0` (verified: TweenStruct Win `+0x1A0` / Mac `+0x1B0`; network occupant Win `+0x1EC` /
Mac `+0x1FC`; path vector Win `+0x1F8` / Mac `+0x208`), while `+0x40 … +0xB8` agree. **Vtable slots agree up to
slot 62; from slot 79 upward Mac slot = Windows slot + 7** (verified pairs: `DoCollideTick` Win 120 / Mac
`+0x1FC`·127; blocked wait Win `0xA3` / Mac `+0x2A8`·170; station Win `0xA4` / Mac `+0x2AC`·171; plummet Win
`0xB9` / Mac `+0x300`·192; emergency 100 Win `0xC3` / Mac `+0x328`·202). Offsets in this section are **Windows**
unless a Mac address is cited.

**Two corrections to the slot tables of §7.4 / §11** (re-integrate1):

- **`MoveTo` is slot 16 / `vtable+0x40`**, not slot 8 or 14. `cSC4Automaton::MoveTo(float const* pos, float
  const* orient)` (Mac `0x000c53a4`) takes either pointer null. It writes `this+0x40/0x44/0x48` from `pos`,
  `this+0x58/0x5C/0x60` (normalised) from `orient`, rebuilds the `cS3DTransform` of every model in the instance
  vector at `this+0x110` and does the 1/16 cell bookkeeping. `SetPosition` (Mac `0x000bc802`) and `SetOrientation`
  (Mac `0x000c58ca`) are separate 1-argument methods and are **not** what these ticks call.
- **Frame `dt` is milliseconds** (`cSC4Vehicle::DoAnimationTick` Mac `0x000a0c52` computes `param_2/1000.0`;
  `DoPullOverTick` computes `radius*1.5*dt/1000.0`; countdowns compare against `1000.0` and are initialised
  `rand*1000.0`). **Tween durations are seconds** — `cSC4TrafficAutomaton::DoAnimationTick` (Mac `0x000cad9e`)
  steps the speed tween as `elapsed += dt/1000.0`, and the plummet splash pairs `TweenOpacity(0, r)` with
  `state+0x38 = r*1000.0` ms.

**TweenStruct** (automaton `+0x1B0` speed, `+0x1C0` opacity; Win `+0x1A0`/`+0x1B0`):
`+0x0` start, `+0x4` target, `+0x8` duration, `+0xC` elapsed. `GetCurrent` (`0x004FA960` / Mac `0x0062c82a`) =
`start + clamp(elapsed/duration, 0, 1)·(target−start)`, or `target` when `duration ≤ 0`. Two setter slots, both
`(target, durationSeconds)`:

- **slot 33 / `+0x84` = `TweenOpacity(target, dur)`** *[inferred name]* — called `(1.0, 0)` for an instant opaque
  at player-drive entry (Mac `0x000c0c8e` case `0x10`), `(0, tuning+0xBC)` on entering state `0x67`,
  `(1.0, tuning+0xBC)` in the teleport tick, `(0, r)` for the plummet splash.
- **slot 36 / `+0x90` = `TweenSpeed(target, dur)`** *[inferred name]* — `SlowDownForStopLight` (Mac `0x000ca9de`)
  `(0, 1.5/m)`, `ReturnToDefaultSpeed` (Mac `0x000caa42`) `(defaultSpeed, 0.3/m)`, pull-over `(0.0040365, 1.0)`,
  station entry `(0, 0.5)`, path-following `(leaderSpeed, 0.5)`.

**Automata flag bits**, from `GetDebugFlagsText` (`0x00511A90` / Mac `0x000c3346`) — this table supersedes the
shorter one in §6.1 field 4: `0x1` remove · `0x2` self control · `0x4` reached dest · `0x8` collide · `0x10` stop
light · `0x20` stopped · `0x40` special route · `0x80` Emergency! · `0x200` repathing · `0x400` teleporting ·
`0x800` ignore lights · `0x2000` (checked before path-following) · `0x20000` · `0x40000` · `0x4000` ·
`0x80000` (player-drive helpers) · `0x10000` · `0x2000000`.

**MATCHES / DIFFERS / PARTLY** against the §6.2 text as it stood before this lane (re-integrate1):

- **MATCHES** — the type→class→size table and the Write/Read addresses; the base record's *types and order*
  (`u16` v3, `ref` controller, `u32`, `f32`, `bool`, `u32`, `u32`, and a final `u32` in v3, with `+0x24` zeroed
  on Read; Mac `0x000c3aa0`/`0x000c43f8` confirm field for field); "type 13 is the traffic automaton's blocked
  wait", the push with `t × 1000.0`, flag `0x10`, and the early return while it is on top; the derived
  type→name mapping and the Lua table; "random walks add [the attraction vector] to their step"
  (`0x000c4b58`, with the guard flag `ignore_attractors` = `0x4000`); Collide having no extra record fields.
- **DIFFERS** — five points, each now corrected in place: (1) `+0x0C` is a stack index recomputed after any
  stack mutation, not a plain saved `u32`; (2) `+0x14` is `final` and makes `SetState` a no-op, so the riot
  end's `SetState(1, …)` does nothing to the leader; (3) Plummet's stream order is `+0x28, +0x34, +0x30, +0x2C,
  +0x38` and there is an unsaved sixth `ref` at `+0x3C`; (4) AttachAnim's `bool` sits at `+0x2C` before the
  `u16` at `+0x30`; (5) Chase is a pure record with no tick. See §12 quirks 10–15 and the §6.2 layouts.
- **PARTLY** — Bumped (0x11) has a record and a class but **no tick anywhere** (quirk 13); type 0 `DEFAULT` is
  not "nothing" but the generic seek-to-destination (quirk 16); and for types 50–58 / 70–78 the spec's "base
  only, except 52" is true of the *record*, but 50, 51, 53, 54, 55, 56, 57 and 70–75, 77, 78 all have real
  per-tick logic — only **58 `CROP_DUST`** and **76 `MARINA_PARKED`** are name-only (§6.5, §6.6).

**Where the two reports disagree** (re-integrate1 — both readings given with their addresses, neither picked):

1. **Plummet's `+0x28` at enter.** The automata report gives `+0x28 = −0.004166667` (−1/240) in
   `HandleEnterState` case `0x0E` (Windows `0x005114A1`), and describes `+0x28` as a descent velocity that
   `DoPlummetTick` advances by a gravity term from `_DAT_00B22980`. The vehicle report gives the same field's
   enter value as `0xBB88888A` = **−0.0080729163** (Win `0x0055E1F0` / Mac `0x000a2e16`, enter at Win
   `0x00511450` / Mac `0x000c0c8e` case `0x0E`), and names the gravity `kfGravityAccel = −0.00083333347`, a
   lazy-init function-local static at `0x00B22980`. **Both agree** on the gravity's address and on the impact
   value `0x3991A2B5` = 0.0002777778 (1/3600); only the enter constant differs, and they differ by roughly 2×.
   Settle by reading the constant in `HandleEnterState` case `0x0E` on both builds.
2. **PullOver's `+0x28`.** The automata report reads `if (+0x28 == 0.0)` as an **"already initialised" latch**
   (Win `0x0055E550` / Mac `0x000a2978`): 0 seeds `+0x2C..+0x34` from the orientation and `+0x38..+0x40` as
   its −90° Y-rotation. The vehicle report reads it as an **elapsed-milliseconds counter** — 0 = init,
   `0 < +0x28 < 1000` = driving off, `+0x28 ≥ 1000` = finish — because the tick ends with
   `+0x28 += dt` and the finish branch restores the saved heading and pops. These are compatible only if the
   field is an elapsed counter that happens to be 0 on the first tick; the "latch" reading misses the 1000 ms
   finish threshold and the increment. Settle by watching `+0x28` across a pull-over.

**Per-frame detail for the vehicle and traffic states** (re-integrate1). Offsets are Windows; the platform note
at the top of this section applies.

- **Plummet (`0x0E`)** — `cSC4Vehicle::DoPlummetTick`, Win `0x0055E1F0` / Mac `0x000a2e16`, record `0x40`.
  Fields: `+0x28` vertical velocity, `+0x2C` Y-spin rate (enter 0.5), `+0x30` Y-orientation centre (copy of
  `this+0x5C`), `+0x34` tumble countdown ms (enter `U(1.0, 2.5)·1000`), `+0x38` post-splash countdown ms
  (enter 0), `+0x3C` water-splash effect pointer (unsaved). Per frame:
  1. flag 1 set → `MoveTo(&newPos, 0)` and return 1;
  2. **tumble while `+0x34 > 0`:** `+0x34 -= dt`; build orientation `(this+0x58, this+0x5C + dt·0.01·(+0x2C),
     this+0x60)`; if the new Y leaves `[+0x30 − 0.05, +0x30 + 0.05]` flip the sign of `+0x2C` (XOR `0x80000000`)
     and clamp Y to the band edge; `Normalize`, `MoveTo(0, &orient)`, return 1. **No translation in this phase.**
  3. **fall:** new position = `(this+0x40, this+0x44 + dt·(+0x28), this+0x48)`;
  4. if `+0x38 > 0`: `+0x38 -= dt`; on `≤ 0` destroy the effect at `+0x3C` (`vtable+0x10(0)`, `vtable+0x08()`,
     then `+0x3C = 0`) and `Remove(0)` (slot 13);
  5. `+0x28 += dt · kfGravityAccel` (`0x00B22980`);
  6. `SL::spCity->vtable+0x188(x, z)` in-city test — outside → `Remove(0)`;
  7. inside: `hi = max(terrain height `spTerrain->vtable+0x30`, water height `spTerrain->vtable+0x7C`)`. If
     `hi < newY` or `+0x38 > 0` nothing happens; otherwise the surface is reached and `+0x28 = 0x3991A2B5`
     (1/3600). **Water** (`h1 < newY`) starts the splash: `spEffectsManager->vtable+0x18(_kWaterSplashEffectName,
     &state+0x3C)`, an identity transform translated to the new position, `vtable+0x0C(0)`, `TweenOpacity(0, r)`
     (slot 33) and `+0x38 = r·1000.0` — **no `Remove` here**. **Land** sets `+0x38 = 0` and falls through to
     `Remove(0)`;
  8. `MoveTo(&newPos, 0)` (slot 16), return 1.

  It ends via `Remove(0)` — either on terrain impact or leaving the city, or `+0x38` ms after the water splash
  expires. **No `PopState`/`SetState`**: the state is never popped, the automaton is destroyed.

- **PullOver (`0x15`)** — `cSC4Vehicle::DoPullOverTick`, Win `0x0055E550` / Mac `0x000a2978`, record `0x44`.
  Ctor (Mac `0x0063817a`) sets `+0x28 = 0`, `+0x2C = 1.0`, `+0x30..+0x38 = 0`, `+0x40 = 1.0` — the unit pair
  `(1,0,0)` and its −90° Y-rotation `(0,0,1)`. `+0x2C..+0x34` is the pull-off heading (unit `vec3`), `+0x38..+0x40`
  the sideways drift direction (the heading rotated `−1.5707964` about Y). Per frame:
  1. flag 1 set → return 1 immediately (no timer advance);
  2. branch on `+0x28` (see the conflict note above): **0 (init)** → `TweenSpeed(0x3B88888A = 0.0040364582, 1.0)`
     (slot 36) to creep off the road for 1 s; seed the two vectors from `this+0x58/5C/60`; then ask
     `controller->vtable+0x18` → `->vtable+0x20` for a point, and if `dot(position − point, +0x38..+0x40) < 0`
     negate all three components of `+0x38..+0x40` so the vehicle drifts to the correct side. **`0 < +0x28 < 1000`**
     (moving off) → only if `GetSpeed()·dt > 0` (slot 34): a sideways step of length `this+0x7C (radius) · 1.5 ·
     dt/1000.0` plus a heading step of length `speed·dt`; `spTrafficNetworkMap->vtable+0x48(x, z)` snaps the new Y
     to the terrain; the normalised delta becomes the orientation (written only when its Y is finite — an explicit
     NaN/Inf test); `this+0x94 = 0`; `MoveTo(&newPos, &newOrient)`. **`+0x28 ≥ 1000`** (finish) → restore the saved
     heading into `this+0x58/5C/60`, `MoveTo(0, &orient)`, then `spCity->vtable+0x17c(x, z, &a, &b)` and
     `spTrafficSim->vtable+0x64(1, a, b)`; **if that returns > 3 → `PopState()`** (slot 53); then
     `controller->vtable+0x5C(controller, &this+0x40, 0)` and **if it returns false → `PopState()`** again;
  3. always (flag 1 clear): `+0x28 += dt`.

  It ends by `PopState()` only — the automaton is not removed.

- **Blocked wait (13, `0x0D`)** — `cSC4TrafficAutomaton::DoStopSignTick`, **Windows `0x005500D0`** (Ghidra
  leaves it as `FUN_005500D0`) / Mac `0x000cbe5e`, Windows slot `0xA3` / Mac `+0x2A8`. Record is base only
  (`0x28`), so it uses the base `+0x10` countdown. Per frame: if `state+0x10 > 0`, `CheckAutomataFlag(0x20)` —
  **clear → return 0** (no-op); set → `state+0x10 -= GetSpeedModifier() · dt` (`FUN_00505AB0` / Mac `0x000bb5e6`,
  which returns `cSC4AutomataManager::sfSimSpeedModifier`, clamped to 0/1 when flag `0x80` is set) and return 1
  while still > 0. On expiry: slot `0xA0` (`+0x280` Win / `+0x29C` Mac) = **`ReturnToDefaultSpeed`**
  (Windows `0x0054DA50`; AT19 answered — see below), then `SetAutomataFlag(0x20, 0)` and `SetAutomataFlag(0x10, 0)`,
  then `map = spCity->vtable+0xA0` — **`cSC4City::GetTrafficNetworkMap`** (city interface slot 40) — and
  `map->vtable+0x84(map, this+0x1EC /* network occupant, Mac +0x1FC */, this, routeInfo, &out)` where
  `routeInfo = this->vtable+0x88(this)` (Win `+0x220` / Mac `+0x23C`) — that method is
  **`cSC4TrafficNetworkMap::CheckIntersection`** (`cISC4TrafficNetworkMap` slot 33, `0x004B06F0`; AT18
  answered): it looks up the automaton's approach lane for its cell and facing and returns the **count of
  automata occupying that lane**, writing the wait delay into the out `f32` (`*out = FLT_MAX` when nothing was
  found). **If it returns 0 → `PopState()`** (the approach lane is free — the way is clear); a non-zero return
  keeps the wait state. Return **0**. No position, destination, orientation or model changes.

- **STOP_AT_STATION (30)** — `cSC4TrafficAutomaton::DoStopAtStationTick`, Win `0x0054E3D0` / Mac `0x000ca8bc`,
  Windows slot `0xA4` / Mac `+0x2AC`. It is a **speed-tween wrapper**, not station logic: read
  `TweenStruct::GetCurrent(this+0x1B0)` (Win `+0x1A0`); if `t > 0` call slot `0x81` = `DoPathTick(this, dt)` to
  keep driving while the tweened speed has not reached 0; then `cSC4Automaton::DoStateTick(this, dt)`; return 1.
  Entry (Mac `0x000cf390` case `0x1E`) is `TweenSpeed(0, 0.5)`. The **bus** variant
  `cSC4Bus::DoStopAtStationTick` (Win `0x00524180` / Mac `0x0039155e`) does the real dwell: requires flag `0x20`
  or 4; if `state+0x10 ≤ dt` run the base tick and return (dwell expired) else `state+0x10 -= dt`; then
  `RandomUint32Uniform` skips roughly half the frames; query `spOccupantManager->vtable+0x90` for occupants in an
  AABB of `kfNearbyPedRadius` around `this+0x40`/`+0x48`, filter `0x496E5783`, and for each hit `QueryInterface
  (0xA9B40F05)` then `GetTopStateType()` (slot 59) — **if it is `4` QUEUE, `Remove(0)` that pedestrian** (slot 13)
  and stop scanning. If no queued pedestrian was boarded → `PopState()`; if one was, keep dwelling.

- **Emergency vehicles 100–103, and 104 for patrol cars** — `cSC4EmergencyVehicle::DoStateTick` Win `0x0052F490` /
  Mac `0x00235632`. Empty stack or `GetState() == 0` → return false. Then switch on `state+4`:

  | Type | Name | Win slot | Mac `vtable+` | Method |
  |---|---|---|---|---|
  | 100 `0x64` | `EN_ROUTE` | `0xC3` | `0x328` | `DoEnRouteTick` Win `0x0052FD50` / Mac `0x0023571c` |
  | 101 `0x65` | `REPATHING` | `0xC4` | `0x32C` | `DoRepathingTick` Win `0x0052FE50` / Mac `0x00235abe` |
  | 102 `0x66` | `HANDLING_EVENT` | `0xC5` | `0x330` | `DoHandlingEventTick` (base stub Mac `0x00234a28`) |
  | 103 `0x67` | `TELEPORT` | `0xC6` | `0x334` | `DoTeleportTick` Win `0x0052F570` / Mac `0x00235d84` |
  | 104 `0x68` | `ARREST` (patrol car only) | `0xC8` | `0x33C` | `cSC4PatrolCar::DoArrestTick` Win `0x00544060` / Mac `0x00397b62` |

  Any other type falls through to `cSC4Vehicle::DoStateTick`; if that returns false, the result is
  `GetState() !=` the pointer captured at entry (i.e. "the state stack changed"). Naming evidence
  (AT12 answered 2026-10-20, re-answers33): the **dispatch states 0–6** are named by the shipped debug
  strings in `cSC4EmergencyVehicle::GetDebugExtraText` (`0x00532290`, jump table `0x00532414`):
  `0 "[at station]"`, `1 "[route blocked]"`, `2 "[done]"`, `3 "[en route]"`, `4 "[no resources]"`,
  `5 "[handling event]"`, `6 "[returning home]"` (strings at `0x00A98574`…`0x00A98514`). The state types
  100–103 are tied to those names by the code that pairs them: `SetDispatchState(3)` + `SetState(100)`
  (`0x0052FF60` case 3), `FUN_0052F210` does `NotifyCallbacks(5, 0)` + `SetDispatchState(5)` +
  `SetState(0x66=102)` on arrival at an event, or `SetDispatchState(1)` + `PushState(0x65=101)` +
  `PushState(7, …)` on a blocked route, and `FUN_0052F330` does `SetDispatchState(6)` +
  `PushState(0x67=103)`. So 100 `EN_ROUTE`, 101 `REPATHING`, 102 `HANDLING_EVENT`, 103 `TELEPORT`
  stand on the shipped strings plus these pairings, not on a name table (none exists in the exe or Lua).
  `HandleEnterState` case `0x67` fades opacity
  out, which only the teleport tick fades back in. Slots 25/26 are `SetDispatchState(int)` / `GetDispatchState()`
  (offsets `0x64`/`0x68`).

  - **`DoEnRouteTick`** — while the route is unfinished and the network occupant (`+0x1FC`) is set: flag
    `0x2000` clear → `cSC4Vehicle::DoPathTick`, else `cSC4TrafficAutomaton::DoStraightLineTick`. When finished:
    `Remove(0)` or `HandleReachedRouteDestination`. `ValidateNetworkOccupant` failing → `Remove(0)`. Otherwise,
    if the route deque is non-empty, flag `0x20` is set on both this automaton and its leader `+0xB0`, slot `0x94`
    (Mac `+0x26C`) supplies a replacement leader. Returns 1.
  - **`DoRepathingTick`** — `spCity->vtable+0xAC` is the path-finder. On success: `SetDispatchState(3)`,
    `PopState()`, retry counter `this+0x2D8 = 0`. Otherwise increment `this+0x2D8`; while `≤ sTuningParams[0xF1]`
    send the path-blocked message (Win `0x0052F910`) and `PushState(7, 0, sTuningParams[0xEC] · 1000.0, 0, 0, 0, 0)`
    (IDLE for that many ms); past that, `Remove(0)`. Fields: the incident `vec3` at `+0x2C8/0x2CC/0x2D0`, the retry
    counter `u8` at `+0x2D8` (Windows ≈ `0x2B8/0x2BC/0x2C0`, `0x2C8`).
  - **`DoTeleportTick`** — read the **opacity** tween at `this+0x1C0`; while `t > 0.6` (or NaN) return 1 (still
    fading out). Then `TweenOpacity(1.0, sTuningParams[0xBC])` to fade back in. If `this+0x2D4 == 0` (the
    `ref` occupant `0x26BB767E`, the incident) → `Remove(0)`. If `|cur − target| < (u8)sTuningParams[0xF0] · 16`
    (cells → metres) → `Remove(0)`. Otherwise `SetDispatchState(6)` and request `{0, this+0x2D4, 1, position}` from
    `spCity->vtable+0xAC`: on success `this+0x2D8 = 0` and **`SetState(0, 0, −1.0, 0, 0, 0, 0)`** (snap to
    DEFAULT and drive the new route); on failure `Remove(0)`.
  - **`cSC4FireTruck::DoHandlingEventTick`** Win `0x005335F0` / Mac `0x003988ae` (the Windows decomp fails; the
    Mac is authoritative) — from `GetFireCount` / `GetToxicSpillCount`: dispatch 2 with work outstanding →
    `SetDispatchState(5)`; no fire left while dispatch 5 and not player-driven → `SetDispatchState(2)`;
    dispatch 2 with nothing left → `ReturnToSourceOccupant()` (slot 29). No movement, no model changes.
    `cSC4PatrolCar::DoHandlingEventTick` (Mac `0x00397afa`) is thinner and checks the riot manager
    (`spCity->vtable+0x120` → `vtable+0x28`).
  - **`DoArrestTick`** — dwell on the base `+0x10`, then swap model 0 to the arrest animation and wait for that
    animation to finish, then `PopState()`. While `state+0x10 ≤ 0`: if the model's current frame
    (`vtable+0x78`) is `≥ frames − 1` (`vtable+0x7C`) → `PopState()`. Otherwise `state+0x10 -= dt`, and on
    reaching `≤ 0` this tick query `spCrimeSim` (`vtable+0x24` / `vtable+0x28` at 20.0 units) and either push
    `SetState(7, 0, 5000.0, 1, 0, 0, 0)` on a suspect or loop up to 5 times on `vtable+0x2C(1)`; then build the
    key `(this+0x11C, +0x120, +0x124)`, pick `_kArrestAnimationInstance[index]` with
    `index = (int)((pol->vtable+0x64(…) · 100.0) / 40.0)` when `this+0x2D4` is set and the police sim agrees,
    `ReplaceModelInstance` (Mac `0x000c11fc`) and play it (`model[0]->vtable+0x8C(1)`).
  - **BUMPED (`0x11`) and COLLIDE (`0x14`)** — `cSC4Vehicle::DoBumpedTick` is a 7-byte `return 0` (Mac
    `0x000a083e`); `cSC4Automaton::DoBumpedTick` is `return !CheckAutomataFlag(1)` (Mac `0x000bb14e`). There is
    no `0x11` case anywhere, so a vehicle's Bumped state expires only through the base countdown's `PopState`.
    `cSC4Vehicle::DoCollideTick` (Mac `0x000a0846`, 29 bytes) tail-calls `cSC4Automaton::DoCollideTick`
    (Win `0x00508D70` / Mac `0x000bdbc8`, Windows slot 120 / Mac `+0x1FC`). That tick: flag 1 set → return 0;
    `*(byte*)(this+0x33) & 2` → `SetVisible(false)` (slot 9); apply the saved crash transform at `this+0x170` to
    the front model (`this+0x110`) through `->vtable+0x20`; `*(byte*)(this+0x33) & 1` → notify
    `spOccupantManager->vtable+0x60(&this+4, 0)`; every time `*(f32*)(this+0x1D0) > 100.0` emit event id 9 and
    reset it. Return 1. No position, destination or speed changes.

- **`DoPathTick` / `DoStraightLineTick`** (movement, driven every frame from `DoAnimationTick` — see above):
  `cSC4Vehicle::DoPathTick` Win `0x0055CAF0` / Mac `0x000a4450` and `cSC4TrafficAutomaton::DoPathTick`
  **Win `0x0054FDC0`** / Mac `0x000d12fe` both advance along the path with
  `step = GetSpeed() · speedScale · dt`, matching the leader's speed (`TweenSpeed(leaderSpeed, 0.5)`, slot 36)
  when the distance remaining is under 16.0, flagging `0x20` ("stopped") and `4` ("reached dest") at the ends,
  and `Remove(0)`ing when the step goes below `−GetMaxSpeed()` and flag `0x40` is clear. **`this+0x94`**
  (AT19 answered): `cSC4Vehicle::DoPathTick` writes `this+0x94 = |newPos − oldPos| / dt` — the measured
  current speed in units per millisecond (`FDIV [ESP+0x50]` = dt, `FSTP [ESI+0x94]` at `0x0055CFBC`).
  **The traffic version does not write it** (`0x0054FDC0` has no `+0x94` store), so the "both DoPathTicks"
  reading was wrong. `cSC4Vehicle::DoPullOverTick` zeroes `+0x94` at the end of its move tick
  (`MOV [EDI+0x94],0`, `0x0055E95C`). No reader of `+0x94` was found in any tick; the one consumer found is
  `cSC4Train::HandleCollision` (`0x00558E00`), which copies the *train's* `+0x94` onto each coupled vehicle
  before spinning them — a debug/derived-speed field *[inferred]*. GetSpeed itself (slot 34, `0x004FB370`)
  reads the **speed tween** (`ADD ECX,0x1A0; JMP TweenStruct::GetCurrent`), not `+0x94`. The vehicle version
  writes a back-anchored destination `out − radius·0.5·facing` and keeps the path vector; the traffic version
  writes the path-advance output straight into the destination field and passes nulls for the path vector.
  `cSC4TrafficAutomaton::DoStraightLineTick` Win `0x00550190` / Mac `0x000d2c62` forces destination Y to 0,
  flags arrival without snapping to the destination on the last frame, and `Remove(0)`s when the collision query
  `obj->vtable+0xB0(obj, &position, &newPos, flags)` fails.

**The behaviour table.** "Returns 1" means the state moved itself (so `DoAnimationTick` does not seek);
"returns 0" means it did not.

| # | Lua name | What it does per tick | Extra record fields |
|---|---|---|---|
| 0 | `DEFAULT` | nothing of its own; `DoAnimationTick` seeks `destination` at `GetCurrentSpeed()` and sets flag **4** on arrival | none |
| 1 | `DISAPPEAR` | each tick `Remove(0)` unless flag 1 already set; returns 1 | none |
| 2 | `BEE_LINE` | normalise automaton `+0xA4/+0xA8/+0xAC`, `MoveForwardWithCollision` (`0x00546150`) | none |
| 3 | `CROWD` | walk toward the controller's attractor; consume a queued push at `+0x25C` (flag at `+0x258`); cycle random reaction anims (`0x00545B10`) | none |
| 4 | `QUEUE` | controller does queue bookkeeping; `DoStraightLineTick`; with flag **0x20** face the queue head and play the idle anim (`0x00545690`) | none |
| 6 | `STRIKE` | re-path toward a shrinking arrival box around the strike target; hands off to `ReturnToSourceOccupant` (`0x00544950`) | none |
| 7 | `IDLE` | `CheckAutomataFlag(1)` gate then a controller/behaviour refresh (`0x005096B0`, `FUN_005055C0` `0x005055C0`); watercraft also accumulates a 60 s lifetime idle budget. The tick is: `PopState()` first (slot 53); if the pop succeeded return 0, otherwise run the behaviour refresh (automaton slot 105, `0x005096B0`) and return 1 | none |
| 8 | `RIOT_LEADER` | walks the road grid picking a walkable heading per cell; dies if it leaves the network (`0x00546220` / Mac `0x000b37f2`) | `s32` last cell X, `s32` last cell Z, `u8` heading 0–3 |
| 9 | `RIOT_FOLLOWER` | acquires a leader by IID `0xA9B40F05`, mills about, steers toward it past 10 units (`0x00546530`) | none |
| 10 | `SCRIPTED` | stub: `return !CheckAutomataFlag(1)` (`0x00505690`) | none |
| 11 `0x0B` | `RANDOM_WALK` | walk/idle cycle driven by the behaviour template; adds the attraction vector to its heading (`0x0050CF90`); see the countdown contract under the Lua-key table above | `vec3` heading, `f32` turn countdown, `f32` walk/idle countdown, `bool` idle-anim-playing |
| 12 | `IGNITE_OCCUPANT` | **one-shot**: starts a fire on the controller's target, then `SetState(1,…)` (`0x00545280`) | none |
| 13 `0x0D` | *(none; traffic blocked wait)* | waits out `+0x10` at the speed-modifier rate while flag **0x20** holds, then clears `0x20`/`0x10` and pops if the path is clear (`0x005500D0`) | none |
| 14 `0x0E` | *(none)* | vehicle tumbles then falls for a random 1–2.5 s, then a splash phase (`0x0055E1F0`) | 5 × `f32` + unsaved `ref` at `+0x3C` |
| 15 `0x0F` | `PLOP_JUMP` | vertical hop: `y = max(+0x28, y + dt·velY)`, gravity on `velY` once above the floor (`0x005055F0`); enter velocity `U(30, 50)` **km/h** `× 1/3600` → 0.0083–0.0139 m/ms, gravity `−5.5555563e−05` m/ms² (lazy `0x00B22740`, guard `0x00B22744`), so a hop peaks at 0.62–1.7 m over 150–250 ms (AT9 answered) | `f32` floor Y, `f32` vertical velocity |
| 16 `0x10` | `PLAYER_DRIVE` | 13 elapsed-time channels tick up; `TranslatePlayerDriveCommands`; forces flags `0x2000000`, 2, `0x10000` (`0x00508C50`); the saved `+0x28` is the drive command accumulator integrated by `PlayerDriveUpdateSpeed` (AT6 answered) | one saved `f32` + 13 × 9 runtime channel arrays |
| 17 `0x11` | `BUMPED` | **stub on both base and vehicle** (`return !CheckAutomataFlag(1)` / `return 0`); nothing anywhere reads the record | 4 × `vec3`, no reader anywhere (AT7 answered) |
| 18 | `MAGNET` | move straight at the controller's position at the speed-tween rate; sample terrain Y unless flag `0x40000` (`0x00508E10`) | none |
| 19 `0x13` | `ATTACH_ANIM` | advances an attached model's animation; pops when the frame test passes (`0x00508F80`) | `ref`, `u16`, `bool` |
| 20 `0x14` | `COLLIDE` | registers a `*_collide_fx` effect, hides the automaton, poses the model; pushed with a **5000 ms** timeout and `final` (`0x00508D70`) | none |
| 21 `0x15` | `PULL_OVER` | vehicle steers through a rotating pull-over arc (`0x0055E550`) | one saved `f32` |
| 30 | `STOP_AT_STATION` | only moves while the speed tween > 0; the bus variant does the actual stop | none |
| 50 | `RETURN_HOME` | `AI_Land`; pops when landed | none |
| 51 | `GO_TO_DESTINATION` | `AI_GotoDestination`; pops on arrival or when the destination is the uninitialised vector | none |
| 52 `0x34` | `CHASE` | **record only** — holds the chase target; the helicopter reads it | `ref` automaton |
| 53 | `LOITER` | hover 2 s (airplane, spending one ammo count) or the pushed timeout (helicopter) | none |
| 54 | `LOOK_FOR_CRIME` | helicopter only: claim a crime cell, then LOITER + GO_TO_DESTINATION | none |
| 55 | `LOOK_FOR_TRAFFIC` | helicopter only: claim a busy traffic cell, then LOITER + GO_TO_DESTINATION | none |
| 56 | `RESCUE` | helicopter only: push RETURN_HOME, then claim the most recent accident location | none |
| 57 | `PUT_OUT_FIRE` | airplane only: `FindFireNearLocation`, then GO_TO_DESTINATION + LOITER while ammo lasts | none |
| 58 | `CROP_DUST` | **name only** — `AI_PursueGoal` has no case; it just pops. The helicopter dispatch (`0x0053BBA0`: byte table `0x0053C5A0`, jump table `0x0053C57C`) sends every unlisted type 1–`0x31` and anything above `0x38` to its own default block `0x0053C437`, which is **PopState only** — there is **no fall-through** into the `0x32` body (AT17 answered: the earlier decompile put the two blocks adjacent; the byte layout separates them) | none |
| 70 | `CRASH_RUNAGROUND` | heel/pitch ramp over the crash duration, then stops (no transition; the boat stays stuck — AT14 answered) | none |
| 71 | `CRASH_SINK` | sink-angle ramp over the same duration, then stops (no transition; AT14 answered) | none |
| 72 | `IMMEDIATE_REMOVE` | `Remove(true)` every tick | none |
| 73 | `SLOW_REMOVE` | `Remove(false)` every tick | none |
| 74 | `FERRY_DOCK` | `DoDockingTick`; pops when docked (angle < 1° and distance < 1) | none |
| 75 | `FERRY_QUEUE` | `Ferry_DockRequests`: request a berth → push 74, request a wait → push 75, else 73 | none |
| 76 | `MARINA_PARKED` | **name only** — no `DoStateTick` case; enter just zeroes the pitch/roll accumulators | none |
| 77 | `BEING_TOWED` | `DoFollowingTick`: hold station behind the tug at radius + `sfTowDistance` | none |
| 78 | `UNDOCKING` | `DoUndockingTick`; pops when clear of the berth by 2× radius | none |
| 100–103 `0x64`–`0x67` | *(not in Lua)* | emergency-vehicle dispatch states → `DoEnRouteTick` `0x0052FD50`, `DoRepathingTick` `0x0052FE50`, `DoTeleportTick`, `DoHandlingEventTick` `0x005335F0` | none |
| 104 `0x68` | *(patrol car only)* | `ARREST`: dwell on `+0x10`, swap model 0 to the arrest animation, pop when it finishes (`0x00544060`) | none |

**Movement is not driven by the state type** (re-integrate1). The dispatcher is
`cSC4TrafficAutomaton::DoAnimationTick` (Win `0x0054E050`, Mac `0x000cad9e`), run every frame: it steps the
speed tween inline (`elapsed += dt/1000.0`, snapping `start = target` at the end), steps the opacity tween
(slot `0x168`) and another (slot `0x1B8`), and then either `DoPathTick` (slot `0x81`) or `DoStraightLineTick`
(slot `0x82`) depending on whether a network occupant (`+0x1FC`), `+0x200` and the path-point vector `+0x208`
are all set and flag `0x2000` is clear. **State 0 `DEFAULT` and 2 `BEE_LINE` have no `DoStateTick` handler at
all** — `cSC4Automaton::DoStateTick` (Mac `0x000bd464`) switches only on 1, 7, 10, `0x0B`, `0x0F`, `0x10`,
`0x12`, `0x13`, `0x14`; everything else falls to `default` → return 0, and the vehicle keeps path-following from
`DoAnimationTick`. Direct (non-virtual) callers of the two movement ticks: `cSC4EmergencyVehicle::DoEnRouteTick`
and `cSC4Pedestrian::DoQueueTick` (Mac `0x000b2d80`).

**The per-tick behaviour refresh** (`0x005096B0`, automaton slot 105; AT13 answered 2026-10-20, re-answers33).
Called by the `IDLE` tick (`FUN_005055C0`, slot 112: `PopState()` first; on failure run the refresh) and by the
emergency family's override (`cSC4*::vftable_2` slot 146 = `0x0052F730`, which skips it when flag `0x80` is set
or the top state is `0x34` CHASE). Contract:

1. If `CheckAutomataFlag(0x4000)` (ignore attractors) the refresh returns at once.
2. It asks the city (slot `0xBC`) for the occupant-group lookup (controller manager `GetControllerQInfo`
   territory) with the automaton's occupant (`this+4`), the attraction vector `this+0xA4` and an out point
   `this+0x9C`, yielding the **behaviour template** for the automaton's current group. The found template is
   stored at `this+0x98` (the previous one released).
3. If a template was found, it queries the group's queue info (slot `+0x80` with the position `this+0x40`),
   draws a uniform random, and compares it against the group's strength field.
4. Then it walks the **top of the state stack**: while the top state exists, if the state's **controller**
   (`state+0x08`) is null, or its controller's group differs from the automaton's group, the state is popped
   (slot 53 `PopState`); if the state's **behaviour template** (`state+0x24`) already equals the found template
   the walk stops and nothing more happens. Otherwise it pushes the behaviour (slot 55 `PushBehavior`
   = `vtable+0xDC` with the found template and the current one).

So **both `state+0x24` and `state+0x08` are read every refresh**: `+0x24` is compared against the freshly
found template (equal ⇒ no push), `+0x08`'s controller is compared by group (mismatch or null ⇒ the state is
popped first). The refresh runs only in `IDLE` (and the emergency override), not on every tick of every state.

### 6.6 Aircraft and watercraft state families *[compat]*

**There is no Lua name table for the 50–58 / 70–78 names.** A tree-wide search of `SimCity4.exe`'s decomp and
the Mac symbol artifacts finds those names only as copies of this spec; there is no string table or constant
table in the executable. The names `RETURN_HOME`, `CROP_DUST`, `MARINA_PARKED` and the rest are **project
documentation labels for numeric types**. The behavioural answer comes from the switch tables in
`cSC4Watercraft::DoStateTick` `0x00573340`, `cSC4Airplane::AI_PursueGoal` `0x00504590` and
`cSC4Helicopter::AI_PursueGoal` `0x0053BBA0` — not from strings. A later reader should not go looking for a
string table (re-integrate1).

**Watercraft.** Offsets below are Windows (Mac = Windows + `0x10` above `~0x1A0`). Two accumulators:

| Field (Win / Mac) | Name | Behaviour |
|---|---|---|
| `+0x2BC` / `+0x2CC` | **`idleElapsedMs`** | zeroed once in `Init` (`cSC4Watercraft::Init` `0x0056DE40`, `MOV [ESI+0x2BC],0` at `0x0056E2A0`); `DoStateTick` case 7: `+= dt`, and `> 60000.0` (`0x00A9DA40`) → `PushState(0x49 SLOW_REMOVE, 0, −1.0, 0,0,0,0)` (slot 52, `+0xD0`, `0x0057339C`). **Corrected 2026-10-21 (re-boatsped):** there is a second writer, `cSC4Watercraft::DoAnimationTick` (`0x00572D14`–`0x00572D54`), which adds its second time argument when the byte `+0x2CC` is set and pushes `0x49` above 60000.0 on every tick; watercraft.md §5.1. It is never re-zeroed, so it is **cumulative over the boat's life**: 60 s total in `IDLE` (or of animation ticks with `+0x2CC`) slow-removes the boat (AT15) |
| `+0x2C0` / `+0x2D0` | **`crashElapsedMs`** | zeroed on entering 70/71, `+= dt` there; compared against the crash duration `+0x398` / `+0x3A8` to drive the sink/heel ramp |

`FUN_00570270` is **`cSC4Watercraft::DoCrashSequenceTick(float)`** (Windows `0x00570270`, Mac `0x00384d56`).
Enter/exit: `HandleEnterState` `0x0056E340` (Mac `0x0038539e`), `HandleExitState` `0x0056D770` region /
`cSC4Watercraft.c:2762` (Mac `0x00385ca6`).

`eW_ChangeState` — `cSC4Watercraft::SetStateInternal` (Mac `0x00380298`; Windows inlined into `0x00568570`):

| Value | Action |
|---|---|
| 0 | `PushState(0x48 IMMEDIATE_REMOVE, 0, -1.0, 0, 0, 0, 0)` |
| 1 | `PushState(0x49 SLOW_REMOVE, 0, -1.0, 0, 0, 0, 0)` |
| 2 | slot 102/109 `(0xB, 0)`; `PushState(0x46 CRASH_RUNAGROUND, 0, +0x398, 1, 0, 0, 0)` |
| 3 | slot 102/109 `(0xB, 0)`; `PushState(0x47 CRASH_SINK, 0, +0x398, 1, 0, 0, 0)` |
| 4 | `field262_0x2B2 = 1`, then as 3 |

**Slot 102 (`Windows vtable+0x198` / Mac 109) is now named (AT17 answered, re-answers33):** Windows
`0x0050B540`, 2 stack args, `ret 8`. It reads the callback-target list at `this+0x188`; if empty it does
nothing. Otherwise it allocates a `cRZMessage2Standard`, sets the message **type `0x9D92FC3`** (the
automaton-notification message the controller manager handles, §10), the sender to `this`, and the two
payload dwords to the arguments, then broadcasts it to every target in the list. The crash call sites pass
`(0xB, duration)` — in `HandleEnterState` (`0x0056E40B`) the second argument is the crash duration, and in
`Crash_DetermineCase` (`0x00568570`) it is `0`. A proposed name is **`NotifyAutomatonEvent(u32 type, u32
value)`** *[inferred]*. (Slot 80, `0x00505860`, is a one-line wrapper that calls slot 102 with `(6, 0)`.)

`Crash_DetermineCase` (`0x00568570`) tests the four bbox corners against `spSurfaceWater` slot 13. If any
corner is dry it compares `-heading` (`0x30C/0x310/0x314`) with the terrain normal at `+0x2F4`/`+0x2FC`; an
angle above `+0x3A8` gives case 2 (run aground), otherwise case 3 (sink). All-wet gives case 3. Then it reads
**`GetTopStateType` (slot 59, `vtable+0xEC`)** and pushes the crash state only **when the top state is `0x10`
PLAYER_DRIVE** (AT16 answered — the slot-59 read is not a "don't re-crash" guard; it is the **player-drive
test**, and Ghidra's older decompile dropped the `== 0x10` comparison, which is present in the bytes: the
player-driven boat gets `NotifyCallbacks(0xB, 0)` + `PushState(0x46/0x47, 0, +0x398, 1, …)` (duration
`+0x398`); **any other boat gets `PushState(0x49 SLOW_REMOVE, 0, −1.0, …)`**). The whole crash animation is
therefore a **player-drive-only** feature; AI boats just vanish. **What happens when the crash duration
expires (AT14 answered):** states 70 and 71 **never transition** and nothing reaps the boat —
`cSC4WatercraftManager::DoAnimationTick` (`0x005736A0`) contains no state-70/71 cleanup, and no watercraft
function other than `DoStateTick` case 7 and `Init` touches the crash accumulators. The beached or sunk
boat simply sits in its final state until something else ends it: flag 1 removal (`Remove`), the occupant
being removed from the city (`HandleOccupantRemoved`, `0x00574520`), or a new `SetState` (the player taking
the wheel again pushes state `0x10`). A replacement may reproduce the same "stuck forever" behaviour.

**Aircraft.** There is a second, independent enum `cSC4Airplane::State` at `this+0x1FC`, written only by
`cSC4Airplane::SetAirplaneState` `0x004FCEF0` (Mac `0x00495f8e`) and `cSC4Helicopter::SetHelicopterState`
`0x00533DC0` (Mac `0x004aaa50`), values 0, 1, 3. `SetAirplaneState(1)` also sets `this+0x240 = 3` (ammo refill)
and toggles AEMS sound groups 8 / 9 / 10 / `0xD`. **This is not the state stack** — it is a flight/ground phase.
The airplane returns early if the phase is unchanged; the helicopter does not,
and its setter handles phase 4 as well. Exact per-owner sound and engine actions
are in aircraft.md §§3.5/4.2.3. The stack is dispatched by `AI_PursueGoal`.

The airplane implements 50, 51, 53, 57; the helicopter implements 0, 50, 51, 52, 53, 54, 55, 56. **58
`CROP_DUST` is name-only on both.** `LOITER` duration differs: the airplane pushes it with `-1.0` and
self-terminates when a 2.0 s remaining timer reaches **≤0**, plus one ammo decrement
(aircraft.md §3.3.1 gives countdown order, read behavior and zero-ammo semantics); the helicopter pushes it with
**6000.0 ms** from states 54/55/56. None of 50–58 other than 57 has an enter/exit hook.

**Chase (52) and its `+0x28` ref.** Producer `cSC4AutomataManager::ChaseAutomaton`, manager slot 22 (2 args,
Windows `FUN_005072E0`): refuses if the chaser already has state `0x10`, flag `0x40`, flag 1 or state `0x34`
(`FindState`, slot 57); otherwise `PushState(0x34, 0, -1.0, 0, 0, 0, 0)` and
`cRZAutoRefCount::operator=(stateRecord+0x28, target)` — exactly the `tAutomataStateChase` ref at `+0x28`
(IID `0xA9B40F05`). Slot 21 (4 args, `FUN_00511260`) is the radius search that calls slot 22. The **only**
consumer in the state machine is `cSC4Helicopter::AI_PursueGoal` case `0x34` (Windows `0x0053BBA0`, Mac
`0x004b02c8`): pop if `+0x28` is null or the target's `GetTopStateType` (slot 59) is no longer `0x10`
PLAYER_DRIVE; otherwise lead the target (`pos += orient · speed · 1000 · dt`, with dt already in **seconds**), aim the spotlight, teleport when
too far and the opacity is 0, and hover 20 units above it when close (`AI_ControlHover` `0x005353C0`). An
airplane handed a Chase state just pops it. Aircraft.md §4.2.2 gives the exact 192/32 distance gates,
opacity gates and teleport placement. `cSC4Automaton::GetChaseVector` (Mac `0x000bc4e6`) is the other
reader of `+0x28`.

## 7. Calls other specs use *[compat]*

### 7.1 Controller manager (city slot 47)

| Slot | Call | Behaviour |
|---|---|---|
| 5 | `AddControllerFromTemplateName(occupant, name, extra)` (`0x00514CA0`) | Look up the template by name (script slot 17). None: return null. Otherwise return slot 6 `AddControllerFromTemplate(occupant, template, extra)`. |
| 9 | `AddControllersFromOccupantGroup(groupID, occupant, extra)` (`0x00517190`) | Look up the occupant group by **ID** (script slot 11). No script system or no group: return false. Otherwise add one controller per template in the group's list (slot 6 each) and return true. |
| 10 | `AddControllersFromOccupantGroup(name, occupant, extra)` (`0x00517150`) | The same, with the group looked up by **name** (script slot 12). |
| 11 / 12 | `RemoveControllersFromOccupantGroup` by ID / by name (`0x0051AB40` / `0x0051AB00`) | Look the group up (script slot 11 / 12); none: return false. Otherwise remove the occupant's controllers whose **template is in the group's template list** (body `0x0051A350`, below). |
| 14 | `RemoveControllers(occupant)` (`0x0051A520`) | Remove all controllers of the occupant. |
| 24 | `GetControllerTime` (`0x0051A310`) | Returns a pointer to `time` **A**, the current controller time (§3.1). |

**The `extra` argument of slots 5, 6, 9 and 10 (AM6).** It is a `const vec3*` **position**, which may be null. Slot
6 (`AddControllerFromTemplate`, `0x0051AD20`) creates the controller and adds it to the occupant's list. Then:
- with a non-null occupant, it attaches the controller to the occupant (controller slot 14, `+0x38`,
  `0x0051B091`), and `extra` is **not used**;
- with a null occupant and a non-null `extra`, it places the controller at that position (controller slot 17,
  `+0x44`, `0x0051B117`);
- either way it then sets the template (slot 15), `Init`s the controller (slot 3) and adds it to the controller
  grid.

Slots 5, 9 and 10 pass `extra` through to slot 6 unchanged. Every caller in §7.1 passes an occupant and 0, so
ignoring `extra` is correct for them. A replacement that accepts a null occupant must place the controller at
`*extra`.

**What slots 11 and 12 remove (AM7).** The original does **not** record which controllers a group created. The
body (`0x0051A350`; Mac `RemoveControllersFromOccupantGroup(template*, occupant*)`) works like this:
1. Find the occupant's controller list. None: return false.
2. Walk it. A controller is removed when its template (controller slot 8, `+0x20`, `0x0051A3C6`) is one of the
   group's templates (pointer comparison). So a controller from the same template that another path added, for
   example slot 5 by name, is removed too. A controller that has lost its template is not.
3. Each removed controller:
   1. the manager **sends** `0xEA298D50` (`kSC4MessageRemoveAutomataController`, `0x0051A413`) naming it;
   2. it leaves the controller grid;
   3. it is shut down (slot 4) and released.
4. If the occupant's list is now empty:
   - its queue entry (§3.3) is cleared and erased, releasing the occupant's reference;
   - its list entry is erased, releasing the occupant again.
5. Return true if anything was removed.

A null template (used internally by slot 14) matches every controller. The cores14a template-match reading is
therefore the original's rule, and it works after a load because nothing is tracked.

Callers: the fire occupant (slot 10, `fire_occupant`, §8.2), the civic ape escape (slot 10, `ape_escape`), the
police jailbreak (slot 10 to begin, slot 12 to end), the strike states of the fire, police and residential
simulators (slot 10, then slot 12), riots (slots 5 and 14, §8.1).

### 7.2 Pedestrian manager (city slot 44) slot 15: `CreateAutomaton`

Seven arguments, `__thiscall`, callee pops 28. The pedestrian, vehicle and aircraft managers share `0x00505370`;
the watercraft manager overrides the slot (below). Q5 is answered: the names and types are the Mac build's
signature, and the Windows code matches.

| # | Argument | Use |
|---|---|---|
| 1 | `u32` automata group ID | selects the exemplar and the class through the script system's group |
| 2 | `const vec3&` position | `SetPosition` (slot 8) after creation |
| 3 | `u32` automata flags | set on the new automaton with slot 30 `SetAutomataFlag(flags, true)` (`0x005107EF`). Bit 2 = self-controlling; bit `0x80` lets the creation through when the manager is full (below) |
| 4 | model-key reference | when its instance is 0, the model comes from an exemplar chosen for the group (manager slot 25); otherwise that key is used. Many callers pass an all-zero key; construction passes an explicit key (§7.6, including its Windows memory order) |
| 5 | `cIGZMessageTarget2*` | callback target, added with slot 39 `AddCallback` when non-null |
| 6 | `u8` priority | stored with slot 64. `0xFF` forces the creation |
| 7 | `const vec3*` orientation | slot 14 `SetOrientation` when non-null |

The call creates the automaton (manager slot 46, `CreateAutomatonInternal` `0x00510690`). It then applies arguments
5, 2 and 7 in that order, adds the automaton to the manager (slot 7) and releases its own reference. The returned
pointer therefore holds **no reference for the caller**: a caller that keeps it must `AddRef`, as the riot does
(§8.1 step 4).

`CreateAutomatonInternal` refuses (returns null) in two cases:
1. Priority 0, while the manager is inactive (slot 6 false).
2. The manager holds more automata than its **max count** (count `+0x48` > max `+0x3C`, a strict test, so the
   creation that reaches the max still passes), the make-room call (slot 47, `0x0050A740`) fails, flag `0x80` is
   clear and the priority is below `0xFF`.

**Make room** (slot 47, `0x0050A740`, AM4) gets the new automaton's priority p.
- It fails at once when the count has reached the **hard limit** (count `+0x48` ≥ `+0x40`, `0x0050A747`), or when
  p is 0.
- Otherwise it walks the list from the front (the oldest; `AddAutomaton` appends at the back). It takes the
  **first** automaton whose priority (automaton slot 63) is below p and asks it to remove itself: automaton slot 13
  `Remove(false)`, `0x0050A79A`. That call sets flags 1 (marked for removal) and `0x10000000` and starts a fade-out
  (Mac `cSC4Automaton::Remove`); it does not erase the automaton from the list at once. Then make-room succeeds.
- When no automaton has a lower priority, it fails.

So, above the max count:
- a creation succeeds only when it can push out a lower-priority automaton and the hard limit is not reached;
- at or above the hard limit, only flag `0x80` or priority `0xFF` gets through, and nothing bounds those two.

The cores14a reading ("null at the hard limit") is right only for the hard limit. Between the max and the hard
limit, creation depends on the eviction, and the two bypasses ignore the hard limit.

Any priority below `0xFF` is first offered to the city situation manager:
- if its slot 19 accepts the group,
- and its slot 28 count is below its slot 27 limit,
- then the priority becomes `0xFE` (a situation automaton, §9).

| Caller | Arguments |
|---|---|
| riot | (`0x4107`, cell position, 2, zero key, null, `0xFF`, null) |
| generators | (current group, spawn point, `0x2000002`, zero key, null, the template's priority byte, null) |
| building construction crew | (`0x4101`, terrain-height spawn point, `0x32000`, explicit construction model key, null, `0x32`, null); then clear flag 2 and set orientation (§7.6) |

**Watercraft override** (`0x00568F30`, also pops 28). It returns null without creating anything when either check
fails:
1. **Spacing.** For each watercraft in the manager's list (`+0x44`), it takes the distance from that boat's
   position (automaton slot 7) to the requested position, as a full 3-D length. If the distance is below 2.0
   (`f32` at `0x00B0A158`) × that boat's radius (slot 17), the request is refused (`0x00568C10`). The first boat
   that is too close stops the walk. Nothing is refused for an empty list.
2. **Bounds.** The position's x and z are turned into a cell (city slot 95, `0x00568F74`), and the cell must be
   inside the city (city slot 99, `0x00568F8C`).

When both checks pass, it calls the shared `0x00505370` with arguments 1–6 unchanged and **null** as the
orientation (`0x00568FAA`). A watercraft's orientation argument is therefore always ignored. The override does not
look at water, the flags or the priority itself; those are handled by the shared code as above.

### 7.3 Manager slots that other specs use

- slot 5 `Activate(bool)`, slot 6 `IsActive`;
- slot 9 `Pause`, slot 10 `Resume`, slot 11 `IsPaused` (city-situation.md);
- slot 13 `IsDrivingMode`: true when the current view's ID is `0x2B709676` (the U-Drive-It view; advisors.md);
- slot 23 `SetMaxCount(n)` (`0x0050AA60`, AM4):
  1. Clamp n to 0–5000.
  2. If n equals the current max (`+0x3C`), do **nothing**; the hard limit is not recomputed either.
  3. Otherwise, walk the list from the **front** (oldest first) while n < count (`+0x48`). Skip an automaton that
     has flag `0x200000` (slot 31) or priority `0xFF` (slot 63, `0x0050AAB7`). Every other automaton goes to the
     manager's internal *unplace* hook (manager slot 62, `+0xF8`, `0x00507430`; Mac `RemoveAutomatonInternal`):
     - if the automaton is in the renderer (status bit 2 of byte `+0x33`), it is taken out (automaton `+0x11C`,
       Mac `PlaceInRenderer(false)`);
     - if it is in the occupant manager (bit 1), it is taken out of that too (automaton `+0x118`, Mac
       `PlaceInOccupantManager(false)`).
  4. Set the max to n and the hard limit to `trunc(n × 1.25)`: `fild`, then `fmul` by the `double` 1.25 at
     `0x00A955A0`, then the CRT truncation `0x009EEF04`.

  The hook neither erases the automaton from the list nor lowers the count *[quirk]*, so the walk's condition
  never changes. When the count is above n, **every** eligible automaton in the list is unplaced, not only the
  excess. The automata stay in the manager's list and in the save. A replacement that does not animate needs only
  the clamp, the "unchanged n does nothing" rule and the two stored values. It must not drop automata from the
  saved list *[compat]*.
- slot 31 `TrackAutomataGroupInstances(u32 group, bool track)` (`0x00513C40`, AM12). This is **not** a counter. It
  keeps an unsaved map from automata group ID to a **set of automata**, each held with a reference. Neither the map
  nor the sets are in the record; §4 field 6 is a different map.
  1. If slot 30 `CanCreateGroup(group)` (`0x0050C300`: the group is a key of the manager's map from automata group to exemplar list) is false, return
     true and do nothing.
  2. With `track` true and no set yet for the group:
     - create an empty set;
     - put every automaton now in the manager's list that belongs to the group into it (occupant slot 22, `+0x58`,
       with the group ID), adding a reference to each;
     - return true. If the map insert fails, return false.

     When a set already exists, nothing changes.
  3. With `track` false: if a set exists, release every automaton in it (slot 33 `ReleaseAutomatonSet`,
     `0x0050CDE0`), erase the entry and return true.

  While a group is tracked, `AddAutomaton` adds each new member to the set (with a reference). Removing an
  automaton takes it out of every set that holds it and releases it, and so does the occupant-group-changed path
  (Mac `AddAutomaton`, `RemoveAutomaton`, `HandleOccupantGroupChanged`).
- slot 32 `GetInstancesOfGroup(group, set&)` (`0x00510040`) fills the caller's set. The members come from the
  group's tracked set when there is one, otherwise from a scan of the list. Each one belongs to the group, does not
  have flag 1 (marked for removal) and is not in the set already. It is added with a reference. The call returns
  true when the caller's set is not empty.
- slot 36 `GetSourceBuildingCount(occupant group)` and slot 37 `GetSourceBuildingCountForAutomataGroup` read the
  §4 field 6 counts.

### 7.4 Automaton slots that other specs use

slot 5 `AsOccupant`; slot 9 `SetVisible(bool)`; slot 13 `Remove` (dispatch despawn); slot 29
`ReturnToSourceOccupant` (dispatch "go home"); slot 51 `SetState(type, …6 more)`; slot 2 `Release`.

### 7.5 The three `vec3` fields and their setters (Q2, completed) *[compat]*

An automaton carries **three** `vec3` fields, stored consecutively and set one field at a time from a `const
vec3*` (a pointer to three `f32` in x, y, z order). All three are **city coordinates in metres**; there is no
unit conversion anywhere in the setters. The copy order is always x, then y, then z (three separate `f32`
stores).

| Field | Object offsets (x, y, z) | Slot | Setter | Getter |
|---|---|---|---|---|
| position | `+0x40`, `+0x44`, `+0x48` | 8 | `SetPosition` `0x00507D90` | slot 7 `GetPosition` `0x00505520` |
| destination | `+0x4C`, `+0x50`, `+0x54` | 10 | `SetDestination` base `0x00505560`, traffic `0x0054E5B0` | slot 11 `GetDestination` `0x004ADB30` |
| orientation | `+0x58`, `+0x5C`, `+0x60` | 14 | `SetOrientation` `0x0050AF10` | slot 15 `GetOrientation` `0x00505540` |

Write (`0x0050AC00`) serialises them in that order (§6.1 field 2). The meanings are the Mac accessors'
(`GetPosition`, `GetDestination`, `GetOrientation`); every value must round-trip unchanged *[compat]*.

**`SetPosition` (`0x00507D90`)**, `__thiscall`, one stack argument, `ret 4`:

1. Read the byte at **`+0x33`**. If **both** bit 0 and bit 1 are set, do not store anything: call **slot 16
   `MoveTo(position, 0)`** — i.e. `MoveTo` with the new position and a null orientation — and return.
2. Otherwise copy the three floats into `+0x40`, `+0x44`, `+0x48`.
3. Then walk the array of model-instance pointers from `+0x110` to `+0x114` and call each element's **slot 1**
   with `&this->position`. This is how a moved automaton repositions its render models; a replacement may
   animate differently, but the position store is the observable part.

**`SetOrientation` (`0x0050AF10`)**, same guard and same signature:

1. The same `+0x33` test; when both bits are set, call **slot 16 `MoveTo(0, orientation)`** and return.
2. Otherwise copy the three floats into `+0x58`, `+0x5C`, `+0x60`.
3. **Normalise in place**: replace the vector by `v × (1.0 / |v|)` (constant `1.0` at `0x00B09978`). The stored
   orientation is therefore always a **unit vector**, and a zero input stays zero (the divide is not guarded).
4. Then call `PositionModelInstance` (`0x005094A0`) for every entry of the same `+0x110`…`+0x114` array.

**`SetDestination` (slot 10)** has **two** implementations, and only one of them has side effects:

- **Base, `0x00505560`** — a plain store. Copy the three floats into `+0x4C`, `+0x50`, `+0x54` and return
  (`ret 4`). Nothing else happens: no flag, no turn, no `MoveTo`. This is what aircraft and the classes whose
  vtable is not in the traffic family use (vtables `0x00A94608`, `0x00A94B38`, `0x00A95AC0`, `0x00A97188`,
  `0x00A991E0`, `0x00A9DBB8`).
- **Traffic override, `0x0054E5B0`** — the pedestrian, vehicle, emergency, patrol, trailer and watercraft-family
  behaviour (every other vtable that shares the automaton getters). This is the one with the extra steps below.

The traffic override:

1. `SetAutomataFlag(4, 0)` (slot 30): **clears the "reached destination" flag** (§6.1 field 4, bit 4).
2. Copy the three floats into `+0x4C`, `+0x50`, `+0x54`.
3. Let `d = |destination − position|` (the Euclidean length of the per-axis differences).
4. If `d > 0.0` (compare against `0.0` at `0x00A81054`): write the **unit vector** `(destination − position) /
   d` into the *orientation* fields `+0x58`…`+0x60` (constant `1.0` at `0x00B0A07C`), then call
   **`MoveTo(0, &orientation)`**. So setting a destination also turns the automaton to face it.
5. If `d ≤ 0.0` (the destination is on top of the automaton): `SetAutomataFlag(4, 1)` — mark it as having
   reached the destination instead, and do not touch the orientation.

`MoveTo` likewise has three bodies: the traffic one `0x00544530` (pedestrian family), the plain helper
`0x005080C0` used directly as slot 16 by most other classes, and `0x00567D80` on vtable `0x00A9DBB8`. A
replacement that only keeps the records needs none of them; one that animates should pick the base behaviour
(a store) and add the traffic behaviour for vehicles and pedestrians.

**`MoveTo` (slot 16, `0x00544530`; helper `0x005080C0`)** takes two optional `const vec3*`: a position and an
orientation. When the orientation is given it is copied into a local with **y forced to 0** — only x and z are
kept, so the facing is always horizontal *[quirk]*. When the position is null the local is not built and the
helper is called with a null orientation. The helper (`0x005080C0`) rejects non-finite coordinates (it tests the
exponent field of y and the magnitude of x and z against a tolerance) before doing the move.

**What the setters do *not* do:** none of them writes the destination, none of them sets or clears automata
flags except `SetDestination`'s flag 4, and none of them touches the state stack.

**The `+0x33` guard.** Both `SetPosition` and `SetOrientation` short-circuit into `MoveTo` when the byte at
`+0x33` has bits 0 **and** 1 set. Which two flags those are is **not settled** here *[inferred]*: it is a pair of
automaton state bits that mean "do not accept a direct position/orientation write; go through the movement
system instead". A breakpoint on `0x00507D93` while an automaton walks would name them. Until then a
replacement may treat the guard as "if both bits are set, delegate to `MoveTo`", which is exactly what the
original does either way.

**The controller-manager automaton set's two `vec3`s** (§3.3) are not set through these setters. They are the
queue point and the queue direction, written by the point search (`0x00516070`) and by the record reader
(`0x0051B520`), and they are already specified in §3.3: both are metres, the point is 1 m outside a lot edge,
the direction is a unit axis vector, and the entry's `vec3`s are written point-then-direction.

### 7.6 Native construction-site boundary (NM17) *[compat]*

The owner of the crew is a native **building construction occupant**. Its SetLot call sequence and a
starting-site worked case are in [network-manager.md §3.2.1](network-manager.md#321-construction-site-automata-boundary-nm17-compat).
This section owns the automata-side contract, including a factory boundary that is stricter than the
construction caller's use of the returned interface. Addresses below were checked in Windows instruction
bytes and the pedestrian's primary vtable `0x00A9A718`.

#### 7.6.1 Interfaces, arguments and ordered calls

The construction caller obtains `cISC4AutomataManager` through city slot 44 (`+0xB0`), and receives the
**primary `cISC4Automaton*` A** (IID `0xA9B40F05`) from manager slot 15 (`+0x3C`). Its ABI is `__thiscall`:
the interface pointer is `this`, pointers and scalar arguments each occupy one 4-byte stack slot, and
the callee removes **28 bytes**. Priority uses the low byte of its slot. A valid primary automaton
interface cannot be substituted with an occupant-interface pointer: the two have different slot tables.

Construction passes `(0x4101, &position, 0x32000, &modelKey, null, 0x32, null)`
(`0x005CE67B`–`0x005CE6B1`). Position is three `f32` values, x/y/z. For this **Windows in-memory argument**,
the model key is 12 bytes with the following order, independently of §1's record notation:

| Byte offset from model-key argument | Value |
|---|---|
| `+0x00` | group `0x29A5D1EC` |
| `+0x04` | type `0x49A593E7` |
| `+0x08` | chosen instance: ground `0x0210B000`, `0x0210C000`, `0x0210A000`; roof `0x02109000` or `0x0210D000` |

The model key, position and subsequent orientation are caller-local buffers. Copy their values during
the call; no ownership of their storage is transferred, and none can be retained as a pointer into the
construction caller's stack. Null callback and orientation are valid independent arguments.

When native `CreateAutomaton` `0x00505370` is used, it first calls manager slot 46 (`+0xB8`) with
`(group,flags,&modelKey,null,priority)`; that is a five-argument call with **20-byte** callee cleanup.
On a non-null result, its remaining order is:

| Order | Interface and slot (byte offset) | Arguments / effect |
|---|---|---|
| 1 | A slot 39 (`+0x9C`) AddCallback | the callback argument, **including null**; the native slot is a null-safe no-op for this crew |
| 2 | A slot 8 (`+0x20`) SetPosition | one pointer to the three-`f32` position; callee removes 4 bytes |
| 3 | A slot 14 (`+0x38`) SetOrientation | one vector pointer **only if** the creation orientation is non-null; construction's null skips this call |
| 4 | manager slot 7 (`+0x1C`) AddAutomaton | A; establishes manager-list ownership and calls A slot 3 Init |
| 5 | A slot 2 (`+0x08`) Release | balances the creation reference; return A as a **borrowed** interface |

Those calls are `0x0050538E`–`0x005053D8`. Native AddAutomaton takes its reference before Init and list
insertion (`0x0050F7A9`–`0x0050F7DA`, pedestrian entry `0x005469EF`); its remaining placement/visibility
behaviour still applies. On null creation none of these post-creation calls occurs.

After slot 15 returns non-null, the **construction caller**, without any QI or interface adjustment, calls:

| Order | A slot (byte offset) | Arguments | Callee cleanup |
|---|---|---|---|
| 1 | 30 (`+0x78`) SetAutomataFlag | mask 2, false; clear only that mask | 8 bytes, even though false is a bool |
| 2 | 14 (`+0x38`) SetOrientation | one pointer to `(direction.x,0,direction.z)`, three `f32` | 4 bytes |
| 3 | 1 (`+0x04`) AddRef | none; construction-owner reference | 0 bytes |

The horizontal direction comes from `RandomDirection2` (`0x0080B680`), whose two components the caller
puts into x/z and whose y it forces to zero (`0x005CE6C6`–`0x005CE71B`). SetOrientation obeys §7.5;
it is a vector setter, not a second creation method or a callback registration. The first call clears
bit 2 without clearing `0x32000` (native flag method `0x004FAD20`–`0x004FAD4F`).

Later scaffold movement uses A slot 7 (`+0x1C`) **GetPosition(outVec3*)**: it writes all three `f32`,
returns **that same output pointer in EAX**, and removes 4 argument bytes. It does not return a vector
by value, an internal-position pointer, or an x87 value. The scaffold caller ignores EAX and reads its
own output buffer. A slot 8 then receives one input-vector pointer with only y changed. AddRef/Release
have no stack arguments. These signatures matter even when the replacement only records workers and
does not animate them (`0x00505520`–`0x00505535`, `0x005CC3A9`–`0x005CC3C8`; continuation in §7.6.4).

#### 7.6.2 Native internal creation requires concrete embedded views

If **native manager slot 46** (`0x00510690`) creates the object, it asks GZCOM slot 3 (`+0x0C`) for the
chosen class and **IID `0xA9B40F05`**, keeping that acquired creation reference. The class comes from the
selected automata-group template, falling back to the manager's class. The ordinary pedestrian class
is `0x896E75AF` (`0x00546030`). An IID is the requested view, not that class ID.

After a non-null factory result A, native slot 46 performs these operations **before** §7.6.1's wrapper:

1. A slot 64 (`+0x100`) SetPriority with the effective priority, then A slot 30 (`+0x78`) with
   `(requested flags,true)` (`0x005107D3`–`0x005107EF`). The situation-priority adjustment in §7.2 still applies.
2. For a nonzero requested model instance, call **the concrete helper** AddModelResourceKey at
   `0x00510360` directly with A as its base and the model-key pointer. With a zero instance and a chosen
   model description, pass that description's key instead (`0x005107F2`–`0x0051080A`). This is not a vtable
   dispatch that an unrelated facade can satisfy by adding a similarly named method.
3. If a model description was resolved, use the embedded occupant view at **A + `0x04`**, slot 6
   (`+0x18`), to get its property holder; use the embedded exemplar-holder view at **A + `0x20`**, slot 3
   (`+0x0C`), to attach the description's exemplar; add the description's occupant-group variant as
   property `0xAA1DD396` on the property holder, slot 14 (`+0x38`, final argument false). If the description's
   marker requires it, set flag `0x100000`. If there is no model description but a group template exists,
   add that template's group variant to the occupant's property holder instead. These branches are
   `0x0051080F`–`0x00510899`. They obtain embedded views by address arithmetic, **without QI**.
4. A slot 95 (`+0x17C`) SetAutomataGroup with the selected template (possibly null), then A slot 33
   (`+0x84`) SetOpacity with `(0.0,0)` and then `(1.0,fadeDuration)` (`0x0051089C`–`0x005108C5`). Both opacity
   arguments occupy one 4-byte slot each; the first is `f32`. `fadeDuration` is the native `f32` tuning
   value at `0x00B21C84` (initially zero), not a fixed extra creation priority. Return the creation reference.

These are the concrete data dependencies crossed by that native setup, relative to **primary A**:

| Offset | Native boundary requirement |
|---|---|
| `+0x04` | embedded occupant vptr, not a pointer to a separate occupant object; slot 5 IsInitialized and slot 6 property-holder accessor must work on this adjusted view |
| `+0x0C` | native property-holder view reached by that accessor (`0x006C2044` adds 8 to the occupant view); returned borrowed, with no AddRef |
| `+0x20` | embedded exemplar-holder vptr; slot 3 takes one exemplar pointer, AddRefs a changed new value and Releases the old value (`0x00458E60`) |
| `+0xC0`, `+0xC4`, `+0xC8` | model-key vector begin, end, capacity-end pointers; initially null, then a valid native vector of **12-byte keys** with storage compatible with the native allocator/growth/destruction |
| `+0x11C`, `+0x120`, `+0x124` | first model key's group, type, instance words; when incoming instance is nonzero and `+0x124` is zero, the direct helper copies all three |
| `+0x128` | also receives that first nonzero model instance |

AddModelResourceKey reads/writes these key fields, appends a complete copied key to the vector, then
calls **embedded occupant slot 5** (`+0x14`) IsInitialized. If already initialized it also calls the native
AddModelInstance helper; a freshly constructed object has not reached manager AddAutomaton's Init yet
(`0x00510366`–`0x005103F2`, constructor `0x005141BB`–`0x00514208`). The first-key copy is conditional;
the vector append still occurs when it is not the first key.

Native QI confirms the view relationship: IID `0xA9B40F05` returns primary A; occupant IID `0x26BB767E`
returns A + `0x04`, and successful QI AddRefs the common identity (`0x00505D8D`–`0x00505DA6`,
`0x00505DC5`–`0x00505E09`). Primary AsOccupant slots 5/6 return A + `0x04` **without** acquiring a reference
(`0x005FD7E0`); construction's successful-creation sequence does not call those slots itself.

The table is the observed direct-setup footprint, not a complete object-size or constructor substitute.
Retaining any native concrete helper requires its actual constructed subobjects and containers, in
addition to the primary interface slots. A replacement implementing the entire manager creation path
can supply these behaviours itself; the construction caller alone reads no concrete fields of A. Its
uses of A in this boundary are virtual calls. Merely answering occupant QI on a separate object does
not satisfy native slot 46's fixed-offset accesses.

#### 7.6.3 Removal, ownership and discriminating cases

Each successful construction request stores one **extra AddRef** beyond the manager-list reference.
There is no ownership of the stack argument buffers and no extra construction reference on a null
return. In the simple case without tracked groups, callbacks or other owners: factory result = 1;
manager AddAutomaton = 2; wrapper Release = 1; construction AddRef = **2**. Reference-bearing optional
tracking sets retain their own independent references (§7.3).

Construction Shutdown fetches the pedestrian manager and, for every worker in creation order, calls
manager slot 8 (`+0x20`) with `(A,false)` when that manager exists, followed by **A Release once** regardless
of manager presence (`0x005D1007`–`0x005D1036`). It does not call A Shutdown or consume the manager's
ownership itself. Native manager `RemoveAutomaton(A,false)` is **deferred**: notify event `(2,0)` through
A slot 102 (`+0x198`), set the removal flag through slot 89 (`+0x164`, argument true, delegating to flag
mask 1), set opacity to `(0.0,fadeDuration)` through slot 33, then stop its effects. It does not erase
the base-list node or Release the list reference in this false branch (`0x0050CAE9`–`0x0050CB2A`).

The effects operation is another concrete dependency **if the native removal body is retained**: at
**A + `0x18C`**, it expects an embedded `cSC4ModelRegEffects` object, calls that subobject's slot 7
(`+0x1C`) with true, then slot 2 (`+0x08`) with false. That slot 2 is an effects operation, not A's
IUnknown Release. It is a constructed object with its own effect container, not an optional pointer
or a zero-filled area (`0x00514292`–`0x0051429E`, constructor `0x005E8670`). Later immediate removal or the
manager's normal bulk drain performs the manager-owned shutdown/release (§5.2.1); it may also have
happened before the construction owner drops its final reference.

Use the starting-site case in network-manager.md §3.2.1 to check these independent boundaries:

- **Returned interface:** all three workers receive flag-clear, orientation and AddRef in that order;
  the manager creation call removes 28 bytes, flag setter 8, vector setter 4, and AddRef 0. Local key
  words are `(0x29A5D1EC,0x49A593E7,instance)`; the group word never becomes a callable address.
- **Native factory continuation:** when native slot 46 is used, a first ground worker with instance
  `0x0210B000` has copied first-key words `(0x29A5D1EC,0x49A593E7,0x0210B000)`, mirror instance
  `0x0210B000`, one vector entry (end = begin + 12), and working embedded occupant/exemplar/property-holder
  views before AddAutomaton Init. Correct QI alone does not establish this result.
- **Scaffold and normal removal:** setting scaffold height to 9 calls GetPosition/SetPosition for the
  roof worker only. Removing the site requests `(A,false)` for all three workers, then drops one
  construction reference each. With a live native manager and no other owners, each still has its
  manager-list reference until the later immediate removal/drain. If that drain happened first, the
  construction reference instead keeps each worker valid until its own final Release.
- **Refused creation:** let the middle request return null; expect no setter, AddRef, stored list entry
  or later construction Release for that request. The other two workers and later dust setup still
  proceed. This is the caller's accepted null path, not a requirement to refuse valid requests.

NM17's log observations do not determine which replacement contract failed. The native model-group
constant `0x29A5D1EC` explains why that value is present among the creation arguments, but neither a
stack-imbalance diagnosis nor a factory-layout diagnosis follows from the reported crash address alone.
These rules enable an engine boundary test; they do not report a passing full-profile action run.

#### 7.6.4 Construction continuation and the position return (AT25) *[compat]*

**Position ABI.** Primary `cISC4Automaton` IID `0xA9B40F05`, slot 7, is a one-pointer `__thiscall` with
4-byte callee cleanup. The original copies the position's x/y/z words into caller-owned storage and
leaves the **address of that storage in EAX** (`0x00505523`–`0x00505535`). There is no allocation,
AddRef, ownership transfer, or floating-point-stack result. Preserve this register result when
implementing the interface; the ABI table's `eax-or-void` does not settle it. It is not permission to
return the worker's position field or to retain the output pointer.

**The construction consumer.** SetScaffoldHeight walks the saved crew in creation order and skips
entries whose roof marker is false. For every roof worker it performs exactly this sequence:

| Order | Receiver / slot | Operation and lifetime |
|---|---|---|
| 1 | saved primary A, slot 7 (`+0x1C`) | pass a writable local three-`f32` buffer; copy the worker's current position into it |
| 2 | construction caller | overwrite only the buffer's y word with the supplied scaffold height, already `f32` |
| 3 | the **same saved A**, slot 8 (`+0x20`) | pass that buffer as input; copy its position during the call; 4-byte callee cleanup |

The caller reloads A from its list, rather than using the getter's EAX as a receiver. It overwrites EAX
immediately after GetPosition returns (`0x005CC3B3`–`0x005CC3C8`); it consumes **neither** an EAX vector
reference nor an x87 result. Each call borrows the existing construction reference: no QI, AddRef,
Release, AsOccupant, new worker, or worker Init/Shutdown occurs in this loop. Repeated identical heights
still produce the getter/setter pair. Ground workers receive neither call. The setter has §7.5's
semantics, including model-position updates where applicable; the local buffer expires with the call.

**When this happens.** The first pass is **inside SetLot, before it returns**, not on the first daily
tick. After CreateConstructionCrew, SetLot adds roof props, dust, sparks and the lot texture, records
the simulation day, initializes the phase/deadline and zero elapsed animation time, sets building
completion to zero, then calls SetScaffoldHeight with **`r24(baseY + 0.001)`**. Here `r24` means rounding
to `f32`; the addition uses the double constant at `0x00AA2270` before that store. This pass moves roof
workers from their terrain-height spawn to just above the construction base. Crane setup, construction
messages and renderer insertion follow (`0x005D09C5`–`0x005D09E6`, `0x005D0AFD`–`0x005D0BA8`).

Daily message `0x66956814` calls DoDailySim. It returns without a phase change when there is no building,
phase is 4, or current day is below the deadline. At a due deadline it resets elapsed animation time
and performs **one** transition, scheduling the next deadline from the current day; it does not loop
through missed days (`0x005D0686`–`0x005D06D8`). Let N be the site's scaffold-step count and k its
completed/entered step counter, initially zero. Durations below are the integer durations
already computed for the site by SetLot, not worker lifetimes:

| Current phase | Due-day transition | Effect on stored workers at that daily call |
|---|---|---|
| 0, initial rise; or 2, building work | increment k; if k ≤ N, enter 1 with next deadline now + lift duration; otherwise enter 3 with next deadline now + lowering duration | no worker call; entering 3 removes construction props, not the crew |
| 1, scaffold lift | enter 2 with next deadline now + work duration; scatter sparks | no worker call |
| 3, scaffold lowering | enter 4, set completion to 1, remove the construction occupant and finish the lot's construction transition | occupant teardown reaches §7.6.3's per-worker removal/Release |

The transition bodies are `0x005D06DF`–`0x005D07A2`, `0x005D07EB`–`0x005D081D` and
`0x005D0842`–`0x005D0872`. Calls on prop/model lists in those bodies are not additional slots on A.

Animation message `0x86AD10EE` supplies an **unsigned integer** time value through message slot 14
(`+0x38`); the caller divides it by 1000 with integer truncation and passes the quotient to
DoAnimationTick (`0x005D1267`–`0x005D127E`). The helper adds that integer to the site's elapsed counter,
converts the unsigned total to floating point, multiplies by the `f32` constant `0x3B03126F`
(approximately 0.002), and rounds the product to `f32` (`0x005CE116`–`0x005CE14A`). This animation
clock and the daily deadline are separate inputs; do not infer one animation call per day.

In phases **0, 1 and 3**, the helper computes the current rising/lifting/lowering height, rounds it to
`f32`, and calls SetScaffoldHeight, hence the same roof-only slot 7/8 pairs. In phase **2** it updates
building completion instead; in phase **4** it makes no scaffold call. The three height-call sites are
`0x005CE1A8`, `0x005CE228`, `0x005CE33F`; the phase-2 completion call is `0x005CE2B9`. These paths
do not recreate the crew. A separate model-rebuild message can also replay DoAnimationTick with zero
elapsed increment, temporarily treating phase 2 as phase 1 to restore scaffold height; it still uses
the same stored A (`0x005D1090`, model-rebuild branch).

**Boundary and failure discrimination.** For a manager supplying its own creation and removal bodies,
this continuation introduces **no concrete pedestrian/occupant/effects layout** and no native worker
callback. It requires the stable primary interface returned at creation, copied input vectors, a
working getter/setter, and ownership lasting across all these later calls and through the owner's final
Release. Do not substitute an occupant view (IID `0x26BB767E`) for A. Native creation/removal or other
retained native helpers still have their independently documented concrete dependencies (§§7.6.2–7.6.3).

The reported `SimCity 4.exe+0x1CC3B3` is **the dispatch into slot 7**, before GetPosition executes.
At that instruction ECX is saved A and EAX is the vtable word just loaded from A; EDX is the output-buffer
address. Thus reported EAX `0x43960000` makes the caller read `0x4396001C` to find slot 7. `0x43960000`
also encodes `f32` 300.0, but this alone does not identify a write, a stale object, or a bad interface
adjustment. It cannot be the return value of this not-yet-entered GetPosition call. Validate the stored
pointer, its live primary vtable and buffer writes across setup and ticks; changing only the getter's
EAX result cannot repair this dispatch. The other reported thread/crash addresses establish no further
worker slot or concrete-view requirement.

#### 7.6.5 Worked crew through the first simulated days (AT25)

Use NM17's detail/lot/random-draw fixture to obtain **two ground workers G1/G2 and one roof worker R**,
all successful. For R choose terrain spawn `(12.5,7,34.5)` and model instance `0x02109000`; ground
models may be `0x0210B000` and `0x0210C000`. Each request has §7.6.1's group, flags, priority and setter
sequence. With no optional owners, each worker has one manager reference and one construction reference
after setup. Record **the actual three primary pointers** for the later calls.

For the continuation fixture, supply a site with base y **7**, initial scaffold target **9**, N = **2**,
initial-rise duration **1 day**, lift duration **1 day**, work duration **2 days**, lowering duration
**1 day**, and scaffold step height **3** (the unoverridden value at `0x00B0A7C8`). These are explicit
fixture inputs/results of site setup; worker count and time of day alone do not determine them. The
one-day rise/lift/lowering values respect SetLot's signed **minimum with 1**, not a lower bound of 1
(`0x005D0A44`–`0x005D0ACE`). Disable independent worker movement
and unrelated rebuild messages for this trace. Take D0 as the setup day and deliver the listed events
in order. Here q is the elapsed integer animation counter, reset only by a due daily transition.

| Event | Construction phase / clock after event | Expected calls on crew / resulting R position |
|---|---|---|
| each successful creation | before the next request | that worker receives slot 30 `(2,false)`, slot 14 horizontal orientation, slot 1 AddRef, in order |
| finish D0 SetLot | phase 0, k = 0, q = 0, deadline D0 + 1 | R slot 7 then 8; position `(12.5,r24(7.001),34.5)`; G1/G2 skipped |
| animation increment 250 | phase 0, q = 250; scaled time rounds to 0.5 | R slot 7 then 8; `(12.5,8,34.5)` |
| another daily delivery on D0 | before deadline; phase 0, q unchanged | no worker call and no new worker |
| animation increment 250 | phase 0, q = 500; scaled time rounds to 1 | R slot 7 then 8; `(12.5,9,34.5)` |
| daily D0 + 1 | phase 1, k = 1, q = 0, deadline D0 + 2 | no worker call; the same references remain live |
| two animation increments of 250 | phase 1, q successively 250, 500 | two R slot 7/8 pairs; R y successively **10.5, 12**; x/z unchanged |
| daily D0 + 2, then animation increment 250 | phase 2, k = 1, q = 250, next deadline D0 + 4 | building completion changes; no R getter/setter; R stays at y = 12 |
| daily D0 + 3 | before deadline; phase 2, q unchanged | no worker call |
| daily D0 + 4, then two animation increments of 250 | phase 1, k = 2, q successively 250, 500, next deadline D0 + 5 | R slot 7 then 8 per increment; y successively **13.5, 15**; the original G1/G2/R pointers are still used |
| daily D0 + 5 | phase 2, k = 2, q = 0, next deadline D0 + 7 | no worker call |
| daily D0 + 7, then two animation increments of 250 | phase 3, k = 3, q successively 250, 500, next deadline D0 + 8 | R slot 7/8 per increment; y successively **11, 7**; crew remains owned |
| daily D0 + 8 | phase 4 and construction teardown | §7.6.3 removal request and owner Release for G1, G2, R, in that order |

The worked heights follow the phase-0 and phase-1 instruction order at `0x005CE15B`–`0x005CE228`;
the lift fraction is stored to `f32` before multiplication by the step height; lowering uses
`0x005CE2D8`–`0x005CE33F`. The initial SetLot height
is `f32` **`0x40E00831`** (about 7.00099993), not exactly decimal 7.001. A native-message test obtains
each increment 250 from slot-14 value 250000; passing 250 directly to the helper is a distinct test.
At every roof getter, verify all three output words, EAX equal to its supplied buffer address, unchanged
stack depth after 4-byte cleanup, and no x87 result; verify the setter copies the buffer and preserves
the saved A/vtable. Keep G1/G2 unchanged by scaffold passes. At site removal, expect one manager
`RemoveAutomaton(A,false)` followed by one construction Release per saved worker in creation order;
exercise the manager-list drain both before and after that final owner Release (§7.6.3). This is a boundary
oracle, not a claim that the reported generated-action acceptance has passed.

## 8. Riot and fire hooks *[compat]*

### 8.1 Riot leaders (disasters.md §6)

**Start** (`AddRiotLeader`, `0x0057DD00`), in this order:

1. find the nearest road tile; none → fail;
2. stop when the leader count has reached its cap;
3. `CreateAutomaton` on the pedestrian manager (city slot 44, slot 15) with group `0x4107` (§7.2); null → fail;
4. `AddRef` the automaton;
5. automaton slot 51 `SetState(8, 0, −1.0, 1, 0, 0, 0)`: the RiotLeader state — the `1` in the fourth argument
   is `final` (§6.2). The push is at `0x0057DD97`.
6. automaton slot 9 `SetVisible(true)`;
7. register the leader's location with the disaster layer (city slot 72).

**Stage rise** (`Update`, `0x0057DFE0`): format the template name `riot_leader_<stage>` (the format
`riot_leader_%d`, `0x00A9E860`, with the leader's stage) and call controller manager slot 5
`AddControllerFromTemplateName(leader as occupant, name, 0)`.

**End**, in this order:

1. automaton slot 51 `SetState(1, 0, −1.0, 0, 0, 0, 0)`;
2. controller manager slot 14 `RemoveControllers(leader as occupant)`;
3. disaster layer (city slot 72) slot 15: unregister the location by occupant;
4. release the automaton and erase the leader.

**Corrects (re-integrate1):** step 1 **does nothing**. The leader's RiotLeader state was pushed `final=1`
(step 5 of Start, `0x0057DD97`), and `SetState` returns 0 and changes nothing while the top state has `final`
(Mac `0x000bd3ca`, Windows `0x00509860`; §6.2). So the call is made but the leader keeps state 8. Steps 2–4
still run: the leader is `Release`d and unregistered immediately after. This section and conformance test 8
assert the call is made; **they do not say it succeeds**.

**Nothing pops the `final` RiotLeader (AT11 answered 2026-10-20, re-answers33).** The removal path is in
`cSC4DisasterInstanceRiot::Update` (`0x0057DFE0`), per leader in the list at `this+0x1C`: it accumulates
elapsed ticks (`node+0xC`) and derives the riot phase (`elapsed / this+0x50`); when the leader's occupant
answers `IsFlagSet(1)` (occupant slot 32) **true** — the leader has been flagged for removal — the end branch
runs: occupant slot `0xCC` = `SetState(1, 0, −1.0, 0, 0, 0, 0)` (the no-op above), the script system
(city slot 47) unregisters the leader's occupant group (slot `0x38`), the disaster layer (city slot 72)
unregisters the location by occupant (slot `0x3C`), the automaton's `Release` runs (slot 2), and the leader
node is unlinked from the list and freed. **The DISAPPEAR state never runs for a riot leader**: the automaton
is released while still in state 8 with `final` set, and nothing calls `PopState` on it. A replacement should
do the same (release without popping) — or pop first *[choice]*; both are invisible to the save.

### 8.2 Fire occupant (disasters.md §5)

`SetTarget` (`0x00471900`): when the target occupant's type is `0x278128A0` (a building), call controller
manager slot **10** `AddControllersFromOccupantGroup("fire_occupant", target, 0)`. `Shutdown` (`0x00470DC0`) calls
slot 14 `RemoveControllers(target)` and drops the target. (disasters.md §5.11 said slot 9; corrected, §13 Q1.)

## 9. U-Drive-It hooks

The U-Drive-It missions belong to the city situation manager (city-situation.md). The automata layer supplies:

- manager slot 9 `Pause` / slot 10 `Resume`: when a mission activates, the city situation manager pauses all four
  managers and resumes them when its start countdown ends (city-situation.md);
- manager slot 13 `IsDrivingMode` (§7.3);
- the player's automaton runs state `0x10` (PlayerDrive, §6.2), whose record adds one `f32`;
- `CreateAutomatonInternal` (`0x00510690`) asks the city situation manager (slots 19, 27, 28) whether a new
  automaton can start a mission, and the cheat handler (`0x0050DD30`) uses its slot 33.

None of this changes a save beyond the records above.

## 10. Messages *[compat]*

| Receiver | Messages handled |
|---|---|
| controller manager (`0x00519360`) | `0xA6B79602` view update, `0x86AD10EE` animation tick (drives `DoAnimationTick`), `0x09D92FC3` automaton notifications, `0x26D31EC1` PostCityInit (§3.4), `0x44778230` HourChanged, `0x89D3E9F3`, `0x99EF1142` InsertOccupant, `0x99EF1143` RemoveOccupant (drops the occupant's controllers [inferred]) |
| controller manager, cheats (`0x005187F0`) | `0x230E27AC` with `0xABA2B5BB`: a cheat command that toggles a debug flag |
| every manager (base, `0x00506970`) | `0x99EF1143`, `0x4A7FB807` emergency pause, `0x26D31EC2` PreCityShutdown, `0x06CDB65A`, `0x26D31EC1`, `0x2977AE2F` building completion changed, `0x86AD10EE`, `0x89D3E9F3`, `0x99EF1142`, `0xCAD61E00`, `0xA6B79602`, `0xA6B79612`, `0xC9F775BB`, `0xEA298D50`, `0xEAD61E16`, `0xEBFF18A3` |
| pedestrian manager (`0x00545450`) | adds `0x230E27AC` (cheats) and `0xA9E3B6E2` (crowd reaction), then the base |
| vehicle manager (`0x00562620`) | adds `0xA6B79611`, `0x0772FAD4` FundsChanged, `0x44778234` DayNightChanged, `0xA6B79601`, `0xC53D10AA` exemplar changed, `0xC9DA96E8` dawn and `0xC9DA96E9` dusk, then the base |
| aircraft manager (`0x004FC710`) | adds `0x99EF1142` / `0x99EF1143` (`HandleOccupantRemoved` `0x004FB8D0` keeps §5.2 consistent), then the base |
| watercraft manager (`0x00575500`) | adds `0x2C06BA82` ferry pass finished, `0x99EF1142` / `0x99EF1143` (§5.3 lists), then the base |

The occupant insert and remove handlers keep the saved lists of §3.2, §5.2 and §5.3 consistent: an occupant that
leaves the city takes its controllers, airport pairs and ferry entries with it. A replacement must do the same,
or it will write refs to objects that are no longer saved.

## 11. Interface slot tables

`cISC4AutomataControllerManager` (vtable `0x00A96058`) is already in `docs/abi/interfaces.tsv`. The proposed rows
for the three below are in `build-runs/re-specs13a-abi-rows.tsv` of this lane.

**`cISC4AutomataScriptSystem`** (vtable `0x00A96BA8`): 0 QueryInterface, 1 AddRef, 2 Release, 3 Init, 4 Shutdown,
5 GetAutomataGroupMap, 6 GetAutomataGroup (2 args), 7 GetAutomataGroup (1 arg), 8 GetOccupantGroupNameMap,
9 GetOccupantGroupIDMap, 10 GetSpecialFilterOccupantGroups, 11 GetOccupantGroup(ID) `0x0051CFF0`,
12 GetOccupantGroup(name) `0x0051CFC0`, 13 CreateOccupantGroup, 14 GetDriveCommand, 15 GetControllerTemplateMap,
16 CreateControllerTemplate, 17 GetControllerTemplate(name) `0x0051CF90`, 18 GetTrackBuildingGroups.

**`cISC4AutomataManager`** (pedestrian `0x00A9A470`, vehicle `0x00A9D1E0`, aircraft `0x00A94850`, watercraft
`0x00A9DF38`): 0 QueryInterface, 1 AddRef, 2 Release, 3 Init, 4 Shutdown, 5 Activate, 6 IsActive, 7 AddAutomaton,
8 RemoveAutomaton, 9 Pause, 10 Resume, 11 IsPaused, 12 GetAutomataCount, 13 IsDrivingMode, 14 GetMinZoom,
15 CreateAutomaton (7 args), 16 GetRandomCreationPoint (7 args), 17–18 Dispatch, 19–20 TranslateDispatchLocation,
21–22 ChaseAutomaton, 23 SetMaxCount, 24 HandleTuningParametersChanged, 25 ChooseRandomExemplar,
26 ChooseRandomExemplarByFilter, 27 ChooseExemplarByCrimeType, 28 GetExemplarData, 29 GetExemplarDataMap,
30 CanCreateGroup, 31 TrackAutomataGroupInstances, 32 GetInstancesOfGroup, 33 ReleaseAutomatonSet,
34 SetCameraFollow, 35 IsCameraFollowUpdating, 36 GetSourceBuildingCount, 37 GetSourceBuildingCountForAutomataGroup.
The serializable views are at `0x00A9A448`, `0x00A9D1B4`, `0x00A94828` and `0x00A9DF10`.

**`cISC4Automaton`** (pedestrian primary vtable `0x00A9A718`), slots 0–69: 0–4 QueryInterface, AddRef, Release,
Init, Shutdown; 5–6 AsOccupant; 7 GetPosition; 8 SetPosition; 9 SetVisible; 10 SetDestination; 11 GetDestination;
12 GetTransform; 13 Remove; 14 SetOrientation; 15 GetOrientation; 16 MoveTo; 17 GetRadius; 18 GetDisabled;
19–20 PlayAnim; 21 PlayAnimByName; 22 AttachAnim; 23–28 call number, dispatch state and dispatch info ID (get/set
pairs); 29 ReturnToSourceOccupant; 30 SetAutomataFlag; 31 CheckAutomataFlag; 32–33 opacity; 34–38 speeds;
39–40 Add/RemoveCallback; 41–47 effects; 48 GetLeader; 49 SetLeader; 50 FindDrivingAutomaton; 51 SetState;
52 PushState; 53 PopState; 54 SetBehavior; 55 PushBehavior; 56 GetState; 57 FindState; 58 PrevState;
59 GetBehavior; 60 HandlePlayerDriveCommand; 61–62 Start/StopSFX; 63–64 priority; 65 HandleCollision;
66 GetCollidePosition; 67 GetAutomataGroup; 68–69 Register/UnregisterInGameDataRegistry.

**Slots past 69, from the vtable bytes (`AT17`/`AT19` answered 2026-10-20, re-answers33).** The primary vtable
continues; the slots the tick paths call are:

| Slot | `+offset` | Windows | Name | Evidence |
|---|---|---|---|---|
| 70 | `+0x118` | `0x00507E60` | *(not traced here)* | |
| 77 | `+0x134` | `0x0050C330` | **`EnableActiveSound(bool)`** — named by the export (`auto:A/high`). Reads/writes the `+0x1DC` byte and, when enabling, fills the active-sound list at `+0x1C8` from exemplar property `0x6C044BB5`. The earlier *[inferred]* name `EnableMotion` is **wrong** | Mac symbol `cSC4Automaton::EnableActiveSound` `0x000bfba6` |
| 78 | `+0x138` | `0x0050A130` | `UpdateActiveSound` (named by the export) | |
| 79 | `+0x13C` | `0x00505820` | flag reset used by the emergency guard: `SetAutomataFlag(4,1)` + `(0x20,1)` + … | `FUN_0052F210` tail-jumps into it |
| 80 | `+0x140` | `0x00505860` | wrapper: tail-calls slot 102 with `(6, 0)` | 4-byte body `JMP [vtable+0x198]` after pushing 6 and 0 |
| 82 | `+0x148` | `0x004FB340` | **`GetFirstModelInstance`** *[inferred]* — returns `*(this+0x110)` when the model-instance vector `+0x110..+0x114` is non-empty, else null. The earlier *[inferred]* name `SetTransform` is **wrong**: `cSC4Automaton::SetTransform` (Mac `0x000c1c68`) is a different body that applies a transform to every instance | body is 22 bytes on `+0x110` |
| 105 | `+0x1A4` | `0x005096B0` | the **behaviour refresh** (`AT13` answered; see §6.5) — the emergency family overrides it (`cSC4*::vftable_2` slot 146, `0x0052F730`: skip when flag `0x80` is set or the top state is `0x34`) | `cSC4Pedestrian::vftable_2` slot 146 = `0x005096B0` |
| 102 | `+0x198` | `0x0050B540` | **`NotifyAutomatonEvent(u32 type, u32 value)`** *[inferred]* — posts message `0x9D92FC3` to every callback target in `this+0x188` with `this` as sender and the two arguments as payload (Mac slot 109 = Windows + 7) | the crash enter passes `(0xB, duration)`, `0x0056E40B`; `FUN_0052F210` passes `(5, 0)` |
| 120 | `+0x1E0` | `0x00508D70` | `DoCollideTick` (already in §6.5) | |
| 125 | `+0x1F4` | `0x00505AB0` | `GetSpeedModifier` (already spec-named; returns 1.0 or the emergency-vehicle clamp) | §6.5 blocked wait |
| 160 | `+0x280` | `0x0054DA50` | **`ReturnToDefaultSpeed`** (AT19 answered): reads the max speed (slot 125? no — slot `+0x1F4` result), computes the tween duration `1.0 / maxSpeed × 0.3` (or `0.1` when the max speed is 0), then `TweenSpeed(defaultSpeed, duration)` — slot `+0x8C` = 35 supplies the default speed and slot `+0x90` = 36 is `TweenSpeed`. Matches the Mac `ReturnToDefaultSpeed` `0x000caa42` (`(defaultSpeed, 0.3/m)`) | `0x00A81228 = 1.0`, `0x00A97D74 = 0.3` |
| 163 | `+0x28C` | `0x005500D0` | `DoStopSignTick` (§6.5) | |

The Windows/Mac slot offset stays +7 above slot 79 as §6.5 records.

Overload pairs (script 6/7 and 11/12, manager 17–22, automaton 5/6 and 19/20) are in the Windows order; the
argument types given above for script slots 11 and 12 are checked, the others are not.

## 12. Quirks *[quirk]*

1. **Flag-8 controllers are never saved**, and refs to them from automata and states are written as null (§3.2,
   §6.1, §6.2).
2. **Duplicate occupant keys in the controller manager record corrupt memory.** When Read meets an occupant it
   has already seen, it appends to a stale list pointer. The original never writes duplicates. A replacement must
   skip the repeated entry's controllers (read them, then drop them) *[choice]*.
3. **An emergency vehicle cannot be reloaded** (corrected by AM8). Its class ID is the vehicle's (`0x896E714A`,
   the shared `GetGZCLSID` `0x00532240`), so the loader builds a plain vehicle and runs the vehicle Read over the
   emergency record (§6.4). Every nested reader then meets the `u16` one level up, and the checks fail at the
   fourth:

   | Reader | Reads | Check | Result |
   |---|---|---|---|
   | vehicle | the emergency's `u16` 2 | needs 2 | passes |
   | traffic automaton | the vehicle's `u16` 2 | needs 2 | passes |
   | automaton base | the traffic automaton's `u16` 2 | ≤ 8 | passes |
   | occupant base | the automaton base's `u16` 8 | needs 4 (`0x005E8D75`) | **fails** |

   So that object's Read fails. It does not succeed with a tail left unread, as this quirk said before. The §6.4
   layout is the correct one: the Write produces it. The corpus has none, because emergency vehicles are not in the
   managers' saved lists [inferred]. A replacement must not save emergency vehicles *[choice]*. It should decode
   such a record by the §6.4 layout only in tests.
4. **The manager base Read does not report stream errors**, but the wrappers of the current versions do (§4.1).
5. **Discarded fields.** The traffic manager's `u32` (§5.1 field 3), the vehicle's path entries and constant vector
   (§6.4) and the controller manager's version are read and thrown away.
6. **Pointer-order maps.** The aircraft manager's pairs (§5.2) are written in pointer order, so two saves of the
   same city may differ in order.
7. **The controller manager's loaded flag gates `PostCityInit`.** A city that was not loaded (a new city) skips
   the template re-binding.
8. **`SetMaxCount` unplaces instead of removing** (§7.3). Lowering the max below the count takes every eligible
   automaton out of the renderer and the occupant manager. None leaves the list, and an unchanged n does nothing at
   all.
9. **A trailer only keeps vehicle refs** (§6.4). Its refs are checked against the vehicle's class ID used as an
   IID, so any other automaton reads as null and is dropped.
10. **`+0x0C` is a stack index, not a semantic field** (§6.2, re-integrate1; **wording corrected by A1**,
    re-answers31). `PushState` computes it as `stack.size()-1` after the push (Mac `0x000bf7d0`) and `PrevState`
    uses it as an index (Mac `0x000bbee4`, Windows `0x00509900`). It round-trips. A replacement must **recompute
    it after any stack mutation** (`PushState` / `PopState`) and must **store the streamed value unchanged on
    Read** — `tAutomataState::Read` (`0x00505E50`) reads it into `state+0x0C` and does not derive it, and
    `Write` (`0x00509D00`) emits it, so re-deriving on Read breaks the byte-exact re-save. "Rather than trust
    the stream" in the earlier wording applied to *stack mutations*, not to Read; test 29 as originally written
    read the other way and was wrong.
11. **`+0x14` is `final`, and it has teeth** (§6.2, re-integrate1). `SetState` returns 0 and changes nothing
    while the top state has it (Mac `0x000bd3ca`, Windows `0x00509860`). RiotLeader (`0x0057DD97`) and Collide
    (Mac `0x000a091e`) are both pushed `final=1`. `PopState` on a `final` state that is not already type 1 does
    not reveal the state below — it pushes DISAPPEAR instead (Mac `0x000bd2de`, Windows `0x0050B660`). So a
    Collide with its 5000 ms timeout ends as DISAPPEAR → `Remove(0)`, and the riot end's `SetState(1, …)` is a
    no-op (§8.1).
12. **Plummet's five `f32` are not appended in offset order** (§6.2, re-integrate1). The stream is
    `+0x28, +0x34, +0x30, +0x2C, +0x38` (Mac `0x000c3e36` / `0x000c47f4`), and the object has a **sixth** slot
    `+0x3C` which is a `ref` zeroed on Read, not an `f32`. §6.2's "5 × `f32`" is right about the saved part and
    misses the sixth. A round trip must use that order.
13. **Bumped has a record and a class but no tick and no reader** (§6.2, re-integrate1; **AT7 answered**,
    re-answers33 2026-10-20). Base `DoBumpedTick` is `return !CheckAutomataFlag(1)` (Mac `0x000bb14e`) and
    `cSC4Vehicle::DoBumpedTick` is `return 0` (Mac `0x000a083e`). The 4 × `vec3` at `+0x28/+0x34/+0x40/+0x4C`
    have **no reader at all**: on the Mac side `tAutomataStateBumped` is referenced only by its own ctor
    (`0x00638236`, zeroes them), `Read` (`0x000c4608`, fills them from the stream) and `Write`
    (`0x000c3c62`), plus the factory case `0x11` (`0x000c8f5a`). No code pushes BUMPED (nothing passes
    type `0x11` to `SetState`/`PushState`), and `cSC4Vehicle::HandleCollision` (Mac `0x000a091e`,
    Windows `0x0055C8C0`) writes the *orientation* `+0x58..+0x60` and pushes **COLLIDE**, not Bumped.
    The four vectors are a **leftover bump-response record**: a replacement must round-trip them from a
    save (they carry whatever the save holds) but no code path ever reads them.
14. **AttachAnim's `bool` sits before its `u16` in memory** (§6.2, re-integrate1). Stream `ref, u16, bool`;
    object `+0x28, +0x30, +0x2C` (Mac `0x000c3bba` / `0x000c453a`).
15. **Chase is a pure record** (§6.2, re-integrate1). No `0x34` case exists in any `DoStateTick`; the `ref` is
    read by `GetChaseVector` (Mac `0x000bc4e6`) and the helicopter's `AI_PursueGoal` (§6.6).
16. **Type 0 `DEFAULT` is not "nothing"** (§6.5, re-integrate1) — it is the generic seek-to-destination in
    `DoAnimationTick`, which is exactly what a state returns 0 *for*.
17. **`dt` is milliseconds but tween durations are seconds** (§6.5, re-integrate1). Mixing them up is the
    easiest way to get a 1000× wrong speed or timeout.
18. **Windows and Mac disagree on offsets and on high vtable slots** (§6.5, re-integrate1). Above `~+0x1A0` the
    Windows struct runs `0x10`–`0x18` lower than the Mac; from vtable slot 79 upward Mac slot = Windows slot + 7.
    Quote both, or state which one you mean.

## 13. Open questions

- **Q1 (settled here).** disasters.md §5.11 step 4 named controller manager slot 9 for `fire_occupant`. The exe
  calls slot 10 (`+0x28`, at `0x00471A48`), the by-name variant; disasters.md is corrected in the same commit.
- ~~**Q2.** The meaning of the two `vec3`s and the `bool` of a controller-manager automaton set (§3.3).~~
  **ANSWERED 2026-10-16 (re-answers16; QUESTIONS.md AT2).** Each set is an occupant's **queue**:
  - the automata in line, front first;
  - the queue point, 1 m outside the middle of the lot edge that touches a network;
  - the unit direction in which the line grows;
  - "point found".

  State 4 (`QUEUE`) drives it (`0x0051BB60`–`0x0051BD50`, point search `0x00516070`). The leader field of §6.1 links
  each automaton to the one ahead. §3.3.
  **Completed 2026-10-16 (re-answers29):** the automaton's **own** three `vec3` fields and what their setters do
  are now specified in **§7.5** — which offsets each setter writes (position `+0x40`, destination `+0x4C`,
  orientation `+0x58`, always x then y then z, always metres), that `SetOrientation` normalises to a unit vector,
  that `SetDestination` clears automata flag 4, turns the automaton to face the destination and calls `MoveTo`,
  and that both `SetPosition` and `SetOrientation` short-circuit into `MoveTo` when the byte at `+0x33` has bits 0
  and 1 set. The one thing still open there is which two flags `+0x33` holds.
- ~~**Q3.** The type numbers of the RandomWalk, PlopJump, Plummet, PullOver, Bumped and Collide states, and the
  meaning of corpus state types 0, 2, 3, 7 and `0x0D` (all use the default record).~~ **ANSWERED 2026-10-16
  (re-answers16; AT3).**
  - The classed states are RandomWalk `0x0B`, Plummet `0x0E`, PlopJump `0x0F`, Bumped `0x11`, Collide `0x14` and
    PullOver `0x15`. The Windows factory sizes match the Mac classes, and the game's Lua `automata_states` table
    agrees.
  - The default-record corpus types are 0 `DEFAULT`, 2 `BEE_LINE`, 3 `CROWD` and 7 `IDLE`.
  - 13 (`0x0D`) is the traffic automaton's blocked wait (`0x0054E322`).

  §6.2 has the full table.
  **Completed for this spec's scope (re-answers29, 2026-10-16).** The game's own `automata_states` table was read
  from the shipped `sit_constants.lua` and matches §6.2's number-to-name table exactly, so the numbers an
  implementer keys off are now confirmed against the original data rather than inferred from the factory sizes.
  What each state *does on a tick* is deliberately **not** in this short form (§0: animation is out of scope and
  a replacement may animate however it likes). An implementer that only loads, re-saves and services the riot,
  fire and U-Drive-It hooks needs nothing more than §6.2's type → record-shape mapping plus state 4 (`QUEUE`,
  §3.3) and state 13 (the blocked wait, §6.2). An **animation** lane would still want per-type enter/tick/exit
  behaviour; that is a separate task.
- ~~**Q4.** The records of aircraft and watercraft automata (helicopter, plane, boat). None occur in the corpus,
  and their Write and Read functions are not mapped here.~~ **ANSWERED 2026-10-16 (re-answers16; AT4):**
  - aircraft `0xC96E75DC`: `u16` 1 + base;
  - airplane `0xCBEB4F62` and helicopter `0x8BA98EDA`: `u16` 1 + aircraft record + `vec3` home position;
  - watercraft `0x096E75F5`: `u16` 2 + base.

  §6.4.
  **Note (re-answers29).** The record *layouts* above come from the executable alone and stand without a sample.
  What a save containing planes or boats would still add is **corpus validation**: a real aircraft/watercraft
  automaton record to parse to its exact end, and a real manager record with a non-empty pair list to pin the
  write order of §5.2's pointer-ordered map. The 421-save corpus has none, and no such save was found under
  `Documents\SimCity 4\` in this session. To produce one: build an airport (or a marina and a ferry) in a city,
  let it spawn aircraft/watercraft, and save. Until then, treat §6.4 and §5.2 as executable-only
  *[compat]* facts and keep the round-trip tests of §14 item 15 driven by synthetic records.
- ~~**Q5.** The meaning of the generator's two trailing `u32`s, the automaton base's unnamed fields, and
  `CreateAutomaton`'s mode and last three arguments.~~ **ANSWERED 2026-10-16 (re-answers16; AT5).**
  - The generator's `u32`s are the current automata group ID and the number spawned from it (§3.5).
  - The automaton base fields are named in §6.1: position, destination, orientation, radius, speed, flags, dispatch
    info ID, attractor, its distance and strength, attraction vector, leader, model instance, the speed and opacity
    tweens, priority and group.
  - `CreateAutomaton`'s arguments 3–7 are automata flags, model key, callback target, priority and orientation
    (§7.2).
  **Completed 2026-10-16 (re-answers29):** the **controller** and **state** records' remaining unnamed fields are
  now named from use.
  - The controller base's object offsets and names are in **§3.5**: the template name object (`+0x24`, characters
    `+0x28`), the position, the age counter at `+0x50` (incremented by 1 per animation tick), the unsaved lifetime
    limit at `+0x54` (`SetLifetime`, copied from the template at `Init`; `IsExpired` = limit > 0 and reached), the
    automata `std::list` (head `+0x40`, size `+0x44`) and the flag dword at `+0x3C`. Two record `u32`s (`+0x38`
    and the saved float at `+0x4C`) stay unnamed: both are 0 in the whole corpus.
  - The state base record's offsets and stream types are exact (**§6.2**): `+0x0C` `u32`, `+0x10` `f32`, `+0x14`
    `bool`, `+0x18` `u32`, `+0x1C` `u32`, `+0x20` `u32` (version 3 only), and `+0x24` is **not** in the record —
    Read zeroes it. Write always emits version 3 and all seven fields.
  **Still open:** what the state base fields and the two leftover controller fields *mean* (the state fields are
  named from use by re-answers29's state-behaviour pass, below), and the flag-offset contradiction of §3.5
  (`CheckFlag`/`SetFlag` test `+0x40`, the list head, while the record's flag dword is `+0x3C`).
- ~~**Q6.** Messages `0x89D3E9F3`, `0x06CDB65A`, `0xCAD61E00`, `0xEA298D50`, `0xEAD61E16`, `0xEBFF18A3`,
  `0xA6B79611`, `0xC9DA96E7` are handled but not named in other specs.~~ **Answered (re-answers12).** The names
  come from the executable's own message-name table (pairs of ID and name around `0x00B08360`–`0x00B086F8`):

  | ID | Name | Posted by |
  |---|---|---|
  | `0x89D3E9F3` | `kSC4MessageNetworkOccupantUsableChange` | the network manager (network-manager.md §5) |
  | `0x06CDB65A` | `kSC4MessagePreViewFilterChange` | `cSC4View3DWin::SetCurrentViewFilterInfo` (`0x007E6435`) |
  | `0xCAD61E00` / `0xEAD61E16` | `kSC4MessageMouseScrollBegin` / `…End` | the UI |
  | `0xEA298D50` | `kSC4MessageRemoveAutomataController` | the controller manager's `RemoveController` (`0x00517924`, `0x0051A610`), `DoAnimationTick` (`0x00518D3B`) and `RemoveControllersFromOccupantGroup` (`0x0051A414`) |
  | `0xEBFF18A3` | `kSC4MessageOccupantGroupChanged` | `cSC4Occupant::AddOccupantGroup` (`0x005E93EA`) |
  | `0xA6B79611` | `kSC4MessageViewTChange2` | the camera control (`0x007CC16B` and three more) |

  `0xC9DA96E7` is **not a message the vehicle manager handles**: it occurs once in the executable, as the lower
  bound of a range test in the vehicle manager's `DoMessage` (`0x00562763`: `E7 < id ≤ E9`), so the handled IDs
  are `0xC9DA96E8` dawn and `0xC9DA96E9` dusk (subscribed through message-server slot 5 at `0x0055B82B` / `0x0055B838`). Dusk calls slot 176
  (`+0x2C0`) with `(true, true)` on every vehicle of the list at `+0x38`, dawn with `(false, true)`
  (`0x00562788`–`0x005627C8`) *[inferred: headlights]*; then the base handler runs. The §10 table is corrected.

- **AM1–AM12 (cores14a, answered 2026-10-16 by re-answers20; QUESTIONS.md "From the automata layer core").**
  In brief:
  - AM1: writers always write the current version (§4.1);
  - AM2: the object Reads require exact versions (§4.1, §6.4);
  - AM3: the cursor holds a layer ID, and a new manager writes 1 (§5.1);
  - AM4: max-count refusal and eviction, and `SetMaxCount` unplaces front to back without removing (§7.2, §7.3);
  - AM5: the controller times (§3.1);
  - AM6: `extra` is a position (§7.1);
  - AM7: removal matches templates (§7.1);
  - AM8: the §6.4 layout is right, and quirk 3 is corrected (§12);
  - AM9: the watercraft manager fails above 6 (§5.3);
  - AM10: the trailer's IID is `0x896E714A` (§6.4);
  - AM11: the stream-error rules (§4.1);
  - AM12: slot 31 tracks sets of automata, and §4 field 6 is the source building counts (§4, §7.3).

New from the re-integrate1 reports (the probe named is the one the source gives) — **all fourteen answered
2026-10-20 by re-answers33 (QUESTIONS.md AT6–AT19)**; each answer's rule is written into the section the
entry names:

- ~~**Q7 (AT6).** What is PlayerDrive's saved `f32` at `+0x28`?~~ **ANSWERED 2026-10-20 (re-answers33;
  AT6). DIFFERS** from the guess: it is not untouched. `cSC4Vehicle::TranslatePlayerDriveCommands`
  (Mac `0x000a336c`) integrates the throttle/brake keys into it — `+0x28 += dt_s × params+0x10/0x14`
  (the accel constant), sign by forward/reverse, zeroed when both drive channels are inactive — and
  `cSC4Vehicle::PlayerDriveUpdateSpeed` (Mac `0x000a3e1a`) integrates it into the speed `+0x2C`
  (`vel += dt_s × +0x28`, clamped to the params' max/min). It is a **drive command accumulator**
  in speed-per-second units. §6.2, §6.5.
- ~~**Q8 (AT7).** What reads Bumped's 4 × `vec3`?~~ **ANSWERED 2026-10-20 (re-answers33; AT7). MATCHES**
  the "leftover record" guess: **nothing reads them**. On the Mac side `tAutomataStateBumped` is referenced
  only by its ctor (zeroes), `Read` (fills from the stream), `Write` and the factory case `0x11`; no code
  pushes type `0x11`, and `HandleCollision` pushes COLLIDE. §6.2, quirk 13.
- ~~**Q9 (AT9).** PlopJump's units.~~ **ANSWERED 2026-10-20 (re-answers33; AT9). MATCHES** the ms reading,
  with the missing unit name: the `U(30, 50)` draw is in **km/h** (`× 1/3600` converts km/h → m/ms), so the
  vertical velocity is 0.0083–0.0139 m/ms and the gravity `−5.5555563e−05` (lazy `0x00B22740`, seeded at
  `0x00505606`) is m/ms². A hop peaks at 0.62–1.7 m over 150–250 ms. §6.2.
- ~~**Q10 (AT11).** What pops the `final` RiotLeader?~~ **ANSWERED 2026-10-20 (re-answers33; AT11). DIFFERS**
  from the implied "something pops it": **nothing does**. `cSC4DisasterInstanceRiot::Update` (`0x0057DFE0`)
  ends a leader when its occupant reports `IsFlagSet(1)` true: the no-op `SetState(1,…)`, the script system
  and disaster-layer unregisters, `Release`, unlink. The automaton is released while still in state 8 with
  `final` set; DISAPPEAR never runs. §8.1.
- ~~**Q11 (AT10).** `random_walk.turn`'s exact template offsets.~~ **ANSWERED 2026-10-20 (re-answers33;
  AT10). DIFFERS** — there is **no off-by-4**. `ImportBehaviorTable` (`0x00522860`) writes the array/scalar
  into `+0x58` (amplitude) and `+0x5C` (bias) — exactly what `DoRandomWalkTick` reads (`FLD [template+0x58]`
  `0x0050D139`, `FLD [template+0x5C]` `0x0050D187`). The `+0x54/+0x58` reading mis-simulated the stack
  frame; the ctor defaults (`+0x58 = 0.1`, `+0x5C = 0.2`, not 0.01/0.2 — the spec's default row is corrected)
  pin the layout, and Ghidra's decompile of the importer shows the turn pair as the locals the decompiler
  lays out at exactly those offsets in the copied 0x7C-byte template. §6.2.
- ~~**Q12 (AT12).** The mapping of emergency-vehicle states 100–103.~~ **ANSWERED 2026-10-20
  (re-answers33; AT12). MATCHES** the slot mapping, with the naming evidence now cited from the exe: the
  dispatch-state names are the shipped debug strings (`0x00532290`, `0x00A98514`–`0x00A98574`), and the
  state types are tied to them by the pairings `SetDispatchState(3)+SetState(100)`, `NotifyCallbacks(5,0)`
  + `SetDispatchState(5)` + `SetState(0x66)`, `SetDispatchState(1)+PushState(0x65)`,
  `SetDispatchState(6)+PushState(0x67)` (`0x0052F210`, `0x0052F330`, `0x0052FF60`). §6.5.
- ~~**Q13 (AT13).** The exact contract of `FUN_005096B0`.~~ **ANSWERED 2026-10-20 (re-answers33; AT13).**
  It is **automaton slot 105** (the emergency family overrides it at class-vtable slot 146 with
  `0x0052F730`, which skips it when flag `0x80` is set or the top state is `0x34`). It looks up the
  behaviour template for the automaton's current occupant group (storing it at `this+0x98`), then walks the
  top of the state stack: **`state+0x24`** is compared against the found template (equal ⇒ no push);
  **`state+0x08`**'s controller is compared by group (null or mismatch ⇒ the state is popped); otherwise
  `PushBehavior` (slot 55) pushes the found template. New subsection in §6.5.
- ~~**Q14 (AT14).** The fate of a boat that finishes `CRASH_RUNAGROUND`/`CRASH_SINK`.~~ **ANSWERED
  2026-10-20 (re-answers33; AT14). DIFFERS** — nothing reaps it. No watercraft manager cleanup touches
  states 70/71 (all 41 watercraft functions scanned); the beached or sunk boat sits in its final state
  until flag-1 removal, `HandleOccupantRemoved` (`0x00574520`), or a new `SetState`. §6.6.
- ~~**Q15 (AT15).** Is the idle budget's never being re-zeroed intentional?~~ **ANSWERED 2026-10-20
  (re-answers33; AT15). MATCHES** the lifetime reading: `Init` (`0x0056DE40`, `MOV [ESI+0x2BC],0` at
  `0x0056E2A0`) zeroes it and `DoStateTick` case 7 adds to it, so 60 s **total** in `IDLE` over the boat's
  life slow-removes it. §6.6. *Corrected 2026-10-21 (re-boatsped):* `DoAnimationTick` (`0x00572D14`) is a
  third writer when `+0x2CC` is set (watercraft.md §5.1); it does not reset it either.
- ~~**Q16 (AT16).** Is `Crash_SetStateInternal`'s slot-59 read a "don't re-crash" guard?~~ **ANSWERED
  2026-10-20 (re-answers33; AT16). DIFFERS**: the slot-59 read (`GetTopStateType`, `vtable+0xEC`) is the
  **player-drive test** — the crash states are pushed only when the top state is `0x10` PLAYER_DRIVE; any
  other boat gets `SLOW_REMOVE`. The `== 0x10` comparison is present in the bytes of `0x00568570`; the
  older decompile dropped it. §6.6.
- ~~**Q17 (AT17).** Does the helicopter `AI_PursueGoal` `default:` fall into the `0x32` body?~~
  **ANSWERED 2026-10-20 (re-answers33; AT17). DIFFERS** — no fall-through. The dispatch (byte table
  `0x0053C5A0`, jump table `0x0053C57C`) sends types 1–`0x31` and anything above `0x38` to their own
  default block at `0x0053C437`, which is `PopState` only; `0x32` has its own block at `0x0053C441` that
  also calls slot 25 with 0 before popping. §6.6.
- ~~**Q18 (AT18).** Semantics of vtable slot 102/109, and the names of slots 77 and 82.~~ **ANSWERED
  2026-10-20 (re-answers33; AT18). DIFFERS on two of three names**: slot 102 (Windows `0x0050B540`) posts
  message `0x9D92FC3` to the `this+0x188` callback list with `this` as sender and the two arguments as
  payload — *[inferred]* `NotifyAutomatonEvent(u32, u32)`; the crash call passes `(0xB, duration)`, so the
  "effect/anim kill" reading was wrong (it is a notification, and the second argument is the crash
  duration in `HandleEnterState`). Slot 77 is **`EnableActiveSound(bool)`** (named by the export, Mac
  `0x000bfba6`) — not `EnableMotion`. Slot 82 (`0x004FB340`) is **`GetFirstModelInstance`**
  *[inferred]* — not `SetTransform` (Mac `0x000c1c68` is a different body). New slot table in §11.
- ~~**Q19 (AT19 part 1).** What the blocked-wait exit city query means.~~ **ANSWERED 2026-10-20
  (re-answers33; AT19). DIFFERS** from both guesses: the query is **`cSC4City::GetTrafficNetworkMap`**
  (city slot 40) then **`cSC4TrafficNetworkMap::CheckIntersection`** (`cISC4TrafficNetworkMap` slot 33,
  `0x004B06F0`), which returns the **count of automata occupying the automaton's approach lane** (and
  writes the wait delay to the out `f32`; `FLT_MAX` when nothing was found). Returning 0 means the lane is
  **free** — the wait pops. §6.5.
- ~~**Q20 (AT20 → the second half of the same question).** Is `this+0x94` the measured speed, and is
  Windows slot `0xA0` `ReturnToDefaultSpeed`?~~ **ANSWERED 2026-10-20 (re-answers33; AT19).** `+0x94` is
  written as `|Δpos|/dt` **only by `cSC4Vehicle::DoPathTick`** (`0x0055CFBC`) — not by the traffic
  version — and zeroed by the pull-over tick (`0x0055E95C`); no tick reads it (`GetSpeed`, slot 34,
  reads the speed tween, not `+0x94`); the only consumer found is `cSC4Train::HandleCollision`, which
  copies the train's value onto its cars *[inferred: a derived-speed debug field]*. Windows slot `0xA0`
  (`+0x280`, `0x0054DA50`) **is `ReturnToDefaultSpeed`**: duration `1.0/maxSpeed × 0.3` (0.1 when the max
  is 0), then `TweenSpeed(defaultSpeed, duration)` — matching the Mac `0x000caa42`. §6.5, §11.
- **A4 (settled here).** The city's Init fault at `SimCity 4.exe+0x0045BF1B` with the six automata GZCOM classes
  registered. **ANSWERED 2026-10-16 (re-answers31; QUESTIONS.md A4).** The null is the component at
  `cSC4City + 0x1E4`, the script system; `QueryInterface` answers the **CLSID** `0x4A1B664E` but not the **IID**
  `0x8A1B6647` the city creates it with, so the out parameter is left null and the city calls `Init` on it
  without a null check. The same IID gap is on all six classes. Full chain, addresses and the change to make:
  **§15**.
- **A1 (settled here).** Whether `+0x0C` (the state stack index) must be recomputed **on Read**.
  **ANSWERED 2026-10-16 (re-answers31; QUESTIONS.md A1).** **DIFFERS** from test 29 as written: the stream value
  is authoritative and must be stored as read, or a byte-exact re-save breaks. Recompute only after a *stack
  mutation* (`PushState` / `PopState`), which is what §12 quirk 10 already says. See the correction to §14
  test 29.

## 14. Conformance tests (must exist in `engine/conformance`)

Each test reads the record, checks the decoded fields, writes it back and compares bytes.

1. **Script system.** `01 00` → version 1. A record `02 00` must fail to load.
2. **Controller manager, empty** (30 bytes, New City (11)):
   `01 00 | 18 93 01 0A 29 F9 E7 A7 | 00 00 00 00 00 00 | 00 00 00 00 00 00 | 00 00 00 00 | 00 00 00 00`
   → version 1; clock ref ID `0x0A019318`, class `0xA7E7F929`; times (0.0 h, day 0) twice; no controllers; no sets.
3. **Controller manager times** (Getting Started Tutorial): both times are bytes `77 77 5F 41 03 00` = (13.9667 h,
   day 3).
4. **Vehicle manager, empty** (22 bytes): `02 00 | 04 00 | 50 01 00 00 | 00 | 00 | 00 00 00 00 | 01 00 00 00 |
   01 00 00 00` → version 2; base version 4, max 336, inactive, flag 0, no automata, no group map (version < 5);
   `u32` 1; cursor 1. The **pedestrian** record is the same with max `80 01 00 00` (384) and `u32` 2.
5. **Attractor** (82 bytes, Konradshohe): `05 00 | 2C 21 89 17 2D 88 BD A9 | 0E 00 00 00 "police_attract" |
   12 × 00 (position) | 8 × 00 | 00 00 00 00 | 07 00 00 00 | 00 00 00 00 | 01 00 | 02 00 00 00 | 00 00 00 00
   00 00 70 42 | 00 00 48 42 00 00 70 42` → base version 5, building `0x1789212C`, flags 7, no automata; curve
   ((0, 60), (50, 60)).
6. **Generator** (76 bytes, Tegel): `04 00 | 05 00 | 54 6E BD 09 47 AA 77 29 | 08 00 00 00 "dog_make" | 20 × 00 |
   00 00 00 00 | 87 00 00 00 | 00 00 00 00 | 00 00 20 41 00 00 | AB AA 2A 3E 00 00 | 22 41 00 00 | 00 00 00 00`
   → occupant ID `0x09BD6E54` (a prop, class `0x2977AA47`), flags `0x87`, times (10.0, 0) and (0.16667, 0), `u32`s 16674 and 0.
7. **Watercraft, version 6** (126 bytes, Konradshohe):
   `06 00 | 06 00 14 00 00 00 01 00 00 00 00 00 0C 00 00 00 05 13 00 00 00 00 00 00 06 13 00 00 00 00 00 00 07 13
   00 00 00 00 00 00 08 13 00 00 00 00 00 00 09 13 00 00 00 00 00 00 39 19 00 00 00 00 00 00 41 19 00 00 00 00
   00 00 10 31 01 00 00 00 00 00 20 31 01 00 01 00 00 00 30 31 01 00 02 00 00 00 0A 15 00 00 00 00 00 00 0F 15
   00 00 00 00 00 00 | 00 00 00 00 | 00 00 00 00 | 00 00 00 00`
   → base version 6, max 20, active, flag 0, no automata, 12 group counts: `0x1305`–`0x1309`, `0x1939`, `0x1941`,
   `0x13110` with 0, `0x13120` with 1, `0x13130` with 2, `0x150A` and `0x150F` with 0; then three empty lists.
8. **Riot order.** With a fake pedestrian manager and controller manager, `AddRiotLeader` must call, in order,
   `CreateAutomaton(0x4107, …)`, `SetState(8, …)`, `SetVisible(true)`; a stage rise to 2 must call
   `AddControllerFromTemplateName(leader, "riot_leader_2", 0)`; the end must call `SetState(1, …)` before
   `RemoveControllers(leader)`.
9. **Fire.** `SetTarget` on a building calls controller manager slot 10 with `"fire_occupant"`; on a non-building
   it calls nothing; `Shutdown` calls slot 14.
10. **Flag 8.** A controller manager holding one controller with flag 8 and one without writes n1 = 1, m = 1, and
    an automaton whose controller has flag 8 writes a null controller ref.
11. **Unknown template.** Loading a controller whose template name the scripts do not define drops it at
    `PostCityInit` and keeps the rest.
12. **Corpus round trip.** Every component 40, 41, 43–46 record and every referenced object record in the corpus
    parses to its exact end.
13. **Queue entry** (Big City Tutorial): an automaton set with occupant ref, k = 0, point (1256, 0, 1041),
    direction (−1, 0, 0) and `true` round-trips.
    - The point search for a one-cell lot at cells (78, 64), facing 2, gives side 3.
    - If the traffic network map has a tile at cell (78, 65), that is point (1256, 0, 1041) and direction (−1, 0, 0).
    - With no tile on any side, the flag stays false after four tries.
14. **Queue order.** Automata A, B and C enter state 4 at the same occupant in that order. C's leader is B, and B's
    is A. When B leaves, C's leader becomes A. When A leaves, C has no leader.
15. **Aircraft records.** An airplane writes `01 00 | 01 00 | <automaton base> | <vec3>`, and a watercraft writes
    `02 00 | <automaton base>`. Reading an aircraft base with version 2, or a watercraft with version 1, fails.
16. **Watercraft spacing.** A watercraft manager holds one boat of radius 10 at (100, 0, 100).
    - Creating a boat at (115, 0, 100) returns null, because 15 < 2.0 × 10.
    - At (120, 0, 100) the distance equals 20, which is not below 20, so the request goes on to the bounds check
      and the shared creation.
    - At (100, 0, 100) with an orientation, the new boat's orientation is not set from the argument.
17. **Version checks** (§4.1). Each of these fails to read: a pedestrian with `u16` 4 or 6; a vehicle, emergency
    vehicle, patrol car or trailer with `u16` 1 or 3; a traffic manager whose incident list starts with `u16` 2; a
    watercraft manager with `u16` 7. A watercraft manager with `u16` 5 reads as a version-5 base-only record.
18. **Re-save** (§4.1). Reading the 22-byte version-2 vehicle manager of test 4 into a live manager and saving it
    writes `03 00`, a version-6 base (with the group map the live manager holds: `00 00 00 00` when the scripts
    track no building groups), the `u32` 1, the cursor 1, then `01 00 00 00 00 00` (an empty incident list).
19. **Layer cursor** (§5.1). A new vehicle manager writes cursor 1 and a new pedestrian manager writes 1. A vehicle
    manager record with cursor 5 re-saves 5. One with cursor 11 (no such layer) re-saves 0.
20. **Controller times** (§3.1). Start from A = B = (23.5, day 6). `HourChanged` gives A = (24.5, 6), unnormalised,
    and B unchanged; the controllers receive delta (1.0, 0). A tick at clock 0:00 then gives A = (0.0, 0), because
    the day becomes 7 and 7 mod 7 is 0, and B = A.
21. **Trailer IID** (§6.4). A trailer record whose vehicle list holds a ref to a pedestrian reads that entry as null
    and drops it. A ref to a vehicle or a patrol car is kept.
22. **SetMaxCount** (§7.3). A manager with max 10 holds 5 automata, the third with priority `0xFF`. `SetMaxCount(2)` keeps all
    5 in the list (count 5). It unplaces the other four and sets max 2 and hard limit 2. A second `SetMaxCount(2)`
    calls nothing.
23. **Group tracking** (§7.3). `TrackAutomataGroupInstances(g, true)` with two members of g in the list makes
    `GetInstancesOfGroup(g)` return both. A third member added later is in the set as well. `(g, false)` releases
    the set. Neither call changes the saved record.
24. **Plummet stream order** (§6.2, quirk 12). A Plummet record with the five `f32` set to distinct values
    writes them in the order `+0x28, +0x34, +0x30, +0x2C, +0x38`, **not** offset order, and writes nothing for
    `+0x3C`. Reading it back leaves `+0x3C` zeroed and each `f32` at its own offset.
25. **AttachAnim layout** (§6.2, quirk 14). An AttachAnim record writes `ref`, `u16`, `bool` in that stream
    order while the object holds the `bool` at `+0x2C` and the `u16` at `+0x30`. Round-trips byte-exact.
26. **`final` blocks `SetState`** (§8.1, quirk 11). Push a RiotLeader with `SetState(8, 0, −1.0, **1**, 0, 0, 0)`,
    then call `SetState(1, 0, −1.0, 0, 0, 0, 0)`. `SetState` returns 0, the top state type stays 8, and nothing
    on the stack changes. This is the riot end's step 1.
27. **`PopState` on a `final` state** (quirk 11). Push Collide with `final=1` and a 5000 ms timeout over a
    DEFAULT state, then let the countdown expire. The DEFAULT state is **not** revealed: `PopState` pushes
    DISAPPEAR instead, and the next tick of DISAPPEAR calls `Remove(0)`.
28. **Returned 0 means the generic seek** (§6.5). A state type with no `DoStateTick` case (0, 2, 3, 4, …) makes
    `DoStateTick` return 0, and `DoAnimationTick` then seeks `destination` at `GetCurrentSpeed()`, setting flag
    4 on arrival. A type that returns 1 (e.g. RANDOM_WALK) does not seek.
29. **Stack index: stored as read, recomputed on stack mutation** (quirk 10; **corrected by A1**,
    re-answers31 2026-10-16). Push three states, pop one, and check that every remaining state's `+0x0C` equals
    its 0-based position from the bottom — that recomputation is the job of `PushState` / `PopState` only.
    **A record read from a stream must keep the `+0x0C` the stream gives it, unchanged.** Do **not** recompute
    it on Read: `tAutomataState::Read` (`0x00505E50`) reads the `u32` straight into `state+0x0C`
    (`lea eax,[edi+0x0C]` / `call [stream+0x24]` at `0x505E99`–`0x505E9F`) and never derives it, while
    `tAutomataState::Write` (`0x00509D00`) emits it. Recomputing on Read therefore changes a saved byte and
    breaks the byte-exact re-save the corpus requires (it shifts byte 174 of a 342-byte pedestrian record).
30. **`dt` is ms, tween durations are seconds** (quirk 17). `TweenSpeed(target, 1.0)` with `dt` of 500 ms
    reaches the target after two ticks; `state+0x10 = 1000.0` expires after one 1000 ms tick.
31. **Chase is record-only** (§6.2, quirk 15). `ChaseAutomaton` (manager slot 22) pushes `0x34` and stores the
    target in `state+0x28`. No `DoStateTick` handles `0x34`, so `DoStateTick` returns 0 every frame; only
    `GetChaseVector` and the helicopter's `AI_PursueGoal` read the ref.
32. **The blocked wait's exit** (§6.2 type 13, §6.5). While `state+0x10 > 0` and flag `0x20` is clear the tick
    returns 0 and does nothing. With `0x20` set it decrements by `GetSpeedModifier()·dt`. On expiry it clears
    `0x20` and `0x10`, and `PopState`s only when the network map reports the way clear.

## 15. GZCOM creation contract (city Init) *[compat]*

**Answer to A4** (glue19 automata lane: the city's Init phase faults at `SimCity 4.exe+0x0045BF1B` with
`ACCESS_VIOLATION reading 0`, `eax=1 ecx=0 edx=0x3000 ebx=0x00A80810`, after every component reports `Read -> 1`).
Verdict: **DIFFERS** — the missing piece is not a component record and not a list sentinel. It is the **IID the
city asks for at create time**. See the change to make at the end of this section.

### 15.1 The faulting instruction

`0x0045BF1B` is inside `FUN_0045bd20` (`0x45bd20`–`0x45c196`), the city's one-shot initialiser. The instruction is

```
0x45BF1B   8B 11      MOV  EDX, dword ptr [ECX]     ; fetch the object's vtable
0x45BF1D   FF 52 0C   CALL dword ptr [EDX+0x0C]     ; vtable slot 3 = Init
```

`ECX = 0` is the object pointer, so the vtable fetch reads address 0. The pointer is loaded one instruction
earlier:

```
0x45BF15   8B 8E E4 01 00 00   MOV ECX, dword ptr [ESI+0x1E4]
```

`ESI` is the `cSC4City` `this` (the prologue keeps it there; `0x45bebb` has `8B CE` `MOV ECX, ESI`). So the null
is the **component pointer at `cSC4City + 0x1E4`**, and the city calls its `Init` (slot 3) with **no null check**.

The call order in `FUN_0045bd20` around the fault is `+0x1F4`, `+0x1F0`, `+0x1FC` (the effects manager,
`cSC4EffectsManager`, class `0x49822F75`, `re/class_id_table.tsv:396`), then **`+0x1E4`**, then `+0x1E0`. The four
automata managers (`+0x1D0`, `+0x1D4`, `+0x1D8`, `+0x1DC`) are initialised much later in the same loop, which is
why the fault lands on the script system first.

### 15.2 What lives at `cSC4City + 0x1E4`

`cSC4City::field138_0x1e4` is the **automata script system** (component 40, §2). `re/class_id_table.tsv:361`
gives `0x4A1B664E -> cSC4AutomataScriptSystem`. The city creates it in its ensure-components helper:

```c
if (this->field138_0x1e4 == (int *)0x0) {
  (**(code **)(*piVar1 + 0xc))(0x4a1b664e, 0x8a1b6647, &this->field138_0x1e4);   // decomp/cSC4City.c:2144-2145
}
```

That is `CreateObject(clsid = 0x4A1B664E, iid = 0x8A1B6647, &out)`: **class** `cSC4AutomataScriptSystem`,
**interface** `cISC4AutomataScriptSystem`. Note the asymmetry: the create helper *does* null-check before
creating (`2144`) and the destroy helper *does* null-check before releasing (`2300`–`2303`), but the Init
dispatcher (`FUN_0045bd20`) and the other raw slot-3 call at `decomp/cSC4City.c:2722` do not.

The same pattern creates the other five, each with its own **IID** — note that for the four managers the IID is
shared and is *not* any of their CLSIDs:

| City field | Class (CLSID) | IID asked for | Interface |
|---|---|---|---|
| `+0x1D0` | `0xA98F9D3B` vehicle | `0x098F9BF4` | `cISC4AutomataManager` |
| `+0x1D4` | `0xC98F9D17` pedestrian | `0x098F9BF4` | `cISC4AutomataManager` |
| `+0x1D8` | `0xC98F9D4A` aircraft | `0x098F9BF4` | `cISC4AutomataManager` |
| `+0x1DC` | `0xA98F9D28` watercraft | `0x098F9BF4` | `cISC4AutomataManager` |
| `+0x1E0` | `0xCA14CA03` controller mgr | `0xCA14C9FC` | `cISC4AutomataControllerManager` |
| **`+0x1E4`** | **`0x4A1B664E` script system** | **`0x8A1B6647`** | **`cISC4AutomataScriptSystem`** |

(`decomp/cSC4City.c:2132`–`2149`; the IIDs match the §0 table.)

### 15.3 The third cause: `QueryInterface` answers the CLSID, not the IID

The create call resolves the class and then `QueryInterface`s for the **IID**. A replacement class must therefore
answer its **interface IID**, not merely `1` (`GZIID_cIGZUnknown`) and its own CLSID. The lane's implementation
does the latter:

```cpp
bool cFC4AutomataScriptSystem::QueryInterface(uint32_t riid, void** ppvObj) {
    if (ppvObj == nullptr) return false;
    *ppvObj = nullptr;                                    // <-- clears the out param first
    if (riid == GZIID_cIGZSerializable) { ... }
    else if (riid == 1 || riid == kFC4_CLSID_cSC4AutomataScriptSystem) { ... }   // 0x4A1B664E = the CLASS id
    else return false;                                    // <-- 0x8A1B6647 lands here
    ...
}
```

(`cFC4Automata.cpp:94`–`105` in the automata lane's worktree; `kFC4_CLSID_cSC4AutomataScriptSystem = 0x4A1B664E`
at `cFC4Automata.h:36`.) `0x8A1B6647` matches no branch, so the function sets `*ppvObj = nullptr` and returns
false — the city's out parameter is left **null**, the create helper does not notice, and `FUN_0045bd20` later
dereferences it at `0x45BF1B`. Exactly the observed `ECX = 0`.

The same defect is on all six classes, which is why registering them all still faults at the first one in Init
order:

- script system (`cFC4Automata.cpp:94`): accepts `GZIID_cIGZSerializable` (`0xE4FDA3D4`),
  `GZIID_cIGZUnknown` (`1`) and CLSID `0x4A1B664E` — **missing IID `0x8A1B6647`**.
- controller manager (`cFC4Automata.cpp:147`): those plus `cIGZMessageTarget2` (`0x452294AA`) and CLSID
  `0xCA14CA03` — **missing IID `0xCA14C9FC`**.
- the shared manager (`cFC4Automata.cpp:212`): those plus `classId_` — **missing IID `0x098F9BF4`** for all four.

This is the third cause in the same family as the two already fixed (the `cIGZSerializable` primary-pointer mix-up
and the zeroed `SC4List` containers): it is a `QueryInterface` contract bug, not a record bug.

**Change to make.** In every automata class's `QueryInterface`, accept the **interface IID** and return the
interface sub-object for it:

| Class | Accept `riid ==` | Return |
|---|---|---|
| `cFC4AutomataScriptSystem` | `0x8A1B6647` | `cFC4ISC4AutomataScriptSystem*` |
| `cFC4AutomataControllerManager` | `0xCA14C9FC` | `cFC4ISC4AutomataControllerManager*` |
| `cFC4AutomataManager` (all four) | `0x098F9BF4` | `cFC4ISC4AutomataManager*` |

Keep the CLSID branch if you like (the original answers the CLSID too — that is the ordinary GZCOM convention),
but it is **not** a substitute for the IID. Do **not** return the raw primary `cIGZUnknown*` for the IID: return
the interface sub-object the city will call `+0x0C` on, so the vtable the game fetches has `Init` at slot 3
(§11). The four manager classes share one IID even though they have four distinct CLSIDs — a single
`riid == 0x098F9BF4` branch is correct for all four.

**Scope.** Fixing this unblocks the `Read -> 1`/Init crash only. It does not by itself make the six components
behave; the records and hooks are §2–§7.

### 15.4 On the `Read -> 1` log

All six logging `Read -> 1` and still ending with a null at `+0x1E4` is not a contradiction: the two paths ask for
different IIDs. The city's create path is `CreateObject(CLSID, IID)` (`decomp/cSC4City.c:2145`), while the save
path is `SC4ReadGZSerializable(stream, <other IID>, &field, 0)` (`decomp/cSC4City.c:3378`, itself gated on save
version `> 10`), which resolves through the class-object store and a different IID. A class can therefore answer
the serialization IID and report `Read -> 1` while the create path still leaves `+0x1E4` null
*[inferred]* — the two paths were not traced end to end here.

**Probe to settle it** (needs an in-game run, not available to this lane): log the return of
`CreateObject(0x4A1B664E, 0x8A1B6647, …)` and the value written to `cSC4City+0x1E4` immediately after the
ensure-components helper, and separately log what `SC4ReadGZSerializable` writes into the same field. If the
create leaves null and the read writes non-null, the ordering of ensure-components versus `cSC4City::Read`
decides which one wins and the crash depends on that order.
