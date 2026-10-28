# Monthly checks and short re-checks

After landing the harness and tools together, **re-record every reference once**:

```powershell
python tools/check.py record --scenarios all --jobs 12
```

Keep the existing pinned reference DLL. Monthly inputs have separate keys; old
references remain usable with `check.py --legacy` or `qcheck.py --legacy`.
The lane records only its five proof scenarios, not the entire reference corpus.

`check.py run` captures a full save at every month end, and at a terminal partial
month. Generated script actions use the observer's existing `save` operation.
The default-off `golden_monthly` harness directive starts a watcher alongside the
game. It verifies the unique script and PID, suspends that game while copying and
hashing, and rejects a missed date. The engine and fixed schedule are unchanged.
There is no generated pause/speed/UIWait gap: under the pinned fixed scheduler,
UIWait can advance the calendar despite Pause or speed zero. Only the watcher's
OS process suspension holds time during capture.
Monthly runs set the existing `autotest_stall_grace_ms` to 60000, matching the
full-save grace. Suspension stops the watchdog thread, but its wall clock keeps
running; the ordinary ten-second limit can otherwise mistake capture work for
a stuck simulator. The outer run deadline still bounds failures. This is a
watchdog allowance, not a fixed-schedule or simulation input.

`monthly.json` contains SHA-256 hashes keyed by date and full TGI, after DBPF/QFS
decompression and class-aware object-ID normalization. Monthly and terminal
comparisons use the same normalizer. Class-local labels prevent a new class from
renumbering unrelated classes. Frame checksums are serialization metadata.

An ordinary check stops the candidate at the first differing month, preserving
that month's save, hashes and log. `--no-early-stop` captures all later months.
`check.py credit --dll <WT> --replace <keys> --scenarios all` always runs the full
timeline and writes per-owner verdicts and first differences in `credit.json`.
Record ownership describes observed state, including downstream effects; it does
not prove which replacement caused a difference. Missing records are unobserved;
owners after a controlled stop are incomplete. `--replace each` still works and
uses full scenarios for its qualification table. Credit rejects window inputs.
`credit --replace exact` can reuse combined credit from the same DLL when every
scenario in an owner's cover has complete, exact owned-record evidence. It also
accepts the old `each` table. An owner with no records in a selected scenario is
reported separately as unobserved; an errored scenario makes its credit incomplete.

```powershell
python .claude/lane-bin/qcheck.py --wt <WT> --replace zones --scenarios timbuktu-rush-hour
python .claude/lane-bin/qcheck.py --wt <WT> --replace advisors --scenarios timbuktu-rush-hour --from 2010-04-30
```

Qcheck defaults to the last matching month of the previous full-run failure for
those selected keys. Without a prior failure it runs the full scenario.
Failure anchors are scoped to the candidate DLL path, so lanes do not overwrite
each other's anchors and rebuilding the same path preserves its re-check point.
`--full` forces that path; `--from <date>` selects an anchor; `--months N` changes the
one-month window. An exact short re-check preserves the full-run failure anchor.
Both the original and candidate reload the same reference snapshot and replay
only actions after it. When falling back to the initial city, initial setup
actions are retained. The original window is recorded once and cached by inputs,
independently of replacement keys. Subsequent checks run only the candidate.

Snapshots are thinned by actual compressed save size: at most 96 MiB per entry
and 18 GiB across the store, with publication/accounting serialized between
writers. Every month's hashes remain. If an anchor was thinned, the tool prints
the earlier retained date actually loaded and keeps the requested window end.
A window's exact verdict applies to that reload/window, not the full scenario;
run full credit after a fix. Cold windows include an original recording, and
very short scenarios may have no time saving. Qcheck prints wall seconds.

## Validation on 2026-10-28

The tooling gate passes 421 tests. On `timbuktu-rush-hour`, the full advisors run,
controlled early stop and cached window agree on 2010-05-31 and the same 41
differing TGIs. Full/early took 164.389/76.346 seconds before the final scanner
optimization; the cached window took 28.725 seconds. A zones window was exact.
On a real 157-record checkpoint, the final scanner preserved every normalized
hash and reduced normalization from 4.603 to 1.408 seconds.

**The five-scenario timing proof is incomplete.** The 731-day generated reference,
a full zones check and combined-owner credit hit watchdog stalls on the shared
host. The final watcher suspends the game during decompression/normalization;
this is not yet established to be safe against the observer's ten-second
no-tick watchdog under load. Initial recording attempts also exposed a terminal
capture race, now covered by a regression test and a terminal hash completion
wait. Failed and superseded runs are retained separately in the lane's
`build-runs/evidence-fastloop36-v1` through `-v4`/golden run directories.
Resolve the watchdog interaction and complete the five timing/credit checks
before the coordinator performs the one-time corpus re-record described above.
