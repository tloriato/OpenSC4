# network-manager: open questions
<!-- Questions about [network-manager.md](../network-manager.md). Append new entries at the END; when one is answered, move it to answered/network-manager.md. Rules: README.md. -->

## From netocc-cities29 (2026-10-26)

<!-- from: automata-actions29 (2026-10-26) -->
<!-- from: network-exact33 (2026-10-28) -->
- **NM21 - exact iteration order of lots, potholes and configuration groups (sections 2.2, 3.2, 4.2):**
  **Partial answer 2026-10-28 (re-blockers37b):** §2.2.1 proves the two saved maps' u16 identity hash,
  collision prepend/overwrite, prime resize and rehash traversal, serialized iteration and ordinary Read
  insertion. Initial bucket requests, configuration-group map rules and an actual resize/save fixture remain.
  Please specify each map's hash function, initial bucket count, resize trigger/prime selection, collision
  insertion and overwrite order, rehash traversal, and whether Read uses ordinary insertion. Section 8.5
  gives these rules for `pending`, but does not establish that the three maps use the same parameters.
  A worked component-11 save with colliding cell keys before/after a resize would fix the reference order.
  The core currently uses insertion-order `OrderedMap` for lots/potholes and first-loaded configuration
  groups. The coordinator's pre-round netlot check on timbuktu-rush-hour first shifts object ID 16
  (class 6A26D049); this round's qcheck-2 has no fresh baseline, so map order is an unproven candidate,
  not an attribution of that difference. [current choice: retain insertion order; do not borrow the
  pending set's bucket rules for unspecified maps]
  R3 follow-up: after correcting the plugin's section 6.2 message payload dispatch, qcheck-14 on
  timbuktu-rush-hour now differs in only 6 save records (5 only relabelled); the first content
  difference is component 11 itself, E9A6CD3B/299B2D1B/0. Both saves contain 11,775 objects and
  364 lots with the same keys/configuration IDs, no potholes, and a 5,118-byte lot-manager entry.
  The first serialized lot key is 0x7072 in the reference and 0x1E64 in ours; object IDs first
  shift at 1357, class C9C05C6E. No rand() or DIGEST difference. Run:
  C:/Users/tlori/FreeCity4/build-runs/golden-runs/compare-20261028-032628-8e028584/result.
  The original-game shadow network-exact33-netlot-shadow-r3-1 is clean (ref_warnings=0), exact
  5/5 samples by its entry comparison, and reports record11 "same entries (order differs)" after
  city changes. Map serialization order is now a measured remaining difference; the specific
  native hash/bucket contract is still missing, so no implementation parameters were inferred.

<!-- from: network-exact33 R12 (2026-10-28) -->
