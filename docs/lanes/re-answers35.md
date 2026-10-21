| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| re-answers35 (RE answers: mysims questions MS1-MS7, docs only) | `re-answers35` / `C:\Users\tlori\FreeCity4\.claude\worktrees\re-answers35` (branch `re-answers35`, RE lane) | **merged train19 (832c7238e0)** — MS1-MS7 answered (docs only) | Implementers: future mysims changes listed in the report |

## Report

All seven open mysims questions answered from the Windows export (re_query.py asm, decomp/, exe bytes via build-runs/rd.py);
REA not needed. Spec docs/specs/mysims.md updated in place, entries moved to docs/specs/questions/answered/mysims.md,
QUESTIONS.md regenerated (mysims: 0 open, 7 answered). Commits eecb2081 (MS1), 77929a0c (MS2), 758a3a4b (MS3),
78863510 (MS4), 9e3cb632 (MS5), 45595d3b (MS6), 5521a893 (MS7).

| Q | Verdict | Rule (spec section) |
|---|---|---|
| MS1 | DIFFERS | row 25 pair = dead data (ctor 0, I/O only); row 30 = pylon-locked flag (§2.2, §6.2) |
| MS2 | MATCHES | WinMgr slot 42 = modal depth != 0 pauses day and month steps (§4.2) |
| MS3 | DIFFERS (part) | Init re-resolves neighbour city when no job and cell != (0,0); sets commute-check flag; no separate neighbor_job bool (§2.2.1) |
| MS4 | MATCHES | special list = occupant groups 0x150A/0x150B; distance truncated x87 sqrt, unsigned min (§4.9) |
| MS5 | DIFFERS | congestion = border ring only, both max edges clamp to CellCountX-1, capacity != 0 cells only; neighbour 1/0/1/1; agriculture city job => job_has_water 1 (§4.5) |
| MS6 | MATCHES | commute queries never ApplyTraffic; no trip recorded (§9) |
| MS7 | DIFFERS | announce check = RequestToPlaySound 4-second throttle; states 4 to work / 5 home / 6 elsewhere (§2.2, §4.1) |

Implementer impact (engine/core/src/mysims/, not edited here): congestion ring + clamp (MS5), agriculture water (MS5),
Init commute-check flag (MS3), RequestToPlaySound throttle on MySimMovedIn (MS7), pylon-locked persistence (MS1).
Frames for successors: Write/Read this = MySim+0xC, DoMessage +0x10, Init/Shutdown/RequestToPlaySound +0x8 or +0xB8
(cISC4MySim sub-object at +0xB8). No ABI rows, no game runs. Next: conformance vectors for MS5 ring and MS7 throttle.
