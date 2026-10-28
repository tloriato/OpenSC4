# Advisor system (`cSC4AdvisorSystem`, `cSC4Advisor`, the seven advisor classes) — behaviour spec

Status: round 3, written from the Windows 1.1.641 executable. The Mac named pseudocode was used for names and
intent only. Every fact below was checked on Windows unless it is tagged *[inferred]*. The game's own Lua scripts
(`SimCity_1.dat`, group `0x4A5E8EF6`) are data. This spec names their fields and functions but does not copy them.

Tags: *[compat]* observable, must match; *[choice]* FreeCity4 decision; *[inferred]* not proven;
*[quirk]* original behaviour that looks unintended but is reproduced (or that a replacement must survive).

Shared material cited and not repeated here: streams, refs and strings (foundations §2), class store (§3), city
component table (§3.6), Init order (§3.7), response curves (§4), messages (§5), random generators (§6), month order
(§13), slot-order rule (§16), framework slot tables (§17), x87 precision (§18); scheduler and pause semantics
(simulator spec §5–§6); grid ownership (simgrid §6.4). Subsystem getters are named as in their own specs (aura,
budget, crime, demand, neighbors, ordinance, plumbing, pollution, power, protection, residential, landvalue).

Scope: the simulation side. The advisor windows, the popup tracker, the news ticker and the head animations are UI.
They are named where the simulation calls them and are otherwise left out. MySim (`cSC4MySim`, the MySim advisors)
is out of scope; §14.3 lists every point where this system touches it. The generic Lua host (`cSCLua`, script
packages, the resource loader) is described only as far as the advisor system uses it (§4.2); it belongs in its own
spec.

---

## 1. Role and identity

The advisor system runs the **advice engine**. Advisors are objects created from the Lua `advisors` table. Each owns
the advices of one `advice_type` from the Lua `advices` table. Once per simulated day, each advisor:

1. publishes city statistics into the Lua table `game` (the "variables", §7), and
2. evaluates the Lua **trigger** of a slice of its advices. An advice whose trigger is true becomes **active**: it is
   announced to the UI by message, can raise a "global effect" in the 3D view and can carry a **subject** (a map
   location or an object). It stays active until it times out, its trigger turns false, or a script retires it.

Advices can also react to **events** (`PushEvent(eventID, subject)`) pushed by advisors, other subsystems and
scripts. Reward advices decide whether reward buildings are unlocked (§6.6). The system hosts the Lua state the
advisors, the tutorial and MySim share.

### 1.1 Classes

| Class | CLSID (`GetGZCLSID`) | Interfaces (IID) | Vtables (Windows) | Size |
|---|---|---|---|---|
| `cSC4AdvisorSystem` | `0x4A0DD616` | `cISC4AdvisorSystem` `0x4A0DD617` (primary), `cIGZMessageTarget2` `0x452294AA` (+4), `cIGZMessageTarget` `0xE98B2F57` (+8), `cIGZSerializable` `0xE4FDA3D4` (+0x10) | `0x00A82AC8` (29 slots), `0x00A82A70`, `0x00A82A88` | — |
| `cSC4Advisor` | `0xEAC937B4` | `cISC4Advisor` `0xCA09F544` (+8), `cIGZSerializable` (+0xC), `cIGZMessageTarget2` (+0x10) | class `0x00A81204` (9), iface `0x00A811B0` (21), ser. `0x00A81194`, msg `0x00A81184` | `0xB8` |
| `cSC4AdvisorBase` | `0xCA09F543` (factory only, §15 Q1) | as `cSC4Advisor` | class `0x00A812CC` (10), iface `0x00A81278`, ser. `0x00A81260`, msg `0x00A81250` | `0xC0` |
| `cSC4AdvisorCityPlanning` | `0x2A5F877D` | as above; QI also answers `0x2A5F877D` | class `0x00A81514`, iface `0x00A814C0`, ser. `0x00A814A8`, msg `0x00A81498` | `0xDC` |
| `cSC4AdvisorCitySituations` | `0x4BE372CD` | as above | class `0x00A81C9C` (11), iface `0x00A81C48`, ser. `0x00A81C30`, msg `0x00A81C20` | `0xC0` |
| `cSC4AdvisorFinance` | `0xAA5292D7` | as above | class `0x00A81D4C`, iface `0x00A81CF8`, ser. `0x00A81CE0`, msg `0x00A81CD0` | `0xC4` |
| `cSC4AdvisorHQ` (health and education) | `0x6A5F8755` | as above | class `0x00A81FDC`, iface `0x00A81F88`, ser. `0x00A81F70`, msg `0x00A81F60` | `0xC4` |
| `cSC4AdvisorSafety` | `0xEA19E825` | as above | class `0x00A8237C`, iface `0x00A82328`, ser. `0x00A82310`, msg `0x00A82300` | `0xC4` |
| `cSC4AdvisorTraffic` (transportation) | `0x4A3AD3E1` | as above | class `0x00A82FEC` (10), iface `0x00A82F98`, ser. `0x00A82F7C`, msg `0x00A82F6C` | — |
| `cSC4AdvisorUtility` | `0xCA2C2A2F` | as above | class `0x00A83B8C` (10), iface `0x00A83B38`, ser. `0x00A83B20`, msg `0x00A83B10` | `0xC4` |
| `cSC4Advice` | `0x8A09F5F4` | `cISC4Advice` `0x8A09F5F5` (primary), `cIGZSerializable` (+0xC) | `0x00A808B8` (31), ser. `0x00A80884` | — |
| `cSC4RewardAdvice` | `0xAA371C32` | as `cSC4Advice` | — | — |
| `cSC4AdviceNeghborDeal` | `0x6A3848DB` | as `cSC4Advice`, plus `0xEA3848BA` | `0x00A83BE8` (33) | — |
| `cSC4AdviceSubject` | `0x6A26D049` | `cISC4AdviceSubject` `0x6A26D050` (at +0xC), `cIGZSerializable` | `0x00A81090` (14), ser. `0x00A810C8` | `0x28` |

All interfaces of one object share one reference count. Every class above is registered with the game's COM
director (`cSC4COMDirector` constructor, `AddCls(clsid, factory)`), so `GZCOM()->CreateInstance(clsid, iid)` builds
it. The other advice classes (`cSC4CitySituationAdvice` `0x8BE3753B`, `cSC4MySimAdvice` `0x6A9335DE`,
`cSC4MySimBalloonAdvice` `0x6B70EFAD`) and the MySim advisors (`0x4A1DBBBF`, My Street sim `0x0BF606A5`) are
created through the same tables but belong to the city-situation and MySim specs.

For **record-only** integration, `cSC4CitySituationAdvice` is covered here too: its CLSID must resolve to an
advice supporting IID `0x8A09F5F5`, with exactly the base advice record (§11.3.1, AD17). Its additional
city-situation interface and live behaviour still belong to the city-situation spec.

Factory CLSIDs are not generally QI IIDs: the advisor's additional QI identity is `cIGZUnknown` 1; only
CityPlanning adds its own CLSID. Non-MySim advisors reject `cISC4MySim` `0x0A1DA42F`. See §12.2 (AD14) for
pointer adjustments, ownership and failure output.

### 1.2 Creation and Init order

- The **system** is city component **#47** (foundations §3.6). The city creates it with
  `GetClassObject(0x4A0DD616, 0x4A0DD617)` and reaches it with `cISC4City` `+0x144` `GetAdvisorSystem`, cached as
  `SL::spAdvisorSystem`. Its constructor also stores itself in a **private global** that every advisor, advice and
  Lua extension uses instead of the city getter (§14.2) *[compat]*.
- It is initialised after the vehicle managers and before the city situation manager, the tutorial system and the
  MySim agent simulator (foundations §3.7). So every simulator it reads is already initialised.
- **Advisors** are created by the system from the Lua `advisors` table (§4.3), or by the class store when a saved
  city is loaded (§11).
- **Advices** are created by their advisor from the Lua `advices` table (§4.5), or by the class store on load.
- **Subjects** are created by anyone through `GZCOM()->CreateInstance(0x6A26D049, 0x6A26D050)`.

---

## 2. Data model

### 2.1 The `advisors` table (script data)

The system reads the global Lua table `advisors` (an array, index 1..n). Fields read by the engine:

| Field | Read by | Meaning |
|---|---|---|
| `class_id` | system | C++ class to create (§1.1). Entries whose class is `cSC4AdvisorBase` use the base class |
| `id` | system, advisor | advisor ID, the key of the system's advisor map. **0 = ignored** (template entries) |
| `caption` | advisor | display name. Only the text before the first `|` is kept |
| `name` | advisor | the advisor's personal name |
| `advice_type` | advisor | which `advices` entries belong to this advisor |

Shipped advisors (adv_advisors.lua / adv_const.lua):

| id | Advisor | Class | advice_type |
|---|---|---|---|
| 1–5 | MySim 1–5 | `0x4A1DBBBF` (MySim, out of scope) | 1 |
| 21 | Utilities | Utility | 21 |
| 22 | Health and education | HQ | 22 |
| 23 | City planning | CityPlanning | 23 |
| 24 | Transportation | Traffic | 24 |
| 25 | Environment | `cSC4AdvisorBase` | 25 |
| 26 | Finances | Finance | 26 |
| 27 | Safety | Safety | 27 |
| 28 | Fluff news | `cSC4AdvisorBase` | 28 |
| 29 | City situations | CitySituations | 29 |
| 30 | My Street sim | `0x0BF606A5` (MySim, out of scope) | 1 |

### 2.2 The `advices` table (script data)

An advisor owns every `advices` entry whose `type` equals its `advice_type`, whose `guid` is non-zero and whose
`class_id` is non-zero. An advice keeps the **table index** of its entry. Fields read by `cSC4Advice::Init`:

| Field | Meaning | Stored as |
|---|---|---|
| `guid` | the advice's unique ID (an exemplar-style instance) | GUID; must equal the saved GUID (§4.5) |
| `class_id` | C++ advice class. A mismatch with the object's own class sets the **class mismatch** flag | — |
| `priority` | 0..100 (popup filter, §6.8) | int |
| `mood` | 0 alarm, 1 bad job, 3 neutral, 5 great job, 7 fluff | int |
| `frequency` | minimum days between two activations | int (days) |
| `timeout` | days an active advice stays active | int (days) |
| `trigger` | Lua **expression** (text). Missing or empty means `"0"` | compiled to a function (§4.5) |
| `once` | non-zero: the advice can activate once per city | flag `once` |
| `news_only` | non-zero: news ticker only | flag `news_only` |
| `event` | non-zero: an event ID. The advice is evaluated only when that event is pushed | eventID, flag `event` |
| `command` | non-zero: a UI command ID sent when the advice activates | commandID |
| `persist` | non-zero: stays active when its trigger turns false | flag `persist` |
| `no_timeout` | non-zero: never times out | flag `no_timeout` |
| `effects` | list of global-effect IDs asserted in the 3D view while active | effect list |
| `title`, `message` | text keys (`text@<id>`) or literal text, localized then detokenized for display (§6.9.1) | UI only; no `news` field is registered in the advice's text-field list |

Subclasses read extra fields: `condition` (a function, reward advices, §6.6) and `nd_type` (neighbour-deal advices,
§7.8).

### 2.3 Advice flags

| Flag | Name | Set by | Meaning |
|---|---|---|---|
| `0x1` | news_only | Init | |
| `0x2` | **active** | activation / deactivation (§6.4) | |
| `0x4` | once | Init | deactivation sets `nextDay := 0x7FFFFFFF` |
| `0x8` | expire request | `game.expire_advice()`, base DoMessage (§5.4) | the advisor deactivates it on its next day |
| `0x10` | text dirty | many | the text is rebuilt on the next day pass |
| `0x40` | event | Init | skipped by the daily evaluation; evaluated on events |
| `0x80` | no text | — | forces `GetTrigger` false (§6.1) *[inferred: set by subclasses]* |
| `0x100` | persist | Init | |
| `0x200` | debug show GUID | debug message `0xEA917750` | setting or clearing it also sets `0x10` |
| `0x400` | forced trigger | debug message `0x4A91DD6E` | `GetTrigger` returns true |
| `0x800` | class mismatch | Init | the advisor re-creates the advice (§4.4) |
| `0x1000` | reward | `cSC4RewardAdvice::Init` | |
| `0x2000` | city-situation advice | `cSC4CitySituationAdvice::Init` (`0x004239E7`) | the news ticker skips it. Persisted with the flags |
| `0x4000` | queued for a popup | the popup tracker (§6.8.1) | set when the advice is queued and cleared after it is shown or dropped. An advice with **both** `0x2000` and `0x4000` set is not queued. Persisted with the flags |
| `0x8000` | no_timeout | Init | |

### 2.4 Advice info (per advisor, per advice)

Each advisor keeps a vector of **advice info** records, one per advice it owns, in table-entry creation order:

| Field | Meaning |
|---|---|
| advice | the advice (owning reference) |
| nextDay | first day on which the daily evaluation may test it again |
| expiry | day on which an active advice times out |
| lastMonth | value of the advisor's month counter when it was last evaluated |

plus an **active map** (advice → index in the vector), an **event multimap** (event ID → index) and an **event queue**
of (event ID, subject) pairs.

### 2.5 Subjects

A subject is a small refcounted value: an optional **location** (x, z as float32, in **city cell** coordinates), an
optional **object** (an `IGZUnknown`, AddRef'd), and two `uint32` data words. `SetLocation(x, z)` stores the pair
and then clamps it to `[0, cellCountX] × [0, cellCountZ]` (`cISC4City` `+0x174`/`+0x178`) *[compat]*. Subjects
reach Lua as userdata of the advice-subject tag (§8.1).

---

## 3. State

### 3.1 System

| Item | Initial | Persisted | Notes |
|---|---|---|---|
| advisor map (advisor ID → advisor, ascending ID) | empty | yes (§11.1) | |
| event subscriptions (event ID → set of advisors, ordered by pointer) | empty | no | rebuilt by advisor Init |
| context-advice stack | empty | no | §6.7 |
| popup thresholds `[3]` (one per difficulty) | 100, 100, 100 | no | §6.8 |
| own `cRZRandom` | seeded with `0xFFFFFFFF` (wall clock) in the constructor | no | §9 |
| Lua context (`cISCLua`) | the app's Lua (`SL` global) | no | set in Init |
| two Lua userdata tags | — | no | §8.1 |
| lastDay | 0 | no | signed int32 at primary `cISC4AdvisorSystem` +`0x48` (message-target-2 pointer +`0x44`); last simulation day number handled by the ticker; capture boundary: mysims.md §3.1.1 |
| idle timer (`cRZTimer`) | — | no | 500 ms paused-update timer |
| loadedFromSave | false | no | true after a successful `Read`; cleared on PostCityInit |
| initialised | false | no | |
| reloadPending | false | no | never set (§16 Q2) |
| advisor window | null | no | UI |

### 3.2 Advisor (`cSC4Advisor`)

| Item | Initial | Persisted |
|---|---|---|
| table index (entry in `advisors`) | — | yes |
| caption, personal name | from script | yes (and re-read on Init) |
| month counter | 0 | yes |
| current day (simulation day number) | 0 | yes |
| advisor ID | — | yes |
| evaluation cursor (index in the advice vector) | 0 | yes |
| advice info vector, active map | empty | yes |
| event multimap | empty | rebuilt on Read and Init |
| event queue | empty | yes |
| head animation lists, current head animation | both lists empty; selector true, index −1 (§12.2 AD15) | no (UI; rebuilt from `advisor_heads` during Init) |

### 3.3 Advice (`cSC4Advice`)

| Item | Initial | Persisted |
|---|---|---|
| table index, GUID | from the entry | yes |
| flags | 0 | yes |
| advisor ID | — | yes |
| priority, frequency, timeout, mood | from script | yes (and re-read on Init) |
| global-effect list | from script | yes |
| event subject | null | yes (ref) |
| event ID, command ID | from script | no (re-read on Init) |
| compiled trigger (Lua reference) | none (`−2`) | no |
| trigger text, title, message | from script | no |
| session ID | — | no |
| loaded | false | no (true between Read and Init) |

### 3.4 Subclass state

| Advisor | Item | Persisted |
|---|---|---|
| all subclasses (and the base) | **dirty** byte (variables need an update) | no. Set to 1 by the constructor and by Init |
| all subclasses (and the base) | pending-update flags (bits 1, 2, 4) | no |
| CityPlanning | building count, park count, recreation count, landmark map (exemplar instance → count) | **yes** (§11.5) |
| Traffic | set of lots with a car "zot" | no |
| Safety | fire-active, riot-active, user-disaster bytes | no |
| HQ | ten static building-group vectors (filled once) | no |

---

## 4. Lifecycle

### 4.1 System `Init()` → true *[compat]*

If already initialised, only the last step runs. Otherwise:

1. Take the app's Lua context (instance 1, already opened with `0x7E` by `cSC4App::Init`, lua-host.md §1.4).
   `Open(0x7E)` (`cISCLua` slot 3) is therefore a no-op returning true.
2. `register_api(global = true)` (§8.2).
3. Create two Lua tags with `NewTag`. For each, push the C function **tag-gc** and `SetTagMethod(tag, "gc")`. Tag 0 is
   the advice-subject tag, tag 1 the string tag (§8.1). `GetDataTag(i)` returns them.
4. Register `gAdviceDetokenizerFunction` with the string detokenizer service (`SL` global, slot `+0x1C(fn, this,
   1)`). The detokenizer expands `#token#` text in advice titles and messages (UI, §8.5).
5. `reload_data()` (§4.2).
6. If the advisor map is not empty and the system was **not** loaded from a save: `kill_advisors()` (§4.3).
7. Register the cheat codes `DollyLlama` (`0xAAB7F42A`) and, only when the app's debug functionality is enabled
   (`cISC4App` `+0x54`), `Adv` (`0xA919D25E`).
8. If not loaded from a save: `load_advisors()`. Then mark initialised, `update_advisors()` and the popup thresholds
   (§6.8).
9. Subscribe `PostCityInit` `0x26D31EC1`, `CityEstablished` `0x26D31EC4` and the **ticker** `0xCA5501F2`.
10. Release any advisor window (member `+0x5C`: null it, then `Release` the old one) and call slot 24
    (`startAdvisorWindow`) through the system's own vtable (`0x41B7DD`). Slot 24 builds the window proc (§12.1.1).

### 4.2 Script loading and reload (`reload_data`) *[compat]*

1. Clear reloadPending. If the Lua global `_on_reload` is a function, call it (protected, no arguments).
2. Override existing advisor scripts from loose files: for `<plugin directory>\advisors\` and then
   `<SKU-specific directory>\advisors\` (so the **SKU-specific file wins**), every `*.lua` file whose resource key
   `{0xCA63E2A3, 0x4A5E8EF6, 0xFF000000 | CRC24(basename)}` already exists replaces that script's data. New names are
   ignored. The directories (AD10, verified at `0x0041A54E`–`0x0041A672`) come from the app (`0x00B43C94`, its
   `cISC4App` interface, vtable `0x00A86AF0`): first `+0xD4` slot 53 `GetPluginDirectory(cIGZString& out, int32
   language = −1)` (`0x0044BF90`; −1 skips the language sub-folder append; the field is defaulted by
   `PreFrameWorkInit` to the data directory + `Plugins\`, `0x00452B7B`), then `+0xDC` slot 55
   `GetSkuSpecificDirectory(cIGZString& out)` (`0x00449E20`); `advisors\` is appended to each. The data directory
   (slot 51 `+0xCC`) is **not** used. The earlier "data directory, then plugin directory" wording **DIFFERS**.

   **SKU getter (AD11):** in this executable the SKU-specific string starts empty and the normal app startup
   leaves it empty; it is not the separately populated `Sku_Data\` path. The constructor installs the app
   interface at class +`0x0C` (`0x0044AD19`) and constructs an empty string at class +`0xC8`
   (`0x0044AE5C`–`0x0044AE8C`). Slot 55 copies its character buffer, interface +`0xC0` = class +`0xCC`, into
   the caller's string (`0x00449E26`–`0x00449E32`); it does not construct a directory or perform a lookup.
   `PreFrameWorkInit` defaults the preceding string to `Sku_Data\` (`0x00452BD4`–`0x00452C2B`) and proceeds
   to user data (`0x00452C30`), skipping the SKU-specific string. No later writer to that string is present
   in the app's exported instruction paths (constructor/destructor only). Thus the ordinary second search
   path is relative `advisors\` after the append, with filesystem resolution supplied by the directory helper.
   This is a static startup result, not a measured runtime path. A replacement using the host must still call
   slot 55 and use its returned text, including empty text; it must not substitute `Sku_Data\` or a guessed
   absolute directory if the getter returns empty.

   **Native calls (AD10, verified):** a replacement may call the three helpers as they are. All are plain `cdecl`
   (callee `RET` without pop, caller pops, no `this`):
   `int32 UpdateResourcesFromDirectory(uint32 group, const char* dir)` (`0x005FC510`, 8 bytes; returns the file
   count in EAX, ignored); `int32 LoadPackageScripts(cISCLua* lua, uint32 group, uint32 firstPackage)`
   (`0x005FBA20`, 12 bytes; `reload_data` passes the system's Lua (`+0x58`), `0x4A5E8EF6`, 0 and ignores the EAX
   result; `reload_data` pops 20 bytes at once for the second update call plus this one); `_dofile_from_resource`
   (`0x005FC020`) is a Lua C function `int f(lua_State*)` (returns 1) and works only on a state owned by the game's
   `cSCLua` host (it QIs the object stored 4 bytes before the state, lua-host.md §7.5), which is the case for the
   advisor Lua. It is registered as entry 37 of §8.2 (global).
3. Load the script **packages** of group `0x4A5E8EF6` in ascending package ID order (package headers `--#-package:<hex>`
   in the first bytes of the script). Shipped packages: `0x00000001` (system), `0x00002000`, `0x00003000` (advisors),
   `0x00004000`, `0x01001000` (expansion).
4. `register_api(global = false)` (registers the `game.*` functions). It always returns true (`0x00418940`), so the
   "return false" branch never runs.
5. Hand the Lua context to the tutorial registry (`cISC4App` `+0xC4` → `SetLua`). Return true.

The resource key hash (CRC24, OpenPGP polynomial `0x864CFB`, initial `0xB704CE`), the package loader and
`_dofile_from_resource` are generic Lua host behaviour and are specified in lua-host.md (§7).

