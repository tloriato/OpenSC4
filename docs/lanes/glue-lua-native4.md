| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| glue-lua-native4 (native Lua ScriptServer wrapper) | `glue-lua-native4` / `glue-lua-native4` (engine lane) | in progress | Implement the published owned native boundary; measure acquisition and lifecycle gaps. |

## Handoff
- Goal: implement only the published owned ScriptServer native boundary behind enable=luascripts.
- Reserved detached worktree reused; no branch/ref changes permitted.
- Published LH1/LH2 read; core CLSID/service/priority roles corrected.
- Added bounded private CAPI lifecycle transport and three-view C++ wrapper; incomplete factory is never registered.
- LH2 uses public CommandServer service acquisition, separate typed Lua-generator interface and borrowed sibling Shutdown.
- Milestone bf59c831: bounded CAPI/native implementation committed before call 30.
- Added enable=luascripts public acquisition traces at six framework phases. LH4 insertion route filed; LH3 withdrawn (6.3.6 already supplies payload offsets/copy behavior).
- Baseline all PASS: cargo 4961/0, clippy 0 diagnostics, build 210/210, ctest 68/68; tree changed during baseline.
- CAPI checks cover failed-init latch, Release at zero, cleanup after unbound primitive, borrowed generator sibling and busy owner refusal.
- fmt PASS using absolute touched paths; relative-path fmt failed because wrapper working directory differs.
- First final all: cargo 4964/0, clippy 0, build 4/4; ctest 67/68 (ret_channel_check: two new unreviewed uses).
- Open now cites published AL Boolean contract; unused original IsServiceRunning diagnostic removed rather than interpreting its return.
- Next: stable final all, then immutable opt-in game trace through 2032 and full save/shutdown; no activation or comparison credit.
- No game requests outstanding. No implementation changes yet.
- Manifest: C:/Users/tlori/FreeCity4/build-runs/run4-glue-lua-native4-r1-manifest.json.
- No tools/, analysis, executable, shared maps/defaults/ABI-table inputs permitted.
