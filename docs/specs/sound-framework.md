# The framework sound system: `cGZSndSys`, `cGZSndMgr` and the sound director's classes (behaviour spec)

Status: M6 behaviour spec, written 2026-10-22 (re-sndsys lane, outside contributor). Facts were read on the Windows
executable `bin/SimCity4.exe` (1.1.641, image base `0x00400000`) from the Ghidra export; `cGZSndMgr`, whose export
fails for every method ([audio.md](audio.md) §8.6), was read on the instruction bytes. Addresses are Windows
addresses. Implementers work from this spec, the LGPL gzcom-dll SDK headers and general knowledge only, never from
decompiled code.

Tags: *[compat]* observable, must match; *[inferred]* consistent with the evidence, not proven; *[quirk]* original
behaviour reproduced as is; *[as read]* read from the export without a byte check.

Scope. The GZ framework's sound layer that every SC4 sound goes through: the sound system (`cGZSndSys`: the sound
thread, the event queue and handlers, the update targets, the renderers and the 3D listener, the extension map), the
sound manager (`cGZSndMgr`: sound and data-source registries, creators, auto-play), and the helper classes of
`cGZSoundCOMDirector` (data-source creator, property and instance caches, the default event handler, the segment
data sources). Not here: the sound-event object and its pool ([audio.md](audio.md) §3–§4), the EA SNDSYS renderer
and codecs (the EA-library spec), the SC4 managers ([audio.md](audio.md) §5–§9).

Relation to [sound-system.md](sound-system.md). That spec is the interface contract for `cGZSndSys`, `cGZSndMgr`
and the directors (its §7–§8); this one adds what was read below it on the bytes: the sound thread and queue, the
target periods, the manager's registries, the helper classes and the segment data sources. Where the two disagree,
the instruction evidence cited here was re-checked on 2026-10-23: `PreAppShutdown` calls slot 34 (`DisableUpdates`,
`call [edx+0x88]` at `0x00966551` on the `cIGZSndSys` pointer from `RZ::SS`), not `Panic` (slot 47); the table tail
is 48 `SetTestMode`, 49 `GetTestMode`, 50 `GetSoundSystemThreadId` (`0x00AD7240`–`0x00AD7248`).

---

## 1. The director *[compat]*

`cGZSoundCOMDirector` (director ID `0xE9D3A40B`, constructor `0x00966498`) registers:

| CLSID | Factory | Object | Size |
|---|---|---|---|
| `0xE41CE9FD` | `cGZSndSys::Create` (`0x009663DF`) | the sound system (§2) | 0x128 |
| `0x7AF85B98` | `cGZSndMgr::Create` (`0x009663F8`) | the sound manager (§3) | 0xA8 |
| `0xDC1F73D6` | `0x00966411` | the segment data-source creator (§4.1) | 0x10 |
| `0x1D07EB4B` | `0x0096647F` | the XA file data source ([audio.md](audio.md) §10.3) | 0xF84 |
| `0xBB7051F5` | `0x00966469` | the WAV file data source (§4.4) | 100 |
| `0x9D4C6B95` | `0x00966427` | the sound property cache (§4.2) | 0x18 |
| `0x7DC495CC` | `0x00966453` | `cGZSndDefaultEventHandler` (§4.3) | 0x10 |
| `0xBDF613B4` | `0x0096643D` | the sound instance cache (§4.2) | 0x14 |

`OnStart` (`0x0096655A`): adds the director to the framework (slot 7); creates the sound system and the sound
manager under IID `0x287FB697` and adds both as system services; creates the default event handler under IID
`0xBB523BB2` and registers it with the sound system (slot 15). `PostAppInit` (`0x00966678`) creates the global
property set (`0xCC3014EE`) in the property cache. `PreAppShutdown` (`0x0096653F`) disables the sound system's
updates (slot 34).

---

## 2. The sound system (`cGZSndSys`)

### 2.1 Identity

