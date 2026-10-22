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
P+0x3C (sound-view offset +0x30 plus its +12 adjustment).

Primary AddRef increments the 32-bit counter at P+0x50 and returns its new value
(`0x0093A897`). Release compares that counter **unsigned** with one. For values
0 or 1 it invokes primary slot 20 with deleting flag 1 and returns zero; otherwise
it decrements and returns the new counter (`0x0093A89E–0x0093A8B4`). In particular,
a just-created zero-reference object is deletable by Release; do not underflow it
first. No atomic operation appears in either reference-count body.

AsIGZSndMultipleSources returns P+24 and AsIGZSndRegisterSet returns P+4, with a
null input mapped to null, and neither increments the count
(`0x0093E43D`, `0x009CFA36`).

Owned COM adjustment thunks are exact tail transfers (no extra AddRef of their
own): `0x0093E582/0x0093E58A/0x0093E592` subtract 12 and jump to primary
QI/AddRef/Release, respectively; `0x0093E59A/0x0093E5A2/0x0093E5AA` subtract 16;
`0x0093E5B2/0x0093E5BA/0x0093E5C2` subtract 20. Other views' shared thunks are
not additional owned bodies in this audit.

## 3. Register-ID validation

CheckedRegId treats its input as a **signed 32-bit** integer. Values greater than
74 call HitMan's diagnostic slot +0x74 with this player and the text
`Illegal Register Id`, then return register 1. All other values, including negative
values and 74, pass through unchanged. There is no lower-bound check in this
helper. This does not by itself establish that negative registers are safe in
all downstream methods. Evidence: compare/JLE and fallback at
`0x0093A7EF–0x0093A80D`.

Two comparison helpers accept a receiver R and overwrite byte flags R+0x9C
(equal), R+0x9D (greater), R+0x9E (less), in that order. The integer helper
`0x0093A8CB` compares two signed 32-bit words (SETZ/SETG/SETL). The float helper
`0x0093A8F9` compares two binary32 inputs using x87; unordered comparisons make
all three flags false, and +0 and -0 compare equal. Neither returns a prescribed
boolean. These helpers do not themselves perform register lookup or clamp an ID.

## 4. Construction, destruction and state

Construction (`0x0093E8F4`, 429 bytes) installs the eight views in §2 and constructs
the name string at P+0x20. It explicitly zeros the following **32-bit** fields:
0x34, 0x38, 0x3C, 0x40, 0x44, 0x48, 0x4C, 0x50, 0x54, 0x5C, 0x64,
0x68, 0x70, 0x7C, 0x80, 0x84, 0x88, 0x8C, 0x90, 0x94,
0xA0, 0xA4, 0xA8, 0xAC, 0xB0 and 0xC0. It zeros the **byte** fields
0x58, 0x9C, 0x9D, 0x9E, 0xC4 and 0x100. It constructs list/container members
at 0x78 and 0x98 through `0x0093F7EB`, the container at 0xB4 through
`0x0093DE80`, and the channel subobject at 0xC8 through `0x0093FE12`.
It grows the initially empty reference vector at 0xA0 to ten null entries through
`0x0093E8DC(10)`; this is a size change, not merely reserved capacity.

It obtains sound-system, HitMan and sound-manager service references in that order,
retains them at P+0x90, P+0x8C and P+0x94, increments the global instance counter
at `0x00B633B4`, releases the three temporary service references in reverse order,
and returns P. This body contains no separate all-services-present failure return.
The service constructors and channel/container implementations are boundaries,
not evidence of the apparent C++ types assigned by the decompiler.
Evidence: `0x0093E96F–0x0093EAA0` (byte versus dword stores included).
Fields absent from this list must not be assumed zero solely from construction.

The deleting destructor (`0x0093E882`) calls the cleanup routine at `0x0093E60A`
with P, then frees P with GlobalHeapFree (`0x005E5620`) only if the low bit of its
one-word flag is set. It returns P and pops that flag. The cleanup routine's current
export name is `cHitChannel::~cHitChannel`; this does **not** establish inheritance.
Its internal resource teardown is an external contract gap in HT2.

The owned vector resize helper `0x0093E89E` takes a vector V (begin/end/capacity
words), requested unsigned size n and a reference-value address. Existing size is
the arithmetic-shifted pointer difference `(end-begin)/4`. If n is smaller,
delegate erase of `[begin+4*n,end)` to `0x0098155B`; otherwise delegate insertion
of n-size copies at end to `0x0093E34F`, including the zero-copy case. The wrapper
`0x0093E8DC` supplies the address of a local null reference. Its constructor call
with n=10 consequently creates ten null slots. Allocation, insertion and erase
helpers still have their own open inventory entries; no allocation-failure policy
is inferred here. Evidence: unsigned JNC at `0x0093E8B0–0x0093E8B2`.

