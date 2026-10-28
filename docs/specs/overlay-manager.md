# Spec: terrain overlays, decals and shadows (`cSTEOverlayManager`)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`). Implementers must work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled
or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[compat-limit]* depends on something FreeCity4 cannot reproduce, so only the rule is matched.

Related specs: [terrain-view.md](terrain-view.md) (the terrain view that owns the four managers, and its land and
water passes), [terrain.md](terrain.md) §15.5 (the view's `+0x78`–`+0x84` and the terrain's `+0xF8`),
[draw-context.md](draw-context.md) (the states, texture stages and texture transforms used here),
[texture-bindings.md](texture-bindings.md) (the binding factory), [render-properties.md](render-properties.md).

## 0. Summary

An overlay manager keeps textured **decals** and **shadows** that are drawn onto the terrain: each overlay is a
texture with a world-to-texture transform and a rectangle of cells it covers, and is drawn by redrawing those
terrain cells with the texture projected on them. Callers (tools, lots, buildings, effects) get a handle when
they add an overlay and use it to move, show, hide, recolour, fade or remove it. The terrain view owns four
managers: static and dynamic, each for land and for water.

## 1. The object *[compat]*

- 0xFC bytes, constructor `0x0073BC20`; vtables: `+0x00` `0x00AB39E8` (the interface, IID `0x895D6544`, 28 slots,
  §3), `+0x04` `0x00AB39D0` (the draw interface the draw registry calls: slot 3 `Draw`, `0x00739490`), `+0x08` a
  `cRZUnknown`, `+0x10` the message target. `QueryInterface` (`0x00735260`) answers `0x895D6544` with the object,
  anything else through the `cRZUnknown`.
- `Init(view, dynamic, water)` (`0x0073CB60`): keeps the view (`+0x8C`), the two flags (`+0x15` dynamic, `+0x16`
  water) and the initialised flag (`+0x14`); takes the texture binding factory (`+0x90`) and the terrain (`+0x88`);
  from the terrain's cell counts (`+0x5C`, `+0x60`) and cell width the cell scale (`+0x68` = 1 ÷ width) and the city
  size in metres (`+0x6C`–`+0x74`); the binding for key `0xAA592B77` *[inferred: the shadow texture of §4.3,
  `+0xD4`]*; the draw states (§4.1); for a static manager the per-cell index (`0x0073CB10`, §2.2); subscribes to
  `0xC9F775BB`; and reads `kShadowQuality` (integer 12, `+0x18`) and *[inferred]* `kShadowDebug` (integer 4,
  `+0x1C`).
- The terrain view creates four and initialises them as (static, land) `+0x78`, (static, water) `+0x7C`,
  (dynamic, land) `+0x80`, (dynamic, water) `+0x84`, registered at draw layers 2, 2, 5, 5 with priorities 0, 1100,
  1000, 1100 (terrain.md §15.5.4). The terrain keeps the first at its `+0xF8`.
- `0x0073AC90` (on shutdown) unsubscribes, releases the view, factory and terrain and clears overlay/handle
  live ranges/free queues. It is distinct from destruction; see §1.2 for retained groups/index/shadow binding.

### 1.1 Initialization and repeat calls (OV4, proven subset)

The constructor (`0x0073BC20`) sets initialized false, shadow quality 2, second-stage setting 0; parent, terrain,
factory and the three arenas' begin/end/capacity pointers are null. Each arena has free-head and free-tail −1.
It sets the shadow binding and index storage pointers null. It does **not** initialize the dynamic/water bytes,
cell dimensions, reciprocal scale, city rectangle or counters; those are established by Init. No public
getter/draw contract is supplied before Init.
`0x0073BC6A–0x0073BC79`, `0x0073BCF5–0x0073BD9B`.

Init (`0x0073CB60`) takes `(parent, dynamicByte, waterByte)`, returns no specified result and has no initialized
guard or rollback. It replaces the parent with AddRef-before-Release when different, stores both bytes and sets
initialized true **before** looking up the texture factory (framework class/interface `0x1AC0E11A/0xFAC0E219`).
It similarly retains the new factory and global terrain and releases replaced references. Identical pointers
avoid those reference operations. Terrain slots 7/8 provide signed cell counts; slot 6 returns the cell width.
Init stores width as binary32, reciprocal as binary32 of the x87 reciprocal, and city bounds `(0,0,nx,nz)`.
It resets counters and `+0x94`, releases any old `+0xD4` binding, then requests key `0xAA592B77` through
**factory slot 5** (byte offset `0x14`), into `+0xD4`. It initializes draw states, rebuilds the static index when
dynamic is zero, registers message `0xC9F775BB` when that global server exists, then reads quality/second-stage
settings. Repeated Init executes this sequence again and does not clear the overlay/handle/group arenas.
Missing factory/terrain/framework services do not produce a clean false result: required pointers are later
dereferenced without checks, after earlier mutations. Atomic rejection of repeated/failed Init is not native
behavior. Evidence: `0x0073CB60–0x0073CD3B` (including binding call `0x0073CCE7`).

The overlay-record constructor (`0x00735F40`) sets flags 1 (shown), draw-state byte 0 and collection stamp 0,
first matrix identity, alpha 1, colour from the three binary32 defaults at `0x00B0D4E4/E8/EC` (all 1 in the
shipped image), and binding/source pointers null. The cell rectangle and first free-list word are not initialized
there; allocation sets the live word −1 and placement establishes bounds before observation. The second matrix
is filled from scratch/default global `0x00B4B3C8`; no usable second-transform value is promised until a shadow
add fills it. Likewise the handle allocator copies a complete owner-constructed placement/aspect/target record;
there is no independently initialized public default handle. Evidence: `0x00735F40–0x007360F9`, with alpha's
literal 1 established at `0x00735FF2`, colour loads `0x007360BE–0x007360DC`, and allocator §2.3.

### 1.2 Shutdown versus destruction (owner boundary)

Shutdown `0x0073AC90–0x0073AD70` runs without an initialized guard. It unsubscribes the message target if
the server exists, then nulls/releases parent, terrain and factory, in that order. It clears the overlay live
range, releasing source and binding references in increasing record order, and resets overlay free-head/tail
to −1; then clears the handle live range and resets its free-head/tail to −1; initialized becomes false last.
It retains the arena allocations. It does **not** clear/reset the group arena/free queue, cell-index heads,
index dimensions/stamp, shadow binding `+0xD4`, cached settings, or statistics. In particular shutdown alone
does not remove each overlay from the retained index. Draw/use between Shutdown and valid reinitialization
has no supported contract. Overlay range destruction releases source before binding, including payloads in
freed slots (`0x00737E10–0x00737E4A`); ordinary Remove does neither (§2.3).

The destructor body `0x0073BE00–0x0073BF0A` is a separate path: it destroys all index lists and frees index
storage, releases `+0xD4`, frees group then handle storage, destroys overlay payloads and frees overlay
storage, releases factory, parent and terrain, and destroys bases. It does **not** invoke Shutdown or
unsubscribe. The index-list destructor `0x0073A170–0x0073A1A7` visits heads in array order, frees each chain
from head to tail and nulls the head. The deleting-destructor wrapper `0x0073C980–0x0073C99D` additionally
frees the object when argument bit 0 is set. Subscription lifetime therefore requires the explicit shutdown
protocol; destruction must not be described as automatically performing it.

Paper lifecycle discriminator: after removing an overlay, its references remain; Shutdown releases payload
references in the overlay range but retains `+0xD4` and group/index allocations. Destruction releases that
binding and storage. Neither path submits a terrain redisplay request per removed record.

## 2. Records and handles *[compat]*

### 2.1 Two tables

- **Overlays** (`+0x98`, 0xB4 bytes each, with a free list `+0xA4`, `+0xA8`): a free-list link (−1 while in use),
  flags (`+0x04`: bit 0 shown, bit 1 repeat the texture, bit 3 shadow, bit 4 decal, bit 5 a texture matrix is set,
  bit 6 self-lit, bit 7 drawn in snapshots), the draw state index (`+0x08`, §4.1), the covered cell rectangle
  (`+0x0C`, x0, z0, x1, z1), a 4 × 4 world-to-texture matrix (`+0x1C`), the alpha (`+0x9C`), the colour (`+0xA0`, RGB),
  the texture binding (`+0xAC`) or a texture source object (`+0xB0`).
- **Handles** (`+0xAC` table, 0x30 bytes each): what callers hold. A handle with the high bit set names a handle
  record: its placement (nine floats from `+0x04`) and the overlay it drives (`+0x2C`). An overlay index whose high
  bit is set names a **group** (`+0xC0`, 0x24 bytes: eight overlay indices), and every call on a group is applied to
  its eight parts where the method supports group recursion. Handle −1 is "none"; scalar getters return their
  sentinels and placement getters leave their output untouched (§2.3).

Counters: the last invoked nonempty decal/shadow passes (`+0xD8`, `+0xDC`); skipped passes preserve them (§4.4).

### 2.2 The cell index (static managers)

A static manager keeps, per terrain cell, the overlays that cover it (`0x00738230` adds an overlay's rectangle,
`0x007382E0` removes it), so that a frame only visits the overlays of visible cells (§4). Any change to an overlay
marks it changed (`0x00737D40`); a round decal (slot 7) also redisplays the terrain under each part's rectangle
(terrain slot 68).

Native bounds are half-open for cell iteration and redisplay: positive area requires x1>x0 and z1>z0.
Intersection helper `0x0048ED90` uses signed comparisons; a strictly separated rectangle becomes all zeros,
whereas touching edges retain a zero-width/height intersection at that edge. Thus clipping `(−3,2,−1,4)`
against `(0,0,8,8)` gives `(0,0,0,0)`, while `(−3,2,0,4)` gives `(0,2,0,4)`. Bounds are not swapped to
normalize inverted input. `MarkChanged` invokes terrain slot 68 `(0,waterByte,&rect,0)` only for positive area
(`0x00737D40–0x00737DA3`).

`BuildCellIndex` establishes shift 0, grid nx by nz, nx*nz null list heads and a zero 16-bit collection stamp
(`0x0073CB10–0x0073CB57`). For shift k in the indexing routines, covered coarse bounds are arithmetic shifts
`[x0>>k,z0>>k,(x1+(2^k−1))>>k,(z1+(2^k−1))>>k]`. Addition traverses X outermost, Z innermost and **prepends**
the overlay ID to each list, without duplicate suppression (`0x00738230–0x007382D8`). Removal traverses Z
outermost, X innermost and unlinks every matching ID while preserving others' order (`0x007382E0–0x0073838B`).
Collection appends on first encounter, using overlay `+0x0A` versus the manager's incremented 16-bit stamp;
it does not sort the selected IDs (`0x00738480–0x007384B7`, `0x0073854E–0x00738580`). Exact visible-row endpoint
inputs and stamp-wrap effects remain under OV8. A sorted/deduplicated host selection is not this owner algorithm.

### 2.3 Allocation, reuse, getters and invalid records (OV4/OV5)

All three arenas allocate ascending zero-based indices when no freed entry exists. Removal appends to a
free **queue**, and allocation takes its head: reuse is **FIFO**, independently for overlays, handles and groups.
The first word is −1 while live; a free entry's word links to the next (the tail word is set to 0 and is not
followed when head equals tail). No generation is encoded in an ID. A stale handle may therefore operate on a
new occupant of the same slot. There is no portable result for an out-of-range ID or null required pointer;
native paths dereference the computed address, rather than rejecting the operation atomically.
Evidence: handle allocate/free `0x00739EC0–0x00739FB1` / `0x00736210–0x0073624D`, group allocate
`0x00739FC0–0x0073A06D`, overlay allocate `0x0073AEC0–0x0073AF7B`, removal `0x007388C0–0x00738A08`.

Ordinary source/binding decals allocate an overlay **and** an indirect placement handle, even if the clipped
rectangle is empty. Round decals allocate eight overlays, a group and one indirect handle. The city-wide
rectangle add returns a **direct overlay ID**, without a placement handle: not every add allocates one.
Slots 12–14 ignore nonnegative direct IDs. For indirect removal, the handle is freed first; group removal
recurses over its eight indices in array order and then frees the group. Direct overlay removal on a static
manager removes its index entries and marks the old rectangle before checking whether its free-list word
already says free. Payload is left in freed records (including binding/source references); removal itself
does not release them. Reuse/arena destruction, rather than removal, disposes retained payload references.
There is no general live-record validation on setters/getters. Evidence: `0x0073B127–0x0073B141`,
`0x0073C069–0x0073C082`, `0x0073B2CF–0x0073B38E`, `0x0073B9A7–0x0073BA04`, `0x007388F2–0x00738A08`.

Scalar getters accept direct overlay IDs or resolve an indirect handle. If that handle resolves to a group,
they read **part 0** only, never a reduction/average across parts. A group ID by itself is not a public handle.

| Slot | Actual value | −1 behavior |
|---|---|---|
| 22 (`0x00736580`) | shown: `(flags & 1)`, boolean | false |
| 23 (`0x007365E0`) | binary32 alpha at `+0x9C`, returned on x87 stack | +0 |
| 24 (`0x00736640`) | three colour words from `+0xA0` | writes three zero words |
| 25 (`0x007366C0`) | zero-extended **byte** at `+0x08` | 0 |
| 26 (`0x00736720`) | self-lighting: `(flags >> 6) & 1`, boolean | false |

Slot 24 has output first and handle second (callee pops 8); it returns the output address in EAX. No null
output check exists, even for handle −1. Slot 14's no-handle/direct-ID path, in contrast, does not touch output.
These identities follow the field loads, not the earlier descriptive function name `IsShown` for slot 26.

Slots 16/18/19 write shown/colour/draw-state and mark a nonempty static rectangle even when supplied values
are identical; dynamic writes omit redisplay. Slot 19 writes only the low byte. Slot 13 always re-places;
static re-placement with the same clipped rectangle still marks it once, and a changed rectangle removes old
index entries, marks old, writes new bounds, inserts new entries, then marks new. Slot 12's tolerance is in §3.2.
Evidence: `0x00737EB8–0x00737EED`, `0x00738078–0x007380B0`, `0x00738128–0x0073814D`,
`0x007393DF–0x0073943E`. These are owner operations, not caller-supplied computed rectangles or redraw results.

Paper reuse vector: allocate indices 0,1,2; remove 1 then 0; the next allocations reuse 1 then 0, then append 3
if no other frees occur. For a group with differing part colours/alpha, getters observe part 0 only.

### 2.4 Native observation: arena headers and liveness (OV12)

These are **32-bit process addresses**, relative to the complete manager/interface at `+0x00`, not to its
draw interface at `+0x04`. Each arena is a contiguous constructed range followed by reserved storage.
The end pointers are exclusive byte addresses, not counts, last-element pointers, or public handles.

| Arena | Begin | Constructed end | Allocation end | Free head | Free tail | Record stride |
|---|---|---|---|---|---|---|
| Overlays | `98` | `9C` | `A0` | `A4` | `A8` | `B4` |
| Placement handles | `AC` | `B0` | `B4` | `B8` | `BC` | `30` |
| Groups | `C0` | `C4` | `C8` | `CC` | `D0` | `24` |

All offsets and strides in this table are hexadecimal. For stride s and header `(b,e,c)`, constructed count
is `(e-b)/s`, including freed entries; reserved count is `(c-b)/s`. Record i is at `b+i*s`, for
`0 <= i < constructedCount`. Never enumerate `[e,c)`. All-null is the constructor's empty state; a retained
allocation may instead have `b=e` with nonzero addresses. Growth may move the entire arena. No record address
is a stable identity across mutations. Evidence: constructor `0x0073BD22–0x0073BD79`; append/reuse
`0x00739ECC–0x00739F29`, `0x00739FC0–0x0073A06D`, `0x0073AECC–0x0073AF7B`; overlay relocation
`0x0073A070–0x0073A16C`.

