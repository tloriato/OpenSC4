# Answered lighting questions

<!-- from: core-lighting4 (2026-10-22) -->
- **LT6 - §2.2 light distance:** is the change test the Euclidean RGB distance, its square, or a different metric? What precision is used? [Literal Euclidean length against the threshold. A displacement (0.00008,0.00008,0) exceeds 0.0001 under this choice; a squared-distance comparison does not.]
  **ANSWERED 2026-10-22 (re-lighting-contracts4):** MATCHES — Euclidean RGB length, strictly above the binary32 threshold. Differences, squares, blue-plus-green-plus-red sum and square root stay in x87 precision until the threshold comparison. Equality and NaN fail the change test. See §2.2 and §11 vector 10; instructions `0x007D9E21–0x007D9E68`.

<!-- from: core-lighting4 (2026-10-22) -->
- **LT5 - §1.1–§1.4 constructor, statics and identity:** what are the initial reference count, successful QueryInterface AddRef behavior, last-announced colors/fractions, initial direction/cache values, and the initial statics for properties whose table gives only shipped values? What does a view change use for fade before city dimensions exist? [Core requires explicitly seeded shared statics; shipped() is an explicit fixture/configuration, not inferred native defaults. Unspecified instance fields start zero, last-announced/cache light colors white, reference count 1, successful QueryInterface acquires a reference, and the no-city fade gradient is zero. These choices are not exact-owner credit.]
  **ANSWERED 2026-10-22 (re-lighting-contracts4):** DIFFERS — constructor count 0, announcements/directions/cache black/zero, fractions 1/0, current light/tint white, unrotated sun (1,−1,−1). Geometry and two temporal map pointers are unset; no universal pre-city fade result. Successful QueryInterface AddRefs once. Initial shared statics are separately tabulated in §1.4; see §1.2 and vector 13.

<!-- from: core-lighting4 (2026-10-22) -->
- **LT7 - §1.5/§2.1/§4.2 image boundaries and failure:** does min-red scan active width×height pixels or padding too? When the last-row time sample spills beyond the allocation, or the surface map exists but model map is absent/shorter, what is the defined outcome? What Init cleanup is required if decoding/copying fails after earlier maps loaded? [Scan active pixels; preserve linear row spill when storage exists; return explicit safe Rust errors for unavailable pixels/model map. Failed initialization owns partial resources until Shutdown/destruction or retry; no foreign image/lighting methods are called.]
  **ANSWERED 2026-10-22 (re-lighting-contracts4):** DIFFERS — min-red includes vector padding, excludes unused capacity. Sample/model accesses are unchecked; allocated linear spill is read, inaccessible storage has no defined owner colour/error outcome. Missing-image Load assigns null and Init continues successfully. There is no lighting-level transactional copy/exception recovery. Shutdown explicitly frees maps/unsubscribes; destruction alone does neither. See §1.5 and vectors 11/14.

<!-- from: core-lighting4 (2026-10-22) -->
- **LT8 - §5.2/§5.5 terrain bounds:** what is the numeric epsilon at 0x00AA2270, and how does the upper clamp round before terrain slot 27? Are rectangular city grids supported by the sweep, and if so which row/column dimensions are used per rotation? What is required for sx=0? [Interior terrain normals and negative-coordinate lower clamps implemented; upper-edge/outside reads return ContractGap until epsilon is specified. Square sweep only; rectangular sweep returns ContractGap; nonfinite drift skips and retains the prior rectangle.]
  **ANSWERED 2026-10-22 (re-lighting-contracts4):** DIFFERS — epsilon is binary64 0.001; compare with the x87 difference, store a replaced argument binary32. Sweep always uses X rows of Z floats at every rotation; no rectangular rejection/count swap. sx=0 yields nonfinite drift and skips with the old rectangle under masked exceptions. Executed no-change rectangle is (−1,−1,0,0). See §5.2/§5.5 and vectors 12/15.

