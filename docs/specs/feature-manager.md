# Spec: the feature manager (`cSC4FeatureManager`)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (instruction bytes read directly, from the Steam build: it has
the GOG build's `.text` bounds and all 28,766 vtable slots of `docs/abi/vtables.tsv`, plus a SteamStub `.bind`
section) and the shipped data files. Implementers must work from this spec, the public LGPL gzcom-dll SDK headers
and general knowledge only. They must **not** consult decompiled or disassembled code (see
[CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose.

Related specs: [data-formats.md](data-formats.md) (§3 the resource manager and the startup load order, §4.3 Rule N for
number text), [network-rules.md](network-rules.md) (§9.1, the INI reader's entry enumeration),
[building-development.md](building-development.md) (§1.3, the application's shutdown callbacks, `cISC4App`
slots 69 and 70), [nation.md](nation.md) and [tutorial.md](tutorial.md) (the one consumer, §6).

## 0. Summary

The feature manager answers two questions for the rest of the program: **which expansion packs are installed** and
**which optional features are switched on**. Both answers come from small INI resources in the game data, one per
expansion pack: pack *n* is "installed" exactly when its INI resource exists, and its `[Features]` section switches
feature IDs on or off.

The shipped Deluxe data holds only pack 1's INI, and its `[Features]` section is empty. So in an unmodified game
pack 1 is the only installed pack, the current pack is 1, and **every feature ID is off**. Inside the executable the
only consumer is the nation, which asks for the current pack to pick the default tutorial region (in its `Init` and
when it resets the tutorial cities, §6); no caller of the two "is it on" queries was found. The interface is public
in the SDK (`cISC4FeatureManager`, `cISC4App::GetFeatureManager`), so DLL plugins may call it, and data plugins can
add pack INIs (§5.2).

Implementation size: about 60 lines; no messages, no agents, no save record, no tuning exemplar, no random numbers.

## 1. Identity

| Item | Value |
|---|---|
| Interface | `cISC4FeatureManager` (SDK header), IID **`0x0B562673`** |
| Class | `cSC4FeatureManager`, vtable `0x00A88274` (9 slots, §7). No class ID, no GZCOM factory |
| Created by | the application, in `cSC4App::Init` (`0x0044CAA6`): global heap, **0x0C** bytes, constructor `0x0046F8B0` |
| Owner | the application object (`cSC4App` field reached as `cISC4App`-this `+0x308`) holds the one reference |
| Getter | `cISC4App` slot 32 `GetFeatureManager` (`+0x80`, `0x006EFA20`): returns the pointer **without** `AddRef` |
| Cached global | `0x00B43C98`, written once by `SL::OnAppInit1` (`0x006023FB`) from the getter |

`QueryInterface` (slot 0, `0x0046F740`): IID `1` (`cIGZUnknown`) or `0x0B562673` → writes `this` to the out pointer,
calls `AddRef` (slot 1) and returns true. Any other IID → returns false and leaves the out pointer **unwritten**
*[compat]*.

## 2. State

Per object (12 bytes):

| Offset | Field | Initial |
|---|---|---|
| `+0x00` | vtable | `0x00A88274` |
| `+0x04` | `initialized` (bool) | false |
| `+0x08` | reference count (int32) | 0 |

Process-wide (static; shared by every instance, survives the object):

| Address | Field | Initial |
|---|---|---|
| `0x00B21624` | `staticInitDone` (bool) | false |
| `0x00B21628` | `installedPacks`: bit *k* set ⇔ pack *k* + 1 installed (u32) | 0 |
| `0x00B2162C` | `enabledFeatures`: an ordered set of u32 feature IDs (STLport `std::set<uint32_t>`; header node pointer here, element count at `0x00B21630`) | empty |

The set's header node (0x14 bytes) is allocated at program start by the static constructor `0x00A71C70` from the
STL node allocator (allocator B in [docs/abi/allocators.md](../abi/allocators.md)), which also registers the static
destructor `0x00A7AE50` with the C runtime's exit list. Both are compiler-generated and need no counterpart.

## 3. Lifetime *[compat]*

**Application start** (`cSC4App::Init`, in this order):

1. `SetupResources` (`0x0044C97E`) registers every data package, including both Plugins folders (data-formats §3.1).
   If it fails, `Init` stops and no feature manager is created.
2. The feature manager is constructed (count 0) and stored in the application; the application `AddRef`s it
   (count 1).
3. `SL::OnAppInit1` caches `GetFeatureManager()` in `0x00B43C98`.
4. The application calls the feature manager's `Init` (slot 3), which reads the pack INIs (§4.1).

So every package a player installs, plugins included, is visible to the INI scan. Step 3 only stores the pointer;
nothing queries the manager before step 4.

**Application shutdown** (`cSC4App::Shutdown`, `0x0044A897`): if the pointer is not null, `Shutdown` (slot 4), then
`Release` (slot 2), then the field is cleared. The release destroys the object (the count was 1).

**Reference counting.** `AddRef` (slot 1, `0x004BDF00`, a body shared with other classes) increments `+0x08` and
returns the new count. `Release` (slot 2, `0x0046F7F0`): when the count is **above 1** (signed) it decrements and
returns the new count; otherwise (1, or 0 or negative for an object nobody `AddRef`ed) it calls `Shutdown`
(slot 4), then the deleting destructor (slot 8 with flag 1) and returns 0, **without** first decrementing.

**32-bit overflow (FM4).** These are native machine operations, not a claim that signed C++
overflow is defined. At `0x004BDF00–0x004BDF07`, AddRef loads the 32-bit count into EAX,
increments EAX, stores those bits back and returns without changing EAX again. An input count
`0x7FFFFFFF` therefore becomes and returns **EAX=`0x80000000`**. The following Release loads
those bits, compares them with 1 and takes the **signed** less-or-equal branch
(`0x0046F7F3–0x0046F7F9`). It neither decrements nor stores a replacement count on that branch.
It calls virtual Shutdown, then virtual deleting destruction with flag 1, then explicitly clears
EAX and returns **EAX=0** (`0x0046F801–0x0046F814`); the destructor's own `this` return does not
escape Release. In the native class Shutdown clears only the initialized byte; destruction
resets the vtable and frees the allocation (`0x0046F770–0x0046F77D`, `0x0046F7D0–0x0046F7EC`).
Neither operation clears the static feature state. Observing the count before freeing would
still show `0x80000000`; dereferencing the object after Release is invalid. A replacement must
use defined 32-bit wrapping operations rather than depend on source-language signed overflow.

**Deleting destructor** (slot 8, `0x0046F7D0`, pops 4): resets the vtable pointer; if flag bit 0 is set, frees the
object to the global heap; returns `this`. It does not touch the static state.

**Static teardown.** The first static scan registers `StaticShutdown` (`0x0046FA20`) as an application shutdown
callback (`cISC4App` slot 69 `+0x114`, arguments `(callback, 0)`; building-development.md §1.3 for the mechanism).
When it runs and `staticInitDone` is set, it clears `staticInitDone`, sets `installedPacks` to 0 and empties
`enabledFeatures`; it returns true. Unlike some other static cleanups it does **not** unregister itself (no slot 70
call). *[choice]* A replacement whose state lives as long as the process need not register anything: nothing reads
the manager after the application has shut down.

## 4. Behaviour

### 4.1 `Init` (slot 3, `0x0046FD50`) → true, and the static scan (`0x0046FBF0`)

`Init`: if `initialized` is clear, set it and run the static scan; return **true** in every case.

The static scan runs **once per process** (guarded by `staticInitDone`, which it sets first), whichever object asks:

```
if staticInitDone: return true
staticInitDone := true
register StaticShutdown with the application (§3)
installedPacks := 0
enabledFeatures := {}
for k in 0 .. 31:                                   # ascending: pack 1 first, pack 32 last
    key := (type 0x00000000, group 0x8A5971C5, instance 0x2B563701 + k)
    if the resource manager has a file for key:      # SC4GetResourceAsFile(key, &file, 0), 0x005F9670
        installedPacks := installedPacks | (1 << k)
        parse the file as an INI; for each entry of its section "Features", in file order: apply §4.2
        release the file
return true
```

- A pack is **installed exactly when SC4GetResourceAsFile succeeds for its key**, whatever the file contains (an empty file, a file
  with no `[Features]` section, or a file that is not an INI at all still sets the bit) *[compat]*.
- Packs need not be contiguous: a pack-3 file without a pack-2 file installs packs 1 and 3 only.
- The lookup is the ordinary segment search (data-formats §3): when several packages hold the same key, the
  highest-priority one supplies the file, and only that one is read *[compat]*. It reads the raw record
  (`cIGZPersistResourceManager` slot 37 → slot 32 `FindDBSegment(key)`, then the segment opens the record for
  reading), so the resource cache of data-formats §3 is not involved.
- The resource keys are INI text resources of the same kind as the other game INIs in group `0x8A5971C5` (terrain,
  network and sound INIs).

**The INI reader** (`cRZIniFile`, shared with every other game INI):

- Section headers are lines whose first non-blank character is `[`; the name is the text **between** the `[` and the
  first `]` (at least one character, not trimmed), compared **case-insensitively** (`[FEATURES]` matches,
  `[ Features ]` does not). If a section name repeats, the first header wins.
- Entries are read as network-rules.md §9.1 states: from the header to the next line starting with `[`; blank lines
  and lines starting with `;` (after leading blanks) are skipped; each line splits at its **first** `=`; the key is
  trimmed on the right, the value on both sides; a line whose key or value is then empty is skipped. Lines end at CR
  or LF.
- There are **no inline comments**: everything after the `=` is the value *[quirk]* (§4.2).

#### 4.1.1 Resolved-file and streaming failures (FM3) *[compat]*

The scan's decision is the **boolean result of SC4GetResourceAsFile**, not proof that reading
the file will succeed. It sets the installed bit before constructing the INI reader
(`0x0046FC95–0x0046FCC3`). It ignores EnumEntries' result: the next instructions destroy the
section-name string and INI reader, then Release the acquired non-null file pointer, advance
to the next pack and eventually return true (`0x0046FD03–0x0046FD44`). Thus ordinary Open/Read
failures do not uninstall a successfully acquired pack, abort the 32-pack scan, roll back
features, clear `staticInitDone`, or clear the object's `initialized` byte. Another Init,
even after Shutdown, does not retry the failed file unless static teardown has reset the scan.

The foreign reader determines which entries reach the feature callback:

| Phase / failure | Entries delivered to the feature owner |
|---|---|
| File is not open and the reader cannot open it | None. LoadSectionNames returns false; EnumEntries returns false. |
| Section-index pass stops on a false GetFileLine | Headers already indexed remain; the reader marks the index loaded and returns true, treating this stop like EOF. If Features has not been indexed, no entries are delivered. |
| Features was indexed, but seeking to it returns a position at or beyond reported file size, or rereading its header returns false | None. EnumEntries returns false. The seek result is used as a position, not tested as a boolean. |
| Header was reread, and the first or a later entry GetFileLine returns false | Stop gathering entries; deliver **all valid pairs already gathered**, in order, and return true, including true for an empty gathered list. No transaction rollback. |

Indexing seeks to the beginning and records each header's starting position; enumeration then
seeks back to the selected header. These are separate reads. A failure during indexing does
not imply the same failure on the later pass: provider seek/recovery behavior matters.
The section-index stop/loaded path is `0x0091DBD3–0x0091DBF1`,
`0x0091DCF3–0x0091DD12`, `0x0091DD49–0x0091DD50`; Open failure is
`0x0091DB89–0x0091DBAD`. EnumEntries' index/seek/header checks are
`0x0091D92B–0x0091D948`, `0x0091D976–0x0091D992`, `0x0091D9BD–0x0091D9CA`.

EnumEntries **gathers first, calls back afterward**: valid pairs are appended to a temporary
sequence (`0x0091DA61–0x0091DA78`); a false line result and a new section both reach its callback
loop (`0x0091DA99–0x0091DAD5`). Callbacks apply §4.2 directly to the static set and their return
values are ignored. EnumEntries returns true after this loop (`0x0091DB21`). A later pack can
therefore override the surviving prefix exactly like any other entries. No callbacks from
this pack have run while its entry lines are still being gathered.

**Read failure is not necessarily a lost line.** GetFileLine (`0x0091CA15`) asks the file's
slot 14 (`+0x38`) to Read chunks of at most **40 bytes**, with a boolean result and no actual-byte
count. A false result ends the current line operation. It returns false if no chunk succeeded;
if an earlier chunk succeeded, it returns true with the accumulated prefix, even without a
line terminator. Bytes from the failing Read are not appended. A prefix that still forms a
valid key/value pair can consequently be applied. Read success is recorded at
`0x0091CA87–0x0091CA9A`; accumulated text and that flag are returned through
`0x0091CB1D–0x0091CB41`; the 40-byte bound is `0x0091CA6E–0x0091CA78`.
Single-byte reads used to consume CR/LF also test the boolean; a failure there does not undo
the already obtained line (`0x0091CACB–0x0091CAF3`).

**Ownership and limits.** Given a valid acquired file, the INI constructor AddRefs it once
(`0x0091CF73–0x0091CF9C`); its default is to leave the file open between operations
(`0x0091CF5A`). On every normally returning scan path after successful acquisition, INI
destruction asks IsOpen, calls Close if open, and Releases its reference
(`0x0091CDE6–0x0091CE0C`), then the scan Releases the acquisition reference
(`0x0046FD26–0x0046FD30`). Close failure does not bypass these releases. There is no guarantee
that a faulting provider, thrown exception or invalid pointer reaches this normal cleanup.

SC4GetResourceAsFile itself opens the raw record, queries its serial interface, creates a
file adapter and queries its record-binding interface; its success path calls serial-record
binding without checking its return and returns the acquired file interface
(`0x005F9692–0x005F9717`). Acquisition failure is
different from subsequent Read failure: the helper's normal false path nulls the output
(`0x005F976C–0x005F977C`), so that pack's installed bit is not set. Merely locating a record
does not prove the helper succeeds. The outcomes above describe the owner/INI response to
specified file operations; they do not assert that a particular shipped record backend can
produce every failure pattern. A raw-byte provider with complete bytes cannot express these
streaming events. Nor can a generic short read be inferred from a byte prefix alone: a file
reporting Read success is assumed to have supplied the requested bytes; a provider violating
that contract has no portable short-read result specified here. No runtime failure was measured.

### 4.2 One `[Features]` entry (callback `0x0046FB10`) *[compat]*

For an entry `key = value`:

1. `id :=` the key read as number text by **Rule N** (data-formats §4.3, `RZConvertToUint32A`): `0x…` is hexadecimal;
   text that shows a letter `a`–`f` before its first non-hex character is hexadecimal; anything else is decimal.
   So `0x12345678` and `1A` (26) are hexadecimal, while `12345678` is **decimal** 12345678 (`0x00BC614E`). Text
   with no leading digit gives 0 (`xyz`), except that a leading run of hex letters is hexadecimal (`abc` = `0xABC`).
2. The value is lower-cased. If it is exactly `off` (three characters), `id` is **removed** from `enabledFeatures`
   (nothing happens if it is absent).
3. Otherwise `id` is **added** (nothing happens if it is present).

Consequences:

- Only `off` (any case) switches a feature off. **Every other non-empty value switches it on**, including `0`,
  `no`, `false` and `disabled` *[quirk]*. An inline comment makes `off` a different string: `0x10 = off ; old`
  switches `0x10` **on** *[quirk]*.
- Entries apply in order: pack by pack ascending, and within a file in line order (a repeated key is applied each
  time). The last mention of an ID wins, so a later pack can switch off what an earlier pack switched on, and the
  reverse.
- An ID that no file mentions is off.

### 4.3 `Shutdown` (slot 4, `0x0046F770`) → true

Clears `initialized` and returns true. The static state is **kept**: after `Shutdown`, `IsFeatureEnabled`,
`IsExpansionPackInstalled` and `GetCurrentExpansionPack` still answer from the scan, and a later `Init` does not scan
again (`staticInitDone` is still set) *[compat]*.

### 4.4 `IsFeatureEnabled(id)` (slot 5, `0x0046FA80`, pops 4)

Returns 1 when `id` is in `enabledFeatures`, else 0 (EAX holds 0 or 1). It does not require `Init`: before the first
scan, and after `StaticShutdown`, every ID is off.

### 4.5 `IsExpansionPackInstalled(n)` (slot 6, `0x0046F780`, pops 4)

Returns 1 when bit `((n − 1) mod 32)` of `installedPacks` is set, else 0 (EAX holds 0 or 1). The shift count is the
low five bits of `n − 1` *[quirk]*: `n = 0` tests pack 32's bit, and `n = 33` tests pack 1's. *[compat]* for
`n` in 1–32; a replacement should reproduce the wrap for other `n`, since a plugin may pass any value.

### 4.6 `GetCurrentExpansionPack()` (slot 7, `0x0046F7A0`)

Returns the **highest** `n` from 32 down to 1 for which `IsExpansionPackInstalled(n)` is true, or **0** when none is.
The test is a virtual call through slot 6 on the same object, so an object that overrides slot 6 changes this
answer too *[compat]*.

## 5. Shipped data and plugins

### 5.1 The shipped game *[compat]*

Measured in the Deluxe data files (`SimCity_1.dat` … `_5.dat`, `EP1.dat`, `Sound.dat`, `Intro.dat`,
`SimCityLocale.DAT`): the only key of the form `(0, 0x8A5971C5, 0x2B563701 + k)` is **pack 1's**
(`0x2B563701`, in `SimCity_1.dat`, 263 bytes). It is a comment block that explains the `0x… = on` / `0x… = off`
syntax and that features are off by default, followed by an empty `[Features]` section. `EP1.dat` (the Rush Hour
content) holds none of these keys.

So with no plugins: `installedPacks = 0x00000001`; `IsExpansionPackInstalled(1)` is true and 2–32 are false;
`GetCurrentExpansionPack() = 1`; `IsFeatureEnabled(id)` is false for every `id`.

### 5.2 Plugins *[compat]*

Because the scan uses the ordinary resource lookup after the Plugins folders are registered (§3), a plugin package
can:

- **add a pack**: a resource `(0, 0x8A5971C5, 0x2B563701 + k)` makes pack `k + 1` installed and, if it is the highest,
  the current pack (which changes the nation's default tutorial region, §6);
- **switch features**: its `[Features]` entries apply as in §4.2;
- **replace pack 1's INI**: a plugin copy of `0x2B563701` hides the shipped one (only the winning file is read).

A FreeCity4 replacement must therefore run its scan after every package, plugins included, is registered.

## 6. Who uses it

- `SL::OnAppInit1` (`0x006023F5`) stores the pointer in `0x00B43C98` (§1).
- `cSC4Nation::Init` (`0x004987F3`) calls `GetCurrentExpansionPack()` and passes the result to the tutorial
  registry's `GetTutorialRegionDefaultDirName` (`0x004B6F60`), which picks a directory name by pack number from the
  `default_regions` table of the shipped Lua script `tutorial_registry` (`SimCity_1.dat`, type `0xCA63E2A3`, group
  `0x4A5E8EF6`, instance `0xFF7D91E7`). The script registers packs 0 and 1, both as `timbuktu`; the registry's
  compiled fallback for pack 0 is `maxisland`. `add_default_region(n, dir)` writes `default_regions[n]`, and the
  registry stores each entry under its numeric key, so pack 1 maps to `timbuktu` through key 1 and the script's key 0
  overwrites `maxisland`. The shipped game therefore uses `timbuktu` for every pack (an unknown pack falls back to
  key 0; tutorial-registry.md §3.2, §4.2).
- `cSC4Nation::ResetTutorialCities` (`0x00498D82`) asks the application for the manager itself (slot 32) and makes
  the same `GetCurrentExpansionPack()` → `GetTutorialRegionDefaultDirName` call.
- **Slot 6 is called internally by slot 7** (`0x0046F7B0–0x0046F7BD`), testing packs 32 downwards until a
  pack is present. Therefore both nation call chains above reach `IsExpansionPackInstalled` through
  `GetCurrentExpansionPack` *[compat]*. The call is virtual on the same manager; it is not a direct call to
  `0x0046F780`. No external caller of slot 5 or independent external caller of slot 6 was identified
  *[inferred]*. This narrower negative result comes from two scans of the whole code section: (a) every read of
  the cached global `0x00B43C98` (the nation's `Init` only), and every `cISC4App` slot-32 call on a register loaded
  from the application global `0x00B43C94` (the cache store and `ResetTutorialCities`); (b) every slot-32 call
  (`+0x80`, any receiver) whose result is used within the next few instructions for a slot 5, 6 or 7 call (only
  `ResetTutorialCities`; three other hits pass an argument to slot 32, so they are other interfaces). A result kept in
  memory and used later would escape both.

## 7. Interface slot table *[compat]*

`cISC4FeatureManager`, vtable `0x00A88274`. The SDK header declares slots 0–7 with the same order and argument sizes;
slot 8 is the class's own deleting destructor, reached only from `Release`.

| Slot | Offset | Method | Address | Pops | Return |
|---|---|---|---|---|---|
| 0 | `+0x00` | `QueryInterface(iid, void**)` | `0x0046F740` | 8 | AL |
| 1 | `+0x04` | `AddRef` | `0x004BDF00` (shared body) | 0 | EAX, new count |
| 2 | `+0x08` | `Release` | `0x0046F7F0` | 0 | EAX, new count or 0 |
| 3 | `+0x0C` | `Init` | `0x0046FD50` | 0 | AL, always true |
| 4 | `+0x10` | `Shutdown` | `0x0046F770` | 0 | AL, always true |
| 5 | `+0x14` | `IsFeatureEnabled(uint32 id)` | `0x0046FA80` | 4 | EAX, 0 or 1 |
| 6 | `+0x18` | `IsExpansionPackInstalled(uint32 n)` | `0x0046F780` | 4 | EAX, 0 or 1 |
| 7 | `+0x1C` | `GetCurrentExpansionPack()` | `0x0046F7A0` | 0 | EAX, 0–32 |
| 8 | `+0x20` | deleting destructor `(flags)` | `0x0046F7D0` | 4 | EAX, `this` |

Helpers outside the vtable: constructor `0x0046F8B0`; static scan `0x0046FBF0`; `[Features]` entry callback
`0x0046FB10` (cdecl `(key, value, context)`, context 0 and unused, result ignored by the INI reader); static
shutdown callback `0x0046FA20`; static constructor and destructor of the feature set `0x00A71C70` / `0x00A7AE50`.

## 8. Quirks

1. Any value other than `off` switches a feature **on**, including `0`, `no` and `false` (§4.2).
2. There are no inline comments: `0x10 = off ; note` switches `0x10` on (§4.1, §4.2).
3. A key written without `0x` and without a letter `a`–`f` is decimal: `12345678 = on` enables `0x00BC614E` (§4.2).
4. A pack is installed whenever its key resolves, even if its file is empty or not an INI (§4.1).
5. `IsExpansionPackInstalled` wraps its argument modulo 32: `0` means pack 32 and `33` means pack 1 (§4.5).
6. `Shutdown` keeps the scan's results, and a second `Init` never re-reads the files (§4.3).
7. `Release` on an object whose count is 0 or 1 destroys it without decrementing (§3).
8. The static shutdown callback does not unregister itself (§3).

## 9. Open questions

FM1 remains open for independent external users of slots 5/6. The internal slot-7-to-slot-6 call on both
identified nation call chains is confirmed in §6; the query contracts remain §4.

- ~~**FM2:** the exact key the `tutorial_registry` script's `add_default_region(n, dir)` writes, and so whether pack 1
  maps to `timbuktu` through key 1 (§6). Belongs with the tutorial registry's spec.~~ **ANSWERED 2026-10-23
  (re-tutreg7):** key `n` itself; pack 1 maps to `timbuktu` through key 1 (§6, tutorial-registry.md §3.2).

## 10. Conformance tests

Each test builds a resource store with the listed INI resources under `(0, 0x8A5971C5, 0x2B563701 + k)` (pack
`k + 1`), constructs the manager, calls `Init` once, then checks the queries.

1. **Shipped data:** only pack 1's INI with an empty `[Features]` section. `GetCurrentExpansionPack() = 1`;
   `IsExpansionPackInstalled(1) = 1`, `(2) = 0`, `(32) = 0`; `IsFeatureEnabled(x) = 0` for `x` in
   `{0, 1, 0x12345678, 0xFFFFFFFF}`. `Init` returns true.
2. **No pack at all:** an empty store. `GetCurrentExpansionPack() = 0`; `IsExpansionPackInstalled(n) = 0` for every
   `n` in 0–33; every feature off.
3. **Gaps and the wrap:** packs 1 and 3 present (instances `0x2B563701`, `0x2B563703`), pack 3's file empty (0 bytes).
   `GetCurrentExpansionPack() = 3`; `IsExpansionPackInstalled(2) = 0`, `(3) = 1`, `(35) = 1`, `(33) = 1`, `(0) = 0`;
   with a pack-32 file (`0x2B563720`) added: `(0) = 1` and `GetCurrentExpansionPack() = 32`.
4. **Entry parsing:** pack 1's file is
   `[features]` / `0x10 = on` / `20 = yes` / `1A = enabled` / `0x30 = OFF` / `0x40 = 0` / `0x50 = off ; note` /
   `; 0x60 = on` / `0x70 =` / ` = on`.
   Enabled: `0x10`, `0x14` (decimal 20), `0x1A`, `0x40`, `0x50`. Off: `0x30`, `0x60` (comment), `0x70` (empty value),
   and nothing is added for the empty key. `IsFeatureEnabled(20) = 1`.
5. **Order:** pack 1 `0x99 = on`, pack 2 `0x99 = off` → `0x99` off. Swapped (pack 1 `off`, pack 2 `on`) → on. One
   file `0x99 = on` then `0x99 = off` → off; `off` then `on` → on.
6. **Sections:** pack 1's file has `[Other]` / `0x11 = on` / `[FEATURES]` / `0x22 = on` / `[Features]` / `0x33 = on`.
   Only `0x22` is on (case-insensitive name, first header wins, `[Other]` ignored). A file whose header is
   `[ Features ]` enables nothing.
7. **Shutdown keeps state:** after test 4's `Init`, call `Shutdown` (returns true): `IsFeatureEnabled(0x10) = 1`
   still; call `Init` again: the store is not read again (replace the resources between the calls and the answers do
   not change).
8. **Reference counting:** a new object, `AddRef` → 1, `AddRef` → 2, `Release` → 1, `Release` → 0 and the object is
   destroyed. `QueryInterface(0x0B562673)` and `QueryInterface(1)` give `this` and add a reference;
   `QueryInterface(0x12345678)` returns false and leaves the out pointer unchanged.

### 10.1 Independent streaming and ownership vectors (FM3)

These are source-derived discriminating vectors, **not executed tests**. Start each with fresh
static scan state. Pack 1 is readable and enables `0x10` and `0x55`; pack 2 successfully acquires
a valid referenced file F with the operations specified below; pack 3 is readable and contains
`0x20=on`. All other keys miss. File size/positions are coherent, seeks work unless stated,
and successful Reads supply all requested bytes. Failures occur at the stated phase, not at
an unspecified read-call number (line reading includes chunk reads and CR/LF lookahead).

Every case installs packs 1, 2 and 3, reports current pack 3, returns true from Init, keeps both
initialization flags set, and processes pack 3. F receives one INI AddRef and two Releases
(INI then acquisition); its Close is attempted before those Releases if IsOpen is true.

| Vector | F behavior | Required feature result / distinguishing trace |
|---|---|---|
| FM3-A | IsOpen=false; attempted Open returns false | No pack-2 callbacks; `0x10`, `0x55`, `0x20` remain on. No Close if IsOpen stays false; both references still released. |
| FM3-B | Index pass: first GetFileLine returns false before any successful chunk; bytes would contain Features if readable | No indexed Features, no entry pass/callback; same features as A. A subsequently repaired file is not retried by Shutdown/Init. |
| FM3-C | Index pass reads Features header then stops on read failure. Seeking back recovers; entry pass reads `0x10=off` to normal EOF | `0x10` off, `0x55` and `0x20` on. Partial indexing did not discard the already indexed header or prevent rereading it. |
| FM3-D | Index succeeds. Entry pass rereads header; gathers `0x10=off`, `0x30=on`; next GetFileLine fails before any successful chunk; unread suffix would turn `0x10` on | Both gathered entries delivered in that order: `0x10` off; `0x30`, `0x55`, `0x20` on. During gathering the feature set is still pack 1's set; callbacks begin only after the stop. |
| FM3-E | Index succeeds. Entry pass rereads header, then first entry Read fails with no successful chunk | Empty gathered sequence, no callbacks, EnumEntries true; feature result as A. Distinguishes empty entry pass from failed header reread, which returns false. |
| FM3-F | Index succeeds. Entry pass: one entry begins with a successful 40-byte chunk consisting of `0x55 = off` plus 30 spaces; next chunk fails. Next GetFileLine also fails. Unread suffix would have added `; comment` | Accumulated prefix is a valid `off` entry: `0x55` off, `0x10` and `0x20` on. Rejecting all unterminated lines or discarding the entire pack gives a different result. |
| FM3-G | Index succeeds, but rereading the selected header returns false before any successful chunk; Close returns false while IsOpen reports true | No callbacks, EnumEntries false; same features as A. Close failure still precedes both Releases; later packs still scanned. |

Control: if pack 2's **acquisition helper returns false with null output**, pack 2 is uninstalled,
no INI AddRef/Release occurs for it, and pack 3 still runs. For an override check, change pack 3
in D to `0x10=on`: the final `0x10` must be on. A streaming provider must preserve the phase and
ownership traces above; a complete-byte-only provider can document that it cannot inject them.

### 10.2 Independent reference-count bit vectors (FM4)

These are instruction-derived expectations for a live native-layout object with the indicated
initial count, not a claim that billions of AddRefs or any runtime fault injection were run.
Observe pre-destruction state through lifecycle logging, never by reading a freed pointer.

| Initial count | AddRef stored count / EAX | Following Release |
|---|---|---|
| `0x7FFFFFFE` | `0x7FFFFFFF` / `0x7FFFFFFF` | Decrements to `0x7FFFFFFE`, EAX=`0x7FFFFFFE`; no Shutdown or destruction. |
| `0x7FFFFFFF` | `0x80000000` / `0x80000000` | No count decrement/store; Shutdown once, deleting slot 8 once with flag 1, heap free once, EAX=0. Count at Shutdown entry is still `0x80000000`. |
| `0xFFFFFFFF` | `0x00000000` / `0x00000000` | Same destruction path, count still 0 at Shutdown entry, EAX=0. |

For both destruction cases, an initially set initialized byte is cleared, while pre-existing
installed-pack bits and enabled-feature IDs remain in static storage. The deleting destructor's
own EAX=`this` is overwritten by Release's final zero; an unsigned count comparison would
incorrectly retain the overflow case instead.
