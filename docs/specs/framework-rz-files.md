# Framework services (RZ): files, INI files, directories, file streams, command lines

Behaviour spec for the RZ framework's file object (`cRZFile`, the `cIGZFile` implementation) and the
INI reader/writer built on it (`cRZIniFile`). Written from the exe (SimCity 4 Deluxe 1.1.641) by lane
re-rzservices4, 2026-10-21. Evidence: Ghidra export (`tools/re_query.py asm`), decomp/ pseudocode,
vtables and string constants read from `bin/SimCity4.exe`. The REA session was unavailable this round
("No app is open"), so no REA pseudocode was used. Addresses are VAs in that exe.

Outline: §1 owners and identity. §2 cRZFile (§2.1 layout, §2.2 vtable, §2.3 open, §2.4 position and
seek, §2.5 read buffer, §2.6 write buffer, §2.7 close, length, flush, §2.8 errors, §2.9 path-level
operations, §2.10 static helpers). §3 cRZIniFile (§3.1 layout and ownership, §3.2 vtable, §3.3 file
syntax as read, §3.4 open and lifetime, §3.5 reads and types, §3.6 writeback, §3.7 enumeration).
§4 worked examples. §5 open questions. Round 3: §6 cRZDirectory (§6.1 layout, §6.2 separators,
§6.3 setters, §6.4 enumeration, §6.5 string lists and sort, §6.6 recursive listing, §6.7 path helpers); §7 cRZOFStream (§7.1 layout and lifetime, §7.2 vtable,
§7.3 number writers, §7.4 strings and flushing); §8 cRZCmdLine (§8.1 layout, §8.2 tokenising, §8.3 vtable,
§8.4 switch values).

## 1. Owners and identity

| Owner | Bytes (coverage) | Identity |
|---|---|---|
| `cRZFile` | 8,216 | vtable `0x00ABFED0`, 33 slots, object 0x80 bytes, interface `cIGZFile` iid `0xE020364F` (QueryInterface also accepts iid 1, the base) |
| `cRZIniFile` | 6,617 | vtable `0x00ACF150`, 20 slots, no QueryInterface (slot 0 is the deleting destructor); `cHitIniFile` (vtable `0x00AD23B8`) derives from it |

Sparse matrix/column iterator behavior is separately bounded in [rz-sparse-matrix.md](rz-sparse-matrix.md).
The compiled `cRZSysServPtr` utility bodies have a separate bounded contract in
[system-service-pointer.md](system-service-pointer.md).
Not in this spec yet:
`cRZTimer` (same subsystem, later rounds); the language utility (docs/specs/language-utility.md, owned
by the strings lane).

Text encoding: paths and INI text are byte strings in the framework's multibyte encoding (UTF-8; the
framework code page is the constant 8, see language-utility.md §1). Paths are converted to the OS
encoding just before each OS call: the wide-character (`...W`) OS entry points are used when the
framework found them at start-up, otherwise the ANSI ones.

## 2. cRZFile

### 2.1 Layout

| Offset | Meaning | Initial |
|---|---|---|
| +0x04 | path (`cRZString`, 0x14 bytes; char data at +0x08..+0x0C) | empty or ctor argument |
| +0x18 | open flag (byte) | 0 |
| +0x1C | OS handle | -1 |
| +0x20 / +0x24 / +0x28 | access, creation, share modes passed to Open | 0 |
| +0x2C | reference count | 0 |
| +0x30 | last error code | 0x20000000 |
| +0x34 | second string, unused by the methods specified here | empty |
| +0x48 | logical position (what the caller sees) | 0 |
| +0x4C | physical position (where the OS file pointer is) | 0 |
| +0x50 | read buffer capacity | 0x200 |
| +0x54..+0x5C | read buffer storage (allocated on first fill) | none |
| +0x60 | file offset of the read buffer's first byte | 0 |
| +0x64 | valid bytes in the read buffer (0 = buffer empty) | 0 |
| +0x68 | write buffer capacity | 0x200 |
| +0x6C..+0x74 | write buffer storage (allocated on first buffered write) | none |
| +0x78 | file offset the write buffer flushes to | 0 |
| +0x7C | pending bytes in the write buffer | 0 |

Constructors: default (0x00918B96), from a `cIGZString` path (0x00918C00), from a C string path
(0x00919A60; a null pointer leaves the path empty). The destructor (0x00918C70) closes the file if open
(flushing pending writes) and frees both buffers. Reference counting: AddRef increments +0x2C and returns
it; Release decrements when the count is above 1 and returns the new count, otherwise calls the deleting
destructor and returns 0 (so Release on a count of 0 or 1 destroys).

### 2.2 vtable 0x00ABFED0

| Slot | Address | Behaviour |
|---|---|---|
| 0 | 0x009182FF | QueryInterface (iids 0xE020364F and 1; AddRef on success) |
| 1, 2 | 0x00918324, 0x0091832B | AddRef, Release (§2.1) |
| 3 | 0x00919B00 | Open(access, creation, share) (§2.3) |
| 4 | 0x004770F0 | IsOpen: the +0x18 flag |
| 5 | 0x009188A5 | Close (§2.7) |
| 6 | 0x00918345 | Position: logical position, or -1 when closed |
| 7 | 0x009186F5 | Length (§2.7) |
| 8 | 0x009188F1 | SetLength(n) (§2.7) |
| 9, 10, 11, 12 | 0x00918352, 0x0091835C, 0x00918366, 0x00918374 | SeekToBegin = Seek(0, begin); SeekToEnd = Seek(0, end); SeekRelative(n) = Seek(n, current); SeekToPosition(n) = Seek(n, begin) |
| 13 | 0x0091895A | Seek(offset, mode) (§2.4) |
| 14 | 0x00918382 | Read(buf, n): ReadWithCount, true only if it succeeded and all n bytes arrived |
| 15 | 0x009192A9 | ReadWithCount(buf, &n) (§2.5) |
| 16 | 0x009183AD | Write(buf, n): WriteWithCount, true only if it succeeded and n is unchanged |
| 17 | 0x009193E3 | WriteWithCount(buf, &n) (§2.6) |
| 18 | 0x009189E2 | Flush (§2.7) |
| 19 | 0x00919CD8 | Remove: closes if open, deletes the file at the path (§2.9) |
| 20 | 0x0091AB4E | Rename(newPath) (§2.9) |
| 21 | 0x0091AB92 | Exists (§2.9) |
| 22 | 0x0091ABDE | CopyFrom(cIGZFile& source, overwrite) (§2.9) |
| 23 | 0x0091ABB2 | CopyFrom(cIGZString& sourcePath, overwrite) (§2.9) |
| 24 | 0x0091C570 | path as `cIGZString*` (the +0x04 object) |
| 25 | 0x0091880E | GetPath(cIGZString& out): out := path |
| 26 | 0x009C32AC | path as a C string (the +0x08 data pointer) |
| 27 | 0x00919D88 | SetPath(const char*): ignored when null |
| 28 | 0x00919474 | SetPath(cIGZString&) |
| 29 | 0x009183D8 | GetOpenModes(&access, &creation, &share): false when closed, else the stored modes |
| 30 | 0x00918402 | GetBufferSizes(&read, &write) |
| 31 | 0x00919081 | SetBufferSizes(read, write) (§2.5) |
| 32 | 0x0081D150 | deleting destructor |

