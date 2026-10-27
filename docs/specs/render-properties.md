# Spec: render properties (`cSC4RenderProperties`)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`). Implementers must work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled
or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[compat-limit]* depends on something FreeCity4 cannot reproduce, so only the rule is matched.

Related specs: [graphics-rules.md](graphics-rules.md) (how `Graphics Rules.sgr` sets these properties),
[graphics-formats.md](graphics-formats.md) (§4.2, `kModelLODOffset`), [lighting.md](lighting.md) (§1.3, §2.1, §6),
[glyph-textures.md](glyph-textures.md) (§1.2), [INVENTORY-M6.md](INVENTORY-M6.md) (R1).

## 0. Summary

The render properties are a registry of named rendering settings in three typed tables — 38 booleans, 24 integers
and 15 floats — each entry with a fixed index, a name, a short name and a value. Code reads and writes them by
index; the Graphics Rules (and anything else holding a name) finds them by name. The table of §3 is the complete
registry with its defaults.

## 1. The object *[compat]*

### 1.1 Identity and lifetime

- A `cRZUnknown`-based object of 0x30 bytes: vtable `0x00ABB6C0` at `+0x00` (18 slots, §2), `0x00ABB6A8` at
  `+0x04` (the `cRZUnknown` interface; its slot 3, `0x007DE9F0`, is the deleting destructor through `this − 4`).
- `cSC4App::PreFrameWorkInit` (`0x00452360`) builds the application's object; `cSC4App::GetRenderProperties`
  (`0x0044B660`) returns it and `SL::OnAppInit1` keeps it in the global `0x00B43CC4` *[inferred: the same
  object]*. `cSC4View3DWin`'s constructor builds a second one for the view.
- Destruction (`0x007DE960`; deleting `0x007DEEB0`) frees the three tables.

#### Two interface views and reference ownership (RP3)

Let **P** be the complete object / render-properties interface pointer and **U = P + 4** the
`cRZUnknown` interface pointer. Both views share the 32-bit reference count at **P + 8**.
Construction starts at **0**, not 1: `0x007E3658–0x007E3668` passes U to the base constructor,
whose `0x0090DA20` clears U + 4. The constructor installs the final U and P vtables at
`0x007E366D` and `0x007E3675`. Constructing the tables does not itself acquire a reference.

The exported data references identify these actual slots (some shared bodies have unrelated
automatic names in the export; those names do not define the behavior):

| Operation | P vtable `0x00ABB6C0` | U vtable `0x00ABB6A8` | Effective receiver/body |
|---|---|---|---|
| QueryInterface, slot 0 | `0x005BCB40` | `0x005BCB50` | U / `0x009457C6` |
| AddRef, slot 1 | `0x005BE3E0` | `0x005BE420` | U / `0x00968B60` |
| Release, slot 2 | `0x005BCB30` | `0x005BCB60` | U / `0x0090D997` |
| Deleting destructor, slot 3 | not a destructor (GetBoolCount) | `0x007DE9F0` | P / `0x007DEEB0` |

The P thunks add 4 to the receiver; the U thunks subtract 4 and enter the corresponding P
thunk, giving U again. Thus QueryInterface through **either** view accepts **only integer
IID 1** and writes **U**, never P, to the caller-provided output cell. It then calls U slot 1
(one AddRef) and returns true in AL. Every other 32-bit ID returns false in AL **without
writing the output cell or changing the count** (`0x009457C6–0x009457DE`). This is a Boolean
result, not an HRESULT or a promised value in all 32 bits of EAX. A successful query requires
a writable output cell; there is no null-output check. An unsupported ID does not dereference
the output pointer. No render-properties-specific IID is accepted by these slots.

AddRef increments the shared 32-bit field and returns the new value in EAX
(`0x00968B60–0x00968B66`). It is a plain, non-atomic increment, with no saturation or overflow
check; the literal arithmetic wraps modulo 2^32. Reference ownership must avoid overflow.

Release compares the count **unsigned** with 1 (`0x0090D99A–0x0090D99D`). Above 1 it decrements
and returns the new count. At **either 1 or 0** it invokes U slot 3 with deletion flag 1 and
returns 0; it does not first store a decremented count (`0x0090D9A4–0x0090D9AB`). In particular,
Release at the constructor count of zero destroys the object; it does not underflow to
`0xFFFFFFFF` or leave the object alive. A further operation through either stale pointer is
outside the lifetime contract.

U slot 3 adjusts back to P (`0x007DE9F0`) and enters the deleting destructor. The latter always
runs the ordinary destructor, then frees the **complete object P** with `GlobalHeapFree`
**iff flag bit 0 is set**; other flag bits have no effect on that decision
(`0x007DEEB3–0x007DEEC0`). It returns P in EAX, including after freeing it; this is not a live
object reference. Flags 0 or 2 destroy the contents without freeing the outer allocation;
flags 1 or 3 also free that allocation. Ordinary destruction releases the instance float,
integer and Boolean tables, in that order, then destroys the base (`0x007DE974–0x007DE9E5`).
Direct destructor calls are lifetime termination, not reference-count operations.

### 1.2 Tables and entries

`+0x0C`, `+0x18`, `+0x24` are the boolean, integer and float tables (begin, end, capacity). An entry holds:

| Offset | Boolean entry (0x20 bytes) | Integer or float entry (0x28 bytes) |
|---|---|---|
| `+0x00` | index | index |
| `+0x04` | name (a string) | name |
| `+0x10` | short name (a string) | short name |
| `+0x1C` | value (byte) | value |
| `+0x1D`, `+0x1E` | minimum and maximum (bytes), always 0 | — |
| `+0x20`, `+0x24` | — | minimum and maximum: the range of some entries (§3), else 0 and 0 |

