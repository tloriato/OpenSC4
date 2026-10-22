| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| verify-simgrids-shadow4: bounded native grid verifier | `verify-simgrids-shadow4` (engine, detached worktree) | ready: code7e9f580e; cargo4940/0, clippy0, ctest68/68; final-e clean2033/2034/2035 ref0; native8621314/8621314 exact, differences0/refused0/failures0; current40 refused64 lifetimes/4812918 calls; SG2 filed, SG1 open | 1. Integrate bounded opt-in verifier only. 2. Keep SG1/SG2 and unobserved contracts explicit; no promotion. |

## Final report

Ready to integrate the bounded, opt-in verifier. This report grants equality credit only to the observed native pollution-grid calls and the separately constructed cases. It grants no full simgrids replacement, utility, owner, or default-promotion credit. SG1 and SG2 remain open. Original cap3, thisround2; this round audited existing evidence without code changes, rebuilding, restarting, or new game requests.

### Implementation and boundaries

- Base `eaae6c54de3454c6a53205a9403c4d3389133b2a`; substantive implementation `65ca724a`, fixture completion `1d66e53e`, pre-capture initialization `f5902bb7`, final runtime-owner selection `ea1e4837`, final code `7e9f580e`. Detached HEAD retained; no branch/ref updates, merge, or push.
- Added `FC4SimGridsShadow.{cpp,h}` and private `capi/src/simgrids_verify.rs`, adapter observations in `FC4HostGrid.cpp`, opt-in setting/fixture wiring and additive build/module registration. Public ABI entries, ABI version, maps, defaults and test seam tables are unchanged. `fc4_simgrids_verify` is an internal C-linkage static-library bridge, absent from the public ABI table/header; no public DLL export is added.
- Native objects are accessed through public interfaces. Each accepted natural lifetime supplies one initial serialized input; Rust retains and evolves its own record. Bootstrap/roundtrip validation is excluded from comparisons. Later native captures are compared, never installed as expected state. No native fallback or resync.
- Most operations deserialize an independent core `SimGrid`. The final performance correction uses typed `GridElem` reads/writes at validated offsets of the retained record for scalar op1/op5, avoiding whole-grid reconstruction. Scalar counts therefore verify retained typed record values, not a separate full `SimGrid` method invocation on every call. Complete native Write comparisons also cover the retained mutations.
- Natural observation is limited to shared adapter tags `power` and `pollution`. Only pollution was exercised in these traces. The earlier weather follow experiment did not reach this adapter; it earns no weather coverage. Unobserved raw-pointer/structural mutation behavior remains under SG1.

### Gate and formatting

- Baseline attempt was incomplete: workspace cargo and clippy passed, but a delayed release build encountered a temporary new-code placeholder after implementation began. Do not call this a clean baseline.
- Earlier stable gate at `e0522ffd`: cargo4940/0, clippy0 diagnostics, build PASS, ctest68/68. One overlapping retry failed to link the DLL while an older CTest still held it; the stable gate followed that test's completion.
- Final code `7e9f580e`, gate session83656 completed0: quick529/0; full cargo4940/0, clippy with `-D warnings` passed, CMake build PASS, ctest68/68. This round re-read the existing complete test-result lines and build/ctest conclusions. No later source changes, so no repeated baseline, gate or games.
- Logs: `build-runs/qbuild-test.log`, `qbuild-clippy.log`, `fc4verify-quick-test.log`, `fc4verify-build.log`, `fc4verify-ctest.log`. Clippy log includes an environment path-canonicalization warning, with no clippy diagnostics. New module qbuild fmt check repeated: exit0/files1, same environment warning. Recursive lib.rs formatting previously encountered unrelated existing child-module formatting; no mass reformat.

### Final-e complete natural counters

All final labels below have prefix `sgv4-r1-` and suffix `-20261022e`. All three verdicts are CLEAN, foreign_exceptions=0, watchdog_stalls=0, rtc=0, overruns=0, ref_warnings=0. Every `simgrids-shadow` line in all three complete logs was parsed: 176 natural39, 129 union40, 176 roundtrip39. Bootstrap/refusal starts were paired by token and lifetime with every release; no unmatched start/release, unclassified shadow source, bootstrap failure or difference diagnostic remained. Tokens can be reused across lifetimes and were not deduplicated across releases.

