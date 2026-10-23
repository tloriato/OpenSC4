# Curve entry views

Status: WIP, first dedicated native behavior publication, 2026-10-23, lane re-curve-editor-views4.
Evidence: Windows SimCity 4 Deluxe 1.1.641 native export queried with `re_query.py`;
instruction bytes govern widths and predicates. Local pseudocode is navigation only.
This is a bounded specification, not whole-family coverage of an approximate byte count.
The working PE SHA-256 and `ghidra/export/windows/export-summary.txt` both give
`3bc5c7fe807fe5aa24a3abb06e40f13486670bc1150612c09baf4ecedafe1a4b`, image base
0x00400000; the PE timestamp is 0x4C12BFB7. No native execution was performed.

## 1. Owner and interface boundary

The resource editor's [class registry](resource-exemplar-editor.md#8-value-editor-classes-compat-for-the-ids)
identifies Happiness class 0x071E971B/factory 0x006648F0 and GeneralResponse class
0xE8DDA013/factory 0x006642A0. Both factories allocate 0x110 bytes, test the returned
pointer, construct, test again, and return object +0xD8 or null. Constructors are
0x006647F0 and 0x0066B630. Their primary vtables are 0x00AAD130 and 0x00AADEE8;
entry-interface vtables at +0xD8 are 0x00AAD10C and 0x00AADEC0. The interface at
+0x100 has vtable 0x00AAD0FC or 0x00AADEB0.

QueryInterface recognizes 0x273B784C, writes object +0x100 through the supplied
output pointer, calls primary slot +4 to acquire a reference, and returns AL=1
(0x0066B110, 0x0066B190). Other IDs delegate to 0x00687910; Happiness first
clears the output. Valid non-null receiver and output storage are the normal
domain; GeneralResponse's nominal null-receiver branch still dereferences that
receiver and is not a safe null API.

Both constructors call the standard-type base constructor 0x00687C20. Happiness
initializes the container at +0x104: allocates a 0x18-byte sentinel, zeroes its
byte +0 and dword +4, sets links +8/+0xC to itself, and sets object +0x108 to zero
(0x0066481C–0x0066485F). This allocation is dereferenced without a null test.
GeneralResponse zeroes +0x104, +0x108, +0x10C (0x0066B65E–0x0066B66A).
These are different state layouts, not interchangeable curve implementations.

## 2. Happiness point mutation

Primary slot +0x264, `addPoint` (0x0066CDB0), takes a stack dword whose low
16 bits are interpreted as a signed key and a binary32 value. It rejects keys
below 0 or above 255. It then loads the binary32 value, compares against
binary32 30 (bits 0x41F00000 at 0x00AA20C0), captures x87 status and tests
AH & 0x41; a zero result rejects. A second load compares against binary32
+0 (0x00A81054); testing AH & 5 followed by JNP rejects when the masked
bits have odd parity (PF=0). For ordinary finite operands this admits 0 through 30,
inclusive. Unordered comparison passes these status predicates if execution
continues under the current floating-point exception state; exception behavior
is not specified here.

On admission, container lookup/insertion 0x0066C570 receives the address of the
key in the stack argument. The original value dword is copied verbatim through
the returned pointer (0x0066CDF5–0x0066CDF9), then primary virtual +0x260 is
called to refresh the display. There is no owner equality test before that
store or refresh. Rejection has neither effect. The return register is not a
documented boolean. Duplicate-key/container mechanics remain a separate
body-scoped evidence obligation; this rule does not invent sorting or endpoints.

Primary slot +0x268, misleadingly named `SetValue` (0x0066BD30), takes only a
key, applies the same signed-low-word 0..255 gate, calls 0x0066B710 with receiver
object +0x104 and key address, and refreshes through +0x260 only if the returned
full dword is unsigned greater than zero. It is not the entry-interface
variant SetValue method. No redraw follows zero. The helper's export name
does not establish its semantics.

## 3. Command routing

Happiness uses folded command body 0x0066B160: selector 0x287259F6 dispatches the
second dword argument to primary virtual +0x25C (its button handler); other
selectors return AL=0. GeneralResponse's 0x0066DCE0 uses the same selector and
directly calls 0x0066CE10, returning that result. Neither command body owns a
general window-message algorithm. Button decisions and value readback are
specified separately as their bodies are qualified.

## 4. Held values, import and readback

All offsets in this document are from the complete primary object unless an
entry receiver is explicitly named. Entry QueryInterface thunks subtract 0xD8
(0x00664870, 0x0066B680). The shared entry setter +0x18 is 0x0066B180, a jump
to 0x00687950: it adds 4 to the entry receiver and tail-calls variant assignment
0x009161DB. Thus the assigned variant is at complete object +0xDC; this is a
base primitive, not a curve-owned parse or point update. The two owner readback
bodies use their point collections, not that original variant. Entry slots
+0x10/+0x14 store/load one opaque dword at entry +0x24 = complete +0xFC
(0x00664010/0x00664020). They do not acquire/release that dword.

### 4.1 Happiness packed records

On Init with the existing init flag clear, 0x0066C629–0x0066C68A gets the
held variant's full-dword count through +0x10 and loops while a signed index
is less than the saved signed count. Every iteration obtains the buffer
through +0x19C and reads one dword. The high word is the map key; comparisons
in 0x0066C570 interpret it as signed 16-bit. The low word is sign-extended to
32 bits, FILD-loaded, multiplied by binary32 0x3C23D70A (approximately 0.01),
then FSTP-stored as binary32 at the map value. Import bypasses the public
addPoint range gates. It does not clear existing map entries first.

The lookup helper descends with signed word comparisons, returns the existing
node's value pointer for an equal key, or calls insert-unique with a temporary
key/zero-value pair, then returns node +0x14 (0x0066C570–0x0066C5E8).
Consequently later imported duplicates overwrite earlier values. Node key is
at +0x10 and value at +0x14; traversal for readback is ascending signed key.
Tree balancing/allocation are container primitives, not editor policy. The
key-only slot +0x268's helper 0x0066B710 finds the equal-key range via
0x0066B440 (signed lower/upper bounds), counts its successors with 0x0046F860,
erases that range through 0x0066B380, then returns the saved full-dword count.
For a valid unique-key map that is 0 for absent or 1 for present. The owner
therefore refreshes after successful removal, not after an absent key. Its
export name `cSCStandardTypeEntryView::SetValue` is folded container attribution,
not an entry-value setter. Node unlink/free/rebalance is a foreign primitive;
0x0066B42C–0x0066B432 decrements the container's count after each removed node.

