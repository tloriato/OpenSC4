# network-manager: open questions
<!-- Questions about [network-manager.md](../network-manager.md). Append new entries at the END; when one is answered, move it to answered/network-manager.md. Rules: README.md. -->

## From netocc-cities29 (2026-10-26)

- **NM18 — terrain removals beyond §7.6's destruction decisions (formerly filed as NM16):** in `gen-tegel-s0-default`, the shadow
  reports 14 extra original removals around cells (65,69..70), (65..66,77), (80..81,69..70). The original's
  component 53 and every component-11 lot/config/zone/wealth entry agree. The core queries with mask 0x1FFF,
  applies the kind/pipe/subway/0x80000 gates and the specified four-corner altitude test. Which native handler
  or network-tool side effect removes these pieces, and at what point should the comparison capture their
  terrain/candidate inputs? Is the §7.6 query/filter or the ownership of the resulting removals incomplete?
  `build-runs/suite/netocc-cities29-before5/gen-tegel-s0-default.log` reproduces 14 errors, netmgr exact 2/3,
  clean/ref_warnings=0. A late listener registered after city Init checks each held candidate immediately
  after 0x278E6003 / 0x6A3B0AFD. `netocc-cities29-probe1/gen-tegel-s0-default.log` still reports 14 errors;
  every listed extra removal has `placed after handler=0`. This disproves a simple later-in-the-tick removal
  explanation; moving the check alone is not justified. The diagnostic now also captures placement at the
  candidate query. This blocks the 795 terrain-detail occurrences in the supplied census, including Tegel
  and the gen27 Konradshohe/Spandau cases. No mismatch, candidate or tolerance has been removed or relaxed.
  The current boundary probe reproduces Tegel with the shadows-profile simulator settings:
  `C:/Users/tlori/FreeCity4/build-runs/game-results/netocc-cities29-profile5-tegel.log`, clean/ref_warnings=0,
  netocc exact 2/2, netmgr exact 1/3 (16 mismatches). Every reported extra removal was placed at the query
  and absent at the late terrain listener (`placed before/after handler=1/0`). Thus already-removed query
  results and a simple later-tick check do not explain these cases.
  A separate timing case is now proved in `netocc-cities29-broad6-06.log` (gen27 Tegel grow): some pieces
  reported as original-only removals were still placed at the late terrain listener (`1/1`). The comparison
  now checks every candidate, including predicted destructions, at each handler boundary; the next-tick
  path remains the fallback for explicitly selected early-listener mode. NM18 remains about the `1/0` cases.
  With that correction, `netocc-cities29-bound8-grow.log` reports 12 rather than 17 errors, and
  `netocc-cities29-bound8-default.log` reports 10 rather than 16. Both are clean/ref_warnings=0; every
  remaining reported extra removal has `1/0`. These logs are in the main checkout's `build-runs/game-results`.

- **NM19 — Port of Shiganshina's first lot/random divergence (§§8.5, 8.7, 17.3):** the census sample
  `C:/Users/tlori/FreeCity4/build-runs/run7/census3/paradis-port-of-shiganshina.shadows.txt` first differs on
  2041-01-01: lot (1,25), config `4BA69450`, exists only in the original, with variation 1, rotation 3,
  flip 0, usable 1 and current compatibility false; zone/wealth are 0/0 in both. There are also two different
  compatible choices and generator next-word indices 386/383. Component 53 and all zone/wealth cells agree.
  After the avenue hash-order/bounds fix, which message or direct entry should first affect this edge lot,
  and which inputs must be retained before the native handler changes them? In particular, does the specified
  replay need transient forced/locked lot state not represented by component 11, or another native draw path?
  Do not infer an answer from the later incompatible configuration alone. The new diagnostic reports the
  first random disagreement's message and four data words without resyncing the generator or hiding errors.
  Fresh attempts `netocc-cities29-probe4-paradis` (corrupt during load) and `netocc-cities29-profile5-paradis`
  (stalled at the script's 2040-12-12 terrain action) cannot reach the census's first differing month;
  the latter starts with an exact netmgr load sample. This blocks that distinct census-only case, not a
  claim that the avenue fix has passed it. All comparison checks remain enabled.
<!-- from: automata-actions29 (2026-10-26) -->
- **NM20 - section 9; native consumer after generated s2 network actions:** Please identify the
  consumer/interface boundary at `SimCity 4.exe+0x3529C0` (`0x007529C0`), its required object view,
  nullability and ownership contract, and the subsystem that supplies that object. With the fixed
  NM17/AT25 automata boundary, the full house list's `gen27-konradshohe-s2-grow.script` stops at
  **84/216** actions: `aa29-at25-house-s2-b.log` reads null+`0x6C` on 2026-09-01. Removing only
  `automata` produces the **same fault and 84/216** on 2026-08-31 in `aa29-at25-s2-noauto-b.log`;
  that log explicitly confirms automata is off. Both runs use DLL SHA256 prefix `afb59058`.
  Logs and exact requests are in main's `build-runs/game-results/` and `build-runs/game-queue/done/`.
  The last scripted call is line 88, `network 40 51 40 19 5`, returning `ok=0`, followed by tool
  Shutdown. This timing does not establish that placement caused the fault: please route the
  boundary to another spec if necessary. Section 9 specifies the placement sequence but does not
  identify this later consumer. A full-house repeat on the earlier same-source DLL (`6c4bb917`,
  `aa29-at25-house-s2-a.log`) has the same fault at 84/216. In contrast, s1 completes 216/216 and
  quits cleanly (`aa29-at25-house-s1-a.log`). [current choice: leave s2 unaccepted; stop this item
  rather than invent a layout, null fallback or native ownership rule]

<!-- from: network-exact33 (2026-10-28) -->
- **NM21 - exact iteration order of lots, potholes and configuration groups (sections 2.2, 3.2, 4.2):**
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
- **NM22 - manager lookup slots 10/11/12: typed ABI and ownership (§§7.5, 7.9):**
  Please specify the three argument types/order and result channel for `FindPipeOccupant` and
  `FindPipeConnectionOccupant`, including the lookup filters, coordinates, failure/output behavior
  and reference ownership. Section 7.5 currently only forwards their names to static pipe-tool
  helpers; network-tools does not define those helpers. ABI interfaces.tsv gives pop 12 and
  `eax-or-void`, but no argument meaning. For `GetAllIntersectionPieces` (slot 12, pop 16), §7.9
  defines the flood but leaves the four argument types/order, output container and acquired-entry
  ownership unspecified. Please supply those interface facts and one worked lookup/flood example.
  Current implementation retains zero-return stubs with SPEC-GAP comments. This is an implementation
  blocker, not an attribution of an observed rand/save difference: R12's new-pin qcheck is pending
  reference recording. The specified station/water/subway predicates have been implemented separately.
