# GZ persistence runtime: records, persist resources, key lists and buffer resources — behaviour spec

Status: written 2026-10-21 (lane re-persistence4) from the Windows executable `bin/SimCity4.exe` (1.1.641, image base
`0x00400000`); every address is a Windows address. Implementers work from this spec, the public LGPL gzcom-dll SDK
headers and general knowledge only. They must **not** consult decompiled or disassembled code (see
[CONTRIBUTING.md](../../CONTRIBUTING.md)).

Tags: *[compat]* observable, a replacement must match it; *[choice]* a FreeCity4 decision; *[inferred]* consistent
with the evidence but not proven; *[quirk]* original behaviour that looks unintended but is reproduced.

## 1. Scope and owners

This spec covers the **framework runtime objects** of the GZ persistence layer: the objects a package hands out
for one entry (records), the typed field API on them, the base classes of persistent resources and their
factories, the resource key list, and the generic image resource (`cGZBufferResource`) with its factory and its
on-disk forms. The companion [persistence-manager.md](persistence-manager.md) specifies the resource manager
and its built-in cache (§10). It does **not** cover:

| Topic | Where |
|---|---|
| DBPF container, index, hole table, DIR, QFS/RefPack, package discovery/load order | [data-formats.md](data-formats.md) §2–§3 |
| SC4 segment streams (`cISC4DBSegmentIStream`/`OStream`), references, the save file | [foundations.md](foundations.md) §2–§3 |
| Variant memory types and the variant disk tags | [foundations.md](foundations.md) §9 |
| FSH / S3D resources | [graphics-formats.md](graphics-formats.md) |

Owner classes specified here (coverage-map owners; byte counts from `tools/coverage.py --owners`, 2026-10-21):

| Owner | Functions | Bytes | Sections |
|---|---|---|---|
| `cGZPersistDBRecord` | 9 | 207 | §2 |
| `cGZPersistDBSerialRecord` (incl. its folded `icf::` field helpers) | 49 | 6,172 | §3 |
| `cGZPersistResource` | 6 | 115 | §4.1 |
| `cGZPersistResourceFactory` | 14 | 680 | §4.2 |
| `cGZPersistResourceKeyList` | 22 | 1,071 | §5 |
| `cGZBufferResource` | 32 | 5,030 | §6 |
| `cGZBufferResourceFactory` | 21 | 1,517 | §7 |
| `cGZPersistResourceManager` (incl. attributed built-in cache and container helpers; 2026-10-22 selection) | 109 | 8,437 | §10 / companion |
| **total** | **262** | **23,229** | |

See [SC resources](sc-resources.md) for `cGZPersistBufferResource` (BMP/TGA),
`cGZPersistASCIIWriter`, `cSLTokenRecord`, its text adapter, and `nSCRes` script
services. Still outside this spec: the persist-buffer factory,
`cGZPersistBinaryReader`/`Writer`, `cGZPersistSurveyorWriter`,
`cGZPropertySet*`/`cPropertySet*`, and the remaining `cSL*`/`SL` owners.

## 2. DB record (`cGZPersistDBRecord`, `cIGZPersistDBRecord` IID `0xE56B8F03`)

A record is the object a DB segment (package) returns when an entry is opened. It holds *[compat]*:

| State | Set by | Notes |
|---|---|---|
| reference count, 16-bit | `AddRef`/`Release` | not thread-safe |
| access flags, 16-bit | constructor 0; `SetAccessFlags` (`0x0096D829`) | `GetAccessFlags` (`0x0096D824`) returns them unchanged; the segment decides what they mean (read 1, write 2, read-write 3 are the values its users pass, *[inferred]* from the SDF users in §7) |
| resource key (type, group, instance) | constructor; `SetKey` (`0x0096D80C`) copies all three words | `GetKey` copies them out |
| owning segment | constructor (AddRef'd if non-null) | released by `Close` or by the destructor |

- `QueryInterface` answers `cIGZPersistDBRecord` and `cIGZUnknown` (1) with the record itself and AddRef.
- `Release` (`0x009759CE`): if the count is above 1 it decrements and returns the new count; otherwise it destroys
  the record and returns 0 (*[quirk]* the count is never seen as 0).
- `Close` (`0x009759ED`): if the record has a segment, calls the segment's close-record entry with the record,
  releases the segment and forgets it, and returns the segment's answer; a record without a segment (already
  closed or never opened) returns false. A second `Close` therefore returns false and does nothing.