0x128 bytes (constructor `0x00968773`). `QueryInterface` (`0x00966723`): `1` and `0x106077D2` (`cIGZSndSys`) →
base; `0x287FB697` → `+0x04` (system service); `0x452294AA` → `+0x08` (message target); `0x406077D2` → `+0x0C` (3D
listener); `0x5B30E7B3` → `+0x10` (extended listener); `0xAB79E31C` → `+0x14` (renderer broadcast). The service ID
kept at `+0x38` is `0x23CECBA0`; master volume `+0xE0` = `0x400`; enabled `+0x3C` = 1; updates enabled `+0x68` = 1.
The data directory is set from the file system at construction (`SetDataDirectory` `0x009667B3`).

The `cIGZSndSys` table (`0x00AD7180`): 5–7 dispatch-now helpers, 8 `DoEvent(e)`, 9–11 post helpers, 12
`PostEvent(e)`, 13/14 `EventCreate`, 15/16 event handlers, 17 `Update`, 18 thread-running flag, 19/20 data
directory, 21/22 enabled flag, 23–27 extension map, 28/29 master volume, 30/31 targets, 32 `EnumerateTargets`, 33/34
enable/disable updates, 35–38 renderers, 39 `FindPlayingGZSnd`, 40 `IsSndPlaying`, 41 lock, 42/43 property cache,
44/45 instance cache, 46 `AddToMainlineReleaseList`, 47 `Panic`, 48/49 test mode, 50 sound-thread id.

### 2.2 Threads and the event queue *[compat]*

`Init` (`0x00967FE4`): sets the data directory when missing; subscribes the `+0x04` interface to the message server
for message type `0xDB53383B` (`kSndEventMsgTypeStd`); when threaded and not yet set up: creates the event queue
(`cGZMessageQueue2`, `+0x30`), the property cache (`0x9D4C6B95`, IID `0x5D4C6B90`) and the instance cache
(`0xBDF613B4`, IID `0x3DF613A1`), starts its timers, and creates a **callback timer** (`+0x58`) with a **20 ms**
period whose callback (`0x009666F4`) calls `Update(dt)` with the milliseconds since the previous call (0 when the
clock went back). *[compat]* The timer's slot 12 returns its scheduler, whose
slot 5 forwards to its embedded worker thread's priority setter (`0x00968164`,
`0x009D8564`, scheduler table `0x00AD1F6C`, `0x00933653`). Priority 4 maps to
Windows thread priority +1 (above normal), through `SetThreadPriority`; it does
not change timer resolution (`0x0092064A`, thread table `0x00ACFB90`). If the
worker has not been created, the setter retains that priority for creation (SY1).

- `PostEvent(e)` (`0x00966A95`) pushes the event's message on the queue. The helpers `0x00966AB2`, `0x00966B55`,
  `0x00966C15` create a `cGZSndEventStd` (`EventCreate`, slot 13, IID `0x5B523BA4`), fill data1–data4 (payload slots
  11, 16, 21, 26), void1 (slot 33) or a single extra value (slot 30), and post it.
- `DoEvent` variants (`0x00966C82`, `0x00966D3B`, `0x00966DBE`) do the same but dispatch at once under the system
  lock (`+0x60`).
- `Update(dt)` (`0x00967B2E`, under the lock): when the sound thread runs (slot 18, set by `Init`), pop every queued message and hand it to the
  `+0x08` interface (slot 3), which accepts message type `0xDB53383B`, takes its `0x5B523BA4` event and, when there are
  handlers, dispatches it; then update the targets (§2.3) with `dt`.
- `DoEvent(e)` (`0x009679AD`): offer the event to each registered **event handler** (slot 3) in order; the first that
  returns true stops it. Handlers registered or removed during dispatch are applied afterwards
  (`RegisterEventHandler` slot 15, `UnregisterEventHandler` slot 16). The main-line release list is emptied after each
  dispatch (`AddToMainlineReleaseList` `0x00967D61` defers releases to it).

### 2.3 Update targets *[compat]*

