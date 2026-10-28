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