Entry GetValue +0x1C is 0x0066BD70 (receiver object +0xD8). It allocates four
bytes per map entry, traverses from sentinel +8 in successor order, and for
each value performs this exact x87 schedule: FLD binary32 Y; FMUL binary32
100 (0x42C80000); FADD binary32 12582912 (0x4B400000); FSTP binary32 scratch;
mask the scratch's raw bits with 0xFFFF. Sign-extend the node's key word, shift
left 16, add those masked bits, and store the resulting dword. This is a
floating bias/bit-extraction operation, **not** an integer cast. Current x87
PC/RC and exceptional operands matter; no environment-independent rounding
promise is made. It then calls output variant slot +0x198 with buffer/count,
frees the buffer, and frees an otherwise unused temporary 8-byte allocation
(0x0066BE0A–0x0066BE22). Empty map still calls that output slot with count zero.
No output setter success test or allocation-null recovery exists in this body.

### 4.2 GeneralResponse pairs

With the init flag clear, GeneralResponse first changes window flags using
primary +0x110 with (0x800,0), then (0x10000,1), then calls base window Init
(0x0066D253–0x0066D27B). It asks the held variant's count, unsigned-shifts it
right one, and calls vector-size helper 0x0066D1E0 on +0x104. That helper
requests truncation or appends zero-filled 8-byte records to reach the size;
the storage operations are foreign vector primitives.

The import loop obtains the count again, then for indices 0,2,4,... below
that saved signed count calls variant +0x1BC for the buffer. X is copied as
a raw dword. It calls +0x1BC again, FLD-loads the next binary32 Y, duplicates
ST0 and FSTP-stores one copy into record +4; the other copy participates in
Init's bounds setup (0x0066D2C2–0x0066D386). Order and duplicate X values are
preserved. No type predicate, sort, add-action epsilon check, or accepted-range
gate occurs in this import prefix. Normal domain is a valid variant buffer
with nonnegative **even** element count. Odd count is not silently truncated:
the loop attempts to read a final Y beyond the logical count and write a
record beyond the requested size. No portable malformed-input result is claimed.

Entry GetValue +0x1C is 0x0066B220. Count is (end-begin) arithmetic-shifted
right 3; it allocates eight bytes per record, copies both dwords in stored
order (four-record unrolling plus remainder), calls output variant +0x1B8
with buffer and **twice** the record count, then frees the buffer
(0x0066B319–0x0066B32C). There is no float conversion on this readback,
no deduplication, and no nonempty guard before the zero-count output call.
No allocation or output-setter result is tested.

For both owners, assigning a new held variant after Init is not proof of
re-import: the shared setter only assigns the variant. Init checks AL from
the existing init flag and returns AL=1 immediately when set
(0x0066C60E–0x0066C628; 0x0066D246–0x0066D24D). Do not invent reset,
cancel/revert, target-property writeback, or dirty notifications here.

## 5. Button and current-point decisions

Raw lookup is primary slot +0x88 by child ID. A non-null returned child is
queried for 0x231A1C57; code then uses the output interface pointer without
testing the query's AL/fullword result or output pointer. Reading text calls
interface +0xB4 with a temporary string, then releases via +8 before parsing
that string. Successful interface acquisition/output and live control pointers
are normal-domain requirements, not checked failure recovery.

### 5.1 Happiness

`doButtonPress` 0x0066B9B0 initializes parsed key to zero, binary32 Y to +0,
and a local flag to false. Action 0x26C10A3B looks up X child 0x26C10A3E,
reads/releases/parses with 0x009EF84A (`atol`) if present, saving the full
integer result and setting the local flag. It **also looks up** Y child
0x26C10A3F even when X was missing; the flag is tested only after that lookup
(0x0066BA54–0x0066BA65). If X existed, Y is read/releases/parsed with
0x00910991 (`RZAtofEnglishA`) when present and its x87 result FSTP-stored as
binary32. Missing Y leaves +0. It calls owner +0x264 with parsed key dword
and Y. That callee's low-word and float gates govern admission.

Action 0x26C10A3C first compares the map count at +0x108 **unsigned** against
2; only greater permits X lookup/read/release/atol and owner +0x268 call.
There is no protected-endpoint test in the handler. Other actions have no
point effect. Unlike GeneralResponse, this handler returns AL=1 even for
unknown actions, missing controls, or a blocked delete (tail 0x0066BB21).
The key-only helper's removal/count behavior is specified in §4.1.

### 5.2 GeneralResponse

`DoButtonPress` 0x0066CE10 takes a raw dword action ID, not a float despite
the pseudocode signature. All parsing uses 0x0090FC5B (`RZAtofA`), with its
extended return immediately FSTP-stored to binary32. Add requires X child
0x26C10A3E; absent X returns AL=0 and skips Y. If Y child 0x26C10A3F is absent,
the X string is reused and parsed again for Y: **Y becomes the parser result
for X text**, not a zero default (0x0066CEC0–0x0066CF00). Present Y replaces
the temporary string after X has been parsed. There is no owner parse-success
test. String syntax and parser failure values remain foreign parser contracts.

Define match as the exact schedule at 0x0066CF50–0x0066CF63: FLD stored X;
FSUB binary32 parsed X; FABS; FCOMP **binary64** constant at 0x00AADEA0
(bits 0x3EE4F8B588E368F1, the binary64 value of 1e-5); FNSTSW AX; TEST AH,5;
JNP selects match (odd parity). For ordinary finite arithmetic this is strict
absolute difference less than that constant; equality is not a match and an
unordered comparison is not a match. No intermediate binary32 store occurs
between subtraction and comparison.

