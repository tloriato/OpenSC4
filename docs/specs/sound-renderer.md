# The DirectSound renderer (`GZSndSysMacCOMDirector` classes) (behaviour spec)

Status: M6 behaviour spec, written 2026-10-22 (re-sndrender lane, outside contributor). Facts were read on the
Windows executable `bin/SimCity4.exe` (1.1.641, image base `0x00400000`) from the Ghidra export of the
`GZSndSysMacCOMDirector` owner (`0x008747B0`–`0x008792D0`); the unit conversions were checked on the instruction
and data bytes and carry addresses. Implementers work from this spec, the LGPL gzcom-dll SDK headers, the public
DirectSound API and general knowledge only, never from decompiled code.

Tags: *[compat]* observable, must match; *[inferred]* consistent with the evidence, not proven; *[quirk]* original
behaviour reproduced as is; *[as read]* read from the export without a byte check.

Scope. The back end that turns framework sounds into DirectSound buffers: the renderer service, the listener
service, the 2D and 3D sound objects and the 3D-buffer helper registered by `GZSndSysMacCOMDirector` (added by
`cSC4AudioCOMDirector::OnStart`, [audio.md](audio.md) §1.1). The framework above it is
[sound-system.md](sound-system.md) (interface contract; its §8 also lists this director) and
[sound-framework.md](sound-framework.md) (byte-level detail); the EA SNDSYS/AEMS library used for AEMS banks is not in
this owner.

---

## 1. Identity *[compat]*

`GZSndSysMacCOMDirector` (constructor `0x0087484B`, director ID `0xE9D3A016`) registers:

| CLSID | Factory | Object | Size | Returned |
|---|---|---|---|---|
| `0xFDED7F31` | `0x008747F5` | the 2D sound (`0x00877672`) | 0xD8 | base |
| `0xFDED7F34` | `0x0087480E` | the 3D sound (`0x00878D9F`) | 0x100 | `+0xD8` |
| `0xE9D28415` | `0x008747B0` | the renderer (`0x0087517F`) | 0xA0 | `+0x08` |
| `0x89D3D983` | `0x008747D4` | the listener (`0x00875F76`) | 0x54 | `+0x04` |
| `0x1C5F21FE` | `0x00874835` | the 3D listener buffer helper (`0x00878E1F`) | 0x14 | base |

`OnStart` (`0x008748BF`): adds the director to the framework, then creates the renderer and the listener under IID
`0x287FB697` and adds both as system services. The 2D and 3D CLSIDs are the "default data-source player" classes the
sound manager is told about by the Hit INI ([hit-music.md](hit-music.md) §2 step 1; [sound-framework.md](sound-framework.md)
§3.1).

## 2. The renderer (`0xE9D28415`)

QI (`0x008749C4`): `1` and `0x49D28666` → base, `0x287FB697` → `+4`, `0xC9D25F29` → `+8`. Defaults (constructor
`0x0087517F`): output format **16-bit, 22050 Hz, 2 channels** (`+0x24`, `+0x28`, `+0x2C`), a DirectSound caps block
(`+0x20`, 0x60 bytes), a device list (`+0x94`, 0x2C-byte entries), a lock, counters of live sounds (`+0x7C`) and 3D
sounds (`+0x78`) (debug text `"Sound: %u, 3D: %u"`).

`Init` (`0x008754C7`) *[compat]*:

1. take the main window from the framework (service `0x23EA70A1`, slot 5);
2. enumerate the devices (`DirectSoundEnumerateA`), keeping GUID, description and module of each;
3. create the **primary** buffer object (a 2D sound) with the PCM format above (block align and byte rate from the
   bits, rate and channels);
4. `DirectSoundCreate(default device)`; `GetCaps`, keeping three caps fields;
5. `SetCooperativeLevel(window, 3)`; on failure report the error (renderer slot 35) and try **2**, then **1**; all
   three failing ends with failure;
6. give the primary object the renderer, create its buffer and `Play(0, 0, DSBPLAY_LOOPING)`;
7. register the renderer with the sound system (`RegisterRenderer`, slot 35).

Any failure releases the primary object and the DirectSound object. Device choice (`0x00874F7C`): one device →
default; two whose first description begins with `Primary` (case-insensitive) → index 1; otherwise the first entry
without a GUID.

