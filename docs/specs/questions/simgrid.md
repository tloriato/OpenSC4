# simgrid: open questions
<!-- Questions about [simgrid.md](../simgrid.md). Append new entries at the END; when one is answered, move it to answered/simgrid.md. Rules: README.md. -->

<!-- from: glue-simgrids4 (2026-10-21) -->
- **SG1 - section 6.3 (with sections 4 and 6.4.5):** a layout-compatible replacement of the grid class
  (`enable = simgrids`, engine/plugin/FC4SimGridLogic.h) keeps `vector<vector<T>>` in the STLport layout with
  every buffer from the STL node allocator B. Two points the spec leaves open: (a) does any original code outside
  `cSC4SimGrid<T>` ever change the vectors' structure through the `GetGridData` pointer (resize, assign, swap,
  `clear`, destroy, or keep a row pointer across a later `SetTractSize` / `Read`), or does it only read and write
  element values within `[begin, end)`? (b) Do original callers ever pass out-of-range indices to slots 7, 8, 22,
  23 or 26 in normal play (section 4: no bounds checks)? The replacement returns 0 for out-of-range reads and
  ignores out-of-range writes [choice]; if an original caller depends on reading a neighbouring row's memory,
  please name it.

<!-- from: verify-simgrids-shadow4 (2026-10-22) -->
- **SG2 - sections 2, 4, 5 and 6.2 (factory pre-initialization contract):** what public operations are valid
  on a factory-created original grid before `Init` and the first `SetTractSize`? In particular, are its geometry,
  initialized flag and `Write` record defined at this point? The spec defines sized grid behavior but does not
  state constructor defaults or the pre-initialization serialization contract. The first verification run
  (`sgv4-r1-native39-20261022a`) raised one foreign exception after the existing 117-grid probe and before any
  fixture result; no causal diagnosis from executable bytes or exception dumps was attempted. [choice] The
  constructed verifier now establishes `Init` and `SetTractSize(8)` before capturing its first input record.
  This setup receives no comparison credit; unsized factory serialization remains unverified. This is a parked
  contract question, not a request for additional RE. SG1 replacement/out-of-range choices remain unchanged.

<!-- from: verify-simgrids-methods-final4 (2026-10-22) -->
- **SG3 - section 4.1 (shipped C++ method differences):** same-process native versus independently owned
  shipped `cFC4SimGrid<T>` objects expose constructed average differences previously hidden by the
  retained-record/Rust transport. In `sgm4-r1-native39-20261022a`, u32/u64/i64 each differ on all ten
  average returns (cell and tract rectangles for S=8,4,3,8,8), and f64 differs on the cell average at S=3:
  31 differences among 600 fixture checks; every complete serialized state comparison is retained.
  Inputs per size: fill7, (1,1)=unsigned max for u32 or 0x0123456789ABCDEF for u64/i64, (0,0)=T(1.9),
  other tracts7; cell rectangle(0,0,2*S,2*S), tract rectangle(0,0,2,2). f64 uses (1,1)=-3 and (0,0)=1.9.
  First u32 average has native low byte00 versus shipped low byte03. Which precise return/conversion
  behavior should the published contract require for these typed cases? Later diagnostic runs retain
  complete scalar bit patterns with op/size/rectangle, not just their first differing byte.
  [choice] Keep the published production implementation unchanged, preserve and count every difference
  under SG3, and grant no exact-average or full-family verification credit. This is a bounded contract
  question, not authorization for another RE round. SG1/SG2 remain open.
  Diagnostic native39-b complete return bits (zero-extended little-endian T): u32 S=8 cell/tract average
  native0x40000000 versus shipped0x40000003; i64 S=8 cell/tract average native0x0048D15A00000000 versus
  shipped0x0048D159E26AF380; f64 S=3 cell average native0x4014E38E40000000 versus shipped0x4014E38E38E38E39.
  The independent normal-speed `sgm4-r1-normal39-20261022b` run repeats the same31 differences, so they are
  not confined to turbo. Its verdict is CLEAN/ref0; native natural scalar/full-buffer comparisons remain exact.
