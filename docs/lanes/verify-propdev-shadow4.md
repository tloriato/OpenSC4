| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| verify-propdev-shadow4: prop developer shadow evidence | `verify-propdev-shadow4` / engine lane (detached HEAD) | wip: cargo4938/0, clippy0, ctest68/68; native and literal40 clean2032/ref0; normal clean2026-10/ref0; independent0/50673; PV14/PV15 parked; ABI unchanged | 1. Resolve published capture contracts PV14/PV15. 2. Implement identical-input native interception before any independent operation credit. 3. Add an authorized reload control. |

## Result and scope

WIP, not an independent-operation verification delivery. The requested full native-original-versus-Rust shadow for the replaced developer remains parked. Existing published interfaces and this implementation do not supply an identical typed native host/RNG/callback transcript. No selection, placement, lifetime, resource, natural persistence-operation, secondary-owner or reload exactness is credited. The replacement remains the existing opt-in implementation; this lane hardens the original-authoritative diagnostic shadow.

## Changes

- `6ab0dbbe`: replace the monthly Rust-decoded-record oracle with direct native Write versus running Rust Write byte and reference-identity comparisons. Probe native signs visibility against the isolated Rust query. Preserve byte order, map order, inactive fields and reference identities; no sorting or field masking.
- Eliminate monthly and scanner-divergence resync, plus post-fault state regrowth. One explicit checkpoint bootstrap per native owner is logged and earns no operation credit. Remove the unused second-core comparison helpers and swap path.
- Add cumulative `PropDevEvidence`: every intercepted natural native Idle/Day delivery remains in the denominator, including refusals and failed replay. Log native owner/agent identity, returns, probe attempts/results, failure events, checkpoint bootstraps, shadow fallbacks/resyncs and secondary-listener messages. Current capture is incomplete, so independent comparisons stay zero and every observed native delivery is refused.
- Retain uncertain-block removal diagnostics instead of filtering those blocks. Detail text remains capped by the existing logger, while mismatch totals remain uncapped for the categories actually compared. Creation counts are explicitly unverified and their observed disagreements are preserved below; the legacy diagnostic mismatch field does not include them.
- Add fixture negative controls to the existing plugin prop developer test: matching returns with missing inputs, unavailable replay and mismatching returns cannot become independent exactness. These three synthetic calls are not game coverage.
- `96caf552` and `fbaa0b67`: remove obsolete recovery helpers, retain the fault stop, and document the signs-query AL return from spec section 8.1 at the ABI checker call site. No C ABI exports, shared ABI tables, defaults, maps, generated ABI entries or Rust files changed.

## Gate

All checks used the main quiet wrappers. Baseline `qbuild all`: cargo 4938 passed/0 failed, clippy zero diagnostics, CMake 208/208, CTest 68/68. Source changed during that run, so it is only a baseline. Focused `qbuild quick`: PASS, build 9/9 edges.

The first final-gate attempt had one failure (CTest 67/68): the new native signs query needed a return-channel review comment. Spec section 8.1 specifies AL boolean. After that concrete fix, stable `qbuild all` on `fbaa0b67dcdbbc95490376582c109efbc37e81b4`: cargo 4938/0, clippy zero diagnostics, CMake 7/7, CTest 68/68 PASS. No engine edits followed. Rust fmt is n/a. Logs: `build-runs/qbuild-test.log`, `qbuild-clippy.log`, `fc4verify-build.log`, `fc4verify-ctest.log`.

## Immutable game evidence

Every request used `build-runs/dll-fbaa0b67/FreeCity4.dll`, SHA256 `0925A71397DEB05DBEE86EE0AFEE9D1E932F51B2DB8F4FFCFD07C34686B787C8`. Requests were unique, sequential and never replaced or rerun. The single saved waiter process per request used no qwait. All three completed verdicts are retained under `build-runs/game-results/` with their logs; no requests remain outstanding.

| Label | Profile and date | Verdict | Evidence |
|---|---|---|---|
| pd4-native-fbaa2032-1022a | Native prop developer, default simulator/budget replacements; turbo through2032-01-01 | clean, ref_warnings=0; 71.985s simulation | 45486 native Idle/Day returns observed; independent0/45486, refused45486; serialization72/72, query72/72 diagnostic matches |
| pd4-combined-fbaa2032-1022a | User's exact literal40 enable list; turbo through2032-01-01 | clean, ref_warnings=0; 126.407s simulation | Native shadow INACTIVE because propdev is replaced. Native0 is no equality evidence. Replacement Read1, Write2, Shutdown1 are counters only |
| pd4-native-fbaanormal-1022a | Native developer, same default profile, turbo0 through2026-10-01 | clean, ref_warnings=0; 23.688s simulation | 5187 native Idle/Day returns observed; independent0/5187, refused5187; serialization9/9, query9/9 diagnostic matches |

