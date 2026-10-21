| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| re-netrules-boundary4: answer NR13 (interoperability boundary of the tile rule solver) | `re-netrules-boundary4` / .claude/worktrees/re-netrules-boundary4 | ready, docs only, no ABI change; NR13 answered MATCHES (network-rules.md §2.4, §12, §14) | 1. glue: wire `fc4_network_rules_*` only inside our network tool (FC4NetToolHost `InsertOccupantsForSolvedCells`), no `enable = netrules` key. 2. glue: when our tools replace the original's, own the puzzle-piece control or keep the original `cIntRule` map loaded (§2.4.3 point 2). |

## Report

**What changed** (docs only): docs/specs/network-rules.md new §2.4 "Interoperability boundary (NR13)" (what the
objects are, the complete caller set, the supported hosts), §12 answer entry, §14 strike-through, amendment line;
NR13 moved to docs/specs/questions/answered/network-rules.md (MATCHES); QUESTIONS.md regenerated.

**Findings (byte-backed, re_query.py asm + an image scan of bin/SimCity4.exe):**
- `cSC4TileEvaluator`: 0x10 bytes, `GlobalHeapAlloc(0x10)` in `LoadEvaluator` `0x0063D360` (its only creator),
  ctor `0x00622A90` (one caller), vtable `0x00AA8338` with one slot (deleting dtor `0x00622AE0`), no RTTI; freed by
  `cSC4NetworkTypeInfo::Clear` `0x006421D0` through slot 0. `Evaluate` `0x006220D0` thiscall, pops 8, direct calls only.
- `cSC4TileRule` stack struct (7 dwords), `Read` pops 4; `cSC4TileInfo` = cdecl free functions over static tables.
- `cIntRule` in the image-global map `0x00B452B0`; used by the network tool and `cSC4ViewInputControlNetworkIntxTool`
  (created by `new` in `cSC4View3DWin::InvokeCatalogItemCommand`).
- `ReloadRules` runs from `cSC4NetworkTool::Init` / `cSC4PowerLineTool::StaticInit` under flag `0x00B452A8`, cleared by
  the app static-data cleanup callback `0x006232F0` (app slot 69).
- No 32-bit pointer to any solver function in the image (scan script build-runs/peread.py, not committed).
- Tools are created by `new` in `cSC4NetworkManager::GetNetworkTool`/`Init`; the only GZCOM class whose replacement
  controls them is the network manager `0xC990BD46`. Data override of the rule resources is the other host.

REA not used (export + decomp were enough). No ABI rows proposed. No game runs. No new questions.