Two lists: **fast** targets (`+0x6C`), updated on every `Update`, and **slow** targets (`+0x70`), updated when more
than **250 ms** have passed since the last slow round. `RegisterTarget(t, period)` (slot 30, `0x00967DD4`) and
`UnregisterTarget(t)` (slot 31, `0x00967E4F`, ignored during shutdown): changes made while the lists are being walked
are queued and applied at the end of the round (removals first, then additions with period 20, then with period
250). `EnumerateTargets(f, a, b)` (slot 32, `0x00967641`) calls `f(target, a, b)` over both lists until it returns
false (used to find, stop or kill sounds by id: `FindPlayingGZSnd` `0x009669BE`, `IsSndPlaying` `0x009669F3`).
`EnableUpdates`/`DisableUpdates` gate `UpdateTargets`.

`Panic` (`0x00967BA1`): collects every target that is a sound (`0x206077D2`) and kills each (slot 9).

### 2.4 Renderers and the listener

Renderers (`+0x90`): `RegisterRenderer` (no duplicates), `UnregisterRenderer`, `IsRegisteredRenderer`,
`NumRenderers`. `SetMasterVolume` sends `+0xE0` to every renderer (slot 3). The listener state is pushed to every
renderer's 3D interfaces: position (`0x406077D2` slot 7), orientation (6 floats, slot 9), velocity (`0x5B30E7B3`
slot 4) and three factors on the extended listener (slots 8, 10, 12 — Doppler, roll-off
and distance, respectively); `CommitDeferredSettings`
calls `0x406077D2` slot 5 on each. Getters return the stored copies.

**Factor mapping (SY2).** *[compat]* The framework setters are `0x0096742C`
(Doppler), `0x009674BD` (roll-off) and `0x0096754E` (distance). They forward to
extended-listener slots 8, 10 and 12. At the DirectSound helper, these reach
SetDopplerFactor (`0x00878A94`), SetRolloffFactor (`0x00878B03`) and
SetDistanceFactor (`0x00878AB9`), with immediate application. The helper also
caches the distance factor for distance conversion; its extended-listener slot 11
returns that cached value (`0x00878CD9`, table `0x00AC2F3C`).

### 2.5 Extension map

`SetExtensionDefaultDataSourceClassId(ext, clsid)` (`0x009689E9`): lower-case, trim and drop one leading `.`; an
empty result fails; otherwise `map[ext] = clsid`. `GetExtensionDefaultDataSourceClassId(ext)` normalises the same
way. The map is filled from the Hit INI ([hit-music.md](hit-music.md) §2 step 2; [audio.md](audio.md) §10.3 lists the
shipped mapping).

### 2.6 `Shutdown` (`0x009683EF`)

Marks shutting down, stops and deletes the callback timer and the timers, unsubscribes `0xDB53383B`, clears the
handler lists, kills every target that is a sound, clears every list and the extension map, and drops the caches.

---

## 3. The sound manager (`cGZSndMgr`)

### 3.1 Identity *[compat]*

0xA8 bytes (constructor `0x00969AF1`): a `cRZSystemService` with service ID **`0x1AF85BFB`** and priority
`0x7A11E`; `cIGZSndMgr` (IID `0x3AF85BB7`) at `+0x1C` (vtable `0x00AD7300`); an update target at `+0x20`. Fields
(base offsets): `+0x24` initialised, `+0x28` the sound system, `+0x2C` the resource manager, `+0x30` = `0x400`, `+0x34`
registered sounds (hash id → sound, 100 buckets), `+0x4C` system-sound table (hash index → id), `+0x60` the lock (a
heap `cRZCriticalSection`), `+0x64` sound creators (list), `+0x6C` the auto-play list, `+0x74` data-source creators
(map id → creator), `+0x84` registered data sources (map id → source), `+0x94` a set, `+0xA0`/`+0xA4` the default 2D
and 3D data-source player class IDs.

### 3.2 Methods *[compat]*

