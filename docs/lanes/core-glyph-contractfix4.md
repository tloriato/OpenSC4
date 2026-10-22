| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| core-glyph-contractfix4 (glyph and string texture contracts) | `Codex` / `core-glyph-contractfix4` (engine lane) | in progress | (1) Correct existing owner; (2) focused and final full gate; (3) queue bounded source review. |

## Handoff
- Goal: apply published portable glyph manager/info contracts to existing Rust owner only.
- Done: read published §§1–3.4/7, answered GT3–GT6 and signed P contract; baseline all launched before edits (logs in build-runs).
- Done: substantive corrections committed as 6dcdff59 (before call 30); inherited state, wrapping refs, storage-only destruction, renderer slot writes, signed arithmetic, identity removal and unique insertion.
- Done: 11 independent contract tests added (30 total glyph tests), expanded metric vectors, exact rational float32 UV stores and fixed-point sweep arithmetic, wrapped tree/cache count words, typed unsupported string paths; touched-file fmt PASS.
- Done: corrected source and first 30 glyph tests committed as fabf19ce; exact 43-body/shared-entry/unsupported checklist below; fmt PASS (2 touched Rust files), diff --check clean.
- Done: 5169862c fixes x87 subtraction tails via error-free TwoSum and 64-bit-significand rounding, distinguishing 2^-60, half-spacing 2^-65, and 2^-70. There are now 31 glyph tests (12 new); fmt PASS on both touched Rust files after this commit, diff --check clean.
- Done: restore now reads font identity from each map key (glyph-info has no native font-ID field); a 32nd glyph test covers two map keys holding the same pointer with separate references.
- Final phase (phase 2/2, logical 3/3, no further round): OS Toolhelp/NtQueryInformationProcess command-line inspection found no live lane qbuild/compiler/PowerShell driver; CIM was denied. Old session numbers are historical, not liveness evidence.
- Preserved prior four gate logs as build-runs/phase1-*.log before replacement; quick logs end at lock wait. Historical qbuild-test.log contains 40440 NUL bytes, LNK1104 on automata_records and 4441 baseline tests/19 glyph tests, not a final-source gate. Its four doctest suites completed but are baseline only.
- Narrow fix: remove the demonstrated blank line at mod.rs:437 (clippy empty_line_after_doc_comments); behavior/source 6d551576 otherwise retained. Current intra-doc link global_cache_misses resolves to the public function in this module; require fresh doctest completion before claiming final PASS.
- Done: f019f36a removes only the demonstrated blank doc line; diff from 6d551576 is one removed blank line in mod.rs. Source frozen for serial quick -> touched-file fmt -> all, CARGO_BUILD_JOBS=RUST_TEST_THREADS=1.
- Failed attempt 1: f019f36a quick core finished 3059 passed/1 failed (3060 actual), glyph 31/32; compiler 17m28s, tests 105.90s. Failure is old sweep vector (111,100)->101, conflicting with published GT3 float32 factor. Full logs retained as build-runs/phase2-attempt1-fc4verify-quick-{test,deps}.log. Remaining own dependency builds/driver were stopped before any edits; OS inspection found no surviving lane cargo/compiler.
- Narrow test correction: 0x3DCCCCCD = 13421773/134217728, so 111 - 100*factor = 101 - 20/134217728, truncating to 100. Correct stale expected 101 to 100 and add adjacent (112,100)->101; retain 32 tests/13 new. No production arithmetic change or source-fitting.
- Done: 6c5a12ab corrects the stale contract vector only; production remains 6d551576 plus f019f36a's blank-line removal. Failed driver is stopped and logs archived; no overlapping compiler/quick driver before restart.
- Running: final-glyph-gate-r2.ps1 (exec session 23749), serial quick -> touched-file fmt -> all, jobs/testthreads=1. Frozen source 6c5a12abbb1069b37b71b578d69d262d492c3e32; engine ea151c3fa85633f402efd7c4aba4dc77601f1f86; glyph 3eeae93406b7c7b851155666d0eed81b2eaa69e7. Recheck actual OS processes before any restart, never infer from a session number.
- Measured corrected-source focused cargo: core 3060 passed/0 failed, glyph 32/32, core doctest suite completed (0 tests); compile 10m21s, core tests 177.30s, raw log NUL=0. Engine/glyph hashes still match frozen source. Quick's dependency compilation is now running; fmt/all/full CTest remain pending, no final PASS or queue.
- Next: collect completed quick/fmt/all, summed workspace totals, clippy diagnostics, all doctests/link completion, full CTest and engine tree-changed flag. Do not duplicate this live pipeline or edit source.
- Scope: glyph_textures only plus independent tests/conformance; no ABI, game, textures_materials, RE, or shared-table changes.
- Reservation is FINAL phase 2/2, logical 3/3, maxRounds=1; no further round, rename or budget reset. Original parked evidence retained.
- GT1/GT2 remain foreign unknown boundaries. Unsupported invalid-memory/nontermination must stay explicit.
- Baseline cargo completed: 4441 passed / 0 failed across workspace, integration and doc-test summaries; capi 2, conformance 1232, core 3047 (19 original glyph tests), formats 147, other tests 13. Installed-data sweep finished in 515.42s. Baseline clippy waits on the build-directory lock; native/CTest still pending. No corrected-test PASS claimed.
- Historical pipeline final-glyph-gate.ps1/session 79604 is dead, as are prior baseline/quick/all wrappers (96574/92060/15722). q.py failed with Bash E_ACCESSDENIED (q/20261022-215043-5792.log); documented direct PowerShell fallback is used. No game requests.
- Independently recomputed published inventory: manager 30/3938, glyph 8/324, string 5/213 = 43/4475, including four remote entries but excluding two actual ordinance behaviors. Current/original/new glyph tests are 32/19/13. No whole-owner/native credit.
- Final fmt PASS on both touched Rust files at 22:04 local; final tree clean before this handoff update. Only four tracked files changed from base (owner, tests, size vectors, this lane record). No queue entry, no game request, ABI unchanged. Formatting helper uses rustfmt stdin plus engine/rustfmt.toml and apply_patch only.
- Manifest: C:/Users/tlori/FreeCity4/build-runs/run4-core-glyph-contractfix4-r2-manifest.json; final maxRounds=1, no subagents/game/native/ABI/default/maps/coverage work.

