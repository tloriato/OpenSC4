| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| core-span565-4: pure Rust span renderer 565 core (`nGZDrawSoftRender::cSpanRender565`) from span-render-565.md | `core-span565-4` / (engine lane) | ready, core only, no ABI change; commits `0e89c2ad`, `552856c3` + test fix; gate cargo 3671/0, clippy 0, ctest 62/62; no game runs (core lane, no consumer); SR565-2..4 filed | 1. coordinator: coverage `span-render-565` row core path `core/src/span_render565`; regenerate QUESTIONS.md for SR565-2..4. 2. spec lane: answer SR565-2..4. 3. glue lane only once a real consumer (a 565 render-to-buffer path) exists. |

## Report

**What changed** (engine/core only; no capi, plugin, CMake or ABI):
- `engine/core/src/span_render565/mod.rs`: records (`Span`, `Dest`, `Source`, `BltParams`, `Gradient`), the five
  methods `render_solid` (§2), `render_alpha` (§3), `render_gradient` (§4.2, B start logical `>>3`, step
  arithmetic `>>3`, so the B drift is reproduced), `render_alpha_gradient` (§5, `w = (V3·0x84) >> 26`, up to 63),
  `render_blt` (§6, row `key_flag + 2·format` unchecked except against the 14 × 12 table).
- `blend.rs`: `B(S, D, w)` packed formula (§3) and `alpha_weight`.
- `blit.rs`: the §7 body-address table (identity only), `slot(row, kind)` (format = row/2, keyed = row&1,
  op = kind&3, addressing = kind>>2), texel → 565 (§7.2), Modulate 8-bit expansion and `(c·cc+128)>>8` (§7.3),
  ConstantAlpha `ca>>3`, ModulateAlpha `mA>>3`, key `(texel & mask_f) == key` (8-bit mask 0), plain / stretch
  (logical `>>16` of 32-bit wrapping sums, absolute x/y) / tiled (`(x+u0) mod 2³² mod W`, step and wrap at W).
  Per pixel texel → destination → write, so aliased source = ascending copy (§1).
- Memory model: one byte arena with signed offsets for dst and src (aliasing works). Explicit preconditions,
  returned as `SpanError`, not claims about the original: empty list (original walks 2³² records), span not on
  one row / `end.x < start.x`, address outside the arena (original has no bounds check, 32-bit pointer wrap not
  modelled), slot outside the table, tiled W or H = 0 (original faults). Pixels written before an error stay.
- The core reads no texel in 8-bit Blt / ConstantAlpha / Modulate slots and in the 8-bit keyed compare (spec:
  texel ignored); this only decides which out-of-arena inputs are rejected.

**Tests** (19, `span_render565::tests`): blend worked example + per-channel lerp equivalence on 20 000 random
inputs; all 32 alpha-weight thresholds; solid low-16 bits, zero-length span, negative stride; preconditions;
RenderAlpha a8 ≤ 7 / ≥ 254; gradient B drift worked example, exact R/G bits, white 0x00FF8000 → 0xFFFF, step
rounding toward −∞; alpha gradient w = 32 and out-of-range w = 63; §7.2 conversion examples; §7.3 column
examples (white over black 30/61/30, R565 × 0x80808080 = 0x8410, A4444 modulate); key masks, key bits outside
the mask, 8-bit keyed row; stretch logical shift (u = 0xFFFF0000 → texel 0xFFFF); tiled wrap and 32-bit u0;
aliasing smear; dispatch out of table and key_flag 2; body table 168 / 141 distinct / 139 in owner range;
every folded slot pair (27) gives identical results on random inputs; every slot runs and keys hold.

**Status**: fully implemented exact owner for everything the spec states; three literal readings open as
questions: SR565-2 (A1555 1-bit alpha → 0/255 in ModulateAlpha), SR565-3 (A-format ConstantAlpha bodies are
distinct but §7.3 ignores texel alpha), SR565-4 (X888 texel read as 4 bytes). SR565-1 (gradient reachability)
remains. Producer `SetGradientTransform` (other class, x87 rounding) not implemented here.

**Gate**: baseline cargo 3652/0, clippy 0, ctest 62/62; final cargo 3671/0, clippy 0, ctest 62/62. Game: none
(core-only lane, no consumer, game false).
