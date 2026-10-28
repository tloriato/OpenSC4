// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the Free
// Software Foundation, either version 3 of the License, or (at your option)
// any later version. This program is distributed WITHOUT ANY WARRANTY; see
// the GNU General Public License for more details. You should have received a
// copy of the license along with this program (see LICENSE at the repository
// root); if not, see <https://www.gnu.org/licenses/>.

// The DLL's test seam: one export, FC4TestSeam, that tests (which load the
// DLL without a game framework) use to see what would otherwise only reach
// the game. The game never calls it; nothing changes unless it is called.
//
//   intptr_t __cdecl FC4TestSeam(uint32_t op, void* a, void* b);
//
//   op 1  kFC4SeamBudgetPostSink   a = FC4PostSinkFn (or null), b = context.
//         Every budget message post builds the message even without a
//         message server and hands it to the sink before posting (the sink
//         must not keep the pointer).
//   op 2  kFC4SeamObserverCity     a = cISC4City* the observer uses instead
//         of asking the application (null: back to the application).
//   op 3  kFC4SeamObserverTick     the observer's OnTick.
//   op 4  kFC4SeamObserverMessage  the observer's DoMessage with a standard
//         message of type (uint32_t)a and data1 (intptr_t)b.
//   op 5  kFC4SeamObserverApp      a = cISC4App* the observer's save autotest
//         uses instead of the framework's application (null: back to it).
//         The popup-dialogs switch (FC4Popups.h) uses it too (borrowed).
//   op 6  kFC4SeamPowerBudgetPath  a = cISC4BudgetSimulator* (initialised):
//         runs the power glue's department / line-item callbacks on it,
//         including a department reference read back from a save; returns
//         0, or the first failed step (logged).
//   op 7  kFC4SeamFpuReference     a = null: the calling thread's x87 control
//         word becomes the guard's reference (as PreAppInit does in the
//         game); a != null: the per-tick check alone (FpuTick).
//   op 8  kFC4SeamRustPanic        a deliberate panic in the core
//         (fc4_debug_panic): release builds log the panic report and
//         abort with a fail-fast.
//   op 9  kFC4SeamFlammabilityFire a = cISC4City*: the flammability glue's
//         ignition path (IgniteCell, then SetCause(5)) on the city's
//         disaster layer; returns 0, or 1 (no city) / 2 (no ignition).
//   op 10 kFC4SeamBudgetShadow     a = const FC4ShadowTestInput*
//         (FC4BudgetShadow.h): one budget-shadow month on a fake city whose
//         budget is the "original"; returns the month's mismatch count, -1
//         (not taken) or -2 (delivered, not compared).
//   op 11 kFC4SeamDemandShadow     a = const FC4DemandShadowTestInput*
//         (FC4DemandShadow.h): one demand-shadow delivery (NewDay or SimIdle)
//         to a fake "original" demand simulator; returns the update's
//         mismatch count, -1 (not taken) or -2 (delivered, nothing compared).
//   op 12 kFC4SeamRefAuditRelease  a = cIGZUnknown* (a game object whose
//         Release returns 0), b = mode: bit 0 = note an acquired reference
//         first (FC4_ACQUIRED), bit 1 = release with FC4_RELEASE_CREATED,
//         bit 2 = with FC4_RELEASE_AT_DESTRUCTION (else FC4_RELEASE); returns
//         the reference warnings it caused.
//   op 13 kFC4SeamCrimeShadow      a = const FC4CrimeShadowTestInput*
//         (FC4CrimeShadow.h): one crime-shadow month (SimNewMonth, then
//         SimNewDay up to pipeline state 4) to a fake "original" crime
//         simulator; returns the month's mismatch count, -1 (not taken) or
//         -2 (delivered, nothing compared).
//   op 14 kFC4SeamAuraShadow       a = const FC4AuraShadowDeliverInput*
//         (FC4AuraShadow.h): the aura shadow's city-load start on a fake city
//         whose aura simulator is the "original"; returns 1 (ours started
//         from its record), 0 (not) or -1 (not run).
//   op 16 kFC4SeamPollutionShadow  a = const FC4PollutionShadowTestInput*
//         (FC4PollutionShadow.h): one pollution-shadow delivery (SimNewDay or
//         SimNewMonth) to a fake "original" pollution simulator; returns the
//         delivery's difference count, -1 (not taken) or -2 (delivered,
//         nothing compared). (15 is left to the parallel glue lanes.)
//   op 17 kFC4SeamFloraShadow      a = const FC4FloraShadowTestInput*,
//         b = intptr_t* (or null) for the registry check's difference count
//         (FC4FloraShadow.h): one flora-shadow SimNewMonth to a fake
//         "original" flora simulator; returns the month's mismatch count, -1
//         (not taken) or -2 (delivered, nothing compared).
//   op 18 kFC4SeamLandfillShadow   a = const FC4LandfillShadowTestInput*
//         (FC4LandfillShadow.h): one landfill-shadow SimNewMonth delivery to
//         a fake "original" landfill developer; returns the month's
//         difference count, -1 (not taken) or -2 (delivered, nothing
//         compared).
//   op 20 kFC4SeamDemandCapacity   a = const FC4DemandCapacityTestInput*
//         (cFC4Demand.h): the demand glue's capacity-map read of a building
//         profile (DemandReportCapacities, no simulator): the visitor gets
//         every entry; returns the entries reported, 0 (switch off) or -1
//         (refused, logged).
//   op 21 kFC4SeamPowerShadow      a = const FC4PowerShadowTestInput*
//         (FC4PowerShadow.h): one power-shadow delivery (SimNewDay, SimIdle
//         or SimNewMonth) to a fake "original" power simulator; returns the
//         delivery's difference count, -1 (not taken), -2 (delivered,
//         nothing compared) or -3 (passed through: the original idle).
//   op 22 kFC4SeamPlumbingShadow   a = const FC4PlumbingShadowTestInput*
//         (FC4PlumbingShadow.h): a run of deliveries (SimIdle, SimNewDay,
//         SimNewMonth) to a fake "original" plumbing simulator, one shadow for
//         the run; returns the summed difference count of the passes and
//         months compared, -1 (none taken) or -2 (taken, nothing compared).
//   op 23 kFC4SeamLandValueShadow  a = const FC4LandValueShadowTestInput*
//         (FC4LandValueShadow.h): one land value shadow event on a fake city
//         whose land value simulator is the "original" (a pass end, a
//         SimNewDay, PostCityShutdown); returns the pass's difference count,
//         -1 (not run) or -2 (ran, nothing compared); a day: the maps that
//         changed.
//   op 24 kFC4SeamFlammabilityShadow a = const FC4FlammabilityShadowTestInput*
//         (FC4FlammabilityShadow.h): one flammability shadow event on a fake
//         city whose flammability simulator is the "original" (one of its six
//         messages, a SimCycleComplete, PostCityShutdown); returns the cycle's
//         difference count, -1 (not run) or -2 (ran, nothing compared: the
//         baseline or INCOMPLETE); a message: 1 mirrored, 0 not.
//   op 25 kFC4SeamResidentialShadow a = const FC4ResidentialShadowTestInput*
//         (FC4ResidentialShadow.h): one SimNewDay delivery to a fake
//         "original" residential simulator, wrapped by the residential shadow
//         (its record captured before and after through its own Write, its
//         slots 16 / 32-34 read after); returns the delivery's difference
//         count, -1 (not taken) or -2 (taken, nothing compared).
//   op 26 kFC4SeamRawArrayBytes    a = cISCPropertyHolder*, b =
//         FC4RawArrayBytesTestIo*: fc4::props::GetRawArrayBytes of property
//         b->id (the crime effect read, crime.md §6.10 / CQ13): b->len = the
//         size, b->bytes = its first 32 bytes; returns 1 (read), 0 (absent,
//         scalar or object array) or -1 (null argument).
//   op 27 kFC4SeamLotCenter        a = cISC4Lot*, b = float[3]: the crime
//         glue's lot GetCenter thunk (slot 15, crime.md §13 / CQ3) into b;
//         returns the values the call left on the x87 stack (TOP change mod
//         8: 0 = unchanged, the original; 1 = one pushed, NOT popped, so the
//         caller must pop it; 7 = one popped), -1 (null argument).
//   op 28 kFC4SeamCellCenter       a = cISC4City*, b = FC4CellCenterTestIo*:
//         the protection and the landfill glue's city slot 97
//         CellCenterToPosition calls (void, pop 16; docs/abi corrected
//         2026-10-14) for (b->cx, b->cz); returns 1, or -1 (null argument).
//   op 29 kFC4SeamPipeBurst        a = cISC4City*, b = const int32_t[2]
//         (x, z): the plumbing glue's AttemptPipeBurst (plumbing.md §7.1
//         step 4); its result is ignored, never released. Returns 1 (the
//         manager existed), 0 (none) or -1 (null argument).
//   op 30 kFC4SeamVariant          a = cFC4IGZVariant** (out): a heap
//         variant of the plugin's own class (the one handed to the game),
//         with one reference for the caller; returns 1, or -1.
//   op 31 kFC4SeamX87Check         the x87 leftover guard (FC4Fpu.h) at the
//         site "test seam"; returns the occupied x87 registers it found.
//   op 32 kFC4SeamProtectionShadowVtables a = fire, b = police primary
//         vtable (const void*) the protection shadow accepts as the original
//         classes' instead of the compat table's, to read the unadjusted grid
//         at primary +0x84 (FC4ProtectionShadow.h, PR5); both null: back to
//         the compat table. Returns 1.
//   op 34 kFC4SeamDevelopmentShadow a = const FC4DevelopmentShadowTestInput*
//         (FC4DevelopmentShadow.h): the development shadow's list comparison
//         (mode 0: the mismatch count) or its record bridge (mode 1: a growth
//         developer's record read into a new read-only core: 1 read, 0
//         refused, -2 no record); -1: a null or malformed input.
//   op 35 kFC4SeamAuraShadowDeliver a = const FC4AuraShadowDeliverInput*
//         (FC4AuraShadow.h): one delivery to a fake original aura simulator
//         through the aura shadow (its state read before and after, our
//         steps), or an occupant message to the shadow's subscription;
//         returns the original's state after the delivery, -1 (not taken,
//         delivered by the seam), -2 (taken, state unreadable after), or for
//         an occupant message 1 (ours received it) / 0.
//   op 38 kFC4SeamZones            a = const FC4ZonesTestInput*
//         (cFC4ZoneManager.h): the zone manager's test hooks (a host table
//         answering instead of the game, a ZoningChange sink, the outLots
//         node allocator) or one zones shadow comparison. Returns what
//         ZonesTestSeam returns, -1 for a null argument.
//   op 39 kFC4SeamGrowthProbe      a = const FC4GrowthProbeTestInput*
//         (FC4GrowthProbe.h): the growth probe started on a given vtable
//         (instead of the compat table's), one of its messages delivered
//         with a given city, or the probe stopped; see GrowthProbeTest for
//         the return values (-1: null argument).
//   op 42 kFC4SeamTraffic          a = const FC4TrafficTestInput*
//         (cFC4TrafficSimulator.h): the traffic simulator's test hooks (a
//         host table answering instead of the game, a post sink), or the
//         count of live slot-6 pathfinders. Returns what TrafficTestSeam
//         returns, -1 for a null argument.
//   op 46 kFC4SeamScript          a = const char* path (the harness lane,
//         FC4Script.h): loads that script in place of the current one, as the
//         `script` setting does at game start. Returns the number of actions
//         it will run, -1 (cannot be read), -2 (refused: parse errors).
//   op 47 kFC4SeamDialogs         a = const FC4DialogsTestInput* (the harness
//         lane, FC4Dialogs.h): the dialog closer's test hooks (a window
//         manager, a legacy message server and a cSC4WinGameDelay vtable used
//         instead of the game's; start, stop, counts); -1 for a null input.
//   op 48 kFC4SeamNeighborsApp    a = cISC4App* (borrowed; null: the
//         framework's again): the app whose GetPopupDialogsEnabled (slot 23)
//         the neighbours simulator's RequestCreateConnection reads (NG14,
//         cFC4Neighbors.h NeighborsTestSetApp). Returns 1. With `b`
//         non-null instead: a = cISC4NeighborDeal* (borrowed), b =
//         cISC4NeighborDeal** receiving a new deal of ours with a's state
//         (one reference for the caller; NeighborsTestCopyDeal, NG4).
//         Returns 1, 0 if none was made.
//   op 51 kFC4SeamTrafficShadow   a = const FC4TrafficShadowTestInput*
//         (FC4TrafficShadow.h): one delivery through the traffic shadow to a
//         fake original; returns the delivery's difference count, -1 (not
//         taken, nothing delivered), -2 (delivered, nothing compared).
//   op 52 kFC4SeamLotDev          a = const FC4LotDevTestInput*
//         (cFC4LotDeveloper.h): the lot developer's test hooks (a host table
//         answering instead of the game, test allocators), one shadow
//         comparison, or the class's counters. Returns what LotDevTestSeam
//         returns, -1 for a null argument.
//   op 53 kFC4SeamZoneDev          a = const FC4ZoneDevTestInput* (cFC4ZoneDeveloper.h),
//         b = the state block for kFC4ZoneDevTestState: the zone developer's test
//         hooks (a host table answering instead of the game, the raw structures'
//         allocators), one zonedev shadow comparison, or what it holds. Returns
//         what ZoneDevTestSeam returns, -1 for a null argument.
//   op 54 kFC4SeamHistClock       a = const FC4HistClockTestInput*
//         (FC4HistClock.h; the history and 24-hour clock lane): op 0 installs
//         the hooks the history warehouse, the clock and their shadows call
//         instead of the game (the date, notifications, agents, the record's
//         MessageSend, the clock's MessagePost, the framework's system-service
//         and on-idle lists, the cheat manager, the timer, the tuning, the
//         shadows' city); the other ops run one shadow event. Returns what
//         HistClockTestSeam returns, -1 for a null argument.
//   op 57 kFC4SeamParks           a = const FC4ParksTestInput* (the cmglue
//         lane, cFC4Parks.h): the park host's ZooEscape post (parks §4.5) to
//         a's message server, with void1 = a's manager and IGZUnknown = a's
//         zoo; returns the number of posts the server accepted, -1 for a null
//         input.
//   op 58 kFC4SeamTerrainShadow   a = FC4TerrainShadowTestInput* (the glue16
//         lane, FC4TerrainShadow.h): the terrain shadow's scenario on a
//         stand-in original made inside the DLL; returns its mismatch count
//         (the counters in a), -1 for a bad input.
//   op 59 kFC4SeamCitySitShadow    a = FC4CitySitShadowTestInput* (the glue17
//         lane, FC4CitySitShadow.h): event 0 reports whether citysit_shadow
//         is on and the run's tallies (1 / 0), event 1 writes the city
//         shutdown summary. -1 for a bad input.
//   op 60 kFC4SeamDemolitionShadow a = FC4DemolitionShadowTestInput* (the
//         glue17 lane, FC4DemolitionShadow.h); same events as op 59.
//   op 61 kFC4SeamDispatchShadow   a = FC4DispatchShadowTestInput* (the
//         glue17 lane, FC4DispatchShadow.h); same events as op 59.
//   op 59 kFC4SeamTerrainGlue     a = FC4TerrainGlueTestInput* (the glue18
//         lane, cFC4Terrain.h): the terrain replacement through the same code
//         as in game (create, the consumer slots of terrain.md §15.3, TR18's
//         refuse-and-count, a change and its posts); returns its mismatch
//         count (the counters in a), -1 for a bad input.//   op 66 kFC4SeamNeighborsConfirm a = fc4::NeighborsConfirmFn (cdecl bool
//         (cIGZString* title, cIGZString* text, cIGZString* yes,
//         cIGZString* no); null: the game's again) (the apply12 lane,
//         neighbors.md §15.4 NG35): the confirmation dialog the neighbours
//         simulator's RequestCreateConnection calls when popups are on.
//         Returns 1.
#pragma once

