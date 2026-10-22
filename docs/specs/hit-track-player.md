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
descriptions. The short-key translation table remains external HT3 work; these
wrappers must not be replaced by guessed semantics from their names. The core
register-view dispatch bodies are specified in §5.2; the translation table and
property-notification actions are specified in §5.3.
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
table is an external encoding boundary. The helper indexes its 80-word table
at `0x00B169F8` only for unsigned inputs below 80 and otherwise returns zero
(`0x009377C9–0x009377DC`). Float reads and writes do **not** perform
this translation in their own bodies.

| Register-view slot / body | Local contract |
|---|---|
| +0x10 / `0x0093B200`, integer read(key) | After key translation, scan every vector entry in order, skipping nulls. Ask provider slot +0x20 with key address and a zero-initialized integer output variant (table `0x00AD25FC`). Return the first successful provider's 32-bit result, including zero; if none succeeds, return zero. A successful zero does not continue searching. |
| +0x20 / `0x0093ADDD`, float read(key) | Same ordered search using a binary32 output variant (`0x00AD2BA4`) and the literal, untranslated key. Return the first successful output by FLD of its binary32 word. On failure return +0.0 (raw zero word at `0x00A81054`). This body performs no integer cast of the stored float. |
| +0x18 / `0x0093AE2D`, contains(key) | Translate the key, ask entry zero's slot +0x24; if true return true, otherwise return entry one's +0x24 result. It does not inspect later providers and does not null-check either entry it accesses. This is deliberately different from read's full-vector search. |
| +0x0C / `0x0093B268`, integer write(key,value,suppress) | Translate key; update primary comparison flags against signed zero using §3. Retain vector entry zero locally and write its +0x0C using the integer variant. Then perform the child propagation below. If suppress's low byte is zero, call primary +0x80(key) and return that callback's byte result; otherwise return true. Release the retained local provider before returning. Ignore the property write and child write return values. |
| +0x1C / `0x0093B36F`, float write(key,value,suppress) | Use the literal key; compare the binary32 input against +0.0 using §3; retain entry zero and write its +0x0C using the float variant. Propagate to eligible children **through their integer-write slot**, converting input with shared `_ftol2` at `0x009EEF04` and passing its low 32 bits. For finite inputs in signed-32 range this truncates toward zero, e.g. -3.9 becomes -3. Suppression, callback-result selection and local provider release match integer write. See §5.3 for high-bit, low-word wrapping and masked-invalid conversion outcomes; no int32 clamp is introduced here. |
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

### 5.3 Property notifications and spatial coordinates

HandleNotifySetPropertyValue (`0x0093BD06`, 745 bytes) enters on P with a
32-bit key. First read its integer value through P+4 slot +0x10, even for an
unknown or float-valued key. Dispatch by exact key equality as follows. Unless
stated otherwise, return true and ignore delegated results; this method does not
set the error latch itself. Keys absent from the table have no further action.

| Keys | Action after the initial integer read |
|---|---|
| `0xFF0150F2`, `0xFF3071B9`, `0xFF734343`, `0xFF86FE79`, `0xFFD3D39B` | If main primary P+0x7C exists, re-read the key through the **float** getter P+4 slot +0x20, spill its x87 result to binary32, then call main primary +0x1C(key,float,0). No child enumeration. |
| `0xFF050A94`, `0xFF595FFD`, `0xFF7E5922`, `0xFFA42951`, `0xFFA9B525`, `0xFFBDB196`, `0xFFF3EDF2`, `0xFFF7B8DF`, `0xFFFF4E79` | If main primary exists, call its integer setter +0x0C(key,initial integer value,0). |
| `0xFF3EC880`, `0xFF40FF96`, `0xFF444C9A`, `0xFF469820`, `0xFF4A01D6`, `0xFF4C6660`, `0xFF53DBF3`, `0xFF90AAC8`, `0xFFA6A614`, `0xFFAAB623`, `0xFFC20061`, `0xFFC6B36D`, `0xFFCC3B29`, `0xFFCC4D2D`, `0xFFCE9997` | Call UpdateVolPan; ignore its boolean. |
| `0xFF099DB2` | Call UpdatePitch; ignore its boolean. |
| `0xFFAEC905` | Call UpdatePatch; ignore its boolean. |
| `0xFF2D16F2` | If the initial integer value is nonzero, clear byte P+0x100. Zero leaves the byte unchanged. |
| `0xFF5F15E1` | Call primary +0x68(initial integer value), the cursor-setup helper. **Return its byte result**, including false. |
| `0xFFB36659` | If main primary exists, re-read this key as integer. Nonzero calls main sound-view Pause(0); zero calls its Unpause. Do not change the parent's pause dword here. |
| `0xFFC6E666` | Clear/release P+0x84 first. A zero initial value finishes. Otherwise scan dependent sounds in list order, calling each sound-view +0x30 and comparing its returned 32-bit ID. On the first match, QI it for `0xEBB10CDD` into P+0x84 and stop searching. Failed QI diagnoses `Attempt to set target to object with no register interface` through HitMan +0x74(P,text), yet returns true. No match leaves the target null. |

Evidence: complete decision tree and returns `0x0093BD06–0x0093BFEE`;
float FSTP and three-argument setter `0x0093BE45–0x0093BE58`; target
clear-before-search `0x0093BF58–0x0093BFB7`. Notifications are synchronous
property side effects, not queued city messages.