Shutdown (`0x0093AC5D`, entered on P+12) calls main sound-view slot +0x10 if
P+0x80 is non-null, then clears/releases P+0x7C and P+0x80, in that order, and
returns true. It does not itself traverse dependent sounds or decrement the load
counter. Reset (`0x0093ADAA`, on P) calls slot +0x14 of vector entry zero
(`*(P+0xA0)`) and returns true, without testing that entry for null.

| P-relative state | Observed meaning |
|---|---|
| +0x3C, dword | Sound ID written by SetID |
| +0x40, dword | Load nesting counter |
| +0x44, dword | Interpreter wait remaining |
| +0x48, dword | Callback countdown that stops the main source on reaching zero |
| +0x50, dword | COM reference count |
| +0x58, byte | Sequencer-active flag used by IsPlaying and timer |
| +0x5C, dword | Paused state; nonzero suppresses the timer |
| +0x6C / +0x70 | Current / stopped position words selected by CurrentPlayPosition |
| +0x7C / +0x80 | Main source primary / sound view references |
| +0x88 | Optional volume/pan provider |
| +0x98 | Dependent-sound list sentinel member |
| +0xA0 / +0xA4 / +0xA8 | Ordered property-provider vector begin / end / capacity |
| +0xAC / +0xB0 | Hit-list references selected by GetHitList |
| +0xC0 | Attached callback reference (view at P+28) |
| +0xC4, byte | Error-condition latch |
| +0x100, byte | Stop-after-fade pending |

## 5. Public accessors and forwarding contracts

The sound view below means P+12; register view means P+4. Unless noted otherwise,
wrappers return the delegated result without imposing a clamp, range conversion,
or independently computed success. Property words are raw 32-bit values.

| Entry / method | Exact local action |
|---|---|
| `0x0093A8B5` Update(elapsed), P | Tail-call primary slot +0x60 with the same argument. It does not poll the mixer independently. |
| `0x0093E5CA` update-target thunk | Subtract 20 from this, then tail-call Update. |
| `0x0093AA6F`, `0x0093AA77` error condition, P | Parameterless setter writes byte P+0xC4=1; getter returns that byte in AL. The setter has no prescribed return. There is no setter argument or dword error code in these bodies. |
| `0x0093A959` CurrentPlayPosition(out), P | Ask sound-view IsPlaying; write P+0x6C if true, else P+0x70; always return true. This is not a byte offset in a decoded sample. |
| `0x0093E449` SetID(id), sound | Write P+0x3C, no provider lookup. |
| `0x0093E482` IsLoaded, sound | Return whether P+0x40 is nonzero. |
| `0x0093E48B` IsPaused, sound | Return whether P+0x5C is nonzero. |
| `0x0093E453` GetSoundPriority | Register-view slot +0x10(19). |
| `0x0093E45E` SetSoundPriority(v) | Register-view slot +0x0C(19, v, 0). |
| `0x0093E494`, `0x0093E49F` volume get / set | Same pair of register-view calls for register 21. |
| `0x0093E4C5`, `0x0093E4D0` pan get / set | Same pair for register 22. |
| `0x0093E51A` GetFrequency | Register-view slot +0x10(23). |
| `0x0093E471` GetOriginalFrequency | Register-view slot +0x10(73). |
| `0x0093E509` IsFading | Return whether register-view slot +0x10(42) returns nonzero. |
| `0x0093AF0D` SetFrequency(v) | Write property `0xFF099DB2` through register-view slot +0x0C(key,v,0); if P+0x7C is non-null, repeat this write through its slot +0x0C and return that result. Otherwise return true, regardless of the first write's result. |
| `0x0093E525` SetName(s), sound | Invoke string member P+0x20 slot +0x38(s), return true. |
| `0x00949B47` GetUseCount | Return zero; this is not the COM counter. |
| `0x0093ADBA` SetRegisterVal (export label), P | If P+0x7C exists and its sound view IsPlaying is true, Kill that sound. Return true in all branches; do not infer a register write from this name. |
| `0x0093AE78` main-source stop helper, P | If P+0x7C exists, tail-call P+0x80 slot +0x20; else return false. |
| `0x0093AE8C` UpdateVolPan, P | If both P+0x7C and P+0x88 exist, call provider P+0x88 slot +0x14(P+4, main-primary), return true. Otherwise return false. |
| `0x0093AF73` multi-source notification | Subtract 24 from this, call UpdateVolPan, return true even if it returned false. |
| `0x0087B103` GetRegisterFloat (export label), P | Add 4 to this and tail-call register-view slot +0x10, preserving the incoming key. This body has no integer-to-float conversion. |
| `0x0093E4B2` HasRegisterVal(key), P | Register-view slot +0x0C(23,key,0); preserve delegated result. |
| `0x0093E4E3` ClearRegisterVal(key), P | Register-view slot +0x0C(25,key,0). |
| `0x0093E4F6` GetRegisterVal(key), P | Register-view slot +0x0C(26,key,0); this dispatch is not an ordinary lookup of key. |

