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
font source `RZ::WCM()` (slot 5 maps a font ID to a font object *[inferred]*). When the driver asks for textures to be
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

### 1.2 `Init` (system-service slot 8, `0x005D7D50`) and `Shutdown` (slot 9, `0x005D8A00`)

`Init` runs once: initialised := true; budget := the render property `kStringTextureCacheSize` (KB, default 1024;
read as `+0x3B4` of the render-properties object `+0x18` of the application's settings, `cSC4App` slot 44) × 1024;
registers the service with the framework (framework slots 3 and 9 *[inferred: the tick list]*); subscribes the
`+0x20` target to `kSGLMessage_RestoreTextures` `0x8ACAA95F`. Always true.

`Shutdown` (when initialised): for each glyph in the map, clear its manager link (releasing it) and release the map's
reference; empty the map; unsubscribe; initialised := false. String cache entries are not released here.

### 1.3 Glyph textures: `GetGlyphTexture(key, out)` (slot 3, `0x005D9120`)

The key is (character string, font ID); the map orders keys by the string comparison (string slot 12, case flag 1),
then by font ID (`0x005D7C00`).

- **Found:** add a reference, return it; true.
- **Else:** font := `RZ::WCM()` slot 5 (font ID); none → false. A new glyph texture (§2) is rendered from the
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

So a glyph lives exactly as long as someone besides the map holds it.

## 3. String texture (0x28 bytes, vtable `0x00AA2A60`) *[compat]*

Constructor `0x005D80E0`. Fields: width and height (`+0x04`, `+0x06`, 16-bit), the reference count (`+0x08`), the
manager (`+0x0C`, counted), the text (`+0x10`), the font (`+0x24`, counted).

| Slot | VA | Behaviour |
|---|---|---|
| 1 | shared body | AddRef (`+0x08`) |
| 2 | `0x005D7C90` | Release: at 1 with a manager, nothing more (the cache holds it); at 1 without a manager, or 0, delete *[quirk: a last reference without a manager is deleted at 1]*; else the count |
| 3 | `0x005D8130` | width and height |
| 4 | `0x005D8020` | the texture handle, through the manager (§1.4) |
| 5 | `0x005D8290` | deleting destructor: releases the font, the text and the manager |

## 4. Users

Text in the 3D view, through the application (`cSC4App::Init` creates the manager). The render property
`kStringTextureCacheSize` (`cSC4RenderProperties`, `0x007E2410`) sets the budget.

## 5. Quirks

1. `QueryInterface` answers only `0x0B54C519` (§1.1).
2. A large cache excess is trimmed by only 0.1 × budget per 50 ms sweep (§1.5).
3. `Shutdown` leaves the string cache alone (§1.2); the restore message empties it (§1.6).
4. A string texture without a manager is deleted at a count of 1 (§3).

## 6. Open questions

See QUESTIONS.md GT1–GT2: what `RZ::WCM()` is, and the framework slots 3 and 9 `Init` calls.

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
