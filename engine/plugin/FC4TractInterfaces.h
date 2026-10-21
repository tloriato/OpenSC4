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

// The tract developer's interfaces as the game has them
// (docs/specs/tract-developer.md §7, §8), checked by
// tests/abi_conformance_test against docs/abi/interfaces.tsv:
//
//   cFC4ISC4TractDeveloper            cISC4TractDeveloper (IID 0xA818ED1B),
//                                     declared once, in FC4DevelopmentInterfaces.h
//                                     (the development lane calls it, the tract
//                                     lane implements it): slots 3-14 with the
//                                     compiled return channels of §7.1.
//   cFC4ISC4LotConfigurationManager   cISC4LotConfigurationManager (28 slots):
//                                     the SDK's two AddLotConfiguration
//                                     overloads under names of their own, slot
//                                     5 with one argument (pop 4, the SDK has
//                                     two), slot 9's set the game's STLport
//                                     hash_set (B). Containers the tract
//                                     developer does not use are void*.
// The configuration filter (§7.4) is the SDK's cISC4LotConfigurationFilter
// (its 4 slots match docs/abi).
#pragma once

#include "FC4DevelopmentInterfaces.h"
#include "FC4GameList.h"

#include "cIGZUnknown.h"

#include <atomic>
#include <cstdint>

class cGZPersistResourceKey;
class cISC4LotConfiguration;
class cISC4LotConfigurationFilter;
class cISC4LotConfigurationManager;
class cS3DRect2D;
class cSC4LotConfigurationObject;

static const uint32_t kFC4_CLSID_cSC4TractDeveloper = 0x2990C142;
static const uint32_t kFC4_IID_cISC4TractDeveloper = 0xA818ED1B;

namespace fc4 {

// Allocator B (docs/abi/allocators.md), or the test seam's (op 45) while one
// is installed: the outcome list's nodes, the active-style vector and the
// tract developer's own game containers. Install the test allocator before a
// container takes memory and remove it only after the last one is gone.
struct TractNodeAllocator {
    using AllocateFn = void* (*)(uint32_t size);
    using FreeFn = void (*)(void* p);
    static std::atomic<AllocateFn>& TestAllocate() {
        static std::atomic<AllocateFn> f{nullptr};
        return f;
    }
    static std::atomic<FreeFn>& TestFree() {
        static std::atomic<FreeFn> f{nullptr};
        return f;
    }
    static bool Available() { return TestAllocate().load() != nullptr || StlNodeAllocator::Available(); }
    static void* Allocate(uint32_t size) {
        if (AllocateFn a = TestAllocate().load()) return a(size);
        return StlNodeAllocator::Allocate(size);
    }
    static void Free(void* p) {
        if (p == nullptr) return;
        if (FreeFn f = TestFree().load()) return f(p);
        StlNodeAllocator::Free(p);
    }
    static const void* StringVtable() { return nullptr; }
};

// std::list<cISC4TractDeveloper::Outcome> (§9.2: B, 12-byte nodes): the
// tract developer appends int32 codes and never clears it.
using TractOutcomeList = GameList<int32_t, TractNodeAllocator>;
// std::vector<uint32_t> of slots 9 and 10 (B).
using TractStyleVector = GameVector<uint32_t, TractNodeAllocator>;
static_assert(sizeof(TractOutcomeList) == sizeof(GameList<int32_t>), "same list layout");
static_assert(sizeof(TractStyleVector) == sizeof(GameVector<uint32_t>), "same vector layout");

// The interface's containers (allocator B) seen with the tract allocator
// (B, or the test seam's): the layout does not depend on the allocator, and
// in the game both allocate from B.
inline TractOutcomeList& AsTract(GameList<int32_t>& l) { return reinterpret_cast<TractOutcomeList&>(l); }
inline const TractStyleVector& AsTract(const GameVector<uint32_t>& v) {
    return reinterpret_cast<const TractStyleVector&>(v);
}
inline GameVector<uint32_t>& AsGameB(TractStyleVector& v) { return reinterpret_cast<GameVector<uint32_t>&>(v); }

}  // namespace fc4

