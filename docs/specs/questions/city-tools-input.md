# city-tools-input: open questions
<!-- Questions about [city-tools-input.md](../city-tools-input.md). Append new entries at the END; when one is answered, move it to answered/city-tools-input.md. -->

<!-- from: the re-citytools4 lane (2026-10-21) -->
- **CT4 - §2.1, the base constructor:** not re-read; confirm it zeroes `+0x04..+0x20` and the reference count
  starts at 0. [zeroes all *[inferred]*]

<!-- from: re-query (2026-10-21) -->
- **CT14 - §8.13, §8.12.1, §8.12.4, remaining dialog/writer details:** specify the neighbour row fillers
  `0x104B`–`0x104E`, `0x1066`–`0x106A`, all MySim icon `0x105B` branches, and the complete debug description
  writer content/value sourcing (`DescribeBuilding`, `DescribePopulation`, `DescribeDemand`, `DescribeCell`,
  `WriteDetailedDebugInfo`). Kind priority and all one/two-record resource choices are now §8.16.6; kinds 4/5
  count records rather than look up another window. Writer ordering is §8.16.7. [partially answered 2026-10-23;
  owner closure still open]

<!-- from: re-query (2026-10-21) -->
- **CT15 - §8.15:** route mode: `MakeRoutePath`, `GetBuildingOccupantPaths`, `GetNetworkOccupantPaths`,
  `DoesRouteCrossCell`, `GetRouteTravelTypes` and the legend's travel-type toggles in detail. [outline]

<!-- from: core-query-tool5 (2026-10-23) -->
- **CT18 - §8.12.1, empty-cell sound conflict:** conformance test 10 requires a failure sound for an empty
  non-zone cell although DescribePick's numbered steps omit it; should that sound be emitted? Occupant slots
  `0x1026`–`0x1028` are confirmed null and the six funding-slider related sequences are now §8.16.1, including
  hospital staff's additional IDs `4`, `0x3F800000`. [partially answered 2026-10-23; sound still open]

<!-- from: core-query-tool5 (2026-10-23) -->
- **CT19 - §8.12.4/§8.14, remaining numeric contracts:** verify aura averaging's inclusive endpoints against
  instructions; airport noise enumeration, signed aggregation/clamp; property-to-curve slope construction,
  duplicate/unsorted/empty knots; missing query records, zero commercial capacity, nonfinite/out-of-range
  numeric conversions and overflowing summed budget costs. Best-developer ranking, wealth geometry/constant,
  commute sampling, evaluation of an already constructed finite ordered curve, and the fixed lost-luggage
  resource key are now §8.16.2–§8.16.5. [partially answered 2026-10-23; exceptional values and remaining numeric
  instructions still open]

<!-- from: core-query-tool5 (2026-10-23) -->
- **CT20 - §8.7/§8.10/§8.11.4/§8.12.1, remaining traffic/debug contracts:** specify
  `GetBuildingOccupantTrafficInfo`, `GetNetworkOccupantTrafficInfo`, their trip-volume helpers, network
  debug backing fields/capacity context and complete description-writer bodies (shared with CT14).
  Debug enable's application-byte/keyboard source and the distinction between building-pick bookkeeping
  and subsequent cell writers are now §8.16.7. [partially answered 2026-10-23; traffic tips and writer bodies
  remain Unsupported, with no native owner callbacks]

<!-- from: core-query-tool5b (2026-10-23) -->
- **CT21 - §8.16.1/§8.12.4, funding cost refresh target:** §8.16.1 says the related-ID sequences are in addition
  to a separate `ID + 2` cost update "in §8.12.4", and QCT5-3 requires that separate update. However §8.12.4
  specifies only the helper's `ID + 0x100` money child, and §8.13 also specifies `ID + 0x100` on notification.
  Is `ID + 2` a typo, or another update? If separate, specify which sliders, its amount, lookup parent and
  initialization/notification/write-back timing. The core retains the explicit `ID + 0x100` helper and
  notification behavior and marks the additional update Unsupported rather than inventing cost controls.

<!-- from: core-place-lot5b (2026-10-23) -->
- **CT25 - section 3.7 feedback visibility:** step 1 resets flag +0x95 false and no later step assigns true,
  yet section 3.14 defines the second feedback argument as visibility and says a missing terrain pick suppresses
  feedback while an invalid picked lot can remain visible. Where does a hit assign visibility true (or is +0x95
  not this argument)? [literal section 3.7 retains false; the complete 3.14 renderer is exposed through the core
  feedback method with independent validity and visibility, but mouse-driven visibility is not claimed exact]

<!-- from: core-place-lot5b (2026-10-23) -->
- **CT26 - section 3.7 versus quirk 4.5 validation gate:** section 3.7 says CanPlaceLot still runs for reason
  text after a failed coverage/facing step, whereas quirk 4.5 gates that call by previous placeability when
  automatic facing/configuration lookup fails. Which rule applies, and what are the retained cost/text/position
  outputs when the gate is false? [follow the detailed 3.7 unconditional validation for now; no native callback]

<!-- from: glue-querytool-shadow13 (2026-10-24) -->
- **CT27 - section 8 native observation fixture:** specify a nonmodal deterministic fixture contract for
  replaying a loaded-city cell/occupant as a renderer hit and capturing the native selected target and
  displayed query fields without entering the dialog message pump. What exact renderer hit record layout,
  pixel/cell conversion and native output snapshot API can an observer use? CT13 already covers the callout
  interface, and CT14/CT20 cover writer bodies; this question asks for the fixture seam rather than those
  bodies. [default-off shadow measures only live hover/describe predicates from copied type/property inputs;
  full target, displayed strings/fields and highlight verification are not claimed]