`reload_data` runs from `Init` and from the debug cheat `Adv reload` (§5.2). The ticker also calls it when
reloadPending is set, which never happens (§16 Q2).

### 4.3 Advisor creation (`load_advisors`, `update_advisors`, `load_advisor`, `kill_advisors`)

- **`load_advisor(i)`**: read `advisors[i]`. If its `id` ≠ 0: `CreateInstance(class_id, 0xCA09F544)`. If that
  succeeds, `map[id] := advisor`, then `advisor.Init(i)`. If `Init` fails, `RemoveAdvisor(id)`. Otherwise post
  `AdvisorAdded` `0x8A09FA6A` (§10).
- **`load_advisors()`**: if the global `advisors` is a table, `load_advisor(i)` for i = 1..n. Return true; false if
  there is no table.
- **`update_advisors()`**: for each entry i with `id` ≠ 0: if `GetAdvisor(id)` is null, `load_advisor(i)`;
  otherwise `advisor.Init(i)` (re-initialise in place). This is how a loaded city's advisors pick up the current
  scripts.
- **`RemoveAdvisor(id)`** (slot 6): if not found, return false. Send `AdvisorRemoved` `0x8A09FA69`. Remove the advisor
  from every event subscription set (dropping empty sets), erase it from the map. Return true.
- **`AddAdvisor(a)`** (slot 5): `map[a.GetID()] := a` (replacing). Post `AdvisorAdded`. Return true. A null `a`
  returns true and does nothing.
- **`kill_advisors()`**: send `AdvisorRemovedAll` `0x8A09FA6B`, shut down and release every advisor, clear the map.

### 4.4 Advisor `Init(i)` / `Shutdown()` (`cSC4Advisor`)

`Init(i)` → bool:

1. Read `advisors[i]`. Its `id` must equal the advisor's ID when the advisor already has one; otherwise return false.
   Store the table index, caption (cut at `|`), personal name and `advice_type`. Post `AdvisorChanged` `0x8A09FA68`.
2. Collect the matching `advices` entries (§2.2) as a map GUID → (table index, class ID).
3. Clear the **active state** of every advice: `SetFlags(active, off)` only (no deactivation messages), clear the
   active map, send `AdviceRemovedAll` `0x8A09FA67`. *[quirk]* An advice that was active when the city was saved is
   inactive after loading, while its expiry stays as saved.
4. For each existing advice (processed back to front):
   - its GUID is no longer in the table → remove it;
   - `advice.Init(index)` fails → clear its class-mismatch flag and remove it (it is **not** re-created);
   - `Init` succeeds but sets class mismatch → remove it and create it again below;
   - otherwise it is kept, and its GUID is taken off the to-create list.
   Removing = `RetireAdvice(advice)` (slot 20) + `advice.Shutdown()` + erase.
5. Create the remaining entries in ascending GUID order: `CreateInstance(class_id, 0x8A09F5F5)`, `Init(index)`,
   `SetAdvisorID(id)`, append info `{nextDay = currentDay, expiry = 0, lastMonth = monthCounter − 1}`.
6. Rebuild the event multimap and subscribe this advisor (system slot 16) for every event advice's event ID.
7. Load the head animations from `advisor_heads` (§12.2 AD15; every Init), then, once, subscribe the four debug
   messages (§5.4). Return true.

The **base** `Init` (`cSC4AdvisorBase`, also used by every subclass) first subscribes, once, `InsertOccupant`
`0x99EF1142`, `RemoveOccupant` `0x99EF1143`, `PostCityInit` `0x26D31EC1`, `PostCityInitComplete` `0xEA8AE29A` and
`OrdinanceUpdated` `0xC2C09554`, then calls `cSC4Advisor::Init`. Every subclass `Init` sets **dirty := 1** (so the
variables are published on the next day) and then calls the base `Init`. Finance and CitySituations share one `Init`
body; CityPlanning, Safety, Traffic and Utility also subscribe their own messages (§5.5).

`Shutdown()` → true: unsubscribe the debug messages; month counter := current day := 0; clear the active map; send
`AdviceRemovedAll`; unsubscribe all events (system slot 17); shut down and release every advice. The base and
subclass `Shutdown`s unsubscribe their messages first and clear dirty.

### 4.5 Advice `Init(i)` → bool (`cSC4Advice`)

1. Read `advices[i]`. Its `guid` must equal the saved GUID if the advice was loaded; otherwise return false.
2. Re-read priority, mood, frequency, timeout (§2.2).
3. **Compile the trigger**: take the `trigger` text (`"0"` if empty) and run the Lua chunk
   `_atrg_ = function () return (<trigger>); end`. If that succeeds and `_atrg_` is a function, keep a Lua
   reference to it. With the app's debug functionality enabled, the trigger text is also kept for display.
4. Set or clear `once`, `news_only`, `event` (and the event ID), `persist`, `no_timeout` from their fields. Read
   `command`. Compare `class_id` with the object's own class ID: different → set class mismatch.
5. Read `effects` (a list of IDs) and **append** it to the effect list.
6. **Text keys (AD12).** Take the entry's `message` and `title` strings as they are (no URL
   decoding here) and parse each into its text key (§6.9.1): message into the message string (advice `+0x50`), then
   title into the title string (`+0x30`). `SetFlags(no text 0x80, both input strings empty)`, then
   `SetFlags(text dirty 0x10, on)` (`0x004091B7`–`0x004091E6`).
7. If the advice is active and was not just loaded, rebuild its text. Clear the loaded byte.
8. Clear the advice's text-field list (`+0x10`); unless `no text` is set, append `("message", message string)` then
   `("title", title string)` (`0x00409207`–`0x00409253`). Only the fields in this list are rebuilt (§6.9.1). Return
   true. Because step 7 runs before step 8 clears and refills the list, the rebuild in step 7 uses the list of the
   previous `Init` (empty on the first one, so nothing is rebuilt and text dirty stays set).

`cSC4RewardAdvice::Init` also sets flag `0x1000`. `cSC4AdviceNeghborDeal::Init` reads `nd_type`.

### 4.6 System `Shutdown()` → true

Unregister the detokenizer; unsubscribe `0x26D31EC1`, `0x26D31EC4`, `0xCA5501F2`; if member `+0x5C` is set,
remove the advisor window (slot 28 through the own vtable, `0x41A423`; §12.1.1) and release `+0x5C` again if it is
still set; `kill_advisors()`; unregister both cheat codes; set the Lua global `game` to nil and force a collection
(`SetGCThreshold(0)`); release the Lua context; give the tutorial registry a null Lua context; clear initialised.

---

## 5. Cadence and messages handled

The advisor system is **not** a simulator agent. It runs from its own self-posted **ticker** message, so it runs in
the message pump after the simulator has advanced the date *[inferred: one ticker per pump pass]*.

### 5.1 The ticker `0xCA5501F2` (`kAdvisorSystemTicker_MessageType`) *[compat]*

`tick_self()` posts a new ticker message (no target, `MessagePost(msg, 0)`). Handling one:

1. If a city exists and is **not established** (`cISC4City` `+0x58`): return **without** re-posting. The chain stops
   until `CityEstablished` or `PostCityInit` restarts it.
