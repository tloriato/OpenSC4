# Run apply15b-cores (2026-10-16)

Lane `apply15b`, DLL `engine/build-x86/FreeCity4.dll` at `e63d0b8f` (sha256 `b652c228`), city **Konradshohe**,
`turbo = 1`, `end_date = 2032-01-01`, `seconds = 600` (timeout 750). Request `build-runs/game-queue/apply15b-cores.req`,
results `build-runs/game-results/apply15b-cores.{verdict,log}`, slot 2, wall 42 s.

```
[ini]
enable = ports, citydetail, propdev, history
```

## Verdict

`verdict: clean`

```
AUTOTEST VERDICT: clean (foreign_exceptions=0, watchdog_stalls=0, rtc=0, overruns=0, ref_warnings=0,
  julian=2463249, date=2032-01-17, real_seconds=24.516, days_per_second=90.02, switches_off=-, disabled=-)
```

Problems (all 0): foreign first-chance exceptions, watchdog stalls, reference warnings, Rust panics, structured
exceptions, shadow INCOMPLETE lines, other WARNING lines. Shutdown lines: `budget Shutdown`; `history Shutdown`.

## What it shows

- The DLL loads into the real game and runs Konradshohe to 2032 clean (`ref_warnings=0`), with the
  re-answers22 cores' C ABI in place (ABI version unchanged).
- **`enable = ports, citydetail, propdev` has no effect in this tree.** The port developer, city detail manager and
  prop developer are engine cores plus a C ABI only (`engine/core/src/{port_developer,city_detail,prop_developer}`,
  `engine/capi/src/{port_developer,city_detail,prop_developer}.rs`); no plugin class and no shadow registers them
  (`engine/plugin/FC4Subsystems.cpp`'s table ends at `history` and `clock24`). The log contains **0** occurrences of
  `ports`, `citydetail` or `propdev`, and the subsystem listing shows only `history` (and the always-on `budget` and
  `simulator`) as enabled. There are also no matching shadows for these three. The three cores are therefore covered
  by `cargo test` and the conformance vectors, not by an in-game run — see question 1 in the lane report.
- `history` was enabled in the same run so the game's own query load (advisors, the demand simulator's tax series,
  the graph window) went through FreeCity4's warehouse — the class that answers `GetPoint` for HW2:

```
history Read -> 1 (143 series, 18215 points)
history Init: ok=1 city=030E3014 simulator=04C0D1C0 server=030C8414; 143 series, 18215 points
history Shutdown: 143 series, 18310 points; 67444 records sent, 1420 queries, 0 off-main calls
```

1420 queries served from a second thread-safe path, 0 off-main calls.

## Shadow summary lines

None: this run enables replacements only (the port / city detail / prop developer have no shadow, and `history_shadow`
needs the original's warehouse so it cannot run beside `enable = history`).
