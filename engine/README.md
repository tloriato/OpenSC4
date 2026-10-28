<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# FreeCity4 engine

Clean-room engine code for FreeCity4. It is written only from [`docs/specs/`](../docs/specs/) and the LGPL
gzcom-dll SDK. Read [CONTRIBUTING.md](../CONTRIBUTING.md) before you change anything here.

| Path | What | Language |
|---|---|---|
| [`core/`](core/) | `fc4-core`: the portable simulation core. No GZCOM or OS types, no `unsafe`. | Rust |
| [`capi/`](capi/) | `fc4-capi`: a stable C ABI over the core (`include/fc4.h`), built as a static library | Rust + C header |
| [`conformance/`](conformance/) | Language-neutral JSON test vectors (the migration contract) and their Rust runner | JSON + Rust |
| [`formats/`](formats/) | `fc4-formats`: readers for the game's data files (DBPF packages, QFS/RefPack, exemplars and cohorts, LTEXT, FSH textures and S3D models), the `fc4-dat` inspection tool, and a sweep test over the installed game (`FC4_GAME_DIR`). Groundwork for the standalone engine (M6); not used by the plugin. | Rust |
| [`native/`](native/) | `fc4-native`: `fc4native.dll`, drop-in replacements ("ports") for the game's own functions, one function at a time, with the original's address, calling convention, object offsets and calls. No imports, no C runtime. The plugin loads it only when `FreeCity4.ini` lists functions. | Rust |
| [`plugin/`](plugin/) | `FreeCity4.dll`: GZCOM director with a subsystem table, the `cISC4Simulator` adapter, the budget classes (`cISC4BudgetSimulator`, `cISC4DepartmentBudget`, `cISC4LineItem`), the land value, weather, ordinance, demand and power simulators, the standard ordinances and the demand nodes (off by default), stream and property helpers, and our own standard message class (foundations §5.1; the SDK's `cRZMessage2Standard` is not used). Windows, x86 only. | C++17 |
| [`tests/`](tests/) | `capi_smoke` (drives the C ABI from C++ with the plugin's compiler and CRT), `message_test` (foundations §7 item 12) and `plugin_load_test` (loads the DLL like a host and drives the simulator and the budget) | C++17 |
| `third_party/gzcom-dll/` | gzcom-dll SDK (LGPL-2.1+), git submodule | C++ |

```
SimCity 4.exe ── GZCOM ──► FreeCity4.dll (plugin/, C++)
                              cFC4Director  ─ registers the classes of FC4Subsystems.cpp
                              cFC4Simulator ─ cISC4Simulator, cIGZSerializable,
                                              cIGZSystemService, cIGZMessageTarget2
                              cFC4BudgetSimulator, cFC4DepartmentBudget, cFC4LineItem
                                     │  fc4.h (C ABI)
                                     ▼
                           fc4_capi.lib (capi/, Rust staticlib)
                                     │
                                     ▼
                           fc4-core (core/, Rust): simulator, budget, streams, class-object
                           store, variants and property holders, curves, RNGs
```

The C ABI is the seam between the two languages. A future all-Rust GZCOM layer can replace `plugin/` and keep
`fc4.h` unchanged.

## Prerequisites

- Rust (stable, 1.75 or newer) with the `i686-pc-windows-msvc` target:
  `rustup target add i686-pc-windows-msvc`.
- For the DLL: Visual Studio 2022 Build Tools (MSVC x86, Windows SDK). CMake 3.20+ and Ninja ship with them.
- The SDK submodule: `git submodule update --init`.

## Build and test

Rust only (any OS):

```sh
cd engine
cargo test                                   # host target
cargo test --target i686-pc-windows-msvc     # 32-bit Windows, the plugin's target
cargo clippy --workspace --all-targets -- -D warnings   # must be clean, tests included
```

Run cargo from `engine/`, so `engine/.cargo/config.toml` applies. It links the C runtime statically
(`+crt-static`) for the Windows targets.

The plugin, from a shell with the x86 MSVC environment
(`"C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars32.bat"`):

```bat
cmake -S engine -B build-x86 -G Ninja -DCMAKE_BUILD_TYPE=RelWithDebInfo
cmake --build build-x86
ctest --test-dir build-x86 --output-on-failure
```

CMake runs `cargo build --release -p fc4-capi --target i686-pc-windows-msvc`, with its output in
`build-x86/cargo/`, then links `fc4_capi.lib` into `build-x86/FreeCity4.dll`. Everything uses the static release
CRT (`/MT`), so the DLL needs no runtime DLLs. CTest runs `cargo test` for the same target, plus the C++
`capi_smoke` test. On other platforms or architectures CMake builds and tests only the Rust parts and skips the
plugin.

### Native replacements (`fc4native.dll`)

`native/` builds `fc4native.dll`. Each port replaces one function of `SimCity 4.exe` and keeps its calling
convention, the object offsets it reads and writes, the calls it makes (in the same order, to the same addresses)
and its x87 arithmetic (`native/src/x87.rs`). A port is exported as `fc4p_<address>` (the 8 lowercase hex digits of
the original's entry) and listed, with its owner (the class) and name, in the exported table `fc4native_ports`.

```sh
cd engine
cargo test -p fc4-native --target i686-pc-windows-msvc    # the ports on objects at the game's layout, with fakes
cargo build --release -p fc4-native --target i686-pc-windows-msvc
```

The CMake build runs the release build and copies `fc4native.dll` and its pdb next to `build-x86/FreeCity4.dll`.
The release DLL has no imports, no entry point and no C runtime (`/NOENTRY /NODEFAULTLIB`), keeps its relocations
and prefers base 0x5B000000 (`native/build.rs`). Dev builds of the crate (the workspace's unwinding profile) link
std; they are never loaded into the game.

To use ports in a game, add to `FreeCity4.ini`:

```ini
[native]
functions = 006bd540, 006bd810   ; 8-hex addresses, owner names (landvalue) or all
dll = C:\path\to\fc4native.dll    ; optional; default: next to FreeCity4.dll
```

At game start, once the executable is identified as 1.1.641, the plugin (`plugin/FC4Native.cpp`) loads the DLL,
checks each enabled port's export against the table and writes a 5-byte `jmp` to it at the original's entry, in
memory only. The log has one `native: jump installed ...` line per port and a summary line. Without `functions`
the plugin loads nothing, writes nothing and logs nothing. Only `functions` and `dll` belong to `[native]`;
other keys stay global wherever they stand (the ini reader ignores other section headers). A slot run of
`tools/ingame_run.ps1` keeps only FreeCity4.dll in the slot's Plugins folder, so give `dll =` there.

To add a port: declare it with `port!` in a module of `native/src/` (see `native/src/port.rs`), list it in that
module's `PORTS`, and test it on an object at the original layout (`layout!`) with fakes for the game functions
it calls (`game_fn!`, `testing::fake`) under the game's control word (`testing::GameFpu`).

### Save sweep (records read back from every save)

`capi/src/save_sweep_tests.rs` reads class records from every distinct `.sc4` below `FC4_SAVES_DIR` (else
`FC4_GAME_DIR`, else the default GOG install; read-only; identical files counted once; it passes with a note when
the directory is missing), reads each entry into a fresh object over the C ABI and writes it back. References
(foundations §2.7) go through as `classID << 32 | objectID`, so a record needs no other record of the save.
The history and 24-hour clock records have their own record mode in `fc4-formats` (`formats/tests/save_records.rs`,
over the core types); these four go through the C ABI with the `fc4-capi` test hosts, so the two stay separate tests.

| Record | Class | Check |
|---|---|---|
| commercial (commercial-industrial §8.1, §12 test 9) | `0x8990C080` | byte-identical |
| industrial (§8.2) | `0xA990C022` | byte-identical |
| civic (civic-buildings §7, §12 test 10) | `0x0A413481` | the record with each multi-entry bucket chain reversed (CB6), and byte-identical after a second round trip |
| parks (parks §7, §12 test 8) | `0xAA3FE6BC` | byte-identical |
| prop developer (prop-developer §7; `capi/src/prop_city_sweep_tests.rs`) | `0x89C48F47` | version 5 byte-identical; version 4 written as version 5 (version 4 -> 5, `01 00000000` appended) and byte-identical after a second trip |
| city detail (city-detail §4; the same file) | `0xEAD4BA36` | byte-identical |

```sh
cd engine
cargo test -p fc4-capi --release save_sweep -- --nocapture
# another corpus, e.g. the regions folder:  FC4_SAVES_DIR=<dir> cargo test -p fc4-capi --release save_sweep -- --nocapture
```

Each class prints `save sweep <name>: records=… identical=… chains-reversed=… (chains …) second-trip-identical=…
failures=…; sizes: …` and the first failures. Result on SimCity 4 Deluxe (GOG), 2026-10-15: 417 saves, all
distinct; commercial 417/417 identical (19 bytes each), industrial 417/417 (2 bytes), civic 417/417 identical and
417 identical after a second trip (no save has a bucket with two entries; 3,821 to 5,305 bytes), parks 417/417
(10 bytes ×411, 18, 34, 58, 82).

### Network rules sweep (the shipped rule files)

`formats/tests/network_rules.rs` loads every shipped network rule resource (the tile rule files with
`#Transmogrify`, RUL0, RUL1, RUL2 and the network INI; network-rules.md §2) with `fc4_core::network_rules`, read
from the install's `.dat` files at run time and never committed, and reports the expansion counts and the load
order's evaluator and rule counts. See `formats/README.md` for the result.

```sh
cd engine
cargo test -p fc4-formats --release --test network_rules -- --nocapture
```
The prop developer and city detail records have their own test (`cargo test -p fc4-capi --release
prop_city_save_sweep -- --nocapture`), printing `save sweep propdev|citydetail: records=… identical=…
v4-written-as-v5=… second-trip-identical=… failures=…`. Result, 2026-10-16: 417 saves; prop developer 417 records,
10 version 5 identical and 407 version 4 (122 bytes) written as version 5, all identical after a second trip; city
detail 14 records (8 bytes, 403 saves have none), all identical.

## Installing and checking in game

1. Copy `build-x86/FreeCity4.dll` to the game's plugin folder, for example
   `Documents\SimCity 4\Plugins\` or `<game dir>\Plugins\`.
2. Start the game. `FreeCity4.log` appears next to the DLL, or in `%TEMP%` if that folder is not writable.
   - On start you should see `FreeCity4 … loaded (… replacement mode)` and `PostAppInit (simulator class 0x2990C1E5
     provided by FreeCity4)`.
   - When a city opens, you should see `cFC4Simulator created`, `Read: …`, `Init: …` and then `PostCityInit: hidden pause released`. While the city runs you
     should see one `SimNewMonth …` line per game month.
   - Diagnostics: `FC4_LOG_LEVEL` = `0` (off), `1` (lifecycle only) or `2` (default, verbose). Verbose mode traces
     `DllMain`, every director call (`InitializeCOM`, `OnStart`, `EnumClassObjects` with each class ID reported,
     every `GetClassObject` with the clsid, the iid, the result and whether our factory ran), plus simulator
     `QueryInterface` (the first 200 calls), `Init`, `Read`/`Write`, `OnTick` (the first tick and every 1000th),
     `DoMessage` (the first 50), agent registration and pause calls. Lines marked `[v]` are verbose. Every line is
     written straight to the OS. The log is opened with full sharing, so a viewer that holds it open cannot block
     writes. The path never changes once chosen. Failed writes are retried once and then reported on the next line.
   - Crash tracing: the class-object path (`GetClassObject`, the factory, `QueryInterface`) runs under a Win32
     structured-exception guard. A first-chance vectored handler also logs every exception raised inside
     `FreeCity4.dll`'s image, with code, address and offset from the DLL base, and passes it on unhandled.
     Exceptions outside our image are reported at most twice per faulting address (16 addresses), as
     `module+offset`, with the registers, the x87 state (`  fpu: CW=0x027F SW=0x0000 (TOP=0 SF=0 IE=0 C1=0)
     TW=0xFFFF occupied=0 [FXSAVE FTW=0x00 occupied=0] indefinite NaN: none`: TW decoded as the full tag word,
     the FXSAVE part when the context carries it; `indefinite NaN` lists the registers that hold the x87 default
     NaN), the first 8 raw dwords at esp (`  esp dwords: 0x… ×8 (at esp=0x…)`), the return addresses into
     `SimCity 4.exe` and `FreeCity4.dll` in the top 4 KiB of the stack (map the executable's with `python tools/addr2func.py --log <log>`, coordinator
     only: it reads RE material) and the last interface calls. The handler never
     blocks: it takes the log lock only with a time-limited try-lock and otherwise writes through a spare
     handle. Stack overflows are only counted. `FC4_DIAG_SELFTEST=1` raises one deliberate access violation at
     load to prove that both work.
   - Heartbeat (always on, also at `FC4_LOG_LEVEL=1`): one line per speed or pause change and per month start,
     and a `heartbeat (periodic)` line every `FC4_HEARTBEAT_MS` (default 10000) with the julian date, speed,
     the pause/hidden/emergency flags, and since the last heartbeat the `OnTick` count, agent deliveries,
     interface calls and verbose lines suppressed by caps, then the memory: address space used, the largest
     free block (the game is a 2 GiB process) and the private bytes, and last `x87 leftovers: N` (with the three
     sites with the most findings when N > 0, e.g. `x87 leftovers: 12 (deliver (after) 8, tick 4)`).
   - x87 leftover guard (always on, `plugin/FC4Fpu.h`): every x87 exception is masked in the game, so a value a
     wrong return channel leaves on the x87 register stack (or takes off it) does not fault; eight leftovers later
     loads yield NaN. On the main thread, the plugin FXSAVEs and counts the occupied registers at the entry and the
     end of our `OnTick`, at each framework tick, before and after every agent delivery, at the entry and exit of
     every `DoMessage` of ours, after our synchronous `MessageSend`, and at every `fpu …` line (which includes the
     monthly observation). Nothing is popped. Each finding is counted per site; the first per site per game month
     is logged: `x87 leftovers at '<site>' (msg 0x<type>, '<agent>'): N register(s) occupied (FSW=0x…. TOP=t,
     FTW=0x..), indefinite NaN: <ST list|none>; previous check '<site>' had M; game month YYYY-MM, K at this site,
     T in all`. The first time the sticky stack-fault flag is seen: `x87: the stack-fault flag is set at '<site>'
     (FSW=0x….: SF=1 IE=. C1=., a stack overflow|underflow …). Logged once.`
   - Hang watchdog: a background thread watches markers the simulation thread publishes without locks (inside
     `OnTick`, the agent delivery in progress with level, message and agent name, core `fc4_*` calls, our
     interface methods). If the simulation thread stays inside our code for more than `FC4_WATCHDOG_STALL_MS`
     (default 5000), or no `OnTick` arrives for `FC4_WATCHDOG_NOTICK_MS` (default 15000) while a city runs and
     is not hidden-paused, it logs `WATCHDOG: … stalled` with that activity, the last 64 interface calls, and a
     sample of the thread: suspended only to copy EIP/ESP and its stack, then resumed before anything is
     formatted. The sample lists the return addresses into `SimCity 4.exe` and `FreeCity4.dll` as
     `module+offset`, plus process and thread CPU time (a thread that waits gains no CPU, a spinning one does).
     It repeats every `FC4_WATCHDOG_SAMPLE_MS` (default 5000), at most 10 samples per stall, and ends with
     `WATCHDOG: stall over after …`. `FC4_NO_WATCHDOG=1` disables the thread.
   - Scheduler tracing: the first 300 agent deliveries are logged with level, message, agent and result. Per
     agent, the log counts deliveries, "not finished" results and the current streak of consecutive ones, and
     notes when a streak passes 100, 1,000, 10,000, and so on. Every `FC4_STATUS_EVERY` ticks (default 500) it
     writes a status block: the day-barrier conditions (`DAY_GATE_OPEN`, interval timer, blocking levels,
     counters), and for each level its cursor (agent name or END), its size and its top five false streaks. The
     first 200 broadcast posts are logged, and every SimNewDay, SimNewWeek, SimNewMonth and SimNewYear post. Date
     reads are logged too: the first 20 calls to each `GetSimDate` variant and `GetSimDateNumber`, and the first
     200 method calls on our `cIGZDate` objects.
   - At `PostAppInit`, verbose mode also asks `GZCOM()->GetClassObject(0x2990C1E5)` once. The answer shows whether
     the game's registry resolves the class to our factory. The object it creates is released at once. Set
     `FC4_NO_PROBE=1` to skip this check.
   - Limit-timer calibration (spec §5.5): the simulator keeps the game's two CPU-cycle coefficients
     (`CyclesToMillisecs`, `MillisecsToCycles`) up to date. Subsystems such as the lot developer use them to budget
     their work. Writing them needs the version-specific addresses in `plugin/compat/`, and only happens after the
     host has been identified as the exact 1.1.641 executable. The log shows each check (`compat: …`) and the
     published values (`calibration …`), both at `Init` and in every status block. `FC4_DISABLE_CALIBRATION=1`
     turns off the writes (for A/B tests).
   - Budget: `budget Init: …` when a city opens (funds, departments, bonds, tax rates), `budget Read/Write`,
     cheats and bonds as they happen, and three `budget settlement: …` lines each month: funds before and after
     with income and expense, the tax of each of the 12 groups with its rate, and the bonds (count, borrowed,
     monthly payments). `FC4_BUDGET_FIX_CS3=1` enables the intended CS$$$ tax weight (budget spec §13.1; off by
     default, as in the original).
3. **A/B against the original:** `FC4_DISABLE` takes a comma- or space-separated list of subsystems to leave to
   the game (`simulator`, `budget`, or `all`), e.g. `FC4_DISABLE=budget`. `FC4_DISABLE_SIMULATOR=1` still works.
   The DLL still loads, but it does not register those classes, so the game's own ones run. The simulator and the
   budget only talk through the SDK interfaces, so either can be switched off alone.
4. **Bisection (`FreeCity4.ini`):** a `key = value` file next to `FreeCity4.dll` (`#` or `;` after whitespace
   starts a comment, `[sections]` are ignored). A key in the ini wins over the environment variable.
   - `disable = budget` means the same as `FC4_DISABLE`. It is read once, when the game enumerates the classes,
     so a change needs a game restart.
   - Some subsystems are **off by default** and only registered with `enable = …` (or `FC4_ENABLE`); `disable`
     still wins (`enable = landvalue, weather` takes a list). The demolition utility is one of them: `enable = demolition` registers `cSC4Demolition` (class `0x0990C38A`) over `plugin/FC4DemolitionHost.h`, the full 84-callback `fc4_demolition_host`. Without it the log says
     `subsystem '…': off by default` and the game's class runs, so budget bisection runs are unaffected:
     - `landvalue`: the land value simulator (`cSC4LandValueSimulator`, landvalue spec; its grids are the game's
       own `cSC4SimGrid` objects, created through GZCOM, simgrid spec §6.3);
     - `weather`: the weather simulator (`cSC4WeatherSimulator`, weather spec; the map view reads its moisture
       grid through an `SC4Vector`-compatible mirror, §11.2);
     - `ordinance`: the ordinance simulator and the 21 standard ordinance classes (`cSC4OrdinanceSimulator`,
       ordinance spec; replaced together, §10.2; plugin ordinances are accepted through `cISC4Ordinance`);
     - `demand`: the demand simulator and the demand node class (`cSC4DemandSimulator`, `cSC4Demand`, demand
       spec; the graph comes from the game's demand exemplars). The building profile's capacity maps (§7.2) are
       the game's STLport maps (docs/abi/allocators.md, layouts.md): `plugin/FC4GameMap.h` walks them read-only
       (count, depth, link, key-order and step checks; a map that fails them, or faults, reports nothing) on any
       executable. `GetLocalPopulationSummary` (§5) empties and fills the caller's STLport map with the game's own
       subtree free and `operator[]` (docs/abi/direct-calls.md), only on the identified executable and under the
       structured-exception guard; elsewhere the map is left untouched (logged once). Switches (default `1`,
       environment `FC4_DEMAND_<KEY>`): `demand_capacity`, `demand_direct_calls`. Log lines:

       ```
       demand: capacity map read: building type 0x…, N map(s), M entries: 0x1010=12, …        (once)
       demand: GetLocalPopulationSummary: N entries copied into the game's map (K old entries cleared, count now N;
         main|another thread)                                                                 (once)
       demand month: … export cities; capacity: reads=… entries=… units=… (occupancy: reads=… units=…) refused=…;
         summary: calls=… copied=… untouched=… failed=…                                       (each month)
       ```

       `reads` counts building profiles read (census and §7.2), `occupancy` the §7.2 reads alone (StateChanged),
       `refused` the profiles whose maps failed the checks; the month counters restart at each month line.
     - `power`: the power simulator (`cSC4PowerSimulator`, power spec; its powered grid is the game's own
       `cSC4SimGrid<uint8>`, created through GZCOM). Not yet done (logged once): power lines are not detached on
       terrain changes (the line's `GetConnectedPoles` is not in docs/abi), and the Nuclear Free Zone does not close
       the 3D view's placement tool. The pole's `RemoveConnection` is a direct call, made only on the identified
       executable. Its game calls have their own switches (default `1`, environment `FC4_POWER_<KEY>`):
       `power_occupants`, `power_poles`, `power_lots`, `power_neighbors`, `power_budget`, `power_grid_writes`,
       `power_posts`, `power_terrain`, `power_zone_tool`, `power_view`, `power_radiation`,
       `power_occupant_query`, `power_demolish`, `power_direct_calls` (see `plugin/FC4Config.h`).
     - `pollution`: the pollution and garbage simulator (`cSC4PollutionSimulator`, pollution spec; its eight grids
       are the game's own `cSC4SimGrid<int16>`, created through GZCOM). The getters are answered from any thread
       (the game asks from a second thread during a day or a month). Not yet done (logged once): the visual effects
       of §4.10 (`cISC4VisualEffect` / `cS3DTransform` are not in docs/abi). The log has a status line at `Init`,
       `Read` and every month (`pollution month: init=… cells=… lists wtp/inc/wte/rec=… avg air/water/garbage=…
       supply=… capacity=… last month produced/…/landfilled=… cost=…; grids airSrc nonzero/tracts sum [min..max],
       …`; at `Read` a grid can still have 0 tracts, its record is read later; a month that finds one so logs
       `pollution: WARNING grid … has 0 tracts at a month …` once). Its game calls have their own switches (default `1`, environment `FC4_POLLUTION_<KEY>`):
       `pollution_occupants`, `pollution_lots`, `pollution_neighbors`, `pollution_budget`,
       `pollution_grid_writes`, `pollution_posts`, `pollution_history`, `pollution_ordinance`,
       `pollution_traffic`, `pollution_zoning`, `pollution_power`, `pollution_landfill`, `pollution_population`,
       `pollution_props`, `pollution_terrain` (see `plugin/FC4Config.h`).
     - `protection`: the fire and police simulators together (`cSC4FireProtectionSimulator`,
       `cSC4PoliceSimulator`, protection spec; replaced together, §13.2). Their coverage grids are the game's own
       `cSC4SimGrid<uint8>` / `<int16>`, created through GZCOM. The fire plane and police helicopter dispatch
       sources are the game's classes, created through GZCOM; their `Init` / `Shutdown` are direct calls, made only
       on the identified executable (otherwise no air source, logged). A station's dispatch effect (its cone while a
       dispatch is active, and `GetStationDispatchRadius`) is scaled by the station's efficiency (PR14); one with no
       centre adds nothing. The getters the game calls from its second
       thread are served under a per-object lock; a thread that cannot get it within 20 ms gets the last month-end
       values (or a neutral one) and is counted as `fallbacks=`. The game calls have their own switches (default
       `1`, environment `FC4_PROTECTION_<KEY>`): `protection_occupants`, `protection_budget`,
       `protection_dispatch`, `protection_automata`, `protection_grid_writes`, `protection_posts`,
       `protection_history`, `protection_air_sources` (see `plugin/FC4Config.h`). Log lines:
       `cFC4FireProtectionSimulator(…) created`, `protection Init (fire): init=1 … stations=… department=…
       grid=… air=…`, once per game month `protection month (fire|police): … stations=… coveredCells=…
       maxCoverage=… avg=… pct=… dispatches=… units=… strike=… reentrant=… fallbacks=… posts=…` (police adds
       `protection month (police jails): jails=… capacity=… population=…`), `protection Read/Write (…) -> 1`,
       `protection Shutdown (…)`, `cFC4…Simulator destroyed (N fallbacks)`. `fallbacks` counts calls from a thread
       other than the main one that could not get the lock in 20 ms and were answered from a snapshot or a neutral
       value; no protection slot is known to be called off the main thread (protection spec §13.1), so it should stay
       0, and a non-zero count is a finding. The grids' geometry (tracts, tracts per
       day, the iterator rectangle) is taken at `Init`: on a loaded city the `Read` line can show `tracts=0x0` with a
       non-null `grid=`, because the game reads the grid's own record after the simulator's. A month that still
       finds the coverage grid without tracts logs `protection (fire|police): WARNING the coverage grid … has 0x0
       tracts at a month (init=… tractsPerDay=…): no daily cycle runs (logged once)`.
     - `crime`: the crime simulator (`cSC4CrimeSimulator`, crime spec; its crime and crime-effect grids are the
       game's own `cSC4SimGrid<uint8>`, created through GZCOM; police coverage comes through whatever police
       simulator the city has). The getters are answered from any thread (they read a snapshot); the prop
       requester's calls run on the main thread only (a call from another thread is answered 0 and logged once).
       The random generator is seeded with `derive_seed(S, 0x6C09C3F6)`, `S` the session seed (`session_seed`,
       else the original's `cRZTime` clock, seconds since 1901-01-01 UTC, `time() + 2177452800`, crime spec §9, at
       game start; "Session seed" below), or `crime_seed = N` (env `FC4_CRIME_SEED`, decimal or `0x…`) for a repeatable
       run. Crime type names follow crime spec §7: the LTEXT (`0x2026960B`, group + language ID, instance), then
       (group, instance), detokenized by the game's string detokenizer, U+2022 → U+200B; `0xAA12FCB0` if that
       fails, else an empty name (a failed load never keeps the text, crime spec §7 / CQ16). The log has a
       status line at `Init` and `Read` (`crime Init:
       init=… state=… types=… S=… decay=…`) and one every month from the first (`crime month yyyy-mm:
       crimes=N prevented=P arson=A/T incidents=I grid sum/max=… (tracts=…) average=… hotspot=(x,z)=v props
       requested=R selected=K/Q jailed=J released=L criminal base=B to commit=C attempts=… (no lot …) police
       grid=yes|no ordinance=… posts=…/… state=… types=… days=… months=…`). Its game calls have their own
       switches (default `1`, environment `FC4_CRIME_<KEY>`): `crime_lots`, `crime_police`, `crime_residential`,
       `crime_demand`, `crime_ordinance`, `crime_occupants`, `crime_fire`, `crime_props`, `crime_posts`,
       `crime_grid_writes`, `crime_zoning` (see `plugin/FC4Config.h`).
     - `aura`: the aura (mayor rating) simulator (`cSC4AuraSimulator`, aura spec; its base, aura, transient,
       park, landmark and work grids are the game's own `cSC4SimGrid<int8>` / `<int16>`, created through GZCOM,
       because the map view reads the aura grid and the park map as raw memory, §14.2). The getters are answered
       from any thread (they read a snapshot the core publishes after every main-thread call). The random
       generator is seeded from the wall clock at construction (§9), as the original's. Inputs come only through
       the game's interfaces, whichever simulator is active; one that is not there (no pollution, crime, traffic
       or residential simulator, or a null grid) is logged once (`aura: input … not available (…); its stage is
       skipped (the spec's neutral value)`) and listed on the cycle line. [choice]s, logged once where they show:
       the lots are found by `GetLot(x, z, false)` over every cell (the safe lot iterator's layout is not in
       docs/abi); lot populations are read with `GetPopulation(type)` (lot slot 53) for the 12 developer types,
       not `GetPopulations` (its container is not in docs/abi); the game-data registry node `aura` is not
       published (as budget's); the base auras are not copied to the process-wide developer-type table (§16
       question 7); the CPU-cycle limit timer runs on `QueryPerformanceCounter`. The log has a status line at
       `Init`, `Read` and every month (`aura month: init=… state=… mayor rating=R max=M@(x,z) min=m@(x,z) aura
       tracts=N sum=S [lo..hi] base tracts with data=… hotspots=H riot roll=P departments=… global effect=…
       base auras=…/…/… decay long/short=… slice step/miss=… reentrant=… days=… months=… cycles=… lot walks=…
       lots=…; inputs none missing so far | not available (neutral): …`), one line per completed cycle (`aura
       cycle N completed (day D, started day D0): mayor rating=… max=… min=… aura tracts=… sum=… [..] base
       tracts with data=… hotspots=… riot roll=… lots=…; inputs all read | not available (neutral): …`), `aura
       Write -> 1`, `aura Read -> 1`, `aura Shutdown (D days, M months, C cycles)`, and `aura: riot roll won:
       AttemptDisasterStart(x, z) on manager 0x… (called)` when a riot starts. Its game calls have their own
       switches (default `1`, environment `FC4_AURA_<KEY>`): `aura_lots`, `aura_lot_populations`,
       `aura_occupants`, `aura_budget`, `aura_ordinance`, `aura_land_value`, `aura_inputs`, `aura_grid_writes`,
       `aura_posts`, `aura_history`, `aura_disaster`, `aura_strikes` (see `plugin/FC4Config.h`).
     - `flora`: the flora simulator (`cSC4FloraSimulator`, flora spec). The trees stay the game's own
       `cSC4FloraOccupant` objects, created through GZCOM (class `0xA9C05C85`) and handled only through
       `cISC4Occupant`, `cISC4FloraOccupant` and the occupant manager. The flora type registry is process-wide (§3.2):
       its records are built once, at the first `Init` of the process, from the flora exemplars in the resource
       manager's order, and `GetStaticFloraData` hands out a raw block in the game's layout (docs/abi/layouts.md,
       0x178 bytes), because the flora occupant, the flora tool and the terrain brush read it and count its
       references directly. FreeCity4 keeps one reference on every block for the life of the process, so no block
       (or the exemplar it holds) is ever freed. The registry getters (slots 7-14) are answered from any thread,
       from a snapshot. Every other call runs under one process-wide lock; a call from another thread is counted
       (`offMainThread=`) and logged once. The random generator is seeded from the wall clock and the process clock
       at construction (§8). Burnt trees (§6.8, §6.9) keep one reference each until they are cleared.

       Not yet done (logged once):
       - the obliteration save and restore (§10.3): `cIGZPersistDBSegment`'s record I/O is not in docs/abi.
         `SaveObliterated` writes nothing and returns true; an obliterated city's trees are not re-created.
       - the application shutdown callback (§4.1 step 2): the records are kept for the life of the process.
       - `GetFloraTypesByFilter` with a non-null filter: `cISC4FloraTypeFilter` is not in docs/abi, so nothing
         is inserted.

       Status lines, at `Init`, at `Read` and every month: `flora month yyyy-mm-dd: flora=N (young …, maturing …,
       mature …, burnt …, dying …, other …; types T; not flora …); planted=…(+…) spread=…(+…) aged=…(+…)
       reborn=…(+…) dying=…(+…) burnt=…(+…) cleared=…(+…) refused=…(+…) removedMonth=… removedOther=… moved=…
       created=… createFailed=…; burntList=… enabled=… propagation=… …; registry loaded=… map=… types=… families=…
       blocks=…/… recordsWithoutBlock=…; reentrant=… queued=… lockWaits=…/… offMainThread=…`.

       The counters:
       - The census (`flora=`) is every flora occupant in the occupant manager, by stage.
       - `(+…)` is the change since the previous line.
       - `planted` counts trees planted outside the monthly pass (the tool, lots, cheats); `spread` counts seeds
         taken during it.
       - `refused` counts inserts the occupant manager refused.
       - `cleared` counts burnt trees removed during `SimNewDay`.

       Other lines: `flora Read -> 1`, `flora Write -> 1` and `flora Shutdown (D days, M months, N messages)`.

       Its game calls have their own switches (default `1`, environment `FC4_FLORA_<KEY>`): `flora_occupants`,
       `flora_create`, `flora_manager`, `flora_queries`, `flora_terrain`, `flora_weather`, `flora_records`,
       `flora_posts`, `flora_cheats`, `flora_exemplars`, `flora_obliteration` (see `plugin/FC4Config.h`).
     - `landfill`: the landfill developer (`cSC4LandfillDeveloper`, landfill spec). It reports the free capacity
       to the pollution simulator (`SetGarbageCapacity`), moves garbage out of its supply into the heaps
       (`GarbagePutInLandfill`), sets the active-landfill flag of every tile cell, levels the zone through the lot
       developer, books line item 30 of the sanitation department, and places the zot prop, all through the game's
       interfaces. The getters are answered from any thread (another thread waits up to 20 ms for the object's
       lock, then answers from the last snapshot and counts a fallback); `DoMessage` waits for the lock on every
       thread and logs the first few messages from a thread other than the main one (`landfill: DoMessage 0x…
       on thread … (not the main thread); it waits for the lock`). Landfill tiles are game
       `cSC4LandfillOccupant`s made through GZCOM with their tile exemplar attached (`SetDefaultExemplar`), so the
       model is drawn, the query tool shows them, the maintenance reads each tile's cost from its exemplar and the
       game saves them; a tile that cannot be made is kept in FreeCity4 (the cell keeps pollution's
       active-landfill flag; a load rebuilds only those). No interface of the tile answers the exemplar-holder QI
       `0x0AC2B5F7` (LG27), so `SetDefaultExemplar` is called, as the original does, on the holder at the
       occupant + `0x20` (docs/abi/layouts.md), after checking that `AsOccupant` and `AsPropertyHolder` return
       the documented sub-objects (+`0x04`, +`0x0C`). The first tile made is logged in full (`landfill: game tile
       made (first one; …): tile 0x… orientation … at cell (x, z), position (…); exemplar holder at the occupant +
       0x20 (LG27, …); GetDefaultExemplar returns it; tile cost 0xEA54D286 … found; type 0x6A11BD2B …; Init
       true, IsInitialized yes; bounding box …; before the insert (LG29): flag 1 clear, AsPropertyHolder
       non-null; InsertOccupant true; flag 1 set`), and so are the first four failures, each with the step that
       failed and what every step before it gave (`landfill: WARNING game tile failure N: tile … not made: <step>.
       Steps: exemplar (0x6534284A, 0x07BDDF1C, 0x…) found; GZCOM GetClassObject(0x8A22BEBE, 0x2A15F262) made
       one; AsOccupant ok (object +0x4), AsPropertyHolder object +0xC; exemplar holder …`); a refused insert names
       its LG29 case (flag 1 already set, a null `AsPropertyHolder`, or already listed in its first cell) and how
       many landfill occupants the cell lists. Later failures are counted in `tilesFailed`.
       New landfill cells get the landfill ground texture (a
       usage-1 `cSC4LotBaseTextureOccupant`). At `Init` the log says how the month agent is ordered (`landfill:
       month agent registered with …`; with the game's simulator a `WARNING` when the landfill would run before
       pollution or the budget). Left out and logged once: the zone effects (garbage trucks and seagulls, §4.10,
       rendering only), the zot's orientation (the view rotation of §4.7.5 is not read: 0), the 3D view level,
       and `GetLandfillZoneFromCell` (null, which §9 allows). The log has a status line at `Init`, `Read` and
       every month from the first (`landfill month: init=… loaded=… zones=… connected=… cells=… heaps=… valid=…
       active=… stored=… capacity=… used=… free=… reported=… moved=… supplyDelta=… maintenance=… activeCells=…
       tiles=… rebuilt=… gameTiles=… gameTileFailures=… zots=… department=… reentrant=… fallbacks=…
       offThreadMessages=… placeZones=… zotsPlaced=… zotsFailed=… zotsRemoved=… occupantsRemoved=…
       tilesInserted=… tilesFailed=… textures=… texturesFailed=… texturesRemoved=… levellings=… history=…
       months=…`), `landfill Write -> 1`, `landfill Read -> 1`, `landfill Shutdown: M months, F fallbacks` and
       `cFC4LandfillDeveloper destroyed (F fallbacks)`. A cell region whose cell map does not match its
       rectangle is ignored and logged once
       (`landfill: WARNING a cell region does not have the documented layout …`). Its game calls have their own
       switches (default `1`, environment `FC4_LANDFILL_<KEY>`): `landfill_terrain`, `landfill_levelling`,
       `landfill_pollution`, `landfill_budget`, `landfill_zoning`, `landfill_occupants`, `landfill_props`,
       `landfill_traffic`, `landfill_history`, `landfill_effects` (see `plugin/FC4Config.h`).
     - `residential`: the residential simulator, education and health included (`cSC4ResidentialSimulator`,
       residential and residential-education-health specs). All 60 interface slots, the class slots 60-63, the
       message target and the record (version 7, grids by reference). The AL-only getters (cell and global life
       expectancy, average new age) return the value zero-extended in EAX; the by-value percentages use the hidden
       result pointer. Its grids are the game's own `cSC4SimGrid<T>`, created through GZCOM with the game's class
       IDs, tract sizes and instance IDs. It reads the game only through its interfaces: the BDS building map and
       keys, the growth developers' population maps, pollution, terrain, the simulator date, occupants and their
       properties, the four education and health departments, budget items and line items, ordinances, lot
       centres and the automata controllers; it sends history records and posts the strike messages. An input
       that cannot be read is logged once (`residential: …`) and answers the spec's neutral value. The buzzards
       (visual effects, §5.5 step 2, RG1) are created through the city's effects manager (slot 7
       `CreateVisualEffect("buzzards")`), positioned, started and released; without an effects manager they are
       not shown but counted as created (both draws, the tract's flag), as the original with shipped data. A
       department whose funding cannot be read leaves slot 20's outputs as the caller set them (RG10). Its
       generator's session seed is
       `residential_seed` (env `FC4_RESIDENTIAL_SEED`), else the clock. The log has `residential Init: …`,
       `residential Read: …` (the state), `residential Write -> 1`, a month line from the first day after `Init`
       and at every `SimNewMonth` (`residential month yyyy-mm-dd: pop=N (R$ a R$$ b R$$$ c) EQ=… HQ=… LE=…
       school covered=… buildings=… strike=0|1 hospital covered=… buildings=… strike=0|1 state=S … center x87=c/n
       …`; `c` counts the lot `GetCenter` calls of `n` that changed the x87 stack depth: 0 in the game, the first
       is logged once as `residential: WARNING lot GetCenter (slot 15) changed the x87 stack depth by …`), one line
       per completed monthly cycle (`residential cycle N done yyyy-mm-dd: …`), `residential Shutdown: …` and
       `cFC4ResidentialSimulator destroyed (…)`. Its game calls have their own switches (default `1`,
       environment `FC4_RESIDENTIAL_<KEY>`): `residential_occupants`, `residential_budget`,
       `residential_developers`, `residential_pollution`, `residential_ordinance`, `residential_automata`,
       `residential_grid_writes`, `residential_posts`, `residential_history`, `residential_lots`,
       `residential_terrain`, `residential_effects` (see `plugin/FC4Config.h`). The month line counts the
       buzzards: `effects=asked started=… failed=…`.
     - `development`: the building development simulator and the growth developers, replaced as one unit
       (`cSC4BuildingDevelopmentSimulator`, `cSC4GrowthDeveloper`; building-development and growth-developer
       specs). The BDS's 59 slots, its two message targets and its record (version 4); each growth developer's 31
       slots (full-EAX 0/1 for slots 10 and 28, AL for 19-22, ST0 for 24, AX for 29), its message target and its
       record (version 1, five grids by reference). The containers the game reads keep the game's layouts: the
       growth and decline lists (B vectors of 12-byte items), the style map, the developer map and the catalogue
       profiles (STLport maps built with B), the special buildings (an SC4List). Every call into the cores holds
       one process-wide lock; another thread waits at most 20 ms and then gets the values published after the last
       main-thread call. `development_growth_attempts_per_day = N` is the daily growth budget (§4.4.1; default 0 =
       unlimited) and `development_seed` (env `FC4_DEVELOPMENT_SEED`) the session seed of both generators (default:
       the wall clock). The log has `development Init|Read|Shutdown: …`, `development month yyyy-mm-dd: …`,
       `development decline plan yyyy-mm-dd: …`, `growth 0x<type> Init|Read: …` and one `growth 0x<type> update
       yyyy-mm-dd: growth=… decline=… capacity=… phantom=… size=… …` per finished monthly update. Its game calls
       have their own switches (default `1`, environment `FC4_DEVELOPMENT_<KEY>`): `development_map_writes`,
       `development_occupants`, `development_zoning`, `development_lots`, `development_lot_developer`,
       `development_demolition`, `development_advisor`, `development_posts`, `development_cheats`,
       `development_tract`, `development_city` (see `plugin/FC4Config.h`).
     - `tract`: the tract developer (`cSC4TractDeveloper`, CLSID `0x2990C142`; tract-developer spec), the class
       that grows, declines and populates the lots of a tract for the growth developers. Its 15 slots with the §7.1
       return channels (slot 7 returns 0, slots 12 and 14 `this`, slot 10 the address of slot 9's style vector),
       the agent (Month and Year) and the record (version 3, 19 + 4 x styles bytes). It works with the original
       BDS, growth developers, lot developer and lot manager (or ours) through their interfaces only; the outcome
       list, the style vector and every list or set it makes use the game's allocator B. The configuration filters
       (one per wealth and purpose) are registered with the lot configuration manager and unregistered at Shutdown.
       Every call into the core holds the object's lock; another thread waits at most 20 ms and then answers from
       the values published after the last call. `tract_seed = N` (env `FC4_TRACT_SEED`) fixes the session seed
       (default: the cRZTime clock); `tract_grow_log = 1` (env `FC4_TRACT_GROW_LOG`) adds one line per Grow call
       (`tract Grow dev=… tract=x,z stage=… -> built=… flag=… desired=…/… scored=… codes=… cfg=… building=0x…`).
       The log has `tract Init|Read|Write|NewYear|Shutdown: init=… configured=… style=0x… index=… years=…/…
       styles=… filters=… grows=… built=… … filtersRegistered=…/… configRefs=… fallbacks=…` (`configRefs`: the
       configuration references taken from the lot configuration manager and released after each call, kept
       outside the reference audit) and one line per month:
       `tract month yyyy-mm-dd: grows=… built=… declines=… declined=… populations=… outcomes=code:count,…
       constructions=… reoccupations=… abandonments=… lotsCreated=… lotsDeleted=… demolitions=… style=0x… …
       reentrant=… fallbacks=…` (the counts since the previous line; `reentrant` and `fallbacks` should stay 0),
       followed by the `tract tally (ours) …` line of the tract shadow below (the same format, for comparing a
       replacement run with a shadow run of the original).
       Its game calls have their own switches (default `1`, environment `FC4_TRACT_<KEY>`): `tract_construction`,
       `tract_abandon`, `tract_lot_changes`, `tract_demolition`, `tract_population`, `tract_resources`,
       `tract_filters`, `tract_lot_lists`, `tract_traffic`, `tract_style_map`, `tract_roads`, `tract_family_sets`,
       `tract_agents` (see `plugin/FC4Config.h`).
     - `lotdev`: the lot developer (`cSC4LotDeveloper`, CLSID `0xA990BFE0`; lot-developer spec), the class that
       looks after existing lots: the daily pass (abandonment, condition, zots, business units), construction and
       its lot content (props, flora, base textures), demolition and explosion, the night lights, and the terrain
       helpers (altitudes, slopes, levelling, foundations, development cost) other systems call. Its 65 slots with
       the §11.1 return channels (Release returns 0; slots 15-18 and 27-31 ST0, 19-21 counts, 32 EDX:EAX, 39 and 40
       `&orient`), the message target (+4: the agent at levels 0, 4 and 6, nine messages, the animation tick) and
       the record (version 1; 24 bytes with an empty night deque, its buildings as object references).
       It works with the original BDS, tract developer, lot manager and occupant managers (or ours) through their
       interfaces only. Every call into the core holds the object's lock; references the game's lists give and
       objects made for a call are kept until the outermost call returns. `lotdev_seed = N` (env `FC4_LOTDEV_SEED`) fixes the session seed (default: the
       cRZTime clock); `lotdev_lot_log = 1` (env `FC4_LOTDEV_LOT_LOG`) adds one line per lot the daily pass changed
       (`lotdev lot x,z: state a->b condition c->d zot e->f building=0x…`). The log has
       `lotdev Init|Read|Write|Shutdown: init=… daily=… visited=… vertsX=… extent=…,… cells=…,… night=cursor/len
       mode=… lit=… … lots=… abandoned=… restored=… conditions=… zots=… started=… finished=… demolitions=…
       nightBatches=… refused=… queued=… agents=… tick=… buildings=…/… props=… … iterators=… fallbacks=… faults=…
       listsRefused=… content: …` (counters: ID arrays, flora types, flora and prop adds, completions, and the
       props and flora the configurations listed) and one line per month: `lotdev month yyyy-mm-dd: days=…
       redelivered=… lots=… abandoned=…
       restored=… conditions=… zots=… started=… finished=… demolitions=… nightBatches=… night=…/… mode=…
       buildings=… props=… flora=… textures=… removed=… lights=… refused=… queued=…` (the counts since the previous
       line; `refused` should stay 0). Not specified, so not done (logged once each): retaining walls, the
       burnt-building prop and the rubble decal, network connection nodes, UpdateLotObjectAlitudes.
       Its game calls have their own switches (default `1`, environment `FC4_LOTDEV_<KEY>`): `lotdev_lot_writes`,
       `lotdev_buildings`, `lotdev_demolition`, `lotdev_props`, `lotdev_textures`, `lotdev_terrain`,
       `lotdev_night`, `lotdev_units`, `lotdev_agents` (see `plugin/FC4Config.h`).
     - `commercial`: the commercial simulator (`cSC4CommercialSimulator`, CLSID `0x8990C080`;
       commercial-industrial spec): the CO$$ and CO$$$ proximity grids (the game's own `cSC4SimGrid<uint8>`,
       created through GZCOM or handed over by the save), the four agents, the occupant subscriptions, the
       `DoIdle` state machine and the record (version 1, 19 bytes with both grids). Slot 5 hands out the grid
       pointers published after each `Init`, `Read` and `Shutdown`, from any thread. The log has `commercial
       Init|Read|Shutdown: ok=… init=… dirty=… state=… flag=… grids CO$$=… CO$$$=… …`, `commercial Write -> 1`,
       one `commercial rebuild N yyyy-mm-dd (msg …): CO$$ cells=… edge1=… zeros=…; CO$$$ …` line per finished
       pair of map rebuilds and one `commercial month yyyy-mm-dd: dirty=… state=… rebuilds=… occupants=…
       offices=… days=… idles=…` line per SimNewMonth. Its grid writes have the switch `commercial_grid_writes`
       (default `1`, environment `FC4_COMMERCIAL_GRID_WRITES`).
     - `industrial`: the industrial simulator stub (`cSC4IndustrialSimulator`, CLSID `0xA990C022`): no agent, no
       subscription, the record `01 00`. The log has `industrial Init|Read|Write|Shutdown`.
     - `civic`: the civic building simulator (`cSC4CivicBuildingSimulator`, CLSID `0x0A413481`; civic-buildings
       spec): the month agent, the occupant and lot-state subscriptions, the exclusion IDs, the conditional
       building statuses, the disease-center and maintained-building lists, the Landmarks department, the lot
       condition pass and the record (version 6). Slot 6 hands out a stable `ConditionalBuildingStatus*` per key:
       a mirror entry in the game's layout (the tooltip a game `cRZString` from the game's string allocator) that
       is written back to the core before the core next reads the statuses; without the game's allocator (tests)
       slot 6 answers null and the core keeps every status. The Lua table `civic` is not published (no interface
       rows for the registry). `civic_seed = N` (env `FC4_CIVIC_SEED`) fixes the session seed of the ape escape's
       temporary generators, else the clock. The log has `civic Init|Read|Shutdown: ok=… exclusions=… statuses=…
       centers=… maintained=… landmarks=… …`, `civic Write -> 0|1 (…)` and one `civic month yyyy-mm-dd:
       exclusions=… statuses=… … occupants=…/… lots=…/… writebacks=…` line per SimNewMonth.
     - `parks`: the park manager (`cSC4ParkManager`, CLSID `0xAA3FE6BC`; parks spec): the month agent, the
       occupant subscriptions, the park and zoo lists, the Parks and Recreation line items, the run-down latch,
       the zoo escape (the ZooEscape message `0xAA667CE2` with void1 = the manager and IGZUnknown = the zoo,
       `MessagePost(msg, false)`, then `zoomanji` controllers) and the record (version 2). The tuning exemplar
       "Parks Manager" (`0x6534284A` / `0x67CD5FA1` / `0x8A4805BC`) is handed to the core at the first `Init`.
       `parks_seed = N` (env `FC4_PARKS_SEED`) fixes the session seed of the member generator, else the clock.
       The log has `parks Init|Read|Shutdown: ok=… init=… parks=… zoos=… threshold=… curve=… dept=… latch=… …`,
       `parks Write -> 0|1 (…)` and one `parks month yyyy-mm-dd: parks=… zoos=… funding=… latch=… lot-writes=…
       escapes=… occupants=…` line per SimNewMonth.
     - `propdev`: the prop developer (`cSC4PropDeveloper`, CLSID `0x89C48F47`, city component 10;
       prop-developer spec) and its wealth requester (`cSC4WealthRequester`, class `0x89C49986`, resolved from
       saved records): the Idle and Day agents (the block step), the 13 subscriptions of §3.1, the animation tick
       (priority `0xC0`, the night batches), the requesters (wealth answered by the core, crime and pollution
       held), the night, time-of-day, dated, power, trigger and sign lists, and the record (version 5; a version-4
       record is read and written back as 5). The lighting manager's IsNight is read through the city view's
       renderer (slot 9; question PV15). `propdev_seed = N` (env `FC4_PROPDEV_SEED`) fixes the session seed of the
       member and wealth generators, else the session seed. The log has `propdev Init|Read|Shutdown|Write -> 0|1:
       ok=… scan=… scanner=(…) wealthAdded=… requesters=… night=… …` and one `propdev month yyyy-mm-dd: blocks=…
       (idle …, day …) removed=… created=… posts=… …` line per SimNewMonth.
     - `citydetail`: the city detail manager (`cSC4CityDetailManager`, CLSID `0xEAD4BA36`, city component 51;
       city-detail spec): Init's reduce / increase pass over every lot (the lot list's order) and
       every network lot (IterateAllNetworkLots' callback answers true: §3.4 redevelops every lot), slot 5 and the record (version 0, 8 bytes). The setting is the preferences byte
       (`videoPreferences.cityDetail`, default 3). `citydetail_seed = N` (env `FC4_CITYDETAIL_SEED`) fixes the
       session seed of the pass generator. The log has `citydetail Init: setting=… level a->b pass=none|reduce|increase
       removed=… created=… draws=… network-lots=…` and `citydetail Read|Write -> 0|1 (level …)`.
     - `zonedev`: the zone developer / lot manager (`cSC4ZoneDeveloper`, CLSID `0x498F9B01`; zones.md §1-§11),
       one object with the original's four interfaces at its offsets (`cISC4ZoneDeveloper` +0, `cISC4LotManager`
       +4 in the compiled slot order of §11.3, `cIGZMessageTarget2` +8, `cIGZSerializable` +0xC; city components 7
       and 8). Parcellization (§7), the cell -> lot map, the lot lists of slots 7-10 (B nodes, one reference per
       lot for the caller), CanCreateLot (§8.4), DeleteLot, the messages (§5.2) and the record (version 1, a
       reference per cell) are the Rust core's. The master lot list is kept in the game's raw layout, so the
       safe lot iterators of slot 15 (8-byte C blocks) work as the original's for the budget and traffic code;
       an iterator still registered at Shutdown is orphaned (its list object cleared) and its owner frees it.
       Lots are built from the game's lot pool by the game's lot constructor (`0x006BF430` / `0x006C1E50`,
       docs/abi/direct-calls.md; §8.5.1), then laid on their cells, given their insertion point and their empty
       base texture (§8.5). UpdateZoneType (§7.2.2) dezones the cells a parcellization left without a lot
       through the zone manager's PlaceZone; an inserted network piece joins dead-end streets and crosses rail
       with the street / road tools (§5.2.1, §5.2.2); CanCreatePloppedLot (slot 21, §8.4.1) checks the plop
       footprint, the slope and the configuration's constraint and network objects. Without the two addresses
       (the executable not identified) or with `zonedev_lot_changes = 0` every CreateLot fails cleanly and logs
       `zonedev: CreateLot(x, z, w, h, facing f) refused: …` (three times, then silence). Lots loaded from a
       save are kept, queried, deleted and saved. `zonedev_seed = N` (env `FC4_ZONEDEV_SEED`) fixes the
       session seed (default: the cRZTime clock). Every call into the core holds the object's lock; another
       thread waits at most 20 ms and then gets the lot count / grid snapshot or the neutral answer (a fallback).
       The log has `zonedev Init|Read|Write|Shutdown: version=1 init=… loaded=… real=… lots=… map=XxZ created=…
       deleted=… iterators=… issued=… parcellizations=… dezones=… reentrant=… refused=… built=… streets=… joins=…
       demolitions=… levellings=… removed=… posts=… options=0x… grid=… view=0x… fallbacks=…` and the same line once a month
       (`zonedev month yyyy-mm-dd: …`), one line per parcellization or dezone (`zonedev parcellize|dezone:
       type=… rect=… cells=… flag=… lots=… failed=… deleted=… streets=placed/failed demolish=…`, with
       `[lots refused, see CreateLot]` when a lot was refused) and `zonedev InsertOccupant: …` when an
       inserted network deleted or re-created lots. HighlightParcels hands out no region (`*out = null`); the
       first few CanCreatePloppedLot answers are logged (`zonedev: CanCreatePloppedLot(…) = code`).
       Its game calls have their own switches (default `1`, environment `FC4_ZONEDEV_<KEY>`):
       `zonedev_grid_writes`, `zonedev_messages`, `zonedev_network_tools`, `zonedev_posts`, `zonedev_demolition`,
       `zonedev_streets`, `zonedev_occupants`, `zonedev_terrain`, `zonedev_lot_changes`, `zonedev_growth`,
       `zonedev_levelling`, `zonedev_lot_developer`, `zonedev_traffic` (see `plugin/FC4Config.h`).
   - Each budget behaviour that touches game objects has a switch. The switches are re-read at every city load
     (budget `Read`/`Init`). Each defaults to `1`; `0` makes the behaviour a no-op with a neutral result. The
     matching environment variable is `FC4_BUDGET_<KEY>`. `budget_default = 0` turns every unset switch off.

     | Key | When off |
     |---|---|
     | `lot_scan` | `ForEachLot` sees no lots, so property tax is 0 |
     | `property_writes` | `AddPropertyVariant` and property copies are skipped but reported as written |
     | `property_reads` | properties read as absent |
     | `message_posting` | no budget messages are posted |
     | `history` | no history records or queue lengths |
     | `building_name` | empty line-item names |
     | `exemplar_loads` | the tuning exemplar is not loaded (code defaults) |
     | `disaster` | RiskyMoney never starts the earthquake |
     | `cheats` | the budget's cheats are not registered |
     | `holder_refs` | the budget core holds no references on game objects (references taken while the switch was on are still given back) |
     | `ltext` | LTEXT lookups return "not found" |

     The log shows `config (…): budget switches OFF (n of 11): …` at each load. At budget `Shutdown` it shows how
     many calls each switch skipped.
   - `refaudit = 0` turns the reference audit off. `refaudit_lines = N` sets its verbose line cap (default 2000).
   - **Autotest (unattended bisection):** `autotest_seconds = N` (env `FC4_AUTOTEST_SECONDS`, read at game start).
     N seconds after the first framework tick that follows a city load (`PostCityInit`) the DLL writes one line,
     flushes the log and ends the game with `TerminateProcess(…, 0)`:

     ```
     AUTOTEST VERDICT: clean|corrupt|stalled|ended (foreign_exceptions=…, watchdog_stalls=…, rtc=…, overruns=…,
       ref_warnings=…, julian=…, date=…, real_seconds=…, days_per_second=…, switches_off=…, disabled=…)
     ```

     With `close_dialogs = 1` (the dialog closer below) the line gains `, dialogs_closed=N` right after
     `ref_warnings=…`: the game dialogs the closer ended (by button or by the EndModal fallback). Without it the
     line is unchanged.

     - `clean`: at the deadline, with no foreign exception, RTC failure or out-parameter overrun.
     - `corrupt`: a first-chance exception outside `FreeCity4.dll` (written at once, after its report; the
       process ends 5 s later so a stall can still be logged), or RTC failures / overruns at the deadline.
     - `stalled`: the watchdog saw the main thread stuck for more than 10 s (inside our code, or no tick while
       a city runs). The watchdog thread writes it and terminates. Under autotest the no-tick threshold is
       lowered to `FC4_WATCHDOG_STALL_MS`.
     - `ended`: the game shut the city down by itself before the deadline (for example a bankrupt city; run 69),
       with no corruption seen. The observer sees `PreCityShutdown` `0x26D31EC2` that the autotest did not ask for
       (logged as a WARNING with the last day played), and at `PostCityShutdown` `0x26D31EC3` the run ends: with
       `autotest_end = quit` through `RequestQuit` (verdict at `PostSystemServiceShutdown`), otherwise the verdict
       is written and the process terminated. Its `julian` is the last day played before that shutdown.
     - `switches_off` lists the budget switches that are off, `disabled` the subsystems left to the game.
     - `autotest_unpause = 1` or `2` (env `FC4_AUTOTEST_UNPAUSE`): after the load, resume the city's simulator
       (`Resume` while `IsPaused`) and set that speed, through the simulator's own interface (ours or the
       original), at the first tick and again after 3 s and 10 s.
     - `autotest_end = quit` (env `FC4_AUTOTEST_END`; default `terminate`): at the deadline, quit through the game
       instead of `TerminateProcess`. The main thread's next tick calls `cISC4App::RequestQuit(false, false)` (slot
       10: no dialog, no save), the city and the app shut down as on a normal quit, and the verdict is written at
       `PostSystemServiceShutdown`, so it counts reference warnings from every subsystem's `Shutdown`. Its `julian`
       is the last day played. A quit that has not reached that hook in 90 s is `stalled`; a process still alive
       30 s after the verdict is terminated.
     - The run figures (the harness lane; `plugin/FC4HarnessLogic.h`): `date` is `julian` as `YYYY-MM-DD`,
       `real_seconds` the time since the autotest clock started (`S.mmm`), and `days_per_second` the new days
       (`SimNewDay` messages) seen since then over that time (`X.YY`); `-` where unknown (no clock: the save
       autotest without `autotest_seconds`).
     - `autotest_end_date = YYYY-MM-DD` (env `FC4_AUTOTEST_END_DATE`; tooling): the run ends at the first new day
       on or after that date, as `autotest_end` says (verdict as at the deadline, then `TerminateProcess`, or the
       quit at the next tick). It is checked inside the observer's `SimNewDay`, so a terminate run stops on that
       very day even under the turbo clock. `autotest_seconds` stays a timeout: whichever comes first ends the
       run; with an end date alone the autotest is on with no timeout. A city loaded after the end date ends at
       its first new day. A script `save` still waiting for its tick at the end date runs first (the verdict then
       comes at that tick: `...; the script's waiting action(s) first, then the verdict and TerminateProcess at the
       next tick`). Not a date: a WARNING and ignored. Lines:

       ```
       autotest: end date 2001-01-01 (julian 2451911): the run ends at the first new day on or after it
       autotest: clock started; verdict at 2001-01-01 (autotest_end_date), no timeout
       autotest: end date 2001-01-01 reached at the new day 2001-01-01 (julian 2451911), 29.312 s after the clock
         started; verdict, then TerminateProcess
       AUTOTEST VERDICT: clean (…, julian=2451911, date=2001-01-01, real_seconds=29.312, days_per_second=12.48, …)
       ```
   - **Fixed scheduler** (`sim_fixed_step = 1`, env `FC4_SIM_FIXED_STEP`; **off by default**):
     golden tooling uses our simulator with every other subsystem original (`enable = simulator`,
     `disable = all`, `pacing = original`, all shadows/compares off). The scheduler clock advances by
     one virtual millisecond per agent delivery; a frame has a fixed 64-delivery ceiling, independent
     of real time, speed, `sim_turbo_frame_ms` or the frame budget. Each new day ends the frame so
     observer actions and deferred saves get a tick before another day can open. Empty/paused
     frames stop after 4096 scheduler clock reads. This implies turbo's ready day gate, including
     before ConnectionsReady; use `popup_dialogs = 0`. It does **not** change the original agents'
     own wall-time slices, RNG seeds or framework animation ticks. Measure residual variance;
     this switch alone does not promise deterministic original subsystems. No ABI/save change.
   - **Turbo clock** (`sim_turbo = 1`, env `FC4_SIM_TURBO`; off by default; `plugin/FC4Turbo.h`; a *tooling
     deviation* from simulator.md §5, our simulator only): days are no longer paced by msPerDay. The day gate's
     `ready` (§5.2 step 3) is always true, so a day opens as soon as its agents have finished the previous one;
     the frame budget is `sim_turbo_frame_ms` (env `FC4_SIM_TURBO_FRAME_MS`, 1..10000, default 250) at any
     speed while running (the §5.1 value while paused); pause and hidden pause stop time as before.
     `sim_turbo_until = YYYY-MM-DD` (env `FC4_SIM_TURBO_UNTIL`) switches it off at the first new day on or after
     that date. Off, nothing changes (the conformance vectors all pass again with turbo configured off).
     After each city load the request (from `sim_turbo` or the script's `turbo on`) is **held**, days paced as
     usual, until that load's first `ConnectionsReady` `0x6AC284F3` reaches the observer (after `popup_dialogs`
     has handled it: the first in-game turbo run reached an advisor dialog before `ConnectionsReady`, so
     `popup_dialogs = 0` came too late and the dialog's hidden pause held the city). If it never comes, the request
     engages after `sim_turbo_wait_days = N` game days (env `FC4_SIM_TURBO_WAIT_DAYS`, 0..3650, default 30; 0 = no
     fallback) with a WARNING. Lines:

     ```
     simulator: turbo ON (sim_turbo = 1; tooling deviation from simulator.md section 5): days not paced by
       msPerDay, frame budget 250 ms at any speed; until 2001-01-01 (julian 2451911)      (game start)
     simulator: turbo requested (sim_turbo) at 2000-01-01: held, days paced by msPerDay, until the city's first
       ConnectionsReady (or 30 game day(s) without it, sim_turbo_wait_days)                    (each city's Init)
     simulator: turbo ON at 2000-01-03 (after ConnectionsReady; sim_turbo): frame budget 250 ms, until 2001-01-01
     simulator: WARNING no ConnectionsReady within 30 game day(s) of the turbo request (sim_turbo); turbo ON at …
       anyway (sim_turbo_wait_days): …                                                       (the fallback)
     simulator: turbo month 2000-02-01: 31 day(s) in 2.480 s real = 12.50 days/s, 4.03 fps (10 frames)
     simulator: turbo OFF at 2001-01-01 (sim_turbo_until reached); days paced by msPerDay again
     simulator: WARNING sim_turbo = 1 has no effect: our simulator is disabled (disable = simulator), ...
     ```
   - **Perf profile** (`perf_profile = 1`, env `FC4_PERF_PROFILE`; off by default; `plugin/FC4Perf.h`; tooling, no
     behaviour change): where a game month's real time goes. Every agent delivery of our simulator is timed per
     agent and level (the original's agents and ours alike) and every `OnTick` as a whole; named sections time
     the development unit's inner paths (each outermost call into the BDS or a growth developer core with its
     Publish, the calls into the tract developer's `Grow` / `Decline` / `UpdatePopulation`, each growth
     developer state); the grid callbacks, the whole-grid reads and writes (`grid:bulk`) and the getters answered from
     the published copies (`bds:peek`, `growth:peek`) are counted, and every outermost call by the glue function that
     made it (the `calls` line). At each turbo month line (under `sim_turbo`) it logs one set
     of lines and starts over; off, no clock is read. Compare runs at equal settings (`turbo = 1`, one
     `end_date`) and read several months: a loaded machine slows any one month. Lines:

     ```
     perf month 2029-04-01: real=372ms days=31 ms/day=12.0 tick=500.6ms (2 ticks) agents=320.9ms; outside ticks=…
     perf month 2029-04-01 agents: cSC4TrafficSimulator/4 125.2ms (44) cSC4BuildingDevelopmentSimulator/4 …
       (name/level, time, deliveries; top 24 by time; agents of one name and level summed)
     perf month 2029-04-01 sections: bds:call …ms (n) bds:publish … growth:state5 … grid:get N grid:set N grid:bulk N
     perf month 2029-04-01 calls: growth:cFC4GrowthDeveloper::GetSubType N bds:… (top 16 by count)
     ```
   - **Scripted actions** (`script = <path>`, env `FC4_SCRIPT`; off by default; `plugin/FC4Script.h`, the grammar
     in `plugin/FC4HarnessLogic.h`; tooling): a text file of dated in-city actions, one per line,
     `YYYY-MM-DD <action> <args>`, `#` comments anywhere, blank lines allowed. A relative path is relative to the
     folder of `FreeCity4.ini`. The whole file is parsed at game start; every error is logged with its line
     number and a script with any error is refused. Each action runs once, on the main thread, inside the
     observer's `SimNewDay` at the first new day on or after its date (several on one date in file order; dates
     already past all run at the first new day, by date then file order). The exception is `save`: it waits for
     the observer's next framework tick (as the save autotest, not inside a simulator message), and every action
     due after it waits behind it, so the file order holds; under turbo several days can pass before that tick
     (the log then says `(run on <date>)`). Everything goes through the game's interfaces, so it works with the
     original's classes and with ours, and none uses the keyboard or the mouse:

     | Action | Call |
     |---|---|
     | `zone x0 z0 x1 z1 type` | `cISC4ZoneManager::PlaceZone` (city slot 36, slot 11; zones.md §6): commit = 1, skipFunds = 0, checkSlope = 1, skipSizeCheck = 0, checkLotInside = 1, outLots = null; the region (§11.4) in our memory, every cell of the inclusive rectangle set |
     | `pricezone x0 z0 x1 z1 type` | the same with commit = 0 (a dry run) |
     | `dezone x0 z0 x1 z1` | the same with type 0, commit = 1 |
     | `network ax az bx bz type` | a network run from cell (ax, az) to (bx, bz) as network-manager.md §9 gives it: the network manager's `GetNetworkTool(type, true)` (city slot 38, slot 5; a private tool, released after), tool `Init`, `PlaceNetworkOccupants(ax, az, bx, bz, 0, null, 1, -1)` (slot 7, real mode; network-tools.md §3), `Shutdown`; free (nothing is withdrawn), no regional connection, a failed validation places nothing of the run; type 0..12 (network-manager.md §3.1: 0 road, 1 rail, 3 street, 6 avenue, …); the two ends in either order; `-> ok=<AL of slot 7> (PlaceNetworkOccupants type T, real, free)` |
     | `speed 0\|1\|2`, `pause`, `resume` | the city simulator's `SetSimSpeed` / `Pause` / `Resume` (one call) |
     | `turbo on\|off` | our simulator's turbo clock (`sim_turbo_frame_ms`, no end date); `on` before the city's first `ConnectionsReady` is held until it (`-> turbo requested: held …`), `off` also drops a held request; no effect with `disable = simulator` |
     | `digest` | the observer's `DIGEST` line, now |
     | `mark <text>` | a log line |
     | `funds amount` | sets the treasury to 0..999999999 whole simoleons through the city's budget `SetTotalFunds`; logs before/after; only an explicit script action changes funds |
     | `establish <name>` | names and establishes an unfounded city through `cISC4City::SetCityName` / `SetEstablished(true)`; an already established city is unchanged; put this first, dated at or before the loaded date, to bootstrap an empty tile at a framework tick without `SimNewDay` |
     | `disaster fire\|riot x z`, `disaster fire\|riot auto`, `disaster survey` | the disaster layer's manager (city slot 72, layer slot 7, `0x621CDA33` fire / `0xCA380B06` riot) `AttemptDisasterStart(x, z)` (manager slot 6; disasters.md §3.1, §5.5, §6.2): the original's own start path, also with the disasters shadow; `auto` takes the survey's candidates in order (a fire: until the fire count rises; a riot: the first); `survey` logs the candidate cells: buildings on a lot away from the edge with flammability above 0 and fire coverage 0, and road cells with police coverage at most 64 (no call into the layer); `-> … AttemptDisasterStart(x, z) -> instance … fires a->b active=… ongoing=…` |
     | `save [fast]` | `cISC4App::SaveCity(fast)` (`docs/abi/vtables.tsv` `cSC4App::vftable_3` slot 16, pop 4, eax-or-void; foundations.md §3.7): the current city saved in place, a full save unless `fast`, through the same app reference and exception guard as the save autotest; `-> ok=<what SaveCity returned> (SaveCity(full\|fast) in N ms)`; no city: `ok=0 (skipped: no city)` |
     | `terrain x0 z0 x1 z1 h [kind]` | `cISTETerrain::SetAltitude(SC4Rect(x0, z0, x1, z1), h, true, kind)` (city slot 84, terrain slot 49; terrain.md §6): every vertex of the inclusive rectangle set to `h` metres (decimal, 0..5000), its water reset, the redisplay posted with `kind` (0..6, default 1; §8.2); a rectangle past the city's vertices is refused (`skipped: …`; the original does not check it); `-> SetAltitude(x0,z0..x1,z1, h, true, kind): vertex (x0,z0) before -> after`; with `terrain_shadow = 1` the shadow mirrors the call (below) |
     | `port airport\|seaport x z [facing [type]]` | tooling (glue-portdev4; `PortScriptBuild`, `plugin/cFC4PortDeveloper.h`): the stage-0 lot of port type `type` (default 0; the tuning exemplar's list `0x8A270FC3`, the type's lot configuration list `0xCA271781` entry 0) built with its top-left cell at (x, z) as port-developers.md §4.5's swap builds a stage: lot manager `CreateLot` (size from the configuration), `SetLotConfiguration`, `UpdateZoneType`, lot developer `ConstructDefaultLot(lot, cfg, 0, true)`; a site with any lot on it is skipped, and one port of each kind per session is built, so a script can list candidate sites |
     | `tutorial query`, `tutorial task n`, `tutorial next`, `tutorial end 0\|1`, `tutorial event e`, `tutorial reset a b` | tooling (glue-tutorial4; `RunTutorial`): the city's tutorial system (city slot 82; tutorial.md §6) with one call: none (`query`), slot 19 `SetTutorialTaskNumber(n)`, slot 20 `IncrementTutorialTaskNumber`, slot 6 `endTutorial(bool)`, slot 18 `OnTutorialEvent(e)`, slot 17 `ResetTutorialNumAndIndex(a, b)`; then slots 8-12: `-> ret=<AL or -1> isTutorial=.. edgeScroll=.. currentTask=null\|set task=.. guid=0x..`. No `start`: the original's `startTutorial` opens its window. Works with the original class and with `enable = tutorial` |

     `clockmode 0|1|2` enables day/night transition notifications on the city's borrowed 24-hour clock,
     then selects normal/day/night visual time (clock24.md sections 4.5/8, city slot 86, clock slots 29/5).
     It uses the game's interface and runs as a dated script action; it does not change saved preferences.

     `lightinghour 0..23` calls the native lighting manager's `SetTimeOfDay(float hours)` (lighting.md
     section 2.2, slot 6), through the borrowed renderer/manager path of prop-developer.md section 8.5.
     It logs the change result and night flag; the lighting manager produces any dusk/dawn messages.

     The explicit diagnostic action `portprops x z state` switches native spatial props with property `0xC98204B8`
     on the existing lot at (x,z) to state 0 or 1. It logs filter counts, setter calls and before/after states,
     and calls no port-developer method.

     `autotest_stall_grace_ms = 0` is **off by default**. In an autotest only,
     a positive value (capped at 120000 ms) raises both watchdog thresholds and
     the terminating stall limit. The suite scales this allowance, the full-save
     grace and its external time limits using city size and scripted duration.
     It leaves the watchdog and hard deadline active; ordinary runs retain their
     existing thresholds. Set either grace to zero with `suite.py --extra` for
     a diagnostic control run.
     `script_save_grace_ms = 0` is **off by default**. An explicit positive value
     (clamped to 10000..120000 ms) raises the watchdog's stall threshold only while
     a scripted full `SaveCity` executes. Golden runs request 60000 ms because the
     original full save can exceed autotest's ten-second stall limit. Fast saves
     and all other activity keep the ordinary watchdog rules; a full save exceeding
     the grace still fails. No simulation steps or saved fields are changed.

     `save_as <name>` is not available (a parse error that says so). The interface exists
     (`SaveCity(cIGZString const& path, bool fast)`, slot 15, pop 8), but what saving under another path does is
     not documented: whether the city's save path and its region entry move to the new file, and what the region
     does with a second save of the same city tile in its folder. Coordinates are cells 0..4095 with x0 <= x1 and
     z0 <= z1; zone types 0..15 (zones.md §2). Lines:

     ```
     script: loaded '…\script.txt' (script setting; tooling): 3 action(s), 2000-01-03 to 2000-01-05; …
     script: line 2 2000-01-03 zone 10 20 12 22 1 -> ok=1 error=0 (none) cost=… accepted=9 of 9 cell(s)
     script: line 3 2000-01-03 pricezone 30 20 32 22 4 -> dry run: ok=1 error=0 (none) cost=… accepted=9 of 9 cell(s)
     script: line 4 2000-01-05 dezone 10 20 11 22 -> ok=1 error=0 (none) cost=… accepted=6 of 6 cell(s)
     script: all 3 action(s) of '…\script.txt' have run
     script: line 7 2000-02-01 save: waits for the next framework tick (not run inside SimNewDay); the actions due
       after it wait behind it
     script: line 7 2000-02-01 save -> ok=1 (SaveCity(full) in 412 ms)
     script: line 5 2000-01-06 network 4 4 14 4 0 -> ok=1 (PlaceNetworkOccupants type 0, real, free)
     script: ERROR line 2: unknown action 'zonee' (zone, pricezone, dezone, network, speed, pause, resume, turbo,
       digest, mark, save, terrain, disaster)
     script: REFUSED '…\script.txt' (script setting): 1 parse error(s); no action of it runs
     ```

     `error` is PlaceZone's code (1 insufficient funds, 2 landfill on water, 3 too small, 4 too large, 5 blocked);
     `accepted` counts the cells whose bits PlaceZone left set. A date passed while no city ran is caught up at the
     next city's first new day; an action never runs twice in one game session.
   - **Dialog closer** (`close_dialogs = 1`, env `FC4_CLOSE_DIALOGS`; off by default; `plugin/FC4Dialogs.h`; a
     *tooling* switch, docs/specs/modal-dialogs.md §3.2): ends every game modal dialog from inside the game, through
     its window manager, so an unattended run needs no screenshot and no click (several games at once, or a covered
     window, are fine). At `PostAppInit`, before any city load (the edge-reconciliation and missing-plugin-pack
     dialogs come before ConnectionsReady), it subscribes to the legacy message server's modal begin / end notices
     (`0x62BA7B42` / `0x62BA7B43`). A begin notice records the window, its ID and its nesting depth; nothing is
     called inside it. On a later frame (the modal loop keeps ticking the observer) the innermost dialog, 5 frames
     after its begin notice, is checked (`IsWindowValid`, the nest count = depth + 1, the window's modal flag
     `0x1000`) and gets button 0 (`PostMessageToTarget(window, 3, 0x287259F6, 0, 0)`, what a click does). A dialog
     that ignores button 0 (edge reconciliation, missing plugin packs) gets `EndModal(window, 0)` 30 frames later,
     after the same checks. Only button 0 is ever sent (never the city-situation popup's 1, 6 or 7); `SC4GameDelay`'s
     invisible modal (recognised by the `cSC4WinGameDelay` vtable, compat table) is left alone; a window that is gone
     is never called; Win32 message boxes are out of reach (no frame runs while they are open; detect them from
     outside). Button 0 answers "no" / cancel, so a quit, close-city or go-to-region prompt the harness asks for
     itself is refused; the autotest's quit asks nothing. Log (one line per dialog):

     ```
     dialogs: subscribed to the modal begin / end notices 0x62BA7B42 / 0x62BA7B43 (legacy message server 0x64063387) at PostAppInit
     dialogs: closed 0x2A5CFB2C (window 0x0F3A1C40, depth 0) by button after 3 frame(s)
     dialogs: closed 0x6A4D0A59 (window 0x0F3A2E80, depth 0) by EndModal fallback after 2 frame(s)
     dialogs: left 0x… (window 0x…, depth 0) alone: SC4GameDelay's invisible modal ends itself
     dialogs: 0x… (window 0x…, depth 0) ended by itself after K frame(s)
     dialogs: WARNING could not close 0x… (window 0x…, depth N): the window is no longer valid (IsWindowValid
       false) and no end notice came; record dropped, nothing called on it
     dialogs: WARNING could not close …: not the current modal window for 30 frame(s) (nest count …, expected …;
       modal flag …); still checked each frame
     dialogs: WARNING could not close …: no end notice 30 frame(s) after EndModal; nothing more is called on it
     ```

     K counts frames from the button press (or `EndModal`) to the end notice. `0` (or unset) changes nothing: no
     subscription, no call, no log line, and the autotest verdict keeps its format. As §3.4 says, the harness should
     not use the `GameDelay` automation command or the debug plop-all commands while the closer is on; if it does,
     the closer leaves the delay's window alone on the compat table's executable, and on any other one (it logs
     `compat: cSC4WinGameDelay vtable NOT available`) would end the delay early.
   - **Clock probe** (`clock_probe = 1` or `2`, env `FC4_CLOCK_PROBE`; off by default; `plugin/FC4ClockProbe.h`):
     the M1 clock differential. From game start it logs every message the simulator posts (`CLOCK post day|week|
     month|year j=… c=…`, `CLOCK post pause|hidden|emergency|speed d1=…`), and at every city load it registers an
     agent at the day..year levels and logs each delivery (`CLOCK agent day|week|month|year j=… c=…`). With `2`
     the week agent reports "not finished" once per week, so §5.3's repeat delivery shows as `… again`.
     `tools/clock_diff.ps1 -A <original log> -B <our log>` checks both streams against simulator.md §2.3/§5.2 and
     compares the two runs from the first day both saw (runs 56/57: identical over 240 days).
   - **Growth probe** (`growth_probe = 1`, env `FC4_GROWTH_PROBE`; off by default; `plugin/FC4GrowthProbe.h`):
     the calibration of `growthAttemptsPerDay` (building-development.md §12), on the ORIGINAL building development
     simulator and tract developer (enable neither). At game start, on the identified 1.1.641 executable only, and
     only if the original tract developer vtable (`0x00AB3118`) holds the original `Grow` (`0x0070D130`) in slot 5,
     it patches that one entry with a counting forwarder (same `this`, arguments and EAX; callee-pop 12; integer
     work only) and puts the original back at `PreAppShutdown` (and at DLL detach). Otherwise it logs why and
     patches nothing. One line per posted `SimNewDay`, then a summary at each city's shutdown:

     ```
     growth probe yyyy-mm-dd: attempts=A qualifying=Q cut=0|1 elapsed_us=E per_attempt_us=P lots=L population=N
       max_us=M cycles=C
     growth probe summary (city shutdown): days=D; attempts min/median/max=…; cut days=K, attempts on cut days
       min/median/max=…; qualifying min/median/max=… (unknown on U day(s)); mean_us_per_attempt=…; …
     ```

     `attempts` = `Grow` calls since the previous line; `qualifying` = developers in the BDS developer map (slot 34)
     with trunc(active demand) > 5, read when the line is written; `cut` = attempts < qualifying (approximate: a
     pick that ends without a `Grow` call is not counted); `elapsed_us`/`max_us`/`cycles` = the time inside the
     calls; `lots` = lot manager slot 16; `population` = the residential simulator's. `-` = not readable. At each
     city load a line says whether the city's tract developer and BDS are the original classes. With
     `FC4_LOG_LEVEL=2` the x87 leftover guard also runs at the forwarder's exit.
   - **Session seed** (`session_seed = N`, env `FC4_SESSION_SEED`, decimal or `0x…`; *tooling*; `plugin/FC4Seed.h`,
     `core/src/seeding.rs`; foundations.md §6.3 *[choice]*): the plugin picks one 32-bit session seed `S` per
     process and every core of ours seeds each of its generators with `derive(S, K) = S XOR K` (0 and `0xFFFFFFFF`
     moved by XOR 1), `K` by the generator's kind: a file-local generator the original's constant (budget
     `0x6C09C3D7`, crime `0x6C09C3F6`, pollution effects `0x4C09C4D3`, building development `0x0C09C0B8`, growth
     `0xAC09C0E8`, tract `0x4C09C113`, lot `0x6C09C0F4`, zone developer `0x8C09C68A`), a member generator seeded
     from the clock the owning class's GZCLSID (residential, weather, flora, aura, fire, police, plumbing,
     traffic, parks), a temporary seeded from the clock the GZCLSID XOR its creation count (civic monthly,
     neighbours' proposals, a growth decline key whose coordinate seed is `0xFFFFFFFF`); flammability keeps its
     constant 0 and its zero-state reseed takes `derive(GZCLSID)`; coordinate-seeded temporaries are unchanged
     (tract (0, 0) uses 1); `rand()` stays unseeded. The budget's, the crime simulator's and the pollution
     effects' generators are process-wide like the original's statics (RN3, `seeding::process_super`): the first
     create of the process seeds them and later city loads continue their streams. Without the key `S` is the `cRZTime` wall clock at game start
     (seconds since 1901-01-01, `time() + 2177452800`), like the clock the original seeds from. A subsystem's own
     `<key>_seed` (`crime_seed`, `development_seed`, `tract_seed`, `lotdev_seed`, `zonedev_seed`, `civic_seed`,
     `parks_seed`, `residential_seed`, `neighbors_seed`, `traffic_seed`) still overrides `S` for that subsystem.
     `fc4_seed_derive(S, K)` (fc4.h) returns `derive(S, K)`. One line at game start:

     ```
     session seed: S = 0x12345678 (session_seed = 0x12345678, FreeCity4.ini; tooling: every generator of ours is
       seeded with derive(S, K), foundations section 6.3, so runs with the same S and inputs draw the same)
     session seed: S = 0xEDC0FFEE (the cRZTime wall clock at game start, like the original; session_seed = N in
       FreeCity4.ini fixes it); every generator of ours is seeded with derive(S, K), foundations section 6.3
     session seed: WARNING session_seed = 'abc' (FreeCity4.ini) is not a 32-bit number; using the clock
     ```

     Two runs with the same `session_seed` give our generators the same seeds, and with deterministic pacing
     (below) nearly the same city, but not yet the same one. Runs of the 27 regress replacements of run 121 on
     Konradshohe with `session_seed = 0x5EED1234` (lane detpacing, 2026-10-16):
     - before deterministic pacing, two turbo runs to 2032-01-01 differed in the `DIGEST` from the first month;
     - with it, two turbo runs to 2032-01-01 give the same `DIGEST` until 2027-02 (`dp-final1` / `dp-final2`;
       until 2026-12 and 2027-05 in earlier pairs), and a speed-2 run without turbo to 2028-01-01 gives the turbo
       run's `DIGEST` until 2027-05 (`dp-s1` / `dp-c1`); the `ordinance` and `zones` month lines are the same in
       every run, and
       `demand`, `landfill`, `plumbing`, `power`, `protection` and `neighbours` are the same at speed 2 and in
       turbo to 2028;
     - the first differences come from the original's frame-driven or clock-seeded subsystems still in the city:
       the occupant messages counted by commercial, civic and parks (transient occupants of the automata and the
       construction crew; from the third month), the occupants whose flammability the flammability scan reads
       (`wateredQueries`, one query a month), the traffic route counts by travel type (`routes applied … by travel
       type`, from the seventh month), and the props the original prop developer places and asks crime to select
       (its generator is seeded from the clock; crime's `SelectProp` draws crime's own generator). The first
       difference in our state is one tree more or less that flora plants on a cell (after 9 to 12 months), which
       moves the flammability scan and the pollution absorption; small differences reach population (R ±4) after
       11 to 16 months and then every subsystem. `rand()` (state 1 at process start) is drawn by the original in a
       frame-dependent order.
   - **Pacing** (`pacing = deterministic | original`, env `FC4_PACING`; `plugin/FC4Pacing.h`, `core/src/pacing.rs`,
     fc4.h "pacing"; simulator.md §5.7 *[choice]*): how the work of our cores is bound to game days. The default
     is `deterministic`, except that with any `<x>_shadow` on and `pacing` unset it is `original` (the shadows
     compare our cores with the original's under the original's pacing; `pacing = deterministic` with a shadow on
     logs a warning). Read once at game start, before any core exists (`fc4_pacing_set_process`); every core
     captures it when it is constructed. Under `deterministic`, and only while our simulator runs:
     - every day our simulator opens has a **settle point**: when the day's levels 2..7 have drained (the end of
       the agent walk), each enabled idle agent of ours (its vtable lies in FreeCity4.dll) gets one `SimIdle`
       (data3 22) and runs its cycle to completion in the core: aura, growth developers, plumbing, the demand
       census, land value, crime, power, commercial, the BDS rebuild (which answers false while it waits for a
       developer's map update: it is delivered again after the other idle agents, at most 4 passes); the
       neighbours' push runs there too. The frame idle passes reach only the original's idle agents; the next
       day opens only after the settle point;
     - the day runs from open to settle in one simulator tick: the levels of an open day ignore the frame budget
       (an open day is capped at 10 s of real time), so no frame, idle pass or message-server pump falls inside
       a day;
     - the clock is never read for slicing: the §5.5 adaptive helpers (aura lots, growth population pass) and
       the demand census run without a time limit (an item cap only), land value runs with an unbounded tract
       count, the history warehouse's yearly thinning thins its per-pass series on every delivery (no 5 ms
       limit); the lot developer's daily budget and the BDS's daily growth budget (building-development §4.4.1)
       were already item counts;
     - the original's prop developer (one block step per `SimIdle`, prop-developer.md §3.3) is paced too: 20
       `SimIdle` at each settle point (a day at the middle speed: 1000 ms per day at 20 frames per second), none
       from the frame idle passes; the original's other idle agents keep the frame idle passes;
     - the simulator's posted `SimNewDay` / `SimNewWeek` / `SimNewMonth` / `SimNewYear` and our lot developer's
       lot `StateChanged` (`0xE8BDA2E7`) posts made while a day is open are held and sent (`MessageSend`, in
       posting order, with any they cause) at that day's settle point, so every subscriber (ours, the
       original's, the observer's `DIGEST` and month lines) sees them before the next day's agents, however
       many frames the day took; then the message server's tick runs at the settle point until its post queue
       stops shrinking (at most 64 ticks), which delivers every other post of the day (the original's lot
       `StateChanged` among them). One message the original posts again at every tick stays queued (every
       settle point in the runs; it reaches its target at the frame's own tick).

     `original` keeps the clock- and frame-bound behaviour (one step per idle delivery, ms budgets, posts
     delivered by the message server's next pump). Lines (game start, our simulator's `Init`, every month):

     ```
     pacing: deterministic (the default; simulator.md section 5.7 choice: our idle cycles complete within their
       day at its settle point, item counts instead of ms budgets, posted day/month/StateChanged messages sent
       at the settle point; pacing = original keeps the clock-bound pacing)
     pacing: original (the default with a shadow on: aura_shadow; the original's frame- and clock-bound pacing,
       simulator.md section 5.7 items 1-4, for the conformance shadows; pacing = deterministic forces the default)
     Init: ok=1 speed=… … pacing=deterministic
     pacing: the original's idle agent 'cSC4PropDeveloper' (…) gets 20 SimIdle per day at the settle point
     pacing: days settled N, messages deferred D, sent at settle S, released unsent U, message-server ticks T,
       settles with posts left L
     pacing: the post queue still holds 1 message(s) after 2 message-server tick(s) at the settle point of julian …
     post SimNewMonth 0x66956816 d1=… (yyyy-mm-dd) d2=0 d3=… -> deferred true
     ```
   - **Monthly digest** (also at `FC4_LOG_LEVEL=1`; `digest = 0` turns it off): at every `SimNewMonth`, one line
     for diffing the original against FreeCity4 on the same save, read only through public interfaces:

     ```
     DIGEST julian=… (yyyy-mm-dd) funds=… population R/C/I=…/…/… demand
       R$/R$$/R$$$/Cs$/Cs$$/Cs$$$/Co$$/Co$$$/I-R/I-D/I-M/I-HT=…/… budget income/expense=…/…
     ```

     R is `cISC4ResidentialSimulator::GetPopulation`; C and I are the summed `QuerySupplyValue` of the
     commercial and industrial demands (jobs); demand values are `QueryDemandValue` of
     `cISC4DemandSimulator::GetDemand(id, 0)` (`-` when the game has no such demand).
   - **Observe** (`observe = 1`, env `FC4_OBSERVE`; default on in checked builds, off in release builds): at every
     `SimNewMonth`, and in every mode, one more line with the month's money:

     ```
     observe month yyyy-mm: date=yyyy-mm-dd julian=… funds=… income=… expense=… tax[0..11]=…/… dept_expense=0xID:…,…
       demand R$/…/I-HT=…/… population R/C/I=…/…/…
     ```

     `income` / `expense` are the budget's total monthly income and expense, `tax[t]` is `GetTaxIncome(t)` for the
     budget's 12 tax types by index (their zone and wealth names are not verified, so the line does not name
     them), `dept_expense` is `GetTotalExpenses` of every department of every budget group (sorted by ID; `-`
     without the game's allocator). At the first tick after every city load it also writes
     `observe snapshot (after load): date=… julian=… funds=… fundings=0xID:pct,…` (the date from the
     simulator, `pct` the stored `SC4Percentage`). Only slots checked against `docs/abi` are called; the list is
     at the top of the observe section of `plugin/FC4Observer.cpp`.
   - **Save autotest** (`autotest_save_after_months = N`, env `FC4_AUTOTEST_SAVE_AFTER_MONTHS`): at the first tick
     after the N-th `SimNewMonth` since the city load, write `observe snapshot (before save): …`, save the city in
     place with `cISC4App::SaveCity(bool)` (a full save; `autotest_save_fast = 1` for a fast one), log
     `autotest: save after N month(s): SaveCity(full) returned 1 in … ms`, then write the verdict and end the
     process like the autotest. It needs no `autotest_seconds`; with `autotest_unpause` (which does) the city
     runs by itself, so set `autotest_seconds` beyond the N months. A second run that loads the saved city
     (autoload) logs `observe snapshot (after load): …` to compare with the line before the save.
   - **Popup dialogs off** (`popup_dialogs = 0`, env `FC4_POPUP_DIALOGS`; not set = the game is left alone): for
     unattended runs, where an advisor popup holds the city (a hidden pause of the simulator) until someone clicks
     Close. FreeCity4 calls `cISC4App::SetPopupDialogsEnabled(false)` (slot 24) when **ConnectionsReady**
     (`0x6AC284F3`) arrives, a few frames after each city load, and sets it true again at the next city load
     (advisors spec §6.8.1, option 1; foundations §11.6). Not earlier: with popups off, the terrain edge
     reconciliation is skipped and the regional city never posts ConnectionsReady, so power, plumbing and the
     neighbours simulator stay disabled all session (runs 58–64 did this at `PostAppInit`; run 64 vs 65 shows it).
     Advisor popups are then dropped without pausing, and the generic notification,
     confirmation and scripted modal dialogs return without asking (false / 0; the quit, go-to-region, obliterate
     and budget-failure prompts use the scripted one, and what they then do is not in the spec). City-situation
     popups still show and pause. The log proves it
     took effect by reading the getter (slot 23) before and after:
     `popup dialogs: disabled (cISC4App::SetPopupDialogsEnabled(false); was enabled=1, now=0) at ConnectionsReady`.
     `popup_dialogs = 1` calls it with `true`; any other value logs `config: WARNING popup_dialogs='…' … is not
     0/1` and leaves the game alone. Manual fallback without FreeCity4: untick Options → Play Options → "Urgent
     Advice Dialogs" (saved in the user's preferences; advisor popups only).
   - **Budget shadow** (`budget_shadow = 1`, env `FC4_BUDGET_SHADOW`; off by default; `budget_shadow_lots = N`,
     default 6): a same-process differential of property tax. It needs **our simulator and the original budget**
     (`disable = budget`); with any other combination it logs `budget shadow: requested … but INACTIVE` at game
     start. Our simulator wraps its `SimNewMonth` delivery to the original budget's month agent: just before it,
     FreeCity4's own `ComputeTax` (`fc4_budget_shadow_tax`, the core code our budget runs) scans the live lots with
     the original's tax rates; just after it, the original's `GetWeightedAssessedTaxValue(t)`, `GetTaxIncome(t)`,
     `GetTaxIncome()`, year-to-date income/expense and funds are read and compared. Read-only: the scan's host
     table has no write callback, and only getters are called on the game. One line per month:

     ```
     budget shadow yyyy-mm-dd: tax orig/ours=a/b,…(12 types; ! = differs) ; total orig/ours=…; income orig/ours=…;
       expense orig/ours=…; net orig/ours=…; lots=N; mismatches=M [tax 3,4 total income expense]; months=K clean=C
     ```

     A type mismatches when its assessed sum or its `GetTaxIncome(t)` differs. `ours` income is our tax plus the
     ordinance, neighbour-deal and department income the settlement books; `ours` expense is ordinance + neighbour
     deals + the departments' expenses after the original's settlement + bond payments (so income/expense check
     the settlement's composition; only the tax is computed by our core). At the January 1st settlement the
     year-to-date values are reset, so income/expense read `-` there and only `net` (the funds delta) compares.
     For each mismatched type: `budget shadow … type T: assessed orig/ours=…/… (orig-ours=D); tax …; ours:
     counted=…, not-livable=…`, then up to N lots of that type, (x,z), size, status, purpose, building/lot wealth,
     value, land value, assessed and tax, those whose assessed value equals |D| first, then not-livable ones.
   - **Demand shadow** (`demand_shadow = 1`, env `FC4_DEMAND_SHADOW`; off by default; `demand_shadow_nodes = N`,
     default 8, 0..64): a same-process differential of RCI demand. It needs **our simulator and the original demand
     simulator** (do not enable `demand`); otherwise it logs `demand shadow: requested … but INACTIVE` at game start.
     Our simulator wraps two deliveries to the original demand simulator's agent (`cSC4DemandSimulator`):
     - `SimNewDay` (demand §10.2: `DoIdle(5)`, then the propagation `DoDemandIndexLinks`, the only step that changes
       committed and active demand): just before it every node of the graph is read through `cISC4Demand` getters
       (supply, demand, pending supply/demand, active, min/max, tax/economy modifiers, cap, region use); just after
       it the same, plus `GetNeutralTaxRate()` (only when `0x1001`, `0x3001` and `0x4001` exist). FreeCity4's own
       propagation (`fc4_demand_shadow_propagate`, the core code our demand simulator runs, on a graph built once from
       the same demand exemplars) then runs over the states read before, and every node's six values are compared
       bitwise. All propagation state is in the nodes, so seeding ours from the original each day keeps the inputs
       identical. If the `DoIdle(5)` slice set the 12 total nodes' modifiers (census state 0, §8.1), ours uses the
       original's new values (noted). If a census finished inside it (census nodes moved although they only fold
       their own pending values), their pending values are inferred as post − pre and ours runs again (noted, and
       counted as `inferred=`: an ulp there is not conclusive).
     - `SimIdle`: the 12 total nodes' tax/economy modifiers are read before and after; when one changed (census
       state 0), FreeCity4's §8.1 (`fc4_demand_shadow_modifiers`) from the census sums read before is compared. The
       CO/I economy bonuses are not readable and are taken as 0 (§15 Q5).

     Read-only: the core's host table (`DemandReadOnlyHost`) has the exemplar reads and the §8.1 getters only (no
     node creation, reference counting, posts or history writes), and only getters are called on the original;
     both halves run under the structured-exception guard. Lines (always written):

     ```
     demand shadow yyyy-mm-dd: demand orig/ours=a/b,…(R$ … I-HT local demand, as the digest; ! = bits differ);
       active orig/ours=…(the 12 total nodes); neutral orig/ours=…; nodes=N; mismatches=M [0xKEY,…] max|orig-ours|=…
       maxulps=…; passes=P transfers=T; updates=K clean=C [inferred=I] [; notes]
     demand shadow yyyy-mm-dd node 0xKEY (ru R): supply orig/ours=…[!(orig-ours=d, u ulp)]; demand …; pending supply …;
       pending demand …; active …; cap … | before: …          (up to N nodes, 48 lines per update)
     demand shadow yyyy-mm-dd modifiers (SimIdle|NewDay): tax orig/ours=…(12); econ orig/ours=…(12); neutral=…;
       mismatches=M; checks=K clean=C
     demand shadow yyyy-mm-dd modifiers R$ (0x21010): tax …; econ …; before …; ours: max rate … ratio … curve …
     ```
   - **Pollution shadow** (`pollution_shadow = 1`, env `FC4_POLLUTION_SHADOW`; off by default;
     `pollution_shadow_cells = N`, env `FC4_POLLUTION_SHADOW_CELLS`, default 4, 1..64): a same-process differential
     of the eight pollution grids. It needs **our simulator and the original pollution simulator** (do not enable
     `pollution`); otherwise it logs `pollution shadow: requested … but INACTIVE` at game start. Our simulator wraps
     each `SimNewDay` and `SimNewMonth` delivery to the original's agent (`cSC4PollutionSimulator`): just before it,
     the original's own `Write` (`cIGZSerializable`) goes into a memory stream, and FreeCity4's core loads that
     record, copies the eight live grids it refers to and runs its own day or month on the copies, reading every
     other input (city geometry, tuning, terrain, zones, air traffic, ordinances, deals, power, population,
     landfill, occupant properties, budget items) through a read-only host table; just after it, the record is
     captured again and our grids are compared with the live ones cell by cell, with the saved fields, and with
     the occupant properties and deal usage. Nothing is written to the game (the shadow's host table has no write
     callbacks; its grid table only reads). Not readable read-only, hence not compared (said in the start line):
     the radiation sum (not saved), the posted messages, history and budget values, the visual effects and the
     worst tracts. Lines (always written; longer ones continue on `pollution shadow (cont.)` lines):

     ```
     pollution shadow yyyy-mm-dd day|month: …    a delivery with a difference (at most 8 a month), a rerun with
                                                 the original's tract order, or INCOMPLETE (why); not compared
     pollution shadow month to yyyy-mm-dd: deliveries=N (days D, months M) clean=C incomplete=I reruns=R
       uncached=U; cells=X mismatches=Y; airSrc y/x max|d|=…, waterSrc …, garbageSrc …, radSrc …, airVal …,
       waterVal …, garbageVal …, radVal …; first differing cells: …; fields: …; props compared=… mismatched=…;
       deals compared=… mismatched=… [; incomplete because: …]
     ```

     Success is `clean` equal to `deliveries` and `mismatches=0` on every month line.
   - **Protection shadow** (`protection_shadow = 1`, env `FC4_PROTECTION_SHADOW`; off by default): a same-process
     differential of the fire and police coverage grids. It needs the **original** fire and police simulators (do
     not enable `protection`; otherwise `protection shadow: requested … but INACTIVE`). At every `SimNewMonth` (the
     observer's notification), per simulator, the original's adjusted grid (slot 5), department, strike flag,
     station list, chance of strike, total dispatch count and per-station efficiency and dispatch radius are read
     through its public getters; FreeCity4's core (`fc4_protection_shadow_run`, over a read-only host table: no
     callback that changes the game) rebuilds the unadjusted grid from those stations and the dispatch manager's
     active dispatches (§5.1) and derives the adjusted grid (§2), which is compared tract by tract. The original's
     adjusted grid lags its unadjusted grid by up to one daily pass (§5.3, about 51 fire / 41 police days), so an
     exact match is expected only when our grid was the same at two month ends in a row and no dispatch is active
     (`expectExact=1`); a mismatch then also logs a `WARNING` line. The original's unadjusted int16 grid and its
     cycle accumulators have no getter (logged at game start), so they are not compared. Lines (always written):

     ```
     protection shadow (fire|police) yyyy-mm-dd: cells=N mismatches=M max|diff|=D stations=S dispatches=K stable=s
       expectExact=0|1 first=(x,z):orig/ours,…(8); efficiency mismatches=a/S [(first …)]; radius mismatches=b/S;
       strike=0|1; chance orig/ours=…/…[ MISMATCH]; totalDispatch orig/ours=…/…; avg=… pct=… (original)
     protection shadow (fire|police) yyyy-mm-dd: WARNING M cells differ although the grid should be exact (…)
     protection shadow (fire|police) yyyy-mm-dd: skipped (…)
     ```

     Success: `mismatches=0` on every line with `expectExact=1`, `efficiency mismatches=0/S`, `radius
     mismatches=0/S`, equal chance and total dispatch.
   - **Crime shadow** (`crime_shadow = 1`, env `FC4_CRIME_SHADOW`; off by default; `crime_shadow_cells = N`, env
     `FC4_CRIME_SHADOW_CELLS`, default 8, 0..64): a same-process differential of the crime grid. It needs **our
     simulator and the original crime simulator** (do not enable `crime`); otherwise it logs `crime shadow:
     requested … but INACTIVE` at game start. The shadow follows the original's pipeline (crime spec §5) from our
     simulator's deliveries to its agent (`cSC4CrimeSimulator`): a new or loaded city starts in state 6,
     `SimNewMonth` sets 6 → 0, each `SimNewDay` / `SimIdle` runs one step. It wraps the delivery that runs state 4
     (decay, commit this month's crimes, average): just before, the crime grid (every tract) and the hotspot are
     read; just after, the grid, hotspot, monthly count, average, the incident list (into a vector of ours) and
     each incident's lot rectangle and the zone type at every tract corner. FreeCity4's state-4 grid work (the core
     code our crime simulator runs) is then replayed on the grid read before with the original's incidents, and
     compared cell by cell, with the hotspot, count and average. Only getters are called on the original, its grid,
     the lot and zone managers and lots; both halves run under the structured-exception guard. **Not compared**
     (said in every line): the number of crimes to commit (it needs `ResetUnjailedCriminals`, which has side
     effects), which attempts the police stopped and the type each crime got (the original's random draws), the
     lots tried (the lot manager's generator) and the crime-effect grid (no interface exposes it). Lines (always
     written):

     ```
     crime shadow: tracking the original crime simulator …     (each city)
     crime shadow yyyy-mm-dd: state 4 compared on CxZ tracts (S=s; …): cells mismatched=M max|orig-ours|=D
       [cells (x,z) orig/ours: (x,z) a/b pre p, …]; hotspot orig/ours=(x,z)=v/(x,z)=v (before: …, replayed from …);
       count orig/ours=a/b; average orig/ours=a/b; incidents=I (…); mismatches=N; months=K clean=C; not compared: …
     crime shadow yyyy-mm-dd: state 4 NOT OBSERVED in this delivery …   (the mirror is out of step: resynchronised
                                                                          at the next SimNewMonth)
     crime shadow yyyy-mm-dd: INCOMPLETE (why); not compared
     ```

     Success is `mismatches=0` on every month line (`clean` equal to `months`) and no `NOT OBSERVED` or
     `INCOMPLETE` line. A `!` marks a differing hotspot, count or average.
   - **Aura shadow** (`aura_shadow = 1`, env `FC4_AURA_SHADOW`; off by default; `aura_shadow_cells = N`, env
     `FC4_AURA_SHADOW_CELLS`, default 4, 1..64): a same-process differential of the aura simulator that follows
     the original step by step (aura spec §14.3).
     - **Needs:** the **original** aura simulator (do not enable `aura`) and FreeCity4's simulator (`simulator`,
       the default): the original's deliveries pass through `cFC4Simulator::HostDeliver`. Otherwise it logs
       `aura shadow: requested … but INACTIVE` at game start.
     - **City load** (PostCityInit): the original's own `Write` (`cIGZSerializable`) goes into a memory stream,
       and FreeCity4's aura core starts from that record (Read, then Init: state 1, as the original after a load).
     - **Each step:** every SimIdle / SimNewDay / SimNewMonth delivery to the original's agent is wrapped: its
       pipeline state is read before and after (a raw, read-only s32 at `+0x70` of its `cISC4AuraSimulator`
       pointer, under a structured-exception guard, only while its first word is the original's vtable from the
       compat table, `0x00AB07C8`). When the state moved from s to s', our steps s..s'-1 run at once, before any
       other agent, reading their inputs live and read-only (lots, populations, land value, occupants, funding,
       tax rates, ordinances, the other simulators' grids; the host table has no write callback), so each of our
       states reads what the original's read. Before a delivery in state 15 or 17 the original's transient grid is
       copied into ours (effects we cannot see). After our steps 5-13 our work grid is compared with the
       original's (`+0x44`).
     - **Occupant messages** (`0x99EF1142` / `0x99EF1143`) reach ours through the shadow's own subscription, when
       the original gets them.
     - **Cycle end** (the original's state back to 0: the end of state 17, or state 4 in a city without lots): its
       record is captured and compared tract by tract with ours (base, aura, park and landmark maps, transient;
       mayor rating, best / worst, hotspots), and ours restarts from that record. The tax-change baseline (§6.5.2,
       never saved) is kept across restarts. The cycle-completed message (`0x2A09B21A`) is not used: it is posted
       and arrives late.
     - **Not compared** (said in the start line): the riot roll (the original's generator is seeded from the wall
       clock, §9), and the posts, history, riots and strikes (writes).

     Lines (always written; longer ones continue on `aura shadow (cont.)` lines):

     ```
     aura shadow: ON (aura_shadow from …): …                               (game start)
     aura shadow: following the original aura simulator … (agent …, vtable …): state S at +0x70; work grid
       diagnostic on (+0x44)                                                (each city)
     aura shadow: the original aura simulator … is NOT followed: why; nothing compared
     aura shadow: comparing with the original aura simulator … (city …, X x Z cells); …
     aura shadow: city load: ours starts from the original's record (Read, then Init: state 1): base B of T
       tracts with data, transient K nonzero, D departments, H hotspots
     aura shadow yyyy-mm-dd cycle N (followed states 1-17 over D days: M moves, C calls, K occupant messages):
       tracts=T; base mismatches=a max|d|=…; aura mismatches=b max|d|=… (m where the transient differs); park map
       mismatches=…; landmark map mismatches=…; transient mismatches=… (copied before states 15 and 17: n times);
       work after states 5-13: w5,…,w13; mayor rating orig/ours=r/r; max orig/ours=v@(x,z)/v@(x,z); min
       orig/ours=…; hotspots orig/ours=h/h (same|differ); tax baseline: …[; ended in state 4 (no lots: …)]
       [; first differing (x,z) orig/ours: base (x,z) a/b …; aura …; work after state s …][; desync: …]; riot
       roll not compared (wall-clock RNG); inputs: all read | inputs not available (neutral): …
     aura shadow yyyy-mm-dd cycle N (…): not compared (joined the cycle in state S); ours restarts from this record
     aura shadow yyyy-mm-dd cycle N: INCOMPLETE (why); ours restarts from the next record
     ```

     Success is every cycle line with all mismatch counts 0, `work after states 5-13` all 0 (`-` for a step the
     cycle did not run), equal mayor rating and best / worst, `hotspots … (same)`, and no `desync` part. A cycle
     the shadow joined mid-way (the first after the shadow came up during a cycle) is not compared.
   - **Flora shadow** (`flora_shadow = 1`, env `FC4_FLORA_SHADOW`; off by default): a same-process differential of
     the flora simulator.
     - **Needs:** our simulator and the **original** flora simulator (do not enable `flora`). Otherwise it logs
       `flora shadow: requested … but INACTIVE` at game start.
     - **Registry:** at the first `SimNewMonth` of each city, FreeCity4 builds a registry from the same flora
       exemplars, in the same order. It is compared with the original's type array, `IsFloraTypeValid(0)`, and every
       record read from the block `GetStaticFloraData` returns. The record fields compared are next and seed type,
       water, the 14 floats, cluster radius and height, the 256 table bytes, the unnamed value, flag bits 0-2,
       the families, and the plop cost.
     - **Census:** our simulator wraps every `SimNewMonth` delivery to the original's agent
       (`cSC4FloraSimulator`).
       - Just before it: the original's own `Write` goes into a memory stream. Every flora occupant of the city is
         read: type, state, birth date, last seeding date, position, box, flag 1, and the moisture type and terrain
         altitude at its position.
       - Just after it: the same again.
       - The core then finds the blocks the pass visited from the record's iterator. It applies the deterministic
         part of §6.7.2 (rebirth, stage, removal, last seeding date) to each occupant of those blocks, with the
         original's own records. It checks that no other occupant changed, and that only `today` and the iterator
         moved in the record.
     - **Read only:** nothing is written to the game, and both halves run under the structured-exception guard.
     - **Not compared** (said in the start line):
       - the random part (§8): where seeds land and whether they are accepted, cluster merges, wild seeds, and the
         generator state. Occupants that vanish while predicted present are counted as `vanished`, new ones as
         `new`;
       - the family map, and the type sets of slots 8, 10, 13 and 14;
       - flag bits 3-7, and the key-0 record's flags and plop cost;
       - a pass that wraps the iterator twice.

     Lines (always written; longer ones continue on `flora shadow (cont.)` lines):

     ```
     flora shadow: ON (flora_shadow from …): …                                  (game start)
     flora shadow: comparing with the original flora simulator … (city …); elevation scale …   (each city)
     flora shadow registry: exemplars E, types ours T original T; records compared R; key 0 …; differences D[; …]
     flora shadow month yyyy-mm-dd: blocks B of N (k K, enabled 1, propagation 1); census before N after M;
       visited V; matched A mismatched X (kind n, …); predicted reborn … aged … removed … seeded …; not compared
       (random): vanished … new …; burnt list …[; first differences]
     flora shadow month yyyy-mm-dd: INCOMPLETE (why); census before N after M
     flora shadow census yyyy-mm-dd: before N (young …, maturing …, mature …, burnt …, dying …, other …; types T)
       after M (…); type/stage buckets changed K (0xTYPE stage a->b, …)
     ```

     Success means all of these:
     - `differences 0` on the registry line;
     - `mismatched 0` on every month line;
     - no `INCOMPLETE` line.

     On the census line, the changed type/stage buckets are what the month did: aging, removals and seeding. They
     are not a mismatch by themselves.
   - **Landfill shadow** (`landfill_shadow = 1`, env `FC4_LANDFILL_SHADOW`; off by default;
     `landfill_shadow_diffs = N`, env `FC4_LANDFILL_SHADOW_DIFFS`, default 8, 1..64): a same-process
     differential of the landfill developer. It needs **our simulator and the original landfill developer** (do
     not enable `landfill`); otherwise it logs `landfill shadow: requested … but INACTIVE` at game start. It wraps
     our simulator's `SimNewMonth` delivery to the original's agent (`cSC4LandfillDeveloper`): just before, the
     original's own `Write` (`cIGZSerializable`) goes into a memory stream and its maintenance is read (slot 7,
     not saved); FreeCity4's month (the core code our developer runs) then runs on that record, reading every
     input live and read-only (geometry, tuning, terrain, pollution's garbage supply, capacities and
     active-landfill flags, the department's funding and line item, zones, occupants, traffic; the host table
     has no write callback, so every write is recorded and none is sent). Just after the original's month the
     record and slot 7 are read again and compared: the zones and their heaps, the free capacity pollution
     holds, the garbage moved out of pollution's supply, the maintenance and the active-landfill flags. **Not
     compared** (said in the start line, not readable read-only): the zots, trucks and seagulls the original
     drew, the tile occupants it changed, the line item expenses it booked and the history records it sent.
     Lines (always written; longer ones continue on `landfill shadow (cont.)` lines):

     ```
     landfill shadow: ON (landfill_shadow from …): …                               (game start)
     landfill shadow: comparing with the original landfill developer … (city …); differences listed per month: N
     landfill shadow month to yyyy-mm-dd: clean|mismatches=N; zones orig/ours a/b; heaps compared=… mismatched=…
       max|d contained|=…; free capacity orig/ours a/b; garbage moved orig/ours a/b (supply x -> y); maintenance
       orig/ours a/b (before …, recomputed …); active cells compared=… mismatched=…; ours: put in landfill … in …
       calls, tiles drawn … removed …, zots placed … removed … (not placed: shadow), history values …; first
       differences: none | …
     landfill shadow month to yyyy-mm-dd: INCOMPLETE (why); not compared
     ```

     Success is `clean` on every month line and no `INCOMPLETE` line.
   - **Residential shadow** (`residential_shadow = 1`, env `FC4_RESIDENTIAL_SHADOW`; off by default;
     `residential_shadow_cells = N`, env `FC4_RESIDENTIAL_SHADOW_CELLS`, default 4, 1..64, differing cells listed
     per grid): a same-process differential of the residential simulator. It needs **our simulator and the
     original residential simulator** (do not enable `residential`); otherwise it logs `residential shadow:
     requested … but INACTIVE` at game start. It wraps each `SimNewDay` delivery to the original's agent
     (`cSC4ResidentialSimulator`): the original runs one state of its monthly cycle per day, and a time-sliced
     state can stop part-way, so only a per-delivery wrap sees the inputs that state read and where it stopped.
     Just before, the original's own `Write` goes into a memory stream; FreeCity4's state runs on that record and
     copies of the 25 grids it refers to, reading every input live and read-only (states 1, 2 and 4 read nothing
     the delivery changes and run just after, with the original's progress). Just after, the record is read
     again with the original's population, global EQ / HQ and LE (slots 16, 32-34), and compared: the 25 grids
     cell by cell, the saved fields, the occupant properties and local funding, and the four scalars. A day in
     state 10 (waiting for `SimNewMonth`) is not compared. What is not readable read-only is listed once at game
     start (`residential shadow: not compared (not readable read-only): …`). Lines (always written; longer ones
     continue on `residential shadow (cont.)` lines):

     ```
     residential shadow: ON (residential_shadow from …): …                              (game start)
     residential shadow: comparing with the original residential simulator … (city …); differing cells listed
       per grid: N
     residential shadow yyyy-mm-dd state N (name): …; mismatches=N                    (one per delivery)
     residential shadow yyyy-mm-dd …: INCOMPLETE (why); not compared
     residential shadow cycle to yyyy-mm-dd: deliveries=… compared=… clean=… incomplete=…; cells=…
       mismatches=…; grids with differences: none | …; …                    (after the state 9 -> 10 delivery)
     ```

     Success is `mismatches=0` on every delivery line, `mismatches=0` and `incomplete=0` on every cycle line,
     and no `INCOMPLETE` line.
   - **Power shadow** (`power_shadow = 1`, env `FC4_POWER_SHADOW`; off by default; `power_shadow_cells = N`, env
     `FC4_POWER_SHADOW_CELLS`, default 8, 1..64, differing cells listed per line; `power_shadow_lines = N`, env
     `FC4_POWER_SHADOW_LINES`, default 32, -1 = all, cycle lines with a difference per month): a same-process
     differential of the power simulator. It needs **FreeCity4's simulator and the original power simulator**
     (do not enable `power`); otherwise `power shadow: requested … but INACTIVE` at game start. Our simulator
     wraps (`plugin/FC4PowerShadow.cpp`) every `SimNewMonth` and each `SimNewDay` / `SimIdle` that runs one step
     of the original's `DoIdle` (power spec §6: 4 → 0 → 1 → 2 → 3); a delivery while the original is idle (state
     4, no recompute requested, none of its §5 messages since) passes straight through. Before: the original's
     own `Write` into a memory stream (the §9.1 record) and its plant counts (slots 27-29); FreeCity4's step (the
     core code our power simulator runs, `fc4_power_shadow_begin`) runs on that record and a copy of the live
     powered grid, reading every other input live through a read-only host table (no grid writes; occupant
     properties, deal usage, connection flags and budget item costs go to overlays inside the shadow). Then the
     original's delivery, unchanged; after: the record and the counts again, compared
     (`fc4_power_shadow_finish`): every record field, the consumption map, the mark / wire / powered bitmaps, the
     powered grid cell by cell, the plant counts and the property / deal / connection / item writes. The
     original's unsaved values are known from the **city start**: at PostCityInit our simulator makes the shadow
     (`PowerShadowCityInit`) with the ordinance factor read then, pass import / export / demand and
     brokenFraction 0 (power spec §8, §9.1, §7.6: what the original's `Init` leaves after a load), and the
     removals, zoning changes and tuning reloads the original receives are forwarded to it
     (`fc4_power_shadow_message`), so each wire rebuild uses the original's dirty rectangle (§5.3, PW5) and the
     factor follows the original's (PW7). A shadow switched on **mid-city** (a settings change) does not know
     them: states 1 and 2 are `not_exact` until the first month, state 3's totals until the first state 0, the
     month's broken count always, and a wire rebuild before its first step runs again with the changed bits'
     bounding box. A shortfall network whose result depends on the original's `rand()` (§6.3 step 6) runs again
     with the original's powered bits as the block iterator's target, which recovers the original's draw when
     the blocks are the same, else the cells are counted as `not_exact`. Nothing is written to the game. **Not
     compared** (said in the start line): posted messages, history values, budget line items, demolitions, lot
     conditions, the poles the month's `random_shuffle` picks (the broken count is compared), deal flags. Lines
     (always written; longer ones continue on `power shadow (cont.)` lines):

     ```
     power shadow: ON (power_shadow from …): …                                  (game start)
     power shadow: comparing with the original power simulator … (city …) from the city start|from mid-city
       (…); …                                                                     (each city)
     power shadow yyyy-mm-dd cycle: steps 4>0 0 1 2 3 on WxH cells; grid N cells compared, max|d|=D; first
       differing cells: grid(x,z) o/b [step] …; production=o/b consumption=o/b import=o/b export=o/b demand=o/b
       requirement=o/b consumers=o/b powered=o/b unpowered=o/b plants=o/b aged=o/b distressed=o/b
       overworked=o/b deficit=o/b poles=o/b broken=o/b; orders: floods=… order-dependent=… matched=…
       unmatched=…; props a/b deals a/b connections a/b items a/b (compared/mismatched); not compared: posts=…
       history=… budget=… lots=… demolitions=… other=…; [step] details …; not_exact=K; mismatches=M
                                     (orig/ours; one per recompute cycle, at its state 3; `!` = step differs)
     power shadow yyyy-mm-dd month: N cells compared; <the same scalars>[; fields orig/ours: …]; not_exact=K;
       mismatches=M
     power shadow yyyy-mm-dd bookkeeping: the original's running counts (orig) vs FreeCity4's insertion (§5.1)
       of the N live occupants and a wire rebuild of the whole map (ours), orig/ours: consumers=…;
       requirement=…; plantCount=…; poleCount=…; costBase=…; consumption map d/N cells differ max|d|=…; wire
       … cells differ; baseline not set yet (the first line … | N differences changed since the last line …):
       N differences [..] | baseline (the first line equal to the one before it …): N differences [..] |
       inherited (as at the baseline line …) N [..]; in flight (changed since the last line) N [..]; gone since
       the last line N [..]; baseline entries absent (0 at this line …) N [..]; unchanged while others changed
       (… counted at the next line) N [..]; persistent (…) N [..]; wire bits, not counted (…) N [..];
       mismatches=M
     power shadow month to yyyy-mm-dd: deliveries=… steps=… idle passed through=… not enabled=… cycles=…
       (clean …, lines …) incomplete=… reruns=… uncached=… factor=…[ (not known)] pass totals known|unknown
       dirty rectangle known|unknown; not_exact=K; mismatches=M
     power shadow yyyy-mm-dd step|month: INCOMPLETE (why); not compared
     ```

     The first four clean cycles of each month are logged, later ones only counted (`power_shadow_lines` caps
     the cycle lines with a difference a month). Success is `mismatches=0` on every cycle, month and bookkeeping
     line, `not_exact=0` from a city start (mid-city: only until the first month) and no `INCOMPLETE` line. The
     bookkeeping line (power spec §5.2, PW8) compares running counts the original inherits from the save and
     never rebuilds. The occupant messages are posted, so the original's counts lag the live occupants until the
     next frame's message-server tick: at a month line, the buildings of that day (under the turbo clock, of every
     day since the frame began) can be in flight. The baseline is the first line equal to the one before it
     (nothing changed between them, so nothing was in flight; the first line itself can have a building in
     flight, runs pt-base-turbo-1 / -normal-1); until then nothing is counted. It is never edited afterwards: a
     baseline entry that reads 0 at a line (masked by a difference in flight that cancels it; run gsT1
     2027-12-01, consumers 660/660) is listed as absent and is inherited again when it comes back. A difference
     equal to the baseline's is inherited, one that changed since the last line is in flight, and one that is
     unchanged since the last line without being inherited is a mismatch when nothing else changed at that line,
     or when it is also unchanged since the line before (two buildings in flight at two lines can give the same
     counts, never the same cells); otherwise it is listed as unchanged while others changed and judged at the
     next line. The wire bits are listed apart and not counted, nor used for the baseline: ours are one rebuild
     of the whole map, the original's what its dirty-rectangle rebuilds left, and power spec §5.3 makes those
     depend on the rectangles (PW5: a bit whose only source is a zoned cell outside the rectangle is lost; runs
     pt-fix1-turbo-1 / -2 kept such bits 0/1 for eight or nine months). Each rebuild is compared exactly on the
     cycle lines. Limit: a frame that opens two month days keeps the first one's messages in flight at the
     second, which then looks settled.
   - **Plumbing shadow** (`plumbing_shadow = 1`, env `FC4_PLUMBING_SHADOW`; off by default;
     `plumbing_shadow_cells = N`, env `FC4_PLUMBING_SHADOW_CELLS`, default 8, 1..64, the differing cells listed per
     line; `plumbing_shadow_passes = N`, env `FC4_PLUMBING_SHADOW_PASSES`, default 4, the passes replayed per game
     day, 0 = all): a same-process differential of the water pass. It needs **our simulator and the original
     plumbing simulator** (do not enable `plumbing`); otherwise it logs `plumbing shadow: requested … but
     INACTIVE` at game start. The original runs one pass state per `SimIdle` / `SimNewDay` (plumbing spec §6);
     our simulator wraps each `SimIdle`, `SimNewDay` and `SimNewMonth` delivery to its agent. At the start of a
     pass (its passCounter, record field 7, moved: state 0 ran, plumbing spec §6.1 PS6; read after every
     delivery through the original's own `Write` into a header-only stream; a pass already running when the
     shadow started is followed from the original's swap, then `exact=0`) the original's own `Write`
     (`cIGZSerializable`) goes into a memory stream and FreeCity4's core (`fc4_plumbing_shadow`) replays the
     pass on private copies of the record's grids, reading
     every other input live through a read-only host table (no write callback; its grid table only reads). It
     runs state by state: each of our states 2 to 5 just before the delivery in which the original runs the same
     state, on the original's flag map of that moment (from its record), so a building inserted, removed or
     changed between the steps is read as the original reads it (§5.3 PS8). A pass has no time limit: the
     original's states 6 and 7 are time-sliced and a loaded game delivers few `SimIdle`, so a pass may take days
     (`days=` in its line); it is followed until its swap or its restart. Only a flag-map change during the
     original's state 6 (after ours, which is not sliced) makes it `exact=0 (flag map changed during state 6)`. At the
     original's swap the published grid is compared cell by cell, with each producer's production, efficiency,
     pollution and water used, the polluted count and average pollution, the deal usage and the water
     connections; after its state 8, the produced, consumed, imported and exported water, the watered and
     unwatered cells, cost, demand (the original's running sum, loaded and moved by the occupant messages during
     the pass, never recomputed: ours is the record's at the comparison, PS4 / PS8), pipes, distressed and burst
     pipes, sources, each producer's budget item and (on an ageing pass) its condition; when the shadow cannot
     know whether a state 8 aged (made mid-city around a month), the producers' conditions show it (PS7,
     `ageing=yes|no (inferred: …)`). At each `SimNewMonth` the pipe funding (§7.1) of the simulator's own
     department (the one its record names, PS5) is checked: the distressed count exactly when the shadow was
     made at the city start (`PostCityInit`: the original's pipe condition is 0 after every load, PS3), else
     marked not exact (the shadow started mid-city); the new bursts (the original's random draw) by range,
     marked not exact. **Not compared** (said in the start line, not readable read-only): the pipes' watered
     states (a non-virtual call), redisplay, posts, history, line-item expenses, lot conditions, deal flags, the
     original's pipe condition (not saved; known only at the city start) and its pass state. Our
     `pipeCondition` is a distress fraction that moves each month by the funding curve (PS2; a step above 1 sets
     1); it is not saved either: the shadow tracks it from the city start, starting at 0. Lines
     (always written; longer ones continue on `plumbing shadow (cont.)` lines):

     ```
     plumbing shadow: ON (plumbing_shadow from …): …                              (game start)
     plumbing shadow: comparing with the original plumbing simulator … (city …); …; made at the city start (…)
       | made mid-city (…)                                                       (each city)
     plumbing shadow yyyy-mm-dd pass N: aligned=start|swap exact=1|0 [(why not)]; grid XxZ cells=C differing=D
       max|diff|=M [first (x,z) orig/ours: (x,z) a/b, …]; producers P; production orig/ours=[…]; efficiency …;
       polluted …; used …; numPolluted orig/ours=a/b; averagePollution …; deals compared=… differing=…;
       connections compared=… active differing=… visited compared=… differing=…; produced orig/ours=a/b;
       consumed …; imported …; exported …; wateredCells …; unwateredCells …; cost …; demand …; pipes …;
       distressed …; burst …; sources …; maintenance compared=… differing=…; ageing=yes|no|unknown
       [condition orig/ours=[…]]; deliveries=K days=D; mismatches=N [(not exact: n differences not counted)];
       passes compared=… clean=…
     plumbing shadow yyyy-mm-dd month: pipes=P distressedBefore=… burstBefore=…; distressed orig/ours=a/b;
       burst orig/ours=+0/+0 | burst +n (not exact: …; ours allows 0..m) | distressed orig/ours=a/b burst +n
       (ours allows 0..m) (not exact: the original's pipe condition is not known, …); pipeCondition ours=… (orig
       not readable; its distressed count implies …)[; funding from department … by ID (why)]; funding=F;
       mismatches=N; months=M clean=C; passes since the last month line: seen=… compared=… clean=… notExact=…
       incomplete=… differing=…
     plumbing shadow yyyy-mm-dd[ pass N]: INCOMPLETE (why); not compared
     ```

     A `!` marks a differing scalar. Success is `mismatches=0` on every `aligned=start exact=1` pass line and
     on every month line (`clean` equal to `months`), with `incomplete=0`. Passes followed from the swap
     (`exact=0`: the pass already running when a shadow switched on mid-city started) or changed while they ran
     (`exact=0 (…)`) only report their differences. With the shadow on when the city loads, every pass is seen
     from its start.
   - **Land value shadow** (`landvalue_shadow = 1`, env `FC4_LANDVALUE_SHADOW`; off by default;
     `landvalue_shadow_cells = N`, env `FC4_LANDVALUE_SHADOW_CELLS`, default 4, 1..64): a same-process
     differential of the land value grids. It needs the **original** land value simulator (do not enable
     `landvalue`; otherwise `landvalue shadow: requested … but INACTIVE` at game start). Its own message target
     (from `PostAppInit`) hears the original's `LandValueMapChanged` (`0x87CC75B3`, land value spec §9: posted
     at the end of state 8, so the original has just finished a pass; a month boundary is not a pass boundary,
     and the grids are written in place while a pass runs). Then the original's three grids (slots 7-9, every
     tract), `GetLandValueType` (slot 5) at each total tract's first cell, the average (slot 10), the mode
     altitude (slot 11) and its record (its own `Write` into memory: the dirty-or-running flag and the mean
     altitude, which have no getter) are read, and FreeCity4's pass (states 0-8, the core code our simulator
     runs, `fc4_landvalue_shadow_compare`) runs on local grids (the intrinsic grid starts as a copy of the
     original's: state 4 keeps it without an altitude curve), reading the terrain, the zones, the lot developer's
     area slope and the growth developers' desirability maps live and read-only; several developers apply in
     ascending pointer order, as in the original (land value spec §7.4, LV1). `SimNewDay` snapshots the listed
     developers' desirability maps while their `GrowthAndDeclineListsAreValid` (slot 10) is true (LV2): when the
     total grid differs and a map changed after the original's pass read it, our pass runs again with the map
     from before the developer's last state-17 copy and the better result is kept (`desirability=previous map
     …`). **Not compared** (said at game start): the scratch grid, the histogram, min / max altitude, pivot and
     ranges (no getter, not saved), the pipeline state and the dirty flag apart (the record holds their OR,
     logged as `settled`), and the inputs as each state read them. Lines (always written; longer ones continue on `landvalue shadow (cont.)` lines):

     ```
     landvalue shadow: ON (landvalue_shadow from …): …                                    (game start)
     landvalue shadow: NOT readable read-only (left out of the comparison): …            (game start)
     landvalue shadow: comparing with the original land value simulator … (city …); tuning …   (each city)
     landvalue shadow yyyy-mm-dd pass N: compared on CXxCZ tracts (S=s), slope SXxSZ (S=8): intrinsic cells=…
       mismatched=a max|d|=…; total cells=… mismatched=b max|d|=…; slope cells=… mismatched=c max|d|=…; types
       mismatched=t/T; average orig/ours=x/y; modeAlt orig/ours=…/…; meanAlt orig/ours=…/…; settled=0|1;
       desirability=live (k developer(s)) | previous map (…) | none (…); mismatches=M; passes=P clean=C[; first
       differing (x,z) orig/ours: total (x,z) a/b …]
     landvalue shadow yyyy-mm-dd pass N: INCOMPLETE (why); not compared
     ```

     Success is `mismatches=0` on every pass line (`clean` equal to `passes`). A `!` marks a differing scalar;
     the slope and the altitudes are compared bitwise. `settled=0` means the original's record says an input
     changed after its pass started, so a difference on that line may be that change.
   - **Flammability shadow** (`flammability_shadow = 1`, env `FC4_FLAMMABILITY_SHADOW`; off by default;
     `flammability_shadow_cells = N`, env `FC4_FLAMMABILITY_SHADOW_CELLS`, default 4, 1..64): a same-process
     differential of the flammability grids. It needs the **original** flammability simulator (do not enable
     `flammability`; otherwise `flammability shadow: requested … but INACTIVE`). Its own message target hears the
     original's six messages (occupant inserted / removed / changed, season change, ordinance updated, condition
     changed, flammability spec §5), which go to a mirror simulator (ours, on its own grids; the occupants, lots,
     ordinances, weather, plumbing and zones are read live and read-only), and the original's `SimCycleComplete`
     (`0x08F415FE`, §9: posted by the end of a cycle, delivered queued after the day agent; every tract was
     visited once in that cycle, and the average and the hotspots exist only per cycle). There the original's
     record (its own `Write` into memory: the adjusted and raw grids through their references, the multiplier,
     the average, the hotspots) and getters (slot 6 per city cell, slots 7, 8) are read and
     `fc4_flammability_shadow_cycle` compares: the raw grid exactly; each adjusted tract against the values the
     cycle could have written (its daily value from the raw values, multipliers and watered states it had, or
     its previous value: at most 2 × tractsPerDay tracts may be "unvisited", visited on the wrap day before the
     previous comparison, or the day count's number of them when larger, as under the turbo clock); the average
     against the range those values allow; the multiplier bitwise; the hotspots against every rotation of the
     fixed visiting order; the getters on the mirror after it took the original's state. The watered state is
     the one at the visit (flammability spec §6.5, FL5-FL7): plumbing slot 11 is read for every tract and
     compared **by content** with the last poll (the published grid's pointer is only counted: an aborted pass
     or a quick restart brings it back unseen, FL7, FL9). The polls: right after every `SimNewDay` our
     simulator delivers to the original (`FlammabilityShadowDeliver`, in `cFC4Simulator`'s delivery: the
     original reads the state only there), at every event (the target also hears the plumbing pass end
     `0x6946DC34`) until the first such day, and at the cycle end. A tract whose state changed in the cycle may
     hold its daily value under either state (no other tract is loosened). The changes seen after the last wrap
     day are **carried** into the next cycle (FL8: the new cycle's first tracts were visited before them and
     keep those values for a whole cycle); the wrap days come from counting days since the original's `Init`
     (day 1 wraps at once, §6.5), checked against the cycle ends delivered; when the count is off or there are
     no day polls, only the comparison's own poll is carried. The first cycle seen (and the one after a new
     simulator, a new geometry or an `INCOMPLETE`) is the baseline. **Not compared** (said at game start and in
     every line): the fire roll (a clock-seeded generator), the iterator start and the hotspot shuffle (C `rand()`
     draws), the random lot, the candidate list and the zoned sum and count. Lines (always written; longer ones
     continue on `flammability shadow (cont.)` lines):

     ```
     flammability shadow: ON (flammability_shadow from …): …                            (game start)
     flammability shadow: NOT readable read-only (left out of the comparison): …        (game start)
     flammability shadow: following the original flammability simulator of city …; …    (each city)
     flammability shadow yyyy-mm-dd cycle N: baseline (why): our simulator takes the original's grids (…)
     flammability shadow yyyy-mm-dd cycle N: tracts=CXxCZ (S=1) tractsPerDay=d; raw cells=… mismatched=a
       max|d|=…; adjusted cells=… mismatched=b max|d|=… (exact …, explained …, unvisited u of at most B); average
       orig/ours=x/y[..z]; multiplier orig/ours=…/…; hotspots orig/ours=h/h same (rotation k of n hot tracts) |
       differ (…)[; first differing (x,z) orig/ours: …]; events=e; watered polls=P by content (at a day D, with a
       change C; published grid pointer changes G); tracts whose watered state changed: t (k carried in); carried
       into the next cycle: c (changed after the wrap day W (day d since Init; n tracts of the new cycle visited) |
       the comparison's own poll only (no day polls | the day count is off: …)); hotspot getter
       orig/ours=v@(x,z)/…; average getter orig/ours=…/…; flammable getter cells=… mismatched=…; mismatches=M;
       cycles=K clean=C; not compared: …
     flammability shadow yyyy-mm-dd cycle N: INCOMPLETE (why); not compared, the next cycle is a new baseline
     ```

     Success is `mismatches=0` on every cycle line (`clean` equal to `cycles`). The average shows as a range
     `x/lo..hi` when inputs changed in the cycle; a miss counted as `maybe hot` or `not counted` is reported but
     not counted.
   - **Development shadow** (`development_shadow = 1`, env `FC4_DEVELOPMENT_SHADOW`; off by default;
     `plugin/FC4DevelopmentShadow.h`): a same-process differential of the building development simulator and the
     growth developers. It needs **our simulator and the original development unit** (do not enable
     `development`); otherwise it logs `development shadow: requested … but INACTIVE` at game start. Its cores run
     on read-only contexts: no post, registration, creation or write reaches the game.
     - Growth developers: the shadow wraps the `SimIdle` / `SimNewDay` deliveries to each original developer's
       agent (`cSC4GrowthDeveloper`). At the first delivery after the BDS started the monthly update
       (`IsMapUpdateFinished` went from true to false), a new FreeCity4 core starts from the original's record (its
       own `Write`, its five grids copied), Init with the developer exemplar and `StartMapUpdate`; every delivery
       then goes to the original and, unchanged, to ours. When the original finishes, ours runs to its end if it is
       behind and both are compared: D and the population tract by tract, the built and maximum capacity, the total
       capacity, the growth list in order, the decline list (in order; industrial purposes 5-8 as a multiset: their
       order comes from the original's wall-clock salt), `EstimateDesirability` at one cell per tract, and the
       construction info, threshold and caps. Its own message target hands `ZoningChange` and `StateChanged` to
       every core, so at the next baseline the previous core's month of bookkeeping (zoned counts, built and maximum
       capacity, total, phantom sum) is compared with the original's record.
     - BDS: after the original BDS's `SimNewMonth`, a read-only BDS core over the city (its catalogue loaded once
       per process from the same exemplars) is compared once per city on the building types (in order and as a
       set), every profile and the maximum density table, and every month on `IsSuitableForConstruction` for every
       catalogue type (the result and the reason).
     - What is not compared is listed once at game start (`development shadow: not compared or assumed: …`).
       Lines (always written; longer ones continue on `development shadow (cont.)` lines):

     ```
     development shadow: ON (development_shadow from …): …                                  (game start)
     development shadow growth 0x<type> update yyyy-mm-dd: tracts=XxZ D=ok population=ok capacity=ok
       maxCapacity=ok total=…/… growth=…/… decline=…/… (in order) estimate=N cells tuning=ok; month bookkeeping
       (… messages since …): zoned=ok capacity=ok maxCapacity=ok total=…/… phantom=…/…; ours: …; mismatches=N
     development shadow growth 0x<type> update yyyy-mm-dd: INCOMPLETE (why); not compared
     development shadow catalogue yyyy-mm-dd: types=…/… (same order) profiles=N (0 differ) density=N (0 differ);
       mismatches=N                                                                         (once per city)
     development shadow suitability yyyy-mm-dd: profiles=N suitable=…/… (original's reasons: …); mismatches=N
     ```

     Success is `mismatches=0` on every line and no `INCOMPLETE` line (the first update of a developer seen
     mid-update is skipped without a line).
   - **Tract shadow** (`tract_shadow = 1`, env `FC4_TRACT_SHADOW`; off by default; `plugin/FC4TractShadow.h`): a
     same-process differential of the tract developer. It needs the **original** tract developer (do not enable
     `tract`); otherwise it logs `tract shadow: requested … but INACTIVE` at game start. At PostCityInit and each
     SimNewMonth it reads the original's style state (slots 8, 9, 11, 13; never a setter) and its record (its own
     `Write` into memory), loads the record into a FreeCity4 core in shadow mode (then ReadTuning, as the
     original's Init does), and compares the core's `Write` bytes and getters with the original's. UpdatePopulation
     is compared at every call: on the identified executable, when the original's vtable slot 7 holds the
     original UpdatePopulation (`0x00706BC0`), that one entry is pointed at a forwarder in FreeCity4.dll at the
     first city load (same `this`, argument and EAX); per call the core computes the populations first (nothing
     written), the original runs, and each lot's population is compared with ours. The entry is put back at
     PreAppShutdown and at DLL detach. Tract (0, 0) is forwarded without a comparison (the original seeds it from
     the clock). Without the forwarder the population line samples up to 32 tracts per growth developer at each
     month instead (it drifts: the original set those populations at its last call). Grow and Decline are not
     compared (they change the city and draw from the generators): slots 5 (`0x0070D130`) and 6 (`0x00709200`)
     are forwarded the same way and only counted. Per growth developer (its subtype) the tally keeps the Grow
     calls, their results, the outcome codes they appended, their wall time, and the Decline calls and results;
     the `tract tally` line gives them once a month and starts over. The BDS gives growth about 3 ms a day
     (building-development.md §4.4), so the Grow time decides how many developers get a Grow call that day.
     Lines (always written):

     ```
     tract shadow: ON (tract_shadow from …): …                                              (game start)
     tract shadow: UpdatePopulation forwarder installed at … (vtable 00AB3118 slot 7): 0x00706BC0 -> …
     tract shadow (styles) yyyy-mm-dd load|month: record=31 bytes roundtrip=same current=0x… index=… styles=…
       all=… years=…/… mismatches=N first=…
     tract shadow (calls) yyyy-mm-dd month: calls=N compared=C origin=O failures=F lots=L mismatches=N first=…
     tract shadow: Grow (tally) forwarder installed at … (vtable 00AB3118 slot 5): 0x0070D130 -> …
     tract shadow: Decline (tally) forwarder installed at … (vtable 00AB3118 slot 6): 0x00709200 -> …
     tract tally (original) yyyy-mm-dd month: grows=G built=B codes=A successes=S declines=D declined=X growMs=T
       unlisted=U devs=0xSUBTYPE:gG/bB/dD/xX/tAVG,MAX/code:count,… (ascending subtype; Grow time in µs)
     tract shadow: UpdatePopulation forwarder removed (PreAppShutdown): slot 7 holds 0x00706BC0 again; …
     ```

     Success is `roundtrip=same`, `mismatches=0` and `failures=0` on every line.
   - **Traffic shadow** (`traffic_shadow = 1`, env `FC4_TRAFFIC_SHADOW`; off by default;
     `traffic_shadow_diffs = N`, env `FC4_TRAFFIC_SHADOW_DIFFS`, default 8, 1..64; `traffic_shadow_lines = N`,
     env `FC4_TRAFFIC_SHADOW_LINES`, default 8; `plugin/FC4TrafficShadow.h`): a same-process differential of the
     traffic simulator. It needs **our simulator and the original traffic simulator** (do not enable `traffic`);
     otherwise it logs `traffic shadow: requested … but INACTIVE` at game start. It wraps every delivery of our
     simulator to the original's agents (`cSC4TrafficSimulator`: the one-shot, the day and the month). The
     original's day ends on a 4 ms wall-clock deadline (traffic-simulator.md §3.3), so FreeCity4's simulator (a
     private copy, started on the original's record at its first delivery) runs **after** it, on the inputs as
     they stood before: just before the delivery the original's own `Write` goes into a memory stream; when it
     differs from the copy's record the copy reads it (a resync, counted); the trip lists and the lot and
     connection holder arrays the delivery may change are snapshotted. Just after, the record again gives the
     cycle state and phase progress the original stopped at; the copy steps one unit at a time to the same point
     (a cell-scanning phase stopped mid-scan stops at the original's iterator cell, FinishTripMap after the lot at
     the original's saved cell; in ReRouteTrips and FullTrips the copy then runs on, examining no trip, to the
     original's saved source cell: a "run-on"; that source's list end is taken when the original's record or trip
     list shows it was: "list ends") and takes the original's day step on its share, then its record (fields
     named), every trip list, the holder arrays and its grids are compared bitwise with the original's. The copy
     reads the city through the traffic glue's read callbacks (every write callback NULL: its writes, posts,
     budget calls and references stay inside it) and the original's grids through `GetTractValue`; the messages
     the original subscribes to are forwarded to it. The lot strategy byte (no getter) is read as the last byte of the lot's own record
     (§6.3). **Not compared** (said in the start line): the strategy draws (property `0x4A611773`) and the
     pothole and accident draws' choices (the original's generator is not readable), posted messages, history,
     incomes, network costs, budget items and accidents. **Known differences** are counted apart on the month
     line, not as mismatches, and cause no resync: a new trip's undefined destination and travel types (§4.1.1),
     a status-0 or status-1 trip's destination and cleared flags bit 0 (written by a search the original's deadline
     suspended, TS65; ours never suspend) and the record fields still unmodelled (labels starting `~`, named
     one by one). The pass counters (TS59), the day index and re-route context cell (TS60), the statistics
     byte (TS61) and the return routes (TS58) are modelled and compared exactly. `traffic_shadow_hypotheses`
     (env `FC4_TRAFFIC_SHADOW_HYPOTHESES`) is retired (ABI 69): its three readings were confirmed
     (traffic-simulator TS55-TS57: the ConfirmTrips merge sets status 0 only on a trip that absorbed one, row
     19's pairs are (published, building) and FullTrips' end swaps them, the ReRouteTrips merge absorbs routed
     trips too) and are the core's only behaviour; the key is still accepted, ignored and logged once at game
     start. Lines (always written; longer ones continue on `traffic shadow (cont.)` lines):

     ```
     traffic shadow: ON (traffic_shadow from …): …                                          (game start)
     traffic shadow: comparing with the original traffic simulator … (city …); …            (first delivery)
     traffic shadow yyyy-mm-dd day|month|begin (state A -> B, progress P, units U): mismatches=N; record FIELD:
       orig … ours …; trips of lot (x, z): …; holder … property 0x…: …; grid NAME: K of T tracts differ: …
     traffic shadow yyyy-mm-dd day: INCOMPLETE (why); not compared
     traffic shadow month to yyyy-mm-dd: clean|mismatches=N; deliveries=D (day d, month m,
       other o) compared=C clean=K incomplete=I; record fields compared=… mismatched=… (FIELD xN, …); trip lists compared=…
       mismatched=…; holder properties compared=… mismatched=…; grid tracts compared=… mismatched=…; known
       differences (not counted): … xN, …; between deliveries: resyncs=R; ours: units=… run-ons=… (list ends …)
       day steps=…, kept posts=… …; first differences: …
     ```

     Success is `clean` on every month line, `resyncs=0` and no `INCOMPLETE` line.
   - **Lot developer shadow** (`lotdev_shadow = 1`, env `FC4_LOTDEV_SHADOW`; off by default;
     `plugin/FC4LotDevShadow.h`): a same-process differential of the lot developer. It needs the **original** lot
     developer (do not enable `lotdev`); otherwise it logs `lotdev shadow: requested … but INACTIVE` at game start.
     At PostCityInit and each SimNewMonth it reads the original's record (its own `Write` into memory) into a
     FreeCity4 core made for the run over a read-only table on the city's services (Read, then Init, which stops
     before the agent: the table has no simulator), and compares the core's `Write` bytes and night state with the
     record; then, on up to 64 lots spread over the city, the original's slots 15-18, 22, 25-32, 39, 40, 46, 48,
     51, 56 and 57 with the core's, bit for bit (never a slot that changes the city). Not compared (logged at game
     start): the daily pass, construction, demolition, explosion and the night lights (they change the city and
     draw from the generators), levelling and the foundation add / remove slots, slots 42 / 43 (the boundary map
     is filled past the read-only Init), the §5 tests and the tuning (no getter). Lines (always written):

     ```
     lotdev shadow: ON (lotdev_shadow from …): …                                            (game start)
     lotdev shadow: NOT compared: …                                                         (game start)
     lotdev shadow yyyy-mm-dd load|month: record=N bytes roundtrip=same night=L cursor=C mode=M lit=B lots=N
       sampled=S compared=C mismatches=N rawAL=R first=…
     ```

     Slot 22 is compared by its rectangle only: its AL is the x87 status flags `PositionToCell` leaves, not a result
     (LD18, LD27); `rawAL` counts the lots where the original's byte was neither 0 nor 1 (information only). A slot
     25 difference adds a `s25r@x,z` entry with the rectangle and both sides' slot 27 range (slot 25 compares each
     vertex with the first vertex read with water, LD29).
     Success is `roundtrip=same` and `mismatches=0` on every line, and no `INCOMPLETE` line.
   - **Commercial shadow** (`commercial_shadow = 1`, env `FC4_COMMERCIAL_SHADOW`; `commercial_shadow_cells = N`
     differing cells listed per grid, default 4; off by default; `plugin/FC4CommercialShadow.h`): a same-process
     differential of the commercial simulator's proximity maps. It needs FreeCity4's simulator and the
     **original** commercial simulator (do not enable `commercial`). Before each SimIdle / SimNewDay delivery to the
     original's agent it reads the original's `dirty` byte and `state` (+0x20 / +0x24, read-only, only while the
     object's vtable is the original's from the compat table); before a rebuild step (state 0..2) the original's
     CO$$ and CO$$$ grids are copied into the shadow core's own grids; after the original's step the shadow core is
     set to the original's before-state (`fc4_commercial_set_machine`, ABI 53) and runs the same step on the live
     building map, and the machine and both grids are compared cell by cell. Idle-state steps are compared
     (machine only) and counted in the next line. Lines (always written):

     ```
     commercial shadow: ON (commercial_shadow from …): …                                     (game start)
     commercial shadow: following the original commercial simulator … (agent …, vtable 00AB0CCC): dirty=… state=…
     commercial shadow yyyy-mm-dd rebuild N CO$$|CO$$$ (state s->s' orig, s->s' ours; dirty d/d): cells=… edge1=…
       zeros=…; other grid unchanged mismatches=…; idle steps since the last line=K (machine mismatches=m);
       mismatches=N[; first differing (x,z) orig/ours: …]
     commercial shadow: city shutdown: N rebuild lines, M with mismatches=0
     ```

     Success is `mismatches=0` on every line.
   - **Civic shadow** (`civic_shadow = 1`, env `FC4_CIVIC_SHADOW`; off by default; `plugin/FC4CivicShadow.h`): a
     same-process differential of the civic building simulator. It needs FreeCity4's simulator and the
     **original** civic simulator (do not enable `civic`). A shadow core with a read-only host follows the occupant
     and lot-state messages the original receives (its own subscription). At each SimNewMonth delivery to the
     original's agent the original's record is captured through its own Write and compared with the shadow core
     (exclusions and status keys in hash order, building counts, the maintained and disease-center lists, the
     Landmarks department, slots 5 and 8); then the shadow core restarts from that record (with each multi-entry
     bucket chain pre-reversed, because `Read` reverses them, civic §7 / CB6) and runs its month with
     lot writes recorded, the original's month runs, and each maintained building's lot condition is compared
     with ours. The ape escape is not compared (the original seeds its generator from the clock); status bytes and
     tooltips written by others through slots 6 / 7 are counted as external writes. Lines (always written):

     ```
     civic shadow: ON (civic_shadow from …): …                                                (game start)
     civic shadow: following the original civic simulator … (agent …)
     civic shadow month yyyy-mm-dd: followed=yes|first exclusions=… statuses=… counted=… maintained=… centers=…
       landmarks=… excluded-slot5=n/m; external status writes=k; messages followed=…; pass: lots=… funding-reads=…
       run-down=… recovered=…; ape escape not compared (centers=…, ours escaped=…); mismatches=N[; differences: …]
     civic shadow: city shutdown: N month lines, M with mismatches=0
     ```

     Success is `mismatches=0` on every line.
   - **Parks shadow** (`parks_shadow = 1`, env `FC4_PARKS_SHADOW`; off by default; `plugin/FC4ParksShadow.h`): a
     same-process differential of the park manager. It needs FreeCity4's simulator and the **original** park
     manager (do not enable `parks`). At the first SimNewMonth delivery to the original's agent a shadow core with
     a read-only host starts from the original's record (captured through its own Write) and the shipped tuning,
     then follows the occupant messages the original receives (when the original's threshold reads as the code
     default 0.25 while the exemplar gives another value, ours starts as without the exemplar, and every line says
     `tuning=as-without-exemplar`). At each SimNewMonth the park and zoo lists are
     compared with the original's record (a difference restarts ours), the original's latch (+0x15) and threshold
     (+0x34) are read raw (only while its vtable is the original's, compat table) and compared with ours before
     and after the month, ours runs its month with lot writes recorded, the original's month runs, and every park
     lot's condition is compared with ours. The zoo escape is not compared (clock-seeded generator); both escape
     counts are given (the original's from its ZooEscape posts). Lines (always written):

     ```
     parks shadow: ON (parks_shadow from …): …                                                (game start)
     parks shadow: following the original park manager … (agent …, vtable ok): latch=… threshold=…
     parks shadow month yyyy-mm-dd: followed=yes|first|resynced parks=… zoos=… latch orig a->b ours c->d
       threshold orig/ours tuning=exemplar|as-without-exemplar (…); funding=… p=…; pass: lots=… run-down=…
       restored=…; zoo escapes orig=… ours=…
       (not compared: clock-seeded generator); messages followed=…; mismatches=N[; differences: …]
     parks shadow: city shutdown: N month lines, M with mismatches=0
     ```

     Success is `mismatches=0` on every line.
   - **Prop developer shadow** (`propdev_shadow = 1`, env `FC4_PROPDEV_SHADOW`; off by default;
     `plugin/FC4PropDevShadow.h`): a same-process differential of the prop developer. It needs FreeCity4's
     simulator and the **original** prop developer (do not enable `propdev`). At the first Idle or Day delivery to
     the original's agent a shadow core with a read-only host reads the original's record (its own Write) and
     inits; a listener follows the 13 messages of §3.1 and the animation tick for it. Every block step of the
     original is wrapped: ours steps first (its removals and creations recorded, not made; a crime keep check
     is answered without calling the crime simulator, certain only for a prop with crime incident ID 0), the
     block's props are listed, the original steps, and the scan flag and scanner (raw +0x14 / +0x18, only while
     the object's vtable lies in the identified executable and its class is `0x89C48F47`) are compared with ours;
     when every keep check was certain, the number of props the original removed from the block must equal ours,
     and every prop ours did not keep must be among them. Which kept prop the member generator removes and the
     creations are not compared (member and wealth generators). At each SimNewMonth the original's record is read
     into a second core and compared with ours (scan flag, scanner, wealthAdded, night state, requesters in order,
     the night list as a set, the time-of-day, dated, power and sign lists in order, the trigger map); a
     difference resyncs ours. Lines (always written):

     ```
     propdev shadow: ON (propdev_shadow from …): …                                              (game start)
     propdev shadow: following the original prop developer … (agent …, class 0x89C48F47, raw scanner readable)
     propdev shadow month yyyy-mm-dd: followed=yes|first|resynced steps=… blocks=… certain=… raw-compared=…
       removed orig/ours a/b (not kept k) created orig/ours c/d (not compared) messages=… ticks=…
       message-removals=…; record: scan=… scanner=(…) requesters=… night=… …; mismatches=N[; differences: …]
     propdev shadow: city shutdown: N month lines, M with mismatches=0
     ```

     Success is `mismatches=0` on every line.
   - **City detail shadow** (`citydetail_shadow = 1`, env `FC4_CITYDETAIL_SHADOW`; off by default;
     `plugin/FC4CityDetailShadow.h`): needs the **original** class (do not enable `citydetail`). The city's own
     instance is not reachable, so at each PostCityInit a fresh original object (GZCOM class `0xEAD4BA36`) and a
     fresh core (read-only host) go through the same inputs that leave the city alone: a fresh Init, a Read of the
     current setting's record then Init, a Read of the none record then Init, and a version-1 record (both must
     refuse it); Read's result, slot 5 and the written record are compared. The other levels would run a pass on
     the original, so only ours runs them with a recording host and the line gives what its pass would do. Line:
     `citydetail shadow load yyyy-mm-dd: setting=… fresh=… read-same=… read-none=… read-v1=…; dry runs: …;
     mismatches=N[; differences: …]`.
   - **Disasters shadow** (`disasters_shadow = 1`, env `FC4_DISASTERS_SHADOW`; off by default;
     `plugin/FC4DisasterShadow.h`, host `plugin/FC4DisasterHost.h`): a same-process differential of the disaster
     layer, fire and riot. It needs the **original** layer (there is no `enable = disasters` yet). At the first city
     the original's vtable entries (compat table `DisasterHook`: the layer's begin, end and location calls, the fire
     instance's entry points and animation tick, the fire manager's `IgniteOccupant`, the riot manager's start and
     the riot instance's tick and network lock, the tutorial's `IsTutorial`) are pointed at forwarders, each only if
     it holds the original function; all are put back at PreAppShutdown and DLL detach. A mirror core
     (`fc4_disasters` with the deterministic generator, `fc4_disasters_set_mirror_random`) reads the city through the
     host and records every write instead of making it; it follows every original ignition (same arguments, after
     the original), every fire step (counted by the tutorial reads inside the original's tick; the core's clock
     reaches its next step exactly then), every riot start (the original's registered leaders are the core's) and
     every riot second (real clock). Compared after each: fire count, burning targets, phase, lot conditions,
     steps; riot phase, leaders, active count, locked network occupants (`CanRemove(2)`); the layer's ongoing count
     and location list; at each quiet point (30 frames with nothing ongoing either side) the posts per type. Use the
     script's `disaster` action to start a fire or a riot. Lines (always written):

     ```
     disasters shadow: ON (disasters_shadow from …): …                                         (game start)
     disasters shadow: forwarders installed: …; NOT installed: …
     disasters shadow: city loaded: mirror core init=1 fire instance 1, 2 tuning exemplar(s); …
     disasters shadow fire Ignite|step N|ExtinguishCell …|BurnDown|…: orig steps=… fires orig=… ours=…
       burning orig=… ours=… phase o/o ongoing o/o locations o/o lots=… mismatches=N[; differences: …]
     disasters shadow riot start x,z: orig instance … ours …
     disasters shadow riot R start|update N|removed …: phase o/o leaders o/o [cells] active o/o locked o/o
       ongoing o/o locations o/o mismatches=N[; differences: …]
     disasters shadow quiet: posts orig/ours …; cost record buildings b/b total c/c fire e/e police p/p; …
       mismatches=N[; differences: …]
     disasters shadow: city shutdown: N lines, M with mismatches=0 (fire a/b, riot c/d, quiet e/f); lines whose
       every difference is a numbered question: …; differences by kind: <kind> [Qnn] count, …
     ```

     The quiet line's "cost record" is layer slot 18, which returns a **pointer to the statistics record** (§2.6,
     DS39), not a cost: the four fields the safety advisor reads (the damaged-building count at `+0x00`, the total
     cost at `+0x08`, the fire and police effectiveness at `+0x5C` / `+0x60`) are read through that pointer and
     compared with the mirror's. Every `Ignite` line carries the original-side inputs §5.4's five refusals are
     taken on (DS38): `type`, `flammability` read as an unsigned integer, the clamped cell `rect`, its average
     `coverage`, `burning` (`0x69DBEBE1`) and the pause samples; a refusal the original made and the mirror did
     not also names which of exits 1, 2, 3 and 5 those inputs admit (exit 4 needs the D9 curve: read it from the
     `coverage`, ≥ 51 blocks with the shipped curve).

     Success is `mismatches=0` on every line. A difference with a question number (`[Q37]` and so on, the lane
     reports) is a known gap of the spec, not a defect of the core.

     The shadow's writes are recorded, never made. The host's two creates are **wired for a live layer**, though
     there is no `enable = disasters` yet: `create_fire_occupant` makes the fire occupant class `0x69D25C5F`
     (GZCOM `0x29D25F1D`, interface `0x69D25C60`) and runs its own `SetTarget` (§5.11), `create_riot_leader` a
     pedestrian automaton of group `0x4107` with `SetState(8, ...)` and `SetVisible(true)` (§6.4, automata.md
     §8.1). `end_riot_leader` makes the `SetState(1, ...)` call the spec's End order names but does **not** rely
     on it (the RiotLeader state was pushed `final=1`, so the call is a no-op against a leader still in it):
     `RemoveControllers` and the release are what end it. `set_riot_effect` is still not made (its add,
     `AddRegEffect`, takes two names and §8.1 names one).
     Success is `mismatches=0` on every line. A difference with a question number (`[Q37]` and so on, the glue13
     lane report) is a known gap of the spec, not a defect of the core.
   - **Automata shadow** (`automata_shadow = 1`, env `FC4_AUTOMATA_SHADOW`; off by default;
     `plugin/FC4AutomataShadow.h`, host tables `plugin/FC4AutomataHost.h`): a same-process differential of the
     six saved components of the automata layer (40 the script system, 41 the controller manager, 43-46 the four
     automata managers) and of every object their records reference. It needs the **original** components (there
     is no `enable = automata` yet); otherwise it logs `automata shadow: requested … but INACTIVE`. At each city
     load and each SimNewMonth each component's own `Write` is captured into memory and read by a fresh shadow
     core (`fc4_automata_scripts` / `_cm` / `_manager`), which writes it back; the two byte sequences are
     compared byte for byte and the first difference is named. A reference travels as its host token, which here
     is the capture's 1-based object number (the number the original's own `Write` puts in the record). The
     original is only ever read. Movement, path following, animation and sound are out of scope (spec §0): this
     shadow compares the records, which are the surface the save file depends on. Lines (always written):

     ```
     automata shadow: ON (automata_shadow from …): …                                        (game start)
     automata shadow yyyy-mm-dd load|month: script N bytes identical; cm …; vehicle …; pedestrian …;
       aircraft …; watercraft …; mismatches=0
     automata shadow: city shutdown: N lines, M with mismatches=0; components compared …, bytes compared …,
       mismatches …
     ```

     Success is `mismatches=0` on every line (each component's record reads into the shadow core and writes back
     byte for byte). A run to 2032 on Konradshohe gives 73 lines, all clean: 438 components compared, 774,664 to
     780,487 bytes, 0 mismatches (glue19-shadow-2 to -4).

     The same lane's GZCOM classes for the six components (`plugin/cFC4Automata.h`, over `fc4_automata_*`, the
     interfaces of `plugin/FC4AutomataInterfaces.h`) are **not registered**: with them in the subsystem table the
     city's Init phase faults at `SimCity 4.exe+0x0045BF1B` after all six records have read back, so `enable =
     automata` is inert until that is fixed (the lane report, question A4). The riot and fire host tables
     (`plugin/FC4AutomataHost.h`) and the U-Drive-It pause / resume are wired either way.
   - **MySims shadow** (`mysims_shadow = 1`, env `FC4_MYSIMS_SHADOW`; off by default;
     `plugin/FC4MySimsShadow.h`): a same-process differential of the original MySim agent simulator and its
     advisors (mysims.md). Needs the **original** classes (there is no `enable = mysims`). At each city load the
     five MySims (advisor ids 1-5) and the My Street sim (30) are captured read-only (each one's own
     version-17 record via its own `Write`) and read into a shadow core (`fc4_advisors` with the MySim host, no
     Lua), which then follows the original: the advisor system's ticker (advisors.md §5.1 daily/monthly) and the
     agent's §3.2 messages are forwarded through the shadow's own message subscriptions. At load and each
     SimNewMonth the decoded saved fields (state, slot, wealth, sign, is_male, age, birth_month, retired,
     generation, home cell/position, neighbor/commute cells, jobless days, keys, bitmap, bio, portrait path,
     last_local_disaster, last_player_trip_day, EQ/HQ rings at float32 bits), the advisor part (personal name,
     month counter, current day, cursor, advice count), the agent slots (1-5, 30) and the getters
     (`GetActiveMySims` ids + order, `MySimLivesHere` per residence) are compared field by field. Not compared
     (named on the lines): `last_commute_time` (pathfinder timing), the pylon and dispatch tool (UI), the
     unnamed pair/flag (question MS1), per-advice nextDay/expiry/active (no Lua in the shadow), the trip state,
     and the active UI panel index (set only by the UI). Lines (always written):

     ```
     mysims shadow: ON (mysims_shadow from …): …                                          (game start)
     mysims shadow yyyy-mm-dd load: sims=N … mismatches=N                                 (city load)
     mysims shadow yyyy-mm-dd month: sims=N mismatches=N                                  (each month)
     mysims shadow (cont.) id=N <date>: <field>=<orig>/<shadow>; …                        (details)
     mysims shadow: city shutdown: months=N clean=M last_mismatches=K                     (city shutdown)
     ```

     Success is `mismatches=0` on every line. The core's per-MySim RNG derives from the session seed
     (`derive(S, 0x4A1DBBBF ^ index)`), so its only draw (FindNewResidence's start position, mysims.md §8) is
     deterministic in the shadow; the original's is wall-clock seeded and unreadable.
   - **Zone developer shadow** (`zonedev_shadow = 1`, env `FC4_ZONEDEV_SHADOW`; off by default;
     `plugin/FC4ZoneDevShadow.h`): a same-process differential of the lot manager. It needs the **original** zone
     developer (do not enable `zonedev`); otherwise it logs `zonedev shadow: requested … but INACTIVE`. At
     PostCityInit and each SimNewMonth a FreeCity4 core in shadow mode loads the original's own record (its
     `Write` into memory, each cell's lot by reference) over read-only host callbacks, runs Init, and is compared
     with the original lot manager: the lot count (slot 16), every cell (slot 6, originOnly 0 and 1), 8 sample
     rects of slots 7 and 10 (lots in order and the count), slot 14 on sample occupants and CanCreateLot (slot
     20) on sample cells and sizes. Parcellization, lot creation and deletion, the safe lot iterator and the
     master list order are not compared (no dry run exists; a Read rebuilds the list in first-cell order).
     Lines (always written):

     ```
     zonedev shadow: ON (zonedev_shadow from …): …                                         (game start)
     zonedev shadow (lots) yyyy-mm-dd load|month: mismatches=N count=O/U (orig/ours) map=XxZ city=XxZ init=1
       record=B bytes unread=0
     zonedev shadow (cells) …: cells=… covered=… origins=… mismatches=N; first=x,z:orig/ours,…
     zonedev shadow (lists) …: calls=32 mismatches=N lots=O/U (orig/ours) rects=…; first=…
     zonedev shadow (occupants) …: occupants=… onLots=… mismatches=N; first=…
     zonedev shadow (CanCreateLot) …: calls=… mismatches=N codes 0/1/2/3/7/8 orig=… ours=…; first=…
     ```

     Success is `mismatches=0` and `unread=0` on every line; a line that cannot compare says `INCOMPLETE (why)`.
   - **City situation, demolition and dispatch shadows** (`citysit_shadow = 1`, `demolition_shadow = 1`,
     `dispatch_shadow = 1`; env `FC4_CITYSIT_SHADOW`, `FC4_DEMOLITION_SHADOW`, `FC4_DISPATCH_SHADOW`; off by
     default; `plugin/FC4CitySitShadow.h`, `plugin/FC4DemolitionShadow.h`, `plugin/FC4DispatchShadow.h`, the
     shared record round trip in `plugin/FC4CoreShadow.h`): same-process differentials of the city situation
     manager (city-situation.md), the demolition utility (demolition.md) and the dispatch manager (dispatch.md).
     Each needs the **original** (do not enable `citysit`, `demolition` or `dispatch`); otherwise it logs
     `… requested … but INACTIVE`. At PostCityInit and each SimNewMonth, the original's record (its own
     `cIGZSerializable::Write` into memory) is read into a FreeCity4 core made for the run over a read-only host
     table, and the core's `Write` must give the same bytes back (a reference is written as the capture's own
     object number). Then the original's getters are compared with the core's: for dispatch, the record and
     slots 10-12, 18 and 21 for the four dispatch types and 8 (counts, ID arrays, records field by field and as
     object identities, the active pylon); for citysit, the record and slots 5-15, 17, 18, 27, 28, 30 and 32 (the
     success counts of every GUID the record carries, the indicator flag, the status block and the three ST0
     float times compared bit for bit) — this shadow is the only in-game check of that core; for demolition, the
     record and the core's own state against the §6 code defaults. Lines (always written):

     ```
     dispatch shadow: ON (dispatch_shadow from …): …                                  (game start)
     dispatch shadow: NOT readable read-only (so not compared): …
     dispatch shadow yyyy-mm-dd load|month: record=N bytes roundtrip=same|DIFFERENT; compared=C mismatches=M first=…
     dispatch shadow: city shutdown: L lines, C with mismatches=0; compared K values, M mismatched; differences: …
     ```

     and the same shape for `citysit shadow …` and `demolition shadow …`. Success is `mismatches=0` on every line
     and `roundtrip=same`. What is not compared is named at game start: for dispatch the automaton events
     (`0x09D92FC3`), the posted messages and the vehicle view's tracking; for citysit the indicator tracking, the
     mission itself (city-situation.md §0 leaves targets, pursuit, coverage and the player-drive camera to the
     host), the Lua table, the popup and the reward list; for demolition the `Demolish*` prices and the
     `ModifyTerrainHeight` counts (a dry run needs the demolition host's occupant-query callbacks, which this lane
     did not wire — the core is covered by its conformance vectors `dm00`-`dm12`), the effects and the §6 tuning
     (module globals, no getter).
   - **Demolition pricing differential** (`demolition_price_shadow = 1`; env
     `FC4_DEMOLITION_PRICE_SHADOW`; off by default; `plugin/FC4DemolitionPriceShadow.h`): needs the
     **original** utility (do not `enable = demolition`). It prices every `DemolishOccupant` of every occupant
     the occupant manager lists over the city on **both** sides - FreeCity4's core over the full host table and
     the original's - and compares every price (the int64 cost and the bool the call returns) at PostCityInit and
     every SimNewMonth. Success is `mismatches=0` on every line; the shutdown line totals them:
     `demolition price shadow: city shutdown: L lines, C with mismatches=0; compared K prices, M mismatched;`
     `differences: ...`. Our own whole-city `DemolishRegion` dry run is logged each month as well. The
     **original's** region dry run is not attempted: demolition.md §4.4 step 2 fetches the 3D view's filter with
     no null test in the original, and that dereference faults in a headless run (lane question DQ7).
     `ModifyTerrainHeight`'s vertex counts and the two effect methods are not priced here either.

   - **Dispatch manager** (`enable = dispatch`; `plugin/cFC4Dispatch.*`, over the `fc4_dispatch` core): the
     city's `cSC4DispatchManager` (class `0x4990BD93`, city component 3, the slot 39 getter). It answers
     `cISC4DispatchManager` (dispatch.md §8, 24 slots), `cIGZMessageTarget2` and `cIGZSerializable`; `Read` runs
     before `Init` and sizes nothing from the city. It holds the message server and subscribes to RemoveOccupant
     (`0x99EF1143`), DisasterPeriodEnded (`0xAA1B2002`) and the view-changed message, posts the four dispatch
     messages of §3, and reads the terrain altitude and the protection simulators' dispatch sources (§8.2). The
     vehicle view and the pylons are not wired (the tool's presentation). Lines: `dispatch Init: …`,
     `dispatch Read|Write -> …`, `dispatch Shutdown: …`. **Off by default**; the `dispatch_shadow` differential
     above is the in-game check while it is.
   - **Occupant manager shadow** (`occmgr_shadow = 1`, env `FC4_OCCMGR_SHADOW`; off by default;
     `occmgr_shadow_samples = N`, default 4, 1..64; `occmgr_shadow_probe = 0/1`, default 1;
     `plugin/FC4OccMgrShadow.h`): a same-process differential of the city's occupant manager
     (`docs/specs/occupant-manager.md`) against the `fc4_occmgr` core. It follows **whichever manager the city
     holds** - the original, or the `enable = occmgr` replacement below, whose wrapper and record it then checks.
     At PostCityInit a core with a read-only host starts
     from the original's record (its own `Write` into memory); each occupant's stored box and flag 1 are read from
     the occupant into the host's own state, which the core then changes instead of the occupant (nothing is
     posted, initialised or shut down; the core's references are audited AddRefs). The core follows the Insert,
     Remove and Move messages the original posts (after each Insert or Move the core's box is compared with the box
     the original stored in the occupant). At each SimNewMonth: occupants whose stored box changed without a Move
     message are moved in the core; the original's record is compared with the core's cells, in list order
     (occupants whose Insert or Remove message is still queued, by their flag 1, are set aside; a difference left
     after that restarts the core from the record, `resynced`); then, on the followed core when nothing was set
     aside and otherwise on a fresh core read from the month's record, slots 25, 27 and 28-47 each run N times on
     the original (through its
     vtable, slots 46/47 in the game's order) and on the core with the same arguments (points, cells and
     rectangles around the record's occupants or anywhere around the city, a null filter or one deterministic
     filter shared by both, record types, 0 or an unused type, maxCounts and callbacks that stop early), and the
     return values and occupants (in order) must be equal. The probe points the original's vtable entries of
     slots 1, 2, 12, 20-22, 24, 25, 27, 28, 30, 32, 34, 36, 38, 40, 42 and 44 at counting forwarders (only while each
     holds the 1.1.641 address; removed at PreCityShutdown) and counts calls per thread, the shadow's own excluded
     (`overlaps` = entries while another thread was inside one of them; questions OM1, OM3). Lines (always written):

     ```
     occmgr shadow: ON (occmgr_shadow from …): …                                            (game start)
     occmgr shadow: following the original occupant manager …: record B bytes, N occupants, R references; …
     occmgr shadow: probe installed on 18 of 18 slots of vtable … (main thread …)
     occmgr shadow month yyyy-mm-dd: followed=yes|first|resynced occupants=N refs=R record=exact (P set aside)|D
       cells differ (P set aside); silent moves=S; messages ins/rem/move=a/b/c refused=r off-thread=t boxes=x/y;
       queries on followed|fresh core q/Q (every slot exact)| differ: sN=e/t …; msgorder=a/b (exact)| destroy=o/n/e;
       probe: slots=18 main=… other=… (threads tid:calls …)
       overlaps=… slot12=… overflow=0; stray releases=0; mismatches=N[; differences: …]
     occmgr shadow: city shutdown: N month lines, M with mismatches=0; resyncs=K; per slot exact/total: 25=…;
       probe: … thread tid (main): s1=… …
     ```

     `msgorder` is the message-order comparison (glue25): the manager's own messages (Insert `0x99EF1142`, Remove
     `0x99EF1143`, Move `0x99EF1146`) as the followed manager posts them against the core's own posts, element by
     element on kind, occupant and data4. `destroy=o/n/e` is §5.3 step 4's interleaving over the `0x99EF1141`
     stream: `o` Destroy messages immediately after their own Remove, `n` whose own Remove is outstanding but not
     just before them (the Removes and the Destroys batched the other way round - a mismatch), and `e` of occupants
     shut down outside the manager (counted only). Success is `msgorder=… (exact)` and `destroy=…/0/…`.
   - **Occupant manager** (`enable = occmgr`; `plugin/cFC4OccMgr.*`, over the `fc4_occmgr` core): the city's
     `cSC4OccupantManager` (class `0x098F964D`, city component 4). It answers `cISC4OccupantManager`
     (occupant-manager.md §11, 50 slots in the game's compiled order - every overload pair has a unique name and
     slots 46/47 are typed-first, §9.4), `cIGZSerializable` at +4 and `cIGZMessageTarget2` at +8. The grid of
     192 x 192 64 m cells, the newest-first cell lists, the Insert / Remove / RemoveOccupants / Move message
     order, the queries and the v1 save record all live in the core; the class adapts the interfaces and the
     `fc4_occmgr_host` table (occupant AddRef / Release / Init / Shutdown / box / flags / `PostOccupantMessage`,
     and `resolve_ref` for the save's references). `Read` runs before `Init` and sizes nothing from the city.
     Lines: `occmgr Write|Read -> … (version 1, size … counts … rows=… holders=… nodes=…)`. **Off by default**;
     the `occmgr_shadow` differential above follows this manager when it is on.

     Success is `mismatches=0` on every line (they count cell, box and query differences and stray releases).
     `refused` counts followed messages the core refused (an Insert of an occupant already read from the record, a
     Remove of one not in it): possible right after the city load or a `resynced` month, otherwise a sign of a
     missed message. In the lane's turbo runs to 2032 on Konradshohe (alone and with the 27 replacements) every
     line had `mismatches=0`, `resyncs=0`, `refused=0` and every probed call on the main thread. OM1 was answered
     statically (re-answers18): the background model loader's thread calls slot 40, one standard cell at a time,
     when its filter flag is set; slot 40 is probed for that reason.
   - **Simulation grids** (`enable = simgrids`; `plugin/FC4SimGridLogic.h`, `plugin/cFC4SimGrid.*`; glue-simgrids4):
     factories for the grid classes `0x49B9E601`..`0x49B9E60B` (docs/specs/simgrid.md §2; `0x49B9E601` makes a
     uint8 grid). simgrid.md §6.4.2 allows registering them only for layout-compatible grids: these keep the
     STLport `vector<vector<T>>` (§6.3, rows by x) with every buffer from the game's STL node allocator B, count 0
     on creation, the §4 slots and the v1 record (§5). With the switch, every grid the class store creates on load
     (and every grid our glue creates through GZCOM) is ours; the original simulators' directly built grids stay
     the game's. Lines: `simgrids: read|write clsid=… instance=… S=… count=XxZ iface_writes=N changed=-1|0|1`
     (changed: values differ from the loaded ones; with iface_writes=0 the original wrote them through
     `GetGridData`), and at the last release `simgrids: all grids released: created=… raw_changed=…`.
     **Off by default.** Out-of-range reads return 0 and such writes are ignored [choice] (question SG1).
     The read/write lines also carry `digest=` (FNV-1a 64 of the record bytes) and `nonzero=` (cells not 0).
     Tooling `simgrids_probe = 1` (`plugin/FC4SimGridProbe.*`, works with or without `enable = simgrids`): every
     grid a save hands to our glue (`IStreamAdapter::ReadRef`) is held until PostCityInit, then its own `Write`
     is captured: `simgrids-probe: grid clsid=… instance=… digest=… nonzero=…` per grid and a summary line naming
     the grid class (ours or the game's), so a save written by our grids can be checked after the game's reload.
   - **City situation manager** (`enable = citysit`; `plugin/cFC4CitySituation.*`, over the `fc4_citysit` core):
     the city's `cSC4CitySituationManager` (class `0x0BB14381`, city component 52, the slot 49 getter). It answers
     `cISC4CitySituationManager` (city-situation.md §8, 40 slots), `cIGZMessageTarget2`, the legacy
     `cIGZMessageTarget` and `cIGZSerializable`; `Read` runs before `Init` and sizes nothing from the city. This is
     the spec's §8 **minimal replacement** [choice]: the record byte for byte and the interface answered with "no
     situation" — the mission gameplay (targets, pursuit, coverage, the player-drive camera, the popup) is
     deliberately out of scope (§0 calls a full one "several thousand lines"), and unattended play never starts a
     mission (§0: "its popup never opens by itself"). The host wires the message server and the ten subscriptions
     of §3, the preferences' reward list, the advice and automaton queries the interface needs, the aura's
     `AddTransientEffect` and the budget's `DepositFunds` (the §4.5 outcome) and the messages of §3; the mission
     pieces are not wired. Lines: `citysit Init: …`, `citysit Read|Write -> …`, `citysit Shutdown: …`. **Off by
     default**; the `citysit_shadow` differential above is the in-game check while it is.
   - **Airport and seaport developers** (`enable = ports`; `plugin/cFC4PortDeveloper.*`, interfaces
     `plugin/FC4PortInterfaces.h`, over the `fc4_port_developer` core; off by default): one shim, two classes
     (`cSC4AirportDeveloper` `0xAA2049A1`, `cSC4SeaportDeveloper` `0xEA230A28`) with the four sub-objects of
     port-developers.md §1 (QueryInterface(1) gives the port sub-object at +4). The host reaches the budget
     department and line 1, the demand simulator's occupant effects, lots, the lot developer, the zone manager
     (dezone), the traffic simulator, the terrain, the history warehouse message and the advisor system. The glue
     holds one reference per port lot the core lists and one on the department. Not wired (logged once): the
     seaport's "no sea access" prop state switch. Lines: `ports <kind> Init|Read|Shutdown: ok=… tuning=… ports=…
     dept=… sea=… subscriptions=… agent=… held=…`, `ports <kind> Write -> 1 (ports=…)`, `ports <kind> month
     yyyy-mm-dd: ports=… ran=… history=… event=… avg=… trips=… fr=… required=… writes: …` (months with ports and
     the first month).
     PD12 completion: the no-sea-access prop switch is implemented through the native occupant-manager spatial
     list, occupant-holder property presence and acquired prop `SetState(1)`. `ports PD12` lines report filter,
     list and setter counts; `unavailable` identifies safe-host precondition failures separately from empty results.
   - **Ports shadow** (`ports_shadow = 1`, env `FC4_PORTS_SHADOW`; off by default; `plugin/FC4PortsShadow.h`):
     needs FreeCity4's simulator and the **original** developers (do not enable `ports`). One shadow core per
     developer, with a read-only host, starts from the original's record at its first SimNewMonth and follows the
     messages of §3; a growth message restarts it from the record the next month (`followed=regrown`). Each month
     the record's ports are compared (a difference restarts ours, `followed=resynced`), ours runs with writes
     recorded, the original's month runs, then compared: the history value, GetAvgEfficiency (bits), trips,
     GetNumPorts, GetAllPortLots, each port lot's usage values, maintenance and condition, line 1's full expenses
     with our required budget; the seaport's sea map (CityHasSea and every cell) at the first month and after a
     terrain message. Lines:

     ```
     ports shadow: ON (ports_shadow from …): …                                                 (game start)
     ports shadow: following the original airport|seaport developer … (agent …)
     ports shadow <kind> month yyyy-mm-dd: followed=yes|first|resynced|regrown ports=… avg o/u trips o/u
       history o/u values sea=… map=…; ours: …; messages followed=…; mismatches=N[; differences: …]
     ports shadow: city shutdown: <kind> N month lines, M with mismatches=0
     ```

     Success is `mismatches=0` on every line.
   - **History warehouse and 24-hour clock** (`enable = history`, `enable = clock24`; `plugin/cFC4History.*`,
     `plugin/cFC4Clock24.*`, over the `fc4_history` and `fc4_clock24` cores). The warehouse is the city's
     `cSC4HistoryWarehouse` (class `0x89EFA536`): it hears the record and queue-length messages, adds itself as a
     simulator agent for SimNewDay and SimNewYear (the yearly thinning pass) and answers the history queries. The
     clock is `cSC424HourClock` (class `0xA7E7F929`): a framework system service (`0xA7E7F92A`, priority
     -2,000,000) on the on-idle list, the two cheats (WhatTimeIzIt, Stopwatch) and the seven clock messages
     (`0x44778230`..`36`). Under turbo the framework's idle loop is starved (about one OnIdle a second), so the
     clock steps rarely (each step is capped at 1200 game seconds). Both `Write`s also read their record back
     into a scratch core and write it again; the log says `read back and rewritten identical`. Lines:
     `history Init: …`, `history Read|Write -> …`, `history Shutdown: …`, `clock24 Init: ok=… cheats=… tuning=…`,
     `clock24 Read|Write -> …`, `clock24 Shutdown: N idle
     passes; posted hour …, tick …, visual hour …, visual tick …, day/night …, pause …, hidden pause …; N
     off-main calls`.
   - **History shadow** (`history_shadow = 1`, env `FC4_HISTORY_SHADOW`; off by default;
     `plugin/FC4HistoryShadow.h`): needs the **original** warehouse (do not enable `history`). A shadow core is
     read from the original's record at each city load and written back (compared byte for byte); it hears the
     same record and queue-length messages with the date the original reads, and after each of the original's
     SimNewDay / SimNewYear agent steps it takes its own and the results are compared (when the original's 5 ms
     timer splits a step, the shadow skips the repeats). At each SimNewMonth and at the end of each yearly pass
     both records are compared byte for byte, and the queries (slots 5-8) on a few ranges per series at 24-bit and
     53-bit x87 precision, bit for bit. Queue lengths set through slot 13 (not a message) are not seen. Lines:

     ```
     history shadow yyyy-mm-dd load: the original's record N bytes (S series, P points) read by the shadow core:
       ok, written back identical; mismatches=0
     history shadow yyyy-mm-dd month|pass end: series=… points=… record=N bytes identical; queries=Q (24/53-bit)
       differing=0; records=… lengths=… days=… years=… split_steps=… step_results_differing=0 pass=…;
       mismatches=0; comparisons=C clean=C
     ```
   - **Clock shadow** (`clock24_shadow = 1`, env `FC4_CLOCK24_SHADOW`; off by default;
     `plugin/FC4Clock24Shadow.h`): needs the **original** clock (do not enable `clock24`). The original's timer
     has no read path in the spec, so the shadow core mirrors the original's steps rather than replaying real
     time: its own idle service samples the original's game time each idle pass; a move of 61 to 1200 game
     seconds is one step, taken by the shadow core through a synthetic elapsed time at magnification 1, any other
     change is a jump (SetHour, SetMinute, SetSeconds); the mode and the pause flags are mirrored. Then the visual
     time, both percentages, IsNight, the event hours and the magnification and its limits are compared (bits),
     and the original's seven messages must equal the shadow core's posts in order. Lines:

     ```
     clock shadow yyyy-mm-dd load: the original's record 26 bytes (version 2; …) read by the shadow core and
       initialised with the tuning (…): ok, written back identical; visual …; values=17 differing=0; mismatches=0
     clock shadow yyyy-mm-dd month: time=… visual=… mode=… pause=… hidden=…; samples=… steps=… jumps=… modes=…
       pauses=…; posts original=… matched=… unmatched=0 pending=…; values=… differing=0; mismatches=0;
       comparisons=C clean=C
     ```
   - **Prop manager and lot configuration manager replacements** (`enable = propmgr`, `enable = lotcfg`;
     `plugin/cFC4PropManager.*`, `plugin/cFC4LotConfigManager.*`, over the `fc4_prop_manager` and
     `fc4_lot_config_manager` cores, docs/specs/prop-lotconfig.md Parts A and B; the glue21 lane). Both are
     **off by default** and swap in through the GZCOM registry (`disable` still wins). The prop manager is the
     city's `cSC4PropManager` (class `0xA990BD10`, city component 5): the prop catalogue (process-wide, built
     once), the catalogue queries, `CreateProp` / `CreateGenericProp`, adding and removing city, lot, street and
     building props, and `Write` / `Read`. The lot configuration manager is `cSC4LotConfigurationManager`
     (class `0x299062C0`, component 9): the configuration map and family set, the set queries, the building
     tests and `Write` / `Read`. Neither interface is subclassed from the SDK's: `plugin/FC4PropLotInterfaces.h`
     and `plugin/FC4TractInterfaces.h` declare `cFC4ISC4PropManager` / `cFC4ISC4LotConfigurationManager` with no
     overloads, in the game's verified slot order (§A9 slots 0-40, §B7 slots 0-28; the last slot of each is the
     class's scalar deleting destructor).

     Two process-lifetime rules hold for both (glue15 found both the hard way): the catalogues are created once
     and torn down only at `PreAppShutdown`, and `fc4_lot_config_unload` runs **once** there - never per city or
     per run, or the map empties and the next rebuild takes a different bucket order than the original's.
     Lines: `propmgr: process prop catalogue …`, `lotcfg: process configuration catalogue …`, `propmgr: command
     … registered with the core …`, `propmgr: SetBuildingProps …`, `lotcfg: SetConfigObjects …`.

     Known limit (GLUE21-1): `cSC4LotConfiguration` is **not** GZCOM-creatable (lot-configuration.md §1: "the
     manager allocates it directly", 0xC0 bytes, constructor `0x004797A0`) and FreeCity4 does not replace that
     class, so `enable = lotcfg` cannot build a configuration catalogue on a stock game. `create_configuration`
     probes GZCOM for `0xA99062B4` and refuses when it does not answer rather than inventing a layout; the log
     says `cSC4LotConfiguration (0xA99062B4) is not GZCOM-creatable … (GLUE21-1)`.
   - **Prop manager, lot configuration and tutorial shadows** (`propmgr_shadow = 1`, `lotcfg_shadow = 1`,
     `tutorial_shadow = 1`, env `FC4_PROPMGR_SHADOW` / `FC4_LOTCFG_SHADOW` / `FC4_TUTORIAL_SHADOW`; off by
     default; `propmgr_shadow_cells = N` sample cells a run, default 32, 1..256; `plugin/FC4PropLotShadow.h`; the
     glue15 lane). Same-process differentials of `cSC4PropManager`, `cSC4LotConfigurationManager` and
     `cSC4TutorialSystem`: at `PostCityInit`, every `SimNewMonth` and `PreSave` the class the city holds is
     written to memory, a FreeCity4 core is read from that record over a **read-only** host, and the answers are
     compared. Compared: the prop catalogue in bucket order (each type's key and 0x1C-byte profile, each
     family's members, the catalogue message's data2), `GetCityProps` / `GetLotProps` / `GetStreetProps` on
     sample cells, `CreateProp` on sample types, both records byte for byte and reference by reference, the lot
     configuration map and family set in bucket order, each configuration's object array at the city detail, the
     ID-set queries and `GetObjectIDArray` on samples, and the tutorial's `IsTutorial`, task number and GUID.
     Not compared (logged at start): `GetBuildingProps` (the `0xA8322B7F` property's layout is not in
     docs/abi), every call that changes the city, the filter caches, the footprints, `PostCityInit`'s building
     information, and the tutorial window, registry and cheat (TU6, TU7).

     Since the glue21 lane the shadows stay **on** when a replacement is installed: they then compare
     FreeCity4's own class against the private core, which is a class-layer self-check of the vtable, the QI
     table and the record round trip (over the original it is the original-vs-core differential). The shadow and
     the replacement keep separate process catalogues and neither tears the other's down. Lines
     (`mismatches=N` on each):

     ```
     propmgr shadow (catalogue|queries|CreateProp|record) YYYY-MM-DD load|month|save: ...
     lotcfg shadow (map|families|arrays|queries|record) YYYY-MM-DD load|month|save: ...
     tutorial shadow (state|record) YYYY-MM-DD load|month|save: ...
     ```

     and at city shutdown one totals line per shadow:
     `<name> shadow totals: lines=N exact=E mismatched=M incomplete=I`. Success is `mismatched=0`.
   - **Tutorial system replacement** (`enable = tutorial`; `plugin/cFC4Tutorial.*`, over the `fc4_tutorial`
     core, docs/specs/tutorial.md; the glue15 lane). The city's `cSC4TutorialSystem` (class `0xAA3836FB`, city
     component 48): the 11-byte v2 record with the GUID mapping, `IsTutorial` and the getters, `Init` /
     `Shutdown`, and `fc4::TutorialIsTutorial` (what the fire host and the flammability glue read instead of
     tutorial slot 8). **Off by default**. There is no tutorial window, registry or cheat (TU6, TU7).
   - **Network manager and network lot manager** (`enable = netmgr`, `enable = netlot`;
     `plugin/cFC4NetworkManager.*`, `plugin/cFC4NetworkLotManager.*`, over the `fc4_netmgr` / `fc4_netlot` cores):
     the city's `cSC4NetworkManager` (class `0xC990BD46`, component 53, the slot 38 getter) and
     `cSC4NetworkLotManager` (`0xE9A6CD3B`, component 11, slot 42). Each answers its `cISC4*` interface in the
     compiled slot order (`plugin/FC4LotDevInterfaces.h`), `cIGZMessageTarget2` and `cIGZSerializable`; `Read`
     runs before `Init` and sizes nothing from the city. The managed list, the flags table, the terrain rule,
     the removal queue, the crews, the retrofit, the lot choice, the road and avenue styles (§8.7), the lot
     objects, the potholes and the riot lock are the core's; the host tables are the shadow's read side
     (`plugin/FC4NetMgrHost.h`) plus the writes §13 needs (subscriptions, occupant remove / insert / refresh,
     the retrofit and crews, the lit and flag writes, the style writes, the pothole effect props).
     **Not wired** (a NULL callback the core counts at Init): the 13 tools and their five callbacks, the
     underground refit's tool callbacks, the street props and flora of §8.6, the lot-configuration object
     bridge (so `GetNetworkLotConfigurationSet` inserts nothing and a lot's object array is empty) and
     `view_rotation_changed`. The three presentation slots (13-15) are identity mappings (§13 [choice]).
     Lines: `cFC4NetworkManager(%p) created, core %p`, `netmgr Init: …`, `netmgr Read|Write -> … (N bytes,
     record53 round trip same|DIFFERENT)`, `netmgr Shutdown: …`, and the matching `netlot …` /
     `record11 round trip` lines. The round trip is on every save: the record is written, read back by a
     scratch core and written again, and the bytes must match. **Off by default**; the `netmgr_shadow`
     differential below needs the **original** managers, so it is not run together with either replacement.
   - **Network manager shadow** (`netmgr_shadow = 1`, env `FC4_NETMGR_SHADOW`; off by default;
     `netmgr_shadow_order = late|early`, env `FC4_NETMGR_SHADOW_ORDER`, default `late`;
     `plugin/FC4NetMgrShadow.h`): a same-process differential of `cSC4NetworkManager` (component 53) and
     `cSC4NetworkLotManager` (component 11) (network-manager.md). It needs the **original** managers (there is
     no replacement yet). Two shadow cores with read-only hosts (`plugin/FC4NetMgrHost.h`: every write
     callback NULL, nothing reaches the game) start at PostCityInit from the originals' own records (their
     `Write` into memory), the lot core with the 543 network lot configurations of the exemplars (type `0x21`,
     read once per process), and follow the originals' messages (§6): the manager core through a listener
     registered at PostAppInit (before every city listener), the lot core, with `late`, through a second target
     subscribed at PostCityInit, after the original network lot manager, so it reads the game as the original
     did (`early`: the first listener). The cores' writes are recorded, not made: the terrain rule's
     destructions (§7.6) are checked at the next framework tick against the occupants the original removed, and
     the road style writes (§8.7) against the occupants' `GetVariation` (slot 28) and flags `0x6000000`. At
     PostCityInit, each SimNewMonth and before a scripted `save`: our component 53 `Write` against the
     original's (byte for byte, object for object), our component 11 `Write` against the original's (entry by
     entry: occupant and configuration; pothole keys), the original's `GetZoneTypeForNetwork` /
     `GetWealthForNetwork` (slots 9, 10) against ours at every lot cell, and, for a configuration that differs,
     whether both are compatible now (§8.4): then it is a random draw (the original's generator is seeded from
     the clock, §8.5, §17), counted and not a mismatch. A core whose record differs restarts from the
     original's. Lines (always written):

     ```
     netmgr shadow: ON (netmgr_shadow from …): …                                            (game start)
     netmgr shadow: N exemplar keys enumerated, M network lot configuration exemplars (exemplar type 0x21)
     netmgr shadow (load|month|save) yyyy-mm-dd: mismatches=N start|followed; managed=a/b record53=equal (6
       bytes); lots=a/b potholes=a/b record11=equal|same entries (order differs)|DIFFERS; lot cells=… zone
       diffs=0 wealth diffs=0 configs same=… draws=… incompatible=0; configs loaded=543; messages=… (mgr …, lot
       …); terrain msgs=… tool calls=… destroyed=… confirmed=… kept-by-orig=0 orig-only=0 rebuilt=…
       candidates=… refits=…; style writes=… checked=… gone=… mismatches=0; potholes created=…; order late:
       probe before=…/…; per message: N compared, variations agree=… ours-only=… orig-only=… differ=…;
       0x…: N msgs N cells, the original changed …, ours changed …[ (first notes)][; cells before/orig/ours
       [0x… rect …: x,z:b/o/u …]]; NM10 (lot state border's first row, open)=K[; resynced: mgr lot]
       [; differences: …][; NM10 differences: …]
     netmgr shadow: city shutdown: N lines, M with mismatches=0 (exact M/N); D configuration draws not
       compared; R resyncs; NM10 (…) differences K on L lines
     netmgr shadow: lot state changed (0xE8BDA2E7) border cells with a network tile, by place in the grown
       rect (cells / the original changed the variation / ours changed it where the original kept it): …
     ```

     Success is `mismatches=0` on every line. A style difference names the cell, both variations and the
     zone / wealth both managers give the cell now, with the variation they imply. With `late`, every lot
     message's cells are also compared message by message: the early target reads their variations before
     the original handles the message, the late one after it and after the lot core, so the line counts, per
     message type, the cells the original changed and those ours changed where the original kept the value.

     **NM10 (open question).** In game the original never restyles the inner cells of the first row
     (x0 < x < x1, z = z0) of a lot state message's grown rect (§6.2 `0xE8BDA2E7`), which the spec walks with
     the rest of the border; Konradshohe to 2032: 0 of 589 such cells changed by the original, 80 changed by
     ours, while every other place of the border agrees; whole-city snapshots (`netmgr_shadow_wide = N`, env
     `FC4_NETMGR_SHADOW_WIDE`: the first N lot state messages, default 0) show the original restyling such a
     cell later, during another message, only rarely. Until NM10 is answered, a style or lot difference at a
     cell our lot core updated there since the city loaded is counted and listed under `NM10`, not as a
     mismatch.
   - **Terrain replacement** (`enable = terrain`; `plugin/cFC4Terrain.*`, `plugin/FC4TerrainInterfaces.h`, over the
     `fc4_terrain` core; the glue18 lane, **not yet clean in game**). The city's `cSTETerrain` (city component 49,
     class `0xE98F9525`) and the `cSTESurfaceWater` it is kept next to (class `0x0B8DA7DB`): both are built from
     the registry CLSID, so the originals never run. The two are stateless views over one `fc4_terrain` handle
     (the height field, the surface water's depths, the bodies of water, the lava). `cISTETerrain` is declared in
     the game's slot order, not the SDK's: the SDK's three `SetAltitude` overloads compile to slots 49/50/51 in a
     different order, and slot 91 is the deleting destructor. The object's fields are laid out to
     `docs/specs/terrain.md` §2.3 because the game reads them directly. Messages are posted queued with
     `void1 = IGZUnknown = the terrain` (foundations §5). The terrain 3D view (class `0xC9B84E10`) is created at
     `DoPostCityInit` through the class factory; its own initialisation is not in the spec and is what still stops
     a run (TR21). Lines:

     ```
     terrain: Read (class record = version + ref, §13.2) -> ok=1 version=2
     terrain: created cSTESurfaceWater (a view over the shared fc4_terrain handle)
     terrain: replacement ON (enable = terrain): 128 x 128 cells, sea=250, default brush level=0
     terrain: created the terrain 3D view (class 0xC9B84E10) at DoPostCityInit
     terrain: TR9 default brush level -> N (view update)
     terrain: TR18 refused host callback '<name>' from inside the terrain handle (guard count N)
     terrain: shutdown: posts_pending=… brushes=… reentrant refused core=… guard=… (both expected 0 outside
       god-mode brush kinds 1 and 8-10)
     ```

     TR18 (`terrain.md` §15.4) is enforced at the glue boundary as well as in the core: a host callback that comes
     back into the same handle is refused, counted and logged. Both counts stay 0 on the simulation's paths. TR9
     (§2.3): the default brush level follows the view-update message `0xA6B79602`; the initial camera copy is not
     reproduced (no camera interface), so it stays 0 until that message, which is what a harness with no view sees.
   - **Terrain shadow** (`terrain_shadow = 1`, env `FC4_TERRAIN_SHADOW`; off by default; `terrain_shadow_points = N`
     sample points a month, default 256, 16..4096; `terrain_shadow_diffs = N` differences named per line, default 8,
     1..64; `terrain_shadow_lines = N` change lines a month, default 8, 0..1000; `plugin/FC4TerrainShadow.h`): a
     same-process, read-only differential of the terrain core (`fc4_terrain`, docs/specs/terrain.md) against the
     **original** `cSTETerrain` / `cSTESurfaceWater`. The shadow never writes to the city's terrain, and needs none
     of our simulators (its own message subscription). It needs the **original** terrain, so with
     `enable = terrain` it is **inactive** and says so once (the same rule as the history shadow): the two never
     fight, and `terrain_shadow = 1` alone is unchanged. At city load a FreeCity4
     terrain is built from the original's altitudes and sea level (the water rebuilt from the sea level, §10.1, and
     compared with the original's depths, which are then copied), the original's cached minimum and maximum taken
     (§4.6), and every cell compared (CellHasWater, CellHasLand, GetWaterDepthForCell, GetWaterType, IsNavigable
     r = 0) plus a sample of GetAltitude and GetFaceNormal. On every `0x278E6003` / `0x678E6E3A` (§8.1) the
     message's rectangle is re-read (the moved vertices replayed through our SetAltitudeAtVertex and the depth rule
     classified), our RedisplayTerrain must post the same four data words, the cached heights must match with the
     terrain flag, and the queries are compared on the rectangle grown by one (cells touching a vertex changed
     elsewhere, whose own message is still queued, are left out and counted). The messages are queued posts,
     delivered after the change (under turbo, weeks of game time later; a change line's date is its delivery's). At
     each `SimNewMonth` every vertex is re-read: one that differs from ours is copied and kept pending (`ahead`); the
     change message that covers it later puts ours back first and follows it (`late`); one that no message covered
     by the next month counts as `unfollowed`. Then the cache, the sea level and a date-seeded sample of all the
     queries (IsNavigable r = 0..2, half on wet cells) are compared. The script
     action `terrain` is mirrored exactly. Floats are compared bit for bit; the navigability host (bridges, power
     poles) reads the game only (TR18; the refused count is in every line). Lines (always written):

     ```
     terrain shadow: ON (…) | requested but INACTIVE (…)                         (game start)
     terrain shadow load yyyy-mm-dd|-: cells=XxZ sea=…; depths rebuilt from the sea level e/t; cache …; allowed
       heights e/t; queries <name> e/t …; refused=R; mismatches=N[; first: …]        (- : before the first day)
     terrain shadow change yyyy-mm-dd terrain|water kind=K rect=x0,z0..x1,z1: vertices=V moved=M wet=W depth rules
       slot50/raiselower/reset/none=a/b/c/d; message e/t; [late L (…); ]changed elsewhere E[ (rect=…; cells
       touching them skipped S)]; [cache …; ]queries …; refused=R; mismatches=N
     terrain shadow script yyyy-mm-dd SetAltitude rect=… h=… kind=K: vertices e/t; cache …; refused=R; mismatches=N
     terrain shadow month yyyy-mm-dd: unfollowed=U[ (altitudes, depths only, rect, first)]; ahead=A; cache …;
       sea e/t; queries …; changes terrain=T water=W …; refused=R; mismatches=N[; first: …]
     terrain shadow: city shutdown: N lines, M with mismatches=0; totals <name> e/t …
     ```
   - **Effects** (`enable = effects`, docs/specs/effects.md; off by default; `plugin/cFC4Effects.h`,
     `plugin/FC4EffectsHost.h`): replaces `cSC4EffectsManager` (CLSID `0x49822F75`, city slot 85, city +0x1FC)
     with ours, backed by `fc4_effects`. It is not a saved component (§13). Creation by name under the detail
     setting, the level-of-detail ranges and the select groups come from the core; the animation tick
     (`0x86AD10EE`) dilates the step by the simulator's slot 32 and runs while the game is paused. The brush
     effect reaches terrain `ApplyBrush`; the game effect reaches the fire disaster manager
     (`IgniteCellRadius`, `SetCause`, `ExtinguishCell`), the toxic-spill manager and `DemolishRegion`.
     - **`effects_finish`** (env `FC4_EFFECTS_FINISH`; default 0): where effects finish (§7, EF2). `0` finishes
       **outside the draw pass** — the same §7 sweep runs after each animation tick (`fc4_effects_finish_hook`),
       so a session that never draws the city still ends its effects and posts their end messages. `1` is the
       original's compat mode: only the 3D view's draw pass finishes effects (`fc4_effects_draw_pass`), and a
       session that never draws never finishes one. No end message is ever synthesised.
     - Animations are stubbed: particles, decals, sequences, sounds, cameras and attractors are core
       placeholders, and `render_component_active` is always false, so a render component never holds an effect
       (`[choice, EF1]`). An effect therefore finishes when its brush, game and meta work is done, not after the
       shipped decals' 45-50 s.
   - **Effects shadow** (`effects_shadow = 1`, env `FC4_EFFECTS_SHADOW`; off by default;
     `plugin/FC4EffectsShadow.h`): a same-process mirror of the effects manager. A second `fc4_effects` runs
     over a host whose reads come from the game and whose **every write is recorded in the context instead of
     being made**, so the mirror never changes the city. It is ticked with the same microseconds as the live
     manager and finishes through the same sweep; the live side's create / start / stop / end-message calls are
     mirrored into it. One line at shutdown:

     ```
     effects shadow: ticks N, created N, end posts N, select draws N, brush applies N, sweeps N, host writes N, differences N
     ```

     `differences` counts the times the mirror could not make an effect the live side made. The shadow does not
     hook the original's vtables, so with `enable = effects` off it reports only its own state and recorded
     writes; the game runs use both switches, which drives the two cores call for call.
   - The autotest clock, the digest, the observe lines, the save autotest and the tick feed for the watchdog
     come from the **observer**
     (`plugin/FC4Observer.cpp`), a system service and message target registered at `PostAppInit` in every mode,
     including `FC4_DISABLE=all`.
   - **x87 control word** (always on; foundations §18.2): the game runs its main thread at 24-bit x87 precision and
     FreeCity4 never changes it. `fpu <where>: x87 CW=0x… (PC 24-bit, RC nearest, masks 0x3F) MXCSR=0x…` is logged at
     DLL attach (before and after our initialisation), at every framework hook (`PreAppInit` is the guard's
     reference: the first hook after the game's `PreFrameWorkInit`), at city load, at each observe month line,
     around our budget settlement, and at the first tick and every tick where the control word changed (with the
     tick number). The guard writes `fpu: WARNING the main thread's control word changed (…)` the first time the
     main thread differs from the reference, and `fpu: WARNING the main thread is not at 24-bit x87 precision (…)`
     the first time its precision is not 24-bit. A run without 24-bit precision is not a valid reference for
     float-sensitive values.
   - **Power recompute report** (`power_report = 1`, env `FC4_POWER_REPORT`; default on in checked builds, off in
     release builds; our power simulator only): after each recompute cycle
     `power recompute N: cells …x… widths … ; wire …x… words … bits set …; consumption cells …; plants …, floods …;
     production … consumption … demand … powered … unpowered …`, then one `plant 0x…: visited … position (…) seed
     (x, z) cells [x1,z1..x2,z2] capacity … funding … condition … efficiency … produced … connected …` line per
     plant and one `flood from (x, z) wired …: reached … cells, rows …; supply … import … available …; consumers …
     need …; powered …` line per flood. The first 40 cycles are logged, then the first after each month.
   - **Reference audit:** every `AddRef`/`Release` we do on a game object (vtable outside our DLL), and every
     reference the game hands us (QueryInterface, out-parameters, lot lists, save references), is logged at
     verbose level under its own cap. Each line gives the object, its class (`GetGZCLSID`), its vtable as
     module+offset, the count returned and our call site.
     - Always logged: `WARNING: released last reference to game object …` and `WARNING: over-release …`. A site that
       frees a temporary the game created for us (a new object returned through an out-parameter with one reference,
       e.g. the demand glue's resource key list) releases it with `FC4_RELEASE_CREATED` after `FC4_ACQUIRED`: that
       expected free is counted in the summary (`temporaries created for us`), not warned.
     - At each simulator or budget `Shutdown`: the net count per class and the objects we still hold.
5. **Checked build:** `cmake -S engine -B build-x86-checked -G Ninja -DCMAKE_BUILD_TYPE=RelWithDebInfo
   -DFC4_CHECKED=ON`.
   - The plugin is built with `/Od /RTC1`, so an argument-count or callee-pop mismatch on any vtable call is
     caught at the call.
   - RTC failures go to the log, not a dialog: the kind, the call site as `FreeCity4.dll+offset` with our
     function and line, the frames, the activity and the last interface calls. Copy `FreeCity4.pdb` next to the
     DLL for the names.
   - Small buffers we pass to game methods (`GetSize`, `GetLocation`, `GetSimDateParts`, QueryInterface and
     `GetResource` out-pointers, the lot list, `ReadGZSerializable`) are surrounded by canaries. A write past
     them logs `OUT-PARAM OVERRUN`.
   - `FC4_CHECKED_SELFTEST=1` provokes one of each at start-up.

## Conventions

- Every source file starts with an `SPDX-License-Identifier: GPL-3.0-or-later` header.
- Spec gaps are decided in code and tagged `[choice]`. Behaviour the SDK cannot express yet is tagged
  `TODO(spec §n)`.
- Panics never cross the C ABI. Release builds use `panic = "abort"`, and each `fc4_*` entry point also catches
  unwinding.
- Every spec behaviour needs a conformance vector. See [`conformance/README.md`](conformance/README.md).
- Game interfaces are called through vtable slots, so slot order is part of the ABI (foundations §16). An
  interface with overloaded methods is declared in [`plugin/FC4Interfaces.h`](plugin/FC4Interfaces.h) in the
  spec's verified slot order with a unique name per method, never taken from the SDK header: MSVC regroups and
  reverses overloads, so with overloads the layout follows the compiler, not the declaration.
  `tests/vtable_layout_test.cpp` checks every slot of every interface we implement against the spec tables, and
  `plugin_load_test` calls each overload of the real objects through its slot with the game's argument count and
  checks that the stack balances. New code must not implement or call overloads of an interface until its spec
  has a verified slot table. Declared from the foundations §17 tables: `cFC4IGZDate` (implemented by
  `cFC4Date`), `cFC4IGZVariant` (implemented by `cFC4Variant`, the only variant we hand to the game; never pass
  the SDK's `cRZBaseVariant`, whose slots 8 and 60 differ from the game's), `cFC4ISCPropertyHolder`,
  `cFC4ISC4LotManager`, `cFC4ISC4Lot`, and `cFC4ISC4NeighborsSimulator` (from the ABI table: slots 32/33
  return an int64 in EDX:EAX, neighbours NG11). `cFC4ISCStringDetokenizer` (`plugin/FC4CrimeInterfaces.h`) follows lua-host §10.1, whose
  slot 8 is the resource-key `Detokenize` and slot 9 the string one. Interfaces still used through SDK declarations: `cIGZString`, `cISC4LineItem`,
  `cISC4BuildingDevelopmentSimulator`, `cISC4DisasterManager`, `cIGZPersistResourceManager`,
  `cISC4HistoryWarehouse`, `cISC4OrdinanceSimulator`, `cISCResExemplar`.
- **ABI conformance** (`tests/abi_conformance_test.cpp`, CTest `abi_conformance_test`): every declaration above
  is checked against [`docs/abi/interfaces.tsv`](../docs/abi/interfaces.tsv), read at run time. Per slot:
  argument bytes (each argument rounded up to 4, int64/double 8, plus 4 for a hidden result pointer) ==
  `callee_pop`; float/double return ⇔ `ret = x87`; int64 return ⇒ `edx:eax`, and `edx:eax` (medium
  confidence) ⇒ int64. Per interface: slots contiguous and as many as the table's. The slots are decoded from
  the compiler's vcall thunks, so the test checks what MSVC actually laid out. The listing
  `tests/abi_entries.inc` is generated, not checked in: the CMake target `fc4_abi_entries` runs
  `tests/gen_abi_entries.py` on every build and this test depends on it, so it is always current and a stale
  copy can never be compiled. Run `python tests/gen_abi_entries.py` (from `engine/`) yourself to refresh it
  without a build; with no Python 3 this test is skipped, like `ret_channel_check`. `--dump` prints every slot
  next to its table row. SDK methods that disagree with the game but are never called by the plugin are
  waived (`kWaivers`); the waiver lapses, and the test fails, as soon as a file in `plugin/` calls one.
  Reviewed slot-count differences (class-private trailing virtuals) are in `kReviewedSlotCounts`. Interfaces
  the plugin only calls, and which `interfaces.tsv` lists under another name, are checked against a game
  class's vtable in `docs/abi/vtables.tsv` instead (entries named `vt:<class>`, e.g. `vt:cSC4City::vftable`
  for `cISC4City`); there the class may have more slots than the interface declares.
- **Return channels** (`tests/check_ret_channels.py`, CTest `ret_channel_check`, needs Python 3): every place
  the plugin uses a game method's return value, against the slot's `ret` column and what the specs say it
  returns (`SPEC_FACTS`, each entry with its citation). MSVC reads only AL for a `bool`, and `eax-or-void` (the
  table's default) may be a method that sets nothing (the GetCityLocations no-load), so a `bool` read of a
  non-`al` slot fails until the spec's answer is in `SPEC_FACTS` or the call carries an `[abi-ret]` comment
  saying why the value is fine; a slot the spec gives as "none" or as a wide count always fails. `--verbose`
  also lists the spec-silent framework slots trusted as the SDK declares them, `--impl` the `eax` rows the
  plugin implements as `bool`.
- **Test seam:** `FreeCity4.dll` exports `FC4TestSeam` ([`plugin/FC4TestSeam.h`](plugin/FC4TestSeam.h)) for
  tests that load it without a game: a sink that sees every budget message as posted (budget §10: void1 =
  IGZUnknown = the sender), a fake city for the observer, and direct observer ticks and messages. The game
  never calls it. `plugin_autotest_test` runs child processes (clean, corrupt, stalled, ended, save, and a Rust
  panic) that end through the autotest's `TerminateProcess` (the panic through fail-fast). The harness lane adds
  five with `autotest_end_date`: an end date alone, a timeout that comes first, an end date already past with
  `autotest_end = quit`, a value that is not a date, and a script `save` due on the end date (the verdict waits
  for it). `plugin_ini_script_test` runs a `script` through a stub zone manager on the fake city (slot 36; its
  slot 11 records the region layout and the arguments, and clears a bit as the game does) and our simulator;
  seam op 46 loads a refused, an unreadable, an empty, a late and a save script (the fake app's slot 16 counts
  the saves, which wait for the observer tick in file order). `plugin_ini_dialogs_test` plays DoModalWin and its
  loop on a fake window manager, legacy message server and windows (seam op 47; every slot the closer must not
  call traps): button 0 on a later frame, never inside the begin notice; the EndModal fallback for a dialog that
  ignores it; a stale window (no call after IsWindowValid); nested dialogs (inner first); SC4GameDelay's window
  left alone; a window not yet modal; a late subscription that finds the open dialog through the focus; and, in
  child processes, `close_dialogs = 0`, unset and not 0/1 (no subscription, no call, no line) and the
  environment alone. A sixth autotest child checks `dialogs_closed=1` in the verdict.
- Return types are part of the ABI too: SDK declarations can be wrong (budget erratum:
  `cISC4DepartmentBudget` slot 26 returns a float POINTER in EAX, not a float in ST0; `cIGZDate` slots 33–36
  return a `uint32` day number and leave the date alone; `cISC4Lot` slot 61 returns `SC4Percentage` by value
  through a hidden result pointer; from the ABI table: `cIGZVariant` slot 41 and `cISC4Lot` slot 15 return in
  ST0, `cISC4BudgetSimulator` slots 20–23 in EAX only). Such interfaces are declared in `FC4Interfaces.h`, and the slot tests check
  the x87 stack and the stack balance: only slots the spec says return a float may leave a value there.

`simgrid_shadow = 1` (environment `FC4_SIMGRID_SHADOW`, default 0) enables the
SG3 owner/getter inventory record shadow at PostCityInit and each month. It
reacquires every role, walks the current growth-developer map in key order,
captures each grid's record, and compares Rust reads of its metadata and every
typed tract against the native getters. The shipped inventory is 86 roles;
modified developer maps change that count. `simgrid-shadow:` reports exact roles,
mismatches, unavailable roles, cells and bytes. Missing roles and refused reads
cannot count as exact. This verifies record reads over simgrid.md sections
6.2.1/6.2.2, not simulation algorithms or owner-private/map-view grids. The older
`simgrids_shadow` fixture/natural-operation switch remains separate.
# LR12 master lot record observation

`lotrec_shadow = 1` (default `0`, environment `FC4_LOTREC_SHADOW`) captures all
current master-list lots at PostCityInit and every month. It compares the full
record, including properties/trips and reference identities, through the Rust
record reader without Init. `lotrec-shadow:` reports expected/visited/captures/
exact counts and explicit errors. Off-list lots, business-unit payloads and all
occupants (including unspecified prop subclasses) are outside this scope.

`lottrip_shadow = 1` (default `0`, environment `FC4_LOTTRIP_SHADOW`) reports
`cSC4TripData` record evidence separately as `lottrip shadow` load/month lines.
It uses the same complete master-lot capture and full Rust re-encoding as
`lotrec_shadow`; enabling both shares one walk. Trips receive exact credit only
when their entire containing lot compares byte-for-byte. Inventory/capture/reader
failures remain mismatches even when no trip count can be recovered. `empty=1`
supplies no nonempty trip evidence. Off-list trips are outside this inventory.
LR15's separate scoped owner inventories are described below.

`netrules_shadow = 1` enables the default-off, record-only network tile-rule
observer (NR16/NR17). It requires the published GOG fingerprint, fixed image
base and every byte at all three observation sites; any validation failure
leaves the observation set inactive. Reload completion compares parsed tile
tables in order, including duplicate keys. Actual original evaluations run
once; copied neighbourhood requests drive detached Rust replay. Monthly
`netrules shadow` lines report cumulative table/result differences, incomplete
inputs, unavailable observations and evaluated sample counts. `exact=1`
requires tested samples and zero problems. Intersection tables and solver
paths that bypass Evaluate remain explicitly unobserved. No native result,
cache query, reference ownership or saved-city state is replaced.
`strings_shadow = 1` (default off) records monthly constructed same-process
`cRZString` comparisons. A live city-name prefix (at most 96 bytes), two ASCII
witnesses, a counted interior-NUL witness and an empty witness exercise assignment,
copy, resize, append, insert, replace, erase, sensitive/ASCII-insensitive equality,
ordering and forward/reverse search, plus integer/string Sprintf. Each sample logs
hex inputs and both outputs; `strings shadow YYYY-MM-DD ...; mismatches=N` sums
every comparison, including core refusals. Only private strings are mutated.
There are at most 78 comparisons per month. This measures constructed native
calls, not interception of natural call traffic. CompareTo uses equal-prefix
length differences; other return magnitudes remain RZS9. Alternate hexadecimal
zero padding (RZS10) and empty-haystack Find (RZS11) remain open after native
differences; plain zero-padded integer formatting and empty RFind are measured.
Finite decimal extraction (RZS7), pointer formatting, nonvirtual substring/case
helpers, detokenizer and language owners remain open for this shadow. Non-ASCII
case/format probes are excluded and counted (RZS2/RZS5); other operations retain
their counted bytes. The shadow never changes locale or replacement dispatch.
- `lua_shadow = 1` (default off) records the original Lua extension collector
  and app VM's registered callback identities at city load and monthly. The
  `lua-registry shadow YYYY-MM-DD` lines compare all 47 ordered names/flags,
  replay core `register_api(true/false)` with the recorded native callback
  bindings, and compare globals, `game` fields and `_tf_`, preserving the native
  stack. No nondeterministic values are excluded. This is bounded registration
  verification: full chunk/global/table/call recording and VM replay remain
  open under lua-host LH6-LH7 and receive no full Lua coverage credit.
  LH6/LH7 now establish that public access supplies no complete observer or
  replay-input contract; full replay stays disabled. Current house summaries use
  `lua shadow YYYY-MM-DD ...: scope=registration ... mismatches=N`, replacing the
  earlier `lua-registry` summary prefix without broadening verification scope.
  Capture failures include missing C functions and unbalanced stacks. Portable
  preferred-address labels are limited to validated executable-image callbacks;
  comparisons retain exact actual pointers and never execute foreign callbacks.
`scres_shadow = 1` enables record-only forwarding of the original binary/text
token adapter's numeric, descriptor and resource-key reads, starting before
application initialization. It replays consumed bytes through the production
core and compares output bytes, status, counter and byte consumption. It defaults
to off and never replaces a resource service. Dated `scres shadow` lines report
cumulative observations at city load and monthly; empty samples are failures.
This is **partial adapter-read coverage**: string, parser, image and writer
capture remains off pending SCR8 in `docs/specs/questions/sc-resources.md`.
Capture uses SDK reads with the original post-call position restored; it does
not claim to be a cursor-free observer. Do not credit the complete subsystem
or add the key to a general shadow profile from this partial measurement.

`scres_file_shadow = 1` adds SCR10 physical file-parser replay to the natural
command observation. It defaults to off. Constructor/open association is scoped
to each recursive file invocation; borrowed ReadLine output is copied with its
length, bool and logical position before native comment processing. The core's
production file loop replays include paths, blank/comment lines, physical line
numbers, false-read completion, open failures and inclusive return counts without
resynchronization. Commands remain an observed execution boundary; this does not
verify graphics-rule state, arbitrary command exceptions or ReadLine's byte
decoder. False reads are labeled EOF-or-failure, as their native bool cannot
distinguish the two. Wide-mode samples are refused. The existing command-token
and constructed-adapter measurements remain independent.

SCR11 physical preprocessing runs in the production file loop: literal `#<`
erases its tail before an active block searches for `#>` and keeps its suffix.
The flag is local to each file. Suppressed reads still count, EOF in an active
block succeeds, and markers in quotes or ordinary comments participate. Raw
replay compares these decisions without native suppression inputs or resyncs;
the earlier foreign-preprocessing control replay has been removed.

### Construction occupant getter shadow

`construction_shadow = 1` (default off; `FC4_CONSTRUCTION_SHADOW` also works)
samples initialized construction occupants on city load and each month through
the documented typed occupant iterator. It compares native position bitwise with
the Rust construction owner using only native bounds as input, and checks the
construction class and occupant type (lot-records §9.2/§9.3, T79).
`construction shadow` lines report occupants, compared fields, skipped
uninitialized instances, empty samples and mismatches. An empty sample alone is
not evidence for the owner. This is partial getter coverage: no native Write,
Read, Init, Shutdown, tick, Draw, schedule or rendering probe is performed.
`lotunit_shadow = 1`, `lotbasetexture_shadow = 1`, and `lotwall_shadow = 1`
enable independent LR15 record observations at load/month boundaries (all default
to `0`; environment variables `FC4_LOTUNIT_SHADOW`, `FC4_LOTBASETEXTURE_SHADOW`,
and `FC4_LOTWALL_SHADOW`). Units are the distinct nonnull GetUnit results from
the complete master-lot walk; empty/off-master-only units are outside this scope.
Textures and walls are the registered occupants selected by full inclusive
world-grid queries (0,0)…(191,191), including padding and objects outside lots.
Neither inventory establishes an all-allocation census or replacement coverage.
The `lotunit shadow`, `lotbasetexture shadow`, and `lotwall shadow` lines report
discovery completeness, capture/exact counts, failures and successful empty
inventories separately. Full record comparisons preserve reference identities,
saved texture frames and unnamed bytes; wall-mask growth on Read is a mismatch.

`reach = on` enables a native entry census (default off). `reach_cap = 32`
limits counted entries per function per window; `reach_window = month` uses
the observer's calendar messages, including the initial partial month.
`reach_contracts` selects the ABI `functions.tsv` (default: the build source
tree's `docs/abi/functions.tsv`). Missing/invalid contracts disable tracing.
`reach_output` selects the output TSV; harness requests should use the absolute
worktree `build-runs/game-results/<label>.reach.tsv` path. The default is the
plugin log path plus `.reach.tsv`. No executable file is opened.

Entries are snapshotted before subsystem entry patches and armed after the
city-load hooks and probes. Changed entries, preexisting jumps/breakpoints,
trap-sensitive first instructions, and exception/loader runtime names are
skipped with reasons. Tracing ends before city shutdown restores other hooks.
Monthly constructed shadow passes and deferred load probes (overlay, queued
DBPF and observe snapshot) pause the census process-wide, avoiding
unbounded traps on fixture-heavy getters. Scripted game actions remain counted.
`reach_include_probes = 1` disables these pauses for call-count calibration
(default 0). Calls on other threads during excluded passes are also omitted.
Consequently this is
a census of the running city, excluding load/shutdown and monthly fixtures.
At shutdown the log reports one `reach window N date=YYYY-MM-DD new=N total=N`
line per window, including empty windows; `new` counts first-seen functions
and `total` is cumulative. The initial window uses the city-load date.
The log reports `reach summary functions=... armed=... reached=... skipped=...
windows=... traps=...`, followed by ignored/aborted/slot-exhaustion/save counters.
`traps` includes breakpoint and single-step exceptions.

The breakpoint is absent while its first instruction executes. Simultaneous
calls to the same entry can pass during that interval, so counts are lower
bounds, not complete call totals for concurrent functions. The synthetic DLL
test measures this loss separately from exact serialized call-count controls.
Executable pages stay writable and the VEH's DLL stays pinned until process
exit so delayed exceptions remain serviceable. This diagnostic is opt-in.

## S3 in-place oracle experiment (WIP)

All switches default off. `inplace = all` or a comma-separated list of names from
`FC4Inplace.cpp` selects 20 view-utility functions. Natural calls sample the first
50 and then one in 20. `inplace_fixture = 1` additionally runs labelled,
constructed inputs at city load. `inplace_mutate = <names>` changes one result
word on the first exact sample and logs its detection through the replay
comparator. `inplace summary` separates natural and fixture counts.

The initial typed wrappers faulted in inplace26-fixture-1. The state bridge now
preserves incoming and outgoing integer registers, flags, x87/SSE state and
control modes, including nested entries. Seven synthetic machine-state cases
and a completed mutation fixture cover that correction. The experiment still
has outside-change signals, refused aliases and unsupported entry encodings.
Final measured result: INCONCLUSIVE. Both corrected runs are clean; all 18,278
comparisons are exact, 33/33 mutations are detected, and 14/20 functions qualify
without outside signals or refusals. Measured throughput misses 11 KB/hour.
See `tests/spike-evidence/spike-inplace26/` for measured values and rejections.
Capture observes net changes in containing writable private committed regions
(64 MiB cap). It misses undone writes, unrelated allocations and image/global
storage. Concurrent neighbour changes have unknown attribution. Unsupported or
occupied entries fail closed. Do not enable this experiment in normal runs.

### Function compare harness (run 7)

The S3 facility above is now registry driven: `plugin/compare/<Class>.tsv`
selects native-layout Rust adapters without per-function plugin C++ changes.
Use `compare = <names|class:Class|all>` (`inplace` remains an alias). Everything
is off without that selection. Per-row compare/record/replace modes, bounded
value-only corpus/case exports and nearby-input replay are documented as a
stable interface in [docs/harness.md](../docs/harness.md#stable-function-compare-interface-run-7).

### Construction quit experiments

`autotest_quit_post = 1` (env `FC4_AUTOTEST_QUIT_POST`, default off) makes
`autotest_end = quit` post the application's native `0xC7AA61C6` command to
its legacy message target. The main loop delivers it to the game's quit
handler after the observer's tick. The verdict still requires
`PostSystemServiceShutdown`; a missing service/target is a failed run, never
a clean termination. Native target-post has no Boolean result (00930273).

`script_port_probe = 1` logs the building pointer/type after the port action.
`script_port_instant = 1` disables its construction animation. Both default
off and affect only scripted port actions; they are diagnostic controls.

The `findings` scenario `quit-construction28` preserves a native construction
shutdown fault at `0x005D0F94`: both direct and posted quit fault after its
one animated airport action, including after a pause or full save. Disabling
the animation avoids that incomplete object and exits cleanly; it is a
separate control, not a correction to the finding. The suite retains
`clean-exit-crash`; it does not enable either diagnostic switch by default.

### Formats replacement

`enable = formats` is off by default. It selects the existing FC4 exemplar and
cohort resource factories (data-formats section 8.1), including their FC4
property holders and property values backed by `fc4-formats`. The `exemplars`
key keeps its existing behavior; selecting both installs the factories once.
Packed-file segments remain original and are the next replacement step.
`formats_shadow = 1` checks the replacement against the formats crate and labels
the comparison as a replacement consistency check. The install-time exemplar
contract fixture also compares the retained original factories with ours.

`dbpf_shadow_hold_date = 1` opts into holding the simulation date throughout
each DBPF shadow acquisition. It uses a balanced hidden pause, which also
suppresses idle agents and can change the first traffic pass after load.
It defaults off; ordinary DBPF shadows continue their complete comparisons
on framework ticks. Interrupted acquisitions still require an observed native
save return and the same city/date before replay (data-formats section 8.3.2);
an advanced date remains a refusal, never an exact old-date sample.
