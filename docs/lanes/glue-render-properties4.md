| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| glue-render-properties4: native render properties | Codex / `glue-render-properties4` (engine lane, detached HEAD) | wip: native CRT adapter and independent verification; cargo4642/4642, clippy clean, ctest69/69; three clean ref0 runs; names1001/finds14300/setters1078 exact, resync0; 21 live-value differences RP8; RP7 installation/factory missing; ABI additive | Resolve RP7/RP8 and controlled RP6 profiles before full shadow or replacement credit. |

## Result

Added the native text adapter and same-process comparison, keeping the original installed.
`renderprops` is opt-in and defaults off. No application/view replacement, complete-owner
shadow, replacement/default promotion, hook installation or coverage credit is claimed.
The task's conditional installation step is blocked by RP7; live configuration inputs
needed for an independent full-value comparison remain RP8. This is a wip deliverable.

## Implementation

- `engine/capi/src/render_properties.rs`: seven additive C operations over the existing
  Rust registry, protected by a mutex. Construction requires both borrowed callbacks;
  missing callbacks return null. Names cross as raw counted bytes, getters copy bytes,
  values preserve int32/binary32 bits. Invalid indexed calls are a documented safe adapter
  extension. Callback arguments/results are int32 cdecl; nonzero classification and low
  mapping byte semantics come from the core. No Unicode/locale substitution or map cache.
- `engine/capi/include/fc4_render_properties.h`, included additively by `fc4.h`, documents
  the new interface. C ABI exports changed; FC4_ABI_VERSION was not bumped by this lane.
- `compat/FC4Compat.*`: bind only RVAs 0x005EF5A9 and 0x005F0543 after the existing supported
  x86 PE identification (timestamp, size, preferred/loaded base) and committed executable
  main-image page validation. No fabricated CRT exports or plugin-CRT fallback. This lane
  did not independently hash/open the game executable; its spec SHA identifies provenance.
- `FC4RenderProperties.*`: acquire cISC4App via SDK QueryInterface, AddRef the borrowed
  properties view before releasing the app interface, copy borrowed names while live,
  and release exactly that retained reference at PreAppShutdown. Executable callbacks
  are borrowed and never freed. Calls stay on the invoking game thread.
- `FC4Script.cpp`: existing `mark renderprops-check` action invokes the opt-in probe.
  The three dates in `build-runs/renderprops-actions.txt` exercise the owner in city;
  checks also run at PostAppInit and PreAppShutdown. No UI or Documents fixture writes.

The independent C++ fixture has all77 mixed-case source spellings from spec section3.
It compares native and Rust stored name bytes against direct scalar native-CRT folding,
and computes a separate first-entry/full-before-short lookup oracle. Queries include
reverse registration order, collisions, leading k, empty/leading NUL, trailing spaces,
embedded NUL with high-byte tails, 0xFF before NUL, UTF-8-looking bytes, I/i, and all128
high bytes both alone and following RS. Neither owner supplies the other's oracle/state.

Setter probes independently send 0/1, int32 extremes, negative zero and -37.25 to both
owners at every valid index, check exact bits, then restore each owner's own saved value.
No original getter output initializes/resynchronizes Rust. Native P/U QueryInterface,
unsupported output-cell preservation and balanced AddRef/Release checks pass. Those are
native self-consistency checks, not a claimed Rust/native P/U lifetime differential.
The opaque Rust C handle does not expose a native vtable. Fresh native construction,
count-zero Release and deleting-destructor comparisons require RP7's missing factory ABI.

## Gates and committed milestones

- Initial full baseline: cargo/clippy/build PASS, ctest68/68 PASS.
- Adapter and RP7 committed `961fdaeb`; quick gate 521 C API tests PASS, build159/159 PASS.
- C++/Rust ABI fixture committed `48237e0d`: actual callback calling convention, signed
  arguments, first-NUL, nonzero classification, low-byte results, name buffer lengths,
  invalid-index rejection, float bit patterns, independent handles/destruction.
