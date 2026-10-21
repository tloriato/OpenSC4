# command-line: open questions
<!-- Questions about [command-line.md](../command-line.md). Append new entries at the END; when one is answered, move it to answered/command-line.md. Rules: README.md. -->

<!-- from: re-cmdline (2026-10-19) -->
- **CL1 - §3:** slot 16 of `cRZCmdLine` (`0x00913E3B`, argument `i` as a `cRZString const&`, or the empty string)
  has no direct caller in the executable. Is it part of the published `cIGZCmdLine` interface, so plugins can call
  it, or a virtual of `cRZCmdLine` only? (Slot 15 is the deleting destructor, so slot 16 follows the interface.)
