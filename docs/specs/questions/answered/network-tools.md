# network-tools: answered questions

<!-- copied from: glue-nettool-exact6 round 3 (engine lane, 2026-10-24); unmerged source branch -->
- **NT33 - live clearance-ray and blocked-cell interface boundary:** NT31
  requires actual terrain/classifier observations for the city powerline.
  Power-line-tool section 4.1 specifies the ray origin, normalized direction,
  hit-distance test and two zero arguments, but not the callable game
  interface/IID, slot, argument order/layout, return register or ownership of
  the hit output. The permitted SDK and corrected cFC4ISTETerrain declaration
  have no named terrain-ray method. Please publish that ABI and the equivalent
  callable boundary for World::blocked(cell, 0), including failure semantics.
  Is the helper prefix count passed to DeterminePolePositions initially the
  complete straight-step count for a fresh slot-7 placement, or a different
  previously-placed prefix? [the new bounded selection ABI requires explicit
  callbacks and an explicit prefix; its controlled NT31 vector passes, but
  unavailable city reads are not replaced with a no-hit/clear answer and no
  city placement, model, wire or curve coverage is claimed]
  **ANSWERED 2026-10-24 (re-nettool-nt32):** DIFFERS — network-tools §8.3.1.1,
  tests 39–40: renderer cISC43DRender (primary IID 0xE9C622D8) slot 61,
  bool AL, pop20, origin/direction/hit XYZ pointers then two zeros. Hit is
  caller-owned 12-byte value storage. Final blocked probe is lot-manager
  (IID 0xE818ED2B) slot 6 GetLot(x,z,false), borrowed pointer EAX, pop12;
  it runs only after earlier classifier tests leave zero. Missing service
  is unavailable, not clear. +0x74 is the straight prefix, 13 for the fresh
  13-step axis fixture, not a count of committed occupants.

<!-- copied from: glue-nettool-exact6 round 3 (engine lane, 2026-10-24); unmerged source branch -->
- **NT34 - nonflat height-solve fixture after NT30 refresh:** Fresh turbo run
  `nettool-exact6-r3-20261024-7c9f` is clean through 2032-01-16, refs=0,
  with 8/14 exact counted shadows. The four price differences remain
  native/core 110/113, 170/192, 90/98 and 55/61. The new replay inventories
  respectively contain 11/44, 17/68, 9/36 and 11/44 entries/corner observations;
  each has exactly the same pre-refresh solved-entry count and smoothing-cell
  count as its final entry count. No position/terrain neighbour was appended.
  Grading changed 12, 34, 18 and 12 vertices. The sums of absolute retained
  corner versus live native getter-time terrain differences, computed from
  the nine-digit diagnostic text, are approximately 32.997318, 218.549516,
  76.532530 and 63.342398; shared vertices are repeated observations.
  Representative straight-road corner: cell (13,4), corner 2, vertex 659,
  old=retained=network=terrain-satisfier 273.388092, both lookups hit,
  live native terrain=271.987152, run=1, intersection/placed/connection=0,
  solved index 9 before and after, in the first refresh population.
  Please provide an admitted **nonflat constraint/height fixture** separating
  the piece/height solve from NT30 commit refresh: initial terrain, per-piece
  fixed/support constraints, both ordered satisfier solutions, refreshed
  corners and getter-time terrain. Which stage/constraint distinguishes the
  native grading target from this replay target? NT30's flat inventories pass
  but cannot resolve this difference. Full replay traces are in the named
  run's `nettool corner`/`nettool inventory` lines. [retain all four mismatches;
  do not alter targets, suppress repeated corners, fit residuals or substitute
  the native total; powerline and bridge differences remain separate NT31
  integration gaps, with the callable powerline input gap tracked by NT33]
  **ANSWERED 2026-10-24 (re-nettool-nt32):** DIFFERS — network-tools §8.1.3,
  test 41: admitted nonflat constraint-boundary fixtures separate fixed/support
  terrain preference, unsupported grading and network-only corner refresh.
  Ordered network/terrain solutions, writes, refreshed corners and getter-time
  terrain are explicit. At vertex 659 the supplied replay's two solutions
  already disagree with native getter-time terrain before any corner refresh
  could repair it. The supplied observations do not identify a unique native
  constraint; compare pre-solve inputs and terrain writes/readback. No city
  snapshot was recovered and all four city price mismatches remain evidence.

