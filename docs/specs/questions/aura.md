# aura: open questions
<!-- Questions about [aura.md](../aura.md). Append new entries at the END; when one is answered, move it to answered/aura.md. Rules: README.md. -->

<!-- from: verify-regress4 (2026-10-21) -->
- **AU16 - section 14.3 (with sections 5.3 and 6.3):** section 14.3 names state 5 as the only state spread over
  several calls, but state 3 takes one department per call (section 6.3), so with N departments the original reads
  department k's grid at its k-th call. A shadow that runs all of state 3 at the 3 -> 4 move reads later grids: in
  run regress-train19 (cycle 2, 2026-03-11, 11 days) a landfill tile pair inserted between the original's state-3
  calls gave the 208-tract / max 188 landmark-map difference of section 5.3's worked example (orig -91 / ours -96 at
  tract 3,6). Should section 14.3 also say: when the original stays in state 3 but its departmentCursor (+0x7C)
  moves, run one state-3 call of ours? [yes: verify-regress4 does this (the core's 3 -> 3 step); please confirm the
  cursor offset and that a delivery which leaves state and cursor unchanged never consumes a department]