| Action | Owner decisions in order | Result |
|---|---|---|
| 0x26C10A3B add | For empty valid vector, push pair then refresh. Otherwise scan from begin: first match stores only Y's raw dword, refreshes, stops; otherwise if stored X > parsed X, insert pair before that record (0x00665DB0), refresh, stop; otherwise advance 8 bytes. At end, append (raw two-dword store/end increment if capacity, growth helper 0x007C61A0 otherwise), then refresh. | AL=1 once add reaches mutation path; missing X AL=0 |
| 0x26C10A3C delete | Require X child; parse X; scan stored order using same match. First match calls vector erase 0x00757430, saves returned successor, **refreshes first**, then rereads begin/end. If nonempty, select returned successor or, when it equals end, preceding record; pass its two raw fields to SetCurrentPoint. | AL=1 for a found/erased point, including deleting last point; AL=0 if missing child or no match |
| 0x26C10A40 previous | Require/parse X; scan first match. Select previous index, wrapping index 0 to count-1. | SetCurrentPoint then AL=1; no match/empty/missing child AL=0 |
| 0x26C10A41 next | Same scan. Select next index, wrapping last index to 0. | SetCurrentPoint then AL=1; no match/empty/missing child AL=0 |
| anything else | No read or mutation after action tests. | AL=0 |

Insertion's greater-than test is FCOMP stored X against parsed X, TEST
AH,0x41, JZ (0x0066CF65–0x0066CF70). Unordered does not insert-before; scan
continues. Approximate-match replacement retains the old X even when parsed
X differs. Neither same-Y replacement nor same-point navigation is suppressed.
Import need not have sorted records; add scans their actual stored order and
does not sort the existing vector. No X/Y range or mandatory endpoint is
imposed by these branches. Container allocation/erase mechanics remain raw
primitives; owner decisions and sequencing above are not delegated to them.

`SetCurrentPoint` 0x0066C060 takes two binary32 stack arguments. It creates
a temporary string, looks up X child 0x26C10A3E and, if present, queries its
text interface, promotes X via FLD binary32/FSTP binary64 and formats `%f`
(0x00AAE160). It calls interface +0xC0 with (string,0), then releases.
**Only inside that X-present path** does it look up Y child 0x26C10A3F and
perform the same formatting/set/release with Y. If X is absent, no Y lookup;
if Y is absent, X's already-completed set remains. It destroys the temporary
string on exit. There is no point mutation or repaint in this body, nor a
checked setter return. A button can therefore return AL=1 even when this
helper sets no control.

## 6. Happiness display sampling is owned here

0x0066BB50 first looks up summary child 0x26C10A3A. If present, it constructs
a temporary variant, calls its own entry GetValue, calls foreign property
formatter 0x00674140 with +0xFC and that variant, queries child interface
0x212CDC1F, sets text through +0x10, releases, destroys variant, then string.
Absent summary skips that prefix but does not skip graph lookup.

It then looks up child 0x26C10A3D; if absent it returns. Otherwise it queries
0x12CF2351 (unchecked output), calls graph interface +0x20(), +0x14(1),
+0x18(0), then performs **256 owner-computed samples** for integer X 0..255.
This interpolation belongs to the entry view, not the foreign graph.

Let `current` and `previous` initially both be the least signed-key node.
At each sample i, if i equals current's signed key, use that node's raw Y
bits, set previous=current, and advance current to its tree successor.
Otherwise calculate with the current and previous nodes in this exact order:
FLD current.Y; FSUB previous.Y; FIMUL signed dword (i-previous.key);
FIDIV signed dword (current.key-previous.key); FADD previous.Y; FSTP
binary32 scratch (0x0066BCCC–0x0066BCF0). Pass that raw result to graph
+0x50(Y,0,-1). There is no per-sample X argument; graph positioning is foreign.
After sample 255 it calls +0x6C(0) and releases the graph interface.

A sufficient normal domain for this sampling path is a valid map with first
key 0 and last key 255, and valid increasing intermediate keys. This is a
domain qualification, **not** an enforced owner invariant: import and deletion
do not establish it. The body has no empty/single-point or sentinel check and
no clamp, extrapolation policy, or divide-by-zero branch. For a first key other
than 0 the initial interpolation uses the same node on both sides, producing
a zero denominator. Empty maps and advancing beyond the final usable key can
read sentinel key/value storage; no fabricated empty-graph result is specified.
Summary serialization occurs before this potentially unsupported sampling suffix.

## 7. GeneralResponse display and paint

### 7.1 Summary and graph primitive calls

0x0066C180 has the same ordered summary prefix as Happiness: lookup
0x26C10A3A, own GetValue into temporary variant, foreign property formatter
with +0xFC, query 0x212CDC1F, text +0x10, Release, variant destruction,
string destruction. Graph lookup 0x26C10A3D follows even when summary is
absent. If graph is absent, return. If present, query graph interfaces
0x32CF2351 then 0x02CF2351, with no success/pointer tests before use.

On 0x32CF2351, call +0x18(-1), +0x14(-1), +0x2C(1,-1), +0x1C(0x2B,-1)
in that order. Initialize a temporary service pointer to null; call framework
getter 0x008793EC; if framework non-null, call its +0x14 with
(0xC416025C,0x0073283C,&service). The body subsequently dereferences service
even if framework was null or service acquisition failed. Call service +0x88
with (0,0,0,-1); pass its full EAX to graph 0x32CF2351 slot +0x24; release
the temporary service through 0x004495C0 (0x0066C2BA–0x0066C300).
No integer/pointer interpretation of that service result is imposed here.

After bounds setup (§7.2), set interface 0x02CF2351 slots in order:
+0x60(1,Y tick), +0x60(0,X tick), +0x3C(0,X lower), +0x40(0,X upper),
+0x3C(1,Y lower), +0x40(1,Y upper). For each record in current stored order,
call graph 0x32CF2351 +0x3C(X raw dword,Y raw dword,-1,-1). Recompute the
count after each callback (0x0066C52E–0x0066C540); no snapshot is taken.
Finally call that graph's +0x6C(0), release 0x32CF2351, then release
0x02CF2351. The owner supplies records; rendering/interpolation within these
foreign graph calls is not specified. There is no owned resampling loop here.

### 7.2 Native bounds schedule

