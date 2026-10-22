| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| core-scene-renderer4 (first portable cSC43DRender owner) | Codex / `core-scene-renderer4` (engine lane, detached HEAD) | in progress; owner denominator 130 functions; exact body bytes unknown; ABI unchanged | (1) Baseline and inventory. (2) Commit independent fixtures. (3) Implement bounded production algorithms and verify. |

## Handoff
- Goal: first bounded production scene renderer from render3d sections 1–12 and view3d sections 3–4.
- Done: reused reserved detached worktree; clean initial status; read lane rules and published renderer sections. Independent literal fixtures committed before production implementation (D1–D6, B1–B4, queue boxes, stable duplicates, S1–S6).
- Done: first substantive production module implements owned dirty/list/backing algorithms, typed partial model queue, bounded shake state. Full draw/lifecycle and missing geometry remain Unsupported. Source not yet checked.
- Done: discriminating tests now cover dirty ties/penalty/deadline/threshold/area boundary, lists, backing failures/shared ownership, queue kinds/refs/box policy, typed record partial failures and bounded shake cases. Added typed camera reference operations and actual existing camera-control forwarding. RND5–RND7 appended. Rust sources formatted via rustfmt stdout applied with apply_patch.
- Next: collect initial all session 3939 before starting quick; fix compiler/test/clippy failures, then serial final all and absolute touched-file fmt. Baseline compile took 18m16s; initial test log has progressed (do not overlap).
- Scope: only new scene_renderer module/tests, additive lib declaration, this report, appended render3d questions.
- No native/game/ABI/glue work; no whole-owner or byte-total credit; section 7 residual remains unsupported.
- Required fixtures before implementation: D1–D6, B1–B4, model queue, stable duplicates, S1–S6.
- Questions retained: RND1, RND2, RND4; next available RND5.
- Baseline: initial all launched in session 3939. Existing cargo PID 6076 has 131 handles; Windows denies command-line/path attribution, no pre-existing qbuild logs in this lane. No processes killed. No game requests.

## Initial named-method inventory (before implementation)

Denominator: **130 owner functions**, including folded bodies. Exact implemented body-byte subtotal **unknown**. Interface slots are not unique bodies (slots 18/19 share one folded body); no 130/130 claim can be made. The publication does not enumerate every physical body by name/address, so unnamed residual members of that denominator stay uncredited.