## Review checklist (bounded portable contracts)

The exact attribution remains **43 bodies / 4475 bytes**: manager 30/3938, glyph-info 8/324, string-info 5/213. Attribution is not a 43-method manager API and does not establish whole-owner completion. Portable receiver adjustment delegates to the same primary Rust operations; this lane adds no binary thunks or integration.

| Attributed bodies | Portable implementation / disposition |
|---|---|
| Manager 005D7B50, 005D7B70 | Wrapping Release/deletion and manager-only QI; shared AddRef wraps too. |
| Manager 005D7B90, 005D7C00 | Wrapping metric+2, signed P, narrowing; foreign string comparison followed by unsigned font IDs. |
| Manager 005D7D50, 005D7E20 | Once-only Init/registration/subscription and wrapped budget; signed sweep, rounded float32 B, exact float32 constants and truncation boundaries. |
| Manager 005D7F00, 005D8150 | Lazy string cache, identity hits/MRU/used, wrapped stored-dimension cost/counters; independent string creation and retained-count-1 Release quirk. |
| Manager 005D8330, 005D8570 | Restore clears/releases strings before ordered glyph replacement, trusts renderer output on failure without metadata changes; array clears/releases old entries, code-page splitting and signed remaining-length loop. |
| Manager 005D8800, 005D8900 | First pointer-identity removal erases before Release; destruction frees logical container storage without Shutdown, unsubscription, glyph-link changes or stored object Releases. |
| Manager 005D8980, 005D8990, 005D89A0, 005D89B0, 005D89C0, 005D89D0, 005D89E0, 005D89F0 | Adjusted-view operations use primary query/ref methods; auxiliary deleting entry uses flags wrapper; message target delegates to primary query/ref methods. Receiver offsets are native layout, not new owner decisions. |
| Manager 005D8A00, 005D8AF0, 005D8BA0 | Shutdown separately clears links/releases map refs/unsubscribes, leaves cache alone. Private helpers compute unique insertion/ordered linkage/count in Rust; duplicate retains first pointer, no glyph AddRef. Sorted safe storage represents logical tree order/endpoints, not native node allocation/rotations. |
| Manager 005D8CA0, 005D8D40, 005D9120 | Exact published initial manager/base words; storage-only destructor, bit-0 deletion flag and returned address; shared glyph lookup/output, x87-rounded subtraction through truncation, raw renderer metadata/UV order and signed original dimensions. |
| Remote 0080BCD0, 0080BD00 | Adjusted manager AddRef, same primary wrapping word. |
| Remote 0080BCE0, 0080BCF0 | **Excluded neighboring ordinance Release/QI attribution**, never exposed as manager operations. |
| Glyph 005D7C40, 005D7C70 | Equal-link no-op, acquire/store/release assignment; stored raw UV getter. |
| Glyph 005D8030, 005D8080, 005D80A0, 005D80C0 | Empty string/null binding/zero refs/null manager; unwritten metadata represented by None; character copy and zero-extended stored-size getters. |
| Glyph 005D8240, 005D8870 | Manager then binding then character destruction; bit-0 allocation flag; unsigned wrapping Release and map-removal recursion. |
| String 005D7C90, 005D8020, 005D80E0, 005D8130, 005D8290 | Wrapping Release and count-1 return/lifetime; live-manager delegation; empty/null/zero constructor with unwritten size; stored-size getter; font/text/manager destructor order and bit-0 deletion. |
| Shared entries outside that subtotal | Info QI always false with untouched output (including IID 1); info AddRef wraps; glyph binding getter borrows. Service slots 3–7/11–12 implement ID store/load, both priorities, unnormalized low-byte running store/load and inert true. Auxiliary slots 4/5 use a distinct saturating counter word. |

Unsupported paths are explicit, not native error/termination claims: signed-P nontermination; null-font metric dereference; zero/oversized code-page lengths; stale/native invalid object memory; reading constructor-unwritten metadata; string slot without a live local manager/font/binding; foreign-manager removal dispatch at glyph count 1; invalid x87 integer conversion and external nondefault FP environment. The auxiliary word's constructor value is not published, so its safe representation is unspecified until an explicit receiver word exists. Allocation failure/native node addresses and tree shape are not modeled. Rust arena teardown can free safe allocations later; that is separate from observable native destructor/Release operations.

GT1/GT2 remain explicit foreign font-source/registration boundaries. No new service identity, native/default/integration/whole-owner promotion, original-output seeds, ABI changes or game runs. This is FINAL logical round **3/3**, phase **2/2**, maxRounds **1**; the original parked/evidence record remains intact.
