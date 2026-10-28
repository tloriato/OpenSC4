# Function capture and proof

Run from the main checkout on Windows with 64-bit Python:

```powershell
python -m pip install unicorn==2.1.4 pefile capstone psutil
python tools/coverage.py --owners build-runs/coverage-owners.tsv
python tools/prove.py capture --functions landvalue --scenario berlin-tegel --hits 8 --slot 10
python tools/prove.py capture --functions landvalue --scenario london-fulham --hits 8 --slot 9
python tools/prove.py capture --functions landvalue --scenario berlin-spandau --hits 8 --slot 10
python tools/prove.py list
python tools/prove.py check 006bd540,006bd810,009eef04 --dll engine/build-x86/fc4native.dll --variants 1000
```

`landvalue` and `cSC4LandValueSimulator` resolve through the coverage owner table.
Addresses can be mixed with owners in a comma-separated list. Capturing the helper
requires adding `009eef04` to `--functions`. Only slots 9 and 10 are accepted.
Scenarios come from `engine/tests/scenarios/suite.tsv`. The harness uses the pinned
fixed-schedule observer, with native functions off and a 120-second stall grace.
The installed game is read only; entry and return breakpoints exist in memory.
The existing isolated harness stages cities in test slots, outside Documents.

The x87 helper builds automatically on first use. It needs Visual Studio 2022 C++
Build Tools: Python finds the installation with `vswhere`, then runs `ml64` and
`link`. Its outputs go in `build-runs/prove/bin`. A private Unicorn installation
in `build-runs/prove/pylib` is also supported. `UC_IGNORE_REG_BREAK=1` is set by
the tool before importing Unicorn.

All captures, DLL copies, reports, and caches go under `build-runs/prove`.
`pages.db` stores compressed, content-addressed 4096-byte pages shared across hits.
Each compressed hit record contains entry and exit contexts, memory-region facts,
entry page hashes, changed exit page hashes, and its validation result. Pages are
read on demand from the stopped process; no full memory images are written.
The stack around ESP is retained even when replay does not touch all of it.
Nested calls and tail calls can share a return stop; all matching captures finish
before the thread resumes. Incomplete returns are invalid.

Validation replays the original twice and checks CPU results, x87 values/tags and
control word, MXCSR control bits, callee-saved registers, ESP, DF, and memory outside
the dead stack below exit ESP. It additionally checks the whole EAX/ECX/EDX CPU
result. Kernel shared timestamps have explicit byte-level exceptions: deprecated
tick count, interrupt/system time, tick count, time-update lock, QPC baselines and
time biases. Each excused byte records its address, field, CPU value, and replay
value. Other changing kernel data, including processor count, invalidates the hit.
Invalid hits remain stored and cannot earn EXACT.

Checks copy and hash the input DLL, reject imports, apply PE32 relocations, and
choose a 64-KB-aligned gap free in every captured address space. The synthetic GDT
also occupies a separate free page. Replays retain captured CS/SS/DS/ES/FS/GS
selectors, using a private IRETD bootstrap for user privilege and the captured
TEB base. Each check revalidates its original replay against the stored CPU exit
using only the byte exceptions logged by the live validator. The selected original entry points receive
in-memory jumps during port replay. Executing the function's original body,
writing the port's image (even the same value), changing an entry jump, faulting,
or accessing uncaptured pages fails. Compiler helpers are omitted from the call
trace. Calls within the DLL are omitted; calls into the executable retain target,
ECX, EDX and at least four stack words, extended to the declared ABI argument count.

Return channels come from caller reads, including observed virtual return sites;
both tested land-value functions have an AL return channel. The whole x87 stack is
always compared, including ST0 returns. MXCSR comparison masks with `0xFFC0`.
RDTSC/RDTSCP sites are found in every loaded page, including system-library pages,
and use the same fixed sequence for original and port. Integer AVX2 byte comparisons
and byte-sign masks used by the pinned observer's string routines have explicit
fallbacks. Synchronous observer log writes model successful completion in process
memory without writing a file. Heap commits are zeroed during live capture; replay
cannot commit pages outside the stored capture. Unsupported operations fail validation.

`--variants N` requests N generated inputs **per function**, distributed across its
captures. IDs continue across captures. Inputs vary EDI/ESI/EBX/EBP, EAX/EDX/ECX,
stack arguments, non-pointer scalar fields read before being written, and nonempty
entry ST0. Sweeps include integer boundaries, float boundaries, NaNs, infinities,
denormals and conversion-overflow values, followed by seeded random values.
The synthetic GDT and DLL are not object inputs. The default seed is `0xFC4`.
Every successful original and port input runs twice, comparing complete outcomes
and changed pages for determinism. An original variant that faults is logged as
skipped; a port fault is DIFFERENT. A function stops at its first difference.

Checks write a full JSON report and refresh `docs/proven.tsv`. Its capture count
counts attempted valid captures, including the first failing capture. Variant
counts are passing generated inputs; skips are separate. `--report PATH` selects
the report destination. Use `--no-proven` for deliberately broken DLLs. Exit codes
are 0 for EXACT/success, 1 for DIFFERENT, and 2 for an input/environment error.
EXACT is evidence for the recorded captures and inputs, not exhaustive equivalence
or owner-level whole-game credit.

