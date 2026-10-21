# Spec: command system (`cGZCommandParameterSet`, `cGZStringCommandGenerator`, `cGZMessage2CommandDispatcher`)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`). Implementers must work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled
or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[compat-limit]* depends on something FreeCity4 cannot reproduce, so only the rule is matched.

Related specs: [foundations.md](foundations.md) (§5.1 the standard message and `MessageSend`, §9.1 the variant
memory types), [data-formats.md](data-formats.md) (§5.1 the converting getters and `GetValString`'s text),
[command-line.md](command-line.md) (§2, `cRZCmdLine`'s own splitter, which differs from §2 here),
[graphics-formats.md](graphics-formats.md) (§3.11, `cRZString::MakeLower`), [lua-host.md](lua-host.md) (§9.4, the
Lua call closures' parameter sets), [flora.md](flora.md) (§12, `operator[]`), [civic-buildings.md](civic-buildings.md)
(§4.1, a registry callback's output), [network-rules.md](network-rules.md) (§8.1, another user of §2's splitter).

## 0. Summary

A **command** is a 32-bit command ID plus an input and an output **parameter set**. The framework's command server
(`cGZCommandServer`, §5) keeps the table of command names and owns one dispatcher and one string generator:

```
"SetDebugLevel 5"  --cGZStringCommandGenerator (§3)-->  id 0x2BB40833, in = {0: "5"}
   --server slot 20--> cGZMessage2CommandDispatcher (§4)
   --MessageSend--> message (type = id, data1 = in, data2 = out) --> every target subscribed to type id
   <-- status = out[0x2B999D2B] (0 when nobody set it)
```

- `cGZCommandParameterSet` (§1) is a sparse, ordered map from a 32-bit parameter ID to a variant.
- `RZParseDelimitedText` (§2) splits the command line into words (blanks, double quotes).
- `cGZStringCommandGenerator` (§3) converts text to a command (first word = name, looked up case-insensitively; the
  other words become **string** parameters 0, 1, …) and back.
- `cGZMessage2CommandDispatcher` (§4) executes a command by sending one reusable message synchronously. It has no
  table of its own: the handlers are ordinary message targets that subscribe to the command ID as a message type.

Nothing here is saved. All of it is reachable by plugins through `cIGZCommandServer` and the parameter-set interface.

## 1. `cGZCommandParameterSet` *[compat]*

### 1.1 State (0x28 bytes, global heap)

| Offset | Field |
|---|---|
| `+0x00` | vtable `0x00AD16C8` (16 slots, §1.3) |
| `+0x04` | reference count (starts at 0) |
| `+0x08` / `+0x0C` | the parameter map: an ordered tree keyed by the unsigned 32-bit parameter ID (header node, node count) |
| `+0x10` | the map's (empty) comparator |
| `+0x14` | `fallback`: a `cRZVariant` (0x14 bytes), Empty at construction, handed out by `operator[]` for a missing ID |

Each map node holds the ID and a `cRZVariant` value. The parameter count is the node count. IDs are keys, not
indexes: a set may hold IDs 0, 1 and `0x2B999D2B` and nothing between.

### 1.2 Construction and destruction

| Function | Address | Behaviour |
|---|---|---|
| `cGZCommandParameterSet()` | `0x0092A5B0` | count 0, empty map, `fallback` Empty |
| `~cGZCommandParameterSet()` | `0x0092A4FA` | destroys `fallback`, then the map with every value |
| deleting destructor (slot 15) | `0x0092A5D1` | the destructor, then frees the object to the global heap when flag bit 0 is set |

Every construction site allocates 0x28 bytes on the global heap. `AddRef` (slot 1, `0x00968B60`, shared) increments
and returns the count. `Release` (slot 2, `0x008D29B9`, a body shared with other classes): with the count above 1 it
decrements it and returns the new count; otherwise it calls slot 15 with 1 and returns 0 (the count is not
decremented first).

### 1.3 Slots (vtable `0x00AD16C8`)

"Copy" below means the value is copied by type with `cRZVariant`'s assignment from a `cIGZVariant`: scalars and
arrays (strings included) are duplicated, an object value (`IGZUnknown`, type `0x10`) is shared with one more
reference, an unknown type becomes Empty.

| Slot | Method | Address | Pops | Return | Behaviour |
|---|---|---|---|---|---|
| 0 | `QueryInterface(iid, out)` | `0x00929676` | 8 | AL | IIDs `0x4B8C39E4`, `0x4B8C39E5` and `1`: `out := this`, AddRef, true; else false, `out` unchanged |
| 1 | `AddRef` | `0x00968B60` (shared) | 0 | EAX | §1.2 |
| 2 | `Release` | `0x008D29B9` (shared) | 0 | EAX | §1.2 |
| 3 | `GetParameterCount()` | `0x009CEDE1` (shared getter of `+0x0C`) | 0 | EAX | the number of parameters, the status parameter included |
| 4 | `SetParameterCount(n)` | `0x0092A112` | 4 | none | while count < `n`: `AppendParameter(Empty)`; then while count > `n`: remove the parameter with the **highest** ID (§1.4) |
| 5 | `operator[](id)` | `0x0092A162` | 4 | EAX ptr | the value stored under `id`; for a missing ID the address of `fallback` (never null). Nothing is inserted |
| 6 | `GetParameter(id)` | `0x0092A186` | 4 | EAX ptr | the value stored under `id`, or **null** |
| 7 | `SetParameter(id, const cIGZVariant& v)` | `0x0092ABEB` | 8 | (none) | find or insert `id` (an inserted value starts Empty), then copy `v` into it. EAX is left holding the stored value's address; callers must not rely on it |
| 8 | `AppendParameter(const cIGZVariant& v)` | `0x00929E19` | 4 | (none) | `SetParameter(k, v)` with `k` = 0 for an empty set, else **(the highest ID) + 1**, wrapping to 0 after `0xFFFFFFFF` (§1.4) |
| 9 | `RemoveParameter(id)` | `0x0092A1A5` | 4 | none | removes `id`; a missing ID does nothing |
| 10 | `SetStatusParameterValue(int32 status)` | `0x009296A5` | 4 | none | `SetParameter(0x2B999D2B, Sint32 variant status)` (§1.5) |
| 11 | `CompareParameterString(id, const char* text, bool caseSensitive)` | `0x0092A5ED` | 12 | AL | §1.4 |
| 12 | `ValidateParameter(id, u32 kind)` | `0x009296D5` | 8 | AL | true only when `GetParameter(id)` is not null **and** `kind = 0xFF`; any other `kind` gives false *[quirk]* (GC2) |
| 13 | `GetFirstParameterID()` | `0x00929E40` | 0 | EAX | the lowest ID, or `0xFFFFFFFF` for an empty set |
| 14 | `GetNextParameterID(prev)` | `0x0092A1C9` | 4 | EAX | the next higher ID **after `prev`, which must be present**: a missing `prev` gives `0xFFFFFFFF` even when higher IDs exist *[quirk]*; after the highest ID `0xFFFFFFFF` |
| 15 | deleting destructor | `0x0092A5D1` | 4 | EAX this | §1.2 |

Slots 7 and 8 take the variant by reference through its interface; any `cIGZVariant` may be passed. The SDK's
declaration has the same 15 interface slots in this order.

### 1.4 Edge cases *[compat]*

- **Iteration** is in ascending unsigned ID order: `id := GetFirstParameterID()`, then `GetNextParameterID(id)`
  until `0xFFFFFFFF`. A parameter whose ID is `0xFFFFFFFF` is indistinguishable from the end, so such a loop stops
  before (or at) it *[quirk]*.
- **Numbering:** `AppendParameter` numbers after the highest ID present, not after the count: on `{0, 5}` it uses 6;
  after `SetStatusParameterValue` it uses `0x2B999D2C` *[quirk]*. When the highest ID is `0xFFFFFFFF` the new ID
  is 0 and an existing parameter 0 is **overwritten**, so the count does not grow; `SetParameterCount(n)` with `n`
  above the count would then never end in the original *[quirk]*. *[choice]* FreeCity4 stops growing when an append
  does not raise the count.
- **Shrinking** removes the highest IDs first, so the status parameter (`0x2B999D2B`) goes before ordinary low IDs.
  `SetParameterCount(0)` is the way to clear a set; there is no other clear or copy method (callers copy parameter by
  parameter).
- **`fallback`** is one variant per set, shared by every missing-ID `operator[]` call and **writable** through the
  returned pointer: a caller that stores into it changes what later missing-ID lookups return; clearing the set does
  not reset it *[quirk]*. Read-only callers (flora.md §12) see Empty.
- **`CompareParameterString(id, text, cs)`**:
  - `id = 0xFFFFFFFE` (−2): true when the comparison is true for **any** parameter (each one through slot 11, in ID
    order), else false.
  - otherwise: false when `id` is missing. The value is read with `GetValString` (data-formats.md §5.1); when that
    returns false (any value that is not a string or a string object, e.g. Sint32 5 against `"5"`) the result is
    false. Else true exactly when the string equals `text` in full, compared byte for byte when `cs` is true, or after
    `cRZString::MakeLower` of both when `cs` is false (graphics-formats.md §3.11). The empty text equals only the
    empty string.

### 1.5 The status parameter *[compat]*

ID `0x2B999D2B` (the SDK's `kStatusParameterID`) holds a command's result in its **output** set. The dispatcher
returns it (§4.4); a handler reports failure with `SetStatusParameterValue(2)` (for example
`cGZMiscCommandSystemSupport::DoMessage`, `0x005C8C10`) *[inferred meaning: 0 success, 2 failure; GC4]*. It is an
ordinary parameter otherwise: it counts in `GetParameterCount`, is visited by iteration, and is written by
`ConvertCommandOutputToString` (§3.6). The Lua call closures skip it when they return outputs (lua-host.md §9.4).

## 2. Splitting a command line: `RZParseDelimitedText` *[compat]*

`bool RZParseDelimitedText(const char* text, u32 length, char delim, cRZString* words, u32* count)` (`0x00910BD3`,
cdecl, the caller pops 20 bytes, AL). On entry `*count` is the capacity of `words`; on return it is the number of
words written. `length` bytes are read; a NUL inside them is an ordinary character. **Blanks** are exactly space
(`0x20`) and tab (`0x09`); CR, LF and every other byte are ordinary characters.

### 2.1 Rules with `delim = ' '` (the command generator and the command server)

1. Leading blanks are skipped. If nothing else remains: `*count := 0`, return true.
2. Scan the rest left to right, counting the `"` characters seen since the current word began. A blank is a
   **break** only when that count is even; a blank after an odd number of quotes is part of the word.
3. At a break the word so far is stored, the quote count restarts, and the blanks that follow are skipped, so a run
   of blanks is one break.
4. The text after the last break is the last word; trailing blanks are breaks, so they add nothing.
5. A stored word that both **starts and ends** with `"` loses its last character and then, if it is not empty, its
   first. No other quote is removed and there is no escape character: `a"b c"d` stays one word `a"b c"d`; `""` and a
   lone `"` give one empty word; an unbalanced `"` swallows the rest of the line, trailing blanks included (§2.2).
6. **Capacity:** when the text has more words than `*count`, the function returns **false** with `*count` = the
   capacity and the first that many words written. Otherwise it returns true, also when the words exactly fill it.

Examples (the words, separated by `|`; `⇥` is a tab, `∅` an empty word):

| Text | Words |
|---|---|
| empty, or only blanks | none (`*count = 0`, true) |
| `  SetDebugLevel   5  ` | `SetDebugLevel` \| `5` |
| `a⇥b` | `a` \| `b` |
| `say "hello world"` | `say` \| `hello world` |
| `"List Commands" x` | `List Commands` \| `x` |
| `x "" y` | `x` \| ∅ \| `y` |
| `a "b⇥c" d` | `a` \| `b⇥c` \| `d` |
| `x "a b" "c"d` | `x` \| `a b` \| `"c"d` |
| `"a"b"` | `a"b` |
| `a "b` | `a` \| `"b` |
| `"abc def` | `"abc def` |

These are the rules of `cRZCmdLine`'s splitter (command-line.md §2.1) except that a newline is an ordinary
character here.

### 2.2 Other delimiters (other callers, e.g. network-rules.md §8.1)

With `delim ≠ ' '` only `delim` breaks a word (again only after an even number of quotes), blanks are ordinary
inside a word, and a run of delimiters gives empty words. After a break the following blanks are skipped (a leading
trim). A word ended by a delimiter loses its trailing blanks before the quote rule is applied; the **last** word
keeps its trailing blanks and is only quote-stripped as is *[quirk]*: the right trim of the whole text at the start
of the function tests the first non-blank character instead of the last one, so it never removes anything.
Examples with `,`: `a, b ,,c  ` gives `a` | `b` | ∅ | `c  `; ` x , "y" ` gives `x` | `"y" ` (not stripped, it ends
with a blank).

## 3. `cGZStringCommandGenerator` *[compat]*

### 3.1 State (0x1C bytes, global heap)

| Offset | Field |
|---|---|
| `+0x00` | vtable `0x00AD1708` (12 slots, §3.3) |
| `+0x04` | reference count (starts at 0) |
| `+0x08` | the command server (counted reference; null until `Init`) |
| `+0x0C` … `+0x17` | `words`: a vector of `cRZString` (begin, end, end of storage), reused by every conversion |
| `+0x18` | a `cGZCommandParameterSet` created by the constructor (counted reference) and **never used** *[quirk]* |

### 3.2 Construction, `Init` and `Shutdown`

- Constructor `0x0092A6C4`: count 0, no server, empty `words`, `+0x18` := a new parameter set.
- `Init` (slot 3, `0x0092994D`): if no server is held, take the framework system service `0xEB903A32` with interface
  `0xCB90253A` (`cIGZCommandServer`; `0x00562DA0`); if the framework has none, `+0x08` stays null. Returns true.
- `Shutdown` (slot 4, `0x00929979`): releases the server and the `+0x18` set; returns true. `Init` does not recreate
  the set.
- Destructor `0x0092A514`: releases `+0x18`, destroys `words`, releases the server (no `Shutdown`). Deleting
  destructor (slot 7) `0x0092A704`: the destructor, then a global-heap free when flag bit 0 is set.
- `Release` (slot 2, `0x00929724`): as §1.2, but deletes through slot 7. `AddRef` is the shared `0x00968B60`.

Without a server every conversion fails (§3.4–§3.7).

### 3.3 Slots (vtable `0x00AD1708`)

| Slot | Method | Address | Pops | Return | Behaviour |
|---|---|---|---|---|---|
| 0 | `QueryInterface(iid, out)` | `0x009296F5` | 8 | AL | IIDs `0x0B90251F` (`cIGZCommandGenerator`), `0x0B90251E` and `1`: `out := this`, AddRef, true; else false |
| 1 | `AddRef` | `0x00968B60` (shared) | 0 | EAX | |
| 2 | `Release` | `0x00929724` | 0 | EAX | §3.2 |
| 3 | `Init()` | `0x0092994D` | 0 | AL = 1 | §3.2 |
| 4 | `Shutdown()` | `0x00929979` | 0 | AL = 1 | §3.2 |
| 5 | `CommandRegistered(id, const cIGZString& name, const cIGZString& info)` | `0x0092973B` (shared) | 12 | AL = 1 | ignored: the generator keeps no copy of the table and asks the server each time |
| 6 | `CommandUnregistered(id, name, info)` | `0x0092973B` (shared) | 12 | AL = 1 | ignored |
| 7 | deleting destructor | `0x0092A704` | 4 | EAX this | §3.2 |
| 8 | `ExecuteCommand(const char* text, u32 length, cIGZCommandParameterSet* out)` | `0x00929994` | 12 | EAX int32 | §3.7 |
| 9 | `ConvertStringToCommand(const char* text, u32 length, u32& id, cIGZCommandParameterSet*& params)` | `0x0092B139` | 16 | AL | §3.4 |
| 10 | `ConvertCommandToString(u32 id, cIGZCommandParameterSet* params, cIGZString& out)` | `0x0092A720` | 12 | AL | §3.6 |
| 11 | `ConvertCommandOutputToString(cIGZCommandParameterSet* params, cIGZString& out)` | `0x0092A810` | 8 | AL = 1 | §3.6 |

Slots 0–6 are `cIGZCommandGenerator` in SDK order; slots 8–11 are the methods the command server forwards (§5).

### 3.4 `ConvertStringToCommand` (slot 9)

1. No server: return false (nothing written).
2. `length = 0xFFFFFFFF` means `strlen(text)`.
3. Split with §2.1 into `words`, trying capacities 1, 2, 4, …, 256 until the split returns true. If the line has
   more than **256 words**, the attempt with capacity 256 fails and the first 256 words are used; the rest is
   **dropped silently** *[quirk]*.
4. No words: return false; `id` and `params` are not written.
5. `id := server.GetCommandID(word 0)` (server slot 12, §5): the registered command whose name equals word 0 in any
   case. `id` is written in every case, **0** when the name is unknown; then return false with `params` not written.
6. `params :=` a new `cGZCommandParameterSet` holding **one reference that the caller owns**. Word `k` (k ≥ 1)
   becomes parameter `k − 1` as a **string** variant (RZChar array `0x800E`, count = the word's length, no
   terminator; an empty word gives an empty string). No number is parsed and no case is changed: a handler reads
   `5` from the string `"5"` with a converting getter (data-formats.md §5.1). Return true.

`words` belongs to the generator, so two conversions on different threads would collide *[inferred]*.

### 3.5 Quoting a word (`0x00929740`)

`ConvertStringToArgumentString(const cIGZString& src, cIGZString& dst)` (a member that ignores `this`, pops 8):
`dst := src` unless they are the same object; then, if `src` contains a space or a tab, `dst := "\"" + dst +
"\""`. Inner quotes are not escaped and an empty string is not quoted *[quirk]*.

### 3.6 Commands and outputs to text

`ConvertCommandToString(id, params, out)` (slot 10):

1. `out` is cleared (erase from 0, length `0xFFFFFFFF`) **first**, whatever happens next.
2. No server, or `server.GetCommandInfo(id, name, null)` false (an unregistered ID): return false (`out` empty).
3. `out := quote(name)` (§3.5), the name as registered (its case kept).
4. If `params` is not null, for each parameter in ID order: `s := value.GetValString()` (its text, data-formats.md
   §5.1; the boolean result is ignored, so a Sint32 7 gives `7`), then `out += " " + quote(s)`.
5. Return true.

The IDs themselves are not written, so `{0: "a", 5: "b"}` and `{0: "a", 1: "b"}` give the same text. Re-parsing the
text loses empty-string parameters (`x` + `""` gives `Name x ` with a trailing blank) and values that contain quotes
*[quirk]*.

`ConvertCommandOutputToString(params, out)` (slot 11): `out` is resized to 0, then each parameter in ID order is
appended as `quote(GetValString text)`, separated by one space (none before the first). It does **not** skip the
status parameter (§1.5) and returns true, also for an empty set (`out` empty). `params` must not be null (the
original dereferences it) *[choice]*: FreeCity4 treats null as an empty set.

### 3.7 `ExecuteCommand(text, length, out)` (slot 8)

1. No server: return **2**.
2. `ConvertStringToCommand(text, length, id, params)` through slot 9. False: release `params` if one was made (never,
   in this class) and return **2**.
3. `r := server.ExecuteCommand(id, params, out)` (server slot 20, §5, which reaches §4.4); release `params`; return
   `r` (EAX).

## 4. `cGZMessage2CommandDispatcher` and its message *[compat]*

### 4.1 State (0x1C bytes, global heap)

| Offset | Field |
|---|---|
| `+0x00` | vtable `0x00AD1644` (7 slots, §4.2) |
| `+0x04` | reference count (starts at 0) |
| `+0x08` | the message server 2 (counted reference) |
| `+0x0C` | the one command message it reuses for every command (§4.5; counted reference) |
| `+0x10` | the default input set (counted reference) |
| `+0x14` | the default output set (counted reference) |
| `+0x18` | a `cGZStringCommandGenerator` it creates and initialises but **never uses** *[quirk]* |

The constructor (`0x00929C74`) zeroes everything. Destructor `0x00929A49`: releases `+0x18`, `+0x14`, `+0x10`,
`+0x0C` and `+0x08` in that order (it does not call `Shutdown`). Deleting destructor (slot 6) `0x00929C91`.

### 4.2 Slots (vtable `0x00AD1644`)

| Slot | Method | Address | Pops | Return | Behaviour |
|---|---|---|---|---|---|
| 0 | `QueryInterface(iid, out)` | `0x009297DE` | 8 | AL | IIDs `0x0B903CC3`, `0x0B903CC4` and `1`: `out := this`, AddRef, true; else false |
| 1 | `AddRef` | `0x00968B60` (shared) | 0 | EAX | |
| 2 | `Release` | `0x009223C9` (shared with two other classes) | 0 | EAX | as §1.2, deleting through slot 6 |
| 3 | `Init()` | `0x0092A8B0` | 0 | AL = 1 | §4.3 |
| 4 | `Shutdown()` | `0x00929A90` | 0 | AL = 1 | §4.3 |
| 5 | `ExecuteCommand(u32 id, cIGZCommandParameterSet* in, cIGZCommandParameterSet* out)` | `0x00929AD5` | 12 | EAX int32 | §4.4 |
| 6 | deleting destructor | `0x00929C91` | 4 | EAX this | §4.1 |

Slots 0–5 are the SDK's `cIGZCommandDispatcher` in order.

### 4.3 `Init` and `Shutdown`

`Init` creates each member that is still null, in this order: the message server 2 (framework service `0x04FA845B`,
interface `0x652294C7`; it stays null if the framework has none), the command message (§4.5, 0x38 bytes from the
framework's `operator new` `0x009133DA`), the default input set, the default output set, and the generator, whose
`Init` it then calls. A member that exists is kept, so a second `Init` changes nothing. It always returns true.

`Shutdown` releases the message server, the message and both default sets; if it holds a generator it calls the
generator's `Shutdown` and releases it. It returns true. `Init` after `Shutdown` builds everything again.

### 4.4 `ExecuteCommand(id, in, out)` (slot 5)

1. No message server (not initialised, or no service): return **2**; nothing is sent and neither set is touched.
2. If `in` is null, the default input set is cleared (`SetParameterCount(0)`) and used; the same for `out` with the
   default output set. Sets passed by the caller are **not** cleared.
3. Prepare the message: `SetType(id)`, `SetData1(in)`, `SetData2(out)` (the same slots as `SetVoid1`/`SetVoid2`,
   foundations.md §5.1), and keep a counted reference to each set in the message (§4.5).
4. `MessageSend(message)` on the message server 2 (slot 3): synchronous delivery to every target subscribed to type
   `id`, then to the type-0 (wildcard) targets (foundations.md §5.1). Its result is ignored.
5. `s := out.GetParameter(0x2B999D2B)`. Present: return `s.GetValSint32()` (the converting getter with a 0 default,
   so a string `"7"` gives 7, data-formats.md §5.1). Absent: `out.SetStatusParameterValue(0)` and return **0**.

Consequences *[quirk]*:

- A command that no target handles (unknown ID, or a handler that does not set a status) **succeeds**: it returns 0
  and the caller's output set gains a status parameter `Sint32 0`, which also moves later `AppendParameter`
  numbering on that set past `0x2B999D2B` (§1.4).
- The message and the default sets are shared by every call. A target that executes another command while it
  handles one (on the same dispatcher) changes the message's type and data under the remaining targets of the outer
  command, and reuses (clears) the default sets *[inferred: no lock or copy exists; GC3]*.
- The message keeps its references to the last command's sets until the next command or `Shutdown`.

### 4.5 The command message (`cGZMessage2CommandDispatcherMessage`, 0x38 bytes)

| Offset | Field |
|---|---|
| `+0x00` | vtable `0x00AD1638` (3 slots: QI `0x009297B0`, AddRef `0x009297A0` (a shared forwarder), Release `0x009297A8`) |
| `+0x04` … `+0x2F` | a complete standard message (`cRZMessage2Standard`, foundations.md §5.1): vtable `0x00AD15A0`, reference count at `+0x08`, type at `+0x10`, data1 at `+0x14`, data2 at `+0x18` |
| `+0x30` | counted reference to the input set of the last command |
| `+0x34` | counted reference to the output set of the last command |

- Constructor `0x00929C2B` (only from the dispatcher's `Init`): the standard message's constructor on `+0x04`
  (count 0, type 0), both references null. Destructor `0x009299F5`: releases `+0x34` and `+0x30`, then destroys the
  standard part. Deleting destructor `0x00929C58` (reached through slot 37 of the `+0x04` vtable, the thunk
  `0x00929A41`, from the standard message's `Release`), freed with the matching framework `operator delete`.
- The `+0x04` vtable `0x00AD15A0` has 38 slots: 0–2 are thunks to the three primary slots (`0x00929A29`,
  `0x00929A31` shared, `0x00929A39`), 3–36 are the standard message's own methods (the same targets as
  `cRZMessage2Standard`'s vtable `0x00ACEC00`), 37 is the deleting destructor. Its reference count is the standard
  part's: `AddRef`/`Release` on either pointer change the same count, and `Release` at count ≤ 1 destroys the whole
  object.
- `QueryInterface` (`0x009297B0`): IIDs `0x4B99446A` and `0x0B903CC5` give the primary pointer (`+0x00`) with
  AddRef; any other IID goes to the standard message's QI on `+0x04` (so `0x65297976`, `0xA52294B4` and `1` give
  the `+0x04` pointer).
- The message server receives the `+0x04` pointer. Its `Create` (slot 3) makes a plain standard message with the
  same type, not a command message.

### 4.6 How a handler receives a command

There is no registration in the dispatcher and no lookup structure: routing is the message server's notification
list, keyed by message type = command ID. A handler is a `cIGZMessageTarget2` that subscribes to its command IDs
(`AddNotification`) and, in `DoMessage(msg)`, reads `msg.GetType()` (the ID), `GetData1()` (the input set) and
`GetData2()` (the output set), writes its results into the output set and optionally its status (§1.5). Some
handlers first ask the message for `0x4B99446A` to tell a command from an ordinary message of the same type (the 3D
view, the region screen); others read data1 and data2 directly (the command-system supports, flora).

## 5. The command server, as far as these classes depend on it

`cGZCommandServer` (vtable `0x00AD17A0`, 25 slots; framework service `0xEB903A32`, interface `0xCB90253A`) is not
specified in full here (INVENTORY-M6 F3). What §3 and §4 rely on *[compat]*:

| Server slot | Address | Behaviour |
|---|---|---|
| 3 `Init` | `0x0092A982` | if `GetCommandDispatcher()` (slot 17, `+0x30`) is null: create a dispatcher (kept at `+0x34`), `Init` it and `SetCommandDispatcher` it. Then, every time, create a string generator (kept at `+0x38`, replacing a previous one), `Init` it and `RegisterCommandGenerator` it (slot 15). Returns true |
| 5 `CreateCommandParameterSet(out**)` | `0x0092A9F8` | `*out :=` a new parameter set with one reference (the caller's); EAX = that count (1) |
| 11 `GetCommandInfo(id, cIGZString& name, cIGZString* info)` | `0x0092A1F6` | unknown ID: false, nothing written; else `name :=` the registered name, `info :=` its parameter text when `info` is not null; true |
| 12 `GetCommandID(const cIGZString& name)` | `0x00929EE0` | scans the commands in ascending ID order and returns the first ID whose registered name equals `name` **in any case** (`cRZString::CompareTo` without case, both sides `MakeLower`ed); 0 when none. `RegisterCommand` (slot 6, `0x0092B075`) does not reject a repeated name, so with two such names the lower ID wins |
| 13 `GetCommandIDFromCommandString(text, length)` | `0x0092AA4A` | null text: 0. `length = 0xFFFFFFFF` means `strlen`. Splits with §2.1 into **one** word: a line with a second word fails the split and gives **0** *[quirk]*; else `GetCommandID(word)` |
| 18 `SetCommandDispatcher(d)` | `0x00929B92` | keeps `d` (counted) at `+0x30` without `Init` or `Shutdown`; true |
| 19 `ExecuteCommand(const char* text, u32 length, set* out)` | `0x00929BBA` | the generator's slot 8 (§3.7); 2 without a generator |
| 20 `ExecuteCommand(u32 id, set* in, set* out)` | `0x00929BA8` | the dispatcher's slot 5 (§4.4); 2 without a dispatcher |
| 21 `ExecuteCommandFormatted(format, set*, …)` | `0x009298AE` | cdecl. `_vsnprintf` into a 512-byte buffer (at most 511 characters), then slot 19 with the result's length and the set; a formatting error, an empty result or one longer than 511 characters gives 2. The variable arguments are read **starting with the set argument itself**, so the first conversion of `format` consumes the set pointer *[quirk]* |
| 22, 23, 24 | `0x00929BCC`, `0x00929BDD`, `0x00929BEE` | the generator's slots 9, 10, 11 (§3.4, §3.6); false without a generator |

Slot 19 is the text overload and slot 20 the ID overload; an implementation must follow these slot numbers,
whatever order a header declares the two in.

## 6. Users

- **Command server** (`cGZCommandServer`, §5): creates the dispatcher, the generator and parameter sets.
- **Parameter sets are created by** `cSCLua::Open` (`0x00880860`, the two sets of a Lua instance, lua-host.md §9.4),
  `cGZLua5CommandGenerator::Init` (`0x008B6750`, two sets), the server's slot 5, the generator (§3.2, §3.4) and the
  dispatcher (§4.3).
- **Command handlers** (message targets that read data1/data2): `cGZMiscCommandSystemSupport::DoMessage`
  (`0x005C8B30`; its `Init` `0x005C48E0` registers about 30 names such as `ListCommands` `0xABAC4EE4`,
  `SetDebugLevel` `0x2BB40833`, `ExecuteScript` `0xABAC067C`, `ExecuteCheat` `0xABAC26B5`, and subscribes to them
  through its auto-notification list `0x00913556`), `cGZWinCommandSystemSupport::DoMessage` (`0x005C7180`),
  `cGZToolCommandSystemSupport::DoMessage` (`0x005C5380`), `cSC4View3DWin::DoMessage` (`0x007F53C0`, QI
  `0x4B99446A`, then its own command slot), `cSC4WinRegionScreen::DoMessage` (`0x007B0AF0`, then `ProcessCommand`
  `0x007AF720`), the flora simulator (`0x006B3497`, flora.md §5 and §12). `cGZMiscCommandSystemSupport` is the
  framework's console-command handler; it is a user of this system, not part of it (it also reads the timer factors,
  timers.md §1.4).
- **Other parameter-set readers**: game-data-registry call entries (civic-buildings.md §4.1, budget.md §4.4) and
  the Lua call closures (lua-host.md §9.4), which reach sets without the dispatcher.
- **`RZParseDelimitedText`** has 16 call sites; besides the generator and the server's slot 13 they include the
  network rules (network-rules.md §8.1, comma).
- The cheat-code manager (`cGZCheatCodeManager`, simulator.md §9) is a separate mechanism; `ExecuteCheat` is only
  one console command.

## 7. Quirks

1. `operator[]` returns the set's single, writable `fallback` variant for every missing ID (§1.4).
2. `AppendParameter` numbers after the highest ID, the status ID included, and wraps to 0 (§1.3, §1.4).
3. `GetNextParameterID` needs a present `prev`; ID `0xFFFFFFFF` ends iteration (§1.3, §1.4).
4. `SetParameterCount` shrinks by removing the highest IDs, the status parameter first (§1.4).
5. `ValidateParameter` is true only for the kind `0xFF` (§1.3).
6. `CompareParameterString` never matches a non-string value (§1.4).
7. Only an even number of quotes lets a blank break a word; an unbalanced quote swallows the rest (§2.1).
8. With another delimiter the last field keeps its trailing blanks (the right trim tests the wrong byte) (§2.2).
9. More than 256 words: the rest of the line is dropped (§3.4).
10. All parameters from text are strings (§3.4).
11. `ConvertStringToCommand` writes `id = 0` for an unknown name but leaves it untouched for an empty line (§3.4).
12. Command-to-text drops parameter IDs, empty strings and quote escaping (§3.5, §3.6).
13. `ConvertCommandOutputToString` includes the status parameter (§3.6).
14. The generator's `+0x18` set and the dispatcher's `+0x18` generator are never used (§3.1, §4.1).
15. An unhandled command returns 0 and adds `status = 0` to the output set (§4.4).
16. One message and two default sets are shared by all commands, also nested ones (§4.4).
17. The server's `GetCommandIDFromCommandString` gives 0 for a line with parameters; `ExecuteCommandFormatted`
    formats the set argument as the first value (§5).

## 8. Open questions

- **GC1:** (§1.3, §3.3, §4.2, §4.5) the names of the interfaces behind the second IIDs: `0x4B8C39E4` / `0x4B8C39E5`
  (parameter set), `0x0B90251E` (string generator), `0x0B903CC3` / `0x0B903CC4` (dispatcher), `0x4B99446A` /
  `0x0B903CC5` (command message). Only `0x4B99446A` is asked for in the executable (two view handlers). Do plugins
  QI for any of the others?
- **GC2:** (§1.3) what is `ValidateParameter`'s second argument? Only `0xFF` gives true, whatever the value's type.
  The function has no direct caller; whether any code calls slot 12 of a parameter set is not known.
- **GC3:** (§4.4) does any handler execute a command while it handles one (for example `ExecuteScript` or
  `ExecuteCheat` running script text that issues commands), so that the shared message and default sets are
  reused during delivery? An in-game probe that logs nested `ExecuteCommand` calls would settle it.
- **GC4:** (§1.5) which status values do the handlers use besides 2? A survey of every `SetStatusParameterValue`
  call in the handlers would give the vocabulary.

## 9. Conformance tests

Variants are written `S"…"` (string, RZChar array), `I32 n` (Sint32), `U32 n`, `E` (Empty).

1. **Empty set:** `GetParameterCount() = 0`, `GetFirstParameterID() = 0xFFFFFFFF`, `GetParameter(0) = null`,
   `operator[](0)` is non-null and Empty, and the count is still 0 afterwards.
2. **Append numbering:** three `AppendParameter(S"a")` give IDs 0, 1, 2; then `SetParameter(10, U32 7)` and one
   more append gives ID 11; `SetStatusParameterValue(5)` stores `I32 5` under `0x2B999D2B` and the next append uses
   `0x2B999D2C`; `GetParameterCount() = 7`.
3. **Count:** on IDs `{0, 1, 2, 10}`, `SetParameterCount(2)` leaves `{0, 1}`; on `{0, 5}`, `SetParameterCount(4)`
   gives `{0, 5, 6, 7}` with 6 and 7 Empty; on `{0, 0x2B999D2B}`, `SetParameterCount(1)` leaves `{0}`.
4. **Iteration:** on `{0, 5}`: first 0, next(0) = 5, next(5) = `0xFFFFFFFF`, next(3) = `0xFFFFFFFF`.
5. **Fallback:** on an empty set, `operator[](99).SetValSint32(4)`; then `operator[](98)` reads `I32 4`;
   `SetParameterCount(0)` does not change that; `GetParameter(98)` is null.
6. **Remove and copy:** `RemoveParameter(42)` on `{0}` changes nothing; `SetParameter(1, v)` with `v = S"xy"` stores a
   copy: changing `v` afterwards leaves parameter 1 `S"xy"`.
7. **Compare:** with `{0: S"ABC", 1: I32 5, 2: S""}`: `CompareParameterString(0, "abc", false)` true, `(0, "abc",
   true)` false, `(0, "AB", false)` false, `(1, "5", false)` false, `(2, "", true)` true, `(7, "x", false)` false,
   `(0xFFFFFFFE, "abc", false)` true, `(0xFFFFFFFE, "zzz", false)` false.
8. **Validate:** with `{0: E}`: `ValidateParameter(0, 0xFF)` true, `(0, 7)` false, `(1, 0xFF)` false.
9. **Split (§2.1):** every row of the example table in §2.1; with capacity 1, `a b` returns false with one word `a`
   and `a  ` returns true with one word `a`; with `,`, `a, b ,,c  ` gives `a`, `b`, empty, `c  `.
10. **Text to command:** with a scripted server knowing `0x2BB40833 "SetDebugLevel"`, `0x10 "dup"`, `0x20 "DUP"` and
    `0x30 "My Cmd"`: `ConvertStringToCommand("  setdebuglevel   5  ", −1, id, p)` is true with `id = 0x2BB40833`
    and `p = {0: S"5"}` holding one reference; `"Dup"` gives `0x10` and an empty set; `"\"my cmd\" a \"b c\""` gives
    `0x30` and `{0: S"a", 1: S"b c"}`; `"Nope 1"` is false with `id = 0` and `p` untouched; `""` and `"   "` are
    false with `id` untouched; 300 words `SetDebugLevel 1 2 … 299` give 255 parameters (`S"1"` … `S"255"`); with no
    server every call is false.
11. **Command to text:** `ConvertCommandToString(0x2BB40833, {0: S"a b", 1: I32 7, 2: S""}, out)` is true with `out =
    SetDebugLevel "a b" 7 ` (a trailing blank); with null `params` `out = SetDebugLevel`; `0x30` gives `"My Cmd"`;
    an unregistered ID is false and `out` is empty even if it held text before.
12. **Output to text:** `ConvertCommandOutputToString({0: S"x y", 1: U32 3, 0x2B999D2B: I32 0}, out)` gives
    `"x y" 3 0`; an empty set gives the empty string; both return true.
13. **Dispatch:** with a scripted message server and one target subscribed to `0x1234`: `ExecuteCommand(0x1234,
    null, null)` sends one message of type `0x1234` whose data1 and data2 are the (emptied) default sets, and
    returns 0, the default output set now holding `0x2B999D2B: I32 0`; if the target calls
    `out.SetStatusParameterValue(2)` the result is 2; if it stores `S"7"` there the result is 7; with caller sets
    `in = {0: S"a"}`, `out = {}` the target sees exactly those objects and `out` ends with the status; an ID nobody
    subscribes to returns 0; a wildcard (type 0) target also receives every command; two calls send the same message
    object; before `Init` the result is 2 and nothing is sent.
14. **End to end:** server `ExecuteCommand("SetDebugLevel 5", −1, out)` delivers type `0x2BB40833` with data1 =
    `{0: S"5"}` and returns the handler's status; `"Nope"` returns 2 without sending anything;
    `GetCommandIDFromCommandString("SetDebugLevel", −1) = 0x2BB40833` and `("SetDebugLevel 5", −1) = 0`.
15. **Message identity:** the delivered message answers QI `0x4B99446A` (the command message) and `0x65297976`
    (standard); `Create` on it gives a standard message of the same type that does not answer `0x4B99446A`.
16. **Reference counts:** a new set, generator or dispatcher has count 0; `AddRef` gives 1; `Release` then destroys
    it and returns 0. The set returned by `ConvertStringToCommand` and by server slot 5 has count 1.
