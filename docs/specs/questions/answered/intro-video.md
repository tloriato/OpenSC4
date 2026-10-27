# intro-video: answered questions (archive)

<!-- from: re-introvid7 (2026-10-23) -->
- ~~**IV1 - §4:** `QueueMovie` calls the resource manager's slot 37 `(key, &resource, 0)`, then wraps the serial record
  in GZCOM class `0x0A32C0DA` (IID `0x8A32C0AC`) and queues that object's slot 5 result. Is slot 37 "open the
  resource" and the class a record stream for the decoder? [inferred]~~
  **ANSWERED 2026-10-27 (re-q-misc):** MATCHES, refined — slot 37 (`0x00972B78`) opens the DB record of the key: it finds the segment holding the key (slot 32) and opens the record in it for reading (access 1; 3 when the third argument is set); class `0x0A32C0DA` is `cGZDBSerialRecordAsIGZFile`, a serial record seen as a file, and its slot 5 is the `cIGZFile` interface the decoder reads (intro-video.md §4; record-as-file.md).

<!-- from: re-introvid7 (2026-10-23) -->
- ~~**IV2 - §3:** `GZPaint` calls the graphics system's (`0xC47B747C`) slot 3, then slots 12 `(0, 0, 0, 0)`, 15 `(1)`
  and 11 `(0x4000)`. Is that a clear of the frame to black? [inferred]~~
  **ANSWERED 2026-10-27 (re-q-misc):** MATCHES, refined — the graphics system's driver (slot 3) gets `ClearColor(0,0,0,0)`, `ColorMask(1)` and `Clear(0x4000)`: the colour buffer is cleared to black, with depth and stencil untouched (intro-video.md §3).
