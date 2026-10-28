# Network manager and network lot manager (behaviour spec)

Status: M5 behaviour spec, written 2026-10-15 (re-specs13c lane). Every fact was checked on the Windows executable
`bin/SimCity4.exe` (1.1.641, image base `0x00400000`); every address is a Windows address. Numeric details (float or
double, operation order, widths) were checked against the instruction bytes. Implementers work from this spec, the
public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled or disassembled
code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Tags:

- *[compat]*: observable; a replacement must match it.
- *[choice]*: a FreeCity4 decision.
- *[inferred]*: consistent with all the evidence, but not proven.
- *[quirk]*: original behaviour that looks unintended but is reproduced.

`r24(e)` is `e` rounded to float32 (foundations §18).

Scope. Two city components:

- **cSC4NetworkManager** (component 53): the owner of the 13 network tools, the list of *globally managed* network
  occupants (the saved list), batched network removal, the reaction of networks to terrain edits, connection nodes
  for lots, lighting of the managed pieces, and a retrofit pass for old interchange pieces.
- **cSC4NetworkLotManager** (component 11): the *network lots*. A network lot is a decoration record attached to one
  network cell (a road, street, avenue… piece). It picks a *network lot configuration* that fits the cell's zone,
  wealth, position, rotation and slope, and creates its props and flora. The same class owns the **pothole** effect
  props.

What is not here:

- The network tools (placement geometry, cost, preview, bridges, tunnels) are their own subsystem. This spec states
  only what the simulation and the harness's scripted actions see of them (§9). The tool's placement, cost,
  removal and underground refit are in [network-tools.md](network-tools.md).
- The traffic graph built from the occupants is [traffic-network.md](traffic-network.md). It is cited, not repeated.
- The occupant manager (insert, remove, queries, filters) belongs to the re-specs13b lane.

Related specs (cited, not repeated):

- [traffic-network.md](traffic-network.md): network flags `0x200000` and `0x40000` in the graph (§4.2), its
  cISC4NetworkOccupant slots 53 and 54 (§7.2), Q3 (flag meanings).
- [traffic-simulator.md](traffic-simulator.md) §3.6 and §4.12: the pothole sets and the `0x69247DC8` posts.
- [demolition.md](demolition.md) §4.2–§4.6: `DemolishRegion`, `UpdateNetworks`, the removal privileges.
- [disasters.md](disasters.md) §6.3–§6.4: the riot instance. Its Q4 is answered here (§10).
- [plumbing.md](plumbing.md): the pipe flags `0x10000` and `0x20000`.
- [zones.md](zones.md) §5.2 and §5.2.1: who places networks by code, and `data4` of the insert message.
- [lot-developer.md](lot-developer.md): `AddNetworkConnectionNodes` (`0x006C9C80`), `RemoveNetworkConnectionNodes`
  (`0x006C9FB0`) and `GetAreaSlope` (slot 29).
- [prop-developer.md](prop-developer.md) and [city-detail.md](city-detail.md) §3.4: callers of the network lot
  manager.
- [flora.md](flora.md): `AddNewFloraOccupant` (slot 17), `DemolishFloraInCellArea` (slot 20).
- [clock24.md](clock24.md): `IsNight`, and the dusk and dawn messages.
- [building-development.md](building-development.md) §4.11 and [landfill.md](landfill.md) §3.2: the terrain message
  `0x278E6003` and its `data4` kinds.
- [foundations.md](foundations.md): §3.6 components, §3.7 load order (Read before Init), §6 random generators,
  §18 precision.
- [simgrid.md](simgrid.md) §7.4: the scanning and boundary iterators.

---

## 1. Identity

| | Network manager | Network lot manager |
|---|---|---|
| Class ID | `0xC990BD46` | `0xE9A6CD3B` |
| Component | 53 (`foundations.md` §3.6) | 11 |
| Primary IID | `0xA9764C8C` (cISC4NetworkManager) | `0x89A5BA3F` (cISC4NetworkLotManager) |
| Main vtable | `0x00AA7AD0`, 25 slots (0–24) | `0x00AA7958`, 16 slots (0–15) |
| cIGZMessageTarget2 (`0x452294AA`) | sub-object `+4`, vtable `0x00AA7ABC` | sub-object `+4`, vtable `0x00AA7948` |
| cIGZSerializable (`0xE4FDA3D4`) | sub-object `+8`, vtable `0x00AA7AA4` | sub-object `+8`, vtable `0x00AA7930` |
| QueryInterface | `0x00610750`: IID 1 and `0xA9764C8C` → this; `0x452294AA` → `+4`; `0xE4FDA3D4` → `+8` | `0x0060C760`: the same pattern with `0x89A5BA3F` |
| Global | `SL::NetworkManager` (city slot 38) | city slot 42 |
| Init order (foundations §3.7) | after the prop manager | after the prop developer |

Both are created empty, `Read` (if the city was saved) **before** `Init` (foundations §3.7). Release deletes the
object when the count drops to 1 or below. All `DoMessage` branches of both classes return true.

---

## 2. State

### 2.1 Network manager

| Field | Type | Initial (`0x00613330`) | Saved |
|---|---|---|---|
| `initialized` | bool | false | no |
| `ukPaths` | bool: apply the UK path remapping (§7.11) | false; set in `Init` | no |
| `longTerrainOp` | bool: a long terrain operation is running (§7.6) | false | no |
| `dirty` | cell rect `x1, z1, x2, z2` (int32): the accumulated terrain-change area | `0x7FFFFFFF, 0x7FFFFFFF, −1, −1` (empty) | no |
| `managed` | vector of network occupant refs: the **globally managed** pieces | empty | **yes** (§4.1) |
| a hash set | `hash_set(100)`; its use was not traced | empty | no |
| texture, shadow and UK path remap tables | maps read from an ini resource by `Reload` (`0x00613140`) | §7.11 | no |
| `pending` | vector of occupant refs queued for removal (§7.3) | empty | no |
| `queueing` | bool | false | no |
| `retrofit` | circular list of network occupant refs (§7.10) | empty | no |
| `crews` | vector of construction-crew refs (§7.12) | empty | no |
| `tools[13]` | the cached network tools, one per network type | null; created in `Init` | no |

`Reload` reads the ini resource `0x8A5971C5 / 0x8A5993B9` with the sections "Texture remapping", "Shadow
remapping", "Shadow edge remapping" and "UK path remapping". Those tables only feed the presentation slots 13–15
(§7.11).

### 2.2 Network lot manager

| Field | Type | Saved |
|---|---|---|
| `lots` | hash map `u16 key → network lot` (key below) | **yes** (§4.2) |
| `configs` | hash map `u32 pieceId → vector of network lot configurations`; load order within each vector, hash order between groups (§2.2.1) | no; rebuilt in `Init` |
| `forced` | network lot configuration ref or null: the configuration for the next piece (slot 13) | no |
| a scanning iterator over the city | reset in `Init` to `[0, X−1] × [0, Z−1]`; no other reader was found | no |
| `potholes` | hash map `u16 key → pothole effect prop` | **yes** (§4.2) |
| `initialized` | bool | no |
| `pending` | hash set of `u16` keys: cells whose network lot must be re-examined (§8.5) | no |

The cell **key** is `(x & 0xFF) << 8 | z` (u16). `X` and `Z` are the city's cell counts (city slots 93 and 94).

#### 2.2.1 Lots, potholes and configuration-group iteration (NM21) *[compat]*

The following applies to both the `lots` and `potholes` u16-key maps; it is proved from their shared
instantiated functions, not borrowed from `pending`. The lot-manager constructor is the unnamed function
`0060EE50`: it constructs `lots` at `+0C`, configuration groups at `+20`, `potholes` at `+58`, and `pending`
at `+74`. **Each requests 100 buckets**, through `00829BC0`, which selects the first prime at least 100:
**193 initially**, with count zero (`0060EE62–0060EEA4`, `0060EEE5–0060EF12`, `00829BC0–00829C27`). This
shared constructor initializes bucket storage only; it does not change each map's key width or node type.
Their hash is the **zero-extended u16 key itself**, bucket `key % B`; collision equality compares exactly
16 bits (`0060DA58–0060DA80`). A node has next/key/value at `+0/+4/+8`. `operator[]` searches the existing
chain first. A hit returns the existing value slot without moving its node or resizing. A miss calls
`find_or_insert` (`00712120`), first requesting resize for `count+1`, then prepends the new node to its
bucket and increments count. Thus overwriting a saved key does not make it newest in its bucket.

Resize occurs only if the requested new count exceeds B. It selects the first prime not below that count
from the table at `00A829B0`, beginning `53,97,193,389,769,1543,3079,6151`, and does nothing unless that prime
exceeds the old B (`00710DD0–00710E2A`). Rehash visits old buckets in ascending order, repeatedly removes
their head node and prepends it to `key % newB` (`00710E51–00710E8B`). The old insertion order therefore
cannot reproduce all later orders. Component-11 Write follows each chain head-to-tail, then scans later
buckets in ascending order (`0060D72A–0060D765`); the lots and potholes use this order separately.

Read inserts both kinds through this **ordinary `operator[]`**, then assigns the acquired lot/prop pointer
(`0060DB67`, `0060DBBB`); it does not restore a serialized bucket layout or append to collision tails.
For example, at initial B=193, keys 2 then 195 collide, producing chain `[195,2]`; reassigning key 2 keeps
that order. A later resize to 389 separates these two into buckets 2 and 195. Reading serialized keys
195 then 2 into an empty B=193 map produces `[2,195]`, rather than preserving that collision order.

The configuration-group map uses the **full unsigned u32 piece ID** as its hash and equality key,
bucket `pieceId % B` (`0060E3CB–0060E3F9`). Its node contains next/key/vector at `+0/+4/+8`. A hit returns
the existing vector without moving or resizing the node. A miss requests `count+1`, then prepends a new
node containing an empty vector (`0060E401–0060E43D`, `0060E250–0060E2C8`). Its resize routine `004F0940`
uses the same prime table/strict count-over-B trigger, and moves old heads to new heads while walking old
buckets in ascending order (`004F094C–004F09FA`). `AddLotConfiguration` first AddRefs the configuration,
gets its piece ID, and appends the pointer to that group's vector (`0060E7A0–0060E7FC`). Adding another
configuration to an existing group neither replaces earlier pointers nor changes group position.
`GetNetworkLotConfigurationSet` enumerates groups by ascending bucket/head-to-tail chain, then each
vector from index zero upward; it appends and AddRefs each returned configuration (`0060DC2E–0060DCCB`).
Thus resource callback/load order determines **within-group** order, not a global first-loaded group order.
Configuration groups are rebuilt, not read from component 11. No CRT `rand()` occurs in these map paths.

**Worked component-11 ordering fixture, derived from the native operations (not a recorded game save).**
Use an otherwise empty manager, no potholes, and insert lots with keys `0…191` in increasing order,
then key 389 (`0185`, cell `(1,133)`), for 193 distinct entries. Component-11 Write's lot entries are
`0,1,2,389,3,4,…191`: at B=193, bucket 3 contains `[389,3]`. Updating key 3 leaves this order unchanged.
Now insert key 778 (`030A`, cell `(3,10)`), the 194th distinct key. The count request 194 grows B to
389 **before insertion**. Rehash visits old bucket 0's key 0 before old bucket 3's key 389, so new bucket
0 becomes `[389,0]`; insertion prepends 778 there. The next Write enumerates
`778,389,0,1,2,3,…191`, with count 194 and a zero pothole count. Each entry's own cell key and reference
payload are unchanged; only the entry/reference visitation order changes. Potholes undergo the same
transformation in their separate map. For a group-map collision, adding piece IDs 2 then 195 yields
group order `[195,2]` within bucket 2; adding a second configuration for piece 2 appends inside its vector
and retains that group order. These are finite conformance examples, not attribution of the question's
reported object-ID shifts to a particular insertion history.

### 2.3 Network lot (cSC4NetworkLot, vtable `0x00AA78E0`, IID `0x89A5BA66`)

A 24-byte reference-counted object.

| Field | Meaning |
|---|---|
| `occupant` | the network occupant of the cell (ref) |
| `config` | the chosen network lot configuration (ref; null only between `Read` and `Init`) |
| `configId` | the configuration ID read from the save; only used to resolve `config` in `Init` |
| `locked` | bool: the lot was created from a forced configuration and is never re-chosen (§8.4) |

Slots: 3 `GetNetworkOccupant` (borrowed), 4 `GetLotConfigurationObjectCount`, 5 `GetLotConfigurationObjectArray`
(the configuration's objects; §8.6), 6 deleting destructor.

### 2.4 Network lot configuration (cSC4NetworkLotConfiguration, vtable `0x00AA7998`)

**Interface identity (NM15).** There is no separate configuration IID accepted by this implementation.
Its slot 0 is the folded function `0x0048C940`: only IID **`0x00000001` (IUnknown)** succeeds, writes the
unchanged object pointer and calls slot 1 once. Every other IID returns false without writing the output
(`0x0048C940`–`0x0048C959`). The descriptive name `cISC4NetworkLotConfiguration` does not identify an
additional QueryInterface route. A caller already holding the typed pointer uses its slots directly.

The object is 108 bytes; its unsigned 32-bit reference count at `+0x68` starts at zero. Slot 1
(`0x0060C830`) increments it; slot 2 (`0x0060C840`) decrements when greater than one, otherwise invokes
the deleting destructor, slot 7 (`0x00610070`), and returns zero. Slot 3 (`0x0060FB50`) returns the
object-vector reference: the three-word begin/end/capacity descriptor, not its first object.

**Creation is direct allocation, not a GZCOM request.** The manager's `AddLotConfigurationStatic`
(`0x0060FB90`, constructor call `0x0060FBF1`) and the network-lot editor (`0x00499E85`) allocate and construct
configurations directly. The lot manager also allocates network lots directly in its creation/read paths.
These paths supply no native lot/configuration factory CLSID: do not use the lot IID `0x89A5BA66` as a
CLSID or invent a configuration IID. FreeCity4-only registration IDs describe replacement-owned views;
they do not change the native creation paths. The native lot accepts both IUnknown and `0x89A5BA66`,
returning the same pointer with one AddRef (`0x0060C800`–`0x0060C822`). See §8.6 for the set boundary.

Read from exemplars of exemplar type `0x21` (property `0x10`) by `ReadLotConfiguration` (`0x0060F5F0`). Defaults are
those of the constructor (`0x0060FA90`).

| Field | Property | Default | Meaning |
|---|---|---|---|
| `pieceId` | `0xC9A5A1BE` (required) | — | the network piece this configuration decorates; the `configs` key (configuration slot 6) |
| `id` | `0x21` | 0 | the configuration ID (configuration slot 4); saved in network lot records |
| `name` | `0x20` | empty | — |
| `zones` | `0x88EDC793`: a list of zone-type bytes | 16 × false | allowed zone types; values ≥ 16 are ignored |
| `wealths` | `0x88EDC795`: a list of wealth bytes | 4 × false | allowed wealths; values ≥ 4 are ignored |
| `mask[4]` | `0x49D55951`: exactly 4 bytes, else ignored | `0F 0F 0F 0F` | position pattern rows (§8.3) |
| `period` | `0xCA81B8D4` (the low byte is used) | 4 | the pattern's period n |
| `minSlope`, `maxSlope` | `0xAA120972`, `0xAA120973` (float) | 0.0, 64.0 | allowed slope range |
| `rotations` | `0xEC3BD470` (byte) | `0x0F` | bit r allows piece rotation r |
| `flipRule` | `0xCC3E4755` (byte) | 0 | 0 any; 1 the piece must be flipped; 2 it must not be; other values: any |
| `objects` | the lot-object properties | empty | 52-byte lot configuration objects, built by the shared `ConvertPropertiesToObjectArray` (`0x00479D10`, see city-detail.md) and then moved by `CorrectObjectPositions` for a 16 × 16 m lot |
| (version) | `0x88EDC789` | 1 | below 2 the configuration is written back through `WriteLotConfiguration` (slot 14). Tooling only; FreeCity4 skips it *[choice]* |

A configuration whose zone or wealth list is missing allows nothing, so it is never chosen.

---

## 3. Lifetime

### 3.1 Network manager

**Init** (slot 3, `0x00610C10`). Nothing if already initialized; otherwise:

1. `initialized := true`.
2. `ukPaths := (language manager slot 38, then slot 25) == 0` (the UK-style path remapping, §7.11).
3. Subscribe on the message server, in this order: `0xA6B79602`, `0x278E6003`, `0xEA3B0AF1`, `0x6A3B0AFD`,
   `0x6AB97795`, `0x26D31EC2`, `0x26D31EC1`, `0x99EF1142`, `0x99EF1143`, `0xC9DA96E8`, `0xC9DA96E9`.
4. `longTerrainOp := false`.
5. Create the 13 tools, one per network type (types: 0 road, 1 rail, 2 highway, 3 street, 4 pipe, 5 power line,
   6 avenue, 7 subway, 8 light rail, 9 monorail, 10 one-way road, 11 dirt road, 12 ground highway): the generic
   network tool for types 0–3, 6 and 8–12, the pipe tool for 4, the power-line tool for 5, the subway tool for 7.
   Each is held (AddRef) in `tools[type]`.
6. Return true.

**Shutdown** (slot 4, `0x00611EB0`). Nothing if not initialized; otherwise `initialized := false`; release and clear
`managed`; release every `retrofit` entry and free the list; null every tool; unsubscribe the same 11 messages in the
same order. Return true. `pending`, `crews` and `dirty` are left as they are.

**PreCityShutdown** (`0x26D31EC2`, §6.1): empties `crews` and drops the tools before Shutdown runs.

### 3.2 Network lot manager

**Init** (slot 3, `0x00610120`). Nothing if already initialized; otherwise:

1. `initialized := true`.
2. Subscribe, in this order: `0x89D3E9F3`, `0x99EF1143`, `0xA9B6C2D6`, `0x07699455`, `0xE8BDA2E7`, `0x69247DC8`.
3. Reset the whole-city scanning iterator (§2.2).
4. **Load the configurations**: list every exemplar key (type `0x6534284A`) through the resource manager (slot 18)
   and, for each key in the list's order (`0x0060FB90`): if the exemplar's type (property `0x10`) is `0x21`, read
   a configuration (§2.4). If the read succeeds, append it to `configs[pieceId]`.
5. **Resolve the loaded lots**: for each network lot (in `lots` order) search the configurations (each group in map
   order, each group in vector order) for the first whose `id` equals the lot's `configId`, and set `config` to it.
   If none matches, the lot's key is noted (a "Missing lot configuration 0x%08x" message is formatted and thrown
   away). After the loop every noted key is erased from `lots`. The lot object is **not** released *[quirk: leak]*;
   its props and flora stay in the city.
