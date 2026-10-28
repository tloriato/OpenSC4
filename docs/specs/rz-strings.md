# Framework strings and token replacement

Target: SimCity 4 Deluxe, Windows GOG 1.1.641. Owners: `cRZString` and
`cSCStringDetokenizer`. Requirements below are *[compat]* unless marked otherwise.
Addresses identify evidence in that build; they are not implementation dependencies.

This spec covers byte storage and the base token replacement service. See
[strings and language](strings-language.md#2-framework-string-encoding) for the
framework encoding and language manager, [language utility](language-utility.md) for
localized number/date formatting, [string encoding](string-encoding.md) for the
counted converters and wrappers, [LTEXT records and locale](data-formats.md#6-ltext-and-locale)
for resource decoding, and [crime names](crime.md) §7 for an existing
`SC4LoadString` caller contract. The LTEXT loader may clear its output after a
detokenizer failure; that is different from the detokenizer's partial-output rule here.
[Variant storage](variant-storage.md) documents string-valued variant ownership and
finite embedded-NUL copies. Do not replace any of these boundaries with a single
Unicode-string abstraction.

## 1. cRZString: identity, state and lifetime *[compat]*

The interoperable object occupies 0x14 bytes. Its vptr is at +0x00, buffer begin at
+0x04, logical end at +0x08, allocation end at +0x0C, and unsigned reference count
at +0x10. See [the ABI layout](../abi/layouts.md), “Game string”.
Some callers read this layout directly, so a foreign `cIGZString` implementation
is not interchangeable at those callers merely because it has the same methods.

The logical value is the counted byte range from begin to end. There is a separate
heap buffer, no inline storage, and a NUL byte at logical end. Interior NUL bytes
are allowed. `Strlen` is the byte count, not a C-string scan, glyph count or UTF-16
unit count. `ToChar` returns the borrowed buffer address. Its mutable and const
interface entries return the same address. A caller changing those bytes does
not automatically change the stored end pointer. Mutations may invalidate borrowed
pointers. Independent copies own their bytes; there is no shared-buffer reference
counting (`0x0090E4B6`, `0x009D7E63`, `0x0090F546`, `0x0044D3A0`).

Default construction produces an empty, terminated buffer with eight allocated
bytes and reference count zero (`0x00578390`–`0x005783C4`). Eight allocated bytes
includes space for termination; do not infer eight writable characters. Ordinary
C-string construction copies up to the first NUL and starts the count at zero
(`0x00408480`). Its input must be non-null; this constructor scans it before
assignment. A null-accepting setter is a different boundary. Counted construction
and range construction preserve interior NULs; the range-buffer helper at
`0x00408420` is not itself a complete 0x14-byte object constructor.

`QueryInterface` accepts unknown IID 1 and string IID `0x089B7DC8`, writes this
interface and adds a reference. An unsupported IID returns false without clearing
the supplied output (`0x0090E1D1`–`0x0090E1F2`). `AddRef` increments the 32-bit
count and returns it (`0x0094D34F`). `Release` decrements when the count is greater
than one; otherwise it invokes the deleting destructor and returns zero
(`0x0090E1F3`). Thus releasing at count zero also deletes. Stack/local native
objects are destroyed directly, not by balancing a constructor reference.
Destruction releases the character allocation.

Allocation failure, invalid pointers and unrepresentable byte counts have no
demonstrated recoverable Boolean error contract. They are not successful empty
assignments. A portable core's internal allocation strategy is *[choice]*; the
adapter must preserve the layout, ownership and termination contract. See RZS3
in [open questions](questions/rz-strings.md) for overlap and failure edges.

## 2. cRZString method inventory and dispatch *[compat]*

Slots are zero-based. The complete table at `0x00A80810` contains 29 entries;
offset is four times slot. Table bytes were checked against the executable and
the shared `cRZStringResource` entries in [ABI vtables](../abi/vtables.tsv).
Pointer/count inputs below are byte inputs. `s` denotes another string interface;
`sensitive` is a byte Boolean, with nonzero meaning case-sensitive.

| Slot | Entry address | Contract / arguments |
|---|---|---|
| 0 | 0x0090E1D1 | QueryInterface(iid, out) |
| 1 | 0x0094D34F | AddRef() |
| 2 | 0x0090E1F3 | Release() |
| 3 | 0x0090F546 | FromChar(pointer, count) |
| 4 | 0x0090F50F | FromChar(C string) |
| 5, 6 | 0x009D7E63 | ToChar(), mutable / const buffer access |
| 7 | 0x0090E4B6 | Strlen() |
| 8 | 0x0090ED5D | IsEqual(pointer, count, sensitive) |
| 9 | 0x0090E242 | Alias forwarding arguments to slot 10 |
| 10 | 0x0090EC57 | IsEqual(s, sensitive) |
| 11 | 0x0090EF5B | CompareTo(pointer, count, sensitive) |
| 12 | 0x0090EE61 | CompareTo(s, sensitive) |
| 13 | 0x0090E230 | Copy/assignment wrapper; invokes slot 14 and returns this |
| 14 | 0x0090E20A | Copy(s), through receiver slot 3 |
| 15 | 0x0090F042 | Resize(new length), zero-fill extension |
| 16 | 0x0090F06C | Append(s) |
| 17 | 0x0090F053 | Append(pointer, count) |
| 18 | 0x0090F0B0 | Insert(position, s) |
| 19 | 0x0090F093 | Insert(position, pointer, count) |
| 20 | 0x0090FC30 | Replace(position, s) |
| 21 | 0x0090FC13 | Replace(position, C string, erase count) |
| 22 | 0x0090E4D8 | Erase(position, count) |
| 23 | 0x0090F16C | Find(s, start, sensitive) |
| 24 | 0x0090F0DB | Find(C string, start, sensitive) |
| 25 | 0x0090F28C | RFind(s, start, sensitive) |
| 26 | 0x0090F1FB | RFind(C string, start, sensitive) |
| 27 | 0x0090F574 | Sprintf(format, varargs); native cdecl stack boundary |
| 28 | 0x0044BF10 | Scalar deleting destructor, deletion flag |

The misleading export label `cRZCOMDllDirector::Release` at `0x0090E242` is only
a tail dispatch to string slot 10. It is not a reference-count operation.
Similarly, slot 13 returns the receiver, not a newly allocated string.
Copy obtains source slot 7 and const buffer slot 6, and calls receiver slot 3
with the pointer and count; it does not retain the source interface.
Append, Insert, Replace and Erase return the receiver. FromChar and Copy have
no success Boolean. Resize exposes no reliable status result.

Nonvirtual/native helpers are part of the inventory too:

| Methods | Addresses | Rule / section |
|---|---|---|
| BeginsWith / EndsWith | 0x0090E443 / 0x0090E478 | Explicit byte prefix/suffix (§4) |
| LTrim / RTrim / Trim | 0x0090E869 / 0x0090E887 / 0x0090E8A6 | Space and tab only (§5) |
| MakeLower / MakeUpper | 0x0090EAC5 / 0x0090EB8E | Encoding-aware fallback (§5) |
| Mid / Left / Right | 0x0090EA57 / 0x0090F45F / 0x0090F4B3 | Independent substrings (§5) |
| Strcat / Strncpy | 0x0090EA42 / 0x0090F430 | C-string append / bounded copy (§3) |
| SplitToken | 0x0090E98C | Destructive literal delimiter split (§5) |
| SplitTokenDelimited / SplitTokenSeparated | 0x0090F335 / 0x0090F39D | Single-byte-separator splits into a caller string (§5) |
| Native assignment / append entry points | 0x00408A70 / 0x004081D0 | Buffer operations underlying owned copies; not additional interface slots |

Neither this owner nor the base detokenizer has a message subscription, message
handler, renderer, draw-context method, stream load/save method or asynchronous
completion contract. Their dispatch is synchronous method dispatch. String
resource loading and text drawing belong to their callers. There is no implicit
LTEXT read, language change notification or font measurement when bytes change.

### 2.1 Native substring and case-helper ABI (RZS8)

These five entries use x86 **thiscall**, with the native receiver address in
ECX. Mid/Left/Right return a native `cRZString` **by value**, using a hidden
caller-supplied result pointer. The following table gives the complete machine
boundary; stack offsets are relative to ESP on entry, including the return
address at +0. Arguments are four-byte stack words, pushed right to left.
Positions and counts are unsigned 32-bit byte quantities, not signed indices.

| Entry | +4 | +8 | +12 | Return / stack cleanup |
|---|---|---|---|---|
| Mid, `0x0090EA57` | Result-storage pointer | Start | Count | EAX = result pointer; callee removes 12 argument bytes |
| Left, `0x0090F45F` | Result-storage pointer | Count | — | EAX = result pointer; callee removes 8 argument bytes |
| Right, `0x0090F4B3` | Result-storage pointer | Count | — | EAX = result pointer; callee removes 8 argument bytes |
| MakeLower, `0x0090EAC5` | — | — | — | Void; no stack arguments |
| MakeUpper, `0x0090EB8E` | — | — | — | Void; no stack arguments |

Result storage is a separate, writable, suitably aligned **0x14-byte native
object slot**, initially unconstructed. The helper constructs it, including the
native vptr and reference count zero, and gives it its own character allocation;
the caller must not preconstruct it or treat the hidden pointer as an assignment
destination. The result survives destruction or mutation of the source. The
caller destroys the constructed result exactly once, including empty results;
for caller-owned storage, native destructor slot 28 with flag zero releases the
buffer without freeing the object slot. Do not call Release on such a local
object or delete its storage. Internal temporaries are the helper's obligation.
Construction/return/cleanup evidence is `0x0090EA9F`–`0x0090EAC2`,
`0x0090F48D`–`0x0090F4B0`, `0x0090F4E9`–`0x0090F50C`; the native result
constructor initializes vptr/count at `0x00408C0A`–`0x00408C10`, and non-deleting
scalar destruction is `0x0044BF64`–`0x0044BF8C`. The case helpers mutate the
receiver and have **no defined return value**: EAX is incidental, including on
the empty and wide-conversion paths, and must not be interpreted as the receiver,
a Boolean, or another owned object (`0x0090EB77`–`0x0090EB8D`,
`0x0090EC40`–`0x0090EC56`). Successful ordinary allocations and distinct valid
receiver/result storage are assumed; RZS3 still covers invalid alias/failure edges.

## 3. Assignment and edits *[compat]*

`FromChar(pointer,count)` replaces the entire value with exactly count bytes.
A null pointer clears regardless of count. `FromChar(C string)` also clears for
null input, otherwise copies through the first NUL. Both leave a terminating NUL
outside the logical range. `Copy(s)` uses the source's counted length and retains
interior NULs; subsequent source changes cannot change the copy.

`Resize(n)` changes logical length, truncating the suffix if n is smaller and
appending zero bytes if n is larger. It is not merely capacity reservation. The
new length is observable even when the new bytes are NUL (`0x0090F042` passes
zero as the extension fill byte to `0x00442E90`).

Append adds the supplied range after the old end. The object overload uses its
counted length and mutable buffer accessor. Insert adds the supplied range before
the byte at position, preserving the suffix; position equal to length appends.
Position beyond length reaches a range-error path, rather than clamping to end
(`0x0090E8B6`). Counted append/insert are not C-string operations. Do not apply
the `0xFFFFFFFF` scan convention of comparisons to every count-bearing method.

Erase removes min(count, length-position) bytes. Count `0xFFFFFFFF` therefore
erases to end. Erasing at length does nothing; a position greater than length
reaches the underlying string range-error path (`0x0090E4D8`, `0x004425C0`).

Replace has an unusual interface: its explicit count is the number of OLD bytes
to erase, not the number of source bytes to insert. It removes
min(erase count, length-position), inserts the entire NUL-terminated source, and
keeps the remaining suffix. Its object overload sets the erase count to
source.Strlen(), but still inserts only the source's C-string prefix
(`0x0090FC13`–`0x0090FC55`, `0x005C2EB0`). Position equal to length appends;
position beyond length reaches a range error. For example, replacing at 1 in
`abcdef` with `XY`, erase count 3, produces `aXYef`. Replacing at 1 with an
object holding bytes `X 00 Y` removes three old bytes but inserts just `X`.

`Strcat` scans its source for NUL before appending. `Strncpy` copies
min(requested count, source C-string length) bytes; it neither pads to the
requested count nor preserves source bytes after NUL. These are distinct from
counted FromChar (`0x0090EA42`, `0x0090F430`). Null is not demonstrated as valid
for these helpers, Append, Insert, Replace or comparison/search operands.

## 4. Equality, ordering and search *[compat]*

IsEqual first requires equal ORIGINAL counted lengths. In sensitive mode it then uses a
NUL-stopping comparison limited to that length. Thus equal-length `A 00 X` and
`A 00 Y` compare equal through these virtual methods. In insensitive mode it
lowers two independent counted copies, requires their resulting lengths to be
equal too, and uses a full byte comparison; the same two inputs differ.
Pointer/count equality and ordering accept count
`0xFFFFFFFF` to mean the source C-string length (`0x0090ED5D`, `0x0090EF5B`).
This asymmetry is intentional compatibility, not a claim that all native string
operators behave like slot 10. Sensitive branches were verified at
`0x0090EC57` and `0x0090ED5D`; the native count-aware equality discussed in
[variant storage](variant-storage.md) must retain its own contract.

CompareTo compares the common prefix, using the same sensitive NUL-stopping
comparison or lowered full-byte comparison. The insensitive prefix length is
the minimum of the two **resulting lowered lengths**, including truncation on
malformed encoding. If that result is zero, it returns
the difference between the ORIGINAL byte lengths. Negative/zero/positive convey
ordering; an unequal compared prefix returns exactly **-1 or +1**, according to
unsigned-byte ordering, never the arithmetic difference between mismatching bytes
(RZS9). This applies to both overloads and both sensitivity modes. The original
length difference remains observable for equal prefixes, so the whole operation
is not a normalized three-way comparator. The sensitive runtime's result is
normalized at `0x009F1146`–`0x009F1152`; the insensitive full-byte comparator
normalizes at `0x009F196B`–`0x009F1973`. Both owners preserve a nonzero result
(`0x0090EF3F`–`0x0090EF52`, `0x0090F02D`–`0x0090F03B`).
For example `ab` versus `abcd` returns -2, and equal-length `A 00 X` versus
`A 00 Y` returns zero in sensitive mode (`0x0090EE61`, `0x0090EF5B`).

Find returns the first byte position at or after start. RFind returns the last
position at or before start, limited to positions where the whole needle fits.
Failure is unsigned `0xFFFFFFFF`. Empty-needle Find returns start only when
**start < length**, and failure at or beyond the counted end (RZS11). Thus an
empty haystack fails even for an empty needle and start zero, in either
sensitivity mode and either needle overload. This is a native quirk: the forward
range search returns the candidate start for an empty needle, but its owner
maps a result equal to the haystack end to failure (`0x004426C9`–`0x004426D7`,
`0x0044282E`–`0x0044283C`). It is not an empty-haystack-only guard.
Empty-needle RFind returns
min(start,length). Both needle overloads ultimately scan the needle to its first
NUL: even an object needle does not use its stored length for the final search
(`0x00442850`, `0x005C2620`). The haystack remains a counted range, so a search
can find bytes after an interior NUL. Insensitive search lowers temporary copies
first, passes start **unchanged** to the search on the lowered haystack, and
returns that search's byte offset unchanged. Start, bounds and results therefore
refer to the resulting byte string. There is no translation to original offsets
when conversion changes a UTF-8 byte length or drops a malformed suffix (RZS2).
This applies to slots 23–26 (+0x5C through +0x68): pointer Find passes the original
start at `0x0090F12B`–`0x0090F134`, object RFind at
`0x0090F2DA`–`0x0090F2E3`; the other two use the same sequence at
`0x0090F1BA`–`0x0090F1C3` and `0x0090F24B`–`0x0090F254`.
The final needle-length scan stops at the first NUL **after** lowering too:
conversion may introduce that NUL. A malformed needle that lowers to empty uses
the empty-needle rules above. The receiver and object needle remain unchanged.

Conversion produces ordinary temporary strings, without an error result for
these owners to propagate. Thus insensitive IsEqual can accept equal original
lengths whose different malformed suffixes were discarded; it rejects unequal
original lengths even if their lowered strings are identical. CompareTo instead
compares the surviving lowered prefixes and then uses the original length
difference, not the lowered length difference. The post-conversion equality
length checks are `0x0090ECDB`–`0x0090ECEB` and `0x0090EDD9`–`0x0090EDE9`;
CompareTo's lowered minimum is `0x0090EEE5`–`0x0090EF09` and
`0x0090EFD4`–`0x0090EFF9`, with the original-length tie rule in §4 above.

BeginsWith(pointer,count) and EndsWith(pointer,count) compare exactly count
bytes, without folding and without stopping at NUL. They return false if count
exceeds the string length, and true for count zero. The suffix comparison starts
count bytes before the counted end (`0x0090E443`–`0x0090E4B5`).

## 5. Native transforms and formatting *[compat]*

LTrim removes leading bytes in the set `{0x20,0x09}`. RTrim removes trailing
bytes in the same set; Trim does both. Newline and carriage return are not trim
characters. Empty or all-space/tab inputs become empty. The set is stored at
`0x00ACE73C`; entries `0x0090E869`–`0x0090E8B5` pass that exact set.

Mid(start,count) returns an independently owned substring, clamping count to the
remaining bytes. Start at or beyond length returns empty, rather than invoking
the native range-error behavior of Insert/Erase. Left(count) and Right(count)
return at most count bytes; count >= length returns the whole value. All three
count bytes, not characters, and may split a multibyte sequence.

SplitToken(delimiter) searches for the first complete delimiter byte string.
When found, it returns the prefix and removes prefix plus delimiter from the
receiver. No match returns empty and leaves the receiver unchanged. A null or
empty delimiter returns a copy of the receiver without consuming it
(`0x0090E98C`). This is a literal substring delimiter, not a set of whitespace
characters. The separately named Delimited/Separated variants are not aliases.

SplitTokenDelimited(separator byte, destination string) and
SplitTokenSeparated(separator byte, destination string) take a single separator
byte first and the string to receive the token second, and return a Boolean
*[compat]*. Both are destructive on the receiver and replace (never append to)
the destination's old content:

- Delimited (`0x0090F335`) finds the first separator byte. When found, the
  destination becomes the text before it (possibly empty), the receiver loses that
  text and the separator, and the result is true. An empty field between two
  separators, or before the first one, is therefore returned as an empty token
  with true. When there is no separator byte, the destination takes the
  whole receiver, the receiver becomes empty and the result is false (so the last
  token comes with false, and an empty receiver gives an empty destination and
  false).
- Separated (`0x0090F39D`) skips leading separators first: while the receiver
  begins with the separator, the whole leading run is erased. It then finds the
  next separator. When one is found, the destination becomes the text before it and
  the receiver loses that text plus the entire following run of separator bytes
  (an all-separator remainder empties the receiver); the result is true. When there
  is no separator, a nonempty receiver moves wholesale to the destination, leaving
  the receiver empty, with true. An empty receiver (including one that held only
  separators) returns false and leaves the destination **unchanged**. Repeated
  separators therefore never produce empty tokens, and false means "nothing
  left".

Neither variant reads the separator as a string; both compare one byte
(`0x0090F335`–`0x0090F39A`, `0x0090F39D`–`0x0090F42D`).

MakeLower and MakeUpper transform in place. Bytes through 0x7E use runtime byte
classification/case mapping (ordinary A–Z/a–z folding in the C or an
ASCII-compatible locale). Encountering any byte >=0x7F switches to conversion of
the whole **current** string to UTF-16, runtime case mapping of individual 16-bit
units, and conversion back to framework encoding (`0x0090EAC5`, `0x0090EB8E`).
Section 5.2 defines the tables, failure handling and remaining runtime observation
for RZS2. These are counted transforms, not normalization or full Unicode case
folding. Conversion itself remains canonical in [string encoding](string-encoding.md),
including its nonstandard UTF-8/UTF-16 behavior and the prior MakeLower witness.

Sprintf replaces the receiver with formatted output and returns the receiver,
not a character count. It builds output separately and assigns the completed
byte range at `0x0090FBB1`–`0x0090FBDF`. An empty format clears the receiver.
It parses percent conversions, supports `%%`, flags `- + space 0 #`, width and
precision (numeric or `*`), and modifiers `h`, `l`, `ll`, `I64`. Conversion
families include integer `d i u o x X`, floating `e E f g G`, byte `c`, string
`s`, pointer `p` and count-store `n`. This is a custom formatter, not permission
to substitute arbitrary host printf behavior. Floating arguments occupy eight
bytes at the native varargs boundary, ordinary integers/pointers four.

Width is based on glyph counting rather than just the emitted byte length.
String precision limits the number of characters traversed through GetCharLength,
so a valid multibyte character is not intentionally cut in half. No precision
uses the source C-string length. The sign, alternate-prefix and padding machinery
is separate from the underlying number conversion; `-` cancels zero padding and
`+` cancels space-sign (`0x0090F64B`–`0x0090F65F`,
`0x0090FA19`–`0x0090FB78`). Exact malformed-format, negative-width, signed-extreme,
wide-conversion and `%n` quirks remain RZS5. The finite formatting vectors below
specify the settled subset and are not a complete printf equivalence claim.

### 5.1 Numeric conversion and assembly (RZS6)

These rules describe the bundled runtime in the original executable, with its
ordinary `.` decimal separator and successful allocations. They are not a request
to delegate the complete format to a host printf. The owner passes only `%`,
optional `#`, optional nonnegative precision, and the conversion letter to the
floating converter (`0x0090F83F`–`0x0090F8CA`). Width, alignment, zero padding and
positive signs belong to the owner. A floating argument is an IEEE binary64;
the instructions load, compare and store a double, not a binary32
(`0x0090F870`–`0x0090F89E`). Ordinary negative values are converted as magnitudes
and receive an outer minus sign, even when rounding produces zero.

| Conversion | Default / explicit precision | Digits and alternate form |
|---|---|---|
| `f` | Default 6; precision counts fractional digits | Fixed decimal notation, with a leading `0` below one; exactly that many fractional digits. At precision zero the point is absent unless `#` is present. |
| `e`, `E` | Default 6; precision counts fractional digits | One digit before the point; exactly that many after it. Exponent is `e` or `E`, always a sign and **three** decimal digits, including leading zeros. Zero uses exponent `+000`. At precision zero the point is absent unless `#` is present. |
| `g`, `G` | Default 6 significant digits; explicit zero means 1 | Round to the requested significant digits first. Use exponential notation when the resulting decimal exponent is below -4 or at least the precision, otherwise fixed notation. Remove trailing fractional zeros and then an empty point unless `#` is present. `#` retains the point and the significant-digit zeros. `G` changes the exponent letter, not the spelling of special-value letters. |

The runtime caps floating precision at **512**. Precision above 163 uses a
separate runtime allocation; if that allocation fails, the converter falls back
to 163. This is distinct from the owner's output-buffer growth. Defaults, the
zero-to-one `g` rule, cap and fallback are visible at `0x009F9572`–`0x009F96C6`;
alternate-point insertion and zero trimming at `0x009F96F5`–`0x009F9721`.
The `g` notation decision uses the exponent **after rounding**, including a
carry to a new power of ten (`0x009F6B86`–`0x009F6BBE`). Exponent construction
uses a three-digit template (`0x009F697F`–`0x009F69D6`).

Decimal conversion has a **17-significant-digit intermediate**, not an unlimited
exact expansion or shortest-round-trip spelling. The bundled legacy converter
scales the binary value using its 12-byte extended arithmetic, generates 17
significant decimal digits and a guard digit, and rounds the magnitude upward
when the guard is at least 5. Trailing zeros in that intermediate can be omitted;
they are supplied again when requested by the format. The requested decimal
rounding is then applied to that intermediate: retain precision+1 significant
digits for `e`, precision significant digits for `g`, or the digits through the
requested fractional place for `f`; pad missing digits with zeros. A first
discarded byte at least ASCII `5` increments the retained magnitude with decimal
carry. Thus decimal halfway cases round away from zero, not to even. Rounding
at a place wholly before the first significant digit produces zero, except when
that first digit is the guard and is at least 5. More than 17 requested significant
digits expose zero padding of the intermediate, not further binary64 expansion.
This two-stage rounding, including legacy extended scaling at the last digit,
is the numeric primitive's compatibility contract; a modern host's correctly
rounded arbitrary-precision or shortest converter is not an equivalent primitive.
Evidence: 17-digit request at `0x00A0279E`–`0x00A027AE`, intermediate rounding at
`0x00A04B63`–`0x00A04B89`, and final guard/carry and decimal-position adjustment
at `0x00A02687`–`0x00A026A8`.

#### 5.1.1 Finite decimal primitive (RZS7)

For every finite nonzero binary64 magnitude, the following defines the decimal
digits and decimal position consumed by §5.1. It uses integer arithmetic; no
host floating-point rounding mode, logarithm or decimal conversion is implicit.
The result is a digit string `D` and decimal position `d`, representing
`integer(D) × 10^(d-length(D))`. This is the legacy intermediate, before the
requested format precision is applied. Negative input uses its magnitude here;
the owner supplies its sign as already specified. Zero and non-finite inputs
remain governed by §5.1 above.

**Representation and exact input widening.** A working extended value is
`(M,E)`, with an unsigned **80-bit significand**, including an explicit leading
one, and a biased 15-bit exponent, bias 16383. Its magnitude is
`M × 2^(E-16383-79)`. The 12-byte storage holds little-endian `M` in bytes 0–9
and sign/exponent in bytes 10–11. This is software arithmetic, not x87's 64-bit
significand stored in a padded 12-byte slot. Initially the binary64 is widened
exactly to a 64-bit explicit significand and then extended with 16 zero low
bits. Equivalently, if its binary64 exponent field is `b` and fraction is `F`:

- Normal input: `M=(2^52+F) × 2^27`, `E=b+15360`.
- Subnormal input: let `L` be the bit length of nonzero `F`;
  `M=F × 2^(80-L)`, `E=15308+L`. Do not flush subnormals to zero.

Exact widening and subnormal normalization: `0x00A026C1`–`0x00A02772`;
insertion of the extra 16 low bits: `0x00A04A4D`, `0x00A04A67`–`0x00A04A80`.

**Legacy multiply.** All multiplications below use this same operation, rounding
after **each** multiplier. For the positive normal operands reached here,
split each significand into five unsigned 16-bit limbs, least significant first:
`A=sum(a_i × 2^(16i))`, `B=sum(b_j × 2^(16j))`. Form the 96-bit integer

`P = sum(a_i × b_j × 2^(16(i+j-4)))`, over exactly the pairs `i+j >= 4`.

There are fifteen such products. Products with `i+j < 4` are omitted entirely:
their carry is **not** added to `P`. Thus this is not generally
`floor(A×B / 2^64)`, nor an exact extended multiplication. Set the provisional
exponent to `E_A+E_B-16382`. Normalize `P` by left shifts until bit 95 is set,
subtracting one from the exponent for each shift. Split normalized `P` into
an 80-bit upper part and a 16-bit remainder. Increment the upper part when the
remainder exceeds `0x8000`, or equals `0x8000` and the upper part is odd.
This local rounding is nearest, ties to even, **on the truncated product**;
there is no sticky bit from omitted products. If incrementing carries beyond
80 bits, use `M=2^79` and increment the exponent. Otherwise the upper part is
the new `M`. All operands and products reached by finite binary64 extraction
are normal in this extended range; extended underflow/overflow branches are
not needed for this primitive. Evidence: fifteen-product accumulation
`0x00A04E42`–`0x00A04EB4`, normalization `0x00A04EB6`–`0x00A04EE7`, rounding
and storage `0x00A04F22`–`0x00A04F8C`. The later **decimal** guard rounding
remains half-up, a separate operation.

**Scaling constants.** The table gives the raw stored `(E,M)`, in hexadecimal,
for the powers needed by binary64 extraction. All significands contain exactly
20 hex digits; the leading integer bit is included. They are data constants,
not powers to regenerate using host arithmetic. Before using a selected table
entry, if its low 16 bits are at least `0x8000`, decrement its 32-bit field in
bits 16–47 by one, leaving the other fields unchanged. For every listed entry
to which this applies, this is simply `M := M-0x10000`. This adjustment occurs
on a temporary copy and does not change the table. Do not apply it to the
separate correction constant below. The argument preserving the low 16 bits of
the current value is enabled on this path (`0x00A04A75`, `0x00A04FE5`–`0x00A04FED`).

| k | Raw `10^k`: E : M | Raw `10^-k`: E : M |
|---|---|---|
| 1 | `4002:A0000000000000000000` | `3FFB:CCCCCCCCCCCCCCCDCCCD` |
| 2 | `4005:C8000000000000000000` | `3FF8:A3D70A3D70A3D70A3D71` |
| 3 | `4008:FA000000000000000000` | `3FF5:83126E978D4FDF3B645A` |
| 4 | `400C:9C400000000000000000` | `3FF1:D1B71758E219652CD3C3` |
| 5 | `400F:C3500000000000000000` | `3FEE:A7C5AC471B4784230FD0` |
| 6 | `4012:F4240000000000000000` | `3FEB:8637BD05AF6C69B6A640` |
| 7 | `4016:98968000000000000000` | `3FE7:D6BF94D5E57A42BC3D33` |
| 8 | `4019:BEBC2000000000000000` | `3FE4:ABCC77118461CEFDFDC2` |
| 16 | `4034:8E1BC9BF040000000000` | `3FC9:E69594BEC44DE15B4C2F` |
| 24 | `404E:D3C21BCECCEDA1000000` | `3FAF:9ABE14CD44753B53C492` |
| 32 | `4069:9DC5ADA82B70B59EF020` | `3F94:CFB11EAD453994BA67DE` |
| 40 | `4083:EB194F8E1AE525FD5DD0` | `3F7A:8B61313BBABCE2C62324` |
| 48 | `409E:AF298D050E4395D79671` | `3F5F:BB127C53B17EC1595561` |
| 56 | `40B9:82818F1281ED44A0BFF9` | `3F44:FB158592BE068D2FEED7` |
| 64 | `40D3:C2781F49FFCFA6D53CBF` | `3F2A:A87FEA27A539E9A53F24` |
| 128 | `41A8:93BA47C980E98CE0C66F` | `3E55:DDD0467C64BCE4A1AC7D` |
| 192 | `427C:E070F78D3927556B85BC` | `3D81:91FF83775423CC067B63` |
| 256 | `4351:AA7EEBFB9DF9DE8EDDBC` | `3CAC:C0314325637A193AFA91` |
| 320 | `4426:81842F29F2CCE376E6A1` | `3BD7:FD00B897478238D18921` |

The first seven positive entries occupy `0x00B19D00`–`0x00B19D48`, the next
seven start at `0x00B19D54`, and the 64-power group starts at `0x00B19DA8`;
negative groups start at `0x00B19E60`, `0x00B19EB4`, `0x00B19F08`. Entries
are 12 bytes apart. Selection, temporary adjustment and multiply are confirmed
at `0x00A04FF7`–`0x00A05031`.

**Decimal position and order of operations.** For the widened input, set `H`
to its top significand byte, `floor(M/2^72)`. The initial decimal-position
estimate is the signed integer

`d = floor((E×0x4D10 + 0x4D×(floor(E/256)+2H) - 0x134312F4) / 65536)`.

The division rounds toward negative infinity, as an arithmetic right shift
does; do not replace it with truncation toward zero. These integer operations
do not overflow signed 32 bits for finite binary64 inputs. Scale the working
value by `10^-d`, decomposing `abs(d)` in base eight. Visit the base-eight
digits from least significant to most significant. A nonzero digit `r` in
position `j` selects the table power `k=r×8^j`; select `10^-k` for positive
`d`, `10^k` for negative `d`. Apply its adjustment, then the legacy multiply,
before visiting the next digit. A zero digit performs no multiplication;
`d=0` performs none. Do not combine constants or reverse their order.

If the scaled exponent is at least `0x3FFF`, increment `d` once and apply one
additional legacy multiply with the fixed constant
`E=0x3FFB, M=0xCCCCCCCCCCCCCCCCCCCC`. This constant is slightly below one tenth
and differs from the table's raw and adjusted `10^-1`. There is no reciprocal
correction for a value just below a decimal boundary. Estimate and one-step
correction: `0x00A04A42`–`0x00A04AA2`; fixed constant:
`0x00A0496B`–`0x00A04997`.

**Digit extraction and decimal rounding.** Convert the final scaled `(M,E)`
to the unsigned fixed-point integer
`R = floor((M×2^8) / 2^(16382-E))`. Here `E <= 16382`; this conversion
discards low bits without rounding. Obtain eighteen successive digits: each
digit is the integer part of `10R / 2^88`, and the new residue is `10R modulo
2^88`. This is exact integer extraction from that fixed-point value, with no
further extended or binary64 rounding. Retain the first seventeen characters,
then increment their decimal magnitude if the eighteenth is at least `5`.
Propagate carry through `9`s; carry past the first character leaves `1` and
increments `d`. Omit trailing zeros in the resulting seventeen-character
field; missing digits at subsequent requested precision mean zeros. Extraction
can begin with `0` just below a boundary: do not insert a normalization step
that would change which eighteen digits are generated. The guard rounding
itself can correct that leading zero. Fixed-point setup:
`0x00A04AC9`–`0x00A04B07`; digit extraction:
`0x00A04B09`–`0x00A04B61`; guard, carry and trimming:
`0x00A04B63`–`0x00A04BC9`. The caller requests seventeen digits with the
fractional-mode flag off (`0x00A0279E`–`0x00A027AE`).

For example, exact binary64 `1000000000000000.25` (bits `430C6BF526340002`)
has estimate `d=15`. Its scaling uses adjusted `10^-7`, then adjusted
`10^-8`, then the separate correction constant, incrementing `d` to 16.
After those multiplies the significand is `CCCCCCCCCCCCDB362616`, exponent
`0x3FFB`. Its eighteen extracted characters are `100000000000000024`, so the
intermediate is `D=10000000000000002, d=16`. Exact rational conversion rounded
half-up directly to seventeen digits would instead end in `3`. At `%.2f`,
the legacy result is therefore `1000000000000000.20`, not `.25` or `.30`.
Even input `1.0` takes the correction multiply: its eighteen characters are
`099999999999999999`, which guard-round and trim to `D=1, d=1`. These quirks
are observable at requested precisions beyond the intermediate's digit count.

RZ41–RZ50 below are static deductions from the instructions and stored
constants, checked by integer reconstruction and exact rational comparison;
they are not native execution measurements.

#### 5.1.2 Signs, special values and owner field assembly

Both signs of input zero produce unsigned zero digits: the runtime normalizes
zero's sign (`0x00A04BBB`–`0x00A04BC9`) and the owner's less-than-zero test does
not extract a minus from negative zero. The owner's `+` or space flag can still
add its requested sign. A negative nonzero value that rounds to zero keeps the
owner's minus. The floating primitives do not emit C99 `inf`/`nan`: their
intermediate text is `1#INF` for infinity, `1#QNAN` for a quiet NaN, or `1#IND`
for the canonical negative indefinite NaN (binary64 `FFF8000000000000`).
The runtime also has `1#SNAN` for a signaling NaN, but the owner's x87 double
load/comparison can quiet signaling inputs before conversion; this is not a
payload-preserving stringification interface. Special text has decimal position
1 and passes through the same precision, point, rounding and zero-trimming rules
as digits, so default `f` gives `1.#INF00`, default `e` gives
`1.#INF00e+000`, and default `g` gives `1.#INF`. Precision can truncate or even
increment the letters: `%.2f` of positive infinity gives `1.#J` because the
discarded `N` exceeds ASCII `5` and increments retained `I`. Case conversion
letters do not lowercase these markers. Negative infinity uses the outer minus;
negative NaNs retain the runtime's inner minus because unordered comparison does
not extract it. Consequently `%+g` of canonical negative indefinite gives
`+-1.#IND`. These are compatibility quirks, not errors to normalize. Special
markers and sign classification: `0x00A049C1`–`0x00A04A40`; outer comparison:
`0x0090F875`–`0x0090F89A`; final runtime sign: `0x009F9722`–`0x009F972C`.

`p` consumes one 32-bit pointer value and produces exactly **eight uppercase
hexadecimal digits**, including `00000000` for null, without `0x`. The owner
calls the runtime with bare `%p`; its precision, `#`, `+` and space flags do not
change the pointer digits or add a prefix/sign. Width and `-`/`0` still apply to
the eight digits. Evidence: pointer dispatch `0x0090F97A`–`0x0090F984`, runtime
eight-digit precision and hexadecimal selection `0x009F97B5`–`0x009F97CA`.

Integer digits are obtained without forwarding precision, width or `#` to the
runtime (`0x0090F7B6`–`0x0090F824`, `0x0090F99F`–`0x0090FA16`). Consequently
zero with zero precision is still `0`, for every integer radix. Integer
precision is a minimum **digit count** and inserts leading zeros; it never
suppresses digits. Alternate `o` unconditionally adds one extra `0`; alternate
`x`/`X` unconditionally adds `0x`/`0X`, including for zero and even when octal
precision already supplies a leading zero. Prefix bytes do not count toward
the precision. Unlike conventional printf, numeric precision does **not**
disable `0` field padding. Width accounts for digits, precision zeros, prefix
and sign. With right alignment, field padding precedes the prefix, then come
precision zeros and digits; zero field padding can therefore occur **before**
`0x`. A sign precedes zero field padding, but follows space field padding.
With `-`, zero padding is disabled and any width deficit becomes trailing spaces.
Only signed integer/floating families acquire the `+`/space sign.
In particular, `%#010x` with unsigned 3473 produces `000000xd91` (RZS10):
three digits plus the two-byte `0x` prefix leave five field-padding zeros;
the prefix's own zero follows those five zeros. `%#010X` produces `000000XD91`.
`%#010.5x` also produces ten bytes, with different prefix placement: three field zeros,
then `0x`, then two precision zeros, then `d91`, namely `0000x00d91`.
The latter illustrates that field zeros and precision zeros occupy different
positions even though their total count is unchanged.
Evidence: precision/prefix selection `0x0090FA42`–`0x0090FA8A`, sign and width
accounting `0x0090FA8C`–`0x0090FAFE`, and prefix/precision/digit emission
`0x0090FB03`–`0x0090FB78`.

### 5.2 Case conversion, failures and runtime boundary (RZS2)

**Owner order and conversion failure.** Scan the counted receiver, including
interior NULs, and apply the byte classification/mapping before advancing to the
next byte. On the first byte >=0x7F, stop that scan and convert the entire current
receiver, including any already modified prefix. Reserve N UTF-16 words, where
N is its original counted byte length: N <=256 uses the 512-byte stack buffer;
larger N uses a temporary word vector. Call the framework-to-raw wrapper
`0x009113EF` with target 0x10 and capacity N. Ignore its consumed-source return;
the updated capacity is the **produced word count**. Map exactly those words,
then unconditionally call raw-to-framework wrapper `0x00911358` with that count
and source 0x10, replacing the receiver. Ignore this wrapper's return too.
Evidence: lower `0x0090EAE4`–`0x0090EB72`, upper
`0x0090EBAD`–`0x0090EC3B`; both end as void methods (§2.1).

The custom 8-to-0x10 conversion stops before an invalid/incomplete sequence,
with only its valid prefix produced. No replacement character, retry, original
suffix retention or rollback is added by either owner. Zero words still go
through the reverse wrapper, so failure at the first sequence **clears** the
receiver. Interior zero words are mapped and encoded as counted zero bytes, not
terminators. The reverse wrapper assigns its reported byte prefix even for a
partial result ([string encoding §4.1](string-encoding.md#41-raw-source-to-framework-string-00911358)).
With ordinary successful allocation its 6-bytes-per-word scratch capacity is
sufficient for every 16-bit unit, so the reverse custom conversion completes.
Allocation/overlap failure remains RZS3. This rule composes the prior decoder;
it does not revise that decoder or replace the prior Latin-1 `Café` → `caf`
MakeLower witness with a new execution claim.

**Exact wide-case primitive.** Lower calls `0x009F4C35`, upper `0x009F4CE1`.
Each call obtains the CRT thread's locale-info pointer from thread data +0x64,
refreshing it against the global locale pointer at `0x00B1921C`. Within that
locale info, +0x14 is the LC_CTYPE locale handle and +0x04 its code page. This
is the C runtime locale, not merely the game's stored language ID: successful
language switching requests `setlocale(LC_ALL,name)` at `0x0092806D`; a failed
request leaves the previous runtime locale. See [strings and language §4.2](strings-language.md#42-switching-language-and-country-compat).

| Condition for the input 16-bit unit W | Lower result | Upper result |
|---|---|---|
| W = FFFF | FFFF | FFFF |
| LC_CTYPE handle = 0 (C locale) | W+0020 only for 0041..005A; otherwise W | W-0020 only for 0061..007A; otherwise W |
| Nonzero handle, W <0100, classification mask below absent | W | W |
| Nonzero handle, eligible W | One-unit runtime mapping with flag 00000100 | One-unit runtime mapping with flag 00000200 |

The sub-0100 gate reads the shipped wide classification table through
`0x00B19714` (initial table VA `0x00AEA762`). Lower requires bit 0001, present
exactly at **0041..005A, 00C0..00D6, 00D8..00DE**. Upper requires bit 0002,
present exactly at **0061..007A, 00DF..00F6, 00F8..00FF**. Other sub-0100 units
are returned unchanged, even if a host Unicode library would map them; e.g.
00B5 does not reach the upper mapper. Units >=0100 other than FFFF reach the
mapper without this classification test. Evidence: C branches
`0x009F4C4A`–`0x009F4C75`, `0x009F4CF6`–`0x009F4D16`; non-C gates
`0x009F4BD8`–`0x009F4C07`, `0x009F4C84`–`0x009F4CB3`; table access
`0x00A01DE7`–`0x00A01DFD`. The ranges above are read from the original PE table,
not inferred from a modern Unicode database.

The mapping call is the bundled CRT wrapper `0x00A01B82`, with the active
LC_CTYPE handle, the flag above, source count **1**, destination capacity **1
word**, and the locale code page. On a nonzero wrapper return the caller takes
the one destination word; on zero it keeps W, ignoring anything written to the
destination (`0x009F4C07`–`0x009F4C32`, `0x009F4CB3`–`0x009F4CDE`). Thus an
unrepresentable multi-unit mapping is not an expansion: if the one-word request
fails, that unit stays unchanged. No surrogate pair assembly or cross-unit
context is supplied. Only AX is stored back.

The wrapper probes Windows `LCMapStringW` and caches support at `0x00BADAE0`:
1 means the wide API succeeded, 2 means its probe failed with error 120
(CALL_NOT_IMPLEMENTED). On the supported-wide path it forwards the locale,
flags, one-word source and one-word destination directly to that API
(`0x00A01B93`–`0x00A01C13`, import at `0x00A801E4`). On the unsupported-wide
path it bridges through the locale's ANSI code page, `WideCharToMultiByte`
(flags 0), `LCMapStringA` with the requested case flag, then
`MultiByteToWideChar` (flags 1); a failed bridge returns zero. A probe
failure other than 120 leaves support state zero and also tries this ANSI bridge
for the current request; the next request probes again. States other than 0, 1
and 2 return zero (`0x00A01C18`–`0x00A01C75`,
`0x00A01CDA`–`0x00A01DAD`).

**What static evidence cannot make into a fixed table.** Non-C mappings are
Windows NLS replies for the active locale and installed Windows build. They are
not a case-pair table shipped in SimCity4.exe. No exact non-C table or default
locale for an unspecified running installation is inferred, and no host Unicode
folding substitution is authorized. The settled C table, classification gate,
wrapper inputs, failure identity and owner/search behavior above can be
implemented independently of obtaining those replies.

To settle a particular runtime table, record the original process's OS/NLS
version, CRT locale name, the actual thread locale-info +0x14/+0x04 values and
the wrapper support state at the string call. For every 0000..FFFF unit, obtain
the original lower and upper result under that unchanged locale; for eligible
units also record the mapping wrapper's return count and output word on the
one-source/one-destination-word request. Record zero-return cases separately
from successful identity mappings. For registry collation, record the same
locale's 256 results of `___tolower_mt(locale,unsigned byte)` and its +0x24
ASCII fast-path flag, +0x28 maximum character width and +0x48 classification
table (§7.2). These observations, with per-call input/output bytes for RZ70–RZ73,
would settle the installation-specific remainder of RZS2. They have **not** been
performed in this static documentation lane. The explicit mapping replies in
the vectors below are primitive inputs, not claims about every Windows version.

## 6. cSCStringDetokenizer: state, interface and lifetime *[compat]*

The base object holds a vptr at +0x00, unsigned reference count at +0x04, a
case-insensitive token-to-value registry rooted at +0x08, a token-to-method
registry rooted at +0x14, unknown-method set rooted at +0x20, and HTML-clean byte
at +0x2C. Construction starts with count zero, all three collections empty and
HTML cleaning off (`0x00601340`–`0x006013C5`). Values and names are copied;
callbacks carry an unowned function pointer plus 32-bit context. No automatic
AddRef/Release contract exists for that opaque context.

QueryInterface accepts 1 and `0x8A2C3240`, returns this and AddRefs; on failure
it leaves the output alone (`0x005FFE60`–`0x005FFE84`). AddRef increments the
32-bit count. Release decrements if count >1, otherwise destroys and returns zero,
including when called at count zero (`0x005FFE90`). Destruction empties all three
registries and frees owned string storage (`0x00600F30`).

The base table is `0x00AA6DFC`. The default game subclass uses `0x00A86C68` and
overrides initialization; its city/population/year and other built-in bindings
are outside the base registry contract. Do not create such bindings implicitly
in a base instance.

| Slot | Address | Contract |
|---|---|---|
| 0 | 0x005FFE60 | QueryInterface(iid,out) |
| 1 | 0x005F47F0 | AddRef() |
| 2 | 0x005FFE90 | Release() |
| 3, 4 | 0x00648F00 | Base lifecycle hooks: return true, no state change |
| 5 | 0x00601AA0 | SetTokenReplacementValue(name,value) |
| 6 | 0x00601B80 | SetTokenReplacementMethod(name,function,context) |
| 7 | 0x00600410 | AddUnknownTokenReplacementMethod(function,context,enable) |
| 8 | 0x005FFEE0 | Detokenize(resource key,output string) |
| 9 | 0x005FFEB0 | Detokenize(input string,output string), tail dispatch to slot 13 |
| 10 | 0x005FFEC0 | SetHTMLCleanMode(byte) |
| 11 | 0x005FFED0 | GetHTMLCleanMode(), stored byte |
| 12 | 0x006013D0 | Deleting destructor |
| 13 | 0x00600810 | PerformDetokenize(input,output), Boolean result |
| 14 | 0x00600C20 | ConvertToken(trimmed token,output), Boolean result |

Offsets are slot ×4. Despite its export name `operator.new`, `0x00648F00` is
just a true-returning stub (three instruction bytes), not an allocator. Slot 9
dispatches virtually through slot 13, and parsing dispatches virtually through
slot 14. Overrides must therefore be observed; calling a private copy of the
base parser/converter bypasses the interface contract.

## 7. Registration and replacement dispatch *[compat]*

Registry key comparison is a runtime bytewise case-insensitive comparison over
the common counted prefix, followed by original byte-length ordering. It uses
the runtime `strnicmp` family, not cRZString::MakeLower (§7.2). In the C or an
ASCII-compatible runtime locale,
`City`, `city` and `CITY` identify the same key. Registration does not trim the
name or strip `#`; parsing trims before lookup. Thus registering ` City ` does
not make it match the parsed token `# City #`.

SetTokenReplacementValue copies name and value through their string interfaces.
Setting an equivalent key replaces its previous value. Setting a method copies
the name and replaces its function/context pair. The two registries coexist:
neither setter removes the corresponding entry from the other registry
(`0x00601AA0`, `0x00601B80`, lookup `0x00600200`, comparator `0x006000F0`).

Unknown-method registration enables or removes a record. The collection is
ordered by the unsigned native function address, not registration order. Identity
is the function address: re-enabling the same function with a new context does
not replace its existing context. Disabling that function removes it regardless
of the supplied context. This follows the shared unique-insert comparator
(`0x00600410`, `0x004FB6C0`, comparisons at `0x004FB738`–`0x004FB73D`). A
portable implementation can model deterministic ordered callback identities
*[choice]*, but the native adapter must preserve native address order.

ConvertToken applies the following priority:

1. A stored value wins and is copied to the output; return true.
2. Otherwise, call the named method if present. A true result accepts its output.
3. If there is no named method, or it returns false, try unknown methods in
   ascending function-address order. Stop at the first true result.
4. If none accepts, replace the output with the token name alone and return false.

Each callback is a synchronous native cdecl call with three arguments:
`(token string interface, mutable output string interface, 32-bit context)`;
its Boolean result is in AL. The same output object is passed to successive
callbacks. A false callback is not rolled back or cleared before the next one.
However, final unhandled fallback overwrites it with the token name. Never call
a null function as a request to remove a named method: no null-method guard is
present (`0x00600C8C`–`0x00600C98`). Callback mutation follows the live traversal
rules in §7.1; there is no registration snapshot.

With HTML-clean mode nonzero, the selected result is cleaned after value lookup,
accepted callback, OR unhandled fallback. Each `"` becomes `&quot;`, `&` becomes
`&amp;`, `<` becomes `&lt;`, and `>` becomes `&gt;`; other bytes pass through.
This is one pass over the original replacement and does not re-escape newly
inserted entity bytes. Existing `&lt;` therefore becomes `&amp;lt;`. The cleaner
scans to the first NUL and overwrites the output, dropping subsequent counted
bytes (`0x00600640`–`0x006007DB`, constants `0x00AA6DBC`–`0x00AA6DD8`). Literal
text outside tokens is never cleaned by this mode.

### 7.1 Callback mutation and recursive calls (RZS1)

Slot 14 (+0x38) retains an unknown-method **node iterator**, not a copied list of
function/context pairs. After a named callback returns false it reads the current
unknown-set count at +0x24 and obtains its then-current first node from the header
at +0x20 (`0x00600CB5`–`0x00600CC8`). Each unknown call loads the function at node
+0x10 and context at node +0x14. A true result leaves the traversal immediately;
only a false result advances the iterator through the node's current tree links
(`0x00600CD0`–`0x00600CEF`, successor helper `0x005D17C0`–`0x005D17FD`).

Provided the object, token and output remain alive, mutation has these results:

| Change inside the current callback | Effect on this conversion |
|---|---|
| Unknown callback disables itself through slot 7 (+0x1C), then returns true | Its output is accepted. The removed node is not read again by this invocation. The registration is absent on subsequent conversions. |
| Unknown callback disables itself, then returns false | **Outside the supported domain:** erasure frees the current node before the successor helper dereferences it. There is no defined next callback, false result, exception or crash guarantee. Removing and re-enabling the same address does not repair this guarantee. |
| Disable a different unknown function, whether earlier or later in address order | Erasure preserves the current node. A removed later function is skipped; a removed earlier function is not revisited. Disabling an absent function has no effect. The supplied removal context is ignored. |
| Enable a previously absent unknown function while retaining the current node | If the current callback returns false, a new address greater than the current address is eligible in this same pass, in its new sorted position. An address less than the current one is not revisited. A true return ends the pass before either can run. |
| Enable the current or another already registered unknown function with a new context | Unique insertion retains the existing context. To change it requires removal and insertion; the current-node/false-return restriction still applies. |
| A named callback enables/disables unknown functions, then returns false | The unknown traversal starts from the updated set, including newly inserted addresses below every previous address. It has not yet acquired an unknown iterator. |
| Set a stored value or replace a named function/context through slots 5/6 (+0x14/+0x18) | The current function/context call has already been selected. Its return still decides this attempt. Neither the value lookup nor named-method lookup is repeated; a false result proceeds to unknown callbacks. Subsequent or recursive conversions see the new registrations and the ordinary value-before-method priority. |

Slot 7's disable path erases and frees synchronously, including its whole-set
case (`0x00600441`, `0x006002C8`–`0x006002F1`, `0x00600347`–`0x00600360`,
`0x006000C0`–`0x006000DB`). Erasure transplants tree links, rather than copying a
successor's callback into the erased node (`0x0040DB45`–`0x0040DB9D`); this is why
removing a different node preserves the active iterator. Insertion is the unique
insertion in §7. Named replacement stores the new pair at
`0x00601BDF`–`0x00601BE2`; the active call reads its pair before dispatch at
`0x00600C8C`–`0x00600C92`.

Recursive slot 9/13 or slot 14 calls on the same live detokenizer run immediately
on the calling thread. There is no busy flag, depth counter, lock, deferred
registration queue or cycle detection. Each parser invocation owns its input
copy, accumulated result and per-token replacement string; the latter starts
empty for each complete token (`0x00600810`, `0x00600A6A`–`0x00600AC5`). Registries
and the HTML byte at +0x2C are shared. A separate inner output therefore does not
clear the outer callback's output; explicitly passing that same output aliases
it and permits the inner conversion to overwrite it. The outer traversal resumes
after the recursive call, with any registry changes already applied. In
particular, recursive removal of the outer current unknown node has the same
false-return invalidation as direct self-removal. Unconditionally resolving the
same token recursively has no supported finite result: recursion continues until
the environment fails. It is not reported as an unresolved token.

HTML cleaning reads the current +0x2C byte after a callback has returned, rather
than a saved entry value (`0x00600C9C`–`0x00600CA4`). A callback or recursive call
that changes it affects the outer result too. Cleaning an inner result and then
accepting that text in an outer callback can consequently escape it twice. The
same per-token output remains shared among failed callbacks as specified above;
recursion does not introduce rollback.

### 7.2 Non-ASCII registry comparison (RZS2)

The value and named-method maps use the predicate at `0x006000F0`. It calls
`__strnicmp` (`0x00A044F0`) on exactly the minimum of the two counted byte lengths.
A negative result means the left key is less; a positive result means it is not.
On a zero result the predicate compares the ORIGINAL unsigned byte lengths.
Equivalent keys therefore require equal original lengths as well as a zero
prefix comparison (`0x00600109`–`0x00600148`). No UTF-8 decoding, normalization,
whole-string wide mapping, length-changing transform or cRZString::IsEqual occurs.

`__strnicmp` captures the current CRT thread locale (refreshing it as in §5.2).
For the C locale (+0x14 = 0), `0x00A05320` folds only byte 41..5A to 61..7A and
compares **unsigned** bytes; every other byte, including 80..FF, retains its
numeric value. A mismatch determines ordering; a NUL in either input or the
count limit ends comparison. Thus raw C9 and E9 are different keys, and UTF-8
É (`C3 89`) and é (`C3 A9`) are different keys too. This rule is not Latin-1
accent folding (`0x00A044F0`–`0x00A04529`, `0x00A0533C`–`0x00A0537A`).

For a nonzero LC_CTYPE handle, each next byte is zero-extended and independently
passed to `___tolower_mt(locale,byte)` at `0x009F047B`; adjacent bytes are never
assembled into one character by the registry loop. Compare the returned integer
values, stopping on a mismatch, a zero left result, or count exhaustion. The
runtime returns their difference, of which the registry uses only the sign
(`0x00A0452B`–`0x00A04565`). The byte-mapping primitive has these rules:

| Condition | Result |
|---|---|
| Locale +0x24 nonzero and byte <=7F | ASCII A–Z to a–z, otherwise unchanged |
| Other byte without uppercase bit 0001 in the locale classification table at +0x48 | Unchanged; multibyte-locale classification of these unsigned bytes uses that same table |
| Uppercase byte requiring runtime mapping | Bundled `___crtLCMapStringA` at `0x009FBEDF`, LC_CTYPE handle, flag 00000100, source count 1 byte, destination capacity 3 bytes, code page from +0x04 and final conversion-control argument 1 |
| Mapping returns zero | Original unsigned byte |
| Mapping returns one byte | That zero-extended byte |
| Mapping returns more than one byte | First output byte ×256 plus second output byte, as an integer comparison value |

Evidence: fast path and classification `0x009F0487`–`0x009F04C9`, mapping request
`0x009F04EC`–`0x009F0511`, reply interpretation and unchanged fallback
`0x009F0514`–`0x009F053E`, unsigned-byte classification table access
`0x009F8722`–`0x009F873A`. This CRT wrapper can itself bridge via Windows wide
mapping; that does not turn the registry operation into whole-name UTF-8 case
folding. Its 256 integer replies, rather than a Unicode case table, fully
determine non-C registry comparisons. They remain installation/locale inputs
to observe as specified in §5.2. Locale-specific changes can affect ASCII too;
the unconditional ASCII examples assume the C/ASCII-compatible case path.

Equal prefixes ending at matching interior NULs still fall through to original
length ordering. For example equal-length `41 00 58` and `61 00 59` are the same
registry key in the C locale, despite their different suffixes. This differs
from insensitive cRZString::IsEqual, whose lowered comparison includes those
suffixes. The maps neither save a locale-specific comparator at registration nor
rebuild when a caller changes runtime locale. The worked collation cases keep
the locale unchanged between insertion and lookup; changing it can change the
predicate on an existing tree and supplies no promised reindexing behavior.

## 8. Parsing, resource IO and failure results *[compat]*

PerformDetokenize snapshots the input's counted bytes before producing output;
the same object may be supplied as input and output. With no `#` in the input,
it returns true and copies the original string when output is distinct, or leaves
it in place when aliased. No callback is invoked and HTML mode does nothing.

With any hash present, `##` pairs are first protected over the whole snapshot,
before tokens are paired. Nonoverlapping pairs collapse to one protected byte.
There is a compatibility exception to ordinary escaped-delimiter parsing: the
position of the FIRST original hash remains the first opening position, even
when that hash was collapsed into a protected byte. Later opening/closing
positions are unprotected hashes. Text between opening and closing positions is
trimmed of leading/trailing ASCII
space and tab only, then passed to virtual ConvertToken. Both delimiters are
discarded, whether conversion succeeds or fails. Replacement text is appended
once; it is not scanned for more tokens. Literal spans retain their bytes.

An unmatched final hash and its remaining suffix are copied literally; this is
not a conversion failure. Every complete token is processed even if a previous
conversion returned false. The final Boolean is true iff every complete token
conversion returned true, or there were no complete tokens. The output is always
the assembled result, including replacements for successful tokens and bare
trimmed names for unresolved tokens. It is neither cleared nor rolled back on
false (`0x00600810`–`0x00600C1C`).

Protection uses an interior NUL marker. Before assigning the assembled result,
every NUL byte in that result becomes `#`, including pre-existing interior NULs
and NULs introduced by callbacks/values. This final restoration occurs on the
hash-processing path only; the no-hash fast path preserves interior NULs. The
pair erase/marker write is verified at `0x00600921`–`0x00600935`, retention of
the first opening position at `0x0060096F`–`0x00600985`, and restoration at the
end of `0x00600810`. Consequently `##` alone becomes `#`, but `###` forms an
EMPTY token and, without an empty-name handler, returns false and empty output.
Likewise `##A#` can resolve token A, because its protected initial byte still
acts as the opening position. `#A##B#` forms one token whose name contains the protected interior NUL,
not two adjacent tokens. Adjacent tokens require separating literal text to
avoid this global escape rule. A marker in a token name is not restored before
lookup or callback dispatch.

The resource overload (slot 8) asks the resource manager for the supplied key as
IID `0x089B7DC8`, with the last two retrieval arguments zero. On success it calls
virtual PerformDetokenize with the acquired string and caller output, then
releases the acquired interface and returns the parser result. On retrieval
failure it releases any non-null acquired interface, returns false, and does not
call the parser or change caller output (`0x005FFEE0`–`0x005FFF4A`, assembly
confirms two arguments despite the incomplete pseudocode signature). It does not
apply locale offsets, infer an LTEXT key type or clear failed output itself.
Those policies belong to the [LTEXT and locale spec](data-formats.md#6-ltext-and-locale)
and to `SC4LoadString` callers.

All string pointers supplied to these methods must denote valid live interfaces.
The base parser does not validate null input/output, catch callback exceptions,
report token offsets, enforce an expansion depth limit, or persist registries.
No callback on a resource-load failure is observable. HTML mode does not alter
the Boolean result of an otherwise unhandled token.

## 9. Conformance vectors *[compat]*

These are declarative vectors for an engine lane to encode in its test format;
this RE lane does not write files under engine/. Bytes in brackets are hexadecimal
counted bytes; a trailing allocation terminator is not included. Unless stated,
strings are independent, tokens are ASCII, and HTML mode is off. Callback traces
are part of the expected result.

| ID | Setup / operation | Expected |
|---|---|---|
| RZ01 | Default object; query Strlen and ToChar | length 0, non-null buffer beginning with 00, refcount 0 |
| RZ02 | FromChar([41 00 42],3); Copy to second; change source byte 0 to 43 | second bytes [41 00 42], length 3 |
| RZ03 | Nonempty; FromChar(null,123) | empty, terminated |
| RZ04 | `ab`; Resize(4), then Resize(1) | [61 62 00 00] length 4; then [61] length 1 |
| RZ05 | `ab`; Append([00 63],2); Insert(1,[58],1); Erase(2,0xFFFFFFFF) | intermediate [61 62 00 63], then [61 58 62 00 63], final `aX` |
| RZ06 | `abcdef`; Replace(1,`XY`,3) | `aXYef`, returns receiver |
| RZ07 | `abcdef`; object Replace(1,[58 00 59]) | `aXef`; old-byte erase count 3, insert count 1 |
| RZ08 | [41 00 58] versus [41 00 59], IsEqual and CompareTo | sensitive: true, zero; insensitive: false, nonzero |
| RZ09 | `ab`.CompareTo(`abcd`, sensitive=true) | -2 |
| RZ10 | [61 00 62 63].Find(`bc`,0,true) | 2 |
| RZ11 | `abcd`.Find(object [62 00 7A],0,true) | 1 (needle C prefix `b`) |
| RZ12 | `ababa`: Find(`ba`,0,true); RFind(`ba`,0xFFFFFFFF,true); Find(`x`,0,true) | 1; 3; 0xFFFFFFFF |
| RZ13 | `ab`: Find(empty,2,true); Find(empty,3,true); RFind(empty,99,true) | failure; failure; 2 (corrected by RZS11) |
| RZ14 | ` \tA\r\n \t`; Trim | `A\r\n` |
| RZ15 | `abcd`: Mid(2,99), Mid(4,1), Left(99), Right(2) | `cd`, empty, `abcd`, `cd` |
| RZ16 | `a::b`; SplitToken(`::`) twice | first `a`, receiver `b`; then empty, receiver still `b` |
| RZ16a | `,a,,b`; SplitTokenDelimited(`,`) three times, then once more | empty true; `a` true; empty true; then `b` false, receiver empty |
| RZ16b | `,a,,b`; SplitTokenSeparated(`,`) twice, then once more | `a` true; `b` true; then false with the destination still `b` |
| RZ17 | [41 7A 00 42]: MakeLower, then MakeUpper | [61 7A 00 62], then [41 5A 00 42] |
| RZ18 | Nonempty; Sprintf(`x=%s %%`, `ok`); then Sprintf(empty) | `x=ok %`; then empty; both return receiver |
| RZ19 | Sprintf(`%04u`,7); Sprintf(`%-4s`, `ab`) | `0007`; `ab  ` |
| RZ20 | [41 00 58].BeginsWith([41 00],2); EndsWith([00 59],2) | true; false |
| RZ21 | Sprintf separately with `%.0d`, `%.0u`, `%.0o`, `%.0x`, `%.0X`, argument zero | `0` in every case |
| RZ22 | Sprintf `%#o`, `%#x`, `%#X`, argument zero | `00`; `0x0`; `0X0` |
| RZ23 | Sprintf `%#.3o`, 1; `%#.3x`, 1; `%#.3X`, 1 | `0001`; `0x001`; `0X001` |
| RZ24 | Sprintf `%08.3u`, 7; `%#08.3x`, 1; `%#8.3x`, 1 | `00000007`; `0000x001`; `   0x001` |
| RZ25 | Sprintf `%+08.3d`, 7; `%-08.3u`, 7 | `+0000007`; `007     ` |
| RZ26 | Sprintf `%p`, pointer bits 0 / 0x12AB / 0xFFFFFFFF | `00000000`; `000012AB`; `FFFFFFFF` |
| RZ27 | Sprintf `%#+.2p`, pointer 0x12AB; `%010p`, same; `%-10p`, same | `000012AB`; `00000012AB`; `000012AB  ` |
| RZ28 | Sprintf `%f`, 1.25; `%e`, 1.25; `%E`, 1.25; `%g`, 1.25 | `1.250000`; `1.250000e+000`; `1.250000E+000`; `1.25` |
| RZ29 | Sprintf `%.0f`, 0.5; `%.0f`, 2.5; `%.2f`, 0.125; `%.2f`, -0.125 | `1`; `3`; `0.13`; `-0.13` |
| RZ30 | Sprintf `%.2f`, binary64 bits 0x4005666666666666 (nearest 2.675); `%.20f`, bits 0x3FB999999999999A (nearest 0.1) | `2.67`; `0.10000000000000001000` |
| RZ31 | Sprintf `%.2e`, 12.5; `%.2E`, 0.125; `%.0e`, 0.0 | `1.25e+001`; `1.25E-001`; `0e+000` |
| RZ32 | Sprintf `%.0f`, 1.0; `%#.0f`, 1.0; `%#.0e`, 1.0 | `1`; `1.`; `1.e+000` |
| RZ33 | Sprintf `%.4g`, 0.000125; `%.4g`, 0.0000125; `%.4G`, 10000.0 | `0.000125`; `1.25e-005`; `1E+004` |
| RZ34 | Sprintf `%.4g`, 9999.5; `%.0g`, 12.5; `%#.4g`, 12.5; `%#.4G`, 10000.0 | `1e+004`; `1e+001`; `12.50`; `1.000E+004` |
| RZ35 | Sprintf `%+010.2f`, 1.25; `%010.2f`, -1.25; `%-010.2f`, 1.25 | `+000001.25`; `-000001.25`; `1.25      ` |
| RZ36 | Sprintf `%f`, negative zero bits 0x8000000000000000; `%+g`, same; `%.0f`, -0.125 | `0.000000`; `+0`; `-0` |
| RZ37 | Sprintf `%f`, `%e`, `%G` separately, positive infinity bits 0x7FF0000000000000 | `1.#INF00`; `1.#INF00e+000`; `1.#INF` |
| RZ38 | Sprintf `%f`, quiet NaN bits 0x7FF8000000000000; `%g`, same; `%+g`, negative indefinite bits 0xFFF8000000000000 | `1.#QNAN0`; `1.#QNAN`; `+-1.#IND` |
| RZ39 | Sprintf `%.0f`, `%.2f`, `%.3f` separately, positive infinity | `1`; `1.#J`; `1.#IO` |
| RZ40 | Sprintf `%.513f`, 1.0; `%#.0g`, 1.0 | `1.` followed by 512 zeros (length 514, successful allocation); `1.` |
| RZ41 | Magnitude bits `0000000000000001` (smallest positive subnormal); Sprintf `%.20e` | Raw eighteen digits `494065645841246544`; `D=49406564584124654, d=-323`; `4.94065645841246540000e-324` |
| RZ42 | Magnitude bits `7FEFFFFFFFFFFFFF` (largest finite); Sprintf `%.20e` | Raw `179769313486231570`; `D=17976931348623157, d=309`; `1.79769313486231570000e+308` |
| RZ43 | Magnitude bits `000FFFFFFFFFFFFF` (largest subnormal); Sprintf `%.16e` | Raw `222507385850720088`; `D=22250738585072009, d=-307`; `2.2250738585072009e-308` |
| RZ44 | Magnitude bits `0010000000000000` (smallest normal); Sprintf `%.16e` | Raw `222507385850720138`; `D=22250738585072014, d=-307`; `2.2250738585072014e-308` |
| RZ45 | Bits `430C6BF526340002`, exact `1000000000000000.25`; Sprintf `%.2f` | Raw `100000000000000024`; `D=10000000000000002, d=16`; `1000000000000000.20`; direct exact-to-17-digit half-up would give `D=10000000000000003` |
| RZ46 | Bits `430C6BF526340006`, exact `1000000000000000.75`; Sprintf `%.2f`; repeat with sign bit set (`C30C6BF526340006`) | Raw magnitude `100000000000000074`; `D=10000000000000007, d=16`; `1000000000000000.70` / `-1000000000000000.70`; exact-to-17-digit half-up would end `8` |
| RZ47 | Bits `42D6BCC41E900008`, exact `100000000000000.125`; Sprintf `%.3f` | Raw `100000000000000124`; `D=10000000000000012, d=15`; `100000000000000.120`; exact-to-17-digit half-up would end `3` |
| RZ48 | Bits `3FB999999999999A` (binary64 nearest to 0.1); Sprintf `%.20e` | Raw `100000000000000005`; `D=10000000000000001, d=0`; `1.00000000000000010000e-001` |
| RZ49 | Bits `3FF0000000000000` (1.0); Sprintf `%.20e` | Raw `099999999999999999`; `D=1, d=1`; `1.00000000000000000000e+000` |
| RZ50 | Bits `4415AF1D78B58C40` (exact 10^20); Sprintf `%.16e` | Raw `999999999999999999`; guard carry gives `D=1, d=21`; `1.0000000000000000e+020` |
| RZ51 | Native Mid on `abcdef`, start 2, count 99; independent unconstructed result slot per §2.1 | EAX equals result slot; 12 argument bytes removed; result `cdef`, native vptr, refcount 0; receiver unchanged |
| RZ52 | Native Left(2) and Right(2) on `abcdef`, each with fresh result storage | Each returns its result pointer, removes 8 argument bytes; results `ab` / `ef`; changing or destroying source does not change either result; destroy each result once with slot 28 flag 0 |
| RZ53 | Native Mid(6,1), Left(0), Right(0) on `abcdef`, fresh result slots | Three independently constructed empty native strings, each terminated and requiring destruction; source unchanged |
| RZ54 | Native MakeLower then MakeUpper on counted [41 00 7A]; also each on empty | Lower [61 00 7A], upper [41 00 5A]; empty stays empty; ECX receiver, no arguments or return-value assertion |
| RZ55 | `Konradshohe` versus `Ab`, CompareTo, sensitive and insensitive, both overloads | Exactly +1 in all four cases; reversed operands give -1 |
| RZ56 | `abABabcdef` versus `AB`, CompareTo sensitive, both overloads | Exactly +1; insensitive returns +8 (equal lowered prefix, original lengths 10 minus 2) |
| RZ57 | Counted [01] versus [7E], CompareTo, either sensitivity, both overloads | Exactly -1, not -125; reversed +1 |
| RZ58 | Counted [41 00 58] versus [41 00 59], CompareTo, both overloads | Sensitive 0; insensitive exactly -1 |
| RZ59 | Sprintf `%#010x` and `%#010X`, unsigned 3473 | `000000xd91`; `000000XD91`, each length 10 |
| RZ60 | Sprintf `%#010.5x`, unsigned 3473 | `0000x00d91`, length 10 (three field zeros before prefix, two precision zeros after it) |
| RZ61 | Sprintf `%#10x` and `%#-010x`, unsigned 3473 | `     0xd91`; `0xd91    ` (five spaces) |
| RZ62 | Empty native haystack; Find(empty,0,sensitive), each needle overload and sensitivity | 0xFFFFFFFF in all four combinations |
| RZ63 | `ab`; Find(empty,start,sensitive), each overload and sensitivity, starts 0,1,2,3 | 0; 1; 0xFFFFFFFF; 0xFFFFFFFF |
| RZ64 | Empty haystack; RFind(empty,start,sensitive), each overload and sensitivity, starts 0 and 99 | 0 in every case; forward and reverse differ |
| RZ65 | `ab`; Find(`b`,1,true), Find(`b`,2,true), each overload | 1; 0xFFFFFFFF (ordinary nonempty search unchanged) |
| RZ66 | C locale; lower [43 41 46 C3 89], upper [63 61 66 C3 A9] separately | [63 61 66 C3 89]; [43 41 46 C3 A9]; non-ASCII letters unchanged in this locale |
| RZ67 | C locale; lower / upper Latin-1 bytes [43 61 66 E9] | [63 61 66] / [43 41 46]; the invalid final sequence is dropped, not retained or replaced |
| RZ68 | C locale; lower [FF 41], with nonempty receiver | Empty receiver; no Boolean/error return; failed decoding still assigns zero produced words |
| RZ69 | C locale; lower counted [41 00 42 80 58] | [61 00 62]; counted NUL survives, invalid 80 and following suffix are dropped |
| RZ70 | C locale; haystack [F0 90 81 81 58] (U+10041 then X), needle `X`, both overloads | Sensitive Find(start 0) = 4. Insensitive lowered haystack [61 78]: Find(start 0) = 1, Find(start 2) = failure, RFind(start 99) = 1. Inputs unchanged; decoder supplies low word 0041, not a surrogate pair |
| RZ71 | C locale; insensitive comparisons of A=[41 80], B=[61 FF], C=[61], both overloads with explicit counts | A and B both lower to [61]: IsEqual(A,B)=true, CompareTo(A,B)=0. IsEqual(A,C)=false from original lengths; CompareTo(A,C)=+1 from original 2−1 |
| RZ72 | Non-C primitive fixture: W=212A maps successfully to 006B; ASCII X maps to x. Haystack [E2 84 AA 58], needle `X`, both overloads | Lowered [6B 78]; Find(start 0)=1, Find(start 3)=failure, RFind(start 99)=1. Original X offset was 3; start is not translated. IsEqual([E2 84 AA],[6B],false)=false; CompareTo=+2 |
| RZ73 | Non-C primitive fixture: eligible W=0100 lower mapping returns zero, scratch word happens to be 0078; lower [C4 80] | [C4 80], not [78] or empty; runtime mapping failure preserves the input word |
| RZ74 | Upper [C2 B5] (U+00B5), C or non-C locale | [C2 B5]; non-C path rejects it at the shipped lower-classification gate, so no OS mapping request occurs |
| RZ75 | C locale; insensitive A=[41 80] versus B=[61 62], both counted overloads | Lowered A=[61], B=[61 62]: IsEqual=false (different lowered lengths); CompareTo=0 (equal one-byte lowered prefix and equal original lengths) |
| RZ76 | C locale; haystack `abc`, malformed needle [80], both overloads | Needle lowers to empty. Insensitive Find(start 0)=0, Find(start 3)=failure, RFind(start 99)=3; no conversion-error result |
| RZ77 | Non-C primitive fixture: W=023A maps successfully to 2C65; X maps to x. Haystack [C8 BA 58], needle `X`, both overloads | Lowered [E2 B1 A5 78]; Find(start 0)=3, Find(start 2)=3, RFind(start 2)=failure. Original X offset was 2; a longer encoding moves it to 3 |
| DT01 | Fresh base instance; literal `a<b` -> distinct output | true, `a<b`, no callbacks; HTML byte zero |
| DT02 | Set value `City`=`Oak`; parse `Hi # city #!` | true, `Hi Oak!` |
| DT03 | Set same key `CITY`=`Elm`; also named callback for `city` | true, `Elm` for `#City#`; callback not invoked |
| DT04 | No registrations; parse `a# missing #b#tail` | false, `amissingb#tail` |
| DT05 | Value `A`=`#B#`; value `B`=`done`; parse `#A#` | true, `#B#`; replacement is not recursive |
| DT06 | No registrations; parse `##`, `####`, `###`, `a#tail` separately | results true, true, false, true; outputs `#`, `##`, empty, `a#tail` |
| DT07 | Named callback N returns false after writing `x`; unknown U sees `x`, writes `y`, returns true | trace N,U; true, `y` |
| DT08 | Two unknown callbacks with identities 0x1000 and 0x2000, registered in reverse order; both accept | only 0x1000 called |
| DT09 | Register unknown F/context1, then F/context2; disable F with context3 | before disabling F sees context1; after disabling no F call |
| DT10 | Callback writes `junk`, returns false; no accepting unknown | false, bare trimmed token name overwrites `junk` |
| DT11 | HTML on; value A=`<&\"` and input `<b>#A#</b>` | true, `<b>&lt;&amp;&quot;</b>` |
| DT12 | HTML on; unresolved token `# <x> #` | false, `&lt;x&gt;` |
| DT13 | Value A holds [58 00 59]; parse `#A#`, HTML off / on | off: [58 23 59]; on: [58] |
| DT14 | Input [41 00 42] versus [41 00 42 23], no registrations | first true, identical bytes; second true, [41 23 42 23] |
| DT15 | `# A #` with stored value A=`v`, input and output same object | true, `v` |
| DT16 | Resource retrieval fails, prefilled output `keep` | false, `keep`, no parser/callback call; release non-null returned interface |
| DT17 | QueryInterface unknown IID with sentinel output, either owner | false, sentinel untouched |
| DT18 | Named callback fails for token X, value A succeeds; parse `#X# / #A#` | false, `X / value-of-A`; later token still processed |
| DT19 | Value A=`v`; parse `##A#` | true, `v` (original first-hash position remains opening) |
| DT20 | Unknown A disables itself, writes `a`, returns true; parse `#X# / #X#` | trace A once; false, `a / X`; self-removal is safe on the accepted path |
| DT21 | Unknown addresses A=0x1000, B=0x2000, C=0x3000. A writes `a`, disables B and returns false; C appends `c`, returns true | trace A,C; true, `ac`; B is not called |
| DT22 | Initially unknown A=0x1000 and C=0x3000. A writes `a`, enables new B=0x2000 and returns false; B appends `b`, returns false; C appends `c`, returns true | trace A,B,C in this same conversion; true, `abc` |
| DT23 | Named N for X writes `n`, sets value X=`v`, enables unknown U, returns false; U appends `u`, returns true; parse `#X# / #X#` | trace N,U; true, `nu / v`; current dispatch does not restart value lookup |
| DT24 | Named N for X replaces its binding with M, writes `n`, returns true; M writes `m`, returns true; parse `#X# / #X#` | trace N,M; true, `n / m`; replacement affects the next conversion |
| DT25 | Named N for X uses a context depth flag. At depth zero it writes `outer:`, sets depth one, recursively parses `#X#` into a separate output, appends it and returns true; at depth one it writes `inner` and returns true | trace N at depth zero, N at depth one; inner true/`inner`, outer true/`outer:inner`; both callback outputs start empty |
| DT26 | HTML initially off. Named X callback switches it on, recursively parses `#Y#` with value Y=`&` into separate output, copies that text to its own output, returns true | inner true/`&amp;`; outer true/`&amp;amp;`; mode is shared and read after callbacks |
| DT27 | Stable C locale; set raw one-byte value names [C9]=`upper`, [E9]=`lower`; call ConvertToken separately with each raw name | true/`upper`, true/`lower`; registry does not decode the invalid UTF-8 names or accent-fold them |
| DT28 | Stable C locale; set counted name [41 00 58]=`first`, then [61 00 59]=`second`; ConvertToken either name | true/`second` for both; strnicmp stops at NUL and original lengths are equal |
| DT29 | Stable non-C byte-mapping fixture: C9→E9 and E9→E9. Set raw [C9]=`first`, then [E9]=`second` | One equivalent key; ConvertToken either name is true/`second`. This is a supplied byte-table reply, not a claim about all Windows locales |
| DT30 | Stable non-C byte-mapping fixture: C3→E3, 89→89, A9→A9. Set UTF-8 [C3 89]=`upper`, [C3 A9]=`lower`; ConvertToken each | Distinct keys: true/`upper`, true/`lower`; lowering the lead byte does not equate the two continuation bytes |

## 10. Open boundaries

[RZS3–RZS5](questions/rz-strings.md) track allocation/overlap failures, remaining
native split variants and formatter edge behavior. RZS2's installation-specific
non-C NLS tables are narrowed to the explicit runtime observations in §5.2;
their absence does not weaken the settled owner rules or conditional primitive
vectors above. No in-game or executable replacement run was performed for this
documentation lane.
