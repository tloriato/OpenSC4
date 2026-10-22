| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| core-glyph-contractfix4 (glyph and string texture contracts) | `Codex` / `core-glyph-contractfix4` (engine lane) | in progress | (1) Correct existing owner; (2) focused and final full gate; (3) queue bounded source review. |

## Handoff
- Goal: apply published portable glyph manager/info contracts to existing Rust owner only.
- Done: read published §§1–3.4/7, answered GT3–GT6 and signed P contract; baseline all launched before edits (logs in build-runs).
- Done: substantive corrections committed as 6dcdff59 (before call 30); inherited state, wrapping refs, storage-only destruction, renderer slot writes, signed arithmetic, identity removal and unique insertion.
- Done: 11 independent contract tests added (30 total glyph tests), expanded metric vectors, exact rational float32 UV stores and fixed-point sweep arithmetic, wrapped tree/cache count words, typed unsupported string paths; touched-file fmt PASS.
- Done: corrected source and first 30 glyph tests committed as fabf19ce; exact 43-body/shared-entry/unsupported checklist below; fmt PASS (2 touched Rust files), diff --check clean.
- Done: final arithmetic audit found f64 subtraction loses a retained x87 tail at integer boundaries; error-free TwoSum plus explicit 64-bit-significand rounding now distinguishes 2^-60, half-spacing 2^-65, and 2^-70. Added a 31st independent glyph test.
- Next: run serial stable quick/fmt/all on the new arithmetic commit, inspect counts, replace Handoff with measured report, queue tested source.
- Scope: glyph_textures only plus independent tests/conformance; no ABI, game, textures_materials, RE, or shared-table changes.
- Family reservation remains logical round 2/3, this phase max 2 remaining rounds; original parked evidence retained.
- GT1/GT2 remain foreign unknown boundaries. Unsupported invalid-memory/nontermination must stay explicit.
- Baseline command returned session 96574; quick command session 92060 (do not poll; inspect quiet logs). No game requests.
- Focused quick reissued on corrected source after the independent vectors; quiet logs own the completion record.
- Prior all launched on fabf19ce (session 15722), superseded by the necessary x87-tail correction; do not treat it as the final stable gate.
- Baseline measured so far: capi 2/0, conformance 1232/0; all 19 original glyph tests passed. Whole baseline core/cargo/clippy/native/CTest still pending at 21:45 local. Corrected 30-test execution has not been claimed.
- Earlier quick/all wrappers still own their logs. The stable serial pipeline will use q.py to retain its own unambiguous console-count log under build-runs/q/.
- Baseline and first quick still compiling at 21:37 local, machine has many concurrent Rust compilations; no PASS claimed. Formatting helper uses rustfmt stdin plus engine/rustfmt.toml; only touched files change through apply_patch.
- Manifest: C:/Users/tlori/FreeCity4/build-runs/run4-core-glyph-contractfix4-r1-manifest.json.

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

GT1/GT2 remain explicit foreign font-source/registration boundaries. No new service identity, native/default/integration/whole-owner promotion, original-output seeds, ABI changes or game runs. The family stays logical round **2/3**, total cap **3**, this reservation maxRounds **1**; the original parked/evidence record remains intact.