Spatial read `0x0093A9BC` (82 bytes) and SetPosition `0x0093AA0E` (97 bytes)
enter on **P+8**, using P+4 as their register view. The coordinate order is
`0xFFCC4D2D`, `0xFF4A01D6`, `0xFF469820`. Reading performs three ordered
integer lookups, interprets each word as **signed** int32 with FILD, and stores
each result to its caller-provided binary32 output. There is no scale factor,
axis interchange, normalization or separate success return.

SetPosition receives three binary32 arguments. In that same order, convert each
through shared `_ftol2` (`0x009EEF04`) and write the low EAX word using integer
setter(key,word,0), ignoring each result. Then write `0xFFCC156F`=0 with
suppression 0 and return that final delegated result. Consequently observers can
see partially updated coordinates; a failed coordinate write does not suppress
later writes or the final invalidation write.

For finite binary32 inputs representable as signed int64, this helper truncates
toward zero, and the caller keeps the low 32 bits; it does not clamp to int32.
For example, 2147483648.0 produces word `0x80000000`, read back as -2147483648.0;
4294967296.0 produces zero. With x87 invalid exceptions masked, NaN, infinities
and inputs outside signed-int64 range take the integer-indefinite path and also
produce low word zero. Unmasked invalid exceptions can trap; no local recovery
is installed. Evidence: helper FISTP **qword**, correction and indefinite path
`0x009EEF0D–0x009EEF78`; coordinate FILD/FSTP and low-word arguments at the
two owned entries. These are integer coordinate units exposed by the properties;
this body establishes no conversion to city/world distance.

### 5.4 Pitch computation and completion time

The 49-byte helper `0x0093AA7E` computes a double-base, signed-int32-exponent
power using x87 intermediates. Start an accumulator at 1 and current base at the
incoming binary64 value. Take the exponent's magnitude by 32-bit negation when
negative (INT_MIN remains unsigned magnitude `0x80000000`). Process bits from
least significant upward: multiply accumulator by current base when the bit is
set; shift magnitude logically right; if it is now nonzero, square the current
base and repeat. Pop the current base at the end. For a negative original
exponent return 1/accumulator, otherwise accumulator. No binary64 intermediate
store occurs inside this helper. Exponent zero returns 1 even for a NaN base;
zero base with a negative exponent can raise divide-by-zero or yield infinity
under masked exceptions. This is not a library pow call with reordered products.

UpdatePitch (`0x0093B508`, 454 bytes, P):

1. Null main primary returns false. Otherwise QI it for `0x206077D2` into a
   null-initialized temporary, **ignore the QI boolean**, and immediately call
   the output's IsLoaded (+0x40). Failed/null QI can fault. If not loaded,
   Release the temporary if present and return false.
2. If parent containment (§5.2) says `0xFF099DB2` is absent, initialize an
   integer variant to the temporary sound's GetOriginalFrequency (+0x70).
   Ask this player's GetTrackDef for `0xEBB10CDD`; on success ask that provider
   +0x20(key-address `0xFF099DB2`,variant) to override it, ignoring that query's
   boolean. Write the resulting word locally with suppression **0**, then
   Release the queried definition if present. This initialization can notify
   recursively; there is no special recursion guard in this body.
3. Read offsets a=`0xFF8DCFD8`, b=`0xFF69510B`, then frequency
   f=`0xFF099DB2`, in that order. If both offsets are zero, use f directly and
   skip random selection and the suppressed local frequency write below.
4. Otherwise interpret f as signed int32 and store it as binary64. Compute two
   **independent** endpoints: A=f when a=0, else f times the helper's power
   of a; B=f when b=0, else f times that helper's power of b. The binary64
   base constant is raw `0x3FF0F38F92D97963` (about 1.0594630943592953).
   Each product uses the original stored f, not the other endpoint. Spill A
   and B separately to binary64, convert A then B through `_ftol2`, keeping
   their low 32-bit words. Call HitMan +0x68 with arguments **(B,A)**. Write
   its returned word as parent `0xFF099DB2` with suppression 1 and use that
   returned word thereafter, regardless of the local write's result.
5. Write main-primary `0xFF099DB2`=chosen word with suppression 0. If the parent
   contains `0xFF3071B9`, invoke its property notification; then do the same for
   `0xFF86FE79`. Release the temporary sound view and return true. Neither
   setter failures nor those notification results change this return.

There is **no local clamp, endpoint sort or sum of pitch offsets**. The called
HitMan +0x68 maps (B,A) to A+U(B-A), all word arithmetic modulo 2^32. For bound
n=0, U returns zero without advancing its generator; otherwise U(n) is the high
32 bits of an unsigned generator word times n. Thus normal increasing endpoints
select [A,B), while reversed endpoints use a wrapped bound. This specifies the
player's use of the external random service, not that service's full seed/stream
contract. Evidence: QI/load and default `0x0093B511–0x0093B5B4`; signed FILD,
two products, binary64 spills and conversion order `0x0093B5B7–0x0093B66F`;
final forwarding `0x0093B675–0x0093B6C9`; shared Random and bounded draw
`0x00939993–0x009399AA`, `0x009136B1–0x009136C9`.

CompletionTime (`0x0093ACD6`, 212 bytes, P) returns binary32-rounded duration,
promoted back into the x87 return register. It never interprets the track stream
or includes dependent sounds. Null main primary returns +0.0. Otherwise QI main
primary for `0x09D002D2` into a null temporary. Failed QI releases any non-null
output and returns +0.0. On success call that view's +0x10 for a secondary
pointer, then take one retained reference to it (`0x0096E773`). Null secondary
releases the queried view and returns +0.0. Successful QI with null output is
not guarded before its first virtual call.