## 3. Serial record (`cGZPersistDBSerialRecord`)

### 3.1 Interfaces and lifetime *[compat]*

The serial record extends §2 with a position, a size and typed field access. Concrete records (DBPF packed file
read/write records, the SDF file record, the Win32 bitmap-resource record) derive from it and provide four raw
operations: read *n* bytes at the position, write *n* bytes at the position, size/position/seek, and close.
Every typed operation below is built on those raw operations.

`QueryInterface` answers:

| IID | Returns |
|---|---|
| `0xC56B8F08` (`cIGZPersistDBSerialRecord`) | the record |
| `0xC51B11CB` | an embedded `cIGZOStream` adapter *[inferred: its destructor is `cRZOStream`'s]* |
| `0x051B11BD` | an embedded `cIGZIStream` adapter *[inferred: its destructor is `cRZIStream`'s]* |
| anything else | §2 |

The adapters share the record's reference count and position; the stream `SetVoid`/`GetVoid`/`SetGZStr`/
`SetRZCharStr` entries forward to the record's raw write/read and to `SetFieldString` (§3.4). An `SetRZCharStr(s)`
writes exactly what `SetFieldString` writes for `s`.

Each record carries a one-byte **native-order flag**, fixed at construction. The SDF file record (`0x00979985`)
and the bitmap-resource record (`0x0097CCFF`) pass 1; the DBPF packed-file records pass the value their
segment supplies (see question PR-1). Every shipped package and save is little-endian (foundations §2.2), so the
flag is 1 in practice.

### 3.2 Integer fields *[compat]*

For each width (8, 16, 32, 64 bits), signed and unsigned versions are the same operation (same bytes). Each comes
as a scalar form (`GetFieldUint32(v&)`) and an array form (`GetFieldUint32(v*, count)`).

| Operation | Bytes | When the native-order flag is 0 |
|---|---|---|
| `GetField*8` / `SetField*8` | 1 per element | no change |
| `GetField*16` / `SetField*16` | 2 per element | bytes swapped |
| `GetField*32` / `SetField*32` | 4 per element | bytes reversed |
| `GetField*64` / `SetField*64` | 8 per element | bytes reversed |

Rules:

- A get reads `width × count` bytes with one raw read and returns the raw read's result. With the flag 0 it then
  reverses each element **in place, whether or not the read succeeded** *[quirk]* (`0x00975D13`, `0x00975D4C`,
  `0x00975D9A`, `0x00975DE5`, `0x00975E19`, `0x00975E49`).
- A set with the flag 0 reverses into a scratch copy (256 elements on the stack, larger counts on the heap) and
  writes the copy; **the caller's array is not modified** (`0x00975ED0`, `0x00975FA4`, `0x0097608B`). With the
  flag 1 it writes the caller's bytes directly.
- An array of count 0 is a zero-length raw read/write.

### 3.3 Float fields *[compat]*

`GetFieldFloat32/64` and `SetFieldFloat32/64` (scalar and array, `0x00975A86`–`0x00975B54`) are **raw** reads and
writes of 4 or 8 bytes per element. *[quirk]* They are **never byte-swapped**, even when the native-order flag is 0.
A big-endian record therefore stores integers big-endian and floats in host (little-endian) order. FreeCity4 only
needs the flag-1 behaviour; a reimplementation that supports flag 0 must keep this asymmetry *[compat]*.

### 3.4 Strings *[compat]*

- `SetFieldString(s)` (`0x00976139`): writes `uint32 length` (the string's byte length, through the §3.2 32-bit
  setter) then the bytes with no terminator. The top byte of the length word is never set by this writer.
  If the length write fails, the bytes are not written and the result is false.
- `GetFieldString(s)` (`0x00977054`): the reader of foundations §2.3 *exactly* (it is the same routine the SC4
  segment stream uses): header `encoding = top byte`, `count = low 24 bits`, `count > 200,000` fails, encoding
  `0x10` reads `2 × count` bytes as UTF-16 (swapped when the flag is 0) and converts them, the "second byte is 0"
  UTF-16 guess, and the legacy byte-by-byte path for other encodings. Two refinements:
  - When fewer than 4 bytes remain for the header, the reader clears the string and appends the remaining bytes
    one at a time; it returns **true** (foundations §2.3 quirk 2).
  - Legacy path: steps back 4 bytes, clears the string and appends as many single bytes as the record's total
    size, ignoring failed reads; returns true (foundations §2.3 quirk 3).
  - Buffers up to 1,024 bytes are read on the stack; larger ones on the heap. The result is the read's result.

### 3.5 Resource keys *[compat]*

`SetFieldResKey(key)` writes type, group, instance as three 32-bit fields (§3.2) in that order and stops at the
first failure (`0x00975BB5`). `GetFieldResKey(key)` reads them back in the same order, stopping at the first
failure; fields after a failure keep their old values (`0x00975ACA`).

### 3.6 Variants *[compat]*

`SetFieldVariant(v)` (`0x00976AB2`) and `GetFieldVariant(v**)` (`0x0097621F`) use the encoding of foundations §9.2
**without the marker word**: `uint32 diskTag`, then the value. The memory-type → disk-tag table is the one of
foundations §9.2 (a 17-entry table at `0x00AD8C50` indexed by the memory type without its array bit; the array bit
is copied). Memory types whose tag has no writer case — IGZUnknown (tag `0x0D`), VoidPtr (`0x12`) and their
arrays (`0x800D`, `0x8012`) — write the tag word and then fail: object references cannot be stored here.
`Char` and `RZUnicodeChar` scalars (tags `0x11`, `0x10`) are written as one and two bytes but are not accepted by
the reader (foundations §9.2 quirk).

- Element encodings use the §3.2/§3.3 operations of the element's width (so integer elements follow the
  native-order flag, floats do not).
- Arrays: `uint32 count` then the elements. Disk tag `0x8000` (void array): count then raw bytes. Disk tag
  `0x800C` (string): the §3.4 string, its header being the count.
- `GetFieldVariant` creates a **new** variant (refcount 1) and returns it through `v**` only on success; on any
  failure (tag read fails, element read fails, unknown tag) it releases the new variant and returns false.
  Disk tag 0 yields an Empty variant and true.
- Reader tag set: scalars `0x00`–`0x0C`, arrays `0x8000`–`0x800C`, `0x8010`, `0x8011`; any other tag fails.
  Scalars `0x0B` (Bool) and `0x0C` are each read as one byte. Array element buffers up to 128 elements are on
  the stack, larger ones on the heap.

## 4. Persistent resources and their factories

### 4.1 Resource base (`cGZPersistResource`, `cIGZPersistResource` IID `0x456B8F1D`) *[compat]*

- Constructed with reference count 0 and a zero key (`0x00975633`).
- `AddRef` is a **thread-safe** (interlocked) increment (`0x009753B7`); `Release` (`0x009753C5`, shared by the
  derived resources) is the matching interlocked decrement: when the result is 0 it destroys the object and
  returns 0, otherwise returns the new count.
- `QueryInterface` answers `cIGZPersistResource` and `cIGZUnknown` with the object and AddRef.
- The resource key (type, group, instance) is set by the resource manager when it registers or loads the
  resource; `GetKey`/`SetKey` copy it. `GetType` returns the class's own type ID (§6: `0x62B9DA24`).

### 4.2 Factory base (`cGZPersistResourceFactory`, `cIGZPersistResourceFactory` IID `0xA56B8F17`) *[compat]*

The factory base has a reference count guarded by a critical section (both `AddRef` and `Release` take it).
Native vtable `0x00AD8C20` distinguishes record construction from type construction:

| Slot | Public entry | Behaviour |
|---|---|---|
| 3 | `CreateInstance(record, iid, out, a, b)` | under lock, get record key.type; if a=b=0 allocate via hook slot 9 `(type, resourceIID, outResource)`, otherwise call slot 4 with those forwarded arguments. On allocation success set resource key from record, Read via slot 5, then QueryInterface to requested IID only if Read succeeds; release temporary resource. Return false on allocation/Read/QI failure (`0x0097554F`, folded export label is misleading) |
| 4 | `CreateInstance(type, iid, out, …)` | calls hook slot 9 `(type, iid, out)`; this base forwarding body ignores the final two arguments (`0x0097553B`) |
| 5 | `Read(resource, record)` | under the factory's lock, QueryInterfaces the record for `cIGZPersistDBSerialRecord`; if that succeeds calls hook slot 10 `SerialRead(resource, serialRecord)`, otherwise returns false (`0x00975495`) |
| 6 | `Write(resource, record)` | same with hook slot 11 `SerialWrite` (`0x009754E8`) |
| 7 | (destructor) | |
| 8 | `CreateInstance(record, iid, out)` convenience form | calls slot 3 with final arguments (0, 0) (`0x0093FF03`) |
| 9 | type-allocation hook | derived factory implements `(type, iid, out)` |

The record/type ordering above corrects the earlier reversed slot labels: the native table points slot 3
at `0x0097554F` and slot 4 at `0x0097553B`; manager load calls factory offset `0x0C` with a record, while
explicit creation calls offset `0x10` with key.type (`persistence-manager.md` §6). Native record-construction
branches and Read/QI sequence: `0x00975566–0x00975605`.

`Release` destroys the factory when the count is below 2 at entry (returns 0); otherwise decrements and returns
the new count. A factory that cannot write implements `SerialWrite` as "return false".

## 5. Resource key list (`cGZPersistResourceKeyList`, `cIGZPersistResourceKeyList` IID `0x656B8F12`) *[compat]*

An ordered sequence of resource keys (a deque), **duplicates allowed**:

| Operation | Behaviour |
|---|---|
| `Insert(key)` (`0x00974F39`) | appends the key at the end, even if present; returns true |
| `Insert(list)` (`0x00974F5B`) | appends every key of the other list in its enumeration order |
| `Erase(key)` (`0x00974EA8`) | removes the **first** equal key (type, group and instance all equal); true if one was removed, false otherwise |
| `EraseAll()` (`0x00974C62`) | removes all keys; true |
| `IsPresent(key)` (`0x00974C6D`) | linear search; true if an equal key exists |
| `Size()` (`0x009749BE`) | number of keys |
| `GetKey(index)` (`0x00974741`) | the key at `index` in insertion order; *[quirk]* no bounds check (callers must pass `index < Size()`) |
| `EnumKeys(callback, context)` (`0x009749CB`) | calls `callback(key, context)` for each key in order, each with a copy of the key |

The count is a plain 32-bit counter (not thread-safe); `Release` destroys the list when the count is ≤ 1 at entry.

## 6. Image buffer resource (`cGZBufferResource`, type `0x62B9DA24`)

### 6.1 Object *[compat]*

A `cGZBufferResource` is a persistent resource (§4.1) that owns one `cIGZBuffer` (a 2-D pixel buffer). The
constructor (`0x0097D636`) creates the buffer through the COM director (class `0xC470D325`, interface
`0x20732180`); the destructor releases it.

- `QueryInterface(iid)` first asks the **buffer**: an interface the buffer answers is returned from the buffer.
  Otherwise IID `0x22B9DA52` (`cIGZBufferResource`) returns the resource's own interface, and anything else
  goes to §4.1.
- `GetBuffer(out)` QueryInterfaces the buffer for `0x20732180`; `GetBufferAs(iid, out)` for any IID. Both do
  nothing when there is no buffer.
- `SetBuffer(b)` (`0x0097DCCD`) AddRefs `b` (if non-null), releases the old buffer and stores `b`.

The buffer's **colour type** decides its bits per pixel *[compat]* (`0x00449500`):

| Colour type | Bits per pixel |
|---|---|
| 2 | 8 |
| 3, 4, 5, 6, 7 | 16 |
| 8, 9 | 32 |
| anything else | 0 (initialisation fails) |

### 6.2 Loading (`LoadFromSerialRecord(record, target)`, `0x0097D6D9`) *[compat]*

`target` is a (colour type, bits per pixel) pair: the format the caller wants the pixels in. The call fails if
the record or the resource's buffer is null. It reads the first **8 bytes raw** (never byte-swapped) and
dispatches:

| First bytes | Form | Then |
|---|---|---|
| first 16-bit word `0x4D42` ("BM") | Windows BMP file (§6.4) | reads 6 more bytes (the rest of the 14-byte file header) |
| first word `0x62B9DA24` | legacy form v0 | second word = width; reads `uint32 height` |
| first word `1` | form v1 | second word = data size; reads `uint32 width`, `uint32 height`, `uint32 colourType` |
| first word `2` | form v2 | second word = compressed size; reads `width`, `height`, `colourType` likewise |
| anything else | — | fails |

The 32-bit header words after the first 8 bytes use §3.2 (they follow the native-order flag).

**v0** (`0x0097CA88`): initialises the buffer at `width × height` in the target format, locks it for writing,
reads `width × height` 32-bit pixels (§3.2 array) in row-major order (row 0 first) and stores each through the
buffer's set-pixel entry, which converts the 32-bit value (ARGB *[inferred]*) to the buffer format. The result is
the array read's result.

**v1** (`0x0097D21F`): initialises the buffer at `width × height` in the **stored** colour type (with its bits per
pixel from §6.1), locks it, reads `dataSize` raw bytes of uncompressed pixels and, on success, converts the buffer
to the target colour type. If the buffer's row stride equals `width × bytesPerPixel` the bytes go straight into
the buffer; otherwise they go to a scratch copy and `height` rows of `width × bytesPerPixel` bytes are copied.
*[quirk]* In that copy the **source** advances by the buffer's stride and the **destination** by the packed row
size (swapped relative to v2), so a padded buffer receives wrong rows (`0x0097D2E7`–`0x0097D308`). FreeCity4
copies packed rows into stride-spaced rows *[choice]*; buffers created by FreeCity4 always have
stride = packed row size, where both behaviours agree.

**v2** (`0x0097D345`): as v1, but the `compressedSize` bytes are decompressed with the QFS/RefPack codec of
data-formats §2.7 and the load fails unless the output is exactly `width × height × bytesPerPixel` bytes. Rows are
copied packed-source to stride-spaced destination (correct order).

In v1 and v2 the final conversion to the target colour type decides the result; in all forms the buffer is
unlocked before returning.

### 6.3 Saving (`SaveToSerialRecord(record, version)`, `0x0097D675`) *[compat]*

Fails if the record or the buffer is null, or if the buffer cannot be locked for reading.

- `version = 2` (`0x0097D4A8`): packs the rows (dropping stride padding), compresses them with the QFS/RefPack
  compressor (output buffer `size × 1.5 + 1,024` bytes), and writes `uint32 2`, `uint32 compressedSize`,
  `uint32 width`, `uint32 height`, `uint32 colourType` (§3.2), then the compressed bytes raw. Compression failure
  writes nothing and fails.
- `version = 0x55D4FB53` (`0x0097CB63`): a 24-bit Windows BMP, **only for 16-bit buffers** (any other depth
  fails, nothing written). File header: `"BM"`, file size `rowBytes × height + 54`, two reserved 0 words, data
  offset 54. Info header: size 40, width, height (positive: bottom-up rows), planes 1, 24 bits, compression 0,
  image size `rowBytes × height`, then four zero words. `rowBytes = width × 3` rounded up to a multiple of 4.
  Pixels: the buffer's 16-bit-to-RGB conversion, stored blue, green, red; the buffer's row 0 is written last.
  The three blocks are written raw; their write results are ignored and the call returns true *[quirk]*.
- Any other version: nothing written, false. (v0 and v1 are never written.)

### 6.4 BMP input (`0x0097D0C5`) *[compat]*

After the 14-byte file header, reads the 40-byte info header. Accepted only if compression is 0 and the bit depth
is 4, 8 or 24; the file header's data offset and the info header's height sign are not used. The buffer is
initialised at the BMP's width and height in the **target** format and locked for writing. A palette follows for
4-bit (`colorsUsed × 4` bytes, 64 when `colorsUsed = 0`) and 8-bit images (`colorsUsed × 4`, 1,024 when 0); then
`rowBytes × height` pixel bytes with `rowBytes = ceil(bits × width / 8)` rounded up to 4. Supported (source bits →
target bits) translations: 4 → 16, 8 → 8 or 16, 24 → 16, 24 or 32; any other pair fails. The result is the
translation's result.

### 6.5 Shipped-data inventory (PR2)

A read-only scan on 2026-10-24 found no entries of type `0x62B9DA24` in GOG
1.1.641 hotfix installer 25621: 857 DBPF package paths, representing 440 distinct
package contents and 232,303 distinct-content index entries. This includes all
417 shipped city saves, their identical installer support copies, and the root,
locale, SKU and configuration packages. Thus no BMP, v0, v1 or v2 buffer-resource
header occurs under this type in the enumerated corpus.

The [scan report](evidence/persistence-pr2-gog-25621.md) records provenance,
reproduction, positive-control counts and limitations; the accompanying manifest
contains hashes and metadata only. This observation does not change §§6.1–6.4,
prove absence in other editions, user saves or plugins, or settle PR3's separate
question about runtime callers depending on padded v1 loads.

## 7. Buffer resource factory (`cGZBufferResourceFactory`, interface IID `0xA2C72982`) *[compat]*

A §4.2 factory for type `0x62B9DA24` with two settings:

- **Target format** (colour type, bits): default **(3, 16)** (`0x0097DBBE`); `SetColorType(pair)` replaces it.
  Every load through the factory uses it as the §6.2 target.
- **Write-version stack**: starts as `[2]`. `PushWriteVersion(v)` pushes; `PopWriteVersion()` pops and returns true
  only if more than one entry remains (the bottom entry is never removed; false otherwise); `GetWriteVersion()`
  returns the top (0 if empty, which cannot happen).

| Entry | Behaviour |
|---|---|
| `CreateInstance(type, iid, out)` (`0x0097D833`) | only for type `0x62B9DA24`: creates a resource and QueryInterfaces it for `iid`; the creation reference is released, so `out` holds the only reference. Other types: false |
| `SerialRead(resource, record)` (`0x0097CEB4`) | only if `resource.GetType() = 0x62B9DA24` and it answers `0x22B9DA52`: §6.2 with the factory's target format |
| `SerialWrite(resource, record)` (`0x0097CF04`) | same check, then §6.3 with `GetWriteVersion()` |
| `LoadFromSerialRecord(out, record)` | temporary resource, §6.2 with the factory's target, then `GetBuffer(out)` on success |
| `SaveToSerialRecord(buffer, record)` | false if either is null; temporary resource holding `buffer`, §6.3 with the current write version |
| `LoadFromFile(path, out, …, target)` (`0x0097D889`) | opens the file as an SDF record with access 1 (read), §6.2 with the **caller's** target, `GetBuffer(out)`, closes the record |
| `SaveToFile(path, buffer)` (`0x0097D929`) | false if `buffer` is null; opens the file as an SDF record with access 3 (read-write), §6.3 with the current write version, closes |
| `LoadFromResourceId(…)` (`0x0097D9D8`, `0x0097DA48`) | reads a Win32 bitmap resource of the given module (the first form uses the executable's own module and the caller's target; the second the given module and the factory's target) through the bitmap-resource record, which presents it as a BMP file (§6.4) *[inferred]* |

## 8. Questions

Open questions live in [questions/persistence.md](questions/persistence.md).

## 9. Conformance tests (must exist in `engine/conformance`)

1. Serial record, flag 1: `SetFieldUint32(0x11223344)` writes `44 33 22 11`; flag 0 writes `11 22 33 44`;
   `SetFieldFloat32(1.0)` writes `00 00 80 3F` under both flags (§3.3).
2. Flag 0 array set leaves the caller's array unchanged (§3.2).
3. `GetFieldString` on header `0x00000003` + `"abc"` returns "abc"; on header `0x00030D41` (200,001) fails (§3.4).
4. `GetFieldVariant` on `03 00 00 00 2A 00 00 00` returns a Uint32 variant 42; on tag `0x0D` fails (§3.6).
5. Key list: insert A, B, A; `Size` 3; `Erase(A)` removes index 0; `GetKey(1)` is A (§5).
6. Buffer resource v2 round trip at 16-bit colour type 3: save, load with target (3, 16), identical pixels (§6).
7. v2 load with a compressed stream that inflates to one byte fewer than `w × h × bpp/8` fails (§6.2).
8. 24-bit BMP save of a 2×2 16-bit buffer: 54 + 2 × 8 = 70 bytes, row padding 2 bytes, rows bottom-up (§6.3).
9. Factory write-version stack: `Pop` on `[2]` returns false and `Get` stays 2 (§7).

## 10. Resource manager (`cGZPersistResourceManager`)

The complete manager contract is in [persistence-manager.md](persistence-manager.md): load/create dispatch,
factory and type multimap order, ordered segments, exact-key resource caching, ownership, retry/failure,
direct records, writes, lifecycle and the attributed built-in cache. Its §11 records all 109 selected
functions (8,437 B); §10 supplies 30 independent proposed conformance vectors PM01–PM30. These are spec
vectors, not executed conformance or game/shadow results. PR4 tracks the service update's uninitialized
return byte; callback effects are deterministic and specified.
