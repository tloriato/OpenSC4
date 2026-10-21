# Game data formats as the original loads them — behaviour spec

Status: round 1 (M6), written from the Windows 1.1.641 executable (`bin/SimCity4.exe`, image base `0x00400000`) and
checked against the shipped data of SimCity 4 Deluxe (GOG install, read-only). `engine/formats` (written from
public documentation) was the starting point; §11 lists where the original differs from it.

Tags: *[compat]* observable, must match; *[choice]* FreeCity4 decision; *[inferred]* not proven;
*[quirk]* original behaviour that looks unintended but is reproduced (or that a replacement must survive);
*[compat-limit]* depends on something outside the game (file system order, OS).

Shared material cited and not repeated here: stream primitives, strings and resource keys (foundations §2.2,
§2.3, §2.5), the city save container (§3.1), response curves and their four missing-property patterns (§4.1),
variants and their disk tags (§9), property holders in saves (§10), arithmetic precision (§18), slot tables of
`cIGZVariant`, `cISCPropertyHolder`, `cISCResExemplar` and the resource manager (§17.1, §17.2, §17.5, §17.6).

---

## 1. Scope

| Item | Where |
|---|---|
| DBPF packages: header, embedded-header scan, index, holes, DIR, entry reads | §2 |
| QFS (RefPack) decoder | §2.7 |
| Resource manager: segments, load order, priority, cache | §3 |
| Exemplars and cohorts: binary and text forms, parents, duplicates | §4 |
| Property getters and type coercion | §5 |
| LTEXT and locale selection | §6 |
| Tuning catalogue (M1–M5) | §7 |
| Interfaces and replaceability | §8 |
| Conformance tests | §10 |

Survey of the shipped data used throughout (scripts in the scratch folder, read-only):

