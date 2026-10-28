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
`--store <path>` selects the coordinator's reference pin and store, including
cached window originals; run outputs still go to the lane. After a coordinator
repin, merge main and rebuild before comparing, and wait for its `RECORDING`
marker to disappear. An older pin's measurements remain historical evidence.

## Validation on 2026-10-28

Before the r6 repin, the tooling gate passed 424 tests. On `timbuktu-rush-hour`, the full advisors run,
controlled early stop and cached window agree on 2010-05-31 and the same 41
differing TGIs. Full/early took 164.389/76.346 seconds before the final scanner
optimization; the cached window took 28.725 seconds. A zones window was exact.
On a real 157-record checkpoint, the final scanner preserved every normalized
hash and reduced normalization from 4.603 to 1.408 seconds.

The existing stall allowance and faster scanner let all five test references
complete, including the 731-day generated case (762.353 seconds). Cached windows
were exact for the 3-, 32- and 62-day controls (130.110, 158.121 and 32.498 seconds).
Those timings used the lane's direct slot 13 harness; earlier full-check timings
used main's queue. Host load/startup varied, so these are observations, not a
controlled blanket speedup claim. The long case's window was not run before
main started recording a new pin.

Pre-r6 measurements in seconds (cold originals are excluded from cached windows):

| Scenario | Days | Main full qcheck | Cached lane window | Window verdict |
|---|---:|---:|---:|---|
| span565-buffer | 3 | 32.393 | 130.110 | exact |
| reach29-house | 32 | 38.540 | 158.121 | exact |
| maxisland-simicon-valley | 62 | 59.029 | 32.498 | exact; full had an unowned difference |
| timbuktu-rush-hour | 92 | 37.384 | 117.997 | zones exact |
| gen28-konradshohe-s0-default | 731 | 99.668 | pending | interrupted by repin |

**The five-scenario proof remains incomplete for the active r6 pin.** Main was
merged as authorized, retaining its schedule, draw checks and explicit store.
After that merge, the full tooling run printed `435 passed, 3 failed`: a native
cache probe failed through the Cargo corpus round-trip test, and two Lua staging
tests timed out. The two Lua cases plus the save/draw-focused tests subsequently
passed (`63 passed`). The required qbuild's Rust stage printed 8453 passed and
one failed (`golden_supported_owners_roundtrip_when_corpus_is_present`, missing
map key at `formats/tests/city_save.rs:449`); clippy passed without diagnostics.
The native build passed; CTest passed 147/148, failing through that same Cargo
test. Main's r6 `RECORDING` marker remained present after two nine-minute waits.
All lane operations finished; the post-repin game proof has not started.
The preceding-pin store measured 163,256,395 bytes, including 58 snapshots
totalling 79,323,381 bytes; the long reference retained 25 hash checkpoints.
Failed and superseded runs remain in `build-runs/evidence-fastloop36-v1` through
`-v4`; r6 proof uses a separate directory and a byte-identical coordinator pin.
Complete active-pin timing, full zones and combined credit before the one-time
monthly corpus re-record described above; the current r6 re-record is separate.