All three tables hold the same record template, `{index, name, short name, value, minimum, maximum}`, instantiated per
type (the export names the boolean instance `cSC4RenderProperty<bool>`) *[inferred: the minimum and maximum names
follow from that layout and from the registered ranges of §3, e.g. 1–5 for `kEffectPriorityLevel`]*. Each
registration (§1.3) stores the value, minimum and maximum together; a boolean is registered as (value, 0, 0), so its
two extra bytes are an unused range of the same record type (RP2). **Nothing reads the minimum and maximum**
(RP1) *[inferred]*: none of the 18 slots does, set neither clamps nor checks them (§2), the table copy of the
constructor only carries them along, and every direct reader of the tables found in the executable (all code that
reaches the application's object through the global `0x00B43CC4`, `0x0044B660`) uses the value offset only. They are
registration metadata; a re-implementation may keep them for documentation but must not enforce them.

So the value of boolean i is the byte at `table + 0x20 i + 0x1C`, and of integer or float i the 4 bytes at
`table + 0x28 i + 0x1C`. Other classes read the values directly at these offsets (lighting.md §2.1 reads boolean 28
and float 6, glyph-textures.md §1.2 integer 23).

### 1.3 Registration and names

The registry is filled once at static initialisation (`cSC4RenderProperties::cProperties`, `0x007E3210`): each
property's registration (`cReg<name>`, §3) appends its entry — index, name as written (`kRenderShadows`), the value
and the two extra values — to one of three static tables (`0x00B4E3A4`, `0x00B4E3B0`, `0x00B4E3BC`). The
constructor (`0x007E3650`) copies the three tables into the object and then, for every entry:

1. drops the first character of the name (the `k`);
2. sets the short name to the name's capital letters, lower-cased (`0x007DDF80`): `RenderShadows` → `rs`;
3. lower-cases the name: `rendershadows`.

#### Byte strings, NUL and locale (RP5)

Find takes a **pointer to a readable, NUL-terminated byte string**, not a counted string,
UTF-16 string or Unicode case-folding request. The first zero byte ends the query; subsequent
bytes are not part of its length or comparison. This is proved by the byte-at-a-time zero
scans at `0x007DE250–0x007DE272` (Boolean), `0x007DE390–0x007DE3A2` (integer), and
`0x007DE4E0–0x007DE4F2` (float). A leading NUL is an empty query and matches no registered
name. A null query pointer or missing readable terminator is outside the supported input
domain; no safe native failure result is promised. The caller does not need a writable query:
Find copies the bytes before folding them.

Folding is the executable CRT **tolower**, at `0x009F0543`, despite its incorrect automatic
`std::transform` name in the export. Find supplies that callback at `0x007DE282`,
`0x007DE3B2`, and `0x007DE502`. The loop at `0x00586AD8–0x00586AE2` **sign-extends each input
byte to an int**, calls the callback, and stores only its low byte. It does not decode a
multibyte character or UTF-8 sequence, expand a character into several bytes, remove accents,
trim whitespace, or strip a query-leading `k`.

The callback obtains the thread CRT locale record and synchronizes it with the global locale
record when needed (`0x009F0543–0x009F055D`), then calls `___tolower_mt` at `0x009F047B`.
For its **C-locale path**, the literal rule is: int values `0x41–0x5A` become value + `0x20`;
all others are unchanged (`0x009F052F–0x009F053C`). Consequently ASCII bytes A–Z fold to a–z,
and bytes `0x80–0xFF`, passed as negative ints, retain their original low byte on that path.
The non-C path uses the locale classification information and CRT locale/codepage mapping
(`0x009F0487–0x009F052B`, including `___crtLCMapStringA` at `0x009F050C`). There is **no fixed
owner-specific codepage** and no unconditional Unicode or ASCII-only fold across all locales.
In particular, replacing the sign extension with unsigned-byte promotion is not a proven
equivalent for high bytes in a non-C locale.

This locale dependency is live, not merely a CRT possibility: the language manager can call
`setlocale(0, language-string)` and then set the multibyte codepage from that locale
(`0x00928068–0x0092807A`). GetCurrentSystemCodePage can also change the C locale to the system
default (`0x00910025–0x00910054`; executable byte strings at `0x00AC7020` and `0x00A80BF0`
are respectively `C` and empty). Neither construction nor Find selects or freezes a locale.
Names and short names are folded **at construction time**; queries are folded **at call time**.
Short-name selection itself uses CRT isupper on sign-extended bytes, then the same tolower
(`0x007DDFB0–0x007DDFCE`; locale lookup at `0x009EF5A9–0x009EF5DF`). A locale change does not
rebuild existing entries. Thus §3 spellings and the conformance examples assume the C locale
(or a locale with identical classification/folding for these ASCII spellings) during both
construction and lookup.

The current pure-core compatibility domain is **ASCII query prefixes through the first NUL,
under that C-locale profile**, covering all 77 registered names and abbreviations. Within
this domain, ASCII folding plus first-NUL truncation is sufficient. A Rust string API that
compares bytes after an embedded NUL differs from native, even if its character folding is
otherwise correct. Non-ASCII text is not established as UTF-8 or any particular Windows
encoding by this owner. Matching a non-C locale, including ASCII I/i where locale-sensitive,
or high-byte query behavior requires additional locale-profile evidence (RP6); an ASCII-only
core must not claim those cases.

After folding, comparison requires **equal byte length and equal bytes**, with each entry's
full name tested before its short name, then the next entry. It returns the first matching
zero-based table position, not a pointer or an independently interpreted entry ID; no match
returns the 32-bit value `0xFFFFFFFF` (−1). Evidence: Boolean comparison/return at
`0x007DE2B8–0x007DE314`, integer at `0x007DE3F3–0x007DE465`, float at
`0x007DE543–0x007DE5B5`. In the C-locale profile, `RS\0ignored` finds Boolean 9,
`STCS\0ignored` finds integer 23, and `PD\0ignored` finds float 3; `rs ` and `kRenderShadows`
do not find a Boolean entry. Here `\0` denotes an actual zero byte, not backslash and digit.

### 1.4 Native text-service binding (RP6)

This is the binding contract for an adapter running **inside the supported 32-bit game
process**. It does not turn the portable ASCII profile into a complete non-C text profile.
The two foreign primitives below, together with the byte and timing rules, let an adapter
use the actual active CRT rather than reproduce undocumented locale tables.

#### Primitive ABI and provenance

| Operation | Native primitive signature | Supported executable address / RVA | Result consumed by this owner |
|---|---|---|---|
| Uppercase classification | `int32_t __cdecl isupper(int32_t character)` | `0x009EF5A9` / `0x005EF5A9` | Zero means omit the source byte from the abbreviation; any nonzero value means include it after lowercasing. This is an integer result, not a C++ bool-returning ABI. |
| Lowercase mapping | `int32_t __cdecl tolower(int32_t character)` | `0x009F0543` / `0x005F0543` | Retain exactly the low 8 result bits as one output byte, including when the full integer result is negative or wider than one byte. |

These names describe the measured primitives, **not PE export names**. Each takes one
32-bit stack argument, returns its integer in EAX, and leaves argument removal to the
caller. There is no `this`, context pointer, locale argument, out parameter, callback
registration, reference count or result allocation. Evidence: the complete wrappers at
`0x009EF5A9–0x009EF5E2` and `0x009F0543–0x009F0564`, and caller stack cleanup at
`0x007DDFB4–0x007DDFBD`, `0x007DDFC8–0x007DDFD9`, `0x00586ADB–0x00586AE2`.
The export's automatic `std::transform` label for the second primitive is wrong.

The supported PE is i386/PE32, preferred image base `0x00400000`, SHA-256
`3bc5c7fe807fe5aa24a3abb06e40f13486670bc1150612c09baf4ecedafe1a4b`.
Both targets lie in executable `.text`. Its import directory contains **no CRT DLL**;
these are statically linked executable CRT routines. Its only named exports are
`GZDllGetGZCOMDirector` and `__export__GetMemoryReport`. Neither primitive is exported.
Calling a plugin's own `tolower`/`isupper`, or resolving either name from an arbitrary
MSVCRT/UCRT DLL, does not select the game's CRT or its locale state.

The actual lowercasing backend is also more than a direct Windows byte-map call:
`___tolower_mt` (`0x009F047B`) invokes `___crtLCMapStringA` (`0x009FBEDF`) with the
CRT locale's codepage and LCID when required; the latter can use Windows wide mapping
plus byte conversions or the ANSI mapping path. The PE imports `LCMapStringA/W`,
`GetStringTypeA/W`, `MultiByteToWideChar` and `WideCharToMultiByte` from KERNEL32.
Their IAT cells include `0x00A801D4`, `0x00A801E4`, `0x00A801D0`, `0x00A801E8`,
`0x00A80108`, `0x00A802C0` respectively. These OS imports are **dependencies**, not
interchangeable replacements for the two primitives. Classification uses the CRT table
or `___isctype_mt` (`0x009F871E`); replacing both with one Unicode case-fold operation
loses the classification and byte-result rules.

#### Lookup, lifetime and unavailable binding

The public LGPL SDK declares the eighteen render-properties methods in
`external/gzcom-dll/gzcom-dll/include/cISC4RenderProperties.h`, but declares no ctype
provider, locale handle or additional render-properties text-service slot. The documented
application route is `cIGZFrameWork::Application()` to the application, successful
`QueryInterface(0x26CE01C0, void**)` to `cISC4App`, then
`cISC4App::GetRenderProperties()` to the properties view. The last method is a borrowed
pointer getter: native `0x0044B660–0x0044B666` just returns the stored pointer, with no
AddRef. Native application QueryInterface accepts that IID and acquires a reference
(`0x004492A7–0x004492C2`), independently confirming the SDK acquisition route.
Check for an unavailable application, failed interface query or null property
pointer before use. If retaining the properties pointer, AddRef the **properties view**
while its application owner is still live, before releasing the successfully acquired
application-interface reference; eventually Release that retained properties reference.
For a borrowed use, keep the application owner live throughout that use. Do not
try to obtain that view by QueryInterface ID 1: §1.1 establishes that ID 1 returns the
different U view. No new GZCOM service ID or IID is specified here.

*[choice: adapter binding protocol]* For this measured build an adapter may obtain the
already loaded main executable with Windows `GetModuleHandleA(NULL)` and bind the two
verified RVAs relative to that module base. This is a **build-specific internal binding**,
not a discovered public API. Verify the executable identity, x86 ABI and target mapping
before making either call. The supplied module handle and function pointers are borrowed
for the lifetime of the loaded executable: no `FreeLibrary`, AddRef, Release or freeing
of primitive results. An address alone is not a GZCOM object. A host-supplied bridge may
instead provide these same operations, but such a bridge is adapter infrastructure,
not a claimed game export or native registration hook.

Resolve **both** operations before constructing an owner advertised as fully native-text
compatible. If either is unavailable, the executable identity is unsupported, or the
property-service acquisition fails, report an unavailable binding through the adapter's
existing recoverable failure mechanism and do not publish a partially initialized native
owner. No new C export or particular error-enum layout is prescribed. An explicitly
selected ASCII-profile mode may remain available under §1.3; silently substituting it is
not a successful native binding. Binding failure is distinct from an ordinary Find miss
(`−1`) and from a primitive result of zero or `−1`.

Once invoked, these native primitives have **no separate recoverable error result** for
the owner to inspect. If Windows mapping fails inside `___tolower_mt`, it returns the
original integer (`0x009F0514–0x009F053C`); the owner still stores its low byte. The
multibyte classification helper returns zero when its OS classification fails
(`0x009F877E–0x009F878B`), indistinguishable to this owner from non-uppercase.
Do not reinterpret those results as adapter-unbound, throw on a negative mapping result,
or use GetLastError as an added owner success protocol. CRT thread-state acquisition can
allocate, and its failure goes to `__amsg_exit` (`0x009F7620–0x009F7668`); a blanket
promise that native calls are allocation-free or always fail recoverably is unsupported.

#### Byte and locale boundaries

For every byte passed by this owner, promote **as signed 8-bit to signed 32-bit**:
`0x49 → 73`, `0x80 → −128`, `0xD0 → −48`, `0xFF → −1`.
This is measured executable behavior, not an instruction to substitute the standard
C library's portable input-domain rules. In particular, do not convert `0xFF` to 255,
discard it as EOF, stop the byte scan on it, or decode a multibyte sequence first.
Only the original input's first NUL terminates a Find query. Primitive result truncation
is separate from input termination; it does not change the copied query's byte count.
`0x00586AD8–0x00586AE7` proves the one-input-byte/one-output-byte loop. All registered
source names are ASCII; this owner does not offer arbitrary-name registration as a
public SDK method, so an added high-byte abbreviation constructor API is not implied.

| Boundary | Required calls and retained state |
|---|---|
| Construction, each entry after dropping `k` | Visit the still-mixed-case source bytes in order. Classify each byte; lowercase and append only those classified uppercase. Then lowercase every byte of the full name. Retain the resulting full and short byte strings in that instance. The short name is not derived from the already folded full name. |
| Each Find call | Copy the input through its first NUL, excluding that NUL; lowercase every copied byte with the live primitive; compare the resulting counted bytes to the retained full/short names under §1.3. Do not classify query bytes or re-fold the stored names. |
| Name getter | Return the stored full-name bytes. Do not decode, re-fold or reconstruct a display spelling under the current locale. |
| Locale change after construction | Retained names stay unchanged; later Find byte mappings may change. A newly constructed instance may therefore have different name bytes from an older instance. |

The constructor's order is explicit at `0x007E3887–0x007E38CE`; the abbreviation's
classify-then-map order is at `0x007DDFB0–0x007DDFD1`. Boolean Find supplies the primitive
at `0x007DE282`; the integer/float equivalents are at `0x007DE3B2` and `0x007DE502`.
The SDK name getters return `char const*`; native Boolean getter
`0x007DDF20–0x007DDF30` returns the stored string pointer without allocating. Treat names
as borrowed byte strings backed by that instance, keep the owner alive while reading,
and copy bytes if retaining them beyond its lifetime. Value setters do not rebuild names.

Locale selection is live **per primitive invocation**, not a captured construction
locale object, per-Find locale snapshot, or fixed Windows ACP. Both wrappers obtain
the executable CRT's current thread record with `__getptd` (`0x009F7604`), compare its
locale record with the current global record and synchronize when required. Locale
update is internally locked (`0x009F4224–0x009F4255`); the owner does not take a lock
covering an entire name, construction or Find. This contract does not promise an atomic
profile if another thread changes locale midway. Invoke on the game's calling thread
to preserve its CRT context; forwarding all calls to a worker or caching a 256-entry
mapping at adapter startup is not established equivalent. No adapter-owned CRT locale
record layout, thread-state allocation or shutdown hook is required or specified.

The live mutators already identified in §1.3 remain relevant: language selection calls
the **same executable** `setlocale` (`0x009F48E2`) and, on nonnull return, `_setmbcp(-4)`
(`0x00928068–0x0092807F`); the system-codepage helper can replace `C` with the system
default and does not restore it (`0x00910025–0x00910078`). Neither is an adapter
initialization requirement: do not call them to force a desired profile. Successful
language-manager selection alone does not prove `setlocale` succeeded: its null-return
branch still reaches the success path (`0x00928072–0x00928098`). Exact shipped-language
profiles, OS acceptance and startup ordering remain RP6's explicit open evidence gap.
The language argument comes from the language-manager runtime-library-name method
at `0x0092440F` (vtable slot 13, called at `0x00928061`), which supplies CRT request
strings such as `english-us`, `english-uk`, `french`, `japanese` and `turkish`
(`0x00924430–0x00924499`, `0x00924541`). These literals identify requests; they do not
establish an accepted LC_CTYPE, numeric codepage or installed/shipped language inventory.

**Graphics-rules GR6 boundary.** An imported full name can contain non-UTF-8 bytes under
the active CRT profile; the SDK's `char const*` promises no UTF-8 validation. Preserve
those bytes at the properties boundary, including the distinction between stored length
and a C-string consumer's first NUL. This does not establish the separately owned dump
logger's headings, padding, line endings, `%g` spelling, write/close failures or its
handling of embedded zero bytes. Those remain the graphics-rules GR6 contract; merely
decoding names with replacement characters cannot claim byte-exact logging.

### 1.5 Configured state, the view snapshot and observation (RP7)

**There is no public ordered-write-history interface in this owner.** Its eighteen slots
provide current counts, names, values, lookup and assignment, plus reference operations;
they provide neither a change journal nor an observer registration. The application getter
in §1.4 returns the current owner, not its inputs. Graphics-rule logging is also not such
an interface: the value dump is a current-state report with formatted floats, and the option
summary is preferences/recommendations ([graphics-rules.md](graphics-rules.md) §6).
Reading these outputs into expected state would test a copy of native results, not an
independent prediction. Registered defaults predict a newly constructed object only.

#### Sources and order of live changes *[compat]*

The rules manager applies each property set in **Boolean-list order, then integer-list
order, then float-list order**, preserving repeated writes within each list
(`0x007CE1F9–0x007CE258`). There is no combined source-text assignment order across the
three types. Rule selection, option selection and preference reapplication are specified
in graphics-rules.md §§1, 5, 6 and 8; they are inputs to prediction, not a new render-owner
method. The same registered defaults can consequently lead to different live values.
Reconstructing startup requires the actual configuration/rule inputs, environment and
preferences, with subsequent option/preference changes in their actual order.

The view's misleadingly named private **SetRenderProperties** (`0x007ECDA0`) writes the
**application** owner held in `0x00B43CC4`, not its embedded snapshot. It resolves a name
Boolean first, then integer, then float; an empty Boolean value toggles the current truth
value, while nonempty Boolean/integer/float text uses the corresponding SCRes parser.
It returns false if there is no application owner or no matching property, and true for
a resolved assignment; this is not a setter validation/status result. The successful path
can post `0xC9F775BB` through the message server. That notification carries neither an
ordered list of assignments nor their values. Evidence: target and Boolean path
`0x007ECDC6–0x007ECE43`, numeric paths `0x007ECE4B–0x007ECEBE`, notification and result
`0x007ECEE7–0x007ECF32`. The static parser callback at `0x007EDE30` forwards to this helper;
neither is a public render-properties slot. Predict textual inputs by the SCRes parsing
contract; do not substitute host-language number/Boolean parsing.

#### The second object is a private scroll snapshot *[compat, quirk]*

Let **V** mean the complete `cSC4View3DWin` object, not an arbitrarily acquired secondary
interface. Its constructor constructs a separate properties object **S = V + 0x380**
(`0x007F0E32–0x007F0E70`). S initially has the ordinary defaults. It is embedded storage,
not a separately allocated service. The view's public vtable `0x00ABCE40` has no
render-properties/snapshot getter; its QueryInterface at `0x007E5630–0x007E56C0` does not
expose S. **There is no public acquisition/signature contract for S.** In particular,
neither GetRenderer nor the helper named SetRenderProperties returns it. Do not invent
a view-properties IID or reinterpret a secondary interface plus `0x380` as S.

When application Boolean **30 (`kNoPartialBackingStoreCopies`)** is true, scroll begin saves the
application's three tables in S, then writes application Booleans **9, 10, 32, 13** to
false in that order and assigns application float **9** from float **10**. The snapshot
is the state **before** these writes. Scroll end, when that same application flag is true
at the end call, restores all three application tables from S. These operations are
conditional on the flag at each call, not on a saved “begin succeeded” flag
(`0x007F6427–0x007F64CF`, `0x007F6177–0x007F61D8`). Thus changing that flag between calls
can suppress restoration, or permit restoration of an older snapshot. No balancing or
automatic recovery rule is supplied by these two bodies.

The save/restore copies the table contents rather than replaying setters. Boolean-table
copy uses `0x007F5F00`, integer/float-table copy `0x007F6010`; their call sites above address
the three table descriptors directly. S and the application consequently have independent
stored values: changing the application after a save does not update S, and restore
overwrites intervening application assignments. The paths also literally copy the 32-bit
field at P + 8 (the reference count) from source to destination
(`0x007F645C–0x007F646F`, `0x007F61B3–0x007F61C0`). This is an internal snapshot quirk,
not AddRef/Release and not a way to transfer public reference ownership.

An image-specific diagnostic bridge may identify V and borrow S while that view remains
alive, after verifying object identity and layout. This is a **private diagnostic binding**,
not SDK acquisition. Do not AddRef/Release or delete S as an independently owned heap object;
§1.1's deleting destructor can free the complete properties pointer, which here points
inside V. If the bridge cannot establish the complete V and lifetime, report the snapshot
binding unavailable. The ordinary application acquisition in §1.4 remains sufficient for
application counts/names/Find; it does not acquire S.

#### Non-mutating input-observation contract *[choice: diagnostic protocol]*

There is **no native subscription or existing DR0 entitlement** implied by this spec.
The following is a protocol for a host-supplied trace/bridge, not a claim that such a
bridge exists or that native measurements have been performed. It must observe genuine
operations without issuing additional setters, restoring native values, replacing native
results, or seeding expected values from getters/table reads. A monthly sample is not
enough to recover missing input history.

| Event/input | Required observation and replay rule |
|---|---|
| Construction/lifetime | Distinct instance identity and generation, construction before any configuration write, and destruction/end of borrowed lifetime. Initialize expected tables from §3, independently of native outputs. Never replay destruction or reference tests against the borrowed live application or S. |
| Setter entry | Record target P/generation, sequence number, thread, type, signed 32-bit index, and the argument before execution. On this x86 build ECX is P, index is the 32-bit stack argument at entry +4, and value is at entry +8: Boolean low byte, integer four bytes, float binary32 four bytes. All three bodies remove eight argument bytes and promise no status (`0x007DDE60–0x007DDEB4`). Preserve duplicate/no-change writes and inter-type order across calls. |
| Setter completion | Associate entry with the same completed operation; apply only completed writes at the comparison boundary. Preserve the native register/flags/stack and x87 environment. For ordinary finite binary32 inputs, replay the setter width in §2.1; do not route floats through decimal logs. Exceptional x87/NaN inputs require that environment, not a universal payload-preservation promise. |
| Snapshot/save and restore | Record the source/destination instance relationship and completion of each save/restore. Copy the **independent expected source** tables to the expected destination. Native copied table values must never become expected inputs. These events are necessary even when no setter is seen. Track the P+8 copy separately if claiming reference-field coverage. |
| Sampling | Compare native outputs only after all earlier completed events have been replayed and with no write/copy in progress. A late attach, missing event, ambiguous ordering, unidentified source, lost lifetime, or observation failure makes dependent comparisons unavailable/incomplete; do not silently reset expectations from native state. |

At the setter boundary these are **external inputs to that owner**, so replay can test
its assignments and getters independently; it does not verify that the rules manager,
preferences or parser selected the correct inputs. That requires predicting their inputs
and behavior under their own specs. Likewise a copy event must not be misreported as an
invocation of every setter or as evidence for unobserved setter methods.

The shared SCRes/weather observer owns its debug state: SCRes can occupy all four hardware
execution slots, and its documented DR0 loan is restricted to weather on the installing
thread outside active parser/file frames or pending forwarding
([glue-weather-boot8.md](../lanes/glue-weather-boot8.md)). Property setters can run **inside**
parser frames. That loan therefore cannot serve as continuous setter observation, and
DR1–DR3 are not spare slots to assume available. A conforming bridge for simultaneous
observation must use a separately coordinated trace facility that leaves the existing
DR0–DR3/DR6/DR7 state and exception dispatch intact, or report this observer unavailable.
Blindly clearing, replacing, temporarily borrowing or swallowing the existing breakpoints
does not satisfy the protocol. Merely restoring them later does not recover missed events.
No particular in-process hook or new debugging implementation is prescribed here.

For owner closure, public counts/names/ASCII Find probes can remain independently credited
without this bridge. Live values need a complete input/copy history; setter credit needs
observed setters (or an isolated owned-instance fixture); S needs the private lifetime
binding above. QueryInterface/refcount/destruction and independent copying need separate
owned-object tests (§7.1), never destructive calls on a borrowed live service. RP1/RP2's
metadata (§1.2) is closed; RP6's locale profiles remain a separate question. A registry-only shadow
does not become whole-owner verification by adding a dump of configured native values.

## 2. Slots (vtable `0x00ABB6C0`) *[compat]*

| Slot | VA | Behaviour |
|---|---|---|
| 0–2 | shared bodies | `QueryInterface`, `AddRef`, `Release` of the `cRZUnknown` at `+4` |
| 3, 4, 5 | `0x007DDE10`, `0x007DDE20`, `0x007DDE40` | number of boolean, integer, float entries |
| 6, 7, 8 | `0x007DDEC0`, `0x007DDEE0`, `0x007DDF00` | `GetBool(i)`, `GetInt(i)`, `GetFloat(i)` |
| 9, 10, 11 | `0x007DDE60`, `0x007DDE80`, `0x007DDEA0` | `SetBool(i, v)`, `SetInt(i, v)`, `SetFloat(i, v)` |
| 12, 13, 14 | `0x007DE1F0`, `0x007DE320`, `0x007DE470` | `FindBool(name)`, `FindInt(name)`, `FindFloat(name)`: the index or −1 |
| 15, 16, 17 | `0x007DDF20`, `0x007DDF40`, `0x007DDF60` | the name of boolean, integer, float i |

- Get and set do **not** check the index, and set writes the value without looking at the range *[quirk]*.
- **Find** lower-cases the query and compares it, entry by entry from index 0, with the entry's name and then its
  short name; the first entry that matches either wins. Comparison is exact after lower-casing (no wildcards).

### 2.1 Index domain and native unchecked access (RP4)

For slots 6–11 and 15–17, the compatibility input domain is a **live P view** and a 32-bit
index **0 ≤ i < the count of that type**. The last valid Boolean, integer and float indices
are respectively **37, 23, 14**; **38, 24, 15** are already outside their respective tables.
Counts come from (end − begin) divided by entry stride (`0x007DDE10–0x007DDE59`), not from
reserved capacity. An in-range set ignores the two extra range fields. GetBool returns a
byte in AL, GetInt a 32-bit value in EAX, GetFloat loads a stored binary32 into x87; name
getters return the entry's name pointer in EAX. Native setters provide **no success/failure
status**. Boolean set copies the argument byte (supported Boolean values are 0 and 1), integer
set copies four bytes, and float set loads/stores binary32 through x87.

| Type | Table begin field | Byte stride | Value address / width | Name-pointer field address / width |
|---|---|---|---|---|
| Boolean | P + `0x0C` | `0x20` | begin + `0x20` × i + `0x1C` / 1 | begin + `0x20` × i + 4 / 4 |
| Integer | P + `0x18` | `0x28` | begin + `0x28` × i + `0x1C` / 4 | begin + `0x28` × i + 4 / 4 |
| Float | P + `0x24` | `0x28` | begin + `0x28` × i + `0x1C` / 4 | begin + `0x28` × i + 4 / 4 |

These are **unchecked 32-bit effective-address calculations**, not checked multiplication
or table operations with error results. Boolean bodies shift the supplied index left by 5;
integer/float bodies form 5 × index and use address scale 8. Each immediately accesses the
resulting address: sets `0x007DDE60–0x007DDEB4`, value gets `0x007DDEC0–0x007DDF10`, name
gets `0x007DDF20–0x007DDF70`. None loads the table end or capacity, tests the index sign,
clamps it, or branches on validity.

**Negative indices, index equal to count, and extreme 32-bit indices have undefined native
indexing behavior for this compatibility contract.** Address arithmetic can wrap and may
reach other memory (even alias a valid entry); the result depends on allocation/layout and
is not a supported property lookup. No native return of None, null, false, zero, −1, an
exception, or even a guaranteed crash is specified for these accesses. Invalid setters may
corrupt memory. Do not run native out-of-range accesses as expected-result conformance tests.
A safe core may reject them with None/false and no mutation, as the current core reports;
that is an implementation-defined safety extension, **not an observed native outcome**.
The −1 from an unsuccessful Find must be checked before calling an indexed slot.

## 3. The registry *[compat]*

"Short" is the short name of §1.3; "Range" is the two extra values when they are not both 0.

**bool** (38)

| # | Name | Short | Default | Range | Registered at |
|---|---|---|---|---|---|
| 0 | `kForceSoftwareDriver` | `fsd` | false | — | `0x007DEED0` |
| 1 | `kRenderDynamicView` | `rdv` | true | — | `0x007DEFA0` |
| 2 | `kRenderStaticView` | `rsv` | true | — | `0x007DF080` |
| 3 | `kRenderWaterEffects` | `rwe` | true | — | `0x007DF160` |
| 4 | `kRenderCloudsAndFog` | `rcaf` | true | — | `0x007DF240` |
| 5 | `kForcePointSampling` | `fps` | false | — | `0x007DF320` |
| 6 | `kForceBilinear` | `fb` | false | — | `0x007DF400` |
| 7 | `kForceNoAlphaBlend` | `fnab` | false | — | `0x007DF4E0` |
| 8 | `kForceNoAlphaTest` | `fnat` | false | — | `0x007DF5C0` |
| 9 | `kRenderShadows` | `rs` | true | — | `0x007DF6A0` |
| 10 | `kRenderDecals` | `rd` | true | — | `0x007DF780` |
| 11 | `kUseSecondStage` | `uss` | true | — | `0x007DF860` |
| 12 | `kForceDebugTexture` | `fdt` | false | — | `0x007DF940` |
| 13 | `kRenderParticleEffects` | `rpe` | true | — | `0x007DFA20` |
| 14 | `kRenderFloraShadows` | `rfs` | true | — | `0x007DFB00` |
| 15 | `kRenderTerrain` | `rt` | true | — | `0x007DFBE0` |
| 16 | `kShowUpdateRegions` | `sur` | false | — | `0x007DFCC0` |
| 17 | `kUseCursorEffects` | `uce` | true | — | `0x007DFDA0` |
| 18 | `kDrawLotTextures` | `dlt` | true | — | `0x007DFE80` |
| 19 | `kDrawLotFoundations` | `dlf` | true | — | `0x007DFF60` |
| 20 | `kDrawLotWalls` | `dlw` | true | — | `0x007E0040` |
| 21 | `kForceTextureRepeat` | `ftr` | false | — | `0x007E0120` |
| 22 | `kUseScreenShake` | `uss` | true | — | `0x007E0200` |
| 23 | `kRenderUI` | `rui` | true | — | `0x007E02E0` |
| 24 | `kUseUpdateRectClipping` | `uurc` | true | — | `0x007E03C0` |
| 25 | `kUseVertexBuffers` | `uvb` | true | — | `0x007E04A0` |
| 26 | `kShowBoundingBoxes` | `sbb` | false | — | `0x007E0580` |
| 27 | `kUseOverlayCulling` | `uoc` | true | — | `0x007E0660` |
| 28 | `kRenderSnapshot` | `rs` | false | — | `0x007E0740` |
| 29 | `kNoAddSigned` | `nas` | false | — | `0x007E0820` |
| 30 | `kNoPartialBackingStoreCopies` | `npbsc` | false | — | `0x007E0900` |
| 31 | `kUseScreenFlash` | `usf` | true | — | `0x007E09E0` |
| 32 | `kRenderAutomata` | `ra` | true | — | `0x007E0AC0` |
| 33 | `kPreferOpaque` | `po` | false | — | `0x007E0BA0` |
| 34 | `kDisplayAlertBorders` | `dab` | true | — | `0x007E0C80` |
| 35 | `kDrawLotOpaqueTextures` | `dlot` | true | — | `0x007E0D60` |
| 36 | `kUseViewCleanup` | `uvc` | false | — | `0x007E0E40` |
| 37 | `kDoAutoZoomAndRotationChanges` | `dazarc` | true | — | `0x007E0F20` |

**int** (24)

| # | Name | Short | Default | Range | Registered at |
|---|---|---|---|---|---|
| 0 | `kViewOffsetY` | `voy` | 0 | — | `0x007E1000` |
| 1 | `kViewOffsetX` | `vox` | 0 | — | `0x007E10D0` |
| 2 | `kTextureBits` | `tb` | 32 | — | `0x007E11B0` |
| 3 | `kLODOffset` | `lodo` | 0 | — | `0x007E1290` |
| 4 | `kShadowDebug` | `sd` | 0 | — | `0x007E1370` |
| 5 | `kMaxParticlesTarget` | `mpt` | 10000 | — | `0x007E1450` |
| 6 | `kParticleLODOffset` | `plodo` | 0 | — | `0x007E1530` |
| 7 | `kEffectPriorityLevel` | `epl` | 5 | 1–5 | `0x007E1610` |
| 8 | `kBackingStoreMaxZoomAll` | `bsmza` | 5 | 0–5 | `0x007E16F0` |
| 9 | `kBackingStoreMaxZoomBuildingsProps` | `bsmzbp` | 5 | 0–5 | `0x007E17D0` |
| 10 | `kBackingStoreMaxZoomTerrain` | `bsmzt` | 5 | 0–5 | `0x007E18B0` |
| 11 | `kBackingStoreMaxZoomFlora` | `bsmzf` | 5 | 0–5 | `0x007E1990` |
| 12 | `kShadowQuality` | `sq` | 2 | — | `0x007E1A70` |
| 13 | `kMinVehicleZoom` | `mvz` | 3 | — | `0x007E1B50` |
| 14 | `kMinPedZoom` | `mpz` | 4 | — | `0x007E1C30` |
| 15 | `kCursorType` | `ct` | 1 | — | `0x007E1D10` |
| 16 | `kDepthOffset` | `do` | 2 | — | `0x007E1DF0` |
| 17 | `kDebugTexture` | `dt` | 0 | — | `0x007E1ED0` |
| 18 | `kDirtyRectMergeFrames` | `drmf` | 4 | — | `0x007E1FB0` |
| 19 | `kSoundDetail` | `sd` | 2 | — | `0x007E2090` |
| 20 | `kBuildingVariety` | `bv` | 2 | — | `0x007E2170` |
| 21 | `kBuildingLoaderSpeed` | `bls` | 1 | — | `0x007E2250` |
| 22 | `kModelLODOffset` | `mlodo` | 0 | — | `0x007E2330` |
| 23 | `kStringTextureCacheSize` | `stcs` | 1024 | — | `0x007E2410` |

**float** (15)

| # | Name | Short | Default | Range | Registered at |
|---|---|---|---|---|---|
| 0 | `kParticleEffectsLOD` | `pelod` | 5 | 1–10 | `0x007E24F0` |
| 1 | `kSplatTextureScale` | `sts` | 50 | 1–128 | `0x007E25D0` |
| 2 | `kTexBindMaxFree` | `tbmf` | 2 | 0–32 | `0x007E26B0` |
| 3 | `kParticleDensity` | `pd` | 1 | — | `0x007E2790` |
| 4 | `kParticleDamping` | `pd` | 0 | — | `0x007E2870` |
| 5 | `kParticleScale` | `ps` | 1 | — | `0x007E2950` |
| 6 | `kLightingUpdateDelta` | `lud` | 0.0001 | — | `0x007E2A30` |
| 7 | `kShadowModelThreshold` | `smt` | 12 | — | `0x007E2B10` |
| 8 | `kDemolishModelThreshold` | `dmt` | 12 | — | `0x007E2BF0` |
| 9 | `kRenderModelThreshold` | `rmt` | 6 | — | `0x007E2CD0` |
| 10 | `kRenderModelThresholdScroll` | `rmts` | 40 | — | `0x007E2DB0` |
| 11 | `kParticleSizeThreshold` | `pst` | 300 | — | `0x007E2E90` |
| 12 | `kMaxVehiclePct` | `mvp` | 1 | — | `0x007E2F70` |
| 13 | `kMaxPedPct` | `mpp` | 1 | — | `0x007E3050` |
| 14 | `kOpaqueUIAlphaValue` | `ouiav` | 0.55 | — | `0x007E3130` |

Short-name collisions *[quirk]*: `rs` is both `RenderShadows` (9) and `RenderSnapshot` (28), `uss` both
`UseSecondStage` (11) and `UseScreenShake` (22) among the booleans; `sd` both `ShadowDebug` (4) and `SoundDetail`
(19) among the integers; `pd` both `ParticleDensity` (3) and `ParticleDamping` (4) among the floats. A find by such
a short name always returns the lower index.

## 4. Users

- The Graphics Rules set properties by name (graphics-rules.md): `Graphics Rules.sgr` writes `particleDensity`,
  `shadowQuality` and others, so the names there are the lower-cased ones of §1.3.
- The lighting manager reads `kShadowQuality` (integer 12, ≥ 2 turns hill shadows on), `kRenderSnapshot` (boolean
  28) and `kLightingUpdateDelta` (float 6) (lighting.md §1.3, §2.1, §6).
- The glyph texture manager reads `kStringTextureCacheSize` (integer 23; glyph-textures.md §1.2).
- The model maker reads `kModelLODOffset` (integer 22; graphics-formats.md §4.2).
- The effects manager's handler of `0xC9F775BB` (`cSC4EffectsManager::DoRenderPropertiesChanged`, `0x00593E10`) pushes
  `kTextureBits` (integer 2), `kLODOffset` (integer 3) and `kTexBindMaxFree` (float 2, × 1,000,000 bytes) into the
  texture binding factory (texture-bindings.md §1.9).
- `cSC4View3DWin` keeps a private scroll snapshot and can restore the application's tables without setters
  (§1.5); its text assignment helper writes the application and posts `0xC9F775BB` (graphics-formats.md §4.2).

## 5. Quirks

1. Four short names are ambiguous; the lower index wins (§3).
2. Get and set do not check the index; set ignores the range, and nothing else uses it (§1.2, §2).
3. Scroll snapshot save/restore bypasses setters and copies the reference-count field too; its enable flag is
   checked separately at begin and end (§1.5).

## 6. Open questions

RP1 (who reads the two extra values of an entry) and RP2 (what the two extra bytes of a boolean entry are) are
answered in §1.2 and archived. RP3–RP5 are answered in
§1.1, §2.1 and §1.3 respectively; their original entries are archived. RP6 asks for the
remaining shipped-language locale/codepage profiles and startup ordering. §1.4 specifies
the native primitive ABI, binding/failure protocol and byte/timing boundaries; §7.3 gives
independent native discriminators. Those are instruction-derived recipes, not measured
per-language outcomes. The C-locale ASCII core domain remains distinct from native binding.
RP7 is answered in §1.5 and §7.4: no public ordered-input journal or snapshot getter exists;
the private scroll snapshot restores tables without setters. Its original entry is archived.

## 7. Conformance tests

1. **Counts:** slots 3, 4, 5 return 38, 24, 15.
2. **Defaults:** `GetInt(23)` = 1024, `GetFloat(2)` = 2.0, `GetBool(28)` = false, `GetBool(9)` = true.
3. **Find:** `FindBool("RenderShadows")` = `FindBool("rendershadows")` = 9; `FindBool("rs")` = 9;
   `FindBool("kRenderShadows")` = −1 (the `k` is not part of the name).
4. **Short names:** `FindInt("sd")` = 4; `FindFloat("pd")` = 3; `FindFloat("ParticleDamping")` = 4.
5. **Set:** `SetInt(23, 4096)` then `GetInt(23)` = 4096.
6. **Names:** slot 15 for 37 gives `doautozoomandrotationchanges`.

### 7.1 Owner and interface discriminators (RP3)

These are instruction-derived **test recipes**, not claims of executed native tests or game
verification. Use a fresh object for each row and heap-compatible storage for any operation
that frees it. A pure model may represent P and U symbolically; an interface adapter must
also preserve the actual pointer adjustments. Observe destruction/free events, never inspect
the object after its lifetime ends.

| Case | Operations / observations | Required result |
|---|---|---|
| Initial count and shared views | Construct; P.AddRef; U.AddRef; P.Release; U.Release | Initial count 0; returned counts 1, 2, 1, 0. The final call destroys all tables and frees P exactly once. |
| Successful queries | From fresh P, QueryInterface(1, out) through P, then through returned U | Both return true in AL and write U (P + 4), never P. Shared count becomes 1 then 2. Releasing the two acquired references returns 1 then 0 and destroys once. |
| Failed query preserves output | Acquire one reference; initialize out to a non-null sentinel; query IDs 0, 2 and `0xFFFFFFFF` through each view, resetting the sentinel each time | Every query returns false in AL, leaves out equal to the sentinel and count equal to 1. Final Release returns 0 and destroys once. |
| Zero-count Release | On separate newly constructed objects, call P.Release or U.Release without AddRef | Each returns 0 and destroys/frees once; neither wraps the count nor keeps the object alive. |
| Destructor flag bit | On separate objects, invoke the deleting destructor with flags 0, 1, 2, 3, through U slot 3 (or its P-receiver body) | Contents destroyed once in every case; outer P allocation freed only for flags 1 and 3. Return value is P. No later query/release is allowed. |

The shared-view case discriminates an erroneous independent counter for each view; the QI
case discriminates returning the render-properties pointer or accepting a second IID; the
failed-query case discriminates clearing out on failure. No overflow or use-after-free test
is needed to establish supported reference ownership.

### 7.2 String and index discriminators (RP4–RP5)

Construct and query under the C locale for these cases; `\0` denotes an actual NUL byte.

| Query bytes before the final readable terminator | Boolean Find | Integer Find | Float Find |
|---|---|---|---|
| `RS\0ignored` | 9 | −1 | −1 |
| `STCS\0ignored` | −1 | 23 | −1 |
| `PD\0ParticleDamping` | −1 | −1 | 3 |
| `\0RenderShadows` (also test an empty string) | −1 | −1 | −1 |
| `RenderShadows ` / ` RenderShadows` / `RenderShadow*` / `kRenderShadows` | −1 for each | −1 for each | −1 for each |

Also query `rEnDeRsHaDoWs` → Boolean 9 and `ParticleDamping` → float 4 after the `PD` case;
this verifies both case folding and scratch-query replacement. Append any high bytes **after**
the first NUL and require the same result as the ASCII prefix; they are not folded or compared.
The no-NUL, all-ASCII queries in the existing §7 tests remain required. Non-ASCII bytes before
NUL and non-C locales are excluded from this table, pending RP6; do not substitute a Unicode
case-folding expected value.

For each of Boolean/integer/float, test **get value, get name and set** at index **0** and
**count − 1**, and read the value back. The last names are respectively
`doautozoomandrotationchanges`, `stringtexturecachesize`, `opaqueuialphavalue`.
Last defaults: true (registration byte 1 at `0x007E0F82`), 1024 (`0x007E2472`), and
binary32 bits `0x3F0CCCCD` (`0x007E3192`; decimal 0.55 rounded to binary32). Use exact, ordinary
values for set/get round trips: false/true; signed 32-bit minimum/maximum; float −2.0/0.5.
Verify that setting one endpoint does not change the other endpoint or a different instance.
To discriminate range clamping independently of index validity, `SetInt(7, −1)` must retain −1
despite the registered 1–5 range; `SetFloat(0, −2.0)` must retain −2.0 despite its 1–10 range
(integer range literals at `0x007E166C–0x007E167F`; float literals at
`0x007E2552–0x007E2562`). No raw NaN or noncanonical Boolean
argument behavior is asserted by these recipes.

If the safe core exposes invalid-index rejection, test **−1, count, signed 32-bit minimum,
signed 32-bit maximum** for each value getter, name getter and setter: None/false, with no
mutation, is the documented safe extension. Label those tests **core safety only**, not
native exactness. Never probe the original with those indices to derive a stable expected
value. Do not pass a failed Find result into an indexed native slot.

### 7.3 Native-binding discriminators (RP6)

These are **unexecuted, instruction-derived recipes**. Use the supported original
executable and a controlled native fixture, not an implementation's outputs as the oracle.
Record PE identity, OS version, game language, calling thread, actual executable-CRT
locale selection success and construction/lookup order. A language menu label or the
plugin CRT's `setlocale` result is not an observation of the executable CRT. Arrange stable
locale state for each recipe; save and restore the original state when deliberately
changing it. Failure to activate a requested profile is an unavailable experiment, not
an ASCII-equivalent result. Do not inject invalid property indices or alter registry data.
For each stable construction profile, build an independent expected short-name list by
direct native classification/mapping of §3's original mixed-case names after `k`, before
constructing the owner. There is no public SDK short-name getter. Retain this list with
the full-name bytes obtained from native getters; do not inspect a guessed object layout
or reuse a portable implementation's abbreviation output as the oracle.

| Discriminator | Independent expectation / observation to retain |
|---|---|
| Primitive identity and C-profile scalar results | Call the two independently verified native targets. In an established C profile, classification of `A` and `I` is nonzero, classification of `a` and `1` is zero. Mapping of 65/73 is 97/105; mapping of −128/−48/−1 preserves that integer. This follows the C branch at `0x009F052F–0x009F053C`, independently of the registry's successful ASCII examples. A bridge declared bool-returning cannot stand in for the integer ABI. |
| Actual arguments and call order | Observe native construction/Find calls without replacing their results. For each source name after `k`, construction classifies original mixed-case bytes, maps selected uppercase bytes into the short name, then maps every full-name byte. Find only maps its copied prefix. A query prefix containing bytes `80 D0 FF` must supply −128, −48, −1 in that order. This distinguishes unsigned promotion, EOF handling, query classification and deriving abbreviations from folded full names. No high-byte classification probe is needed. |
| First-NUL separation | In an established C profile, Boolean bytes `52 53 00 80 D0 FF` find 9 and invoke mapping for 82 and 83 only. Bytes `52 53 FF 00` map three arguments, including −1; they retain a third query byte and miss Boolean 9. `FF` is not an end marker. |
| Locale changed before construction | Choose a profile only after direct original-CRT probes show an actual difference for a registered source character or classification. Construct an original owner there and capture all 77 full-name byte sequences via SDK name getters, and the lookup outcomes for the mixed-case source names and published abbreviations. Compare adapter construction against those native bytes/outcomes, not guessed locale-name tables. If no registered character differs, record that fact; it does not test high-byte equivalence. |
| Locale changed after construction | Keep an original owner built in profile L0 alive; change the executable CRT to an accepted L1 with a directly observed differing mapping, then read names and repeat Find. Its name bytes must stay as captured under L0. Compute the query's expected folded byte sequence using separate direct native scalar probes in stable L1, then apply equal-length/equal-byte, full-before-short, first-entry matching against the captured L0 entries. Also construct a second owner under L1. This distinguishes startup caching, refolding stored entries and retroactive reconstruction. No universal I/i outcome is assigned to an unmeasured profile. |
| High-byte and multibyte profiles | Under each actually accepted single-/multibyte profile, independently probe native mapping for every signed byte −128…−1 and retain both the full integer and low byte. Feed readable NUL-terminated queries containing individual high bytes, adjacent high-byte pairs and UTF-8-looking sequences to the original owner. Derive expected results by independent scalar mapping plus the native stored-name bytes. The owner must still map one signed byte at a time; do not compare with whole-sequence Windows/Unicode lowercasing or assert a universal miss. |
| Binding failure and ownership | In an adapter-only fixture, withhold either primitive, supply an unsupported image identity, or fail the SDK application/properties lookup. Expect an unavailable native binding and no published partial owner, distinct from Find miss. For success, repeated mapping returns integers without allocations to free; name-pointer use ends before owner destruction. These are adapter policy/lifetime checks, not claims about safe execution of a wrong native address. |

Do not describe these recipes as executed native vectors, game/shadow evidence, or proof
that every shipped-language profile has been covered. The last four columns of a future
profile record should include accepted CRT locale/codepage, construction name bytes,
lookup mapping observations and final IDs; retain failures and unchanged profiles too.

### 7.4 Configured state and snapshot vectors (RP7)

These are **instruction-derived conformance vectors and adapter test recipes**, not
executed native/game results. Mutation belongs in an isolated owned fixture; never change
the borrowed application/S merely to provoke a probe. For snapshot cases, an isolated
view fixture must also provide its renderer/message dependencies. A pure expected-state
fixture can check the replay rules, but cannot count that as native view-method coverage.

| ID | Inputs / operation | Required result |
|---|---|---|
| RP7-1: configuration duplicates | A rules property set lists Boolean (9, false), (9, true), integer (23, 4096), (23, 2048), float (2, 1.5). Start with fresh tables. | Setter events are B9=false, B9=true, I23=4096, I23=2048, F2=1.5, in exactly that order. Final values B9=true, I23=2048, F2=1.5; no deduplication. A final-state dump cannot establish the first/fourth writes. |
| RP7-2: raw finite argument | On an isolated P, set float 2 from binary32 bits `0x80000000` (negative zero), then from `0x3FC00000` (1.5); set integer 23 to −17. Use the ordinary masked x87 environment. | Each entry argument remains distinguishable in the trace; float getter after the first assignment has negative-zero sign, after the second returns 1.5, and integer getter returns −17. Range metadata does not clamp −17. No success Boolean is read from a setter. |
| RP7-3: ordinary scroll | Application B30=true; B9=B10=B32=B13=true, F9=1.5, F10=2.0, I23=2048. Begin scroll; during scroll set I23=4096; end scroll with B30 still true. | Save captures the stated pre-begin values. Begin writes B9/B10/B32/B13=false in that order, then F9=2.0; S still has the original values. End restores the originals, including I23=2048 and F9=1.5. There are no setter events for the restoration. |
| RP7-4: disabled begin/end | B30=false throughout; invoke begin and end in the fixture. | No property save, restore or scroll-specific setters from either path. This does not claim absence of their other message/renderer side effects. |
| RP7-5: flag changes | First establish S with a valid enabled begin/end. With B30=true begin again, then set B30=false before end. Separately, after a disabled begin set B30=true before end. | First end skips restoration and leaves intervening writes intact. Second end restores the previously saved S, including S's B30 value; no fresh save is inferred from the disabled begin. Record both condition changes as inputs. |
| RP7-6: text helper target | In an owned fixture, call the view text helper with name `RenderShadows` and empty value, with B9=true; then use a missing name. | First resolves Boolean 9 and sets application B9=false; S is unchanged. Missing name returns false and issues no setter. This helper is not a snapshot getter or a new owner slot. |
| RP7-7: no history bootstrap | Attach after native configuration with no captured startup inputs; or omit a snapshot-restore event from an otherwise complete setter trace. | Mark live expected values incomplete/unavailable. Do not seed from native getters or count resulting equality as independent verification. Counts/names/eligible Find probes can continue under their own domain. |
| RP7-8: observer conflict/lifetime | With the known SCRes/weather debug state occupied, request property tracing without a coordinated independent facility; separately withhold the complete-view bridge or end V's lifetime. | Property observer/snapshot binding is unavailable, debug state is preserved, and S is never used past lifetime or independently released/deleted. This is an adapter obligation, not a native owner error return. |

For an instrumented native run, retain the ordered input/copy record and its completion/gap
status, object generations, comparison boundaries, executable identity, and observer
availability alongside the verdict. Finite setter traces alone do not cover exceptional
floating inputs, invalid indices, lifetime slots, or non-C locale behavior.
