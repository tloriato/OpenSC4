# Spec: occupant groups and automata groups (`cSC4OccupantGroupTemplate`)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, which has the GOG build's `.text` bounds and all 28,766 vtable slots of `docs/abi/vtables.tsv`) and
the shipped Lua scripts. Implementers must work from this spec, the public LGPL gzcom-dll SDK headers and general
knowledge only. They must **not** consult decompiled or disassembled code (see
[CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[compat-limit]* depends on something FreeCity4 does not reproduce (here Lua's table order), so
only the rule is matched, not each outcome.

Related specs: [automata.md](automata.md) (§2 the script system component, §7.1 the controller manager calls that
look groups up), [lua-host.md](lua-host.md) (the Lua state and its slot table, the Lua 4 tag methods),
[occupant-manager.md](occupant-manager.md) (§4.4 the occupant filter base class), [foundations.md](foundations.md)
(§6 random numbers), [building-development.md](building-development.md) (§4.7.1.1 the `OccupantGroups` property).

## 0. Summary

An **occupant group** is a named set of occupants with common behaviour, defined by the Lua automata scripts: "every
school gets these generators", "10 % of the houses get a dog". An **automata group** is a special occupant group
for automata (cars, people, boats, planes): it adds the C++ class to create, animations, models, trailers, speed,
drive parameters and player-drive key effects. Both are one C++ class, `cSC4OccupantGroupTemplate`, created by the
automata script system (`cSC4AutomataScriptSystem`, city component 40, automata.md §2) at its `Init` and never saved.

A group is also an **occupant filter**: it includes an occupant whose `OccupantGroups` property (`0xAA1DD396`)
lists the group's ID, then keeps a random share of them given by the group's `percentage`. Groups form a tree
through Lua `_parent`; several queries walk up that tree.

This spec covers the group object (its fields and its ten methods), how the script system fills it from Lua, and
the lookups other code uses. The script system's other tables (controller templates, behaviours, drive commands)
stay with automata.md.

## 1. Identity *[compat]*

| Item | Value |
|---|---|
| Class | `cSC4OccupantGroupTemplate`, **0x84** bytes, global heap; no class ID of its own, no GZCOM factory |
| Base | the occupant filter base `cSC4OccupantFilter` (constructor `0x005EA080`, occupant-manager.md §4.4) |
| `cISC4OccupantFilter` view, `+0x00` | vtable `0x00A96C0C`, 7 slots (§7) |
| `cIGZSerializable` view, `+0x04` | vtable `0x00A96BF4`: the base's Write `0x005E97C0` / Read `0x005E97E0` and `GetGZCLSID` `0x005E95B0` = **`0xA9C74DC9`** |
| Created by | `cSC4AutomataScriptSystem::CreateOccupantGroup` (script slot 13, `0x00520BD0`); in the game only from the script system's own `Init` (§3) |

`QueryInterface` is the base's (`0x005E95D0`): IIDs `1` and `0x46A80026` (`cISC4OccupantFilter`) → `this`;
`0xE4FDA3D4` (`cIGZSerializable`) → `this + 4`; anything else → false. The reference count is at `+0x08`; `Release`
(`0x007CB920`) destroys the object when the count is **1 or less, unsigned** (so 0 too) and returns 0, otherwise
decrements and returns the new count.

The serializable view writes a `u16` 3 and reads a `u16` that must be 3 (the base's record; nothing saves a group).

## 2. State

| Offset | Field | Initial | Filled from (§3) |
|---|---|---|---|
| `+0x0C` | `parent` (group) | null | `_parent`, linked after import (§3.4) |
| `+0x10` | `groupID` (u32) | 0 | `group_id` at creation only |
| `+0x14` | `classID` (u32) | 0 | `class_id` |
| `+0x18` | `idList`: a variant holding a `u32` array, the cached answer of `GetGroupIDVariant` | empty | lazily (§4.6) |
| `+0x2C` | `percentage` (float) | 1.0 | `percentage` |
| `+0x30` | `controllers`: list of controller templates | empty | `controllers` |
| `+0x34` | `anims`: hash map name → `u32` model offset | empty | `anims` |
| `+0x48` | `models`: list of resource keys (type, group, instance) | empty | `models` |
| `+0x4C` | `ownRef` (int16 Lua registry reference to the group's own table) | −2 | import; dropped at link |
| `+0x4E` | `parentRef` (int16 Lua registry reference to the `_parent` table) | −2 | `_parent`; dropped at link |
| `+0x50` | `trailerIDs`: list of `u32` group IDs | empty | `trailers`, resolved at link |
| `+0x54` | `trailerNames`: list of strings | empty | `trailers`; emptied at link |
| `+0x58`, `+0x5C` | `kphMin`, `kphMax` (float) | not set | `kph` (automata groups; 0 when absent) |
| `+0x60` | drive parameter block (0xA0 bytes: airplane, helicopter or watercraft), owned | null | `airplane` / `helicopter` / `WatercraftDriveParams` |
| `+0x64` | player-drive parameter block (0x94 bytes), owned | null | `playerdrive` |
| `+0x68` | `csiImage` (u32) | 0 | `csi_image` |
| `+0x6C` | `sourceBuilding` (u32, an occupant group ID) | 0 | `source_building` |
| `+0x70` | `keyEffects`: hash map drive command (u8) → key effect (§3.3) | empty | `key_effects` |

The contents of the two parameter blocks are the script system's (its `Import…Params` readers); this spec only
records that a group owns them and frees them.

## 3. Building the groups (script system `Init`) *[compat]*

### 3.1 Order

`cSC4AutomataScriptSystem::Init` (`0x00523C80`):

1. **Built-in groups** (`CreateDefaultOccupantGroups`, `0x0051D9B0`): `vehicle`, ID `0x4002`, class ID
   `0x896E714A`, and `pedestrian`, ID `0x4001`, class ID `0x896E75AF`; both are automata groups (§3.2).
2. The scripts run (`ExecuteScripts`, `0x00523B10`), then, in this order: attractor templates, generator
   templates, **automata groups** (`ImportAutomataGroups`, `0x00521C70`), **occupant groups**
   (`ImportOccupantGroups`, `0x005216A0`), controller-template linking, **group linking** (`LinkOccupantGroups`,
   `0x0051F5C0`), the drive-command key map, the track-building groups.

Each import walks its Lua global table (`automata_group`, `occupant_group`) with `lua_next`, so the order of the
entries is Lua's table order *[compat-limit]*; it is observable only where two groups collide and in the order of the
special filter list (§3.2, OG1).

### 3.2 Creating a group (`CreateOccupantGroup(name, id, isAutomata)`, script slot 13)

1. A new group gets `groupID := id`.
2. It is entered in the **name map** (all groups, script `+0x4C`, keyed by the exact, case-sensitive name). If the
   name is already there, the new group is destroyed and the call returns null. A message
   `Couldn't create occupant group named "<name>"` is formatted and discarded.
3. If `isAutomata`, it is also entered in the **automata name map** (script `+0x60`).
4. If `id ≠ 0` it is entered in the **ID map** (script `+0x74`). The first group with an ID keeps it: a later group
   with the same ID is not entered and cannot be found by ID *[compat]*.
5. If `id = 0` it is appended to the **special filter list** (script `+0x88`, script slot 10).

Every container holds one reference.

### 3.3 Reading one Lua group table (`ImportSingleOccupantGroup`, `0x00520DF0`)

The reader is given the entry's name and its table. Every field is read with **`lua_gettable`**
(`SCLuaHelp::GetField`, `0x005FB6A0`). The scripts build most groups with `TTT{…}`, whose `index` tag method
returns the `_parent` table's field when a field is missing (lua-host.md, Lua 4 compat). So **a TTT child inherits
every field below from its Lua parent chain**, while a plain table (no `TTT`) sees only its own fields *[compat]*.

1. `group_id`: a number is truncated to `u32`; a string is read with `strtoul(s, 16)`, so `"0x4002"` and `"4002"`
   are both `0x4002` and a decimal string such as `"100"` is `0x100` *[quirk]*. Absent: 0.
2. If the name map already has this name, that group is **reused** and its `groupID` is **not changed**. Otherwise
   the group is created (§3.2) with `isAutomata = false`.
3. `percentage`: the number; absent: **1.0**.
4. `controllers`: an array of controller-template names. Each name is looked up (script slot 17); found templates
   are **appended** to `controllers`, unknown names are dropped. Absent: `controllers` is emptied.
5. `models`: an array of resource keys (each three hex strings); **appended** to `models`. Absent: emptied.
6. `anims`: `anims` is emptied, then each `name = offset` pair is added (a number, or a hex string read with
   `strtoul(s, 16)`; anything else 0). The first pair with a name wins.
7. `_parent`: if present, `parentRef` := a registry reference to it (an earlier reference is released first).
8. `class_id`: as `group_id` (number or hex string); absent: **0**.
9. The reader stores a light handle to the group in the Lua table under `__group_handle` (raw set), and
   `ownRef` := a registry reference to the table (an earlier one released first).

The two references are kept in 16 bits (`int16`) *[quirk]*; −2 means none.

**Automata groups** (`ImportAutomataGroups`) read each `automata_group` entry as above, then:

- enter the group in the automata name map (if the name is not there yet);
- `trailers`: an array of group names, appended to `trailerNames`;
- `kph`: `kphMin`, `kphMax` := its first and second elements as the script system's array reader stores them
  (each 0 when missing; floats in km/h *[inferred]*, the template comment's "min/max default speed, in kph");
- the drive parameter block is freed and re-read from the first of `airplane`, `helicopter`,
  `WatercraftDriveParams` that is present (none: stays null);
- the player-drive block is freed and re-read from `playerdrive` (absent: null);
- `csi_image`, `source_building`: read as text, then `strtoul(text, 16)` (absent: 0);
- `key_effects`: each element that is a table becomes a key effect (below).

**Key effects** (`ImportKeyEffectsTable`, `0x00521800`). A key effect is three strings and a flag word:

| Lua field | Stored | Flag |
|---|---|---|
| `cmd` | the map key: the low byte of the number (`drive_command`: 0 ACCELERATE … 7 SPECIAL, 9 ALTERNATE …) | |
| `fx` | `fx` | `0x10` when not empty |
| `bone` | `bone` | |
| `controller` | `controller` | `0x20` when not empty |
| `hold` | | `0x01` when true |
| `one_shot` | | `0x02` when true |
| `hard_transition` | | `0x04` when true |
| `override` | | `0x08` when true |

An element without an `fx` (empty or missing) is **dropped**, even if it names a `controller` *[quirk]*. The first
element for a `cmd` wins; later ones are ignored. `bone` is stored as written: an empty `bone` is not replaced by
`fx` here (the scripts' "bone defaults to fx" rule is applied, if anywhere, by the effect code, OG2). `controller`
and `hard_transition` are not in the scripts' template comment but are read.

### 3.4 Linking (`LinkOccupantGroups`)

Three passes over the name map (every group):

1. **Parents.** For a group with a `parentRef`: if the referenced value is a table whose `__group_handle` field
   (read with `lua_gettable`, so it may come from further up a TTT chain) holds a group, `parent` := that group.
   Then the reference is released.
2. **Handles.** Each group's `__group_handle` is removed from its table (raw set to nil) and `ownRef` released.
3. **Trailers.** Each name in `trailerNames` is looked up in the name map; a found group with a non-zero ID adds
   that ID to `trailerIDs`. Then `trailerNames` is emptied.

A `_parent` that is not a group table leaves `parent` null.

## 4. Methods *[compat]*

### 4.1 `IsOccupantIncluded(occupant)` (filter slot 3, `0x0051C3D0`, pops 4, AL)

1. Read the occupant's property `0xAA1DD396` (`OccupantGroups`) through its property holder (occupant slot 6).
2. If `groupID = 0`, or the property is absent, or none of its `u32` values equals `groupID`: **false**. Only the
   group's own ID counts, not its ancestors'.
3. If `percentage ≥ 1.0`: **true**.
4. Otherwise one draw `r := RandomDoubleUniform()` from a **new** `cRZRandom` seeded with `0xFFFFFFFF` (the wall
   clock, foundations §6.1): **true** when `percentage ≥ r`, else false. So the answer for one occupant can change
   between calls *[quirk]*. *[choice]* FreeCity4 seeds that temporary with `derive(0xA9C74DC9 XOR n)`, `n` counting
   earlier draws by this group object (foundations §6.3, temporaries seeded from the clock).

Slots 4 `IsOccupantTypeIncluded(type)` and 5 `IsPropertyHolderIncluded(holder)` return **true** for anything
(`0x004FC900`, a shared body).

### 4.2 `IsOccupantGroup(id)` (`0x0051C380`, pops 4, AL)

True when `id` equals the `groupID` of this group or of any ancestor (`parent`, its `parent`, …).

### 4.3 `GetClassID()` (`0x0051C3B0`, EAX)

The first non-zero `classID` from this group up through its ancestors; 0 if there is none.

### 4.4 `GetSourceBuildingGroup()` (`0x00505330`, EAX)

The first non-zero `sourceBuilding` from this group up through its ancestors; 0 if there is none.

### 4.5 `GetGroupHierarchyDepth()` (`0x0051C4A0`, EAX)

The number of groups on the chain from this one to the root, this one included (a group without a parent: 1). Called
on a null group it returns 0.

### 4.6 `GetGroupIDVariant()` (`0x0051CD90`, EAX = the `idList` variant)

If `idList` already holds values, return it. Otherwise fill it with the non-zero `groupID`s of this group and its
ancestors, **this group first**, as a `u32` array, and return it. The cache is never refreshed (a group whose chain
has no non-zero ID builds an empty list again on every call).

### 4.7 `GetKeyEffect(cmd, out)` (`0x005204D0`, pops 8, AL)

If `keyEffects` has an entry for `cmd` (the low byte of the argument), copy its three strings and flag word into
`out` and return true; else return false and leave `out` unchanged.

### 4.8 Construction and destruction

The constructor (`0x005206B0`) sets the values of §2's "Initial" column. The destructor (`0x00520410`) frees the two
parameter blocks, the key effects, the trailer lists, the models, the anims, the controller list and `idList`, then
runs the base destructor. The deleting destructor (filter slot 6, `0x00520780`) does that and, with flag bit 0,
frees the object to the global heap. The controller templates in `controllers` are only pointers (no reference
counted).

## 5. Who uses it *[compat]*

### 5.1 Lookups (the script system)

| Script slot | Call | Map |
|---|---|---|
| 5 | `GetAutomataGroupMap()` | automata name map |
| 6 | `GetAutomataGroup(id)` | **the ID map**: an alias of slot 11, so it returns any group with that ID, automata group or not *[quirk]* |
| 7 | `GetAutomataGroup(name)` | automata name map |
| 8 | `GetOccupantGroupMap()` | name map |
| 9 | `GetOccupantGroupIDMap()` | ID map |
| 10 | `GetSpecialFilterOccupantGroups()` | the ID-0 groups, in creation order |
| 11 | `GetOccupantGroup(id)` | ID map |
| 12 | `GetOccupantGroup(name)` | name map |
| 13 | `CreateOccupantGroup(name, id, isAutomata)` | §3.2 |

### 5.2 Consumers

- **Controllers** (automata.md §7.1, controller manager slots 9–12): the group's `controllers` list.
- **Creating an automaton** (`cSC4AutomataManager::CreateAutomatonInternal`, `0x00510690`): finds the group by ID
  (script slot 6), else the manager's default group (the same lookup with the manager's default ID); the class
  created is that group's `GetClassID()`, or the manager's default class when there is no group or the class ID is
  0. Without a model exemplar it stores `GetGroupIDVariant()` as the new automaton's `OccupantGroups` property.
- **Automaton exemplars** (`InitExemplarData`, `0x0050F9C0`): a model's `OccupantGroups` list is extended with the
  IDs of each listed group's ancestors (at most 10 IDs in all), `IsOccupantGroup` picks the most specific listed
  group, and `GetSourceBuildingGroup()` gives its source-building group.
- **`cSC4Automaton::PostInit`** (`0x0050CE60`), for an automaton that has no group yet, walks its `OccupantGroups`
  IDs as an ordered set and calls `GetGroupHierarchyDepth` on each one's group but **ignores the result**: the
  automaton's group becomes the **largest** ID of the set *[quirk]*.
- **Player drive** (`cSC4ViewInputControlPlayerDrive::HandleKeyEffects`, `0x00566A70`; summary only, the player
  drive is not specified here): `GetKeyEffect` for the pressed command; `0x02` fires once, `0x01` follows the key,
  otherwise the key toggles; `0x10` starts or stops the effect `fx` (with `0x04` as the hard-transition argument);
  `0x20` switches the named controller; `0x08` suppresses the command's default action.

## 6. Shipped data *[compat]*

The automata scripts are 78 Lua resources of type `0xCA63E2A3`, group `0x4A5E8F3F` in `SimCity_1.dat` (the
template declarations are in instance `0xFF1D4800`). Counted over the scripts as written (Lua keeps only the last of
several assignments to one name, so the game sees fewer):

- 88 `occupant_group.<name>` and 138 `automata_group.<name>` definitions; `pedestrian`, `child` and `arsonist` are
  each assigned twice in `automata_group`. No name is in both tables.
- 35 occupant groups have no `group_id`, even through `_parent`: they are the **special filter groups** (ID 0;
  for example `fire_occupant`, `crowd`, `jailbreak`, the disaster and tool groups). No group inherits its
  `group_id` from a parent, and no two groups share an ID.
- `automata_group.pedestrian` is assigned once with `group_id = "0x8a1e1740"` and once with `"0x4001"`. Either way
  the built-in `pedestrian` group already exists, so its ID stays `0x4001` (§3.3 step 2). `automata_group.vehicle`
  uses `0x4002` and class `0x896e714a`, the built-in values.
- 3 groups set their own `percentage` (two 0.2, one 0.1); the many other `percentage` fields in the scripts belong
  to generator and attractor templates, not to groups. 12 automata groups define `key_effects`, `vehicle` among them
  (its TTT children inherit them, §3.3).

## 7. Interface slot table *[compat]*

`cISC4OccupantFilter` view (vtable `0x00A96C0C`):

| Slot | Offset | Method | Address | Pops | Return |
|---|---|---|---|---|---|
| 0 | `+0x00` | `QueryInterface` | `0x005E95D0` (base) | 8 | AL |
| 1 | `+0x04` | `AddRef` | `0x004BDF00` (shared) | 0 | EAX |
| 2 | `+0x08` | `Release` | `0x007CB920` (base) | 0 | EAX |
| 3 | `+0x0C` | `IsOccupantIncluded(occupant)` | `0x0051C3D0` | 4 | AL |
| 4 | `+0x10` | `IsOccupantTypeIncluded(type)` | `0x004FC900` (shared "true") | 4 | AL |
| 5 | `+0x14` | `IsPropertyHolderIncluded(holder)` | `0x004FC900` (shared "true") | 4 | AL |
| 6 | `+0x18` | deleting destructor | `0x00520780` | 4 | EAX |

Non-virtual methods (thiscall): `IsOccupantGroup` `0x0051C380` (pops 4, AL), `GetClassID` `0x0051C3B0`,
`GetGroupHierarchyDepth` `0x0051C4A0`, `GetGroupIDVariant` `0x0051CD90`, `GetKeyEffect` `0x005204D0` (pops 8, AL),
`GetSourceBuildingGroup` `0x00505330` (all pops 0 and EAX unless noted). Helpers: the key-effect copy
`0x0051FD30`, the key-effect map clear `0x0051FC80`.

## 8. Quirks

1. A TTT group inherits every field from its Lua `_parent` chain, a plain table none (§3.3).
2. A string `group_id` or `class_id` is always hexadecimal, with or without `0x` (§3.3).
3. A group whose name already exists is reused and keeps its first ID; `controllers` and `models` are appended,
   while `anims`, `percentage` and `class_id` are replaced (§3.3).
4. Two groups with one ID: only the first can be found by ID (§3.2).
5. `GetAutomataGroup(id)` also returns non-automata groups (§5.1).
6. `IsOccupantIncluded` draws from a new clock-seeded generator on every call (§4.1).
7. A key effect without `fx` is dropped; the first one per command wins (§3.3).
8. The Lua references are stored in 16 bits (§3.3).
9. `cSC4Automaton::PostInit` computes hierarchy depths and ignores them (§5.2).

## 9. Open questions

- **OG1:** Lua's table order decides which of two groups with one ID wins (§3.2) and the order of the special filter
  list. The shipped data has no ID collision, so only the special-filter order is affected; does any consumer of
  script slot 10 depend on that order?
- **OG2:** where the scripts' rule "if `bone` is not given it is the same as `fx`" is applied: the importer stores
  an empty `bone` (§3.3). The automaton's effect start (automaton slots 44, 45 and 47, `+0xB0`/`+0xB4`/`+0xBC`)
  was not traced.

## 10. Conformance tests

Each test runs the import of §3 on scripted Lua tables (with the `TTT` index tag method of the shipped scripts) and
checks the groups.

1. **Built-ins:** with empty `automata_group` and `occupant_group` tables, `vehicle` has ID `0x4002` and class ID
   `0x896E714A`, `pedestrian` ID `0x4001` and class ID `0x896E75AF`; both are found by name in both name maps and by
   ID; the special filter list is empty.
2. **IDs:** `group_id = "0x10"`, `"10"` and `16` give `0x10`, `0x10` and `0x10`; `"100"` gives `0x100`; a group
   without `group_id` has ID 0, is in the special filter list and not in the ID map.
3. **Reuse:** `automata_group.pedestrian = TTT{ group_id = "0x8a1e1740", class_id = "0x1" }` leaves the ID at
   `0x4001` and sets the class ID to 1.
4. **Duplicate ID:** groups A and B both with `group_id = "0x99"`: exactly one is found by ID `0x99`, and both by
   name.
5. **Inheritance through TTT:** `occupant_group.p = TTT{ group_id = "0x1", percentage = 0.25, controllers = { "c" } }`
   and `occupant_group.k = TTT{ _parent = occupant_group.p, group_id = "0x2" }` (with a controller template `c`):
   `k` has percentage 0.25 and controllers `{c}`, `parent(k) = p`. The same `k` as a plain table: percentage 1.0, no
   controllers, `parent(k) = p` still.
6. **Chain queries:** a root R (ID 1, class 0, source 7), child C (ID 0, class 5), grandchild G (ID 3, class 0):
   `G.IsOccupantGroup(1) = true`, `G.IsOccupantGroup(2) = false`; `G.GetClassID() = 5`;
   `G.GetSourceBuildingGroup() = 7`; `G.GetGroupHierarchyDepth() = 3`; `G.GetGroupIDVariant() = [3, 1]`, and the same
   array after R's ID is changed (cached).
7. **Filter:** a group with ID 5 and percentage 1.0 includes an occupant whose `OccupantGroups` is `[2, 5]`, and not
   one with `[2]`, one with no property, or one whose list holds only an ancestor's ID; a group with ID 0 includes
   nothing. With percentage 0.5 and a fixed draw source, it includes the occupant exactly when the draw is ≤ 0.5.
8. **Key effects:** `key_effects = { { cmd = 7, fx = "a", hold = true }, { cmd = 7, fx = "b" },
   { cmd = 9, controller = "x" }, { cmd = 9, fx = "c", controller = "x", one_shot = true, hard_transition = true } }`:
   `GetKeyEffect(7)` gives `fx = "a"`, flags `0x11`; `GetKeyEffect(9)` gives `fx = "c"`, controller `"x"`, flags
   `0x36`; `GetKeyEffect(8)` is false.
9. **Trailers:** `automata_group.t = { group_id = "0x20", trailers = { "car", "nosuch", "special" } }` with `car`
   (ID `0x21`) and `special` (ID 0): after linking, `trailerIDs = [0x21]` and no trailer names remain.
