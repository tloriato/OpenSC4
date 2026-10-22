# lot-records: open questions
<!-- Questions about [lot-records.md](../lot-records.md). Append new entries at the END; when one is answered, move it to answered/lot-records.md. Rules: README.md. -->

<!-- from: core-lotrecords-final4 (2026-10-22) -->
- **LR6 - section 7.1 GetLotWallEdgeFlags step 4:** when lot manager slot 11 `GetVerticesAtEdgeOfLot(R, 1 << i, V)`
  returns false (its return is not tested), which vertex range does the loop walk: what V holds before the call
  (initialised by the caller, or left from the previous edge), or what slot 11 writes on its false path?
  [the host always fills V and the loop walks whatever it filled]

<!-- from: core-lotrecords-final4 (2026-10-22) -->
- **LR7 - section 7.2 step 3:** when occupant manager slot 33 returns true with `w` null, the update path calls
  `QueryInterface` through null. Is that reachable (does slot 33 ever return true without an occupant)? [treated as a
  host contract violation: the core panics, no null guard is synthesised]

<!-- from: glue-lotrecords-final4 (2026-10-22) -->
- **LR12 - sections 3.2.1/3.2.2, lot slot 46 GetBuildingType(true):** when the stored building
  type is zero, what is the complete configuration-fallback selection contract? Which configuration
  object/ID is selected; how are an absent configuration, empty primary-building list, multiple IDs,
  and a family ID handled; are there any writes, catalogue changes, or RNG calls? The LR10/LR11
  vectors supply this slot's result as a collaborator input, and the configuration spec describes
  the holder/list but not this lot method's selection. [the mutation component takes the result
  from its host; do not forward a native same-owner method on Rust storage or guess the first ID;
  native activation remains parked]

<!-- from: glue-lotrecords-final4 (2026-10-22) -->
- **LR13 - lot slot 61 GetOccupancy, used by lot-developer TestCondition:** what is its complete
  value contract (state gate, stored versus fallback profile, aggregation across capacity/population
  maps, business-unit/travel-designate participation, zero capacities/empty vectors, floating-point
  rounding and side effects)? Foundations section 17.8 and lot-developer section 11.4 publish the
  hidden-result ABI, while LR11 specifies the distinct slot 62 occupancy-by-type operation. No
  rule equating slot 61 to a sum/mean of slot 62 is published. [do not infer that equivalence or
  forward the native method on replacement storage; concrete owner remains parked]

<!-- from: glue-lotrecords-final4 (2026-10-22) -->
- **LR14 - section 3.2.2 step 1, lot slot 50 GetCapacity(devType, false):** which state bytes
  does the false-argument path admit, and does it otherwise use the same first-capacity-map lookup
  and GetBuildingType(true) path as the explicitly specified true-argument path? Slot 52's second
  argument admits state 4, but slot 50's false gate is not specified by that rule. [implement only
  slot 50's true-argument operation in the mutation component; do not infer the false state gate]