<!-- from: core-lighting4 (2026-10-22) -->
- **LT9 - §6 terrain change rectangles:** are incoming dirty rectangles inclusive vertices or cells, and how does 0x007D6540 convert the changed stored vertex rectangle to terrain slot 67/68 world cells (expansion, endpoint inclusion and clipping)? On long-operation end, is RedisplayTerrain called when a sweep changes nothing, and which rectangle is used then? [Core currently accepts inclusive vertices, inverts the specified grid indexing exactly without cell expansion, and emits slot-67/68-style events for the same nonempty changed world vertex rectangle. This is provisional, not an exact slot-67/68 contract.]
  **ANSWERED 2026-10-22 (re-lighting-contracts4):** MATCHES — incoming vertex rectangle goes unchanged to altitude copying. The §6 transform table passes sweep-changed inclusive endpoints verbatim to slot 67, with no lighting-side expansion/clipping. Deferred end also calls slot 68 with that rectangle; executed no-change sweep calls neither. Skipped sweeps can notify the retained previous rectangle. See §6 and vector 16. Recipient-side cell expansion is outside this owner.

<!-- from: core-lighting4 (2026-10-22) -->
- **LT11 - §5.6 CalcShadowTerrainRect:** specify the full arguments, box-top ray origins, axis stepping, fractional direction handling, terrain sample coordinates, stop condition, rectangle endpoint inclusion and clipping after extension. Does it scan each box-edge ray or a bounding extremum? [Overview is insufficient for an exact extension algorithm; method returns ContractGap and preserves output. No substitute provider or guessed ray traversal.]
  **ANSWERED 2026-10-22 (re-lighting-contracts4):** DIFFERS — §5.7 specifies two unused arguments, six-float box and inclusive integer output; inclusive initial intersection then quadrant-specific ordered corner probes, binary32 axis slopes, no cross-axis drift, strict height-above-terrain stepping, retained stopped endpoints, no final clip and unconditional true return. It samples selected extrema, never every box-edge ray; the second probe continues the first edge/height. See §5.7 and vector 17.

<!-- from: core-lighting4 (2026-10-22) -->
- **LT10 - §5.6 CalcShadowProjection:** specify the plane basis, model-frame direction transform, complete input/output matrix layout, flag-dependent composition and arithmetic precision. Does the determinant test use a signed or absolute determinant? [Only the stated 2D affine least-squares subproblem with signed determinant cutoff 0.01 is implemented; complete projection returns ContractGap and preserves output, without an original-method provider.]
  **ANSWERED 2026-10-22 (re-lighting-contracts4):** DIFFERS — §5.8 now specifies the complete count-first method: flag-4 conditional transpose of R, negated direction without scale/translation/normalization, fixed vertical/unordered basis fallback, distinct retained/spilled normal sums, all cofactor/coefficient/lift spills, signed binary64 0.01 cutoff (NaN proceeds), whole-byte nonzero composition gate, inverse translation, flag-2 reciprocal scale on linear components only, and column-major sixteen-float output. Failure leaves output untouched; no scale-zero/NaN recovery is added. Native arithmetic is not a generic symmetric least-squares solve: vector 21 rejects a noncollinear triangle, and identical UV columns in vector 22 yield different U/V translation constants. See §5.8.1–§5.8.4 and vectors 18–22; instruction evidence `0x007D8C80–0x007D9813`, attributed basis `0x0080B9F0–0x0080BAC6`. This resolves the spec question; it does not assert portable implementation or measured runtime exactness.