Changing the path (slots 27, 28) does not close an open file; the handle keeps referring to the old file.

### 2.3 Open (0x00919B00)

- Already open: returns true at once and changes nothing (the new modes are ignored; the file is not
  reopened). To change modes a caller must Close first.
- Access mode bits: 1 = read, 2 = write (both may be set). Share bits: 1 = others may read; 2 = others
  may read and write (bit 2 alone already grants both). 0 = exclusive.
- Creation mode: 0 create new (fail if it exists), 1 create always (truncate), 2 open existing, 3 open
  always (create if missing), 4 truncate existing. Any other value: returns false, sets no error.
- Path handling: a path whose extension is `.lnk` (case-insensitive) is resolved as a Windows shell
  shortcut and the shortcut's target is opened; if resolution fails the `.lnk` file itself is opened.
- Success: open flag set; modes stored; logical and physical positions := the OS position (0); both
  buffers emptied. With creation modes 1 and 3 the OS "already exists" status is recorded in the error
  field (0x20000001 if the file existed, otherwise the OS code, normally 0). Returns true.
- Failure: records the OS error (§2.8), returns false; the object stays closed.

### 2.4 Position and Seek (0x0091895A)

Seek(offset, mode) with mode 0 = from start, 1 = from current, 2 = from end:
- Closed: returns -1.
- Mode 1 is relative to the logical position: when logical and physical positions differ (read buffer
  ahead, or writes pending), the target is converted to an absolute position (logical + offset) first.
- Any other mode value does not seek and returns Position().
- Otherwise pending writes are flushed, the OS pointer is moved, and logical = physical = the new
  position, which is returned. On OS failure both positions are re-read from the OS, the error is
  recorded and -1 is returned.
- Seeking does not discard the read buffer; a later read reuses it if the new logical position falls
  inside it (§2.5).

### 2.5 Reading (ReadWithCount 0x009192A9) and buffer sizes

ReadWithCount(buf, &n):
- Closed: false. n = 0: true.
- Pending writes are flushed first.
- Read buffer capacity 0 (unbuffered): one OS read of n bytes; n := bytes read; positions advance;
  result is the OS result (a short read at end of file is a success with smaller n).
- Buffered: if the logical position lies inside the buffered window [+0x60, +0x60 + +0x64), copy as much
  as available from it. While bytes remain: empty the buffer, seek the OS pointer to the logical position
  if the physical position differs (failure ends with false), fill the buffer with one OS read of the
  full capacity at the current position, copy min(remaining, filled) bytes. A fill of 0 bytes (end of
  file) stops with success. A read larger than the buffer goes through the buffer in capacity-sized
  steps. n := bytes delivered.
- Read (slot 14) is the all-or-nothing form.

SetBufferSizes(read, write) (0x00919081), the same rule for each size: -1 means 0x200; the value is
rounded down to even; a non-zero value below 4 becomes 4; values above 0x1F4000 (2,048,000) are clamped.
0 disables that buffer. Shrinking the read buffer empties it; shrinking the write buffer flushes it.
Always returns true.

### 2.6 Writing (WriteWithCount 0x009193E3, FlushWriteBuffer 0x009187B1)

WriteWithCount(buf, &n):
- Closed: false.
- A non-empty read buffer is discarded, and the OS pointer is moved back to the logical position if it
  differs.
- Write buffer capacity 0: one OS write; n := bytes written; positions advance; OS result returned.
- Buffered: bytes are appended to the write buffer; whenever it becomes full it is flushed with one OS
  write. The logical position advances by n even if a flush failed; n is not reduced. Returns false only
  when a flush failed (remaining bytes are then dropped).
- FlushWriteBuffer: if bytes are pending, one OS write at the physical position; success advances the
  physical position by the bytes written; failure re-reads the physical position from the OS and
  returns false. Pending count := 0 either way (data of a failed flush is lost).

### 2.7 Close, Length, SetLength, Flush

- Close (0x009188A5): closed: false. Otherwise flush pending writes, empty both buffers, close the
  handle (failure is recorded but Close still returns true), handle := -1, modes and physical position
  := 0, open flag cleared. The logical position keeps its last value but Position() reports -1.
- Length (0x009186F5): closed: -1. Otherwise the OS file size (32-bit); on OS failure -1 with error
  recorded. While writes are pending and the OS size is below the logical position, the logical
  position is returned instead.
- SetLength(n) (0x009188F1): closed: false. Empties the read buffer, flushes, moves the OS pointer to n
  and sets end of file there; logical = physical = the OS position afterwards (so n on success).
- Flush (0x009189E2): closed: false. Flushes the write buffer; only if bytes were pending it also asks
  the OS to commit file buffers. Returns false when that commit failed, otherwise true (a failed buffer
  write alone does not make Flush return false).

### 2.8 Error codes (recorder 0x00918577)

Every failing OS call stores a code in +0x30: OS error 5 (access denied) becomes 0x20000002, OS error 183
(already exists) becomes 0x20000001, any other OS error is stored unchanged. The initial value is
0x20000000. No vtable slot returns this field; it is observable only to code that reads the object.

