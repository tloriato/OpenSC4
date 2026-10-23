# Curve entry views

Status: first dedicated native behavior publication, 2026-10-23, lane re-curve-editor-views4.
Evidence: Windows SimCity 4 Deluxe 1.1.641 native export queried with `re_query.py`;
instruction bytes govern widths and predicates. Local pseudocode is navigation only.
This is a bounded specification, not whole-family coverage of an approximate byte count.

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
Tree balancing/allocation are container primitives, not editor policy.

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
The key-only helper's precise erase/returned-count primitive remains open.

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

## 7. Independent literal ledger — PROPOSED / UNEXECUTED

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
| H-G | key-only slot +0x268 key 17; helper returns 0, then separately 2 | no refresh for 0; one refresh for 2; helper effects remain its boundary |
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

## 8. Coverage and open work

Published behavior includes owner import/readback schedules, point mutation,
button decision paths and Happiness's interpolation. Full Init control geometry,
GeneralResponse graph bounds/paint, formatting and destruction/reference
contracts are not yet complete. Precise body-scoped obligations live in
[the question file](questions/curve-editor-views.md). No replacement source,
test suite or native execution supplied expected values.