<!-- from: glue-lighting-shadow9 (2026-10-24) -->
- ~~**LT14 - §§1.3, 2, 3, 6 live-owner input replay:** what public/serializable interfaces provide the lighting
  owner's complete ordered external inputs at city load and thereafter: the time-of-day driver (independent
  of GetTimeOfDay), tint/reset events, render settings including the unidentified LT1 byte, view changes and
  terrain notifications? There is no owner Read/Write slot in §7. Please specify a safe input snapshot/replay
  contract, including initial state and notification ordering, rather than copying native lighting outputs
  into the expected model. [Default-off shadow measures only §5.6 GetShadowParams and
  GetDecalShadowParams(null), using §1.4's explicitly shipped parameter inputs in the Rust core. The published
  §0 global locates the borrowed owner; no hardware observers, setters or native lifecycle calls. Slots
  0–13, 15, 17–29 and nonnull-holder slot 16 remain uncompared; whole-owner verification is not claimed.]~~
  **ANSWERED 2026-10-24 (re-lighting-lt14):** DIFFERS — there is no complete public/serializable owner snapshot. §1.6 specifies independent ordered replay from known constructor/Init plus process-shared property history, delivery-time dependency results, direct tint/reset calls, settings including the raw LT1 byte, view and terrain events; missing inputs limit comparison scope. §2.4 proves the normal driver reads current clock visual hour/minute at post-city-init and VisualTimeTick delivery and supplies binary32(hour + minute × binary32 0x3C888889), not saved game time, message payload or visual seconds. No native mutator/lifecycle call is needed or permitted to manufacture replay state. Vectors 24–27 distinguish history and delivery ordering. This closes the requested contract; it does not claim an available complete public capture facility or whole-owner verification. LT1/LT12/LT13 remain separate questions.

<!-- from: glue-lighting-shadow9 (2026-10-24) -->
- ~~**LT15 - §1.4 shipped decal texture:** the table gives `0x69D3BC3B` for property `0x085489C8`, but the
  house-switch run lighting9-house-2032-r1-20261024 returns `0x69D3A23B` in slot 16(null) on all 46 captures
  (2026-01-27 through 2029-10-01). Is the shipped texture value in the table a transcription error?
  [The first literal shipped-input measurement is preserved as 0/46 exact, one mismatch per line. The
  next revision captures the six §5.6 input properties independently from resource
  `6534284A/A9189CF0/00000001` through the public resource/property interfaces, then feeds the Rust
  property-reading implementation; no expected value is copied from a native lighting getter.]~~
  **ANSWERED 2026-10-24 (re-lighting-lt14):** DIFFERS — §1.4 corrects the shipped u32 texture to `0x69D3A23B` (1775477307), verified directly in GOG SimCity_1.dat exemplar `6534284A/A9189CF0/00000001`, property `085489C8`. The old `0x69D3BC3B` was a transcription error. The getter copies the shared integer unchanged, with optional per-holder override; missing properties retain prior configuration. Vector 23 covers shipped value, reload retention and holder isolation. The initial run's 0/46 verdict is preserved, not relabeled exact.

<!-- from: re-lighting (2026-10-19) -->
- ~~**LT1 - §2.1, §2.2:** which setting is the byte `+0x39C` of the `+0x0C` member of the settings object at
  `0x00B43CC4`? When it is set the fade colour is black, the fade strength 0 and no dawn or dusk message is posted.~~
  **ANSWERED 2026-10-27 (re-q-misc):** MATCHES — the byte is the boolean render property kRenderSnapshot (boolean 28: table offset 0x20 × 28 + 0x1C = `+0x39C`; registered index `0x1C`, default false). When set, time-of-day recomputation uses a black fade colour and fade strength 0 and posts no dawn/dusk message (lighting.md §2.1, §2.2; render-properties.md §3).

<!-- from: re-lighting (2026-10-19) -->
- ~~**LT2 - §4.3:** which network occupants answer true to slot 8 with `0x40000`? Their models are lit with the up
  normal and the default weight, ahead of the type tests.~~
  **ANSWERED 2026-10-27 (re-q-misc):** MATCHES — slot 8 of the network occupant (`HasNetworkFlag`, all given bits) with `0x40000` is true for pieces on a bridge step, abutments included, so bridge pieces are lit with the up normal and the default weight (lighting.md §4.3; network-geometry.md §10.3).
