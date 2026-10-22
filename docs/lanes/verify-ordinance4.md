| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| verify-ordinance4 (independent ordinance shadow) | `verify-ordinance4` / (engine lane, detached worktree) | WIP: cargo4938/0, clippy clean, ctest68/68; native turbo and normal clean2032/ref0 but1760 serialized differences each (SV3); combined native ref1; current40 clean2032/ref0; ABI unchanged | (1) resolve SV3; (2) fix crime native ordinance lifetime; (3) capture actual native host reads/effects and natural lifecycle/save/reload calls |

## Report and measured milestones
- Goal: independent same-process original-versus-Rust ordinance verification; native authoritative.
- Worktree reused at detached HEAD; initial tree clean, no main.. commits.
- Implemented first independent serialized bootstrap + monthly follow + natural simulator SDK dispatch observation.
- Native remains authoritative. No native algorithm wrappers, repair/resync, promotion, or ABI edits.
- Done: 2dedf01b adds opt-in shadow; subsequent changes add direct ordinance SDK query/set observers, scripted public simulator setters, parser bounds tests and QI runtime owner identity.
- Next: stable final qbuild all, immutable DLL game run with ordinance_shadow and scripted boundary actions, then current40 replacement control; save/reload control if feasible.
- Baseline: cargo 4938/0, clippy clean, CMake 208/208, ctest 68/68; wrapper flagged tree changed during baseline. Focused quick passed 10/10 edges after direct owner hooks/parser tests.
- Explicit untested gaps: exact native host-read capture, native side effects/messages, lifecycle/persistence interception, simulator names/lookup, direct misc holder, zero-call slots. WIP until required surfaces proved.
- Full gate attempt after e5c5360c: cargo 4938/0, clippy clean, ctest 67/68 (ret_channel_check flags two new script return uses). Fixed by invoking setters without consuming uncertain return at script layer; paired public-contract return comparison remains in shadow. Concurrent quick link encountered DLL held by ctest (LNK1168); no code workaround.
- Added check-source counters (dynamic/explicit fixture/serialized sample); no initial-baseline equality credit. Final stable gate next, with no build overlap.
- 1729582e gate: cargo 4938 passed, clippy clean, build 4/4, ctest 68/68 PASS. Added missing known-key registration afterward; rebuild/full gate required for final immutable DLL.
- Final stable c2baaf8a gate PASS: cargo 4938/0, clippy 0 diagnostics, CMake 4/4, ctest 68/68 (73.49s). Logs preserved build-runs/ordinance-final-{cargo,clippy,ctest}.log.
- Immutable DLL build-runs/dlls/ordinance-c2baaf8a/FreeCity4.dll SHA256 E6445DCBEAC88EECAE1C13C579F4C18E091BAC13460735943E297F998BBE40F4.
- Enqueue next: vo4r1-native-c2baaf8a-20261022a (native ordinance + shadow, fixture toggles, full saves April2026/December2031, quit2032).
- Completed vo4r1-native-c2baaf8a-20261022a: clean2032-01-17/ref0; 72 months, 1 bootstrap, 0 resync/fallback. All21 script actions ran, two full saves ok=1. Aggregate 731207/732967 matches, 1760 differences (all20 ordinance serialized records each month and fixture); dynamic729255/729255, fixture368/688, serialized sample1584/3024. No exact claim.
- Next: diagnostic first differing byte/class/size (no semantic fixes), full gate then normal-speed and combined replacement profiles. Serialize mismatch question after diagnostic evidence; required host/effect/reload gaps remain WIP.
- Diagnostic2269d93c full stable gate PASS: cargo4938/0, clippy0 diagnostics, build4/4, ctest68/68 (72.49s). Immutable DLL build-runs/dlls/ordinance-2269d93c/FreeCity4.dll.
- Next queued label: vo4r1-normal-2269d93c-20261022b; same actions at normal speed through2032, no simultaneous request.
- Normal completed clean2032-01-01/ref0,72 months,744378/746138 total matches,1760 differences; dynamic742426/742426, fixture368/688, sample1584/3024; bootstrap1/resync0/fallback0/refusal0. Both saves succeeded, all21 actions ran. Differences persist without turbo.
- Diagnostic: all20 records equal lengths, first_diff=size-68,native17/Rust0A; CPR232bytes offset164, Smoke247bytes offset179. SV3 filed in docs/specs/questions/ordinance.md; literal semantics retained. No further code changes planned.
- Next queued label vo4r1-union-native-2269d93c-20261022c: literal current40 enable list with ordinance disabled for native authority and shadow; then full40 replacement control.
- Combined native run completed2032-01-01, harness clean BUT ref_warnings=1 (FAIL ref0),178173/179933 matches,1760differences; dynamic176221/176221, fixture368/688,sample1584/3024. Warning is last release of native ordinance simulator classA990C08E; preserve log, do not waive.
- Last pending control label vo4r1-current40-2269d93c-20261022d uses all40 replacements and requests shadow (must be INACTIVE, never self-compare).

