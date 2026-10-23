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
bits have even parity. For ordinary finite operands this admits 0 through 30,
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

## 4. Independent literal ledger — PROPOSED / UNEXECUTED

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

## 5. Coverage and open work

This first milestone qualifies construction, special-interface acquisition,
command gates and Happiness's direct mutation predicates. It does not yet
claim Init, variant copies/readback, display serialization, point controller,
button parsing, container helpers or destruction complete. Precise open
obligations live in [the question file](questions/curve-editor-views.md).
