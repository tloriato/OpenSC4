| Lane | Agent/worktree | State at stop | Next steps |
|---|---|---|---|
| verify-weather4: original weather shadow (partial) | `verify-weather4` / detached lane worktree (engine) | BLOCKED/WIP; implementation 2bdd48f5. Cargo 4938 passed, clippy 0 diagnostics; C++ C1083 22 failed edges, ctest skipped. WY1. No game runs or exact-shadow claim. | 1. Repair lane build writes. 2. Publish WY1 capture contract. 3. Complete replay and game controls. |

## Result

Partial implementation, blocked on the native input-capture contract and build infrastructure. This is not a completed simulation shadow and must not be credited as exact or ready.

`weather_shadow = 1` / `FC4_WEATHER_SHADOW` enables original-authoritative delivery instrumentation when the clock is replaced and weather remains original. An isolated Rust core compares `GetCurrentSeasonMonth` and `GetCurrentSeason` after original day/week/month agent deliveries using captured city hemisphere and simulator calendar inputs. No Rust Init, simulation operation, snapshot import, terrain/post callback or agent registration runs. The control seed is not claimed to match the native seed; no RNG draws are compared. All stateful replay is explicitly refused.

Runtime ownership checks require the published QueryInterface vtable entry to belong to the process executable and each delivery target to QueryInterface to the same weather interface. The original receives one authoritative delivery. Shutdown releases an acquired reference and clears retained pointers. Duplicate loads are refused instead of replacing state.

Logs preserve cumulative natural deliveries seen, wrapped native calls, passthroughs, owner refusals, simulation refusals, calendar equal/compared/mismatch counts and failures. First-day, first-week and monthly lines include dispatch evidence and captured calendar inputs. All lines say `INCOMPLETE WY1`, `full_exact=0` and `simulation_compared=0`, with zero coverage for Init/Read/Write/ambient/wind/grids/Shutdown. PreSave and PreCityShutdown are labeled as notifications, not evidence that original serialization/lifecycle calls succeeded. No fixture, fallback or resynchronization earns credit.

## Commits

- `aee7faff`: initial durable handoff from clean detached base `d796eda2`.
- `d8e91506`: substantive calendar-only shadow plumbing and WY1.
- `0fa584d7`: checked native date outputs, failure accounting and pointer lifecycle fixes.
- `2bdd48f5`: all eligible deliveries in refusal denominator, passthrough/input evidence, shared additions grouped at ends of lists.

## Validation

- Required baseline `qbuild all`: Rust 4938 passed / 0 failed / 0 ignored; clippy 0 diagnostics. C++ failed 22/208 edges with C1083 object writes denied; ctest skipped. This predates implementation.
- Focused `qbuild quick`: no Rust crate touched; C++ failed 22/209 edges with the same errors.
- Worktree-local SCCACHE_DIR and unique SCCACHE_SERVER_PORT=44267 did not resolve the errors. Output-parent ACL allows sandbox Modify. No permission/default/tool edits or service restarts were made.
- Final-source `qbuild all` at `2bdd48f5`: Rust 4938 passed / 0 failed / 0 ignored, clippy exit 0 / 0 diagnostics; C++ failed 22/209 edges, ctest skipped (0 run). Errors include `diag_logic_test.cpp.obj`, `traffic_cost_logic_test.cpp.obj` and `cFC4Message2Standard.cpp.obj`, all Permission denied. The new weather translation unit was not reached: its compilation remains unverified.
- Summary: `build-runs/weather-final-gate.txt`; detailed logs: `build-runs/qbuild-test.log`, `qbuild-clippy.log`, `fc4verify-build.log`, `fc4verify-configure.log`.
- `git diff --check d796eda2 HEAD` passed before this report-only change. No Rust files touched; rustfmt n/a. No ABI export/table, generated ABI entry, grid-family, tool or default edits.
- `q.py` could not launch Bash (E_ACCESSDENIED), so capped PowerShell reads were used. Git emitted a packed-refs.lock permission warning during detached commits, but commits succeeded; no refs/config/hooks/maintenance changes were attempted.

## Game evidence

No requests submitted: the DLL cannot be built. No outstanding labels. Observed runtime native comparisons: 0; exactness is unverified, not zero-call equality. Own 2032/ref0, combined literal 40-key profile, monthly/weekly runtime boundaries, full save/reload and actual Shutdown controls remain outstanding. No portable vectors or fixture results are claimed as native evidence.

## Parked question and next steps

`WY1` in [weather questions](../specs/questions/weather.md) asks for a published capture contract for the native constructor seed/private cRZRandom stream, the game-private per-thread CRT rand draws and lifecycle/host inputs. Weather saves its private cRZRandom state but not the shared CRT state; foundations section 12 says unrelated callers consume that stream. Post-initialization or repeated native snapshots cannot establish independent initialization or continued simulation coverage.

The coordinator must repair compiler output writes, and the spec side must resolve WY1. Then complete lifecycle/serialization/host replay and output comparisons without resynchronization, run focused/full gates and unique-label game controls covering original-authoritative weather, full save/reload/shutdown to 2032 and the current combined replacement profile. Regenerate the question index on the coordinator/spec side: this lane may not run prohibited tools scripts. No forbidden sources or executable bytes were accessed; no game files are committed.