The last four labels are retained for locating the export, not as trustworthy API
descriptions. Register-view key translation and notifications remain HT3; these
wrappers must not be replaced by guessed semantics from their names. The core
register-view dispatch bodies are specified in §5.2; the translation table and
property-notification actions remain external/open boundaries.
Evidence: bytes at the individual entries; register forwarding adjustments are
`this-8` on the sound view and `this+4` on the primary view.

GetTrackDef (`0x0093AC87`) tail-forwards QueryInterface's two arguments to provider
vector entry 1 if non-null, otherwise returns false, leaving output untouched.
It assumes the vector storage exists. SetHitList (`0x0093AC9D`) assigns a retained
reference at P+0xAC and returns true. GetHitList (`0x0093ACB1`) first forwards QI to
P+0xB0 when non-null. Otherwise it returns false when P+0xAC is non-null. When
both are null, the shipped instructions dereference address zero while preparing
a QI call. That branch is not evidence of a successful null-list result or of a
second valid provider; do not invent fallback lookup behavior.

The attached-reference view (P+28) exposes `0x0093A97C(new)` and
`0x0093A9A3()`. The first releases old P+0xC0 when non-null, stores new, AddRefs
new when non-null, and returns true. The second releases and clears P+0xC0 only
when non-null and returns true. Assignment is **release-before-AddRef**, with no
self-assignment check; callers must keep an independent reference if passing the
same object. Evidence: `0x0093A97D–0x0093A99E`, `0x0093A9A4–0x0093A9B8`.

### 5.1 Load nesting and pause

Load (`0x0093AF41`) increments the 32-bit P+0x40 counter. Only when its new value
is one and a main primary source exists does it call main sound-view Load (+0x38).
Unload (`0x0093AF5D`) decrements it and calls main sound-view Unload (+0x3C) only
when the result is zero and the primary source exists. Both return true and ignore
the source result. There is no underflow check: Unload at zero produces
`0xFFFFFFFF` and IsLoaded is then true. A later Load wraps it to zero and does not
invoke source Load. Evidence: INC/DEC and exact equality branches at these entries.

IsPlaying (`0x0093AFEE`, sound view) first tests sequencer-active P+0x58. If clear,
it scans the dependent list in order and returns true on the first sound whose
slot +0x1C is true, otherwise false. It does not directly ask the main source.

Pause(flag) (`0x0093B01E`) sets P+0x5C=1, then forwards Pause(flag) to every
dependent. If this player's IsPlaying is true, it pauses the main source only
when flag's low byte is nonzero and that source exists, and unregisters update
target P+20 through sound-system slot +0x7C. It always returns true. A false flag
therefore still pauses interpretation and dependent sounds.

Unpause (`0x0093B08B`) clears P+0x5C, unpauses every dependent, and unpauses the
main source only when present and its own IsPaused is true. Next it invokes
primary Update(0). If IsPlaying is then true, it registers P+20 with the sound
system's +0x78 slot and period argument **20**. Finally it returns whether primary
GetErrorCondition's byte is zero. This final test occurs after the zero-time update;
Unpause is not an unconditional success. Evidence: `0x0093B0C6–0x0093B107`.

### 5.2 Register-view dispatch and propagation

These six bodies enter with R=P+4. The vector they consult is P+0xA0 through
P+0xA4. For **integer** reads, writes, containment and clear propagation, a key
whose unsigned value is at most 255 is passed through shared translator
`0x009377C9`; use its nonzero result, otherwise retain the original key. Negative
signed words therefore do not enter this translation branch. The translator's
table is an external encoding boundary. Float reads and writes do **not** perform
this translation in their own bodies.

