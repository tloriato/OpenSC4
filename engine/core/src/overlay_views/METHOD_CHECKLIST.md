# Public owner method checklist

Sources: overlay-views.md §§1–3; network-overlay-views.md §§1–2. Core only, unbound.
The six exact owners are below. The brief's 54-function inventory is not a public
method list; OVV6 requests the missing catalogue. No complete-owner credit yet.
Terrain-view §8 describes terrain-manager methods, intentionally outside this module.
No terrain replacement or TR33–TR37 contract is changed.

| Owner | Published method | Implementation / remaining contract |
|---|---|---|
| cSC4PathView | Init | Semantic plain/textured states; lifecycle OVV6 |
| cSC4PathView | SetThickness | Stores every binary32; checked draw supports positive finite geometry |
| cSC4PathView | SetPathTexture | Selected record release/null/bind/repeat, including load failure; no global capture |
| cSC4PathView | NewPath | Binary32 priority, equal-newest-first, unordered continues; packed colour/flags; unset repeat |
| cSC4PathView | AddPathVertex | Strict binary64 1e-5 x-z test, equality/NaN append; sentinel no-op |
| cSC4PathView | ClearPaths | Drops owned resources; stale selection retained as safe logical token |
| cSC4PathView | endpoint / joint helpers | Published handedness, squared miter and unscaled small-q fallback |
| cSC4PathView | DrawPath | Locally owned ribbon, 3D longitudinal projections, joint offsets; unset repeat remains unsupported |
| cSC4PathView | DrawArrowhead | Local triangle, 3D threshold, 4.5t cap, start-before-end shortening |
| cSC4PathView | Update | Ordered draws, identity, offset 15/restoration, texture wrapping/plain restoration |
| cSC4PathView::PathInfo | creation / storage / lifetime | OVV2 record fields; no dangling dereference; full public surface/lifecycle OVV6 |
| SC4MarkedCellView | SetCellSize | Stores every float and inverse; unchecked invalid Draw is outside portable domain |
| SC4MarkedCellView | SetShouldIgnoreDepthBuffer | Flag and semantic state |
| SC4MarkedCellView | SetShouldUseWaterAltitude | Water-max before frustum in both branches |
| SC4MarkedCellView | SetCellColor | Blocked OVV4; byte helper does not claim packing |
| SC4MarkedCellView | Init | Semantic state, foreign filter; published mask 0x1F4F exposed, native allocation remains unbound |
| SC4MarkedCellView | DrawCells | OVV5 corner/diagonal order, grid clamp before scale, lower conversion, water/frustum/lot order; ≤1024 vertices/batch |
| SC4MarkedCellView | Update | Identity, offset 4/restoration, normal then hidden additive/greater pass |
| cSTEVertBufExtn | GetVertexBufferExtension | Bool 25 gate, IID/format, validation/release, exact row capacity without subtraction |
| cSTEVertBufExtn | CreateBuffer | Explicit lock; reset counts only on success; no implicit flush |
| cSTEVertBufExtn | row-copy helper | Two compact rows of n+1 raw 32-byte vertices |
| cSTEVertBufExtn | SetIndexBuffer | Borrowed u16 row-pair source, byteFlag address rule |
| cSTEVertBufExtn | DrawPrimsIndexedUsingVertBuf | Modulo u16 rebase; vertex-only flush, index growth, zero-n copies two, no lazy lock |
| cSTEVertBufExtn | FlushBuffer | Null no-op; unlock, clear pointer, draw /3; ignore raw driver draw failure |
| cSTEVertBufExtn | ReleaseVertexBufferExtension | Releases only interface; retains counts/lock token; allocation cleanup emits no unlock/draw |
| SC4PathInfoView | Init | Semantic state; fields/format NOV2 |
| SC4PathInfoView | Update | Pulse, cell/occupant traversal, byte filter, geometry; colours NOV2 |
| SC4PathInfoView | path draw | Cap/height, known colours, cross, arrow/bars; details NOV2 |
| SC4TrainJunctionView | Init | Blocked NOV3; resource helper does not replace Init |
| SC4TrainJunctionView | GetPathBitmap | Explicit entry direction, chord classifier, literal keys; details NOV3 |
| SC4TrainJunctionView | GetArrowBitmapQuad | Blocked NOV3 |
| SC4TrainJunctionView | DrawTrainJunction | Blocked NOV3; transparency helper implemented |
| SC4TrainJunctionView | Update | Own traversal; fails NOV3 even for empty frames |

Rust constructors/snapshots/explicit-data helpers are core APIs, not invented native
slots. No guessed packed state/default colour/key names/line format/transform/model
choice for the excluded owners is published. Provider outputs are foreign primitives only.

OVV2/3/5/7 finite algorithms are supported within checked backing/capacity domains.
OV-L1..6 and L8..13 have independent portable fixtures; original execution remains
PROPOSED/UNEXECUTED. No universal x87 bit identity, unchecked/OOM/reentry behavior,
OVV4 independent outline/remaining colour APIs, OVV6 inventory/lifecycle completeness,
NOV2/3, region, terrain-manager, native/runtime/default or whole-owner credit.