<!-- copied from: glue-nettool-exact6 (engine lane, 2026-10-24); unmerged source branch -->
- **NT32 - retained counted poles after structural release:** NT31 §8.3.1
  requires price 65 after endpoint references are released. Power-line-tool
  §1 says ReleasePoleInfos also structurally clears the records, and §2's
  wrapper calls it before returning. Older network-geometry §11 equates
  `+0x388` with the current pole-vector size. Should the getter consume the
  count retained by DeterminePolePositions rather than overwrite it with
  the now-empty vector size? [literal newest NT31 contract: retain counted
  poles=3 and wire cells=10 through ReleasePoleInfos; controlled recording
  placement returns valid=13/failure=0/price=65; reset still zeros counts]
  **ANSWERED 2026-10-24 (re-nettool-nt32):** MATCHES — network-tools §8.3.1,
  test 38: the getter reads retained counts; structural release clears records,
  not the counts. The next slot-53 placement resets both counts before selection.

<!-- from: core-powerline4 (engine lane, 2026-10-21) -->
- **NT19 - network-geometry §11, the power-line parameter defaults:** only `+0x3A4` has a stated default (20). What
  are the defaults of `+0x3A8` (`0x098B27D0`), `+0x3AC` (`0x4A5F5953`) and `+0x3B0` (`0x0A5F596B`) when the exemplar
  lacks them, and what do `+0x3A4`/`+0x3A8` control? The core uses 0. [missing]
  **ANSWERED 2026-10-22 (re-powerline-completion4):** DIFFERS — network-geometry §11/power-line-tool §§1,4:
  fresh Init defaults uint32 20, float32 7.0, uint32 0, float32 0.0; spacing in drag steps at pivots;
  clearance raises ray endpoints. Re-Init resets only spacing/clearance. Missing cost properties retain
  prior values. Evidence `0x006506AA–0x00650704`, `0x00650A43–0x00650A62`, `0x00650CE8–0x00650D9C`.

- **NT20 - §9 step 4 and §11 NT3, two literal readings:** (a) "When `real`, `AttemptNeighborConnections` is followed
  by `PlaceNeighborConnections`": does `AttemptNeighborConnections` also run in draft mode, and does
  `PlaceNeighborConnections` depend on its result? The core runs Attempt whenever the conditions hold and Place
  after it when real, regardless of the result. (b) In `CheckForReservedTiles`, a zoned (1) or road (3) last cell
  leaves `last` at the previous clear cell, so a line ending on a zoned cell loses that cell (v = last + 1); is that
  the original's behaviour, and what does a `CellIsOKForPlacement` result above 3 do (the core ignores it)? [ambiguous]
  **ANSWERED 2026-10-22 (re-powerline-completion4):** DIFFERS — network-tools §9: Attempt runs in both modes;
  Place requires success and real=true (`0x00652A34–0x00652A46`); gate counts input anchors, not poles.
  Network-geometry §11/power-line-tool §6: zones/roads preserve last except final road after result 0,
  which stops at currentStep-1. Clear sets last; initial last=count-1 explains an all-zone walk retaining
  count. Results above 3 preserve last/failure but replace previous result; first previous is uninitialised.

- **NT21 - power-line-tool.md §§5,9, connection helpers:** prove `0x0064E3E0`'s connection geometry and
  optional wire-vector ownership. Enable/restore `0x00649910` and reference-copy `0x0064B6E0` now have
  instruction-derived contracts in §5.2; these two helper obligations are resolved.
  Reach attributes these pole-occupant helpers to this tool. Observed ordering is specified, numeric
  geometry is not. [not proved]
  **ANSWERED 2026-10-22 (re-powerline-completion4):** DIFFERS — §§5.2,5.3,5.5 specify unsigned
  endpoint ordering, four attachment pairs, parameter defaults/properties, x87 control and cubic
  evaluation, N=truncate(max(minimumCount,length/sampleDistance)), and independent geometry storage.
  Optional wire-vector copying takes AddRefs only at the geometry-building endpoint, leaves the
  input unchanged and transfers their release responsibility to the stored connection list.

- **NT23 - regional Attempt:** full type-5 AttemptNeighborConnections `0x00628BD0` endpoint/direction
  derivation, failure/consent behavior, vector mutations and preview effects need instruction checks.
  NT20 resolves the wrapper pairing only. [not proved]
  **ANSWERED 2026-10-22 (re-powerline-completion4):** MATCHES — §5.4 traces all 1438 B for type 5:
  final-anchor/step direction, candidate border and validation, regional type 4, failure returns
  without failure-code writes, append-only paired vectors and no consent UI or preview creation
  inside Attempt. The wrapper's separate Place call remains governed by NT20.

