# view-utilities: open questions
<!-- Questions about [view-utilities.md](../view-utilities.md). Append new entries at the END; when one is answered, move it to answered/view-utilities.md. Rules: README.md. -->

<!-- from: re-viewutils (2026-10-20) -->
- **VU2 - §1:** what the fields `+0x3DC`-`+0x410` hold; the constructor sets `+0x3F4` and `+0x3F8` to 1 and
  `+0x410` to 0.3, and none of the functions in this spec reads them. [unused here]

<!-- from: re-viewutils (2026-10-20) -->
- **VU3 - §3.1, §5:** whether the camera's view volume is always centred (left = -right, bottom = -top). The
  corners are built from the half-extents around the forward axis, so an off-centre volume would draw where
  the frustum tests do not look. [centred (inferred)]