### 2.9 Path-level operations (vtable)

All of these work on the stored path, record the OS error on failure and return true/false.
- Remove (slot 19): closes the file if open, then deletes it.
- Rename(newPath) (slot 20): closes if open, moves the file (with the wide API the move may copy across
  volumes), then sets the stored path to newPath whether or not the move succeeded.
- Exists (slot 21): true if the path names an existing file.
- CopyFrom (slots 22, 23): copies the source file onto this object's path; when `overwrite` is false the
  copy fails if this path already exists. The object is not opened.

### 2.10 Static helpers (by address)

| Address | Behaviour |
|---|---|
| 0x00919DC2 FileCreate(path, truncate) | open-always with read access, share read+write; if `truncate`, cut to 0 bytes; close; true if the file could be opened |
| 0x00919F17 GetAttribute(path, flags) | true only if every requested flag holds: 0x02 writable (not read-only), 0x08 directory, 0x10 resolvable `.lnk` shortcut (skipped when 0x400 is also set), 0x20 archive, 0x40 hidden, 0x80 system, 0x1000 read-only. A missing file reads as all attribute bits set |
| 0x0091A035 SetAttribute(path, flags, on) | applies 0x02 (on = clear read-only), 0x20, 0x40, 0x80, 0x1000 (on = set read-only); flags 0x08 or 0x10 make it fail; no OS call when nothing changes |
| 0x0091A1A6 Remove(path), 0x0091A225 Rename(old, new), 0x0091A2F9 Copy(src, dst, overwrite) | OS delete / move / copy (`overwrite` false = fail if dst exists) |
| 0x0091A3E2 GetLength(path) | size of the first directory match for the path, -1 if none |
| 0x0091AC34 IsFileWritable(path) | missing file: try to create it, delete it again, true if the create worked; existing file: true if it opens with write access, open-existing, share read |
| 0x00918D35 GetTempDirectory(out) | the OS temporary directory, long-name form |

## 3. cRZIniFile

### 3.1 Layout and ownership

| Offset | Meaning |
|---|---|
| +0x04 | INI path (`cRZString`) |
| +0x18 | leave-file-open flag (byte, initial 1) |
| +0x19 | section index loaded flag (byte, initial 0) |
| +0x1C.. | section index: lower-cased section name -> file offset of its header line |
| +0x24.. | display names: lower-cased name -> name as first written in the file |
| +0x34 | the `cIGZFile` used for I/O (reference held) |
| +0x38 | the file is open for writing (byte) |

Constructors: default (0x0091CE6E) creates its own `cRZFile` with an empty path; from a path (0x0091CEE3)
stores the path and creates a `cRZFile` on it; from a `cIGZFile*` (0x0091CF46) uses that file (AddRef),
or creates its own when null. The destructor (0x0091CDE3) closes the file if open and releases it.
SetFile(file) (slot 4): when different from the current file, closes the INI (§3.4), AddRefs the new
file, releases the old one; a null file is replaced by a new `cRZFile` given the INI path. Always true.
SetPath (slot 2): closes the INI if its index is loaded, then stores the path (the file's own path is set
the next time the file is opened, §3.4). GetPath (slot 1) returns the stored path; slot 3 returns the file.

### 3.2 vtable 0x00ACF150

| Slot | Behaviour |
|---|---|
| 0 | deleting destructor |
| 1, 2, 3, 4 | GetPath, SetPath, GetFile, SetFile (§3.1) |
| 5 | Close (§3.4) |
| 6 | ReadEntry(section, key, out string) (§3.5) |
| 7 | ReadNumber(section, key, out long) (§3.5) |
| 8 | ReadBinaryEntry(section, key, buffer, size) (§3.5) |
| 9 | ReadBinaryEntry(section, key, out allocated pointer) (§3.5) |
| 10 | WriteEntry(section, key, value) (§3.6) |
| 11 | WriteNumber(section, key, long) (§3.6) |
| 12 | WriteBinaryEntry(section, key, data, size) (§3.6) |
| 13 | EnumEntries(section, callback, context) (§3.7) |
| 14 | EnumSections(callback, context) (§3.7) |
| 15 | SectionExists(section) |
| 16 | SetLeaveFileOpenBetweenOperations(flag) |
| 17 | LoadSectionNames(forWrite) (§3.4) |
| 18 | GetFileLine(out string) (§3.3) |
| 19 | Open(forWrite) (§3.4) |

### 3.3 File syntax as read

Lines (GetFileLine 0x0091CA15): a line is the bytes up to the first CR or LF. After it, every following
CR and LF byte is consumed, so CRLF, LF, CR and any run of blank lines all act as one separator: blank
lines are never seen by the parser. The last line need not end in a newline. Lines have no length limit
(read in 40-byte chunks). There is no byte-order-mark handling: a UTF-8 BOM stays at the start of the
first line.

Each line is left-trimmed of spaces and tabs (only those two characters, here and in every trim below),
then:
- empty: ignored;
- first character `;`: comment, ignored (`#` is not a comment);
- first character `[`: a section header. The name is the text after `[` up to the first `]`, taken as is
  (no trimming inside the brackets); text after `]` is ignored. `[]` and a `[` with no `]` define no
  section (but still end the previous section for lookups, see below);
- anything else: an entry. The key is the text before the first `=`, right-trimmed; the value is the text
  after the first `=`, trimmed on both sides (so values may contain `=`; quotes are not interpreted). A
  line with no `=` has an empty key.

Case: section names and keys are compared case-insensitively. Lower-casing maps ASCII A-Z; a string
holding any byte above 0x7E is lower-cased as a whole through UTF-16 (wide lower-case of each unit).

Duplicates:
- Section: the index keeps the first header with a given (lower-cased) name. Lookups read only the
  block after that first header; entries under a later duplicate header are invisible to ReadEntry,
  EnumEntries and WriteEntry.
- Key: ReadEntry and WriteEntry use the first matching line of the section; EnumEntries reports all.

A section's block ends at the next line starting with `[` (even an invalid header) or end of file.
Entries before the first header belong to no section and are never read.

### 3.4 Open, close and the section index

