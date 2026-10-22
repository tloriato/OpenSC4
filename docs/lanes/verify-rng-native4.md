| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| verify-rng-native4: independent native RNG verification | Codex / `verify-rng-native4` (engine lane; detached HEAD) | in progress; binding audit blocked on RN4/RN6; baseline all running; no production changes | Finish baseline record and commit blocked report within 15 calls. |

## Handoff
- Goal: verify production persistent cRZRandom/cRZRandomSuper against an independent native source, only with a published owned binding or proven seeded read-only draw stream.
- Done: reused reserved detached worktree; initial status clean; baseline qbuild all launched; published binding audit complete; RN6 filed (RN4 remains).
- Next: collect baseline all result and finalize blocked report; no native attempt is authorized by the published contracts.
- SDK cISC4AdvisorSystem.h:45 returns opaque intptr_t GetRandom, with comment that the RZRandom cannot be recreated yet. cIGZCOM.h:55 offers generic GetClassObject, but no published RNG class/interface ID, construction, methods or lifetime.
- Exact-owner/method search of public SDK and docs/abi yielded no callable RNG contract (incidental prop timer prose is not one).
- Production rng.rs has persistent RzRandom and RzRandomSuper; seeding.rs process_super shares Rust state across city loads. Neither establishes independent native seed/state provenance.
- No native call, game request, production edit, ABI edit, or coverage claim made.
- Binding absent: append a numbered foundations question and stop blocked within 15 calls; do not fabricate construction or use live simulator reseeding.
- Manifest: C:/Users/tlori/FreeCity4/build-runs/run4-verify-rng-native4-r1-manifest.json.
