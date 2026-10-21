# paths: open questions
<!-- Questions about [paths.md](../paths.md). Append new entries at the END; when one is answered, move it to answered/paths.md. Rules: README.md. -->

<!-- from: re-paths (2026-10-19) -->
- **PA1 - §4:** the four replacement function pointers at `0x00B6291C`, `0x00B62920`, `0x00B62924` and `0x00B62928`
  (find first, find next, create directory, remove directory): nothing in the executable writes them. Are they part
  of a framework hook a plugin can set (for example for wide-character paths), or dead code?

<!-- from: re-paths (2026-10-19) -->
- **PA2 - §3.5, §4:** which callers pass `includeTop` false to the recursive listing, and do mods rely on the plugin
  loader's directory order, which is the file system's find order (§4)?

- **PA3 - section 8 (PR integration review):** paths.md section 8 test 2 lists two outputs for the identical input `C:/`. Section 1.2 step 5 explains the doubled separator. Confirm which overload each vector exercises before implementation; do not guess.