| Method | Address | Behaviour |
|---|---|---|
| `Init` | `0x00968BA7` | once: keep the sound system and register the `+0x20` target with it, period **250** (slot 30); keep the resource manager |
| `GetSound(id, a, b, iid, out)` | `0x009693C0` | fails when the sound system reports no renderer (its slot 38); a sound registered under `id` → its `QueryInterface(iid)`; otherwise each creator in list order (slot 3 with all five arguments) until one succeeds |
| `GetSound(name, a, b, iid, out)` | `0x00968AE7` | `id := CRC24(name) \| 0xFF000000` (CRC24 as in [advisors.md](advisors.md) §4.2), then the id form |
| `RegisterSnd(snd, id)` / `UnregisterSnd(id)` / `UnregisterSnd(snd)` | `0x009699BA` / `0x00969710` / `0x00969752` | the registry keeps the first sound per id (a second register is ignored) |
| `RegisterSndCreator(c)` | `0x00969335` | `c` must answer `0x3AF9866F`; removed first, then inserted at the **front** (newest creator tried first) |
| `RemoveSndCreator(c)` | `0x00968D08` | |
| `RegisterSndDataSourceCreator(c, id)` | `0x00969A31` | `c` must answer `0x9B75709E`; the old creator for `id` is removed; `map[id] := c` |
| `RemoveSndDataSourceCreator(id)` / `(c)` | `0x00968FB0` / `0x00968F4F` | |
| `GetSndDataSourceCreator(id, iid, out)` | `0x00968FFE` | |
| `RegisterSndDataSource(src, id)` / `UnregisterSndDataSource` | `0x009697B7` / `0x0096958D`, `0x009695CB` | first registration wins |
| `GetSndDataSource(id, a, b, iid, out)` | `0x00969055` | a registered source → `QueryInterface(iid)`; otherwise each data-source creator in id order (slot 3) until one succeeds |
| `SetSystemSndID(i, id)` / `SystemSndID(i)` | `0x0096995C` / `0x009692FC` | table of numbered system sounds; unknown → 0 |
| `SndAutoPlay(id, vol)` | `0x009694A2` | the registered sound, or `GetSound(id, 0, 0, 0x206077D2)`; keep it in the auto-play list; play it (slot 6, `(vol, 0, 1)`) |
| `SystemSndAutoPlay(i, vol)` | `0x00969536` | `SndAutoPlay(SystemSndID(i), vol)` |
| `SndAutoPlay(id)` / `SystemSndAutoPlay(i)` | `0x00968B1D` / `0x00968B2C` | the same at the system-sound volume (`SetSystemSndVol` `0x00893D87`) |
| default player class IDs | `0x00968B67`–`0x00968B96` | plain get/set |
| `GetResource(key, iid, out)` | `0x00968C18` | resource manager slot 3 |
| `Shutdown` | `0x0096966C` | clears every registry and list; initialised := 0 |

The update target (`0x00968F11`) drops the front of the auto-play list once that sound no longer plays.

**Answers** [audio.md](audio.md) **AU9**: the `0xFF000000` tag in the name form of `GetSound` marks an id computed
from a name — `CRC24(name)` with the top byte set — the same rule the advisors and the freshness external path use.

---

## 4. The director's helper classes

### 4.1 The segment data-source creator (`0xDC1F73D6`)

0x10 bytes (`0x00969E08`): a list of resource groups and the resource manager. QI `0x9B75709E`. Each INI entry adds a
group (`0x00969EA3`, the value read with `RZConvertToUint32A`). `Create(id, a, b, iid, out)` (`0x00969BF2`)
*[compat]*:

1. find a group whose resource (`0x2026960B`, group, id) exists (resource manager slot 32); none → fail;
2. try, in order, until one opens (slot 17): the segment XA source (`0x0096C5AA`, 0xFEC bytes), the UTalk source
   (`0x0096C152`, 0xDA0 bytes, §4.4), the MP3 source (GZCOM `0x3CEC2B47`, IID `0x3AFD825F`, given the key), the WAV
   source (`0x0096BD94`, 0x48 bytes, §4.4); none → fail;
3. return the source's `QueryInterface(iid)`.

### 4.2 The property cache (`0x9D4C6B95`) and the instance cache (`0xBDF613B4`)