2. `d :=` the simulator date (`+0x1C` `GetSimDate`). If its day number > lastDay (signed):
   1. lastDay := d.
   2. If the day of the month is **28**: force a Lua garbage collection (`GetGCCount`, `GetGCThreshold`,
      `SetGCThreshold(0)`), then **monthly step**: `Simulate(1)` on every advisor in ascending ID order.
   3. Set the Lua variables `g_year`, `g_month`, `g_day`, `g_date` (the day number) and `g_year_count` (current year −
      the year of the simulator's start date, `+0x14`). Each value is converted from uint32 to double.
   4. Go to step 4.
3. Not a new day: if the simulator is paused (`IsAnyPaused`, `+0x48`) **and** the idle timer shows more than 500 ms,
   go to step 4. Otherwise go to step 5.
4. If reloadPending: `reload_data()` and `update_advisors()`. Then **daily step**: `Simulate(0)` on every advisor in
   ascending ID order. Restart the idle timer.
5. `tick_self()`.

Consequences:
- While running, the daily step runs once per simulated day and the monthly step on day 28 of each month, just before
  that day's daily step.
- *[quirk]* If the simulation advances more than one day between two tickers and skips day 28, that month has no
  monthly step *[inferred: depends on frame pacing]*.
- *[quirk]* While paused, the daily step repeats every 0.5 s on the same day. It re-runs the active-advice pass
  (§6.2 step 3) but cannot re-evaluate advices already evaluated this month.

### 5.2 Other system messages

| Message | Handling |
|---|---|
| `PostCityInit` `0x26D31EC1` | If the city (message void1) is established: `tick_self()`; if lastDay < 1, monthly step. Then clear loadedFromSave. |
| `CityEstablished` `0x26D31EC4` | `PushEvent(0xAA31A210 NEW_CITY, null)`; `tick_self()`; if lastDay < 1, monthly step. |
| Cheat `0x230E27AC`, data `0xAAB7F42A` `DollyLlama` | toggle a global byte read only by advisor UI code. |
| Cheat, data `0xA919D25E` `Adv <args>` | debug only. `guids on`/`guids off` → post `0xEA917750` (data1 1/0); `trigger <hex>` → post `0x4A91DD6E` (data1 = hex); `print` → post `0x4A92ECEF`; `refresh` → post `0x8A917759`; `reload` → `reload_data()` and `update_advisors()`. Payload (AD9, verified): see below the table. |

**Cheat payload (AD9, verified).** The system receives cheats as a `cIGZMessage2` (its `DoMessage` `0x0041AAC0` is
the message-target-2 entry), sent by `cGZCheatCodeManager::SendCheatNotifications(text, id)` (`0x00920ECE`, manager
slot 20 `+0x50`): a new standard message with `SetType(0x230E27AC)`, `SetData1(id)` (`+0x2C`) and `SetVoid2(text)`
(`+0x44`), delivered to every message-2 notification target registered for that ID or for 0, then released. So the
cheat ID is **data1** (`GetData1`, `+0x24`) and the typed text is **data2 as a `cIGZString*`** (`GetVoid2`,
`+0x3C`), a borrowed pointer (no AddRef). `DoDefaultCheatCodeProcessing` (`0x00921621`) passes the same string it
was given (it trims/lower-cases only its own copy), so the text is the whole entered line as typed (e.g.
`Adv reload`). For `0xA919D25E` the system takes `GetVoid2()` with **no null test** and calls its `ToChar`
(`+0x14`), builds a `cRZCmdLine` from it (`0x00914E14`) and tests the switches `guids` / `trigger` / `print`
(`IsSwitchPresent` with value, `0x009143CB`) and `refresh` / `reload` (`0x00913EC2`). The "only the cheat ID is
passed" guess **DIFFERS**. (The older `cIGZMessageTarget` list at manager `+0x2C` gets a `cGZMessage {type,
data1 = id, data2 = text}` instead; the advisor system is not on it.)

### 5.3 Advisor `Simulate(kind)` (iface slot 14) → true

- `Simulate(0)` (daily): current day := the simulator date's day number; call the class's **simulate_day**.
- `Simulate(1)` (monthly): month counter += 1; call class slot 8. For the base and every subclass, slot 8 calls
  **update_variables** (§7). For a plain `cSC4Advisor` it does nothing.
- Any other value: nothing.

The base `simulate_day`: if dirty, `update_variables()` and dirty := 0. Then the `cSC4Advisor` day pass (§6.2).

So the variables are published (a) on every monthly step, (b) on the first daily step after `Init` or after a
`PostCityInit`, and (c) in part (the "occupant variables") whenever buildings change (§5.4).

### 5.4 Base `DoMessage` (`cSC4AdvisorBase`) *[compat]*

| Message | Handling |
|---|---|
| `InsertOccupant` / `RemoveOccupant` | If the occupant is a building (`0x278128A0`) and its property `0xEA3209F8` (conditional building) is true and `BuildingDevelopmentSimulator` `+0x38` `GetBuildingKeyFromType(type, &key)` succeeds: post **UpdateSelf**(1, key instance). |
| `UpdateSelf` `0xCA8F3DB6`, data1 = 1 | `a := GetAdviceByGUID(data2)`. If found: `a.GetTrigger()` (this refreshes the reward status, §6.6). If the civic simulator's status for that building is missing or lacks bit 2: `a.SetFlags(expire request, on)` and dirty := 1. |
| `UpdateSelf`, data1 = 2 | `GetTrigger()` on every advice with the reward flag. |
| `UpdateSelf`, data1 = 0 | nothing here (subclasses use it). |
| `OrdinanceUpdated` | post UpdateSelf(2, 0). |
| `PostCityInitComplete` | `GetTrigger()` on every advice whose class is `cSC4RewardAdvice`. |
| `PostCityInit` | `update_variables()`. |
| anything else | `cSC4Advisor::DoMessage` (debug messages below). |

UpdateSelf(a, b) is posted to the advisor's own message target (`MessageServer2` `+0x1C`
`GeneralMessagePostToTarget`), data1 = a, data2 = b.

`cSC4Advisor::DoMessage` handles only debug messages. `0x4A91DD6E` (data1 = an advisor ID or an advice GUID): set
forced trigger (`0x400`, or `0x600` for a GUID) on matching inactive advices whose trigger text is not `"=(0)"`, then
post `0xEA917750`. `0x4A92ECEF` prints, `0x8A917759` refreshes texts, `0xEA917750` sets or clears `0x200` on the
active advices.

### 5.5 Subclass messages *[compat]*

All subclasses keep a set of pending-update bits. On a building insert or remove they post UpdateSelf(0, 0) if no bit
was set, then set their bits; on UpdateSelf they run the matching update and clear the bits.

| Advisor | Messages | Effect |
|---|---|---|
| Finance | Insert/RemoveOccupant (building) | post UpdateSelf(0,0) if idle; mark pending. UpdateSelf clears it. *[quirk]* Nothing is recomputed: the flag has no reader. Then the base. |
| CityPlanning | Insert/RemoveOccupant | Non-buildings: return **without** calling the base *[quirk]*. Buildings: building count ±1; if group `0x1006` park count ±1; if group `0x1006` again, recreation count ±1 *[quirk: same group]*; if group `0x150A` (landmark): `key := GetBuildingKeyFromType`, insert sets `landmarks[key.instance] := 1`, remove decrements it if present (never erased). Then post/mark, then the base. |
| CityPlanning | UpdateSelf | occupant variables if pending; then the base. |
| CityPlanning | `0xEA394B74`, `0xCB3BE815`, `0xEB3BE415`, `0x2B3BE44A`, `0x6B3BE6D8`, `0x0B3BE81A` (airport growth), `0x8A394DC4` (seaport growth), `0x4A490D33` (seaport bad location) | subscribed, but no handler exists *[quirk]*: they fall through to the base and do nothing. |
| HQ | Insert/RemoveOccupant (building), UpdateSelf | pending → occupant variables. The base is called for every message. |
| Safety | Insert/RemoveOccupant, UpdateSelf | as HQ. |
| Safety | strikes, jailbreak, disasters, pipe burst, blocked vehicles | set variables and push events (§7.9). No base call. |
| Traffic | `0x4C04DE69` `LotDeveloperNotification` | data1 = 1: add the lot (message IGZUnknown) to the zot set; 2: remove it. Then `g_car_zot_count` := set size. No base call. |
| Traffic | Insert/RemoveOccupant (building or network `0xC772BF98`) | pending → occupant variables. Then the base. |
| Utility | Insert/RemoveOccupant (building or network) | pending (bits 1, 2) → occupant variables. |
| Utility | `0xEA418D8F` (neighbour-deal status report, neighbors.md §5.3) | pending (bit 4) → deal variables. data1 3/4/5/6 → push event `0x3A383059` / `0x0A383053` / `0xAA383052` / `0x4A383049` to **this advisor** with a subject whose object is the deal (§6.3). |
| Utility | `0xCA358F85` / `0xCA358F9C` (power strike begin/end) | `g_power_strike` := 1/0, `PushEvent(0xEA31A30A / 0x2A31A30F)`. *[quirk]* No code posts these messages. |
| CitySituations | none of its own | |

### 5.6 Ordering

The daily step reads the other subsystems' **current** values at the moment the ticker runs, after the simulator's
day tick *[inferred]*. Values computed monthly by the simulators (foundations §13) are read as they are, whatever
their month position. Nothing in the simulation reads the advisor variables back except through Lua (§14.1).

---

## 6. Advice evaluation *[compat]*

### 6.1 Triggers

`get_raw_trigger()` (raw value):
1. `PushContextAdvice(this)` (§6.7).
2. No compiled trigger → false. Otherwise call the compiled function (protected, 0 arguments, 1 result).
   A Lua error → false (with debug functionality, the error is reported).
3. A **boolean** result is used as is. A **number** is true unless it equals 0.0 (NaN → true). Any other type → false.
4. Pop the result and `PopContextAdvice()`.

`GetTrigger()` (slot 8) → bool in AL: forced trigger (`0x400`) → true; otherwise `get_raw_trigger() && !no text
(0x80)`. Reward advices override it (§6.6).

### 6.2 The day pass (`cSC4Advisor::simulate_day`)

Let n = number of advices.

1. **Batch size** `b = trunc(f32(f32(n × 0.033333335f) + 1.0f))` (float32 per foundations §18). Evaluate b advices
   starting at the cursor, wrapping, and leave the cursor after the last one. So each advice is visited about once
   every 30 days.
2. For each visited advice that is neither active nor an event advice: if `nextDay ≤ currentDay` and `lastMonth <
   monthCounter`: lastMonth := monthCounter; if `GetTrigger()` → **activate** (§6.4).
3. Then for every **active** advice (in active-map order):
   - deactivate it if `currentDay ≥ expiry` and not no_timeout, **or** expire request is set, **or** it is not
     persist and `GetTrigger()` is false;
   - otherwise, if text dirty: rebuild its text and send `AdviceChanged` `0x8A09FA66`.
4. Process the event queue (§6.3).

Consequence *[compat]*: an advice is tested at most once per advisor month (monthCounter changes on the monthly step),
and not before `nextDay`.

### 6.3 Events

- **System `PushEvent(event, subject)`** (slot 15) → true: for every advisor subscribed to `event` (in pointer
  order), `advisor.PushEvent(event, subject)`.
- **Advisor `PushEvent(event, subject)`** (slot 18) → true: append (event, subject) to its queue (AddRef).
- `SubscribeToEvent` / `UnsubscribeFromEvent` (slots 16/17) maintain the system's event → advisor sets.
- **`simulate_day_events`** (end of the day pass): pop each queued (event, subject) in order. For each advice
  registered for `event`:
  - inactive: lastMonth := monthCounter; `SetEventSubject(subject)`; if `GetTrigger()` → activate. *[quirk]* No
    `nextDay`, `once` or month check: an event advice can fire on every event.
  - active: set text dirty; if `GetTrigger()` → `SetEventSubject(subject)`.

The **Utility** advisor overrides `simulate_day_events` for the four neighbour-deal events: for each advice of the
event, it reads the deal from the subject object (`cISC4NeighborDeal`, IID `0x0773FEF1`) and requires the deal's
type to equal the advice's `nd_type`. An inactive match is tested (`GetTrigger` before `SetEventSubject`) and
activated; an active advice for the same deal is refreshed; an active advice for **another** deal is cloned
(`Clone`, slot 5), and the clone is added to the vector and the event multimap and activated if its trigger is true.
*[quirk]* Clones are never removed. Other events follow the base logic, except that the trigger is tested before the
subject is set.

### 6.4 Activation and deactivation

**Activate** (index i): nextDay := 0; expiry := currentDay + timeout; active map[advice] := i; `SetFlags(active,
on)` (rebuilds the text if dirty and asserts the global effects); `SetFlags(expire request, off)`; post
`AdviceAdded` `0x8A09FA64` (IGZUnknown = advice). If the command ID ≠ 0, post `AdviceCommand` `0x8A09FA6C`
(IGZUnknown = advisor, data1 = command ID).

**Deactivate**: `SetFlags(active | expire request | forced trigger, off)` (sets text dirty, retracts the global
effects); nextDay := `0x7FFFFFFF` if once, else currentDay + frequency; expiry := 0; `SetEventSubject(null)`; **send**
`AdviceRemoved` `0x8A09FA65` (IGZUnknown = advice).

### 6.5 Retire, command test, lookups

- `RetireAdvice(a)` (slot 20) → bool: false if `a` is null or inactive. Otherwise erase it from the active map and
  deactivate it; true. *[quirk]* The base version does not check that `a` is in the active map; an active advice
  missing from the map would use a garbage index. The CitySituations override checks both and returns true even when
  the advice is not found.
- `CommandWouldTrigger(cmd)` (slot 19) → bool: scanning from the last advice, true if some **non-event** advice has
  `GetCommandID() == cmd` and a true `GetTrigger()`. Each tested trigger runs its Lua function.
- `GetAdviceByGUID(g)` / `GetAdviceBySessionID(s)` (slots 12/13): linear search, null if none.

### 6.6 Reward advices and conditional buildings

A reward advice (`cSC4RewardAdvice`, class `0xAA371C32`, used by the CityPlanning and Finance tables) belongs to a
reward building whose exemplar instance is the advice's GUID. Its `GetTrigger()`:

1. `PushContextAdvice(this)`. `st := CivicBuildingSimulator.GetConditionalBuildingStatus(guid)` (civic `+0x18`),
   copied (all zero if missing).
2. Call the entry's Lua function `condition` (protected). Its result sets the status:
   - a **string**: status &= ~3; the tooltip text := the detokenized string;
   - **number 0**: status := (status & ~2) | 1;
   - **number 1**: if status lacks bit 4: create a subject from the building type's data (BuildingDevelopmentSimulator
     `+0xA0`), `SetEventSubject` it and set trigger := true. Status := 6.
3. `CivicBuildingSimulator.UpdateConditionalBuildingStatus(guid, status)` (civic `+0x1C`). `PopContextAdvice()`.
4. Return trigger && !no text.

So **the civic building simulator does not compute reward availability**: the advice writes it. The shipped
conditions compare `game.g_population`, `game.ga_mayor_rating`, grades and similar variables with tuning constants.
Reward triggers are re-run on `PostCityInitComplete`, on `OrdinanceUpdated` and when a conditional building is
added or removed (§5.4). The Lua functions `get_reward_building_state`, `unlock_reward_building` and
`reward_instance_count` read and write the same status (§8.3).

### 6.7 Context advice

A stack of advices (each AddRef'd). `PushContextAdvice(a)` (slot 18) pushes; `PopContextAdvice()` (slot 19) removes
the top, releases it and returns the pointer (now unreferenced by the stack); `GetContextAdvice()` (slot 20) returns
the top or null. Most Lua functions act on the context advice (§8.3).

### 6.8 Popup priority thresholds

`init_advice_popup_priority_thresholds`: thresholds := 100, 100, 100, then read
`tuning_constants.ADVICE_POPUP_PRIORITIES_BY_SKILL_LEVEL` {`low`, `medium`, `high`} into difficulty 0, 1, 2 (a missing
key keeps 100). Shipped: 60, 80, 100. `GetAdvicePopupPriorityThreshold()` (slot 21) returns the value for
`cISC4City.GetDifficultyLevel()` (`+0x60`); any value other than 1 or 2 uses difficulty 0. The popup tracker
uses it (§6.8.1).

The thresholds are read **once**, at the system's first `Init`, after `reload_data` (§4.1 step 8). Each key is read
from the Lua table as a number and stored as an `int32`. No interface slot sets them, and a later change to the Lua
table has no effect until the next city's `Init`.

#### 6.8.1 Popup display and the simulation pause *[compat]*

Written for unattended runs (docs/ops/running-the-game.md, "Popups": an advisor dialog holds the date while `OnTick` keeps counting). Everything
here was read from the Windows bytes. The popup is UI, but it is the one place where the advisor system stops the
simulation, so a replacement or a test harness needs it.

**Who shows the popup.** The **advisor popup tracker** (`cSC4AdvisorPopupTracker`, `0x00762BA0`–`0x007638E0`) is a
24-byte UI object. The city view creates it in `cSC4View3DWin::AddGUI` (`0x007EEFE5`), one per city view. It is
**not** a GZCOM class: it has no class ID and is not in the COM director, so a plugin can neither replace it nor
reach it through an interface. `QueryInterface` answers `0x00000001`, `0x2A5A754D` (its window-procedure
interface) and `0x452294AA` (`cIGZMessageTarget2`). At `Init` it subscribes on MessageServer2 to three messages and
posts one tick to itself:
- `AdviceAdded` `0x8A09FA64`;
- its own tick `0xEA83607B`;
- `kSC4MessagePreCityShutdown` `0x26D31EC2`.

It keeps a FIFO **queue** of advices (each AddRef'd).

**What it does with each message** (`DoMessage`, `0x00763450`):

1. **`AdviceAdded`.** The advisor posts this on every activation (§6.4). The tracker QIs the message's
   `IGZUnknown` for `cISC4Advice`. Unless the advice has **both** flags `0x2000` and `0x4000` set, the tracker:
   - appends the advice to the queue;
   - sets `0x4000` (`SetFlags`, slot 14);
   - posts a tick `0xEA83607B` to itself (`GeneralMessagePostToTarget`).

   The skip test is `CheckFlags(0x6000)` (slot 15), which tests **all** bits of the mask. One of the two bits alone
   does not keep an advice out.
2. **`PreCityShutdown`.** Clear the queue.
3. **Tick.** If the queue is empty, or there is no city, or the city has no advisor system: nothing. Otherwise the
   **safety gate** `SC4GetPopupModalDialogsSafe` (`0x005F9000`) runs. It is false in any of these cases:
   - a mouse button is held: Win32 `GetKeyState` of `VK_LBUTTON`, `VK_MBUTTON` or `VK_RBUTTON` is negative;
   - **a modal window is already running**: the window manager (framework service `0xA417445E`, IID `0x5A4`)
     slot 42 (`+0xA8`) reports a modal depth ≠ 0;
   - the city's UI lock count is not 0 (`cISC4City` slot 108, `+0x1B0`, `UIGetLockCount`).

   **If the gate is false**, the advice stays queued and the tick is re-posted, so it is retried on every pump pass.
   **Two tracker popups therefore never nest**: a queued advice waits until the open dialog closes.

   **If the gate is true**, the tracker pops the front advice and shows it **only if all of these hold**:

   | Gate | Test | Interface |
   |---|---|---|
   | a | app popup dialogs enabled | `cISC4App` slot 23 (`+0x5C`) `GetPopupDialogsEnabled`, through `SC4GetPopupModalDialogsEnabled` (`0x005F9090`) |
   | b | the "Urgent Advice Dialogs" play option is on | the preferences byte at offset `0xF0B` (see option 2 below) |
   | c | `advice.GetPriority()` ≥ `system.GetAdvicePopupPriorityThreshold()` (signed) | `cISC4Advice` slot 7; `cISC4AdvisorSystem` slot 21 (§6.8) |
   | d | no tutorial is running | `cISC4City` slot 82 (`+0x148`) tutorial system, if any: its slot 8 (`+0x20`) `IsTutorial()` is false |
   | e | the object at `cISC4City` slot 43 (`+0xAC`), if any, returns false from its slot 13 (`+0x34`) | *[inferred: the vehicle manager's U-Drive-It mode]* |

   The tracker does **not** look at `news_only` (`0x1`), and it does not check that the advice is still active. Mood
   only picks the layout.

   **When the advice is shown**, in this order:
   1. Create the dialog from the UI script (group `0x96A006B0`, instance `0x4A5A89D4` for mood < 3, `0x4A5A89D5` for
      mood ≥ 3).
   2. **Pause** (below).
   3. Set the dialog up, then run the window manager's **modal loop** (slot 41, `+0xA4`, `DoModalWin`). This call
      **blocks inside the message handler** until the dialog ends.
   4. Afterwards, if the advice is a reward advice (`0x1000`): `SetFlags(8 expire request, on)`.
   5. **Resume**.

   **In every case after a pop:** `SetFlags(0x4000, off)` and release the advice. If the queue is still not empty,
   re-post the tick.

   So an advice that fails a gate leaves the popup path for this activation. It stays active in its advisor, and the
   news ticker can still show it (the ticker skips advices with `0x2000`, `0x00772BDB`).

**How the popup stops the simulation.** A scoped `cSC4AutoGamePause(sound no, animation no, simulator yes, clock
yes)` (`0x005F9320` / `0x005F9360` → `SC4GamePause` `0x005F91C0`) surrounds the modal loop:

| Object | On show | On close |
|---|---|---|
| simulator: framework service `0x46956659`, IID `0x8695664E` | `cISC4Simulator` slot 10 (`+0x28`) **`HiddenPause()`** | slot 13 (`+0x34`) **`HiddenResume()`** |
| city 24-hour clock: `cISC4City` slot 86 (`+0x158`, `Get24HourClock`) | clock slot 26 (`+0x68`) with 1 | the same slot with 0 |

It is the **hidden** pause (simulator.md §6), not `Pause()`. So `IsPaused()` stays false while `IsHiddenPaused()` and
`IsAnyPaused()` are true. With FreeCity4's simulator replacement, these two calls arrive in **our**
`cISC4Simulator`; that is the `hidden` count in the plugin heartbeat.

While the city is held:
- the advisor ticker takes its paused branch, so the daily step runs every 0.5 s on the same day (§5.1 step 3);
- advices can therefore still activate and deactivate, and new `AdviceAdded` messages queue behind the open dialog;
- the date does not advance.

**The hidden count is not a popup count.** Other code also uses the hidden pause, and any of these can raise the count
while a popup is open:
- the city view's own short pause during camera moves (`cSC4View3DWin::DoTemporarySimHiddenPause` `0x007E5B40`,
  released by a timer);
- city-situation popups;
- the generic modal dialogs;
- saving.

The tracker itself never holds more than one.

**How the dialog ends** (the tracker's window procedure, `0x00762D30`):
1. On **every** window message the dialog receives, the tracker first reads the advice the dialog shows (window
   property `0xEA9F13C0`). If that advice is **no longer active** (`CheckFlags(2)` false) or has an **expire request**
   (`CheckFlags(8)`), it ends the dialog.
2. Then, by message:
   - a command message (type 3) whose data1 is `0x287259F6`, "a button was pressed" (data2 is the button's ID, and
     **any** button ID counts): end the dialog;
   - a link message `0xE4A85D91`: follow the link (`cSC4HTMLLinkHandler::DoURL`), then end the dialog;
   - message types `0xA2BF8AD5` and `0xA2BF8AD6`: swallowed with no action. They are the notices a window procedure
     gets when it is attached to or detached from a window (`cGZWinGen::SetWinProc`, `0x009998C8`), not keys. The
     procedure handles no keyboard messages at all, which is why **Enter and Esc do nothing** (run 48).
     (Corrected by modal-dialogs.md §6.)

Ending clears the property, ends the modal loop (window manager `EndModal`, modal-dialogs.md §2.4) and destroys the
window. The loop then returns into the tick handler, which resumes the simulator and shows the next queued advice,
if any. That is why closing one popup shows the next.

**Other popups that hold the simulation** (gates b–e do not apply to them):
- **City-situation popups** (`cSC4CitySituationManager::ShowSituationPopup`, `0x0052E910`) take a hidden pause
  through the same `cSC4AutoGamePause` (with sound paused as well). They test **neither** gate a nor gate b.
- **The generic dialogs** (`nSC4UI::GenericNotificationDialog` `0x0078DD80`, `GenericConfirmationDialog`
  `0x0078DFF0`, `DoScriptedModalDialog` `0x0078E2F0`) test only gate a.

**Ways for a plugin to suppress or dismiss advisor popups, best first.**

1. **`cISC4App::SetPopupDialogsEnabled(false)`.** This is `cISC4App` (IID `0x26CE01C0`) slot 24 (`+0x60`): thiscall,
   one `bool`, pops 4, no return value. The getter is slot 23. The flag:
   - is set to **true** by the app constructor (`0x0044B316`);
   - is written by nothing else except this slot;
   - is **not persisted**, and no ini, registry or command-line switch sets it.

   Call it after ConnectionsReady (see below), and before the first popup tick. With the flag
   false:
   - every queued advice fails gate a and is dropped, with **no pause**;
   - `GenericNotificationDialog` shows nothing;
   - `GenericConfirmationDialog` returns false without asking;
   - `DoScriptedModalDialog` returns 0 without asking. It is used by the quit, go-to-region, obliterate and
     budget-failure prompts (what the callers then do: modal-dialogs.md §4; budget failure still closes the
     city);
   - the missing-plugin report and the terrain edge-reconciliation prompt are skipped;
   - **the whole terrain edge reconciliation is skipped, and with it ConnectionsReady.** With the default
     preference, `ReconcileEdgesWithNeighboringCities` returns before posting TerrainReconciliation, so the regional
     city never posts ConnectionsReady. Power, plumbing and the neighbours simulator then stay disabled for the
     session (foundations.md §11.6).

   City-situation popups are not affected.

   **So set the flag false only after ConnectionsReady has arrived** (subscribe to `0x6AC284F3`), not at PostAppInit
   or at city load.

   The original has its own automation command for exactly this: `SetPopupModalDialogsEnabled` (command `0x6BCE8AE6`,
   registered by `cGZMiscCommandSystemSupport` next to `GetPopupModalDialogsEnabled` `0xABCE8AA7` and
   `GetPopupModalDialogsSafe` `0x0BCE8AA5`). So its developers turned popups off this way for scripted runs.
2. **The "Urgent Advice Dialogs" play option** (Options → Play Options, checkbox control `0x4A7F19F5`, default on).
   This is gate b, and it affects advisor popups only.
   - The value is byte `0xF0B` of the preferences block that `cISC4App::GetPreferences()` (slot 38, `+0x98`)
     returns. The `cSC4Preferences` constructor sets it to 1.
   - It is saved with the rest of the block by `cISC4App::SavePreferences` (slot 17) into the app preferences file,
     so **unticking it once by hand keeps it off** for that user profile.
   - A plugin can write the byte, but that depends on the block's layout, not on an interface.
3. **Lua thresholds above 100** (gate c; advisor popups only). The thresholds are read only at the system's `Init`
   (§6.8), so the table must already hold the new values when `reload_data` finishes:
   - `tuning_constants.ADVICE_POPUP_PRIORITIES_BY_SKILL_LEVEL` is assigned in `adv_const.lua` (key `0xFF048A3E`);
   - `package_startup` (`0x2000`) runs that file, and several later shipped scripts run it again;
   - so set `low`, `medium` and `high` to 101 or more from a **package script that loads after every shipped
     package**: a Lua resource (type `0xCA63E2A3`, group `0x4A5E8EF6`) in a plugin `.dat` whose header is
     `--#-package:<id>#`, with id > `0x01001000` as a signed number (lua-host.md §7.4).

   A loose file in `<plugin dir>\advisors\` cannot do it: such files only *replace* an existing script (lua-host.md
   §7.3), so overriding `adv_const.lua` would mean shipping the whole game file. **Not tested.**
4. **On the simulator side** (FreeCity4 only). Our `cISC4Simulator` receives the `HiddenPause`/`HiddenResume` pair.
   An autotest switch could count hidden pauses without honouring them. The framework keeps calling `OnTick` during
   the modal loop (about 1500/s in run 48), so the date would advance under an open dialog. Caveats:
   - this also cancels every other hidden pause: the hold from `Init` until PostCityInit (simulator.md), the camera
     pauses and the saving pauses. So it must start after PostCityInit and only in autotest;
   - the dialog stays on screen, and later advices queue behind it.

   Use it only together with option 1.
5. **Dismissing an open popup without a click** (*[inferred, not tested]*). Make the shown advice inactive or flag
   it, by either of:
   - `cISC4Advice::SetFlags(8, true)` (slot 14);
   - retiring it through its advisor: `cISC4AdvisorSystem::GetAdvisor(advice.GetAdvisorID())`, then
     `cISC4Advisor::RetireAdvice` (slot 20).

   The dialog then ends at the **next window message it receives** (step 1 of "How the dialog ends"). It was not
   checked whether such a message arrives with no user input.

   To find the advice: while it is queued or on screen it carries `0x4000` (`CheckFlags(0x4000)`). It can be
   reached through the advisors' active maps (`cISC4Advisor` slots 10/11), or by subscribing to `AdviceAdded`
   `0x8A09FA64` and remembering what arrives. Keys cannot dismiss it (they are swallowed).
6. **Dismissing an open popup through the window manager** (best for a harness). Post the dialog the same command a
   click posts: `cIGZWinMgr::PostMessageToTarget(window, 3, 0x287259F6, 0, 0)` (slot 14), with the window taken from
   the modal begin notice `0x62BA7B42`. The same approach closes the other modal dialogs too (two of them need
   their own button ID); see modal-dialogs.md §3. Do not use `EndModal` alone on an advisor popup: the window would
   never be destroyed.

**Not available:**
- There is no per-advice priority setter: `cISC4Advice` has only `GetPriority` (slot 7). The priority comes from the
  Lua `priority` field at advice `Init`.
- There is no threshold setter: system slot 21 is a getter.
- The advisor system's own cheat codes (`DollyLlama`, `Adv`, §4.1 step 7) do not touch popups.
- Setting `0x2000 | 0x4000` on an advice before the tracker handles its `AdviceAdded` keeps it out of the queue.
  But `0x2000` also hides it from the news ticker, and a plugin cannot control the order in which it and the
  tracker receive the message. **Not recommended.**

*[choice]* For long unattended runs FreeCity4 should use option 1, set once per session and logged, and may tell the
user about option 2 as a manual fallback.

### 6.9 `SetFlags`, text and global effects

`SetFlags(mask, on)` (slot 14) → true:
- on: flags |= mask. If mask contains active: rebuild the text if text dirty, and if the advice was not already active,
  notify the global effects (assert).
- off: flags &= ~mask. If mask contains active: set text dirty, and if the advice was active, notify the global
  effects (retract).
- Either way, mask containing `0x200` also sets text dirty.

`NotifyGlobalEffectTriggerChanged()` (slot 23, void): for each effect ID, View3D `+0xBC`
`AssertGlobalEffectTrigger(id)` if active, else `+0xC0` `RetractGlobalEffectTrigger(id)`.

`UpdateText()` (slot 28) calls the class's text rebuild (class slot 30). Text rebuilding reads `message` and
`title` (the only registered text fields), runs the detokenizer, and is UI only (§6.9.1).

<a id="ad12"></a>

#### 6.9.1 Text keys, `GetTextTitleGUID` / `GetTextMessageGUID` and the text rebuild (AD12) *[compat]*

The title (advice `+0x30`) and the message (`+0x50`) are localized-string objects (`cSLString`): a text plus a
resource key `{type +0x14, group +0x18, instance +0x1C}` of the string object.

**Parsing a field into a key** (`cADVString::FromScriptText` `0x00417A00` → `cADVString::init` `0x004178F0`; the
length argument is ignored): the key becomes type `0x2026960B`, group `0x6A231EA4`, instance = the first of these
`sscanf` patterns that converts one value, tried in this order: `text@0x%08x`, `text@%08x`, `text@%u`, `text@%i`;
instance **0** when none matches. Only the key is set; the string's text is not touched. Examples:
`"text@0x4A1B0001"` → `0x4A1B0001`; `"text@4a1b0001"` → `0x4A1B0001` (hex, the second pattern); `"Hello"` → 0;
`""` → 0.

**Slots 19 and 21** (`0x00407D70`, `0x00407DA0`) return the **instance** of the title / message key (zero-initialised
local key filled by `cSLString::GetKey` `0x00602F70`, read back at `+8`). So the GUID is 0 for literal (non-`text@`)
text and for an advice whose key was never parsed. The value reflects the last parse (Init step 6 or the last
rebuild). A replacement that returns 0 for every advice **DIFFERS** for `text@` fields.

**Slots 18 and 20**, `cIGZString* __thiscall GetTextTitle()` and `GetTextMessage()`, return borrowed pointers to
the string objects themselves (`+0x30`, `+0x50`; `0x00800640`, `0x00407BB0`); there is no AddRef or allocation.
Their text is whatever the last rebuild left (empty before the first one). Reading a GUID or text getter does
not rebuild it. These are `__thiscall` methods: the GUID is a u32 in EAX, and the string pointer is its address
in EAX; no stack arguments. `GetKey` copies three u32s; `SetKey` only writes them
(`0x00602F70`, `0x00602F90`).

**Text rebuild** (class slot 30, `cSC4Advice::refresh_text` `0x00409300`) → true:
1. If the text-field list (`+0x10`, filled by Init step 8) is empty: return at once (text dirty is **not**
   cleared, no context push).
2. `PushContextAdvice(this)` (system slot 18); open a table reader on `advices[tableIndex]` (index at advice
   `+0x88`) with the system's Lua state.
3. If the reader is valid, for each `(name, string)` pair in list order (message, then title):
   1. Read the entry's field `name` as a string (`PushString(name)`, `GetTable(-2)`, `ToString(-1)`, `Pop(1)`).
   2. URL-decode it (`gConvertURLFromEndodedForm` `0x004184B0`).
   3. Parse the decoded text into the string's key (as above).
   4. `cSLString::Load` (`0x00603300`, both arguments 0): load the text for the key from the text database. If
      that fails, the string's text becomes `"***_NOT_IN_TEXT_DATABASE_*** "` followed by the decoded field text,
      converted from code page 1252 (`ConvertStringEncoding(.., 0x4E4, ..)`) (`0x00409459`–`0x00409558`). So a
      literal (instance 0) field displays with that prefix if its resource lookup fails too.
   5. Detokenize in place: `cSLString::ConvertTokens(null, true)` (`0x00602FB0`): the global detokenizer
      (`0x00B43CDC`, lua-host.md §10), if present, with HTML-clean mode forced on for the call and restored after,
      string overload `Detokenize(str, str)` (`0x00602FBC`–`0x00602FEF`). If the global is null, this step does
      nothing. Tokens: §8.5 (the context advice pushed in step 2 is what the tokens see).
   6. If flag `0x200` (debug show GUID) is set and the key instance ≠ 0: the text becomes
      `sprintf("=text@%08X= %s =adv@%08X=", instance, previousText, GetGUID())`.
4. `PopContextAdvice()` (system slot 19), `SetFlags(text dirty, off)`, close the reader. Return true.

An invalid `advices[tableIndex]` reader skips all field work but still pops the context and clears dirty
(`0x00409348`–`0x00409350`, `0x0040961C`–`0x0040963F`). The localized strings and keys remain as before; this is
different from the empty-field-list early return. The push owns a reference until the pop (§12.1 slots 18–19).
Neither localization keys nor displayed text are in the advice save record (§11.3); the keys are re-read by Init.

Vectors: `message = "text@0x4A1B0001"` with that LTEXT present → message GUID `0x4A1B0001`, text = the detokenized
LTEXT; `title = "Fire alert"` (no key; assuming no LTEXT with instance 0 loads) → title GUID 0, title text
`"***_NOT_IN_TEXT_DATABASE_*** Fire alert"`. Discriminating cases: §17 items 28 and 34.

**Missing/non-string fields (AD16):** Init's field reader and the rebuild use the same conversion contract:
Lua strings supply their C-string bytes; numbers supply the runtime's `sprintf("%.14g", double)` text;
absent/nil, boolean, table, function and userdata supply a null pointer, which the receiving string setter
turns into empty text. There is no Lua `tostring`/`__tostring` call and booleans do not become `"true"` or
`"false"`. Init's reader clears its reusable scratch string before each lookup, so an absent title cannot
reuse the preceding message (`0x004079FA`–`0x00407A37`); both temporary strings are initially empty
(`0x00409111`–`0x0040911E`) and are copied from that reader then parsed as in AD12. Rebuild independently
clears its scratch string and assigns the `ToString(-1)` result (`0x0040938D`–`0x004093D7`). The provider
chain is `cSCLua::ToString` `0x008802D0` → `0x008B7740` → `_lua_tostring` `0x008BABE0`; only numeric type 3
is converted by `_luaV_tostring` (`0x008C5D98`–`0x008C5DB7`, double load and `%.14g`). String assignment
slot 4 is `cRZString::FromChar` `0x0090F50F`: its null branch empties the destination
(`0x0090F515`–`0x0090F53E`). Consequently each such empty field parses to instance 0, exactly like `""`;
fresh Init with both converted fields empty sets no-text and registers neither text field (§4.5). If a rebuild
already has registered fields, a newly nil/boolean field still goes through URL decoding, key-0 localization
and the missing-LTEXT fallback, whose suffix is empty; it does not retain the previous field's text. Embedded
NUL in a Lua string ends the supplied text at that byte (`FromChar` uses `strlen`). These are literal provider
semantics, not evidence that malformed values were exercised in a native game run.

---

## 7. The Lua `game` variables *[compat]*

### 7.1 Conventions

- `SetGlobalValue(name, double)` (system slot 13) does `game[name] = value`. `SetGlobalValue(name, cIGZString*)`
  (slot 12) stores a string. `SetLocalValue(name, value, subject)` (slot 14) does `game[name] = value` and
  `game[name .. "_subject"] = subject` (subject-tag userdata, AddRef'd; a null subject gives a null userdata).
  All three assume `game` exists.
- **L** marks a local value (with subject). Subject "cell (x, z)" = a new subject with `SetLocation(float(x),
  float(z))`, city cells. Subject "object" = `SetObject(occupant)`.
- Conversions (foundations §18: every add, multiply and divide rounds to float32):
  - "×100" = `f32(v × 100.0f)`, then stored as double.
  - "u32" = the unsigned value converted exactly (the 2^32 correction add only applies above 2^31).
  - "int" = signed, exact. "i64" = signed 64-bit, exact.
  - "float" = the callee's float, widened.
- All values are recomputed in place; nothing is smoothed or averaged by the advisors.

### 7.2 System (ticker)

`g_year`, `g_month`, `g_day` (day of month), `g_date` (day number), `g_year_count` (years since the start date): u32.

### 7.3 CityPlanning `update_variables`

| Variable | Source | Conversion |
|---|---|---|
| `ga_mayor_rating` | Aura `+0x20` `GetMayorRating` (int8) | sign-extended int |
| `l_mayor_rating_l` / `l_mayor_rating_h` | Aura `+0x28` `GetMinMayorRating(&p)` / `+0x24` `GetMaxMayorRating(&p)` | int8 value; subject cell (p.x, p.z) |
| `g_city_r_population`, `_c_`, `_i_` | Demand `GetDemand(0x1001 / 0x3001 / 0x4001, 0)` → `QuerySupplyValue` | float |
| `g_city_rci_population` | `f32(f32(I + C) + R)` | float |
| `g_city_r1_population` … `_r3_` | supply of `0x1011`, `0x1021`, `0x1031` | float |
| `g_city_ir/id/im/iht/co3/co2/cs1/cs2/cs3_population` | Demand `+0x24` `GetJobsBySensus(0x4101, 0x4201, 0x4301, 0x4401, 0x3331, 0x3321, 0x3111, 0x3121, 0x3131)` | u32 |
| `g_region_r/c/i_population`, `g_region_rci_population` | as the city ones with region `0x20000` | float |
| `g_city_workforce_population` / `g_region_workforce_population` | supply of `0x2100` (city / region) | float |
| `g_r1_demand`, `g_r2_demand`, `g_r3_demand`, `g_co2_demand`, `g_co3_demand` | region nodes `0x1010`, `0x1020`, `0x1030`, `0x3320`, `0x3330` → `QueryDemandValue` | float |
| `g_workforce_demand` | region `0x2000` → `QueryDemandValue` | float |
| `g_*_active_demand` (r1, r2, r3, co2, co3, id, im, iht, workforce) | region nodes as above plus `0x4200`, `0x4300`, `0x4400`, `0x2000` → `QueryActiveDemandValue` (`+0x30`) | float |
| `g_current_{id,im,iht,co2,co3,r1,r2,r3}_cap` | region node → `GetDemandCap` (`+0x68`, a percentage) | ×100 |
| `g_tax_rate_neutral` | Demand simulator `+0x28` `GetNeutralTaxRate` | float |
| `g_num_rzone_{ld,md,hd}_tiles`, `g_num_czone_{ld,md,hd}_tiles`, `g_num_izone_{r,l,h}_tiles` | ZoneManager `+0x18` zone count of zone type 1..9 | int |
| `g_num_rail_neighbors`, `g_num_road_neighbors`, `g_num_avenue_neighbors` | Neighbors `+0x44` `GetConnectionsOfType(1 / 2 / 8)` list length | u32 |
| `g_num_cities_connected`, `_adjacent`, `_connected_indirectly` | Neighbors `+0x54` / `+0x50` / `+0x58` city set size | u32 |
| `g_city_has_sea` | Seaport developer `+0x3C` `CityHasSea` | 0/1 |
| `g_{r1,r2,r3,cs1,cs2,cs3,co2,co3,ir,id,im,ih}_demand_extrap` | RegionalCity `+0x44` `GetExtrapolatedPopulation(t)` − `+0x24` `GetPopulation(t)` for t = `0x1010`, `0x1020`, `0x1030`, `0x3110`, `0x3120`, `0x3130`, `0x3320`, `0x3330`, `0x4100`, `0x4200`, `0x4300`, `0x4400` | int (32-bit difference) |
| `g_r_demand_extrap`, `g_c_demand_extrap`, `g_i_demand_extrap` | sums of the r / c / i rows above | int |
| `g_city_tile_size` | `f32(SizeX × SizeZ)` (`cISC4City` `+0x164`, `+0x168`) | float, **square metres** (1 048 576 / 4 194 304 / 16 777 216) |
| `g_water_tile_count` | Terrain `+0xD8` (the count of water points) *[inferred name]* | int |

Then the occupant variables (also run on UpdateSelf):

| Variable | Source | Conversion |
|---|---|---|
| `g_parks_funding_p` / `g_landmarks_funding_p` | Budget `+0x70` `GetDepartmentBudget(0x8A4260A6 / 0x8A416368)` → `+0x28` funding percentage | ×100 |
| `g_num_buildings`, `g_num_parks`, `g_num_recreation` | the persisted counters (§5.5) | int |

### 7.4 Finance

| Variable | Source (Budget unless noted) | Conversion |
|---|---|---|
| `g_funds` | `+0x18` `GetTotalFunds` | i64 |
| `g_income_monthly` / `g_expense_monthly` | `+0x40` / `+0x38` | i64 |
| `g_tax_rate_{r_low,r_med,r_high,cs_low,cs_med,cs_high,co_med,co_high,i_resource,i_dirty,i_manufacturing,i_hightech}` | `+0x90` `GetTaxRate(0..11)` | float |
| `g_tax_income` | `+0x4C` | i64 |
| `g_borrowed`, `g_borrowing_limit`, `g_bond_payments_monthly` | `+0xA4`, `+0xA8`, `+0xB4` | i64 |
| `g_nd_income` / `g_nd_expense` | Neighbors `+0x80` / `+0x84` | i64 |

### 7.5 HQ (health and education)

`update_variables`:

| Variable | Source | Conversion |
|---|---|---|
| `g_health_funding_p` / `g_education_funding_p` | department `0x09188F42` / `0x09188F4C` → funding | ×100 |
| `ga_life_exp` | Residential `+0x88` `GetGlobalLE` (uint8 in AL) | int |
| `ga_health` / `ga_education` | Residential `+0x84` / `+0x80` | float |
| `l_tract_hq_l` / `l_tract_hq_h` | Residential `+0xB0` `GetHQMinAndMaxCellCoords` → min / max | float; subject cell of the min / max tract centre |
| `l_tract_life_exp_l` / `_h` | the **same** min / max HQ values and cells *[quirk]* | |
| `l_tract_eq_l` / `l_tract_eq_h` | Residential `+0xAC` `GetEQMinAndMaxCellCoords` | as above |
| `g_num_treated_patients`, `g_health_capacity` | **uninitialised** stack values *[quirk]* | u32 of garbage. FreeCity4 *[choice]*: 0 |

Occupant variables. Lists come from Residential `+0xC4` `GetHospitalBuildings(list, groups)` and `+0xBC`
`GetSchoolBuildings`. Groups: hospitals `0x1513`, clinics `0x1512`, all health {`0x1513`, `0x1512`}, elementary
`0x150F`, high `0x1510`, college `0x1504`, all schools {`0x150F`, `0x1510`, `0x1514`, `0x1504`}, museums `0x1506`,
libraries `0x1505`.

| Variable | Rule |
|---|---|
| `g_num_hospitals`, `g_num_clinics`, `g_num_schools`, `g_num_museums`, `g_num_libraries` | list length, u32 |
| `l_X_funding_l` (X = hospital, clinic, school, museum, library) | minimum over the list (walked **back to front**, strict `<`) of the building's funding fraction (Residential `+0x50`; health purpose for hospitals and clinics, school purpose otherwise), ×100; subject object = that building. Empty list: value `FLT_MAX × 100` = 3.4028234e40 and a null subject *[quirk]* |
| `l_X_grade_l` | minimum of `f32(grade × 100)` from the building's query data (Residential `+0x6C` hospital / `+0x68` school; grade = float property `0x092453CB` / `0x292453C5`). Empty: `FLT_MAX` |
| `g_health_coverage_p` | totals over all health (Residential `+0x3C`): `f32(f32(served / f32(population + 1)) × 100)` |
| `ga_health_grade` | `H = f32(hospitals + clinics)`; H > 0 ? `f32(f32(Σgrade / H) × 100)` : 0 |
| `ga_school_grade`, `ga_museum_grade`, `ga_library_grade` | Residential `+0x38` totals; count > 0 ? `f32(f32(Σgrade / count) × 100)` : 0 |
| `g_num_museum_served`, `g_num_library_served` | served total, u32 |
| `g_education_coverage_p` | as health coverage over all schools |
| `g_num_educational_buildings` | the all-schools count again *[quirk]* |
| `g_num_elem_schools`, `g_num_high_schools`, `g_num_colleges` | list lengths |

### 7.6 Safety

`update_variables` (the department `0x28F55A9F` is fetched first and discarded):

| Variable | Source | Conversion |
|---|---|---|
| `g_fire_strike_chance` | Fire `+0x30` `GetChanceOfStrike` | ×100 |
| `g_population` | Residential `+0x40` | int |
| `l_flammability_h` | Flammability `+0x1C` `GetFlammabilityHotspot(&v, &x, &z)` (v, x, z zeroed first) | int; subject cell (x, z) |
| `ga_crime` | Crime `+0x18` `GetAverageCrime`, **low byte only** *[quirk]* | int |
| — | HistoryWarehouse `+0x30` `RecordHistoryData(0xAA662693, trunc(ga_crime))`: the **only writer** of the GA_CRIME trend | |
| `l_crime_h` | Crime `+0x20` `GetCrimeHotspot` into the **same** v, x, z, not re-zeroed: with no hotspot it repeats the flammability hotspot *[quirk]* | int; subject cell |
| `g_inmate_count` | Police `+0x78` (jail population) | int |
| `g_criminal_count` | Police `+0x7C` (jailed count) | int |
| `g_crime_commited_count` | Crime `+0x1C` `GetMonthlyCrimeCount` | u32 |
| `g_arrest_count` | Police `+0x88` | int |
| `g_police_strike_chance` | Police `+0x2C` `GetChanceOfStrike` | ×100 |
| `g_school_strike_chance` / `g_health_strike_chance` | Residential `+0x28` / `+0x2C` | ×100 |

Occupant variables:

| Variable | Source |
|---|---|
| `g_fire_funding_p`, `g_police_funding_p` | department `0x28F55A9F` / `0xA2963983` funding ×100 |
| `g_fire_station_count`, `g_police_station_count`, `g_jail_count` | Fire `+0x5C`, Police `+0x60`, `+0x8C` (u32) |
| `ga_flammability` | Flammability `+0x20` (uint8) |
| `ga_fire_coverage` / `g_fire_coverage_p` | Fire `+0x4C` (float) / `+0x50` ×100 |
| `ga_police_coverage` / `g_police_coverage_p` | Police `+0x48` / `+0x4C` ×100 |
| `g_jail_capacity` | Police `+0x74` (int) |
| `l_fire_funding_p` / `l_police_funding_p` | minimum station funding (Fire `+0x58` list, `+0x20` per station; Police `+0x54`, `+0x1C`), walked front to back, strict `<`, starting from **100.0**: ×100, subject object = that station. No stations: 10000 and a null subject *[quirk]* |
| `l_fire_station_no_roads` / `l_police_no_roads` | 1 with subject = the first station whose lot exists and has no road access (Lot `+0x70`), else 0 |

Messages (§5.5): health strike `0x077ACCD2` and school strike `0x077ACCD0` set `g_health_strike` / `g_school_strike`
and push `0xAA60FACA`/`0x2A60FACF`, `0xEA60FABF`/`0x2A60FAC7`; jailbreak `0x2A614891`/`0x2A614892` → `g_jailbreak`,
events `0xCA616650`/`0x6A61665A`; fire strike `0x68F54CE1`/`0x68F54CE2` → `g_fire_strike`, events
`0x0A31A314`/`0x6A31A319`; police strike `0xA8DAF511`/`0xA8DAF512` → `g_police_strike`, events
`0x6A31A2FE`/`0x8A31A303`; transit strike `0x69247DC4`/`0x69247DC5` → `g_transit_strike`, event = the message ID;
nuclear explosion `0xAA552F77` → event `0xAA552F77`; zoo escape `0xAA667CE2` → event `0x2A667FC3`; pipe burst
`0xEA1F0DCD` → event `0x4A31CF15` (subject cell (data1, data2)) unless a user disaster is running; emergency route
blocked `0xCA8DCDB4` (data2 > 2) → `0xCA3FCEFA` (data1 0) or `0xCA3FCEF4` (data1 1).

`DisasterPeriodStarted` `0xCA1B1E64`: `g_disasters_in_progress` := 1; the disaster type from the instance
(`0x621CDA33` fire, `0xCA380B06` riot, `0x4A1332E8` pipe burst); `g_disasters_fire_started`, `_fire_ended` := 0,
`g_disasters_riot_started`, `_riot_ended`; for fires the cause is obtained by QI `49B43F58` followed by its
**virtual slot 14 (`+0x38`) GetCause**, not an object-field read. Generic instance QI `22223BA4` precedes subject
creation; the type comes from generic GetManager followed by manager GetID. The counted pointer identity,
typed slots, reference ownership and exact call order are disasters.md §3.2.1 (DSEX33-DS01).
Causes 1, 3 are user; 2 arson; 4 effects (and user); 5 flammability →
`g_disasters_fire_cause_{arson,effects,flammability}`. Any type other than fire, riot or pipe burst counts as a user disaster. Event `0xCC3298DC XOR
type` for a user disaster, else `0xCA1B1E64 XOR type`, subject cell (data2, data3); then, if not a user disaster,
event `0xCA3C0412`.
`DisasterPeriodEnded` `0xAA1B2002`: `g_disasters_in_progress` := 0; from the disaster layer's statistics:
`g_disasters_damage_building_count` (u32), `g_disasters_damage_cost` (i64),
`g_disasters_{police,fire}_dispatch_effectiveness` (×100); fire / riot ended flags; event `0xAA31A2F7`.

### 7.7 Traffic

Most values come from the traffic simulator's statistics block (Traffic `+0x88` `GetTrafficStats`, a pointer into
the traffic simulator; traffic spec). Per network (road, rail, street, avenue, subway, elevated, monorail, bridge):
`ga_<net>_congestion` (average ×100), `g_<net>_congestion_tile_count` (u32), `l_<net>_congestion_h` (worst value
×100, **not** truncated, subject cell). Per station kind (bus, train, subway, elevated, monorail):
`ga_<kind>_station_utilization_p` (×100), `l_<kind>_station_utilization_pl` (`trunc(f32(v × 100))`, subject cell).
Also `g_road_funding_p`, `g_masstransit_funding_p` (×100), `g_transit_strike` (0/1), `ga_freight_trip_length` (u8).

| Variable | Source |
|---|---|
| `ga_airport_efficiency` / `ga_seaport_efficiency` | port developer `+0x14` `GetAvgEfficiency` ×100. With no port the average is 1.0 → 100 *[quirk]* |
| `l_airport_no_road_access` / `l_seaport_no_road_access` | 1 with subject cell of the first port lot without road access (scanning from the back), else 0 and a null subject |
| `l_road_connection_congestion_h` / `l_avenue_connection_congestion_h` | maximum (strict `>` from 0) of the congestion map (Traffic `+0x28`, `GetCellValue`) at each road / avenue connection's inner cell (Neighbors `+0x44`, connection `+0x1C`); subject cell. *[quirk]* The avenue pass does not reset the location: with no congested avenue cell its subject is the road maximum's cell |
| `g_rail_neighbor_connection_count` | rail connection count (u32) |

Occupant variables: station counts, `g_train_freight_depot_count`, `g_num_rapidtransit_transitions`, `g_pothole_count`,
`g_<net>_tile_count` (road, rail, subway, highway, street, avenue, elevated_rail, monorail, dirtroad,
groundhighway); `g_airport_count` and `g_{small,medium,large}_airport_count` and the `_s1`/`_s2`/`_s3` stage counts
from `GetNumPorts(sizeMask, stageMask)` (port `+0x24`; sizes 2/4/8, all = 0xE; stages 1/2/4, all = 0x1F);
`g_seaport_count`, `g_seaport_s1..s3_count`; `g_seaport_volume` (`+0x34`, u32); `g_car_zot_count` (§5.5).
*[quirk]* `ga_rail_congestion` and the elevated and monorail station values are written twice (same value).

### 7.8 Utility

| Variable | Source | Conversion |
|---|---|---|
| `g_power_funding_p` | Power `+0xC0` | ×100 |
| `g_power_production_capacity` | Power `+0x30` `GetMonthlyProduction` (last month's production, not capacity *[quirk]*) | int |
| `g_power_consumed`, `g_overworked_power_plant_count`, `g_unpowered_building_count` | Power `+0x44`, `+0x6C`, `+0x54` | int |
| `g_power_strike` / `g_power_strike_chance` | Power `+0x9C` (AL) / `+0xA0` (reads the argument through EAX) | 0/1, ×100 |
| `g_power_imported` / `g_power_exported` | RegionalCity `+0xE0` / `+0xD8` (commodity 1) | double as returned |
| `g_water_funding_p` | department `0xAA4014B4` (no null check) | ×100 |
| `g_water_production_capacity` / `g_water_consumed` | Plumbing `+0x38` / `+0x3C` (low 32 bits) | int |
| `g_water_imported` / `g_water_exported` | RegionalCity `+0xE0` / `+0xD8` (commodity 2) | double |
| `g_pollution_funding_p` | department `0x6A42A8F5` | ×100 |
| `g_incinerator_capacity_daily`, `g_waste_to_energy_capacity_daily` | Pollution `+0x7C` / `+0x80` × 30 (int32, wraps) *[quirk: monthly × 30]* | u32 |
| `g_recycling_center_effect_p` | Pollution `+0x84` | ×100 |
| `ga_air_pollution`, `ga_water_pollution`, `ga_garbage_pollution` | Pollution `+0x28`, `+0x2C`, `+0x30`, max(v, 0) | int |
| `g_uncollected_garbage` | Pollution `+0x60` | int |
| `g_landfill_capacity` / `g_available_landfill_capacity` | LandfillDeveloper `+0x28` `GetLandfillUsage(&u, &cap)`: cap / cap − u | u32 |
| `g_garbage_produced`, `_recycled`, `_to_energy`, `_to_landfill` | Pollution `+0x8C`, `+0x98`, `+0xA0`, `+0xA4` | i64 |
| `g_garbage_imported` / `g_garbage_exported` | RegionalCity **`+0xD8` / `+0xE0`** (commodity 3; the slots are swapped on purpose, neighbors.md §8.2) | double |
| `l_air_pollution_h`, `l_water_pollution_h`, `l_uncollected_garbage_h` | Pollution `+0x44` / `+0x48` / `+0x4C` worst tract, max(v, 0) | int; subject cell |

Occupant variables: `g_power_plant_count` (Power `+0x60`), `g_power_plant_count_{coal,gas,oil,wind,solar,nuclear,
waste,fusion}` (`+0x5C` type 1, 3, 6, 9, 7, 5, 8, 2), `g_aged_power_plant_count` (`+0x70`), `g_power_pole_count`
(`+0x68`); `l_power_plant_funding_pl` / `l_power_plant_age_h` (Power `+0x78` `FindPowerPlants`, funding preset 1.0,
age preset 0; subject object = the plant or null); `g_water_source_count` (Plumbing `+0x30`), `g_water_pipe_count`,
`g_water_distressed_pipe_count`, `g_water_burst_pipe_count` (`+0x48`, `+0x4C`, `+0x50`), `g_watered_building_count`
/ `g_unwatered_building_count` (`+0x54` / `+0x58`), `g_water_pollution_pump_shutdown_count` (`+0x34`);
`l_water_building_funding_pl` / `l_water_building_age_h` (Plumbing `+0x6C`); `g_incinerator_count`,
`g_waste_to_energy_building_count`, `g_water_treatment_plant_count`, `g_recycling_center_count` (Pollution `+0xA8`,
`+0xAC`, `+0xB4`, `+0xB0`).

Neighbour-deal variables:
- `g_nd_count_{buy_water,sell_water,buy_power,sell_power,export_garbage,import_garbage}`: Neighbors `+0x78`
  `FindDeals(type)` list length for `0x4773FFE2`, `E3`, `E0`, `E1`, `E4`, `E5` (u32).
- `g_nd_connection_present_{water,power,garbage}`: over `GetAllConnections` (`+0x40`), pipe → water, power pole →
  power, rail or road → garbage *[quirk: not highway or avenue]*.
- `g_nd_can_{buy_water,buy_power,export_garbage,sell_water,sell_power,import_garbage}`: from `ProposeDeals(list,
  false)` (`+0x64`). *[quirk]* This call has side effects: it refreshes the regional commodity statistics and
  **draws from the neighbours' process-wide random generator** once per proposal (neighbors.md §7.1).

### 7.9 Events pushed by the advisors

CityEstablished → `0xAA31A210`; Safety and Utility messages (§5.5, §7.6); the Utility deal events (§6.3). Other
pushers: the building development simulator (event `0x4A64A9CC` to the CityPlanning advisor on building changes
*[inferred details]*), the port developers (`SendAdvisorMessage`), the city situation manager, MySim, and Lua
`game.trigger_event` (§8.3).

---

## 8. Lua functions

### 8.1 Tags and the gc method

Tag 0 = advice-subject userdata, tag 1 = string userdata (`cIGZString`). Each userdata holds one AddRef'd pointer.
The gc method (shared by both tags) checks that the Lua state is the system's, the value is userdata and its tag is
tag 0 or 1, then releases the pointer.

### 8.2 Registration (`register_api(bool global)`)

Every extension is `{name, function, isGlobal}` in one list built by two static collectors (38 simulation entries
and 9 UI entries). `register_api(true)` registers the global entries as globals; `register_api(false)` registers the
others into the table `game` (which the scripts must have created; otherwise the protected-less `SetTable` raises a
Lua error outside any protected call, which goes to the panic path and ends the process with `exit(1)`,
`0x008BB830`, lua-host.md §4.2). Registration leaves the global `_tf_` pointing at the last function *[quirk]*
(lua-host.md §8.2).

**Vector order (AD7, verified).** The list is one process-wide vector (begin `0x00B207CC`, end `0x00B207D0`, capacity
`0x00B207D4`), 12-byte entries `{name, function, isGlobal byte}`, appended by the two collector constructors. They run
as C++ static initialisers from the CRT table: `0x00B07030` → `0x00A6FF20` → simulation collector `0x0040B8D0`, then
`0x00B07034` → `0x00A6FF30` → UI collector `0x0040CAB0`. So the **simulation entries come first**, then the UI
entries, each in its constructor's source order:

| # | Simulation collector (38) | # | Simulation (cont.) | # | UI collector (9) |
|---|---|---|---|---|---|
| 1 | `advice_info` | 20 | `advice_subject` | 39 | `tool_plop_network` |
| 2 | `trend_value` | 21 | `trigger_event` | 40 | `tool_plop_zone` |
| 3 | `trend_slope` | 22 | `building_is_group` | 41 | `tool_plop_building` |
| 4 | `trend_count` | 23 | `event_building_distance` | 42 | `tool_plop_flora` |
| 5 | `camera_jump` | 24 | `mysim_distance_to_closest_building` | 43 | `tool_button` |
| 6 | `camera_zoom` | 25 | `mysim_distance_to_closest_landmark_or_reward` | 44 | `window_budget` |
| 7 | `ordinance_is_on` | 26 | `mysim_local_school_grade` | 45 | `window_map` |
| 8 | `ordinance_set_off` | 27 | `mysim_local_hospital_grade` | 46 | `window_query` |
| 9 | `ordinance_set_on` | 28 | `mysim_dispatch_tool` | 47 | `window_graph` |
| 10 | `ordinance_is_available` | 29 | `event_disaster_distance` | | |
| 11 | `ordinance_name` | 30 | `random_chance` | | |
| 12 | `ordinance_description` | 31 | `nd_cancel` | | |
| 13 | `ordinance_enact_fee` | 32 | `reward_instance_count` | | |
| 14 | `ordinance_repeal_fee` | 33 | `unlock_reward_building` | | |
| 15 | `ordinance_monthly_cost` | 34 | `get_reward_building_state` | | |
| 16 | `difficulty_level` | 35 | `is_tutorial` | | |
| 17 | `event_feedback` | 36 | `_combine_guids` (global) | | |
| 18 | `expire_advice` | 37 | `_dofile_from_resource` (global, `0x005FC020`) | | |
| 19 | `retire_advice` | 38 | `print` (global, `0x005FB850`) | | |

Only entries 36–38 have isGlobal set. Note the UI order ends `window_query`, `window_graph` (the §8.3 table lists
them in a different order). `register_api(g)` (`0x00418940`) walks the vector in this order; for each entry it
pushes the target table (globals when g and isGlobal; the global `game` when neither; otherwise skips the entry,
detected by an unchanged stack top), then sets `_tf_` to the function as a global, does `table[name] = _tf_` and
pops the table. Consequences: after `Init`'s `register_api(true)` the global `_tf_` is `print` (`0x005FB850`); after
every `reload_data`'s `register_api(false)` it is `window_graph` (`0x0040C890`, the UI collector's last entry).
A replacement must register in this order, or at least leave `_tf_` = `window_graph` after each reload *[compat]*.
Duplicate names cannot occur (all 47 differ).

### 8.3 `game.*` functions by subsystem *[compat]*

"ctx" is the context advice (§6.7). Numbers are truncated toward zero where an integer is needed. A bad argument
usually returns no value.

| Group | Function | Behaviour |
|---|---|---|
| History | `trend_value(type, months)` | HistoryWarehouse `+0x20` value at day `now − 30·months`; 0 if missing |
| | `trend_slope(type, months)` | `+0x18` trend slope m over [now − 30·months, now]: `f32(atan(m) × 63.6618f)` (≈ ±100); 0 if missing |
| | `trend_count(type, months)` | `+0x14` count over [now − 30·months + 1, now] |
| Ordinance | `ordinance_is_on(id)`, `ordinance_is_available(id)` | Ordinance `+0x20` / `+0x1C`: 1 or nil |
| | `ordinance_set_on(id)`, `ordinance_set_off(id)` | `+0x34 SetOrdinanceOn(id, true/false)` |
| | `ordinance_enact_fee`, `ordinance_repeal_fee`, `ordinance_monthly_cost` | ordinance `+0x2C` / `+0x30` / `+0x50` (i64 as double); 0 if missing |
| | `ordinance_name(id)`, `ordinance_description(id)` | string userdata (tag 1) or nil |
| Advice | `event_feedback([n])` | post a message whose type is ctx's event ID, IGZUnknown = ctx's subject object, data1 = n (default 0) |
| | `expire_advice()` | ctx `SetFlags(expire request, on)` |
| | `retire_advice()` | ctx's advisor `RetireAdvice(ctx)` |
| | `advice_subject()` | a **clone** of ctx's subject (subject `Clone`, slot 13) as tag-0 userdata |
| | `trigger_event(guid)` | system `PushEvent(guid, null)` |
| | `difficulty_level()` | 0/1/2, or −1 |
| | `random_chance(p)` | §9: 1 if a uniform draw in [0, 100) is < p, else nil |
| | `is_tutorial([guid])` | RegionalCity `+0x104` tutorial GUID compare |
| | `advice_info()` | debug message box |
| Subjects | `building_is_group(subj, group)` | the subject object as occupant → `IsOccupantGroup` |
| | `event_building_distance()` | subject data1 (u32), only for event `0xCA4E8986` (MySim building added) |
| | `event_disaster_distance()` | subject data1 (u32) |
| | `nd_cancel(subj)` | the subject object as neighbour deal → `Cancel` (`+0x88`) |
| MySim | `mysim_distance_to_closest_building`, `…_landmark_or_reward`, `mysim_local_school_grade`, `mysim_local_hospital_grade`, `mysim_dispatch_tool` | ctx's advisor as a MySim (§14.3) |
| Rewards | `reward_instance_count(id)` | civic status building count; if no status, the CityPlanning landmark count for `id` |
| | `get_reward_building_state(id)` | 1 if no status, 2 if status bit 2; **otherwise returns its argument** *[quirk]* |
| | `unlock_reward_building(guid)` | status := 2 (whole byte), `UpdateConditionalBuildingStatus` |
| UI | `tool_plop_network`, `tool_plop_zone`, `tool_plop_building`, `tool_plop_flora`, `tool_button`, `window_budget`, `window_map`, `window_graph`, `window_query` | the 9 entries of the UI collector, §8.3.1 |
| Camera | `camera_jump`, `camera_zoom` | simulation-collector entries (`0x40AF90`, `0x40B1F0`), not UI-collector ones: move the camera to a subject / set the zoom, §8.3.2 |

Globals: `_combine_guids(a, b)` = `a XOR b` as a signed int32 (−1 on bad arguments); `_dofile_from_resource(name)`
(Lua host); `print(...)` (no output).

#### 8.3.1 The UI collector's 9 entries *[compat]*

The UI collector (`cLUAExtensionsCollectorUI` constructor `0x40CAB0`, run by the static initializer `0xA6FF30`, after
the simulation collector `0x40B8D0` from `0xA6FF20`) appends 12-byte records `{const char* name, function,
bool isGlobal}` to the **same** vector as the simulation collector (begin/end/capacity `0x00B207CC` / `0x00B207D0` /
`0x00B207D4`; growth helper `0x626240`). All 9 UI records have `isGlobal = false`, so they go into `game`. In
vector order:

| # | Name | Function | Item / window ID | Reads `0x00B207C0` |
|---|---|---|---|---|
| 1 | `tool_plop_network` | `0x40C720` | `0x4A2B2467` | no |
| 2 | `tool_plop_zone` | `0x40C740` | `0x4A2B2466` | no |
| 3 | `tool_plop_building` | `0x40C760` | `0x6A2B245F` | no |
| 4 | `tool_plop_flora` | `0x40C780` | `0xEA5AD08D` | no |
| 5 | `tool_button` | `0x40C8B0` | (message `0x287259F6`) | yes: slot 20 |
| 6 | `window_budget` | `0x40C870` | `0xEA5BDFBC` | yes: slot 20 |
| 7 | `window_map` | `0x40C850` | `0x6A5BDFCD` | yes: slot 20 |
| 8 | `window_graph` | `0x40C890` | `0x6A720993` | yes: slot 20 |
| 9 | `window_query` | `0x40C9B0` | — | yes: slots 20 and 22 |

**ABI.** Each of the 9 is a plain Lua 5.0 `lua_CFunction`: cdecl, one stack argument `lua_State*`, the caller pops,
`int` in EAX, and every path returns **0** (no Lua results). Rows 1–4 call the shared helper `0x40C680` and rows 6–8
the helper `0x40C7A0`; both helpers are non-standard (`lua_State*` in EAX, the ID as one cdecl stack argument) and
are never registered themselves. None calls `lua_*` directly: each takes the host from the 4 bytes before the state,
QIs it for `cISCLua` `0xEA10C4A2` without checking the result (lua-host.md §1.3), uses `cISCLua` slots only and
releases the host before returning. "The 3D view" is the global `0x00B43DC4` (`cISC4View3DWin*`); each entry skips
its action when it is null. A "message" below is 16 bytes on the stack, four dwords at `+0`, `+4`, `+8`, `+0xC`.

- **`tool_plop_*(n)`** (`0x40C680`): if `GetTop() ≠ 0`, `Type(-1) = 3` (number) and the 3D view is set: build the
  message `{3, 0x49D993D4, item ID, trunc(ToNumber(-1))}` (x87 `_ftol2`, toward zero) and pass it to
  `view.AsIGZWin()` (view slot 3) → `cIGZWin` slot 3 `DoMessage(msg)` (ui-windows.md). The result is ignored.
- **`window_budget` / `window_map` / `window_graph(n)`** (`0x40C7A0`): first, unconditionally, call the advisor
  system's slot 20 `GetContextAdvice` on the pointer in `0x00B207C0` (no null test) and discard the result. Then, if
  `GetTop() ≠ 0`, `Type(-1) = 3` and the 3D view is set: message `{window ID, trunc(ToNumber(-1)), 0, 0}` to
  `view.AsIGZWin()` → `DoMessage`, result ignored.
- **`tool_button(n)`** (`0x40C8B0`): the same discarded slot 20 call on `0x00B207C0`. Then only if `GetTop() = 1`
  **exactly**, `Type(-1) = 3`, `n = trunc(ToNumber(-1)) ≠ 0` and the 3D view is set: message `{3, 0x287259F6, n, 0}`;
  QI `view.AsIGZWin()` for `0xC6AE7085`; if that succeeds, call its slot 3 with (`view.AsIGZWin()` fetched again,
  `&message`) (the shape of a win proc's `DoWinProcMessage(cIGZWin*, cGZMessage&)` *[inferred]*), ignore the result
  and release the interface.
- **`window_query(subject)`** (`0x40C9B0`): load the system pointer from `0x00B207C0`, call its slot 20 and discard
  the result. Then require, in order: `GetTop() ≠ 0`; `Type(-1) = 7` (userdata); `ToUserData(-1)` non-null; the
  system's slot 22 `GetDataTag(0)` equal to `Tag(-1)` (the subject tag, §8.1); `ToUserTag(-1)` (the subject) non-null;
  the subject's slot 8 `GetObjectA()` non-null. Then call `GetObjectA()` again, QI that object for `0x26BB767E` into a
  holder and, on success, call the static `cSC4ViewInputControlQuery::DoQueryDialog` `0x4D3C30` (cdecl, one
  argument: the QI'd pointer) and release the holder. Any failed test returns 0 with no effect.

**Under a replacement system.** Rows 1–4 do not touch the advisor system. Rows 5–8 need a valid
`cISC4AdvisorSystem*` in `0x00B207C0` (a null crashes) but no result from it; row 9 also needs slot 22 to return the
tag the subject userdata carries. No row reads a system, advisor or advice member directly. So a shim may register
the original addresses with `register_api`'s `game` table only if it also keeps a `cISC4AdvisorSystem*` in
`0x00B207C0` (§14.2) *[choice]*; or it may reimplement them through the public interfaces above with the same
messages. Neither the null/no-op route nor dropping an entry is the original.

**Other registrars.** The vector is read only by the system's `register_api` `0x418940` (`0x41894F`, `0x418957`,
`0x4189EC`) and freed by the atexit destructor `0xA79940`; a byte scan of the executable for the three vector
addresses finds no other code (2026-10-21). The name strings of the 9 UI entries and of `camera_jump` /
`camera_zoom` are referenced only by their collector constructor. So nothing else (app, tutorial, MySim) registers
any of these entries into a Lua context.

#### 8.3.2 `camera_jump` and `camera_zoom` *[compat]*

**Registration.** Both are records of the **simulation** collector (constructor `0x40B8D0`), records 5 and 6 of that
vector after `advice_info` and three others: `{"camera_jump" 0x00A80F80, 0x40AF90, isGlobal = false}` (`0x40BA53`)
and `{"camera_zoom" 0x00A80F74, 0x40B1F0, false}` (`0x40BAAE`); the flag byte is `BL`, zero from `0x40B8E3` on. So
they are `game.camera_jump` and `game.camera_zoom`. An earlier text of §8.3 put them in the UI collector; the bytes
place them in the simulation one (AD5, AD6).

**ABI and host.** The same as §8.3.1: cdecl `lua_CFunction`, one stack argument `lua_State*`, the caller pops,
every path returns **0** in EAX (no Lua results, nothing pushed or popped: both only read index −1). The host is the
pointer in the 4 bytes before the state, QI'd for `cISCLua` `0xEA10C4A2` into a holder without a result test, and
released (slot 2) on every exit; all Lua access goes through `cISCLua` slots (lua-host.md §1.3). "The camera
control" is the renderer global `0x00B43DD0` (`cISC43DRender`) slot 16 `GetCameraControl()` (view3d.md).

**`camera_jump(subject)`** (`0x40AF90`, 608 bytes). In order:

1. Load the advisor system from `0x00B207C0` (no null test) and call its slot 20 `GetContextAdvice`; the result is
   discarded (as in §8.3.1 rows 5–9).
2. Require: `GetTop() > 0` (signed, slot 6); `IsUserData(-1)` (slot 73); `ToUserData(-1)` non-null (slot 27); the
   **same** system's slot 22 `GetDataTag(0)` equal to `Tag(-1)` (slot 17), i.e. the argument is advice-subject
   userdata (§8.1); the renderer `0x00B43DD0` non-null; its `GetCameraControl()` non-null. Any failure goes to the
   error box below.
3. `s := ToUserTag(-1)` (slot 28), the boxed `cISC4AdviceSubject*` (§12.4). **Not** null-tested: a subject
   userdata whose box holds null crashes.
4. A local `vec3 p` is first filled with the float at `0x00B207C8` (0.0 in the image; nothing else is known to write
   it) in all three components.
5. If `s.HasLocation()` (slot 6): with the terrain `t` = `0x00B43CF4` (`cISTETerrain`, not null-tested):
   `p.x := f32(t.CellWidth() × s.GetLocationX())`, `p.z := f32(t.CellWidth() × s.GetLocationZ())` (slots 6, 4 and
   5; each product is one x87 `FMUL` of the two float results, stored as float), then
   `p.y := t.GetAltitude(p.x, p.z)` (slot 13, terrain.md §4.1), stored as float. Call the camera control's
   position routine `0x7CD810` with `&p` (below), release the host, return 0. CellWidth is 16.0, so a subject at
   cell (10, 20) gives `p = (160, altitude(160, 320), 320)`.
6. Else, if `s.GetObjectA()` (slot 8) is non-null: call `GetObjectA()` again and QI it for the occupant IID
   `0x26BB767E` into a holder. On success: occupant slot 8 `GetPosition(&p)` (occupant-manager.md), then
   `0x7CD810(&p)`, release the holder and the host, return 0. On QI failure: release the holder if set and take the
   error box.
7. Error box (every failed test of 2 and 6, and a subject with neither location nor object): build two `cRZString`s
   (`0x407FA0`, from each literal's begin and end pointers), text **"The subject for camera jump is unknown..."** (`0x00A80B50`)
   and caption **"game.LUAExtension_camera_jump()"** (`0x00A80B30`), and call `gAdviceMessageBox` `0x4179A0`
   (cdecl, `(caption*, text*)`). That helper gets the window manager (framework service `0xA417445E`, IID `0x5A4`;
   the holder is used without a null test) and calls its slot 44 (`+0xB0`) with `(text, caption, 0, 1, 0)`; the
   modal-window meaning is *[inferred]* from the name and §6.8.1. It is **not** gated on a debug setting. Then free
   the strings, release the host, return 0.

The position routine `0x7CD810` (thiscall on the camera control, one argument `const vec3*`, an
`cSC4CameraControl` method by its callees) returns with **no change** if any component is NaN, below −100000.0
(`0x00ABAC9C`) or above 100000.0 (`0x00ABACA8`; both bounds inclusive-accepted); otherwise it stores the point at `+0x44..+0x4C`, zeroes
`+0x50..+0x58` and, when `+0x0C` is set, calls `UpdateCameraPositionParams` `0x7CC290`, `SC4ViewUtilities::
UpdateCameraPosition` `0x7F83F0` (when `0x00B43DD8` is set) and `UpdateScrollBounds` `0x7CCC60`, then posts camera
messages (`0x4A9F2FFE`, `0xA6B79611`). Its full behaviour belongs to the view spec; for a caller it is "look at
`p`".

**`camera_zoom(level)`** (`0x40B1F0`, 317 bytes). In order:

1. `0x00B207C0` slot 20 `GetContextAdvice`, result discarded (no null test).
2. Require `GetTop() > 0`, `IsNumber(-1)` (slot 14), the renderer
   non-null and `GetCameraControl()` non-null.
3. Call `cSC4CameraControl::SetZoom` `0x7CDC50` (thiscall, one float stack argument) with `f32(ToNumber(-1))`: the
   double from slot 23 rounded to float by `FSTP` (round to nearest). No truncation to an integer and **no range
   check here**: the range handling is SetZoom's own (its first test compares with the control's `+0xF4`). Release
   the host, return 0.
4. Any failed test of 2: the error box as above with text **"'zoom_level' was not specified or valid range of
   zooms."** (`0x00A80B9C`) and caption **"game.LUAExtension_camera_zoom()"** (`0x00A80B7C`).

*[quirk]* The zoom message speaks of a "valid range", but an out-of-range number never shows it: only a missing
or non-numeric argument, or a missing renderer or camera control, does.

**Under a replacement system.** Neither entry reads a system, advisor, advice or city member. Both call slot 20 on
the pointer in `0x00B207C0` with no null test, so a null there crashes; `camera_jump` also needs slot 22 to return
the subject tag its userdata carries. So the original addresses may be registered under a replacement only if a
`cISC4AdvisorSystem*` with working slots 20 and 22 stays in `0x00B207C0` (§14.2) *[choice]*; or the replacement
reimplements both through the interfaces above (terrain, renderer, camera control, occupant, window manager). With
no city view (renderer null) both show the error box rather than doing nothing.

### 8.4 What reads the variables

Only the Lua scripts: triggers, reward conditions, text tokens and the MySim scripts. Shipped reward conditions use
`ga_mayor_rating`, populations and grades (§6.6).

### 8.5 Detokenizer (UI)

`#token#` text in advice texts (`##` is a literal `#`): `link_id`, `sit_link_id`, mission tokens (city situation data), `advisor`,
`building_initial_cost`, `building_monthly_cost`, and Lua expressions evaluated as `_dasd_=(expr)` and formatted by a
type prefix (`s@`, `m@`, `d@`, `t@`, `p@`). A token nothing resolves ends up as the bare token text (the
`UNKNOWN TOKEN` text is built and then overwritten, `0x00600C20`). The host side is in lua-host.md §10.