- Open(forWrite) (slot 19, 0x0091C5DB): if the file is already open with the same access (read = 1,
  write = read|write = 3), true at once. Otherwise it opens with access read (or read+write), creation
  open-existing (or open-always for write), share read (or exclusive for write). Note cRZFile::Open does
  nothing when the file is already open (§2.3), so a file opened for reading stays read-only if Open(true)
  is called without closing first; WriteEntry closes first itself (§3.6). On failure, if the file exists
  it retries: sleep 250 ms (250,000 microseconds) and retry, giving up once a retry
  starts 3 or more whole seconds (C `time`) after the first attempt; a missing file fails at once. On
  success the +0x38 flag := forWrite.
- LoadSectionNames(forWrite) (slot 17, 0x0091DB73): clears the section index; if the file is not open,
  gives it the INI path and calls Open(forWrite) (false on failure). Then reads every line from the start,
  recording for each valid header the offset of the header line's first byte. Sets the loaded flag,
  returns true. The file stays open.
- Every read operation (ReadEntry, EnumEntries, EnumSections, SectionExists) loads the index first if it
  is not loaded (read mode) and closes the file again at the end when leave-open is false.
- Close (slot 5): closes the file if open, clears the section index and the loaded flag (the
  display-name map is kept).
- SetLeaveFileOpenBetweenOperations(flag) (slot 16): stores the flag; switching it off closes the INI.
  The default is on (leave open), so an INI object keeps its file handle (shared read) between calls
  until Close, SetPath, SetFile or destruction.
- Edits made to the file by others after the index is loaded are not seen until Close.

### 3.5 Reads and types

- ReadEntry(section, key, out) (0x0091CBB3): finds the section in the index; seeks to its header,
  reads the header line, then scans the block (§3.3). On the first entry whose lower-cased key equals the
  lower-cased requested key: out := value, true. Missing section, header offset at or beyond the file
  length, or key not found: false, out unchanged. Defaults are the caller's: nothing is written on a miss.
- ReadNumber (0x0091C8D2): ReadEntry, then C `sscanf("%ld")`: leading whitespace allowed, optional sign,
  decimal only, stops at the first non-digit (`"12abc"` reads 12, `"0x10"` reads 0); true only if a
  number was parsed. The result is a signed 32-bit value; oversized decimal values wrap modulo
  2^32 and still succeed. The exact compatibility rule and failure cases are in §3.5.1.
- Binary entries are hexadecimal text, two characters per byte, written in upper case. Reading accepts
  `0-9`, `A-F`, `a-f`; any other character (spaces included) counts as 0 and makes the conversion fail.
  - ReadBinaryEntry(section, key, buffer, size) (0x0091C84F): converts min(length of value, 2 x size)
    characters; returns the number of bytes produced (characters / 2), or 0 when the entry is missing or
    the conversion failed (the buffer may already be partly written). An odd value length fails.
  - ReadBinaryEntry(section, key, &ptr) (0x0091C7B5): allocates length/2 bytes, converts, returns
    length/2 and sets ptr; on conversion failure frees, sets ptr := null, returns 0. Missing entry:
    returns 0, ptr unchanged. The caller frees ptr with the framework allocator.
- SectionExists (0x0091C95A): true if the lower-cased name is in the index.

#### 3.5.1 Decimal overflow (RZ6) *[compat]*

For an ASCII decimal value returned by ReadEntry, interpret the accepted signed decimal
prefix as a mathematical integer N. Let u be its residue in [0, 2^32). ReadNumber stores
u when u < 2^31, otherwise u - 2^32. This applies beyond both signed and unsigned
32-bit limits and beyond 64-bit limits: no saturation, range error or overflow-based
parse failure is introduced. A minus sign negates the magnitude modulo 2^32.

The original executable's embedded scanner, rather than a replacement host C library,
defines this behavior. ReadNumber calls it with the literal `%ld` at 0x00ACF148
(call 0x0091C904 to 0x009EF536) and reports true exactly for one assignment
(0x0091C90C–0x0091C90F). The decimal arithmetic has 32-bit width without a range
rejection (0x009F8518–0x009F8523, 0x009F8565–0x009F856F); sign handling and the
long-value assignment preserve that width (0x009F85C7–0x009F85D0,
0x009F8626–0x009F862F).

At least one accepted digit makes the conversion succeed, even when its residue is
zero. Leading zeroes do not select octal; a non-digit ends the decimal prefix, so
trailing letters or an exponent-looking suffix are not evaluated. With no digits,
the conversion fails and does not assign the numeric output. A missing ReadEntry
also returns false without assigning it. Empty input, whitespace alone, a sign alone,
`abc`, `+abc` and `--1` are examples of failed conversions; this does not invent a
new overflow failure or a default value.

| Value text | ReadNumber result | Numeric output |
|---|---|---:|
| `2147483647` | true | 2147483647 |
| `2147483648` | true | -2147483648 |
| `4294967295` | true | -1 |
| `4294967296` | true | 0 |
| `4294967297` | true | 1 |
| `-2147483649` | true | 2147483647 |
| `-4294967297` | true | -1 |
| `18446744073709551616` | true | 0 |
| `18446744073709551617` | true | 1 |
| `+4294967297junk` or `4294967297e3` | true | 1 |
| `012` | true | 12 |
| `0x10` | true | 0 |
| Empty text, `+`, `abc` | false | unchanged |

Static analysis and 32 isolated original-scanner executions, including 1,024-digit
inputs, support this contract. The [evidence and reproduction notes](evidence/rz6-ini-number-overflow.md)
separate scanner emulation from the wrapper's verified assignment-count check.
No running-game or engine conformance result is claimed, and no additional locale
or non-ASCII digit rules are established here.

### 3.6 Writeback (WriteEntry 0x0091D168)

Lines written are `key = value` followed by CR LF (`"%s = %s%s"`), headers `[Section]` CR LF, using the
section and key text exactly as the caller passed them. WriteNumber writes `"%ld"` of the value; 
WriteBinaryEntry writes the upper-case hex string; both go through WriteEntry. Empty section or empty key:
false, nothing written. Values are written raw (no escaping; a value with CR or LF breaks the file).

1. If the INI path is not empty and no file exists there: drop the index, close the file, open it for
   writing (creating it), write the header and the entry, close unless leave-open; true. The index is
   not rebuilt here (it is reloaded on the next operation).
