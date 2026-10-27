# Spec: command line (`cRZCmdLine`)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`). Implementers must work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled
or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[compat-limit]* depends on something FreeCity4 cannot reproduce, so only the rule is matched.

Related specs: [clock24.md](clock24.md) (§4.8, CK11: the splitting rules as the cheats see them, which §2.1 restates
as the normative home), [flora.md](flora.md) (§5.2, a cheat parsed with this class),
[graphics-formats.md](graphics-formats.md) (§3.11, `cRZString::MakeLower`), [foundations.md](foundations.md) (§3.7,
start-up).

## 0. Summary

`cRZCmdLine` is the framework's `cIGZCmdLine` object: a text and the list of arguments split from it. The game
builds one from the process command line at start-up and reads its switches (`-name`, `-name:value`, `-c`, `-cvalue`),
and the cheat handlers build one from each cheat line to read its words. It is reference counted and never saved.

## 1. Object *[compat]*

### 1.1 State (0x3C bytes)

| Offset | Field |
|---|---|
| `+0x00` | vtable `0x00A82A20` (17 slots, §3) |
| `+0x04` | reference count (starts at 0) |
| `+0x08` … `+0x13` | `args`: a vector of `cRZString` (0x14 bytes each; begin, end, end of storage) |
| `+0x14` | `text` (`cRZString`): the command-line text |
| `+0x28` | `empty` (`cRZString`): always empty; returned for an out-of-range index |

### 1.2 Construction and destruction

| Function | Address | Behaviour |
|---|---|---|
| `cRZCmdLine(char const* s)` | `0x00914E14` | `text := s` verbatim; `args := split(text)` (§2.1) |
| `cRZCmdLine(cIGZCmdLine const& o)` | `0x00914A30` | `o.GetCommandLine(text)` (slot 5), then for `i` in `0 … o.argc() − 1` append `o.argv(i)` (slots 3 and 4). The text is **not** split again |
| `~cRZCmdLine()` | `0x00419490` | destroys `empty`, `text` and every argument, frees the vector |
| deleting destructor (slot 15) | `0x00419500` | the destructor, then frees the object to the global heap when flag bit 0 is set |

`Release` (slot 2, a body shared with other classes, `0x008D29B9`): with the count above 1 it decrements it and
returns the new count; otherwise it calls slot 15 with 1 and returns 0. `AddRef` (slot 1, `0x00968B60`, shared)
increments and returns the count.

## 2. Splitting and joining *[compat]*

### 2.1 Split (`0x00914C70`, used by the `char const*` constructor)

1. The separators are exactly **space, tab and newline**; `\r` and every other character are ordinary *[quirk]*.
2. Separators at the start and the end of the text are removed. An empty result gives no arguments.
3. The text is scanned left to right counting the `"` characters seen since the current argument began. A separator
   ends the argument only when that count is **even**; the count then restarts at 0 and any separators that follow
   are skipped (a run of separators is one break). A separator after an odd number of quotes is part of the argument.
4. The text after the last break is the last argument.
5. An argument that both **starts and ends** with `"` loses its last character and then, if it is not empty, its
   first. No other quote is removed: `a"b c"d` stays `a"b c"d`, `"x y"` gives `x y`, `""` gives an empty argument, a
   lone `"` gives an empty argument, and `"abc` keeps its quote.

With the process command line on Windows NT the first argument is the program path (`GetCommandLineA`, §5).

### 2.2 Join (`0x00914174`, used after `InsertArgument` and `EraseArgument`)

`text` is cleared, then for each argument in order: if it contains a space, tab or newline it is written between two
`"`, otherwise as it is; arguments are separated by one space. Quotes inside an argument are not escaped, and an
empty argument writes nothing (two separators in a row) *[quirk]*. Both helpers are members that ignore `this`
(two stack arguments, the source and the destination).

## 3. Slots (vtable `0x00A82A20`) *[compat]*

`index` arguments are signed 32-bit; `n := argc`. Strings are compared after `cRZString::MakeLower` where the table
says "any case" (graphics-formats.md §3.11; ASCII text lowers with `tolower`).

