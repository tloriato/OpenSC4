# audio-scape: open questions
<!-- Questions about [audio-scape.md](../audio-scape.md). Append new entries at the END; when one is answered, move it to answered/audio-scape.md. Rules: README.md. -->

<!-- from: re-scape7 (2026-10-23) -->
- **SCP1 - section 6.2:** nothing in the owner adds to the pending-cluster list `+0x58`, so the countdown branch of
  `OnTick` never runs. Is there a caller outside the audio owners that fills it, or is it dead? [dead]
  **PARTIAL 2026-10-27:** The constructor creates an empty pending list (0x004F6140), and the service tick consumes it using the base+4 view (0x004F67C0). No enqueue path was established. Owner-local absence does not prove that every outside caller leaves it empty; retained open.

<!-- from: re-scape7 (2026-10-23) -->
- **SCP2 - sections 3.2, 4.3, 5.1:** no caller found sets flag bits `0x4` (untracked members) or `0x10` (keep
  clusters on clear); every scape seen has flags 0 or 1. Do any data-driven scapes use them? [not used]
  **PARTIAL 2026-10-27:** CreateScape accepts flags directly (0x004F64F0); there is no resource decoder here (§3.5). Whether installed data or external clients request bits 0x4/0x10 remains unresolved without identifying that producer.

<!-- from: re-scape7 (2026-10-23) -->
- **SCP3 - section 1:** what is the third interface at `+0x08` (vtable `0x00A942F8`, slot 3 `0x004F5200` returning
  whether its argument is non-zero), which `QueryInterface` never returns? [an unused callback interface]
  **PARTIAL 2026-10-27:** Slot 3 is a nonnull-argument test (0x004F5200), and QueryInterface exposes only the primary and system-service views (0x004F5210). Construction uses the same abstract four-slot table as message targets (0x004F6140, 0x00968773), but that shared layout alone does not establish its intended interface identity or a subscriber; retained open.
