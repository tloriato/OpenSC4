| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| glue-lua-native4 (native Lua ScriptServer wrapper) | `glue-lua-native4` / `glue-lua-native4` (engine lane) | in progress | Implement the published owned native boundary; measure acquisition and lifecycle gaps. |

## Handoff
- Goal: implement only the published owned ScriptServer native boundary behind enable=luascripts.
- Reserved detached worktree reused; no branch/ref changes permitted.
- Published LH1/LH2 read; core CLSID/service/priority roles corrected.
- Added bounded private CAPI lifecycle transport and three-view C++ wrapper; incomplete factory is never registered.
- LH2 uses public CommandServer service acquisition, separate typed Lua-generator interface and borrowed sibling Shutdown.
- Added enable=luascripts public acquisition traces at six framework phases. LH3 payload ABI and LH4 insertion route filed.
- Next: build/fix focused checks; baseline all still running; native execution/list transport is explicitly incomplete.
- No game requests outstanding. No implementation changes yet.
- Manifest: C:/Users/tlori/FreeCity4/build-runs/run4-glue-lua-native4-r1-manifest.json.
- No tools/, analysis, executable, shared maps/defaults/ABI-table inputs permitted.
