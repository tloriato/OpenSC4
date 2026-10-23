# model-instances: open questions
<!-- Questions about [model-instances.md](../model-instances.md). Append new entries at the END; when one is answered, move it to answered/model-instances.md. Rules: README.md. -->

<!-- from: re-modelinst (2026-10-20) -->
- **MI1 - §6.1:** what the five header values after a sprite animation's type tag, and the value after its five
  zoom keys, hold; nothing in the reader interprets them, and the sprite instance was not traced reading them.
  [kept, not interpreted]

<!-- from: re-modelinst (2026-10-20) -->
- **MI2 - §4:** what `LightModel`'s `flip` argument is for (it negates the rotation used to light the normals) and
  which callers pass true. [as read]

<!-- from: core-model-instances-contractfix4 (2026-10-22) -->
- **MI11 - §§2.1,3.1,4.1 numeric precision:** which x87 precision-control settings must the portable subset reproduce, and where are the binary32 stores in kind-dispatched point transforms, the ray helper and LengthOfLine? The revised contract specifies stores for bounds/score/Advance but says the transient last bits depend on active control. Is binary64 transient arithmetic an accepted portable limit or must 64-significand-bit x87 be emulated? [PARTIAL 2026-10-23 (re-model-instance-numerics4): §2.2 proves kind0–5/default stores, mixed ray transients and unrounded LengthOfLine return, plus conditional PC24 initialization and PC53 parser/Lua writers. N1–N6 independently distinguish modes, storage and thresholds. No blanket f64/64/24 equivalence is justified. WIP: exact entering precision/rounding mode on complete normal native bounds/pick/Advance/processor paths, intervening runtime/external callback preservation and externally supplied CW. Fixed portable modes must remain explicitly portable.]

