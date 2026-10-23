# model-instances: answered questions

<!-- from: core-model-instances4 (2026-10-22) -->
~~- **MI3 - §§1–2, constructor/records:** what are the base/static initial owner, custom data, properties and draw attributes; which bytes in the 20-byte draw-attribute record hold flags; and what is the transform change counter's width and SetTransform increment source? [Rust accepts explicit base defaults; static custom data/properties start zero, supplied draw attributes; semantic flags plus 16 opaque bytes without native packing; wrapping u32 instance-local counter, never an ABI layout]~~

**ANSWERED 2026-10-22 (re-model-instance-contracts4):** DIFFERS — §§1.1–1.2 and 2: both properties default to 2; draw flags occupy byte 0, bytes 2–3 are unwritten; transform counters are bytes, Get increments destination and Set increments instance, ignoring source counter.

<!-- from: core-model-instances4 (2026-10-22) -->
~~- **MI10 - §1 reference lifecycle:** what is the initial native reference count and the behavior when its 16-bit counter overflows? [a returned Rust Reference owns one count; exposed arithmetic wraps at 16 bits, while backing Arc storage remains alive until all Rust handles are released, avoiding invalid memory access on counter overflow; ordinary non-overflow releases destroy the most-derived owner at zero]~~

**ANSWERED 2026-10-22 (re-model-instance-contracts4):** DIFFERS — §§1.1 and 2: constructors start at zero; basic has wrapping 16-bit count, static has wrapping 32-bit count; Release destroys at either zero or one, including after overflow. Safe surviving backing storage is a portable choice, not native lifetime.

<!-- from: core-model-instances4 (2026-10-22) -->
~~- **MI6 - §§2,4 material/processor lifetime:** are SetModel's first-material mutations confined to the instance copy; does processor SetModel return true when a non-format-2 mesh leaves it with only the instance; and do explicit ReleaseModel/destruction restore original vertices? [instance copy; true for a retained instance, false only for explicitly refused multi-frame animation; restore only at the documented first step of SetModel, ReleaseModel/destruction release all references exactly once]~~

**ANSWERED 2026-10-22 (re-model-instance-contracts4):** DIFFERS — §4.1: material mutation is instance-local, but processor SetModel also fails and clears both references for missing, empty or ineligible containers. Destruction restores originals; ReleaseModel alone leaves replacements and a borrowed container pointer, and is not idempotent.

<!-- from: core-model-instances4 (2026-10-22) -->
~~- **MI7 - §6.1 upward sheet pass:** what happens to initial own-instance keys before any non-I source exists (including all-I keys); does encountering a non-I source preserve the downward pass's scale? [preserve downward results until a source exists; retain scales of non-I keys because the upward pass only assigns a scale to I keys; retain opaque MI1 values]~~

**ANSWERED 2026-10-22 (re-model-instance-contracts4):** MATCHES — §6.1: leading own keys retain downward results until a nonzero non-own upward source exists; source keys retain their downward scales, including all-own input. Added I=0 failure and wrapping-key limits.

<!-- from: core-model-instances4 (2026-10-22) -->
~~- **MI8 - §4 repeated SortMeshes:** are ties stable against container order on each call or against the preceding processor permutation? [restart from container order each call, then literal descending stable insertion sort, including unordered comparisons]~~

**ANSWERED 2026-10-22 (re-model-instance-contracts4):** DIFFERS — §§4 and 4.1: restart from container order each call, but insertion is before equal and unordered scores; ties reverse container order rather than remaining stable.

<!-- from: core-model-instances4 (2026-10-22) -->
~~- **MI4 - §2 world bounds:** is the transformed model box the enclosing AABB of all eight corners, including negative uniform scale and non-symmetric matrices? [literal enclosing AABB; preserve empty boxes]~~

**ANSWERED 2026-10-22 (re-model-instance-contracts4):** DIFFERS — §2.1: full row-matrix/scale/translation interval transform encloses all endpoint combinations, handles negative scale, ignores kind, and reorders inverted bounds instead of preserving empty sentinels.

<!-- from: core-model-instances4 (2026-10-22) -->
~~- **MI5 - §2 Pick/view-utilities §7:** is nearest hit distance the signed triangle ray parameter t, or Euclidean distance to the intersection for non-unit directions and intersections behind the origin? [literal signed t; the geometry ray test explicitly accepts t < 0]~~

**ANSWERED 2026-10-22 (re-model-instance-contracts4):** DIFFERS — §2.1 and view-utilities §7: distance is Euclidean origin-to-hit length, positive even for accepted behind-origin hits; strict ordered decrease only, not signed t.

<!-- from: core-model-instances4 (2026-10-22) -->
~~- **MI9 - §§2–4 malformed/numeric inputs:** are non-finite playback/transform inputs and invalid primitive vertex/index references in scope? [preserve literal f32 comparisons/arithmetic, no FMA; unordered animation magnitude takes the one-step catch-up branch; under-sized primitive triangle counts saturate to zero and invalid mesh references are skipped for memory safety, consistent with graphics-formats' safe reader choices]~~

**ANSWERED 2026-10-22 (re-model-instance-contracts4):** DIFFERS — §§2.1, 3.1, 6.2, 9.1: x87 transient arithmetic with explicit f32 stores, unordered animation magnitude does not step; primitive subtraction is signed without saturation and iteration stops for nonpositive results; invalid-reference skips and checked parsing are portable safety choices, not native rules.

<!-- from: core-model-instances-contractfix4 (2026-10-22) -->
~~- **MI13 - §6.1 wrapped interior-zero sheet keys:** for I=0x80000001 and input keys (0,0,0x7FFFFFFF,0,0), unsigned normalization gives (I,I,0,I,I). "A zero key cannot supply a downward source" does not specify whether encountering this interior zero fails immediately, resets the source, or fails only for a later equal-zero key. Does ReadAnim succeed, and what exact scales/source zooms are written? Also specify an initially zero key at zoom 4 after wrapping. [literal conservative failure whenever a normalized key is zero; this is marked a portable choice, separate from the proved I=0 false result and nonzero unsigned-wrap vector. No exact claim for wrapped-zero native resources.]~~

**ANSWERED 2026-10-23 (re-model-instance-numerics4):** DIFFERS — §6.1 and §6.1.1: both wrapped-zero examples succeed. Different zero resets the source and writes unit scale; equal zero skips the scale write (and fails at zoom 0). Initial zoom-4 zero leaves its scale untouched. Literal resources distinguish these paths, nonzero wrap and I=0 failure.

<!-- from: core-model-instances-contractfix4 (2026-10-22) -->
~~- **MI12 - §10 vector 15 second-instance stage:** does "source and second instance retain their flag and byte" mean a second frame-builder copy before that instance's own SetModel? §2 requires every SetModel with blend (8,1) to clear its own flag 0x80 and byte 0x18, so two already-bound static instances cannot both retain the original values. [test the source and a second independent frame-builder copy while SetModel mutates the first; test that independently binding either instance applies its own material changes. No shared mutable material fields.]~~

**ANSWERED 2026-10-23 (re-model-instance-numerics4):** MATCHES — §10 vector 15 explicitly stages two independent frame copies before binding. Every SetModel inspects its own material even on same-model rebinding; two already-bound instances both have cleared values (§§2,4.1).