- **natural39**, end2033-01-01: all64 `natural-release` reports, owner=pollution, CLSID49B9E605 (int16), **4,312,882/4,312,882 exact**; differences0, refused0, failures0. Calls4,818,173 = get4,312,690 + bulk_read56 + set505,419 + shutdown8; bulk_write0, fill0. Retained-record changed136,639. Bootstrap64, resync0, fallback0.
- **roundtrip39**, end2035-01-01: all64 `natural-release` reports, same owner/type, **4,308,432/4,308,432 exact**; differences0, refused0, failures0. Calls4,812,966 = get4,308,240 + bulk_read56 + set504,662 + shutdown8; bulk_write0, fill0. Changed140,306. Bootstrap64, resync0, fallback0.
- **Native totals, release reports only:** 128 lifetimes, **8,621,314/8,621,314 exact**; differences0, refused0, failures0. Calls9,631,139 = get8,620,930 + bulk_read112 + set1,010,081 + shutdown16; bulk_write0, fill0. Changed276,945; bootstrap128 excluded, resync0, fallback0. Scalar setters contribute operation/change counts, not one equality check per setter.
- Comparison denominator cross-check: get8,620,930 + bulk-data112 + bulk-complete-Write112 + shutdown-return16 + shutdown-complete-Write16 + release-complete-Write128 = **8,621,314**. No zero-denominator result is counted as exactness.
- **All natural-shutdown reports, separately:** natural39 has8 reports, exact1,198,830/1,198,830, calls1,200,067, get1,198,814, bulk_read0, set1,245, shutdown8, changed341. Roundtrip39 has8 reports, exact1,197,978/1,197,978, calls1,199,182, get1,197,962, bulk_read0, set1,212, shutdown8, changed260. Both have differences0/refused0/failures0/bulk_write0/fill0/resync0/fallback0. Together16 reports: exact2,396,808/2,396,808, calls2,399,249, get2,396,776, set2,457, shutdown16, changed601. These are cumulative subsets of the release totals and **are not added again**. Each corresponding release adds exactly one complete-Write comparison with no extra operation call.
- Provenance in each native run: all64 starts and releases are pollution/int16; bootstrap shapes49 at64x64 and15 at128x128. Instances49D5BC40,49D5BC41,49D5BC42,89D5BBF3 each appear in15 lifetimes; instances89D5BBF0,89D5BBF1,89D5BBF2,49D5BC43 each appear once. Shapes/IDs identify observations, not additional algorithm comparisons. Power has no observed natural operations or comparison credit. No other natural owners/types are claimed.

### Current40 refusals and constructed cases

- **union40**, end2034-01-01: replacement enable is confirmed in its log (11 simgrids classes). All64 pollution lifetimes are explicitly refused as replacement factories; all64 release reports have exact0/0, with **4,812,918 unverified adapter calls** in total. These are refused lifetimes, not64 compared operations. No natural-shutdown comparison reports exist in this run; no native comparison, difference or capture-failure denominator exists for its rejected objects. resync0/fallback0; no equality credit.
- union40 also logs **10 constructed-format refusals**, reason=replacement-factories, comparisons0. Its refused fixtures are separate from64 natural refused lifetimes and4,812,918 unverified calls; none is an allocation success or exact result.
- In **each** final native run, all ten constructed classes49B9E602 through49B9E60B (u8/i8/u16/i16/u32/i32/u64/i64/f32/f64) report60/60 exact: **600/600 constructed checks per run**, differences0/refused0/failures0;10 allocations and10 final_release=0 results. These are separate from all natural totals. Combined final native fixtures1200/1200, never folded into8,621,314.
- Constructed cases cover Init/Shutdown returns and records, tract sizes8/4/3/8/8, typed signed/fractional/high-bit values, scalar/cell/average reads, geometry/coordinates/bounds/instance metadata, saved-record Read, literal stored fields and consumed bytes, invalid version rejection, AddRef/Release behavior. Each begins with Init+SetTractSize(8) before its first capture. That initial capture is input, not equality credit. Explicit fixture Read replays the same supplied input in native and Rust; it is not natural resynchronization.
- There were no final-e differences to question or to test at normal speed. All final-e games used turbo1; no normal-speed equivalence claim is made.

### Immutable DLL and save/reload controls

Frozen final plugin/PDB: `build-runs/dll-sgv4-r4`. Rehashed DLL SHA256:
`ED12FAE2F0A13F6FDE1A03E4171CD29C7D5CB5EDF264A004C5C0294D17A1E1C1`.
All three verdicts identify that exact DLL path and harness hash `ed12fae2`. The script `build-runs/run-sgv4-final.ps1` freezes the binary before queuing the sequential runs; the39-key profile is unchanged except for adding simgrids in the middle run. Same DLL throughout; no new requests or outstanding request.

- natural39 loads preserved `sgv4-r1-native39-20261022c.sc4`, SHA256 `F03DA3420CD8FABE50BD887890310D2D5577E5C5C109F76E3906EF84E255FB7A`. Script d0 executes full SaveCity ok1 on2032-12-01 (11078ms), then digest on2032-12-02. Its preserved output `sgv4-r1-natural39-20261022e.sc4` hashes `693951FC3CDD742BB9F9DC229C89A0ED8FF8E23CCB1D65F88A4C5A47C8D01614`.
- union40's harness fixture receipt matches exactly that natural39 output hash. Script d1 full SaveCity ok1 on2033-12-01 (11063ms), digest2033-12-02. Preserved output `sgv4-r1-union40-20261022e.sc4` hashes `DE418AC20118B8B1ADBC1FF0CFF138BD83DAEC3A38D348285695E9C783398DAC`.
- roundtrip39's fixture receipt matches exactly that union40 output hash. Script d2 full SaveCity ok1 on2034-12-01 (11047ms), digest2034-12-02. It ends2035-01-01 CLEAN/ref0. No final-round output copy or subsequent reload is claimed.
- Preserved saves are under main `build-runs/fixtures/`; rehashed this round, not inferred from the reused slot's current city. All three full-save and digest actions were verified in the complete logs. The final-e chain is native39/current40/native39 saved-city compatibility evidence within this bounded profile, not a universal persistence proof.

