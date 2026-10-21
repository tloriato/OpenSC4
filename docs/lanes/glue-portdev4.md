| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| glue-portdev4 | `C:\Users\tlori\FreeCity4\.claude\worktrees\glue-portdev4` (branch `glue-portdev4`) | **ready.** No ABI change. Validated the existing glue12b2 airport/seaport classes (`enable = ports`) on a real port scenario: new tooling script action `port airport or seaport x z [facing [type]]` builds a stage-0 port lot through the game's interfaces; both developers adopt it. Ports shadow 144/144 exact (portdev4-shd-2), replacement clean ref 0 to 2032 (portdev4-rep-3) and at normal speed with a save (portdev4-rep-4n). Gate: cargo 3676/0, clippy 0, ctest 62/62. Question PD12 (seaport prop switch). | coordinator: union `--add-replace ports` candidate run; regenerate QUESTIONS.md for PD12; a Read-of-saved-ports run. |

## Report

### What changed
- `engine/plugin/cFC4PortDeveloper.{h,cpp}`: `PortScriptBuild(kind, type, x, z, facing)` (tooling). Reads the tuning
  exemplar (`0xCA270E9D` / `0xEA29AD92`, list `0x8A270FC3`) and the type's lot configuration list `0xCA271781`
  entry 0, then builds the lot as the §4.5 swap builds a stage: lot manager `CreateLot` (size from the configuration,
  slot 7), `SetLotConfiguration`, `UpdateZoneType`, lot developer `ConstructDefaultLot(lot, cfg, 0, true)`. It skips a
  site with any lot on it, and builds one port of each kind per session. CreateLot's reference is taken with
  `FC4_ACQUIRED` and dropped with `FC4_RELEASE` (as the swap host does). No new implementation of developer behaviour.
- `engine/plugin/FC4HarnessLogic.h`, `FC4Script.cpp`: the `port` action (grammar, dispatch), with parse tests in
  `engine/tests/harness_logic_test.cpp` and the unknown-action message in `plugin_load_test.cpp`. README script table row.
- `engine/plugin/FC4PortsShadow.cpp`: the core's §4.4 step 3 prop switch (`set_access_props_state`) was counted as a
  "growth or removal write" and showed up as a mismatch. It is now counted as `props=` in the `ours:` summary.
- `docs/specs/questions/port-developers.md` (new): **PD12**. The spec does not say which interface lists a seaport
  lot's props, where `0xC98204B8` is read, or which `cISC4Prop` slot SetState is, so the prop switch stays not wired
  (it is logged; advisor event `0x4A490D33` is pushed).

### Measured (game runs, all Konradshohe)
- portdev4-rep-1 (`enable = ports`, no script, turbo to 2032-01-01): clean, ref 0. No port in the city (ports=0).
- portdev4-rep-2 (ports + `build-runs/ports-sites.txt`): airport cfg `0xCA2436C0` (type `0x0A271653`, 6x18) at
  (2, 2); seaport cfg `0x4A52917D` (type `0xCA2D8CE6`, 11x5) at (0, 90). Both adopted (ports=1), required 700 / 500,
  history 100, the seaport raised the no-access event. ref_warnings=2 came from my own release in PortScriptBuild,
  fixed in c80c5fbb.
- portdev4-shd-1 (`ports_shadow = 1`, original developers, same script): 143/144. The one difference was the shadow
  classification described above.
- portdev4-shd-2: **144/144 mismatches=0** (airport 72/72, seaport 72/72), clean, ref 0.
- portdev4-rep-3 (ports + script, turbo to 2032-01-01): clean, ref 0, Init/Shutdown balanced (held 1 -> 0).
- portdev4-rep-4n (normal speed to 2026-07-01, `save` on 2026-04-02): clean, ref 0; `Write -> 1 (ports=1)` for both.
- Gate (`qbuild all`): cargo 3676 passed / 0 failed, clippy 0, build PASS, ctest 62/62. No Rust touched (no fmt).

### Not covered / next
- Read of a saved city with ports (the slot's saved copy is not reloaded by a later run).
- Growth (§4.5) was not triggered: no advice answer is scripted. The airport is skipped from 2026-06 on (no road;
  the original does the same, shadow exact).
- Seaport prop switch: waiting on PD12. QUESTIONS.md index not regenerated (tools/ is off-limits for this lane).
