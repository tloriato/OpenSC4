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
