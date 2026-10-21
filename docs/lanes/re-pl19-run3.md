| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| re-pl19-run3: RE of the growth-pass release wave 0x00709798 (PL19, prop-lotconfig §B5.2/§B5.4/§B7 slot 9) | `re-pl19-run3` / (RE spec lane) | merged train20 (37544298d4; docs only). PL19 filed and ANSWERED DIFFERS: the wave is the per-element Release in cSC4TractDeveloper::ListPossibleLotConfigurations (0x00709650) over the local slot-9 result set; the original slot 9 AddRefs every returned element on every call, cache hit included (0x0048B29F-0x0048B2BC), so net 0 per query; the PL18 double-add is wrong. No game runs, no ABI. | 1. glue21 owner: make slot 9 add exactly one reference per returned element on every call (including filter-cache hits) on the pointer put in the set; drop the PL18 second add; re-run the per-op trace and expect release count = add count per configuration. |

## Report

What changed:
- docs/specs/prop-lotconfig.md: §B5.2 new "Slot 9 reference rule" (the exe's per-call AddRef on all paths; the
  filter cache holds no references; return count = cache length) and the caller contract of the growth pass
  (`0x00709650`: fresh local set per call, filter registered once per key via slot 18, slot 9 once, walk the set
  to its end, suitability verdict, candidate insert without AddRef on verdict 0x1B, one Release per element, no
  early exit). §B5.4 bullet and §B7 slot 9 row point at the rule.
- docs/specs/questions/prop-lotconfig.md: PL19 filed verbatim from glue21, then moved to
  answered/prop-lotconfig.md as DIFFERS; QUESTIONS.md reindexed.

Findings (from the Windows export instructions; REA not needed):
- `0x00709798` = return address of `CALL [EDX+0x8]` (Release) at `0x00709795`; it iterates the local hash set
  filled by slot 9 at `0x00709701`; stops at set end (`0x0047C1A0` bucket advance returns null).
- Slot 9 `0x0048B1A0`: registered-filter path builds a reference-less cache (`0x0048B222`) on a miss, then on
  miss and hit alike AddRefs each cached element (`0x00489790` = `JMP [vtable+4]`) and range-inserts
  (`0x0048A170`). Unregistered/null-filter path AddRefs per match (`0x0048B32D`).
- So release count per configuration = number of growth queries whose result contained it; the original stays
  balanced. A drain of one per query in glue21 means its slot 9 handed out no reference on that pointer for those
  calls (cache-hit path skipping the add, or add on a different object than the one inserted): which one is not
  decidable from the exe (engine/ not read, by rule).

Questions: PL19 answered; none filed open.
