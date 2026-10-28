# Spec: foundations (streams, save file, response curves, messages, random numbers, variants, property holders, regional records, agent order)

Status: **implementation spec**, clean-room input. This document describes *behaviour*, not code. It was
written by the analysis team from observing the original program and sample saves. Implementers must work from
this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled
or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* marks a finding that is consistent with all evidence but not proven; *[quirk]* marks original
behaviour that is surprising and documented so that nobody "fixes" it by accident; *[compat-limit]* marks original
behaviour that depends on something FreeCity4 cannot reproduce (object addresses, wall-clock time), so only its
distribution is matched, not each individual outcome.

Other specs refer to this one for the meaning of `str`, `ref`, `reskey`, "response curve", "standard message"
and the random number generators, and (§9–§13) for variants, the "property holder record", the regional-city
record, the C runtime `rand()` and the order of agents within a level.

## 1. Two contexts

- **Plug-in mode** (milestones M1–M5). The original game owns the save file, the streams and the message server.
  Our replacement objects receive `cIGZOStream` / `cIGZIStream` pointers in `Write` / `Read` and must produce and
  accept exactly the bytes described here. Read-side quirks (§2.9) are what our `Read` code will observe.
- **Standalone mode** (M6+). FreeCity4 implements the streams, the save container and the message server itself.
  It must write byte-identical data. On the read side, the quirks are *[choice]*: we may treat them as errors.

## 2. Serialization streams

### 2.1 Model

During a save or load, every serializable object reads or writes through a **segment stream** opened on one
record of the save package (§3). The stream implements `cIGZOStream` (write) or `cIGZIStream` (read) and also the
segment interfaces `cISC4DBSegmentOStream` (IID `0x6999A8FB`) / `cISC4DBSegmentIStream` (IID `0x4999A8F5`), which
add resource keys, object references and variants. Code that needs references must obtain the segment interface
with `QueryInterface`. A plain `cIGZIStream` cannot read references.

A stream has a position inside its record and a single **good** flag:

- The flag is set by a successful `Open`.
- Every `Set*`/`Get*`/`Write*`/`Read*`/`Skip` call does nothing and returns false if the flag is already clear.
  Otherwise it performs the operation and then sets the flag to the operation's result. The error state is
  therefore **sticky**: after the first failure every later call fails.
- `GetError()` returns 0 while the flag is set and a non-zero value (1) after a failure.
- `Flush()` does nothing. `SetUserData` / `GetUserData` are not used by the simulation.

### 2.2 Primitive encodings *[compat]*

All multi-byte values are **little-endian** (confirmed on sample saves). There is no alignment and no padding.

| Method (write / read) | Bytes | Encoding |
|---|---|---|
| `SetSint8` / `GetSint8`, `SetUint8` / `GetUint8` | 1 | two's complement / unsigned |
| `SetSint16` / `GetSint16`, `SetUint16` / `GetUint16` | 2 | LE |
| `SetSint32` / `GetSint32`, `SetUint32` / `GetUint32` | 4 | LE |
| `SetSint64` / `GetSint64`, `SetUint64` / `GetUint64` | 8 | LE |
| `SetFloat32` / `GetFloat32` | 4 | IEEE-754 binary32, LE bit pattern, written unchanged |
| `SetFloat64` / `GetFloat64` | 8 | IEEE-754 binary64, LE bit pattern, written unchanged |
| `SetVoid(p, n)` / `GetVoid(p, n)` | n | raw bytes, no length prefix |
| `SetGZStr` / `GetGZStr` | 4 + n | §2.3 |
| `SetRZCharStr` / `GetRZCharStr` | 4 + n | §2.4 |
| `WriteResKey` / `ReadResKey` | 12 | §2.5 |
| `SetGZSerializable` / `GetGZSerializable` | payload | inline object, §2.6 |
| `WriteGZSerializable` / `ReadGZSerializable` | 4 or 8 | object reference, §2.7 |
| `WriteVariant` / `ReadVariant` | 8 + value | §9 |

- **Booleans** have no method of their own. By convention they are written with `SetUint8` as 0 or 1
  *[compat]*. *[choice]* Readers treat any non-zero byte as true.
- Signed and unsigned variants of a width produce identical bytes.
- **Reads are atomic.** A read that needs *n* bytes when fewer remain in the record fails, consumes nothing and
  leaves the destination unchanged. Callers that read into uninitialised variables must initialise them first.

### 2.3 Strings (`SetGZStr` / `GetGZStr`)

**Write** *[compat]*:

```
uint32  header = byteLength          (top byte 0 = 8-bit text)
byte[byteLength]  the string's bytes, no terminator
```

- The bytes are the `cIGZString`'s own bytes, unchanged. The game's internal text encoding is **UTF-8**
  *[inferred]*: its conversion routines treat encoding code 8 (UTF-8) as the native string format. Sample saves
  contain ASCII only (open question 2).
- Writers must keep strings at most **200,000 bytes** and must not put NUL bytes in them *[compat]* (see the read
  quirks below).

**Read**: read the 32-bit header; `encoding = header >> 24`, `count = header & 0xFFFFFF`.

| Case | Behaviour |
|---|---|
| `encoding = 0x00` | read `count` bytes; they are the string |
| `encoding = 0x10` | read `count` UTF-16 code units (2 × `count` bytes, LE) and convert them to UTF-8 |
| `count > 200,000` (either encoding) | the read fails (header consumed, stream enters the error state) |
| `count = 0` | empty string, success |
| any other encoding byte | *[quirk]* legacy format, below |

Quirks of the original reader:

1. *[quirk]* If at least 2 bytes were read and the **second byte is 0**, the data is decoded as UTF-16 even when
   the header said 8-bit. For an 8-bit string this decodes memory beyond the string. Hence the "no NUL" rule.
2. *[quirk]* If fewer than 4 bytes remain for the header, the string becomes the remaining 0–3 bytes and the read
   **succeeds** (the stream stays good).
3. *[quirk]* Legacy format (encoding byte not 0 or 0x10): the reader steps back over the header and appends bytes
   one at a time, as many as the record's total size, ignoring failed reads. It returns success. No writer in
   the shipped game produces this.

*[choice]* In standalone mode, quirks 2 and 3 are read errors, and quirk 1 does not exist (the header decides).

### 2.4 C strings (`SetRZCharStr` / `GetRZCharStr`)

- `SetRZCharStr(s)` writes exactly what `SetGZStr` would write for the string `s` (without its terminator).
- `GetRZCharStr(buf, maxBytes)` reads a string as in §2.3 and copies `min(length + 1, maxBytes)` bytes, including
  the terminating NUL if it fits. If the string is truncated, the buffer is **not** NUL-terminated *[compat]*.
  Its result is the stream's good flag.

### 2.5 Resource keys

`WriteResKey(key)` writes three `uint32`: **type, group, instance**, in that order. `ReadResKey` reads them back.
It fails if any of the three reads fails.

**Persistent-resource references.** Some records refer to a game resource (for example an exemplar) instead of a
saved object. The convention is three `uint32` (type, group, instance) of the resource; a missing resource is
written as three zeros. On load the key is resolved through the resource manager, requesting the interface the
reader needs. A zero key does not resolve. Subsystem specs say where this form is used.

### 2.6 Inline nested objects (`SetGZSerializable` / `GetGZSerializable`)

`SetGZSerializable(obj)` simply calls `obj.Write(stream)` on the same stream. `GetGZSerializable(obj)` calls
`obj.Read(stream)`. There is no length, class ID or marker. The reader must already hold an object of the right
class. The stream's good flag becomes the method's result. **This is not the reference mechanism.** Despite the
similar name, objects shared or owned across records use §2.7.

### 2.7 Object references

A **reference** (`ref` in other specs) is written with `cISC4DBSegmentOStream::WriteGZSerializable(obj)`:

```
uint32  objectID          0 = null reference, nothing follows
uint32  classID           only if objectID ≠ 0: obj->GetGZCLSID()
```

The referenced object itself is **not** written here. Writing a reference registers the object with the save's
**class-object store** (§3.3), which writes each registered object exactly once into the record for its class,
after all other records. The object's data therefore lives in a different record from the reference.

**Object IDs** *[compat]*: any non-zero 32-bit value that is the same for every reference to the same object
during one save and different for different objects of the same class. The original uses the object's memory
address, so the IDs in sample saves look like heap pointers and are unique across all classes. *[choice]*
FreeCity4 numbers objects 1, 2, 3, … in registration order. IDs mean nothing after the save is loaded.

**Reading** (`cISC4DBSegmentIStream::ReadGZSerializable(ppObj, loadNow)`):

1. Read `objectID`. If it is 0, the result is a null pointer and success.
2. Read `classID`. Resolve `(classID, objectID)` in the store's registry:
   - already known → the same object (`AddRef`);
   - `classID` not listed in the save's class index (§3.3) → **failure** (the stream enters the error state);
   - otherwise create a new instance with `GZCOM()->GetClassObject(classID, GZIID_cIGZSerializable, &p)`. If
     creation fails → failure. If it succeeds, register it under `(classID, objectID)`.
3. The destination pointer is set to null **before** anything is read, and the object it held is **not** released
   (the game's helper `SC4ReadGZSerializable`, `0x005D9730`). A failed read therefore leaves null in the
   destination. List readers (`0x005DB3C0`) append that null for every failed reference *[compat]*.
4. The new object's `Read` is **not** called here. It is called when the loader reaches the object's entry in its
   class record (§3.5). The shipped game never sets `loadNow`; when set, the object's entry would be located and
   read immediately *[inferred]*.

Usual helper semantics *[compat]*: callers ask for an interface. After a successful reference read the result
is `QueryInterface`d to the requested IID. A null reference gives a null pointer and success. A non-null object
that lacks the interface gives failure, but the stream itself stays good.

*[compat]* **SDK discrepancy.** The shipped `cISC4DBSegmentIStream::ReadGZSerializable` takes **two** arguments
(`cIGZSerializable**`, `bool loadNow`), but the gzcom-dll header declares only the pointer. With MSVC
`__thiscall` the callee removes its arguments from the stack, so calling it through the SDK declaration corrupts
the stack. The plug-in shim must declare the extra `bool` (pass false).

Consequences for every `Read` implementation *[compat]*:

- A referenced object may not have been read yet. It may even be read later in the load, or never if its entry is
  missing. `Read` must only store the pointer. Work that depends on other objects' state belongs in `Init` or a
  later initialisation message.
- Cycles (A refers to B, B refers to A) are allowed and common, for example a department and its line items.

**References the load keeps** *[compat]* (SV1). The registry of step 2 is the **object table of the city
segment's COM serializer** (`cSC4COMSerializer`; the segment `cSC4DBSegment` creates one in its constructor,
`0x005D3740`, holds it at `+0x1CC` and returns it from its slot 4, `0x005D2070`). The table is a map from class ID to a
map from object ID to an auto-ref-counted pointer (`+0x1C`; resolver `0x005D2DD0`).

| Event | References |
|---|---|
| a reference read **creates** an object | +1 from `GetClassObject`, handed to the destination; +1 kept by the table entry |
| a reference read **resolves** a known object | +1 for the destination (`AddRef`); the table adds none |
| the typed helper (`0x005D9730` → `0x005D96C0`) | QIs the result to the requested IID (+1 into the destination) and releases the `cIGZSerializable` pointer (−1): net +1 per successful read; a failed QI releases it and leaves null |
| the loader reaches an entry (§3.5) | resolves into a local (+1), calls `Read`, releases the local (`0x005D3D58`): net 0 |

So after a load **every object the load created holds exactly one extra reference, owned by the table**, on top of
the references held by whatever stored the destination pointers. The table drops them all at once when the
serializer is destroyed, which happens when the **segment that loaded the city is closed** (segment `Close`
`0x005D20C0` releases the serializer; its destructor `0x005D3800` calls the reset `0x005D3250`, which releases
every entry). That happens:

1. **At the next save.** `cSC4App::SaveCity` (`0x00455E20`) writes the city through a **new** segment, whose own
   table holds one reference per object the save registers (writing a reference registers the object,
   `0x005D43F0`) until that segment is closed at the end of the write (`0x0045641B`). It then unregisters and
   closes the loaded segment (`0x00456449` → `UnregisterCityDBSegment` `0x00454A10`), which drops the load's
   references, and registers the saved file as the new city segment (`0x0045662F` → `0x00454B20`). That segment
   is only opened for reading, and its table stays empty. After a save, the table holds no reference on
   anything.
2. **At city close, if no save came first.** `PartialCloseCity` (`0x00454A60`) posts `0x26D31EC2`, calls the
   city's `Shutdown`, posts `0x26D31EC3`, releases its city pointer and only then unregisters the segment
   (`0x00454AF3`). So during every subsystem's `Shutdown` in a loaded, unsaved city the table still holds its
   reference: a subsystem's release in `Shutdown` is **not** the last one, and the objects are destroyed when the
   segment closes.
3. **On a failed load** (`0x00455957`), the same way.

Not at the end of the load: `LoadCity` (`0x004555F0`) keeps the segment open, registered with the resource
manager and the app.