<!-- from: glue-nettool-shadow5 (engine lane, 2026-10-23) -->
- ~~**NT27 - same-process scripted placement result observation:** specify a public-interface
  observation contract for native solved cells with network types, piece IDs and orientation
  after slot 7, and confirm slots 23/25/26/27 expose the ordered dragged cells and current
  valid/straight/total step counts after real placement, before Shutdown. Section 2 names
  internal offsets but does not define a public solved-record getter or its ownership/layout.
  [compare only public drag/status/cost getters; never invent class-method signatures or
  read undocumented object storage; solved piece/type coverage remains unverified]
  The existing plugin host also leaves bridge types/models and powerline-specific placement
  unwired. The diagnostic replay uses the existing adapter's literal zero underground Init
  values, shipped geometry parameters and zero demolition cost: its differences in these
  paths are recorded, not suppressed or credited as exact whole-owner coverage.~~
  **ANSWERED 2026-10-24 (re-nettool-nt27):** MATCHES — network-tools §2.1/§14 and tests 20/30:
  slot 23 assigns a caller-owned coordinate-vector copy; 25/26/27 return current valid/straight/total
  step counts before Reset/Shutdown. No public solved-record getter exists; coordinates do not
  certify type/piece/orientation or missing bridge/power-line adapters. Original entry recovered
  from glue-nettool-shadow5 because this lane's base predates its filing.

<!-- from: glue-nettool-shadow5 (engine lane, 2026-10-24) -->
- ~~**NT28 - observed public result differences in Konradshohe:** run
  nettool-shadow5-r1-20261024-a is clean through 2032-01-02, but the original road
  (20,4)-(28,12) getter returns 17 dragged cells where the core returns 9 (both
  total=9, straight=0, valid=9). Avenue (4,28)-(16,28) returns 26 cells in a
  different order from the core's 26. What is the ordered slot-23 projection
  relative to geometry's drag cells/extra cells, including width ordering?
  Rail (4,24)-(16,24) and avenue (4,28)-(16,28) return false, failure=1,
  valid=0; the core returns true, failure=0, valid=0. Which failure path yields
  literal 1 (rather than the section 5.3 F000xxxx codes), and what rule is
  missing from the documented validation contract? The lowered-strip bridge
  attempt has native valid=17 versus core 0; powerline has 13 versus 0
  (known unwired adapter paths). [retain all differences, no exact owner credit;
  costs are sampled against post-commit terrain in subsequent source so that
  pre/post terrain phases are not mistaken for an engine cost difference]~~
  **ANSWERED 2026-10-24 (re-nettool-nt27):** DIFFERS — network-tools §3.5/§5.3, tests 21–24:
  slot 23 includes all staircase turn cells, grouped in major-axis steps; minor coordinates
  increase within each step even on reversal. Extra cells are a separate inventory. Slot 14
  is a normalized Boolean, not the internal F000xxxx code: every nonzero reason yields 1.
  A refused nonempty run returns false with default stop-on-failure. Boolean logs cannot identify
  the specific rejecting pass. Original entry recovered from glue-nettool-shadow5.

<!-- from: glue-nettool-shadow5 (engine lane, 2026-10-24) -->
- ~~**NT29 - cost/height replay observations:** corrected-source turbo run
  nettool-shadow5-r1-20261024-b is clean to 2032-01-02. Both cost getters now
  read the live post-commit terrain, yet road (4,4)-(14,4) costs original/core
  110/113, and street (64,8)-(74,8) costs 55/61; a repeat of the first road
  costs 110/110. The two diagonal road runs cost 170/102 and 90/54 (native
  slot 23 has additional cells, NT28). Which retained height/terrain records
  must the post-commit GetCostOfSolution sample, and is the extra-cell
  projection also the cost-cell inventory? [retain earthwork/cell-cost
  differences; no tolerance or compensating cost is applied; bridge and
  powerline costs remain explicitly uncovered by the existing host]~~
  **ANSWERED 2026-10-24 (re-nettool-nt27):** DIFFERS — network-tools §8.1/§8.1.1, tests 25–29:
  price retained solved entries (including appended neighbours), their cached network corners
  refreshed from the network satisfier during commit, and live terrain at stored vertex IDs.
  Count shared corners per entry and round each running sum addition to float32. Neither slot 23
  nor the extra-cell vector is the general pricing inventory. Cell +0x52 excludes intersection
  cells, not already-built cells; placed +0x51 does not suppress the charge. Original entry
  recovered from glue-nettool-shadow5; no guessed per-run height residual or compensating price.

