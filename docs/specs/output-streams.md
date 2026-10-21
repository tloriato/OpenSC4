# Spec: output streams (`cRZOStream`, `cRZTextOStream`, `cRZNullOStream`, `cRZOFStream`, and the input-side `cRZIStream`, `cRZNullIStream`)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`). Implementers must work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled
or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[compat-limit]* depends on something FreeCity4 cannot reproduce (here the C runtime's locale and
the host's file system), so only the rule is matched, not each value.

Related specs: [foundations.md](foundations.md) (§2, the save-file segment stream, whose number encodings these
classes share but whose string encoding they do not), [data-formats.md](data-formats.md) (§2, the DBPF records
whose record objects contain a `cRZOStream`), [save-records.md](save-records.md) (record layouts; written through
the segment stream of foundations §2, not through the methods here).

## 0. Summary

`cRZOStream` is the framework's abstract **binary output stream**. It implements the `cIGZOStream` interface except
`Flush`, `SetVoid` and `GetError`: every typed write turns the value into bytes and hands them, in **one** call, to
the derived class's `SetVoid(p, n)`. A per-object flag picks the byte order of integers (little-endian when true,
byte-reversed when false); floats are never reversed. Strings carry a 7-bit variable-length size prefix.

`cRZTextOStream` derives from it and writes **text**: numbers become decimal strings (`printf` formats `%d`, `%u`,
`%lld`, `%llu`, `%g`) that it hands, one value per call, to the derived class's `SetRZCharStr(s)`. It adds no
separators, no line ends and no padding.

Neither class buffers anything; buffering, errors and the final destination belong to the concrete class: the null
stream (§3, discards everything), the file stream (§4, an STL `ofstream`, binary or text mode) and the DBPF record
objects (§6). In the shipped game every file stream is created in binary mode, so `cRZTextOStream`'s code is never
reached (§2.5), and the base's string encoding is reached only through the null stream (§1.5).

## 1. `cRZOStream` *[compat]*

### 1.1 Object (16 bytes)

| Offset | Field | Initial |
|---|---|---|
| `+0x00` | vtable (`0x00AD1818` for the base itself, §1.3) | |
| `+0x04` | reference count (u32) | 0 |
| `+0x08` | `littleEndian` (bool, one byte): true writes integers in the host's order (little-endian), false byte-reversed (big-endian) | the constructor's argument |
| `+0x0C` | user data: a counted interface pointer (the SDK calls it `cIGZVariant*`) | null |

### 1.2 Construction and destruction

- `cRZOStream(bool littleEndian)` (`0x0092BBDE`, pops 4): the fields of §1.1. Derived constructors may inline it:
  `cRZNullOStream` stores false, `cRZOFStream` stores true and then its own argument (§4.1).
- Destructor (`0x0092B62D`): resets the vtable to `0x00AD1818` and releases the user data if it is set (its slot 2).
- Deleting destructor (slot 21, `0x0092BBF8`, pops 4): the destructor, then frees the object to the global heap when
  bit 0 of the argument is set; EAX = this.

### 1.3 Slot table (vtable `0x00AD1818`, 24 slots)

Pops are bytes popped by `ret`. "Sink" is the derived class's `SetVoid` (slot 17); "the sink's result" is passed
through unchanged (callers read AL). The SDK's `cIGZOStream` declares slots 0–20 in this order.