The valid vector domain is begin/end/capacity describing integral 8-byte
records. Empty is tested by `(end-begin) & 0xFFFFFFF8` equal zero, not a
variant-type predicate. Empty starts X lower=0, X upper=1, Y lower=0,
Y upper=1 (binary32 one at 0x00A81228). Nonempty initializes all four from
the first record. Then visit remaining records in order, selecting X upper,
Y upper, X lower, Y lower in that order. Each ordered extremum test uses
FCOMP, FNSTSW, TEST AH,5 and JNP: upper is replaced only when old upper is
strictly less than candidate; lower only when candidate is strictly less
than old lower. Ties/unordered comparisons retain the earlier value. X
choices and Y-lower choices are raw dword copies; Y upper is carried through
x87 FLD and FST binary32, with an extended copy retained on the stack
(0x0066C380–0x0066C3E2). No global probability/range assumption is used.

Compare Y lower and the carried Y upper using FUCOMPP, FNSTSW, TEST AH,0x44,
JP. Ordered equality follows the non-jump path and adds binary32 1 to the
carried upper, then FST-stores a binary32 copy while retaining the extended
value. Other ordered results and unordered skip the increment. Apply the same
equality test to X lower/upper and, on equality, FLD X upper, FADD binary32 1,
FSTP binary32 X upper (0x0066C3FC–0x0066C430). This widens equal extrema;
it does not change the records.

For Y tick: retained Y upper minus binary32 Y lower, times binary32
0x3E4CCCCD (approximately 0.2 at 0x00A94D54), FSTP binary32 directly into
the callback argument. For X tick: FLD binary32 X upper, FSUB binary32 X
lower, FMUL the same constant, FSTP binary32 callback argument. Tick callbacks
precede padding. The extended Y value after its equality increment is used
for tick arithmetic; the later padding reloads the stored binary32 copy.

For Y lower, FCOMP against +0, TEST AH,0x41: if zero (strictly greater),
replace with raw +0. Otherwise FLD the stored lower, FMUL binary32 0x3F8CCCCD
(approximately 1.1 at 0x00A8FAF4), FSTP binary32 lower. For Y upper, compare
against +0 with the same predicate, reload upper, multiply by 0x3F8CCCCD
if strictly greater or 0x3F666666 (approximately 0.9 at 0x00A88218) otherwise,
then FSTP binary32 upper (0x0066C469–0x0066C4B9). X extrema get no corresponding
padding. These compare/store rules, not algebraically rearranged expressions,
govern floating-point exceptions, signed zeros and unusual PC/RC modes.

### 7.3 Paint wrapper

0x0066B1D0 loads the pointer at +0x68 without a null test, calls its +0x18
with 0x8001 and tests **AL**. If nonzero, it calls owner virtual +0x19C(),
passes that EAX, the address of object +0x24 and zero to the +0x68 object's
slot +0x50, then calls its +0x1C(0x8001). Both branches tail-delegate to
0x00949ADE. That export's unrelated name does not establish paint return
semantics. This is a raw surface/window primitive boundary, not an editor
point mutation or a rewrite of the shared painting algorithm.

## 8. Formatting and lifetime boundaries

### 8.1 Formatting callbacks

Both formatter bodies consume three stack dwords (RET 0xC); the first is
unused in these bodies, second is input variant pointer, third is output
string pointer. They do not consult receiver curve state. Both return AL=1
and do not test output-string primitive results.

Happiness 0x0066BE40 builds a temporary accumulated string. It checks variant
count +0x10 as an unsigned full dword and, for each index, obtains +0x19C
twice. From the first buffer it masks the record's low word to **unsigned**
0..65535, FILD-loads that nonnegative dword, FMULs binary32 0x3C23D70A,
FSTP-stores binary64 for the formatting argument. From the second buffer it
sign-extends the high word. It formats `(%d,%.2f) ` with signed key then
promoted scaled value (0x0066BECC–0x0066BF12), appends to the accumulator,
destroys each item string, and obtains count again for the next unsigned
comparison. Finally it assigns accumulator through output +0x38 and destroys
the accumulator. Empty input assigns an empty string. **This unsigned low
word formatting differs from Init's signed low word import.** The unreachable
negative-dword adjustment after masking does not alter the normal rule.

GeneralResponse 0x0066BF90 first calls output +0x58(0,-1), obtains variant
buffer via +0x1BC once, then count +0x10, unsigned-shifts count right one,
and visits that many pairs. For each, FLD binary32 Y, FSTP binary64 argument;
FLD binary32 X, FSTP binary64 argument; format `(%.2f,%.2f) ` and append with
output +0x40. Destroy the reusable temporary string on exit. Empty count clears
the output via the initial call; an odd positive count formats only the
complete pairs. This differs from its Init odd-count overrun, not a safe
malformed-value import policy. Decimal conversion itself is the raw formatter
primitive; locale, exceptional spelling and formatting failure remain foreign.

### 8.2 References and destruction

Primary AddRef/Release slots are shared jumps 0x007BE160 → 0x0094C47C and
0x007BE170 → 0x0099B7A3. Entry thunks 0x007BE550/0x007BE560 subtract 0xD8;
formatter-interface thunks 0x00664890/0x0066B6A0 subtract 0x100, then use those
same shared primitives. Formatter QueryInterface thunks subtract 0x100
(0x00664880/0x0066B690). No independent curve reference counter is evidenced.
The special-interface query writes the output **before** invoking AddRef;
there is no replacement-release of the caller's previous output pointer.
Same-output queries therefore still acquire a reference. Unsupported/null
output storage is outside the proven normal domain. Full shared refcount
algorithm and interface behavior on delegated IDs are not re-specified here.

Happiness deleting destructor 0x006648A0 rewrites its three vtable pointers,
destroys the map at +0x104 via 0x006647A0, calls base destructor 0x00663FE0,
then tests bit 0 of its stack flag's **low byte**; if set, free the complete
object with 0x005E5620. It returns the original object address in full EAX.
The base destructor resets base vtables, destroys held variant +0xDC, then
tail-delegates to foreign window destructor 0x0099E1A2.

