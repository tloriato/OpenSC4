| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| verify-regress4 (train17 vs train19 shadow regression: netmgr 73->71/73, aura 71->70/71, netmgr_shadow ini warning) | `verify-regress4` / (engine lane) | **merged train21 (9e5a0dd8e9)** (plugin + capi, no C ABI export change; gate at 28971497: cargo 3577/0, clippy 0, ctest 62/62; game vr4-a1 clean, ref_warnings=0, aura 14/14 exact, netmgr 11/13 with every difference under NM10/NM11, no unknown-key warning) | answers to NM10, NM11, AU16; a normal-speed netmgr run; coordinator regress to confirm aura and netmgr counts |

## Report

**Cause.** The engine delta between train17 (fed3a354) and train19 (832c7238) is MySims shadow wiring; neither
netmgr nor aura code changed. Both drops come from game-run timing: train19's turbo run scheduled the cycle and the
lot-state messages differently (aura cycle 2 took 11 days, against 9 in train17). train17's own previous run was
already 72/73 on netmgr, so that count was never stable.

**Aura (fixed).** Train19's cycle-2 difference (landmark map 208 tracts, max |d| 188, orig -91 / ours -96 at 3,6)
matches the worked example in aura.md section 5.3: two landfill tiles inserted while state 3 was running. The original
reads one department per call (section 6.3). The shadow ran all of state 3 at the 3->4 move, so it read the department grid
after the insert. Now the plugin also reads the original's departmentCursor (+0x7C, section 14.3). When the original stays
in state 3 but moves its cursor, the core's new 3->3 step runs one department (`aura_shadow.rs` step_department).
Test: `aura_tests::shadow_follows_state_3_one_department_per_call`. Section 14.3 names only state 5, so this is filed as
AU16.

**Netmgr (not suppressed, questions filed).** Train19's two mismatches (2026-04: lot 20,103 held only by the
original; 2026-05: lot 16,93 held only by ours) are at cells that were not yet in the NM10 first-row set. In
train17 the lot-state message on those cells came first, so the same cells were counted under NM10. NM10 had been
cited in the code but never filed; it is filed now. New diagnostics log the network occupant's variation, rotation,
flip, usable flag, zone and wealth, and whether the configuration is compatible, for every lot that only one side
holds. Run vr4-a1 repeats both cells: the original ran UpdateRoadsInCell there when no followed message had put
their keys in ours' pending set. Filed as NM11.

**Config.** `netmgr_shadow`, `netmgr_shadow_order` and `netmgr_shadow_wide` were read but never registered in
FC4Config.cpp. They are now registered, and vr4-a1 has no unknown-key line.

**Commits.** 3a3e40b8 lane file; 28971497 fix, diagnostics, questions; later commits hold the lane file and the
NM11 evidence.

**Game.** vr4-a1: turbo to 2027-01-01, Konradshohe, simulator on, budget off, aura_shadow and netmgr_shadow on.
Verdict clean, ref_warnings=0. Aura 14 cycles, every mismatch count 0. Netmgr 13 lines, 11 exact. 2026-02 and
2026-03 differences are all under NM10/NM11.

**Next.** Answers to NM10, NM11 and AU16. One normal-speed netmgr run, since vr4-a1 was turbo only. The
coordinator's house regress will measure the new counts.
