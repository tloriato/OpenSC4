# occupant-groups: open questions
<!-- Questions about [occupant-groups.md](../occupant-groups.md). Append new entries at the END; when one is answered, move it to answered/occupant-groups.md. Rules: README.md. -->

<!-- from: re-occgroups (2026-10-19) -->
- **OG1 - §3.2, §5.1:** the special filter groups (ID 0; 35 in the shipped scripts) are listed in creation order,
  which is Lua's `lua_next` order over `occupant_group`. Does any consumer of script slot 10
  `GetSpecialFilterOccupantGroups` depend on that order? The same order decides which of two groups with one ID
  can be found by ID, but the shipped data has no such collision.

<!-- from: re-occgroups (2026-10-19) -->
- **OG2 - §3.3:** the scripts say an empty key-effect `bone` means the same as `fx`, but the importer stores the
  empty string. Is the default applied when the effect starts (automaton slots `+0xB0`, `+0xB4`, `+0xBC`, called
  from `HandleKeyEffects` `0x00566A70`), or is an empty bone passed on as is?
