# network-tools: open questions

<!-- Questions about [network-tools.md](../network-tools.md). Append new entries at the END; when one is answered, move it to answered/network-tools.md. Rules: README.md. -->

<!-- from: the cores27 lane, the network tools core (2026-10-20) -->
- **NT6 - §3.3 step 4.1, the single-cell neighbour probe:** the spec says "Look at the four neighbours (offset
  tables `0x00AA8390` / `0x00AA83A0`)" but not the probe order, nor whether a hit stops the scan. Implemented as
  west, east, north, south in that order, all four asked (a hit only decides the axis), then the single cell
  placed through the straight path. Does the original stop at the first hit, and in which order does it ask?
  [west, east, north, south, all four *[inferred]*]
- **NT7 - §3.3 step 5.2, the join record for a same-type piece at both ends:** when the same-type probe extends
  the run at the start and the end in the same direction pass, is the join list built once per call or once per
  direction? Implemented as one `joins` list for the whole call, appended in probe order (start probe first,
  then end probe, per direction pass). [one list *[inferred]*]
- **NT8 - §10 step 3, the closure's iteration order:** the spec says "Repeat while the list grows" but not the
  scan order: does the closure rescan the whole list from the start each round, or only the new tail? Implemented
  as a tail-only walk (`k` advances, new entries append), which keeps the pointer order the removal then uses.
  [tail-only *[inferred]*]
- **NT9 - §10 step 5, the removal order of the closure's additions:** "Remove each occupant, in pointer order
  (`std::set` order)". The caller's list arrives in pointer order; the closure's additions (neighbours,
  intersection structures, paired portals) enter the vector in discovery order, which is not a `std::set`
  insertion. Implemented as: caller's order first, then additions in discovery order. Does the original sort the
  grown list before the removal walk? [caller order, then discovery *[inferred]*]
- **NT10 - §11.1 step 2.3, the refit's cell record edges:** "the type's edge mask (slot 38 with the type)" and
  "recomputed edges" — what the recomputation does is not specified. Implemented as: the record carries the
  occupant's vertex heights and the allowed ranges; the edge mask is stored but the smoothing solve reads only
  the heights and ranges (the geometry core's smoothing uses the run's edges, which a single-cell refit record
  does not carry). Does the original rebuild the edge mask from the occupant's connections before smoothing?
  [edges not needed by the solve *[inferred]*]
