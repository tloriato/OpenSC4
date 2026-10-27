# Spec: texture bindings and materials (`cS3DTextureBindingFactory`, `cS3DTextureBinding`, `cS3DMaterialManager`)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`). Implementers must work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled
or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[compat-limit]* depends on something FreeCity4 cannot reproduce, so only the rule is matched.

Related specs: [graphics-formats.md](graphics-formats.md) (§2.6, the image formats; §2.10, the level upload; §3.8,
the MATS record; §4.1, the texture lookup order; §5, the address table this spec extends),
[INVENTORY-M6.md](INVENTORY-M6.md) (R3, the model and texture system), [foundations.md](foundations.md) (§5.1,
messages; §17.6, the resource manager).

## 0. Summary

The **texture binding factory** turns images (FSH textures or images built in memory) into renderer textures
("bindings"). It sizes each binding to powers of two minus a LOD offset, picks an internal format from the image
format and the display depth, caches bindings by resource key, keeps released bindings in a **free pool** of at
most 2,000,000 bytes for reuse, and re-uploads keyed textures when the driver asks for them to be restored. A
**binding** is a counted object holding one texture handle per frame. The **material manager** builds the in-memory
material records of models from the MATS data.

## 1. `cS3DTextureBindingFactory` *[compat]*

### 1.1 Identity

| Item | Value |
|---|---|
| COM class | `0x9AC0E23C` (`GZCOM_cS3DTextureBindingFactoryCLSID` `0x00809120`); `GZCOM_CreatecS3DTextureBindingFactory` (`0x0080A680`) allocates 0x84 bytes; `cSC4App::AddSystemServices` also constructs one |
| System service | ID `0x1AC0E11A`, priority `0x130EE8` (the `cRZSystemService` at `+0x08`) |
| `QueryInterface` (`0x00809130`) | `1` and `0xFAC0E219` → this; `0x088CEB67` → `+0x04`; `0x287FB697` → `+0x08`; anything else false |
| Vtables | `+0x00` `0x00ABD8C8` (19 slots, §1.10), `+0x04` `0x00ABD8B8`, `+0x08` `0x00ABD884` (system service), `+0x0C` `0x00ABD86C`, `+0x24` `0x00ABD85C` (message target) |

The interfaces at `+0x04`, `+0x08`, `+0x0C` and `+0x24` start with `this − n` thunks to `QueryInterface`, `AddRef`
and `Release` (`0x00809920`…`0x008099E0`); the `+0x0C` one's slot 3 runs the deleting destructor
(`0x008099B0` → `0x00809FF0`). `AddRef` and `Release` (`0x008091B0`, `0x008091C0`) are the system service's. The `+0x04` interface (IID
`0x088CEB67`, TB1) has only those three thunks and one further slot (`0x00648F10`, a one-argument body that returns at
once, shared with many unrelated vtables); no other code in the executable names that IID *[inferred: an inert view]*.

### 1.2 State (0x84 bytes)

| Offset | Field | Initial |
|---|---|---|
| `+0x28` | binding table: binding → record (buckets `+0x2C`, count `+0x38`) | empty (size hint 100) |
| `+0x3C` | key table: (group, instance) → record (count `+0x4C`) | empty (size hint 100) |
| `+0x50` | free list (records, newest first) | empty |
| `+0x54`, `+0x58` | bytes in the free pool, bytes in use | 0 |
| `+0x5C`, `+0x60` | bindings in use, bindings in the free pool | 0 |
| `+0x64` | free-pool limit in bytes | 2,000,000 |
| `+0x68` | display bit depth | 32 |
| `+0x6C` | LOD offset | 0 |
| `+0x70` | resource group | `0x1ABE787D` |
| `+0x74`, `+0x78`, `+0x7C`, `+0x80` | graphic system, driver, resource manager, message server 2 | null |