| Data | Result |
|---|---|
| Root `.dat` (8), `<Language>\SimCityLocale.DAT` (15), `Sku_Data\Sku1_US\Plugin_002_TheLivingMall.dat` | all DBPF 1.0, index 7.0, 20-byte entries, no holes, no duplicate key inside a file, every compressed entry uses signature `10 FB` |
| City saves (in the install's `Regions` and the 417 user saves, engine/formats README) | DBPF 1.0, index 7.0; holes present (e.g. 73 holes, hole table 584 bytes = 73 × 8) |
| Exemplars / cohorts (root, Sku, English) | 8,957 `EQZB1###`, 388 `CQZB1###`, 2 `EQZT1###`; all 9,345 binary records parse with the grammar of §4.2 and end exactly at the record end; 152,175 properties, all of kind 0 (§4.2); no duplicate property ID inside a record; 3,461 have a parent |
| Property tags used (binary) | `0x8003` 89,943; `0x0003` 17,067; `0x0001` 12,124; `0x800C` 9,578; `0x8001` 9,459; `0x8009` 5,597; `0x000B` 2,929; `0x0009` 2,326; `0x0008` 1,798; `0x8007` 776; `0x0007` 389; `0x8008` 185; `0x800B` 3; `0x0002` 1. One-element arrays: 4,634 Uint8, 3,564 Uint32, 168 Sint64, 119 Float32, 7 Sint32 |
| LTEXT (English locale) | 4,309 + 1,651 (compressed) UTF-16 records, 1 8-bit record, 1 headerless text record; `Sound.dat` stores audio (XA, RIFF) under the same type |

---

## 2. DBPF packages

### 2.1 Header *[compat]*

A package is opened by the packed-file segment (`cGZDBSegmentPackedFile`, class `0x086D4A89`). The header is
96 bytes (`0x60`), little-endian:

| Offset | Field | Checked by the original |
|---|---|---|
| `0x00` | magic `"DBPF"` | must match |
| `0x04` | major version | must be **≤ 1** |
| `0x08` | minor version | not checked |
| `0x0C`–`0x1F` | user fields, flags, dates | not used |
| `0x20` | index major version | **not checked** |
| `0x24` | index entry count | must be `< 0x07FFFFFF` |
| `0x28` | index offset | must be `< file length`, and `offset + index size ≤ file length` |
| `0x2C` | index size (bytes) | (above) |
| `0x30` | hole count | must be `< 0x07FFFFFF` |
| `0x34` | hole table offset | must be `< file length`, and `offset + hole size ≤ file length` |
| `0x38` | hole table size (bytes) | (above) |
| `0x3C` | index minor version | **not checked** |
| `0x40`–`0x5F` | reserved | not used |

- *[compat]* The index minor version is ignored: entries are **always 20 bytes** (§2.3). A package written with
  index 7.1 (24-byte entries) is misread: the fourth field of each 24-byte entry is taken as the offset. With the
  usual resource ID of 0 the offset is 0, which fails the entry check of §2.3, and the package does not open.
  The original therefore does not support index 7.1. *[choice]* FreeCity4 in standalone mode may accept 7.1 as an
  extension, but writers must produce 7.0.
- A header that fails any check is retried **once** with the embedded-header scan (§2.2). If that fails too, the
  package does not open (the segment is not usable; for a plugin it is skipped).

### 2.2 Embedded header scan *[compat]*

When the header at offset 0 is not acceptable, the reader searches the file for an embedded package: it scans in
512-byte windows (overlapping by 16 bytes) for the 16-byte marker

```
80 9D 88 EC 8F 24 03 6C C9 A6 31 56 5B CF 77 22
```

followed immediately by `"DBPF"`. The package then starts at the byte after the marker: every offset in that
header, its index, hole table and entries is relative to that start. Files of 16 bytes or less are not scanned.
A marker not followed by `"DBPF"` is skipped; the first marker followed by `"DBPF"` is used, and if that header
fails the checks of §2.1 the package does not open. (The marker is stored in the executable with its first byte inverted, so the
executable itself never matches.) No shipped file uses this. *[choice]* FreeCity4 implements the scan only if a
real file needs it; it must not treat the marker as part of the format otherwise.

### 2.3 Index *[compat]*

`count` entries of 20 bytes from the index offset:

| Offset | Field |
|---|---|
| 0 | type ID |
| 4 | group ID |
| 8 | instance ID |
| 12 | data offset (relative to the package start) |
| 16 | data size (bytes stored) |

- **Entry check at open**: every entry must satisfy `offset ≥ 0x60`, `offset < file length` and
  `offset + size ≤ file length`. **One bad entry makes the whole package fail to open** (not just that entry).
  Zero-size entries must still have an offset in range.
- **Duplicate keys inside one package: the first entry wins.** The index is loaded into a map keyed by
  (type, group, instance) with insert-if-absent, so a later entry with the same key is ignored. (No shipped
  package contains duplicates.)
- The index size field is not compared with `20 × count`; the reader reads `20 × count` bytes.

### 2.4 Hole table *[compat]*

`count` entries of 8 bytes (`uint32 offset, uint32 size`) read from the hole table offset. When the count is
non-zero, the open also requires that the **table offset** is 0 or lies in `[0x60, file length)`, and that
`count = table size >> 3`. The individual hole entries are not checked. Holes are free space left by in-place
rewriting of saves; readers ignore their contents. Writers of new packages write no holes *[choice]*.

### 2.5 DIR record *[compat]*

The entry with key (type `0xE86B1EEF`, group `0xE86B1EEF`, instance `0x286B1F03`) lists the compressed entries.
It is `size / 16` records of 16 bytes:

| Offset | Field |
|---|---|
| 0 | type ID |
| 4 | group ID |
| 8 | instance ID |
| 12 | uncompressed size |

- Always 16 bytes per record (there is no 20-byte form; the index version does not change it).
- Duplicate keys in the DIR: the first wins.
- An entry is decompressed **if and only if** its key is in the DIR. Data that looks like QFS but is not listed is
  returned as stored (one city save contains such a record, engine/formats README).
- The size the resource manager reports for a listed entry is the DIR's uncompressed size.

### 2.6 Reading an entry *[compat]*

- Unlisted entry: the stored bytes.
- Listed entry: the stored bytes must pass the QFS check (§2.7) and decompress; **otherwise the read fails** (the
  resource is not loaded). There is no fallback to the stored bytes.

### 2.7 QFS / RefPack *[compat]*

Stored layout of a compressed entry:

```
uint32  compressed length (little-endian), the 4-byte DBPF prefix
byte    flags    0x10 or 0x11
byte    0xFB
[3 bytes]        present only when flags = 0x11 (skipped, unused)
uint24  uncompressed size, BIG-endian
...     control codes
```

**Acceptance check** (before decoding): the stored data is at least 7 bytes; the signature (bytes 4–5) is exactly
`10 FB` or `11 FB` (so no `0x80` four-byte-size form, no other flag bits); the prefix value is at most
`0x01000000` (16 MiB); the uncompressed size is also limited to 16 MiB, which a 24-bit size always satisfies.

**Control codes** (standard RefPack; `c0..c3` are the code bytes):

| First byte | Bytes | Literal bytes copied first | Copy length | Copy distance |
|---|---|---|---|---|
| `0x00–0x7F` | 2 | `c0 & 3` | `((c0 >> 2) & 7) + 3` | `((c0 & 0x60) << 3) + c1 + 1` |
| `0x80–0xBF` | 3 | `c1 >> 6` | `(c0 & 0x3F) + 4` | `((c1 & 0x3F) << 8) + c2 + 1` |
| `0xC0–0xDF` | 4 | `c0 & 3` | `((c0 & 0x0C) << 6) + c3 + 5` | `((c0 & 0x10) << 12) + (c1 << 8) + c2 + 1` |
| `0xE0–0xFB` | 1 | `((c0 & 0x1F) << 2) + 4` | — | — |
| `0xFC–0xFF` | 1 | `c0 & 3`, then stop | — | — |

The copy is byte by byte from `distance` bytes back, so overlapping copies repeat data. The decoder has **no bounds
checks** on input or output *[quirk]*: malformed data can read or write outside the buffers. *[choice]* FreeCity4
fails the read when a code would read past the input, copy from before the output start, or write past the
uncompressed size.

---

## 3. Resource manager: combining packages *[compat]*

The persistent resource manager (`cIGZPersistResourceManager`, framework service `0x056B906E`, interface
`0x656B8EFC`; slot table foundations §17.6) keeps an ordered list of **segments** (a single package, or a
multi-package segment `cGZPersistDBSegmentMultiPackedFiles`, class `0x090F8619`, holding many packages). It can
add a segment at the **front** or at the **back**.

**Lookup.** A key is looked up in the segments from the front: the **first segment that has the key wins**. In a
multi-package segment, a key present in several of its packages resolves to the package **listed last** (its key
map is filled in listing order with overwrite). Inside one package the first index entry wins (§2.3).

**Cache** (EX37). `GetResource` asks the registered resource caches first, and a resource object already cached is
returned as is. The manager's constructor (`0x009744B2`) creates **one default cache** (vtable `0x00AD9428`) and
registers it in its cache list (`+0x88`, `0x00974544`–`0x00974574`); no other registration was found. The default
cache:

- accepts every key: it has no key filter unless one is set, and none is at construction (`0x0097BFAC`);
- keeps a counted reference to every resource that loads successfully, keyed by type, group and instance (insert
  `0x0097C595`; a key already present keeps its first object);
- returns that object, with a new reference, to every later request for the key (`0x0097C36E`), before any segment
  is consulted;
- is not touched when a segment is registered or removed (slots 27, 28, 29). So a key read once keeps its first
  record even after a later segment that has the key is added in front.

An entry leaves the cache only by:

- removal of that key (manager slot 9, `0x0097341B`, asks every cache that has it);
- a full clear (cache slot 18, `0x0097C452`);
- a **purge**, which drops the entries that only the cache still references (cache slot 20, `0x0097C48F`). A purge
  runs on demand (manager slot 43 → cache slot 17, `0x0097BF63`, which repeats it up to 8 times while it drops
  something). It also runs automatically on every 129th call of cache slot 14 (`0x0097BF43`), but only when the
  auto-purge flag (manager slot 42 → cache slot 16) is on.

*[unsettled: the callers of manager slots 9, 42 and 43 and of cache slot 14 were not traced. An in-game probe that
reads the default cache's entry count before and after a month would settle whether purges happen in play.]*
A purged record is read again at its next request, from the segment that wins then.

Worked example: package P1 holds exemplar K = A. K is looked up, giving A, which the cache keeps. Package P2 with
K = B is then registered at the front. The next lookup of K still gives A. Only if A was purged in between (no one
but the cache held it) does it give B.

FreeCity4's store has no cache, so in this example it gives B. The two agree whenever every package is registered
before the first lookup. In the shipped game every data package is registered in one pass at startup (§3.1),
before a city loads, and the city's save goes to the back, so it never displaces a key that is already found
*[inferred: a lookup during the startup pass itself was not ruled out]*. *[choice]* No cache is kept.
A replacement that registers packages after lookups have started must keep the first record of every key looked up
to match the original.

**Factories.** The object type is chosen by the key's **type ID** (one factory per type). The factory's read
checks the record type again (§4.1, §6).

**When records are read** (EX31). Registering a segment (slot 27 at the front, `0x0097437B`; slot 28 at the back,
`0x009743C1`; both first drop an earlier registration of the same segment, slot 29 `0x009742B7`) only adds it to
the list: it reads no record, runs no factory and leaves the cache alone. A factory runs **lazily**, inside
`GetResource` (`0x009732EB`), when a key is requested and no registered resource cache returns it. The loader
(slot 46, `0x00972EAE`) then picks the factory by the key's type and reads the record from the first segment that
has the key **at that moment**. A successful result is offered to the registered caches in turn and kept by the
first that accepts its key (slot 44, `0x009733C6`). A failure is not kept, so the next request reads again
(§4.4). Consequences:

- A factory registered late applies to every later request for a key of its type that is not cached, including
  keys in segments registered before it.
- A key that no one has requested yet is read from whichever segment wins when it is first requested, even if
  that segment was added after the key's first segment.
- A record that failed, for example because its parent cohort was in no segment yet, loads on a later request
  once a segment with the parent has been added.

*[choice]* Reading every record when its package is loaded (eagerly) gives the same results if all three hold:
(1) all packages are loaded before the first lookup, or a later package's record replaces an earlier read of the
same key that has not been looked up yet; (2) a record that failed is read again when a later package is loaded,
or at its next lookup; (3) a factory registered later is also applied to the records of packages loaded before
it. Reading lazily at the first lookup, as the original does, meets them without extra work.

**A factory added late** (EX38). The original never refuses one: the factory is found by type at each uncached
request (slot 46), so it serves every key of its type requested from then on, in segments registered before it as
well. Worked example: P1 holds a record of type T and is registered; nothing has requested it; then a factory for T
is added. The next request for the record builds it with the new factory. FreeCity4's store reads eagerly and
refuses a factory added after the first package (`add_factory` returns false). That breaks condition (3) above,
so it gives the same results only while every factory is added before the first package is loaded. *[choice]*
Refusing is acceptable on that condition, which holds when FreeCity4 registers all its factories at startup.
A refused factory must be logged as an error, never ignored silently. A store that must accept late factories
applies each one to the records of its type in the packages already loaded, as condition (3) says.

### 3.1 Load order at startup

At startup the application registers, **each at the front** (so every later registration has higher priority):

| Step | What | Segment kind |
|---|---|---|
| 1 | every `*.dat` file in the install folder, in directory-listing order | one packed-file segment per file |
| 2 | every `*.dat` file in the current language's folder (`<install>\<LanguageDir>\`, §6.2) | one segment per file |
| 3 | `<install>\Sku_Data\`, recursively | one multi-package segment |
| 4 | `<install>\Plugins\`, recursively (created if missing): (a) every file whose extension is **not** `.dat` and that opens as a DBPF package, each as its own segment, in listing order; then (b) all `.dat` files in one multi-package segment | (a) one per file, (b) one multi |
| 5 | the user's `Plugins` folder (`Documents\SimCity 4\Plugins`), the same way as step 4 | as step 4 |

Resulting priority, highest first: user Plugins `.dat` files, user Plugins non-`.dat` files, install Plugins
`.dat`, install Plugins non-`.dat`, Sku_Data, language files, root `.dat` files.

- Within one step, files later in the listing win (registered later = further front; in the multi segment, last
  listed wins).
- Listing order: a directory's **files first, then its subdirectories**, recursively (pre-order), each directory
  in the order the OS returns entries *[compat-limit]*: on NTFS this is case-insensitive name order, so
  `Plugins\z.dat` beats `Plugins\a.dat`, and `Plugins\sub\x.dat` beats `Plugins\y.dat`.
- Steps 3, 4b and 5b use the same multi-package segment (class `0x090F8619`, `cGZPersistDBSegmentMultiPackedFiles`).
  The application only sets its folder (`SetupResources` `0x004578E6`–`0x00457946` for Sku_Data); the segment's
  own file filter, set in its constructor (`0x0097EB52`), is `*.dat`. So Sku_Data takes **only `*.dat` files**,
  recursively, exactly like 4b. The filter goes to the OS directory search (`FindFirstFileA` through
  `RZFileFindFirst`), so it matches case-insensitively and with the OS wildcard rules *[compat-limit]* (on a
  volume with 8.3 short names, `*.dat` also matches e.g. `x.data`).
- In any multi-package segment a listed file that fails to open as a package (no `DBPF` magic and no embedded
  header, §2.1–2.2) is released and **skipped silently** (no message); the loop goes on with the next file
  (`0x0097EA75`–`0x0097EACE`).
- In the same Plugins tree a `.dat` file beats any non-`.dat` package (e.g. `.SC4Lot`, `.SC4Desc`), whatever their
  names *[compat]*.
- The city being played is added at the **back** (lowest priority): a resource stored in the save (for example a
  parent exemplar, foundations §10.3) is used only if no data file or plugin has that key.
- Shipped data: the only duplicate between root, Sku and English files is LTEXT
  (`0x2026960B`, `0x6A231EAA`, `0x0A6DF4DF`), identical in `Plugin_002_TheLivingMall.dat` and
  `English\SimCityLocale.DAT` (Sku wins). Non-English locale files override a few root entries, e.g.
  (`0xA2E3D533`, `0x6A231EAA`, `0x6A9362EF`) from `SimCity_1.dat`, and (`0x0A8B0E70`, 1, 2) from `Intro.dat`.
- *[choice]* FreeCity4 standalone mode sorts each directory's entries by case-insensitive name (ordinal on the
  upper-cased UTF-16 names) to be deterministic on every OS, which matches NTFS in all common cases.

---

## 4. Exemplars and cohorts

Two resource types: **exemplar** (type `0x6534284A`, `cSCResExemplar`, factory `cSCResExemplarFactory`) and
**cohort** (type `0x05342861`, `cSCResExemplarCohort`). A cohort is the same structure used only as a parent.

| Interface | IID | Exemplar | Cohort |
|---|---|---|---|
| main (`cISCResExemplar` / cohort interface) | `0xA52160F5` / `0x452160FB` | `0xA52160F5` only | `0x452160FB` only |
| `cISCPropertyHolder` | `0x25216283` | yes | yes |
| `cIGZPersistResource` | `0x456B8F1D` | yes | yes |

An exemplar does **not** answer the cohort IID, and a cohort does not answer `0xA52160F5` *[compat]*.

### 4.1 Record header *[compat]*

The factory first checks the record's type ID: an exemplar must be stored under `0x6534284A`, a cohort under
`0x05342861`; otherwise the read fails. Then it reads 8 signature bytes and compares **only the first four**:

| Resource | Binary | Text |
|---|---|---|
| exemplar | `EQZB` | `EQZT` |
| cohort | `CQZB` | `CQZT` |

Bytes 5–8 (`1###` in every shipped record) are not checked. A cohort record with an `EQZ` signature, or the
reverse, fails.

### 4.2 Binary form *[compat]*

```
char[8]  signature           "EQZB1###" / "CQZB1###"
uint32   parent type          parent cohort key (foundations §2.5 order: type, group, instance)
uint32   parent group
uint32   parent instance
uint32   count
count × property
```

The original parses each property as **a kind byte followed by a variant in foundations §9.2 disk form without
the marker**:

```
uint32  property ID
uint8   kind        0 = value, 1 = resource key; anything else: the read fails
kind 0: uint32  disk tag (foundations §9.2)  then the value as §9.2 (scalar element / array count + elements /
                tag 0x800C: a §2.3 string)
kind 1: uint32 type, uint32 group, uint32 instance  (a resource-key property, §4.6)
```

This is byte-compatible with the usual description ("`uint16` value type `0x0100`…`0x0C00`, `uint16` key type
`0x00`/`0x80`, one unused byte"): the low byte of the value type is the kind (0), and the next four bytes (high
byte of the value type, the two key-type bytes, the "unused" byte) form the little-endian disk tag. For example
`00 03 80 00 00` is kind 0, tag `0x00008003` (Uint32 array). Consequences *[compat]*:

- The "unused" byte and the key type's high byte must be **0**, otherwise the tag is invalid and the read fails.
- Tags accepted: scalars `0x00`–`0x0C`, arrays `0x8000`–`0x800C`, `0x8010`, `0x8011` (foundations §9.2 reader
  set). The value types `0x0400` Uint64, `0x0500` Sint8, `0x0600` Sint16, `0x0A00` Float64 are valid.
- **Tag 0 (Empty)** has no value bytes: the next property starts right after the tag. The property is added like
  any other and holds an **Empty** variant (memory type 0, no data, `GetCount()` 0), so `HasProperty` and
  `IsPropertyLocal` are true for it. What the getters return for it is in §5.1 ("Empty and object values"). For
  example count 2, `10 00 00 00 | 00 | 00 00 00 00` then `11 00 00 00 | 00 | 03 00 00 00 | 07 00 00 00`: `0x10` is
  Empty and `0x11` is Uint32 7. Tag `0x8000` is a **void array** (memory type `0x8000`: a `uint32` count, then
  that many raw bytes). No shipped record uses tag 0, tag `0x8000`, kind 1 or the char tags `0x10`, `0x11`,
  `0x8010`, `0x8011` (sweep of the 9,345 binary exemplar and cohort records in the install, 2026-10-14).
- Key type `0x80` gives an **array** even for one element (count 1); key type `0x00` gives a scalar. Arrays may
  have count 0.
- A string is tag `0x800C`: the count is the §2.3 string header (top byte `0x10` = UTF-16, quirks of §2.3
  included). Tag `0x000C` (value type `0x0C00`, key type 0) is **one RZChar byte**, not a string.
- Bool (`0x0B`): any non-zero byte is true.
- **Parent**: a key of three zeros means no parent. Otherwise the factory loads the parent through the resource
  manager, requesting the cohort IID `0x452160FB`; **if that fails, the whole read fails** and the exemplar does
  not exist for the game. Because only cohorts answer that IID, the parent must be a cohort record
  (type `0x05342861`) *[compat]*.
- Properties are added in record order; a later property with the same ID **replaces** the earlier one (§4.5).
- Bytes after the last property are ignored. A record that ends early fails (stream error).
- The factory has a set of property IDs to skip on read; no code in either build adds to it, so it is always
  empty *[inferred]*. Replacements read every property.

### 4.3 Text form *[compat]*

After the 8 signature bytes the reader **skips 2 more bytes** (the line break `\r\n` in every shipped record; not
checked) and splits the rest into lines at `\n`, removing one trailing `\r`. Empty lines are skipped. Then:

1. The first line must be `name=value` with name `ParentCohort` (case-insensitive, both sides trimmed). The value
   must begin with `Key:{` and end with `}` (case-sensitive). It holds three numbers separated by commas in the
   order **group, instance, type**. All three zero = no parent; otherwise the parent is loaded as in §4.2 and a
   failure fails the read. The fields are **not counted** (`0x0067DFD0`): the reader drops the text up to the
   `{` and reads the group with rule N, then drops the text up to the next `,` and reads the instance, then the
   same for the type. Fields after the third are ignored. When there is no further `,` the rest of the text is
   dropped and that field and any later one read **0**. Nothing inside the braces is trimmed; rule N skips no
   white space before choosing the base, so a **space after `{` or `,` turns a `0x` field into 0** (a decimal
   field survives, because `strtoul` skips it). Worked examples: `Key:{0x1,0x2,0x3,0x4}` gives group 1,
   instance 2, type 3; `Key:{0x1, 0x2, 0x3}` gives group 1, instance 0, type 0; `Key:{1, 2, 3}` gives 1, 2,
   3; `Key:{0x1,0x2}` gives type 0, so the parent key is not all zero and its load fails; `Key:{}` gives three
   zeros, no parent.
2. The next line must be `PropCount=<number>` (name case-insensitive). As on every `name=value` line, the name
   and the value are trimmed of spaces and tabs (`0x0067E0B0`); the number is rule N (`0x0067E250`).
3. Exactly `PropCount` property lines follow. If the record runs out of lines first, the read fails; lines after
   them are never read.

```
0x<ID>:{"<name>"}=<Type>:<reps>:{<values>}
```

- **ID**: the text left of the first `=` (trimmed) is converted as a number (rule N below); conversion stops at the
  first character that is not a digit, so `0x00000010:{"Exemplar Type"}` gives `0x10`. The name is ignored. A
  name containing `=` breaks the line *[quirk]*.
- **Type**: one of `Bool`, `Uint8`, `Sint8`, `Uint16`, `Sint16`, `Uint32`, `Sint32`, `Uint64`, `Sint64`,
  `Float32`, `String`, **case-sensitive**. Anything else fails the read. (No `Float64` in text.) It is the text
  before the first `:`, compared **exactly** (`0x0067E780`): the value was trimmed at its start, but nothing is
  trimmed around the `:`, so `Uint32 :0:{1}` fails the read.
- **reps**: number (rule N). `0` = a scalar with one value; `n ≥ 1` = an array of `n` values (even for `n = 1`).
  For `String` the reps field is ignored. It is read from the text right after the first `:`, **not trimmed**;
  rule N stops at the next `:`. So ` 3` gives 3, but ` 0x3` gives 0, a scalar. The `{` must follow the second `:`
  directly: `Uint32:0: {1}` fails the read.
- **values**: must start with `{`. Values are separated by commas. In an **array** each value is trimmed of
  leading spaces and tabs; a **scalar's** value is not, which matters only for rule N: `{ 0x10}` gives 0 as a
  Uint32 scalar but 16 as a one-element array. Only `String` is cut at `}`. A number stops at the first character
  that cannot continue it, `,` and `}` included, and any text after it is ignored (the last Float32 array value is
  split at `}`, with the same result). Per type:
  - integer types up to 32 bits: rule N, then truncated to the type's width (two's complement);
  - `Uint64` / `Sint64`: always **hexadecimal** (`RZAtoUint64HexA` `0x0090FD90`): leading white space skipped, an
    optional `0x` or `0X`, then at most 16 hex digits. There is no sign, so a negative Sint64 must be written in
    two's complement: `10` gives 16 and `-1` gives 0;
  - `Float32`: decimal with `.` as separator, locale-independent; `80` gives 80.0. The parser (`RZStrtodEnglishA`
    `0x00910426`) is the game's own, not the C runtime's. It skips white space and takes one optional sign. Then
    it builds the value **digit by digit in x87 arithmetic**: each integer digit gives value × 10 + digit; each
    fraction digit multiplies a running scale by 0.1 and adds digit × scale. Every step is rounded to the caller's
    precision, **24 bits on the main thread** (foundations §18.2), and the result is stored as Float32. An `e` or
    `E` exponent is read as a 32-bit integer; the power 10ⁿ is computed **in 32-bit integer arithmetic, wrapping**
    (`0x009103DA`), then multiplied in. A negative exponent is **not** a division: the parser first rounds the
    reciprocal 1/10ⁿ to the caller's precision, then multiplies it in, so the result is rounded twice
    (`0x00910507`–`0x0091050B`, EX35). There is no hex, `inf` or `nan`:
    parsing stops at the first character that does not fit, so `inf` gives 0 and `1.#INF` gives 1. The same parser
    serves the §5.1 text round trip, where a Float64 target gets the same 24-bit value. Worked examples (24-bit):
    - `0.05` gives `0x3D4CCCCC`, where a correctly rounded parse gives `0x3D4CCCCD`;
    - `0.001` gives `0x3A83126E` and `3.14159` gives `0x40490FCE`;
    - `0.1`, `0.3`, `123.456` and `2.5e3` come out correctly rounded;
    - `1e10` gives 1,410,065,408 (10¹⁰ mod 2³²), `1e12` gives −727,379,968, and `1e-10` gives about
      7.09 × 10⁻¹⁰;
    - negative exponents round twice: `9e-1` gives `0x3F666667`, because 1/10 rounds to `0x3DCCCCCD` and
      9 × 0.100000001490116… = 0.900000013411… rounds up, where one division gives `0x3F666666`. Likewise `5e-2`
      gives `0x3D4CCCCC` (one division: `0x3D4CCCCD`) and `2.5e-3` gives `0x3B23D70B` (one division:
      `0x3B23D70A`). `1e-1`, `3e-1`, `7e-1`, `1e-3` and `1.5e-2` come out the same either way;
    - `1e32` and any larger exponent give 0 (10ⁿ mod 2³² is 0 from n = 32), `1e-32` gives +∞ and `0e-32` gives
      NaN.

    Details of the arithmetic (`0x00910426`):
    - **Constants** (EX21). 10.0 (`0x00AA2028`) and 0.1 (`0x00A9EFE0`, bits `0x3FB999999999999A`) are doubles,
      used directly as double operands. So each scale step rounds the exact product scale × 0.1 (double) once. With
      `0.1f` instead, `0.05` would give `0x3D4CCCCE`.
    - **Roundings per digit** (EX22). An integer digit rounds twice: value × 10 (`fmul`), then + digit (`faddp`),
      each to the caller's precision (`0x00910476`–`0x0091048A`). A fraction digit rounds three times: scale × 0.1,
      digit × scale, then + value (`0x009104A7`–`0x009104BB`). Storing each partial value as a double between digits
      loses nothing more. Worked example (24-bit): `33573415` gives 33,573,412.0. The steps are 3,357,341 × 10 =
      33,573,410, which rounds (a tie, to even) to 33,573,408; then + 5 = 33,573,413, which rounds to 33,573,412. One
      rounding of the exact 33,573,415 would give 33,573,416.0.
    - **Exponent** (EX23). After the mantissa, whatever it consumed, an `e` or `E` starts the exponent
      (`0x009104C2`). It takes one optional `+` or `-` (`0x009104D4`–`0x009104DE`), then decimal digits; with no
      digits the exponent is 0. So the exponent is read even when no mantissa digit came first, and the value is
      then 0 × 10ⁿ:
      - `e5` and `.e5` give 0, `-e5` gives −0.0, and `2.5e+3` gives 2500;
      - `e-32` gives NaN, as `0e-32` does: 10³² wraps to 0, 1/0 is +∞, and 0 × ∞ is the default NaN, stored as
        `0xFFC00000`. A single division 0/0 would give the same NaN (EX35);
      - a leading `-` is applied last, by `fchs` on the result (`0x00910526`), and it flips a NaN's sign too
        (EX36): `-e-32` and `-0e-32` give `0x7FC00000`, and `-1e-32` gives −∞;
      - `1e` gives 1.

      In the §5.1 round trip, an `e` with no digit before it stops the classifier first, so `e5` there is the
      unsigned number 0 and never reaches this parser.

    Shipped text Float32 values are small integers, which are exact. *[inferred: text exemplars are read on the
    main thread at 24-bit precision; at 53-bit precision `0.05` would give `0x3D4CCCCD`]*
  - `Bool`: leading spaces **and tabs** trimmed, lower-cased; true exactly when it begins with `true` (`1` gives
    false). The trim is `cRZString::LTrim` (`0x0090E869`), whose set is space and tab (`0x00ACE73C`), the same as
    for array values (EX26). So `{\ttrue}` (a tab) is true;
  - `String`: everything up to the first `}`, trimmed, then one leading `"` and one trailing `"` removed. A string
    cannot contain `}` *[quirk]*. The trim removes spaces and tabs at both ends (`cRZString::Trim` `0x0090E8A6`,
    the set at `0x00ACE73C`; EX27). It runs before the quotes are removed, so white space inside the quotes stays:
    `{ " a " }` gives ` a ` (with its spaces).
- The reader does not check that the number of values matches `reps`, and there is **no cap** on `reps`
  (`0x0067E780`). It reads `reps` values in turn, dropping the text up to each next `,`; when no `,` is left the
  rest of the text is dropped, so **missing values read as 0** (false for Bool, 0.0 for Float32) and values beyond
  `reps` are ignored. Worked examples: `Uint32:4:{1,2}` gives `{1, 2, 0, 0}`; `Uint32:1:{1,2}` gives `{1}`.
  The values go into a buffer of `reps` × element size bytes, computed in 32 bits, and the allocation's result
  is not checked, so a **huge `reps` crashes the original**. From `0x40000000` Uint32 values the size wraps to a
  small buffer, which the loop overruns. A size too large to allocate has the same effect. *[choice]*
  FreeCity4 fills missing values with 0 as the original does, and fails the read when `reps` × element size
  exceeds 16 MiB. Shipped text records use `reps` 13 at most.

**Rule N** (number text): if it begins with `0x` or `0X`, hexadecimal. Otherwise, scan from the start: if a letter
`a`–`f`/`A`–`F` appears before the first character that is not a hex digit, hexadecimal; else decimal. The
conversion is C `strtoul` in that base (a leading `-` negates modulo 2³²). So `16` = 16, `1A` = 26, `0x10` = 16,
`-1` = `0xFFFFFFFF`. The base is chosen on the raw text (`RZConvertToUint32A` `0x0090FE93`): a leading space
stops the scan with base 10, and `strtoul` then skips the space, so ` 16` = 16, ` 1A` = 1 and ` 0x10` = 0. A
value above `0xFFFFFFFF` gives `0xFFFFFFFF`, with or without a leading `-` (`strtoul` saturates): `4294967296` and
`-4294967296` both give `0xFFFFFFFF`.

Shipped text exemplars: two, both in `Plugin_002_TheLivingMall.dat` (`0xCA386E22`/`0x04C00000` and
`0xA8FBD372`/`0x6AC4085B`). The first starts:

```
EQZT1###
ParentCohort=Key:{0xca386e22,0x0a64ed55,0x05342861}
PropCount=0x00000016
0x00000010:{"Exemplar Type"}=Uint32:0:{0x00000002}
0x099afacd:{"Bulldoze Cost"}=Sint64:0:{0x0000000000001356}
0x27812810:{"Occupant Size"}=Float32:3:{80,52,80}
```

Its lot configuration writes `LotConfigPropertyWealthTypes` (`0x88EDC795`) and `LotConfigPropertyPurposeTypes`
(`0x88EDC796`) as `Uint8:0:{}`, which reads as a **scalar** Uint8 0 (rule N on `}` gives 0). That is what the
original stores, and it is not observable as different from an empty array. Both readers (`0x0047A16C`,
`0x0060F710`) use the byte-list getter `0x005FFB40`, which copies `GetCount()` bytes from the raw data
pointer (§5.2), and `GetCount()` is 0 for a scalar. So the lot gets **no** wealth or purpose types.
`Uint8:1:{}` would instead give one type, 0.

**Writing text** (EX32). The exemplar factory's `SerialWrite` (`0x00680030`) writes the text form when the
factory's write-mode byte is non-zero and the binary form when it is 0. The factory's constructor sets it to 1
(`0x0067E373`); slot 12 (`0x0067CB10`) changes it, and a pattern search found no caller *[inferred: callers not
fully traced]*. So whenever the game writes an exemplar (foundations §10.3, "Write side extra"), it is probably
text. Each Float32 value, scalar or array element, is printed with `%f` (six decimals, from the runtime's 17
significant digits rounded half-up, §5.1) and then trimmed (`RemoveUnnecessaryFloatingPointDigits`,
`0x0067C2B0`). The trim removes the zeros at the end of the fraction, then a `.` left at the end, and a text made
only of `0` and `.` becomes `0`. The writer **never fails**, and it does not always reproduce the value, read
back with the 24-bit parser above:

| Float32 | written | read back |
|---|---|---|
| 3.14159 (`0x40490FD0`) | `3.14159` | `0x40490FCE` |
| 0.05 (`0x3D4CCCCD`) | `0.05` | `0x3D4CCCCC` |
| 1.0000001 (`0x3F800001`) | `1` | `0x3F800000` |
| 123456.85 (`0x47F1206D`) | `123456.851563` | `0x47F1206C` |
| 1e-7 (`0x33D6BF95`) | `0` | 0 |
| −1e-7 (`0xB3D6BF95`) | `-0` | zero |
| 0.1 (`0x3DCCCCCD`) | `0.1` | `0x3DCCCCCD` |
| 1e30 (`0x7149F2CA`) | `1000000015047466200000000000000` | `0x7149F2CA` |

NaN and infinities come out as the runtime's `%f` text trimmed the same way, `1.#QNAN`, `1.#INF`, `-1.#IND`
*[inferred from the MSVC runtime]*, which read back as ±1. *[choice]* FreeCity4 need not copy this lossy text:
the reader accepts both forms, so a writer may emit text that reads back exactly, refuse a value it cannot
reproduce, or write the binary form.

### 4.4 Parents and inheritance *[compat]*

Each exemplar or cohort keeps two property lists, both sorted by ascending unsigned property ID:

- **own** properties (those of its record);
- the **effective** list = the parent's effective list, then the own properties applied on top (an own property
  replaces the parent's property with the same ID).

The lookups `HasProperty` (holder slot 3, `0x0067C620`), `GetProperty(id)` (slot 9, `0x0067C670`), the typed getters
of §5 (slots 5–8 call slot 9) and `GetPropertyList` (slot 4, `0x0067C4B0`) use the **effective** list: a binary
search (`0x005FE940`) in a sorted vector of `{id, property}` pairs (property-holder sub-object `+0x18`/`+0x1C`); the
parent chain is **not** walked at lookup time, because the vector already holds every inherited property.
`IsPropertyLocal(id)` (`cISCResExemplar` slot 8, `0x0067C210`) tests the own list only (`+0x08`/`+0x0C`).

**Correction (DG29): `EnumProperties` lists the own properties only.** Exemplars and cohorts do not override the
base `cSCPropertyHolder::EnumProperties` (slots 19 and 20, `0x005FDA80` / `0x005FDA50`), which walks the own list
(`+0x08`). An earlier version of this section listed it with the effective lookups. The **effective enumerator is
`GetPropertyList`**: it creates a new `cIGZUnknownList` (`0x0090DC0F`), adds every property of the effective list in
ascending ID order (list slot 7), and returns it with one reference, which the caller releases; it returns false
(and no list) for a null out-pointer. Occupants differ: an occupant's holder (`cSCExemplarPropertyHolder`, e.g. a
building's `+0x0C`, building-development §4.9.1) answers `GetProperty` from its own list, then from its default
exemplar's effective list, and its `EnumProperties` (`0x005FEE10` / `0x005FEF00`) visits its own properties, then
the exemplar's `GetPropertyList` minus the IDs it overrides.

Worked example: an exemplar E whose record holds only a name and a capacity, with parent cohort C holding
`ExemplarType` = 2 and a family list `0x27812870`. `GetProperty(0x10)` on E finds 2 and E's `GetPropertyList`
contains `0x10` and `0x27812870`, but E's `EnumProperties` visits only the name and the capacity. This is the shipped
case: of the 743 building types in the shipped `.dat` files (the devglue shadow counted 744 in game), 653 inherit
`ExemplarType` 2 from a cohort and 90 carry it themselves, and all 556 building exemplars with a family list inherit
it. A reader that sees own properties only finds 90 building types and no families.

- The effective list is **built when the parent is set and updated on every add**: it is a snapshot. Changing a
  cohort later does not change exemplars already built from it *[compat]* (the game never changes cohorts at run
  time).
- Because a parent is fully loaded (with its own effective list) before its child, the chain is resolved
  recursively at load: grandparent properties appear in the child unless overridden. **There is no depth limit.**
  Shipped chains are at most 3 levels deep (engine/formats README).
- **Cycles**: a cohort that is (directly or indirectly) its own parent makes loading recurse without end
  *[inferred: no guard was found; the resource is not in the cache until its read completes]*. *[choice]*
  FreeCity4 stops at the first repeated key and at depth 32, and treats the record as failing to load. Depth
  counts the nested parent loads below the record requested, so at most 32 ancestors load. The original has no
  limit to match, and it **does not cache failures**: `GetResource` (`0x009732EB`) adds an object to the cache
  only after its load succeeds, so a key that failed is loaded again on the next request. FreeCity4 likewise
  remembers no failure, so a record that failed only on depth loads when it is later requested at a shallower
  depth.

### 4.5 Duplicate property IDs *[compat]*

Inside one record (binary or text) the **last** property with an ID wins (adding replaces in place). A child's
property always wins over any ancestor's. No shipped record has duplicates.

### 4.6 Resource-key properties (kind 1) *[compat]*

A kind-1 property holds a resource: its key (type, group, instance) is loaded through the resource manager
(`cIGZPersistResource`, `0x456B8F1D`) while the exemplar is read, and the property keeps the loaded object. If the
resource cannot be loaded, **the exemplar read fails**. The resource manager (`0x009732EB`) can load a key when:

- a resource cache already holds an object with that key; or
- (`0x00972EAE`) a factory is registered for the key's **type**, a loaded package holds a record with that key,
  and the factory's read of that record succeeds.

A key that is in a package index but has no factory for its type does not load. Neither does a key whose
record fails to read, such as a cohort whose own parent fails. Any type with a factory counts, not only
exemplars and cohorts *[inferred: the registered factory set was not enumerated]*. No shipped record uses
kind 1; the text form cannot express it. *[choice]* FreeCity4 reads kind 1 for compatibility but no simulation
code depends on it.

---

## 5. Property getters and type coercion *[compat]*

Simulators read tuning through three families of calls. Which one a call site uses decides what happens when the
stored type differs from the requested one; §7 records it per property where the owning spec gives it.

### 5.1 Converting getters

`cISCPropertyHolder` slot 8 `GetProperty(id, uint32&)`, slot 7 `GetProperty(id, cIGZString&)` (foundations
§17.2), and every `bool cIGZVariant::GetValX(X&)` (foundations §17.1) except `GetValVoid`. Missing property:
return false, output unchanged. Slot 5 `GetProperty(id, void*, uint32&)` does not convert (§5.2).

If the stored type equals the requested type, the value is returned as is. Otherwise:

| Stored | Requested | Result |
|---|---|---|
| scalar Bool…RZChar (memory types `0x01`–`0x0E`) | Float32 | value → float64 → **rounded to float32** |
| scalar | Float64 | value → float64; unsigned types as unsigned, with the 24-bit rounding below for Uint32 ≥ 2³¹ and for every Uint64; signed types exact (Sint64 rounded to 53 bits) |
| scalar | integer type, Bool, char types | value → int64 (unsigned types zero-extended, signed sign-extended, Char and RZChar **sign-extended**, RZUnicodeChar zero-extended, **floats truncated toward zero, not saturated**: see below), then the low bits of the target width; Bool = (int64 ≠ 0) |
| array (any element type), string, or a requested non-scalar type | any | **text round trip** (below) |

**Unsigned 64-bit and large unsigned 32-bit values.** The to-double converter (`0x00915B85`) turns a Uint64 into a
double as (its low 63 bits, loaded as a signed int64) + 2⁶³ when the top bit is set, + 0 otherwise, and a Uint32
with the top bit set as (its value loaded as signed) + 2³². The add runs at the caller's x87 precision, 24-bit on
the main thread (foundations §18.2), so the result keeps **24 significant bits even for a Float64 target**. For a
Uint64 the add is done whether or not the top bit is set. Signed values and Uint32 below 2³¹ are loaded exactly.
Worked example, `GetValFloat64` on the main thread: Uint64 16,777,217 (2²⁴ + 1) gives **16,777,216.0**; Sint64
16,777,217 gives 16,777,217.0; Uint32 16,777,217 gives 16,777,217.0; Uint32 `0x80000001` gives
**2,147,483,648.0**. Text parsed as an unsigned number for a Float32 or Float64 target (round trip below) takes the
same path. FreeCity4 reproduces it by rounding these values to 24 significant bits (round to nearest even).

**Floats to integers.** A Float32 or Float64 is truncated toward zero to an int64 by the C runtime's `_ftol`
(`0x009EEF04`, foundations §18.1), and the target keeps the low bits: out-of-range values **wrap**, they do not
saturate. NaN, ±∞ and magnitudes ≥ 2⁶³ give the int64 `0x8000000000000000`, so **0** for targets of 32 bits or
fewer, `0x8000000000000000` for Uint64 and Sint64, and **true** for Bool. Worked examples: Float32 300.7 read as
Uint8 gives 300 mod 256 = **44**; Float32 3,000,000,000 read as Sint32 gives **−1,294,967,296**; Float32 −1.5 read
as Uint32 gives `0xFFFFFFFF`; NaN read as Uint32 gives **0** and read as Sint64 gives `0x8000000000000000`; Float32
0.5 read as Bool gives **false** (it truncates to 0), NaN read as Bool gives **true**. A number parsed from text as
floating point (round trip below) is truncated the same way for integer targets.

**Char types.** Both converters (`0x00915AA5` to int64, `0x00915B85` to double) widen **Char (`0x0C`) and RZChar
(`0x0E`) as signed bytes** and RZUnicodeChar (`0x0D`) as an unsigned 16-bit value. Worked example: RZChar `0xE9`
read as Uint32 gives `0xFFFFFFE9`, as Sint16 −23, as Float32 −23.0, and as Uint8 `0xE9` (only the low byte is
kept); RZUnicodeChar `0xFFE9` read as Uint32 gives 65,513.

**Empty and object values.** An Empty value (memory type 0, §4.2) and an object value (IGZUnknown `0x10`) always
take the text round trip: `ConvertTypes` (`0x00917350`) sends memory type 0 and every type above `0x0E` there.
Empty becomes the empty text, which parses as the unsigned number 0, so every converting getter **succeeds**:
integer and char targets 0, Float32 and Float64 0.0, Bool false; `GetProperty(id, uint32&)` returns true and 0. An
object becomes the object's own text if it answers `cIGZString` (`0x089B7DC8`), otherwise its address printed as
`0x%08x`; that text parses as hexadecimal, so `GetProperty(id, uint32&)` returns true and the **object's address**
(0 for a null object). `GetValString`, and with it `GetProperty(id, cIGZString&)` (`0x009172A1`), returns true only
for a string (RZChar array) or a string object. For every other type, Empty included, it **writes the round-trip
text into the output and returns false**: a Uint32 7 gives the text `7` and false. *[choice]* FreeCity4 does the
same for Empty, and treats an object value as a missing property (false, output unchanged), because the original's
result is a heap address. Only kind-1 properties (§4.6) hold objects, and no shipped record has one.

**Text round trip** (arrays and strings): the source is turned into text, then the text is parsed into the target.

- Text of a numeric or Bool array is its **first element** only (`%u` / `%d` / `%llu` / `%lld` for integers, the
  float formatter below for floats, `0`/`1` for Bool); an array of count 0 gives `0`. A **string** (RZChar array
  `0x800E`) and a **Char array** (`0x800C`) give **all** their characters. An **RZUnicodeChar array** (`0x800D`)
  gives all its characters, each UTF-16 unit below `0x80` as that byte and every other unit as `?` (`0x0090E901`). A
  **void array** (`0x8000`) and a VoidPtr array (`0x800F`) give the **empty text**, which parses as 0. Worked
  examples (`ConvertToString` `0x00916EAA`), read with `GetProperty(id, uint32&)`: void array `31 32` gives 0 (not
  0x31); Char array `31 32` ("12") gives 12; RZUnicodeChar array `0x00E9 0x0035` gives the text `?5`, which parses
  as 0.
- **Floats as text** (`RZFtoaEnglishA` `0x0091052A`, called with a 64-byte buffer and 24 digits; a Float32 is
  widened to double first) are **fixed-point, never with an exponent**: `-` for a negative value, the integer digits
  in full (`0` when |x| < 1), then, if a non-zero fraction digit exists, `.`, the fraction's leading zeros and the
  digits up to the last non-zero one. The digits are those of the C runtime's `_fcvt(x, 64)`, which asks the
  runtime's decimal conversion (`$I10_OUTPUT` `0x00A04942`, called from `_fltout2` at `0x00A027A0`) for **17
  significant digits, rounded half-up** (a tie rounds away from zero), and pads them with zeros (`_fptostr`
  `0x00A0264A`). So a value prints its first 17 significant digits and never more; an integer part longer than 17
  digits ends in zeros. The printer's own limit of 24 digits after the fraction's leading zeros never takes effect,
  because at most 17 of them can be non-zero (EX29). Examples: 2.75 gives `2.75`; Float32 0.1 (exactly
  0.100000001490116119384765625) gives `0.10000000149011612`; Float32 0.001 gives `0.0010000000474974513`; Float32
  1e30 (exactly 1000000015047466219876688855040) gives `1000000015047466200000000000000`; Float32 2⁻²⁵ (exactly
  0.0000000298023223876953125, a tie at the 18th digit) gives `0.000000029802322387695313`. NaN and infinities come
  out as the C runtime prints them: `1.#INF`, `-1.#INF`, `1.#QNAN`, `-1.#IND` (the default NaN) *[inferred from the
  MSVC runtime; its strings `1#INF`, `1#QNAN`, `1#SNAN`, `1#IND` are in the exe]*, which parse back as ±1. **The
  round trip is not exact**: the parser (`RZStrtodEnglishA`, §4.3 Float32) adds the digits one by one at the main
  thread's 24-bit precision, so Float32 3.14159 (`0x40490FD0`), printed `3.1415901184082031`, reads back as
  `0x40490FCE`; Float32 0.05 (`0x3D4CCCCD`), printed `0.05000000074505806`, as `0x3D4CCCCC`; and Float32 0.001
  (`0x3A83126F`) as `0x3A83126E`. 2.75, 1.5 and other short binary fractions survive, and so do Float32 0.1 and
  1e30.

  **The 63-character cut** (EX29) applies to every float, a Float32 included. A text longer than 63 characters is
  cut to its first 63 and the printer returns false; both callers (`0x00916F77`, `0x0091711E`) ignore the result
  and keep the cut text. When the cut falls among the significant fraction digits, that is, a digit after the
  fraction's leading zeros is still to be written when 64 characters are already there, the printer also
  **strips trailing zeros** from the 63 characters it keeps (`0x00910672`–`0x00910697`). A cut anywhere else, and a
  text of exactly 64 characters, is not stripped (`0x00910647`, `0x00910662`). Worked example: Float64
  1.0000000000000002e-47 (digits `10000000000000002` after 46 leading zeros) needs 65 characters; the cut keeps
  `0.`, 46 zeros and `100000000000000`, and the strip leaves `0.`, 46 zeros and `1`, 49 characters.
  The strip removes **only `0` characters** (EX40): it ends the text at the 64th character, then walks back from the
  63rd and clears each `0` until it meets another character, never looking at the first character
  (`0x00910686`–`0x00910697`). When the first significant digit is the 64th or 65th character and more digits
  follow, the 63 kept characters are `0.` (or `-0.`) and zeros, so the strip leaves **`0.`** or **`-0.`**, the
  point kept. Worked examples: 1.5e-62 (`0.`, 61 zeros, then `15`) prints `0.`; −1.5e-61 (`-0.`, 60 zeros, then
  `15`) prints `-0.`. A lone last digit at the 64th character is not this case: the text is then exactly 64
  characters and is cut without a strip, as below.
  A Float32 reaches the cut only as a **negative subnormal below 1e-44 in magnitude**, the seven values `0x80000001` to
  `0x80000007`: `-0.`, 44 leading zeros and 17 digits make 64 characters, so the 17th digit is lost. For example
  `0x80000001` (−1.40129846432481707…e-45, 17 digits `14012984643248171`) gives `-0.`, 44 zeros and `1401298464324817`,
  63 characters. The positive values fit exactly (63 characters), and from `0x80000008` (−1.12e-44) on there is one
  leading zero fewer. A Float64 reaches the cut more often: for example −1e62 (`-` and 63 digits), 1e63 (64 digits), and
  tiny values whose fraction runs past 61 places.

  **Below 10⁻⁴⁸ the digits are rounded a second time** (EX34). `_fcvt(x, 64)` (`0x009F662C`) keeps digits only up
  to the 64th decimal place. With the 17-digit value written as 0.d₁d₂… × 10^decpt, `_fpcvt` (`0x009F65CF`) asks
  `_fptostr` (`0x00A0264A`) for 64 + decpt digits. When that is fewer than 17, which happens when decpt ≤ −48
  (the 17-digit value is below 10⁻⁴⁸), `_fptostr` keeps that many digits and rounds **half-up** on the next one, a
  carry running through 9s (`0x00A0268E`–`0x00A0269E`). At decpt = −64 it keeps no digit and rounds on the first;
  below that it keeps nothing and does not round. From 10⁻⁴⁸ up to 10⁻⁴⁷ all 17 digits still fit. The second
  rounding can change the visible text, because a positive value shows places 1 to 61 before the cut:

  - 2e-49 (the double 1.99999999999999987…e-49, digits `19999999999999999` from place 49): 16 digits are kept and
    the 17th, a 9, rounds them up, so it prints `0.`, 48 zeros and `2` (51 characters). With one rounding it
    would print `0.`, 48 zeros and `1999999999999` (63 characters, cut).
  - 1e-48 (the double 9.9999999999999997…e-49) rounds up the same way to `0.`, 47 zeros and `1` (50 characters).
  - 9.5e-65 (digits `95000000000000004` from place 65) rounds up to a 1 at the 64th place. The text would be
    `0.`, 63 zeros and `1`; the cut falls among the leading zeros, so it prints `0.` and 61 zeros (63 characters,
    nothing stripped).
  - 4e-65, and 5e-65 (the double 4.99999999999999998…e-65), round down to no digit and print `0`, as does
    every value below that.

  **A tiny negative value keeps its sign.** `_fcvt` reports the sign of any non-zero value (`0x009F6616`), even
  when no digit is left. So −4e-65 prints `-0` and −9.5e-65 prints `-0.` and 60 zeros (63 characters). Only a
  true −0.0 prints `0` (EX24, below).

  **Negative zero** (EX24) prints `0`, for a Float32 and a Float64 alike. The runtime's digit conversion
  (`0x00A04942`) takes a zero's sign from the sign bit but then resets it to "positive" on its zero path
  (`0x00A04BC1`). So `_fcvt` reports no sign, and the printer writes no `-`, then `0` (|x| < 1) and no fraction.

  **Signalling NaN** (EX28) prints as a quiet NaN, `1.#QNAN` or `-1.#QNAN`, for a Float32 **and a Float64**. Both
  call sites load the value onto the x87 stack (`fld dword` at `0x00916F62`/`0x009170FD`, `fld qword` at
  `0x00916F67`/`0x00917117`) and store it as a double, and the printer loads it again (`0x00910542`). Loading a
  Float32 or Float64 sNaN raises the invalid-operation exception, which is masked (foundations §18.1). The value is
  then quieted, keeping its sign and payload. So the runtime's `1#SNAN` case (`0x00A049E6`) is never reached from
  here. A quieted sNaN keeps a non-zero payload below the quiet bit, so it is never the default-NaN pattern that
  prints as `-1.#IND`.
- Text of a **scalar** (what `GetProperty(id, cIGZString&)` writes for one): integers with the formats above, Bool
  `0`/`1`, floats as for arrays, a Char or RZChar **its one character** (a zero byte gives a one-character text
  holding NUL), an RZUnicodeChar its character as one byte when it is `0xFF` or less, otherwise `?`. Worked
  example: RZChar `0x41` gives `A`; RZUnicodeChar `0x00E9` gives the byte `0xE9`, `0x20AC` gives `?` (note the
  array rule above keeps only units below `0x80`).
- **Parsing** (`ConvertFromString` `0x009167C5`), in this order:
  1. The **whole text** `t` or `true` gives 1 and `f` or `false` gives 0, **case-sensitive** (`T`, `True`, `TRUE`
     go on to step 3 and parse as 0).
  2. Text that **starts with lowercase `0x`** and has a hex digit as its third character is unsigned hexadecimal
     (`RZAtoUint64HexA` `0x0090FD90`): at most **16** hex digits after the `0x`, up to the first non-hex character;
     digits after the sixteenth are ignored. `0X` is not recognised, and neither is `0x` after a leading space.
  3. Otherwise the characters are scanned from the start, setting three flags. `.` sets *dot* wherever it is, with
     or without digits. `-` sets *minus*. `e` or `E` sets *exponent* only if **some earlier character was a decimal
     digit**, not necessarily the one just before it. The scan **stops early** at an `e`/`E` with no digit before
     it, at a second `e`/`E`, at a second `-` (counted separately before and after the exponent letter) and at a
     second `.` after the exponent letter; the flags already set decide. A second `.` before any exponent letter
     makes the text floating point at once.
  4. *dot* or *exponent* set: **floating point** (`RZAtofEnglishA` `0x00910991`, the parser of §4.3 Float32).
     Otherwise *minus* set: signed decimal; otherwise unsigned decimal. Both decimal cases use `RZAtoi64A`
     (`0x0090FC6F`): leading white space skipped, one optional `+` or `-`, then decimal digits up to the first
     other character, accumulated in 64 bits with wraparound. There is **no hexadecimal without the `0x`
     prefix**. Signed and unsigned give the same bits; they differ only for float targets, where an unsigned
     value takes the 24-bit path above ("Unsigned 64-bit…") and a signed one is converted exactly, then rounded
     to the target.

     **Signed text into a Float32** (EX25): the int64 is loaded exactly (`fild`) and stored to the Float32 once
     (`0x00916BF6`–`0x00916BF9`; Float64 `0x00916C58`). That is one rounding from the exact value, so `-16777217`
     gives −16,777,216.0 (a tie, to even). A **Sint64 scalar** read as Float32 rounds twice: first to the double
     that the converter stores (`0x00915B85`, which writes the variant's double), then to Float32. The two differ
     only above 2⁵³ in magnitude. Worked example: −18,014,399,583,223,809 (−(2⁵⁴ + 2³⁰ + 1)) as text gives
     −18,014,400,656,965,632.0 (−(2⁵⁴ + 2³¹)). As a Sint64 scalar it becomes the double −(2⁵⁴ + 2³⁰), which is a
     tie for Float32 and goes to even: −18,014,398,509,481,984.0 (−2⁵⁴).
  5. Then the target. Integer types keep the low bits; floating point is first truncated toward zero as in
     "Floats to integers". **Bool** is value ≠ 0, so floating-point text **`0.5` gives true** (a Float32 scalar
     0.5 gives false, because it is truncated first) and NaN text gives true. Float32 and Float64 as in the table.
     **Char, RZChar and RZUnicodeChar targets always get 0**, and the getter succeeds.

  Worked examples, read with `GetProperty(id, uint32&)`: `0x1F` gives 31; `0X1F` gives 0; `1A` gives 1; `true`
  gives 1 and `TRUE` gives 0; `-5` gives `0xFFFFFFFB`; `1-2` gives 1; `e5` gives 0 (no digit before the `e`);
  `1e3` gives 1000; `.5` gives 0 (and 0.5 as Float32); `1.2.3` gives 1 (floating point, which stops at the second
  `.`); `12 apples` gives 12 (the `e` of `apples` follows a digit, so the text is parsed as floating point, which
  stops at the space); `0x123456789ABCDEF012` read as Uint64 gives `0x123456789ABCDEF0`.

Consequences that matter for tuning *[compat]*:

- `GetProperty(id, uint32&)` on a **one-element Uint32 array** (3,564 shipped properties) returns that element; on
  a longer array it returns the **first** element. On a Float32 array `{2.75}` it returns 2.
- A Uint32 property `5` read as Float32 gives 5.0; a Float32 `2.9` read as Uint32 gives 2, `-1.5` gives
  `0xFFFFFFFF`.
- A string property read as Uint32 is parsed as a number (`"12"` → 12, `"0x1F"` → 31).

### 5.2 Raw accessors (no check)

`cIGZVariant::AsX()` (slots 62–91) return the stored bits reinterpreted as `X` **without any type check**, and
`RefX()` (slots 93–123, no argument) return the data pointer without a check *[compat] [quirk]*. On an array
`AsUint32()` returns the **pointer** value; on a scalar `RefFloat32()` returns the value bits as a pointer. This is
what foundations §4.1 describes for curves ("reinterprets the raw bytes of other value types as floats").
`GetCount()` is 0 for a scalar and the element count for an array.

`cISCPropertyHolder` slot 5 `GetProperty(id, void* buffer, uint32& length)` (`0x005FD290`, for exemplars and
cohorts too) calls the variant's `GetValVoid` (`0x00915714`). It requests a **void array** (memory type `0x8000`,
disk tag `0x8000`, §4.2) and nothing else. For a void array of `n` bytes it lowers `length` to `n` if it was
larger, copies `length` bytes into the buffer, and returns true. For **any other type**, strings and other arrays
included, it returns **false** and leaves both outputs unchanged: there is no text round trip. Worked example:
a void array `01 02 03` read with `length` 8 copies 3 bytes and sets `length` to 3; with `length` 2 it copies 2
bytes and leaves `length` at 2; a Uint8 array `01 02 03` gives false. No shipped record has a void array, so on
shipped data slot 5 always returns false.

The buffer's own size is not passed (EX30). The original trusts `length` as the room available and copies with a
plain memory copy (`0x009F19B0`), so a caller whose buffer is shorter than `length` is overrun whenever the array is
longer than the buffer. *[choice]* An interface that knows the buffer's size copies at most that many bytes. This is
not observable on shipped data.

Call sites using the `AsX()` and `RefX()` accessors expect a fixed stored type; with shipped data that type is
always present. On a mismatch the original reads whatever bytes the variant holds: `AsX()` loads 1, 2, 4 or 8
bytes from the value field at offset 4 (`0x0040CE50`, `0x0060CE80`, `0x005FD610`–`0x005FD640`). Worked
examples: a Float32 2.75 read with `AsUint32()` gives `0x40300000`; a 32-bit scalar read with `AsSint64()` or
`AsFloat64()` takes its upper half from the variant's next field, which is not part of the value; an array read
with `AsUint32()` gives its heap address. *[choice]* Because the result is another type's bits or an address,
FreeCity4 converts element-wise where a spec says so, and otherwise treats a type mismatch as a missing property.

**The vector helpers** (EX33) used by the readers (§7.5) do the same for whole arrays. `vec<bool>`
(`0x005FF000`), `vec<u32>` (`0x005FF510`), `vec<u8>` (`0x005FFB40`), `vec<s32>` (`0x005FFC00`), `vec<f32>`
(`0x005FFCC0`) and `vec<s64>` (`0x005FFD80`) each take the property's raw data pointer and `GetCount()`, resize the
output to that count and copy count elements **of their own width**, with no type check and no conversion
(`vec<bool>` makes each byte a bit, non-zero being true). So:

- A stored type of the same width gives its bits: a Float32 array `{2.75}` read with `vec<u32>` gives
  `{0x40300000}`, not 2.
- A narrower stored type is over-read: a Uint8 array of 3 read with `vec<f32>` copies 12 bytes from a 3-byte
  buffer, the last 9 from whatever follows it in memory.
- A wider stored type gives its leading bytes: a Uint32 array `{0x11223344, 5}` read with `vec<u8>` gives
  `{0x44, 0x33}`.
- A scalar has count 0 and gives an empty vector (§4.3, `Uint8:0:{}`).

**Shipped scalars that only a vector helper reads** (EX39). A sweep of the binary exemplars and cohorts of the
shipped `.dat` files (2026-10-14), against every property ID passed to a vector helper, finds four properties
stored as scalars (key type 0) in some records. The original reads each of those records as an **empty** vector:

| Property | Helper and readers | Scalar records | Effect in the original |
|---|---|---|---|
| `0x88EDC793` (lot configuration zone types) | `vec<u8>`: `0x0047A0EC`, `0x0060F690` | 10, all Uint8 0; landmark lots such as `LM6x6_LincolnCenter` | no zone type |
| `0x88EDC795` (wealth types) | `vec<u8>`: `0x0047A16C`, `0x0060F710` | 161, all Uint8 0 | no wealth type |
| `0x88EDC796` (purpose types) | `vec<u8>`: `0x0047A20C` | 172, all Uint8 0 | no purpose type |
| `0xEA54D286` (budget item cost) | `vec<s64>`: `0x0069112B`, `0x0069131A`, `0x00693899` (budget simulator) | 2: landfill tiles `0x02F10000` (1) and `0x034E0000` (10) | no cost entry |

The scalar records of the first three are 172 lot configurations (exemplar type 16): all 172 hold `0x88EDC796`
as a scalar, 161 of them `0x88EDC795` too, and 10 of those also `0x88EDC793`. The budget simulator checks each index
against each array's size and gives 0 for a missing entry (`0x006939A9`); whether it reads landfill tiles was not
traced. The landfill maintenance cost reads the same property with the converting getter `0x005FD4B0` instead, so for it
the two tiles cost 1 and 10 (landfill §4.9.1). So the original's result differs by reader, and only the vector helpers
give "empty".

A vector reader that returns a scalar's one value gives these 172 lot configurations a bit 0 in their purpose set
(and, for some, in their wealth and zone sets) that the original does not give them. FreeCity4's vector readers must
therefore return an **empty** vector for a scalar, as the original does, and keep the one value only in the converting
getters (§5.1).

On shipped data every raw read finds the type it expects except one, property `0xEA0232A0` (shipped Sint32, read
with `vec<u32>`), where copying and converting give the same bits (§7.4). *[choice]* Converting every element as
in §5.1 is not observable on shipped data, and it replaces the over-read with defined values.

### 5.3 Exemplar lookup helper

`SC4GetExemplarProperty(instance, propertyID)` (`0x005F95E0`) loads the exemplar (`0x6534284A`, group 0,
the given instance) requesting `0xA52160F5` and returns the property's variant, or null when the exemplar or the
property is missing. Simulator tuning is mostly read by key (group `0xE7E2C2DB`) through the curve loaders of
foundations §4.1 and the subsystem code listed in §7.

---

## 6. LTEXT and locale

### 6.1 LTEXT records *[compat]*

Type `0x2026960B`. The string resource (`cRZStringResource`) reads the payload with the foundations §2.3 string
reader (`GetGZStr`), so the record is:

```
uint32  header      encoding = header >> 24, count = header & 0x00FFFFFF
        encoding 0x10: count UTF-16LE code units (converted to UTF-8)
        encoding 0x00: count bytes of 8-bit text
```

- The usual description "`uint16` count, `uint16` marker `0x1000`/`0x0000`" is the same bytes for counts below
  65,536; the original reads a **24-bit** count.
- All §2.3 rules apply: count > 200,000 fails; an 8-bit text whose **second byte is 0** is decoded as UTF-16
  *[quirk]*; an encoding byte other than 0 and 0x10 selects the legacy path, which reads the whole record as raw
  bytes *[quirk]*; **bytes after the string are ignored** (the length need not match the record size).
- Shipped: every UTF-16 record has exactly `4 + 2·count` bytes. Each locale file has one 8-bit record and the
  English file also one headerless plain-text record (group `0x55555555`, instance `0xAA41380E`, 96 bytes,
  read through the legacy path as its full text).
- The same type ID holds **audio** in `Sound.dat` (XA and RIFF clips). The game never reads those as text: the
  caller decides. *[choice]* FreeCity4 keeps `ltext::is_ltext` for tools only.
- Internally the game's text is UTF-8: `SC4LoadString` additionally replaces every U+2022 (`E2 80 A2`) by U+200B
  (`E2 80 8B`) in the loaded string.

### 6.2 Locale selection *[compat]*

- The language manager (`cIGZLanguageManager`, `cRZLanguageManager`) holds the current language ID. The
  language folder loaded in step 2 of §3.1 is that language's directory name (`English`, `German`, …;
  special names include `UKEnglsh` for ID 2, `Norwgian`, `Portgese`, `TChinese`). Locale files carry the same
  LTEXT keys in every language; the one that is loaded wins over the root files.
