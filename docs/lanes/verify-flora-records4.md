| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| verify-flora-records4 (occupant native verification, final round 3) | `Codex` / `verify-flora-records4` (engine lane, detached HEAD) | in progress: resumed c5fa937b; baseline all running | Add normal 20-case corpus, production Rust queries, supported QI/visibility checks; preserve FQ16/FQ17 exclusions. |

## Handoff

- Goal: final occupant-family round; no fourth continuation, static/whole-owner/default promotion.
- Start c5fa937b; prior round2 report and evidence below retained intact.
- Baseline qbuild all started before changes; logs build-runs/qbuild-*.
- q.py still fails Bash E_ACCESSDENIED; capped PowerShell used.
- Done: substantive Rust query/visibility and 25-case native corpus/QI matrix implemented; first code commit includes this handoff.
- Next: focused quick/fmt, review measured differences, stable final all then immutable-DLL game controls.
- FQ18 added for failed-QI output/null-address policy and multi-bit flag semantics; no guessed equivalence.
- Need final stable all, immutable DLL, unique coordinator guarded2032/save/reload/probe-off runs.
- No game request outstanding; wait without qwait; reuse completed unique verdict.
- FQ16 static callable and FQ17 partial-state/default/bounds/override gaps remain refused.
- Source allowed: occupant-manager section13.1 for inherited methods, flora3.4/9.1/11.4, lot-records2.3/11, public SDK.
- Separate mixed assertions, independent queries, bytes, stream results, QI, references and natural observations.

## Preserved round2 report

## Result: occupant-only implementation and evidence

This round delivers an independent Rust occupant record codec, inherited flag/highlight/removal mutations, atomic counted-reference lifetime model, and an opt-in native factory probe. It advances the independent occupant task without using the missing static-record contract. **WIP, not full owner verification:** remaining inherited/lifecycle/QI cases and corpus limitations are listed below. No coverage map or default changes were made.

- Goal: owned native factory versus independent Rust occupant; full specified queries/mutations/serialization/lifetime; read-only natural snapshots.
- Prior round d5c82b44 parked static-record FQ16; excluded this round, no static getter calls or static coverage credit.
- Start HEAD d5c82b44 detached; no branch/ref changes. Prior report below is historical.
- Baseline all: workspace tests and clippy passed, CMake/ctest 68/68 passed (logs build-runs/qbuild-*, fc4verify-*).
- q.py cannot start Bash (E_ACCESSDENIED); use capped local PowerShell commands.
- Done: a25cef82 independent Rust occupant codec committed before call 30; FQ17 partial-state/override question appended.
- Done: 6f3a7fad typed probe; quick 3452 tests PASS, dependents compiled, build PASS (tree changed; stable gate still required).
- Game vfocc-r2-d1-20261022-1758 clean/ref0: 585 comparisons/0 differences, 340 FQ17 refusals, 2607 natural occupants/15642 read-only queries, invalid0.
- Stable full all on 17369af7: cargo 4941/0, clippy0 diagnostics, CMake4/4, CTest68/68 (242.2s); no tree-changed marker.
- Guard2032 vfocc-r2-guard2032-20261022-a clean/ref0: each probe810/0, 340 refusals,70 undefined returns,0 fallbacks. Script full save ok1 (11484ms), date2031-01-11.
- Immutable DLL build-runs/dlls/vfocc-r2-final/FreeCity4.dll SHA256 daed6fc7e7d83e3ed793787adbf4c20061c5b289220c1743b82ebaad2384a8aa.
- Saved isolated-slot game-data fixture main build-runs/fixtures/vfocc-r2-save-20261022-a.sc4 SHA25682de3ffe2ac3e7b11ec36ce04917203d1afd82ad6d8286a2d42ce1ea396d4390 (never committed).
- Reload vfocc-r2-reload2032-20261022-a clean/ref0 to2032-01-09; fixture hash matched; probes2x810/0, natural2575 then2569/invalid0.
- Control vfocc-r2-control2032-20261022-a clean/ref0 to2032-01-04, same saved fixture and same immutable DLL, probe/natural lines0 as expected.
- Expanded probe compares native release counts with independently allocated Rust atomic reference model and destroys both at zero.
- Probe opt-in flora_occupant_verify=1; enable simulator, keep flora_shadow=0 (avoids static-record getters entirely).
- No game request outstanding. New labels unique; wait without qwait.

## Independent inputs, boundary and accounting

`engine/core/src/flora/occupant.rs` implements flora version3, base version4 via the existing independent base codec, sign/zero byte narrowing, position/dates/type/lot-tag, the base-version rejection quirk, inherited mutations, and atomic references. Five new C ABI exports bridge typed input/encode/decode/mutation/write/reference operations (encode, decode, mutate, write, reference). ABI version and ABI tables were not changed.