2. Otherwise make sure the index is loaded (opening for writing if needed: false when that fails), and
   that the file is open for writing (a read-mode file is closed and reopened for writing).
3. Section not in the index: go to end of file. If the file is empty or its last byte is not CR or LF,
   write two CR LF; else one CR LF (one blank line separates the new section). Then the header and the
   entry; the new header offset is added to the index. True.
4. Section found but its header offset is at or beyond the file length: false.
5. Section found: read the header line, then scan the block line by line, remembering the start offset
   of each line:
   - first entry whose key matches (case-insensitive): the rest of the file after that line is read
     into memory, the new `key = value` line is written where the old line started, the rest is written
     back after it, and the file is truncated at the new end. The old line's key spelling, spacing and
     the rest of that line are replaced. The copied tail starts at the next non-blank line (the line
     reader had consumed the old line's whole newline run), so blank lines directly after the replaced
     line are dropped; everything from the next non-blank line on is kept byte for byte. Index flag
     cleared (offsets moved).
   - next section header reached first: the new line is inserted at the first byte of that header line,
     i.e. after any blank lines and comments that precede the header, and the header follows it directly.
     Index flag cleared.
   - end of file reached: the line is appended at end of file. No newline is inserted first, so when the
     file's last line has no trailing newline the new text is joined onto that line.
   True in all three cases, even if reading the tail failed (then nothing is written).
6. Closes the file at the end unless leave-open is set.

### 3.7 Enumeration

- EnumEntries(section, callback, context) (0x0091D91C): false if the index cannot be loaded or the section
  is missing or past end of file. Otherwise collects, in file order, every entry of the block whose key
  (right-trimmed, original case) and value (trimmed) are both non-empty, including duplicate keys, then
  calls callback(key, value, context) for each. True.
- EnumSections(callback, context) (0x0091CFA5): for every section in the index in ascending byte order
  of the lower-cased names, calls callback(displayName, emptyString, context), where displayName is the
  spelling of the first header with that name (falling back to the lower-cased name). True; false if the
  index cannot be loaded.

## 4. Worked examples

File `a.ini` (CR LF line ends):

```
; top comment
[Video]
Width = 1024
height=768

[video]
Width = 640
[Audio]
Volume = 80 ; loud
```

- ReadEntry("VIDEO", "width") -> `1024` (first `[Video]` block; the `[video]` duplicate is never read).
- ReadEntry("Audio", "volume") -> `80 ; loud` (no inline comments); ReadNumber -> 80, true.
- EnumSections -> ("Audio", ""), ("Video", "") in that order (lower-cased names `audio` < `video`).
- WriteEntry("Video", "HEIGHT", "800"): the line `height=768` becomes `HEIGHT = 800`; the blank line
  after it is dropped, so `[video]` now follows directly; the rest of the file is unchanged.
- WriteEntry("Video", "Depth", "32") on the original file: no `depth` key before the next header, so
  `Depth = 32` CR LF is inserted after the blank line, directly before `[video]`.
- WriteEntry("Net", "Port", "80") when the file ends with `Volume = 80 ; loud` and no newline: appends CR LF
  CR LF `[Net]` CR LF `Port = 80` CR LF.
- WriteBinaryEntry("B", "k", {0x0A, 0xFF}, 2) writes `k = 0AFF`; ReadBinaryEntry reads it back as 2 bytes;
  a value `0aff` also reads back; `0A F` fails.

## 5. Open questions

- (Resolved this round) The sleep unit is microseconds: SleepCurrentThread (0x009205A0) divides by 1000
  before the OS sleep, so the retry interval is 250 ms.
- Q2: What reads the cRZFile error field +0x30 (no vtable getter); search callers that read +0x30 of a
  `cRZFile`.
- Q3: §3.6 step 5 blank-line handling is derived from the line reader's rule (the position after a
  line is past its whole newline run, 0x0091CA15); confirm with a runtime round trip.
- Q4: the cRZIniFile constructors from a path or a file (0x0091CEE3, 0x0091CF46) do not write +0x38
  (checked in the instructions; only the default constructor clears it). WriteEntry reads it (§3.6 step 2) to
  decide whether an open file must be reopened for writing; a reimplementation should start it at 0. Does the allocator zero memory?
- Q5 (round 3): the C runtime the exe links decides whether the text-mode `%lld` / `%llu` formats of
  cRZOFStream (§7.3) print 64-bit values; confirm with a runtime write of 2^40.
- Q6 (round 3): §8.4 slot 10 (letter switch value) is read from the instruction skeleton only; confirm
  the value offsets with a pseudocode pass when REA is available.

## 6. cRZDirectory (round 3)

A non-COM helper (no vtable, no reference count; callers keep it on the stack or embed it; 28 call
sites of the constructor 0x0091BD31: plug-in, region, music and Lua-resource scans). Evidence:
instructions of every member 0x0091AD3E..0x0091BF90 and the find helpers RZFileFindFirst 0x0091B0A1,
find-next 0x0091B21D, RZFileFindFinish 0x0091AE66.

### 6.1 Layout

| Offset | Field |
|---|---|
| +0x00 | entry list (std::list sentinel pointer); node = next, prev, then the entry at node+8 |
| +0x04 | entry count (maintained by ReadDirectoryEntries only) |
| +0x08 | directory path, cRZString (0x14 bytes), ends in a separator after every setter |
| +0x1C | entry filter (wildcard pattern), cRZString, default `*` |
| +0x30 | bool: synthesise / keep the `..` entry |
| +0x31 | bool: synthesise / keep the `.` entry |

Entry (`cRZDirectoryEntry`, 0x18 bytes): name cRZString at +0, type dword at +0x14: 4 = directory,
8 = file. Names are the bare names the OS returns (no path), converted to the framework encoding.

### 6.2 Separators and path normalisation

A separator is the byte 0x5C (backslash) or 0x2F (slash) (test 0x005F8FE0). "Ensure trailing
separator" (MakeSurePathEndsWithSeparator 0x0091AE31) appends a backslash when the path is empty or
its last byte is not a separator, so an empty path becomes a lone backslash (the root of the current
drive). Mixed separators are kept as given; nothing else is normalised (no case folding, no `.`/`..`
collapsing, no trimming).

