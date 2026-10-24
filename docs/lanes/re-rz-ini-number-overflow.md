| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| re-rz-ini-number-overflow: RZ6 decimal conversion | Codex / `re-rz-ini-number-overflow` (outside contributor, RE side) | ready locally: RZ6 answered MATCHES; 32 original-scanner probes passed; questions/doccheck/diff checks pass; tools 163 passed, 33 subtests passed; no engine/ABI/game run | Review and publish separately when authorized; preserve both RZ6 and the earlier local RZ10 answer when integrating their archives/index |

## Report

ReadNumber's previous description delegated overflow to an unspecified C runtime.
The supported Windows executable instead establishes a precise rule: accepted
ASCII decimal values wrap modulo 2^32 and are interpreted as signed 32-bit values.
Overflow does not make conversion fail; no-digit failures preserve the destination.
This confirms, rather than changes, the provisional wrap policy named in RZ6.

`framework-rz-files.md` §3.5.1 now states the compatibility rule and examples around
2^31, 2^32 and 2^64, with signs, suffix termination and failed conversions. RZ6 moved
to its answered archive with original provenance preserved; the question index was
regenerated. No other question, engine implementation or coverage attribution changed.

The evidence report `docs/specs/evidence/rz6-ini-number-overflow.md` records static
locations in the verified executable and a repeatable analysis-only probe. The
probe calls the original embedded scanner in Unicorn using the real `%ld` literal.
It supplies only thread context pointing to the original initial C-locale tables.
The original scanner's classification, arithmetic and assignment paths execute.
Its reported ReadNumber boolean is derived from the separately verified wrapper
assignment-count check; ReadEntry and the running game are outside the probe scope.

## Validation

- `python tools/probe_ini_number.py <matching-exe> --output build-runs/ini-number-native-probe.json`:
  all 32 cases passed, including 1,024 nines and 1,024 zeroes. Every case returned
  to the exact fixture return address within its instruction budget.
- Unsupported-executable control using `README.md`: rejected with exit 2, no report written.
- Executable SHA-256 before and after probing:
  `3bc5c7fe807fe5aa24a3abb06e40f13486670bc1150612c09baf4ecedafe1a4b`.
- `python tools/questions.py index`: expected framework/total count changes only.
- `python tools/questions.py check`: passed.
- `python tools/doccheck.py check`: passed.
- `python -m pytest -q tools/tests`: 163 passed, 33 subtests passed. Windows PowerShell
  child-process module discovery used the machine setting in the test process:
  `$env:PSModulePath = [Environment]::GetEnvironmentVariable('PSModulePath','Machine')`.
- `git diff --check`: passed.
- Coverage regeneration could not run without the absent Ghidra export. No mapping,
  owner status, whole-game conformance, shadow or coverage-increase claim is made.

All technical work and agent review were performed by Codex. No human validation
is claimed. No executable bytes, assets, decompiled output or instruction listings
are committed; local probe output and the proprietary input remain untracked.
This independent contribution does not operate the maintainer's internal workers.
