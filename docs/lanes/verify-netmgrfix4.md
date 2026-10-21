| verify-netmgrfix4 | engine | NM12/NM13 exact draw replay: core pending as the SGI hash set (B persistent) + raw MT state load/read (tests 25-28), shadow captures generator + B before the original, replays, checks post-state; resync removed | wip/parked: gate PASS at b547f52c; vnmf4r3-turbo2032-a clean ref0 but 71/73 lines with mismatches: shadow configs lack §8.6 objects, so object draws are missing (orig draws 4-13 more per step) |

## Report (round 3, 2026-10-21): parked, negative evidence

**What changed**
- Merged main (4df3e324; questions file taken from main, NM12 answered there).
- Core (da495dfd): `network_manager/pending.rs` `PendingSet`, the SGI `hash_set<u16>` of the §8.5 replay contract
  (identity hash, B = 193 from the request 100, resize with count+1 before every insert incl. duplicates, rehash
  old buckets ascending each from head with head pushes, head push of new keys, erase never shrinks, iteration
  buckets ascending from head); replaces the ascending-key BTreeSet ([choice] gone). `RzRandomSuper::from_raw/raw`
  ((state[624], k)), `NetworkLotRandom::raw_state`, lot manager `load_random_raw`, `random_raw`,
  `pending_bucket_count`, `set_pending_bucket_count` (refused while keys pending). Tests t25-t28 (§16 25-28,
  incl. the rehash example: 0x1416 resize, buckets 359/368, B kept at 389).
- capi (b547f52c, additive, abi=true, no version bump): `fc4_netlot_load_random_raw`, `fc4_netlot_random_raw`,
  `fc4_netlot_pending_bucket_count`, `fc4_netlot_set_pending_bucket_count`.
- Shadow (b547f52c): first listener (before the original) captures 0x00B44870 words, k = 624 - left (next audited),
  B and count (+0x78/+0x7C/+0x84, vtable 0x00AA7958 audited) for 0x07699455/0xE8BDA2E7/0x89D3E9F3/0xA9B6C2D6; the
  lot core loads them before its step; after it, words/k/B are compared with the original's (check, never copy);
  a capture differing from our previous post-state is reported (rule 4). Resync removed (mgr and lot); a differing
  configuration is always a mismatch. New line part `replay: ...` and a shutdown `replay` total line.
- Vector nm16_23 follows the hash order (pending list, texture events 0x62, 0x60, 0x5F).

**Gate** (b547f52c): qbuild all PASS: cargo test 3902 passed / 0 failed, clippy 0, build PASS, ctest 63/63.

**Game** `vnmf4r3-turbo2032-a` (turbo, Konradshohe, to 2032-01-12): verdict clean, ref_warnings=0; 0 resyncs;
73 lines, only 2 exact. Replay totals: captures 2055, bad 0, B max 389 (rule-5 count always 0, B diffs 0: the
hash-order/B part is exact), but rng post diffs 21 (= pre-diffs 21). In every failing step the original drew
MORE than ours: k orig/ours 322/318, 339/326, 349/341, 354/350, 389/385, 394/390, 401/396, 411/407. Cause: the
shadow adds the configurations with `fc4_netlot_add_configuration_props(g_lot, 0, p, nullptr, 0)`: no §8.6
objects and no object_ids, so our CreateLotObjects makes no prop/flora draws. After the first missed draw a
configuration differs (e.g. lot at 30,108) and, without resync, stays different on later lines.
Normal-speed run not made (the turbo result already fails; not worth a slot until the fix).

**Questions**: none (a shadow host gap, not a spec gap).

**Next**
- Shadow: load the configurations' objects as cFC4NetworkLotManager::ReadConfigurations does (0x88EDC900+k
  properties -> fc4_netlot_object, raw ID lists per token) and give the shadow host an `object_ids` with the
  §B5.6 expansion (cFC4NetworkLotManager.cpp ObjectIds; factor it into a shared helper rather than copying).
- Then reruns with new unique labels (e.g. vnmf4r4-turbo2032-a, vnmf4r4-normal-a), expecting rng diffs 0.
- The avenue flood's local set (lots.rs `work` BTreeSet) may also need the original's order (§8.7); check.

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
