| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| core-span8888-4: pure Rust core of span-render-8888 (`nGZDrawSoftRender::cSpanRender8888`) | `core-span8888-4` / (engine lane) | ready: code 444fad75; gate cargo 3703/0 (baseline 3676), clippy 0, ctest 62/62; no ABI, no game run (no consumer yet); questions SR8888-1..5 | 1. answer SR8888-1..5; 2. wire to CAPI/plugin only when a real consumer (cGZDrawContextRenderToBuffer) exists |

## Report

What changed (engine/core only; no CAPI, plugin or CMake):
- `engine/core/src/span_render8888/` (mod.rs, methods.rs, blt.rs, tests.rs), `pub mod span_render8888;` in
  engine/core/src/lib.rs. Written only from docs/specs/span-render-8888.md.
- Entries: `render_solid`, `render_alpha` (weight `(a>>24)+(a>>31)`), `render_gradient` (accR `|= 0xFF000000`,
  R carry/borrow reaches alpha), `render_alpha_gradient` (verbatim packed blend, exact for w up to 65535),
  `render_blt` (dispatch `table[format*2+keyed][mode]`) and `run_kernel` for each of the 168 kernels
  (`Kernel::from_index(row, mode)`: 7 formats x keyed x 3 geometries x 4 ops).
- Shared rules: `blend` (per-channel `floor((256d + w(s-d) + 128)/256)`, alpha 0xFF), `widen` (exact bit
  replication of all 7 formats), `modulate` (`(C*K+128)>>8` per byte incl. alpha), `keyed_skip` (raw-texel masks
  0x0FFF/0x7FFF/0x7FFF/0xFFFF/0xFFFFFF/0xFFFFFF; A8 keyed by `key == 0`), geometry direct/stretch (32-bit wrapping
  u/v, unsigned >>16)/tiled (`(x+u0) mod width`, wrap to 0).
- Memory model: `Mem::Shared(&mut [u8])` (source and destination offsets in one buffer, may overlap) or
  `Mem::Split`. Pixels run spans in order, left to right, each reading live memory, so overlap behaves like the
  pointer loop (smear when the source is one pixel left; no snapshot/memmove). Signed strides supported.
- Bounded API choices (not original behaviour): count 0 -> EmptySpanList (U1); format > 6, keyed byte > 1,
  mode > 11 -> BadKernelIndex (U2); tiled width/height 0 -> ZeroTileSize (U4); any destination or texel address
  outside the slice (64-bit exact arithmetic) -> Dest/SourceOutOfBounds, checked for every pixel before any write.

Tests (27): worked examples of sections 2, 5, 9.2; blend ends and rounding; packed alpha-gradient blend equals the
rule for every w 0..256 over a channel grid and two hand-computed w > 256 values; every 16-bit texel widened; all
168 kernels distinct and round-trip; dispatch rejects; key masks per format incl. raw-before-widen and A8; stretch
steps and 32-bit u wrap; tiled offset and wrap; row padding and negative stride; error paths leave memory untouched;
overlap left/right/in place/across spans/16-bit source over 32-bit writes.

Gate: `qbuild all` cargo test 3703 passed 0 failed (baseline 3676), clippy 0 diagnostics, CMake build PASS, ctest
62/62. Game: none (pure core, no consumer). Coverage: implemented core for the whole owner (5 methods, 168 kernels,
blend helper); shadow/replace none.

Questions filed (docs/specs/questions/span-render-8888.md): SR8888-1 overlap traversal order per kernel,
SR8888-2 keyed byte > 1 / format wrap, SR8888-3 keyed A8 key 0 reads, SR8888-4 "garbage" for w > 256 not
reproduced in computed cases, SR8888-5 32-bit address wrap.
