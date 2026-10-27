# glyph-textures: answered questions

<!-- from: core-glyph-textures4 (2026-10-22) -->
~~- **GT3 - §§1.2–1.5 numeric boundaries:** please publish the input/output integer widths and signedness for font dimensions, `P(n)` at zero/overflow, the point of 16-bit narrowing versus UV division, and whether cache bytes use stored string dimensions or render-result dimensions. What precision, rounding and zero-budget behavior apply to `B/L > 1.1` and `B - 0.1L`, and are budget multiplication/byte counters wrapping? [Core uses unsigned u32 metrics, checked P(width+2), u16 assignment afterwards; UVs divide by original renderer dimensions; string stored dimensions for w*h*4; f64 target truncated after subtraction, IEEE zero-budget division, wrapping u32 field arithmetic. Unrepresentable P is an explicit error, not a claimed native result.]~~

**ANSWERED 2026-10-22 (re-small-renderer-contracts4):** DIFFERS — §3.2: signed P and metric interpretation, low-u16 storage, signed original dimensions for UVs, stored dimensions for cost, float32 constants/rounded B with x87 target arithmetic and wrapping counters.

<!-- from: core-glyph-textures4 (2026-10-22) -->
~~- **GT5 - §1.6 restore outputs/failure:** does restore change only the glyph binding or also glyph/texture dimensions and UVs? Is map order the re-render order? After releasing the old binding, what happens if font lookup or rendering fails? What source rectangle is passed? [Literal core: clear string cache first, ordered map traversal, release old binding before lookup/render, unit source rectangle, replace binding on success and leave it empty on failure, retain metadata. No factory leaves glyphs untouched.]~~

**ANSWERED 2026-10-22 (re-small-renderer-contracts4):** DIFFERS — §3.2: map-order restore clears old binding and supplies its slot directly to the renderer even on failure, without metadata updates; no manager-level rollback.

<!-- from: core-glyph-textures4 (2026-10-22) -->
~~- **GT6 - §§1.3–1.4, 2–3 output/reference and invalid-input contracts:** does glyph slot 7 return a borrowed binding or AddRef it; what are glyph/string slot 0 QI IDs and output mutation rules on failed lookups/renders; what happens on a null font in GetCachedStringTexture (specified always true); how does StringToGlyphTextureArray handle GetCharLength returning 0 or a length beyond remaining bytes; and does an initial empty source string attempt a glyph? [Core exposes a borrowed binding getter, token-returning lookups, explicit MissingFont error for strings, stops at invalid character boundaries, and makes no glyph call for empty input. No native QI/failure-output contract invented.]~~

**ANSWERED 2026-10-22 (re-small-renderer-contracts4):** DIFFERS — §§3.1–3.2: both QI slots always fail with untouched output, binding getter borrows, glyph failure preserves output, null string font is dereferenced, character lengths are trusted and empty input makes no glyph request.

<!-- from: core-glyph-textures4 (2026-10-22) -->
~~- **GT4 - complete owner/body inventory and lifecycle:** the brief assigns 43 functions/4475B to cSC4GlyphTextureManager, cSC4GlyphTextureInfo and cSC4StringTextureInfo, but the published spec does not enumerate every body. Please reconcile the exact inventory, including manager constructor/destructor (`0x005D8900`, deleting wrapper `0x005D8D40`), initial reference counts/last-sweep time, system-service slots 3–7/11–12, auxiliary-view slots 4–5, message-target QI/refcount thunks, and glyph/string slot 0 if present. What destructor cleanup order and flags behavior apply? Does manager-link assignment AddRef the new manager before releasing the old, and what does glyph-map removal do if its key is absent? [Core implements published paths only; explicit caller-supplied initial manager references, zero initial sweep time, provisional shutdown-then-cache context teardown; no complete-manager or 43-body credit claimed.]~~

  **Partial proof 2026-10-22 (re-small-renderer-contracts4):** §1.1.1 establishes constructor at 005D8CA0, initial refs/sweep=0, inherited slots/thunks, storage-only destructor distinct from Shutdown, acquire-before-release manager assignment, absent-pointer removal no-op; §3.1 establishes info slot 0. Residual: exact legacy 43/4475 inventory scope versus 38 named bodies/4047B plus shared/unnamed slots; no complete-manager count claim.

**ANSWERED 2026-10-22 (re-small-renderer-contracts4):** DIFFERS — §§1.1.1/3.1/3.3–3.4 establish construction, inherited entries, destruction and private insertion. Exact 43/4475 attribution includes two tree helpers and two unrelated ordinance neighbor thunks; it is not a 43-method manager API.

<!-- from: re-glyphs (2026-10-19) -->
- ~~**GT1 - §0, §1.3:** what `RZ::WCM()` (`0x00913C72`) returns, and whether its slot 5 is a font lookup by ID as this
  spec infers.~~
  **ANSWERED 2026-10-27 (re-q-misc):** MATCHES — `RZ::WCM()` returns the window control manager service (`cIGZWinCtrlMgr`, interface `0x22C2EB1F` of service `0xC2C2EB0F`), cached in `0x00B628C4`; its slot 5 (`SystemFont`, `0x00997B55`) looks the font ID up in the manager's table and returns the stored font object or null, as the spec inferred (glyph-textures.md §1.3).

<!-- from: re-glyphs (2026-10-19) -->
- ~~**GT2 - §1.2:** which framework services `Init` registers the glyph manager with through framework slots 3 and 9
  (`0x005D7D50`); the tick list is inferred from `OnTick`.~~
  **ANSWERED 2026-10-27 (re-q-misc):** MATCHES — framework slot 3 is `AddSystemService` and slot 9 (`+0x24`) is `AddToTick`, both given the `+0x04` system-service view, so the manager is registered under service ID `0xABEB5E37` at priority −2,000,000 and ticked every frame, as the spec inferred (glyph-textures.md §1.2).
