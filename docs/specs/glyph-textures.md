# Spec: glyph and string textures (`cSC4GlyphTextureManager`)

Status: **behaviour spec**, clean-room input. This document describes *behaviour*, not code. It was written by the
analysis team from the Windows 1.1.641 executable (read in the instruction bytes and in a Ghidra 12.1.4 project of
the Steam build, whose code equals the GOG build's; evidence in `docs/lanes/re-dates.md`). Implementers must work
from this spec, the public LGPL gzcom-dll SDK headers and general knowledge only. They must **not** consult decompiled
or disassembled code (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Normative words: **must**, **should**, **may**. Tagged notes: *[compat]* is required for compatibility with the
original game and its plugins; *[choice]* is a behaviour we chose where the original was arbitrary or undefined;
*[inferred]* is consistent with all evidence but not proven; *[quirk]* is surprising original behaviour that is
kept on purpose; *[compat-limit]* depends on something FreeCity4 cannot reproduce, so only the rule is matched.

Related specs: [texture-bindings.md](texture-bindings.md) (the bindings these textures are made of; the restore
message), [foundations.md](foundations.md) (§5.1, messages).

## 0. Summary

Text drawn in the 3D view (labels, signs, balloons) is made of textures rendered from a font. The glyph texture
manager keeps two kinds:

- **glyph textures**: one texture per (character, font), shared by everyone who asks for that pair and freed as soon
  as nobody holds it;
- **string textures**: a whole string in one texture, created on first use and kept in a cache of at most
  `kStringTextureCacheSize` KB (1 MiB by default), trimmed every 50 ms with a second-chance sweep.

Both are rendered by `nS3D::CreateTextureFromText` (`0x0080AB90`, out of scope here) with a font from the framework's
font source `RZ::WCM()`, the window control manager (§1.3; its slot 5 maps a font ID to a font object). When the driver asks for textures to be
restored, the string cache is dropped and every glyph texture is rendered again.

## 1. `cSC4GlyphTextureManager` *[compat]*

### 1.1 Identity and state

| Item | Value |
|---|---|
| System service | ID `0xABEB5E37`, priority `0xFFE17B80` (−2,000,000), the `cRZSystemService` at `+0x04`; allocated (0x4C bytes) by `cSC4App::Init` (`0x0044C320`) and kept at the application's `+0x650` |
| `QueryInterface` (`0x005D7B70`) | answers **only** `0x0B54C519` (not even IID 1) *[quirk]* |
| Vtables | `+0x00` `0x00AA2AD4` (6 slots), `+0x04` `0x00AA2AA0` (system service: `Init` slot 8, `Shutdown` 9, `OnTick` 10), `+0x08` `0x00AA2A88`, `+0x20` `0x00AA2A78` (message target, `DoMessage` slot 3) |
| `+0x24` | reference count; `Release` (`0x005D7B50`) deletes at 0 |
| `+0x28` | initialised |
| `+0x2C`, `+0x30` | the glyph map: (character string, font ID) → glyph texture, and its size |
| `+0x38` | the string cache (a list, oldest first) |
| `+0x3C` | the cache budget in bytes (constructor 1,048,576) |
| `+0x40`, `+0x44`, `+0x48` | bytes cached, time of the last sweep, entries |

The other interfaces start with `this − n` thunks (`0x005D8980`…`0x005D89F0`); `+0x08` slot 3 runs the deleting
destructor (`0x005D89C0` → `0x005D8D40`, which runs `0x005D8900` and frees on bit 0).

### 1.1.1 Native manager construction, inherited slots and destruction (GT4)

The constructor is `0x005D8CA0–0x005D8D36` (151 bytes); `0x005D8900` is **destruction**, not
construction. The manager starts with reference count **0**, initialized=false, an empty glyph tree
and string list, budget 1048576, cached bytes=0, last-sweep time=0 and entry count=0
(`0x005D8CE2–0x005D8D2B`). The base service sets ID `0xABEB5E37`, both initial priority words to
`0xFFE17B80`, and running=false (`0x0091DE23–0x0091DE41`). Manager AddRef/Release operate on the
complete object's `+0x24` modulo 2^32; Release at zero wraps to 0xFFFFFFFF; decrement to zero invokes
the auxiliary deleting destructor with flags 1 (`0x005D7B40–0x005D7B66`).

For the +4 system-service receiver, offsets in the following table are relative to that receiver.
Names attached to shared bodies in the export do not determine these methods' semantics.

| Slot | Body | Native behavior |
|---|---|---|
| 3 | `0x009CEDE1` | Return u32 at +0x0C (service ID). |
| 4 | `0x00894F6D` | Store argument word at +0x0C; no defined boolean result. |
| 5 | `0x0099BE66` | Return word at +0x10 (priority). |
| 6 | `0x00932B83` | Return byte at +0x18 (running). |
| 7 | `0x0091DD91` | Store argument low byte at +0x18; no normalization to 0/1. |
| 11 | `0x0091559A` | Return true, discard one word, no mutation. |
| 12 | `0x009D4714` | Return word at +0x14 (second priority word). |

For the +8 auxiliary receiver, slot 4 (`0x0090D981`) decrements its unsigned word at +4 only when
positive, returning that word; zero saturates and nothing is deleted. Slot 5 (`0x009D7E63`) reads the
same word. This is a **different** counter from manager +0x24. The +4/+8 AddRef thunks at `0x0080BCD0`
and `0x0080BD00` subtract 4/8 then call manager AddRef. Message-target QI/AddRef/Release at
`0x005D89D0/0x005D89E0/0x005D89F0` subtract 0x20 and delegate to the manager's primary entries.

Destruction (`0x005D8900–0x005D8971`) restores the four vtables, frees string-cache list nodes and
sentinel, destroys glyph-tree key strings and nodes and its sentinel, then destroys the +4 base
service. The list helper `0x005650A0–0x005650DD` frees node storage without releasing texture-info
pointers; tree cleanup `0x005D82E0–0x005D8329` destroys key strings without releasing mapped glyph
pointers. The destructor **does not call Shutdown**, clear glyph manager links, unsubscribe, or
release cached string-info entries. Its deleting wrapper (`0x005D8D40–0x005D8D5D`) always performs
that cleanup, frees the complete allocation iff flags bit 0 is set, and returns the object address.
Shutdown (§1.2) is a separate required lifecycle call for its documented link/unsubscription work.

SetManager acquires the new nonnull manager **before** storing it and releasing the old manager;
equal pointers cause no reference operations (`0x005D7C49–0x005D7C66`). RemoveGlyph walks the map
by **glyph pointer identity**, not a supplied key; absent pointer/empty map returns without erasure
or Release. On a match it erases that node then calls the glyph's Release
(`0x005D8800–0x005D886F`). Thus removing an absent pointer does not consume a reference.

Inventory: 38 owner-named function bodies total **4047 bytes** (manager 25, glyph-info 8,
string-info 5); the three named vtable rows are data. The attribution count is reconciled in §3.4.
Info-object slot 0 and lifetime are established in §3.1.

Distinguishing vectors: a fresh manager has references=0 and sweep time=0; Release immediately
returns 0xFFFFFFFF. Auxiliary slot 4 at zero returns zero without deleting. Destruction with cache
entries frees container storage without texture-info Release calls; Shutdown has its separate glyph
link/release behavior. Replacing a manager link records new AddRef, store, old Release in that order.

### 1.2 `Init` (system-service slot 8, `0x005D7D50`) and `Shutdown` (slot 9, `0x005D8A00`)

`Init` runs once: initialised := true; budget := the render property `kStringTextureCacheSize` (KB, default 1024;
read as `+0x3B4` of the render-properties object `+0x18` of the application's settings, `cSC4App` slot 44) × 1024;
registers the service with the framework: first `AddSystemService` (framework slot 3, argument: the `+0x04`
system-service view, so the service is found under its ID `0xABEB5E37` with priority −2,000,000 and the framework
rejects a second service of that ID), then `AddToTick` (framework slot 9, `+0x24`, same argument), which puts the
service on the framework's tick list so that `OnTick` (§1.5) runs once per frame, in descending priority order, tying with
the simulator's priority, so the relative order depends on insertion (flammability.md, framework-gz-services.md §4.4); subscribes the
`+0x20` target to `kSGLMessage_RestoreTextures` `0x8ACAA95F`. Always true.

`Shutdown` (when initialised): for each glyph in the map, clear its manager link (releasing it) and release the map's
reference; empty the map; unsubscribe; initialised := false. String cache entries are not released here.

### 1.3 Glyph textures: `GetGlyphTexture(key, out)` (slot 3, `0x005D9120`)

The key is (character string, font ID); the map orders keys by the string comparison (string slot 12, case flag 1),
then by font ID (`0x005D7C00`).

- **Found:** add a reference, return it; true.
- **Else:** font := `RZ::WCM()` slot 5 (font ID); none → false. `RZ::WCM()` (`0x00913C72`) returns the window control
  manager service (`cIGZWinCtrlMgr`, interface `0x22C2EB1F` of service `0xC2C2EB0F`), cached after the first call in
  the global `0x00B628C4` (framework-misc2.md §1); its slot 5 (`SystemFont`, `0x00997B55`) is a plain lookup of the
  font ID in the manager's font table (widget-ctrlmgr.md): the stored font object, or null when the ID is not
  registered (there is no fallback to the default font here, unlike the manager's internal helper). A new glyph texture (§2) is rendered from the
  character with that font, the source rectangle starting as (0, 0, 1, 1); a failed render → false. On success
  it records the manager, the character, the texture size and the glyph size (both 16-bit), and the UV rectangle
  as the rendered extents divided by the texture size; it enters the map (holding one reference) and the caller gets
  a second; true.

`StringToGlyphTextureArray(key, text, out vector)` (slot 4, `0x005D8570`) splits the text into characters
(`GetCharLength` in the framework code page) and appends, for each, the result of slot 3 for (character, the key's
font ID); a failed character is skipped.

### 1.4 String textures

`GetCachedStringTexture(font ID, text, out)` (slot 5, `0x005D8150`) never renders: it makes a string texture (§3)
holding the manager, the text and the font (`RZ::WCM()` slot 5), sized `P(width + 2)` × `P(height + 2)` from the
font's text width (font slot 48) and height (font slot 37), P the next power of two (`0x0080A700`;
`0x005D7B90`), and returns it with one reference; true.

The texture is made when the string texture is asked for its texture (§3 slot 4 → `0x005D7F00`):

- **Cached:** the entry moves to the back (newest), its used flag is set, and its binding's first texture handle is
  returned.
- **Else:** a miss counter (`0x00B23040`) is incremented, the text is rendered (`nS3D::CreateTextureFromText`); a
  failed render returns 0. Else the cache adds a reference to the string texture and appends an entry (binding,
  string texture, w × h × 4 bytes, used); bytes and entries grow; the first handle is returned.

### 1.5 The sweep (`OnTick(t)`, system-service slot 10, `0x005D7E20`)

When t is below the last sweep time, the last time := t. When t − last > 50 (ms): last := t, and if the cache holds
more bytes B than the budget L:

- target := B − 0.1 L when B / L > 1.1, else L (so a large excess is cut by a tenth of the budget per sweep);
- walk the cache from the front (oldest) while B > target: an entry whose used flag is clear is dropped (binding and
  string texture released, bytes and entries reduced); one whose flag is set gets it cleared and stays.

### 1.6 Restore (`DoMessage`, `+0x20` slot 3, `0x005D8330`)

On `0x8ACAA95F`: drop every string cache entry (bindings and string textures released; bytes and entries 0). Then,
when the texture binding factory exists (global `0x00B43CD4`, set by `SL::OnAppInit1` from service `0x1AC0E11A`),
render every glyph of the map again with its font into its texture slot, releasing the old binding first. Always
true.

## 2. `cSC4GlyphTextureInfo` (glyph texture, 0x3C bytes) *[compat]*

Vtable `0x00AA2A3C`; constructor `0x005D8030`. Fields: the character (`+0x04`), the texture size (`+0x18`, `+0x1A`),
the glyph size (`+0x1C`, `+0x1E`; *[inferred naming]*), the UV rectangle (`+0x20`…`+0x2C`), the binding (`+0x30`), the reference count
(`+0x34`), the manager (`+0x38`, counted; `0x005D7C40` sets it).

| Slot | VA | Behaviour |
|---|---|---|
| 1, 7 | shared bodies | AddRef (`+0x34`), the binding |
| 2 | `0x005D8870` | Release: when the count drops to 1 and the manager is set, the manager removes the glyph from its map (`0x005D8800`) and releases it, which deletes it; at 0, delete; returns the count (0 after a delete) |
| 3 | `0x005D8080` | copy the character into the given string (string slot 13) |
| 4, 5 | `0x005D80A0`, `0x005D80C0` | texture size, glyph size (two 32-bit values) |
| 6 | `0x005D7C70` | the UV rectangle (4 floats) |
| 8 | `0x005D8240` | deleting destructor: releases the manager and the binding |

So a mapped glyph lives exactly as long as someone besides the map holds it. The transition to count 1 without
a manager also deletes it (`0x005D887C–0x005D8894`); it does not leave a standalone reference alive.

## 3. String texture (0x28 bytes, vtable `0x00AA2A60`) *[compat]*

Constructor `0x005D80E0`. Fields: width and height (`+0x04`, `+0x06`, 16-bit), the reference count (`+0x08`), the
manager (`+0x0C`, counted), the text (`+0x10`), the font (`+0x24`, counted).

| Slot | VA | Behaviour |
|---|---|---|
| 1 | shared body | AddRef (`+0x08`) |
| 2 | `0x005D7C90` | Release: at 1 with a manager, retain the object but return **0**; at 1 without a manager, or 0, delete and return 0; else the count |
| 3 | `0x005D8130` | width and height |
| 4 | `0x005D8020` | the texture handle, through the manager (§1.4) |
| 5 | `0x005D8290` | deleting destructor: releases the font, the text and the manager |

### 3.1 Complete glyph-info and string-info slot/lifetime contracts (GT4, GT6)

Native vtable data at `0x00AA2A3C` (9 words) and `0x00AA2A60` (6 words) establishes the complete virtual
surfaces in §§2–3. Neither object has an additional private QueryInterface implementation: both slot 0 words
are `0x0062C980`, returning false without inspecting either argument or writing the output. This includes
IID 1. Failure leaves a caller's prefilled output untouched.

Glyph slot 1 is `0x00459530` (increment and return the 32-bit word at +0x34); string slot 1 is `0x004BDF00`
(the same operation at +0x08). Both wrap. Release decrements modulo 2^32 and uses unsigned comparisons:
releasing count 0 returns `0xFFFFFFFF` without destruction. Glyph slot 7 is `0x006AAC50`, a plain load of
the binding at +0x30, with **no AddRef**. Size getters zero-extend the stored 16-bit fields to their two
32-bit outputs; no new metric calculation occurs. String slot 4 delegates to manager `GetStringTexture`
with this string-info object; it needs a live manager, not a standalone renderer.

The glyph constructor (`0x005D8030–0x005D8074`) constructs an empty framework string, sets binding,
reference count and manager to zero, and leaves sizes and UV words **unwritten**. The string constructor
(`0x005D80E0–0x005D8124`) constructs an empty string, sets reference count, manager and font to zero,
and leaves the two size words unwritten. Do not turn those unspecified fields into promised zero defaults.

Glyph `SetManager` (`0x005D7C40–0x005D7C6E`) is a no-op for equal pointers; otherwise AddRef the nonnull
new manager, assign the link, then Release the nonnull old manager. String creation assigns its manager/font
links in that same order (`0x005D81A2–0x005D81F1`). Glyph removal (`0x005D8800–0x005D886F`) scans map
**values by object identity**, not a reconstructed key. If absent it neither erases nor releases anything;
if found it erases that node first and then calls the object's Release.

Glyph destruction (`0x005D8240`) releases manager, then binding, then destroys the character string.
String destruction (`0x005D8290`) releases font, destroys text, then releases manager. Each frees its own
allocation only if destructor flags bit 0 is set; other flag bits have no effect. These and the existing
nonvirtual `SetManager` account for each standalone info object's named bodies and folded slot bodies;
they do not establish manager-owner completion.

### 3.2 Native numeric/output boundaries (GT3, GT5–GT6)

- Font metrics and `CalcStringTextureSize` outputs are 32-bit words. Add 2 with wrapping arithmetic before
  calling `P` (`0x005D7BD5–0x005D7BF9`); use the **signed**, potentially nonterminating P from
  texture-bindings §1.11. Store only the low 16 bits in string info (`0x005D8211–0x005D8223`). For example,
  metric 65534 rounds to 65536 then stores **0**; metric `0xFFFFFFFE` plus 2 becomes 0 and stores **1**.
- Glyph texture sizes likewise narrow after rendering. UV division uses the original **signed 32-bit**
  renderer dimensions (`FILD`), not those narrowed words. For precision, let r0,r1,r2,r3 be the four float32
  words in the renderer's rectangle output, in memory order. Glyph width/height are the low 16 bits of the
  truncations toward zero of **r0−r2** and **r1−r3**, respectively. The four stored UV words are
  **r2/W, r3/H, r0/W, r1/H**, each rounded on float32 store. The subtraction/division is x87 arithmetic;
  there is no intermediate float32 store before integer truncation (`0x005D926E–0x005D92CE`). This raw
  ordering is normative; do not substitute a conventional rectangle layout based on field names.
- Cached byte cost uses the **stored unsigned 16-bit string dimensions**, multiplied together and then
  shifted left 2, keeping the low 32 bits. It does not use renderer-result dimensions
  (`0x005D7F2A–0x005D7F4A`). Byte/count updates and budget KB×1024 wrap at 32 bits.
- Sweep compares B and L as **signed** words. It first converts B to float32, then divides that rounded B
  by signed L in x87 arithmetic. The threshold is float32 bits `0x3F8CCCCD` (1.1f), and the decrement factor
  is float32 bits `0x3DCCCCCD` (0.1f), not exact decimal doubles. If the ratio is greater, target is truncation
  toward zero of rounded-B minus L times 0.1f; otherwise target is L. The target arithmetic has no intervening
  float32 store (`0x005D7E47–0x005D7E89`; constants `0x00A8FAF4`, `0x00A8C950`). With L=0 and positive
  B, the unmasked normal IEEE division gives +infinity and the selected target is rounded-B: no forced
  zero-budget flush. E.g. B=100,L=0 gives target 100, so no eviction. Floating exception masks are external
  process state, not changed by this method.
- A failed glyph font lookup or render returns false **without writing out**
  (`0x005D9190–0x005D9197`, `0x005D9227–0x005D9258`). Success writes the object and AddRefs it.
  `GetCachedStringTexture` has no null-font failure branch before calling the metric helper: that helper
  dereferences the font. Its “always true” description applies to valid inputs, not a missing-font result.
- `StringToGlyphTextureArray` clears/releases existing vector entries before processing. Its loop tests
  signed remaining length > 0, so an empty input makes no glyph request. It trusts `GetCharLength`: zero
  repeats without advancing; a value greater than remaining length is used for the substring copy, then
  subtracted without validation. There is no stop-at-invalid-boundary policy (native body `0x005D8570`,
  character loop and advancement; these are malformed-provider cases, not safe new error APIs).
- Restore (§1.6) visits glyphs in map order, uses rectangle initial words (0,0,1,1), clears the binding slot
  before releasing its old value, and passes that slot as the renderer output. It does not branch on font
  lookup or renderer success and does not write glyph dimensions, texture dimensions or UVs. Thus the
  resulting binding is **whatever the renderer writes**, even on failure; no manager-level rollback or
  metadata refresh is promised (`0x005D8330`, binding replacement within `DoMessage`). If the factory global
  is null it clears the string cache but leaves glyphs untouched.

### 3.3 Manager lifecycle corrections and remaining inventory (GT4)

The constructor is `0x005D8CA0`, **not** `0x005D8900` (the latter is destruction). It sets manager reference
count to 0, initialized false, empty map/list, budget 1,048,576 and cached bytes/last sweep/entry count to 0
(`0x005D8CE2–0x005D8D2B`). The deleting wrapper `0x005D8D40` calls the destructor and frees iff bit 0.

Destruction does **not** call Shutdown. It destroys the cache nodes/sentinel, then map nodes/key strings and
map sentinel, then the system-service base (`0x005D8900–0x005D8971`). The cache-node helper
`0x005650A0–0x005650DD` frees nodes without releasing their stored binding/string pointers; the map helper
`0x005D82E0–0x005D8329` destroys keys/nodes without releasing glyph values. Thus destruction cannot be
specified as implicit Shutdown or as releasing all held object references.

Inherited system-service/auxiliary slot behavior is in §1.1.1. The 43-body inventory is reconciled below;
that numerical scope includes helpers and unrelated neighboring thunks and is not a virtual API list.

### 3.4 Body inventory and private tree insertion (GT4)

The brief's **43 / 4475** is reproduced by export-body attribution, as follows. Addresses and sizes
below are hexadecimal addresses / decimal bytes; groups on each row enumerate separate bodies.

| Attribution owner | Address / bytes |
|---|---|
| Manager: reference/query, key/size | `005D7B50`/23, `005D7B70`/31, `005D7B90`/111, `005D7C00`/56 |
| Manager: Init/tick/string | `005D7D50`/122, `005D7E20`/224, `005D7F00`/279, `005D8150`/236 |
| Manager: restore/array/remove/destruction | `005D8330`/568, `005D8570`/649, `005D8800`/112, `005D8900`/114 |
| Manager: adjusted entries | `005D8980`/8, `005D8990`/8, `005D89A0`/8, `005D89B0`/8, `005D89C0`/8, `005D89D0`/8, `005D89E0`/8, `005D89F0`/8 |
| Manager: Shutdown/tree helpers/construction/wrapper/glyph | `005D8A00`/234, `005D8AF0`/163, `005D8BA0`/241, `005D8CA0`/151, `005D8D40`/30, `005D9120`/498 |
| Manager: remote attribution entries | `0080BCD0`/8, `0080BCE0`/8, `0080BCF0`/8, `0080BD00`/8 |
| Glyph info | `005D7C40`/47, `005D7C70`/30, `005D8030`/69, `005D8080`/18, `005D80A0`/20, `005D80C0`/20, `005D8240`/78, `005D8870`/42 |
| String info | `005D7C90`/34, `005D8020`/12, `005D80E0`/69, `005D8130`/20, `005D8290`/78 |

Manager is 30/3938, glyph info 8/324, string info 5/213. Starting from the name-only 38/4047,
remove folded AddRef `005D7B40` (8 bytes), add the two tree helpers (404 bytes) and four remote
entries (32 bytes): **43/4475**. Shared QI/AddRef/getter bodies used by the info vtables and the
system-service slots remain callable entries without being exclusively attributed here.

Native jumps show that only `0080BCD0` and `0080BD00` are manager AddRef thunks (subtract 4/8,
jump to `005D7B40`). `0080BCE0` subtracts 4 and jumps to ordinance Release `0080C990`;
`0080BCF0` subtracts 8 and jumps to ordinance QueryInterface `0080C920`. The latter two enter the
count by **neighbor attribution**, not by manager-vtable membership. They must not be exposed as
new manager operations or substituted for its actual Release/QI entries.

The remaining two bodies are private map insertion, not unlisted texture APIs:

- `005D8BA0–005D8C90` performs unique-key insertion using string slot 12 with flag 1, then
  **unsigned** font-ID comparison. It returns the node plus a byte: 1 if inserted, 0 for an
  equivalent key. A duplicate returns the existing node without replacing its value or increasing
  node count (`005D8C33–005D8C90`).
- `005D8AF0–005D8B92` links a new node on the selected side, adjusts root/leftmost/rightmost
  sentinel links as necessary, rebalances, increments the 32-bit tree count and returns the node.
  Node creation (`005D88A0–005D88F7`, outside this attribution subtotal) allocates 44 bytes,
  copies key string/font ID and the glyph pointer; it performs **no glyph AddRef**. Allocation and
  string-storage failure behavior belongs to those primitives; this is not an invented fallback.

Distinguishing vectors: for equal strings, font ID `0x7FFFFFFF` precedes `0x80000000`. Inserting
the same string/ID twice through the private helper leaves one node, returns inserted bytes 1 then
0, preserves the first glyph pointer and makes no helper-level glyph-reference call. This does not
remove the separate references taken by public GetGlyphTexture.

## 4. Users

Text in the 3D view, through the application (`cSC4App::Init` creates the manager). The render property
`kStringTextureCacheSize` (`cSC4RenderProperties`, `0x007E2410`) sets the budget.

## 5. Quirks

1. `QueryInterface` answers only `0x0B54C519` (§1.1).
2. A large cache excess is trimmed by only 0.1 × budget per 50 ms sweep (§1.5).
3. `Shutdown` leaves the string cache alone (§1.2); the restore message empties it (§1.6).
4. A string texture without a manager is deleted at a count of 1 (§3).

## 6. Open questions

GT1 (what `RZ::WCM()` is) and GT2 (the framework slots 3 and 9 that `Init` calls) are answered in §1.3 and §1.2 and
archived in [questions/answered/glyph-textures.md](questions/answered/glyph-textures.md).

## 7. Conformance tests

1. **Sharing:** two `GetGlyphTexture` calls for ('A', font 7) return the same object; after both callers release
   it, the next call renders a new one.
2. **Size:** a string 61 × 13 pixels in its font gets a 64 × 16 string texture; 62 × 14 gets 64 × 16; 63 × 15 gets
   128 × 32.
3. **Lazy render:** `GetCachedStringTexture` renders nothing; the first slot 4 call renders once, the second hits
   the cache.
4. **Budget:** with budget 1,048,576 and 1,200,000 bytes cached (ratio 1.14 > 1.1) the sweep's target is
   1,095,142.4 → 1,095,142; with 1,100,000 bytes the target is the budget.
5. **Second chance:** two entries, both used, over budget: the first sweep clears both flags and drops nothing; the
   next drops the oldest first.
6. **Sweep rate:** ticks at 0, 30 and 60 ms sweep at 60 only (60 − 0 > 50); a tick at 40 after one at 100 resets the
   last time to 40.
7. **Restore:** after `0x8ACAA95F` the cache is empty (0 bytes, 0 entries) and the next slot 4 call renders again.