6. Return true.

**Shutdown** (slot 4, `0x0060E570`). Nothing if not initialized; otherwise `initialized := false`; unsubscribe the
six messages in the Init order; release every network lot; release every configuration; clear `configs`; for every
pothole prop call its slot 8 with 1 (stop) and release it. Return true. `lots` and `potholes` are not cleared
*[inferred: the destructor clears them]*.

**City teardown and action-created occupants (NM16).** Distinguish the early PreCityShutdown message from
the later city Shutdown method. Crews must finish slot 5 and unregister while `city.GetNetworkManager()`
(city slot 38, `+0x98`) still returns the live manager. Their shutdown also needs the message server and,
when animation was subscribed, the animation-tick service. Keep the occupant/automata services usable until
the crews have stopped their vehicles and released their occupant references. Dropping the replacement
manager identity or its opaque-crew bridge before these callbacks violates the native ordering (§7.12).

The relevant **native city Shutdown order** is different from normal in-city removal:

1. `cSC4City::Shutdown` (`0x0045B1D0`) first calls `SL::OnCityShutdown` (`0x00601FF0`). This clears cached
   city-service pointers, including occupant manager `0x00B43D0C`, prop manager `0x00B43D10`, network-lot
   manager `0x00B43D58` and the automata-manager caches. It does **not** clear the current-city pointer
   `0x00B43CEC` or application message server `0x00B43CCC`, and does not itself Release the city-owned
   services. A null cache here is intentional, not proof that its owner object has already been destroyed.
2. Among the services relevant here, shut down/release the network lot manager (city field `+0x160`),
   then the network manager (`+0x15C`). Network-lot destruction releases its configuration first and its
   network occupant second (`0x0060D139`–`0x0060D14E`); it does not remove lot props/flora from the city.
   Network-manager shutdown releases its globally-managed occupant references. These releases precede
   the occupant-manager drain; that manager still owns the occupants inserted in its spatial lists.
3. Shut down the occupant manager (city field `+0x12C`), then release it. Its Shutdown (`0x0049E000`)
   calls slot 22 (`+0x58`) with `(null filter, true, 0)`. RemoveOccupants (`0x0049FCD0`) detaches the
   selected occupants and clears their in-city flag under the lock, then for each detached occupant
   sends its removed notification (`0x99EF1143`), calls occupant slot 4 Shutdown, and finally Releases it.
   The detached work list retains the ownership needed through each Shutdown callback.
4. The city shuts down/releases the prop manager (city field `+0x130`) after the occupant manager.
   Keep framework message delivery, animation unsubscription and the owned child objects valid until
   all native occupant Shutdown callbacks and their final Releases have returned. Complete the later
   service/facade disposal only after that drain. This is a dependency order, not a requirement to keep
   the cleared SL caches artificially non-null.

For **ordinary removal while the city is live**, a construction occupant uses the cached prop manager
(`0x00B43D10`) to RemoveProp, slot 29 (`+0x74`), for its construction props and dust effect before releasing
its own references. During bulk city teardown that cache is already null, so those RemoveProp calls are
skipped and the occupant-manager drain handles the children. The message server and animation-tick
service are still used for unsubscription in either path. Automata removal similarly uses the live city's
pedestrian-manager getter (`+0xB0`) when present. Do not dispose a returned manager while a native callback
is still using it (`0x005D0CE0`–`0x005D1088`).

**What `0x005D0F94` actually requires.** This instruction reads the vtable of the dust-effect pointer at
construction-occupant interface `+0x134` (complete object `+0x13C`), before Release at `0x005D0F96`.
It is an **unconditional object dereference**, not a network-manager lookup. CreateDustClouds allocates
and AddRefs a visual-effect prop, registers/starts it and stores that pointer at `0x005CBD05`.
SetLot (`0x005D0890`) invokes CreateDustClouds only after resolving a non-null building from its lot;
its early-return path leaves the constructor's null dust pointer intact. Init alone does not construct
the dust effect. A valid newly created construction occupant must complete that SetLot setup before
the initialized object can reach native Shutdown. Service lifetime changes alone cannot make a null
dust pointer valid. Preserve its owned reference until Shutdown's single Release; do not replace it with
a borrowed facade, or treat a partial Init as a fully configured construction occupant.

**Worked lifetime case (NM16).** Let a native base network occupant N have one occupant-manager ownership
reference and one network-lot ownership reference; let configuration C have one catalogue reference and
one lot reference. Drop any caller's lot view, then shut down the lot manager: destruction of the last
lot reference releases C and N once each. N still has its occupant-manager reference; releasing the
catalogue's C reference can now destroy C. The later occupant-manager drain calls N.Shutdown and then
releases N's remaining reference. A view which acquired another reference must release exactly that
reference; there is no additional permanent pin to add in response to a last-reference warning.

In the same teardown test include an initialized native construction occupant with a valid lot/building
and a retained dust effect D. Complete SetLot, deliver PreCityShutdown, perform the cache-clear and manager
order above, and drain the occupant manager. Expect message/tick unsubscriptions to succeed, optional
RemoveProp calls to be skipped after cache clear, and one Release of the still-valid D. A separately
tested Init-only/failed-SetLot occupant has no D and does **not** meet native Shutdown's precondition;
the address in the reported crash identifies that missing object but does not establish how it became
missing. The twelve lot-destructor warnings and the separate no-module action crash do not establish
a cause or authorize changes to unrelated services. Action acceptance still needs an engine run of
the corrected lifetime/setup contract; the earlier 73/73 shadow result is not that test.

#### 3.2.1 Construction-site automata boundary (NM17) *[compat]*

This is the **building construction occupant's** crew, created during SetLot, not the network manager's
`cSC4NetworkConstructionCrew` in §7.12. The automata-side creation, interface and concrete-layout contract
belongs to [automata.md §7.6](automata.md#76-native-construction-site-boundary-nm17-compat). A native
construction occupant may use replacement automata even when network-lot objects remain native.

**Setup order.** After accepting a lot with a building, SetLot (`0x005D0890`) rebuilds the model, divides the
lot into eight candidate rectangles, attempts the crane, adds ground props, and calls CreateConstructionCrew
(`0x005D09C5`). After the crew call returns, it adds roof props, creates dust, sparks and the lot texture
(`0x005D09CA`–`0x005D09E6`). Thus an automaton returned during crew creation must already support the calls
below; the dust-effect precondition from NM16 has not yet been established at that point.

**Crew requests** (`0x005CFA60`). If the native detail gate at `0x005CFA6F` is below 3, make no crew requests.
Otherwise draw a ground-attempt count in **[2,4)**. On each attempt, search the eight candidate rectangles
in a freshly randomized iterator order for one whose x and z spans are both strictly greater than 2.0.
Stop ground attempts when none qualifies. Choose a model uniformly from `0x0210B000`, `0x0210C000`,
`0x0210A000`, and request one ground worker in the qualifying rectangle. Then draw **1 or 2** roof attempts;
before each, require the building rectangle's x and z spans to exceed 2.0, choose between `0x02109000` and
`0x0210D000`, and request a roof worker there. A null creation does not undo the rectangle reservation or
stop subsequent attempts. The count bounds are half-open for the ground draw: the integer random helper
uses the high half of the random-word product, with no added endpoint (`0x009136B1`–`0x009136DE`). The
model tables are `0x00AA1E68`–`0x00AA1E78`; span comparisons are `0x005CFB48`–`0x005CFB70` and
`0x005CFBE0`–`0x005CFC04`.

For **each request**, CreateAutomata (`0x005CE580`) does the following in order:

1. Fetch the current city's pedestrian manager (city slot 44, `+0xB0`). For input rectangle R, draw x in
   `[R.minX + 0.5, (R.maxX − 0.5) − 1.0)` and z analogously, using double range arguments; round each
   result to `f32`. Reserve the 1 × 1 rectangle from `(x,0,z)` to `(r24(x+1),1,r24(z+1))` by RemoveRectangle.
   The subtractions occur in the written order, before conversion to the double range arguments
   (`0x005CE59A`–`0x005CE62F`); the constants are `f32` 0.5 and 1.0.
2. The spawn x and z are the midpoints of those already rounded rectangle endpoints, each rounded to
   `f32`; spawn y is the terrain height queried at those rounded midpoint coordinates (terrain slot 13,
   `+0x34`), rounded to `f32`. Roof workers also start at this terrain height. The saved roof marker
   controls a later scaffold-height update (`0x005CE634`–`0x005CE677`).
3. Call pedestrian-manager slot 15 (`+0x3C`) with **seven arguments**: group `0x4101`, spawn-position
   reference, flags `0x32000`, explicit model-key reference, null callback, priority `0x32`, null orientation.
   The key's logical type is `0x49A593E7`, group is `0x29A5D1EC`, and instance is the selected model ID.
   Its actual Windows argument layout and the callee's 28-byte cleanup are in automata.md §7.6.
4. On null, return without a list entry or AddRef. On success A, call **A slot 30** (`+0x78`) with `(2,false)`,
   generate a random horizontal direction and call **A slot 14** (`+0x38`) with its three-`f32` orientation,
   then **A slot 1** (`+0x04`) AddRef. Append A to the construction occupant's list with an initial false
   first marker and the request's ground/roof marker. No QI, AsOccupant or Release occurs in this caller
   during successful creation (`0x005CE6B1`–`0x005CE755`). The manager's returned pointer to A is borrowed;
   this AddRef establishes the separate construction-owner reference.

**After setup.** SetScaffoldHeight obtains each roof worker's position through A slot 7 (`+0x1C`, one
output-vector pointer), replaces **only y** with the scaffold height, and calls A slot 8 (`+0x20`, one
input-vector pointer). Ground workers are skipped (`0x005CC394`–`0x005CC3C8`). During Shutdown, for each
stored worker in list order, call the live pedestrian manager's slot 8 (`+0x20`) with `(A,false)` if that
manager exists; then Release A once, even if the manager is absent. Finally free the list entries
(`0x005D0FF5`–`0x005D1075`). This removal request and Release are distinct events; automata.md §7.6 specifies
the native manager's deferred-removal behaviour and additional embedded-effects dependency.

**Worked starting-site case.** Permit the detail gate, supply a valid lot/building, and arrange the draws
to select two ground rectangles and one roof rectangle, all wider/deeper than 2.0. For the first ground
request let R span x 10–20 and z 30–40, with draws x = 12, z = 34 and terrain height 7. The reserved rectangle
is `(12,0,34)`–`(13,1,35)`; expect position `(12.5,7,34.5)`, group `0x4101`, flags `0x32000`, priority 50
and model instance `0x0210B000` when its model draw selects index 0. Expect flag-clear, orientation and
AddRef in that order before the next worker request. With all three requests successful and no other
owners, each worker ends setup with one manager-list reference and one construction-owner reference;
there is no extra caller reference from slot 15. Complete the rest of SetLot, including dust creation.
At scaffold height 9, only the roof worker's y becomes 9; x and z survive. Shutdown requests removal and
drops the construction reference once per worker. The automata-owned continuation and null/mixed-native
test cases are in automata.md §7.6.

**Reading the reported crash.** `SimCity 4.exe+0x1D09CA` is the SetLot continuation after the crew call.
`0x29A5D1EC` is also the model key's group word, stored as data at `0x005CE698`; it is not an interface ID
or a callable entry. This identifies a value to check when testing argument layout, stack cleanup and
pointer lifetime, but does not establish the replacement DLL's fault. NM17 answers the native contract;
the stopped full-profile action acceptance still requires an engine run with a conforming implementation.

---

## 4. Records

Both records are written through the component's cIGZSerializable (`+8`). Stream slots: 7 `SetUint16` /
`GetUint16`, 9 `SetUint32` / `GetUint32`, 18 `GetError`. A ref is written with `SC4WriteGZSerializable`
(`0x005D99A0`) and read with `SC4ReadGZSerializable(stream, iid, &ptr, 0)` (`0x005D9730`) (foundations §2.7).

### 4.1 Component 53 (network manager)

| # | Field | Type |
|---|---|---|
| 1 | version | u16 = **1** |
| 2 | count | u32 |
| 3 | `count` × ref | network occupant, read with IID `0xA821EF94` |

**Write** (`0x006111E0`): the fields in order, `managed` in vector order. Returns `GetError() == 0`.

**Read** (`0x006124C0`): false if the stream is already in error; false unless the version is 1; then `count` refs,
each appended to `managed` (the vector is **not** cleared first). Any failed ref read returns false at once. Then
returns `GetError() == 0`.

What counts as a failed ref read (NM6, `0x00612521`–`0x0061252B`, helper `0x005D96C0`) *[compat]*:

- A **null reference** (saved object ID 0) is a successful read. The helper returns true with a null pointer
  (`0x005D96F2`–`0x005D9713`), and the null is appended to `managed` like any other entry.
- A reference the stream cannot resolve (an unknown class, a failed creation; foundations §2.7) is a failed read,
  and so is an object that does not answer QueryInterface for `0xA821EF94`. The read returns false at once
  (`0x00612583`). Nothing is released or removed: the entries appended so far stay in `managed`.

Worked example: an empty list is `01 00 | 00 00 00 00` (6 bytes).

### 4.2 Component 11 (network lot manager)

| # | Field | Type |
|---|---|---|
| 1 | version | u16 = **2** |
| 2 | lot count n | u32 |
| 3 | n × { key (u16), occupant ref, configuration ID (u32) } | ref read with IID `0xA821EF94` |
| 4 | pothole count m | u32 |
| 5 | m × { key (u16), pothole prop ref } | ref read with IID `0x09820792` |

**Write** (`0x0060D6B0`): `lots` and then `potholes`, each in its hash map's iteration order. The configuration ID is
the live configuration's `id` (configuration slot 4), not the `configId` field. Returns `GetError() == 0`.

**Read** (`0x0060DAC0`): false unless the version is 2 (nothing else is read). For each lot entry: create a network
lot (AddRef), read the key, the occupant ref into `occupant`, the u32 into `configId`, and store it as `lots[key]`
(an existing entry is overwritten without release). For each pothole entry: read the key and the ref, and store
`potholes[key]`. A failed ref read does not stop the loop. Returns `GetError() == 0`. The configurations are attached
in `Init` (§3.2 step 5).

Worked example: a city with no network lot and no pothole saves `02 00 | 00 00 00 00 | 00 00 00 00` (10 bytes).

---

## 5. Network flags

A network occupant's flags are a u32 (cISC4NetworkOccupant slots 8 `HasNetworkFlag` (all bits), 9
`HasAnyNetworkFlag`, 10 `SetNetworkFlag`, 11 `ClearNetworkFlag`, 12 `GetNetworkFlag`). The low 13 bits are the
network types (`1 << type`, table `0x00AA79E4`). Masks used below: `0x1F4F` the surface networks (all types but
pipe `0x10`, power line `0x20` and subway `0x80`), `0x1FFF` all types, `0xB0` the underground ones (pipe, power line,
subway), `0x90` pipe and subway, `0xD` road, highway and street.

| Bit | Meaning | Set by | Read by |
|---|---|---|---|
| `0x0001`…`0x1000` | network types | the tools | everyone |
| `0x2000` | path connections exist | traffic-network.md §7.2 | the occupant |
| `0x4000` | when set, the occupant also calls its slot 58 (`SetTranslucency(1, 0x80)`) | — | `SetNetworkFlag` (`0x00619EC0`) |
| `0x10000` | pipe **distressed** (leaking) | plumbing (`ApplyPipeFunding` `0x006DD110`, its `Read` `0x006DB620`) | the pipe model choice (`0x0065A880`) only |
| `0x20000` | pipe **burst** | plumbing | the pipe model choice only |
| `0x40000` | the piece is on a **bridge step** (abutments included) | the tools (network-geometry.md §8.1) | traffic-network.md §2.2 (record flag bit 0), the power wire bits, the water display handler (§6.1) |
| `0x80000` | the piece is a **tunnel portal** | the tools (network-geometry.md §8.1) | the terrain handler skips it for kind 6 (§7.6); road style skips it (§8.7); `DestroyOccupantsAndResolve` |
| `0x100000` | part of an intersection or complex piece | the tools | `RemoveOrReplaceOccupant` (§7.4), `GetAllIntersectionPieces` (§7.9) |
| `0x200000` | lot **connection node** | `CreateConnectionNode` (§7.7) | excluded from the traffic graph (traffic-network.md §4.2), from network lots (§8.5) and from road style |
| `0x400000` | **immovable** (slots 17 and 18): a prebuilt-model piece the tool did not place (traffic-network.md Q3) | the prebuilt-model constructor | `JoinAcrossRail` (zones.md §5.2.2) |
| `0x2000000`, `0x4000000` | set together by road style when an under-texture is applied (§8.7); cleared by `CreateConnectionNode` and the retrofit | | *[inferred: presentation]* |
| `0x8000000` | set by `CreateConnectionNode` and the retrofit | | *[inferred: presentation]* |
| `0x10000000` | **usable** (slots 19 `IsUsable` and 20 `SetUsable`) | the tools when construction ends | the network lot manager (§8.5), road style |
| `0x40000000` | **elevated**: the piece has support structures | `InsertNetworkOccupant` (network-geometry.md §10.2) | road style skips it |
| `0x80000000` | a **draft** (preview) occupant | `CreateNetworkOccupant` in draft mode (network-geometry.md §10.1) | the traffic simulator's subway count (traffic-network.md §3), `UpdateRoadsInCell` skips it |

**The usable message** *[compat]*. `SetNetworkFlag` (`0x00619EC0`): if the new bits contain `0x10000000`, the
occupant did not have it, and the occupant is in the city (cISC4Occupant `GetFlags() & 1`), it posts (queued,
message-server slot 4) `0x89D3E9F3` *kSC4MessageNetworkOccupantUsableChange* with IGZUnknown = the network
occupant and data1 = 1. `ClearNetworkFlag` (`0x00619F70`) posts the same message with data1 = 0 when it clears
`0x10000000` that was set. That clear test does not check `GetFlags() & 1`. Both then apply the bits.

