# ui-helpers: open questions
<!-- Questions about [ui-helpers.md](../ui-helpers.md). Append new entries at the END; when one is answered, move it to answered/ui-helpers.md. Rules: README.md. -->

<!-- from: re-uihelpers7 (2026-10-23) -->
- **UIH3 - §3:** the catalog properties `0x8A2602BA` (menu), `0x8A2602B9` (order), `0x8A2602B8` (icon),
  `0x8A2602BB` (button ID), `0x8A2602A9` (description), `0x8A416A99` and `0xCA416AB5` (name and description text
  keys) and `0x899AFBAD` (a name). Do the shipped property names confirm these meanings? [inferred]
  **PARTIAL 2026-10-27:** The consumers at 0x0078EC90 and 0x0078EF30 confirm text-key/fallback, icon/button, menu-filter and ordering roles (§3). No executable property-name table confirming the shipped names was located; confirmation from shipped property metadata remains unresolved.
