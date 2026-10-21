# Lane glue-propdev4 (finish/validate the cFC4PropDeveloper replacement, enable=propdev)

| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| glue-propdev4: validate cFC4PropDeveloper (enable=propdev); fix its deterministic-pacing idle cadence | .claude/worktrees/glue-propdev4, branch glue-propdev4 | **ready** (engine/plugin only, no C ABI change; gate PASS; game r1/sh1/r2 clean ref0, shadow 24/24 mismatches=0) | coordinator: union regress `--add-replace propdev` before promoting it to the default replace set |

## Report

### Why the replace credit was absent
No landed run ever enabled `propdev`: every regress log reads `subsystem 'propdev': off by default`, and the only
earlier run with `enable = propdev` (apply15b-cores) was before glue12b1 registered the classes. The glue12b1 classes
(cFC4PropDeveloper, cFC4WealthRequester) were complete; they had never been run in the game.

### Gap found and fixed (ce95ff32)
Under deterministic pacing (the default) the simulator gave the ORIGINAL `cSC4PropDeveloper` idle agent 20 SimIdle a
day at the settle point (FC4Pacing.h PacingFixedIdleSteps), but FreeCity4's own agent got 1 SimIdle a day, the rule
for our run-to-completion idle agents. Our prop developer does one block step per Idle (prop-developer.md §3.3), so
it did 62 block steps a month against the original's 588-651. Now `PacingFixedIdleSteps` applies to either agent
(cFC4Simulator.cpp AddAgent; the log line names "FreeCity4" or "original's"). After the fix the block steps per
month match the original's month for month (105, 588, 651, 630, ...).

### Game runs (Konradshohe, turbo to 2028-01-01, all clean, ref_warnings=0, 0 problems)
- glue-propdev4-r1 (enable=propdev, save 2027-03-01, before the fix): Read ok=1 (requesters=3, night=431, tod=1175,
  dated=19, power=466, wealth-objects=1), Init (AddAgent level 10 and 4, 13 subscriptions, tick registered),
  Write -> 1 at the save, RemovePropRequester -> 1, Shutdown (lists cleared, wealthAdded=0), destructor. 24 month
  lines, but 62 blocks/month (the gap above).
- glue-propdev4-sh1 (propdev_shadow=1, original developer): 24 month lines, 24 with mismatches=0; every step
  followed and raw-compared (scan flag and scanner exact). The list drop after load (tod 1175 -> 97, power 466 -> 2)
  is the original's too.
- glue-propdev4-r2 (enable=propdev, save, after the fix): same lifecycle trace; "the FreeCity4 idle agent
  'cSC4PropDeveloper' gets 20 SimIdle per day"; 14300 idle / 715 day deliveries; blocks/month identical to sh1;
  created 14, 100, 4, ... vs the original's 14, 102, 7, ... (member/wealth draws, not compared by design: §5).

### Gate
Baseline and final `qbuild all`: cargo 3652 passed, clippy 0, build PASS, ctest 62/62.

### Questions
None filed: the fix follows spec §3.3 (one block per Idle) and the existing pacing choice.

### Next steps
- Coordinator: union regress with `--add-replace propdev`, then decide promotion.
- Creations are not shadow-compared (generator draws); a seeded comparison would need the original's generator state.