For **each** arena, a constructed record's first signed word is exactly −1 when live. Removal changes it to
zero, then links the previous free tail to that record's **zero-based index** and updates the tail. Thus a
free tail has word zero, including a singleton free entry at index zero. Zero is not a null record address
and does not terminate a multi-entry queue: index zero can be the next free entry. To enumerate the queue,
start at head, emit that index, and stop **after emitting tail**, without interpreting tail's link. Empty has
both head and tail −1. A live last constructed record still has word −1; it is not the free tail. There is
no live/free ambiguity requiring guesses from flags, resource pointers, rectangle area, or whether an entry
is referenced by a placement handle. Evidence: handle free `0x00736210–0x0073624D`; group free
`0x0073894A–0x00738980`; overlay free `0x007389B5–0x00738A04`; allocation restores −1 after copying.

A read-only observer can emit live records in ascending arena index and separately emit the exact FIFO free
queue. That ascending enumeration is a diagnostic convention, not a claim of creation order: reuse preserves
indices and destroys generation information. Resolve public indirect ID `0x80000000 | i` through live handle
i's `+0x2C`; a nonnegative target is an overlay, a high-bit target is group `target & 0x7FFFFFFF`. Its eight
part words are overlay indices in part order. Direct overlay IDs need no handle. Enumerate all three arenas
independently, so direct/model-shadow overlays are not omitted and group parts are not counted twice.

The original supplies no snapshot lock, public enumerator, or corruption-validation protocol. The observation
contract therefore requires a quiescent point on the owning thread, outside add/remove/placement/draw/shutdown
and factory changes. Copy the data while addresses remain valid; retain no native pointers afterward. An
adapter must bound/check address ranges, aligned/divisible extents, indices and queue/list cycles before
following them, and report a failed/incomplete sample if inconsistent. These checks are observer safeguards,
not native validation behavior. A changed header invalidates a sample; an unchanged header alone does not
prove no intervening in-place mutation. Do not call AddRef, getters with unknown side effects, collection or
Draw to make a sample: collection itself writes stamps and Draw changes counters/state.

### 2.5 Complete constructed-record fields (OV12)

Offsets below are hexadecimal and relative to the **record**, not the manager. Numeric payloads are
little-endian; floats are stored binary32 words. Preserve their bit patterns when reporting observations.

| Overlay offset | Width | Meaning |
|---|---|---|
| `00` | 4 | Signed liveness/free-queue link (§2.4) |
| `04` | 4 | Flags (§2.1); retain the complete word, including bits not named there |
| `08` | 1 | Draw-state index; not a 32-bit field |
| `09` | 1 | Padding; not initialized/copied as a semantic field |
| `0A` | 2 | Last static-collection stamp, compared to manager `+0xE0` |
| `0C,10,14,18` | 4 each | Signed covered-cell bounds `(x0,z0,x1,z1)` |
| `1C–58` | 16 × 4 | Primary matrix M, four consecutive rows; entry `(r,c)` at `1C+4*(4*r+c)` |
| `5C–98` | 16 × 4 | Separate secondary shadow matrix N, same storage convention |
| `9C` | 4 | Alpha |
| `A0,A4,A8` | 4 each | Colour R,G,B |
| `AC` | 4 | Retained `cS3DTextureBinding*`, or null |
| `B0` | 4 | Retained texture-source interface pointer, or null |

The two matrices occupy different fields; the secondary is neither a pointer nor an ownership/geometry
record. Its keyed-shadow derivation is §3.5 and model-shadow derivation §3.7. The default constructor fills
the primary identity and secondary from its sixteen default-word loads; rectangle-helper creation does not
derive a secondary transform (§3.6). Do not assume every shadow's N can be derived by the keyed-shadow rule.
Evidence for byte/stamp/padding distinction: `0x00735F44–0x00735F4A`, `0x00735A02–0x00735A0C`;
matrix/scalar/reference extents: `0x00735F4E–0x007360F9`, copy `0x007359F0–0x00735CA6`;
model secondary destination `0x0073C7A2–0x0073C82B`.

| Placement-handle offset | Width | Meaning |
|---|---|---|
| `00` | 4 | Liveness/free link |
| `04–24` | 9 × 4 | Public placement words in §3.2 order |
| `28` | 4 | Retained texture aspect used by placement; not the public record's ninth word |
| `2C` | 4 | Overlay index or high-bit group index |

| Group offset | Width | Meaning |
|---|---|---|
| `00` | 4 | Liveness/free link |
| `04–20` | 8 × 4 | Part 0 through part 7 overlay indices |

Handle copies cover all twelve words (`0x00736280–0x007362DB`, `0x00739F54–0x00739FB1`);
group copies cover nine (`0x00739FC0–0x0073A06D`). These records contain no texture key, model pointer,
creation timestamp or call-history pointer. The overlay's reference fields own references independently of
the placement/group records. Freeing leaves payloads, including references, in constructed storage; those
payloads are **not live overlays**. Copy/reuse/disposal performs reference operations; observation does not.
Compare named fields, not a raw `memcmp` over padding, allocator addresses or reference counts.

### 2.6 Read-only static-cell enumeration (OV12)

Manager `+0xE4` is the 32-bit index shift, `+0xE8/+0xEC` the signed coarse X/Z dimensions, and
`+0xF0/+0xF4/+0xF8` the begin/constructed-end/allocation-end of a vector of **4-byte list-head pointers**.
These are not selected-overlay IDs. `BuildCellIndex` sets shift zero, dimensions to terrain `+0x5C/+0x60`,
resizes to nx*nz null heads and resets the 16-bit stamp at `+0xE0` (`0x0073CB10–0x0073CB57`,
resize `0x0073CAB0–0x0073CB0A`). On an ordinarily initialized static manager, cell `(x,z)` uses head
`heads[z*nx+x]`. Each heap node is eight bytes: next pointer at `+0`, direct overlay ID at `+4`; null ends
the list. Read the head then next pointers, preserving duplicates and encounter order. There is no sentinel
node and no per-cell count. Nodes do not embed overlays and their next words are addresses, unlike arena
free links. Evidence: `0x00738280–0x007382B5`, unlink `0x00738344–0x00738378`.

Emit cells in row order (Z outer, X inner) with each stored list intact for a deterministic diagnostic dump;
native insertion traversal is still X outer/Z inner and **prepending**, as §2.2 says. Neither sorting IDs nor
rebuilding lists from current rectangles recovers their native order. A static re-placement can change that
order without changing the set of live records. Dynamic managers do not maintain this index; a normally
initialized dynamic manager's null index is not a missing sample. Repeated Init/shutdown quirks in §1 still
apply: leftover storage is not proof that a dynamic manager uses an index. Selection order and stamp wrapping
are §4.5. Merely reading a cell must leave both per-record and manager stamps untouched.

**Inactive headers (OV14).** An ordinary dynamic manager has no static index: shift at `+0xE4` and
dimensions at `+0xE8/+0xEC` are **not applicable and have no guaranteed values**, including no guarantee
of zero, positivity, agreement with terrain dimensions, or a usable shift range. The constructor zeros
only the index vector's three pointers (`0x0073BD85–0x0073BD91`); it does not initialize those three
scalar words. Init establishes the separate terrain dimensions at `+0x5C/+0x60`, but invokes
BuildCellIndex only for dynamic byte zero (`0x0073CCF1–0x0073CCF8`). Consequently an observer retains
inactive scalar words diagnostically without validating, shifting by, multiplying, or dereferencing through
them. Report no applicable static cells, and still enumerate every live overlay, placement and group.
This does not waive arena integrity checks. After a static-to-dynamic repeat Init, old index storage and
header words can remain (§1.1); they are still inactive, not a dynamic cell-membership contract.

### 2.7 Texture identity, dimensions and the limit of resident data (OV12)

The overlay stores **references**, not image pixels or a resource key. For a binding at overlay `+0xAC`,
the binding's 16 bytes are `+0` pointer to a per-frame array of 32-bit driver texture handles, `+4` frame
count, `+8` owning factory pointer, `+0xC` reference count (`0x008090C0–0x008090DD`). A texture handle is
opaque driver identity, never a CPU image pointer. Decal drawing uses frame zero (`0x0073688A–0x0073689B`),
even if the binding has more frames. Binding pointer equality can establish sharing **within one sample**;
neither a native pointer nor a driver handle is a portable resource identity across executions.

For a binding belonging to the manager's factory `+0x90`, factory slot 8 (`0x00809E10`) writes this info:

| Output offset | Width | Value |
|---|---|---|
| `00,04` | 4 each | Resource group, instance |
| `08,0C,10` | 4 each | Binding level count, width, height |
| `14,15,16,17` | 1 each | R,G,B,A bit counts |
| `18` | 1 | Bits per texel |

The function has no success boolean contract; an unknown binding leaves output untouched. A read-only
adapter need not invoke it: factory `+0x2C/+0x30` delimit the vector of bucket pointers in its binding map;
nodes contain next at `+0`, binding pointer at `+4`, record pointer at `+8`. Search for exact binding pointer
equality in bounded chains. The 0x34-byte binding record has binding at `00`, frame-handle-array pointer at
`04`, in-use byte at `08`, instance/group at `0C/10`, frame/level counts at `14/18`, width/height at `1C/20`,
accounted byte size at `24`, RGBA bit counts at `28–2B`, bits-per-texel byte at `2C`, and 32-bit internal format
at `30`; intervening padding has no comparison contract. This lookup can independently distinguish an
unknown binding from a valid all-zero info result. Evidence: map lookup and output `0x00809E10–0x00809E9A`,
record construction `0x0080A110–0x0080A319`. Use the binding's own factory identity when testing ownership;
do not assume an externally passed binding belongs to manager `+0x90`.

Instance zero denotes an unkeyed record. Nonzero instance with group identifies the current resource image
key `(type=0x7AB50E44,group,instance)`, as texture-bindings §§1.8–1.10 specifies. These dimensions are the
**binding** dimensions after power-of-two/LOD processing (§1.4 of that spec), not necessarily source image
dimensions. An indirect placement's stored aspect is the value retained at creation; deriving it again from
today's binding info is not proof of the original input. Changing the factory resource group can unkey live
records without changing the overlay pointer or its pixels.

For a source at overlay `+0xB0`, the owner requests dimensions through source slot 3 (`+0x0C`, complete output
two unsigned 32-bit words width,height), retains the source with slot 1, releases with slot 2, and asks slot 4
(`+0x10`) for the texture handle during drawing. Slot 4 has **no stack arguments**; the zero pushed at
`0x00736892` belongs to SetTexture's stage argument. The complete adapter ABI is §2.7.1. Evidence: source dimensions and aspect
`0x0073B035–0x0073B06D`, references `0x0073B09E–0x0073B0C7`, draw `0x0073689D–0x007368AB`.
There is no universal source-object field layout or resource key: the retained pointer is an interface whose
implementation owns its state. With a binding present drawing takes that route first. Copying the source
pointer/vtable identifies sharing and implementation locally; calling an unknown source implementation is
not a guaranteed mutation-free observation of dimensions or pixels.

**CPU texture data cannot be enumerated from these overlay records or binding-info fields.** Factory upload
takes transient image-info data and forwards it to driver slot 61 (vtable byte offset `0xF4`); the binding
record retains handles and metadata, not those level-data pointers (`0x00809B90–0x00809CC6`, upload call
`0x00809C87`; texture-bindings §§1.5,1.12). A keyed
resource can be loaded independently and decoded under graphics-formats, but that is a source-resource
comparison, not proof of the current GPU contents: unkeying, runtime uploads and restore can change that
relationship. Unkeyed bindings and arbitrary sources have no general CPU-byte recovery through this owner.
An observer must report `data unavailable` rather than treating a handle as pixels or manufacturing a hash.
Verifying contents requires observing the original image-info/frame/level upload inputs (and subsequent
uploads/restores), or a separately specified driver/source readback contract. This limitation is deterministic,
not a nondeterministic exclusion from a purported full-record/texture shadow.

### 2.7.1 Native source call contracts and worked calls (OV16)

These are 32-bit x86 **thiscall** methods on the retained source interface itself: ECX is the source
pointer, and its first word supplies the vtable. No manager, factory, binding, frame index or texture stage
is an additional source-method parameter. The names below describe the calls; they do not assert original
source-language declarations. The exact callable adapter prototypes, in words, are:

| Source slot | Adapter prototype | Argument stack bytes / callee cleanup | Result channel |
|---|---|---|---|
| 3, vtable `+0x0C` | Void-result thiscall on source, with one pointer to writable dimensions storage | 4 / 4 | The pointed-to 8 bytes, width then height, both unsigned 32-bit integers |
| 4, vtable `+0x10` | Thiscall on source, with no parameters, returning one opaque 32-bit driver texture handle | 0 / 0 | EAX; the native string source returns zero on render failure |

For slot 3, the caller must provide a nonnull pointer to **an 8-byte result object with 4-byte alignment**; larger
scratch storage is permitted but unnecessary. Output offsets `+0` and `+4` are width and height.
There are no trailing info fields, padding to reserve, pixel pointers, constructor or destructor in this
result. The method receives one pointer to the first word, not two output pointers or a pointer-to-pointer.
The caller owns the storage throughout the call; it does not receive or release an allocated info object.
The original caller neither initializes the output nor tests a success return before reading both words
(`0x0073B035–0x0073B05C`). The adapter must read the supplied storage, not interpret EAX as dimensions,
an info-object pointer or a success boolean. No x87 or register-pair result is involved.

A concrete source used by text decals proves the full write extent and the two method entries:
the string-texture vtable `0x00AA2A60` has slot 3 `0x005D8130` and slot 4 `0x005D8020`.
Its size method zero-extends its stored unsigned 16-bit dimensions to those two 32-bit outputs,
writes only bytes `+0` through `+7`, and removes the one pointer argument (`0x005D8130–0x005D8143`).
That body leaves **EAX equal to the supplied output address**; this is not a separate source-owned info
pointer. The void-result adapter above deliberately consumes no return register. Whether a source-level
declaration spells this as an output parameter or a caller-storage aggregate return does not change the
machine call: ECX plus that single stack pointer, with an 8-byte memory result and four-byte callee cleanup.
In particular, an aggregate-return wrapper must not introduce a second hidden argument or return the two
words in EDX:EAX. String-source creation and its use as the overlay source are independently linked by
manager slot 5 at `0x00AA2AE8`, creation `0x005D8179–0x005D8189`, and text-decal calls
`0x005F81C7–0x005F81D5`, `0x005F787A–0x005F788A`; see also glyph-textures §§1.4,3.

For slot 4, the concrete source delegates to its manager with this source as an **internal** argument,
then returns its manager's handle without removing any caller argument (`0x005D8020–0x005D802B`).
The manager's cached path returns the binding's frame-zero handle; a rendering failure returns zero
(`0x005D7FF8–0x005D8016`, glyph-textures §1.4). Calling this method can therefore populate or reorder
the cache; it is not a pure field getter. The overlay forwards the full returned word to SetTexture,
without an AddRef or zero-handle rejection. The already-pushed stage zero stays on the stack across the
source call; after pushing the returned handle, SetTexture consumes **handle, stage** and removes eight
bytes (`0x00736892–0x007368AB`, `0x007D4070–0x007D40AD`). A source must not consume that pending zero.

**Worked calls (instruction-derived test vectors, not an in-game measurement):**

| Call and inputs | Required outputs and stack balance |
|---|---|
| Slot 3 of an initialized string source whose dimensions are width 512, height 128. ECX is that source. Supply result address `P=0x0012F004`, initially eight `0xA5` bytes, with guard bytes immediately before and after. Caller ESP before pushing P is `0x0012EFE0`; at method entry the return address is at `0x0012EFD8` and P at `0x0012EFDC`. | Output words at P and P+4 become `0x00000200`, `0x00000080` (little endian). Both guards remain untouched; P requires neither 8- nor 16-byte alignment. This native size body leaves EAX=P; consume the memory result. On return caller ESP is `0x0012EFE0`. With no binding, AddOverlay retains aspect height/width = binary32 0.25. |
| Slot 4 of the same source, with a live manager and an existing cache entry whose binding's first handle is `0x13572468`. ECX is that source; no source arguments are pushed. To mirror DrawDecals, caller ESP starts at `0x0012EFE0` and SetTexture's pending stage zero is at `0x0012EFDC` before the source call. | EAX is `0x13572468`; the pending zero survives and caller ESP immediately after the source return is `0x0012EFDC`. The cached entry becomes used/newest as in glyph-textures §1.4. Pushing the handle and calling SetTexture submits handle `0x13572468`, stage 0; SetTexture returns ESP to `0x0012EFE0`. The handle is never dereferenced as CPU image data. |