| Slot | Method | Address | Pops | Behaviour |
|---|---|---|---|---|
| 0 | `QueryInterface(iid, out)` | `0x00913E16` | 8 | IIDs `1` and `0xA31E539C` (`cIGZCmdLine`): `out := this`, AddRef, true; else false |
| 1 | `AddRef` | `0x00968B60` (shared) | 0 | §1.2 |
| 2 | `Release` | `0x008D29B9` (shared) | 0 | §1.2 |
| 3 | `argc()` | `0x00913E8A` | 0 | `n` |
| 4 | `argv(index)` | `0x00913E62` | 4 | the character buffer of argument `index` when `index < n`, else that of `empty` (""). A **negative** index is not checked and reads before the list *[quirk]* |
| 5 | `GetCommandLine(cIGZString& out)` | `0x0093E536` (shared) | 4 | `out := text` (through `out`'s copy slot 14); true |
| 6 | `Find(cIGZString const& s, bool caseSensitive)` | `0x009145A5` | 8 | the first index whose argument equals `s` (any case when `caseSensitive` is false); else −1 |
| 7 | `FindSubString(cIGZString const& s, bool caseSensitive)` | `0x00914691` | 8 | the first index whose argument contains `s`; else −1. An empty `s` is found in the first non-empty argument |
| 8 | `IsSwitchPresent(cIGZString const& name, cIGZString& value, bool unquote)` | `0x009143CB` | 12 | the `-name:value` form, §4.1 |
| 9 | `IsSwitchPresent(cIGZString const& name)` | `0x00913EC2` | 4 | `GetIndexOfSwitch(name, 0) ≥ 0`, or else slot 8 with a scratch value and `unquote` true |
| 10 | `IsSwitchPresent(char c, cIGZString& value, bool unquote)` | `0x00914240` | 12 | the `-cvalue` form, §4.2 |
| 11 | `GetIndexOfSwitch(cIGZString const& name, int start)` | `0x00914001` | 8 | the bare `-name` form, §4.3 |
| 12 | `GetIndexOfSwitch(char c, int start)` | `0x00913F1A` | 8 | the bare `-c` form, §4.3 |
| 13 | `InsertArgument(cIGZString const& s, int index)` | `0x00914BAE` | 8 | `index` is clamped to `0 … n` (0 when the list is empty or `index < 0`); `s` is inserted there; join (§2.2); true |
| 14 | `EraseArgument(int index)` | `0x00914C25` | 4 | `index ≥ n` becomes `n − 1`; the argument there is removed; join; true. A **negative** index, or any index on an empty list, removes before the list (memory corruption) *[quirk]* |
| 15 | deleting destructor | `0x00419500` | 4 | §1.2 |
| 16 | `GetArgument(index)` → `cRZString const&` | `0x00913E3B` | 4 | argument `index`, or `empty`, with the same unchecked negative index as slot 4 (CL1) |

## 4. Switches *[compat]*

A **switch argument** begins with one of the three prefix characters `/`, `-` or `\` (the string at `0x00ACECC8`).
Every search goes through the arguments in order and stops at the first match. Single letters compare in both ASCII
cases (`A`–`Z` with `a`–`z`; other characters exactly).

### 4.1 `-name:value` (slot 8)

1. If `name` is empty, false.
2. An argument matches when it is a switch argument, its length is at least `len(name) + 3`, its character at
   `len(name) + 1` is `:`, and its characters `1 … len(name)` equal `name` in any case. So the value has at least one
   character: `-name:` alone does not match *[quirk]*.
3. `value :=` the argument from position `len(name) + 2` to its end, in its original case.
4. If `unquote` is true and the value's first character is `"` or `'` **and** its last character is `"` or `'` (the
   two need not be the same kind), the first character and then the last character are removed. A value that is a
   single quote character leaves nothing after the first removal, and the second removal then starts past the end:
   STLport raises `out_of_range` *[quirk]*. *[choice]* FreeCity4 gives an empty value instead (it must not crash).
5. `value` is written to the out string; true. No match: false, and `value` is not written.

### 4.2 `-cvalue` (slot 10)

1. An argument matches when it is a switch argument of at least two characters whose **second** character is `c`
   in either case. Any longer argument matches too: `-intro:off` matches `i`, with value `ntro:off` *[quirk]*.
2. For a two-character argument (`-c` alone): true, and the out string is **not written** (it keeps whatever it held)
   *[quirk]*.
3. Otherwise, if `unquote` is true, the argument has at least 4 characters, its third character is `"` and its last
   is `"`, the value is the characters between them (empty for exactly 4); only double quotes count here. Otherwise
   the value is everything after the first two characters.
4. The value is written; true. No match: false.

### 4.3 Bare switches (slots 11 and 12)

Both start at `max(start, 0)`; a start at or beyond `n` gives −1 at once. They return the first matching index, else
−1.