Ask secondary +0x40 for a borrowed format pointer. If pointer or its dword +8
is zero, Release secondary then queried view and return +0.0. Otherwise get
secondary +0x28 as numerator; treat numerator and format+8 denominator as
**unsigned** 32-bit words. Each is loaded as signed int32 and corrected by adding
4294967296.0 when its high bit is set. Divide numerator by denominator in x87,
spill to binary32 **before** releasing secondary then queried view, reload that
binary32 and return it. No additional divisor or milliseconds multiplier exists.
Do not infer units beyond this provider ratio. Evidence: provider and failure
paths `0x0093ACDB–0x0093AD35`, unsigned conversion and spill/release order
`0x0093AD3A–0x0093ADA9`; correction constant `0x00A80AA8` is binary32
`0x4F800000`, zero `0x00A81054` is raw zero.

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

### 6.4 PlayPause: gated success, resume and fresh activation

`0x0093B791` (541 bytes) enters on P. In this subsection G(k) means integer
read through P+4 slot +0x10; S(k,v,s) means its integer write slot +0x0C,
including the suppression argument s. Every read is performed at its stated
position; property notifications may change subsequent reads.

1. Read `0xFFB0333D`. If nonzero, ask HitMan +0x8C for that ID. If it returns
   an object, obtain its sound view through +0x10, then read that view's +0x50.
   A zero result returns **true immediately**, without clearing fade state,
   changing pause, resetting cursors or acquiring the active reference. A missing
   object or a nonzero result continues. This is a gate, not a failed start.
2. Ask this player's sound-view IsPlaying. Only if true, ask IsPaused. If both
   are true, write P+0x5C=1 and return sound-view Unpause's result. Its Update(0)
   and possible error check therefore happen through §5.1, not the fresh path.
3. Otherwise clear byte P+0x100. If G(`0xFFFF4E79`) is nonzero, perform these
   writes in order: if G(`0xFF0AD592`) is nonzero, S(`0xFF53DBF3`,
   G(`0xFF2D16F2`),1); then S(`0xFF47F685`,`0xFF53DBF3`,1),
   S(`0xFF2D16F2`,G(`0xFF53DBF3`),1), S(`0xFF53DBF3`,0,1).
   Read `0xFF2D16F2` again, multiply by 1000 retaining the low 32 bits,
   read `0xFFFF4E79` again and perform **signed** division of that wrapped product
   by this second denominator, truncating toward zero. Write the quotient to
   `0xFFD1C82F` with suppression 1. If a fresh read of `0xFFD1C82F` is nonzero,
   S(`0xFF0AD592`,1,0). The initial denominator test does not protect a later
   zero denominator after callbacks; zero or signed quotient overflow faults
   at the division, rather than returning a managed failure.
4. Clear P+0x5C, S(`0xFFAEC905`,G(`0xFFAEC905`),0), then UpdateVolPan.
   S(`0xFF5F15E1`,G(`0xFF5F15E1`),0) is the first write whose false result
   stops this path: return false with prior changes retained.
5. Copy P+0x70 to both P+0x6C and P+0x74. Retained-assign P+0xAC into P+0xB0.
   If G(`0xFF3C2160`) is nonzero, obtain a temporary HitMan reference and call
   its +0x40(ID,`0xDB1ACFB2`,address of P+0xB0). Release the temporary in either
   case. A false result returns false; the cursor and list changes are not rolled
   back. A zero ID skips this lookup.
6. Set wait P+0x44=G(`0xFF0246FE`); UpdatePitch; set byte P+0x58=1;
   AddRef this primary player; S(`0xFF245400`,1,0); return true, ignoring that
   last write's result. This body does not register the periodic update target.

Evidence: gate/resume `0x0093B79A–0x0093B800`; ordered writes and
IMUL/CDQ/IDIV `0x0093B805–0x0093B8C9`; failure and activation
`0x0093B8CC–0x0093B9A6`. Both early-success paths bypass the activation AddRef.

### 6.5 Patch selection and NoteOn

UpdatePatch (`0x0093B9AE`, 321 bytes, P) reads patch ID `0xFFAEC905`, then
`0xFF6F9546`. If the latter is nonzero and equal to the patch ID, diagnose
`Recursive sound id` through HitMan +0x74(P,text) and return false without
clearing the existing source. Otherwise, when ID differs from cached P+0x64,
clear/release P+0x7C, clear/release P+0x80, then store the new ID in P+0x64.
These are reference releases, not explicit Stop or Kill calls.

If P+0x7C is null and ID nonzero, request sound-manager P+0x94 slot +0x48
with arguments (ID,0,0,`0xFD584467`,address of P+0x7C). The output-reference
helper clears/releases the old destination before the call. A false result
diagnoses `Sample file not found` and returns false; there is no rollback of
the cached ID. Otherwise, if P+0x7C exists, clear/release P+0x80 and QI that
primary for `0x206077D2` into P+0x80. This QI result is ignored.

Still within the non-null-primary branch, read `0xFF891EAA`. If nonzero, ask
HitMan P+0x8C slot +0x58(ID), retained-assign its result into P+0x88, and if
that result is non-null call provider +0x18(P+4,main primary). Zero ID leaves
the previous provider in place. Then write property `0xFFF7B8DF` on the main
primary, using the value read from P+4 and suppression 0. Finally, whether or
not there was a primary, call UpdateVolPan, then UpdatePitch, and return true.
Their results and the property write result do not determine this return.
Evidence: `0x0093B9BB–0x0093BA57`, `0x0093BA61–0x0093BAE8`.