<!-- from: glue-nettool-shadow5 round 2 (engine lane, 2026-10-24) -->
- **NT30 - retained-corner differential fixtures after NT29:** clean same-process
  run nettool-shadow5-r2-20261024-a reaches 2032-01-04 with all fourteen ordered
  footprints exact. Four ordinary-road/street public cost totals still differ:
  road (4,4)-(14,4) 110/113, road (20,4)-(28,12) 170/192, shared-endpoint road
  (14,4)-(18,8) 90/98, street (64,8)-(74,8) 55/61 (original/core). The repeat
  road is 110/110. The replay retains solved cell keys, type/run/intersection
  inputs and final network corners; each getter reads live terrain at each
  retained vertex ID, without re-solving or deduplicating. Unit fixtures cover
  NT29's shared vertices, cached-but-unsolved exclusion and repeated charges.
  Can the spec provide an admitted per-entry/corner fixture for these drags,
  including commit-added neighbours and corner refreshes, to distinguish the
  remaining solve/refresh input differences? NT27 rules out discovering those
  internal inputs through a fabricated public solved-record getter.
  [keep all four differences; no fitted adjustment, terrain resample of the
  retained network solution, tolerance, or owner-verification credit]
  **ANSWERED 2026-10-24 (re-nettool-nt30):** MATCHES — network-tools §6.5/§8.1.2 and tests 31–34
  supply per-entry/corner fixtures for the four footprints and repeat, plus stale solved-corner refresh and
  incident-neighbour admission with a missing network height. The first commit pass refreshes all smoothing
  cells even when already solved; the second appends only placed unsolved incident cells and preserves corners
  on missing network heights. These are admitted controlled fixtures, not recovered city snapshots; preserve
  all four city differences until replay solve/refresh inputs agree. No fitted residual or native getter.

<!-- from: glue-nettool-shadow5 round 2 (engine lane, 2026-10-24) -->
- **NT31 - subclass adapter fixture/resource boundary:** in the same clean
  round-2 run, the generic replay now correctly propagates refusals instead
  of discarding the internal reason: powerline (4,32)-(16,32) has original/core
  ok=1/0, public failure=0/1, valid=13/0, cost=65/0; bridge attempt
  (42,43)-(58,43) has ok=1/0, failure=0/1, valid=17/0, cost=974/0. Ordered
  coordinates and straight/total counts match in both. Existing bridge-type
  enumeration returns zero, and the shadow does not call the separate
  powerline pole/wire host. Which minimum admitted resource/host fixture can
  connect those subclass placement and retained-price paths for this city,
  including the powerline tuning and bridge selection/model inputs, while
  keeping the comparison record-only? The public boundary in NT27/NT29 is
  understood; this requests concrete subclass replay inputs rather than a
  nonexistent public piece getter.
  [leave the generic-adapter differences visible and default-off; do not
  substitute native totals, claim pole/wire/model coverage, or guess signatures]
  **ANSWERED 2026-10-24 (re-nettool-nt30):** DIFFERS — network-tools §8.3 and tests 35–37
  specify shipped Utilities tuning (10,7,15,2), direction-mask pole resources and recording-host requirements;
  road bridge enumeration, ordered minimum spans, preferred-index selection, model/height input inventory
  and independent retained-price vectors. Generic zero resources do not replay these owners. Power-line
  pricing multiplies in x87 without an intervening float32 store (qualifying the older r24 shorthand).
  Actual city bridge span/model and pole inventory are not inferable from public totals: no injected 974/65,
  guessed model choice, or owner coverage credit; city differences remain visible and default-off.

