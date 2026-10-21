| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| core-audioambience4: pure-Rust cSC4AudioAmbienceManager owner (docs/specs/audio-ambience.md), honest partial | `core-audioambience4` / (engine lane) | ready: commit 9fefaa12 + report; gate cargo 3784/0 (baseline 3756), clippy 0, ctest 63/63; no ABI/CAPI/plugin/CMake change; no game run (core only); questions AA5-AA9 filed | 1. Spec answers AA1 (layer behaviour) and AA2 (lot condition update) before the owner can be complete. 2. A glue lane wires AmbienceHost to the game services and a shadow. |

## Report

What changed
- `engine/core/src/audio/ambience.rs` (new): `AmbienceManager<L>` with typed `AmbienceHost` (framework, `cIGZSndSys`,
  message server, occupant/lot managers, city, view zoom, audio-director listener, prefs, sim clock, freshness
  player) and `AmbienceLayer` (the slots of spec §8). Implements §1 initial state, §2.1 Init (12 subscriptions
  in order, failure returns false), §2.2 city init (tuning, layer creation with the failure log line, pref
  volume, city world corners, attention rect, tick list, init walk with no sounds), §2.3/§2.4 shutdowns,
  §3.1-§3.5 occupant/automaton/crossing sounds (instance id = occupant + sound id, preview skip, tracked set
  and count, alarms), §3.6 alarm clock (integer minute division quirk, one-shot via extended post with fresh
  instance id, on/off with wrap tests, last := t), §4 tuning (default 70.0) and attention rect (clamp, even,
  first-call both rects), §5 OnTick stages with fall-through and a host budget, §6 DoEvent, §7 DoMessage.
- `engine/core/src/audio/ambience_tests.rs` (new): 28 tests incl. spec §10.1-§10.6.
- `engine/core/src/audio/mod.rs`: module registration.

Unsupported (partial owner): layer behaviour (AA1, layers are host objects), lot condition update (AA2,
reported to the host via `condition_update_unspecified`, no sounds posted), RandomIterator order (row-major
choice, AA7).

Questions filed: AA5-AA9 in docs/specs/questions/audio-ambience.md.

Gate: qbuild all PASS: cargo test 3784 passed / 0 failed, clippy 0, fc4verify build PASS, ctest 63/63; fmt 0 diffs.
Game: none (no plugin wiring).