## 3. The interface (`0x00AB39E8`) *[compat]*

| Slot | VA | Call | Behaviour |
|---|---|---|---|
| 3 | `0x0073C0B0` | `AddDecal(source, positionXZ, width, turns)` | a decal textured from a texture source object (its size from the object's slot 3) |
| 4 | `0x0073BF10` | `AddDecal(binding, positionXZ, width, turns)` | the same from a texture binding; −1 without one |
| 5 | `0x00736150` | `AddDecal(key, positionXZ, width, turns)` | the factory's binding for the key (factory slot 6), then slot 4 |
| 6 | `0x007361B0` | `AddDecal(key, positionXZ, width, turns)` | the same with factory slot 5 |
| 7 | `0x0073B170` | `AddRoundDecal(key, positionXZ, width, turns)` | a **group** of eight outline rectangles omitting the centre (§3.1) |
| 8 | `0x0073C9A0` | `AddCityDecal(key)` | factory slot 5; direct rectangle overlay, flags `0x13`, exact matrix in §3.6 |
| 9 | `0x0073B3B0` | `AddShadow(key, positionXZ, width, turns, flipV)` | factory slot 6 resolves the key; shadow matrices as §3.5 |
| 10 | `0x00735480` | `AddShadow(groupWord, positionXZ, width, turns, flipV)` | −1 when `kShadowQuality` is 0; otherwise constructs the key described in §3.5 and calls slot 9 |
| 11 | `0x0073C0D0` | `AddModelShadow(binding,n,source,pairs,box,descriptor)` | direct shadow record; lighting projection, quality-dependent bounds and owned secondary transform (§3.7) |
| 12 | `0x0073AD80` | `SetPosition(handle, xz)` | for a handle record: when the two floats change, the overlay is placed again |
| 13 | `0x0073ADE0` | `SetPlacement(handle, nine floats)` | the whole placement, then placed again |
| 14 | `0x00736490` | `GetPlacement(handle, out)` | the nine floats |
| 15 | `0x007388C0` | `Remove(handle)` | a group's parts are removed, then the record is freed |
| 16 | `0x00737E50` | `SetShown(handle, on)` | flag bit 0 |
| 17 | `0x00737F00` | `SetAlpha(handle, a)` | `+0x9C`; a static manager only redraws when a reaches 0 or 1 or moves by more than the step `0x00AB39A0` |
| 18 | `0x00738010` | `SetColor(handle, rgb)` | `+0xA0` |
| 19 | `0x007380C0` | `SetDrawState(handle, index)` | `+0x08` |
| 20 | `0x007364F0` | `EnableSelfLighting(handle, on)` (descriptive name) | when the low bit of `on` is set, OR flag bit 6; false does not clear it; groups recurse over all eight parts |
| 21 | `0x00738160` | `SetTextureScale(handle, size, offset)` | the matrix becomes a scale of 1 ÷ size with the second axis flipped and an offset of 1 + offset ÷ size; flag bit 5 |
| 22–26 | `0x00736580`–`0x00736720` | getters | shown; alpha; colour (output first); draw-state byte; self-lit (§2.3) |
| 27 | `0x007352A0` | `GetStatsString(out)` | "%d decals, %d shadows" from the counters |

Slots 0–2 are `QueryInterface`, `AddRef`, `Release`. The ordinary decal add signatures in the table above use
`positionXZ, width, angleInTurns`, rather than an input nine-float record. Slots 5/6 forward all three. The exact
stored placement and owner computations are in §3.2. `ComputeCellRect` (`0x007354E0`) is a different helper;
its margin must not be imported into ordinary `PlaceOverlay` coverage.

### 3.1 Round decals: eight outline parts (`0x007394B0`, OV9)

This is an **outline partition with an omitted centre**, not eight filled-disc strips or exact cell-circle
intersection. Slot 7 resolves its key through factory slot 5 and returns −1 before allocation if the binding
output is null. It allocates eight shown decal records, retains the same binding in each, allocates one group,
and places the group without redisplay. All eight parts receive the same full-footprint primary texture
matrix/repeat flag computed by §3.2; a part is not independently stretched to fill its rectangle. The public
width is a full width, not a radius. Evidence: `0x0073B170–0x0073B2F1`, group matrix stores
`0x0073A67A–0x0073AB75`.

The partition helper receives the **unclipped binary32 cell-space bounding rectangle** already produced by
placement, including any rotation/aspect expansion. Call its endpoints `(fx0,fz0,fx1,fz1)`. It first converts
that rectangle to integers `(x0,z0,x1,z5)` using floor minima and ceil maxima (§2.2). Calculations below use
the original floating endpoints, not the city-clipped bounds. The formulas apply to ordinary finite city-sized
inputs under native round-to-nearest FP state; E/F have the meaning in §3.2.

| Quantity | Native evaluation |
|---|---|
| W, H | `W=E(fx1−fx0)` retained in x87; `H=F(E(fz1−fz0))` |
| cx, cz | `F(E(fx0+fx1)×1/2)`, `F(E(fz0+fz1)×1/2)` |
| R² | `F(E(W×1/2)²)` |
| I² | `F(E(E((1−g8)×W)×1/2)²)` |
| n1, n2 | `ceil(E(E(g4×H)×1/2))`, `ceil(E(E(g0×H)×1/2))` |
| Four sample rows | `z1=z0+n1`, `z2=z0+n2`, `z3=z5−n2`, `z4=z5−n1` |

Here g0/g4/g8 are the **binary32** image globals `0x00B0D4D0/D4/D8`:
0.4000000059604645, 0.20000000298023224, 0.10000000149011612. The radius multipliers use binary64
1/2 (`0x00A92D28`), while the centre uses binary32 1/2. Replacing these constants with exact decimal
fractions changes split rows: H=20 yields n1=3 and n2=5, not 2 and 4. Source:
`0x007394BD–0x00739645`. The integer conversion uses the same bias/correction operations as
`0x00735380` (bias binary32 12582912 at `0x00AB390C`); mathematical floor/ceil here describe their
normal finite range, not a saturating conversion for arbitrary nonfinite or huge inputs.

At each sample row zi, let `S=F(E(cz−zi)²)`. The outer half-chord is
`Oi=F(sqrt(E(R²−min(S,R²))))` and the inner half-chord is
`Ji=F(sqrt(E(I²−min(S,I²))))`. The clamp is on the **stored square**, before subtraction/sqrt; no negative
radicand repair after sqrt is substituted. The fourth inner half-chord J4 alone stays in x87 precision until
its subsequent endpoint conversions rather than first passing through F. Define
`Li=floor(E(cx−Oi))`, `Ri=ceil(E(cx+Oi))`, `li=ceil(E(cx−Ji))`, `ri=floor(E(cx+Ji))`.
Outer endpoints round outward; inner endpoints round inward. Chord evaluation is at the integer split rows,
not cell centres. Evidence: `0x0073963D–0x0073979F`, endpoint conversions `0x007397A1–0x00739B9D`.

Let `b=min(r2,r3)` and `a=min(max(l2,l3),b)`. The eight pre-clipping rectangles, in **group part order**, are:

| Part | `(x0,z0,x1,z1)` |
|---|---|
| 0 | `(L1,z0,R1,z1)` |
| 1 | `(L4,z4,R4,z5)` |
| 2 | `(x0,z2,a,z3)` |
| 3 | `(b,z2,x1,z3)` |
| 4 | `(L2,z1,l1,z2)` |
| 5 | `(L3,z3,l4,z4)` |
| 6 | `(r1,z1,R2,z2)` |
| 7 | `(r4,z3,R3,z4)` |

This central-boundary adjustment permits parts 2/3 to meet when inward endpoints cross; it does not fill a
normal nondegenerate centre. No general rectangle normalization or re-partition after clipping occurs.
Assembly evidence: `0x00739B9F–0x00739CE9`. Each rectangle is independently intersected with the city's
cell bounds. For placement without redisplay, the helper writes the eight clipped rectangles directly in part
order. For re-placement with redisplay, it calls UpdatePartRect in that same order, with the remove/mark-old/
write/insert/mark-new ordering of §2.3. Evidence: `0x00739CF0–0x00739EB6`.

After initial placement, slot 7 visits parts 0 through 7, inserts each into the static cell index and redisplays
each positive-area clipped rectangle via terrain slot 68. A dynamic manager skips that index/redisplay loop.
Empty clipped parts remain allocated in the group; they are not removed or replaced by −1. The add path has
no partial-allocation failure branch or rollback loop: its only explicit −1 return is the initial missing
binding. Allocation exceptions/invalid allocation results must not be specified as a clean atomic failed add.
Evidence: `0x0073B2F6–0x0073B3A6`.

Paper vector: a square texture, zero turns, cellScale=1/16, centre `(128,128)`, width 256, city containing
cell box `(0,0,16,16)` gives parts
`(2,0,14,2)`, `(2,14,14,16)`, `(0,4,3,12)`, `(13,4,16,12)`,
`(1,2,5,4)`, `(1,12,5,14)`, `(11,2,15,4)`, `(11,12,15,14)`.
Cell `(8,8)` belongs to none; the centre still maps to texture `(1/2,1/2)` in every part's shared matrix.
Clipping the city edge changes only the intersections, not the chord samples or texture matrix.

### 3.2 Placement record and owner calculations (OV1/OV5)

The public record is nine consecutive **binary32** values, copied bit-for-bit by slots 13/14. It is not a
three-dimensional centre and three-dimensional extents. Offsets below are relative to the public record;
the handle stores it at `+0x04` and also stores an owner-derived texture aspect at `+0x28`.

| Offset | Name | Consumed by `PlaceOverlay` |
|---|---|---|
| `00`, `04` | cx, cz | world X/Z centre; the only values changed by slot 12 |
| `08` | w | full world width |
| `0C` | turns | rotation; multiplied by binary32 `0x40C90FDB` (6.283185482025146484375), then native x87 sine/cosine |
| `10` | stretch | multiplies the retained texture aspect, making depth/width |
| `14`, `18` | repeatU, repeatV | multipliers of the two texture coordinates, independent of footprint size |
| `1C` | offset | added to **both** texture coordinates |
| `20` | retained | copied and returned, but not read by this placement routine |

Ordinary binding/source adds construct `[cx,cz,w,turns,1,1,1,0,0]`. The binding form obtains texture dimensions
from factory slot 8 and retains binary32 `height/width` separately; the source form obtains dimensions from
source slot 3. This retained aspect is not overwritten by slot 13. The round add constructs the same defaults
and likewise retains the binding aspect. Evidence: `0x0073AFB2–0x0073B073`, `0x0073BF33–0x0073BFC1`,
`0x0073B1AE–0x0073B237`, copies `0x007364AE–0x007364E9`, `0x0073AE05–0x0073AE37`.

Let F mean a binary32 store/reload, E native x87 arithmetic under the caller's control word (this routine does
not change it), a = F(stretch × retainedAspect), hx = F(w × 0.5), hz = F((a × w) × 0.5). The multiplication
forming a is stored before use. In the zero-angle path the world-to-texture matrix is stored as four consecutive
rows, used with a **row vector** `[x,y,z,1]`:

| Row | Four entries |
|---|---|
| 0 | A, 0, 0, 0 |
| 1 | 0, 0, 0, 0 |
| 2 | 0, D, 1, 0 |
| 3 | Tu, Tv, 0, 1 |

The finite, nonzero-size evaluation is A = F(repeatU × E(1/w)), R = E(E(1/a) × E(1/w)),
D = F(repeatV × F(R)), Tu = F(F(repeatU × F((hx−cx) × E(1/w))) + offset),
Tv = F(F(repeatV × F((hz−cz) × R)) + offset). The first reciprocal is kept in x87 precision;
R is also stored to binary32 before the multiplication yielding D. This gives the familiar real-number
coordinates `u=repeatU*(0.5+(x−cx)/w)+offset`, `v=repeatV*(0.5+(z−cz)/(a*w))+offset`, but that algebraic
simplification is not the rounding contract. Zero products in the matrix are actually floating multiplications
in several positions; the displayed zeros describe ordinary finite inputs, not NaN propagation or signed-zero
canonicalization. Evidence: `0x0073A1C1–0x0073A34A`, matrix stores `0x0073AB99–0x0073ABF8`.

For nonzero turns the owner rotates world X/Z about `(cx,cz)` before the above transform. With
c = F(cos(E(turns × binary32(2π)))) and s = F(sin(E(turns × binary32(2π)))), its real-number equations are
`x′=c*(x−cx)−s*(z−cz)+cx`, `z′=s*(x−cx)+c*(z−cz)+cz`. The matrix also transforms its third output coordinate;
it is not a two-dimensional texture-only rotation. Both signed zeros take the no-rotation branch; unordered
angle comparison takes the rotation branch. Ordered rotated intermediates are specified in §3.2.1.
Evidence: `0x0073A34E–0x0073A63A`.

Repeat flag bit 1 is set unless **both bit patterns** repeatU and repeatV are `0x3F800000`.
The unrotated footprint uses hx/hz, regardless of repeat factors or offset. For a rotated placement with
both repeat factors exactly one, replace hx by F((abs(c) + a*abs(s)) × hx) and hz by
F((abs(c) + F(1/a)*abs(s)) × hz), with sums/products kept in x87 precision until those stores.
For any other repeat factors this footprint expansion is skipped. Four cell bounds are then stored to binary32:
`[(cx−hx)*cellScale,(cz−hz)*cellScale,(cx+hx)*cellScale,(cz+hz)*cellScale]`.
The owner converts minima to floor and maxima to ceil for normal finite city coordinates under round-to-nearest,
then intersects with the city's cell rectangle. No 0.07 margin is applied here. Exact conversion and cell-index
ordering are tracked under OV8. Evidence: `0x0073A5F5–0x0073A675`, `0x0073ABFB–0x0073AC6F`,
conversion helper `0x00735380–0x00735475`.

Slot 12 ignores -1 and nonnegative direct overlay IDs. It repositions only when either absolute coordinate
difference is **at least** binary32 `0x38D1B717` (0.00009999999747378752), or is unordered. Otherwise neither
coordinate is written. Slot 13 copies all nine values and re-places even for identical values. Slot 14 ignores
-1/direct IDs and leaves output untouched; for an indirect ID it copies all nine words. There are no null-pointer,
range, free-record or generation checks on these paths; callers must supply live records and valid pointers.
Evidence: `0x007352D0–0x0073531A`, `0x0073AD80–0x0073AE52`, `0x00736490–0x007364EE`.

#### 3.2.1 Ordered rotated coefficients

