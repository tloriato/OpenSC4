# overlay-views: open questions
<!-- Questions about [overlay-views.md](../overlay-views.md). Append new entries at the END; when one is answered, move it to answered/overlay-views.md. Rules: README.md. -->

<!-- from: re-overlayviews (2026-10-20) -->
- **OVV1 - §1.1, §2.1:** which lot flags `0x304` (marks raised 15.5 m) and `0x100000` (marks hidden unless the lot
  says otherwise) stand for, and where a path's texture repeat factor r (path record `+0x38`) is set. [as read]
  **Partial answer 2026-10-23 (re-overlay-views-contracts4):** §1.2 establishes the second binary32
  SetPathTexture argument as the repeat source; NewPath does not initialize a default. Lot-flag
  identities remain open, so this bundled entry is retained here.

<!-- from: core-overlay-views4 (2026-10-22) -->
- **OVV4 - §2, SetCellColor and marked-cell state:** publish native r/g/b/a input types/ranges,
  byte conversion/rounding/clamping and packed byte order, default/common/per-cell and outline colours,
  and public cell mutation signatures. Can the outline pass be disabled independently of prefer-opaque?
  [Core retains explicit RGBA byte channels with no packed representation claim; SetCellColor(float...)
  fails with PackedColourOvv4. Explicit byte/replace_cells helpers do not claim native API equivalence.]

  **Native evidence 2026-10-22 (re-overlay-views-contracts4):** §2.2 now proves float-vector input,
  bit-based conversion without clamping, common/per-cell/default/outline state and three unnamed mutation
  wrappers. Residual: identify/prove an independent outline-disable API, or prove its absence across
  the complete owner inventory; constructor-disabled and prefer-opaque suppression alone do not answer it.

<!-- from: core-overlay-views4 (2026-10-22) -->
- **OVV6 - owner catalogue and lifecycle:** the task inventory says 54 functions across cSC4PathView,
  cSC4PathView::PathInfo, SC4MarkedCellView, cSTEVertBufExtn, SC4PathInfoView and SC4TrainJunctionView;
  current specs name a subset and ABI rows do not identify usable vtables. Publish the complete public
  method/address checklist: constructors/destructors, init/shutdown, getters/setters, bounds/transforms
  and reference ownership/failure semantics. [Core checklist limits coverage to published methods;
  Rust RAII does not claim native AddRef/Release or unpublished lifecycle. All six owners are withheld
  from complete-owner/runtime promotion.]

  **Native evidence 2026-10-22 (re-overlay-views-contracts4):** §3.2 lists all 49 currently named
  non-template entries plus three unnamed marked-cell wrappers, all four five-slot primary interfaces,
  and the proved/residual scope by owner. The claimed 54-entry inventory is not reconciled. Shared
  QueryInterface/AddRef/Release dispatch, zero-release destruction and remaining setters still need proof.
  **Qualification 2026-10-23 (re-overlay-views-contracts4):** §3.4 now independently proves the shared
  ID-1 QI, u32 AddRef/Release and constructor-table-to-deleting-body chain for all four views,
  including zero-count deletion and train deletion without Shutdown. These portions are supported;
  OVV6 remains partial for historical inventory membership, additional public setters/interface
  completeness and caller Shutdown discipline. Two newly traced train cleanup addresses do not by
  themselves identify the historical 54 entries. §3.3 supplies the per-owner method ledger.

<!-- from: core-overlay-views-contracts4 (2026-10-23) -->
- **OVV8 - §7.1 OV-L10 source extent:** L10 submits [1,3) (n=2), but lists only six source words and six
  appended indices. §3.1 requires 6n=12 words/indices. Are the six printed entries a prefix, or should
  this discriminator use a one-cell range? [Portable fixture treats the published six values as the
  specified prefix, supplies six explicit zero words as backing, and asserts all twelve are consumed.
  The six published rebased values remain unchanged; no original execution is claimed.]