- **Slot 11** (`name`): a switch argument of **at least two characters** whose length is `len(name) + 1` and whose
  characters after the prefix equal `name` in any case (`-Intro` matches `intro`; `-intro:off` does not). The
  minimum-length guard precedes the name-length comparison (`0x0091405E–0x0091409D`). Consequently an empty
  name always returns −1, including on a lone `-`, `/` or `\`. Slot 9 therefore returns false for an empty name:
  its fallback to slot 8 also rejects it (`0x00913EDD–0x00913EF8`, `0x009143CB`).
- **Slot 12** (`c`): a switch argument of exactly two characters whose second is `c` in either case.

## 5. Users

- **Start-up** (`_main`, `0x0044C2B0`): on Windows NT (`GetVersion` high bit clear) the object is built from
  `GetCommandLineA()`, so argument 0 is the program path; on Windows 9x from `_main`'s third argument (the
  `WinMain` command line, without the program name *[inferred]*). It is handed to `SC4AppStart` (`0x0044C170`) and
  destroyed when that returns.
- **Copies** of the application's command line (the `cIGZCmdLine` constructor): `cSC4App::Init`,
  `cSC4App::PostFrameWorkInit`, `cSC4App::DoPreferencesChanged`, `cSC4ConfigRuleManager::SetOptionsFromConfiguration`
  and `cSC4View3DWin::ReadTunableValues` each make their own copy.
- **Cheat lines** (the `char const*` constructor, then `argc` / `argv`): the `DoMessage` handlers of the automata
  manager (3), the application (2), the 24-hour clock (2; clock24.md §4.8), the 3D view, the tutorial system, the
  MySim agent simulator, the flora simulator (flora.md §5.2), the effects manager, the city situation manager, the
  city, the budget simulator, the automata controller manager and the advisor system, and
  `cSC4Simulator::DoCheatCode` (2).
- **Lua**: `cGZLuaScriptServer::ScriptArgSetupThreadArguments` and `SCLuaHelp::LoadPackageScripts` split argument
  strings with it (lua-host.md).

## 6. Quirks

1. `\r` is not a separator (§2.1).
2. Slots 4 and 16 do not check for a negative index; slot 14 removes before the list for a negative index or an empty
   list (§3).
3. The join drops empty arguments and does not escape quotes; after an insert or erase, `GetCommandLine` returns the
   rebuilt text, not the original (§2.2).
4. `-name:` with nothing after the colon is not a `name` switch for slots 8, 9 and 11 (§4.1, §4.3).
5. Slot 8 strips mismatched quote kinds, and throws on a value that is one quote character; slot 9 passes
   `unquote` true, so `IsSwitchPresent("name")` throws on `-name:"` too (§4.1).
6. Slot 10 matches any switch argument by its second character, and leaves the value untouched for a bare `-c`
   (§4.2).
7. Slot 10 removes only double quotes, slot 8 either kind (§4.1, §4.2).

## 7. Open questions

- **CL1:** slot 16 (`0x00913E3B`, the argument as a `cRZString`) has no direct caller in the executable; is it part
  of the published `cIGZCmdLine` interface (a plugin-visible slot) or a `cRZCmdLine`-only virtual?

## 8. Conformance tests

1. **Split:** `  "C:\Games\SimCity 4.exe"  -intro:off` + tab + `-w ` gives 3 arguments: `C:\Games\SimCity 4.exe`,
   `-intro:off`, `-w`. `a"b c"d e` gives `a"b c"d` and `e`; `""` gives one empty argument; `x\ry` (a carriage return
   inside) is one argument; the empty text and `" \t\n "` give none.
2. **Join:** after `InsertArgument("a b", 99)` on `[p, -w]` the text is `p -w "a b"`; `EraseArgument(99)` then
   removes `a b`; `InsertArgument("", 0)` on `[x]` gives the text ` x` (a leading space).
3. **Value switch:** on `[p, -INTRO:Off, /size:"1 2", -q:'z"]`: `IsSwitchPresent("intro", v, true)` is true with
   `v = "Off"`; `("size", v, true)` gives `1 2`, with `unquote` false `"1 2"`; `("q", v, true)` gives `z`;
   `("intro:", v, true)` is false; on `[-intro:]` `("intro", v, true)` is false and `v` is unchanged.
4. **Bare switches:** on `[p, -Intro, /w, -width]`: `GetIndexOfSwitch("intro", 0) = 1`, `("intro", 2) = −1`,
   `('W', 0) = 2`, `('w', 3) = −1`, `("width", −5) = 3`; `IsSwitchPresent("intro") = true`.
   An empty switch name on `[-, /, \]` gives −1 through slot 11 and false through slot 9 (CL2).
5. **Letter switch with value:** with `v` preset to `keep`: on `[-w]` `IsSwitchPresent('w', v, true)` is true and
   `v = keep`; on `[-W1024]` `v = 1024`; on `[-w"x y"]` `v = x y`; on `[-w""]` `v` is empty; on `[-intro]`
   `IsSwitchPresent('i', v, false)` gives `ntro`; on `[w]` (no prefix) false.
6. **Find:** on `[p, -Intro:Off]`: `Find("-intro:off", false) = 1`, `Find("-intro:off", true) = −1`,
   `FindSubString("ntro", true) = 1`, `FindSubString("", true) = 0`.
7. **Bounds:** with 2 arguments `argv(2)` and `argv(1000)` are empty strings; `argc()` of an object built from
   `"   "` is 0.
8. **Reference count:** a new object has count 0; `AddRef` gives 1; `Release` then destroys it and returns 0.
