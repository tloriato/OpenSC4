# Audio scapes: `cSC4AudioScapeManager` and the fire handler (behaviour spec)

Status: M6 behaviour spec, written 2026-10-23 (re-scape7 lane, outside contributor). Facts were read on the Windows
executable `bin/SimCity4.exe` (1.1.641, image base `0x00400000`), from the Ghidra export and, for every slot number,
offset, constant and branch below, on the instruction bytes. Addresses are Windows addresses. Implementers work from
this spec, the LGPL gzcom-dll SDK headers and general knowledge only, never from decompiled code.

Tags: *[compat]* observable, must match; *[inferred]* consistent with the evidence, not proven; *[quirk]* original
behaviour reproduced as is.

Scope. The **audio scape manager** groups many point sources of one kind of sound (fires, construction sites) into
**clusters** of nearby sources, plays each kind's sounds once per cluster instead of once per source, and chooses
which sound a cluster plays from how many sources it holds. Also `cSC4AudioEventHandlerDisasterFire`, its simplest
client (§9). The other clients are the audio event handler, whose calls are [audio.md](audio.md) §7.3 (slots 13–15
here), and the ambience manager, which fetches the service but calls no slot ([audio-ambience.md](audio-ambience.md)
§2.1). The sound events it posts are executed by the SC4 sound manager ([audio.md](audio.md) §8.3).

---

## 1. Identity *[compat]*

| Item | Value |
|---|---|
| CLSID | `0x29E2797F`; factory `Create` `0x004EB120`: allocates 0x98 bytes, constructs (`0x004F6140`), returns the `+0x04` interface |
| service ID | `0x49E50BA1` (`kAudioScapeManagerSystemServiceID`, stored at `+0x80`) |
| `QueryInterface` `0x004F5210` | `1` and `0x29E39DB4` (`cISC4AudioScapeManager`) → the object; `0x287FB697` (system service) → `+0x04`; anything else → false |
| vtables | `+0x00` `0x00A94340` (19 slots, §1.1); `+0x04` `0x00A94308` (system service, §2.3); `+0x08` `0x00A942F8` (3 + 1 slots, §10) |

### 1.1 The primary table (`0x00A94340`)

| Slot | Address | Operation |
|---|---|---|
| 0–2 | `0x004F5210`, `0x00524750`, `0x004F5270` | QueryInterface, AddRef, Release (§2.4) |
| 3 | `0x004F53E0` | `Init` (§2.1) |
| 4 | `0x004F5700` | `Shutdown` (§2.2) |
| 5 | `0x004F64F0` | `CreateScape(name, id, flags, radius)` (§4.1) |
| 6 | `0x004F6A00` | `RemoveScape(id)` (§4.2) |
| 7 | `0x004F65C0` | `ClearScape(id)` (§4.3) |
| 8 | `0x004F6620` | `ClearScapesWithFlags(mask, keepClusters)` (§4.3) |
| 9 | `0x004F7850` | `GetNearClusterPositions(idA, idB, rect, out, &count, &total, useRect)` (§7.3) |
| 10 | `0x004F7AF0` | `GetClusterGrid(id, rect, out, &count, &total, useRect)` (§7.2) |
| 11 | `0x004F57B0` | `GetClusterPositions(id, out, &count, &total)` (§7.1) |
| 12 | `0x004F7300` | `AddEmanation(id, soundId, flag, key, pos, f)` (§5.1) |
| 13 | `0x004F51E0` | `AddEmanation(id, key, pos, f)`: slot 12 with `soundId` 0 and `flag` 0 |
| 14 | `0x004F6B30` | `RemoveEmanation(id, key)` (§5.4) |
| 15 | `0x004F7510` | `MoveEmanation(id, key, pos, radius)` (§5.5) |
| 16 | `0x004F5E30` | `AddSound(id, soundId, flag, extra, min, max)` (§4.4) |
| 17 | `0x004F5A20` | `RemoveSound(id, soundId)` (§4.4) |
| 18 | `0x004F69E0` | deleting destructor (§2.4) |

Primary data operations in slots 5–12 and 14–17 take the scape lock (§3.1); slot 13
delegates to slot 12. QI/AddRef/Release do not take that data-operation lock.

## 2. Lifetime *[compat]*

### 2.1 `Init` (`0x004F53E0`)

Already initialised (`+0x14`) → true. Otherwise:
1. Get the sound system (service `0x23CECBA0`, IID `0x106077D2`). Absent → false. Present → keep it at `+0x0C` and in
   the process global `0x00B21AEC`, the pointer every post of §6 uses.