<!-- from: glue-nettool-close7 (engine lane, 2026-10-24) -->
- ~~**NT35 - live piece/plane/support inputs beyond NT34:** the NT34 nonflat A/B/C
  solve/grading/refresh fixtures pass. Applying network-geometry §9.1's SGI bucket/chain
  iteration, including resize-before-duplicate and retained bucket count on shutdown,
  changes the shared-endpoint city replay cost from 98 to 93 (native 90). Clean turbo
  run nettool-close7-r2-20261024-b812 still has four native/core prices 110/113,
  170/192, 90/93 and 55/61; ordered drag cells and status agree. What admitted
  callable observation or fully specified live piece/plane/support input fixture lets
  the adapter obtain the native retained solved-entry inventory and corner constraints
  for these probes, beyond the controlled NT30/NT34 boundary vectors? Public totals
  cannot identify those inputs. [keep literal solving and pricing; no residual fitting,
  tolerance, native-total substitution or exact city-cost credit; replacement default-off]~~
  **ANSWERED 2026-10-24 (re-nettools-ng9):** MATCHES — network-tools §8.1.4.
  No public tool getter extracts that inventory or those constraints. Reconstruct from public
  environment observations and resources, or use labelled constraint-boundary fixtures;
  the measured totals cannot specify a unique native live fixture. Three boundary vectors
  distinguish coordinate observation, supplied constraints and unavailable reconstructed cost.

<!-- from: glue-nettool-close7 (engine lane, 2026-10-24) -->
- ~~**NT36 - occupied-cell and subclass/model host integration boundary:** in the same
  clean 2032 run the NT33 live renderer/lot/terrain/zoning/tuning adapter performs
  10 rays and 40 lot reads with straight prefix 13, but reports selection complete=0.
  Its bounded ABI refuses occupied cells rather than classifying/reusing them.
  Native/core powerline valid counts are 13/0 and prices 65/0; bridge valid counts
  are 17/0 and prices 974/0. Beyond NT31 recording fixtures and NT33 observation
  calls, what admitted callable interfaces, layouts and ownership contract should
  connect occupied-cell classification/reuse and the real subclass placement/model
  hosts? The bridge model host is still unwired. [incomplete remains unavailable,
  never clear/no-hit or successful placement; retain both measured differences and
  default-off replacement; selection/resource fixtures do not confer owner coverage]~~
  **ANSWERED 2026-10-24 (re-nettools-ng9):** DIFFERS — network-tools §8.3.3.
  Occupancy is not a refusal: classify the ordered manager list, preserve the last qualifying
  assignment and select the last accepted pole/wire QI with independent reference ownership.
  Slot 40 takes a null filter and unlimited count, not a flags mask. Seven vectors cover order,
  capability, pole-cell filtering, reuse and bridge failure. Class/model creation remains a separate
  host responsibility; original private constructors and slot-54 cell records are not a general
  model callback ABI. Supply the specified model/corner/connection effects or report unavailable;
  retain the measured 13/0,65/0 and 17/0,974/0 differences without city or owner coverage credit.

<!-- copied from: nettool-cost27 (engine lane, commit 464c18187); NT36 title collision retained -->
- ~~**NT36 - Tempelhof single-cell road draft costs 11 versus 10:** please give an
  admitted nonflat single-cell fixture identifying the retained network corners
  (and any additional solved entries) that produce the native price, including
  the plane/range and equality-propagation inputs needed to reconstruct them.
  Reproduced in `nettool-cost27-tempelhof-diag1`, Berlin/Tempelhof, unfounded,
  date 2000-01-01, shadow julian=0. Road `(86,3)-(86,3)` alone differs:
  original/ours cost=11/10; return=1/1, failure=0/0, valid=1/1, straight=1/1,
  total=1/1, cells=1/1. City is 128 square, vertex stride 129. Our live corner
  IDs are `(473,602,603,474)`, terrain float32 values in that order
  `(291.402679,293.308929,290.792847,289.532623)`; water=false, water depths
  `(-41.4026794,-43.3089294,-40.7928467,-39.5326233)`, height range / 16 =
  0.236019135, nearest city edge distance=3, occupied=false, zone=0.
  Our sole retained run cell has flags=1, intersection=false and corners
  `(290.792847,290.792847,290.792847,290.792847)`. Its absolute corner deltas
  sum to 4.386138916015625, so section 8.1 with road cell cost 10 and earthwork
  factor float32 0.1 correctly prices these *replay* corners at 10.
  Network-geometry section 9.8's per-run plane is currently left to the
  replay's host; section 9.1 specifies an initial propagation pass and hash
  picks, but the native public getters cannot establish this cell's retained
  corners/constraints (section 8.1.4). Does the specified plane account for
  this case, and what ordered ranges/preferences/equalities and network result
  should this fixture have? Please specify equality propagation/order if it
  affects the first hash pick; no native private state was inspected and no
  alternative height or additive charge has been inferred from the total.
  Diagnostic run verdict clean, ref_warnings=0; network exact 0/1 (one of 12
  vectors), powerline exact 1/1 (six vectors). [blocked; no price adjustment]~~
  **ANSWERED 2026-10-25 (re-nt36):** DIFFERS — network-tools §8.1.5 and
  network-geometry §§9.1/9.8.1 specify the complete isolated-cell fixture.
  Level 0 fails its boundary-fixed flat equality; level 1 frees the run vertices.
  The stored float32 plane is `(0,256,-25.33203125,-73144)`; initial equality
  propagation uses ascending-id work sets until quiescent, giving all four
  allowed ranges `[257.0517578125,325.468505859375]` while preserving individual
  initial heights/preferences. With the admitted fresh 193-bucket satisfier,
  vertex 602 wins the first hash tie; all retained corners become
  293.308929443359375. One run entry, no extra entries: earthwork sum
  8.198638916015625, factor float32 0.1, one nearest-even result 1, road 10,
  draft price 11. Singleton propagation really appends pending pairs; the old
  unused-stack claim is corrected. Flat and gentle single cells remain 10;
  the supplied gentle eleven-cell straight road remains 110. These are derived
  fixtures with explicit environment/hash inputs, not private city snapshots
  or an authorization to fit other native totals. The older occupied-cell NT36
  above is a separate filing and keeps its answer.