- `SC4LoadString(out, group, instance)` first asks for (`0x2026960B`, **group + current language ID**,
  instance), and, if that is missing and the language ID is not 0, for (`0x2026960B`, group, instance). The game
  passes the shipped groups (e.g. `0xEA231E96`), so with shipped data the first lookup always misses
  *[compat]*: a plugin can supply per-language text at `group + languageID`.
- How the current language is chosen at startup (registry, command line, installed folders) is not covered here
  (open question 1).

---

## 7. Tuning catalogue (M1–M5)

All exemplars below have type `0x6534284A`. Every one exists exactly once, in `SimCity_1.dat`, with no parent
cohort; no other shipped file or the Sku plugin carries a copy (so load order (§3.1) and inheritance (§4.4) do not
affect shipped tuning). Readers and getter helpers were checked in the executable, shipped values read from
`SimCity_1.dat`, owning specs cited. Names from `re/tuning/simulators.md`, "(spec)" = name from the owning spec.

**Code type** is the call the reader uses: the scalar helpers `cSCPropertyHelp::GetPropertyValue`
(`0x005FD390` bool, `0x005FD3C0` u8, `0x005FD3F0` s16, `0x005FD420` u16, `0x005FD450` s32, `0x005FD480` u32,
`0x005FD4B0` s64, `0x005FD4F0` f32, `0x005FD530` f64) fetch the property and call the variant's converting
`GetValX(X&)`, so §5.1 applies (any numeric shipped type is converted; a one-element array gives its element).
`0x005FDAC0` converts any numeric type to float and multiplies by 0.01f. "raw" and the vector helpers
(`0x005FF510` u32, `0x005FFCC0` f32, `0x005FFD80` s64, `0x005FFC00` s32, `0x005FF000` bool, `0x005FFB40` u8)
copy the variant's data with **no type check** (§5.2). Curves: `0x005F4700` by key and ID, `0x005F4620` from a
property, `0x005F4480` from a code point list (foundations §4.1 patterns).

### 7.1 Exemplars

| Exemplar (T/G/I) | Name | Copies (file, parent) | Readers | Spec |
|---|---|---|---|---|
| 0x6534284A/0xE7E2C2DB/0xE9590E52 | Main Simulator | SimCity_1.dat parent 0 | 0x007025B0 cSC4Simulator::ReadSimulatorTunableValues (from Init) | simulator.md §7 |
| 0x6534284A/0xE7E2C2DB/0x0BFF6824 | Budget Tuning Parameters | SimCity_1.dat parent 0 | 0x00692210 cSC4BudgetSimulator::ReadSimulatorTunableValues (from Init) | budget.md §9 |
| 0x6534284A/0xE7E2C2DB/0x4A1F3CE6 | Demand Simulator | SimCity_1.dat parent 0 | 0x006A8790 cSC4DemandSimulator::ReadSimulatorTunableValues (Init; DoMessage 0xCA1B151C) | demand.md §11.1 |
| 0x6534284A/0xE7E2C2DB/0x4A1F6115 | Nation | SimCity_1.dat parent 0 | 0x00497B90 cSC4Nation::ReadSimulatorTunableValues | demand.md §11.2 |
| 0x6534284A/0x7A4A8458/0x1A2FDB6B | Weather Tuning Parameters | SimCity_1.dat parent 0 | 0x00725AF0 cSC4WeatherSimulator::ReadSimulatorTunableValues | weather.md §9 |
| 0x6534284A/0xE7E2C2DB/0xE7E2C8D8 | Land Value Sim | SimCity_1.dat parent 0 | 0x006BE110 cSC4LandValueSimulator::ReadSimulatorTunableValues (from Init) | landvalue.md §8 |
| 0x6534284A/0x88CD66E9/0x00000001 | Terrain Properties | SimCity_1.dat parent 0 | 0x0074B6A0 cSTETerrain::ReadTuningParameters (terrain, not a simulator) | landvalue.md §8 (named only) |
| 0x6534284A/0xE7E2C2DB/0xC911E35B | Utilities | SimCity_1.dat parent 0 | pollution 0x006E5B10 ReadSimulatorTunableValues (+ 0x006E44B0 SimulationBegin, 0x006E7930 EndOfCycle for 0x6911E2DF); power 0x006ED1C0 ReadSimulatorTunableValues; plumbing 0x006DC9E0 ReadSimulatorTunableValues | pollution.md §7.1; power.md §8; plumbing.md §8 |
| 0x6534284A/0xE7E2C2DB/0xC8E8F6C5 | Crime Simulator | SimCity_1.dat parent 0 | 0x006A07A0 cSC4CrimeSimulator::ReadSimulatorTunableValues (Init; DoMessage 0x006A21C0 on 0xC53D10AA) | crime.md §8 |
| 0x6534284A/0xE7E2C2DB/0xAA023079 | Aura Simulator | SimCity_1.dat parent 0 | 0x0068D770 cSC4AuraSimulator::ReadSimulatorTunableValues (Init; DoMessage 0x0068FA60 on 0xC53D10AA); curve helper 0x0068D700 | aura.md §10 |
| 0x6534284A/0xE7E2C2DB/0x68F5524A | Fire Protection Simulator | SimCity_1.dat parent 0 | 0x006ADDA0 cSC4FireProtectionSimulator::ReadSimulatorTunableValues (Init; DoMessage 0x006AE070 on 0xC53D10AA) | protection.md §6.7 |
| 0x6534284A/0xE7E2C2DB/0x28DFCF89 | Police Simulator | SimCity_1.dat parent 0 | 0x006E2110 cSC4PoliceSimulator::ReadSimulatorTunableValues (Init; DoMessage 0x006E2920 on 0xC53D10AA) | protection.md §6.7 |
| 0x6534284A/0x7A4A8458/0x1A2FDB6A | Flora Tuning Parameters | SimCity_1.dat parent 0 | 0x006B00F0 cSC4FloraSimulator::ReadSimulatorTunableValues (Init; DoMessage 0x006B2F70) | flora.md §7 |
| 0x6534284A/0xE7E2C2DB/0xA8F41EE3 | Flammability Simulator | SimCity_1.dat parent 0 | 0x006AEB50 cSC4FlammabilitySimulator::ReadSimulatorTunableValues | flammability.md §8 |
| 0x6534284A/0xE7E2C2DB/0x69189C84 | Residential simulator | SimCity_1.dat parent 0 | 0x006FC0C0 ReadTunablePopulationValues; 0x006FBAB0 ReadSimulatorTunableValues; per call: 0x006F88B0 GetAverageNewAgeByWealth, 0x006FC570 GetCellLifeExpectancy, 0x006FC9E0 GetGlobalLE, 0x006FC6E0 GetCellWorkforcePercent, 0x006FC860 GetGlobalWorkforcePercent (all cSC4ResidentialSimulator) | residential.md §7; residential-education-health.md §4.13 |
| 0x6534284A/0x7A4A8458/0x6A469021 | Neighbor Deals | SimCity_1.dat parent 0 | 0x006D38F0 cSC4NeighborsSimulator::Init (holds the exemplar at this+0x28); per-call readers 0x006D0520 create_deal_proposition, 0x006D0E50 simulate_deals, 0x006D21A0 ProposeDeals, 0x006D2A20 UpdateCommodityStats, 0x006D3250 GetDealCost, 0x006D3620 GetDealMaxAmount | neighbors.md §12, §8.3 |
| 0x6534284A/0x67CD5FA1/0x6A2806AF | LandfillDeveloper | SimCity_1.dat parent 0 | 0x006B8C70 cSC4LandfillDeveloper::ReadTuningParams | landfill.md §4.1 |
| 0x6534284A/0xE7E2C2DB/0xE8DA7677 | Building Development | SimCity_1.dat parent 0 | 0x006959E0 cSC4BuildingDevelopmentSimulator::ReadSimulatorTunableValues; 0x006C4A20 cSC4LotDeveloper::ReadSimulatorTunableValues; 0x00706FC0 cSC4TractDeveloper::ReadSimulatorTunableValues | building-development.md §4.13; lot-developer.md §3.5; tract-developer.md §4.21 |
| 0x6534284A/0xE7E2C2DB/0xE9482490 | Zone Manager | SimCity_1.dat parent 0 | 0x004DC460 cSC4ZoneManager::ReadSimulatorTunableValues | zones.md §4 |
| 0x6534284A/0x67CD5FA1/{0x1010, 0x1020, 0x1030, 0x3110, 0x3120, 0x3130, 0x3320, 0x3330, 0x4100, 0x4200, 0x4300, 0x4400} | Growth developers (12) | 12 exemplars, each once: SimCity_1.dat parent 0 | 0x006B78B0 cSC4GrowthDeveloper::ReadSimulatorTunableValues (holder passed to Init at this+0x30) | growth-developer.md §4.8, §4.9 |
| 0x6534284A/0xE7E2C2DB/0xC9133286 | Traffic Simulator | SimCity_1.dat parent 0 | 0x00716AE0 cSC4TrafficSimulator::ReadSimulatorTunableValues (Init; DoMessage 0x007241B0 re-reads when the changed exemplar is 0xC9133286); 0x00711810 UpdateTravelStrategies (called at the end of the reader and on OrdinanceUpdated) | traffic-simulator.md §4.14, §4.1.2; traffic-network.md §4.5; pathfinder.md (uses) |

### 7.2 Properties

