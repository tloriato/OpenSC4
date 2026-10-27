# animation-coordinator: open questions
<!-- Questions about [animation-coordinator.md](../animation-coordinator.md). Append new entries at the END; when one is answered, move it to answered/animation-coordinator.md. Rules: README.md. -->

<!-- from: core-animation-coordinator4 (2026-10-22, corrected owner task) -->
- **AC5 - §§1.1–3 geometry:** publish visible-world and power-change rectangle edge conventions,
  power message rectangle units/payload, rounding at exact 256 m boundaries and handling of negative,
  non-finite or empty coordinates and a city smaller than 16 cells. [Core provisionally uses half-open
  block rectangles, floor lower/ceil upper for world overlap, clamps positions, rejects non-finite
  geometry and cannot register on an empty grid. Power messages require already normalized block
  coordinates; no native payload decoder is claimed.]

<!-- from: core-animation-coordinator4 (2026-10-22, corrected owner task) -->
- **AC6 - §§1.1–2 lifetime/references:** which service/filter/subscription/occupant/queried-target
  references does this owner acquire and release, in what order, and does Shutdown stop tick targets or
  clear occupant flag 0x20 before emptying the grid? Are registration sets owning references? Publish
  deleting-destructor/refcount/interface rules. [Portable service, subscription and QI leases release
  only their own acquisition. Occupant IDs are borrowed identities; no native AddRef counts, native
  deletion behavior or additional Shutdown tick removals are invented.]

<!-- from: core-animation-coordinator4 (2026-10-22, corrected owner task) -->
- **AC7 - §§1.1–3 failure/order:** publish Init rollback/retry and repeat-Shutdown behavior; failed
  occupant/target QI and tick add/remove effects/return values; iteration order within/across blocks,
  callback reentry/mutation behavior, and ordering of set membership, target calls and flag changes.
  [Core provisionally commits registration before callbacks, applies manager operation before flag,
  returns explicit errors with unchanged flag on target/manager failure, aborts refresh at first error,
  uses row-major blocks/sorted IDs and leaving-before-entering scrolling. Owned leases roll back on
  failed Init. These choices are regression-tested, not native conformance claims.]

<!-- from: core-animation-coordinator4 (2026-10-22, corrected owner task) -->
- **AC8 - §§1.1–3 triggers/messages:** when is the opaque global byte sampled and when are all blocks
  refreshed after its threshold changes? Is that part of 0x231BBF91, SetOccupantAnimation, or another
  operation? Publish payload/return conventions, budget-target registration rules, and how moving or
  registered occupants are handled when their current block/filter changes. [Core exposes explicit
  byte-refresh, keeps 0x231BBF91 as only the specified all-block refresh, never registers through a
  budget message, and applies Set on/off to the current position's block. Rotation payloads are typed
  old/new values; no native message decoding is claimed.]

<!-- from: core-animation-coordinator4 (2026-10-22, corrected owner task) -->
- **AC9 - full-owner inventory:** the corrected target attributes 45 functions / 7726 bytes to
  cSC4AnimationCoordinator. Please publish its complete method/helper catalogue with per-entry
  attribution and behavior, including entries not individually described in §§1–3. [Only published
  portable operations are implemented; native lifetime, payload, budget and remaining catalogue
  entries prevent whole-owner promotion. No scheduler or timeline requirements are requested.]
