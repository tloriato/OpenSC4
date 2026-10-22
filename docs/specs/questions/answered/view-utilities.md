# view-utilities: answered questions (archive)
<!-- Archive for [view-utilities.md](../../view-utilities.md). Lanes do not read this file. Open questions are in ../view-utilities.md. -->

<!-- from: re-viewutils (2026-10-20) -->
- ~~**VU1 - §7.1:** what mesh primitive types 6 and 7 are, and whether their odd triangles are intended: type 6
  gives (2i, 2i + 1, 2i + 2) for even i but (2i, 2i + 2, 2i + 3) for odd i, so a 4-vertex group reads vertices 4
  and 5; type 7 repeats triangles 1 and 2. A survey of the shipped S3D models' group types would settle whether
  either is ever picked. [the walk as measured]~~
  **ANSWERED 2026-10-20 (re-geometry):** graphics-formats.md §3.7: type 6 is a quad list and type 7 a quad strip;
  every shipped S3D primitive is type 0, so the odd walk is met only by meshes built at run time (the foundation
  faces, drawn directly, and `AddBoxMesh`, geometry-utilities.md §6.2, neither of them picked). The walk stays as
  measured; view-utilities.md §7.1 and §10 now say so.