Audit provenance: `build-runs/audit-sgv4-final.py`, `audit-sgv4-final.json`, and `audit-<full-label>-simgrids.txt` retain parsed totals, every raw shadow line with original line numbers, lifecycle pairing, controls and hashes. They are ignored local evidence, not game files in Git. Complete source logs/verdicts remain under `build-runs/game-results/`.

- natural39 log SHA256 `C49281752C0C00EF612CF7D49EAEB949E2C67687689EFBD5EBE41C7EAEFC1393`; verdict `299313C06B0D1A6D154280D7257FA0008304C6807116AAD63E7F0AB5B02C3E50`.
- union40 log `08E061831639D6A57B1989B060EE6ACBDEAB540C27F017915EBB59D042AFA310`; verdict `B9D745C6E8393E0F45471020F839D967A32A0DC0310688AE9953010F0CC64368`.
- roundtrip39 log `901370FC371BA3D555100F81595784BDC14D8D36700A038EB1E71E1851B118E9`; verdict `8973538E8339C4CDFC7A5B93053BC4C931272F6019208FBB6EB2AF221B904B05`.

### Earlier adverse and incomplete evidence retained

- `sgv4-r1-native39-20261022a`: **CORRUPT**, foreign_exceptions1, immediately after the original117-grid probe, before any fixture result. No algorithm credit and no executable/dump investigation. Frozen r1 DLL `397C55ADE6EA4698799BF70BC50A18CE544D415128A19FF9F9FC47B249ED9EC5`.
- `sgv4-r1-control39-off-20261022b`: CLEAN2032-01-01/ref0, shadow OFF, full SaveCity completed. This control does not excuse the failed ON run or establish its cause.
- `f5902bb7` establishes Init+SetTractSize before fixture capture. SG2 parks the previously undefined pre-initialization Write contract without claiming a causal diagnosis. Its stable all gate cargo4940/clippy0/buildPASS/ctest68 and quick529 preceded frozen r2 DLL `CAD1864980992DAA5F352043EF456F3B25EC9D0323027DA9FF19E3381C5E8AFB`.
- `sgv4-r1-native39-20261022c`: CLEAN2032/ref0; ten constructed formats600/600, differences0/refused0/failures0,10 final_release0. **ZERO natural calls**; no natural equality credit. Its full save was preserved and hashed above.
- `sgv4-r1-current40-reload-20261022c`: CLEAN2033/ref0 and full save, but the script stopped at an ambiguous source-save filename (two matches). No c native roundtrip was submitted and no c current40 output is claimed as preserved. The exact original source was slot1/Regions/FC4 Test/City - Konradshohe.sc4, later subject to slot reuse. The final-e preserved chain is separate evidence.
- `sgv4-r1-natural39-20261022d`: **STALLED** during initialization, watchdog_stalls1/ref0;8 native bootstrap identities/shapes, no final counters. Natural denominator unknown, no exactness credit, no downstream d requests. Frozen r3 DLL `1DE39C04EB7AEC3A31BD69F71810C556B4B653F3766FD9EE5E9DC4CAFCE7E523`.
- Final code `7e9f580e` removes avoidable verification work: no per-scalar1MiB output clearing and no whole-grid scalar decode. It does not suppress observations, filter counters, introduce native fallback or resync. The earlier corrupt/stalled traces remain failures of those versions, not reclassified as successes.
- Prior context guard stopped the final log audit at docs commit `d9d32c0b`, leaving counters explicitly unread and no queue. This round completed that audit. A local q.py attempt could not start Bash (E_ACCESSDENIED); the audit then ran directly under PowerShell with capped output, without affecting game or build evidence.

### Questions and next steps

- Filed **SG2** in `docs/specs/questions/simgrid.md`: factory/pre-Init/pre-SetTractSize behavior and serialization remain unverified. No new question in this audit round.
- **SG1** remains unchanged and open: raw grid-data structural mutation/lifetime and out-of-range original caller dependencies. Replacement safe out-of-range choices are not validated by this verifier.
- Keep natural bulk_write/fill, other natural formats/owners, shared map utilities/iterators, unsized factory serialization and unobserved contracts outside the evidence claim. No allocation failure, bootstrap, refused replacement, original fallback or0/0 is treated as exact.
- Integrate the opt-in verifier and its bounded evidence only; retain all local frozen binaries/logs/saves and the earlier failures. Any later expansion or promotion needs its own authorization and evidence. This lane is finished at its current scope and cap; do not restart it, merge, push or change defaults/ABI tables.
