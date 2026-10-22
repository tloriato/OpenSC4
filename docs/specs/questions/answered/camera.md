# camera: answered questions (archive)
<!-- Archive for [camera.md](../../camera.md). Lanes do not read this file. Open questions are in ../camera.md. -->

<!-- from: re-camera (2026-10-19) -->
- ~~**CA2 - §1.1, §1.4:** the depth bias at `+0x1A4` of `cS3DCamera`: nothing in this lane sets it. Which caller does,
  and to what?~~
  **ANSWERED 2026-10-20 (re-drawcontext):** `cS3DCamera::SetDepthBias` (`0x007FF0F0`) sets it; its one caller is
  `SC4DrawContext::SetDepthOffset(n)` (`0x007D4480`), which passes `kDepthOffset` (integer property 16, 2 by
  default) × n and reloads the projection. Written into camera.md §1.1–§1.2 and draw-context.md §8.