**There is no network flag 2 "riot-blocked".** Bit 2 is rail. The riot changes the occupant's removal counter
instead (§10).

**The pipe flags.** `0x10000` and `0x20000` are read only by the plumbing simulator and by the visual model choice of
the underground occupants (vtables `0x00AA97B8`, `0x00AAA3C8`, `0x00AAAD40`). Their slots 10 and 11 (`0x0065A900`,
`0x0065A940`) mark the model dirty when either bit changes. The model is burst if `0x20000` is set, else distressed if
`0x10000` is set, else normal. No other simulation reads them. The plumbing rules are plumbing.md's.

---

## 6. Messages

### 6.1 Network manager (`DoMessage` `0x00613950`)

| Message | Handling |
|---|---|
| `0x26D31EC1` PostCityInit | `Retrofit()` (§7.10). Then, if the 24-hour clock's `IsNight` (clock slot 28), `SetLit(true)` (slot 61) on every `managed` occupant whose `CanBeLit` (slot 62). |
| `0x26D31EC2` PreCityShutdown | While `crews` is not empty, call slot 5 of its first entry (`0x00610B80`; the crew is expected to unregister itself, §7.12). Then null all 13 tools. |
| `0x99EF1142` occupant inserted | `n := void1.QueryInterface(0xA821EF94)`; none → done. If `n.IsManagedGlobally()` (slot 5): append `n` to `managed`, keeping the reference (**no duplicate check**). Otherwise release it. |
| `0x99EF1143` occupant removed | `n` as above; none → done. If `n.IsManagedGlobally()` and `n` is in `managed`: release that entry, move the **last** entry into its place and drop the last. Release `n`. |
| `0xC9DA96E9` dusk | `SetLit(true)` on every `managed` occupant that `CanBeLit` (clock24.md). |
| `0xC9DA96E8` dawn | `SetLit(false)` on the same set. |
| `0xA6B79602` view changed | If `void1+4 ≠ void2+4`: `DoViewRotationChange` (presentation). |
| `0x6AB97795` water surface display changed (posted only by `cSTETerrain::SetWaterSurfaceDisplayState` `0x007434E0`) | For every network occupant (occupant manager iteration by type `0xC772BF98`) with network flag `0x40000` and occupant flag 4 (cISC4Occupant slot 32): post `0x99EF1145` for it (`0x00610BB0`; presentation refresh). |
| `0xEA3B0AF1` long terrain operation begins | `dirty := empty`; `longTerrainOp := true`. |
| `0x278E6003` terrain changed | §7.6. |
| `0x6A3B0AFD` long terrain operation ends | §7.6 (the underground refit). |

Only these 11 types are subscribed. In the queued delivery order, a `0x99EF1142` for a globally managed piece always
precedes its `0x99EF1143`, so `managed` holds exactly the managed pieces in the city, plus duplicates if one piece
is inserted twice.

**Which occupants are managed globally** (slot 5, per occupant vtable): the prebuilt-model pieces (`0x00AA7038`), the
bridge pieces (`0x00AA7300`) and the tunnel pieces (`0x00AA9308`) answer true. The base occupant (`0x00AA8080`) and
the underground pipe and subway occupants answer false. Only bridges answer true to `CanBeLit`.

### 6.2 Network lot manager (`DoMessage` `0x0060FC30`)