NoteOn (`0x0093BFEF`, 82 bytes, P, one incoming word) does **not** use its
argument. If P+0x7C is null on entry, diagnose `Attempt to play note with no
channel` and return false, without attempting UpdatePatch. Otherwise call
UpdatePatch and ignore its result, dereference P+0x80 and call its +0x18
(the three-word Play entry) with (1024,0,1). Save its byte result, call
UpdateVolPan and UpdatePitch, then return that saved byte. There is no second
null check after UpdatePatch, and no failure return for a now-null sound view;
an invalid view can fault. In particular, the incoming note word does not become
a frequency or Play argument in this body. Evidence: `0x0093BFF2–0x0093C03E`.

### 6.6 Dependent sounds and child visitors

StartSndDependent (`0x0093C0EC`, 317 bytes, P, ID) asks the sound manager from
RZ::SM at +0x48(ID,0,0,`0x206077D2`,out). The call's boolean is ignored;
a null output returns false. A non-null output is appended as a retained
reference to P+0x98 **before** any property copying or Play call. Take a
temporary retained reference to provider-vector entry zero and QI the child for
`0xFD584467`. If successful, enumerate the parent's provider zero through
+0x18(out key,out value pointer), then +0x1C for subsequent entries. For each
entry, ask the child's register view +0x18(key). Only if absent, write its
integer word from value-pointer+4 with child +0x0C(key,word,1). Existing child
values win; failed writes do not stop enumeration. This copies provider zero,
not the merged lookup across all parent providers.

Read the parent's properties in this order: `0xFF141C48`, `0xFF9250B3`,
`0xFF9EC945`, `0xFF1885BE`; call the child's sound-view +0x14 Play with those
four values in the listed argument order. Ignore its result. Release temporary
child-register, parent-provider and child-sound references in that order, then
return true. The list's reference remains even on Play failure. There is no
sequencer activation, active-player AddRef, rollback or periodic registration in
this body. Evidence: creation/list insertion `0x0093C0FC–0x0093C14B`, copy
`0x0093C150–0x0093C1AE`, Play and releases `0x0093C1B0–0x0093C220`.

The two 137-byte visitor bodies `0x0093AB4B` and `0x0093ABD4` take a child
interface and a 32-bit match ID. Each QIs the child for player primary
`0xBB1BE367`; if successful calls that player's GetTrackDef
(`+0x20`)(`0x456B8F1D`,out), then calls the returned interface's +0x1C.
This is an interface query through GetTrackDef, **not** an integer property
lookup. Compare the returned word for equality with match ID. On equality,
obtain the sound view through primary +0x0C and call respectively Kill (+0x24,
first visitor) or Stop (+0x20, second visitor). Ignore the sound action result,
release the queried definition interface, release the queried primary and
return false. On either query failure or ID mismatch, release whichever queried
references exist and return true. The original child argument is borrowed and
is not released. Returned sound view is borrowed. Both outputs start null;
there is no null check between a successful query and dereference. Caller-level
meaning of the false visitor result remains part of interpreter HT1.
Evidence: `0x0093AB4F–0x0093ABD3`, `0x0093ABD8–0x0093AC5C`; borrowed sound
view adjustment verified at shared `0x00968303`.

### 6.7 Track definition and provider-chain installation

SetTrackDef (`0x0093EAA1`, 404 bytes, P, definition) assumes vector storage
exists. If provider zero exists, call its +0x14 (clear/reset). Otherwise
allocate 24 bytes, construct the property-set object at `0x00939237` if allocation
succeeds, and retained-assign the resulting pointer (or null) into slot zero.
Retained-assign the incoming definition into slot one. Copy slot one's reference
into a temporary and query its +0x20 with key-address `0xFF4C97BE` and a
zero-initialized integer output variant (`0x00AD25FC`).

While the query succeeds and its returned word is nonzero, release/clear the
temporary current provider, then ask sound-manager P+0x94 +0x0C for the key
triple (type `0x0B8D821A`, group `0xFC45CAF7`, instance=returned word), IID
`0xEBB10CDD`, and temporary output. If output is null, stop the chain. Otherwise
retained-assign it into the next vector slot (first slot two), query that new
provider for `0xFF4C97BE`, and repeat. There is no bounds, capacity, repeated-ID
or cycle check, and **no vector resize** in this body. Slots beyond the last
written one are not cleared. A shorter replacement chain can therefore retain
old trailing providers, which §5.2 still searches. A chain exceeding allocated
slots is not a safe, dynamically growing operation established by this routine.

After the chain, read `0xFF5F15E1`; if nonzero invoke primary +0x68(value),
the cursor-setup helper at `0x0093AEB1`, ignoring its result. Then read
`0xFF891EAA`; if nonzero ask HitMan +0x58(value) and retained-assign its result
to P+0x88; zero leaves that provider unchanged. Finally QI the original incoming
definition for `0x3B93C5E7`; on success obtain its +0x1C word and pass it to
this player's sound-view SetID. Release that QI reference and the temporary
chain reference, then return true. Allocation, failed dependency lookup,
cursor-setup failure and failed final QI do not produce a false result here.
Null incoming definitions are not checked before querying provider one.

