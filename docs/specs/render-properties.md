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

### 1.2 Tables and entries

`+0x0C`, `+0x18`, `+0x24` are the boolean, integer and float tables (begin, end, capacity). An entry holds:

| Offset | Boolean entry (0x20 bytes) | Integer or float entry (0x28 bytes) |
|---|---|---|
| `+0x00` | index | index |
| `+0x04` | name (a string) | name |
| `+0x10` | short name (a string) | short name |
| `+0x1C` | value (byte); two more bytes, 0 | value |
| `+0x20`, `+0x24` | — | two more values: a range for some entries (§3) *[inferred: minimum, maximum]* |

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
- `cSC4View3DWin` keeps its own copy (§1.1) and re-posts render-properties changes (`0xC9F775BB`, graphics-formats.md
  §4.2).

## 5. Quirks

1. Four short names are ambiguous; the lower index wins (§3).
2. Get and set do not check the index; set ignores the range (§2).

## 6. Open questions

See [questions/render-properties.md](questions/render-properties.md) RP1–RP2: who reads the two extra values of an
entry, and what the two extra bytes of a boolean entry are for.

## 7. Conformance tests

1. **Counts:** slots 3, 4, 5 return 38, 24, 15.
2. **Defaults:** `GetInt(23)` = 1024, `GetFloat(2)` = 2.0, `GetBool(28)` = false, `GetBool(9)` = true.
3. **Find:** `FindBool("RenderShadows")` = `FindBool("rendershadows")` = 9; `FindBool("rs")` = 9;
   `FindBool("kRenderShadows")` = −1 (the `k` is not part of the name).
4. **Short names:** `FindInt("sd")` = 4; `FindFloat("pd")` = 3; `FindFloat("ParticleDamping")` = 4.
5. **Set:** `SetInt(23, 4096)` then `GetInt(23)` = 4096.
6. **Names:** slot 15 for 37 gives `doautozoomandrotationchanges`.
