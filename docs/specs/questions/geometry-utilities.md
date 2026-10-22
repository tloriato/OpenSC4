# geometry-utilities: open questions
<!-- Questions about [geometry-utilities.md](../geometry-utilities.md). Append new entries at the END; when one is answered, move it to answered/geometry-utilities.md. Rules: README.md. -->

<!-- from: re-geometry (2026-10-20) -->
- **GU1 - §4.4:** Does any caller of `RaysIntersect2D` pass a null output, so that the direction-difference test is ever used? The polygon clipper always passes one; the eight calls in `0x00752FC3`-`0x0075530A` were not traced. [the rule is specified as read either way]
