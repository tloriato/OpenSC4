| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| glue-advisors-impl4: GZCOM glue for the whole advisor hierarchy (enable=advisors), implementation round 1 | glue-advisors-impl4 / .claude/worktrees/glue-advisors-impl4 | blocked: map mirrors + ctest landed-ready (gate PASS); system not enabled, AD4 (window direct calls) and AD5 (9 UI Lua entries) filed | RE answers AD4/AD5, then round 2: system/advisor/advice/subject classes + host on the core, game run through 2032 |

## Report (run4, implementation round 1, 2026-10-21)

**What changed** (branch glue-advisors-impl4, base main 604bcc05):
- `engine/plugin/FC4AdvisorMaps.h` (new, c1b704d7): `fc4::AdvisorGameMap<Alloc>`, the read-only game-layout mirrors
  that advisors.md §12.2 / AD3 allow: the system advisor map (slot 8, map<int32, cRZAutoRefCount<cISC4Advisor>>,
  signed key order) and an advisor's active map (slots 10/11, map<cRZAutoRefCount<cISC4Advice>, int32>, pointer
  order). Rebuilt whole from the core's state on change; unchanged content keeps the tree; each listed object holds
  one AddRef taken by the mirror and released when it leaves (so it stays alive while listed); a duplicate key is
  refused with the old mirror kept; Clear for Shutdown. Built with the game's node allocator (StlNodeAllocator) in
  the plugin, a fake in the test.
- `engine/plugin/FC4GameMapBuild.h`: `BuildGameMapBy` (a comparator for the signed int32 map); `BuildGameMap`
  delegates to it with the unsigned order (behaviour unchanged for its existing users).
- `engine/tests/advisor_maps_test.cpp` + CMake target `advisor_maps_test` (additive, end of list), fixed in 0972c4dd
  (the signed map is checked by its own in-order walk; WalkGameMap enforces unsigned order).
- Questions (docs/specs/questions/advisors.md): **AD4** (14f92796) calling convention / args / pops / return / `this`
  for the window route `0x41DB50` ctor, `0x41E320` Init, `0x41D650` show, `0x41D710` hide, `0x41D830` teardown, the
  ctor's initial refcount, which pointer +0x5C holds, refs the win proc keeps, and which advisor/advice slots it
  reads. **AD5** (a6ff203a) the 9 UI Lua entries of §8.2/8.3 ("UI only"): names, global/game placement, behaviour or
  original lua_CFunction addresses and whether they read the private global, and who else registers the 47 entries.
  QUESTIONS.md not regenerated (tools/ is off limits to this lane): coordinator runs `questions.py index`.

**Why the system is not enabled.** §4.1 step 10 requires slot 24 to build the original window (AD1: null/no-op is not
the original) and the only route is a direct call of the game's code; the spec gives addresses and bodies but not
the call contract, so FC4Compat cannot get a `Direct` entry without guessing (AD4). `register_api` must register 47
entries, 9 of which have no specified behaviour or addresses (AD5). No class was registered as a no-op or behind a
disabled path.

**Measured.** Full gate (`qbuild all`): cargo test 3652 passed / 0 failed, clippy 0 diagnostics, CMake build PASS,
ctest 63/63 PASS (incl. advisor_maps_test). Baseline run before the fix: 62/63 (only the new test's walker issue).
Coverage unchanged: advisors specified yes, implemented yes, shadow -, verified -, replaced absent.
**Game:** no request made: plugin behaviour is unchanged (no new class registered, no key), so a run would not trace
any advisor callback; the meaningful 2032/ref0 trace needs the enabled family (round 2).

**Next (round 2, after AD4/AD5):**
1. FC4Compat `Direct` entries for the five win proc functions from the AD4 answer (identified-executable check).
2. `FC4AdvisorsHost`: the 64 callbacks on game interfaces (Lua on the app's cISCLua instance 1 as cFC4Tutorial
   takes it; tags/gc, compile/call trigger, register_api 38 core + 9 UI per AD5; readers per §13), the MySim host.
3. GZCOM classes: cFC4AdvisorSystem (29 slots + message target + serializable; slot 8 -> AdvisorGameMap),
   advisor (21 slots; 10/11 -> AdvisorGameMap per advisor), advice (31), subject (14), registered under
   `enable = advisors` for the whole family incl. MySim/city-situation/reward/neighbour-deal class ids.
4. Game run `sim_turbo = 1` to 2032 with traced PostCityInit, ticker, Simulate, triggers, popups, window slots 25-27;
   then normal speed; then candidate union regress.

## Coordinator staged disposition
Queue only the tested map-building scaffolding and public AD4/AD5 questions; the advisor system remains disabled and blocked. Gate63/63 passed; no behavior/replacement credit and no real-game advisor validation claimed. One original scoping round plus this implementation round used2 total; only ONE final implementation round remains after AD4/AD5 land. Landing this prerequisite stage permits a dependent RE answer lane; it does not reset the budget.