The following completes the coefficient order for finite nonzero size/aspect; it does not substitute a generic
matrix multiplication (which can round differently). All operations between explicit F stores use E, with
left-to-right sums as parenthesized. Start with rw=E(1/w), R=E(E(1/a)*rw), q=F(0*rw),
du=F((hx−cx)*rw), dv=F((hz−cz)*R), rv=F(R). The unrotated coefficients before rotation are
A=F(repeatU*rw), B=F(repeatU*q), C=F(repeatU*q), P=F(repeatV*F(0*R)),
Q=F(repeatV*E(0*R)), S=F(repeatV*rv). Keep T=E(F(repeatU*du)+offset) in x87 precision;
V=F(F(repeatV*dv)+offset) is stored. Unrotated rows are `[A,P,0,0]`, `[B,Q,0,0]`, `[C,S,1,0]`,
`[F(T),V,0,1]` (the middle rows' last two zeros are literal integer zero stores).

When turns is ordered equal to zero, use those rows directly. Otherwise take c/s as in §3.2, n=F(−sin(angle)),
DX=F(cx*(1−c)+cz*s), DZ=E(cz*(1−c)−cx*s), b0=F(B*0), q0=F(Q*0), t0=F(T*0),
v0=F(V*0), c0=F(c*0), n0=E(n*0). The double literal 1 is used in `1−c`. Then store:

| Row | Four binary32 entries, each rounded at its enclosing F |
|---|---|
| 0 | F(((A*c+C*s)+t0)+b0), F(((P*c+S*s)+v0)+q0), F(c0+s), F(s*0+c0) |
| 1 | F(((A+C)*0+t0)+B), F(((P+S)*0+v0)+Q), +0, +0 |
| 2 | F(((C*c+A*n)+t0)+b0), F(((S*c+P*n)+v0)+q0), F(c+n0), F(c0+n0) |
| 3 | F(((C*DZ+A*DX)+b0)+T), F(((S*DZ+P*DX)+q0)+V), F(DX*0+DZ), F((DZ*0+DX*0)+1) |

The two translated texture entries are stored before the other rotated coefficients; DX is rounded but DZ
remains in x87 precision until those uses. Source: `0x0073A1E2–0x0073A34A`, rotation and stores
`0x0073A364–0x0073A5F1`. No additional angle reduction, trigonometric snapping, FMA, or matrix normalization
is performed by this owner. The trigonometric operation itself follows native x87 behavior and caller FP state;
exceptional-angle/size outcomes are not replaced with a fabricated finite rectangle.

#### 3.2.2 Ordinary decal flags and setter preservation (OV15)

A successful ordinary **binding add (slot 4)** creates a shown, snapshot-eligible decal with the complete
flags word **`0x00000091`** (bits 0, 4 and 7). Bit 7 is established by the add itself, before allocation and
placement, not by a later setter or draw. The default record has flags 1 (`0x00735F44`,
`0x007360EE–0x007360F5`); slot 4 adds both decal and snapshot eligibility with the constant `0x90`
(`0x0073C009–0x0073C023`). This is unconditional for a nonnull binding on the successful ordinary path:
binding identity, dimensions/aspect, land/water selection, static/dynamic mode, shadow quality and snapshot
render state do not select bit 7. Null binding returns −1 before record creation (`0x0073BF18–0x0073BF30`).
The source add (slot 3) has the same flag rule: its wrapper calls AddOverlay, which also adds `0x90`
(`0x0073C0C3–0x0073C0C6`, `0x0073B0C7–0x0073B0E1`). Key adds (slots 5/6) inherit slot 4's rule when
their resolved binding is nonnull (`0x00736189`, `0x007361E9`). This rule concerns ordinary adds; the distinct
city/model-shadow forms retain their separately specified flags. Bit 7 is eligibility, not proof of an actual
snapshot submission: the dynamic-manager snapshot skip still applies (§4.4).

For the live6 suffix, the complete flags word after **each completed call** is:

| Call, in order | Flags word |
|---|---|
| Slot 4: binding add at `(128,128)`, width 32, turns 0 | `0x00000091` |
| Slot 12: move the returned indirect handle to `(144,160)` | `0x00000091` |
| Slot 17: alpha 0.5 | `0x00000091` |
| Slot 18: RGB `(0.25,0.5,0.75)` | `0x00000091` |
| Slot 19: draw-state byte 3 | `0x00000091` |
| Slot 20: self-lighting argument 1 | `0x000000D1` |

Placement changes only repeat bit 1 in this flags word; the ordinary default repeat factors are both exactly
one, so that bit stays clear (`0x0073ABFB–0x0073AC24`). Slot 12 preserves those factors (§3.2).
Alpha/color/draw-state write their own fields, not flags (`0x00737FDF`, `0x00737FF6`,
`0x0073808D–0x0073809E`, `0x0073813A`). Their static redisplay helper only reads the rectangle and
invalidates terrain (`0x00737D40–0x00737DA3`). Slot 20 ORs `0x40` and preserves every other flag
(`0x00736558–0x0073656F`); a following argument 0 does not clear self-lighting. Thus the native `0xD1`
versus replay `0x51` observation is a missing **creation-time bit 7**, not an alpha/RGB/draw-state or
self-lighting side effect. Slot 19's byte 3 is a separate record field and must not be folded into flags.

### 3.3 Alpha acceptance (OV6)

Slot 17's argument and stored alpha are binary32; the threshold at `0x00AB39A0` is **binary64**
`0x3FA999999999999A` (the double representation of 0.05). A static manager first rejects an ordered equal
old/new value. Otherwise it accepts new alpha exactly 0 or 1, or an ordered absolute x87 subtraction result
**strictly greater** than that double threshold. A rejected sub-step change does not update stored alpha;
the next call compares against the last accepted value. An accepted write stores the supplied bits and calls
`MarkChanged`. There is no clamp: negative values, values above one and infinities can be accepted. New NaN is
rejected in the static path (including when old is NaN); with old NaN only an endpoint new value is accepted.
A dynamic manager stores every input, including equal values and NaN, without calling `MarkChanged`.
Groups recurse through all eight parts in order; -1 is ignored. Evidence: `0x00737F00–0x00738000`, in particular
the double comparison at `0x00737FCB` and rejection at `0x00737FD6`.

First exact paper vectors (not an executed native/game verdict):

| Input/state | Required result |
|---|---|
| Placement `[32,48,16,0,1,1,1,0,99]`, retained aspect 2, cellScale 1/16, city containing it | a=2, hx=8, hz=16; matrix rows `[1/16,0,0,0]`, `[0,0,0,0]`, `[0,1/32,1,0]`, `[-1.5,-1,0,1]`; cell rectangle `(1,2,3,4)`; repeat clear; retained 99 has no placement effect |
| Same with repeatU=2 and offset=1/4 | A=1/8, Tu=−2.75, Tv=−0.75; footprint unchanged; repeat set |
| Slot 13 receives the same nine words again | copy and placement still occur |
| Slot 12: old `(0,0)`, new `(2^−14,0)` | no write, because 2^−14 is below the coordinate threshold |
| Static alpha old 0; write binary32 `0x3D4CCCCC` | rejected, stored 0 |
| Static alpha old 0; write binary32 `0x3D4CCCCD` | accepted: 0.05000000074505806 is greater than double 0.05 |
| Static alpha old 0; write 0.03125 then 0.0625 | first rejected; second accepted relative to stored 0 |
| Static alpha old 1; write 1 | no write/redraw |
| Dynamic alpha old 1; write a quiet NaN | stores that NaN, no redraw call |

### 3.4 Slot 21: exact replacement matrix (OV7, proven subset)

For a single overlay, slot 21 replaces all sixteen matrix entries; it does not compose a scale with placement.
Let r = E(1/size), t = F(E(r × offset) + 1). The consecutive binary32 rows are:

| Row | Four entries |
|---|---|
| 0 | F(r), +0, +0, +0 |
| 1 | +0, F(−r), +0, +0 |
| 2 | F(r), +0, 1, +0 |
| 3 | +0, t, +0, 1 |

Thus the first coordinate is `(x+z)/size` and the second is `−y/size+1+offset/size` in real-number terms,
not a simple world X/Z rectangle mapping. The reciprocal remains in x87 precision until each store; t uses
that reciprocal, not a rounded reciprocal. The owner ORs flag bit 5 and marks the rectangle on static managers,
even for equal input. There is no size-zero guard or clamp: with masked FP exceptions, size +0 gives +infinity,
−infinity, +infinity at indices 0,5,8, and a NaN at index 13 when offset is zero. Negative zero reverses the
infinities. Unmasked FP exceptions follow caller FP state. The method ignores −1, resolves an indirect handle,
but **does not recurse over groups**; a handle resolving to a group is an unsupported unsafe native call.
Evidence: `0x00738160–0x00738220`, especially stores `0x007381D7–0x00738207`.

Paper vector: size=16, offset=8 gives rows `[1/16,0,0,0]`, `[0,−1/16,0,0]`, `[1/16,0,1,0]`,
`[0,1.5,0,1]`; input `[16,8,16,1]` produces `[2,1,16,1]`.

### 3.5 Keyed shadows: slots 9/10 (OV1, proven subset)

Both calls take five stack words. Slot 9's first argument is a **resource-key address**, not a texture binding.
It returns −1 without resolving that key when shadow quality is zero; otherwise factory slot 6 receives the
key and an initially null binding output. A null result returns −1. On success, factory slot 8 supplies the
dimensions and the owner retains binary32 `height/width` (unsigned dimension conversion), constructs
`[cx,cz,width,turns,1,1,1,0,0]`, allocates one overlay with shadow bit 3, and places it with redisplay disabled.
The last argument is tested as a **byte**, so `0x100` is false and `0x101` is true. Evidence:
`0x0073B3B9–0x0073B49A`, `0x0073B4CD–0x0073B520`.

Let M be the primary row-vector matrix produced by placement. For true flipV, replace column 1, in each
row r, with `F(M[r,3] + F(−M[r,1]))`; other columns stay unchanged. Thus the homogeneous texture output
becomes `(u,w−v,z,w)`. This changes the matrix, not the nine public placement words or the cell rectangle.
Evidence: `0x0073B526–0x0073B5A8`.

The owner next derives a separate sixteen-float secondary matrix N from this possibly flipped M. For ordinary
finite inputs its real-number action is `(u,v,z,w) → (u/2+w/4,v/2+w/4,z,w)`. It must not replace M with N.
The native constants are binary32 +0 at `0x00A81054`, 1/4 at `0x00A8821C` and 1/2 at `0x00A84D2C`.
For each source row `(a,b,c,d)`, the native store boundaries are:

| Intermediate/output | Evaluation (E and F as §3.2; additions evaluated left to right) |
|---|---|
| C, Q, B, A | `C=F(E(c×0))`; `Q=F(E(d×1/4)+C)`; `B=F(E(b×0))`; `A=F(E(a×0))` |
| N column 0 | `F(E(a×1/2)+B+Q)` |
| N column 1 | `F(E(b×1/2)+A+Q)` |
| N column 2, rows 1–3 | `F(E(d×0)+c+A+B)` |
| N column 2, row 0 | `F(E(d×0)+A+c+B)` |
| N column 3 | `F(E(A+B)+C+d)` |

Products/additions without an F remain x87 intermediates; zero products really occur, including for
nonfinite inputs. Rows are computed 3,2,1,0 and then stored to the secondary matrix. Evidence:
`0x0073B5AA–0x0073B83A`. The owner then performs its static index/redisplay operation (`0x00738A10`),
allocates an indirect handle retaining the placement/aspect, and releases temporary references. The matrices
are owner-derived; callers do not supply a projected rectangle or a completed transform.

Slot 10 obtains one word from factory slot 11 and constructs a temporary three-word key, in address order:
`[0x7AB50E44, groupWord, factorySlot11Result]`. It forwards its other four arguments unchanged to slot 9.
This is a key construction, not passing the slot-11 result as a binding. Evidence: `0x00735497–0x007354D3`.

Paper discriminator: with square texture, centre `(0,0)`, width 16, turns 0, flipV false, the centre maps to
primary `(1/2,1/2,0,1)` and secondary `(1/2,1/2,0,1)`; at world `(0,0,8)` primary v=1 and secondary v=3/4.
With flipV true the latter primary v=0 and secondary v=1/4. The placement getter is identical in both cases.
Model-shadow slot 11 uses the distinct owner construction in §3.7; these rules must not be substituted for it.

### 3.6 Rectangle helper and city-wide add (OV2)

`AddOverlayForRect` (`0x0073B8A0`) takes `(binding, cellRectangle, matrix16, extraFlags)` and returns a
**direct overlay index**, or −1 for a null binding. It constructs the default overlay (§1.1), retains the
binding, copies all four supplied rectangle words and all sixteen primary-matrix words, ORs extraFlags into
the default shown flag, and allocates the record. It does not clip, normalize, derive a placement record,
allocate an indirect handle, or initialize a shadow-specific secondary transform. For a non−1 allocation
on a static manager it inserts the rectangle into the cell index, then calls MarkChanged; a dynamic manager
skips both. Local source/binding references are released after allocation; the retained record owns its
references. Evidence: `0x0073B8AE–0x0073B8F1`, copies `0x0073B8F1–0x0073B9A7`, post-add/cleanup
`0x0073B9AC–0x0073BA04`.

Slot 8 resolves its one key through **factory slot 5**, calls that helper with city rectangle `(0,0,nx,nz)`
and extraFlags `0x12`, then releases the temporary binding. Thus final flags are `0x13` (shown, bit 1,
decal bit 4); it returns a direct ID and has no placement getter record. A missing binding yields −1 through
the helper. Its primary matrix is unusual and must be preserved rather than replaced with a conventional
city-sized X/Z texture mapping. In the row/column convention of §3.2 its rows are
`[0,0,0,0]`, `[0,F(E(2/nx)),0,0]`, `[0,0,1,0]`, `[0,0,0,1]`.
The numerator is binary32 2 at `0x00A8825C`; nx is converted from a signed integer and the quotient is
stored once as binary32. Neither nz nor the world cell width enters that matrix. No nx=0 guard exists.
Evidence: `0x0073C9A6–0x0073CA6E` (matrix base and sole quotient store `0x0073CA6A`), helper call
`0x0073CA75–0x0073CA83`, return/temporary release `0x0073CA88–0x0073CA9E`.

Paper discriminator: nx=64, nz=32 gives rectangle `(0,0,64,32)` and sole nonzero texture coefficient
M[1,1]=1/32. A static add with binding present returns a direct ID with flags `0x13`; slot 14 does not
write an output placement for that ID. Changing nz alone leaves this matrix unchanged.

### 3.7 Model-shadow add and its owned records (OV1/OV2)

Slot 11 (`0x0073C0D0–0x0073C838`, callee pops 24 bytes) has **six** arguments:

| Argument | Owner-observed role |
|---|---|
| 1 | texture binding passed to the rectangle helper; this is not a resource key or texture source |
| 2 | signed point count n; later controls the projected-pair bounding scan |
| 3 | source geometry pointer forwarded to lighting slots 17/18; the overlay owner does not decode its elements |
| 4 | pointer to n consecutive binary32 pairs `(u,v)` used after projection succeeds; forwarded to lighting slot 17 |
| 5 | world box pointer, six binary32 words `(xmin,ymin,zmin,xmax,ymax,zmax)` in the fallback bounds calculation; also forwarded to lighting slot 18 |
| 6 | projection descriptor forwarded to lighting slot 17; its internal layout is outside this owner |

If cached shadow quality is exactly zero, return −1 before accessing these pointers or requesting lighting.
Otherwise obtain lighting through the global service's slot 9 and invoke lighting slot 17
`CalcShadowProjection(n, sourceGeometry, pairs, descriptor, outP)`. False in AL returns −1. For the bounds
choice, read **current global** quality (render-properties integers `+0x1FC`), not the cached field again.
Signed quality ≥5 invokes lighting slot 18 `CalcShadowTerrainRect(n, sourceGeometry, worldBox, outRect)`;
false returns −1. Signed quality <5 requests shadow direction through lighting slot 13 and uses the local
fallback below. These service calls precede the null-binding rejection. Evidence: `0x0073C0DC–0x0073C1D6`.
The owner does not infer projected geometry from an ordinary placement record. The production of P/pairs
and the high-quality rectangle belongs to those named lighting bodies, not to a caller-supplied completed
overlay record; no substituted TR33–TR37 terrain contract is implied here.

The fallback `ComputeCellRect` (`0x007354E0–0x007356C3`) accepts
`(direction3, n, sourceGeometry, worldBox6, outRect)`. It does not read n or sourceGeometry. For ordinary
finite inputs in the native conversion range, first compute
`Lx=floor(E((xmin+e)×cellScale))`, `Rx=ceil(E((xmax−e)×cellScale))`, and analogously Lz/Rz,
where e is **binary32** `0x3D8F5C29` (0.07000000029802322, `0x00AA27E8`), with the sums retained in x87.
Compute `h=ceil(E((ymax−ymin)×1/16))`; 1/16 is binary32 at `0x00A8E174`, independent of cellScale.
If direction X is ordered >0, add h to Rx; otherwise subtract h from Lx. Likewise direction Z >0 adds h to
Rz, otherwise subtracts h from Lz. Zero and unordered direction take the subtract-minimum branch. Intersect
the resulting rectangle with the city bounds using §2.2's helper; copy all four words to output.
There is no nonnegative-height clamp. Conversion uses the floor/ceil bias-and-correction family of §3.1,
not saturation of arbitrary nonfinite/huge boxes. Evidence: epsilon/scale/conversion
`0x007354E7–0x007355EC`, height `0x007355EC–0x0073562E`, signs/clipping `0x00735632–0x007356C1`.

The add then calls §3.6's rectangle helper with `(binding, outRect, P, 8)`; missing binding or returned −1
returns −1. Success is a **direct** ID with flags 9, default alpha/colour/draw-state and primary matrix P.
There is no placement handle, retained aspect or group. Static insertion/redisplay happens inside that
helper, **before** the following secondary matrix is written; no second redisplay is requested afterward.
Evidence: `0x0073C1D6–0x0073C208`, secondary stores `0x0073C7B1–0x0073C82B`.

For the secondary matrix, scan pairs in increasing index order starting from pair 0, maintaining minimum u/v
and maximum u/v. Minimum/maximum retain the old operand for equality or unordered comparison; update only
for an ordered strict improvement. Four-pair unrolling preserves encounter order. The first pair is read
even when n≤0; no empty-polygon success rule or zero-extent guard exists. Write `du=F(E(umax−umin))`,
`dv2=F(E(E(vmax−vmin)×2))`. Set `su=F(E(1/(E(du×2))))`, `sv=F(E(1/dv2))`,
`tu=F(E(1/4−E(1/(E(du×2)))×umin))`, `tv=F(E(1/4−E(1/dv2)×vmin))`.
The 1/4 literals are **binary64** (`0x00AA74E8`). The translation products use the unrounded reciprocal
still in x87, not the stored su/sv. Evidence: pair bounds `0x0073C208–0x0073C4F9`, coefficients
`0x0073C4FF–0x0073C553`.

For each row `(a,b,c,d)` of P define `z=F(E(c×0))`, `y=F(E(b×0))`, `x=F(E(a×0))`.
The secondary row is:

| Column | Ordered evaluation, outer F is its binary32 store |
|---|---|
| 0 | `F(((a×su+d×tu)+y)+z)` |
| 1 | `F(((d×tv+b×sv)+x)+z)` |
| 2 | `F(((d×0+c)+x)+y)` |
| 3 | `F(((d+x)+y)+z)` |

All unmarked operations retain x87 precision. Rows are evaluated 3,2,1,0, with column 0/1 intermediates
retained until their eventual stores; zero products are real operations, including for nonfinite values.
Evidence: row 3 `0x0073C555–0x0073C5FE`, row 2 `0x0073C605–0x0073C690`, row 1
`0x0073C697–0x0073C71F`, row 0/final stores `0x0073C723–0x0073C82B`.

Independent paper vectors, conditional on the named lighting output (not an invented lighting algorithm):

- Fallback box `(16,0,32,48,32,64)`, cellScale=1/16, direction `(1,0,−1)` and a large enough city gives
  `(1,0,5,4)`: unextended bounds `(1,2,3,4)`, h=2. Direction X=0 instead gives xmin=−1 before clipping.
- If successful lighting supplies P=identity and pair extrema `(0,0)`–`(2,4)`, secondary rows are
  `[1/4,0,0,0]`, `[0,1/8,0,0]`, `[0,0,1,0]`, `[1/4,1/4,0,1]`.
  Minimum and maximum pair coordinates therefore map to 1/4 and 3/4, rather than 0 and 1.
- Cached quality 0 suppresses all lighting work even when current global quality is 6. Cached quality 2
  with current global quality 6 selects lighting slot 18, demonstrating the two distinct quality reads.

### 3.8 Independent replay versus observation of existing records (OV12)

There is **no complete snapshot-to-creation-input inverse** in this owner. Sections 2.4–2.7 make the extant
records observable, but those records are results, not an event log. Public vtable slots (§3) provide no
enumeration, source-image readback, original resource-key getter or model-shadow input getter. In particular,
model shadows are direct overlays with no placement record; their geometry pointer, point count, pairs,
world box and projection descriptor are not retained. Their primary/secondary matrices and clipped rectangle
do not uniquely encode those inputs. Keyed shadows likewise retain no flipV byte or indication of which
creation method was used. Slot 21 replaces M without changing placement, and slot 13 can subsequently
re-place the same overlay. Thus placement plus current flags/aspect is insufficient to explain every current
matrix. Evidence: model creation/service inputs `0x0073C14E–0x0073C208`, final stores
`0x0073C7A2–0x0073C838`; key/flip creation §3.5; replacement `0x00738160–0x00738220`; complete fields §2.5.

For a shadow which recomputes owner results independently, the sufficient **input boundary** is the original
ordered calls and the services/resources they consume, starting before the first overlay allocation:

| Creation family | Inputs to retain/reproduce independently of computed records |
|---|---|
| Source/binding/key decal (§3, §3.2) | Creation slot and arguments: source or binding identity, or key/ID and factory group; centre, width and turns; resource dimensions returned then; manager cell scale and city dimensions |
| Round decal (§3.1) | Its binding/key and original placement, dimensions/aspect at creation; eight parts are owner outputs, not eight caller rectangles |
| City-wide keyed decal (§3.6) | Key/ID and factory group, resource lookup result, nx/nz and static/dynamic mode; direct ID is the output |
| Keyed shadow (§3.5) | Original resource key, centre, width, turns and flipV low byte; cached quality and dimensions at creation; later placement/matrix mutations in order |
| Model shadow (§3.7) | All six original arguments, including pointed-to data valid at the call, cached and current global quality, lighting/terrain state consumed by slots 13/17/18, resource binding identity, cell scale and city dimensions |
| Rectangle helper (§3.6, internal) | A genuinely caller-supplied binding/rectangle/matrix/flags is an input to this helper only; its use by model-shadow creation does not turn that creation's computed P/rectangle into independent inputs |

Include failed calls and null-resource outcomes; they determine whether any indices were allocated. Retain
Init/shutdown, removals and subsequent setter calls in actual order, including repeated/equal calls where
native tolerance or redisplay rules matter. FIFO allocation must run from the same empty arena state to
compare IDs; grouping and direct versus indirect IDs follow the original creation family. Track resource
identity with observer-owned tokens and map it consistently, rather than passing native addresses into the
independent owner. Native FP state and the stored binary32 inputs also matter for exact matrix/rectangle
comparisons (§§3.1–3.7). A dump of live indices cannot reconstruct freed/failed operations or the allocation
history that produced them.

This describes the information required by a future observation adapter, **not a new native logging API**.
If an adapter was attached only after city load, it may compare observed record/getter invariants and graph
relationships, but cannot claim an exact independent full-owner replay by seeding native M/N/rectangles,
cell lists or resource hashes as answers. Slot 21's original `(size,offset)` is a valid input when actually
captured; copying its resulting matrix back is not a test of that slot's calculation. For model shadows,
feeding native lighting outputs P/rectangle into a test can test the downstream overlay arithmetic only,
with that boundary declared. It does not verify lighting projection/terrain rectangle production (OV1).

Even a complete current rectangle set loses **cell-list history**: a newly inserted overlay is prepended;
changed-rectangle placement removes and reinserts it, whereas same-rectangle placement preserves the list
and only marks it. A snapshot can supply expected ordered lists to compare against independently replayed
operations, but sorting/deduplicating them or reconstructing solely from bounds is a different contract.
Similarly, comparing texture identities/dimensions does not verify image bytes (§2.7). A full shadow must
report any missing event/resource/lighting input as incomplete coverage; the absence of those inputs is the
answer to OV12's snapshot-reconstruction question, not permission to inject computed owner results.

#### 3.8.1 Capture availability and the earliest boundary (OV13/OV18)

There is **no supported native capture-registration or history-export API on this owner**: the §3
interface exposes overlay operations/getters, not an event subscription for their arguments or consumed
resources. The concrete earliest boundary for a future instrumented adapter is terrain-view initialization,
before the four manager Init calls at `0x0075C6BC`, `0x0075C6C8`, `0x0075C6D7`, `0x0075C6E7`:
static land, static water, dynamic land, dynamic water respectively. These are direct calls, following direct
construction of the managers (`0x0075C5B2–0x0075C6B3`); replacing only interface slots does not capture Init.
The complete pre-allocation routing and matching teardown boundary are specified below (OV18).
Capture must precede Init itself, not merely the first successful add, because Init acquires resources and
settings (§1.1). This is an instrumentation boundary, not a callback the game promises to invoke. A
post-load read-only observer cannot supply it retrospectively. Its supported result remains the §2
record/getter/graph observation, with creation replay and image bytes explicitly unavailable. Closing OV13
does not confer full-owner verification credit on such an observer.

For an adapter that actually records calls from that boundary, the following is the **input contract**;
the native owner does not retain a copy for it. Keep one ordered stream per manager plus the order of shared
service/resource changes. Record entry arguments while pointees are valid, and separately record exit
results as expectations. Nested internal calls are evidence of the enclosing operation, not additional
public adds to replay (e.g. model-shadow creation's rectangle helper). Snapshotting a pointer address and
reading it after return is not equivalent to copying its original data.

| Input family | Data required at consumption time |
|---|---|
| Lifetime and mutation | Manager identity; original Init parent identity and dynamic/water bytes; all original creation arguments from §§3.1–3.7; removals, placement/scalar/colour/visibility/self-lighting/texture-scale setters, Shutdown and repeat Init, including failures, equal-value calls and sentinel calls in order. Capture quality changes/messages and the settings actually consulted. Do not infer missing calls from live IDs. |
| Model geometry and UVs | Slot 11's six arguments (§3.7). For ordinary valid positive n, copy n contiguous three-binary32 points (12-byte stride) and n contiguous two-binary32 UV pairs (8-byte stride), not pointers to model objects or interleaved render vertices. UVs are original projection inputs as well as later extrema inputs, not generated owner matrices. |
| Model box and transform descriptor | Copy the six binary32 box words in §3.7. The descriptor is lighting's transform: flag byte at +1, nine binary32 matrix entries at +4 through +36, translation at +40/+44/+48, scale at +52. Bytes +0/+2/+3 are not matrix entries. Preserve all flag bits; use lighting §5.8 for their meaning and precision. The separate 64-byte outP is an output, never an input descriptor. |
| Lighting and terrain | Reproduce lighting's original state and terrain inputs under lighting §§5.7–5.8: view-rotated direction, terrain cell counts/width and height samples used by the edge probes. Retain both cached manager quality and current global quality; the latter selects the bounds route. Capturing only returned direction/P/rectangle establishes a downstream service-output test boundary, not independent verification of those services. |
| Resources and sources | Original source/binding identity as stable observer tokens; original key/ID and factory group/settings/cache history; dimensions and lookup success/failure at each consumption. Source slot 3's two dimension words and arbitrary source implementations need their own capture boundary (§2.7); there is no universal source-object pixel layout. A newly loaded copy of a keyed file alone does not prove the resident resource consumed then. |
| Image uploads | Independently retain source resource/image-info contents, format/type, source frame/level counts and each consumed level's bytes while valid; retain binding size/levels and all later upload/restore events. Factory slot 7 selects the last binding-level-count entries of each source frame, and forwards format/type and level pointers to driver slot 61 (texture-bindings §§1.5,1.12; graphics-formats §§2.6,2.10). Record frame/level, dimensions and original bytes at that boundary; driver handles and resource hashes are not bytes. Unknown formats/invalid pointers without an established readable extent make byte coverage unavailable, not an invented zero-length image. |
| Numeric environment | Original binary32 input words and inherited x87 control state at relevant arithmetic, as §§3.1–3.7 and lighting §5.8 require. Do not round-trip inputs through printed decimals or substitute computed native M/N/rectangles. |

The model pointee layouts are proved at `0x007D8C87–0x007D8D05`, array accesses
`0x007D8F26–0x007D90F3`; original owner forwarding/quality selection is
`0x0073C14E–0x0073C1D6`. Transform widths were checked against the instructions, not inferred from
decompiler types. Array sizes above describe the valid caller domain, not new native validation: cached
quality zero rejects before pointee access, and n≤0 does not become a safe empty-model operation (§3.7).
Upload pointers are transient (`0x00809B90–0x00809CC6`); neither the owner nor binding metadata supplies a
complete recording facility. Missing any of these input families limits the claimed coverage to the
boundary actually captured; it does not block implementing the observation invariants now.

##### Pre-allocation construction and teardown boundary (OV18)

In terrain-view Init (`0x0075C420`), the complete manager creation/Init unit is the half-open instruction
interval **[`0x0075C5B2`, `0x0075C6EC`)**. Entry is before the first allocation's size argument; continuation
is after the fourth Init returns and before height updates. The construction-only prefix ends at
`0x0075C6B6`. There is no test of the four existing manager fields before allocation: pre-populating them
causes replacement/reference release, not a construction bypass. The optional ripple path rejoins at
`0x0075C5B2` (`0x0075C582`, `0x0075C5AD`). These are semantic boundaries for replacing the entire unit;
they do not specify a forwarding gateway or authorize executing native constructors on foreign storage.

All four allocations/constructions and field-reference transfers finish **before any** manager Init:

| View field / role | Size argument begins | Allocation call | Constructor call | New AddRef / field store / old Release | Init call and arguments after this |
|---|---|---|---|---|---|
| `+0x78`, static land | `0x0075C5B2` | `0x0075C5B7` | `0x0075C5C5` | `0x0075C5DF` / `0x0075C5E4` / `0x0075C5ED` | `0x0075C6BC`: view, 0, 0 |
| `+0x7C`, static water | `0x0075C5F0` | `0x0075C5F5` | `0x0075C603` | `0x0075C61D` / `0x0075C622` / `0x0075C62B` | `0x0075C6C8`: view, 0, 1 |
| `+0x80`, dynamic land | `0x0075C62E` | `0x0075C633` | `0x0075C641` | `0x0075C65E` / `0x0075C663` / `0x0075C66F` | `0x0075C6D7`: view, 1, 0 |
| `+0x84`, dynamic water | `0x0075C672` | `0x0075C677` | `0x0075C685` | `0x0075C6A2` / `0x0075C6A7` / `0x0075C6B3` | `0x0075C6E7`: view, 1, 1 |

Call/continuation contracts, established from the instruction bytes:

- Allocation is `GlobalHeapAlloc`, `0x005E55E0`: one 32-bit size argument **`0xFC`**, caller cleanup 4 bytes,
  storage pointer in EAX. No incoming ECX object is consumed by this allocator. Null skips that constructor
  and produces a null new pointer; it is not a clean Init failure. Each constructor is `0x0073BC20`,
  ECX=allocated storage, no explicit arguments or argument cleanup, EAX=constructed manager base
  (`0x0073BD98–0x0073BD9B`). Its result becomes EDI and is used for the field-reference transfer.
- A field transfer compares new and old pointers. If different: AddRef the nonnull new object through
  primary-interface slot 1, publish the new pointer, then Release the nonnull old object through slot 2.
  If equal, none of those operations occurs. Their scalar returns are ignored. The constructor's embedded
  reference count starts at **zero**, so the ordinary new object's field AddRef establishes count 1;
  there is no extra constructor/factory reference for the caller to release. Evidence: embedded-base
  construction at `0x0073BC24–0x0073BC36`, zero store `0x0090DA20`, AddRef `0x004302A0`/`0x00968B60`.
- Each Init is thiscall: ECX=the manager read from that view field; three 32-bit argument slots in order
  **(complete view base, dynamic byte, water byte)**. The latter slots contain 0 or 1; Init reads their low
  bytes. At callee entry these are `[ESP+4]`, `[ESP+8]`, `[ESP+12]`; Init removes **12 argument bytes**
  (`0x0073CD39`). No Init result is tested or consumed. All four calls are unconditional, including after
  a null allocation. Init's first new-parent retention takes one view AddRef per manager (§1.1,
  `0x0073CB6D–0x0073CB96`); it does not add another manager reference.
- At whole-unit entry ESI=complete view base and EBP=0. EBX holds the original terrain argument of view
  Init; EDI still holds the service pointer loaded at `0x0075C4F7`. Neither EBX nor EDI is a creation input.
  EAX/ECX/EDX depend on the preceding optional ripple path and supply no inputs to this unit. Preserve
  ESI, EBP, the inherited floating-point control state, and the caller's saved registers/local frame.
  With S denoting view Init's entry ESP (pointing to its return address), boundary ESP is **S−`0x28`**,
  and remains that value at continuation. No additional arguments belong to the whole unit itself;
  a bridge's own return address must not be mistaken for this native frame.
- The native unit uses EBX/EDI as temporaries: on exit EBX is the former `+0x84` pointer and EDI is the
  new `+0x84` pointer. The continuation has no live dependency on those values, EAX/ECX/EDX, condition
  flags or a scalar result; preserving entry EBX/EDI instead is sufficient. It does depend on ESI, zero
  EBP, the unchanged stack and the four initialized fields. Evidence: frame setup `0x0075C425–0x0075C487`,
  continuation `0x0075C6EC–0x0075C77A`, later temporary definitions `0x0075C810–0x0075C882`, and epilogue
  `0x0075C964–0x0075C9C1`. The whole view Init later returns boolean true; that is not a manager result.

The native allocation belongs to the view-held reference protocol, not to terrain-view slot 18 or to the
draw adapter. Final primary-interface Release reaches the embedded unknown at manager `+8`; its deleting
destructor route subtracts 8 (`0x0073BDC0`), runs the body (§1.2), then uses **GlobalHeapFree**
`0x005E5620` on the complete allocation when deletion bit 0 is set (`0x0073C983–0x0073C995`). Allocation
and free are a matched native heap pair. Replacing the whole unit may use a different allocation owner,
but that object's Release/destruction must stay with its own owner; neither the native constructor nor
native destructor/free is a foreign-object lifetime gateway.

Creation is not the only direct route that must change when these fields contain replacement objects:

| View teardown route | Calls / effects in order |
|---|---|
| Initialized view Shutdown (`0x007560F0`) | If the draw registry exists, unregister the four draw subobjects (nonnull manager base **+4**) at `0x00756126`, `0x0075613F`, `0x0075615B`, `0x00756177`, in field order. |
| Manager Shutdown calls | `0x0075621F`, `0x00756227`, `0x00756232`, `0x0075623D` directly call `0x0073AC90` on `+0x78`, `+0x7C`, `+0x80`, `+0x84` respectively; ECX=manager base, no arguments/cleanup, no consumed result. A single nonnull `+0x78` check gates this entire block, not four independent checks. |
| View-field release after those four calls | Clear each nonnull field **before** primary slot-2 Release at `0x0075624E`, `0x0075625D`, `0x00756272`, `0x00756287`. The same first-field guard skips these releases too when `+0x78` is null; continuation is `0x0075628A`. |
| View destructor (`0x00756980`) | If view initialized byte `+0x18` is nonzero, direct call `0x007569B0` invokes the view Shutdown above. Independently, the loop at `0x007569D0–0x007569E0` makes primary slot-2 Release at `0x007569DC` on nonnull fields in reverse order: `+0x84`, `+0x80`, `+0x7C`, `+0x78`. It does not directly call the manager destructor. |

The four direct manager Shutdown calls therefore require replacement routing even with valid replacement
vtables. Virtual field Releases and the destructor's residual-release loop must reach the replacement's
own implementation. Normal Shutdown clears the fields, making the destructor loop empty. Draw registration
is later, outside the creation unit: calls `0x0075C89F`, `0x0075C8D1`, `0x0075C909`, `0x0075C95E` pass
the corresponding base+4 draw subobject at the layers/priorities of §1. Registry-held references are
separate from the one field reference established here; they are not a constructor reference to transfer.

Worked lifecycle case (service callbacks otherwise inert): start with four null fields and successful
allocations A/B/C/D. Construct A, retain/publish A, then B, C, D in that order; their field-owned counts
are each 1. Only then Init A/B/C/D with (view,0,0), (view,0,1), (view,1,0), (view,1,1). Those parent
retentions add four view references. No allocation or Init scalar result decides whether to continue.
With no draw registry or other manager owners, routed view Shutdown must call all four manager Shutdowns
before releasing any field reference: parent retentions fall by four, then clear/release A/B/C/D and destroy
each with its matching owner. The view destructor afterward makes no further manager calls. For the
replacement variant, the same trace must contain **zero** native manager constructions, native manager
Shutdowns or native manager frees on A/B/C/D. A second discriminator is a pre-existing `+0x78`=P: native
creation still allocates A, AddRefs A, publishes A, Releases P, and later Initializes A.

## 4. Drawing (`Draw`, `0x00738A50`) *[compat]*

A dynamic manager is not drawn under `kRenderSnapshot` (boolean 28). Otherwise:

1. The model-reset helper is called and the resulting context matrix is inverted, so that a camera-space
   position can be taken back to world; composition order and the reset guard are specified in §4.7.
2. Selection: a static manager with byte `+0x20` nonzero collects through its cell index; otherwise scan all
   live overlay records in increasing index order (`0x00739245–0x007392DD`). The message in §5 refreshes that byte.
3. With `kRenderDecals` (boolean 10): the decals (§4.2). With `kRenderShadows` (boolean 9): the shadows, the rough
   form (`DrawShadowsRough`, `0x00737870`) when `kShadowQuality` < 3 or `kUseSecondStage` is off, else the full form
   (`DrawShadows`, `0x00736BF0`).
4. The texture transform is cleared and the depth offset restored.

### 4.1 Draw states (`InitDrawStates`, `0x0073BA10`)

Five packed states, all with colour write, depth test, blending, one texture and **culling off**, and stage-0
mode 1 except state 4 (which keeps the mode). The alpha reference is the packing input; packed replay uses
the quantized reference of draw-context.md §5, rather than forwarding that input unchanged:

| Index | Depth write | Depth function | Blend | Alpha test |
|---|---|---|---|---|
| 0 | no | less-or-equal | source alpha / one minus source alpha | no |
| 1 | no | less-or-equal | source alpha / one (additive) | no |
| 2 | no | less-or-equal | zero / source colour (multiply) | no |
| 3 | no | greater | source alpha / one minus source alpha | no |
| 4 | yes | less | source alpha / one minus source alpha | greater; reference input 0.5 |

#### Complete native packing arguments and combiner bytes (OV19)

The native vector at manager `+0x24` contains five **8-byte** records: value dword at record `+0`, write-mask
dword at `+4`, in little-endian order. Record i starts at vector begin +8i, not at an enum value or at a
combiner offset. `InitDrawStates` resizes this vector to 5 before packing (`0x0073BA15–0x0073BA1C`).

These are the complete **14 arguments**, excluding this, to `PackRenderState` (`0x007D2B50`), split across
two tables by ordinal. Each argument occupies a four-byte stack slot; the boolean arguments use its low
byte, the reference is binary32, and the packer removes 56 argument bytes (`0x007D2CFF`, `0x007D2D13`).

| State | 1 colour write | 2 depth write | 3 depth test | 4 alpha test | 5 blending | 6 texture count | 7 culling |
|---|---|---|---|---|---|---|---|
| 0 | 1 | 0 | 1 | 0 | 1 | 1 | 0 |
| 1 | 1 | 0 | 1 | 0 | 1 | 1 | 0 |
| 2 | 1 | 0 | 1 | 0 | 1 | 1 | 0 |
| 3 | 1 | 0 | 1 | 0 | 1 | 1 | 0 |
| 4 | 1 | 1 | 1 | 1 | 1 | 1 | 0 |

| State / native pack call | 8 depth function | 9 source blend | 10 destination blend | 11 alpha function | 12 alpha reference | 13 stage-0 mode | 14 stage-1 mode |
|---|---|---|---|---|---|---|---|
| 0 / `0x0073BA41` | 3 | 4 | 5 | −1 | −1.0 | 1 | −1 |
| 1 / `0x0073BA67` | 3 | 4 | 1 | −1 | −1.0 | 1 | −1 |
| 2 / `0x0073BA8C` | 3 | 0 | 2 | −1 | −1.0 | 1 | −1 |
| 3 / `0x0073BAB2` | 4 | 4 | 5 | −1 | −1.0 | 1 | −1 |
| 4 / `0x0073BADA` | 1 | 4 | 5 | 4 | 0.5 | −1 | −1 |

Integer −1 is the word `0xFFFFFFFF`; the exact reference argument words are `0xBF800000` for −1.0 and
`0x3F000000` for 0.5. Compare/blend integers use graphics-formats.md §3.8's native enums. A −1 argument
means **preserve the corresponding field**, not choose a default. States 0–3 leave alpha function,
alpha-reference nibble and stage-1 mode alone: their write masks are **`0x1E1FE3FF`**. State 4 leaves both
stage modes alone: its mask is **`0x11FFFFFF`**. All five masks include culling bit 6, which is explicitly
cleared. Texture count 1 explicitly enables stage 0 and disables stage 1 (bits 5/28), even when the modes
are preserved. These mask choices follow the packer's explicit field masks, not the current context state
(`0x007D2BBA–0x007D2BFC`, `0x007D2C64–0x007D2D13`).

The following are the three **complete 12-byte native combiner records after construction and
InitDrawStates**. Column labels are byte offsets within each record; all entries are hexadecimal bytes.

| Manager offset | 00 | 01 | 02 | 03 | 04 | 05 | 06 | 07 | 08 | 09 | 0A | 0B |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `+0x38` | 00 | 02 | 03 | 01 | 01 | 00 | 02 | 01 | 00 | 00 | 00 | 00 |
| `+0x44` | 00 | 01 | 03 | 01 | 01 | 00 | 01 | 01 | 00 | 00 | 00 | 00 |
| `+0x50` | 00 | 00 | 03 | 01 | 00 | 00 | 03 | 01 | 00 | 00 | 00 | 00 |

The constructor establishes every byte, including byte zeros at relative `+8/+9` and **word zeros at
`+0x0A`**: stores at `0x0073BC8D–0x0073BCF1`. InitDrawStates then overwrites only the selected bytes at
`0x0073BAE0–0x0073BB0C`; the other constructor bytes persist. Thus the final four zero bytes in each row
are proved storage contents, including any padding, rather than assumed uninitialized padding. Borrowing
a native descriptor means borrowing these 12 bytes, not reinterpreting a language enum or a packed-state
vector element. Their full-shadow normal/debug selection is §4.8; these records establish the adapter
inputs without claiming execution of the terrain geometry consumers.

Worked record/state cases: initialize the 36 combiner bytes to a nonzero sentinel before running the
specified constructor/Init stores; the final three records must equal the rows above, including all four
tail zeros. In a nonempty full normal pass with a shadow binding, stage 0 receives the `+0x38` row and
stage 1 the `+0x44` row; debug setting exactly 4 instead sends `+0x38` then `+0x50` to stage 0 (§4.8).
For state replay, begin with culling enabled, alpha function 6, reference nibble 3 and stage-1 mode 4.
State 0 disables culling and alpha testing, selects depth function 3 and blend 4/5, but preserves function
6, nibble 3 and mode 4 while disabling stage-1 texturing. Alternatively begin with stage modes 4/7 and
culling enabled: state 4 preserves modes 4/7, disables culling/stage-1 texturing, enables depth write and
alpha testing, and packs alpha function 4 with binary32 reference input 0.5. These distinguish keeping a
mode from keeping its enable bit, and keeping an alpha comparison from disabling alpha testing.

### 4.2 Decals (`DrawDecals`, `0x00736790`)

Depth offset 2. Each overlay in use whose flags have bits 0 and 4 (and bit 7 under `kRenderSnapshot`): the alpha as
the transparency, the colour as the shade (self-lit with bit 6) with vertex colours off for a self-lit decal, its
draw state, its texture (filters linear / linear-mipmap-linear), wrap clamp / clamp, or repeat / repeat with bit 1,
or repeat / clamp with bit 5, the texture transform, then its cell rectangle is drawn (`DrawRect`, `0x00735720`)
from the terrain view's land or water arrays (`0x007541C0`–`0x00755700`, by the water flag, the dynamic flag and
the vertex-buffer extension). Transparency, shade and vertex colours are restored.

