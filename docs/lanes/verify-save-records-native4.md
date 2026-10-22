| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| verify-save-records-native4: native save-record verification | `verify-save-records-native4` / engine lane, detached worktree | in progress | 1. Establish documented owned/read-only native record endpoint within 15 calls. 2. Verify only if endpoint and lifecycle are specified. |

## Handoff
- Goal: independently compare original SC4 record reader with production save.rs/stream.rs using identical raw records and identifiers.
- Scope excludes DBPF archive/index/packedsegment and generic persistence manager work.
- Worktree reserved and reused, clean detached HEAD; main.. log empty at start.
- Endpoint audit complete before call 15: typed methods exist but no supported isolated SC4 segment/serializer acquisition and lifecycle contract. SV2 appended to foundations questions; native work blocked.
- Sources: foundations sections 2.1/2.7/3.3-3.5/3.6/17; save-records sections 5/9/10; public cISC4DBSegment, cISC4DBSegmentIStream, cIGZPersistDBSegment and serial-record headers; ABI vtable/interface/direct-call/allocator/layout records.
- Production scope fact: save.rs is the simulator v4 payload (not a generic class-record parser); stream.rs implements production stream traits and portable bytes. No verifier clone added.
- Baseline all gate launched; session 76018; next collect result and finalize blocked report.
- Native endpoint deadline: call 15; no private constructors, live-city mutation, or cRZFile substitution.
- No game request outstanding; no production/plugin edits or verification claims. Do not enqueue native/clock24/HOUSE40 without the missing endpoint.
- Manifest: C:/Users/tlori/FreeCity4/build-runs/run4-verify-save-records-native4-r1-manifest.json.
