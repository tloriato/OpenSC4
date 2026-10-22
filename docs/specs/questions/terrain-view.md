# terrain-view: open questions
<!-- Questions about [terrain-view.md](../terrain-view.md). Append new entries at the END; when one is answered, move it to answered/terrain-view.md. Rules: README.md. -->

<!-- from: re-terrainview (2026-10-20) -->
- **TV1 - §4.1:** what the draw context's `+0x50` holds when `Update` runs (0 selects the land layers, anything
  else the water layers) and who sets it; the view registers itself twice with the draw registry, at layers 0
  and 2 (terrain.md §15.5.4). [the draw registry's current pass, inferred]

<!-- from: re-terrainview (2026-10-20) -->
- **TV2 - §1, §8.2:** which pipe-coverage state each plumbing colour (`0x497F42A4`, `0xE97F42B0`, `0xA97F42CE`,
  `0x097F4C4E`) stands for, what the flag `0x297F42B7` switches, and what `0x29AB15C7` (0.9) weighs in the cliff
  colours. [the values as shipped]

<!-- from: re-terrainview (2026-10-20) -->
- **TV3 - §9.3:** what the object at `+0x74` is: it is registered at draw layer 5 next to the ripples, listens to
  `0x89171496` and sits on the animation tick manager (its shutdown is `0x00762300`). [a second animated water
  effect, inferred]
