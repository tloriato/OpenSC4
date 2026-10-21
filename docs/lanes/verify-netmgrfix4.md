| verify-netmgrfix4 | engine | NM10/NM11 pending lot-state walk per network-manager.md 6.2/6.3/8.5 | in progress |

## Handoff
- goal: implement the answered NM10/NM11 lot-state boundary walk (spec 6.2/6.3/8.5, conformance 23/24) in core + shadow, drop the NM10 exclusion in FC4NetMgrShadow.cpp, verify in game (turbo 2032 + normal), ref0/resync0.
- done: worktree created from main dfa28678.
- next: read spec 6.2/6.3, 8.5, 16 (tests 23/24); read FC4NetMgrShadow.cpp NM10 code.
