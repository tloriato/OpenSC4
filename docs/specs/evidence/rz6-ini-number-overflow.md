# RZ6: INI decimal overflow

Date: 2026-10-24. Analysis/spec-side evidence for [framework-rz-files.md §3.5.1](../framework-rz-files.md#351-decimal-overflow-rz6-compat).

## Finding

The supported Windows executable wraps the accepted signed decimal prefix modulo
2^32, then exposes those bits as a signed 32-bit value. It neither clamps the number
nor rejects a valid decimal prefix because its magnitude is too large. This confirms
the existing provisional wrapping policy for this domain.

ReadNumber's output is assigned only after a successful conversion. A numeric prefix
whose residue is zero still succeeds. No-digit input fails without changing the
numeric destination. The spec supplies distinguishing examples around 2^31, 2^32
and 2^64, as well as sign, trailing-text and no-digit cases.

## Static evidence

Input: GOG 1.1.641 Windows `SimCity 4.exe`, SHA-256
`3bc5c7fe807fe5aa24a3abb06e40f13486670bc1150612c09baf4ecedafe1a4b`.
Repository base: `7a010ed17e850437b114b04f56c893c41a26906e`.

The following are behavior facts and evidence locations, not instruction listings:

| Location | Established fact |
|---|---|
| ReadNumber 0x0091C8D2; call 0x0091C904 | After a successful ReadEntry, the embedded scanner 0x009EF536 receives the literal `%ld` at 0x00ACF148 and the caller's numeric output |
| 0x0091C90C–0x0091C90F | ReadNumber requires exactly one assignment; it has no independent magnitude/range check |
| 0x009F8518–0x009F8523, 0x009F8565–0x009F856F | Decimal accumulation has 32-bit arithmetic width without an overflow rejection |
| 0x009F85C7–0x009F85D0 | Negative sign retains modulo-2^32 behavior |
| 0x009F85E2–0x009F85F8, 0x009F8626–0x009F862F | A nonempty digit sequence permits one full 32-bit assignment; no digits bypass the destination write |

These facts were checked with the repository's `tools/x86dis.py`, pefile and capstone.
No behavior was inferred from the host Python or Windows C library's integer parser.

## Original-scanner probe

The analysis-only `tools/probe_ini_number.py` loads the user's own executable into
an isolated x86 emulator and calls its embedded scanner at 0x009EF536, using the
same `%ld` literal in the loaded image. It runs 32 fixed cases and compares their
assignment counts and destination bits against explicit modulo-arithmetic expectations.
Each case starts with a fresh emulator, the numeric output `0x12345678`, an input
string and a private stack. The executable hash must match before probing.

Only the native thread-context getter at 0x009F7604 is intercepted. Its fixture
points to the executable's original initial single-byte C-locale data. The original
digit classification, string input, decimal arithmetic, sign handling and output
assignment execute without replacement hooks. A private exception-chain memory page
supplies the scanner's x86 environment; no host Windows process or game instance is run.
Every case must return to the exact supplied return address within one million
instructions, otherwise the probe fails.

All 32 cases passed. They cover ordinary values, signed/unsigned 32-bit boundaries,
negative overflow, beyond-64-bit values, suffix termination, leading whitespace and
zeroes, failed conversions, 1,024 nines and 1,024 zeroes. The 1,024-nines input stores
-1; the 1,024-zeroes input stores 0; both report one assignment.

The host Python's arbitrary-precision arithmetic is used only for expected values.
The observed values and conversion count come from executing the original scanner.
The JSON field `read_number_boolean` is derived from its observed count and the
statically verified wrapper rule; it is not a claim that the complete ReadNumber
or ReadEntry path was emulated.

## Reproduction

Observed environment: Windows 11, Python 3.14.3, pefile 2024.8.26, capstone 5.0.9
for static inspection, and Unicorn 2.1.4 for the probe. Install probe dependencies
in a separate virtual environment, then run from the repository:

```powershell
python -m pip install pefile unicorn
python tools/probe_ini_number.py 'C:\Game\Apps\SimCity 4.exe' --output build-runs/ini-number-native-probe.json
```

The output parent directory must exist. The probe reads the executable and writes
only synthetic inputs/observations; no executable bytes, assets, extracted code or
instruction listings are part of its output. Keep the executable outside tracked
deliverables. The script is analysis-side tooling, not input for an engine author.

## Limits

This establishes the embedded scanner's `%ld` conversion for ASCII decimal values
under the supplied initial C-locale profile, plus ReadNumber's separate wrapper
check. It does not test file access, section/key lookup, changed runtime locales,
non-ASCII digits, errno, full game behavior, engine conformance or same-process shadow
equivalence. No owner, verification or replacement coverage is promoted.
