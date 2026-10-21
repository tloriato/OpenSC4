| verify-netmgrfix4 | engine | NM10/NM11 pending lot-state walk per network-manager.md 6.2/6.3/8.5 | in progress |

## Handoff
- goal: implement the answered NM10/NM11 lot-state boundary walk (spec 6.2/6.3/8.5, conformance 23/24) in core + shadow, drop the NM10 exclusion in FC4NetMgrShadow.cpp, verify in game (turbo 2032 + normal), ref0/resync0.
- done: core walk dc2a3029 (lots.rs lot_state_walk, tests t23/t24/edges); shadow NM10 exclusion removed + walk cells compared, vector nm16_23 61 keys 2ede3e2f. Gate all PASS (3678 tests, clippy 0, ctest 62/62).
- game vnmf4-turbo2032-a: clean ref0; 73/73 lines mismatches=0; per message ours-only/orig-only/differ 0; walk columns north 28091 cells, orig changed 23, ours agreed; 4 resyncs = the lines with clock-seeded config draws (5 draws).
- next: normal-speed run vnmf4-normal-a (end 2026-07-01), then final report + merge-queue json.