2. Get the framework sound manager (service `0x1AF85BFB`, IID `0x3AF85BB7`) into `+0x10`. Absent → false; the sound
   system stays held. `+0x10` is never used afterwards *[inferred: no read found]*.
3. With the framework: `AddSystemService(+0x04)` (framework slot 3) and `AddToTick(+0x04)` (slot 9); start the timer
   `+0x20`; `+0x14` := 1; true.

### 2.2 `Shutdown` (`0x004F5700`)

Not initialised → true. Otherwise, under the lock: `RemoveScape` (slot 6) of the first scape until no scape is left;
`RemoveFromTick(+0x04)` (framework slot 10) and `RemoveSystemService(+0x04)` (slot 4); release `+0x0C`, `+0x10` and
the global `0x00B21AEC` (each set to null); `+0x14` := 0; true.

### 2.3 The system-service interface (`+0x04`, `0x00A94308`)

Slots 3/4 get and set the service ID (`+0x80`); 5 the priority, `0x80000000` (`0x004F6230`); 6/7 get and set the
running byte `+0x84`; 8/9 forward to `Init`/`Shutdown`; 10 `OnTick` (`0x004F67C0`, §6.2); 11 `OnIdle` returns false;
12 the tick priority, which forwards to slot 5.

### 2.4 Reference count and destruction

The count is `+0x18`. `Release` (`0x004F5270`): above 1 → decrement and return the new count; otherwise call slot 18
with 1 and return 0. The destructor (`0x004F62C0`) restores the three vtables, frees the cell buffer `+0x88`, destroys
the three lists, the three critical sections and the timer, and releases `+0x10` and `+0x0C`. It frees list nodes
only: scapes still listed are leaked *[quirk]*. Normally `Shutdown` has removed them.

## 3. State *[compat]*

### 3.1 The manager (0x98 bytes)

| Offset | Field |
|---|---|
| `+0x0C` | the sound system (§2.1) |
| `+0x10` | the framework sound manager |
| `+0x14` | initialised (byte) |
| `+0x18` | reference count |
| `+0x20` | timer (milliseconds), started by `Init` |
| `+0x38` | the timer reading at the previous tick |
| `+0x3C`, `+0x44`, `+0x4C` | three critical sections: `+0x44` is the **scape lock**, `+0x4C` guards the pending list, `+0x3C` is never entered |
| `+0x54` | the scape list (nodes `{next, prev, scape}`) |
| `+0x58` | the pending-cluster list (§6.2); nothing adds to it |
| `+0x5C` | a third list, unused |
| `+0x60`–`+0x74` | six floats, zero, unused |
| `+0x80` | service ID `0x49E50BA1`; `+0x84` the running byte |
| `+0x88`–`+0x90` | the grid cell buffer of §7.2 (begin, end, capacity; 16-byte cells) |

### 3.2 A scape (0x34 bytes, constructor `0x004F6340`)

| Offset | Field |
|---|---|
| `+0x00` | name (a string) |
| `+0x18` | id |
| `+0x1C` | cluster list; `+0x20` the cluster count |
| `+0x24` | flags: bit 0 `0x1` auto sound (§4.1), bit 2 `0x4` untracked members (§5.1), bit 4 `0x10` keep clusters on clear (§4.3). The constructor does not set this field |
| `+0x28` | sound definitions (0x18 bytes each, §4.4) |
| `+0x2C` | radius (float) |
| `+0x30` | emanation total |

### 3.3 A cluster (from the shared block pool `0x00B098A8`; initialiser `0x004F5CD0`)

| Offset | Field |
|---|---|
| `+0x00`–`+0x08` | average position (x, y, z) |
| `+0x0C`–`+0x14` | position sum; `+0x18` the number of positions summed |
| `+0x1C` | an add counter, incremented by every add (§5.2), zeroed and used as a mark by slot 9 (§7.3), read nowhere else |
| `+0x20` | radius (float), the scape's radius when the cluster is made |
| `+0x24` | members: 16-byte nodes `{key, x, y, z}` from the same pool; `+0x30` their count |
| `+0x28` | playing sounds: private copies of sound definitions; `+0x34` their count. A copy's address is its sound's **instance id** |
| `+0x2C` | a countdown, read only by the pending list |
| `+0x38`, `+0x3C` | current volume of the two channels, initially 0 |
| `+0x40`, `+0x44` | target volume of the two channels, `0x400` (1024), never changed |
| `+0x48` | cached start volume, initially 0; recomputed only when starting a new sound copy as signed low-32-bit `(+0x38 × +0x3C) / 1024`, truncated toward zero |

