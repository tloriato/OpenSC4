# Hit: the sequenced-sound engine (HitMan, track players, channels, tracks and hit lists) (behaviour spec)

Status: M6 behaviour spec, written 2026-10-22 (re-hitman lane, outside contributor). Every fact was read on the
Windows executable `bin/SimCity4.exe` (1.1.641, image base `0x00400000`) from the Ghidra export of the `cHit*`
classes; numeric facts (IDs, tables, opcode numbers, operand widths, constants) were checked on the instruction and
data bytes and carry addresses. Implementers work from this spec, the LGPL gzcom-dll SDK headers and general
knowledge only, never from decompiled code.

Tags: *[compat]* observable, must match; *[inferred]* consistent with the evidence, not proven; *[quirk]* original
behaviour reproduced as is; *[as read]* read from the export without a byte check.

Scope. "Hit" is the EA sequenced-sound engine SimCity 4 uses for every Hit-driven sound: the **HitMan** service, the
**track player** (a small bytecode machine that plays one *track*), the **channel** (one voice on a framework sound),
the **track data blocks** (the compiled `.hit` files), the **track definitions** (TKDT records), the **hit lists**,
the **sems** (sound-parameter mappers), the **control groups**, the Hit **INI** file and the debug-info service. It is
the "Hit" layer that [audio.md](audio.md) §2 names as boundary only (AU8). It does **not** cover:

- the framework sound system and manager (`cGZSndSys`, `cGZSndMgr`, the data sources and the `tSound` record):
  [audio.md](audio.md) §8.6, [sound-system.md](sound-system.md) §7 and [sound-framework.md](sound-framework.md);
- the TKDT text format's measured samples and the Hit-list binary layout: [audio.md](audio.md) §10.10 (this spec adds
  the field order, the reader and the semantics);
- the `.tlo` records (type `0x9D796DB4`): [audio.md](audio.md) §10.11;
- the freshness score and the radio, which sit on top of Hit (their own specs).

Relation to [sound-system.md](sound-system.md) §3–§5, the interface contract for `cHitTrackPlayer`, `cHitMan`,
`cHitChannel`, `cHitIniFile` and `cHitPropertySetTrackResource`: this spec adds the behaviour that spec leaves open
(the track bytecode of its SS1, the `.hit` file scan and hit lists of SS2, the channel of SS3) and the other twelve
`cHit*` classes. Two points were re-read on the bytes on 2026-10-23: `Play`'s retrigger calls `Kill` (sound slot 9,
`call [eax+0x24]` at `0x0093E16E`), and `Play` writes registers 0–3 **silently** (third argument 1 to register slot
3, `0x0093B268`, which skips the notification when it is non-zero, `0x0093B346`).

---

## 1. Identity *[compat]*

### 1.1 The director and its classes

`cHitManCOMDirector` (constructor `0x00935F28`, director ID `0x9B2D3D2F`, [audio.md](audio.md) §1.1) registers ten
classes:

| CLSID | Factory | Object | Size | Returned pointer |
|---|---|---|---|---|
| `0x7B1BD993` | `CreatecHitMan` | `cHitMan` (§3) | — | base |
| `0x3BC9E6E0` | `CreatecHitDebugInfo` | `cHitDebugInfo` (§11.4) | — | base |
| `0x5B945038` | `0x009358C0` | `cHitResourceFactory` (§4.1) | 8 | base |
| `0xDC45DCB1` | `0x00935DC5` | the persist-resource-manager holder (`0x009358EC`) | 0x1C | base |
| `0x2B2EB869` | `CreatecHitSegmentDataSourceChannelCreator` | channel creator (§11.2) | 0x20 | base |
| `0xA3CD96CF` | `0x00935886` | `cHitTrackPlayer` (§6) | 0x104 | `+0x0C` |
| `0x3B1EA5BE` | `CreatecHitChannel` | `cHitChannel` (§9) | 0x38 | base |
| `0x3D802D67` | `cHitIniFile::Create` | `cHitIniFile` (§2) | 0x68 | `+0x3C` |
| `0xABA06EF4` | `0x00935EBD` | `cPropertySetUint32XmlReader` | 0x30 | `+0x04` |
| `0x7B1ACFCD` | `0x009358D6` | a Hit list resource (§4.4) | 0x18 | base |

`OnStart` (`0x00935FF1`) creates `cHitMan` and `cHitDebugInfo` under IID `0x287FB697` and adds both to the framework's
system-service list. `PreAppInit` (`0x00936090`) registers factory `0x5B945038` with the persist resource manager
(its slot 19, `+0x4C`) for the four Hit resource types `0xDB9A4F83`, `0xA3CD96CF`, `0x7B1ACFCD` and `0xEBFEE345`, then
creates `0x2B2EB869` under IID `0x3AF9866F` and adds it to the sound manager as a sound creator (sound-manager slot 15,
`+0x3C`). `PreAppShutdown` calls `cHitMan` slot 4 (`Shutdown`) and `cHitDebugInfo` slot 3.

### 1.2 Services and interfaces

| ID | Meaning | Where |
|---|---|---|
| `0x1B2CE671` | HitMan system-service ID | `cHitMan` ctor `0x00939EA4` (`cRZSystemService(0x1B2CE671, 0x7A116)`) |
| `0xBB1BD9A8` | `cIHitMan` IID | `cHitMan::QueryInterface` `0x00937818` returns `this+0x1C` for it and for `1` |
| `0xFBC9CD04` | HitDebugInfo service ID | `cHitDebugInfo` ctor `0x0093A280` |
| `0x7BBA0CBD` | `cIHitDebugInfo` IID | `cHitDebugInfo::QueryInterface` `0x0093A035` (also `0x287FB697` → `this+4`) |
| `0xBB1BE367` | track-player IID | `cHitTrackPlayer::QueryInterface` `0x0093A810` (returns base) |
| `0x206077D2` | `cIGZSnd` (a playable sound) | track player `+0x0C`, channel base |
| `0x306077D2` | 3D sound interface | track player `+0x08`, channel `+0x04` |
| `0xFD584467` | register set (`cIGZSndRegisterSet`) | track player `+0x04`, channel `+0x0C` |
| `0x5B34F0AF`, `0xAA11A64F` | further sound interfaces | track player `+0x14` / `+0x10`, channel `+0x08` / `+0x10` |
| `0xDD583E2D`, `0xE9D00A9A` | track-player-only interfaces | `+0x18`, `+0x1C` |
| `0x09D002D2` | channel interface | channel `+0x14` |
| `0x9B82EAE7` | track data block | `cHitTrackDataBlock::QueryInterface` `0x00938CEA` |
| `0xDDCF3BD7` | sem | `cHitSem::QueryInterface` `0x0093860A` |
| `0xDB1ACFB2` | Hit list | `cHitList::QueryInterface` `0x0094392E` |
| `0xFB1C0050` | control group | the group's QI (`0x00937880`) |
| `0x3B93C5E7`, `0x456B8F1D` | Hit resource (persist resource) | `cHitResource<T>` / `cHitPropertySetTrackResource` QI |
| `0xEBB10CDD` | property-set (register) interface of a resource | every resource lookup in this spec |

`cHitMan::GetServicePriority` (`0x00939989`) returns 499,999; `cHitDebugInfo`'s (`0x00875794`) returns 499,998.

### 1.3 Resource keys

| Type | Group | Instance | Meaning |
|---|---|---|---|
| `0x0B8D821A` | any (`0xFC45CAF7` for the built-in ones) | track id | TKDT track definition, text form (§5) |
| `0x0B9EB87E` | any | track id | TKDT track definition, binary property-set form |
| `0x7B1ACFCD` | `0x9DBDBF74` in `Sound.dat` | list id | Hit list (§4.4) |
| `0xEBFEE345`, `0xEBFEE349` | any | id | plain Hit property sets (register sets, channel definitions) |
| `0xA3CD96CF` | — | — | a track player made as a resource (factory case) |
| `0xEBFEE345` | `0xFC45CAF7` | `0x0BFD8404` | the HitMan's **global register set** (§3.6) |
| `0x0B8D821A` | `0xFC45CAF7` | `0xAB903A1B` | the **default track**, parent of every TKDT (§5.3) |
| `0xEBFEE345` | `0xFC45CAF7` | `0xCB954E2C` | the **default channel** definition (§9.1) |

---

## 2. The Hit INI file (`cHitIniFile`)

A `cRZIniFile` subclass (0x68 bytes, QI `1` / `0xDD802D53`). Its path comes from the sound system (slot 20). Section
and key names are compared case-sensitively by `cRZIniFile` *[as read]*. `EnumDefaultEntries` (`0x0093756A`) runs, in
this order *[compat]*:

1. `[Config]`: `DefaultSndDataSourcePlayer2DClassId` → sound manager slot 8 (`SetDefaultSndDataSourcePlayer2DClassId`),
   `DefaultSndDataSourcePlayer3DClassId` → slot 10.
