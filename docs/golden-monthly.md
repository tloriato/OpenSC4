# Monthly checks and snapshot windows

`tools/check.py record`, `run`, and `credit` use monthly save checkpoints for
fixed-schedule save scenarios. Draw scenarios keep their existing capture path.
The generated dated script calls the observer's existing full `save` operation
at every month end and at a terminal partial month. No scheduler changes are
needed. `--legacy` retains the end-save-only path for older references.

Each reference contains `monthly.json`: the date and SHA-256 of every decompressed
DBPF record, keyed by its full type/group/instance. Monthly and terminal save
comparisons use the same class-reference traversal to normalize object identity.
Record contents, class identities, and reference sharing still participate in
the comparison. A normalizer change requires new references.

## First differences and owner credit

```powershell
python tools/check.py run --dll C:/path/to/lane --replace zones,advisors --scenarios timbuktu-rush-hour
python tools/check.py credit --dll C:/path/to/lane --replace zones,advisors --scenarios timbuktu-rush-hour
```

`run` reports the first differing month, every changed record and its owner,
and the last matching month. Its watcher stops only that run's game process
after retaining the differing save and hashes. Missing or wrong-date captures
are errors. A stopped run marks later owners incomplete.

`credit` completes the timeline with all selected owners enabled together. Each
owner receives its first differing month and records, or `exact`, `unobserved`,
or `incomplete`. These are observations of owned records; a changed owner is
not proof that its replacement caused the difference. Combined credit can
qualify `--replace exact` when the same DLL has complete exact evidence on all
of that owner's covers. `--replace each` remains available for isolation.

## Rechecking a failure

```powershell
python .claude/lane-bin/qcheck.py --wt C:/path/to/lane --replace advisors --scenarios timbuktu-rush-hour
python .claude/lane-bin/qcheck.py --wt C:/path/to/lane --replace advisors --scenarios timbuktu-rush-hour --from 2010-04-30
```

The default uses the last matching month from that candidate path and owner
selection's previous full-run failure. Without such a failure it runs the full
scenario. `--full` explicitly starts at the original save. `--no-early-stop`
retains every checkpoint of a candidate run.

A window normally runs one month; use `--months N` to extend it. Both original
and candidate reload the same reference snapshot and execute only the remaining
dated actions. The original reload is recorded once under its own immutable
cache key. Subsequent checks reuse it. A successful window keeps the full-run
failure anchor for the next recheck and cannot be published as full-scenario
credit. A window proves behavior after that reload; follow with a full check
when the fix needs validation of its earlier history.

Window checks and direct-backend save checks do not append supplemental draw
sweeps. Full queue checks still add required draws, and full owner qualification
still requires their evidence. Explicitly selected draw rows remain selected.

Snapshots are thinned evenly by their actual sizes, at most 96 MiB per reference
and 18 GiB across the store. All monthly hashes remain. If the requested snapshot
was thinned out, the check reports the earlier retained date actually loaded
and keeps the requested window end. `--store` selects an alternate pinned store.

## Landing and reference migration

Land the tooling and harness together, then have the coordinator run the
following **once** against the chosen pin:

```powershell
python tools/check.py record --scenarios all --jobs 12
```

The previous end-save references do not contain monthly hashes or snapshots.
Until this recording completes, use `--legacy` for those references. The lane's
proof records only its five test scenarios in an isolated store; it does not
replace the coordinator's full recording.

When main repins, take main's schedule and harness before rebuilding. Wait for
`build-runs/golden/RECORDING` to disappear before running candidate comparisons.
Keep evidence from different pins separate.
