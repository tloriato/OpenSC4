# Lane apply15d

| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| apply15d | `.claude/worktrees/apply15d` (branch `apply15d`) | **ready** — the traffic answers the traffic glue still needed are applied: TG23 (`update_network_costs`, `SetFullExpenses` at five call sites), TG24 (path-info slots 14/3/7, so lane continuity runs) and TG25 (the counted tunnel pair, borrowed), plus the re-answers13 TS64 model (state 13 resets the saved scanning iterator). TG26 verified as already matching, untouched. Gate green: cargo test 2596 passed / 0 failed (baseline 2595, plus the new TS64 test), clippy `-D warnings` clean, rustfmt clean on every touched file, plugin builds, ctest **58/58** including `ret_channel_check` (0 flagged, 7 reviewed) and the new `traffic_cost_logic_test`. Three game runs on Konradshohe to 2032, all `clean` with `ref_warnings=0`; the traffic shadow is 89 lines, 55 exact, with the 34 differences all the known pacing one. `docs/abi/` untouched by me (rows staged in `build-runs/pending-abi/apply15d-rows.tsv`); `FC4_ABI_VERSION` not bumped; no C ABI export added, so `"abi": false`. No merge lock taken. | Merge `apply15d` into main with `--no-ff` (`build-runs/merge-queue/apply15d.json`). Then, for the coordinator: (1) `build-runs/pending-abi/apply15d-rows.tsv` holds four `cISC4PathInfo` rows for `interfaces.tsv`; (2) the six numbered questions below, of which question 1 is the one the task itself set up. |

## What changed

| Id | File(s) | Change |
|---|---|---|
| TG23 | `engine/plugin/FC4TrafficCostLogic.h` (new), `cFC4TrafficSimulator.cpp`, `engine/tests/traffic_cost_logic_test.cpp` (new), `engine/CMakeLists.txt` | `UpdateNetworkCosts` prices the 13 per-type counts with the §4.9.1 rule and sets roads items 2, 1, 3, 4 and mass transit items 3, 4, 5, 6 through `SetFullExpenses` (cISC4LineItem slot 12). The pricing is a shared header so a ctest can pin the worked example and the ties. The plugin's NULL bracket for `update_network_costs` is gone |
| TG24 | `engine/plugin/cFC4TrafficSimulator.cpp` | `PathInfoExitsFromEntry` (path info slot 14 then slot 3: every path from an entry, with its exit byte and its borrowed first vertex) and `PathInfoEntriesNear` (slot 7, the by-value `vector<u32>` through a hidden first argument, freed with allocator B), which is what feeds the core's `RemoveBadElevatedPaths`. `GetPathInfo` and the turn-around test were verified to MATCH; only their comments moved from `[choice]` to the answer |
| TG25 | `engine/plugin/cFC4TrafficSimulator.cpp` | `TunnelPairedPortal` reads the counted pointer at `+0x1D0` of the concrete `cSC4NetworkTunnelOccupant` and returns it **borrowed** — no `AddRef`, no `Release` — then takes the pair's `AsOccupant` (network occupant slot 48, `this + 4`), which is the `cISC4Occupant*` identity every occupant token of this host already is |
| TG26 | — | **verified, not changed.** `TrafficSimulator::shutdown` takes the source iterator and releases every connection reference of the snapshot before the connections go with the city (`cycle.rs`, §1, §4.1). `Read` never touches it |
| TS64 | `engine/core/src/traffic_network/subnetworks.rs`, `traffic_simulator/{cycle,state,shadow,tests}.rs` | State 13 drives the simulator's own scanning iterator (the saved `+0x818`) instead of a private index cursor: reset to state 0 at `(0, 0)` on entry with the rectangle and step kept, the budget tested **before each cell**, the pass resting in state 2 at `(x0, z1 + 1)`. Entering 12 does not touch it. `dayIndex` cited at `+0x918` (TS60). The shadow's `is_scan_phase` takes state 13 so a mid-scan stop is followed |
| test seam | `engine/tests/plugin_load_test.cpp` | The traffic stub answers the four callbacks the plugin now wires. Without them the seam host left those slots NULL and the plugin's real implementations ran against the stub's fake tokens (`SetFullExpenses` on a non-object — `plugin_ini_enable_traffic_test` segfaulted). Also a real TG23 check: `TrUpdateNetworkCosts` counts the calls and asserts PostCityInit makes exactly two, `UpdateMassTransitBudget` then `UpdateRoadsBudget` |

