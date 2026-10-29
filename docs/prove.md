# Proving native functions

Run from the main checkout with 64-bit Python on Windows:

```powershell
python -m pip install unicorn==2.1.4 pefile capstone psutil
python tools/prove.py list
python tools/prove.py revalidate --jobs 8
python tools/prove.py check landvalue --dll engine/target/i686-pc-windows-msvc/release/fc4native.dll --variants 1000 --jobs 8 --keep-going --report build-runs/prove/landvalue-final.json
python tools/prove.py selftest --variants 1000
```

Addresses and coverage owners/subsystems can be mixed in a comma-separated selection.
The coverage owner map must already exist at `build-runs/coverage-owners.tsv`.
Prove writes only `docs/proven.tsv`; it never refreshes the coverage report.
If the map is missing, create it with
`python tools/coverage.py --owners build-runs/coverage-owners.tsv --out build-runs/prove/coverage-owner-map.md`
so the companion report stays under ignored outputs.
For example `landvalue`, `cSC4LandValueSimulator`, or `006bd540,006bd810`.
Checks copy and hash the DLL before use. They reject imports, apply PE32 relocations,
and map the image in a gap free in every captured address space.
The synthetic GDT occupies a separate free page; captured segment selectors and
TEB addressing are preserved with a private user-mode bootstrap.

A check uses every stored **valid** capture for the selected functions. Invalid
captures are listed separately and do not earn proof. The original runs twice and
is revalidated against the stored CPU exit before the port runs. Only the precise
named kernel-input exceptions apply to this CPU comparison, with every differing
byte logged during capture/revalidation. They cover shared timestamps and
`KUSER_SHARED_DATA.UnparkedProcessorCount` and the error-port-present bit of
`SharedDataFlags`, which can change while game threads
are stopped. Other bytes on that page are audited and remain comparable.
Unsupported instructions, uncaptured pages, incomplete returns, or other differences
cannot earn EXACT. Missing captures are reported as DIFFERENT with `no captures`;
this is a coverage limitation, not evidence of an incorrect port.

`--variants N` generates N inputs per function, distributed across valid captures.
Sweeps vary arguments, mutable scalar fields read before writing, entry registers
including callee-saved registers, and nonempty entry ST0. Values include integer
boundaries, NaNs, infinities, denormals and conversion-overflow values, followed by
seeded random values. Immutable image constants and values pointing into committed
memory are excluded from scalar-field mutation. Unknown argument counts are not
padded. The default seed is `0xFC4`. Successful original and port variants each run
twice for determinism. An original variant that faults, leaves the captured input,
or exceeds the deterministic instruction limit is skipped. The default is
200,000 instructions (`--variant-instructions`); `deadline_skips` reports these
separately per function. A port fault or instruction limit is DIFFERENT.
The thirty-second wall watchdog (`--replay-timeout`) aborts the check without
writing proof rows, because system load must not change a verdict. Skips never
count as passing inputs.

`--jobs N` uses independent worker processes (default 8). Work is divided by
capture and bounded variant batches (`--variant-batch`, default 32). Inputs retain
their global numbers; the parent reduces results in function/capture/input order,
so completion order cannot change the first difference or proof rows.

By default a function stops at its first difference. `--keep-going` tests the rest
of its captures and generated inputs while retaining the first difference and
counting capture/variant failures. The report includes passing variants, skips,
failed variants, and capture failures separately. `docs/proven.tsv` records the
DLL hash, attempted valid captures, passing variants, skips, scenarios and verdict.
Use `--no-proven` for experiments and deliberately broken DLLs.

## What is compared

* The caller-observed return channel, including observed virtual return sites.
* EBX, ESI, EDI, EBP, exit ESP, and the direction flag.
* All eight x87 stack values and tags, stack top and control word; MXCSR control
  bits with mask `0xFFC0`. MXCSR status bits are excluded because the emulator's
  status reporting differs from the CPU.
* Every changed byte outside the dead stack below exit ESP. The port's own frame
  below entry ESP is dead at exit too. A callee writing local output values there
  does not itself cause a difference. Caller argument slots at or above exit ESP
  remain live and are compared.
* Ordered calls into executable code, excluding compiler helpers and calls within
  the port DLL. Each call compares its target and only the selected callee inputs.

Call-input decisions are cached per callee and included in `call_input_decisions`
in the JSON report. Known argument counts use the ABI table. Low-confidence or
missing counts are unknown; a consistent callee `ret imm16` can establish a count,
otherwise cleanup after the call in the **original** caller is inspected (`add esp`,
`lea esp`, or a discard pop). Batched or ambiguous cleanup remains unknown, with no
padding. A virtual forwarding entry may consequently retain an unknown arity.