- Stable final full qbuild: cargo4642/4642, clippy zero diagnostics, build6/6,
  ctest69/69 (91.0s), all PASS. Logs: `build-runs/qbuild-test.log`, `qbuild-clippy.log`,
  `fc4verify-build.log`, `fc4verify-ctest.log`. Only docs changed after that gate.
- New Rust module quiet fmt PASS. Checking shared lib.rs recursively also reports
  existing unrelated advisors.rs formatting differences; no mass reformat was applied.
  The targeted skip-children check through q.py could not start Bash (E_ACCESSDENIED).
- Gate report `db55bbd3`; native verdict/RP8 `a4f3c337`; union verdict `7fef5225`.

## Immutable game evidence

All three requests used `build-runs/renderprops-dll/FreeCity4.dll`, built from source
commit48237e0d, SHA256 `5053463be47b1b96f77fbd6e57bef8aed62a2d1e7fff74ba1d8e09632faa4346`.
No source changes or rebuilds intervened. Full logs/verdicts are under `build-runs/game-results/`.
Completed unique verdicts were retained; no resend, failed start, or harness error occurred.

- `grprops4-r1-native-48237e0d`: clean2032-01-07, ref_warnings=0, turbo1, 28.688s;
  five probes: names385/385, finds5500/5500, setters462/462 exact, restore_diff0/resync0.
- `grprops4-r1-union39-48237e0d`: clean2032-01-01, ref_warnings=0, turbo1, 118.156s;
  five probes with identical exact counts, all three scripted probes and shutdown present.
- `grprops4-r1-normal-48237e0d`: clean2026-04-01, ref_warnings=0, request turbo0, 3.532s;
  three probes: names231/231, finds3300/3300, setters154/154 exact, restore_diff0/resync0.

The union request spells out39 existing keys plus candidate renderprops (no `all` alias):
`simulator,budget,landvalue,weather,ordinance,demand,power,plumbing,flammability,pollution,protection,crime,aura,flora,landfill,residential,zones,neighbors,development,traffic,tract,lotdev,commercial,industrial,civic,parks,zonedev,history,clock24,demolition,propdev,citydetail,netmgr,netlot,propmgr,lotcfg,occmgr,simgrids,exemplars,renderprops`.
This report preserves the literal tested set rather than inferring enabled-owner credit.

Totals: names1001/1001, finds14300/14300 (separate scalar-oracle differences0), setters1078/1078,
restoration failures0, resync0, retained-reference release and Rust destruction at shutdown
in every process. `build-runs/verify-renderprops-evidence.py` checks all verdicts, probe
counts, differences, lifetime flags and immutable DLL hash; `renderprops-evidence.json`
holds its measured summary. None of these counts is full-value shadow credit.

## Questions and next steps

- RP7: public factory/construction and application/view installation/ownership route
  are absent. No constructor ABI, private pointer writes, class ID, forwarding proxy,
  or guessed insertion was invented. Publish that contract before native owner use.
- RP8: exactly21 live application values differ from section3 defaults in all13 probes,
  with the same native/Rust pairs across candidate, union and normal controls. The
  question lists every pair. Obtain the caller/configuration input order and feed the
  same inputs independently; do not fix constructor defaults or copy original outputs.
- RP6 remains open. Its addendum records native signed-byte scalar observations,
  notably isupper(-48)=1 and tolower(-48)=240. This profile is not the pure C path.
  Accepted CRT locale/codepage, shipped-language profiles and controlled locale changes
  were not measured. No unsupported locale mutator/factory was called to force a result.
- Questions are appended only to `docs/specs/questions/render-properties.md`. The
  generated QUESTIONS.md was not edited and prohibited tools were not run; coordinator
  should regenerate its index. No original question was falsely closed.

No game request remains outstanding. Detached HEAD was preserved; no branch/ref/config,
merge, push, restart, network, external communication or subagent action occurred.