*[compat]* Wrap precedence: the replacement-matrix bit 5 is tested **first**. When bit 5 is set the decal uses
repeat in the first texture axis and clamp in the second, even if the repeat bit 1 is also set; only with bit 5
clear does bit 1 select repeat/repeat, and with neither bit set the texture is clamp/clamp
(`0x007368BD`–`0x007368E4`). Slot 21 sets bit 5 without clearing bit 1 (§3.4), so a repeated decal that later gets
a replacement matrix switches from repeat/repeat to repeat/clamp.

### 4.3 Shadows (`DrawShadows`, `0x00736BF0`; `DrawShadowsRough`, `0x00737870`)

Both forms set depth offset 3 and obtain global shadow parameters through view-manager slot 9's lighting
result, slot 14. The exact alpha-test and exit actions are in §4.6. The full form has distinct normal/debug
stage routes, a conditional location-strength rejection and a water dispatch choice (§4.8); it is not simply
the rough form with a second texture. There is no proved direct terrain-height query here.

### 4.4 Counters and skipped draws (OV8, proven subset)

The two manager words are independent **last invoked nonempty pass** counters, not unconditional frame totals.
Slot 27 formats the decal word `+0xD8`, then shadow word `+0xDC`, as `%d decals, %d shadows`.
No reset occurs at entry to Draw. The following paths preserve both previous words:

- A dynamic manager under snapshot returns before camera or selection setup (`0x00738A61–0x00738A76`).
- An empty selected-ID vector skips both passes (`0x007392F1–0x00739301`).
- Disabling a pass preserves that pass's word (`0x00739309–0x00739340`).

Each pass independently returns on an empty input vector before its reset. On a nonempty vector, decals reset
`+0xD8` at `0x007367E4`; full shadows reset `+0xDC` at `0x00736D4A`; rough shadows reset it at
`0x00737983`. Each visits the supplied IDs in vector order. An eligible decal is an in-use record (first word
−1) with all mask bits `0x11`, or `0x91` in a static snapshot. An eligible shadow in **both** shadow passes is
an in-use record with all bits `0x09`; no snapshot bit-7 test is added there. Bit 4 is decal eligibility;
bit 3 is shadow eligibility. Neither test excludes the other bit, so a record carrying both can count in both.

The counter increments exactly once **after** calling the cell-draw helper for each submitted eligible record, with no
triangle count, draw-success, alpha, or nonempty-rectangle test at the increment. The full shadow pass selects
DrawRect or DrawShadowRect before their common increment. Evidence: decal `0x00736813–0x00736831`,
`0x00736B88–0x00736B9C`; full shadow `0x00736D7F–0x00736D95`, `0x00737814–0x0073782D`;
rough shadow `0x007379C1–0x007379D3`, `0x00737CDB–0x00737CEB`. This is a count of eligible submitted
records; eight submitted group parts count eight, even when a helper emits no geometry. In the full normal
shadow path, the location-strength check in §4.8 can reject a mask-eligible record before submission and
before increment. Consequently the mask alone is not a sufficient count oracle for that path.

Paper discriminators starting from counters `(3,2)`: empty selection leaves `(3,2)`; a nonempty selection
containing only one hidden in-use record with both passes enabled gives `(0,0)`; the same selection with
decals disabled gives `(3,0)`; a nonempty selection of one visible decal with alpha zero counts that decal.
These are expected native outcomes, not executed game verdicts. Remaining DrawRect/state/visibility and
shadow arithmetic details are tracked in OV8.

### 4.5 Visible-ID collection (OV8, proven subset)

Draw uses CollectVisibleOverlays only for a static manager with its index enabled; otherwise it clears the
selected-ID vector and appends all in-use overlay indices in ascending arena order, including hidden records
that a later pass may reject (`0x00739245–0x007392DD`). Collection is not a sorted set operation.