| Register-view slot / body | Local contract |
|---|---|
| +0x10 / `0x0093B200`, integer read(key) | After key translation, scan every vector entry in order, skipping nulls. Ask provider slot +0x20 with key address and a zero-initialized integer output variant (table `0x00AD25FC`). Return the first successful provider's 32-bit result, including zero; if none succeeds, return zero. A successful zero does not continue searching. |
| +0x20 / `0x0093ADDD`, float read(key) | Same ordered search using a binary32 output variant (`0x00AD2BA4`) and the literal, untranslated key. Return the first successful output by FLD of its binary32 word. On failure return +0.0 (raw zero word at `0x00A81054`). This body performs no integer cast of the stored float. |
| +0x18 / `0x0093AE2D`, contains(key) | Translate the key, ask entry zero's slot +0x24; if true return true, otherwise return entry one's +0x24 result. It does not inspect later providers and does not null-check either entry it accesses. This is deliberately different from read's full-vector search. |
| +0x0C / `0x0093B268`, integer write(key,value,suppress) | Translate key; update primary comparison flags against signed zero using §3. Retain vector entry zero locally and write its +0x0C using the integer variant. Then perform the child propagation below. If suppress's low byte is zero, call primary +0x80(key) and return that callback's byte result; otherwise return true. Release the retained local provider before returning. Ignore the property write and child write return values. |
| +0x1C / `0x0093B36F`, float write(key,value,suppress) | Use the literal key; compare the binary32 input against +0.0 using §3; retain entry zero and write its +0x0C using the float variant. Propagate to eligible children **through their integer-write slot**, converting input with shared `_ftol2` at `0x009EEF04` and passing its low 32 bits. For finite inputs in signed-32 range this truncates toward zero, e.g. -3.9 becomes -3. Suppression, callback-result selection and local provider release match integer write. Invalid/out-of-range conversion remains the compiler helper's boundary rather than a new clamp here. |
| +0x14 / `0x0093B481`, clear propagation(key,suppress) | Translate key and visit all dependent sounds. For each supporting primary IID `0xBB1BE367`, obtain its register view through primary +0x18 and call its +0x14(key,0), then release the temporary primary view. This body does not erase a local provider property, query a child's track definition or check its override. If suppress's low byte is zero, call own primary +0x80(key) afterwards. There is no prescribed result. |

Integer/float writes traverse dependent sounds in list order after the local
write. QI each sound for `0xBB1BE367`, then ask its GetTrackDef for IID
`0xEBB10CDD`. If the returned definition pointer is null, skip that child. If
the definition's +0x24 says the key exists, skip it: the child's own track
definition protects the value. Otherwise obtain the child's register view and
call its integer-write +0x0C(key,value,suppress). Release temporary definition and
primary interfaces before continuing. The float parent forwards the literal key,
so a child's integer-write entry can subsequently translate a short key. This
propagation happens even when the parent's suppress flag is nonzero; suppress
controls the notification calls, not the local write or traversal.

Evidence: integer key translation `0x0093B208–0x0093B221`; provider precedence
`0x0093B23D–0x0093B263`; float FLD/output and no short-key branch
`0x0093ADDD–0x0093AE2B`; integer propagation `0x0093B2D1–0x0093B36C`;
float conversion and integer dispatch `0x0093B416–0x0093B42D`;
clear propagation `0x0093B4AF–0x0093B505`.

## 6. Playback entry points

### 6.1 Play and Step

Play(a,b,c,d), entered on the sound view (`0x0093E146`), first asks IsPlaying.
When already playing and property `0xFF303B1B` is zero, return true without any
argument writes or reinitialization. When already playing and that property is
nonzero, Kill first. Then store a,b,c,d under keys `0xFF141C48`, `0xFF9250B3`,
`0xFF9EC945`, `0xFF1885BE`, respectively, using register-view +0x0C with third
argument 1. Read mode key `0xFF4DCD0D`; additional writes (also third argument 1)
occur in this exact order:

| Mode | Additional ordered key=value writes |
|---:|---|
| 1 | `0xFF90AAC8`=a, `0xFF3EC880`=b |
| 2 | `0xFFB4335C`=a |
| 3 | `0xFFB4335C`=a, `0xFF90AAC8`=b, `0xFF3EC880`=c |
| 4 | `0xFFCC4D2D`=a, `0xFF4A01D6`=b, `0xFF469820`=c |
| 5 | `0xFFB4335C`=a, `0xFFCC4D2D`=b, `0xFF4A01D6`=c, `0xFF469820`=d |
| other | No extra writes |

Call primary PlayPause (+0x3C), save its byte result, then read `0xFF59FDD8`.
If that word is nonzero, call StartSndDependent with it **even when PlayPause
failed**. If PlayPause succeeded, return sound-view Unpause's result; otherwise
return false. Evidence: `0x0093E299–0x0093E2D5`. These argument words are not
implicitly floats, positions or sample IDs outside the mode-specific dispatch.

The three-argument Play wrapper (`0x0093A8BA`, sound view) calls the four-word
Play slot with its first word followed by **three zeroes**. It pops all three
incoming words, discarding the other two. Step (`0x0093E2D8`, primary view) calls
PlayPause only if IsPlaying is false, then calls Pause(0), then tail-calls
DoCommand once. It does not promise to resume periodic updates.

### 6.2 Stop's fade setup

Stop (`0x0093B108`, sound view) first calls Stop on every dependent sound. If
IsPlaying is now false it returns true immediately. Otherwise it takes the fade
branch only when a main source exists and is playing, property `0xFF595FFD` is
nonzero, and `0xFF0AD592` is zero. All other playing cases call this player's Kill
and return true.

