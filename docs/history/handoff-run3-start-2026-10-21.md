## 0. Current state (2026-10-21, run 3 starting)

**Run 3 train19 landed (832c7238e0):** MySims shadow exact 73/73 in the combined regress, gate and regress PASS, ref_warnings=0; MS1-MS7 answered. Coverage now 36.8% specified / 28.3% implemented / 12.1% verified / 15.9% replaced (the current regress remains the authority). STOP is set at 8/8 rounds; PL19 docs are queued for the final docs train.

**The wind-down is lifted for run 3** (the user, 2026-10-21). The harness now enforces what was prompt text before:
`.claude/lane-bin/laneguard.py` runs before every lane tool call and enforces the context budget, a cap on calls
between commits, the STOP flag and parking. The RE gate checks that REA is reachable. See
[ops/coordinating.md](ops/coordinating.md) "Lane guard". Lane output is capped (`q.py`, `qgrep.py`, `re_query.py`).
A lane denied by the guard returns; do not route around it. To wind down: `python tools/lanes.py stop "<reason>"`,
then land what is on disk.

**Main** is pushed, the merge queue is empty, and no lane is running. The gate on main passes (clippy clean, build,
ctest 62/62). Coverage: **36.8% specified / 28.3% implemented / 12.4% verified / 15.9% replaced** (replaced was 12.1% until the replace list caught up with the landed replacements, `regresslists6`). Run 2 (the
lean-lane GLM run, 2026-10-20 20:31 → 10-21 08:24) landed trains 10-18. Its cost and what it bought are in
[ops/efficiency-log.md](ops/efficiency-log.md).

**Parked work** (every branch is on origin; every other worktree branch is fully in main):

| Branch | Tip | What | Next |
|---|---|---|---|
| `glue21` | `405da5f0` | `enable = propmgr`/`lotcfg` glue; glue21fix2 rounds 1-5 | blocked on PL19 (the growth-pass wave drains lot configurations to zero); handoff in `docs/lanes/glue21fix2.md` on the branch |
| `mysims1` | `53403af4` | the MySims shadow, `FC4MySimsShadow.cpp` (~1,750 lines); lane gate PASS | its game run, then the regress, then the train |
| `re-answers35` | `aa328486` | lane file only. MS1-MS7 are unanswered | redo with REA available; the last attempt cost ~$16 for nothing |
| `glue25` | `60a69143` | WIP from the 2026-10-16 halt: the occupant manager's GZCOM class, `enable = occmgr` | 5 days stale; rebase or redo |
| `glue26` | `fca64f23` | WIP from the 2026-10-16 halt: zones ZG22 (slot 19 `*ok`), ZG24 (HighlightParcels region) | 5 days stale; rebase or redo |

**Open items, ranked:**
1. `enable = propmgr`/`lotcfg`: PL19 (glue21 above).
2. `enable = occmgr`: glue25 (parked above), now that the occupant specs have landed. Rebase it in one fix round.
3. `enable = terrain`: the terrain 3D view contract (TR21, `terrain.md` §15.5) landed in train14. The glue is next.
4. The MySims shadow (mysims1 above).
5. ABI: 63 lane proposals disagree with main's row for the same key: [abi/unreconciled-rows.tsv](abi/unreconciled-rows.tsv).
   Decide each one; don't apply in bulk.
6. Efficiency: the context budget and stall kill are now enforced (laneguard). Still open: the provider routing (the
   DigitalOcean pin did not hold in run 2). See "Next levers" in [ops/efficiency-log.md](ops/efficiency-log.md).

After every landing: `python tools/gen_handoff_tables.py`, `python tools/coverage.py` and
`python tools/coverage_treemap.py`, then push. Check `git merge-base --is-ancestor <lane> main` for each landed lane.
Operator notes (union glue, ABI rows, stalls) are in [ops/coordinating.md](ops/coordinating.md) "Merge-train
operator notes".

<!-- BEGIN GENERATED: latest-run -->
- **Latest in-game run:** 223 (train19 PASS, MySims shadow exact 73/73 alongside the full regression, ref_warnings=0); 222 (MySims shadow exact 73/73 through 2032, including load, ref_warnings=0); 221 (the network-tool replacement places roads and streets in the real game (inserts=8..69, valid=solved, failure 0), 20/20 scripted placements ok, city runs to 2032 clean with ref_warnings=0.); 216 (component 53 / 11 save round trip, record53 equal at load and save); 215 (devperf2 shadows over the replacement exact after the cell-walk host change). See `docs/manual-checks.md`.
<!-- END GENERATED -->