---

## 9. Random numbers *[compat]*

| Draw | Generator | Draws |
|---|---|---|
| `game.random_chance(p)` | the system's own `cRZRandom`, seeded with `0xFFFFFFFF` (wall clock) in the constructor; `RandomDoubleRangeUniform(0, 100)` (foundations §6.1) | 1 per call: `u = f32(int32(mul)/2^32 + 0.5)`, `r = f32(u × 100)` |
| Utility deal variables | the neighbours' process-wide `cRZRandom` through `ProposeDeals` | 1 per proposal (neighbors.md §7.1) |

No other draw. Seed handling follows foundations §6.3 *[choice]*. `GetRandom()` (slot 9) returns the address of the
system's generator.

---

## 10. Messages posted *[compat]*

| ID | Name | How | Data |
|---|---|---|---|
| `0x8A09FA64` | AdviceAdded | post | IGZUnknown = advice |
| `0x8A09FA65` | AdviceRemoved | send | IGZUnknown = advice |
| `0x8A09FA66` | AdviceChanged | send | IGZUnknown = advice |
| `0x8A09FA67` | AdviceRemovedAll | send | IGZUnknown = advisor |
| `0x8A09FA6C` | AdviceCommand | post | IGZUnknown = advisor, data1 = command ID |
| `0x8A09FA68` | AdvisorChanged | post | IGZUnknown = advisor |
| `0x8A09FA69` | AdvisorRemoved | send | IGZUnknown = advisor |
| `0x8A09FA6A` | AdvisorAdded | post | IGZUnknown = advisor |
| `0x8A09FA6B` | AdvisorRemovedAll | send | — |
| `0xCA8F3DB6` | UpdateSelf | post to the advisor's own target | data1, data2 (§5.4) |
| `0xCA5501F2` | ticker | post (broadcast) | — |
| ctx event ID | `game.event_feedback` | post | IGZUnknown = subject object, data1 |
| `0xEA917750`, `0x4A91DD6E`, `0x4A92ECEF`, `0x8A917759` | debug | post | data1 |