| Published methods / distinct overloads | Specification status | Initial implementation scope |
|---|---|---|
| QueryInterface; AddRef; Release; constructor; GetGZCLSID | §1 algorithms, destruction body absent | Typed identity/count; last Release reaches explicit Shutdown boundary before destruction |
| Write; Read | §2 record, RND1 unresolved native slot name | Portable typed record only; version and partial-read ordering, no native wire binding |
| SetMapScaleFactor; GetViewUtilities; GetLightingManager; GetDrawContext; GetViewportSize; SetViewportSize; SetSubpixelImageBias; GetCamera; SetCamera; GetCameraControl; SetCameraControl | §3 specified | Local state/reference operations, reuse existing camera/view utilities within their domains |
| GetProjectionMatrixEntries; GetViewMatrixEntries (same folded body) | view3d slot names only | Unsupported |
| SetZoom; ZoomIn; ZoomOut; SetRotation; RotateLeft; RotateRight; SetZoomAndRotation | §3 forwarding only | Preserve camera-control domain; no new camera math |
| Set3DCameraView; Set2DCameraView; SetUICameraView; BorrowRenderSystem; ReturnRenderSystem | §3 specified primitive effects | Bounded state/effect operations; no native service mutation |
| GetViewFogEnabled; SetViewFogEnabled; GetLightingEnabled; SetLightingEnabled; SetViewportClearColor; EnableBackgroundPicture; GetBackgroundPicture; EnableTransparentBackground; GetBackingStoreEnabled; SetBackingStoreEnabled; GetShakeOffsets | §3 specified | Local state and explicit shared backing state; RND2 retained |
| ForceFullRedraw; DoFullViewUpdate; ClearDirtyRectangleQueue | §§4,6 specified | Coalescing and owned queue effects; existing frustum helper restrictions propagate |
| UpdateViewportRect; DoPartialViewUpdate; ProcessDirtyRectangleQueue; DoViewCleanup | §§4,6 specified outer policy, own draw/clip helpers absent | Partial/Unsupported at first missing helper, no callbacks returning renderer outcomes |
| AddDirtyRectangle(rect); AddDirtyRectangle(box); AddDirtyRectangleInternal; signed rect filter 0x007C2C70 | §6 specified | Owned merge/filter algorithm; existing projection only |
| ModelShouldGoIntoBackingStore; queue-item copy 0x007C2E50; ProcessModelChangeQueue; IsModelInserted; InsertModel; RemoveModel; MoveModel; ChangeModel; UpdateGridElementObjectFlags; MoveGridElement; SetQuadrantFlagsForElement | §5 specified queue policy; flag derivation/grid geometry partial | Owned typed queue and decision effects; missing geometry/flag derivation remains explicit |
| StartScreenShake(parameters); StartScreenShake(overload); StopScreenShake; SetShakeOffsets; node-copy 0x007C63A0 | §§6,10 specified | Owned list, start/stop, seed-88 random table, bounded exact arithmetic; RND4 fade=0 choice; extended sine domain unsupported |
| AddViewObject; InsertIntoViewObjectList; FindInViewObjectList; FindViewObject; RemoveViewObject; list clear 0x007C3B60 | §11 specified | Stable unsigned lists, duplicate refs, first removal per list, null terrain quirk |
| IsModelVisible(model); IsModelVisible(element) | §11 routing, existing helper restrictions | Reuse current view utilities only |
| PickTerrain(ray); PickTerrain(screen); Pick(ray); Pick(screen) | §11 specified selection/routing but own ray-container helper absent for Pick | Terrain routing may be bounded; full Pick stays partial |
| PickTerrainAlways(ray); PickTerrainAlways(screen); point insert 0x007C6EC0 | §11.6 outline | Unsupported; no invented ray clip |
| CreateBackingStore; DestroyBackingStore; SaveBackingStore; RestoreBackingStore(rect); RestoreBackingStore(); ReevaluateBackingStoreObjectFlags | §12 specified | Explicit shared buffers and owned clipping/probe/gates; raw driver primitives only |
| Init; ReadProperties; Shutdown | §9 outer sequence, grid/destruction/static service dependencies partial | Never-initialized no-op paths; full lifecycle Unsupported at missing own helper |
| DrawPreDynamicView; DrawPostDynamicView; DrawPreStaticView; DrawPostStaticView | view3d §3.3 calls specified, slot interpretation inferred | No native binding or invented pass contract |
| DoMessage; Draw; DrawStaticView; DrawDynamicView | view3d §§3–4 outer policy partial, own helpers missing | Explicit Unsupported, no unexecuted plan promoted as complete Draw |
| SetGraphicSystem; SetGDriver; StaticSetGDriver; StaticShutdown; destructor body; deleting destructor | §7 residual / outline | Unsupported |
| DoTranslatedViewUpdate; DoViewZoomChange3; DoViewRotationChange2; AttemptTranslatedViewUpdate; UpdateVisibleTerrainMetrics | §7 residual | Unsupported |
| WorldToContainerRect; GetViewFrustumContainerRect; GetDrawFrustumContainerRect; GetVisibleContainerRect; GetViewportRayContainerRect; GetModelContainerRect; CreateQuadGrid; RebuildQuadGrid; SetClipRect; UnsetClipRect; SC4ComputeViewportZProjection | §7 residual / call sites | Unsupported |
| ClearViewport; ClearViewportWithSolidColor; DrawViewportRectangle; DrawTexturedViewportRectangle; SetBackgroundPicture(two overloads); TakeSnapshot(two overloads); GetStatsString1; GetStatsString2; TurnOnLighting; TurnOnFog; CreateMaterialVariant | §7 / view3d slot names, not complete bodies | Unsupported |

This inventory distinguishes method algorithms from raw foreign model/QI/world-box, grid, buffer, draw-context writes, notification, clock, property and reference primitives. It grants no byte credit and no whole-owner/native equivalence.