GeneralResponse deleting wrapper 0x0066B990 calls 0x0066B6B0 first, applies
the same low-byte flag-bit/free test, returns original address. The inner
body resets all three owner vtables, conditionally frees vector begin +0x104
through 0x0090CF63 if non-null, resets base vtables, destroys held variant
+0xDC, then tail-delegates to 0x0099E1A2. No per-record reference Release
occurs; the records are raw two-dword values. The symbol naming that deallocation
as a string destructor does not make vector records strings.

All collection stores precede refresh callbacks; no rollback is evidenced.
Summary text changes and graph setup are observable prefixes before later
unchecked graph/service dereferences. GeneralResponse delete saves an erase
iterator across refresh and then uses it; graph enumeration rereads count after
callbacks; Happiness retains node pointers across graph callbacks. There is
no proved reentry guard or stable snapshot. A callback that mutates/deletes
owner state is outside the normal non-mutating callback domain; do not claim
safe cancellation or iterator repair. Variant assignment owns no wrapper-level
same/null branch: such input semantics belong to its foreign assignment
primitive, while subsequent imports require the valid-buffer domains in §4.

## 9. Independent literal ledger — PROPOSED / UNEXECUTED

Expected effects below are derived from the instruction predicates, not from
running the original or an implementation. Refresh means the owner virtual
call, without a claim about its foreign drawing primitive.

| Trace | Native input and precondition | Expected owner effects |
|---|---|---|
| H-A | addPoint key dword 0xABCD0011, value 7.25 (0x40E80000), successful container primitive | low signed word is 17; lookup/insert key 17, store 0x40E80000, refresh |
| H-B | key 0x0000FFFF, value 7.25 | signed key -1 rejects before float comparisons; no container call or refresh |
| H-C | key 256, value 7.25 | rejects; no store/refresh |
| H-D | key 0, value +0; key 255, value 30 | both boundary calls admitted; each stores original bits then refreshes |
| H-E | key 17, value -0.5; key 17, value 30.5 | each rejects before container mutation |
| H-F | key 17, value -0 (0x80000000) | admitted; original negative-zero bits stored, then refresh |
| H-G | key-only slot +0x268 key 17; absent then separately present in valid map | helper returns0 then1; first has no refresh, second erases before refresh |
| C-A | selector 0x287259F5, action 0x26C10A3B | both owners return AL=0, no button dispatch |

Additional independently derived traces use ordinary finite values and assume
foreign primitives return normally. Numeric round-trip expectations explicitly
assume x87 nearest-even and adequate precision; text examples assume the `%f`
primitive returns conventional six-place decimal text.

| Trace | Input / controls / initial points | Expected native result or ordered effect |
|---|---|---|
| H-I1 | Import words 0x00FF04B0, 0x000000C8, 0x001102D5, 0x00110320 | map keys ordered 0,17,255; Y are binary32 stores of 200*c, 800*c, 1200*c respectively, c=0x3C23D70A; later key-17 value wins |
| H-I2 | Import word 0xFFFEFF85 | signed key -2; Y=FSTP32(-123*c); no import range rejection; graph-present sampling outside qualified domain |
| H-R1 | map (0,2), (17,7.25), (255,12) | output words 0x000000C8, 0x001102D5, 0x00FF04B0; slot +0x198 count=3, then free buffer |
| H-R2 | empty map, invoke GetValue directly | output slot +0x198 count=0, buffer free; no invented text or graph behavior |
| H-S1 | map (0,2), (16,10), (255,10); graph query valid | graph sample 0=2 raw; sample 1=(10-2)*1/16+2=2.5; sample 8=6; sample 16=10 raw; sample 255=10 raw; exactly 256 +0x50 calls then +0x6C(0), Release |
| H-S2 | same map and addPoint(16,10) | equal value still stored and display called; summary prefix then all graph samples if controls present |
| H-B1 | add, X child missing, Y child present | X lookup then Y lookup; no text read or point mutation; AL=1 |
| H-B2 | add, X parser result 17, Y child missing | call addPoint(17,+0); admitted store/refresh; AL=1 |
| G-I1 | held float words for (8,3),(-2,9),(8,-4) | import preserves three records and their order/duplicate X; readback copies six words, count=6; no normalization |
| G-I2 | held count=3 with only three valid float words | requested vector size 1, loop attempts second pair and fourth source word; outside normal domain, no portable success output |
| G-A1 | empty vector; X text parser yields 3.5; Y child missing | second parse uses X text; insert (3.5,3.5), refresh, AL=1 |
| G-A2 | points (-2,7),(4,9); parsed pair (1,13) | no epsilon match at -2 or 4; insert before 4; resulting order (-2,7),(1,13),(4,9); refresh; AL=1 |
| G-A3 | points (1,7),(4,9); parsed X=1+2^-20, Y=13 | first difference 2^-20 < binary64 epsilon; first Y becomes13, X stays exactly1; refresh; AL=1 |
| G-A4 | points (1,13); parsed pair (1,13) | same Y still stored; refresh; AL=1 |
| G-A5 | points (1,7); parsed X=1+2^-16, Y=-6 | difference 2^-16 > epsilon; append (1+2^-16,-6); no clamping |
| G-D1 | points (-2,7),(1,13),(4,9); delete parsed X=4 | erase last; refresh; SetCurrentPoint(1,13); AL=1 |
| G-D2 | single (4,9); delete X=4 | erase; refresh (GeneralResponse graph empty policy still separately qualified); no SetCurrentPoint; AL=1 |
| G-D3 | points (1,13); delete X=2 | no match, no refresh/control set; AL=0 |
| G-N1 | points (-2,7),(1,13),(4,9); previous at -2; separately next at4 | select (4,9); separately select (-2,7); no point mutation/refresh; AL=1 |
| G-N2 | single (4,9); previous or next at4 | select same pair and set controls again; AL=1 |
| G-C1 | SetCurrentPoint(3.5,-6.25), X child absent | no Y lookup/set, no collection change |
| G-C2 | same, X valid and Y absent | format/set X to `3.500000`, release X; Y lookup fails; no undo of X set |
| G-F1 | X child exists but query leaves a null output pointer | next interface dereference is unsupported; no claimed parser/default/rollback result |
| G-V1 | GetValue on empty vector | output slot +0x1B8 with count0, free buffer; no point read |
| G-V2 | stored pairs (8,3),(-2,9),(8,-4) | output words 0x41000000,0x40400000,0xC0000000,0x41100000,0x41000000,0xC0800000, count6 |
| G-P1 | graph valid, empty vector, valid service | X bounds0..1; Y lower +0 and upper binary32 0x3F8CCCCD; both ticks0x3E4CCCCD; no per-record calls; +0x6C(0), release graph then axes |
| G-P2 | graph valid, one point(4,-6) | X bounds4..5; Y extrema first -6..-6 widen upper to-5; both ticks FSTP32(1*0x3E4CCCCD); Y bounds FSTP32(-6*0x3F8CCCCD)..FSTP32(-5*0x3F666666); one raw point callback |
| H-T1 | formatter input word 0xFFFEFF85 | key -2 and unsigned low65413; formatted scaled value rounds to654.13 under ordinary formatter/nearest mode, rather than Init's negative1.23 |
| G-T1 | formatter words for (3.5,-6.25), extra unmatched float7; count3 | clear then append `(3.50,-6.25) `; third word ignored by formatter; AL=1 |
| R-Q1 | same receiver/output storage, two successful special-interface queries | output assigned object+0x100 twice; two AddRef calls, zero replacement Release calls |
| R-D1 | destructor flag0x100 vs0x101, normal live owner | both destroy own collection then held variant/window prefix; flag0x100 does not free object; flag0x101 does |