// cISC4LotConfigurationManager (vtable 0x00A8EC58, 28 slots).
//
// Corrected by the glue21 lane against prop-lotconfig.md B7 and the `ret`
// column of docs/abi/interfaces.tsv (no call site used the wrong shapes):
// B7's SDK error - the header declares slot 5 as
// AddLotConfiguration(cISC4LotConfiguration*, bool) and slot 6 as
// AddLotConfiguration(cGZPersistResourceKey const&, void*), but the game's
// slot 5 takes ONLY the key (pop 4) and slot 6 takes the configuration and the
// flag (pop 8). The set queries return the count in EAX (SDK: bool).
class cFC4ISC4LotConfigurationManager : public cIGZUnknown {
public:
    virtual bool Init() = 0;                                                                   // 3
    virtual bool Shutdown() = 0;                                                               // 4
    virtual bool AddLotConfigurationByKey(cGZPersistResourceKey const& key) = 0;               // 5, pop 4
    virtual bool AddLotConfiguration(cISC4LotConfiguration* config, bool add) = 0;             // 6, pop 8
    virtual bool RemoveLotConfiguration(uint32_t id) = 0;                                      // 7
    virtual cISC4LotConfiguration* GetLotConfiguration(uint32_t id) = 0;                       // 8  (no reference)
    // Inserts each matching configuration with one reference (the caller
    // releases each); does not clear; EAX = the count (B5).
    virtual uint32_t GetLotConfigurationsByFilter(fc4::GameHashSetU32<fc4::TractNodeAllocator>& configs,
                                                  cISC4LotConfigurationFilter* filter) = 0;    // 9
    virtual uint32_t GetLotConfigurationsByFamily(void* configs, uint32_t family) = 0;         // 10, count
    virtual uint32_t GetLotConfigurationsBySize(void* configs, uint32_t x, uint32_t z) = 0;    // 11, count
    virtual void ClearLotConfigurationSet(void* configs) = 0;                                  // 12, no return
    virtual uint32_t GetLotConfigurationIDsByFilter(void* ids, cISC4LotConfigurationFilter* filter) = 0;  // 13
    virtual uint32_t GetLotConfigurationIDsByFamily(void* ids, uint32_t family) = 0;           // 14, count
    virtual uint32_t GetLotConfigurationIDsBySize(void* ids, uint32_t x, uint32_t z) = 0;      // 15, count
    virtual bool LotConfigurationHasBuildingFamily(cISC4LotConfiguration* config, uint32_t family,
                                                   cSC4LotConfigurationObject** object) = 0;  // 16
    virtual bool LotConfigurationHasBuildingType(cISC4LotConfiguration* config, uint32_t type,
                                                 cSC4LotConfigurationObject** object) = 0;    // 17
    virtual void RegisterLotConfigurationFilter(cISC4LotConfigurationFilter* filter) = 0;      // 18, no return
    virtual void UnregisterLotConfigurationFilter(cISC4LotConfigurationFilter* filter) = 0;    // 19, no return
    virtual void* GetLotConfigurationMap() = 0;                                                // 20, EAX
    virtual void* GetLotConfigurationFamilyIDSet() = 0;                                        // 21, EAX
    virtual bool CreateGenericLotConfiguration(cISC4LotConfiguration*& config, uint32_t id) = 0;  // 22
    virtual uint32_t FixLotConfiguration(cISC4LotConfiguration* config) = 0;                   // 23, EAX code
    // B5.5-B5.6: no defined return value (interfaces.tsv eax-or-void).
    virtual void GetMaxFootprintForBuildingFamilies(void const* families, cS3DRect2D& rect) = 0;  // 24
    virtual void GetMaxFootprintForFloraFamilies(void const* families, cS3DRect2D& rect) = 0;     // 25
    virtual void GetMaxFootprintForPropFamilies(void const* families, cS3DRect2D& rect) = 0;      // 26
    virtual void GetObjectIDArray(cSC4LotConfigurationObject* object, void* ids) = 0;          // 27
    // slot 28: the implementing class's `virtual ~` (scalar deleting dtor).
};

namespace fc4 {
// The manager travels as the SDK's pointer (city slot 37); single
// inheritance, vtable at offset 0. Call it only through this declaration.
inline cFC4ISC4LotConfigurationManager* AsGame(cISC4LotConfigurationManager* p) {
    return reinterpret_cast<cFC4ISC4LotConfigurationManager*>(p);
}
}  // namespace fc4