The fade branch performs, in order, register-view writes
`0xFF47F685`=`0xFF53DBF3` (third argument 1), `0xFF2D16F2`=0 (1), and
`0xFFD1C82F`=rate (1). Rate is the **unsigned** quotient of the low 32 bits of
`read(0xFF53DBF3)*1000` divided by `read(0xFF595FFD)`; there is no floating
calculation or widened product. It then rereads rate and, if nonzero, writes
`0xFF0AD592`=1 (third argument 0). Set P+0x100 to 1 **even when rate is zero**,
and return true without immediately killing. Evidence: IMUL/DIV at
`0x0093B1B3–0x0093B1BB`, final branch `0x0093B1D5–0x0093B1F1`.
The timer performs deferred termination in §7.

### 6.3 Kill and callback lifetime

Kill (`0x0093BBF0`, sound view) performs the following order; unlike Stop it never
sets up a fade:

1. Repeatedly Kill the first dependent sound, then pop/release its list node,
   until the dependent list is empty. Invoke the main-source Stop helper,
   then invoke main sound-view Kill if the main primary exists.
2. Write `0xFF0AD592`=0 (third argument 0), clear byte P+0x100, and clear/release
   reference P+0x84. Set wait P+0x44=0 and copy P+0x70 into P+0x6C.
3. AddRef the primary player before external callbacks. Unregister update target
   P+20 through sound-system +0x7C. Erase the full pointer list at P+0x78.
4. Call HitMan +0x9C with sound view P+12 and zero. If P+0xC0 exists, invoke its
   slot +0x0C with that same sound view. Write `0xFF245400`=0 (third argument 0).
5. If sequencer-active P+0x58 is nonzero, clear it and Release the primary once.
   Release the primary once more for the temporary reference and return true.

Thus observers run before the active byte is cleared, and while the temporary
reference is held. The final Release can delete the object. The body neither
clears the pause dword nor unloads or clears the main source references.
Evidence: `0x0093BC4F–0x0093BC69` (byte stores and position copy),
`0x0093BC84–0x0093BCFC` (notifications and two conditional/unconditional releases).

## 7. Timer and interpreter scheduling

Update reaches HandleTimerCallback through primary +0x60. The callback body at
`0x0093DF4C` and the 74-byte body **misnamed NoteOff** at `0x0093E0FC` form one
control-flow region: the former jumps into the latter, which uses the same frame,
loops back into the former, and returns with one word popped. The latter is not
evidence of a separately callable MIDI NoteOff operation.

For elapsed word e, the normal callback path is:

1. If P+0x5C is nonzero, return without pruning, fading, waiting or interpreting.
2. Scan dependents; remove/release each whose IsPlaying is false, retaining the
   successor iterator after removal. If sequencer-active is false and the list
   is empty, invoke sound-view Kill and return.
3. If `0xFF0AD592` is nonzero, obtain key K from `0xFF47F685`, current x by
   reading K, target t from `0xFF2D16F2`, and rate r from `0xFFD1C82F`.
   Zero r clears the fade-enable property, but execution continues through the
   target write. Negate r when `(x<t)` differs from `(r>0)`, using signed
   comparisons. Add to x the signed quotient `low32(r*e)/1000`, rounded toward
   zero. Clamp x to t and clear enable only if positive r overshoots t or negative
   r undershoots t. Equality alone does **not** clear it. Write K=x with third
   argument 0. Operations wrap as 32-bit integer instructions; no fractional
   remainder is saved. Evidence: `0x0093E016–0x0093E079` (IMUL, CDQ, IDIV).
4. If P+0x100 is set and rereading fade-enable now yields zero, clear P+0x100,
   Kill, and return before processing waits.
5. If wait P+0x44 is nonzero, subtract the smaller of e and wait using **signed**
   comparison. For ordinary nonnegative inputs this saturates at zero. There is
   no guard against negative e.
6. While unpaused, sequencer-active, and wait is zero, invoke DoCommand. Count
   executions locally starting at zero. After each command, post-increment the
   count; if its old value exceeds 100, call HitMan diagnostic +0x74 with P and
   `Too many instructions executed without wait (infinite loop?)`. **Continue
   looping**, rather than breaking or returning. Thus the first diagnostic is
   after command 102, and further commands can each diagnose. The command's
   changes to pause, active and wait are retested each iteration.
7. After that loop exits normally, if P+0x48 is nonzero, decrement it once. If the
   result is zero, invoke the main-source stop helper (`0x0093AE78`).