The primary buffer (`0x00876603`) is created with flags `DSBCAPS_PRIMARYBUFFER | DSBCAPS_CTRL3D` (`0x11`), falling
back to `DSBCAPS_PRIMARYBUFFER` alone, and given the output format (`SetFormat`).

## 3. The listener (`0x89D3D983`)

QI (`0x0087579A`): `1`/`0x49D28669` base, `0x287FB697` `+4`, `0x406077D2` `+0xC`, `0x5B30E7B3` `+0x10`. `Init`
(`0x00875B2E`): when the sound system runs, create the 3D helper (`0x1C5F21FE`, IID `0x406077D2`), keep it and its
`0x5B30E7B3` interface in two globals (`0x00B540A4`, `0x00B540A0`), and register the listener as a renderer. Every
listener call (position, orientation, velocity, the three factors, commit) is forwarded to the helper
(`0x00875A30`–`0x00875F18`), which applies it to DirectSound's listener.

## 4. The 2D sound (`0xFDED7F31`)

0xD8 bytes (constructor `0x00877672`): the framework sound (`cGZSnd`, [sound-framework.md](sound-framework.md) §4.6) on a
DirectSound buffer (static loading in §4.1, streaming allocation in §4.5). Fields: the data source `+0x18`, the buffer `+0x80`, the volume `+0x50`, the priority
`+0x60`, the buffer flags `+0xC4`, the renderer `+0xD0`, the use count `+0xC8`.

### 4.1 Loading (`0x00876504`)

Open the data source; read its size and its 20-byte format; create the buffer (§4.2); `Lock` the whole buffer, let
the data source fill it, `Unlock`, close the data source. A failure releases the buffer.

### 4.2 Buffer flags (`0x008766D8`) *[compat]*

`DSBUFFERDESC` with the sound's flags, size and format; a priority below **64** forces `DSBCAPS_LOCSOFTWARE` (and
clears `DSBCAPS_LOCHARDWARE`); 64 and above clears `DSBCAPS_LOCSOFTWARE`.

### 4.3 Unit conversions *[compat]*