2. `[SndFileDefaultDataSourceClassIds]`: each `ext=clsid` → sound system slot 23.
3. `InitPackedDBSegments`: section `PackedDbSegments`, entries `type,x,path`; a leading `%DataDirectory%` is replaced
   by the data directory. When the file exists: GZCOM creates `0x286D4A6A` (IID `0x656B8F0C`), slot 3 `(id, path,
   0)`, slot 6 `(1, 0)`; the segment goes to HitMan `AddDBSegment` (slot 43) and the resource manager (slot 27,
   `+0x6C`); the entry is remembered in the set at `+0x40` so step 4 skips it.
4. `InitDBSegments` (slot 4): a `[Config]` flag (stored at `+0x50`); section `DBSegments` creates the segments,
   `DBSegmentDirectoryFiles` binds extensions (`group=type:ext,type:ext…` → HitMan `AddTypeToGroupAssociation`,
   slot 46; the shipped line `0xca4d19e3=0x9d796db4:tlo,0x2026960b:hit` is [audio.md](audio.md) §10.11), then the
   segments are initialised. Entries already packed in step 3 are skipped.
5. `InitTrackDataBlocks`: section `TrackDataBlocks`, each value a `.hit` path → HitMan `LoadHitFile` (slot 9, §3.2).
6. `InitHitManTypeGroups`: section `HitManTypeGroups`, lists of ids → HitMan slot 46 per id.
7. `InitDataSourceCreators`: `SndDataSourceCreators` → creator CLSID created with IID `0x9B75709E` → sound manager
   slot 25 (`RegisterSndDataSourceCreator`); each further entry → creator slot 4.
8. `InitSndCreators`: `SndCreators` → CLSID with IID `0x3AF9866F` → sound manager slot 15 (`RegisterSndCreator`); an
   optional per-creator definition-list section → creator slot 3.
9. `InitResourceFactories`: `ResourceFactories`, `type=clsid` → resource manager slot 19 (`+0x4C`).

---

## 3. The HitMan service (`cHitMan`)

### 3.1 Object and lifecycle

`cHitMan` derives from `cRZSystemService` and exposes `cIHitMan` at `+0x1C` (vtable `0x00AD2968`, 47 slots, §3.9).
Fixed fields read on the bytes: `+0x20` initialised flag, `+0x21` paused flag (read by `IsPaused`, never written by
any HitMan method read here), `+0x2C` the `cIGZSndSys` pointer, `+0x44`/`+0x48` the global register resource and its
register interface, `+0x4C`/`+0x50` the default track and its interface, `+0x54`/`+0x58` the default channel and its
interface, `+0xA8` the track-flow error handler, and the two default sems constructed in place at `+0xDC`
("2D default") and `+0xFC` ("3D default").

The constructor (`0x00939EA4`) sets the error handler to `0x009377DD` (§3.8) and seeds a `cRZRandom` with
`0xFFFFFFFF` (`+0x90`).

`Init` (`0x009399CB`, no-op when already initialised) *[compat]*:

1. registers the two default sems (`RegisterSem`, §3.5);
2. stores `cIGZSndSys`;
3. creates the **global register set**: a plain Hit property set keyed (`0xEBFEE345`, `0xFC45CAF7`, `0x0BFD8404`),
   registers it with the resource manager (slot 8, `+0x20`) and keeps its `0xEBB10CDD` interface;
4. creates the **default track** (`0x0B8D821A`, `0xFC45CAF7`, `0xAB903A1B`) the same way and fills it from the
   54-pair table at `0x00B16B38` (§5.3);
5. creates the **default channel** (`0xEBFEE345`, `0xFC45CAF7`, `0xCB954E2C`), sets `0xFF4C97BE = 0`, then fills it
   from the 29-pair table at `0x00B16CE8` (§9.1);
6. sets the initialised flag.

`Shutdown` (`0x00938643`, no-op when not initialised): releases the hit-list map, kills every track player still in
the player list (slot 9 of each), frees the type-group lists, drops the three built-in resources, clears the sem map
and the channel list, and clears the flag.

### 3.2 Track data blocks and `.hit` files

A **track data block** (`cHitTrackDataBlock`, 0x38 bytes) holds one compiled `.hit` file: its upper-cased absolute
filename (`+0x24`), the data pointer (`+0x08`) and size (`+0x20`), and a hash map *entry-point id → byte offset*.
`SetFilename` (`0x00939C7B`) makes a relative path absolute against the **application** directory (`RZ::ADir`), then
upper-cases it. `AddEntryPoint(id, off)` overwrites; `GetEntryPointOffset(id, &off)` fails for unknown ids.

The HitMan keeps the blocks in a list (`+0x7C`) guarded by a busy counter (`+0x80`).

- `RegisterTrackDataBlock` (slot 11, `0x009384C1`): the object must answer `0x9B82EAE7`; it is first removed (slot 12)
  then appended.
- `RemoveTrackDataBlock` (slot 12): removes every list entry equal to the block.
- `GetTrackDataBlockAndOffset(id, &block, &off)` (slot 13, `0x00937C00`): the **first** registered block whose map has
  `id` wins.
- `TrackDataEntryPoint(id)` (slot 14, `0x009379A4`): that block's data pointer + offset, or 0.

`LoadHitFile(path)` (slot 9, `0x0093880A`) *[compat]*:

1. if a registered block already has exactly `path` as its filename (string compare, **no** normalisation of `path`),
   return false;
2. if `FilepathHasEmbeddedKey(path)` (§3.3) yields a key and `ParseHitFileFromIGZPersistRecordAsIGZFile(key)`
   (`0x009379EF`) succeeds, return true. That function finds the DB segment holding the key (resource manager slot
   32), opens the record (segment slot 21), wraps it in GZCOM object `0x0A32C0DA` (IID `0x8A32C0AC`) as a file and
   runs `ParseHitFile` on it;
3. otherwise open `path` as a file (read, open-existing) and return `ParseHitFile`'s result; false if it cannot open.

`ParseHitFile(file)` (`0x00938D8E`) reads the whole file and builds a block *[compat]*:

- name := the file's name; data := the whole file (heap copy, owned by the block);
- the block is registered (slot 11);
- entry points: scan **byte by byte** for the dword `'ENTP'` (`0x50544E45`); after it read `(id, offset)` dword pairs
  until a pair's id equals `'EENT'` (`0x544E4545`) or the data ends; each pair → `AddEntryPoint`;
- the debug-info service gets `(name, data, size)` (its slot 9).

*[quirk]* The function first scans the data dword by dword for `'TRAX'` (`0x58415254`) and then discards the result.

`UnloadHitFile(path)` (slot 10, `0x0093906D`): makes `path` absolute against the **data** directory (`RZ::DDir`)
when it has no drive, upper-cases it, finds the block with that name, frees the block's data and removes it.
*[quirk]* Load resolves names against the application directory and Unload against the data directory.

### 3.3 `FilepathHasEmbeddedKey`

`0x00938EC6`: scans the path for three runs of 8 hex digits (`0-9a-fA-F`); each run is parsed as hex and must be
non-zero; success returns (type, group, instance) in that order *[compat]*. *[quirk]* After a run is found the scan
resumes **on that run's last character**, so in an unbroken run of hex digits the next key overlaps the previous one
by one digit; with separators (`-`, `_`, `.`) this has no effect. A zero value ends the scan with failure.

### 3.4 DB segments, type groups and hit lists

