# Handoff (start here)

The one page every agent reads first: the current state, and where everything else is. The coordinator updates §0
before it stops and moves the stop note it replaces, verbatim, to [docs/history/](history/). Operating knowledge
lives in [docs/ops/](ops/); history is `git log` and [docs/history/handoff-2026-10.md](history/handoff-2026-10.md).
Keep §0 to the current state: what is true now, what is parked, and what is next. Narration goes in `git log`.

## 0. Current state (2026-10-21, run 4 active)

**Train25 landed bc937e312e (4 lanes):** PL19 replacement gate62/62 and union35-key regress PASS, both clean/ref0; aura71/71 and MySims73/73 remain exact, netmgr72/73 remains questioned. Terrain TR33-TR41 answers now landed; glue-terrain3 may resume after slots/check, with2 run4 rounds left. Audio and budget-window specs landed with export/asm evidence; default propmgr/lotcfg and exact-owner map promoted after69 tools tests. Coverage38.7/28.3/12.4/16.5 (run4 deltas+1.9/+0.0/+0.3/+0.6). Total21/60; five lanes active. The old noncanonical glue21 run narrative moved to history so run-table generation works.

**Run4 checkpoint15/60:** trains21 and22 landed; coverage36.8/28.3/12.4/16.2 versus baseline36.8/28.3/12.1/15.9 (verified+0.3, replaced+0.3 points). Five live slots are refilled after checks; terrain RE is capped3/3 and ready awaiting a batch, never round4. Audio/panels returned supported-export specs but reported no live REA app; checking that workspace diagnostic separately from the guard before accepting their coverage.

**Train22 landed 6df8889064:** occmgr replacement clean through2032, gate62/62 and union33-key regress PASS; default occmgr enabled only after landing (tools tests69 PASS). Coverage36.8/28.3/12.4/16.2, replaced +0.3 points. Terrain RE ready at its3-round cap (parked pending landing, never round4); five other lanes active, total13/60. Expanded budget recorded and pushed; finish batches at2+ ready.

**Train21 landed 9e5a0dd8e9:** gate/regress PASS; aura restored71/71, netmgr72/73 with NM10/NM11 filed, unknown-key warning fixed. Coverage36.8/28.3/12.4/15.9 (specified/implemented/verified/replaced), verified up0.3 points. Run224 normal pair exact73/73 each; run225 is the combined train. At the expanded checkpoint13/60 rounds are dispatched: verify1 landed; occmgr2 in train22 (gate62/62 PASS, union regress running); glue21 round2 resolving current-main integration; terrain engine1 blocked pending the ready terrain RE docs, capped at3 rounds. Four new lanes run: widget, panels and audio RE, effects core. Never spawn terrain RE round4.

**Run 4 is authorized; STOP was lifted by the user.** Limits, expanded by the user: 60 total lane rounds, 3 rounds per lane, 5 live lanes, at most 3 glue lanes. Refill a returned slot in the same turn after slots/check with explicit subsystem touches; do not wait for trains to land. Run batches as soon as 2 or more lanes are ready. Every 15 rounds record rounds, landed trains and coverage delta here. A lane with 3 rounds and no landing is parked and recorded here; do not restart it. Every workflow launch is maxRounds=1, with slots and check (resume for an existing worktree) before each launch. The guard is authoritative; no denied lane continues. At 60 rounds set STOP with reason run4 round limit, land ready work, regenerate and push. A usage/rate limit pauses refills until it clears; running lanes finish.

The live round ledger is build-runs/run4-coordinator-state.json; load it after compaction along with this section and lanes.py guard before any spawn. It records dispatched rounds and process IDs; do not infer the count from old guard totals. Main must remain clean; tool changes require pytest tools/tests and their own immediate commit.

**Work order:**
1. Verified regressions: compare train17 fed3a354 with train19 832c7238. Netmgr 71/73 (record11 lots, April/May 2026), aura 70/71, and netmgr_shadow unknown-key warning; fix or file a numbered question. This should restore verified coverage, currently 12.1%.
2. glue25fix3 at 209ff7fc: replacement g1 corrupt 2028-04-01; shadow-only g2 is not replacement evidence. Diagnose from branch logs. After a clean replacement run: regress including occmgr, train; only after landing add occmgr to regress Replace and coverage-map replace key.
3. glue-terrain3 at d112c328: gt3-repl-4 corrupt before game date; view/raw-record contract and slot96 Load already committed and gate 62/62. Diagnose crash; TR33-TR37 must be questions, never guessed.
4. glue21 at 405da5f0: PL19 is answered on main; implement per-call slot9 AddRef on every returned pointer including cache hits, remove the PL18 extra add, then validate against its trace.
Then choose work from coverage-report.md subsystem bytes, largest eligible first: spec yes/core absent gets core; core yes/replace absent gets glue after core lands; nonexact shadows get verify; absent spec gets RE. Use explicit subsystem touch keys on every check. Skip glue26 and ABI reconciliation.

**Baseline:** main 29e22e21, queue empty, no lane running at start. Train19 gate and full regress PASS, MySims exact73/73; train20 landed MS/PL documentation. Coverage 36.8% specified /28.3% implemented /12.1% verified /15.9% replaced. WIP branches above are pushed.

**Runtime:** CLI hosts must set CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS=0 and wait for TaskOutput. Use only the guarded saved fc4-lane workflow, never direct unguarded agents. The six-slot game queue is pid22296, started with a working Windows PowerShell module path after a Get-FileHash startup failure. RE spawns need a fresh REA preflight GO. After every train regenerate handoff tables, coverage and treemap; add a line here, commit and push.

