# geometry-utilities: answered questions

<!-- from: core-geometry-utilities4 (2026-10-22) -->
- **GU4 - §6.4 text texture failures:** Please specify size/rect mutations on failed surface creation/upload, font-mode restoration timing on each failure, surface/font ownership and release order, text width/height integer types, dimension overflow/negative-size handling, and any surface row pitch. [Pure core requests a packed BGRA surface through a host trait, restores font mode after drawing/on surface failure, leaves outputs untouched before upload, writes size/rect after upload even when upload fails, and rejects unrepresentable dimensions. Actual font/surface ownership is delegated to the host; no native lifecycle claim.]

  **ANSWERED 2026-10-23 (re-geometry-contracts4):** DIFFERS — [geometry-utilities.md](../../geometry-utilities.md)
  §§6.4.1–6.4.2 specify the native owner call sequence: no font returns false without mode/output changes;
  once a font is obtained it is AddRef'd, mode 3 remains through drawing/upload, and missing GS or failed
  surface factory/create/upload joins restore-mode → write size/rect → release nonnull surface → release font.
  Thus a supplied 30×12 measurement and absent GS still returns false with size 32×16 and rect (32,13,2,1).
  String length and dimensions are 32-bit integers with wrapping additions/doublings and signed comparisons;
  no negative/overflow validation is performed, and some targets never terminate. Plain upload uses supplied
  byte pitch divided signed by bytes-per-pixel; compressed source stepping derives from pitch unsigned>>2.
  Helper access/cleanup order and valid normal domains are explicit. These are owner-issued reference/access
  calls, not claims about foreign font/surface implementations, device results or general destructor behavior.

<!-- from: re-geometry (2026-10-20) -->
- ~~**GU1 - §4.4:** Does any caller of `RaysIntersect2D` pass a null output, so that the direction-difference test is ever used? The polygon clipper always passes one; the eight calls in `0x00752FC3`-`0x0075530A` were not traced. [the rule is specified as read either way]~~

**ANSWERED 2026-10-27 (re-q-framework):** MATCHES — All ten direct callers supply nonnull hit storage; none uses the direction-difference branch. No address-taking reference was found (§4.4).