The reset releases the entries in **descending class ID**, and within a class in descending object ID (the
saved IDs, which are the saving session's addresses). FreeCity4's core does not need to reproduce this order: it
holds no reference for the game, and its objects are destroyed by their own last release. A glue layer that
compares reference counts with the original must count the table's reference (one per loaded object) until the
next save or the city's close.

### 2.8 Variants

`WriteVariant` writes a `uint32` **marker** (0, or `0xA9BEE9AC` for an object-valued variant) and then a
`uint32` **disk type tag** (bit 15 set = array) and the value. Arrays are a `uint32` count followed by the
elements, each in its primitive encoding. The disk tags are **not** the in-memory `cIGZVariant` type numbers.
The full format is §9. Variants appear in saves inside property holders (§10).

### 2.9 Error behaviour summary

| Situation | Result |
|---|---|
| read past the end of the record | that call fails, nothing consumed, destination unchanged, stream error (sticky) |
| any call after an error | returns false, does nothing |
| string longer than 200,000 | failure |
| short read of a string header | *[quirk]* success with 0–3 garbage bytes |
| reference to a class not in the save | failure |
| reference whose class cannot be created (for example a missing plugin) | failure |
| `Skip(n)` with fewer than n bytes left | failure. Some bytes may already be consumed *[inferred]* |

### 2.10 Version convention

The framework itself has no version field. By convention every serializable object starts its payload with a
`uint16` version and its `Read` rejects versions it does not know (each subsystem spec lists its own). Records
opened through the game's "auto stream" helpers (the root record in §3.2) also start with a `uint16` version
written by the helper.

## 3. City save file

### 3.1 Container

A `.sc4` city is a **DBPF 1.0** package with index version 7.0 (20-byte index entries). The container format is
the same as the game's `.dat` files (see `tools/dbpf.py` for a reader). Record data may be QFS (RefPack)
compressed. Compressed records are listed in the directory record (type/group `0xE86B1EEF`, instance
`0x286B1F03`).

Compression rule of the original writer *[compat for byte-identical output, otherwise choice]*: a record is
compressed when it is at least 50 and at most 16,000,000 bytes long and compression shrinks it below 80 % of its
size. Readers must accept both compressed and uncompressed records.

### 3.2 Records used by the simulation

| Type | Group | Instance | Contents |
|---|---|---|---|
| `0x499B23FE` | `0x299B2D1B` | 0 | class-object **index** (§3.3) |
| *classID* | `0x299B2D1B` | 0 | **class record**: every saved object of that class (§3.3) |
| `0x49DD6F4C` | `0x49DD6F4E` | 1 | **root record**: `uint16 1` (stream helper version), `uint16 2` (app version), `ref` to the city (IID `0x66BA4DB5`), 12 bytes |
| `0xEA3FF4F5` | `0xEA3FF506` | `0xEA3FF4F5` | app load/save version: a bare `uint32` = **65** (`0x41`), no version prefix |
| `0x0B7D45E0` | `0x0B7D45E1` | `0x0B7D45E2` | module version: `uint16 3`, then four `uint32` (the executable's version parts) *[inferred]* |
| any | `0xAB85E27D` | any | **retained records** (§3.8) |

**This is the CLSID-as-type convention.** A subsystem's data is stored as one entry in the class record whose
**type is the subsystem's class ID**, group `0x299B2D1B`, instance 0. For example the simulator lives in
`0x2990C1E5/0x299B2D1B/0` and the budget simulator in `0xE990BE01/0x299B2D1B/0`. Subsidiary objects, such as
departments, line items and demand nodes, live in their own class records. A subsystem record is **never
located by its key alone**. It is reached through a reference from the city (§3.6). The key only follows from the
class ID.

Other records in a city file include region thumbnails and views, terrain data and city name strings
`0x2026960B` in groups `0x6AAEE931`/`0x6AAEE932`. The city's auxiliary cheat-state record
`0x89D9B9EE/0x69D9B9F1/0` is specified in save-records §2.1; the **regional-city record**
`0xCA027EDB/0xCA027EE1/0` is specified in §11.

### 3.3 Class-object store

**Class record** `(classID, 0x299B2D1B, 0)`: a sequence of entries, nothing else:

```
uint32  entrySize        bytes from the start of this field to the end of the payload (12 + payload length)
uint32  crc              §3.4
uint32  objectID
byte[]  payload          exactly what the object's Write produced
```

**Index record** `(0x499B23FE, 0x299B2D1B, 0)`:

```
uint32  version = 1
uint32  classCount
int32   entriesWritten                       total entries in all class records
classCount × { uint32 classID; uint32 objectCount }      ascending classID
```

`objectCount` is the number of distinct objects of that class registered during the save. Sample saves were
checked: the counts match the number of entries in each class record, and every class record parses into whole
entries with no trailing bytes.

### 3.4 Entry checksum *[compat]*

`crc` = CRC-32 with polynomial `0x04C11DB7`, **not reflected** (most significant bit first), initial value
`0xFFFFFFFF`, **no final XOR** (the "CRC-32/MPEG-2" parameter set). It covers the bytes from `objectID` onwards,
`min(entrySize − 8, 250000)` bytes. Larger payloads are only checksummed over their first 250,000 bytes (the
`objectID` included). Check value: CRC of ASCII `"123456789"` = `0x0376E6E7`. All entries of two sample saves
verify under this rule, including entries above 250,000 bytes.

### 3.5 Writing and loading the store

**Saving** (after all ordinary records are written) *[compat]*:

```
while some registered object has not been written:
    for each class C, ascending class ID, with unwritten objects:
        open record (C, 0x299B2D1B, 0) for appending
        for each unwritten object O of C, ascending object ID:
            p := position; write uint32 0, uint32 0, uint32 ID(O)
            ok := O.Write(stream)
            if ok:  patch entrySize and crc at p; continue after the payload
            else:   move back to p (the entry is discarded; later entries overwrite it)
            mark O written
            if the stream is in error: abort the save
        close the record
write the index record (from the start of the record)
```

- The class loop walks the **live** sorted set of classes: a class registered during a sweep is written in the
  same sweep when its ID is higher than the current one (save-records.md §5) *[compat]*.
- Writing an object may register more objects through references. They are written in a later pass and appended
  to their class records. Entries in a class record are therefore sorted by ID within each pass, but not overall.
- When a class is registered for the first time in a save, any existing record with its key is deleted first.
  This only matters when writing into an existing package. The game always writes a fresh file (§3.7).
- *[quirk]* A discarded entry still counts in the index's `objectCount`, and the loader would then miss an entry.
  **Our `Write` must never return false except for a stream error** *[choice]*.
- If no object was ever registered, nothing is written (no index).
- *[compat]* Many saved fields hold Microsoft debug-heap fill words that the original never initialised
  (`0xCDCDCDCD`, `0xBAADF00D`, `0xCCCCCCCC`: budget by-month ordinance arrays, growth developer, residential, flora
  RNG, traffic, trips). A reader must accept them and a writer must carry loaded values through unchanged
  (save-records.md §6).

**Loading** runs once, right after the package is opened and **before any other record is read**:

```
read the index; version must be 1, otherwise the load fails
for each class C in the index, ascending class ID:
    open (C, 0x299B2D1B, 0); if missing, skip the class
    if objectCount > 9,999,999: the class fails
    repeat objectCount times:
        p := position; read entrySize, crc
        verify crc over min(entrySize − 8, 250000) bytes from p + 8   (position unchanged)
        on mismatch: the class fails, stop reading it
        read objectID; resolve (C, objectID) as in §2.7 (create if new)
        if resolution failed: move to p + entrySize and continue
        call O.Read(stream)          (its result is ignored)
        if position ≠ p + entrySize: move to p + entrySize
    the class fails if the number of objects registered for C ≠ objectCount
```

- **Entry framing protects neighbours.** If `Read` reads too little or too much (within the record), the loader
  re-synchronises at the next entry. But a `Read` that puts the stream into the **error state** makes every later
  entry of that class fail, because the error is sticky. `Read` must not read past its own payload *[compat]*.
- Objects are read in ascending class ID, so a subsystem is usually read **before or after** the objects it refers
  to, never in a guaranteed order (§2.7).
- *[quirk]* The original's overall result reflects only the last class whose record could be opened. A failure in
  an earlier class (for example objects of an uninstalled plugin class) does not fail the load. The game aborts
  the load when this result is false. *[choice]* In standalone mode FreeCity4 reports per-class failures and
  fails the load only if a class that the city references fails.
- The original sanity check on `entrySize` is weak. *[choice]* We reject an entry whose `p + entrySize` exceeds the
  record size, or whose `entrySize` is below 12.

### 3.6 The city and its subsystems

The **city** object (class `0x8990C372`) is stored in the class-object store like any other object, at
`0x8990C372 / 0x299B2D1B / 0`. The root record (§3.2) refers to it. Write is `0x0045C5B0`; Read is
`0x0045CB10`. Write emits **version 12**. Read accepts **8, 9, 10, 11, 12 only**; its two unsigned comparisons
at `0x0045CB42`–`0x0045CB50` reject every other `u16` after consuming the version, before the property holder.

#### 3.6.1 Complete city payload *[compat]*

All fields below are in order, little-endian and unaligned. `u8`, `u16`, `u32`, `s32`, `f32` are 1, 2, 4, 4,
4 bytes. A string is the **full §2.3 encoding**, including its `u32` header; an empty string still occupies
4 bytes. A `ref` is §2.7 (4 bytes for null, 8 for non-null), not an inline component payload. `S+offset` names
a field relative to the **serializable interface passed to Read/Write**; this is 8 bytes after the city's
primary interface. These offsets disambiguate retained unnamed state and are not additional wire bytes.

| Order | Field | Wire encoding / version gate |
|---|---|---|
| 1 | city version | `u16`, writer 12 |
| 2 | city's property holder | inline §10.2 holder: `u16 2`, `u32 propertyCount`, then each `u32 key`, `u32 propertyID`, §9.2 variant; no object reference or byte-size prefix |
| 3 | own state word (`S+0x28`) | `u32` |
| 4 | occupant serial counter (`S+0x2C`) | `u32` |
| 5 | original language, original country | `s32`, `s32` (`S+0x30`, `+0x34`) |
| 6 | last language, last country | `s32`, `s32` (`S+0x38`, `+0x3C`) |
| 7 | own state word (`S+0x40`) | `u32` |
| 8 | established | `u8` (`S+0x1D`) |
| 9 | city state | **v10–12 only**, `u8`, zero-extended into the internal 32-bit field `S+0x20` |
| 10 | city name | §2.3 string (`S+0x6C`) |
| 11 | second city-name string | §2.3 string (`S+0x80`), retained separately |
| 12 | mayor name | §2.3 string (`S+0x94`) |
| 13 | second mayor-name string | §2.3 string (`S+0xA8`), retained separately |
| 14 | city description | §2.3 string (`S+0xBC`) |
| 15 | second description string | §2.3 string (`S+0xD0`), retained separately |
| 16 | world longitude, latitude, base elevation | `f32` × 3 (`S+0xE4`, `+0xE8`, `+0xEC`) |
| 17 | own state word (`S+0xF0`) | `u32` |
| 18 | six size/scale values | `f32` × 6, in field order `S+0xF8`, `+0xFC`, `+0x100`, `+0x104`, `+0x108`, `+0x10C` |
| 19 | cell dimensions | `s32` X, `s32` Z (`S+0x110`, `+0x114`) |
| 20 | components | the applicable references from §3.6.2, in that table's order |
| 21 | final city flag | `u8` (`S+0x1FC`), **in every accepted version**, immediately after the last applicable reference |

No trailing size, checksum, or alignment belongs to this payload; the enclosing class entry supplies its size
and CRC (§3.3–§3.4). In particular, the property's version/count and all six string headers must be read before
looking for component references. The two name strings and two description strings in each pair are distinct
serialized fields, even if one is empty or their contents match.

**Byte-size check.** Let `H` be the complete inline holder's byte size, `C` the number of component-reference
slots for the city version, `U` the number of those slots that are non-null, and `D` the total encoded data
bytes in the six strings, excluding their six headers. The payload size is
**`104 + H + (1 if version ≥ 10, otherwise 0) + 4×C + 4×U + D`**. Reference bytes inside properties belong to
H, not U. For example, a v12 payload with an empty holder (`H = 6`), six empty strings (`D = 0`) and all 53
component references non-null occupies **535 bytes**, or **547 bytes** with its class-entry header. Omitting
the state byte for v8/v9 and the absent reference slots changes the size; adding null placeholders does not
reproduce those legacy layouts.

**Legacy differences and load effects.** The own-field sequence above is shared by v8–12 apart from row 9.
For v8/v9 there is **no state byte** between established and the first string. Read synthesizes city state
0 when established is zero, otherwise 2 (`0x0045CBB8`–`0x0045CBE0`). Versions 10–12 read the state byte
literally. If established is zero, the reader replaces the loaded city-name field with localized string
`0x6A231EAA / 0xAA738C4E` after the scalar fields. That substitution consumes no save bytes. Later, after
the auxiliary record, existing language-specific city/mayor name records (type `0x2026960B`, groups
`0x6AAEE931`/`0x6AAEE932`) may replace those names; those records are not embedded here.

**Instruction checks.** Property holder `0x0045C5DB` / `0x0045CB5B`; first seven scalar fields
`0x0045C5E0`–`0x0045C62A`; established and state are **byte** writes `0x0045C637`, `0x0045C644`; six strings
`0x0045C64F`–`0x0045C695`; nine binary32 fields and the intervening u32 `0x0045C6A3`–`0x0045C721` (not
doubles); two s32 dimensions `0x0045C72F`, `0x0045C73D`; final flag `0x0045CA03`–`0x0045CA13`.

#### 3.6.2 Component references *[compat]*

Version 12 has **53 references** to components, in this fixed order. On read each reference is resolved and
`QueryInterface`d to the IID shown, with deferred loading (`loadNow = false`, §2.7). The class IDs are those
found in sample saves. Every slot consumes a reference, including the zone developer / lot manager alias;
two references to the same object share its object ID rather than omitting the second slot.

| # | Component | IID requested on read | Class (sample saves) |
|---|---|---|---|
| 1 | history warehouse | `0x69F4A803` | `0x89EFA536` |
| 2 | demolition utility | `0x26DDE2EA` | `0x0990C38A` |
| 3 | dispatch manager | `0x694028EC` | `0x4990BD93` |
| 4 | occupant manager | `0xC6BB8334` | `0x098F964D` |
| 5 | prop manager | `0x28326F3E` | `0xA990BD10` |
| 6 | zone manager | `0x67652433` | `0x298F9B2D` |
| 7 | zone developer | `0xE818ED2B` | `0x498F9B01` |
| 8 | lot manager | `0x681905BD` | `0x498F9B01` (the zone developer object) |
| 9 | lot configuration manager | `0xC8EDBEC0` | `0x299062C0` |
| 10 | prop developer | `0xE9A1F4B6` | `0x89C48F47` |
| 11 | network lot manager | `0x89A5BA3F` | `0xE9A6CD3B` |
| 12 | landfill developer | `0x8A09D502` | `0xEA09D4AE` |
| 13 | airport developer | `0xEA20548D` | `0xAA2049A1` |
| 14 | seaport developer | `0xCA22FE3E` | `0xEA230A28` |
| 15 | park manager | `0xAA3FE6BC` | `0xAA3FE6BC` |
| 16 | lot developer | `0x4818E5D2` | `0xA990BFE0` |
| 17 | tract developer | `0xA818ED1B` | `0x2990C142` |
| 18 | **simulator** | `0x8695664E` | `0x2990C1E5` |
| 19 | aura simulator | `0xC773E880` | `0x0990BDD8` |
| 20 | budget simulator | `0x6772BD23` | `0xE990BE01` |
| 21 | building development simulator | `0x07813EAE` | `0x8990BE7A` |
| 22 | commercial simulator | `0x077ABEB2` | `0x8990C080` |
| 23 | crime simulator | `0x67659555` | `0x8990C09A` |
| 24 | demand simulator | `0x47967E16` | `0x2990BFF2` |
| 25 | fire protection simulator | `0x88F54C7B` | `0x2990C05A` |
| 26 | flammability simulator | `0x48F41617` | `0xE990C045` |
| 27 | flora simulator | `0x06C66177` | `0xC990C034` |
| 28 | industrial simulator | `0x477A9E7D` | `0xA990C022` |
| 29 | land value simulator | `0xE7781D8C` | `0xE990C18E` |
| 30 | neighbours simulator | `0x0773FEF0` | `0xC990C0A4` |
| 31 | ordinance simulator | `0x41193C3A` | `0xA990C08E` |
| 32 | plumbing simulator | `0x47782CCC` | `0x0990C075` |
| 33 | police simulator | `0x67659895` | `0x6990C04F` |
| 34 | pollution simulator | `0x877181EC` | `0x8990C065` |
| 35 | power simulator | `0xE9E7BFF5` | `0x0990C005` |
| 36 | residential simulator | `0x077AC1EE` | `0x4990C013` |
| 37 | traffic simulator | `0x68027F37` | `0x6990C1AA` |
| 38 | weather simulator | `0xC6C66245` | `0x2990C1BC` |
| 39 | civic building simulator | `0x8A41322F` | `0x0A413481` |
| 40 | automata script system | `0x8A1B6647` | `0x4A1B664E` |
| 41 | automata controller manager | `0xCA14C9FC` | `0xCA14CA03` |
| 42 | traffic network map | `0xCA0F81B9` | `0x6A0F82B2` |
| 43–46 | vehicle, pedestrian, aircraft, watercraft managers | `0x098F9BF4` (all four) | `0xA98F9D3B`, `0xC98F9D17`, `0xC98F9D4A`, `0xA98F9D28` |
| 47 | advisor system | `0x4A0DD617` | `0x4A0DD616` |
| 48 | tutorial system | `0xAA3836F6` | `0xAA3836FB` |
| 49 | terrain | `0xC70BD37E` | `0xE98F9525` |
| 50 | 24-hour clock | `0x681BD53A` | `0xA7E7F929` |
| 51 | city detail manager (city version ≥ 9) | `0x6AD4B9F1` | `0xEAD4BA36` |
| 52 | city situation manager (version ≥ 11) | `0x6BB1437A` | `0x0BB14381` |
| 53 | network manager (version ≥ 12) | `0xA9764C8C` | `0xC990BD46` |

| City version | References present | Count | Fields immediately after the references |
|---|---|---|---|
| 8 | #1–50 | 50 | final `u8` |
| 9, 10 | #1–51 | 51 | final `u8` |
| 11 | #1–52 | 52 | final `u8` |
| 12 | #1–53 | 53 | final `u8` |

The three reference gates are independent comparisons at `0x0045D0D6`, `0x0045D0F4`, `0x0045D112`.
There are **no placeholder null references** for absent legacy slots. The fire-protection slot class
`0x2990C05A` is the class the ID table also names `cSC4ProtectionSimulator`.

#### 3.6.3 Auxiliary record and corpus proof

After the final flag, Write separately opens **`0x89D9B9EE / 0x69D9B9F1 / 0`, version 1**, using an auto-save
record. Its payload holds cheat-code save data; the full outer and inner layouts and load quirks are in
[save-records §2.1](save-records.md#21-city-auxiliary-cheat-state-record-compat). These bytes are never
appended to the city class entry. Retained-record writes also target separate DBPF records.

**SR33-3 proof, 2026-10-28.** All **547** available reference `build-runs/golden/<key>/end.sc4` files were read
in place. A throwaway parser consumed all 547 city class records (v12, **318,073 decompressed bytes** including
entry headers), all **28,991** component-reference slots and all 547 auxiliary records (v1, **14,222 bytes**,
including their inner cheat-data framing), with zero bytes left over in any record. The size equation in
§3.6.1 also matched every city payload. The city's inline property holders were empty in this corpus.
Legacy city gates and nonempty property holders follow the executable
and §10.2; this check is not a claim of legacy save fixtures.

**What a replacement subsystem must do to interoperate** *[compat]*:

1. Register a class factory for its class ID so that `GZCOM()->GetClassObject(classID, iid, &p)` creates it, both
   for new cities (the city asks for its own interface) and on load (the store asks for `GZIID_cIGZSerializable`,
   `0xE4FDA3D4`).
2. Answer `QueryInterface` for `cIGZSerializable` and for the IID in the table above.
3. Return its class ID from `GetGZCLSID()`. That value becomes the record type.
4. Accept `Read` on a fresh, un-initialised instance. `Read` happens **before** `Init` (§3.7).
5. Write only through the stream it is given, following its own subsystem spec's layout.

### 3.7 Save and load sequences

**Save** (`cSC4App::SaveCity`): the game writes a **new** package to a temporary file and then replaces the city
file with it (keeping the old one until the rename succeeds). In order:

1. Create the package. Send `kSC4MessagePreSave` `0x26C63343` (void1 = package segment, void2 and void3 = file
   path strings, data4 = the caller's flag), then `0x26C63346` with the same data (name unknown).
2. Write the app version record (65) and the module version record.
3. Write the root record. Its reference registers the city. *(The city's `Write`, and through it every subsystem's
   `Write`, runs later, in step 5.)* The same helper then writes the current city's regional-city record (§11.1)
   while the root record is still open, so the regional-city record precedes the root record in the file.
4. Send `kSC4MessageSave` `0x26C63344` and `kSC4MessagePostSave` `0x26C63345`.
5. Run the class-object store save (§3.5): the city, the 53 components and everything they reference.
6. Close the package, then rename it over the city file.
7. If the save succeeded: rewrite the regional-city record inside the file of every adjacent city (§11.1).

Subscribers to PreSave/Save may write their own records into the package (void1). They run **before** any
object's `Write`. The View3D record (`0xA9C8647C / 0xE9C8647E / 1`) is written this way during the Save message,
also before the root record closes, and it holds a ref to the simulator. That ref registers the simulator first,
which is why the simulator's class record comes first in every save that keeps the write order (410 of 417). A
replacement simulator must therefore be registered and serializable when the Save message is sent
(save-records.md §5) *[compat]*.

**Load** (`cSC4App::LoadCity`):

1. Open the package. The app version record must hold 62..65. If the record is missing it counts as 65. If it is
   present but unreadable the load is rejected.
2. Send `kSC4MessagePreLoad` `0x26C63340` (void1 = segment, void2 = path).
3. **Load the class-object store** (§3.5). Every saved object, including the city and all subsystems, is created
   and `Read` here, in ascending class ID order. If this step reports failure, the load fails.
4. Read the root record: `uint16` must be 1, then `uint16` must be 2, then the city reference.
5. Send `kSC4MessageLoad` `0x26C63341` and `kSC4MessagePostLoad` `0x26C63342`. **Send** (synchronously)
   `kSC4MessagePreCityInit` `0x26D31EC0`, run the city's `Init` (which initialises its components, the simulator
   first), then **post** (queued) `kSC4MessagePostAppServicesInit` `0x2B96B3EA`, `kSC4MessagePostCityInit`
   `0x26D31EC1`, `kSC4MessagePostAppServicesInitComplete` `0xAB96B05F` and `kSC4MessagePostCityInitComplete`
   `0xEA8AE29A`. These city messages carry void1 = IGZUnknown = the city.

The city's `Init` initialises components in this order (relevant for `Init`-time dependencies) *[inferred]*:
simulator, history warehouse, terrain, surface water, effects manager, automata script system, automata controller
manager, occupant manager, zone manager, lot configuration manager, zone developer, landfill / airport / seaport
developers, park manager, lot manager, 24-hour clock, prop manager, network manager, dispatch manager, prop
developer, network lot manager, **budget, ordinance, power, plumbing, neighbours, building development**, tract
developer, lot developer, **demand, aura, commercial, crime, fire protection, flammability, flora, industrial, land
value, police, traffic, pollution, residential, weather, civic building**, disaster layer, demolition utility,
traffic network map, vehicle / pedestrian / aircraft / watercraft managers, advisor system, city situation
manager, tutorial system, MySim agent simulator, city detail manager.

**When objects, grids included, are read (HG1, from the exe).** Every saved object is created and `Read` in step 3,
before any `Init`:
1. `LoadCity` runs the store load at `0x0045574C`, through `cSC4DBSegment::LoadClassObjects` (`0x005D2080`), into
   the store's loader (`cSC4COMSerializer`, `0x005D38D0`).
2. The loader puts the index's classes into a sorted map, so it walks them in ascending class ID whatever the file
   order. Within a class it reads the entries in record order, and it calls each object's `Read` at `0x005D3C63`.
3. A reference read inside some `Read` only creates the referenced object (empty, if it is new). The object's own
   `Read` runs when the walk reaches its entry.
4. Only after the store load come the root record, Load and PostLoad, PreCityInit (sent at `0x00455898`), and then
   the city's `Init` (`cSC4City::Init` `0x0045BD20`, called at `0x004558A2`), which initialises the simulators.

So every saved grid (classes `0x49B9E602`–`0x49B9E60B`, simgrid.md §2) has its saved geometry before any `Init`
runs. Its entry is read once, so nothing later in the load changes that geometry. Whether a subsystem's `Read` sees
its grids empty or already read depends only on its class ID:
- **below `0x49B9E602`, read before the grids:** power `0x0990C005`, plumbing `0x0990C075`, fire protection
  `0x2990C05A` (run 70's case), residential `0x4990C013` and others;
- **above it, read after the grids:** police `0x6990C04F`, pollution `0x8990C065`, crime `0x8990C09A`, land value
  `0xE990C18E` and others.

*[compat]* A `Read` must therefore not use a referenced grid's size, and `Init` may. The first non-empty geometry a
saved grid shows is its final one, except in a malformed save that lists the same object twice. One caveat for
FreeCity4's host grid: a grid saved as 0 × 0 stays empty after its `Read`, so "empty" alone cannot tell it from a
grid not yet read.

### 3.8 Retained records

When a city is loaded, every record in group **`0xAB85E27D`** (up to 10,000,000 bytes each) is kept in memory
and written back unchanged on the next save *[compat]*. *[choice]* FreeCity4 may use this group for its own
extension data. The original game will carry it through a save.

### 3.9 Unknowns

See §8, questions 1–5.

## 4. Response curves

Tuning properties named "… curve" (for example "Funding To Efficiency Response Curve") hold a flat `float32`
array `x0, y0, x1, y1, …` that the game evaluates as a **response curve**. One shared helper builds and evaluates
them (`cSC4ResponseCurve`). More than a hundred call sites use it, including the budget, demand, traffic,
neighbours and service simulators.

### 4.1 Loading from a property

- The property's values are taken as `float32` and paired up: `n = count / 2` points. An odd trailing value is
  ignored.
- *[compat]* Curve properties must be `float32` arrays. The original reinterprets the raw bytes of other value
  types as floats. A single value (a scalar, whose variant count is 0, or a one-element array) gives **zero**
  points. *[choice]* FreeCity4 converts integer arrays to
  float, and treats non-numeric or single-value properties as absent.
- Loading a **present** property first discards the curve's old segments, then builds the new ones (§4.2). It
  returns false when there are no points; the curve is then empty (§4.3, zero points).
- **A missing property: the behaviour is per caller** *[compat]* (corrects the earlier blanket rule "a missing
  property removes the curve"; see Errata, M3 item 1). The shared loaders never delete a curve object. What a
  missing exemplar or property does depends on what the caller does around the load, and there are four
  patterns. Each subsystem spec must say which pattern applies to each of its curves, and what an absent or empty
  curve means for it.

  | Pattern | What the caller does | Exemplar or property missing | Users (verified call sites) |
  |---|---|---|---|
  | **keep** | loads in place with the "by exemplar key and property ID" loader, which fetches the exemplar, looks up the property and loads only if both exist | the curve keeps its **previous** segments (empty on the first load); the loader returns false | budget, traffic, pollution's per-occupant curves |
  | **default** | first rebuilds the curve from a code-default point list, then loads as in "keep" | the curve holds the **code default** | crime, fire protection, police (curves with a code default; protection §6), power's five tuning curves (power §8). Plumbing's curve `0x6A410BF2` is a variant: "keep", then (0,0)(100,100) if the curve is still empty after the load (plumbing spec) |
  | **empty** | first empties the curve (or uses a fresh temporary one), then loads as in "keep" | the curve is **empty** | pollution's tuning curves (pollution §9), pollution's temporary curves in `SimulationBegin` / `EndOfCycle` |
  | **delete** | if the property is present, loads it (creating the curve object when needed); otherwise deletes its curve object. Flammability, and the aura's riot curve, delete first and then create a new object when the property is present; the aura's other 12 curves are reloaded in place (created when missing) or deleted when the property is missing (aura §10) | the curve is **absent** (no object) and the caller takes its "no curve" path | land value (landvalue §8), aura (all curves), flammability (`0x29B4013D`), demand (`0x4A1F38B5`, `0x4A1F38B6`); power's two per-building curves `0x8A1E07EE` / `0x0911E2E7` follow it per call, with a fresh temporary curve (power §8) |

  "Keep" differs from the others only when tuning is re-read (another `Init` after `Shutdown`, or a tuning-reload
  message) after the property disappeared, which shipped content never does. A present property with no points,
  or with a single value (the property-pointer loader `0x005F4620` makes `count >> 1` points), always gives an
  empty curve. *[choice]* FreeCity4 implements the pattern stated per curve, and treats an empty
  curve like an absent one only where the subsystem spec says so.

### 4.2 Construction *[compat]*

The point list is turned into an ordered list of **segments**. For each input point *i*, the predecessor is
input point *i − 1*. For *i = 0* the predecessor is `(−FLT_MAX, y0)`.

```
for i in 0 .. n−1:
    (x, y)   := point i
    (px, py) := predecessor (input point i−1, even if it was dropped)
    dy := y − py ;  dx := x − px                     (float32)
    if |dy| < 1e-10:            keep segment {x, y, px, py, slope = 0}
    else if dy · 1e-10 < dx:    keep segment {x, y, px, py, slope = dy / dx}     (slope stored as float32)
    else:                       drop the point
```

- Flat points are **always kept**, whatever their x.
- A point with the same x as its predecessor and a **higher** y is dropped. One with a **lower** y is kept with
  slope −∞, but §4.3 never selects it. Either way the curve makes a **vertical step** at that x.
- The predecessor is always the previous *input* point. After a dropped point, the next segment interpolates
  from the dropped point, not from the last kept one (example 7 in §4.4).
- Points are expected in ascending x. Out-of-order points that survive (only flat ones can) make the search in
  §4.3 operate on unsorted data. *[choice]* That case is undefined. Conformance tests do not cover it.

### 4.3 Evaluation *[compat]*

With `m` kept segments and `last = m − 1`:

```
k := the smallest index in 0 .. last whose x > input; k := last if there is none
if k = 0:                                   return y of segment 0
if k < last or input < x of segment last:   return py_k + (input − px_k) · slope_k
otherwise:                                  return y of segment last
```

- **Clamped at both ends.** Below the first x the output is `y0`, and from the last x on it is `y_last`.
- **Exact at the points.** At an interior point `x_i` the search selects the next segment, whose start is
  `(x_i, y_i)`, so the result is exactly `y_i`.
- **One point**: constant `y0`.
- **NaN input** *[compat]*: with two or more kept points the result is **NaN**, not an end value. The search
  compares with `test ah, 5; jp`, which treats an unordered compare like "input ≥ x", so it moves to the last
  segment; the final "input < x of segment last" test is `test ah, 1`, which an unordered compare passes, so the
  last segment is interpolated with a NaN input (`0x005F4250`). With one point the result is `y0`. FreeCity4: if
  the input is NaN and `m ≥ 2`, return NaN (a quiet NaN; callers only compare it or multiply it).
- **Zero points**: the original evaluator (`0x005F4250`) still reads the first segment. On a curve object that has
  never held points that is a null pointer, and the game crashes; on one that held points before an in-place
  reload it returns the stale first `y` *[inferred]*. *[choice]* Return 0.0. Callers should treat an empty curve as
  absent (aura §10, flammability §8).

Arithmetic: on the Windows build the expression `py + (input − px) · slope` is evaluated on the x87 at 24-bit
precision (§18): the subtraction, the product and the sum are each rounded to a 24-bit significand, i.e. three
`float32` operations. *[compat]* FreeCity4 evaluates it as three `f32` operations in that order. Tests compare
with a relative tolerance of 1e-6 (corrected: the earlier text assumed one rounding at the end).

### 4.4 Worked examples (shipped tuning values, `re/tuning/simulators.md`)

1. **Police funding → efficiency** `0x68DDAE93` = (0,0) (100,100) (120,110):
   f(−10) = 0, f(0) = 0, f(50) = 50, f(100) = 100, f(110) = 105, f(150) = 110 (clamped).
2. **Health quotient → life expectancy** `0x6941A67F` = (0,50) (20,55) (40,60) (60,65) (200,100):
   f(10) = 52.5, f(60) = 65, f(100) = 65 + 40 × 0.25 = 75, f(250) = 100.
3. **Crime, R$ jobless ratio → criminality** `0x8A0CBAA7` = (−1,0.1) (−0.7,0.07) (0,0) (0.7,−0.07) (1,−0.1):
   f(−2) = 0.1, f(−0.85) = 0.085, f(0.35) = −0.035, f(1.5) = −0.1 (all as float32).
4. **Jailbreak** `0xEA614F63` = (0,0) (80,0) (90,10) (100,100): f(50) = 0, f(85) = 5, f(95) = 55, f(100) = 100.
5. **Air pollution → HQ decay** `0x6941B022` = (0,0) (512,0.1) (1024,50): f(256) = 0.05, f(768) ≈ 25.05,
   f(2000) = 50.
6. **Land value → simoleons** `0x4AC519B8` = (0,0) (255,0): flat, so 0 everywhere. **Neighbour garbage cost**
   `0x4B61FFC9` = (0,1e−5) (0.0075,0.0075) (1,0.075): f(0) = 1e−5, f(0.5) ≈ 0.0409950, f(2) = 0.075.
7. Synthetic step (not shipped): (0,0) (5,10) (5,20) (10,30). The third point is dropped. The fourth interpolates
   from (5,20) with slope 2. f(4.999) ≈ 9.998, f(5) = 20, f(7.5) = 25.
8. Synthetic drop (not shipped): (0,0) (5,20) (5,10) (10,0): f(4.9) = 19.6, f(5) = 10, f(7.5) = 5.
9. Synthetic out-of-order x (not shipped): (0,0) (10,10) (5,20) (20,40). (5,20) is dropped, and the last segment
   starts from it: f(5) = 5, f(10) ≈ 26.667 (a jump), f(15) ≈ 33.333, f(20) = 40.

## 5. Message objects

### 5.1 The standard message (`cRZMessage2Standard`)

The simulation uses one concrete message class for clock messages, notifications and agent calls. Its class ID
is **`0x46DF45E7`** and it implements:

| Interface | IID |
|---|---|
| `cIGZMessage2Standard` | `0x65297976` |
| `cIGZMessage2` | `0xA52294B4` *[compat]* (confirmed: the standard message class answers `QueryInterface` for `1`, `0x65297976` and `0xA52294B4`) |
| `cIGZUnknown` | `1` |

You get one from `GZCOM()->GetClassObject(0x46DF45E7, 0x65297976, &p)`, from the message server's
`CreateMessage(0x46DF45E7, …)`, or from `Create(iid, &p)` on an existing message, which makes a new standard
message with the same **type**.

| Field | Initial value | Setter effect | `GetHas…` |
|---|---|---|---|
| type | 0 | `SetType(t)` | (internal "has type" bit = `t ≠ 0`) |
| data1 … data4 | 0 | `SetDataN(v)` and `SetVoidN(p)` write the **same** 32-bit slot | true once set, never cleared |
| string | none | `SetString(s)` stores a **reference** (`AddRef`), not a copy. Null clears it. | true while set |
| IGZUnknown | none | `SetIGZUnknown(o)` stores a reference. Null clears it. | true while set |
| extra | none | `SetExtra(s)` stores a reference to a `cIGZSerializable`. Null clears it. | true while set |

- `GetDataN` and `GetVoidN` return the slot unchanged. On the 32-bit game a pointer and an integer share it.
- `GetString(iid, &p)` and `GetExtra(iid, &p)` are really `QueryInterface(iid, &p)` on the stored object. They
  return false when nothing is stored. *[compat]* The SDK names the first parameter `dwMaxLen`. It is an
  interface ID (use `GZIID_cIGZString` `0x089B7DC8`).
- `operator==` and `operator<` compare **types** only (unsigned).
- Reference counting: a new message starts at count 0. The creator `AddRef`s it. `Release` destroys it when the
  count was 1 (or 0) and otherwise decrements. Destruction releases the string, IGZUnknown and extra references.

**Conventions used by specs** *[compat]*: "message (type T, data1 = a, data2 = b, data3 = c)" means a fresh
standard message with `SetType(T)` and `SetData1/2/3`. Unset fields stay 0. "void1 = X" means `SetVoid1(X)`.
"IGZUnknown = X" means `SetIGZUnknown(X)`.

**Delivery** (message server, `cIGZMessageServer2`):

- `MessageSend(m)`: synchronous. It calls `DoMessage(m)` on every target subscribed to `m`'s type, then on every
  target subscribed to type **0** (wildcard), each in subscription order. The targets are snapshotted before
  delivery. It returns true.
- `MessagePost(m, highPriority)`: queues `m` (the queue holds a reference) at the back, or at the front if
  `highPriority`. Queued messages are sent on a later message-server tick.
- The simulator's own pattern *[compat]*: create, `AddRef`, set type and data1..3, send or post, `Release`.

*[compat]* **SDK discrepancies.** The gzcom-dll `cRZMessage2Standard` implementation differs from the original. It
copies strings, does not implement `GetExtra`/`SetExtra`, and its `GetHasData2..4` are wrong because of a bit test
error. Our shim must implement its own message class or use the game's (through `GetClassObject`). It must not use
the SDK's implementation.

### 5.2 Shared IDs used by several simulators (resolved in M3)

These IDs appear in several subsystem specs. Their meaning is defined once here.

| ID | Kind | Meaning | Evidence |
|---|---|---|---|
| `0x029244DB` | occupant property, bool | **Preview occupant** *[inferred name]*. The lot-placement tool (the building model shown under the cursor before a lot is placed) and the toll-booth tool set it to true on the temporary occupants they create. Grid simulators ignore insert / remove / change messages for such occupants. Aura, crime, flammability and plumbing test only that the property **exists**; pollution tests that its value is **true**. The model maker also creates no shadow for them. | setters in the place-lot and toll-booth tools; readers in every grid simulator's occupant handler |
| `0xC53D10AA` | message | **Exemplar changed** *[inferred name]*. Posted (`MessagePost`, normal priority) by the exemplar resource code when an exemplar resource is written out, and by the exemplar debug view when it updates one. data1 = the exemplar's group, data2 = its instance. Each **subscribing** simulator re-reads its tuning when (data1, data2) equals its tuning exemplar's group and instance; the message is posted queued, so only subscribers see it. Flammability neither subscribes nor handles it (flammability §5). Shipped play never writes exemplars, so the message is not expected in a normal session; FreeCity4 must still handle it exactly for the simulators that subscribe. | senders: exemplar factory write path, exemplar resource view |
| `0xC6CDBF1A` / `0xC6CDBF1B` | message | **PreViewLevelChange / PostViewLevelChange** (names from the game's own message-name table). Sent synchronously (`MessageSend`) by the 3D view when its view level changes, Pre first and then Post. void1 points at the **old** level and void2 at the **new** level (u32 values, valid only during delivery). View levels: **1 = surface**, **2 = subway**, **3 = water pipes** *[inferred: the per-occupant view level is 2 for subway occupants and 3 for pipe occupants, otherwise 1]*. | sender: the 3D view's "set current view level"; readers: pollution (effects shown only when the new level is 1), landfill developer, pipe-burst disaster |
| `0x8A4260A6` | department ID | **Parks and Recreation**. The park manager creates this department budget at `Init` when it does not exist and names it with LTEXT group `0x6A3FF01C`, instance `0xCA49375E` ("Parks and Recreation" in `SimCityLocale.dat`). The cruise-ship reward building's exemplar also charges this department (budget-department property `0xEA54D283`), which is where the earlier "cruise ship" label came from. | park manager `Init`; exemplar `RW43x30_Cruise_Ship_Building_03BC` |

**Lot state and lot condition** (`cISC4Lot` compiled slots 33/34 `GetState`/`SetState` and 35/36
`GetCondition`/`SetCondition`) are two different bytes:

- State 4 is **abandoned**: the lot developer's "abandon building" sets state 4.
- The condition is updated by the lot developer's per-lot update. A lot in state 3 gets a condition in 0..3, never
  4. A lot in state 4 (abandoned) moves through conditions 1 → 2 → 3 → 4: it gets 1 at once, and then one more
  step each time its "days since last state change" exceeds the lot developer's threshold. `SetCondition` on an
  abandoned lot restarts that day counter, so each step takes the full threshold. Other states keep their
  condition.
- So **condition 4 means an abandoned lot at its last decay stage** *[inferred from the update rule; the enum has
  no names in the binary]*. Crime's arson mask (crime §7) and flammability's abandonment multiplier `0x29B40134` (flammability §6) apply
  only to such lots. Those specs read the **condition**, not the state.

## 6. Random numbers

### 6.1 Generators

The game has two generator classes. Both are exact algorithms that our engine must reproduce bit for bit
*[compat]* (§6.4 says why).

**`cRZRandom`** holds two 32-bit states, `lcg` and `mul` (both set to the seed), plus a cached-Gaussian flag:

| Operation | Definition |
|---|---|
| `Seed(s)` | `lcg := mul := s`. If `s = 0xFFFFFFFF`, s is first derived from the wall clock (non-deterministic): s := `time(NULL) XOR ((c << 16) \| c)` with `c = clock()` read once (`0x00913667`; RG21). |
| `NextRaw()` (`RandomUint32Uniform()`) | `t := lcg × 0x41C64E6D + 0x3039` computed in **64 bits**; `lcg := t mod 2^32`; result = `(t >> 16) mod 2^32`. The result's high 16 bits come from the upper half of the 64-bit product. |
| `RandomUint32Uniform(n)` | `n = 0` → 0, no state change. Otherwise `(NextRaw() × n) >> 32` (64-bit product). |
| `RandomSint32RangeUniform(lo, hi)` | `lo + RandomUint32Uniform(hi − lo)` (unsigned difference). |
| `RandomDoubleUniform()` | if `mul = 0`, `mul :=` the process clock. Then `mul := mul × 0x278DDE6D mod 2^32`; result = `int32(mul) / 2^32 + 0.5` in [0, **1]**, with 24-bit rounding (precision notes below). |
| `RandomDoubleRangeUniform(a, b)` | `a + RandomDoubleUniform() × (b − a)` |
| `RandomDoubleGaussian()` | Marsaglia polar method on two `2·RandomDoubleUniform() − 1` draws, repeated until `0 < s < 1`. It returns one value and caches the other for the next call. |
| `RandomDoubleRangeGaussian(a, b)` | `g := Gaussian + 3`; if not `g < 6`, `g := 5.9999`; if `g < 0`, `g := 0`; mapped `a + (b − a) · (g × 1/6)` (operation order below; g = 5.9999 gives 0.9999833 after scaling) |
| `RandomDoubleRangeGaussianFast(a, b)` | `a + ((u3 + u2) + u1) · (b − a) × 1/3` with three `RandomDoubleUniform` draws u1, u2, u3 in draw order |
| `RandomSint32RangeGaussianFast(lo, hi)` | always exactly 4 `NextRaw()` draws `r1..r4`: `a = (r1 + r2 + r3 + r4 + 2) >> 2` with the sum in 64 bits, then `lo + ((a × (u32)(hi − lo)) >> 32)`. It draws 4 times even when the range is empty. Test: seed 1, `(0, 25)` gives 10, then 7 (plumbing spec) |

The integer methods use only `lcg`. The double methods use only `mul`. A generator seeded with 0 has a
deterministic `lcg`, but its double methods fall back to the clock.

**`cRZRandomSuper`** is a Mersenne Twister (MT19937: N = 624, M = 397, matrix `0x9908B0DF`, standard tempering)
with a **non-standard seeding**:

```
x := seed | 1          (seed 0xFFFFFFFF → derived from the wall clock first)
for i in 0 .. 623:
    state[i] := (x & 0xFFFF0000) | ((x · 69069 mod 2^32) >> 16)
    x := (x · 0x1C587629 + 0x10DCE) mod 2^32           (= two steps of x·69069 + 1)
regenerate all 624 words (standard MT twist); next output is word 0
```

**Bit 0 of the seed is ignored** *[compat]* (RN1; `Seed` `0x00913A72`, the `or eax, 1` at `0x00913A97`). The OR comes
after the clock test, so it applies to a given seed and to a clock-derived one alike. Seeds `s` and `s XOR 1` give
identical streams. Every `cRZRandomSuper` therefore has only 2^31 distinct streams, and the clock value's bit 0 is
lost. `cRZRandom::Seed` (`0x00913667`) stores all 32 bits in both states, so the effect is limited to the
`cRZRandomSuper` users of §6.2 and §6.3: the file-local generators (budget, crime, pollution effects, lots, network
lots and the rest of that list) and any other caller that seeds a `cRZRandomSuper`, such as the park generator.
A test that varies the session seed to prove a `cRZRandomSuper` user depends on it must change a bit other than
bit 0 (for example `S XOR 2`). The same applies to the `derive` fix-up of §6.3, whose XOR 1 is invisible to these
generators.

| Operation | Definition |
|---|---|
| `RandomUint32Uniform()` | next tempered word |
| `RandomUint32Uniform(n)` | `(next × n) >> 32` |
| `RandomSint32RangeUniform(lo, hi)` | `lo + ((next × (hi − lo)) >> 32)` |
| `RandomDoubleUniform()` | `int32(next) / 2^32 + 0.5`, rounded as for `cRZRandom` (below) |
| `RandomDoubleRangeUniform(a, b)` | `a + RandomDoubleUniform() × (b − a)` |
| `RandomUint32WeightedChoice(n, w[])` | n < 2 → 0. Otherwise, with `S` = sum of the positive weights: if `S = 0` → `RandomUint32Uniform(n)`. Otherwise `r := RandomDoubleUniform() × S`, walk the positive weights subtracting until `r < w[i]`, return i (n − 1 if none). |

**Precision of the double methods** *[compat]* (§18). The generators run on the caller's thread, so for all
simulation callers every floating-point step is rounded to 24 bits like any other simulation arithmetic. The
"double" results are therefore `float32` values (returned in a `double`). `r24(·)` below is one rounding to a
24-bit significand, round-to-nearest-even; `a` and `b` are the caller's `double` arguments and enter unrounded.

| Operation (both classes where it exists) | Exact sequence |
|---|---|
| `RandomDoubleUniform` | `x := r24(i × 2^-32)` with `i` the signed 32-bit value (`mul`, or the Mersenne Twister word); `u := r24(x + 0.5)`. The scale constant is `float` 2^-32 in `cRZRandom` (`0x0091372E`) and `double` 2^-32 in `cRZRandomSuper` (`0x00913A29`); both are exact, so the two classes round the same way |
| `RandomDoubleRangeUniform` | `r24(r24(u × r24(b − a)) + a)` |
| `RandomDoubleGaussian` (`0x00913777`) | `v1 := r24(2·u1 − 1)`, `v2 := r24(2·u2 − 1)` (u1 drawn first); `s := r24(r24(v2·v2) + r24(v1·v1))`; redraw both if `s ≥ 1` or `s = 0`. `f := sqrt(r24(r24(ln(s) / s) × −2))`, the square root rounded to 24 bits. Return `r24(f × v2)`; cache `r24(v1 × f)` for the next call |
| `RandomDoubleRangeGaussian` (`0x0091381F`) | `g := r24(G + 3)`; the clamps of the table above (5.9999 is the `double` constant, unrounded); result `r24(r24(r24(b − a) × r24(g × 1/6)) + a)`, where 1/6 is a `double` constant |
| `RandomDoubleRangeGaussianFast` (`0x0091386B`) | `r24(r24(r24(r24(r24(u3 + u2) + u1) × r24(b − a)) × 1/3) + a)`, where 1/3 is a `double` constant |
| `RandomUint32WeightedChoice` (`0x00913AD3`) | `S` sums the positive `double` weights in index order, each addition rounded; `r := RandomDoubleRangeUniform(0, S)`; the walk subtracts with `r := r24(r − w[i])` |

Consequences:

- **`RandomDoubleUniform` can return exactly 1.0.** The first rounding maps `i` to 24 significant bits, and the
  addition rounds again: the 191 values `i ≥ 2^31 − 191` give 1.0 (probability ≈ 4.4·10^-8 per draw);
  `i = 2^31 − 192` gives `1 − 2^-24`. At the other end, `i` near `−2^31` gives 0. Callers that scale `u` by a size
  or a count can therefore reach the upper bound itself; each subsystem spec follows the original there.
- The logarithm in `RandomDoubleGaussian` is the C runtime's `log` (`0x009F4DF0`), whose result is **not**
  affected by the precision control: on CPUs with SSE2 (every CPU except Intel family 15, the Pentium 4 line, which
  takes the x87 `fyl2x` path) it is the runtime's SSE2 double-precision logarithm. FreeCity4 computes `ln(s)` in
  `double` and rounds only the division. The result can differ from the original only when `ln(s)/s` falls within
  a `double` rounding error of a `float32` rounding boundary (about 2^-29 per draw) *[choice]*.
- The square root (`0x009F1D04`) keeps the caller's precision (§18.1), so it is a 24-bit `fsqrt`.

### 6.2 Which generator the simulation uses

There is **no** single simulation RNG. Each subsystem uses its own instance:

| Generator instance | Seed | Used by (examples) |
|---|---|---|
| a `cRZRandom` member of the subsystem | wall clock (`0xFFFFFFFF`) at construction | residential, weather, flora, power, aura, fire protection, police, zone manager, advisor system (advisors.md §9), each MySim (mysims.md); plumbing (created at `Init`). The zone manager's member is never drawn (zones.md) |
| a `cRZRandom` member | **0** (integer path deterministic, double path clock-seeded) | flammability |
| a file-local `cRZRandomSuper` per source file | local wall-clock seconds XOR a per-file constant | budget (the `RiskyMoney` disaster roll), crime, pollution effects, lots, network lots (full list in §6.3) |
| a file-local `cRZRandom` per source file (seeded once at static initialisation, never re-seeded) | `cRZTime()` local wall clock XOR a per-file constant | building development simulator (`0x00B485F0`, XOR `0x0C09C0B8`; building-development.md §4.14), growth developers (`0x00B49080`, XOR `0xAC09C0E8`, shared by all of them; growth-developer.md §7), tract developers (`0x00B4AFD8`, XOR `0x4C09C113`, shared; tract-developer.md §4.22), zone developer (`0x00B4B398`, XOR `0x8C09C68A`; zones.md), lot developer (`0x00B49AF0`, XOR `0x6C09C0F4`; lot-developer.md) |
| a temporary `cRZRandom` | `((x << 16) + z) << 4` XOR a per-month salt | growth developer decline-list ordering (growth-developer.md §7) |
| a temporary `cRZRandom` | `((tx << 16) + tz) << 4` (tract (0, 0) seeds 0, so its multiplicative path is clock-seeded) | tract developer `UpdatePopulation` (tract-developer.md §4.18) |
| a `cRZRandomSuper` member | wall clock at `Init` | traffic simulator, traffic network map |
| a `cRZRandomSuper` member | wall clock × 28 (mod 2³²) **at construction** (`0x006F34A3`–`0x006F34B1`), not at `Init` | prop developer (prop-developer.md §5; re-specs10) |
| a `cRZRandomSuper` member | wall clock (seed −1) at construction | city situation manager, draws only during a mission (city-situation.md §5; re-specs10); park manager (`0x006D5855`; parks.md §5) |
| a process-wide `cRZRandom` (lazy) | wall clock | neighbours deal propositions |
| a temporary `cRZRandom` | **cell coordinates** `(x << 16) + z` | lot developer desirability test (one `RandomDoubleRangeUniform(0, 7)`): deterministic per cell (lot-developer.md) |
| a temporary `cRZRandom` | wall clock | civic building monthly simulation |
| the C runtime `rand()` | state 1 at process start, **never seeded** by the game (§12) | `std::random_shuffle` of power poles, pipes, props and lot lists; landfill tiles; the random cell iterators (§12.5) |

- **No RNG state is saved** in any record specified so far (simulator, budget, demand), and seeding never reads
  the save. Loading the same save twice in the original gives different random streams *[compat]*: a saved city
  has no defined random future.
- *[compat]* Each subsystem spec must name the generator instance and the exact method (and therefore the number
  of state advances) at every random draw.

### 6.3 Seeding in FreeCity4 *[choice]*

- The core takes one 32-bit **session seed** `S` from the host (`fc4.h`) and derives every generator's seed from it
  with a fixed per-generator constant `K`:

  ```
  derive(K) := S XOR K;   if derive(K) = 0 or derive(K) = 0xFFFFFFFF: derive(K) := derive(K) XOR 1
  ```

  The fix-up avoids the two values that make the original fall back to the clock (`Seed(0xFFFFFFFF)`, and a zero
  multiplicative state, §6.1). The shim takes `S` from the wall clock like the original, unless a test or replay
  fixes it.
- **Which `K`**, by the kinds of §6.2:
  - *File-local generators* (seeded once at static initialisation with the clock XOR a constant): `K` = the
    original's constant, so `derive(K)` is exactly what the original computes with `S` in place of the clock. The
    table below lists all of them. They are **process-wide**: the original seeds each once per process and never
    again, so its stream continues across city loads in one session *[compat]*. FreeCity4 seeds each once per
    session and keeps it across city loads, wherever the core keeps the generator (RN3: the budget's `0x00B47B78`
    is referenced only by its static initialiser `0x00A749D2` and the draws at `0x00693544`–`0x006935C6`, the
    pollution effects' `0x00B49BD0` only by `0x00A74E42` and the draws at `0x006E668D`, `0x006E66DC`; neither is
    re-seeded).
  - *Member generators seeded from the clock* (`0xFFFFFFFF` at construction, or the clock at `Init`): `K` = the
    owning class's GZCLSID. Classes with several instances (MySims) use `GZCLSID XOR i`, where `i` is the instance's
    creation index from 0. A further transformation the original applies to the clock value (the prop developer's
    × 28) is applied to `derive(K)` instead.
  - *Members seeded with a constant* (flammability: 0) keep the constant. Where a zero multiplicative state later
    reseeds from the clock (§6.1), FreeCity4 uses `derive(GZCLSID)` for that reseed.
  - *Temporaries seeded from the clock* (civic building monthly simulation, the neighbours' process-wide
    generator): `K` = the owner's GZCLSID XOR `n`, where `n` counts earlier creations at that site by the
    owning object, from 0 when the object is constructed (not saved; a newly loaded city starts again at 0;
    QUESTIONS CB14).
    A temporary seeded from coordinates whose seed happens to be `0xFFFFFFFF` also reads the clock in the original
    (`cRZRandom::Seed`, §6.1), so it is one of these: the growth developer's decline-sort key (growth-developer.md
    §7) uses `K` = `0x0990C138 XOR n`, with `n` counting only those clock reads by the developer object (RN2). The city
    detail manager's increase pass is another (city-detail.md §3.5; per manager object, CD5).
  - *Temporaries seeded from coordinates*: unchanged, because they are deterministic already. The one case whose
    seed is 0 (tract developer `UpdatePopulation` for tract (0, 0)) uses `0x00000001` *[choice]*
    (tract-developer.md §4.18).
  - The C runtime `rand()` is not seeded by the game and stays unseeded (§12).

**File-local generators of the Windows build** (static initialisers at `0x00A71000`–`0x00A75300`; found by scanning
for "clock XOR constant → generator constructor"). "Clock" is the same value for all of them: each calls the
`cRZTime()` constructor `0x00470B50`, whose value (seconds since 1901 rebuilt from the local calendar) crime.md §9
specifies (CQ21):

| Static | Class | `K` | Used by |
|---|---|---|---|
| `0x00B485F0` | `cRZRandom` | `0x0C09C0B8` | building development simulator |
| `0x00B49080` | `cRZRandom` | `0xAC09C0E8` | growth developers |
| `0x00B4AFD8` | `cRZRandom` | `0x4C09C113` | tract developers |
| `0x00B49AF0` | `cRZRandom` | `0x6C09C0F4` | lot developer |
| `0x00B4B398` | `cRZRandom` | `0x8C09C68A` | zone developer |
| `0x00B49120` | `cRZRandomSuper` | `0x0C09C461` | `cSC4Lot` |
| `0x00B47B78` | `cRZRandomSuper` | `0x6C09C3D7` | budget simulator (`DoMessage`) |
| `0x00B48630` | `cRZRandomSuper` | `0x6C09C3F6` | crime simulator |
| `0x00B49BD0` | `cRZRandomSuper` | `0x4C09C4D3` | pollution simulator (visual effects) |
| `0x00B4A5C0` | `cRZRandomSuper` | `0x4C09C51F` | wealth requester (prop selection) |
| `0x00B44870` | `cRZRandomSuper` | `0x8C09C379` | network lot manager |
| `0x00B43E88` | `cRZRandomSuper` | `0x6C09C07B` | network construction crew |
| `0x00B22FB0` | `cRZRandom` | **`0x00000000`** | construction occupant. The static seed (clock XOR `0xEC09C32A`) is overwritten before any draw: the first construction occupant's `Init` calls `StaticInit` (`0x005CED80`, once per process), which reseeds it with the plain `cRZTime()` value (`0x005CEDAC`). So `K` = 0 (`derive(0) = S`) |
| `0x00B23078` | `cRZRandom` | `0x8C09C2D5` | lot base-texture occupant |
| `0x00B230C8` | `cRZRandom` | `0x0C09C347` | lot retaining-wall occupant |
| `0x00B231B0` | `cRZRandomSuper` | `0xAC09C2EC` | prop occupant |
| `0x00B21648` | `cRZRandom` | `0xEC09C147` | fire occupant (flames) |
| `0x00B22AE8` | `cRZRandom` | `0x6C09C1AB` | earthquake disaster |
| `0x00B22B60` | `cRZRandom` | `0xEC09C1D1` | pipe-burst disaster |
| `0x00B22B80` | `cRZRandom` | `0x4C09C187` | riot disaster |
| `0x00B21D78` | `cRZRandomSuper` | `0x4C09C170` | automata |
| `0x00B217C0` | `cRZRandom` | `0xAC357AF9` | region creation |
| `0x00B22BE8`, `0x00B22C10`, `0x00B22C38`, `0x00B22DF8` | `cRZRandom` | `0xAC09C1EF`, `0xAC09C20E`, `0xAC09C22B`, `0xEC09C261` | brush, decal, dynamic-particle and geometry-particle effects (rendering) |
| `0x00B22E58` | `cRZRandom` | `0x2C09C27B` | advice |
| `0x00B4BC78`, `0x00B4D530` | `cRZRandom` | `0x4C09C543`, `0x8C09C57A` | terrain, water ripples (rendering) |
| `0x00B228A0`, `0x00B22E28`, `0x00B4B3D0`, `0x00B4C7C8`, `0x00B4D5D0` | `cRZRandom` | `0x6C09BFC3`, `0xCC09C24E`, `0x6C09C5C8`, `0xAC09C558`, `0xAC09C596` | no reader found (constructed only) |

  The owners come from the functions that read each static (`re/names_windows.tsv`); several are UI or rendering
  and only matter beyond M5.
- Generators are separate objects per subsystem, exactly as in §6.2. Merging them would change the sequence each
  subsystem sees.

### 6.4 Differential testing *[choice]*

Because the original seeds from the clock, exact differential runs need the observer plugin to **force seeds**
in the original (hooking the generators' seed operation) and log each subsystem's draws. With both sides seeded
identically and the algorithms of §6.1, the draw sequences must match exactly. Without forced seeds, compare only
RNG-free quantities (clock, budget arithmetic) or distributions (growth statistics, milestone M4).

## 7. Conformance tests (must exist in `engine/conformance`)

1. **Primitives**: each `Set*` writes the bytes of §2.2 (for example `SetUint32(0x11223344)` →
   `44 33 22 11`; `SetFloat32(1.0)` → `00 00 80 3F`; `SetFloat64(1.0)` → `00 00 00 00 00 00 F0 3F`); round trip
   for edge values (0, −1, min/max, NaN bit patterns, −0.0).
2. **Sticky error**: on a 6-byte record, `GetUint32` succeeds, the next `GetUint32` fails without consuming, the
   destination is unchanged, `GetError() ≠ 0`, and a following `GetUint8` also fails.
3. **Strings**: `"Fire Dept."` ↔ `0A 00 00 00 46 69 72 65 20 44 65 70 74 2E`; empty string ↔ `00 00 00 00`; a
   UTF-16 header `0x10000002` followed by `41 00 E9 00` reads as UTF-8 `"Aé"`; count 200,001 fails;
   `GetRZCharStr` truncation without terminator.
4. **Resource key**: `{type 1, group 2, instance 3}` ↔ `01 00 00 00 02 00 00 00 03 00 00 00`.
5. **References**: null ↔ `00 00 00 00`; object (ID 5, class `0x8990C12D`) ↔ `05 00 00 00 2D C1 90 89`; two
   references to the same object resolve to one instance; a cycle A↔B round-trips; an unknown class ID fails.
6. **CRC**: `"123456789"` → `0x0376E6E7`; empty input → `0xFFFFFFFF`; bytes `01 00 00 00 01 00` → `0xF34B70D6`.
7. **Class record**: an object with ID 1 whose `Write` emits `01 00` gives the entry
   `0E 00 00 00 D6 70 4B F3 01 00 00 00 01 00`. The index for that store alone is
   `01 00 00 00 01 00 00 00 01 00 00 00` + class ID + `01 00 00 00`. Payloads over 250,000 bytes use the truncated
   CRC.
8. **Store save order**: classes ascending, objects ascending within a pass, objects registered during a pass
   appended in a later pass; one object referenced twice is written once.
9. **Store load**: forward and backward references resolve. A `Read` that under-reads or over-reads within its
   entry does not disturb the next entry. A CRC mismatch stops that class only. An unregistered class's
   entries are skipped. Write → load → write is byte-identical (with IDs renumbered consistently).
10. **City references**: a stub city with 53 slots writes and reads them in the order of §3.6. The lot manager
    slot may refer to the same object as the zone developer.
11. **Response curves**: every example in §4.4 (tolerance 1e-6 relative), plus one point (constant), odd value
    count (last value ignored), and zero points (0.0, curve treated as absent).
12. **Messages**: `SetVoid1(p)` then `GetData1()` returns p; `GetHasData1..4` false until set; `SetString(null)`
    clears; `Create` copies the type only; `==` compares type only; reference counting frees at the last
    `Release`.
13. **`cRZRandom`**: seed 1 → `RandomUint32Uniform()` gives `0x000041C6`, `0x59D4967E`, `0xBF022781` (lcg states
    `0x41C67EA6`, `0x967EB0E7`, `0x2781E494`). Seed 1 → `RandomDoubleUniform()` gives 0.6545084714889526,
    0.48173871636390686, 0.8512561321258545 (mul states `0x278DDE6D`, `0xFB533A69`, `0x59EBECB5`; full `double`
    arithmetic would give 0.6545084968674928, 0.48173871100880206, 0.851256174268201, which is wrong under §18).
    Seed 1 → `RandomDoubleRangeUniform(0, 100)` gives 65.45084381103516. Seed 1 →
    `RandomDoubleRangeGaussianFast(0, 10)` gives 6.625010967254639.
    `RandomUint32Uniform(0)` returns 0 without advancing.
14. **`cRZRandomSuper`**: seed 1 → first words of the initial table `0x00000001`, `0x1C59C359`; first outputs
    `0xD3F6B9E5`, `0xEF284157`, `0x0D3639C7`. Seed 4357 → `0xAAE64AC3`, `0x5E7C47DA`, `0xB98E8121`. These vectors
    were computed from §6.1 and must be confirmed against the original (open question 6).
15. **Weighted choice**: all weights ≤ 0 falls back to uniform; weights {0, 1, 0} always pick 1.
16. **Uniform rounding** (§6.1): `i = 0x7FFFFF41` (2^31 − 191) → `RandomDoubleUniform` = 1.0; `i = 0x7FFFFF40` →
    0.99999994039535522 (1 − 2^-24); `i = 0x80000000` → 0.0. Clamped Gaussian: `g = 5.9999` with `(a, b) = (0, 10)`
    → 9.999833106994629.

## 8. Open questions

1. **Load result quirk** (§3.5): does the shipped game really accept a save in which an early class fails, as the
   analysis suggests? A save with a deliberately corrupted early class record would settle it.
2. **Text encoding** of `SetGZStr` for non-ASCII city, mayor and department names. A save with an accented city
   name would confirm UTF-8 (or reveal a code page).
3. ~~**Variant tags** (§2.8)~~: answered in §9 (tag table) and §10 (property holders, the records that use them).
4. **Other city-file records** (§3.2): region, thumbnail and terrain records, and unnamed message `0x26C63346`.
   The auxiliary `0x89D9B9EE/0x69D9B9F1` record is answered in save-records §2.1 (SR33-3).
5. ~~**C runtime `rand()`** seeding~~: answered in §12 (never seeded by the game; algorithm and users listed).
6. **RNG vectors** (§7 items 13–14): confirm with a runtime capture from the original (force a seed and log the
   first outputs).
7. **Object `Init` order on load** (§3.7): confirmed only from the city's `Init` list. Which `Init`-time
   cross-subsystem calls depend on it is for each subsystem spec.

### Addendum: message vtable slot order *[compat]* (verified against the game)

The standard message class's vtable matches the gzcom-dll `cIGZMessage2Standard` declaration order exactly. The order is: QueryInterface, AddRef, Release, Create, GetType (`+0x10`), SetType, `==`, `<`. Then, for data slots 1-4, `GetHasDataN`/`GetDataN`/`GetVoidN`/`SetDataN`/`SetVoidN`; data 1 starts at `+0x20`. Then String ×3, IGZUnknown ×3 (`GetIGZUnknown` at `+0x80`) and Extra ×3. Game agents read messages through `+0x10`, `+0x20`, `+0x24`, `+0x3C` and `+0x80`.

## 9. Variants (`cIGZVariant`)

Sources: the gzcom-dll `cIGZVariant` header (in-memory type numbers), the original reader and writer, and the
bytes of property holders in sample saves (§10.6), which confirm the marker, the tag width and the tag values
for `Uint8`, `Uint32`, `Sint32`, `Sint64`, `Float32` and the `Uint16`, `Uint32` and `Float32` arrays.

### 9.1 In-memory types *[compat]*

A variant holds one value or an array of values of one type. `GetType()` returns a 16-bit **memory type**; bit 15
(`0x8000`) marks an array, whose length is `GetCount()`. The numbers are those of the SDK header and are what
command parameter sets (`cIGZCommandParameterSet`), messages, Lua glue and property values see:

| Memory type | Name | Memory type | Name |
|---|---|---|---|
| `0x00` | Empty | `0x09` | Sint64 |
| `0x01` | Bool | `0x0A` | Float32 |
| `0x02` | Uint8 | `0x0B` | Float64 |
| `0x03` | Sint8 | `0x0C` | Char |
| `0x04` | Uint16 | `0x0D` | RZUnicodeChar (UTF-16 unit) |
| `0x05` | Sint16 | `0x0E` | RZChar (8-bit char) |
| `0x06` | Uint32 | `0x0F` | VoidPtr |
| `0x07` | Sint32 | `0x10` | IGZUnknown (an object reference) |
| `0x08` | Uint64 | `0x8000 \| t` | array of `t`; `0x8000` alone = raw bytes ("void array") |

A **string** value is an `RZChar` array (`0x800E`) whose count is the string length (no terminator) *[compat]*.
Exemplar string properties load as this type.

For the bounded original construction, storage, replacement, alias and clone lifetime contract,
see [variant-storage.md](variant-storage.md). This is separate from the type/slot inventory and
does not claim completion of the 127 methods.

### 9.2 On-disk encoding (`WriteVariant` / `ReadVariant`) *[compat]*

```
uint32  marker          0xA9BEE9AC if the memory type is exactly IGZUnknown (0x0010), otherwise 0
if marker = 0xA9BEE9AC:
    ref   object        (§2.7) the held object QueryInterface'd to cIGZSerializable; null if none or not serializable
else:
    uint32  diskTag     from the table below (array bit 0x8000 kept)
    value               scalar: one element.   diskTag 0x800C: a string (§2.3).
                        other arrays: uint32 count, then count elements.  diskTag 0: nothing.
```

The **disk tag is a different numbering** from the memory type. It is the property type numbering of exemplar
files (`diskTag << 8` = the exemplar's type code: `0x0100` Uint8, `0x0300` Uint32, `0x0900` Float32, `0x0B00`
Bool, `0x0C00` string, ...).

| Memory type | Disk tag | Element bytes | Read back |
|---|---|---|---|
| Empty `0x00` | `0x00` | none | yes (empty variant) |
| Bool `0x01` | `0x0B` | 1 (0 or 1) | yes; any non-zero byte is true |
| Uint8 `0x02` | `0x01` | 1 | yes |
| Sint8 `0x03` | `0x05` | 1 | yes |
| Uint16 `0x04` | `0x02` | 2 | yes |
| Sint16 `0x05` | `0x06` | 2 | yes |
| Uint32 `0x06` | `0x03` | 4 | yes |
| Sint32 `0x07` | `0x07` | 4 | yes |
| Uint64 `0x08` | `0x04` | 8 | yes |
| Sint64 `0x09` | `0x08` | 8 | yes |
| Float32 `0x0A` | `0x09` | 4 (IEEE bits) | yes |
| Float64 `0x0B` | `0x0A` | 8 (IEEE bits) | yes |
| Char `0x0C` | `0x11` | 1 | **no** *[quirk]* |
| RZUnicodeChar `0x0D` | `0x10` | 2 | **no** *[quirk]* |
| RZChar `0x0E` | `0x0C` | 1 | yes |
| VoidPtr `0x0F` | `0x12` | write fails | — |
| IGZUnknown `0x10` | (marker path) | `ref` | yes |
| void array `0x8000` | `0x8000` | count raw bytes | yes |
| Bool array `0x8001` | `0x800B` | 1 each | yes |
| Uint8 … Float64 arrays `0x8002`–`0x800B` | `0x8001`, `0x8005`, `0x8002`, `0x8006`, `0x8003`, `0x8007`, `0x8004`, `0x8008`, `0x8009`, `0x800A` (same order as the scalar rows) | as the scalar | yes |
| Char array `0x800C` | `0x8011` | 1 each | yes |
| RZUnicodeChar array `0x800D` | `0x8010` | 2 each | yes |
| RZChar array (string) `0x800E` | `0x800C` | §2.3 string: `uint32` header + bytes | yes |
| VoidPtr array `0x800F`, IGZUnknown array `0x8010` | `0x8012`, `0x800D` | write fails | — |

Details *[compat]*:

- **String arrays** (`0x800C`) have no separate count: the §2.3 header is the count. For 8-bit text the bytes are
  therefore identical to "count, then count bytes". The reader applies §2.3 completely (including the UTF-16
  header `0x10` and its quirks) and stores the result as an `RZChar` array.
- **Write failures**: for the unsupported types the marker and the disk tag are written first, then the call fails
  and the stream enters the error state (§2.1). Writers must not store `VoidPtr` values or arrays of `VoidPtr` or
  `IGZUnknown` in anything that is saved.
- *[quirk]* `Char` and `RZUnicodeChar` **scalars** are written (tags `0x11`, `0x10`) but the reader does not accept
  these tags. Reading one fails. Writers should store such values as `Uint8` / `Uint16` *[choice]*.
- **Reader tag set**: scalar tags `0x00`–`0x0C`; array tags `0x8000`–`0x800C`, `0x8010`, `0x8011`. Any other tag
  fails (stream error).
- **Marker on read**: only `0xA9BEE9AC` selects the object path; *[quirk]* **any** other value, not just 0, is
  treated as 0. *[choice]* Standalone readers may reject markers other than 0 and `0xA9BEE9AC`.
- **Object path on read**: read the reference (§2.7, without `loadNow`). A null reference succeeds and leaves the
  destination variant **unchanged** *[quirk]*. Otherwise the variant becomes an `IGZUnknown` holding the object.
  Every reader in the game reads into a **freshly constructed (Empty)** variant, so a null reference in practice
  yields an Empty value. A property holder then drops it (§10.2).
- **Array counts** have no upper limit in the original. *[choice]* FreeCity4 rejects a count whose elements
  would extend past the end of the record.
- The array path stores the **elements only**. There is no terminator, alignment or padding.
- `WriteVariant`/`ReadVariant` exist only on the segment streams (§2.1). The `<<` / `>>` helpers that property code
  uses ask the stream for the segment interface. *[quirk]* On a plain stream they write or read **nothing** and
  do not fail.

### 9.3 Worked examples

| Value | Bytes |
|---|---|
| Uint32 7 | `00 00 00 00` `03 00 00 00` `07 00 00 00` |
| Float32 6.0 (sample save) | `00 00 00 00` `09 00 00 00` `00 00 C0 40` |
| Sint64 50 (sample save) | `00 00 00 00` `08 00 00 00` `32 00 00 00 00 00 00 00` |
| Bool true | `00 00 00 00` `0B 00 00 00` `01` |
| Uint16 array {0x5C, 0x253} | `00 00 00 00` `02 80 00 00` `02 00 00 00` `5C 00 53 02` |
| Float32 array {100.0} (sample save) | `00 00 00 00` `09 80 00 00` `01 00 00 00` `00 00 C8 42` |
| string "Abc" | `00 00 00 00` `0C 80 00 00` `03 00 00 00` `41 62 63` |
| Empty | `00 00 00 00` `00 00 00 00` |
| IGZUnknown, null | `AC E9 BE A9` `00 00 00 00` |
| IGZUnknown, object ID 5 of class `0x8990C12D` | `AC E9 BE A9` `05 00 00 00` `2D C1 90 89` |

## 10. Properties and property holders in saves

### 10.1 Property (`cISCProperty`) *[compat]*

A property is a 32-bit **property ID** plus a variant value (§9). `Write(stream)`:

```
uint32   propertyID
variant  value         (§9.2)
```

`Read` reads the same fields into the property. Both return "the stream is not in the error state" (§2.1).

### 10.2 Plain holder (`cISCPropertyHolder`, the "property holder record") *[compat]*

In memory a holder keeps its properties **sorted by ascending unsigned property ID**, at most one per ID.
Adding a property whose ID is already present **replaces** that property in place. Adding never fails. The
`bool` argument of the SDK's `AddProperty(cISCProperty*, bool)` has no effect.

**Ownership: the holder keeps objects, not copies** *[compat]* (run r50e2, 2026-10-14). Any holder that game code
can reach must follow this, including a FreeCity4 object's own holder:
- `AddProperty(cISCProperty* p, bool)` (slot 15) stores **`p` itself** and AddRefs it. It does not copy it.
- Callers go on using `p` after the call, even after they Release their own reference.
- `GetProperty(id)` (slot 9) returns the stored object **borrowed** (no AddRef). The callers never Release it.
- The property's `GetPropertyValue()` (slot 5) returns its variant borrowed.
- Callers write the value **in place** through the variant's data pointers: the u16-array data getter (variant
  slot 99) and the u32-array data getter (slot 103). They also replace the data through the copying setter
  (slot 102, `SetValUint32(data, count)`).
- So the stored object, its variant and that variant's data must stay the same objects for the life of the
  holder entry.

Example: the traffic simulator's element store (traffic-simulator.md §4.2) on a holder without `0x8A0B4100`:
1. It creates the game's own property and gives it a 12-element u16 array.
2. `SetPropertyID(0x8A0B4100)`, then `holder.AddProperty(p, false)`, then `p.Release()`.
3. `p.GetPropertyValue()`: the variant's type, then its data getter, and a write to one element.

A holder that copies `p` frees it at step 2 (the caller held the only reference), and step 3 calls through freed
memory. Later readers (`GetProperty(0x8A0B4100)` → value → data getter) must also see the element written at step 3.

```
uint16  version = 2
uint32  count
count × {
    uint32    key          the ID under which the holder files the property (= its propertyID in every sample)
    property  (§10.1)      uint32 propertyID; variant
}
```

- **Write**: entries in the holder's order (ascending ID). The property ID therefore appears **twice** in each
  entry. The result is "stream not in error".
- **Read**:
  1. Read `version`. If it is not 2, return false. The stream is **not** put into the error state, and
     only the 2 version bytes have been consumed.
  2. Read `count`. **Remove all** existing properties.
  3. Repeat `count` times: read `key` (it is **ignored**), then read a property into a **new** property whose
     value starts Empty. If the property read succeeded, its value is not Empty, and it is not an `IGZUnknown`
     value holding a non-null object, add it under its **own** `propertyID`. A duplicate ID in the stream
     therefore replaces the earlier entry.
  4. Return "stream not in error". There is no upper limit on `count`. After a failed property read every later
     read also fails (§2.1), so the loop ends quickly in effect.
- *[quirk]* **Every** object-valued property is written (§9.2) but **dropped** on read *[compat]*:
  - with a non-null object, the value reads back as `IGZUnknown` holding the object, and the rule above drops it;
  - with a null object (marker plus null reference), the variant stays Empty (§9.2), and the "not Empty" rule
    drops it.

  A holder therefore **never** comes out of a load with an `IGZUnknown` property. The holder's test "not an
  `IGZUnknown` with a non-null object" can only matter for variants that are already `IGZUnknown`/null before the
  read, and no reader produces those. On save, both kinds are written (marker `0xA9BEE9AC` + reference, null =
  reference 0). No sample save contains either kind. (Corrects round 1, which said null-object entries survive.)
- *[choice]* FreeCity4 writers use `key = propertyID`. Standalone readers may reject `count` values that cannot fit
  in the remaining record.

### 10.3 Exemplar holder (`cISCExemplarPropertyHolder`) *[compat]*

An exemplar holder is a plain holder with an optional **parent exemplar** (the "default exemplar"). `HasProperty`
and `GetProperty` look at the holder's **own** properties first and then at the parent's. Only the own
properties are saved: inherited values are **not** copied into the save, only the parent's key is.

```
uint16  version = 2
...     plain holder record (§10.2)
uint32  group          parent exemplar key, in the order GROUP, TYPE, INSTANCE
uint32  type           (note: not the type, group, instance order of §2.5)
uint32  instance       all three zero = no parent
```

- **Read**: `version` must be 2 and the plain holder's `Read` must succeed. Otherwise return false (nothing after
  the failing part is read). Read the key. If all three fields are zero, the parent becomes none. Otherwise the
  holder asks the resource manager for the resource with that key, requesting `cISCResExemplar` (`0xA52160F5`).
  If it is found, it becomes the parent. If it is missing, the parent is **left unchanged** (none for a freshly
  constructed holder) and the read still succeeds. Return "stream not in error".
- **The containing reader decides whether to propagate that Boolean** (LG33). The exemplar reader really
  tests the plain holder's AL at `0x005FF454`–`0x005FF456`. In contrast, occupant base `Read` calls the
  exemplar reader at `0x005E8DD8`, discards its AL, and returns only `GetError() == 0`
  (`0x005E8DE1`–`0x005E8DED`). A bad holder version consumes its u16 without setting a stream error;
  an occupant owner can therefore continue at that cursor. Landfill tests the base result, then reads its
  own 21-byte tail there (landfill §6.4). This does not weaken the holder's own acceptance rule or establish
  a rule for other owners. Bad exemplar/plain versions retain the parent. After a successful plain read,
  the three parent-key destinations start at zero and key processing happens **before** the final error
  check (`0x005FF465`–`0x005FF4E9`): even a short key can clear the parent if the resulting key is all zero.
- Consequence *[compat]*: the parent is resolved against the **current** data files and plugins at load time.
  Changing an exemplar changes the inherited values of every saved object that refers to it. Removing it leaves
  the saved object with its own properties only.
- **Write side extra** *[inferred]*: if the parent has a non-zero property `0x6A871B82` (the same property the
  game's missing-plugin reporting checks, believed to be a plugin-pack ID), the writer also stores the parent
  exemplar itself as a resource in the save package, under the parent's own key. The city package is a resource
  source while the city is loaded, so the saved copy resolves even without the plugin. No sample save contains
  one. *[choice]* FreeCity4 writes the same in standalone mode once exemplar writing exists, and never in plug-in
  mode (the original writer does it there).

### 10.4 Which records use which holder

| Record | Holder | Spec |
|---|---|---|
| ordinance | exemplar holder | ordinance §9.3 |
| neighbour connection, neighbour deal (in the city's neighbours objects and in regional-city records) | plain holder | neighbors §4.3, §5.6; §11 |
| occupants (inside the occupant base record) | exemplar holder (`0x005E8D60` → `0x005FF420`) | lot-records.md §2.3 |
| lots | plain holder (`0x006C35F0` → `0x005FF280`) | lot-records.md |
| many other saved objects | one of the two | their own specs |

The budget's locally funded objects refer to holders through `ref` (§2.7), not inline.

### 10.5 Duplicates, order and the parent *[compat]*

- A writer emits at most one entry per ID, in ascending unsigned ID order. A reader tolerates duplicates (the last
  one wins) and any order (it re-sorts).
- Saving loses nothing that a lookup can observe, except dropped object-valued properties (§10.2) and parent
  values that change in the data files (§10.3).

### 10.6 Worked examples

(a) **Ordinance holder without own properties** (sample save, Commuter Shuttle). The parent is group
`0xA9C2C209`, type `0x6534284A` (exemplar), instance `0xA2BF1DE5`:

```
02 00                    exemplar-holder version 2
02 00 00 00 00 00        plain holder: version 2, count 0
09 C2 C2 A9 4A 28 34 65 E5 1D BF A2
```

(b) **Neighbour-connection holder** (sample save, 8 properties). The first and the last entries:

```
02 00 08 00 00 00
00 41 0B 8A  00 41 0B 8A  00 00 00 00  02 80 00 00  0C 00 00 00  (12 × uint16: 00 00 … 5C 00 00 00 00 00 53 02)
…            (IDs 0xAA9EA81A … 0xAA9EA81F: Uint32 arrays of 7 zeros, tag 03 80 00 00)
77 70 94 CA  77 70 94 CA  00 00 00 00  09 00 00 00  00 00 C0 40        (0xCA947077 = Float32 6.0)
```

(c) **Synthetic** plain holder {`0x00000010` = Uint8 41, `0x80000001` = Float32 1.5}. The order is by unsigned ID:

```
02 00 02 00 00 00
10 00 00 00 10 00 00 00 00 00 00 00 01 00 00 00 29
01 00 00 80 01 00 00 80 00 00 00 00 09 00 00 00 00 00 C0 3F
```

The same properties as an exemplar holder with the parent of (a) add `02 00` in front and the 12 key bytes of (a)
at the end (57 bytes in total).

## 11. Regional-city record

### 11.1 Where it lives *[compat]*

The region stores **nothing per city outside the city files**. A region folder holds `region.ini`, `config.bmp`
and one `.sc4` file per city tile. Each city file contains one **regional-city record**, type `0xCA027EDB`,
group `0xCA027EE1`, instance 0. It is opened with the auto-stream helper, so its payload starts with
`uint16 1` (§2.10). The region's regional-city objects (`cISC4RegionalCity`) are created with
`GZCOM()->GetClassObject(0x6A037C24, 0x69D2BE1D)` (the IID is the one the reader requests; the SDK names no IID).
If that fails, they are constructed directly. Their `Read` is then called on this record. The objects are
built from these records:

- when the region loads, from every city file in the folder;
- by `cISC4Region::ReloadCity`, which re-reads one city's file (the neighbours simulator does this for every
  adjacent established city in its `Init`, neighbors §14.1);
- by moves in the region view.

It is written:

1. while the current city is saved: into the new package, right after the root record (§3.7 step 3);
2. after a **successful** save of the current city: into the existing file of **every adjacent city**,
   established or not, that has a save path. Only this record is replaced in the neighbour's file (§3.7 step 7).
   This is how deals and connections reach the neighbours' files;
3. when a city is moved in the region view.

### 11.2 Layout (version 13) *[compat]*

All values are little-endian. `str` is §2.3, `f32`/`f64` are IEEE.

```
u16  1                            auto-stream helper version
u16  version = 13
s32  posX, posZ                   region cell of the city's corner (GetPosition)
s32  sizeX, sizeZ                 size in region cells (GetCitySize)
s32  population, commercialJobs, industrialJobs
f32  workforcePercentage          a fraction (0.48 = 48 %)
s8   mayorRating
u8   difficulty
u8   moved                        set when the city is moved in the region; cleared by its next TerrainReconciliation (§11.3)
u32  serialNumber
s32  originalLanguage, originalCountry, lastLanguage, lastCountry
u32  birthDate
u8   established
str  cityName, s1, mayorName, s2, utilityAdvisorName, description, s3      (s1..s3: unknown, empty in samples)
f32  worldLongitude, worldLatitude, worldBaseElevation
s32  tutorialID                   −1 = none
u32  n;  n × { u32 populationType; s32 value }     population by type (ascending type)
u32  n;  n × { u32 populationType; s32 value }     extrapolated population
u32  n;  n × { u32 populationType; s32 value }     potential population
u32  12; 12 × f32                 tax rates (percent)
u32  n;  n × connection record    (neighbors §4.3, including its plain holder §10.2)
u32  n;  n × { u32 a; s32 x; s32 z; u8 flag }       pending connection changes (meaning partly unknown)
f64  budget
f64  income
s32  n;  n × { s32 commodity; f64 produced; f64 demanded; f64 imported; f64 exported; f64 costPerUnit }
                                  ascending commodity: 1 power, 2 water, 3 garbage
s32  n;  n × deal record          "other" deal list O (neighbors §5.6)
s32  n;  n × deal record          "local" deal list L
u32  n;  n × { u32 poiType; u32 m; m × { f32, f32, f32 } }     points of interest: type 0 airports, 1 seaports
```

Example (sample save, the first 43 bytes): `01 00 0D 00 06 00 00 00 05 00 00 00 02 00 00 00 02 00 00 00 04 0A
00 00 29 01 00 00 8C 02 00 00 8F C2 F5 3E 33 02 00 FB CB E3 53` gives helper 1, version 13, position (6, 5),
size (2, 2), population 2564, 297 commercial jobs, 652 industrial jobs, workforce 0.48, mayor rating 51,
difficulty 2, moved 0 and serial `0x53E3CBFB`. All 85 city files of the test region parse completely under
this layout (versions 9, 10 and 13, with the version rules below). None contains a deal, but two shipped saves
(`City - Fulham.sc4`, `City - Kensington.sc4`) hold two deals each and parse with this layout (save-records.md §7.5).

**Read** *[compat]*:

- `version ≥ 14` → fail. Older versions: without `workforcePercentage` below 10 (0.5 is used); version 10 stores
  it as a **percentage** (multiplied by 0.01 on read); without `difficulty` below 13 (2 is used); without the
  points of interest below 12.
- `difficulty` is applied only when `established` is set.
- Limits: each population map ≤ `0x20000000` entries, tax rates ≤ 12, connections, changes and commodities each
  ≤ `0x100000`. Exceeding a limit makes the read return false. Deal counts are not limited.
- `tutorialID` is passed through the tutorial registry's ID mapping.
- If the city is not established, `cityName` is replaced by the localized default name. Then the city-name and
  mayor-name string records of the same package (§3.2, groups `0x6AAEE931` / `0x6AAEE932`, current language),
  when present, override `cityName` and `mayorName`.
- Connections are read into new **original** connection objects, constructed directly.
- Deals: each deal is read into a new **original** deal object. Deals that read successfully are appended to **O**.
  This holds for **both** saved lists, the first and then the second. Every deal read gets `SetFlags(Silent, true)`.
- The save-file path is not stored. The region sets it from the file name.

**Read into an existing object (RY7).** `Read` (`0x004AC5B0`, on the serializable view at object `+8`)
mutates its receiver as it consumes the stream. It does **not** clear the population maps, commodity map,
connections, pending changes, deals or POI map first. Population entries assign by key (later duplicate wins);
commodity entries find or insert the key and then overwrite its five doubles (later duplicate wins).
Consequently a freshly constructed object's zero entries for commodities **1, 2 and 3 remain** unless overwritten;
an absent key does not erase one. Re-reading an object also retains keys absent from the new stream.
POI entries find or insert a list and **append** the positions: repeated POI keys concatenate in wire order,
including any positions already in the object. A repeated key with zero positions does not clear its list.
Evidence: population assignment `0x004AC841`–`0x004AC846`, commodity lookup and writes
`0x004ACCBA`–`0x004ACD37`, POI lookup/append `0x004ACFC0`–`0x004AD128`.

The count guards above compare the **unsigned 32-bit representation**, including the commodity count read
through the signed-integer stream method (`0x004ACC3F`–`0x004ACC4B`). Thus commodity count `FFFFFFFF` fails
immediately as greater than `0x100000`; it does not mean an empty map. Both deal loops instead test only
**nonzero**, decrement a 32-bit count and repeat until zero (`0x004ACD4B`–`0x004ACD6A`,
`0x004ACE46`–`0x004ACE7C`, `0x004ACF56`–`0x004ACF5C`). A negative signed count is **not an empty loop**:
`FFFFFFFF` requests 4,294,967,295 attempts, subject to allocation/failure behaviour. Do not run such a record
to completion as a conformance test or infer successful acceptance of a truncated file from this loop.

Object-level failure is **not atomic**: an over-limit count or the final stream-error test can return false
after earlier fields and containers have changed. There is no rollback. Primitive read results are generally
not checked individually; the final stream error is checked at the end. This differs from publication by
`SC4ReadRegionalCity(package, &out)`, which uses a new object and leaves `out` untouched on failed `Read`
([regional-city-records.md](regional-city-records.md) §2.1). A safe standalone wire decoder may reject malformed
counts before allocation and publish only on success, but those protections do not describe a native object's
in-place `Read` contract.

Worked cases: (a) a fresh object reading only commodity keys 1 and 2 with produced values 10 and 20 finishes
with keys **1, 2, 3**, with key 3 still all zero; writing it emits three entries. (b) two key-1 commodity
entries with produced values 10 then 25 leave 25; two POI key-7 entries containing positions A then B leave
`[A, B]`, not `[B]`. (c) a pre-existing object's position `(8,9)` is overwritten by `(2,3)` before an
over-limit commodity count causes false; the position remains `(2,3)`. The wrapper leaves its caller's old
object reference untouched in the equivalent failed package read. (d) with a recording stream, a deal count
`FFFFFFFF` enters the first deal attempt rather than proceeding directly to the second list; stop the trace
there instead of allocating billions of objects.

### 11.3 The two deal lists *[compat]*

- **O ("other")**: deals loaded from disk that no simulator has claimed yet, plus the deals of the record's city
  with third cities.
- **L ("local")**: the deals between the record's city and the **current** city (for the current city's own
  record: all of its deals). `GetLocalDeals(out)` replaces `out` with a copy of L (references).

**Claiming** (`extract(src, dst, removeClaimed)`): does nothing unless a current city with a neighbours
simulator exists. Let `cur` be the current city's regional record and `this` the record being updated. For each
deal `d` of `src`, in list order:

```
A := region city at d's city-side region cell;  B := region city at d's other-side region cell
if A or B is missing:   remove d from src (always, whatever removeClaimed says)
else if (this = cur and (A = cur or B = cur)) or (this ≠ cur and {A, B} = {cur, this}):
    append d to dst (a reference, AddRef);  if removeClaimed: remove d from src
```

| Method | Behaviour |
|---|---|
| `UpdateLocalDeals()` | `extract(O, L, true)`. L is **not** cleared first. Then `SetFlags(Silent, false)` on every deal that is **still in O** (third-party deals become visible to `UpdateImportExport`). *[inferred]* Under neighbors §5.4, clearing Silent may post a status report for those deals. Returns true. |
| `SetLocalDeals(list)` | `L := empty`; `extract(list, L, false)`; `UpdateImportExport()`. Returns true. L holds **references to the caller's objects**. *[quirk]* The caller's list loses every deal with a missing region city. The neighbours simulator passes **its own deal list**, so such deals disappear from the simulator. |
| `GetLocalDeals(out)` | `out := L` (copy of the references). |
| `UpdateImportExport()` | For commodities 1–3: `imported := 0`, `exported := 0`. Then for every deal of O, then of L: skip it if its other-side city is missing or it has any flag of `0xBC` (Cancelled, NoConnection, CannotAfford, NoLandfill, Silent). The commodity is E0/E1 → 1, E2/E3 → 2, E4/E5 → 3; other types are skipped. For a **buy** type (E0, E2, E4): if the other side is `this`, `exported[c] += amount`, else `imported[c] += amount`. For a **sell** type: if the other side is `this`, `imported[c] += amount`, else `exported[c] += amount`. Produced, demanded and cost per unit are not touched. |

**Messages** handled by every regional-city object *[compat]*:

| Message | Action |
|---|---|
| PostLoad `0x26C63342`; PostCityShutdown `0x26D31EC3` for the current city | Move all of L into O: repeatedly take the **last** deal of L and append it to O. L's order is therefore **reversed** at the end of O. |
| PostCityInit `0x26D31EC1`, only on the current city's record | Refresh the cached statistics from the city (`UpdateCityCache`). |
| PreSave `0x26C63343`, only on the current city's record | `UpdateCityCache`, then recompute the potential and extrapolated populations. |
| TerrainReconciliation `0xCA63D517`, only on the current city's record | Rebuild symmetric connections and post **ConnectionsReady**. When the city is not established, data3 ≠ 0 or `moved` is set, existing connections are first marked for removal and incoming ones treated as new. Then `moved` is cleared (§11.6). |

`Write` writes O, then L (§11.2).

### 11.4 Life of a deal across the region

1. **Load** of city X: the records come from disk, with every deal in O and Silent. PostLoad moves any L back
   into O.
2. **Neighbours `Init`** (neighbors §14.1): `ReloadCity` each adjacent established city; `R.UpdateLocalDeals()`
   (X's own record claims every deal involving X); for each adjacent city C: `C.UpdateLocalDeals()` (claims the
   X–C deals), then the simulator clones C's L into its own list (as seen from X).
3. **Push** (neighbors §9.2): for the adjacent established cities, then X's record: `SetLocalDeals(simulator
   list)` and/or `UpdateImportExport()`. The original visits the adjacent cities in **reverse** of the
   `GetAdjacentCities` order. That order is not observable, because each city's result depends only on the list.
   `SetLocalDeals` runs `UpdateImportExport` itself, so with both flags set the totals are computed twice
   (harmless).
4. **Save** of X: X's record is written into X's file, with L = the simulator's live deal objects, written with
   their own `Write`. Each adjacent city's record is rewritten into its file: O = its deals with third cities,
   L = its deals with X (again the simulator's objects).
5. Later, loading neighbour C: C's simulator rebuilds its X–C deals from X's record (step 2 with roles
   swapped). The copy in C's own file is used for C's import/export totals.

### 11.5 Consequences for FreeCity4

- Plug-in mode *[compat]*: the original region objects hold our deal objects (through `SetLocalDeals`) and write
  them with our `Write`. Our deals must produce neighbors §5.6 exactly and must tolerate `SetFlags` / `Clone`
  calls from region code at any time. FreeCity4 does not replace the regional-city class in plug-in mode
  *[choice]*, although the GZCOM creation path (§11.1) would allow it. It uses the region objects only through
  `cISC4RegionalCity`.
- Standalone mode: FreeCity4 implements §11.2–§11.4 itself, including the PostLoad reversal and the
  `SetLocalDeals` pruning, so that saves and the deal life cycle match.

### 11.6 TerrainReconciliation and ConnectionsReady *[compat]*

**The one poster of ConnectionsReady `0x6AC284F3`** is `cSC4RegionalCity::CreateSymmetricConnections`
(`0x004AA6D0`). It posts at its very end (`0x004AB739`–`0x004AB77F`), on every call and whatever it found:
- the message is a new standard message (`0x2C` bytes) with only `SetType`, so data1–data4, the voids and the
  IGZUnknown are all 0;
- it is posted queued, at normal priority (`MessagePost(msg, false)`, message-server slot 4, `0x004AB778`), and then
  released.

A search of the image for the constant finds only this poster and the three subscribers (power, plumbing and
neighbours: `Init`, `Shutdown`, `DoMessage`). The other hit is the message-name table.

`CreateSymmetricConnections` has one caller: the regional city's TerrainReconciliation handler (`0x004ABDB0`). The
handler acts only on the record that `cISC4App::GetRegionalCity()` (slot 36) returns, and only while a city
(`0x00B43CEC`) exists:

1. `forced := (data3 ≠ 0)` and `established := city.GetEstablished()` (city slot 22). data1 and data2 are read
   and ignored.
2. If the city is not established, `forced` is true or `moved` is set:
   - the pending connection changes are dropped (`0x004A8570`);
   - when `moved` is set, the city's own connections are marked for removal (`0x004A9580`, regional-city.md §6.2);
   - incoming connections are treated as new (`0x004A9220`).
3. `CreateSymmetricConnections`, which posts ConnectionsReady.
4. `moved := false`.

**Who posts TerrainReconciliation `0xCA63D517`.** Only the city's terrain (`cSTETerrain`, created for each city by
`cSC4City::CreateCityComponents`, `0x0045AA10`), in three ways:

1. **Automatically, once per terrain object, so once per city load.** This is `DoAnimationTick` (`0x0074A390`). It
   runs for each animation tick `0x86AD10EE` that the terrain receives (`DoMessage` `0x0074A590`).
   - A counter (`+0xD8`) counts the ticks that arrive after the terrain's 3D view exists. It is 0 from the
     constructor (`0x0074C380`) and is never reset. The terrain's own PostCityInit handler (`0x00749540`) creates
     the view.
   - On the **second** such tick, the game preference at preferences `+0x4EC` decides. It is **1** by default
     (`cSC4Preferences` constructor `0x00449070`). The Game Preferences dialog (`0x0076B110`) and the
     edge-mismatch dialog's checkbox change it.
   - If the preference is **0**, TerrainReconciliation is posted at once (`0x0074A3E3`, all data 0).
   - Otherwise (the default), message `0x2A612643` is posted to the terrain itself (`GeneralMessagePostToTarget`,
     server slot 7). Its handler (`0x0074A505`) does one of three things:
     - with no window manager (service `0xA417445E`), it stops, and nothing is ever retried;
     - while a modal window is running, it posts `0x2A612643` again and waits. The test is window-manager slot 42,
       a modal depth `+0x17C ≠ 0` that `DoModalWin` raises;
     - otherwise it calls `ReconcileEdgesWithNeighboringCities(0, 1, 1)` (terrain slot 89).
2. **`ReconcileEdgesWithNeighboringCities(a, b, c)`** (`0x0074ED40`). It **returns at once, posting nothing, when
   popup dialogs are disabled** (`SC4GetPopupModalDialogsEnabled` `0x005F9090`, which is `cISC4App` slot 23).
   Otherwise:
   - it compares the edge vertices with the neighbouring cities (`ComputeEdgeVerticesToReconcile` `0x0074D010`);
   - on a mismatch, with a = 0 and b ≠ 0, it shows the modal edge-mismatch dialog (`0x00734E10`). If that dialog is
     accepted, and b ≠ 0, it shows the damage dialog (`0x00735120`). If that is accepted too, it applies the change
     (`LockEdgeVerticesAndSmoothen` `0x00748C90`);
   - with no mismatch and a ≠ 0, it shows the "no mismatch" dialog;
   - it **always** posts TerrainReconciliation at the end (`0x0074EF20`). data1 = a, data2 = 1 when a mismatch
     was found, data3 = 1 when the change was applied, and data4 = 0.

   The automatic call (0, 1, 1) with no mismatch shows no dialog and posts all zeros.
3. **By the player.** The reconcile-edges command of the landscape menu (`cSC4View3DWin::DoLandscapeMenu`,
   control `0x4A551A6B`) calls `ReconcileEdgesWithNeighboringCities(1, 1, 1)`. So ConnectionsReady is posted
   again each time the player uses it, behind the same popup gate.

So after a load, ConnectionsReady arrives **once**, a few frames after PostCityInit. With the default preference
(1), it never arrives, or arrives late, in these cases:
- **popup dialogs are disabled** (`cISC4App::SetPopupDialogsEnabled(false)`, advisors.md §6.8.1) when the deferred
  message is handled. Then no TerrainReconciliation is posted that session, and power, plumbing and the neighbours
  simulator stay disabled *[compat]*;
- a modal window stays open. The reconcile waits until it closes, and the mismatch and damage dialogs are
  themselves modal;
- the window-manager service is missing.

It also needs the current regional-city record and a live city. With the preference at 0, only the regional-city
condition remains.

*[choice]* In plug-in mode, FreeCity4 does not disable popup dialogs until it has seen ConnectionsReady (it subscribes
to `0x6AC284F3`). In standalone mode it posts ConnectionsReady once per load, after its own symmetric-connection
rebuild, which follows PostCityInit *[inferred: no simulation day runs between PostCityInit and the second
animation tick]*.

## 12. The C runtime `rand()`

### 12.1 Algorithm *[compat]*

The executable links the multithreaded C runtime statically. `rand()` keeps a **per-thread** 32-bit state `s`.
A thread's state is set to **1** when the runtime first creates that thread's data.

```
srand(x):  s := x
rand():    s := (s × 214013 + 2531011) mod 2^32;   return (s >> 16) & 0x7FFF          (0 … 32767)
```

From state 1: 41, 18467, 6334, 26500, 19169, 15724, 11478, 29358, 26962, 24464. After `srand(12345)`: 7584,
19164, 25795.

### 12.2 Seeding *[compat]*

- **The game never calls `srand`.** The only call site is Lua's `math.randomseed(n)` (`srand(trunc(n))`), and
  `math.random` draws from the same generator. No shipped Lua script uses `math.random` or `math.randomseed`. All
  Lua resources in the shipped `.dat` files were searched. Plugin scripts may use them.
- The main thread's sequence therefore starts at state 1 when the process starts. It is **shared** by every
  caller on that thread, including non-simulation ones: radio-station playlist shuffles and idle handling,
  audio ambience and automata audio events, helicopter AI and others. The values that a subsystem gets depend on
  every earlier `rand()` call in the process: menu time, music settings, the cities played before. In practice
  they are **not reproducible** from a save. This is consistent with §6.2 (no RNG state is saved).
- Whether any `rand()` caller runs on a thread other than the simulation's is unknown (§15 question 4).
  Such a caller would have its own state, starting at 1.

### 12.3 `random_shuffle` *[compat]*

The library's `std::random_shuffle(first, last)` without a generator argument does:

```
for i in 1 .. n−1:   j := rand() mod (i + 1);   swap(a[i], a[j])
```

It makes exactly `n − 1` calls (none for `n ≤ 1`). The modulo is of non-negative values. For `n > 32768` the
later positions are never fully mixed: the original's behaviour, kept. Example: `a = [0, 1, …, 9]` from state 1
→ `[8, 1, 9, 2, 0, 5, 7, 3, 4, 6]`. The same algorithm is used on vectors and deques.

### 12.4 Random cell iterators *[compat, summary]*

The random cell iterator of weather §6.7 has two modes: in prime-step mode a reset makes one `rand()` call
(`start := rand() mod count`); in table mode, at each reset with shuffling requested, the table of packed absolute coordinates is
`random_shuffle`d (§12.3). The table is refilled in natural order first at every table-mode reset (its refill
flag is never cleared; weather §6.7). A **linear**
variant of the same utility iterates indices `0 … n−1` instead of a rectangle. It uses the same prime table, caps
`n` at 130,444 in prime-step mode, and is empty for `n = 0`. Each subsystem spec that uses an iterator must say
which mode it uses and when it resets.

### 12.5 Users in the simulation (Windows build call sites)

| User | Use |
|---|---|
| power simulator (pole funding), plumbing simulator (pipe funding) | `random_shuffle` of a 32-bit list |
| prop developer (message handler) | `random_shuffle` of a 32-bit list |
| lot developer (message handler) | `random_shuffle` of a deque of `cISC4BuildingOccupant*` (the night-light list; n − 1 calls) |
| landfill zones (monthly landfill developer update) | `rand()` directly: one call per redrawn cell whose tile has all four surrounding heaps active, 0–4 per heap whose active state changes (landfill.md §4.8) |
| random cell iterator | flammability (daily), flora (init, monthly), pollution (init, daily, visual effects), power grid update, tract developer (lot listing, decline), weather (init, weekly); also obliterate-city, volcano lava, helicopters, audio ambience |
| linear iterator | construction occupants (table mode, n − 1 `rand()` calls per reset; they order lot areas, spark sides and prop lists: lot-developer.md §6.7), traffic simulator (accident check), one unnamed simulation function |

Names come from the symbol port and may be imprecise *[inferred]*.

### 12.6 FreeCity4 *[choice]*

- The game's runtime state cannot be reached through public interfaces, so FreeCity4 must not try to use it.
  The core provides a `CrtRand` generator implementing §12.1, plus `random_shuffle` per §12.3.
- Each replaced subsystem owns one `CrtRand` instance, seeded with 1 by default and overridable by the host. This
  matches a fresh process in which nothing else drew numbers. Subsystem specs must state the number of `rand()`
  calls for each operation, so that differential tests can align sequences once the observer forces the
  original's state.
- Under the fixed schedule (simulator.md §5.8) the state is set at the city load and before every agent delivery,
  in both games, and every replaced subsystem draws from the game's one state per thread, as the original's callers
  do: inside the original's process the host hands the cores its own state (`fc4_crt_set_host`).

## 13. Order of agents within a level (month level)

### 13.1 The original rule

The original simulator keeps the agents of each level in a **set ordered by the unsigned value of the
`cIGZMessageTarget2` pointer** passed to `AddAgent`. Adding the same pointer twice is a no-op. Delivery walks the
set in ascending order (simulator spec §5). The order is therefore fixed by heap addresses, not by the program.
FreeCity4's simulator runs agents in **registration order** (simulator spec §4).

### 13.2 Observation

Month-level (level 6) registrations, from a logged session with the FreeCity4 simulator in the test region
(registration order, and the rank of each agent's target pointer):

| Reg. | Agent | Addr. rank | Reg. | Agent | Addr. rank |
|---|---|---|---|---|---|
| 1 | terrain | 10 | 14 | lot developer | 18 |
| 2 | landfill developer | 20 | 15 | aura | 6 |
| 3 | airport/seaport developer | 21 | 16 | commercial | 25 |
| 4 | airport/seaport developer | 22 | 17 | crime | 19 |
| 5 | park manager | 7 | 18 | fire protection | 11 |
| 6 | **budget** | **4** | 19 | flora | 15 |
| 7 | **ordinance** | **24** | 20 | land value | 26 |
| 8 | power | 1 | 21 | police protection | 17 |
| 9 | plumbing | 5 | 22 | traffic | 8 |
| 10 | **neighbours** | **23** | 23 | pollution | 3 |
| 11 | demand | 16 | 24 | residential | 2 |
| 12 | building development | 14 | 25 | weather | 13 |
| 13 | tract developer | 12 | 26 | civic buildings | 9 |

In all three logged sessions the budget simulator's pointer was far below the neighbours' and the ordinance
simulator's (for example `0x0A9AAF70` against `0x17787798` and `0x17787818`). The budget is created before
both in a new and in a loaded city (city slot 20 against slots 30 and 31, §3.6).

### 13.3 Dependencies and the requirement

The budget's month agent books `GetOrdinanceMonthlyIncome/Expense` and the neighbour deal totals (budget §5.1,
§5.5). The ordinance simulator recomputes those totals in **its** month agent (ordinance §5). The neighbours
simulator activates, renews (re-prices), cancels and removes deals in **its** month agent (neighbors §7.3).

*[compat]* **Within the month level, the budget simulator's agent runs before the ordinance simulator's and the
neighbours simulator's agents.** Both the observed original order and FreeCity4's registration order give this,
because the city initialises the budget before the ordinance and neighbours simulators (§3.7) and each registers
its month agent in `Init`. Consequences, which tests must check:

- The settlement on the 1st of month M+1 books the ordinance totals computed on the 1st of month M (the
  previous run). An ordinance enacted during month M has its enactment amount folded into the totals on the 1st
  of M+1, and the budget books it on the 1st of **M+2**. The same one-month lag applies to repeal amounts and
  monthly ordinance costs.
- Neighbour deals are booked as they stood **before** this month's `simulate_deals`. That is the state left by
  the previous month-end run and by the re-runs that status reports triggered in between. A deal activated or
  renewed at this month-end is booked from the next settlement on. A deal removed at this month-end is still
  booked at this one if it had none of the excluded flags at that moment.
- A replacement budget, ordinance or neighbours simulator must register its month agent **in `Init`** (not lazily
  later), and a FreeCity4 city must keep the §3.7 `Init` order, so that registration order keeps the budget
  first.

No other ordering requirement is derived:

- **Ordinance vs neighbours**: their order differs between the original (neighbours first) and FreeCity4
  (ordinance first). Neither reads the other's month results.
- **Power, plumbing vs neighbours**: power and plumbing come first in both orders.
- *[compat]* **Landfill developer** (landfill.md §3.3): its month agent runs **after** pollution's and the
  budget's, and **before** the neighbours' and the ordinance simulator's. It feeds pollution's `landfilled` counter
  and line item 30, so the order decides whether those are seen this month or next. Evidence: in all eight logged
  level-6 registration sets (test sessions run3–run9 and the current plugin log, 24 agents each) the landfill
  developer's address rank is 19–20, pollution 3–14, budget 1–11, neighbours 21–22, ordinance 22–23. Registration
  order puts the landfill developer second, so FreeCity4 must order it explicitly (for example by registering its
  month agent after the city's other `Init` calls, or by a priority).
- What the eight logs confirm: the tail of the set is stable. Landfill developer (rank 19–20), port developers
  (20–21), neighbours (21–22) and ordinance (22–23) always come in that order, after every agent ranked 1–18;
  commercial (23–24) and land value (19–24) are at the end too. The ranks of the other 18 agents vary between
  sessions (for example pollution 3–14, budget 1–11, aura 2–18), so their mutual order stays open
  (§15 questions 5 and 7).
- Other pairs that differ between the two orders (for example residential, pollution and power before the budget
  in the original) were not analysed (§15 question 5). *[choice]* Until they are, FreeCity4 keeps registration
  order.

### 13.4 Decision: how FreeCity4 orders agents (2026-10-13)

`simulator.md` §4 originally asserted that because the original orders agents by object address, "no behaviour may
depend on it". **That assertion is false** and is corrected there: `landfill.md` §3.3 derives an observable
requirement from the order (whether this month's landfill intake is reported and charged this month or next), and
§13.3 above derives a second one for the budget. Order is therefore behaviourally significant, and FreeCity4 must
choose one deliberately.

**Decision.** The original's order is *not reproducible*: it is heap-address order, and the ranks of the first 18
month agents vary between sessions (pollution 3–14, budget 1–11, aura 2–18). FreeCity4 does not attempt to
reproduce it, because there is nothing stable to reproduce. Instead it satisfies the two properties that the
evidence does support:

1. **Determinism.** Within one FreeCity4 build the order is fixed and reproducible, so conformance vectors and
   seeded runs are meaningful. Registration order gives this; address order cannot.
2. **The derived data dependencies hold.** Every *[compat]* ordering requirement derived in this section and in the
   subsystem specs must be satisfied.

Concretely, month-level (level 6) agents are ordered by:

- **A fixed rank for the stable tail** — *[compat]* for the four-agent run that the evidence pins, *[choice]* for
  the two appended after it:
  - *[compat]* **landfill developer, port developers (×2), neighbours, ordinance**, in that order. The eight logged
    sessions agree on this run and agree that it comes after every agent ranked 1–18, and §13.3 derives an
    observable requirement from it (landfill after pollution and budget; landfill before neighbours and ordinance).
  - *[choice]* **commercial, then land value**, appended after that run. The logs put commercial at rank 23–24 and
    land value at 19–24, so land value's range overlaps the whole *[compat]* run above and its position is **not**
    pinned by evidence. No derived requirement involves either agent, so the order between them, and theirs against
    the four above, is FreeCity4's to choose. It is recorded here so it is chosen once and pinned by a vector, not
    left to registration accident.
- **Registration order for everything else**, i.e. the first 18 agents, whose relative order is genuinely unknown
  (§15 questions 5 and 7).

**Agent names of the tail** (AO1; the names the originals pass to `AddAgent`, so a rank can be keyed by name):

| Tail agent | `AddAgent` name | Levels registered (in order), flags | Evidence |
|---|---|---|---|
| landfill developer | `"cSC4LandfillDeveloper"` | 6; 2 | landfill §1.1 |
| airport developer **and** seaport developer | `"cSC4PortDeveloper"` (**the same name for both**) | 6; 2 | shared base `cSC4PortDeveloper::Init` `0x006EA950` (string `0x00A89650`, level push `0x006EA989`), called by both derived `Init`s |
| neighbours | `"cSC4NeighborsSimulator"` | 6, 4, 10; 2 | neighbors §14.1 |
| ordinance | `"cSC4OrdinanceSimulator"` | 6; 2 | ordinance §8.1 |
| commercial | `"cSC4CommercialSimulator"` | 0, 10, 4, 6; 2 | `Init` `0x0069EF90` (string `0x00A89920`, calls `0x0069EFDC`–`0x0069F00F`) |
| land value | `"cSC4LandValueSimulator"` | 0, 10, 4, 6; 2 | landvalue §4.1 |

Because both port developers share one name, a name-keyed rank gives them the same rank; their order between
themselves is free (*[choice]*: registration order). In the original the two are ranked by heap address like every
other agent, and the logs do not separate them, so there is no fixed original order to follow (AO3). Neither reads
the other's results: each walks its own port list, records history under its own key, makes no random draw
(port-developers.md §5), and its demand updates are additive. The only visible effect of their order is which of
two upgrade advisor messages sent in the same month comes first. `GetAgentEnabled` / `SetAgentEnabled("cSC4PortDeveloper")`
(simulator §4) likewise act on both.

**With the original simulator** (AO2) the order stays ascending target-pointer order (§13.1) and nothing in the
original can re-rank it. A glue object registered there gets the rank of whatever address it has. The observed
placement of the original landfill developer (after pollution and budget, before neighbours and ordinance, in all
logged sessions) comes from where the game's own heap put it; a glue target allocated elsewhere (the plug-in's CRT
heap) can land anywhere. No original behaviour fixes this, so logging the order at `AddAgent` with a warning when
the derived requirement fails is the right minimum. *[choice]* A glue that wants the original's placement passes as
its month target a small forwarding object allocated from the game's own heap at the point in city `Init` where the
original object is created (§3.7), which reproduces the original's distribution but still not a guarantee.

That placement satisfies all three derived requirements at once: pollution and budget are both in the first 18, so
they precede the landfill developer; the landfill developer precedes neighbours and ordinance because all three are
in the fixed tail in that order; and budget precedes ordinance and neighbours for the same reason (§13.3).

**Implementation note for the core:** this needs an explicit ordering mechanism, not a change to registration order
alone — registering the landfill agent late would also work but is fragile, because it depends on when the city
happens to call `Init`. A rank or priority attached to the agent (defaulting to registration order) is the robust
form. Whatever is chosen must be pinned by conformance vectors: a test that the landfill month agent runs after
pollution's and the budget's and before the neighbours' and the ordinance's, and a test that the other agents keep
registration order.

**Sequencing — do not retrofit this ahead of need.** The simulator core is already written, merged and verified
in-game (budget and power run correctly with it, runs 27/28). None of the six tail agents exists as a core yet
except ordinance and land value; landfill is being written now, and neighbours, the port developers and commercial
do not exist. So the ordering mechanism is **not needed until a tail agent's glue actually lands**. Introducing it
earlier means reworking verified, in-game-tested code for no behavioural gain, and risks the budget and power paths
that currently work. When it is needed, the change must be made with the existing month-agent behaviour pinned by
vectors first, so a regression in the verified budget settlement is caught rather than discovered in-game.

**What this does not settle:** the relative order of the first 18 agents, which is unobservable in the original and
therefore unconstrained except where a later analysis derives a requirement (as landfill §3.3 did). If a future
differential shows a value depending on, say, residential-before-budget, add the requirement here and to the rank
table rather than reordering registrations ad hoc.

### 13.5 The same reasoning applies at every level, not just month

The decision above is about the month level (level 6) because that is where a requirement was derived. It rests on a
property that holds at **all** levels, so it answers the same question wherever a subsystem spec asks it — for
example `residential.md` §10.2 question 3, which asks about the day-level (level 4) order of residential relative to
the growth developers, pollution and the BDS:

- The original orders agents **by the unsigned value of the `cIGZMessageTarget2` pointer** passed to `AddAgent`
  (§13.1), and delivery walks that set in ascending order. There is no per-level ordering logic; the level only
  selects which set is walked.
- Therefore the order within *any* level is an artifact of heap addresses. §13.2's measurements show it is not even
  stable between sessions at the month level (pollution rank 3–14, budget 1–11, aura 2–18), so there is **no original
  day-level order to recover either**. Asking "what order does the original run residential in at day level?" has no
  stable answer to give.
- Consequently, for every level: FreeCity4 uses **registration order** (*[choice]*, deterministic and reproducible),
  and the *only* thing that can make order matter is a **derived data dependency** — one agent reading state another
  writes in the same pass. Where such a dependency exists it must be derived from the code and recorded as a
  *[compat]* requirement with evidence, exactly as landfill §3.3 did, and then honoured by the rank table.
- Where a spec has no derived dependency and simply says the order "follows pointer order in the original and was not
  analysed", the correct disposition is not to analyse it further — it is to state that registration order is the
  *[choice]* and move on. Analysing it cannot produce a reproducible answer, and any value that appears to depend on
  it is depending on heap layout.

So `residential.md` §4.3's existing *[choice]* — "FreeCity4 uses registration order" — is confirmed correct, and the
part of its open question 3 that asked what the original's day-level order is, is closed: there is nothing to match.
That does **not** mean residential has no coupling to other agents — §5 state 1 reads the BDS building map and the
pollution source grids, and §4.3 notes it reads the growth developers' population maps. It means such a coupling can
only be resolved as a *derived* requirement with evidence, never by reproducing pointer order. If a future
differential shows residential's day step reading one of those maps before the agent that writes it has run this day,
that is a derived *[compat]* requirement and belongs in §13.4's rank table.

**One caveat on measuring it.** Delivery logs cannot recover the original's order either: every run that logs
deliveries (`deliver #N: level … -> agent …`) has FreeCity4's own simulator enabled, so it records *our* registration
order, not the original's. Run 19 is the only run with the original simulator and our observer, and it predates
delivery logging (zero `deliver` lines). The delivery log is also capped (300 lines), which in runs 23–28 captured
only levels 0 and 10. Recovering the original's order would need the observer running against the original simulator
with the cap raised — and even then it would only measure one session's heap layout, which is the thing that is not
worth reproducing.

Day-level agents run before month-level ones on the 1st of a month (simulator spec §5); this is independent of
the order within a level.

## 14. Conformance tests for §9–§13 (must exist in `engine/conformance`)

16. **Variant encoding**: every row of §9.3 encodes to and decodes from the given bytes. Every readable tag of
    §9.2 round-trips, including arrays of length 0. A Bool byte `02` reads as true.
17. **Variant failures**: writing a `VoidPtr` emits `00 00 00 00 12 00 00 00` and then fails (sticky error).
    Reading tag `0x11` or `0x10` (scalar) fails. Marker `0x00000001` is read like 0. A null object reference
    leaves a pre-set Uint32 variant unchanged. A string variant with header `0x10000002` and bytes `41 00 E9 00`
    reads as "Aé" (§2.3).
18. **Plain holder**: example (c) of §10.6 round-trips. Adding properties in the order `0x80000001`, `0x10` writes
    them in the order `0x10`, `0x80000001`. A stream with duplicate IDs keeps the last value. An entry whose key
    differs from its property ID is filed under the property ID. Empty entries, object-valued entries and
    null-object entries (`AC E9 BE A9 00 00 00 00`) are all dropped. Version 3 returns false with only 2 bytes consumed and the stream still
    good.
19. **Exemplar holder**: example (a) of §10.6 round-trips. A stub resource manager resolves the parent, and a
    lookup of a property only the parent has succeeds. An unknown parent key still loads the own properties and
    leaves the parent unset. A key of zeros gives no parent. The key is read as group, type, instance.
20. **Regional-city record**: check the four Read cases in §11.2, including retention of constructor commodity
    key 3 and concatenation of duplicate POI keys. A two-commodity wire map does not round-trip byte for byte
    through a fresh native object. For a canonical round trip include all three default commodity keys and
    put both deals in O with L empty (both input lists load into O). The header example of §11.2 decodes to the
    stated values. Version 10 reads the workforce as a percentage, version 9 has no workforce, version 12 has no
    difficulty, and version 14 fails. Both saved deal lists load into O, and every loaded deal is Silent.
21. **Deal claiming**: with region cells for cities X (current), C and D, and deals X–C, C–D, X–D and one with a
    missing cell: `C.UpdateLocalDeals()` moves X–C to L, keeps C–D in O with Silent cleared, and drops the
    broken deal. `X.SetLocalDeals(list)` keeps X–C and X–D and removes the broken deal **from the caller's
    list**. PostLoad moves L = [d1, d2] to the end of O as [d2, d1].
22. **Import/export**: a buy-power deal of amount 100 from X (city side) with C (other side): on C's record
    `exported[1] = 100`, on X's record `imported[1] = 100`. The same deal flagged Silent or Cancelled counts
    nothing. A deal of an unknown type counts nothing.
23. **`rand()`**: the sequences of §12.1. The shuffle example of §12.3 makes exactly 9 calls. A prime-step
    iterator reset makes exactly one call.
24. **Month order**: with stub budget and ordinance simulators registered in §3.7 order, an ordinance enacted on
    15 January with enactment cost 100 is charged at the 1 March settlement, not at the 1 February one. With
    registration order reversed the test detects the difference.

## 16. Interface slot order *[compat]* (overloads)

Original code calls our objects through vtable **slots**, so each interface's compiled slot order must match the
game exactly. A mismatched argument count under `__thiscall` corrupts the caller's stack.

How overloads end up in the vtable: the game was built with an MSVC-compatible compiler, which **groups overloaded
virtual methods and reverses each group**. The public SDK headers list overloads in source order. Compiled with
MSVC, they therefore produce the game's layout: it was verified in-game for every overload pair in
`cISC4Simulator` and `cISC4BudgetSimulator`. Two cautions:
- The verified tables in the interface specs describe the **compiled** layout (slot N = method). Compare against
  them, not against header text order.
- **Non-adjacent overloads** are the real risk. The compiler pulls a later overload up next to the first one, which
  shifts every method in between. An implementation should declare its interfaces with **unique method names**
  matching the spec slot tables, so the layout no longer depends on compiler grouping rules. It should also test each
  slot through the vtable with the game's argument counts.
- Verified compiled tables of framework and shared-service interfaces are in §17. They also list the places where
  the SDK **signature** itself is wrong (argument count or return type), independent of order.

## 15. Open questions for §9–§13

1. The meaning of the regional-city record's unknown strings `s1`–`s3`, of the connection-change entries, and the
   exact meaning of the twelve tax-rate slots.
2. Who keeps the regional record's `budget` and `income` current. The neighbours simulator's affordability checks
   read them, and `UpdateCityCache` does not set them.
3. Whether the original ever stores object-valued (`IGZUnknown`) or `Char` / `RZUnicodeChar` scalar properties in
   a saved holder. If it does, those values are lost or break the load (§9.2, §10.2).
4. Which thread runs the radio-station and audio callers of `rand()`. This decides whether they share the
   simulation thread's state (§12.2).
5. Month-level pairs other than budget/ordinance/neighbours whose order differs between the original's address
   order and registration order (residential, pollution and power before the budget; commercial and land value
   last). Each subsystem spec should check whether its month agent reads another one's month results.
6. Whether clearing Silent in `UpdateLocalDeals` really posts status reports (§11.3). That depends on the order of
   the check inside `SetFlags`, which a runtime capture would settle.
7. *(partly answered)* The landfill developer's position relative to pollution, budget, neighbours and ordinance
   is confirmed in all eight logged sessions (§13.3, now *[compat]*). Still open: the positions of the other
   month agents relative to each other where they vary between sessions, and whether any of those pairs exchange
   month results.

## 17. Compiled slot tables of game-framework interfaces *[compat]*

Method (applies to every table here and in the subsystem specs). Each interface's SDK header was laid out by the
MSVC rule of §16: every overload group sits at the position of its first declaration, and its members are in
reverse declaration order (rule re-checked with MSVC 2022, including a group whose members are not adjacent).
Each slot was then compared with the original Windows vtable named below. The callee's `ret N` operand gives
its stack-argument bytes (`int64`/`double` = 8, everything else 4; a by-value class return adds a hidden 4-byte
pointer). When an overload pair has the same byte count, its members were told apart by what the body does with
the argument. "Non-adjacent" marks overloads whose SDK declarations are separated by other methods: under MSVC
the later one moves up and shifts every method in between. An implementation that gives overloads unique names
(as §16 recommends) must place each one at the slot listed here.

**SDK signature errors** found by this check are listed with each interface. Calling them through the SDK
declaration corrupts the stack or returns garbage. The tables list only the overloaded and the wrong methods.
Every other slot matches the SDK in both order and argument size.

### 17.1 `cIGZVariant` (original vtable `0x00AA6788`, 127 interface slots)

The table here is the embedded interface. Native standalone clones install a distinct table;
the original constructor/interface evidence and bounded lifetime rules are in
[variant-storage §2](variant-storage.md#2-native-identities-and-attribution).
On normal native return, Erase/CopyFrom/CreateCopy explicitly set **AL=true**; the upper
EAX bytes are not contractual. CreateCopy constructs a distinct standalone variant,
initializes its own reference count to zero, then AddRefs it before publishing **one owned
reference**. This establishes neither a clean allocation-failure return nor whole-interface
behavioral equivalence; see [variant-storage §5](variant-storage.md#5-createcopy-and-return-channels).

| Slots | Group (compiled order) |
|---|---|
| 9/10, 12/13, 15/16, 18/19, 21/22, 24/25, 27/28, 30/31, 33/34, 36/37, 39/40, 42/43, 45/46, 48/49, 51/52 | `GetValX()` (no argument, returns the value) **then** `bool GetValX(X&)`, for X = Bool, Uint8, Sint8, Uint16, Sint16, Uint32, Sint32, Uint64, Sint64, Float32, Float64, Char, RZUnicodeChar, RZChar, VoidPtr. Example: `+0x84` = `int64 GetValSint64()`, `+0x88` = `bool GetValSint64(int64&)`, `+0x8C` = `SetValSint64` |
| 59/60 | `cIGZUnknown* GetValUnknown()`, then `bool GetValUnknown(uint32 riid, void** ppv)` |
| 62/63 … 90/91 | `AsX() const` (value), then `X& AsX()` (reference), for the 15 types in the order above |
| 92/93 … 122/123 | `RefX(X* p, uint32 count)`, then `X* RefX()`, for the 15 types and `RefVoid` |
| 124/125/126 | `RefIGZUnknown(cIGZUnknown*)`, `bool RefIGZUnknown(uint32 riid, void** ppv)`, `cIGZUnknown* RefIGZUnknown()` |

SDK errors: slot 8 is `bool CanGetVal(uint16 type) const` (one argument, always true in the original), not
`CanGetVal()`. Slot 60 takes `(riid, void**)`, not `(cIGZUnknown**)`.

#### 17.1.1 Native instance layout (HG9) *[compat]*

These are the original Windows object's bytes, separate from the serialized variant (§9.2).
Let **V** be its `cIGZVariant` interface pointer. A standalone `cRZVariant` begins at V,
occupies **20 bytes (`0x14`)**, and uses **four-byte alignment/packing**. There is no
padding between the fields or at the end. In particular, the eight-byte payload begins
at `V+4`, with only four-byte alignment required; it is not an eight-byte-aligned member.
This describes the native layout, not the spelling of a compiler packing directive.
The SDK `cRZBaseVariant` declaration with a 32-bit reference count does not describe it.

| Field | Offset from V | Width | Offset from a containing `cSCProperty` owner B |
|---|---:|---:|---:|
| Variant vtable pointer | `0x00` | 4 bytes | `0x04` |
| Payload, first/low word | `0x04` | 4 bytes | `0x08` |
| Payload, second/high word | `0x08` | 4 bytes | `0x0C` |
| Unsigned reference count | `0x0C` | **2 bytes** | `0x10` |
| Memory type (§9.1), including array bit | `0x0E` | **2 bytes** | `0x12` |
| Element count | `0x10` | 4 bytes | `0x14` |

All multibyte fields are little-endian. The two payload words form one eight-byte
storage area: a Uint64 uses its low 32 bits at `V+4` and high 32 bits at `V+8`.
Smaller scalars occupy their type's width at `V+4`; the remaining payload bytes are
not a second value. An array stores its 32-bit data pointer at `V+4` and its element
count at `V+0x10`; it does not store elements inline. Scalar VoidPtr/IGZUnknown also
use the first payload word. Construction clears all 16 bytes following the vtable,
including reference count zero; an owned CreateCopy result has reference count one.
Scalar count zero and disposal rules remain those of [variant-storage §3](variant-storage.md#3-representation-construction-and-erase).

The constructor's stores establish every offset and width (`0x009166D8`–`0x009166EC`);
the copy constructor repeats them (`0x0091671B`–`0x0091672E`). CreateCopy requests
exactly `0x14` bytes (`0x00916769`–`0x0091676D`), and a native stack temporary reserves
`0x14` without extra payload alignment (`0x00473CE4`, `0x00473D17`–`0x00473D24`).
The Uint64 setter writes the low/high words in that order (`0x009158EA`–`0x009158F5`);
the Float64 accessor reads eight bytes at `V+4` (`0x005FD640`). AddRef increments only
the word at `V+0x0C` and zero-extends it for return (`0x00914E78`–`0x00914E7C`).
Destroy clears payload/type/count but preserves that reference word and the vtable
(`0x00914ED4`–`0x00914EDE`). These instruction widths, rather than inferred SDK members,
establish the packing.

**Embedded property view.** The property constructor constructs the same 20-byte
variant at **V = B+4**, then installs the embedded table `0x00AA6788` there
(`0x005FDE54`–`0x005FDE68`). The standalone table is `0x00ACECD0`. The property's
own interface vtable is at B and its property ID is at `B+0x18`, immediately after
the embedded variant (`0x005FDE6E`, `0x005FDE72`). There is no additional payload
alignment gap or separate variant allocation. Slots 0/1/2 of the embedded table
subtract four from V before reaching the owner's identity/reference paths
(`0x005FD6F0`, `0x005FD700`, `0x005FD710`); storage methods use V directly.
The owner's AddRef and non-final Release operate on the **same two bytes** at
`B+0x10 = V+0x0C` (`0x005FCFF0`–`0x005FCFF4`, `0x005FD002`–`0x005FD00D`).
Thus the embedded reference word serves the property owner; it is not an independent
variant reference counter alongside another owner counter. Final release follows
the owner's destruction path, rather than freeing V as a standalone allocation.

Mutable numeric/character AsX aliases point exactly to `V+4` (`0x0060FB50`), so an
embedded alias points to `B+8`. A no-argument RefX instead returns the pointer stored
at `V+4` (`0x0060CE80`), which is the separate element allocation for an array.
AsVoidPtr is the already documented exception: it loads that pointer value too.
The matching scalar type determines an AsX alias's writable width; see
[variant-storage §4](variant-storage.md#4-copying-array-setters-and-mutable-aliases).

**Worked layout cases (static expectations; unexecuted).** Use valid allocations;
the addresses below are illustrative, not fixed allocation requirements.

1. Construct a standalone variant at V and acquire one reference. Set its Uint64
   value to `0x1122334455667788`. Bytes `V+0x04` through `V+0x13`, grouped as
   payload / reference count / type / count, are
   `88 77 66 55 44 33 22 11 / 01 00 / 08 00 / 00 00 00 00`.
   Mutable AsUint64 returns V+4. Writing `0x0102030405060708` through it changes
   just the payload to `08 07 06 05 04 03 02 01`; reference/type/count stay unchanged.
   AddRef then returns 2 with only `V+0x0C..0x0D` changed to `02 00`; non-final
   Release restores 1. Erase zeros payload/type/count and keeps reference count 1.
   If V is `0x01001000`, the eight-byte alias is `0x01001004`, demonstrating why
   the native representation cannot require an eight-byte-aligned payload.
2. From that erased, still-owned object, use the copying Uint32 array setter with
   `[17,34,51]`. For returned allocation D, `V+4` contains D, `V+8` remains zero,
   `V+0x0C` is 1, `V+0x0E` is `0x8006`, and `V+0x10` is 3. D contains the three
   elements (12 bytes); RefUint32 returns D. The mutable scalar AsUint32 address
   would be V+4, the pointer storage, and is not an element alias in this state.
   Erase frees D and clears payload/type/count without changing the reference word.
   The array stores and their widths are at `0x00915064`, `0x00915076`, `0x00915079`.
3. Construct a property with ID `0x12345678` at B=`0x01002000`. Its variant view is
   V=`0x01002004`, its inline alias is `0x01002008`, its shared reference word is
   `0x01002010`, type is at `0x01002012`, count at `0x01002014`, and ID at
   `0x01002018`. Acquire one property reference and set the embedded value as in
   case 1: the same 16 bytes occur from `B+8`, while the ID remains `78 56 34 12`.
   AddRef through the variant view returns 2 and updates only the shared reference
   word; property non-final Release returns it to 1. Erase through the variant view
   preserves both vtables, the shared reference word and the property ID.

### 17.2 `cISCPropertyHolder` (original vtable `0x00AA6CD8`, 22 interface slots)

| Slot | Method |
|---|---|
| 5 | `bool GetProperty(uint32 id, void* buffer, uint32& length)` |
| 6 | `bool GetProperty(uint32 id, uint32 riid, void** ppv)` |
| 7 | `bool GetProperty(uint32 id, cIGZString& value)` |
| 8 | `bool GetProperty(uint32 id, uint32& value)` |
| 9 | `cISCProperty* GetProperty(uint32 id)` |
| 10 | `bool AddProperty(uint32 id, void* data, uint32 length, bool)` |
| 11 | `bool AddProperty(uint32 id, cIGZUnknown* value, bool)`: **SDK error**, the SDK says `int32 lValue`. The original stores an object value |
| 12 | `bool AddProperty(uint32 id, cIGZString const& value, bool)`: **SDK error**, the SDK omits the `bool` (the callee pops 12 bytes) |
| 13 | `bool AddProperty(uint32 id, uint32 value, bool)` |
| 14 | `bool AddProperty(uint32 id, cIGZVariant const* value, bool)` |
| 15 | `bool AddProperty(cISCProperty* property, bool)` |
| 17 | `bool RemoveProperty(uint32 id, bool)`: **SDK error**, the `bool` is missing |
| 18 | `bool RemoveAllProperties(bool)`: **SDK error**, the `bool` is missing |
| 19 | `bool EnumProperties(bool (*filter)(cISCProperty*, void*), void (*fn)(cISCProperty*, void*), void* data) const`: **SDK error**, the SDK has 2 parameters |
| 20 | `bool EnumProperties(void (*fn)(cISCProperty*, void*), void* data) const` |

The `bool` arguments' meaning is not established. The callers seen pass 0 (false). The exemplar's property-holder
vtable (`0x00AAF038`) has the same layout.

### 17.3 `cIGZString` (cRZString vtable `0x00A80810`, 28 slots)

| Slot | Method |
|---|---|
| 3 / 4 | `FromChar(char const*, uint32 len)` / `FromChar(char const*)` |
| 8 / 9 / 10 | `IsEqual(char const*, uint32 len, bool cs)` / `IsEqual(cIGZString const&, bool cs)` / `IsEqual(cIGZString const*, bool cs)` |
| 11 / 12 | `CompareTo(char const*, uint32, bool)` / `CompareTo(cIGZString const&, bool)` |
| 16 / 17 | `Append(cIGZString const&)` / `Append(char const*, uint32)` |
| 18 / 19 | `Insert(uint32, cIGZString const&)` / `Insert(uint32, char const*, uint32)` |
| 20 / 21 | `Replace(uint32, cIGZString const&)` / `Replace(uint32, char const*, uint32)` |
| 23 / 24 | `Find(cIGZString const&, uint32, bool)` / `Find(char const*, uint32, bool)` (same size: told apart by the body, slot 23 calls `ToChar` on its argument) |
| 25 / 26 | `RFind(cIGZString const&, uint32, bool)` / `RFind(char const*, uint32, bool)` (same check) |
| 27 | `Sprintf(char const*, ...)`: variadic, so **cdecl**, caller cleans |

No SDK errors.

### 17.4 `cIGZDate` (cGZDate vtable `0x00ACEA38`, 46 slots; the date the simulator returns from `GetSimDate()`)

| Slot | Method |
|---|---|
| 3 | `bool Set(char const* monthName, uint32 day, uint32 year)` (compares month names) |
| 4 | `bool Set(uint32 dayNumber)` |
| 5 | `bool Set(cRZTime const&)` |
| 6 | `bool Set(uint32 month, uint32 day, uint32 year)` |
| 7 | `bool Set(uint32 day, uint32 year)` |
| 16 / 17 | `FirstDayOfMonth(uint32 month)` / `FirstDayOfMonth()` |
| 33 | `operator-(int32 days)` (**non-adjacent**) |
| 34 | `operator-(cIGZDate const&)` (**non-adjacent**) |
| 35 | `operator+(int32 days)` (**non-adjacent**) |
| 36 | `operator+(cIGZDate const&)` (**non-adjacent**) |

In the SDK the two `operator-` and the two `operator+` are interleaved (`-`, `+`, `-`, `+`). MSVC compiles them as
`-(int)`, `-(date)`, `+(int)`, `+(date)` at slots 33–36. Slots 37–40 (`++`, `--`, `+=`, `-=`) follow unchanged.

**SDK error (return type)**: slots 33–36 do **not** return `cIGZDate&`, and they do not modify the date. They
return a **uint32 day number**: `DayNumber() − n`, `DayNumber() − other.DayNumber()`, `DayNumber() + n`,
`DayNumber() + other.DayNumber()`. A FreeCity4 `cIGZDate` must return these integers. Callers must not treat the
result as a pointer.

### 17.5 `cISCResExemplar` (vtable `0x00AAF0D0`, 16 slots)

Corrected from 14: slots 14 (`0x0067E5B0`) and 15 (`0x0067DDC0`, rebuilds the effective property list; called by
`SetCohort` `0x0067C510` through `vt+0x3C`) exist (data-formats.md §8).

`AsISCPropertyHolder()` and `AsISCPropertyHolder() const` (slots 6/7) share one body. Their order is
unobservable. No errors.

### 17.6 `cIGZPersistResourceManager` (vtable `0x00AD89D0`, 44 slots)

| Slot | Method |
|---|---|
| 5 / 6 | `GetNewResource(uint32 type, uint32 riid, void**, uint32, cIGZUnknown*)` / `GetNewResource(cGZPersistResourceKey const&, uint32 riid, void**, uint32, cIGZUnknown*)` *(same size; by rule)* |
| 7 / 8 | `RegisterResource(cIGZPersistResource*)` / `RegisterResource(cGZPersistResourceKey const&, cIGZPersistResource&)` |
| 11 / 12 | `Save(cIGZPersistResourceKeyList*, cIGZPersistDBSegment*)` / `Save(cGZPersistResourceKey const&, cIGZPersistDBSegment*)` (checked: slot 12 calls `GetResource` with its key) |
| 21 | `FindObjectFactory(uint32 type, cIGZPersistResourceFactory**)` |
| 22 | `FindObjectFactory(cGZPersistResourceKey const& key, cIGZPersistResourceFactory**)`: **SDK error**, the SDK has a single `factory**` parameter. The original forwards `key.type` to slot 21 |
| 23 | `FindObjectFactory(cIGZPersistResource*, cIGZPersistResourceFactory**)` |
| 31 / 32 | `FindDBSegment(uint32, cIGZPersistDBSegment**)` / `FindDBSegment(cGZPersistResourceKey const&, cIGZPersistDBSegment**)` *(same size; by rule)* |
| 35 / 36 | `bool EnumerateDBSegments(callback*, cIGZPersistDBSegment*)` / `uint32 EnumerateDBSegments(cIGZPersistDBSegment**, uint32*)` *(same size; by rule)* |

`GetResource` (slot 3), which the simulation uses for exemplars and LTEXT, is not overloaded.

### 17.7 `cISC4GameDataRegistry` (vtable `0x00A8CBF8`, 29 slots)

| Slot | Method |
|---|---|
| 3 | `AddChildEntry(void* key, char const* name, callCallback (input/output parameter sets), cEntry* parent, flags, void* context)` |
| 4 | `AddChildEntry(void* key, char const* name, exchangeCallback (variant, set), cEntry* parent, flags, void* context)` |
| 5 | `AddChildEntry(cIGZUnknown* key, char const* name, cEntry* parent, flags, void* context)` |
| 6 / 7 | `RemoveChildEntry(void*)` / `RemoveChildEntry(cIGZUnknown*)` |
| 8 / 9 | `GetEntryID(void*)` / `GetEntryID(cIGZUnknown*)` |
| 24 / 25 | `AddNotificationTarget(cIGZMessageTarget2*)` / `AddNotificationTarget(cISC4GameDataRegistryNotificationTarget*)` |
| 26 / 27 | `RemoveNotificationTarget(cIGZMessageTarget2*)` / `RemoveNotificationTarget(cISC4GameDataRegistryNotificationTarget*)` |

Slots 3–9 were confirmed by the budget's own calls (budget §4.4). Slots 24–27 were placed by the rule only (same
size).

### 17.8 Shared simulation services

**`cISC4LotManager`** (zone developer's lot-manager vtable `0x00AB37C0`, 28 slots)

| Slot | Method |
|---|---|
| 5 / 6 | `GetLot(cS3DVector3 const&)` / `GetLot(int32 x, int32 z, bool)` |
| 7 | `GetLotsInCellRect(list, x1, z1, x2, z2, bool)`: not overloaded (meaning in budget Errata round 2, item 5) |
| 8 / 9 | `GetLotsSurroundingLot(int32 x, int32 z, list, int32 flags)` / `GetLotsSurroundingLot(cISC4Lot const*, list, int32 flags)` |
| 15 | `GetSafeLotIterator()` (the master lot list, budget §5.2) |
| 22 | `CreateLotIfPossible(int32 x, int32 z, int32 w, int32 h, int32, cISC4Lot*& lot, bool, bool, cISC4LotConfiguration*)`: **SDK error**, the `cISC4Lot*&` out-parameter is missing (36 bytes, not 32) |

**`cISC4Lot`** (vtable `0x00AB19B0`, 70 slots)

| Slot | Method |
|---|---|
| 50 / 51 | `GetCapacity(DeveloperType, bool forceCalculation)` / `GetCapacity(WealthType)` |
| 61 | `GetOccupancy() const`: **SDK error**, the method returns an `SC4Percentage` **by value**. MSVC passes a hidden result pointer, so the callee pops 4 bytes and writes the float through that pointer. Declare it as returning a one-float class type (or call it with a pointer argument). The SDK's `float` return is wrong |

**`cISC4BuildingDevelopmentSimulator`** (vtable `0x00AB0AF8`, 59 slots)

| Slot | Method |
|---|---|
| 6 / 7 / 8 | `AddNewBuildingOccupant(uint32, int32, int32, int32)` / `(uint32, float*, int32)` / `(cISC4BuildingOccupant*)` |
| 9 | `int64 GetCostOfOccupantDemolition(cISC4FoundationOccupant*, uint32)` (**non-adjacent**: declared after `DemolishFoundationOccupant`. **SDK error**: the SDK says `(void)`, but the method takes 8 bytes) |
| 10 | `int64 GetCostOfOccupantDemolition(cISC4BuildingOccupant*, uint32)` (non-adjacent) |
| 11 / 12 | `GetAllBuildingTypes(SC4Vector<uint32>&)` / `GetAllBuildingTypes(uint32* array, uint32& count)` |
| 18 / 19 | `GetBuildingTypesByFamily(SC4Vector<uint32>&, family)` / `GetBuildingTypesByFamily(hash set&, family)` (checked by body). The original's set is an STLport `hash_set`, **not** `std::unordered_set` as in the SDK, so never pass one |
| 23 / 24 | `AddNewFoundationOccupant(uint32, int32, float*, float, float, int, float)` / `(cISC4FoundationOccupant*)` |
| 32 / 33 | `GetDeveloper(DeveloperType)` / `GetDeveloper(PurposeType, WealthType)` |
| 37 / 38 | `GetExemplarName(type)` / `GetBuildingName(type)` (not overloaded; budget Errata round 2, item 2) |
| 42 / 43 | `GetBuildingMapKey(PurposeType, WealthType)` / `GetBuildingMapKey(uint32 type)` |

Because slot 9 moves up, everything from `GetAllBuildingTypes` to `DemolishFoundationOccupant` (header order)
sits one slot later than its header line count suggests.

The compiled return types differ from the SDK's in several more slots (building-development.md §7.1 has the full
table): slots 6, 7 and 47 return the building-occupant pointer (or null), not `bool`; slots 17, 18, 19, 21 and 27
return a count, not `bool` (a count that is a multiple of 256 reads as false in AL); slots 11 and 13 leave EAX
undefined; slots 42 and 43 return the key in AL only; slot 45 (`GetMaxCapacity`) returns a `float` in ST0, not
`int64`; slots 48 and 49 return `int32`. A replacement must follow the compiled types.

**`cISC4HistoryWarehouse`** (vtable `0x00A8CCB4`, 14 slots)

| Slot | Method |
|---|---|
| 9 | `Coalesce(uint32 type, cIGZDate const& start, cIGZDate const& end, int32)`: **SDK error**, the SDK has one date (16 bytes, not 12) |
| 10 / 11 | `Dispose(uint32 type)` / `Dispose(uint32 type, cIGZDate const& start, cIGZDate const& end)` |

`RecordHistoryData` (12) and `SetHistoryDataQueueLength` (13), which the budget uses, are not overloaded.

**`cISC4DisasterManager`** (earthquake manager vtable `0x00A9E090`, 12 slots)

| Slot | Method |
|---|---|
| 6 | `cISC4DisasterInstance* AttemptDisasterStart(int32 x, int32 z)` (the budget's RiskyMoney call, `+0x18`) |
| 7 | `cISC4DisasterInstance* AttemptDisasterStart()` (returns null for the earthquake) |

For the earthquake, the two integers are world coordinates (budget §4.3), whatever the SDK's parameter names say.

## 18. Arithmetic precision (x87) *[compat]*

The Windows build does all floating-point arithmetic on the x87 FPU (no SSE). The precision-control field of the
x87 control word therefore decides how every intermediate result is rounded. This section says which setting is in
effect, on which threads, and what that means for the specs.

### 18.1 What the game sets

| When | Where | Effect on the control word |
|---|---|---|
| Process start (C runtime init, before `WinMain`) | CRT `_cinit` → `_fpmath(1)` → `_setdefaultprecision` = `_controlfp(_PC_53, _MCW_PC)` | precision **53-bit**; rounding to nearest; all exceptions masked (Windows default) |
| Application start-up, first statement of `cSC4App::PreFrameWorkInit` (`0x00452360`), main thread | `CALL 0x005F85E0` (the first call after the prologue, verified in the bytes at `0x0045236C`), which calls the RZ framework's "set FPU precision" helper (`0x008929E0`) with mode 0 unconditionally | precision **24-bit** (PC = 00); rounding and exception masks unchanged |
| Every guarded Lua call (`cGZLua5Thread` / `cGZLua5` wrappers, the Lua thread library) | the same helper, guarded by the matching "get precision" helper (`0x00892A0A`) | if the entry precision is 24-bit: switch to **53-bit** until the call returns, including native callbacks and their re-entry (§18.4), then switch back to 24-bit. Entry at 53-bit or 64-bit skips both changes; an inner guard does not restore an outer guard's 24-bit setting |
| The exemplar float-expression parser (`nSCRes::ParseFloat` `0x0067BBA0` → `ParseExpression` `0x0067BB20` → the additive parser `0x0067BA40`, reached from `0x0067AC30` and `0x0067BAE0`) | the additive parser `0x0067BA40` calls the getter and, when it reports 24-bit, the helper with mode 1 — and then **returns with no mode-0 call in this chain** | precision **53-bit on return**. Verified in the bytes of `0x0067BA40` (one getter call, one setter call, no setter on the return path at `0x0067BACC`); no function in `0x0067AC30`, `0x0067B990`, `0x0067BA40`, `0x0067BAE0`, `0x0067BB20` or `ParseFloat` restores, and no direct caller of `ParseFloat` (e.g. `cCameraParamsCommand`, `cDecalLifeCommand`) wraps it. It can leave a persistent 53-bit setting; an already active outer Lua guard that entered at 24-bit can still restore 24-bit (§18.4). The unmodified save evidence excludes a persistent leak at those budget settlements, not every transient precision change. Reachability from **text** resource parsing, while shipped tuning and building data is binary, explains why this is a latent bug *[compat]* |
| Direct3D 7 renderer | `SetCooperativeLevel` with `DDSCL_NORMAL` (windowed) or `DDSCL_FULLSCREEN \| DDSCL_EXCLUSIVE`; **neither** `DDSCL_FPUPRESERVE` nor `DDSCL_FPUSETUP` | Direct3D 7 may set the FPU to single precision itself *[inferred from the DirectX 7 documentation]*. That is the same setting the game already chose, so it changes nothing |
| A DLL `LoadLibrary`d during app init, whose own CRT runs `_fpmath(1)` → `_controlfp(_PC_53, _MCW_PC)` | the CRT of that DLL | precision **53-bit** on the loading thread, unless a surrounding scope subsequently restores it. The unmodified save evidence excludes a persistent 53-bit setting at those budget settlements. `DSOUND.dll`, `OPENGL32.dll` and `WINMM.dll` are **static imports** (CRT init at process start, before our first measurement at 24-bit, so excluded from a later load-time change); `DDRAW.DLL` is **not** a static import and is loaded on demand |
| Truncating conversions through the C runtime's `_ftol` (`0x009EEF04`, the common path) | none: it converts with `fistp` to a **64-bit** integer under the current rounding (nearest), then corrects the result by one unit toward zero when rounding went the wrong way, so the result is truncation toward zero as an `int64` in EDX:EAX. Callers keep the low 32, 16 or 8 bits. NaN, ±∞ and magnitudes ≥ 2^63 give `0x8000000000000000`, whose low 32 bits are **0** | no change to the control word |
| Inline `fistp` to a 32-bit destination (where a spec says so) | depends on the site | NaN and out-of-range give `0x80000000` |

The helper's modes are 0 = 24-bit, 1 = 53-bit, 2 = 64-bit. Nothing in the game calls it with mode 2, and nothing
else writes the precision field. The C library's x87 math helpers (`pow`, `exp`, `log`, …) keep the caller's
precision (they only force round-to-nearest and masked exceptions while they run).

**A load-order fact worth knowing before reading the run logs.** The CRT row really does set 53-bit at process start:
the stored pointer `PTR___fpmath_00b18b88` holds `0x009EEEE5` (`__fpmath`), so `__cinit`'s null test passes and calls
it with 1, and `__setdefaultprecision` (`0x009F6C30`) is literally `push 0x30000; push 0x10000; call _controlfp` =
`_controlfp(_PC_53, _MCW_PC)`. Yet the plugin measures **24-bit** at `DLL_PROCESS_ATTACH`. Both are consistent only
if the GZCOM plugin load happens *inside* `PreFrameWorkInit`, after its first statement (`CALL 0x005F85E0`) has
already lowered the precision. So "24-bit at DLL attach" is **not** evidence about the CRT default, and it is not
evidence that anything ran before our DLL except that one assert. It does rule out our DLL's own load-time CRT init
as the cause of the later 53-bit reading (that init had already finished, at 24-bit).

### 18.2 Threads

- **Main thread: 24-bit baseline**, established by `PreFrameWorkInit`. The simulator, its agents
  (simulator.md), grid simulators, budget and message handlers run on this thread *[inferred: the simulator is
  driven by framework idle/timer callbacks and the game creates no simulation thread]*. Thread identity does
  **not** establish the precision of an individual invocation. A Lua guard temporarily selects 53-bit and native
  calls, including simulation reached by a nested modal frame, inherit that setting (§18.4). The scheduler does
  not assert 24-bit at callback entry. The exemplar parser can also leave 53-bit selected (§18.1).
- **Worker threads** created by the game (`cRZThread`: background model loader, HTTP client, name lookup,
  file-change notification; and the sound driver's thread) start with the Windows default for new threads
  (53-bit). None of them runs simulation code *[inferred: no simulator, agent or grid code was found reachable
  from their entry points]*.
- **Confirmed in a save of the unmodified game**: three monthly budget settlements produced the 24-bit result
  of a float-sensitive expression (budget.md §7.2 and Errata round 3: `300 × 0.41f × 100` gives 12300, not
  12299). In an in-game run with the FreeCity4 plugin loaded (run 19), the same original code produced the
  53/64-bit result. Something in that process left the main thread at a higher precision. **Testing rule:**
  a differential comparison against the **24-bit** rules requires PC = 00 at the corresponding native arithmetic.
  A callback at PC = 10 is instead a reference for the 53-bit context of §18.4. Harnesses must log the word and
  active Lua-call scope on the same OS thread at the affected call; a sample at an unrelated tick is insufficient.

**The expression really is precision-sensitive** (verified in the assembly of
`cSC4DepartmentBudget::RecalculateAllLineItemCurrentExpenses`, `0x006ABC93`): `FILD qword` (cost, exact) →
`FMUL float` (local funding) → `FMUL float [0x00A80AA0]` (the constant 100.0) → `FILD qword` (the running sum,
exact) → `FADDP` → `_ftol`. There is **no `FSTP float` between the operations**, so the intermediates stay in the
80-bit register and are rounded by PC alone. At PC = 24 the product rounds to 123.0 and the answer is 123; at
PC = 53 it stays 122.99999892711639…, truncates to 12299 and the answer is 122. The saved value 123 therefore
does establish that the unmodified game settled that budget at **24-bit**.

**What is verified about the control word (bytes and xrefs, 2026-10-13):**

- The helper `0x008929E0` is a real setter, 42 bytes: it clears the PC field of a stack copy of the control word
  (24-bit), sets bit 1 for mode 1 (53-bit) or bits 0–1 for mode 2 (64-bit), then `FLDCW`s it back. Ghidra's
  decompiler renders it as an empty function, so pseudocode cannot be trusted here. The getter `0x00892A0A`
  returns 0 for PC = 00, 1 for PC = 10, 2 for PC = 11.
- `0x005F85E0` (an unconditional `helper(0)` = 24-bit) has **exactly one xref in the whole image**: `0x0045236C`,
  the first call of `PreFrameWorkInit`. This is the startup assert, not a scheduler entry guard.
- Every other mode-0 call site is Lua-related and guarded by the getter. Because the guard skips **both** the
  set-to-53 and the set-back-to-24 when the thread is not already at 24-bit **at that guard's entry**. An outer
  guard whose entry was 24-bit still restores 24-bit when it returns, after all its native re-entry (§18.4).
- The exemplar float-expression parser leaks (§18.1, row 3): verified in the bytes of `0x0067BA40` — one
  `CALL 0x00892A0A`, one `PUSH 1; CALL 0x008929E0`, and no setter call on the return path at `0x0067BACC`.

**The enumeration of precision writes is complete** (direct xrefs, pointer-table scan of the whole image, and the
CRT chain, 2026-10-13). Nothing in the image calls `_controlfp`/`_control87` except the CRT's own
`__setdefaultprecision` (`0x009F6C30` → `0x00A02F9B`, one xref), which only `__fpmath` reaches; there is no
`_fpreset`; and no function pointer to the helper `0x008929E0`, the getter `0x00892A0A` or the 24-bit wrapper
`0x005F85E0` is stored anywhere in the image, so indirect calls cannot hide any site. Every remaining mode-0
(24-bit) site is Lua-related **and guarded by the getter**, and the guard skips both the set-to-53 and the
set-back-to-24 unless the thread is already at 24-bit. `0x005F85E0` — the only unconditional 24-bit assert — is
called from exactly one place, `PreFrameWorkInit`.

**Consequence:** a persistent change outside all restoring guards is not repaired by later Lua calls that
enter at the higher precision. A temporary change inside an active guard is a different case (§18.4).
The historical A/B observations are:

| Run | What computed the budget | `0x09188F42` / `0x09188F4C` | Precision implied |
|---|---|---|---|
| a save written by the **unmodified** game (Spandau) | the game's own code | **123 / 105** (`369 = 3 × 123`, `315 = 3 × 105`) | **24-bit** |
| run 19 (`build-runs/run19-original-observe.log`) | the game's own code — our DLL loaded, **observer only**, neither simulator nor budget replaced | **122 / 104** | **53-bit** |
| run 20 | our budget replacement | 123 / 105 | 24-bit (we reproduce 24-bit) |
| runs 23 and 24, `CW` logged directly | our simulator + budget | `CW = 0x027F` from PostAppInit-enter onward | 53-bit |

Runs 19 and the save run **the same game code on the same city**; the only difference is that FreeCity4.dll was
loaded in run 19. The original code therefore produced 122 with our DLL present and 123 without it, which is a
measurement that our process raises the precision — not an inference. (Run 19 predates control-word logging, so it
proves the effect but not the moment; runs 23/24 date it to between PreAppInit and PostAppInit.)

The save proves PC = 00 at those settlements. It does **not** prove that every simulation callback in the
unmodified game runs at 24-bit, or exclude a temporary 53-bit interval that an enclosing Lua guard restores.
The earlier inference that the whole session must remain at 24-bit is withdrawn (VC29-PC).

The float-sensitive worked examples recomputed for 24-bit (cde56f4) remain correct **in that context**. A native
53-bit result alone does not show that the original is computing incorrectly: establish whether an enclosing
Lua guard is active before treating it as a persistent precision leak (§18.4). The historical investigation
concerned the change between PreAppInit and PostAppInit. The plugin's own load-time CRT init is
excluded (the control word is still 24-bit at `DLL_PROCESS_ATTACH` *after* our initialisation), and run 19 had
nothing replaced, so the cause is not a replaced subsystem — it is something our DLL does just by being present.
The two candidates the enumeration leaves are our code calling a game interface that reaches `nSCRes::ParseFloat`
(a **text**-format resource parse — the leak in §18.1 row 4), or our code loading a DLL whose CRT runs
`_fpmath(1)`.

**What to do about it (queued for lane 1).** Bisecting the subsystem switches cannot find this: run 19 already had
every subsystem off (observer only) and still ran at 53-bit. The cause is in what the DLL does while being present,
in the window between PreAppInit and PostAppInit. Two cheap next steps, in order:

1. **Narrow the window.** Log the control word at finer granularity between the two hooks — after each framework
   service registration, each `QueryInterface` for a service, each class-registry replacement, and each
   `LoadLibrary`/`CoCreateInstance` our code performs. The window is currently ~2.4 s (23:16:59.073 PreAppInit →
   23:17:01.448 PostAppInit-enter, run 23) and contains a lot of our init.
2. **Test the minimal DLL.** Stage a build whose attach does nothing but open the log, and run it with the same
   autoload. If the control word still reaches 53-bit, the trigger is in DLL load/attach itself (a static
   initialiser, a CRT startup path, or a dependency we pull in — note that linking or loading a DLL whose CRT runs
   `_fpmath(1)` would do exactly this). If it stays 24-bit, bisect our init steps.

These historical diagnostics must distinguish an active Lua scope from a persistent change. Re-asserting
24-bit at every simulation entry changes the original's nested-callback behavior; it is **not** the native
boundary contract. Restoring a proven external persistent leak is separate from reproducing §18.4's re-entry.

**Calling conventions, verified in the bytes** (needed to call the helper safely):

- Setter `0x008929E0` is **cdecl, one int argument, no return value**. Body: `push ebp / mov ebp,esp / push ecx`,
  clear the PC field of a stack copy of the control word, `fstcw` it, OR in bit 1 if `[ebp+8]==1` (mode 1 = 53-bit)
  or bits 0–1 if `[ebp+8]==2` (mode 2 = 64-bit), then `fldcw` it back, `leave`, `ret`. The plain `ret` means **the
  caller pops the argument** — consistent with every call site doing `push 1; call; add esp,4`. Declare it
  `void (__cdecl *)(int mode)`; a `__stdcall`/`__thiscall` declaration would corrupt the stack.
- Getter `0x00892A0A` is **cdecl, no arguments, returns `char` in AL**: `fstcw` to a local, mask to `0x300`, return
  0 for PC = 00 (24-bit), 1 for PC = 10 (53-bit), 2 for PC = 11 (64-bit).
- The two Lua precision changes are guarded by the **saved entry mode**. A wrapper entered at 53/64-bit
  performs neither change. A wrapper entered at 24-bit restores that PC field after its entire call, including
  native callbacks. Differential results are qualified by this invocation context, not by assuming that every
  original result observed at 53-bit is wrong (§18.4).

### 18.3 What 24-bit precision means

Under 24-bit precision control, every x87 `fadd`/`fsub`/`fmul`/`fdiv`/`fsqrt` (and their integer-operand forms)
rounds its **exact** result to a 24-bit significand, round-to-nearest-even. That is the same as one IEEE `float32`
operation, with three differences that matter for exact reproduction:

1. **Operands are not rounded first.** A `double` or `int32` operand loaded from memory (`fld qword`, `fild`) keeps
   its full value in the register. `x × 0.1` with a `double` constant rounds once, which can differ from
   `float32(x) × float32(0.1)`. Likewise `int32` values above 2^24 enter exactly.
2. **Exponent range.** Results keep the 15-bit x87 exponent while in registers, so intermediate overflow and
   underflow do not happen where `float32` would overflow or go denormal. Shipped values never get near those
   limits.
3. **Loads, stores, compares and conversions are not affected.** Storing a result to a `double` keeps the
   24-bit value exactly. Storing to a `float` does not round again. Comparisons and `_ftol` truncation see the
   24-bit register value.

**Percentages use the float32 `0.01f`** (RS1, 2026-10-13). The shared percentage property getter `0x005FDAC0`
and every simulator that scales a percentage multiply by the float32 constant `0.01f` = `0x3C23D70A` =
0.0099999998 (at `0x00A823A4`; about 215 references). The image's only `double` 0.01 (`0x00A92688`) is referenced
12 times, all outside the simulation: the query dialog's window procedure (`0x004D3F40`), particle rendering
(`0x005B3DA0`, `0x005B8DC0`), one network-manager function (`0x006105C0`, four sites), bridge and support placement
in the network tool (`0x0062EEB0`, `0x0062F610`, `0x00643FB0`) and the lighting manager (`0x007D68B0`,
`0x007D8C80`). So a spec's "`× 0.01`" means "× 0.01f,
rounded to float32". Shipped percentages then become, for example, 10 → 0.099999994, 20 → 0.19999999,
80 → 0.79999995 (`0x3F4CCCCC`, an exact tie rounded to even) and 120 → 1.1999999, while 25, 50, 90 and 100 give
0.25, 0.5, 0.9f and 1.0. Under point 3 above, a product that is truncated before it is stored is truncated *after*
its 24-bit rounding: 1200 × 0.01f rounds to 12.0, so `_ftol` gives 12, where the unrounded 11.99999973 would give 11.

In the **24-bit baseline context**, specs that said a value "is computed in x87 extended precision" before
truncation or rounding mean, for the Windows build: **each operation is rounded to 24 bits as above**.
The live-precision exception, including native simulation inside Lua, is §18.4. *[compat]* FreeCity4 reproduces
the baseline by doing every simulation arithmetic step as one
`float32` operation (rounding each step), with the operand caveat of point 1: where a spec names a `double`
constant or a large integer operand, that operand is used **unrounded** for that one operation (compute in `f64`
and round the result to `f32`; for the four basic operations this gives the correctly rounded result, because
`f64` has more than twice the significand bits of `f32`).

The Mac build (GCC, SSE or x87 at 64-bit) differs; this section applies only to the Windows build.

### 18.4 Lua-to-native re-entry uses the live precision (VC29-PC)

**The original does not establish a separate 24-bit simulation boundary.** Native callbacks invoked from Lua,
and framework/simulation callbacks reached before that Lua wrapper returns, use the calling OS thread's live
x87 precision. The 24-bit rules of §18.3 describe the ordinary startup-established context, not an unconditional
property of a simulator method. An implementation reproducing these nested calls must carry the active
precision as invocation context. Forcing 24-bit around each scheduler callback, even with restoration afterwards,
changes original behavior. Precision is not a saved city field or a permanent higher-precision simulation mode.

**Boundary evidence (GOG 1.1.641, instructions checked with bytes):**

- The precision setter `0x008929E0` changes **only PC**, leaving the other control-word bits as they currently
  stand. The getter `0x00892A0A` returns the mode, not a saved whole control word. The unconditional startup
  selector `0x005F85E0` is not called by the scheduler.
- `cGZLua5Thread::Call` selects 53-bit at `0x008B7F57`, calls Lua at `0x008B7F6D`, and restores the saved
  24-bit mode at `0x008B7F7A` only after Lua returns. The C-closure path of Lua's dispatcher calls the native
  function directly at `0x008BBEB9`; it does not lower PC on entry or restore it on return. A nested guarded Lua
  call sees 53-bit at entry and performs neither precision change.
- The same scope includes protected-call error reporting: `CallProtected` selects at `0x008B8F50`, invokes
  Lua at `0x008B8FB9` or `0x008B8FDE`, can call the native error reporter at `0x008B9016`, then restores at
  `0x008B903A`. `ResumeThread` likewise brackets the resume (`0x008B8B7C`) and error reporter (`0x008B8BBF`)
  between its selector (`0x008B8B51`) and restorer (`0x008B8C03`). These are normal wrapper-return paths,
  including handled Lua errors; this does not promise restoration after an escape that bypasses the wrapper.
- Native modal re-entry really pumps the framework. The modal loop (`0x009DA84F`) invokes framework slot 16
  at `0x009DA8EA`. The **SC4 framework override** is `0x004735F0` (table entry `0x00A8C9B0`), not the
  base framework implementation `0x0087A812`. It reaches service callbacks at `0x0047350F` and
  `0x004736FF`, without a precision guard. The simulator's
  `OnTick` (`0x00703E70`) also has no guard: it reaches time passage, agents and idle simulation at
  `0x00703F38`, `0x00703F3F` and `0x00703F4E`. Its existing busy flag may suppress an already active
  simulator tick, but it does not select a precision. A callback that actually runs inherits the live setting.
  See also [modal-dialogs §2.3](modal-dialogs.md#23-the-loop-modalmessageloop-slot-63-0x009da84f).

With round-to-nearest and all exception masks unchanged, the normal scopes are:

| Entry to outer guarded Lua call | Inside Lua, its native callback, and a framework frame nested beneath it | After a nested guarded Lua call returns | After the outer wrapper returns |
|---|---|---|---|
| `0x007F` (PC 24) | `0x027F` (PC 53) | `0x027F` | `0x007F` |
| `0x027F` (PC 53) | `0x027F` | `0x027F` | `0x027F` |
| `0x037F` (PC 64) | `0x037F` | `0x037F` | `0x037F` |

This table assumes the callbacks do not themselves change the control word. The wrapper restores **PC** only
when its saved entry mode was 24-bit; it does not save/restore the caller's entire word. A 53-bit invocation is
therefore expected inside the first row's scope and is not, by itself, a precision leak.

#### Arithmetic and storage in the affected methods

At PC 53, round each arithmetic instruction's result to a 53-bit significand in the original order. Preserve
the actual memory widths and explicit stores: a float store still rounds to float32; a double store retains
the 53-bit result. Loads do not first round their operands to PC (§18.3). Thus replacing every `r24` with an
implicit float store is wrong, and widening all stored floats is also wrong. The following qualifies the named
specs' 24-bit formulas for re-entry; it does not change their ordinary 24-bit vectors.

| Method | Required operation and storage boundaries |
|---|---|
| regional-city §5, `GetCommodityBalance` (`0x004A7680`) | Getter order is **produced, imported, demanded, exported**. Form produced + imported, then demanded + exported, then subtract the second sum from the first. The arithmetic at `0x004A76A3`, `0x004A76C5`, `0x004A76CB` uses live PC. The first sum is stored as a **double** (`0x004A76AA`), not a float; the return stays in the x87 register. Double field storage does not imply PC 53 when entered at PC 24. |
| lot-developer §11.4, `GetAreaSlope` (`0x006C5DC0`) | Minimum/maximum heights and their vertex coordinates are stored as floats. The distance helper (`0x005D76A0`–`0x005D76C2`) does both coordinate subtractions, squares, sum and square root at live PC and returns in the register. Distance, cascade products, height difference and division remain in registers through the return (`0x006C5EAB`–`0x006C5F16`); there is no float spill between those steps. Constants such as 0.3f, 0.35f and 0.4f remain the original float values, loaded exactly. A caller's float store can round the returned slope again. |
| residential-education-health §4.7, newcomer EQ/HQ blend | The sum with newcomer Q, division by m, multiplication by delta, old-population product, addition and final division use live PC with no intervening float spill of those arithmetic results (`0x006FA96C`–`0x006FA97F`, HQ `0x006FA9D7`–`0x006FA9E9`). The resulting Q **is stored as float32** (`0x006FA983`, `0x006FA9ED`) before being passed to the grid setter; the upper-bound comparison still uses the register result, then substitutes 200 if needed. |
| residential-education-health §4.8, global EQ/HQ | At each tract, store the first wealth's product as float, then the second wealth's product plus that first result as float. Add the third wealth's product to that subtotal, then add the running sum; store the new running sum as float. EQ stores: `0x006F77D4`, `0x006F7805`, `0x006F7839`; HQ: `0x006F7927`, `0x006F7958`, `0x006F798C`. Each intervening operation uses live PC. The final division by population (`0x006F7863`, HQ `0x006F79B6`) returns in the register without a final float store in these methods. |

#### Worked cases for conformance

**Boundary and regional balance.** Give one commodity produced = 16777216, imported = 1, demanded = 16777216,
exported = 0, all exact doubles. Call the same read at each stage; the native routine changes no precision.

| Invocation | First sum | Second sum | Balance | Precision on return from the native read |
|---|---|---|---|---|
| Directly in ordinary PC 24 | 16777216 (the half-way addition rounds to even) | 16777216 | **0** | PC 24 |
| In a C callback of a guarded Lua call entered at PC 24 | 16777217 | 16777216 | **1** | PC 53 |
| In a framework callback dispatched by a modal loop beneath that C callback | 16777217 | 16777216 | **1** | PC 53 |
| After the outer Lua wrapper returns | 16777216 | 16777216 | **0** | PC 24 |

An inner Lua call within the C callback leaves PC 53 on return to that callback. Repeat with the outer entry
already at PC 53: the balance is 1 throughout and the outer return remains PC 53. Use the same inputs in both
contexts; neither a tolerance nor changed serialization is needed to express the distinction.

**Slope with a float sink.** Let the first minimum be height 0 at vertex (0,0), the first maximum height 13 at
(3,4), and all other sampled vertices strictly between those heights. The initial distance is exactly 5;
the >6 and >5 branches do not fire, then the >4 branch multiplies by 0.35f (`0x3EB33333`) and returns 13/d.

| Active precision | Scaled d | Slope in the return register | Slope stored by the caller as float32 |
|---|---|---|---|
| PC 24 | 1.75 | 7.4285712242126465 | **7.4285712242126465** |
| PC 53 | 1.7499999701976776 | 7.428571555079248 | **7.428571701049805** |

**Newcomer blend and global query.** Use old population 3, new population 7, delta 4 and both the current Q
and newcomer Q = float32 0.1 (`0x3DCCCCCD`, exactly 0.100000001490116119384765625). Disable decay and
generational reset for this case; Q > 0 gives m = 2 and the cap is inactive. The newcomer contribution is
0.4000000059604645 in both contexts, but the old-population product and final result differ:

| Active precision | Old-population product | Numerator after addition | New Q stored in the grid |
|---|---|---|---|
| PC 24 | 0.30000001192092896 | 0.7000000476837158 | **0.10000000894069672** (`0x3DCCCCCE`) |
| PC 53 | 0.30000000447034836 | 0.7000000104308128 | **0.10000000149011612** (`0x3DCCCCCD`) |

For a subsequent global EQ or HQ query in the same precision context, put those seven residents in R$ at
one tract, with all other populations zero and population counter = 7. The stored running sum is
0.7000000476837158 at PC 24 and 0.699999988079071 at PC 53. The final register quotients are respectively
**0.10000000894069672** and **0.09999999829701015**; if the caller stores that result as float32, the latter
becomes **0.10000000149011612**. The grid remains float32 in both runs.

#### Runtime observation still needed for reach29-views

The static boundary is settled; the cause of the particular broad-run intervals is not. The cited log already
shows a return to `0x007F` at tick 106 (line 3554), then `0x027F` at ticks 108 and 180 (lines 3560, 4181).
This is consistent with a temporary outer guard but does not identify the active call stack at each mismatch.
The four-shadow run's exact counts do not establish that stack either.

To settle attribution, capture, **without changing precision**, the same OS thread's control word and active
outer Lua-call depth at entry and exit of the guarded Call/CallProtected/ResumeThread used by the action;
capture the nested modal/frame entry and the affected balance, slope and residential invocation with their
input snapshots. One stack showing the affected invocation under the still-active Lua wrapper and modal
frame, PC 53 there, and PC 24 immediately after that outer wrapper returns establishes Lua re-entry. If no
such wrapper is active, or PC remains 53 after the last wrapper entered at PC 24 returns, the observation
instead requires tracing the first other precision writer. Replay the same recorded inputs using the
observed PC and the storage boundaries above. A tick-level word alone cannot distinguish these cases or
prove that all listed last-digit differences have the same cause. No native precision change or tolerance
relaxation follows from the log alone.

## 19. Threading model *[compat]*

No other spec states one, and getting it wrong is expensive: it decides whether core state must be `Sync`, and it
caused the run-23 and run-26 crashes (`0xC0000409`) that took two sessions to diagnose.

### 19.1 The original is single-threaded on the simulation path

The master clock and every simulator run on the main thread and have **no locking of any kind**. Verified by
searching the decompilation of `cSC4Simulator` and every `cSC4*Simulator` for `EnterCriticalSection`,
`LeaveCriticalSection`, `InitializeCriticalSection`, `InterlockedExchange`, `InterlockedCompareExchange` and
`WaitForSingleObject`: **zero hits**. Message delivery is synchronous on the calling thread — `cGZMessageQueue` and
`cGZMessageQueue2` only construct a `cRZThreadSignal`; there is no dispatch thread that would deliver messages from
elsewhere.

The game's worker threads (`cRZThread`: background model loader, HTTP client, name lookup, file-change
notification; and the sound driver's thread — §18.2) do not reach the clock or the simulators. *[inferred: from the
`cRZThread` users in the decompilation — `cRZHTTPClient`, `cRZNameLookup`, `cGZDBSegmentPackedFile` — none of which
holds a simulator or city reference. **Partly contradicted by measurement:** §19.2 caught a game thread calling `IsPaused`
off the main thread in runs 30 and 31, so at least one non-main thread does reach the clock. Its owning subsystem is not
identified.]*

**Consequence — corrected 2026-10-13.** This section previously said a replacement "must not assume the game will call it
concurrently." §19.2 disproved that: a game thread *does* call the clock concurrently. The accurate statement is narrower
and more dangerous:

- The original has **no locking**, so it cannot be relying on serialisation.
- It survives concurrent calls only because its clock getters **read plain members** — a race there yields a stale value,
  not a fault.
- A Rust replacement cannot inherit that tolerance. `RefCell`/`Cell` **panic** on the same access that merely races in
  C++, and with `panic = "abort"` the process dies (`0xC0000409`). This is exactly what runs 23 and 26 did.

So: do not assume the game serialises calls for you, and do not assume it will not call concurrently — it does both
erratically. Assume concurrent access and make the state `Sync` (§19.3 rule 1).

### 19.2 The concurrent reader is a *game* thread, not ours (measured, 2026-10-13)

This section previously claimed our own watchdog and autotest threads were the concurrent readers of core state. **That
was wrong, and measurement inverted it.** A detector built for the purpose caught, in runs 30 and 31, a **fourth thread**
calling `IsPaused` off the main thread — run 31: tid `22796`, against watchdog `17100`, autotest `20216` and main
`20232`, all distinct. That thread logged nothing else, so it is **not one of ours**; it is a game thread. The original
tolerates this because its clock getters read plain members, where our `RefCell` panicked.

What our threads actually do, from the code rather than from inference:

- `fc4_sim_*` is called from **exactly one file**, `cFC4Simulator.cpp`, and every call site is a GZCOM method that the
  *game* invokes. `FC4Watchdog.cpp` and `FC4Autotest.cpp` call **zero** of them.
- The **watchdog** (`FC4Autotest`/`FC4Diag`) emits `heartbeat (periodic)` lines. Its values do advance with the
  simulation across runs 8–28 (julian `2452170 → 2452180 → 2452191 → 2452201 …`; `pause 1 hidden 2 emergency 1` in the
  last run-28 heartbeat against `pause 0 hidden 0 emergency 0` earlier; and it reports what the main thread is doing at
  that moment: `delivering msg 0x6695681A (level 10) to agent 00E1E36C 'cSC4PowerSimulator' for 16 ms`, `main: OnTick
  (15 ms) > core fc4_sim_on_tick (15 ms)`). But it reads those from **plugin globals the main thread publishes** via
  `PublishSimState`/`DeliveryBegin` — not from live simulator state. Publishing is the main thread's act; the read is
  therefore a read of our own snapshot, not a concurrent read of core state.
- `fc4_date_from_julian` is pure.
- The **autotest** thread decides when the verdict is due and terminates the process.

**The run-26 attribution is superseded.** §19.2 previously inferred that run 26's panicking thread (tid `23860`, neither
main `8328` nor watchdog `22672`, reported `(thread unnamed)`) was the autotest thread, because every run was assumed to
have exactly three threads. There are at least four. Whether run 26's panic was the autotest thread or the game thread is
**not proven** and cannot be recovered from that run — the process aborted before anything else logged. Do not rely on
either attribution.

**On thread naming:** naming our threads would *not* have settled this. Rust prints `(thread unnamed)` for any thread
created through Win32 `CreateThread`, which is what the game uses. The panic sink therefore now classifies the faulting
tid against the known main/watchdog/autotest ids instead of relying on a name — that classification is what identified
the fourth thread.

**Consequence:** core state reachable from the clock *is* read concurrently, **by game code**, so it must be `Sync`. A
`RefCell` there panics under a concurrent read, and because the release build uses `panic = "abort"` the process dies
with `0xC0000409` — which is what run 23 and run 26 did. The fix is a `Mutex` held for exactly one `with()`/`read()`
section and **never** across a `deliver`/`post`/refcount hook, so a blocked getter waits microseconds rather than a
whole tick and cannot deadlock against a game lock that a delivering agent needs. This is unchanged and still necessary —
only the identity of the reader changed, which matters because chasing our own threads would never have found it.

### 19.3 Rules for a core author

1. Any state the clock exposes to a caller must be `Sync`: `Mutex`/`RwLock`/atomics, not `RefCell` or `Cell`. **The game
   itself calls clock getters off the main thread** (§19.2), so this is not a FreeCity4-only concern — any core reachable
   from the clock needs it. Main-thread-only state (for example the timer list, which `Init` installs and only `OnTick`
   reads) may stay a `RefCell` and should be documented as such.
2. Do not let the diagnostic threads read core state at all. Publishing a few values as atomics from the main thread and
   letting the heartbeat read those removes the contention instead of merely making it safe — this is what the watchdog
   already does, and it is why it never caused the crash. Where both are cheap, do both.
3. Naming your threads (`thread::Builder::name`) is still good hygiene, but **do not rely on a name to identify a
   faulting thread**: Win32-created threads report as unnamed. Classify the tid against the ids you know instead.
4. Never hold a core lock across a call into the game — a delivery, a message post, an `AddRef`/`Release`, or any
   vtable call. The game may re-enter us, and it may also call us from another thread while we hold it.
5. A temporary in an `if` condition lives through the whole `if` body, so `if self.st.borrow().loaded { …host call… }`
   holds a `RefCell` borrow across a host call even though it looks scoped. The zones core audited and fixed seven such
   sites; check for this shape explicitly.

### 19.4 What this does not settle

- **Whether the original's clock is ever called cross-thread: settled, yes.** §19.2's detector caught a game thread
  inside `IsPaused` in two separate runs. The earlier "no evidence of it, and no locking to support it" is retracted.
  §19.1's claim that the original neither serialises nor assumes concurrency still holds — it tolerates cross-thread
  reads precisely because its getters read plain members and nothing there can tear observably. **Copying that tolerance
  is not available to us**: Rust's `RefCell` panics where a plain member read merely races.
- **What the fourth thread is for.** It called `IsPaused` and nothing else in the observed window. Its purpose, its
  owning subsystem, and whether it calls other clock methods on a longer window are all unknown.
- **Which thread panicked in run 26** (§19.2). Not recoverable from that run.
- **What raises the x87 control word to 53-bit** between `PreAppInit` and `PostAppInit` (§18.2). One narrowing result is
  in hand: inside that ~2 s window our DLL makes exactly two calls (`director Release`, `director AddRef`) and nothing
  else runs. The next decisive step is a minimal-DLL test, not finer logging.

## Errata / clarifications (round 2)

1. **Null object values in property holders** (§9.2 vs §10.2, §14 test 18) *[compat]*. Both statements were about
   different things, and §10.2 / test 18 were wrong; they are corrected above. `ReadVariant` on a null reference
   leaves its destination unchanged, but the holder reader always reads into a **fresh, Empty** property, so the
   entry arrives Empty and is dropped by the "not Empty" rule. Non-null object entries arrive as `IGZUnknown`
   with an object and are dropped by the other rule. Result: **on load, every object-valued entry (null or not)
   is dropped; on save, both are written** (marker `0xA9BEE9AC` plus the reference, 0 for null). Loading such a
   holder and saving it again therefore removes those entries.

2. **Stream error handling** (implementer's question 7): confirmed. The game's streams expose only
   `GetError()` (compiled slot 18 of `cIGZIStream`) and no `SetError`. Serializable
   objects never put a stream into the error state themselves. A `Read` returns "`GetError() == 0` after
   reading everything", or false for a rejected version (with the stream untouched). All partial-failure
   behaviour comes from the stream's own sticky flag (§2.1, §2.9).

3. **Overload order** (§16): the deployed `FreeCity4.dll` of 14:50 (built with MSVC from SDK-order declarations)
   was compared slot by slot with the original budget vtable. Every slot with a readable `ret` matches, including
   18/19 and 27/28. The MSVC rule therefore held in practice, and the crash attributed to overload order must
   have had another cause.

4. **Variant overloads**: §17.1 shows that `cIGZVariant` follows the same rule (for example `GetValSint64(int64&)`
   is `+0x88`, not `+0x84`). It also lists two SDK signature errors there (`CanGetVal`, `GetValUnknown`).

5. **SDK signature errors** found in round 2 (details in §17 and the subsystem specs): `cISCPropertyHolder`
   (`AddProperty` ×2, `RemoveProperty`, `RemoveAllProperties`, `EnumProperties`), `cIGZVariant` (`CanGetVal`,
   `GetValUnknown`), `cIGZDate` (`operator±` return a day number), `cIGZPersistResourceManager`
   (`FindObjectFactory(key, …)`), `cISC4LotManager::CreateLotIfPossible`, `cISC4Lot::GetOccupancy` (by-value
   return), `cISC4BuildingDevelopmentSimulator::GetCostOfOccupantDemolition` (foundation overload),
   `cISC4HistoryWarehouse::Coalesce`, and `cISC4RegionalCity` (neighbors spec: setters take `double`,
   `GetWorldBaseElevation(float)` is a setter).

## Errata / clarifications (M3)

1. **Missing curve properties** (§4.1). The earlier text said that a missing property removes the curve. That is
   true only for callers that delete their own curve object (land value, aura, flammability, demand). The common
   "load by exemplar key and property ID" loader returns false and leaves the curve unchanged, so its callers end
   up with the previous, the code-default or an empty curve, depending on what they did before the load. §4.1 now
   lists the four patterns; every subsystem spec states its pattern per curve.
2. **Fire and police curves** (§4.1): they use the "default" pattern, not "keep" (protection §6). Police also
   has its own clock-seeded `cRZRandom` (§6.2).
3. **Plumbing curve, Gaussian-fast draw** (§4.1, §6.1): plumbing is not plain "default" (see the §4.1 table), and
   `RandomSint32RangeGaussianFast` is now exact.
4. **Power curves** (§4.1): power's tuning curves use the "default" pattern, not "keep" (code defaults are rebuilt
   before every load, power §8). Its per-building curves are read into a fresh temporary curve only when the
   property exists, so they behave like "delete" for each call.
5. **SDK signature errors found in M3 (power)**, each checked against the exe (callee-pop and epilogue):
   - `cISC4PowerSimulator` slots 45 / 46 `GetCurrentPlantEfficiencyPercent` / `GetCurrentPlantConditionPercent`
     (`0x006EB5F0`, `0x006EC820`): the SDK declares `float f(occupant)`. The game returns an `SC4Percentage` **by
     value through a hidden pointer** passed as the first stack argument; the callee pops 8 (pointer +
     occupant), returns that pointer in EAX and leaves nothing in ST0.
   - `cISC4PowerSimulator` slots 32 / 33 / 34 `GetOccupantPowerConsumed` / `…Produced` / `…MaxPowerProduced`
     (`0x006EB0D0` pop 8, `0x006EB420` pop 12, `0x006EB110` pop 8): declared `void`, but they return a **bool in AL**
     (upper EAX undefined).
   - `cISC4PowerSimulator` overloads compiled in reverse declaration order (§16): slot 16 is
     `GetMonthlyConsumption(x, z)` (`0x006EBA10`, pop 8) and slot 17 is `GetMonthlyConsumption()` (pop 0); slot 23
     is `GetPowerPlantCount(type)` (`0x006EFA00`, pop 4) and slot 24 is `GetPowerPlantCount()` (pop 0).
   - `cISC4NetworkTool` slot 10 `UpdateOnZoneChange`: the SDK declares no parameters. The game takes
     `(SC4Rect by value, uint32)` and the callee pops 20 (base `0x0062C900` `ret 0x14`; power-line tool override
     `0x006522F0`, pop 20).