### 3.4 A sound definition (0x18 bytes)

`{min, max, extra, soundId, flag (byte), f}`: `min`/`max` the member-count band (unsigned) in which the sound plays,
`soundId` the sound, `f` a float (50.0 by default). `extra`, `flag` and `f` are stored and copied but read by nothing
in this owner *[inferred]*.

**Instance identity (SCP4).** *[compat]* The sound manager receives the private
copy's address as an opaque 32-bit instance key, not as a record to inspect. The
scape start helper posts only sound ID and instance key, then volume; its supplied
flag does not reach the start payload (`0x004F5610`). The receiver reads the event's
sound ID, instance key and optional event flag (`0x004FA1F0`); it uses the key for
lookup/replacement and stores it in the new sound instance (`0x004F9D70`). It does
not read the definition's extra, flag or f through that key. Those stored/copied
fields therefore do not supply additional playback parameters on this path.


### 3.5 Input format and ownership (AU42)

There is **no scape resource decoder** in this owner. Slot 5's first argument is a string
interface: it is copied through the scape's string slot 14 (`+0x38`), not opened as a file or
TGI resource (`0x004F6551`–`0x004F656B`). The remaining arguments are a u32 ID, u32 flags
and an f32 radius. The radius is copied without a clamp, sign check or conversion. Sound
definitions are supplied by slot 16 or temporarily by slot 12; positions are pointers to
three f32 words, copied into members, with no retained caller pointer. No caller data buffer
becomes owned by the manager. Slot 11's metadata output is the **emanation total** `+0x30`,
not a resource ID, byte length or count of returned positions.

For a sound definition the exact record offsets are `+0 min:u32`, `+4 max:u32`,
`+8 extra:u32`, `+0x0C soundId:u32`, `+0x10 flag:u8`, `+0x14 f:f32` (24 bytes including
padding). Slot 16 sets f to `50.0` (`0x42480000`); the supplied flag occupies only its low
byte. A cluster owns a private 24-byte copy for each playing sound; that copy's address is
posted as its integer instance ID. SoundManager treats the address as an ID and does not
dereference this record. Stopping/freeing the copy ends that address's ownership here.
Scape strings/definitions, cluster/member storage and list nodes are manager-owned; foreign
sound references remain in the sound system. This is an in-memory grouping contract.

## 4. Scapes *[compat]*

### 4.1 `CreateScape(name, id, flags, radius)` (slot 5)

An existing scape with this `id` → false. Otherwise a new scape: name copied, `+0x18` := id, `+0x24` := flags,
`+0x2C` := radius, appended to the list. If `flags` bit 0 is set, `AddSound(id, id, 0, 0, 1, 0x7FFFFFFF)` follows:
the scape's own id is also a sound, played by every cluster with at least one member. True.

### 4.2 `RemoveScape(id)` (slot 6)

Walk the scapes in list order. *[quirk]* The flags `+0x24` of **every scape walked** are set to 0, the matching one
included, before its id is compared (`0x004F6A26`). So removing scape `k` clears the flags of all scapes before it,
and an unknown id clears all of them. On the match: `ClearScape(id)` (slot 7); when that is true, free its sound
definitions and lists and the scape, unlink it, and return true. No match → false.

### 4.3 Clearing