| Slot | Offset | Method | Address | Pops | Behaviour / return |
|---|---|---|---|---|---|
| 0 | `+0x00` | `QueryInterface(iid, void** out)` | `0x0092B2A3` | 8 | §1.6; AL |
| 1 | `+0x04` | `AddRef()` | `0x00968B60` (shared) | 0 | count + 1; EAX the new count |
| 2 | `+0x08` | `Release()` | `0x0092B2C8` (shared with `cRZIStream`) | 0 | §1.6; EAX the new count, 0 when deleted |
| 3 | `+0x0C` | `Flush()` | pure | | |
| 4 | `+0x10` | `SetSint8(v)` | `0x0092B267` | 4 | `SetVoid(&v, 1)`; the sink's result |
| 5 | `+0x14` | `SetUint8(v)` | `0x0092B267` (same body) | 4 | as slot 4 |
| 6 | `+0x18` | `SetSint16(v)` | `0x0092B640` | 4 | 2 bytes, §1.4 |
| 7 | `+0x1C` | `SetUint16(v)` | `0x0092B640` (same body) | 4 | as slot 6 |
| 8 | `+0x20` | `SetSint32(v)` | `0x0092B676` | 4 | 4 bytes, §1.4 |
| 9 | `+0x24` | `SetUint32(v)` | `0x0092B676` (same body) | 4 | as slot 8 |
| 10 | `+0x28` | `SetSint64(v)` | `0x0092B6A3` | 8 | 8 bytes, §1.4 |
| 11 | `+0x2C` | `SetUint64(v)` | `0x0092B6A3` (same body) | 8 | as slot 10 |
| 12 | `+0x30` | `SetFloat32(f)` | `0x0092B276` | 4 | the 4 bytes of `f` as stored in memory, never reversed |
| 13 | `+0x34` | `SetFloat64(d)` | `0x0092B285` | 8 | the 8 bytes of `d`, never reversed |
| 14 | `+0x38` | `SetRZCharStr(char const* s)` | `0x0092B734` | 4 | §1.5 |
| 15 | `+0x3C` | `SetGZStr(cIGZString const& s)` | `0x0092B765` | 4 | §1.5 |
| 16 | `+0x40` | `SetGZSerializable(cIGZSerializable const& o)` | `0x0092B294` | 4 | `o.Write(*this)` (the object's slot 3) and nothing else; its result |
| 17 | `+0x44` | `SetVoid(void const* p, uint32 n)` | pure | | the sink |
| 18 | `+0x48` | `GetError()` | pure | | |
| 19 | `+0x4C` | `SetUserData(p)` | `0x0092B9BB` (shared with `cRZIStream`) | 4 | §1.6 |
| 20 | `+0x50` | `GetUserData()` | `0x009CEDE1` (shared getter) | 0 | EAX `+0x0C`, no AddRef |
| 21 | `+0x54` | deleting destructor | `0x0092BBF8` | 4 | §1.2 |
| 22 | `+0x58` | `Init(bool littleEndian)` | `0x0092B25B` (shared with `cRZIStream`) | 4 | `+0x08 :=` the argument; AL 1 |
| 23 | `+0x5C` | `Shutdown()` | `0x00949ADE` (shared) | 0 | does nothing; AL 1 |

Slots 22 and 23 carry the Mac build's names; `Init` is the only way to change the byte order after construction.
No method keeps an error state or refuses to write after a failure: each call returns what its sink calls returned,
and stickiness, if any, is the sink's (§4.2).

### 1.4 Numbers *[compat]*

Each write is exactly one `SetVoid(p, n)` call with `n` = the width (1, 2, 4 or 8 bytes); there is no alignment,
padding, tag or count.

- **8-bit**: the byte itself, whatever the flag. Signed and unsigned writes are the same body.
- **16, 32 and 64-bit, `littleEndian` true**: the value's bytes, least significant first.
- **16, 32 and 64-bit, `littleEndian` false**: the same bytes in reverse order (most significant first). For 64
  bits the whole 8-byte value is reversed (helper `0x0092B55B`, which the DBPF serial record's 64-bit field methods
  also use).
- **Floats**: the IEEE-754 bit pattern as stored by the host (little-endian), **also when `littleEndian` is false**
  *[quirk]*: a big-endian stream writes big-endian integers and little-endian floats.
- **Booleans** have no method; by convention they are `SetUint8(0 or 1)` (foundations.md §2.2).

### 1.5 Strings (the base encoding)

`SetRZCharStr(s)`: `len := strlen(s)` (so `s` must not be null and stops at its first NUL); write `len` as below; then,
if `len > 0`, `SetVoid(s, len)`. No terminator is written.

`SetGZStr(s)`: `len := s.Strlen()` (string slot 7, `+0x1C`, so embedded NULs count); write `len`; then, if
`len > 0`, `SetVoid(s.ToChar(), len)` (string slot 5, `+0x14`).

The **length** (`EncodeStringLength`, `0x0092B6E2`, pops 4) is an unsigned LEB128 number written byte by byte with
`SetUint8` (slot 5, a virtual call): seven bits at a time, least significant group first, bit 7 set on every byte
but the last. 0 is the single byte `00`; 1–127 one byte; 128 is `80 01`; 300 is `AC 02`; 16,384 is `80 80 01`;
2³² − 1 is `FF FF FF FF 0F`.

Return value *[quirk]*: the results of the length bytes are ignored. An empty string returns true (AL 1) whatever
the sink did; a non-empty one returns the result of the final `SetVoid` alone.

This is **not** the save-file string format (foundations.md §2.3, a 32-bit header): the DBPF record classes, the
file stream and the text stream all override slots 14 and 15, so in 1.1.641 this encoding reaches only the null
stream, which discards it. *[choice]* A replacement must keep the slot behaviour for plugins that hold a
`cRZOStream`-based stream, but no shipped data depends on the bytes.

### 1.6 Reference counting, interfaces, user data

- `QueryInterface(iid, out)`: for IID **1** (`cIGZUnknown`) or **`0xC51B11CB`** (`cIGZOStream`; the DBPF record and
  save segment streams answer the same IID) it stores **this** in `*out`, calls `AddRef`, returns true. Any other
  IID returns false and leaves `*out` unwritten. The stream is not reachable as a `cIGZIStream` (`0x051B11BD`).
- `Release()`: when the count is **at most 1** (unsigned, so also 0) it calls the deleting destructor with 1 and
  returns 0, without decrementing first; otherwise it decrements and returns the new count *[quirk]* (as dates.md
  §3.1).
- `SetUserData(p)`: a counted assignment to `+0x0C`. When `p` differs from the stored pointer: `p.AddRef()` (if not
  null), store it, then release the old one (if not null). The same pointer changes nothing. EAX is the address of
  the field (`this + 0x0C`), not a status; callers ignore it.
- `GetUserData()`: the stored pointer, without AddRef. The destructor releases it (§1.2). The simulation does not
  use the user data (foundations.md §2.1).

## 2. `cRZTextOStream`

### 2.1 Object and slot table (vtable `0x00AD1998`, 24 slots)

A `cRZOStream` with no fields of its own (16 bytes). It is abstract: `Flush` and `SetRZCharStr` stay pure, and the
derived class's `SetRZCharStr` is the **text sink**. It has no constructor of its own (the derived class sets the
base fields directly) and ignores the byte-order flag. Slots not listed are `cRZOStream`'s (§1.3).

