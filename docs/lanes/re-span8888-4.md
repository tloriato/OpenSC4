| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| re-span8888-4 | workflow / `re-span8888-4` | **ready** (specs only, no ABI rows): `docs/specs/span-render-8888.md` for `nGZDrawSoftRender::cSpanRender8888` (0x00854E10-0x0085E640, 174 fn, 37,218 B), all 174 functions executed against the spec, 0 mismatches | coverage "specified" (proposed verified-by-execution) for the class; optional: U4 input ranges |

## Report (round 1, 2026-10-21)

**What changed.** New spec `docs/specs/span-render-8888.md` (sections 1-12) and proof harness
`docs/specs/proofs/span-render-8888/` (emu.py interpreter adapted from re-span4444-4 plus `REP STOSD`, pe.py,
check.py for the 168 kernels, meth.py for the four span methods, blendproof.py numpy sweep; set `FC4_EXE` to
`bin/SimCity4.exe`).
- Class: vtable `0x00AC1874`, created for colour types 8/9; thiscall methods, `this` unread.
- Blend `B(d,s,w) = floor((256d + w(s-d) + 128)/256)` per R,G,B, alpha out 0xFF; packed RB borrow cancels exactly.
- RenderAlpha weight `(alpha>>24)+(alpha>>31)` (0..256); RenderGradient 8.16 B/G/R record, alpha byte = top of the
  running R accumulator (0xFF unless R wraps); RenderAlphaGradient weight `accW>>16` unclamped, exact packed rule
  for w>256 documented.
- RenderBlt: `table[format*2+keyed][mode]`, row pointers `0x00AC23D8`, entries `0x00AC2138-0x00AC23D7`, all 168 in
  range; widening to 8888 by bit replication; op1 weight `K>>24` (0..255), op2 MMX modulate `(C*K+128)>>8`,
  op3 blend by M alpha; key masks per format, A8 key degenerate.

**Measured.** check.py 150: 168/168 kernels, 747,927 px, 0 mismatches; EXH sweep 25 kernels 1,312,000 px, 0;
meth.py 300: 36,741 px, 0; BIGW 200: 6,323 px, 0; blendproof 2,425,356,288 cases, 0. Mutations all detected.

**Unknowns.** U1 count 0, U2 unchecked indices, U3 caller meaning of unread fields, U4 unexecuted input ranges.

**Questions.** None filed. **Tools.** REA not used (export + Capstone execution sufficed).

**Commits.** a5e402c2 start, 7f2ae734 spec, a7a38916 proofs, final commit with this report.