`ClearScape(id)` (slot 7, body `0x004F5F20`): an unknown id → false. Otherwise each cluster in turn:
- flags bit `0x10` clear: drop its members (§5.4's member freeing, average count := 0), stop its sounds (§6.1) and
  free it;
- bit `0x10` set: a cluster whose average count `+0x18` is non-zero loses its members but is kept, sounds playing; a
  cluster with none is stopped and freed.

Freed clusters leave the list and the count. Finally the emanation total `+0x30` := 0. True.

`ClearScapesWithFlags(mask, keep)` (slot 8): for every scape whose `flags & mask = mask`, every cluster loses its
members. When `keep` is 0 and the sound system is present, the cluster is then stopped, freed and removed. *[quirk]* With `keep` 0
and **no** sound system (before `Init` or after `Shutdown`), the walk does not advance past the first cluster and
never ends: the branch at `0x004F66A8` goes to `0x004F6785`, past the advance at `0x004F6782`. The emanation totals are not reset. True.

### 4.4 Sound definitions

`AddSound(id, soundId, flag, extra, min, max)` (slot 16): unknown scape, or a definition with this `soundId` already
present → false. Otherwise append `{min, max, extra, soundId, flag, 50.0}`; true. Nothing starts until the next
change of a cluster (§5.3).

`RemoveSound(id, soundId)` (slot 17): remove and free the definition with this `soundId`; true, or false when the
scape or the definition is unknown. Copies already playing in clusters are **not** stopped *[quirk]*; they stop when
their cluster is cleared or freed.

## 5. Emanations and clusters *[compat]*

An **emanation** is one source, identified by a caller-chosen 32-bit `key` (the handlers use the occupant's interface
address) and placed at a position `pos` (three floats).

### 5.1 `AddEmanation(id, soundId, flag, key, pos, f)` (slot 12)

1. **Unknown scape**: `CreateScape("nameless", id, 1, 50.0)` (so the scape's own id becomes its sound) and then
   slot 13 `(id, key, pos, f)`. The result is slot 13's. *[quirk]* `soundId` and `flag` of the first call are dropped.
2. Build a temporary sound definition `{1, 0x7FFFFFFF, 0, soundId, flag, f}`; it is used only when `soundId` ≠ 0
   (§5.3).
3. When flags bit 2 (`0x4`) is **clear**: if any member of any cluster of this scape has this `key` → false.
   Otherwise allocate a member node `{key, pos}`. When bit 2 is set no member is made (the emanation cannot later be
   removed or moved).
4. Add to a cluster (§5.2). On success the emanation total `+0x30` += 1; true. On failure the member node is freed;
   false.

The `f` argument reaches the cluster logic only as the temporary definition's last field. Slot 13 (sound 0) never
uses it, so slot 13's fourth argument has no effect *[quirk]*. (The event handler passes 50.0, audio.md §7.3.)

### 5.2 Adding to a cluster (`0x004F6C80`)

1. Find, among the scape's clusters, the **nearest** one whose distance from `pos` (Euclidean, three axes) is at most
   its radius `+0x20`, scanning in list order and keeping the first of equal distances (the comparison is strict,
   starting from `FLT_MAX`, `0x00A942B0`).
2. None → make a cluster at `pos` with the scape's radius, append it, cluster count += 1.
3. When flags bit 2 is clear, append the member node to the cluster (member count `+0x30` += 1).
4. Add `pos` to the cluster's running average (`0x004F5520`): the first position replaces the sum, later ones add to
   it; count += 1; average := sum × (1 / count).
5. The cluster's add counter `+0x1C` += 1, then its sounds are re-chosen (§5.3).

### 5.3 Choosing a cluster's sounds (`0x004F6AB0`)

For the per-emanation temporary definition (when given) and then for every sound definition of the scape, in order:
the sound plays when `min ≤ member count ≤ max` (unsigned compares on the cluster's `+0x30`), else it is stopped.
**Playing** means: when no copy with the same `soundId` is in the cluster's list, append a copy and start it (§6.1);
a sound already playing is left alone. **Stopping** removes and frees the copy with that `soundId`, if any. Then
the cluster's average position is posted to every playing copy (§6.1).

Each sound plays **at most once per cluster**, so N sources in one cluster are heard as one. The band test uses
the cluster's member count, not the scape total. With bit 2 set (no members) only bands with `min` 0 can play.

### 5.4 `RemoveEmanation(id, key)` (slot 14)

Unknown scape → false. For each cluster in order, look for a member with `key`. On the first hit:
1. Subtract the member's position from the running average (`0x004F55A0`). *[quirk]* The new average is
   `(sum − p) / n_old`: it divides by the count **before** the decrement (`0x004F55DD`–`0x004F55E8`). Removing one of
   two sources at x = 10 and x = 20 leaves the average at x = 5, not 10.
2. Free the member; member count −= 1; the scape's emanation total −= 1.
3. Re-choose the cluster's sounds (§5.3) and post its position.
4. When the cluster has no member left and the sound system is present: stop its sounds, free it and remove it.

True. No member with `key` → false.

### 5.5 `MoveEmanation(id, key, pos, radius)` (slot 15)

The `id` argument is **not used**: every cluster of every scape is searched for a member with `key` *[quirk]*. On the
first hit:
1. Subtract the old position from the average (same old-count division as §5.4) and add `pos` (§5.2 step 4); the
   member's position := `pos`.
2. Re-choose the cluster's sounds and post its position.
3. When `distance(pos, cluster average) ≤ radius + cluster radius`, the emanation stays: true.
4. Otherwise unlink the member from the cluster (member count −= 1) without touching its average, so the old
   cluster's average still includes the departed position *[quirk]*. When the cluster is now empty (and the sound
   system present) it is stopped, freed and removed. Then the member is added to the nearest cluster of its scape
   (§5.2, a new one if needed). True, or false after freeing the member when that fails.

No member with `key` → true without effect (the walk ends at `0x004F759D`, which returns 1).

## 6. Sound events *[compat]*

### 6.1 Posts

Every post goes through the sound system (`cIGZSndSys` slot 11, the four-word post; [audio.md](audio.md) §8.3 lists
the events), using the global `0x00B21AEC`. When it is null nothing is posted.

| Operation | Events, in order |
|---|---|
| start a copy (`0x004F5610`) | `0xEA451903 (soundId, instance, 0, 0)`, then `0x4A451915 (instance, start volume, 0, 0)` with the cluster's `+0x48` |
| stop a copy (`0x004F5AF0`, all: `0x004F5B80`) | `0x6A451908 (instance, 0, 0, 0)` |
| position (`0x004F5900`) | `0xCA45190F (instance, trunc x, trunc y, trunc z)` of the cluster's average, for every playing copy |
| volume (`_PostVolume` `0x004F56B0`) | `0x4A451915 (instance, v, 0, 0)` for every playing copy |

The start helper receives the definition's `flag` but does not use it *[quirk]*.

### 6.2 `OnTick` (`0x004F67C0`)

Under the pending-list lock: `dt` := timer − the previous reading (`+0x38` := timer). Each cluster on the pending
list `+0x58` has `dt` subtracted from its countdown, clamped at zero; at zero it is stopped and freed. Nothing ever
adds to that list, so this does nothing *[inferred]*.

Then, under the scape lock, every cluster of every scape **fades** toward its target volume. For each of the two
channels whose current value differs from the target: `d := target − current`; `step := trunc(d / 7)`, or `d`
itself when that is 0; `current += step`; post the new channel value to every playing copy (`_PostVolume`). The two
channels move identically, so each copy gets the same value twice. The fade counts ticks, not time, so its speed
follows the frame rate *[quirk]*. From 0 the value goes 146, 271, 378, 470, 549, 616, 674, 724, … 1018 and reaches
1024 on the 37th tick (when the gap falls below 7 it jumps the rest). New clusters start at volume 0, so every scape sound fades
in. There is no fade-out: stops are immediate. True.

The service callback receives the `base+4` view and ignores its tick argument and enabled
byte. Its timer is at **base+0x20**, previous reading at **base+0x38**. GetElapsedTime's EAX
and both readings are u32 milliseconds; dt is `(now − previous) mod 2^32`. For each pending
cluster subtract **unsigned min(dt,countdown)** from its u32 `+0x2C` (`0x004F67DB`–`0x004F681F`).
This is not a signed comparison or a 64-bit elapsed duration. A null pending entry releases
the pending lock and returns false before the scape walk (`0x004F6803`, `0x004F691E`–`0x004F6930`).
Ordinary ticks retain the pending lock while acquiring the scape lock; release scape then
pending at exit. Neither channel fade uses dt. Once any channel differs from its target,
the two channels are considered in order 0 then 1; an already-equal channel emits no post.
Differences/additions are 32-bit, and division by 7 is **signed truncation toward zero**
(`0x004F6960`–`0x004F6988`). `_PostVolume` updates that channel and posts its value, not the
product of both channels. The cached product `+0x48` changes only on a new copy's start
(`0x004F6103`–`0x004F611A`), so ticking alone does not refresh that cache.

## 7. Position queries *[compat]*

Rectangles are `{x0, z0, x1, z1}` over the x and z axes, **inclusive** on all four sides.

### 7.1 `GetClusterPositions(id, out, &count, &total)` (slot 11)

Unknown scape, or more clusters than `*count` → false, nothing written. Otherwise `*count` := the cluster count,
`*total` := the emanation total, and each cluster's average position is copied to `out` (12 bytes each), in list
order; true.

### 7.2 `GetClusterGrid(id, rect, out, &count, &total, useRect)` (slot 10, body `0x004F6E60`)

Unknown scape → `*count` := 0, false. No clusters → `*count` := `*total` := 0, false. Otherwise `*total` := the
emanation total and:

- **Few clusters** (count ≤ `*count`): `*count` := 0, then every cluster (inside `rect` when `useRect`) is written
  packed to `out[*count]` and counted. True.
- **Many clusters**:
  1. `n := trunc(sqrt(capacity))`, with `capacity` the incoming `*count`.
  2. The cell sizes are integers, `w := trunc((x1 − x0) / n)` and `h := trunc((z1 − z0) / n)`.
  3. An `n × n` grid of `{sum x, sum y, sum z, count}` cells (the buffer `+0x88`, grown when smaller) is zeroed.
  4. Every cluster (inside `rect` when `useRect`) adds its average to cell `trunc((z − z0) / h) × n +
     trunc((x − x0) / w)`, when that index is below `n²`.
  5. Each non-empty cell `i` writes the mean of its points to **`out[i]`**, and `*count` is the number of non-empty
     cells. True.

*[quirk]* Grid mode has three oddities:
- The output is sparse, indexed by cell, while `*count` counts entries as if they were packed.
- A point on the right edge can spill into the next row: the index is tested against `n²` only.
- A zero cell size (a rectangle narrower than `n`) divides by zero, and the resulting index writes outside the
  grid.

### 7.3 `GetNearClusterPositions(idA, idB, rect, out, &count, &total, useRect)` (slot 9)

Either scape unknown → `*count` := 0, false.
1. Build a temporary scape (radius := A's) and zero the add counter `+0x1C` of every A cluster, which serves as a
   "taken" mark.
2. For each B cluster `b` and, inside, each A cluster `a`, while A clusters remain untaken: when `a` is not taken
   and `distance(b, a) < A radius + B radius` (scape radii, strict), copy `a`'s average into a new cluster of the
   temporary scape and mark `a`.
3. The answer is §7.2 on the temporary scape (its total = the number of A clusters taken).
4. The temporary scape is then cleared and freed.

Its flags field is never set; either branch of §4.3 frees every cluster here, so this does not matter.

### 7.4 Capacity, writes and numeric boundaries (AU42)

All count/total outputs are required writable 32-bit words on paths that use them; no
null-pointer sizing mode exists. Positions have a 12-byte stride. Slot 11 compares signed
cluster count to signed capacity; slots 9/10 ultimately use the same signed comparison in
`0x004F6EB1`–`0x004F6EB9`. The total is independent of rectangle filtering:

| Operation/path | Return | Count output | Total output | Positions |
|---|---|---|---|---|
| Slot 11, missing group or insufficient capacity | false | unchanged | unchanged | unchanged |
| Slot 11, existing group and sufficient capacity, even empty | true | stored cluster count | stored emanation total | raw consecutive averages, list order |
| Slot 9, either ID missing; slot 10, ID missing | false | 0 | unchanged | unchanged |
| Slots 9/10, resulting group has no clusters | false | 0 | 0 | unchanged |
| Slots 9/10, nonempty group, direct mode | true | number passing filter | group's full emanation total | packed records, rest unchanged |
| Slots 9/10, nonempty group, grid mode | true | number of occupied cells | group's full emanation total | record at each occupied cell's index, holes/rest unchanged |

Thus a nonempty group entirely outside the rectangle can succeed with count 0 and nonzero
total. Slot 9's temporary group's total counts selected **A clusters**, not A's emanations.
It resets A's add-counter marks and leaves the selected marks set when it returns.

Grid n is `trunc(sqrt(signed capacity))` with the square root in x87; w/h are truncations
of x87 rectangle differences divided by the integer n (`0x004F6F5C`–`0x004F6F97`). There
is no intermediate f32 rounding of those differences. Per cluster compute row/column by
x87 subtraction and division by the signed integer h/w, truncating each separately; form
the low-32-bit `row*n + column`. The only index check is **signed index < n*n**; there is
no lower-bound or individual row/column check (`0x004F70E0`–`0x004F7110`). Each cell's
x/y/z sums are stored f32 after each addition; its count is a 32-bit word. At output use
the x87 reciprocal of signed cell count multiplied by each f32 sum, then store f32
(`0x004F7114`–`0x004F713E`, `0x004F718E`–`0x004F71B6`). Empty output cells are not zeroed.

No validation rejects negative/zero capacity, degenerate rectangles, NaNs, negative grid
indices or arithmetic overflow. Such inputs can reach invalid floating conversions or
memory outside the grid; they are not a safe false/required-size path. The defined usable
grid domain requires positive capacity, n > 0, nonzero w/h and in-range nonnegative indices.

### 7.5 Allocation and normal failure (AU42)

Allocation failure is **not** translated to false by these owner algorithms. In slot 5,
failed 0x34-byte scape allocation leads to a null string-vtable dereference at `0x004F6555`.
In slot 16, failed 0x18-byte definition allocation reaches a null write at `0x004F5ECA`.
Creating the temporary scape in slot 9 likewise writes through the null result at
`0x004F78F4`. New cluster and member blocks are unconditionally dereferenced after their
shared-pool refill (`0x004F6D29`, `0x004F7478`); cluster-list and member-list node allocations
are also required to produce storage. Starting a copy unconditionally copies all six words
through its allocation result before linking (`0x004F6050`–`0x004F60F3`). There is no
transactional unwind returning false after these storage failures. Grid vector growth
(`0x004F6DF0`, `0x004F63B0`, `0x004F5D50`) likewise has no owner-level failure return;
the subsequent cell writes require allocated storage. An allocator may itself raise/fail
before those dereferences; that does not create a scape boolean failure contract.

Normal false results are the missing/duplicate keys and capacity conditions in §§4–7,
duplicate tracked emanation key, or a false delegated call. Slot 12 and MoveEmanation's
recluster path free an uninserted member if the internal add helper returns false; they do
not make allocator failure recoverable. A missing global sound system suppresses sound
posts, but does not veto successful local insertion or copy linkage. In particular a sound
copy linked while the global is absent is not automatically retried when the global returns.

## 8. Callers *[compat]*

| Caller | Calls |
|---|---|
| fire handler (§9) | slot 5 once, slot 16 three times, then 13 and 14 per fire occupant |
| audio event handler ([audio.md](audio.md) §7.3) | slots 13, 14 and 15 on construction scapes (e.g. `0xC9DC0A61`, `0x49E7B4B0`, `0xAA62C82A`/`0xEA62C800`, `0x49DE8A94`), keyed by the occupant; those scapes are auto-created on first use as "nameless" with flags 1 and radius 50 (§5.1) |
| ambience manager | holds the service (audio-ambience.md §2.1) |

No caller of slots 7–11 or 17 was found in the audio owners *[inferred]*.

## 9. `cSC4AudioEventHandlerDisasterFire` *[compat]*

CLSID `0x69E500BC`, factory `0x004EB100`, object 0x34 bytes, made by `cSC4AudioCOMDirector::PostAppInit` (audio.md
§7.4). Interfaces: `+0x00` the message target (vtable `0x00A941E8`; IID `0x452294AA`; `DoMessage` `0x004F05D0`);
`+0x04` the main interface (`0x00A941D4`; IIDs `1`, `0x89E51472`; slot 3 `Init` `0x004F0740`, slot 4 `Shutdown`
`0x004F03A0`); `+0x08` an occupant filter (`0x00A941B8`, slot 3 `0x004F0440`); `+0x0C` its serialisable part
(`0x00A941A0`, the generic occupant-filter `Write`/`Read`/`GetGZCLSID`); `+0x14` a sound-event handler (`0x00A94190`;
IID `0xBB523BB2`; slot 3 always false). Fields: `+0x18` initialised, `+0x1C` reference count, `+0x20` the scape
manager.

- **Filter** (`0x004F0440`): an occupant passes when it answers `0x69D25C60` (the fire occupant interface
  *[inferred]*).
- **`Init`** (no-op when initialised):
  1. With the message server (`0x4FA845B` / `0x652294C7`; absent → false), subscribe `+0x00` to `0x99EF1142`
     (occupant inserted), `0x99EF1143` (occupant removed) and `0x26D31EC1` (city init).
  2. Get the scape manager into `+0x20`; absent → false.
  3. `CreateScape("Fires", 0x49E4CE85, 0, 100.0)`; false → `Init` fails.
  4. `AddSound(0x49E4CE85, s, 1, 0, min, max)` for (`0x29DC19EF`, 1, 9), (`0x09DC19A2`, 10, 49) and (`0x09DC1982`,
     50, `0x7FFFFFFF`).
  5. Initialised := 1; true.

  So a cluster of 1–9 fires plays the first loop, 10–49 the second, 50 or more the third. *[quirk]* A failure in
  steps 2–3 leaves the subscriptions in place. A second `Init` after `Shutdown` fails at step 3, because `Shutdown`
  does not remove the scape.
- **`DoMessage`** (`0x004F05D0`, true for every message): needs a standard message (QI `0x65297976`).
  - **City init** walks every occupant through the occupant manager (`0x00B43D0C` slot 44) with this filter. For
    each one it calls `AddEmanation(0x49E4CE85, occupant, position, 0.0)` (slot 13), the position being the
    occupant's slot 8.
  - **Inserted / removed**: the message's object, when it answers `0x26BB767E` and passes the filter, is added the
    same way, or removed with `RemoveEmanation(0x49E4CE85, occupant)` (slot 14).
- **`Shutdown`** (no-op when not initialised): unsubscribe the three messages, release the scape manager, clear the
  flag; true.

## 10. Open items

Questions are in [questions/audio-scape.md](questions/audio-scape.md) (prefix **SCP**). Not covered:
- the role of the third vtable `0x00A942F8`, whose slot 3 (`0x004F5200`) returns whether its argument is non-zero;
- the sound manager's handling of the posted events (audio.md §8).

## 11. Conformance tests

1. `CreateScape("a", 7, 1, 10.0)` then `AddEmanation(7, 0, 0, k1, (0,0,0), 0)`: one cluster, one member, and posts
   start `(7, I, 0, 0)` and volume `(I, 0, 0, 0)` for one instance `I`, then a position `(I, 0, 0, 0)`.
2. A second emanation at (5,0,0) (within 10): same cluster, member count 2, average (2.5, 0, 0); no new start; a
   position post `(I, 2, 0, 0)` (truncated).
3. A third at (50,0,0): a second cluster with its own instance.
4. Bands: `AddSound(7, 9, 0, 0, 2, 3)`. A cluster with one member plays only sound 7; adding a second member starts
   9; a fourth member stops 9.
5. Duplicate key with flags bit 2 clear → `AddEmanation` false, totals unchanged.
6. Remove-average quirk: members at x = 10 and 20, remove the second → average x = 5.
7. Move quirk: members at 0 and 1 in one cluster (radius 10), `MoveEmanation(_, k2, (100,0,0), 0)` → k2 in a new
   cluster at 100; the old cluster's average sum still includes 100.
8. `RemoveScape` of the second of three scapes: the first two scapes' flags become 0, the third's are unchanged.
9. Fade: a new cluster's copies receive volume values 146, 271, 378, … on successive ticks, 1024 at the 37th.
10. Grid: capacity 9 (n = 3), rect (0,0)–(30,30), clusters at (5,·,5) and (25,·,25) with more clusters than 9 →
    out[0] and out[8] written, `*count` = 2.
11. Unknown scape in slot 13 → a "nameless" scape with flags 1, radius 50 is created and plays its own id.
12. Fire handler: 10 fire occupants at one spot → one cluster, sound `0x09DC19A2` playing, `0x29DC19EF` stopped.
13. AU42 timer wrap: previous `0xFFFFFFF0`, now `0x00000020` gives dt 48. Pending countdown
    60 becomes 12; countdown 20 becomes 0 and its cluster is stopped/freed. A null pending
    entry instead returns false, with no fade walk.
14. AU42 channels/cache: after one tick from zero both channels are 146 and each playing
    copy receives two volume posts of 146. Starting another sound copy then caches/posts
    `trunc(146*146/1024) = 20`; later ticks change channels but leave that cache at 20 until
    another copy starts. A channel at 1030 targeting 1024 jumps to 1024 (−6/7 truncates to 0).
15. AU42 outputs: slot 11 for two clusters, total 5, capacity 1 fails with all outputs
    untouched. Slot 10 on the same nonempty scape, capacity 2 and a rectangle excluding both,
    succeeds with count 0 and total 5. A missing ID sets count 0 but preserves a total sentinel.
16. AU42 grid: use ten clusters, nine at `(5,0,5)` and one at `(25,0,25)`, capacity 9,
    rectangle `(0,0,30,30)`: n=3, w=h=10; only out[0] and out[8] change, count=2, total=10.
    The seven holes retain caller sentinels. A point `(30,0,5)` maps to index 3, rather than
    being rejected as column 3 outside a three-column grid.
17. AU42 definitions: register name `"test"`, ID 7, flags 0, radius 10; add definition sound 9,
    extra 123, flag 255, band [2,3]. One member produces no start, two start one copy of sound 9,
    a fourth stops it. No resource read occurs; neither extra nor flag nor f changes posted payloads.