| Message | Handling |
|---|---|
| `0x89D3E9F3` usable changed (§5) | `n := IGZUnknown.QueryInterface(0xA821EF94)`; none → done. Unless `n.HasAnyNetworkFlag(0x1F4F)` and not `n.HasNetworkFlag(0x200000)`, just release. If data1 ≠ 0: cell := `city.PositionToCell(n.AsOccupant().GetPosition())`; `UpdateRoadsInCell(x, z)` (§8.5). Else `DoNetworkOccupantRemoved(n)` (§8.4). Release `n`. |
| `0x99EF1143` occupant removed | `n := void1.QueryInterface(0xA821EF94)`; with the same flag filter: `DoNetworkOccupantRemoved(n)`. Release `n`. |
| `0x07699455` zoning changed | rect := `*data2` (a cell rect) grown by one cell on every side. For every cell of it, row by row, inside the city: add its key to `pending`. Then `ProcessUpdateList()` (§8.5). |
| `0xE8BDA2E7` lot state changed | `lot := IGZUnknown.QueryInterface(0xE818FCED)`; none → done. rect := `lot.GetBoundingRect()` (lot slot 14) grown by one cell. For every cell of the **boundary walk** of §6.3 (not the rect's border: the walk starts at cell (0, 0)) inside the city: add its key to `pending`. `ProcessUpdateList()`. Release the lot. The data fields (old and new state) are not read. *[quirk]* (answers NM10, NM11) |
| `0x69247DC8` road damage changed (traffic-simulator.md §3.6) | `DoRoadDamageChange(x = data1, z = data2, damaged = data3 low byte)` (§8.8). |
| `0xA9B6C2D6` | `CreateLotObjects(void1)` (§8.6). *[quirk]* No code in the executable posts this message. |

There is no handler for occupant **inserted**: a network lot appears when the piece becomes usable, not when it is
placed.

### 6.3 The lot-state boundary walk (answers NM10, NM11)

The `0xE8BDA2E7` handler builds its boundary iterator on the stack with a constructor (`0x0060C930`) that sets the
current cell to **(0, 0)**, the state to fresh and the step to 1, and then sets only the rectangle (`0x0060C960`:
x0, z0, x1, z1 from the grown rect, then `x1 := max(x1, x0)`, `z1 := max(z1, z0)`). Nothing moves the current
cell to (x0, z0) (the zoning handler, by contrast, does: `0x0060FD9B`). `GetNext` (`0x0060CEE0`) then works from
(0, 0) *[quirk]*:

1. Fresh: return the current cell, state := running. The first cell is always (0, 0).
2. Running: `x += step`. If `x ≤ x1`: return (x, z).
3. Otherwise, if `z ≥ z1`: state := done, return false. Otherwise `x := x0`, `z += 1`, and
   `step := max(x1 − x0, 1)` if `z < z1`, else `step := 1`; return (x0, z).

So for a grown rect {x0, z0, x1, z1} with z1 ≥ 1 the visited cells are, in this order:

| Part | Cells |
|---|---|
| row 0 | (0, 0), (1, 0), …, (x1, 0) (step 1; (0, 0) even if x1 < 0) |
| rows 1 … z1 − 1 | (x0, z), then (x1, z) if x1 > x0 |
| row z1 | (x0, z1), (x0 + 1, z1), …, (x1, z1) |

Each visited cell with `0 ≤ x < width` and `0 ≤ z < height` (city slots 93/94, `0x174`/`0x178`) puts key
`x << 8 | z` in `pending` (`0x0060FF90`–`0x0060FFBF`). Consequences:

- **The first row's inner cells are never visited** when 1 ≤ z0 < z1: row z0 gets only (x0, z0) and (x1, z0).
  (When z0 ≤ 0, row 0 is the full walk from x = 0 to x1, so the inner cells of a first row at z = 0 are visited.)
- **Columns x0 and x1 are visited from row 1 down to the rect**, and row 0 from x = 0 to x1, so a lot state
  message re-evaluates cells far north of the lot: any cell (c, r) with r ≥ 1 is visited by every message whose
  grown rect has x0 = c or x1 = c and z1 > r; every cell (c, 0) with c ≤ x1 is visited. Cell (0, 0) is visited by
  every message.
- With the full rect row z1 the walk still covers the left, right and bottom sides of the border; only the top
  side is replaced by row 0.

Example: lot cells 15..16 × 94..96, grown rect {14, 93, 17, 97}. Visited: (0..17, 0); (14, z) and (17, z) for
z = 1..96; (14..17, 97). Cells (15, 93) and (16, 93) (the top border's inner cells) are not visited; (14, 40) and
(17, 40) are.

The walk does not change which keys `ProcessUpdateList` drains, only which keys are added; keys already in
`pending` are not duplicated (hash set).

---

## 7. Network manager behaviour

### 7.1 GetNetworkTool(type, unique) (slot 5, `0x006108D0`)

- `type ≥ 13` (unsigned) → null.
- `unique = false` → `tools[type]`, borrowed (no AddRef). It is null before `Init`, after PreCityShutdown and after
  `Shutdown`.
- `unique = true` → a **new** tool of that type (the same classes as `Init`). Null if allocation fails. The new
  tool's reference count is **0** (`+0x14`, cleared by the base constructor `0x0062C5F0` at `0x0062C617`, which types
  0, 3 and 11 call directly, `0x0061090B`, `0x00610928`, `0x00610A68`; the factory
  returns it with no AddRef, `0x00610907`–`0x00610910`): the caller must AddRef it, and its last Release (tool
  slot 2 `0x00623270`: a count of at most 1 runs the deleting destructor) frees it. The zone developer's AddRef at
  PostCityInit is the only reference, so its `Shutdown` Release destroys the tool (zones.md §1.2, ZG39).

zones.md §5.2 takes three unique tools at PostCityInit; power.md takes the shared power-line tool.

### 7.2 DestroyNetworkOccupants(list) (slot 6, `0x00612110`)

`list` is an occupant list (B list, allocators.md).

1. If `queueing`: AddRef each element and append it to `pending`. Done.
2. Otherwise, for each group in this order (tables `0x00AA7B40` / `0x00AA7B34`): mask `0x1F4F` with tool type 0,
   mask `0x10` with tool type 4, mask `0x80` with tool type 7:
   1. Collect, in list order, each element whose network occupant (QI `0xA821EF94`) has `GetNetworkFlag() & mask`
      ≠ 0, AddRef'd.
   2. If any: `t := GetNetworkTool(type, false)`; AddRef; `t.Init()`; `t.DestroyOccupantsAndResolve(collected)`
      (tool slot 50, `0x0062A5C0`); `t.Shutdown()`; Release.
   3. Free the collected vector **without** releasing its elements *[quirk: leak of one reference each]*.

An element of several groups (a road over a pipe is two occupants, so this does not arise in practice) would be
passed to several tools. Power lines (`0x20`) are not handled here: demolition removes power poles as plain
occupants (demolition.md §4.6). The geometry of `DestroyOccupantsAndResolve` (rebuilding the neighbours' pieces) is
the network tool's.

### 7.3 QueueRemovals(on) (slot 7, `0x00613450`)

1. If `on ≠ queueing`: `queueing := on`.
2. If `on` is false and `pending` is not empty: sort `pending` by **pointer value** (std::sort, `0x006132E0`); build
   an occupant list from it, dropping (and releasing) adjacent duplicates; clear `pending`; call
   `DestroyNetworkOccupants(list)` through the vtable (so `queueing` is false and it destroys at once); release the
   list (occupant manager slot 48).

*[quirk]* The destruction order is memory order, not the order of the requests. FreeCity4 orders by a stable
per-occupant creation number instead *[choice]*. The outcome differs only through the tool's neighbour rebuilds.

demolition.md §4.2 brackets `DemolishRegion`'s lot removal with `QueueRemovals(true)` … `QueueRemovals(false)`, so
the networks of a demolished area go in one batch, each occupant once.

### 7.4 RemoveOrReplaceOccupant(tool, occ, &replaced) (slot 8, `0x006110B0`, pop 12)

1. False if `occ` is null, if `occ.GetFlags() & 1` is 0 (not in the city), or if it is not a network occupant.
2. If it has network flag `0x100000` (a piece of an intersection or complex piece):
   1. `(x, z) := occ.GetOccupiedCell()` (slot 59).
   2. `info := tool.GetCellInfo((z << 16) + x)` (`0x00633220`).
   3. If `occupantManager.RemoveOccupant(occ, false, 0)` (slot 21) succeeds: set the tool's byte `+0x50`; call
      `tool.InsertNetworkOccupant(info, {0, 0, 0}, 0)` (tool slot 72); `replaced := true`; return true.
   4. Otherwise return false.
3. Otherwise return `occupantManager.RemoveOccupant(occ, false, 0)`.

So removing one cell of an intersection re-inserts a plain piece built from the cell's info in its place.

### 7.5 Small slots

| Slot | Behaviour |
|---|---|
| 9 `IsOccupantSubwayStation(occ)` | `cSC4SubwayOccupantFilter::IsOccupantSubwayStation` |
| 10, 11 | forward to the pipe tool's static `FindPipeOccupant` and `FindPipeConnectionOccupant` |
| 16 `RegisterConstructionCrew(c)` | AddRef and append to `crews` |
| 17 `UnregisterConstructionCrew(c)` | find, erase, release |
| 18, 19 | return a new water-occupant filter or subway-occupant filter |
| 22 `GetEnglishNetworkName(type)` | entry `type` of the table `0x00AA845C`: "Road", "Rail", "Highway", "Street", "Pipe", "PowerLine", "Avenue", "Subway", "LightRail", "Monorail", "OneWayRoad", "DirtRoad", "GroundHighway"; entry 13 is "road". No bounds check *[quirk]* |
| 23 `AddToRetrofitList(n)` | AddRef and append to `retrofit`. cSC4NetworkOccupant::Init calls it when its `CheckForRetrofit` is true |
| 24 | deleting destructor |

### 7.6 Terrain changes

**`0x278E6003`** (data1, data2 = packed cells, data4 = kind; building-development.md §4.11, landfill.md §3.2; the
kinds and their senders are listed in terrain.md §8.2):

1. `x1 := (data1 >> 16) − 1`, `z1 := (data2 >> 16) − 1`, `x2 := data1 & 0xFFFF`, `z2 := data2 & 0xFFFF`. Clamp
   `x1, z1 ≥ 0`, `x2 ≤ X − 1`, `z2 ≤ Z − 1`.
2. If `longTerrainOp`: `dirty := dirty ∪ rect` (componentwise min and max). Otherwise `dirty := rect`.
3. If `kind ≠ 5` (the levelling reason, landfill.md):
   1. Query the occupants in the rect whose network flags meet `0x1FFF` (occupant manager slot 40, an OR filter).
   2. For each, in query order, keep it unless:
      - `kind = 4`; or
      - `kind ≠ 3` and it `IsOfType(4)` (pipe) or `IsOfType(7)` (subway); or
      - **no corner mismatch** (below); or
      - `kind = 6` and it has network flag `0x80000`.
   3. `DestroyNetworkOccupants(kept)` (§7.2), then release the lists.
4. If not `longTerrainOp`: run the underground refit below at once.

**Corner mismatch** (`0x006105F0`). For the occupant's cell `(cx, cz)`, `X0 := float(cx × 16)`,
`Z0 := float(cz × 16)`. For each corner `(X0, Z0)`, `(X0 + 16, Z0)`, `(X0, Z0 + 16)`, `(X0 + 16, Z0 + 16)`:
`d := r24(occ.GetAltitude(x, z) − terrain.GetAltitude(x, z))` (occupant slot 56, terrain slot 13, both float). The
corner mismatches if `d < 0.0f` or `d > 0.01` (the double `0x3F847AE147AE147B`). A NaN `d` does **not** mismatch
*[compat]*. One mismatching corner is enough.

So a network that the terrain edit left floating by more than 1 cm, or buried at all, is destroyed. A pipe or subway
survives every kind but 3.

**Precision boundary (NM18).** The helper explicitly stores the occupant's altitude as binary32,
then obtains the terrain altitude in ST0 and subtracts it from that saved value. It does not insert
another binary32 store of the terrain result or of the difference before comparing with zero and
the binary64 0.01 constant (`0x00610620–0x00610653`, repeated through `0x00610730`). Here `r24`
denotes the game's x87 arithmetic precision, not permission to add host float stores at every
interface return. A numeric probe must retain the returned terrain value at its actual precision
and the x87 control word, as well as the saved occupant f32. Corners are tested in the order above,
and evaluation stops at the first mismatch.

#### 7.6.1 Direct decisions versus tool removals (NM18) *[compat]*

The filtered list in step 3 is the **seed input to destruction**, not an enumeration of every old
occupant that will cease to be placed. The manager makes the slot-6 call at `0x00613C46–0x00613C4B`;
§7.2 dispatches each nonempty layer group to tool slot 50. The downstream contract is
[network-tools.md §10](network-tools.md#10-destroyoccupantsandresolvelist-slot-50-0x0062a5c0):

- closure can add adjoining immovable prebuilt pieces, all pieces of a selected intersection/complex
  structure, and the paired end of a tunnel;
- destruction and the neighbour solve can remove an **old** neighbour occupant and insert a new one
  at its cell, even though that neighbour passed the direct terrain test;
- the removal notifications and resulting network-lot teardown are effects of these tool operations,
  not a second evaluation of this section's corner predicate.

Thus “placed at the terrain query, absent after the handler” proves an old object's removal, but
does not prove that the direct filter selected it, that its cell is now empty, or that another terrain
listener selected it. Keep old-object identity and cell occupancy as separate observations. The
query/filter is still the one in §7.6; total removal ownership extends through §7.2 into the tool.
The underground-refit path below is another nested tool operation and must be distinguished from
the direct surface destruction call.

**Worked comparison case.** Let ordinary adjacent surface pieces A and B both be present at query.
Only A has a mismatching corner. Pass [A] to surface tool 0. In the ordinary neighbour-rebuild case
of network-tools §10, A is removed, old B is removed during rebuilding, and B′ is inserted at B's
cell. Before/after placement for old B is 1/0; B's direct decision is still “keep”, while its cell
contains B′. Expect removal notifications for A and B and an insertion for B′. A direct-decision
comparison compares [A] with the slot-50 seed, while a downstream tool comparison also expects B's
replacement. Neither comparison may count this as an unexplained extra direct selection.

**NM18-A — remaining Tegel observation.** The supplied `netocc-cities29-bound8-default` result
(10 remaining mismatches) and `bound8-grow` result (12) do not contain the tool's closure/solve
provenance, so static analysis cannot assign each of their 1/0 cases to one of these paths.
Retain the mismatch until one run records, within each terrain-message delivery:

1. Message ID, all four data words, long-operation/dirty state, ordered query identities and flags,
   cell and **the altitude inputs before the native handler consumes or changes them**. A query-boundary
   capture alone is insufficient if an earlier nested tool operation changes a later candidate's inputs;
   record that candidate's actual helper calls before the first destructive tool call too.
2. Each layer's ordered slot-50 seed vector, and membership added by closure, with source identity
   and reason (neighbour/prebuilt, structure, tunnel pair).
3. Every occupant-manager removal and insertion, old/new identity and cell, with its active tool phase
   (initial destruction, remove/replace, solve/insertion, underground refit) or other caller and message nesting.

For cells (65..66,69..70), (65..66,77), (80..81,69..70), the first removal of each retained old
identity settles whether it is a direct seed, closure/rebuild effect or another native caller. If
the direct seed itself differs, replay the captured corner operands and precision first. Checking
only at a later listener, moving to the next tick, or inferring closure merely from the cell pattern
does not settle NM18-A. The known earlier 1/1 timing cases are separate evidence.

**`0x6A3B0AFD`** (and the tail of step 4), the **underground refit**:

1. If `dirty` is not empty (`x1 ≤ x2` and `z1 ≤ z2`, signed, `0x00613C94`–`0x00613CA6`): query the occupants in
   `dirty` with flags meeting `0x90` (occupant manager slot 40). If the query returns false, skip to step 3.
   Otherwise split them, in query order: for each occupant and each type `t` = 0 … 12 it `IsOfType` (slot 22), its
   cell (slot 59) is appended to a per-type cell list; an occupant of type 4 (pipe) or 7 (subway) is also AddRef'd and
   appended to that kind's occupant list. Each occupant is released once after its 13 tests.
2. **For each kind whose occupant list is not empty** (NM9; pipes `0x00613E53`–`0x00613E59`, subways
   `0x00613E9B`–`0x00613EA1`): `GetNetworkTool(4, false)`, `Init`,
   `UpdateUndergroundOccupantTerrainHeights(cells, list, 4)` (`0x006588D0`), `Shutdown`. The same for the subways
   with `GetNetworkTool(7, false)` and 7. A kind with no occupant in `dirty` gets no call at all, not a call with an
   empty list.
3. `dirty := empty` (`x1 = z1 = 0x7FFFFFFF`, `x2 = z2 = −1`).
4. Always `longTerrainOp := false`.

### 7.7 CreateConnectionNode(&out, type, &p, edgesA, edgesB, pathOverride, c) (slot 20, `0x00611300`, pop 28)

Used by the lot developer for transit lots (lot-developer.md, `AddNetworkConnectionNodes`).

1. Tool `tools[type]`: `Init`; `PlaceNetworkOccupantById(p.x, p.z, 0x100, false, type)` (tool slot 8); `Shutdown`.
2. Find the occupant at `p` with an OR filter for `1 << type` (occupant manager slot 32). None → return false.
3. `n :=` its network occupant.
4. If the lot at `p` (lot manager slot 6, `GetLot(x, z, false)`) has a building (lot slot 48): set all four vertex
   heights of `n` (slot 36) to the height the tool computes (tool slot 74 of `GetNetworkTool(type, false)`).
5. If `pathOverride ≠ 0`: `n.SetConnectionPathIDOverride(pathOverride)` (slot 16) and
   `n.SetQuadDraw(n.PieceId(), c)` (slot 30).
6. Set flags `0x200000` and `0x8000000`; clear `0x2000000` and `0x4000000`; `SetUsable(false)` (slot 20).
7. `n.SetConnectionNodeEdgeTypes(edgesA, edgesB)` (slot 14).
8. `out :=` the occupant, AddRef'd. Return true.

A connection node carries `0x200000`, so the traffic graph, the network lots and road style all ignore it.

### 7.8 AutoconnectLotNodes(lot, nodes) (slot 21, `0x00611530`)

`rect := lot.GetBoundingRect()` (lot slot 14). For each node in `nodes`, in order:

1. `t :=` the first type with `HasNetworkFlag(1 << t)`.
2. `(x, z) := GetOccupiedCell()`.
3. `(a, b) := GetConnectionNodeEdgeTypes()`. `dirs :=` the set of sides `d` (0 W, 1 N, 2 E, 3 S) whose byte `d` of
   `a` has any of its low 3 bits set, minus the low byte of `GetActiveEdges()` (slot 40).
4. For each `d` in `dirs`: `(nx, nz) := (x + DX[d], z + DZ[d])` (DX = −1, 0, 1, 0; DZ = 0, −1, 0, 1; tables
   `0x00AA7A18` / `0x00AA7A28`). *[quirk]* For `d ≥ 2` only, a neighbour inside `rect` is skipped. If an occupant of
   the node's surface or underground kind (`0x1F4F` or `0xB0` filter) exists at the neighbour, walk on in direction
   `d` through the lot's cells while occupants keep being found; the end cell is the last one found.
5. Place from the node's cell to the end cell: `tools[t]` AddRef, `Init`, `PlaceNetworkOccupants(x, z, ex, ez, 0,
   null, true, −1)` (tool slot 7), `Shutdown`, Release. These are code placements (`data4 = 3`, zones.md §5.2).

### 7.9 GetAllIntersectionPieces (slot 12, `0x00612780`; helper `0x00612300`)

A recursive flood over the 4-neighbours inside the city from the start piece's cell. It collects every network
occupant (filter `0x1F4F`; a null occupant manager argument means the global one) that has flag `0x100000` and
`IsPartOfTheSameStructure(start)` (slot 66). The result is the set of cells of one intersection or complex piece.

#### 7.9.1 Typed lookup and flood ABI (NM22) *[compat]*

Manager slots 10 and 11 (`00610550`, `00610570`) are thiscall, pop **12**, Boolean result in **AL**:
`FindPipeOccupant(int32 x, int32 z, cISC4PipeOccupant** out)` and
`FindPipeConnectionOccupant(int32 x, int32 z, cISC4PipeConnectionOccupant** out)`. The manager receiver is
unused; each forwards the three arguments unchanged to the caller-cleaned static pipe-tool helper
`00648F70` or `00649080`. Coordinates are standard city **cells**, not world-position floats.

The helper builds a local occupant list, queries global occupant-manager slot 40 (`+0xA0`) with
`(outList, &cell, &cell, nullFilter, FFFFFFFF)`, and **then** writes `*out=null`. The cell is the pair of the
two input int32 arguments. Slot 40's coordinate clamping, strict overlap, list order and acquired references
are occupant-manager.md §8.2; this helper adds no bounds or layer gate. For each returned occupant, in list
order, QI for **`49C05B9F`** (pipe) or **`EA0A5ACF`** (pipe connection). On success release any previously
selected interface, replace `*out` with the newly acquired interface, and set the Boolean result true.
Release each query-list occupant whether or not its QI succeeded, and free the list nodes afterward.
Consequently the **last successful QI wins**; there is no early return and no separate flag/type test
(`00649020–00649068`, corresponding connection-helper sequence at `00649080+0xB0`). On no match the result
is false and output null. The incoming value of `*out` is overwritten without being released; the caller
must pass an output slot whose previous ownership it already handled. A successful output owns **one QI
reference**, which the caller releases. The helper does not return an occupant pointer in EAX.

Manager slot 12 is thiscall, pop **16**, Boolean AL (normally true), with arguments in this order:

| Argument | Type / meaning |
|---|---|
| 1 | `cISC4NetworkOccupant* start`, required |
| 2 | caller-owned `ilist<cISC4Occupant*>* out`, required; append to existing contents |
| 3 | `cISC4OccupantManager* queryManager`, null selects global occupant manager |
| 4 | `cISC4NetworkOccupant* structureOrigin`, null selects `start` |

The list is the native sentinel doubly linked list of occupant pointers (node next/previous/value at
`+0/+4/+8`), not a cell set, vector or returned-list pointer. First allocate/retain an OR network filter
with mask `1F4F`, get start's `AsOccupant` (network slot 48 / `+0xC0`), and scan the existing output for
that exact pointer. If already present, return true without extending the flood. Otherwise recurse
(`00612780–0061281E`, helper `00612300`). Each recursion:

1. AddRef its network interface once, append its borrowed `AsOccupant` to output, and obtain its cell
   with network slot 59 (`+0xEC`). That reference is the output entry's ownership, through the shared
   occupant/network count. The seed is included without testing `100000` or comparing it with origin.
2. Visit neighbours **west, north, east, south**: `(-1,0),(0,-1),(1,0),(0,1)` (tables `00AA7A18/00AA7A28`).
   Reject coordinates outside unsigned city width/height, then call query-manager slot 32
   `GetFirstOccupantByStandardCityCell(&occupant,x,z,filter)`.
3. On a hit, QI that first occupant for network IID `A821EF94`; require its network flag `100000`;
   skip if its `AsOccupant` pointer is already in output; then call its slot 66 (`+0x108`)
   `IsPartOfTheSameStructure(structureOrigin)`. Only on true recurse immediately, before the next neighbour.
   A non-network first hit does not cause a second spatial search. Release both local query and QI
   references after this branch, including after recursion (`006123B6–00612464`).
4. Retain the output entries; return true. The outer operation releases the filter. The caller releases
   **one occupant reference per newly appended entry** and owns the list storage. Neither input manager
   nor structureOrigin is transferred or independently retained by the outer operation.

Example: with an initially empty output and seed A at (5,5), qualifying B west at (4,5) and C north of B
at (4,4), traversal appends **A,B,C** before considering A's north neighbour. A repeated call with A and
that same list appends nothing. A pipe lookup whose list order is P,Q,R and only P/R accept the pipe IID
returns R with one owned reference, after releasing P's selected reference and all three list acquisitions.
These lookups/floods draw no CRT `rand()`. Allocation failure in the outer flood returns true without a
new flood; it is not an empty-success guarantee for pre-existing output contents.

### 7.10 Retrofit (`0x00612820`, at PostCityInit)

For old saves: the pieces that asked for it at `Init` (§7.5 slot 23).

For each `retrofit` entry whose `CheckForRetrofit()` (slot 6) is still true:

1. Gather its structure's pieces (slot 12).
2. Find the origin piece through `GetComplexPieceOrigin` (slot 71) and the occupant there.
3. Look up its old piece ID in the 11-entry table `0x00AA7B50`, which maps the old highway interchange IDs
   `0x02010000`–`0x02012300` to the new `0x0C000000`–`0x0C00B000`.
4. For each piece, from its offset within the bounding box:
   1. Attach the new prebuilt model (exemplar `0x6534284A / 0xA8434037 / instance`).
   2. Set its path override.
   3. Clear flags `0x2000000` and `0x4000000`; set `0x8000000`.
   4. `SetRetrofitFlag(false)` (slot 7).
   5. Post `0x99EF1145` for it.

Then empty the list (`0x00611A20`). The model and path work is presentation and path data.

The list's references (NM7) *[compat]*:

- An empty list returns at once (`0x0061283A`); nothing is created or released.
- Otherwise every entry is **released once inside the walk**, right after its `CheckForRetrofit` test and the
  retrofit, **whether or not it still asked** (`0x00612DB3`; the false branch at `0x006128E0` jumps straight there).
  This balances the AddRef that slot 23 took.
- `0x00611A20` then frees the list nodes only (through the allocator service `0x3AE4BEA3`/`0x3AE4BEAB`, slot 4); it
  releases nothing. So the list is always emptied, and each entry loses exactly one reference. FreeCity4 loads only
saves written by 1.1.641, whose pieces need no retrofit *[choice]*. It keeps the list and slot 23 for
compatibility.

### 7.11 Presentation slots

Slot 13 `MapPreviewTexture`, 14 `MapShadowTexture` and 15 `MapPath` look up the remap tables of §2.1. `MapPath`
applies the UK path remapping only when `ukPaths` is set. Slot 15 changes which path IDs the occupants use (left-hand
traffic), but the traffic graph itself is traffic-network.md's.

### 7.12 Construction crews

**Creation and interfaces (NM14).** A crew is a directly allocated, 84-byte helper, not an automaton or a
GZCOM-created class. `cSC4NetworkTool::PlaceAllSegments` (`0x0063BDC0`) creates one when its construction
argument is true and the network type is 0–3, 6 or 8–12. Types 4, 5 and 7 do not take this branch. It allocates
84 bytes through the game heap (`0x0063BE07`–`0x0063BE17`), invokes the constructor `0x006070C0`, AddRefs
the result and calls slot 4. No factory/CLSID request occurs. Allocation failure is not handled safely: the
following virtual call still uses the result. Creation failure is therefore not a supported false Init result.

The constructor sets vtable `0x00AA74AC`, reference count zero, initialized and animation-subscribed flags
false, scalar state zero and empty working containers. There is one interface pointer, at object offset zero:

| Slot | Byte offset | Contract |
|---|---|---|
| 0 | `+0x00` | QueryInterface, `0x005C3E30`: only message-target IID `0x452294AA` succeeds; returns this and AddRefs once. Other IIDs, **including IUnknown 1**, return false and leave the output unchanged. |
| 1 | `+0x04` | AddRef, folded `0x004F7BA0`, count at object `+0x10`. |
| 2 | `+0x08` | Release, `0x00608C20`: decrement above one; otherwise Shutdown, FinalRelease, game-heap free, return zero. |
| 3 | `+0x0C` | DoMessage, `0x00607ED0`. |
| 4 | `+0x10` | Init, `0x00607190`, six 32-bit stack arguments, callee pops 24 bytes; bool result in AL. |
| 5 | `+0x14` | Shutdown, `0x00607920`, no stack arguments, true in AL. |
| 6 | `+0x18` | StaticInit, `0x00605620`, sets the process-wide crew-initialized flag; true. |
| 7 | `+0x1C` | StaticShutdown, `0x00605640`, clears that flag; true. |

There is no QueryInterface route to a separate construction-crew IID. Manager slots 16/17 receive this
opaque pointer directly. Keep the native creation/Init path when only bridging registration and shutdown.

**Init arguments, in call order.** The caller copies them from the selected network-type record
(`0x0063BE29`–`0x0063BE56`); the record stride is `0x114`.

| Argument | Type / source in network-type record | Meaning and retained state |
|---|---|---|
| 1 | 32-bit integer, `+0x1C` | Network width in cells, copied to crew `+0x0C`; width 1 uses the single-width branch in BeginConstruction. |
| 2 | binary32, `+0xC8`; placement property `0x4A270DB0`, default 1.5 | Seconds per occupant. Multiply by binary32 1000.0, truncate toward zero through `_ftol2`, and retain its low 32-bit result at `+0x04` as the per-cell countdown in milliseconds. |
| 3 | binary32, `+0xCC`; `0x4A270E94`, default 32.0 | Construction-vehicle speed. Multiply by binary32 `0x358637BD` (approximately 0.000001), store binary32 at `+0x08`; the animation path uses elapsed microseconds. |
| 4 | 32-bit integer, `+0xD0`; `0x4A271248`, default 0 | Vehicle model instance ID, copied to `+0x20`, then to the first vehicle record's model field (crew-data `+0x40`, vehicle record starts at `+0x3C`). |
| 5 | 32-bit integer, `+0xD4`; `0x4A27124F`, default 0 | Second/lead vehicle model instance ID, copied to `+0x24`, then crew-data `+0x60` (vehicle record starts at `+0x5C`). |
| 6 | 32-bit integer, `+0xD8`; `0x4A275748`, default 1 | Path-type selector, copied to `+0x28` and used in the edge/path queries. It is not an object pointer or a network-type index. |

The numeric contract comes from `0x006071B9`–`0x006071F1`, constants `0x00A8FAF0`/`0x00A86510`,
and `_ftol2` (`0x009EEF04`–`0x009EEF78`), not the recovered pseudocode signature. Vehicle-model copies
are at `0x00605A88`–`0x00605AA6`; CreateConstructionVehicle consumes the vehicle record's ID at `+4`
(`0x00605740`). Property types/defaults are checked at `0x0063CF43`–`0x0063CFC9`. No argument is AddRef'd.

**Init effects, in order.** Ensure StaticInit has run, then return true immediately if this crew is already
initialized. Otherwise register this pointer with the live network manager, slot 16 (`+0x40`); store the six
scalar settings above; if application service `0x00B43CB8` is non-null, retain it at crew `+0x18` with one
AddRef. This retained reference is used as the availability test for construction notifications. Empty the
occupant work list, solved-segment vector, coordinate vector and per-crew-data vector (destroying old
crew-data elements). If the message server exists, subscribe this same message-target pointer to
`0x99EF1143` (occupant removed), then `0x26D31EC2` (PreCityShutdown), using server slot 5 (`+0x14`).
Finally set initialized true and return true. Init does not subscribe to animation ticks, create vehicles,
or begin construction. See `0x00607190`–`0x006072D3`.

**Ownership and transitions.** RegisterConstructionCrew (`0x006122B0`) appends and AddRefs once, without
deduplication. UnregisterConstructionCrew (`0x006112A0`) finds the first matching pointer, erases it preserving
the later entries' order, then Releases once; an absent pointer does nothing. The tool adds network segments,
calls BeginConstruction (`0x00608770`), and drops its own crew reference. BeginConstruction either starts
construction and subscribes to animation mask `0xC0`, or, for at most four solved-segment entries, marks the
occupants usable without animation and calls Shutdown. Init alone leaves animation-subscribed false.

Shutdown is idempotent: when initialized, clear that flag first; unsubscribe occupant-removed and
PreCityShutdown; if subscribed and the tick manager exists, clear the animation flag and unsubscribe mask
`0xC0`; stop every per-crew construction operation, mark outstanding occupants usable, empty their work list,
Release/null the retained application service, and **last** call network-manager slot 17 (`+0x44`). This can
release the manager's last reference to the crew. Release's finalization path also calls Shutdown, so the
cleared flag must prevent a second unregistration. DoMessage dispatches occupant removal, PreCityShutdown
and animation messages to their corresponding handlers (`0x00607ED0`).

At PreCityShutdown the manager calls slot 5 of the first crew until `crews` is empty (`0x00610B80`). A crew
that did not unregister itself in slot 5 would loop forever *[quirk]*. FreeCity4 bounds the loop by the
starting size *[choice]*. The manager identity must remain live during these callbacks (§3.2).

**Worked Init/Shutdown case (NM14).** Start with one tool-owned reference and an empty manager vector.
Init `(1, 1.5, 32.0, 0, 0, 1)` returns true, retains countdown 1500 and speed bits `0x380637BD`, appends
one manager reference, subscribes the two messages and leaves animation unsubscribed. A second Init with
different arguments returns true with no scalar changes, registration, AddRef or subscription. Shutdown
removes those subscriptions, releases the optional service and unregisters once; the tool reference remains.
A second Shutdown changes nothing. Releasing the tool reference then frees the crew. For a running crew
whose tool reference has already gone, PreCityShutdown instead releases the manager's final reference
through the reentrant Shutdown/Release path and must make progress to an empty vector.

---

## 8. Network lot manager behaviour

### 8.1 GetNetworkLot(x, z) (slot 5, `0x0060D650`)

`lots[(x & 0xFF) << 8 | z]`, borrowed, or null.

### 8.2 GetZoneTypeForNetwork(x, z) (slot 9, `0x0060D030`)

1. `zt :=` the zone type of cell `(x, z)` (the zone manager's zone grid, a signed byte). If `zt ≠ 0`, return it.
2. `best := 0`. Visit the neighbours in this order: W `(x−1, z)`, N `(x, z−1)`, E `(x+1, z)`, S `(x, z+1)`
   (offset tables `0x00AA7858` / `0x00AA7878`). After the fourth, if `best ∈ {0, 7, 14}` (none, agriculture,
   landfill), go on with the diagonals `(x−1, z−1)`, `(x−1, z+1)`, `(x+1, z−1)`, `(x+1, z+1)`.
3. For each neighbour inside the city with zone `n`: if `P[n] < P[best]` (strict), `best := n`.
4. Return `best`.

Priority `P` (table `0x00AA76E8`, lower wins):

| Zone | 0 | 1 R$ | 2 R$$ | 3 R$$$ | 4 C$ | 5 C$$ | 6 C$$$ | 7 I-R | 8 I-D | 9 I-HT | 10–13 | 14 landfill | 15 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| P | 18 | 4 | 7 | 6 | 5 | 4 | 3 | 8 | 2 | 1 | 16 | 17 | 16 |

Ties keep the earlier neighbour. So industry beats commerce, which mostly beats residence.

**Zone values outside 0..15** (NM8). The zone grid holds only 0..15 in a game that the zone manager wrote (zones.md
§2), so these values come only from a corrupt or foreign save. The original has no range check:

- A non-zero own-cell value is returned as it is, sign-extended (`0x0060D05D`–`0x0060D062`).
- A neighbour's value `n` (read with sign extension, `0x0060D0E7`) indexes `P` as a signed 32-bit table with no
  bound (`0x0060D0EB`, compared signed at `0x0060D0F9`). Out of range, it reads the neighbouring read-only data: from
  16 on, the bytes of the `V` and `T` tables of §8.7 and the offset tables; below 0, the data before `P`. Most such
  `n` read a large value and never win, but some read less than 18 and do win, for example `n = 28` reads 0 (the first
  entry of `T`), `n = 92` reads −1 and `n = −4` reads 2.

*[choice]* FreeCity4 treats a neighbour zone outside 0..15 as never winning, and returns an out-of-range own-cell
value unchanged. The result differs from the original only for such corrupt grids.

### 8.3 GetWealthForNetwork(x, z) (slot 10, `0x0060CBB0`)

1. `count[0..3] := 0`.
2. Visit the same eight offsets as §8.2 in the same order. Stop after the fourth if at least one of the four had a
   lot; otherwise go on through the diagonals.
3. For each neighbour inside the city with a lot (lot manager `GetLot(x, z, false)`):
   - `w := lot.GetOccupantWealth()` (lot slot 41).
   - If `lot.IsInLivableState()` (slot 37) and `w = 0`: take the building type `lot.GetBuildingType(false)`
     (slot 46). If it is non-zero and the building development simulator has a profile for it (BDS slot 40), use
     the profile's wealth.
   - If `w ∉ [0, 3]`: `w := 0`.
   - `count[w] += 1`.
4. `r := 0`. If `count[1] ≠ 0` and `count[0] ≤ count[1]`: `r := 1`. If `count[2] ≠ 0` and `count[r] ≤ count[2]`:
   `r := 2`. If `count[3] ≠ 0` and `count[r] ≤ count[3]`: `r := 3`. Return `r`. Ties go to the **higher** wealth.

### 8.4 IsLotCompatible(cfg, x, z, zone, wealth, rotation, flip) (`0x0060CD30`)

All of these must hold, tested in this order:

1. **Position**: `row := (z mod n)`, `col := (x mod n)`, with `n := cfg.period` (signed division).
   `mask[row]` must have bit `col`. The shift is done in 8 bits, so `col ≥ 8` never matches. `n = 0` divides by zero
   in the original *[quirk]*; FreeCity4 treats it as "never compatible" *[choice]*. Rows past 3 read the next bytes
   of the configuration: row 4 is `n`'s low byte and rows 5–7 are 0 *[compat]*. Rows of 8 and more are treated as 0
   *[choice]*.
2. `cfg.zones[zone]`.
3. `cfg.wealths[wealth]`.
4. `cfg.rotations` has bit `rotation` (`rotation ≥ 8` never matches).
5. `flipRule`: 1 needs `flip ≠ 0`; 2 needs `flip = 0`; other values allow both.
6. **Slope**: `s := lotDeveloper.GetAreaSlope({x, z, x, z}, true)` (lot developer slot 29, through BDS slot 36).
   `cfg.minSlope ≤ s ≤ cfg.maxSlope`, compared as floats. A NaN fails.

**Zone and wealth outside the tables** (NM8). `zones` and `wealths` are bit vectors (`+0x34`/`+0x38` and `+0x48`),
indexed with no range check (`0x0060CD66`–`0x0060CD9E`, `0x00477A40`). The constructor (`0x0060FA90`) sizes them to
16 and 4 bits through `0x0060D310`. On an empty vector that call takes its "grow" branch, which allocates one fresh
storage word and writes only the 16 (or 4) bits in use (`0x00477E70` → `0x004779E0`, a bit-by-bit fill). Nothing
later clears the rest of the word (`ReadLotConfiguration` sets bits below 16 and 4 only). So:

- zone 16..31 and wealth 4..31 read the unused high bits of that word, whose value is whatever the STL node
  allocator's block held;
- a negative value, or 32 and above, reads outside the vector's storage (heap memory).

Neither case has a defined result. The wealth argument always comes from §8.3, which clamps it to 0..3. The zone
argument comes from §8.2, so only a corrupt zone grid reaches this. *[choice]* FreeCity4 fails any zone outside 0..15
and any wealth outside 0..3, which is what the original gives when the leftover bits happen to be 0.

### 8.5 Choosing, keeping and removing network lots

**Teardown ownership (NM16).** A network lot owns its occupant and configuration references (§2.3).
GetNetworkOccupant returns a borrowed pointer; keeping that pointer beyond the owning lot's lifetime
requires a separate AddRef. Releasing the last lot reference releases the configuration, then the occupant,
without calling either occupant Shutdown or RemoveOccupant (`0x0060D130`). Thus a lot view must balance
its acquisitions, while the live occupant manager or another real owner supplies the references needed
by subsequent callbacks. See §3.2 for the native teardown order and worked reference-count case.

**UpdateRoadsInCell(x, z)** (`0x0060F980`):

1. `n := trafficNetworkMap.FindNetworkTile(x, z, 0x1F4F, true)` (traffic network map slot 8, via city slot 40).
   None → done.
2. If `n.HasAnyNetworkFlag(0x80200000)`: done (connection nodes and bit 31).
3. `UpdateRoadStyle(x, z, n)` (§8.7).
4. If `n.HasNetworkFlag(0x40)` (avenue): done (avenues get their lots in `UpdateAvenueStyle`).
5. `w := GetWealthForNetwork(x, z)`, then `zt := GetZoneTypeForNetwork(x, z)` (both through the vtable);
   `UpdateLot(n, x, z, zt, w)`.

**UpdateLot(n, x, z, zone, wealth)** (`0x0060E2D0`):

- If a network lot `L` exists at the key:
  1. If `L.locked`: done.
  2. If `L.config` is non-null and `IsLotCompatible(L.config, x, z, zone, wealth, n.GetRotation(), n.GetFlip())`
     (slots 24 and 25): done (the lot is kept).
  3. `o := L.occupant`; null → done. Hold `o`; `DoNetworkOccupantRemoved(o)`;
     `DoNetworkOccupantInserted(o, zone, wealth)`; release.
- Otherwise, if `n.IsUsable()`: `DoNetworkOccupantInserted(n, zone, wealth)`.

**DoNetworkOccupantInserted(n, zone, wealth)** (`0x0060DCE0`):

1. `group := configs[n.PieceId()]` (slot 23). If there is no group and no `forced`: done.
2. `(x, z) := city.PositionToCell(n.AsOccupant().GetPosition())`.
3. If `forced` is set: `cfg := forced`. Otherwise:
   1. Collect, in group order, the configurations for which `IsLotCompatible(cfg, x, z, zone, wealth,
      n.GetRotation(), n.GetFlip())` holds.
   2. None → done.
   3. `cfg := candidates[RandomUint32Uniform(count)]`, drawn from the network lot manager's generator `0x00B44870`
      (foundations §6.3). One draw, even when `count = 1`.
4. Create a network lot: `occupant := n`, `config := cfg` (both AddRef'd), `locked := false`. Store it as
   `lots[key]` (an existing entry is overwritten without release; `UpdateLot` always removes first).
5. `CreateLotObjects(lot)` (§8.6).
6. If `forced` was set: `lot.locked := true`; release `forced` and set it to null.

**DoNetworkOccupantRemoved(n)** (`0x0060D8B0`):

1. `(x, z) := city.PositionToCell(n.AsOccupant().GetPosition())`.
2. `propManager.RemoveStreetPropsFromTile(x & 0xFF, z & 0xFF)` (prop manager slot 33).
3. `flora.DemolishFloraInCellArea({x, z, x, z}, 0)` (flora slot 20).
4. If a lot exists at the key: release it and erase the entry.

Steps 2 and 3 run even when the cell has no network lot.

**SetLotConfigurationForNextPiece(cfg)** (slot 13, `0x0060CB80`) sets `forced` (with a reference). The next
`DoNetworkOccupantInserted` uses it whatever its compatibility and whatever the piece ID, and locks the lot.

#### NM19: transient inputs and the first divergent lot *[compat]*

Component 11 is not a complete snapshot of a live lot-manager operation. It omits `forced`, every
lot's `locked` byte, pending/hash history and the generator. In particular, `UpdateLot` checks
`locked` **before** compatibility (`0x0060E338–0x0060E340`); a locked lot can remain incompatible
with current zone, wealth, slope, rotation or flip indefinitely. Load-created lots start unlocked;
the record does not restore that transient protection (§4.2).

The next-configuration setter retains its argument and releases the previous one
(`0x0060CB89–0x0060CBA6`). At insertion, a non-null `forced` bypasses the missing-piece-group and
compatibility checks (`0x0060DD23–0x0060DD28`, `0x0060DD66–0x0060DD6B`) and consumes **no
configuration-selection draw**. `CreateLotObjects` still makes its normal prop/flora draws.
After that call the native insertion **re-reads** `forced`: if then non-null it locks the lot,
releases that reference and clears it (`0x0060DF1D–0x0060DF3D`). Capture nested direct setter calls
in order if callbacks occur; do not assume a post-handler null proves it was null on entry.

**Worked case.** With empty object arrays, set forced F, although F is incompatible with zone/wealth
0/0 and there is no piece-ID group for n. A subsequent insertion creates a locked F lot with no
generator advance. A later `UpdateLot` at that key keeps F and makes no draw. Save/load preserves
its configuration ID but not the lock. Given one compatible replacement A after load, the next
`UpdateLot` removes F and chooses A with exactly one selection draw. These outcomes distinguish
transient locking from current compatibility without fabricating a new random path.

**NM19-A — remaining first-event observation.** The census's 2041-01-01 lot (1,25), configuration
`0x4BA69450`, variation 1, rotation 3, flip 0, usable 1 and generator positions 386/383 is a later
sample, not the event that produced it. It cannot determine a first message or prove forced state.
The two fresh attempts cited in the question failed before that month and supply no such trace.
The native entry graph remains the §8.5 table: queued zoning/lot-state passes and usable-change
handling, including their avenue floods, can reach `UpdateLot`; removal messages delete lots.
Object draws additionally enter through direct `RedevelopLot` and `0xA9B6C2D6` (§17.1). There is
no extra edge-cell random rule established by this evidence.

To settle this case, begin at the last exact sample in a run that reaches the affected month, and
retain inputs **before** every native lot-manager handler and direct setter/redevelopment entry:
message ID/data1–4 or direct slot/arguments, nesting, ordered pending keys and bucket state, all live
lots' occupant/configuration identities and lock bytes, forced configuration (or null), and the
terrain/traffic/zone/wealth inputs actually queried. Capture the complete generator at the quiescent
boundary prescribed in §17.3; trace nested entries and draw positions separately, without copying
an in-flight generator into the replay. For each affected cell retain
piece ID, position-to-cell result, rotation/flip, usability/flags, configuration-group order,
compatibility inputs and object candidate lists. Avenue traversal must retain neighbour queries
when they happen, rather than reconstructing them from the post-handler city. Hold references or
copy scalar data so that native removal/reconfiguration does not invalidate the capture.

Replay from those inputs, log generator positions before/after each operation and each selection
or object draw, and stop at the **first** changed lot or draw disagreement. Its enclosing message
or direct call identifies the first entry affecting (1,25), or shows an earlier divergence elsewhere
that moved the shared generator. An unexpected advance outside the listed entries requires its
native caller before adding a draw rule. Do not resync the generator or adopt post-handler chosen
configurations/locks as replay inputs. This observation, including direct entries and transient
state, is required in addition to the message/four-word diagnostic already present.

**ProcessUpdateList()** (`0x0060FA10`): while `pending` is not empty, take its first key in iteration order, call
`UpdateRoadsInCell(key >> 8, key & 0xFF)`, then erase the key. `UpdateAvenueStyle` erases further keys itself
(§8.7).

*[compat]* The original's order is its hash set's order: ascending `key mod bucketCount`, and the latest insert first
within a bucket. Only two things depend on it: which cell starts an avenue flood (§8.7) and the order of the random
draws. The generator is seeded from the wall clock, so draws are not reproducible from a seed; they are reproducible
from a capture of its state (§17.3), which makes the draw order observable. FreeCity4 processes keys in
ascending key order *[choice]*; where two cells of one pass draw, this differs from the original (NM13). The
original's order, exactly (answers NM13):

- `pending` is an SGI `hash_set<u16>`: hashtable object at manager `+0x74`, bucket vector begin `+0x78`, end
  `+0x7C`, element count `+0x84`. The hash is the key itself; a key's bucket is `key mod B`, `B` the bucket count.
- The constructor (`0x0060EE50`, `PUSH 0x64` at `0x0060EF0D`, call `0x00829BC0` at `0x0060EF12`) asks for 100
  buckets, so `B` starts at **193**: the first entry not below the request in the 28-entry prime list at
  `0x00A829B0` (53, 97, 193, 389, 769, 1543, 3079, 6151, 12289, 24593, 49157, 98317, … , 4294967291).
- Insert (`0x0060E210`) first calls resize (`0x00710DD0`) with `count + 1`, even when the key is already present.
  If `B < count + 1`, `B` becomes the first prime not below `count + 1` and every node moves: old buckets in
  ascending order, each from its head, each node pushed to the **head** of its new bucket. Then a new key is pushed
  to the head of its bucket; an existing key is left where it is.
- Erase never shrinks `B`; nothing else touches the bucket vector (Shutdown does not). `B` therefore depends on the
  manager's history (it passes 193 only once some message has queued more than 193 keys at once, for example a zoning rect of
  more than 193 cells after growing it by one)
  and is part of the §17.3 capture: `B = (dword[+0x7C] − dword[+0x78]) / 4`, read on the cISC4NetworkLotManager
  pointer (the object, vtable `0x00AA7958` at +0).
- Iteration (`ProcessUpdateList` takes `begin()` each time, `0x0060FA2E`-`0x0060FA4A`: scan from bucket 0 to the
  first non-null head, copy that node's key, call `UpdateRoadsInCell`, then erase **by key value**): buckets
  ascending, each from its head. A key that `UpdateAvenueStyle` already erased is simply never reached; erasing the
  current key a second time is a no-op. Within one bucket the order is "latest inserted first" **only if no resize
  happened after those keys went in** (see the rehash example below).

Example (`B = 193`): a lot-state message queues, in this order, keys `0x0102` (258, bucket 65), `0x0201` (513,
bucket 127), `0x0143` (323, bucket 130) and `0x00C3` (195, bucket 2). Processing order: `0x00C3`, `0x0102`,
`0x0201`, `0x0143`. Ascending key order would give `0x00C3`, `0x0102`, `0x0143`, `0x0201`, so if `0x0201` and
`0x0143` both create a lot, their configuration draws are swapped. With keys 2 (bucket 2) inserted before 195
(bucket 2), the order is 195 then 2.

**Replay contract for `pending`** *[compat]* (NM13, round 2; for exact shadow replay of the draw order):

1. *No insert during a pass.* The only insert call sites are `0x0060FE45` and `0x0060FFBF` (§8.5 table below), both
   inside `DoMessage` for `0x07699455` and `0xE8BDA2E7`, and each runs `ProcessUpdateList` after its own insert
   loop. Both messages reach the manager **queued**: the zone manager's `PlaceZone` posts `0x07699455`
   (landfill.md, answered Q4) and `cSC4Lot::SetState` posts `0xE8BDA2E7` (`0x006C3100`, lot-developer.md §3.2), so
   lots created or removed inside a pass cannot re-enter the handler. Nothing reachable from `UpdateRoadsInCell`
   inserts. So no resize happens during a pass, and the processing order is fixed when the insert loop ends.
2. *Empty between messages.* `ProcessUpdateList` loops while the count (`+0x84`) is non-zero, so `pending` is empty
   whenever no handler is running. The **only** state that carries from one message to the next is `B`. The set is
   not saved (§2) and no other code touches it (the constructor `0x0060EE50` is its only setup; the manager is created
   with the city, §1, so `B = 193` at every city creation or load).
3. *Closed form for `B`.* Let `P` be the largest value of `count + 1` passed to resize by any insert call since the
   manager was created (duplicates count: an insert of a key that is already present still asks for `count + 1`).
   Then `B` is the first prime of the `0x00A829B0` list that is not below `max(100, P)`. Within one message, `count`
   before an insert is the number of distinct keys inserted so far by that message. For the zoning handler every key
   of the grown rect is distinct, so `P` for that message is the number of rect cells inside the city. For example,
   `B` stays 193 until a message inserts its 194th distinct key (or calls insert with 193 keys present), then becomes
   389, then 769 from the 389th, and so on.
4. *Insertion order of one message.* Zoning (`0x0060FD6C`-`0x0060FE4A`): the rect `{x0−1, z0−1, x1+1, z1+1}`
   (inclusive) walked with **x fastest**: for z ascending, for x ascending; cells with `x < 0`, `z < 0`,
   `x ≥ city cells x` or `z ≥ city cells z` are skipped; key `= (x & 0xFF) << 8 | z`. Lot state: the §6.3 walk order,
   duplicates included.
5. *What a shadow must capture, and when.* Before the original handles the message (any time after its previous
   `0x07699455`/`0xE8BDA2E7` handler returned; by 2, there is no in-flight state): `B = (dword[+0x7C] −
   dword[+0x78]) / 4` on the cISC4NetworkLotManager object. `dword[+0x84]` must read 0; anything else is a capture
   difference to report. Together with the §17.3 generator capture this is the complete input of the pass's draw
   order. A shadow may instead track `B` itself by rule 3 from 193 at city creation or load, provided it sees every
   insert call; the captured value is the check.
6. *Typed replay.* Start from an empty set with the captured `B`; for each key in rule-4 order apply the insert rule
   (resize with `count + 1`, then head-push unless present); then process by rule 1's iteration, skipping keys
   already erased by an avenue flood. After the pass, compare the shadow's `B` with the original's (read the same
   way): a check, never a copy. Do not substitute ascending key order or adopt the original's output order.

This contract determines the pending set's order. Full live-operation replay additionally needs the
transient forced/locked state and direct entries specified under **NM19** above; the component-11
record plus `B` and the random generator alone is not a complete snapshot of those operations.

Rehash example (from the rules above, not game output; reproduced by a simulation of rules 3, 4 and 6): a zoning
change of the 12 × 12 rect x 10–21, z 10–21 in a city of at least 23 × 23 cells inserts the 196 keys of x 9–22,
z 9–22, starting from `B = 193`. The 194th insert (key `0x1416`, x 20, z 22) resizes to `B = 389`; the last two keys
go in afterwards. Processing then starts `0x1409`, `0x140A`, `0x140B`, … (buckets 72, 73, 74, …: `key mod 389`), not
at the lowest key `0x0909`. Bucket 359 holds `0x0C0A` (18th insert, old bucket 187) and `0x0F14` (161st insert, old
bucket 0). The rehash walks old bucket 0 first, so `0x0F14` lands in bucket 359 first and `0x0C0A` is pushed on its
head: `0x0C0A` is processed **before** `0x0F14`, although it went in earlier. Six buckets of this example are ordered
earliest-first in this way (359, 360, 361, 368, 369, 370), so "latest first" is not a rule once a resize has happened.

**Every path that changes network lots** (answers NM11; from the call and cross-reference graph of the
executable, no data references to any of these functions):

| Entry | Reached from | Effect |
|---|---|---|
| key into `pending` | only `0x0060FE45` (zoning changed, full grown rect) and `0x0060FFBF` (lot state changed, §6.3 walk) | `ProcessUpdateList` |
| `UpdateRoadsInCell` | only `ProcessUpdateList` (`0x0060FA68`) and the usable-changed handler (`0x0060FD19`, the piece's own cell) | §8.5 |
| `UpdateLot` | only `UpdateRoadsInCell` and `UpdateAvenueStyle` (`0x0060F424`, every cell of the avenue flood, §8.7) | keep, replace or create |
| `DoNetworkOccupantRemoved` | only `UpdateLot` and the occupant-removed handler (`0x0060FEAF`) | remove |
| `DoNetworkOccupantInserted` | only `UpdateLot` | create |

`RedevelopLot` (slot 12) only rebuilds a lot's objects (§8.6); `pending` is written nowhere else (the traffic
simulator's `DoRoadChanged` inserts into its own set at `+0x7B8`; the avenue flood into a local set). The traffic
network's piece updates reach this manager only as `0x89D3E9F3` (usable changed) and `0x99EF1143`; a neighbour's
style change does not enqueue anything except through the avenue flood. So a lot appears or disappears outside the
six messages' own cells only through the §6.3 walk (cells north of the lot along columns x0 and x1, and row 0) and
the avenue flood.

### 8.6 Lot objects

**CreateLotObjects(lot)** (`0x0060D410`). For each object `i` of the configuration's object array (52-byte records),
in order:

- **Type 1 (prop)**, and bit 0 of the object's flags byte (offset 1) clear: `ids :=` lot configuration manager
  `GetObjectIDArray(object)` (slot 27). If it is not empty: `pick := RandomUint32Uniform(|ids|)` (generator
  `0x00B44870`); `CreateProp(lot, i, ids[pick])` through the vtable (slot 8).
- **Type 4 (flora)**: `ids` as above. If it is not empty: `pick` as above; `(pos, orientation) :=
  GetPositionForLotConfigurationObject(lot, object)`; `f := flora.AddNewFloraOccupant(ids[pick], pos, true)`
  (flora slot 17); if `f`: `f` slot 28 `(i)`.
- Other types: nothing.

Then, if the traffic network map exists and the lot has an occupant: `trafficNetworkMap.UpdateIntersection(occupant)`
(slot 36).

**CreateProp(lot, i, id)** (slot 8, `0x0060CF80`). `(pos, orientation) := GetPositionForLotConfigurationObject(lot,
object i)`. If `id ≠ 0`: `p := propManager.AddStreetProp(id, pos, orientation)` (prop manager slot 32). If `p`:
`p` slot 28 `(object's field at +0x24)`; if the lot's occupant `GetFlip()`: `p` slot 29 `(0x80)`. Return `p`,
borrowed. Otherwise null.

**GetPositionForLotConfigurationObject(lot, object)** (`0x0060C9B0`). The object's offsets `ox`, `oy`, `oz` are its
floats at +4, +8 and +0xC; its orientation `o` is the byte at +2. `(cx, cz) :=` the occupant's `GetOccupiedCell`,
`X0 := float(cx × 16)`, `Z0 := float(cz × 16)`, `r :=` the occupant's `GetRotation()` (low byte):

| r | x | z |
|---|---|---|
| 0 | `(X0 + 16) − ox` | `(Z0 + 16) − oz` |
| 1 | `X0 + oz` | `(Z0 + 16) − ox` |
| 2 | `X0 + ox` | `Z0 + oz` |
| 3 | `(X0 + 16) − oz` | `Z0 + ox` |
| other | 0 | 0 |

Every operation is rounded to float32. Then `y := r24(occupant.GetAltitude(x, z) + oy)`. If the occupant
`GetFlip()`: `x := r24(r24(16 − r24(2 × r24(x − X0))) + x)`, which mirrors x in the cell, and
`orientation := (−(o + r)) mod 4` (table `0x00AA78A0`). Otherwise `orientation := (o + r) & 3`. *[quirk]* `y` is the
altitude at the point before the mirror.

**RedevelopLot(x, z, lot)** (slot 12, `0x0060D5E0`): `RemoveStreetPropsFromTile(x & 0xFF, z & 0xFF)`;
`DemolishFloraInCellArea({x, z, x, z}, 0)`; `CreateLotObjects(lot)`. The lot and its configuration stay.

**IterateAllNetworkLots(cb, ctx)** (slot 11, `0x0060D820`): for each entry in `lots` order, `cb(key >> 8,
key & 0xFF, lot, ctx)`. Stop and return false at the first false; return true otherwise.

**GetNetworkLotConfigurationSet(out)** (slot 6, `0x0060DBF0`; NM15) returns void, pops one 4-byte argument,
and replaces a caller-constructed **set of configuration pointer values**. It neither constructs a set in
uninitialized memory nor accepts a vector, hash set or array of configuration IDs. Its native 32-bit layout is:

| Location | Contents |
|---|---|
| set `+0` | Pointer H to a separately allocated tree header. |
| set `+4` | Unsigned 32-bit element count. The set object is 8 bytes. |
| H/node `+0` | 32-bit red/black colour, red 0 and black 1; header red. |
| H `+4`, `+8`, `+0xC` | Root, leftmost and rightmost pointers. Empty: root null, leftmost/rightmost H. |
| ordinary node `+4`, `+8`, `+0xC` | Parent, left and right pointers; missing children are null, root's parent is H. |
| ordinary node `+0x10` | Configuration pointer value. Each allocation is 20 bytes including the 16-byte tree base. |

The header also has a 20-byte allocation; its value word is not a configuration entry. The comparer is
**unsigned 32-bit pointer less-than**, not configuration-ID, piece-ID, name or object-content order
(`0x0049A070`–`0x0049A084`, `0x0049A0C8`–`0x0049A0CD`). In-order iteration yields increasing pointer
values; two different objects with the same configuration ID remain separate entries. An equal pointer
is not inserted twice. This is the native SGI red-black-tree set representation, with its normal insertion
rebalancing; retaining only its header/count while substituting another node layout is not compatible.

**Allocator.** Header and entry nodes use the game's 20-byte small-allocation path: allocation wrapper
`0x0090CF54` and deallocation wrapper `0x0090CF63`, backed by the allocator at `0x00B62698`.
Insertion allocates 20 bytes at `0x0047C0B8`/`0x0047C0DE`; clearing entries calls `0x00499F50`, which
frees nodes through `0x0090CF63`. The native allocator uses metadata before the returned pointer
(`0x005051C8`–`0x005051F1`); a foreign CRT allocation with the same visible size is not interchangeable.
This is distinct from the GZ allocator-service lists used elsewhere in this spec. The caller constructs
and eventually frees H with the matching allocator. The getter leaves H in place and never frees it.

**Values and ownership, in exact order.** If the old count is nonzero, free all old entry nodes, reset
H to its empty links and set count zero. **No Release of an old configuration occurs.** Walk every
configuration group in manager hash-map order, and every group's vector in load order; for each pointer,
attempt unique insertion, then call its AddRef unconditionally, even if insertion found an equal pointer.
See `0x0060DC0A`–`0x0060DC26`, `0x0060DC57`–`0x0060DC78`. Normal manager loading places each object
in one vector once, so there is one owned returned reference per entry. There is no extra AddRef for
the manager, header or tree node. Native set destruction, like clearing, frees nodes without releasing
their pointed-to configurations: the caller must Release the returned references before clearing,
refilling or destroying the set. The manager catalogue keeps its separate ownership references.

**Worked set case (NM15).** With distinct configuration pointers A=`0x10002000`, B=`0x10001000` and
C=`0x10003000`, suppose catalogue order is A, C, B. A correctly constructed empty set is filled with
count 3 and in-order values B, A, C; each receives one AddRef. Equal configuration IDs do not alter this
result. Release B, A and C once each before the next fill. The next call frees the three old nodes and
adds three new owned values; it does not release the old values on the caller's behalf. With no loaded
configurations the result is count zero with root null and both extrema H. Separately, QI of A for IID 1
succeeds with one AddRef; QI for an invented configuration IID fails without touching its output (§2.4).

**GetNetworkLotConfigurationById(id, out)** (slot 7) returns true if some configuration has that `id`. It never
writes `out` *[quirk]*.

### 8.7 Road style

**UpdateRoadStyle(x, z, n)** (`0x0060F510`):

1. If `n.HasAnyNetworkFlag(0x40280304)` (highway `0x4`, light rail `0x100`, monorail `0x200`, `0x80000`,
   `0x200000`, `0x40000000`): done.
2. If not `n.IsUsable()`: done.
3. If `n.HasNetworkFlag(0x40)` (avenue): `UpdateAvenueStyle(x, z)`; done.
4. `zt := GetZoneTypeForNetwork(x, z)`, then `w := GetWealthForNetwork(x, z)`.
5. If `n.HasAnyNetworkFlag(2)` (rail) and not `n.HasAnyNetworkFlag(0xD)`: `tex := (zt ≠ 0) ? 0x08400000 : 0`.
   Otherwise `tex := T[zt][w]`.
6. `n.SetUnderTexture(tex)` (slot 29). If `tex ≠ 0`: `n.SetNetworkFlag(0x6000000)`. A zero texture does not clear
   the flags *[quirk]*.
7. `n.SetVariation(V[zt][max(w − 1, 0)])` (slot 27).

`T` (table `0x00AA7758`, `T[zone][wealth]`): zone 0 → `0, 0x08100000, 0x08200000, 0x08300000`; zones 7 and 14 →
`0x08400000` for every wealth; zones 8 and 9 → `0x08200000, 0x08100000, 0x08200000, 0x08200000`; every other zone →
`0x08200000, 0x08100000, 0x08200000, 0x08300000`.

`V` (table `0x00AA7728`, `V[zone][w']`): zone 0 → 0 0 0; zones 1 and 4 → 1 2 3; zones 2, 3, 5 and 6 → 4 5 6;
zones 7 and 14 → 7 7 7; zones 8 and 9 → 1 1 2; zones 10–13 and 15 → 1 1 1.

Both lookups are unchecked (NM8): `T` is read at dword `4·zt + w` (`0x0060F5AA`–`0x0060F5AE`) and `V` at byte
`3·zt + w'` (`0x0060F5D6`–`0x0060F5DA`). A zone outside 0..15 (§8.2 passes a corrupt own-cell value through) reads
the bytes after the tables (for zone 16, `T` gives the side offset table: `0xFFFFFFFF, 0, 1, 0`) or before them.
*[choice]* FreeCity4 gives such a zone the "every other zone" rows of `T` and the zones 10–13 row of `V`
(`1 1 1`). This differs from the original only for corrupt zone grids.

**UpdateAvenueStyle(x, z)** (`0x0060EF40`): an avenue is styled as one block.

1. `s := FindNetworkTile(x, z, 0x40, true)`; none → done. `c := s.IsIntersection()` (slot 55).
2. Flood from `(x, z)` (NM4, from the exe). The flood uses three hash sets of `u16` cell keys, `key = (x << 8) | z`:
   the work set W, the accepted set A and the rejected set R (`0x0060EF7A`–`0x0060EFB0`). All inserts are
   insert-unique (`0x0060D270`). Start with W = {key(x, z)}. While W is not empty:
   - Take W's **first** element in bucket order (`begin`, `0x004B0390`) and erase it from W.
   - If the cell has no avenue tile, or the tile's `IsIntersection() ≠ c`: insert it into R (`0x0060F285`).
   - Otherwise insert it into A, then `zoneCount[GetZoneTypeForNetwork(cell)] += 1` and
     `wealthCount[GetWealthForNetwork(cell)] += 1`. Let `e := GetEdgeTypesFlag(6)` (slot 38). For each side `d` in
     the order W (−1, 0), N (0, −1), E (+1, 0), S (0, +1) (tables `0x00AA7858` / `0x00AA7878`), when byte `d` of
     `e` has any of its low 3 bits set, take the neighbour (nx, nz) and test it:
     1. **Bounds:** skip it unless 0 ≤ nx ≤ countX and 0 ≤ nz ≤ countZ (city slots 93 and 94). The far bound is
        inclusive *[quirk]*: one cell past the east and south edges passes.
     2. **Key:** `k := ((nx & 0xFF) << 8) | nz`, kept as `u16` (`0x0060F204`–`0x0060F217`).
     3. **Accepted:** skip it if `k` is in A (an inline bucket search, `0x0060F228`–`0x0060F242`).
     4. **Rejected:** skip it if `k` is in R (`0x0060D1B0`).
     5. **Queue:** otherwise insert `k` into W (`0x0060E210`; a key already queued is not added twice).

   Each cell is therefore examined at most once: it is accepted or rejected in the same step that takes it from W.
   The accepted set is everything reachable from the start through accepted cells along the start cells' own edge
   bits, whatever the order. The order matters only for step 5's order.

   *[quirk]* The past-the-edge cells and the 8-bit packing:
   - In a 64- or 128-cell city, a past-the-edge cell decodes to a cell outside the city. It has no tile, so it
     goes to R.
   - In a 256-cell city the key wraps. nx = 256 decodes as x = 0. nz = 256 sets bit 8, so it decodes as
     (nx | 1, 0).
   - So an avenue whose edge bits point off the far edge of a large city can pull in an avenue cell on the opposite
     edge, when that cell's tile has the same intersection flag. Reproduce the packing.
3. `zt :=` the zone 1–15 with the largest count, ties to the lower zone, 0 if all are 0. `w :=` the wealth 1–3 with
   the largest non-zero count, ties to the lower wealth, else 0. *[quirk]* The ties go the other way from §8.3.
4. `tex := T[zt][w]`, `var := V[zt][max(w − 1, 0)]`.
5. For each accepted cell, in A's bucket order (`key mod bucket count`, buckets ascending, the newest key first
   within a bucket; the sets start at 193 buckets and grow through the STL prime list): look up its avenue tile
   again. If it exists and `IsUsable()` (slot 19): `SetUnderTexture(tex)` (slot 29), `SetVariation(var)` (slot 27),
   `UpdateLot(tile, x, z, zt, w)` (`0x0060E2D0`). In every case erase the cell's key from `pending` (`+0x74`,
   `0x00710910`).

Avenues get no rail branch and no `0x6000000` flags.

### 8.8 Potholes: DoRoadDamageChange(x, z, damaged) (`0x0060DF50`)

`key := (x & 0xFF) << 8 | z`.

**Repaired** (`damaged = 0`): if `potholes` has `key`: erase it; `p` slot 8 `(1)` (stop); `occupantManager.
RemoveOccupant(p.AsOccupant(), true, 0)`; release `p`. Otherwise nothing.

**Damaged** (`damaged ≠ 0`): if `key` is already present, nothing. Otherwise:

1. Create an effect prop: GZCOM class `0x2977AA49`, IID `0x09820792`. Failure → done.
2. `(px, pz) := city.CellCenterToPosition(x, z)` (city slot 97). `py := terrain.GetAltitude(px, pz)`.
3. `name := "pothole"`. Find the first occupant at the cell with flags meeting `0x1F4F` (occupant manager slot 32).
   If found:
   1. `py := its GetAltitude(px, pz)`.
   2. `e :=` the low byte of `GetActiveEdges()`.
   3. `px += r24(4.0 × DXE[e])`, `pz += r24(4.0 × DZE[e])`.
   4. If `GetNetworkFlag() & 0x1FFF = 2` (rail only): `name := "pothole_rail"`.
4. `p` slot 6 `(name)`; `p.AsOccupant().SetPosition({px, py, pz})`; `occupantManager.InsertOccupant(p.AsOccupant(),
   0)` (slot 20); `p` slot 7 `(1)` (start); `potholes[key] := p` (the reference from step 1).

`(DXE, DZE)` for `e` = 0…15 (signed bytes, table `0x00AA7910`): 1 → (−1, 0); 2 → (0, −1); 3 → (−1, −1); 4 → (1, 0);
6 → (1, −1); 8 → (0, 1); 9 → (−1, 1); 12 → (1, 1); every other value → (0, 0). A dead end or a curve moves the
pothole 4 m towards its open side. A straight piece, a T or a cross keeps it at the centre. *[quirk]* The height is
the one at the centre, before the move.

The traffic simulator sends `damaged = 1` for each new pothole and 0 when a damaged cell is repaired or removed
(traffic-simulator.md §3.6, §4.12). The pothole props are saved in component 11 (§4.2) as well as through the occupant
manager.

---

## 9. Placement and removal as the simulation and the harness see them

**Placing a network by code** (what a scripted action should do) *[compat]*:

1. `t := networkManager.GetNetworkTool(type, true)` for a private tool, or `false` for the shared one (do not
   release the shared one).
2. `t.Init()`; `t.PlaceNetworkOccupants(x1, z1, x2, z2, 0, null, true, −1)` (tool slot 7: a straight run in real
   mode, checked by the tool); `t.Shutdown()`. The call is **free**: only the player's tool UI withdraws
   `GetCostOfSolution()` from the budget. It makes no regional connection (network-tools.md §3.4, §8).

The tool inserts each occupant through the occupant manager with `data4 = 3` (zones.md §5.2). The listeners see, in
this order:

- `0x99EF1142` per occupant: the traffic network (traffic-network.md §3), the zone manager's stub joining (zones.md
  §5.2.1) and `managed` here for bridges and tunnels.