| Slot | Offset | Method | Address | Pops | Behaviour / return |
|---|---|---|---|---|---|
| 3 | `+0x0C` | `Flush()` | pure | | |
| 4 | `+0x10` | `SetSint8(v)` | `0x0092C7AD` | 4 | text of `v` sign-extended, format `%d` (§2.2) |
| 5 | `+0x14` | `SetUint8(v)` | `0x0092C808` | 4 | `v` zero-extended, `%u` |
| 6 | `+0x18` | `SetSint16(v)` | `0x0092C863` | 4 | sign-extended, `%d` |
| 7 | `+0x1C` | `SetUint16(v)` | `0x0092C8BE` | 4 | zero-extended, `%u` |
| 8 | `+0x20` | `SetSint32(v)` | `0x0092C919` | 4 | `%d` |
| 9 | `+0x24` | `SetUint32(v)` | `0x0092C972` | 4 | `%u` |
| 10 | `+0x28` | `SetSint64(v)` | `0x0092C9CB` | 8 | `%lld` |
| 11 | `+0x2C` | `SetUint64(v)` | `0x0092CA27` | 8 | `%llu` |
| 12 | `+0x30` | `SetFloat32(f)` | `0x0092CA83` | 4 | `f` widened (exactly) to a double, `%g` |
| 13 | `+0x34` | `SetFloat64(d)` | `0x0092CAE1` | 8 | `%g` |
| 14 | `+0x38` | `SetRZCharStr(s)` | pure | | the text sink |
| 15 | `+0x3C` | `SetGZStr(s)` | `0x0092B355` | 4 | `SetRZCharStr(s.ToChar())` (string slot 5); its result |
| 17 | `+0x44` | `SetVoid(p, n)` | `0x0092B36F` | 8 | hex dump (§2.4); EAX 0 / 1 |
| 18 | `+0x48` | `GetError()` | `0x00949ADE` (shared) | 0 | AL 1 ("error"), never observable (§2.5) |
| 21 | `+0x54` | deleting destructor | `0x0092BCB9` | 4 | vtable `:= 0x00AD1998`, the `cRZOStream` destructor, free on bit 0 |