## Verification

| Item | How it is verified |
|---|---|
| TG23 | `fc4_traffic_cost_logic_test` (ctest) pins the §4.9.1 worked example (Road 538 → 54, Street 458 → 23, GroundHighway 21 → 10, …; roads items 2/1/3/4 = 55/23/60/6; mass transit 3–6 = 4/18/1/1) and the §11 test 16 ties (Street 10/30/50 → 0/2/2, 15/25 → 1/1), the two-type unsigned 64-bit carry and the out-of-range quirk (−3 → `0xFFFFFFFD`). The five call sites were already correct in the core and are untouched; `plugin_ini_enable_traffic_test` asserts PostCityInit's two calls in the §3.5 step 14 order. In game: `apply15d-traffic-1`/`-2` clean with the budget replacement active |
| TG24 | `cargo test` (the core's `traffic_network` lane-continuity tests, unchanged and still green) plus `ret_channel_check` clean. In game: `path_info_exits_from_entry` and `path_info_entries_near` now answer on every cell-graph rebuild of two six-year runs. The borrowed `GetPathInfo` and the whole-of-EAX turn-around test were read against §7.2/§4.2 and left alone |
| TG25 | `ref_warnings=0` on all three game runs is the refaudit evidence: the pair is never Released without a matching AddRef. `plugin_ini_enable_traffic_test` covers the no-pair path. The §11 test 16 requirement ("A's and B's reference counts are the same before and after the insert") follows from the borrow |
| TS64 | `traffic_simulator::tests::ts64_state_13_scanning_iterator` pins all four points: entering 12 leaves the iterator alone, entering 13 resets it to `(0, 0)` fresh with the rectangle kept, a finished pass rests at `(x0, z1 + 1)` done, and a deadline stop leaves it running at the last cell visited with the next call continuing from there. `apply15d-shadow-1` shows no state-13 or subnetwork difference |
| TG26 | Read `cycle.rs::shutdown`: the source iterator's connection references are released on shutdown. Left as it is |

## Gate

`build-runs/coord-verify-lane.ps1 -Engine <worktree>\engine -Tag apply15d` →
`build exit=0`, `100% tests passed, 0 tests failed out of 58`, `ctest exit=0`
(`build-runs/gate-apply15d.out`).

- `cargo test --workspace --target i686-pc-windows-msvc`: **2596 passed / 0 failed**. The
  pre-change baseline was 2595 (`build-runs/base-cargo.txt`); the one test this lane added is
  `ts64_state_13_scanning_iterator`. Recorded at
  `build-runs/final-results.txt`.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean (`build-runs/clippy.txt`).
- `rustfmt --check --edition 2021` on the five touched `.rs` files: clean.
- `tests/check_ret_channels.py`: `0 flagged use(s) not reviewed, 7 reviewed`. `SetFullExpenses`'s
  return channel is `unknown` in `interfaces.tsv`, so its result is never read.

One ctest failure appeared mid-lane and was fixed (`plugin_ini_enable_traffic_test` segfault);
see the test-seam row above. It was never green with the new callbacks and a stub that did not
answer them.

## Game runs

| Label | Verdict | Notable |
|---|---|---|
| `apply15d-traffic-1` | `clean`, `ref_warnings=0`, date 2032-01-02, 56.37 days/s | `enable = traffic` **and** `traffic_shadow = 1` as asked; the shadow reports INACTIVE (see question 1) |
| `apply15d-traffic-2` | `clean`, `ref_warnings=0`, date 2032-01-04, 55.98 days/s | `enable = traffic` alone |
| `apply15d-shadow-1` | `clean`, `ref_warnings=0`, date 2032-01-02, 5.42 days/s | shadow only: **89 lines, 55 exact** |

Shadow summary line: `traffic 89 lines, 55 with every mismatch count 0` (72 month lines, 55
exact; 17 day lines, all the pacing difference). `INCOMPLETE` lines 0. Details in
`docs/runs/apply15d-shadow-1.md`.

## Numbered questions

1. **`enable = traffic` and `traffic_shadow = 1` cannot both take effect.** The plugin is explicit
   (`traffic shadow: requested ... but INACTIVE: it needs FreeCity4's simulator and the original
   traffic simulator (do not enable traffic)`). The task asked for both in one request; I ran
   them as two requests instead (`apply15d-traffic-1` for the replacement, `apply15d-shadow-1`
   for the shadow). Is a combined mode intended, or is the split the right reading?
2. **§4.9.1 `p := r24(n × tileCost[t])`:** I keep `n` exact and round the product once, as the
   formula is written and as x87 does it (`fild` loads the integer exactly, the multiply runs at
   24-bit precision). Reading `n` as an f32 first would round it again above 2^24 counts. The two
   agree everywhere inside the spec's stated `0 ≤ p < 2^22` range. Confirm the exact-`n` reading.
3. **`tunnel_paired_portal`'s result.** The member is a counted reference held by the portal, so
   I borrow it (no `AddRef`, no `Release`). But the token this host hands back is the pair's
   `AsOccupant` (network occupant slot 48, `this + 4`), not the `+0x1D0` pointer itself, because
   every occupant token of this host is a `cISC4Occupant*` and the core reads the position of
   whatever comes back (occupant slot 8). The original instead `AddRef`s the pair, takes
   `AsOccupant`, then `Release`s. Both leave the reference counts unchanged. Confirm the
   pure-borrow + `AsOccupant` reading.
4. **`reset_scan` resets at `(0, 0)`,** not at the rectangle's own origin (TS31 and TS64 both say
   `(0, 0)`; state 2 at once if `(0, 0)` is outside the rectangle). Every city rectangle starts
   at `(0, 0)`, so the two readings agree in play; they differ only for a rectangle that does not
   contain the origin. Confirm.
