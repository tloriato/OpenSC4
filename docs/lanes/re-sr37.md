| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| re-sr37 — RE save layouts | Codex / .claude/worktrees/re-sr37 / detached HEAD (reserved branch re-sr37) | ready; spec commit 7e3425681; docs gate passed; 749 saves, 5,618 objects, zero remainder/differences; ABI false; game/shadow n/a; SR36-1/2/3 and SR33-2/3 answered | - |

- SR36-1 answered: lot-records §10.5, cross-linked from prop-occupant and save-records §3.1.
  Consumed 44 class records / 4,683 terrain-decal objects / 641,571 decompressed bytes; fixed 47-byte extension.
- SR36-2 answered: disasters §5.11.2, cross-linked from save-records §3.1.
  Consumed 498 class records / 860 fire objects / 1,264 inline flames / 107,840 decompressed bytes.
- SR36-3 answered: lot-records §10.6, cross-linked from prop-occupant and save-records §3.1.
  Consumed 38 class records / 75 burnt-building objects / 8,550 decompressed bytes; one 24-byte trailing box.
- All three outer readers accept only v3. Specified inner prop timer gates and flame-v2 legacy / v3+ compact transforms.
- All 5,618 objects / 757,961 class-record bytes parsed field by field and re-encoded exactly; no opaque retained tail.
- Final corpus frozen at 749 golden end saves after earlier successful 737-save and 749-save scans; reference files read in place.
- Throwaway audit and frozen-key report: build-runs/audit-sr37.py and build-runs/audit-sr37-report.json (uncommitted).
- Legacy inner versions and nonzero flame transform masks have executable evidence only; all observed outer versions are 3.
- Sources: main export queries, class decompilation and paged instruction bytes; no REA call or session error.
- Archived SR36-1/2/3 and removed stale SR33-2/3 open copies, preserving existing re-sr36 answers and owner pointers.
- Question file: docs/specs/questions/answered/save-records.md; index regenerated, five answers recorded once, none open here.
- Gate: build-runs/check-docs-sr37.py passed (owner anchors, question disposition, fresh index, corpus totals, diff whitespace).
- Global questions.py check remains exit 1: baseline 18 issues, current 18 identical issues, zero introduced; all outside save-records.
- Cargo/clippy/ctest and game runs n/a (spec-only); no standalone full-type audit or game-shadow claim.
- No engine/tools/ABI edits or proposed ABI rows. Automata-controller prop layout remains outside this task.
