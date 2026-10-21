| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| glue-span4444-4: plugin glue for span render 4444 (nGZDrawSoftRender::cSpanRender4444) | `glue-span4444-4` / (engine lane) | blocked on SP3: no supported interop boundary; no code change; SP3 filed; gate cargo 3676/0, clippy 0, ctest 62/62; no game runs | 1. RE side answers SP3 (a-d) with a reconciled docs/abi vtables row + convention. 2. Then a glue lane adds a compat slot forwarder shadow and a scripted game run. |

## Report

What changed: nothing in engine/. One question filed (SP3) in docs/specs/questions/span-render-4444.md.

Finding: the spec places `cSpanRender4444` inside `cGZDrawContextRenderToBuffer` (created in its Init for colour
type 7, stored at context `+0xB0`). There is no GZCOM interface or CLSID for it, no docs/abi row (vtables.tsv,
interfaces.tsv, unreconciled-rows.tsv: no match), and the spec states argument lists but not the calling
convention, the stack bytes popped or return values of slots 1-5. The plugin compat layer
(engine/plugin/compat/FC4Compat.h) only takes entries from docs/specs or docs/abi, and the task forbids inventing a
detour/signature, so no forwarder, shadow or replacement was wired. No in-game path that creates a colour-type-7
context is documented either, so a shadow would have no known real native inputs to compare.

SP3 asks for: (a) the convention/pop counts/returns of slots 1-5; (b) a reconciled vtables.tsv row and whether the
vtable is shared by all instances; (c) any GZCOM path to the renderer (SDK has `kcGZDrawContextAsBuffer` with no
interface); (d) the shipped code paths and a scripted action that reach slots 1-5 in GOG 1.1.641.

Gate (unchanged code): cargo test 3676 passed / 0 failed, clippy 0 diagnostics, CMake build 184/184, ctest 62/62.
Game runs: none (nothing in the plugin changed; no boundary to shadow).

## Coordinator docs disposition
Queue only public SP3 and report; no engine code changed and no span replacement exists. This first round is blocked pending SP3; retain two remaining implementation rounds across later stages. No replacement/shadow credit.
