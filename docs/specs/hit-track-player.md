# cHitTrackPlayer

Status: **partial owner specification**, 2026-10-22, lane `re-hittrack4`.
Target: Windows GOG SimCity 4 Deluxe 1.1.641. Address citations refer to that image.
This document owns only the track player. It does not extend the contracts of
cHitMan, cHitChannel, the mixer, or ambience classes.

## 1. Scope and evidence

The ownership audit reproduced on this lane yields **110 bodies / 16012 bytes**.
The inventory below includes named, vtable, reach, neighbour, and compiler-funclet
ownership; it is not just a list of exported method names. Folded bodies assigned
to another owner remain external boundaries even when used in these vtables.
A row marked open is not specified by the presence of its name or address here.
No complete-owner credit is claimed while any row remains open.

Evidence: `ghidra/export/windows/{functions.tsv,function-details.tsv,instructions.asm}`,
`re/names_windows.tsv`, `docs/abi/vtables.tsv`, and focused local pseudocode.
Inventory is reproducible with the existing coverage attribution rules (names,
vtable targets, propagation and neighbours), not a newly invented contiguous range.
Numeric and adjusted-view contracts below use instruction bytes as authority.

## 2. Identity and COM views

Let **P** be the primary object pointer. Successful QueryInterface writes the
following pointer, calls primary AddRef exactly once, and returns true in AL.
Unsupported IDs return false without writing the output pointer or incrementing
the reference count. The method does not validate the output pointer. The
success/failure return is a byte boolean; upper EAX bits are not a second status.
Evidence: `0x0093A810–0x0093A896`.

| IID | Returned pointer | Installed table | Role established here |
|---|---|---|---|
| `0x00000001`, `0xBB1BE367` | P | `0x00AD3E48` | Unknown identity / track-player primary view |
| `0xFD584467` | P+4 | `0x00AD3E24` | Register/property view |
| `0x306077D2` | P+8 | `0x00AD3DE0` | Spatial view |
| `0x206077D2` | P+12 | `0x00AD3D48` | Sound data-source-holder view |
| `0xAA11A64F` | P+16 | `0x00AD3D38` | Callback view |
| `0x5B34F0AF` | P+20 | `0x00AD3D28` | Update target |
| `0xDD583E2D` | P+24 | `0x00AD3D08` | Multiple sources |
| `0xE9D00A9A` | P+28 | `0x00AD3CF0` | Attached reference view |

The constructor installs these tables at `0x0093E933–0x0093E963`.
These views share one identity and reference count; they are not separately
allocated objects. Interface-relative field offsets in pseudocode must first be
converted to P-relative offsets. The sound view's SetID, for example, writes
P+0x60 rather than P+0x54.

Primary AddRef increments the 32-bit counter at P+0x50 and returns its new value
(`0x0093A897`). Release compares that counter **unsigned** with one. For values
0 or 1 it invokes primary slot 20 with deleting flag 1 and returns zero; otherwise
it decrements and returns the new counter (`0x0093A89E–0x0093A8B4`). In particular,
a just-created zero-reference object is deletable by Release; do not underflow it
first. No atomic operation appears in either reference-count body.

AsIGZSndMultipleSources returns P+24 and AsIGZSndRegisterSet returns P+4, with a
null input mapped to null, and neither increments the count
(`0x0093E43D`, `0x009CFA36`).

## 3. Register-ID validation

CheckedRegId treats its input as a **signed 32-bit** integer. Values greater than
74 call HitMan's diagnostic slot +0x74 with this player and the text
`Illegal Register Id`, then return register 1. All other values, including negative
values and 74, pass through unchanged. There is no lower-bound check in this
helper. This does not by itself establish that negative registers are safe in
all downstream methods. Evidence: compare/JLE and fallback at
`0x0093A7EF–0x0093A80D`.

## 4. Conformance cases

1. Seed a QI output with a sentinel, ask for an unsupported IID, and require false,
   unchanged sentinel and unchanged reference count. Ask for each supported IID
   and require the exact adjusted pointer and one AddRef.
2. Release counts 0, 1 and 2: the first two enter the deleting slot; the last
   returns 1 without destruction. Treat the count comparison as unsigned.
3. CheckedRegId(-1), CheckedRegId(74), CheckedRegId(75) yield -1, 74, 1;
   only the last emits the diagnostic. An unsigned clamp fails this case.

## 5. Owned-body inventory

Byte sizes are exported body-byte counts, not distance to the next entry. The
attribution reason preserves how unnamed and folded-adjacent bodies entered the
audit. Open rows are tracked in the questions file as implementation gaps.

