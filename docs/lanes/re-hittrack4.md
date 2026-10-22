| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| re-hittrack4: cHitTrackPlayer behaviour | `re-hittrack4` / RE spec, detached worktree | in progress | 1. Inventory owned bodies. 2. Specify verified contracts and exact gaps. |

## Handoff
- Goal: narrowly scoped cHitTrackPlayer spec; no other owner, engine, tools, or ABI table edits.
- Worktree reserved and detached at coordinator base; reuse HEAD, never switch branches.
- Done: first concrete contract/inventory commit 442a01aa (before call 30).
- Done: 69/110 bodies, 3454/16012 bytes specified: COM, constructor/destructor wrapper, public forwarding, load/pause/play/stop/kill, timer normal path and exception funclet.
- Remaining: 41 bodies / 12558 bytes, partitioned exactly into HT1–HT5 in docs/specs/questions/hit-track-player.md; no complete-owner credit.
- Next: validate inventory/question partition and review byte-derived contracts; finish report and queue docs as partial-owner work.
- First concrete contract commit required by call 30; round cap 100 calls / 150000 context.
- Main read sources: C:/Users/tlori/FreeCity4/{decomp,re,ghidra/export/windows,docs/abi}.
- Guard manifest: C:/Users/tlori/FreeCity4/build-runs/run4-re-hittrack4-r1-manifest.json.
- REA request 0x0093DF4C failed target_unavailable (no app open); used local export/bytes, no session opening or additional tools.
- Key corrections: NoteOff at 0x0093E0FC is callback tail; SetRegisterVal at 0x0093ADBA kills main sound; error is byte +0xC4 latch; volume provider +0x88.
- No game requests or ABI proposals. Git commit succeeded despite nonfatal packed-refs.lock permission diagnostic.
