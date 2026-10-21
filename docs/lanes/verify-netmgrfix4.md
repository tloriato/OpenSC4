| verify-netmgrfix4 | engine | NM10/NM11 lot-state boundary walk per network-manager.md 6.2/6.3 (tests 23/24) in core + netmgr shadow; NM10 exclusion removed | round 3 in progress (NM12/NM13 answered); was blocked on NM12 (gate PASS at 14a7c938; vnmf4-turbo2032-a 73/73 mismatches=0 but 4 resyncs, vnmf4-normal-a 7/7 but 2 resyncs: not zero-resync exact) |

## Handoff (round 3, 2026-10-21)

Goal: exact draw replay per network-manager.md §17.3 + §8.5 replay contract (rules 1-6), zero resyncs, game
runs turbo + normal to 2032 clean/ref0, then full gate and merge-queue file.
Done: merged main (4df3e324, questions file = main's); core PendingSet (SGI hash_set, B=193, resize count+1,
head pushes) + RzRandomSuper::from_raw/raw + lot manager load_random_raw/random_raw/pending_bucket_count/
set_pending_bucket_count, tests t25-t28 (da495dfd); capi exports fc4_netlot_load_random_raw, _random_raw,
_pending_bucket_count, _set_pending_bucket_count (additive, abi=true); shadow: PreCapture in the first listener
before the original for 0x07699455/0xE8BDA2E7/0x89D3E9F3/0xA9B6C2D6 (624 words, left->k, next audit, vtable
0x00AA7958, B, count=0), load before our step, PostCheck after (rng + B equality), pre-diff vs our previous
post-state (unobserved draws, e.g. RedevelopLot), resync disabled, config diffs always mismatches;
vector nm16_23 now in hash order.
Next: qbuild all, then game runs vnmf4r3-turbo2032-a (turbo) and vnmf4r3-normal-a (normal).
Facts: round-1/2 report below.

## Round 2 (2026-10-21): blocked on NM12

Goal: zero resyncs by capturing the original's section 17 configuration draw as a typed external input before our
step (no copying of the chosen configuration). The core already takes the draw through `NetworkLotRandom`
(engine/core/src/network_manager/mod.rs), so only the capture source is missing. The public contract does not
give one: the generator `0x00B44870` is a file-local `cRZRandomSuper` seeded at static initialisation (before the
plugin loads) from `cRZTime()` XOR `0x8C09C379`, advanced also by the section 8.6 object draws; no interface slot
(section 11.2) or message exposes its seed or state, foundations section 6.1 gives no object layout, and the glue
rules allow other subsystems only through the game's interfaces. Filed NM12 (docs/specs/questions/network-manager.md;
QUESTIONS.md index not regenerated: tools/ is off limits to this lane, the coordinator should run
`tools/questions.py index`). No code changed this round, no game runs. The round-1 walk (dc2a3029, 2ede3e2f) stays.
Next: on an NM12 answer that pins a capture point, implement a replaying `NetworkLotRandom` in the shadow, add
input/lifetime tests, then the gate and runs `vnmf4-turbo2032-b` / `vnmf4-normal-b` (unique labels).

## Report (round 1)

**What changed**
- `engine/core/src/network_manager/lots.rs`: new `lot_state_walk(rect)` (exported), the §6.3 iterator: starts at (0, 0),
  step 1, rect clamped `x2 := max(x2, x1)`, `z2 := max(z2, z1)`; row 0 from x = 0 to x1, columns x0/x1 for
  z = 1..z1-1, full bottom row. `add_border_to_pending` (the `0xE8BDA2E7` handler) now adds the walk's cells inside
  the city instead of the rect's border. Commit dc2a3029.
- Tests (`network_manager/tests.rs`): t23 now the spec's 61 keys (and not (10,19), (11,19), not the lot's cells);
  t24 top edge (lot (5,0) -> (0..6,0), (4..6,1)); `lot_state_walk_order_and_edges`: the §6.3 example
  {14,93,17,97} (order, 214 cells, (14,40)/(17,40) in, (15,93)/(16,93) out), one column, rect left of x = 0,
  z1 = 0, z1 = 1, reversed rect. Conformance vector `nm16_23_lot_state_ring.json` follows test 23 (61 keys),
  with streets at (5,0) (styled), (10,19) (top inner, not styled), (9,19), (10,20), (12,23); edge case
  {-1,-1,1,1} -> (0,0),(0,1),(1,0),(1,1).
- `engine/plugin/FC4NetMgrShadow.cpp/.h`: the NM10 exclusion is gone (no `g_nm10Cells`, no NM10 counter or log
  part; every lot/style difference counts as a mismatch). The per-message snapshot of a lot state message now
  covers the grown rect plus the §6.3 walk cells (C++ `LotStateWalk`, same rule), with two new places in the
  shutdown table: "walk row 0", "walk columns north". Commit 2ede3e2f.

**Gate**: `qbuild all` PASS: cargo test 3678 passed / 0 failed, clippy 0, build 184/184, ctest 62/62.

**Game runs** (Konradshohe, netmgr_shadow = 1, order late, no NM10 exclusion):
- `vnmf4-turbo2032-a` (turbo, to 2032-01-03): clean, ref_warnings=0; 73 lines, 73 mismatches=0; per message
  ours-only/orig-only/differ = 0 on every line. Places (cells / orig changed / ours changed where orig kept):
  first row x0 801/30/0, first row inner 719/0/0, first row x1 842/14/0, middle x0 811/64/0, middle x1 831/62/0,
  last row 2213/97/0, walk row 0 0/0/0, walk columns north 28091/23/0 (the original restyled 23 cells north of
  the rect and ours did the same). 5 configuration draws, 4 resyncs.
- `vnmf4-normal-a` (normal speed, to 2026-07-01): clean, ref_warnings=0; 7 lines, 7 mismatches=0; per message all
  0; walk columns north 6212/9/0, first row inner 161/0/0. 2 draws, 2 resyncs.
- 20,103 (old orig-only, 2026-04) and 16,93 (old ours-only, 2026-05): no difference listed at either in either run;
  the 2026-04 and 2026-05 lines are mismatches=0 in both.

**Resyncs are not 0**: every resync line is a line with draws >= 1 (record11=DIFFERS with configs same = lots -
draws, zone/wealth diffs 0, mismatches 0): the lot core's configuration draw differs from the original's
clock-seeded draw (§8.5 [compat], §17), and the shadow restarts the lot core from the original's record. Not
NM10/NM11 related and not a spec gap, so no question filed.

**Questions**: none.

**Next**
- Optional: let the shadow adopt the original's configuration for a drawn lot instead of a whole lot-core resync,
  so a run can show resync=0 (shadow design change, not a behaviour change).
- `walk row 0` had no network tile in Konradshohe; a scripted road along row 0 would exercise that part in game.
