| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| glue-render-properties4: native render properties | Codex / `glue-render-properties4` (engine lane, detached HEAD) | in progress | Bind published native CRT services; independent same-process verification; assess public replacement route. |

## Handoff
- Goal: native CRT binding and independent render-properties shadow, replacement only with published insertion/lifetime contract.
- Reserved worktree reused; detached HEAD; no branch/ref changes authorized.
- Baseline full qbuild: cargo and clippy PASS; native build PASS; ctest 68/68 PASS.
- Implemented seven C ABI operations over a mutex-protected Rust owner with borrowed signed-int32 CRT callbacks.
- Implemented validated-image RVA binding, SDK-acquired application reference, 77-entry independent spellings/oracle, byte/NUL/query-order checks and restoring setter probes.
- RP7 filed: no public native factory or application/view installation and ownership boundary. Replacement withheld.
- Quick gate PASS: 521 C API tests, native build 159/159. Added independent C++/Rust ABI test; it passes in full gate now running (session 51976).
- New Rust module fmt PASS; shared lib.rs fmt recurses into pre-existing unrelated advisors.rs diffs; do not reformat those.
- RP6 locale profiles remain unmeasured; no unsupported locale mutation or native construction calls added.
- Stable full gate PASS: cargo 4642/4642, clippy zero diagnostics, build 6/6, ctest 69/69 (91.0s).
- Substantive adapter commit 961fdaeb; C++ ABI fixture/quick gate commit 48237e0d.
- Game grprops4-r1-native-48237e0d clean to2032-01-07 ref0; 385 names,5500 finds exact; 462 restoring setter probes exact; P/U and balanced ref checks pass; shutdown releases retained reference.
- 21 persistent live/default differences filed RP8; no getter-output seeding or resync. RP6 uncontrolled native profile probes recorded, not a locale-profile closure.
- DLL snapshot build-runs/renderprops-dll/FreeCity4.dll SHA256 5053463be47b1b96f77fbd6e57bef8aed62a2d1e7fff74ba1d8e09632faa4346.
- Next: explicit39-key union+renderprops run with same immutable DLL and three scripted probes, then final wip report (RP7/RP8 blockers).
- Guard manifest: C:/Users/tlori/FreeCity4/build-runs/run4-glue-render-properties4-r1-manifest.json.
- No game request outstanding. No ABI changes or questions yet.
