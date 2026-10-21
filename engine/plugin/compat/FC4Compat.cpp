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
            0x0075C420,  // terrain 3D view Init (direct-calls.md; terrain.md §15.5.4)
            0x0074FE40,  // cSTETerrain::StaticInit (terrain.md §15.5.12.2; TR40)
            0x007560F0,  // terrain 3D view Shutdown (terrain.md §15.5.11 item 15a; TR35)
            0x0045BA70,  // SC4GetIGZDBSegment (terrain.md §13.2; TR39)
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
                                         "terrain 3D view Init",
                                         "cSTETerrain::StaticInit", "terrain 3D view Shutdown",
                                         "SC4GetIGZDBSegment"};
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

}  // namespace compat
}  // namespace fc4
