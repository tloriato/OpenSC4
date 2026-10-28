| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| re-blockers37a: loaded automata and pathfinder blockers (RE/docs) | Codex; re-blockers37a; detached HEAD, reserved branch re-blockers37a | ready; AD6 answered (7a2bbb6e0); TE33-PATH-1 partial (6e2bad6ff, citation corrected 2b64dfdec); docs diff check passed; question diagnostics unchanged (18), new 0; cargo/clippy/ctest/game n/a; ABI unchanged | 1. Coordinator land queued docs. 2. Obtain native TE33 search inputs/prefix for scenario replay. |

## Report
Scope: AD6 and TE33-PATH-1; no game runs or engine/tools edits.
Done: 7a2bbb6e0 answers AD6 in automata §§4.2.1–4.2.5 and archives it under answered/automata-managers.md.
AD6: native retained Init, model/effect ownership, conditional shared RNG, controller outputs, manager slot 68 (ret 4, s32 type), activation and allocator header/pool preconditions.
Correction: traffic second pass repairs path-info and LEADER (+0xB0, slot 148 FindLeader), not model. No atomic public restore service is called; Init/common 38-slot interface alone is insufficient.
AD6 diagnosis remains unproved: fault uses occupant vtable 00AA41D8 as allocator neighbour; terminal stack cannot identify the earlier corruptor. Contract answered, no DLL/save fix claimed.
TE33-PATH-1 partial: pathfinder §4.7.1 has exact key/g-bit fixtures, first common key, trace observations and required native snapshot/input data (6e2bad6ff; 2b64dfdec corrects the pop citation).
Key finding: (243,61,Bus,side1) branches are DISTINCT keys 0902F33D / 0502F33D (predecessor side); stated continuations first merge at (243,62), key 0502F33E, g bits 3DAAF6A7.
TE33 remains open: original ordered start pushes/scores or full heap/parent/flag snapshot and intervening proposals/pops; Braus additionally lacks first differing query identification.
Evidence: main export/decomp and existing logs; no REA needed, no game run. Copy trace: MAIN/.claude/worktrees/traffic-exact33/build-runs/r9-jinae-corner.txt.
Final review rechecked native list restoration, manager slot 68, traffic reference repair, allocator unlink and heap/equal-g instructions; corrected shared-key pop to trace line 97.
Validation: git diff --check passed; questions.py index regenerated. Helper build-runs/check-question-baseline.py compares checker output against base 32fef08e97fc9667147689e9579f55ef7df0c29e.
Measured: baseline question check=1 / 18 diagnostics; current=1 / same 18; new=0. Raw checker fails on unrelated pre-existing duplicate/ID-format issues; do not edit those files.
Index regenerated; only AD6 changed generated counts. No cargo/clippy/ctest applicable; game labels/verdicts and shadow exact/total n/a.
Queue: build-runs/merge-queue/re-blockers37a.json, ready, actual final full HEAD; no ABI-row proposal file required.
Prior six answers are already merged on main (c982e011e); this round does not rework them. No ABI exports/rows or seam reservations.