Reference assignments used here AddRef a differing new non-null reference,
store it, then Release the old non-null reference; equal pointers are a no-op.
The temporary copy adds one reference; changing it releases the previous one.
Evidence: installation `0x0093EAAA–0x0093EB29`, chain
`0x0093EB2E–0x0093EBA5`, final effects `0x0093EBA7–0x0093EC34`;
shared assignment/copy bodies `0x0094909A`, `0x00969FEA`, `0x00966956`.
The misleading shared label `cS3DVector3::cS3DVector3` at `0x004F20A0`
actually clears/releases the output reference before returning its address.

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

### 7.1 Verified command envelope and selected opcodes (partial body)

DoCommand (`0x0093C229`, **6754 bytes, still open**) begins with current cursor
C=P+0x6C. Fetch one **unsigned byte** opcode, save C locally, and immediately
store C+1 in P+0x6C. Test unsigned (opcode-1) against 96. Values 1–97 dispatch
through the absolute jump table at `0x0093DC8B`; opcode 0 and 98–255 go to
the `Illegal Instruction` diagnostic and return true. The diagnostic itself
does not restore the cursor or directly clear active/wait/error fields. The
envelope contains no end-pointer or available-length check. The following rows
are verified subcontracts, **not** full-body or opcode-set completion credit.

| Opcode / encoded length | Verified action |
|---|---|
| `0x09` / 2 bytes | Read unsigned register byte, advance cursor, CheckedRegId, then signed comparison against 127. On accepted result, read integer register and store its full word as wait P+0x44; return true. Larger accepted-helper results diagnose `TRACK ERROR: kTrackCmdWait`, return true. A raw byte 255 is diagnosed by CheckedRegId and becomes register 1; it is not sign-extended to -1. |
| `0x0B` / 1 byte | If main primary is absent or main sound IsPlaying is false, return true at C+1. Otherwise decrement current cursor back to C, set wait=**2**, return true. This polls and re-executes itself; it does not block the host thread. |
| `0x0C` / 1 byte | If active byte P+0x58 is set, clear it **before** primary Release. Return true. If already clear, simply return true. There is no Kill, source Stop, cursor reset or update-target removal in this branch. |
| `0x0D` / opcode plus 4-byte operand | Obtain a temporary HitMan reference; read the little-endian 32-bit operand at C+1 and set cursor to P+0x68 plus that word, modulo 2^32. Release the temporary, return true. Target is relative to the track base, not C or C+5. No operand-end or target-range check, and no separate C+5 cursor store, occurs in this branch. |
| `0x0F`, `0x1D`, `0x1E`, `0x5D`, `0x5E` / 1 byte | Jump directly to the true-return tail. These consume no operands and perform no local state action beyond the envelope's cursor advance. |
| `0x60` / 9 bytes | Read raw 32-bit destination key, advance cursor to C+5; read raw 32-bit source key, advance to C+9. Integer-read source from P+4 and integer-write destination with that result and suppression 0. No CheckedRegId or byte-key rejection. If write fails, diagnose `Could not set local property at kTrackCmdSetDirect`, yet return true. |
| `0x61` / 9 bytes | Same raw operands and two cursor advances as 0x60; integer-read the source key, then integer-read the **returned word as a key**, then write destination with that second result and suppression 0. Failed write diagnoses `Could not set local property at kTrackCmdSetIndirect`, yet returns true. |

All operand reads in these rows precede the associated external calls except
the explicit temporary service acquisition in 0x0D; truncated operands are not
locally guarded. Do not replace them with an inferred graceful-EOF contract.
Evidence: envelope `0x0093C237–0x0093C24C`; wait/end/poll
`0x0093C8A9–0x0093C915`, wait=2 tail `0x0093D639`; base jump
`0x0093C821–0x0093C837`, release tail `0x0093C885`; direct/indirect property
operations `0x0093C45A–0x0093C4D3`; diagnostic/true tail
`0x0093DC72–0x0093DC8A`. Raw jump-table words verify the listed opcode labels,
including all five no-op destinations `0x0093DC84`; decompiler case labels alone
are not the mapping authority. All other opcode contracts remain HT1.

### 7.2 Integer, property and comparison-wait families

Notation for the command tables: `b` is one unsigned byte; `w` is one
little-endian 32-bit word, read at the current byte address without alignment
padding. Lengths include the opcode. Each encoded register byte is passed to
CheckedRegId (§3), **zero-extended**, before use. Thus bytes 75–255 diagnose
and become register 1; they do not become negative register IDs. A table
operand `d` or `s` denotes this translated key, and `R(k)` denotes integer
property-view P+4 slot +0x10. `W(k,v)` is that view's +0x0C with `(k,v,0)`.
Unless stated otherwise, arguments are consumed and translated left to right,
each translation is followed by a signed `key <= 127` check, and failure
diagnoses the named command/operand without consuming later operands. For
ordinary byte operands the check cannot fail after CheckedRegId's fallback;
the checks still occur. There are no bytecode end/length checks. Cursor writes
precede the calls, so a diagnostic, provider failure or exception does not
transactionally undo consumption.

