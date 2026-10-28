# network-geometry: open questions
<!-- Append-only engine questions; index regeneration belongs to the coordinator. -->

- **NG15 — §9.2 nonflat straight-run cross-sections and constraint insertion.**
  The spec gives the three-section shape and slope/smoothness equations, but not the
  exact vertex pairs returned by `GetCrossSections` for each side/edge byte, its
  diagonal flag, or insertion/traversal order when adjacent cells share vertices.
  Please specify those details and an admitted nonflat width-one road fixture with
  its per-level constraints and solve result, so these can be distinguished from a
  missing piece/plane input rather than choosing a propagation order to fit a price.
  Blocking case: `gen27-spandau-s1-default`, julian 2452276, road `(65,7)→(69,7)`
  and reverse: native/core return=1/0, valid=5/0, price=144/0 (same ordered cells and
  counts). The vertical five-cell controls return true on both sides, price=66/75.
  Evidence: `build-runs/suite/plans-cities29-2/gen27-spandau-s1-default.log:8936`.
  The current implementation's inferred section mapping is retained; no constraint
  is removed or relaxed. `plans-cities29-r3-spandau` independently repeats these
  differences (clean, ref_warnings=0, nettool=4/7, powerline=7/7), but staged the
  preceding DLL before the new public-terrain diagnostics finished building.
  A run of that diagnostic DLL is still needed for the multi-cell terrain trace.

  Final-round diagnostic fixtures (native/core pairs; no check or tolerance changed):
  `build-runs/suite/plans-cities29-final12/gen27-spandau-s1-default.log:3413`
  reproduces the horizontal refusal and records every cell's terrain, retained
  corners and vertices; vertical controls at line 3435 still cost 66/75.
  Please also provide the ordered constraints and final network corner heights for
  the following nonflat runs, to distinguish cross-section errors from missing
  piece/plane inputs; a shared cause is not assumed:
  - `timbuktu-big-city.log:2990` in the same directory: road `(10,3)` to `(10,7)`
    and reverse both succeed with valid=5, price=52/54; street price=27/28.
  - `gen27-tegel-s0-default.log:3380`: road `(83,79)` to `(95,79)` succeeds/refuses,
    valid=13/0, price=173/100; street succeeds on both, price=86/83.
  - `maxisland-downtown-weburg.log:3029`: street `(33,3)` to `(37,3)` refuses/succeeds,
    valid=0/5, price=0/105; vertical `(33,3)` to `(33,7)` succeeds, price=78/102.
  These fixtures use the restored specified 24-bit earthwork product. The price
  differences therefore remain blocked on admitted multi-cell solver fixtures,
  rather than being resolved by changing rounding or accepting a price tolerance.

<!-- from: network-exact33 R8 (engine lane, 2026-10-28) -->