Maps kept by the HitMan: `AddDBSegment(id, seg)` (slot 43), `FindDBSegment(id, …)` (slot 44, forwards to the
segment's slot 0), `RemoveDBSegment(id)` (slot 45); `AddTypeToGroupAssociation(type, group)` (slot 46, `0x00938B6E`)
keeps, per type, a list of groups with no duplicates.

`GetHitList(id, iid, &out)` (slot 16, `0x00938118`): for each group associated with type `0x7B1ACFCD`, asks the
resource manager for (`0x7B1ACFCD`, group, `id`) under `iid`; the first hit wins.

### 3.5 Sems, channels, control groups, sequence groups

- **Sems** (§10): a map *sem id → sem*. `RegisterSem(sem)` (slot 19) stores under `sem.slot3()` (its id) and returns
  id ≠ 0; `GetSem(id)` (slots 22/23), `RemoveSem(id)` (20), `RemoveAllSems` (21), `NumSems` (24), `GetSemAt(i)` (25,
  map order).
- **Channels**: `AllocChannel` (slot 17, `0x00938438`) makes a `cHitChannel` (0x38 bytes), keeps its `0x206077D2`
  interface in a list and returns it; `FreeChannel` (slot 18) removes and releases it.
- **Control groups**: `ControlGroup(id)` (slot 35, `0x00938AF0`) returns the group for `id`, creating it on first use
  (0x38 bytes, ctor `0x0093873F`: an id, a set of tracks, `+0x14 = 0x400`); `AddToControlGroup(id, track)` (slot 36)
  adds the track to the group's set (`cHitControlGroup::AddTrack`, `0x00938527`).
- **Sequence groups**: `SetSequenceGroupSnd(id, snd)` (slot 37) / `GetSequenceGroupSnd(id, &snd)` (slot 38): one
  current sound per group id (§8.6, opcodes `0x45`–`0x49`).

### 3.6 Registers and ducking

`GetRegisterVal(id)` (slot 31) / `SetRegisterVal(id, v)` (slot 30) read and write the **global register set**
(§1.3). Ids below `0x100` are aliases (§7.1).

The **duck map** (slots 39–42) maps a sound object to a priority. `DuckMapSetSndobPri(obj, pri)` (`0x00938A41`, only
when initialised): `pri = 0` erases the entry; otherwise it is stored. When the map changed, global register `0x1B`
(`0xFFAE9DDB`, "duck level") := `DuckMapMaxPri()` (the largest priority, 0 when empty) and `UpdateView` runs.
`DuckMapRemoveAll` clears the map, sets register `0x1B` to 0 and runs `UpdateView`. `UpdateView` (slot 5,
`0x0093793F`) enumerates every sound of the sound system (`cIGZSndSys` slot 32) with `EnumFunc_UpdateView`
(`0x0093790B`) so the new duck level is applied *[inferred: the callback re-applies volume]*. The effect of the duck
level is in the sems (§10): a sound whose own `0xFFAE9DDB` priority is below the level plays at half volume.

### 3.7 `Random`, `Callback`, source data

- `Random(a, b)` (slot 26, `0x00939993`) = `cRZRandom::RandomUint32Uniform(a − b) + b` on the HitMan's generator.
  Callers pass (max, min), giving a value in [min, max) *[compat]*.
- `Callback(a, b)` (slot 27) is the stub `0x0090CE94` returning 0: the track opcode `0x22` (§8) does nothing in SC4.
- `RegisterSourceDataRequestHandler(h)` (slot 33) stores a handler; `GetSourceDataField(obj, field, &out)` (slot 32)
  forwards to it (opcode `0x48`).

### 3.8 The track-flow error handler

Every track error (§8.7) calls `HandleTrackFlowError(track, message)` (slot 29), which calls the handler set by
`SetTrackFlowErrorHandler` (slot 28). The default handler (`0x009377DD`) *[compat]* obtains the track's sound interface
and calls Kill (sound slot 9, `0x0093BBF0`), then marks the track's error condition
(track slot 16, `0x0093AA6F`). The mark is set after Kill and is observable through
GetErrorCondition. It ignores the error-message text (HM6; interface access
`0x00968303`, track table `0x00AD3E48`, sound table `0x00AD3D48`).

### 3.9 The `cIHitMan` table

**Pause API (HM5).** *[compat]* Slot 6 clears the duck map through slot 42 and
returns true (`0x00937875`, `0x00938492`). It neither pauses tracks nor sets the
manager's pause flag. Slot 7 simply returns true without changing state
(`0x00949ADE`). Slot 8 reads the flag at object `+0x21` (`0x0093998F`); construction
sets it to zero (`0x00939EA4`), and neither of those operations changes it. Do not
implement a global track pause/resume transition for these two calls.

Slots read from the vtable at `0x00AD2968` (names from the port's symbol tables):

| Slot | Method | Slot | Method | Slot | Method |
|---|---|---|---|---|---|
| 3 | `Init` | 19 | `RegisterSem` | 35 | `ControlGroup` |
| 4 | `Shutdown` | 20 | `RemoveSem` | 36 | `AddToControlGroup` |
| 5 | `UpdateView` | 21 | `RemoveAllSems` | 37 | `SetSequenceGroupSnd` |
| 6 | (named `Pause`; only calls slot 42) | 22, 23 | `GetSem` | 38 | `GetSequenceGroupSnd` |
| 7 | returns true | 24 | `NumSems` | 39 | `DuckMapSetSndobPri` |
| 8 | `IsPaused` | 25 | `GetSemAt` | 40 | `DuckMapSndobPri` |
| 9 | `LoadHitFile` | 26 | `Random` | 41 | `DuckMapMaxPri` |
| 10 | `UnloadHitFile` | 27 | `Callback` | 42 | `DuckMapRemoveAll` |
| 11 | `RegisterTrackDataBlock` | 28 | `SetTrackFlowErrorHandler` | 43 | `AddDBSegment` |
| 12 | `RemoveTrackDataBlock` | 29 | `HandleTrackFlowError` | 44 | `FindDBSegment` |
| 13 | `GetTrackDataBlockAndOffset` | 30 | `SetRegisterVal` | 45 | `RemoveDBSegment` |
| 14 | `TrackDataEntryPoint` | 31 | `GetRegisterVal` | 46 | `AddTypeToGroupAssociation` |
| 15 | `CreateTrackPlayer` | 32 | `GetSourceDataField` | | |
| 16 | `GetHitList` | 33 | `RegisterSourceDataRequestHandler` | | |
| 17 | `AllocChannel` | 34 | returns `this+0x40` | | |
| 18 | `FreeChannel` | | | | |

`CreateTrackPlayer(iid, &out)` (slot 15, `0x00937855`) allocates a `cHitTrackPlayer` (0x104 bytes) and returns its
`QueryInterface(iid)`.

---

## 4. Hit resources

### 4.1 The resource factory

`cHitResourceFactory::CreateInstance` (`0x00942F24`, vtable `0x00AD40D0`) dispatches on the key's type *[compat]*:

| Type | Object |
|---|---|
| `0x0B8D821A`, `0x0B9EB87E` | `cHitPropertySetTrackResource` (0x28) with `0xFF4C97BE = 0xAB903A1B` (parent = default track) and `0xFF1CDF6A = "Track"` |
| `0x7B1ACFCD` | Hit list resource (0x28, ctor `0x0094048B`) |
| `0xA3CD96CF` | track player (0x114, `0x009402A1`) |
| `0xEBFEE345`, `0xEBFEE349` | plain Hit property set (0x28, `0x009396D8`) |
| other | none (fails) |

`cHitResource<T>` objects answer `0x3B93C5E7` and `0x456B8F1D` (returning `this+0x18`), keep their resource key at
`+0x1C`, and copy key and contents in `CreatePrivateCopy` / `operator=`.

### 4.2 Track definitions: reading and writing

`cHitPropertySetTrackResource::Read` (`0x00940DFF`) first sets property `0xFF6F9546` := the record's instance id
(the track's own id), then reads the body: type `0x0B8D821A` → the text reader (§5.1); any other type → the standard
binary property-set serializer. `Write` (`0x0093FF4A`) writes text for `0x0B8D821A` (`Write_OldFormat`) and the
binary property set for `0x0B9EB87E`; other types fail.

### 4.3 Plain Hit property sets

Type `0xEBFEE345`/`0xEBFEE349`: a property set read and written with the standard serializer (`0x0093FF76`,
`0x0093FFA2`). They hold register sets and channel definitions.

### 4.4 Hit lists

**Reading** (`cHitList::ReadResource`, `0x00943DBC`), on the whole record of `size` bytes, choosing the first format
that fits *[compat]*:

| Format | Test | Body |
|---|---|---|
| text | `size == d0 + 4` (d0 = first dword) | the text after the first dword: tokens split by `,`; a token `a-b` adds every id `a…b` (nothing when `a > b`); other tokens are one id (decimal or `0x` hex) |
| binary v1 | `d0 == 1` and `size == d1·4 + 8` | `u32 1, u32 count, count × u32` ([audio.md](audio.md) §10.10) |
| bare | `size == d0·4 + 4` | `u32 count, count × u32` |

Anything else fails. After a successful read the list's **window** is set to `count / 3 + 1` (§4.5).

**Writing** (`0x00943CF2`): `u32 1`, `u32 count`, then the ids in list order (the available ids, then the recently
used ones); the order is preserved.

### 4.5 The Hit list as a shuffle bag *[compat]*

A Hit list has two ordered lists, **available** (`+0x08`) and **recent** (`+0x0C`), a **window** `w` (`+0x10`) and
an "explicit window" flag (`+0x14`). Its methods (vtable `0x00AD4648`):

| Slot | Method | Behaviour |
|---|---|---|
| 4 | `Add(id)` (`0x00943C01`) | append to available; unless the flag is set, `w := (|available| + |recent|) / 2` |
| 5 | `Remove(id)` (`0x00943A2E`) | remove every copy from both lists; recompute `w` the same way unless the flag is set |
| 6 | `Clear` (`0x0094397E`) | empty both lists |
| 7 | `SetWindow(w)` (`0x009404C2`) | `w := w`, flag := 1 |
| 8 | `Choose` (`0x00943C46`) | see below |
| 9 | `Count` (`0x0094067A`) | `|available| + |recent|` |
| 11 | `GetAt(i, &id)` (`0x00943A9B`) | index into available ++ recent; false when out of range |
| — | `SetAt(i, id)` (`0x00943B07`), `EraseAt(i)` (`0x00943B71`) | same indexing |

`Choose` *[compat]*:

```
if available is empty:
    id := recent.front; move it from the front to the back of recent; return id
k := HitMan.Random(|available|, 0)                 -- uniform in [0, |available|)
id := available[k]; erase it from available; append it to recent
if |recent| > w: move recent.front to the back of available
return id
```

The read path sets `w = count/3 + 1` **through `SetWindow`**, so the flag is set and later `Add`/`Remove` do not
recompute it.

---

## 5. Track definitions (TKDT)

### 5.1 The text form

`cHitPropertySetTrackResource::ReadLegacyFormat` (`0x00941930`) *[compat]*:

1. Read the record. If its first dword equals `size − 4` it is a length prefix and is skipped.
2. The next dword is examined: if it is `'2DKT'` (`0x544B4432`), the text starts 8 bytes after it (the record is
   `"2DKT"`, a `u32`, then `"TKDT,…"`); otherwise that dword was the `"TKDT"` tag itself and the text starts after it.
3. Split on `,`. Token 0 (the tag or empty) is skipped; token 1 is the **version** (decimal); token 2 is a name (kept
   as a string, unused).
4. Then the fields of §5.2 in order. An empty token leaves the property unset (it inherits, §7.2). Numbers are read
   with `RZConvertToUint32A` (decimal, or hex with `0x`). Field 6 is present only when version ≥ 1, fields 22–27 only
   when version ≥ 2.
5. Field 1 (sound id) equal to the track's own id (`0xFF6F9546`) makes the read **fail**.
6. Field 9 (sem) defaults to `0x7DCDED2D` ("2D default"), or `0x7DCDED3D` ("3D default") when field 7 is non-zero; a
   non-zero token overrides it. Field 9 is therefore always set.

`Write_OldFormat` (`0x00940E5C`) writes `TKDT,2,,` then the 27 fields, each followed by `,` (unset fields as empty;
fields 1, 2, 9, 13–16 as `0x%08lx`, the others in decimal), then the trailer string at `0x00AD44A0` (`ETKD`
[audio.md](audio.md) §10.10) *[compat]*.

### 5.2 The fields

| # | Property | Default track (§5.3) | Role |
|---|---|---|---|
| 1 | `0xFFAEC905` | 0 | sound id: the sample or track the channel plays (§6.5) |
| 2 | `0xFF5F15E1` | `0x492106EB` | entry-point id of the track code in the `.hit` data (§6.3) |
| 3 | `0xFF4DCD0D` | 0 | play-argument mode (§6.2) |
| 4 | `0xFF050A94` | `0x20` | priority (alias `0x13`) |
| 5 | `0xFFB0333D` | 0 | control-group id; a non-zero group can veto the start (§6.3) |
| 6 | `0xFF706616` | 0 | copied by the sems (alias `0x25`) *[inferred]* |
| 7 | `0xFFCC156F` | 0 | 3D flag (alias `0x1C`): the channel makes a 3D sound (§9.2) |
| 8 | `0xFFCC3B29` | 4 | default minimum distance for 3D (alias `0x38`, §10) |
| 9 | `0xFF891EAA` | `0x7DCDED2D` | sem id (§10) |
| 10 | `0xFFAE9DDB` | `0x20` | duck priority (alias `0x1B`, §3.6) |
| 11 | `0xFFF7B8DF` | 0 | passed to the sound's note-on as a flag; copied to the channel (alias `0x33`) *[inferred: loop]* |
| 12 | `0xFF90AAC8` | `0x400` | volume, `0x400` = unity (alias `0x15`) |
| 13 | `0xFF3C2160` | 0 | Hit list id loaded at start (§6.3) |
| 14 | `0xFFD7EE4D` | 0 | (read and written; no reader in the Hit classes) |
| 15 | `0xFF59FDD8` | 0 | dependent track started with this one (§6.2) |
| 16 | `0xFF2D7CD1` | 0 | (read and written; no reader in the Hit classes) |
| 17 | `0xFF0246FE` | 0 | initial wait, ms (§6.3) |
| 18 | `0xFFFF4E79` | 0 | fade-in time, ms |
| 19 | `0xFF595FFD` | 0 | fade-out time, ms |
| 20 | `0xFF8DCFD8` | 0 | pitch spread, low end, semitones (§6.7) |
| 21 | `0xFF69510B` | 0 | pitch spread, high end, semitones |
| 22 | `0xFF099DB2` | — | frequency (alias `0x17`) |
| 23 | `0xFF53DBF3` | `0x400` | fade gain, `0x400` = unity (alias `0x14`) |
| 24 | `0xFF3EC880` | `0x200` | pan, `0x200` = centre (alias `0x16`) |
| 25 | `0xFF47F685` | `0xFF53DBF3` | the property a fade acts on |
| 26 | `0xFFCAA344` | 0 | 3D flag passed to the 3D sound (alias `0x1D`) |
| 27 | `0xFF303B1B` | 0 | retrigger: `Play` on a playing track restarts it only when non-zero (§6.2) |

### 5.3 The default track and the parent chain

The default track `0xAB903A1B` is filled from the 54 pairs at `0x00B16B38` (`Init`, §3.1) *[compat]*:

```
0xFF4C97BE=0          0xFF1CDF6A=0xFFFA9106  0xFF5F15E1=0x492106EB  0xFFAEC905=0
0xFF4DCD0D=0          0xFF050A94=0x20        0xFFB0333D=0           0xFF706616=0
0xFF303B1B=0          0xFFCC156F=0           0xFFCC3B29=4           0xFF891EAA=0x7DCDED2D
0xFFAE9DDB=0x20       0xFFF7B8DF=0           0xFFB4335C=0xBAADB0DE  0xFF90AAC8=0x400
0xFF53DBF3=0x400      0xFF3C2160=0           0xFFD7EE4D=0           0xFF59FDD8=0
0xFF2D7CD1=0          0xFF0246FE=0           0xFF8DCFD8=0           0xFF69510B=0
0xFF3EC880=0x200      0xFFFF4E79=0           0xFF595FFD=0           0xFF47F685=0xFF53DBF3
0xFF2D16F2=0          0xFFD1C82F=10          0xFF0AD592=0           0xFFCAA344=0
0xFFCC156F=0          0xFFA6A614=1000000     0xFFAAB623=1000000     0xFFCC4D2D=0
0xFF4A01D6=0          0xFF469820=0           0xFF444C9A=0           0xFFC20061=0
0xFFCE9997=0          0xFFC6B36D=0           0xFF40FF96=0           0xFF4C6660=0
0xFFC6E666=0          then nine pairs 0=0
```

*[quirk]* The table's last nine entries are zero, so the default track also holds a property with key 0 and value 0.
`0xFFCC156F` appears twice (both 0).

Every TKDT read through the factory starts with `0xFF4C97BE = 0xAB903A1B`. **`0xFF4C97BE` is the parent link**:
when a track definition is set on a player (§6.1), the player follows it: (`0x0B8D821A`, `0xFC45CAF7`, parent) is
looked up, appended to the player's lookup chain, and its own `0xFF4C97BE` read, until the parent is 0 or the lookup
fails. The default track ends the chain (`0xFF4C97BE = 0`). *[quirk, inferred]* a cycle of parents never ends.

---

## 6. The track player (`cHitTrackPlayer`)

### 6.1 Object

0x104 bytes (ctor `0x0093E8F4`), eight interfaces (§1.2), an embedded `cHitChannel` at `+0xC8`. The fields the
interpreter uses, read on the bytes of `DoCommand`: `+0x44` wait (ms), `+0x48` note-length counter (ticks), `+0x58`
running flag, `+0x68` code base, `+0x6C` instruction pointer, `+0x70` code start, `+0x74` loop point, `+0x78` call
stack, `+0x7C` the channel's register set (`0xFD584467`), `+0x80` the channel's `cIGZSnd` (`0x206077D2`), `+0x84` the
"target" register set, `+0x9C`/`+0x9D`/
`+0x9E` the compare flags (equal, greater, less), `+0xB0` the Hit list. The constructor binds the sound system, the
HitMan and the sound manager, and counts live players in a global (`0x00B633B4`; the channel destructor decrements
it).

**Registers.** A player's register set (interface `+0x04`) reads a property through a **chain** (`+0x9C`… vector):
its own local set first, then the track definition and its parents (§5.3). `Set(id, v, silent)` (`0x0093B268`)
writes the local set, then pushes the value to every **dependent** track (§6.2) whose own local set does not define
`id`, then, unless `silent`, calls `HandleNotifySetPropertyValue(id)` (§6.8). Ids below `0x100` are aliases (§7.1).
`SetTrackDef(def)` (`0x0093EAA1`) rebuilds the chain (§5.3), then applies the definition's entry point (`0xFF5F15E1`,
slot 26) and sem (`0xFF891EAA`, HitMan `GetSem`).

### 6.2 `Play`

`Play(a, b, c, d)` (`0x0093E146`) *[compat]*:

1. If the track is playing: return true unless `0xFF303B1B` (retrigger) is non-zero; then `Kill` it first (sound
   slot 9, `0x0093E16E`).
2. Registers 0–3 (`0xFF141C48`, `0xFF9250B3`, `0xFF9EC945`, `0xFF1885BE`) := `a, b, c, d` (silent).
3. Map the arguments by mode `0xFF4DCD0D`:

| Mode | Effect |
|---|---|
| 1 | volume `0xFF90AAC8` := a, pan `0xFF3EC880` := b |
| 2 | game object `0xFFB4335C` := a |
| 3 | object := a, volume := b, pan := c |
| 4 | position x `0xFFCC4D2D` := a, y `0xFF4A01D6` := b, z `0xFF469820` := c |
| 5 | object := a, x := b, y := c, z := d |
| other | nothing |

4. Load (`Load` increments a use count; the first load loads the channel's sound).
5. If `0xFF59FDD8` is non-zero, start that track as a dependent (`StartSndDependent`, §6.6).
6. If the load succeeded, start (§6.3) and return its result.

### 6.3 Start

The start routine (`0x0093B791`, named `PlayPause`) *[compat]*:

1. A nonzero control-group ID `0xFFB0333D` obtains the group through `ControlGroup`,
   creating it when missing. Group slot 4 returns its sound interface; slot 20 reads
   group volume. Zero volume vetoes the start while returning success. A new group
   starts at 1024, so creation alone does not veto playback (`0x0093B791`,
   `0x00938AF0`, `0x0093873F`; tables `0x00AD2718`, `0x00AD2680`; getter
   `0x0099BE66`). This start path itself can create a group without an outside
   `AddToControlGroup` caller (HM1); whether installed track data requests one is
   a separate data question.
2. Slots 7, 19 and 18 of the player's own `+0x0C` sound interface mean IsPlaying,
   IsPaused and Unpause. IsPlaying includes either the player's running state or a
   playing dependent (`0x0093AFEE`). IsPaused reads the 32-bit pause word at object
   `+0x5C` (`0x0093E48B`), not a separate resume latch. If both predicates are true,
   the start routine sets that same word to 1 and calls Unpause, which immediately
   clears it. It then unpauses dependents and the paused channel, performs a tick
   with elapsed time 0, and registers for 20 ms updates if still playing; it returns
   whether the error condition is clear (`0x0093B08B`). Pause sets the same word to
   1 and removes the update target (`0x0093B01E`). Constructor `0x0093E8F4` and the
   ordinary start path `0x0093B8CC` also clear it. Thus the resume branch does not
   leave subsequent ticks blocked (HM3).
3. **Fade-in**: when `0xFFFF4E79` (fade-in ms) is non-zero: `0xFF2D16F2` (fade target) := current `0xFF53DBF3`
   (unless a fade is already running), `0xFF47F685` := `0xFF53DBF3`, `0xFF53DBF3` := 0,
   `0xFFD1C82F` (fade rate) := `target × 1000 / fadeInMs`; the fading flag `0xFF0AD592` := 1 when the rate is non-zero.
4. Re-set `0xFFAEC905` to its own value (this runs `UpdatePatch` through the notification), `UpdateVolPan`.
5. Set the entry point from `0xFF5F15E1`; failure ends with false. The instruction pointer and the loop point := the
   code start.
6. Clear the Hit list; when `0xFF3C2160` is non-zero load it (HitMan `GetHitList`); failure ends with false.
7. Wait := `0xFF0246FE`; `UpdatePitch`; running := 1; the player takes a reference on itself; register
   `0xFF245400` (alias 44, "playing") := 1.

The tick (§6.4) is driven by the sound system (`RZ::SS()`, the cached `cIGZSndSys`): `Unpause` (`0x0093B08B`)
registers the player's `+0x08` interface with it (slot 30) with the period `0x14` = 20 *[inferred: ms]*; `Pause`
(`0x0093B01E`) and `Kill` unregister it (slot 31). `Pause` and `Unpause` also pause and resume the dependents and the
channel.

### 6.4 The tick (`HandleTimerCallback`, `0x0093DF4C`)

Called with the elapsed time `dt` (ms) *[compat]*. If the pause word is nonzero,
return before dependent cleanup, fades or instruction execution (`0x0093DF64`):

1. Drop the dependents that are no longer playing.
2. If not running and no dependents are left: stop (slot 9 of `+0x0C`) and return.
3. **Fade** (when `0xFF0AD592` is set): `key := 0xFF47F685`, `cur := reg[key]`, `target := 0xFF2D16F2`,
   `rate := 0xFFD1C82F`; a zero rate clears the flag. The rate's sign is made to point from `cur` to `target`;
   `cur += rate × dt / 1000` (integer); when `cur` passes `target` it is clamped to it and the flag cleared;
   `reg[key] := cur`.
4. If the "stop when faded" mark is set and the fade has ended: stop and return.
5. `wait := max(0, wait − dt)`.
6. While running, not paused and `wait == 0`: execute one instruction (§8). After more than 100 instructions in one
   tick the error "Too many instructions executed without wait (infinite loop?)" is reported on every further
   instruction *[compat]*; the loop does not stop.
7. Note length: when non-zero, decrement it (once per tick, not by `dt`); on reaching 0, stop the channel's sound
   (`cIGZSnd` slot 8 of `+0x80`, as opcode `03`).

### 6.5 `UpdatePatch`, `NoteOn`, `UpdateVolPan`

`UpdatePatch` (`0x0093B9AE`): `id := 0xFFAEC905`. If `id` equals the track's own id `0xFF6F9546`: error "Recursive
sound id", false. When `id` changed, the channel is dropped. With no channel and `id ≠ 0`, the sound manager creates
the sound (`GetSound`, slot 18: `(id, 0, 0, IID 0xFD584467)`); failure → error "Sample file not found", false. Then:
take its `0x206077D2` interface; resolve the sem (`0xFF891EAA`) and let it set the sound up (sem slot 6); copy
`0xFFF7B8DF` to the channel; `UpdateVolPan`; `UpdatePitch`.

`NoteOn(note)` (`0x0093BFEF`, one argument, **ignored** *[quirk]*): with no channel → error "Attempt to play note with
no channel", false. Else `UpdatePatch`, the sound's note-on (slot 6) with `(0x400, 0, 1)`, `UpdateVolPan`,
`UpdatePitch`; returns the note-on result.

`UpdateVolPan` (`0x0093AE8C`): when there is a channel and a sem, the sem maps the player's registers onto the
channel (sem slot 5, §10).

### 6.6 Dependents

`StartSndDependent(id)` (`0x0093C0EC`): the sound manager creates `id` (IID `0x206077D2`); it is added to the
dependent list; every property of the player's definition chain that the dependent does not define locally is copied
to it (silent); then the dependent is played with registers 0–3 as its arguments.

### 6.7 `UpdatePitch` (`0x0093B508`) *[compat]*

With a channel whose sound is loaded:

1. Test whether frequency `0xFF099DB2` is present in the local register set or the
   directly assigned track definition. This predicate is register-interface slot 6
   (`0x0093AE2D`); unlike the value getter, it does not walk further parents. If
   absent from both, obtain the sound's original frequency (sound slot 28), attempt
   to write it into the assigned definition, and set the player's frequency through
   the register interface (`0x0093B54D`–`0x0093B5B7`).
2. `f := 0xFF099DB2`, `lo := 0xFF8DCFD8`, `hi := 0xFF69510B`. If either is non-zero:
   `fLo := lo ≠ 0 ? f × r^lo : f`, `fHi := hi ≠ 0 ? f × r^hi : f` with `r = 2^(1/12)` (the double at `0x00AD2B80` =
   1.0594630943592953), both truncated to integers; `f := HitMan.Random(fHi, fLo)`; set `0xFF099DB2 := f` (silent).
3. Send `f` to the channel (`0xFF099DB2`); re-send `0xFF3071B9` and `0xFF86FE79` when set locally.

*[quirk]* Step 2 writes the randomised frequency into the player's local set,
through register slot 3 (`0x0093B268`, called at `0x0093B66F`). Later value reads
prefer this set (`0x0093B200`), and the presence predicate now succeeds there.
Consequently subsequent `UpdatePitch` calls spread around the previous random
value until the frequency is overwritten or removed. Slot 6 is a presence test,
not a setter; the compounding itself is confirmed (HM8).

### 6.8 `HandleNotifySetPropertyValue` (`0x0093BD06`)

Changing a register on the player has these effects *[compat]*:

| Property | Effect |
|---|---|
| `0xFFAEC905` (sound id) | `UpdatePatch` |
| `0xFF099DB2` (frequency) | `UpdatePitch` |
| `0xFFB36659` | channel pause (`0`) or resume |
| `0xFF5F15E1` | set the entry point (slot 26); its result is returned |
| `0xFFC6E666` | clear the target; when non-zero, find the dependent whose id (slot 12) matches and make its register interface the target (error "Attempt to set target to object with no register interface") |
| `0xFF2D16F2` | with a non-zero value, clears the "stop when faded" mark |
| volume, pan, gain, priority, duck, 3D and position/velocity ids | `UpdateVolPan` |
| a set of float ids (`0xFF86FE79`, `0xFF3071B9`, …) | forwarded to the channel as floats |
| other listed ids | forwarded to the channel as integers |

### 6.9 `Stop`, `Kill`, `CompletionTime`

`Stop` (`0x0093B108`): stops the dependents. If the player is playing, its sound is loaded, `0xFF595FFD` (fade-out
ms) is non-zero and no fade runs: fade `0xFF53DBF3` to 0 at `gain × 1000 / fadeOutMs`, set the "stop when faded"
mark and return (the tick stops it, §6.4). Otherwise it is killed now (slot 9) *[as read]*.

`Kill` (`0x0093BBF0`): stops and drops every dependent, note off, stops the sound, clears the fading flag and the mark,
empties the call stack, unregisters the timer, removes the player from the duck map (`DuckMapSetSndobPri(this, 0)`)
and from its control group, sets `0xFF245400 := 0` and releases the self-reference taken at start.

`CompletionTime` (`0x0093ACD6`): the channel's play position over its length as a float; 0 without a channel.

---

## 7. Properties and registers

### 7.1 Register aliases *[compat]*

Ids below `0x100` are translated by the 80-entry table at `0x00B169F8` (`0x009377C9`: `id < 0x50` → `table[id]`; a
zero entry or `id ≥ 0x50` keeps the id as is):

```
 0 FF141C48   1 FF9250B3   2 FF9EC945   3 FF1885BE   4 FF87FAA9   5 FF6A6FB6   6 FF66F640   7 FFE0BABB
 8 FF7FC5AC   9 FFF98957  10 FFF510A1  11 FF735C5A  12 FF4DA274  13 -          14 -          15 -
16 FF4DCD0D  17 FF5F15E1  18 FFAEC905  19 FF050A94  20 FF53DBF3  21 FF90AAC8  22 FF3EC880  23 FF099DB2
24 FFB36659  25 FFA9B525  26 FFF3EDF2  27 FFAE9DDB  28 FFCC156F  29 FFCAA344  30 FFAAB623  31 FFA6A614
32 FFCC4D2D  33 FF4A01D6  34 FF469820  35 FFD4DFBD  36 FF63002C  37 FF706616  38 FF7E5922  39 FF2D16F2
40 FF47F685  41 FFD1C82F  42 FF0AD592  43 -          44 FF245400  45 -          46 -          47 FFC6E666
48 FFB0333D  49 FF303B1B  50 -          51 FFF7B8DF  52 FFB4335C  53 FFD7EE4D  54 FF8DCFD8  55 FF69510B
56 FFCC3B29  57 FF891EAA  58 FF59FDD8  59 FF2D7CD1  60 FF0246FE  61 FFFF4E79  62 FF595FFD  63 FF3C2160
64 FF444C9A  65 FFC20061  66 FFCE9997  67 FFC6B36D  68 FF40FF96  69 FF4C6660  70 -   71 -   72 -   73 -
74 FF734343  75 FFD3D39B  76 FFBDB196  77 FFA42951  78 FF86FE79  79 FF0150F2
```

Registers 0–3 are the play arguments, 4–12 general-purpose *[inferred]*; the others name the properties of §5.2 and
the channel properties of §9.

### 7.2 Roles used across the engine

`0xFF6F9546` own id; `0xFF4C97BE` parent; `0xFFB4335C` game object (default `0xBAADB0DE`); `0xFF245400` playing;
`0xFF0AD592` fading; `0xFF2D16F2` fade target; `0xFFD1C82F` fade rate (per second); `0xFF47F685` faded property;
`0xFFCC4D2D`/`0xFF4A01D6`/`0xFF469820` position; `0xFF444C9A`/`0xFFC20061`/`0xFFCE9997` the second 3D vector
(aliases 64–66) *[inferred: velocity]*; `0xFFAAB623`/`0xFFA6A614` 3D minimum/maximum distance (aliases 30/31).

---

## 8. The track bytecode *[compat]*

### 8.1 Encoding

A track is a byte string in a track data block. The instruction pointer points into it; jump and call targets are
**byte offsets from the code base** (`+0x68`). Each instruction is a one-byte opcode followed by its operands:
`r` = one unsigned byte, a register id (checked: ids above `0x4A` report "Illegal Register Id" and use register 1);
`b` = one signed byte; `l` = a little-endian `u32`/`i32`. Register values are 32-bit signed integers. Every
instruction "succeeds" (the tick continues), except as noted.

`DoCommand` (`0x0093C229`) decodes one instruction; the 97-entry jump table is at `0x0093DC8B` (opcodes
`0x01`–`0x61`; `0x00` and anything above `0x61` is "Illegal Instruction"). Names are the original's, from its error
strings.

### 8.2 The opcodes

| Op | Name | Operands | Effect |
|---|---|---|---|
| 01 | note on (timed) | `r1 r2` | note length := `reg[r2]`; `NoteOn(reg[r1])`; on failure read `0xFFAEC905` and report "Could not set patch - file or resource not found" |
| 02 | note on | `b` | `NoteOn(b)` |
| 03 | note off | (1 byte skipped) | if there is a channel, stop its sound (`cIGZSnd` slot 8 of `+0x80`); returns that call's result |
| 04 | `Loadb` | `r b` | `reg[r] := b` (sign-extended) |
| 05 | `Loadl` | `r l` | `reg[r] := l` |
| 06, 4A | `Set` | `r1 r2` | `reg[r1] := reg[r2]` |
| 07 | call | `l` | push the return address; ip := base + l |
| 08 | return | — | pop the return address; with an empty stack behaves as `0C` |
| 09 | `Wait` | `r` | wait := `reg[r]` ms |
| 0A | call entry | `l` | push the return address; ip := `HitMan.TrackDataEntryPoint(l)` |
| 0B | wait for channel | — | if the channel's sound is playing (slot 7): ip back onto this opcode, wait := 2 |
| 0C | end | — | if running: running := 0, release the self-reference |
| 0D | jump | `l` | ip := base + l |
| 0E | `Test` | `r` | compare `reg[r]` with 0 (§8.3) |
| 0F, 1D, 1E, 5D, 5E | no-op | — | |
| 10 | `Add` | `r1 r2` | `reg[r1] += reg[r2]` |
| 11 | `Sub` | `r1 r2` | `reg[r1] −= reg[r2]` |
| 12 | `Div` | `r1 r2` | `reg[r1] := reg[r1] / reg[r2]` (signed `idiv`; no zero check) |
| 13 | `Mul` | `r1 r2` | `reg[r1] *= reg[r2]` |
| 14 | `Cmp` | `r1 r2` | compare `reg[r1]` with `reg[r2]` |
| 15 | `Less` | `r1 r2` | `reg[r1] := reg[r1] < reg[r2]` |
| 16 | `Greater` | `r1 r2` | `reg[r1] := reg[r1] > reg[r2]` |
| 17 | `Not` | `r` | `reg[r] := (reg[r] == 0)` |
| 18 | `Rand` | `r1 r2 r3` | `reg[r1] := HitMan.Random(reg[r3], reg[r2])`, i.e. in [`reg[r2]`, `reg[r3]`) |
| 19 | abs | `r` | `reg[r] := |reg[r]|` (its error string says `kTrackCmdRand`) |
| 1A, 34–3D | illegal | — | "Illegal Instruction" |
| 1B | stop | — | stop the sound (slot 8 of `+0x0C`) |
| 1C | `Assert` | `r` | if `reg[r] == 0`: as `1B` |
| 1F | `GetVar` | `r1 r2` | `reg[r1] := reg[reg[r2]]` |
| 20 | loop | — | ip := loop point |
| 21 | set loop | — | loop point := ip |
| 22 | `Callback` | `r1 r2` | `HitMan.Callback(reg[r1], reg[r2])` (a stub in SC4, §3.7) |
| 23 | `SmartAdd` | `r` | Hit list `Add(reg[r])` |
| 24 | `SmartRemove` | `r` | Hit list `Remove(reg[r])` |
| 25 | smart clear | — | Hit list `Clear` |
| 26 | `SmartSetCrit` | `r` | Hit list `SetWindow(reg[r])` |
| 27 | `SmartChoose` | `r` | `reg[r] := Choose()`; errors "HitList is NULL", "Attempt to choose from empty hitlist" |
| 28 | `And` | `r1 r2` | **`reg[r1] := reg[r1] > reg[r2]`** *[quirk]*: compiled as `Greater` (`setg` at `0x0093D06F`) |
| 29 | `Nand` | `r1 r2` | `reg[r1] := !(reg[r1] && reg[r2])` |
| 2A | `Or` | `r1 r2` | `reg[r1] |= reg[r2]` (bitwise) |
| 2B | `Nor` | `r1 r2` | `reg[r1] := !(reg[r1] || reg[r2])` |
| 2C | `Xor` | `r1 r2` | `reg[r1] ^= reg[r2]` (bitwise) |
| 2D | `Max` | `r l` | if `reg[r] > l`: `reg[r] := l` (an upper bound) |
| 2E | `Min` | `r l` | if `reg[r] < l`: `reg[r] := l` (a lower bound) |
| 2F | `Inc` | `r` | `reg[r] += 1` |
| 30 | `Dec` | `r` | `reg[r] −= 1` |
| 31 | `PrintReg` | `r` | reads `reg[r]`, nothing else |
| 32 | `PlayTrack` | `r` | `StartSndDependent(reg[r])`; 0 → "Attempt to playtrack 0" |
| 33 | kill track | `r` | `reg[r] == 0`: kill this track (`cIGZSnd` slot 9 of `+0x0C`); else enumerate the sound system's sounds (its slot 32) with `0x0093AB4B`, which kills (slot 9) every track whose definition's instance is `reg[r]` |
| 3E | `je` | `l` | jump if equal |
| 3F | `jne` | `l` | jump if not equal |
| 40 | `jg` | `l` | jump if greater and not equal |
| 41 | `jl` | `l` | jump if less and not equal |
| 42 | `jge` | `l` | jump if greater or equal |
| 43 | `jle` | `l` | jump if less or equal |
| 44 | `SmartSetList` | `r` | `reg[r] == 0`: clear the Hit list; else load list `reg[r]` (HitMan `GetHitList`); none → "Undefined Hit List" |
| 45 | `SeqGroupKill` | `r` | see §8.6 |
| 46, 47 | `SeqGroupReturn` | `r` | see §8.6 |
| 48 | source data | `r1 r2 r3` | `reg[r1] := HitMan.GetSourceDataField(reg[0xFFB4335C], reg[r3])` (r2 read and unused); no object → "App data request has no game object id"; failure → "Illegal source parameter request" |
| 49 | `SeqGroupTrackId` | `r1 r2` | `reg[r2] := id of the group `reg[r1]`'s current sound (0 if none)`; group 0 → "Illegal sequence group ID" |
| 4B | `SetLocalTarget` | `r1 r2` | with a target set: `target[r1] := reg[r2]` |
| 4C | `SetTargetLocal` | `r1 r2` | `reg[r1] := reg[r2]` *[quirk]*: the source is the local set, not the target (`0x0093C614`), so it behaves as `Set` |
| 4D | `WaitEq` | `r1 r2` | unless `reg[r1] == reg[r2]`: ip back onto the opcode, wait := 2 |
| 4E | `WaitNe` | `r1 r2` | waits while equal |
| 4F | `WaitGt` | `r1 r2` | waits until `reg[r1] > reg[r2]` |
| 50 | `WaitLt` | `r1 r2` | waits until `reg[r1] < reg[r2]` |
| 51 | `WaitGe` | `r1 r2` | waits until `reg[r1] >= reg[r2]` |
| 52 | `WaitLe` | `r1 r2` | waits until `reg[r1] <= reg[r2]` |
| 53 | duck | — | `HitMan.DuckMapSetSndobPri(this sound, reg[0xFFAE9DDB])` |
| 54 | unduck | — | `HitMan.DuckMapSetSndobPri(this sound, 0)` |
| 55 | `TextX` (range test) | `r1 r2 r3` | `v = reg[r1]`: equal flag := `v < reg[r2] || v > reg[r3]`, greater := `v < reg[r3]`, less := `v > reg[r2]` |
| 56 | `SetLocalGlobal` | `r1 r2` | global property `reg[r1]` := `reg[r2]` in the sound system's property cache (sound-system slot 43, cache slot 11) |
| 57 | `SetGlobalLocal` | `r l` | `reg[r]` := global property `l` (cache slot 13) |
| 58 | throw | — | reports "Track threw an exception (throw instruction)", then executes the next byte as `5B`'s operands |
| 59 | object property | `r1 r2 r3` | set property `reg[r3]` of game object `reg[0xFFB4335C]` to `reg[r1]` through the property cache (cache slot 6); r2 read and unused |
| 5A | stop track | `r` | when `reg[r] ≠ 0`: the same enumeration with `0x0093ABD4`, which **stops** (slot 8) the matching tracks (fade-outs apply, §6.9); both `33` and `5A` report errors as `kTrackCmdKillTrack` |
| 5B | channel set | `r1 r2` | channel register `r1` := `reg[r2]` (through `+0x7C`, notifying the channel) |
| 5C | replay args | — | play the channel's sound (`cIGZSnd` slot 5 of `+0x80`) with registers 0–3 as arguments |
| 5F | `SmartIndex` | `r1 r2` | `reg[r1] := list[min(reg[r2], count − 1)]`; errors "HitList is NULL", "Attempt to index empty hitlist" |
| 60 | `SetDirect` | `l1 l2` | `reg[l1] := reg[l2]` (full 32-bit ids) |
| 61 | `SetIndirect` | `l1 l2` | `reg[l1] := reg[reg[l2]]` |

The `Wait*` opcodes re-execute themselves every 2 ms of wait, so a waiting track polls once per tick.

### 8.3 Flags

`Test` and `Cmp` set the three flags from `(a, b)` (`0x0093A8CB`): equal := `a == b`, greater := `a > b`, less :=
`a < b`. The conditional jumps read them as in §8.2. A float variant (`0x0093A8F9`) exists with the same rule.

### 8.4 The call stack

`07` and `0A` push the address after their operand on a list (`+0x78`); `08` pops. `Kill` empties it.

### 8.5 Errors

Every error in §8.2 goes to `HandleTrackFlowError(this, text)` (§3.8); unless the handler stops the track, execution
continues with the next instruction. An out-of-range register id is replaced by 1.

### 8.6 Sequence groups

`SeqGroupKill r` (`0x45`): `g := reg[r]`; `cur := HitMan.GetSequenceGroupSnd(g)`. If there is none, or it is this
sound: `SetSequenceGroupSnd(g, this)` and continue. If `cur` is not playing: same. Otherwise, when `cur`'s
`0xFF53DBF3` is non-zero, make it fade out (`0xFFD1C82F := 50000`, `0xFF47F685 := 0xFF53DBF3`, fading := 1, unless it
already fades that property), then this track waits (ip back, wait := 2) until it has stopped; when its gain is
already 0 it is stopped at once. `SeqGroupReturn r` (`0x46`, `0x47`): `g = 0` → "kTrackCmdSeqGroupKill: No group ID";
none or self → take the group; `cur` playing → stop this sound; else continue.

---

## 9. The channel (`cHitChannel`)

### 9.1 Object and defaults

0x38 bytes (ctor `0x0093FE12`): a voice that plays one framework sound (`+0x24`) for a track. It has its own register
set (interface `+0x0C`) built like the player's, the property-set chain starting from the default channel
`0xCB954E2C` (set by the channel creator, §11.2), whose 29 pairs at `0x00B16CE8` are *[compat]*:

```
0xFFAAB623=100  0xFFAAB623=10000  0xFF050A94=0x20  0xFF53DBF3=0x400  0xFF90AAC8=0x400  0xFF3EC880=0x200
0xFFB36659=0    0xFFAE9DDB=0      0xFFCC156F=0     0xFFCAA344=0      0xFFCC4D2D=0      0xFF4A01D6=0
0xFF469820=0    0xFF706616=0      0xFF2D16F2=0     0xFF47F685=0xFF53DBF3                0xFFD1C82F=0
0xFFF7B8DF=0    0xFF0AD592=0      0xFFFF4E79=0     0xFF595FFD=0      0xFF3C2160=0      0xFF444C9A=0
0xFFC20061=0    0xFFCE9997=0      0xFFC6B36D=0     0xFF40FF96=0      0xFF4C6660=0      0xFFAEC905=0
```

*[quirk]* `0xFFAAB623` is written twice; the second value, 10000, stands.

### 9.2 Making the sound

`CreateSnd` (`0x0093F304`): `0xFFCC156F` (3D) chooses the sound manager's default 3D (`0xFFCC156F ≠ 0`) or 2D
data-source player class (sound-manager slots 11 / 9); GZCOM creates it with IID `0x206077D2`; the sound's
`0x09D002D2` interface is kept (`+0x28`). For 3D: the 3D interface (`0x306077D2`) gets the `0xFFCAA344` flag (slot
15), the minimum distance `0xFFAAB623` (slot 12) and the maximum distance `0xFFA6A614` (slot 14), each only when
non-zero. Returns whether the interface was obtained.

`UpdatePatch` (`0x0093F687`) creates the sound when missing, hands it the data source (`+0x2C`) through the kept
interface (slot 3, mode 2), then calls `UpdateVolPan` (channel slot 45) and `UpdatePitch` (slot 44).
`NoteOn` (`0x0093F806`): `UpdatePatch`, then (if the channel's check slot 14 passes) the sound's play (slot 6) with
the current volume and the `0xFFF7B8DF` flag. `Load` (`0x0093F84D`) loads the sound.

### 9.3 Volume, pan, pitch

`UpdateVolPan` (`0x0093F136`) *[compat]*: 2D → the sound's pan (slot 25) := `0xFF3EC880`; 3D → the 3D interface
gets minimum distance (slot 12), maximum distance (slot 14; 0 is replaced by 100000.0, the floats at `0x00A81054`
and `0x00AD3F18`), the position `0xFFCC4D2D/0xFF4A01D6/0xFF469820` (slot 5) and the second vector
`0xFF444C9A/0xFFC20061/0xFFCE9997` (slot 7). Both: `vol := 0xFF90AAC8 × 0xFF53DBF3 / 0x400`, kept at `+0x1C` and sent
to the sound (slot 21).

`UpdatePitch` (`0x0093F07E`): with a loaded sound, re-sends `0xFF3071B9` when set; when `0xFF099DB2` is not set
locally it is initialised from the sound's frequency (sound slot 26), otherwise sent to the sound (slot 27).

`HandleNotifySetPropertyValue` (`0x0093FA5B`) *[compat]*:

| Property | Effect |
|---|---|
| `0xFF099DB2`, `0xFF3071B9` | `UpdatePitch` |
| `0xFF3EC880`, `0xFF444C9A`, `0xFFC20061`, `0xFFCE9997`, `0xFFCC4D2D`, `0xFF4A01D6`, `0xFF469820`, `0xFF53DBF3`, `0xFF90AAC8`, `0xFFA6A614`, `0xFFAAB623` | `UpdateVolPan` |
| `0xFF4C97BE` | rebuild the parent chain from (`0xEBFEE345`, `0xFC45CAF7`, parent) lookups, as §5.3 |
| `0xFFB36659` | non-zero: pause the sound (slot 17, `1`); zero: resume (slot 18) |
| `0xFFAEC905` | re-attach (interface `+0x14`, slot 3, `(0, 2)`) |
| `0xFF0AD592` | unless the channel is fading (slot 43), write it back to 0 |
| `0xFF86FE79`, `0xFF0150F2`, `0xFF734343`, `0xFFD3D39B` (floats); `0xFFBDB196`, `0xFFA42951` (integers) | the channel's slots 53, 47, 49, 50, 51, 52 |
| other | nothing |

---

## 10. Sems *[compat]*

A **sem** maps a track's registers onto its sound: `cHitSem` (QI `0xDDCF3BD7`) has an id (`+0x1C`) and a name
(`+0x08`). The HitMan holds two built-in ones:

| Sem | Id | Name | Ctor |
|---|---|---|---|
| `cHitSem2dDefault` | `0x7DCDED2D` | "2D default" | `0x00939623` |
| `cHitSem3dDefault` | `0x7DCDED3D` | "3D default" | `0x00939690` |

Each has an **apply** (source = the track's registers, target = the channel) and a **setup**:

- 2D apply (`0x00944088`): `vol := reg[0x14] × reg[0x15] / 0x400` (gain × volume); if the HitMan's duck level
  (global `0xFFAE9DDB`) is non-zero and the source's own `0xFFAE9DDB` is below it, `vol /= 2`; target pan `0x16` :=
  source pan; target volume `0x15` := `vol`.
- 2D setup (`0x00944117`): copies `0x13`, `0x19`, `0x1A`, `0x1B`, `0x23`–`0x26`, `0x33`.
- 3D apply (`0x009441E3`): copies `0x20`–`0x22` (position) and `0x40`–`0x42`; distances: if `0x1F` is 0 *[quirk:
  tested twice; `0x1E` is never tested]*, `0x1E := reg[0x38]` (or 4 when that is 0) and `0x1F := 0x1E × 500`;
  otherwise both are copied; then the volume as in 2D.
- 3D setup (`0x00944351`): target `0x1C := 1` (3D), copies `0x1D`, `0x13`, `0x19`–`0x1B`, `0x23`–`0x26`, `0x33`,
  `0x1E`, `0x1F`.

Register numbers are the aliases of §7.1 (`0x14` gain, `0x15` volume, `0x16` pan, `0x1E`/`0x1F` minimum/maximum
distance, `0x38` = `0xFFCC3B29`).

---

## 11. The creators, the control group and the debug service

### 11.1 Segment track creator

`cHitSegmentTrackCreator::Create(key…)` (`0x009359E5`): for each group in its list, look up (`0x0B8D821A`, group, id),
then (`0x0B9EB87E`, group, id) under `0xEBB10CDD`; at the first hit, `HitMan.CreateTrackPlayer(0xBB1BE367)`, give it
the definition (slot 7, `SetTrackDef`), call slot 12, then return its `QueryInterface` for the caller's IID.

### 11.2 Segment data-source channel creator

`cHitSegmentDataSourceChannelCreator::Create` (CLSID `0x2B2EB869`): the sound manager (slot 24) builds a data source
(IID `0x3AFD825F`) for the key; a `cHitChannel` is created (`0x3B1EA5BE`, IID `0x09D002D2`) and given the source
(slot 3, `(source, 2)`); its register set (`0xFD584467`) gets `0xFF4C97BE := 0xCB954E2C` (the default channel).

### 11.3 Control group

The control group object (§3.5, QI `0xFB1C0050`) holds a set of tracks (`AddTrack`). It is consulted when a track
starts (§6.3) *[inferred: muting]*.

### 11.4 `cHitDebugInfo`

A development service (ID `0xFBC9CD04`) mapping track data back to source lines. `LoadDebugInfo(path, …)`
(`0x0093A588`) reads a text file next to the `.hit` (`SOURCEFILE <name>  {line,offset}` lines start a source file;
`{line,offset}` lines add a mapping); `LoadSymbolFile` reads `"%s %ld ("` lines and discards them;
`GetDataAddressFromEntryPoint`, `GetHitFilenameFromEntryPoint`, `GetFilenameAndLineNumFromDataAddress` and
`GetDataAddressFromFilenameAndLineNum` answer from the HitMan's blocks and the loaded files. `ParseHitFile` reports
every loaded `.hit` to it (§3.2). Nothing a player hears depends on it *[inferred]*.

---

## 12. Quirks *[quirk]*

1. Opcode `0x28` (`And`) computes `Greater` (§8.2).
2. Opcode `0x19`'s error string names `kTrackCmdRand`; the opcode is abs.
3. `NoteOn` ignores its note argument (§6.5).
4. The `'TRAX'` scan in `ParseHitFile` is discarded (§3.2).
5. Load resolves relative `.hit` names against the application directory, Unload against the data directory; Load's
   duplicate test does not normalise the path (§3.2).
6. `FilepathHasEmbeddedKey`'s runs overlap by one character (§3.3).
7. The default track holds nine `0 = 0` properties; the default channel sets `0xFFAAB623` twice (§5.3, §9.1).
8. The 3D sem tests the maximum distance twice (§10).
9. `UpdatePitch` compounds its random spread (§6.7).
10. A parent cycle in TKDT records never ends (§5.3, inferred).
11. The "Too many instructions" error repeats on every instruction after the 100th in a tick and never stops the loop
    (§6.4).

---

## 13. Open items

Questions are filed in [questions/hit-music.md](questions/hit-music.md) (prefix **HM**).

---

## 14. Conformance tests

1. **Opcode table.** Byte code `05 04 05 00 00 00 05 05 03 00 00 00 10 04 05 0C` leaves register 4 = 8 and ends the
   track.
2. **And is Greater.** `05 04 02 00 00 00 05 05 01 00 00 00 28 04 05` → register 4 = 1; with 1 and 2 → 0.
3. **Max/Min.** `05 04 09 00 00 00 2D 04 05 00 00 00` → 5; `2E 04 07 00 00 00` on 5 → 7.
4. **Wait.** `04 04 32 09 04 0C` ends after 50 ms of ticks (with 20 ms ticks, on the third); the wait counts down by
   each tick's `dt`.
5. **Illegal register.** An operand `0x4B` reports "Illegal Register Id" and uses register 1.
6. **Hit list text.** A 14-byte record `0A 00 00 00` + `1,5-7,0x10` reads as {1, 5, 6, 7, 16} with window
   5/3 + 1 = 2; the same text with a first dword of 11 fails.
7. **Shuffle bag.** With {1, 2, 3} and window 2, any three consecutive `Choose` results are all different; with
   window 1 only two consecutive results are guaranteed to differ.
8. **TKDT read.** The [audio.md](audio.md) §10.10 sample sets `0xFFAEC905 = 0xEC08D88A`, `0xFF5F15E1 = 0x09400FC5`,
   `0xFFCC3B29 = 100`, `0xFF891EAA = 0x7DCDED2D`, `0xFFF7B8DF = 0`, `0xFF90AAC8 = 1024`, `0xFF595FFD = 3000`,
   `0xFF53DBF3 = 768`, and leaves the empty fields inherited from `0xAB903A1B`.
9. **Recursive sound.** A TKDT whose field 1 equals its own instance fails to read.
10. **Alias.** Register 21 reads and writes `0xFF90AAC8`; register 13 is the raw id 13.
11. **Fade-in.** A track with gain `0x400` and fade-in 1000 ms starts at gain 0 and reaches `0x400` after 1 s (rate
    1024/s, clamped).
12. **Duck.** With a sound ducked at priority `0x40`, a track whose `0xFFAE9DDB` is `0x20` plays at half volume; at
    `0x40` or above at full volume.
13. **Pitch spread.** `0xFF8DCFD8 = −12`, `0xFF69510B = 12` on a 22050 Hz sound gives a frequency in [11025, 44100).
