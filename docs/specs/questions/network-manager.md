# network-manager: open questions
<!-- Questions about [network-manager.md](../network-manager.md). Append new entries at the END; when one is answered, move it to answered/network-manager.md. Rules: README.md. -->

<!-- from: verify-regress4 (2026-10-21) -->
- **NM10 - section 6 (`0xE8BDA2E7` lot state changed), section 8.5, simgrid.md section 7.4:** in game, after a lot
  state message whose grown rect is {x0, z0, x1, z1}, the original never changes the road style variation of the
  first row's inner cells (x0 < x < x1, z = z0), while the spec adds every border cell to `pending` and
  `UpdateRoadsInCell` restyles them (ours moves such cells 5 -> 6 or 4 -> 5 where the original keeps them;
  examples: runs regress-train17/19, rect 14,93..17,97 at 16,93; rect at 20..22,103). Rarely the original restyles
  such a cell later, during another message, as if its key had stayed pending. Does the BoundaryIterator skip
  (or postpone) the first row's inner cells, or does `ProcessUpdateList` stop early? The network lot of such a cell
  then differs too (§8.5: a lot is kept only while compatible with the style). [the whole border, as written;
  the netmgr shadow counts differences at such cells under NM10, not as mismatches. The code has cited "NM10" since
  an earlier lane, but it was never filed: this is the entry.]
<!-- from: verify-regress4 (2026-10-21) -->
- **NM11 - section 8.5, section 9:** in run regress-train19 the shadow saw network lot differences at cells that
  were not (yet) in any lot state message's first row: 2026-04 lot at 20,103 (config 0x4A7F8609) held by the
  original only, with no style difference that month; 2026-05 lot at 16,93 (config 0x4A7F8606) held by ours only,
  one month before the first-row message on 16,93. In regress-train17 the same cells differed only after such a
  message (both under NM10). Which messages, other than the six of §6, create or remove a network lot (for example
  a lot created through `UpdateLot` from a pending key of a neighbouring cell whose road style changed, or from the
  traffic network's own piece updates)? [only the six messages of §6.1 reach the lot core in the shadow; the
  verify-regress4 plugin now logs, for each such lot difference, the network occupant's variation, rotation,
  flip, zone, wealth and both sides' compatibility, so the next run can say which input differs]
  Run vr4-a1 (verify-regress4, turbo, 2026-03-01) repeats both cells with those inputs: 20,103 held by the
  original only, variation 6, rotation 3, usable, config compatible now, zone/wealth 3/3 on both sides; 16,93 held
  by ours only, variation 4 (the original's write), config no longer compatible. So the original ran
  `UpdateRoadsInCell` on both cells (creating one lot, removing the other) at moments when no followed message put
  their keys in ours' `pending`. 2026-02-01 shows the same pattern at 48,83, 50,81 and 51,110 (original only,
  compatible, same zone/wealth). What else adds keys to `pending`?
