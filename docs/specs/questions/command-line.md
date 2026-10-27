# command-line: open questions
<!-- Questions about [command-line.md](../command-line.md). Append new entries at the END; when one is answered, move it to answered/command-line.md. Rules: README.md. -->

<!-- from: re-cmdline (2026-10-19) -->
- **CL1 - §3:** slot 16 of `cRZCmdLine` (`0x00913E3B`, argument `i` as a `cRZString const&`, or the empty string)
  has no direct caller in the executable. Is it part of the published `cIGZCmdLine` interface, so plugins can call
  it, or a virtual of `cRZCmdLine` only? (Slot 15 is the deleting destructor, so slot 16 follows the interface.)
  **PARTIAL 2026-10-27:** The body at 0x00913E3B and its sole vtable reference at 0x00A82A60 confirm the concrete slot. QueryInterface at 0x00913E16 returns the same object for the command-line IID, but this cannot establish which slots its published declaration promises. Publication/plugin use remains unresolved.

<!-- from: core-cmdline4 (2026-10-21) -->
- **CL3 - §3 "any case", §4:** for bytes 0x80-0xFF, does the "any case" comparison (`MakeLower` / `tolower`, and the
  single-letter compare of slots 10 and 12) leave them unchanged, or does the game's locale lower some of them (for
  example Latin-1 `É` 0xC9 to `é` 0xE9)? (FreeCity4 core lowers `A`-`Z` only.)

<!-- from: verify-cmdline-native4 (2026-10-22) -->
- **CL4 - §1.1-§1.2, §3: owned original construction ABI and storage:** what published binding safely creates
  an independent original `cRZCmdLine` from caller-selected ASCII text, and an owned copy from `cIGZCmdLine const&`?
  The spec supplies size `0x3C`, constructor addresses `0x00914E14` / `0x00914A30`, nondeleting destructor
  `0x00419490`, initial reference count zero, and deleting-destructor behavior. `docs/abi/allocators.md` also
  supplies global-heap allocate/free entry points. However, no command-line-specific direct-call contract was
  found in `docs/abi/direct-calls.md`: please publish the exact constructor/copy/nondeleting-destructor calling
  conventions, receiver and argument placement, stack cleanup and return channels; required storage alignment
  and initialization; whether caller-owned storage is supported; and the matching destruction/storage-release
  sequence, including whether any reference may be taken before nondeleting destruction. Alternatively, publish
  a supported original factory/service CLSID/IID, typed creation/copy/parse methods and ownership contract.
  The SDK's `cIGZFrameWork::CommandLine()` exposes the process object, not an owned parse/copy facility; it must
  not be mutated to supply fixtures. Its `cIGZCmdLine` declaration also orders the switch overloads differently
  from §3, so that declaration alone is not a verified original dispatch binding. [No original construction,
  borrowed-object mutation, guessed private allocation or SDK substitute attempted; constructed 0/0 and natural
  0/0 comparisons, no verification credit. CL1-CL3 unchanged; stop pending the explicit binding contract.]
