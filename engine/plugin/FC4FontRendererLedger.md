# Constructed small-renderer literals

Authored before running the production adapter or original renderer. Source:
font-renderer.md §§2–6. This is a raw primitive plugin target, never the original
buffer owner. No source-read or same-value-store observability is claimed.

All target storage starts with byte A5 unless a stated initial pixel is supplied;
16-byte prefix/suffix guards are also A5. Expected unmodified bytes stay A5.
Target callback tuples are (virtual byte offset, argument/result words).
Raster callbacks: width 24, height 28, depth 40, format 38 only for mode 3,
then pitch 8C and base 88 only for a nonempty supported-depth raster.
Lock/unlock: 18/1C, flags 8080, returned AL 0 or 1 as supplied.

| Fixture family | Literal expected pixels and independent rationale |
|---|---|
| threshold, depths 16/24/32, coverages 0,127,128,255 | A5A5,A5A5,CDEF,CDEF or A5A5A5A5,A5A5A5A5,89ABCDEF,89ABCDEF; strict >127 |
| palette, depths 16/24/32, coverages 0,1,128,255 | A5A5,5678,CDEF,3210 or A5A5A5A5,12345678,89ABCDEF,76543210; explicit 256 entries |
| format 7, D=0 F=FFFF, coverages 0,1,7,255 | 0000,F000,F000,FEEE; q=0 forces alpha, q=31 leaves 14 |
| formats 4/6, D=0 F=7FFF, coverage 255 | 7BDE; three five-bit components become 30 |
| formats 4/6, D=8000 F=0, coverage 1 | 0000; high bit cleared |
| format 5, D=0 F=FFFF, coverage 255 | F79E; green at bit 6, bit 5 absent |
| format 5, D=0020 F=0, coverage 1 | 0000; low green bit discarded |
| asymmetric format 4 D=0443 F=1467 a=64 | 0844; components (1,2,3) toward (5,3,7), q=8: (2,2,4) |
| dword D=0 F=FFFFFFFF, a=128/255 | 80808080/FEFEFEFE (published packed arithmetic) |
| dword D=11223344 F=FFEEDDCC, a=128 | 88888888 (published §6.2 literal operands) |
| dword a=0 D=11223344 | 11223344 unchanged; store itself unobservable |
| mode 4 depth16, full coverage | A5A5 unchanged; default format3, no format callback |
| 5x3 clip: target8x6 pitch36, dest(-2,-1,7,5), clip(1,1,6,4), src pitch12 | source27..31,39..43,51..55 filled 255; 89ABCDEF at bytes40..59,76..95,112..131 only |
| fresh origins | first dest(-1,-1,1,1) clips to pixel(0,0), source13=0 preserves A5; second dest(1,0,2,1) restarts at source0=255 and writes 89ABCDEF; copied width/height (1,1) do not restrict the preceding 5x3 case |
| zero count null records/target; character1F | true, zero callbacks, no pixels |
| empty/reversed/outside, unsupported depth8 | width,height,depth then format only for mode3; no pitch/base |
| odd pitch16=11 / depth24,32=19 | row stride10 /16; unchanged padding |

Lock modes 1,2,3,4 must all ignore font/mode and forward 8080, preserving AL.
Repeated load/presave/shutdown phases are repetitions, not new algorithms.

Ledger correction before native execution: the first bridge run rejected two
dword fixtures because F=FFFFFFFF was accidentally used for §6.2's asymmetric
FFEEDDCC case. The fixtures now keep those published operand pairs separate;
the expected results are unchanged. No production-derived expected value added.