| Exemplar (group/instance) | Prop | Name | Reader | Code type | Default when missing | Shipped | Spec | Notes |
|---|---|---|---|---|---|---|---|---|
| `E7E2C2DB/E9590E52` | `0x89590E6D` | Simulator Speeds | 0x007025B0 | u32 raw | unchanged (reader sets none); ctor {5000,1000,100} per spec §3 (not r… | u32 arr n=3: 2000, 1000, 50 | simulator.md §7, §3 |  |
| `E7E2C2DB/E9590E52` | `0x89590E6E` | kAnimationTimeDilationProperty | 0x007025B0 | vec<f32> raw | {0,1,1.5,2,1} written just before the read (only when the exemplar ex… | f32 arr n=5: 1, 1, 1.5, 3, 1 | simulator.md §7, §3 | vector takes the property's length (no count check) |
| `E7E2C2DB/0BFF6824` | `0x6A9C7718` | Budget: Initial Funds | 0x00692210 | vec<s64> raw | unchanged (vector; empty from ctor per spec) | s64 arr n=3: 500000, 200000, 100000 | budget.md §9 |  |
| `E7E2C2DB/0BFF6824` | `0x890E0A99` | Budget: MaxTaxRate | 0x00692210 | f32 | 25.0 (imm 0x41C80000 @0x00692234) | f32 scl n=1: 25 | budget.md §9 |  |
| `E7E2C2DB/0BFF6824` | `0x090E0A9F` | Budget: MinAllowableFunds | 0x00692210 | s64 | -100000 (@0x0069223E) | s64 scl n=1: -100000 | budget.md §9 |  |
| `E7E2C2DB/0BFF6824` | `0x490E0AA6` | Budget: BondIncrement | 0x00692210 | s64 | 1000 (@0x0069224B) | s64 scl n=1: 5000 | budget.md §9 |  |
| `E7E2C2DB/0BFF6824` | `0x090E0AAC` | Budget: MaxBonds | 0x00692210 | s32 | 5 (@0x00692255) | u8 scl n=1: 10 | budget.md §9 | shipped u8 is converted by the variant getter; getter s32, shipped u8: converted by the v… |
| `E7E2C2DB/0BFF6824` | `0x890E0AB4` | Budget: MaxBondAmount | 0x00692210 | s64 | 10000 (@0x0069225F) | s64 scl n=1: 200000 | budget.md §9 |  |
| `E7E2C2DB/0BFF6824` | `0x8A3FB98D` | Budget: BondInterestRate | 0x00692210 | f32 | 0.085 (0x3DAE147B @0x0069226F) | f32 scl n=1: 8.5 | budget.md §9 |  |
| `E7E2C2DB/0BFF6824` | `0x0A3FB9A8` | Budget: BondPaybackMonths | 0x00692210 | u32 | 120 (@0x00692279) | u32 scl n=1: 120 | budget.md §9 |  |
| `E7E2C2DB/0BFF6824` | `0x89170C6C` | LV->R tax factor (spec) | 0x00692210 | f32 | global; reader sets none | ABSENT | budget.md §9 | unused per spec; ABSENT in shipped data |
| `E7E2C2DB/0BFF6824` | `0x09170C74` | LV->C tax factor (spec) | 0x00692210 | f32 | global; reader sets none | ABSENT | budget.md §9 | unused per spec; ABSENT in shipped data |
| `E7E2C2DB/0BFF6824` | `0xC9170C78` | LV->I tax factor (spec) | 0x00692210 | f32 | global; reader sets none | ABSENT | budget.md §9 | unused per spec; ABSENT in shipped data |
| `E7E2C2DB/0BFF6824` | `0x49170C7C` | MinAverageLV (spec) | 0x00692210 | u8 | global; reader sets none | ABSENT | budget.md §9 | spec types it float; code reads u8; unused per spec; ABSENT in shipped data |
| `E7E2C2DB/0BFF6824` | `0xA9188EF5` | Budget: TaxRateOccWeight_R$ | 0x00692210 | f32 | unchanged (reader sets none; spec: 1) | f32 scl n=1: 1 | budget.md §9, §13.1 |  |
| `E7E2C2DB/0BFF6824` | `0xE918C920` | Budget: TaxRateOccWeight_R$$ | 0x00692210 | f32 | unchanged (reader sets none; spec: 1) | f32 scl n=1: 1 | budget.md §9, §13.1 |  |
| `E7E2C2DB/0BFF6824` | `0x4918C925` | Budget: TaxRateOccWeight_R$$$ | 0x00692210 | f32 | unchanged (reader sets none; spec: 1) | f32 scl n=1: 1 | budget.md §9, §13.1 |  |
| `E7E2C2DB/0BFF6824` | `0xA918C929` | Budget: TaxRateOccWeight_Cs$ | 0x00692210 | f32 | unchanged (reader sets none; spec: 1) | f32 scl n=1: 1 | budget.md §9, §13.1 |  |
| `E7E2C2DB/0BFF6824` | `0x4918C92C` | Budget: TaxRateOccWeight_Cs$$ | 0x00692210 | f32 | unchanged (reader sets none; spec: 1) | f32 scl n=1: 1 | budget.md §9, §13.1 |  |
| `E7E2C2DB/0BFF6824` | `0x4918C930` | Budget: TaxRateOccWeight_Cs$$$ | 0x00692210 | f32 | unchanged (reader sets none; spec: 1) | f32 scl n=1: 1 | budget.md §9, §13.1 | stored into the CO$$$ weight and then overwritten by 0x4918C93A; the CS$$$ slot (+0x7DC)… |
| `E7E2C2DB/0BFF6824` | `0x8918C937` | Budget: TaxRateOccWeight_Co$$ | 0x00692210 | f32 | unchanged (reader sets none; spec: 1) | f32 scl n=1: 1 | budget.md §9, §13.1 |  |
| `E7E2C2DB/0BFF6824` | `0x4918C93A` | Budget: TaxRateOccWeight_Co$$$ | 0x00692210 | f32 | unchanged (reader sets none; spec: 1) | f32 scl n=1: 1 | budget.md §9, §13.1 |  |
| `E7E2C2DB/0BFF6824` | `0x69A36E40` | Budget: TaxRateOccWeight_IResource | 0x00692210 | f32 | unchanged (reader sets none; spec: 1) | f32 scl n=1: 1 | budget.md §9, §13.1 |  |
| `E7E2C2DB/0BFF6824` | `0xA918C942` | Budget: TaxRateOccWeight_IDirty | 0x00692210 | f32 | unchanged (reader sets none; spec: 1) | f32 scl n=1: 1 | budget.md §9, §13.1 |  |
| `E7E2C2DB/0BFF6824` | `0x8918C93E` | Budget: TaxRateOccWeight_IManufacturing | 0x00692210 | f32 | unchanged (reader sets none; spec: 1) | f32 scl n=1: 1 | budget.md §9, §13.1 |  |
| `E7E2C2DB/0BFF6824` | `0xA9A36E47` | Budget: TaxRateOccWeight_IHighTech | 0x00692210 | f32 | unchanged (reader sets none; spec: 1) | f32 scl n=1: 1 | budget.md §9, §13.1 |  |
| `E7E2C2DB/0BFF6824` | `0x8A480DF7` | Budget: Waning Wealth Property Tax Mult… | 0x00692210 | vec<f32> raw | {1,1,1} (resize(3, 1.0) @0x0069228B before the fetch) | f32 arr n=3: 1, 0.75, 0.5 | budget.md §9 |  |
| `E7E2C2DB/0BFF6824` | `0xEBFF69E7` | Budget: Difficulty Tax Multiplier | 0x00692210 | vec<f32> raw | {100000,100000,100000} (resize(3, 0x47C35000) @0x006922A5) | f32 arr n=3: 1.2, 1.1, 1 | budget.md §9 |  |
| `E7E2C2DB/0BFF6824` | `0x0C032488` | Budget: Initial Tax Rate | 0x00692210 | vec<f32> raw | unchanged (no resize in the reader) | f32 arr n=3: 9, 9, 9 | budget.md §9 |  |
| `E7E2C2DB/0BFF6824` | `0x4AC519B8` | Budget: Land Value to Simoleans | 0x00692210 | curve(key,id) | keep (previous points; spec: ctor (0,0),(255,10), not re-verified) | f32 arr n=4: 0, 0, 255, 0 | budget.md §9 |  |
| `E7E2C2DB/4A1F3CE6` | `0x4A1F38B5` | Demand: Neutral Tax Rate vs. Population | 0x006A8790 | curve(prop*) | delete (curve object freed when the property is missing) | f32 arr n=82: 0, 9, 150000, 8.9, 300000, 8.9, 450000, 8.8, 600000, 8.… | demand.md §11.1; foundations §4.1 |  |
| `E7E2C2DB/4A1F3CE6` | `0x4A1F38B6` | Demand: Tax Modifier vs. Rate Variance | 0x006A8790 | curve(prop*) | delete | f32 arr n=6: 0.2, 0.3, 1, 0, 5, -3.5 | demand.md §11.1 |  |
| `E7E2C2DB/4A1F3CE6` | `0x4A1F38B7` | Demand: Days of Tax Penalty | 0x006A8790 | u32 | 90 (written just before the read @0x006A895D) | u32 scl n=1: 90 | demand.md §11.1 |  |
| `E7E2C2DB/4A1F3CE6` | `0x4A1F38C5` | Demand: Commercical Cap Relief for Conn… | 0x006A8790 | vec<f32> raw | 13 x 0 (written when the read fails) | f32 arr n=13: 10000, 10000, 100000, 0, 0, 0, 20000, 20000, 20000, 200… | demand.md §11.1 |  |
| `E7E2C2DB/4A1F3CE6` | `0x4A1F38C6` | Demand: Commercical Cap Relief for Airp… | 0x006A8790 | f32 | 1.0 (written before the read) | f32 scl n=1: 1 | demand.md §11.1 |  |
| `E7E2C2DB/4A1F3CE6` | `0x4A1F38C7` | Demand: Commercical Cap Relief for Seap… | 0x006A8790 | f32 | 2.0 | f32 scl n=1: 2 | demand.md §11.1 | spec maps it to reliefSeaI although the name says Commercial |
| `E7E2C2DB/4A1F3CE6` | `0x4A1F38C8` | Demand: Industrial Cap Relief for Trips | 0x006A8790 | f32 | 20.0 | f32 scl n=1: 20 | demand.md §11.1 |  |
| `E7E2C2DB/4A1F3CE6` | `0x4A1F38C9` | Demand: Industrial Cap Relief for Airpo… | 0x006A8790 | f32 | 0.1 (0x3DCCCCCD) | f32 scl n=1: 0.1 | demand.md §11.1 |  |
| `E7E2C2DB/4A1F3CE6` | `0x4A1F38CA` | Demand: Industrial Cap Relief for Seapo… | 0x006A8790 | f32 | 20.0 | f32 scl n=1: 20 | demand.md §11.1 | spec maps it to reliefSeaC although the name says Industrial |
| `E7E2C2DB/4A1F3CE6` | `0x8A63F2A1` | Average Salary for Demographic | 0x006A8790 | vec<u32> raw | 12 x 100 (written when the read fails) | u32 arr n=12: 17000, 19250, 21500, 26000, 25000, 31250, 37500, 50000… | demand.md §11.1 |  |
| `E7E2C2DB/4A1F6115` | `0x4A1F629F` | Nation: Demand ID list | 0x00497B90 | u32 raw | no IDs: no curves, and 0x4A1F6300 is never read | u32 arr n=1: 0 | demand.md §11.2 |  |
| `E7E2C2DB/4A1F6115` | `0x4A1F62A0` | Nation: Default Demand Modifier vs. Mon… | 0x00497B90 | curve(prop*) | empty curve object for a listed ID whose property is missing | f32 arr n=2: 0, 0 | demand.md §11.2 | ID = 0x4A1F62A0 + index (lea ecx,[esi+0x4A1F62A0] @0x00497C90) |
| `E7E2C2DB/4A1F6115` | `0x4A1F6300` | Nation: Difficulty Demand Modifier | 0x00497B90 | vec<f32> raw | {0,0,0} | f32 arr n=3: 0.06, 0.03, 0.01 | demand.md §11.2 |  |
| `7A4A8458/1A2FDB6B` | `0xA7607D70` | unnamed | 0x00725AF0 | s32 | unchanged (spec: ctor 8) | s32 scl n=1: 1 | weather.md §9 |  |
| `7A4A8458/1A2FDB6B` | `0xA7607D71` | unnamed | 0x00725AF0 | s32 | unchanged (spec: ctor 8) | s32 scl n=1: 1 | weather.md §9 |  |
| `7A4A8458/1A2FDB6B` | `0xA7607D72` | unnamed | 0x00725AF0 | s32 | unchanged (spec: ctor 1) | s32 scl n=1: 1 | weather.md §9 |  |
| `7A4A8458/1A2FDB6B` | `0xA7607D73` | unnamed | 0x00725AF0 | f32 | unchanged (spec: ctor 24) | f32 scl n=1: 65 | weather.md §9 |  |
| `7A4A8458/1A2FDB6B` | `0xA7607D74` | unnamed | 0x00725AF0 | f32 | unchanged (spec: ctor 0.5) | f32 scl n=1: 0.5 | weather.md §9 |  |
| `7A4A8458/1A2FDB6B` | `0xA7607D75` | unnamed | 0x00725AF0 | f32 | unchanged (spec: ctor 0.5) | f32 scl n=1: 0.5 | weather.md §9 |  |
| `7A4A8458/1A2FDB6B` | `0xA7607D76` | unnamed | 0x00725AF0 | s32 | unchanged (spec: ctor 6) | s32 scl n=1: 10 | weather.md §9 |  |
| `7A4A8458/1A2FDB6B` | `0xA7607D77` | unnamed | 0x00725AF0 | f32 | unchanged (spec: ctor 0.95) | f32 scl n=1: 0.9 | weather.md §9 |  |
| `7A4A8458/1A2FDB6B` | `0xA7607D78` | unnamed | 0x00725AF0 | f32 | unchanged (spec: ctor 0.0035) | f32 scl n=1: 0.082 | weather.md §9 |  |
| `7A4A8458/1A2FDB6B` | `0xA7607D79` | unnamed | 0x00725AF0 | f32 | unchanged (spec: ctor 0.25) | f32 scl n=1: 0.25 | weather.md §9 |  |
| `7A4A8458/1A2FDB6B` | `0xA7607D86` | unnamed | 0x00725AF0 | f32 | unchanged (spec: ctor 3) | ABSENT | weather.md §9 | ABSENT in shipped data |
| `7A4A8458/1A2FDB6B` | `0xA7607D87` | unnamed | 0x00725AF0 | f32 | unchanged (spec: ctor 0) | ABSENT | weather.md §9 | ABSENT in shipped data |
| `7A4A8458/1A2FDB6B` | `0xA7607D88` | unnamed | 0x00725AF0 | f32 | unchanged (spec: ctor 1) | ABSENT | weather.md §9 | ABSENT in shipped data |
| `7A4A8458/1A2FDB6B` | `0xA7607D7A` | unnamed | 0x00725AF0 | f32 | unchanged (spec: ctor 0.5) | f32 scl n=1: 0.2 | weather.md §9 |  |
| `7A4A8458/1A2FDB6B` | `0xA7607D7B` | unnamed | 0x00725AF0 | f32 | unchanged (spec: ctor 0.5) | f32 scl n=1: 0.2 | weather.md §9 |  |
| `7A4A8458/1A2FDB6B` | `0xA7607D7C` | unnamed | 0x00725AF0 | f32 | unchanged (spec: ctor 0.4) | f32 scl n=1: 1 | weather.md §9 |  |
| `7A4A8458/1A2FDB6B` | `0xA7607D7D` | unnamed | 0x00725AF0 | f32 | unchanged (spec: ctor 128 stored (slopeK)) | f32 scl n=1: 0.35 | weather.md §9 |  |
| `7A4A8458/1A2FDB6B` | `0xA7607D7E` | unnamed | 0x00725AF0 | f32 | unchanged (spec: ctor 0.12) | f32 scl n=1: 1 | weather.md §9 |  |
| `7A4A8458/1A2FDB6B` | `0xA7607D7F` | unnamed | 0x00725AF0 | f32 | unchanged (spec: ctor 4) | f32 scl n=1: 2.5 | weather.md §9 |  |
| `7A4A8458/1A2FDB6B` | `0xA7607D80` | unnamed | 0x00725AF0 | u32 | unchanged (spec: ctor 64) | u32 scl n=1: 40 | weather.md §9 |  |
| `7A4A8458/1A2FDB6B` | `0xA7607D81` | unnamed | 0x00725AF0 | u32 | unchanged (spec: ctor 15) | u32 scl n=1: 16 | weather.md §9 |  |
| `7A4A8458/1A2FDB6B` | `0xA7607D89` | unnamed | 0x00725AF0 | f32 | unchanged (spec: ctor 0.25 stored (windSpread)) | f32 scl n=1: 60 | weather.md §9 |  |
| `E7E2C2DB/E7E2C8D8` | `0x47E2C200` | Tract Size | 0x006BE110 | u8 | unchanged | u8 scl n=1: 2 | landvalue.md §8 |  |
| `E7E2C2DB/E7E2C8D8` | `0x47E2C300` | Land Value Intrinsic Min/Max | 0x006BE110 | u8 raw | none: a missing property is a null dereference (spec agrees) | u8 arr n=2: 1, 64 | landvalue.md §8 |  |
| `E7E2C2DB/E7E2C8D8` | `0x47E2C301` | Land Value Wealth Boundaries | 0x006BE110 | u8 raw | none: a missing property is a null dereference (spec agrees) | u8 arr n=2: 70, 120 | landvalue.md §8 |  |
| `E7E2C2DB/E7E2C8D8` | `0x47E2C310` | Land Value Water Effect | 0x006BE110 | u8 raw | none: a missing property is a null dereference (spec agrees) | u8 arr n=2: 50, 10 | landvalue.md §8 |  |
| `E7E2C2DB/E7E2C8D8` | `0x47E2C320` | Land Value Altitude Effect | 0x006BE110 | curve(prop*) | delete (freed, this+0x84 := 0) | f32 arr n=14: 0, 0, 25, 5, 50, 10, 75, 21, 100, 50, 125, 30, ... (14) | landvalue.md §8 |  |
| `E7E2C2DB/E7E2C8D8` | `0x47E2C330` | Land Value Desirability IDs | 0x006BE110 | u32 raw | none: missing property is a null dereference | u32 arr n=1: 4144 | landvalue.md §8 |  |
| `E7E2C2DB/E7E2C8D8` | `0x47E2C331` | Land Value Desirability Factors | 0x006BE110 | f32 raw | none: missing property is a null dereference | f32 arr n=1: 2.5 | landvalue.md §8 |  |
| `E7E2C2DB/E7E2C8D8` | `0x47E2C332` | Land Value Desirability Curve | 0x006BE110 | curve(prop*) | delete (freed, this+0x88 := 0) | f32 arr n=4: -255, -255, 255, 255 | landvalue.md §8 |  |
| `88CD66E9/00000001` | `0x48CD7B26` | MaxAllowedHeight (spec) | 0x0074B6A0 | f32 | not checked (terrain code) | f32 scl n=1: 2500 | landvalue.md §8 |  |
| `88CD66E9/00000001` | `0x48CD7B27` | MinAllowedHeight (spec) | 0x0074B6A0 | f32 | not checked (terrain code) | f32 scl n=1: 20 | landvalue.md §8 |  |
| `E7E2C2DB/C911E35B` | `0x69501944` | Traffic air pollution factor | 0x006E5B10 | f32 | 8.0 (0x41000000), written before the fetch | f32 scl n=1: 0.25 | pollution.md §7.1 |  |
| `E7E2C2DB/C911E35B` | `0x49134BD4` | Max garbage value for UI | 0x006E5B10 | u32 | 27000, written before the fetch | u32 scl n=1: 32000 | pollution.md §7.1 |  |
| `E7E2C2DB/C911E35B` | `0x49134BDD` | Max air pollution value for UI | 0x006E5B10 | u32 | 27000, written before the fetch | u32 scl n=1: 1024 | pollution.md §7.1 |  |
| `E7E2C2DB/C911E35B` | `0x09134BE2` | Max water pollution value for UI | 0x006E5B10 | u32 | 32000, written before the fetch | u32 scl n=1: 1024 | pollution.md §7.1 |  |
| `E7E2C2DB/C911E35B` | `0x89135044` | Max radiation pollution value for UI | 0x006E5B10 | u32 | 32000, written before the fetch | u32 scl n=1: 1024 | pollution.md §7.1 |  |
| `E7E2C2DB/C911E35B` | `0x6A406201` | Air Pollution Delta | 0x006E5B10 | u32 | 50, written before the fetch | u32 scl n=1: 150 | pollution.md §7.1 |  |
| `E7E2C2DB/C911E35B` | `0x8A40620B` | Water Pollution Delta | 0x006E5B10 | u32 | 50, written before the fetch | u32 scl n=1: 150 | pollution.md §7.1 |  |
| `E7E2C2DB/C911E35B` | `0x0913519B` | Garbage Pollution Threshold | 0x006E5B10 | u32 | 16000, written before the fetch | u32 scl n=1: 1500 | pollution.md §7.1 |  |
| `E7E2C2DB/C911E35B` | `0xA91351A6` | Air Pollution Threshold | 0x006E5B10 | u32 | 16000, written before the fetch | u32 scl n=1: 512 | pollution.md §7.1 |  |
| `E7E2C2DB/C911E35B` | `0x091351AE` | Water Pollution Threshold | 0x006E5B10 | u32 | 16000, written before the fetch | u32 scl n=1: 512 | pollution.md §7.1 |  |
| `E7E2C2DB/C911E35B` | `0x291351B5` | Radiation Pollution Threshold | 0x006E5B10 | u32 | 1, written before the fetch | u32 scl n=1: 1 | pollution.md §7.1 |  |
| `E7E2C2DB/C911E35B` | `0x09135479` | Garbage decompostion amount | 0x006E5B10 | u32 | 5000, written before the fetch | u32 scl n=1: 2000 | pollution.md §7.1 |  |
| `E7E2C2DB/C911E35B` | `0x09135482` | Garbage UI scaling factor | 0x006E5B10 | f32 | 0.125 (0x3E000000), written before the fetch | f32 scl n=1: 0.01 | pollution.md §7.1 |  |
| `E7E2C2DB/C911E35B` | `0x4A652679` | Max Garbage on Valve | 0x006E5B10 | u32 | 655340, written before the fetch | u32 scl n=1: 65536 | pollution.md §7.1 |  |
| `E7E2C2DB/C911E35B` | `0x4A65267A` | Landfill Tile Capacity | 0x006E5B10 | u32 | 32767, written before the fetch | u32 scl n=1: 30000 | pollution.md §7.1 |  |
| `E7E2C2DB/C911E35B` | `0xAA1F8AB7` | Garbage service cost per unit of garbage | 0x006E5B10 | f32 | unchanged (reader sets none) | f32 scl n=1: 0.005 | pollution.md §7.1 |  |
| `E7E2C2DB/C911E35B` | `0x69A437F6` | Garbage Prop Request Curve | 0x006E5B10 | curve(key,id) | empty (curve emptied with 0x005F4480 before the fetch) | f32 arr n=6: 0, 0, 0.0625, 5, 0.125, 10 | pollution.md §7.1 |  |
| `E7E2C2DB/C911E35B` | `0x0A10C772` | Water Pollution Effect Strength Curve | 0x006E5B10 | curve(key,id) | empty (curve emptied with 0x005F4480 before the fetch) | f32 arr n=10: 0, 0, 0.002, 0.05, 0.005, 0.15, 0.01, 0.4, 0.02, 1 | pollution.md §7.1 |  |
| `E7E2C2DB/C911E35B` | `0x0A10C773` | Air Pollution Effect Strength Curve | 0x006E5B10 | curve(key,id) | empty (curve emptied with 0x005F4480 before the fetch) | f32 arr n=10: 0, 0, 0.005, 0.05, 0.01, 0.25, 0.02, 0.5, 0.03, 0.6 | pollution.md §7.1 |  |
| `E7E2C2DB/C911E35B` | `0x0A10C774` | Radiation Pollution Effect Strength Cur… | 0x006E5B10 | curve(key,id) | empty (curve emptied with 0x005F4480 before the fetch) | f32 arr n=6: 0, 0, 0.001, 1, 0.01, 1 | pollution.md §7.1 |  |
| `E7E2C2DB/C911E35B` | `0x0A10C775` | Garbage Pollution Effect Strength Curve | 0x006E5B10 | curve(key,id) | empty (curve emptied with 0x005F4480 before the fetch) | f32 arr n=16: 0, 0, 0.02, 0.02, 0.03, 0.06, 0.04, 0.2, 0.05, 0.4, 0.0… | pollution.md §7.1 |  |
| `E7E2C2DB/C911E35B` | `0x8C19C3E9` | Garbage Transport Versus Landfill Tiles | 0x006E5B10 | curve(key,id) | empty (curve emptied with 0x005F4480 before the fetch) | f32 arr n=4: 0, 0, 65536, 3.2768e+08 | pollution.md §7.1 |  |
| `E7E2C2DB/C911E35B` | `0x0911E2E8` | Funding percentage to decay rate multip… | 0x006E5B10 | curve(key,id) | (0,1)(1,1) built before the fetch (default pattern) | f32 arr n=6: 0, 1.1, 1, 1, 1.2, 0.9 | pollution.md §7.1 | same property also read by power and plumbing with the same default |
| `E7E2C2DB/C911E35B` | `0x0911E2E9` | Usage percentage to decay rate multipli… | 0x006E5B10 | curve(key,id) | (0,1)(1,1) built before the fetch (default pattern) | f32 arr n=14: 0, 0.1, 0.25, 0.2, 0.5, 0.45, 0.75, 0.85, 0.9, 1.25, 1… | pollution.md §7.1 | same property also read by power and plumbing with the same default |
| `E7E2C2DB/C911E35B` | `0x6911E2DF` | Power Funding To Efficiency Curve | 0x006E44B0, 0x006E7930 | curve(key,id) | empty (temporary curve) | f32 arr n=10: 0, 0, 50, 25, 90, 80, 100, 100, 120, 110 | pollution.md §4.7, §7.1 | the same property is power's funding->efficiency curve |
| `E7E2C2DB/C911E35B` | `0xA911E2F1` | Power radiation radius | 0x006ED1C0 | u32 | 2 (@0x006ED1FF, before the fetch) | u32 scl n=1: 2 | power.md §8 |  |
| `E7E2C2DB/C911E35B` | `0xE911E2FC` | Power plant max production ratio | 0x006ED1C0 | f32 | 1.1 (0x3F8CCCCD @0x006ED209) | f32 scl n=1: 110 | power.md §8 | dead per spec |
| `E7E2C2DB/C911E35B` | `0x29121E8B` | Power Min Acceptable Funding % | 0x006ED1C0 | f32 | 0.6 (0x3F19999A @0x006ED531) | f32 scl n=1: 60 | power.md §8 | dead per spec |
| `E7E2C2DB/C911E35B` | `0x6911E2DF` | Power Funding To Efficiency Curve | 0x006ED1C0 | curve(key,id) | (0,0)(100,100)(120,110) built before the fetch (default) | f32 arr n=10: 0, 0, 50, 25, 90, 80, 100, 100, 120, 110 | power.md §8 |  |
| `E7E2C2DB/C911E35B` | `0xE911E2D1` | Power max allowed funding | 0x006ED1C0 | f32 | 1.2 (0x3F99999A @0x006ED213) | f32 scl n=1: 120 | power.md §8 |  |
| `E7E2C2DB/C911E35B` | `0x09122027` | Utility Strike Efficiency % | 0x006ED1C0 | f32 | 0.5 (0x3F000000 @0x006ED516) | f32 scl n=1: 20 | power.md §8 |  |
| `E7E2C2DB/C911E35B` | `0x2912202E` | Utility Strike Duration | 0x006ED1C0 | s32 | 10 (@0x006ED520) | u32 scl n=1: 12 | power.md §8 | shipped u32; dead per spec; getter s32, shipped u32: converted by the variant |
| `E7E2C2DB/C911E35B` | `0x49122036` | Utility Min Funding to End Strike | 0x006ED1C0 | f32 | 0.9 (0x3F666666 @0x006ED527) | f32 scl n=1: 90 | power.md §8 | dead per spec |
| `E7E2C2DB/C911E35B` | `0x0911E2E6` | Utility Strike Response Curve | 0x006ED1C0 | curve(key,id) | (0,20)(100,100) (default) | f32 arr n=8: 0, 0, 20, 45, 50, 75, 100, 100 | power.md §8 | dead per spec |
| `E7E2C2DB/C911E35B` | `0x0911E2E8` | Funding percentage to decay rate multip… | 0x006ED1C0 | curve(key,id) | (0,1)(1,1) (default) | f32 arr n=6: 0, 1.1, 1, 1, 1.2, 0.9 | power.md §8 |  |
| `E7E2C2DB/C911E35B` | `0x0911E2E9` | Usage percentage to decay rate multipli… | 0x006ED1C0 | curve(key,id) | (0,1)(1,1) (default) | f32 arr n=14: 0, 0.1, 0.25, 0.2, 0.5, 0.45, 0.75, 0.85, 0.9, 1.25, 1… | power.md §8 |  |
| `E7E2C2DB/C911E35B` | `0xAA713782` | Funding percentage to power pole decay/… | 0x006ED1C0 | curve(key,id) | (0,-0.02)(120,0.01) (default; 0xBCA3D70A, 0x42F00000/0x3C23D70A) | f32 arr n=10: 0, -0.02, 25, -0.005, 50, 0, 100, 0.025, 110, 0.05 | power.md §8 |  |
| `E7E2C2DB/C911E35B` | `0x68F5436A` | Water pipe effect radius | 0x006DC9E0 | u32 | unchanged (reader sets none; spec ctor 7) | u32 scl n=1: 7 | plumbing.md §8 |  |
| `E7E2C2DB/C911E35B` | `0x68F5436B` | Fresh water pump reach | 0x006DC9E0 | u32 | unchanged (reader sets none; spec ctor 2) | u32 scl n=1: 256 | plumbing.md §8 |  |
| `E7E2C2DB/C911E35B` | `0x68F5436C` | Salt water pump reach | 0x006DC9E0 | u32 | unchanged (reader sets none; spec ctor 1) | u32 scl n=1: 256 | plumbing.md §8 |  |
| `E7E2C2DB/C911E35B` | `0x68F5436D` | Fresh water distance loss | 0x006DC9E0 | u32 | unchanged (reader sets none; spec ctor 20) | u32 scl n=1: 100 | plumbing.md §8 |  |
| `E7E2C2DB/C911E35B` | `0xAA161F81` | Max acceptable pumped water pollution l… | 0x006DC9E0 | u32 | unchanged (reader sets none; spec ctor 100) | u32 scl n=1: 512 | plumbing.md §8 |  |
| `E7E2C2DB/C911E35B` | `0x8A235C56` | Pipe burst probability | 0x006DC9E0 | f32 | unchanged (spec ctor 0.3) | f32 scl n=1: 0.03 | plumbing.md §8 |  |
| `E7E2C2DB/C911E35B` | `0xEA4025A9` | Water Department Max Funding | 0x006DC9E0 | numeric->f32 x0.01 | unchanged (spec ctor 1.2) | f32 scl n=1: 120 | plumbing.md §8 |  |
| `E7E2C2DB/C911E35B` | `0x8A84778F` | Monthly Cost Per Pipe | 0x006DC9E0 | f32 | unchanged (spec ctor 0) | f32 scl n=1: 0.1 | plumbing.md §8 |  |
| `E7E2C2DB/C911E35B` | `0x6A410BF2` | Water Funding To Efficiency Curve | 0x006DC9E0 | curve(key,id) | keep (untouched when the exemplar is missing); empty after load -> (0… | f32 arr n=10: 0, 0, 50, 30, 90, 80, 100, 100, 120, 110 | plumbing.md §8 |  |
| `E7E2C2DB/C911E35B` | `0x0911E2E8` | Funding percentage to decay rate multip… | 0x006DC9E0 | curve(key,id) | (0,1)(1,1) built first (default) | f32 arr n=6: 0, 1.1, 1, 1, 1.2, 0.9 | plumbing.md §8 |  |
| `E7E2C2DB/C911E35B` | `0x0911E2E9` | Usage percentage to decay rate multipli… | 0x006DC9E0 | curve(key,id) | (0,1)(1,1) built first (default) | f32 arr n=14: 0, 0.1, 0.25, 0.2, 0.5, 0.45, 0.75, 0.85, 0.9, 1.25, 1… | plumbing.md §8 |  |
| `E7E2C2DB/C911E35B` | `0xCA7138C4` | Funding percentage to pipe decay/repair | 0x006DC9E0 | curve(key,id) | (0,-0.02)(120,0.01) built first (default) | f32 arr n=10: 0, -0.01, 20, -0.005, 50, 0, 100, 0.02, 110, 0.05 | plumbing.md §8 |  |
| `E7E2C2DB/C8E8F6C5` | `0x47E2C200` | crime grid tract size (spec) | 0x006A07A0 | s32 | 2 (@0x006A07C1, before the fetch) | ABSENT | crime.md §8 | absent in shipped data; spec says u32, getter is s32 (converting); ABSENT in shipped data |
| `E7E2C2DB/C8E8F6C5` | `0x49A34301` | Crime Prop Request Curve | 0x006A07A0 | curve(key,id) | keep (no code default) | f32 arr n=10: 0, 0, 0.1, 2, 0.2, 4, 0.4, 6, 1, 10 | crime.md §8 |  |
| `E7E2C2DB/C8E8F6C5` | `0x8A0CBAAC` | Crime, Crime Grid Decay Factor | 0x006A07A0 | f32 | 0.5 (0x3F000000 @0x006A07C7) | f32 scl n=1: 0.8 | crime.md §8 |  |
| `E7E2C2DB/C8E8F6C5` | `0x8A0CBAAD` | Crime, Criminal Grid Decay Factor | 0x006A07A0 | f32 | 1.0 (@0x006A07CD) | f32 scl n=1: 0.66 | crime.md §8 |  |
| `E7E2C2DB/C8E8F6C5` | `0x8A0CBAA4` | Crime, R$: EQ -> Base Criminality Curve | 0x006A07A0 | curve(key,id) | (0,0.1)(255,0) (default pattern) | f32 arr n=6: 0, 0.4, 100, 0.3, 200, 0 | crime.md §8 | ID formed as edi-3 with edi = 0x8A0CBAA7+i (@0x006A09CF) |
| `E7E2C2DB/C8E8F6C5` | `0x8A0CBAA5` | Crime, R$$: EQ -> Base Criminality Curve | 0x006A07A0 | curve(key,id) | (0,0.1)(255,0) (default pattern) | f32 arr n=6: 0, 0.15, 100, 0.11, 200, 0 | crime.md §8 | ID formed as edi-3 with edi = 0x8A0CBAA7+i (@0x006A09CF) |
| `E7E2C2DB/C8E8F6C5` | `0x8A0CBAA6` | Crime, R$$$: EQ -> Base Criminality Cur… | 0x006A07A0 | curve(key,id) | (0,0.1)(255,0) (default pattern) | f32 arr n=6: 0, 0.1, 100, 0.07, 200, 0 | crime.md §8 | ID formed as edi-3 with edi = 0x8A0CBAA7+i (@0x006A09CF) |
| `E7E2C2DB/C8E8F6C5` | `0x8A0CBAA7` | Crime, R$: Jobless ratio -> Criminality… | 0x006A07A0 | curve(key,id) | (1,0)(2,0.1)(5,0.5) (default pattern) | f32 arr n=10: -1, 0.1, -0.7, 0.07, 0, 0, 0.7, -0.07, 1, -0.1 | crime.md §8 |  |
| `E7E2C2DB/C8E8F6C5` | `0x8A0CBAA8` | Crime, R$$: Jobless ratio -> Criminalit… | 0x006A07A0 | curve(key,id) | (1,0)(2,0.1)(5,0.5) (default pattern) | f32 arr n=10: -1, 0.07, -0.7, 0.05, 0, 0, 0.7, -0.05, 1, -0.07 | crime.md §8 |  |
| `E7E2C2DB/C8E8F6C5` | `0x8A0CBAA9` | Crime, R$$$: Jobless ratio -> Criminali… | 0x006A07A0 | curve(key,id) | (1,0)(2,0.1)(5,0.5) (default pattern) | f32 arr n=10: -1, 0.05, -0.7, 0.03, 0, 0, 0.7, -0.03, 1, -0.05 | crime.md §8 |  |
| `E7E2C2DB/C8E8F6C5` | `0x8A0CBAAA` | Crime, Criminality -> Crimes Curve | 0x006A07A0 | curve(key,id) | (0,0)(1e6,50000) (default; 0x49742400/0x47435000) | f32 arr n=18: 0, 0, 200, 1, 5000, 5, 10000, 10, 20000, 25, 200000, 40… | crime.md §8 |  |
| `E7E2C2DB/C8E8F6C5` | `0x8A0CBAAB` | Crime, Police Protection -> Crime Atten… | 0x006A07A0 | curve(key,id) | (0,0)(255,1) (default) | f32 arr n=8: 0, 0, 100, 0.05, 200, 0.2, 255, 0.5 | crime.md §8 |  |
| `E7E2C2DB/AA023079` | `0xEA023270` | Aura, Park Effect | 0x0068D770 | curve(prop*) | delete | f32 arr n=6: 0, 0, 50, 2, 100, 3 | aura.md §10 |  |
| `E7E2C2DB/AA023079` | `0xEA023275` | Aura, School Effect | 0x0068D770 | curve(prop*) | delete | f32 arr n=4: 0, -7, 1, 7 | aura.md §10 |  |
| `E7E2C2DB/AA023079` | `0xEA023276` | Aura, Hospital Effect | 0x0068D770 | curve(prop*) | delete | f32 arr n=4: 0, -7, 1, 7 | aura.md §10 |  |
| `E7E2C2DB/AA023079` | `0xEA023277` | Aura, Crime Effect | 0x0068D770 | curve(prop*) | delete | f32 arr n=12: 0, 10, 40, 5, 60, 2, 80, -3, 100, -5, 255, -50 | aura.md §10 |  |
| `E7E2C2DB/AA023079` | `0xEA023278` | Aura, Trip Length | 0x0068D770 | curve(prop*) | delete | f32 arr n=4: 0, 7, 255, -7 | aura.md §10 |  |
| `E7E2C2DB/AA023079` | `0xEA023279` | Aura, Traffic | 0x0068D770 | curve(prop*) | delete | f32 arr n=4: 0, 7, 255, -7 | aura.md §10 |  |
| `E7E2C2DB/AA023079` | `0xEA02327F` | Aura, Tax Rate Change Effect | 0x0068D770 | curve(prop*) | delete | f32 arr n=20: -20, 25, -10, 20, -5, 10, -2, 7, -1, 5, 1, -5, ... (20) | aura.md §10 |  |
| `E7E2C2DB/AA023079` | `0xEA023280` | Aura, Value/Wealth Effect | 0x0068D770 | curve(prop*) | delete | f32 arr n=10: -2, 5, -1, 3, 0, 0, 1, -3, 2, -5 | aura.md §10 |  |
| `E7E2C2DB/AA023079` | `0xEA023271` | Aura, Pollution Effect, Air | 0x0068D770 | curve(prop*) | delete | f32 arr n=12: 0, 10, 200, 7, 400, 0, 600, -10, 800, -20, 1024, -35 | aura.md §10 |  |
| `E7E2C2DB/AA023079` | `0xEA023272` | Aura, Pollution Effect, Water | 0x0068D770 | curve(prop*) | delete | f32 arr n=12: 0, 10, 200, 7, 400, 0, 600, -10, 800, -20, 1024, -35 | aura.md §10 |  |
| `E7E2C2DB/AA023079` | `0xEA023273` | Aura, Pollution Effect, Garbage | 0x0068D770 | curve(prop*) | delete | f32 arr n=12: 0, 10, 200, 7, 400, 3, 1800, -4, 2400, -16, 4096, -35 | aura.md §10 |  |
| `E7E2C2DB/AA023079` | `0xEA023274` | Aura, Pollution Effect, Radiation | 0x0068D770 | curve(prop*) | delete | f32 arr n=4: 0, 0, 1, -50 | aura.md §10 |  |
| `E7E2C2DB/AA023079` | `0xEA02327A` | Aura, Long Term Decay Factor | 0x0068D770 | f32 | 0.6 (0x3F19999A, written just before the read) | f32 scl n=1: 0.9 | aura.md §10 |  |
| `E7E2C2DB/AA023079` | `0xEA02327B` | Aura, Short Term Decay Factor | 0x0068D770 | f32 | 0.4 (0x3ECCCCCD) | f32 scl n=1: 0.75 | aura.md §10 |  |
| `E7E2C2DB/AA023079` | `0xEA02327C` | Aura, Base Aura (Low) | 0x0068D770 | s32 | unchanged | s32 scl n=1: 0 | aura.md §10 | copied into the static table at 0x00B0D150 afterwards |
| `E7E2C2DB/AA023079` | `0xEA02327D` | Aura, Base Aura (Med) | 0x0068D770 | s32 | unchanged | s32 scl n=1: 0 | aura.md §10 |  |
| `E7E2C2DB/AA023079` | `0xEA02327E` | Aura, Base Aura (High) | 0x0068D770 | s32 | unchanged | s32 scl n=1: 0 | aura.md §10 |  |
| `E7E2C2DB/AA023079` | `0xEA023290` | Aura, Transient Effect, Fire | 0x0068D770 | vec<f32> raw | {0,0} (both members zeroed before each read) | f32 arr n=2: -20, 128 | aura.md §10, §8.1 | spec types it s32[2] (converted); code and shipped data are float32 |
| `E7E2C2DB/AA023079` | `0xEA023291` | Aura, Transient Effect, Flora | 0x0068D770 | vec<f32> raw | {0,0} (both members zeroed before each read) | f32 arr n=2: 3, 32 | aura.md §10, §8.1 | spec types it s32[2] (converted); code and shipped data are float32 |
| `E7E2C2DB/AA023079` | `0xEA023292` | Aura, Transient Effect, Civic Plop | 0x0068D770 | vec<f32> raw | {0,0} (both members zeroed before each read) | f32 arr n=2: 25, 128 | aura.md §10, §8.1 | spec types it s32[2] (converted); code and shipped data are float32 |
| `E7E2C2DB/AA023079` | `0xEA023293` | Aura, Transient Effect, Network Plop | 0x0068D770 | vec<f32> raw | {0,0} (both members zeroed before each read) | f32 arr n=2: 0, 0 | aura.md §10, §8.1 | spec types it s32[2] (converted); code and shipped data are float32 |
| `E7E2C2DB/AA023079` | `0xEA0232A0` | Aura, Hotspot Settings | 0x0068D770 | vec<u32> raw | {196, 8, 16} written before the read (+0x13C=196, +0x138=8, +0x140=16) | s32 arr n=3: -50, 3, 4 | aura.md §10 | shipped s32 arr 3; raw copy, so signedness is preserved; raw/vector vec<u32> raw, shipped… |
| `E7E2C2DB/AA023079` | `0xEA0232A1` | Aura, Hotspot Riot Probability | 0x0068D770 | curve(key,id) | delete (absent) | f32 arr n=8: 0, 0, 5, 10, 10, 60, 20, 100 | aura.md §10 |  |
| `E7E2C2DB/68F5524A` | `0x29233E98` | FireProtectionSim, Strike Response Curve | 0x006ADDA0 | curve(key,id) | (0,20)(100,100) (default pattern) | f32 arr n=8: 0, 100, 25, 25, 50, 10, 70, 0 | protection.md §6.7 |  |
| `E7E2C2DB/68F5524A` | `0x29233E8D` | FireProtectionSim, Funding Efficiency C… | 0x006ADDA0 | curve(key,id) | (0,0)(100,100)(120,110) (default pattern) | f32 arr n=12: 0, 10, 50, 25, 75, 45, 90, 75, 100, 100, 120, 110 | protection.md §6.7 |  |
| `E7E2C2DB/68F5524A` | `0x29233E8F` | FireProtectionSim, Max Allowed Funding | 0x006ADDA0 | numeric->f32 x0.01 | 1.2 (0x3F99999A @0x006ADED8) | f32 scl n=1: 120 | protection.md §6.7 |  |
| `E7E2C2DB/68F5524A` | `0x29233E94` | FireProtectionSim, Strike Efficiency Mu… | 0x006ADDA0 | f32 | 0.5 (@0x006ADEDE) | f32 scl n=1: 0.2 | protection.md §6.7 |  |
| `E7E2C2DB/68F5524A` | `0x29233E96` | FireProtectionSim, Min Funding To End S… | 0x006ADDA0 | numeric->f32 x0.01 | 0.9 (0x3F666666 @0x006ADEA5) | f32 scl n=1: 90 | protection.md §6.7 |  |
| `E7E2C2DB/68F5524A` | `0x69B425DB` | FireProtectionSim, StationNoRoad Multip… | 0x006ADDA0 | f32 | 0.8 (0x3F4CCCCD @0x006ADEBE) | f32 scl n=1: 0.8 | protection.md §6.7 |  |
| `E7E2C2DB/68F5524A` | `0x29233E95` | FireProtectionSim, Max Strike Duration | 0x006ADDA0 | s32 | 60 (0x3C @0x006ADF75) | s32 scl n=1: 548 | protection.md §6.7 |  |
| `E7E2C2DB/28DFCF89` | `0x68DDAE92` | jail overflow factor (spec; unnamed) | 0x006E2110 | numeric->f32 x0.01 | 1.1 (0x3F8CCCCD @0x006E213C) | ABSENT | protection.md §6.7 | ABSENT in shipped data |
| `E7E2C2DB/28DFCF89` | `0x68DDAE93` | Police, Funding To Efficiency Response… | 0x006E2110 | curve(key,id) | (0,0)(100,100)(120,110) (default) | f32 arr n=6: 0, 0, 100, 100, 120, 110 | protection.md §6.7 |  |
| `E7E2C2DB/28DFCF89` | `0x68DDAEA0` | Jail, Funding To Efficiency Response Cu… | 0x006E2110 | curve(key,id) | (0,0)(100,100)(120,110) (default) | f32 arr n=8: 0, 0, 10, 50, 100, 100, 120, 110 | protection.md §6.7 |  |
| `E7E2C2DB/28DFCF89` | `0x68DDAE95` | Police, Max Allowed Funding Percentage | 0x006E2110 | numeric->f32 x0.01 | 1.2 (0x3F99999A @0x006E2236) | f32 scl n=1: 120 | protection.md §6.7 |  |
| `E7E2C2DB/28DFCF89` | `0x68DDAE97` | Police, Monthly Criminal Rehabilitation… | 0x006E2110 | numeric->f32 x0.01 | 0.01 (0x3C23D70A @0x006E2240) | f32 scl n=1: 10 | protection.md §6.7 |  |
| `E7E2C2DB/28DFCF89` | `0x68DDAE9A` | Police, Strike Efficiency | 0x006E2110 | numeric->f32 x0.01 | 0.5 (@0x006E224A) | f32 scl n=1: 20 | protection.md §6.7 |  |
| `E7E2C2DB/28DFCF89` | `0x68DDAE9B` | Police, Max Strike Duration | 0x006E2110 | s32 | 10 (@0x006E2254) | s32 scl n=1: 548 | protection.md §6.7 |  |
| `E7E2C2DB/28DFCF89` | `0x68DDAE9C` | Police, Min Funding To End Strikes | 0x006E2110 | numeric->f32 x0.01 | 0.9 (0x3F666666 @0x006E225E) | f32 scl n=1: 90 | protection.md §6.7 |  |
| `E7E2C2DB/28DFCF89` | `0x68DDAE9F` | Police, Arrest Threshold | 0x006E2110 | s32 | 16 (0x10 -> this+0x144 @0x006E2268) | u32 scl n=1: 20 | protection.md §6.7 | shipped u32, converted; getter s32, shipped u32: converted by the variant |
| `E7E2C2DB/28DFCF89` | `0x68DDAE9E` | Police, Strike Response Curve | 0x006E2110 | curve(key,id) | (0,20)(100,100) (default) | f32 arr n=8: 0, 100, 25, 25, 50, 10, 70, 0 | protection.md §6.7 |  |
| `E7E2C2DB/28DFCF89` | `0xEA614F63` | Jail, Jailbreak Response Curve | 0x006E2110 | curve(key,id) | (0,20)(100,100) (default) | f32 arr n=8: 0, 0, 80, 0, 90, 10, 100, 100 | protection.md §6.7 |  |
| `E7E2C2DB/28DFCF89` | `0xEA614F64` | Jail, Jailbreak Escapee Percentage | 0x006E2110 | f32 raw | 1.0, 5.0 (unscaled; written @0x006E23C0/@0x006E23CA) | f32 arr n=2: 10, 50 | protection.md §6.7 |  |
| `E7E2C2DB/28DFCF89` | `0xEA614F65` | Jail, End Jailbreak Capacity | 0x006E2110 | numeric->f32 x0.01 | 0.5 (@0x006E23B0) | f32 scl n=1: 80 | protection.md §6.7 |  |
| `7A4A8458/1A2FDB6A` | `0xC9DBDD00` | unnamed | 0x006B00F0 | f32 | unchanged (spec: rate 1.0) | f32 scl n=1: 1 | flora.md §7 |  |
| `7A4A8458/1A2FDB6A` | `0xC9DBDD01` | unnamed | 0x006B00F0 | f32 | unchanged (spec: 4.0) | f32 scl n=1: 1 | flora.md §7 |  |
| `7A4A8458/1A2FDB6A` | `0x8761ABD0` | unnamed | 0x006B00F0 | s32 | unchanged (spec: 6) | u32 scl n=1: 24 | flora.md §7 | spec says u32; getter is s32 (converting); getter s32, shipped u32: converted by the vari… |
| `7A4A8458/1A2FDB6A` | `0x085489C7` | unnamed | 0x006B00F0 | s32 | unchanged (spec: 5) | u32 scl n=1: 0 | flora.md §7 | spec says u32; getter is s32 (converting); getter s32, shipped u32: converted by the vari… |
| `7A4A8458/1A2FDB6A` | `0xC9DBDD02` | unnamed | 0x006B00F0 | f32 | unchanged (spec: 0.08) | f32 scl n=1: 800 | flora.md §7 |  |
| `E7E2C2DB/A8F41EE3` | `0xC9B40128` | Flammability multiplier, summer | 0x006AEB50 | f32 | 1.2 (0x3F99999A @0x006AEB7B) | f32 scl n=1: 1 | flammability.md §8 |  |
| `E7E2C2DB/A8F41EE3` | `0x29B40134` | Flammability multiplier, abandonment | 0x006AEB50 | f32 | 1.1 (@0x006AEB87) | f32 scl n=1: 1.5 | flammability.md §8 |  |
| `E7E2C2DB/A8F41EE3` | `0x29B4013B` | Flammability multiplier, no water | 0x006AEB50 | f32 | 1.1 (@0x006AEB8E) | f32 scl n=1: 1.25 | flammability.md §8 |  |
| `E7E2C2DB/A8F41EE3` | `0x29B4013C` | Flammability, hotspot settings | 0x006AEB50 | vec<u32> raw | {196, 8, 16} (+0x9C=196, +0x98=8, +0xA0=16) | u32 arr n=3: 54, 8, 40 | flammability.md §8 |  |
| `E7E2C2DB/A8F41EE3` | `0x29B4013E` | Flammability, random fire probability | 0x006AEB50 | f32 | 0.01 (0x3C23D70A @0x006AEB98) | f32 scl n=1: 5 | flammability.md §8 |  |
| `E7E2C2DB/A8F41EE3` | `0x29B4013D` | Flammability, hotspot fire probability | 0x006AEB50 | curve(key,id) | delete (absent) | f32 arr n=12: 0, 0, 39, 0, 40, 7, 50, 15, 70, 20, 100, 25 | flammability.md §8 |  |
| `E7E2C2DB/69189C84` | `0xC929C433` | EQ Decay | 0x006FC0C0 | f32 | 1.0 (@0x006FC12E) | f32 scl n=1: 0.2 | residential.md §7.1; REH §4.13 |  |
| `E7E2C2DB/69189C84` | `0x0929C439` | HQ Decay | 0x006FC0C0 | f32 | 1.0 | f32 scl n=1: 0.2 | residential.md §7.1; REH §4.13 |  |
| `E7E2C2DB/69189C84` | `0xC929C652` | Low Wealth Average Age | 0x006FC0C0, 0x006F88B0 | s16/u8 | 0x006FC0C0: 25; 0x006F88B0: local {25,30,25} (0x19,0x1E,0x19 @0x006F8… | u8 scl n=1: 25 | residential.md §7.1, §6.5 | two readers with different getter types (s16 / u8); shipped u8 (both getters convert); ge… |
| `E7E2C2DB/69189C84` | `0x0929CC30` | Medium Wealth Average Age | 0x006FC0C0, 0x006F88B0 | s16/u8 | 0x006FC0C0: 30 (@0x006FC146); 0x006F88B0: local {25,30,25} (0x19,0x1E… | u8 scl n=1: 30 | residential.md §7.1, §6.5 | two readers with different getter types (s16 / u8); shipped u8 (both getters convert); ge… |
| `E7E2C2DB/69189C84` | `0x0929CC36` | High Wealth Average Age | 0x006FC0C0, 0x006F88B0 | s16/u8 | 0x006FC0C0: 35 (@0x006FC14C); 0x006F88B0: local {25,30,25} (0x19,0x1E… | u8 scl n=1: 35 | residential.md §7.1, §6.5 | two readers with different getter types (s16 / u8); shipped u8 (both getters convert); ge… |
| `E7E2C2DB/69189C84` | `0xCA027307` | Average Age reset value | 0x006FC0C0 | u8 | 40 (@0x006FC152) | u8 scl n=1: 40 | residential.md §7.1; REH §4.13 |  |
| `E7E2C2DB/69189C84` | `0xCA027325` | EQ reset percent | 0x006FC0C0 | numeric->f32 x0.01 | 0.7 (0x3F333333 @0x006FC156) | f32 scl n=1: 80 | residential.md §7.1; REH §4.13 |  |
| `E7E2C2DB/69189C84` | `0xCA027328` | HQ reset percent | 0x006FC0C0 | numeric->f32 x0.01 | 0.7 (@0x006FC15D) | f32 scl n=1: 80 | residential.md §7.1; REH §4.13 |  |
| `E7E2C2DB/69189C84` | `0xC929CC58` | Low Wealth EQ | 0x006FC0C0 | f32 | 4.0 (@0x006FC164) | u8 scl n=1: 20 | residential.md §7.1; REH §4.13 | shipped u8, converted; getter f32, shipped u8: converted by the variant |
| `E7E2C2DB/69189C84` | `0x0929CC65` | Medium Wealth EQ | 0x006FC0C0 | f32 | 6.0 (@0x006FC16B) | u8 scl n=1: 40 | residential.md §7.1; REH §4.13 | shipped u8, converted; getter f32, shipped u8: converted by the variant |
| `E7E2C2DB/69189C84` | `0x2929CC6B` | High Wealth EQ | 0x006FC0C0 | f32 | 8.0 (@0x006FC172) | u8 scl n=1: 60 | residential.md §7.1; REH §4.13 | shipped u8, converted; getter f32, shipped u8: converted by the variant |
| `E7E2C2DB/69189C84` | `0xC929CC8D` | Low Wealth HQ | 0x006FC0C0 | f32 | none (never written: uninitialised) | u8 scl n=1: 20 | residential.md §7.1; REH §4.13 | shipped u8, converted; getter f32, shipped u8: converted by the variant |
| `E7E2C2DB/69189C84` | `0x6929CC92` | Medium Wealth HQ | 0x006FC0C0 | f32 | none (uninitialised) | u8 scl n=1: 40 | residential.md §7.1; REH §4.13 | shipped u8, converted; getter f32, shipped u8: converted by the variant |
| `E7E2C2DB/69189C84` | `0x4929CC97` | High Wealth HQ | 0x006FC0C0 | f32 | none (uninitialised) | u8 scl n=1: 60 | residential.md §7.1; REH §4.13 | shipped u8, converted; getter f32, shipped u8: converted by the variant |
| `E7E2C2DB/69189C84` | `0x6AB70DA7` | Death's door HQ | 0x006FC0C0 | f32 | 0 (@0x006FC179) | f32 scl n=1: 2 | residential.md §7.1; REH §4.13 |  |
| `E7E2C2DB/69189C84` | `0x6AB71654` | Death's door effect height | 0x006FC0C0 | f32 | 0 | f32 scl n=1: 50 | residential.md §7.1; REH §4.13 |  |
| `E7E2C2DB/69189C84` | `0x4AB71659` | Death's door effect height delta | 0x006FC0C0 | f32 | 0 | f32 scl n=1: 10 | residential.md §7.1; REH §4.13 |  |
| `E7E2C2DB/69189C84` | `0x4AB70DAC` | Death's door effect chance | 0x006FC0C0 | f32 | 0 | f32 scl n=1: 1.5 | residential.md §7.1; REH §4.13 |  |
| `E7E2C2DB/69189C84` | `0x6941B022` | Air Pollution to HQ Decay Curve | 0x006FC0C0 | curve(key,id) | (0,0)(32000,100) (0x46FA0000) if the exemplar exists; keep if it is m… | f32 arr n=6: 0, 0, 512, 0.1, 1024, 50 | residential.md §7.1; REH §4.13 |  |
| `E7E2C2DB/69189C84` | `0xA941B028` | Water Pollution to HQ Decay Curve | 0x006FC0C0 | curve(key,id) | (0,0)(32000,100) (0x46FA0000) if the exemplar exists; keep if it is m… | f32 arr n=6: 0, 0, 512, 0.1, 1024, 50 | residential.md §7.1; REH §4.13 |  |
| `E7E2C2DB/69189C84` | `0xC941B02E` | Garbage Pollution to HQ Decay Curve | 0x006FC0C0 | curve(key,id) | (0,0)(32000,100) (0x46FA0000) if the exemplar exists; keep if it is m… | f32 arr n=6: 0, 0, 512, 0.2, 1024, 20 | residential.md §7.1; REH §4.13 |  |
| `E7E2C2DB/69189C84` | `0xE918946F` | (OBS) Health Min Acceptable Funding % (… | 0x006FBAB0 | f32 | 0.6 (0x3F19999A @0x006FBC8C) | ABSENT | residential.md §7.2; REH §4.13 | spec: shipped absent, value never used; ABSENT in shipped data |
| `E7E2C2DB/69189C84` | `0x2918949E` | Health Funding To Efficiency Curve | 0x006FBAB0 | curve(key,id) | (0,0)(100,100)(120,110) (default) | f32 arr n=6: 0, 0, 100, 100, 120, 110 | residential.md §7.2; REH §4.13 |  |
| `E7E2C2DB/69189C84` | `0x0A53B2F9` | Health Funding to Coverage Curve | 0x006FBAB0 | curve(key,id) | (0,20)(100,100) (default) | f32 arr n=4: 0, 50, 100, 100 | residential.md §7.2; REH §4.13 |  |
| `E7E2C2DB/69189C84` | `0xA9189479` | Health Init/Max Allowed Funding | 0x006FBAB0 | vec<f32> raw | 0.25 (0x3E800000 @0x006FBCA0), 1.2 | f32 arr n=2: 25, 120 | residential.md §7.2; REH §4.13 |  |
| `E7E2C2DB/69189C84` | `0x89189482` | Health Strike Efficiency % | 0x006FBAB0 | f32 | 0.2 (0x3E4CCCCD @0x006FBC6E) | f32 scl n=1: 20 | residential.md §7.2; REH §4.13 |  |
| `E7E2C2DB/69189C84` | `0x8918948B` | Health Strike Duration | 0x006FBAB0 | s32 | 10 (spec; small immediate not re-checked) | u32 scl n=1: 500 | residential.md §7.2; REH §4.13 | shipped u32, converted; getter s32, shipped u32: converted by the variant |
| `E7E2C2DB/69189C84` | `0x89189495` | Health Min Grade to End Strike | 0x006FBAB0 | numeric->f32 x0.01 | 0.9 (0x3F666666 @0x006FBC82) | f32 scl n=1: 90 | residential.md §7.2; REH §4.13 |  |
| `E7E2C2DB/69189C84` | `0x691894A4` | Health Strike Response Curve | 0x006FBAB0 | curve(key,id) | (0,20)(100,100) (default) | f32 arr n=4: 0, 100, 80, 0 | residential.md §7.2; REH §4.13 |  |
| `E7E2C2DB/69189C84` | `0xC91894A8` | (OBS) School Min Acceptable Funding % (… | 0x006FBAB0 | f32 | 0.6 (@0x006FBD15) | ABSENT | residential.md §7.2; REH §4.13 | spec: shipped absent, value never used; ABSENT in shipped data |
| `E7E2C2DB/69189C84` | `0x691894C6` | School Funding To Efficiency Curve | 0x006FBAB0 | curve(key,id) | (0,0)(100,100)(120,110) (default) | f32 arr n=6: 0, 0, 100, 100, 120, 110 | residential.md §7.2; REH §4.13 |  |
| `E7E2C2DB/69189C84` | `0xEA53B30F` | School Funding to Coverage Curve | 0x006FBAB0 | curve(key,id) | (0,20)(100,100) (default) | f32 arr n=6: 0, 50, 100, 100, 120, 110 | residential.md §7.2; REH §4.13 |  |
| `E7E2C2DB/69189C84` | `0x891894AD` | School Init/Max Allowed Funding | 0x006FBAB0 | vec<f32> raw | 0.25 (@0x006FBD29), 1.2 | f32 arr n=2: 25, 120 | residential.md §7.2; REH §4.13 |  |
| `E7E2C2DB/69189C84` | `0xE91894B3` | School Strike Efficiency % | 0x006FBAB0 | f32 | 0.2 (@0x006FBCF7) | f32 scl n=1: 20 | residential.md §7.2; REH §4.13 |  |
| `E7E2C2DB/69189C84` | `0xA91894BC` | School Strike Duration | 0x006FBAB0 | s32 | 10 (spec) | u32 scl n=1: 500 | residential.md §7.2; REH §4.13 | shipped u32, converted; getter s32, shipped u32: converted by the variant |
| `E7E2C2DB/69189C84` | `0x691894C0` | School Min Grade to End Strike | 0x006FBAB0 | numeric->f32 x0.01 | 0.9 (@0x006FBD0B) | f32 scl n=1: 90 | residential.md §7.2; REH §4.13 |  |
| `E7E2C2DB/69189C84` | `0xC91894CB` | School Strike Response Curve | 0x006FBAB0 | curve(key,id) | (0,20)(100,100) (default) | f32 arr n=4: 0, 100, 80, 0 | residential.md §7.2; REH §4.13 |  |
| `E7E2C2DB/69189C84` | `0xEA540621` | Avg. Age to 1-10 Age Group | 0x006FBAB0 | curve(key,id) | (0,0)(100,100)(120,110) (L1 default) | f32 arr n=10: 20, 24, 30, 6, 40, 9, 50, 4, 60, 0 | residential.md §7.2, §5.6 |  |
| `E7E2C2DB/69189C84` | `0xEA540622` | Avg. Age to 11-20 Age Group | 0x006FBAB0 | curve(key,id) | (0,0)(100,100)(120,110) (L1 default) | f32 arr n=10: 20, 28, 30, 24, 40, 12, 50, 6, 60, 0 | residential.md §7.2, §5.6 |  |
| `E7E2C2DB/69189C84` | `0xEA540623` | Avg. Age to 21-30 Age Group | 0x006FBAB0 | curve(key,id) | (0,0)(100,100)(120,110) (L1 default) | f32 arr n=10: 20, 31, 30, 27, 40, 12, 50, 6, 60, 0 | residential.md §7.2, §5.6 |  |
| `E7E2C2DB/69189C84` | `0xEA540624` | Avg. Age to 31-40 Age Group | 0x006FBAB0 | curve(key,id) | (0,0)(100,100)(120,110) (L1 default) | f32 arr n=10: 20, 12, 30, 20, 40, 13, 50, 8, 60, 3 | residential.md §7.2, §5.6 |  |
| `E7E2C2DB/69189C84` | `0xEA540625` | Avg. Age to 41-50 Age Group | 0x006FBAB0 | curve(key,id) | (0,0)(100,100)(120,110) (L1 default) | f32 arr n=10: 20, 5, 30, 12, 40, 23, 50, 19, 60, 13 | residential.md §7.2, §5.6 |  |
| `E7E2C2DB/69189C84` | `0xEA540626` | Avg. Age to 51-60 Age Group | 0x006FBAB0 | curve(key,id) | (0,0)(100,100)(120,110) (L1 default) | f32 arr n=10: 20, 0, 30, 7, 40, 14, 50, 27, 60, 39 | residential.md §7.2, §5.6 |  |
| `E7E2C2DB/69189C84` | `0xEA540627` | Avg. Age to 61-70 Age Group | 0x006FBAB0 | curve(key,id) | (0,0)(100,100)(120,110) (L1 default) | f32 arr n=10: 20, 0, 30, 4, 40, 11, 50, 20, 60, 31 | residential.md §7.2, §5.6 |  |
| `E7E2C2DB/69189C84` | `0xEA540628` | Avg. Age to 71-80 Age Group | 0x006FBAB0 | curve(key,id) | (0,0)(100,100)(120,110) (L1 default) | f32 arr n=10: 20, 0, 30, 0, 40, 5, 50, 8, 60, 11 | residential.md §7.2, §5.6 |  |
| `E7E2C2DB/69189C84` | `0xEA540629` | Avg. Age to 81-90 Age Group | 0x006FBAB0 | curve(key,id) | (0,0)(100,100)(120,110) (L1 default) | f32 arr n=10: 20, 0, 30, 0, 40, 1, 50, 2, 60, 3 | residential.md §7.2, §5.6 |  |
| `E7E2C2DB/69189C84` | `0x8A891421` | School Capacity To Staff Divisor | 0x006FBAB0 | u32 | 30 (written in the found branch only, @0x006FBFCB) | u32 scl n=1: 30 | residential.md §7.2; REH §4.13 |  |
| `E7E2C2DB/69189C84` | `0x8A89142A` | Hospital Capacity To Staff Divisor | 0x006FBAB0 | u32 | 30 (found branch only) | u32 scl n=1: 30 | residential.md §7.2; REH §4.13 |  |
| `E7E2C2DB/69189C84` | `0x6A9F6EDD` | Coverage radius slop multiplier | 0x006FBAB0 | f32 | 1.1 (found branch only, @0x006FBFF6) | f32 scl n=1: 1.1 | residential.md §7.2; REH §4.13 |  |
| `E7E2C2DB/69189C84` | `0x4AA84161` | Maximum EQ Boost | 0x006FBAB0 | f32 | 10.0 (found branch only) | f32 scl n=1: 1.3 | residential.md §7.2; REH §4.13 |  |
| `E7E2C2DB/69189C84` | `0x2AA84164` | Maximum HQ Boost | 0x006FBAB0 | f32 | 10.0 (found branch only) | f32 scl n=1: 1.1 | residential.md §7.2; REH §4.13 |  |
| `E7E2C2DB/69189C84` | `0xEC43826B` | School Capacity Chunk Size | 0x006FBAB0 | vec<u32> raw | none (vector untouched) | u32 arr n=7: 500, 750, 1000, 1000, 10000, 300, 1200 | residential.md §7.2; REH §4.13 |  |
| `E7E2C2DB/69189C84` | `0xAC43826D` | Hospital Capacity Chunk Size | 0x006FBAB0 | vec<u32> raw | none | u32 arr n=3: 500, 500, 2000 | residential.md §7.2; REH §4.13 |  |
| `E7E2C2DB/69189C84` | `0x6941A67F` | Health Quotient to Life Expectancy Curve | 0x006FC570, 0x006FC9E0 | curve(key,id) | (0,0)(200,200) (0x43480000); missing exemplar -> query returns 0 | f32 arr n=10: 0, 50, 20, 55, 40, 60, 60, 65, 200, 100 | residential.md §7.3; REH §4.13 |  |
| `E7E2C2DB/69189C84` | `0x4941A92B` | Life Expectancy to Workforce % Curve | 0x006FC6E0, 0x006FC860 | curve(key,id) | (0,0)(200,100); missing exemplar -> 0.0 | f32 arr n=4: 50, 40, 100, 60 | residential.md §7.3; REH §4.13 |  |
| `7A4A8458/6A469021` | `0x0A469353` | unnamed | 0x006D38F0 | curve(prop*) | no curve (curve untouched) | f32 arr n=8: 0, 1, 0.1, 0.3, 0.3, 0.1, 1, 0 | neighbors.md §12, §8.3 |  |
| `7A4A8458/6A469021` | `0x4B61FFC9` | unnamed | 0x006D2A20 | curve(prop*) | curve unchanged | f32 arr n=6: 0, 1e-05, 0.0075, 0.0075, 1, 0.075 | neighbors.md §12, §8.3; budget.md §13.2 | overwrites the scarcity curve from the first month on (spec documents this) |
| `7A4A8458/6A469021` | `0xEA46932C` | unnamed | 0x006D21A0, 0x006D3250… | f32 | 0 (spec) | f32 scl n=1: 1000 | neighbors.md §12 |  |
| `7A4A8458/6A469021` | `0x6A469337` | unnamed | 0x006D21A0, 0x006D3250… | f32 | 0 (spec) | f32 scl n=1: 1000 | neighbors.md §12 |  |
| `7A4A8458/6A469021` | `0x8A46933D` | unnamed | 0x006D21A0, 0x006D3250… | f32 | 0 (spec) | f32 scl n=1: 1000 | neighbors.md §12 |  |
| `7A4A8458/6A469021` | `0xEA4AD2FF` | unnamed | 0x006D3250, 0x006D3620 | f32 | 1.0 (0x3F800000 @0x006D3291) | f32 scl n=1: 2.5 | neighbors.md §12 |  |
| `7A4A8458/6A469021` | `0x6A469342` | unnamed | 0x006D3250 | f64 | 1.0 (double) | f32 scl n=1: 0.1 | neighbors.md §12 | spec says f32; getter is f64 (shipped f32 converted); getter f64, shipped f32: converted… |
| `7A4A8458/6A469021` | `0xCA469346` | unnamed | 0x006D21A0, 0x006D3250 | f32/f64 | 1.0 (f32 0x3F800000 @0x006D22AC; double 1.0 in GetDealCost) | f32 scl n=1: 0.5 | neighbors.md §12 | two readers with different getter types (f32 / f64); getter f64, shipped f32: converted b… |
| `7A4A8458/6A469021` | `0x6B588FAD` | unnamed | 0x006D0E50 | f32 | 0 (spec) | f32 scl n=1: 90 | neighbors.md §12 |  |
| `7A4A8458/6A469021` | `0x8B58C59E` | unnamed | 0x006D0E50 | f32 | 0 (written @0x006D12BC) | f32 scl n=1: 90 | neighbors.md §12 |  |
| `7A4A8458/6A469021` | `0xEA46934F` | unnamed | 0x006D0520, 0x006D0E50… | f32 | 5 in 0x006D0520, 0 in 0x006D0E50, unset in 0x006D2A20 (spec) | f32 scl n=1: 1 | neighbors.md §12 | three readers, three defaults |
| `7A4A8458/6A469021` | `0xAA46934B` | unnamed | 0x006D0520 | f32 | 1 (spec) | f32 scl n=1: 1 | neighbors.md §12 |  |
| `7A4A8458/6A469021` | `0xCA46931F` | unnamed | 0x006D21A0 | f32 | 1.0 (0x3F800000 @0x006D22E6) | f32 scl n=1: 60 | neighbors.md §12 |  |
| `7A4A8458/6A469021` | `0xEA4AE9EF` | unnamed | 0x006D21A0 | f32 | 0 (spec; local zeroed with edi) | f32 scl n=1: 5000 | neighbors.md §12 |  |
| `67CD5FA1/6A2806AF` | `0x4A280510` | unnamed | 0x006B8C70 | f32 | 0.1 (.data) | f32 scl n=1: 0.07 | landfill.md §4.1 |  |
| `67CD5FA1/6A2806AF` | `0x4A280513` | unnamed | 0x006B8C70 | f32 | 1.0 (.data) | f32 scl n=1: 5 | landfill.md §4.1 |  |
| `67CD5FA1/6A2806AF` | `0x4A677E5B` | unnamed | 0x006B8C70 | u32 | 40000 (.data) | u32 scl n=1: 327680000 | landfill.md §4.1 |  |
| `67CD5FA1/6A2806AF` | `0x4A677E5C` | unnamed | 0x006B8C70 | u32 | 5000 (.data) | u32 scl n=1: 5000 | landfill.md §4.1 |  |
| `67CD5FA1/6A2806AF` | `0x4A677E5E` | unnamed | 0x006B8C70 | u32 | 3000 (.data) | u32 scl n=1: 1500 | landfill.md §4.1 |  |
| `67CD5FA1/6A2806AF` | `0x4A677E5F` | unnamed | 0x006B8C70 | s64 | 0 (.bss) | s64 scl n=1: 5 | landfill.md §4.1 |  |
| `67CD5FA1/6A2806AF` | `0x2AA00FE7` | unnamed | 0x006B8C70 | f32 | 1.0 (.data) | s64 scl n=1: 4 | landfill.md §4.1 | shipped s64 converted to float (spec agrees); getter f32, shipped s64: converted by the v… |
| `67CD5FA1/6A2806AF` | `0x4A677E5D` | unnamed | 0x006B8C70 | bool | true (.data byte 1) | bool scl n=1: 1 | landfill.md §4.1 |  |
| `E7E2C2DB/E8DA7677` | `0x47E2C380` | Construction Resources Min/Max | 0x006959E0 | u32 raw | 100, 2000 (written before the fetch @0x00695A05) | u32 arr n=2: 8192, 20480 | building-development.md §4.13 |  |
| `E7E2C2DB/E8DA7677` | `0x47E2C381` | (unnamed; per-resident factor) (spec) | 0x006959E0 | f32 | 0.5 (0x3F000000 @0x00695A19) | ABSENT | building-development.md §4.13 | ABSENT in shipped data |
| `E7E2C2DB/E8DA7677` | `0x47E2C382` | (unnamed; bias) (spec) | 0x006959E0 | u32 | 10 (@0x00695A1F) | ABSENT | building-development.md §4.13 | ABSENT in shipped data |
| `E7E2C2DB/E8DA7677` | `0x47E2C383` | Stage Popluation Multiplier | 0x006959E0 | vec<f32> raw | unchanged (empty from ctor per spec) | f32 arr n=3: 1.2, 1.1, 1 | building-development.md §4.13 |  |
| `E7E2C2DB/E8DA7677` | `0x47E2C400` | Lot Developer Stay New Time | 0x006C4A20 | u16 | unchanged (spec ctor 90) | u16 scl n=1: 90 | lot-developer.md §3.5 |  |
| `E7E2C2DB/E8DA7677` | `0x47E2C401` | Lot Developer Occupancy Thresholds | 0x006C4A20 | f32 raw | 0.2, 0.4, 0.6 (written after the fetch, before the read @0x006C4A85..… | f32 arr n=3: 0.3, 0.45, 0.5 | lot-developer.md §3.5 |  |
| `E7E2C2DB/E8DA7677` | `0xE900EA24` | kMaxCellAltitudeDeltaForFlatnessProperty | 0x006C4A20 | f32 | unchanged (spec ctor 1.0) | f32 scl n=1: 1 | lot-developer.md §3.5 |  |
| `E7E2C2DB/E8DA7677` | `0xE900EA25` | kMaxSlopeForFlatnessProperty | 0x006C4A20 | f32 | unchanged (spec ctor 1.0) | f32 scl n=1: 1 | lot-developer.md §3.5 |  |
| `E7E2C2DB/E8DA7677` | `0xE900EA26` | kDefaultLotFoundationTypeProperty | 0x006C4A20 | u32 | unchanged (spec ctor 0x890B7312) | u32 scl n=1: 2299228946 | lot-developer.md §3.5 |  |
| `E7E2C2DB/E8DA7677` | `0x6916516B` | (unnamed) (spec) | 0x006C4A20 | u32 | unchanged (spec ctor 0x890B7312) | ABSENT | lot-developer.md §3.5 | ABSENT in shipped data |
| `E7E2C2DB/E8DA7677` | `0xE900EA27` | kShouldAttemptLotLevellingProperty | 0x006C4A20 | bool | unchanged (spec ctor true) | u32 scl n=1: 1 | lot-developer.md §3.5 | shipped u32, read as bool (converted); getter bool, shipped u32: converted by the variant |
| `E7E2C2DB/E8DA7677` | `0xE900EA28` | kShouldAttemptInternalLotLevellingPrope… | 0x006C4A20 | bool | unchanged (spec ctor true) | u32 scl n=1: 1 | lot-developer.md §3.5 | shipped u32, read as bool (converted); getter bool, shipped u32: converted by the variant |
| `E7E2C2DB/E8DA7677` | `0xE900EA29` | kLotAutoLevelInternalAltitudeDeltaPrope… | 0x006C4A20 | f32 | unchanged (spec ctor 0.0) | f32 scl n=1: 20 | lot-developer.md §3.5 |  |
| `E7E2C2DB/E8DA7677` | `0xE900EA2A` | MaxAreaSlopeAllowedForLevellingProperty | 0x006C4A20 | f32 | unchanged (spec ctor 0.0) | f32 scl n=1: 40 | lot-developer.md §3.5 |  |
| `E7E2C2DB/E8DA7677` | `0xE900EA2B` | Lot Developer: Night Update Interval | 0x006C4A20 | u32 | unchanged (spec ctor 200) | u32 scl n=1: 100 | lot-developer.md §3.5 |  |
| `E7E2C2DB/E8DA7677` | `0xE900EA2C` | Lot Developer: Night Update Count | 0x006C4A20 | u32 | unchanged (spec ctor 8) | u32 scl n=1: 10 | lot-developer.md §3.5 |  |
| `E7E2C2DB/E8DA7677` | `0x2A2D7824` | Lot Developer: Cost Multiplier vs lot s… | 0x006C4A20 | curve(prop*) | keep | f32 arr n=20: 0, 0, 0.555556, 10, 1.111111, 20, 1.666667, 40, 2.22222… | lot-developer.md §3.5 |  |
| `E7E2C2DB/E8DA7677` | `0x47E2C500` | Tract Developer Overbuild | 0x00706FC0 | f32 | 1.5 (@0x0070701A); missing exemplar: nothing written | f32 scl n=1: 1.5 | tract-developer.md §4.21 |  |
| `E7E2C2DB/E8DA7677` | `0x47E2C501` | Tract Developer Reoccupy Thresholds | 0x00706FC0 | f32 raw | {0, 1.2} (@0x00707028/2F); missing exemplar: nothing written | f32 arr n=2: 0, 1.2 | tract-developer.md §4.21 |  |
| `E7E2C2DB/E8DA7677` | `0x47E2C502` | Tract Developer Redevelop Size Threshold | 0x00706FC0 | f32 | 1.5 (@0x0070706C); missing exemplar: nothing written | f32 scl n=1: 1.5 | tract-developer.md §4.21 |  |
| `E7E2C2DB/E8DA7677` | `0x47E2C510` | Tract Developer Demolition Costs | 0x00706FC0 | f32 raw | {1, 0.2} (@0x0070707A/81); missing exemplar: nothing written | f32 arr n=2: 1, 0.2 | tract-developer.md §4.21 |  |
| `E7E2C2DB/E8DA7677` | `0x47E2C511` | Tract Developer New Lot Costs | 0x00706FC0 | f32 raw | {10, 5} (@0x007070B4/BB); missing exemplar: nothing written | f32 arr n=2: 1, 1 | tract-developer.md §4.21 |  |
| `E7E2C2DB/E8DA7677` | `0x47E2C530` | Tract Developer Occupancy Variation | 0x00706FC0 | f32 | 0.2 (@0x007070F8); missing exemplar: nothing written | f32 scl n=1: 0 | tract-developer.md §4.21 |  |
| `E7E2C2DB/E8DA7677` | `0x47E2C531` | Tract Developer Min Vacancy Range | 0x00706FC0 | f32 raw | {0.01, 0.1} (@0x00707106/0D); missing exemplar: nothing written | f32 arr n=2: 0.01, 0.1 | tract-developer.md §4.21 |  |
| `E7E2C2DB/E8DA7677` | `0x47E2C540` | Tract Developer Kick Out Lower Wealth | 0x00706FC0 | bool | true (@0x0070714C); missing exemplar: nothing written | bool scl n=1: 1 | tract-developer.md §4.21 |  |
| `E7E2C2DB/E8DA7677` | `0x47E2C550` | Tract Developer Available Styles | 0x00706FC0 | u32 raw | empty; missing exemplar: nothing written | u32 arr n=4: 8192, 8193, 8194, 8195 | tract-developer.md §4.21 |  |
| `E7E2C2DB/E8DA7677` | `0x47E2C551` | Tract Developer Years Between Styles | 0x00706FC0 | s32 | 40 (0x28 @0x0070721C, only when the field is 0); missing exemplar: no… | s32 scl n=1: 5 | tract-developer.md §4.21 |  |
| `E7E2C2DB/E9482490` | `0xE94825B2` | kZM_mZoneMinCellSize | 0x004DC460 | s32 raw | unchanged | s32 arr n=16: 0, 1, 1, 1, 1, 1, 1, 4, 1, 1, 1, 1, ... (16) | zones.md §4 |  |
| `E7E2C2DB/E9482490` | `0xE94825B3` | kZM_mZoneMaxCellSize | 0x004DC460 | s32 raw | unchanged | s32 arr n=16: 32768, 32, 32, 32, 32, 32, 32, 48, 32, 32, 32, 32, ...… | zones.md §4 |  |
| `E7E2C2DB/E9482490` | `0xE94825B9` | Zone Max Slope | 0x004DC460 | f32 raw | unchanged | f32 arr n=16: 256, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, ... (1… | zones.md §4 |  |
| `E7E2C2DB/E9482490` | `0xE94825B4` | kZM_mZoneCreationCostArray | 0x004DC460 | s64 raw | 16 x 0 (zeroed before the fetch) | s64 arr n=16: 0, 10, 20, 50, 10, 20, 50, 10, 20, 50, 1, 1, ... (16) | zones.md §4 |  |
| `E7E2C2DB/E9482490` | `0xE94825B5` | kZM_mZoneDestructionCostArray | 0x004DC460 | s64 raw | 16 x 0 | s64 arr n=16: 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, ... (16) | zones.md §4 |  |
| `E7E2C2DB/E9482490` | `0xE94825B6` | kZM_mZoneTextureResourceInstanceArray | 0x004DC460 | u32 raw | unchanged | u32 arr n=16: 4121, 4117, 4118, 4116, 4099, 4100, 4098, 4112, 4113, 4… | zones.md §4 |  |
| `E7E2C2DB/E9482490` | `0xE94825B7` | Zone Display Alpha | 0x004DC460 | u8 | unchanged (spec 0xFF) | u8 scl n=1: 170 | zones.md §4 |  |
| `E7E2C2DB/E9482490` | `0xE94825B8` | Zone Drag Color | 0x004DC460 | u32 raw | unchanged | u32 arr n=16: 4278190335, 12845311, 10092799, 7471359, 1333264383, 54… | zones.md §4 |  |
| `67CD5FA1`/growth | `0x00000021` | unnamed | 0x006B78B0 | u32 | keep | 12/12 copies; u32 scl n=1: varies across copies (e.g. 0x1010: 4112) | growth-developer.md §4.8 |  |
| `67CD5FA1`/growth | `0x87CD6300` | unnamed | 0x006B78B0 | u8 | keep | 12/12 copies; u8 scl n=1: varies across copies (e.g. 0x1010: 1) | growth-developer.md §4.8 |  |
| `67CD5FA1`/growth | `0x87CD6301` | unnamed | 0x006B78B0 | u32 | keep | 12/12 copies; u32 scl n=1: varies across copies (e.g. 0x1010: 4112) | growth-developer.md §4.8 |  |
| `67CD5FA1`/growth | `0x87CD6310` | unnamed | 0x006B78B0 | u8 | keep (64 per spec) | 12/12 copies; u8 scl n=1: 4 | growth-developer.md §4.8 |  |
| `67CD5FA1`/growth | `0x87CD6330` | unnamed | 0x006B78B0 | u8 | keep | 12/12 copies; u8 scl n=1: varies across copies (e.g. 0x1010: 1) | growth-developer.md §4.8 |  |
| `67CD5FA1`/growth | `0x87CD6332` | unnamed | 0x006B78B0 | u8 | keep | 12/12 copies; u8 scl n=1: varies across copies (e.g. 0x1010: 1) | growth-developer.md §4.8 |  |
| `67CD5FA1`/growth | `0x87CD6333` | unnamed | 0x006B78B0 | u8 raw | keep | 12/12 copies; u8 arr n=2: 0, 255 | growth-developer.md §4.8 |  |
| `67CD5FA1`/growth | `0x87CD633F` | unnamed | 0x006B78B0 | bool | false (written before the read) | 12/12 copies; bool scl n=1: 0 | growth-developer.md §4.8 |  |
| `67CD5FA1`/growth | `0x87CD6340` | unnamed | 0x006B78B0 | curve(prop*) | delete | 12/12 copies; f32 arr n=36: varies across copies (e.g. 0x1010: 0, 10… | growth-developer.md §4.8, §4.2, §4.3 |  |
| `67CD5FA1`/growth | `0x87CD6398` | unnamed | 0x006B78B0 | curve(prop*) | delete | 12/12 copies; f32 arr n=4: varies across copies (e.g. 0x1010: 0, 0, 2… | growth-developer.md §4.8, §4.2, §4.3 |  |
| `67CD5FA1`/growth | `0x87CD6343` | unnamed | 0x006B78B0 | curve(prop*) | delete | 12/12 copies; f32 arr n=4/36: varies across copies (e.g. 0x1010: 0, 5… | growth-developer.md §4.8, §4.2, §4.3 |  |
| `67CD5FA1`/growth | `0x87CD6344` | unnamed | 0x006B78B0 | curve(prop*) | delete | 12/12 copies; f32 arr n=4/36: varies across copies (e.g. 0x1010: 0, -… | growth-developer.md §4.8, §4.2, §4.3 |  |
| `67CD5FA1`/growth | `0x87CD6345` | unnamed | 0x006B78B0 | curve(prop*) | delete | 12/12 copies; f32 arr n=4/36: varies across copies (e.g. 0x1010: 0, -… | growth-developer.md §4.8, §4.2, §4.3 |  |
| `67CD5FA1`/growth | `0x87CD6391` | unnamed | 0x006B78B0 | curve(prop*) | delete | 12/12 copies; f32 arr n=4/36: varies across copies (e.g. 0x1010: 0, 0… | growth-developer.md §4.8, §4.2, §4.3 |  |
| `67CD5FA1`/growth | `0x87CD6392` | unnamed | 0x006B78B0 | curve(prop*) | delete | 12/12 copies; f32 arr n=4/36: varies across copies (e.g. 0x1010: 0, 0… | growth-developer.md §4.8, §4.2, §4.3 |  |
| `67CD5FA1`/growth | `0x87CD6341` | unnamed | 0x006B78B0 | curve(prop*) | delete | 12/12 copies; f32 arr n=4: varies across copies (e.g. 0x1010: -100, -… | growth-developer.md §4.8, §4.2, §4.3 |  |
| `67CD5FA1`/growth | `0x87CD634B` | unnamed | 0x006B78B0 | curve(prop*) | delete | 12/12 copies; f32 arr n=4/36: varies across copies (e.g. 0x1010: 0, 2… | growth-developer.md §4.8, §4.2, §4.3 |  |
| `67CD5FA1`/growth | `0x87CD634C` | unnamed | 0x006B78B0 | curve(prop*) | delete | 12/12 copies; f32 arr n=4: 0, 0, 255, 0 | growth-developer.md §4.8, §4.2, §4.3 |  |
| `67CD5FA1`/growth | `0x87CD634D` | unnamed | 0x006B78B0 | curve(prop*) | delete | 12/12 copies; f32 arr n=4/36: varies across copies (e.g. 0x1010: 0, 2… | growth-developer.md §4.8, §4.2, §4.3 |  |
| `67CD5FA1`/growth | `0x87CD634E` | unnamed | 0x006B78B0 | curve(prop*) | delete | 12/12 copies; f32 arr n=36: 0, 0, 1, -2000, 30, -2000, 45, -2000, 60… | growth-developer.md §4.8, §4.2, §4.3 |  |
| `67CD5FA1`/growth | `0x87CD6348` | unnamed | 0x006B78B0 | curve(prop*) | delete | 12/12 copies; f32 arr n=4/36: varies across copies (e.g. 0x1010: 0, -… | growth-developer.md §4.8, §4.2, §4.3 |  |
| `67CD5FA1`/growth | `0x87CD6349` | unnamed | 0x006B78B0 | curve(prop*) | delete | 12/12 copies; f32 arr n=4/36: varies across copies (e.g. 0x1010: 0, -… | growth-developer.md §4.8, §4.2, §4.3 |  |
| `67CD5FA1`/growth | `0x87CD634A` | unnamed | 0x006B78B0 | curve(prop*) | delete | 12/12 copies; f32 arr n=4/36: varies across copies (e.g. 0x1010: 0, 2… | growth-developer.md §4.8, §4.2, §4.3 |  |
| `67CD5FA1`/growth | `0x87CD6347` | unnamed | 0x006B78B0 | curve(prop*) | delete | 12/12 copies; f32 arr n=4/36: varies across copies (e.g. 0x1010: 0, 1… | growth-developer.md §4.8, §4.2, §4.3 |  |
| `67CD5FA1`/growth | `0x87CD6346` | unnamed | 0x006B78B0 | curve(prop*) | delete | 12/12 copies; f32 arr n=4/36: varies across copies (e.g. 0x1010: 0, 2… | growth-developer.md §4.8, §4.2, §4.3 |  |
| `67CD5FA1`/growth | `0x87CD6399` | unnamed | 0x006B78B0 | curve(prop*) | delete | 12/12 copies; f32 arr n=4: varies across copies (e.g. 0x1010: 0, 0, 2… | growth-developer.md §4.8, §4.2, §4.3 |  |
| `67CD5FA1`/growth | `0x87CD6397` | unnamed | 0x006B78B0 | curve(prop*) | delete | 12/12 copies; f32 arr n=4: -127, -100, 127, 100 | growth-developer.md §4.8, §4.2, §4.3 |  |
| `67CD5FA1`/growth | `0x87CD6357` | unnamed | 0x006B78B0 | s32 | 80 (0x50 @0x006B80EA) | 12/12 copies; s32 scl n=1: varies across copies (e.g. 0x1010: 25) | growth-developer.md §4.8 |  |
| `67CD5FA1`/growth | `0x87CD6355` | unnamed | 0x006B78B0 | s32 | 80 (@0x006B8105) | 12/12 copies; s32 scl n=1: 50 | growth-developer.md §4.8 |  |
| `67CD5FA1`/growth | `0x87CD6354` | unnamed | 0x006B78B0 | s32 | growth threshold | 12/12 copies; s32 scl n=1: 50 | growth-developer.md §4.8 |  |
| `67CD5FA1`/growth | `0x87CD6356` | unnamed | 0x006B78B0 | bool | false (@0x006B8140) | ABSENT | growth-developer.md §4.8 | ABSENT in shipped data |
| `67CD5FA1`/growth | `0x87CD6360` | unnamed | 0x006B78B0 | u8 | 0 (@0x006B8174) | 12/12 copies; u8 scl n=1: varies across copies (e.g. 0x1010: 8) | growth-developer.md §4.8 |  |
| `67CD5FA1`/growth | `0x87CD6361` | unnamed | 0x006B78B0 | curve(prop*) | (0,0) constant 0 | 12/12 copies; f32 arr n=50: varies across copies (e.g. 0x1010: 0, 100… | growth-developer.md §4.8, §5.5 | I developers ship 3 stages (k=3), so 0x87CD6364..68 exist only for R/C |
| `67CD5FA1`/growth | `0x87CD6362` | unnamed | 0x006B78B0 | curve(prop*) | (0,0) constant 0 | 12/12 copies; f32 arr n=50: varies across copies (e.g. 0x1010: 299, 0… | growth-developer.md §4.8, §5.5 | I developers ship 3 stages (k=3), so 0x87CD6364..68 exist only for R/C |
| `67CD5FA1`/growth | `0x87CD6363` | unnamed | 0x006B78B0 | curve(prop*) | (0,0) constant 0 | 12/12 copies; f32 arr n=34/46: varies across copies (e.g. 0x1010: 431… | growth-developer.md §4.8, §5.5 | I developers ship 3 stages (k=3), so 0x87CD6364..68 exist only for R/C |
| `67CD5FA1`/growth | `0x87CD6364` | unnamed | 0x006B78B0 | curve(prop*) | (0,0) constant 0 | 8/12 copies; f32 arr n=40: varies across copies (e.g. 0x1010: 745, 0… | growth-developer.md §4.8, §5.5 | I developers ship 3 stages (k=3), so 0x87CD6364..68 exist only for R/C; in 8/12 copies |
| `67CD5FA1`/growth | `0x87CD6365` | unnamed | 0x006B78B0 | curve(prop*) | (0,0) constant 0 | 8/12 copies; f32 arr n=32/36: varies across copies (e.g. 0x1010: 1546… | growth-developer.md §4.8, §5.5 | I developers ship 3 stages (k=3), so 0x87CD6364..68 exist only for R/C; in 8/12 copies |
| `67CD5FA1`/growth | `0x87CD6366` | unnamed | 0x006B78B0 | curve(prop*) | (0,0) constant 0 | 8/12 copies; f32 arr n=24/28: varies across copies (e.g. 0x1010: 3205… | growth-developer.md §4.8, §5.5 | I developers ship 3 stages (k=3), so 0x87CD6364..68 exist only for R/C; in 8/12 copies |
| `67CD5FA1`/growth | `0x87CD6367` | unnamed | 0x006B78B0 | curve(prop*) | (0,0) constant 0 | 8/12 copies; f32 arr n=16/20: varies across copies (e.g. 0x1010: 6646… | growth-developer.md §4.8, §5.5 | I developers ship 3 stages (k=3), so 0x87CD6364..68 exist only for R/C; in 8/12 copies |
| `67CD5FA1`/growth | `0x87CD6368` | unnamed | 0x006B78B0 | curve(prop*) | (0,0) constant 0 | 8/12 copies; f32 arr n=12: varies across copies (e.g. 0x1010: 9570, 0… | growth-developer.md §4.8, §5.5 | I developers ship 3 stages (k=3), so 0x87CD6364..68 exist only for R/C; in 8/12 copies |
| `67CD5FA1`/growth | `0x87CD6373` | unnamed | 0x006B78B0 | u8 | keep (0) | 12/12 copies; u8 scl n=1: 0 | growth-developer.md §4.8 |  |
| `67CD5FA1`/growth | `0x87CD6370` | unnamed | 0x006B78B0 | u8 | keep (3) | 12/12 copies; u8 scl n=1: varies across copies (e.g. 0x1010: 3) | growth-developer.md §4.8 |  |
| `67CD5FA1`/growth | `0x87CD6371` | unnamed | 0x006B78B0 | u8 | keep (3) | 12/12 copies; u8 scl n=1: 9 | growth-developer.md §4.8 |  |
| `67CD5FA1`/growth | `0x87CD6372` | unnamed | 0x006B78B0 | u8 | keep (6) | 12/12 copies; u8 scl n=1: varies across copies (e.g. 0x1010: 9) | growth-developer.md §4.8 |  |
| `67CD5FA1`/growth | `0x87CD6380` | unnamed | 0x006B78B0 | f32 raw | 0.2 x3 (0x3E4CCCCD @0x006B8342..6F) | ABSENT | growth-developer.md §4.8 | ABSENT in shipped data |
| `67CD5FA1`/growth | `0x87CD6381` | unnamed | 0x006B78B0 | u8 | 1 | 12/12 copies; u8 scl n=1: 0 | growth-developer.md §4.8 |  |
| `67CD5FA1`/growth | `0x87CD63A0` | unnamed | 0x006B78B0 | u32 raw | empty | 4/12 copies; u32 arr n=4: 12289, 50, 12290, 50 | growth-developer.md §4.8 | in 4/12 copies |
| `67CD5FA1`/growth | `0xCAB1CE3A` | unnamed | 0x006B78B0 | vec<s32> | keep | 12/12 copies; s32 arr n=7: varies across copies (e.g. 0x1010: 14, 14… | growth-developer.md §4.8 |  |
| `67CD5FA1`/growth | `0xCAB1CE3C` | unnamed | 0x006B78B0 | vec<s32> | keep | 12/12 copies; s32 arr n=7: varies across copies (e.g. 0x1010: 21, 21… | growth-developer.md §4.8 |  |
| `E7E2C2DB/C9133286` | `0x491332E6` | Traffic Volume Per Population | 0x00716AE0 | f32 | unchanged (spec: unused) | f32 scl n=1: 2 | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0x491332E7` | Max speed by Network for walking | 0x00716AE0 | vec<f32> raw | property missing: all 13 -> 0 and FLT_MAX (0x7F7FFFFF); short vector:… | f32 arr n=13: 3.5, 0, 0, 3.5, 0, 0, 3.5, 0, 0, 0, 3.5, 0, ... (13) | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0x491332E8` | Max speed by Network for driving | 0x00716AE0 | vec<f32> raw | property missing: all 13 -> 0 and FLT_MAX (0x7F7FFFFF); short vector:… | f32 arr n=13: 31, 0, 82, 21, 0, 0, 40, 0, 0, 0, 31, 0, ... (13) | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0x491332E9` | Max speed by Network for a bus | 0x00716AE0 | vec<f32> raw | property missing: all 13 -> 0 and FLT_MAX (0x7F7FFFFF); short vector:… | f32 arr n=13: 46, 0, 100, 31, 0, 0, 60, 0, 0, 0, 46, 0, ... (13) | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0x491332EA` | Max speed by Network for a train | 0x00716AE0 | vec<f32> raw | property missing: all 13 -> 0 and FLT_MAX (0x7F7FFFFF); short vector:… | f32 arr n=13: 0, 110, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, ... (13) | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0x491332EB` | Max speed by Network for a truck | 0x00716AE0 | vec<f32> raw | property missing: all 13 -> 0 and FLT_MAX (0x7F7FFFFF); short vector:… | f32 arr n=13: 31, 0, 82, 21, 0, 0, 40, 0, 0, 0, 31, 0, ... (13) | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0x491332EC` | Max speed by Network for a freight train | 0x00716AE0 | vec<f32> raw | property missing: all 13 -> 0 and FLT_MAX (0x7F7FFFFF); short vector:… | f32 arr n=13: 0, 150, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, ... (13) | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0x491332ED` | Max speed by Network for subways | 0x00716AE0 | vec<f32> raw | property missing: all 13 -> 0 and FLT_MAX (0x7F7FFFFF); short vector:… | f32 arr n=13: 0, 0, 0, 0, 0, 0, 0, 150, 0, 0, 0, 0, ... (13) | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0x491332EE` | Max speed by Network for el train | 0x00716AE0 | vec<f32> raw | property missing: all 13 -> 0 and FLT_MAX (0x7F7FFFFF); short vector:… | f32 arr n=13: 0, 0, 0, 0, 0, 0, 0, 0, 150, 0, 0, 0, ... (13) | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0x491332EF` | Max speed by Network for monorail | 0x00716AE0 | vec<f32> raw | property missing: all 13 -> 0 and FLT_MAX (0x7F7FFFFF); short vector:… | f32 arr n=13: 0, 0, 0, 0, 0, 0, 0, 0, 0, 200, 0, 0, ... (13) | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0x29136788` | Monthly Traffic Density Reduction | 0x00716AE0 | f32 | unchanged (spec: uninitialised) | f32 scl n=1: 0 | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0x29136789` | unnamed | 0x00716AE0 | f32 | unchanged (spec ctor 0.03125) | ABSENT | traffic-simulator.md §4.14 | absent in shipped data (spec agrees); ABSENT in shipped data |
| `E7E2C2DB/C9133286` | `0x6A84493E` | Monthly cost for network tile | 0x00716AE0 | vec<f32> raw | 13 x 0 (local vector empty when missing) | f32 arr n=13: 0.1, 0.03, 0.5, 0.05, 0, 0, 0.15, 0.3, 0.2, 0.3, 0.1, 0… | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0xA92356AE` | Max roads funding percent | 0x00716AE0 | numeric->f32 x0.01 | unchanged (spec ctor 1.2) | f32 scl n=1: 120 | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0xA92356AF` | Max mass transit funding percent | 0x00716AE0 | numeric->f32 x0.01 | unchanged (spec ctor 1.2) | f32 scl n=1: 120 | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0xA92356B0` | Damaged road extra step cost | 0x00716AE0 | f32 | unchanged (spec ctor 32.0) | f32 scl n=1: 0.1 | traffic-simulator.md §4.14; traffic-net… |  |
| `E7E2C2DB/C9133286` | `0xA92356B6` | unnamed | 0x00716AE0 | u32 | unchanged (spec ctor 365) | ABSENT | traffic-simulator.md §4.14 | absent in shipped data (spec agrees); ABSENT in shipped data |
| `E7E2C2DB/C9133286` | `0xA92356B7` | unnamed | 0x00716AE0 | u32 | unchanged (spec ctor 128) | ABSENT | traffic-simulator.md §4.14 | absent in shipped data (spec agrees); ABSENT in shipped data |
| `E7E2C2DB/C9133286` | `0xA92356B8` | Maximum distance from origin to network | 0x00716AE0 | u32 | unchanged (spec ctor 1) | u32 scl n=1: 1 | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0xA92356B1` | Income per tile by travel type | 0x00716AE0 | vec<f32> raw | unchanged | f32 arr n=9: 0, 0, 0.001, 0.001, 0, 0, 0.001, 0.001, 0.001 | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0xA92356B3` | Network Traffic Capacity | 0x00716AE0 | vec<f32> raw | unchanged | f32 arr n=13: 1000, 3000, 4000, 100, 0, 0, 2500, 3000, 3000, 3000, 20… | traffic-simulator.md §4.14; traffic-net… |  |
| `E7E2C2DB/C9133286` | `0xA92356B4` | Travel type generates traffic | 0x00716AE0 | vec<bool> | unchanged; short vector -> true | bool arr n=9: 0, 1, 0, 1, 1, 1, 1, 1, 1 | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0xA92356BF` | Travel type affected by traffic | 0x00716AE0 | vec<bool> | unchanged; short vector -> true | bool arr n=9: 0, 1, 1, 1, 1, 1, 1, 1, 0 | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0xA92356B5` | Travel type can reach destination | 0x00716AE0 | vec<bool> | unchanged; short vector -> true | bool arr n=9: 1, 1, 0, 0, 1, 1, 0, 0, 0 | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0xA92356B9` | Congestion vs Speed | 0x00716AE0 | curve(key,id) | keep (then inverted again) | f32 arr n=8: 0, 1, 1, 1, 2, 0.65, 3, 0.3 | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0xA92356BA` | Commute trip max time | 0x00716AE0 | f32 | unchanged (spec ctor 90.0) | f32 scl n=1: 6 | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0xA92356BB` | Intersection and Turn Capacity Effect | 0x00716AE0 | vec<f32> raw | unchanged | f32 arr n=3: 0.7, 0.8, 0.9 | traffic-simulator.md §4.14; traffic-net… |  |
| `E7E2C2DB/C9133286` | `0xA92356BC` | Trip Starting Cost by travel type | 0x00716AE0 | vec<f32> raw | unchanged | f32 arr n=9: 0, 0.4, 0, 0, 0, 0, 0, 0, 0 | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0xEA8C3CDB` | Trip Starting Cost by travel type for M… | 0x00716AE0 | vec<f32> raw | unchanged | f32 arr n=9: 0, 1.95, 0, 0, 0, 0, 0, 0, 0 | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0xCAD64136` | Trip Starting Cost by travel type for C… | 0x00716AE0 | vec<f32> raw | unchanged | f32 arr n=9: 0.1, 0, 0, 0, 0, 0, 0, 0, 0 | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0xA92356BD` | Job Scaling Constant | 0x00716AE0 | f32 | unchanged (spec ctor 1.0) | f32 scl n=1: 1.2 | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0x49A2E8BE` | Freight traffic scaling factor | 0x00716AE0 | f32 | unchanged (spec: uninitialised) | f32 scl n=1: 0.05 | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0x4A678060` | Nearest Destination Attractiveness | 0x00716AE0 | f32 | unchanged (spec ctor 0.75) | f32 scl n=1: 0.09 | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0xCA76013B` | Trip Length to Minutes Display Multipli… | 0x00716AE0 | f32 | unchanged (spec ctor 0.5) | f32 scl n=1: 25 | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0xEA7B5F06` | Max Mass Transit Strategy Trip Length | 0x00716AE0 | f32 | unchanged (spec ctor 4.0) | f32 scl n=1: 4 | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0xA92356BE` | Population Background Traffic | 0x00716AE0 | vec<f32> raw | unchanged; short vector -> 1.0 | f32 arr n=3: 0.05, 0.2, 0.2 | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0xCA5F7821` | Transit Switch Entry Cost vs. Budget | 0x00716AE0 | curve(prop*) | (0,0)(1,1) | f32 arr n=4: 0, 2, 100, 1 | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0x4BE09EC9` | Congestion to Accident Probability | 0x00716AE0 | curve(prop*) | (0,0)(0.25,0)(1,0.05)(2,0.25) | f32 arr n=8: 0, 0, 0.25, 0, 1, 0.05, 2, 0.25 | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0x4BE09ECA` | Capacity to Accident Probability | 0x00716AE0 | curve(prop*) | (0,1)(1000,1) | f32 arr n=4: 0, 1, 1000, 1 | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0x4BE09ECB` | Accident Duration | 0x00716AE0 | f32 | 10 (0x41200000 @0x007179A2) | f32 scl n=1: 20 | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0x4BE09ECC` | Accident Check Period | 0x00716AE0 | f32 | 60 (0x42700000 @0x007179BA) | f32 scl n=1: 60 | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0x0BD86FD4` | Funding to Damage Acceleration Curve | 0x00716AE0 | curve(prop*) | (0,0.02)(100,-0.01)(120,-0.04) | f32 arr n=6: 0, 0.02, 100, -0.01, 120, -0.04 | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0x0BD86FD5` | Rail Damage Accident Factor | 0x00716AE0 | f32 | 1.0 (@0x00717B95) | f32 scl n=1: 1 | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0x8C3CAEF5` | Ferry Fare | 0x00716AE0 | f32 | unchanged (spec ctor 0.001) | f32 scl n=1: 0.01 | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0x0CAAAEB8` | unnamed | 0x00716AE0 | f32 | unchanged (spec ctor 0.128) | f32 scl n=1: 0.128 | traffic-simulator.md §4.14 |  |
| `E7E2C2DB/C9133286` | `0x4953E8A3` | Travel strategy percent WealthNone | 0x00711810 | vec<u8> | missing exemplar: re-read skipped (ordinance offsets accumulate, spec… | u8 arr n=3: 0, 0, 100 | traffic-simulator.md §4.1.2 |  |
| `E7E2C2DB/C9133286` | `0x4953E8A4` | Travel strategy percent Wealth$ | 0x00711810 | vec<u8> | missing exemplar: re-read skipped (ordinance offsets accumulate, spec… | u8 arr n=3: 80, 20, 0 | traffic-simulator.md §4.1.2 |  |
| `E7E2C2DB/C9133286` | `0x4953E8A5` | Travel strategy percent Wealth$$ | 0x00711810 | vec<u8> | missing exemplar: re-read skipped (ordinance offsets accumulate, spec… | u8 arr n=3: 30, 0, 70 | traffic-simulator.md §4.1.2 |  |
| `E7E2C2DB/C9133286` | `0x4953E8A6` | Travel strategy percent Wealth$$$ | 0x00711810 | vec<u8> | missing exemplar: re-read skipped (ordinance offsets accumulate, spec… | u8 arr n=3: 10, 80, 10 | traffic-simulator.md §4.1.2 |  |

### 7.3 Per-occupant properties named by the M1–M5 specs

Types and defaults as the specs give them (not re-disassembled); "Shipped types" counts every shipped exemplar that carries the property.

| Prop | Name | Reader | Type (spec) | Default (spec) | Shipped types | Spec | Notes |
|---|---|---|---|---|---|---|---|
| 0x029244DB | preview occupant (inferred) | pollution, plumbing, crime, flammability (existence test) | bool (existence only) | absent = normal occupant | not in shipped data | foundations.md §5.2; pollution.md §7.2; plumbing.md §8 |  |
| 0x27812851 | pollution at centre [air, water, garbage, radiation] | pollution DoOccupantChanged | s32[4] | absent = no pollution | s32[] x490 | pollution.md §4.2.2, §7.2 |  |
| 0xAA5832F3 | pollution outer values | pollution | s32[4] | 0 | s32[] x1 | pollution.md §7.2 |  |
| 0x68EE9764 | pollution radii (tracts) | pollution | f32[4] | 0 | f32[] x489 | pollution.md §7.2 |  |
| 0xC8EBD131 | base capacity: water treatment | pollution | u32 | 0 | u32 x1 | pollution.md §7.2 |  |
| 0xA8EBCF0C | base capacity: incinerator | pollution | u32 | 0 | not in shipped data | pollution.md §7.2 |  |
| 0x48EBD0B1 | base capacity: waste-to-energy | pollution | u32 | 0 | u32 x1 | pollution.md §7.2 |  |
| 0x88ED1059 | recycling percentage | pollution | (list membership) | absent = not a recycling centre | u32 x1 | pollution.md §7.2 |  |
| 0x68ED1736 | recycling centre population served | pollution | f32 | - | u32 x1 | pollution.md §7.2 |  |
| 0x48EE7A44 | building garbage capacity | pollution slot 47 | u32 | 0 | not in shipped data | pollution.md §7.2 |  |
| 0x27812852 | Power Generated | power; pollution (WTE output, inferred) | u32 | 0 | u32 x17 | power.md §8; pollution.md §7.2 |  |
| 0x08FD1613 | WTE max output (inferred) | pollution | u32 | - | not in shipped data | pollution.md §7.2 |  |
| 0xC910B4CF | condition / wear (written) | pollution, plumbing, power | f32 | 0 | not in shipped data | pollution.md §7.2; plumbing.md §8; power.md §8 |  |
| 0xEA1CF220 | Age degradation rate | pollution, plumbing, power | f32 | 0 | f32 x13 | pollution.md §7.2; plumbing.md §8; power.md §8 |  |
| 0xEA1CF221 | Soft failure threshold | pollution, plumbing, power | f32 | 99 (power) | f32 x13 | power.md §8 |  |
| 0xEA1CF222 | Hard failure threshold | pollution, plumbing, power | f32 | 99 (power) | f32 x13 | power.md §8 |  |
| 0x8A1E07EE | Age to output level response curve | pollution, plumbing, power | curve (f32 pairs) | delete (no curve, per call) | f32[] x13 | power.md §8; pollution.md §7.2; plumbing.md §8 |  |
| 0x0911E2E7 | Age to maintenance cost multiplier curve | pollution, plumbing, power | curve (f32 pairs) | delete (per call) | f32[] x13 | power.md §8; pollution.md §7.2; plumbing.md §8 |  |
| 0x2A58D350 | base maintenance cost (written) | pollution, plumbing, power | s64 | - | not in shipped data | power.md §8; pollution.md §7.2; plumbing.md §8 |  |
| 0x27812854 | Power Consumed | power | u32 | 0 | u32 x449 | power.md §8 |  |
| 0x27812853 | Power Plant Type | power | read as s32 | 0 | u32 x8 | power.md §8 |  |
| 0x08D9763D | power / water used (written) | power, plumbing | s32 (power) / u32 (plumbing) | - | not in shipped data | power.md §8; plumbing.md §8 |  |
| 0xC910B4CE | Power plant energy conversion cost | power | f32 | 0 | not in shipped data | power.md §8 |  |
| 0x6A43150F | Hard Failure Type | power | u32 | 0 | u32 x10 | power.md §8 |  |
| 0xAA5C04C5 | Radiation Spew Intensity | power | f32 | 0 | f32 x1 | power.md §8 |  |
| 0xAA5C04C8 | Radiation Spew Radius | power | f32 | 0 | f32 x1 | power.md §8 |  |
| 0x09132DE8 | Power pole monthly upkeep | power | s64 | - | s64 x6 | power.md §8 |  |
| 0xC8ED2D84 | water consumption | plumbing | s32 (demand) / u32 (flood) | 0 | u32 x442 | plumbing.md §8 |  |
| 0x88ED3303 | water production capacity | plumbing | u32 | absent = not a producer | u32 x4 | plumbing.md §8 |  |
| 0x48F23A7E | water source type (inferred) | plumbing | u8 | - | u8 x4 | plumbing.md §8, §7.2 |  |
| 0xEA1E3C53 | water cost per unit used (inferred) | plumbing | f32 | - | not in shipped data | plumbing.md §8 |  |
| 0xCA5B9306 | crime effect {m, r} | crime DoOccupantChanged | byte array (2, no length check) | absent = no effect | u8[] x6 | crime.md §6.10, §8 |  |
| 0x8AA864D5 | prop crime effect | crime (props, type 0xA823821E) | - | absent = not counted | u32 x10 | crime.md §6.10, §8 |  |
| 0x08F55DC0 | fire station effect centre | 0x006AD7C0 fire GetStationEffect | s32 | fails when absent | s32 x3 | protection.md §7.1 |  |
| 0x08F55DC1 | fire station effect outer | 0x006AD7C0 | s32 | 0 | s32 x3 | protection.md §7.1 |  |
| 0x08F55DC2 | fire station effect radius (marks a fire station) | 0x006AD7C0; IsFireStation 0x006ACF80 | f32 | fails when absent | f32 x3 | protection.md §7.1 |  |
| 0x08F55DC5 | fire dispatch unit count | fire | s32 | 0 | s32 x3 | protection.md §7.3 |  |
| 0x48D71ED0 | police station effect centre | 0x006E0D50 police GetStationEffect | s32 | fails when absent or 0 | s32 x4 | protection.md §7.1 |  |
| 0x48D71ED1 | police station effect outer | 0x006E0D50 | s32 | unchanged | s32 x4 | protection.md §7.1 |  |
| 0x48D71ED2 | police station effect radius (marks a police station) | 0x006E0D50; IsPoliceStation 0x006E01C0 | f32 | unchanged | f32 x4 | protection.md §7.1 |  |
| 0x48D71ED5 | police dispatch unit count | police | s32 | 0 | s32 x4 | protection.md §7.3 |  |
| 0xEAA69F83 | station strike start day (written) | fire, police | u32 | 0 = not striking | not in shipped data | protection.md §6.3 |  |
| 0x8A70A0B9 | police station statistics (written) | police | u32 vector | - | not in shipped data | protection.md §9.2 |  |
| 0x29244DB5 | flammability | flammability | read as s32 (shipped u8) | 0 | u8 x619 | flammability.md §6.1 |  |
| 0x691B42B3 | School Coverage Radius | REH | f32 | building skipped | f32 x14 | residential-education-health.md §4.13 |  |
| 0x091B42A2 | Hospital coverage radius | REH | f32 | building skipped | f32 x4 | residential-education-health.md §4.13 |  |
| 0x8922041B | school capacity | REH | - | skipped / 0 | u32 x14 | residential-education-health.md §4.13 |  |
| 0x69220415 | hospital capacity | REH | - | skipped / 0 | u32 x4 | residential-education-health.md §4.13 |  |
| 0xA92AE446 | school effect multiplier | REH | - | 1.0 | f32 x14 | residential-education-health.md §4.13 |  |
| 0xC9299659 | hospital effect multiplier | REH | - | 1.0 | f32 x4 | residential-education-health.md §4.13 |  |
| 0x27812840 | demand satisfied (key, amount) pairs | 0x006A8420 AddToAgentEffectMap | u32 pairs | empty list keeps previous | u32[] x56 | demand.md §7.1 |  |
| 0x27812841 | demand created (key, amount) pairs | 0x006A8420 | u32 pairs | empty list keeps previous | u32[] x75 | demand.md §7.1 |  |
| 0x27812842 | satisfied amount overrides | 0x006A8420 | f32 list | - | not in shipped data | demand.md §7.1 |  |
| 0x27812843 | created amount overrides | 0x006A8420 | f32 list | - | not in shipped data | demand.md §7.1 |  |
| 0x27812844 | exclude from demand effects | 0x006A8420 | bool | false | bool x12 | demand.md §7.1 |  |
| 0xEA54D283 | budget item department IDs | budget | u32[] | - (defines the item count) | u32[] x82 | budget.md §2 |  |
| 0xEA54D284 | budget item line item IDs | budget | u32[] | 0 | u32[] x81 | budget.md §2 |  |
| 0xEA54D285 | budget item purpose IDs | budget | u32[] | 0 | u32[] x81 | budget.md §2 |  |
| 0xEA54D286 | budget item monthly cost | budget; landfill tile cost (first value) | s64[] | 0 | s64[] x174, s64 x2 | budget.md §2; landfill.md §4.1 |  |
| 0xEA54D287 | budget item local funding % | budget | f32[] | 100 | not in shipped data | budget.md §2 |  |
| 0xE91A0B5F | building value | budget | s64 scalar | 0 | s64 x290 | budget.md §5.2 |  |
| 0xE90E25A1 | transit switch rules | traffic network (`0x005FFB40`: raw bytes, `GetCount` bytes from `RefUint8`) | u8 array, 4 bytes per rule | absent = not a switch; count not a multiple of 4 = DoLotActivated returns | u8[] x13 (8–32 bytes) | traffic-network.md §4.7.1, §4.7.2 | a u32 array is misread (§4.7.2) |
| 0xE90E25A2 | transit switch time factor | traffic network | f32 | - | f32 x13 | traffic-network.md §2.4 |  |
| 0xE90E25A3 | transit switch capacity | traffic network | f32 | - | f32 x13 | traffic-network.md §2.4 |  |
| 0x4BFC47B0 | counts as intersection | traffic network | bool | - | bool x4 | traffic-network.md §2.4, §4.6 |  |
| 0x0BD86FD3 | ferry type | traffic network | 0..2 | absent = no ferry | u8 x2 | traffic-network.md §2.6, §4.7.1 |  |
| 0x88FCD877 | building foundation type | lot developer | u32 | lot developer default (0x6916516B) | u32 x109 | lot-developer.md §11.1 |  |

Keyed families read by the simulators but outside this table: demand exemplars (group `0xC7BB4816`, keyed by demand ID, demand.md §4.1); building agent effects `0x27812840..44` read per building type by `0x006A8420` AddToAgentEffectMap; ordinance exemplars (enumerated, property `0x10` = 14, ordinance.md §5) with their effect properties such as `0x8A612FEE`; landfill tile exemplars (group `0x07BDDF1C`).

### 7.4 Disagreements found (spec vs code vs data)

- **budget.md §9 (row 0x89170C6C...0x49170C7C)**. Spec: 0x49170C7C MinAverageLV is a float. Exe/data: read with the u8 getter 0x005FD3C0 into global 0x00B0D1C0 (the other three use the f32 getter). Evidence: call @0x006923C4.
- **aura.md §10 (rows 0xEA023290-93, and the sentence 'converted to float')**. Spec: s32[2] {magnitude, radius}, converted to float. Exe/data: read with the float-vector getter 0x005FFCC0 (raw words, no conversion), used as float32 by 0x0068BAD0 (fld); shipped type is f32 arr 2 (e.g. 0xEA023290 = -20.0, 128.0). No int->float conversion exists. Evidence: 0x0068D94E, 0x0068BADA; SimCity_1.dat 6534284A/E7E2C2DB/AA023079.
- **neighbors.md §12 (header 'All values are f32'; rows 0x6A469342, 0xCA469346)**. Spec: all values are f32. Exe/data: GetDealCost 0x006D3250 reads 0x6A469342 and 0xCA469346 with the f64 getter 0x005FD530 into doubles (then adds 1.0); ProposeDeals 0x006D21A0 reads 0xCA469346 with the f32 getter 0x005FD4F0. Shipped type is f32, so the f64 reads go through the variant's conversion; values agree, types do not. Evidence: calls @0x006D3512, @0x006D3530, @0x006D22B4.
- **zones.md §4 (exemplar header line)**. Spec: Zone Manager exemplar 0xE9482490 has type 0xA52160F5. Exe/data: the key built in 0x004DC460 is {type 0x6534284A, group 0xE7E2C2DB, instance 0xE9482490}; 0xA52160F5 is the cISCResExemplar IID passed to the resource manager. Shipped type is 0x6534284A. Evidence: @0x004DC492..AB; SimCity_1.dat 6534284A/E7E2C2DB/E9482490.
- **crime.md §8 (row 0x47E2C200) vs landvalue.md §8 (row 0x47E2C200) [spec vs spec, spec vs code]**. Spec: crime.md types the tract size 0x47E2C200 as u32; landvalue.md types the same property ID as u8. Exe/data: crime 0x006A07A0 reads it from the Crime exemplar 0xC8E8F6C5 with the s32 getter 0x005FD450 (absent there, default 2); land value 0x006BE110 reads it from 0xE7E2C8D8 with the u8 getter 0x005FD3C0 (shipped u8 2). Both getters convert, so only the spec types differ. Evidence: calls in 0x006A07A0 and 0x006BE110; SimCity_1.dat 6534284A/E7E2C2DB/E7E2C8D8.
- **flora.md §7 (rows 0x8761ABD0, 0x085489C7) [spec vs code]**. Spec: u32. Exe/data: both are read with the s32 getter 0x005FD450 (shipped u32 24 and 0, converted). Evidence: 0x006B00F0.
- **power.md §8 (row 0x49122036, Shipped column) [spec vs data]**. Spec: shipped 0.9. Exe/data: shipped f32 90; 0.9 is the value after the reader's x0.01 (the neighbouring rows write '20 -> 0.19999999'). Evidence: SimCity_1.dat 6534284A/E7E2C2DB/C911E35B.
- **re/tuning/simulators.md (0x4A1F38C7 'Commercical Cap Relief for Seaports', 0x4A1F38CA 'Industrial Cap Relief for Seaports') [simulators.md vs code]**. Spec: demand.md §11.1 already documents this: the names say Commercial / Industrial. Exe/data: 0x006A8790 stores 0x4A1F38C7 into this+0xCC and 0x4A1F38CA into this+0xD8, which demand.md maps to the industrial and commercial seaport relief (swapped against the names). Evidence: 0x006A8790.

**Code type vs shipped type, no conversion on the read path** (1):

- 0x6534284A/0xE7E2C2DB/0xAA023079: 0xEA0232A0: raw/vector vec<u32> raw, shipped s32: same width, bits copied (signedness differs)

**Properties a reader reads that are missing from the shipped data (each is also marked absent in its owning spec)** (19):

- 0x6534284A/0xE7E2C2DB/0x0BFF6824: 0x89170C6C read by 0x00692210 (f32) is not in the shipped exemplar; default: global; reader sets none
- 0x6534284A/0xE7E2C2DB/0x0BFF6824: 0x09170C74 read by 0x00692210 (f32) is not in the shipped exemplar; default: global; reader sets none
- 0x6534284A/0xE7E2C2DB/0x0BFF6824: 0xC9170C78 read by 0x00692210 (f32) is not in the shipped exemplar; default: global; reader sets none
- 0x6534284A/0xE7E2C2DB/0x0BFF6824: 0x49170C7C read by 0x00692210 (u8) is not in the shipped exemplar; default: global; reader sets none
- 0x6534284A/0x7A4A8458/0x1A2FDB6B: 0xA7607D86 read by 0x00725AF0 (f32) is not in the shipped exemplar; default: unchanged (spec: ctor 3)
- 0x6534284A/0x7A4A8458/0x1A2FDB6B: 0xA7607D87 read by 0x00725AF0 (f32) is not in the shipped exemplar; default: unchanged (spec: ctor 0)
- 0x6534284A/0x7A4A8458/0x1A2FDB6B: 0xA7607D88 read by 0x00725AF0 (f32) is not in the shipped exemplar; default: unchanged (spec: ctor 1)
- 0x6534284A/0xE7E2C2DB/0xC8E8F6C5: 0x47E2C200 read by 0x006A07A0 (s32) is not in the shipped exemplar; default: 2 (@0x006A07C1, before the fetch)
- 0x6534284A/0xE7E2C2DB/0x28DFCF89: 0x68DDAE92 read by 0x006E2110 (numeric->f32 x0.01) is not in the shipped exemplar; default: 1.1 (0x3F8CCCCD @0x006E213C)
- 0x6534284A/0xE7E2C2DB/0x69189C84: 0xE918946F read by 0x006FBAB0 (f32) is not in the shipped exemplar; default: 0.6 (0x3F19999A @0x006FBC8C)
- 0x6534284A/0xE7E2C2DB/0x69189C84: 0xC91894A8 read by 0x006FBAB0 (f32) is not in the shipped exemplar; default: 0.6 (@0x006FBD15)
- 0x6534284A/0xE7E2C2DB/0xE8DA7677: 0x47E2C381 read by 0x006959E0 (f32) is not in the shipped exemplar; default: 0.5 (0x3F000000 @0x00695A19)
- 0x6534284A/0xE7E2C2DB/0xE8DA7677: 0x47E2C382 read by 0x006959E0 (u32) is not in the shipped exemplar; default: 10 (@0x00695A1F)
- 0x6534284A/0xE7E2C2DB/0xE8DA7677: 0x6916516B read by 0x006C4A20 (u32) is not in the shipped exemplar; default: unchanged (spec ctor 0x890B7312)
- 0x6534284A/0x67CD5FA1/{0x1010, 0x1020, 0x1030, 0x3110, 0x3120, 0x3130, 0x3320, 0x3330, 0x4100, 0x4200, 0x4300, 0x4400}: 0x87CD6356 read by 0x006B78B0 (bool) is not in the shipped exemplar; default: false (@0x006B8140)
- 0x6534284A/0x67CD5FA1/{0x1010, 0x1020, 0x1030, 0x3110, 0x3120, 0x3130, 0x3320, 0x3330, 0x4100, 0x4200, 0x4300, 0x4400}: 0x87CD6380 read by 0x006B78B0 (f32 raw) is not in the shipped exemplar; default: 0.2 x3 (0x3E4CCCCD @0x006B8342..6F)
- 0x6534284A/0xE7E2C2DB/0xC9133286: 0x29136789 read by 0x00716AE0 (f32) is not in the shipped exemplar; default: unchanged (spec ctor 0.03125)
- 0x6534284A/0xE7E2C2DB/0xC9133286: 0xA92356B6 read by 0x00716AE0 (u32) is not in the shipped exemplar; default: unchanged (spec ctor 365)
- 0x6534284A/0xE7E2C2DB/0xC9133286: 0xA92356B7 read by 0x00716AE0 (u32) is not in the shipped exemplar; default: unchanged (spec ctor 128)

**Code type vs shipped type, converted by the variant getter (harmless today)** (21):

- 0x6534284A/0xE7E2C2DB/0x0BFF6824: 0x090E0AAC: getter s32, shipped u8: converted by the variant
- 0x6534284A/0xE7E2C2DB/0xC911E35B: 0x2912202E: getter s32, shipped u32: converted by the variant
- 0x6534284A/0xE7E2C2DB/0x28DFCF89: 0x68DDAE9F: getter s32, shipped u32: converted by the variant
- 0x6534284A/0x7A4A8458/0x1A2FDB6A: 0x8761ABD0: getter s32, shipped u32: converted by the variant
- 0x6534284A/0x7A4A8458/0x1A2FDB6A: 0x085489C7: getter s32, shipped u32: converted by the variant
- 0x6534284A/0xE7E2C2DB/0x69189C84: 0xC929C652: getter s16, shipped u8: converted by the variant
- 0x6534284A/0xE7E2C2DB/0x69189C84: 0x0929CC30: getter s16, shipped u8: converted by the variant
- 0x6534284A/0xE7E2C2DB/0x69189C84: 0x0929CC36: getter s16, shipped u8: converted by the variant
- 0x6534284A/0xE7E2C2DB/0x69189C84: 0xC929CC58: getter f32, shipped u8: converted by the variant
- 0x6534284A/0xE7E2C2DB/0x69189C84: 0x0929CC65: getter f32, shipped u8: converted by the variant
- 0x6534284A/0xE7E2C2DB/0x69189C84: 0x2929CC6B: getter f32, shipped u8: converted by the variant
- 0x6534284A/0xE7E2C2DB/0x69189C84: 0xC929CC8D: getter f32, shipped u8: converted by the variant
- 0x6534284A/0xE7E2C2DB/0x69189C84: 0x6929CC92: getter f32, shipped u8: converted by the variant
- 0x6534284A/0xE7E2C2DB/0x69189C84: 0x4929CC97: getter f32, shipped u8: converted by the variant
- 0x6534284A/0xE7E2C2DB/0x69189C84: 0x8918948B: getter s32, shipped u32: converted by the variant
- 0x6534284A/0xE7E2C2DB/0x69189C84: 0xA91894BC: getter s32, shipped u32: converted by the variant
- 0x6534284A/0x7A4A8458/0x6A469021: 0x6A469342: getter f64, shipped f32: converted by the variant
- 0x6534284A/0x7A4A8458/0x6A469021: 0xCA469346: getter f64, shipped f32: converted by the variant
- 0x6534284A/0x67CD5FA1/0x6A2806AF: 0x2AA00FE7: getter f32, shipped s64: converted by the variant
- 0x6534284A/0xE7E2C2DB/0xE8DA7677: 0xE900EA27: getter bool, shipped u32: converted by the variant
- 0x6534284A/0xE7E2C2DB/0xE8DA7677: 0xE900EA28: getter bool, shipped u32: converted by the variant

**simulators.md vs shipped data** (1):

- 0x4A1F3CE6 0x4A1F38B5: simulators.md truncates the value with '…' (58 of 82 values shown); simulators.json has all

**Shipped simulator exemplars (group 0xE7E2C2DB) not catalogued**:

- 0x08ED34DD 'UI': no immediate reference in the exe
- 0x48EA97D4 'Ordinance Simulator': no immediate reference in the exe
- 0x4AA4047A '': no immediate reference in the exe

### 7.5 Getter helper use in the readers

Call sites in the catalogued reader functions (007025B0, 00692210, 006A8790, ...; 40 functions, terrain 0x0074B6A0 excluded). The neighbour functions also call these helpers for non-tuning holders.

| Helper | Meaning | Call sites |
|---|---|---|
| 0x005FD4F0 | f32 | 125 |
| 0x005F4480 | curve(vector) | 49 |
| 0x005F4700 | curve(key,id) | 48 |
| 0x005F4620 | curve(prop*) | 42 |
| 0x005FD480 | u32 | 38 |
| 0x005FD450 | s32 | 21 |
| 0x005FFCC0 | vec<f32> raw | 19 |
| 0x005FD3C0 | u8 | 17 |
| 0x005FDAC0 | numeric->f32 x0.01 | 15 |
| 0x005FD390 | bool | 6 |
| 0x005FF510 | vec<u32> raw | 5 |
| 0x005FD4B0 | s64 | 4 |
| 0x005FD3F0 | s16 | 3 |
| 0x005FFC00 | vec<s32> | 3 |
| 0x005FF000 | vec<bool> | 3 |
| 0x005FD530 | f64 | 2 |
| 0x005FFD80 | vec<s64> raw | 1 |
| 0x005FD420 | u16 | 1 |
| 0x005FFB40 | vec<u8> | 1 |
| variant slot +1BC | raw data pointer | 9 |
| variant slot +19C | raw data pointer | 8 |
| variant slot +17C | raw data pointer | 4 |
| variant slot +1A4 | raw data pointer | 2 |
| variant slot +1B4 | raw data pointer | 2 |

### 7.6 Not verified

- 0x88CD66E9/0x00000001 terrain exemplar: only 0x48CD7B26/27 catalogued; the instance (1) and the code defaults in cSTETerrain::ReadTuningParameters 0x0074B6A0 were not checked.
- Growth developer 0x87CD6310: the u8 getter writes one byte into a dword local whose upper bytes are not initialised before SetTractSize (0x006B3D30); whether SetTractSize masks it was not checked.
- Constructor defaults quoted as 'spec ctor' (lot developer, plumbing, weather, zones 0xE94825B7) were taken from the specs and not re-verified in the constructors.
- Traffic simulator: the 'spec ctor' defaults (constructor values) were not re-verified in the constructor; the curve rows say only what the reader writes.
- Traffic simulator curves 0xCA5F7821/0x4BE09EC9/0x4BE09ECA/0x0BD86FD4: the reader loads the property in place when present and builds the code default only when it is missing; the spec's 'rebuilt from the default, then loaded' gives the same result, assuming 0x005F4620 fully replaces the points (not re-checked).
- Per-occupant table: entries, types and defaults are taken from the specs; their readers were not re-disassembled. The shipped-type column is computed over every shipped exemplar that carries the property.

---

## 8. Interfaces and replaceability

| Interface | Class | Vtable | Slots | Notes |
|---|---|---|---|---|
| `cISCResExemplar` | `cSCResExemplar` | `0x00AAF0D0` | 16 | foundations §17.5 lists 14; slots 14 `0x0067E5B0` and 15 `0x0067DDC0` (rebuild the effective list from the parent, §4.4) follow |
| `cISCPropertyHolder` | `cSCResExemplar` | `0x00AAF038` | 25 | §17.2 layout; slot 15 `AddProperty(property, bool)` `0x0067DCE0` also updates the effective list and, when the bool is true, posts message `0xC53D10AA` |
| cohort interface (IID `0x452160FB`) | `cSCResExemplarCohort` | `0x00AAEFF8` | 16 | slot 7 `SetCohort(cohort, bool)` `0x0067C430` (posts `0x853D11F1` when the bool is true), slot 9 returns the property holder, slot 15 `0x0067DAE0` rebuild |
| `cISCPropertyHolder` | `cSCResExemplarCohort` | `0x00AAEF60` | 25 | as the exemplar's |
| resource factory | `cSCResExemplarFactory` | `0x00AAEEF8` | 13 | `CreateInstance` `0x0067D820` (type `0x6534284A`), `SerialRead` `0x0067F1E0`, `SerialWrite` `0x00680030` |
| resource factory | `cSCResExemplarCohortFactory` | `0x00AAEF2C` | 13 | `CreateInstance` `0x0067D8D0` (type `0x05342861`), `SerialRead` `0x0067F7D0`, `SerialWrite` `0x006804D0` |
| `cIGZLanguageManager` | `cRZLanguageManager` | `0x00AD14A0` | not counted | slot 7 `GetCurrentLanguage` `0x009DB663` |

Replaceability: the exemplar and cohort factories are registered with the resource manager by class ID and type
ID (§8.1); a replacement factory for `0x6534284A` / `0x05342861`, installed as §8.1 says, takes over all exemplar
loading. A replacement must keep the
effective-list semantics of §4.4, the parent failure rule of §4.2, first-four-byte signatures, and the converting
and raw accessor behaviour of §5 (callers outside the class use `AsX`/`RefX` on the variants it returns). Nothing
outside the factories reads exemplar memory directly *[inferred]*. The DBPF segment and QFS decoder are framework
classes; FreeCity4 in standalone mode replaces them wholesale (engine/formats), in plug-in mode it uses the
originals.

### 8.1 Resource-manager factory contract *[compat]*

The resource manager is system service `0x056B906E`, interface IID `0x656B8EFC` (`cGZPersistResourceManager`,
vtable `0x00AD89D0`). Slot numbers below are slots of that vtable. Factories are reached through IID
`0xA56B8F17`; resources through IID `0x456B8F1D` (`cIGZPersistResource`).

**Registration (slot 19, `0x00974407`).** Three arguments, in order: **factory class ID**, **resource type ID**,
**factory pointer (may be null)**. Returns true when registered.

- If the pointer is not null the manager AddRefs it and keeps that reference. If it is null, the manager asks GZCOM
  to create the class ID with IID `0xA56B8F17`; if that fails it returns false and registers nothing.
- It then adds two entries under its lock: class ID → factory, and type → class ID. Both tables are hash
  **multimaps**: a second registration never replaces or refuses an earlier one; it adds another entry next to the
  existing one with the same key (insert at `0x00973AC4` / `0x00973B37`).
- The game itself registers exactly two factories, in its resource library director's framework-hooks slot 4
  (PreAppInit, `0x00664540`): (`0x453429B3`, `0x6534284A`, null) for exemplars and (`0x053429C8`, `0x05342861`,
  null) for cohorts. The objects are created from those class IDs by GZCOM.

**Lookup (slot 21, `0x00974042`, arguments type and out pointer).** Find type → class ID, then class ID → factory;
on success store the factory in the out pointer with **one added reference** (the caller releases it) and return
true. Each step takes the **first** entry its table's `find` reaches. There is no fall-through: if the class ID
found for the type has no factory entry, the lookup fails even when a later registration for the same type exists.
Slot 22 (`0x00972D4E`) is the same lookup taking a resource key (it uses the key's type).

- Precedence between two registrations for one key is **not defined**: an equal key is inserted right after the
  first matching entry, so the earliest registration is found, but growing the table rehashes it by pushing each
  entry onto the front of its new bucket (`0x009978AF`), which reverses entries with the same key. Which one is
  found then depends on how many registrations happened in between.
- **Unregister (slot 20, `0x00973FD6`, one argument: class ID).** Removes **every** class-ID → factory entry for
  that class ID, releasing each factory; returns true if at least one was removed. It does **not** remove any
  type → class-ID entries.
- Nothing in the game calls slot 20 (no direct callers; the director's later hook slots are the no-op at
  `0x00648F00`). The game never unregisters its own factories; the manager's own shutdown releases what it holds.
- Consequence for a replacement (*[compat]* rule for plug-ins): to take over type `0x6534284A` or `0x05342861`,
  call slot 20 with the game's class ID (`0x453429B3` / `0x053429C8`), then register the replacement **under the
  same class ID** with a non-null pointer. A replacement registered under a new class ID is not reliably found: the
  type entry that lookup reaches may still name the old class ID, and after slot 20 that class has no factory, so
  loading that type fails. A plug-in that wants to restore the original at shutdown repeats the procedure with the
  original factory; it is not required to unregister anything.

**Loading a resource (slots 3, 46, 47).** `GetResource` (slot 3, `0x009732EB`, five arguments: key, requested IID,
out pointer, two u32 values passed through unchanged) first asks each registered resource cache for the key; on a
miss it calls slot 46 (`0x00972EAE`), which looks up the factory (slot 22), finds the segment holding the key
(slot 32), and calls slot 47 (`0x00972F1D`), then stores the new resource in the cache (slot 44) and finally
QueryInterfaces it for the requested IID into the caller's out pointer. Slot 47:

1. Asks the segment to open the record for **reading** (segment slot 21 `OpenRecord(key, &record, 1)`).
2. Calls the factory's slot 3 with (record, `0x456B8F1D`, &resource, u32, u32). The exemplar and cohort factories
   do not override slot 3; the shared implementation (`0x0097554F`) does:
   - under the factory's lock, get the record's key; if **both** trailing u32 values are 0, call slot 9
     `CreateInstance(type, IID, &out)` (three arguments); otherwise call slot 4 with the type, IID, out pointer and
     the two u32 values. The base slot 4 (`0x0097553B`, also not overridden by the two factories) **ignores** the
     two values and forwards to slot 9. For the exemplar and cohort factories the trailing values therefore have no
     effect.
   - give the new object its key (resource slot 4), then call slot 5 `Read(resource, record)` (`0x00975495`),
     which QueryInterfaces the record for the serial-record IID `0xC56B8F08` and calls slot 10
     `SerialRead(resource, serial record)` (`0x0067F1E0` / `0x0067F7D0`).
   - if the read succeeds, QueryInterface the object for the IID asked for and return that result; in every case
     release the factory's own reference to the object.
3. If step 2 succeeded, closes the record (segment slot 24 `CloseRecord`, `0x00975845`) and releases it; if the
   open or step 2 failed, aborts the record (segment slot 25 `AbortRecord`, `0x00975811`). **The factory never
   closes the record.**

**`CreateInstance` (slot 9, `0x0067D820` / `0x0067D8D0`).** Arguments: resource **type ID**, requested **IID**,
**out pointer**; it takes no record and no key. If the type is not the factory's own (`0x6534284A` /
`0x05342861`) it stores null and returns false. Otherwise it creates an empty object (`cSCResExemplar`, 64 bytes /
the cohort) and QueryInterfaces it for the IID; on success the out pointer holds the reference from that
QueryInterface and it returns true. On an **unknown IID** it destroys the new object, stores null and returns
false.

**Reference ownership.** Each level owns the reference it receives and releases its own: the factory releases the
object after its QueryInterface for the caller, slot 47 releases after its QueryInterface, and `GetResource`
releases its local reference after storing the object in the cache (the cache keeps its own) and QueryInterfacing
it for the caller. The caller of `GetResource` ends up with exactly **one** reference, which it must release.

**The record given to `SerialRead`.** It is a fresh record opened for this read, at read position **0** of the
resource, and its bytes are **already decompressed**: when the key is listed in the DIR (§2.5) the packed-file
segment decompresses the entry while opening the record (`0x009701FD`, through `DecompressRecord`) and the record
reads from that buffer (constructor `0x0096DA30`, position field set to 0). Unlisted entries are read as stored,
either buffered in full or, above a size threshold of the segment, straight from the file. A factory must not look
for or decode a QFS stream itself.

**Object identity.** The manager depends only on the object answering `0x456B8F1D` through QueryInterface and on
resource slot 4 (set key); it never checks the class. Readers of exemplars reach them through QueryInterface for
`cISCResExemplar` and `cISCPropertyHolder` (table above). A replacement object must therefore answer those IIDs with
the contracts of §4, §5 and the table above; that no reader goes past these interfaces stays *[inferred]* (the
callers were not audited one by one).

---

## 9. Quirks and choices (summary)

| # | Behaviour | Tag |
|---|---|---|
| Q1 | Index minor ignored, entries always 20 bytes; 7.1 packages fail to open | *[compat]* |
| Q2 | One out-of-range index entry fails the whole package | *[compat]* |
| Q3 | First duplicate wins inside a package and in the DIR; last listed wins in a multi segment | *[compat]* |
| Q4 | QFS: only `10 FB` / `11 FB`; DIR-listed data that fails the check fails the read | *[compat]* |
| Q5 | QFS decoder without bounds checks | *[quirk]*, FreeCity4 checks *[choice]* |
| Q6 | Exemplar signature: first 4 bytes only | *[compat]* |
| Q7 | Binary property = kind byte + §9.2 tag; non-zero "unused" byte fails the read | *[compat]* |
| Q8 | Unresolvable parent (or kind-1 resource) fails the exemplar | *[compat]* |
| Q9 | Effective list snapshot; no depth limit; cycles recurse without end | *[compat]*, *[inferred]* for cycles |
| Q10 | Text form: 2 bytes after the signature skipped; type names case-sensitive; key order group, instance, type; 64-bit values hex | *[compat]* |
| Q11 | Converting getters: arrays give their first element through a text round trip; floats truncate | *[compat]* |
| Q12 | `AsX`/`RefX` reinterpret without checks | *[compat]* *[quirk]* |
| Q13 | LTEXT read as a §2.3 string: 24-bit count, trailing bytes ignored | *[compat]* |
| Q14 | `SC4LoadString` tries group + language ID first | *[compat]* |

---

## 10. Conformance tests

Byte strings are hexadecimal. "Package P(entries)" means: header with magic `DBPF`, major 1, minor 0, index major
7, index minor 0, the given entries packed after the 96-byte header in order, index after the data, no holes.

### 10.1 DBPF

1. **Minimal package** (`0x78` bytes). Header: `44 42 50 46 01 00 00 00 00 00 00 00`, 20 zero bytes, then at
   `0x20` `07 00 00 00 01 00 00 00 64 00 00 00 14 00 00 00` (index major 7, count 1, offset `0x64`, size 20),
   then zeros to `0x5F`. At `0x60` the data `41 42 43 44`; at `0x64` the entry
   `78 56 34 12 00 00 00 00 01 00 00 00 60 00 00 00 04 00 00 00`. Open succeeds; key (`0x12345678`, 0, 1) reads
   `41 42 43 44`.
2. Same as 1 with major version 2 at `0x04`: the package does not open (no embedded header).
3. Same as 1 with major 0 and minor 7: opens (minor not checked).
4. Same as 1 with index offset `0x65`: `0x65 + 0x14 > 0x78`, the package does not open.
5. Same as 1 with the entry offset `0x10`: the package does not open (offset < `0x60`), although the entry is
   never read.
6. Same as 1 with the entry size 5: the check is against the **file** length, `0x65 ≤ 0x78`, so the package opens
   and the read returns `41 42 43 44 78` (the last byte is the first index byte).
7. **First duplicate wins**: data `41 41 41 41` at `0x60` and `42 42 42 42` at `0x64`, two index entries with key
   (`0x12345678`, 0, 1) pointing at `0x60` then `0x64`: the read returns `41 41 41 41`.
8. **7.1 index**: same as 1 but index minor 1 and a 24-byte entry
   `78 56 34 12 00 00 00 00 01 00 00 00 00 00 00 00 60 00 00 00 04 00 00 00` (resource ID 0, index size `0x18`):
   the original reads offset 0 and the package does not open.
9. **Holes**: test 1 plus a hole table of one entry appended at `0x78` (header: hole count 1, table offset `0x78`,
   size 8; file `0x80` bytes). Entry `10 00 00 00 04 00 00 00`: opens (entries are not checked). Table offset
   `0x10` (with the table bytes there): does not open. Hole count 2 with table size 8: does not open. Hole count 0
   with any table fields that pass §2.1: opens.
10. **Embedded header**: 32 bytes of `00`, the 16-byte marker of §2.2, then the 120-byte package of test 1 (its
    offsets unchanged): opens, the key reads `41 42 43 44`.

### 10.2 DIR and QFS

11. **QFS, short copy**: stored entry `11 00 00 00 10 FB 00 00 0C E0 41 42 43 44 14 03 FC` (17 bytes), listed in the
    DIR with size 12: reads `41 42 43 44` × 3.
12. **QFS, long copy**: `13 00 00 00 10 FB 00 01 30 E0 41 42 43 44 C4 00 03 27 FC` (19 bytes): code `C4 00 03 27`
    copies 300 bytes from distance 4; result `41 42 43 44` × 76 (304 bytes).
13. **QFS, `11 FB`**: `12 00 00 00 11 FB 00 00 0E 00 00 04 E0 41 42 43 44 FC`: the 3 bytes after the signature are
    skipped, size 4, result `41 42 43 44`.
14. **Rejected signatures** (entry listed in the DIR): `10 FC …`, `90 FB …`, `30 FB …`, or fewer than 7 stored
    bytes: the read fails.
15. **Unlisted QFS-looking data**: the bytes of test 11 under a key that is not in the DIR read back unchanged
    (17 bytes).
16. **DIR record size**: a DIR entry of 32 bytes lists two keys (16 bytes each); a DIR entry of 20 bytes lists one
    key (the remaining 4 bytes are ignored).
17. **Prefix limit**: the bytes of test 11 with the prefix `01 00 00 01` (`0x01000001`, above 16 MiB): the read
    fails. (The 24-bit uncompressed size can never exceed the limit.)

### 10.3 Load order

These tests check the resource manager and the startup registration (§3, §3.1), not the package reader.
*[choice]* They are out of scope for `engine/formats` and belong to the module that implements §3.1.

18. Install `SimCity_1.dat` and `Plugins\a.dat` both define exemplar K with property `0x10` = 1 and 2: the game
    sees 2.
19. `Plugins\a.dat` (K → 2) and `Plugins\b.dat` (K → 3): 3. `Plugins\sub\a.dat` (K → 4) added: 4.
20. `Plugins\z.SC4Lot` (K → 5) and `Plugins\a.dat` (K → 2): 2 (`.dat` wins).
21. Install `Plugins\a.dat` (K → 2) and user `Plugins\a.dat` (K → 6): 6.
22. A city save containing K (→ 7) while `SimCity_1.dat` has K (→ 1): 1. With no other K: 7.

### 10.4 Exemplars

Header used below: `45 51 5A 42 31 23 23 23` (`EQZB1###`), parent `00`×12, then the count.

23. **Scalar Uint32**: count `01 00 00 00`, property `10 00 00 00 | 00 | 03 00 00 00 | 07 00 00 00`:
    property `0x10` = Uint32 7.
24. **Uint32 array**: `20 00 00 00 | 00 | 03 80 00 00 | 02 00 00 00 | 05 00 00 00 06 00 00 00`: Uint32 array
    {5, 6}, count 2.
25. **String**: `20 00 00 00 | 00 | 0C 80 00 00 | 05 00 00 00 | 48 65 6C 6C 6F`: string "Hello".
26. **UTF-16 string**: `20 00 00 00 | 00 | 0C 80 00 00 | 02 00 00 10 | 48 00 69 00`: string "Hi".
27. **Scalar 0x0C**: `20 00 00 00 | 00 | 0C 00 00 00 | 48`: one RZChar `H` (memory type `0x0E`, count 0), not a
    string; the record is 1 byte shorter than a string reader would expect.
28. **Unused byte set**: `10 00 00 00 | 00 | 03 80 00 01 | …`: tag `0x01008003` is invalid, the read fails.
29. **Unknown kind**: `10 00 00 00 | 02 | …`: the read fails.
30. **Signature**: `EQZB2###` and `EQZBabcd` read as test 23; `EQZA1###` fails; a cohort record with
    `EQZB1###` fails; an exemplar record stored under type `0x05342861` fails.
31. **Duplicate IDs**: count 2, `10 00 00 00 00 03 00 00 00 07 00 00 00` then
    `10 00 00 00 00 03 00 00 00 09 00 00 00`: property `0x10` = 9.
32. **Missing parent**: parent `61 28 34 05 | 01 00 00 00 | 02 00 00 00` (cohort `0x05342861`, 1, 2) not
    present: the exemplar fails to load (lookups behave as if it did not exist).
33. **Parent of exemplar type**: parent (`0x6534284A`, 1, 2) present as an exemplar: the read fails
    *[inferred]*.
34. **Inheritance**: cohort C (`CQZB1###`, no parent) has `0x10` = 1, `0x11` = 2; exemplar E (parent C) has `0x11`
    = 5, `0x12` = 6. E: `0x10` → 1, `0x11` → 5, `0x12` → 6; `IsPropertyLocal(0x10)` false, `(0x11)` true;
    `GetPropertyList` yields IDs `0x10, 0x11, 0x12` in that order.
35. **Three levels**: cohort A (`0x10` = 1), cohort B (parent A, `0x11` = 2), exemplar E (parent B, `0x10` = 3):
    E `0x10` → 3, `0x11` → 2.
36. **Trailing bytes**: test 23 followed by `FF FF`: reads as test 23.
37. **Text**:
    ```
    EQZT1###\r\nParentCohort=Key:{0x00000000,0x00000000,0x00000000}\r\nPropCount=0x00000003\r\n
    0x00000010:{"A"}=Uint32:0:{0x00000007}\r\n
    0x00000011:{"B"}=Float32:2:{1.5,2}\r\n
    0x00000012:{"C"}=String:0:{"Hi"}\r\n
    ```
    gives `0x10` Uint32 7, `0x11` Float32 array {1.5, 2.0}, `0x12` string "Hi".
38. Text variants: `PropCount=3` (decimal) works; `PropCount=10` is ten; `PropCount=1A` is 26; type `uint32`
    fails; `Uint32:1:{7}` gives a one-element array; `Sint64:0:{1356}` gives `0x1356`; `Bool:0:{TRUE}` true,
    `Bool:0:{yes}` false; `ParentCohort=Key:{0x0A64ED55,0x00000001,0x05342861}` asks for cohort
    (`0x05342861`, `0x0A64ED55`, 1).
39. Text without the 2 bytes: `EQZT1###ParentCohort=…` loses `Pa`; the first line is `rentCohort=…` and the read
    fails.

### 10.5 Getters

40. Uint32 array {5, 6}: `GetProperty(id, uint32&)` → true, 5. Float32 array {2.75} → 2. Float32 scalar −1.5 →
    `0xFFFFFFFF`. Bool scalar true → 1. String "0x1F" → 31. String "12" → 12. Empty Uint32 array → 0.
41. `GetValFloat32` on Uint32 scalar 16,777,217 → 16,777,216.0 (rounded to float32).
42. `AsUint32()` on Uint32 scalar 7 → 7; `GetCount()` → 0. On a one-element Uint32 array: `GetCount()` → 1,
    `RefUint32()[0]` → the element.
43. Missing property: `GetProperty(id, uint32&)` → false, output unchanged.

### 10.6 LTEXT

44. `04 00 00 10 48 00 69 00 21 00 21 00`: "Hi!!".
45. `03 00 00 00 41 42 43`: "ABC". `03 00 00 00 41 42 43 44`: "ABC" (trailing byte ignored).
46. `02 00 00 00 41 00`: second byte 0, decoded as UTF-16 *[quirk]* (FreeCity4 standalone: "A\0" is invalid; the
    header decides *[choice]*).
47. `41 42 43 44 45` (no header; encoding byte `0x44`): legacy path, the whole record `ABCDE`.
48. Header with count 200,001: the read fails.
49. Shipped: (`0x2026960B`, `0x6A231EAA`, `0x0A6DF4DF`) is 8-bit text beginning `Don't localize.` (header
    `90 00 00 00`, 144 bytes) in both `English\SimCityLocale.DAT` and `Plugin_002_TheLivingMall.dat`.
50. `SC4LoadString(out, G, I)` with language ID 1: key (`0x2026960B`, G + 1, I) is used if present, else
    (`0x2026960B`, G, I).

### 10.7 Text exemplar and getter details

Text lines below go into a record shaped like test 37, with `PropCount` set to match.

51. **ParentCohort fields** (§4.3): `Key:{0,0}` gives three zeros, so no parent.
    `Key:{0x0A64ED55,0x00000001,0x05342861,0x99}` asks for cohort (`0x05342861`, `0x0A64ED55`, 1); the fourth
    field is ignored. `Key:{0x0A64ED55, 0x00000001, 0x05342861}` asks for (0, `0x0A64ED55`, 0).
52. **reps and type name**: `Uint32: 0x2:{1,2}` gives a scalar 1. `Uint32:4:{1,2}` gives {1, 2, 0, 0}.
    `Uint32:1:{1,2}` gives {1}. `Uint32 :0:{1}` and `Uint32:0: {1}` fail the read.
53. **Values**: `Uint32:0:{ 0x10}` gives 0, and `Uint32:1:{ 0x10}` gives {16}. `Uint64:0:{10}` gives 16, and
    `Sint64:0:{-1}` gives 0. `Bool:0:{1}` gives false.
54. **Float32 text** (main thread, 24-bit): `Float32:0:{0.05}` gives the bits `0x3D4CCCCC`. `{1e10}` gives
    1,410,065,408.0, `{1e32}` gives 0.0 and `{inf}` gives 0.0.
55. **Too few lines**: `PropCount=3` followed by two property lines fails the read.
56. **Round-trip parse** (§5.1), `GetProperty(id, uint32&)` on string properties: `0X1F` gives 0, `TRUE` gives
    0, `true` gives 1, `12 apples` gives 12, `-5` gives `0xFFFFFFFB`, `1.2.3` gives 1, `1e3` gives 1000, and
    `.5` gives 0. `GetValBool` on the string `0.5` gives true. `GetValRZChar` on the string `A` gives 0 and true.
57. **Scalar conversions** (§5.1): Float32 300.7 read as Uint8 gives 44. A Float32 NaN read as Uint32 gives 0,
    and read as Bool gives true. RZChar `0xE9` read as Uint32 gives `0xFFFFFFE9`. `GetValFloat64` on Uint64
    16,777,217 gives 16,777,216.0.
58. **Float text round trip**: Float32 `0x40490FD0` (3.14159) read with `GetProperty(id, cIGZString&)` gives
    `3.1415901184082031` and false. A string holding that text, read with `GetValFloat32`, gives `0x40490FCE`.
59. **Empty and void**: a tag-0 property read with `GetProperty(id, uint32&)` gives true and 0, and with
    `GetProperty(id, cIGZString&)` gives false and the empty text. A void array `31 32` read with
    `GetProperty(id, uint32&)` gives true and 0. Read with slot 5 and `length` 8, it copies `31 32` and sets
    `length` to 2.

---

## 11. Original versus engine/formats

| engine/formats (README, source) | Original | Change |
|---|---|---|
| `Package::find` returns the **last** duplicate | first index entry wins (§2.3) | return the first |
| index 7.1 (24-byte entries) supported; entry size from `index size / count` | always 20 bytes; 7.1 packages do not open (§2.1) | standalone may keep 7.1 as a *[choice]* extension, flagged |
| entries outside the file open with a warning, fail when read | the whole package fails to open (§2.3) | fail the open (or *[choice]* warn, but then document) |
| DIR entry size 16 or 20 by index version | always 16 (§2.5) | always 16 |
| DIR-listed entry without a QFS signature returned as stored | the read fails (§2.6) | fail |
| QFS accepts flag bits `0x80`/`0x01` | only `10 FB` / `11 FB`; size ≤ 16 MiB (§2.7) | reject others |
| exemplar signature requires byte 4 = `'1'` | only the first 4 bytes are checked (§4.1) | relax |
| value type `u16` + key type `u16` + unused byte | kind byte + `u32` tag; unused/key-high bytes must be 0; kind 1 = resource key (§4.2) | reject non-zero, read kind 1 |
| key type 0 string | tag `0x000C` is one RZChar byte (§4.2) | follow the original |
| missing parent: chain resolution tolerant | exemplar fails to load (§4.2) | fail the exemplar |
| `MAX_PARENT_DEPTH` = 32, cycles end the walk | no limit; cycles recurse (§4.4) | keep as *[choice]*, treat as load failure |
| lookups return the last duplicate property | same (last wins) | confirmed |
| text parent key order group, instance, type | same (§4.3) | confirmed |
| text: 2 bytes after the signature | skipped unchecked (§4.3) | match |
| LTEXT `u16` count + `u16` marker, 8-bit form must have exact length | 24-bit count, §2.3 quirks, trailing bytes ignored (§6.1) | relax length; 24-bit count |
| getters on Empty and object values return None (QUESTIONS EX1, EX5) | Empty reads as 0, 0.0 or false and succeeds; the string getter writes the text and returns false; an object gives its address (§5.1) | follow for Empty; object as missing *[choice]* |
| Uint64 to float unsigned (EX2) | unsigned, but only 24 significant bits on the main thread, even for Float64 (§5.1) | round to 24 bits |
| float to integer saturates, NaN gives 0 (EX3) | truncated to int64, low bits kept (wraps); NaN and ±∞ give `0x8000000000000000` (§5.1) | follow |
| RZChar zero-extended (EX4) | Char and RZChar sign-extended (§5.1) | follow |
| void array as text: first byte as `%u` (EX6) | empty text, so 0 (§5.1) | follow |
| floats as text with `%.24g` (EX8) | fixed-point, 17 significant digits rounded half-up, trailing fraction zeros dropped, cut at 63 characters (zeros stripped again when the cut falls among the digits); below 10⁻⁴⁸ rounded half-up again at the 64th place (EX34); no exponent (§5.1, EX29) | follow |
| round-trip parse (EX9) | as bracketed, plus: char targets always get 0; Bool from floating-point text is value ≠ 0 (§5.1) | follow |
| text ParentCohort: exactly 3 fields, leading spaces trimmed (EX10) | fields not counted, a missing one reads 0; nothing trimmed inside the braces (§4.3) | follow |
| text reps trimmed (EX11) | not trimmed; rule N (§4.3) | follow |
| more text reps than the record length fails (EX12) | no cap; missing values read 0; a huge reps crashes (§4.3) | fill with 0; *[choice]* fail above 16 MiB |
| Float32 text parsed as f64, then rounded to f32 (EX14) | the game's digit-by-digit parser at 24-bit precision; 10ⁿ wraps in 32 bits (§4.3) | follow |
| kind-1 key resolvable when in a package index (EX15) | needs a factory for the type and a successful read (§4.6) | follow |
| holder slot 5 not implemented (EX17) | raw copy from a void array; false for every other type (§5.2) | implement (always false on shipped data) |
| factories run eagerly in `load_package` (EX31) | lazily, at the first `GetResource` of an uncached key, from the segment winning then; failures are read again (§3) | eager allowed under the three conditions of §3 *[choice]* |

---

## 12. Open questions

1. How the current language ID is chosen at startup (registry `Language` value, command line, folder probing) and
   the full ID → directory table.
2. ~~Whether the resource cache can hold a stale object after a plugin segment is added during a session (not done
   by the shipped game).~~ Answered in §3 (EX37): it can. The default cache keeps the first record of every key
   until the key is removed or purged; only the callers of the purge are untraced.
3. Cycle behaviour of cohort chains (§4.4) is inferred, not observed.
4. The QFS 4-byte prefix is checked only against 16 MiB; whether any path compares it with the index size.