- Later, `0x89D3E9F3` with data1 = 1, when the piece becomes usable (flag `0x10000000`). That is when the network lot
  manager gives the cell its style and network lot. The tool sets the flag before `PlaceNetworkOccupants` returns;
  the message is posted, so it is delivered after (network-tools.md §6.3).

**Removing networks.**

- Demolition goes through `DestroyNetworkOccupants` (§7.2), batched by `QueueRemovals` (§7.3) inside
  `DemolishRegion` (demolition.md §4.2, §4.6 `UpdateNetworks` `0x004681A0`).
- The tool's `DestroyOccupantsAndResolve` removes the occupants and rebuilds their neighbours.
- `0x99EF1143` per occupant then drives the traffic network, `managed`, and the network lot removal (props and flora
  of the cell, §8.5).
- `RemoveOrReplaceOccupant` (§7.4) is the single-cell path. It keeps an intersection whole by putting a plain piece
  back.
- Terrain edits remove what they leave floating or buried (§7.6).

**NM20: a draw after generated actions.** The fault at `0x007529C0` belongs to
[terrain-view.md §4.4.1](terrain-view.md#441-native-receiver-terrain-lifetime-and-generated-action-fault-nm20-compat),
which specifies the drawing-interface receiver, retained terrain and cell-texture caller, with a
worked lifetime case and the remaining runtime observation NM20-A. It is outside this placement
notification sequence. Neither the preceding `ok=0` nor tool Shutdown establishes a terrain-view
shutdown or a cause in automata. Keep the supplied s2 runs unaccepted pending that observation.

**Lots that need networks.**

- Transit lots (stations, ports) get **connection nodes**: `CreateConnectionNode` (§7.7), from lot-developer.md
  `AddNetworkConnectionNodes`. `AutoconnectLotNodes` (§7.8) then draws the network from each node out to the
  nearest network.
- Connection nodes are network occupants with flag `0x200000`, ignored by the traffic graph, the network lots and road
  style.
- Toll booths are network pieces placed by their own UI control (`cSC4ViewInputControlTollBooth`), not network lots.
  They are outside this spec.

---

## 10. The riot's removal lock (answers disasters.md Q4)

disasters.md §6.3 "flag 2" is not a network flag, and it changes nothing in traffic. It is the player's demolition
privilege:

1. **MarkRiotNetworkOccupants(x, z)** (`0x0057D300`). For `i` from `x − 3` to `x + 3` and `j` from `z − 3` to `z + 3`
   (x outer): `tile := trafficNetworkMap.SafeGetTileInfoAtCell(i, j)` (slot 14). If the tile and its occupant are
   non-null, call `riotManager.LockNetworkOccupant(occupant)`.
2. **LockNetworkOccupant(n)** (riot manager interface at vtable `0x00A9E7C8`, slot 9, `0x0057DA40`). If `n` is null
   or already in the manager's set: nothing. Otherwise AddRef; `n.AsOccupant().UnsetRemovalFlags(2)`
   (cISC4Occupant slot 13); insert `n` into the set.
3. The removal counters (`0x005E8A60`, `0x005E8AA0`, `0x005E8AE0`). `SetRemovalFlags(m)` increments, and
   `UnsetRemovalFlags(m)` decrements, a byte counter per bit: bit 1 the system's, bit 2 the player's.
   `CanRemove(level)` is true while the level's counter is 0. Starting from 0, the decrement wraps to 255, so
   `CanRemove(2)` becomes false. The player's bulldozer (demolition.md §4.2, privilege 1) then refuses the piece. The
   system's privilege (`CanRemove(1)`) is unchanged.
4. **ClearRiotNetworkOccupants** (`0x0057D720`). For each occupant in the set: `SetRemovalFlags(2)`, which
   increments it back, then Release. It runs only when the last riot instance is removed (`RemoveInstanceFromList`
   `0x0057D7E0`) or at the riot manager's `Shutdown` (`0x0057DA20`).

Quirks *[compat]*:

- **Edge quirk.** The loop bounds are compared **unsigned**. If `x < 3`, `x − 3` wraps past `x + 3` and the outer loop
  runs no row. If `z < 3`, every row is empty. A riot within 3 cells of the west or north edge locks nothing.
- **Inverted counter.** The pairing works only because the counter starts at 0. An occupant already locked by
  something else would be unlocked during the riot (its counter goes from 1 to 0).
- **No unlock in between.** Nothing unlocks while any riot instance remains. A city with overlapping riots keeps
  every marked piece locked until the last one ends.
- **No unlock before the city closes** (re-answers14, 2026-10-15). An ended riot (phase 3) is **not** removed from
  the riot manager's list; only its `Shutdown` removes it, and that runs from the riot manager's `Shutdown`
  (disasters.md §6.7). So the list never becomes empty during play, and `ClearRiotNetworkOccupants` runs only at
  the riot manager's `Shutdown`. Every piece any riot locked stays unbulldozable by the player for the rest of the
  session. Leaders also lock more road as they walk (disasters.md §6.5).

---

## 11. Interface slot tables

### 11.1 cISC4NetworkManager (vtable `0x00AA7AD0`)

| Slot | Offset | Address | Method | Pop | Return |
|---|---|---|---|---|---|
| 0–2 | | `0x00610750`, `0x004664A0`, `0x006107D0` | QueryInterface, AddRef, Release | 8, 0, 0 | AL, EAX, EAX |
| 3 | +0xC | `0x00610C10` | `bool Init()` | 0 | AL |
| 4 | +0x10 | `0x00611EB0` | `bool Shutdown()` | 0 | AL |
| 5 | +0x14 | `0x006108D0` | `cISC4NetworkTool* GetNetworkTool(int type, bool unique)` | 8 | EAX |
| 6 | +0x18 | `0x00612110` | `void DestroyNetworkOccupants(list)` | 4 | — |
| 7 | +0x1C | `0x00613450` | `void QueueRemovals(bool)` | 4 | — |
| 8 | +0x20 | `0x006110B0` | `bool RemoveOrReplaceOccupant(tool, occ, bool& replaced)` | 12 | AL |
| 9 | +0x24 | `0x00610540` | `bool IsOccupantSubwayStation(occ)` | 4 | AL |
| 10 | +0x28 | `0x00610550` | `FindPipeOccupant` (3 args) | 12 | |
| 11 | +0x2C | `0x00610570` | `FindPipeConnectionOccupant` (3 args) | 12 | |
| 12 | +0x30 | `0x00612780` | `GetAllIntersectionPieces` (4 args) | 16 | |
| 13 | +0x34 | `0x00610AD0` | `MapPreviewTexture` | 8 | |
| 14 | +0x38 | `0x00611D50` | `MapShadowTexture` | 16 | |
| 15 | +0x3C | `0x00610B40` | `MapPath` | 4 | |
| 16 | +0x40 | `0x006122B0` | `RegisterConstructionCrew` | 4 | |
| 17 | +0x44 | `0x006112A0` | `UnregisterConstructionCrew` | 4 | |
| 18 | +0x48 | `0x00610590` | `GetWaterOccupantFilter` (new) | 0 | EAX |
| 19 | +0x4C | `0x006105C0` | `GetSubwayOccupantFilter` (new) | 0 | EAX |
| 20 | +0x50 | `0x00611300` | `bool CreateConnectionNode(out&, type, point&, edgesA, edgesB, pathOverride, uint8)` | 28 | AL |
| 21 | +0x54 | `0x00611530` | `AutoconnectLotNodes(lot, nodes)` | 8 | |
| 22 | +0x58 | `0x006107F0` | `const char* GetEnglishNetworkName(type)` | 4 | EAX |
| 23 | +0x5C | `0x00612480` | `AddToRetrofitList(occ)` | 4 | — |
| 24 | +0x60 | `0x00612590` | deleting destructor | 4 | |

cIGZMessageTarget2 (`0x00AA7ABC`): 0 `0x006120D0`, 1 `0x006C2270`, 2 `0x006120E0` (thunks), 3 `DoMessage`
`0x00613950`. cIGZSerializable (`0x00AA7AA4`): 0–2 thunks `0x006120F0`, `0x006C2290`, `0x00612100`; 3 `Write`
`0x006111E0`; 4 `Read` `0x006124C0`; 5 `GetGZCLSID` `0x00610530` (returns `0xC990BD46`).

### 11.2 cISC4NetworkLotManager (vtable `0x00AA7958`)

| Slot | Offset | Address | Method | Pop |
|---|---|---|---|---|
| 0–2 | | `0x0060C760`, `0x0060C730`, `0x0060C740` | QueryInterface, AddRef, Release | 8, 0, 0 |
| 3 | +0xC | `0x00610120` | `bool Init()` | 0 |
| 4 | +0x10 | `0x0060E570` | `bool Shutdown()` | 0 |
| 5 | +0x14 | `0x0060D650` | `cISC4NetworkLot* GetNetworkLot(x, z)` (borrowed) | 8 |
| 6 | +0x18 | `0x0060DBF0` | `GetNetworkLotConfigurationSet(set&)` | 4 |
| 7 | +0x1C | `0x0060D9A0` | `bool GetNetworkLotConfigurationById(id, cfg*)` | 8 |
| 8 | +0x20 | `0x0060CF80` | `cISC4Occupant* CreateProp(lot, index, id)` (borrowed) | 12 |
| 9 | +0x24 | `0x0060D030` | `ZoneType GetZoneTypeForNetwork(x, z)` | 8 |
| 10 | +0x28 | `0x0060CBB0` | `WealthType GetWealthForNetwork(x, z)` | 8 |
| 11 | +0x2C | `0x0060D820` | `bool IterateAllNetworkLots(cb, ctx)` | 8 |
| 12 | +0x30 | `0x0060D5E0` | `RedevelopLot(x, z, lot)` | 12 |
| 13 | +0x34 | `0x0060CB80` | `SetLotConfigurationForNextPiece(cfg)` | 4 |
| 14 | +0x38 | `0x0060E810` | `bool WriteLotConfiguration(cfg, segment)` | 8 |
| 15 | +0x3C | `0x0060EF20` | deleting destructor | 4 |

cIGZMessageTarget2 (`0x00AA7948`): 0–2 thunks `0x0060E510`, `0x0060E520`, `0x0060E530`; 3 `DoMessage`
`0x0060FC30`. cIGZSerializable (`0x00AA7930`): 0–2 thunks `0x0060E540`, `0x0060E550`, `0x0060E560`; 3 `Write`
`0x0060D6B0`; 4 `Read` `0x0060DAC0`; 5 `GetGZCLSID` `0x0060C720`.

### 11.3 cISC4NetworkOccupant slots used here (compiled slot = SDK index + 3)

5 IsManagedGlobally, 6 CheckForRetrofit, 7 SetRetrofitFlag, 8 HasNetworkFlag (all bits), 9 HasAnyNetworkFlag,
10 SetNetworkFlag (`0x00619EC0`), 11 ClearNetworkFlag (`0x00619F70`), 12 GetNetworkFlag, 13/14
Get/SetConnectionNodeEdgeTypes, 16 SetConnectionPathIDOverride, 17/18 IsImmovable/SetImmovable (flag `0x400000`),
19/20 IsUsable/SetUsable (flag `0x10000000`), 21 IsPlaced (`GetFlags() & 1`), 22 IsOfType, 23 PieceId,
24 GetRotation, 25 GetFlip, 27 SetVariation, 29 SetUnderTexture, 30 SetQuadDraw, 36 SetVertexHeights,
38 GetEdgeTypesFlag, 40 GetActiveEdges, 48 AsOccupant, 55 IsIntersection, 56 GetAltitude (ST0), 58 SetTranslucency,
59 GetOccupiedCell, 61 SetLit, 62 CanBeLit, 66 IsPartOfTheSameStructure, 71 GetComplexPieceOrigin. (Slots 53 and 54
are reversed from the SDK, traffic-network.md §7.2.)

---

## 12. Dependencies

| Interface | Slots used | For |
|---|---|---|
| message server `0x00B43CCC` | 4 post, 5 subscribe, 6 unsubscribe | §3, §5 |
| city `0x00B43CEC` | 38 network manager, 40 traffic network map, 42 network lot manager, 93/94 cell counts, 95 PositionToCell, 97 CellCenterToPosition | throughout |
| occupant manager `0x00B43D0C` | 20 InsertOccupant, 21 RemoveOccupant, 32 first occupant at a cell (filter), 40 occupants in a rect (filter), 45 iterate by type, 48 release list | §6.1, §7, §8.8 |
| lot manager `0x00B43D08` | 6 GetLot | §7.7, §8.3 |
| terrain `0x00B43CF4` | 13 GetAltitude | §7.6, §8.8 |
| 24-hour clock `0x00B43DBC` | 28 IsNight | §6.1 |
| prop manager `0x00B43D10` | 32 AddStreetProp, 33 RemoveStreetPropsFromTile | §8.5, §8.6 |
| zone manager `0x00B43D14` | the zone grid | §8.2 |
| lot configuration manager `0x00B43D18` | 27 GetObjectIDArray | §8.6 |
| traffic network map `0x00B43D54` | 8 FindNetworkTile, 14 SafeGetTileInfoAtCell, 36 UpdateIntersection | §8.5, §8.6, §10 |
| building development `0x00B43D68` | 36 lot developer (then its slot 29 GetAreaSlope), 40 GetBuildingProfilePtr | §8.3, §8.4 |
| flora `0x00B43D80` | 17 AddNewFloraOccupant, 20 DemolishFloraInCellArea | §8.5, §8.6 |
| resource manager `0x00B43CA8` | 18 key list (then list slot 7 to enumerate) | §3.2 |
| generator `0x00B44870` | `RandomUint32Uniform` | §8.5, §8.6 |

---

## 13. Replaceability

The simulation-visible parts are:

- the two records;
- the `managed` list;
- the flags;
- the terrain rule (§7.6);
- the batched removal;
- the network lot choice;
- the style;
- the objects;
- the potholes;
- the riot lock.

The tools, models, remap tables and retrofit are presentation or legacy. A FreeCity4 replacement:

- implements both components with the IDs of §1;
- keeps the message order of §3 and the records of §4 byte for byte;
- may leave slots 13–15 as identity mappings until a renderer needs them *[choice]*.

---

## 14. Quirks (summary)

1. `Read` of component 53 appends without clearing (§4.1). Inserting a managed piece twice duplicates it (§6.1).
2. `DestroyNetworkOccupants` leaks one reference per destroyed element (§7.2).
3. `QueueRemovals` destroys in memory order (§7.3).
4. `GetEnglishNetworkName` has no bounds check (§7.5).
5. `AutoconnectLotNodes` skips in-lot neighbours only for E and S (§7.8).
6. The network lot manager's `Init` drops lots with missing configurations without releasing them (§3.2).
7. `0xA9B6C2D6` is subscribed but never posted (§6.2).
8. `IsLotCompatible`: 8-bit column shift, rows past 3 read neighbouring bytes, `n = 0` faults (§8.4).
9. `GetNetworkLotConfigurationById` never fills its output (§8.6).
10. The wealth ties go up in §8.3 and down in §8.7.
11. Road style never clears `0x6000000` (§8.7).
12. A pothole's height is taken at the cell centre (§8.8). An object's height is taken before the mirror (§8.6).
13. The riot lock: the edge quirk, the inverted counter, and no unlock until the last riot ends (§10).
14. A construction crew that does not unregister hangs PreCityShutdown (§7.12).
15. The lot-state walk starts at (0, 0): row 0, columns x0/x1 north of the lot, no inner top row (§6.3).

---

## 15. Open questions

- ~~**NM1.** Are the globally managed occupants also saved by the occupant manager, or only through component 53?~~
  **Answered (re-specs14b):** both. The occupant manager's grid (component 4) saves a ref to every occupant in every
  cell, and component 53 saves the managed list. Refs resolve to one object per ID (foundations §2.7), so the load
  gives one object, and `managed` holds it once (§4.1). Each occupant's own base record carries its "in the manager"
  flag and its stored box (occupant-manager.md §13, OM2).
- ~~**NM2.** Does the occupant manager post `0x99EF1142` while a city loads?~~ **Answered (re-specs14b):** no.
  Occupant-manager `Read` posts nothing. The base occupant `Read` (`0x005E8D60`) sets bit 1 of the occupant's state
  byte, and with that bit set the base `Init` (`0x005E8C90`) posts **no** Create `0x99EF1140` either. The network,
  bridge, tunnel and prebuilt `Init`s (`0x0061CB50`, `0x00604BF0`, `0x006471C0`, `0x0061DB00`) never insert. So
  `managed` comes only from component 53, once per piece.
- ~~**NM3.** Meanings of flags `0x40000`, `0x80000`, `0x40000000`, `0x80000000` and of terrain kinds 3, 4 and 6
  (§7.6).~~ **ANSWERED 2026-10-15 (re-specs15b):** network-geometry.md §10.3. `0x40000` = bridge-step piece
  (MATCHES); `0x80000` = tunnel portal (MATCHES); `0x40000000` = elevated with supports (MATCHES); `0x80000000` = draft
  (preview) occupant, set by `CreateNetworkOccupant` in draft mode and cleared in real mode. The §5 table is updated.
  History: *partly answered* (re-specs14a): the kinds are settled in terrain.md §8.2 (3 = disasters and water
  changes, 4 = the network height solver's own grading, 5 = levelling, 6 = the tunnel cut); `0x40000` is the bridge
  flag *[inferred from use]*. The other three flags stay open.
  Leads for the open flags (network-tools.md §6.2, §10):
  - `0x40000000` is set with `0x8000000` when the tool adds support structures (elevated pieces) *[inferred]*;
  - `0x80000` marks a tunnel portal: `DestroyOccupantsAndResolve` QIs such a piece to the tunnel occupant
    (`0x8A4BD52B`) to bring in the other portal *[inferred]*.
- ~~**NM4.** The exact visited and rejected set tests of the avenue flood (§8.7, marked inferred). Styling an avenue
  loop that crosses an intersection, then logging the styled cells, would settle them.~~ **ANSWERED 2026-10-16
  (re-answers18):** from `0x0060EF40`, written into §8.7 step 2.
  - Neighbours are tested at queue time: skipped when in the accepted set, then when in the rejected set, else
    inserted unique into the work set. So each cell is examined once, and the bracket MATCHES.
  - Two quirks are added. The far bounds are inclusive (nx ≤ countX, nz ≤ countZ). The `u16` key packs x into 8
    bits, so in a 256-cell city a neighbour one past the east or south edge aliases a cell on the opposite edge.
- ~~**NM5.** The effect-prop slots 6, 7 and 8 used for potholes (§8.8) are named from their use (set the effect name,
  start, stop).~~ **Answered (re-specs14b):** confirmed. They are `SetEffectType(char const*)`, `StartEffect(bool)` and
  `StopEffect(bool)` of cISC4PropOccupantVisualEffect (vtable `0x00AA4ED8`; the two `SetEffectType` overloads compile
  in reverse SDK order). Behaviour: network-tools.md §13.2.
- ~~**NM6–NM9** (filed by the cores14c lane, QUESTIONS.md).~~ **ANSWERED 2026-10-16 (re-answers24):** NM6 (§4.1, a
  null reference is a successful read), NM7 (§7.10, every entry released once in the walk, the list always emptied),
  NM8 (§8.2, §8.4, §8.7, no range checks: out-of-range values read neighbouring data; the FreeCity4 choices are
  confirmed) and NM9 (§7.6, **DIFFERS**: each kind runs only when its occupant list is not empty).
- ~~**NM12.** Can the configuration draw's input be captured before our step?~~ **Answered (re-netmgr-rng4):** yes,
  by an in-process read of the generator's layout between network lot manager operations (§17.1–§17.3); the draw
  is compared exactly, no resync.
- ~~**NM13.** The original's key order within one `ProcessUpdateList` pass.~~ **Answered (re-netmgr-rng4):**
  **DIFFERS** from FreeCity4's ascending key order: hash order `key mod B`, latest first unless a resize intervened, `B` from 193 up; capture and
  typed-replay rules in §8.5 "Replay contract for `pending`".

---

## 16. Conformance tests

Each test names its section. "City" means a fresh 64 × 64 city unless stated otherwise.

1. **Component 53, empty** (§4.1). Write → `01 00 00 00 00 00`. Read of `02 00 …` → false.
2. **Component 11, empty** (§4.2). Write → `02 00 00 00 00 00 00 00 00 00`. Read of `01 00` → false, nothing else
   read.
3. **Managed list order** (§6.1). Insert bridges A, B, C (`0x99EF1142`), then remove A. `managed` is C, B. Save →
   count 2, refs C then B.
4. **Duplicate insert** (§6.1). Insert A twice → `managed` is A, A; remove A once → A.
5. **Not managed** (§6.1). Insert a plain road piece → `managed` unchanged.
6. **Lighting** (§6.1). `managed` = a bridge and a tunnel; dusk → `SetLit(true)` on the bridge only.
7. **GetNetworkTool** (§7.1). `(13, false)` → null; `(5, false)` twice → the same pointer, reference count
   unchanged; `(5, true)` → a new tool.
8. **QueueRemovals dedupe** (§7.3). `QueueRemovals(true)`; destroy lists [A, B] and [A]; `QueueRemovals(false)` → one
   `DestroyOccupantsAndResolve` call for the surface group, containing A and B once each.
9. **Groups** (§7.2). A list with a road R, a pipe P and a subway S → three tool calls: type 0 with [R], type 4 with
   [P], type 7 with [S], in that order.
10. **Terrain rect** (§7.6). data1 = `0x00060009`, data2 = `0x0000000F`, 64 × 64 city → rect x 5..9, z 0..15 (z1 = −1 clamped to 0).
    With `longTerrainOp` and a previous `dirty` of x 8..20, z 3..4 → `dirty` = x 5..20, z 0..15.
11. **Corner rule** (§7.6). Road whose corners stand 0.005 above the terrain, kind 1 → kept. 0.02 above → destroyed.
    0.001 below → destroyed. Same road, kind 4 → kept. A pipe 0.02 off, kind 1 → kept; kind 3 → destroyed.
    Kind 5 → no query at all.
12. **Riot lock** (§10). Road at (10, 10), riot at (12, 12): `CanRemove(2)` false, `CanRemove(1)` true. A second riot
    at (11, 11) marks nothing new (the set). End the first riot → still locked; end the second → `CanRemove(2)` true.
13. **Riot edge** (§10). Riot at (2, 10) → no road is locked, even the one at (2, 10).
14. **Zone for network** (§8.2). Cell (5, 5) unzoned; W = R$ (1), N = C$$$ (6), E = I-D (8), S = none → 8. Unzoned
    with W = agriculture (7) and the rest none → diagonals are visited; diagonal (4, 6) = R$$ (2) → 2. Cell zoned
    C$ → 4 whatever the neighbours.
15. **Wealth for network** (§8.3). Orthogonal lots of wealth 1, 2, 2, 1 → counts (0, 2, 2, 0) → 2 (tie up). Lots
    1, 1, 2 → 1. No orthogonal lot, one diagonal lot of wealth 3 → 3. No lot → 0.
16. **Compatibility pattern** (§8.4). n = 2, mask rows `01 02 …`: (x, z) = (4, 6) → row 0, col 0, bit 1 → ok;
    (5, 6) → col 1 → no; (5, 7) → row 1, col 1 → ok. A configuration with no zone list → never compatible.
    Slope 3.0 with min 0, max 2 → no.
17. **Object position** (§8.6). Cell (10, 20), object offsets (3, 0.5, 5), o = 2, rotation 1, not flipped →
    x = 165, z = 333, orientation 3, y = altitude(165, 333) + 0.5. Flipped → x = 171, orientation 1.
18. **Choose and keep** (§8.5). A usable street piece with two compatible configurations → one random draw, one lot.
    Re-running `UpdateRoadsInCell` with the zone unchanged → no draw, the same lot. Rezoning the neighbour so that the
    configuration no longer fits → the props and flora of the cell are removed, and a new lot is chosen.
19. **Forced configuration** (§8.5). `SetLotConfigurationForNextPiece(F)`; place a piece → its lot uses F, is locked,
    `forced` is cleared. Rezoning around it never replaces it.
20. **Road style** (§8.7). A street with zone R$ (1) and wealth 2 → texture `0x08200000`, variation 2, flags
    `0x6000000` set. Rail only next to zone 0 → texture 0, flags unchanged. A street next to agriculture → texture
    `0x08400000`, variation 7.
21. **Pothole** (§8.8). `0x69247DC8 (10, 10, 1)` on a road dead end whose only open side is W (`e = 1`) → a "pothole"
    effect at the cell centre − 4 m in x. Repeating it → nothing. A rail-only cell → "pothole_rail".
    `(10, 10, 0)` → the effect is removed. `(11, 11, 0)` with no pothole → nothing.
22. **Usable message** (§5). `SetNetworkFlag(0x10000000)` on a placed piece → one `0x89D3E9F3` (data1 = 1); setting it
    again → none; `ClearNetworkFlag(0x10000000)` → one with data1 = 0. On a piece not in the city: set → none; clear
    → one.
23. **Lot state walk** (§6.2, §6.3). A 2 × 3 lot at x 10..11, z 20..22 changes state (grown rect {9, 19, 12, 23},
    city at least 24 × 24, `pending` empty) → `pending` gets 61 keys: (0..12, 0); (9, z) and (12, z) for
    z = 1..22; (9..12, 23). Not (10, 19), (11, 19) nor the lot's own 6 cells.
24. **Walk at the top edge** (§6.3). A 1 × 1 lot at (5, 0) (grown {4, −1, 6, 1}) → keys (0..6, 0) and (4..6, 1):
    row 0 is walked in full from x = 0, so it includes the lot's own cell (5, 0).
25. **Raw-state replay** (§17.2–§17.4). Load the generator from seed `0x9E3D9501` and draw 3, 4, 2 → 2, 2, 1.
    Load it raw at `k = 622` (the words of that seed after 622 draws) and draw 3, 4, 2 → 1, 3, 1, ending at
    `left = 623`, `k = 1` after a reload. The post-state equals that of the seeded generator after 625 draws.
26. **Pending order** (§8.5, NM13). `B = 193`, keys queued `0x0102`, `0x0201`, `0x0143`, `0x00C3` → processed
    `0x00C3`, `0x0102`, `0x0201`, `0x0143`. Keys 2 then 195 (same bucket) → 195 first. 194 distinct keys queued
    from empty → `B = 389` after the 194th insert.
27. **Pending rehash order** (§8.5 replay contract). `B = 193`, city ≥ 23 × 23, zoning change rect x 10–21,
    z 10–21 → 196 keys inserted, resize to `B = 389` at insert 194 (key `0x1416`); processing starts `0x1409`,
    `0x140A`, `0x140B`, `0x140C`; bucket 359 yields `0x0C0A` then `0x0F14`, bucket 368 `0x0909` then `0x0C13`.
    After the pass `B = 389`, count 0. A following zoning change of one cell from that state keeps `B = 389`
    (its 9 keys run by `key mod 389`).
28. **Duplicate insert resizes** (§8.5 rule 3). With 193 distinct keys present at `B = 193`, inserting one of them
    again resizes to `B = 389` and adds nothing (count stays 193).

---

## 17. Random draws

Only the network lot manager draws, all from its own generator `0x00B44870` (`RandomUint32Uniform(n)`, foundations
§6):

- one draw per new network lot chosen without a forced configuration (§8.5 step 3.3);
- one draw per prop or flora object with a non-empty candidate list (§8.6), in object order.

So `RedevelopLot` (city-detail.md §3.4) draws again for every object of every lot. It removes the cell's street
props and flora and creates new ones, which may differ from the old ones. The prop manager's and the flora's saved
state therefore change, while component 11 (the lot and its configuration) does not.

The network manager draws nothing.

### 17.1 Ownership and advance sites (answers NM12)

The generator is a file-local static object of the Windows build, not a member of the manager. Its whole lifetime:

| Site | Instruction | Effect |
|---|---|---|
| static initialiser `0x00A73BF0` | `MOV ECX,0xB44870` (`b9 70 48 b4 00`) at `0x00A73C01`, then the constructor `0x00913B74` | seeded once per process with `cRZTime() XOR 0x8C09C379` (foundations §6.3) |
| `CreateLotObjects`, prop object | `0x0060D4A7` / call `0x00913A06` at `0x0060D4AC` | one `RandomUint32Uniform(n)` |
| `CreateLotObjects`, flora object | `0x0060D50D` / call at `0x0060D512` | one `RandomUint32Uniform(n)` |
| `DoNetworkOccupantInserted`, configuration | `0x0060DE54` / call at `0x0060DE59` | one `RandomUint32Uniform(count)`, skipped when `count = 0` (`0x0060DE4F`) |

No other instruction of the executable holds the address `0x00B44870` or any address inside the object
(`0x00B44870`–`0x00B45237`; the next static, at `0x00B45238`, belongs to other code), and the export has no data
reference to it. The generator is never re-seeded, never saved and never loaded (foundations §6.2): its stream runs
on across city loads within one process. So it advances **only** inside the three network lot manager draw sites,
and those are reached only from:

- `DoNetworkOccupantInserted`, through `UpdateLot` (§8.5 table: `ProcessUpdateList` on messages `0x07699455` and
  `0xE8BDA2E7`, the usable-changed handler on `0x89D3E9F3`, and the avenue flood inside either);
- `CreateLotObjects`, through `DoNetworkOccupantInserted`, `RedevelopLot` (slot 12; only caller the city detail
  pass, city-detail.md §3.4) and message `0xA9B6C2D6` (`0x00610002`; never posted, §6).

*[compat]* Between two of those operations the generator's state is constant.

### 17.2 Layout (Windows build, image base `0x00400000`, relocations stripped, no ASLR)

`cRZRandomSuper` has no vtable. Its 0x9C8 bytes, from the constructor (`0x00913B7B` `83 a6 c0 09 00 00 00`,
`0x00913B82` `c7 86 c4 09 00 00 70 02 00 00`), the draw (`0x009139B9`) and the reload (`0x009138A9`, which ends with
`left := 0x270`, `next := &state[0]` at `0x00913961`–`0x0091396B`):

| Offset | Absolute (this generator) | Field |
|---|---|---|
| +0x000 | `0x00B44870`–`0x00B4522F` | `state[624]`, u32, the MT19937 words |
| +0x9C0 | `0x00B45230` | `next`, a pointer to the next word to temper |
| +0x9C4 | `0x00B45234` | `left`, signed i32 |

One draw: `left := left − 1`; if `left < 0`: reload (standard twist of all 624 words, `left := 624`,
`next := &state[0]`) and `left := left − 1` again; then temper `*next` and advance `next` by one word. Seeding ends
with a reload, so a fresh generator holds `left = 624`. Between draws `0 ≤ left ≤ 624` and
`next = 0x00B44870 + 4 × (624 − left)`. The **position** `k := 624 − left` is the index of the next word to temper;
`k = 624` means the next draw reloads first. `(state[0..623], k)` is the complete state: the seed is neither stored nor
needed.

### 17.3 Capturing the draw input *[compat]* (for comparison and shadow harnesses)

No interface slot (§11), message or saved record exposes the generator; the capture is an in-process read of the
§17.2 layout of this build. It is sanctioned at any instant when no network lot manager function is executing
(between message dispatches; never from inside a callback the manager is running). By §17.1, a capture taken after
the last draw-performing operation and before the next one is exactly that operation's input, wherever between the
two it is taken. A harness that runs its step after the original's (both answer the same message) therefore takes
the **pre-state** of step n+1 at the end of its handling of step n, and must observe every trigger listed in §17.1
(the three messages, the city detail pass's `RedevelopLot` calls, and `0xA9B6C2D6` should anything post it).

Replay rule:

1. Load the captured `(state, k)` into FreeCity4's generator as raw state (not as a seed: the seed is lost).
2. Run the step. Every draw of §8.5 step 3.3 and §8.6 then sees the original's word; `cfg`, `pick` and the objects
   become deterministic outputs and are compared like any other field of component 11 and the prop and flora state.
3. After the step, FreeCity4's `(state, k)` must equal the original's, read the same way. This is a check, never a
   copy: a difference means a different number or order of draws. The next capture is the next step's input.
4. A capture that differs from FreeCity4's post-state of the previous step means a draw the harness did not
   observe; it is reported, not absorbed.

So the configuration draw is **not** outside exact comparison: with the capture, a cell whose configuration
differs among `count ≥ 2` compatible candidates is a mismatch, and a resync after it is not specified behaviour.
Exactness also needs the original's draw order. Within one `ProcessUpdateList` pass that order follows the hash
set's iteration order (§8.5 *[compat]* note, NM13), which FreeCity4 currently replaces by ascending key order
*[choice]*. The two orders agree whenever at most one cell of the pass draws. For exact replay when several draw,
the capture also takes `pending`'s bucket count `B` (§8.5) and the step processes keys in the original's order;
the §8.5 replay contract (rules 1-6) gives what to capture, when, and the typed replay.

**NM19 supplement.** Those rules supply random/hash state, not all live city inputs. Retain the
pre-handler forced reference, lot lock bytes and queried inputs described under §8.5 NM19. Observe
the direct setter and redevelopment entries as well as messages; a monthly component-11 equality
check cannot recover transient state that a native handler has consumed. NM19-A gives the exact
first-event trace needed for the Port of Shiganshina case. Nothing in this supplement permits
absorbing a generator disagreement or copying a native chosen configuration into the replay.

### 17.4 Worked vectors (from foundations §6.1; implementers compare their generator against these)

Session seed `S = 0x12345678`, so the seed is `derive(0x8C09C379) = 0x9E3D9501`. After seeding, `state[0] =
0x3508F7E2`, `state[623] = 0x06CD7FA3`, `left = 624`, `k = 0`.

| Draw | Word (tempered) | Use | Result |
|---|---|---|---|
| k = 0 | `0xB0602CE4` | configuration, 3 candidates | `(w × 3) >> 32` = 2 → third candidate |
| k = 1 | `0x88C93DBB` | prop object, 4 IDs | 2 |
| k = 2 | `0xDD056A8E` | flora object, 2 IDs | 1 |

Reload crossing, the reason `left` must be captured with the words: the same generator after 622 draws holds
`k = 622`, `left = 2`, `next = 0x00B45228`. A lot with 3 candidates and two objects (4 IDs, then 2 IDs) draws
`state[622]` → `0x902AFD71` → 1 (`left` 1), `state[623]` → `0xC2CB90BE` → 3 (`left` 0), then reloads and draws the
new `state[0]` → `0xB7E822C6` → 1 (`left` 623, `k` 1). The captured words are the pre-reload words: the replayer
must run the reload itself on them, and the words alone (without `left`) do not say which word is next.