The **property cache** (0x18 bytes, QI `0x5D4C6B90`) keeps one property set per sound instance in the resource
manager under (`0xEBFEE345`, `0xAC31543D`, instance); `0xCC3014EE` is the **global** set. Creating an instance set
(`0x0096A3DE`) makes and registers the resource; get/set/has/remove go through that resource's `0xEBB10CDD`
interface. `SetGlobalProperty` (`0x00969F64`), `RemoveGlobalProperty` (`0x00969FBE`) and `RemoveInstanceProperty`
(`0x00969F31`) also post the events `0xAA232CF8`, `0xCA232CFF` and `0xCA232CF2` through the sound system (cached at
`0x00B63604`). The sound system returns the cache from its slot 42.

The **instance cache** (0x14 bytes, QI `0x3DF613A1`) maps a key to a list of ids and answers which key a given id
belongs to (`0x0096A53F`) *[as read]*.

**Established cache operations (SY3, partial).** *[compat]* The instance cache's
slot 6 ensures a list exists for the supplied key, then appends an ID only when
absent (`0x0096A760`, table `0x00AD7440`). Slot 7 removes the first ID in that
key's list (`0x0096A6C2`); slot 10 removes the requested ID once from each list
(`0x0096A6F5`). Slot 11 scans keys in map order and returns the first key whose
list contains the requested ID (`0x0096A53F`). These are integer keys and values;
the container bodies do not establish that they are sound IDs or sound-instance
IDs. The caller that assigns those meanings remains unresolved.

### 4.3 The default event handler (`cGZSndDefaultEventHandler`, `0x7DC495CC`)

QI `0xBB523BB2`; holds the sound manager (`+0x08`) and the sound system (`+0x0C`). `DoEvent` (`0x0096A972`) reads the
event's payload (`0x65297976`) and *[compat]*:

| Event | Effect |
|---|---|
| 1 | `GetSound(data1)` and play it (slot 6, `(0x400, 0, 1)`); returns true |
| 2 | stop the sound `data1` (slot 8) |
| 3 | nothing |
| 4 | kill the sound `data1` (slot 9) |
| `0x0A35540C` | property cache slot 10 with (data2, data1) |
| `0x0A355411` | property cache slot 11 with (data2, data1) |
| `0x0A355417` | property cache slot 15 with data1 |
| `0xCA3553D6` | property cache slot 3 with data1 (create an instance set) |
| `0xCA3553E7` | property cache slot 4 with data1 |
| `0xEA355407` | property cache slot 6 with (data3, data2, data1) |

Every case other than a successful event 1 returns false, so later handlers still see the event.

### 4.4 Segment data sources

- **WAV in a segment** (`cGZSndSegmentWavDataSource`, `Open` `0x0096BE03`): opens the record; `FindChunk(tag)`
  (`0x0096BD3E`) reads **dword by dword** until one equals `tag`, then reads the size *[quirk: chunk bodies are not
  skipped, so a matching dword inside another chunk is taken]*; `fmt ` (`0x20746D66`) is read into the format block,
  `data` (`0x61746164`) gives the data size.
- **UTalk** (`cGZSndSegmentDataSourceUTalk::OpenRecords`, `0x0096C1E1`): the record must start with `UTM0`
  (`0x304D5455`), then a `u32`, then a header size that must be **0x14**, followed by that 20-byte header; the data
  size is `recordSize − 0x14 − 12`.
- **WAV file** (`0xBB7051F5`, 100 bytes; `Open` `0x0096AE6B`): `mmioOpen`; the file must be `RIFF`/`WAVE`; the `fmt `
  chunk must be at least 16 bytes with format tag **1** (PCM only); then the `data` chunk. Errors kept at `+0x58`: 4
  open or read failed, 3 not a PCM WAVE, 5 a chunk missing. Reads (`0x0096AFCA`) copy from the mmio buffer, never
  past the data chunk.

### 4.5 The XA file data source (`0x1D07EB4B`) *[compat]*

0xF84 bytes (`0x0096B1F5`). **Open** (`0x0096B078`): the file (`CreateFileA`, read, share-read, open-existing)
starts with an 8-byte header — `"XA"`, two opaque bytes, a `u32` **decoded size** in bytes — then a 16-byte PCM format block
(`wFormatTag` must be 1, channels, rate, byte rate, block align, bits). Errors at `+0xF7C`: 1 open failed, 3 not XA
or not PCM, 4 read failed.

