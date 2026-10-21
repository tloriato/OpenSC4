# timers: open questions
<!-- Questions about [timers.md](../timers.md). Append new entries at the END; when one is answered, move it to answered/timers.md. Rules: README.md. -->

<!-- from: re-timers (2026-10-19) -->
- **TM1 - §2.2, §2.4:** what is the framework timer service (`0xA76C78E7`, interface `0x276C784B`) that
  `cRZCallbackTimer` asks for, and the built-in default object at `0x00B62AB0` (built by `0x009335F4`) it falls back
  to? Which thread calls the handler, and what do the fourth argument of the service's slot 3 (1 for `StartOnce`, 0
  for `Start`) and the fifth (always 0) mean?