Tool checks:

```powershell
python -m unittest discover -s tools/prove -t tools -p test_prove.py
```

## Measured acceptance, 2026-10-28

The acceptance work is **incomplete**. The strict four-word call trace rejects
the Rust baseline before its generated variants. That rule remains enabled.
The 23 synthetic tool tests pass; they are separate from live-game proof.

The latest three runs are `20261028-221049-5446c9` (Tegel, slot 10, 39.934 s),
`20261028-221059-363a33` (Fulham, slot 9, 45.103 s), and
`20261028-221220-f9daa5` (Spandau, slot 10, 36.918 s). All use pinned observer
SHA256 `ee75f849f82f1fd3127865594bb0b8f67aae101b4a3f33dae6b31b9091f564a4`,
schedule `c7ac4ea4290c77db`, and each city's suite script through its suite end date.
Together: **349 hits, 343 valid, 6 invalid**, including 24 valid helper hits.
Land value alone: **325 hits, 319 valid, 6 invalid; 21/27 entries observed**.
Each table cell is captured/valid for that latest run.

| Address | Function | Tegel | Fulham | Spandau |
|---|---|---:|---:|---:|
| 006bd4b0 | GetGZCLSID | 3/3 | 3/3 | 3/3 |
| 006bd4c0 | QueryInterface | 5/5 | 5/5 | 5/5 |
| 006bd540 | TractIsZoned | 8/8 | 8/8 | 8/8 |
| 006bd5c0 | TractHasLand | 8/8 | 8/8 | 8/8 |
| 006bd640 | AddDesirability | 8/8 | 8/8 | 8/8 |
| 006bd6b0 | DoEndOfSimulationCycle | 4/4 | 4/4 | 4/4 |
| 006bd700 | GetLandValueType | 8/8 | 0/0 | 8/8 |
| 006bd740 | GetLandValue | 8/8 | 8/8 | 8/8 |
| 006bd750 | SetupAltitudeEffect | 4/4 | 4/4 | 4/4 |
| 006bd810 | LookupAltitudeEffect | 8/8 | 8/8 | 8/8 |
| 006bd8a0 | Release | 8/8 | 8/8 | 8/8 |
| 006bd8d0 | Write | 2/1 | 2/2 | 2/0 |
| 006bd950 | Read | 1/1 | 1/1 | 1/1 |
| 006bd9f0 | tree cleanup helper | 0/0 | 0/0 | 0/0 |
| 006bda30 | ProcessTwoGridsWithCallback | 8/8 | 8/8 | 8/8 |
| 006bdd10 | destruction helper | 0/0 | 0/0 | 0/0 |
| 006bdd90 | GetAverageLandValue | 0/0 | 0/0 | 0/0 |
| 006bdda0 | GetModeAltitude | 0/0 | 0/0 | 0/0 |
| 006bddb0 | interface thunk | 1/1 | 1/1 | 1/1 |
| 006bddc0 | interface thunk | 1/1 | 1/1 | 1/1 |
| 006bddd0 | interface thunk | 8/8 | 8/8 | 8/8 |
| 006bded0 | deleting destructor | 0/0 | 0/0 | 0/0 |
| 006bdf70 | Shutdown | 0/0 | 0/0 | 0/0 |
| 006be110 | ReadSimulatorTunableValues | 1/1 | 1/1 | 1/1 |
| 006be3b0 | Init | 1/0 | 1/0 | 1/0 |
| 006be7b0 | DoIdle | 8/8 | 8/8 | 8/8 |
| 006bf0d0 | DoMessage | 8/8 | 8/8 | 8/8 |
| 009eef04 | float-to-int helper | 8/8 | 8/8 | 8/8 |

All six latest invalid hits differ at kernel shared data `0x7ffe036a`, the unparked
processor count. This is not a clock exception. Earlier development attempts are
retained: cumulative **1,386 captures, 1,347 valid, 39 invalid**. Earlier failures
include unsupported operations and the corrected shared-return-stop debugger bug.
One earlier Init capture did validate. No invalid capture is counted as proof.

The six unobserved entries are four cleanup/destruction functions and two getters.
Fixed-schedule runs terminate after saving, so they do not perform orderly city
shutdown. These scripts did not exercise the two getters. No exact reason beyond
the missing entry hits is established for a particular caller path. GetLandValueType
was not observed in Fulham. No synthetic calls are counted as live captures.

The shared page database is **27,328,512 bytes**, containing 12,800 unique pages
with 19,806,101 compressed payload bytes. Compressed hit metadata averages
**15,243 bytes**; metadata plus amortized database cost averages **34,961 bytes/hit**.
All prove outputs were about 206 MB, well below 25 GB; disk use was 71.9%.
The three latest runs total 121.954 run-seconds (some ran concurrently).
Original replay averages **0.02820 s/hit**, median **0.002403 s**, over those 349 hits.
`build-runs/prove/acceptance.json` contains per-run and per-function measurements.