5. **The shadow's remaining 17 of 72 month lines** are attributed to §3.4 *[choice]* (a work-unit
   count instead of the original's 4 ms deadline): every differing day line is `ours ran past the
   original's stop (state 15, progress 0)`, and the record fields on a differing month line are
   always `cycle state`, `phase positions`, `scanning iterator` and `statistics (working)` in
   equal counts. TS63 is **not** applied by this lane. Confirm this residual is the known pacing
   difference and out of scope here.
6. **TG24's "borrowed GetPathInfo and the turn-around test MATCH"** is read as *verify only*. I
   changed their comments from `[choice]` to the answer and left the code alone. Confirm no
   behaviour change was wanted there.

## Notes for the merge

- `build-runs/pending-abi/apply15d-rows.tsv`: four `cISC4PathInfo` rows for `interfaces.tsv`
  (slots 3, 4, 7, 14 of vtable `00a99e70`), with the TG24 signatures, pops and the borrowed/whole-
  EAX return notes. `docs/abi/vtables.tsv` already held the raw `cSC4PathInfo::vftable` block with
  those four addresses; `interfaces.tsv` is the curated table and had no `cISC4PathInfo` entry.
  I did not edit `docs/abi/` myself.
- No C ABI export added or removed (the host callbacks were already declared in `fc4.h` and
  `capi/src/traffic.rs`; this lane only filled in the plugin's function pointers), so
  `FC4_ABI_VERSION` is untouched and the merge-queue entry says `"abi": false`.
- `engine/tests/abi_entries.inc` was regenerated only by the normal build; never hand-merged.