`engine/plugin/FC4FloraOccupantProbe.inc` creates owned originals using published CLSID0xA9C05C85 and QIs SDK flora/serializable views. The typed input is statically checked at44 bytes. Every native call uses the published SDK interface and slot contract; there are no address-based calls, native field extraction, binary scans, static-record getters, simulator RNG changes, or copied native outputs used as computed expectations. `AsOccupant` is borrowed; acquired views and explicit AddRefs are balanced; created occupants use FC4_RELEASE_CREATED. Final Release counts match independently allocated Rust references and both lifetimes end at zero.

Each final probe pass creates5 native originals and performs810 **mixed checks**, not810 independent algorithm results. These include native/Rust record/query/mutation/stream/reference comparisons, published ABI/type/identity assertions and Rust-only sanity checks. The successful native Writes are compared against independently generated typed-input Rust bytes, never a second Rust oracle. Each pass has10 complete byte comparisons of66 bytes,345 Read-result comparisons (full inputs, every strict truncation0..65, bad flora version, bad base version),15 Write-result checks including pre-existing sticky errors, and25 native/Rust reference-count comparisons. Native setter/get-position return channels without specified results are ignored and counted separately.

The final guarded run has8 passes/40 owned creations,6480 checks/0 differences,2720 refusals,560 undefined return observations and0 fallbacks. Reload has2 passes/10 owned creations,1620 checks/0 differences,680 refusals,140 undefined observations and0 fallbacks. Combined:50 created originals across repetitions of5 typed cases,8100 mixed checks/0 differences,100 complete record-byte comparisons (6600 bytes),3450 Read results,150 Write results,250 reference counts,3400 refusals and700 undefined observations. These are repetitions, not50 unique input cases.

Each pass's340 refusals are330 unproved partial-state comparisons after short reads and10 unproved bounds projections; only the short-read **result** is compared. Unexercised methods below are explicit coverage gaps, not additional measured runtime refusals or zero-event successes. No unaccounted mismatch or fallback was observed within instrumented paths; no assertion of full method accounting is made.

Natural snapshots are separate read-only observations: guard20658 repeated occupant visits/123948 public queries; reload5144 visits/30864 queries; combined25802 visits/154812 queries, invalid0, mutations0 and computed credit0. Their values never become owned-corpus expected outputs. The older diagnostic DLL's585/0 checks and2607 visits are retained separately, not added to final-DLL totals.

## Validation, questions and remaining work

All four unique run labels above have completed clean/ref0 verdicts. Final DLL SHA256 and saved fixture SHA256 are recorded above and matched in both fixture staging results. The script saved the isolated game city successfully before the reload; game files remain uncommitted. The probe-disabled control ran to2032 with zero probe events. No turbo-only differences occurred, so no normal-speed diagnostic was required. This is not a cross-run city-state equality claim: original pacing varies, and the end dates overshoot by different days.

Baseline all passed; final stable all on code17369af7 passed4941 Rust tests, clippy-Dwarnings with0 diagnostics, CMake4/4 and CTest68/68. Focused quick passed3452 tests and dependent builds before the final two tests were added. New Rust files pass qbuild fmt; checking shared module roots recursively reports existing formatting differences in unrelated modules (left untouched). Evidence: build-runs/qbuild-*.log, fc4verify-*.log, game-results/<label>.{log,verdict}, flora-r2-counts.json, and immutable dlls/vfocc-r2-final/.

FQ17 is newly appended to docs/specs/questions/flora.md for short-read partial state, constructor defaults and unproved flora-specific inherited projections. FQ16 remains excluded; no static-record credit is requested. Coordinator must regenerate the question index; no new RE was requested or performed.

Remaining work before full occupant verification credit: exercise normal states0..4/orientations0..3 as well as the current byte-narrowing cases; complete positive canonical-IUnknown/cross-view QI and its failure/out-parameter matrix; add supported inherited visibility, lot-tag, holder/group and lifecycle cases, and explicitly enumerate refusals for methods whose flora override or message lifetime remains unpublished. Init/Shutdown, placeable objects, notifications, bounds/city-cell queries, nonempty property holders, static-pointer assignment and constructor-default state are not proved by these runs. Some flag/removal snapshot predicates remain C++ assertions over independent Rust fields rather than Rust query methods. Do not promote the whole owner or simulator71/71 coverage from this partial instrumentation.

No game requests remain outstanding. No subagents, restart, merge, push, tools/maps/default/ABI-table/unreconciled edits, game executable access, or private native fields were used.