The two bytes at file offsets 2–3 are not validated by Open: only the first two
signature bytes are tested (`0x0096B078`). *[compat]* Accept any values there and
obtain channel count from the following PCM format block. Their intended format
meaning (version or padding, for example) remains unknown (SY4).

**Decoding** (`0x0096B28C`): the compressed stream is read 0xF00 bytes at a time (`0x0096B4DE`). It is made of
frames of 28 samples per channel: mono frames are 15 bytes (1 header byte + 14 data bytes), stereo frames 30 bytes
(2 header bytes, left then right, then 14 byte pairs, left byte first). Each header byte holds a coefficient index
(high nibble) and a shift (low nibble); each data byte holds two samples, high nibble first. Per channel, with the
two previous outputs `p1` (latest) and `p2`:

```
s = floor(p1 * K1[c] + p2 * K2[c] + N[shift][nibble])      -- single-precision floats
K1 = { 0, 0.9375, 1.796875, 1.53125 }   K2 = { 0, 0, -0.8125, -0.859375 }     (0x00B170E8, 0x00B170F8)
N[shift][n] = signed4(n) << (12 - shift) for shift 0..12, and 0 for shift 13..15          (0x00B17108)
p2 = p1; p1 = s; output (int16) s
```

`floor` is computed with the `(x − 0.5) + 12582912.0` bit trick and a correction when `x` is an integer
(`0x00A84D2C` = 0.5, `0x00AD74A8` = 12582912.0). *[quirk]* There is **no clamping**: the history keeps the full
value and the output is its low 16 bits, so an overflowing sample wraps. A coefficient index above 3 reads past `K1`
into the next table. A request delivers at most the remaining decoded size, in steps of one sample frame (4 bytes
mono = 2 samples, 8 bytes stereo).

The segment XA source (`0x0096C5AA`) reads the same stream from a DB record.

### 4.6 `cGZSnd` and `cGZSnd3D`

The base sound class answers `1`/`0x206077D2` (base), `0x49D28669` (`+4`), `0x3B143F0A` (`+8`), `0x5B34F0AF`
(`+0xC`), `0x09D002D2` (`+0x10`), `0xE9D00A9A` (`+0x14`) (`0x00876028`); `cGZSnd3D` adds `0x306077D2` at `+0xD8`
(`0x008781B7`).

---

## 5. Quirks *[quirk]*

1. The segment WAV reader finds chunks by scanning every dword (§4.4).
2. `RegisterSndCreator` puts the newest creator first; `RegisterSnd` keeps the oldest sound per id (§3.2).
3. The default event handler returns false for everything except a successful play (§4.3).
4. The XA decoder does not clamp; out-of-range coefficient indices read the neighbouring table (§4.5).

## 6. Open items

Questions are filed in [questions/sound-framework.md](questions/sound-framework.md) (prefix **SY**); the
interface-level gaps are in [questions/sound-system.md](questions/sound-system.md) (prefix **SS**).

## 7. Conformance tests

1. **Name ids.** `GetSound("abc", …)` asks for id `CRC24("abc") | 0xFF000000`.
2. **Creator order.** With creators A then B registered, `GetSound` of an unregistered id asks B first.
3. **Slow targets.** A target registered with period 250 is updated at most once per 250 ms of `Update` calls;
   one with period 20 on every call.
4. **Extension map.** `SetExtensionDefaultDataSourceClassId(" .WAV ", x)` then `Get("wav")` returns `x`; `"."`
   fails.
5. **Handlers.** With two handlers where the first returns true, the second never sees the event.
6. **UTalk header.** A record `UTM0`, a `u32`, `0x14`, 20 bytes, then 100 bytes of data opens with data size 100.
7. **Default handler.** Event 4 with data1 = a playing sound's id kills it and returns false.
8. **XA.** A mono frame `0x1C` + 14 bytes `0x70 …` (coefficient 1, shift 12) from silence decodes its first two
   samples as `floor(0 + 7) = 7` and `floor(7 × 0.9375 + 0) = 6`.