The owned exception funclet (`0x0093E0DF`, 26 bytes) obtains HitMan, calls its
diagnostic +0x74 with the saved player pointer and
`Unknown exception while executing instruction`, then returns continuation
address `0x0093E0F9` to the exception runtime. This body does not perform a Kill or
clear the wait/error/active fields. The target's instructions are absent from the
export; exact continuation behavior, exception-type matching and runtime dispatch
remain a shared SEH boundary, not a claimed portable exception implementation.
Command encodings and individual command state changes remain HT1.

## 8. External boundaries and current limits

Sound-system slots +0x78/+0x7C register/unregister the adjusted update-target
pointer, not P. Child sounds use their own IsPlaying, Stop, Kill, Load, Unload,
Pause, Unpause and IsPaused slots; their result and lifetime rules belong to the
provider. Service acquisition and retained-reference assignment use shared
helpers. No OS timer resolution, mixing policy or actual audible output is
inferred from this player's 20-period registration argument.

This is a call-driven object: the evidence above does not establish a city-message
subscription or a DoMessage implementation. Its property-notification callback is
`0x0093BD06`; the view at P+16 forwards to another callback body through a thunk.
Those unresolved callback actions are HT3, not an assumed no-op message handler.

## 9. Open coverage

Only inventory rows with a section reference have a concrete contract here.
Coverage: **75 / 110 bodies, 4385 / 16012 bytes specified**. Exactly
**35 bodies / 11627 bytes remain open** (HT1: 1/6754; HT2: 7/1939;
HT3: 3/924; HT4: 3/715; HT5: 21/1295). These are body counts/bytes,
not estimates from source-line length. The largest gap is DoCommand.
Rows marked open remain necessary implementation work, grouped into HT1–HT5 in
`questions/hit-track-player.md`. Reach/neighbour attribution alone is not proof
of a method's apparent prototype. In particular the label corrections above
must be preserved when those gaps are filled.

## 10. Conformance cases

1. Seed a QI output with a sentinel, ask for an unsupported IID, and require false,
   unchanged sentinel and unchanged reference count. Ask for each supported IID
   and require the exact adjusted pointer and one AddRef.
2. Release counts 0, 1 and 2: the first two enter the deleting slot; the last
   returns 1 without destruction. Treat the count comparison as unsigned.
3. CheckedRegId(-1), CheckedRegId(74), CheckedRegId(75) yield -1, 74, 1;
   only the last emits the diagnostic. An unsigned clamp fails this case.

4. Load twice and Unload twice with a source whose calls fail. Require exactly
   one source Load, one source Unload, and true wrapper results throughout.
   Separately Unload at zero: IsLoaded becomes true; the next Load wraps the
   counter to zero without invoking source Load.
5. Pause(0) with a playing dependent and main source: dependent Pause(0) occurs,
   main Pause does not, the interpreter pauses, and updates unregister. Unpause
   performs Update(0) before registering and before checking the error byte.
6. Play while already playing, restart property zero: require no argument
   property writes. With restart nonzero, require Kill before writes. Make
   PlayPause fail with a nonzero dependent-sound key: the dependent start still
   occurs, but Unpause does not.
7. Give Stop a main source, fade denominator 3, and current value 5. Rate is
   1666. With current value 4294968 and denominator 3, the product wraps to 704
   and rate is 234, rather than the widened-product quotient 1431656000. A zero
   rate still sets deferred-stop state and is killed by the next eligible timer.
8. Fade x=0 to t=10 at r=1000, e=10: write x=10 and leave enable set. The next
   positive tick overshoots, clamps and clears it. At r=1, e=20, repeated ticks
   each contribute zero; no fractional carry accumulates.
9. With wait=10 and e=20, interpret in this callback after wait becomes zero.
   A command stub that leaves wait=0 must reach command 102 and issue a diagnostic;
   let the stub set wait on command 103 and require a second diagnostic and a
   normal loop exit. A hard cap of 100 commands fails.
10. Clear sequencer-active but leave one playing dependent: IsPlaying remains
    true. Stop that dependent and run a callback: it is pruned, then Kill occurs.
11. Supply provider values [absent, zero, nonzero]: integer read returns the
    second provider's zero without consulting the third. Containment consults
    only the first two providers, even if a later provider contains the key.
12. Write float -3.9 under a full property key with suppression enabled: local
    float storage receives -3.9, an eligible child's integer setter receives -3,
    and the parent's property notification is suppressed. A child whose track
    definition already contains the key receives no write. Repeat with an
    untranslated short key to distinguish parent float lookup from child integer
    key translation.

These are discriminating implementation cases, not results of an executed game
or an implementation test suite. No game/shadow claim is made for this docs lane.

## 11. Owned-body inventory

Byte sizes are exported body-byte counts, not distance to the next entry. The
attribution reason preserves how unnamed and folded-adjacent bodies entered the
audit. Open rows are tracked in the questions file as implementation gaps.

