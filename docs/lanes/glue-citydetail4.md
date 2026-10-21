| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| glue-citydetail4: validate cFC4CityDetail replacement (enable=citydetail) and its native shadow in game | Claude lane, .claude/worktrees/glue-citydetail4 | ready: gate PASS; cd4-shadow-a clean mismatches=0; cd4-repl-a clean, real reduce pass (removed=8496, 525 network lots); round 2: increase pass and two-session reload blocked on contract, CD6/CD7 filed | coordinator: candidate union --add-replace citydetail before promotion; regenerate QUESTIONS.md for CD6/CD7; answer CD6/CD7 (harness keys) to close the two gaps |

## Round 2 (2026-10-21): the two validation gaps

No code change, no new game run (each would repeat a run already done or give a zero value without a reason). Commit: questions + README fix.
- **Increase pass (preference 3 vs saved level 1):** no public path exists. The harness has no request or `[ini]` key for the detail preference. The slot's preference is 1 and the city's record is 3, so only reduce can run.
  Writing `SC4Preferences::videoPreferences.cityDetail` from the plugin is not a supported path. Spec §3.1 says the lot configuration manager filters its catalogue with d when it builds it, so a later write leaves the catalogue at d=1, and the increase pass would have nothing to create (created=0, the zero value the task rules out).
  The write also risks the game saving the changed preference to the user's config at exit. Filed **CD6** (start-up preference key / ApplyVideoPreferences contract, catalogue rebuild timing).
- **Reload in a second process:** harness.md says each slot starts from a fresh copy of the region, and `save_as` is refused by the script parser (the alias contract is undocumented). So a save by ours cannot be reloaded in a distinct process without a new harness feature. The baseline city stays untouched and no alias artifact exists to clean up.
  Filed **CD7**. The evidence so far comes from one process: cd4-shadow-a shows the native Read accepts our record and both classes write identical bytes; cd4-repl-a traced Read/Init/Write x2/Shutdown, clean, ref_warnings=0.
- engine/README.md: the citydetail entry's stale "question CD6" now cites §3.4. The code comment at cFC4CityDetail.cpp:243 still says CD6 "was never filed". It was left unchanged, because editing it would be a code change that needs a full gate. The next code round should drop that sentence.
- QUESTIONS.md was not regenerated, because lanes may not run tools/. The coordinator runs `questions.py index`.

## Report

**What changed** (commit ead3c541, plugin only, no ABI or Rust change):
- `engine/plugin/cFC4CityDetail.{h,cpp}`: `Shutdown` logs `citydetail Shutdown -> 1 (level, inits, QueryInterface own/serializable/refused, GetCityDetail)`.
  The counters are atomic (§5.1). The Read/Write log lines now read the core level directly, so their own reads do not count as GetCityDetail calls.
- The network-lot callback comment named a question "CD6" that was never filed (QUESTIONS.md: city-detail next free ID is CD6). It now cites the public contract:
  city-detail.md §3.4 redevelops every lot, and network-manager.md slot 11 stops at the first false, so the callback returns true. No question is needed.

**Gate** (`qbuild all`, baseline and final): cargo test 3756 passed / 0 failed, clippy 0 diagnostics, CMake build PASS, ctest 63/63.

**In-game runs** (Konradshohe, turbo to 2032-01-01, script `build-runs/cd4-script.txt`: full save 2028-01-05, digest, fast save 2031-06-01; the same DLL, sha256 6d283439):
- `cd4-shadow-a` (citydetail_shadow=1, original class live): clean, ref_warnings=0, reached 2032-01-17. Shadow line: `setting=1 fresh=1/1 record 0000000001000000 read-same=1/1 read-none=1/1 read-v1=read 0/0; mismatches=0` (native vs Rust core, 1/1 load lines clean).
  Our core's dry runs at PostCityInit (after the original already reduced): level 2 and 3 give reduce removed=0, network-lots=525. Both saves ok=1.
- `cd4-repl-a` (enable=citydetail): clean, ref_warnings=0, reached 2032-01-04. Trace:
  - created, then `Read -> 1 (level 3)` (the slot city's record is 3, while the slot's preference is 1);
  - `Init: setting=1 level 3->1 pass=reduce removed=8496 created=0 draws=0 network-lots=525 (lot order: the lot list)`;
  - `Write -> 1 (level 1)` at the full save and again at the fast save;
  - `Shutdown -> 1 (level 1, inits=1, QueryInterface own=1 serializable=3 refused=0, GetCityDetail=0)`. GetCityDetail=0 matches spec §3.1: nothing calls slot 5.
  - The city ran through 2032 after a real reduce pass (8496 prop/flora removals and 525 RedevelopLot calls), with no exception.

**Limits:**
- The shadow does not compare the reduce/increase pass on the original, because the pass would change the live city. The original's own pass has already run before PostCityInit, so the native and Rust removal counts are not compared in game.
  Conformance vectors cd8_04/cd8_05 cover the pass logic.
- The increase pass is not exercised in game: the slot's preference is 1 and the record is 3, and no script action changes the preference.
- No reload of a city saved by ours: each slot starts from a fresh region. The record format is covered by the shadow: the native Read accepts our bytes and both write identical bytes.
- DIGEST funds differ between the two runs from 2026-02-01 (935 vs 957). These are two separate clock-seeded sessions, not a paired comparison, so the difference was not attributed to anything.
- Both requests ran at the same time (slots 1 and 2): the queue picked the second up from `build-runs/game-staging/` before it was moved into the queue folder. Use one-at-a-time staging outside build-runs next time.

**Questions:** none filed.

**Next:** coordinator `candidate union --add-replace citydetail`. Optionally, a run at preference 3 against a level-1 record would exercise the increase pass, but no harness switch exists for the preference.