#include <cstdint>

class cIGZMessage2Standard;

// kFC4SeamCellCenter's in/out block: the cell, then each glue's position.
struct FC4CellCenterTestIo {
    int32_t cx, cz;
    float protectionX, protectionZ;
    float landfillX, landfillZ;
};

// kFC4SeamRawArrayBytes's in/out block.
struct FC4RawArrayBytesTestIo {
    uint32_t id;
    uint32_t len;
    uint8_t bytes[32];
};

enum : uint32_t {
    kFC4SeamBudgetPostSink = 1,
    kFC4SeamObserverCity = 2,
    kFC4SeamObserverTick = 3,
    kFC4SeamObserverMessage = 4,
    kFC4SeamObserverApp = 5,
    kFC4SeamPowerBudgetPath = 6,
    kFC4SeamFpuReference = 7,
    kFC4SeamRustPanic = 8,
    kFC4SeamFlammabilityFire = 9,
    kFC4SeamBudgetShadow = 10,
    kFC4SeamDemandShadow = 11,
    kFC4SeamRefAuditRelease = 12,
    kFC4SeamCrimeShadow = 13,
    kFC4SeamAuraShadow = 14,
    kFC4SeamPollutionShadow = 16,
    kFC4SeamFloraShadow = 17,
    kFC4SeamLandfillShadow = 18,
    kFC4SeamDemandCapacity = 20,
    kFC4SeamPowerShadow = 21,
    kFC4SeamPlumbingShadow = 22,
    kFC4SeamLandValueShadow = 23,
    kFC4SeamFlammabilityShadow = 24,
    kFC4SeamResidentialShadow = 25,
    kFC4SeamRawArrayBytes = 26,
    kFC4SeamLotCenter = 27,
    kFC4SeamCellCenter = 28,
    kFC4SeamPipeBurst = 29,
    kFC4SeamVariant = 30,
    kFC4SeamX87Check = 31,
    kFC4SeamProtectionShadowVtables = 32,
    kFC4SeamDevelopmentShadow = 34,
    kFC4SeamAuraShadowDeliver = 35,
    kFC4SeamZones = 38,
    kFC4SeamGrowthProbe = 39,
    kFC4SeamTraffic = 42,
    kFC4SeamTract = 45,   // the tract developer (cFC4TractDeveloper.h FC4TractTestInput)
    kFC4SeamScript = 46,  // the harness lane
    kFC4SeamDialogs = 47,  // the harness lane
    kFC4SeamNeighborsApp = 48,  // the neighbours apply lane (NG14)
    kFC4SeamTrafficShadow = 51,  // the traffic shadow (FC4TrafficShadow.h FC4TrafficShadowTestInput)
    kFC4SeamLotDev = 52,  // the lot developer (cFC4LotDeveloper.h FC4LotDevTestInput)
    kFC4SeamZoneDev = 53,  // the zone developer (cFC4ZoneDeveloper.h FC4ZoneDevTestInput)
    kFC4SeamHistClock = 54,  // the history and clock lane
    kFC4SeamParks = 57,         // the cmglue lane (cFC4Parks.h FC4ParksTestInput)
    kFC4SeamTerrainShadow = 58,  // the glue16 lane (FC4TerrainShadow.h FC4TerrainShadowTestInput)
    kFC4SeamCitySitShadow = 59,  // the glue17 lane (FC4CitySitShadow.h)
    kFC4SeamDemolitionShadow = 60,  // the glue17 lane (FC4DemolitionShadow.h)
    kFC4SeamDispatchShadow = 61,  // the glue17 lane (FC4DispatchShadow.h)
    kFC4SeamTerrainGlue = 67,  // the glue18 lane (cFC4Terrain.h FC4TerrainGlueTestInput; 59 taken by kFC4SeamCitySitShadow)
    kFC4SeamNeighborsConfirm = 66,  // the apply12 lane (NG35, cFC4Neighbors.h NeighborsTestSetConfirm)
    kFC4SeamMySimsShadow = 68,  // the mysims1 lane (FC4MySimsShadow.h MySimsShadowTest)
    // the verify-automatarefs4 lane (cFC4Automata.h AutomataCmTeardownTest):
    // a = cIGZUnknown* game object the controller manager holds to its
    // destruction, b = cIGZUnknown* one it releases before (or null); returns
    // the reference warnings caused.
    kFC4SeamAutomataCmTeardown = 69,
    kFC4SeamUnreadRefs = 70,  // glue-exemplars4: a reference read in Read is not touched before the store load ends
    kFC4SeamAdvisorsShadow = 71,  // record-only snapshot of a = borrowed cIGZUnknown*
    kFC4SeamOrdinanceShadow = 72,  // a = borrowed cISC4OrdinanceSimulator*, capture/step copies only
    kFC4SeamAutomataCapture = 73,  // a = reference-only writer, b = requested IID
    kFC4SeamConstructionCapture = 74,  // a = construction writer; 1 exact, 0 mismatch, -1 capture failed
    kFC4SeamConstructionRecordFactory = 76,  // record staging only; not a registered replacement
    kFC4SeamLotConfigurationCreate = 75,  // returns the manager's own created reference (not GZCOM).
    // Isolated factory only: never registered for native power-line tools.
    // Returns an owned cIGZUnknown* (one reference); a and b must be null.
    kFC4SeamNetOccPoleFactory = 0x4E4F0001,
    kFC4SeamNetworkLotOwnership = 77,  // a,b = unknown prefixes of fake network occupants
    kFC4SeamNetworkCrewOwnership = 78,  // specified slot-5 shutdown and reentrant unregister
    kFC4SeamNetOccPoleLink = 0x4E4F0002, // a = FC4NetOccPoleLinkTest*, b = null
    kFC4SeamNetworkToolOccupantViews = 0x4E540001, // a = network view, b = synchronous message server
    kFC4SeamDemolitionLotBuilding = 0xD3000001, // a = borrowed lot; returns borrowed occupant token
    kFC4SeamDemolitionLotOccupants = 0xD3000002, // a = query-owned occupant list; returns reference warnings
    kFC4SeamMySimsActiveIds = 0x4D530001, // a = query-owned MySim vector; b = int32[5] IDs; returns ref warnings
    kFC4SeamMySimsUiBlocked = 0x4D530002, // a = borrowed window manager; returns specified modal gate
    kFC4SeamMySimsSavedReference = 0x4D530003, // a = borrowed Serializable; b = requested IID; returns borrowed host token
};

