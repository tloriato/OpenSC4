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

// Version-specific interop with the host executable. This is the ONLY place
// that knows fixed addresses inside a particular SimCity 4 build; every entry
// comes from docs/specs or docs/abi (never from inspecting the executable). Anything here
// is used only after the running executable has been positively identified.
#pragma once

#include <cstdint>

namespace fc4 {
namespace compat {

// Non-virtual game functions called by address (docs/abi/direct-calls.md).
enum class Direct : unsigned {
    PoleRemoveConnection,  // cSC4PowerPoleOccupant::RemoveConnection: thiscall (other base, bool), pop 8
    PipeSetWateredState,   // cSC4PipeOccupant::SetWateredState: thiscall (bool), pop 4
    AirDispatchInit,       // air dispatch source Init: thiscall (), pop 0, AL
    AirDispatchShutdown,   // air dispatch source Shutdown: thiscall (), pop 0, AL
    MapU32Index,           // STLport map<uint32_t, 4-byte T>::operator[]: thiscall (const uint32_t*), pop 4,
                           //   EAX = the value's address (FC4GameMap.h)
    MapEraseSubtree,       // the same maps' subtree free: thiscall (node* top), pop 4 (FC4GameMap.h)
    GlobalHeapFree,        // allocator C free (the game's operator delete): cdecl void (void*)
                           //   (docs/abi/allocators.md; FC4SafeLotIterator.h frees the
                           //   safe lot iterator's block with it)
    GlobalHeapAllocate,    // allocator C allocate (the game's operator new): cdecl void* (uint32_t size)
                           //   (docs/abi/allocators.md; the zone developer's safe lot iterator
                           //   blocks, zones.md §8.3 slot 15, §13.2)
    LotPoolAllocate,       // the cSC4Lot pool's allocate: cdecl void* (uint32_t size = 0x8C, unread), the
                           //   caller pops (docs/abi/direct-calls.md; zones.md §8.5.1)
    LotConstruct,          // the cSC4Lot constructor: thiscall (x, z, w, h, facing), pop 20, EAX = this
                           //   (docs/abi/direct-calls.md; zones.md §8.5.1)
    GenericConfirmationDialog,  // nSC4UI::GenericConfirmationDialog: cdecl bool (cIGZString* title,
                                //   cIGZString* text, cIGZString* yes, cIGZString* no), the caller pops
                                //   16, AL (neighbors.md §15.4 NG35; advisors.md §6.8)
    // The retaining-wall helpers (docs/abi/direct-calls.md; lot-developer.md §7.2, LD3): cdecl statics of
    // cSC4LotRetainingWallOccupant on the cISC4Lot*, the caller pops.
    RetainingWallAdd,                 // AddRetainingWallOccupant: bool (cISC4Lot*, bool add as 4 bytes), pops 8
    RetainingWallsUpdate,             // UpdateRetainingWalls: bool (cISC4Lot*), pops 4, AL undefined
    RetainingWallsUpdateSurrounding,  // UpdateSurroundingRetainingWalls: void (cISC4Lot*), pops 4
    BurntBuildingSetBounds,           // cSC4PropOccupantBurntBuilding::SetBoundsFromOccupant: thiscall on the
                                      //   base (cISC4Occupant* source), pop 4, void (direct-calls.md; LD14)
    TerrainView3DInit,                // the terrain 3D view's Init: fastcall (ECX = view base, EDX unused,
                                      //   stack cISTETerrain* primary), pop 4, EAX 1 ok / 0 already
                                      //   (direct-calls.md; terrain.md §15.5.4)
    TerrainStaticInit,                // cSTETerrain::StaticInit: no this, no arguments, AL 1, once per process
                                      //   (terrain.md §15.5.12.2, §15.5.11 item 18; TR40)
    TerrainView3DShutdown,            // the terrain 3D view's Shutdown: thiscall (ECX = view base), no
                                      //   arguments (terrain.md §15.5.11 item 15a; TR35)
    GetIGZDBSegment,                  // SC4GetIGZDBSegment: cdecl (stream), EAX = the DB segment the stream
                                      //   reads from (terrain.md §13.2; TR39)
    Count
};

// The disaster shadow's forwarders (the plugin's disasters_shadow, tooling;
// docs/specs/disasters.md §15, docs/abi/vtables.tsv / interfaces.tsv): each
// names one vtable entry of the original layer, its fire and riot classes or
// the tutorial system, and the original function that entry must hold.
enum class DisasterHook : unsigned {
    LayerBegun,             // cSC4DisasterLayer slot 8 DisasterHasBegun(instance), pop 4
    LayerEnded,             // slot 9 DisasterHasEnded(), pop 0
    LayerRegister,          // slot 13 RegisterDisasterLocation(instance, x, z, occupant, zoom, focus), pop 24
    LayerUnregisterAll,     // slot 14 UnregisterDisasterLocations(instance), pop 4
    LayerUnregisterByOcc,   // slot 15 UnregisterDisasterLocationByOccupant(instance, occupant), pop 8
    FireIgnite,             // fire instance internal slot 6 Ignite(occupant, force, fireID, focus), pop 16, AL
    FireIgniteCell,         // slot 7 IgniteCell(x, z, useProbability, teleport, fireID), pop 20
    FireExtinguishCell,     // slot 8 ExtinguishCell(x, z, amount), pop 12
    FireQuickBurnCell,      // slot 9 QuickBurnCell(x, z), pop 8
    FireIgniteCellRadius,   // slot 10 IgniteCellRadius(x, z, r0, r1), pop 16
    FireExtinguish,         // slot 11 Extinguish(occupant), pop 4
    FireBurnDown,           // slot 12 BurnDown(occupant), pop 4
    FireSetCause,           // slot 13 SetCause(c), pop 4
    FireMessage,            // fire instance cIGZMessageTarget2 slot 3 DoMessage(msg), pop 4
    FireManagerIgniteOcc,   // fire manager cISC4DisasterManagerFire slot 4 IgniteOccupant(o), pop 4
    RiotStart,              // riot manager slot 6 AttemptDisasterStart(x, z), pop 8
    RiotRemoveInstance,     // riot manager slot 9 RemoveInstanceFromList(instance), pop 4
    RiotMessage,            // riot instance cIGZMessageTarget2 slot 3 DoMessage(msg), pop 4
    RiotLock,               // riot manager's riot interface slot 9 LockNetworkOccupant(n), pop 4
    TutorialIsTutorial,     // cSC4TutorialSystem slot 8 IsTutorial(), pop 0
    Count
};

struct DisasterHookSite {
    uint32_t vtableVA;
    uint32_t slot;
    uint32_t functionVA;  // what the entry must hold
};

// One supported executable and its interop addresses (spec §5.5).
struct ExecutableVersion {
    const char* name;
    // Identification (main module PE headers, as loaded in memory).
    uint32_t timeDateStamp;
    uint32_t sizeOfImage;
    uint32_t imageBase;
    // CPU-cycle limit-timer coefficients (32-bit floats, virtual addresses).
    uint32_t cyclesToMillisecsVA;
    uint32_t millisecsToCyclesVA;
    // The STL node allocator instance (docs/abi/allocators.md): cdecl
    // `void* (uint32_t size)` / `void (void*)`.
    uint32_t stlNodeAllocateVA;
    uint32_t stlNodeFreeVA;
    // The game's cRZString vtable (docs/abi/allocators.md, Strings).
    uint32_t rzStringVtableVA;
    // The RZ framework's "set FPU precision" helper (foundations §18.1-18.2):
    // cdecl, one int mode, no return value, the caller pops.
    uint32_t fpuSetPrecisionVA;
    // Direct-call targets, indexed by Direct (docs/abi/direct-calls.md).
    uint32_t directVA[static_cast<unsigned>(Direct::Count)];
    // The original fire / police simulators' primary vtables (protection.md
    // §15.1, §15.2): the protection shadow reads the unadjusted grid at
    // primary +0x84 only from an object with one of these (§2, PR5).
    uint32_t protectionVtableVA[2];
    // The growth probe (building-development.md §12): the original tract
    // developer's primary vtable and the Grow its slot 5 holds
    // (tract-developer.md §1.1, §7.1; docs/abi/interfaces.tsv), and the
    // original BDS's primary vtable (building-development.md §1.1, §7.1).
    uint32_t tractDeveloperVtableVA;
    uint32_t tractDeveloperGrowVA;
    uint32_t bdsVtableVA;
    // The original aura simulator's cISC4AuraSimulator vtable (aura.md
    // §12.1): the aura shadow reads the object's pipeline state (§14.3) only
    // when its vtable is this one.
    uint32_t auraSimulatorVtableVA;
    // The dialog closer (modal-dialogs.md §3.4; tooling `close_dialogs`):
    // cSC4WinGameDelay's primary vtable (docs/abi/vtables.tsv
    // `cSC4WinGameDelay::vftable`, whose slots are the cIGZWin ones), so the
    // closer can leave SC4GameDelay's invisible modal window alone.
    uint32_t gameDelayWindowVtableVA;
    // The original neighbours simulator's cISC4NeighborsSimulator vtable
    // (neighbors.md compiled slot table, §15.4 NG7): the neighbours shadow
    // reads the object's state bytes only when its vtable is this one.
    uint32_t neighborsSimulatorVtableVA;
    // The original commercial simulator's primary vtable
    // (commercial-industrial.md §1.1, §2.1): the commercial shadow reads the
    // object's dirty byte (+0x20) and DoIdle state (+0x24) only when its
    // vtable is this one.
    uint32_t commercialSimulatorVtableVA;
    // The original park manager's primary vtable (parks.md §1): the parks
    // shadow reads the object's latch (+0x15) and distress threshold (+0x34)
    // only when its vtable is this one.
    uint32_t parkManagerVtableVA;
    // The disaster shadow's forwarder sites, indexed by DisasterHook.
    DisasterHookSite disasterHooks[static_cast<unsigned>(DisasterHook::Count)];
};

// The running executable if it is a table entry loaded at its preferred base
// (once per process, logged), else nullptr.
const ExecutableVersion* IdentifiedVersion();

using StlNodeAllocateFn = void*(__cdecl*)(uint32_t size);
using StlNodeFreeFn = void(__cdecl*)(void* block);

// The RZ framework's "set FPU precision" helper (foundations §18.1-18.2):
// clears the PC field of the control word, then sets bit 1 for mode 1
// (53-bit) or bits 0-1 for mode 2 (64-bit), so mode 0 means 24-bit. Cdecl,
// one int argument, no return value, and the callee's plain `ret` leaves the
// caller to pop - every game call site is `push 1; call; add esp,4`. A
// `__stdcall`/`__thiscall` declaration would corrupt the stack.
using FpuSetPrecisionFn = void(__cdecl*)(int mode);

// The game's set-FPU-precision helper, or false (logged once) if the
// executable is not identified or the address is not executable code inside
// its image. Never guessed. `mode` comes from FC4FpuLogic.h (0 = 24-bit).
bool FpuSetPrecision(FpuSetPrecisionFn* setPrecision);

// The game's STL node allocator entry points (docs/abi/allocators.md), or
// false (logged once) if the executable is not identified or the addresses
// are not executable code inside its image. Never guessed.
bool StlNodeAllocator(StlNodeAllocateFn* allocate, StlNodeFreeFn* release);

// The game's cRZString vtable, or nullptr (logged once) if the executable is
// not identified or the address is not read-only data inside its image whose
// first entry is code inside the image.
const void* GameStringVtable();

// The original fire (kind 0) or police (kind 1) simulator's primary vtable
// (protection.md §15.1 / §15.2), or nullptr (logged once per kind) if the
// executable is not identified or the address is not committed, readable
// memory inside its image whose first entry is code inside the image.
const void* ProtectionPrimaryVtable(unsigned kind);

// The growth probe (building-development.md §12): the original tract
// developer's primary vtable and the original Grow its slot 5 must hold, or
// false (logged once) if the executable is not identified, the vtable is not
// committed, readable memory inside its image whose first entry is code
// inside the image, or the Grow address is not executable code inside the
// image. The vtable is returned as it is; the probe checks the entry itself.
bool TractDeveloperGrow(void*** vtable, uintptr_t* grow);

// The original building development simulator's primary vtable
// (building-development.md §7.1), or nullptr (logged once; same checks).
const void* BuildingDevelopmentVtable();

// The address of a direct-call target, or 0 (logged once per target) if the
// executable is not identified or the address is not executable code inside
// its image. Never guessed.
uintptr_t DirectCall(Direct which);

// The original aura simulator's cISC4AuraSimulator vtable (aura.md §12.1),
// or nullptr (logged once) if the executable is not identified or the
// address is not committed data inside its image whose first entry is code
// inside the image. For the aura shadow's raw state read (§14.3).
const void* AuraSimulatorVtable();

// The dialog closer (modal-dialogs.md §3.4): cSC4WinGameDelay's primary
// vtable, or nullptr (logged once) if the executable is not identified or the
// address is not committed data inside its image whose first entry is code
// inside the image. Compared, never called through.
const void* GameDelayWindowVtable();
// The original neighbours simulator's cISC4NeighborsSimulator vtable
// (neighbors.md §15.4 NG7), or nullptr (logged once; the same checks). For
// the neighbours shadow's raw read of the original's state bytes.
const void* NeighborsSimulatorVtable();
// The original commercial simulator's primary (cISC4CommercialSimulator)
// vtable (commercial-industrial.md §1.1), or nullptr (logged once; the same
// checks). For the commercial shadow's raw read of dirty and state (§2.1).
const void* CommercialSimulatorVtable();
// The original park manager's primary (cISC4ParkManager) vtable (parks.md
// §1), or nullptr (logged once; the same checks). For the parks shadow's raw
// read of the latch and the threshold (§2).
const void* ParkManagerVtable();

// The disaster shadow (disasters_shadow): the vtable entry of `which` and the
// original function it must hold, or false (logged once per hook) if the
// executable is not identified, the vtable is not committed, readable memory
// inside its image, or the function is not executable code inside the image.
// The entry's current value is not checked here (the shadow compares it
// before it patches).
bool DisasterHookEntry(DisasterHook which, void*** entry, uintptr_t* function);

// The compatibility table.
const ExecutableVersion* SupportedVersions(int* count);

// Identifies the running executable and validates the limit-timer globals
// (once per process; every check and its outcome is logged). Returns true if
// writes are allowed.
bool LimitTimersAvailable();

// Writes CyclesToMillisecs / MillisecsToCycles if LimitTimersAvailable() and
// the values currently stored there are still plausible. Returns true if the
// values were written. Never writes otherwise (logs a warning once).
bool PublishLimitTimers(float cyclesToMs, float msToCycles);

// Number of successful publishes (diagnostics).
uint64_t PublishCount();

}  // namespace compat
}  // namespace fc4
