# renderer-driver: open questions
<!-- Questions about [renderer-driver.md](../renderer-driver.md). Append new entries at the END; when one is answered, move it to answered/renderer-driver.md. Rules: README.md. -->

- **RD3 - section 6:** the unknown mode-record fields (+0x10..+0x1F, the "has stencil" byte +0x21 and who sets it,
  byte +0x22, the 14th stored dword copied from object `+0x274`), whether `0x00887D80` is a plain append, and the
  contracts of `SetViewport` (74 with four arguments, 75 with none) and `GetViewport` (76).
  *Partly answered 2026-10-21 (re-driverdraw4): slots 74-76 are §11 (open there: the writer of byte `+0x261`);
  the mode-record fields are still open.*
  *Further narrowed 2026-10-23 (re-gdriver5): §14.4 specifies mask fields, private selector dword and capability
  writes, and identifies append as vector insertion. Still open: full meanings of descriptor capability bytes,
  especially +0x22 and +0x2C, and every format-enumeration outcome.*
