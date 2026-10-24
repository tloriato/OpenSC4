| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| re-persistence-buffer-corpus: shipped buffer-resource inventory | Codex / `re-persistence-buffer-corpus` (outside contributor, RE side) | ready locally: PR2 answered for installer 25621; 440 distinct package contents, zero target entries; questions/doccheck pass; tools 178 passed, 1 skipped, 41 subtests passed; no engine/ABI/game run | User approval to publish fork branch and open focused PR; PR3 remains open |

## Report

The existing persistence PR2 asks whether shipped packages or saves contain the
legacy `0x62B9DA24` buffer resource. `persistence.md` §6.5 now records a complete
read-only inventory of Luca's GOG installer 25621, with a reproducible scanner and
a metadata-only manifest. The executable hash matches GOG 1.1.641.

The physical corpus has 857 DBPF files and 281,399 index entries. Deduplicating
identical support copies leaves 440 distinct contents and 232,303 entries:
22 data packages, 417 shipped city saves and one configuration package. No target
entries were found. The independent existing DBPF reader agreed on all distinct
files, including the positive control of 2,294 PNG-type entries. Full provenance,
scope and limits are in `docs/specs/evidence/persistence-pr2-gog-25621.md`.

PR2 moved from the open file to its existing answered archive, with provenance
preserved. No other question was closed. No coverage-map, symbol, engine or ABI
change is made, and no runtime or whole-owner verification is claimed.

## Validation

- `python tools/questions.py index`: regenerated only the expected persistence/total counts.
- `python tools/questions.py check`: passed.
- `python tools/doccheck.py check`: passed.
- `python -m pytest -q tools/tests`: 178 passed, 1 skipped, 41 subtests passed.
  The skipped case needs Windows symlink-creation privilege.
- The full suite initially had two pre-existing `Get-FileHash` discovery failures
  in Windows PowerShell child processes. Restoring the machine `PSModulePath` in
  the test process resolved both, without changing repository code:
  `$env:PSModulePath = [Environment]::GetEnvironmentVariable('PSModulePath','Machine')`.
- `git diff --check`: passed.
- `python tools/coverage.py --exe <matching-exe> --out build-runs/coverage-check.md`:
  unavailable because this checkout has no Ghidra analysis export. No ownership or
  coverage input was changed; the existing generated report is preserved.

Installer extraction, all proprietary files, the raw JSON inventory and local
cross-check script remain outside tracked deliverables. Public material consists
of behavioral/spec text, hashes/counts, an original read-only scanner and synthetic
tests. The maintainer's internal stop/queue/workers were not changed. Delivery is
through the outside contributor's fork and PR after publication approval.
