# save-records: answered questions

<!-- from: save-roundtrip33 brief, assigned to re-sr36 (2026-10-28); the open save-records question file was not present at this lane's base. -->

- **SR33-2 — Traffic network map:** What is the complete layout of city component 42, record type
  `0x6A0F82B2`, including version gates, intersections and sides, reference lists and the final occupant table?
  **ANSWERED 2026-10-28 (re-sr36):** MATCHES — [traffic-network §6.2](../../traffic-network.md#62-traffic-network-map-0x6a0f82b2-compat)
  specifies every field in wire order. `0xCA0F81B9` is the IID; `0x6A0F82B2` is the class/record type.
  The writer emits v7; the reader rejects v0–2, accepts v3+ without an upper-version check, and gates the v4–7
  additions. Intersections and the final occupant-to-four-lists table follow simulator §5.8 creation order.
  All 547 available reference end saves / 547 map objects / 583,750,387 decompressed record bytes consumed exactly;
  legacy gates and empty marked-train/keyed-reference lists are supported by executable evidence.

- **SR33-3 — City and auxiliary city record:** What surrounds the component references in class
  `0x8990C372`, which reference gates apply to legacy versions, and what is `0x89D9B9EE / 0x69D9B9F1 / 0`?
  **ANSWERED 2026-10-28 (re-sr36):** MATCHES — [foundations §3.6](../../foundations.md#36-the-city-and-its-subsystems)
  specifies the inline property holder, own fields, six strings, references and final byte. Read accepts only
  v8–12: v10 adds the state byte, v9/v11/v12 add component references 51/52/53. [save-records §2.1](../../save-records.md#21-city-auxiliary-cheat-state-record-compat)
  specifies the separate v1 auxiliary record and its nested, byte-counted cheat-code data, including the
  original enabled-ID overread and the bounded standalone rule. All 547 city objects / 318,073 bytes and 547
  auxiliary records / 14,222 bytes consumed exactly, including the inner cheat-data framing.

- **SR36-1 — Terrain-decal prop persistence (2026-10-28):** The 547-save standalone corpus has
  32 records of class `0x2977AA48`, totaling 425,111 decompressed bytes. lot-records §1/§16 Q6
  explicitly leaves the subclass layout unspecified; prop-occupant §2 supplies identity and size
  only. Please specify the outer version gates, embedded base and complete terrain-decal payload,
  including legacy variants. This type remains unhandled rather than counting retained bytes as
  decoded state.
  **ANSWERED 2026-10-28 (re-sr37):** MATCHES — [lot-records §10.5](../../lot-records.md#105-csc4propoccupantterraindecal-0x2977aa48-compat)
  specifies outer u16 version 3 (only accepted value), a complete embedded prop and the fixed
  47-byte extension: raw decal ID, texture key, size, x/z, opacity and fade state, s8 mode and
  two u8 flags. Inner prop versions ≤5 omit timer bits and default them to `0x0F`; higher
  versions read that byte. In a frozen 749-end-save corpus, 44 class records / **4,683 objects** /
  641,571 decompressed bytes were consumed completely and re-encoded exactly. Legacy inner
  versions are instruction-derived, not present in this corpus.

- **SR36-2 — Fire-occupant persistence (2026-10-28):** Class `0x29D25F1D` occurs in 368 records
  totaling 79,732 decompressed bytes. disasters §5.11 specifies the owner, fields and runtime
  operations, but not the Serializable wire layout. Please supply its Read/Write field order,
  widths, base nesting and version gates. The archive does not infer a wire layout from object
  offsets.
  **ANSWERED 2026-10-28 (re-sr37):** MATCHES — [disasters §5.11.2](../../disasters.md#5112-saved-fire-occupant-and-inline-flames-sr36-2-compat)
  specifies outer v3 only, occupant v4 and holder, u32 counter/stage/max-stage, target reference,
  u32 count and inline flames. Each flame writes v3; Read rejects v0–1, uses the 56-byte legacy
  transform at v2 and the compact masked transform at v3+. Flame fields are a one-byte Boolean,
  u32 facing index and three f32 position values before the transform. In the same 749 saves,
  498 class records / **860 fire objects / 1,264 inline flames** / 107,840 decompressed bytes
  were consumed completely and re-encoded exactly. The v2 transform and nonzero masks have
  instruction evidence only.

- **SR36-3 — Burnt-building prop persistence (2026-10-28):** Class `0xC9EFB016` occurs in
  28 records totaling 6,042 decompressed bytes. lot-records §1/§16 Q6 leaves its subclass save
  layout open. Please specify its version gates, embedded prop record and all trailing fields
  so its owner can decode and write the saved state.
  **ANSWERED 2026-10-28 (re-sr37):** MATCHES — [lot-records §10.6](../../lot-records.md#106-csc4propoccupantburntbuilding-0xc9efb016-compat)
  specifies outer u16 v3 only, complete embedded prop and exactly six trailing f32 source bounds
  (min x/y/z, max x/y/z), with no further fields or padding. The embedded prop's timer-byte gate
  is the same as SR36-1. In the same 749 saves, 38 class records / **75 objects** / 8,550
  decompressed bytes were consumed completely and re-encoded exactly. Legacy inner versions
  have instruction evidence only. The old absence claim in lot-records §1/§16 Q6 is corrected
  for both prop subclasses; the automata-controller prop remains a separate open question.