class cIGZUnknown;
struct FC4NetOccPoleLinkTest {
    uint32_t operation; // 0 enable, 1 add, 2 refresh, 3 attachment points, 4 remove
    cIGZUnknown* first;
    cIGZUnknown* second;
    void* buffer;       // native wire header for add, xyz[4] for attachment points
    uint32_t value;     // enable/quiet or attachment orientation
};

using FC4PostSinkFn = void (*)(void* context, cIGZMessage2Standard* msg);
// Native GZ owner construction, one caller reference. b=0 buffer, b=1 server;
// server a is a borrowed helper command service queried/retained by the seam.
constexpr uint32_t kFC4SeamGZFrameworkCreate = 0x475A0001;

// a = borrowed power-line unknown, b = uint64_t[2]; 1 for two non-null poles.
constexpr uint32_t kFC4SeamPowerLineConnections = 0x50570001;

// a = borrowed occupant, b = borrowed city; bits 0/1 = default/realistic inclusion.
constexpr uint32_t kFC4SeamDemolitionFilters = 0xD3000003;

using FC4TestSeamFn = intptr_t(__cdecl*)(uint32_t op, void* a, void* b);

namespace fc4 {
// FC4BudgetHost.cpp
void SetBudgetPostSink(FC4PostSinkFn fn, void* context);
}  // namespace fc4