### 6.3 Construction, setters, destruction

- Constructor (path, readNow, keepParent, keepSelf): count 0, path copied then given a trailing
  separator, filter `*`, +0x30 = keepParent, +0x31 = keepSelf; when readNow, ReadDirectoryEntries.
- ChangeDirectory(path, readNow) 0x0091BCAD: replaces the path, ensures the separator, reads when
  readNow; always returns true (the read result is discarded, so a missing directory still "succeeds"
  with an empty list).
- SetNewDirectoryEntryFilter(filter, readNow) 0x0091BCD7: an empty filter becomes `*`; reads when
  readNow. The filter is matched by the OS (Win32 FindFirstFile wildcard rules), never by the framework.
- Setters without readNow leave the old entries and count in place.
- Destructor: destroys filter, path, then the entry list.

### 6.4 Enumeration (ReadDirectoryEntries 0x0091BA22)

1. Pattern = path + filter (plain concatenation; the path already ends in a separator).
2. The list is emptied and the count set to 0 before the search. If the first find fails (missing
   directory, nothing matches) the call returns false with an empty list.
3. Order is the OS enumeration order (no sort). Each match is appended (push_back) with type 8 when
   the OS attribute bit 0x10 (directory) is clear, type 4 when it is set. No other attribute is
   consulted: hidden, system and read-only entries are listed like any other.
4. Directories named exactly `.` / `..` (byte-equal, 0x00418A40) are special: the first `.` is kept
   only when +0x31 is set, the first `..` only when +0x30 is set; each is remembered as seen either
   way. A second entry of the same name would be listed as a plain directory.
5. After the search (handle closed): if +0x30 is set, `..` was not seen and the filter is exactly `*`
   (byte compare), a `..` directory entry is pushed to the FRONT unless an entry of that name is
   already listed (byte-equal, 0x0091AD3E); then the same for `.` with +0x31. When both are
   synthesised the front order is `.`, `..`, then the OS entries (the drive-root case, where the OS
   reports neither). A filter other than `*` never synthesises them.
6. Each kept or synthesised entry increments the count; returns true.

GetNthEntry(n) 0x0091AD83 walks from the front and returns the n-th entry (0-based), or null when
n >= count.

### 6.5 Reading into a caller's string list

ReadDirectorySpecificEntriesIntoStringList(list, flags) 0x0091B36E uses this object's path and
filter but never touches its entry list or count. It APPENDS names to the caller's list (no clear).
Flags: 8 = files, 4 = directories, 1 = `.`, 2 = `..`.

- File (attribute 0x10 clear): appended when flag 8.
- Directory: skipped without flag 4; with flag 4 the first `.` is appended only with flag 1, the first
  `..` only with flag 2, any other directory is appended.
- After the search, when the filter is exactly `*`: flag 2 and no `..` seen: push_front `..`; then
  flag 1 and no `.` seen: push_front `.`. No duplicate check here (unlike §6.4). Front order `.`, `..`.
- Returns false (list unchanged) when the first find fails, else true.

Wrappers: ReadDirectoryFileEntriesIntoStringList(list) 0x0091B9C7 = flags 8 (files only).
ReadDirectoryDirectoryEntriesIntoStringList(list, dotFlags) 0x0091B9D5 = (dotFlags & 3) | 4.

AlphabetizeEntryList(list) 0x0091AE1E sorts a caller's string list (not the entry list) in place with
a stable merge sort (std::list::sort 0x007B75F0), ascending by unsigned byte comparison of the common
prefix, the shorter string first when one is a prefix of the other. Case-sensitive: `Zoo` sorts before
`apple`. Only caller: cGZWinFileBrowser::UpdateFileList.

### 6.6 Recursive file listing (0x0091BD88)

ReadDirectoryFileEntriesIntoStringListRecursive(path, outList, includeTopLevelFiles):

1. A process-wide depth counter (0x00B62914, not thread-safe) is incremented, then
   ChangeDirectory(path, readNow = true) on this object, so its path and entries are overwritten at
   every level.
2. Files: when includeTopLevelFiles is true or the depth is above 1, this directory's files (§6.5 flags
   8, current filter) are appended to outList as full paths: the path argument as given, plus a
   backslash if it does not end in a separator, plus the name.