## 10. Per-body attribution ledger

`Owned` means the body-specific normal contract above is published, not that
every hostile pointer/exception path is supported. `Partial` explicitly keeps
the unread suffix or foreign primitive open. All native `this` receivers below
are complete primary pointers unless an adjustment is named. Full EAX or AL
is distinguished; incidental return-register contents are not boolean promises.

| Owner/body | Bytes / attribution | Args and result channel | Published state/effects; boundary |
|---|---|---|---|
| Happiness factory 0x006648F0 / constructor 0x006647F0 | 37 / 118, owned wrappers | factory pointer EAX; ctor ECX→EAX | allocation test, +0xD8 interface return; base ctor foreign, map sentinel state §1 |
| GeneralResponse factory 0x006642A0 / constructor 0x0066B630 | 37 / 68, owned wrappers | same | distinct zeroed vector state §1 |
| Happiness QI 0x0066B110 | 69, owned | ID dword/output pointer, RET8, AL | +0x100 write/acquire; delegated IDs foreign §1 |
| GeneralResponse QI 0x0066B190 | 64, owned | same | own match and unchecked null-receiver branch §1 |
| Happiness command 0x0066B160 | 30, folded/shared name cSCEditValueDlg | selector/action dwords, RET8, AL | selector gate/virtual dispatch §3; no exclusive owner byte credit |
| GeneralResponse command 0x0066DCE0 | 28, owned | selector/action dwords, RET8, AL | selector/direct button dispatch §3 |
| Happiness button 0x0066B9B0 | 406, owned | action dword, RET4, AL=1 | parse/control/action/count gates §5.1; text and parser primitives foreign |
| GeneralResponse button 0x0066CE10 | 970, owned | action dword, RET4, AL | insert/replace/delete/wrap sequencing §5.2; vector memory helpers foreign |
| Happiness add 0x0066CDB0 | 89, owned | key low signed word/Y binary32, RET8, no boolean guarantee | comparisons/raw store/refresh §2 |
| Happiness key-only mutation 0x0066BD30 | 53, owned, misleading SetValue name | low signed key word, RET4 | erase count/refresh §2/§4.1 |
| Happiness map lookup 0x0066C570 / erase wrapper 0x0066B710 | 121 / 83, container specialization/folded | key pointer; value pointer EAX / count EAX | signed comparisons/equal-key association; helper algorithms foreign §4.1 |
| Happiness GetValue 0x0066BD70 | 193, owned | entry ECX=object+0xD8, output pointer, RET4 | bias conversion, packed order/count, free schedule §4.1 |
| GeneralResponse GetValue 0x0066B220 | 286, owned | same | raw two-dword records/doubled count/free §4.2 |
| Both SetValue 0x0066B180→0x00687950 | 5→8, shared thunk/base wrapper | entry ECX, variant input pointer, foreign assignment result | held variant +0xDC only §4; foreign assignment alias/null/copy rules |
| Both metadata 0x00664010 / 0x00664020 | 10 / 12, shared | input dword / output pointer, RET4 | raw store/load +0xFC; no owner ref semantics |
| Happiness display 0x0066BB50 | 477, owned | ECX, no result contract | summary then 256 samples; unsafe missing-endpoint domains §6 |
| GeneralResponse display 0x0066C180 | 998, owned | ECX, no result contract | summary/service/bounds/record callbacks/release §7; graph drawing foreign |
| GeneralResponse SetCurrentPoint 0x0066C060 | 277, owned | X/Y binary32 dwords, RET8 | formatted X then nested Y controls, no point store §5.2 |
| Happiness formatter 0x0066BE40 | 327, owned | three stack dwords, RET0xC, AL=1 | unsigned low word text callback §8.1 |
| GeneralResponse formatter 0x0066BF90 | 204, owned | same | clear/complete-pair formatting §8.1 |
| Happiness Init 0x0066C5F0 | 1972 metadata bytes, ordinary flow owned | ECX; flag AL; final base-Init AL, RET | §4.1 import; §12 controls/geometry/ref/normal failure prefixes; empty refresh and foreign primitives remain qualified boundaries |
| GeneralResponse Init 0x0066D240 | 2715, **partial** | ECX, AL guard known; final result unqualified | flags/base-init/import prefix §4.2; UI creation/geometry/final selection not fully qualified |
| GeneralResponse resize 0x0066D1E0 | 85, vector specialization | size dword, RET4 | zero pair growth/truncation request §4.2; foreign storage operations |
| GeneralResponse paint 0x0066B1D0 | 71, owned wrapper | ECX; result foreign tail | AL surface gate and raw calls §7.3 |
| Happiness destructor 0x006648A0 | 69, owned | low-byte flag bit0, RET4, original pointer EAX | map then base then optional object free §8.2 |
| GeneralResponse destructor 0x0066B990 / inner 0x0066B6B0 | 30 / 83, owned | flag as above / ECX | vector buffer then held variant/window then optional object free §8.2 |
| Both ref/QI adjusted thunks | individual 11-byte adjustments, shared where stated | ECX−0xD8 or −0x100, result delegated | no independent curve counter; §1/§8.2 |
| Base ctor/dtor and window/variant/ref primitives | foreign/shared, no owner body credit | raw call channels cited above | inherited behavior is not reset or republished as curve-owned |

