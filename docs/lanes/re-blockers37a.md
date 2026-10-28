| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| re-blockers37a: traffic and automata blocker specs (RE/docs) | Codex; re-blockers37a; detached HEAD, reserved branch re-blockers37a | continue; six answered, two partial; commits 6390b0b4b, 35e265661, 51aca609d, e26948e5e; docs gate passed; cargo/clippy/ctest n/a; game n/a; ABI unchanged; budget guard blocked queue write | 1. Finish ready queue using actual HEAD. 2. Implement six published contracts; retain AD6 and TE33-PATH-1 as partial. |

Answered 2026-10-28:
- TE33-ACCIDENT-1: traffic-simulator §§4.13.1–4.13.2, §9.1; whole-second clock, deadline zero, strict expiry, conditional CRT reset draw and per-entry generator draws.
- AD5: automata-managers §3.4.1; concrete thiscall ABI, borrowed effect token, list/controller ownership, shared expiry and native direct-call boundary.
- TE33-MAPS-1: traffic-simulator §4.9 steps 4–5, §4.9.2; switch-ID cells force volume >=1 without network info.
- TC29-STATS-1: traffic-simulator §4.9.2; reset, category-bit counts, sums, strict minima and publication.
- TC29-STATS-2: traffic-simulator §4.12.1; monthly funding snapshots in fraction units; getter has no update.
- TC29-FERRY-1: traffic-simulator §4.12.2, traffic-network §4.9.3; reset total before ordinary state-19 rider rotation.
Partial, left open:
- AD6: automata §4.2; ordered survivor Init/removal, recount, view/zoom and traffic reference repair proved. Missing supported private-repair boundary, complete overridden placement/callback effects and first allocator corruption.
- TE33-PATH-1: pathfinder §4.7.1; strict heap comparisons, two synthetic fixtures and ring-cell order proved. Missing original start-node/score inputs, preceding heap and earlier pop/update sequence for Jinae/Braus.
Validation: final cumulative git diff --check passed; questions.py index regenerated.
Read-only evidence: export, decomp and cited Braus save-fault log; no game runs, REA calls or engine reads.
Earlier whitespace gate failures were corrected before this finish; they are not reported as passing checks.
No ABI exports or proposed ABI rows; no engine/tools edits. Six original question entries archived with section pointers.

## Handoff
Goal: publish blocker contracts, archive complete answers, and enqueue ready; substantive spec work is complete.
Done: 6390b0b4b accidents/AD5; 35e265661 AD6 partial; 51aca609d maps/statistics/ferries and PATH partial; e26948e5e finish record and whitespace correction.
Next: read actual HEAD and create build-runs/merge-queue/re-blockers37a.json with lane/branch re-blockers37a, that full tip, kind docs, abi false, game_runs [], state ready; then return ready. The guard rejected the queue creation, so no queue exists.
Facts: final cumulative diff --check passed and questions index regenerated; worktree was clean. No game run or code gate is needed. Six answered/two partial details above remain the intended final scope.