| Entry | Bytes | Name / local role | Attribution | Contract |
|---|---:|---|---|---|
| `0x0087B103` | 8 | `cHitTrackPlayer::GetRegisterFloat` | name-low | §5 |
| `0x0093A7EB` | 37 | `cHitTrackPlayer::CheckedRegId` | name-high | §3 |
| `0x0093A810` | 135 | `cHitTrackPlayer::QueryInterface` | name-high | §2 |
| `0x0093A897` | 7 | `cHitTrackPlayer::AddRef` | name-low | §2 |
| `0x0093A89E` | 23 | `cHitTrackPlayer::Release` | name-high | §2 |
| `0x0093A8B5` | 5 | `cHitTrackPlayer::Update` | name-low | §5 |
| `0x0093A8BA` | 17 | `cHitTrackPlayer::Play` | name-high | §6 |
| `0x0093A8CB` | 46 | unnamed | reach | §3 |
| `0x0093A8F9` | 83 | unnamed | reach | §3 |
| `0x0093A959` | 35 | `cHitTrackPlayer::CurrentPlayPosition` | name-high | §5 |
| `0x0093A97C` | 39 | unnamed | neighbour | §5 |
| `0x0093A9A3` | 25 | unnamed | neighbour | §5 |
| `0x0093A9BC` | 82 | unnamed | neighbour | open |
| `0x0093AA0E` | 97 | `cHitTrackPlayer::SetPosition` | name-high | open |
| `0x0093AA6F` | 8 | `cHitTrackPlayer::SetErrorCondition` | name-low | §5 |
| `0x0093AA77` | 7 | `cHitTrackPlayer::GetErrorCondition` | name-low | §5 |
| `0x0093AA7E` | 49 | unnamed | reach | open |
| `0x0093AAB5` | 29 | unnamed | reach | open |
| `0x0093AAD2` | 40 | unnamed | neighbour | open |
| `0x0093AAFA` | 47 | unnamed | reach | open |
| `0x0093AB4B` | 137 | unnamed | reach | open |
| `0x0093ABD4` | 137 | unnamed | reach | open |
| `0x0093AC5D` | 42 | `cHitTrackPlayer::Shutdown` | name-high | §4 |
| `0x0093AC87` | 22 | `cHitTrackPlayer::GetTrackDef` | name-high | §5 |
| `0x0093AC9D` | 20 | `cHitTrackPlayer::SetHitList` | name-high | §5 |
| `0x0093ACB1` | 37 | `cHitTrackPlayer::GetHitList` | name-high | §5 |
| `0x0093ACD6` | 212 | `cHitTrackPlayer::CompletionTime` | name-high | open |
| `0x0093ADAA` | 16 | `cHitTrackPlayer::Reset` | name-high | §4 |
| `0x0093ADBA` | 35 | `cHitTrackPlayer::SetRegisterVal` | name-high | §5 |
| `0x0093ADDD` | 80 | unnamed | neighbour | §5.2 |
| `0x0093AE2D` | 75 | unnamed | neighbour | §5.2 |
| `0x0093AE78` | 20 | unnamed | reach | §5 |
| `0x0093AE8C` | 37 | `cHitTrackPlayer::UpdateVolPan` | name-medium | §5 |
| `0x0093AF0D` | 52 | `cHitTrackPlayer::SetFrequency` | name-high | §5 |
| `0x0093AF41` | 28 | `cHitTrackPlayer::Load` | name-high | §5 |
| `0x0093AF5D` | 22 | `cHitTrackPlayer::Unload` | name-high | §5 |
| `0x0093AF73` | 11 | unnamed | neighbour | §5 |
| `0x0093AF7E` | 44 | unnamed | neighbour | open |
| `0x0093AFAA` | 29 | unnamed | reach | open |
| `0x0093AFC7` | 27 | unnamed | reach | open |
| `0x0093AFE2` | 12 | unnamed | neighbour | open |
| `0x0093AFEE` | 48 | `cHitTrackPlayer::IsPlaying` | name-high | §5 |
| `0x0093B01E` | 109 | `cHitTrackPlayer::Pause` | name-high | §5 |
| `0x0093B08B` | 125 | `cHitTrackPlayer::Unpause` | name-high | §5 |
| `0x0093B108` | 248 | `cHitTrackPlayer::Stop` | name-high | §6 |
| `0x0093B200` | 104 | unnamed | neighbour | §5.2 |
| `0x0093B268` | 263 | unnamed | neighbour | §5.2 |
| `0x0093B36F` | 274 | unnamed | neighbour | §5.2 |
| `0x0093B481` | 135 | unnamed | neighbour | §5.2 |
| `0x0093B508` | 454 | `cHitTrackPlayer::UpdatePitch` | name-high | open |
| `0x0093B6CE` | 32 | unnamed | neighbour | open |
| `0x0093B6EE` | 136 | unnamed | neighbour | open |
| `0x0093B776` | 27 | unnamed | neighbour | open |
| `0x0093B791` | 541 | `cHitTrackPlayer::PlayPause` | name-high | open |
| `0x0093B9AE` | 321 | `cHitTrackPlayer::UpdatePatch` | name-high | open |
| `0x0093BB39` | 51 | unnamed | reach | open |
| `0x0093BB6C` | 132 | unnamed | reach | open |
| `0x0093BBF0` | 278 | `cHitTrackPlayer::Kill` | name-high | §6 |
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
| `0x0093DF4C` | 403 | `cHitTrackPlayer::HandleTimerCallback` | name-medium | §7 |
| `0x0093E0DF` | 26 | `lib::Catch@0093e0df` | neighbour | §7 |
| `0x0093E0FC` | 74 | `cHitTrackPlayer::NoteOff` | name-medium | §7 |
| `0x0093E146` | 402 | `cHitTrackPlayer::Play` | name-high | §6 |
| `0x0093E2D8` | 43 | `cHitTrackPlayer::Step` | name-medium | §6 |
| `0x0093E303` | 54 | unnamed | neighbour | open |
| `0x0093E339` | 22 | unnamed | neighbour | open |
| `0x0093E34F` | 238 | unnamed | reach | open |
| `0x0093E43D` | 12 | `cHitTrackPlayer::AsIGZSndMultipleSources` | name-low | §2 |
| `0x0093E449` | 10 | `cHitTrackPlayer::SetID` | name-low | §5 |
| `0x0093E453` | 11 | `cHitTrackPlayer::GetSoundPriority` | name-low | §5 |
| `0x0093E45E` | 19 | `cHitTrackPlayer::SetSoundPriority` | name-high | §5 |
| `0x0093E471` | 11 | `cHitTrackPlayer::GetOriginalFrequency` | name-low | §5 |
| `0x0093E482` | 9 | `cHitTrackPlayer::IsLoaded` | name-low | §5 |
| `0x0093E48B` | 9 | `cHitTrackPlayer::IsPaused` | name-low | §5 |
| `0x0093E494` | 11 | `cHitTrackPlayer::GetVolume` | name-low | §5 |
| `0x0093E49F` | 19 | `cHitTrackPlayer::SetVolume` | name-high | §5 |
| `0x0093E4B2` | 19 | `cHitTrackPlayer::HasRegisterVal` | name-high | §5 |
| `0x0093E4C5` | 11 | `cHitTrackPlayer::GetPan` | name-low | §5 |
| `0x0093E4D0` | 19 | `cHitTrackPlayer::SetPan` | name-high | §5 |
| `0x0093E4E3` | 19 | `cHitTrackPlayer::ClearRegisterVal` | name-high | §5 |
| `0x0093E4F6` | 19 | `cHitTrackPlayer::GetRegisterVal` | name-high | §5 |
| `0x0093E509` | 17 | `cHitTrackPlayer::IsFading` | name-high | §5 |
| `0x0093E51A` | 11 | `cHitTrackPlayer::GetFrequency` | name-low | §5 |
| `0x0093E525` | 17 | `cHitTrackPlayer::SetName` | name-high | §5 |
| `0x0093E582` | 8 | unnamed | vtable | §2 |
| `0x0093E58A` | 8 | unnamed | vtable | §2 |
| `0x0093E592` | 8 | unnamed | vtable | §2 |
| `0x0093E59A` | 8 | unnamed | neighbour | §2 |
| `0x0093E5A2` | 8 | unnamed | neighbour | §2 |
| `0x0093E5AA` | 8 | unnamed | neighbour | §2 |
| `0x0093E5B2` | 8 | unnamed | neighbour | §2 |
| `0x0093E5BA` | 8 | unnamed | neighbour | §2 |
| `0x0093E5C2` | 8 | unnamed | neighbour | §2 |
| `0x0093E5CA` | 8 | `__ZThn20_N15cHitTrackPlayer6UpdateEl` | name-low | §5 |
| `0x0093E882` | 28 | `cHitTrackPlayer::~cHitTrackPlayer` | name-high | §4 |
| `0x0093E89E` | 62 | unnamed | reach | §4 |
| `0x0093E8DC` | 24 | unnamed | reach | §4 |
| `0x0093E8F4` | 429 | `cHitTrackPlayer::cHitTrackPlayer` | name-medium | §4 |
| `0x0093EAA1` | 404 | `cHitTrackPlayer::SetTrackDef` | name-high | open |
| `0x00949B47` | 3 | `cHitTrackPlayer::GetUseCount` | name-low | §5 |
| `0x009CFA36` | 12 | `cHitTrackPlayer::AsIGZSndRegisterSet` | name-low | §2 |