| Entry | Bytes | Name / local role | Attribution | Contract |
|---|---:|---|---|---|
| `0x0087B103` | 8 | `cHitTrackPlayer::GetRegisterFloat` | name-low | open |
| `0x0093A7EB` | 37 | `cHitTrackPlayer::CheckedRegId` | name-high | §3 |
| `0x0093A810` | 135 | `cHitTrackPlayer::QueryInterface` | name-high | §2 |
| `0x0093A897` | 7 | `cHitTrackPlayer::AddRef` | name-low | §2 |
| `0x0093A89E` | 23 | `cHitTrackPlayer::Release` | name-high | §2 |
| `0x0093A8B5` | 5 | `cHitTrackPlayer::Update` | name-low | open |
| `0x0093A8BA` | 17 | `cHitTrackPlayer::Play` | name-high | open |
| `0x0093A8CB` | 46 | unnamed | reach | open |
| `0x0093A8F9` | 83 | unnamed | reach | open |
| `0x0093A959` | 35 | `cHitTrackPlayer::CurrentPlayPosition` | name-high | open |
| `0x0093A97C` | 39 | unnamed | neighbour | open |
| `0x0093A9A3` | 25 | unnamed | neighbour | open |
| `0x0093A9BC` | 82 | unnamed | neighbour | open |
| `0x0093AA0E` | 97 | `cHitTrackPlayer::SetPosition` | name-high | open |
| `0x0093AA6F` | 8 | `cHitTrackPlayer::SetErrorCondition` | name-low | open |
| `0x0093AA77` | 7 | `cHitTrackPlayer::GetErrorCondition` | name-low | open |
| `0x0093AA7E` | 49 | unnamed | reach | open |
| `0x0093AAB5` | 29 | unnamed | reach | open |
| `0x0093AAD2` | 40 | unnamed | neighbour | open |
| `0x0093AAFA` | 47 | unnamed | reach | open |
| `0x0093AB4B` | 137 | unnamed | reach | open |
| `0x0093ABD4` | 137 | unnamed | reach | open |
| `0x0093AC5D` | 42 | `cHitTrackPlayer::Shutdown` | name-high | open |
| `0x0093AC87` | 22 | `cHitTrackPlayer::GetTrackDef` | name-high | open |
| `0x0093AC9D` | 20 | `cHitTrackPlayer::SetHitList` | name-high | open |
| `0x0093ACB1` | 37 | `cHitTrackPlayer::GetHitList` | name-high | open |
| `0x0093ACD6` | 212 | `cHitTrackPlayer::CompletionTime` | name-high | open |
| `0x0093ADAA` | 16 | `cHitTrackPlayer::Reset` | name-high | open |
| `0x0093ADBA` | 35 | `cHitTrackPlayer::SetRegisterVal` | name-high | open |
| `0x0093ADDD` | 80 | unnamed | neighbour | open |
| `0x0093AE2D` | 75 | unnamed | neighbour | open |
| `0x0093AE78` | 20 | unnamed | reach | open |
| `0x0093AE8C` | 37 | `cHitTrackPlayer::UpdateVolPan` | name-medium | open |
| `0x0093AF0D` | 52 | `cHitTrackPlayer::SetFrequency` | name-high | open |
| `0x0093AF41` | 28 | `cHitTrackPlayer::Load` | name-high | open |
| `0x0093AF5D` | 22 | `cHitTrackPlayer::Unload` | name-high | open |
| `0x0093AF73` | 11 | unnamed | neighbour | open |
| `0x0093AF7E` | 44 | unnamed | neighbour | open |
| `0x0093AFAA` | 29 | unnamed | reach | open |
| `0x0093AFC7` | 27 | unnamed | reach | open |
| `0x0093AFE2` | 12 | unnamed | neighbour | open |
| `0x0093AFEE` | 48 | `cHitTrackPlayer::IsPlaying` | name-high | open |
| `0x0093B01E` | 109 | `cHitTrackPlayer::Pause` | name-high | open |
| `0x0093B08B` | 125 | `cHitTrackPlayer::Unpause` | name-high | open |
| `0x0093B108` | 248 | `cHitTrackPlayer::Stop` | name-high | open |
| `0x0093B200` | 104 | unnamed | neighbour | open |
| `0x0093B268` | 263 | unnamed | neighbour | open |
| `0x0093B36F` | 274 | unnamed | neighbour | open |
| `0x0093B481` | 135 | unnamed | neighbour | open |
| `0x0093B508` | 454 | `cHitTrackPlayer::UpdatePitch` | name-high | open |
| `0x0093B6CE` | 32 | unnamed | neighbour | open |
| `0x0093B6EE` | 136 | unnamed | neighbour | open |
| `0x0093B776` | 27 | unnamed | neighbour | open |
| `0x0093B791` | 541 | `cHitTrackPlayer::PlayPause` | name-high | open |
| `0x0093B9AE` | 321 | `cHitTrackPlayer::UpdatePatch` | name-high | open |
| `0x0093BB39` | 51 | unnamed | reach | open |
| `0x0093BB6C` | 132 | unnamed | reach | open |
| `0x0093BBF0` | 278 | `cHitTrackPlayer::Kill` | name-high | open |
| `0x0093BD06` | 745 | `cHitTrackPlayer::HandleNotifySetPropertyValue` | name-high | open |
| `0x0093BFEF` | 82 | `cHitTrackPlayer::NoteOn` | name-high | open |
| `0x0093C041` | 35 | unnamed | neighbour | open |
| `0x0093C064` | 53 | unnamed | neighbour | open |
| `0x0093C099` | 41 | unnamed | neighbour | open |
| `0x0093C0C2` | 42 | unnamed | reach | open |
| `0x0093C0EC` | 317 | `cHitTrackPlayer::StartSndDependent` | name-high | open |
| `0x0093C229` | 6754 | `cHitTrackPlayer::DoCommand` | name-high | open |
| `0x0093DE80` | 25 | unnamed | reach | open |
| `0x0093DE99` | 179 | unnamed | reach | open |
| `0x0093DF4C` | 403 | `cHitTrackPlayer::HandleTimerCallback` | name-medium | open |
| `0x0093E0DF` | 26 | `lib::Catch@0093e0df` | neighbour | open |
| `0x0093E0FC` | 74 | `cHitTrackPlayer::NoteOff` | name-medium | open |
| `0x0093E146` | 402 | `cHitTrackPlayer::Play` | name-high | open |
| `0x0093E2D8` | 43 | `cHitTrackPlayer::Step` | name-medium | open |
| `0x0093E303` | 54 | unnamed | neighbour | open |
| `0x0093E339` | 22 | unnamed | neighbour | open |
| `0x0093E34F` | 238 | unnamed | reach | open |
| `0x0093E43D` | 12 | `cHitTrackPlayer::AsIGZSndMultipleSources` | name-low | §2 |
| `0x0093E449` | 10 | `cHitTrackPlayer::SetID` | name-low | open |
| `0x0093E453` | 11 | `cHitTrackPlayer::GetSoundPriority` | name-low | open |
| `0x0093E45E` | 19 | `cHitTrackPlayer::SetSoundPriority` | name-high | open |
| `0x0093E471` | 11 | `cHitTrackPlayer::GetOriginalFrequency` | name-low | open |
| `0x0093E482` | 9 | `cHitTrackPlayer::IsLoaded` | name-low | open |
| `0x0093E48B` | 9 | `cHitTrackPlayer::IsPaused` | name-low | open |
| `0x0093E494` | 11 | `cHitTrackPlayer::GetVolume` | name-low | open |
| `0x0093E49F` | 19 | `cHitTrackPlayer::SetVolume` | name-high | open |
| `0x0093E4B2` | 19 | `cHitTrackPlayer::HasRegisterVal` | name-high | open |
| `0x0093E4C5` | 11 | `cHitTrackPlayer::GetPan` | name-low | open |
| `0x0093E4D0` | 19 | `cHitTrackPlayer::SetPan` | name-high | open |
| `0x0093E4E3` | 19 | `cHitTrackPlayer::ClearRegisterVal` | name-high | open |
| `0x0093E4F6` | 19 | `cHitTrackPlayer::GetRegisterVal` | name-high | open |
| `0x0093E509` | 17 | `cHitTrackPlayer::IsFading` | name-high | open |
| `0x0093E51A` | 11 | `cHitTrackPlayer::GetFrequency` | name-low | open |
| `0x0093E525` | 17 | `cHitTrackPlayer::SetName` | name-high | open |
| `0x0093E582` | 8 | unnamed | vtable | open |
| `0x0093E58A` | 8 | unnamed | vtable | open |
| `0x0093E592` | 8 | unnamed | vtable | open |
| `0x0093E59A` | 8 | unnamed | neighbour | open |
| `0x0093E5A2` | 8 | unnamed | neighbour | open |
| `0x0093E5AA` | 8 | unnamed | neighbour | open |
| `0x0093E5B2` | 8 | unnamed | neighbour | open |
| `0x0093E5BA` | 8 | unnamed | neighbour | open |
| `0x0093E5C2` | 8 | unnamed | neighbour | open |
| `0x0093E5CA` | 8 | `__ZThn20_N15cHitTrackPlayer6UpdateEl` | name-low | open |
| `0x0093E882` | 28 | `cHitTrackPlayer::~cHitTrackPlayer` | name-high | open |
| `0x0093E89E` | 62 | unnamed | reach | open |
| `0x0093E8DC` | 24 | unnamed | reach | open |
| `0x0093E8F4` | 429 | `cHitTrackPlayer::cHitTrackPlayer` | name-medium | open |
| `0x0093EAA1` | 404 | `cHitTrackPlayer::SetTrackDef` | name-high | open |
| `0x00949B47` | 3 | `cHitTrackPlayer::GetUseCount` | name-low | open |
| `0x009CFA36` | 12 | `cHitTrackPlayer::AsIGZSndRegisterSet` | name-low | §2 |