CollectVisibleOverlays (`0x00738390`) clears the output vector and obtains four outputs from terrain-view
GetDrawRows (`0x00752820`): row begin, row end, an array of per-row starts, and an array of per-row ends.
Let k be the cell-index shift, t=2^k−1. It visits signed coarse rows from `rowBegin >> k` up to, **excluding**,
`(rowEnd+t−1) >> k`. At each loop row y it directly reads the two arrays at index y and visits coarse columns
from `starts[y] >> k` up to, excluding, `(ends[y]+t−1) >> k`. The shifts are arithmetic. In the initialized
k=0 case this means rows `[rowBegin,rowEnd−1)` and columns `[starts[y],ends[y]−1)`: these are the cell ranges
derived from terrain vertex-row endpoints, not inclusive cell ends. No additional city clipping is performed
inside collection. Evidence: `0x007383C1–0x00738447`, loop ends `0x00738564–0x00738580`.

Rows increase, then columns increase; each cell list is followed from its head through next links. A record is
appended at its first encounter only, by comparing its 16-bit stamp at `+0x0A` with the manager stamp at `+0xE0`.
The manager stamp increments modulo 65536 **before** the row-range empty test. No zero avoidance or all-record
stamp reset occurs on wrap. A matching stale stamp can therefore suppress a record, including a never-collected
record with constructor stamp zero when the collection stamp wraps to zero. Collection does not check shown,
snapshot, decal/shadow or in-use bits before appending the list's ID; those filters belong to the passes.
Evidence: `0x007383FC–0x00738413`, `0x00738480–0x007384B7`, `0x0073854E–0x00738580`.

Paper discriminators: at k=0, vertex rows `[2,5)` visit cell rows 2 and 3, and per-row vertices `[3,7)` visit
columns 3,4,5. A first cell whose head-to-tail IDs are `[9,2]`, followed by a cell `[2,7]`, yields `[9,2,7]`,
not `[2,7,9]`. With collection stamp 65535, increment wraps to zero and a referenced record stamped zero is
omitted. There is no caller-supplied sorted visible-ID result in the public owner contract.

### 4.6 Rectangle submission and state changes (OV8, proven subset)

DrawRect receives a draw context and the stored four-int **cell** rectangle. It constructs a by-value vertex
rectangle `(x0,z0,x1+1,z1+1)`, using 32-bit integer increments without an empty-area guard. It delegates to the
owning terrain view, never to externally supplied vertex/index arrays. Dispatch is:

| Manager/context | Terrain-view consumer |
|---|---|
| water, static | `0x00755560` DrawOverlayWaterRect |
| water, dynamic | `0x00755700` DrawOverlayWaterRectDynamic |
| land, static, no vertex-buffer extension | `0x007541C0` DrawOverlayLandRect |
| land, dynamic, no extension | `0x007543C0` DrawOverlayLandRectDynamic |
| land, static, extension present | `0x007545C0` DrawOverlayLandRectWithVertBufExtension |
| land, dynamic, extension present | `0x00754910` DrawOverlayLandRectDynamicWithVertBufExtension |

Water ignores the extension; land extension consumers additionally receive the acquired extension pointer.
Evidence: `0x00735720–0x00735819`, including separately exported tail blocks `0x0073576A`/`0x007357FE`.
DrawShadowRect (`0x007356D0`) likewise expands maxima by one, but supplies the expanded rectangle by address
to land shadow consumers `0x00754C60` (static) or `0x00754F10` (dynamic). Exact terrain-owned vertex/index-array
formats inside those consumers remain unproved here; these dispatch rules do not claim their geometry bodies.
Their callable argument order, ECX object and stack cleanup are specified in §4.6.1.

Both shadow passes ask view-manager slot 9's result for slot 14 outputs `(q,s,rgb)`, with q/s binary32 scalars.
They set the texture environment colour to rgb. The comparison minimum is **binary64 zero** at `0x00A80990`:
when ordered s>0 they enable alpha testing and set comparison function 6 (greater-or-equal) with reference
`F(E(q×s))`; when s≤0 or unordered they disable alpha testing. They do **not** skip shadow submission on that
branch and do not clamp the reference. Evidence: full `0x00736C49–0x00736CAD`, rough
`0x007378E2–0x0073793A`. The old wording “above a minimum” must not be implemented with a guessed positive
epsilon. For q=1/2,s=1/4 the reference is 1/8; s=0 disables testing and does not itself suppress a submission.
The full normal pass still has the distinct qLocal rejection in §4.8.

After a nonempty decal pass, the owner calls ResetTransparency and ResetShade and requests vertex colours
enabled with both arguments 1 (`0x00736BC1–0x00736BDA`); this is a reset, not saving/restoring arbitrary
incoming shade/transparency. Full shadows disable texture state for stage 1 on their common exit
(`0x00737855–0x0073785B`). Rough shadows save render-device flag 7, enable it, and disable it on exit only
when it was initially off (`0x007378A2–0x007378BA`, `0x00737D10–0x00737D22`). Neither is a complete GPU
state snapshot/restore. Draw's common exit clears texture transform **stage 0**, restores the depth-offset
word saved from context `+0xBC`, and releases/zeros the acquired vertex-buffer extension. These actions also
occur for an empty selection after normal setup, but not for the early dynamic-snapshot return.
Evidence: `0x007391F9–0x00739203`, `0x007392E2–0x007392EF`, `0x00739378–0x0073939B`.

#### 4.6.1 Native rectangle-consumer call boundary (OV17)

All eight direct entry points below use 32-bit x86 **thiscall**. ECX is the **complete
`cSTETerrainView3D` object base**, the pointer retained at manager `+0x8C`. Its `cISTETerrainView`
interface is at base `+0x0C`; when starting with that interface pointer, subtract `0x0C` to obtain the
ECX required here. The retained manager pointer already is the complete view base. There is no stack
argument for this pointer. Evidence: view Init preserves its incoming base at `0x0075C42E` and passes
it unchanged to all four manager Init calls at `0x0075C6B6–0x0075C6E7`; manager Init retains that
argument at `0x0073CB65–0x0073CB87`. The two rectangle dispatchers load it directly into ECX at
`0x007356F2`, `0x0073573F`, `0x00735797` and `0x007357D3`. Independently, QueryInterface returns
base `+0x0C` for terrain-view IID `0x6771477D` at `0x00752744–0x0075275F`.

In the following tables, **rectangle value** means exactly 16 consecutive argument bytes: four
signed 32-bit integers `(x0,z0,x1+1,z1+1)` in that order, after the expansion in §4.6. The **rectangle
pointer** addresses a caller-owned, 4-byte-aligned 16-byte object with that same layout, available
through the call. Context and extension are each one 32-bit pointer; context is the draw context
passed to DrawRect/DrawShadowRect, and extension is the acquired pointer at manager `+0x94`.
The argument sequences below are in callable, left-to-right order, which is also their order at
increasing stack addresses after the return address. Dynamic/static selection adds no argument.

| Consumer | Argument sequence (ECX excluded) | Argument bytes / callee cleanup | Result channel | Cleanup instruction address |
|---|---|---|---|---|
| `0x007541C0` land, static | rectangle value, context | 20 / 20 | void | `0x007543B2` |
| `0x007543C0` land, dynamic | rectangle value, context | 20 / 20 | void | `0x007545B3` |
| `0x007545C0` extension land, static | context, extension, rectangle value | 24 / 24 | void | `0x00754901` |
| `0x00754910` extension land, dynamic | context, extension, rectangle value | 24 / 24 | void | `0x00754C51` |
| `0x00755560` water, static | rectangle value, context | 20 / 20 | byte in AL; DrawRect ignores it | `0x007556F4` |
| `0x00755700` water, dynamic | rectangle value, context | 20 / 20 | byte in AL; DrawRect ignores it | `0x00755894` |
| `0x00754C60` shadow land, static | rectangle pointer, context | 8 / 8 | void | `0x00754F04` |
| `0x00754F10` shadow land, dynamic | rectangle pointer, context | 8 / 8 | void | `0x007551B4` |

For an exact stack layout, let S be ESP at consumer entry, before its prologue; S holds the 4-byte
return address. Every offset in this table is relative to S. Rectangle values have no hidden pointer
or hidden result argument.

| Call family | Stack words after the return address |
|---|---|
| Ordinary land / water | `+0x04`: x0; `+0x08`: z0; `+0x0C`: x1+1; `+0x10`: z1+1; `+0x14`: context |
| Extension land | `+0x04`: context; `+0x08`: extension; `+0x0C`: x0; `+0x10`: z0; `+0x14`: x1+1; `+0x18`: z1+1 |
| Shadow land | `+0x04`: rectangle pointer; `+0x08`: context |

The normal-return ESP is S plus 4 plus the argument-byte count. The caller must not remove those
argument bytes again. For shadows, the separate 16-byte rectangle object is caller storage, outside
the 8 argument bytes: native DrawShadowRect reclaims its local object after the consumer returns
(`0x0073570F` / `0x0073571A`). That local-storage adjustment does not change the consumer's 8-byte
callee cleanup. Argument construction evidence: ordinary land `0x007357D0–0x0073580E`, water
`0x0073573C–0x0073577A`, extension land `0x00735790–0x007357C4`, shadows `0x007356D0–0x00735715`.
Extension entry points read the second stack argument as the extension receiver for CreateBuffer at
`0x007545C5–0x007545CD` / `0x00754915–0x0075491D`, independently confirming that order.

**Worked calls, all eight destinations.** Let the complete view base be `0x01234000` (terrain-view
interface `0x0123400C`), context `0x02345000`, extension `0x03456000`, and input cell rectangle
`(2,5,7,11)`. The expanded vertex rectangle is `(2,5,8,12)`. Let ESP before installing the consumer
arguments be `0x0012F000`, with any caller locals already reserved. For each shadow example, the
16-byte rectangle local starts at `0x0012F000` and contains `(2,5,8,12)`. Each row is a separate call;
ECX is `0x01234000` throughout. Listed words start at entry ESP `+0x04`, excluding the return address.

| Destination | Entry ESP | Argument words, increasing addresses | ESP after return |
|---|---|---|---|
| `0x007541C0` | `0x0012EFE8` | 2, 5, 8, 12, `0x02345000` | `0x0012F000` |
| `0x007543C0` | `0x0012EFE8` | 2, 5, 8, 12, `0x02345000` | `0x0012F000` |
| `0x007545C0` | `0x0012EFE4` | `0x02345000`, `0x03456000`, 2, 5, 8, 12 | `0x0012F000` |
| `0x00754910` | `0x0012EFE4` | `0x02345000`, `0x03456000`, 2, 5, 8, 12 | `0x0012F000` |
| `0x00755560` | `0x0012EFE8` | 2, 5, 8, 12, `0x02345000` | `0x0012F000` |
| `0x00755700` | `0x0012EFE8` | 2, 5, 8, 12, `0x02345000` | `0x0012F000` |
| `0x00754C60` | `0x0012EFF4` | `0x0012F000`, `0x02345000` | `0x0012F000` |
| `0x00754F10` | `0x0012EFF4` | `0x0012F000`, `0x02345000` | `0x0012F000` |

These are call-boundary vectors; they do not specify the consumers' geometry algorithms (OV8).

### 4.7 Camera input and composition boundary (OV7, proven subset)

Draw first calls the no-argument model-reset helper `0x007D2D20`, then copies **sixteen binary32 words** from
draw-context `+0x1AC` via `0x007D2730`. The reset helper is permitted to return immediately when context byte
`+0x138` is nonzero; otherwise it refreshes from the context camera's ViewXform (`0x007FFED0`). Consequently
the authoritative input to this owner is the post-reset context matrix, not a fabricated camera position/yaw,
projection matrix, or caller-provided completed world-to-texture matrix. Evidence: `0x00738AC9–0x00738AD5`,
getter `0x007D2736–0x007D2741`, reset guard/source `0x007D2D23–0x007D2D34`.

The owner inverts that matrix inline and stores the result as sixteen binary32 words. It computes a reciprocal
with native x87 arithmetic at `0x00739020–0x00739028` and multiplies/stores inverse entries before copying
them back at `0x0073914D–0x007391FF`. It does not substitute a transpose or rigid-camera-only inverse. The
complete cofactor/spill evaluation remains OV7; using a library inverse is not yet
an established bit-exact contract.

There is no determinant-zero comparison, epsilon guard or conditional fallback in the inverse interval.
The reciprocal is computed as native x87 `1/determinant`, **stored binary32**, then reloaded for scaling
all sixteen output entries (`0x00738FEE–0x00739141`). Singular/nonfinite inputs therefore follow the caller's
x87 exception/rounding environment; under masked exceptions, infinity/NaN can propagate. They are not
replaced with identity, a transpose, or an early successful no-draw result. This establishes singular-path
control flow, without claiming all cofactor spill arithmetic is yet specified.

Let C denote the resulting camera-to-world matrix and M an overlay's primary matrix. Decals submit the
row-vector composition **C × M** to SetTexTransform4, stage 0. In ordinary real arithmetic this takes a
camera-space input back to world before applying placement. It is not M × C. All sixteen outputs are stored
as binary32; intermediate products and additions use x87, without a store after each operation. For example,
output row 3 column 0 is `F(E(C[3,2]×M[2,0])+E(C[3,0]×M[0,0])+E(C[3,1]×M[1,0])+E(C[3,3]×M[3,0]))`,
added left to right (`0x007368E9–0x00736905`), whereas row 3 column 1 sums terms in order 3,1,2,0
(`0x00736907–0x00736923`). A uniform dot-product accumulation order is therefore not proven bit-exact.
Final submission: `0x00736B7C`. Complete sixteen-entry evaluation order in the decal and shadow
compositions remains explicitly open in OV7; the shadow branch/stage choices are specified in §4.8.

Paper order discriminator: C=translation `(16,0,0)` and M from a square, width-16 decal centred at zero
give submitted row-3 U translation 1.5. Reversing the multiplication gives 16.5. This tests composition order
without pretending to validate native inverse rounding on general matrices.

### 4.8 Full-shadow stage, strength and water boundaries (OV7/OV8)

The signed cached quality must exceed 2 **and** context render boolean `+0x17C` must be nonzero to choose
the full pass; otherwise an enabled shadow pass uses the rough body (`0x00739342–0x00739376`). This test
is separate from the add-time quality decisions in §3.7.

For a nonempty full pass, debug mode is exactly cached second-stage setting ==4. The complete 12-byte
combiner records at the three offsets below are specified in §4.1 (OV19). Stage 0 gets filtering
`(1,1)`, combiner descriptor manager `+0x38`. If `+0xD4` exists, enable and bind it on stage j, where
j=0 in debug and j=1 otherwise; filtering is `(0,0)`, wrap modes `(2,2)`, and combiner is manager `+0x50`
in debug or `+0x44` otherwise (`0x00736CB2–0x00736D3A`). No dummy binding is supplied when it is absent.
For each eligible record, the normal branch binds that record's binding on stage 0, sets wrap modes `(3,3)`
when flag bit 1 is set and `(2,2)` otherwise, and submits binary32 camera-to-world × primary on stage 0,
then camera-to-world × secondary on stage 1. Debug bypasses the per-record binding/primary composition
and submits camera-to-world × secondary on stage 0. Source intervals: normal
`0x00736D9B–0x007373D5`, debug `0x00737493–0x007377B9`. These are row-vector compositions; exact
all-entry accumulation orders still belong to OV7. The normal path's secondary matrix uses world Y through
that composition; it is not evidence for an extra direct terrain slot-15 height sample.

After the two normal transforms, byte global `0x00B0D4CC` gates a **location-strength** operation (shipped
initial byte 1). When zero, skip this entire block. Otherwise start with the global q of §4.6, compute
world midpoint `(F(E(E(cellWidth×1/2)×(x0+x1))), F(E(E(cellWidth×1/2)×(z0+z1))))` from the overlay's
**clipped stored** rectangle. Integer endpoint sums are native 32-bit sums converted as signed; 1/2 is
binary64 at `0x00A92D28`. On land only, request lighting slot 15's strength at that X/Z pair and store
`qLocal=F(E(result×q))`. On water use qLocal=q without the lighting call. Reject only an **ordered**
qLocal < binary64 0.05 (`0x00AB39A0`); equal and unordered continue. A rejected record makes no geometry
call and no counter increment, although its transforms have already been submitted. If continuing and the
global s is ordered >0, update alpha reference to `F(E(qLocal×s))`; submit environment colour again with
the fourth word changed to qLocal. This block does not multiply by the record's alpha. Debug mode bypasses
the block completely. Evidence: `0x007373DA–0x0073748E`, rejection jump `0x00737456`.