"send" = `MessageSend` (synchronous), "post" = `MessagePost(msg, 0)` (foundations §5).

---

## 11. Persistence *[compat]*

Records use the stream methods and refs of foundations §2. Each object writes only its own record; references are
written with `SC4WriteGZSerializable` and read with `SC4ReadGZSerializable(stream, iid, &p, 0)`.

### 11.1 System (`0x4A0DD616`), version 1

```
u16  1
u32  advisorCount
advisorCount × { ref advisor ; s32 advisorID }      // map order (ascending ID)
```

`Read`: version must be 1, else false. Each advisor is read with IID `0xCA09F544` and stored as `map[id]`. Then
loadedFromSave := !error; returns that. Nothing else is saved: subscriptions, thresholds and the Lua state are
rebuilt by `Init`.

### 11.2 Advisor (`0xEAC937B4` and every subclass CLSID), version 3

```
u16  3
s32  tableIndex
str  caption                    // GZStr
str  personalName
s32  monthCounter
s32  currentDay
s32  advisorID
s32  cursor
u32  n
n × { ref advice ; s32 expiry ; s32 lastMonth ; s32 nextDay ; s8 active ; s32 eventID }
u32  m
m × { s32 eventID ; ref subject }                   // the event queue
```

`active` is `CheckFlags(2)`; `eventID` is the advice's `GetEventID()`. `Read`: version must be 3. Advices are read
with IID `0x8A09F5F5`; an `active` advice is put in the active map; a non-zero event ID goes into the event
multimap. Subjects are read with IID `0x6A26D050`. Returns `!GetError()`. On `Init` the active state is cleared
again (§4.4).

