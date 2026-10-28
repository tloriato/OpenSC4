# Next steps

The one pick-up document for the next coordinator session. It states what we do next and nothing else. Every write
replaces the whole file: no history, no logs, no "previously". Work that is done and verified is deleted from it.
Evidence lives in git, in `docs/lanes/` and in `build-runs/`, never here. Where another document disagrees with this
one, this one wins.

State as of 2026-10-28 14:30: **run 8 is running** (12 Codex lanes, `build-runs/run8-state.json`). Landed today,
each exact on its covers: zones, budget, parks, regionalcity, neighbors, citysit, commercial, propmgr, clock24. Also
landed: the save round trip (save-roundtrip33), draw streams in exact checks (draw-check35), corpus35, the save layouts
of every remaining record type (re-sr36, re-sr37) and the fast loop (fastloop36). All 189 references carry monthly
record hashes and snapshots (re-recorded 14:15 on pin r6).

**Priority (the user, 08:30): the fast loop, not measurement.** Do not spend game slots on headline numbers. A check
stops at the first month that differs and names its owner; a re-check replays one month from the last matching
snapshot (`docs/golden-monthly.md`). Measure Exact-replaced only from the per-owner verdicts of a credit run that lanes
need anyway.

## 1. The goal, worked backwards

**Goal:** a standalone game written in Rust/C++ that behaves exactly like SimCity 4: the same inputs under the fixed
schedule give identical results at the boundaries that survive the exe: the **save file** (decompressed records),
the **draw calls** into `cIGZGDriver`, **sound events**, **UI window state** and **resource lookups**. The C ABI seam
`fc4.h` is scaffolding; checks there are debugging aids, not proof.

## 2. The fixed schedule

`docs/specs/simulator.md` §5.8, applied to the original by `engine/plugin/FC4Schedule.cpp` (in-memory hooks only).
The reference store (`tools/golden.py`, `build-runs/golden`) keys each reference on the schedule code, the pinned DLL
and the run settings, and refuses a candidate whose schedule code differs.

**Pinned now (r6):** sha ee75f849, schedule code c7ac4ea4290c77db (main 3165c0612: a new train junction's four
indices start at 0, simulator §5.8 Uninitialised values; before it, the stack contents they carried into the traffic
network map's save made 6 of 55 scenarios differ with no owner on whenever the DLL changed; and corpus35's UI driver). While references are re-recorded, `build-runs/golden/RECORDING` tells lanes to merge main and
wait. A plugin change that alters what the original does (the UI driver, the schedule) needs a repin even when the
schedule code is unchanged: the store only refuses a different schedule code.

**Schedule requests.** Lanes never edit `FC4Schedule.cpp`/`.h`; they write a "Schedule request" section in their lane
file with the evidence. The coordinator finds the walk in the exe, adds the hook, states the order in simulator §5.8
(Address order row) and the owner's spec, commits, repins (`python tools/golden.py pin --dll <copy> --repin`),
records the requesting lane's scenarios first, then everything, and answers in the lane's spec task. A repin forces
every lane to merge main, so batch requests when more than one is open.

**Address order.** Where the original walks a container keyed by object address, the schedule makes it walk in
creation order or the order its spec states. Two detectors run the original with other addresses: `GOLDEN_INI_EXTRA=
"heap perturb = holes"` (or `= reverse`) `python tools/check.py run --dll <copy> --replace none --scenarios all`.
Both must stay exact on every recorded scenario of each pin; the chain below runs them last.

## 3. The metric

**Exact-replaced** (`tools/coverage.py`, rule in `docs/COVERAGE.md`): an owner's bytes count only if its key is in the
credit run's keys, a reference enters its code, every scenario entering it is exact in the credit run (all exact keys
on together, `python tools/check.py credit --dll <copy> --replace exact`), and no scenario still enters the original's
code of it with the key on. **Reach** is the ceiling: the share of game code the references execute.

First measurement (pin 1a5cc769, 2026-10-28 04:40): **Exact-replaced 0.0%** (1,442 bytes: crime 1,350, simgrids 46,
aura 36, gzfw 10). **Reach 39.8%** of game code (44.3% of the implemented owners' code); corpus34's three scenarios
add 45,081 bytes (census reach 42.35% to 44.05%), counted from the next measurement. 21 keys are exact alone on
their covers; with all 21 on together, 16 of 177 scenarios are exact.

Worked backwards, three things stand between those 21 keys and credit, each with a lane:
1. **Exact alone, not together** (`together34`): in most outside scenarios nothing diverges; a few save records
   differ, first from plumbing (30 scenarios), weather 12, aura 11, occmgr 9, history 6, power 2, civic 1.
   Data: `build-runs/run8/together34.txt`.
2. **The original's code still runs with the key on** (`wiring34`): span565 134 functions (36,854 bytes, entered in
   every scenario), gzfw 40, simgrids 27, tract 4, aura 4, propdev 3, weather 2 and 1-3 each for plumbing, span1555,
   span4444, occmgr, history, ports, protection, industrial. Data: `build-runs/run8/wiring34.tsv`.