The previous stop note is preserved verbatim in [history/handoff-run3-stop-2026-10-21.md](history/handoff-run3-stop-2026-10-21.md).

<!-- BEGIN GENERATED: latest-run -->
- **Latest in-game run:** 227 (train25 four-lane PASS, PL19 replacement promoted, terrain spec blockers answered, coverage38.7/28.3/12.4/16.5); 226 (train22 PASS, occupant-manager replacement promoted, replaced coverage15.9 to16.2 percent); 225 (train21 PASS, aura restored to 71/71, verified coverage 12.1 to 12.4 percent; netmgr differences documented); 223 (train19 PASS, MySims shadow exact 73/73 alongside the full regression, ref_warnings=0); 222 (MySims shadow exact 73/73 through 2032, including load, ref_warnings=0). See `docs/manual-checks.md`.
<!-- END GENERATED -->

## Where everything else is

| File | What | Who reads it |
|---|---|---|
| [ops/rules.md](ops/rules.md) | §2: clean room, never commit, public actions, commits | everyone |
| [ops/conventions.md](ops/conventions.md) | §8: cargo from `engine/`, fmt/clippy baseline, git, PowerShell, build infra | everyone; engine and tooling lanes before the first build |
| [ops/running-the-game.md](ops/running-the-game.md) | §3: regress, one run, ini keys, test cities, verdicts, hazards | coordinator; engine lanes with game runs; tooling lanes on the harness |
| [ops/lessons.md](ops/lessons.md) | §5: threading, refcounts, x87, differentials, demand (cited as "HANDOFF §5.x") | engine lanes |
| [ops/coordinating.md](ops/coordinating.md) | §6: spawning lanes, running the merge train, gate, operator notes | coordinator |
| [../.claude/workflows/fc4-lane.js](../.claude/workflows/fc4-lane.js) | the lane template (`Workflow({name: 'fc4-lane', args})`) | coordinator |
| [lane-tools.md](lane-tools.md) | `tools/lanes.py`: slots, check, brief, wait, monitor, ledger | coordinator; RE and tooling lanes (`wait`) |
| [token-ledger.md](token-ledger.md) | measured tokens per lane, merge share, tokens per coverage point | coordinator |
| [merge-train.md](merge-train.md) | how a lane lands: queue file, batches, hotspots | coordinator; every lane at its end |
| [lanes/README.md](lanes/README.md) | lane-file and run-file formats, the queue contract | every lane at its end |
| [lanes/INDEX.md](lanes/INDEX.md) | generated table: every lane's state and next steps | coordinator; a lane resuming another's work |
| [manual-checks.md](manual-checks.md) | generated in-game run log | coordinator; engine lanes comparing runs |
| [PROGRESS.md](PROGRESS.md) | milestones M0-M6 | coordinator |
| [coverage-report.md](coverage-report.md) | measured coverage | coordinator; tooling lanes |
| [specs/questions/README.md](specs/questions/README.md) | the question workflow | RE lanes; engine lanes filing questions |
| [../CONTRIBUTING.md](../CONTRIBUTING.md) | the clean-room roles (the single source) | everyone |
| [ARCHITECTURE.md](ARCHITECTURE.md), [../engine/README.md](../engine/README.md), [harness.md](harness.md) | layout, build, ini keys, switches | engine and tooling lanes |
| [re-workspace.md](re-workspace.md) | RE toolchain and workspace | RE lanes only |
| [history/handoff-2026-10.md](history/handoff-2026-10.md) | older stop notes and superseded HANDOFF text | nobody by default |

## 1. The project

Reverse engineering SimCity 4 Deluxe (GOG 1.1.641, exe SHA-256 `3bc5c7fe…a4b`) into a clean-room GPL-3.0
reimplementation that replaces the game piece by piece, OpenRCT2-style: Rust core (`engine/core`), formats
(`engine/formats`), C ABI (`engine/capi`), C++ GZCOM plugin (`engine/plugin`, `FreeCity4.dll` in the user's
`Plugins` folder), conformance vectors (`engine/conformance`), C++ tests (`engine/tests`). Specs in `docs/specs/`,
ABI facts in `docs/abi/`.

## 4. Milestones

[PROGRESS.md](PROGRESS.md); keep it current at every merge. M0-M2 done, M3 exit met (run 103), M4-M6 in progress.

## Section numbers cited elsewhere

§2 → [ops/rules.md](ops/rules.md), §3 → [ops/running-the-game.md](ops/running-the-game.md), §5.x →
[ops/lessons.md](ops/lessons.md) (same numbering), §6 → [ops/coordinating.md](ops/coordinating.md), §8 →
[ops/conventions.md](ops/conventions.md).

## 7. Waiting on the user

1. **Revoke the OpenRouter key** pasted into run 2's first message. It sits in plaintext in that session's transcript.
2. Done 2026-10-21: 195 merged worktrees removed (C: 94% -> 72% full). Kept: the five parked branches above,
   `train-integration`, and three app session worktrees (`claude/*`). Their `build-runs/` text files are archived
   in `build-runs/worktree-archive-2026-10-21.tar.gz` (local only). The branches remain, locally and on origin.
3. Two human checks: the budget window's look and the cheats (`docs/manual-checks.md`).
4. Upstream the SDK errata (public; re-run `docs/sdk-errata-draft.md` first). 5. Publish the repo (public).
6. Delete the local KillaVolt clone (`external/sc4-decomp`, `external/extracted`); download the Mac Rev A binary.
7. Keep or delete `build-runs/windown-backup-20261013/`.