- **NT11 - §11.1 step 4, `FindUndergroundOccupant`'s class argument:** the spec names the class "from slot 81"
  but the piece lookup happens per solved cell; whether the original looks up by the cell's own occupant or by
  the class across the whole cell is not stated. Implemented as: per solved cell, the host's
  `find_underground_solved(x, z)` returns the cell's occupant. Is the lookup the cell's occupant (any) or the
  first of the class? [the cell's occupant *[inferred]*]
- **NT12 - §6.1 step 3.1, the crew's `&set` argument:** `crew.AddNetworkSegment(&set, …)` — whether `set` is the
  solved-cell set `+0xA4`, the solved-cell vector `+0x98` or the world-cache records is not stated. Implemented
  as: the host receives the run (cells of the valid steps) and decides. Which container does the original pass?
  [the solved-cell set *[inferred]*]
- **NT13 - §5.1 step 7, the regional retry's failure handling:** after `AttemptNeighborConnections` finds one
  and the solve is retried, a retry failure — does the original keep the first attempt's failure code or
  overwrite it with the retry's? Implemented as: the retry's code overwrites (the same `validate_and_solve`
  path). [the retry's *[inferred]*]
- **NT15 - §14, the interface slot order at 16/17 (ClearOccupantHighlights / ClearCellPreviews):** the SDK
  header says "In the Windows binary, ClearOccupantHighlights is defined before ClearCellPreviews" (its
  note on slots 16/17), while network-geometry.md §1.1's Mac-name note reads the table as
  ClearCellPreviews (16) before ClearOccupantHighlights (17). The compiled table gives both slots pop 0,
  so the replacement's layout is unaffected; cFC4ISC4NetworkToolFull (glue29) declares the SDK's Windows
  order. Which of the two methods sits at slot 16 in the Windows binary? [the SDK's order *[inferred]*]
- **NT16 - §6.2 step 3 (the insert), `AsOccupant` and the reference:** the plain piece's `cISC4Occupant`
  view (slot 3) is folded — it returns `this + 8` (network-occupants.md §4.2), i.e. the same object with
  no AddRef. glue29's insert therefore releases only the `cISC4NetworkOccupant` reference from
  `GetClassObject` (run 218: clean, ref_warnings=0). Is there any caller of slot 3 that receives an
  owned reference (a QI-like AddRef inside the folded body), or is the folded view always borrowed?
  [borrowed *[inferred]* — game code treats the views as one object]
- **NT17 - §3.2, a diagonal PlaceNetworkOccupants with mask 0 (the in-game crash, glue29 run 219):** the spec's
  quirk 1 covers diagonal + mask ≠ 0 (places nothing, returns true). With mask 0 the run goes through §3.2's
  straight path; the passes refuse it (0xF0000001), but in glue29's game run the process then crashed inside
  the game's own code (SimCity 4.exe+0x9F0FA, reading 0x100) while walking the run's pieces — plausibly the
  plain un-modelled pieces the replacement inserts (the piece solver NG5 is the declared follow-up) reaching
  the game's vertex-array draw path. Does the original reject a diagonal mask-0 run before the commit (and
  with which code), or does it also reach the piece/draw path? [refused by the passes *[inferred]*]

<!-- from: core-powerline4 (engine lane, 2026-10-21) -->
- **NT18 - §9 step 2, `DeterminePolePositions` (`0x00650840`) and `PlacePoles(1)` (`0x00651560`):** the spec names
  them but gives no body: how pole cells are chosen along the drag (spacing, corners, reuse of existing poles), what
  the 48-byte pole record holds besides the cell, how `+0x384` (wire cells) is counted, what `PlacePoles`' argument
  means, and where slot 81 `CheckForReservedTiles` is called on this path (it is the only source of the power-line
  codes, and §9 step 4 reads the valid count and the code after step 2). The core keeps them as host hooks. [missing]
  **PARTIALLY RESOLVED 2026-10-22 (re-powerline-completion4):** power-line-tool.md §§4–5 now specifies records,
  spacing/pivots, clearance ray, counting, interception eligibility/flags, off-pivot pending emission,
  cell-index group consumption, zone/blocked/end removal asymmetry, saved endpoint processing and
  append-only repairs, allowRepair and draft undo. Slot 81 is not called inside Determine/PlacePoles.
  **Final-round residual:** source/reachability of the `+0x398` cell-index groups (§6.6). Connection
  geometry NT21 is answered. NT22 retains only the backward split invariant; no group construction
  is attributed to zone update without proof. Keep whole-owner host hooks pending those contracts.

<!-- from: re-powerline-completion4 (RE lane, 2026-10-22) -->
- **NT22 - power-line-tool.md §6, zone repair:** prove UpdateOnZoneChange `0x006522F0` deduplication,
  rectangle-edge/split-cell behavior and query/ref-release order; FindNewPolePosition `0x006520C0`
  endpoint selection when either endpoint is null. Entry guard and outside-clear search are proved;
  remaining pseudocode has invalid stack/register attribution and needs instruction tracing. [not proved]
  **Partial answer 2026-10-22 (re-powerline-completion4):** §6.2 now proves all of FindNewPolePosition:
  ninth dword unused; optional tenth pointer controls mask and geometry; null still creates/reuses one
  pole; nonnull replaces anchors and solves. Remaining UpdateOnZoneChange caller obligations unchanged.
  §6.3 additionally proves wire endpoint-overlap deduplication, skip/search/detach and split-call order.
  **Precise final residual:** the caller invariant allowing backward relocation `(k,count,-1)` at
  `0x00652828` to find an outside clear cell before indexing below zero. The helper has no such guard.
  **Final-round partial answer 2026-10-22 (re-powerline-completion4):** §6.4 proves entry/query,
  signed pole bounds, per-connection relocation arguments and final removal/ref-release order.
  The remaining NT22 obligation is the backward split's data/reachability invariant: none of the
  local guards proves a clear outside cell at or before k. No safe lower bound is inferred.
- **NT24 - power-line-tool.md §9 support inventory:** prove full classifier `0x0064FD00` zone/occupant
  precedence, FindPoleOccupant `0x0064FFE0` candidate filters, wire probe `0x0064FBF0` selection/ref ownership,
  allocator `0x0064A2E0`, and the 13 container helpers' copy/erase ownership. Reconcile brief 49 functions
  against measured 46 entries, both 16156 bytes. [not proved]
  **Partial answer 2026-10-22 (re-powerline-completion4):** §6.1 proves classifier call/overwrite order,
  last-match pole/wire probes and their QI ownership, and raw allocator. Remaining: the 13 container
  helpers and exact source of the 49-versus-46 function-count discrepancy were still open after round 2.
  **Final-round partial answer 2026-10-22 (re-powerline-completion4):** §6.5 specifies all 13
  container helpers / 1530 B, including independent inner buffers, raw descriptor-pointer copies,
  zero AddRef/Release, erase and destruction. The measured inventory remains 46 unique entries /
  16156 B. The sole remaining NT24 item is the exact source/counting basis of the brief's 49;
  no missing functions or sizes are invented to reconcile an unsourced count.

<!-- from: core-powerline-contractfix4 (engine lane, 2026-10-22) -->
- **NT25 - power-line-tool §5.3/§5.5 invalid geometry allocation:** the literal count formula can
  produce N<=0, or a very large count, under invalid resource values. What is the observable
  allocation/failure behavior and retained-reference cleanup in those cases? [unsupported in the
  portable builder: report the raw nonpositive count explicitly; do not clamp N or promote coverage]

<!-- from: core-powerline-contractfix4 (engine lane, 2026-10-22) -->
- **NT26 - power-line-tool §4.1 versus PL-G1:** for a fresh horizontal six-step drag with one
  cell per step and anchors (0,0),(5,0), diagonal-distance equality selects pivots only at the
  anchors. PL-G1 expects an intermediate pole at step 3 for spacing=3. Does PL-G1 assume a
  nonzero previously-placed count making intermediate steps pivots, or is another pivot/spacing
  rule missing? [literal pivot rule implemented; PL-G1 spacing vector tested with previous=6,
  fresh previous=0 remains endpoint-only; no guessed intermediate pivot]

<!-- from: network-exact33 R5 (engine lane, 2026-10-28) -->
- **NT37 - slot 59 UpdateOccupantHighlights, validation and demolition cost:** sections 5.1/6/8
  **Partial answer 2026-10-28 (re-blockers37b):** §8.0 specifies outer ordered cell enumeration, the helper's
  inputs/output writes and n/n−1 result. Helper eligibility/cost/ref order and subclass overrides remain.
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

<!-- NT38 answered in §§2, 11.2; moved to answered/network-tools.md (2026-10-28). -->