### 2.2 Numbers as text *[compat]*

Each number write formats the value into a temporary string with the framework's `cRZString::Sprintf`
(`0x0090F574`) and calls `SetRZCharStr` once with it; the result is that call's AL. The format strings are read at
each call through five pointers in data that nothing writes: `0x00B169B0` → `"%d"`, `0x00B169B4` → `"%u"`,
`0x00B169B8` → `"%lld"`, `0x00B169BC` → `"%llu"`, `0x00B169C0` → `"%g"`.

`cRZString::Sprintf` treats `ll` (and `I64`) as 64-bit and hands each conversion to the C runtime (`sprintf` with
`%d`, `%u`, `%I64d` or `%I64u`; `_snprintf` with `%g`), passing a negative value as its magnitude and adding the
minus sign itself. That gives the standard results:

- Integers: plain decimal, a leading `-` for negative signed values, no `+`, no padding, no grouping:
  `-128`, `255`, `-2147483648`, `4294967295`, `-9223372036854775808`, `18446744073709551615`.
- Floats, `%g` of the MSVC 7 C runtime: 6 significant digits, trailing zeros and a trailing point removed;
  exponent form when the decimal exponent is below −4 or at least 6; the exponent has a sign and **at least three
  digits** (`1e+006`, `1.23457e+006`, `1e-005`), from the runtime's exponent template `e+000` (`0x00AE685C`, used by
  `0x009F692D`) *[compat]*. Negative values get `-`; the comparison is with 0.0, so −0.0 is formatted by the runtime
  itself *[inferred: "-0"]*. Infinities and NaNs use the runtime's `1.#INF`, `1.#QNAN`, `1.#IND` forms (strings at
  `0x00AECFF0`–`0x00AED008`) *[inferred]*.
- The decimal point is the C runtime's current one (`0x00B19C60`). The game changes the runtime locale with
  `setlocale(LC_ALL, …)`: to the user's default the first time a code-page query finds `"C"` (`0x00910014`), and to
  the language's locale in `cRZLanguageManager::SetCurrentLanguage` (`0x00928021`). In a locale with a decimal comma
  a float would print with a comma *[compat-limit]*.

Nothing is added between values: `SetUint32(1)` then `SetUint32(2)` produce `12` *[quirk]*.

### 2.3 Strings

`SetGZStr(s)` writes the characters of `s.ToChar()` through the sink, which takes a NUL-terminated string: the text
stops at the first NUL of `s` *[quirk]*. No length, quotes or terminator are written. `SetRZCharStr` is the
derived class's (§4.2).

### 2.4 Raw bytes as a hex dump (`SetVoid(p, n)`, `0x0092B36F`)

Written through `SetRZCharStr`, one call per piece:

1. `"\nHEXDUMP:\n"` (LF, the text, LF).
2. For each byte `i = 1 … n`: `sprintf("%02x ", b)` where `b` is the byte **sign-extended** to an int, so 0x00–0x7F
   print as two digits and 0x80–0xFF as eight (`ffffff80 `) *[quirk]*. After every 25th byte, `"\n"`.
3. A final `"\n"`, so a count that is a multiple of 25 (and not 0) ends with an empty line *[quirk]*; `n = 0`
   writes `"\nHEXDUMP:\n\n"`.

The dump stops at the first sink call that returns false and returns 0; after the final `"\n"` succeeds it returns
1 (all of EAX).

### 2.5 Reachability in 1.1.641

The only concrete text stream is `cRZOFStream` in text mode (§4), and both places that create a `cRZOFStream` pass
binary mode (§4.3); the file stream also overrides `SetVoid`, so the hex dump is unreachable even in text mode.
Plugins cannot create either class (no class ID). *[choice]* FreeCity4 need not implement `cRZTextOStream`; if it
does, §2.2–§2.4 are the reference.

## 3. `cRZNullOStream` *[compat]*

A `cRZOStream` (16 bytes, vtable `0x00AD18D8`) that discards everything. The constructor (`0x0092BC4A`, no
arguments) zeroes the count and the user data and sets `littleEndian` to **false**. The slots are `cRZOStream`'s
except:

| Slot | Method | Address | Behaviour |
|---|---|---|---|
| 3 | `Flush()` | `0x00A6D837` (shared, a bare `ret`) | nothing |
| 17 | `SetVoid(p, n)` | `0x0092B33A` (shared) | nothing; AL 1; pops 8 |
| 18 | `GetError()` | `0x0094A5EA` (shared) | AL 0 |
| 21 | deleting destructor | `0x0092BC5E` | vtable `:= 0x00AD18D8`, the `cRZOStream` destructor, free on bit 0 |

So every write returns true, `GetError()` is always 0, and `SetGZSerializable(o)` still runs `o.Write` against it.

## 4. `cRZOFStream`, the file sink

### 4.1 Object and construction

A `cRZTextOStream` of 24 bytes (vtable `0x00AD19F8`):

| Offset | Field |
|---|---|
| `+0x00`–`+0x0F` | `cRZOStream` (§1.1) |
| `+0x10` | the STL `ofstream` (a 0x118-byte heap object), or null if that allocation failed |
| `+0x14` | `binary` (bool) |
| `+0x15` | `autoFlush` (bool) |

Constructors `cRZOFStream(path, binary, littleEndian, autoFlush, append)` (pops 0x14): `0x0092CE7B` takes the path as
a `cIGZString const&` (its slot 5), `0x0092CEF6` as a `char const*`. They open the file with the STLport mode
`out`, plus `binary` when `binary` is true, plus `app` when `append` is true, else `trunc`; a failed open leaves the
stream's failbit set. The destructor (`0x0092B9C3`) deletes the `ofstream` (which closes the file), then runs the
base destructors; the deleting destructor is `0x0092BCDB`.

### 4.2 Slot table (vtable `0x00AD19F8`)

Slots 0–2, 16 and 19–23 are `cRZOStream`'s; slot 15 is `cRZTextOStream`'s (§2.3).

| Slot | Method | Address | Behaviour / return |
|---|---|---|---|
| 3 | `Flush()` | `0x0092BCF7` | `ofstream.flush()`; a failed sync sets badbit; EAX not a status |
| 4–11 | the integer writes | `0x0092CB3F`, `0x0092CB4F`, `0x0092CB5F`, `0x0092CB6F`, `0x0092CB7F`, `0x0092CB8F`, `0x0092CB9F`, `0x0092CBBC` | `binary`: `cRZOStream`'s (§1.4); else `cRZTextOStream`'s (§2.2) |
| 12, 13 | `SetFloat32`, `SetFloat64` | `0x0092CBD9`, `0x0092CBF6` | the same choice |
| 14 | `SetRZCharStr(s)` | `0x0092C757` | `binary`: `SetVoid(s, strlen(s))`, **no length prefix and no terminator**; text: `ofstream << s`. Result: AL = the stream state is good. Then, if `autoFlush`, `Flush()` (its outcome is not in the result) |
| 17 | `SetVoid(p, n)` | `0x0092C1EE` | `ofstream.write(p, n)` in both modes; AL = the state is good |
| 18 | `GetError()` | `0x0092B9EE` | AL = any of the stream's bad, fail or eof bits is set |

Errors are **sticky**: once the `ofstream` has a bad or fail bit, every later write fails (the STL refuses and sets
badbit) and `GetError()` stays true. The STL's own buffer is the only buffering; `autoFlush` flushes after each
string or text value, not after binary numbers.

### 4.3 Where the game creates them

| Site | Path | binary | littleEndian | autoFlush | append |
|---|---|---|---|---|---|
| `SC4CreateGZLog` (`0x005C6E80`; called by `cSC4App::PreFrameWorkInit` `0x00452360` and `cGZMiscCommandSystemSupport::DoMessage` `0x005C8B30`) | its first argument | true | **false** | the negation of its third argument | its fourth argument (when false the file is first deleted with `cRZFile::Remove`) |
| `cSC4ViewInputControlQuery::WriteDetailedDebugInfo` (`0x004D12A0`, a stack object, flushed at `0x004D30FC`) | `Query.txt` | true | **false** | false | false |

Both are big-endian for integers *[quirk]* and binary, so strings are written raw: the text a caller passes,
byte for byte, with its own `\n` line ends and no CR added.