## Final outcome

WIP; no exact-owner coverage or promotion. Substantive implementation commits:2dedf01b,e5c5360c,1729582e,c2baaf8a,2269d93c. Native remains authoritative whenever shadow is active. SDK dispatch is observed per runtime instance, with QI owner identity; no executable bytes/private state maps or native algorithm forwarding. Bootstrap captures native public Write once into independent Rust objects; bootstrap equality receives no credit. Serialization samples, natural dynamic calls and explicit fixtures have separate denominators. No resync/fallback; all mismatches retained. Shadow host writes are suppressed and counted, never applied twice.

Final stable code gate at2269d93c: cargo4938 passed/0failed, clippy0diagnostics, ctest68/68. Rust files untouched, fmt not applicable. No C ABI export/version/table changes. Diagnostic DLL SHA256 AEA6A17CA2551AC8C37A3D0BDF8CE93A49E5BDF0C38A7D188D1650840E05D5B6; immutable paths and raw logs remain under build-runs.

| Unique label | Outcome | Paired output matches/total |
|---|---|---|
| vo4r1-native-c2baaf8a-20261022a | clean2032-01-17/ref0,72months | 731207/732967;1760differences |
| vo4r1-normal-2269d93c-20261022b | clean2032-01-01/ref0,72months | 744378/746138;1760differences |
| vo4r1-union-native-2269d93c-20261022c | harness clean2032/ref1: FAIL ref0 | 178173/179933;1760differences |
| vo4r1-current40-2269d93c-20261022d | clean2032-01-01/ref0 | shadow INACTIVE because ordinance replaced; no comparison credit |

All runs used the same21 explicit actions: missing-ID failures, unchanged setters, unavailable-on repetition, reavailability, enabled toggles, month boundary, two full saves, digests and full quit. Normal-speed mismatch persistence rules out a turbo-only explanation. Combined native warning: cFC4CrimeSimulator::ReleaseServices lambda line996 released last native ordinance simulator reference; not waived or hidden. Full40 control passed and does not perform Rust self-comparison. No request remains outstanding.

SV3 records concrete serialized lengths/offsets and current literal refresh choice; the question index was not regenerated because tools/ is forbidden. It does not excuse missing implementation. Exact native tuning/date/funds/host read capture, native effects/message comparison, natural Init/Read/Write/Shutdown and Simulate interception, simulator name/lookup calls, direct misc holder, natural setter calls and controlled save/reload2032 are UNTESTED. Serialized samples are not natural save-call verification. Zero-call slots remain untested. Direct native monthly-income getter calls are real (e.g. CPR716 calls in normal run); fixture setters are separately logged4 availability/8on/4enabled. Actual native lifecycle/reload and same-input proof remain required before ready.

Next steps: resolve published persistence contract SV3 without guessing; repair the crime reference ownership using public contracts; implement native input/effect capture and controlled reload evidence. Preserve all four unique results and both immutable DLLs. Do not promote or relabel this partial shadow as exact.
- No game requests outstanding. No ABI changes. No questions yet.
- Manifest: C:/Users/tlori/FreeCity4/build-runs/run4-verify-ordinance4-r1-manifest.json
