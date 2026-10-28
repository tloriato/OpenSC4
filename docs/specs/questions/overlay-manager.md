# overlay-manager: open questions
<!-- Questions about [overlay-manager.md](../overlay-manager.md). Append new entries at the END; when one is answered, move it to answered/overlay-manager.md. Rules: README.md. -->

<!-- from: re-overlays (2026-10-20) -->
- **OV1 - §3:** the layout of the nine-float placement record (slots 12-14) and the last arguments of the add
  calls (slots 3-6, 9-11); a centre, extents and an orientation are inferred from how `PlaceOverlay`
  (`0x0073A1B0`) turns it into a cell rectangle and a texture matrix. [the record as copied]

  Owner resolution (2026-10-22, re-overlays-final4): §§3.2/3.2.1/3.5/3.7 give all nine placement fields,
  ordinary/round/key-shadow add arguments, and all six model-add arguments with direct record construction,
  fallback bounds and ordered post-transform equations. Exact residual is **outside owner**: lighting slot 17
  `0x007D8C80` source-geometry/projection-descriptor layout and primary projection/pair production, plus slot 18
  `0x007D6CC0` high-quality terrain bounds/precision. The owner forwards those inputs; their full producer
  algorithms are not established by ordinary placement. Parked at this final round; no further round requested.

<!-- from: core-terrain-overlays4 (2026-10-22) -->
- **OV7 - §§3,4 slots21/Draw:** specify the placement-to-world/texture equations, matrix axis/storage/composition conventions, slot21 matrix entries and behavior for size=0, and camera transform inputs/precision. The public text gives semantic scales 1/size and -1/size with second-axis offset 1+offset/size but not their world matrix positions. [core stores those semantic coefficients only; no externally computed owner matrices accepted; full world transform deferred with OV1]

  Partial resolution (2026-10-22, re-overlays-contract4): §3.2/§3.2.1 specify row-vector placement coefficients;
  §3.4 specifies all slot21 entries, FP behavior, no group recursion and redisplay. §4.7 proves post-reset context
  matrix input, binary32 storage, C×M order and differing dot-product orders. Still open: complete inverse
  cofactor/spill behavior and all sixteen output accumulation orders. Final-round §§4.7/4.8 now specify
  reciprocal binary32 store, no singular guard, row-vector shadow stages and exact normal/debug branch
  selection. Remaining native intervals: `0x00738ADA–0x00739020` cofactor/spills and all-entry dot-product
  accumulation in `0x00736790`, `0x00736BF0`, `0x00737870`. Parked; no further round requested.

<!-- from: core-terrain-overlays4 (2026-10-22) -->
- **OV8 - §§2.2,4:** specify rectangle endpoint conventions, clipping/margin and cell-index insertion/update/removal ordering; decal/shadow classification (bit4 is described as both decal/shadow-part and decal eligibility); shadow selection/strength minimum, alpha-test equation, second transform/height coordinate, DrawRect land/water/dynamic array input formats, GPU restoration and counter reset/increment points. Does empty or skipped Draw preserve last-frame stats? [core uses explicitly internal half-open rectangles and sorted/deduplicated selection only; no host draw result, fabricated successful GPU pass or guessed stats; normal Draw reports a gap]

  Partial resolution (2026-10-22, re-overlays-contract4): §§2.1–2.3 and §3.2 distinguish shadow bit3/decal bit4,
  half-open bounds, floor/ceil conversion without a margin, strict separation versus touching-edge clipping,
  native index traversal/prepend/removal and same/changed-rectangle redisplay. §4.4 proves selection-empty,
  pass-disabled and snapshot counter preservation, nonempty pass resets, masks and increment-after-submission.
  §§4.5/4.6 now prove visible-row endpoint arithmetic and native encounter order, stamp wrap without reset,
  cell-to-vertex rectangle dispatch, exact zero minimum/alpha reference and bounded exit-state actions.
  Final-round §4.8 proves normal/debug stage selection, conditional location-strength rejection (including
  skipped counter increment), the midpoint lighting input, and exact water slot-53 return comparison/routes.
  There is no proved extra direct terrain-height sample. Still open: outside-owner vertex/index consumers
  `0x007541C0`, `0x007543C0`, `0x007545C0`, `0x00754910`, `0x00754C60`, `0x00754F10`, `0x00755560`,
  `0x00755700`; terrain slot 53's internal meaning; lighting slot 15 `0x007D6BB0` strength interpolation and
  terrain inputs. Composition precision is OV7. No guessed TR33–TR37 contract or complete renderer is claimed.
  Exact residuals parked at this final round; no further round requested.

<!-- from: core-overlay-manager-close5 (2026-10-23) -->
- **OV10 - §§1.1,3.1,3.2,3.7 exceptional numeric domain:** what are the cell-conversion results and ordered mutation prefix for zero/nonfinite resource aspect or placement size, nonfinite/out-of-i32 bounds, negative/inverted round extents, and invalid terrain dimensions? The published floor/ceil contracts qualify ordinary finite coordinates; they do not establish exceptional conversion or allocation behavior. Also specify required precision for rotated sine/cosine under caller FP state. [core implements ordered rotated coefficients with portable f64 intermediates and binary32 stores; rejects unconvertible bounds instead of fabricating saturated rectangles; invalid backing handles/services remain explicit safe-Rust errors, not native validation claims]
