# geometry-utilities: open questions
<!-- Questions about [geometry-utilities.md](../geometry-utilities.md). Append new entries at the END; when one is answered, move it to answered/geometry-utilities.md. Rules: README.md. -->

<!-- from: core-geometry-utilities4 (2026-10-22) -->
- **GU2 - §§1–6 numeric sequence:** Please publish the operation association and float-store boundaries for the formulas, including dot/cross products, normalisation, trigonometry, mesh lighting and byte conversion. Are constants such as 0.3/0.7 double or float operands, and which comparisons retain extended temporaries? What are the NaN/overflow conversion rules for the colour integer? [Core uses left-associated expressions, separate 24-bit operations through the existing foundation arithmetic, f32 constants/stores, and nearest-even colour conversion; nonfinite/out-of-i32 colour conversions use the x87 indefinite integer. No exact x87/extreme-input credit claimed until confirmed.]

  Partial resolution 2026-10-23 (re-geometry-contracts4): **DIFFERS** in the published finite domain. §§1–5,
  6.1 and 6.3 now give individual associations, memory stores versus resident comparisons, double 0.3/0.7,
  and the float-bit colour conversion (no integer-indefinite operation). §10.1 N1–N3/M1 discriminate them.
  Remains open for any wider ambient PC/RC/caller-preservation guarantee, native transcendental result bits,
  signaling exceptions and whole-call extreme-colour behavior outside the supplied-control/valid-memory domain.

<!-- from: core-geometry-utilities4 (2026-10-22) -->
- **GU3 - §6.1 builder lifecycle:** Please specify Reset's changed flag, array retention and bounds state; BeginPrimitive/EndPrimitive behaviour when another primitive is open or none is open; whether AddVertex requires BeginPrimitive and updates its count; when setters mark changed; empty Mesh bounds; destructor/resource ownership. [Reset retains allocated arrays, empties bounds and marks changed; only BeginPrimitive/AddVertex/EndPrimitive mark changed; EndPrimitive closes the last primitive if present; AddVertex appends without implicitly closing or creating a primitive. Rust owns and drops its arrays. These choices remain provisional.]

  Partial resolution 2026-10-23 (re-geometry-contracts4): **DIFFERS** (§6.1). Reset writes neither dirty nor
  bounds, retains handles and issues zero-length resize requests; AddVertex needs a valid vertex array,
  wraps its u16 vertex count, does not dirty or increment a primitive count. Setters only copy fields;
  BeginPrimitive dirties/appends without closing. §10.1 M2/M3 propose traces. Remains open: foreign resize
  allocation retention, empty/tie/NaN bounds, missing general EndPrimitive/destructor and resource ownership.

<!-- from: core-geometry-utilities4 (2026-10-22) -->
- **GU5 - §§4.6, 5.2, 6.1 unordered/capacity cases:** What exact operands win min/max ties and NaN comparisons in overlap outputs and builder bounds? What happens when the initial polygon or an intermediate clipped polygon exceeds the 40-point scratch capacity, and when max exceeds 40? [Core uses ordered conditional selections retaining the first bound on ties/unordered comparisons; clipping uses dynamically sized safe scratch storage with the published 40-point intermediate capacity, avoiding native out-of-bounds behaviour.]

  Partial resolution 2026-10-23 (re-geometry-contracts4): **DIFFERS** for overlap (§4.6): second operand wins
  ties/unordered; outMin null gates both writes. §§5.1–5.2 specify initial unbounded-by-max copy, literal40
  first-edge truncation, signed min with max, m≤0 bypass and crossing-failure write gating. §10.1 B1–B3/C1–C5
  use valid backing and do not predict uninitialized scratch values. Remains open: the separately owned
  vertex-array bounds helper's tie/NaN/empty policy; overlap's rule must not be transferred to it.

<!-- from: core-geometry-utilities4 (2026-10-22) -->
- **GU6 - complete owner inventory:** The lane assignment cites 37 functions/7509 bytes and transforms/sorting, but the published geometry spec lists 20 geometry functions, builder methods/helpers, and four nS3D helpers, with no transform or sorting contracts or complete owner inventory. Please publish the precise 37-entry owner/method checklist (including any destructors, bounds/corner helpers and primitive closure) and any omitted contracts. [Implement only the named published behaviours; do not invent transform/sort methods or claim reconciled 37/37 ownership.]

  Partial resolution 2026-10-23 (re-geometry-contracts4): §11 now individually enumerates 37/7509 with
  behavioral argument shapes, source ranges, section/status per body: 20 geometry, 9 builder, 8 nS3D.
  No general EndPrimitive/destructor/transform/sort occurs among those exported owners. All eight nS3D
  bodies now have local contracts, including upload helpers and corners. Remains open: any folded/missing
  lifecycle attribution and foreign bounds/container dependencies; 37 attributed bodies is not full-owner
  completion or proof of algorithms in draw-context, ModelInstances, renderer or font services.

<!-- from: core-geometry-utilities-contracts4 (2026-10-23) -->
- **GU7 - §6.4.2 plain upload auxiliary fields:** The compressed descriptor explicitly has unit auxiliary dimensions; are any auxiliary dimensions initialized/passed by the plain helper, and if so to what values? [The raw portable descriptor models these as None for plain upload and Some([1,1]) for compressed upload. It implements the published plain width/height, source offset, signed stride, format/type and cleanup, without inventing unestablished auxiliary values.]
