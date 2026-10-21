| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| core-span8-4: pure-Rust span render 8 core (nGZDrawSoftRender::cSpanRender8, alpha plane) from docs/specs/span-render-8.md | `core-span8-4` / (engine lane) | ready, no ABI change; core commit `c3e97a63`; gate cargo 3696/0 (baseline 3676, +20), clippy 0, ctest 62/62; no game runs (core lane); SR8-3 filed | 1. coordinator: regenerate QUESTIONS.md index (SR8 prefix not listed yet). 2. coverage map: span render 8 core path `engine/core/src/span_render8`. 3. glue only once SR8-2 names a shipped type-2 render target and SR8-1 the solid byte mapping. |

## Report

Complete against the spec: all five render entries and the full 168-slot dispatch with 125 distinct bodies.

What changed (core only, no CAPI/plugin/CMake): `engine/core/src/span_render8/` (mod.rs, blt.rs, tests.rs), plus
`pub mod span_render8;` in engine/core/src/lib.rs.
- The target is an alpha (coverage) plane, one byte per pixel. Nothing treats it as an indexed colour.
- `render_solid` stores the colour's low byte. `render_alpha` ignores argb and calls Solid.
  `render_gradient`/`render_alpha_gradient` use only channel 3, `(c3 + dx3*x + dy3*y) >> 16 & 0xFF`, wrapping.
- `DISPATCH` is the 14x12 table of body addresses (F/T fill bodies). Row = key byte + 2*f, with the key byte
  added as-is (the quirk is kept). Indexes past the table are errors.
- Texel alpha per §7.2. Columns: Blt = A; ConstantAlpha and ModulateAlpha = 0xFF; Modulate = (A*ca+128)>>8, so
  255*255 gives 254.
- Key: the full dword is compared against texel & mask. Row 13 tests only key != 0.
- Plain, stretch (wrapping, logical shift) and tiled (unsigned mod per span, sx wraps at W) addressing.
- `render_blt_aliased` is for a source inside the target buffer. It reads and writes in ascending order, per span.
- SR8-1 and SR8-2 are not guessed. The solid byte is an explicit input. No consumer exists until a type-2 target
  caller is known.
- Bounded API: every destination and texel address is checked before any write. Count 0, a bad index, W=0 (or H=0
  for a body that reads texels) and out-of-range reads are errors, which is a choice of this API.

Tests (20) come from spec statements and examples:
- every spec example
- 125 bodies, all in the owner range
- every shared body gives identical output on all of its slots (randomised, key hits and misses)
- the column rule for all 168 slots
- the key-byte row quirk, logical-shift stretch, tiled wrap including 32-bit v0, zero tile sizes
- fills that read nothing, aliasing, overlap, negative stride, bounds

None of the RE proofs or original bytes were used.

Questions: SR8-3 (does row 13 ConstantAlpha read a texel in plain/stretch; does row 13 with key 0 read texels).
