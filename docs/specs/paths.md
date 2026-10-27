# Spec: paths and directories (`cRZPath`, `cRZDirectory`)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`). Implementers must work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled
or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[compat-limit]* depends on something FreeCity4 cannot reproduce (here the host's file system and
current directories), so only the rule is matched.

Related specs: [lua-host.md](lua-host.md) (§7.1, the script resource key's `basename`, built on §1 and §2), [output-streams.md](output-streams.md)
(the GZ log file the game creates through a path), [command-line.md](command-line.md) (the other text splitter).

## 0. Summary

The framework's path code is a set of free functions over C strings (§1) plus two classes: `cRZPath` (§2) splits a
resolved path into drive, directory, file name and extension and joins them back, and `cRZDirectory` (§3) lists,
tests, creates and removes directories through the Win32 find functions (§4). Separators are `\` and `/` alike;
paths are never case-folded; nothing here is saved.

## 1. Path utilities *[compat]*

A **separator** is `\` or `/` (`IsPathSeparator`, `0x005F8FE0`). The explicit `IsASCIILetter` helper
(`0x00917AE0`) recognizes ASCII A–Z and a–z; Fullpath's drive branch uses it (`0x00918624–0x0091862D`).
IsFullPath and FixDriveString instead use CRT locale classification (§2.3).

### 1.1 `IsFullPath(p)` (`0x00918080`; on a `cRZString`, `0x009186E7`)

True when `p` starts with a letter (C `isalpha`), `:` and `\`, or with `\\`, **and** contains neither `\.` nor `/.`
anywhere. So `C:/x`, `\x` and `C:x` are not full, and neither is `C:\a\.hidden\f` or `C:\a\..\b` *[quirk]*: they go
through the normaliser (§1.2), which keeps `.hidden` but resolves `..`.

### 1.2 Normalise against a base (`Fullpath(out, rel, max, base)`, `0x00917E7C`)

`max` counts the terminating NUL; every caller passes 520 (`0x208`). The result is built in a 521-byte work buffer.

1. `rel` null: fail.
2. If `base` is not empty: copy it (failing when it reaches 521 characters) and, if it does not end with a separator,
   append `\`.
3. Append `rel`; if the total is 521 characters or more, fail. Remember whether the joined text ends with a
   separator (`trailing`).
4. Walk the joined text component by component, copying characters. At each separator or at the end:
   - if the text copied so far ends with *separator* `.`, those two characters are removed (a `.` component);
   - else if it ends with *separator* `..`: if the character before that separator is `:`, **fail** (`C:\..`);
     otherwise remove back to the previous separator (a `..` pops one component). When there is no previous
     separator the code tests the byte **before** its work buffer, which is stack contents: it normally fails, and
     otherwise corrupts that byte *[quirk]*. *[choice]* FreeCity4 fails.
   - then the separator itself is copied **as typed** (`/` stays `/`).
5. At the end: a final separator is dropped; if the text then ends with `:`, `\` is appended (`C:` → `C:\`); if
   `trailing`, `\` is appended. Both can apply, so `C:/` becomes `C:\\` *[quirk]*.
6. If the result plus its NUL exceeds `max`, fail. Otherwise copy it out (or, with a null `out`, return a `malloc`ed
   copy).

Empty components are kept (`C:\a\\b` stays `C:\a\\b`); a component of three or more dots is ordinary.

### 1.3 Normalise against the current directory (`Fullpath(out, rel, max)`, `0x009185C4`)

1. `rel` starts with two separators: base `\\`, and `rel` loses those two characters.
2. Else if `rel` is a letter followed by `:`: the drive is that letter upper-cased, and `rel` loses the two.
3. Else the drive is the current one (C runtime `_getdrive`; `C` if it reports none).
4. If (the rest of) `rel` starts with a separator: base `X:\` for the drive, and `rel` loses the separator.
   Otherwise the base is that drive's current directory (`_getdcwd`, at most 521 bytes, a `\` appended when
   missing); no current directory: **fail** *[compat-limit]*.
5. §1.2 with that base.

### 1.4 `ResolvePath(out, in, base)` (`0x00919738`)

If `IsFullPath(in)`: `out := in` unchanged, true. Otherwise §1.2 against `base` when `base` is given
(`0x009196CC`; a null base text counts as empty), else §1.3 (`0x00919672`); on success `out` gets the result and
the call returns true; on failure `out` is unchanged and it returns false.

### 1.5 `Splitpath(p, drive, dir, name, ext)` (`0x00917BCB`; on `cRZString`s, `0x009194A0`)

1. If `p` has at least one character and its second character is `:`, `drive :=` its first two characters (any
   first character, not only a letter) and the rest is split; else `drive := ""`.
2. `dir :=` the rest up to and including its **last** separator (empty when there is none).
3. Of what follows, the **last** `.` starts `ext` (the dot included); `name` is the text before it. With no dot after
   the last separator, `name` is all of it and `ext := ""`. A dot inside `dir` never starts an extension.
4. Each part is cut at 255 characters. Any output may be null (not written).

So `.lua` gives name `""` and extension `.lua`; `a.b.c` gives `a.b` and `.c`; `C:foo.txt` gives `C:`, `""`, `foo`,
`.txt`. The `cRZString` form uses its own 4-byte argument slot as the drive buffer, which holds the 2 characters and
the NUL.

### 1.6 `SplitDirectory(path, parts, count&)` (`0x00917D1A`)

Splits from the end into at most `count` 260-byte parts: part 0 is the last component, part 1 the next, and so on;
the last part filled holds everything before. Each part gets a trailing `\` when it does not end with a separator.
An empty `path`, or a component of 255 characters or more, fails with `count := 0`. Otherwise `count :=` the number
of parts written and it returns true. With `count = 2`: `C:\a\b\` and `C:\a\b` give `b\` and `C:\a\`.

A path with no separator except possibly a trailing one is a single part, and without the trailing separator its
**last character is lost**: `abc` gives `ab\` (`abc\` gives `abc\`) *[quirk]*.

## 2. `cRZPath` *[compat]*

### 2.1 State (0x64 bytes; five `cRZString`s)

| Offset | Field |
|---|---|
| `+0x00` | `drive` |
| `+0x14` | `dir` |
| `+0x28` | `name` |
| `+0x3C` | `ext` |
| `+0x50` | `buffer` (the resolved path, then the last joined result) |

### 2.2 Methods (all non-virtual)

| Method | Address | Behaviour |
|---|---|---|
| `cRZPath(cIGZString const& p)` | `0x0091C2C5` | empty strings, then `SetPath(p)` |
| `SetPath(p)` | `0x0091C268` | `ResolvePath(buffer, p, no base)` (§1.4; on failure `buffer` keeps its old text), then `Splitpath(buffer)` into the four parts; true |
| `~cRZPath()` | `0x0043AF80` | destroys the five strings |
| `GetDrive()` | `0x0091C3CF` | fix `drive` (below), return it |
| `GetDirectory()` | `0x0091C37E` | fix `dir`, return it |
| `GetFileName()` | `0x0091C264` | `name`, as stored |
| `FileName()` | `0x0091C298` | if `name` is empty and `buffer` is not, `name :=` the name part of `Splitpath(buffer)`; return `name` |
| `GetExtension()` | `0x0091C31F` | fix `ext`, return it |
| `SetFileName(s, split)` | `0x0091C3DC` | if `s` differs from `name`: `name := s`, or with `split` true the name part of `Splitpath(s)`; true |
| `SetExtension(s, split)` | `0x0091C41B` | if `s` differs from the fixed `ext`: `ext := s` and fix it, or with `split` true the extension part of `Splitpath(s)`; true |
| `SetBuffer(parts)` | `0x0091C46E` | `buffer := ""`, then append the fixed drive (bit 0), the fixed directory (bit 1), the name (bit 2) and the fixed extension (bit 3) |
| `GetPath(out)` | `0x0091C507` | `SetBuffer(15)`, `out := buffer` |
| `GetDriveAndDirectory(out)` | `0x0091C52A` | `SetBuffer(3)`, `out := buffer` |
| `GetFileNameAndExtension(out)` | `0x0091C54D` | `SetBuffer(12)`, `out := buffer` |

The **fixes** change the stored part in place and only when it is not empty *[compat]*:

- drive (`FixDriveString`, `0x0091C38C`): cut or padded with NULs to 2 characters; a first character that is not a
  letter (`isalpha`) becomes `C`; the second becomes `:`;
- directory (`FixDirectoryString`, `0x0091C32D`): `\` prepended when it does not start with a separator, `\`
  appended when it does not end with one;
- extension (`FixExtensionString`, `0x0091C2FE`): `.` prepended when it does not start with one.

So `GetPath` of `adv_const.lua` resolved against `C:\Games\SC4` is `C:\Games\SC4\adv_const.lua`. A successfully
resolved path already has a letter drive and a directory that starts and ends with a separator, so the fixes change
only what a failed resolution or a `Set…` call left behind.

### 2.3 Byte strings, equality and locale (PA4)

The 255/520/521 limits in §§1–2 count **bytes**, with the separately stated treatment of trailing
NUL. Fullpath copies single bytes, advances pointers by one and adds `strlen(rel)` before the 521
comparison (`0x00917EAE–0x00917EF8`); Splitpath uses strlen, byte scans and copies
(`0x00917BDA–0x00917C23`). These owners do not decode, normalize or transcode bytes, and establish
no particular UTF-8 or other encoding.

SetFileName and SetExtension compare through `0x00454BB0–0x00454C06`: compare stored begin/end byte
lengths, then all bytes case-sensitively. Embedded NUL does **not** end comparison. Unsplit assignment
uses the source's entire begin/end range (`0x00424399–0x004243A5`), preserving embedded NULs. Split
assignment passes the character pointer to the C-string splitter, stopping at the first NUL
(`0x009194C3–0x009194C9`). IsFullPath's string overload also passes only the character pointer
(`0x009186E7–0x009186F4`). ResolvePath can therefore accept a fully qualified C-string prefix, then
copy the complete stored byte range (`0x0091973C–0x00919753`).

IsFullPath and FixDriveString sign-extend the first byte and call `_isalpha`
(`0x00918085–0x00918089`, `0x0091C3B0–0x0091C3B6`). The CRT uses the thread's locale information,
refreshing it when stale; for single-byte locales it tests mask `0x103` in the locale's table, and
for multibyte locales calls `___isctype_mt` (`0x009EF56A–0x009EF5A8`). Thus their native rule is
**CRT locale-dependent**, not unconditional ASCII or Unicode alphabetic membership. These owners
establish neither a fixed runtime locale nor safe behavior for every negative signed byte outside
the CRT classifier's valid domain.

Distinguishing vectors: `a` and `A` differ; stored `a\0x` and `a\0y` differ despite equal C-string
prefixes. An unsplit filename setter retains all three bytes; split mode processes only `a`. A
sequence of 128 two-byte characters has byte length 256 for these bounds. High-bit drive tests
require the native CRT locale/input-domain behavior, not host Unicode classification.

## 3. `cRZDirectory` *[compat]*

### 3.1 State (0x34 bytes)

| Offset | Field |
|---|---|
| `+0x00` / `+0x04` | `entries`: a list of entries (head node, count) |
| `+0x08` | `path` (`cRZString`), always ending with a separator |
| `+0x1C` | `filter` (`cRZString`), `*` by default |
| `+0x30` | `includeParent`: keep `..` in `entries` |
| `+0x31` | `includeSelf`: keep `.` in `entries` |

An **entry** (as `GetNthEntry` returns it) is `{ +0x00 the owning directory, +0x04 name (cRZString), +0x18 kind }`,
kind 4 for a directory and 8 for a file.

### 3.2 Construction and settings

| Method | Address | Behaviour |
|---|---|---|
| `cRZDirectory(path, read, includeParent, includeSelf)` | `0x0091BD31` | as the fields; `filter := "*"`; `MakeSurePathEndsWithSeparator`; `ReadDirectoryEntries` if `read` |
| `~cRZDirectory()` | `0x0091B2F7` | destroys `filter`, `path` and `entries` |
| `MakeSurePathEndsWithSeparator()` | `0x0091AE31` | appends `\` when `path` is empty or does not end with a separator |
| `ChangeDirectory(path, read)` | `0x0091BCAD` | `path :=` it, made to end with a separator; `ReadDirectoryEntries` if `read`; true |
| `SetNewDirectoryEntryFilter(f, read)` | `0x0091BCD7` | `filter := f`, or `*` when `f` is empty; `ReadDirectoryEntries` if `read` |
| `GetNthEntry(n)` | `0x0091AD83` | the `n`-th entry in list order, or null when `n ≥` the count |
| `AlphabetizeEntryList(stringList)` (static) | `0x0091AE1E` | sorts the supplied **string** list by byte order; see §3.10 |

### 3.3 `ReadDirectoryEntries()` (`0x0091BA22`)

1. Clear `entries`. Search `path + filter` (§4). No match at all: return false.
2. For each match in the order the system returns them: kind 4 for a directory, 8 for a file. A directory named
   exactly `.` is kept only with `includeSelf`, one named `..` only with `includeParent`; everything else is kept.
   Kept entries are appended.
3. After the search, **only when `filter` is exactly `*`**: if `includeParent` and no `..` was seen, an entry `..`
   (kind 4) is put at the **front** (unless the list already holds that name); then the same for `.` with
   `includeSelf`, so `.` comes first and `..` second. (A drive root returns neither from the system.)
4. Return true.

### 3.4 Name lists

`ReadDirectorySpecificEntriesIntoStringList(list, flags)` (`0x0091B36E`) appends **names** (not paths) to the
caller's list without clearing it: files when bit 3 (8) is set; directories when bit 2 (4) is set, `.` among them
only with bit 0 and `..` only with bit 1. Then, if the filter is `*`: with bit 1 and no `..` seen, `..` is put at
the front; with bit 0 and no `.` seen, `.` is put at the front. It returns false when the search finds nothing (the
list unchanged), else true. It does not touch `entries`.

- `ReadDirectoryFileEntriesIntoStringList(list)` (`0x0091B9C7`): flags 8.
- `ReadDirectoryDirectoryEntriesIntoStringList(list, dots)` (`0x0091B9D5`): flags `(dots & 3) | 4`.

### 3.5 `ReadDirectoryFileEntriesIntoStringListRecursive(dir, list, includeTop)` (`0x0091BD88`)

A process-wide depth counter (`0x00B62914`) is incremented. `ChangeDirectory(dir, true)`: the object itself moves,
and stays at the last directory visited when the call returns *[quirk]*. If `includeTop`, or the depth is above 1,
each file name matching the current filter is appended to `list` as `dir` + (a `\` when `dir` does not end with a
separator) + name. Then, with the filter set to `*` for the moment, the subdirectories (no `.` or `..`) are listed and
each is visited as `dir + name + "\"`, with the same filter and `includeTop`. The counter is decremented; the result
is always true. The order is: this directory's files, then each subdirectory's results in system order. The global
counter makes concurrent calls on two threads interfere *[inferred]*.

*[compat]* None of the six nonrecursive direct call sites passes `includeTop=false`: both resource-setup
calls (`0x00457A2F`, `0x00457CAE`), region copying (`0x004982C5`), tutorial reset (`0x004996F9`), CD-path
validation (`0x0089132B`) and multi-packed-file opening (`0x0097E9DE`) pass true. The sole remaining direct
call is recursion itself and forwards the incoming flag (`0x0091BF3A–0x0091BF47`). This survey establishes
no built-in top-level user of false. Whether third-party mods depend on traversal order remains PA2.

### 3.6 Existence and search (static)

- `DoesAnyEntryExistThatMatchesPattern(pattern)` (`0x0091B312`): true when the search (§4) finds anything.
- `DoesDirectoryExist(path)` (`0x0091B6CA`): the pattern `path` + `\` (when missing) + `*`. Any directory matches
  (even an empty one, which lists `.`), a file or a missing path does not *[compat-limit]*.

### 3.7 `CreateDirectoryA(path)` (static, `0x0091B738`)

1. Empty `path`: false. Append `\` when it does not end with a separator.
2. A path that starts with exactly two backslashes (`\\server\…`) has its prefix up to and including its third `\`
   (`\\server\`) taken as existing.
3. Walk the rest character by character; at each separator, if the text so far is not an existing directory
   (§3.6), create it (`_mkdir` on the text converted to the system code page, ending with the separator) and remember
   it.
4. A failed creation removes the directories created by this call, **in creation order** (`_rmdir` each), and returns
   false. Because a parent is removed before its child, the removal of every directory but the last created fails
   and those stay behind *[quirk]* (*[choice]*: FreeCity4 removes in reverse order). Otherwise true.

### 3.8 `RemoveDirectoryA(path, recursive)` (static, `0x0091BF93`)

1. `p := path` with a trailing separator. `RemoveDirectoryA(p)` (Win32); success: true.
2. Failure without `recursive`: false. With it: false also when the error is "file not found" (2), "path not
   found" (3) or "write protected" (19, `0x13`), or when `p` is not an existing directory.
3. Otherwise build a `cRZDirectory` on `p` (not read, no dots); for each subdirectory, `RemoveDirectoryA(p + name,
   true)`, stopping at the first failure; if all succeeded, delete each file (`cRZFile::Remove`, `0x0091A1A6`),
   stopping at the first failure; if all succeeded, `RemoveDirectoryA(p)` again and return its result; else false.

Callers: deleting a region (`cSC4Nation::DeleteRegion`, `cSC4Region::Delete`), resetting the tutorial cities, deleting
a snapshot album, and the file browser.

### 3.9 `SplitDirectoryPath(path, last, parent)` (static, `0x0091B017`)

§1.6 with two parts: `last :=` part 0, `parent :=` part 1, true; false (both unchanged) when §1.6 fails. When the
path has only one part, `parent` is read from a buffer §1.6 did not write *[quirk]*; *[choice]* FreeCity4 gives an
empty `parent`.

### 3.10 Directory-entry owner and string-list sort (PA5, PA8)

The native named out-of-line `cRZDirectoryEntry` inventory is **three bodies, 94 bytes**, not an inferred
COM interface: default constructor `0x0091ADC2` (22 bytes), copy constructor `0x0091AEED` (41), deleting
destructor `0x0091ADFF` (31). There is no vptr in the constructed layout (§3.1). No separate assignment,
getter, name/path composition, comparison or ownership method is established by this inventory; list-template
operations are not additional entry methods.

- Default construction sets the owner pointer null, constructs an empty name string at +4, then sets the
  32-bit kind word at +0x18 to zero (`0x0091ADC2–0x0091ADD7`). Kind zero is the default, not file kind 8.
- Copy construction copies the owner pointer verbatim, copy-constructs the **whole name string**, then copies
  the kind word (`0x0091AEED–0x0091AF15`). It performs no directory AddRef and does not resolve a full path.
  The copied name has an independent string lifetime; the directory pointer remains borrowed.
- Destruction destroys only the name. Flags bit 0 additionally frees the entry allocation; it neither releases
  nor destroys the directory (`0x0091ADFF–0x0091AE1D`). Other flag bits do not select extra cleanup.
- `GetNthEntry` returns the actual list-node payload (node +8), with no copy/reference increment
  (`0x0091AD83–0x0091ADA6`). A reread erases every old entry **before** attempting the new search, including
  on a failed search (`0x0091BA73–0x0091BAA8`). Those borrowed pointers and their names then become invalid.
  Directory destruction also destroys the list after its filter/path strings (`0x0091B2F7–0x0091B311`).
  Changing path with `read=false` keeps the existing entries and pointers alive, with the owner directory now
  holding the new path; `read=true` invalidates them through reread (`0x0091BCAD–0x0091BCD6`). A standalone
  copied name survives reread/directory destruction, but its copied owner pointer does not keep the owner alive.

**Signature correction:** `AlphabetizeEntryList` (`0x0091AE1E–0x0091AE30`) is a static utility taking a
**list of strings**, with no comparison callback. The native caller builds a directory-name string list and
passes that same list (`0x009A4F6C–0x009A4F7E`). It does not pass a directory object, an entry list or a
function pointer. Thus it neither rearranges nor invalidates the entries returned by `GetNthEntry`.

Its list sort (`0x007B75F0`) merges by case-sensitive unsigned-byte lexicographic order, comparing counted
string spans with the shorter length first, then lengths when the common prefix matches
(`0x007B7158–0x007B71B2`). Embedded NUL is a byte in that comparison. It splices list nodes, rather than
copying strings. Equal strings keep their original order: a right-run element moves ahead only when it is
strictly less; equality advances the left-run cursor (`0x007B71B2–0x007B71F8`). For example `b`, `A`, `a`
sorts to `A`, `a`, `b`; two equal `a` nodes retain identity/order. A portable caller-supplied name comparator
is an extension, not this native method's contract.

### 3.11 SplitDirectory byte-boundary vectors (PA6)

The 255 checks in §1.6 use byte/index arithmetic, **not** a Unicode character count. Capacity and intermediate
indices are signed 32-bit values. For a nonempty input, capacity zero yields **true with count 0**, without
writing any part (`0x00917D68–0x00917D70`, `0x00917E14–0x00917E6B`). Empty input still fails with count 0.
An input consisting of one byte (`x` or a single separator) succeeds with count 0 even with spare capacity:
the final prefix is emitted only when its tracked end index is positive (`0x00917D39`, `0x00917E10`).

The bound applies to the **computed span**, which differs by branch. A suffix split at a separator copies
`trackedEnd−separatorIndex` bytes, including an existing trailing separator, then rejects span ≥255
(`0x00917DD4–0x00917DED`, `0x00917DA3–0x00917DB1`). The no-earlier-separator branch instead copies the
tracked end index bytes, accounting for §1.6's lost last character on a bare name. The final aggregate prefix
also checks its tracked end index against 255; short individual components do not exempt a long prefix
(`0x00917E10–0x00917E39`). A rejection resets count to 0 and returns false; earlier output writes need not
be undone. A separator already present is retained; otherwise a backslash and NUL are appended.

| Input (N denotes that many `x` bytes) | Capacity | Result |
|---|---:|---|
| `abc` | 0 | true, count 0; output untouched |
| 254 `x` followed by `\` | 2 | true, one part: 254 `x` plus `\` |
| 255 `x` followed by `\` | 2 | false, count 0 |
| `a\` + 254 `x` + `\` | 2 | false: suffix span is 255 including its existing separator |
| `a\` + 253 `x` + `\` | 2 | true: suffix span 254, then `a\` |
| `\a\` | 8 | true, only `a\`; the leading root separator is not a separate part |
| `C:\a\` | 8 | true, `a\`, then `C:\` |

With capacity 1, a remaining aggregate prefix of at least 255 bytes is rejected even when all of its component
names are short. The previously published assumption that any root is retained as its own part is incorrect.

## 4. Finding files (`RZFileFindFirst`, next, `RZFileFindFinish`)

`RZFileFindFirst(pattern)` (`0x0091B0A1`) converts the pattern to the system code page (`ConvertStringEncoding`,
`0x0091152F`) and calls `FindFirstFileA`; on success it returns a 0x30-byte search object holding the name of the
match (converted back), whether it is a directory (attribute bit 4, `FILE_ATTRIBUTE_DIRECTORY`) at `+0x14`, the
pattern and the search handle at `+0x2C`; otherwise null. The next-match function (`0x0091B21D`) calls
`FindNextFileA` and refreshes the name and the directory flag (false at the end). `RZFileFindFinish` (`0x0091AE66`)
closes the handle and frees the object. Every pattern is the system's: `*` and `?` wildcards, case-insensitive
matching, short-name matches *[compat-limit]*. The order of matches is the file system's (NTFS returns names in its
collation order); nothing here sorts *[compat-limit]*.

Four function pointers (`0x00B6291C`, `0x00B62920`: find first and next; `0x00B62924`, `0x00B62928`: create and
remove directory) select alternate paths when populated. No built-in writer or public installation entry was
identified; with the pointers zero the ordinary ANSI paths apply *[inferred]*. This is not proof that an
external plugin can never populate them (PA1).

*[compat]* The alternate find path converts the input pattern to UTF-16, supplies a wide find-data buffer,
and converts the returned wide name back to framework encoding (`0x0091B0BB–0x0091B143`,
`0x0091B23E–0x0091B276`; input conversion `0x0091AF87–0x0091AFF1`). Create-directory selects its
UTF-16 path only when **both** create and remove pointers are nonnull (`0x0091B822–0x0091B846`),
so rollback can use the remove pointer (`0x0091B90F–0x0091B91E`). Remove-directory tests its own pointer
independently (`0x0091BFDA–0x0091BFF9`, `0x0091C1E9–0x0091C209`). Search cleanup still closes the
returned handle through the ordinary Win32 close operation (`0x0091AE6F–0x0091AE78`). These are concrete
wide-path branches; their existence does not establish a published plugin hook API.

### 4.1 Exhaustion state (PA7)

`RZFileFindNext` returns false for a null search object or a failed next call (hook or Win32 branch). On either
next-call failure it leaves the cached name, directory flag **and handle** unchanged
(`0x0091B246–0x0091B24A`, `0x0091B280–0x0091B288`, `0x0091B2D1–0x0091B2D5`). Only success assigns
the new name/flag. It neither closes the handle nor marks the object exhausted. Calling next again reissues
the provider call using the same stored handle; there is no native terminal-state guard. The separate finish
operation still owns closure. Automatic close-on-first-failure is a portable lifetime policy, not this body.

## 5. Users

- **Plugins and data**: `cSC4App::AddCodePlugins` (the DLL plugins), `cSC4App::SetupResources`,
  `cSC4App::CreateDBSegmentCallback` and `cGZPersistDBSegmentMultiPackedFiles::Open` (the data files) list
  directories with `cRZDirectory`.
- **Regions**: `cSC4Nation::LoadRegions`, `cSC4Region::LoadCitiesFromDirectory`, region deletion (§3.8).
- **Other**: the snapshot album, `cSC4MySimPool::Import`, the radio stations (`cRZRadio::SetupDiskStations`,
  `cRZDiskRadioStation`), the file browser window (`cGZWinFileBrowser`), and `cRZPath` users such as the Gimex image
  reader and writer, the region city cache and the script resource key's `basename` (lua-host.md §7.1).

## 6. Quirks

1. `IsFullPath` rejects any path containing `\.` or `/.`, so such paths are re-normalised (§1.1).
2. Separators are kept as typed; only a restored trailing separator or a drive root gets `\` (§1.2).
3. A `..` above the first component reads outside the work buffer (§1.2).
4. Any character before a colon is a drive in `Splitpath`; `FixDriveString` replaces a non-letter with `C` (§1.5,
   §2.2).
5. `SplitDirectory` drops the last character of a single component without a trailing separator, and
   `SplitDirectoryPath` then reads an unwritten parent (§1.6, §3.9).
6. The recursive listing moves the directory object and uses a global depth counter (§3.5).
7. `.` and `..` are added at the front only for the filter `*` (§3.3, §3.4).
8. A failed `CreateDirectoryA` leaves the parents it created (§3.7).
9. `C:/` normalises to `C:\\` (the root fix and the restored trailing separator both apply) (§1.2).

## 7. Open questions

- **PA1:** (§4) the four replacement function pointers `0x00B6291C`–`0x00B62928`: nothing in the executable writes
  them; are they part of a framework hook a plugin can set (for example for wide-character paths)?
- **PA2:** (§3.5) which callers pass `includeTop` false, and is the plugin loader's directory order (the system's
  find order, §4) relied on by mods that depend on load order?

## 8. Conformance tests

With a scripted file system and current directory `C:\Games\SC4` on drive C:

1. **Full path:** `IsFullPath`: `C:\a` true, `\\s\x` true, `C:/a` false, `\x` false, `C:` false, `1:\x` false,
   `C:\a/.b` false.
2. **Normalise:** against `C:\Games\SC4`: `adv_const.lua` → `C:\Games\SC4\adv_const.lua`; `..\x.dat` →
   `C:\Games\x.dat`; `C:\a\..\b.txt` → `C:\b.txt`; `C:\a\.\b` → `C:\a\b`; `C:\a\` → `C:\a\`; `C:/` → `C:\`;
   `C:\..` fails; `C:/a/b.txt` → `C:\a/b.txt` (not full: its third character is `/`); `C:/` → `C:\\`;
   `a\..\..\x` → `C:\Games\x`; `C:\a\\b` stays as it is (full); `C:\a/./b` → `C:\a/b`;
   `d:rel.txt` with `D:` current directory `D:\Data` → `D:\Data\rel.txt`; `\x\y.z` → `C:\x\y.z`;
   `\\srv\share\f.txt` is full and untouched; `\\srv\..\x` → `\\x`.
3. **Splitpath:** `C:\x\advisors\Taxi.lua` → `C:`, `\x\advisors\`, `Taxi`, `.lua`; `.lua` → `""`, `""`, `""`, `.lua`;
   `a.b.c` → `a.b`, `.c`; `C:foo.txt` → `C:`, `""`, `foo`, `.txt`; `a\b.c\d` → dir `a\b.c\`, name `d`, no extension;
   a 300-character name is cut to 255.
4. **cRZPath:** from `adv_const.lua`: `GetPath` = `C:\Games\SC4\adv_const.lua`; `GetDriveAndDirectory` =
   `C:\Games\SC4\`; `GetFileNameAndExtension` = `adv_const.lua`; `SetExtension("dat", false)` then `GetPath` ends
   with `adv_const.dat`; `SetFileName("x\y.txt", true)` keeps only `y`; `FileName()` after `SetFileName("", false)`
   re-reads `adv_const` from the buffer.
5. **SplitDirectory** (two parts): `C:\a\b\` and `C:\a\b` → `b\`, `C:\a\`; `abc\` → one part `abc\`; `abc` → one part
   `ab\`; empty → false, count 0.
6. **Listing:** a directory holding `b.dat`, `a.dat` and subdirectory `sub`, which the system returns in the order
   `.`, `..`, `a.dat`, `b.dat`, `sub`: with `includeParent` and `includeSelf` false, `entries` = `a.dat` (8),
   `b.dat` (8), `sub` (4); with both true, `.` and `..` lead; the file list (flags 8) is `a.dat`, `b.dat`; with filter
   `*.dat` and both dot flags, no `.` or `..` is added; on a drive root (no dot entries from the system) with both
   dot flags and the filter `*`, `.` and `..` lead the list.
7. **Recursive:** for `C:\P\` with `x.dat` and `sub\y.dat` and `includeTop` true: `C:\P\x.dat`, `C:\P\sub\y.dat`; with
   `includeTop` false: only `C:\P\sub\y.dat`.
8. **Create and remove:** `CreateDirectoryA("C:\n\m")` creates `C:\n\` and `C:\n\m\` and returns true; for
   `C:\n\m\k` with the creation of `k` failing (scripted) and neither `n` nor `m` existing, the result is false,
   `C:\n\m` is removed and `C:\n` remains; `RemoveDirectoryA("C:\n", true)` removes the tree;
   `RemoveDirectoryA("C:\missing", true)` is false.
