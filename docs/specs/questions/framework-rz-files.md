# framework-rz-files: open questions
<!-- Questions about [framework-rz-files.md](../framework-rz-files.md). Append new entries at the END; when one is answered, move it to answered/framework-rz-files.md. Rules: README.md. -->

<!-- from: core-rzfiles4 (2026-10-21) -->
- **RZ1 - §2.5 SetBufferSizes:** (a) is a size of 1 rounded down to 0 (buffer disabled) before the "non-zero
  below 4 becomes 4" rule, or does 1 become 4? (b) negative values other than -1: unsigned (clamped to
  0x1F4000) or something else? The core applies the rules in the order written (1 -> 0) and treats other
  negatives as unsigned. [implemented literally]
- **RZ2 - §2.5 / §2.6 failure paths:** (a) when the buffered ReadWithCount's re-seek or fill fails part-way,
  what is n (the core: bytes already delivered) and are the positions changed; (b) when WriteWithCount's
  move of the OS pointer back to the logical position fails (read buffer discarded), does the write go on or
  return false? The core records the error and goes on. [implemented literally]
- **RZ3 - §2.10 IsFileWritable:** which access / creation / share modes does the trial create of a missing file
  use? The core uses write, create-new, exclusive. [choice]
- **RZ4 - §3.3 lower-casing:** a name with a byte above 0x7E that is not valid UTF-8: how does the UTF-16 path
  convert it (the core decodes lossily, U+FFFD)? And is the wide lower-case the C runtime `towlower` in the
  "C" locale (ASCII only) or the OS full mapping (the core: Unicode single-unit lower case)? [choice]
- **RZ5 - §3.3 entry without `=`:** the key is empty; what is the value (whole trimmed line or empty)? Only
  visible to ReadEntry with an empty key. The core uses empty. [choice]
- **RZ6 - §3.5 ReadNumber overflow:** "follows the C runtime": wrap, clamp to LONG_MAX/LONG_MIN, or other?
  The core wraps modulo 2^32. [choice]
- **RZ7 - §3.4 / §3.6 "closes the file" with leave-open off:** is that the INI Close (slot 5: index and loaded
  flag dropped too) or only the cRZFile Close (index kept while the file is closed)? The core uses the INI
  Close. Also §3.6 step 1: when Open(forWrite) fails for a missing file, is the result false (the core) ?
  [implemented literally]
- **RZ8 - §3.5 ReadBinaryEntry odd length:** "An odd value length fails": the value's own length, or the
  converted count min(length, 2 x size)? (A 5-character value read into a 2-byte buffer.) The core checks the
  converted count in slot 8 and the value length in slot 9. [choice]
