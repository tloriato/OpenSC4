# automata-managers: open questions
<!-- Questions about [automata-managers.md](../automata-managers.md). Append new entries at the END; when one is answered, move it to answered/automata-managers.md. Rules: README.md. -->

<!-- from: re-comdir (2026-10-21) -->
- **AD1 - §3.2:** the population branches of layers 4, 5, 6, 8, 9 and 10, the recall rule of the per-tile step,
  and the creation of one traffic automaton (slots 79 and 80) in full. [as read]

<!-- from: re-comdir (2026-10-21) -->
- **AD2 - §2.2, §5:** what the base manager's slots 51, 52 and 56 do, and the roles of the vehicle slots marked
  *[summary]* (151, 165, 168–170, 176, 181–189). [as read]

<!-- from: glue-automata-managers4 (2026-10-22) -->
- **AD3 - §§1, 2.1–2.2, 4; automata.md §§7, 11, 15:** Please publish the complete inherited
  `cSC4AutomataManager` owner contract, including constructor/destructor, Init/Shutdown, message subscription
  and unsubscription, tuning/exemplar reload, tracked-group ownership, and the calling signatures/return
  conventions of class slots 38–67. Which concrete classes override each base method, and which base slots
  are abstract? The director's 30-class list publishes factories for vehicle, pedestrian, aircraft and
  watercraft managers, but no separately created base manager or traffic manager. Confirm that inherited
  implementation must be installed through these concrete routes, rather than assigning a new CLSID to
  either base. AD2 covers the missing bodies of slots 51/52/56; this asks for the surrounding complete
  lifecycle and ABI needed to replace the owner without retaining native same-owner methods. The current
  plugin exposes only interface slots 0–37 and stubs CanCreateGroup (slot 30), exemplar selection and
  tracked-group calls; records alone cannot establish a complete base owner. [blocked; no new factory,
  address call, enable key, forwarding adapter, or whole-owner credit]

<!-- from: glue-automata-managers4 (2026-10-22) -->
- **AD4 - §§2.3, 3.2–3.4, 4:** For the inherited `cSC4TrafficAutomataManager` owner, please publish
  constructor/destructor and Init/Shutdown ordering, acquisition/release rules for traffic data, network
  occupants, requests and effect controllers, timer initialization/reset and pause behavior, and reconciled
  calling signatures/returns for slots 68–91. AD1 already requests the missing population/recall/creation
  bodies (including slots 79/80); no duplicate body question is intended. Which complete concrete manager
  route exercises those inherited methods, including class-specific overrides, and which version-gated
  public interfaces supply the traffic counts, view rectangle and network services? The summaries in §4
  and §3.4 do not define a complete lifecycle, and slot numbers plus function addresses alone do not give
  a callable ABI. [blocked pending AD1/AD2 and this lifecycle/ABI contract; no same-owner native delegation]

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

<!-- from: automata-exact33 (2026-10-28, run4 r9) -->
- **AD6 - automata.md sections 4/10; manager PostCityInit and loaded native automata.**
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