Direct caller evidence: 0x0066D1B0 is the export's single direct caller of
SetCurrentPoint; addPoint has zero direct callers because the button invokes
primary virtual +0x264. Vtable dispatch, not direct-call absence, establishes
that inventory. These per-body sizes are identifiers, not summed whole-family
coverage; the historical approximate 8000-byte number is not a denominator.

## 11. Coverage and open work

Published behavior includes owner import/readback schedules, point mutation,
button decision paths, both displays, formatting, destruction wrappers and
Happiness's interpolation and its finite ordinary Init control/geometry/failure
prefixes (§12). GeneralResponse Init remains partial. Foreign variant alias/null behavior, x87 environment
and callback reentry outside the normal domain are not silently filled in.
The two remaining work packages and body-scoped questions live in
[the question file](questions/curve-editor-views.md). No replacement source,
test suite or native execution supplied expected values.

## 12. Init: shared control boundary and Happiness ordinary flow

The Init rules specify individual foreign operations, not a service that returns
a completed editor. Window slots +0xA4/+0xA8 supply width/height, +0xE0 takes
(x,y), +0xCC takes width, +0xDC takes (left,top,right,bottom), +0x100 takes
the raw ID, +0x110 takes (flag,value), and owner +0x38 attaches a window.
These names identify call inputs, not successful primitive effects. Unless a
test is expressly stated, returned status registers do not affect owner flow.
Normal flow requires each subsequently dereferenced pointer to remain valid;
callbacks, nonreturns and invalid pointers are outside this finite contract.

### 12.1 Shared helpers, counted once

These helpers are shared property-view bodies, not extra bytes of either Init.
They retain raw creation/text/ref/geometry boundaries. They do not authorize
research into framework, resource-loader, screen, base-window or graph algorithms.

| Helper | Raw request and local order | Returned-failure boundary |
|---|---|---|
| Button 0x0066E9F0, 313 metadata bytes | With the two resource-key words zero, request WCM +0x28 (kind 2, supplied ID, supplied text,0,0,0); store full EAX in caller output; call button +0x84; +0x14(0x100,1), (0x200,0), (0x400,0); +0x8C(text); free empty scratch string and destroy temporary resource buffer | No returned-pointer/status test before dereference. These Init callers always take the zero-resource branch, not the resource-loading alternative. Kind 2 is the raw creation selector, not a proved COM class/IID. |
| Label 0x0066DDB0, 219 metadata bytes | Clear output; GZCOM +0xC(class 0x592,IID 0x212CDC1F,out); test AL. On nonzero, get window via label +0xC; obtain colour with window +0x1B0(0xE0,0xE0,0xE0); label +0x1C(2,0), (4,0); label +0x48(style); +0x28(font); window +0x1A8(colour); +0x128(text); label +0x14(0,0); window +0x110(0x10000,0) | AL=0 skips all configuration and returns with output as supplied by creation (initially cleared). Caller still uses it. Style zero requests WCM +0x14(0xC8963C5C); no status test on that result. |
| Line input 0x0066DF60, 469 metadata bytes | GZCOM +0xC(class 0x231A1C58,IID 0x231A1C57,local out); test AL. Nonzero obtains two temporary screen-service references using (0xC416025C,0x0073283C), obtains colours with +0x88(0,0,0x80) then (0xFF,0xFF,0xFF), releasing each temporary after its colour call. Get input window via +0x14; compute bottom=y+18, then read parent width and compute right=width−10; window +0xDC(x,y,right,bottom). Request WCM +0x14(0xC8963C5C); input +0x24(result), +0x30(0,font), +0x48(font), +0x30(4,white), +0x30(5,dark colour), +0x5C(dark colour), +0xC0(text,0), +0x130(1), +0x130(0x200). Get window, set ID, get window, +0x110(0x10000,1), get window, parent attach, input release +8 | AL=0 returns without attachment. Framework lookup statuses and service pointer validity are not tested recovery paths. No line pointer is held in the owner by this helper. |

Label caller arguments are (output,text,font,style); these Init calls pass
owner +0xF0 as font and zero style. Line arguments are (x,y,parent,ID,text,font).
Caller text temporaries are destroyed after each helper returns, before the
next owner operation. For ordinary nonempty literals the owner allocates
length+1 bytes, copies exactly length bytes, and stores a zero terminator;
empty text still allocates eight bytes and writes its initial zero byte.
The caller tests the saved string-buffer pointer solely to decide whether to
free it; that test is not a widget-creation success test. Evidence:
0x0066DDB0–0x0066DE8A, 0x0066DF60–0x0066E134,
0x0066E9F0–0x0066EB28; only the button helper's reachable zero-key branch is
part of these Init obligations.

### 12.2 Happiness control and geometry ledger

Init 0x0066C5F0–0x0066CDA3 first allocates an eight-byte empty scratch buffer,
then tests the existing window init flag **AL**. Nonzero frees that buffer
and returns AL=1; no import, flags, creation, refresh or base Init follows.
Zero imports §4.1, then calls owner +0x110(0x800,0), (0x10000,1).
There is no endpoint insertion, public addPoint call, point-count requirement,
or empty-map guard anywhere between import and first refresh.

Each button/label below is a temporary held interface: obtain its window with
+0xC separately for each window operation, attach through owner +0x38, release
the held interface through +8, and clear the local pointer. Getter +0xC is not
paired with a release of every returned window. There is no persistent owned
control-pointer field written by this Init. Primitive attachment/reference
effects remain foreign; the owner calls just listed are the handoff proof.

Let A be Add's returned width; D be Delete's returned height; B and H be
Bar Value label's returned width and height; K be Weight label's returned
width; S be the empty status label's returned height. All geometry uses
32-bit integer arithmetic, with no owner clamp or minimum-size test.