| Opcodes (hex) / native entry | Operands; full length | Action after decoding |
|---|---|---|
| `04` / `0x0093C3BF` | d:b, value:b; 3 | W(d, unsigned value); **255 stays 255**. False write result diagnoses Loadb. |
| `05` / `0x0093C40C` | d:b, value:w; 6 | W(d, full value word); false write diagnoses Loadl. |
| `06`, `4A` / `0x0093C4D8` | d:b, s:b; 3 | W(d,R(s)); false write diagnoses Set. |
| `0E` / `0x0093CBD2` | s:b; 2 | Compare R(s) with signed zero; set equal/greater/less flags at P+0x9C/+0x9D/+0x9E. |
| `10` / `0x0093CDA4` | d:b, s:b; 3 | Read R(d), then R(s); W(d,low32(first+second)). |
| `11` / `0x0093CE10` | d:b, s:b; 3 | Same read order; W(d,low32(first−second)). |
| `12` / `0x0093CE76` | d:b, s:b; 3 | Same read order; signed 32-bit quotient first/second, truncated toward zero. No zero or overflow check; divide by zero and INT_MIN/−1 raise the native divide exception before W. |
| `13` / `0x0093CEE4` | d:b, s:b; 3 | Same read order; W(d,low32(first×second)). |
| `14` / `0x0093CCB9` | a:b, b:b; 3 | Read R(a), then R(b); overwrite the three flags with signed equality, greater-than, less-than, respectively. No property write. |
| `15` / `0x0093CF4E` | d:b, s:b; 3 | W(d, signed R(d)<R(s) ? 1 : 0), reading both in that order. |
| `16` / `0x0093CFBE` | d:b, s:b; 3 | W(d, signed R(d)>R(s) ? 1 : 0). |
| `17` / `0x0093D226` | d:b; 2 | W(d, R(d)==0 ? 1 : 0), logical negation, not word complement. |
| `18` / `0x0093D25E` | d:b, min:b, max:b; 4 | Read R(min), then R(max); call HitMan +0x68 with **(max value,min value)**; W(d, returned word). No local sorting or range validation. |
| `19` / `0x0093D2F7` | d:b; 2 | W(d, signed absolute value of R(d)); INT_MIN remains 0x80000000 by wrapping negation. The bad-destination diagnostic says Rand. |
| `1B` / `0x0093D3F8` | none; 1 | Call sound view P+12 slot +0x20 (Stop). Ignore its result. |
| `1C` / `0x0093D3C5` | s:b; 2 | Read R(s); only when zero, perform the same Stop call as 1B. Invalid key diagnoses Assert. |
| `1F` / `0x0093D405` | d:b, s:b; 3 | W(d,R(R(s))); the second lookup key is a full word, with no CheckedRegId or 127 check. |
| `28` / `0x0093D009` | d:b, s:b; 3 | **Same signed greater-than result as 16**, despite the And diagnostic. It joins the same native comparison tail. |
| `29` / `0x0093D077` | d:b, s:b; 3 | Logical NAND, result 0/1. Read R(d) first; read R(s) only if the first is nonzero. Both encoded keys were already consumed/checked. |
| `2A` / `0x0093D0E4` | d:b, s:b; 3 | Read both, in order; W(d, bitwise OR). |
| `2B` / `0x0093D14D` | d:b, s:b; 3 | Logical NOR, result 0/1. Read R(s) only if R(d) is zero. |
| `2C` / `0x0093D1BD` | d:b, s:b; 3 | Read both, in order; W(d, bitwise XOR). |
| `2D` / `0x0093D335` | d:b, bound:w; 6 | Read R(d); write bound only if signed R(d)>bound. It imposes an upper bound although diagnostic says Max. |
| `2E` / `0x0093D380` | d:b, bound:w; 6 | Write bound only if signed R(d)<bound; diagnostic says Min. Equality skips W. |
| `2F` / `0x0093C7B2` | d:b; 2 | W(d,low32(R(d)+1)). |
| `30` / `0x0093C7EF` | d:b; 2 | W(d,low32(R(d)−1)). |
| `4D` / `0x0093C91A` | a:b, b:b; 3 | Comparison wait: proceed when R(a)==R(b). |
| `4E` / `0x0093C991` | a:b, b:b; 3 | Comparison wait: proceed when R(a)!=R(b). |
| `4F` / `0x0093CA02` | a:b, b:b; 3 | Comparison wait: proceed when signed R(a)>R(b). |
| `50` / `0x0093CA76` | a:b, b:b; 3 | Comparison wait: proceed when signed R(a)<R(b). |
| `51` / `0x0093CAEA` | a:b, b:b; 3 | Comparison wait: proceed when signed R(a)>=R(b). |
| `52` / `0x0093CB5E` | a:b, b:b; 3 | Comparison wait: proceed when signed R(a)<=R(b). |

Arithmetic, logical, indirect-read and bound writes above ignore W's boolean
result and return true. They do **not** update comparison flags. The flag
helper belongs only to explicit tests/comparisons here; incidental processor
flags are not persisted. Comparison waits consume/check both keys, read the
first property then the second, and on an unsatisfied condition subtract 3
from the **current** P+0x6C and set P+0x44=2. On satisfaction they leave wait
unchanged and retain the advanced cursor. They never set sequencer-active,
clear pause, or clear a previous error. Read/callback reentrancy can change the
current cursor; the subtraction is not an assignment from the saved opcode
address.

Native-width/order evidence: unsigned immediate load `0x0093C3E1–0x0093C3F2`;
dword load `0x0093C42E–0x0093C440`; arithmetic
`0x0093CDF9–0x0093CF49` (including CDQ/IDIV at `0x0093CEDC–0x0093CEDD`);
signed comparison and the shared 16/28 tail `0x0093D054–0x0093D072`;
NAND/NOR short-circuit tails `0x0093D0C1–0x0093D0DF`,
`0x0093D197–0x0093D1B8`; random argument pushes `0x0093D2C5–0x0093D2E9`;
bound comparisons `0x0093D370–0x0093D3C3`; wait tests/rewind
`0x0093C980–0x0093C98C`, `0x0093CA68–0x0093CA71`,
`0x0093CADC–0x0093CAE5`, `0x0093CB50–0x0093CB59`,
`0x0093CBC4–0x0093CBCD`; return tail `0x0093DC84–0x0093DC8A`.
Jump-table entries at `0x0093DC8B` fix the opcode identities; these are native
subcontracts and still carry **zero whole-DoCommand credit** until all other
families and failure paths are reconciled.

