| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| core-glyph-contractfix4 (glyph and string texture contracts) | `Codex` / `core-glyph-contractfix4` (engine lane) | in progress | (1) Correct existing owner; (2) focused and final full gate; (3) queue bounded source review. |

## Handoff
- Goal: apply published portable glyph manager/info contracts to existing Rust owner only.
- Done: read published §§1–3.4/7, answered GT3–GT6 and signed P contract; baseline all launched before edits (logs in build-runs).
- Done: substantive owner corrections: constructor/inherited state, unsigned wrapping refs, separate storage-only destruction, raw renderer output slots, signed numeric paths, identity removal, unique insertion, array clear/release and explicit unsupported paths; existing vectors adjusted.
- Next: inspect baseline/quick gate results; add independent distinguishing vectors and exact body/unsupported checklist; verify formatting; commit stable source then final all.
- Scope: glyph_textures only plus independent tests/conformance; no ABI, game, textures_materials, RE, or shared-table changes.
- Family reservation remains logical round 2/3, this phase max 2 remaining rounds; original parked evidence retained.
- GT1/GT2 remain foreign unknown boundaries. Unsupported invalid-memory/nontermination must stay explicit.
- Baseline command returned session 96574; quick command session 92060 (do not poll; inspect quiet logs). No game requests.
- Manifest: C:/Users/tlori/FreeCity4/build-runs/run4-core-glyph-contractfix4-r1-manifest.json.