Immediately before geometry, land always invokes DrawRect. Water passes a **copy** of the stored cell
rectangle to terrain slot 53 (byte offset `0xD4`), obtains signed integer r, and compares it with
`(x1−x0+1)×(z1−z0+1)` computed from the original stored rectangle using native 32-bit arithmetic.
If r is signed less than that product, invoke DrawShadowRect with the **original** rectangle; otherwise
DrawRect. Any mutations to the temporary rectangle are not copied into the overlay or dispatched bounds.
Both routes increment `+0xDC` once after the helper. Evidence: `0x007377BE–0x0073782D`.
The rough pass invokes DrawRect unconditionally for every eligible submission and has neither this terrain
slot-53 branch nor the normal full pass's location-strength block (`0x007379B0–0x00737CEB`). What terrain
slot 53 counts or changes internally is an outside-owner gap; no water-height adjustment is inferred here.

Independent paper discriminators (conditional service inputs, not native/game execution):

- Full normal land with q=1/2, lighting strength=1/16 stores qLocal=1/32 and rejects before geometry/count;
  with strength=1/4, qLocal=1/8 continues. Setting the byte gate to zero bypasses both decisions.
- At rectangle `(1,2,3,4)` and cellWidth=16, land location-strength input is `(32,48)`. It is not the
  original unclipped centre retained by an indirect handle.
- Full water rectangle `(1,2,3,4)` has comparison product 9. Terrain return 8 selects DrawShadowRect;
  return 9 selects DrawRect. Changing the temporary rectangle in that service cannot change these bounds.
- Debug setting 4 bypasses record binding/primary transform/location rejection; setting 5 takes the normal
  path. Neither setting is a bitmask test.

## 5. Messages

The message target subobject is at manager `+0x10`; its vtable `0x00AB39A8` has handler slot 3
`0x00736100`. Init subscribes that subobject to `0xC9F775BB` when the global message server exists;
Shutdown unsubscribes it (`0x0073CD09–0x0073CD18`, `0x0073ACA2–0x0073ACB5`).

The handler obtains the message type from message slot 4. For `0xC9F775BB` it refreshes manager `+0x18`
from render-properties integers `+0x1FC` (shadow quality), `+0x1C` from integers `+0xBC` (second-stage/debug
setting), and byte `+0x20` from render-properties booleans `+0x37C`. All are copied, without range checks.
Other types leave all those fields alone. Both paths return true in AL. No overlay traversal, re-placement,
index rebuild, counter reset, terrain invalidation or geometry request occurs. This is a render-settings
refresh notification, not evidence for a terrain/zoom re-placement rule. The complete handler is
`0x00736100–0x00736141`, with the three stores at `0x00736121`, `0x0073612D`, `0x00736139`.
An independently identified sender, `cSC4View3DWin::SetRenderProperties`, constructs this message after its
settings mutation and dispatches it through server slot 4 (`0x007ECEE7–0x007ECF2D`). Init copies the two
integer settings but does not refresh `+0x20`; that byte retains constructor value 1 or a prior notification.

Paper vector: start with quality 2, second-stage setting 0, byte `+0x20`=1 and nonempty records/index.
With source settings `(0,7,0)`, an unrelated message returns true and preserves all three cached values;
`0xC9F775BB` returns true and changes them to `(0,7,0)`, leaving placement/index/counters untouched.

## 6. Users

The terrain view creates and draws the managers; the tools, lots, buildings and effects add and update decals and
shadows through the terrain view's `GetOverlayManager(index)` (slot 18) or the terrain's `+0xF8`.

## 7. Quirks

1. A dynamic manager's overlays vanish from snapshots (§4).
2. Alpha changes smaller than the step are not redrawn in a static manager (§3, slot 17).
3. Group-aware operations repeat on eight parts; scalar getters use part 0 and slot 21 does not recurse (§§2.3,3.4).

## 8. Open questions

See [questions/overlay-manager.md](questions/overlay-manager.md): OV1 outside-owner lighting projection
inputs/production; OV7 complete camera-inverse and all-entry composition rounding; OV8 terrain consumers and
outside-owner lighting/terrain services. OV2/OV3 are answered in §§3.6/3.7/5. OV4–OV6 and OV9 remain
answered in the named sections above. The complete per-body disposition, including unproved scaffolding,
is [overlay-manager-native-inventory.md](overlay-manager-native-inventory.md). This final RE round publishes
bounded contracts and parks the exact residuals; it does not claim complete native rendering equivalence.

OV12 is answered in §§2.4–2.7/3.8: native arenas, complete fields, ordered cell lists and texture metadata
are observable, but current records do not retain a complete independent creation/mutation or image-upload
history. In particular there is no model-shadow snapshot inverse. This does not close OV1's outside-owner
lighting algorithms or OV7/OV8's renderer precision/consumer residuals.

## 9. Conformance tests

1. **Handles:** a call with handle −1 does nothing.
2. **Groups:** `SetShown(group, false)` hides all eight parts.
3. **Shadow quality:** with `kShadowQuality` 0, slot 10 returns −1.
4. **Texture scale:** `SetTextureScale(h, 16, 0)` sets flag bit 5 and a matrix scale of 1/16.
5. **Snapshot:** a dynamic manager draws nothing while `kRenderSnapshot` is on.
6. **Stats:** after nonempty enabled passes submit 3 eligible decals and 2 eligible shadows, slot 27 gives
   "3 decals, 2 shadows"; empty selection and disabled passes preserve their previous counters (§4.4).
7. **Outline:** §3.1's width-256 vector produces eight specified rectangles and omits centre cell `(8,8)`.
8. **Split precision:** cell-space height 20 yields split offsets 3 and 5, using the shipped binary32 constants.
9. **Shadow mapping:** §3.5's byte flipV changes upper-edge primary V from 1 to 0 and secondary V from 3/4
   to 1/4 without changing the placement getter; `0x100` and `0x101` discriminate the low-byte test.
10. **Selection order/stamps:** §4.5's lists yield `[9,2,7]`; stamp wrap does not reset record stamps.
11. **Camera order:** §4.7's translation vector yields row-3 U=1.5, not 16.5.

### 9.1 Native observation and replay vectors (OV12)

These are paper conformance vectors for an observer and owner replay; hexadecimal words/offsets are
explicitly prefixed. They require no original-program pointer values in the independent owner.

| Case | Input/state | Required observation/result |
|---|---|---|
| OV12-A: constructed versus reserved | Overlay header `b=0x1000,e=0x121C,c=0x1384`, head/tail −1; first words at `0x1000,0x10B4,0x1168` all `0xFFFFFFFF` | Three live overlay IDs `[0,1,2]`; capacity five, not five live records. Do not inspect slots 3/4. The three corresponding field spans end at `e`. |
| OV12-B: singleton tail zero | Three constructed overlays; free head=tail=0; first words `[0,0xFFFFFFFF,0xFFFFFFFF]` | Free queue `[0]`, live IDs `[1,2]`. Stop at tail before following word zero; do not loop at index zero. |
| OV12-C: next index zero | Allocate 0,1,2; remove 1, then 0 | Head=1, tail=0, links at indices 1/0 are 0/0; queue `[1,0]`, live `[2]`. Allocate twice: returned IDs 1 then 0, both restored to `0xFFFFFFFF`; next allocation appends 3. Apply independently with strides `0x30` and `0x24` to handles/groups. |
| OV12-D: tail at last constructed slot | Three constructed entries; head=tail=2, first words `[0xFFFFFFFF,0xFFFFFFFF,0]` | Live `[0,1]`, free `[2]`; a nonzero payload/source in slot 2 does not make it live. With head/tail −1 and all first words −1, slot 2 is live. |
| OV12-E: direct overlays and groups | Live handle 0 target=`0x80000000`; live group 0 parts=`[0,1,2,3,4,5,6,7]`; live overlay 8 has no handle | Public handle `0x80000000` resolves to eight parts in stated order; overlay enumeration emits nine IDs, including direct 8, once each. Group index 0 is in a different arena from handle index 0. |
| OV12-F: padding and secondary | Two record dumps differ only at byte `+0x09`; then change N[0,0] at `+0x5C` only | First difference is not a semantic mismatch. Second is a secondary-matrix mismatch; primary M at `+0x1C` and public placement can still be bit-identical. |
| OV12-G: ordered static lists | Static nx=2,nz=2, shift 0; add overlay 0 on `(0,0,2,1)`, then 1 on `(1,0,2,2)` | Row-major heads yield `(0,0):[0]`, `(1,0):[1,0]`, `(0,1):[]`, `(1,1):[1]`. Nodes are `{nextPointer,id}`, not `{id,nextPointer}`. Reading leaves all stamps unchanged. |
| OV12-H: list history survives equal bounds | Start with overlays 0 and 1 both covering `(0,0,1,1)`, added in that order | List `[1,0]`. Re-place 0 with the same clipped bounds: still `[1,0]`. Move 0 out, then back: `[0,1]`, though final bounds/live IDs match the initial snapshot. A bounds-only reconstruction cannot choose correctly. |
| OV12-I: empty and dynamic | Constructor has all-null arena/index headers; separately, an ordinary initialized dynamic manager has live overlays and no static index | Empty constructor sample has zero constructed records (no division/dereference of null); it does not license pre-Init getters. Dynamic sample reports static index not applicable, not zero live overlays. |
| OV12-J: binding metadata and absent bytes | Known binding-info output group G, instance I≠0, levels 2, width 64,height 32; binding frame count 3 | Report keyed identity `(0x7AB50E44,G,I)`, binding size 64×32/2 levels/3 frames and local sharing token. Driver-handle array is three words, not pixel bytes. Pixel-data verification is unavailable without independent upload/resource evidence. An unknown binding leaves an initialized info buffer unchanged, not all zeros. |
| OV12-K: model input loss | Two successful model calls have equal P, bounding rectangle and pair extrema but different interior projected pairs | Same resulting record is possible; no handle, point-count field or original geometry/pairs pointer appears in either overlay. Do not infer original geometry from identical M/N. Independent projection verification needs the original geometry/descriptor and lighting state. |
| OV12-L: overwritten transform history | Add an ordinary indirect decal; call slot 21 with size 16,offset 8, without slot 13 afterward | Placement remains the original nine words while M is §3.4's replacement and bit 5 is set. Replaying only the current placement predicts a different M; capture the slot-21 input/event. |

For every observation sample, a queue/list cycle, out-of-range link or changing arena extent must produce an
explicit incomplete/failed sample, never an exact verdict based on a truncated prefix. Counts of compared
records, ordered cell lists, metadata and available image data must be reported separately. These vectors
specify expected behavior; they are not claims that an in-game live-record shadow has already run.

### 9.2 Capture boundary and inactive-header vectors (OV13–OV14)

These are paper conformance vectors, not measured game runs or a claim that an input recorder exists.

| Vector | Setup | Required result |
|---|---|---|
| OV14-A: inactive poison words | Ordinary dynamic manager, null index-vector pointers, valid arenas with one live overlay; index shift `0xFFFFFFFF`, nx `0x80000000`, nz `0x7FFFFFFF` | Complete arena observation; static grid N/A and no static cells. Retain raw header words diagnostically; do not reject on shift/shape, multiply dimensions, or drop the live record. |
| OV14-B: repeat Init | Initialize static, create an overlay, then Init dynamic using the same terrain | Arenas survive repeat Init; old grid storage can survive too. Dynamic observation does not enumerate that retained storage as active cells or require dynamic membership. Terrain counts +5C/+60 remain distinct from inactive index dimensions. |
| OV14-C: active grid discriminator | Same poison scalar words as A, but ordinarily initialized static manager claiming an active index | A is not permission to accept an invalid active static grid. Apply §2.6's active-grid shape/storage checks; an invalid sample is incomplete/failed. |
| OV13-A: late attach | Attach after city load; observe valid arena/getter/ordered-cell/metadata data, but no original creation events or upload data | Observation-invariants coverage may be complete for that sample. Creation-history replay and image-byte verification are unavailable; equal matrices or metadata do not change that verdict. |
| OV13-B: interface-only interception | Record all §3 virtual add/setter calls, but miss the four direct Init calls and initial factory/quality inputs | Not a complete pre-allocation input stream. First successful add does not establish the empty initialized state or original dimensions/settings. |
| OV13-C: borrowed model inputs | n=3, three packed XYZ points (36 bytes), three packed UV pairs (24 bytes), six-word box (24 bytes), descriptor fields through +52; caller reuses those buffers after return | Replay uses entry copies and original lighting/terrain state. Descriptor +1 is flags, +4 begins the matrix, +52 is scale. Reading buffers later, using a descriptor pointer as outP, or supplying native P/bounds instead fails independent projection coverage. |
| OV13-D: failed and nested calls | A model call fails projection; a later successful model call invokes the internal rectangle helper once | Keep the failure in the ordered stream with no overlay allocation; replay the successful public model call once, not once plus a second helper add. IDs are compared as outputs. |
| OV13-E: transient upload | One source frame has three mip levels; binding has two. Bytes at source levels 1 and 2 are uploaded, then the caller changes/frees the table and bytes | Keep original consumed level-1/2 bytes and metadata before return, plus any later upload/restore events. A post-return table or stable binding/driver handle cannot prove original or current image bytes. |

### 9.3 Ordinary decal flag vectors (OV15)

These are instruction-derived conformance expectations, not newly executed native/game tests. Use valid
nonnull resources with finite positive dimensions, valid initialized managers and sufficient city bounds;
inspect the live overlay's complete 32-bit flags word, resolving the returned indirect handle (§2.4).

| Vector | Input / operation | Required result |
|---|---|---|
| OV15-A: controlled suffix | Slot 4 with a 64×64 binding, position `(128,128)`, width 32, turns 0; then slots 12/17/18/19/20 with the exact values in §3.2.2 | Flags after the six calls: `[0x91,0x91,0x91,0x91,0x91,0xD1]`. Draw-state byte is 3; it contributes no bits to the flags word. |
| OV15-B: mode and resource independence | Repeat A on static land, static water, dynamic land and dynamic water; use distinct 64×64 and 128×64 bindings; compare quality 0 with 5 | Every run has the same six flags words as A. Aspect changes placement coverage/matrix as §3.2 specifies, not bit 7. Dynamic snapshot suppression does not remove the stored bit. |
| OV15-C: creation families | Ordinary slot 3 with a valid source, or slots 5/6 resolving to valid bindings, at A's position/width/turns | New ordinary overlay flags `0x91`; after self-lighting 1, `0xD1`. A null slot-4 binding returns −1 and allocates no overlay/handle. |
| OV15-D: preserve bit 7 across later setters | Starting after A, slot 20 argument 0; then slot 13 with placement `[144,160,32,0,1,2,1,0,0]`; then slot 13 restoring both repeats to 1 | Flags `0xD1`, then `0xD3`, then `0xD1`. False self-lighting does not clear bit 6; repeat changes affect bit 1 only. |

### PR review: slot 20 self-lighting

The body at `0x007364F0` ignores handle -1, resolves indirect handles as in section2.1, and applies itself recursively to all eight indices of a group. For a resolved overlay it sets flag bit6 only when the low bit of the second argument is1. It does not clear the bit for0, and makes no redraw call. This is a two-argument member call, callee pops8; no return value is specified. Evidence: branches at `0x007364FA`, `0x00736515`, recursion at `0x00736548`, bit test at `0x00736558` and flag update at `0x0073656F`.

Conformance: starting with flags0, calls with on=0,1,0 yield flags0,0x40,0x40. A group applies that same operation to its eight indices; -1 leaves every record unchanged.