## 8. External boundaries and current limits

Sound-system slots +0x78/+0x7C register/unregister the adjusted update-target
pointer, not P. Child sounds use their own IsPlaying, Stop, Kill, Load, Unload,
Pause, Unpause and IsPaused slots; their result and lifetime rules belong to the
provider. Service acquisition and retained-reference assignment use shared
helpers. No OS timer resolution, mixing policy or actual audible output is
inferred from this player's 20-period registration argument.

This is a call-driven object: the evidence above does not establish a city-message
subscription or a DoMessage implementation. Its property-notification callback is
`0x0093BD06`, specified in §5.3. The P+16 view's action entry at table
`0x00AD3D44` points to shared thunk `0x0093E602`, which subtracts 16 and
tail-calls UpdateVolPan (`0x0093AE8C`). It adds no reference or independent
completion action. This non-owned thunk adds no body to the inventory.

## 9. Open coverage

Only inventory rows with a section reference have a concrete contract here.
Coverage: **88 / 110 bodies, 7963 / 16012 bytes specified**. Exactly
**22 bodies / 8049 bytes remain open** (HT1: 1/6754; HT2: external cleanup only;
HT3: external key-map boundary only; HT4: answered; HT5: 21/1295). These are body counts/bytes,
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

13. Gate PlayPause with a nonzero `0xFFB0333D`, a returned peer whose sound-view
    +0x50 returns zero, and seeded pause/fade/cursors. Require true with these
    fields unchanged, no active AddRef and no activation-property write.
14. Fresh PlayPause, denominator=3, copied value=-5: rate is -1666 (signed
    division); value=4294968 gives wrapped product 704 and rate 234. Make the
    `0xFF5F15E1` self-write fail: prior writes remain, but no cursor reset or
    active AddRef occurs. Make the later Hit-list lookup fail instead: the
    cursor reset has already occurred and is not rolled back.
15. Start a dependent whose Play returns false, with parent provider-zero
    values A=7 and B=-3, child containing A=0 and lacking B. Require no A write,
    suppressed integer B=-3 write, a retained list entry and true return. Parent
    provider-one values are not enumerated for this copy.
16. Replace a chain [definition,D1,D2] with one having no dependency: slot one
    changes, but D1/D2 remain retained and searchable. A failed dependency fetch
    ends the chain yet SetTrackDef returns true. Keep fixtures within vector
    allocation; no unsafe overrun experiment is required.
17. Give NoteOn a main primary, make UpdatePatch diagnose recursive ID, and
    retain a valid old sound view. Require Play(1024,0,1), volume and pitch
    updates despite that failure, and return the Play byte. Vary the incoming
    note word: it changes none of these calls.
18. Visitors: an equal queried definition ID invokes Kill or Stop respectively
    and returns false even when the action fails. A mismatch invokes neither
    action and returns true. In both cases release definition before primary;
    do not release the borrowed input or sound view.

19. Notify an unknown key: require one initial integer read, true, and no source
    call. Notify `0xFF3071B9` with integer result 12 but float result -1.25:
    require a later float read and main float setter(key,-1.25,0), not integer
    forwarding. A failed cursor-setup notification alone propagates false.
20. Select a dependent target when two children share an ID. Require release of
    the old target before scanning and QI only on the first matching child. Make
    that QI fail: one diagnostic, null target, no second-match retry, true result.
21. Set coordinates (-3.9,2147483648.0,4294967296.0). Require integer words
    -3, `0x80000000`, 0 in coordinate order, then the invalidation write. Make
    the first setter fail: later writes still occur and the final write controls
    the returned result. Readback interprets `0x80000000` as negative.
22. Pitch f=100, a=0, b=1 gives truncated endpoints A=100, B=105. A controlled
    generator word zero selects 100; `0xFFFFFFFF` selects 104. With a=b=1 both
    endpoints are 105: chosen word 105 and no generator advance. With a=1,b=0,
    the wrapped bound is `0xFFFFFFFB`; generator `0xFFFFFFFF` yields word 99,
    demonstrating the lack of sorting/clamping. These control the random-service
    input, not the original game's seed stream.
23. Completion numerator `0xFFFFFFFF`, denominator `0x80000000`: unsigned ratio
    is 1.9999999995343387, spilled binary32 result exactly 2.0. Zero denominator
    or absent format returns +0.0 without division. Require secondary Release
    before queried-view Release, after the result has been spilled.
24. Power base 2, exponent -3 returns 0.125; base 1, exponent INT_MIN returns 1
    and terminates. The latter distinguishes logical magnitude shifts from a
    signed loop that cannot terminate.
25. Command `09 FF`: unsigned raw register 255 becomes fallback register 1,
    with the CheckedRegId diagnostic, then wait receives register 1's full word.
    Command 0B with a playing main sound leaves its cursor at the opcode and
    sets wait=2. Command 0C's Release observer sees active already clear. Each
    no-op consumes exactly one byte; illegal 00 or FF also advances once and
    returns true after its diagnostic.