<!-- from: network-exact33 R11 (engine lane, 2026-10-28) -->
- **NT38 - underground refit's solved-cell order and missing depth-property defaults (§§2, 11):**
  Please specify the solved-cell set's hash, buckets, insertion/rehash and iteration order used by
  step 4, including a colliding-cell example. Also specify the four underground properties' defaults
  and whether ReloadNetworkSpecificInfo uses the pipe/subway instances listed in network-geometry
  §2.4. The replacement now seeds the geometry from the refit inputs, with the specified ranges and
  depth target, and invokes the live tool instead of its old no-op callback. It preserves first-input
  order where the hash contract is absent, uses that per-type exemplar table, and keeps zero for absent
  depth/cover values. These are explicit implementation gaps, not inferred native map parameters;
  no qcheck difference has yet been attributed to either choice.

**ANSWERED 2026-10-28 (re-blockers37b):** DIFFERS — network-tools.md §§2, 11.2 specifies Init's pipe/subway exemplar selection, retain-on-missing reads with no constructor-provided first-use numeric default, solved-set B=193 construction, retained capacity on clear, explicit pre-insert resizing, full-u32 identity hash, collision/rehash/traversal order and derived collision examples. No run difference is attributed to these choices.

<!-- from: network-exact33 R5 (engine lane, 2026-10-28) -->
- **NT37 - slot 59 UpdateOccupantHighlights, validation and demolition cost:** sections 5.1/6/8
  name this pass and its cost but do not specify its candidate enumeration, demolition eligibility,
  valid-prefix reduction, or cost accumulation. Please supply those rules and the interface calls
  that observe their inputs, including the surface versus underground overrides. The plugin's
  current host preserves the input valid count and has no demolition-cost observation.
  Original-tool diagnostic `network-exact33-nettool-shadow-r5-1` is clean/ref_warnings=0. On
  gen27-konradshohe-s1-default's first road, 2026-01-30 (57,80)-(25,80), native reports
  ok=0/failure=1/valid=0/cost=0, replay 1/0/33/330, with identical 33 dragged cells. The next
  street succeeds on both sides but costs 4031 versus 168. The Boolean failure does not identify
  the refusing pass; these observations do not establish that slot 59 caused the first refusal.
  Log: build-runs/game-results/network-exact33-nettool-shadow-r5-1.log in this lane worktree.
  [retain the existing pass until specified; do not infer a rejection rule from failure=1]

**ANSWERED 2026-10-28 (re-blockers37b):** DIFFERS — network-tools.md §§8.0–8.0.1 specifies ordered cell/lot-region expansion, layer-filter membership checks and releases, forbidden-zone refusal, retained/cleared output rules, the typed preview-demolition call and its eligibility/int64-price/reference ownership via demolition.md §§4.2–4.4, n/n−1 return and underground no-work overrides. Derived fixtures distinguish region refusal from demolition pricing; no observed refusal/price difference is attributed to this pass.
