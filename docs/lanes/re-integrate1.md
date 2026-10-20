# Lane re-integrate1

| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| re-integrate1 (RE integration: automata state behaviours, effects EF1/EF3, vehicle/traffic ticks, aircraft/watercraft ticks) | `C:\Users\tlori\FreeCity4\.claude\worktrees\re-integrate1` (branch `re-integrate1`) | **ready** — docs only. Four RE reports that existed only as agent messages are now in the repo: the raw extracts are kept in `build-runs/re-integrate1-automata.md`, `build-runs/re-integrate1-effects.md`, `build-runs/re-integrate1-vehicle-ticks.md` and `build-runs/re-integrate1-air-water-ticks.md`. `automata.md` gains §6.2's named state-base fields, `PushState`'s signature, the `tBehaviorTemplate` Lua-key table, the per-type record layouts, and new §6.5 (per-tick behaviour of every state type) and §6.6 (aircraft/watercraft families); §8.1 corrects the riot end; quirks 10–18, questions Q7–Q20 and tests 24–32 added. `effects.md` corrects §3 (in-memory vs on-disk record sizes), §5.2 (the `[1,6)` LOD claim is false for 840 of 3373 components) and §7 (particles and decals are **not** ignored — `flags = 0`; the ignore bit is on game/attractor/camera and 98.7% of sound); §10.1 gains the measured `volcano_control` tree, §10.2 the slave-family census (143/33/20/21/3/6); quirks 1 and 8 amended, 9 and 15.1 added, EF6–EF10 filed, tests 11–15 added. `QUESTIONS.md` closes EF2 (statically) and EF3 and files EF1 (narrowed), EF4–EF10, AT6–AT19. `build-runs/pending-abi/re-integrate1-rows.tsv` has 15 rows. No engine changes, no game runs. | Merge into main (docs + the pending ABI rows; `abi: true` in the merge-queue file). An apply lane can then take AT6–AT19 and EF1/EF6 into the engine. The single most valuable follow-up is EF1's one probe: a runtime trace of `0x005BFD70` on a live `volcanocreator*` logging `comp+0x40` and each `IsActive()` return, which both closes how a particle component ever reports inactive and gives the real wall-clock end of `volcano_control`. Two report-vs-report conflicts are recorded in place rather than silently resolved: Plummet's `+0x28` is given as an initial descent velocity of −0.004166667 (−1/240) by the automata report and as −0.0080729163 at enter with a gravity of −0.00083333347 per frame by the vehicle report; PullOver's `+0x28` is an "initialised" latch in one and an elapsed-ms counter (0 = init, ≥1000 = finish) in the other. Both readings with their addresses are in `automata.md` §6.2/§6.5 and in the raw reports. |

## Where the raw reports came from

The four agents' transcripts were not in the paths the coordinator quoted (those `.output` files were 0-byte
stubs). The real JSONL transcripts are under

`C:\Users\tlori\AppData\Local\Temp\claude\...\` **and**
`C:\Users\tlori\.claude\projects\C--Users-tlori-FreeCity4--claude-worktrees-glue17\b3808f10-8715-44d8-a91d-e3e20f56fceb\subagents\agent-<id>.jsonl`

for ids `a2026b74f7d972a89` (automata), `a328b343056e78d05` (effects), `a5ed243b3af01b912` (vehicle/traffic) and
`a82f9042fccc7706e` (aircraft/watercraft). Each report was taken from the agent's final markdown answer and
copied verbatim. Two of them needed a second message appended: the effects agent wrote an initial report and
then a **measurement addendum** after a background search found a live `SimCity_1.dat` and an existing format
parser — the addendum **supersedes** its own earlier inference about particle flags, and both passes are kept
in the scratch file and in `effects.md` §7. The air/water agent appended a short note confirming there is no
Lua name table for the 50–58 / 70–78 state names anywhere in the exe or the Mac symbol artifacts.

## Not done / left alone

- `docs/abi/interfaces.tsv` was **not** edited. The `cSC4EffectComponent` rows for the visual-effect-as-component
  (`0x00AA1148`), brush (`0x00A9EDD0`) and game (`0x00AA0FF8`) vtables are already filed there by `re-specs16`,
  so they were not duplicated. What is new is `cSC4ParticlesEffect`'s component vtable `0x00AA0B88` (12 slots),
  the draw-pass override `Update(SC4DrawContext*)` at `0x00A9F25C`, `cSC4DecalEffect`'s `IsActive` at
  `0x008264B0`, and the off-vtable finish test `0x005BFD70`.
- Some measured addresses were **not** filed as ABI rows because a required column was not measured and guessing
  would have invented it: `cSC4DecalEffect`'s vtable base (only its pool ctor `0x00583190` was read),
  `cSC4DynamicParticleEffect::SetVoidParams` `0x00584170` and `AttachSlaveEffect` `0x005840E0`, and
  `cSCParticleSimulatorBase`'s `Start` `0x005B9CF0` / `Stop` `0x005B9E70` / `CheckForStop` `0x005B9D40`. Their
  addresses and behaviour are documented in `effects.md` §7 and §10.2.
- The stale unstruck copies of AM8–AM12 near `QUESTIONS.md` line 4954 were **not** touched. That file already
  holds their struck, answered text at its end (out of order, after the HG/LD/ZG block) — a pre-existing
  duplication from an earlier lane, not something these reports answer.
