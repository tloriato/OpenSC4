| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| core-rzfiles4 (pure Rust core for cRZFile + cRZIniFile, framework-rz-files.md §1-5) | `core-rzfiles4` / (engine lane) | ready: code bc3cd1a8; engine/core/src/rz_files (host trait, file, ini, 17 tests); no ABI, no game run; gate cargo 3693 passed / clippy 0 / ctest 62/62; questions RZ1-RZ8 filed | 1. Coordinator: run tools/questions.py index (new prefix RZ). 2. Glue lane: Win32 RzFileHost + cIGZFile/cRZIniFile C ABI when a consumer needs it. 3. Spec lane: answer RZ1-RZ8. |

## Report

What changed (pure Rust, no C ABI / plugin / CMake):
- `engine/core/src/rz_files/host.rs`: `RzFileHost`, the explicit OS contract (open with framework modes, read,
  write, seek, tell, size, set end, commit, delete, rename, copy, exists, shortcut resolution, attributes,
  first-match size, temp dir, sleep in microseconds, C `time` seconds). Paths are framework bytes.
- `file.rs`: `RzFile`, cRZFile §2: refcount/QI (iid 0xE020364F, 1), Open (already-open no-op, creation 0..4,
  invalid -> false without error, `.lnk` resolve with fallback, existed status for modes 1/3), Seek (current
  converted to absolute when buffered, unknown mode = Position, failure re-reads OS position), buffered and
  unbuffered ReadWithCount / Read, WriteWithCount / Write (logical advances on failed flush, data dropped),
  FlushWriteBuffer, Close, Length (logical while pending), SetLength, Flush (commit only when pending),
  Remove / Rename (path set even on failure) / Exists / CopyFrom, SetBufferSizes rule, error mapping
  (5 -> 0x20000002, 183 -> 0x20000001), §2.10 static helpers. `destroy(host)` stands in for the destructor.
- `ini.rs`: `RzIniFile`, cRZIniFile §3: line reader (CR/LF runs collapsed, 40-byte chunks, no BOM handling),
  line syntax, ASCII / UTF-16 lower-casing, first-section and first-key rules, index + display names,
  Open with 250 ms sleep and 3 s window (missing file fails at once), leave-open, SetPath / SetFile ownership
  (shared `Arc<Mutex<RzFile>>` with AddRef/Release counts), ReadEntry / ReadNumber (`%ld`) / both
  ReadBinaryEntry forms, SectionExists, WriteEntry steps 1-6 (new file, new section, replace with tail copy
  and blank-line drop, insert before next header, append without newline), WriteNumber,
  WriteBinaryEntry, EnumEntries, EnumSections.
- `tests.rs`: 17 tests on an in-memory host: open modes and error codes, `.lnk`, read boundaries and buffer
  reuse (OS read counts), seek conversion, write buffering/readback, failed flush, buffer-size rules,
  refcount and path ops, every §4 worked example byte-exact, round trips, binary/number parsing,
  duplicates/syntax/BOM, newline runs and long lines, retry window (12 sleeps) and missing file (0 sleeps),
  leave-open and shared-file ownership, lower-casing.

Exact vs partial: cRZFile and cRZIniFile behaviour of §2-§4 implemented in full in the core; literal choices
at the spec's silent points are marked in code and filed as RZ1-RZ8
(docs/specs/questions/framework-rz-files.md, new prefix RZ; QUESTIONS.md index not regenerated: tools/ is off
limits to this lane). Not done (by brief): C ABI, plugin glue, real Win32 host, Directory/OFStream/CmdLine.

Gate: baseline cargo 3676 passed / clippy 0 / ctest 62/62; final cargo 3693 passed (+17) / clippy 0 /
ctest 62/62 (`qbuild all` PASS). rustfmt clean on the new files. Game: none (pure core, game false).
