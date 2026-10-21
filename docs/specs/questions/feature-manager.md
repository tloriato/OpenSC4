# feature-manager: open questions
<!-- Questions about [feature-manager.md](../feature-manager.md). Append new entries at the END; when one is answered, move it to answered/feature-manager.md. Rules: README.md. -->

<!-- from: re-featuremgr (2026-10-19) -->
- **FM1 - §6:** is slot 5 `IsFeatureEnabled` or slot 6 `IsExpansionPackInstalled` called anywhere in the shipped
  executable? Two whole-section scans (spec §6) find only slot-7 users: `cSC4Nation::Init` (through the cache
  `0x00B43C98` that `SL::OnAppInit1` fills) and `cSC4Nation::ResetTutorialCities` (through `cISC4App` slot 32). A
  manager pointer kept in memory and used later would escape both. An in-game probe that counts calls of slots 5
  and 6, with return addresses, over one session would settle it. [reachability only: the spec's answers stand
  either way]

<!-- from: re-featuremgr (2026-10-19) -->
- **FM2 - §6:** the key `tutorial_registry`'s `add_default_region(n, dir)` writes into `default_regions`, and so
  whether pack 1 reaches `timbuktu` through key 1 of the registry's map (`0x004B79A4`). Belongs with the tutorial
  registry. [reachability only: the spec's answers stand either way]