26. Command 60 with little-endian destination `0x11223344` and source
    `0x55667788`, at cursor `0x1000`, reads that full source key and writes
    destination with cursor already `0x1009`. For 61, make the first read return
    `0xFEED0123`: a second read uses precisely that full key. Failed writes
    diagnose but still return true. For 0D with track base `0x2000`, opcode
    address `0x1000` and operand `0x20`, require target `0x2020`, not `0x1025`.

27. **Unsigned byte load and fallback.** Stream `04 02 FF` writes register 2
    with 255 and leaves cursor at C+3. Stream `04 FF 80` first diagnoses the
    register byte 255, translates it to 1, then writes 128 to register 1.
28. **Arithmetic widths and divide exception.** With R(2)=−7 and R(3)=2,
    `12 02 03` writes −3 to register 2. With divisor zero or INT_MIN/−1, require
    an integer divide exception before any write, cursor already at C+3;
    do not invent a zero result or boolean failure. `2F 02` on INT_MAX wraps
    to INT_MIN without changing saved comparison flags.
29. **Names are not operations.** With R(2)=8,R(3)=3, `28 02 03` writes 1,
    whereas bitwise AND would write 0. `29 02 03` writes 0. With R(2)=0,
    NAND writes 1 without calling the getter for key 3; NOR does read it.
30. **Flags, bounds and indirect lookup.** R(2)=−1,R(3)=0:
    `14 02 03` sets (equal,greater,less)=(0,0,1). `2D 02 FE FF FF FF`
    writes −2; repeating it skips the setter. `1F 04 03`, with R(3)=0x1234
    and R(0x1234)=99, writes 99 to key 4 without translating 0x1234.
31. **Wait retry family.** At C=0x1000, R(2)=−1,R(3)=0, opcode 4F with
    bytes `02 03` reads keys 2 then 3, rewinds to 0x1000 and writes wait=2.
    Repeating with R(2)=1 leaves cursor 0x1003 and wait unchanged. At equality,
    4D/51/52 proceed and 4E/4F/50 retry. Use signed comparisons on high-bit words.
32. **Random boundary order.** For `18 04 02 03`, R(2)=7,R(3)=−9,
    require getter order (2,3), HitMan +0x68 arguments (−9,7), then write its
    supplied return word to key 4. No guessed distribution or endpoint repair.

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
| `0x0093A9BC` | 82 | unnamed | neighbour | §5.3 |
| `0x0093AA0E` | 97 | `cHitTrackPlayer::SetPosition` | name-high | §5.3 |
| `0x0093AA6F` | 8 | `cHitTrackPlayer::SetErrorCondition` | name-low | §5 |
| `0x0093AA77` | 7 | `cHitTrackPlayer::GetErrorCondition` | name-low | §5 |
| `0x0093AA7E` | 49 | unnamed | reach | §5.4 |
| `0x0093AAB5` | 29 | unnamed | reach | open |
| `0x0093AAD2` | 40 | unnamed | neighbour | open |
| `0x0093AAFA` | 47 | unnamed | reach | open |
| `0x0093AB4B` | 137 | unnamed | reach | §6.6 |
| `0x0093ABD4` | 137 | unnamed | reach | §6.6 |
| `0x0093AC5D` | 42 | `cHitTrackPlayer::Shutdown` | name-high | §4 |
| `0x0093AC87` | 22 | `cHitTrackPlayer::GetTrackDef` | name-high | §5 |
| `0x0093AC9D` | 20 | `cHitTrackPlayer::SetHitList` | name-high | §5 |
| `0x0093ACB1` | 37 | `cHitTrackPlayer::GetHitList` | name-high | §5 |
| `0x0093ACD6` | 212 | `cHitTrackPlayer::CompletionTime` | name-high | §5.4 |
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
| `0x0093B508` | 454 | `cHitTrackPlayer::UpdatePitch` | name-high | §5.4 |
| `0x0093B6CE` | 32 | unnamed | neighbour | open |
| `0x0093B6EE` | 136 | unnamed | neighbour | open |
| `0x0093B776` | 27 | unnamed | neighbour | open |
| `0x0093B791` | 541 | `cHitTrackPlayer::PlayPause` | name-high | §6.4 |
| `0x0093B9AE` | 321 | `cHitTrackPlayer::UpdatePatch` | name-high | §6.5 |
| `0x0093BB39` | 51 | unnamed | reach | open |
| `0x0093BB6C` | 132 | unnamed | reach | open |
| `0x0093BBF0` | 278 | `cHitTrackPlayer::Kill` | name-high | §6 |
| `0x0093BD06` | 745 | `cHitTrackPlayer::HandleNotifySetPropertyValue` | name-high | §5.3 |
| `0x0093BFEF` | 82 | `cHitTrackPlayer::NoteOn` | name-high | §6.5 |
| `0x0093C041` | 35 | unnamed | neighbour | open |
| `0x0093C064` | 53 | unnamed | neighbour | open |
| `0x0093C099` | 41 | unnamed | neighbour | open |
| `0x0093C0C2` | 42 | unnamed | reach | open |
| `0x0093C0EC` | 317 | `cHitTrackPlayer::StartSndDependent` | name-high | §6.6 |
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
| `0x0093EAA1` | 404 | `cHitTrackPlayer::SetTrackDef` | name-high | §6.7 |
| `0x00949B47` | 3 | `cHitTrackPlayer::GetUseCount` | name-low | §5 |
| `0x009CFA36` | 12 | `cHitTrackPlayer::AsIGZSndRegisterSet` | name-low | §2 |

