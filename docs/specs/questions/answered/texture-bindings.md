# texture-bindings: answered questions (archive)
<!-- Archive for [texture-bindings.md](../../texture-bindings.md). Lanes do not read this file. Open questions are in ../texture-bindings.md. -->

<!-- from: re-texbind (2026-10-19) -->
- ~~**TB3 - §3:** what the material texture-slot byte `+0x0C` (1 by default, `0xFF` for an unused slot) and the float
  `+0x10` (1.0) mean to the renderer.~~
  **ANSWERED 2026-10-20 (re-drawcontext):** the draw context reads slot byte `+0x0C` as the stage's texture-environment
  mode, `+0x0D` as its coordinate set, `+0x0E` bit 0 as a texture-transform switch and the float `+0x10` as that
  transform's scale divisor (k = 1 / value); slots past T are never read. Written into texture-bindings.md §3 and
  draw-context.md §6.

<!-- from: core-textures-materials4 (2026-10-22) -->
~~- **TB5 - §§1.4, 1.6, 2 numeric domain:** specify operand widths, signedness, operation order and overflow/shift behavior for zero image levels, zero frames, dimensions above `0x80000000`, LOD/level shifts of at least 32, byte totals outside u32, aggregate pool/in-use counter overflow, and binding-reference overflow. [Portable core rejects zero levels/unrepresentable per-binding sizes and reference overflow, uses mathematical right shifts (zero after the word width), computes bytes with wide intermediates and aggregate counters, and accepts zero frames. These boundary choices are safety extensions pending publication, not native parity.]~~

**ANSWERED 2026-10-22 (re-small-renderer-contracts4):** DIFFERS — §1.11: signed power rounding, x86 masked shifts and wrapping u32 arithmetic; zero levels select one level, zero frames are accepted, binding references wrap.

<!-- from: core-textures-materials4 (2026-10-22) -->
~~- **TB6 - §§1.2, 1.5 keyed instance zero:** slot 6 says to cache any (group, instance), but the record uses instance zero as the unkeyed sentinel. What happens for a key whose instance is zero, including reuse, destruction and restoration? [Core rejects keyed instance zero; image-info bindings remain unkeyed. No native failure behavior is claimed.]~~

**ANSWERED 2026-10-22 (re-small-renderer-contracts4):** DIFFERS — §1.12: instance zero is accepted and cached; reuse leaves a stale key and eviction leaves a dangling value; restoration visits table entries.

<!-- from: core-textures-materials4 (2026-10-22) -->
~~- **TB7 - §§1.5, 1.8 upload/failure boundaries:** on driver creation error, is the just-returned handle deleted as well as prior successful handles (including a zero handle)? For SetBindingImageData/restoration, publish behavior when image frame/level counts disagree with the binding, a source level is missing/null, or the upload driver reports an error; is keyed restoration traversal order observable/required? [Core includes the current returned handle in creation cleanup, stops at a missing source level or insufficient level count, ignores upload error status, and restores every keyed entry in deterministic key order. These invalid/error-domain and ordering choices are not additional native contracts.]~~

**ANSWERED 2026-10-22 (re-small-renderer-contracts4):** DIFFERS — §1.12: failure cleanup excludes the current returned handle; upload trusts source frames and binding levels, forwards null data, ignores driver returns; restore follows native hash traversal.

<!-- from: core-textures-materials4 (2026-10-22) -->
~~- **TB4 - §§2–3 complete owner/body inventory:** publish the missing `cS3DMaterial` methods and lifetime (including whether/how its 16-bit count and held texture bindings are released), and material-manager primary slots 6/7 (`0x0062C980` in the public ABI table). Section 3 publishes the material constructor state only and manager slots 3–5, not these bodies. Also reconcile any owner-private/destructor methods absent from §§1–3 against the requested 58-function owner inventory. [Core implements the published constructor state and manager slots 0–5 with explicit foreign system-service primitives; no material reference/binding-release or slots 6/7 contract is invented, and no blanket four-owner completion claim is made.]~~

  **Partial proof 2026-10-22 (re-small-renderer-contracts4):** §§3–3.1 establish slots 6/7, manager destruction, material release at counts 0/1, wrapping acquisition, reverse binding destruction, and value-copy semantics. Residual: legacy 58/4883 counting scope versus 55 named bodies/4758B plus four class/create helpers/111B; no blanket inventory-completion claim.

**ANSWERED 2026-10-22 (re-small-renderer-contracts4):** DIFFERS — §§3.1–3.2 establish material lifetime and manager slots 6/7; the exact 58/4883 subsystem scope is 53 class bodies/4749 plus five free functions/134, excluding two folded entries/9 and all named data.

<!-- from: re-texbind (2026-10-19) -->
- ~~**TB2 - §1.9:** who sets the factory's display bit depth (slot 10) and its LOD offset (slot 9), and from which
  settings? graphics-formats.md §2.10 infers the texture-quality option for the LOD offset.~~
  **ANSWERED 2026-10-27 (re-q-misc):** MATCHES — the only setter found is the effects manager's handler of RenderPropertiesChanged `0xC9F775BB` (`0x00593E10`, also run once at effects load): it calls `SetFreePoolMax` with kTexBindMaxFree × 1,000,000, `SetLodOffset` with kLODOffset (integer render property 3) and slot 10 with kTextureBits (integer 2); defaults equal the factory's initial state. Which option feeds those properties is data (Graphics Rules), not in the exe (§1.9).
