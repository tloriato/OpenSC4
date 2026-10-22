# animated-sprite: open questions
<!-- Questions about [animated-sprite.md](../animated-sprite.md). Append new entries at the END; when one is answered, move it to answered/animated-sprite.md. Rules: README.md. -->

<!-- from: re-sprites (2026-10-20) -->
- **AS1 - §2, §4.1:** the layout of the per-zoom sprite-animation record (type `0x69B6F01C`) and of a frame
  entry: which bytes are the page, the frame's pixel position and size, the two flip bits and the offsets, and
  what the animation table at `+0x34` (sequences with a first frame and a length) comes from. [the use as read]

<!-- from: re-sprites (2026-10-20) -->
- **AS2 - §2, §7:** which models are sprite animations (resource type `0x29A5D1EC`) in the shipped data and
  which occupants use them, and whether the swapped-looking names of `0x007FD7D0` / `0x007FED50` reflect the Mac
  build. [the behaviour as read]