- **Volume** (`0x00876894`): `v` clamped to 0–`0x400` and stored; `v' := trunc(sqrt(v / 1024) × 1024)`; with the
  renderer's master volume `m` (slot 28), `w := v' × m / 1024` (integer); DirectSound volume (`0x00875FFF`):
  `w = 0 → −10000`, `w = 1024 → 0`, else `w × 6000 / 1024 − 6000` (hundredths of dB, integer division). So full
  volume is 0 dB, anything above zero is between −60 dB and 0 dB along a square-root curve, and zero is −100 dB
  (`0x00AC2C10` = 1/1024, `0x00AC2C08` = 1024.0, `sqrt` at `0x009F1D04`).
- **Pan** (`0x008767C1`): `p` clamped to 0–`0x400`; DirectSound pan = `p × 20000 / 1024 − 10000` (`0x200` = centre).
  *[quirk]* The pan getter (`0x00876798`) returns DirectSound's value (−10000…10000) when a buffer exists and `0x200`
  otherwise.
- **Frequency** (`0x0087683C`): 0 is passed through (the buffer's own rate); other values are clamped to
  **1000–100000 Hz**.

### 4.4 Fades (`0x0087630C`)

`Fade(from, to, ms, async)`: synchronous — set `from`, then 10 steps of `(to − from) / 10`, each after busy-waiting
`ms / 10` ms; asynchronous — `step := (to − from) / (ms / 20)`; zero → set `to` at once; otherwise set `from`, keep
the target, mark the fade active and register the sound as a **fast** update target (20 ms) of the sound system
(`0x008760C3`, which also unregisters it once no fade, pan or pitch change is pending).

### 4.5 Other members *[as read]*

Play/stop/pause through the buffer; releasing the last reference stops and deletes
the sound (`0x00876097`).

**Streaming allocation level (SRN1).** *[compat]* The setter `0x00876238` belongs
to the interface at object `+8` (table `0x00AC2C54`, QI adjustment `0x00877842`).
It clamps an unsigned argument to 1–6 and stores it at interface `+0xB8`, hence
object `+0xC0`; the default is 1 (`0x00877672`). It scales streaming buffer
allocation in groups of six chunks, independently of voice priority (object
`+0x60`, §4.2). Streaming setup computes an initial chunk size of
`4 × (floor(sampleRate/8) mod 8388608)` bytes, multiplies it by `6 × level` with
32-bit wrap, then divides by `6 × level` to recover the chunk size. For a filename
ending in XA, a chunk not divisible by 4 is rounded up to a multiple of 4. The
allocated DirectSound buffer is the final chunk size times `6 × level`
(`0x00877D7A`, especially `0x00877EB9`–`0x00877EF3`; the 3D streaming path
`0x00878E5D` uses the same calculation). At 22050 Hz and level 1, this gives six
11024-byte chunks, 66144 bytes total; level 2 doubles that total. The setter
itself only stores the level; it does not reallocate an existing buffer.

**Remaining-time calculation (SRN3).** *[compat, quirk]* `0x00876255` is the
completion/remaining-time operation of the interface at object `+0x14`
(table `0x00AC2C18`, QI adjustment `0x0087788A`). For a playing, non-streaming
sound, subtract the DirectSound play cursor in bytes from the loaded byte count,
divide that remainder by **bits per sample**, multiply by 8000, then divide by
sample rate. Both divisions are unsigned integer divisions and multiplication
keeps the low 32 bits. The calculation omits channel count. While a streaming
sound plays it returns `0xFFFFFFFF`. When not playing it instead uses the full
loaded byte count in that formula and multiplies the result by another 1000
(with 32-bit wrap). Preserve the extra stopped-state factor and the division
order; this is not an elapsed playback-position getter. For 16000 loaded bytes,
16 bits/sample and 8000 Hz, the stopped result is 1000000; while playing with a
byte cursor of 8000 it is 500. A zero bits/sample or sample rate is unchecked.


## 5. The 3D sound (`0xFDED7F34`) and the 3D helper

The 3D sound is a 2D sound plus a DirectSound 3D buffer (QI `0x306077D2` at `+0xD8`). Distances (`0x0087845D`): a
level clamped to 1–10 picks `T[level]` from the table at `0x00B11DE0` (`T[1..10]` = 1, 10, 50, 100, 200, 500, 1000,
2000, 5000, 10000); minimum distance := `T / factor` (the listener's cached distance factor, `0x00B540A0` slot 11,
`0x00878CD9`) and maximum distance := that × 100 (`0x00A80AA0`). The minimum distance sent to DirectSound is raised
to 1.0 when below it (`0x008784D9`, the 3D buffer's `SetMinDistance`). A 0–127 value is scaled by 1/127
(`0x00AC2774`) before it reaches the renderer's auxiliary helper (`0x00878762`).
*[compat]* This path obtains the helper through renderer slot 12 and passes the
DirectSound 3D-buffer pointer and the scaled float to its slot 6. The helper body
`0x008792D0` queries a property-set interface and writes property 1 as four bytes
under the GUID at `0x00B11E30`; it is not a call to DirectSound's listener factor
setters. The acoustic meaning of that property remains unresolved (SRN2); the
0–127 input range is a caller convention, with no clamp in `0x00878762`.

## 6. Quirks *[quirk]*

1. The pan getter mixes DirectSound and framework units (§4.3).
2. Synchronous fades busy-wait on the calling thread (§4.4).
3. Low-priority sounds are always software-mixed (§4.2).

## 7. Open items

Questions are filed in [questions/sound-renderer.md](questions/sound-renderer.md) (prefix **SRN**).

## 8. Conformance tests

1. **Volume.** `SetVolume(0x400)` with master `0x400` gives 0; `0x100` gives `trunc(sqrt(0.25)×1024) = 512` → −3000;
   `0` gives −10000.
2. **Pan.** 0 → −10000, `0x200` → 0, `0x400` → 10000.
3. **Frequency.** 500 → 1000, 200000 → 100000, 0 → 0.
4. **Cooperative level.** When level 3 is refused the renderer tries 2, then 1.
5. **Buffer flags.** Priority 63 → `LOCSOFTWARE` set; priority 64 → `LOCSOFTWARE` cleared.
6. **Distances.** Level 4 with factor 1.0 → minimum 100, maximum 10000.
