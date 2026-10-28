# Monthly/window proof, 2026-10-28

All results below use the coordinator's unchanged r6 pin:
`ee75f849f82f1fd3127865594bb0b8f67aae101b4a3f33dae6b31b9091f564a4`,
schedule fingerprint `c7ac4ea4290c77db`. Only the five test scenarios were
recorded, in an isolated store. No engine or scheduler changes were authored.

## Five real scenario timings

Times are wall seconds, including game startup and city loading. Full uses
qcheck's existing end-save-only path (`--legacy --full`). Cold window includes
recording its original reload; cached window reuses that original. Both paths
use the same direct backend, slot 13, harness, pin and candidate selection.
Backend/slot are cache inputs, so end-save-only originals were also recorded
for these five scenarios rather than borrowing the queue's incompatible keys.
Rush Hour enables `zones`; the other timing cases use `none`.

| Scenario | Full span, days | Full end-save check | Cold window | Cached window | Actual window |
|---|---:|---:|---:|---:|---|
| span565-buffer | 3 | 21.183 | 50.154 | 29.720 | 2026-01-27 to 2026-01-30 |
| reach29-house | 32 | 24.227 | 137.492 | 38.415 | 2026-01-31 to 2026-02-28 |
| maxisland-simicon-valley | 62 | 37.099 | 110.247 | 36.925 | 2102-10-31 to 2102-11-07 |
| timbuktu-rush-hour | 92 | 30.497 | 48.044 | 27.028 | 2010-06-30 to 2010-07-31 |
| gen28-konradshohe-s0-default | 731 | 90.234 | 50.604 | 29.753 | 2027-12-31 to 2028-01-28 |

Every completed comparison in this table was exact. The five measured cached
invocations sum to **161.841 seconds**; the two-year check is about three times
faster. Startup dominates short cases, which do not consistently improve.
These are individual observations on a shared host, not a latency distribution
or a guarantee about queue waiting. The five first-use windows sum to 396.541
seconds: the under-five-minute result applies after their originals are cached.

## First-difference and credit proof

On `timbuktu-rush-hour`, advisors full, early-stop and cached-window checks all
first differ on **2010-05-31**, with the identical set of **41 TGIs**. Their wall
times were 34.974, 27.089 and 30.252 seconds. The full timeline is complete;
the early run stops at May 31 rather than simulating to July 31. The default
window selects the previous failure's last matching date, 2010-04-30.

Zones is exact both full and in that window. One combined
`check.py credit --replace advisors,zones --scenarios timbuktu-rush-hour` run
completed the full monthly timeline and reported advisors `different`, zones
`exact`. It did not need owner isolation. Credit remains scoped to the selected
scenario; it does not qualify an owner on untested covers or infer causality.

The final store contains 134,522,640 bytes, including 40 snapshots occupying
56,912,824 bytes. Every reference respects the 96 MiB snapshot allowance;
the global snapshot cap is 18 GiB. All month hashes are retained.

## Validation and excluded attempts

`python -m pytest tools/tests -q` printed
`443 passed in 459.88s (0:07:39)`. Focused monthly/draw tests printed
`33 passed in 0.44s`. The separate full native gate still reports 147/148:
`cargo_test` fails in the existing city-save corpus test (`no entry found for
key`). The tooling cache probe now requires its own fresh, named, passing
compiler-cache CTest proof, rather than inheriting that unrelated failure.
The native gate itself remains failed and its logs are retained.

An initial queue benchmark appended seven draw sweeps and was cancelled as
incomplete. A direct check against queue-only reference keys failed and was
replaced by the isolated, matching-backend originals. An initial two-year full
candidate stalled at 2026-07-05 after 76.544 seconds; its harness logged a
DirectDraw initialization error. The unchanged-source retry completed exact in
90.234 seconds. None of these failed/incomplete attempts count as an exact run
or a timing success.

Commands, stdout, pin metadata and all attempts are retained in
`<WT>/build-runs/evidence-fastloop36-r6/results.json`; the cancelled operation is
separate in `incomplete-main-qcheck.json`. Running
`python build-runs/summarize-fastloop36-r6.py` verified the first month/TGI sets,
early-stop completeness, combined owner verdicts, five timing pairs, actual
snapshot dates and store bounds, and wrote `proof-summary.json`.

After tooling and harness land together, the coordinator should perform the
**one** complete monthly re-record described in [the migration guide](golden-monthly.md).