ECX is selected for thiscall/fastcall or an entry-path read before overwrite; EDX
for fastcall or a read before overwrite. Static scans follow direct branches and
calls and report incomplete paths. A register push used only to reserve a local,
whose bytes are overwritten before being read, is excluded. The report retains
both ABI confidence and entry-read evidence; an incorrect ABI label can still
produce a false input requirement and is a known limitation.
Partial register writes remove only the bits they replace: reading `DL` after
writing `DL` does not require incoming `EDX`, while untouched higher bits remain
inputs if used. Those masks also propagate through direct calls.

When a compared argument/register value points between the current call ESP and
the tested function's entry ESP in either replay, it compares as a frame token.
Local layouts differ between compilers. Other addresses compare numerically.
Incoming argument slots above entry ESP remain outside this normalization, even
when callee cleanup makes those slots dead on return.
The raw values and frame bounds remain in diagnostics. This normalization applies
only to call inputs: external memory writes, later calls, and returns still compare
normally, so an escaping pointer or changed local content can expose a difference.

Selected executable entries jump to the DLL during port replay. Running original
body instructions, writing the DLL image, changing an enabled entry jump, or
accessing uncaptured pages fails. RDTSC/RDTSCP use a fixed sequence in both runs,
including sites in system libraries. x87 transcendentals execute in the host helper.
The helper builds automatically with VS 2022 C++ Build Tools (`vswhere`, `ml64`,
`link`), under `build-runs/prove/bin`. A private Unicorn copy under
`build-runs/prove/pylib` is supported. `UC_IGNORE_REG_BREAK=1` is set before import.

## Capturing and limits

Only the coordinator should start captures during coordinated work. Capture needs
the pinned fixed-schedule DLL, with native replacements off. Before starting, check
that neither permitted slot is already running a game:

```powershell
python tools/prove.py capture --functions landvalue,009eef04 --scenario berlin-tegel --hits 8 --slot 10
```

Slots 9 and 10 are supported; lanes must use their assigned slot (10 for this work).
The isolated harness stages cities outside
Documents; the installed executable is read only and breakpoints exist in memory.
Each hit stores contexts, touched pages plus the nearby stack, changed exit pages,
and validation details. The page store deduplicates compressed 4-KB pages; no full
memory image is written. Uncaptured input pages cannot be invented during checks.
Other threads remain suspended while the tested thread runs. The hold timer counts
only intervals after debugger continuation, excluding paused callbacks. A sampled
system wait can release held threads to let the game finish; that capture records
the wait and remains invalid. An unproven sixty-second isolation limit is a
separate failure. Newly created threads are held too.
Failure to suspend a live thread invalidates the capture.

`revalidate [selection]` repeats original replay from stored entry pages and
compares the stored CPU exit, updating each record atomically. Without a selection
it visits the whole store. `--dry-run` produces a report without updating records.
Replays use frozen entry pages, including zeroed heap commits, rather than reading
live exit memory for a repeat. Missing CPU exits and known lost thread isolation
cannot be repaired offline. Reports retain each previous verdict and the precise
first repeated output difference. No game is started.

Synchronous observer file writes and `NtSetInformationFile` with
`FilePositionInformation` are modelled through their process-visible completion
records. Other information classes and unsupported services remain failures;
effects outside the process are outside this replay check.
The pinned observer's debug runtime can itself retain stack addresses in global
memory; those differences remain visible and need separate diagnosis. No blanket
observer-memory exception is applied.

Exit codes: 0 for success/EXACT, 1 for DIFFERENT, 2 for environment/input errors.
EXACT covers the recorded captures and generated inputs, including any explicitly
unknown call inputs; it is not exhaustive equivalence or whole-game owner credit.
No-capture functions and incomplete scans need more evidence.

## Tests and reports

```powershell
python -m unittest discover -s tools/prove -t tools -p test_prove.py
python tools/prove.py selftest --variants 1000
```

The independent Rust fixture is `tools/prove/fixtures/selftest.rs`. It retains x87
extended intermediates and exact integer loads, and truncates through 64 bits before
keeping the low 32 bits. `selftest` builds it and seventeen single-mutation copies
under `build-runs/prove-selftest/observable`, checks both exports, and requires the
unchanged export to remain EXACT. Sources, build logs and per-case reports are
retained there; `summary.json` contains the baseline verdicts and every first
mutation difference. Self-tests do not update the proof table.

Measured results, classifications, run statistics and historical evidence belong
under `build-runs/prove/`, not in this guide. Review notes and validation history
are under `build-runs/prove/reports/`.