3. Subdirectories: the filter is saved, set to `*`, the directory names (no dot names) are read into a
   fresh list, and the filter is restored. For each, in OS order, recurse with path = the path
   argument AS GIVEN + name + backslash. No separator is inserted between the argument and the name:
   called with `C:\Plugins` (no trailing separator) the first-level children become `C:\PluginsSub\`.
   Deeper levels are well formed because their paths end in a backslash.
4. Depth decremented; always returns true. Output order is pre-order: this directory's files, then
   each subdirectory's output depth-first, all in OS order. Afterwards this object points at the last
   directory visited, with the original filter.

### 6.7 Path helpers

| Function | Behaviour |
|---|---|
| DoesAnyEntryExistThatMatchesPattern(pattern) 0x0091B312 (static) | true when the OS find on the pattern returns anything; the find is closed at once |
| DoesDirectoryExist(path) 0x0091B6CA (static) | path + backslash (when empty or not ending in a separator) + `*`, then the pattern test. True for an existing directory (its `.` entry matches); false for a file path or a missing path |
| CreateDirectory(path) 0x0091B738 (static) | creates every missing level, below |
| SplitDirectoryPath(last, parent) 0x0091B017 (member, reads +0x08) | splits at the last separator, below |

CreateDirectory(path):

1. Empty path: false. A backslash is appended when the last byte is not a separator.
2. UNC prefix: when the path is longer than 2 bytes, starts with two backslashes and the third byte is
   not a backslash, everything up to and including the third backslash (the `\\server\` part) is
   taken as existing and never tested.
3. The remaining bytes are scanned left to right; at every separator the prefix so far (including the
   separator) is tested with DoesDirectoryExist and, when missing, created (CreateDirectoryW with no
   security attributes when the wide API was found at start-up, else `_mkdir` on the OS-encoded path).
   Each created prefix is remembered in creation order.
4. On a create failure every remembered directory is removed (RemoveDirectoryW or `_rmdir`) in creation
   order, outermost first; a parent that still holds a created child cannot be removed and stays. The
   call returns false. Otherwise it returns true (also when nothing had to be created).

SplitDirectoryPath(last, parent) splits this object's path with the C helper SplitDirectory
0x00917D1A asked for 2 parts, scanning back from the end (a trailing separator is skipped):

| Path (+0x08) | last | parent | Result |
|---|---|---|---|
| `C:\A\B\` | `B\` | `C:\A\` | true |
| `C:\A\B` | `B\` | `C:\A\` | true |
| `C:\` | `C:\` | not produced (the original then copies an unwritten 260-byte stack buffer; return empty) | true |
| empty | unchanged | unchanged | false |

Each part gets a backslash appended unless it already ends in a separator; a part longer than 254
bytes fails the whole call (false, outputs unchanged).

## 7. cRZOFStream (round 3)

A `cIGZOStream` (iid 0xC51B11CB; QueryInterface 0x0092B2A3 also accepts iid 1 and AddRefs on
success) that writes to a C++ file stream, either as raw bytes ("binary") or as printf text ("text").
vtable 0x00AD19F8, 24 slots. Two call sites: SC4CreateGZLog (0x005C6EFF, path from a cIGZString) and
cSC4ViewInputControlQuery::WriteDetailedDebugInfo (0x004D12BD, `Query.txt`, binary, big-endian, no
auto-flush, truncate).

### 7.1 Layout and lifetime

| Offset | Field |
|---|---|
| +0x00 | vtable |
| +0x04 | reference count (cRZRefCount), starts 0 |
| +0x08 | bool littleEndian (cRZOStream byte order), default 1, then set from the constructor |
| +0x0C | user-data pointer (slot 19 SetUserData assigns it as a ref-counted pointer, slot 20 returns it); starts null |
| +0x10 | owned std::ofstream (0x118 bytes, heap) |
| +0x14 | bool binary |
| +0x15 | bool autoFlush |

Constructors (path as `const char*` 0x0092CEF6, or as `cIGZString` 0x0092CE7B, which takes its char
pointer from string slot 5): (path, binary, littleEndian, autoFlush, append). The file stream is
opened at once with mode out | (append ? app : trunc) | (binary ? binary : 0) (mode word bits app 1,
binary 4, out 0x10, trunc 0x20). An open failure is not reported by the constructor; it shows as the
stream's error state.

Release 0x0092B2C8: a count above 1 is decremented and returned; at 1 (or 0) the object is destroyed
(deleting destructor, slot 21) and 0 returned. The destructor destroys the owned file stream, which
flushes pending output and closes the file. That is the only close: there is no explicit Close.

### 7.2 vtable 0x00AD19F8

| Slot | Function | Behaviour |
|---|---|---|
| 0-2 | QueryInterface, AddRef, Release | above |
| 3 | Flush 0x0092BCF7 | std::ostream::flush on the file stream |
| 4-9 | SetSint8 .. SetUint32 | §7.3 |
| 10-11 | SetSint64, SetUint64 | §7.3 |
| 12-13 | SetFloat32, SetFloat64 | §7.3 |
| 14 | SetRZCharStr(const char*) 0x0092C757 | §7.4 |
| 15 | SetGZStr(cIGZString) 0x0092B355 | the string's char pointer (its slot 5) passed to slot 14, both modes |
| 16 | SetGZSerializable(obj) 0x0092B294 | calls obj slot 3 (Write) with this stream; returns its result |
| 17 | SetVoid(data, size) 0x0092C1EE | std::ostream::write of size raw bytes; returns true when the stream state is clean |
| 18 | GetError 0x0092B9EE | true when any stream state bit (bad, fail, eof) is set |
| 19 / 20 | SetUserData / GetUserData | +0x0C |
| 21 | deleting destructor 0x0092BCDB | |
| 22 | Init(littleEndian) 0x0092B25B | sets +0x08, returns true |
| 23 | Shutdown 0x00949ADE | no-op, returns true (the file stays open) |

### 7.3 Number writers

Binary mode (+0x14 set): the value is written with SetVoid in its natural size.

| Writer | Bytes | Byte order |
|---|---|---|
| Sint8 / Uint8 | 1 | n/a |
| Sint16 / Uint16 | 2 | little-endian when +0x08 is set, else big-endian (byte swap 0x0092B640) |
| Sint32 / Uint32 | 4 | same rule |
| Sint64 / Uint64 | 8 | same rule, full 8-byte reversal when big-endian (0x0092B55B) |
| Float32 / Float64 | 4 / 8 | ALWAYS the native little-endian IEEE bytes; +0x08 is ignored (0x0092B276, 0x0092B285) |

Text mode (+0x14 clear): the value is printed into a temporary string with the framework sprintf and
the text is passed to slot 14 (so auto-flush applies). Formats (pointers 0x00B169B0..0x00B169C0):

| Writer | Format | Notes |
|---|---|---|
| Sint8, Sint16, Sint32 | `%d` | sign-extended |
| Uint8, Uint16, Uint32 | `%u` | zero-extended |
| Sint64 / Uint64 | `%lld` / `%llu` | |
| Float32, Float64 | `%g` | Float32 widened to double first |

No separator, space or newline is ever added: writing Sint32 12 then Sint32 34 produces `1234`.
Callers add their own separators through slot 14. Text mode opens the file without the binary flag,
so the C runtime translates each `\n` written into CR LF on Windows.

### 7.4 Strings and flushing

SetRZCharStr(s): text mode writes the C string with the stream's `<<` (bytes up to the NUL) and
returns true when the stream state is clean; binary mode writes strlen(s) raw bytes through slot 17
(no terminator, no length prefix) and returns slot 17's result. Then, in both modes, when autoFlush
(+0x15) is set, slot 3 (Flush) runs. Auto-flush happens ONLY here: binary number writers and direct
SetVoid calls never auto-flush (text number writers do, because they go through slot 14).

Example: binary, big-endian (+0x08 = 0): SetUint16(0x1234), SetFloat32(1.0), SetRZCharStr("ab") writes
`12 34 00 00 80 3F 61 62`. Text: SetSint16(-5), SetRZCharStr(","), SetFloat64(0.1) writes `-5,0.1`.

## 8. cRZCmdLine (round 3)

The `cIGZCmdLine` implementation (iid 0xA31E539C; QueryInterface 0x00913E16 also accepts iid 1).
vtable 0x00A82A20, 17 slots. Used for the program command line and, through the parsing constructor
0x00914E14 (22 call sites), for cheat and message argument strings (advisors, tutorial, clock).

### 8.1 Layout

| Offset | Field |
|---|---|
| +0x00 | vtable |
| +0x04 | reference count |
| +0x08..+0x10 | std::vector of cRZString arguments (begin, end, capacity; element 0x14 bytes); argc = size |
| +0x14 | the whole command line, cRZString |
| +0x28 | an always-empty cRZString (returned for out-of-range argv) |

Constructors: from a string (0x00914E14) keeps the text at +0x14 and parses it (§8.2); from another
cIGZCmdLine (0x00914A30) copies its command line text (source slot 5) and its arguments one by one
(source slots 3 and 4) without re-parsing.

### 8.2 Tokenising (0x00914C70)

Whitespace is exactly space, tab and LF (CR is an ordinary character).

1. The argument list is cleared; trailing, then leading whitespace is trimmed; an empty result gives
   argc 0.
2. Scanning left to right, a double quote toggles "inside quotes" (a running count, odd = inside).
   Whitespace outside quotes ends the current token; the whole following whitespace run is skipped.
   Whitespace inside quotes is part of the token. Single quotes are ordinary characters.
3. Each token (including the last, which ends at the end of the text) is post-processed: when its
   first AND last bytes are `"`, the last byte is removed and then, if anything is left, the first.
   Inner quotes stay: `a"b c"d` is one argument `a"b c"d`; `"x y"` is `x y`; `""` is an empty argument;
   a lone unbalanced quote swallows the rest of the line into one token.