Turbo native owner/agent: `134648CC` / `134648D0`; normal native owner/agent: `132248CC` / `132248D0`. Class identity was `0x89C48F47`, not a Rust object. Both native runs had failure events0, checkpoint bootstraps1, resyncs0, shadow fallbacks0. Secondary listener messages were30392/4036; these are not intercepted original method operations and get no credit. Shadow fallback0 says nothing about unobserved native factory/resource fallback decisions.

All50673 intercepted native deliveries had matching boolean returns, but **independent exact0/50673, compared0, refused50673**. Native Write/query probes are deliberately requested diagnostics, not natural interception; their81/81 matches each and the two bootstrap captures are not selection/placement/lifetime verification. The full literal40 run has no native denominator and makes no0/0 equality claim. Unknown original method calls outside Idle/Day are unobserved, not silently treated as zero.

## Natural actions, discrepancies and save controls

`build-runs/propdev-actions-96caf552.txt` drove network placement (ok1), zoning (22/24 cells accepted), dezone (24/24), full saves and digests. Native saves both succeeded on2027-01-09 and2031-12-07; combined saves both succeeded on2027-01-01 and2031-12-01. Both through2032 runs reached clean shutdown. Replacement Read1 occurs before Init; its two Write and one Shutdown logs are observed natural replacement calls, not independent native comparisons. Native natural Read/Write/Shutdown were not intercepted.

The72 native monthly reports contain45465 blocks, all45465 raw scanner observations and45465 legacy certain blocks; native/Rust removed counters418/418, not-kept49. Creations are **552/809**, differing in **67/72 months**. Another21 native deliveries after the final month remain in the full45486 denominator. These are block set-difference/count diagnostics, not observed CreateProp/RemoveProp method counts. The zero legacy mismatch field only covers its compared categories and must not erase the observed creation disagreement.

Normal-speed script `build-runs/propdev-normal-fbaa0b67.txt` repeats network/zone/dezone and a successful full save at2026-09-01, followed by the2026-09-02 digest and clean shutdown. Its9 monthly reports contain5187 blocks, removed77/77, creations **166/171**, differing in **2/9 months**. Creation disagreements therefore occur at normal speed too; no turbo-only or lucky-run equality claim is made. Different independent RNG streams and uncaptured host results remain PV14/PV15, not engine changes made to fit the observations.

There is no published reload action in the existing script grammar. No slot save, Documents folder, game bytes or game file was copied or committed; no reload control or persisted-reload exactness is claimed. Scripted actions are real game interface calls; monthly probes and synthetic accounting tests are separately identified.

## Parked questions and next steps

Appended `PV14` and `PV15` to `docs/specs/questions/prop-developer.md`, using the next-free PV14 from the index. The generated index was not edited or regenerated with prohibited tools.

1. PV14: publish an observation-only native RNG interface/transcript contract covering member generator, file-static wealth generator, C rand shuffle, bounds and order. Do not inspect executable data or advance native generators.
2. PV15: publish the typed host/object/resource/tuning/date/lot/callback observation boundary, including mutation results and natural method lifecycle. Existing pre-step live getters, stand-in creations and guessed selector answers cannot earn identical-input credit. Secondary owners remain parked.
3. Implement capture/interposition and isolated replay only when those published contracts support it; add natural requester, sign, prop/lot, persistence and shutdown operation comparisons with full denominators and no duplicate mutations or original-operation forwarding credit. Keep replacement profile native-inactive until a real native authoritative path exists.
4. Provide a supported save/reload control before claiming persisted reload behavior. This lane is queued as **wip**, not ready, despite clean game verdicts and the passing gate.

Operational note: q.py cannot launch Bash here (`Bash/Service/CreateInstance/E_ACCESSDENIED`); bounded PowerShell plus required Python wrappers worked. Commits succeeded on detached HEAD despite nonfatal attempts to lock read-only shared packed refs. No branch switches, RE, subagents, restarts, merges, pushes or external communications occurred. Guard manifest: `C:/Users/tlori/FreeCity4/build-runs/run4-verify-propdev-shadow4-r1-manifest.json`.