## 5. The input-side sibling (`cRZIStream`, `cRZNullIStream`)

Recorded here because its bodies sit with the output classes and two of them are shared (§1.3). It mirrors §1:
16 bytes, the same layout and flag (`+0x08`), constructor `cRZIStream(bool littleEndian)` (`0x0092BC14`),
destructor `0x0092B79F`, deleting destructor `0x0092BC2E`, vtable `0x00AD1878` (the `cIGZIStream` order: slot 3
`Skip`, 17 `GetVoid` and 18 `GetError` pure). `QueryInterface` (`0x0092B318`) accepts IIDs 1 and `0x051B11BD`.

| Slot | Method | Address | Behaviour |
|---|---|---|---|
| 4, 5 | `GetSint8`, `GetUint8(out&)` | `0x0092B2DF` | `GetVoid(&out, 1)`; its result |
| 6, 7 | `GetSint16`, `GetUint16` | `0x0092B7B2` | `GetVoid(&out, 2)`; on success, reversed in place when the flag is false; AL 1, else 0 (`out` then holds whatever `GetVoid` left) |
| 8, 9 | 32-bit | `0x0092B7F1` | as above, 4 bytes |
| 10, 11 | 64-bit | `0x0092B82B` | as above, 8 bytes (helper `0x0092B55B`) |
| 12, 13 | `GetFloat32`, `GetFloat64` | `0x0092B2ED`, `0x0092B2FB` | `GetVoid(&out, 4 / 8)`, never reversed |
| 14 | `GetRZCharStr(buf, max)` | `0x0092B8BC` | length as §1.5; if `GetError()` or length 0: `buf[0] := 0`; else read `k = min(length, max)` bytes into `buf`, `buf[k] := 0` (**one past `max` when truncated**) *[quirk]*, and `Skip(length − k)` if no error. Returns `GetError() = 0` (EAX 0 / 1) |
| 15 | `GetGZStr(out&)` | `0x0092B92D` | `out.FromChar("")`; length; then chunks of at most 255 bytes appended to `out` while no error. Returns `GetError() = 0` |
| 16 | `GetGZSerializable(o&)` | `0x0092B309` | `o.Read(*this)` (the object's slot 4) |

The length decoder (`0x0092B85F`) reads bytes with `GetUint8` while bit 7 is set and combines the groups as in
§1.5; it collects them in an 8-byte buffer, so a malformed length of more than 8 bytes (8 continuation bytes and
more) writes past it *[quirk]*.

`cRZNullIStream` (vtable `0x00AD1938`, 20 bytes, constructor `0x0092BC80`, deleting destructor `0x0092BC97`) adds
an error byte at `+0x10` (0 at construction): `Skip` (`0x0092B33F`) and `GetVoid` (`0x0092B348`) set it to 1 and
return false, `GetError` (`0x0092B351`) returns it. Every read therefore fails and leaves its output unwritten.

## 6. Users

- **`cRZOStream` as a base**: the DBPF record objects. `cGZPersistDBSerialRecord` contains a `cRZOStream` at
  `+0x1C` and a `cRZIStream` at `+0x2C` and returns them for `QueryInterface(0xC51B11CB)` and `(0x051B11BD)`
  (`0x00975A17`); its derived records have their own output vtables (`0x00AD7C50` the serial record, `0x00AD7E48`,
  `0x00AD8048`, `0x00AD8248` the packed-file records, `0x00AD8588` `cGZDBWriteRecordPackedFile`, `0x00AD90D8`
  `cGZDBSDFRecord` with the stream at `+0x20`, `0x00AD9548` `cGZDBRecordBitmapRes`). They keep `cRZOStream`'s slots
  4–13, 16 and 19–23 and supply their own `Flush` (nothing), string slots, `SetVoid` (append to the record) and
  `GetError` (always 0). The constructor's flag also goes to the record's own field writers (`+0x3C`); the record
  creators seen pass **true**: `cGZDBSegmentPackedFile::DoOpenRecordForWrite` (`0x00971178`), `DoCreateNewRecord`
  (`0x009712D7`), `cGZDBSDFRecord` and `cGZDBRecordBitmapRes`. So DBPF data written through a record's
  `cIGZOStream` is little-endian, as data-formats.md and foundations.md §2.2 say.
- **The save-file segment stream** (`cSC4DBSegmentOStream`, vtable `0x00AA2310`) is **not** a `cRZOStream`: it keeps
  its own good flag and forwards each value to the record's field writers (record slots `+0x70` … `+0xC4`), which
  apply the same byte-order rule *[inferred: the forwarding target was read in the slot bodies, not traced per
  record class]*. Save-record layouts are in save-records.md and foundations.md §2.
- **`cRZTextOStream`**: only as the base of `cRZOFStream` (§2.5).
- **`cRZNullOStream`**: the framework's default standard output streams (`0x008796BD`, `0x0087970D`, created on first
  use when none was set; the SDK's `StdOut` / `StdErr` *[inferred]*), `cSC4App::AddSystemServices` (`0x00450350`)
  and `cGZPersistSurveyorWriter::OutputStream` (`0x009812D4`, a static one when the writer has no stream). The
  framework's default input stream (`0x0087975D`) is a `cRZNullIStream`.
- **`cRZOFStream`**: the GZ log and `Query.txt` (§4.3).

## 7. Quirks

1. Floats are never byte-reversed, so a big-endian stream mixes byte orders (§1.4).
2. The base string length is a LEB128 varint, unlike the save files' 32-bit header (§1.5).
3. The base string writes ignore the length bytes' results; an empty string always reports success (§1.5).
4. `Release` deletes at a count of 0 or 1 without decrementing (§1.6).
5. Text values are written with no separator between them (§2.2).
6. Float text has three-digit exponents and the runtime locale's decimal point (§2.2).
7. Text strings and binary file strings stop at the first NUL (§2.3, §4.2).
8. The hex dump sign-extends bytes 0x80–0xFF to eight hex digits (§2.4).
9. The hex dump ends with an empty line when the count is a non-zero multiple of 25 (§2.4).
10. `cRZTextOStream`'s own `GetError` reports an error (§2.1); unobservable.
11. Both file streams of the game write integers big-endian (§4.3).
12. A binary file stream writes `SetRZCharStr` without length or terminator, unlike the base class (§4.2).
13. `cRZIStream::GetRZCharStr` writes its terminator at `buf[max]` when the string is truncated, and the length
    decoder overruns its buffer on a length of more than 8 bytes (§5).

## 8. Open questions

- **OS1:** (§4.3) what does the GZ log write to its stream (`SC4CreateGZLog` `0x005C6E80`, the stream handed to the
  framework log)? Only strings, or also numbers (which would come out as raw big-endian bytes)? And which file name
  does `cSC4App::PreFrameWorkInit` pass?
- **OS2:** (§2.2, §4.1) only for a text-mode file stream, which the game never creates: does the STLport file
  buffer translate `\n` to CR LF in text mode, and which decimal point is in effect when such a stream would write
  a float (the C runtime locale after `0x00910014` and `SetCurrentLanguage`)?

## 9. Conformance tests

With a recording sink that keeps the arguments of every `SetVoid` call (binary) or `SetRZCharStr` call (text) and
returns a scripted result (true unless stated):

1. **Little-endian** (`littleEndian` true): `SetUint8(0xAB)` → `AB`; `SetSint8(-1)` → `FF`; `SetUint16(0x1234)` →
   `34 12`; `SetSint16(-2)` → `FE FF`; `SetUint32(0x12345678)` → `78 56 34 12`; `SetSint64(-2)` → `FE FF FF FF FF FF
   FF FF`; `SetUint64(0x0102030405060708)` → `08 07 06 05 04 03 02 01`. Each is one sink call of the width.
2. **Big-endian** (false): `SetUint8(0xAB)` → `AB`; `SetUint16(0x1234)` → `12 34`; `SetSint16(-2)` → `FF FE`;
   `SetUint32(0x12345678)` → `12 34 56 78`; `SetUint64(0x0102030405060708)` → `01 02 03 04 05 06 07 08`.
3. **Floats** with either flag: `SetFloat32(1.0)` → `00 00 80 3F`; `SetFloat64(1.0)` → `00 00 00 00 00 00 F0 3F`;
   `SetFloat32(-2.5)` → `00 00 20 C0`. `Init(false)` then `Init(true)` switches the integer order and returns true.
4. **Base strings**: `SetRZCharStr("")` → one call `00`; `SetRZCharStr("abc")` → calls `03` and `61 62 63`; a
   128-byte string → `80`, `01`, then the 128 bytes (three calls); 300 bytes → `AC`, `02`, the bytes; a
   `cIGZString` `"a\0b"` (Strlen 3) → `03`, `61 00 62`.
5. **String results**: with a sink that always fails, `SetRZCharStr("")` returns true and `SetRZCharStr("abc")`
   false; with a sink that fails only its first call, `SetRZCharStr("abc")` returns true.
6. **Serializable**: `SetGZSerializable(o)` calls `o.Write` once with the stream itself and returns its result;
   nothing is written by the stream.
7. **Counting and interfaces**: a new stream has count 0; `AddRef` → 1, 2; `Release` → 1; `Release` with count 1
   destroys it and returns 0. `QueryInterface(1)` and `(0xC51B11CB)` return true, the stream itself and count + 1;
   `(0x051B11BD)` returns false and leaves the out pointer as it was. `SetUserData(v)` adds a reference to `v`;
   `SetUserData(w)` releases `v`; destroying the stream releases `w`; `GetUserData()` returns `w` without a
   reference.
8. **Text numbers**: `SetSint8(-5)` → `"-5"`; `SetUint8(200)` → `"200"`; `SetSint16(-32768)` → `"-32768"`;
   `SetUint16(65535)` → `"65535"`; `SetSint32(-2147483648)` → `"-2147483648"`; `SetUint32(4294967295)` →
   `"4294967295"`; `SetSint64(-9223372036854775808)` → `"-9223372036854775808"`; `SetUint64(2⁶⁴ − 1)` →
   `"18446744073709551615"`. `SetUint32(1)`, `SetUint32(2)` → `"1"`, `"2"` (no separator).
9. **Text floats** (C locale): `SetFloat32(0.5)` → `"0.5"`; `SetFloat32(0.1)` → `"0.1"`; `SetFloat64(-2.5)` →
   `"-2.5"`; `SetFloat64(0)` → `"0"`; `SetFloat64(100000)` → `"100000"`; `SetFloat64(1000000)` → `"1e+006"`;
   `SetFloat64(1234567)` → `"1.23457e+006"`; `SetFloat64(0.0001)` → `"0.0001"`; `SetFloat64(0.00001)` →
   `"1e-005"`; `SetFloat64(1e100)` → `"1e+100"`.
10. **Text strings**: `SetGZStr("ab")` → `"ab"`; a `cIGZString` `"a\0b"` → `"a"`.
11. **Hex dump**: `SetVoid({00, 7F, 80, FF}, 4)` → `"\nHEXDUMP:\n"`, `"00 "`, `"7f "`, `"ffffff80 "`,
    `"ffffffff "`, `"\n"`, returns 1; `SetVoid(p, 0)` → `"\nHEXDUMP:\n"`, `"\n"`; 25 zero bytes → the header, 25 ×
    `"00 "`, `"\n"`, `"\n"`; 26 zero bytes → the header, 25 × `"00 "`, `"\n"`, `"00 "`, `"\n"`; the first 4-byte
    dump with a sink that fails its third call makes exactly three calls and returns 0.
12. **Null stream**: every `Set*` returns true and nothing is kept; `GetError()` = 0; `SetGZSerializable(o)` still
    calls `o.Write`.
13. **File stream** (on a temporary file): `(path, binary true, littleEndian false, autoFlush false, append false)`
    then `SetUint32(1)`, `SetRZCharStr("hi\n")`, `SetFloat32(1.0)`, destroy → the file is `00 00 00 01 68 69 0A 00 00
    80 3F`; reopening with `append` true and writing `SetUint8(7)` adds `07`; with `append` false the file restarts.
    A path that cannot be opened gives `GetError()` true and every write false.
14. **Reader round trip** (`cRZIStream`, same flag): every value of tests 1–4 reads back equal;
    `GetRZCharStr(buf, 2)` on the encoded `"abcde"` gives `buf = "ab\0"` (3 bytes written) and consumes all 6 bytes;
    `GetGZStr` of a 300-byte string returns it whole; a read from a null input stream returns false, leaves the output
    unchanged and makes `GetError()` 1.