| Step, instruction span | Control / raw inputs | Owner operation and saved geometry |
|---|---|---|
| H1, 0x0066C6AD–0x0066C768 | Button kind 2, ID 0x26C10A3B, text `Add`, resource words 0,0 | Free caller text; window position (5,5); read width A; save A+10; attach, release |
| H2, 0x0066C769–0x0066C827 | Button kind 2, ID 0x26C10A3C, text `Delete`, resource words 0,0 | Free caller text; position (A+10,5); read height D; save y1=D+10; attach, release |
| H3, 0x0066C828–0x0066C900 | Label class/IID §12.1, text `Bar Value:` | Free text; position (5,y1); read width B, save x1=B+10; read height H and save it separately; attach, release |
| H4, 0x0066C901–0x0066C972 | Line class/IID §12.1, ID 0x26C10A3E, empty text | Helper receives (x1,y1,owner,ID,text,font); free text; save y2=y1+H+5 |
| H5, 0x0066C973–0x0066CA2D | Label text `Weight:` | Free text; position (5,y2); read width K; save x2=K+10; attach, release. Its height is not read. |
| H6, 0x0066CA2E–0x0066CAA3 | Line ID 0x26C10A3F, empty text | Helper receives (x2,y2,owner,ID,text,font); free text; save y3=y2+H+5, reusing **Bar Value** height |
| H7, 0x0066CAA4–0x0066CB8C | Label empty text; window ID 0x26C10A3A | Free text; set ID, position (5,y3); read owner width W, request label width W−10; read label height S; save graphTop=y3+S+5; attach, release |
| H8, 0x0066CB8D–0x0066CBF4 | Allocate 0x260, construct foreign line graph 0x009BA988 if allocation nonzero; graph ID 0x26C10A3D | Allocation zero selects null then immediately dereferences it; no normal failed-allocation return. Acquire graph primary +4; set ID; read owner height, save height−5 as bottom; read owner width, compute width−5 as right; graph +0xDC(5,graphTop,right,bottom) |

Width/height reads occur at their steps, not from a snapshot. The two line
helpers each read parent width independently. H7's owner width, H8's owner
height and then width are later distinct calls. The spacing uses the named
returned dimensions, not button right/bottom coordinates or assumed fonts.

### 12.3 Happiness graph, completion and ordinary failure prefixes

The graph interface is the constructed graph +0xD8; there is **no** QueryInterface
or query-result gate in this Init. In order, it receives +0x4C(0,`Bar Value`,0),
then +0x4C(1,`Weight`,0), freeing each caller string after the call; +0x58(1,1);
+0x3C(0,binary32 +0); +0x40(0,255 [0x437F0000]);
+0x3C(1,binary32 +0); +0x40(1,30 [0x41F00000]);
+0x60(1,5 [0x40A00000]). These are raw graph configuration calls, not proof of
rendering, tick placement, interpolation or automatic endpoint generation.
At 0x0066CD23–0x0066CD64 two separate WCM lookups pass
(0x68963C4C,0x80000000) and (0xC8963C5C,0x80000000). The first returned word
is passed to embedded graph +0x22C slot +0x10; the second to graph-interface
+0x9C. Their statuses are ignored. Then attach the primary graph; release its
primary reference; call owner refresh +0x260; call base window Init 0x0099C2C3;
save **AL**, free the initial scratch buffer, restore that byte and return
(0x0066CD6A–0x0066CDA3). Upper EAX is not the Init result contract.

There are only two ordinary returns: already-initialized AL=1 and this final
base-Init AL, after all earlier effects. The import count loop and optional
string frees reconverge; allocation's two paths reconverge at a dereference.
No helper result, attach result, graph configuration status or refresh result
is tested here. A line creation returning AL=0 skips that helper's effects
and the owner continues with spacing and later controls. Label creation
returning AL=0 with null output reaches a caller dereference after text cleanup,
not a deterministic false return. Button null output is dereferenced inside
its helper. Final base Init AL=0 preserves all preceding ordinary effects;
there is no rollback or compensating detach in this body.

First refresh still has §6's empty-map unsupported/crash boundary; Init does
not repair it. An empty import with a preexisting nonempty live map retains
that map, while an empty live map is not a safe successful Init example.

### 12.4 Independent proposed Happiness Init traces (UNEXECUTED)

These are primitive input/return proposals, not native goldens or an owner
verdict supplied by a host. Primitive side effects are specified independently
of returned register bits; all pointers below are valid unless stated.

| Proposal | Independent inputs | Expected owner trace |
|---|---|---|
| H-asymmetric | Initial flag AL=0; import one packed word 0x00000064 into empty map (key 0, Y from signed 100 times binary32 0x3C23D70A); widths/heights A=31,D=17,B=43,H=11,K=29,S=13; every owner width read=317, height=193; label/line creations AL=1; no callback mutation | Buttons at (5,5),(41,5); Bar label (5,27); first line rect (53,27,307,45); Weight label (5,43); second line rect (39,43,307,61); status label (5,59), width 307; graph rect (5,77,312,188). Graph config order §12.3, attach/release, §6 refresh, base Init, scratch free, return its AL. No key 255 node inserted. |
| H-status | Same inputs; primitives perform the same effects but return arbitrary distinct full words for ignored flag/position/attach/config/refresh statuses; final base Init returns 0x13572400 | Same calls/stores/ref handoffs, final AL=0. High register bits never convert ignored statuses into early exits. |
| H-missing-lines | H-asymmetric, except first and/or second line GZCOM returns AL=0 and creates nothing | Zero or one line attaches, as selected independently; both labels, spacing, status and graph remain in the owner trace. No retry/default input creation. |
| H-repeat | Flag getter returns 0x12345601; scratch allocator returns a valid eight-byte buffer | Allocate/zero scratch, query flag, free scratch, AL=1. No held-variant query, widget request or refresh. |
| H-empty-import | Flag clear; held variant count 0; retain an already valid one-node map; other inputs H-asymmetric | No import iteration; existing node survives; controls and first refresh follow unchanged. If live map is also empty, no successful refresh/result is specified. |