3. **Deliveries that diverge:** delivery #21 to the lot developer (vtable 0x00AB1B88) is the most common first
   difference in the engine lanes' owners.

**The landing rule.** An engine lane lands only through `python tools/merge-train.py run --lanes <lane> --add-replace
<keys>`, then `land`: its check runs the credit keys plus the lane's keys on their covers and must be exact
(`regress --add-replace` reruns only the check). Batch every ready lane into one train. While a train runs, commit nothing under tools/ or engine/ on main: `land` then refuses after a passing check.

## 4. The loops

| Loop | What runs | Duration |
|---|---|---|
| Reference recording | `python tools/check.py record --scenarios all --jobs 12` | 50 min for all 189 with monthly snapshots |
| Per change | the lane's build, then `.claude/lane-bin/qcheck.py --wt <WT> --replace <keys>`: the first run stops at the first differing month, later runs replay one month from the last matching snapshot | about 30 s per scenario window; a full replay 20-90 s per scenario |
| Landing | merge-train run with `--add-replace`, then land | under 15 min gate + 20 min check |

The game queue runs two instances on the same 12 slots: the default (started 10-27 16:19, older code) and `fair`
(`-Instance fair`, commit 1b4bbe6b3), where a lane request waiting 15 minutes ranks with the coordinator's requests.
Restart the default at a quiet moment only (stopping it loses its running games).

## 5. Run 8

Lanes start with `FC4_LANE_BUSY_PCT=101 python tools/lane_launch.py build-runs/run8/specs/<name>.spec.json --state
build-runs/run8-state.json` (concurrency 12). A later round needs `"resume": true`; a merged branch continues under a
new name (growth-exact34). A relaunch loop restarts any lane whose round ended wip; ready lanes wait for review, blocked
ones for a relaunch with an answer. Codex "Selected model is at capacity" fails a round: relaunch.

| Lane | Owners | Aim |
|---|---|---|
| traffic-exact33 (round limit 18) | traffic | the accident timer's reset draw (traffic-simulator §4.13), then the map and statistics answers |
| automata-exact33 (round limit 18) | automata | the native accident call boundary (automata-managers §3.4.1): the fault in seven covers |
| network-exact33 (round limit 18) | netmgr, netrules, nettool, netlot | map iteration order (network-manager §2.2.1), then slots 10-12, tools |
| disasters-exact33 (round limit 18) | disasters, effects | effects tick and timer (effects §§6.1-6.1.1), height maps, period-start identity |
| re-blockers37a (spec) | automata, pathfinder specs | AD6 (Braus's post-load fault), TE33-PATH-1 |
| advisors-exact36 | advisors, mysims | citysit landed exact |
| growth-exact36 | residential, development, demand, zonedev, lotdev, lotcfg, exemplars | zones, commercial, propmgr landed exact |
| services-exact36 | power, pollution, landvalue, ordinance, demolition, dispatch, … | budget 8/8, dispatch 9/10 |
| together34 | the 21 exact-alone keys | exact with all keys on |
| wiring34 | span565, gzfw, simgrids, tract, … | the original's code of switched-on owners stops running |
| save-roundtrip36 | formats, save, persistence | the last three types (SR36-1..3, specified by re-sr37): 169 of 169 |

**Known issues.**
- Run-8 worktrees are 9-13 GB each: remove a merged lane's worktree at once (`cmd /c "rd /s /q \\?\<path>"`, then
  `git worktree prune`; plain removal fails on long paths). Disk at most 85%.

## 6. The standalone track

1. **Save round trip:** every reference and shipped save comes back with no decompressed difference; save-roundtrip36
   decodes 164 of 169 record types (98.6% of bytes) in their owners' modules and is finishing the last three.
2. **Headless host:** our exe loads the data files and a save, runs N days under the fixed schedule, writes a save
   compared with the same references. It implements the host side of `fc4.h` over our own world model; no host crate
   yet.
3. **Renderer and UI checks:** the draw-call capture and `tools/drawcheck.py` are on main (draw-oracle33); next,
   streams from ours against the original's on paused saves, then live under the fixed schedule.
4. Then a standalone renderer behind the same interface, UI, audio.

## 7. Stopped (do not restart)

Record/replay at the `fc4.h` seam as the main loop; house-city verified, strict and shadow-based credit; single-method
credit lanes; an operator layer between the coordinator and the lanes.

## 8. What the user does (5-minute checks)

Any check that costs the coordinator about an hour once and the user about 5 minutes: comparing UI screens, clicking
through dialogs, and playing and recording short sessions for the corpus. Give exact steps and what to look for.