4. No escape characters, no empty tokens between separators (runs collapse), no limit on count.

Example: `  -w   /CustomResolution:enabled "C:\My Plugins\x" ""  ` gives 4 arguments: `-w`,
`/CustomResolution:enabled`, `C:\My Plugins\x`, `` (empty).

Rebuilding the text (0x00914174, after Insert/Erase): +0x14 is cleared and the arguments are joined
with single spaces; an argument containing a space, tab or LF is wrapped in double quotes; nothing is
escaped (an argument holding a quote does not round-trip), and an empty argument contributes nothing
(it disappears on re-parse).

### 8.3 vtable 0x00A82A20

Switch prefix characters are `/`, `-` and backslash (string 0x00ACECC8). Name comparisons lower-case
both sides with cRZString::MakeLower; character comparisons fold ASCII A-Z/a-z only.

| Slot | Function | Behaviour |
|---|---|---|
| 0-2 | QueryInterface, AddRef, Release | standard ref counting |
| 3 | argc 0x00913E8A | number of arguments |
| 4 | argv(i) 0x00913E62 | char pointer of argument i; for i >= argc the empty string at +0x28 (negative i is not checked: do not pass it) |
| 5 | GetCommandLine(out) 0x0093E536 | copies +0x14 into the cIGZString, returns true |
| 6 | Find(text, caseSensitive) 0x009145A5 | index of the first argument equal to text (both lower-cased when not caseSensitive), else -1 |
| 7 | FindSubString(text, caseSensitive) 0x00914691 | index of the first argument containing text, else -1; an empty text matches the first non-empty argument |
| 8 | GetSwitchValue(name, out, stripQuotes) 0x009143CB | §8.4 |
| 9 | IsSwitchPresent(name) 0x00913EC2 | true when slot 11 finds `/name` from index 0, or slot 8 (stripQuotes 1) finds `/name:value` |
| 10 | GetSwitchValue(letter, out, stripQuotes) 0x00914240 | §8.4 (partial) |
| 11 | GetIndexOfSwitch(name, start) 0x00914001 | first index >= start (negative start = 0) of an argument whose first byte is a switch character and whose remaining bytes equal name case-insensitively (exact length: `/name:x` does not match), else -1 |
| 12 | GetIndexOfSwitch(letter, start) 0x00913F1A | first index >= start of a 2-byte argument: switch character + letter (ASCII case-insensitive), else -1 |
| 13 | InsertArgument(text, index) 0x00914BAE | index clamped to 0..argc (0 when empty or negative); inserts, rebuilds +0x14, returns true |
| 14 | EraseArgument(index) 0x00914C25 | an index >= argc erases the LAST argument; erases, rebuilds +0x14, returns true. Not checked: negative index, or argc 0 (erases at -1); a reimplementation should return false there |
| 15 | destructor 0x00419500 | |
| 16 | GetArgument(i, out) 0x00913E3B | for 0 <= i < argc (signed compare) copies argument i into the cIGZString |

Duplicates: every lookup returns the FIRST match in argument order; later repeats are ignored.

### 8.4 Switch values

GetSwitchValue(name, out, stripQuotes) (slot 8): an empty name returns false. The first argument
that (a) starts with a switch character, (b) is at least len(name) + 3 bytes long, (c) has `:` at
offset len(name) + 1 and (d) has bytes 1..len(name) equal to name after lower-casing, wins. The value is
everything after the colon (at least one byte, so `/name:` alone never matches). With stripQuotes, a
value whose first byte is `"` or `'` AND whose last byte is `"` or `'` (not necessarily the same) loses
its first and last byte. The value is copied into out; returns true. No match: false, out unchanged.

| Arguments | Call | Result |
|---|---|---|
| `-Lang:'de'` | slot 8 (`lang`, strip) | true, `de` |
| `/x:"a'` | slot 8 (`x`, strip) | true, `a` |
| `/x:` | slot 8 (`x`) | false; slot 9 (`x`) also false |
| `/W`, `-w:1` | slot 9 (`w`) | true (index 0 via slot 11) |

GetSwitchValue(letter, out, stripQuotes) (slot 10, from the instruction skeleton only: partial): finds
the first argument starting with a switch character followed by letter (ASCII case-insensitive, same
test as slot 12 but without the 2-byte length limit); the value is the bytes after those two (empty
when the argument is exactly 2 bytes); with stripQuotes and an argument of at least 4 bytes whose byte 2
and last byte are both `"`, the value is bytes 3..len-2. Returns true when found.
