# SPDX-License-Identifier: GPL-3.0-or-later
#
# FreeCity4 engine build.
#
#   - The simulation core is Rust (Cargo workspace in this directory). CMake
#     invokes cargo to build the fc4-capi static library for the same
#     architecture as the C++ compiler, and runs `cargo test` from CTest.
#   - On Windows/MSVC x86 (32-bit) it also builds the GZCOM plugin
#     FreeCity4.dll (engine/plugin) against the gzcom-dll SDK submodule.
#
# Typical Windows build (x86 Developer environment):
#   cmake -S engine -B build-x86 -G Ninja -DCMAKE_BUILD_TYPE=RelWithDebInfo
#   cmake --build build-x86 && ctest --test-dir build-x86 --output-on-failure

cmake_minimum_required(VERSION 3.20)
project(FreeCity4 VERSION 0.1.0 LANGUAGES C CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

# Static CRT, release flavour in every configuration: the Rust staticlib is
# built with +crt-static (libcmt), and FreeCity4.dll must not depend on any
# runtime DLL.
set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreaded")

enable_testing()

# NR17 reference/byte-mutation/base/cleanup lifetime fixtures (no game image).
add_executable(fc4_netrules_observation_test tests/netrules_observation_test.cpp)
target_include_directories(fc4_netrules_observation_test PRIVATE plugin)
add_test(NAME netrules_observation_test COMMAND fc4_netrules_observation_test)

# Checked plugin build (in-game bisection tooling): no optimisation and the
# MSVC run-time checks (/RTC1: ESP verified after every call through a
# pointer or vtable, local-variable frames guarded), plus out-parameter
# canaries (plugin/FC4Checked.h). Use a separate build directory, e.g.
#   cmake -S engine -B build-x86-checked -G Ninja -DCMAKE_BUILD_TYPE=RelWithDebInfo -DFC4_CHECKED=ON
option(FC4_CHECKED "Build the checked plugin (/Od /RTC1, out-parameter canaries)" OFF)
if(FC4_CHECKED AND MSVC)
    # /RTC1 cannot be combined with optimisation: strip /O* from every configuration.
    foreach(fc4_lang C CXX)
        foreach(fc4_cfg "" _DEBUG _RELEASE _RELWITHDEBINFO _MINSIZEREL)
            string(REGEX REPLACE "/O[0-9a-z]+" "" CMAKE_${fc4_lang}_FLAGS${fc4_cfg} "${CMAKE_${fc4_lang}_FLAGS${fc4_cfg}}")
            string(REGEX REPLACE "/RTC[1csu]+" "" CMAKE_${fc4_lang}_FLAGS${fc4_cfg} "${CMAKE_${fc4_lang}_FLAGS${fc4_cfg}}")
        endforeach()
    endforeach()
    message(STATUS "FC4_CHECKED: plugin built with /Od /RTC1 and out-parameter canaries")
endif()




# Debug info embedded in each object (/Z7) instead of a shared per-target PDB (/Zi): the objects become
# cacheable (sccache, below) and the compiler no longer talks to mspdbsrv. The linker still writes the PDB.
if(MSVC)
    foreach(fc4_lang C CXX)
        foreach(fc4_cfg "" _DEBUG _RELEASE _RELWITHDEBINFO _MINSIZEREL)
            string(REGEX REPLACE "/Z[iI]" "/Z7" CMAKE_${fc4_lang}_FLAGS${fc4_cfg} "${CMAKE_${fc4_lang}_FLAGS${fc4_cfg}}")
        endforeach()
    endforeach()
endif()

# Compiler cache. Lanes build in fresh git worktrees, so every lane used to compile everything from scratch.
# With sccache installed (`cargo install sccache --version 0.18.0 --locked`; docs/ops/coordinating.md "Build
# cache") the C/C++ compiler and rustc go through it. fc4verify uses normalized cache keys and maps
# replayed MSVC header dependencies into the consuming worktree before Ninja records them. The cargo env is
# the one fc4verify uses: sccache hashes every CARGO_* variable, and incremental builds are not cacheable.
option(FC4_COMPILER_CACHE "Compile C/C++ and Rust through sccache when it is installed" ON)
set(FC4_CARGO_ENV "")
if(FC4_COMPILER_CACHE)
    find_program(FC4_SCCACHE_EXECUTABLE sccache
        HINTS "$ENV{CARGO_HOME}/bin" "$ENV{USERPROFILE}/.cargo/bin" "$ENV{HOME}/.cargo/bin")
    if(FC4_SCCACHE_EXECUTABLE)
        if(DEFINED ENV{SCCACHE_BASEDIRS} AND NOT "$ENV{SCCACHE_BASEDIRS}" STREQUAL "" AND NOT FC4_CACHE_LAUNCHER)
            message(FATAL_ERROR "SCCACHE_BASEDIRS requires FC4_CACHE_LAUNCHER to repair cached /showIncludes; use fc4verify")
        endif()
        # fc4verify supplies a /showIncludes mapper before sccache: cache hits
        # may contain another worktree's absolute header paths (train278).
        set(CMAKE_C_COMPILER_LAUNCHER ${FC4_CACHE_LAUNCHER} "${FC4_SCCACHE_EXECUTABLE}")
        set(CMAKE_CXX_COMPILER_LAUNCHER ${FC4_CACHE_LAUNCHER} "${FC4_SCCACHE_EXECUTABLE}")
        # Preserve fc4verify's wrapper (registry cwd normalization and uncached
        # clippy diagnostics), also for ctest's Cargo command.
        set(FC4_CARGO_ENV CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0)
        if(NOT DEFINED ENV{RUSTC_WRAPPER})
            list(APPEND FC4_CARGO_ENV "RUSTC_WRAPPER=${FC4_SCCACHE_EXECUTABLE}")
        endif()
        message(STATUS "FC4_COMPILER_CACHE: compiling through ${FC4_SCCACHE_EXECUTABLE}")
    else()
        message(STATUS "FC4_COMPILER_CACHE: sccache not found, compiling without a cache")
    endif()
endif()

# ---- Rust core (cargo) ----------------------------------------------------------

find_program(CARGO_EXECUTABLE cargo
    HINTS "$ENV{CARGO_HOME}/bin" "$ENV{USERPROFILE}/.cargo/bin" "$ENV{HOME}/.cargo/bin"
    REQUIRED)

set(FC4_IS_WIN32_X86 OFF)
if(MSVC)
    if(CMAKE_SIZEOF_VOID_P EQUAL 4)
        set(FC4_RUST_TARGET "i686-pc-windows-msvc")
        set(FC4_IS_WIN32_X86 ON)
    else()
        set(FC4_RUST_TARGET "x86_64-pc-windows-msvc")
    endif()
endif()
set(FC4_RUST_TARGET "${FC4_RUST_TARGET}" CACHE STRING "Rust target triple for fc4-capi (empty = host)")

set(FC4_CARGO_TARGET_DIR "${CMAKE_BINARY_DIR}/cargo")
set(FC4_CARGO_TARGET_ARGS "")
set(FC4_CARGO_OUT_DIR "${FC4_CARGO_TARGET_DIR}/release")
if(FC4_RUST_TARGET)
    set(FC4_CARGO_TARGET_ARGS --target ${FC4_RUST_TARGET})
    set(FC4_CARGO_OUT_DIR "${FC4_CARGO_TARGET_DIR}/${FC4_RUST_TARGET}/release")
endif()

if(MSVC)
    set(FC4_CAPI_LIB "${FC4_CARGO_OUT_DIR}/fc4_capi.lib")
    # From `cargo rustc -- --print native-static-libs`.
    set(FC4_CAPI_SYSTEM_LIBS kernel32 ntdll userenv ws2_32 dbghelp)
else()
    set(FC4_CAPI_LIB "${FC4_CARGO_OUT_DIR}/libfc4_capi.a")
    set(FC4_CAPI_SYSTEM_LIBS pthread dl m)
endif()

# Cargo does its own incremental build; always ask it. It must run from
# engine/ so that engine/.cargo/config.toml (static CRT) applies.
# --target-dir rather than CARGO_TARGET_DIR: sccache hashes CARGO_* variables, and the
# target directory must not split the cache between this build and a lane's own cargo runs.
add_custom_target(fc4_capi_cargo ALL
    COMMAND ${CMAKE_COMMAND} -E env ${FC4_CARGO_ENV}
            ${CARGO_EXECUTABLE} build --release -p fc4-capi ${FC4_CARGO_TARGET_ARGS}
            --target-dir "${FC4_CARGO_TARGET_DIR}"
    WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
    BYPRODUCTS "${FC4_CAPI_LIB}"
    COMMENT "cargo build fc4-capi (${FC4_RUST_TARGET})"
    USES_TERMINAL
    VERBATIM)

add_library(fc4_capi INTERFACE)
target_include_directories(fc4_capi INTERFACE "${CMAKE_CURRENT_SOURCE_DIR}/capi/include")
target_link_libraries(fc4_capi INTERFACE "${FC4_CAPI_LIB}" ${FC4_CAPI_SYSTEM_LIBS})
add_dependencies(fc4_capi fc4_capi_cargo)

# ---- tests ---------------------------------------------------------------------------

# Rust unit tests + conformance vectors, for the same target as the plugin.
add_test(NAME cargo_test
    COMMAND ${CMAKE_COMMAND} -E env ${FC4_CARGO_ENV}
            ${CARGO_EXECUTABLE} test --workspace ${FC4_CARGO_TARGET_ARGS}
            --target-dir "${FC4_CARGO_TARGET_DIR}"
    WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}")

# The C ABI exercised from C++ with this compiler/architecture/CRT.
add_executable(fc4_capi_smoke tests/capi_smoke.cpp)
target_link_libraries(fc4_capi_smoke PRIVATE fc4_capi)
add_test(NAME capi_smoke COMMAND fc4_capi_smoke)

# ---- GZCOM plugin (Windows x86 only) -------------------------------------------------

set(FC4_GZCOM_DIR "${CMAKE_CURRENT_SOURCE_DIR}/third_party/gzcom-dll/gzcom-dll")

if(FC4_IS_WIN32_X86)
    if(NOT EXISTS "${FC4_GZCOM_DIR}/include/cRZCOMDllDirector.h")
        message(FATAL_ERROR "gzcom-dll submodule missing: run `git submodule update --init`")
    endif()

    add_library(FreeCity4 SHARED
        plugin/cFC4Director.cpp
        plugin/cFC4Simulator.cpp
        plugin/cFC4Simulator.h
        plugin/cFC4Date.cpp
        plugin/cFC4Date.h
        plugin/cFC4Message2Standard.cpp
        plugin/cFC4Message2Standard.h
        plugin/FC4Log.cpp
        plugin/FC4Log.h
        plugin/dllmain.cpp
        plugin/FC4Diag.cpp
        plugin/FC4Diag.h
        plugin/FC4Watchdog.cpp
        plugin/FC4Watchdog.h
        plugin/FC4Config.cpp
        plugin/FC4Config.h
        plugin/FC4Seed.cpp
        plugin/FC4Seed.h
        plugin/FC4Pacing.cpp
        plugin/FC4Pacing.h
        plugin/FC4Schedule.cpp
        plugin/FC4Schedule.h
        plugin/FC4RefAudit.cpp
        plugin/FC4RefAudit.h
        plugin/FC4Checked.cpp
        plugin/FC4Checked.h
        plugin/FC4Autoload.cpp
        plugin/FC4Autoload.h
        plugin/FC4Autotest.cpp
        plugin/FC4Autotest.h
        plugin/FC4DiagLogic.h
        plugin/FC4Observer.cpp
        plugin/FC4Fpu.cpp
        plugin/FC4FailFast.cpp
        plugin/FC4FailFast.h
        plugin/FC4Observer.h
        plugin/FC4ClockProbe.cpp
        plugin/FC4ClockProbe.h
        plugin/FC4Popups.cpp
        plugin/FC4Popups.h
        plugin/FC4Fpu.h
        plugin/FC4FpuLogic.h
        plugin/FC4DiagContext.h
        plugin/FC4TestSeam.cpp
        plugin/FC4TestSeam.h
        plugin/cFC4Variant.cpp
        plugin/cFC4Variant.h
        plugin/compat/FC4Compat.cpp
        plugin/compat/FC4Compat.h
        plugin/FC4Properties.cpp
        plugin/FC4Properties.h
        plugin/FC4Streams.cpp
        plugin/FC4Streams.h
        plugin/FC4Subsystems.cpp
        plugin/FC4Subsystems.h
        plugin/cFC4Budget.cpp
        plugin/cFC4Budget.h
        plugin/FC4BudgetHost.cpp
        plugin/FC4BudgetShadow.cpp
        plugin/FC4DemandShadow.cpp
        plugin/cFC4LandValue.cpp
        plugin/cFC4LandValue.h
        plugin/cFC4Weather.cpp
        plugin/cFC4Weather.h
        plugin/cFC4Ordinance.cpp
        plugin/cFC4Ordinance.h
        plugin/cFC4Demand.cpp
        plugin/cFC4Demand.h
        plugin/FC4CoreProps.cpp
        plugin/FC4CoreProps.h
        plugin/FC4HostGrid.cpp
        plugin/FC4HostGrid.h
        plugin/cFC4Power.cpp
        plugin/cFC4Power.h
        plugin/cFC4Plumbing.cpp
        plugin/cFC4Plumbing.h
        plugin/FC4PlumbingShadow.cpp
        plugin/FC4PlumbingShadow.h
        plugin/cFC4Flammability.cpp
        plugin/cFC4Flammability.h
        plugin/cFC4Pollution.cpp
        plugin/cFC4Pollution.h
        plugin/FC4PollutionShadow.cpp
        plugin/FC4PollutionShadow.h
        plugin/FC4PowerShadow.cpp
        plugin/FC4PowerShadow.h
        plugin/cFC4Protection.cpp
        plugin/cFC4Protection.h
        plugin/FC4ProtectionShadow.cpp
        plugin/FC4ProtectionShadow.h
        plugin/cFC4Crime.cpp
        plugin/cFC4Crime.h
        plugin/FC4CrimeInterfaces.h
        plugin/FC4CrimeShadow.cpp
        plugin/FC4CrimeShadow.h
        plugin/cFC4Aura.cpp
        plugin/cFC4Aura.h
        plugin/FC4AuraShadow.cpp
        plugin/FC4AuraShadow.h
        plugin/cFC4Flora.cpp
        plugin/cFC4Flora.h
        plugin/FC4FloraShadow.cpp
        plugin/FC4FloraShadow.h
        plugin/cFC4Landfill.cpp
        plugin/cFC4Landfill.h
        plugin/FC4LandfillShadow.cpp
        plugin/FC4LandfillShadow.h
        plugin/FC4TrafficShadow.cpp
        plugin/FC4TrafficShadow.h
        plugin/FC4WriteCapture.cpp
        plugin/FC4WriteCapture.h
        plugin/FC4LandValueShadow.cpp
        plugin/FC4LandValueShadow.h
        plugin/FC4FlammabilityShadow.cpp
        plugin/FC4FlammabilityShadow.h
        plugin/cFC4Residential.cpp
        plugin/cFC4Residential.h
        plugin/FC4ResidentialShadow.cpp
        plugin/FC4ResidentialShadow.h
        plugin/cFC4ZoneManager.cpp
        plugin/cFC4ZoneManager.h
        plugin/FC4ZonesShadow.cpp
        plugin/FC4ZonesShadow.h
        plugin/FC4GameSet.h
        plugin/FC4PropertyHolder.cpp
        plugin/FC4PropertyHolder.h
        plugin/cFC4Neighbors.cpp
        plugin/cFC4Neighbors.h
        plugin/FC4NeighborsShadow.cpp
        plugin/FC4NeighborsShadow.h
        plugin/FC4GrowthProbe.cpp
        plugin/FC4GrowthProbe.h
        plugin/FC4GrowthProbeHook.cpp
        plugin/FC4GrowthProbeHook.h
        plugin/FC4GrowthProbeLogic.h
        plugin/cFC4Development.cpp
        plugin/cFC4Development.h
        plugin/cFC4GrowthDeveloper.cpp
        plugin/cFC4GrowthDeveloper.h
        plugin/FC4DevelopmentShadow.cpp
        plugin/FC4DevelopmentShadow.h
        plugin/FC4DevelopmentInterfaces.h
        plugin/FC4GameMapBuild.h
        # The traffic simulator, its network model and its pathfinders (M5 glue).
        plugin/cFC4TrafficSimulator.cpp
        plugin/cFC4TrafficSimulator.h
        # The history warehouse and the 24-hour clock, their shared lock and
        # test hooks, and their shadows (docs/specs/history.md, clock24.md).
        plugin/FC4Clock24Shadow.cpp
        plugin/FC4Clock24Shadow.h
        plugin/FC4HistClock.cpp
        plugin/FC4HistClock.h
        plugin/FC4HistoryShadow.cpp
        plugin/FC4HistoryShadow.h
        plugin/cFC4Clock24.cpp
        plugin/cFC4Clock24.h
        plugin/cFC4History.cpp
        plugin/cFC4History.h
        # The tract developer (tract-developer.md): `enable = tract`, tract_shadow.
        plugin/cFC4TractDeveloper.cpp
        plugin/cFC4TractDeveloper.h
        plugin/FC4TractHost.cpp
        plugin/FC4TractInterfaces.h
        plugin/FC4TractShadow.cpp
        plugin/FC4TractShadow.h
        # The lot developer (lot-developer.md): `enable = lotdev`, lotdev_shadow.
        plugin/cFC4LotDeveloper.cpp
        plugin/cFC4LotDeveloper.h
        plugin/FC4LotDevHost.cpp
        plugin/FC4LotDevInterfaces.h
        plugin/FC4LotDevShadow.cpp
        plugin/FC4LotDevShadow.h
        # The commercial / industrial simulators, the civic building simulator and the
        # park manager (cmglue): `enable = commercial, industrial, civic, parks`, their shadows.
        plugin/cFC4Commercial.cpp
        plugin/cFC4Commercial.h
        plugin/FC4CommercialShadow.cpp
        plugin/FC4CommercialShadow.h
        plugin/cFC4Civic.cpp
        plugin/cFC4Civic.h
        plugin/FC4CivicShadow.cpp
        plugin/FC4CivicShadow.h
        plugin/cFC4Parks.cpp
        plugin/cFC4Parks.h
        plugin/FC4ParksShadow.cpp
        plugin/FC4ParksShadow.h
        # The terrain shadow (terrain.md; the glue16 lane): terrain_shadow.
        plugin/FC4TerrainShadow.cpp
        plugin/FC4TerrainShadow.h
        # The terrain replacement (terrain.md §4-§10, §15; the glue18 lane): enable = terrain.
        plugin/cFC4Terrain.cpp
        plugin/cFC4Terrain.h
        plugin/FC4TerrainInterfaces.h
        # The disaster layer's host (disasters.md; glue13) and its shadow: disasters_shadow.
        plugin/FC4DisasterHost.cpp
        plugin/FC4DisasterHost.h
        plugin/FC4DisasterShadow.cpp
        plugin/FC4DisasterShadow.h
        # The glue17 lane: the city situation manager, the demolition utility
        # and the dispatch manager (city-situation.md, demolition.md,
        # dispatch.md) and their shadows: citysit_shadow, demolition_shadow,
        # dispatch_shadow. FC4CoreShadow is the record round trip the three
        # shadows share.
        plugin/FC4CoreShadow.cpp
        plugin/FC4CoreShadow.h
        plugin/FC4CitySitShadow.cpp
        plugin/FC4CitySitShadow.h
        plugin/FC4DemolitionShadow.cpp
        plugin/FC4DemolitionShadow.h
        plugin/FC4DispatchShadow.cpp
        plugin/FC4DispatchShadow.h
        # The dispatch manager's GZCOM class (enable = dispatch).
        plugin/cFC4Dispatch.cpp
        plugin/cFC4Dispatch.h
        # The city situation manager's GZCOM class (enable = citysit): the
        # city-situation.md §8 minimal replacement.
        plugin/cFC4CitySituation.cpp
        plugin/cFC4CitySituation.h
        plugin/FC4DemolitionHost.cpp
        plugin/FC4DemolitionHost.h
        plugin/cFC4Demolition.cpp
        plugin/cFC4Demolition.h
        plugin/FC4DemolitionPriceShadow.cpp
        plugin/FC4DemolitionPriceShadow.h
        # the glue22 lane: the effects manager (docs/specs/effects.md)
        plugin/cFC4Effects.cpp
        plugin/cFC4Effects.h
        plugin/FC4EffectsHost.cpp
        plugin/FC4EffectsHost.h
        plugin/FC4EffectsShadow.cpp
        plugin/FC4EffectsShadow.h
        plugin/cFC4NetworkManager.cpp
        plugin/cFC4NetworkManager.h
        plugin/cFC4NetworkLotManager.cpp
        plugin/cFC4NetworkLotManager.h
        # The replacement network manager's tool stand-in (glue24; §13: the
        # tools are their own subsystem, the slot 5 answer must not be null).
        plugin/cFC4NetworkTool.cpp
        plugin/cFC4NetworkTool.h
        # The zone developer / lot manager (zones.md §1.2, §7-§11.3): enable = zonedev,
        # zonedev_shadow.
        plugin/cFC4ZoneDeveloper.cpp
        plugin/cFC4ZoneDeveloper.h
        plugin/FC4ZoneDevHost.cpp
        plugin/FC4ZoneDevShadow.cpp
        plugin/FC4ZoneDevShadow.h
        # The network manager and network lot manager (network-manager.md): netmgr_shadow.
        plugin/FC4NetMgrHost.cpp
        plugin/FC4NetMgrHost.h
        plugin/FC4NetMgrShadow.cpp
        plugin/FC4NetMgrShadow.h
        # The replacement network tool over the fc4_nettool core (network-tools.md;
        # the glue29 lane): enable = nettool.
        plugin/cFC4NetworkTool.cpp
        plugin/cFC4NetworkTool.h
        plugin/FC4NetToolHost.cpp
        plugin/FC4NetToolHost.h
        # The MySims shadow (mysims.md): mysims_shadow (no replacement).
        plugin/FC4MySimsShadow.cpp
        plugin/FC4MySimsShadow.h
        # The MySim replacement (mysims.md; glue-mysims-replace16): enable = advisors, mysims.
        plugin/FC4MySimGameHost.cpp
        plugin/FC4MySimGameHost.h
        plugin/cFC4MySim.cpp
        plugin/cFC4MySim.h
        plugin/cFC4MySimDispatch.cpp
        plugin/cFC4MySimDispatch.h
        # The occupant manager shadow (occupant-manager.md): occmgr_shadow (no replacement).
        # The occupant manager (occupant-manager.md): enable = occmgr; its shadow
        # is occmgr_shadow and follows whichever manager is active.
        plugin/cFC4OccMgr.cpp
        plugin/cFC4OccMgr.h        plugin/FC4OccMgrShadow.cpp
        plugin/FC4OccMgrShadow.h
        # The prop manager, the lot configuration manager and the tutorial system
        # (prop-lotconfig.md, tutorial.md; the glue15 lane): enable = propmgr, lotcfg,
        # tutorial; propmgr_shadow, lotcfg_shadow, tutorial_shadow.
        plugin/cFC4Tutorial.cpp
        plugin/cFC4Tutorial.h
        plugin/FC4PropLotShadow.cpp
        plugin/FC4PropLotShadow.h
        # The two replacement classes (the glue21 lane): enable = propmgr, lotcfg.
        plugin/FC4PropLotInterfaces.h
        plugin/cFC4PropManager.cpp
        plugin/cFC4PropManager.h
        plugin/cFC4LotConfigManager.cpp
        plugin/cFC4LotConfigManager.h
        # The configuration class the replacement manager allocates (GLUE21-1).
        plugin/cFC4LotConfiguration.cpp
        plugin/cFC4LotConfiguration.h
        # The harness lane's tooling: sim_turbo, autotest_end_date, script, close_dialogs.
        plugin/FC4HarnessLogic.h
        # The prop developer and the city detail manager (glue12b1): `enable = propdev,
        # citydetail`, propdev_shadow, citydetail_shadow.
        plugin/FC4PropCityCommon.cpp
        plugin/FC4PropCityCommon.h
        plugin/cFC4PropDeveloper.cpp
        plugin/cFC4PropDeveloper.h
        plugin/FC4PropDevHost.cpp
        plugin/FC4PropDevShadow.cpp
        plugin/FC4PropDevShadow.h
        plugin/cFC4CityDetail.cpp
        plugin/cFC4CityDetail.h
        plugin/FC4CityDetailShadow.cpp
        plugin/FC4CityDetailShadow.h
        # The airport and seaport developers (port-developers.md; glue12b2 lane):
        # `enable = ports`, ports_shadow.
        plugin/cFC4PortDeveloper.cpp
        plugin/cFC4PortDeveloper.h
        plugin/FC4PortInterfaces.h
        plugin/FC4PortsShadow.cpp
        plugin/FC4PortsShadow.h
        plugin/FC4Turbo.cpp
        # The automata layer's host tables (automata.md 8, 9; the glue19 lane) and
        # its shadow: automata_shadow.
        plugin/cFC4Automata.cpp
        plugin/cFC4Automata.h
        plugin/cFC4AutomatonObject.cpp
        plugin/cFC4AutomatonObject.h
        plugin/FC4AutomataDriver.cpp
        plugin/FC4AutomataDriver.h
        plugin/FC4AutomataHost.cpp
        plugin/FC4AutomataHost.h
        plugin/FC4AutomataInterfaces.h
        plugin/FC4AutomataShadow.cpp
        plugin/FC4AutomataShadow.h
        # The harness lane's tooling: sim_turbo, autotest_end_date, script, close_dialogs.
        plugin/FC4HarnessLogic.h
        plugin/FC4Turbo.cpp
        plugin/FC4Turbo.h
        plugin/FC4Script.cpp
        plugin/FC4Script.h
        plugin/FC4Dialogs.cpp
        plugin/FC4Dialogs.h
        plugin/FC4WinInterfaces.h
        # The devperf lane's tooling: perf_profile (where a month's real time goes).
        plugin/FC4Perf.cpp
        plugin/FC4Perf.h
        # The glue-formats4 lane: the data-formats shadow (FC4FormatsShadow.h).
        plugin/FC4FormatsShadow.cpp
        plugin/FC4FormatsShadow.h
        # The layout-compatible grid class family (simgrid.md; glue-simgrids4): enable = simgrids.
        plugin/FC4SimGridLogic.h
        plugin/cFC4SimGrid.cpp
        plugin/cFC4SimGrid.h
        plugin/FC4SimGridProbe.cpp
        plugin/FC4SimGridProbe.h
        # The glue-exemplars4 lane: the exemplar/cohort factory replacement (FC4ExemplarFactory.h).
        plugin/FC4ExemplarFactory.cpp
        plugin/FC4ExemplarFactory.h
        # The cSpanRender4444 native bridge (span-render-4444.md §1.1; glue-span4444-impl4): enable = span4444.
        plugin/FC4SpanRender4444.cpp
        plugin/FC4SpanRender4444.h
        # The cSpanRender1555 native bridge (glue-span1555-impl4): enable = span1555.
        plugin/FC4Span1555Shadow.cpp
        plugin/FC4Span1555Shadow.h
        # The native draw-context route for both span bridges (span-render-4444.md §1.3; glue-span-context-final4).
        plugin/FC4SpanContext.cpp
        plugin/FC4SpanContext.h
        # The glue-advisors-final4 lane: the advisor window slots 23-28 (advisors.md §12.1.1, AD4).
        plugin/FC4AdvisorWindow.h
        plugin/FC4AdvisorWindowGame.cpp
        plugin/cFC4AdvisorSystem.h
        plugin/cFC4AdvisorSystem.cpp
        # The glue-span565-impl4 lane: the 565 span renderer bridge (FC4Span565Bridge.h).
        plugin/FC4Span565Bridge.cpp
        plugin/FC4Span565Bridge.h
        # The glue-advisors-activate4 lane: the advice subject class (advisors.md §12.4).
        plugin/cFC4AdviceSubject.h
        plugin/cFC4AdviceSubject.cpp
        plugin/FC4AdvLife.h
        plugin/cFC4AdvisorObjects.h
        plugin/cFC4AdvisorObjects.cpp
        # gzcom-dll SDK sources (LGPL-2.1-or-later). Not the SDK's
        # Opt-in 8888 bridge (span-render-8888.md section 1.1).
        plugin/FC4SpanRender8888.cpp
        plugin/FC4SpanRender8888.h
        # Independent native grid verification (no public ABI table changes).
        plugin/FC4SimGridsShadow.cpp
        plugin/FC4SimGridsShadow.h
        # cRZMessage2Standard: foundations §5.1 says it differs from the game.
        "${FC4_GZCOM_DIR}/src/cRZCOMDllDirector.cpp"
        "${FC4_GZCOM_DIR}/src/cRZBaseString.cpp"
        "${FC4_GZCOM_DIR}/src/SC4String.cpp"
        "${FC4_GZCOM_DIR}/src/cRZBaseVariant.cpp"
        "${FC4_GZCOM_DIR}/src/StringResourceManager.cpp"
        # Constructed-native industrial verification; no replacement or natural interception.
        plugin/FC4IndustrialShadow.cpp
        plugin/FC4IndustrialShadow.h
        plugin/compat/FC4IndustrialCompat.h
        # Opt-in native language utility verification (production core transport).
        plugin/FC4LanguageVerify.cpp
        plugin/FC4LanguageVerify.h
        # Owned cGZFontSys verification (drawing-fonts sections 12/12.1).
        plugin/FC4FontSysVerify.cpp
        plugin/FC4FontSysVerify.h
        # Opt-in span8 checked adapter and native type2 route (SR9/SR10).
        plugin/FC4SpanRender8.cpp
        plugin/FC4SpanRender8.h
        plugin/FC4Span8Fixture.inc
        plugin/FC4FeatureShadow.cpp
        plugin/FC4FeatureShadow.h
        plugin/FC4PersistManagerVerify.cpp
        plugin/FC4PersistManagerVerify.h
        # Bounded independent plain prop factory verification (default off).
        plugin/FC4PropOccupantVerify.cpp
        plugin/FC4PropOccupantVerify.h
        plugin/FC4PropOccupantFixtures.h
        plugin/FC4PropOccupantCompare.h
        plugin/compat/FC4PropOccupantAuthority.h
        # SG3 complete owner/getter inventory record shadow (default off).
        plugin/FC4SimGridRecordShadow.cpp
        plugin/FC4SimGridRecordShadow.h
        # LR12 current master-list full lot records (default off).
        plugin/FC4LotRecordShadow.cpp
        plugin/FC4LotRecordShadow.h
        plugin/FC4LotRecordObservation.h
        # Record-only, opt-in network occupant shadow.
        plugin/FC4NetOccShadow.cpp
        plugin/FC4NetOccShadow.h
        # NR16/NR17 default-off record-only network-rule observation.
        plugin/FC4NetRulesShadow.cpp
        plugin/FC4NetRulesShadow.h
        plugin/FC4NetRulesObservation.h
        # Default-off constructed ORIGINAL cRZString monthly shadow.
        plugin/FC4StringsShadow.cpp
        plugin/FC4StringsShadow.h
        # Opt-in partial resource/token adapter observation replay.
        plugin/FC4SCResShadow.cpp
        plugin/FC4SCResShadow.h
        # Default-off record-only ordinance monthly shadow.
        plugin/FC4OrdinanceShadow.cpp
        plugin/FC4OrdinanceShadow.h
        # Read-only persistence cache type projection (PR9/SCR11 gaps).
        plugin/FC4PersistShadow.cpp
        plugin/FC4PersistShadow.h
        # LR14/T79 default-off live construction getters.
        plugin/FC4ConstructionShadow.cpp
        plugin/FC4ConstructionShadow.h
        # Default-off public network-tool draw projection (private native tools).
        plugin/FC4NetToolDragShadow.cpp
        plugin/FC4NetToolDragShadow.h
        # Default-off live regional-city record/getter differential.
        plugin/FC4RegionalCityShadow.cpp
        plugin/FC4RegionalCityShadow.h
        # Default-off live query pick predicates (partial owner observation).
        plugin/FC4QueryToolShadow.cpp
        plugin/FC4QueryToolShadow.h
        # Default-off GZ framework constructed-buffer and command differential.
        plugin/FC4GZFrameworkShadow.cpp
        plugin/FC4GZFrameworkShadow.h
        plugin/FC4GZFramework.h
        plugin/FC4GZBuffer.cpp
        plugin/FC4GZCommandServer.cpp)
    target_include_directories(FreeCity4 PRIVATE "${FC4_GZCOM_DIR}/include" plugin)
    target_compile_definitions(FreeCity4 PRIVATE
        _CRT_SECURE_NO_WARNINGS NOMINMAX WIN32_LEAN_AND_MEAN
        FC4_VERSION_STRING="${PROJECT_VERSION}")
    # The fixed schedule's code: tools/golden.py keys references on it and refuses a candidate DLL whose code differs.
    set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS plugin/FC4Schedule.cpp plugin/FC4Schedule.h)
    # (line endings normalised: a checkout with core.autocrlf has CRLF)
    file(READ "${CMAKE_CURRENT_SOURCE_DIR}/plugin/FC4Schedule.cpp" FC4_SCHEDULE_CPP)
    file(READ "${CMAKE_CURRENT_SOURCE_DIR}/plugin/FC4Schedule.h" FC4_SCHEDULE_H)
    string(REPLACE "\r\n" "\n" FC4_SCHEDULE_SOURCE "${FC4_SCHEDULE_CPP}${FC4_SCHEDULE_H}")
    string(SHA256 FC4_SCHEDULE_CODE "${FC4_SCHEDULE_SOURCE}")
    string(SUBSTRING "${FC4_SCHEDULE_CODE}" 0 16 FC4_SCHEDULE_CODE)
    set_source_files_properties(plugin/FC4Schedule.cpp PROPERTIES
        COMPILE_DEFINITIONS "FC4_SCHEDULE_CODE=\"${FC4_SCHEDULE_CODE}\"")
    target_compile_options(FreeCity4 PRIVATE /W3 /permissive- /utf-8 /EHsc)
    if(FC4_CHECKED)
        target_compile_definitions(FreeCity4 PRIVATE FC4_CHECKED=1)
        target_compile_options(FreeCity4 PRIVATE /Od /Oy- /RTC1)
    endif()
    target_link_libraries(FreeCity4 PRIVATE fc4_capi)
    set_target_properties(FreeCity4 PROPERTIES OUTPUT_NAME "FreeCity4" PREFIX "")
    # A fixed load address (no ASLR): the fixed schedule's heap lies below it, and the game's
    # pointer-keyed sets that hold this DLL's objects keep one order (FC4Schedule.cpp kDllBase).
    target_link_options(FreeCity4 PRIVATE /DYNAMICBASE:NO /BASE:0x5C000000)
    # Exports: GZDllGetGZCOMDirector (from cRZCOMDllDirector.cpp) and the
    # test seam FC4TestSeam (plugin/FC4TestSeam.h; never called by the game).

    # foundations §7 item 12: the plugin's standard message class.
    add_executable(fc4_message_test
        tests/message_test.cpp
        plugin/cFC4Message2Standard.cpp
        "${FC4_GZCOM_DIR}/src/cRZBaseString.cpp")
    target_include_directories(fc4_message_test PRIVATE "${FC4_GZCOM_DIR}/include" plugin)
    target_compile_definitions(fc4_message_test PRIVATE _CRT_SECURE_NO_WARNINGS NOMINMAX)
    target_compile_options(fc4_message_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    add_test(NAME message_test COMMAND fc4_message_test)

    # Single ownership of elements pushed into game containers
    # (plugin/FC4GameContainers.h; in-game run 8 heap corruption).
    add_executable(fc4_game_container_test
        tests/game_container_test.cpp
        "${FC4_GZCOM_DIR}/src/SC4String.cpp"
        "${FC4_GZCOM_DIR}/src/cRZCOMDllDirector.cpp"
        "${FC4_GZCOM_DIR}/src/cRZBaseString.cpp")
    target_include_directories(fc4_game_container_test PRIVATE "${FC4_GZCOM_DIR}/include" plugin)
    target_compile_definitions(fc4_game_container_test PRIVATE _CRT_SECURE_NO_WARNINGS NOMINMAX)
    target_compile_options(fc4_game_container_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    add_test(NAME game_container_test COMMAND fc4_game_container_test)

    # The game's STLport map (plugin/FC4GameMap.h): the read-only walk, the
    # building profile capacity maps and the GetLocalPopulationSummary fill,
    # against fake maps built as docs/abi/allocators.md describes them.
    add_executable(fc4_game_map_test tests/game_map_test.cpp)
    target_include_directories(fc4_game_map_test PRIVATE plugin)
    target_compile_options(fc4_game_map_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    add_test(NAME game_map_test COMMAND fc4_game_map_test)

    # The game's STLport set (plugin/FC4GameSet.h): PlaceZone's outLots unique
    # insert against sets built and checked as docs/abi/allocators.md describes.
    add_executable(fc4_game_set_test tests/game_set_test.cpp)
    target_include_directories(fc4_game_set_test PRIVATE plugin)
    target_compile_options(fc4_game_set_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    add_test(NAME game_set_test COMMAND fc4_game_set_test)

    # The master lot list through the safe lot iterator
    # (plugin/FC4SafeLotIterator.h, budget.md §5.2 BQ3) against fake lists
    # built as aura.md §6.5 and docs/abi/allocators.md describe them.
    add_executable(fc4_safe_lot_iterator_test tests/safe_lot_iterator_test.cpp)
    target_include_directories(fc4_safe_lot_iterator_test PRIVATE plugin)
    target_compile_options(fc4_safe_lot_iterator_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    add_test(NAME safe_lot_iterator_test COMMAND fc4_safe_lot_iterator_test)
    # LR12 master-list observation accounting (T58-T65).
    add_executable(fc4_lot_record_observation_test conformance/tests/lot_record_observation_test.cpp)
    target_include_directories(fc4_lot_record_observation_test PRIVATE plugin)
    target_compile_options(fc4_lot_record_observation_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    add_test(NAME lot_record_observation_test COMMAND fc4_lot_record_observation_test)

    # foundations §16: the slot layout of every game interface we implement.
    add_executable(fc4_vtable_layout_test tests/vtable_layout_test.cpp)
    target_include_directories(fc4_vtable_layout_test PRIVATE "${FC4_GZCOM_DIR}/include" plugin tests)
    target_compile_definitions(fc4_vtable_layout_test PRIVATE _CRT_SECURE_NO_WARNINGS NOMINMAX)
    target_compile_options(fc4_vtable_layout_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    add_test(NAME vtable_layout_test COMMAND fc4_vtable_layout_test)

    # The watchdog's stall arithmetic and the autotest verdict rules.
    add_executable(fc4_diag_logic_test tests/diag_logic_test.cpp)
    target_include_directories(fc4_diag_logic_test PRIVATE plugin)
    target_compile_options(fc4_diag_logic_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    add_test(NAME diag_logic_test COMMAND fc4_diag_logic_test)

    # The x87 precision re-assert (foundations §18.2): the game helper's mode
    # arithmetic and how to report a re-assert.
    add_executable(fc4_fpu_logic_test tests/fpu_logic_test.cpp)
    target_include_directories(fc4_fpu_logic_test PRIVATE plugin)
    target_compile_options(fc4_fpu_logic_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    add_test(NAME fpu_logic_test COMMAND fc4_fpu_logic_test)

    # The network cost pricing of traffic-simulator.md §4.9.1 (TG23): the
    # count x tile cost rounding (nearest, ties to even), the two-type sum and
    # the out-of-range quirks.
    add_executable(fc4_traffic_cost_logic_test tests/traffic_cost_logic_test.cpp)
    target_include_directories(fc4_traffic_cost_logic_test PRIVATE plugin)
    target_compile_options(fc4_traffic_cost_logic_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    add_test(NAME traffic_cost_logic_test COMMAND fc4_traffic_cost_logic_test)

    # The growth probe (building-development.md §12): the counting hook on a
    # fake vtable in this process (forwarding, count, result, callee-pop,
    # registers and x87 state, restore) and its day and summary lines.
    add_executable(fc4_growth_probe_test tests/growth_probe_test.cpp plugin/FC4GrowthProbeHook.cpp)
    target_include_directories(fc4_growth_probe_test PRIVATE plugin)
    target_compile_definitions(fc4_growth_probe_test PRIVATE _CRT_SECURE_NO_WARNINGS NOMINMAX)
    target_compile_options(fc4_growth_probe_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    add_test(NAME growth_probe_test COMMAND fc4_growth_probe_test)

    # Every interface we implement or call against docs/abi/interfaces.tsv
    # (callee-pop bytes, return channel, slot count), read at run time.
    set(FC4_ABI_TSV "${CMAKE_CURRENT_SOURCE_DIR}/../docs/abi/interfaces.tsv")
    # tests/abi_entries.inc is a BUILD PRODUCT: tests/gen_abi_entries.py writes
    # it from the plugin's and the SDK's interface declarations. It is not
    # checked in (gitignored) and is regenerated on every build, so a stale copy
    # can never be compiled into the conformance test. A generation failure is a
    # build failure (the generator exits non-zero before writing), never a
    # silent compile of whatever was lying around.
    set(FC4_ABI_ENTRIES "${CMAKE_CURRENT_SOURCE_DIR}/tests/abi_entries.inc")
    # Needed here as well as below: the conformance test needs the generator.
    find_package(Python3 COMPONENTS Interpreter QUIET)
    if(Python3_Interpreter_FOUND)
        if(FC4_COMPILER_CACHE AND FC4_SCCACHE_EXECUTABLE)
            add_test(NAME compiler_cache_dependency_regression
                COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/../.claude/lane-bin/cache_probe.py")
            set_tests_properties(compiler_cache_dependency_regression PROPERTIES TIMEOUT 180)
        endif()
        # ALL, and the command is the target's own action: it runs on every
        # build rather than only when a dependency is newer than the output.
        add_custom_target(fc4_abi_entries ALL
            COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/gen_abi_entries.py"
            WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
            BYPRODUCTS "${FC4_ABI_ENTRIES}"
            COMMENT "gen_abi_entries.py -> tests/abi_entries.inc"
            VERBATIM)
        add_executable(fc4_abi_conformance_test tests/abi_conformance_test.cpp)
        target_include_directories(fc4_abi_conformance_test PRIVATE "${FC4_GZCOM_DIR}/include" plugin tests)
        target_compile_definitions(fc4_abi_conformance_test PRIVATE _CRT_SECURE_NO_WARNINGS NOMINMAX
            FC4_ABI_TSV="${FC4_ABI_TSV}" FC4_PLUGIN_DIR="${CMAKE_CURRENT_SOURCE_DIR}/plugin")
        target_compile_options(fc4_abi_conformance_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
        # Compiles only after the generator has run.
        add_dependencies(fc4_abi_conformance_test fc4_abi_entries)
        add_test(NAME abi_conformance_test COMMAND fc4_abi_conformance_test
            --plugin-dir "${CMAKE_CURRENT_SOURCE_DIR}/plugin" "${FC4_ABI_TSV}")
    else()
        # Same pattern as ret_channel_check below: with no Python 3 there is no
        # way to generate the listing, so the test is skipped rather than
        # compiled against a stale or missing abi_entries.inc.
        message(STATUS "Python 3 not found: skipping abi_conformance_test "
            "(tests/abi_entries.inc is generated and cannot be built without it)")
    endif()

    # Every use the plugin makes of a game method's return value, against the
    # slot's `ret` channel in docs/abi and what the specs say it returns
    # (tests/check_ret_channels.py). Skipped when no Python 3 is found.
    if(Python3_Interpreter_FOUND)
        add_test(NAME ret_channel_check
            COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/tests/check_ret_channels.py")
    endif()

    # Loads the built FreeCity4.dll like a GZCOM host and checks the diagnostics log.
    add_executable(fc4_plugin_load_test tests/plugin_load_test.cpp plugin/cFC4Variant.cpp
        "${FC4_GZCOM_DIR}/src/cRZBaseString.cpp"
        "${FC4_GZCOM_DIR}/src/cRZBaseVariant.cpp" "${FC4_GZCOM_DIR}/src/cSCBaseProperty.cpp")
    target_include_directories(fc4_plugin_load_test PRIVATE "${FC4_GZCOM_DIR}/include" plugin capi/include)
    target_compile_options(fc4_plugin_load_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    add_dependencies(fc4_plugin_load_test FreeCity4)
    if(FC4_CHECKED)
        set(FC4_LOAD_TEST_MODE --checked)
    else()
        set(FC4_LOAD_TEST_MODE --normal)
    endif()
    add_test(NAME plugin_load_test COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" ${FC4_LOAD_TEST_MODE})
    add_test(NAME plugin_disable_test COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --disabled)
    # FreeCity4.ini `disable =` wins over FC4_DISABLE.
    add_test(NAME plugin_ini_disable_test COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-disabled)
    # Default-off subsystems (land value): `enable =` turns them on, `disable =` still wins.
    add_test(NAME plugin_ini_enable_test COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-enable)
    add_test(NAME plugin_ini_enable_disabled_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-enable-disabled)
    add_test(NAME plugin_ini_enable_weather_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-enable-weather)
    add_test(NAME plugin_ini_enable_ordinance_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-enable-ordinance)
    add_test(NAME plugin_ini_enable_demand_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-enable-demand)
    add_test(NAME plugin_ini_enable_plumbing_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-enable-plumbing)
    add_test(NAME plugin_ini_enable_pollution_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-enable-pollution)
    add_test(NAME plugin_ini_enable_crime_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-enable-crime)
    # Autotest: child processes that end with the verdict and TerminateProcess.
    add_test(NAME plugin_autotest_test COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --autotest)
    # All of them write FreeCity4.log next to the DLL.
    set_tests_properties(plugin_load_test plugin_disable_test plugin_ini_disable_test plugin_ini_enable_test
        plugin_ini_enable_disabled_test plugin_ini_enable_weather_test plugin_ini_enable_ordinance_test
        plugin_ini_enable_demand_test plugin_ini_enable_plumbing_test plugin_autotest_test
        plugin_ini_enable_pollution_test plugin_ini_enable_crime_test
        PROPERTIES RUN_SERIAL TRUE)
    # popup_dialogs (FC4Popups.h): child processes, one FreeCity4.ini each.
    add_test(NAME plugin_ini_popups_test COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-popups)
    set_tests_properties(plugin_ini_popups_test PROPERTIES RUN_SERIAL TRUE)
    # Protection (protection.md): `enable = protection` (no city: neutral
    # getters, lifecycle, status lines) and the grid shadow on fake originals.
    add_test(NAME plugin_ini_enable_protection_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-enable-protection)
    add_test(NAME plugin_ini_protection_shadow_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-protection-shadow)
    set_tests_properties(plugin_ini_enable_protection_test plugin_ini_protection_shadow_test
        PROPERTIES RUN_SERIAL TRUE)
    # Aura (aura.md): `enable = aura` (no city: neutral getters, lifecycle,
    # status lines) and the aura shadow following a fake original step by step
    # (seam ops 14 and 35).
    add_test(NAME plugin_ini_enable_aura_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-enable-aura)
    set_tests_properties(plugin_ini_enable_aura_test PROPERTIES RUN_SERIAL TRUE)
    # Flora (flora.md): `enable = flora` (no city: the registry's key-0 record,
    # neutral calls, lifecycle, status lines) and the census shadow on a fake
    # original (seam op 17).
    add_test(NAME plugin_ini_enable_flora_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-enable-flora)
    set_tests_properties(plugin_ini_enable_flora_test PROPERTIES RUN_SERIAL TRUE)
    # Landfill (landfill.md): `enable = landfill` (no city: neutral getters,
    # regions, lifecycle, the record, threads, status lines) and the landfill
    # shadow on fake originals (seam op 18).
    add_test(NAME plugin_ini_enable_landfill_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-enable-landfill)
    set_tests_properties(plugin_ini_enable_landfill_test PROPERTIES RUN_SERIAL TRUE)
    # The land value and flammability grid shadows on fake originals (seam
    # ops 23, 24), and both requested but inactive with our simulators on.
    add_test(NAME plugin_ini_landvalue_shadow_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-landvalue-shadow)
    add_test(NAME plugin_ini_flammability_shadow_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-flammability-shadow)
    add_test(NAME plugin_ini_lvfl_shadow_inactive_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-lvfl-shadow-inactive)
    set_tests_properties(plugin_ini_landvalue_shadow_test plugin_ini_flammability_shadow_test
        plugin_ini_lvfl_shadow_inactive_test PROPERTIES RUN_SERIAL TRUE)
    # Residential (residential.md, residential-education-health.md): `enable =
    # residential` (no city: neutral getters, AL returns, lifecycle, the record,
    # status lines) and the residential shadow on fake originals (seam op 25).
    add_test(NAME plugin_ini_enable_residential_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-enable-residential)
    set_tests_properties(plugin_ini_enable_residential_test PROPERTIES RUN_SERIAL TRUE)
    # Development (building-development.md, growth-developer.md): `enable =
    # development` (no city: neutral slots, AL returns, lifecycle, the record,
    # threads, status lines).
    add_test(NAME plugin_ini_enable_development_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-enable-development)
    set_tests_properties(plugin_ini_enable_development_test PROPERTIES RUN_SERIAL TRUE)
    # The development shadow (FC4DevelopmentShadow.h): ON with the original
    # unit, its list comparison and record bridge (seam op 34).
    add_test(NAME plugin_ini_development_shadow_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-development-shadow)
    set_tests_properties(plugin_ini_development_shadow_test PROPERTIES RUN_SERIAL TRUE)
    # Zones (zones.md): `enable = zones` (no city: the QI table, neutral getters,
    # the slots' return channels, lifecycle, the record round trip, status
    # lines) and, through seam op 38 with stub hosts, PlaceZone (reject, abort,
    # accept, outLots), ZoningChange, the dezone handler and the zones shadow
    # (zones_shadow = 1: INACTIVE with `enable = zones`; runs on a fake original).
    add_test(NAME plugin_ini_enable_zones_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-enable-zones)
    set_tests_properties(plugin_ini_enable_zones_test PROPERTIES RUN_SERIAL TRUE)
    # The zone developer / lot manager (zones.md §1-§11): `enable = zonedev`
    # (no city: the QI table and sub-object offsets, neutral getters, the
    # record round trip, status lines) and, through seam op 53 with stub hosts
    # and malloc'd B / C blocks, Init / Shutdown, STAGE A's refused lot
    # creation, stub lots in the lists (slots 7, 10), the safe lot iterator's
    # raw layout (slot 15), DeleteLot and the zonedev shadow on a fake original
    # (zonedev_shadow = 1: INACTIVE with `enable = zonedev`).
    add_test(NAME plugin_ini_enable_zonedev_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-enable-zonedev)
    set_tests_properties(plugin_ini_enable_zonedev_test PROPERTIES RUN_SERIAL TRUE)
    # The tract developer (tract-developer.md): `enable = tract` (no city: the
    # QI table, the slots' return channels, lifecycle, the record round trip,
    # status lines) and, through seam op 45 with stub hosts, Init / Shutdown
    # with the filter registered and unregistered, Grow on a stub tract, and
    # the tract shadow (tract_shadow = 1: INACTIVE with `enable = tract`; runs
    # on a fake original).
    add_test(NAME plugin_ini_enable_tract_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-enable-tract)
    set_tests_properties(plugin_ini_enable_tract_test PROPERTIES RUN_SERIAL TRUE)
    # The lot developer (lot-developer.md): `enable = lotdev` (no city: the QI
    # table, Release's 0, the record round trip, lifecycle without services)
    # and, through seam op 52 with stub hosts, Init's registrations and the
    # lot developer shadow (lotdev_shadow = 1: INACTIVE with `enable = lotdev`).
    add_test(NAME plugin_ini_enable_lotdev_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-enable-lotdev)
    set_tests_properties(plugin_ini_enable_lotdev_test PROPERTIES RUN_SERIAL TRUE)
    # The cmglue lane (tests/plugin_cmglue_test.inc): `enable = commercial,
    # industrial` (no city: the QI tables, slot 5, the records' round trips,
    # Init without services, DoMessage).
    add_test(NAME plugin_ini_enable_commercial_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-enable-commercial)
    set_tests_properties(plugin_ini_enable_commercial_test PROPERTIES RUN_SERIAL TRUE)
    # `enable = civic` (no city: the QI table, slots 5-8, the record with the
    # mod-193 order, DoMessage).
    add_test(NAME plugin_ini_enable_civic_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-enable-civic)
    set_tests_properties(plugin_ini_enable_civic_test PROPERTIES RUN_SERIAL TRUE)
    # `enable = parks`: the park manager (parks.md): QI table, record, Init
    # without a city, a fake zoo's escape (the ZooEscape message, §4.5).
    add_test(NAME plugin_ini_enable_parks_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-enable-parks)
    set_tests_properties(plugin_ini_enable_parks_test PROPERTIES RUN_SERIAL TRUE)
    # The glue12b1 lane (tests/plugin_propcity_test.inc): `enable = propdev` (the
    # QI tables of the developer and its wealth requester, the record round
    # trip, version 4 written back as 5, Init / Shutdown / the scan flag
    # without a city) and `enable = citydetail` (QI table, record, Init).
    add_test(NAME plugin_ini_enable_propdev_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-enable-propdev)
    set_tests_properties(plugin_ini_enable_propdev_test PROPERTIES RUN_SERIAL TRUE)
    add_test(NAME plugin_ini_enable_citydetail_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-enable-citydetail)
    set_tests_properties(plugin_ini_enable_citydetail_test PROPERTIES RUN_SERIAL TRUE)
    # `enable = ports`: the airport and seaport developers (port-developers.md): the QI
    # tables (1 gives +4), the slots with no ports, the empty records, Init without a city.
    add_test(NAME plugin_ini_enable_ports_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-enable-ports)
    set_tests_properties(plugin_ini_enable_ports_test PROPERTIES RUN_SERIAL TRUE)    # `terrain_shadow = 1`: the terrain shadow (glue16) on a stand-in original
    # built inside the DLL (seam op 58): exact, lost messages, perturbations.
    add_test(NAME plugin_ini_terrain_shadow_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-terrain-shadow)
    set_tests_properties(plugin_ini_terrain_shadow_test PROPERTIES RUN_SERIAL TRUE)
    # `enable = neighbors`: the neighbours simulator, its connections (an
    # original's accepted on load) and deals, through a fake city and region.
    add_test(NAME plugin_ini_enable_neighbors_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-enable-neighbors)
    set_tests_properties(plugin_ini_enable_neighbors_test PROPERTIES RUN_SERIAL TRUE)
    # Traffic (traffic-simulator.md, traffic-network.md, pathfinder.md): `enable =
    # traffic` (no city: the QI table, the x87 slots, the record round trip,
    # status lines) and, through seam op 42 with a stub host, the slot-6
    # pathfinders (FindPath, the compressed route, the caller's route deque).
    add_test(NAME plugin_ini_enable_traffic_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-enable-traffic)
    set_tests_properties(plugin_ini_enable_traffic_test PROPERTIES RUN_SERIAL TRUE)
    # The history warehouse (docs/specs/history.md): off by default; `enable =
    # history` registers it (QueryInterface quirk, slots, record, the record
    # message reaching the core through the test hooks of op 54).
    add_test(NAME plugin_ini_enable_history_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-enable-history)
    set_tests_properties(plugin_ini_enable_history_test PROPERTIES RUN_SERIAL TRUE)
    # The history shadow (FC4HistoryShadow.h): inactive with ours; through
    # seam op 54 op 1, on FreeCity4's warehouse as the "original": the load
    # round trip, messages, the pass steps, the month comparison exact, then a
    # record the shadow did not see found.
    add_test(NAME plugin_ini_history_shadow_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-history-shadow)
    set_tests_properties(plugin_ini_history_shadow_test PROPERTIES RUN_SERIAL TRUE)
    # The 24-hour clock (docs/specs/clock24.md): off by default; `enable =
    # clock24` registers it (the QI table with the +0xC system service, the
    # game-form slots, the framework and cheat registration in order, the
    # step, the cheats, the 26-byte record and versions 1 / 3, threads).
    add_test(NAME plugin_ini_enable_clock24_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-enable-clock24)
    set_tests_properties(plugin_ini_enable_clock24_test PROPERTIES RUN_SERIAL TRUE)
    # The tutorial system (docs/specs/tutorial.md; the glue15 lane): `enable =
    # tutorial` registers it (QI table, the 11-byte record and its GUID
    # mapping, IsTutorial and the getters, Init / Shutdown, the last Release).
    add_test(NAME plugin_ini_enable_tutorial_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-enable-tutorial)
    set_tests_properties(plugin_ini_enable_tutorial_test PROPERTIES RUN_SERIAL TRUE)
    # The 24-hour clock shadow (FC4Clock24Shadow.h): inactive with ours;
    # through seam op 54 op 2, on FreeCity4's clock as the "original": the
    # load, mirrored steps and messages exact, then a magnification and a
    # missing message found.
    add_test(NAME plugin_ini_clock24_shadow_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-clock24-shadow)
    set_tests_properties(plugin_ini_clock24_shadow_test PROPERTIES RUN_SERIAL TRUE)
    # References read from a save (in-game run 88): land value grids and a
    # foreign ordinance, with and without a save before Shutdown, and land value
    # grids created through GZCOM; no reference warning, each released once.
    add_test(NAME plugin_saved_refs_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --saved-refs)
    set_tests_properties(plugin_saved_refs_test PROPERTIES RUN_SERIAL TRUE)
    # The growth calibration probe (building-development.md §12, seam op 39):
    # `growth_probe = 1` on a fake tract developer vtable and a fake city (day
    # lines, cut, summary, restore), and off by default.
    add_test(NAME plugin_ini_growth_probe_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-growth-probe)
    set_tests_properties(plugin_ini_growth_probe_test PROPERTIES RUN_SERIAL TRUE)
    # The harness lane (tooling): the pure logic (dates, rates, the script
    # parser and scheduler), and sim_turbo on our simulator (FC4Turbo.h): off by
    # default, the WARNING with `disable = simulator`, bad values, until a date.
    add_executable(fc4_harness_logic_test tests/harness_logic_test.cpp)
    target_include_directories(fc4_harness_logic_test PRIVATE plugin)
    target_compile_options(fc4_harness_logic_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    add_test(NAME harness_logic_test COMMAND fc4_harness_logic_test)
    add_test(NAME plugin_ini_turbo_test COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-turbo)
    set_tests_properties(plugin_ini_turbo_test PROPERTIES RUN_SERIAL TRUE)
    # script = <path> (FC4Script.h): dated actions through a stub zone manager
    # (the region layout, the arguments, the bits it clears) and our simulator;
    # a refused script and an unreadable one (test seam op 46).
    add_test(NAME plugin_ini_script_test COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-script)
    set_tests_properties(plugin_ini_script_test PROPERTIES RUN_SERIAL TRUE)
    # close_dialogs (FC4Dialogs.h): a fake window manager and legacy message
    # server (test seam op 47): the button, the EndModal fallback, a stale
    # window, nested dialogs, SC4GameDelay, a late subscription; off: nothing.
    add_test(NAME plugin_ini_dialogs_test COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-dialogs)
    set_tests_properties(plugin_ini_dialogs_test PROPERTIES RUN_SERIAL TRUE)
    # The advisor family's game-layout map mirrors (FC4AdvisorMaps.h,
    # advisors.md §12.2; the glue-advisors-impl4 lane): signed / pointer order,
    # references held while listed, a duplicate refused, Clear.
    add_executable(fc4_advisor_maps_test tests/advisor_maps_test.cpp)
    target_include_directories(fc4_advisor_maps_test PRIVATE "${FC4_GZCOM_DIR}/include" plugin)
    target_compile_options(fc4_advisor_maps_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    add_test(NAME advisor_maps_test COMMAND fc4_advisor_maps_test)
    # The layout-compatible grid family (plugin/FC4SimGridLogic.h, simgrid.md
    # §3-§6): slots, averages, record, the vector<vector<T>> layout and allocator.
    add_executable(fc4_simgrid_test tests/simgrid_test.cpp)
    target_include_directories(fc4_simgrid_test PRIVATE "${FC4_GZCOM_DIR}/include" plugin)
    target_compile_definitions(fc4_simgrid_test PRIVATE _CRT_SECURE_NO_WARNINGS NOMINMAX)
    target_compile_options(fc4_simgrid_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    add_test(NAME simgrid_test COMMAND fc4_simgrid_test)
    # The advisor window slots 23-28 (FC4AdvisorWindow.h, advisors.md §12.1.1; glue-advisors-final4).
    add_executable(fc4_advisor_window_test tests/advisor_window_test.cpp)
    target_include_directories(fc4_advisor_window_test PRIVATE plugin)
    target_compile_options(fc4_advisor_window_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    add_test(NAME advisor_window_test COMMAND fc4_advisor_window_test)
    # The advisor system's register_api vector, DoLink cuts, slot 9 shape, cheat payload and reload
    # directories (FC4AdvisorApi.h, advisors.md §17 cases 23-27; glue-advisors-activate4).
    add_executable(fc4_advisor_api_test tests/advisor_api_test.cpp)
    target_include_directories(fc4_advisor_api_test PRIVATE plugin)
    target_compile_options(fc4_advisor_api_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    add_test(NAME advisor_api_test COMMAND fc4_advisor_api_test)
    # Effects opt-in must enumerate/create the same city-manager class whether
    # or not its shadow is enabled.
    foreach(mode enabled disabled default guarded)
        add_test(NAME plugin_effects_registration_${mode}_test
            COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --effects-${mode})
        set_tests_properties(plugin_effects_registration_${mode}_test PROPERTIES RUN_SERIAL TRUE TIMEOUT 60)
    endforeach()
else()
    message(STATUS "FreeCity4.dll is only built with MSVC for 32-bit x86; skipping the plugin target.")
endif()

# The shadow must not dereference untyped payloads of unrelated messages.
add_executable(mysims_message_payload_test tests/mysims_message_payload_test.cpp)
add_test(NAME mysims_message_payload_test COMMAND mysims_message_payload_test)

# PD12 native prop boundary vectors; provider grid policy is emulated by the test.
if(MSVC AND CMAKE_SIZEOF_VOID_P EQUAL 4)
    add_executable(fc4_port_props_test tests/port_props_test.cpp)
    target_include_directories(fc4_port_props_test PRIVATE "${FC4_GZCOM_DIR}/include" plugin)
    target_compile_definitions(fc4_port_props_test PRIVATE _CRT_SECURE_NO_WARNINGS NOMINMAX)
    target_compile_options(fc4_port_props_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    add_test(NAME port_props_test COMMAND fc4_port_props_test)
endif()

# Independent public-SDK packed DBPF transport comparison.
if(TARGET FreeCity4)
    target_sources(FreeCity4 PRIVATE plugin/FC4DbpfNative.cpp)
endif()

# Record-only original advisor shadow (default off; no factory replacement).
if(TARGET FreeCity4)
    target_sources(FreeCity4 PRIVATE plugin/FC4AdvisorsShadow.cpp plugin/FC4AdvisorsShadow.h)
    add_executable(fc4_advisors_shadow_test tests/advisors_shadow_test.cpp)
    target_include_directories(fc4_advisors_shadow_test PRIVATE "${FC4_GZCOM_DIR}/include" plugin capi/include)
    target_compile_definitions(fc4_advisors_shadow_test PRIVATE _CRT_SECURE_NO_WARNINGS NOMINMAX)
    target_compile_options(fc4_advisors_shadow_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    add_test(NAME advisors_shadow_test COMMAND fc4_advisors_shadow_test "$<TARGET_FILE:FreeCity4>")
endif()

# Read-only overlay contracts and OV12 native observation diagnostics.
if(TARGET FreeCity4)
    target_sources(FreeCity4 PRIVATE plugin/FC4OverlaysShadow.cpp plugin/FC4OverlaysShadow.h)
    target_sources(FreeCity4 PRIVATE plugin/FC4OverlaysObservation.cpp plugin/FC4OverlaysObservation.h)
endif()
# PL23: raw evidence and the narrower defined-field shadow comparison.
add_executable(fc4_prop_lot_compare_test tests/prop_lot_compare_test.cpp)
target_include_directories(fc4_prop_lot_compare_test PRIVATE plugin capi/include)
add_test(NAME prop_lot_compare_test COMMAND fc4_prop_lot_compare_test)
# Cached MSVC /showIncludes output can retain another worktree's header paths.
# A shared capture layout change must rebuild local users even with such cached
# dependencies (otherwise static captures can crash in their destructors).
if(TARGET FreeCity4)
    get_target_property(fc4_capture_sources FreeCity4 SOURCES)
    foreach(fc4_capture_source IN LISTS fc4_capture_sources)
        if(fc4_capture_source MATCHES "^plugin/.*\\.cpp$")
            set_property(SOURCE "${fc4_capture_source}" APPEND PROPERTY OBJECT_DEPENDS
                "${CMAKE_CURRENT_SOURCE_DIR}/plugin/FC4WriteCapture.h")
        endif()
    endforeach()
endif()

# Opt-in record-only Lua collector/registration shadow (LH6-LH7 bound scope).
if(TARGET FreeCity4)
    target_sources(FreeCity4 PRIVATE plugin/FC4LuaShadow.cpp plugin/FC4LuaShadow.h)
endif()

# Opt-in record-only weather replay; never replaces the native simulator.
if(TARGET FreeCity4)
    target_sources(FreeCity4 PRIVATE plugin/FC4WeatherShadow.cpp plugin/FC4WeatherShadow.h)
endif()

# OV12 observer safeguards run against the same native adapter as the plugin.
if(TARGET FreeCity4)
    add_executable(fc4_overlays_observation_test tests/overlays_observation_test.cpp)
    target_include_directories(fc4_overlays_observation_test PRIVATE "${FC4_GZCOM_DIR}/include" plugin capi/include)
    target_compile_definitions(fc4_overlays_observation_test PRIVATE _CRT_SECURE_NO_WARNINGS NOMINMAX)
    target_compile_options(fc4_overlays_observation_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    add_test(NAME overlays_observation_test COMMAND fc4_overlays_observation_test)
endif()

# Automata owners: captured IDs must retain the correct live interface.
if(TARGET FreeCity4)
    add_executable(fc4_automata_capture_test tests/automata_capture_test.cpp)
    target_include_directories(fc4_automata_capture_test PRIVATE "${FC4_GZCOM_DIR}/include" plugin capi/include)
    target_compile_definitions(fc4_automata_capture_test PRIVATE _CRT_SECURE_NO_WARNINGS NOMINMAX)
    target_compile_options(fc4_automata_capture_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    add_test(NAME automata_capture_test COMMAND fc4_automata_capture_test "$<TARGET_FILE:FreeCity4>")
endif()

# PV14 genuine selector capture preserves the original ABI and return value.
if(TARGET FreeCity4)
    add_executable(fc4_propdev_transcript_test tests/propdev_transcript_test.cpp)
    target_include_directories(fc4_propdev_transcript_test PRIVATE plugin capi/include)
    target_compile_definitions(fc4_propdev_transcript_test PRIVATE _CRT_SECURE_NO_WARNINGS NOMINMAX WIN32_LEAN_AND_MEAN)
    target_compile_options(fc4_propdev_transcript_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    add_test(NAME propdev_transcript_test COMMAND fc4_propdev_transcript_test)
endif()

# Frozen block inputs preserve each lot's lists once, including empty configurations.
if(TARGET FreeCity4)
    add_executable(fc4_propdev_snapshot_test tests/propdev_snapshot_test.cpp)
    target_include_directories(fc4_propdev_snapshot_test PRIVATE plugin capi/include)
    target_compile_definitions(fc4_propdev_snapshot_test PRIVATE _CRT_SECURE_NO_WARNINGS NOMINMAX)
    target_compile_options(fc4_propdev_snapshot_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    add_test(NAME propdev_snapshot_test COMMAND fc4_propdev_snapshot_test)
endif()
# LV6 published progress: record-only interval decoding, independent of native
# installation (LV8). Never substitutes sampled inputs or advances simulation.
add_executable(fc4_landvalue_progress_test tests/landvalue_progress_test.cpp)
target_include_directories(fc4_landvalue_progress_test PRIVATE plugin)
add_test(NAME landvalue_progress_test COMMAND fc4_landvalue_progress_test)

# LV8 host admission/lifetime acceptance fixtures; no native interceptor implied.
add_executable(fc4_landvalue_capture_contract_test conformance/tests/landvalue_capture_contract_test.cpp)
add_test(NAME landvalue_capture_contract_test COMMAND fc4_landvalue_capture_contract_test)

# Process hash state survives app shutdown; only nodes and buckets are freed.
add_executable(fc4_process_hash_state_test tests/process_hash_state_test.cpp)
target_include_directories(fc4_process_hash_state_test PRIVATE plugin)
add_test(NAME process_hash_state_test COMMAND fc4_process_hash_state_test)
# GZ11: opt-in read-only bounded queue-manager lookup diagnostic.
if(TARGET FreeCity4)
    target_sources(FreeCity4 PRIVATE plugin/FC4GZMsgShadow.cpp plugin/FC4GZMsgShadow.h)
endif()
if(TARGET FreeCity4)
    target_sources(FreeCity4 PRIVATE plugin/FC4MySimDispatchShadow.cpp plugin/FC4MySimDispatchShadow.h)
endif()

# Read-only render-properties registry differential; opt-in only.
if(TARGET FreeCity4)
    target_sources(FreeCity4 PRIVATE plugin/FC4RenderPropsShadow.cpp plugin/FC4RenderPropsShadow.h)
endif()

# LT14: default-off read-only lighting parameter getter comparison.
if(TARGET FreeCity4)
    target_sources(FreeCity4 PRIVATE plugin/FC4LightingShadow.cpp plugin/FC4LightingShadow.h)
endif()

# LR13 T66-T73: actual DLL construction capture and literal-IID replay.
if(TARGET FreeCity4)
    add_executable(fc4_construction_capture_test tests/construction_capture_test.cpp)
    target_include_directories(fc4_construction_capture_test PRIVATE "${FC4_GZCOM_DIR}/include" plugin capi/include)
    target_compile_definitions(fc4_construction_capture_test PRIVATE _CRT_SECURE_NO_WARNINGS NOMINMAX)
    target_compile_options(fc4_construction_capture_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    add_test(NAME construction_capture_test COMMAND fc4_construction_capture_test "$<TARGET_FILE:FreeCity4>")
endif()

# Isolated original-owner render-properties fixture; opt-in shadow only.
if(TARGET FreeCity4)
    target_sources(FreeCity4 PRIVATE plugin/FC4RenderPropsFixture.cpp plugin/FC4RenderPropsFixture.h)
endif()

if(TARGET FreeCity4)
    target_sources(FreeCity4 PRIVATE plugin/FC4CityShadow.cpp plugin/FC4CityShadow.h)
endif()
# DBPF shadow: every rolling year covers all deterministically sorted files.
add_executable(fc4_dbpf_rotation_test tests/dbpf_rotation_test.cpp)
target_include_directories(fc4_dbpf_rotation_test PRIVATE plugin)
add_test(NAME dbpf_rotation_test COMMAND fc4_dbpf_rotation_test)

# Read-only application paths/lifecycle shadow, default off (AP17 scope).
if(TARGET FreeCity4)
    target_sources(FreeCity4 PRIVATE plugin/FC4AppShadow.cpp plugin/FC4AppShadow.h)
endif()

# Default-off sampled DX7 cached-state transition observer.
if(TARGET FreeCity4)
    target_sources(FreeCity4 PRIVATE plugin/FC4GDriverShadow.cpp plugin/FC4GDriverShadow.h)
endif()

# Default-off legacy owner-method differential; no full build replay claim.
if(TARGET FreeCity4)
    target_sources(FreeCity4 PRIVATE plugin/FC4LegacyCtlShadow.cpp plugin/FC4LegacyCtlShadow.h)
endif()

# VU16: read-only partial view utility shadow; no speculative native signatures.
if(TARGET FreeCity4)
    target_sources(FreeCity4 PRIVATE plugin/FC4ViewUtilShadow.cpp plugin/FC4ViewUtilShadow.h)
endif()

# MI14 finite native model-instance fixture, opt-in only.
if(TARGET FreeCity4)
    target_sources(FreeCity4 PRIVATE plugin/FC4ModelInstShadow.cpp plugin/FC4ModelInstShadow.h)
endif()

# MySim dispatch public ABI: constructor-only checks, no concrete trip calls.
add_executable(fc4_mysim_dispatch_contract_test tests/mysim_dispatch_contract_test.cpp)
target_include_directories(fc4_mysim_dispatch_contract_test PRIVATE plugin
    third_party/gzcom-dll/gzcom-dll/include)
add_test(NAME mysim_dispatch_contract_test COMMAND fc4_mysim_dispatch_contract_test)
if(TARGET fc4_plugin_load_test)
    add_test(NAME plugin_mysims_factory_boundary_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-enable-mysims)
    add_test(NAME plugin_mysims_default_off_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-default-mysims)
    add_test(NAME plugin_mysims_replace_factory_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-replace-mysims)
    set_tests_properties(plugin_mysims_factory_boundary_test plugin_mysims_default_off_test
        plugin_mysims_replace_factory_test PROPERTIES RUN_SERIAL TRUE)
endif()

# The pending MySim owner group must coexist with a registered replacement.
if(TARGET fc4_plugin_load_test)
    add_test(NAME plugin_mysims_mixed_factory_boundary_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-mixed-mysims)
    set_tests_properties(plugin_mysims_mixed_factory_boundary_test PROPERTIES RUN_SERIAL TRUE)
endif()

# disasters.md §2.8: opt-in layer, preserving concrete native managers.
if(TARGET FreeCity4)
    target_sources(FreeCity4 PRIVATE plugin/cFC4DisasterLayer.cpp plugin/cFC4DisasterLayer.h)
    add_test(NAME plugin_ini_enable_disasters_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-enable-disasters)
    set_tests_properties(plugin_ini_enable_disasters_test PROPERTIES RUN_SERIAL TRUE)
    add_test(NAME plugin_ini_disable_disasters_test
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-disable-disasters)
    set_tests_properties(plugin_ini_disable_disasters_test PROPERTIES RUN_SERIAL TRUE)
endif()

if(TARGET fc4_plugin_load_test)
    add_test(NAME plugin_advisors_registration
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-enable-advisors)
    add_test(NAME plugin_advisors_default_off
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-default-advisors)
endif()

if(TARGET FreeCity4)
    add_executable(fc4_advisor_message_test tests/advisor_message_test.cpp
        plugin/cFC4Message2Standard.cpp "${FC4_GZCOM_DIR}/src/cRZBaseString.cpp")
    target_include_directories(fc4_advisor_message_test PRIVATE "${FC4_GZCOM_DIR}/include" plugin)
    target_compile_definitions(fc4_advisor_message_test PRIVATE _CRT_SECURE_NO_WARNINGS NOMINMAX)
    target_compile_options(fc4_advisor_message_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    add_test(NAME advisor_message_test COMMAND fc4_advisor_message_test)
endif()

# Construction interface identity and borrowed slot-3 conversion (LR 9.3).
if(TARGET FreeCity4)
    target_sources(FreeCity4 PRIVATE plugin/FC4ConstructionIdentity.h)
    add_executable(fc4_construction_identity_test tests/construction_identity_test.cpp)
    target_include_directories(fc4_construction_identity_test PRIVATE "${FC4_GZCOM_DIR}/include" plugin)
    target_compile_options(fc4_construction_identity_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    add_test(NAME construction_identity_test COMMAND fc4_construction_identity_test)
endif()

# Construction record-owner staging factory: no subsystem registration yet.
if(TARGET FreeCity4)
    target_sources(FreeCity4 PRIVATE plugin/cFC4ConstructionRecord.cpp plugin/cFC4ConstructionRecord.h)
    add_test(NAME construction_record_factory_test
        COMMAND fc4_construction_capture_test "$<TARGET_FILE:FreeCity4>" --record-factory)
    set_tests_properties(construction_record_factory_test PROPERTIES RUN_SERIAL TRUE)
endif()

# Overlay probe ABI and lifecycle against the real private Rust transport.
if(TARGET FreeCity4)
    add_executable(fc4_overlays_contract_test tests/overlays_contract_test.cpp)
    target_include_directories(fc4_overlays_contract_test PRIVATE "${FC4_GZCOM_DIR}/include" plugin capi/include)
    target_compile_definitions(fc4_overlays_contract_test PRIVATE _CRT_SECURE_NO_WARNINGS NOMINMAX)
    target_compile_options(fc4_overlays_contract_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    target_link_libraries(fc4_overlays_contract_test PRIVATE fc4_capi)
    add_test(NAME overlays_contract_test COMMAND fc4_overlays_contract_test)
endif()

# FC4 overlay-owner factory: real director registration, default-off and disable
# precedence, lifecycle and simultaneous getter/setter calls through the Rust core.
if(TARGET FreeCity4)
    target_sources(FreeCity4 PRIVATE plugin/FC4OverlayOwner.cpp plugin/FC4OverlayOwner.h)
    add_executable(fc4_overlay_factory_test tests/overlay_factory_test.cpp)
    target_include_directories(fc4_overlay_factory_test PRIVATE "${FC4_GZCOM_DIR}/include" plugin)
    target_compile_definitions(fc4_overlay_factory_test PRIVATE _CRT_SECURE_NO_WARNINGS NOMINMAX)
    target_compile_options(fc4_overlay_factory_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    foreach(mode off on disabled)
        add_test(NAME overlay_factory_${mode}_test
            COMMAND fc4_overlay_factory_test "$<TARGET_FILE:FreeCity4>" ${mode})
    endforeach()
endif()

if(TARGET fc4_plugin_load_test)
    add_test(NAME plugin_netocc_shadow_factory_isolation
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-netocc-shadow-factories)
    set_tests_properties(plugin_netocc_shadow_factory_isolation PROPERTIES RUN_SERIAL TRUE)
    add_test(NAME plugin_netlot_ownership
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --netlot-ownership)
    set_tests_properties(plugin_netlot_ownership PROPERTIES RUN_SERIAL TRUE)
endif()

if(TARGET FreeCity4)
    target_sources(FreeCity4 PRIVATE plugin/cFC4PowerLineOccupant.cpp plugin/cFC4PowerLineOccupant.h
        plugin/cFC4PowerPoleOccupant.cpp plugin/cFC4PowerPoleOccupant.h plugin/FC4OccupantAdapter.h)
endif()
if(TARGET fc4_plugin_load_test)
    add_test(NAME plugin_netocc_line_factory
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-netocc-line-factory)
    set_tests_properties(plugin_netocc_line_factory PROPERTIES RUN_SERIAL TRUE)
    add_test(NAME plugin_netocc_pole_factory
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-netocc-pole-factory)
    set_tests_properties(plugin_netocc_pole_factory PROPERTIES RUN_SERIAL TRUE)
endif()
# Lot-record replacement boundary and specified live getter projection.
if(TARGET fc4_plugin_load_test)
    add_test(NAME plugin_lotrec_registration_boundary
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-lotrec-boundary)
    add_test(NAME plugin_lotrec_default_off
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-lotrec-default)
    add_executable(fc4_lot_record_view_test tests/lot_record_view_test.cpp)
    target_include_directories(fc4_lot_record_view_test PRIVATE plugin)
    target_compile_options(fc4_lot_record_view_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    add_test(NAME lot_record_view_test COMMAND fc4_lot_record_view_test)
endif()

if(TARGET fc4_plugin_load_test)
    foreach(netrules_mode host overlap default disabled veto-manager veto-tool)
        add_test(NAME plugin_netrules_${netrules_mode}
            COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-netrules-${netrules_mode})
        set_tests_properties(plugin_netrules_${netrules_mode} PROPERTIES RUN_SERIAL TRUE)
    endforeach()
endif()

if(TARGET fc4_plugin_load_test)
    add_test(NAME plugin_lotconf_registration_ownership
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --ini-lotconf-ownership)
    set_tests_properties(plugin_lotconf_registration_ownership PROPERTIES RUN_SERIAL TRUE)
endif()

# Opt-in natural-entry reach tracing (no C ABI exports).
if(TARGET FreeCity4)
    target_sources(FreeCity4 PRIVATE plugin/FC4Reach.cpp plugin/FC4Reach.h)
    target_compile_definitions(FreeCity4 PRIVATE
        FC4_REACH_CONTRACTS_FILE="${CMAKE_CURRENT_SOURCE_DIR}/../docs/abi/functions.tsv")
endif()
if(MSVC AND CMAKE_SIZEOF_VOID_P EQUAL 4)
    add_library(fc4_reach_fixture SHARED tests/reach_fixture.cpp)
    # Incremental-link export thunks start with JMP and would correctly be
    # refused by the tracer; controls need the actual fixture entry points.
    target_link_options(fc4_reach_fixture PRIVATE /OPT:NOICF /INCREMENTAL:NO)
    add_executable(fc4_reach_test tests/reach_test.cpp plugin/FC4Reach.cpp)
    target_include_directories(fc4_reach_test PRIVATE plugin)
    target_compile_definitions(fc4_reach_test PRIVATE FC4_REACH_TEST NOMINMAX)
    target_compile_options(fc4_reach_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    add_test(NAME reach_test COMMAND fc4_reach_test "$<TARGET_FILE:fc4_reach_fixture>")
endif()

# S3 opt-in entry-hook observer and architecture-only mechanics tests.
if(TARGET FreeCity4)
    target_sources(FreeCity4 PRIVATE plugin/FC4Inplace.cpp plugin/FC4Inplace.h plugin/FC4InplaceDetour.h)
    add_executable(fc4_inplace_test tests/inplace_test.cpp)
    target_include_directories(fc4_inplace_test PRIVATE plugin)
    target_compile_options(fc4_inplace_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    add_test(NAME inplace_mechanics COMMAND fc4_inplace_test)
endif()

# Registry-only compare bindings; C++ forwarders are generated from published contracts.
if(TARGET FreeCity4)
    find_package(Python3 REQUIRED COMPONENTS Interpreter)
    file(GLOB FC4_COMPARE_ROWS CONFIGURE_DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/plugin/compare/*.tsv")
    file(GLOB FC4_COMPARE_CONTRACTS CONFIGURE_DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/../docs/abi/functions.tsv")
    set(FC4_COMPARE_GENERATED "${CMAKE_CURRENT_BINARY_DIR}/compare/FC4CompareRows.inc")
    add_custom_command(OUTPUT "${FC4_COMPARE_GENERATED}"
        COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/plugin/compare/generate.py"
            --engine "${CMAKE_CURRENT_SOURCE_DIR}" --output "${FC4_COMPARE_GENERATED}"
        DEPENDS ${FC4_COMPARE_ROWS} ${FC4_COMPARE_CONTRACTS} "${CMAKE_CURRENT_SOURCE_DIR}/plugin/compare/generate.py"
        VERBATIM)
    target_sources(FreeCity4 PRIVATE "${FC4_COMPARE_GENERATED}")
    target_include_directories(FreeCity4 PRIVATE "${CMAKE_CURRENT_BINARY_DIR}/compare")
    target_compile_definitions(FreeCity4 PRIVATE FC4_COMPARE_ROOT="${CMAKE_CURRENT_SOURCE_DIR}/../build-runs")
    add_executable(fc4_compare_data_test tests/compare_data_test.cpp)
    target_include_directories(fc4_compare_data_test PRIVATE plugin)
    target_compile_options(fc4_compare_data_test PRIVATE /W3 /permissive- /utf-8 /EHsc /UNDEBUG)
    add_test(NAME compare_value_schema COMMAND fc4_compare_data_test)
    add_executable(fc4_compare_runtime_test tests/compare_runtime_test.cpp "${FC4_COMPARE_GENERATED}")
    target_include_directories(fc4_compare_runtime_test PRIVATE plugin "${CMAKE_CURRENT_BINARY_DIR}/compare")
    target_compile_definitions(fc4_compare_runtime_test PRIVATE NOMINMAX WIN32_LEAN_AND_MEAN
        FC4_COMPARE_ROOT="${CMAKE_CURRENT_BINARY_DIR}/compare-unit")
    target_compile_options(fc4_compare_runtime_test PRIVATE /W3 /permissive- /utf-8 /EHsc /UNDEBUG)
    add_test(NAME compare_dispatch_modes COMMAND fc4_compare_runtime_test)
    set_tests_properties(compare_dispatch_modes PROPERTIES TIMEOUT 20)
endif()

# RegionalCity private C transport: real Rust owner and host-reference callbacks.
if(TARGET FreeCity4)
    target_sources(FreeCity4 PRIVATE plugin/FC4RegionalCityElements.cpp plugin/FC4RegionalCityElements.h)
    add_executable(fc4_regionalcity_owner_test tests/regionalcity_owner_test.cpp)
    target_include_directories(fc4_regionalcity_owner_test PRIVATE plugin)
    target_compile_options(fc4_regionalcity_owner_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    target_link_libraries(fc4_regionalcity_owner_test PRIVATE fc4_capi)
    add_test(NAME regionalcity_owner_test COMMAND fc4_regionalcity_owner_test)
endif()

# RegionalCity native replacement hosts and the border-call contract (RY9).
if(TARGET FreeCity4)
    target_sources(FreeCity4 PRIVATE plugin/cFC4RegionalCity.cpp plugin/cFC4RegionalCity.h
        plugin/FC4RegionalCityConnections.cpp plugin/FC4RegionalCitySetup.cpp
        plugin/FC4RegionalCityHost.h plugin/FC4RegionalCityBorder.h)
    add_executable(fc4_regionalcity_border_test tests/regionalcity_border_test.cpp)
    target_include_directories(fc4_regionalcity_border_test PRIVATE plugin)
    target_compile_options(fc4_regionalcity_border_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    target_link_libraries(fc4_regionalcity_border_test PRIVATE fc4_capi)
    add_test(NAME regionalcity_border_contract COMMAND fc4_regionalcity_border_test)
endif()

if(TARGET fc4_plugin_load_test)
    add_test(NAME plugin_netlot_default_factories
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --netlot-defaults)
    add_test(NAME plugin_netlot_disabled_factories
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --netlot-disabled)
    add_test(NAME plugin_netlot_object_factories
        COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --netlot-object-factories)
    set_tests_properties(plugin_netlot_default_factories plugin_netlot_disabled_factories plugin_netlot_object_factories
        PROPERTIES RUN_SERIAL TRUE)
endif()

# Same-invocation UI draw/message transcripts, shared by window shadows.
if(TARGET FreeCity4)
    target_sources(FreeCity4 PRIVATE plugin/FC4MapViewDrawStream.cpp plugin/FC4MapViewDrawStream.h plugin/FC4MapViewDrawBoundary.h plugin/FC4MapViewUIBoundary.h)
    target_sources(FreeCity4 PRIVATE plugin/FC4DrawStream.h plugin/FC4DrawStreamCapture.cpp plugin/FC4DrawStreamCapture.h)
    add_executable(fc4_draw_stream_test tests/draw_stream_test.cpp plugin/FC4DrawStreamCapture.cpp)
    target_include_directories(fc4_draw_stream_test PRIVATE plugin)
    target_compile_definitions(fc4_draw_stream_test PRIVATE FC4_DRAW_STREAM_TEST)
    target_compile_options(fc4_draw_stream_test PRIVATE /W3 /permissive- /utf-8 /EHsc /UNDEBUG)
    add_test(NAME draw_stream_oracle COMMAND fc4_draw_stream_test)
    target_sources(FreeCity4 PRIVATE plugin/FC4DrawCapture.cpp plugin/FC4DrawCapture.h plugin/FC4DrawCaptureLogic.h)
    add_executable(fc4_draw_capture_test tests/draw_capture_test.cpp)
    target_include_directories(fc4_draw_capture_test PRIVATE plugin)
    target_compile_options(fc4_draw_capture_test PRIVATE /W3 /permissive- /utf-8 /EHsc /UNDEBUG)
    add_test(NAME draw_capture_buffers COMMAND fc4_draw_capture_test)
endif()
if(TARGET FreeCity4)
    add_executable(fc4_gzfw_owner_test tests/gzfw_owner_test.cpp)
    target_sources(fc4_gzfw_owner_test PRIVATE "${FC4_GZCOM_DIR}/src/cRZBaseString.cpp")
    target_include_directories(fc4_gzfw_owner_test PRIVATE plugin "${FC4_GZCOM_DIR}/include")
    target_compile_options(fc4_gzfw_owner_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    add_test(NAME gzfw_native_owners COMMAND fc4_gzfw_owner_test "$<TARGET_FILE:FreeCity4>")
    set_tests_properties(gzfw_native_owners PROPERTIES TIMEOUT 20)
endif()

# Public DX7 slot/arity checks and bounded native transcript forwarding.
if(TARGET FreeCity4)
    add_executable(fc4_gdriver_trace_test tests/gdriver_trace_test.cpp)
    target_include_directories(fc4_gdriver_trace_test PRIVATE plugin)
    target_compile_options(fc4_gdriver_trace_test PRIVATE /W3 /permissive- /utf-8 /EHsc /UNDEBUG)
    add_test(NAME gdriver_trace_contract COMMAND fc4_gdriver_trace_test)
endif()
# PV18 ordinary prop effects: semantic snapshots and genuine-call transcripts.
if(TARGET FreeCity4)
    add_executable(fc4_propdev_effects_test tests/propdev_effects_test.cpp)
    target_include_directories(fc4_propdev_effects_test PRIVATE plugin)
    target_compile_definitions(fc4_propdev_effects_test PRIVATE _CRT_SECURE_NO_WARNINGS NOMINMAX WIN32_LEAN_AND_MEAN)
    target_compile_options(fc4_propdev_effects_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    target_link_libraries(fc4_propdev_effects_test PRIVATE fc4_capi)
    add_test(NAME propdev_effects_test COMMAND fc4_propdev_effects_test)
endif()

# PV18 post-service creation output: genuine C/holder/tag/reference spans.
if(TARGET FreeCity4)
    add_executable(fc4_propdev_tag_test tests/propdev_tag_test.cpp)
    target_include_directories(fc4_propdev_tag_test PRIVATE plugin)
    target_compile_definitions(fc4_propdev_tag_test PRIVATE _CRT_SECURE_NO_WARNINGS NOMINMAX WIN32_LEAN_AND_MEAN)
    target_compile_options(fc4_propdev_tag_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    target_link_libraries(fc4_propdev_tag_test PRIVATE fc4_capi)
    add_test(NAME propdev_tag_test COMMAND fc4_propdev_tag_test)
endif()

# Legacy-control recording must preserve native interface validation between builds.
if(TARGET FreeCity4)
    add_executable(fc4_legacyctl_hooks_test tests/legacyctl_hooks_test.cpp)
    target_include_directories(fc4_legacyctl_hooks_test PRIVATE plugin)
    target_compile_options(fc4_legacyctl_hooks_test PRIVATE /W3 /permissive- /utf-8 /EHsc /UNDEBUG)
    add_test(NAME legacyctl_hooks_contract COMMAND fc4_legacyctl_hooks_test)
endif()
# Native construction crew continuation and output-buffer ABI (NM17, AT25).
if(TARGET FreeCity4)
    add_executable(fc4_automata_construction_test tests/automata_construction_test.cpp)
    target_include_directories(fc4_automata_construction_test PRIVATE "${FC4_GZCOM_DIR}/include" plugin capi/include)
    target_compile_definitions(fc4_automata_construction_test PRIVATE _CRT_SECURE_NO_WARNINGS NOMINMAX)
    target_compile_options(fc4_automata_construction_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    add_test(NAME automata_construction_test COMMAND fc4_automata_construction_test "$<TARGET_FILE:FreeCity4>")
    set_tests_properties(automata_construction_test PROPERTIES RUN_SERIAL TRUE)
endif()
# City-situation persistence compares unordered map entries (section 7).
add_executable(fc4_citysit_record_test tests/citysit_record_test.cpp)
target_include_directories(fc4_citysit_record_test PRIVATE plugin)
add_test(NAME citysit_record_comparison COMMAND fc4_citysit_record_test)

# Network-manager NM18: provenance of old-neighbour replacements.
add_executable(fc4_netmgr_terrain_observation_test tests/netmgr_terrain_observation_test.cpp)
target_include_directories(fc4_netmgr_terrain_observation_test PRIVATE plugin)
add_test(NAME netmgr_terrain_observation COMMAND fc4_netmgr_terrain_observation_test)
# Effects resource: separate opt-in class, fixed persistence identity and stream ABI.
if(TARGET FreeCity4)
    target_sources(FreeCity4 PRIVATE plugin/FC4EffectsResource.cpp plugin/FC4EffectsResource.h)
    add_executable(fc4_effects_resource_test tests/effects_resource_test.cpp
        plugin/FC4EffectsResource.cpp "${FC4_GZCOM_DIR}/src/cRZBaseString.cpp")
    target_include_directories(fc4_effects_resource_test PRIVATE "${FC4_GZCOM_DIR}/include" plugin)
    target_compile_options(fc4_effects_resource_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    add_test(NAME effects_resource_test COMMAND fc4_effects_resource_test)
    foreach(mode default enabled disabled mixed)
        add_test(NAME plugin_effects_resource_registration_${mode}_test
            COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --effectsres-${mode})
        set_tests_properties(plugin_effects_resource_registration_${mode}_test
            PROPERTIES RUN_SERIAL TRUE TIMEOUT 60)
    endforeach()
endif()

# Native graphics-rule provider; the transport remains private to the static link.
if(TARGET FreeCity4)
    target_sources(FreeCity4 PRIVATE plugin/cFC4ConfigRules.cpp plugin/cFC4ConfigRules.h
        plugin/FC4ConfigRulesSystem.cpp plugin/FC4ConfigRulesSystem.h plugin/FC4ConfigRulesAcquisition.h
        plugin/FC4ConfigRulesTrace.cpp plugin/FC4ConfigRulesTrace.h)
    target_link_libraries(FreeCity4 PRIVATE version advapi32)
    add_executable(fc4_cfgrules_acquisition_test tests/cfgrules_acquisition_test.cpp)
    target_include_directories(fc4_cfgrules_acquisition_test PRIVATE plugin)
    target_compile_options(fc4_cfgrules_acquisition_test PRIVATE /W3 /permissive- /utf-8 /EHsc /UNDEBUG)
    add_test(NAME cfgrules_acquisition COMMAND fc4_cfgrules_acquisition_test)
    add_executable(fc4_cfgrules_snapshot_test tests/cfgrules_snapshot_test.cpp)
    target_include_directories(fc4_cfgrules_snapshot_test PRIVATE plugin "${FC4_GZCOM_DIR}/include")
    target_compile_options(fc4_cfgrules_snapshot_test PRIVATE /W3 /permissive- /utf-8 /EHsc /UNDEBUG)
    add_test(NAME cfgrules_snapshot COMMAND fc4_cfgrules_snapshot_test)
endif()
if(TARGET fc4_plugin_load_test)
    foreach(mode default enabled disabled)
        add_test(NAME plugin_cfgrules_${mode}
            COMMAND fc4_plugin_load_test "$<TARGET_FILE:FreeCity4>" --cfgrules-${mode})
        set_tests_properties(plugin_cfgrules_${mode} PROPERTIES RUN_SERIAL TRUE)
    endforeach()
endif()
if(WIN32 AND CMAKE_SIZEOF_VOID_P EQUAL 4)
    target_sources(FreeCity4 PRIVATE plugin/FC4View3DShadow.cpp plugin/FC4View3DShadow.h plugin/FC4View3DObservation.h)
    add_executable(fc4_view3d_observation_test tests/view3d_observation_test.cpp)
    target_include_directories(fc4_view3d_observation_test PRIVATE plugin)
    target_compile_options(fc4_view3d_observation_test PRIVATE /W3 /permissive- /utf-8 /EHsc /UNDEBUG)
    add_test(NAME view3d_observation COMMAND fc4_view3d_observation_test)
endif()

# Method evidence must retain incomplete entries and every observed mismatch.
add_executable(fc4_method_evidence_test tests/method_evidence_test.cpp)
target_include_directories(fc4_method_evidence_test PRIVATE plugin)
add_test(NAME method_evidence COMMAND fc4_method_evidence_test)

# Same-invocation sub-budget method recording (private static-link transport).
if(TARGET FreeCity4)
    target_sources(FreeCity4 PRIVATE plugin/FC4BudgetWinShadow.cpp plugin/FC4BudgetWinShadow.h)
endif()

# Same-invocation cHTMLDocument observer; selected with compare=class:cHTMLDocument.
if(TARGET FreeCity4)
    target_sources(FreeCity4 PRIVATE plugin/FC4HTMLDocumentShadow.cpp plugin/FC4HTMLDocumentShadow.h)
endif()
# Cargo's console target can prune cached link steps through Ninja restat.
# Emit a completed edge afterwards so the native gate sees final progress.
add_custom_target(fc4_capi_ready ALL
    COMMAND ${CMAKE_COMMAND} -E true
    COMMENT "Rust static archive ready"
    VERBATIM)
add_dependencies(fc4_capi_ready fc4_capi_cargo)

# Advisor list reads snapshot side effects across the C host's size/fill pair.
add_executable(fc4_advisor_list_read_test tests/advisor_list_read_test.cpp)
target_include_directories(fc4_advisor_list_read_test PRIVATE plugin capi/include)
add_test(NAME advisor_list_read COMMAND fc4_advisor_list_read_test)

# Dispatch's native pylon primary includes the occupant accessor omitted by the SDK.
if(WIN32 AND CMAKE_SIZEOF_VOID_P EQUAL 4)
    add_executable(fc4_dispatch_pylon_test tests/dispatch_pylon_test.cpp)
    target_include_directories(fc4_dispatch_pylon_test PRIVATE plugin capi/include "${FC4_GZCOM_DIR}/include")
    target_compile_options(fc4_dispatch_pylon_test PRIVATE /W3 /permissive- /utf-8 /EHsc)
    add_test(NAME dispatch_pylon COMMAND fc4_dispatch_pylon_test)
endif()

# Incoming automaton events use Data1, independently of outgoing post ownership.
add_executable(fc4_dispatch_message_test tests/dispatch_message_test.cpp)
target_include_directories(fc4_dispatch_message_test PRIVATE plugin)
add_test(NAME dispatch_message COMMAND fc4_dispatch_message_test)
