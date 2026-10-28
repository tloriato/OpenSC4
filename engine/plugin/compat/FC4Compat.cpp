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

#include "FC4Compat.h"

#include "FC4Log.h"

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>

#include <cmath>
#include <cstring>

namespace fc4 {
namespace compat {
namespace {

// ---- the compatibility table (values from docs/specs/simulator.md §5.5) ----------------------

const ExecutableVersion kVersions[] = {
    {
        "SimCity 4 Deluxe 1.1.641 (GOG, Windows)",
        0x4C12BFB7,  // PE TimeDateStamp
        0x007C8000,  // PE SizeOfImage
        0x00400000,  // image base
        0x00B0A840,  // CyclesToMillisecs (float)
        0x00B0A83C,  // MillisecsToCycles (float)
        0x0090CF54,  // STL node allocator: allocate (docs/abi/allocators.md)
        0x0090CF63,  // STL node allocator: free
        0x00A80810,  // cRZString vtable (docs/abi/allocators.md, Strings)
        0x008929E0,  // set FPU precision, cdecl one int (foundations §18.2)
        {
            0x0064DA00,  // cSC4PowerPoleOccupant::RemoveConnection (docs/abi/direct-calls.md)
            0x00648540,  // cSC4PipeOccupant::SetWateredState
            0x006DF8C0,  // air dispatch source Init
            0x006AC860,  // air dispatch source Shutdown
            0x00497A00,  // STLport map<uint32_t, 4-byte T>::operator[] (docs/abi/direct-calls.md)
            0x004A79A0,  // the same maps' subtree free (_M_erase)
            0x005E5620,  // global heap (allocator C) free (docs/abi/allocators.md)
            0x005E55E0,  // global heap (allocator C) allocate (docs/abi/allocators.md)
            0x006BF430,  // the cSC4Lot pool's allocate (docs/abi/direct-calls.md; zones.md §8.5.1)
            0x006C1E50,  // the cSC4Lot constructor (x, z, w, h, facing)
            0x0078DFF0,  // nSC4UI::GenericConfirmationDialog (neighbors.md §15.4 NG35)
            0x005E3070,  // cSC4LotRetainingWallOccupant::AddRetainingWallOccupant (direct-calls.md; LD3)
            0x005E37F0,  // cSC4LotRetainingWallOccupant::UpdateRetainingWalls
            0x005E3960,  // cSC4LotRetainingWallOccupant::UpdateSurroundingRetainingWalls
            0x005EEB70,  // cSC4PropOccupantBurntBuilding::SetBoundsFromOccupant (direct-calls.md; LD14)
            0x0041DB50,  // advisor win proc constructor (advisors.md §12.1.1 Direct-call ABI; AD4)
            0x0041E320,  // advisor win proc Init
            0x0041D650,  // advisor win proc show (slot 25)
            0x0041D710,  // advisor win proc HideWindow (slot 26)
            0x0041D830,  // advisor win proc teardown (slot 28)
            0x0040C720,  // game.tool_plop_network (advisors.md §8.3.1 row 1; AD5)
            0x0040C740,  // game.tool_plop_zone
            0x0040C760,  // game.tool_plop_building
            0x0040C780,  // game.tool_plop_flora
            0x0040C8B0,  // game.tool_button
            0x0040C870,  // game.window_budget
            0x0040C850,  // game.window_map
            0x0040C890,  // game.window_graph
            0x0040C9B0,  // game.window_query
            0x0040AF90,  // game.camera_jump (advisors.md §8.3.2; AD6)
            0x0040B1F0,  // game.camera_zoom
            0x0040A810,  // game.random_chance (advisors.md §8.2 entry 30, §12.1 slot 9; AD8)
            0x005FC020,  // _dofile_from_resource (advisors.md §8.2 entry 37, §4.2; AD10)
            0x005FB850,  // print (advisors.md §8.2 entry 38; AD7)
            0x005FC510,  // UpdateResourcesFromDirectory (advisors.md §4.2; AD10)
            0x005FBA20,  // LoadPackageScripts (advisors.md §4.2; AD10)
            0x005EBDF0,  // the placeable pool's allocate (prop-occupant.md §6.1)
            0x005EC370,  // the cSC43DPlaceableObject constructor (prop-occupant.md §6.1)
            0x004664A0,  // cS3DTextureBinding::AddRef (texture-bindings.md §2)
            0x008090E0,  // cS3DTextureBinding::Release (texture-bindings.md §2)
            0x00649BA0,  // native pole GetAttachmentPoints (network-occupants.md §4.6.3)
            0x0075C420,  // native terrain view Init (terrain.md §15.5.4)
            0x007560F0,  // native terrain view Shutdown (terrain-view.md §9.3)
            // Terrain-view rectangle consumers (overlay-manager.md section 4.6.1).
            0x007541C0,  // static land, rectangle value then context
            0x007543C0,  // dynamic land
            0x007545C0,  // static extension land, context/extension/rectangle value
            0x00754910,  // dynamic extension land
            0x00755560,  // static water, rectangle value then context
            0x00755700,  // dynamic water
            0x00754C60,  // static shadow, rectangle pointer then context
            0x00754F10,  // dynamic shadow,
            0x0058C580,  // collection constructor (effects.md 18.7.2)
            0x0058D0E0,  // collection publication (effects.md 18.7.3)
            0x005B1660,  // EffectsParticleConstruct (effects.md 18.8-18.10; effects-owners.md 5)
            0x00582FE0,  // EffectsDecalConstruct (effects.md 18.8-18.10; effects-owners.md 5)
            0x005836A0,  // EffectsDynamicConstruct (effects.md 18.8-18.10; effects-owners.md 5)
            0x005812F0,  // EffectsBrushConstruct (effects.md 18.8-18.10; effects-owners.md 5)
            0x00580F10,  // EffectsAttractorConstruct (effects.md 18.8-18.10; effects-owners.md 5)
            0x005BC9D0,  // EffectsScrubberConstruct (effects.md 18.8-18.10; effects-owners.md 5)
            0x005BDF10,  // EffectsSequenceConstruct (effects.md 18.8-18.10; effects-owners.md 5)
            0x005BE300,  // EffectsSoundConstruct (effects.md 18.8-18.10; effects-owners.md 5)
            0x00581C10,  // EffectsCameraConstruct (effects.md 18.8-18.10; effects-owners.md 5)
            0x005B0B50,  // EffectsParticleReadOld (effects.md 18.8-18.10; effects-owners.md 5)
            0x005B1DC0,  // EffectsParticleRead (effects.md 18.8-18.10; effects-owners.md 5)
            0x005AFFF0,  // EffectsParticleWrite (effects.md 18.8-18.10; effects-owners.md 5)
            0x005A4B50,  // EffectsDecalRead (effects.md 18.8-18.10; effects-owners.md 5)
            0x005A3E00,  // EffectsDecalWrite (effects.md 18.8-18.10; effects-owners.md 5)
            0x005837C0,  // EffectsDynamicRead (effects.md 18.8-18.10; effects-owners.md 5)
            0x00583620,  // EffectsDynamicWrite (effects.md 18.8-18.10; effects-owners.md 5)
            0x005ABFD0,  // EffectsVisualReadOld (effects.md 18.8-18.10; effects-owners.md 5)
            0x005ABF50,  // EffectsVisualRead (effects.md 18.8-18.10; effects-owners.md 5)
            0x005AB590,  // EffectsVisualWrite (effects.md 18.8-18.10; effects-owners.md 5)
            0x005B1C80,  // EffectsParticleDrop (effects.md 18.8-18.10; effects-owners.md 5)
            0x00583700,  // EffectsDynamicDestroy (effects.md 18.8-18.10; effects-owners.md 5),
            0x0074FE40,  // terrain StaticInit (terrain.md section 15.5.12.2)
            0x00587010,  // visual dictionary grow (effects.md 18.9.7)
            0x00589E80,  // visual dictionary insert, hidden iterator result (effects.md 18.9.7),
            0x007D4480,  // overlay depth offset (draw-context section 8)
            0x007D4530,  // packed render state (section 5.2)
            0x007D2A20,  // transparency (section 3; stdcall float)
            0x007D3140,  // shade
            0x007D31B0,  // self-lit shade
            0x007D2A30,  // reset transparency (no receiver/arguments)
            0x007D3290,  // reset shade
            0x007D4340,  // texture transform (section 7)
            0x007D4420,  // clear texture transform
            0x007D4280,  // filtering
            0x007D41E0,  // wrap
            0x007D3030,  // combiner
            0x007D3EF0,  // texture state
            0x007D30B0,  // environment colour (stdcall pointer)
            0x007D3E90,  // alpha test enable
            0x007D2F50,  // alpha function/reference,
            0x0046F240,  // dispatch.md §4.1.3: TrackOccupant
            0x0046EFB0,  // dispatch.md §4.1.3: UntrackOccupant
            0x0046D3A0,  // functions.tsv: ShowAllIndicators,
            0x0088FF19,  // cRZTimer::GetTimerCycle (timers §1.3; functions.tsv, cdecl out pointer),
            0x004B5D10,  // tutorial.md 4.2.1: registry GetItem
            0x004B5D80,  // registry SetActive
            0x00442920,  // tutorial.md 4.2.2: window constructor
            0x00443E60,  // window Init, ignored result
            0x00442C80,  // window Close
            0x00443970,  // window OnTutorialEvent
        },
        {0x00AB1160, 0x00AB2490},  // fire / police primary vtables (protection.md §15.1, §15.2)
        0x00AB3118,  // cSC4TractDeveloper primary vtable (tract-developer.md §1.1, §7.1)
        0x0070D130,  // cSC4TractDeveloper::Grow, its slot 5 (tract-developer.md §7.1; interfaces.tsv)
        0x00AB0AF8,  // cSC4BuildingDevelopmentSimulator primary vtable (building-development.md §1.1, §7.1)
        0x00AB07C8,  // cSC4AuraSimulator's cISC4AuraSimulator vtable (aura.md §12.1, §14.3)
        0x00AA56C0,  // cSC4WinGameDelay primary vtable (docs/abi/vtables.tsv; modal-dialogs.md §3.4)
        0x00AB1EE0,  // cSC4NeighborsSimulator's cISC4NeighborsSimulator vtable (neighbors.md §15.4 NG7)
        0x00AB0CCC,  // cSC4CommercialSimulator primary vtable (commercial-industrial.md §1.1, §2.1)
        0x00AB20AC,  // cSC4ParkManager primary vtable (parks.md §1, §2)
        {
            // The disaster shadow's forwarder sites (disasters.md §2.4, §2.7, §5.4-§5.8, §6.2,
            // §15; network-manager.md §10; docs/abi/vtables.tsv, interfaces.tsv), DisasterHook order.
            {0x00A9E390, 8, 0x00578FC0},   // cSC4DisasterLayer::DisasterHasBegun
            {0x00A9E390, 9, 0x00577E90},   // DisasterHasEnded
            {0x00A9E390, 13, 0x005783D0},  // RegisterDisasterLocation
            {0x00A9E390, 14, 0x00577D30},  // UnregisterDisasterLocations
            {0x00A9E390, 15, 0x00577DE0},  // UnregisterDisasterLocationByOccupant
            {0x00A9E5B0, 6, 0x0057A7D0},   // cSC4DisasterInstanceFire::Ignite
            {0x00A9E5B0, 7, 0x0057B170},   // IgniteCell
            {0x00A9E5B0, 8, 0x0057B350},   // ExtinguishCell
            {0x00A9E5B0, 9, 0x0057B500},   // QuickBurnCell
            {0x00A9E5B0, 10, 0x0057AF10},  // IgniteCellRadius
            {0x00A9E5B0, 11, 0x00579BD0},  // Extinguish
            {0x00A9E5B0, 12, 0x00579BF0},  // BurnDown
            {0x00A9E5B0, 13, 0x00579230},  // SetCause
            {0x00A9E580, 3, 0x00579600},   // cSC4DisasterInstanceFire::DoMessage
            {0x00A9E4DC, 4, 0x0057B930},   // cSC4DisasterManagerFire::IgniteOccupant
            {0x00A9E798, 6, 0x0057EA10},   // cSC4DisasterManagerRiot::AttemptDisasterStart(x, z)
            {0x00A9E798, 9, 0x0057D7E0},   // cSC4DisasterManagerRiot::RemoveInstanceFromList
            {0x00A9E7F0, 3, 0x0057E740},   // cSC4DisasterInstanceRiot::DoMessage
            {0x00A9E7C8, 9, 0x0057DA40},   // cSC4DisasterManagerRiot::LockNetworkOccupant
            {0x00A861B8, 8, 0x0043F650},   // cSC4TutorialSystem::IsTutorial
        },
        // glue-span4444-impl4: cSpanRender4444's shared static vtable and slots 1-5
        // (span-render-4444.md §1, §1.1).
        0x00AC185C,
        {0x00847CE0, 0x00847D30, 0x00847E40, 0x00847F10, 0x008480C0},
        0x00B207C0,  // the advisor system's private global (advisors.md §14.2, §8.3.1),
        0x0061B490,  // cISC4NetworkOccupant slot 30 SetPiece (network-rules.md §11.2, NR14)
        0x00B44870,  // network-manager §17.2
        0x00B20E88,  // tutorial.md 4.2.2: live owner for the original UI
    },
};

// ---- process-wide state ------------------------------------------------------------------------

enum class Resolution { Unknown, Available, Unavailable };
Resolution g_resolution = Resolution::Unknown;
const ExecutableVersion* g_version = nullptr;
volatile float* g_cyclesToMs = nullptr;
volatile float* g_msToCycles = nullptr;
bool g_disabledAfterCheck = false;
uint64_t g_publishCount = 0;

bool EnvIsOne(const char* name) {
    char value[16] = {};
    const DWORD n = GetEnvironmentVariableA(name, value, sizeof(value));
    return n > 0 && n < sizeof(value) && strcmp(value, "1") == 0;
}

bool Writable(DWORD protect) {
    if (protect & (PAGE_GUARD | PAGE_NOACCESS)) return false;
    const DWORD base = protect & 0xFF;
    return base == PAGE_READWRITE || base == PAGE_WRITECOPY || base == PAGE_EXECUTE_READWRITE ||
           base == PAGE_EXECUTE_WRITECOPY;
}

// Checks that `va` is a committed, writable page of the main module's image.
bool CheckAddress(const char* what, uintptr_t va, uintptr_t moduleBase) {
    MEMORY_BASIC_INFORMATION mbi{};
    if (VirtualQuery(reinterpret_cast<const void*>(va), &mbi, sizeof(mbi)) != sizeof(mbi)) {
        Log("compat: %s at 0x%08lX: VirtualQuery failed (error %lu)", what, static_cast<unsigned long>(va),
            static_cast<unsigned long>(GetLastError()));
        return false;
    }
    const bool ok = mbi.State == MEM_COMMIT && mbi.Type == MEM_IMAGE &&
                    reinterpret_cast<uintptr_t>(mbi.AllocationBase) == moduleBase && Writable(mbi.Protect);
    Log("compat: %s at 0x%08lX: state=0x%lX type=0x%lX protect=0x%lX allocation base=%p -> %s", what,
        static_cast<unsigned long>(va), static_cast<unsigned long>(mbi.State), static_cast<unsigned long>(mbi.Type),
        static_cast<unsigned long>(mbi.Protect), mbi.AllocationBase,
        ok ? "ok (committed, writable, inside the executable image)" : "REJECTED");
    return ok;
}

bool Plausible(float v) { return std::isfinite(v) && v > 0.0f; }

// A pair that someone (the original code or we) already calibrated: both
// positive, finite and reciprocal within 1 %.
bool ConsistentPair(float c2m, float m2c) {
    return Plausible(c2m) && Plausible(m2c) && std::fabs(static_cast<double>(c2m) * m2c - 1.0) < 0.01;
}

const ExecutableVersion* Identify() {
    // 1. Main module PE headers (as mapped in this process).
    HMODULE exe = GetModuleHandleW(nullptr);
    const uintptr_t base = reinterpret_cast<uintptr_t>(exe);
    const auto* dos = reinterpret_cast<const IMAGE_DOS_HEADER*>(exe);
    if (dos == nullptr || dos->e_magic != IMAGE_DOS_SIGNATURE) {
        Log("compat: WARNING main module has no DOS header");
        return nullptr;
    }
    const auto* nt = reinterpret_cast<const IMAGE_NT_HEADERS32*>(reinterpret_cast<const char*>(exe) + dos->e_lfanew);
    if (nt->Signature != IMAGE_NT_SIGNATURE || nt->FileHeader.Machine != IMAGE_FILE_MACHINE_I386 ||
        nt->OptionalHeader.Magic != IMAGE_NT_OPTIONAL_HDR32_MAGIC) {
        Log("compat: WARNING main module is not a 32-bit x86 PE image");
        return nullptr;
    }
    char exePath[MAX_PATH] = {};
    GetModuleFileNameA(nullptr, exePath, MAX_PATH);
    const uint32_t stamp = nt->FileHeader.TimeDateStamp;
    const uint32_t size = nt->OptionalHeader.SizeOfImage;
    const uint32_t prefBase = nt->OptionalHeader.ImageBase;
    Log("compat: host executable %s: loaded at 0x%08lX, PE TimeDateStamp=0x%08lX SizeOfImage=0x%08lX "
        "ImageBase=0x%08lX",
        exePath, static_cast<unsigned long>(base), static_cast<unsigned long>(stamp),
        static_cast<unsigned long>(size), static_cast<unsigned long>(prefBase));

    // 2. Find the table entry: TimeDateStamp + SizeOfImage must both match.
    const ExecutableVersion* v = nullptr;
    for (const ExecutableVersion& e : kVersions) {
        if (e.timeDateStamp == stamp && e.sizeOfImage == size) v = &e;
    }
    if (v == nullptr) {
        Log("compat: WARNING unrecognised executable (no table entry with TimeDateStamp 0x%08lX and SizeOfImage "
            "0x%08lX); no version-specific interop",
            static_cast<unsigned long>(stamp), static_cast<unsigned long>(size));
        return nullptr;
    }
    Log("compat: identified '%s' (TimeDateStamp and SizeOfImage match)", v->name);

    // 3. The addresses assume the executable is loaded at its preferred base.
    if (base != v->imageBase || prefBase != v->imageBase) {
        Log("compat: WARNING executable loaded at 0x%08lX (preferred 0x%08lX), table expects 0x%08lX; no "
            "version-specific interop",
            static_cast<unsigned long>(base), static_cast<unsigned long>(prefBase),
            static_cast<unsigned long>(v->imageBase));
        return nullptr;
    }

    return v;
}

bool Executable(DWORD protect) {
    if (protect & (PAGE_GUARD | PAGE_NOACCESS)) return false;
    const DWORD base = protect & 0xFF;
    return base == PAGE_EXECUTE || base == PAGE_EXECUTE_READ || base == PAGE_EXECUTE_READWRITE ||
           base == PAGE_EXECUTE_WRITECOPY;
}

// `va` is committed executable code of the main module's image.
bool CheckCode(const char* what, uintptr_t va, uintptr_t moduleBase) {
    MEMORY_BASIC_INFORMATION mbi{};
    if (VirtualQuery(reinterpret_cast<const void*>(va), &mbi, sizeof(mbi)) != sizeof(mbi)) {
        Log("compat: %s at 0x%08lX: VirtualQuery failed (error %lu)", what, static_cast<unsigned long>(va),
            static_cast<unsigned long>(GetLastError()));
        return false;
    }
    const bool ok = mbi.State == MEM_COMMIT && mbi.Type == MEM_IMAGE &&
                    reinterpret_cast<uintptr_t>(mbi.AllocationBase) == moduleBase && Executable(mbi.Protect);
    Log("compat: %s at 0x%08lX: state=0x%lX type=0x%lX protect=0x%lX -> %s", what, static_cast<unsigned long>(va),
        static_cast<unsigned long>(mbi.State), static_cast<unsigned long>(mbi.Type),
        static_cast<unsigned long>(mbi.Protect), ok ? "ok (executable code inside the image)" : "REJECTED");
    return ok;
}

void Resolve() {
    g_resolution = Resolution::Unavailable;

    if (EnvIsOne("FC4_DISABLE_CALIBRATION")) {
        Log("compat: FC4_DISABLE_CALIBRATION=1: limit-timer globals will NOT be written");
        return;
    }

    const ExecutableVersion* v = IdentifiedVersion();
    if (v == nullptr) {
        Log("compat: limit-timer globals NOT written (executable not identified). Subsystems that budget work in "
            "CPU cycles may stall.");
        return;
    }
    const uintptr_t base = v->imageBase;
    const uint32_t size = v->sizeOfImage;

    // 4. Both addresses: aligned, inside the image, committed and writable.
    const uintptr_t aC2M = v->cyclesToMillisecsVA;
    const uintptr_t aM2C = v->millisecsToCyclesVA;
    const bool inside = aC2M % 4 == 0 && aM2C % 4 == 0 && aC2M >= base && aC2M + 4 <= base + size && aM2C >= base &&
                        aM2C + 4 <= base + size;
    if (!inside) {
        Log("compat: WARNING table addresses 0x%08lX/0x%08lX are not aligned floats inside the image; NOT written",
            static_cast<unsigned long>(aC2M), static_cast<unsigned long>(aM2C));
        return;
    }
    if (!CheckAddress("CyclesToMillisecs", aC2M, base) || !CheckAddress("MillisecsToCycles", aM2C, base)) {
        Log("compat: WARNING limit-timer globals are not in writable image memory; NOT written");
        return;
    }

    // 5. Current values: the unmaintained static value 1.0f in both, or an
    //    already-calibrated reciprocal pair.
    const float c2m = *reinterpret_cast<const volatile float*>(aC2M);
    const float m2c = *reinterpret_cast<const volatile float*>(aM2C);
    const bool pristine = c2m == 1.0f && m2c == 1.0f;
    const bool calibrated = ConsistentPair(c2m, m2c);
    Log("compat: current values CyclesToMillisecs=%.9g MillisecsToCycles=%.9g -> %s", c2m, m2c,
        pristine ? "ok (both 1.0f, never calibrated)"
                 : (calibrated ? "ok (already a calibrated reciprocal pair)" : "REJECTED (not plausible)"));
    if (!pristine && !calibrated) {
        Log("compat: WARNING unexpected contents at the limit-timer addresses; NOT written");
        return;
    }

    g_version = v;
    g_cyclesToMs = reinterpret_cast<volatile float*>(aC2M);
    g_msToCycles = reinterpret_cast<volatile float*>(aM2C);
    g_resolution = Resolution::Available;
    Log("compat: limit-timer globals will be maintained at 0x%08lX (CyclesToMillisecs) and 0x%08lX "
        "(MillisecsToCycles)",
        static_cast<unsigned long>(aC2M), static_cast<unsigned long>(aM2C));
}

}  // namespace

const ExecutableVersion* IdentifiedVersion() {
    static bool done = false;
    static const ExecutableVersion* version = nullptr;
    if (!done) {
        done = true;
        version = Identify();
    }
    return version;
}

bool StlNodeAllocator(StlNodeAllocateFn* allocate, StlNodeFreeFn* release) {
    static int state = 0;  // 0 unknown, 1 available, -1 refused
    static StlNodeAllocateFn a = nullptr;
    static StlNodeFreeFn f = nullptr;
    if (state == 0) {
        state = -1;
        const ExecutableVersion* v = IdentifiedVersion();
        if (v == nullptr) {
            Log("compat: STL node allocator NOT available (executable not identified); game-filled lists are refused");
        } else if (!CheckCode("STL node allocate", v->stlNodeAllocateVA, v->imageBase) ||
                   !CheckCode("STL node free", v->stlNodeFreeVA, v->imageBase)) {
            Log("compat: STL node allocator NOT available (addresses rejected); game-filled lists are refused");
        } else {
            a = reinterpret_cast<StlNodeAllocateFn>(static_cast<uintptr_t>(v->stlNodeAllocateVA));
            f = reinterpret_cast<StlNodeFreeFn>(static_cast<uintptr_t>(v->stlNodeFreeVA));
            state = 1;
            Log("compat: STL node allocator at 0x%08lX / 0x%08lX ('%s')",
                static_cast<unsigned long>(v->stlNodeAllocateVA), static_cast<unsigned long>(v->stlNodeFreeVA),
                v->name);
        }
    }
    if (state != 1) return false;
    if (allocate != nullptr) *allocate = a;
    if (release != nullptr) *release = f;
    return true;
}

bool FpuSetPrecision(FpuSetPrecisionFn* setPrecision) {
    static int state = 0;  // 0 unknown, 1 available, -1 refused
    static FpuSetPrecisionFn fn = nullptr;
    if (state == 0) {
        state = -1;
        const ExecutableVersion* v = IdentifiedVersion();
        if (v == nullptr) {
            Log("compat: set-FPU-precision helper NOT available (executable not identified); 24-bit precision is not "
                "re-asserted");
        } else if (v->fpuSetPrecisionVA == 0 ||
                   !CheckCode("set FPU precision", v->fpuSetPrecisionVA, v->imageBase)) {
            Log("compat: set-FPU-precision helper NOT available (address rejected); 24-bit precision is not "
                "re-asserted");
        } else {
            fn = reinterpret_cast<FpuSetPrecisionFn>(static_cast<uintptr_t>(v->fpuSetPrecisionVA));
            state = 1;
            Log("compat: set FPU precision at 0x%08lX ('%s')", static_cast<unsigned long>(v->fpuSetPrecisionVA),
                v->name);
        }
    }
    if (state != 1) return false;
    if (setPrecision != nullptr) *setPrecision = fn;
    return true;
}

const void* GameStringVtable() {    static int state = 0;  // 0 unknown, 1 available, -1 refused
    static const void* vtable = nullptr;
    if (state == 0) {
        state = -1;
        const ExecutableVersion* v = IdentifiedVersion();
        MEMORY_BASIC_INFORMATION mbi{};
        const uintptr_t va = v ? v->rzStringVtableVA : 0;
        bool ok = v != nullptr && va % 4 == 0 && VirtualQuery(reinterpret_cast<const void*>(va), &mbi, sizeof(mbi)) == sizeof(mbi) &&
                  mbi.State == MEM_COMMIT && mbi.Type == MEM_IMAGE &&
                  reinterpret_cast<uintptr_t>(mbi.AllocationBase) == v->imageBase &&
                  !(mbi.Protect & (PAGE_GUARD | PAGE_NOACCESS));
        if (ok) {
            // The first entry (QueryInterface) must be code inside the image.
            const uintptr_t first = *reinterpret_cast<const uintptr_t*>(va);
            ok = CheckCode("cRZString vtable entry 0", first, v->imageBase);
        }
        if (ok) {
            vtable = reinterpret_cast<const void*>(va);
            state = 1;
            Log("compat: cRZString vtable at 0x%08lX", static_cast<unsigned long>(va));
        } else {
            Log("compat: cRZString vtable NOT available (%s); game strings are refused",
                v ? "address rejected" : "executable not identified");
        }
    }
    return state == 1 ? vtable : nullptr;
}

const void* ProtectionPrimaryVtable(unsigned kind) {
    static int state[2] = {};  // 0 unknown, 1 available, -1 refused
    static const void* vtable[2] = {};
    static const char* const kNames[2] = {"fire", "police"};
    if (kind > 1) return nullptr;
    if (state[kind] == 0) {
        state[kind] = -1;
        const ExecutableVersion* v = IdentifiedVersion();
        MEMORY_BASIC_INFORMATION mbi{};
        const uintptr_t va = v ? v->protectionVtableVA[kind] : 0;
        bool ok = v != nullptr && va != 0 && va % 4 == 0 &&
                  VirtualQuery(reinterpret_cast<const void*>(va), &mbi, sizeof(mbi)) == sizeof(mbi) &&
                  mbi.State == MEM_COMMIT && mbi.Type == MEM_IMAGE &&
                  reinterpret_cast<uintptr_t>(mbi.AllocationBase) == v->imageBase &&
                  !(mbi.Protect & (PAGE_GUARD | PAGE_NOACCESS));
        if (ok) {
            // The first entry (QueryInterface) must be code inside the image.
            const uintptr_t first = *reinterpret_cast<const uintptr_t*>(va);
            ok = CheckCode(kind == 0 ? "fire simulator vtable entry 0" : "police simulator vtable entry 0", first,
                           v->imageBase);
        }
        if (ok) {
            vtable[kind] = reinterpret_cast<const void*>(va);
            state[kind] = 1;
            Log("compat: original %s simulator vtable at 0x%08lX", kNames[kind], static_cast<unsigned long>(va));
        } else {
            Log("compat: original %s simulator vtable NOT available (%s)", kNames[kind],
                v ? "address rejected" : "executable not identified");
        }
    }
    return state[kind] == 1 ? vtable[kind] : nullptr;
}

namespace {

// A vtable of the identified executable: committed, readable image memory,
// aligned, whose first entry (QueryInterface) is code inside the image.
bool CheckImageVtable(const char* what, const ExecutableVersion* v, uintptr_t va) {
    MEMORY_BASIC_INFORMATION mbi{};
    bool ok = v != nullptr && va != 0 && va % 4 == 0 &&
              VirtualQuery(reinterpret_cast<const void*>(va), &mbi, sizeof(mbi)) == sizeof(mbi) &&
              mbi.State == MEM_COMMIT && mbi.Type == MEM_IMAGE &&
              reinterpret_cast<uintptr_t>(mbi.AllocationBase) == v->imageBase &&
              !(mbi.Protect & (PAGE_GUARD | PAGE_NOACCESS));
    if (ok) {
        const uintptr_t first = *reinterpret_cast<const uintptr_t*>(va);
        ok = CheckCode(what, first, v->imageBase);
    }
    return ok;
}

}  // namespace

bool TractDeveloperGrow(void*** vtable, uintptr_t* grow) {
    static int state = 0;  // 0 unknown, 1 available, -1 refused
    static void** vt = nullptr;
    static uintptr_t fn = 0;
    if (state == 0) {
        state = -1;
        const ExecutableVersion* v = IdentifiedVersion();
        if (v == nullptr) {
            Log("compat: original tract developer vtable NOT available (executable not identified)");
        } else if (!CheckImageVtable("tract developer vtable entry 0", v, v->tractDeveloperVtableVA) ||
                   v->tractDeveloperGrowVA == 0 ||
                   !CheckCode("tract developer Grow", v->tractDeveloperGrowVA, v->imageBase)) {
            Log("compat: original tract developer vtable NOT available (address rejected)");
        } else {
            vt = reinterpret_cast<void**>(static_cast<uintptr_t>(v->tractDeveloperVtableVA));
            fn = static_cast<uintptr_t>(v->tractDeveloperGrowVA);
            state = 1;
            Log("compat: original tract developer vtable at 0x%08lX, Grow (slot 5) 0x%08lX",
                static_cast<unsigned long>(v->tractDeveloperVtableVA), static_cast<unsigned long>(fn));
        }
    }
    if (state != 1) return false;
    if (vtable != nullptr) *vtable = vt;
    if (grow != nullptr) *grow = fn;
    return true;
}

const void* BuildingDevelopmentVtable() {
    static int state = 0;  // 0 unknown, 1 available, -1 refused
    static const void* vtable = nullptr;
    if (state == 0) {
        state = -1;
        const ExecutableVersion* v = IdentifiedVersion();
        if (v != nullptr && CheckImageVtable("BDS vtable entry 0", v, v->bdsVtableVA)) {
            vtable = reinterpret_cast<const void*>(static_cast<uintptr_t>(v->bdsVtableVA));
            state = 1;
            Log("compat: original building development simulator vtable at 0x%08lX",
                static_cast<unsigned long>(v->bdsVtableVA));
        } else {
            Log("compat: original building development simulator vtable NOT available (%s)",
                v ? "address rejected" : "executable not identified");
        }
    }
    return state == 1 ? vtable : nullptr;
}

const void* GameDelayWindowVtable() {
    static int state = 0;  // 0 unknown, 1 available, -1 refused
    static const void* vtable = nullptr;
    if (state == 0) {
        state = -1;
        const ExecutableVersion* v = IdentifiedVersion();
        if (v != nullptr && CheckImageVtable("cSC4WinGameDelay vtable entry 0", v, v->gameDelayWindowVtableVA)) {
            vtable = reinterpret_cast<const void*>(static_cast<uintptr_t>(v->gameDelayWindowVtableVA));
            state = 1;
            Log("compat: cSC4WinGameDelay vtable at 0x%08lX", static_cast<unsigned long>(v->gameDelayWindowVtableVA));
        } else {
            Log("compat: cSC4WinGameDelay vtable NOT available (%s); the dialog closer cannot recognise "
                "SC4GameDelay's modal window",
                v ? "address rejected" : "executable not identified");
        }
    }
    return state == 1 ? vtable : nullptr;
}

uintptr_t DirectCall(Direct which) {
    static const char* const kNames[] = {"RemoveConnection",      "SetWateredState", "air dispatch Init",
                                         "air dispatch Shutdown", "map operator[]",  "map subtree free",
                                         "global heap free",      "global heap allocate",
                                         "lot pool allocate",     "lot constructor",
                                         "GenericConfirmationDialog",
                                         "AddRetainingWallOccupant", "UpdateRetainingWalls",
                                         "UpdateSurroundingRetainingWalls", "burnt building SetBoundsFromOccupant",
                                         "advisor win proc constructor", "advisor win proc Init",
                                         "advisor win proc show", "advisor win proc hide",
                                         "advisor win proc teardown", "lua tool_plop_network",
                                         "lua tool_plop_zone", "lua tool_plop_building", "lua tool_plop_flora",
                                         "lua tool_button", "lua window_budget", "lua window_map",
                                         "lua window_graph", "lua window_query", "lua camera_jump",
                                         "lua camera_zoom", "lua random_chance",
                                         "lua _dofile_from_resource", "lua print",
                                         "UpdateResourcesFromDirectory", "LoadPackageScripts",
                                         "placeable pool allocate", "placeable constructor",
                                         "texture binding AddRef", "texture binding Release",
                                         "pole GetAttachmentPoints",
                                         "terrain view Init", "terrain view Shutdown",
                                         "overlay land rectangle", "overlay dynamic land rectangle",
                                         "overlay extension land rectangle", "overlay dynamic extension land rectangle",
                                         "overlay water rectangle", "overlay dynamic water rectangle",
                                         "overlay shadow rectangle", "overlay dynamic shadow rectangle",
                                         "effects collection construct", "effects collection publish",
                                         "EffectsParticleConstruct",
                                         "EffectsDecalConstruct",
                                         "EffectsDynamicConstruct",
                                         "EffectsBrushConstruct",
                                         "EffectsAttractorConstruct",
                                         "EffectsScrubberConstruct",
                                         "EffectsSequenceConstruct",
                                         "EffectsSoundConstruct",
                                         "EffectsCameraConstruct",
                                         "EffectsParticleReadOld",
                                         "EffectsParticleRead",
                                         "EffectsParticleWrite",
                                         "EffectsDecalRead",
                                         "EffectsDecalWrite",
                                         "EffectsDynamicRead",
                                         "EffectsDynamicWrite",
                                         "EffectsVisualReadOld",
                                         "EffectsVisualRead",
                                         "EffectsVisualWrite",
                                         "EffectsParticleDrop",
                                         "EffectsDynamicDestroy",
                                         "terrain StaticInit",
                                         "effects visual names grow", "effects visual name insert",
                                         "overlay depth offset", "overlay packed draw state",
                                         "overlay transparency", "overlay shade", "overlay self-lit shade",
                                         "overlay reset transparency", "overlay reset shade",
                                         "overlay texture transform", "overlay clear texture transform",
                                         "overlay filtering", "overlay wrap", "overlay combiner",
                                         "overlay texture state", "overlay environment colour",
                                         "overlay alpha enable", "overlay alpha function",
                                         "dispatch TrackOccupant", "dispatch UntrackOccupant",
                                         "dispatch ShowAllIndicators",
                                         "timer counter",
                                         "tutorial registry GetItem", "tutorial registry SetActive",
                                         "tutorial window constructor", "tutorial window Init",
                                         "tutorial window Close", "tutorial window event",
};
    static_assert(sizeof(kNames) / sizeof(kNames[0]) == static_cast<unsigned>(Direct::Count), "one name per target");
    static int state[static_cast<unsigned>(Direct::Count)] = {};  // 0 unknown, 1 available, -1 refused
    static uintptr_t address[static_cast<unsigned>(Direct::Count)] = {};
    const unsigned i = static_cast<unsigned>(which);
    if (i >= static_cast<unsigned>(Direct::Count)) return 0;
    if (state[i] == 0) {
        state[i] = -1;
        const ExecutableVersion* v = IdentifiedVersion();
        if (v == nullptr) {
            Log("compat: direct call %s NOT available (executable not identified)", kNames[i]);
        } else if (v->directVA[i] == 0 || !CheckCode(kNames[i], v->directVA[i], v->imageBase)) {
            Log("compat: direct call %s NOT available (address rejected)", kNames[i]);
        } else {
            address[i] = static_cast<uintptr_t>(v->directVA[i]);
            state[i] = 1;
        }
    }
    return state[i] == 1 ? address[i] : 0;
}

const void* AuraSimulatorVtable() {
    static int state = 0;  // 0 unknown, 1 available, -1 refused
    static const void* vtable = nullptr;
    if (state == 0) {
        state = -1;
        const ExecutableVersion* v = IdentifiedVersion();
        MEMORY_BASIC_INFORMATION mbi{};
        const uintptr_t va = v ? v->auraSimulatorVtableVA : 0;
        bool ok = v != nullptr && va != 0 && va % 4 == 0 &&
                  VirtualQuery(reinterpret_cast<const void*>(va), &mbi, sizeof(mbi)) == sizeof(mbi) &&
                  mbi.State == MEM_COMMIT && mbi.Type == MEM_IMAGE &&
                  reinterpret_cast<uintptr_t>(mbi.AllocationBase) == v->imageBase &&
                  !(mbi.Protect & (PAGE_GUARD | PAGE_NOACCESS));
        if (ok) {
            // The first entry (QueryInterface) must be code inside the image.
            const uintptr_t first = *reinterpret_cast<const uintptr_t*>(va);
            ok = CheckCode("cISC4AuraSimulator vtable entry 0", first, v->imageBase);
        }
        if (ok) {
            vtable = reinterpret_cast<const void*>(va);
            state = 1;
            Log("compat: original cISC4AuraSimulator vtable at 0x%08lX", static_cast<unsigned long>(va));
        } else {
            Log("compat: original cISC4AuraSimulator vtable NOT available (%s); the aura shadow cannot read the "
                "original's state",
                v ? "address rejected" : "executable not identified");
        }
    }
    return state == 1 ? vtable : nullptr;
}

const void* NeighborsSimulatorVtable() {
    static int state = 0;  // 0 unknown, 1 available, -1 refused
    static const void* vtable = nullptr;
    if (state == 0) {
        state = -1;
        const ExecutableVersion* v = IdentifiedVersion();
        MEMORY_BASIC_INFORMATION mbi{};
        const uintptr_t va = v ? v->neighborsSimulatorVtableVA : 0;
        bool ok = v != nullptr && va != 0 && va % 4 == 0 &&
                  VirtualQuery(reinterpret_cast<const void*>(va), &mbi, sizeof(mbi)) == sizeof(mbi) &&
                  mbi.State == MEM_COMMIT && mbi.Type == MEM_IMAGE &&
                  reinterpret_cast<uintptr_t>(mbi.AllocationBase) == v->imageBase &&
                  !(mbi.Protect & (PAGE_GUARD | PAGE_NOACCESS));
        if (ok) {
            // The first entry (QueryInterface) must be code inside the image.
            const uintptr_t first = *reinterpret_cast<const uintptr_t*>(va);
            ok = CheckCode("cISC4NeighborsSimulator vtable entry 0", first, v->imageBase);
        }
        if (ok) {
            vtable = reinterpret_cast<const void*>(va);
            state = 1;
            Log("compat: original cISC4NeighborsSimulator vtable at 0x%08lX", static_cast<unsigned long>(va));
        } else {
            Log("compat: original cISC4NeighborsSimulator vtable NOT available (%s); the neighbours shadow cannot "
                "read the original's state bytes",
                v ? "address rejected" : "executable not identified");
        }
    }
    return state == 1 ? vtable : nullptr;
}

const void* CommercialSimulatorVtable() {
    static int state = 0;  // 0 unknown, 1 available, -1 refused
    static const void* vtable = nullptr;
    if (state == 0) {
        state = -1;
        const ExecutableVersion* v = IdentifiedVersion();
        MEMORY_BASIC_INFORMATION mbi{};
        const uintptr_t va = v ? v->commercialSimulatorVtableVA : 0;
        bool ok = v != nullptr && va != 0 && va % 4 == 0 &&
                  VirtualQuery(reinterpret_cast<const void*>(va), &mbi, sizeof(mbi)) == sizeof(mbi) &&
                  mbi.State == MEM_COMMIT && mbi.Type == MEM_IMAGE &&
                  reinterpret_cast<uintptr_t>(mbi.AllocationBase) == v->imageBase &&
                  !(mbi.Protect & (PAGE_GUARD | PAGE_NOACCESS));
        if (ok) {
            // The first entry (QueryInterface) must be code inside the image.
            const uintptr_t first = *reinterpret_cast<const uintptr_t*>(va);
            ok = CheckCode("cISC4CommercialSimulator vtable entry 0", first, v->imageBase);
        }
        if (ok) {
            vtable = reinterpret_cast<const void*>(va);
            state = 1;
            Log("compat: original cISC4CommercialSimulator vtable at 0x%08lX", static_cast<unsigned long>(va));
        } else {
            Log("compat: original cISC4CommercialSimulator vtable NOT available (%s); the commercial shadow cannot "
                "read the original's dirty byte and state",
                v ? "address rejected" : "executable not identified");
        }
    }
    return state == 1 ? vtable : nullptr;
}

bool DisasterHookEntry(DisasterHook which, void*** entry, uintptr_t* function) {
    static int state[static_cast<unsigned>(DisasterHook::Count)] = {};  // 0 unknown, 1 available, -1 refused
    static void** entries[static_cast<unsigned>(DisasterHook::Count)] = {};
    const unsigned i = static_cast<unsigned>(which);
    if (i >= static_cast<unsigned>(DisasterHook::Count)) return false;
    const ExecutableVersion* v = IdentifiedVersion();
    if (state[i] == 0) {
        state[i] = -1;
        const DisasterHookSite site = v ? v->disasterHooks[i] : DisasterHookSite{};
        const uintptr_t slotVA = site.vtableVA + 4u * site.slot;
        MEMORY_BASIC_INFORMATION mbi{};
        bool ok = v != nullptr && site.vtableVA != 0 && site.vtableVA % 4 == 0 &&
                  VirtualQuery(reinterpret_cast<const void*>(slotVA), &mbi, sizeof(mbi)) == sizeof(mbi) &&
                  mbi.State == MEM_COMMIT && mbi.Type == MEM_IMAGE &&
                  reinterpret_cast<uintptr_t>(mbi.AllocationBase) == v->imageBase &&
                  !(mbi.Protect & (PAGE_GUARD | PAGE_NOACCESS));
        if (ok) ok = CheckCode("disaster hook function", site.functionVA, v->imageBase);
        if (ok) {
            entries[i] = reinterpret_cast<void**>(slotVA);
            state[i] = 1;
        } else {
            Log("compat: disaster hook %u (vtable 0x%08lX slot %u) NOT available (%s)", i,
                static_cast<unsigned long>(site.vtableVA), static_cast<unsigned>(site.slot),
                v ? "address rejected" : "executable not identified");
        }
    }
    if (state[i] != 1) return false;
    if (entry != nullptr) *entry = entries[i];
    if (function != nullptr) *function = v->disasterHooks[i].functionVA;
    return true;
}

bool QueryToolFilterVtables(const void** hover, const void** describe, const void** record) {
    const auto* v = IdentifiedVersion();
    if (!v || v->imageBase != 0x00400000 ||
        reinterpret_cast<uintptr_t>(GetModuleHandleW(nullptr)) != v->imageBase) return false;
    // Fixed addresses are used only for the identified supported image.
    constexpr uintptr_t tables[] = {0x00A909B0, 0x00A90B18, 0x00AA42B0};
    for (uintptr_t va : tables) {
        MEMORY_BASIC_INFORMATION mbi{};
        if (VirtualQuery(reinterpret_cast<const void*>(va), &mbi, sizeof(mbi)) != sizeof(mbi) ||
            mbi.State != MEM_COMMIT || mbi.Type != MEM_IMAGE ||
            reinterpret_cast<uintptr_t>(mbi.AllocationBase) != v->imageBase ||
            (mbi.Protect & (PAGE_GUARD | PAGE_NOACCESS)) ||
            !CheckCode("query filter entry", *reinterpret_cast<const uintptr_t*>(va), v->imageBase)) return false;
    }
    *hover = reinterpret_cast<const void*>(tables[0]);
    *describe = reinterpret_cast<const void*>(tables[1]);
    *record = reinterpret_cast<const void*>(tables[2]);
    return true;
}

const void* ParkManagerVtable() {
    static int state = 0;  // 0 unknown, 1 available, -1 refused
    static const void* vtable = nullptr;
    if (state == 0) {
        state = -1;
        const ExecutableVersion* v = IdentifiedVersion();
        MEMORY_BASIC_INFORMATION mbi{};
        const uintptr_t va = v ? v->parkManagerVtableVA : 0;
        bool ok = v != nullptr && va != 0 && va % 4 == 0 &&
                  VirtualQuery(reinterpret_cast<const void*>(va), &mbi, sizeof(mbi)) == sizeof(mbi) &&
                  mbi.State == MEM_COMMIT && mbi.Type == MEM_IMAGE &&
                  reinterpret_cast<uintptr_t>(mbi.AllocationBase) == v->imageBase &&
                  !(mbi.Protect & (PAGE_GUARD | PAGE_NOACCESS));
        if (ok) {
            // The first entry (QueryInterface) must be code inside the image.
            const uintptr_t first = *reinterpret_cast<const uintptr_t*>(va);
            ok = CheckCode("cISC4ParkManager vtable entry 0", first, v->imageBase);
        }
        if (ok) {
            vtable = reinterpret_cast<const void*>(va);
            state = 1;
            Log("compat: original cISC4ParkManager vtable at 0x%08lX", static_cast<unsigned long>(va));
        } else {
            Log("compat: original cISC4ParkManager vtable NOT available (%s); the parks shadow cannot read the "
                "original's latch and threshold",
                v ? "address rejected" : "executable not identified");
        }
    }
    return state == 1 ? vtable : nullptr;
}

void** AdvisorSystemGlobal() {
    static int state = 0;  // 0 unknown, 1 available, -1 refused
    static void** slot = nullptr;
    if (state == 0) {
        state = -1;
        const ExecutableVersion* v = IdentifiedVersion();
        if (v == nullptr) {
            Log("compat: advisor system global NOT available (executable not identified)");
        } else if (v->advisorSystemGlobalVA == 0 || v->advisorSystemGlobalVA % 4 != 0 ||
                   !CheckAddress("advisor system global", v->advisorSystemGlobalVA, v->imageBase)) {
            Log("compat: advisor system global NOT available (address rejected)");
        } else {
            slot = reinterpret_cast<void**>(static_cast<uintptr_t>(v->advisorSystemGlobalVA));
            state = 1;
        }
    }
    return state == 1 ? slot : nullptr;
}

void** TutorialSystemGlobal() {
    static void** slot = []() -> void** {
        const ExecutableVersion* v = IdentifiedVersion();
        if (!v || !v->tutorialSystemGlobalVA || v->tutorialSystemGlobalVA % 4 != 0 ||
            !CheckAddress("tutorial system global", v->tutorialSystemGlobalVA, v->imageBase)) return nullptr;
        return reinterpret_cast<void**>(static_cast<uintptr_t>(v->tutorialSystemGlobalVA));
    }();
    return slot;
}

bool CaptureNetworkLotRandom(uint32_t* words625) {
    static const uint32_t* source = nullptr;
    static bool checked = false;
    if (!checked) {
        checked = true;
        const auto* v = IdentifiedVersion();
        if (v && CheckAddress("network lot random", v->networkLotRandomVA, v->imageBase) &&
            CheckAddress("network lot random end", v->networkLotRandomVA + 0x9C4, v->imageBase)) {
            source = reinterpret_cast<const uint32_t*>(v->networkLotRandomVA);
        }
    }
    if (!source || !words625) return false;
    const int32_t left = static_cast<int32_t>(source[625]);
    if (left < 0 || left > 624 || source[624] != reinterpret_cast<uintptr_t>(source + 624 - left)) return false;
    std::memcpy(words625, source, 624 * sizeof(uint32_t));
    words625[624] = 624 - left;
    return true;
}

bool CaptureNetworkLotPending(const void* manager, uint32_t* buckets) {
    if (!IdentifiedVersion() || !manager || !buckets) return false;
    const auto* words = static_cast<const uint32_t*>(manager);
    // §8.5: bucket vector +0x78/+0x7C, count +0x84. Called
    // between handlers only; no copying of native nodes or output order.
    if (words[0x84 / 4] != 0 || words[0x7C / 4] < words[0x78 / 4]) return false;
    const uint32_t bytes = words[0x7C / 4] - words[0x78 / 4];
    if (bytes % 4 != 0 || bytes == 0) return false;
    *buckets = bytes / 4;
    return true;
}

const ExecutableVersion* SupportedVersions(int* count) {
    if (count != nullptr) *count = static_cast<int>(sizeof(kVersions) / sizeof(kVersions[0]));
    return kVersions;
}

bool LimitTimersAvailable() {
    if (g_resolution == Resolution::Unknown) Resolve();
    return g_resolution == Resolution::Available && !g_disabledAfterCheck;
}

bool PublishLimitTimers(float cyclesToMs, float msToCycles) {
    if (!LimitTimersAvailable()) return false;
    if (!Plausible(cyclesToMs) || !Plausible(msToCycles)) return false;
    // Re-check before every write that the cells still hold plausible floats
    // (1.0f or a calibrated pair); stop for good if not.
    const float c2m = *g_cyclesToMs;
    const float m2c = *g_msToCycles;
    if (!((c2m == 1.0f && m2c == 1.0f) || ConsistentPair(c2m, m2c))) {
        g_disabledAfterCheck = true;
        Log("compat: WARNING limit-timer globals changed to implausible values (%.9g, %.9g); writes stopped", c2m,
            m2c);
        return false;
    }
    *g_cyclesToMs = cyclesToMs;
    *g_msToCycles = msToCycles;
    if (g_publishCount++ == 0) {
        Log("compat: first publish: CyclesToMillisecs=%.9g MillisecsToCycles=%.9g (%s)", cyclesToMs, msToCycles,
            g_version ? g_version->name : "?");
    }
    return true;
}

uint64_t PublishCount() { return g_publishCount; }

bool SpanRender4444Entries(void*** vtable, uintptr_t functions[5]) {
    static int state = 0;  // 0 unknown, 1 available, -1 refused
    static void** entries = nullptr;
    const ExecutableVersion* v = IdentifiedVersion();
    if (state == 0) {
        state = -1;
        const uintptr_t va = v ? v->spanRender4444VtableVA : 0;
        MEMORY_BASIC_INFORMATION mbi{};
        // Entries 0..5 (24 bytes) inside one committed image region.
        bool ok = v != nullptr && va != 0 && va % 4 == 0 &&
                  VirtualQuery(reinterpret_cast<const void*>(va), &mbi, sizeof(mbi)) == sizeof(mbi) &&
                  mbi.State == MEM_COMMIT && mbi.Type == MEM_IMAGE &&
                  reinterpret_cast<uintptr_t>(mbi.AllocationBase) == v->imageBase &&
                  !(mbi.Protect & (PAGE_GUARD | PAGE_NOACCESS)) &&
                  va + 24 <= reinterpret_cast<uintptr_t>(mbi.BaseAddress) + mbi.RegionSize;
        for (unsigned i = 0; ok && i < 5; ++i) ok = CheckCode("span4444 slot function", v->spanRender4444SlotVA[i], v->imageBase);
        if (ok) {
            entries = reinterpret_cast<void**>(va);
            state = 1;
        } else {
            Log("compat: span4444 vtable 0x%08lX NOT available (%s)", static_cast<unsigned long>(va),
                v ? "address rejected" : "executable not identified");
        }
    }
    if (state != 1) return false;
    if (vtable != nullptr) *vtable = entries;
    if (functions != nullptr) {
        for (unsigned i = 0; i < 5; ++i) functions[i] = v->spanRender4444SlotVA[i];
    }
    return true;
}

bool Span1555SlotEntry(unsigned slot, void*** entry, uintptr_t* function) {
    static const uintptr_t kVtable = 0x00AC182C;  // span-render-1555.md §1.1
    static const uintptr_t kSlots[6] = {0x0082FF00, 0x008310B0, 0x00831100, 0x00831200, 0x008312D0, 0x00831470};
    static int state[6] = {};  // 0 unknown, 1 available, -1 refused
    if (slot >= 6) return false;
    const ExecutableVersion* v = IdentifiedVersion();
    if (state[slot] == 0) {
        state[slot] = -1;
        const uintptr_t slotVA = kVtable + 4u * slot;
        MEMORY_BASIC_INFORMATION mbi{};
        bool ok = v == &kVersions[0] &&
                  VirtualQuery(reinterpret_cast<const void*>(slotVA), &mbi, sizeof(mbi)) == sizeof(mbi) &&
                  mbi.State == MEM_COMMIT && mbi.Type == MEM_IMAGE &&
                  reinterpret_cast<uintptr_t>(mbi.AllocationBase) == v->imageBase &&
                  !(mbi.Protect & (PAGE_GUARD | PAGE_NOACCESS));
        if (ok) ok = CheckCode("span1555 slot function", kSlots[slot], v->imageBase);
        if (ok) {
            state[slot] = 1;
        } else {
            Log("compat: span1555 vtable 0x%08lX slot %u NOT available (%s)", static_cast<unsigned long>(kVtable),
                slot, v ? "address rejected" : "executable not identified");
        }
    }
    if (state[slot] != 1) return false;
    if (entry != nullptr) *entry = reinterpret_cast<void**>(kVtable + 4u * slot);
    if (function != nullptr) *function = kSlots[slot];
    return true;
}

bool NetworkOccupantSetPiece(void* occupant, uint32_t pieceId, uint8_t orientation) {
    static int state = 0;  // 0 unknown, 1 available, -1 refused
    static uintptr_t fn = 0;
    if (state == 0) {
        state = -1;
        const ExecutableVersion* v = IdentifiedVersion();
        if (v == nullptr) {
            Log("compat: network occupant SetPiece (slot 30) NOT available (executable not identified)");
        } else if (v->networkOccupantSetPieceVA == 0 ||
                   !CheckCode("cISC4NetworkOccupant slot 30 SetPiece", v->networkOccupantSetPieceVA, v->imageBase)) {
            Log("compat: network occupant SetPiece (slot 30) NOT available (address rejected)");
        } else {
            fn = v->networkOccupantSetPieceVA;
            state = 1;
        }
    }
    if (state != 1 || occupant == nullptr) return false;
    // The occupant's own slot 30 must be the specified method: another class (or a vtable we have no
    // contract for) is refused, not called.
    const uintptr_t* vtbl = *reinterpret_cast<const uintptr_t* const*>(occupant);
    if (vtbl == nullptr || vtbl[30] != fn) return false;
    using SetPieceFn = void(__thiscall*)(void* self, uint32_t id, uint32_t orientation);
    reinterpret_cast<SetPieceFn>(fn)(occupant, pieceId, static_cast<uint32_t>(orientation));
    return true;
}

bool SpanContextAddresses(SpanContextVA* out) {
    static int state = 0;  // 0 unknown, 1 available, -1 refused
    if (state == 0) {
        state = -1;
        const ExecutableVersion* v = IdentifiedVersion();
        MEMORY_BASIC_INFORMATION mbi{};
        const bool ok = v == &kVersions[0] &&
                        VirtualQuery(reinterpret_cast<const void*>(0x00AC17D0), &mbi, sizeof(mbi)) == sizeof(mbi) &&
                        mbi.State == MEM_COMMIT && mbi.Type == MEM_IMAGE &&
                        reinterpret_cast<uintptr_t>(mbi.AllocationBase) == v->imageBase &&
                        !(mbi.Protect & (PAGE_GUARD | PAGE_NOACCESS));
        if (ok) state = 1;
        else Log("compat: span context vtable 0x00AC17D0 NOT available (%s)", v ? "address rejected" : "executable not identified");
    }
    if (state != 1) return false;
    if (out != nullptr) {
        // span-render-4444.md §1.3; span-render-565.md §1.1 (the seven renderer call sites of 0x008302E0).
        static const SpanContextVA kVA = {
            0x00AC17D0, 0x00AC182C, 0x00AC185C,
            {0x00830647, 0x008305FC, 0x00830627, 0x008305E7, 0x00830591, 0x0083053B, 0x008304D1},
            0x00AC1874,  // glue-span8888-impl4: span-render-8888.md §1.1
        };
        *out = kVA;
    }
    return true;
}

// glue-span8888-impl4 (span-render-8888.md §1.1): the same checks as Span1555SlotEntry.
bool Span8888SlotEntry(unsigned slot, void*** entry, uintptr_t* function) {
    static const uintptr_t kVtable = 0x00AC1874;  // span-render-8888.md §1.1
    static const uintptr_t kSlots[6] = {0x0082FF00, 0x00854E10, 0x00854E60, 0x00854F70, 0x00855040, 0x008551D0};
    static int state[6] = {};  // 0 unknown, 1 available, -1 refused
    if (slot >= 6) return false;
    const ExecutableVersion* v = IdentifiedVersion();
    if (state[slot] == 0) {
        state[slot] = -1;
        const uintptr_t slotVA = kVtable + 4u * slot;
        MEMORY_BASIC_INFORMATION mbi{};
        bool ok = v == &kVersions[0] &&
                  VirtualQuery(reinterpret_cast<const void*>(slotVA), &mbi, sizeof(mbi)) == sizeof(mbi) &&
                  mbi.State == MEM_COMMIT && mbi.Type == MEM_IMAGE &&
                  reinterpret_cast<uintptr_t>(mbi.AllocationBase) == v->imageBase &&
                  !(mbi.Protect & (PAGE_GUARD | PAGE_NOACCESS));
        if (ok) ok = CheckCode("span8888 slot function", kSlots[slot], v->imageBase);
        if (ok) {
            state[slot] = 1;
        } else {
            Log("compat: span8888 vtable 0x%08lX slot %u NOT available (%s)", static_cast<unsigned long>(kVtable),
                slot, v ? "address rejected" : "executable not identified");
        }
    }
    if (state[slot] != 1) return false;
    if (entry != nullptr) *entry = reinterpret_cast<void**>(kVtable + 4u * slot);
    if (function != nullptr) *function = kSlots[slot];
    return true;
}

bool FeatureManagerSupported(const void* object) {
    const ExecutableVersion* v = IdentifiedVersion();
    MEMORY_BASIC_INFORMATION obj{}, table{};
    if (!v || !object || VirtualQuery(object, &obj, sizeof(obj)) != sizeof(obj) ||
        obj.State != MEM_COMMIT || (obj.Protect & (PAGE_GUARD | PAGE_NOACCESS)) ||
        reinterpret_cast<uintptr_t>(object) + 12 > reinterpret_cast<uintptr_t>(obj.BaseAddress) + obj.RegionSize)
        return false;
    const uintptr_t vt = *static_cast<const uintptr_t*>(object);
    if (vt != 0x00A88274 || VirtualQuery(reinterpret_cast<const void*>(vt), &table, sizeof(table)) != sizeof(table) ||
        table.State != MEM_COMMIT || table.Type != MEM_IMAGE ||
        reinterpret_cast<uintptr_t>(table.AllocationBase) != v->imageBase || table.Protect != PAGE_READONLY)
        return false;
    constexpr uintptr_t slots[] = {0x0046F740, 0x004BDF00, 0x0046F7F0, 0x0046FD50, 0x0046F770,
        0x0046FA80, 0x0046F780, 0x0046F7A0, 0x0046F7D0};
    const auto* entries = reinterpret_cast<const uintptr_t*>(vt);
    for (unsigned i = 0; i < 9; ++i)
        if (entries[i] != slots[i] || !CheckCode("feature manager slot", slots[i], v->imageBase)) return false;
    const int32_t count = static_cast<const int32_t*>(object)[2];
    return count > 0 && count < 0x7ffffff0;
}

bool DrawCaptureTables(void*** tables) {
    const auto* version = IdentifiedVersion();
    if (version != &kVersions[0]) return false;
    constexpr uintptr_t addresses[] = {0x00AC4660, 0x00AC36D0, 0x00ACC4D8};
    for (unsigned d = 0; d < 3; ++d) {
        if (!CheckImageVtable("draw capture driver", version, addresses[d])) return false;
        MEMORY_BASIC_INFORMATION memory{};
        auto** table = reinterpret_cast<void**>(addresses[d]);
        if (VirtualQuery(table, &memory, sizeof(memory)) != sizeof(memory) ||
            addresses[d] + 83 * sizeof(void*) > reinterpret_cast<uintptr_t>(memory.BaseAddress) + memory.RegionSize)
            return false;
        for (unsigned s = 0; s < 83; ++s)
            if (!CheckCode("draw capture slot", reinterpret_cast<uintptr_t>(table[s]), version->imageBase)) return false;
        tables[d] = table;
    }
    return true;
}
bool DrawStreamTables(void*** context, void*** tree, void*** list) {
    const auto* version = IdentifiedVersion();
    if (version != &kVersions[0]) return false;
    const uintptr_t tables[] = {0x00AC16C8, 0x00AE2FE0, 0x00AE1780};
    const unsigned slots[] = {35, 88, 88};
    const uintptr_t expected[] = {0x0082C0C0, 0x009D37C6, 0x009CA19A};
    for (unsigned i = 0; i < 3; ++i) {
        MEMORY_BASIC_INFORMATION memory{};
        const auto* table = reinterpret_cast<const uintptr_t*>(tables[i]);
        if (VirtualQuery(table, &memory, sizeof(memory)) != sizeof(memory) ||
            memory.State != MEM_COMMIT || memory.Type != MEM_IMAGE ||
            reinterpret_cast<uintptr_t>(memory.AllocationBase) != version->imageBase ||
            memory.Protect != PAGE_READONLY ||
            tables[i] + (slots[i] + 1) * sizeof(uintptr_t) > reinterpret_cast<uintptr_t>(memory.BaseAddress) + memory.RegionSize ||
            table[slots[i]] != expected[i] || !CheckCode("drawstream entry", expected[i], version->imageBase)) return false;
    }
    *context = reinterpret_cast<void**>(tables[0]);
    *tree = reinterpret_cast<void**>(tables[1]);
    *list = reinterpret_cast<void**>(tables[2]);
    return true;
}
uintptr_t MapViewDrawStreamEntry(unsigned which) {
    const auto* version = IdentifiedVersion();
    // _clock's cdecl/no-body-arguments/EAX transport is in docs/abi/functions.tsv.
    constexpr uintptr_t sites[] = {0x007A2F60, 0x0079ED90, 0x0079F230, 0x009F361F, 0x0099A276};
    if (version != &kVersions[0] || which >= 5 || !CheckCode("MapView drawstream", sites[which], version->imageBase)) return 0;
    return sites[which];
}
void** MapViewDrawStreamTable(unsigned which) {
    const auto* version = IdentifiedVersion();
    constexpr uintptr_t sites[] = {0x00AB838C, 0x00AB8140, 0x00AB7EE0, 0x00ABAB60};
    if (version != &kVersions[0] || which >= 4) return nullptr;
    MEMORY_BASIC_INFORMATION memory{};
    auto** table = reinterpret_cast<void**>(sites[which]);
    if (VirtualQuery(table, &memory, sizeof(memory)) != sizeof(memory) || memory.State != MEM_COMMIT ||
        memory.Type != MEM_IMAGE || reinterpret_cast<uintptr_t>(memory.AllocationBase) != version->imageBase ||
        memory.Protect != PAGE_READONLY) return nullptr;
    return table;
}
}  // namespace compat
}  // namespace fc4
