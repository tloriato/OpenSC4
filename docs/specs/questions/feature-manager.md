# feature-manager: open questions
<!-- Questions about [feature-manager.md](../feature-manager.md). Append new entries at the END; when one is answered, move it to answered/feature-manager.md. Rules: README.md. -->

<!-- from: re-featuremgr (2026-10-19) -->
- **FM1 - §6:** is slot 5 `IsFeatureEnabled` or slot 6 `IsExpansionPackInstalled` called anywhere in the shipped
  executable? Two whole-section scans (spec §6) find only slot-7 users: `cSC4Nation::Init` (through the cache
  `0x00B43C98` that `SL::OnAppInit1` fills) and `cSC4Nation::ResetTutorialCities` (through `cISC4App` slot 32). A
  manager pointer kept in memory and used later would escape both. An in-game probe that counts calls of slots 5
  and 6, with return addresses, over one session would settle it. [reachability only: the spec's answers stand
  either way]
  **PARTIAL 2026-10-27:** Slot 7 already calls slot 6 internally (0x0046F7B8), so §6 now narrows its negative claim to independent external uses. Both nation call chains reach that internal query. A further external slot-5/6 use through a retained manager pointer remains unproved; the question stays open.

<!-- from: verify-feature-manager4 (2026-10-22) -->
- **FM5 - §5.1 shipped input size:** the supported 1.1.641 game provider in
  `fm4-r1-query-1022-171537` reports pack 1 key `(0,0x8A5971C5,0x2B563701)`
  acquired, file Length **378 bytes**, FNV-1a `BC8924D4`, whereas §5.1 records
  263 bytes. Is that measurement specific to a data-package variant, or is there
  a published distinction between raw record and adapter file size to document?
  [No size assumption/normalization in Rust: independently capture the actual
  provider bytes once. Native queries exact 13172/13172; no query difference or
  FM1/FM2 reachability evidence inferred from this input-size discrepancy.]