### 11.3 Advice (`0x8A09F5F4` and subclass CLSIDs), version 4

```
u16  4
s32  tableIndex
u32  guid
u32  flags
s32  advisorID
s32  priority
s32  frequency
s32  timeout
s32  mood
u32  effectCount
effectCount × u32 effectID
ref  eventSubject
```

`Read`: the version is **not checked**. The effect list is read only when version > 3, and **appended** to the
current list *[quirk]*. The subject is released first and read with IID `0x6A26D050`. Then text dirty is set, the
loaded byte := !error, and that is returned. The neighbour-deal advice (`0x6A3848DB`) is the one subclass with its
own record (corrected): `u16 2`, this advice record, then `s32 dealType` (`nd_type`); its Read requires version 2
(`Write` `0x00420F40`, `Read` `0x00420F80`; all 10,842 corpus entries; save-records.md §7.2) *[compat]*.

#### 11.3.1 CitySituationAdvice record and factory identity (AD17)

`cSC4CitySituationAdvice` (`0x8BE3753B`) writes **exactly** the version-4 advice record above: no subclass
version, prefix, suffix or city-situation fields. Its serializable vtable `0x00A84508` uses the very same Write
`0x00408090` and Read `0x004082F0` entries as plain advice's `0x00A80884`; only the CLSID getter differs
(`0x00423000` returns `0x8BE3753B`). The constructor installs that serializable table at object +`0x10`
(`0x0042349A`), after constructing the base advice at object +4 (`0x00423475`–`0x00423480`). Thus the identical
serializer addresses also address the same base fields. Serializable QI adjusts back by `0x10`
(`0x004238D0`); subclass QI delegates to base advice at +4 (`0x0042303B`–`0x00423042`). The game's COM director
registers its own factory (`0x00423DD0`, registration in `0x00460090`), so a reference read with the advice IID
must retain this object rather than fail resolution.

A record-only implementation may register this CLSID with its plain-advice record adapter (`AdviceKind::Plain`
in the current terminology), provided it preserves the captured CLSID and object/reference identity on output,
supports the advice IID and uses the base Read semantics, including text dirty. Writing the object back as
CLSID `0x8A09F5F4`, dropping it from its owner's advice list, or adding a subclass record **DIFFERS**. This
permission covers serialization only; it does not assert that a plain advice supplies the subclass's live
city-situation behaviour or its extra IID `0x6BEB1E8A` (`0x00423024`).

#### 11.3.2 Read transitions and strict record comparison (AD18)

Advice Read is a load operation, not a byte-preserving decode. It always enables `text dirty` after reading
the subject (`0x004083F0`–`0x004083FD`, mask `0x10`, enable 1); Write persists the full 32-bit flags word
(`0x004080BB`–`0x004080C3`). For a fresh object reading a successful version-4 record with input flags `F`, an
immediate Write therefore contains `F OR 0x10`. When the captured advice was clean, the changed low byte at
zero-based record offset **10** is a required load transition *[quirk]*. The loaded byte is not a stream field.

For the **strict live-record shadow**, compare all captured and rewritten bytes without restoring or masking
the dirty bit: this transition remains a reported difference, and that sample cannot be credited as exact.
An additional **load-transition conformance** result may compare the output with the input record after
changing just the expected flags word to `F OR 0x10`; it must check every other byte and label the result as
transition conformance, not strict equality. A diagnostic reconstruction that restores the captured bit must
likewise report the restoration separately and cannot turn a strict mismatch into an exact line. This is a
comparison contract, not permission to change core Read or the native object. In particular a dirty input
already containing `0x10` needs no adjustment and must round-trip exactly under the same fresh-object,
version-4, reference-identity assumptions. Older input versions and reading into a non-empty effect list have
the additional transformations already specified above and are separate conformance cases.

### 11.4 Subject (`0x6A26D049`), version 4

```
u16  4
u8   hasLocation
[f32 x ; f32 z]                 // only if hasLocation
u32  data1
u32  data2
ref  object                     // read with IID 0x00000001
```

`Read` rejects any version other than 4.

### 11.5 CityPlanning (`0x2A5F877D`), version 3

```
u16  3
<advisor record §11.2>
s32  buildingCount
s32  parkCount
s32  recreationCount
s32  landmarkCount
landmarkCount × { s32 exemplarInstance ; s32 count }   // ascending signed key
```

`Read` accepts any version. The three counters are always read. The landmark map is cleared and read only if
version > 2. A failed base read does not stop it; it returns `!GetError()` *[quirk]*.

### 11.6 Base-class advisors *[quirk]*

`cSC4AdvisorBase`'s serializable vtable reports CLSID `0xEAC937B4` (the plain `cSC4Advisor`), not `0xCA09F543`. So
the Environment (25) and Fluff news (28) advisors are saved as `cSC4Advisor` and come back as plain `cSC4Advisor`
objects. Older builds saved them as `0xCA09F543` instead: 403 shipped saves hold two such entries each, with the
plain advisor record (save-records.md §7.6). After loading they have no base message subscriptions and no dirty handling. Neither uses reward advices or
variables, so nothing visible changes. A replacement must write `0xEAC937B4` for them and, on load, create an advisor for both `0xEAC937B4` and
`0xCA09F543` *[compat]*.

---

## 12. Interface behaviour (compiled slot tables) *[compat]*

Pop = callee-pop bytes (`abi_extract.py --check`). Return channels read from the epilogues.

### 12.1 `cISC4AdvisorSystem`, vtable `0x00A82AC8`, 29 slots

| Slot | Off | Method (as the game expects it) | Pop | Return | Address |
|---|---|---|---|---|---|
| 0–2 | | QueryInterface / AddRef / Release | 8/0/0 | | |
| 3 | `+0x0C` | `bool Init()` | 0 | AL true | `0x41B570` |
| 4 | `+0x10` | `bool Shutdown()` | 0 | AL true | `0x41A3B0` |
| 5 | `+0x14` | `bool AddAdvisor(cISC4Advisor*)` | 4 | AL true | `0x4197E0` |
| 6 | `+0x18` | `bool RemoveAdvisor(int32 id)` | 4 | AL | `0x419BC0` |
| 7 | `+0x1C` | `cISC4Advisor* GetAdvisor(int32 id)` (no AddRef) | 4 | EAX | `0x419070` |
| 8 | `+0x20` | `std::map<int32, cRZAutoRefCount<cISC4Advisor>>& GetAdvisors()` | 0 | EAX | `0x520080` |
| 9 | `+0x24` | `cRZRandom* GetRandom()` | 0 | EAX (address of the member) | `0x800640` |
| 10 | `+0x28` | `cISCLua* GetScriptingContext()` | 0 | EAX | `0x40D390` |
| 11 | `+0x2C` | `bool DoLink(const cIGZString&)` | 4 | AL | `0x41B080` |
| 12 | `+0x30` | `bool SetGlobalValue(const char*, cIGZString*)` | 8 | AL | `0x418870` |
| 13 | `+0x34` | `bool SetGlobalValue(const char*, double)` | 12 | AL | `0x4188E0` |
| 14 | `+0x38` | `bool SetLocalValue(const char*, double, cISC4AdviceSubject*)` | 16 | AL | `0x419520` |
| 15 | `+0x3C` | `bool PushEvent(int32, cISC4AdviceSubject*)` | 8 | AL true | `0x4190B0` |
| 16 | `+0x40` | `bool SubscribeToEvent(int32, cISC4Advisor*)` | 8 | AL | `0x41B370` |
| 17 | `+0x44` | `bool UnsubscribeFromEvent(int32, cISC4Advisor*)` | 8 | AL | `0x419EB0` |
| 18 | `+0x48` | `void PushContextAdvice(cISC4Advice*)` | 4 | none (EAX garbage) | `0x419840` |
| 19 | `+0x4C` | `cISC4Advice* PopContextAdvice()` | 0 | EAX (already released) | `0x418CE0` |
| 20 | `+0x50` | `cISC4Advice* GetContextAdvice()` | 0 | EAX | `0x418C10` |
| 21 | `+0x54` | `int32 GetAdvicePopupPriorityThreshold()` | 0 | EAX | `0x418780` |
| 22 | `+0x58` | `int32 GetDataTag(int32 i)` (no bounds check) | 4 | EAX | `0x418C30` |
| 23 | `+0x5C` | `cSC4AdvisorUIAdvisorWinProc* GetAdvisorWinProc()` | 0 | EAX | `0x6A0050` (folded) |
| 24 | `+0x60` | `void StartAdvisorWindow()` | 0 | none | `0x4187B0` |
| 25 | `+0x64` | `void ShowAdvisorWindow()` *[inferred name]* | 0 | none | `0x418820` |
| 26 | `+0x68` | `void HideAdvisorWindow()` | 0 | none | `0x418830` |
| 27 | `+0x6C` | `bool IsAdvisorWindowVisible()` *[inferred name]* | 0 | AL | `0x418810` |
| 28 | `+0x70` | `void RemoveWinProcs()` | 0 | none | `0x418840` |

