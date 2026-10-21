| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| re-span8-4 | workflow / `re-span8-4` | **ready** (specs only, no ABI rows): `docs/specs/span-render-8.md` complete — all 130 owner functions (5 methods + 125 blit bodies) [exact] or [verified]; 168/168 blit slots verified by emulation (random, via-RenderBlt, texel sweep, aliased overlap); SR8-1, SR8-2 filed | set the spec column of `nGZDrawSoftRender::cSpanRender8` in tools/coverage-map.tsv to `span-render-8.md` (not edited here); SR8-1 (type-2 buffer colour mapping), SR8-2 (who creates a type-2 target) |

## Report

**What changed.** Commits `bdcf6efb`, `caad2eab`, `9d1d2a32` (unicorn proof harness `docs/specs/proofs/span8_*.py`:
loader, model, methods, blit slots, texel sweep, aliased overlap), `5708410e` (spec `docs/specs/span-render-8.md`,
questions `docs/specs/questions/span-render-8.md` SR8-1/SR8-2, QUESTIONS.md index). Docs only; no engine/, no ABI.

**Findings (all from the 8-bit bytes).**
- Type-2 buffers get vtable `0x00AC188C` (Init jump table `0x008308B0`). RenderSolid stores the colour's low byte;
  RenderAlpha ignores `argb` and virtually calls RenderSolid; RenderAlphaGradient tail-jumps to RenderGradient;
  RenderGradient reads only channel 3 (c3/dx3/dy3, the alpha channel) and stores bits 16-23.
- RenderBlt: table `0x00AC26B0`, 14x12, 125 distinct bodies. Every blit body uses only the source alpha: A4444
  nibble*17, A1555 bit15 -> 0/255, A8888 byte 3, 8-bit texel itself, X555/R565/X888 -> 255. Blt stores A;
  Modulate stores (A*ca+128)>>8 (opaque * 0xFF = 0xFE); ConstantAlpha and ModulateAlpha store 0xFF (ModulateAlpha
  computes and discards the product). Fill bodies `0x0085FCB0` / `0x00861BB0` (tiled, still divides by W).
  Key: skip when (texel & mask) == full key dword; row 13 tests key != 0. Addressing as the 565 renderer (logical
  shifts, unsigned div). So the buffer behaves as an alpha plane on the blit/gradient paths.

**Measured (2026-10-21).** `span8_methods.py 300` 5/5 PASS; `span8_blt.py 300` 168/168; `span8_blt.py via 100`
168/168; `span8_sweep.py 4` 168/168; `span8_alias.py 200` 168/168.

**Tooling notes.** REA was unavailable (no binary open in the shared session): everything came from re_query.py asm,
decomp/ and emulation. unicorn installed with `pip install --target <WT>/build-runs/pylib unicorn` (the harness finds
it). tools/coverage-map.tsv row for cSpanRender8 still has spec `-`: the coordinator/merge train should set it.

**Questions.** SR8-1 (meaning of the solid colour byte: target vfunc +0x78 mapping), SR8-2 (shipped creator of a
type-2 render target).
