# Lane cores25

| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| cores25: the effects manager core past the corrected rules (tick, brush and game records, select draw, draw-pass finish sweep and end post) | `cores25` / `.claude/worktrees/cores25` (branch `cores25`, engine lane) | **ready** — `fc4_core::effects` grows from the EF1-EF5 rules into the manager: `directory` (3, 4.1, 5.2), `select` (5.3), `brush` (8), `game` (9), `manager` (1, 2, 4-7), `shipped` (8.4, 9.4). `fc4_formats::effects` gains the brush (39-byte), attractor and game (70-byte at v1) records. 7 `ef17_*` conformance vectors. Gate green: cargo test **3175 passed / 0 failed** (main at merge 3087 for this tree, baseline on the starting main 3019), clippy `-D warnings` clean, rustfmt clean on every touched file, plugin builds, ctest **58/58** including `ret_channel_check`. `abi_entries.inc` regenerated **unchanged (178 interfaces)** — no C ABI export added, `FC4_ABI_VERSION` not bumped. No game runs (core-only lane; `enable = effects` is not on main, that is glue22's). Merged `main` (train4, which landed apply15e) and resolved the two add/add conflicts by keeping this lane's supersets. Questions 1-7 below | Next: (1) settle Q1 (the LOD census: 5.2's per-range table sums to 887, its text says 840) and Q2 (the LOD formula versus its "zoom 1-5 gives 1-5" sentence); (2) wire the manager into the plugin behind `enable = effects` with an `effects_shadow`, and run the §17 cases in game; (3) decide Q3 (where the finish sweep runs when there is no 3D view) before any harness depends on the end message; (4) parse the particle and decal description sections so §7's `life` values drive the stub components, closing Q4; (5) add `ef_` ops for the manager-level cases of §17 tests 7, 10 and 11, which the core tests cover but the vectors do not |

## What was implemented

| Id | File(s) | Change |
|---|---|---|
| EF1 | `engine/formats/src/effects.rs` | The on-disk reader, extended with the two record kinds the directory holds and the reader did not cover: `cSC4BrushDescription` (§8.1, **39 bytes** packed, no padding after the one-shot byte) as a `u32` count then records (shipped: 25); the attractor record (§3 step 4, a `u16` version, a string and a `u8`, shipped: 64); and `cSC4ScrubberDescription` (§9.1, **70 bytes at version 1**), whose stream order is **not** the offset order — `+0x28` and `+0x2C` are written last and only when the version is above 0, so a version 0 record is 62 bytes |
| EF2 | `engine/core/src/effects/directory.rs` (new) | §4.1 `FindEffectDesc`: the highest priority `p` with `0 < p ≤ maxPriority`, ties broken by the first entry met in the name map, and the `index = −1` case where the call **still succeeds**. Case-insensitive name lookup (§3 step 8). `ComponentDesc::reaches_table` carries the §5.2 narrower claim |
| EF3 | `engine/core/src/effects/select.rs` (new) | §5.3: the manager's `cRZRandomSuper` seeded `0x1105`, drawn with `RandomUint32Uniform(0x10000)` at the first component of each group run, the weight sum reaching `r`, then `r := 0x10000` so the rest of the run is skipped. Group 0 is always created and draws nothing |
| EF4 | `engine/core/src/effects/brush.rs` (new) | §8.2, §8.3: creation draws strength then radius from the static `cRZRandom` (two draws even for the degenerate shipped ranges). The stroke accumulator `acc = r24(r24(dt·rate) + acc)`, `t = r24(dt + t)`, `n = floor(acc)`, `acc = r24(acc − n)`, `active := 0` at `t > length`; the one-shot that applies `trunc(rate)` and never opens a long terrain operation. `floor` is the 1.5·2²³ idiom with the equality fix-up. The out-of-bounds quirk (§8.3 step 1) applies nothing **and does not update `prev`** |
| EF5 | `engine/core/src/effects/game.rs` (new) | §9.2, §9.3: `pending`/`started`/`timer`/`count`/`cell`/`data`; `SetParameter(13)` replaces `data` (the message's `data1`); a change of cell sets `pending`; `Start` raises Pause/HiddenPause/clock and `Stop` undoes them in the order resume, hidden resume, clock. `single` applies once and goes inactive **without** `Stop`, so its pauses are never undone (quirk 5). `Apply` runs burn, `SetCause(4)`, toxic, demolish (rect clipped to the city), extinguish, message, map — in that order |
| EF6 | `engine/core/src/effects/manager.rs` (new) | The manager: state (§2), creation (§4), activation and component building with the LOD window and select groups (§5.1-§5.3), the tick (§6) and the draw-pass finish sweep with the end post (§7). A meta component spawns its child through the same path and its `IsActive` is the child's `started` byte |
| EF7 | `engine/core/src/effects/shipped.rs` (new) | The 25 brush records (§8.4, TR2) and 57 game records (§9.4) as tables, with the conformance rows they name pinned |
| EF8 | `engine/core/src/disasters/*`, `engine/capi/src/disasters_tests.rs` | From apply15e: the measured `volcano_control` finish condition (§10.1) and `data1 = the instance` only for meta-reached slaves (§10.2) |
| EF9 | `engine/conformance/src/effects.rs` (new), `lib.rs`, `runner.rs` | The `ef_` ops and the seven §17 vectors |

## The corrections honoured

- **§3** — the record tables give *in-memory* sizes; on disk the component record is variable-length and nested in the effect record. The brush and game readers here follow that: 39 and 70 bytes are the *on-disk* sizes, and the game record's stream order is not its offset order.
- **§5.2** — the `[1, 6)` claim is not asserted over all components. `ComponentDesc::reaches_table` carries the narrower brush/game claim the existing test checks, and the LOD census test pins the 3373 / 2533 / 840 figures as **text** while separately pinning that §5.2's per-range table sums to 887 (question 1).
- **§7** — particles and decals are *not* ignored. `runtime_flags` copies `desc+0x10 & 1` into runtime bit 2 (value 4) and adds bit 1 (value 2) for table types 3-15; the finish test tests the runtime bit. The shipped census is asserted as the corrections state it.
- **§10.1 / §10.2** — `volcano_control` waits a full extra draw-pass sweep for its meta child, and `data1 = the instance` only for meta-reached slaves. Both are exercised through the manager.
- **EF2 quirk 1 (proven)** — effects finish and post their end message **only in the 3D view's draw pass**. `draw_pass_sweep` is the only place either happens, and nothing synthesises a finish outside it. §17 test 10 is asserted end to end: 200 ticks past the brush length post nothing, and the first sweep posts exactly one.

## What is stubbed

- **Particles, decals, sequences, sounds, cameras, attractors and models** are `ComponentState::Stub` with a Start/Stop-driven active flag. §7 counts those whose ignore bit is clear, so the flag is modelled; the *rendering* and the emitter/fade lifetimes are not. §16 EF1 is open (how a `cSC4ParticlesEffect` ever reports inactive), so the stub's lifetime is not invented. Question 4.
- **Animations** stay stubbed, as the lane asked.
- **Effect maps** (`§9.3` step 6) are reported as an action so the order is complete; the map itself is render and out of scope.
- **`CreateVisualEffect` by ID pair** (§4.2) resolves an ID pair to a description index, because the directory keys by name and the spec gives no ID-pair table. Question 5.

## Vectors added

`engine/conformance/vectors/ef17_01_02_brush_stroke_counts.json`, `ef17_03_select_draw.json`, `ef17_04_05_brush_apply.json`, `ef17_06_08_game_records.json`, `ef17_09_priority_filter.json`, `ef17_11_12_finish_rule.json`, `ef17_13_15_census_and_volcano.json` — seven vectors covering §17 tests 1-6, 8, 9, 11-15. Tests 7, 10 and 11's manager-level cases are covered by `engine/core/src/effects/manager_tests.rs` but not yet by vectors (question 7).

## Gate

| Check | Result |
|---|---|
| `cargo test --workspace --target i686-pc-windows-msvc` | **3175 passed / 0 failed** (baseline on the starting main 3019) |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| `rustfmt --check --edition 2021` on touched files | clean |
| `build-runs/coord-verify-lane.ps1` | plugin builds, ctest **58/58**, `ctest exit=0` |
| `python tests/gen_abi_entries.py` | **unchanged**, 178 interfaces |

## Questions

1. **§5.2's LOD census: the table and the text disagree.** The text says 2533 of 3373 components are `[1, 6)` and **840** are not; the per-range table printed alongside sums to **887** (182+156+141+106+101+98+30+27+23+10+8+5). 3373 − 2533 = 840, so the text is internally consistent and the table over-counts by 47. I asserted the text figures and pinned the table's own sum as 887 so the discrepancy stays visible. **Ask:** which is the measurement? If the table is right, then 2533 should be 2486 and the "840" figure in §5.2, quirk 8 and §17 test 14 all need re-stating.
2. **§2's LOD formula versus its own example.** "`max(zoom − offset + 1, 1)` … Zoom 1–5 gives 1–5 when the offset is 0" — the formula gives 2–6 for a 1-based zoom. The two agree only if `zoom` is 0-based (0–4). I implemented the formula with a 0-based `zoom` and added `lod_for_zoom_level` that takes the user-facing 1–5 and subtracts one. **Ask:** confirm `zoom` is 0-based in `+0xC84`'s computation, or that the `+ 1` is not there.
3. **Where does the finish sweep run when there is no 3D view?** EF2 quirk 1 is proven and I did not synthesise a finish outside the draw pass, as the lane asked. But §15 says a session that never draws never finishes an effect and never posts the end message, which leaves a volcano in state 5/7 and an effect-disaster emergency pause held forever. A replacement that must work without a 3D view "has to run the same sweep from somewhere else, or synthesise the end message". **Ask:** for the game queue / harness, should `enable = effects` run `draw_pass_sweep` from the animation tick or from a harness-only hook? That is a plugin or harness decision, not this core's, but the core will need a documented caller.
4. **How does a particle component ever report inactive (§16 EF1)?** The shipped data gives all 1921 particle components `flags = 0`, so the finish test waits on them, yet `cSCParticleSimulatorBase::started` is written only by `Start`/`Stop` and `CheckForStop` fires only when a stop was already requested. I left particles as Start/Stop-driven stubs rather than invent a lifetime. **Probe:** the emission path around `cSC4ParticlesEffect::CreateParticle` `0x005B7C20`, or a runtime trace of `0x005BFD70` on a live `volcanocreator*`. Until then the crater decal's 50 s is the bound §7 gives, and the manager cannot reproduce `volcano_control`'s real end time.
5. **`CreateVisualEffect` by ID pair (§4.2).** The directory keys by name and the spec gives no ID-pair table, so `create_visual_effect_by_id` takes a description index. **Ask:** what the two `uint32`s are (type/group of the effect, or a resource key) and where the shipped pairs live.
6. **The game description's `+0x10`.** §9.1's binary record reads and writes `u32 +0x10` alongside the demolish radius and flags, but no text option names it. I kept it opaque in `GameDescription::field_10`. **Ask:** what it is.
7. **§17 tests 7, 10 and 11 have no conformance vector.** Test 7 (move to a new cell) needs the `ef_game_actions` op to take a sequence of positions; test 10 (no end without the draw pass) and test 11's manager-level cases need manager ops (`ef_create`, `ef_tick`, `ef_sweep`, `ef_expect_posts`). All three are asserted in `manager_tests.rs`. **Ask:** whether the vectors should grow manager-level ops, or whether the core tests are enough for a core-only lane.