SDK errors (checked): slot 9 returns a pointer to the system's `cRZRandom` (SDK `intptr_t`, harmless); slot 18
returns nothing (SDK `bool`; a replacement should return true); slots 23–28 are missing from the SDK; slot 6 is spelled
`RemoveAdivsor` in the SDK. The overload pair 12/13 is in reverse declaration order, as foundations §16 predicts.
**Slot 9 `GetRandom()` contract (AD8, verified).** The only caller found (export and decomp) is
`game.random_chance` (`0x0040A810`): it loads the system pointer from `0x00B207C0`, calls slot 9 (`+0x24`), and
passes the result as `this` to the **non-virtual** `cRZRandom::RandomDoubleRangeUniform(0.0, 100.0)` (`0x00913761`,
thiscall, two doubles, callee pops 16), which calls `RandomDoubleUniform` (`0x0091372E`). Those touch only the u32
state at **returned pointer + 4**: if it is 0 it is first set to `clock()`; then `state = state × 0x278DDE6D`
(wrapping) is stored and the draw is `int32(state) × f32 constant + 0.5`. Nothing reads offset 0 (no virtual call).
So a replacement's slot 9 must return a non-null pointer to writable memory whose `+4` dword is the generator state
(e.g. a real `cRZRandom`-shaped `{vtable, u32 seed}`), or it must also replace `random_chance`. Returning null
(the parked wrapper's guess) **DIFFERS**: the first `random_chance` call faults.

**Slot 11 `DoLink(const cIGZString& link)` (`0x0041B080`, AD8, verified).** No direct callers (reached through
the interface by the advisor UI's hyperlink handling). Steps:

1. Copy the link (`Data` slot 6 `+0x18`, `Strlen` slot 7 `+0x1C`) into a local string and trim it.
2. `sscanf(text, "%08x%08x", &advisorID, &sessionID)`. If it does not return 2: return **false**.
3. If advisorID ≠ `0x1D` and ≠ 0, and `GetAdvisor(advisorID)` (slot 7) is non-null, and sessionID ≠ 0, and that
   advisor's `GetAdviceBySessionID(sessionID)` (advisor slot 13 `+0x34`) is non-null: set the local string to its
   rightmost `len − 16` characters (the text after the 16 hex digits) and call the advice's `DoLink(&string)`
   (advice slot 22 `+0x58`, `0x00407ED0`). Return **true**.
4. Otherwise run Lua: `PushContextAdvice(null)` (slot 18), then the Lua host's `DoBuffer(p, strlen(p), p)`
   (cISCLua slot 55 `+0xDC`, lua-host.md) with `p` = the text from character **16** when advisorID = `0x1D`, or
   from character **8** in every other fallback case (ID 0, unknown advisor, session 0, unknown advice) *[quirk]*:
   the second 8 hex digits are then part of the executed chunk. The chunk name is the chunk text itself. Then
   `PopContextAdvice()` (slot 19). The `DoBuffer` status is ignored. Return **true**.

A slot 11 that always returns false (the parked guess) **DIFFERS**.

Slots 23–28 are UI (§12.1.1). The export's labels at `0x418840` ("IsAdvisorWindowVisible") and `0x4187B0`
("startAdvisorWindow") do not all match the bodies; the slot names in the table follow the bodies.

#### 12.1.1 Slots 23–28: the advisor window proc *[compat]*

Member `+0x5C` of the system holds one `cSC4AdvisorUIAdvisorWinProc*` (0x70 bytes, own reference, null until
slot 24 runs). Bodies:

| Slot | Body |
|---|---|
| 23 `GetAdvisorWinProc` | return `+0x5C` (no AddRef; null when there is no window). |
| 24 `StartAdvisorWindow` | if `+0x5C` is set, return. Else allocate 0x70 bytes with the game heap (`0x5E55E0`) and run the win proc's constructor `0x41DB50` directly (not `CreateInstance`). If the new pointer differs from `+0x5C`: AddRef it, store it, release the old one. Then call the win proc's `Init` (`0x41E320`) on `+0x5C`, without a null check (an allocation failure crashes, quirk). |
| 25 `ShowAdvisorWindow` | if `+0x5C` is set, `0x41D650`: in the window's parent, show the panel of the current layout (layout field `+0x58`: 0 → child `0x6A15C767`, 1 → `0xAA15EF06`, 2 → `0x2A1D96B1`); then show the one advisor sub-window whose id matches field `+0x10` in the 7-entry advisor-id table `0x00A83508` when field `+0x14` < 7, else all seven sub-windows (`+0x38`..`+0x50`); set visible byte `+0x56` = 1. |
| 26 `HideAdvisorWindow` | if `+0x5C` is set, `HideWindow` `0x41D710`: hide the three layout panels, hide the same sub-window(s) as slot 25, visible byte `+0x56` = 0. |
| 27 `IsAdvisorWindowVisible` | `+0x5C` set → the win proc's byte `+0x56`; else false. |
| 28 `RemoveWinProcs` | if `+0x5C` is set: `0x41D830` (AddRef self; if its member `+0x5C` is set, call that object's slot `+0x1C`; `0x41D530`; if the script window has a parent, remove the three layout panels and the script window from it; if registered, unsubscribe `0x8A09FA67`, `0x8A09FA65`, `0x8A09FA64`, `0x426841A1`, `0xCA09FA6C`; release its members `+0x0C`, `+0x18` (the script window), `+0x5C` and the two 7-entry pointer arrays `+0x1C` and `+0x38`; Release self). Then, if `+0x5C` is still set, null it and Release the win proc (tail call). |

Win proc `Init` (`0x41E320`): build the window from the UI script (group `0x96A006B0`, instance `0xCBC905CD`) under
the main game window (global `0x00B43DC8`), child id `0x6A15C767`, into the win proc's `+0x18`; align it, clear the
accept-focus flags of the three layout panels and hide them; install the win proc on the window; then
`SetupAllAdvisorWinProcs`.

**Creation route.** The win proc class is not registered with the COM director (no `AddCls` entry; it answers only
`QueryInterface` for `0x22E85D8E`, `0x452294AA` and 1), and its constructor `0x41DB50` has exactly one caller (slot 24,
`0x4187CC`). So no GZCOM interface creates it: only the original slot 24 code (or a direct call of the game's code at
those addresses) can. A replacement that answers null / no-op / false is therefore **not** the literal original: the
original always has a window after `Init`, and these UI callers use it through the city getter
(`cISC4City` `+0x144` → system):

| Caller | Slots |
|---|---|
| `cSC4View3DWin::ShowMayorSubPanel` `0x7E6F60` | 25 Show, 26 Hide |
| `cSC4View3DWin::HandleAdviceLinkMessage` `0x7E7830` | 27 IsVisible, then 26 Hide (four pairs) |
| `cSC4View3DWin::ShowPanel` `0x7E8B90` | 26 Hide |
| `cSC4View3DWin::HidePanel` `0x7E8CA0`, `HandleButtonActivated` `0x7F4690` | 27, then 26 |

No caller of slots 23, 24 or 28 was found outside the system (24 and 28 are called by §4.1 step 10 and §4.6 only).
The win proc itself reads the advisors only through the system's slot 7 (`GetAdvisor`: `SetupSingleAdvisorWinProc`
`0x41DCE0`, `SetupAllAdvisorWinProcs` `0x41DE20`, `SetAllMoods` `0x41DF90`, `SetIndividualAdvisorWindow` `0x41F2D0`,
`SetLargeIndividualAdvisorWindow` `0x41F610`, `SetSmallIndividualAdvisorWindow` `0x41F720`) and the advisors'
`GetCurrentAdvices` (§12.2); it never reads the private global (§14.2). A replacement can thus keep the original
window only by running the original constructor and `Init` (a *[choice]* of the shim); with "no window" the View3D
advisor panel stays empty and slot 27 reports false.

**Direct-call ABI of the five bodies** *[compat]*. All are `thiscall` with **ECX = the block base** (the pointer
the allocator returned, vtable `0x00A83548` at `+0`; the message-target sub-object at `+4`, vtable `0x00A83538`, is
used only for message subscription), **no stack arguments**, plain `RET` (callee pops 0 bytes):

| Address | Role | Return channel | Notes |
|---|---|---|---|
| `0x41DB50` | constructor | EAX = `this` | Writes the vtables, zeroes members `+0x08`..`+0x50` (refcount `+0x08` = **0**), sets `+0x10` and `+0x14` to −1, bytes `+0x54`..`+0x56` and `+0x58`, `+0x5C` to 0, and allocates two empty containers with the global allocator `0x90CF54`: a 12-byte self-linked list head at `+0x60` and a 0x18-byte empty tree header at `+0x64` (count `+0x68` = 0). Reads no global. |
| `0x41E320` | `Init` | none (EAX holds a stale pointer; slot 24 tail-jumps here, so slot 24's EAX is that same garbage) | Reads `0x00B43DC8` (main window); its `SetupAllAdvisorWinProcs` reads the city (below). |
| `0x41D650` | show (slot 25) | none | |
| `0x41D710` | `HideWindow` (slot 26) | none | |
| `0x41D830` | teardown (slot 28) | EAX = the result of its final `Release` (tail jump) | AddRef self first and Release self last, so the net count is unchanged. |

(a) The constructor takes only `this`, leaves the count at 0, and slot 24's `AddRef` (shared `AddRef` `0x4BDF00`:
`+0x08` += 1) makes it 1, owned by system `+0x5C`. `Release` `0x41D460` decrements while the count is > 1; at a count
of 1 **or 0** it calls the deleting destructor (vtable slot 5, argument 1) and returns 0. So the owner must AddRef once
after construction and Release once after teardown (slot 28 does: teardown, then null `+0x5C`, then a tail-called
`Release`). (b) `+0x5C` holds the block base, and the same pointer goes in ECX for `Init`, show, hide and teardown.
(c) Callers ignore every return value. The win proc keeps **no** reference to the system or the city: it reads
`0x00B43CEC` (city) → `+0x144` → system slot 7 fresh on every use. It does keep, until teardown, an AddRef'd
`cISC4Advisor*` at `+0x0C` (the advisor shown in the individual window, from slot 7 in `SetIndividualAdvisorWindow`
`0x41F2D0`) and, in the `+0x60` list (`AddMessage` `0x41D9B0`, called from `DoMessage`), the advices taken from messages *[inferred: held]*; it subscribes its `+4` sub-object
with the message server `0x00B43CCC` for `0x8A09FA67`, `0x8A09FA65`, `0x8A09FA64`, `0x426841A1` and `0xCA09FA6C`
(registered from `DoWinProcMessage` `0x41F877` / `0x41F89D`). So a replacement must run slot 28's sequence before its
advisors are destroyed and while the message server exists; teardown does not touch the `+0x60` list (the destructor `0x41DC00` does *[inferred]*).
(d) Besides system slot 7, the win proc calls: advisor slots 6 `GetName`, 8 `GetPersonalName`, 10/11
`GetCurrentAdvices` (iterating the map in the layout of §12.2); advice slots 6 `GetMood`, 11 `GetAdvisorID` (compared
with the shown advisor ID `+0x10` in `DoMessage` `0x41FD10`), 14 `SetFlags(0x20, false)`, 15 `CheckFlags(2)` and
`CheckFlags(0x20)`, 18 `GetTextTitle`, 20 `GetTextMessage`; and it QIs message objects for `cISC4Advice`
`0x8A09F5F5`. It reads no system, advisor or advice member directly and never reads `0x00B207C0`.

### 12.2 `cISC4Advisor` (`cSC4Advisor` iface vtable `0x00A811B0`, 21 slots)

| Slot | Off | Method | Pop | Return | Address |
|---|---|---|---|---|---|
| 3 | `+0x0C` | `bool Init(int32 tableIndex)` | 4 | AL | `0x410100` (base `0x410E30`) |
| 4 | `+0x10` | `bool Shutdown()` | 0 | AL true | `0x40ED30` (base `0x40EF30`) |
| 5 | `+0x14` | `int32 GetID()` | 0 | EAX | `0x40D390` |
| 6 | `+0x18` | `cIGZString* GetName()` (caption object) | 0 | EAX | `0x40D3A0` |
| 7 | `+0x1C` | `bool SetName(const cIGZString&)` | 4 | (tail call into the string's assign) | `0x40D3B0` |
| 8 | `+0x20` | `cIGZString* GetPersonalName()` | 0 | EAX | `0x800640` |
| 9 | `+0x24` | `bool SetPersonalName(const cIGZString&)` | 4 | tail call | `0x40D3C0` |
| 10, 11 | `+0x28`, `+0x2C` | `GetCurrentAdvices()` (both overloads, one body) | 0 | EAX: `std::map<cRZAutoRefCount<cISC4Advice>, int32>&` (the active map) | `0x40D3D0` |
| 12 | `+0x30` | `cISC4Advice* GetAdviceByGUID(uint32)` | 4 | EAX | `0x40D7A0` |
| 13 | `+0x34` | `cISC4Advice* GetAdviceBySessionID(int32)` | 4 | EAX | `0x40D7E0` |
| 14 | `+0x38` | `bool Simulate(int32 kind)` | 4 | AL true | `0x40D820` |
| 15 | `+0x3C` | `int32 GetHeadAnimCount(bool)` | 4 | EAX | `0x40D870` |
| 16 | `+0x40` | `bool SetHeadAnim(bool, int32)` | 8 | AL | `0x40D3E0` |
| 17 | `+0x44` | `int32 GetHeadAnimID(int32)` | 4 | EAX | `0x40F3A0` |
| 18 | `+0x48` | `bool PushEvent(int32, cISC4AdviceSubject*)` | 8 | AL | `0x40EEC0` |
| 19 | `+0x4C` | `bool CommandWouldTrigger(uint32)` | 4 | AL | `0x40D8A0` |
| 20 | `+0x50` | `bool RetireAdvice(cISC4Advice*)` | 4 | AL | `0x40E9B0` (CitySituations `0x413730`) |

**Container layout and consumers of slots 8 and 10–11** *[compat]*. Both maps are the game's SGI-style
`std::_Rb_tree`: the map object is `{header node*, node count}`; the header's `+4` is the root, `+8` the leftmost,
`+0xC` the rightmost node; a node is `{color byte +0, parent +4, left +8, right +0xC, key +0x10, value +0x14}`. The
system's advisor map (slot 8) is at system `+0x4C` (key int32 advisor id, value `cISC4Advisor*` with a reference);
an advisor's active map (slots 10/11) is at advisor `+0x70` (key `cISC4Advice*` with a reference, value int32).
Consumers outside the family, all read-only:

| Consumer | Access |
|---|---|
| `cSC4MySimAgentSimulator::Init` `0x42EC30` | slot 8: walks the advisor map in key order from the leftmost node; QIs each advisor for `cISC4MySim` `0x0A1DA42F`, keeps id 30 apart and the others in order |
| `cSC4AdvisorUIAdvisorWinProc::GetMessageList` `0x41E000` | slot 10: copies the map into its own map (`+0x64`) with the game's `_Rb_tree::operator=` `0x40EB00`, then walks the copy |
| `cSC4AdvisorUIAdvisorWinProc::get_advisor_mood` `0x41DC20` | slot 11: copy-constructs a local map (`0x40F0C0`), walks it, destroys it |
| `cSC4MySimWinProc::GetMessageList` `0x76F260` | slot 10: as the advisor win proc, `operator=` into its own map |

No code outside the advisor, advice and system classes calls an insert, erase or `operator[]` helper of either
tree type (callers of `0x40DE90`, `0x40E440`, `0x40EFF0`, `0x418EE0`, `0x419230`, `0x419640` are all in the family;
the outside callers of `clear` `0x40E050` clear their own copies). So no consumer mutates the original containers:
a replacement may keep its state in its own core and rebuild the game-layout map whenever its content changes (before
any of the four consumers can run), as long as the nodes and the header follow the layout above and the referenced
objects stay alive while listed. No call of slot 8 or 10/11 was found in the city-situation manager, the tutorial,
the popup tracker, the HTML link handler or View3D (every call through the city getter `+0x144` was traced; a
consumer that caches the system pointer elsewhere would not be seen by that trace, but none of them calls a tree
helper of these types either).

SDK error (checked at `0x40D3D0`): `GetCurrentAdvices` returns a `std::map<advice, index>&`, not an
`SC4List<pair<int32, advice>>&`. The subclasses override only slots 3, 4 and (CitySituations) 20.

<a id="ad14"></a>

**`QueryInterface` (AD14, `cSC4Advisor::QueryInterface` `0x0040D430`)** *[compat]*. `this` is the class object; the
interface vtables reach it through thunks that subtract 8 (`0x0040F370`, `cISC4Advisor`), `0xC` (`0x0040F380`,
`cIGZSerializable`) and `0x10` (`0x0040F390`, `cIGZMessageTarget2`). On success it writes the pointer, AddRefs once through the
class vtable slot 1 (one shared count) and returns true:

| IID | Result |
|---|---|
| `0x452294AA` `cIGZMessageTarget2` | class `+0x10` |
| `0xCA09F544` `cISC4Advisor` | class `+0x8` |
| `0xE4FDA3D4` `cIGZSerializable` | class `+0xC` |
| `1` `cIGZUnknown` | class `+0` (via `cRZUnknown::QueryInterface` `0x009457C6`) |
| anything else, including the advisor's own CLSID and `cISC4MySim` `0x0A1DA42F` | **false**, `*out` **not written** |

The same function serves the base class and the Base, CitySituations, Finance, HQ, Safety, Traffic and Utility class
vtables (8 data references). Only `cSC4AdvisorCityPlanning::QueryInterface` (`0x00411810`, its three thunks
`0x00411EE0`/`0x00411EF0`/`0x00411F00`) adds one IID first: its CLSID `0x2A5F877D` → class `+0` with AddRef, true;
everything else falls through to `0x0040D430`. So `cSC4AdvisorCityPlanning` is the only one of the seven that answers
its own CLSID, and none of the seven answers `cISC4MySim`: the MySim agent's QI (§12.2 consumers) fails for them and
succeeds only for the MySim advisor class (`cSC4MySim::QueryInterface` `0x00424B00`: it first **zeroes** `*out`,
then answers `0x0A1DA42F` → class `+0xB8` and `0x8BC01FFE` → class `+0`, else falls through to `0x0040D430`; MySim
spec). A replacement that answers its CLSID for every advisor **DIFFERS** (only CityPlanning does); one that answers
`0x0A1DA42F` for a non-MySim advisor would put it in the MySim agent's list.

Signature: `bool __thiscall QueryInterface(uint32 iid, void** out)`, callee pops 8, result in AL. The caller must
supply a live object and writable non-null `out`; the apparent null-this branches do not provide a usable null
object call (they still dereference this). Failed base/CityPlanning queries do not AddRef. The successful pointer
belongs to the caller and needs one Release through that interface. See §17 item 30.

<a id="ad15"></a>

**Head animations (AD15)** *[compat]* (UI only, not persisted). Offsets below are from the `cISC4Advisor` interface
pointer (class `+8`), the base of the slot functions, like the active map at `+0x70`. Constructor proof:
`0x0040FF68`–`0x0040FF97` (class offsets are eight greater).

| Item | Offset | Constructor value |
|---|---|---|
| current head index | `+0x88` | −1 |
| current list selector (bool) | `+0x8C` | true |
| list **true** (`vector<int32>` of `advisor_heads` indexes) | `+0x90`..`+0x98` | empty |
| list **false** | `+0x9C`..`+0xA4` | empty |

*Loading* (`cSC4Advisor::load_heads` `0x0040FA10`, called once from `Init` at `0x004109B0`; it is §4.4 step 7):
1. With the system's Lua state: `GetGlobal("advisor_heads")`, `n = GetN(-1)` (cISCLua slot 64), `Pop(1)`.
2. Clear both lists.
3. For k = 1 .. n: open a table reader on `advisor_heads[k]`. `type` = the entry's `advisor_type` as int32
   (`ToSint32`, 0 when the reader is invalid). If `type` ≠ `GetID()` (slot 5): skip. Otherwise, if the reader is
   valid and the entry's `advisor_gender` as int32 is **1**: append k to list true; in every other case (gender ≠ 1,
   absent → 0, or invalid reader) append k to list false.
4. Keep the current selection if it is still valid (index ≥ 0 and < size of the selected list). Otherwise index := −1,
   then: list true non-empty → selector true, index 0; else list false non-empty → selector false, index 0; else
   index stays −1 and the previous selector is retained (`0x0040FC18`–`0x0040FCA4`). Selection is retained by
   list index, not by the identity of the Lua entry that previously occupied it.

*Slot 15 `GetHeadAnimCount(bool which)`* (`0x0040D870`): the number of entries of list `which`.

*Slot 16 `SetHeadAnim(bool which, int32 index)`* (`0x0040D3E0`): if 0 ≤ index < `GetHeadAnimCount(which)`
(called virtually): current index := index, selector := which, return **true**. Otherwise current index := −1 (the
selector is left unchanged) and return **false**.

*Slot 17 `GetHeadAnimID(int32 k)`* (`0x0040F3A0`): if the current index is < 0 or ≥ the size of the selected list,
return 0. Otherwise open a table reader on `advisor_heads[list[index]]`; if it is invalid return 0; else return the
entry's array element `[k]` as int32 (`PushNumber(k)`, `GetTable(-2)`, `ToSint32(-1)`, `Pop(1)`;
`0x0040D330`). `k` is the actual Lua key, with no increment or range clamp: ordinary animation entries start at
1, while a key 0 or a negative key is also passed through. The s32 key is converted exactly to binary64 by
`FILD`/`FSTP double` at `0x0040D33D`–`0x0040D347`.

The field reads above all use Lua's numeric conversion: numbers and numeric strings convert, nil or a value
that cannot convert yields 0; finite in-range fractions truncate toward zero. Slot `+0x58` reaches
`0x00880BC0` → `0x008802B0` → `0x008B7640`, then `lua_tonumber` (`0x008BAB50`) and `_ftol2`
(`0x009EEF04`). Do not interpret `advisor_gender` as a Lua truth test: after conversion **only 1** selects true.
NaN/out-of-s32-range conversion is not specified here.

All three animation methods are `__thiscall`. Count and ID each pop 4 and return a 32-bit integer in EAX; Set
pops 8, reads the selector's low byte, and returns bool in AL. None transfers a reference. Count and Set use the
cached index lists; ID fetches the currently selected Lua entry anew on each call. Init uses a borrowed Lua-state
pointer; these methods require the live advisor system/Lua provider for lookup.

Vectors: `advisor_heads = { {advisor_type=5, advisor_gender=1, 111, 222}, {advisor_type=5, 333}, {advisor_type=6,
advisor_gender=1, 444} }` and advisor ID 5 → after Init: count(true) = 1, count(false) = 1, selector true, index 0,
`GetHeadAnimID(1)` = 111, `GetHeadAnimID(2)` = 222, `GetHeadAnimID(3)` = 0; `SetHeadAnim(false, 0)` → true and
`GetHeadAnimID(1)` = 333; `SetHeadAnim(false, 1)` → false and `GetHeadAnimID(1)` = 0 (index −1). An advisor with no
matching entry: both counts 0, index −1, every ID 0. A replacement with no animations matches only that last case.
Further edge vectors: §17 item 31.

**Class (non-interface) vtable** of `cSC4Advisor` and its subclasses: 0 QI, 1 AddRef, 2 Release, 3 destructor,
4–5 inherited `cRZBaseString` helpers, 6 **simulate_day**, 7 **simulate_day_events**, 8 **monthly hook** (calls 9
in the base), 9 **update_variables**, 10 (CitySituations only) **UpdateMySimAdvices**. These are not public, but
the original calls them virtually and a subclass replacement must keep the order if it reuses the base code.

### 12.3 `cISC4Advice` (`cSC4Advice` vtable `0x00A808B8`, 31 slots)

| Slot | Off | Method | Pop | Return | Address |
|---|---|---|---|---|---|
| 3 | `+0x0C` | `bool Init(int32 tableIndex)` | 4 | AL | `0x408C70` |
| 4 | `+0x10` | `bool Shutdown()` | 0 | AL true | `0x407E90` |
| 5 | `+0x14` | `bool Clone(cISC4Advice**)` | 4 | AL | `0x407C50` |
| 6 | `+0x18` | `int32 GetMood()` | 0 | EAX | `0x407AA0` |
| 7 | `+0x1C` | `int32 GetPriority()` | 0 | EAX | `0x407AB0` |
| 8 | `+0x20` | `bool GetTrigger()` | 0 | **AL** (upper EAX garbage) | `0x408970` |
| 9 | `+0x24` | `cIGZString* GetTriggerString()` | 0 | EAX (string object) | `0x477890` |
| 10 | `+0x28` | `uint32 GetGUID()` | 0 | EAX | `0x7C9FD0` |
| 11 | `+0x2C` | `int32 GetAdvisorID()` | 0 | EAX | `0x407AC0` |
| 12 | `+0x30` | `void SetAdvisorID(int32)` | 4 | EAX = the argument | `0x407AD0` |
| 13 | `+0x34` | `int32 GetSessionID()` | 0 | EAX | `0x4FAF50` |
| 14 | `+0x38` | `bool SetFlags(uint32 mask, bool on)` | 8 | AL true | `0x407AE0` |
| 15 | `+0x3C` | `bool CheckFlags(uint32 mask)` (all bits of mask) | 4 | AL | `0x407B70` |
| 16 | `+0x40` | `int32 GetFrequencyInDays()` | 0 | EAX | `0x407BA0` |
| 17 | `+0x44` | `int32 GetTimeOutInDays()` | 0 | EAX | `0x6EFA50` |
| 18 | `+0x48` | `cIGZString* GetTextTitle()` | 0 | EAX (string object) | `0x800640` |
| 19 | `+0x4C` | `uint32 GetTextTitleGUID()` | 0 | EAX | `0x407D70` |
| 20 | `+0x50` | `cIGZString* GetTextMessage()` | 0 | EAX (string object) | `0x407BB0` |
| 21 | `+0x54` | `uint32 GetTextMessageGUID()` | 0 | EAX | `0x407DA0` |
| 22 | `+0x58` | `bool DoLink(const cIGZString&)` | 4 | AL | `0x407ED0` |
| 23 | `+0x5C` | `void NotifyGlobalEffectTriggerChanged()` | 0 | none | `0x407F40` |
| 24 | `+0x60` | `bool SetEventSubject(cISC4AdviceSubject*)` (sets text dirty) | 4 | AL true | `0x407DD0` |
| 25 | `+0x64` | `cISC4AdviceSubject* GetEventSubject()` | 0 | EAX | `0x7FB1D0` |
| 26 | `+0x68` | `int32 GetEventID()` | 0 | EAX | `0x6E0130` |
| 27 | `+0x6C` | `int32 GetCommandID()` | 0 | EAX | `0x407C20` |
| 28 | `+0x70` | `bool UpdateText()` (tail call to slot 30) | 0 | AL | `0x407C30` |
| 29 | `+0x74` | destructor thunk | 0 | — | `0x407B90` |
| 30 | `+0x78` | text rebuild (class virtual) | 0 | — | `0x409300` |

SDK errors (checked): slot 8 returns `bool` in AL (SDK `intptr_t`: reading EAX gives garbage in the upper bytes);
slots 9, 18, 20 return string objects (SDK `char*`); slot 12 returns nothing meaningful (SDK `cISC4Advice*`; EAX
holds the argument, a replacement should return `this`); slot 23 is void (SDK `intptr_t`); slots 29–30 are missing
(class virtuals).

Slots 19 and 21 (`GetTextTitleGUID`, `GetTextMessageGUID`): §6.9.1 (AD12).

<a id="ad13"></a>

**Slot 22 `DoLink(const cIGZString& link)` (`0x00407ED0`, AD13) → always true.** Called by the system's `DoLink`
(§12.1 slot 11 step 3) with the link text after its 16 hex digits.
1. If `CheckFlags(expire request 0x8)`: return true, nothing run.
2. If not `CheckFlags(active 0x2)`: return true, nothing run.
3. Otherwise `PushContextAdvice(this)` (system slot 18, through the system pointer `0x00B207C0`), take the system's
   Lua state (slot 10), `p = link.ToChar()` (cIGZString slot 5 `+0x14`), run `DoBuffer(p, strlen(p), p)` (cISCLua
   slot 55 `+0xDC`: the chunk name is the text itself), ignore the status, `PopContextAdvice()` (slot 19, result
   ignored). Return true.

Signature: `bool __thiscall DoLink(const cIGZString&)`, callee pops 4, result in AL. The string is borrowed for
the duration of the call, and its byte buffer and the Lua pointer are not AddRef'd. The advice reference is the
one the context stack holds between the push and the pop (§12.1 slots 18–19; push `0x00419840`, pop
`0x00418CE0`). The C-string length stops at the first NUL, independently of the string object's stored length.
An active, non-expiring advice requires the live system/Lua provider: there is no null-provider fallback in
`0x00407EED`–`0x00407F2C`. In the two skipped states neither the string nor provider is accessed.
A replacement that returns false or runs
nothing **DIFFERS**. Vectors: active advice, link `"x=1"` → the global `x` is 1 afterwards, and during the chunk
`GetContextAdvice()` (system slot 20) is this advice; same
advice with `0x8` set, or not active → `x` unchanged, result true; a chunk with a Lua error → result still true.
See §17 item 29 for context restoration and return-value checks.

### 12.4 `cISC4AdviceSubject` (vtable `0x00A81090`, 14 slots)

| Slot | Off | Method | Pop | Return | Address |
|---|---|---|---|---|---|
| 3 | `+0x0C` | `bool SetLocation(float x, float z)` (clamps, §2.5) | 8 | AL | `0x40D110` |
| 4 | `+0x10` | `float GetLocationX()` (0 without a location) | 0 | ST0 | `0x40CE10` |
| 5 | `+0x14` | `float GetLocationZ()` | 0 | ST0 | `0x40CE30` |
| 6 | `+0x18` | `bool HasLocation()` | 0 | AL | `0x40CE50` |
| 7 | `+0x1C` | `bool SetObject(cIGZUnknown*)` (AddRef) | 4 | AL | `0x40D130` |
| 8 | `+0x20` | `cIGZUnknown* GetObjectA()` | 0 | EAX | `0x40CF10` |
| 9 | `+0x24` | `uint32 GetData1()` | 0 | EAX | `0x40CE60` |
| 10 | `+0x28` | `SetData1(uint32)` | 4 | EAX = the argument | `0x8B6DE0` |
| 11 | `+0x2C` | `uint32 GetData2()` | 0 | EAX | `0x40CE70` |
| 12 | `+0x30` | `SetData2(uint32)` | 4 | EAX | `0x40CE80` |
| 13 | `+0x34` | `bool Clone(cISC4AdviceSubject** out)` (new subject, AddRef'd) | 4 | AL | `0x40D180` |

SDK errors (checked): slot 13 `Clone` is missing; `SetData1`/`SetData2` return the argument, not a bool (a
replacement should return true).

### 12.5 `cISCLua` (`cSCLua`, vtable `0x00AC4148`, 78 slots)

The slot order matches the SDK. Semantic differences (reported by the Lua analysis, for the Lua host spec):
`ToUserTag` (`+0x70`) returns the boxed pointer of a userdata, not a tag; `PushUserTag` (`+0x8C`) and `PushUserData`
(`+0x114`) are the same function `(void*, int32 tag)`; `ToUint32`/`ToSint32` are one function (truncate to int32);
`Register` (`+0x110`) takes `(fn, name)`. Further return-type differences (`Close` bool, `NewTable` void, `DoFile`/`DoString`/`DoBuffer` int status,
`CloneTag` returns the tag, `ToDataRegistryEntryID` returns the entry pointer): lua-host.md §5.1.

---

## 13. Dependencies

Callee-pop and return channels are as in each subsystem's own slot table, checked again here for the calls this spec
adds. Only the interfaces not yet in `docs/abi` carry a vtable address.

| Interface | Off | Method | Pop | Return | Used by |
|---|---|---|---|---|---|
| `cISC4City` | `+0x58` | GetEstablished | 0 | AL | ticker, PostCityInit |
| | `+0x60` | GetDifficultyLevel | 0 | EAX | thresholds, `difficulty_level` |
| | `+0x110` | GetResidentialSimulator | 0 | EAX | Safety |
| | `+0x11C` | GetMySimAgentSimulator | 0 | EAX | CitySituations |
| | `+0x120` | GetDisasterLayer | 0 | EAX | Safety |
| | `+0x130` / `+0x134` | GetSeaportDeveloper / GetAirportDeveloper | 0 | EAX | CityPlanning, Traffic |
| | `+0x150`, `+0x164`, `+0x168`, `+0x174`, `+0x178` | GetTerrain, SizeX, SizeZ, cell counts | 0 | EAX / ST0 | CityPlanning, subjects |
| `cISC4Simulator` | `+0x14`, `+0x1C`, `+0x48` | start date, GetSimDate, IsAnyPaused | 0 | EAX / AL | ticker, Simulate |
| `cISC4AuraSimulator` | `+0x20`, `+0x24`, `+0x28` | GetMayorRating, GetMax/MinMayorRating | 0, 4, 4 | AL int8 | CityPlanning |
| `cISC4DemandSimulator` | `+0x18`, `+0x24`, `+0x28` | GetDemand, GetJobsBySensus, GetNeutralTaxRate | 8, 4, 0 | EAX / EAX / ST0 | CityPlanning |
| `cISC4Demand` | `+0x20`, `+0x24`, `+0x30`, `+0x68` | QuerySupplyValue, QueryDemandValue, QueryActiveDemandValue, GetDemandCap | 0 | ST0 / EAX ptr | CityPlanning |
| `cISC4RegionalCity` | `+0x24`, `+0x44`, `+0x98`, `+0x104`, `+0xD8`, `+0xE0` | GetPopulation(t), GetExtrapolatedPopulation(t), set utility advisor name *[inferred]*, GetTutorialGUID, GetExported(c), GetImported(c) | 4,4,4,0,4,4 | EAX / ST0 double | CityPlanning, Utility, Lua |
| `cISC4BudgetSimulator`, `cISC4DepartmentBudget` | §7.4, `+0x70`, `+0x28` | | | | Finance, HQ, Safety, Utility, CityPlanning |
| `cISC4NeighborsSimulator` | `+0x40`, `+0x44`, `+0x50`, `+0x54`, `+0x58`, `+0x64`, `+0x78`, `+0x80`, `+0x84` | | | | CityPlanning, Finance, Utility, Traffic |
| `cISC4PowerSimulator`, `cISC4PlumbingSimulator`, `cISC4PollutionSimulator`, `cISC4LandfillDeveloper` | §7.8 | | | | Utility |
| `cISC4ResidentialSimulator`, `cISC4CrimeSimulator`, `cISC4PoliceSimulator`, `cISC4FireProtectionSimulator`, flammability | §7.5, §7.6 | | | | HQ, Safety |
| `cISC4TrafficSimulator` | `+0x28`, `+0x88` | GetCongestionMap, GetTrafficStats | 0 | EAX | Traffic |
| `cISC4AirportDeveloper` / `cISC4SeaportDeveloper` (`0x00AB0714`) | `+0x14`, `+0x20`, `+0x24`, `+0x28`, `+0x34`, `+0x3C` | GetAvgEfficiency (hidden pointer), GetAllPortLots, GetNumPorts, AsISC4PortDeveloper, GetNumSeaportTrips, CityHasSea | 4,4,8,0,0,0 | | Traffic, CityPlanning |
| `cISC4CivicBuildingSimulator` | `+0x18`, `+0x1C` | GetConditionalBuildingStatus, UpdateConditionalBuildingStatus | | | rewards |
| `cISC4BuildingDevelopmentSimulator` | `+0x38`, `+0xA0` | GetBuildingKeyFromType, building type data | 8, — | AL | base DoMessage, rewards |
| `cISC4HistoryWarehouse` | `+0x14`, `+0x18`, `+0x20`, `+0x30` | count, trend, point, RecordHistoryData | | | Lua, Safety |
| `cISC4OrdinanceSimulator` | `+0x1C`, `+0x20`, `+0x34`, `+0x50` | | | | Lua |
| `cISC4View3DWin` | `+0xBC`, `+0xC0` | Assert/RetractGlobalEffectTrigger | 4 | | advice effects |
| `cIGZMessageServer2` | `+0xC`, `+0x10`, `+0x14`, `+0x18`, `+0x1C` | Send, Post, Add/RemoveNotification, post to target | | | everywhere |
| `cISCLua` | §12.5 | | | | everywhere |

The detailed rows of the subsystem calls are in §7 with their slots. Power `+0xA0` `GetChanceOfStrike` is used by
reading `[EAX]` after the call: a replacement power simulator must return the argument pointer in EAX (see the power
spec correction in the report).

**Response curves**: the advisor classes load none. **Grids**: they own none; Traffic reads the congestion map only
through `GetCellValue` (simgrid §6.4 applies to the traffic simulator, not to this system). **Shared helpers**
(AddRadiusEffect, iterators, flood fills): not used.

---

## 14. Interactions and replaceability

### 14.1 Consumers

| Consumer | Uses |
|---|---|
| City situation manager | `GetAdvisor(29)` → `GetAdviceByGUID`, `RetireAdvice`; `PushEvent`; city-situation advices |
| MySim dispatch (`EnterUDriveIt`), My Street sim, View3D vehicle menu | `GetAdvisor(29)` / `GetAdvisor(30)` |
| MySim (`cSC4MySim`) | `GetScriptingContext`, `GetDataTag`, `SetLocalValue`-style table writes |
| Tutorial system / registry | shares the Lua context (`GetScriptingContext`, `SetLua`) |
| Building development simulator, port developers | `PushEvent` / advisor `PushEvent` |
| Civic building simulator | written by reward advices (§6.6); it does not call the advisors |
| UI (advisor windows, popup tracker, View3D panels, HTML link handler) | window slots 23–28, `DoLink`, `GetAdvicePopupPriorityThreshold`, the advice messages; by trace (§12.1.1): View3D slots 25–27, popup tracker `DoMessage` `0x763450` slot 21, `cSC4HTMLLinkHandler::DoURL` `0x78D140` slot 11, MySim balloon win proc slot 9, the advisor win proc slot 7 |
| Lua scripts | `game.*` (§7, §8) |

### 14.2 Replaceability findings

- **Blocker: private global.** The system's constructor stores `this` in a global (`0x00B207C0`). About 60 call sites
  in the advisor classes, the advice classes, the neighbour-deal advice and the Lua extensions use that global, not
  the city getter. A replacement system class must either replace every advisor class, advice class and Lua extension
  as well, or write its own pointer there (a compatibility-table address, *[choice]* only in the shim).
  **All accesses, by byte scan** (every 4-byte occurrence of `0x00B207C0` in `.text`: 66, all `MOV` encodings, none
  takes the address): 3 writes, the constructor `0x41A9B5` (stores the object's primary `cISC4AdvisorSystem` pointer,
  offset 0, vtable `0x00A82AC8`), `~cSC4AdvisorSystem` helper `0x41A1D1` and the base destructor `0x418A1E` (both store
  null); 63 reads, all in: `cSC4Advice` (`Shutdown`, `DoLink`, `get_raw_trigger`, `Init`, `refresh_text`),
  `cSC4AdviceNeghborDeal::Init`, `cSC4CitySituationAdvice::Init`, `cSC4MySimBalloonAdvice::Init` `0x42FE60`,
  `cSC4RewardAdvice::GetTrigger`, `cSC4Advisor` (`Shutdown`, `GetHeadAnimID`, `load_heads`, `Init`), the advisor
  subclasses' `update_variables` / `update_occupant_variables` / `DoMessage` (city planning, finance, HQ, safety,
  traffic, utility), the Lua extension functions `0x409A50`..`0x40C9B0` (registered by
  `cLUAExtensionsCollector::cLUAExtensionsCollector`, plus the UI collector's helpers `0x40C7A0`, `0x40C8B0`,
  `0x40C9B0`, §8.3.1) and the Lua callback `0x418B50` (registered by the system's
  `Init`). **None** is in the advisor UI win procs, the popup tracker, the city-situation manager, MySim
  (`cSC4MySim`, the agent simulator, the MySim win proc), the tutorial, View3D or the HTML link handler: those reach
  the system only through `cISC4City` `+0x144` (§12.1.1, §12.2). Every read whose first use could be traced calls an
  interface slot on the pointer (GetContextAdvice 29, SetGlobalValue(double) 13, GetScriptingContext 8,
  PushContextAdvice 5, PushEvent 3, UnsubscribeFromEvent 1; 4 not traced past the load); no raw member read of the
  system was seen. So when the whole family (system, advisors, advices including the city-situation, MySim balloon,
  neighbour-deal and reward advices, and the Lua extensions) is replaced, nothing original reads the global and the
  shim need not write it. If only part of it is replaced, the remaining original family code needs a
  `cISC4AdvisorSystem*` there (interface calls only, by this evidence), not a layout-compatible object.
- **Blocker: Lua.** Advices, the tutorial and MySim share the system's `cISCLua`. A replacement must provide a
  compatible Lua 4-style state (tags, `gc` tag methods, `game` table) before the original advice classes can run.
- The advisor map, the active maps and the vectors are reached only through the interfaces, but `GetAdvisors` and
  `GetCurrentAdvices` hand out **references to the original containers** (`std::map` of the original STL). A
  replacement that keeps those slots must lay out compatible containers.
- The subclasses rely on the class (non-interface) vtable order of `cSC4Advisor` (§12.2) and on its members
  (advice info vector, maps) through inherited non-virtual code. A replacement advisor must replace the whole
  hierarchy or nothing.
- Through GZCOM: every class is created by the class registry; the system is replaceable through it only with the two
  blockers above handled. Advisor and advice classes are replaceable individually through the script `class_id`
  fields (a new CLSID registered by a plugin) without touching the system.
- Nobody outside reads advisor or advice memory directly *[inferred: no raw offsets found outside the classes]*.

### 14.3 MySim touch points (for the MySim spec)

- CitySituations `UpdateMySimAdvices` (class slot 10) reads `cISC4City` `+0x11C` → MySim agent simulator `+0x30`
  (`GetActiveUIPanelSim`, which returns a `cISC4MySim*`; mysims.md §7.1) → `+0x110` (`ActivateLuaContext`); it activates or deactivates its advices that are city-situation advices (`0x6BEB1E8A`) with situation flag
  `0x20` (MySim mission) by their trigger. Its caller is not identified (§16 Q5).
- Lua: `mysim_*` functions QI the context advisor to `0x0A1DA42F` and call MySim slots `+0xC8`, `+0xCC`, `+0xD0`,
  `+0xD4`, `+0x10C` (`DistanceToClosest`, `DistanceToClosestSpecial`, `LocalSchoolGrade`, `LocalHospitalGrade`,
  `CreateDispatchTool`). The `GetJobTitle` overloads are compiled in reverse order (foundations §16), so SDK slots
  28–54 sit 4 bytes later in the compiled table; from slot 56 on they match (mysims.md §7.2).
- Event `0xCA4E8986` (MySim building added): MySim stores the distance in metres in the subject's data1 for
  `event_building_distance`.
- The system creates the MySim advisors (ids 1–5, class `0x4A1DBBBF`) and My Street sim (30, `0x0BF606A5`) from the
  table, and MySim advices (`0x6A9335DE`, `0x6B70EFAD`). MySim, the My Street sim and the MySim dispatch call
  `GetAdvisor`, `GetScriptingContext`, `GetDataTag` and the subject functions. Nothing is posted to MySim by message.

---

## 15. Original quirks (reproduced)

1. Active advices lose their active state on load (§4.4) while expiry and nextDay are kept.
2. Event advices ignore `nextDay`, `once` and the monthly gate (§6.3).
3. The monthly step is tied to day 28 of the ticker's view of the date and can be skipped (§5.1).
4. Paused updates every 0.5 s (§5.1).
5. Base-class advisors are saved as `cSC4Advisor` (§11.6).
6. CityPlanning: parks and recreation count the same group; non-building occupant messages skip the base; airport
   and seaport subscriptions have no handler (§5.5).
7. HQ: life expectancy tract values repeat the HQ ones; two variables are uninitialised (§7.5).
8. Safety: `ga_crime` keeps the low byte; the crime hotspot falls back to the flammability hotspot (§7.6).
9. Traffic: avenue connection location not reset; port efficiency 100 with no ports; values written twice (§7.7).
10. Utility: "daily" capacities are monthly × 30; garbage import/export slots swapped (intended); rail counts as a
    garbage connection; `ProposeDeals` side effects and a random draw; the power strike handlers are dead (§7.8).
11. `get_reward_building_state` returns its argument for an unexpected status (§8.3).
12. The advice effect list is appended on Read and on Init (§4.5, §11.3).
13. CityPlanning Read accepts any version (§11.5); the advice Read does not check its version (§11.3).
14. Finance's pending flag has no reader (§5.5).

## 16. Open questions

- Q1. `cSC4AdvisorBase` registers a factory for `0xCA09F543` but reports `0xEAC937B4`; whether this was deliberate is
  unknown. Behaviour is specified as observed.
- Q2. reloadPending is cleared in two places and never set in the Windows build. FreeCity4 *[choice]*: keep the flag
  and set it from a host "reload scripts" command.
- Q3. The exact pump position of the ticker relative to the simulator tick (§5.6).
- Q4. The meaning of the data words of the strike messages and of event `0xCA3C0412`.
- Q5. Who calls CitySituations class slot 10 (`UpdateMySimAdvices`).
- Q6. Terrain `+0xD8` (`g_water_tile_count`): cells or vertices.
- Q7. `0x4A64A9CC` (building development event) has no script name.
- Q8. The Lua host (script packages, the resource hash, the protected-call wrapper, `print`) belongs in a separate
  spec.

---

## 17. Conformance tests (must exist in `engine/conformance`)

1. **Batch size**: n = 0 → 1; 1 → 1; 29 → 1; **30 → 2**; 59 → 2; **60 → 3**; 100 → 4; 250 → 9 (float32 rule).
2. **Evaluation gate**: advice with frequency 35, timeout 45, trigger true, created on day 1000 in month 5 (lastMonth
   4). Day pass on day 1000 → active, expiry 1045. Day 1045 → deactivated, nextDay 1080, AdviceRemoved sent. Day 1080
   in month 6 when lastMonth is 6 → not evaluated; after the next monthly step → evaluated and activated.
3. **Once**: the same with `once` → after deactivation nextDay = `0x7FFFFFFF`; never activates again.
4. **Persist / no_timeout**: persist advice whose trigger turns false stays active until expiry; no_timeout advice
   with trigger false and not persist is deactivated the same day; no_timeout and persist stays forever.
5. **Trigger values**: `true` → true; `1` → true; `0` → false; `0/0` (NaN) → true; `"1"` → false; `nil` → false; a
   Lua error → false. Forced trigger (`0x400`) → true. No-text flag → false.
6. **Event advice**: event advice (event `0xAA31A210`) inactive; `PushEvent(0xAA31A210, s)` → on the next day pass the
   advice gets subject s and activates if its trigger is true; a second push the next day re-activates it after a
   deactivation, regardless of nextDay.
7. **Activation messages**: activation posts AdviceAdded (IGZUnknown = advice); command 0x1234 also posts
   AdviceCommand (data1 0x1234). Deactivation sends AdviceRemoved synchronously.
8. **Ticker**: dates 27→28 of a month → monthly then daily step; variables `g_day` 28. Jump from day 27 to day 29 in
   one ticker → no monthly step that month. City not established → the ticker chain stops.
9. **Paused**: paused, same day, 600 ms after the last update → one daily step; 300 ms → none.
10. **Mayor rating**: aura rating −5, min −20 at cell (14, 22), max 40 at (3, 5) → `ga_mayor_rating` −5,
    `l_mayor_rating_l` −20 with subject (14.0, 22.0), `l_mayor_rating_h` 40 with subject (3.0, 5.0).
11. **Subject clamp**: city 64 × 64 cells, `SetLocation(70.0, −3.0)` → (64.0, 0.0).
12. **Reward condition**: condition returns 1 with status 0 → status 6, subject set, trigger true; returns 0 with
    status 6 → status 5; returns a string with status 6 → status 4 and the tooltip set.
13. **Persistence round trip**: advisor with 2 advices (one active, expiry 1045, lastMonth 5, nextDay 0) and one queued
    event → record bytes as §11.2; after Read + Init the active map is empty and the advice's expiry is 1045.
14. **Advice record**: version 3 record (no effect list) reads the subject right after `mood`.
15. **Subject record**: hasLocation 0 → 1 + 4 + 4 + ref; version 3 → Read false.
16. **System record**: two advisors IDs 23 and 26 → `u16 1, u32 2, ref, s32 23, ref, s32 26`.
17. **CityPlanning record**: counters 10, 2, 2, landmarks {0x1234 → 1} → version 3 record as §11.5; a version 2
    record keeps the current landmark map.
18. **random_chance**: with `mul` = 1 before the call: `mul` becomes `0x278DDE6D`, u = f32(0x278DDE6D/2^32 + 0.5) =
    0.65450847, r = f32(u × 100) = 65.450844 → `random_chance(70)` = 1, `random_chance(60)` = nil (foundations §7
    test 13: 0.6545084714889526).
19. **Utility counts**: incinerator capacity 1000 → `g_incinerator_capacity_daily` 30000. Garbage: regional
    exported(3) = 5 → `g_garbage_imported` 5.
20. **HQ empty lists**: no hospitals → `l_hospital_funding_l` = 3.4028234e40 (double), subject null.
21. **Popup thresholds**: shipped table, difficulty 1 → 80; difficulty 7 → 60; missing table → 100.
22. **Lua functions**: `_combine_guids(0x0F0F0000, 0x00F0F0F0)` = 268431600 (`0x0FFFF0F0`);
    `_combine_guids(0x80000000, 0)` = −2147483648 (signed); one argument missing → −1;
    `trend_slope` with slope 1.0 → `f32(atan(1) × 63.6618f)` = 49.999863 (float32).
23. **Registration order (§8.2)**: after `register_api(true)` the global `_tf_` is `print`; after `register_api(false)`
    it is `game.window_graph`; the 47 names register in the §8.2 table order (simulation 1-38, then UI 39-47).
24. **GetRandom (§12.1)**: slot 9 returns a non-null pointer P; with the u32 at P+4 set to 1, one
    `game.random_chance(p)` advances it to `0x278DDE6D`; with P+4 = 0 it is first seeded from `clock()`.
25. **DoLink (§12.1)**: `"0000001D00000000x=1"` runs the chunk `x=1` between PushContextAdvice(null) and
    PopContextAdvice and returns true; `"00000000"` (one field) returns false; a valid advisor+session link calls the
    advice's slot 22 with the text after character 16 and returns true; an unknown advisor ID runs the text from
    character 8.
26. **Cheat payload (§5.2)**: a message 2 of type `0x230E27AC`, data1 `0xA919D25E`, void2 = cIGZString `Adv reload`
    runs `reload_data` and `update_advisors`; data1 `0xAAB7F42A` toggles the DollyLlama byte and ignores void2.
27. **Reload directories (§4.2)**: a file `X.lua` overriding an existing advisor script in both the plugin and the
    SKU-specific `advisors\` directories ends with the SKU-specific contents; one in the data directory only is not
    read.
28. **AD12 text keys and display (§6.9.1; required vectors, not measured runs)**: use string fields and a working
    Lua/table provider. Init with message `"text@1234"`, title `"text@0x4A1B0001"`: the GUIDs are `0x1234` and
    `0x4A1B0001` before display. Supply LTEXT for both and a deterministic detokenizer: refresh reads message
    before title, loads each text before token conversion, passes the same string as input/output with HTML-clean
    true, then restores the prior mode. With debug flag `0x200`, wrap the converted result in
    `"=text@00001234= <converted message> =adv@<eight uppercase hex digits>="` for the message.
    For a literal `"Fire alert"` and no instance-0 resource, expect GUID 0 and
    `"***_NOT_IN_TEXT_DATABASE_*** Fire alert"`, with no debug wrapper. Change the Lua title to a different key:
    a getter alone keeps the old key/text; explicit refresh changes both. Empty text-field list leaves dirty set
    with no push; a non-empty list with invalid advice table preserves text/key, clears dirty and balances push/pop.
29. **AD13 advice link callback (§12.3 slot 22; required vectors)**: with an existing context A, call active advice
    B's `DoLink("x=1")`; during DoBuffer the context is B, parameters are the same byte pointer as source and
    chunk name, and byte length is 3. Afterwards context is A and B's net reference count is unchanged, result
    true. Repeat with a chunk that returns a normal Lua error status: still pop and return true. Inactive B or
    active B with flag `0x8` set: no ToChar/DoBuffer/context operations and true. These distinguish the reported
    false/no-op replacement from native behavior without requiring the entire UI path.
30. **AD14 QI identities (§1.1, §12.2; required vectors)**: start with a non-null output sentinel and known shared
    reference count. On base/Finance, request each of the three interface IIDs and 1: exact adjusted pointer and
    one AddRef each; releasing each result restores the count. Query own CLSID or `0x0A1DA42F`: false, sentinel
    unchanged, no AddRef. CityPlanning's own CLSID succeeds at class +0; its MySim IID still fails with sentinel
    unchanged. MySim's `0x0A1DA42F` succeeds at class +0xB8; an unsupported IID returns false and null output.
31. **AD15 head animation providers (§3.2, §4.4, §12.2; required vectors)**: use the three-entry table in §12.2 to
    check list filtering, default true selection, keys 1/2/3 and switching to false. Set either list with index −1
    or its count: false, index −1, ID 0, old selector unchanged. Reload with only a matching false-list entry:
    default to false index 0; reload empty: index −1, both counts 0. With a still-valid false selection and both
    lists repopulated, retain the false selection rather than choosing true. A Lua entry with `advisor_gender=2`
    belongs to false; `advisor_gender="1"` belongs to true. For the selected entry, `[0]="444"`, `[1]=−7.75`
    give IDs 444 and −7; an absent key gives 0. Mutating `[1]` to 555 between ID calls changes the next result
    without reloading the lists. Save records contain neither these lists/selection nor the displayed advice text;
    a newly created object must rebuild them via Init (§11.2–§11.3), not read fictitious stream fields.
32. **AD17 city-situation record (required vectors, not measured runs)**: construct a reference graph containing
    a `0x4BE372CD` advisor and two distinct `0x8BE3753B` advice references, one also referenced elsewhere. Resolve
    each with IID `0x8A09F5F5`. Give the first a base version-4 record with flags `0x12`, two effects in order and
    a non-null subject; give the second flags `0x10`, no effects and null subject. Fresh-object Read/Write must
    retain both list entries, the shared reference, both subclass CLSIDs, effects and subject identity, with
    exactly the base record framing. With both input dirty bits set, all records compare byte-for-byte. Repeat
    using a plain record adapter registered for `0x8BE3753B`: same result. An unresolved reference, a remapped
    plain CLSID, an added version word or a dropped list entry fails.
33. **AD18 loading versus strict equality (required vectors, not measured runs)**: use a fresh advice for each
    version-4 input, stable reference IDs and no Init/text refresh between Read and Write. Flags `0x00000002`
    become `0x00000012`; flags `0x80000000` become `0x80000010`. Only byte 10 changes, by `0x10`; strict result
    is different, load-transition conformance passes. Inputs `0x00000012` and `0x80000010` remain unchanged and
    pass both checks. Repeat for `0x8BE3753B` using its base-record adapter. Flip one priority byte as well:
    strict and transition checks must both fail. Restore only the captured dirty bit for a diagnostic compare:
    report that adjustment and retain the original strict mismatch verdict.
34. **AD16 field conversions (required vectors, not measured runs)**: on fresh Init use both fields absent,
    both `false`, and both tables with `__tostring` returning `"text@0x4A1B0001"`: each case has both converted
    strings empty, both GUIDs 0, no-text set and an empty registered-field list; the metamethod must not run.
    Use message `"text@0x4A1B0001"` and absent title: message key is retained, title key is 0, no-text is clear
    and both fields register (no scratch-string carryover). Numeric message 125 and title −2.5 convert to
    `"125"` / `"-2.5"`, both GUIDs 0 and no-text clear. After registration change message to nil and title to
    true, rebuild with no key-0 LTEXT and no detokenizer: both display exactly
    `"***_NOT_IN_TEXT_DATABASE_*** "`, with dirty cleared. A string `"abc\0def"` contributes only `"abc"`.
35. **AD11 empty SKU directory (required vector, not a measured run)**: a host getter returning empty must
    produce second lookup `advisors\`; a host getter returning `D:\Regional\` must produce
    `D:\Regional\advisors\`. Keep the plugin lookup first and preserve the specified override precedence;
    neither case may substitute the data directory or `Sku_Data\`.
