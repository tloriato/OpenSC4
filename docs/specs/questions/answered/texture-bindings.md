# texture-bindings: answered questions (archive)
<!-- Archive for [texture-bindings.md](../../texture-bindings.md). Lanes do not read this file. Open questions are in ../texture-bindings.md. -->

<!-- from: re-texbind (2026-10-19) -->
- ~~**TB3 - §3:** what the material texture-slot byte `+0x0C` (1 by default, `0xFF` for an unused slot) and the float
  `+0x10` (1.0) mean to the renderer.~~
  **ANSWERED 2026-10-20 (re-drawcontext):** the draw context reads slot byte `+0x0C` as the stage's texture-environment
  mode, `+0x0D` as its coordinate set, `+0x0E` bit 0 as a texture-transform switch and the float `+0x10` as that
  transform's scale divisor (k = 1 / value); slots past T are never read. Written into texture-bindings.md §3 and
  draw-context.md §6.