### Rust baseline and mutations

`build-runs/prove-selftest/lib.rs` comes from the spike's Rust source with the two
exports renamed and conversion changed to x87 truncation through a signed 64-bit
integer followed by its low 32 bits. It builds without imports. Rebuild/run:

```powershell
cmd /c build-runs\prove-selftest\build.cmd build-runs\prove-selftest\lib.rs build-runs\prove-selftest\correct.dll
python tools/prove.py check 006bd540,006bd810 --dll build-runs/prove-selftest/correct.dll --variants 1000 --no-proven --report build-runs/prove-selftest/correct.json
python build-runs/prove-selftest/run_mutants.py
```

The baseline returns **DIFFERENT on both functions**, at `call[0]` on the first
Spandau capture. No variants are reached. TractIsZoned's first callee takes zero
declared stack arguments, so all four required words describe the caller's frame.
LookupAltitudeEffect's first callee takes one argument; the three further words
also include frame data. The Rust baseline additionally differs in EDX there.
The checker retains the four raw words and EDX as required; it does not report EXACT.

All **17 single-mutation copies build and return DIFFERENT**. Their reports are
under `build-runs/prove-selftest/mutants/<name>/report.json`; the aggregate is
`mutants.json`. Every first rejection uses the captured input: TractIsZoned
arguments `(0, 0)`, or altitude bits `0x4388f5c1`. This is not a successful full
mutation test because the baseline itself fails, masking several mutations.

| Seed | First rejection on the changed function | Interpretation |
|---|---|---|
| bare Rust `as` | call[0] | baseline mismatch masks rounding |
| cvttss2si | call[0] | baseline mismatch masks conversion overflow |
| off-by-one bound | call[0] | baseline mismatch masks bound |
| missing call | call[0], changed target | missing-call trace exposed |
| extra executable call | call[0], target 006bd4b0 | extra-call trace exposed |
| callee ECX+4 | call[0], ECX+4 | receiver error exposed |
| different callee argument | call[0] | earlier baseline mismatch masks later call |
| conditional unrelated-field write | call[0] | earlier trace mismatch masks memory change |
| extra x87 value | TOP 0 versus 7 | stack error exposed |
| changed x87 control word | 127 versus 1151 | control error exposed |
| changed MXCSR rounding | 1f80 versus 3f80 | control error exposed |
| DF left set | DF 0 versus 400 | direction error exposed |
| EDI clobbered | call[0] | live register-variant acceptance blocked |
| hidden DLL state | write at 50013000 | image-write guard exposed |
| jump to original entry | executed original body at 006bd540 | original-body guard exposed |
| spike TractIsZonedBug | call[0] | baseline mismatch masks bound |
| spike LookupAltitudeEffectBug | call[0], changed argument | rounding argument differs too |

The independent synthetic tests have a passing baseline and test arithmetic,
missing/extra calls, changed ECX/argument, dead/live stack writes, x87/MXCSR/DF,
hidden image writes, original entry/interior execution, uncaptured reads, captured segment selectors, clock
replay, bounded loops, scalar-input selection and float encoding. Their EDI case
passes on its captured EDI=0 and fails only when EDI is varied to 1. These tests
do not replace the blocked Rust baseline acceptance.

### Native DLL and speed

The checked DLL SHA256 is
`90aa88b52ec3cfb8543587c293a01da7692c38ce0670e47cfedc359bcc6ca9db`.
The immutable copy is in `build-runs/prove/dlls`; the complete verdicts are in
`build-runs/prove/native-final.json` and `docs/proven.tsv`.

* **006bd540 DIFFERENT:** one Spandau capture attempted, arguments `(0,0)`, no variants.
  Call 0 targets `00808620`, ECX `208554b4`, EDX `00000001` in both runs.
  Stack words: original `[00000000,20db0334,00000000,00000001]`,
  port `[00000000,006bd615,00000000,00741275]`.
* **006bd810 DIFFERENT:** one Spandau capture attempted, altitude bits `4388f5c1`, no variants.
  Call 0 targets `005f4250`, ECX `2720d084`, EDX `00000000` in both runs.
  Stack words: original `[42860000,20db0334,00000032,006bee31]`,
  port `[42860000,00000000,86000000,cccc4005]`.
* **009eef04 EXACT:** all **96** retained validated helper captures across Tegel,
  Fulham and Spandau, **1,000 passing variants, zero skips**. The inferred channel
  includes EAX, EDX and ECX. This includes generated entry ST0 special values and
  register variations; it is not a proof over all extended-precision values.

The native source was not changed. No whole-game replacement credit is claimed.
The single-capture helper benchmark, including 1,000 passing variants, repeat
replays, comparison and setup, took **3.130 s**, zero skips. Reproduce with
`python build-runs/prove/benchmark.py`; it writes `benchmark-final.json`.
The multi-capture helper check took 3.961 s for 96 captures plus 1,000 variants.