Each binding has a **record** (0x34 bytes): the binding, its per-frame texture handles, in use (bool), the key
instance and group (instance 0 = not keyed), the frame count, level count, width and height of the binding, its
size in bytes, the image's red, green, blue and alpha bit counts, its bits per texel and its internal format.

### 1.3 Lifetime

- `Init` (system-service slot 8, `0x008093A0`): first `Shutdown`; then the resource manager (framework service
  `0x056B906E`, IID `0x656B8EFC`), the graphic system (IID `0xC47B747C` from the framework's graphic system), its
  driver (graphic-system slot 3) — false if any is missing; then the message server 2 (service `0x04FA845B`, IID
  `0x652294C7`): when present, the `+0x24` target subscribes to `kSGLMessage_RestoreTextures` `0x8ACAA95F`. True.
- `Shutdown` (slot 9, `0x008091D0`): `FlushAll` (§1.7); release the driver, the graphic system and the resource
  manager; unsubscribe and release the message server. True.
- `SetGDriver(d)` (slot 3, `0x008092F0`): false when `d` is the current driver; else `FlushAll`, then hold `d`; true.
- The destructor (`0x00809890`) runs `Shutdown` and frees the tables; the deleting destructor (`0x00809FF0`) also
  frees the object when bit 0 of its argument is set.

### 1.4 Binding size and internal format

For an image of `L` levels, width `w` and height `h` (`0x008094F0`):

- `W := P(w)`, `H := P(h)`, P the smallest power of two ≥ the value (1 for values ≤ 1; `NextPowerOfTwo`
  `0x0080A700`);
- `d := min(LOD offset, L − 1)`; the binding has `L − d` levels, width `max(1, W >> d)` and height
  `max(1, H >> d)`. With the level upload of graphics-formats §2.10, the `d` largest levels are skipped.

The internal format (`FindBindingParams`, `0x00809330`) from the image format f (graphics-formats §2.6) and the
image's alpha bits a (`nRZSimGL::FindBitCounts`):

| Condition | Internal format | Bits per texel (`0x00ABD774`) |
|---|---|---|
| f = 7 or 8 (DXT1) | 5 | 4 |
| f = 9 (DXT3) | 6 | 8 |
| f = 10 (DXT5) | 7 | 8 |
| bit depth ≥ 24, a = 0 | 1 | 24 |
| bit depth ≥ 24, a ≠ 0 | 4 | 32 |
| bit depth < 24, a = 0 | 0 | 16 |
| bit depth < 24, a = 1 | 3 | 16 |
| bit depth < 24, a > 1 | 2 | 16 |

The size in bytes is (Σ over the levels of width_k × height_k) × bits per texel × frames / 8, with width and height
halved (no clamp) per level.

### 1.5 Creating and reusing bindings

An **image info** gives the frame count, level count, width, height, format, type and the per-frame level table.

- **Reuse** (`GetFreeBinding`, `0x00809CD0`): scan the free pool **oldest first** for a record with the same frame
  count, level count, width, height (as §1.4 computes them) and internal format. The first match leaves the pool
  (counters moved to "in use"), takes the new bit counts and, when keyed, loses its key.
- **Create** (`CreateNewBinding`, `0x0080A110`): a record; one driver texture per frame (driver slot 60 with the
  internal format, width, height, levels and 2); if the driver reports an error (slot 50) the textures made so far
  are deleted (slot 36) and creation fails. Else a binding (§2) with one reference held by the factory, registered
  in the binding table, counted in use.
- **Upload** (`SetBindingImageData`, slot 7, `0x00809B90`): graphics-formats §2.10 (driver slot 61 per frame and
  level). Unknown bindings are ignored.

Getting a binding (each gives the caller a new reference; failure gives null and false):

| Slot | VA | Arguments | Behaviour |
|---|---|---|---|
| 4 | `0x0080A320` | (image info, out) | reuse or create, mark in use, upload; never keyed |
| 5 | `0x008094C0` | (ID, out) | slot 6 with the key (`0x7AB50E44`, `0x1ABE787D`, ID): the group is **hard-coded**, not `+0x70` *[quirk]* |
| 6 | `0x0080A380` | (key, out) | below |

Slot 6: look up (group, instance) in the key table (the type is ignored). Found: if the record is in the free pool,
take it out (counters moved) and mark it in use, **without** re-uploading; return its binding. Not found: load the
resource through the resource manager (slot 4, IID `0x1AB50C3F`); no resource → false. Its image info (resource
slot 3) → reuse or create, mark in use, key it with (group, instance), enter it in the key table, upload; release the
resource. A null image info gives false.

### 1.6 The free pool

- `FreeBinding(binding)` (slot 18, `0x0080A4D0`; called by the binding, §2): the record leaves "in use", goes to the
  **front** of the free list, and the pool is trimmed.
- **Trim** (`AdjustFreePoolSize`, `0x0080A0A0`): while the pool holds more bytes than the limit, destroy the record at
  the **back** (the oldest); the byte counter is clamped at 0.
- `SetFreePoolMax(n)` (slot 14, `0x0080A580`): limit := n, trim. `FlushFreePool` (slot 15, `0x0080A590`): trim with
  limit 0, then restore the limit. Slot 13 returns the limit.
- **Destroy** (`DestroyBinding`, `0x00809EB0`): drop the key entry when keyed and the binding entry, delete the
  driver textures (slot 36), free the binding object, the handle array and the record.

A keyed texture released and asked for again before it is evicted or reused comes back from the pool unchanged.

### 1.7 `FlushAll` (slot 16, `0x0080A010`)

Empties the free list, then destroys every record in the binding table — **including bindings still in use**, whose
holders are left with freed objects *[quirk]* — and clears the key table. Run by `Shutdown` and `SetGDriver`.

### 1.8 Restoring textures (`DoMessage`, `+0x24` slot 3, `0x008099F0`)

On `0x8ACAA95F` (`kSGLMessage_RestoreTextures`): for every keyed record, reload (`0x7AB50E44`, group, instance)
through the resource manager and upload it into the record's binding (slot 7). Bindings made from an image info
(slot 4) are not restored here *[inferred: their owners redo them]*. Other messages are ignored; the handler always
returns true.

### 1.9 Resource group and settings

- `SetResGroup(g)` (slot 12, `0x00809B00`): when `g` differs and the key table is not empty, every key entry is
  removed and its record un-keyed (instance := 0; `0x00809690` removes one entry); the bindings themselves live on.
  Then `+0x70` := g. Slot 11 returns `+0x70`.
- `SetLodOffset(n)` (slot 9, `0x00809910`): `+0x6C` := n; it affects bindings made afterwards.
- Slot 10 (the shared setter `0x006D6E00`) sets the display bit depth `+0x68`.

**Who sets them (TB2)** *[compat]*. The only caller found for slots 9, 10 and 14 is the effects manager's handler of
`kMessageTypeRenderPropertiesChanged` `0xC9F775BB`, `cSC4EffectsManager::DoRenderPropertiesChanged` (`0x00593E10`),
which also runs once when the effects are first loaded (`0x005947D9`) and so sets the factory at startup. It gets
the factory (service `0x1AC0E11A`, IID `0xFAC0E219`) and calls, in this order (`0x00593F6C`–`0x00593FCC`):

1. `SetFreePoolMax` (slot 14) with the truncated product of the float render property `kTexBindMaxFree` (float 2,
   default 2, range 0–32) and 1,000,000.0, so the shipped 2 reproduces the factory's own 2,000,000-byte limit (§1.2);
2. `SetLodOffset` (slot 9) with the integer render property `kLODOffset` (integer 3, default 0);
3. slot 10 with the integer render property `kTextureBits` (integer 2, default 32).

The defaults equal the initial values of §1.2, so without any Graphics Rules the factory keeps its initial state.
The render properties can be written by name by the Graphics Rules (`Graphics Rules.sgr`, graphics-rules.md), a data
file, so which option or detected value feeds `kLODOffset` and `kTextureBits` is decided there and cannot be read
from the executable (graphics-formats.md §2.10 infers the Texture Quality option for the LOD offset, which stays
*[inferred]*).

### 1.10 Information

- `GetBindingInfo(binding, out)` (slot 8, `0x00809E10`): group, instance, level count, width, height, the four bit
  counts and the bits per texel; untouched for an unknown binding.
- `GetStatsString(s)` (slot 17, `0x00809260`) formats (string slot 27)
  `"Textures: %d (%1.1f MB) in use, %d (%1.1f MB) awaiting reuse, %d (%1.1f MB) total\n"` with the in-use count and
  bytes, the pool count and bytes, and the sums; MB = bytes × 2^−20.

| Slot | VA | Name |
|---|---|---|
| 0–2 | `0x00809130`, `0x008091B0`, `0x008091C0` | QueryInterface, AddRef, Release |
| 3 | `0x008092F0` | SetGDriver |
| 4–6 | `0x0080A320`, `0x008094C0`, `0x0080A380` | GetBinding (info, ID, key) |
| 7, 8, 9 | `0x00809B90`, `0x00809E10`, `0x00809910` | SetBindingImageData, GetBindingInfo, SetLodOffset |
| 10, 11, 13 | shared bodies | set bit depth, get resource group, get free-pool limit |
| 12, 14, 15, 16 | `0x00809B00`, `0x0080A580`, `0x0080A590`, `0x0080A010` | SetResGroup, SetFreePoolMax, FlushFreePool, FlushAll |
| 17, 18 | `0x00809260`, `0x0080A4D0` | GetStatsString, FreeBinding |

### 1.11 Native word arithmetic and invalid-domain boundaries (TB5)

These rules qualify the mathematical description in §1.4. Dimensions, image frame/level counts, LOD offset,
record byte size and factory byte/count fields are 32-bit words. Addition, subtraction and the low-word products
wrap modulo 2^32; there is no checked-size rejection. This describes the native calculation, not a requirement
to allocate an invalid or impossibly large image.

- `P` (`0x0080A700–0x0080A716`) compares its argument and successive powers as **signed** 32-bit integers.
  Zero, one, and any word with bit 31 set return 1. Positive values through `0x40000000` round upward normally.
  Positive values `0x40000001..0x7FFFFFFF` do not terminate: doubling reaches the negative sign bit, then zero,
  and stays zero. There is no native overflow error result.
- The LOD selection uses an **unsigned** comparison: use the offset if `L > offset`, otherwise use `L−1`
  modulo 2^32 (`0x00809514–0x0080952B`). Subtract that word from L for the output level count. Consequently
  `L=0` selects `d=0xFFFFFFFF` and produces **one** binding level, rather than rejecting the input.
- Width and height use logical right shifts with `d & 31`, followed by a **signed** comparison against 1;
  only values signed-greater-than 1 survive (`0x0080952D–0x00809573`). Thus `L=33, offset=32, w=h=8`
  gives one level of **8×8**, not 1×1; `L=0, w=h=8` gives one level of 1×1.
- Size starts with the binding dimensions. For each binding level, add the low 32 bits of width×height to a
  32-bit sum, then logically halve each dimension without clamping. Multiply the wrapped sum first by bits
  per texel, then by frames, retaining only the low word each time; finally logically shift right by 3
  (`0x0080A253–0x0080A296`). It is not a wide product divided by 8. A one-level 65536×65536 image at 32 bits
  and one frame therefore has accounted size **0**. Zero frames also gives size 0 and makes no driver texture
  creation calls; there is no frame-zero rejection (`0x0080A1EE–0x0080A253`).
- Creation increments in-use words with wrapping arithmetic (`0x0080A300–0x0080A30B`). Returning to the pool
  subtracts size from in-use bytes, adds it to pool bytes, decrements/increments the respective counts, all
  wrapping (`0x0080A516–0x0080A537`). Reuse and keyed reacquisition reverse those transfers, also wrapping
  (`0x00809D91–0x00809DB9`, `0x0080A3C9–0x0080A3EE`). Trimming compares bytes and limit **unsigned**;
  its subtraction alone saturates at zero when record size is at least the current pool bytes
  (`0x0080A0A3–0x0080A0FF`). A wrapped pool total can therefore suppress trimming.
- Binding references are a wrapping **32-bit** word, not the material's 16-bit word. `AddRef`
  (`0x004664A0`) turns `0xFFFFFFFF` into 0 without freeing; `Release` (`0x008090E0`) turns 0 into
  `0xFFFFFFFF`. Only a decrement producing exactly 1 with a factory calls `FreeBinding`; exactly 0 frees
  the binding allocation. No overflow/underflow guard exists.

### 1.12 Instance-zero keys and upload failures (TB6–TB7)

Keyed lookup accepts instance zero and inserts the (group, 0) entry just like any other key
(`0x0080A454–0x0080A471`). Releasing and asking for that key before reuse/eviction returns the same binding
without upload. The record's instance word nevertheless remains the unkeyed sentinel:

- Reuse tests the instance word before key removal (`0x00809DCD–0x00809DF0`), so reusing this record for an
  image-info request leaves the old (group, 0) entry pointing at the repurposed record. A subsequent keyed
  lookup can return that binding with its new contents. Reusing it for another key can leave two entries
  pointing to the same record.
- Destruction makes the same zero test (`0x00809EB9–0x00809EE1`), leaving a dangling key-table value after
  eviction. A later lookup is unsafe; there is no specified false/error recovery from the freed record.
  `FlushAll` ultimately clears the key table (§1.7).
- Restoration walks **key-table entries**, not records filtered by nonzero instance. The stale instance-zero
  entry is therefore still visited. It reconstructs the load key from the record's current instance/group
  (`0x00809A30–0x00809A69`), which may differ from the table key after reuse.

Driver creation stores each returned handle before querying the driver error. On failure at zero-based frame
i, slot 36 receives **count i** and the array base (`0x0080A22D–0x0080A244`, `0x0080A2B9–0x0080A2CE`).
The just-returned handle is excluded, even if nonzero. Failure on the first call issues deletion with count 0;
failure on the third deletes only handles 0 and 1. A zero handle among those earlier successful calls is still
included. This path frees the record but does **not** free the separately allocated handle array before returning
failure. These observations correct §1.5's ambiguous “made so far”; there is no per-handle nonzero filter.

Upload (`0x00809B90–0x00809CC6`) reads image-info fields even before deciding whether the binding is known.
For a known binding it loops over the **source image's frame count**, and over the **binding's level count**.
The first source pointer is at source-level-table index `sourceLevels−bindingLevels` (32-bit subtraction);
each successive frame advances by `sourceLevels` entries. Thus fewer source frames leave later handles
unchanged; more source frames read beyond the binding's handle array. Insufficient source levels can address
before the level table. Neither a missing table nor a null level pointer produces a native early failure:
the table entry is read and its value is passed to the driver unchanged. Upload widths/heights are logical
shifts by `level & 31`, without a minimum of 1. The source format/type are forwarded. The driver return is
ignored and no error query follows, so later levels/frames are still attempted after a reported upload error
(`0x00809C50–0x00809CBB`). Invalid memory accesses have no native boolean failure contract.

Restore attempts resource load per table entry, skips upload if load fails or image-info is null, releases a
returned resource, and continues (`0x00809A6C–0x00809A97`). Traversal follows each hash chain then the next
occupied bucket, in ascending bucket index (`0x00809A97–0x00809AD3`), rather than sorted resource-key order.
Driver/load call order is observable. Exact ordering across differing hash-table growth/insertion histories
requires the same table state; this section does not invent a key-order guarantee.

## 2. `cS3DTextureBinding` *[compat]*

16 bytes: the per-frame texture handles, the frame count, the factory, a reference count (constructor `0x008090C0`;
`AddRef` is a shared body). `Release` (`0x008090E0`): decrement; if the binding has a factory and the count is now
1 (only the factory's reference is left), `FreeBinding` (§1.6) and return 1; if it is 0, free the object and return
0; else return the count.

## 3. `cS3DMaterialManager` *[compat]*

| Item | Value |
|---|---|
| COM class | `0x9AC0E23D` (`0x00800BF0`); `GZCOM_CreatecS3DMaterialManager` (`0x00800E30`) allocates and builds 0x20 bytes |
| System service | ID `0x1AC0E11B`, priority `0x130EE8` (at `+0x04`) |
| `QueryInterface` (`0x00800C00`) | `0xFAC0E21A` → this; anything else through the system service |
| Vtables | `+0x00` `0x00ABD3F4` (8 slots), `+0x04` `0x00ABD3C0`, `+0x08` `0x00ABD3A8` (thunks `0x00800C70`, `0x00800CA0`; slot 3 `0x00800CD0` → the deleting destructor `0x00800CE0`) |

- `CreateMaterial(record, out)` (slot 3, `0x00800D70`): a material object (0x34 bytes, `cS3DMaterial` constructor
  `0x00800D20`) with flags := the record's low 16 bits | `0x180`, the alpha and depth functions, the source and
  destination blends and the alpha threshold's high byte (graphics-formats §3.8), and T texture slots, each the
  texture ID and the wrap S, wrap T, mag and min bytes, with no binding yet. Of the two slots, those past T are
  marked unused (slot byte `+0x0C` := `0xFF`); T > 2 writes past the object (graphics-formats §3.8). The material's
  first byte := 1 and its reference count (`+0x02`) := 1; true.
- Default texture slot (constructor): no binding, wrap S and T 3 (repeat), mag and min 1 (linear), slot byte `+0x0C`
  1, bytes `+0x0D` and `+0x0E` 0, a float 1.0 at `+0x10`; the ID is left unset (`CreateMaterial` writes it for
  the used slots). The 3D draw context reads them as the stage's texture-environment mode (`+0x0C`), its
  texture-coordinate set (`+0x0D`), a texture-transform switch (`+0x0E` bit 0) and that transform's scale
  divisor (`+0x10`) (draw-context.md §6); slots past T are never read, so their `0xFF` is only a marker.
- Slot 4 (a shared body) always returns false; `GetStatsString` (slot 5, `0x00800C50`) formats "Nothing to report".
- Primary slots **6 and 7** both target `0x0062C980`: return false, discard two 32-bit arguments, and do not
  dereference them or mutate outputs. No method names or successful behavior are established by that body.
- Primary AddRef/Release (`0x00800C30`, `0x00800C40`) adjust the receiver by +4 and delegate to the system
  service. The deleting destructor (`0x00800CE0–0x00800D13`) restores all three vtables, destroys the base at
  +4, then frees the 0x20-byte allocation iff flags bit 0 is set. It does not enumerate materials or bindings.

### 3.1 Material value and reference lifetime (TB4)

A material is a nonvirtual 0x34-byte object. Its constructor writes count zero at `+0x02`;
CreateMaterial increments that word after publishing the output (`0x00800D27`, `0x00800E1B–0x00800E24`).
Allocation failure still reaches null material field writes (`0x00800D7D–0x00800D99`), not a clean false.

The unnamed body `0x008045B0–0x008045F3` (68 bytes) is material release. Read the count as unsigned
16-bit. If greater than 1, decrement, store and return the count zero-extended. For **either 0 or 1**,
release each nonnull held binding, first stage 1 (`material+0x24`), then stage 0 (`+0x10`), free the
allocation, and return zero. This branch does not store a decremented count. Unused stage markers
do not suppress release. In particular release at zero frees instead of underflowing to 65535.
Material-reference assignment (`0x008048F0–0x0080490F`) increments the new nonnull material's count
with 16-bit wrap, stores its pointer, then releases the old material; equal pointers cause no calls.

Value assignment (`0x00802240–0x008022A9`, 106 bytes) copies the first byte, the **count word**, flags
and bytes `+0x06..+0x0B`, then assigns stage 0 and stage 1; it does not acquire a material reference.
Stage assignment (`0x00802070–0x008020E8`, 121 bytes) copies the texture ID, then, if binding pointers
differ, AddRefs the new nonnull binding, stores it, and Releases the old nonnull binding; finally it
copies sampler/environment/coordinate/transform fields. Equal pointers cause no reference operations.

Value-range destruction (`0x008021F0–0x00802234`, 69 bytes; export label `vector::erase`) walks
0x34-byte values forward, releasing stage 1 then stage 0 per value, without consulting material counts
or freeing the enclosing range. Temporary cleanup at `0x00805676–0x00805686` uses that reverse-stage order.

Distinguishing vectors: count 0 with two bindings releases stage 1, stage 0, then frees; count 2
becomes 1 with no binding calls. Acquiring a material at 65535 wraps to 0 before releasing the old
material. Assigning an equal binding performs no reference calls; assigning a different binding
acquires it before releasing the old binding.

### 3.2 Inventory scope (TB4)

The brief's **58 bodies / 4883 bytes** is the coverage-map subsystem scope, including five free
functions, rather than 58 class methods. Replaying the attribution against the main checkout's
export, name map and ABI tables gives:

| Owner / grouping | Bodies | Export body bytes |
|---|---:|---:|
| cS3DTextureBindingFactory | 42 | 4267 |
| cS3DTextureBinding | 3 | 89 |
| cS3DMaterialManager | 7 | 328 |
| cS3DMaterial | 1 | 65 |
| Class/create free functions: `00800BF0`, `00800E30`, `00809120`, `0080A680` | 4 | 111 (6 + 72 + 6 + 27) |
| NextPowerOfTwo, `0080A700` | 1 | 23 |
| Total | **58** | **4883** |

The class-body membership is below (hexadecimal address / decimal export body bytes). Repeated
adjustor entries count as distinct bodies; repeated vtable uses of the same address do not.

| Class / body group | Addresses / bytes |
|---|---|
| Binding | `004664A0`/8, `008090C0`/32, `008090E0`/49 |
| Material | `00800D20`/65 |
| Material manager | `00800C00`/39, `00800C50`/21, `00800C70`/8, `00800CA0`/8, `00800CD0`/8, `00800CE0`/52, `00800D70`/192 |
| Factory: QI/refcount/Shutdown/stats/driver | `00809130`/115, `008091B0`/8, `008091C0`/8, `008091D0`/108, `00809260`/140, `008092F0`/62 |
| Factory: params/Init/lookup/dimensions | `00809330`/104, `008093A0`/286, `008094C0`/48, `008094F0`/134 |
| Factory: key erasure/destruction/LOD | `00809690`/122, `00809890`/118, `00809910`/10 |
| Factory: adjusted entries, first group | `00809920`/8, `00809930`/8, `00809940`/8, `00809950`/8, `00809960`/8, `00809970`/8, `00809980`/8 |
| Factory: adjusted entries, second group | `00809990`/8, `008099A0`/8, `008099B0`/8, `008099C0`/8, `008099D0`/8, `008099E0`/8 |
| Factory: message/group/upload/free lookup/info/destruction | `008099F0`/264, `00809B00`/140, `00809B90`/311, `00809CD0`/311, `00809E10`/147, `00809EB0`/114, `00809FF0`/30 |
| Factory: flush/trim/create/lookup | `0080A010`/137, `0080A0A0`/100, `0080A110`/522, `0080A320`/89, `0080A380`/332 |
| Factory: return/limit/pool flush/construction | `0080A4D0`/163, `0080A580`/15, `0080A590`/25, `0080A5B0`/200 |

The earlier name-only count **55/4758** additionally includes two folded names: the 5-byte
CreateMaterialVariant tail jump at `005BE2E0` and the 4-byte GetResGroup getter at `00809240`.
These shared entries are callable contracts (§§1.10/3), but attribution does not assign them to
these class owners. Subtract 2/9, then add the five free functions' 5/134 to obtain 58/4883 exactly.
The bits-per-texel table and two named vtable addresses are **data**, not bodies. The earlier
59/4869 helper subtotal was a different scope and omitted NextPowerOfTwo.

This reconciliation does not remove material acquisition/release, assignment, value-copy and
range-cleanup contracts in §3.1 just because their shared/unnamed bodies are outside this subtotal.
Inherited shared system-service slots are likewise entries, not additional exclusive owner bodies.
No extra virtual material API follows from the subsystem count.

## 4. Users

The model frame resolution of graphics-formats §4.1 (slots 5 and 6); code that builds images in memory (slot 4)
*[inferred]*; the S3D reader (`CreateMaterial`, graphics-formats §3.8); the application
(`cSC4App::AddSystemServices`) and the COM director (`0x00460590`). `SL::OnAppInit1` keeps the factory in the global
`0x00B43CD4`, which the glyph texture manager tests before re-rendering its glyphs (glyph-textures.md §1.6).

## 5. Quirks

1. Slot 5 hard-codes the group `0x1ABE787D` instead of the resource group (§1.5).
2. `FlushAll` frees bindings that are still in use (§1.7).
3. A texture keyed with instance 0 counts as un-keyed: when its record is reused or destroyed, the key entry is not
   removed and later finds the wrong record *[inferred]* (§1.2, §1.5).
4. Level sizes are halved without a clamp when the size is computed, but clamped at 1 for the binding (§1.4).

## 6. Open questions

TB2 (who sets the bit depth and the LOD offset, and from what) is answered in §1.9 and archived. Still open in
[questions/texture-bindings.md](questions/texture-bindings.md): TB1, the interface `0x088CEB67` at `+0x04` (§1.1: no
other code in the executable mentions that interface ID, and its only slot beyond the three `this − 4` thunks is a
one-argument body that does nothing and is shared by dozens of vtables, so it is an inert secondary view whose
identity the executable does not reveal). TB3 (the material slot bytes) is answered in §3.

## 7. Conformance tests

1. **Size:** a 6-level 100 × 60 image with LOD offset 1 gives a 5-level binding of 64 × 32.
2. **LOD clamp:** a 1-level 8 × 8 image with LOD offset 2 gives 1 level of 8 × 8.
3. **Format:** DXT1 → 5 (4 bits); 32-bit display, 8-bit alpha → 4 (32 bits); 16-bit display, 1-bit alpha → 3.
4. **Bytes:** one frame, 3 levels of 64 × 64 DXT3: (4096 + 1024 + 256) × 8 / 8 = 5376 bytes.
5. **Cache:** asking twice for the same (group, instance) by key loads the resource once and returns the same binding.
6. **Pool:** with a limit of 10,000 bytes, releasing bindings of 6,000 and then 5,000 bytes destroys the first (the
   oldest) and keeps the second; asking for the second's key again returns it without a load.
7. **Reuse:** with a free 64 × 64 DXT1 binding in the pool, creating another 64 × 64 one-frame DXT1 binding by image
   info reuses it (no driver texture is created) and removes its key.
8. **Release:** a binding returned to one caller has 2 references; the caller's `Release` puts it in the pool.
9. **Stats:** 2 bindings of 1 MiB in use and 1 of 0.5 MiB pooled format as "Textures: 2 (2.0 MB) in use, 1 (0.5 MB)
   awaiting reuse, 3 (2.5 MB) total".
10. **Material:** a record with flags `0x00010022`, T = 1 gives flags `0x01A2`, slot 0 with the ID and slot 1 unused.
