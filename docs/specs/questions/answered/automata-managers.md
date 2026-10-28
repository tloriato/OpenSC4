# automata-managers: answered questions

<!-- from: automata-exact33 (2026-10-28) -->
- **AD5 - §3.4; traffic-simulator.md §4.13: native accident calls into a replacement manager.**
  Please publish the full `cSC4TrafficAutomataManager::CreateAccident` and
  `cSC4TrafficEffectManager::CreateAccident` contracts: ordered argument types and pointer views,
  return convention, admission/failure rules, effect/controller ownership, RNG draws, timers,
  and the version-gated boundary needed when the caller is the original traffic simulator.
  `docs/abi/functions.tsv` identifies entries `00554400` and `00554040`; the former has
  `argument-sources-disagree`, and neither the interface slots nor §3.4 supplies this contract.
  Direct run `automata-exact33-east-baseline-2` (East Weburg, fixed schedule, seed `0x5EED1234`,
  automata+simulator replacements) faults at `0055404B` on the first monthly traffic delivery,
  reading `160F13E0`, while our vehicle manager is `160F0928` (difference `0xAB8`).
  The stack includes original traffic return addresses `00718783`/`00718AF9` and our
  `cFC4Simulator::HostDeliver`; `ref_warnings=0`. This initial reproduction used default
  deterministic pacing; it is diagnostic, not an exactness check. Existing AD4 covers the
  effect lifecycle generally; this question isolates the concrete native entry boundary.
  With original pacing, control `automata-exact33-east-original-1` finishes cleanly;
  replacement `automata-exact33-east-replace-3` instead reproduces the earlier fixed-schedule
  failure in `FC4Schedule.cpp::PoolTaken` on 2112-08-01. These are distinct failure paths.
  No native same-owner forwarding or guessed private-layout storage was introduced.

**ANSWERED 2026-10-28 (re-blockers37a):** DIFFERS — concrete thiscall `(occupantView, s32 radius, u32 durationMs)`, ret 12, borrowed effect token in EAX; direct native call bypasses the manager vtable and requires a supported concrete boundary. Ordered creation, retention, controller call and shared expiry timer are in automata-managers §3.4.1 and traffic-simulator §4.13.2.

<!-- from: automata-exact33 (2026-10-28, run4 r9) -->
- ~~**AD6 - automata.md sections 4/10; manager PostCityInit and loaded native automata.**~~
  Please publish the ordered base/traffic/watercraft manager actions on `PostCityInit`,
  including the calls made on automata retained by Read, their argument/return ABI,
  ownership, placement and callback/model initialization. Section 4 specifies the
  source-count recount, but not the loaded-automaton restoration path; AD3/AT23 ask
  for the surrounding lifecycle. Which public interfaces support that restoration
  when the manager is replaced but the retained concrete automata remain native?
  In qcheck-16, Braus and Jinae load all four replacement managers successfully,
  advance, then fault during the terminal full save in original code at
  `SimCity 4.exe+0x50D03F`, writing `0x00AA41DC`. Braus evidence:
  `compare-20261028-063556-5c19c930/result/game.log`, lines 5292-5333;
  Jinae: `compare-20261028-063628-746bb98f/result/game.log`, lines 6974-7006.
  The same committed source/DLL with `--replace none` is exact on both scenarios
  (qcheck-17, 8/9 exact overall; Stohess has an unrelated original-only record diff).
  This does not establish the cause of the save fault. It isolates a replacement-only
  failure while the current bridge retains loaded objects without restoring them.
  No native owner forwarding, guessed layout, or schedule modification was used.

**ANSWERED 2026-10-28 (re-blockers37a):** DIFFERS — automata.md §4.2, especially
§§4.2.1–4.2.5, specifies retained-object virtual Init, native model/effect ownership,
conditional RNG, controller-manager output references, native traffic Init's call to
extended automata-manager slot 68 (primary A argument, ret 4, s32 traffic type in EAX),
activation/placement, and the subsequent private path-info/**leader** repair. Init and
the common 38-slot manager interface do not provide an atomic equivalent of that repair.
Native deletion requires the matching native heap header/pool links. The save report
shows an occupant vtable used as an allocator link; it does not identify the earlier
corrupting operation or demonstrate a fix. That causal history remains a runtime
diagnostic task, separate from this answered restoration contract. The earlier partial's
description of slot 148/+0xB0 as a model reference is corrected to a leader automaton.
